//! Found by the random op harness: a refused
//! batch leaves no trace, and moving content between parts keeps the relationships it uses.
#![allow(dead_code)]

use crate::common;

use common::{build, repo_path};
use docxtor_core::invariants::check_package;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

fn run(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn parts(doc: &mut Document) -> Vec<(String, Vec<u8>)> {
    let pkg = Package::open(&doc.save().unwrap()).unwrap();
    let mut v: Vec<_> = pkg.parts.iter().map(|p| (p.name.clone(), p.data.clone())).collect();
    v.sort();
    v
}

/// The refused batch must leave every saved part and the "changed" mark as they were.
fn refused_leaves_no_trace(mut doc: Document, op: Value) {
    let before = parts(&mut doc);
    let modified = doc.is_modified();
    assert!(doc.apply_json(&op.to_string(), false).is_err(), "expected a refusal: {op}");
    assert_eq!(doc.is_modified(), modified, "is_modified changed after a refusal");
    let after = parts(&mut doc);
    let names = |v: &Vec<(String, Vec<u8>)>| v.iter().map(|p| p.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(&before), names(&after), "the set of parts changed");
    for (b, a) in before.iter().zip(&after) {
        assert!(b.1 == a.1, "part {} changed after a refused batch", b.0);
    }
}

#[test]
fn refused_batch_on_an_untouched_file_is_not_a_change() {
    let orig = std::fs::read(repo_path("fixtures/rich.docx")).unwrap();
    let mut doc = Document::open(&orig).unwrap();
    assert!(!doc.is_modified());
    assert!(doc.apply_json(&json!({"op":"set_paragraph_text","id":900125,"markdown":"# no"}).to_string(), false).is_err());
    assert!(!doc.is_modified());
    let a = Package::open(&orig).unwrap();
    let b = Package::open(&doc.save().unwrap()).unwrap();
    for p in &a.parts {
        assert!(b.get(&p.name) == Some(&p.data[..]), "{} was rewritten", p.name);
    }
}

#[test]
fn refused_text_edit_leaves_no_trace_on_built_rich() {
    refused_leaves_no_trace(build("rich"), json!({"op":"set_paragraph_text","id":900125,"markdown":"# no"}));
}

#[test]
fn refused_shape_insert_leaves_no_namespaces_behind() {
    refused_leaves_no_trace(
        build("plain"),
        json!({"op":"insert_shape","at":{"p":900926,"o":2},"geometry":"rect","w":40,"h":40,"floating":true}),
    );
}

#[test]
fn refused_control_insert_leaves_no_trace() {
    refused_leaves_no_trace(build("plain"), json!({"op":"control","action":"insert","at":{"p":900894,"o":2},"kind":"comboBox"}));
    refused_leaves_no_trace(build("rich"), json!({"op":"control","action":"insert","at":{"p":900894,"o":2},"kind":"comboBox"}));
}

#[test]
fn refused_batch_after_a_good_op_in_the_same_batch_leaves_no_trace() {
    // The first op adds a note (a new notes part, styles), the second names a paragraph that is not there.
    refused_leaves_no_trace(
        build("plain"),
        json!([{"op":"insert_endnote","at":{"p":1,"o":4},"text":"E1"},{"op":"split_paragraph","at":{"p":900569,"o":25}}]),
    );
}

#[test]
fn namespace_from_a_successful_op_is_undone_with_it() {
    let mut doc = build("plain");
    let start = parts(&mut doc);
    let first = doc.render_all().order[0];
    run(&mut doc, json!({"op":"insert_shape","at":{"p":first,"o":2},"geometry":"rect","w":40,"h":40,"floating":true}));
    assert!(doc.undo());
    assert!(doc.redo() && doc.undo());
    let after = parts(&mut doc);
    for (b, a) in start.iter().zip(&after) {
        assert!(b.1 == a.1, "part {} differs after do and undo", b.0);
    }
}

/// The cause of finding 10: a paragraph inserted "after" the closing section properties went behind them, so the body
/// ended in a paragraph and a later tab left a second closing section that hid the one the header was named in.
#[test]
fn inserting_after_the_closing_section_properties_goes_before_them() {
    let mut doc = build("plain");
    let last = doc.body.last().map(|b| b.id()).unwrap();
    assert!(matches!(doc.body.last(), Some(docxtor_core::model::Block::Opaque(_))));
    run(&mut doc, json!({"op":"insert_paragraph","after":last,"text":"x"}));
    assert!(matches!(doc.body.last(), Some(docxtor_core::model::Block::Opaque(_))), "the section properties must stay last");
}

#[test]
fn header_relationship_survives_adding_a_tab() {
    let mut doc = build("plain");
    run(&mut doc, json!({"op":"insert_break","at":{"p":1,"o":0},"kind":"odd_page"}));
    run(&mut doc, json!({"op":"insert_paragraph","after":2,"text":"x"}));
    run(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.three"}));
    run(&mut doc, json!({"op":"split_paragraph","at":{"p":7,"o":0}}));
    run(&mut doc, json!({"op":"tab","action":"add","name":"Section_2"}));
    let bytes = doc.save().unwrap();
    assert_eq!(check_package(&bytes), vec![]);
}

#[test]
fn note_relationships_move_with_convert_notes() {
    let mut doc = Document::open(&std::fs::read(repo_path("fixtures/simple.docx")).unwrap()).unwrap();
    run(&mut doc, json!([{"op":"insert_endnote","at":{"p":1,"o":12},"text":"E2"},{"op":"insert_toc","before":2,"levels":4,"title":"Contents"}]));
    let mut doc = Document::open(&doc.save().unwrap()).unwrap();
    run(&mut doc, json!({"op":"insert_chart","at":{"p":5,"o":4},"kind":"column","categories":["a","b"],"series":[{"name":"","values":[1.0,2.0]}],"alt":"My alt","height":100.0,"legend":"top","grouping":"stacked"}));
    run(&mut doc, json!({"op":"convert_notes","to":"footnote"}));
    let bytes = doc.save().unwrap();
    assert_eq!(check_package(&bytes), vec![]);
    let pkg = Package::open(&bytes).unwrap();
    let rels = String::from_utf8_lossy(pkg.get("word/_rels/footnotes.xml.rels").expect("footnotes.xml.rels")).to_string();
    assert!(rels.contains("charts/chart"), "the chart's relationship moved with the note: {rels}");
}
