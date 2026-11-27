//! The undo list: each step carries the name of what it did, and a chosen point can be undone to in one go.

use docxtor_core::Document;
use serde_json::{json, Value};

fn run(doc: &mut Document, ops: Value, merge: bool) {
    doc.apply_json(&ops.to_string(), merge)
        .unwrap_or_else(|e| panic!("{e}: {ops}"));
}

fn doc_with_text(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    run(
        &mut doc,
        json!({"op": "insert_markdown", "before": first, "markdown": text}),
        false,
    );
    run(
        &mut doc,
        json!({"op": "delete_blocks", "ids": [first]}),
        false,
    );
    let id = doc.render_all().order[0];
    (doc, id)
}

#[test]
fn steps_are_named_after_their_first_op() {
    let (mut doc, id) = doc_with_text("Hello");
    assert_eq!(doc.undo_labels(), vec!["insert_markdown", "delete_blocks"]);
    run(
        &mut doc,
        json!([{"op": "set_doc_props", "title": "T"}, {"op": "set_language", "lang": "fr-FR"}]),
        false,
    );
    run(
        &mut doc,
        json!({"op": "insert_text", "at": {"p": id, "o": 5}, "text": "!"}),
        false,
    );
    // Typing that joins the step before it keeps that step's name and adds no step.
    run(
        &mut doc,
        json!({"op": "insert_text", "at": {"p": id, "o": 6}, "text": "!"}),
        true,
    );
    assert_eq!(
        doc.undo_labels(),
        vec![
            "insert_markdown",
            "delete_blocks",
            "set_doc_props",
            "insert_text"
        ]
    );
    assert!(doc.redo_labels().is_empty());
}

#[test]
fn undo_moves_the_names_to_the_redo_list_and_redo_brings_them_back() {
    let (mut doc, id) = doc_with_text("Hello");
    run(
        &mut doc,
        json!({"op": "insert_text", "at": {"p": id, "o": 5}, "text": "!"}),
        false,
    );
    run(
        &mut doc,
        json!({"op": "set_doc_props", "title": "T"}),
        false,
    );
    assert!(doc.undo());
    assert_eq!(
        doc.undo_labels().last().map(String::as_str),
        Some("insert_text")
    );
    assert_eq!(doc.redo_labels(), vec!["set_doc_props"]);
    assert!(doc.undo());
    assert_eq!(doc.redo_labels(), vec!["insert_text", "set_doc_props"]);
    assert!(doc.redo());
    assert_eq!(doc.redo_labels(), vec!["set_doc_props"]);
    assert_eq!(
        doc.undo_labels().last().map(String::as_str),
        Some("insert_text")
    );
    // A new edit ends the redo list.
    run(
        &mut doc,
        json!({"op": "set_doc_props", "title": "U"}),
        false,
    );
    assert!(doc.redo_labels().is_empty());
}

#[test]
fn undoing_to_a_chosen_point_takes_back_every_step_after_it() {
    let (mut doc, id) = doc_with_text("Hello");
    for (o, c) in [(5, "a"), (6, "b"), (7, "c")] {
        run(
            &mut doc,
            json!({"op": "insert_text", "at": {"p": id, "o": o}, "text": c}),
            false,
        );
    }
    let text = |d: &Document| docxtor_core::diff::paragraph_texts(d).join("\n");
    assert!(text(&doc).contains("Helloabc"));
    assert!(doc.undo_steps(2));
    assert!(text(&doc).contains("Helloa") && !text(&doc).contains("Helloab"));
    assert_eq!(doc.redo_labels().len(), 2);
    assert!(doc.redo_steps(2));
    assert!(text(&doc).contains("Helloabc"));
    // Asking for more steps than there are stops at the start; nothing left to undo gives false.
    assert!(doc.undo_steps(99));
    assert!(doc.undo_labels().is_empty() && !doc.can_undo() && !doc.undo_steps(1));
    assert_eq!(doc.redo_labels().len(), 5);
}

#[test]
fn an_op_is_named_from_its_variant_the_way_it_is_named_on_the_wire() {
    use docxtor_core::Op;
    // The name comes from a match, not from encoding the op; these keep the two in step (names of several words, ops with
    // flattened fields, and one that carries a lot of text).
    let pos = json!({"p": 1, "o": 0});
    let samples = [
        json!({"op": "insert_text", "at": pos, "text": "x"}),
        json!({"op": "delete", "from": pos, "to": pos}),
        json!({"op": "split_paragraph", "at": pos}),
        json!({"op": "set_doc_props", "title": "T"}),
        json!({"op": "set_language", "lang": "fr-FR"}),
        json!({"op": "move_object", "at": pos}),
        json!({"op": "paste_html", "at": pos, "html": "<p>x</p>".repeat(1000)}),
        json!({"op": "insert_markdown", "before": 1, "markdown": "x"}),
        json!({"op": "delete_blocks", "ids": [1]}),
    ];
    for v in samples {
        let op: Op = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"));
        assert_eq!(op.name(), v["op"].as_str().unwrap());
    }
}
