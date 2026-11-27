//! Lists in headers, footers and notes are drawn with their bullets and numbers, each counting on its own (as in Word).

use docxtor_core::Document;
use serde_json::{json, Value};

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn setup(doc: &Document) -> Value {
    serde_json::to_value(doc.page_setup()).unwrap()
}

fn labels(blocks: &Value) -> Vec<String> {
    blocks.as_array().unwrap().iter().map(|b| b["list"]["text"].as_str().unwrap_or("").to_string()).collect()
}

#[test]
fn a_bulleted_footer_shows_its_bullet() {
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op":"add_header_footer","kind":"footer"}));
    let p = setup(&doc)["footers"]["default"]["blocks"][0]["id"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Footer bullet"}));
    ops(&mut doc, json!({"op":"toggle_list","ids":[p],"kind":"bullet"}));
    assert_eq!(labels(&setup(&doc)["footers"]["default"]["blocks"]), ["•"]);
}

#[test]
fn a_numbered_list_in_a_header_counts_from_one_beside_one_in_the_text() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"1. Body one\n2. Body two"}));
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    let p = setup(&doc)["headers"]["default"]["blocks"][0]["id"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Head one"}));
    ops(&mut doc, json!({"op":"split_paragraph","at":{"p":p,"o":8}}));
    let ids: Vec<u64> = setup(&doc)["headers"]["default"]["blocks"].as_array().unwrap().iter().map(|b| b["id"].as_u64().unwrap()).collect();
    ops(&mut doc, json!({"op":"toggle_list","ids":ids,"kind":"decimal"}));
    assert_eq!(labels(&setup(&doc)["headers"]["default"]["blocks"]), ["1.", "2."]);
}

#[test]
fn a_list_in_a_footnote_shows_its_bullets() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Text"}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":first,"o":4},"text":"A note"}));
    let note = &setup(&doc)["notes"][0]["blocks"];
    let p = note.as_array().unwrap().iter().find(|b| b["t"] == "p").unwrap()["id"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"toggle_list","ids":[p],"kind":"bullet"}));
    assert!(labels(&setup(&doc)["notes"][0]["blocks"]).contains(&"•".to_string()));
}
