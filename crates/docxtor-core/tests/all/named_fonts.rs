//! The font families a document names without a render (`fonts::named_families`): what the editor loads before the first layout.

use docxtor_core::Document;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

#[test]
fn the_font_table_the_theme_and_the_styles_name_their_fonts_once_each() {
    let d = Document::open(&fixture("rich.docx")).unwrap();
    let names = docxtor_core::fonts::named_families(&d.pkg, &d.main_path, &d.styles);
    // The font table's fonts, in its order.
    for want in ["Symbol", "Times New Roman", "Cambria", "Calibri"] {
        assert!(names.iter().any(|n| n == want), "{want} missing from {names:?}");
    }
    // Each family once, whatever its case.
    let mut lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    lower.sort();
    let before = lower.len();
    lower.dedup();
    assert_eq!(before, lower.len(), "a family named twice in {names:?}");
}

#[test]
fn a_document_without_a_font_table_names_its_theme_fonts() {
    let mut d = Document::new_blank();
    d.styles.theme.minor = Some("Calibri".into());
    d.styles.theme.major = Some("Calibri Light".into());
    let names = docxtor_core::fonts::named_families(&d.pkg, &d.main_path, &d.styles);
    assert!(names.contains(&"Calibri".to_string()) && names.contains(&"Calibri Light".to_string()), "{names:?}");
}
