//! Printing a selection: a copy of the document that holds only the selected text, with the page setup, styles and
//! headers of the original. The open document stays as it was.

use docxtor_core::model::{walk_paragraphs, Block};
use docxtor_core::ops::Pos;
use docxtor_core::{package::Package, Document};
use serde_json::json;

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(
        &json!({"op": "insert_markdown", "before": first, "markdown": md}).to_string(),
        false,
    )
    .unwrap();
    doc.apply_json(
        &json!({"op": "delete_blocks", "ids": [first]}).to_string(),
        false,
    )
    .unwrap();
    doc
}

fn pid(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph has {needle:?}"))
}

fn texts(bytes: &[u8]) -> Vec<String> {
    docxtor_core::diff::paragraph_texts(&Document::open(bytes).unwrap())
}

fn assert_valid(bytes: &[u8]) {
    for part in &Package::open(bytes).unwrap().parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc =
                docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc)
                .unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

#[test]
fn only_the_selected_text_is_in_the_copy_and_the_document_is_untouched() {
    let doc = doc_with("# Title\n\nFirst paragraph here.\n\nSecond paragraph here.\n\nThird paragraph here.\n\nFourth.");
    let before = docxtor_core::diff::paragraph_texts(&doc);
    // From the middle of the first paragraph to the middle of the third.
    let (a, b) = (pid(&doc, "First"), pid(&doc, "Third"));
    let out = doc.selection_copy(Pos::new(a, 6), Pos::new(b, 5)).unwrap();
    assert_valid(&out);
    assert_eq!(
        texts(&out),
        vec!["paragraph here.", "Second paragraph here.", "Third"]
    );
    assert_eq!(docxtor_core::diff::paragraph_texts(&doc), before);
}

#[test]
fn a_selection_inside_one_paragraph_is_just_that_text() {
    let doc = doc_with("Alpha beta gamma\n\nDelta");
    let a = pid(&doc, "Alpha");
    let out = doc.selection_copy(Pos::new(a, 6), Pos::new(a, 10)).unwrap();
    assert_eq!(texts(&out), vec!["beta"]);
    // A caret (nothing selected) leaves an empty page, not an error.
    let none = doc.selection_copy(Pos::new(a, 3), Pos::new(a, 3)).unwrap();
    assert_eq!(texts(&none), vec![""]);
}

#[test]
fn a_selection_that_touches_a_table_keeps_the_table_whole() {
    let doc = doc_with("Before\n\n| a | b |\n|---|---|\n| c | d |\n\nAfter\n\nLast");
    let (a, b) = (pid(&doc, "Before"), pid(&doc, "After"));
    let out = Document::open(&doc.selection_copy(Pos::new(a, 0), Pos::new(b, 5)).unwrap()).unwrap();
    let kinds: Vec<&str> = out
        .body
        .iter()
        .map(|x| match x {
            Block::Para(_) => "p",
            Block::Table(_) => "t",
            _ => "o",
        })
        .collect();
    assert_eq!(kinds.first(), Some(&"p"));
    assert_eq!(kinds.iter().filter(|k| **k == "t").count(), 1);
    assert_eq!(kinds.last(), Some(&"o"));
    let words: Vec<String> = docxtor_core::diff::paragraph_texts(&out)
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect();
    assert_eq!(words, vec!["Before", "a", "b", "c", "d", "After"]);
}

#[test]
fn the_copy_is_set_up_for_the_paper_the_selection_is_on() {
    let mut doc = doc_with("One\n\nTwo\n\nThree");
    let two = pid(&doc, "Two");
    // Landscape letter for the whole document.
    doc.apply_json(
        &json!({"op": "set_page_setup", "width": 15840, "height": 12240}).to_string(),
        false,
    )
    .unwrap();
    let out = doc
        .selection_copy(Pos::new(two, 0), Pos::new(two, 3))
        .unwrap();
    assert_valid(&out);
    let copy = Document::open(&out).unwrap();
    assert_eq!(docxtor_core::diff::paragraph_texts(&copy), vec!["Two"]);
    let setup = copy.page_setup();
    assert_eq!((setup.width, setup.height), (15840, 12240));
    // The body still ends with its section properties.
    assert!(matches!(copy.body.last(), Some(Block::Opaque(o)) if o.el.name == "w:sectPr"));
}

#[test]
fn the_section_the_selection_ends_in_gives_the_copy_its_page_size() {
    let mut doc = doc_with("One\n\nTwo\n\nThree");
    let (one, two) = (pid(&doc, "One"), pid(&doc, "Two"));
    // A next-page section break after "One": the first section is landscape, the rest portrait.
    doc.apply_json(
        &json!({"op": "insert_break", "at": {"p": one, "o": 3}, "kind": "next_page"}).to_string(),
        false,
    )
    .unwrap();
    doc.apply_json(
        &json!({"op": "set_page_setup", "section": one, "width": 15840, "height": 12240})
            .to_string(),
        false,
    )
    .unwrap();
    let from_first = Document::open(
        &doc.selection_copy(Pos::new(one, 0), Pos::new(one, 3))
            .unwrap(),
    )
    .unwrap();
    let from_last = Document::open(
        &doc.selection_copy(Pos::new(two, 0), Pos::new(two, 3))
            .unwrap(),
    )
    .unwrap();
    let size = |d: &mut Document| {
        let s = d.page_setup();
        (s.width, s.height)
    };
    let (mut a, mut b) = (from_first, from_last);
    assert_eq!(size(&mut a), (15840, 12240));
    assert_eq!(size(&mut b), (12240, 15840));
}

#[test]
fn headers_come_along() {
    let mut doc = doc_with("# Heading\n\nBody text");
    doc.apply_json(
        &json!({"op": "add_header_footer", "kind": "header"}).to_string(),
        false,
    )
    .unwrap();
    let h = pid(&doc, "Heading");
    let out = doc.selection_copy(Pos::new(h, 0), Pos::new(h, 7)).unwrap();
    assert_valid(&out);
    let pkg = Package::open(&out).unwrap();
    assert!(
        pkg.parts.iter().any(|p| p.name.contains("header")),
        "the header part is in the copy"
    );
    assert_eq!(texts(&out), vec!["Heading"]);
}
