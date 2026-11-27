//! The damaged-file harness (invariant E20).
//!
//! It opens deliberately broken copies of the fixtures and of the other things the engine reads (RTF and HTML paste,
//! EMF/WMF/TIFF pictures, recipient lists, BibTeX, CSL, password-protected files, the JSON of an op) and checks that
//! every one ends in a document or an error: no panic, no stack overflow (in a 1 MB stack, the size WASM has), no
//! hang (more than 2 seconds in a release build) and no runaway memory (more than 1 GB), and that an error message
//! is not an internal one.
//!
//! A case is a small JSON description (`entry point`, `file`, `part`, `mutation`, `seed`) that builds the same broken
//! bytes every time (`common/mutations.rs`); the cases are listed in `common/mutate_cases.rs`.
//!
//! Why processes and not only threads: a thread cannot be stopped when it loops for ever, and a stack overflow or a
//! failed allocation ends the whole process in Rust, so a test that ran the cases in its own threads would die at the
//! first bug it was made to find. So the test runs itself again as a worker (`mutate_worker_entry`, started with an
//! environment variable) that reads one case at a time from stdin and runs it in a thread with a 1 MB stack, a counting
//! allocator that refuses memory past the limit, and the engine call as the only thing inside the clock. The test
//! watches the worker, ends it at the time limit, and when it dies says from its last words why (stack overflow, out
//! of memory) and in which stage. Panics are caught in the worker and reported with where they came from. At most
//! four workers run at once whatever the number of tests.
//!
//! Modes:
//!   `cargo test`                     a fixed set of cases, under a minute in a debug build.
//!   `DOCXTOR_MUTATE_SECS=600 cargo test --release --test mutate_open -- --nocapture`
//!                                    new seeds and sizes for that long; each distinct failure is shrunk (smaller
//!                                    counts and sizes) and printed once; `DOCXTOR_MUTATE_OUT=<file>` writes them.
//!                                    (`DOCXTOR_MUTATE_SEED=n`, `DOCXTOR_MUTATE_THREADS=n`, `DOCXTOR_MUTATE_LIMIT_MS=n`,
//!                                    `DOCXTOR_MUTATE_MEM_MB=n` change the start seed, workers, time and memory limit.)
//!   `DOCXTOR_MUTATE_REPLAY='<case json>' cargo test --test mutate_open replay_a_case -- --ignored --nocapture`
//!                                    runs one case a failure printed (`DOCXTOR_MUTATE_SAVE=<file>` also writes the
//!                                    broken bytes, to open in the app or the CLI).
//!
//! Failures that are real bugs are listed in `common/known_mutate.rs` with their reasons.

#![allow(clippy::type_complexity)]

mod common;
#[path = "common/known_mutate.rs"]
mod known_mutate;
#[path = "common/mutate_cases.rs"]
mod mutate_cases;
#[path = "common/mutations.rs"]
mod mutations;

use serde_json::{json, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// The counting allocator (switched on only inside a worker, only around the engine call)
// ---------------------------------------------------------------------------

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static LIMIT: AtomicUsize = AtomicUsize::new(usize::MAX);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        if n > LIMIT.load(Relaxed) {
            LIVE.fetch_sub(l.size(), Relaxed);
            return std::ptr::null_mut();
        }
        PEAK.fetch_max(n, Relaxed);
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let n = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        if n > LIMIT.load(Relaxed) {
            LIVE.fetch_sub(l.size(), Relaxed);
            return std::ptr::null_mut();
        }
        PEAK.fetch_max(n, Relaxed);
        System.alloc_zeroed(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        LIVE.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        if new > l.size() {
            let d = new - l.size();
            let n = LIVE.fetch_add(d, Relaxed) + d;
            if n > LIMIT.load(Relaxed) {
                LIVE.fetch_sub(d, Relaxed);
                return std::ptr::null_mut();
            }
            PEAK.fetch_max(n, Relaxed);
        } else {
            LIVE.fetch_sub(l.size() - new, Relaxed);
        }
        let q = System.realloc(p, l, new);
        if q.is_null() {
            if new > l.size() {
                LIVE.fetch_sub(new - l.size(), Relaxed);
            } else {
                LIVE.fetch_add(l.size() - new, Relaxed);
            }
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

// ---------------------------------------------------------------------------
// Every call into the engine, in one table. When a reader's signature changes (the budget), change it here.
// ---------------------------------------------------------------------------

/// What an entry point leaves behind: the stage it is in (printed as it goes, so a crash can be placed) and the error
/// texts it saw.
#[derive(Default)]
struct Trace {
    errors: Vec<String>,
}

impl Trace {
    fn stage(&self, s: &str) {
        println!("@@stage {s}");
        let _ = std::io::stdout().flush();
    }
}

/// Opens, draws and saves a document the way the app does after a file is chosen.
fn use_document(doc: &mut docxtor_core::Document, t: &mut Trace) {
    t.stage("render");
    let _ = doc.render_all();
    let _ = doc.page_setup();
    let _ = doc.tabs_info();
    t.stage("save");
    if let Err(e) = doc.save() {
        t.errors.push(e.to_string());
    }
}

fn e_open(bytes: &[u8], c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("open::open");
    let first = docxtor_core::open::open(bytes, "damaged.docx");
    let mut outcome = Err("not opened".to_string());
    match first {
        Ok(mut o) => {
            use_document(&mut o.doc, t);
            // The pictures a document holds are drawn on demand, but the first screen asks for them.
            t.stage("pictures");
            if let Ok(pkg) = docxtor_core::package::Package::open(bytes) {
                for p in pkg.parts.iter().filter(|p| p.name.contains("media/")).take(50) {
                    let _ = o.doc.media_display(&p.name);
                }
            }
            outcome = Ok(format!("opened as {:?}", o.report.format));
            // A document that holds a very deep tree has to be taken apart again too.
            t.stage("drop");
            drop(o);
        }
        Err(e) => {
            t.errors.push(format!("{}: {}", e.code, e.message));
            outcome = outcome.or(Err(format!("{}: {}", e.code, e.message)));
        }
    }
    if bytes.starts_with(b"PK") && c["part"] != "(ops)" {
        t.stage("Document::open");
        match docxtor_core::Document::open(bytes) {
            Ok(mut d) => {
                let _ = d.render_all();
            }
            Err(e) => t.errors.push(e.to_string()),
        }
    }
    outcome
}

fn e_rtf(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("rtf paste reader");
    let frag = docxtor_core::rtf::rtf_bytes_to_fragment(bytes);
    t.stage("paste");
    paste(frag, t)
}

fn paste(fragment: docxtor_core::fragment::Fragment, t: &mut Trace) -> Result<String, String> {
    let mut doc = docxtor_core::Document::new_blank();
    let first = doc.render_all().order[0];
    let op = docxtor_core::Op::InsertFragment { at: docxtor_core::Pos::new(first, 0), fragment, mode: docxtor_core::fragment::PasteMode::Keep };
    let r = doc.apply_batch(&[op], false);
    if let Err(e) = &r {
        t.errors.push(e.to_string());
    }
    use_document(&mut doc, t);
    r.map(|_| "pasted".to_string()).map_err(|e| e.to_string())
}

fn e_html(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    let html = String::from_utf8_lossy(bytes).into_owned();
    t.stage("html paste reader");
    let _ = docxtor_core::html_import::html_image_sources(&html);
    let mut doc = docxtor_core::Document::new_blank();
    let first = doc.render_all().order[0];
    t.stage("paste");
    let op = docxtor_core::Op::PasteHtml { at: docxtor_core::Pos::new(first, 0), html, mode: docxtor_core::fragment::PasteMode::Keep };
    let r = doc.apply_batch(&[op], false);
    if let Err(e) = &r {
        t.errors.push(e.to_string());
    }
    use_document(&mut doc, t);
    r.map(|_| "pasted".to_string()).map_err(|e| e.to_string())
}

fn e_picture(bytes: &[u8], c: &Value, t: &mut Trace) -> Result<String, String> {
    let ext = c["target"].as_str().unwrap_or("emf");
    t.stage("picture converter");
    match docxtor_core::imgconv::display_form(ext, bytes) {
        Some(d) => Ok(format!("converted to {}", d.mime)),
        None => Err("not converted".into()),
    }
}

fn e_csv(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("csv reader");
    for name in ["list.csv", "list.tsv"] {
        if let Err(e) = docxtor_core::merge::read_recipients(bytes, name, None) {
            t.errors.push(e);
        }
    }
    docxtor_core::merge::read_recipients(bytes, "list.txt", None).map(|r| format!("{} rows", r.rows.len()))
}

fn e_xlsx(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("xlsx reader");
    docxtor_core::merge::read_recipients(bytes, "list.xlsx", None).map(|r| format!("{} rows", r.rows.len()))
}

fn e_chart_workbook(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("chart workbook reader");
    let cells = docxtor_core::chart::xlsx::read_cells(bytes);
    t.stage("chart workbook patcher");
    let (cats, series, refs) = mutations::base_series();
    let old: Vec<Option<docxtor_core::chart::xlsx::SeriesRefs>> = refs.into_iter().map(Some).collect();
    let edit = docxtor_core::chart::xlsx_patch::SheetEdit { categories: &cats, series: &series, scatter: false, old_refs: &old, old_size_refs: &[] };
    let patched = docxtor_core::chart::xlsx_patch::patch(bytes, &edit);
    match (cells, patched) {
        (None, None) => Err("not a workbook".into()),
        _ => Ok("read".into()),
    }
}

fn e_bibtex(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    t.stage("bibtex reader");
    let _ = docxtor_core::bibtex::latex_to_text(&text);
    let r = docxtor_core::bibtex::parse(&text);
    if let Ok(sources) = &r {
        t.stage("bibtex writer");
        let _ = docxtor_core::bibtex::to_bibtex(sources);
    }
    r.map(|s| format!("{} sources", s.len()))
}

fn e_csl_json(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("csl-json reader");
    docxtor_core::csl_json::parse(&String::from_utf8_lossy(bytes)).map(|s| format!("{} sources", s.len()))
}

fn e_csl(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("csl style reader");
    let style = docxtor_core::csl::Style::parse(&String::from_utf8_lossy(bytes))?;
    t.stage("csl processor");
    use docxtor_core::citations::{Name, Source, SourceType};
    let book = Source { tag: "Smi20".into(), kind: SourceType::Book, authors: vec![Name::person("Smith", "John Q.")], title: "The Art of Citing".into(), year: Some("2020".into()), publisher: Some("Penguin".into()), ..Default::default() };
    let article = Source { tag: "Doe19".into(), kind: SourceType::Article, authors: vec![Name::person("Doe", "Jane")], title: "On things".into(), container_title: Some("Journal of Things".into()), year: Some("2019".into()), ..Default::default() };
    let sites = vec![
        docxtor_core::csl::CiteSite { items: vec![docxtor_core::csl::CiteItem::new(&book), docxtor_core::csl::CiteItem::new(&article)], note: Some(1) },
        docxtor_core::csl::CiteSite { items: vec![docxtor_core::csl::CiteItem::new(&book)], note: Some(2) },
    ];
    let out = docxtor_core::csl::process(&style, &sites);
    Ok(format!("{} citations", out.citations.len()))
}

fn e_crypt(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("open::open");
    if let Err(e) = docxtor_core::open::open(bytes, "damaged.docx") {
        t.errors.push(format!("{}: {}", e.code, e.message));
    }
    t.stage("password check");
    let _ = docxtor_core::office_crypt::is_encrypted(bytes);
    if let Err(e) = docxtor_core::office_crypt::decrypt(bytes, "wrong password") {
        t.errors.push(e.to_string());
    }
    t.stage("decrypt");
    match docxtor_core::office_crypt::open(bytes, mutations::PW) {
        Ok((zip, _session)) => {
            t.stage("Document::open");
            match docxtor_core::Document::open(&zip) {
                Ok(mut d) => {
                    use_document(&mut d, t);
                    Ok("decrypted and opened".into())
                }
                Err(e) => Err(e.to_string()),
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

fn e_automark(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("automark reader");
    docxtor_core::automark::read(bytes).map(|e| format!("{} entries", e.len()))
}

fn e_ops(bytes: &[u8], _c: &Value, t: &mut Trace) -> Result<String, String> {
    let mut doc = common::build("rich");
    t.stage("apply_json");
    let r = doc.apply_json(&String::from_utf8_lossy(bytes), false);
    if r.is_ok() {
        use_document(&mut doc, t);
    }
    r.map(|r| format!("{} results", r.len())).map_err(|e| e.to_string())
}

/// Tests of the harness itself: each must come back as the failure it stands for.
fn e_selftest(_bytes: &[u8], c: &Value, t: &mut Trace) -> Result<String, String> {
    t.stage("selftest");
    #[inline(never)]
    fn recurse(n: usize, pad: &mut [u8; 512]) -> usize {
        pad[n % 512] = pad[n % 512].wrapping_add(1);
        let mut local = [0u8; 512];
        local[n % 512] = pad[(n + 1) % 512];
        let r = if n == usize::MAX { 0 } else { recurse(n + 1, &mut local) };
        std::hint::black_box(&local);
        r + 1
    }
    match c["fixture"].as_str().unwrap_or("") {
        "stack" => Ok(recurse(0, &mut [0u8; 512]).to_string()),
        "memory" => {
            let v = vec![1u8; 2 << 30];
            Ok(std::hint::black_box(v.len()).to_string())
        }
        "hang" => loop {
            std::hint::black_box(0);
        },
        "panic" => panic!("on purpose"),
        "internal-text" => Err("called `Result::unwrap()` on an `Err` value: ParseIntError { kind: InvalidDigit }".into()),
        _ => Ok("fine".into()),
    }
}

type EntryFn = fn(&[u8], &Value, &mut Trace) -> Result<String, String>;

const ENTRIES: &[(&str, EntryFn)] = &[
    ("open", e_open),
    ("rtf-paste", e_rtf),
    ("html-paste", e_html),
    ("emf", e_picture),
    ("wmf", e_picture),
    ("tiff", e_picture),
    ("csv", e_csv),
    ("xlsx", e_xlsx),
    ("chart-workbook", e_chart_workbook),
    ("bibtex", e_bibtex),
    ("csl-json", e_csl_json),
    ("csl", e_csl),
    ("crypt", e_crypt),
    ("automark", e_automark),
    ("ops-json", e_ops),
    ("selftest", e_selftest),
];

/// The input of a case. An `ops-json` case is a real op shape with live ids and one value made hostile.
fn build_any(case: &Value) -> Result<Vec<u8>, String> {
    match case["target"].as_str().unwrap_or("") {
        "selftest" => Ok(Vec::new()),
        "ops-json" => {
            let shapes = common::load_shapes();
            let m = &case["mutation"];
            let shape = shapes.get(m["shape"].as_u64().unwrap_or(0) as usize).ok_or("n/a: no such shape")?;
            let doc = common::build("rich");
            let mut graveyard = vec![];
            let live = common::Live::of(&doc, &mut graveyard);
            let mut rng = common::Rng::new(case["seed"].as_u64().unwrap_or(0) ^ 0x5bd1);
            let op_name = shape["op"].as_str().unwrap_or("").to_string();
            let swapped = common::swap_live(shape, &live, &mut rng, &op_name);
            Ok(mutations::hostile_ops(&swapped, m, case["seed"].as_u64().unwrap_or(0)).into_bytes())
        }
        _ => mutations::build_input(case),
    }
}

fn run_entry(case: &Value, input: Vec<u8>, t: &mut Trace) -> Result<String, String> {
    let target = case["target"].as_str().unwrap_or("");
    let f = ENTRIES.iter().find(|(n, _)| *n == target).map(|(_, f)| *f).unwrap_or(|_, _, _| Err("unknown entry point".into()));
    f(&input, case, t)
}

// ---------------------------------------------------------------------------
// The worker (the test binary run again, reading cases from stdin)
// ---------------------------------------------------------------------------

static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

fn say(tag: &str, v: &Value) {
    println!("@@{tag} {v}");
    let _ = std::io::stdout().flush();
}

fn time_limit() -> Duration {
    let ms = std::env::var("DOCXTOR_MUTATE_LIMIT_MS").ok().and_then(|s| s.parse().ok()).unwrap_or(if cfg!(debug_assertions) { 8_000 } else { 2_000 });
    Duration::from_millis(ms)
}

fn memory_limit() -> usize {
    std::env::var("DOCXTOR_MUTATE_MEM_MB").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1024) << 20
}

/// Not a test of anything: when the environment says so, this is the worker loop.
#[test]
fn mutate_worker_entry() {
    if std::env::var("DOCXTOR_MUTATE_WORKER").as_deref() != Ok("1") {
        return;
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("docxtor-case") {
            let msg = info.payload().downcast_ref::<String>().cloned().or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            let at = info.location().map(|l| l.file().to_string()).unwrap_or_default();
            *LAST_PANIC.lock().unwrap() = format!("{} at {at}", msg.lines().next().unwrap_or(""));
        } else {
            prev(info);
        }
    }));
    let (wall, mem) = (time_limit(), memory_limit());
    println!("\n@@ready");
    let _ = std::io::stdout().flush();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let case: Value = serde_json::from_str(&line).expect("a case");
        one_case(&case, wall, mem);
    }
}

fn one_case(case: &Value, wall: Duration, mem: usize) {
    // Building the input is not timed and not counted: only the engine call is.
    let input = match build_any(case) {
        Ok(b) => b,
        Err(e) if e.starts_with("n/a") => return say("result", &json!({"skipped": e})),
        Err(e) => return say("result", &json!({"harness": e})),
    };
    let wall = case["limit_ms"].as_u64().map(Duration::from_millis).unwrap_or(wall);
    println!("@@start");
    let _ = std::io::stdout().flush();
    let base = LIVE.load(Relaxed);
    PEAK.store(base, Relaxed);
    LIMIT.store(base.saturating_add(mem), Relaxed);
    let t0 = Instant::now();
    let (tx, rx) = channel();
    let c = case.clone();
    let h = std::thread::Builder::new()
        .name("docxtor-case".into())
        .stack_size(1 << 20)
        .spawn(move || {
            let mut t = Trace::default();
            // The input and everything the engine built are dropped in here, in the small stack too.
            let r = run_entry(&c, input, &mut t);
            let _ = tx.send((r, t.errors));
        })
        .unwrap();
    let got = rx.recv_timeout(wall + Duration::from_secs(5));
    let ms = t0.elapsed().as_millis() as u64;
    let peak = PEAK.load(Relaxed).saturating_sub(base);
    LIMIT.store(usize::MAX, Relaxed);
    match got {
        Ok((r, errors)) => {
            let _ = h.join();
            let (ok, text) = match r {
                Ok(s) => (true, s),
                Err(e) => (false, e),
            };
            say("result", &json!({"ok": ok, "text": text, "errors": errors, "ms": ms, "peak_mb": peak >> 20}));
        }
        Err(RecvTimeoutError::Disconnected) => {
            let _ = h.join();
            let p = std::mem::take(&mut *LAST_PANIC.lock().unwrap());
            say("result", &json!({"panic": p, "ms": ms}));
        }
        Err(RecvTimeoutError::Timeout) => {
            say("result", &json!({"hang": true, "ms": ms}));
            std::process::exit(3);
        }
    }
}

// ---------------------------------------------------------------------------
// Failures, messages, keys
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Failure {
    /// panic, stack, memory, hang, abort, text or harness.
    check: String,
    message: String,
    /// What makes two failures the same: the check, what happened, the stage, the entry point, the kind of part and the
    /// kind of mutation (not the fixture, the seed or the size).
    key: String,
}

impl Failure {
    fn known(&self) -> bool {
        known_mutate::is_known(&self.check, &self.message)
    }
}

fn digits_to_n(s: &str) -> String {
    let mut out = String::new();
    let mut d = false;
    for c in s.chars() {
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

/// A short name for the mutation, with its sizes but without its seed.
fn mutation_label(m: &Value) -> String {
    let mut s = m["kind"].as_str().unwrap_or("?").to_string();
    if let Some(o) = m.as_object() {
        for (k, v) in o {
            if matches!(k.as_str(), "kind" | "close" | "open" | "mode") {
                continue;
            }
            let t = match v {
                Value::String(x) if x.len() <= 28 => x.clone(),
                Value::String(_) => continue,
                other => other.to_string(),
            };
            s.push_str(&format!(" {k}={t}"));
        }
    }
    s
}

/// The part of a mutation label that says what sort of thing it was, without how big.
fn mutation_kind(m: &Value) -> String {
    let mut s = m["kind"].as_str().unwrap_or("?").to_string();
    for k in ["label", "tag", "how", "type", "with", "what", "elem", "attr", "token", "lit", "after_tag"] {
        if let Some(v) = m[k].as_str() {
            s.push_str(&format!(" {k}={}", &v[..v.len().min(28)]));
        }
    }
    s
}

fn case_label(c: &Value) -> String {
    format!("{} {} {} {}", c["target"].as_str().unwrap_or("?"), c["fixture"].as_str().unwrap_or("?"), mutations::part_class(c["part"].as_str().unwrap_or("?")), mutation_label(&c["mutation"]))
}

fn make_failure(case: &Value, check: &str, detail: &str, stage: &str) -> Failure {
    let message = format!("{detail} in {stage} ({})", case_label(case));
    let key = format!("{check}: {} in {stage} ({} {} {})", digits_to_n(detail), case["target"].as_str().unwrap_or("?"), mutations::part_class(case["part"].as_str().unwrap_or("?")), mutation_kind(&case["mutation"]));
    Failure { check: check.to_string(), message, key }
}

// ---------------------------------------------------------------------------
// The supervisor: workers as processes, at most N at a time
// ---------------------------------------------------------------------------

enum Msg {
    Line(String),
    Eof,
}

#[derive(Default, Clone)]
struct Verdict {
    skipped: Option<String>,
    failures: Vec<Failure>,
    /// Opened, or refused with an error.
    opened: bool,
    text: String,
    ms: u64,
    peak_mb: u64,
    errors: Vec<String>,
}

struct Slots {
    free: Mutex<usize>,
    total: usize,
    cv: Condvar,
}

fn slots() -> &'static Slots {
    static S: OnceLock<Slots> = OnceLock::new();
    S.get_or_init(|| {
        let n = std::env::var("DOCXTOR_MUTATE_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(2).clamp(1, 4));
        Slots { free: Mutex::new(n), total: n, cv: Condvar::new() }
    })
}

struct Slot;

impl Slot {
    fn take() -> Slot {
        let s = slots();
        let mut free = s.free.lock().unwrap();
        while *free == 0 {
            free = s.cv.wait(free).unwrap();
        }
        *free -= 1;
        Slot
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let s = slots();
        *s.free.lock().unwrap() += 1;
        s.cv.notify_one();
    }
}

fn worker_count() -> usize {
    slots().total
}

struct Worker {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: Option<Receiver<Msg>>,
    err_path: std::path::PathBuf,
}

impl Worker {
    fn new() -> Worker {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Relaxed);
        Worker { child: None, stdin: None, rx: None, err_path: std::env::temp_dir().join(format!("docxtor_mutate_{}_{id}.err", std::process::id())) }
    }

    fn kill(&mut self) {
        self.stdin = None;
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.rx = None;
    }

    fn spawn(&mut self) {
        self.kill();
        let err = std::fs::File::create(&self.err_path).expect("the worker's error file");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "mutate_worker_entry", "--nocapture", "--test-threads=1"])
            .env("DOCXTOR_MUTATE_WORKER", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(err)
            .spawn()
            .expect("start a worker");
        let out = child.stdout.take().unwrap();
        self.stdin = child.stdin.take();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                if tx.send(Msg::Line(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(Msg::Eof);
        });
        self.child = Some(child);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Msg::Line(l)) if l == "@@ready" => break,
                Ok(Msg::Line(_)) => {}
                _ => panic!("the worker did not start; see {}", self.err_path.display()),
            }
        }
        self.rx = Some(rx);
    }

    fn stderr_text(&self) -> String {
        std::fs::read_to_string(&self.err_path).unwrap_or_default()
    }

    /// Run one case; start the worker again when it died or was ended.
    fn run(&mut self, case: &Value, limit: Duration) -> Verdict {
        let _slot = Slot::take();
        if self.child.is_none() {
            self.spawn();
        }
        let limit = case["limit_ms"].as_u64().map(Duration::from_millis).unwrap_or(limit);
        let line = format!("{case}\n");
        if self.stdin.as_mut().is_none_or(|s| s.write_all(line.as_bytes()).and_then(|_| s.flush()).is_err()) {
            self.spawn();
            let s = self.stdin.as_mut().unwrap();
            s.write_all(line.as_bytes()).unwrap();
            let _ = s.flush();
        }
        let mut stage = "building the input".to_string();
        let mut started: Option<Instant> = None;
        let build_deadline = Instant::now() + Duration::from_secs(400);
        loop {
            let deadline = started.map(|s| s + limit).unwrap_or(build_deadline);
            let msg = self.rx.as_ref().unwrap().recv_timeout(deadline.saturating_duration_since(Instant::now()));
            match msg {
                Ok(Msg::Line(l)) => {
                    if l == "@@start" {
                        started = Some(Instant::now());
                        stage = "open".into();
                    } else if let Some(s) = l.strip_prefix("@@stage ") {
                        stage = s.to_string();
                    } else if let Some(j) = l.strip_prefix("@@result ") {
                        return self.verdict(case, serde_json::from_str(j).unwrap_or(Value::Null), &stage);
                    }
                }
                Ok(Msg::Eof) | Err(RecvTimeoutError::Disconnected) => return self.died(case, &stage, started.is_some()),
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    let mut v = Verdict { ms: limit.as_millis() as u64, ..Default::default() };
                    v.failures.push(if started.is_some() { make_failure(case, "hang", &format!("no answer in {} ms", limit.as_millis().max(1)), &stage) } else { make_failure(case, "harness", "building the input took too long", &stage) });
                    return v;
                }
            }
        }
    }

    /// The worker ended by itself: say why from what it left on stderr.
    fn died(&mut self, case: &Value, stage: &str, started: bool) -> Verdict {
        let status = self.child.as_mut().and_then(|c| c.wait().ok());
        let err = self.stderr_text();
        self.kill();
        let mut v = Verdict::default();
        let tail: String = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(120).collect();
        let f = if !started {
            make_failure(case, "harness", &format!("the worker died while building the input: {tail}"), stage)
        } else if err.contains("overflowed its stack") {
            make_failure(case, "stack", "stack overflow (1 MB stack)", stage)
        } else if err.contains("memory allocation of") {
            let bytes: String = err.split("memory allocation of ").nth(1).unwrap_or("").chars().take_while(|c| c.is_ascii_digit()).collect();
            make_failure(case, "memory", &format!("memory past {} MB (asked for {bytes} more)", memory_limit() >> 20), stage)
        } else {
            #[cfg(unix)]
            let how = {
                use std::os::unix::process::ExitStatusExt;
                status.map(|s| s.signal().map(|n| format!("signal {n}")).unwrap_or_else(|| format!("{s}"))).unwrap_or_default()
            };
            #[cfg(not(unix))]
            let how = format!("{status:?}");
            make_failure(case, "abort", &format!("the process ended ({how}): {tail}"), stage)
        };
        v.failures.push(f);
        v
    }

    fn verdict(&mut self, case: &Value, r: Value, stage: &str) -> Verdict {
        let mut v = Verdict::default();
        if let Some(s) = r["skipped"].as_str() {
            v.skipped = Some(s.to_string());
            return v;
        }
        if let Some(h) = r["harness"].as_str() {
            v.failures.push(make_failure(case, "harness", h, stage));
            return v;
        }
        if r["hang"].as_bool() == Some(true) {
            self.kill();
            v.failures.push(make_failure(case, "hang", "no answer (the worker gave up)", stage));
            return v;
        }
        if let Some(p) = r["panic"].as_str() {
            let (msg, file) = p.rsplit_once(" at ").unwrap_or((p, ""));
            let file = file.split("crates/").last().unwrap_or(file);
            v.failures.push(make_failure(case, "panic", &format!("panic: {msg} at {file}"), stage));
            return v;
        }
        v.ms = r["ms"].as_u64().unwrap_or(0);
        v.peak_mb = r["peak_mb"].as_u64().unwrap_or(0);
        v.opened = r["ok"].as_bool().unwrap_or(false);
        v.text = r["text"].as_str().unwrap_or("").to_string();
        v.errors = r["errors"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let mut texts = v.errors.clone();
        if !v.opened {
            texts.push(v.text.clone());
        }
        // An error message is for a person to read: not a dump of a 16 MB value that was in the input.
        for t in &texts {
            if t.chars().count() > 2000 {
                v.failures.push(make_failure(case, "text", &format!("an error text of {} characters", t.chars().count()), stage));
                break;
            }
        }
        for t in texts {
            if let Some(m) = common::internal_text(&t) {
                v.failures.push(make_failure(case, "text", &format!("an internal error text ({m}) \"{}\"", t.chars().take(80).collect::<String>().replace('\n', " ")), stage));
                break;
            }
        }
        v
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.kill();
        let _ = std::fs::remove_file(&self.err_path);
    }
}

// ---------------------------------------------------------------------------
// Running many cases
// ---------------------------------------------------------------------------

fn run_pool(cases: Vec<Value>) -> Vec<(Value, Verdict)> {
    let n = cases.len();
    let queue = Arc::new(Mutex::new(cases.into_iter().collect::<VecDeque<_>>()));
    let out = Arc::new(Mutex::new(Vec::with_capacity(n)));
    let hs: Vec<_> = (0..worker_count())
        .map(|_| {
            let (queue, out) = (queue.clone(), out.clone());
            std::thread::spawn(move || {
                let mut w = Worker::new();
                loop {
                    let Some(case) = queue.lock().unwrap().pop_front() else { break };
                    let v = w.run(&case, time_limit());
                    out.lock().unwrap().push((case, v));
                }
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    let r = std::mem::take(&mut *out.lock().unwrap());
    r
}

fn pretty(case: &Value) -> String {
    serde_json::to_string(case).unwrap()
}

/// Make a failing case smaller: its counts and sizes go down by tens while it still fails the same way, then a few steps
/// of halving between the last failure and the first pass. Few probes: a probe that hangs costs the whole time limit.
fn shrink(w: &mut Worker, case: &Value, key: &str, mut probes: usize) -> Value {
    let mut best = case.clone();
    let fails = |w: &mut Worker, c: &Value, probes: &mut usize| -> bool {
        if *probes == 0 {
            return false;
        }
        *probes -= 1;
        w.run(c, time_limit()).failures.iter().any(|f| f.key == key)
    };
    for field in ["n", "size", "depth", "count"] {
        let Some(v) = best["mutation"][field].as_u64() else { continue };
        if v <= 1 {
            continue;
        }
        let mut cur = v;
        let mut pass = None;
        if field == "count" {
            // Flipped bytes: the first k positions are the same for every k, so try the smallest prefixes first.
            let mut k = 1;
            while k < cur && probes > 0 {
                let mut c = best.clone();
                c["mutation"][field] = json!(k);
                if fails(w, &c, &mut probes) {
                    best = c;
                    break;
                }
                k *= 2;
            }
            continue;
        }
        while cur > 1 && probes > 0 {
            let next = (cur / 10).max(1);
            let mut c = best.clone();
            c["mutation"][field] = json!(next);
            if fails(w, &c, &mut probes) {
                best = c;
                cur = next;
            } else {
                pass = Some(next);
                break;
            }
        }
        if let Some(mut low) = pass {
            let mut high = cur;
            for _ in 0..3 {
                if high <= low + 1 || probes == 0 {
                    break;
                }
                let mid = (low + high) / 2;
                let mut c = best.clone();
                c["mutation"][field] = json!(mid);
                if fails(w, &c, &mut probes) {
                    best = c;
                    high = mid;
                } else {
                    low = mid;
                }
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

fn candidates() -> &'static Vec<mutate_cases::Family> {
    static C: OnceLock<Vec<mutate_cases::Family>> = OnceLock::new();
    C.get_or_init(mutate_cases::families)
}

/// The fixed cases of a family, a few of each kind of mutation (the dear ones, with 100,000 of something, fewer): each
/// known failure costs the whole time limit, so the set has to stay small for `cargo test` to stay quick. The long run
/// draws from all of them.
fn fixed_cases(f: &mutate_cases::Family) -> Vec<Value> {
    let mut by_kind: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for c in f.cands.iter().filter(|c| c.fixed) {
        by_kind.entry(c.case["mutation"]["kind"].as_str().unwrap_or("?").to_string()).or_default().push(&c.case);
    }
    let mut out = Vec::new();
    for (kind, list) in by_kind {
        let cap = match kind.as_str() {
            "nest" => 4,
            "repeat" | "repeat-lit" | "fill" | "attr" | "bomb" | "chunk" | "many-parts" | "huge-array" | "huge-string" | "huge-batch" | "deep-in-op" | "csl-loop" => 2,
            _ => 8,
        }
        .min(list.len());
        for i in 0..cap {
            out.push(list[i * list.len() / cap].clone());
        }
    }
    out
}

fn long_run() -> bool {
    std::env::var("DOCXTOR_MUTATE_SECS").is_ok()
}

/// The fixed cases of the families that carry `names`, run; known failures are skipped; any other one fails the test
/// with its smaller case printed.
fn fixed(names: &[&str]) {
    if long_run() {
        return;
    }
    let cases: Vec<Value> = candidates().iter().filter(|f| names.contains(&f.name)).flat_map(fixed_cases).collect();
    assert!(!cases.is_empty(), "no fixed cases for {names:?}");
    let started = Instant::now();
    let results = run_pool(cases);
    let (mut known, mut skipped, mut opened) = (0, 0, 0);
    let mut new: Vec<(Value, Failure)> = Vec::new();
    for (case, v) in &results {
        if v.skipped.is_some() {
            skipped += 1;
        }
        if v.opened {
            opened += 1;
        }
        for f in &v.failures {
            if f.known() {
                known += 1;
            } else if !new.iter().any(|(_, g)| g.key == f.key) {
                new.push((case.clone(), f.clone()));
            }
        }
    }
    let mut slow: Vec<&(Value, Verdict)> = results.iter().filter(|(_, v)| v.skipped.is_none() && v.failures.is_empty()).collect();
    slow.sort_by_key(|(_, v)| std::cmp::Reverse(v.ms));
    println!("{names:?}: {} cases in {:.1}s ({opened} opened, {skipped} skipped, {known} known failures)", results.len(), started.elapsed().as_secs_f32());
    for (c, v) in slow.iter().take(3) {
        println!("  slowest passing: {} ms, {} MB: {}", v.ms, v.peak_mb, case_label(c));
    }
    if !new.is_empty() {
        let mut w = Worker::new();
        let mut text = String::new();
        for (_, f) in new.iter().skip(4) {
            text.push_str(&format!("\n{}: {}\n", f.check, f.message));
        }
        for (case, f) in new.iter().take(4) {
            let small = shrink(&mut w, case, &f.key, 6);
            text.push_str(&format!("\n{}: {}\n  case:   {}\n  smaller: {}\n  replay: DOCXTOR_MUTATE_REPLAY='{}' cargo test --test mutate_open replay_a_case -- --ignored --nocapture\n", f.check, f.message, pretty(case), pretty(&small), pretty(&small)));
        }
        panic!("{} new failure(s) opening damaged files (a real bug goes in common/known_mutate.rs with its reason; a mistake of the harness is fixed in the harness):{text}", new.len());
    }
}

#[test]
fn damaged_word_files_big_and_deep() {
    fixed(&["docx"]);
}

#[test]
fn damaged_other_formats_and_passwords() {
    fixed(&["other formats", "password-protected"]);
}

#[test]
fn damaged_pictures_and_pasted_text() {
    fixed(&["rtf paste", "html paste", "pictures"]);
}

#[test]
fn damaged_lists_references_and_ops() {
    fixed(&["recipient list csv", "workbooks", "bibtex", "csl-json", "csl styles", "ops json", "automark file"]);
}

/// The harness is only worth anything if it sees what it is built to see, and survives it.
#[test]
fn the_harness_sees_a_stack_overflow_a_runaway_allocation_a_hang_and_a_panic() {
    if long_run() {
        return;
    }
    let mut w = Worker::new();
    let selftest = |what: &str| json!({"target": "selftest", "fixture": what, "part": "(none)", "mutation": {"kind": "none"}, "seed": 0, "limit_ms": 1500});
    for (what, check) in [("stack", "stack"), ("memory", "memory"), ("hang", "hang"), ("panic", "panic"), ("internal-text", "text")] {
        let v = w.run(&selftest(what), time_limit());
        assert!(v.failures.iter().any(|f| f.check == check), "{what}: wanted a {check} failure, got {:?}", v.failures);
        assert!(v.failures.iter().all(|f| !f.known()), "{what}: a harness self-test must not be a known failure");
    }
    // After all that the worker still answers.
    let v = w.run(&selftest("fine"), time_limit());
    assert!(v.failures.is_empty() && v.opened, "{:?}", v.failures);
}

#[test]
fn the_known_list_names_messages_not_whole_checks() {
    for (check, pattern, why) in known_mutate::KNOWN {
        assert!(["panic", "stack", "memory", "hang", "abort", "text"].contains(check), "{check}");
        assert!(pattern.chars().filter(|c| *c != '*').count() >= 12, "an entry must name its message, not a whole check: {pattern}");
        assert!(why.len() > 40, "an entry needs its reason and repro: {pattern}");
    }
}

/// The bomb's checksum is worked out without going over a gigabyte; it has to be the real one.
#[test]
fn the_checksum_of_zeros_is_right() {
    for n in [0usize, 1, 2, 3, 7, 255, 4096, 100_001] {
        let mut c = flate2::Crc::new();
        c.update(&vec![0u8; n]);
        assert_eq!(mutations::crc_of_zeros(n), c.sum(), "{n} zeros");
    }
}

#[test]
fn every_case_builds_the_same_bytes_twice() {
    if long_run() {
        return;
    }
    // A failure is only worth printing if the case it prints builds the same input again.
    for f in candidates() {
        for c in f.cands.iter().filter(|c| c.fixed).step_by(23).take(6) {
            let (a, b) = (build_any(&c.case), build_any(&c.case));
            assert_eq!(a, b, "{}", pretty(&c.case));
        }
    }
}

// ---------------------------------------------------------------------------
// The long run and the replay
// ---------------------------------------------------------------------------

/// `DOCXTOR_MUTATE_SECS=600 cargo test --release --test mutate_open -- --nocapture`
#[test]
fn damaged_files_for_a_while() {
    let Ok(secs) = std::env::var("DOCXTOR_MUTATE_SECS") else { return };
    let secs: u64 = secs.parse().unwrap_or(60);
    let base: u64 = std::env::var("DOCXTOR_MUTATE_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() % 1_000_000 * 1000).unwrap_or(1));
    let deadline = Instant::now() + Duration::from_secs(secs);
    let families = candidates();
    let total_weight: usize = families.iter().map(|f| f.weight).sum();
    // key -> (times seen, first case, failure)
    let seen: Arc<Mutex<BTreeMap<String, (usize, Value, Failure)>>> = Arc::new(Mutex::new(BTreeMap::new()));
    // family and mutation kind -> (cases, opened, refused, skipped, failed)
    let stats: Arc<Mutex<BTreeMap<String, [u32; 5]>>> = Arc::new(Mutex::new(BTreeMap::new()));
    let errors: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let slowest: Arc<Mutex<Vec<(u64, u64, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let counter = Arc::new(AtomicUsize::new(0));
    let hs: Vec<_> = (0..worker_count())
        .map(|t| {
            let (seen, stats, errors, slowest, counter) = (seen.clone(), stats.clone(), errors.clone(), slowest.clone(), counter.clone());
            std::thread::spawn(move || {
                let mut w = Worker::new();
                let mut seed = base + t as u64 * 100_000;
                while Instant::now() < deadline {
                    seed += 1;
                    let mut rng = common::Rng::new(seed);
                    let mut pick = rng.below(total_weight);
                    let family = families.iter().find(|f| {
                        if pick < f.weight {
                            true
                        } else {
                            pick -= f.weight;
                            false
                        }
                    });
                    let Some(family) = family.filter(|f| !f.cands.is_empty()) else { continue };
                    let mut case = rng.pick(&family.cands).case.clone();
                    case["seed"] = json!(seed);
                    // Other sizes than the big one: smaller, in tens, and now and then a bit bigger.
                    for field in ["n", "size"] {
                        if let Some(v) = case["mutation"][field].as_u64() {
                            let scaled = match rng.below(6) {
                                0 => (v / 10).max(1),
                                1 => (v / 100).max(1),
                                2 if field == "n" => v * 3,
                                _ => v,
                            };
                            case["mutation"][field] = json!(scaled);
                        }
                    }
                    if let Some(c) = case["mutation"]["count"].as_u64() {
                        case["mutation"]["count"] = json!(1 + rng.below(c as usize * 2) as u64);
                    }
                    counter.fetch_add(1, Relaxed);
                    let v = w.run(&case, time_limit());
                    let row = format!("{} / {}", family.name, case["mutation"]["kind"].as_str().unwrap_or("?"));
                    {
                        let mut s = stats.lock().unwrap();
                        let e = s.entry(row).or_default();
                        e[0] += 1;
                        if v.skipped.is_some() {
                            e[3] += 1;
                        } else if !v.failures.is_empty() {
                            e[4] += 1;
                        } else if v.opened {
                            e[1] += 1;
                        } else {
                            e[2] += 1;
                        }
                    }
                    if v.skipped.is_none() {
                        for t in v.errors.iter().chain(if v.opened { None } else { Some(&v.text) }) {
                            *errors.lock().unwrap().entry(digits_to_n(&t.chars().take(110).collect::<String>())).or_default() += 1;
                        }
                        let mut s = slowest.lock().unwrap();
                        s.push((v.ms, v.peak_mb, case_label(&case)));
                        s.sort_by_key(|x| std::cmp::Reverse(x.0));
                        s.truncate(8);
                    }
                    for f in &v.failures {
                        let mut s = seen.lock().unwrap();
                        match s.get_mut(&f.key) {
                            Some(e) => e.0 += 1,
                            None => {
                                s.insert(f.key.clone(), (1, case.clone(), f.clone()));
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
    let total = counter.load(Relaxed);
    println!("\n{total} cases in {secs}s");
    println!("\n{:<52} {:>6} {:>7} {:>8} {:>8} {:>7}", "family / mutation", "cases", "opened", "refused", "skipped", "failed");
    for (k, e) in stats.lock().unwrap().iter() {
        println!("{k:<52} {:>6} {:>7} {:>8} {:>8} {:>7}", e[0], e[1], e[2], e[3], e[4]);
    }
    println!("\nslowest cases that passed or failed:");
    for (ms, mb, l) in slowest.lock().unwrap().iter() {
        println!("  {ms} ms, {mb} MB: {l}");
    }
    let mut errs: Vec<(usize, String)> = errors.lock().unwrap().iter().map(|(k, n)| (*n, k.clone())).collect();
    errs.sort_by_key(|e| std::cmp::Reverse(e.0));
    println!("\nthe error texts people would see ({} kinds, most common first):", errs.len());
    for (n, e) in errs.iter().take(60) {
        println!("  x{n}  {e}");
    }
    let entries: Vec<(String, (usize, Value, Failure))> = std::mem::take(&mut *seen.lock().unwrap()).into_iter().collect();
    println!("\n{} distinct failures; shrinking each", entries.len());
    let queue = Arc::new(Mutex::new(entries.clone()));
    let texts: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let hs: Vec<_> = (0..worker_count())
        .map(|_| {
            let (queue, texts) = (queue.clone(), texts.clone());
            std::thread::spawn(move || {
                let mut w = Worker::new();
                loop {
                    let Some((key, (n, case, f))) = queue.lock().unwrap().pop() else { break };
                    let small = shrink(&mut w, &case, &key, 8);
                    let text = format!("{}{}: {}\nseen in {n} case(s)\ncase:    {}\nsmaller: {}\nreplay:  DOCXTOR_MUTATE_REPLAY='{}' cargo test --test mutate_open replay_a_case -- --ignored --nocapture", if f.known() { "(known) " } else { "" }, f.check, f.message, pretty(&case), pretty(&small), pretty(&small));
                    println!("\n=== FAILURE ===\n{text}");
                    texts.lock().unwrap().push((key, text));
                }
            })
        })
        .collect();
    for h in hs {
        let _ = h.join();
    }
    println!("\n{total} cases, {} distinct failures", entries.len());
    for (k, (n, _, f)) in &entries {
        println!("  x{n}  {}{k}", if f.known() { "(known) " } else { "" });
    }
    if let Ok(path) = std::env::var("DOCXTOR_MUTATE_OUT") {
        let mut all = texts.lock().unwrap().clone();
        all.sort();
        let _ = std::fs::write(path, all.iter().map(|s| s.1.as_str()).collect::<Vec<_>>().join("\n\n"));
    }
}

/// Run one case a failure printed, through the same worker, and print what happened.
#[test]
#[ignore]
fn replay_a_case() {
    let text = std::env::var("DOCXTOR_MUTATE_REPLAY").expect("set DOCXTOR_MUTATE_REPLAY to a case's JSON");
    let case: Value = serde_json::from_str(&text).expect("the case is not JSON");
    if let Ok(path) = std::env::var("DOCXTOR_MUTATE_SAVE") {
        std::fs::write(&path, build_any(&case).expect("the input")).unwrap();
        println!("the broken file is in {path}");
    }
    let mut w = Worker::new();
    let v = w.run(&case, time_limit());
    println!("{} ms, {} MB, opened: {}, {}", v.ms, v.peak_mb, v.opened, v.text);
    for e in &v.errors {
        println!("  error text: {e}");
    }
    for f in &v.failures {
        println!("{}: {}{}", f.check, f.message, if f.known() { " (known)" } else { "" });
    }
    assert!(v.failures.is_empty(), "{} failure(s)", v.failures.len());
}
