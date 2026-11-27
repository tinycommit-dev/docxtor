//! An undo (or a redo) that takes a part back to what the file holds leaves it untouched: the save copies its bytes as they came,
//! as for a part that was never edited (the lossless rule), instead of writing it again in Docxtor's own way.

use docxtor_core::{package::Package, Document};
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    Package::open(bytes).unwrap().get(name).unwrap_or_else(|| panic!("no {name}")).to_vec()
}

#[test]
fn undoing_a_style_change_saves_the_styles_part_as_it_came() {
    let original = fixture("rich.docx");
    let mut d = Document::open(&original).unwrap();
    d.apply_json(&json!({"op":"set_style_definition","style":"Normal","def":{"rpr":{"size":20.0}}}).to_string(), false).unwrap();
    assert!(part(&d.save().unwrap(), "word/styles.xml") != part(&original, "word/styles.xml"));
    assert!(d.undo());
    assert!(part(&d.save().unwrap(), "word/styles.xml") == part(&original, "word/styles.xml"), "the undone styles part is not byte for byte the file's");
    // And a redo writes the change again.
    assert!(d.redo());
    assert!(part(&d.save().unwrap(), "word/styles.xml") != part(&original, "word/styles.xml"));
}

#[test]
fn undoing_a_settings_change_saves_the_settings_part_as_it_came() {
    let original = fixture("rich.docx");
    let mut d = Document::open(&original).unwrap();
    d.apply_json(&json!({"op":"set_layout_settings","even_and_odd_headers":true}).to_string(), false).unwrap();
    assert!(part(&d.save().unwrap(), "word/settings.xml") != part(&original, "word/settings.xml"));
    assert!(d.undo());
    assert!(part(&d.save().unwrap(), "word/settings.xml") == part(&original, "word/settings.xml"));
}

/// Found by the corpus run's random steps (a damaged POI file with no settings part): a setting made, saved, then
/// undone back to the file, which had no settings part, kept the part the save wrote, so the file opened again with even and odd
/// headers the text did not have. The part goes, with its relationship and content type.
#[test]
fn undoing_back_to_a_file_without_settings_takes_the_saved_settings_part_away() {
    let mut pkg = Package::open(&fixture("simple.docx")).unwrap();
    let main = pkg.main_document_path();
    if pkg.has("word/settings.xml") {
        pkg.remove_rels_to(&main, "word/settings.xml");
        pkg.remove_override("word/settings.xml");
        pkg.remove("word/settings.xml");
    }
    let original = pkg.to_bytes().unwrap();
    let mut d = Document::open(&original).unwrap();
    d.apply_json(&json!({"op":"set_layout_settings","even_and_odd_headers":true}).to_string(), false).unwrap();
    let saved = d.save().unwrap();
    assert!(Package::open(&saved).unwrap().has("word/settings.xml"), "the setting is saved");
    assert!(d.undo());
    assert!(!d.page_setup().even_and_odd_headers);
    let again = d.save().unwrap();
    let p = Package::open(&again).unwrap();
    assert!(!p.has("word/settings.xml"), "the settings part the save wrote is gone again");
    assert!(!String::from_utf8_lossy(p.get("word/_rels/document.xml.rels").unwrap()).contains("settings.xml"));
    assert!(!String::from_utf8_lossy(p.get("[Content_Types].xml").unwrap()).contains("settings.xml"));
    assert!(!Document::open(&again).unwrap().page_setup().even_and_odd_headers, "and it opens again without the setting");
}

/// A document whose body is `body` (before the section properties).
fn with_body(body: &str) -> Vec<u8> {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let at = xml.find("<w:body>").unwrap() + "<w:body>".len();
    let xml = format!("{}{}{}", &xml[..at], body, &xml[at..]);
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.to_bytes().unwrap()
}

#[test]
fn an_edit_elsewhere_leaves_a_table_cell_that_ends_with_a_control_as_it_came() {
    // A cell may end with a block-level content control in a real file (Word wants a paragraph last). An edit in another paragraph
    // used to give that cell its paragraph without marking the table: the undo step's copy of the table was stale, and undo took the
    // paragraph back out of a cell the edit never touched.
    let cell_ends_with_control = r#"<w:p><w:r><w:t>Before</w:t></w:r></w:p><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:sdt><w:sdtPr><w:id w:val="5"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>In the control</w:t></w:r></w:p></w:sdtContent></w:sdt></w:tc></w:tr></w:tbl><w:p/>"#;
    let original = with_body(cell_ends_with_control);
    let mut d = Document::open(&original).unwrap();
    let before = serde_json::to_value(d.render_all().blocks).unwrap();
    let first = d.render_all().order[0];
    d.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}).to_string(), false).unwrap();
    // The table is as it was: the edit was in the paragraph before it.
    let after = serde_json::to_value(d.render_all().blocks).unwrap();
    assert_eq!(before[1], after[1], "the table changed with an edit in another paragraph");
    assert!(d.undo());
    assert_eq!(serde_json::to_value(d.render_all().blocks).unwrap(), before);
}

/// Found by the random search (E2): an undo puts back the relationships part as it was when its step ran, and a
/// save in between had taken out a part it names (the numbering, after the list that made it was undone). The saved file then named a
/// part it did not hold. After an undo, the part, its relationship and the text agree, whatever the step put back.
#[test]
fn an_undo_never_leaves_a_relationship_to_a_part_that_is_not_there() {
    let mut d = Document::new_blank();
    let first = d.render_all().order[0];
    d.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"One"}).to_string(), false).unwrap();
    let had_numbering = Package::open(&d.save().unwrap()).unwrap().has("word/numbering.xml");
    d.apply_json(&json!({"op":"toggle_list","ids":[first],"kind":"bullet"}).to_string(), false).unwrap();
    assert!(Package::open(&d.save().unwrap()).unwrap().has("word/numbering.xml"));
    assert!(d.undo());
    // A content control whose placeholder adds the glossary: its step records the relationships part as it is now.
    d.apply_json(&json!({"op":"control","action":"insert","at":{"p":first,"o":0},"kind":"text"}).to_string(), false).unwrap();
    d.save().unwrap();
    assert!(d.undo());
    let saved = d.save().unwrap();
    let broken: Vec<String> = docxtor_core::invariants::check_package(&saved).iter().filter(|v| v.rule == "E2").map(|v| v.to_string()).collect();
    assert!(broken.is_empty(), "{broken:?}");
    let p = Package::open(&saved).unwrap();
    assert_eq!(p.has("word/numbering.xml"), had_numbering, "the numbering is as before the list");
}
