//! Who may change the document: while it is view only, the engine refuses every way in (ops, the assistant's tools,
//! undo and redo, revert, media) and lets through only what changes nothing a person wrote.

use docxtor_core::ai::call_tool;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::{Access, Document};
use serde_json::{json, Value};

fn ops(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn doc_with(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":text}));
    (doc, first)
}

#[test]
fn a_document_is_editable_until_the_host_says_otherwise() {
    let (mut doc, first) = doc_with("Hello");
    assert_eq!(doc.access(), Access::Edit);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":5},"text":" there"}));
    assert_eq!(texts(&doc)[0], "Hello there");
}

#[test]
fn a_view_only_document_refuses_edits_and_stays_as_it_was() {
    let (mut doc, first) = doc_with("Hello");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":5},"text":" again"}));
    let steps = doc.undo_weight().1;
    doc.set_access(Access::ViewOnly);
    let e = doc.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"X"}).to_string(), false).unwrap_err().to_string();
    assert_eq!(e, "This document is read-only.");
    // A batch with one allowed op and one edit is refused whole.
    let batch = json!([{"op":"update_fields","open":true},{"op":"insert_text","at":{"p":first,"o":0},"text":"X"}]);
    assert!(doc.apply_json(&batch.to_string(), false).is_err());
    assert_eq!(texts(&doc)[0], "Hello again");
    assert_eq!(doc.undo_weight().1, steps, "nothing was recorded");
    // Editing works again when the host says so.
    doc.set_access(Access::Edit);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"X"}));
    assert_eq!(texts(&doc)[0], "XHello again");
}

#[test]
fn a_view_only_document_cannot_undo_redo_or_revert() {
    let (mut doc, first) = doc_with("Hello");
    let checkpoint = doc.checkpoint();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":5},"text":" one"}));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":9},"text":" two"}));
    assert!(doc.undo());
    doc.set_access(Access::ViewOnly);
    assert!(!doc.undo());
    assert!(!doc.redo());
    assert!(!doc.undo_steps(3));
    assert!(!doc.redo_steps(3));
    assert!(!doc.revert_to(checkpoint));
    assert_eq!(texts(&doc)[0], "Hello one");
    assert!(doc.can_undo() && doc.can_redo(), "the history is kept");
    doc.set_access(Access::Edit);
    assert!(doc.redo());
    assert_eq!(texts(&doc)[0], "Hello one two");
    assert!(doc.revert_to(checkpoint));
    assert_eq!(texts(&doc)[0], "Hello");
}

#[test]
fn a_view_only_document_takes_no_media() {
    let (mut doc, _) = doc_with("Hello");
    doc.set_access(Access::ViewOnly);
    let e = doc.try_add_media("png", vec![0x89, b'P', b'N', b'G']).unwrap_err().to_string();
    assert_eq!(e, "This document is read-only.");
    assert!(!doc.pkg.parts.iter().any(|p| p.name.starts_with("word/media/")));
    doc.set_access(Access::Edit);
    assert!(doc.try_add_media("png", vec![0x89, b'P', b'N', b'G']).is_ok());
}

#[test]
fn the_assistants_tools_are_refused_too() {
    let (mut doc, first) = doc_with("Hello");
    doc.set_access(Access::ViewOnly);
    let r = call_tool(&mut doc, "apply_ops", &json!({"ops":[{"op":"insert_text","at":{"p":first,"o":0},"text":"X"}]}));
    assert!(r.unwrap_err().contains("read-only"));
    let r = call_tool(&mut doc, "replace_text", &json!({"find":"Hello","replace":"Bye"}));
    assert!(r.unwrap_err().contains("read-only"), "a tool that applies ops is refused");
    assert_eq!(texts(&doc)[0], "Hello");
    // Reading is still allowed.
    assert!(call_tool(&mut doc, "read_document", &json!({})).unwrap().contains("Hello"));
}

#[test]
fn tabs_can_be_shown_and_fields_updated_as_a_file_opens_and_before_printing() {
    let (mut doc, _) = doc_with("Overview text");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Details","first_name":"Overview"}));
    let first_tab = doc.tabs_info().tabs[0].id.clone();
    doc.set_access(Access::ViewOnly);
    // Adding and renaming tabs is an edit.
    assert!(doc.apply_json(&json!({"op":"tab","action":"add","name":"More"}).to_string(), false).is_err());
    // Showing a tab is not an op, so a read-only document needs no exception for it.
    doc.set_open_tab(&first_tab).unwrap();
    assert_eq!(doc.tabs_info().active.as_deref(), Some(first_tab.as_str()));
    ops(&mut doc, json!({"op":"update_fields","open":true}));
    // The update before printing or downloading changes nothing a person wrote: let through, and nothing to undo.
    let could_undo = doc.can_undo();
    ops(&mut doc, json!({"op":"update_fields","quiet":true}));
    assert_eq!(doc.can_undo(), could_undo);
    // Update fields asked for by the person (F9) is an edit: refused.
    assert!(doc.apply_json(&json!({"op":"update_fields"}).to_string(), false).is_err());
}

#[test]
fn only_the_listed_ops_are_view_only() {
    assert!(serde_json::from_value::<docxtor_core::Op>(json!({"op":"tab","action":"open","id":"x"})).is_err(), "opening a tab is not an op");
    let on_open: docxtor_core::Op = serde_json::from_value(json!({"op":"update_fields","open":true})).unwrap();
    assert!(on_open.is_view_only());
    let print: docxtor_core::Op = serde_json::from_value(json!({"op":"update_fields","quiet":true})).unwrap();
    assert!(print.is_view_only());
    let f9: docxtor_core::Op = serde_json::from_value(json!({"op":"update_fields"})).unwrap();
    assert!(!f9.is_view_only());
    let typing: docxtor_core::Op = serde_json::from_value(json!({"op":"insert_text","at":{"p":1,"o":0},"text":"x"})).unwrap();
    assert!(!typing.is_view_only());
}
