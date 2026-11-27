//! AI tools for endnotes, captions, cross-references, the index, tables of figures and updating fields.
//! They live apart from `ai` and are merged into its tool list and executor.

use crate::ai::{find_in_para, id_of, script_for_paragraphs};
use crate::document::Document;
use crate::model::NodeId;
use crate::ops::{CaptionPosition, ChapterNumbering, Id, Op, Pos, XrefShow, XrefTarget};
use serde_json::{json, Value};
use std::fmt::Write as _;

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    let target = json!({
        "type": "object",
        "description": "What to point at: {\"type\":\"heading\",\"p\":\"p12\"}, {\"type\":\"numbered\",\"p\":\"p14\"} (a numbered list item), {\"type\":\"caption\",\"p\":\"p30\"}, {\"type\":\"bookmark\",\"name\":\"Intro\"}, {\"type\":\"footnote\",\"id\":\"2\"} or {\"type\":\"endnote\",\"id\":\"1\"}. Ids and names come from list_cross_reference_targets.",
        "properties": {"type": {"type": "string", "enum": ["heading", "numbered", "caption", "bookmark", "footnote", "endnote"]}}
    });
    vec![
        json!({
            "name": "insert_endnote",
            "description": "Add an endnote (listed at the end of the document, numbered i, ii, iii unless the document says otherwise) right after some text in a paragraph. Numbering is automatic.",
            "input_schema": {"type": "object", "required": ["id", "after_text", "text"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the note mark follows (first occurrence)"},
                "text": {"type": "string", "description": "The note's text"}
            }}
        }),
        json!({
            "name": "insert_caption",
            "description": "Insert a numbered caption paragraph (style Caption) next to a block, such as a table or an image paragraph: \"Figure 1: text\". Numbers are per label (Figure, Table, Equation, or any label) and follow document order; later captions renumber. Pass `after` to put the caption below the block, `before` to put it above.",
            "input_schema": {"type": "object", "required": ["label"], "properties": {
                "after": {"type": "string", "description": "Block id to put the caption after (below)"},
                "before": {"type": "string", "description": "Block id to put the caption before (above)"},
                "label": {"type": "string", "description": "Figure, Table, Equation or your own"},
                "text": {"type": "string", "description": "The caption text after the number"},
                "separator": {"type": "string", "description": "Between the number and the text (default \": \")"},
                "chapter_level": {"type": "integer", "description": "Number the caption with the chapter it is in (\"Figure 2-1\"): the heading level, 1 to 9, whose number is the chapter. The headings must be numbered."},
                "chapter_separator": {"type": "string", "description": "Between the chapter and the number (default \"-\")"}
            }}
        }),
        json!({
            "name": "list_cross_reference_targets",
            "description": "List what a cross-reference can point at: numbered headings, captions, bookmarks, footnotes and endnotes, with the ids to use in insert_cross_reference.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "insert_cross_reference",
            "description": "Insert a live cross-reference (a Word REF, PAGEREF or NOTEREF field) in a paragraph, after some text. `show` is text (the heading, bookmarked text or whole caption), number (heading or list number, a caption's number, a note's number), page, label_number (a caption's \"Figure 1\"), caption_text, number_no_context, number_full_context or number_formatted (a note's mark). Update later with update_fields.",
            "input_schema": {"type": "object", "required": ["id", "target", "show"], "properties": {
                "id": {"type": "string", "description": "Paragraph to put the reference in"},
                "after_text": {"type": "string", "description": "Exact text the reference follows (first occurrence); default is the end of the paragraph"},
                "target": target,
                "show": {"type": "string", "enum": ["text", "number", "page", "label_number", "caption_text", "number_no_context", "number_full_context", "number_formatted"]},
                "link": {"type": "boolean", "description": "Make it a link to the target (default true)"},
                "above_below": {"type": "boolean", "description": "Add \"above\" or \"below\" after it"}
            }}
        }),
        json!({
            "name": "mark_index_entry",
            "description": "Mark text for the index (a hidden XE field, nothing shows in the text). By default marks right after the first occurrence of `text` in the paragraph; with `all`, marks the first occurrence in every paragraph. `entry` is the index entry (default: the text), `sub` a sub-entry, `see` text such as \"See Fruit\" shown instead of page numbers. Then use insert_index.",
            "input_schema": {"type": "object", "required": ["text"], "properties": {
                "id": {"type": "string", "description": "Paragraph id (not needed with all)"},
                "text": {"type": "string", "description": "Exact text to mark"},
                "entry": {"type": "string"}, "sub": {"type": "string"}, "see": {"type": "string"},
                "bold": {"type": "boolean"}, "italic": {"type": "boolean"},
                "all": {"type": "boolean", "description": "Mark every paragraph that has the text"}
            }}
        }),
        json!({
            "name": "insert_index",
            "description": "Insert an index built from the marked entries (mark_index_entry): sorted, sub-entries indented, page numbers merged. It goes in its own continuous section with columns (default 2). Insert it after a block, or at the end. Update later with update_fields.",
            "input_schema": {"type": "object", "properties": {
                "after": {"type": "string"}, "before": {"type": "string"},
                "columns": {"type": "integer", "description": "1 to 4 (default 2)"},
                "run_in": {"type": "boolean", "description": "Sub-entries run on in the entry's paragraph"},
                "headings": {"type": "boolean", "description": "A letter heading before each letter"},
                "right_align": {"type": "boolean", "description": "Page numbers at the right edge of the column (indented type)"},
                "leader": {"type": "string", "enum": ["dot", "hyphen", "underscore", "none"]},
                "page_separator": {"type": "string"}, "range_separator": {"type": "string"},
                "language": {"type": "integer", "description": "Windows locale id that decides the sort (1033 English, 3082 Spanish)"},
                "look": {"type": "string", "enum": ["classic", "modern", "simple"]}
            }}
        }),
        json!({
            "name": "insert_table_of_figures",
            "description": "Insert a table of figures listing the captions with a label (Figure, Table, ...) and their page numbers. Update later with update_fields.",
            "input_schema": {"type": "object", "properties": {
                "after": {"type": "string"}, "before": {"type": "string"},
                "label": {"type": "string", "description": "Default Figure"}
            }}
        }),
        json!({
            "name": "update_fields",
            "description": "Update the document's fields like Word's F9: caption numbers, cross-references, the table of contents, tables of figures, the index, and fields that show a value (DATE, TIME, AUTHOR, TITLE, DOCPROPERTY, NUMWORDS, IF, SET, QUOTE, formulas and the like, with their \\@, \\# and \\* switches). Locked fields stay. Outside the editor, page numbers are estimated from page breaks and NUMPAGES is left alone.",
            "input_schema": {"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}, "description": "Only fields in these paragraphs"}
            }}
        }),
        json!({
            "name": "insert_field",
            "description": "Insert a Word field in a paragraph, after some text. `field` is the field instruction without braces, such as `DATE \\@ \"MMMM d, yyyy\"`, `AUTHOR`, `DOCPROPERTY Company`, `NUMWORDS`, `PAGE`, `= SUM(ABOVE)`, `IF 3 > 2 \"yes\" \"no\"`, `STYLEREF \"Heading 1\"`. Its result is worked out when inserted where possible; update later with update_fields.",
            "input_schema": {"type": "object", "required": ["id", "field"], "properties": {
                "id": {"type": "string", "description": "Paragraph to put the field in"},
                "after_text": {"type": "string", "description": "Exact text the field follows (first occurrence); default is the end of the paragraph"},
                "field": {"type": "string", "description": "The field instruction"}
            }}
        }),
        json!({
            "name": "list_fields",
            "description": "List the fields in the document: the paragraph, the instruction and the result of each.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "set_field_code",
            "description": "Change the instruction of the field that is in a paragraph (the nth one with `index`, default the first) and work out its result again.",
            "input_schema": {"type": "object", "required": ["id", "field"], "properties": {
                "id": {"type": "string"},
                "index": {"type": "integer", "description": "Which field of the paragraph, from 0"},
                "field": {"type": "string", "description": "The new instruction"}
            }}
        }),
    ]
}

/// The time now for fields that show it. The browser build has no clock; the page passes the time in.
fn tool_input() -> Option<crate::field_values::FieldInput> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let now = crate::docprops::now_iso();
        Some(crate::field_values::FieldInput { now: Some(now.trim_end_matches('Z').to_string()), ..Default::default() })
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

fn insert_field(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let field = s(args, "field").ok_or("field is required")?;
    let at = end_of(doc, id, s(args, "after_text"))?;
    doc.apply_batch(&[Op::InsertField { at: Pos::new(id, at), field, result: None, input: tool_input() }], false).map_err(|e| e.to_string())?;
    Ok(format!("Field inserted:\n{}", script_for_paragraphs(doc, &[id])))
}

fn list_fields(doc: &Document) -> String {
    let all = doc.list_fields();
    if all.is_empty() {
        return "The document has no fields.".into();
    }
    let mut out = String::new();
    for f in all {
        let _ = writeln!(out, "p{} @{}{}: {} => {:?}", f.p, f.o, if f.story { " (header, footer or note)" } else { "" }, f.instr.trim(), f.result);
    }
    out
}

fn set_field_code(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let field = s(args, "field").ok_or("field is required")?;
    let n = args.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let at = doc.list_fields().into_iter().filter(|f| f.p == id).nth(n).ok_or(format!("paragraph p{id} has no field number {n}"))?;
    doc.apply_batch(&[Op::SetFieldCode { at: Pos::new(id, at.o), instr: field, input: tool_input(), pages: None }], false).map_err(|e| e.to_string())?;
    Ok(format!("Field changed:\n{}", script_for_paragraphs(doc, &[id])))
}

fn opt_id(args: &Value, k: &str) -> Result<Option<Id>, String> {
    match args.get(k) {
        Some(v) if !v.is_null() => Ok(Some(Id(id_of(v)?))),
        _ => Ok(None),
    }
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}

fn b(args: &Value, k: &str) -> bool {
    args.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Where in a paragraph a reference or mark goes: after `after_text`, else at the end.
fn end_of(doc: &Document, id: NodeId, after_text: Option<String>) -> Result<usize, String> {
    match after_text {
        Some(t) if !t.is_empty() => Ok(find_in_para(doc, id, &t, false)?[0].1),
        _ => Ok(doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16()),
    }
}

/// Run one of these tools; None when the name isn't one of them.
pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "insert_endnote" => insert_endnote(doc, args),
        "insert_caption" => insert_caption(doc, args),
        "list_cross_reference_targets" => Ok(list_targets(doc)),
        "insert_cross_reference" => insert_cross_reference(doc, args),
        "mark_index_entry" => mark_index_entry(doc, args),
        "insert_index" => insert_index(doc, args),
        "insert_table_of_figures" => insert_table_of_figures(doc, args),
        "update_fields" => update_fields(doc, args),
        "insert_field" => insert_field(doc, args),
        "list_fields" => Ok(list_fields(doc)),
        "set_field_code" => set_field_code(doc, args),
        _ => return None,
    })
}

fn insert_endnote(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let after = s(args, "after_text").ok_or("after_text is required")?;
    let ranges = find_in_para(doc, id, &after, false)?;
    let r = doc.apply_batch(&[Op::InsertEndnote { at: Pos::new(id, ranges[0].1), text: s(args, "text").unwrap_or_default() }], false).map_err(|e| e.to_string())?;
    let mut ids = vec![id];
    ids.extend(r.first().and_then(|r| r.caret).map(|c| c.p.0));
    Ok(format!("Endnote added:\n{}", script_for_paragraphs(doc, &ids)))
}

fn insert_caption(doc: &mut Document, args: &Value) -> Result<String, String> {
    let op = Op::InsertCaption {
        after: opt_id(args, "after")?,
        before: opt_id(args, "before")?,
        label: s(args, "label").ok_or("label is required")?,
        text: s(args, "text").unwrap_or_default(),
        position: match s(args, "position").as_deref() {
            Some("above") => Some(CaptionPosition::Above),
            Some("below") => Some(CaptionPosition::Below),
            Some(_) => return Err("position must be above or below".into()),
            None => None,
        },
        separator: s(args, "separator"),
        chapter: args.get("chapter_level").and_then(|v| v.as_u64()).map(|l| ChapterNumbering { level: l.clamp(1, 9) as u8, separator: s(args, "chapter_separator") }),
    };
    let r = doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    let created = r[0].created.clone();
    Ok(format!("Caption inserted:\n{}", script_for_paragraphs(doc, &created)))
}

fn list_targets(doc: &Document) -> String {
    let t = doc.xref_targets();
    let mut out = String::new();
    let _ = writeln!(out, "Headings:");
    for h in &t.headings {
        let _ = writeln!(out, "  p{} level {}{} {:?}", h.p, h.level, h.number.as_ref().map(|n| format!(" number {n}")).unwrap_or_default(), h.text);
    }
    let _ = writeln!(out, "Captions:");
    for c in &t.captions {
        let _ = writeln!(out, "  p{} {} {} {:?}", c.p, c.label, c.number, c.text);
    }
    let _ = writeln!(out, "Bookmarks:");
    for b in &t.bookmarks {
        let _ = writeln!(out, "  {} {:?}", b.name, b.text);
    }
    let _ = writeln!(out, "Footnotes:");
    for n in &t.footnotes {
        let _ = writeln!(out, "  id {} (shown {}) {:?}", n.id, n.label, n.text);
    }
    let _ = writeln!(out, "Endnotes:");
    for n in &t.endnotes {
        let _ = writeln!(out, "  id {} (shown {}) {:?}", n.id, n.label, n.text);
    }
    out
}

fn insert_cross_reference(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let target: XrefTarget = serde_json::from_value(args.get("target").cloned().unwrap_or(Value::Null)).map_err(|e| format!("target: {e}"))?;
    let show: XrefShow = serde_json::from_value(args.get("show").cloned().unwrap_or(Value::Null))
        .map_err(|_| "show must be one of text, number, page, label_number, caption_text, number_no_context, number_full_context, number_formatted".to_string())?;
    let at = end_of(doc, id, s(args, "after_text"))?;
    let link = args.get("link").and_then(|v| v.as_bool()).unwrap_or(true);
    doc.apply_batch(&[Op::InsertCrossReference { at: Pos::new(id, at), target, show, link, above_below: b(args, "above_below"), pages: None }], false).map_err(|e| e.to_string())?;
    Ok(format!("Cross-reference inserted:\n{}", script_for_paragraphs(doc, &[id])))
}

fn mark_index_entry(doc: &mut Document, args: &Value) -> Result<String, String> {
    let text = s(args, "text").ok_or("text is required")?;
    let (entry, sub, see) = (s(args, "entry"), s(args, "sub"), s(args, "see"));
    if b(args, "all") {
        let r = doc
            .apply_batch(&[Op::MarkAllIndexEntries { text: text.clone(), entry, sub, see, bold: b(args, "bold"), italic: b(args, "italic"), ignore_case: false, whole_word: false }], false)
            .map_err(|e| e.to_string())?;
        return Ok(format!("Marked {} paragraph(s) containing {text:?} for the index.", r[0].count.unwrap_or(0)));
    }
    let id = id_of(args.get("id").ok_or("id is required (or set all)")?)?;
    let end = find_in_para(doc, id, &text, false)?[0].1;
    doc.apply_batch(
        &[Op::MarkIndexEntry { at: Pos::new(id, end), entry: entry.unwrap_or_else(|| text.clone()), sub, see, bold: b(args, "bold"), italic: b(args, "italic") }],
        false,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!("Marked {text:?} in p{id} for the index (the mark is hidden)."))
}

fn insert_index(doc: &mut Document, args: &Value) -> Result<String, String> {
    let op = Op::InsertIndex {
        after: opt_id(args, "after")?,
        before: opt_id(args, "before")?,
        columns: args.get("columns").and_then(|v| v.as_u64()).unwrap_or(2).clamp(1, 4) as u8,
        run_in: b(args, "run_in"),
        headings: b(args, "headings"),
        pages: None,
        options: crate::ai_lists::index_options_from(args)?,
    };
    let r = doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    let mut created = r[0].created.clone();
    let n = created.len();
    created.truncate(30);
    Ok(format!("Index inserted ({n} paragraph(s)):\n{}", script_for_paragraphs(doc, &created)))
}

fn insert_table_of_figures(doc: &mut Document, args: &Value) -> Result<String, String> {
    let op = Op::InsertTableOfFigures { after: opt_id(args, "after")?, before: opt_id(args, "before")?, label: s(args, "label").unwrap_or_else(|| "Figure".into()), pages: None };
    let r = doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    let mut created = r[0].created.clone();
    let n = created.len();
    created.truncate(30);
    Ok(format!("Table of figures inserted ({n} paragraph(s)):\n{}", script_for_paragraphs(doc, &created)))
}

fn update_fields(doc: &mut Document, args: &Value) -> Result<String, String> {
    let scope = match args.get("ids") {
        Some(Value::Array(a)) if !a.is_empty() => Some(a.iter().map(|x| id_of(x).map(Id)).collect::<Result<Vec<_>, _>>()?),
        _ => None,
    };
    let r = doc.apply_batch(&[Op::UpdateFields { pages: None, scope, quiet: false, open: false, only: None, input: tool_input() }], false).map_err(|e| e.to_string())?;
    Ok(format!("Updated {} field(s). Page numbers are estimated from page breaks.", r[0].count.unwrap_or(0)))
}
