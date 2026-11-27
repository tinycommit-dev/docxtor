//! Inspect document: what a file carries that the page doesn't show (names, comments, tracked changes, hidden
//! text, header and footer text, custom data), and removing it, like Word's Document Inspector.
//!
//! Removing is one op, `remove_hidden_data`, so it is undoable and the AI tools, the CLI and the MCP server get
//! it too. Parts that go (the comments, custom XML, the preview picture) stay in the package, so undo can bring them
//! back: the saved file leaves them out, and the properties that were blanked are rewritten, through `Scrub`.

use super::*;
use crate::package::{rels_path, resolve_target};
use std::collections::BTreeMap;

/// The name that replaces the author of a comment or a tracked change.
const ANONYMOUS: &str = "Author";

/// The categories, in the order they are listed and removed.
pub const CATEGORIES: &[&str] = &["properties", "authors", "comments", "revisions", "hiddenText", "headersFooters", "customXml", "embedded"];

/// What the saved file leaves out and rewrites after Remove (kept in the undo history).
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Scrub {
    removed: BTreeSet<String>,
    replaced: BTreeMap<String, Vec<u8>>,
}

/// The part a relationships file belongs to (`word/_rels/document.xml.rels` is for `word/document.xml`).
fn rels_owner(rels: &str) -> String {
    match rels.strip_suffix(".rels").unwrap_or(rels).rsplit_once("/_rels/") {
        Some((dir, file)) => format!("{dir}/{file}"),
        None => String::new(),
    }
}

impl Scrub {
    pub(super) fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.replaced.is_empty()
    }

    /// Take the parts out of a package (and what points at them) and rewrite the ones that were blanked.
    pub(super) fn apply(&self, pkg: &mut Package) {
        for part in &self.removed {
            let owners: Vec<String> = pkg.parts.iter().filter(|p| p.name.ends_with(".rels")).map(|p| rels_owner(&p.name)).collect();
            for owner in owners {
                pkg.remove_rels_to(&owner, part);
            }
            pkg.remove(&rels_path(part));
            pkg.remove(part);
            pkg.remove_override(part);
        }
        for (name, data) in &self.replaced {
            if pkg.has(name) {
                pkg.set(name, data.clone());
            }
        }
    }
}

/// What was found in one category.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// `properties`, `authors`, `comments`, `revisions`, `hiddenText`, `headersFooters`, `customXml` or `embedded`.
    pub id: &'static str,
    /// How many things there are (the items of `properties`, the comments, the hidden runs...). 0 means none.
    pub count: usize,
    /// What they are, where that is worth showing: names, the properties set.
    pub detail: Vec<String>,
    /// Whether Remove can take them out. Embedded objects are only reported.
    pub removable: bool,
}

// ---------------------------------------------------------------------------
// Walking the content
// ---------------------------------------------------------------------------

/// Every element in blocks: paragraph and run properties, wrappers, opaque markup and what is inside them.
fn visit_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(&Element)) {
    fn el(e: &Element, f: &mut dyn FnMut(&Element)) {
        f(e);
        for c in e.elements() {
            el(c, f);
        }
    }
    fn inlines(list: &[Inline], f: &mut dyn FnMut(&Element)) {
        for i in list {
            match i {
                Inline::Run(r) => {
                    if let Some(p) = &r.rpr {
                        el(p, f);
                    }
                    for it in &r.items {
                        if let RunItem::Break(e) | RunItem::Object(e) | RunItem::Hidden(e) = it {
                            el(e, f);
                        }
                    }
                }
                Inline::Container(c) => {
                    el(&c.shell, f);
                    inlines(&c.children, f);
                }
                Inline::Opaque(e) => el(e, f),
            }
        }
    }
    for b in blocks {
        match b {
            Block::Para(p) => {
                if let Some(pp) = &p.ppr {
                    el(pp, f);
                }
                inlines(&p.inlines, f);
            }
            Block::Table(t) => {
                for c in &t.children {
                    match c {
                        TableChild::Other(e) | TableChild::Marker(e, _) => el(e, f),
                        TableChild::Row(r) => {
                            for rc in &r.children {
                                match rc {
                                    RowChild::Other(e) | RowChild::Marker(e, _) => el(e, f),
                                    RowChild::Cell(c) => {
                                        if let Some(pr) = &c.tcpr {
                                            el(pr, f);
                                        }
                                        visit_blocks(&c.blocks, f);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Block::Container(c) => {
                el(&c.shell, f);
                visit_blocks(&c.blocks, f);
            }
            Block::Opaque(o) => el(&o.el, f),
        }
    }
}

/// Like `visit_blocks`, changing the elements. Returns whether `f` said anything changed.
fn visit_blocks_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Element) -> bool) -> bool {
    fn el(e: &mut Element, f: &mut dyn FnMut(&mut Element) -> bool) -> bool {
        let mut changed = f(e);
        for c in e.elements_mut() {
            changed |= el(c, f);
        }
        changed
    }
    fn inlines(list: &mut [Inline], f: &mut dyn FnMut(&mut Element) -> bool) -> bool {
        let mut changed = false;
        for i in list {
            match i {
                Inline::Run(r) => {
                    if let Some(p) = &mut r.rpr {
                        changed |= el(p, f);
                    }
                    for it in &mut r.items {
                        if let RunItem::Break(e) | RunItem::Object(e) | RunItem::Hidden(e) = it {
                            changed |= el(e, f);
                        }
                    }
                }
                Inline::Container(c) => {
                    changed |= el(&mut c.shell, f);
                    changed |= inlines(&mut c.children, f);
                }
                Inline::Opaque(e) => changed |= el(e, f),
            }
        }
        changed
    }
    let mut changed = false;
    for b in blocks {
        match b {
            Block::Para(p) => {
                if let Some(pp) = &mut p.ppr {
                    changed |= el(pp, f);
                }
                changed |= inlines(&mut p.inlines, f);
            }
            Block::Table(t) => {
                for c in &mut t.children {
                    match c {
                        TableChild::Other(e) | TableChild::Marker(e, _) => changed |= el(e, f),
                        TableChild::Row(r) => {
                            for rc in &mut r.children {
                                match rc {
                                    RowChild::Other(e) | RowChild::Marker(e, _) => changed |= el(e, f),
                                    RowChild::Cell(c) => {
                                        if let Some(pr) = &mut c.tcpr {
                                            changed |= el(pr, f);
                                        }
                                        changed |= visit_blocks_mut(&mut c.blocks, f);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Block::Container(c) => {
                changed |= el(&mut c.shell, f);
                changed |= visit_blocks_mut(&mut c.blocks, f);
            }
            Block::Opaque(o) => changed |= el(&mut o.el, f),
        }
    }
    changed
}

/// The names of tracked changes: wrappers and property changes.
fn is_revision(name: &str) -> bool {
    matches!(
        name,
        "w:ins" | "w:del" | "w:moveFrom" | "w:moveTo" | "w:rPrChange" | "w:pPrChange" | "w:sectPrChange" | "w:tblPrChange" | "w:trPrChange" | "w:tcPrChange" | "w:tblGridChange" | "w:numberingChange" | "w:cellIns" | "w:cellDel" | "w:cellMerge"
    )
}

fn hidden_run(r: &Run) -> bool {
    r.rpr.as_ref().and_then(|p| p.child("w:vanish")).is_some_and(|v| !matches!(v.attr("w:val"), Some("0") | Some("false") | Some("off")))
        && !r.items.is_empty()
        && r.items.iter().all(|it| matches!(it, RunItem::Text(_) | RunItem::Tab | RunItem::Break(_)))
}

fn count_hidden(inlines: &[Inline]) -> usize {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Run(r) => usize::from(hidden_run(r)),
            Inline::Container(c) => count_hidden(&c.children),
            Inline::Opaque(_) => 0,
        })
        .sum()
}

fn remove_hidden(inlines: &mut Vec<Inline>) -> usize {
    let mut n = 0;
    inlines.retain_mut(|i| match i {
        Inline::Run(r) => {
            let hide = hidden_run(r);
            n += usize::from(hide);
            !hide
        }
        Inline::Container(c) => {
            n += remove_hidden(&mut c.children);
            true
        }
        Inline::Opaque(_) => true,
    });
    n
}

fn is_comment_marker(name: &str) -> bool {
    matches!(name, "w:commentRangeStart" | "w:commentRangeEnd")
}

/// Take comment ranges and references out of some inline content.
fn remove_comment_marks(inlines: &mut Vec<Inline>) -> bool {
    let mut changed = false;
    inlines.retain_mut(|i| match i {
        Inline::Opaque(e) if is_comment_marker(&e.name) => {
            changed = true;
            false
        }
        Inline::Run(r) => {
            let before = r.items.len();
            r.items.retain(|it| !matches!(it, RunItem::Object(e) | RunItem::Hidden(e) if e.name == "w:commentReference"));
            if r.items.len() != before {
                changed = true;
                // A run that held only the reference goes with it.
                return !r.items.is_empty();
            }
            true
        }
        Inline::Container(c) => {
            changed |= remove_comment_marks(&mut c.children);
            true
        }
        Inline::Opaque(_) => true,
    });
    changed
}

fn comments_in_blocks(blocks: &mut Vec<Block>) -> bool {
    let mut changed = false;
    blocks.retain_mut(|b| match b {
        Block::Opaque(o) if is_comment_marker(&o.el.name) => {
            changed = true;
            false
        }
        Block::Para(p) => {
            changed |= remove_comment_marks(&mut p.inlines);
            true
        }
        Block::Table(t) => {
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    changed |= comments_in_blocks(&mut c.blocks);
                }
            }
            true
        }
        Block::Container(c) => {
            changed |= comments_in_blocks(&mut c.blocks);
            true
        }
        Block::Opaque(_) => true,
    });
    changed
}

fn hidden_in_blocks(blocks: &mut [Block]) -> usize {
    let mut n = 0;
    for b in blocks {
        match b {
            Block::Para(p) => n += remove_hidden(&mut p.inlines),
            Block::Table(t) => {
                for r in t.rows_mut() {
                    for c in r.cells_mut() {
                        n += hidden_in_blocks(&mut c.blocks);
                    }
                }
            }
            Block::Container(c) => n += hidden_in_blocks(&mut c.blocks),
            Block::Opaque(_) => {}
        }
    }
    n
}

// ---------------------------------------------------------------------------
// Accepting tracked changes
// ---------------------------------------------------------------------------

fn accept_inlines(inlines: &mut Vec<Inline>) -> bool {
    let mut changed = false;
    let old = std::mem::take(inlines);
    for i in old {
        match i {
            Inline::Container(mut c) if c.kind == InlineKind::Insertion => {
                changed = true;
                accept_inlines(&mut c.children);
                inlines.extend(c.children);
            }
            Inline::Container(mut c) => {
                changed |= accept_inlines(&mut c.children);
                inlines.push(Inline::Container(c));
            }
            Inline::Opaque(e) if matches!(e.name.as_str(), "w:del" | "w:moveFrom" | "w:moveFromRangeStart" | "w:moveFromRangeEnd" | "w:moveToRangeStart" | "w:moveToRangeEnd") => changed = true,
            Inline::Run(mut r) => {
                if let Some(p) = &mut r.rpr {
                    changed |= p.remove_children("w:rPrChange") > 0;
                    if p.children.is_empty() {
                        r.rpr = None;
                    }
                }
                inlines.push(Inline::Run(r));
            }
            other => inlines.push(other),
        }
    }
    changed
}

/// Whether a paragraph's mark was deleted by a tracked change.
fn mark_deleted(p: &Paragraph) -> bool {
    p.ppr.as_ref().and_then(|pp| pp.child("w:rPr")).is_some_and(|r| r.child("w:del").is_some() || r.child("w:moveFrom").is_some())
}

fn accept_blocks(blocks: &mut Vec<Block>) -> bool {
    let mut changed = false;
    for b in blocks.iter_mut() {
        match b {
            Block::Para(p) => {
                changed |= accept_inlines(&mut p.inlines);
                if let Some(pp) = &mut p.ppr {
                    changed |= pp.remove_children("w:pPrChange") > 0;
                    if let Some(rpr) = pp.child_mut("w:rPr") {
                        for n in ["w:ins", "w:moveTo", "w:rPrChange"] {
                            changed |= rpr.remove_children(n) > 0;
                        }
                    }
                    if let Some(s) = pp.child_mut("w:sectPr") {
                        changed |= s.remove_children("w:sectPrChange") > 0;
                    }
                }
            }
            Block::Table(t) => {
                // Rows a tracked change deleted go; the others lose their marks.
                t.children.retain(|c| {
                    let TableChild::Row(r) = c else { return true };
                    let gone = r.children.iter().any(|rc| matches!(rc, RowChild::Other(e) if e.name == "w:trPr" && e.child("w:del").is_some()));
                    changed |= gone;
                    !gone
                });
                for c in &mut t.children {
                    match c {
                        TableChild::Other(e) => {
                            for n in ["w:tblPrChange", "w:tblGridChange"] {
                                changed |= e.remove_children(n) > 0;
                            }
                        }
                        TableChild::Marker(..) => {}
                        TableChild::Row(r) => {
                            for rc in &mut r.children {
                                match rc {
                                    RowChild::Other(e) if e.name == "w:trPr" => {
                                        for n in ["w:ins", "w:trPrChange"] {
                                            changed |= e.remove_children(n) > 0;
                                        }
                                    }
                                    RowChild::Other(e) if e.name == "w:tblPrEx" => changed |= e.remove_children("w:tblPrExChange") > 0,
                                    RowChild::Other(_) | RowChild::Marker(..) => {}
                                    RowChild::Cell(c) => {
                                        if let Some(pr) = &mut c.tcpr {
                                            for n in ["w:tcPrChange", "w:cellIns", "w:cellDel", "w:cellMerge"] {
                                                changed |= pr.remove_children(n) > 0;
                                            }
                                        }
                                        changed |= accept_blocks(&mut c.blocks);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Block::Container(c) => changed |= accept_blocks(&mut c.blocks),
            Block::Opaque(o) => {
                if o.el.name == "w:sectPr" {
                    changed |= o.el.remove_children("w:sectPrChange") > 0;
                }
            }
        }
    }
    // A deleted paragraph mark joins the paragraph to the one after it, which keeps its own properties.
    let mut i = 0;
    while i < blocks.len() {
        let joins = matches!(&blocks[i], Block::Para(p) if mark_deleted(p)) && matches!(blocks.get(i + 1), Some(Block::Para(_)));
        if joins {
            changed = true;
            let Block::Para(first) = blocks.remove(i) else { unreachable!() };
            if let Block::Para(next) = &mut blocks[i] {
                let mut inlines = first.inlines;
                inlines.append(&mut next.inlines);
                next.inlines = inlines;
            }
            continue;
        }
        if let Block::Para(p) = &mut blocks[i] {
            if mark_deleted(p) {
                // Nothing after it to join: the mark stays.
                if let Some(rpr) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:rPr")) {
                    changed |= rpr.remove_children("w:del") > 0;
                    changed |= rpr.remove_children("w:moveFrom") > 0;
                }
            }
        }
        i += 1;
    }
    changed
}

// ---------------------------------------------------------------------------
// The document
// ---------------------------------------------------------------------------

/// The text of an element's `child` element, trimmed.
fn text_of(root: &Element, child: &str) -> Option<String> {
    root.child(child).map(|e| e.text().trim().to_string()).filter(|t| !t.is_empty())
}

impl Document {
    /// The data of a part as the saved file will have it.
    fn part_now(&self, name: &str) -> Option<Vec<u8>> {
        if self.scrub.removed.contains(name) {
            return None;
        }
        self.scrub.replaced.get(name).cloned().or_else(|| self.pkg.get(name).map(|d| d.to_vec()))
    }

    /// The parts of a relationship type the root or the main part points at (absolute names), as the saved file will have them.
    fn related(&self, from: &str, suffix: &str) -> Vec<String> {
        let rels = self.pkg.rels_for(from).unwrap_or_default();
        rels.into_iter()
            .filter(|r| !r.external && (r.rel_type == suffix || r.rel_type.ends_with(&format!("/{suffix}"))))
            .map(|r| resolve_target(from, &r.target))
            .filter(|t| self.part_now(t).is_some())
            .collect()
    }

    /// Comments parts: the comments, their extensions, and the list of people.
    fn comment_parts(&self) -> Vec<String> {
        let mut out = Vec::new();
        for kind in ["comments", "commentsExtended", "commentsIds", "commentsExtensible", "people"] {
            out.extend(self.related(&self.main_path, kind));
        }
        out
    }

    /// Custom XML parts, not counting the bibliography's list of sources (Docxtor and Word keep citations there) or the list of tabs.
    fn custom_xml_parts(&self) -> Vec<String> {
        self.related(&self.main_path, "customXml")
            .into_iter()
            .filter(|p| self.part_now(p).and_then(|d| xml::parse(&d).ok()).is_none_or(|d| !matches!(d.root.local(), "Sources" | "tabs")))
            .collect()
    }

    fn each_story_blocks(&self) -> Vec<crate::body::BlocksRef<'_>> {
        use crate::body::BlocksRef;
        let mut v: Vec<BlocksRef> = vec![BlocksRef::Top(self.main_body())];
        // A header nobody refers to any more (removed, or let go) is not in the saved file, so it is not counted.
        let used = self.referenced_hf_parts();
        v.extend(self.stories.iter().filter(|s| !self.hf_candidates.contains(&s.part) || used.contains(&s.part)).map(|s| BlocksRef::List(&s.blocks)));
        v
    }

    /// The names of the people behind comments and tracked changes.
    fn author_names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for blocks in self.each_story_blocks() {
            visit_blocks(blocks, &mut |e| {
                if is_revision(&e.name) {
                    if let Some(a) = e.attr("w:author").filter(|a| !a.is_empty() && *a != ANONYMOUS) {
                        names.insert(a.to_string());
                    }
                }
            });
        }
        for part in self.comment_parts() {
            let Some(doc) = self.part_now(&part).and_then(|d| xml::parse(&d).ok()) else { continue };
            fn go(e: &Element, names: &mut BTreeSet<String>) {
                if e.local() == "comment" || e.local() == "person" {
                    for a in ["w:author", "w15:author"] {
                        if let Some(n) = e.attr(a).filter(|n| !n.is_empty() && *n != ANONYMOUS) {
                            names.insert(n.to_string());
                        }
                    }
                }
                for c in e.elements() {
                    go(c, names);
                }
            }
            go(&doc.root, &mut names);
        }
        names.into_iter().collect()
    }

    /// Number of comments in the comments part.
    fn comment_count(&self) -> usize {
        self.related(&self.main_path, "comments")
            .iter()
            .filter_map(|p| self.part_now(p).and_then(|d| xml::parse(&d).ok()))
            .map(|d| d.root.children_named("w:comment").count())
            .sum()
    }

    /// Tracked changes in the body and every story.
    fn revision_count(&self) -> usize {
        let mut n = 0;
        for blocks in self.each_story_blocks() {
            visit_blocks(blocks, &mut |e| n += usize::from(is_revision(&e.name) && e.attr("w:author").is_some()));
        }
        n
    }

    /// The personal and identifying properties the file holds: (what, value) pairs.
    fn property_items(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(core) = &self.core {
            if let Some(v) = text_of(&core.root, "dc:creator") {
                out.push(format!("author: {v}"));
            }
            if let Some(v) = text_of(&core.root, "cp:lastModifiedBy") {
                out.push(format!("lastModifiedBy: {v}"));
            }
        }
        if let Some(app) = &self.app {
            for (tag, name) in [("Company", "company"), ("Manager", "manager")] {
                if let Some(v) = app.root.elements().find(|e| e.local() == tag).map(|e| e.text().trim().to_string()).filter(|v| !v.is_empty()) {
                    out.push(format!("{name}: {v}"));
                }
            }
        }
        for part in self.related("", "custom-properties") {
            let n = self.part_now(&part).and_then(|d| xml::parse(&d).ok()).map(|d| d.root.elements().count()).unwrap_or(0);
            if n > 0 {
                out.push(format!("customProperties: {n}"));
            }
        }
        if !self.related("", "thumbnail").is_empty() {
            out.push("thumbnail".into());
        }
        out
    }

    /// Header and footer parts the sections show that hold any text or objects.
    fn header_footer_parts(&self) -> Vec<String> {
        let mut parts = BTreeSet::new();
        for (_, e) in self.section_refs() {
            for tag in ["w:headerReference", "w:footerReference"] {
                for r in e.into_iter().flat_map(|e| e.children_named(tag)) {
                    if let Some(rel) = r.attr("r:id").and_then(|id| self.pkg.resolve_rel(&self.main_path, id)) {
                        parts.insert(rel.target);
                    }
                }
            }
        }
        parts
            .into_iter()
            .filter(|part| {
                let Some(s) = self.stories.iter().find(|s| &s.part == part) else { return false };
                let mut any = false;
                walk_paragraphs(&s.blocks, &mut |p| any |= !p.plain_text().trim().is_empty() || p.text().contains(OBJ));
                any
            })
            .collect()
    }

    fn embedded_parts(&self) -> Vec<String> {
        self.pkg.parts.iter().map(|p| p.name.clone()).filter(|n| n.starts_with("word/embeddings/") || n.starts_with("word/activeX/") || n == "word/vbaProject.bin").collect()
    }

    /// What the document carries besides what the page shows, by category (see `Finding`). Every category is listed,
    /// with a count of 0 when there is nothing.
    pub fn inspect(&self) -> Vec<Finding> {
        let props = self.property_items();
        let hidden: usize = self.each_story_blocks().into_iter().map(|b| {
            let mut n = 0;
            walk_paragraphs(b, &mut |p| n += count_hidden(&p.inlines));
            n
        }).sum();
        let hf = self.header_footer_parts();
        let custom = self.custom_xml_parts();
        let embedded = self.embedded_parts();
        let people = self.author_names();
        let comments = self.comment_count();
        let revisions = self.revision_count();
        let mut out = Vec::with_capacity(CATEGORIES.len());
        out.push(Finding { id: "properties", count: props.len(), detail: props, removable: true });
        out.push(Finding { id: "authors", count: people.len(), detail: people, removable: true });
        out.push(Finding { id: "comments", count: comments, detail: Vec::new(), removable: true });
        out.push(Finding { id: "revisions", count: revisions, detail: Vec::new(), removable: true });
        out.push(Finding { id: "hiddenText", count: hidden, detail: Vec::new(), removable: true });
        out.push(Finding { id: "headersFooters", count: hf.len(), detail: Vec::new(), removable: true });
        out.push(Finding { id: "customXml", count: custom.len(), detail: Vec::new(), removable: true });
        out.push(Finding { id: "embedded", count: embedded.len(), detail: embedded, removable: false });
        out
    }

    /// Take the given categories (see `CATEGORIES`) out of the document. Returns how many things went.
    pub(super) fn op_remove_hidden_data(&mut self, what: &[String]) -> Result<OpResult, EditError> {
        for w in what {
            match CATEGORIES.iter().find(|c| *c == w) {
                Some(&"embedded") => return Err(EditError::Invalid("embedded objects can be listed but not removed here".into())),
                Some(_) => {}
                None => return Err(EditError::Invalid(format!("'{w}' is not something to remove; the choices are {}.", CATEGORIES.join(", ")))),
            }
        }
        let has = |c: &str| what.iter().any(|w| w == c);
        let mut count = 0;
        if has("revisions") {
            count += self.revision_count();
            self.edit_everywhere(&mut |b| accept_blocks(b));
        }
        if has("hiddenText") {
            let mut n = 0;
            self.edit_everywhere(&mut |b| {
                let k = hidden_in_blocks(b);
                n += k;
                k > 0
            });
            count += n;
        }
        if has("comments") {
            count += self.comment_count();
            self.edit_everywhere(&mut |b| comments_in_blocks(b));
            for part in self.comment_parts() {
                self.scrub.removed.insert(part);
            }
        }
        if has("headersFooters") {
            count += self.header_footer_parts().len();
            self.remove_all_headers_and_footers()?;
        }
        if has("customXml") {
            for part in self.custom_xml_parts() {
                // The properties file beside the item goes with it.
                for r in self.pkg.rels_for(&part).unwrap_or_default() {
                    if !r.external {
                        self.scrub.removed.insert(resolve_target(&part, &r.target));
                    }
                }
                self.scrub.removed.insert(part);
                count += 1;
            }
        }
        if has("authors") {
            count += self.author_names().len();
            self.rename_authors();
        }
        if has("properties") {
            count += self.property_items().len();
            self.remove_properties();
        }
        // Edits to stories and settings are written back with the rest; this keeps the file counted as changed.
        self.core_dirty = true;
        self.touch_all();
        Ok(OpResult { count: Some(count), ..Default::default() })
    }

    fn remove_all_headers_and_footers(&mut self) -> Result<(), EditError> {
        let mut parts = BTreeSet::new();
        let ids: Vec<Option<NodeId>> = self.section_refs().into_iter().map(|(id, _)| id).collect();
        for (_, e) in self.section_refs() {
            for tag in ["w:headerReference", "w:footerReference"] {
                for r in e.into_iter().flat_map(|e| e.children_named(tag)) {
                    if let Some(rel) = r.attr("r:id").and_then(|id| self.pkg.resolve_rel(&self.main_path, id)) {
                        parts.insert(rel.target);
                    }
                }
            }
        }
        for id in ids {
            self.with_sect(id.map(Id), &mut |s| {
                s.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "w:headerReference" || e.name == "w:footerReference"));
            })?;
        }
        // The parts nobody refers to now are left out of the saved file.
        self.hf_candidates.extend(parts);
        self.stories_touched = true;
        Ok(())
    }

    /// Replace the names behind comments and tracked changes by a neutral one.
    fn rename_authors(&mut self) {
        self.edit_everywhere(&mut |b| {
            visit_blocks_mut(b, &mut |e| {
                if is_revision(&e.name) && e.attr("w:author").is_some_and(|a| a != ANONYMOUS) {
                    e.set_attr("w:author", ANONYMOUS);
                    true
                } else {
                    false
                }
            })
        });
        for part in self.comment_parts() {
            let Some(mut doc) = self.part_now(&part).and_then(|d| xml::parse(&d).ok()) else { continue };
            fn go(e: &mut Element) -> bool {
                let mut changed = false;
                for (attr, to) in [("w:author", ANONYMOUS), ("w:initials", "A"), ("w15:author", ANONYMOUS)] {
                    if e.attr(attr).is_some_and(|v| v != to) {
                        e.set_attr(attr, to);
                        changed = true;
                    }
                }
                for c in e.elements_mut() {
                    changed |= go(c);
                }
                changed
            }
            if go(&mut doc.root) {
                self.scrub.replaced.insert(part, doc.to_bytes());
            }
        }
        // The list of people has nothing left to say.
        for part in self.related(&self.main_path, "people") {
            self.scrub.removed.insert(part);
        }
    }

    /// Blank the names and company, and take out custom properties and the preview picture.
    fn remove_properties(&mut self) {
        if let Some(core) = &mut self.core {
            for tag in ["dc:creator", "cp:lastModifiedBy"] {
                if let Some(e) = core.root.child_mut(tag) {
                    e.children.clear();
                }
            }
        }
        // The extended properties live in memory (and in the undo snapshot) like the core ones.
        if let Some(app) = &mut self.app {
            let before = app.clone();
            for e in app.root.elements_mut() {
                if matches!(e.local(), "Company" | "Manager") {
                    e.children.clear();
                }
            }
            if *app != before {
                self.app_dirty = true;
            }
        }
        for kind in ["custom-properties", "thumbnail"] {
            for part in self.related("", kind) {
                self.scrub.removed.insert(part);
            }
        }
        // Word then keeps the names out of the properties when it saves the file again.
        self.settings_edit(true, &mut |r| crate::props::put_ordered(r, Element::new("w:removePersonalInformation"), SETTINGS_ORDER));
    }
}
