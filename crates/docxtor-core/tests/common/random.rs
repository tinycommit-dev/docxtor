//! The engine of the random op harness, shared by `tests/random_ops.rs` and `tests/corpus.rs`: steps, the invariant
//! checks run after each one (`Runner`), a guarded run in a thread with a 1 MB stack, and the step generator.
//! Included with `#[path = "common/random.rs"] mod random;` (it needs `mod common;` beside it).

#![allow(dead_code)]

use crate::common::*;
use docxtor_core::invariants::check_package;
use docxtor_core::{Document, Op};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Steps and failures
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Step {
    Apply { ops: Vec<Value>, merge: bool, strict: bool, kind: String },
    Undo,
    Redo,
    UndoN(usize),
    SaveReopen,
    Save,
}

impl Step {
    pub fn to_json(&self) -> Value {
        match self {
            Step::Apply { ops, merge, strict, kind } => json!({"step":"apply","kind":kind,"merge":merge,"strict":strict,"ops":ops}),
            Step::Undo => json!({"step":"undo"}),
            Step::Redo => json!({"step":"redo"}),
            Step::UndoN(n) => json!({"step":"undo_steps","n":n}),
            Step::SaveReopen => json!({"step":"save_and_reopen"}),
            Step::Save => json!({"step":"save"}),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Failure {
    pub check: String,
    pub message: String,
}

impl Failure {
    pub fn new(check: &str, message: impl Into<String>) -> Self {
        Failure { check: check.to_string(), message: message.into() }
    }
    /// What makes two failures the same: the check and the message with names, numbers (ids, offsets), the values of a
    /// difference and the list of everything else the engine found taken out.
    pub fn key(&self) -> String {
        let head = self.message.lines().next().unwrap_or("");
        let diff = head.contains(" :: ");
        let head = head.split(" :: ").next().unwrap_or("");
        // A difference is named by where it is: the path without indices, rows, cells, story names and variants.
        let collapsed;
        let head = if diff {
            let (msg, path) = head.rsplit_once(": ").unwrap_or(("", head));
            let toks: Vec<&str> = path.split('.').filter(|t| !t.chars().all(|c| c.is_ascii_digit()) && !matches!(*t, "blocks" | "rows" | "cells" | "default" | "first" | "even" | "headers" | "footers" | "endnotes" | "notes" | "parts") && !t.starts_with("part#")).collect();
            collapsed = format!("{msg}: {}", toks.join("."));
            collapsed.as_str()
        } else {
            head
        };
        let head = head.split(" | ").next().unwrap_or("");
        let head = head.split(" (before a step)").next().unwrap_or("").split(" (save)").next().unwrap_or("").split(" (after ").next().unwrap_or("");
        let mut out = String::new();
        let (mut in_num, mut in_quote) = (false, false);
        for c in head.chars().take(200) {
            if c == '\'' {
                if !in_quote {
                    out.push_str("'X'");
                }
                in_quote = !in_quote;
            } else if in_quote {
            } else if c.is_ascii_digit() {
                if !in_num {
                    out.push('N');
                }
                in_num = true;
            } else {
                in_num = false;
                out.push(c);
            }
        }
        format!("{}: {}", self.check, out)
    }
    pub fn known(&self) -> bool {
        known::is_known(&self.check, &self.message)
    }
}

thread_local! {
    static PANIC_AT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static IN_GUARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Panics inside a run are caught and reported as failures, so they are not printed; everything else keeps the default hook.
pub fn quiet_panics_in_runs() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if IN_GUARD.with(|g| g.get()) {
                let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
                PANIC_AT.with(|p| *p.borrow_mut() = at);
            } else {
                prev(info);
            }
        }));
    });
}

/// A panic as a failure: the engine's invariant check names its rule; anything else is E20.
pub fn panic_failure(msg: &str, at: &str) -> Failure {
    if let Some(rest) = msg.strip_prefix("Docxtor invariant broken: after ") {
        let mut lines = rest.lines();
        let what = lines.next().unwrap_or("").trim_end_matches(':').to_string();
        let first = lines.next().unwrap_or("").trim().to_string();
        let (rule, text) = first.split_once(": ").unwrap_or(("engine", &first));
        let all: Vec<&str> = rest.lines().skip(1).take_while(|l| !l.starts_with("ops:")).collect();
        return Failure::new(rule, format!("{text} (after {what}) {}", all.join(" | ")));
    }
    let short: String = msg.lines().next().unwrap_or("").chars().take(200).collect();
    // A panic in this file or in common/ is a mistake of the harness (a generator picking from nothing), not of the engine.
    if at.contains("tests/") {
        return Failure::new("HARNESS", format!("panic: {short} at {at}"));
    }
    Failure::new("E20", format!("panic: {short} at {at}"))
}

pub fn payload_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "(panic with no text)".into()
    }
}

// ---------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub struct Stats {
    /// kind -> (tried, applied, refused, bad json)
    pub kinds: BTreeMap<String, (u32, u32, u32, u32)>,
    pub steps: u32,
}

/// What a client of the engine keeps between edits: the blocks it drew and the page setup, brought up to date from
/// `render_changed` only. It must equal a full render after every step (RC).
pub struct Mirror {
    pub order: Vec<Value>,
    pub blocks: BTreeMap<String, Value>,
    pub page: Value,
}

impl Mirror {
    pub fn new(doc: &mut Document) -> Mirror {
        let out = serde_json::to_value(doc.render_all()).unwrap();
        let mut m = Mirror { order: vec![], blocks: BTreeMap::new(), page: serde_json::to_value(doc.page_setup()).unwrap() };
        m.take(&out);
        m
    }
    pub fn take(&mut self, out: &Value) {
        if out["full"] == json!(true) {
            self.blocks.clear();
        }
        for b in out["blocks"].as_array().into_iter().flatten() {
            self.blocks.insert(block_id(b), b.clone());
        }
        // Not sent when it did not move (`orderKept`): the client keeps the one it has.
        if out["orderKept"] != json!(true) {
            self.order = out["order"].as_array().cloned().unwrap_or_default();
        }
        let keep: HashSet<String> = self.order.iter().map(|v| v.to_string()).collect();
        self.blocks.retain(|k, _| keep.contains(k));
    }
    pub fn update(&mut self, doc: &mut Document) {
        let out = serde_json::to_value(doc.render_changed()).unwrap();
        self.take(&out);
        if out["stories"] == json!(true) {
            self.page = serde_json::to_value(doc.page_setup()).unwrap();
        }
    }
    pub fn check(&self, doc: &Document) -> Result<(), Failure> {
        let full = serde_json::to_value(doc.render_snapshot()).unwrap();
        if full["order"] != Value::Array(self.order.clone()) {
            return Err(Failure::new("RC", format!("render_changed order differs from a full render: {}", first_diff(&Value::Array(self.order.clone()), &full["order"]).unwrap_or_default())));
        }
        for b in full["blocks"].as_array().into_iter().flatten() {
            let id = block_id(b);
            match self.blocks.get(&id) {
                None => return Err(Failure::new("RC", format!("render_changed never gave block {id}"))),
                Some(m) => {
                    if let Some(d) = first_diff(m, b) {
                        return Err(Failure::new("RC", format!("a block drawn from render_changed is stale: {d}")));
                    }
                }
            }
        }
        let page = serde_json::to_value(doc.page_setup()).unwrap();
        if let Some(d) = first_diff(&self.page, &page) {
            return Err(Failure::new("RC", format!("the page setup the client holds is stale (render_changed said stories did not change): {d}")));
        }
        Ok(())
    }
}

pub fn block_id(b: &Value) -> String {
    b.get("id").map(|v| v.to_string()).unwrap_or_default()
}

/// The render model as compact JSON text. A step compares up to four of them, so they are kept as text (a long document's
/// model as a JSON tree is several times its text: 107 MB against about 30 for POI's bug65649.docx) and parsed only to say
/// where two differ.
pub struct Fp(String);

pub fn model_fp(doc: &Document) -> Fp {
    let text = |v: Result<String, serde_json::Error>| v.unwrap_or_else(|_| "null".into());
    Fp(format!(
        r#"{{"body":{},"page":{},"tabs":{}}}"#,
        text(serde_json::to_string(&doc.render_snapshot())),
        text(serde_json::to_string(&doc.page_setup())),
        text(serde_json::to_string(&doc.tabs_info()))
    ))
}

/// Where two models differ, or None when they are the same (text that differs only in the order of a map's keys is the
/// same model).
pub fn fp_diff(a: &Fp, b: &Fp) -> Option<String> {
    if a.0 == b.0 {
        return None;
    }
    let (x, y): (Value, Value) = (serde_json::from_str(&a.0).ok()?, serde_json::from_str(&b.0).ok()?);
    first_diff(&x, &y)
}

/// The parts of a saved file that hold what the render model does not (styles, numbering, settings), parsed: a part
/// that is written again after an undo has the same content, not always the same bytes (E8 is about the model).
pub fn side_parts(bytes: &[u8]) -> BTreeMap<String, docxtor_core::xml::Element> {
    let pkg = docxtor_core::package::Package::open(bytes).unwrap();
    pkg.parts.iter().filter(|p| matches!(p.name.as_str(), "word/styles.xml" | "word/numbering.xml" | "word/settings.xml")).filter_map(|p| docxtor_core::xml::parse(&p.data).ok().map(|d| (p.name.clone(), d.root))).collect()
}

/// How two saved files differ, for a refused batch (E10): the first part that differs and whether its content changed
/// or only the bytes (the part was written again).
pub fn bytes_diff(a: &[u8], b: &[u8]) -> String {
    let (pa, pb) = (docxtor_core::package::Package::open(a).unwrap(), docxtor_core::package::Package::open(b).unwrap());
    for p in &pa.parts {
        match pb.get(&p.name) {
            None => return format!("{} is gone", p.name),
            Some(d) if d != &p.data[..] => {
                let same = match (docxtor_core::xml::parse(&p.data), docxtor_core::xml::parse(d)) {
                    (Ok(x), Ok(y)) => x.root == y.root,
                    _ => false,
                };
                return if same { format!("{} was written again with the same content", p.name) } else { format!("the content of {}", p.name) };
            }
            _ => {}
        }
    }
    match pb.parts.iter().find(|p| pa.get(&p.name).is_none()) {
        Some(p) => format!("{} is new", p.name),
        None => "the list of parts".into(),
    }
}

pub struct Runner {
    pub doc: Document,
    pub mirror: Mirror,
    /// What the starting file already breaks (real files do): only new package problems count.
    pub pkg_baseline: HashSet<docxtor_core::invariants::Violation>,
    pub stats: Arc<Mutex<Stats>>,
    pub step_no: usize,
}

impl Runner {
    pub fn new(start: &Start, stats: Arc<Mutex<Stats>>) -> Runner {
        let mut doc = start.open();
        // Release builds collect the engine's own violations instead of panicking.
        doc.set_invariant_checks(std::env::var_os("DOCXTOR_REPLAY_TRACE").is_none());
        let bytes = doc.save().unwrap();
        let pkg_baseline = check_package(&bytes).into_iter().collect();
        let mirror = Mirror::new(&mut doc);
        Runner { doc, mirror, pkg_baseline, stats, step_no: 0 }
    }

    pub fn engine_violations(&mut self) -> Result<(), Failure> {
        let found: Vec<_> = self.doc.take_invariant_violations().into_iter().filter(|v| !docxtor_core::invariants_known::is_known(v)).collect();
        if let Some(v) = found.first() {
            let all: Vec<String> = found.iter().map(|v| v.to_string()).collect();
            return Err(Failure::new(v.rule, format!("{} | {}", v.message, all.join(" | "))));
        }
        Ok(())
    }

    pub fn package_ok(&self, bytes: &[u8], when: &str) -> Result<(), Failure> {
        for v in check_package(bytes) {
            if self.pkg_baseline.contains(&v) || docxtor_core::invariants_known::is_known(&v) {
                continue;
            }
            return Err(Failure::new(v.rule, format!("{} ({when})", v.message)));
        }
        Ok(())
    }

    pub fn save(&mut self, when: &str) -> Result<Vec<u8>, Failure> {
        let bytes = self.doc.save().map_err(|e| Failure::new("E20", format!("save failed: {e}")))?;
        if std::env::var_os("DOCXTOR_REPLAY_TRACE").is_some() {
            let broken: Vec<String> = check_package(&bytes).iter().map(|v| v.to_string()).collect();
            eprintln!("  step {}: save ({when}){}", self.step_no, if broken.is_empty() { String::new() } else { format!(": {}", broken.join(" | ")) });
        }
        self.engine_violations()?;
        self.package_ok(&bytes, when)?;
        Ok(bytes)
    }

    /// E5 and E6 (and E1/E2): the saved file is valid, saving again gives the same bytes, and reopening renders the same.
    pub fn check_saved(&mut self, reopen: bool) -> Result<Option<Document>, Failure> {
        let a = self.save("save")?;
        let b = self.save("second save")?;
        if a != b {
            return Err(Failure::new("E5", format!("saving twice without an edit gives different bytes: {}", bytes_diff(&a, &b))));
        }
        let mut again = Document::open(&a).map_err(|e| Failure::new("E6", format!("the saved file does not open again: {e}")))?;
        if let Some(d) = reopened_diff(&self.doc, &again) {
            return Err(Failure::new("E6", format!("open(save(doc)) renders differently: {d}")));
        }
        if reopen {
            again.set_invariant_checks(std::env::var_os("DOCXTOR_REPLAY_TRACE").is_none());
            Ok(Some(again))
        } else {
            Ok(None)
        }
    }

    /// `DOCXTOR_REPLAY_TRACE=1`: after every step, the rules the document breaks (body and stories), then each content
    /// control of the body, whether it shows its placeholder, and its text.
    fn trace(&mut self) {
        // `DOCXTOR_REPLAY_SAVE=<file>`: the document as saved after each step (the last step's stays), to look at.
        if let Some(out) = std::env::var_os("DOCXTOR_REPLAY_SAVE") {
            if let Ok(bytes) = self.doc.save() {
                let _ = std::fs::write(out, bytes);
            }
        }
        if std::env::var_os("DOCXTOR_REPLAY_TRACE").is_some() {
            for v in self.doc.check_invariants() {
                eprintln!("  step {}: breaks {v}", self.step_no);
            }
            for (_, c, text) in self.doc.list_controls() {
                eprintln!("  step {}: {} '{}' placeholder {}: {text:?}", self.step_no, c.kind, c.alias.as_deref().or(c.tag.as_deref()).unwrap_or(""), c.placeholder);
            }
        }
    }

    pub fn step(&mut self, step: &Step) -> Result<(), Failure> {
        self.step_no += 1;
        self.stats.lock().unwrap().steps += 1;
        match step {
            Step::Apply { ops, merge, strict, kind } => {
                self.apply(ops, *merge, *strict, kind)?;
            }
            Step::Undo => {
                self.doc.undo();
            }
            Step::Redo => {
                self.doc.redo();
            }
            Step::UndoN(n) => {
                self.doc.undo_steps(*n);
            }
            Step::Save => {
                self.check_saved(false)?;
            }
            Step::SaveReopen => {
                if let Some(d) = self.check_saved(true)? {
                    self.doc = d;
                    self.mirror = Mirror::new(&mut self.doc);
                }
            }
        }
        if !matches!(step, Step::Apply { .. }) {
            self.trace();
        }
        self.engine_violations()?;
        self.mirror.update(&mut self.doc);
        self.mirror.check(&self.doc)?;
        if self.step_no.is_multiple_of(20) {
            self.check_saved(false)?;
        }
        Ok(())
    }

    pub fn apply(&mut self, ops: &[Value], merge: bool, strict: bool, kind: &str) -> Result<(), Failure> {
        let parsed: Result<Vec<Op>, _> = ops.iter().map(|v| serde_json::from_value::<Op>(v.clone())).collect();
        {
            let mut st = self.stats.lock().unwrap();
            let entry = st.kinds.entry(kind.to_string()).or_default();
            entry.0 += 1;
            if parsed.is_err() {
                entry.3 += 1;
            }
        }
        let Ok(parsed) = parsed else { return Ok(()) };
        let before = model_fp(&self.doc);
        let (undo_n, redo_n) = (self.doc.undo_labels().len(), self.doc.redo_labels().len());
        let rev = self.doc.revision();
        let (bytes0, side0) = if strict {
            let b = self.save("before a step")?;
            let s = side_parts(&b);
            (Some(b), Some(s))
        } else {
            (None, None)
        };
        match self.doc.apply_batch(&parsed, merge) {
            Err(_) => {
                self.stats.lock().unwrap().kinds.get_mut(kind).unwrap().2 += 1;
                let after = model_fp(&self.doc);
                if let Some(d) = fp_diff(&before, &after) {
                    return Err(Failure::new("E10", format!("a refused batch changed the render model: {d}")));
                }
                if self.doc.revision() != rev || self.doc.undo_labels().len() != undo_n || self.doc.redo_labels().len() != redo_n {
                    return Err(Failure::new("E10", "a refused batch changed the revision or the undo history"));
                }
                if let Some(b0) = bytes0 {
                    let b1 = self.save("after a refused step")?;
                    if b0 != b1 {
                        return Err(Failure::new("E10", format!("a refused batch changed the saved file: {}", bytes_diff(&b0, &b1))));
                    }
                }
            }
            Ok(_) => {
                self.stats.lock().unwrap().kinds.get_mut(kind).unwrap().1 += 1;
                self.trace();
                self.engine_violations()?;
                let pushed = self.doc.undo_labels().len() == undo_n + 1 && self.doc.redo_labels().is_empty();
                if !merge && pushed && self.doc.revision() > rev {
                    let after = model_fp(&self.doc);
                    if !self.doc.undo() {
                        return Err(Failure::new("E8", "undo did nothing after an edit that made an undo step"));
                    }
                    self.engine_violations()?;
                    let undone = model_fp(&self.doc);
                    if let Some(d) = fp_diff(&before, &undone) {
                        return Err(Failure::new("E8", format!("apply then undo does not give the model back: {d}")));
                    }
                    if let (Some(s0), true) = (side0, strict) {
                        let s1 = side_parts(&self.save("after undo")?);
                        for (k, v) in &s0 {
                            if s1.get(k) != Some(v) {
                                return Err(Failure::new("E8", format!("apply then undo does not give {k} back")));
                            }
                        }
                    }
                    if !self.doc.redo() {
                        return Err(Failure::new("E9", "redo did nothing after an undo"));
                    }
                    self.engine_violations()?;
                    let redone = model_fp(&self.doc);
                    if let Some(d) = fp_diff(&after, &redone) {
                        return Err(Failure::new("E9", format!("undo then redo does not give the edited model back: {d}")));
                    }
                }
            }
        }
        Ok(())
    }

    /// The checks at the end of a run.
    pub fn finish(&mut self) -> Result<(), Failure> {
        self.check_saved(false)?;
        Ok(())
    }
}

/// Run steps in a thread with a 1 MB stack (as WASM has; a debug build gets 32 MB, its frames are far bigger) and
/// catch panics. `source` gives the next step from the live document, or None to stop. Steps are kept in `log` as they
/// go. A failed check is noted and the run goes on (so one common bug does not hide the others); a panic ends it.
/// Returns every distinct failure of the run, in the order found.
pub fn guarded<F>(start: &Start, log: Arc<Mutex<Vec<Step>>>, stats: Arc<Mutex<Stats>>, mut source: F) -> Vec<Failure>
where
    F: FnMut(&Document, usize) -> Option<Step> + Send + 'static,
{
    quiet_panics_in_runs();
    let start = start.clone();
    let stack = if cfg!(debug_assertions) { 32 << 20 } else { 1 << 20 };
    let found: Arc<Mutex<Vec<Failure>>> = Arc::new(Mutex::new(Vec::new()));
    let found2 = found.clone();
    let h = std::thread::Builder::new()
        .stack_size(stack)
        .spawn(move || {
            IN_GUARD.with(|g| g.set(true));
            let note = |f: Failure| {
                let mut v = found2.lock().unwrap();
                if !v.iter().any(|x| x.key() == f.key()) {
                    v.push(f);
                }
            };
            let r = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut run = Runner::new(&start, stats);
                let mut i = 0;
                while let Some(step) = source(&run.doc, i) {
                    log.lock().unwrap().push(step.clone());
                    i += 1;
                    if let Err(f) = run.step(&step) {
                        note(f);
                        // The client's picture may be half-way through a step that failed: start it again.
                        run.mirror = Mirror::new(&mut run.doc);
                        let _ = run.doc.take_invariant_violations();
                    }
                }
                if let Err(f) = run.finish() {
                    note(f);
                }
            }));
            if let Err(p) = r {
                let at = PANIC_AT.with(|a| a.borrow().clone());
                note(panic_failure(&payload_text(&p), &at));
            }
        })
        .unwrap();
    if let Err(p) = h.join() {
        found.lock().unwrap().push(panic_failure(&payload_text(&p), ""));
    }
    let v = found.lock().unwrap().clone();
    v
}

// ---------------------------------------------------------------------------
// Generating steps
// ---------------------------------------------------------------------------

pub struct Generator {
    pub rng: Rng,
    pub shapes: Arc<Vec<Value>>,
    pub graveyard: Vec<u32>,
    pub n: usize,
}

pub const ID_KEYS_FOR_POISON: &[&str] = &["id", "after", "before", "at", "section", "ids", "p", "block"];

impl Generator {
    pub fn step(&mut self, doc: &Document, i: usize) -> Option<Step> {
        if i >= self.n {
            return None;
        }
        let r = &mut self.rng;
        let roll = r.below(100);
        Some(match roll {
            0..=5 => Step::Undo,
            6..=8 => Step::Redo,
            9..=10 => Step::UndoN(2 + r.below(4)),
            11..=13 => Step::SaveReopen,
            14..=15 => Step::Save,
            _ => return Some(self.apply(doc)),
        })
    }

    pub fn apply(&mut self, doc: &Document) -> Step {
        let live = Live::of(doc, &mut self.graveyard);
        for _ in 0..20 {
            let r = &mut self.rng;
            let batch = if !self.shapes.is_empty() && r.below(100) < 30 {
                let s = r.pick(&self.shapes).clone();
                let name = s["op"].as_str().unwrap_or("").to_string();
                let op = swap_live(&s, &live, r, &name);
                Some((format!("r:{name}"), vec![op]))
            } else {
                let kind = KINDS[r.below(KINDS.len())];
                gen_kind(kind, &live, r).map(|ops| (kind.to_string(), ops))
            };
            let Some((kind, mut ops)) = batch else { continue };
            if self.rng.one_in(4) {
                for _ in 0..(1 + self.rng.below(2)) {
                    let k2 = KINDS[self.rng.below(KINDS.len())];
                    if let Some(more) = gen_kind(k2, &live, &mut self.rng) {
                        ops.extend(more);
                    }
                }
            }
            let poisoned = self.rng.one_in(10);
            if poisoned {
                poison(&mut ops, &live, &self.graveyard, &mut self.rng);
            }
            let merge = self.rng.one_in(12);
            let strict = poisoned || self.rng.one_in(5);
            return Step::Apply { ops, merge, strict, kind };
        }
        Step::Undo
    }
}

/// Make one position or id in the batch wrong: an offset past the end, or a block that is gone.
pub fn poison(ops: &mut [Value], live: &Live, graveyard: &[u32], rng: &mut Rng) {
    pub fn count(v: &Value) -> usize {
        match v {
            Value::Object(m) => {
                let own = usize::from(m.contains_key("p") && m.contains_key("o")) + m.iter().filter(|(k, x)| ID_KEYS_FOR_POISON.contains(&k.as_str()) && x.is_u64()).count();
                own + m.values().map(count).sum::<usize>()
            }
            Value::Array(a) => a.iter().map(count).sum(),
            _ => 0,
        }
    }
    pub fn apply(v: &mut Value, target: &mut usize, dead: u32, rng: &mut Rng) {
        match v {
            Value::Object(m) => {
                if m.contains_key("p") && m.contains_key("o") {
                    if *target == 0 {
                        if rng.one_in(2) {
                            m.insert("o".into(), json!(10_000 + rng.below(100_000)));
                        } else {
                            m.insert("p".into(), json!(dead));
                        }
                    }
                    *target = target.wrapping_sub(1);
                }
                for (k, x) in m.iter_mut() {
                    if ID_KEYS_FOR_POISON.contains(&k.as_str()) && x.is_u64() {
                        if *target == 0 {
                            *x = json!(dead);
                        }
                        *target = target.wrapping_sub(1);
                    } else {
                        apply(x, target, dead, rng);
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| apply(x, target, dead, rng)),
            _ => {}
        }
    }
    let total: usize = ops.iter().map(count).sum();
    if total == 0 {
        return;
    }
    let gone: Vec<u32> = graveyard.iter().copied().filter(|g| !live.all_ids.contains(g)).collect();
    let dead = if gone.is_empty() || rng.one_in(2) { 900_000 + rng.below(1000) as u32 } else { *rng.pick(&gone) };
    let mut target = rng.below(total);
    for o in ops.iter_mut() {
        apply(o, &mut target, dead, rng);
    }
}

