//! Found by the random op harness: what a client holds after
//! `render_changed` must equal a full render. The client here is the same as in `random_ops.rs`: it keeps the blocks it
//! was given and re-reads the page setup only when `render_changed` says `stories` changed.

use crate::common;

use common::*;
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

#[track_caller]
fn fresh(doc: &Document, c: &Client) {
    if let Some(d) = c.stale(doc) {
        panic!("the client is stale: {d} (line {})", std::panic::Location::caller().line());
    }
}

fn fixture(name: &str) -> Document {
    Start::File(name.into()).open()
}


#[test]
fn a_custom_mark_note_added_after_a_field_update_reaches_the_client() {
    let mut doc = fixture("complex.docx");
    let mut c = Client::new(&mut doc);
    apply(&mut doc, &mut c, json!({"op":"insert_paragraph","before":6,"style":"Quote","text":"{ DATE } { PAGE }"}));
    fresh(&doc, &c);
    apply(&mut doc, &mut c, json!({"op":"update_fields"}));
    fresh(&doc, &c);
    apply(&mut doc, &mut c, json!({"op":"insert_custom_note","kind":"footnote","at":{"p":10,"o":6},"mark":"*","text":"Starred."}));
    fresh(&doc, &c);
    apply(&mut doc, &mut c, json!({"op":"update_toc","pages_only":false}));
    fresh(&doc, &c);
}

/// L8: the page setup lists notes in the order the text refers to them. Notes with a mark of their own have no number, so
/// moving the paragraphs they hang from changes the order without changing any number.
#[test]
fn moving_paragraphs_with_marked_notes_reorders_the_notes_for_the_client() {
    let mut doc = fixture("complex.docx");
    let mut c = Client::new(&mut doc);
    let paras: Vec<u64> = serde_json::to_value(doc.render_snapshot().blocks).unwrap().as_array().unwrap().iter().filter(|b| b["t"] == "p" && b["len"].as_u64().unwrap_or(0) > 3).map(|b| b["id"].as_u64().unwrap()).collect();
    let (a, b) = (paras[0], paras[1]);
    apply(&mut doc, &mut c, json!({"op":"insert_custom_note","kind":"footnote","at":{"p":a,"o":0},"mark":"*","text":"First."}));
    fresh(&doc, &c);
    apply(&mut doc, &mut c, json!({"op":"insert_custom_note","kind":"footnote","at":{"p":b,"o":0},"mark":"+","text":"Second."}));
    fresh(&doc, &c);
    let last = *doc.render_snapshot().order.last().unwrap();
    apply(&mut doc, &mut c, json!({"op":"move_blocks","ids":[a],"after":last}));
    fresh(&doc, &c);
}

// ---- L5: a drawing's id is not a paragraph id ----------------------------------------------------------------------------

/// Opening a file hands out paragraph ids 1, 2, 3 ... while a drawing keeps the id the file gave it. The harness numbers
/// ids by first appearance, so a drawing id that equals a paragraph id after reopening used to number that paragraph
/// early and report a difference that is not one (seed 5300289: shape id 5001, the 5,001st paragraph).
#[test]
fn a_drawing_id_that_equals_a_paragraph_id_after_reopening_is_not_a_difference() {
    let shape = |id: u64| json!({"t":"p","id":id,"segs":[{"k":"shape","shape":{"id":5001,"kind":"shape","children":[{"id":5001,"kind":"picture"}],"text":[{"t":"p","id":id + 1}]}}]});
    let before = json!({"body":{"blocks":[shape(7914), {"t":"p","id":7916}]}});
    let after = json!({"body":{"blocks":[shape(5000), {"t":"p","id":5002}]}});
    assert_eq!(first_diff(&normalise(&before), &normalise(&after)), None);
    // Paragraphs are still numbered: a real difference in paragraph ids is still found.
    let moved = json!({"body":{"blocks":[shape(5000), {"t":"p","id":5000}]}});
    assert!(first_diff(&normalise(&before), &normalise(&moved)).is_some());
}
