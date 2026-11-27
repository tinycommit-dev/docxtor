//! The note numbers and list labels a keystroke's render draws come from what each block holds (`document/facts.rs`),
//! not from walking the whole document. After every step of a mixed session (typing, notes of every kind, lists,
//! sections, a changed list style and note format, undo and redo) what the editing document has drawn on the page is
//! what a copy opened from its saved file draws. In tests the engine also compares every result built from the kept
//! facts with the one built by walking, so a block that changed unmarked panics at once, naming the result.

use docxtor_core::ops::Op;
use docxtor_core::render::RBlock;
use docxtor_core::{perf, Document};
use serde_json::{json, Value};
use std::collections::HashMap;

fn op(v: Value) -> Op {
    serde_json::from_value(v).expect("an op")
}

/// What is drawn: the visible blocks in order, without the ids a reopened file hands out afresh.
fn strip(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for k in ["id", "p", "chunk"] {
                m.remove(k);
            }
            m.values_mut().for_each(strip);
        }
        Value::Array(a) => a.iter_mut().for_each(strip),
        _ => {}
    }
}

fn notes_of(doc: &Document) -> Value {
    let ps = doc.page_setup();
    let mut v = json!({ "notes": serde_json::to_value(&ps.notes).unwrap(), "endnotes": serde_json::to_value(&ps.endnotes).unwrap() });
    strip(&mut v);
    v
}

struct Screen {
    blocks: HashMap<u32, Value>,
    order: Vec<u32>,
}

impl Screen {
    fn new(doc: &mut Document) -> Screen {
        let mut s = Screen { blocks: HashMap::new(), order: Vec::new() };
        let out = doc.render_all();
        s.take(serde_json::to_value(&out).unwrap());
        s
    }
    /// Take in what a render sent, as the editor does: the changed blocks replace the ones it has.
    fn take(&mut self, out: Value) {
        for b in out["blocks"].as_array().unwrap() {
            self.blocks.insert(b["id"].as_u64().unwrap() as u32, b.clone());
        }
        // Not sent when it did not move: the screen keeps the one it has.
        if out["orderKept"] != json!(true) {
            self.order = out["order"].as_array().map(|a| a.iter().map(|i| i.as_u64().unwrap() as u32).collect()).unwrap_or_default();
        }
    }
    fn drawn(&self) -> Vec<Value> {
        self.order
            .iter()
            .map(|i| {
                let mut b = self.blocks.get(i).unwrap_or_else(|| panic!("block {i} is in the order but was never drawn")).clone();
                strip(&mut b);
                b
            })
            .collect()
    }
}

/// Apply one step, draw what changed, and compare with a copy opened from the saved file.
fn step(doc: &mut Document, screen: &mut Screen, what: &str, f: impl FnOnce(&mut Document)) {
    f(doc);
    let out = doc.render_changed();
    screen.take(serde_json::to_value(&out).unwrap());
    check(doc, screen, what);
}

fn check(doc: &mut Document, screen: &Screen, what: &str) {
    let mut fresh = Document::open(&doc.save().unwrap()).unwrap();
    let want: Vec<Value> = fresh
        .render_all()
        .blocks
        .iter()
        .map(|b| {
            let mut v = serde_json::to_value(b).unwrap();
            strip(&mut v);
            v
        })
        .collect();
    let got = screen.drawn();
    if got != want {
        let at = got.iter().zip(&want).position(|(a, b)| a != b);
        panic!("after `{what}` the page differs from a reopened copy ({} blocks drawn, {} reopened); first difference at {at:?}:\n  drawn:    {:?}\n  reopened: {:?}", got.len(), want.len(), at.map(|i| &got[i]), at.map(|i| &want[i]));
    }
    assert_eq!(notes_of(doc), notes_of(&fresh), "after `{what}` the notes differ from a reopened copy");
}

fn ids(doc: &mut Document) -> Vec<u32> {
    doc.render_all().order
}

fn run(doc: &mut Document, v: Value, merge: bool) {
    doc.apply(&op(v.clone()), merge).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn session() -> (Document, Screen) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let text = (0..60)
        .map(|i| match i {
            5..=8 => format!("- bullet {i}"),
            12..=15 => format!("1. step {i}"),
            30..=32 => format!("1. other list {i}"),
            _ => format!("Paragraph {i}: the quick brown fox jumps over the lazy dog."),
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    run(&mut doc, json!({"op": "insert_markdown", "before": first, "markdown": text}), false);
    run(&mut doc, json!({"op": "delete_blocks", "ids": [first]}), false);
    let screen = Screen::new(&mut doc);
    (doc, screen)
}

#[test]
fn what_a_keystroke_draws_is_what_a_reopened_copy_draws() {
    let (mut doc, mut screen) = session();
    let p = ids(&mut doc);
    check(&mut doc, &screen, "the start");
    assert!(serde_json::to_string(&screen.drawn()).unwrap().contains("\"list\""), "the document has lists");

    // Typing in plain paragraphs, in list items, and before and after where notes will be.
    for at in [3usize, 6, 13, 20, 31] {
        let id = p[at];
        step(&mut doc, &mut screen, "typing", |d| {
            run(d, json!({"op": "insert_text", "at": {"p": id, "o": 0}, "text": "x"}), false);
            for i in 1..4 {
                run(d, json!({"op": "insert_text", "at": {"p": id, "o": i}, "text": "y"}), true);
            }
        });
    }

    // Notes of every kind: a footnote, a later one, one before both (everything after it moves), an endnote, a custom mark.
    step(&mut doc, &mut screen, "footnote", |d| run(d, json!({"op": "insert_footnote", "at": {"p": p[10], "o": 5}, "text": "First note"}), false));
    step(&mut doc, &mut screen, "footnote later", |d| run(d, json!({"op": "insert_footnote", "at": {"p": p[40], "o": 5}, "text": "Later note"}), false));
    step(&mut doc, &mut screen, "footnote earlier", |d| run(d, json!({"op": "insert_footnote", "at": {"p": p[2], "o": 5}, "text": "Earlier note"}), false));
    step(&mut doc, &mut screen, "endnote", |d| run(d, json!({"op": "insert_endnote", "at": {"p": p[25], "o": 3}, "text": "An endnote"}), false));
    step(&mut doc, &mut screen, "custom note", |d| run(d, json!({"op": "insert_custom_note", "at": {"p": p[18], "o": 2}, "kind": "footnote", "mark": "*", "text": "Starred"}), false));
    step(&mut doc, &mut screen, "footnote in a list item", |d| run(d, json!({"op": "insert_footnote", "at": {"p": p[7], "o": 3}, "text": "In a list"}), false));
    // Typing in a paragraph that holds a note, and just before and after it.
    for o in [0usize, 5, 20] {
        step(&mut doc, &mut screen, "typing around a note", |d| run(d, json!({"op": "insert_text", "at": {"p": p[10], "o": o}, "text": "z"}), false));
    }

    // Lists: a new one, a level change, a restart, a list given back, a different style, a split and a join.
    step(&mut doc, &mut screen, "new numbered list", |d| run(d, json!({"op": "toggle_list", "ids": [p[45], p[46], p[47]], "kind": "decimal"}), false));
    step(&mut doc, &mut screen, "indent an item", |d| run(d, json!({"op": "indent", "ids": [p[46]], "delta": 1}), false));
    step(&mut doc, &mut screen, "restart numbering", |d| run(d, json!({"op": "restart_numbering", "id": p[14], "start": 1}), false));
    step(&mut doc, &mut screen, "list item out", |d| run(d, json!({"op": "toggle_list", "ids": [p[13]], "kind": "decimal"}), false));
    step(&mut doc, &mut screen, "list item back in", |d| run(d, json!({"op": "toggle_list", "ids": [p[13]], "kind": "decimal"}), false));
    step(&mut doc, &mut screen, "split a list item", |d| run(d, json!({"op": "split_paragraph", "at": {"p": p[31], "o": 9}}), false));
    step(&mut doc, &mut screen, "join list items", |d| run(d, json!({"op": "delete", "from": {"p": p[30], "o": 12}, "to": {"p": p[31], "o": 0}}), false));
    step(&mut doc, &mut screen, "list style", |d| run(d, json!({"op": "set_list_style", "ids": [p[12]], "style": "roman"}), false));
    step(&mut doc, &mut screen, "bullets to numbers", |d| run(d, json!({"op": "set_list_style", "ids": [p[5]], "style": "decimal"}), false));
    step(&mut doc, &mut screen, "typing in a list item", |d| {
        for i in 0..5 {
            run(d, json!({"op": "insert_text", "at": {"p": p[14], "o": i}, "text": "k"}), i > 0);
        }
    });

    // Typing inside a note (a story: the section and style keys do not move, the lists and numbers stay as they are).
    step(&mut doc, &mut screen, "typing inside a note", |d| {
        let note = d.page_setup().notes.iter().find_map(|n| n.blocks.iter().find_map(|b| if let RBlock::Para(p) = b { Some(p.id) } else { None })).expect("a note");
        run(d, json!({"op": "insert_text", "at": {"p": note, "o": 0}, "text": "n"}), false);
    });

    // Blocks come and go and move with no block of the lists or notes changing: the reorder.
    step(&mut doc, &mut screen, "delete a list item", |d| run(d, json!({"op": "delete_blocks", "ids": [p[14]]}), false));
    step(&mut doc, &mut screen, "move a note paragraph past another note", |d| run(d, json!({"op": "move_blocks", "ids": [p[10]], "after": p[42]}), false));
    step(&mut doc, &mut screen, "move a list item past another list", |d| run(d, json!({"op": "move_blocks", "ids": [p[6]], "after": p[44]}), false));
    step(&mut doc, &mut screen, "move a list item back", |d| run(d, json!({"op": "move_blocks", "ids": [p[6]], "after": p[5]}), false));

    // Notes taken away: the text around one, the paragraph that holds one.
    step(&mut doc, &mut screen, "delete across a note", |d| run(d, json!({"op": "delete", "from": {"p": p[2], "o": 3}, "to": {"p": p[2], "o": 12}}), false));
    step(&mut doc, &mut screen, "delete a paragraph with a note", |d| run(d, json!({"op": "delete_blocks", "ids": [p[40]]}), false));
    step(&mut doc, &mut screen, "delete a paragraph with a custom note", |d| run(d, json!({"op": "delete_blocks", "ids": [p[18]]}), false));

    // Note format, and sections: a break, notes that start again in each section, a second break.
    step(&mut doc, &mut screen, "footnote format", |d| run(d, json!({"op": "set_note_format", "kind": "footnote", "format": "lowerRoman"}), false));
    step(&mut doc, &mut screen, "section break", |d| run(d, json!({"op": "insert_break", "at": {"p": p[8], "o": 4}, "kind": "next_page"}), false));
    step(&mut doc, &mut screen, "notes start again in each section", |d| run(d, json!({"op": "set_note_props", "kind": "footnote", "restart": "eachSect"}), false));
    step(&mut doc, &mut screen, "footnote in the new section", |d| run(d, json!({"op": "insert_footnote", "at": {"p": p[35], "o": 2}, "text": "Another"}), false));
    step(&mut doc, &mut screen, "second break", |d| run(d, json!({"op": "insert_break", "at": {"p": p[50], "o": 4}, "kind": "continuous"}), false));
    step(&mut doc, &mut screen, "footnote format back", |d| run(d, json!({"op": "set_note_format", "kind": "footnote", "format": "decimal"}), false));
    step(&mut doc, &mut screen, "typing in the last section", |d| run(d, json!({"op": "insert_text", "at": {"p": p[55], "o": 0}, "text": "q"}), false));

    // A style that a list's paragraphs use changes.
    step(&mut doc, &mut screen, "a style", |d| run(d, json!({"op": "set_style_definition", "def": {"name": "Docxtor facts style", "kind": "paragraph"}}), false));
    step(&mut doc, &mut screen, "heading", |d| run(d, json!({"op": "set_style", "ids": [p[12]], "style": "Heading1"}), false));

    // Undo every step, checking after each, then redo them all.
    let mut undone = 0;
    while doc.undo() {
        undone += 1;
        let out = doc.render_changed();
        screen.take(serde_json::to_value(&out).unwrap());
        check(&mut doc, &screen, &format!("undo {undone}"));
        if undone >= 60 {
            break;
        }
    }
    assert!(undone >= 20, "{undone} steps undone");
    let mut redone = 0;
    while doc.redo() {
        redone += 1;
        let out = doc.render_changed();
        screen.take(serde_json::to_value(&out).unwrap());
        check(&mut doc, &screen, &format!("redo {redone}"));
    }
    assert_eq!(redone, undone);
}

#[test]
fn a_keystroke_walks_no_paragraph_of_the_document() {
    let mut seen = Vec::new();
    let mut in_note = Vec::new();
    for n in [200usize, 2000] {
        let mut doc = Document::new_blank();
        let first = doc.render_all().order[0];
        let text = (0..n)
            .map(|i| if i % 50 == 10 { format!("1. item {i}") } else { format!("Paragraph {i}: the quick brown fox jumps over the lazy dog.") })
            .collect::<Vec<_>>()
            .join("\n\n");
        run(&mut doc, json!({"op": "insert_markdown", "before": first, "markdown": text}), false);
        run(&mut doc, json!({"op": "delete_blocks", "ids": [first]}), false);
        let third = doc.render_all().order[3];
        run(&mut doc, json!({"op": "insert_footnote", "at": {"p": third, "o": 4}, "text": "A note"}), false);
        let order = doc.render_all().order;
        let mid = order[order.len() / 2];
        run(&mut doc, json!({"op": "insert_text", "at": {"p": mid, "o": 0}, "text": "x"}), false);
        doc.render_changed();
        let before = perf::paragraphs_walked();
        let keys = 20;
        for i in 1..=keys {
            run(&mut doc, json!({"op": "insert_text", "at": {"p": mid, "o": i}, "text": "x"}), true);
            doc.render_changed();
        }
        let per_key = (perf::paragraphs_walked() - before) as f64 / keys as f64;
        assert!(per_key < 20.0, "a keystroke in {n} paragraphs walked {per_key} paragraphs");
        seen.push(per_key);
        // Typing inside a note changes a story, not the lists or the note numbers: no more reading of the document.
        let note = doc.page_setup().notes.iter().find_map(|n| n.blocks.iter().find_map(|b| if let RBlock::Para(p) = b { Some(p.id) } else { None })).expect("a note");
        run(&mut doc, json!({"op": "insert_text", "at": {"p": note, "o": 0}, "text": "x"}), false);
        doc.render_changed();
        let before = perf::paragraphs_walked();
        for i in 1..=keys {
            run(&mut doc, json!({"op": "insert_text", "at": {"p": note, "o": i}, "text": "x"}), true);
            doc.render_changed();
        }
        in_note.push((perf::paragraphs_walked() - before) as f64 / keys as f64);
    }
    assert_eq!(in_note[0], in_note[1], "paragraphs walked per keystroke inside a note: {in_note:?} in 200 and 2,000 paragraphs");
    // A handful of paragraphs (the one typed in), whatever the length of the document.
    assert_eq!(seen[0], seen[1], "paragraphs walked per keystroke: {seen:?} in 200 and 2,000 paragraphs");
}

/// A keystroke does not send the list of blocks again (it grows by 4,900 characters per 1,000 paragraphs); an edit that
/// adds, removes or moves a block does, and so does a full render. A client that keeps the last list it was given is
/// then always right.
#[test]
fn the_order_is_sent_only_when_it_moved() {
    let (mut doc, mut screen) = session();
    let p = ids(&mut doc);
    // Typing: kept, nothing in the JSON.
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[3], "o": 0}, "text": "x"}), false);
    let out = doc.render_changed();
    assert!(out.order_kept && out.order.is_empty(), "typing keeps the order");
    let json = serde_json::to_value(&out).unwrap();
    assert!(json.get("order").is_none() && json["orderKept"] == json!(true), "the JSON says so: {json}");
    // A paragraph split adds a block: the whole order again.
    run(&mut doc, json!({"op": "split_paragraph", "at": {"p": p[3], "o": 1}}), false);
    let out = doc.render_changed();
    assert!(!out.order_kept && out.order.len() == p.len() + 1, "a new block sends the order");
    screen.take(serde_json::to_value(&out).unwrap());
    // Deleting the last block: the list is shorter, and a client could not tell that from "kept".
    let last = *screen.order.last().unwrap();
    run(&mut doc, json!({"op": "delete_blocks", "ids": [last]}), false);
    let out = doc.render_changed();
    assert!(!out.order_kept && out.order.len() == p.len(), "a block gone at the end sends the order");
    screen.take(serde_json::to_value(&out).unwrap());
    // Typing again: kept again, and the screen's list is still the document's.
    run(&mut doc, json!({"op": "insert_text", "at": {"p": p[4], "o": 0}, "text": "z"}), false);
    let out = doc.render_changed();
    assert!(out.order_kept);
    screen.take(serde_json::to_value(&out).unwrap());
    assert_eq!(screen.order, doc.render_snapshot().order);
    // Undo brings the old order back: sent, because it differs from the last one.
    assert!(doc.undo());
    assert!(doc.undo());
    let out = doc.render_changed();
    screen.take(serde_json::to_value(&out).unwrap());
    assert_eq!(screen.order, doc.render_snapshot().order);
    // A full render always has it.
    assert!(!doc.render_all().order.is_empty());
}
