//! Every tracked formatting change (`w:rPrChange`, `w:pPrChange` and the like) carries an id that is unique in its
//! part. Splitting a run or a paragraph copies the properties with their change mark, so both halves then carry the
//! same id, and the Open XML SDK (and Word's own checks) refuse the part. After a batch that changed a paragraph
//! holding such a mark, the duplicates get fresh ids.

use crate::model::{walk_elements, walk_elements_mut, Block, Body, Inline, NodeId, Paragraph};
use crate::xml::Element;
use std::collections::{BTreeSet, HashSet};

const CHANGES: &[&str] = &["w:rPrChange", "w:pPrChange", "w:sectPrChange", "w:tblPrChange", "w:tblGridChange", "w:trPrChange", "w:tcPrChange", "w:numberingChange"];

fn is_change(e: &Element) -> bool {
    CHANGES.contains(&e.name.as_str())
}

fn has_change(e: &Element) -> bool {
    is_change(e) || e.elements().any(has_change)
}

fn inlines_have_change(list: &[Inline]) -> bool {
    list.iter().any(|i| match i {
        Inline::Run(r) => r.rpr.as_ref().is_some_and(has_change),
        Inline::Container(c) => inlines_have_change(&c.children),
        Inline::Opaque(_) => false,
    })
}

fn paragraph_has_change(p: &Paragraph) -> bool {
    p.ppr.as_ref().is_some_and(has_change) || inlines_have_change(&p.inlines)
}

fn any_changed_with_change<'a, I: IntoIterator<Item = &'a Block> + Copy>(blocks: I, changed: &BTreeSet<NodeId>) -> bool {
    // A changed id is a top-level block's: only those are looked into, not every paragraph of the document.
    let mut any = false;
    for b in blocks.into_iter().filter(|b| changed.contains(&b.id())) {
        crate::model::walk_paragraphs(std::slice::from_ref(b), &mut |p| any |= changed.contains(&p.id) && paragraph_has_change(p));
    }
    #[cfg(any(test, debug_assertions))]
    {
        let mut all = false;
        crate::perf::unmetered(|| crate::model::walk_paragraphs(blocks, &mut |p| all |= changed.contains(&p.id) && paragraph_has_change(p)));
        assert_eq!(any, all, "a changed paragraph holding a formatting change was missed");
    }
    any
}

/// Ids of the other revision marks, which share the numbers with the formatting changes.
const OTHERS: &[&str] = &["w:ins", "w:del", "w:moveFrom", "w:moveTo", "w:moveFromRangeStart", "w:moveFromRangeEnd", "w:moveToRangeStart", "w:moveToRangeEnd", "w:cellIns", "w:cellDel", "w:cellMerge"];

fn renumber(blocks: &mut [Block], seen: &mut HashSet<String>, max: &mut i64) {
    walk_elements_mut(blocks, &mut |e| {
        if !is_change(e) {
            return;
        }
        let Some(id) = e.attr("w:id").map(String::from) else { return };
        if !seen.insert(id) {
            *max += 1;
            e.set_attr("w:id", max.to_string());
            seen.insert(max.to_string());
        }
    });
}

/// Give the second and later holder of an id a fresh one, when a changed paragraph holds a formatting change.
pub(super) fn unique_change_ids(blocks: &mut [Block], changed: &BTreeSet<NodeId>) {
    if !any_changed_with_change(&*blocks, changed) {
        return;
    }
    // Fresh ids go above every revision's id (insertions, deletions and moves share the numbers with these), so a new
    // one never meets an existing mark.
    let mut max = 0i64;
    walk_elements(&*blocks, &mut |e| {
        if is_change(e) || OTHERS.contains(&e.name.as_str()) {
            max = max.max(e.attr("w:id").and_then(|v| v.parse().ok()).unwrap_or(0));
        }
    });
    renumber(blocks, &mut HashSet::new(), &mut max);
}

/// `unique_change_ids` for the body: only the blocks that hold a repeated id are copied from an undo step that shares them.
pub(super) fn unique_change_ids_body(body: &mut Body, changed: &BTreeSet<NodeId>) {
    if !any_changed_with_change(&*body, changed) {
        return;
    }
    let mut max = 0i64;
    walk_elements(&*body, &mut |e| {
        if is_change(e) || OTHERS.contains(&e.name.as_str()) {
            max = max.max(e.attr("w:id").and_then(|v| v.parse().ok()).unwrap_or(0));
        }
    });
    // The blocks that hold a second or later holder of an id: a look at each block, in order.
    let mut seen: HashSet<String> = HashSet::new();
    let mut repeats = Vec::new();
    for (i, b) in body.iter().enumerate() {
        let mut repeat = false;
        walk_elements(std::slice::from_ref(b), &mut |e| {
            if is_change(e) {
                if let Some(id) = e.attr("w:id") {
                    repeat |= !seen.insert(id.to_string());
                }
            }
        });
        if repeat {
            repeats.push(i);
        }
    }
    // Then the same walk as for a list of blocks, changing only those.
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..body.len() {
        if repeats.contains(&i) {
            renumber(std::slice::from_mut(&mut body[i]), &mut seen, &mut max);
        } else {
            walk_elements(std::slice::from_ref(&body[i]), &mut |e| {
                if is_change(e) {
                    if let Some(id) = e.attr("w:id") {
                        seen.insert(id.to_string());
                    }
                }
            });
        }
    }
}
