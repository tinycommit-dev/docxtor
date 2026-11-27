//! What a client holds after `render_changed` must equal a full render (found by the random op harness).
//! The client here is the same as in `random_ops.rs`: it keeps the blocks it
//! was given and re-reads the page setup only when `render_changed` says `stories` changed.

use crate::common;

use common::*;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::stories::StoryKind;
use docxtor_core::Document;
use serde_json::{json, Value};
use std::collections::BTreeMap;

struct Client {
    order: Vec<Value>,
    blocks: BTreeMap<String, Value>,
    page: Value,
}

fn block_id(b: &Value) -> String {
    b.get("id").map(|v| v.to_string()).unwrap_or_default()
}

impl Client {
    fn new(doc: &mut Document) -> Client {
        let out = serde_json::to_value(doc.render_all()).unwrap();
        let mut c = Client { order: vec![], blocks: BTreeMap::new(), page: serde_json::to_value(doc.page_setup()).unwrap() };
        c.take(&out);
        c
    }
    fn take(&mut self, out: &Value) {
        if out["full"] == json!(true) {
            self.blocks.clear();
        }
        for b in out["blocks"].as_array().into_iter().flatten() {
            self.blocks.insert(block_id(b), b.clone());
        }
        if out["orderKept"] != json!(true) {
            self.order = out["order"].as_array().cloned().unwrap_or_default();
        }
        let keep: std::collections::HashSet<String> = self.order.iter().map(|v| v.to_string()).collect();
        self.blocks.retain(|k, _| keep.contains(k));
    }
    /// Bring the client up to date the way the app does; returns what `render_changed` gave.
    fn update(&mut self, doc: &mut Document) -> Value {
        let out = serde_json::to_value(doc.render_changed()).unwrap();
        self.take(&out);
        if out["stories"] == json!(true) {
            self.page = serde_json::to_value(doc.page_setup()).unwrap();
        }
        out
    }
    /// The first thing the client has that a full render does not give, or None.
    fn stale(&self, doc: &Document) -> Option<String> {
        let full = serde_json::to_value(doc.render_snapshot()).unwrap();
        if let Some(d) = first_diff(&Value::Array(self.order.clone()), &full["order"]) {
            return Some(format!("order: {d}"));
        }
        for b in full["blocks"].as_array().into_iter().flatten() {
            match self.blocks.get(&block_id(b)) {
                None => return Some(format!("never gave block {}", block_id(b))),
                Some(m) => {
                    if let Some(d) = first_diff(m, b) {
                        return Some(format!("block: {d}"));
                    }
                }
            }
        }
        first_diff(&self.page, &serde_json::to_value(doc.page_setup()).unwrap()).map(|d| format!("page setup: {d}"))
    }
}

fn apply(doc: &mut Document, c: &mut Client, op: Value) -> Value {
    doc.apply_json(&json!([op.clone()]).to_string(), false).unwrap_or_else(|e| panic!("{e}: {op}"));
    c.update(doc)
}

fn fresh(doc: &Document, c: &Client) {
    if let Some(d) = c.stale(doc) {
        panic!("the client is stale: {d}");
    }
}

fn fixture(name: &str) -> Document {
    Start::File(name.into()).open()
}

fn first_para(doc: &Document) -> u32 {
    doc.render_snapshot().order[0]
}

// ---- 17: the paragraph that ends a section ----------------------------------------------------------------------------

#[test]
fn two_cover_pages_in_a_row_leave_the_sections_current() {
    let mut doc = build("plain");
    let mut c = Client::new(&mut doc);
    for _ in 0..2 {
        apply(&mut doc, &mut c, json!({"op":"insert_cover_page","design":"docxtor.centered"}));
        fresh(&doc, &c);
    }
}

// ---- 18: headers, footers and notes follow the defaults, styles and language ------------------------------------------

#[test]
fn a_new_default_size_reaches_the_footer() {
    let mut doc = build("rich");
    let mut c = Client::new(&mut doc);
    let out = apply(&mut doc, &mut c, json!({"op":"set_default_format","format":{"size":10.0}}));
    assert_eq!(out["stories"], json!(true));
    fresh(&doc, &c);
}

#[test]
fn a_new_default_size_reaches_the_section_grid() {
    let mut doc = fixture("simple.docx");
    let mut c = Client::new(&mut doc);
    apply(&mut doc, &mut c, json!({"op":"set_default_format","format":{"size":12.0}}));
    fresh(&doc, &c);
}

#[test]
fn a_new_language_reaches_headers_and_notes() {
    let mut doc = build("rich");
    let mut c = Client::new(&mut doc);
    apply(&mut doc, &mut c, json!({"op":"set_language","lang":"es-es"}));
    fresh(&doc, &c);
}

#[test]
fn a_style_set_and_its_undo_reach_the_footer() {
    let mut doc = fixture("rich.docx");
    let mut c = Client::new(&mut doc);
    apply(&mut doc, &mut c, json!({"op":"set_style_set","style_set":"word-2010"}));
    fresh(&doc, &c);
    doc.undo();
    c.update(&mut doc);
    fresh(&doc, &c);
}

#[test]
fn replacing_a_font_and_its_undo_reach_headers_and_footers() {
    let mut doc = build("rich");
    let mut c = Client::new(&mut doc);
    let font = serde_json::to_value(doc.fonts_used()).unwrap()[0]["name"].as_str().unwrap_or("Calibri").to_string();
    apply(&mut doc, &mut c, json!({"op":"replace_font","from":font,"to":"Georgia"}));
    fresh(&doc, &c);
    doc.undo();
    c.update(&mut doc);
    fresh(&doc, &c);
}

#[test]
fn note_paragraph_ids_that_change_are_given_to_the_client() {
    let mut doc = fixture("complex.docx");
    let mut c = Client::new(&mut doc);
    apply(&mut doc, &mut c, json!({"op":"update_toc"}));
    fresh(&doc, &c);
}

// ---- 19: a style that appears changes the paragraphs that use it ------------------------------------------------------

#[test]
fn a_table_of_contents_added_twice_redraws_paragraphs_in_its_styles() {
    let mut doc = fixture("rich.docx");
    let mut c = Client::new(&mut doc);
    apply(&mut doc, &mut c, json!({"op":"insert_toc","before":5,"levels":1,"title":"Contents"}));
    fresh(&doc, &c);
    apply(&mut doc, &mut c, json!({"op":"insert_toc","before":33,"levels":3}));
    fresh(&doc, &c);
}

#[test]
fn the_first_link_recolours_the_links_that_were_already_there() {
    let mut doc = fixture("rich.docx");
    let mut c = Client::new(&mut doc);
    let p = first_para(&doc);
    apply(&mut doc, &mut c, json!({"op":"set_link","from":{"p":p,"o":0},"to":{"p":p,"o":3},"url":"https://example.com/a"}));
    fresh(&doc, &c);
}

// ---- 20: an edit inside a nested story redraws the paragraph around it ------------------------------------------------

#[test]
fn a_chart_in_a_text_box_is_redrawn_after_it_changes() {
    let mut doc = build("plain");
    let mut c = Client::new(&mut doc);
    let p = first_para(&doc);
    apply(&mut doc, &mut c, json!({"op":"insert_text_box","at":{"p":p,"o":5},"w":100,"h":40,"text":"","floating":true}));
    fresh(&doc, &c);
    // The text box holds one empty paragraph.
    let mut inner = 0;
    for st in doc.stories.iter().filter(|s| s.kind == StoryKind::TextBox) {
        walk_paragraphs(&st.blocks, &mut |p| inner = p.id);
    }
    assert!(inner != 0, "the text box has a paragraph");
    apply(&mut doc, &mut c, json!({"op":"insert_chart","at":{"p":inner,"o":0},"kind":"bubble","categories":[],"series":[{"name":"North","sizes":[10,20,30],"values":[3,5,7],"x_values":[1,2,3]}]}));
    fresh(&doc, &c);
    apply(&mut doc, &mut c, json!({"op":"set_chart","at":{"p":inner,"o":0},"title":"Prices"}));
    fresh(&doc, &c);
}

// ---- Typing stays cheap -------------------------------------------------------------------------------------------------

#[test]
fn typing_in_a_long_document_redraws_one_block_and_no_stories() {
    let mut doc = fixture("long.docx");
    let mut c = Client::new(&mut doc);
    let p = first_para(&doc);
    for i in 0..3 {
        let out = apply(&mut doc, &mut c, json!({"op":"insert_text","at":{"p":p,"o":i},"text":"x"}));
        assert_eq!(out["blocks"].as_array().unwrap().len(), 1, "only the touched block is redrawn");
        assert_eq!(out["stories"], json!(false), "typing does not redraw headers and notes");
        fresh(&doc, &c);
    }
}
