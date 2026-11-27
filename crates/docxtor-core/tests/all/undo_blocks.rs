//! An undo step copies only the top-level blocks an edit touched, and undo and redo put back only the blocks that differ.

use docxtor_core::ops::Op;
use docxtor_core::{perf, Document};
use serde_json::{json, Value};

const STAMP: &str = "2026-01-01T00:00:00Z";

fn op(v: Value) -> Op {
    serde_json::from_value(v).expect("an op")
}

fn novel(n: usize) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let text = (0..n).map(|i| format!("Paragraph {i}: the quick brown fox jumps over the lazy dog.")).collect::<Vec<_>>().join("\n\n");
    doc.apply(&op(json!({"op": "insert_markdown", "before": first, "markdown": text})), false).unwrap();
    doc.apply(&op(json!({"op": "delete_blocks", "ids": [first]})), false).unwrap();
    doc
}

#[test]
fn undoing_a_mix_of_edits_gives_back_the_saved_bytes_and_redoing_gives_back_the_edited_ones() {
    let mut doc = novel(300);
    // A style that is edited and then put back is saved again (as the engine writes it, not as the file had it), so the
    // baseline already has the styles part written that way.
    doc.apply(&op(json!({"op": "set_style_definition", "def": {"name": "Baseline style", "kind": "paragraph"}})), false).unwrap();
    // Building the document made steps of its own: what follows is undone down to them.
    let base = doc.undo_weight().1;
    let original = doc.save_stamped(STAMP).unwrap();
    let order = |d: &mut Document| d.render_all().order;
    let ids = order(&mut doc);
    let (a, b, c, d, e) = (ids[10], ids[11], ids[40], ids[41], ids[200]);

    let run = |doc: &mut Document, v: Value, merge: bool| {
        doc.apply(&op(v), merge).unwrap();
        doc.render_changed();
    };
    // Typing: the first key starts a step, the rest join it.
    run(&mut doc, json!({"op": "insert_text", "at": {"p": a, "o": 0}, "text": "x"}), false);
    for i in 1..6 {
        run(&mut doc, json!({"op": "insert_text", "at": {"p": a, "o": i}, "text": "y"}), true);
    }
    // Split, then join the two halves again, and join two paragraphs that were apart.
    run(&mut doc, json!({"op": "split_paragraph", "at": {"p": b, "o": 9}}), false);
    run(&mut doc, json!({"op": "delete", "from": {"p": c, "o": 20}, "to": {"p": d, "o": 0}}), false);
    // Delete paragraphs, move blocks, add a table, restyle, define a style.
    run(&mut doc, json!({"op": "delete_blocks", "ids": [ids[60], ids[61]]}), false);
    run(&mut doc, json!({"op": "move_blocks", "ids": [ids[100], ids[101]], "after": ids[250]}), false);
    run(&mut doc, json!({"op": "insert_table", "after": e, "rows": 3, "cols": 2, "data": [["a", "b"], ["c", "d"], ["e", "f"]]}), false);
    run(&mut doc, json!({"op": "set_style", "ids": [ids[150], ids[151]], "style": "Heading1"}), false);
    run(&mut doc, json!({"op": "set_style_definition", "def": {"name": "Docxtor test style", "kind": "paragraph"}}), false);
    // Typing again in a paragraph that was changed by an earlier step.
    run(&mut doc, json!({"op": "insert_text", "at": {"p": a, "o": 0}, "text": "z"}), false);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": a, "o": 1}, "text": "z"}), true);
    let edited = doc.save_stamped(STAMP).unwrap();
    let edited_order = order(&mut doc);
    assert_ne!(original, edited);

    // Undo everything: every step on the way draws what is on the page, and the file is the one we began with.
    let mut undone = 0;
    // What a client holds: the order it was last given (a render that did not move it does not send it again).
    let mut shown = edited_order.clone();
    while doc.undo_weight().1 > base && doc.undo() {
        undone += 1;
        let out = doc.render_changed();
        if !out.order_kept {
            shown = out.order.clone();
        }
        assert_eq!(shown.len(), doc.body.iter().filter(|b| docxtor_core::render::is_visible(b)).count());
    }
    assert!(undone >= 9, "{undone} steps undone");
    assert_eq!(doc.undo_weight().1, base);
    assert_eq!(doc.save_stamped(STAMP).unwrap(), original, "undoing every edit gives back the saved bytes");
    assert_eq!(order(&mut doc), ids);

    // Redo everything.
    let mut redone = 0;
    while doc.redo() {
        redone += 1;
        doc.render_changed();
    }
    assert_eq!(redone, undone);
    assert_eq!(doc.save_stamped(STAMP).unwrap(), edited, "redoing every edit gives back the edited bytes");
    assert_eq!(order(&mut doc), edited_order);

    // And once more the other way, to make sure the copies the redo made are right.
    while doc.undo_weight().1 > base && doc.undo() {}
    assert_eq!(doc.save_stamped(STAMP).unwrap(), original);
}

#[test]
fn undo_draws_again_only_the_blocks_it_changed() {
    let mut doc = novel(500);
    let ids = doc.render_all().order;
    doc.apply(&op(json!({"op": "insert_text", "at": {"p": ids[250], "o": 0}, "text": "x"})), false).unwrap();
    doc.render_changed();
    assert!(doc.undo());
    let out = doc.render_changed();
    assert!(!out.full);
    assert!(out.order_kept, "undoing a keystroke moves no block, so the order is not sent again");
    assert_eq!(out.blocks.len(), 1, "undoing one paragraph's text draws one block, not {}", out.blocks.len());
    assert!(doc.redo());
    assert_eq!(doc.render_changed().blocks.len(), 1);

    // A split and its undo: the block that was split, and the one that came out of it, are drawn; the rest are not.
    doc.apply(&op(json!({"op": "split_paragraph", "at": {"p": ids[100], "o": 9}})), false).unwrap();
    doc.render_changed();
    assert!(doc.undo());
    let out = doc.render_changed();
    assert_eq!(out.order, ids);
    assert!(out.blocks.len() <= 2, "{} blocks drawn after undoing a split", out.blocks.len());
}

#[test]
fn a_keystroke_in_a_long_document_copies_no_whole_body() {
    let mut doc = novel(1000);
    let ids = doc.render_all().order;
    let mid = ids[500];
    doc.apply(&op(json!({"op": "insert_text", "at": {"p": mid, "o": 0}, "text": "x"})), false).unwrap();
    doc.render_changed();
    let (bodies, blocks) = (perf::body_clones(), perf::block_clones());
    let weight = doc.undo_weight().0;
    let keys = 50;
    for i in 1..=keys {
        doc.apply(&op(json!({"op": "insert_text", "at": {"p": mid, "o": i}, "text": "x"})), true).unwrap();
        doc.render_changed();
    }
    assert_eq!(perf::body_clones(), bodies, "a keystroke copies no whole body");
    let per_key = (perf::block_clones() - blocks) as f64 / keys as f64;
    assert!(per_key <= 2.0, "a keystroke copies {per_key} blocks");
    // Typing joins the last step, which is all it is: no new step, no weight.
    assert_eq!(doc.undo_weight().0, weight);
    // A step of its own costs the block it touched, not the book.
    doc.apply(&op(json!({"op": "insert_text", "at": {"p": ids[700], "o": 0}, "text": "y"})), false).unwrap();
    let step = doc.undo_weight().0 - weight;
    assert!(step < 4096, "a one-paragraph step weighs {step} bytes in a 1,000 paragraph document");
}
