//! Citations and the bibliography: the sources part, citation fields and the bibliography field, written the
//! way Word writes them, in the five styles.

use docxtor_core::ai::{call_tool, docscript, ScriptOpts};
use docxtor_core::model::{find_block, paragraph_order, walk_paragraphs, write_block};
use docxtor_core::render::{RBlock, RSeg};
use docxtor_core::{bibtex, csl_json, package::Package, Document};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const BIB: &str = "http://schemas.openxmlformats.org/officeDocument/2006/bibliography";

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

pub(super) fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

pub(super) fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

pub(super) fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

pub(super) fn part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap_or_else(|| panic!("no part {name}")).to_vec()).unwrap()
}

pub(super) fn has_part(bytes: &[u8], name: &str) -> bool {
    Package::open(bytes).unwrap().has(name)
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

pub(super) fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

pub(super) fn smith() -> Value {
    json!({"type":"book","authors":[{"family":"Smith","given":"John"}],"title":"The Art of Citing","year":2020,"publisher":"Penguin","place":"New York"})
}

pub(super) fn lee() -> Value {
    json!({"type":"article","authors":[{"family":"Lee","given":"Karen"},{"family":"Kim","given":"Min-Jun"}],"title":"A study of things","containerTitle":"Journal of Stuff","year":"2021","month":"3","volume":"12","issue":"3","pages":"45-67","doi":"10.1000/xyz123"})
}

pub(super) fn add(doc: &mut Document, source: Value) -> String {
    let before = doc.sources().len();
    ops(doc, json!({"op":"add_source","source":source}));
    doc.sources()[before].tag.clone()
}

/// A document with two paragraphs of text and two sources.
pub(super) fn cited_doc() -> Document {
    let mut doc = doc_with("Citing is hard here.\n\nSecond paragraph about things.");
    add(&mut doc, smith());
    add(&mut doc, lee());
    doc
}

pub(super) fn cite(doc: &mut Document, para_text: &str, after: &str, tags: &[&str], page: Option<&str>) {
    let p = find_p(doc, para_text);
    let hay = doc.find_any_para(p).unwrap().plain_text();
    let o = hay.encode_utf16().count() - hay[hay.find(after).unwrap()..].encode_utf16().count() + after.encode_utf16().count();
    let mut op = json!({"op":"insert_citation","at":{"p":p,"o":o},"tags":tags});
    if let Some(pg) = page {
        op["page"] = json!(pg);
    }
    ops(doc, op);
}

pub(super) fn bib_texts(doc: &Document) -> Vec<String> {
    for b in &doc.body {
        if let docxtor_core::model::Block::Container(c) = b {
            let mut v = Vec::new();
            walk_paragraphs(&c.blocks, &mut |p| v.push(p.plain_text()));
            return v;
        }
    }
    panic!("no bibliography");
}

mod word_made;
mod sources;
mod citations_and_bibliography;
mod undo_import_and_ai;
mod other_shapes_and_editing_around;

#[allow(unused_imports)]
pub(super) use word_made::*;
#[allow(unused_imports)]
pub(super) use sources::*;
#[allow(unused_imports)]
pub(super) use citations_and_bibliography::*;
#[allow(unused_imports)]
pub(super) use undo_import_and_ai::*;
#[allow(unused_imports)]
pub(super) use other_shapes_and_editing_around::*;
