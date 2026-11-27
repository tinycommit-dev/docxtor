//! `fonts_used` reads the fonts the last render drew each block with, and those of the last page setup, instead of rendering everything
//! again (half a second after opening a long book). In test builds every answer is also compared with a render of everything (a
//! difference panics), so these tests walk the ways the kept fonts can go stale: no render yet, an edit not drawn yet, an edit drawn,
//! a change of style, a header added, undo and redo.

use docxtor_core::Document;
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

fn families(d: &Document) -> Vec<String> {
    d.fonts_used().into_iter().map(|u| u.family).collect()
}

#[test]
fn the_kept_fonts_follow_edits_renders_headers_undo_and_redo() {
    let mut d = Document::open(&fixture("rich.docx")).unwrap();
    // Before any render, and after one (the kept fonts are read then).
    let before = families(&d);
    let first = d.render_all().order[0];
    let _ = d.page_setup();
    assert_eq!(families(&d), before);
    // A font given to some text: not drawn yet, then drawn.
    d.apply_json(&json!({"op":"format_text","from":{"p":first,"o":0},"to":{"p":first,"o":3},"format":{"font":"Courier New"}}).to_string(), false).unwrap();
    assert!(families(&d).iter().any(|f| f == "Courier New"));
    let _ = d.render_changed();
    assert!(families(&d).iter().any(|f| f == "Courier New"));
    // A header, with its text in another font.
    d.apply_json(&json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"}).to_string(), false).unwrap();
    let _ = d.render_changed();
    let _ = families(&d);
    // Undo and redo, drawn or not.
    assert!(d.undo());
    let _ = families(&d);
    let _ = d.render_changed();
    assert!(d.undo());
    let _ = d.render_changed();
    assert!(!families(&d).iter().any(|f| f == "Courier New"));
    assert!(d.redo());
    let _ = d.render_changed();
    assert!(families(&d).iter().any(|f| f == "Courier New"));
}

#[test]
fn a_style_change_reaches_the_kept_fonts() {
    let mut d = Document::open(&fixture("rich.docx")).unwrap();
    let _ = d.render_all();
    let _ = families(&d);
    d.apply_json(&json!({"op":"set_style_definition","style":"Normal","def":{"rpr":{"font":"Georgia"}}}).to_string(), false).unwrap();
    // Not drawn yet: the kept fonts are not used.
    assert!(families(&d).iter().any(|f| f == "Georgia"));
    let _ = d.render_changed();
    assert!(families(&d).iter().any(|f| f == "Georgia"));
}
