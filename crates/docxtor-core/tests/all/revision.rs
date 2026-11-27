//! The revision number: it goes up by one each time the document changes, and only then. "Saved" is defined by it.

use docxtor_core::{Access, Document};
use serde_json::{json, Value};

fn ops(doc: &mut Document, v: Value, merge: bool) {
    doc.apply_json(&v.to_string(), merge).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn typed(doc: &mut Document, at: u32, o: usize, text: &str) -> Result<(), String> {
    doc.apply_json(&json!({"op":"insert_text","at":{"p":at,"o":o},"text":text}).to_string(), false).map(|_| ()).map_err(|e| e.to_string())
}

#[test]
fn a_batch_raises_it_by_one() {
    let mut doc = Document::new_blank();
    let r0 = doc.revision();
    let first = doc.render_all().order[0];
    assert_eq!(doc.revision(), r0, "drawing is not a change");
    typed(&mut doc, first, 0, "Hello").unwrap();
    assert_eq!(doc.revision(), r0 + 1);
    // A batch of two ops is one change.
    ops(&mut doc, json!([{"op":"insert_text","at":{"p":first,"o":0},"text":"A"},{"op":"insert_text","at":{"p":first,"o":1},"text":"B"}]), false);
    assert_eq!(doc.revision(), r0 + 2);
    // Typing that joins the last undo step is still a change.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"C"}), true);
    assert_eq!(doc.revision(), r0 + 3);
}

#[test]
fn undo_redo_and_revert_are_changes() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let cp = doc.checkpoint();
    typed(&mut doc, first, 0, "One").unwrap();
    typed(&mut doc, first, 3, " two").unwrap();
    let r = doc.revision();
    assert!(doc.undo());
    assert_eq!(doc.revision(), r + 1);
    assert!(doc.redo());
    assert_eq!(doc.revision(), r + 2);
    // Reverting two steps is two changes.
    assert!(doc.revert_to(cp));
    assert_eq!(doc.revision(), r + 4);
}

#[test]
fn nothing_to_undo_is_no_change() {
    let mut doc = Document::new_blank();
    let r = doc.revision();
    assert!(!doc.undo());
    assert!(!doc.redo());
    assert_eq!(doc.revision(), r);
}

#[test]
fn a_refused_batch_or_one_that_changes_nothing_leaves_it_alone() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    typed(&mut doc, first, 0, "Hello").unwrap();
    let r = doc.revision();
    // A batch that fails part-way is undone whole.
    let batch = json!([{"op":"insert_text","at":{"p":first,"o":0},"text":"X"},{"op":"insert_text","at":{"p":999999,"o":0},"text":"Y"}]);
    assert!(doc.apply_json(&batch.to_string(), false).is_err());
    assert_eq!(doc.revision(), r);
    // An empty batch.
    doc.apply_batch(&[], false).unwrap();
    assert_eq!(doc.revision(), r);
    // The field update that runs as a file opens changes nothing a person wrote.
    ops(&mut doc, json!({"op":"update_fields","open":true}), false);
    assert_eq!(doc.revision(), r);
    // Read-only: an edit is refused, undo is refused, a view-only op is allowed and changes nothing.
    doc.set_access(Access::ViewOnly);
    assert!(typed(&mut doc, first, 0, "X").is_err());
    assert!(!doc.undo());
    ops(&mut doc, json!({"op":"update_fields","open":true}), false);
    assert_eq!(doc.revision(), r);
    doc.set_access(Access::Edit);
    typed(&mut doc, first, 0, "X").unwrap();
    assert_eq!(doc.revision(), r + 1);
}

#[test]
fn saving_does_not_change_it() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    typed(&mut doc, first, 0, "Hello").unwrap();
    let r = doc.revision();
    let bytes = doc.save().unwrap();
    assert_eq!(doc.revision(), r);
    // A file that was opened starts at the same number whatever was saved in it.
    assert_eq!(Document::open(&bytes).unwrap().revision(), Document::open(&bytes).unwrap().revision());
}
