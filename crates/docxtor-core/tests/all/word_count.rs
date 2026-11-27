//! The status bar's word count is kept per top-level block (`WordFacts` in `document/facts.rs`) and counts again only the
//! blocks an edit marked. After every step it equals a count made from scratch by walking the whole body, through typing,
//! splits and joins, paste, a table, moved and deleted blocks, document tabs and undo and redo (in tests the engine also
//! compares the kept total with a walk on every call, so a block that changed unmarked panics at once). Asking after a
//! key walks the same few paragraphs at every length.

use docxtor_core::ops::Op;
use docxtor_core::{perf, Document};
use serde_json::{json, Value};

fn op(v: Value) -> Op {
    serde_json::from_value(v).expect("an op")
}

fn run(doc: &mut Document, v: Value, merge: bool) {
    doc.apply(&op(v.clone()), merge).unwrap_or_else(|e| panic!("{e}: {v}"));
}

/// The count a fresh walk gives: every paragraph's text split on white space.
fn fresh(doc: &Document) -> usize {
    docxtor_core::diff::paragraph_texts(doc).iter().map(|t| t.split_whitespace().count()).sum()
}

fn same(doc: &mut Document, what: &str) -> usize {
    let n = doc.word_count();
    assert_eq!(n, fresh(doc), "after `{what}` the kept count differs from a fresh one");
    n
}

fn paras(doc: &mut Document) -> Vec<u32> {
    doc.render_all().order
}

fn session(n: usize) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let text = (0..n).map(|i| format!("Paragraph {i}: the quick brown fox jumps over the lazy dog.")).collect::<Vec<_>>().join("\n\n");
    run(&mut doc, json!({"op": "insert_markdown", "before": first, "markdown": text}), false);
    run(&mut doc, json!({"op": "delete_blocks", "ids": [first]}), false);
    doc
}

#[test]
fn the_kept_count_is_the_fresh_count_after_every_kind_of_edit() {
    let mut doc = session(60);
    assert_eq!(same(&mut doc, "the start"), 60 * 11);
    let p = paras(&mut doc);

    // Typing: a word, white space only, and a word joined onto another.
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[3], "o": 0}, "text": "two words "}), false);
    assert_eq!(same(&mut doc, "typing a word"), 60 * 11 + 2);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[3], "o": 0}, "text": "   "}), false);
    same(&mut doc, "typing spaces");
    for i in 0..4 {
        run(&mut doc, json!({"op": "insert_text", "at": {"p": p[4], "o": i}, "text": "k"}), i > 0);
        same(&mut doc, "typing a key");
    }

    // Split and join.
    run(&mut doc, json!({"op": "split_paragraph", "at": {"p": p[10], "o": 9}}), false);
    same(&mut doc, "a split");
    run(&mut doc, json!({"op": "split_paragraph", "at": {"p": p[11], "o": 0}}), false);
    same(&mut doc, "a split at the start");
    run(&mut doc, json!({"op": "delete", "from": {"p": p[20], "o": 12}, "to": {"p": p[21], "o": 3}}), false);
    same(&mut doc, "a join");

    // Paste, a table and its cells, text in a note.
    run(&mut doc, json!({"op": "paste_markdown", "at": {"p": p[30], "o": 5}, "markdown": "pasted *words* here"}), false);
    same(&mut doc, "an inline paste");
    run(&mut doc, json!({"op": "paste_markdown", "at": {"p": p[31], "o": 0}, "markdown": "# A heading\n\nOne more paragraph of text.\n\n- item a\n- item b"}), false);
    same(&mut doc, "a paste of blocks");
    run(&mut doc, json!({"op": "insert_table", "after": p[40], "rows": 3, "cols": 3}), false);
    same(&mut doc, "a table");
    let mut cells = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |q| {
        if !p.contains(&q.id) {
            cells.push(q.id);
        }
    });
    assert!(cells.len() >= 9, "{} cell paragraphs", cells.len());
    for (i, c) in cells.iter().enumerate().take(4) {
        run(&mut doc, json!({"op": "insert_text", "at": {"p": *c, "o": 0}, "text": format!("cell {i} text")}), false);
        same(&mut doc, "text in a cell");
    }
    run(&mut doc, json!({"op": "insert_footnote", "at": {"p": p[12], "o": 5}, "text": "A note with five words"}), false);
    same(&mut doc, "a footnote");
    let note = doc.page_setup().notes.iter().find_map(|n| n.blocks.iter().find_map(|b| if let docxtor_core::render::RBlock::Para(q) = b { Some(q.id) } else { None })).expect("a note");
    run(&mut doc, json!({"op": "insert_text", "at": {"p": note, "o": 0}, "text": "more words "}), false);
    same(&mut doc, "typing inside a note");

    // Blocks move and go.
    run(&mut doc, json!({"op": "move_blocks", "ids": [p[5]], "after": p[50]}), false);
    same(&mut doc, "a move");
    run(&mut doc, json!({"op": "move_blocks", "ids": [p[50], p[51]], "after": p[2]}), false);
    same(&mut doc, "a move back");
    run(&mut doc, json!({"op": "delete_blocks", "ids": [p[6], p[7], p[8]]}), false);
    same(&mut doc, "blocks deleted");
    // A whole stretch of the document, as deleting a section does.
    let ids: Vec<u32> = p[22..29].to_vec();
    run(&mut doc, json!({"op": "delete_blocks", "ids": ids}), false);
    same(&mut doc, "a stretch deleted");

    // Undo every step, counting after each, then redo them all.
    let mut undone = 0;
    while doc.undo() {
        undone += 1;
        same(&mut doc, &format!("undo {undone}"));
        if undone >= 60 {
            break;
        }
    }
    assert!(undone >= 15, "{undone} steps undone");
    let mut redone = 0;
    while doc.redo() {
        redone += 1;
        same(&mut doc, &format!("redo {redone}"));
    }
    assert_eq!(redone, undone);
    // Asking twice with nothing changed in between gives the same.
    let n = same(&mut doc, "the end");
    assert_eq!(doc.word_count(), n);
}

#[test]
fn the_kept_count_follows_document_tabs() {
    let mut doc = session(12);
    let before = same(&mut doc, "the start");
    run(&mut doc, json!({"op": "tab", "action": "add", "name": "Details", "first_name": "Overview"}), false);
    same(&mut doc, "a tab added");
    let tabs = doc.tabs_info().tabs;
    let second = tabs.iter().find(|t| t.name == "Details").expect("the new tab").id.clone();
    let order = paras(&mut doc);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": *order.last().unwrap(), "o": 0}, "text": "words in the second tab "}), false);
    same(&mut doc, "typing in the second tab");
    run(&mut doc, json!({"op": "tab", "action": "duplicate", "id": second}), false);
    same(&mut doc, "a tab duplicated");
    run(&mut doc, json!({"op": "tab", "action": "delete", "id": second}), false);
    same(&mut doc, "a tab deleted");
    // The four steps after the start, undone.
    for _ in 0..4 {
        assert!(doc.undo());
        same(&mut doc, "an undo");
    }
    assert_eq!(same(&mut doc, "all undone"), before);
}

#[test]
fn a_document_that_opens_with_text_counts_what_a_fresh_walk_counts() {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rich.docx")).expect("fixtures/rich.docx");
    let mut doc = Document::open(&bytes).unwrap();
    let n = same(&mut doc, "open");
    assert!(n > 20, "{n} words");
    let order = paras(&mut doc);
    run(&mut doc, json!({"op": "insert_text", "at": {"p": order[1], "o": 0}, "text": "a b c "}), false);
    assert_eq!(same(&mut doc, "typing"), n + 3);
}

/// Asking for the count after a key counts the block typed in and no other, whatever the length of the document.
#[test]
fn asking_after_a_key_walks_the_same_paragraphs_at_every_length() {
    let mut seen = Vec::new();
    for n in [200usize, 2000] {
        let mut doc = session(n);
        let order = paras(&mut doc);
        let mid = order[order.len() / 2];
        run(&mut doc, json!({"op": "insert_text", "at": {"p": mid, "o": 0}, "text": "x"}), false);
        doc.word_count();
        let keys = 20;
        let mut walked = 0;
        for i in 1..=keys {
            run(&mut doc, json!({"op": "insert_text", "at": {"p": mid, "o": i}, "text": "x"}), true);
            doc.render_changed();
            let before = perf::paragraphs_walked();
            doc.word_count();
            walked += perf::paragraphs_walked() - before;
        }
        seen.push(walked);
    }
    assert_eq!(seen[0], seen[1], "paragraphs walked by 20 keys and their counts: {seen:?} in 200 and 2,000 paragraphs");
    assert_eq!(seen[0], 20, "one paragraph counted per key");
    assert!(seen[0] <= 20 * 4, "{} paragraphs walked", seen[0]);
}
