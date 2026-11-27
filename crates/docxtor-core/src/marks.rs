//! Bookmarks, horizontal lines and watermarks: the parts of a document that are written as
//! zero-width markers, a bordered empty paragraph and a VML shape in the headers. This module holds
//! the pure helpers (reading, building, and rewriting blocks); `Document` applies them as ops.

use crate::model::*;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Bookmarks
// ---------------------------------------------------------------------------

/// Word's limit on a bookmark name.
pub const MAX_NAME: usize = 40;

/// Hidden bookmarks (`_Toc…`, `_Ref…`, `_GoBack`) are made by Word and the table of contents, not by people.
pub fn is_hidden(name: &str) -> bool {
    name.starts_with('_')
}

/// Check a name against Word's rules and return it trimmed. Errors are meant to be shown to people.
pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return Err("Give the bookmark a name.".into()) };
    if name.chars().any(char::is_whitespace) {
        let fixed = name.split_whitespace().collect::<Vec<_>>().join("_");
        return Err(format!("A bookmark name can't have spaces. Try {fixed} instead."));
    }
    if !first.is_alphabetic() {
        return Err("A bookmark name has to start with a letter.".into());
    }
    if let Some(bad) = chars.find(|c| !(c.is_alphanumeric() || *c == '_')) {
        return Err(format!("A bookmark name can only have letters, numbers and underscores, not {bad:?}."));
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("A bookmark name can be at most {MAX_NAME} characters long."));
    }
    Ok(name.to_string())
}

/// A bookmark as the interface lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkInfo {
    pub name: String,
    /// The paragraph where the bookmark starts and the UTF-16 offset of its start.
    pub p: NodeId,
    pub o: usize,
    /// Where it ends (the same position for a point bookmark).
    pub end_p: NodeId,
    pub end_o: usize,
    /// The bookmarked text (the start of the paragraph for a point bookmark), at most about 60 characters.
    pub text: String,
    /// Where it lives when not in the main text: a header, footer or note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story: Option<BookmarkStory>,
}

/// The header, footer or note a bookmark is in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkStory {
    pub kind: crate::stories::StoryKind,
    /// The package part (`word/header1.xml`).
    pub part: String,
    /// For a note, its `w:id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
}

fn scan_el(e: &Element, ids: &mut Vec<i64>, names: &mut Vec<String>) {
    if e.name == "w:bookmarkStart" || e.name == "w:bookmarkEnd" {
        if let Some(id) = e.attr("w:id").and_then(|v| v.parse().ok()) {
            ids.push(id);
        }
        if let (true, Some(n)) = (e.name == "w:bookmarkStart", e.attr("w:name")) {
            names.push(n.to_string());
        }
        return;
    }
    for c in e.elements() {
        scan_el(c, ids, names);
    }
}

fn scan_inlines(inlines: &[Inline], ids: &mut Vec<i64>, names: &mut Vec<String>) {
    for i in inlines {
        match i {
            Inline::Opaque(e) => scan_el(e, ids, names),
            Inline::Container(c) => scan_inlines(&c.children, ids, names),
            Inline::Run(_) => {}
        }
    }
}

fn scan_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>, ids: &mut Vec<i64>, names: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Para(p) => scan_inlines(&p.inlines, ids, names),
            Block::Table(t) => {
                for c in &t.children {
                    match c {
                        TableChild::Other(e) | TableChild::Marker(e, _) => scan_el(e, ids, names),
                        TableChild::Row(r) => {
                            for rc in &r.children {
                                match rc {
                                    RowChild::Other(e) | RowChild::Marker(e, _) => scan_el(e, ids, names),
                                    RowChild::Cell(cell) => scan_blocks(&cell.blocks, ids, names),
                                }
                            }
                        }
                    }
                }
            }
            Block::Container(c) => scan_blocks(&c.blocks, ids, names),
            Block::Opaque(o) => scan_el(&o.el, ids, names),
        }
    }
}

/// Every bookmark id and name used in these block lists, wherever the markers sit.
pub fn ids_and_names<'a, L: IntoIterator<Item = &'a Block> + Copy>(lists: &[L]) -> (Vec<i64>, Vec<String>) {
    let (mut ids, mut names) = (Vec::new(), Vec::new());
    for l in lists {
        scan_blocks(*l, &mut ids, &mut names);
    }
    (ids, names)
}

/// What a scan of the markers and text of the blocks saw, in document order.
#[derive(Debug)]
pub enum Kind {
    Start { id: String, name: String },
    End { id: String },
    Text(String),
}

#[derive(Debug)]
pub struct Ev {
    pub p: NodeId,
    pub o: usize,
    pub kind: Kind,
}

#[derive(Debug, Default)]
pub struct Scan {
    pub events: Vec<Ev>,
    pub para_text: HashMap<NodeId, String>,
}

#[derive(Default)]
struct State {
    /// Block-level starts waiting for the paragraph they belong to.
    pending: Vec<usize>,
    last: Option<(NodeId, usize)>,
}

/// Walk block lists in order (the body first, then stories) and note every bookmark marker and run of text.
pub fn scan<'a, L: IntoIterator<Item = &'a Block> + Copy>(lists: &[L]) -> Scan {
    let mut s = Scan::default();
    for l in lists {
        let mut st = State::default();
        walk_blocks(*l, &mut s, &mut st);
    }
    // Starts at the very end of a list belong to no paragraph.
    s.events.retain(|e| e.p != 0);
    s
}

fn walk_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>, s: &mut Scan, st: &mut State) {
    for b in blocks {
        match b {
            Block::Para(p) => {
                for i in st.pending.drain(..) {
                    s.events[i].p = p.id;
                    s.events[i].o = 0;
                }
                let mut o = 0;
                walk_inlines(&p.inlines, p.id, &mut o, s);
                s.para_text.insert(p.id, p.plain_text());
                st.last = Some((p.id, o));
            }
            Block::Table(t) => {
                for row in t.rows() {
                    for cell in row.cells() {
                        walk_blocks(&cell.blocks, s, st);
                    }
                }
            }
            Block::Container(c) => walk_blocks(&c.blocks, s, st),
            Block::Opaque(o) => match o.el.name.as_str() {
                "w:bookmarkStart" => {
                    if let Some(name) = o.el.attr("w:name") {
                        let id = o.el.attr("w:id").unwrap_or_default().to_string();
                        s.events.push(Ev { p: 0, o: 0, kind: Kind::Start { id, name: name.to_string() } });
                        st.pending.push(s.events.len() - 1);
                    }
                }
                "w:bookmarkEnd" => {
                    if let Some((p, len)) = st.last {
                        s.events.push(Ev { p, o: len, kind: Kind::End { id: o.el.attr("w:id").unwrap_or_default().to_string() } });
                    }
                }
                _ => {}
            },
        }
    }
}

fn walk_inlines(inlines: &[Inline], p: NodeId, o: &mut usize, s: &mut Scan) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                let t = r.text();
                if !t.is_empty() {
                    s.events.push(Ev { p, o: *o, kind: Kind::Text(t) });
                }
                *o += r.len16();
            }
            Inline::Container(c) => walk_inlines(&c.children, p, o, s),
            Inline::Opaque(e) => match e.name.as_str() {
                "w:bookmarkStart" => {
                    if let Some(name) = e.attr("w:name") {
                        let id = e.attr("w:id").unwrap_or_default().to_string();
                        s.events.push(Ev { p, o: *o, kind: Kind::Start { id, name: name.to_string() } });
                    }
                }
                "w:bookmarkEnd" => s.events.push(Ev { p, o: *o, kind: Kind::End { id: e.attr("w:id").unwrap_or_default().to_string() } }),
                // An equation takes one position.
                "m:oMath" | "m:oMathPara" => *o += 1,
                _ => {}
            },
        }
    }
}

/// The visible bookmarks of a scan, in document order.
pub fn list(scan: &Scan) -> Vec<BookmarkInfo> {
    let mut out = Vec::new();
    for (i, ev) in scan.events.iter().enumerate() {
        let Kind::Start { id, name } = &ev.kind else { continue };
        if is_hidden(name) {
            continue;
        }
        let end = scan.events[i + 1..].iter().position(|e| matches!(&e.kind, Kind::End { id: eid } if eid == id)).map(|k| i + 1 + k);
        let (end_p, end_o) = end.map(|k| (scan.events[k].p, scan.events[k].o)).unwrap_or((ev.p, ev.o));
        let mut text = String::new();
        if let Some(k) = end {
            let mut last = ev.p;
            for e in &scan.events[i + 1..k] {
                if let Kind::Text(t) = &e.kind {
                    if e.p != last && !text.is_empty() {
                        text.push(' ');
                    }
                    last = e.p;
                    text.push_str(t);
                }
            }
        }
        let tidy = |s: &str| s.replace(OBJ, " ").split_whitespace().collect::<Vec<_>>().join(" ");
        let mut text = tidy(&text);
        if text.is_empty() {
            text = tidy(scan.para_text.get(&ev.p).map(String::as_str).unwrap_or_default());
        }
        if text.chars().count() > 60 {
            text = text.chars().take(60).collect::<String>().trim_end().to_string() + "…";
        }
        out.push(BookmarkInfo { name: name.clone(), p: ev.p, o: ev.o, end_p, end_o, text, story: None });
    }
    out
}

/// Names of bookmarks that start directly in these inlines or inside links and other wrappers.
pub fn starts_in(inlines: &[Inline], out: &mut Vec<String>) {
    for i in inlines {
        match i {
            Inline::Opaque(e) if e.name == "w:bookmarkStart" => out.extend(e.attr("w:name").map(String::from)),
            Inline::Container(c) => starts_in(&c.children, out),
            _ => {}
        }
    }
}

/// Call `f` on a block list and on every list nested in it (table cells, content controls).
pub fn each_list_mut(blocks: &mut Vec<Block>, f: &mut dyn FnMut(&mut Vec<Block>)) {
    f(blocks);
    for b in blocks.iter_mut() {
        match b {
            Block::Table(t) => {
                for r in t.rows_mut() {
                    for c in r.cells_mut() {
                        each_list_mut(&mut c.blocks, f);
                    }
                }
            }
            Block::Container(c) => each_list_mut(&mut c.blocks, f),
            _ => {}
        }
    }
}

fn runs<'a>(inlines: &'a [Inline], f: &mut dyn FnMut(&'a Run)) {
    for i in inlines {
        match i {
            Inline::Run(r) => f(r),
            Inline::Container(c) => runs(&c.children, f),
            Inline::Opaque(_) => {}
        }
    }
}

fn runs_mut(inlines: &mut [Inline], f: &mut dyn FnMut(&mut Run)) {
    for i in inlines {
        match i {
            Inline::Run(r) => f(r),
            Inline::Container(c) => runs_mut(&mut c.children, f),
            Inline::Opaque(_) => {}
        }
    }
}

/// Remove the markers of bookmarks called `name_lc` (lowercase) whose ends carry `ids`.
pub fn delete_bookmark(blocks: &mut Vec<Block>, name_lc: &str, ids: &HashSet<String>) -> bool {
    fn is_start(e: &Element, name_lc: &str) -> bool {
        e.name == "w:bookmarkStart" && e.attr("w:name").is_some_and(|n| n.to_lowercase() == name_lc)
    }
    fn is_end(e: &Element, ids: &HashSet<String>) -> bool {
        e.name == "w:bookmarkEnd" && e.attr("w:id").is_some_and(|i| ids.contains(i))
    }
    fn inlines(list: &mut Vec<Inline>, name_lc: &str, ids: &HashSet<String>) -> bool {
        let before = list.len();
        list.retain(|i| !matches!(i, Inline::Opaque(e) if is_start(e, name_lc) || is_end(e, ids)));
        let mut changed = before != list.len();
        for i in list.iter_mut() {
            if let Inline::Container(c) = i {
                changed |= inlines(&mut c.children, name_lc, ids);
            }
        }
        changed
    }
    let mut changed = false;
    each_list_mut(blocks, &mut |list| {
        let before = list.len();
        list.retain(|b| !matches!(b, Block::Opaque(o) if is_start(&o.el, name_lc) || is_end(&o.el, ids)));
        changed |= before != list.len();
        for b in list.iter_mut() {
            if let Block::Para(p) = b {
                changed |= inlines(&mut p.inlines, name_lc, ids);
                changed |= unlink(&mut p.inlines, name_lc);
            }
        }
    });
    changed
}

/// Links to these bookmarks (names in lower case), which are gone, become plain text in these blocks. Returns the top-level
/// blocks changed.
/// Whether `unlink_gone` could change this block: it has a link or a field somewhere (read only, so a pass over a long
/// body copies only the few blocks that have one).
pub fn may_link(b: &Block) -> bool {
    fn links(list: &[Inline]) -> bool {
        list.iter().any(|i| matches!(i, Inline::Container(c) if c.kind == InlineKind::Hyperlink || links(&c.children)))
    }
    let mut hit = false;
    walk_paragraphs(std::iter::once(b), &mut |p| hit = hit || links(&p.inlines));
    hit || crate::fields::block_has_field(b)
}

pub fn unlink_gone(blocks: &mut [Block], gone: &[String]) -> Vec<crate::model::NodeId> {
    let mut changed = Vec::new();
    for b in blocks.iter_mut() {
        let mut touched = false;
        crate::model::walk_paragraphs_mut(std::slice::from_mut(b), &mut |p| {
            for name in gone {
                touched |= unlink(&mut p.inlines, name);
            }
        });
        if touched {
            changed.push(b.id());
        }
    }
    changed
}

/// A link to a bookmark that is gone becomes plain text: its words stay, the link and its link look go. Covers
/// links written as `w:hyperlink w:anchor` and as `HYPERLINK \l` fields. (A link must lead somewhere:
/// rule E14.)
fn unlink(list: &mut Vec<Inline>, name_lc: &str) -> bool {
    fn plain(list: &mut [Inline]) {
        for i in list.iter_mut() {
            match i {
                Inline::Run(r) => {
                    if let Some(rpr) = r.rpr.as_mut() {
                        if rpr.child("w:rStyle").and_then(|x| x.attr("w:val")) == Some("Hyperlink") {
                            rpr.remove_children("w:rStyle");
                        }
                    }
                }
                Inline::Container(c) => plain(&mut c.children),
                Inline::Opaque(_) => {}
            }
        }
    }
    let to_here = |instr: &str| {
        let f = crate::fields::Instr::parse(instr);
        f.code == "HYPERLINK" && f.value("l").is_some_and(|v| v.to_lowercase() == name_lc)
    };
    let mut changed = false;
    let mut out = Vec::with_capacity(list.len());
    for i in std::mem::take(list) {
        match i {
            Inline::Container(c) if c.kind == InlineKind::Hyperlink && c.shell.attr("w:anchor").is_some_and(|a| a.to_lowercase() == name_lc) => {
                let mut children = c.children;
                plain(&mut children);
                out.extend(children);
                changed = true;
            }
            // A simple field (`w:fldSimple`) that links here: its result stays as text.
            Inline::Container(c) if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some_and(&to_here) => {
                out.extend(c.children);
                changed = true;
            }
            Inline::Container(mut c) => {
                changed |= unlink(&mut c.children, name_lc);
                out.push(Inline::Container(c));
            }
            other => out.push(other),
        }
    }
    *list = out;
    let len: usize = list.iter().map(|i| i.len16()).sum();
    // What a field link showed keeps its own formatting: the field result's runs can't be told apart from other links
    // in the paragraph once the field is gone.
    changed |= crate::fields::unwrap_fields(list, 0, len, &to_here);
    changed
}

/// Remove every bookmark marker from these blocks (for a copy of a part: bookmark names must stay unique).
pub fn strip_bookmarks(blocks: &mut Vec<Block>) -> bool {
    fn marker(e: &Element) -> bool {
        matches!(e.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd")
    }
    fn inlines(list: &mut Vec<Inline>) -> bool {
        let before = list.len();
        list.retain(|i| !matches!(i, Inline::Opaque(e) if marker(e)));
        let mut changed = before != list.len();
        for i in list.iter_mut() {
            if let Inline::Container(c) = i {
                changed |= inlines(&mut c.children);
            }
        }
        changed
    }
    let mut changed = false;
    each_list_mut(blocks, &mut |list| {
        let before = list.len();
        list.retain(|b| !matches!(b, Block::Opaque(o) if marker(&o.el)));
        changed |= before != list.len();
        for b in list.iter_mut() {
            if let Block::Para(p) = b {
                changed |= inlines(&mut p.inlines);
            }
        }
    });
    changed
}

/// Rename the bookmark `old_lc` (lowercase) to `new` and everything in these blocks that points at it:
/// internal links and `REF`, `PAGEREF`, `NOTEREF` and `HYPERLINK \l` fields.
pub fn rename_bookmark(blocks: &mut Vec<Block>, old_lc: &str, new: &str) -> bool {
    fn inlines(list: &mut [Inline], old_lc: &str, new: &str) -> bool {
        let mut changed = false;
        for i in list.iter_mut() {
            match i {
                Inline::Opaque(e) => match e.name.as_str() {
                    "w:bookmarkStart" if e.attr("w:name").is_some_and(|n| n.to_lowercase() == old_lc) => {
                        e.set_attr("w:name", new);
                        changed = true;
                    }
                    "w:fldSimple" => changed |= fix_simple_field(e, old_lc, new),
                    _ => {}
                },
                Inline::Container(c) => {
                    match c.kind {
                        InlineKind::Hyperlink if c.shell.attr("w:anchor").is_some_and(|a| a.to_lowercase() == old_lc) => {
                            c.shell.set_attr("w:anchor", new);
                            changed = true;
                        }
                        InlineKind::Field => changed |= fix_simple_field(&mut c.shell, old_lc, new),
                        _ => {}
                    }
                    changed |= inlines(&mut c.children, old_lc, new);
                }
                Inline::Run(_) => {}
            }
        }
        changed
    }
    let mut changed = false;
    each_list_mut(blocks, &mut |list| {
        for b in list.iter_mut() {
            match b {
                Block::Opaque(o) if o.el.name == "w:bookmarkStart" && o.el.attr("w:name").is_some_and(|n| n.to_lowercase() == old_lc) => {
                    o.el.set_attr("w:name", new);
                    changed = true;
                }
                Block::Para(p) => {
                    changed |= inlines(&mut p.inlines, old_lc, new);
                    changed |= fix_complex_fields(&mut p.inlines, old_lc, new);
                }
                _ => {}
            }
        }
    });
    changed
}

fn fix_simple_field(e: &mut Element, old_lc: &str, new: &str) -> bool {
    let Some(next) = e.attr("w:instr").and_then(|i| rewrite_instr(i, old_lc, new)) else { return false };
    e.set_attr("w:instr", next);
    true
}

/// Split a field instruction into tokens (byte ranges); a quoted string is one token.
fn tokens(s: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some(&(i, c)) = it.peek() {
        if c.is_whitespace() {
            it.next();
            continue;
        }
        let mut end = s.len();
        if c == '"' {
            it.next();
            let mut closed = false;
            for (j, d) in it.by_ref() {
                if d == '"' {
                    end = j + 1;
                    closed = true;
                    break;
                }
            }
            if !closed {
                end = s.len();
            }
        } else {
            while let Some(&(j, d)) = it.peek() {
                if d.is_whitespace() {
                    end = j;
                    break;
                }
                it.next();
            }
        }
        out.push((i, end));
    }
    out
}

/// The instruction with its target bookmark renamed, if it is a `REF`, `PAGEREF`, `NOTEREF` or `HYPERLINK \l`
/// field that points at `old_lc` (lowercase).
pub fn rewrite_instr(instr: &str, old_lc: &str, new: &str) -> Option<String> {
    let toks = tokens(instr);
    let &(a, b) = toks.first()?;
    let at = match instr[a..b].to_ascii_uppercase().as_str() {
        "REF" | "PAGEREF" | "NOTEREF" => 1,
        "HYPERLINK" => toks.iter().position(|&(s, e)| instr[s..e].eq_ignore_ascii_case("\\l"))? + 1,
        _ => return None,
    };
    let &(s, e) = toks.get(at)?;
    let tok = &instr[s..e];
    let quoted = tok.len() >= 2 && tok.starts_with('"') && tok.ends_with('"');
    let bare = if quoted { &tok[1..tok.len() - 1] } else { tok };
    if bare.to_lowercase() != old_lc {
        return None;
    }
    let target = if quoted { format!("\"{new}\"") } else { new.to_string() };
    Some(format!("{}{}{}", &instr[..s], target, &instr[e..]))
}

/// Complex fields keep their instruction in `w:instrText` runs (possibly several): rewrite the ones that
/// point at `old_lc`.
fn fix_complex_fields(inlines: &mut [Inline], old_lc: &str, new: &str) -> bool {
    enum F {
        Begin,
        Sep,
        End,
        Instr(String),
    }
    let mut seq = Vec::new();
    runs(inlines, &mut |r| {
        for item in &r.items {
            if let RunItem::Hidden(e) = item {
                match e.name.as_str() {
                    "w:fldChar" => match e.attr("w:fldCharType") {
                        Some("begin") => seq.push(F::Begin),
                        Some("separate") => seq.push(F::Sep),
                        Some("end") => seq.push(F::End),
                        _ => {}
                    },
                    "w:instrText" => seq.push(F::Instr(e.text())),
                    _ => {}
                }
            }
        }
    });
    if !seq.iter().any(|f| matches!(f, F::Instr(_))) {
        return false;
    }
    let mut texts: Vec<String> = Vec::new();
    let mut repl: HashMap<usize, String> = HashMap::new();
    let mut stack: Vec<(Vec<usize>, bool)> = Vec::new();
    let mut finish = |atoms: Vec<usize>, texts: &[String]| {
        let joined: String = atoms.iter().map(|a| texts[*a].as_str()).collect();
        if let Some(next) = rewrite_instr(&joined, old_lc, new) {
            for (k, a) in atoms.iter().enumerate() {
                repl.insert(*a, if k == 0 { next.clone() } else { String::new() });
            }
        }
    };
    for f in &seq {
        match f {
            F::Begin => stack.push((Vec::new(), true)),
            F::Sep => {
                if let Some(top) = stack.last_mut() {
                    top.1 = false;
                }
            }
            F::End => {
                if let Some((atoms, _)) = stack.pop() {
                    finish(atoms, &texts);
                }
            }
            F::Instr(s) => {
                texts.push(s.clone());
                let n = texts.len() - 1;
                if let Some(top) = stack.last_mut().filter(|t| t.1) {
                    top.0.push(n);
                }
            }
        }
    }
    while let Some((atoms, _)) = stack.pop() {
        finish(atoms, &texts);
    }
    if repl.is_empty() {
        return false;
    }
    let mut k = 0usize;
    runs_mut(inlines, &mut |r| {
        for item in r.items.iter_mut() {
            if let RunItem::Hidden(e) = item {
                if e.name == "w:instrText" {
                    if let Some(t) = repl.get(&k) {
                        e.children = if t.is_empty() { Vec::new() } else { vec![Node::Text(t.clone())] };
                        if !t.is_empty() && e.attr("xml:space").is_none() {
                            e.set_attr("xml:space", "preserve");
                        }
                    }
                    k += 1;
                }
            }
        }
    });
    true
}

// ---------------------------------------------------------------------------
// Horizontal lines
// ---------------------------------------------------------------------------

/// Paragraph properties of a horizontal line, the way Word draws one: a thin bottom border on an empty
/// paragraph. `sect` carries a section break over when the line takes the place of the paragraph that had it.
pub fn line_ppr(sect: Option<Element>) -> Element {
    let bottom = Element::new("w:bottom").with_attr("w:val", "single").with_attr("w:sz", "6").with_attr("w:space", "1").with_attr("w:color", "auto");
    let mut ppr = Element::new("w:pPr").with_child(Element::new("w:pBdr").with_child(bottom));
    if let Some(s) = sect {
        ppr.children.push(Node::Element(s));
    }
    ppr
}

// ---------------------------------------------------------------------------
// Watermarks
// ---------------------------------------------------------------------------

pub const NS_V: &str = "urn:schemas-microsoft-com:vml";
pub const NS_O: &str = "urn:schemas-microsoft-com:office:office";
pub const NS_W10: &str = "urn:schemas-microsoft-com:office:word";
/// Namespace declarations a header needs for the watermark shape.
pub const NAMESPACES: [(&str, &str); 3] = [("xmlns:v", NS_V), ("xmlns:o", NS_O), ("xmlns:w10", NS_W10)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatermarkLayout {
    /// Rotated 45 degrees up to the right (Word's `rotation:315`).
    #[default]
    Diagonal,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WatermarkKind {
    Text,
    Picture,
}

/// A watermark as found in a header.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Watermark {
    pub kind: WatermarkKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Hex, without `#`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 0 to 1; absent means opaque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    pub layout: WatermarkLayout,
    /// Degrees clockwise, 0 to 360, as the file has it (Word's diagonal is 315).
    pub angle: f64,
    /// The font size in points when the file gives one, else the height of the shape (Word fits the
    /// text to a shape and sets the font size to 1pt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// The shape's size in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// For pictures: the image's part name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    /// For pictures: whether Word's Washout is on (`gain` and `blacklevel` on the image data).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub washout: Option<bool>,
}

const TEXT_PATH_TYPE: &str = "#_x0000_t136";

fn is_watermark_shape(shape: &Element) -> bool {
    let id = shape.attr("id").unwrap_or_default();
    if id.starts_with("PowerPlusWaterMarkObject") || id.starts_with("WordPictureWatermark") {
        return true;
    }
    // Other producers: WordArt text centred on the page and sent behind the text.
    let style = shape.attr("style").unwrap_or_default();
    shape.child("v:textpath").is_some() && style.contains("z-index:-") && style.contains("mso-position-horizontal:center")
}

/// Whether a `w:pict` holds only watermark shapes. These are zero-width markers in the text: they don't
/// show as objects and don't take a position.
pub fn is_watermark_pict(e: &Element) -> bool {
    if e.name != "w:pict" {
        return false;
    }
    let mut shapes = Vec::new();
    e.find_all("v:shape", &mut shapes);
    !shapes.is_empty() && shapes.into_iter().all(is_watermark_shape)
}

/// A run that holds nothing but watermark shapes.
pub fn is_watermark_run(r: &Run) -> bool {
    !r.items.is_empty() && r.items.iter().all(|i| matches!(i, RunItem::Object(e) if is_watermark_pict(e)))
}

/// The first watermark shape in the blocks of a header.
pub fn first_watermark(blocks: &[Block]) -> Option<&Element> {
    let mut found = None;
    walk_paragraphs(blocks, &mut |p| {
        if found.is_none() {
            runs(&p.inlines, &mut |r| {
                for item in &r.items {
                    if let (None, RunItem::Object(e)) = (found, item) {
                        if is_watermark_pict(e) {
                            found = Some(e);
                        }
                    }
                }
            });
        }
    });
    found
}

pub(crate) fn style_prop<'a>(style: &'a str, key: &str) -> Option<&'a str> {
    style.split(';').find_map(|kv| {
        let (k, v) = kv.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim())
    })
}

/// A CSS/VML length in points (`527.85pt`, `4in`, `10cm`; a bare number is points).
pub(crate) fn length_pt(v: &str) -> Option<f64> {
    let v = v.trim();
    let split = v.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(v.len());
    let n: f64 = v[..split].parse().ok()?;
    Some(match v[split..].trim() {
        "" | "pt" => n,
        "in" => n * 72.0,
        "cm" => n * 72.0 / 2.54,
        "mm" => n * 72.0 / 25.4,
        "px" => n * 0.75,
        _ => return None,
    })
}

pub(crate) fn parse_opacity(v: &str) -> Option<f64> {
    let v = v.trim();
    let n = if let Some(p) = v.strip_suffix('%') {
        p.parse::<f64>().ok()? / 100.0
    } else if let Some(f) = v.strip_suffix('f') {
        f.parse::<f64>().ok()? / 65536.0
    } else {
        v.parse::<f64>().ok()?
    };
    Some(n.clamp(0.0, 1.0))
}

/// A VML colour (`silver`, `#c0c0c0`, `silver [3216]`) or a plain hex value as uppercase hex.
pub fn colour_hex(v: &str) -> Option<String> {
    let v = v.split('[').next().unwrap_or(v).trim();
    let hex = v.trim_start_matches('#');
    if (hex.len() == 6 || hex.len() == 3) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let full = if hex.len() == 3 { hex.chars().flat_map(|c| [c, c]).collect::<String>() } else { hex.to_string() };
        return Some(full.to_ascii_uppercase());
    }
    let named = match v.to_ascii_lowercase().as_str() {
        "black" => "000000",
        "silver" => "C0C0C0",
        "gray" | "grey" => "808080",
        "lightgray" | "lightgrey" => "D3D3D3",
        "darkgray" | "darkgrey" => "A9A9A9",
        "white" => "FFFFFF",
        "maroon" => "800000",
        "red" => "FF0000",
        "purple" => "800080",
        "fuchsia" | "magenta" => "FF00FF",
        "green" => "008000",
        "lime" => "00FF00",
        "olive" => "808000",
        "yellow" => "FFFF00",
        "navy" => "000080",
        "blue" => "0000FF",
        "teal" => "008080",
        "aqua" | "cyan" => "00FFFF",
        "orange" => "FFA500",
        "brown" => "A52A2A",
        "pink" => "FFC0CB",
        "gold" => "FFD700",
        _ => return None,
    };
    Some(named.to_string())
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches(|c| c == '"' || c == '\'').to_string()
}

/// Read a watermark from the `w:pict` of a header. `media_of` turns a relationship id into a part name.
pub fn read_watermark(pict: &Element, media_of: &dyn Fn(&str) -> Option<String>) -> Option<Watermark> {
    let mut shapes = Vec::new();
    pict.find_all("v:shape", &mut shapes);
    let shape = shapes.into_iter().find(|s| is_watermark_shape(s))?;
    let style = shape.attr("style").unwrap_or_default();
    let width = style_prop(style, "width").and_then(length_pt);
    let height = style_prop(style, "height").and_then(length_pt);
    let rotation = style_prop(style, "rotation").and_then(|v| v.trim().trim_end_matches("deg").parse::<f64>().ok()).unwrap_or(0.0);
    let angle = ((rotation % 360.0) + 360.0) % 360.0;
    let layout = if !(0.5..=359.5).contains(&angle) { WatermarkLayout::Horizontal } else { WatermarkLayout::Diagonal };
    let opacity = shape.child("v:fill").and_then(|f| f.attr("opacity")).and_then(parse_opacity);
    let color = shape.attr("fillcolor").and_then(colour_hex);
    let mut mark = Watermark { kind: WatermarkKind::Text, text: None, font: None, color, opacity, layout, angle, size: height, width, height, media: None, washout: None };
    if let Some(tp) = shape.child("v:textpath") {
        let tps = tp.attr("style").unwrap_or_default();
        mark.text = tp.attr("string").map(String::from);
        mark.font = style_prop(tps, "font-family").map(unquote).filter(|f| !f.is_empty());
        if let Some(fs) = style_prop(tps, "font-size").and_then(length_pt).filter(|f| (f - 1.0).abs() > 0.01) {
            mark.size = Some(fs);
        }
    } else {
        mark.kind = WatermarkKind::Picture;
        let data = shape.child("v:imagedata");
        mark.media = data.and_then(|i| i.attr("r:id")).and_then(media_of);
        mark.washout = Some(data.is_some_and(|i| i.attr("gain").is_some() || i.attr("blacklevel").is_some()));
    }
    Some(mark)
}

/// Take every watermark shape out of the blocks. Returns whether anything changed and the first
/// paragraph that held one (so a replacement goes where the old one was).
pub fn remove_watermarks(blocks: &mut Vec<Block>) -> (bool, Option<NodeId>) {
    fn strip(inlines: &mut Vec<Inline>) -> bool {
        let mut changed = false;
        inlines.retain_mut(|i| match i {
            Inline::Run(r) => {
                let n = r.items.len();
                r.items.retain(|it| !matches!(it, RunItem::Object(e) if is_watermark_pict(e)));
                if r.items.len() != n {
                    changed = true;
                    !r.items.is_empty()
                } else {
                    true
                }
            }
            Inline::Container(c) => {
                changed |= strip(&mut c.children);
                true
            }
            Inline::Opaque(_) => true,
        });
        changed
    }
    let mut changed = false;
    let mut first = None;
    each_list_mut(blocks, &mut |list| {
        for b in list.iter_mut() {
            if let Block::Para(p) = b {
                if strip(&mut p.inlines) {
                    changed = true;
                    first.get_or_insert(p.id);
                }
            }
        }
    });
    (changed, first)
}

/// Put a watermark run at the start of paragraph `near`, else of the first paragraph of the header,
/// else in a new paragraph (with `style`).
pub fn put_watermark(blocks: &mut Vec<Block>, near: Option<NodeId>, run: Run, new_id: NodeId, style: &str) {
    if let Some(p) = near.and_then(|id| find_para_mut(blocks, id)) {
        p.inlines.insert(0, Inline::Run(run));
        return;
    }
    if let Some(Block::Para(p)) = blocks.iter_mut().find(|b| matches!(b, Block::Para(_))) {
        p.inlines.insert(0, Inline::Run(run));
        return;
    }
    let mut p = Paragraph::new(new_id);
    p.ppr = Some(Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style)));
    p.inlines.push(Inline::Run(run));
    blocks.insert(0, Block::Para(p));
}

/// A run holding a watermark shape, as Word writes it.
pub fn watermark_run(pict: Element) -> Run {
    Run { t_attrs: None, attrs: vec![], rpr: Some(Element::new("w:rPr").with_child(Element::new("w:noProof"))), items: vec![RunItem::Object(pict)] }
}

/// The shape's width and height in points for `text` on a text area of `area` points (width, height).
///
/// Word fits WordArt text to its shape, so the shape's aspect ratio is the text's: about 0.8 em of width
/// per capital letter per 1 em of height ("DRAFT" is 4 to 1, as Word makes it), narrower for lower case. A
/// diagonal watermark is as long as the turned text can be and still clear the smaller side of the area.
pub fn watermark_size(text: &str, layout: WatermarkLayout, area: (f64, f64)) -> (f64, f64) {
    let n = text.chars().count().max(1) as f64;
    let aspect = if text.chars().any(char::is_lowercase) { 0.56 * n } else { 0.8 * n }.max(1.0);
    let (w, h) = match layout {
        WatermarkLayout::Diagonal => {
            let w = 0.997 * std::f64::consts::SQRT_2 * area.0.min(area.1) / (1.0 + 1.0 / aspect);
            (w, w / aspect)
        }
        WatermarkLayout::Horizontal => {
            let (mut w, mut h) = (area.0, area.0 / aspect);
            if h > area.1 {
                h = area.1;
                w = h * aspect;
            }
            (w, h)
        }
    };
    ((w * 100.0).round() / 100.0, (h * 100.0).round() / 100.0)
}

/// Points as Word writes them: two decimals at most, none when whole.
fn pt(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `0.5` as Word writes an opacity: `.5`.
fn opacity_attr(o: f64) -> String {
    let s = pt(o.clamp(0.0, 1.0));
    s.strip_prefix('0').filter(|r| !r.is_empty()).map(String::from).unwrap_or(s)
}

pub struct TextMark<'a> {
    pub text: &'a str,
    pub font: &'a str,
    /// Uppercase hex.
    pub color: &'a str,
    pub opacity: f64,
    pub layout: WatermarkLayout,
    /// Width and height in points.
    pub size: (f64, f64),
    /// Distinguishes the shapes of different headers (their ids).
    pub serial: u32,
}

/// The `w:pict` of a text watermark: Word's WordArt shape type (`_x0000_t136`) and a shape that uses it.
pub fn text_pict(m: &TextMark) -> Element {
    let f = |eqn: &str| Element::new("v:f").with_attr("eqn", eqn);
    let formulas = [
        "sum #0 0 10800", "prod #0 2 1", "sum 21600 0 @1", "sum 0 0 @2", "sum 21600 0 @3", "if @0 @3 0", "if @0 21600 @1",
        "if @0 0 @2", "if @0 @4 21600", "mid @5 @6", "mid @8 @5", "mid @7 @8", "mid @6 @7", "sum @6 0 @5",
    ]
    .iter()
    .fold(Element::new("v:formulas"), |el, eqn| el.with_child(f(eqn)));
    let shapetype = Element::new("v:shapetype")
        .with_attr("id", "_x0000_t136")
        .with_attr("coordsize", "21600,21600")
        .with_attr("o:spt", "136")
        .with_attr("adj", "10800")
        .with_attr("path", "m@7,l@8,m@5,21600l@6,21600e")
        .with_child(formulas)
        .with_child(
            Element::new("v:path")
                .with_attr("textpathok", "t")
                .with_attr("o:connecttype", "custom")
                .with_attr("o:connectlocs", "@9,0;@10,10800;@11,21600;@12,10800")
                .with_attr("o:connectangles", "270,180,90,0"),
        )
        .with_child(Element::new("v:textpath").with_attr("on", "t").with_attr("fitshape", "t"))
        .with_child(Element::new("v:handles").with_child(Element::new("v:h").with_attr("position", "#0,bottomRight").with_attr("xrange", "6629,14971")))
        .with_child(Element::new("o:lock").with_attr("v:ext", "edit").with_attr("text", "t").with_attr("shapetype", "t"));
    let rotation = if m.layout == WatermarkLayout::Diagonal { "rotation:315;" } else { "" };
    let style = format!(
        "position:absolute;margin-left:0;margin-top:0;width:{}pt;height:{}pt;{rotation}z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin",
        pt(m.size.0),
        pt(m.size.1)
    );
    let fill = if m.color == "C0C0C0" { "silver".to_string() } else { format!("#{}", m.color.to_ascii_lowercase()) };
    let mut shape = Element::new("v:shape")
        .with_attr("id", format!("PowerPlusWaterMarkObject{}", 357476642 + m.serial))
        .with_attr("o:spid", format!("_x0000_s{}", 2049 + m.serial))
        .with_attr("type", TEXT_PATH_TYPE)
        .with_attr("style", style)
        .with_attr("o:allowincell", "f")
        .with_attr("fillcolor", fill)
        .with_attr("stroked", "f");
    if m.opacity < 1.0 {
        shape = shape.with_child(Element::new("v:fill").with_attr("opacity", opacity_attr(m.opacity)));
    }
    shape = shape
        .with_child(Element::new("v:textpath").with_attr("style", format!("font-family:\"{}\";font-size:1pt", m.font)).with_attr("string", m.text))
        .with_child(Element::new("w10:wrap").with_attr("anchorx", "margin").with_attr("anchory", "margin"));
    Element::new("w:pict").with_child(shapetype).with_child(shape)
}

pub struct PictureMark<'a> {
    /// The relationship id of the image in the header part.
    pub rid: &'a str,
    /// The image's file name, shown as its title.
    pub title: &'a str,
    /// Width and height in points.
    pub size: (f64, f64),
    pub washout: bool,
    /// Distinguishes the shapes of different headers (their ids).
    pub serial: u32,
}

/// The `w:pict` of a picture watermark: Word's picture shape type (`_x0000_t75`) and a shape that uses it,
/// centred on the margins and sent behind the text.
pub fn picture_pict(m: &PictureMark) -> Element {
    let f = |eqn: &str| Element::new("v:f").with_attr("eqn", eqn);
    let formulas = [
        "if lineDrawn pixelLineWidth 0", "sum @0 1 0", "sum 0 0 @1", "prod @2 1 2", "prod @3 21600 pixelWidth", "prod @3 21600 pixelHeight",
        "sum @0 0 1", "prod @6 1 2", "prod @7 21600 pixelWidth", "sum @8 21600 0", "prod @7 21600 pixelHeight", "sum @10 21600 0",
    ]
    .iter()
    .fold(Element::new("v:formulas"), |el, eqn| el.with_child(f(eqn)));
    let shapetype = Element::new("v:shapetype")
        .with_attr("id", "_x0000_t75")
        .with_attr("coordsize", "21600,21600")
        .with_attr("o:spt", "75")
        .with_attr("o:preferrelative", "t")
        .with_attr("path", "m@4@5l@4@11@9@11@9@5xe")
        .with_attr("filled", "f")
        .with_attr("stroked", "f")
        .with_child(Element::new("v:stroke").with_attr("joinstyle", "miter"))
        .with_child(formulas)
        .with_child(Element::new("v:path").with_attr("o:extrusionok", "f").with_attr("gradientshapeok", "t").with_attr("o:connecttype", "rect"))
        .with_child(Element::new("o:lock").with_attr("v:ext", "edit").with_attr("aspectratio", "t"));
    let style = format!(
        "position:absolute;margin-left:0;margin-top:0;width:{}pt;height:{}pt;z-index:-251657216;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin",
        pt(m.size.0),
        pt(m.size.1)
    );
    let mut data = Element::new("v:imagedata").with_attr("r:id", m.rid).with_attr("o:title", m.title);
    if m.washout {
        data = data.with_attr("gain", "19661f").with_attr("blacklevel", "22938f");
    }
    let shape = Element::new("v:shape")
        .with_attr("id", format!("WordPictureWatermark{}", 357476642 + m.serial))
        .with_attr("o:spid", format!("_x0000_s{}", 2049 + m.serial))
        .with_attr("type", "#_x0000_t75")
        .with_attr("style", style)
        .with_attr("o:allowincell", "f")
        .with_child(data)
        .with_child(Element::new("w10:wrap").with_attr("anchorx", "margin").with_attr("anchory", "margin"));
    Element::new("w:pict").with_child(shapetype).with_child(shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_words_rules() {
        assert_eq!(validate_name(" Intro_1 ").unwrap(), "Intro_1");
        assert!(validate_name("").is_err());
        assert!(validate_name("1st").unwrap_err().contains("start with a letter"));
        assert!(validate_name("_hidden").is_err());
        assert!(validate_name("two words").unwrap_err().contains("two_words"));
        assert!(validate_name("a-b").is_err());
        assert!(validate_name(&"a".repeat(40)).is_ok());
        assert!(validate_name(&"a".repeat(41)).is_err());
        assert!(validate_name("Überblick").is_ok());
    }

    #[test]
    fn instructions_follow_renames() {
        assert_eq!(rewrite_instr(" REF Intro \\h ", "intro", "Start").as_deref(), Some(" REF Start \\h "));
        assert_eq!(rewrite_instr("PAGEREF \"Intro\" \\h", "intro", "Start").as_deref(), Some("PAGEREF \"Start\" \\h"));
        assert_eq!(rewrite_instr(" HYPERLINK \\l \"Intro\" ", "intro", "Start").as_deref(), Some(" HYPERLINK \\l \"Start\" "));
        assert_eq!(rewrite_instr(" REF Other \\h ", "intro", "Start"), None);
        assert_eq!(rewrite_instr(" TOC \\o \"1-3\" ", "intro", "Start"), None);
    }

    #[test]
    fn colours_and_lengths() {
        assert_eq!(colour_hex("silver").as_deref(), Some("C0C0C0"));
        assert_eq!(colour_hex("#c00000").as_deref(), Some("C00000"));
        assert_eq!(colour_hex("silver [3216]").as_deref(), Some("C0C0C0"));
        assert_eq!(colour_hex("f0c").as_deref(), Some("FF00CC"));
        assert_eq!(colour_hex("nonsense"), None);
        assert_eq!(length_pt("527.85pt"), Some(527.85));
        assert_eq!(length_pt("1in"), Some(72.0));
        assert_eq!(parse_opacity(".5"), Some(0.5));
        assert_eq!(parse_opacity("32768f"), Some(0.5));
        assert_eq!(parse_opacity("25%"), Some(0.25));
        assert_eq!(opacity_attr(0.5), ".5");
        assert_eq!(opacity_attr(0.25), ".25");
    }

    #[test]
    fn draft_is_four_to_one_like_words() {
        let (w, h) = watermark_size("DRAFT", WatermarkLayout::Diagonal, (468.0, 648.0));
        assert!((w - 527.85).abs() < 2.0, "{w}");
        assert!((w / h - 4.0).abs() < 0.01);
    }
}
