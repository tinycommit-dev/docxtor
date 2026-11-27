//! Created and last changed dates (`docProps/core.xml`): read from the file, and written on save only when the
//! document changed, so documents that were not edited stay byte for byte as they were.

use docxtor_core::{package::Package, Document};
use serde_json::json;

const T1: &str = "2026-10-13T09:30:00Z";
const T2: &str = "2026-10-14T18:05:09Z";
const T3: &str = "2026-10-15T07:00:00Z";

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    Package::open(bytes).unwrap().get(name).map(|b| String::from_utf8(b.to_vec()).unwrap())
}

/// A document whose core properties are the ones Word writes.
fn word_made() -> Vec<u8> {
    let mut pkg = Package::open(&fixture("simple.docx")).unwrap();
    pkg.set(
        "docProps/core.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>Plan</dc:title><dc:creator>Ana</dc:creator><cp:lastModifiedBy>Ben</cp:lastModifiedBy><cp:revision>3</cp:revision><dcterms:created xsi:type="dcterms:W3CDTF">2024-01-05T10:20:00Z</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">2024-02-06T11:21:00Z</dcterms:modified></cp:coreProperties>"#
            .to_vec(),
    );
    if !pkg.rels_for("").unwrap().iter().any(|r| r.target == "docProps/core.xml") {
        pkg.add_rel("", docxtor_core::package::REL_CORE, "docProps/core.xml", false);
        pkg.ensure_override("docProps/core.xml", docxtor_core::package::CT_CORE);
    }
    pkg.to_bytes().unwrap()
}

fn type_something(doc: &mut Document) {
    let first = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}).to_string(), false).unwrap();
}

#[test]
fn the_dates_are_read_from_the_file() {
    let doc = Document::open(&word_made()).unwrap();
    let props = doc.doc_props();
    assert_eq!(props.title.as_deref(), Some("Plan"));
    assert_eq!((props.created.as_deref(), props.modified.as_deref(), props.last_modified_by.as_deref()), (Some("2024-01-05T10:20:00Z"), Some("2024-02-06T11:21:00Z"), Some("Ben")));
    // A document that was never saved with dates has none.
    let blank = Document::new_blank().doc_props();
    assert!(blank.created.is_none() && blank.modified.is_none() && blank.last_modified_by.is_none());
}

#[test]
fn an_untouched_document_is_saved_byte_for_byte() {
    for name in ["simple.docx", "rich.docx", "complex.docx", "long.docx"] {
        let orig = fixture(name);
        let mut doc = Document::open(&orig).unwrap();
        let out = doc.save_stamped(T1).unwrap();
        let a = Package::open(&orig).unwrap();
        let b = Package::open(&out).unwrap();
        for p in &a.parts {
            assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{name}: {} changed", p.name);
        }
        assert_eq!(a.parts.len(), b.parts.len(), "{name}");
    }
    let orig = word_made();
    let mut doc = Document::open(&orig).unwrap();
    let out = doc.save_stamped(T1).unwrap();
    assert_eq!(part(&out, "docProps/core.xml"), part(&orig, "docProps/core.xml"));
}

#[test]
fn saving_after_an_edit_sets_the_changed_date_and_keeps_the_rest() {
    let orig = word_made();
    let mut doc = Document::open(&orig).unwrap();
    type_something(&mut doc);
    let out = doc.save_stamped(T1).unwrap();
    assert_valid(&out);
    let core = part(&out, "docProps/core.xml").unwrap();
    assert!(core.contains(&format!(r#"<dcterms:modified xsi:type="dcterms:W3CDTF">{T1}</dcterms:modified>"#)), "{core}");
    // The created date, the author, the last editor and the revision are as Word wrote them.
    for kept in ["<dcterms:created xsi:type=\"dcterms:W3CDTF\">2024-01-05T10:20:00Z</dcterms:created>", "<dc:creator>Ana</dc:creator>", "<cp:lastModifiedBy>Ben</cp:lastModifiedBy>", "<cp:revision>3</cp:revision>", "<dc:title>Plan</dc:title>"] {
        assert!(core.contains(kept), "{kept}\n{core}");
    }
    assert_eq!(Document::open(&out).unwrap().doc_props().modified.as_deref(), Some(T1));
    // Saving again without editing leaves the date alone, even asked at another time.
    let again = doc.save_stamped(T2).unwrap();
    assert_eq!(part(&again, "docProps/core.xml"), Some(core.clone()));
    // Another edit moves it.
    type_something(&mut doc);
    let third = doc.save_stamped(T3).unwrap();
    assert!(part(&third, "docProps/core.xml").unwrap().contains(T3));
    // Other parts of the package are untouched by the stamp (the statistics in app.xml are brought up to date on an edited save).
    let (a, b) = (Package::open(&orig).unwrap(), Package::open(&third).unwrap());
    for p in a.parts.iter().filter(|p| !["docProps/core.xml", "docProps/app.xml", "word/document.xml"].contains(&p.name.as_str())) {
        assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
    }
}

#[test]
fn a_file_without_properties_gets_them_when_edited() {
    let mut pkg = Package::open(&fixture("simple.docx")).unwrap();
    pkg.parts.retain(|p| p.name != "docProps/core.xml");
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert!(doc.doc_props().modified.is_none());
    // Untouched: still none.
    let out = doc.save_stamped(T1).unwrap();
    assert!(part(&out, "docProps/core.xml").is_none());
    type_something(&mut doc);
    let out = doc.save_stamped(T2).unwrap();
    assert_valid(&out);
    let reopened = Document::open(&out).unwrap().doc_props();
    // The changed date is recorded; the file's creation date is unknown, so none is invented.
    assert_eq!((reopened.modified.as_deref(), reopened.created), (Some(T2), None));
    let pkg = Package::open(&out).unwrap();
    assert!(pkg.rels_for("").unwrap().iter().any(|r| r.rel_type == docxtor_core::package::REL_CORE));
    assert!(part(&out, "[Content_Types].xml").unwrap().contains("/docProps/core.xml"));
}

#[test]
fn a_new_document_gets_both_dates_once() {
    let mut doc = Document::new_blank();
    let out = doc.save_stamped(T1).unwrap();
    assert_valid(&out);
    let p = Document::open(&out).unwrap().doc_props();
    assert_eq!((p.created.as_deref(), p.modified.as_deref()), (Some(T1), Some(T1)));
    // Later edits move the changed date only.
    type_something(&mut doc);
    let out = doc.save_stamped(T2).unwrap();
    let p = Document::open(&out).unwrap().doc_props();
    assert_eq!((p.created.as_deref(), p.modified.as_deref()), (Some(T1), Some(T2)));
}

#[test]
fn setting_the_title_counts_as_a_change_and_a_bad_time_is_ignored() {
    let mut doc = Document::open(&word_made()).unwrap();
    doc.apply_json(&json!({"op":"set_doc_props","title":"New plan"}).to_string(), false).unwrap();
    let out = doc.save_stamped("yesterday").unwrap();
    assert_eq!(Document::open(&out).unwrap().doc_props().modified.as_deref(), Some("2024-02-06T11:21:00Z"));
    let mut doc = Document::open(&word_made()).unwrap();
    doc.apply_json(&json!({"op":"set_doc_props","title":"New plan"}).to_string(), false).unwrap();
    let out = doc.save_stamped(T1).unwrap();
    let p = Document::open(&out).unwrap().doc_props();
    assert_eq!((p.title.as_deref(), p.modified.as_deref()), (Some("New plan"), Some(T1)));
}
