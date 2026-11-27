//! Built-in features name their styles by Word's own ids (`TOC1`, `PlaceholderText`, `IndexHeading`). A file made by
//! another producer, or by a Word in another language, defines the same built-in style under another id (`Verzeichnis1`
//! is named "toc 1"). `ensure_builtin` finds that style by its name and does not add a second one, so a paragraph or
//! run written with Word's id would name a style the file does not have. After a batch the paragraphs it changed are
//! pointed at the id the file really uses.

use super::*;
use crate::model::Inline;
use crate::styles::StyleSheet;
use std::collections::BTreeSet;

/// The id the file defines for `id` when it does not define `id` itself but a style of that name.
fn real_id(styles: &StyleSheet, id: &str) -> Option<String> {
    if styles.exists(id) {
        return None;
    }
    styles.lookup(id).filter(|r| r != id)
}

fn runs(inlines: &mut [Inline], styles: &StyleSheet) -> bool {
    let mut any = false;
    for i in inlines.iter_mut() {
        match i {
            Inline::Run(r) => {
                if let Some(v) = r.rpr.as_mut().and_then(|rpr| rpr.child_mut("w:rStyle")) {
                    if let Some(to) = v.attr("w:val").and_then(|id| real_id(styles, id)) {
                        v.set_attr("w:val", to);
                        any = true;
                    }
                }
            }
            Inline::Container(c) => any |= runs(&mut c.children, styles),
            Inline::Opaque(_) => {}
        }
    }
    any
}

fn paragraph(p: &mut crate::model::Paragraph, styles: &StyleSheet) -> bool {
    let mut any = false;
    if let Some(v) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:pStyle")) {
        if let Some(to) = v.attr("w:val").and_then(|id| real_id(styles, id)) {
            v.set_attr("w:val", to);
            any = true;
        }
    }
    any | runs(&mut p.inlines, styles)
}

/// The same for every paragraph of the blocks.
pub(super) fn point_all_style_refs(blocks: &mut [Block], styles: &StyleSheet) {
    point_style_refs_in(blocks, &BTreeSet::new(), true, styles);
}

/// `point_all_style_refs` for the body: only the blocks that change are copied from an undo step that shares them.
pub(super) fn point_all_style_refs_body(body: &mut Body, styles: &StyleSheet) -> Vec<NodeId> {
    body.edit_each(|s| point_style_refs_in(s, &BTreeSet::new(), true, styles))
}

/// Point the paragraphs in `changed` (and their runs) at the styles the file really defines. Everything inside a
/// changed table or content control counts as changed: an op that adds a control puts new paragraphs in it.
pub(super) fn point_style_refs(blocks: &mut [Block], changed: &BTreeSet<NodeId>, styles: &StyleSheet) {
    point_style_refs_in(blocks, changed, false, styles);
}

/// `point_style_refs` for the body: looks only at the blocks in `changed`.
pub(super) fn point_style_refs_body(body: &mut Body, changed: &BTreeSet<NodeId>, styles: &StyleSheet) {
    for b in body.iter_mut_where(|b| changed.contains(&b.id())) {
        point_style_refs_in(std::slice::from_mut(b), changed, false, styles);
    }
}

/// True when something was pointed at another id.
fn point_style_refs_in(blocks: &mut [Block], changed: &BTreeSet<NodeId>, all: bool, styles: &StyleSheet) -> bool {
    fn go(blocks: &mut [Block], changed: &BTreeSet<NodeId>, all: bool, styles: &StyleSheet) -> bool {
        let mut any = false;
        for b in blocks {
            let here = all || changed.contains(&b.id());
            match b {
                Block::Para(p) if here => any |= paragraph(p, styles),
                Block::Para(_) | Block::Opaque(_) => {}
                Block::Table(t) => {
                    for r in t.rows_mut() {
                        for c in r.cells_mut() {
                            any |= go(&mut c.blocks, changed, here, styles);
                        }
                    }
                }
                Block::Container(c) => any |= go(&mut c.blocks, changed, here, styles),
            }
        }
        any
    }
    go(blocks, changed, all, styles)
}
