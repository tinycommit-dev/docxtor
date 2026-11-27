//! Endnotes, captions, cross-references, updating fields, the index and the table of figures: the ops, the
//! markup they write, what the renderer shows and the AI tools.

use docxtor_core::ai::{call_tool, docscript, ScriptOpts};
use docxtor_core::model::{find_block, walk_paragraphs, write_block};
use docxtor_core::stories::StoryKind;
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

/// Every XML part of a saved package must be namespace-well-formed.
pub(super) fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

pub(super) fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}; run python3 fixtures/make_fixtures.py"))
}

pub(super) fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

pub(super) fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

pub(super) fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

pub(super) fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

pub(super) fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

pub(super) fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

pub(super) fn body_xml(doc: &Document) -> String {
    doc.body.iter().map(|b| write_block(b).to_xml()).collect()
}

pub(super) fn saved_part(doc: &mut Document, name: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

pub(super) fn story_text(doc: &Document, kind: StoryKind) -> String {
    let mut out = String::new();
    for s in doc.stories.iter().filter(|s| s.kind == kind) {
        walk_paragraphs(&s.blocks, &mut |p| out.push_str(&p.plain_text()));
    }
    out
}

/// Reopen a document after editing its package.
pub(super) fn rewrite(doc: &mut Document, f: impl FnOnce(&mut Package)) -> Document {
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    f(&mut pkg);
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

pub(super) fn set_part(pkg: &mut Package, name: &str, f: impl FnOnce(String) -> String) {
    let s = String::from_utf8(pkg.get(name).unwrap().to_vec()).unwrap();
    pkg.set(name, f(s).into_bytes());
}

pub(super) fn reopen(doc: &mut Document) -> Document {
    Document::open(&doc.save().unwrap()).unwrap()
}

/// A blank document whose body is the given XML (before the section properties).
pub(super) fn doc_from_body(xml: &str) -> Document {
    let mut blank = Document::new_blank();
    rewrite(&mut blank, |pkg| {
        set_part(pkg, "word/document.xml", |s| s.replacen("<w:p/>", xml, 1));
    })
}

/// The rendered segments of a paragraph: text (or note id) and link.
pub(super) fn segs(doc: &Document, id: u32) -> Vec<Value> {
    let r = serde_json::to_value(doc.render_snapshot()).unwrap();
    fn find(blocks: &Value, id: u32) -> Option<Value> {
        for b in blocks.as_array()? {
            match b["t"].as_str() {
                Some("p") if b["id"] == id => return Some(b["segs"].clone()),
                Some("tbl") => {
                    for row in b["rows"].as_array()? {
                        for c in row["cells"].as_array()? {
                            if let Some(s) = find(&c["blocks"], id) {
                                return Some(s);
                            }
                        }
                    }
                }
                Some("c") => {
                    if let Some(s) = find(&b["blocks"], id) {
                        return Some(s);
                    }
                }
                _ => {}
            }
        }
        None
    }
    find(&r["blocks"], id).expect("rendered paragraph").as_array().unwrap().clone()
}

/// Pages for the layout-measured `pages` map, from paragraph ids.
pub(super) fn pages(list: &[(u32, u32)]) -> Value {
    let mut m = serde_json::Map::new();
    for (p, n) in list {
        m.insert(p.to_string(), json!(n));
    }
    Value::Object(m)
}

pub(super) fn note_labels(doc: &Document, endnotes: bool) -> Vec<String> {
    let ps = doc.page_setup();
    let list = if endnotes { &ps.endnotes } else { &ps.notes };
    list.iter().map(|n| n.label.clone()).collect()
}

pub(super) fn body_labels(doc: &Document, p: u32) -> Vec<(String, Option<String>)> {
    segs(doc, p).iter().filter(|s| s["k"] == "note").map(|s| (s["label"].as_str().unwrap().to_string(), s["note"].as_str().map(String::from))).collect()
}

mod endnotes;
mod captions_and_cross_references;
mod updating_and_index;
mod figures_and_word_samples;
mod ai_tools;

#[allow(unused_imports)]
pub(super) use endnotes::*;
#[allow(unused_imports)]
pub(super) use captions_and_cross_references::*;
#[allow(unused_imports)]
pub(super) use updating_and_index::*;
#[allow(unused_imports)]
pub(super) use figures_and_word_samples::*;
#[allow(unused_imports)]
pub(super) use ai_tools::*;
