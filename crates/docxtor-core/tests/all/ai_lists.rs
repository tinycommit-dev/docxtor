//! The assistant's tools for the table of contents, the index, AutoMark, placeholders and building blocks.

use docxtor_core::ai::{call_tool, tool_definitions};
use docxtor_core::model::{walk_paragraphs, write_block};
use docxtor_core::Document;
use serde_json::{json, Value};

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn sample() -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"# Alpha\n\nText one.\n\n## Beta\n\nText two.\n\n### Gamma\n\nText three.\n\n# Delta\n\nEnd."}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn body_xml(doc: &Document) -> String {
    doc.body.iter().map(|b| write_block(b).to_xml()).collect()
}

#[test]
fn the_assistant_can_build_change_and_remove_a_table_of_contents() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    let r = call_tool(&mut doc, "insert_table_of_contents", &json!({"before": format!("p{first}"), "title": "Contents", "levels": 2, "page_numbers": false, "look": "modern"})).unwrap();
    assert!(r.contains("Table of contents inserted"), "{r}");
    assert!(texts(&doc).iter().any(|x| x == "Beta") && !texts(&doc).iter().any(|x| x.starts_with("Gamma\t")));
    let r = call_tool(&mut doc, "change_table_of_contents", &json!({"levels": 3, "page_numbers": true, "leader": "underscore"})).unwrap();
    assert!(r.contains("changed"), "{r}");
    assert!(body_xml(&doc).contains(r#"w:leader="underscore""#));
    let body = find_p(&doc, "Text one.");
    let r = call_tool(&mut doc, "add_table_of_contents_entry", &json!({"id": format!("p{body}"), "text": "A marked line", "level": 2})).unwrap();
    assert!(r.contains("Entry added"));
    call_tool(&mut doc, "change_table_of_contents", &json!({"table_entries": true})).unwrap();
    assert!(texts(&doc).iter().any(|x| x.starts_with("A marked line\t")));
    let r = call_tool(&mut doc, "remove_table_of_contents", &json!({})).unwrap();
    assert!(r.contains("removed"));
    assert!(call_tool(&mut doc, "change_table_of_contents", &json!({})).is_err());
}

#[test]
fn the_assistant_can_mark_and_shape_an_index_and_use_placeholders() {
    let mut doc = sample();
    let body = find_p(&doc, "Text one.");
    call_tool(&mut doc, "auto_mark_index", &json!({"entries": [{"text": "Alpha", "entry": "Letters:Alpha"}]})).unwrap();
    call_tool(&mut doc, "insert_index", &json!({"columns": 1, "right_align": false, "language": 3082})).unwrap();
    let r = call_tool(&mut doc, "change_index", &json!({"run_in": true})).unwrap();
    assert!(r.contains("Index changed"), "{r}");
    let r = call_tool(&mut doc, "insert_citation_placeholder", &json!({"id": format!("p{body}"), "tag": "Later"})).unwrap();
    assert!(r.contains("Placeholder cited"), "{r}");
    assert!(texts(&doc).iter().any(|x| x.contains("(Later)")), "{:?}", texts(&doc));
    assert!(call_tool(&mut doc, "list_building_blocks", &json!({})).unwrap().contains("no building blocks"));
    let names: Vec<String> = tool_definitions().as_array().unwrap().iter().filter_map(|t| t["name"].as_str().map(String::from)).collect();
    for n in ["insert_table_of_contents", "change_table_of_contents", "remove_table_of_contents", "add_table_of_contents_entry", "change_index", "auto_mark_index", "insert_citation_placeholder", "list_building_blocks", "insert_building_block"] {
        assert!(names.contains(&n.to_string()), "{n}");
    }
}
