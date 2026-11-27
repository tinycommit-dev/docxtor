//! The outline: levels of the blocks of the main text, promoting and demoting (`set_outline_level`) and moving
//! blocks (`move_blocks`), with undo and a valid file.

use docxtor_core::model::{find_block, write_block};
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn docx(body: &str, extra_styles: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    if !extra_styles.is_empty() {
        let s = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/styles.xml", s.replace("</w:styles>", &format!("{extra_styles}</w:styles>")).into_bytes());
    }
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn para(style: Option<&str>, text: &str) -> String {
    let ppr = style.map(|s| format!(r#"<w:pPr><w:pStyle w:val="{s}"/></w:pPr>"#)).unwrap_or_default();
    format!(r#"<w:p>{ppr}<w:r><w:t>{text}</w:t></w:r></w:p>"#)
}

/// (level, text) of every block, in order.
fn levels(doc: &Document) -> Vec<(u8, String)> {
    doc.outline().into_iter().map(|e| (e.level, e.text)).collect()
}

fn ids(doc: &Document) -> Vec<u32> {
    doc.outline().iter().map(|e| e.id).collect()
}

fn texts(doc: &Document) -> Vec<String> {
    doc.outline().into_iter().map(|e| e.text).collect()
}

fn sample() -> Document {
    docx(
        &[
            para(Some("Heading1"), "One"),
            para(None, "body of one"),
            para(Some("Heading2"), "One A"),
            para(None, "body of one a"),
            para(Some("Heading1"), "Two"),
            para(None, "body of two"),
        ]
        .concat(),
        "",
    )
}

#[test]
fn outline_lists_blocks_with_levels() {
    let doc = sample();
    assert_eq!(
        levels(&doc),
        vec![(1, "One".into()), (0, "body of one".into()), (2, "One A".into()), (0, "body of one a".into()), (1, "Two".into()), (0, "body of two".into())]
    );
    assert!(doc.outline().iter().all(|e| e.kind == "p"));
}

#[test]
fn outline_sees_direct_levels_and_tables() {
    let doc = docx(
        r#"<w:p><w:pPr><w:outlineLvl w:val="2"/></w:pPr><w:r><w:t>Deep</w:t></w:r></w:p><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>after</w:t></w:r></w:p>"#,
        "",
    );
    let o = doc.outline();
    assert_eq!((o[0].level, o[0].kind), (3, "p"));
    assert_eq!((o[1].level, o[1].kind), (0, "t"));
    assert_eq!(o[2].level, 0);
}

#[test]
fn promote_and_demote_switch_heading_styles() {
    let mut doc = sample();
    let id = ids(&doc);
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[2]],"delta":1}));
    assert_eq!(doc.outline()[2].level, 3);
    assert!(write_block(find_block(&doc.body, id[2]).unwrap()).to_xml().contains(r#"w:val="Heading3""#));
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[2]],"delta":-2}));
    assert_eq!(doc.outline()[2].level, 1);
    // Level 1 can't be promoted further.
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[0]],"delta":-1}));
    assert_eq!(doc.outline()[0].level, 1);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn headings_in_a_localized_file_change_level_by_style() {
    // A French Word file: the ids are Titre1.., the names are still "heading 1".., each with its outline level.
    let styles: String = (1..=3)
        .map(|n| format!(r#"<w:style w:type="paragraph" w:styleId="Titre{n}"><w:name w:val="heading {n}"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="{}"/></w:pPr></w:style>"#, n - 1))
        .collect();
    let mut doc = docx(&[para(Some("Titre1"), "Un"), para(None, "texte")].concat(), &styles);
    let id = ids(&doc)[0];
    assert_eq!(doc.outline()[0].level, 1);
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id],"delta":1}));
    assert_eq!(doc.outline()[0].level, 2);
    let xml = write_block(find_block(&doc.body, id).unwrap()).to_xml();
    assert!(xml.contains(r#"w:val="Titre2""#) && !xml.contains("outlineLvl"), "{xml}");
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id],"delta":-1}));
    let xml = write_block(find_block(&doc.body, id).unwrap()).to_xml();
    assert_eq!(doc.outline()[0].level, 1);
    assert!(xml.contains(r#"w:val="Titre1""#) && !xml.contains("outlineLvl"), "{xml}");
    // Taking it out of the outline puts the style back to body text, not a direct level on a heading style.
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id],"level":0}));
    assert_eq!(doc.outline()[0].level, 0);
    let xml = write_block(find_block(&doc.body, id).unwrap()).to_xml();
    assert!(!xml.contains("Titre1") && !xml.contains("outlineLvl"), "{xml}");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn demote_of_level_nine_and_body_text() {
    let mut doc = docx(&[para(Some("Heading9"), "Deepest"), para(None, "plain")].concat(), "");
    let id = ids(&doc);
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[0]],"delta":1}));
    assert_eq!(doc.outline()[0].level, 0);
    assert!(!write_block(find_block(&doc.body, id[0]).unwrap()).to_xml().contains("Heading9"));
    // Body text promotes to one level below the heading above it, or level 1 with none.
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[1]],"delta":-1}));
    assert_eq!(doc.outline()[1].level, 1);
    let mut doc = sample();
    let id = ids(&doc);
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[3]],"delta":-1}));
    assert_eq!(doc.outline()[3].level, 3);
    // Demoting body text leaves it alone.
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[5]],"delta":1}));
    assert_eq!(doc.outline()[5].level, 0);
}

#[test]
fn a_direct_level_changes_directly() {
    let mut doc = docx(r#"<w:p><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:r><w:t>Mine</w:t></w:r></w:p>"#, "");
    let id = ids(&doc)[0];
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id],"delta":1}));
    let xml = write_block(find_block(&doc.body, id).unwrap()).to_xml();
    assert!(xml.contains(r#"<w:outlineLvl w:val="2"/>"#) && !xml.contains("Heading"), "{xml}");
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id],"level":0}));
    assert_eq!(doc.outline()[0].level, 0);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn set_a_level_and_undo() {
    let mut doc = sample();
    let before = levels(&doc);
    let id = ids(&doc);
    ops(&mut doc, json!({"op":"set_outline_level","ids":[id[1], id[3]],"level":2}));
    assert_eq!(doc.outline()[1].level, 2);
    assert_eq!(doc.outline()[3].level, 2);
    assert!(doc.undo());
    assert_eq!(levels(&doc), before);
    assert!(doc.redo());
    assert_eq!(doc.outline()[1].level, 2);
}

#[test]
fn a_level_must_be_given_and_in_range() {
    let mut doc = sample();
    let id = ids(&doc)[0];
    assert!(doc.apply_json(&json!({"op":"set_outline_level","ids":[id]}).to_string(), false).is_err());
    assert!(doc.apply_json(&json!({"op":"set_outline_level","ids":[id],"level":10}).to_string(), false).is_err());
}

#[test]
fn move_a_section_down_and_undo() {
    let mut doc = sample();
    let before = texts(&doc);
    let id = ids(&doc);
    // "One" with everything under it (4 blocks) goes after the last block.
    ops(&mut doc, json!({"op":"move_blocks","ids":[id[0], id[1], id[2], id[3]],"after":id[5]}));
    assert_eq!(texts(&doc), vec!["Two", "body of two", "One", "body of one", "One A", "body of one a"]);
    assert_valid(&doc.save().unwrap());
    assert!(doc.undo());
    assert_eq!(texts(&doc), before);
    assert!(doc.redo());
    assert_eq!(texts(&doc)[0], "Two");
}

#[test]
fn move_before_a_block_keeps_the_order_given_by_the_document() {
    let mut doc = sample();
    let id = ids(&doc);
    // Ids listed out of order still keep document order.
    ops(&mut doc, json!({"op":"move_blocks","ids":[id[5], id[4]],"before":id[0]}));
    assert_eq!(texts(&doc)[..2], ["Two", "body of two"]);
    assert_eq!(texts(&doc).len(), 6);
}

#[test]
fn moves_that_make_no_sense_are_refused() {
    let mut doc = sample();
    let id = ids(&doc);
    let no = |doc: &mut Document, v: Value| assert!(doc.apply_json(&v.to_string(), false).is_err(), "{v}");
    no(&mut doc, json!({"op":"move_blocks","ids":[id[0]],"after":id[0]}));
    no(&mut doc, json!({"op":"move_blocks","ids":[id[0]]}));
    no(&mut doc, json!({"op":"move_blocks","ids":[],"after":id[0]}));
    no(&mut doc, json!({"op":"move_blocks","ids":[id[0]],"after":9999}));
    no(&mut doc, json!({"op":"move_blocks","ids":[9999],"after":id[0]}));
    // Nothing changed.
    assert_eq!(texts(&doc).len(), 6);
}

#[test]
fn moving_blocks_keeps_a_table_with_its_text() {
    let mut doc = docx(
        &[
            para(Some("Heading1"), "A"),
            r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#.to_string(),
            para(Some("Heading1"), "B"),
        ]
        .concat(),
        "",
    );
    let id = ids(&doc);
    ops(&mut doc, json!({"op":"move_blocks","ids":[id[0], id[1]],"after":id[2]}));
    let kinds: Vec<_> = doc.outline().iter().map(|e| e.kind).collect();
    assert_eq!(kinds, ["p", "p", "t"]);
    assert_eq!(doc.outline()[0].text, "B");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn section_ids_runs_to_the_next_heading_of_the_same_or_higher_level() {
    let doc = sample();
    let id = ids(&doc);
    assert_eq!(doc.section_ids(id[0]), id[0..4].to_vec());
    assert_eq!(doc.section_ids(id[2]), id[2..4].to_vec());
    assert_eq!(doc.section_ids(id[4]), id[4..6].to_vec());
    // Body text is just itself.
    assert_eq!(doc.section_ids(id[1]), vec![id[1]]);
    assert!(doc.section_ids(9999).is_empty());
}

#[test]
fn the_ai_tools_change_levels_and_move_sections() {
    let mut doc = sample();
    let id = ids(&doc);
    let out = docxtor_core::ai::call_tool(&mut doc, "set_heading_level", &json!({"ids":[id[0].to_string()],"delta":1,"with_subsections":true})).unwrap();
    assert!(out.contains("Changed the level of 2"), "{out}");
    assert_eq!(levels(&doc)[0].0, 2);
    assert_eq!(levels(&doc)[2].0, 3);
    // The body text under them stays body text.
    assert_eq!(levels(&doc)[1].0, 0);
    let out = docxtor_core::ai::call_tool(&mut doc, "move_section", &json!({"id":id[4].to_string(),"before":id[0].to_string()})).unwrap();
    assert!(out.contains("Moved 2"), "{out}");
    assert_eq!(texts(&doc)[0], "Two");
    assert!(docxtor_core::ai::call_tool(&mut doc, "move_section", &json!({"id":id[0].to_string(),"after":id[2].to_string()})).is_err());
    assert!(docxtor_core::ai::tool_definitions().to_string().contains("move_section"));
    assert_valid(&doc.save().unwrap());
}
