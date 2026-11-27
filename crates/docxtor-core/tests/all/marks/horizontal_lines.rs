//! Tests of: Horizontal lines.

use super::*;

const LINE: &str = r#"<w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="auto"/></w:pBdr></w:pPr>"#;

pub(super) fn is_line(doc: &Document, id: u32) -> bool {
    let xml = para_xml(doc, id);
    xml.contains(LINE) && !xml.contains("pStyle") && !xml.contains("<w:r>")
}

#[test]
fn a_line_in_the_middle_splits_the_paragraph() {
    let mut doc = doc_with("# Heading\n\nAlpha beta gamma\n\nLast");
    let a = find_p(&doc, "Alpha");
    let before = doc.can_undo();
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":a,"o":5}}).to_string(), false).unwrap();
    assert!(before || doc.can_undo());
    assert_eq!(texts(&doc), ["Heading", "Alpha", "", " beta gamma", "Last"]);
    let ids = docxtor_core::model::paragraph_order(&doc.body);
    let line = ids[2];
    assert!(is_line(&doc, line), "{}", para_xml(&doc, line));
    // The caret is at the start of the paragraph after the line.
    let c = r[0].caret.unwrap();
    assert_eq!((c.p.0, c.o), (ids[3], 0));
    assert_eq!(r[0].created.len(), 2);
    // The render marks the line for drawing.
    let rendered = doc.render_snapshot();
    let hr: Vec<bool> = rendered.blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { Some(p.hr) } else { None }).collect();
    assert_eq!(hr, [false, false, true, false, false]);
    // One undo step puts everything back.
    assert!(doc.undo());
    assert_eq!(texts(&doc), ["Heading", "Alpha beta gamma", "Last"]);
    assert!(doc.redo());
    assert_eq!(texts(&doc).len(), 5);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_line_at_the_start_or_end_of_a_paragraph_goes_beside_it() {
    let mut doc = doc_with("Alpha\n\nBeta\n\nGamma");
    let (a, b) = (find_p(&doc, "Alpha"), find_p(&doc, "Beta"));
    // Start of a non-empty paragraph: before it; the caret stays at its start.
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":a,"o":0}}).to_string(), false).unwrap();
    let ids = docxtor_core::model::paragraph_order(&doc.body);
    assert!(is_line(&doc, ids[0]));
    assert_eq!(r[0].caret.unwrap().p.0, a);
    assert_eq!(texts(&doc), ["", "Alpha", "Beta", "Gamma"]);
    // End: after it, before the next paragraph.
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":b,"o":4}}).to_string(), false).unwrap();
    let ids = docxtor_core::model::paragraph_order(&doc.body);
    assert_eq!(texts(&doc), ["", "Alpha", "Beta", "", "Gamma"]);
    assert!(is_line(&doc, ids[3]));
    assert_eq!(r[0].caret.unwrap().p.0, ids[4]);
    assert_eq!(r[0].created, [ids[3]], "no extra paragraph when one follows");
    assert!(doc.undo());
    assert_eq!(texts(&doc), ["", "Alpha", "Beta", "Gamma"]);
}

#[test]
fn a_line_in_an_empty_paragraph_replaces_it_and_leaves_lists_and_headings() {
    let mut doc = doc_with("Intro\n\n- one\n- two\n\nOutro");
    let two = find_p(&doc, "two");
    // Heading style, list numbering and alignment don't carry onto the line.
    ops(&mut doc, json!({"op":"set_style","ids":[two],"style":"Heading 2"}));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":two,"o":3},"text":"!"}));
    ops(&mut doc, json!({"op":"delete","from":{"p":two,"o":0},"to":{"p":two,"o":4}}));
    assert_eq!(texts(&doc)[2], "");
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":two,"o":0}}).to_string(), false).unwrap();
    assert!(is_line(&doc, two), "{}", para_xml(&doc, two));
    assert_eq!(texts(&doc).len(), 4, "the empty paragraph became the line");
    assert!(r[0].created.is_empty());
    assert_eq!(r[0].caret.unwrap().p.0, find_p(&doc, "Outro"));
    // Under a heading, at its end, the line is Normal: no style, no number.
    let mut doc = doc_with("# Title\n\nBody");
    let t = find_p(&doc, "Title");
    ops(&mut doc, json!({"op":"insert_horizontal_line","at":{"p":t,"o":5}}));
    let line = docxtor_core::model::paragraph_order(&doc.body)[1];
    assert!(is_line(&doc, line));
    assert_eq!(doc.format_at(Pos::new(line, 0)).unwrap().style, None);
    assert_eq!(doc.format_at(Pos::new(line, 0)).unwrap().list, None);
}

#[test]
fn a_line_at_the_end_of_the_document_gets_a_paragraph_after_it() {
    let mut doc = doc_with("Alpha\n\nOmega");
    let o = find_p(&doc, "Omega");
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":o,"o":5}}).to_string(), false).unwrap();
    let ids = docxtor_core::model::paragraph_order(&doc.body);
    assert_eq!(ids.len(), 4);
    assert!(is_line(&doc, ids[2]));
    assert_eq!(r[0].caret.unwrap().p.0, ids[3]);
    assert_eq!(texts(&doc)[3], "");
    assert_eq!(r[0].created.len(), 2);
    assert!(doc.undo());
    assert_eq!(texts(&doc), ["Alpha", "Omega"]);
    // In an empty last paragraph it becomes the line and a new empty one follows.
    let mut doc = Document::new_blank();
    let p = doc.render_all().order[0];
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":p,"o":0}}).to_string(), false).unwrap();
    let ids = docxtor_core::model::paragraph_order(&doc.body);
    assert_eq!(ids.len(), 2);
    assert!(is_line(&doc, ids[0]));
    assert_eq!(r[0].caret.unwrap().p.0, ids[1]);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert_eq!(Document::open(&bytes).unwrap().render_snapshot().blocks.len(), 2);
}

#[test]
fn a_line_works_in_table_cells_and_headers() {
    let mut doc = doc_with("Intro");
    let p = find_p(&doc, "Intro");
    ops(&mut doc, json!({"op":"insert_table","after":p,"rows":1,"cols":2,"data":[["Cell one","Cell two"]]}));
    let c1 = find_p(&doc, "Cell one");
    // At the end of the last paragraph of a cell, a paragraph follows the line inside the cell.
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":c1,"o":8}}).to_string(), false).unwrap();
    let c = r[0].caret.unwrap().p.0;
    assert_ne!(c, find_p(&doc, "Cell two"), "the caret stays in the cell");
    let RBlock::Table(t) = doc.render_snapshot().blocks.iter().find(|b| matches!(b, RBlock::Table(_))).cloned().unwrap() else { panic!() };
    let cell = &t.rows[0].cells[0];
    assert_eq!(cell.blocks.len(), 3);
    assert!(matches!(&cell.blocks[1], RBlock::Para(p) if p.hr));
    assert!(matches!(&cell.blocks[2], RBlock::Para(p) if p.id == c && p.len == 0));
    assert_valid(&doc.save().unwrap());
    // In a header.
    let r = doc.apply_json(&json!({"op":"add_header_footer","kind":"header"}).to_string(), false).unwrap();
    let h = r[0].caret.unwrap().p.0;
    ops(&mut doc, json!({"op":"insert_text","at":{"p":h,"o":0},"text":"Head"}));
    let r = doc.apply_json(&json!({"op":"insert_horizontal_line","at":{"p":h,"o":4}}).to_string(), false).unwrap();
    assert_eq!(r[0].created.len(), 2);
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains(LINE) && header.contains("Head"), "{header}");
}

#[test]
fn a_line_after_a_section_break_stays_in_its_section() {
    let mut doc = doc_with("Alpha\n\nBravo");
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":a,"o":5},"kind":"next_page"}));
    assert_eq!(doc.page_setup().sections.len(), 2);
    let end = doc.page_setup().sections[0].end.unwrap();
    ops(&mut doc, json!({"op":"insert_horizontal_line","at":{"p":end,"o":5}}));
    let ps = doc.page_setup();
    let line = ps.sections[0].end.unwrap();
    assert_ne!(line, end, "the section now ends at the line");
    assert!(para_xml(&doc, line).contains("sectPr") && !para_xml(&doc, end).contains("sectPr"));
}
