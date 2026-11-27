//! Random op sequences.
//!
//! A run opens a document (a fixture or one built in code), then takes steps: an op or a short batch of ops from a
//! generator that reads the live document (real paragraph ids, offsets inside the paragraph, bookmark and tab names that
//! exist; one step in ten uses a wrong one on purpose), or a meta step (undo, redo, undo several, save and reopen and
//! carry on with the reopened document, save and keep going). After every step the harness checks, on top of the
//! engine's own invariant check (which panics in a debug build): E8 apply then undo gives the same model, E9 undo then
//! redo, E10 a refused batch changes nothing (model, and the saved bytes on strict steps), E20 no panic (each run is in
//! a thread with a 1 MB stack, as WASM has), and that `render_changed` kept a step-by-step mirror equal to a full render
//! (RC). Every 20 steps and at the end: E1/E2 (`check_package`), E5 (saving twice gives the same bytes), E6
//! (`open(save(doc))` renders like the document, ids and part numbers aside).
//!
//! Besides ~70 hand-written kinds, the generator replays op shapes recorded from the whole test suite
//! (`tests/data/op_shapes.jsonl`) with ids and positions swapped for live ones. To record again and rebuild it:
//!
//!     DOCXTOR_RECORD_OPS=/tmp/ops.jsonl cargo test
//!     DOCXTOR_RECORDED=/tmp/ops.jsonl cargo test --test random_ops reduce_recorded_ops -- --ignored
//!
//! A failing run prints its seed, then removes steps one at a time while it still fails, and prints the short list of
//! steps as JSON (ops ready to paste into a normal test).
//!
//! Modes:
//!   `cargo test`                 a fixed set of seeds, about a minute in a debug build.
//!   `DOCXTOR_RANDOM_SECS=600 cargo test --release --test random_ops -- --nocapture`
//!                                new seeds for that long on every core; prints each distinct failure once
//!                                (`DOCXTOR_RANDOM_SEED=n` starts from seed n, `DOCXTOR_RANDOM_THREADS=n` sets the threads).
//!   `DOCXTOR_RANDOM_REPLAY=<seed>:<start>:<steps>` is not needed: a failure prints everything to paste.
//!
//! Failures that are real bugs are listed in `common/known.rs` with their reasons; a failure that is a mistake of the
//! harness is fixed here instead.

mod common;
#[path = "common/features.rs"]
mod features;
#[path = "common/random.rs"]
mod random;

use common::*;
use random::*;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::Document;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Running a seed, shrinking, reporting
// ---------------------------------------------------------------------------

struct Outcome {
    failures: Vec<Failure>,
    steps: Vec<Step>,
}

fn shapes() -> Arc<Vec<Value>> {
    Arc::new(load_shapes())
}

fn run_seed(start: &Start, seed: u64, n: usize, shapes: &Arc<Vec<Value>>, stats: &Arc<Mutex<Stats>>) -> Option<Outcome> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut g = Generator { rng: Rng::new(seed), shapes: shapes.clone(), graveyard: vec![], n };
    let failures = guarded(start, log.clone(), stats.clone(), move |d, i| g.step(d, i));
    let steps = log.lock().unwrap().clone();
    if failures.is_empty() {
        None
    } else {
        Some(Outcome { failures, steps })
    }
}

fn replay(start: &Start, steps: &[Step]) -> Vec<Failure> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let list = steps.to_vec();
    guarded(start, log, Arc::new(Mutex::new(Stats::default())), move |_, i| list.get(i).cloned())
}

/// Remove steps (and then ops of batches) one at a time while the run still fails the same way.
fn shrink(start: &Start, mut steps: Vec<Step>, key: &str, secs: u64) -> Vec<Step> {
    let same = |s: &[Step]| replay(start, s).iter().any(|f| f.key() == key);
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut changed = true;
    while changed && Instant::now() < deadline {
        changed = false;
        // Big chunks first (halves, quarters, ...), then single steps.
        let mut chunk = steps.len() / 2;
        while chunk > 1 && Instant::now() < deadline {
            let mut at = 0;
            while at < steps.len() && Instant::now() < deadline {
                let mut t = steps.clone();
                let end = (at + chunk).min(t.len());
                t.drain(at..end);
                if same(&t) {
                    steps = t;
                    changed = true;
                } else {
                    at += chunk;
                }
            }
            chunk /= 2;
        }
        let mut i = steps.len();
        while i > 0 && Instant::now() < deadline {
            i -= 1;
            let mut t = steps.clone();
            t.remove(i);
            if same(&t) {
                steps = t;
                changed = true;
            }
        }
        for i in 0..steps.len() {
            if let Step::Apply { ops, .. } = &steps[i] {
                let mut k = ops.len();
                while k > 1 {
                    k -= 1;
                    let mut t = steps.clone();
                    if let Step::Apply { ops, .. } = &mut t[i] {
                        if k < ops.len() {
                            ops.remove(k);
                        }
                    }
                    if same(&t) {
                        steps = t;
                        changed = true;
                    }
                }
            }
        }
    }
    steps
}

fn report(start: &Start, seed: u64, f: &Failure, steps: &[Step]) -> String {
    let list: Vec<Value> = steps.iter().map(Step::to_json).collect();
    format!("{}\nseed {seed}, start {}\nsteps ({}):\n{}", f.key(), start.label(), steps.len(), serde_json::to_string(&list).unwrap())
}

fn print_stats(stats: &Stats) {
    println!("steps run: {}", stats.steps);
    println!("{:<34} {:>7} {:>7} {:>7} {:>7}", "kind", "tried", "applied", "refused", "badjson");
    for (k, v) in &stats.kinds {
        println!("{k:<34} {:>7} {:>7} {:>7} {:>7}", v.0, v.1, v.2, v.3);
    }
}

fn starts() -> Vec<Start> {
    let mut v: Vec<Start> = fixture_files().into_iter().map(Start::File).collect();
    v.push(Start::Built("rich"));
    v.push(Start::Built("plain"));
    v
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// A fixed set of seeds on one start document. Known failures are skipped; the first other one shrinks and fails.
fn fixed(start: Start, seeds: std::ops::Range<u64>, steps: usize) {
    if std::env::var("DOCXTOR_RANDOM_SECS").is_ok() {
        return;
    }
    let shapes = shapes();
    let stats = Arc::new(Mutex::new(Stats::default()));
    let mut known = 0;
    for seed in seeds {
        let Some(o) = run_seed(&start, seed, steps, &shapes, &stats) else { continue };
        let (old, new): (Vec<_>, Vec<_>) = o.failures.iter().cloned().partition(Failure::known);
        known += old.len();
        if let Some(f) = new.first() {
            let small = shrink(&start, o.steps, &f.key(), 40);
            let others: Vec<String> = new.iter().skip(1).map(|f| f.key()).collect();
            panic!("random op failure:\n{}\nalso found in the same run: {others:?}\nfull message: {}", report(&start, seed, f, &small), f.message);
        }
    }
    if std::env::var("DOCXTOR_RANDOM_STATS").is_ok() {
        print_stats(&stats.lock().unwrap());
    }
    if known > 0 {
        println!("{known} failure(s) hit a known bug");
    }
}

#[test]
fn random_ops_simple() {
    fixed(Start::File("simple.docx".into()), 0..6, 40);
}

#[test]
fn random_ops_rich() {
    fixed(Start::File("rich.docx".into()), 100..105, 40);
}

#[test]
fn random_ops_complex() {
    fixed(Start::File("complex.docx".into()), 200..205, 40);
}

#[test]
fn random_ops_long() {
    fixed(Start::File("long.docx".into()), 300..302, 40);
}

#[test]
fn random_ops_built() {
    fixed(Start::Built("rich"), 400..408, 50);
}

#[test]
fn random_ops_plain() {
    fixed(Start::Built("plain"), 500..508, 50);
}

/// The long mode: `DOCXTOR_RANDOM_SECS=600 cargo test --release --test random_ops -- --nocapture`. Threads search for
/// the time given (the first run of each distinct failure is kept); then each distinct failure is shrunk, in parallel.
#[test]
fn random_ops_for_a_while() {
    let Ok(secs) = std::env::var("DOCXTOR_RANDOM_SECS") else { return };
    let secs: u64 = secs.parse().unwrap_or(60);
    let base: u64 = std::env::var("DOCXTOR_RANDOM_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() % 1_000_000 * 1000).unwrap_or(1));
    let threads: usize = std::env::var("DOCXTOR_RANDOM_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 8));
    let deadline = Instant::now() + Duration::from_secs(secs);
    let shapes = shapes();
    let stats = Arc::new(Mutex::new(Stats::default()));
    // key -> (times seen, seed, start, the run's steps, the failure)
    type Entry = (usize, u64, Start, Vec<Step>, Failure);
    let seen: Arc<Mutex<BTreeMap<String, Entry>>> = Arc::new(Mutex::new(BTreeMap::new()));
    let starts = starts();
    let runs = Arc::new(Mutex::new(0usize));
    let hs: Vec<_> = (0..threads)
        .map(|t| {
            let (shapes, stats, seen, starts, runs) = (shapes.clone(), stats.clone(), seen.clone(), starts.clone(), runs.clone());
            std::thread::spawn(move || {
                let mut seed = base + t as u64 * 100_000;
                while Instant::now() < deadline {
                    seed += 1;
                    let start = &starts[(seed as usize) % starts.len()];
                    let n = 30 + (seed as usize * 7) % 90;
                    *runs.lock().unwrap() += 1;
                    // If a stack overflow kills the process, this says which run did it.
                    let _ = std::fs::write(std::env::temp_dir().join(format!("docxtor_random_current_{t}.txt")), format!("seed {seed}, start {}, steps {n}", start.label()));
                    if let Some(o) = run_seed(start, seed, n, &shapes, &stats) {
                        for f in &o.failures {
                            let mut s = seen.lock().unwrap();
                            match s.get_mut(&f.key()) {
                                Some(e) => e.0 += 1,
                                None => {
                                    s.insert(f.key(), (1, seed, start.clone(), o.steps.clone(), f.clone()));
                                }
                            }
                        }
                    }
                }
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    print_stats(&stats.lock().unwrap());
    let total_runs = *runs.lock().unwrap();
    let entries: Vec<(String, Entry)> = std::mem::take(&mut *seen.lock().unwrap()).into_iter().collect();
    println!("\n{total_runs} runs, {} distinct failures; shrinking each", entries.len());
    let texts: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let counts: Vec<(String, usize)> = entries.iter().map(|(k, e)| (k.clone(), e.0)).collect();
    let queue = Arc::new(Mutex::new(entries));
    let hs: Vec<_> = (0..threads)
        .map(|_| {
            let (queue, texts) = (queue.clone(), texts.clone());
            std::thread::spawn(move || loop {
                let Some((key, (n, seed, start, steps, f))) = queue.lock().unwrap().pop() else { break };
                let small = shrink(&start, steps, &key, 90);
                let text = format!("{}{}\nseen in {n} run(s)\nfull message: {}", if f.known() { "(known) " } else { "" }, report(&start, seed, &f, &small), f.message);
                println!("\n=== FAILURE ===\n{text}\n");
                texts.lock().unwrap().push((key, text));
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    println!("\n{total_runs} runs, {} distinct failures", counts.len());
    for (k, n) in &counts {
        println!("  x{n}  {k}");
    }
    if let Ok(path) = std::env::var("DOCXTOR_RANDOM_OUT") {
        let mut all = texts.lock().unwrap().clone();
        all.sort();
        let _ = std::fs::write(path, all.iter().map(|s| s.1.as_str()).collect::<Vec<_>>().join("\n\n"));
    }
}

// ---------------------------------------------------------------------------
// Building the recorded shapes
// ---------------------------------------------------------------------------

/// Reduce the ops recorded from the whole test suite (see the top of this file) to a few examples of each shape:
/// one shape is an op name and its top-level keys (and the `action` or `type` that selects a variant).
#[test]
#[ignore]
fn reduce_recorded_ops() {
    let path = std::env::var("DOCXTOR_RECORDED").expect("set DOCXTOR_RECORDED to the file DOCXTOR_RECORD_OPS wrote");
    let text = std::fs::read_to_string(path).unwrap();
    let mut by_shape: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut seen_text: HashSet<String> = HashSet::new();
    for line in text.lines() {
        let Ok(Value::Array(batch)) = serde_json::from_str::<Value>(line) else { continue };
        for op in batch {
            let Value::Object(m) = &op else { continue };
            let s = op.to_string();
            if s.len() > 3000 || !seen_text.insert(s) {
                continue;
            }
            let name = m.get("op").and_then(Value::as_str).unwrap_or("?");
            let variant = ["action", "type", "kind"].iter().filter_map(|k| m.get(*k).and_then(Value::as_str)).collect::<Vec<_>>().join("/");
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let shape = format!("{name}|{variant}|{}", keys.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(","));
            let e = by_shape.entry(shape).or_default();
            // Keep a few, the smallest first.
            e.push(op);
            e.sort_by_key(|o| o.to_string().len());
            e.truncate(3);
        }
    }
    let mut out = String::new();
    let mut n = 0;
    for ops in by_shape.values() {
        for op in ops {
            out.push_str(&op.to_string());
            out.push('\n');
            n += 1;
        }
    }
    let dest = repo_path("crates/docxtor-core/tests/data/op_shapes.jsonl");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(&dest, out).unwrap();
    println!("{} shapes from {} distinct ops", n, seen_text.len());
}

#[test]
fn the_start_documents_open_and_have_live_positions() {
    let mut graveyard = vec![];
    for s in starts() {
        let doc = s.open();
        let live = Live::of(&doc, &mut graveyard);
        assert!(!live.blocks.is_empty(), "{}", s.label());
        let mut n = 0;
        walk_paragraphs(&doc.body, &mut |_| n += 1);
        assert!(n > 0);
    }
}

// ---------------------------------------------------------------------------
// Replaying a failure
// ---------------------------------------------------------------------------

impl Step {
    fn from_json(v: &Value) -> Step {
        match v["step"].as_str().unwrap_or("") {
            "undo" => Step::Undo,
            "redo" => Step::Redo,
            "undo_steps" => Step::UndoN(v["n"].as_u64().unwrap_or(2) as usize),
            "save_and_reopen" => Step::SaveReopen,
            "save" => Step::Save,
            _ => Step::Apply {
                ops: v["ops"].as_array().cloned().unwrap_or_default(),
                merge: v["merge"].as_bool().unwrap_or(false),
                strict: v["strict"].as_bool().unwrap_or(false),
                kind: v["kind"].as_str().unwrap_or("replayed").to_string(),
            },
        }
    }
}

/// Every failure found and fixed, as the steps that set it off (tests/data/replays, one file each, saying why): they replay
/// clean. Add one with each fix the random search leads to.
#[test]
fn fixed_failures_stay_fixed() {
    let dir = repo_path("crates/docxtor-core/tests/data/replays");
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    assert!(!files.is_empty());
    let mut broken = Vec::new();
    for f in &files {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        let start = match v["start"].as_str().unwrap_or("built:plain") {
            "built:rich" => Start::Built("rich"),
            "built:plain" => Start::Built("plain"),
            other => Start::File(other.to_string()),
        };
        let steps: Vec<Step> = v["steps"].as_array().unwrap().iter().map(Step::from_json).collect();
        for fail in replay(&start, &steps) {
            broken.push(format!("{}: {} ({})", f.file_name().unwrap().to_string_lossy(), fail.message, v["why"].as_str().unwrap_or("")));
        }
    }
    assert!(broken.is_empty(), "{broken:#?}");
}

/// Run the steps a failure printed again:
/// `DOCXTOR_REPLAY=steps.json cargo test --test random_ops replay_a_failure -- --ignored --nocapture`.
/// The file holds `{"start": "<file name in fixtures/, or built:rich, or built:plain>", "steps": [...]}`, the steps
/// being the JSON list the failure printed. Paste the ops into a normal test once the bug is fixed.
#[test]
#[ignore]
fn replay_a_failure() {
    let path = std::env::var("DOCXTOR_REPLAY").expect("set DOCXTOR_REPLAY to a JSON file");
    // A path from the top of the repository works too (cargo runs tests in the crate's folder).
    let path = if std::path::Path::new(&path).exists() { std::path::PathBuf::from(&path) } else { repo_path(&path) };
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap();
    let start = match v["start"].as_str().unwrap_or("built:plain") {
        "built:rich" => Start::Built("rich"),
        "built:plain" => Start::Built("plain"),
        f => Start::File(f.to_string()),
    };
    let steps: Vec<Step> = v["steps"].as_array().unwrap().iter().map(Step::from_json).collect();
    let failures = replay(&start, &steps);
    for f in &failures {
        println!("{}\n  {}", f.key(), f.message);
    }
    assert!(failures.is_empty(), "{} failure(s)", failures.len());
}

// ---------------------------------------------------------------------------
// A file for every feature (for the Open XML validator and LibreOffice, see common/features.rs)
// ---------------------------------------------------------------------------

/// Writes one saved file per op kind (and per recorded shape) and a few documents built like a person or an AI builds
/// them; every file must be a valid package that opens again, and every op kind must be covered or listed with its
/// reason. `DOCXTOR_FEATURES_OUT=<dir>` keeps the files for `npm run corpus:validate` and `npm run corpus:lo` and makes the
/// full run (about 40 s in a release build, 11 minutes in a debug one); without it only the built documents are made, so
/// a plain `cargo test` stays quick. `DOCXTOR_FEATURES_SHAPES=kinds` makes one file per op kind and no more.
#[test]
fn feature_files() {
    quiet_panics_in_runs();
    let keep = std::env::var_os("DOCXTOR_FEATURES_OUT").map(std::path::PathBuf::from);
    let dir = keep.clone().unwrap_or_else(|| std::env::temp_dir().join(format!("docxtor-features-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir);
    let t0 = Instant::now();
    // Without a folder to keep them in, a plain `cargo test` makes the built documents only.
    let mode = match std::env::var("DOCXTOR_FEATURES_SHAPES") {
        Ok(m) => m,
        Err(_) if keep.is_none() => "none".into(),
        Err(_) => String::new(),
    };
    let r = features::write_all(&dir, &mode);
    println!("feature files: {} op kinds of {} have a file of their own, {} more by built documents, {} not covered; {} more shapes, {} built documents; made in {:.1} s, in {}", r.kinds, r.kinds + r.only_built.len() + r.uncovered.len(), r.only_built.len(), r.uncovered.len(), r.shapes, r.built, t0.elapsed().as_secs_f64(), r.dir.display());
    if !r.uncovered.is_empty() {
        for k in &r.uncovered {
            println!("not covered: {k}: {}", r.last_refusal.get(k).map(String::as_str).unwrap_or("no recorded shape"));
        }
    }
    let mut bad = vec![];
    for sub in ["kinds", "shapes", "built"] {
        let Ok(rd) = std::fs::read_dir(dir.join(sub)) else { continue };
        for e in rd.flatten() {
            let bytes = std::fs::read(e.path()).unwrap();
            let broken: Vec<String> = docxtor_core::invariants::check_package(&bytes).iter().map(|v| v.to_string()).collect();
            if !broken.is_empty() {
                bad.push(format!("{}: {}", e.path().display(), broken.join(" | ")));
            } else if Document::open(&bytes).is_err() {
                bad.push(format!("{}: does not open again", e.path().display()));
            }
        }
    }
    if keep.is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(bad.is_empty(), "feature files that are not valid packages:\n{}", bad.join("\n"));
    if mode == "none" {
        return;
    }
    let listed: Vec<&str> = features::NOT_COVERED.iter().map(|(k, _)| *k).collect();
    let unexplained: Vec<&String> = r.uncovered.iter().filter(|k| !listed.contains(&k.as_str())).collect();
    assert!(unexplained.is_empty(), "op kinds with no feature file and no reason in features::NOT_COVERED: {unexplained:?}");
    let stale: Vec<&&str> = listed.iter().filter(|k| !r.uncovered.iter().any(|u| u == **k)).collect();
    assert!(stale.is_empty(), "op kinds listed in features::NOT_COVERED that now have a file: {stale:?}");
}
