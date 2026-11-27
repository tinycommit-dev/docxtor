//! A save writes again only the top-level blocks an edit marked and copies the rest from the last `document.xml`
//! (`document/body_xml.rs`). After every kind of edit, the bytes a save wrote equal the body written whole, and typing
//! writes one block, whatever the length of the document. (In tests the engine also compares every save with the whole
//! body, so a block that changed without being marked panics at once.)

use docxtor_core::ops::Op;
use docxtor_core::package::Package;
use docxtor_core::{perf, Document};
use serde_json::{json, Value};

fn op(v: Value) -> Op {
    serde_json::from_value(v).expect("an op")
}

fn run(doc: &mut Document, v: Value) {
    doc.apply(&op(v.clone()), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

/// Save, and check the written part against the body written whole. Returns the saved file.
fn check(doc: &mut Document, what: &str) -> Vec<u8> {
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    let written = pkg.get("word/document.xml").unwrap();
    let whole = doc.document_xml_whole();
    assert!(written == whole.as_slice(), "after `{what}` the saved document.xml differs from the body written whole");
    // And it is a file that opens, with the same text.
    let back = Document::open(&bytes).unwrap_or_else(|e| panic!("after `{what}` the file does not open: {e}"));
    assert_eq!(docxtor_core::diff::paragraph_texts(&back), docxtor_core::diff::paragraph_texts(doc), "after `{what}` the text differs");
    bytes
}

fn paras(doc: &mut Document) -> Vec<u32> {
    doc.render_all().order
}

fn session(n: usize) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let text = (0..n).map(|i| format!("Paragraph {i}: the quick brown fox jumps over the lazy dog.")).collect::<Vec<_>>().join("\n\n");
    run(&mut doc, json!({"op": "insert_markdown", "before": first, "markdown": text}));
    run(&mut doc, json!({"op": "delete_blocks", "ids": [first]}));
    doc
}

fn fixture(name: &str) -> Document {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name)).expect("a fixture");
    Document::open(&bytes).unwrap()
}

#[test]
fn every_kind_of_edit_saves_what_the_whole_body_writes() {
    let mut doc = session(40);
    check(&mut doc, "the first save");
    let p = paras(&mut doc);

    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[3], "o": 0}, "text": "typed "}));
    check(&mut doc, "typing in a paragraph");
    run(&mut doc, json!({"op": "split_paragraph", "at": {"p": p[10], "o": 9}}));
    check(&mut doc, "a split");
    run(&mut doc, json!({"op": "insert_table", "after": p[20], "rows": 2, "cols": 2}));
    check(&mut doc, "a table");
    let mut cells = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |q| {
        if !p.contains(&q.id) {
            cells.push(q.id);
        }
    });
    run(&mut doc, json!({"op": "insert_text", "at": {"p": cells[0], "o": 0}, "text": "in a cell"}));
    check(&mut doc, "text in a table cell");
    run(&mut doc, json!({"op": "insert_footnote", "at": {"p": p[12], "o": 5}, "text": "A note"}));
    check(&mut doc, "a footnote");
    let note = doc.page_setup().notes.iter().find_map(|n| n.blocks.iter().find_map(|b| if let docxtor_core::render::RBlock::Para(q) = b { Some(q.id) } else { None })).expect("a note");
    run(&mut doc, json!({"op": "insert_text", "at": {"p": note, "o": 0}, "text": "more "}));
    check(&mut doc, "typing in a footnote");

    run(&mut doc, json!({"op": "move_blocks", "ids": [p[5]], "after": p[30]}));
    check(&mut doc, "a move");
    run(&mut doc, json!({"op": "delete_blocks", "ids": [p[6], p[7]]}));
    check(&mut doc, "blocks deleted");
    run(&mut doc, json!({"op": "insert_markdown", "after": p[1], "markdown": "# A heading\n\nA new paragraph.\n\n- a\n- b"}));
    check(&mut doc, "blocks inserted");

    // Page settings and the page colour: outside the blocks, or in the section properties.
    run(&mut doc, json!({"op": "set_page_color", "color": "FFF2CC"}));
    check(&mut doc, "a page colour");
    run(&mut doc, json!({"op": "set_page_setup", "width": 16838, "height": 11906, "margins": [720, 720, 720, 720]}));
    check(&mut doc, "a page setup");
    run(&mut doc, json!({"op": "set_page_color", "color": null}));
    check(&mut doc, "the page colour removed");

    // Undo and redo, saving between.
    let mut n = 0;
    while doc.undo() {
        n += 1;
        check(&mut doc, &format!("undo {n}"));
        if n >= 30 {
            break;
        }
    }
    assert!(n >= 8, "{n} steps undone");
    let mut m = 0;
    while doc.redo() {
        m += 1;
        check(&mut doc, &format!("redo {m}"));
    }
    assert_eq!(m, n);
}

#[test]
fn undo_then_redo_with_saves_between_each_step() {
    let mut doc = session(10);
    check(&mut doc, "start");
    let p = paras(&mut doc);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[2], "o": 0}, "text": "one "}));
    check(&mut doc, "edit");
    assert!(doc.undo());
    check(&mut doc, "undo");
    assert!(doc.redo());
    check(&mut doc, "redo");
    assert!(doc.undo());
    assert!(doc.redo());
    check(&mut doc, "undo and redo with no save between");
}

#[test]
fn w14_is_declared_while_a_block_uses_it_and_not_after() {
    let mut doc = session(20);
    let before = check(&mut doc, "no effects");
    let xml = |b: &[u8]| String::from_utf8(Package::open(b).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(!xml(&before).contains("xmlns:w14"), "nothing uses w14 yet");
    let p = paras(&mut doc);
    let at = |o| json!({"p": p[4], "o": o});
    run(&mut doc, json!({"op": "format_text", "from": at(0), "to": at(5), "format": {"ligatures": "standard"}}));
    let used = check(&mut doc, "a w14 use added");
    assert!(xml(&used).contains("xmlns:w14") && xml(&used).contains("<w14:ligatures"), "declared while used");
    // A second block uses it too; removing one use keeps the declaration, removing the last one drops it.
    let at2 = |o| json!({"p": p[8], "o": o});
    run(&mut doc, json!({"op": "format_text", "from": at2(0), "to": at2(5), "format": {"ligatures": "standard"}}));
    check(&mut doc, "a second use");
    run(&mut doc, json!({"op": "format_text", "from": at(0), "to": at(5), "format": {"ligatures": "default"}}));
    let one = check(&mut doc, "one use removed");
    assert!(xml(&one).contains("xmlns:w14"), "one use left");
    run(&mut doc, json!({"op": "format_text", "from": at2(0), "to": at2(5), "format": {"ligatures": "default"}}));
    let none = check(&mut doc, "the last use removed");
    assert!(!xml(&none).contains("xmlns:w14"), "no use left, no declaration");
}

#[test]
fn containers_and_chunks_in_fixtures_save_what_the_whole_body_writes() {
    for name in ["rich.docx", "complex.docx", "long.docx"] {
        let mut doc = fixture(name);
        // A file that opens and is saved untouched keeps its own bytes; the check starts with the first edit.
        // Every paragraph there is, in tables and containers too.
        let mut order = Vec::new();
        docxtor_core::model::walk_paragraphs(&doc.body, &mut |q| order.push(q.id));
        for (k, i) in [1usize, order.len() / 3, 2 * order.len() / 3, order.len() - 1].into_iter().enumerate() {
            run(&mut doc, json!({"op": "insert_text", "at": {"p": order[i], "o": 0}, "text": "x"}));
            check(&mut doc, &format!("{name}: a key in paragraph {i} ({k})"));
        }
        while doc.undo() {
            check(&mut doc, &format!("{name}: undo"));
        }
    }
}

#[test]
fn a_content_control_edit_saves_what_the_whole_body_writes() {
    let mut doc = session(6);
    check(&mut doc, "start");
    let p = paras(&mut doc);
    docxtor_core::ai::call_tool(&mut doc, "insert_content_control", &json!({"id": p[1], "kind": "date", "alias": "Due", "date_format": "yyyy-MM-dd"})).unwrap();
    check(&mut doc, "a content control");
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[1], "o": 3}, "text": "typed inside"}));
    check(&mut doc, "typing next to a control");
    docxtor_core::ai::call_tool(&mut doc, "insert_content_control", &json!({"id": p[3], "kind": "checkbox", "alias": "Agree"})).unwrap();
    check(&mut doc, "a second control");
}

#[test]
fn document_tabs_save_what_the_whole_body_writes() {
    let mut doc = session(12);
    check(&mut doc, "start");
    run(&mut doc, json!({"op": "tab", "action": "add", "name": "Details", "first_name": "Overview"}));
    check(&mut doc, "a tab added");
    let second = doc.tabs_info().tabs.iter().find(|t| t.name == "Details").expect("the new tab").id.clone();
    let order = paras(&mut doc);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": *order.last().unwrap(), "o": 0}, "text": "in the second tab "}));
    check(&mut doc, "typing in the second tab");
    run(&mut doc, json!({"op": "tab", "action": "duplicate", "id": second}));
    check(&mut doc, "a tab duplicated");
    run(&mut doc, json!({"op": "tab", "action": "delete", "id": second}));
    check(&mut doc, "a tab deleted");
    for _ in 0..4 {
        assert!(doc.undo());
        check(&mut doc, "an undo");
    }
}

/// One key writes one block and the statistics count one, at every length.
#[test]
fn one_key_writes_one_block_and_counts_one_paragraph_at_every_length() {
    let mut seen = Vec::new();
    for n in [200usize, 2000] {
        let mut doc = session(n);
        doc.stamp("2026-11-21T10:00:00Z");
        doc.save().unwrap();
        let order = paras(&mut doc);
        let mid = order[order.len() / 2];
        run(&mut doc, json!({"op": "insert_text", "at": {"p": mid, "o": 0}, "text": "x"}));
        doc.render_changed();
        let counted = perf::paragraphs_walked();
        doc.stamp("2026-11-21T10:00:01Z");
        let in_stamp = perf::paragraphs_walked() - counted;
        let written = perf::blocks_written_total();
        doc.save().unwrap();
        seen.push((in_stamp, perf::blocks_written_total() - written));
    }
    assert_eq!(seen[0], seen[1], "{seen:?}");
    assert_eq!(seen[0], (1, 1), "paragraphs counted by a stamp and blocks written by a save");
}

#[test]
fn the_statistics_stamped_are_the_whole_text_counts() {
    let mut doc = session(30);
    let p = paras(&mut doc);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[2], "o": 0}, "text": "a few more words "}));
    let counts = doc.main_counts();
    assert_eq!(counts, doc.text_stats().main);
    run(&mut doc, json!({"op": "delete_blocks", "ids": [p[4]]}));
    assert_eq!(doc.main_counts(), doc.text_stats().main);
    // Asking for the words and the counts in turn gives the same, whichever asks first.
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[5], "o": 0}, "text": "zz "}));
    let w = doc.word_count();
    assert_eq!(doc.main_counts().words, w);
    assert_eq!(doc.main_counts(), doc.text_stats().main);
}
