//! Page numbers from Insert, Page numbers: PAGE and NUMPAGES fields in the header or footer (or a frame in the margin),
//! replacing the ones already there, and taking them out again.

use docxtor_core::render::RBlock;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn saved(doc: &mut Document, name: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

/// The text of the paragraphs of the first section's header or footer, field results included.
fn story_text(doc: &Document, headers: bool, variant: &str) -> Vec<String> {
    let ps = doc.page_setup();
    let s = &ps.sections[0];
    let map = if headers { &s.headers } else { &s.footers };
    let Some(part) = map.get(variant) else { return vec![] };
    ps.parts[part]
        .blocks
        .iter()
        .map(|b| match b {
            RBlock::Para(p) => p.segs.iter().filter_map(|s| s.x.clone()).collect::<String>(),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn page_numbers_are_written_with_fields_and_replace_each_other() {
    let mut doc = doc_with("Alpha text");
    ops(&mut doc, json!({"op":"insert_page_number","place":"bottom","align":"right"}));
    assert_eq!(story_text(&doc, false, "default"), ["1"]);
    let footer = saved(&mut doc, "word/footer1.xml");
    assert!(footer.contains(" PAGE ") && footer.contains(r#"<w:jc w:val="right"/>"#), "{footer}");
    // Page X of Y, in the document's words, in the header.
    ops(&mut doc, json!({"op":"insert_page_number","place":"top","style":"x_of_y","page_word":"Página","of_word":"de"}));
    assert_eq!(story_text(&doc, true, "default"), ["Página 1 de 1"]);
    let header = saved(&mut doc, "word/header1.xml");
    assert!(header.contains(" PAGE ") && header.contains(" NUMPAGES "), "{header}");
    // A second number takes the first one's place.
    ops(&mut doc, json!({"op":"insert_page_number","place":"top","align":"left"}));
    assert_eq!(story_text(&doc, true, "default"), ["1"]);
    // A page number in the margin is a frame.
    ops(&mut doc, json!({"op":"insert_page_number","place":"margin_right"}));
    let header = saved(&mut doc, "word/header1.xml");
    // Centred in the margin: the 720-twip frame starts a quarter inch into it, not at the paper's edge.
    assert!(header.contains("w:framePr") && header.contains(r#"w:hAnchor="page""#) && header.contains(r#"w:w="720""#) && !header.contains("w:xAlign"), "{header}");
    assert!(header.contains(r#"w:jc w:val="center""#), "{header}");
    let x_of = |h: &str| h.split(r#"w:x=""#).nth(1).and_then(|s| s.split('"').next()).and_then(|s| s.parse::<i64>().ok()).unwrap();
    assert_eq!(x_of(&header), 12240 - 1440 + (1440 - 720) / 2, "{header}");
    ops(&mut doc, json!({"op":"insert_page_number","place":"margin_left"}));
    let header = saved(&mut doc, "word/header1.xml");
    assert_eq!(x_of(&header), (1440 - 720) / 2, "{header}");
    ops(&mut doc, json!({"op":"set_page_setup","margins":[1440,500,1440,500]}));
    ops(&mut doc, json!({"op":"insert_page_number","place":"margin_right"}));
    let header = saved(&mut doc, "word/header1.xml");
    assert!(header.contains(r#"w:w="500""#) && x_of(&header) == 12240 - 500, "{header}");
    assert!(err(&mut doc, json!({"op":"insert_page_number","place":"side"})).contains("margin_left"));
    assert!(err(&mut doc, json!({"op":"insert_page_number","place":"top","style":"fancy"})).contains("x_of_y"));
}

#[test]
fn page_numbers_come_out_of_every_header_and_footer() {
    let mut doc = doc_with("Alpha text");
    ops(&mut doc, json!({"op":"insert_page_number","place":"bottom"}));
    ops(&mut doc, json!({"op":"insert_page_number","place":"top","style":"x_of_y"}));
    let r = ops(&mut doc, json!({"op":"remove_page_numbers"}));
    assert_eq!(r[0].count, Some(2));
    assert_eq!(story_text(&doc, true, "default"), [""]);
    assert_eq!(story_text(&doc, false, "default"), [""]);
    saved(&mut doc, "word/footer1.xml");
    assert!(doc.undo());
    assert_eq!(story_text(&doc, false, "default"), ["1"]);
    assert!(doc.redo());
    assert!(err(&mut doc, json!({"op":"remove_page_numbers"})).contains("no page numbers"));
}

#[test]
fn a_footer_line_with_other_text_is_never_taken_for_a_page_number() {
    let mut doc = doc_with("Alpha text");
    let made = ops(&mut doc, json!({"op":"insert_page_number","place":"bottom"}));
    let pid = made[0].created[0];
    // "Acme Ltd, Page 1 - Draft": the number is in it, but so is the person's own text.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":pid,"o":0},"text":"Acme Ltd, Page "}));
    let len = "Acme Ltd, Page 1".chars().count();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":pid,"o":len},"text":" - Draft"}));
    assert_eq!(story_text(&doc, false, "default"), ["Acme Ltd, Page 1 - Draft"]);
    assert!(err(&mut doc, json!({"op":"remove_page_numbers"})).contains("no page numbers"));
    assert_eq!(story_text(&doc, false, "default"), ["Acme Ltd, Page 1 - Draft"]);
    // Putting a number in from the gallery leaves the line alone too.
    ops(&mut doc, json!({"op":"insert_page_number","place":"bottom","align":"right"}));
    assert_eq!(story_text(&doc, false, "default"), ["Acme Ltd, Page 1 - Draft", "1"]);
    let r = ops(&mut doc, json!({"op":"remove_page_numbers"}));
    assert_eq!(r[0].count, Some(1));
    assert_eq!(story_text(&doc, false, "default"), ["Acme Ltd, Page 1 - Draft"]);
    // The gallery's own Page X of Y is still recognised, in whatever words it was written.
    ops(&mut doc, json!({"op":"insert_page_number","place":"top","style":"x_of_y","page_word":"Seite","of_word":"von"}));
    assert_eq!(story_text(&doc, true, "default"), ["Seite 1 von 1"]);
    let r = ops(&mut doc, json!({"op":"remove_page_numbers"}));
    assert_eq!(r[0].count, Some(1));
}
