//! AI tools for the Review tab: counting words and inspecting a document for personal information and hidden
//! data. They live apart from `ai` and are merged into its tool list and executor.

use crate::document::Document;
use crate::ops::Op;
use serde_json::{json, Value};
use std::fmt::Write as _;

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "count_words",
            "description": "Count the document's paragraphs, words, characters and characters without spaces. The count is of the main text (tables included); notes and text boxes are counted apart, and `include_extras` adds them to the totals as Word's \"Include textboxes, footnotes and endnotes\" does. Headers and footers are never counted. Pages and lines depend on the layout and are not counted here.",
            "input_schema": {"type": "object", "properties": {
                "include_extras": {"type": "boolean", "description": "Add footnotes, endnotes and text boxes to the totals"}
            }}
        }),
        json!({
            "name": "inspect_document",
            "description": "List what the file carries besides what the page shows (Word's Document Inspector): personal information in the properties (author, last saved by, company, manager, custom properties, the preview picture), the names behind comments and tracked changes, comments, tracked changes, hidden text, header and footer text, and custom XML data. Each line says how many there are. Use it before sharing a file; remove what the person wants gone with remove_hidden_data.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "remove_hidden_data",
            "description": "Take personal information and hidden data out of the document (one undo step). `what` lists the categories from inspect_document: properties (names, company, custom properties and the preview picture are removed), authors (names behind comments and tracked changes become \"Author\"), comments (all of them), revisions (tracked changes are accepted, so deletions are gone for good), hiddenText, headersFooters (headers, footers and the watermark in them) and customXml. This can't be taken back once the file is saved and sent. Only remove what the person asked for.",
            "input_schema": {"type": "object", "required": ["what"], "properties": {
                "what": {"type": "array", "items": {"type": "string", "enum": ["properties", "authors", "comments", "revisions", "hiddenText", "headersFooters", "customXml"]}}
            }}
        }),
    ]
}

/// Run one of the tools; None when `name` isn't one of them.
pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "count_words" => Ok(count_words(doc, args.get("include_extras").and_then(|v| v.as_bool()).unwrap_or(false))),
        "inspect_document" => Ok(inspect_document(doc)),
        "remove_hidden_data" => remove_hidden_data(doc, args),
        _ => return None,
    })
}

fn count_words(doc: &Document, extras: bool) -> String {
    let s = doc.text_stats();
    let total = if extras { s.main.plus(&s.notes).plus(&s.boxes) } else { s.main };
    let mut out = format!(
        "{} paragraphs, {} words, {} characters ({} without spaces){}.",
        total.paragraphs,
        total.words,
        total.chars,
        total.chars_no_spaces,
        if extras { ", with notes and text boxes" } else { "" }
    );
    if !extras && (s.notes.words > 0 || s.boxes.words > 0) {
        let _ = write!(out, " Not counted: {} words in footnotes and endnotes, {} in text boxes.", s.notes.words, s.boxes.words);
    }
    out
}

const LABELS: &[(&str, &str)] = &[
    ("properties", "Document properties and personal information"),
    ("authors", "Names behind comments and tracked changes"),
    ("comments", "Comments"),
    ("revisions", "Tracked changes"),
    ("hiddenText", "Hidden text"),
    ("headersFooters", "Headers, footers and watermarks"),
    ("customXml", "Custom XML data"),
    ("embedded", "Embedded objects and macros (can't be removed here)"),
];

fn inspect_document(doc: &Document) -> String {
    let mut out = String::new();
    for f in doc.inspect() {
        let label = LABELS.iter().find(|(id, _)| *id == f.id).map(|(_, l)| *l).unwrap_or(f.id);
        if f.count == 0 {
            let _ = writeln!(out, "{label} ({}): none found.", f.id);
            continue;
        }
        let _ = write!(out, "{label} ({}): {} found", f.id, f.count);
        if !f.detail.is_empty() {
            let _ = write!(out, ": {}", f.detail.join("; "));
        }
        out.push_str(".\n");
    }
    out.trim_end().to_string()
}

fn remove_hidden_data(doc: &mut Document, args: &Value) -> Result<String, String> {
    let what: Vec<String> = args
        .get("what")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if what.is_empty() {
        return Err("what is required: a list of categories from inspect_document".into());
    }
    let r = doc.apply_batch(&[Op::RemoveHiddenData { what: what.clone() }], false).map_err(|e| e.to_string())?;
    Ok(format!("Removed {} item(s) from: {}.", r.first().and_then(|r| r.count).unwrap_or(0), what.join(", ")))
}
