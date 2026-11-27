//! Smart chips are plain content controls with a tag: a person or file chip is a rich text control around a hyperlink,
//! a date chip a date picker, a drop-down chip a drop-down list. Word shows the text; Docxtor draws the chip.

use docxtor_core::model::{paragraph_order, walk_paragraphs};
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

fn ops(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn saved(doc: &mut Document) -> (Vec<u8>, String) {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    (bytes, xml)
}

fn text_of(doc: &Document, id: u32) -> String {
    let mut t = String::new();
    walk_paragraphs(&doc.body, &mut |p| {
        if p.id == id {
            t = p.plain_text();
        }
    });
    t
}

fn first_para(doc: &Document) -> u32 {
    paragraph_order(&doc.body)[0]
}

/// What the editor does for a person chip: write the name, link it, wrap it in a tagged rich text control.
fn person_chip(doc: &mut Document, p: u32, at: usize, name: &str, email: &str) {
    ops(
        doc,
        json!([
            {"op":"insert_text","at":{"p":p,"o":at},"text":name},
            {"op":"set_link","from":{"p":p,"o":at},"to":{"p":p,"o":at + name.len()},"url":format!("mailto:{email}")},
            {"op":"control","action":"insert","at":{"p":p,"o":at},"to":{"p":p,"o":at + name.len()},"kind":"richText","props":{"alias":"Person","tag":"docxtor:chip:person"}}
        ]),
    );
}

#[test]
fn a_person_chip_is_a_tagged_control_around_a_mailto_link() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Ask  today"}));
    person_chip(&mut doc, p, 4, "Ada Lovelace", "ada@example.com");
    assert_eq!(text_of(&doc, p), "Ask Ada Lovelace today");
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<w:tag w:val="docxtor:chip:person"/>"#), "{xml}");
    assert!(xml.contains("<w:richText/>"), "{xml}");
    // The link is inside the control's content, so Word shows a linked name.
    let sdt = &xml[xml.find("<w:sdt>").unwrap()..xml.find("</w:sdt>").unwrap()];
    assert!(sdt.contains("<w:hyperlink") && sdt.contains("Ada Lovelace"), "{sdt}");
    let rels = String::from_utf8(Package::open(&bytes).unwrap().get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    assert!(rels.contains("mailto:ada@example.com") && rels.contains(r#"TargetMode="External""#), "{rels}");

    // It opens again as the same control, and the link is still on the text.
    let mut re = Document::open(&bytes).unwrap();
    let all = re.list_controls();
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].1.kind.as_str(), all[0].1.tag.as_deref(), all[0].1.alias.as_deref()), ("richText", Some("docxtor:chip:person"), Some("Person")));
    assert_eq!((all[0].1.start, all[0].1.end, all[0].2.as_str()), (4, 16, "Ada Lovelace"));
    assert_eq!(saved(&mut re).1, xml);

    // Undo takes the whole chip back out in one step.
    assert!(doc.undo());
    assert_eq!(text_of(&doc, p), "Ask  today");
    assert!(doc.list_controls().is_empty());
}

#[test]
fn a_chip_can_be_turned_back_into_text_or_deleted_with_its_content() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    person_chip(&mut doc, p, 0, "Ada", "ada@example.com");
    ops(&mut doc, json!({"op":"control","action":"remove","target":{"p":p,"n":0}}));
    assert!(doc.list_controls().is_empty());
    assert_eq!(text_of(&doc, p), "Ada");
    assert!(saved(&mut doc).1.contains("<w:hyperlink"));
    ops(&mut doc, json!({"op":"set_link","from":{"p":p,"o":0},"to":{"p":p,"o":3},"url":null}));
    assert!(!saved(&mut doc).1.contains("<w:hyperlink"));

    let mut gone = Document::new_blank();
    let q = first_para(&gone);
    person_chip(&mut gone, q, 0, "Grace", "grace@example.com");
    ops(&mut gone, json!({"op":"control","action":"remove","target":{"p":q,"n":0},"delete_content":true}));
    assert!(gone.list_controls().is_empty());
    assert_eq!(text_of(&gone, q), "");
}

#[test]
fn a_date_chip_is_a_date_picker_that_shows_the_date_in_its_format() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Due "}));
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":4},"kind":"date","props":{"alias":"Date","tag":"docxtor:chip:date","dateFormat":"MMM d, yyyy","placeholder":"Pick a date"}}));
    ops(&mut doc, json!({"op":"control","action":"set","target":{"p":p,"n":0},"date":"2026-10-14"}));
    assert_eq!(text_of(&doc, p), "Due Oct 14, 2026");
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"w:fullDate="2026-10-14T00:00:00Z""#) && xml.contains(r#"<w:tag w:val="docxtor:chip:date"/>"#), "{xml}");
    assert!(xml.contains(r#"<w:dateFormat w:val="MMM d, yyyy"/>"#), "{xml}");
    assert!(!xml.contains("showingPlcHdr"));
    let re = Document::open(&bytes).unwrap();
    let all = re.list_controls();
    assert_eq!((all[0].1.kind.as_str(), all[0].1.full_date.as_deref()), ("date", Some("2026-10-14")));
    // Changing the format writes the same day again.
    ops(&mut doc, json!({"op":"control","action":"props","target":{"p":p,"n":0},"props":{"dateFormat":"dddd, MMMM d, yyyy"}}));
    assert_eq!(text_of(&doc, p), "Due Wednesday, October 14, 2026");
    // Undo goes back through the format, the date and the insert.
    assert!(doc.undo());
    assert_eq!(text_of(&doc, p), "Due Oct 14, 2026");
}

#[test]
fn a_drop_down_chip_keeps_each_choices_colour_in_the_value_and_shows_only_the_text() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    let items = json!([
        {"display":"Not started","value":"gray:Not started"},
        {"display":"In progress","value":"blue:In progress"},
        {"display":"Blocked","value":"red:Blocked"},
        {"display":"Done","value":"green:Done"}
    ]);
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":0},"kind":"dropDown","props":{"alias":"Status","tag":"docxtor:chip:dropdown","placeholder":"Choose a status","items":items}}));
    // The first entry after the prompt is chosen: the placeholder goes.
    ops(&mut doc, json!({"op":"control","action":"set","target":{"p":p,"n":0},"index":1}));
    assert_eq!(text_of(&doc, p), "Not started");
    ops(&mut doc, json!({"op":"control","action":"set","target":{"p":p,"n":0},"index":4}));
    assert_eq!(text_of(&doc, p), "Done");
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"w:displayText="Done" w:value="green:Done""#) || xml.contains(r#"w:value="green:Done""#), "{xml}");
    assert!(!xml.contains("showingPlcHdr"));
    let re = Document::open(&bytes).unwrap();
    let c = &re.list_controls()[0].1;
    assert_eq!(c.kind, "dropDown");
    assert_eq!(c.items.len(), 5);
    assert_eq!((c.items[4].display.as_str(), c.items[4].value.as_str()), ("Done", "green:Done"));
    // Changing the entries keeps the prompt first, as the editor sends it.
    ops(
        &mut doc,
        json!({"op":"control","action":"props","target":{"p":p,"n":0},"props":{"items":[
            {"display":"Choose a status","value":"Choose a status"},
            {"display":"Done","value":"green:Done"},
            {"display":"Later","value":"yellow:Later"}
        ]}}),
    );
    assert_eq!(doc.list_controls()[0].1.items.len(), 3);
    assert_eq!(text_of(&doc, p), "Done");
}

#[test]
fn a_file_chip_links_to_an_address_and_a_chip_survives_inside_a_table_cell() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"See Q3 plan"}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":p,"o":4},"to":{"p":p,"o":11},"url":"https://docxtor.example/#/library/Q3%20plan.docx"}));
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":4},"to":{"p":p,"o":11},"kind":"richText","props":{"alias":"File","tag":"docxtor:chip:file"}}));
    ops(&mut doc, json!({"op":"insert_table","after":p,"rows":1,"cols":2}));
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains("docxtor:chip:file") && xml.contains("<w:tbl>"));
    let re = Document::open(&bytes).unwrap();
    assert_eq!(re.list_controls()[0].1.tag.as_deref(), Some("docxtor:chip:file"));
}
