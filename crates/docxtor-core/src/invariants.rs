//! The invariant check: rules that must hold after any edit, written so code can check them
//!
//! `Document::check_invariants` walks the model (ids, tabs, references, content controls) and `check_package`
//! walks saved bytes (XML parts, content types, relationships). Real files from Word break some of these rules
//! already (a bookmark without an end, a style that was never defined), so only violations an edit *adds* count:
//! the set after the edit minus the set before it, matched on rule and message. Messages therefore name the thing
//! (the bookmark, the style) and never a node id that changes.
//!
//! The check runs at the end of `apply_batch`, `undo`, `redo` and `save`, when the build has debug assertions
//! (every `cargo test`) or after `Document::set_invariant_checks(true)` (the browser build for tests). A new
//! violation panics in a debug build, with the rule, the message and the batch's ops; with the runtime switch in a
//! release build it is collected for `take_invariant_violations` instead. Without either it costs one bool test.
//!
//! This module is a child of `document` (it reads fields the document keeps private); `lib.rs` re-exports it.

use super::Document;
use crate::controls;
use crate::model::*;
use crate::ops::Op;
use crate::package::{self, Package};
use crate::stories::StoryKind;
use crate::xml::{self, Element};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// One broken rule: its rule id (`E12`) and a message that names what is wrong.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Violation {
    pub rule: &'static str,
    pub message: String,
}

impl Violation {
    fn new(rule: &'static str, message: impl Into<String>) -> Self {
        Violation { rule, message: message.into() }
    }
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.rule, self.message)
    }
}

/// What the document remembers between checks.
#[derive(Default)]
pub(super) struct Checker {
    /// The runtime switch (`set_invariant_checks`).
    pub on: bool,
    /// The model's violations as of the last check (or the state before the first edit).
    before: Option<Vec<Violation>>,
    /// Everything the model has ever been found to break (before an edit or after one). Undo and redo go back to
    /// states the document was in, so what they bring back is only new when no state had it.
    seen: HashSet<Violation>,
    /// The package's violations as of the last save (or the state before the first edit).
    pkg_before: Option<Vec<Violation>>,
    /// Everything the saved package has ever been found to break. Undo brings back parts an edit had removed (a chart
    /// whose workbook was never in the file has a dangling relationship from the start), so what it brings back is
    /// only new when no earlier save had it.
    pkg_seen: HashSet<Violation>,
    /// New violations found while the switch was on in a build without debug assertions.
    found: Vec<Violation>,
}

impl Document {
    /// Check the rules that hold for the model: E12 (ids), E13 (tabs), E14 (references), E15 (controls).
    /// Everything that is broken now, including what the file came with.
    pub fn check_invariants(&self) -> Vec<Violation> {
        let mut v = Vec::new();
        check_ids(self, &mut v);
        check_tabs(self, &mut v);
        check_references(self, &mut v);
        check_controls(self, &mut v);
        dedupe(v)
    }

    /// Turn the check after every edit on or off for this document (off by default; always on in builds with
    /// debug assertions). Turning it on notes what is broken already, so only what edits add is reported.
    pub fn set_invariant_checks(&mut self, on: bool) {
        self.inv.on = on;
        self.inv.before = None;
        self.inv.seen.clear();
        self.inv.pkg_before = None;
        self.inv.pkg_seen.clear();
        if on {
            self.inv_prime();
        }
    }

    /// The new violations found since the last call (only collected with the runtime switch, in a build without
    /// debug assertions, which panics instead).
    pub fn take_invariant_violations(&mut self) -> Vec<Violation> {
        std::mem::take(&mut self.inv.found)
    }

    #[inline]
    pub(super) fn inv_enabled(&self) -> bool {
        cfg!(debug_assertions) || self.inv.on
    }

    /// Note what is broken before the first change, once.
    fn inv_prime(&mut self) {
        if self.inv.before.is_none() {
            let now = self.check_invariants();
            self.inv.seen.extend(now.iter().cloned());
            self.inv.before = Some(now);
        }
        if self.inv.pkg_before.is_none() {
            let now = crate::perf::unmetered_check(|| check_parts(&self.pkg));
            self.inv.pkg_seen.extend(now.iter().cloned());
            self.inv.pkg_before = Some(now);
        }
    }

    /// Called before a change.
    #[inline]
    pub(super) fn inv_begin(&mut self) {
        if self.inv_enabled() {
            self.inv_prime();
        }
    }

    /// Called after a change that went through: what is broken now and was not before is reported.
    #[inline]
    pub(super) fn inv_end(&mut self, what: &str, ops: Option<&[Op]>) {
        if !self.inv_enabled() {
            return;
        }
        let after = self.check_invariants();
        let before = self.inv.before.replace(after.clone()).unwrap_or_default();
        // An edit is compared with the state before it; undo and redo, with every state the document has been in.
        let new = if ops.is_some() { fresh(&before, after.clone()) } else { after.iter().filter(|v| !self.inv.seen.contains(*v)).cloned().collect() };
        self.inv.seen.extend(after);
        self.inv_report(what, new, ops);
    }

    /// Called with the bytes `save` produced.
    #[inline]
    pub(super) fn inv_saved(&mut self, bytes: &[u8]) {
        if !self.inv_enabled() {
            return;
        }
        let after = check_package(bytes);
        let before = self.inv.pkg_before.replace(after.clone()).unwrap_or_else(|| after.clone());
        let new: Vec<Violation> = fresh(&before, after.clone()).into_iter().filter(|v| !self.inv.pkg_seen.contains(v)).collect();
        self.inv.pkg_seen.extend(after);
        self.inv_report("save", new, None);
    }

    fn inv_report(&mut self, what: &str, new: Vec<Violation>, ops: Option<&[Op]>) {
        if cfg!(debug_assertions) {
            let new: Vec<Violation> = new.into_iter().filter(|v| !crate::invariants_known::is_known(v)).collect();
            if new.is_empty() {
                return;
            }
            let list: Vec<String> = new.iter().map(|v| format!("  {v}")).collect();
            let ops = ops.map(|o| serde_json::to_string(o).unwrap_or_default()).unwrap_or_else(|| "(none)".into());
            panic!("Docxtor invariant broken: after {what}:\n{}\nops: {ops}", list.join("\n"));
        }
        self.inv.found.extend(new);
    }
}

/// What `after` has that `before` does not.
fn fresh(before: &[Violation], after: Vec<Violation>) -> Vec<Violation> {
    let had: HashSet<&Violation> = before.iter().collect();
    after.into_iter().filter(|v| !had.contains(v)).collect()
}

fn dedupe(v: Vec<Violation>) -> Vec<Violation> {
    let mut seen = HashSet::new();
    v.into_iter().filter(|x| seen.insert(x.clone())).collect()
}

// ---------------------------------------------------------------------------
// E12: ids
// ---------------------------------------------------------------------------

/// Every block of the body and of the stories, with the ones inside tables and containers.
fn collect_ids<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut Vec<NodeId>) {
    for b in blocks {
        out.push(b.id());
        match b {
            Block::Table(t) => t.rows().for_each(|r| r.cells().for_each(|c| collect_ids(&c.blocks, out))),
            Block::Container(c) => collect_ids(&c.blocks, out),
            Block::Para(_) | Block::Opaque(_) => {}
        }
    }
}

/// E12: ids are unique across the body and all stories (headers, footers, notes, text boxes, imported chunks).
/// (The ids of the render order are the body's top-level blocks, which are in the list by construction.)
fn check_ids(d: &Document, out: &mut Vec<Violation>) {
    let mut ids = Vec::new();
    collect_ids(d.main_body(), &mut ids);
    for s in &d.stories {
        collect_ids(&s.blocks, &mut ids);
    }
    let mut seen: HashSet<NodeId> = HashSet::with_capacity(ids.len());
    for id in ids {
        if !seen.insert(id) {
            out.push(Violation::new("E12", format!("block id {id} is used by more than one block")));
        }
    }
}

// ---------------------------------------------------------------------------
// E13: tabs
// ---------------------------------------------------------------------------

/// E13: every tab of the list starts at a marker in the body, in order, each tab after the first right after a
/// section break, and the tabs' blocks add up to the body's content.
fn check_tabs(d: &Document, out: &mut Vec<Violation>) {
    let Some(list) = d.tabs.list.as_ref() else { return };
    let body = d.main_body();
    let content = body.iter().rposition(|b| !matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).map_or(0, |i| i + 1);
    let info = d.tabs_info();
    let found: HashSet<&str> = info.tabs.iter().map(|t| t.id.as_str()).collect();
    let mut all_found = true;
    for t in &list.tabs {
        if !found.contains(t.id.as_str()) {
            all_found = false;
            out.push(Violation::new("E13", format!("tab '{}' has no start marker in the body, or it is out of order", t.id)));
        }
    }
    for t in &info.tabs {
        if !body.iter().take(content).any(|b| b.id() == t.start) {
            out.push(Violation::new("E13", format!("tab '{}' starts at a block that is not in the body", t.id)));
        }
    }
    let total: usize = info.tabs.iter().map(|t| t.blocks).sum();
    if all_found && total != content {
        out.push(Violation::new("E13", format!("the tabs hold {total} blocks together, the body has {content}")));
    }
    if all_found && !d.tabs_structure_ok() {
        out.push(Violation::new("E13", "a tab does not start right after a section break"));
    }
}

// ---------------------------------------------------------------------------
// E14 and E15: what the text refers to, and content controls
// ---------------------------------------------------------------------------

/// What one walk over a list of blocks finds.
#[derive(Default)]
struct Facts {
    para_styles: Vec<String>,
    char_styles: Vec<String>,
    num_ids: Vec<i64>,
    anchors: Vec<String>,
    /// Bookmark starts (id, name) and ends (id).
    starts: Vec<(String, String)>,
    ends: Vec<String>,
    /// Note references: (is an endnote, id).
    notes: Vec<(bool, String)>,
    /// Controls that show their placeholder while holding other text: how to name them.
    bad_controls: Vec<String>,
}

struct Walk<'a> {
    pkg: &'a Package,
    main: &'a str,
    f: Facts,
}

impl Walk<'_> {
    fn blocks<'a>(&mut self, blocks: impl IntoIterator<Item = &'a Block>) {
        for b in blocks {
            match b {
                Block::Para(p) => self.para(p),
                Block::Table(t) => {
                    for c in &t.children {
                        match c {
                            TableChild::Row(r) => {
                                for rc in &r.children {
                                    match rc {
                                        RowChild::Cell(cell) => self.blocks(&cell.blocks),
                                        RowChild::Other(e) | RowChild::Marker(e, _) => self.markers(e),
                                    }
                                }
                            }
                            TableChild::Other(e) | TableChild::Marker(e, _) => self.markers(e),
                        }
                    }
                }
                // An imported chunk holds another document's blocks, with that document's styles and lists.
                Block::Container(c) if is_alt_chunk(c) => {}
                Block::Container(c) => {
                    if c.shell.child("w:sdtPr").is_some() {
                        self.control(&c.shell, controls::blocks_text(&c.blocks));
                    }
                    self.blocks(&c.blocks);
                }
                Block::Opaque(o) => self.markers(&o.el),
            }
        }
    }

    fn para(&mut self, p: &Paragraph) {
        if let Some(ppr) = &p.ppr {
            if let Some(s) = ppr.child("w:pStyle").and_then(|e| e.attr("w:val")) {
                self.f.para_styles.push(s.to_string());
            }
            if let Some(n) = ppr.child("w:numPr").and_then(|n| n.child("w:numId")).and_then(|e| e.attr("w:val")).and_then(|v| v.trim().parse::<i64>().ok()) {
                self.f.num_ids.push(n);
            }
            if let Some(s) = ppr.child("w:rPr").and_then(|r| r.child("w:rStyle")).and_then(|e| e.attr("w:val")) {
                self.f.char_styles.push(s.to_string());
            }
        }
        self.inlines(&p.inlines);
    }

    fn inlines(&mut self, inlines: &[Inline]) {
        for i in inlines {
            match i {
                Inline::Run(r) => {
                    if let Some(s) = r.rpr.as_ref().and_then(|x| x.child("w:rStyle")).and_then(|e| e.attr("w:val")) {
                        self.f.char_styles.push(s.to_string());
                    }
                    for item in &r.items {
                        if let RunItem::Object(e) = item {
                            match e.name.as_str() {
                                "w:footnoteReference" => self.f.notes.push((false, e.attr("w:id").unwrap_or_default().to_string())),
                                "w:endnoteReference" => self.f.notes.push((true, e.attr("w:id").unwrap_or_default().to_string())),
                                _ => {}
                            }
                        }
                    }
                }
                Inline::Container(c) => {
                    if c.kind == InlineKind::Hyperlink {
                        if let Some(a) = c.shell.attr("w:anchor") {
                            self.f.anchors.push(a.to_string());
                        }
                    }
                    if c.kind == InlineKind::Sdt && c.shell.child("w:sdtPr").is_some() {
                        self.control(&c.shell, controls::inline_text(&c.children));
                    }
                    self.inlines(&c.children);
                }
                Inline::Opaque(e) => self.markers(e),
            }
        }
    }

    /// Bookmark starts and ends in an element the model keeps as it came, and inside it.
    fn markers(&mut self, e: &Element) {
        match e.name.as_str() {
            "w:bookmarkStart" => self.f.starts.push((e.attr("w:id").unwrap_or_default().to_string(), e.attr("w:name").unwrap_or_default().to_string())),
            "w:bookmarkEnd" => self.f.ends.push(e.attr("w:id").unwrap_or_default().to_string()),
            _ => {}
        }
        for c in e.elements() {
            self.markers(c);
        }
    }

    /// E15 for one control: showing its placeholder while holding other text.
    fn control(&mut self, shell: &Element, text: String) {
        let Some(pr) = shell.child("w:sdtPr") else { return };
        let v = controls::read_props(pr);
        if !v.placeholder || !controls::is_textual(&v.kind) || text.is_empty() {
            return;
        }
        let ph = controls::placeholder_name(pr)
            .and_then(|n| controls::glossary_text(self.pkg, self.main, &n))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| {
                match v.kind.as_str() {
                    "date" => controls::PLACEHOLDER_DATE,
                    "dropDown" | "comboBox" => controls::PLACEHOLDER_CHOICE,
                    "picture" => controls::PLACEHOLDER_PICTURE,
                    _ => controls::PLACEHOLDER_TEXT,
                }
                .1
                .to_string()
            });
        if text != ph {
            let name = v.alias.or(v.tag).map(|n| format!(" '{n}'")).unwrap_or_default();
            self.f.bad_controls.push(format!("the {} content control{name}", v.kind));
        }
    }
}

/// The body and every story, each with a name that says where it is.
fn lists(d: &Document) -> Vec<(String, crate::body::BlocksRef<'_>)> {
    use crate::body::BlocksRef;
    let mut v: Vec<(String, BlocksRef)> = vec![("the body".to_string(), BlocksRef::Top(d.main_body()))];
    for s in &d.stories {
        let what = match s.kind {
            StoryKind::Header => "header",
            StoryKind::Footer => "footer",
            StoryKind::Footnote => "footnote",
            StoryKind::Endnote => "endnote",
            StoryKind::TextBox => "text box",
        };
        v.push((format!("the {what} in {}", s.part), BlocksRef::List(&s.blocks)));
    }
    v
}

fn facts_of(d: &Document) -> Vec<(String, Facts)> {
    lists(d)
        .into_iter()
        .map(|(name, blocks)| {
            let mut w = Walk { pkg: &d.pkg, main: &d.main_path, f: Facts::default() };
            w.blocks(blocks);
            (name, w.f)
        })
        .collect()
}

/// E14: every reference resolves.
fn check_references(d: &Document, out: &mut Vec<Violation>) {
    let facts = facts_of(d);
    // Word matches bookmark names without regard to case, and so does the engine (rename_bookmark, go to).
    let bookmarks: HashSet<String> = facts.iter().flat_map(|(_, f)| f.starts.iter().map(|(_, n)| n.to_lowercase())).collect();
    let notes: HashSet<(bool, &str)> = d
        .stories
        .iter()
        .filter_map(|s| match (s.kind, s.note_id.as_deref()) {
            (StoryKind::Footnote, Some(id)) => Some((false, id)),
            (StoryKind::Endnote, Some(id)) => Some((true, id)),
            _ => None,
        })
        .collect();
    for (_, f) in &facts {
        for s in &f.para_styles {
            if !d.styles.exists(s) {
                out.push(Violation::new("E14", format!("paragraph style '{s}' is used but not defined")));
            }
        }
        for s in &f.char_styles {
            if !d.styles.exists(s) {
                out.push(Violation::new("E14", format!("character style '{s}' is used but not defined")));
            }
        }
        for n in &f.num_ids {
            if *n != 0 && d.numbering.abstract_of(*n).is_none() {
                out.push(Violation::new("E14", format!("list {n} is used but not defined")));
            }
        }
        for a in &f.anchors {
            if !bookmarks.contains(&a.to_lowercase()) {
                out.push(Violation::new("E14", format!("a link points at bookmark '{a}', which does not exist")));
            }
        }
        for (end, id) in &f.notes {
            if !notes.contains(&(*end, id.as_str())) {
                out.push(Violation::new("E14", format!("{} reference {id} has no {}", if *end { "endnote" } else { "footnote" }, if *end { "endnote" } else { "footnote" })));
            }
        }
    }
    // Starts and ends pair up by id, inside the body and inside each story.
    for (_, f) in &facts {
        let ends: HashSet<&str> = f.ends.iter().map(String::as_str).collect();
        let starts: HashMap<&str, &str> = f.starts.iter().map(|(i, n)| (i.as_str(), n.as_str())).collect();
        for (id, _) in &f.starts {
            if !ends.contains(id.as_str()) {
                // By id and not by name: renaming a bookmark the file already left without an end is not a new problem.
                out.push(Violation::new("E14", format!("a bookmark start (id {id}) has no end")));
            }
        }
        for id in &f.ends {
            if !starts.contains_key(id.as_str()) {
                out.push(Violation::new("E14", format!("a bookmark end (id {id}) has no start")));
            }
        }
    }
}

/// E15: no content control shows its placeholder while it holds other text, in the body and in every story.
fn check_controls(d: &Document, out: &mut Vec<Violation>) {
    for (place, f) in facts_of(d) {
        for c in f.bad_controls {
            out.push(Violation::new("E15", format!("{c} in {place} is marked as showing its placeholder but holds other text")));
        }
    }
}

// ---------------------------------------------------------------------------
// The saved file: E1 and E2
// ---------------------------------------------------------------------------

/// Check saved bytes: E1 (every XML part parses and declares its namespaces) and E2 (every part has a content
/// type, every relationship inside the package points at a part that exists, every `r:id`, `r:embed` and `r:link`
/// used in a part is in that part's relationships). External relationships are exempt.
pub fn check_package(bytes: &[u8]) -> Vec<Violation> {
    match Package::open(bytes) {
        Ok(pkg) => check_parts(&pkg),
        Err(e) => vec![Violation::new("E1", format!("the file does not open as a package: {e}"))],
    }
}

fn is_xml(name: &str) -> bool {
    name.ends_with(".xml") || name.ends_with(".rels")
}

fn check_parts(pkg: &Package) -> Vec<Violation> {
    let mut out = Vec::new();
    // E1, and the parsed parts for E2.
    let mut docs: HashMap<&str, xml::XmlDoc> = HashMap::new();
    for part in &pkg.parts {
        if !is_xml(&part.name) {
            continue;
        }
        match xml::parse(&pkg.bytes_of(part)) {
            Ok(doc) => {
                if let Err(e) = xml::check_namespaces(&doc) {
                    out.push(Violation::new("E1", format!("{}: {e}", part.name)));
                }
                docs.insert(part.name.as_str(), doc);
            }
            Err(e) => out.push(Violation::new("E1", format!("{}: {e}", part.name))),
        }
    }
    check_content_types(pkg, &docs, &mut out);
    check_relationships(pkg, &docs, &mut out);
    dedupe(out)
}

fn check_content_types(pkg: &Package, docs: &HashMap<&str, xml::XmlDoc>, out: &mut Vec<Violation>) {
    let Some(ct) = docs.get("[Content_Types].xml") else {
        if !pkg.has("[Content_Types].xml") {
            out.push(Violation::new("E2", "the package has no [Content_Types].xml"));
        }
        return;
    };
    let defaults: HashSet<String> = ct.root.children_named("Default").filter_map(|e| e.attr("Extension")).map(|e| e.to_ascii_lowercase()).collect();
    let overrides: HashSet<&str> = ct.root.children_named("Override").filter_map(|e| e.attr("PartName")).map(|p| p.trim_start_matches('/')).collect();
    for part in &pkg.parts {
        if part.name == "[Content_Types].xml" {
            continue;
        }
        let ext = part.name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if !overrides.contains(part.name.as_str()) && !defaults.contains(&ext) {
            out.push(Violation::new("E2", format!("{} has no content type", part.name)));
        }
    }
}

/// The part a relationships part belongs to (`word/_rels/document.xml.rels` is `word/document.xml`).
fn source_of(rels: &str) -> Option<String> {
    let stem = rels.strip_suffix(".rels")?;
    if stem == "_rels/" || stem == "_rels/." || rels == "_rels/.rels" {
        return Some(String::new());
    }
    match stem.rfind("/_rels/") {
        Some(i) => Some(format!("{}/{}", &stem[..i], &stem[i + "/_rels/".len()..])),
        None => stem.strip_prefix("_rels/").map(String::from),
    }
}

fn used_rel_ids(e: &Element, out: &mut Vec<(String, String)>) {
    for (k, v) in &e.attrs {
        if matches!(k.as_str(), "r:id" | "r:embed" | "r:link") {
            out.push((k.to_string(), v.to_string()));
        }
    }
    for c in e.elements() {
        used_rel_ids(c, out);
    }
}

fn check_relationships(pkg: &Package, docs: &HashMap<&str, xml::XmlDoc>, out: &mut Vec<Violation>) {
    // Each relationships part: its ids, and where its internal ones point.
    let mut ids_of: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, doc) in docs.iter().filter(|(n, _)| n.ends_with(".rels")) {
        let Some(source) = source_of(name) else { continue };
        let mut ids = HashSet::new();
        for e in doc.root.elements().filter(|e| e.local() == "Relationship") {
            let id = e.attr("Id").unwrap_or_default();
            ids.insert(id.to_string());
            if e.attr("TargetMode") == Some("External") {
                continue;
            }
            let target = e.attr("Target").unwrap_or_default();
            let target = target.split('#').next().unwrap_or_default();
            let path = package::resolve_target(&source, target);
            if !pkg.has(&path) {
                out.push(Violation::new("E2", format!("{name}: relationship '{id}' points at '{path}', which is not in the package")));
            }
        }
        ids_of.insert(source, ids);
    }
    for (name, doc) in docs.iter().filter(|(n, _)| !n.ends_with(".rels")) {
        let mut used = Vec::new();
        used_rel_ids(&doc.root, &mut used);
        for (attr, id) in used {
            if !ids_of.get(*name).is_some_and(|ids| ids.contains(&id)) {
                out.push(Violation::new("E2", format!("{name}: {attr}=\"{id}\" has no relationship in {}", package::rels_path(name))));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> Document {
        let mut d = Document::new_blank();
        let first = d.render_all().order[0];
        d.apply_json(&json!({"op":"insert_markdown","before":first,"markdown":"One\n\nTwo"}).to_string(), false).unwrap();
        d
    }

    fn rules(d: &Document) -> Vec<(&'static str, String)> {
        d.check_invariants().into_iter().map(|v| (v.rule, v.message)).collect()
    }

    fn has(d: &Document, rule: &str, part: &str) -> bool {
        rules(d).iter().any(|(r, m)| *r == rule && m.contains(part))
    }

    /// The first paragraph, to change behind the ops' back. It is marked as changed, as an op would: what the engine keeps
    /// per block (`document/facts.rs`) is read again from it, and only the invariant checks are left to notice.
    fn first_para(d: &mut Document) -> &mut Paragraph {
        d.touch_all();
        d.body.iter_mut_all().find_map(|b| if let Block::Para(p) = b { Some(p) } else { None }).unwrap()
    }

    fn run(text: &str) -> Inline {
        Inline::Run(Run { items: vec![RunItem::Text(text.into())], ..Default::default() })
    }

    #[test]
    fn a_clean_edit_reports_nothing() {
        let mut d = doc();
        assert_eq!(d.check_invariants(), vec![]);
        let first = d.render_all().order[0];
        d.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}).to_string(), false).unwrap();
        d.undo();
        d.redo();
        assert_eq!(d.check_invariants(), vec![]);
        let bytes = d.save().unwrap();
        assert_eq!(check_package(&bytes), vec![]);
        assert!(d.take_invariant_violations().is_empty());
    }

    #[test]
    fn e12_a_repeated_id() {
        let mut d = doc();
        let copy = d.body[0].clone();
        d.body.insert(1, copy);
        assert!(has(&d, "E12", "used by more than one block"));
    }

    #[test]
    fn e12_an_id_in_a_story_and_the_body() {
        let mut d = doc();
        let id = d.body[0].id();
        d.stories.push(crate::stories::Story { kind: StoryKind::Header, part: "word/header1.xml".into(), note_id: None, blocks: vec![Block::Para(Paragraph::new(id))], gen: 0 });
        assert!(has(&d, "E12", "more than one"));
    }

    #[test]
    fn e13_a_tab_that_lost_its_marker() {
        let mut d = doc();
        d.apply_json(&json!({"op":"tab","action":"add","name":"Second"}).to_string(), false).unwrap();
        assert_eq!(d.check_invariants(), vec![]);
        // Take the second tab's start marker out of the body.
        for b in d.body.iter_mut_all() {
            if let Block::Para(p) = b {
                p.inlines.retain(|i| !matches!(i, Inline::Opaque(e) if e.name == "w:bookmarkStart"));
            }
        }
        assert!(has(&d, "E13", "no start marker"));
    }

    #[test]
    fn e14_missing_styles_lists_and_bookmarks() {
        let mut d = doc();
        {
            let p = first_para(&mut d);
            let mut ppr = Element::new("w:pPr");
            ppr.children.push(xml::Node::Element(Element::new("w:pStyle").with_attr("w:val", "NoSuchStyle")));
            ppr.children.push(xml::Node::Element(Element::new("w:numPr").with_child(Element::new("w:numId").with_attr("w:val", "999"))));
            p.ppr = Some(ppr);
            p.inlines.push(Inline::Run(Run { rpr: Some(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", "NoCharStyle"))), items: vec![RunItem::Text("x".into())], ..Default::default() }));
            p.inlines.push(Inline::Container(InlineContainer { kind: InlineKind::Hyperlink, shell: Element::new("w:hyperlink").with_attr("w:anchor", "Nowhere"), content_in: None, children: vec![run("link")] }));
            p.inlines.push(Inline::Opaque(Element::new("w:bookmarkStart").with_attr("w:id", "7").with_attr("w:name", "Lonely")));
            p.inlines.push(Inline::Opaque(Element::new("w:bookmarkEnd").with_attr("w:id", "8")));
            p.inlines.push(Inline::Run(Run { items: vec![RunItem::Object(Element::new("w:footnoteReference").with_attr("w:id", "42"))], ..Default::default() }));
        }
        assert!(has(&d, "E14", "paragraph style 'NoSuchStyle'"));
        assert!(has(&d, "E14", "character style 'NoCharStyle'"));
        assert!(has(&d, "E14", "list 999"));
        assert!(has(&d, "E14", "bookmark 'Nowhere'"));
        assert!(has(&d, "E14", "bookmark start (id 7) has no end"));
        assert!(has(&d, "E14", "bookmark end (id 8) has no start"));
        assert!(has(&d, "E14", "footnote reference 42"));
    }

    #[test]
    fn e14_a_link_to_a_bookmark_in_a_story_resolves() {
        let mut d = doc();
        let mut p = Paragraph::new(9000);
        p.inlines.push(Inline::Opaque(Element::new("w:bookmarkStart").with_attr("w:id", "1").with_attr("w:name", "InHeader")));
        p.inlines.push(Inline::Opaque(Element::new("w:bookmarkEnd").with_attr("w:id", "1")));
        d.stories.push(crate::stories::Story { kind: StoryKind::Header, part: "word/header1.xml".into(), note_id: None, blocks: vec![Block::Para(p)], gen: 0 });
        first_para(&mut d).inlines.push(Inline::Container(InlineContainer { kind: InlineKind::Hyperlink, shell: Element::new("w:hyperlink").with_attr("w:anchor", "InHeader"), content_in: None, children: vec![run("link")] }));
        assert_eq!(d.check_invariants(), vec![]);
    }

    fn control(text: &str) -> Inline {
        let pr = Element::new("w:sdtPr").with_child(Element::new("w:alias").with_attr("w:val", "Name")).with_child(Element::new("w:showingPlcHdr")).with_child(Element::new("w:text"));
        Inline::Container(InlineContainer { kind: InlineKind::Sdt, shell: Element::new("w:sdt").with_child(pr), content_in: Some("w:sdtContent".into()), children: vec![run(text)] })
    }

    #[test]
    fn e15_a_control_showing_its_placeholder_with_other_text_in_a_story() {
        let mut d = doc();
        let mut p = Paragraph::new(9001);
        p.inlines.push(control("Typed by someone"));
        d.stories.push(crate::stories::Story { kind: StoryKind::Footer, part: "word/footer1.xml".into(), note_id: None, blocks: vec![Block::Para(p)], gen: 0 });
        assert!(has(&d, "E15", "'Name'"));
        // The same in the body, and the placeholder itself is fine.
        let mut d = doc();
        first_para(&mut d).inlines.push(control("Typed by someone"));
        assert!(has(&d, "E15", "in the body"));
        let mut d = doc();
        first_para(&mut d).inlines.push(control(controls::PLACEHOLDER_TEXT.1));
        assert_eq!(d.check_invariants(), vec![]);
    }

    #[test]
    fn only_new_violations_count() {
        let mut d = doc();
        first_para(&mut d).inlines.push(Inline::Opaque(Element::new("w:bookmarkStart").with_attr("w:id", "7").with_attr("w:name", "Old")));
        // Real files bring such problems along: what is there already is not reported again.
        d.set_invariant_checks(true);
        let first = d.render_all().order[0];
        d.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}).to_string(), false).unwrap();
        assert!(d.take_invariant_violations().is_empty());
    }

    #[test]
    #[should_panic(expected = "Docxtor invariant broken:")]
    fn an_edit_that_breaks_a_rule_panics_in_test_builds() {
        let mut d = doc();
        let first = d.render_all().order[0];
        d.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}).to_string(), false).unwrap();
        // Now break the model behind the engine's back; the next edit is blamed for it.
        first_para(&mut d).inlines.push(Inline::Opaque(Element::new("w:bookmarkStart").with_attr("w:id", "7").with_attr("w:name", "Lonely")));
        d.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"y"}).to_string(), false).unwrap();
    }

    #[test]
    fn package_checks() {
        let d = doc();
        let mut d = d;
        let bytes = d.save().unwrap();
        assert_eq!(check_package(&bytes), vec![]);
        // A relationship to a part that is not there, a part with no content type, an id with no relationship, an undeclared prefix.
        let mut pkg = Package::open(&bytes).unwrap();
        pkg.set("word/_rels/document.xml.rels", {
            let mut s = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
            s = s.replace("</Relationships>", "<Relationship Id=\"rIdX\" Type=\"http://x/image\" Target=\"media/gone.png\"/><Relationship Id=\"rIdE\" Type=\"http://x/hyperlink\" Target=\"https://example.com\" TargetMode=\"External\"/></Relationships>");
            s.into_bytes()
        });
        pkg.set("word/orphan.dat", vec![1, 2, 3]);
        pkg.set("word/prefix.xml", b"<w:x/>".to_vec());
        let mut doc_xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
        doc_xml = doc_xml.replacen("<w:body>", "<w:body><w:p><w:hyperlink r:id=\"rIdNone\"><w:r><w:t>x</w:t></w:r></w:hyperlink></w:p>", 1);
        pkg.set("word/document.xml", doc_xml.into_bytes());
        let v = check_package(&pkg.to_bytes().unwrap());
        let has = |rule: &str, s: &str| v.iter().any(|x| x.rule == rule && x.message.contains(s));
        assert!(has("E2", "relationship 'rIdX' points at 'word/media/gone.png'"), "{v:?}");
        assert!(!v.iter().any(|x| x.message.contains("rIdE")), "{v:?}");
        assert!(has("E2", "word/orphan.dat has no content type"), "{v:?}");
        assert!(has("E1", "word/prefix.xml"), "{v:?}");
        assert!(has("E2", "r:id=\"rIdNone\""), "{v:?}");
    }
}
