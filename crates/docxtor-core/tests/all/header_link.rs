//! "Link to previous" for headers and footers: unlinking copies, linking drops the copy, undo restores.

use docxtor_core::ai::call_tool;
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

/// Three sections (Alpha, Bravo, Charlie), the first with the header "Company".
fn three_sections() -> (Document, [u32; 3]) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"Alpha\n\nBravo\n\nCharlie"}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    for name in ["Bravo", "Charlie"] {
        let p = find_para(&doc, name);
        ops(&mut doc, json!({"op":"insert_break","at":{"p":p,"o":0},"kind":"next_page"}));
    }
    let ids = [find_para(&doc, "Alpha"), find_para(&doc, "Bravo"), find_para(&doc, "Charlie")];
    call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","text":"Company","section":format!("p{}", ids[0])})).unwrap();
    (doc, ids)
}

fn header_text(doc: &Document, part: &str) -> String {
    let s = doc.stories.iter().find(|s| s.part == part).unwrap_or_else(|| panic!("no story {part}"));
    let mut v = Vec::new();
    walk_paragraphs(&s.blocks, &mut |p| v.push(p.plain_text()));
    v.join("\n")
}

fn type_into(doc: &mut Document, part: &str, text: &str) {
    let s = doc.stories.iter().find(|s| s.part == part).unwrap();
    let p = paragraph_order(&s.blocks)[0];
    ops(doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":text}));
}

fn link(doc: &mut Document, kind: &str, section: u32, linked: bool) {
    ops(doc, json!({"op":"link_header_footer","kind":kind,"variant":"default","section":section,"linked":linked}));
}

fn header_parts(bytes: &[u8]) -> Vec<String> {
    let mut v: Vec<String> = Package::open(bytes).unwrap().parts.iter().map(|p| p.name.clone()).filter(|n| n.starts_with("word/header")).collect();
    v.sort();
    v
}

#[test]
fn later_sections_start_linked() {
    let (doc, _) = three_sections();
    let ps = doc.page_setup();
    assert!(ps.sections[0].linked_headers.is_empty(), "the first section has nothing to link to");
    assert!(ps.sections[1].linked_headers.contains(&"default".to_string()));
    assert!(ps.sections[2].linked_headers.contains(&"default".to_string()));
    assert!(ps.sections[1].linked_footers.contains(&"default".to_string()));
    assert_eq!(ps.sections[1].headers["default"], ps.sections[0].headers["default"]);
}

#[test]
fn unlinking_copies_and_the_copies_differ() {
    let (mut doc, ids) = three_sections();
    let shared = doc.page_setup().sections[0].headers["default"].clone();
    link(&mut doc, "header", ids[1], false);
    let ps = doc.page_setup();
    assert!(!ps.sections[1].linked_headers.contains(&"default".to_string()));
    let own = ps.sections[1].headers["default"].clone();
    assert_ne!(own, shared);
    // The copy starts as the same text, and section three follows section two now.
    assert_eq!(header_text(&doc, &own), "Company");
    assert_eq!(ps.sections[2].headers["default"], own);
    assert!(ps.sections[2].linked_headers.contains(&"default".to_string()));
    // Typing in the second section's header leaves the first alone.
    type_into(&mut doc, &own, "Chapter 2: ");
    assert_eq!(header_text(&doc, &own), "Chapter 2: Company");
    assert_eq!(header_text(&doc, &shared), "Company");
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let again = Document::open(&bytes).unwrap();
    let ps = again.page_setup();
    assert_eq!(header_text(&again, &ps.sections[0].headers["default"]), "Company");
    assert_eq!(header_text(&again, &ps.sections[1].headers["default"]), "Chapter 2: Company");
    assert!(!ps.sections[1].linked_headers.contains(&"default".to_string()));
    // Unlinking twice changes nothing.
    link(&mut doc, "header", ids[1], false);
    assert_eq!(doc.page_setup().sections[1].headers["default"], own);
}

#[test]
fn a_copy_leaves_the_bookmarks_with_the_original() {
    let (mut doc, ids) = three_sections();
    let shared = doc.page_setup().sections[0].headers["default"].clone();
    let head = paragraph_order(&doc.stories.iter().find(|s| s.part == shared).unwrap().blocks)[0];
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":head,"o":0},"to":{"p":head,"o":7},"name":"Masthead"}));
    link(&mut doc, "header", ids[1], false);
    let own = doc.page_setup().sections[1].headers["default"].clone();
    assert_ne!(own, shared);
    assert_eq!(header_text(&doc, &own), "Company");
    // One bookmark in the document, in the first header.
    let list = doc.bookmarks();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].story.as_ref().unwrap().part, shared);
    // The name is still the original's: the copy cannot take it.
    let copy_p = paragraph_order(&doc.stories.iter().find(|s| s.part == own).unwrap().blocks)[0];
    assert!(doc.apply_json(&json!({"op":"insert_bookmark","from":{"p":copy_p,"o":0},"to":{"p":copy_p,"o":3},"name":"Masthead"}).to_string(), false).is_err());
    assert_valid(&doc.save().unwrap());
}

#[test]
fn unlinking_keeps_the_relationships_of_the_copied_part() {
    let (mut doc, ids) = three_sections();
    let shared = doc.page_setup().sections[0].headers["default"].clone();
    // Give the first header a relationship (a link target), as a header with a hyperlink or picture has.
    let rels_path = docxtor_core::package::rels_path(&shared);
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/" TargetMode="External"/></Relationships>"#;
    doc.pkg.set(&rels_path, rels.as_bytes().to_vec());
    link(&mut doc, "header", ids[1], false);
    let own = doc.page_setup().sections[1].headers["default"].clone();
    let copied = doc.pkg.get(&docxtor_core::package::rels_path(&own)).expect("the copy has its own rels part");
    assert_eq!(copied, rels.as_bytes());
}

#[test]
fn linking_drops_the_copy_and_the_part() {
    let (mut doc, ids) = three_sections();
    let shared = doc.page_setup().sections[0].headers["default"].clone();
    link(&mut doc, "header", ids[1], false);
    let own = doc.page_setup().sections[1].headers["default"].clone();
    type_into(&mut doc, &own, "X");
    assert_eq!(header_parts(&doc.save().unwrap()).len(), 2);
    link(&mut doc, "header", ids[1], true);
    let ps = doc.page_setup();
    assert_eq!(ps.sections[1].headers["default"], shared);
    assert_eq!(ps.sections[2].headers["default"], shared);
    assert!(ps.sections[1].linked_headers.contains(&"default".to_string()));
    // The part nothing refers to any more is not in the saved file, nor its relationship or content type.
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert_eq!(header_parts(&bytes), vec![shared.clone()]);
    let pkg = Package::open(&bytes).unwrap();
    let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    let types = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    let name = own.trim_start_matches("word/");
    assert!(!rels.contains(name) && !types.contains(name), "{rels}\n{types}");
    let again = Document::open(&bytes).unwrap();
    assert_eq!(again.page_setup().sections[1].headers["default"], shared);
    // Linking an already linked section does nothing.
    link(&mut doc, "header", ids[1], true);
    assert_eq!(doc.page_setup().sections[1].headers["default"], shared);
}

#[test]
fn undo_and_redo_restore_the_link() {
    let (mut doc, ids) = three_sections();
    let shared = doc.page_setup().sections[0].headers["default"].clone();
    link(&mut doc, "header", ids[1], false);
    let own = doc.page_setup().sections[1].headers["default"].clone();
    type_into(&mut doc, &own, "Own ");
    link(&mut doc, "header", ids[1], true);
    assert_eq!(doc.page_setup().sections[1].headers["default"], shared);
    // Undo the link: the edited copy is back.
    assert!(doc.undo());
    let ps = doc.page_setup();
    assert_eq!(ps.sections[1].headers["default"], own);
    assert_eq!(header_text(&doc, &own), "Own Company");
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert_eq!(header_parts(&bytes).len(), 2);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(header_text(&again, &again.page_setup().sections[1].headers["default"]), "Own Company");
    // Undo the typing and the unlink: shared again, and the file has one header part.
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(doc.page_setup().sections[1].headers["default"], shared);
    assert!(doc.page_setup().sections[1].linked_headers.contains(&"default".to_string()));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert_eq!(header_parts(&bytes), vec![shared]);
    // Redo all the way.
    assert!(doc.redo() && doc.redo() && doc.redo());
    assert_eq!(header_text(&doc, &doc.page_setup().sections[1].headers["default"]), "Company");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn nothing_to_copy_gives_an_empty_one() {
    let (mut doc, ids) = three_sections();
    // No footer anywhere: unlinking the second section's footer makes a new empty one.
    link(&mut doc, "footer", ids[1], false);
    let ps = doc.page_setup();
    assert!(ps.sections[0].footers.is_empty());
    let part = ps.sections[1].footers["default"].clone();
    assert_eq!(header_text(&doc, &part), "");
    assert!(!ps.sections[1].linked_footers.contains(&"default".to_string()));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn the_first_section_cannot_link() {
    let (mut doc, ids) = three_sections();
    for linked in [true, false] {
        let err = doc.apply_json(&json!({"op":"link_header_footer","kind":"header","section":ids[0],"linked":linked}).to_string(), false).unwrap_err();
        assert!(err.to_string().contains("first section"), "{err}");
    }
}

#[test]
fn the_first_page_variant_links_on_its_own() {
    let (mut doc, ids) = three_sections();
    ops(&mut doc, json!({"op":"set_title_page","on":true,"section":ids[0]}));
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","variant":"first","section":ids[0]}));
    ops(&mut doc, json!({"op":"link_header_footer","kind":"header","variant":"first","section":ids[1],"linked":false}));
    let ps = doc.page_setup();
    assert_ne!(ps.sections[1].headers["first"], ps.sections[0].headers["first"]);
    // The default header is still shared.
    assert_eq!(ps.sections[1].headers["default"], ps.sections[0].headers["default"]);
    assert!(ps.sections[1].linked_headers.contains(&"default".to_string()) && !ps.sections[1].linked_headers.contains(&"first".to_string()));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn the_tool_links_and_unlinks() {
    let (mut doc, ids) = three_sections();
    let section = format!("p{}", ids[1]);
    call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","section":section,"link_to_previous":false})).unwrap();
    assert!(!doc.page_setup().sections[1].linked_headers.contains(&"default".to_string()));
    call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","section":section,"link_to_previous":true})).unwrap();
    assert!(doc.page_setup().sections[1].linked_headers.contains(&"default".to_string()));
    assert!(call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","section":section})).is_err());
}
