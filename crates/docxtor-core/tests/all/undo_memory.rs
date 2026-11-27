//! The undo history has a bound: editing for a long time must not make a document hold more and more memory.
//! A step holds only the blocks it copied (the ones the edit touched), so the budgets here are in kilobytes: a typed
//! character costs about one paragraph, not the whole document.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::Document;
use serde_json::json;

fn edit(doc: &mut Document, value: serde_json::Value) {
    doc.apply_json(&value.to_string(), false).unwrap();
}

fn doc_with_paragraphs(n: usize) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let text = (0..n).map(|i| format!("Paragraph {i}: the quick brown fox jumps over the lazy dog, again and again, all afternoon.")).collect::<Vec<_>>().join("\n\n");
    edit(&mut doc, json!({"op": "insert_markdown", "before": first, "markdown": text}));
    edit(&mut doc, json!({"op": "delete_blocks", "ids": [first]}));
    let id = doc.render_all().order[0];
    (doc, id)
}

#[test]
fn ten_thousand_edits_keep_the_undo_history_bounded() {
    let (mut doc, id) = doc_with_paragraphs(20);
    for i in 0..10_000 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": format!("{}", i % 10)}));
    }
    let (bytes, undo, redo) = doc.undo_weight();
    assert_eq!(redo, 0);
    assert!(undo <= 500, "{undo} undo steps are kept");
    assert!(bytes <= 128 << 20, "the undo steps take about {bytes} bytes");
    // What is kept still undoes, one step at a time, and then there is nothing more.
    let mut undone = 0;
    while doc.undo() {
        undone += 1;
    }
    assert_eq!(undone, undo);
    assert!(!doc.can_undo());
    assert_eq!(doc.redo_labels().len(), undone);
}

#[test]
fn a_small_budget_keeps_fewer_steps_so_the_memory_stays_inside_it() {
    let (mut doc, id) = doc_with_paragraphs(400);
    doc.set_undo_budget(16 << 10);
    for i in 0..300 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": format!("{}", i % 10)}));
    }
    let (bytes, undo, _) = doc.undo_weight();
    assert!(bytes <= 16 << 10, "the undo steps take about {bytes} bytes");
    assert!((10..300).contains(&undo), "{undo} undo steps are kept");
    // The newest steps are the ones kept: undo works.
    let checkpoint = doc.checkpoint();
    edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": "!"}));
    assert!(doc.checkpoint() > checkpoint);
    assert!(doc.revert_to(checkpoint));
    assert!(!first_text(&doc, id).starts_with('!'));
}

fn first_text(doc: &Document, id: u32) -> String {
    let mut first = String::new();
    walk_paragraphs(&doc.body, &mut |p| {
        if p.id == id {
            first = p.plain_text();
        }
    });
    first
}

#[test]
fn a_checkpoint_the_budget_let_go_of_is_not_mixed_up_with_the_steps_that_are_left() {
    let (mut doc, id) = doc_with_paragraphs(400);
    doc.set_undo_budget(16 << 10);
    let old = doc.checkpoint();
    for i in 0..300 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": format!("{}", i % 10)}));
    }
    let (_, undo, _) = doc.undo_weight();
    assert!(undo < 300, "{undo} undo steps are kept");
    // Its steps are gone: nothing to compare with, and reverting does nothing rather than undoing part of the way.
    let text = first_text(&doc, id);
    assert!(doc.diff_since(old).is_empty());
    assert!(!doc.revert_to(old));
    assert_eq!(first_text(&doc, id), text);
    assert_eq!(doc.undo_weight().1, undo);
}

#[test]
fn a_pinned_checkpoint_keeps_its_steps_whatever_the_budget_says() {
    let (mut doc, id) = doc_with_paragraphs(400);
    let before_text = first_text(&doc, id);
    doc.set_undo_budget(16 << 10);
    let cp = doc.checkpoint();
    doc.pin(cp);
    for i in 0..300 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": format!("{}", i % 10)}));
    }
    assert_eq!(doc.undo_weight().1, 300, "every step since the checkpoint is kept while it is pinned");
    let diff = doc.diff_since(cp);
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].1.as_deref(), Some(before_text.as_str()));
    // Once it is let go of, the budget applies again.
    doc.unpin(cp);
    assert!(doc.undo_weight().1 < 300);
    // A second hold on the same checkpoint needs its own release.
    let (mut doc, id) = doc_with_paragraphs(400);
    doc.set_undo_budget(16 << 10);
    let cp = doc.checkpoint();
    doc.pin(cp);
    doc.pin(cp);
    for _ in 0..100 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": "x"}));
    }
    doc.unpin(cp);
    assert_eq!(doc.undo_weight().1, 100);
    // Pinned, a revert still takes back everything since the checkpoint.
    assert!(doc.revert_to(cp));
    assert!(!first_text(&doc, id).starts_with('x'));
}

#[test]
fn many_footnotes_are_counted_by_the_step_that_changes_them_and_shared_by_the_others() {
    let (mut doc, id) = doc_with_paragraphs(30);
    for i in 0..300 {
        edit(&mut doc, json!({"op": "insert_footnote", "at": {"p": id, "o": 5}, "text": format!("Footnote number {i}: a long enough note to give the notes part some weight, with sources and page numbers.")}));
    }
    // A step records the state before it. The first step after the notes changed copies the part afresh, the ones
    // after it share that copy, and the budget counts the fresh copy once.
    let step = |doc: &mut Document, op: serde_json::Value| {
        let before = doc.undo_weight().0;
        edit(doc, op);
        doc.undo_weight().0 - before
    };
    let text = |s: &str| json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": s});
    let first = step(&mut doc, text("a"));
    let second = step(&mut doc, text("b"));
    let third = step(&mut doc, text("c"));
    step(&mut doc, json!({"op": "insert_footnote", "at": {"p": id, "o": 7}, "text": "One more."}));
    let after_notes = step(&mut doc, text("d"));
    assert!(third <= second + second / 10, "steps that leave the notes alone cost {second} and {third}");
    assert!(first > second + second / 2, "the step that copies the notes weighs {first}, one that shares them {second}");
    assert!(after_notes > third + third / 2, "the step after a notes change weighs {after_notes}, one that shares {third}");
    // And the budget holds with the notes in the document.
    doc.set_undo_budget(16 << 20);
    for i in 0..200 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": format!("{}", i % 10)}));
    }
    let (bytes, undo, _) = doc.undo_weight();
    assert!(bytes <= 16 << 20, "the undo steps take about {bytes} bytes");
    assert!(undo >= 10);
}

#[test]
fn a_document_bigger_than_the_budget_still_undoes_its_last_steps() {
    let (mut doc, id) = doc_with_paragraphs(200);
    doc.set_undo_budget(1);
    for _ in 0..50 {
        edit(&mut doc, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": "x"}));
    }
    assert_eq!(doc.undo_weight().1, 10);
    assert!(doc.undo_steps(10));
    assert!(!doc.can_undo());
}
