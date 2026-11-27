//! Page numbers in a table of contents and in PAGEREF fields use the format the page prints
//! (i, ii, iii in a roman-numbered section), not plain 1, 2, 3.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn find_para(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

/// The paragraph whose whole text is `text` (a heading, not its entry in the table of contents).
fn exact(doc: &Document, text: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if p.plain_text() == text {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph says {text:?}"))
}

fn pages(list: &[(u32, u32)]) -> Value {
    Value::Object(list.iter().map(|(p, n)| (p.to_string(), json!(n))).collect())
}

/// A preface in a lower-roman section, a chapter in a decimal one that restarts at 1, and a table of contents.
fn book() -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"# Preface\n\nWelcome.\n\n# Chapter\n\nText."}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    let chapter = find_para(&doc, "Chapter");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":chapter,"o":0},"kind":"next_page"}));
    let (preface, chapter) = (find_para(&doc, "Preface"), find_para(&doc, "Chapter"));
    ops(&mut doc, json!({"op":"set_page_numbering","section":preface,"format":"lowerRoman"}));
    ops(&mut doc, json!({"op":"set_page_numbering","section":chapter,"start":1}));
    doc
}

#[test]
fn toc_shows_roman_page_numbers_where_the_page_does() {
    let mut doc = book();
    let preface = find_para(&doc, "Preface");
    let chapter = find_para(&doc, "Chapter");
    // The layout numbers the preface page 2 (a title page came first) and the chapter 1 again.
    let pg = pages(&[(preface, 2), (chapter, 1)]);
    ops(&mut doc, json!({"op":"insert_toc","before":preface,"title":"Contents","pages":pg}));
    let text = docxtor_core::export::to_text(&doc);
    assert!(text.contains("Preface\tii"), "{text}");
    assert!(text.contains("Chapter\t1"), "{text}");
    // Updating follows a new layout.
    let (heading, chapter) = (exact(&doc, "Preface"), exact(&doc, "Chapter"));
    ops(&mut doc, json!({"op":"update_toc","pages":pages(&[(heading, 4), (chapter, 3)])}));
    let text = docxtor_core::export::to_text(&doc);
    assert!(text.contains("Preface\tiv") && text.contains("Chapter\t3"), "{text}");
    // The saved field results carry the roman numeral, and the file stays valid.
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains("<w:t>iv</w:t>"), "{xml}");
}

#[test]
fn cross_references_and_update_fields_use_the_page_format() {
    let mut doc = book();
    let preface = find_para(&doc, "Preface");
    let welcome = find_para(&doc, "Welcome");
    let chapter = find_para(&doc, "Chapter");
    let text = find_para(&doc, "Text.");
    let r = doc
        .apply_json(&json!({"op":"insert_cross_reference","at":{"p":text,"o":5},"target":{"type":"heading","p":preface},"show":"page","pages":pages(&[(preface, 3), (chapter, 1)])}).to_string(), false)
        .unwrap();
    assert!(r[0].caret.is_some());
    let all = docxtor_core::export::to_text(&doc);
    assert!(all.contains("Text.iii"), "{all}");
    // Update fields with a new layout: the PAGEREF follows, still as a numeral.
    ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(preface, 4), (welcome, 4), (chapter, 1), (text, 1)])}));
    let all = docxtor_core::export::to_text(&doc);
    assert!(all.contains("Text.iv"), "{all}");
    // A reference to something in the decimal section stays decimal.
    let r = doc
        .apply_json(&json!({"op":"insert_cross_reference","at":{"p":text,"o":0},"target":{"type":"heading","p":chapter},"show":"page","pages":pages(&[(preface, 3), (chapter, 7)])}).to_string(), false)
        .unwrap();
    assert!(r[0].caret.is_some());
    assert!(docxtor_core::export::to_text(&doc).contains("7Text."), "{}", docxtor_core::export::to_text(&doc));
}
