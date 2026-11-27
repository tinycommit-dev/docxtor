//! Document properties beyond the title: author, subject, keywords, category, status, company, manager, custom
//! properties, and the statistics Word keeps in `docProps/app.xml`, all as ops (undoable) and written as Word does.

use docxtor_core::{package::Package, Document};
use serde_json::json;

const T1: &str = "2026-10-13T09:30:00Z";

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc =
                docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc)
                .unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    Package::open(bytes)
        .unwrap()
        .get(name)
        .map(|b| String::from_utf8(b.to_vec()).unwrap())
}

fn run(doc: &mut Document, ops: serde_json::Value) {
    doc.apply_json(&ops.to_string(), false).unwrap();
}

#[test]
fn every_property_is_written_where_word_keeps_it() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    run(
        &mut doc,
        json!({"op": "set_doc_props", "title": "Plan", "subject": "Budget", "author": "Ana", "keywords": "money, plan", "description": "Draft", "category": "Finance", "status": "Final", "company": "Analytical", "manager": "Ada"}),
    );
    let p = doc.doc_props();
    assert_eq!(
        (
            p.title.as_deref(),
            p.subject.as_deref(),
            p.author.as_deref(),
            p.keywords.as_deref(),
            p.description.as_deref(),
            p.category.as_deref(),
            p.status.as_deref(),
            p.company.as_deref(),
            p.manager.as_deref()
        ),
        (
            Some("Plan"),
            Some("Budget"),
            Some("Ana"),
            Some("money, plan"),
            Some("Draft"),
            Some("Finance"),
            Some("Final"),
            Some("Analytical"),
            Some("Ada")
        )
    );
    let out = doc.save_stamped(T1).unwrap();
    assert_valid(&out);
    let core = part(&out, "docProps/core.xml").unwrap();
    for want in [
        "<dc:subject>Budget</dc:subject>",
        "<dc:creator>Ana</dc:creator>",
        "<cp:keywords>money, plan</cp:keywords>",
        "<dc:description>Draft</dc:description>",
        "<cp:category>Finance</cp:category>",
        "<cp:contentStatus>Final</cp:contentStatus>",
    ] {
        assert!(core.contains(want), "{want} in {core}");
    }
    let app = part(&out, "docProps/app.xml").unwrap();
    assert!(
        app.contains("<Company>Analytical</Company>") && app.contains("<Manager>Ada</Manager>"),
        "{app}"
    );
    // Opening the saved file reads them back.
    let again = Document::open(&out).unwrap().doc_props();
    assert_eq!(
        (
            again.company.as_deref(),
            again.category.as_deref(),
            again.status.as_deref()
        ),
        (Some("Analytical"), Some("Finance"), Some("Final"))
    );
}

#[test]
fn properties_undo_and_an_empty_value_removes_one() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    run(
        &mut doc,
        json!({"op": "set_doc_props", "company": "Analytical", "category": "Finance"}),
    );
    run(
        &mut doc,
        json!([{"op": "set_doc_props", "company": "", "category": "Ops"}, {"op": "set_custom_props", "set": [{"name": "Project", "type": "text", "value": "Babbage"}]}]),
    );
    let p = doc.doc_props();
    assert_eq!(
        (p.company, p.category.as_deref(), p.custom.len()),
        (None, Some("Ops"), 1)
    );
    assert!(doc.undo());
    let p = doc.doc_props();
    assert_eq!(
        (p.company.as_deref(), p.category.as_deref(), p.custom.len()),
        (Some("Analytical"), Some("Finance"), 0)
    );
    assert!(doc.undo());
    assert!(doc.doc_props().company.is_none());
    assert!(doc.redo() && doc.redo());
    let p = doc.doc_props();
    assert_eq!((p.company, p.custom.len()), (None, 1));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn custom_properties_are_added_changed_removed_and_saved() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    run(
        &mut doc,
        json!({"op": "set_custom_props", "set": [
            {"name": "Project", "type": "text", "value": "Babbage"},
            {"name": "Revision", "type": "number", "value": "7"},
            {"name": "Reviewed", "type": "bool", "value": "yes"},
            {"name": "Due", "type": "date", "value": "2026-10-14"}
        ]}),
    );
    let out = doc.save().unwrap();
    assert_valid(&out);
    let custom = part(&out, "docProps/custom.xml").unwrap();
    for want in [
        "name=\"Project\"",
        "<vt:lpwstr>Babbage</vt:lpwstr>",
        "<vt:i4>7</vt:i4>",
        "<vt:bool>true</vt:bool>",
        "<vt:filetime>2026-10-14T00:00:00Z</vt:filetime>",
    ] {
        assert!(custom.contains(want), "{want} in {custom}");
    }
    // The part is related and typed, so Word finds it.
    assert!(part(&out, "_rels/.rels")
        .unwrap()
        .contains("custom-properties"));
    assert!(part(&out, "[Content_Types].xml")
        .unwrap()
        .contains("/docProps/custom.xml"));

    let mut doc = Document::open(&out).unwrap();
    assert_eq!(doc.doc_props().custom.len(), 4);
    run(
        &mut doc,
        json!({"op": "set_custom_props", "set": [{"name": "revision", "type": "number", "value": "8.5"}], "remove": ["Project", "Nope"]}),
    );
    let names: Vec<_> = doc
        .doc_props()
        .custom
        .iter()
        .map(|c| (c.name.clone(), c.value.clone()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("revision".to_string(), "8.5".to_string()),
            ("Reviewed".to_string(), "true".to_string()),
            ("Due".to_string(), "2026-10-14".to_string())
        ]
    );
    assert!(part(&doc.save().unwrap(), "docProps/custom.xml")
        .unwrap()
        .contains("<vt:r8>8.5</vt:r8>"));

    // A bad value is refused and changes nothing.
    let err = doc.apply_json(&json!({"op": "set_custom_props", "set": [{"name": "A", "type": "number", "value": "abc"}]}).to_string(), false);
    assert!(err.is_err());
    assert_eq!(doc.doc_props().custom.len(), 3);
}

#[test]
fn marked_as_final_stays_out_of_the_custom_list_and_survives() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    run(
        &mut doc,
        json!({"op": "protection", "action": "mark_final", "on": true}),
    );
    run(
        &mut doc,
        json!({"op": "set_custom_props", "set": [{"name": "Team", "type": "text", "value": "A"}]}),
    );
    assert_eq!(doc.doc_props().custom.len(), 1);
    let out = doc.save().unwrap();
    assert!(part(&out, "docProps/custom.xml")
        .unwrap()
        .contains("_MarkAsFinal"));
    // The private name cannot be set or removed through the custom list.
    assert!(doc.apply_json(&json!({"op": "set_custom_props", "set": [{"name": "_MarkAsFinal", "type": "bool", "value": "false"}]}).to_string(), false).is_err());
}

#[test]
fn docproperty_fields_read_the_values_as_they_are_now() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    run(
        &mut doc,
        json!({"op": "set_doc_props", "company": "Analytical", "category": "Finance"}),
    );
    run(
        &mut doc,
        json!({"op": "set_custom_props", "set": [{"name": "Project", "type": "text", "value": "Babbage"}]}),
    );
    assert_eq!(
        doc.field_preview("DOCPROPERTY Company", None)
            .unwrap_or_default(),
        "Analytical"
    );
    assert_eq!(
        doc.field_preview("DOCPROPERTY Category", None)
            .unwrap_or_default(),
        "Finance"
    );
    assert_eq!(
        doc.field_preview("DOCPROPERTY Project", None)
            .unwrap_or_default(),
        "Babbage"
    );
}

#[test]
fn saving_an_edited_document_updates_word_s_statistics() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    run(
        &mut doc,
        json!({"op": "insert_markdown", "before": first, "markdown": "Hello big world\n\nSecond paragraph here"}),
    );
    run(&mut doc, json!({"op": "delete_blocks", "ids": [first]}));
    doc.set_save_info(docxtor_core::document::SaveInfo {
        user: Some("Ana".into()),
        pages: Some(2),
        lines: Some(9),
    });
    let out = doc.save_stamped(T1).unwrap();
    assert_valid(&out);
    let app = part(&out, "docProps/app.xml").unwrap();
    for want in [
        "<Pages>2</Pages>",
        "<Lines>9</Lines>",
        "<Words>6</Words>",
        "<Paragraphs>2</Paragraphs>",
        "<Application>Docxtor</Application>",
    ] {
        assert!(app.contains(want), "{want} in {app}");
    }
    let chars: usize = "Hello big world"
        .chars()
        .chain("Second paragraph here".chars())
        .filter(|c| !c.is_whitespace())
        .count();
    assert!(
        app.contains(&format!("<Characters>{chars}</Characters>")),
        "{app}"
    );
    assert!(
        app.contains("<CharactersWithSpaces>36</CharactersWithSpaces>"),
        "{app}"
    );
    assert!(part(&out, "docProps/core.xml")
        .unwrap()
        .contains("<cp:lastModifiedBy>Ana</cp:lastModifiedBy>"));
    // A second save with more text updates the numbers; the page counts stay as given.
    let first = doc.render_all().order[0];
    run(
        &mut doc,
        json!({"op": "insert_markdown", "before": first, "markdown": "One more"}),
    );
    let again = part(
        &doc.save_stamped("2026-10-14T10:00:00Z").unwrap(),
        "docProps/app.xml",
    )
    .unwrap();
    assert!(
        again.contains("<Words>8</Words>") && again.contains("<Paragraphs>3</Paragraphs>"),
        "{again}"
    );
}

#[test]
fn an_untouched_document_keeps_its_statistics_as_they_were() {
    let orig = fixture("rich.docx");
    let mut doc = Document::open(&orig).unwrap();
    doc.set_save_info(docxtor_core::document::SaveInfo {
        user: Some("Ana".into()),
        pages: Some(9),
        lines: Some(99),
    });
    let out = doc.save_stamped(T1).unwrap();
    assert_eq!(
        part(&out, "docProps/app.xml"),
        part(&orig, "docProps/app.xml")
    );
    assert_eq!(
        part(&out, "docProps/core.xml"),
        part(&orig, "docProps/core.xml")
    );
}

#[test]
fn a_document_without_extended_properties_gets_the_part() {
    let mut pkg = Package::open(&fixture("simple.docx")).unwrap();
    let rels = String::from_utf8(pkg.get("_rels/.rels").unwrap().to_vec()).unwrap();
    // Take app.xml out of the package and the relationship file.
    let kept: String = rels
        .split("<Relationship ")
        .enumerate()
        .filter(|(i, r)| *i == 0 || !r.contains("extended-properties"))
        .map(|(i, r)| {
            if i == 0 {
                r.to_string()
            } else {
                format!("<Relationship {r}")
            }
        })
        .collect();
    pkg.set("_rels/.rels", kept.into_bytes());
    pkg.remove("docProps/app.xml");
    let bytes = pkg.to_bytes().unwrap();
    let mut doc = Document::open(&bytes).unwrap();
    run(
        &mut doc,
        json!({"op": "set_doc_props", "company": "Analytical"}),
    );
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert!(part(&out, "docProps/app.xml")
        .unwrap()
        .contains("<Company>Analytical</Company>"));
    assert!(part(&out, "_rels/.rels")
        .unwrap()
        .contains("extended-properties"));
    assert!(part(&out, "[Content_Types].xml")
        .unwrap()
        .contains("/docProps/app.xml"));
}

/// Found by the Open XML validator (scripts/validate.mjs): a custom.xml made from nothing had its XML declaration
/// written twice (`<?xml <?xml version=...?>?>`), which no XML reader accepts.
#[test]
fn a_new_custom_properties_part_has_one_xml_declaration() {
    let mut doc = Document::new_blank();
    doc.apply_json(&json!([{"op": "set_custom_props", "set": [{"name": "Team", "type": "text", "value": "A"}]}]).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    let part = String::from_utf8(pkg.get("docProps/custom.xml").unwrap().to_vec()).unwrap();
    assert!(part.starts_with("<?xml version=\"1.0\""), "{part}");
    assert_eq!(part.matches("<?xml").count(), 1, "{part}");
}
