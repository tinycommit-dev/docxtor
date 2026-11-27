//! AI tools for the table of contents (its options, entries and removal), index options and AutoMark. They are
//! merged into the tool list and executor of `ai`, next to the reference tools in `ai_refs`.

use crate::ai::{find_in_para, id_of, script_for_paragraphs};
use crate::document::Document;
use crate::index::IndexOptions;
use crate::ops::{AutoMarkEntry, Id, Op, Pos};
use crate::toc::Leader;
use crate::toc_spec::{StyleLevel, TocOptions};
use serde_json::{json, Value};

fn toc_schema(extra: Value) -> Value {
    let mut props = json!({
        "levels": {"type": "integer", "description": "Deepest heading level to list, 1 to 9 (0 lists no heading styles)"},
        "outline": {"type": "boolean", "description": "Also list paragraphs that have an outline level"},
        "styles": {"type": "array", "description": "Other paragraph styles to list and the level each one makes", "items": {"type": "object", "properties": {"style": {"type": "string"}, "level": {"type": "integer"}}}},
        "table_entries": {"type": "boolean", "description": "List table entry fields (add_table_of_contents_entry)"},
        "links": {"type": "boolean", "description": "Entries are links to their headings"},
        "page_numbers": {"type": "boolean", "description": "Show page numbers"},
        "right_align": {"type": "boolean", "description": "Page numbers at the right margin (otherwise they follow the entry)"},
        "leader": {"type": "string", "enum": ["dot", "hyphen", "underscore", "none"], "description": "The line between an entry and its page number"},
        "look": {"type": "string", "enum": ["classic", "formal", "modern", "simple"], "description": "Formatting of the TOC styles; omit to keep the document's"}
    });
    if let (Some(p), Some(e)) = (props.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            p.insert(k.clone(), v.clone());
        }
    }
    json!({"type": "object", "properties": props})
}

fn index_schema(extra: Value) -> Value {
    let mut props = json!({
        "columns": {"type": "integer", "description": "1 to 4"},
        "run_in": {"type": "boolean", "description": "Sub-entries run on in the entry's paragraph (otherwise they are indented below it)"},
        "headings": {"type": "boolean", "description": "A letter heading before each letter"},
        "right_align": {"type": "boolean", "description": "Page numbers at the right edge of the column (indented type)"},
        "leader": {"type": "string", "enum": ["dot", "hyphen", "underscore", "none"]},
        "page_separator": {"type": "string", "description": "Between an entry's page numbers (default \", \")"},
        "range_separator": {"type": "string", "description": "Between the first and last page of a range (default an en dash)"},
        "language": {"type": "integer", "description": "Windows locale id that decides the sort: 1033 English, 3082 Spanish, 1031 German, 1036 French"},
        "look": {"type": "string", "enum": ["classic", "modern", "simple"]}
    });
    if let (Some(p), Some(e)) = (props.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            p.insert(k.clone(), v.clone());
        }
    }
    json!({"type": "object", "properties": props})
}

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "insert_table_of_contents",
            "description": "Insert a table of contents (a Word TOC field) before a block or at the start. It lists the headings (and the other styles, outline levels and table entry fields you ask for) with page numbers. Update later with update_fields.",
            "input_schema": toc_schema(json!({"before": {"type": "string"}, "after": {"type": "string"}, "title": {"type": "string", "description": "A title above the table, such as Contents"}}))
        }),
        json!({
            "name": "change_table_of_contents",
            "description": "Change the options of the document's table of contents (the first one, or the one that has paragraph `id`) and build it again. Options you leave out stay as they are.",
            "input_schema": toc_schema(json!({"id": {"type": "string"}}))
        }),
        json!({
            "name": "remove_table_of_contents",
            "description": "Remove the table of contents (the one that has paragraph `id`, else the first) and the title above it.",
            "input_schema": {"type": "object", "properties": {"id": {"type": "string"}}}
        }),
        json!({
            "name": "add_table_of_contents_entry",
            "description": "Add a line to the table of contents for text that is not a heading: a hidden TC field right after some text in a paragraph. The table must list table entry fields (change_table_of_contents with table_entries) to show it.",
            "input_schema": {"type": "object", "required": ["id", "text"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the entry follows; default the end of the paragraph"},
                "text": {"type": "string", "description": "The line in the table"},
                "level": {"type": "integer", "description": "1 to 9 (default 1)"},
                "entry_type": {"type": "string", "description": "A letter for a separate table (default C)"},
                "no_page": {"type": "boolean"}
            }}
        }),
        json!({
            "name": "change_index",
            "description": "Change the options of the document's index (columns, indented or run-in, right-aligned page numbers, leader, separators, language, look) and build it again. Options you leave out stay as they are.",
            "input_schema": index_schema(json!({"id": {"type": "string", "description": "A paragraph of the index (default the first index)"}}))
        }),
        json!({
            "name": "insert_citation_placeholder",
            "description": "Cite a source that has only a tag so far (Word's Add new placeholder), right after some text in a paragraph. The citation shows the tag in brackets and the bibliography leaves the source out until it is filled in with update_source.",
            "input_schema": {"type": "object", "required": ["id"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the citation follows; default the end of the paragraph"},
                "tag": {"type": "string", "description": "Letters and digits, no spaces (default Placeholder1, Placeholder2...)"}
            }}
        }),
        json!({
            "name": "list_building_blocks",
            "description": "List the building blocks (quick parts, AutoText) the document's glossary holds: their names, galleries, categories and the start of their text.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "insert_building_block",
            "description": "Insert a building block of the document's glossary (see list_building_blocks) right after some text in a paragraph, with its formatting, styles, lists and pictures.",
            "input_schema": {"type": "object", "required": ["id", "name"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the block follows; default the end of the paragraph"},
                "name": {"type": "string", "description": "The block's name"}
            }}
        }),
        json!({
            "name": "auto_mark_index",
            "description": "Mark index entries from a list, like Word's AutoMark file: the first occurrence of each `text` in every paragraph (matching case) becomes an entry. `entry` is the index entry; `Fruit:Apple` makes the sub-entry Apple under Fruit. Then insert_index or update_fields.",
            "input_schema": {"type": "object", "required": ["entries"], "properties": {"entries": {"type": "array", "items": {"type": "object", "required": ["text"], "properties": {"text": {"type": "string"}, "entry": {"type": "string"}}}}}}
        }),
    ]
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}

fn flag(args: &Value, k: &str) -> Option<bool> {
    args.get(k).and_then(|v| v.as_bool())
}

fn leader(args: &Value) -> Result<Option<Leader>, String> {
    match s(args, "leader") {
        None => Ok(None),
        Some(l) => Leader::parse(&l).map(Some).ok_or_else(|| "leader must be dot, hyphen, underscore or none".into()),
    }
}

fn opt_id(args: &Value, k: &str) -> Result<Option<Id>, String> {
    match args.get(k) {
        Some(v) if !v.is_null() => Ok(Some(Id(id_of(v)?))),
        _ => Ok(None),
    }
}

/// The options in the arguments laid over `base`.
fn toc_options(args: &Value, base: TocOptions) -> Result<TocOptions, String> {
    let mut o = base;
    if let Some(l) = args.get("levels").and_then(|v| v.as_u64()) {
        o.levels = l.min(9) as u8;
    }
    if let Some(v) = flag(args, "outline") {
        o.outline = v;
    }
    if let Some(list) = args.get("styles").and_then(|v| v.as_array()) {
        o.styles = list
            .iter()
            .filter_map(|e| Some(StyleLevel { style: e.get("style")?.as_str()?.to_string(), level: e.get("level")?.as_u64()?.clamp(1, 9) as u8 }))
            .collect();
    }
    if let Some(v) = flag(args, "table_entries") {
        o.entries = v;
    }
    if let Some(v) = flag(args, "links") {
        o.links = v;
    }
    if let Some(v) = flag(args, "page_numbers") {
        o.page_numbers = v;
    }
    if let Some(v) = flag(args, "right_align") {
        o.right_align = v;
    }
    if let Some(l) = leader(args)? {
        o.leader = l;
    }
    if let Some(l) = s(args, "look") {
        o.look = Some(l);
    }
    Ok(o)
}

fn index_options(args: &Value, base: IndexOptions) -> Result<IndexOptions, String> {
    let mut o = base;
    if let Some(c) = args.get("columns").and_then(|v| v.as_u64()) {
        o.columns = c.clamp(1, 4) as u8;
    }
    if let Some(v) = flag(args, "run_in") {
        o.run_in = v;
    }
    if let Some(v) = flag(args, "headings") {
        o.headings = v;
    }
    if let Some(v) = flag(args, "right_align") {
        o.right_align = v;
    }
    if let Some(l) = leader(args)? {
        o.leader = l;
    }
    if let Some(v) = s(args, "page_separator") {
        o.page_sep = Some(v);
    }
    if let Some(v) = s(args, "range_separator") {
        o.range_sep = Some(v);
    }
    if let Some(v) = args.get("language").and_then(|v| v.as_u64()) {
        o.language = Some(v as u32);
    }
    if let Some(l) = s(args, "look") {
        o.look = Some(l);
    }
    Ok(o)
}

fn insert_toc(doc: &mut Document, args: &Value) -> Result<String, String> {
    let options = toc_options(args, TocOptions::default())?;
    let op = Op::InsertToc { after: opt_id(args, "after")?, before: opt_id(args, "before")?, levels: options.levels.max(1), title: s(args, "title"), pages: None, options: Some(options) };
    let r = doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    let mut created = r[0].created.clone();
    let n = created.len();
    created.truncate(30);
    Ok(format!("Table of contents inserted ({n} paragraph(s)); page numbers are estimated until the editor lays out the pages:\n{}", script_for_paragraphs(doc, &created)))
}

fn change_toc(doc: &mut Document, args: &Value) -> Result<String, String> {
    let at = opt_id(args, "id")?;
    let base = doc.toc_options(at.map(|i| i.0)).ok_or("the document has no table of contents")?;
    let options = toc_options(args, base)?;
    let r = doc.apply_batch(&[Op::SetToc { at, options, pages: None }], false).map_err(|e| e.to_string())?;
    let mut created = r[0].created.clone();
    created.truncate(30);
    Ok(format!("Table of contents changed:\n{}", script_for_paragraphs(doc, &created)))
}

fn remove_toc(doc: &mut Document, args: &Value) -> Result<String, String> {
    let at = opt_id(args, "id")?;
    doc.apply_batch(&[Op::RemoveToc { at }], false).map_err(|e| e.to_string())?;
    Ok("Table of contents removed.".into())
}

fn add_entry(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let text = s(args, "text").ok_or("text is required")?;
    let at = match s(args, "after_text") {
        Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
        _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
    };
    let op = Op::InsertTocEntry { at: Pos::new(id, at), text, level: args.get("level").and_then(|v| v.as_u64()).unwrap_or(1).clamp(1, 9) as u8, id: s(args, "entry_type"), no_page: flag(args, "no_page").unwrap_or(false) };
    doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    Ok(format!("Entry added in p{id} (the field is hidden). Update the table with update_fields."))
}

fn end_of(doc: &Document, id: crate::model::NodeId, after_text: Option<String>) -> Result<usize, String> {
    match after_text {
        Some(t) if !t.is_empty() => Ok(find_in_para(doc, id, &t, false)?[0].1),
        _ => Ok(doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16()),
    }
}

fn placeholder(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let at = end_of(doc, id, s(args, "after_text"))?;
    doc.apply_batch(&[Op::InsertCitationPlaceholder { at: Pos::new(id, at), tag: s(args, "tag") }], false).map_err(|e| e.to_string())?;
    Ok(format!("Placeholder cited:\n{}", script_for_paragraphs(doc, &[id])))
}

fn list_blocks(doc: &Document) -> String {
    let all = doc.building_blocks();
    if all.is_empty() {
        return "The document has no building blocks.".into();
    }
    all.iter().map(|b| format!("{}: gallery {}{}{} {:?}\n", b.name, b.gallery.as_deref().unwrap_or("-"), b.category.as_ref().map(|c| format!(", category {c}")).unwrap_or_default(), if b.inline { ", inline" } else { "" }, b.text)).collect()
}

fn insert_block(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let name = s(args, "name").ok_or("name is required")?;
    let at = end_of(doc, id, s(args, "after_text"))?;
    doc.apply_batch(&[Op::InsertBuildingBlock { at: Pos::new(id, at), index: None, guid: None, name: Some(name) }], false).map_err(|e| e.to_string())?;
    Ok(format!("Building block inserted:\n{}", script_for_paragraphs(doc, &[id])))
}

fn change_index(doc: &mut Document, args: &Value) -> Result<String, String> {
    let at = opt_id(args, "id")?;
    let base = doc.index_options(at.map(|i| i.0)).ok_or("the document has no index")?;
    let options = index_options(args, base)?;
    let r = doc.apply_batch(&[Op::SetIndex { at, options, pages: None }], false).map_err(|e| e.to_string())?;
    let mut created = r[0].created.clone();
    created.truncate(30);
    Ok(format!("Index changed:\n{}", script_for_paragraphs(doc, &created)))
}

fn auto_mark(doc: &mut Document, args: &Value) -> Result<String, String> {
    let list = args.get("entries").and_then(|v| v.as_array()).ok_or("entries is required")?;
    let entries: Vec<AutoMarkEntry> = list
        .iter()
        .filter_map(|e| Some(AutoMarkEntry { text: e.get("text")?.as_str()?.to_string(), entry: e.get("entry").and_then(|v| v.as_str()).unwrap_or_default().to_string(), bold: false, italic: false }))
        .collect();
    let r = doc.apply_batch(&[Op::AutoMarkIndex { entries }], false).map_err(|e| e.to_string())?;
    Ok(format!("Marked {} place(s) for the index (the marks are hidden).", r[0].count.unwrap_or(0)))
}

/// The index options the `insert_index` tool asks for, if it asks for any beyond columns, run-in and headings.
pub fn index_options_from(args: &Value) -> Result<Option<IndexOptions>, String> {
    const KEYS: [&str; 6] = ["right_align", "leader", "page_separator", "range_separator", "language", "look"];
    if !KEYS.iter().any(|k| args.get(k).is_some()) {
        return Ok(None);
    }
    index_options(args, IndexOptions::default()).map(Some)
}

/// Run one of these tools; None when the name isn't one of them.
pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "insert_table_of_contents" => insert_toc(doc, args),
        "change_table_of_contents" => change_toc(doc, args),
        "remove_table_of_contents" => remove_toc(doc, args),
        "add_table_of_contents_entry" => add_entry(doc, args),
        "change_index" => change_index(doc, args),
        "auto_mark_index" => auto_mark(doc, args),
        "insert_citation_placeholder" => placeholder(doc, args),
        "list_building_blocks" => Ok(list_blocks(doc)),
        "insert_building_block" => insert_block(doc, args),
        _ => return None,
    })
}
