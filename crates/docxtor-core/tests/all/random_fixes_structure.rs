//! Found by the random op harness: a refused
//! batch gives back every part it rewrote, a header a section names is never dropped, and a tab starts right after
//! a section break.
#![allow(dead_code)]

use crate::common;

use common::build;
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

fn problems(doc: &Document) -> Vec<String> {
    doc.check_invariants().into_iter().map(|v| format!("{}: {}", v.rule, v.message)).collect()
}

#[test]
fn refused_batch_gives_back_custom_xml_written_by_a_binding() {
    let mut doc = build("rich");
    run(&mut doc, json!({"op":"protection","action":"set","settings":{"edit":"forms","enforce":true}}));
    let before = parts(&mut doc);
    let open = doc.tabs_info().active;
    let r = doc.apply_json(
        &json!([
            {"op":"control","action":"insert","at":{"p":11,"o":19},"kind":"text"},
            {"op":"tab","action":"delete","id":"t1"}
        ])
        .to_string(),
        false,
    );
    assert!(r.is_err(), "the batch should be refused");
    assert_eq!(doc.tabs_info().active, open, "the open tab moved");
    let after = parts(&mut doc);
    assert_eq!(before.len(), after.len());
    for (b, a) in before.iter().zip(&after) {
        assert!(b.1 == a.1, "part {} changed after a refused batch", b.0);
    }
}

#[test]
fn first_page_header_and_a_tab_in_one_batch_keep_the_header() {
    let mut doc = build("plain");
    run(&mut doc, json!({"op":"insert_horizontal_line","at":{"p":3,"o":33}}));
    run(&mut doc, json!({"op":"insert_index","after":2}));
    run(&mut doc, json!({"op":"insert_toc","before":7}));
    run(
        &mut doc,
        json!([
            {"op":"set_language","ids":[9],"lang":"xx"},
            {"op":"add_header_footer","kind":"header","variant":"first"},
            {"op":"tab","action":"add","name":"_hidden"}
        ]),
    );
    let bytes = doc.save().unwrap();
    assert_eq!(check_package(&bytes), vec![]);
    let p = problems(&doc);
    assert!(p.is_empty(), "{p:?}");
}

#[test]
fn tab_after_a_section_break_stays_valid() {
    let mut doc = build("rich");
    run(&mut doc, json!({"op":"remove_cover_page"}));
    run(&mut doc, json!({"op":"tab","action":"duplicate","id":"t1"}));
    run(&mut doc, json!({"op":"move_blocks","ids":[1],"after":36}));
    run(&mut doc, json!({"op":"tab","action":"delete","id":"t2"}));
    let _ = doc.apply_json(&json!({"op":"tab","action":"move","id":"t1","before":null}).to_string(), false);
    let p = problems(&doc);
    assert!(p.is_empty(), "{p:?}");
}


#[test]
fn refused_batch_gives_back_a_custom_xml_part_an_op_rewrote() {
    // A cover page keeps its text in a custom XML part; changing the document properties rewrites it.
    let mut doc = build("plain");
    run(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    let before = parts(&mut doc);
    let r = doc.apply_json(
        &json!([
            {"op":"set_doc_props","title":"A new title","author":"Someone else"},
            {"op":"set_paragraph_text","id":987654321,"markdown":"no"}
        ])
        .to_string(),
        false,
    );
    assert!(r.is_err());
    assert!(before == parts(&mut doc), "a part changed after a refused batch");
}
