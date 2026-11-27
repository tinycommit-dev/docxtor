//! The real-world corpus run.
//!
//! It opens .docx files that other people made (the Apache POI and LibreOffice test documents, fetched into a cache
//! folder by `npm run corpus:fetch`, never into the repository) and checks, for each one:
//!
//! - it opens inside the budget (time, memory, stack), or is refused with a clean coded error (too big, protected by
//!   a password, not a Word file, damaged). A panic, a hang, a budget breach or an internal error text is a failure;
//! - E3 open then save with no edit gives the same bytes for every part (only for files that needed no repair);
//! - E5 saving twice gives the same bytes;
//! - E1/E2 the saved file's XML parts parse and declare their namespaces, and the package is consistent. A problem
//!   the original file already had does not count, only one Docxtor adds;
//! - E6 the render model of the document equals that of `open(save(document))`;
//! - random steps (the harness of `tests/random_ops.rs`, sharing `common/random.rs`) with every invariant check after
//!   each step.
//!
//! Report only: where the file has a table of contents with the results Word computed, update it and compare the
//! entry texts (page numbers left out) with what the file held.
//!
//! Each file is checked in a worker process (`common/proc.rs`: 1 MB stack, a time limit, a 1 GB memory cap), so a
//! panic, hang or memory blow-up is one failed file and not a dead run.
//!
//! Environment:
//!   `DOCXTOR_CORPUS_DIR`       the cache folder (default ~/.cache/docxtor-corpus)
//!   `DOCXTOR_CORPUS=all`       every file instead of the pinned sample of 100 in manifest.json
//!   `DOCXTOR_CORPUS_STEPS=n`   random steps per file (default 20; use 200 for a full run)
//!   `DOCXTOR_CORPUS_OUT=file`  where the JSON results go (default <cache>/results.json)
//!   `DOCXTOR_CORPUS_ONLY=text` only files whose path holds the text
//!   `DOCXTOR_CORPUS_STEPS_OUT=file`  the random steps of a file that failed them, to replay (a debug build names an op that changed a block
//!                          without marking it)
//!   `DOCXTOR_CORPUS_THREADS`, `DOCXTOR_CORPUS_LIMIT_S`, `DOCXTOR_CORPUS_OPEN_MS`, `DOCXTOR_CORPUS_MEM_MB`, `DOCXTOR_CORPUS_SEED`
//!
//! `cargo test --release --test corpus -- --nocapture` for the numbers. A failure that is not in
//! `common/known_corpus.rs` fails the test; with no files in the cache folder it prints that it was skipped and passes.

#![allow(clippy::type_complexity)]

mod common;
#[path = "common/known_corpus.rs"]
mod known_corpus;
#[path = "common/proc.rs"]
mod proc;
#[path = "common/random.rs"]
mod random;

use common::*;
use docxtor_core::invariants::check_package;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::package::Package;
use docxtor_core::{Document, Op};
use random::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOC: proc::Counting = proc::Counting;

const WORKER_ENV: &str = "DOCXTOR_CORPUS_WORKER";

fn env_num(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn corpus_dir() -> PathBuf {
    match std::env::var("DOCXTOR_CORPUS_DIR") {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache").join("docxtor-corpus"),
    }
}

fn file_limit() -> Duration {
    Duration::from_secs(env_num("DOCXTOR_CORPUS_LIMIT_S", if cfg!(debug_assertions) { 240 } else { 60 }))
}

fn open_limit_ms() -> u64 {
    env_num("DOCXTOR_CORPUS_OPEN_MS", if cfg!(debug_assertions) { 10_000 } else { 2_000 })
}

fn mem_mb() -> usize {
    env_num("DOCXTOR_CORPUS_MEM_MB", 1024) as usize
}

fn stack_bytes() -> usize {
    if cfg!(debug_assertions) {
        8 << 20
    } else {
        1 << 20
    }
}

// ---------------------------------------------------------------------------
// One file (runs in a worker)
// ---------------------------------------------------------------------------

fn shapes() -> Arc<Vec<Value>> {
    static S: OnceLock<Arc<Vec<Value>>> = OnceLock::new();
    S.get_or_init(|| Arc::new(load_shapes())).clone()
}

/// Part names with their numbers taken out, so `word/header3.xml` and `word/header4.xml` are one kind of part.
fn part_class(name: &str) -> String {
    let mut out = String::new();
    let mut d = false;
    for c in name.chars() {
        if c.is_ascii_digit() {
            if !d {
                out.push('N');
            }
            d = true;
        } else {
            d = false;
            out.push(c);
        }
    }
    out
}

fn short(s: &str, n: usize) -> String {
    let one: String = s.replace('\n', " ");
    if one.chars().count() > n {
        format!("{}...", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

/// The notes that say the file was repaired or rewritten while it was read: E3 does not apply to those.
fn needed_repair(report: &docxtor_core::open::Report) -> bool {
    report.converted || report.notes.iter().any(|n| matches!(n.code.as_str(), "zip-directory" | "zip-damaged" | "xml-recovered" | "xml-unreadable" | "text-cut" | "rels-restored" | "types-restored" | "strict"))
}

/// The entries of every table of contents as Word wrote them: the text before the page number, per paragraph in a
/// `TOC1` to `TOC9` style.
fn toc_entries(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| {
        let style = p.ppr.as_ref().and_then(|x| x.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or("");
        let is_toc = style.len() == 4 && style.starts_with("TOC") && style.as_bytes()[3].is_ascii_digit() && style != "TOC0";
        if is_toc {
            let t = p.plain_text();
            // Without the page number; the spaces and tab between a number and its heading count as one space.
            let t = match t.rsplit_once('\t') {
                Some((a, page)) if !page.trim().is_empty() && !page.trim().contains(' ') => a,
                _ => &t,
            };
            let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
            v.push(t);
        }
    });
    v
}

fn check_file(case: &Value) -> Value {
    if let Some(s) = case["selftest"].as_str() {
        return selftest(s);
    }
    let path = case["path"].as_str().unwrap_or("");
    let steps = case["steps"].as_u64().unwrap_or(0) as usize;
    let seed = case["seed"].as_u64().unwrap_or(0);
    proc::stage("read");
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return json!({"outcome": "unreadable", "findings": [{"check": "harness", "message": format!("cannot read the file: {e}")}]}),
    };
    let mut findings: Vec<Failure> = Vec::new();
    let mut ran: Vec<&str> = Vec::new();
    let mut known_random = 0;

    proc::stage("open");
    let t0 = Instant::now();
    let opened = docxtor_core::open::open(&bytes, "corpus.docx");
    let open_ms = t0.elapsed().as_millis() as u64;
    let mut o = match opened {
        Ok(o) => o,
        Err(e) => {
            // A clean, coded refusal is a pass; an error that shows the insides of the program is not.
            if let Some(m) = internal_text(&e.message) {
                findings.push(Failure::new("text", format!("an internal error text ({m}) \"{}\"", short(&e.message, 80))));
            }
            return json!({"outcome": format!("refused:{}", e.code), "bytes": bytes.len(), "open_ms": open_ms, "ran": ["open"], "findings": findings_json(&findings)});
        }
    };
    ran.push("open");
    if open_ms > open_limit_ms() {
        findings.push(Failure::new("budget", format!("opening took {open_ms} ms, more than the limit of {} ms", open_limit_ms())));
    }
    let outcome = if o.report.converted { format!("converted:{:?}", o.report.format).to_lowercase() } else if needed_repair(&o.report) { "opened:repaired".to_string() } else { "opened".to_string() };

    // The saved file: E3, E5, E1/E2, E6.
    proc::stage("save");
    let saved = o.doc.save();
    let saved = match saved {
        Ok(s) => Some(s),
        Err(e) => {
            findings.push(Failure::new("E20", format!("save failed: {}", short(&e.to_string(), 120))));
            None
        }
    };
    if let Some(a) = &saved {
        proc::stage("E3");
        if !needed_repair(&o.report) {
            ran.push("E3");
            if let (Ok(orig), Ok(now)) = (Package::open(&bytes), Package::open(a)) {
                let mut diff: BTreeSet<String> = BTreeSet::new();
                for p in &orig.parts {
                    match now.get(&p.name) {
                        None => {
                            diff.insert(format!("{} is gone", part_class(&p.name)));
                        }
                        Some(d) if d != &p.data[..] => {
                            diff.insert(part_class(&p.name));
                        }
                        _ => {}
                    }
                }
                for p in &now.parts {
                    if orig.get(&p.name).is_none() {
                        diff.insert(format!("{} is new", part_class(&p.name)));
                    }
                }
                if !diff.is_empty() {
                    let list: Vec<String> = diff.iter().take(6).cloned().collect();
                    findings.push(Failure::new("E3", format!("open then save with no edit changed: {}{}", list.join(", "), if diff.len() > 6 { ", and more" } else { "" })));
                }
            }
        }
        proc::stage("E5");
        ran.push("E5");
        match o.doc.save() {
            Ok(b) if &b != a => findings.push(Failure::new("E5", format!("saving twice without an edit gives different bytes: {}", bytes_diff_text(a, &b)))),
            Ok(_) => {}
            Err(e) => findings.push(Failure::new("E20", format!("the second save failed: {}", short(&e.to_string(), 120)))),
        }
        proc::stage("E1/E2");
        ran.push("E1");
        ran.push("E2");
        // What the file already breaks does not count. A file that needed repair is compared with its repaired self: the
        // repair is the engine's reading of damage that was there, and only what saving adds is the engine's to answer for.
        let repaired = if needed_repair(&o.report) && !o.report.converted { docxtor_core::open::open_package(&bytes, &docxtor_core::budget::Budget::file()).ok().and_then(|(p, _)| p.to_bytes().ok()) } else { None };
        let baseline: HashSet<_> = check_package(repaired.as_deref().unwrap_or(&bytes)).into_iter().collect();
        for v in check_package(a) {
            if baseline.contains(&v) || docxtor_core::invariants_known::is_known(&v) {
                continue;
            }
            findings.push(Failure::new(v.rule, short(&v.message, 200)));
        }
        proc::stage("E6");
        ran.push("E6");
        match docxtor_core::open::open(a, "saved.docx") {
            Err(e) => findings.push(Failure::new("E6", format!("the saved file does not open again: {}", short(&e.message, 120)))),
            Ok(again) => {
                if let Some(d) = reopened_diff(&o.doc, &again.doc) {
                    findings.push(Failure::new("E6", format!("open(save(doc)) renders differently: {d}")));
                }
            }
        }
    }

    // Report only: a table of contents Word computed, updated and compared.
    proc::stage("toc");
    let before = toc_entries(&o.doc);
    let mut toc = Value::Null;
    if !before.is_empty() {
        match o.doc.apply_batch(&[Op::UpdateToc { pages: None, pages_only: false }], false) {
            Err(e) => toc = json!({"entries": before.len(), "error": short(&e.to_string(), 160)}),
            Ok(_) => {
                let after = toc_entries(&o.doc);
                if after == before {
                    toc = json!({"entries": before.len(), "same": true});
                } else {
                    let at = before.iter().zip(&after).position(|(a, b)| a != b).unwrap_or(before.len().min(after.len()));
                    toc = json!({"entries": before.len(), "same": false, "after": after.len(), "first_difference": at, "was": before.get(at).map(|s| short(s, 100)), "now": after.get(at).map(|s| short(s, 100))});
                }
            }
        }
    }
    drop(o);

    // Random steps with every invariant check after each.
    if steps > 0 {
        proc::stage("random steps");
        ran.push("random");
        let start = Start::Bytes(path.to_string(), Arc::new(bytes));
        let stats = Arc::new(Mutex::new(Stats::default()));
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut g = Generator { rng: Rng::new(seed), shapes: shapes(), graveyard: vec![], n: steps };
        // The steps as the replay reads them, written before each step runs, so a run the worker does not live through (out of
        // memory, a hang) leaves them too: `DOCXTOR_REPLAY=<out> cargo test --test random_ops replay_a_failure -- --ignored`.
        let steps_out = std::env::var("DOCXTOR_CORPUS_STEPS_OUT").ok();
        let mut so_far: Vec<serde_json::Value> = Vec::new();
        let (label, out_now) = (path.to_string(), steps_out.clone());
        let found = guarded(&start, log.clone(), stats, move |d, i| {
            let step = g.step(d, i);
            if let (Some(out), Some(s)) = (&out_now, &step) {
                so_far.push(s.to_json());
                let v = serde_json::json!({ "start": label, "failed": ["the run did not finish"], "steps": so_far });
                let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap());
            }
            step
        });
        // The steps of a file whose random steps failed, with what failed (with `DOCXTOR_CORPUS_ONLY` naming that file).
        if let (false, Some(out)) = (found.is_empty(), steps_out) {
            let steps: Vec<serde_json::Value> = log.lock().unwrap().iter().map(|s| s.to_json()).collect();
            let failed: Vec<String> = found.iter().map(|f| format!("{}: {}", f.key(), f.message)).collect();
            let v = serde_json::json!({ "start": path, "failed": failed, "steps": steps });
            let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap());
        }
        for f in found {
            if f.known() {
                known_random += 1;
            } else {
                findings.push(f);
            }
        }
    }
    json!({"outcome": outcome, "bytes": case["bytes"], "open_ms": open_ms, "ran": ran, "known_random": known_random, "toc": toc, "findings": findings_json(&findings)})
}

/// How two saved files differ, in words, with part numbers taken out.
fn bytes_diff_text(a: &[u8], b: &[u8]) -> String {
    part_class(&bytes_diff(a, b))
}

fn findings_json(f: &[Failure]) -> Value {
    Value::Array(f.iter().map(|x| json!({"check": x.check, "message": short(&x.message, 400)})).collect())
}

/// Tests of the harness itself.
fn selftest(what: &str) -> Value {
    match what {
        "panic" => panic!("on purpose"),
        "hang" => loop {
            std::hint::black_box(0);
        },
        _ => json!({"outcome": "opened", "findings": []}),
    }
}

#[test]
fn corpus_worker_entry() {
    proc::serve(WORKER_ENV, stack_bytes(), mem_mb(), check_file);
}

// ---------------------------------------------------------------------------
// The supervisor
// ---------------------------------------------------------------------------

/// A name for the group of checks a finding belongs to (for the pass and fail counts).
fn group(check: &str) -> &'static str {
    match check {
        "E1" => "E1",
        "E2" => "E2",
        "E3" => "E3",
        "E5" => "E5",
        "E6" => "E6",
        "budget" | "text" => "open (budget, error text)",
        "panic" | "stack" | "memory" | "hang" | "abort" | "harness" => "open (crash, hang, memory)",
        _ => "random steps",
    }
}

/// The group a `ran` entry stands for.
fn ran_group(r: &str) -> &'static str {
    match r {
        "E1" => "E1",
        "E2" => "E2",
        "E3" => "E3",
        "E5" => "E5",
        "E6" => "E6",
        "open" => "open (budget, error text)",
        _ => "random steps",
    }
}

struct Row {
    rel: String,
    bytes: u64,
    outcome: String,
    ms: u64,
    peak_mb: u64,
    open_ms: u64,
    ran: Vec<String>,
    findings: Vec<(String, String, String)>, // check, message, key
    known_random: u64,
    toc: Value,
}

fn crash_findings(ended: &proc::Ended) -> Vec<Failure> {
    match ended {
        proc::Ended::Panic { text, stage } => {
            let (msg, file) = text.rsplit_once(" at ").unwrap_or((text, ""));
            let file = file.split("crates/").last().unwrap_or(file);
            vec![Failure::new("panic", format!("panic: {} at {file} in {stage}", short(msg, 160)))]
        }
        proc::Ended::Hang { ms, stage } => vec![Failure::new("hang", format!("no answer in {} s in {stage}", ms / 1000))],
        proc::Ended::Died { kind, detail, stage } => vec![Failure::new(kind, format!("{detail} in {stage}"))],
        proc::Ended::Done { .. } => vec![],
    }
}

fn run_cases(cases: Vec<Value>) -> Vec<Row> {
    let threads = env_num("DOCXTOR_CORPUS_THREADS", std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(2).clamp(1, 4) as u64) as usize;
    let queue = Arc::new(Mutex::new(cases.into_iter().collect::<VecDeque<_>>()));
    let out = Arc::new(Mutex::new(Vec::new()));
    let done = Arc::new(Mutex::new(0usize));
    let total = queue.lock().unwrap().len();
    let hs: Vec<_> = (0..threads.min(total.max(1)))
        .map(|_| {
            let (queue, out, done) = (queue.clone(), out.clone(), done.clone());
            std::thread::spawn(move || {
                let mut w = proc::Proc::new(WORKER_ENV, "corpus_worker_entry", mem_mb());
                loop {
                    let Some(case) = queue.lock().unwrap().pop_front() else { break };
                    let ended = w.run(&case, file_limit());
                    let rel = case["rel"].as_str().unwrap_or("").to_string();
                    let mut row = Row { rel, bytes: case["bytes"].as_u64().unwrap_or(0), outcome: "crashed".into(), ms: 0, peak_mb: 0, open_ms: 0, ran: vec![], findings: vec![], known_random: 0, toc: Value::Null };
                    let mut fs: Vec<Failure> = crash_findings(&ended);
                    if let proc::Ended::Done { value, ms, peak_mb } = &ended {
                        row.ms = *ms;
                        row.peak_mb = *peak_mb;
                        row.outcome = value["outcome"].as_str().unwrap_or("?").to_string();
                        row.open_ms = value["open_ms"].as_u64().unwrap_or(0);
                        row.ran = value["ran"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                        row.known_random = value["known_random"].as_u64().unwrap_or(0);
                        row.toc = value["toc"].clone();
                        for f in value["findings"].as_array().into_iter().flatten() {
                            fs.push(Failure::new(f["check"].as_str().unwrap_or("?"), f["message"].as_str().unwrap_or("")));
                        }
                    }
                    row.findings = fs.iter().map(|f| (f.check.clone(), f.message.clone(), f.key())).collect();
                    let n = {
                        let mut d = done.lock().unwrap();
                        *d += 1;
                        *d
                    };
                    if n % 50 == 0 || n == total {
                        eprintln!("corpus: {n} of {total} files");
                    }
                    out.lock().unwrap().push(row);
                }
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    let mut rows = std::mem::take(&mut *out.lock().unwrap());
    rows.sort_by(|a, b| a.rel.cmp(&b.rel));
    rows
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The files to check: the pinned sample of the manifest, or every .docx in the cache with `DOCXTOR_CORPUS=all`.
fn pick_files(dir: &std::path::Path) -> Vec<String> {
    let mut rels: Vec<String> = Vec::new();
    if std::env::var("DOCXTOR_CORPUS").as_deref() == Ok("all") {
        fn walk(base: &std::path::Path, d: &std::path::Path, out: &mut Vec<String>) {
            let Ok(rd) = std::fs::read_dir(d) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(base, &p, out);
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("docx")) {
                    out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        for src in ["poi", "libreoffice"] {
            walk(dir, &dir.join(src), &mut rels);
        }
        rels.sort();
    } else if let Ok(text) = std::fs::read_to_string(dir.join("manifest.json")) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            rels = v["sample"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        }
    }
    if let Ok(only) = std::env::var("DOCXTOR_CORPUS_ONLY") {
        rels.retain(|r| r.contains(&only));
    }
    rels.retain(|r| dir.join(r).is_file());
    rels
}

fn bucket_key(check: &str, key: &str) -> String {
    format!("{check}|{key}")
}

#[test]
fn corpus_run() {
    let dir = corpus_dir();
    let rels = pick_files(&dir);
    if rels.is_empty() {
        let msg = format!("corpus: skipped, no files at {} (run npm run corpus:fetch)", dir.display());
        println!("\n{msg}\n");
        eprintln!("\n{msg}\n");
        return;
    }
    let all = std::env::var("DOCXTOR_CORPUS").as_deref() == Ok("all");
    let steps = env_num("DOCXTOR_CORPUS_STEPS", 20);
    let base_seed = env_num("DOCXTOR_CORPUS_SEED", 1);
    let cases: Vec<Value> = rels
        .iter()
        .map(|r| {
            let p = dir.join(r);
            json!({"rel": r, "path": p.to_string_lossy(), "bytes": std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0), "steps": steps, "seed": fnv(r) ^ base_seed})
        })
        .collect();
    let t0 = Instant::now();
    let rows = run_cases(cases);
    let secs = t0.elapsed().as_secs_f64();

    // Outcomes, per-check counts, buckets.
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    let mut checks: BTreeMap<&str, (usize, usize)> = BTreeMap::new(); // group -> (ran, failed)
    let mut buckets: BTreeMap<String, (String, String, usize, String, bool)> = BTreeMap::new(); // key -> (check, message, n, example, known)
    let mut toc_files = (0, 0, 0);
    let mut known_random = 0;
    for r in &rows {
        *outcomes.entry(r.outcome.clone()).or_default() += 1;
        known_random += r.known_random;
        let mut ran: BTreeSet<&str> = r.ran.iter().map(|x| ran_group(x)).collect();
        ran.insert("open (crash, hang, memory)");
        for g in &ran {
            checks.entry(g).or_default().0 += 1;
        }
        let failed: BTreeSet<&str> = r.findings.iter().map(|f| group(&f.0)).collect();
        for g in failed {
            let e = checks.entry(g).or_default();
            e.1 += 1;
            if !ran.contains(g) {
                e.0 += 1;
            }
        }
        for (check, message, key) in &r.findings {
            let known = known_corpus::is_known(check, message);
            let e = buckets.entry(bucket_key(check, key)).or_insert((check.clone(), message.clone(), 0, r.rel.clone(), known));
            e.2 += 1;
        }
        if r.toc.is_object() {
            toc_files.0 += 1;
            if r.toc["same"] == json!(false) {
                toc_files.1 += 1;
            } else if r.toc["error"].is_string() {
                toc_files.2 += 1;
            }
        }
    }
    let mut list: Vec<(&String, &(String, String, usize, String, bool))> = buckets.iter().collect();
    list.sort_by(|a, b| b.1 .2.cmp(&a.1 .2).then(a.0.cmp(b.0)));

    // The JSON results.
    let out_path = std::env::var("DOCXTOR_CORPUS_OUT").map(PathBuf::from).unwrap_or_else(|_| dir.join("results.json"));
    let results = json!({
        "run": {"mode": if all { "all" } else { "sample" }, "files": rows.len(), "steps": steps, "seconds": secs, "build": if cfg!(debug_assertions) { "debug" } else { "release" }},
        "outcomes": outcomes,
        "checks": checks.iter().map(|(k, (ran, failed))| (k.to_string(), json!({"ran": ran, "failed": failed}))).collect::<BTreeMap<_, _>>(),
        "buckets": list.iter().map(|(k, (check, message, n, example, known))| json!({"key": k, "check": check, "message": message, "files": n, "example": example, "known": known})).collect::<Vec<_>>(),
        "toc": {"files_with_a_toc": toc_files.0, "entries_differ": toc_files.1, "update_refused": toc_files.2},
        "files": rows.iter().map(|r| json!({"file": r.rel, "bytes": r.bytes, "outcome": r.outcome, "ms": r.ms, "open_ms": r.open_ms, "peak_mb": r.peak_mb, "ran": r.ran, "toc": r.toc, "findings": r.findings.iter().map(|f| json!({"check": f.0, "message": f.1, "key": f.2})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
    });
    let _ = std::fs::write(&out_path, serde_json::to_string_pretty(&results).unwrap());

    // The summary.
    println!("\ncorpus: {} files ({}), {steps} random steps each, {:.0} s, {} build; results in {}", rows.len(), if all { "all" } else { "the pinned sample" }, secs, if cfg!(debug_assertions) { "debug" } else { "release" }, out_path.display());
    println!("\nwhat happened to each file:");
    for (k, n) in &outcomes {
        println!("  {n:>5}  {k}");
    }
    println!("\nchecks (files that ran it, files that failed it):");
    for (k, (ran, failed)) in &checks {
        println!("  {k:<30} ran {ran:>5}  failed {failed:>4}");
    }
    if known_random > 0 {
        println!("  ({known_random} failure(s) of a bug already listed in common/known.rs were skipped)");
    }
    println!("\ntables of contents (report only): {} files hold one; {} differ after an update; {} refused the update", toc_files.0, toc_files.1, toc_files.2);
    println!("\nfailure buckets ({}), most files first; [known] is in common/known_corpus.rs:", list.len());
    for (_, (check, message, n, example, known)) in &list {
        println!("  {n:>4} files  {check}: {}  {}\n              e.g. {example}", short(message, 150), if *known { "[known]" } else { "" });
    }
    let new: Vec<_> = list.iter().filter(|(_, v)| !v.4).collect();
    if !new.is_empty() {
        let lines: Vec<String> = new.iter().take(25).map(|(_, (check, message, n, example, _))| format!("  {n} files  {check}: {}\n      e.g. {example}", short(message, 200))).collect();
        panic!("{} new corpus failure(s) that are not in common/known_corpus.rs:\n{}\nfull list: {}", new.len(), lines.join("\n"), out_path.display());
    }
}

/// The worker plumbing: a panic and a hang are each one failed file and the run goes on.
#[test]
fn corpus_harness_survives_a_panic_and_a_hang() {
    let mut w = proc::Proc::new(WORKER_ENV, "corpus_worker_entry", 256);
    let panic = w.run(&json!({"selftest": "panic"}), Duration::from_secs(30));
    assert!(matches!(&panic, proc::Ended::Panic { text, .. } if text.contains("on purpose")), "{panic:?}");
    let hang = w.run(&json!({"selftest": "hang"}), Duration::from_secs(2));
    assert!(matches!(hang, proc::Ended::Hang { .. }), "{hang:?}");
    let fine = w.run(&json!({"selftest": "fine"}), Duration::from_secs(30));
    assert!(matches!(fine, proc::Ended::Done { .. }), "{fine:?}");
    let f = crash_findings(&panic);
    assert_eq!(f[0].check, "panic");
}
