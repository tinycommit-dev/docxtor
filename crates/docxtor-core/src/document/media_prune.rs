//! Leaving pictures, charts and workbooks that nothing uses out of the saved file.
//!
//! Deleting a picture or a chart takes it out of the text but not out of the package: undo has to be able to bring
//! it back. So the parts stay in memory, and `save` leaves out the ones nothing refers to any more, together with
//! their relationships and content-type overrides. Word does the same on every save.
//!
//! Only parts that were in use when the file was opened, or that Docxtor added, can be left out. A part that was
//! already unused in the file as it came in is kept as it is: that is the file's own content and Docxtor does not
//! decide it is garbage. A file nobody edited is saved byte for byte, so nothing here runs for it.
//!
//! What counts as "in use" is chosen to err on the side of keeping:
//! - only parts in `word/media`, `word/charts` and `word/embeddings` (and a `media` folder of the glossary) can go;
//! - a relationship from any other part (body, headers, footers, notes, comments, glossary, numbering, VML, theme,
//!   parts Docxtor does not model) uses its target when the part's text names the relationship id anywhere, or when
//!   the part's text cannot be read as markup, or when the relationship is of a type this code does not know;
//! - the parts of a chart (workbook, colours, style) are used whenever the chart is, with no id needed;
//! - a part whose file name stem appears in the text of any other part is kept (a reference by path);
//! - relationships of the package itself (`_rels/.rels`) always count.

use super::picture_watermark::rels_source;
use super::*;
use crate::package::Data;
use std::collections::HashSet;

/// Whether a part is one that may be left out when nothing uses it.
fn is_candidate(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    !l.ends_with(".rels") && (l.starts_with("word/media/") || l.starts_with("word/charts/") || l.starts_with("word/embeddings/") || (l.starts_with("word/") && l.contains("/media/")))
}

/// Relationship types whose id is written in the source part's markup: only for these does an id nobody writes
/// mean the relationship is unused.
fn names_id_in_markup(rel_type: &str) -> bool {
    matches!(rel_type.rsplit('/').next().unwrap_or(""), "image" | "chart" | "chartEx" | "oleObject" | "package" | "hdphoto")
}

struct Rel {
    id: String,
    rel_type: String,
    target: String,
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// The markup of a part as text (in several texts when the part is kept as pieces, cut between elements, so no word or
/// attribute value is cut), or None when it is not markup we can read (binary, UTF-16...).
fn text_of(data: Data<'_>) -> Option<Vec<&str>> {
    let mut out = Vec::new();
    for (i, c) in data.chunks().enumerate() {
        let c = if i == 0 { c.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(c) } else { c };
        let s = std::str::from_utf8(c).ok()?;
        if i == 0 && !s.trim_start().starts_with('<') {
            return None;
        }
        out.push(s);
    }
    (!out.is_empty()).then_some(out)
}

/// Every run of letters, digits and underscores in the text.
fn tokens(s: &str, mut f: impl FnMut(&str)) {
    let b = s.as_bytes();
    let mut start = None;
    for (i, c) in b.iter().enumerate() {
        let word = c.is_ascii_alphanumeric() || *c == b'_';
        match (word, start) {
            (true, None) => start = Some(i),
            (false, Some(st)) => {
                f(&s[st..i]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(st) = start {
        f(&s[st..]);
    }
}

/// Every attribute value (`="..."`) in the text.
fn attr_values(s: &str, mut f: impl FnMut(&str)) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                let q = b[j];
                if let Some(end) = b[j + 1..].iter().position(|c| *c == q) {
                    f(&s[j + 1..j + 1 + end]);
                    i = j + 1 + end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// The part names the package lists (the data a part has once `skip` and `replace` are applied).
fn effective<'a>(pkg: &'a Package, skip: &BTreeSet<String>, replace: &'a HashMap<String, Vec<u8>>) -> Vec<(&'a str, Data<'a>)> {
    pkg.parts
        .iter()
        .filter(|p| !skip.contains(&p.name))
        .map(|p| (p.name.as_str(), replace.get(&p.name).map(|d| Data::One(d.as_slice())).unwrap_or_else(|| pkg.data_of(p))))
        .collect()
}

/// The candidate parts nothing uses, except the ones in `keep`. Empty when in doubt.
fn mark<'a>(candidates: &HashMap<String, &'a str>, name: &str, live: &mut HashSet<&'a str>, queue: &mut Vec<&'a str>) {
    if let Some(found) = candidates.get(&name.to_ascii_lowercase()) {
        if live.insert(found) {
            queue.push(found);
        }
    }
}

fn unused_parts<'a>(parts: &[(&'a str, Data<'a>)], keep: &BTreeSet<String>) -> BTreeSet<String> {
    let candidates: HashMap<String, &str> = parts.iter().filter(|(n, _)| is_candidate(n)).map(|(n, _)| (n.to_ascii_lowercase(), *n)).collect();
    if candidates.is_empty() {
        return BTreeSet::new();
    }
    let present: HashSet<&str> = parts.iter().map(|(n, _)| *n).collect();
    let resolve = |source: &str, target: &str| -> Option<&str> {
        let t = package::resolve_target(source, target);
        candidates.get(&t.to_ascii_lowercase()).or_else(|| candidates.get(&percent_decode(&t).to_ascii_lowercase())).copied()
    };
    // The relationships that point at a candidate, by the part they belong to.
    let mut rels_of: HashMap<String, Vec<Rel>> = HashMap::new();
    for (name, data) in parts.iter().filter(|(n, _)| n.ends_with(".rels")) {
        let Ok(doc) = xml::parse(&data.to_cow()) else { return BTreeSet::new() };
        let source = rels_source(name);
        for e in doc.root.elements().filter(|e| e.local() == "Relationship" && e.attr("TargetMode") != Some("External")) {
            if let Some(target) = resolve(&source, e.attr("Target").unwrap_or_default()) {
                rels_of.entry(source.clone()).or_default().push(Rel { id: e.attr("Id").unwrap_or_default().to_string(), rel_type: e.attr("Type").unwrap_or_default().to_string(), target: target.to_string() });
            }
        }
    }
    let data_of: HashMap<&str, Data<'_>> = parts.iter().copied().collect();
    let mut live: HashSet<&str> = HashSet::new();
    let mut queue: Vec<&str> = Vec::new();
    for k in keep {
        mark(&candidates, k, &mut live, &mut queue);
    }
    // A part that is not a candidate always counts as in use; it uses what its markup names.
    for (source, rels) in &rels_of {
        if candidates.contains_key(&source.to_ascii_lowercase()) {
            continue;
        }
        let ids: HashSet<&str> = rels.iter().filter(|r| names_id_in_markup(&r.rel_type)).map(|r| r.id.as_str()).collect();
        let mut named: HashSet<&str> = HashSet::new();
        let readable = if source.is_empty() || !present.contains(source.as_str()) {
            false
        } else {
            match data_of.get(source.as_str()).and_then(|d| text_of(*d)) {
                Some(texts) => {
                    if !ids.is_empty() {
                        for text in texts {
                            tokens(text, |t| {
                                if let Some(id) = ids.get(t) {
                                    named.insert(id);
                                }
                            });
                            attr_values(text, |v| {
                                if let Some(id) = ids.get(v) {
                                    named.insert(id);
                                }
                            });
                        }
                    }
                    true
                }
                None => false,
            }
        };
        for r in rels {
            if !readable || !names_id_in_markup(&r.rel_type) || named.contains(r.id.as_str()) {
                mark(&candidates, &r.target, &mut live, &mut queue);
            }
        }
    }
    // The parts of a part in use are in use.
    let spread = |live: &mut HashSet<&'a str>, queue: &mut Vec<&'a str>| {
        while let Some(c) = queue.pop() {
            for r in rels_of.get(c).into_iter().flatten() {
                mark(&candidates, &r.target, live, queue);
            }
        }
    };
    spread(&mut live, &mut queue);
    let mut unused: BTreeSet<String> = candidates.values().filter(|n| !live.contains(*n)).map(|n| n.to_string()).collect();
    if unused.is_empty() {
        return unused;
    }
    // A reference by path rather than by relationship: the file name appears in another part's text.
    let stems: HashMap<String, Vec<&str>> = unused.iter().fold(HashMap::new(), |mut m, n| {
        let stem = n.rsplit('/').next().unwrap_or(n).rsplit_once('.').map(|(s, _)| s).unwrap_or(n);
        if !stem.is_empty() && stem.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
            m.entry(stem.to_string()).or_default().push(n.as_str());
        }
        m
    });
    if !stems.is_empty() {
        for (name, data) in parts {
            if name.ends_with(".rels") || *name == "[Content_Types].xml" || is_candidate(name) {
                continue;
            }
            let Some(texts) = text_of(*data) else { continue };
            for text in texts {
                tokens(text, |t| {
                    if let Some(owners) = stems.get(t) {
                        for o in owners {
                            mark(&candidates, o, &mut live, &mut queue);
                        }
                    }
                });
            }
        }
        spread(&mut live, &mut queue);
        unused.retain(|n| !live.contains(n.as_str()));
    }
    unused
}

impl Document {
    /// Remember which pictures, charts and workbooks nothing uses in the file as it came in, before the first edit.
    /// They are the file's own and are kept when saving; see the module comment.
    pub(super) fn ensure_media_baseline(&mut self) {
        if self.baseline_orphans.is_none() {
            let (none, same) = (BTreeSet::new(), HashMap::new());
            let parts = effective(&self.pkg, &none, &same);
            self.baseline_orphans = Some(unused_parts(&parts, &none));
        }
    }

    /// The pictures (`word/media/...`) in the package that nothing in the document uses now: what a save would leave
    /// out. They stay in the package for undo. Empty until the document has been edited. The page uses this to let go
    /// of the object URLs it made for pictures that are gone from the document.
    ///
    /// Asking changes nothing the document reports: which parts a save has to rewrite and whether the document is
    /// modified stay as they were. (The parts are written to the package first, since the text in memory is what
    /// counts; that costs a serialisation of what changed, so the page asks when it is idle, not while drawing.)
    pub fn unused_media(&mut self) -> Vec<String> {
        let Some(keep) = &self.baseline_orphans else { return Vec::new() };
        let keep = keep.clone();
        let marks = self.change_marks();
        self.flush_parts();
        self.restore_change_marks(marks);
        let (none, same) = (BTreeSet::new(), HashMap::new());
        unused_parts(&effective(&self.pkg, &none, &same), &keep).into_iter().filter(|n| n.starts_with("word/media/")).collect()
    }

    /// Add to `skip` and `replace` (what `Package::to_bytes_with` leaves out and rewrites) for the pictures, charts
    /// and workbooks of `pkg`, the package as it is about to be saved, that nothing uses. The package keeps them,
    /// so undo and redo still find them.
    pub(super) fn leave_out_unused_media(&self, pkg: &Package, skip: &mut BTreeSet<String>, replace: &mut HashMap<String, Vec<u8>>) {
        let Some(keep) = &self.baseline_orphans else { return };
        let dead = unused_parts(&effective(pkg, skip, replace), keep);
        if dead.is_empty() {
            return;
        }
        let mut rewritten: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        let mut gone: Vec<String> = Vec::new();
        {
            let parts = effective(pkg, skip, replace);
            let lower: HashMap<String, &String> = dead.iter().map(|d| (d.to_ascii_lowercase(), d)).collect();
            let is_dead = |source: &str, target: &str| {
                let t = package::resolve_target(source, target);
                lower.contains_key(&t.to_ascii_lowercase()) || lower.contains_key(&percent_decode(&t).to_ascii_lowercase())
            };
            for (name, data) in &parts {
                if name.ends_with(".rels") {
                    let source = rels_source(name);
                    if dead.contains(&source) {
                        gone.push(name.to_string());
                        continue;
                    }
                    let Ok(mut doc) = xml::parse(&data.to_cow()) else { continue };
                    let before = doc.root.children.len();
                    doc.root.children.retain(|n| !matches!(n, Node::Element(e) if e.local() == "Relationship" && e.attr("TargetMode") != Some("External") && is_dead(&source, e.attr("Target").unwrap_or_default())));
                    if doc.root.children.len() != before {
                        let empty = doc.root.elements().next().is_none();
                        rewritten.push((name.to_string(), (!empty).then(|| doc.to_bytes())));
                    }
                } else if *name == "[Content_Types].xml" {
                    let Ok(mut doc) = xml::parse(&data.to_cow()) else { continue };
                    let before = doc.root.children.len();
                    doc.root.children.retain(|n| !matches!(n, Node::Element(e) if e.local() == "Override" && e.attr("PartName").is_some_and(|p| lower.contains_key(&p.trim_start_matches('/').to_ascii_lowercase()))));
                    if doc.root.children.len() != before {
                        rewritten.push((name.to_string(), Some(doc.to_bytes())));
                    }
                }
            }
        }
        for (name, data) in rewritten {
            match data {
                Some(d) => {
                    replace.insert(name, d);
                }
                None => {
                    skip.insert(name);
                }
            }
        }
        skip.extend(gone);
        skip.extend(dead);
    }
}
