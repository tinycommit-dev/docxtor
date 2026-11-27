//! A bookmark is a start and an end that belong together. An op that takes out one of them (deleting the paragraph
//! that holds the end of a bookmark spanning paragraphs, a range delete, a merge, a rebuilt field, typing over a
//! placeholder) must not leave the other behind: the saved file would have a start without an end, or the reverse.
//!
//! This is one step of `after_batch`, over a list of blocks, so it covers every op that removes content without each
//! one having to know about bookmarks. The bookmark is kept, shrunk to what is left, so a link to it still works: a
//! start that lost its end gets one at the end of its own paragraph, and an end that lost its start gets the start
//! (same name, same id) at the front of its own paragraph. Halves that were already unpaired before the batch (a file
//! that came that way) are left exactly as they were.

use crate::model::{walk_paragraphs_mut, Block, Body, Inline, NodeId, RowChild, TableChild};
use crate::xml::{Element, Node};
use std::collections::{HashMap, HashSet};

/// What a list of blocks held before a batch: the halves already unpaired, and every bookmark start as it was.
#[derive(Default)]
pub(super) struct Marks {
    unpaired: HashSet<(bool, String)>,
    starts: HashMap<String, Element>,
    /// The bookmarks the links lead to, in lower case.
    anchors: HashSet<String>,
}

fn note(e: &Element, starts: &mut HashMap<String, Element>, ends: &mut HashSet<String>) {
    match e.name.as_str() {
        "w:bookmarkStart" => {
            starts.insert(e.attr("w:id").unwrap_or_default().to_string(), e.clone());
        }
        "w:bookmarkEnd" => {
            ends.insert(e.attr("w:id").unwrap_or_default().to_string());
        }
        _ => {}
    }
    for n in &e.children {
        if let Node::Element(c) = n {
            note(c, starts, ends);
        }
    }
}

fn collect_inlines(list: &[Inline], starts: &mut HashMap<String, Element>, ends: &mut HashSet<String>, anchors: &mut Vec<String>) {
    for i in list {
        match i {
            Inline::Opaque(e) => note(e, starts, ends),
            Inline::Container(c) => {
                if c.kind == crate::model::InlineKind::Hyperlink {
                    if let Some(a) = c.shell.attr("w:anchor") {
                        anchors.push(a.to_lowercase());
                    }
                }
                collect_inlines(&c.children, starts, ends, anchors)
            }
            Inline::Run(_) => {}
        }
    }
}

fn collect<'a>(blocks: impl IntoIterator<Item = &'a Block>, starts: &mut HashMap<String, Element>, ends: &mut HashSet<String>, anchors: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Para(p) => collect_inlines(&p.inlines, starts, ends, anchors),
            Block::Table(t) => {
                for child in &t.children {
                    match child {
                        TableChild::Row(r) => {
                            for rc in &r.children {
                                match rc {
                                    RowChild::Cell(c) => collect(&c.blocks, starts, ends, anchors),
                                    RowChild::Other(e) | RowChild::Marker(e, _) => note(e, starts, ends),
                                }
                            }
                        }
                        // A bookmark over table columns ends after the rows (Word's `colFirst`).
                        TableChild::Other(e) | TableChild::Marker(e, _) => note(e, starts, ends),
                    }
                }
            }
            Block::Container(c) => collect(&c.blocks, starts, ends, anchors),
            Block::Opaque(o) => note(&o.el, starts, ends),
        }
    }
}

impl Marks {
    /// Note the bookmarks in these blocks.
    pub(super) fn of<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Marks {
        let (mut starts, mut ends, mut anchors) = (HashMap::new(), HashSet::new(), Vec::new());
        collect(blocks, &mut starts, &mut ends, &mut anchors);
        let anchors = anchors.into_iter().collect();
        let mut unpaired: HashSet<(bool, String)> = starts.keys().filter(|id| !ends.contains(*id)).map(|id| (true, id.clone())).collect();
        unpaired.extend(ends.iter().filter(|id| !starts.contains_key(*id)).map(|id| (false, id.clone())));
        Marks { unpaired, starts, anchors }
    }

    /// The halves without their other half (`true`: a start).
    pub(super) fn unpaired(&self) -> &HashSet<(bool, String)> {
        &self.unpaired
    }

    /// The bookmarks the links lead to, in lower case.
    pub(super) fn anchors(&self) -> &HashSet<String> {
        &self.anchors
    }

    /// The names of the bookmarks, in lower case (Word matches names without regard to case).
    pub(super) fn names(&self) -> HashSet<String> {
        self.starts.values().filter_map(|e| e.attr("w:name")).map(str::to_lowercase).collect()
    }
}

/// The names of the bookmarks in these blocks, in lower case.
pub(super) fn names_in<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> HashSet<String> {
    Marks::of(blocks).names()
}

/// The ids of the bookmark starts and ends in one top-level block: what the body keeps per block
/// (`Document::body_unpaired_bookmarks`), so that a key does not walk the body to compare its bookmarks.
#[derive(Debug, PartialEq)]
pub(crate) struct Halves {
    pub(super) starts: Vec<String>,
    pub(super) ends: Vec<String>,
    /// The names of the starts, in lower case: a name no block has any more is a bookmark gone, and links to it become text.
    pub(super) names: Vec<String>,
    /// The bookmarks the block's links lead to, in lower case: a link that comes to lead nowhere becomes text.
    pub(super) anchors: Vec<String>,
}

impl Halves {
    /// The halves in a block, or `None` when it has none (nearly every block).
    pub(super) fn of(b: &Block) -> Option<std::sync::Arc<Halves>> {
        let (mut starts, mut ends, mut anchors) = (HashMap::new(), HashSet::new(), Vec::new());
        collect(std::slice::from_ref(b), &mut starts, &mut ends, &mut anchors);
        let names = starts.values().filter_map(|e| e.attr("w:name")).map(str::to_lowercase).collect();
        (!starts.is_empty() || !ends.is_empty() || !anchors.is_empty()).then(|| std::sync::Arc::new(Halves { starts: starts.into_keys().collect(), ends: ends.into_iter().collect(), names, anchors }))
    }
}

/// Every bookmark start in these blocks, by id.
pub(super) fn starts_of<'a>(blocks: impl Iterator<Item = &'a Block>) -> HashMap<String, Element> {
    let (mut starts, mut ends) = (HashMap::new(), HashSet::new());
    for b in blocks {
        collect(std::slice::from_ref(b), &mut starts, &mut ends, &mut Vec::new());
    }
    starts
}

fn has_marker(list: &[Inline], name: &str, id: &str) -> bool {
    list.iter().any(|i| match i {
        Inline::Opaque(e) => e.name == name && e.attr("w:id") == Some(id),
        Inline::Container(c) => has_marker(&c.children, name, id),
        Inline::Run(_) => false,
    })
}

fn drop_marker(list: &mut Vec<Inline>, name: &str, id: &str) {
    list.retain(|i| !matches!(i, Inline::Opaque(e) if e.name == name && e.attr("w:id") == Some(id)));
    for i in list.iter_mut() {
        if let Inline::Container(c) = i {
            drop_marker(&mut c.children, name, id);
        }
    }
}

/// Take out the lone halves among these blocks and in them. Returns the ids of the blocks taken out of this list and of the blocks of
/// it that lost one inside them, so the caller marks them (the kept bookmark facts read only marked blocks).
fn drop_lone_halves(blocks: &mut Vec<Block>, fresh: &[&(bool, String)]) -> Vec<NodeId> {
    let mut changed = Vec::new();
    blocks.retain(|b| {
        if is_lone(b, fresh) {
            changed.push(b.id());
            return false;
        }
        true
    });
    for b in blocks.iter_mut() {
        if drop_lone_inside(b, fresh) {
            changed.push(b.id());
        }
    }
    changed
}

/// Whether this block is a lone bookmark half that is to go.
fn is_lone(b: &Block, fresh: &[&(bool, String)]) -> bool {
    let Block::Opaque(o) = b else { return false };
    let (is_start, id) = (o.el.name == "w:bookmarkStart", o.el.attr("w:id").unwrap_or_default());
    matches!(o.el.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd") && fresh.iter().any(|(s, i)| *s == is_start && i == id)
}

/// Take the lone halves out of what is inside this block. True when one went.
fn drop_lone_inside(b: &mut Block, fresh: &[&(bool, String)]) -> bool {
    match b {
        Block::Container(c) => !drop_lone_halves(&mut c.blocks, fresh).is_empty(),
        Block::Table(t) => {
            let lone = |e: &Element| {
                let (is_start, id) = (e.name == "w:bookmarkStart", e.attr("w:id").unwrap_or_default());
                matches!(e.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd") && fresh.iter().any(|(s, i)| *s == is_start && i == id)
            };
            let n = t.children.len();
            t.children.retain(|c| !matches!(c, TableChild::Other(e) | TableChild::Marker(e, _) if lone(e)));
            let mut any = t.children.len() != n;
            for c in t.children.iter_mut() {
                if let TableChild::Row(r) = c {
                    let n = r.children.len();
                    r.children.retain(|c| !matches!(c, RowChild::Other(e) | RowChild::Marker(e, _) if lone(e)));
                    any |= r.children.len() != n;
                    for cell in r.cells_mut() {
                        any |= !drop_lone_halves(&mut cell.blocks, fresh).is_empty();
                    }
                }
            }
            any
        }
        Block::Para(_) | Block::Opaque(_) => false,
    }
}

/// Put back the other end of every bookmark this batch left with one end. Returns the top-level blocks changed.
pub(super) fn mend(blocks: &mut Vec<Block>, before: &Marks) -> Vec<NodeId> {
    let now = Marks::of(&*blocks);
    let fresh: Vec<&(bool, String)> = now.unpaired.iter().filter(|h| !before.unpaired.contains(*h)).collect();
    mend_fresh(blocks, &fresh, &before.starts)
}

/// Put back the other end of each of the `fresh` halves (the halves a batch left without their other half), the
/// starts from `starts` (the bookmark starts as they were before the batch). Returns the top-level blocks changed.
pub(super) fn mend_fresh(blocks: &mut Vec<Block>, fresh: &[&(bool, String)], starts: &HashMap<String, Element>) -> Vec<NodeId> {
    if fresh.is_empty() {
        return Vec::new();
    }
    // A half that stands alone between blocks (Word's `_GoBack` is written that way, directly under the body, and a
    // table of contents holds its heading bookmarks that way) has no text to shrink to: when the other half is gone,
    // so is this one.
    let mut changed = drop_lone_halves(blocks, fresh);
    for b in blocks.iter_mut() {
        if mend_in(b, fresh, starts) {
            changed.push(b.id());
        }
    }
    changed
}

/// `mend_fresh` for the body: only the blocks that change are copied from an undo step that shares them.
pub(super) fn mend_fresh_body(body: &mut Body, fresh: &[&(bool, String)], starts: &HashMap<String, Element>) -> Vec<NodeId> {
    if fresh.is_empty() {
        return Vec::new();
    }
    let mut changed = Vec::new();
    body.retain(|b| {
        if is_lone(b, fresh) {
            changed.push(b.id());
            return false;
        }
        true
    });
    for i in 0..body.len() {
        let id = body[i].id();
        if body.try_edit(i, |b| {
            let a = drop_lone_inside(b, fresh);
            mend_in(b, fresh, starts) | a
        }) {
            changed.push(id);
        }
    }
    changed
}

/// Put back the missing halves in the paragraphs of this block. True when one was.
fn mend_in(b: &mut Block, fresh: &[&(bool, String)], starts: &HashMap<String, Element>) -> bool {
    let mut touched = false;
    walk_paragraphs_mut(std::slice::from_mut(b), &mut |p| {
        for (is_start, id) in fresh {
            if *is_start && has_marker(&p.inlines, "w:bookmarkStart", id) {
                p.inlines.push(Inline::Opaque(Element::new("w:bookmarkEnd").with_attr("w:id", id)));
                touched = true;
            } else if !*is_start && has_marker(&p.inlines, "w:bookmarkEnd", id) {
                // The start is put back from what it was before, with its name. A start the batch made and then took out
                // again (a bookmark set on a cover page that the same batch removes) was never anywhere before: there is
                // no name to put back, so the end goes too.
                match starts.get(id) {
                    Some(start) => p.inlines.insert(0, Inline::Opaque(start.clone())),
                    None => drop_marker(&mut p.inlines, "w:bookmarkEnd", id),
                }
                touched = true;
            }
        }
    });
    touched
}
