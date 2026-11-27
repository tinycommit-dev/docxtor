//! AI tools for mail merge: look at the merge, start one from a list of recipients, put merge fields in a letter
//! and read what a record would give. They live apart from `ai` and are merged into its tool list and executor.

use crate::document::Document;
use crate::merge::csv;
use crate::merge::instr::MergeFieldSpec;
use crate::merge::run::Answers;
use crate::merge::{MailMergeDef, SourceInfo};
use crate::ops::{Id, Op, Pos};
use serde_json::{json, Value};
use std::fmt::Write as _;

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "mail_merge_info",
            "description": "Show the document's mail merge: the kind of document (letters, labels, envelopes, directory), where the recipients came from, their columns and the first few records, how many are in the merge, and the merge fields in the text.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "start_mail_merge",
            "description": "Start a mail merge (Word's Mailings, Start Mail Merge and Select Recipients). `type` is letters (default), labels, envelopes or directory. `csv` is the recipients as comma, semicolon or tab separated text with a header row. Without `csv` only the kind of document changes. The recipients are kept in the file; Word's own settings are written too.",
            "input_schema": {"type": "object", "properties": {
                "type": {"type": "string", "enum": ["letters", "labels", "envelopes", "directory"]},
                "csv": {"type": "string", "description": "The recipients as text with a header row"},
                "name": {"type": "string", "description": "The file name to record for the list, such as people.csv"}
            }}
        }),
        json!({
            "name": "insert_merge_field",
            "description": "Insert a merge field or rule in a paragraph, at a UTF-16 offset (default the end). `field` is `{kind: \"field\", name: \"First Name\", before?, after?, case?: upper|lower|first_cap|caps, picture?}`, `{kind: \"address_block\", name?: first_last|title_first_last|title_last, company?, country?: never|always|different, except?}`, `{kind: \"greeting_line\", salutation?, name?: title_last|first|first_last|title_first_last, punctuation?, invalid?}`, `{kind: \"if\", field, op: eq|ne|lt|gt|le|ge|empty|not_empty, value, then, otherwise}`, `{kind: \"ask\", bookmark, prompt, default?, once?}`, `{kind: \"fill_in\", prompt, default?, once?}`, `{kind: \"next\"}`, `{kind: \"next_if\" or \"skip_if\", field, op, value}`, `{kind: \"merge_rec\"}`, `{kind: \"merge_seq\"}` or `{kind: \"set\", bookmark, value}`.",
            "input_schema": {"type": "object", "required": ["id", "field"], "properties": {
                "id": {"type": "string", "description": "A paragraph id"},
                "offset": {"type": "integer"},
                "field": {"type": "object"}
            }}
        }),
        json!({
            "name": "preview_merge_record",
            "description": "Show the text a record gives when the document is merged with it: `record` counts from 1 in the merge's order. The document is not changed.",
            "input_schema": {"type": "object", "properties": {"record": {"type": "integer"}}}
        }),
    ]
}

pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "mail_merge_info" => Ok(info(doc)),
        "start_mail_merge" => start(doc, args),
        "insert_merge_field" => insert(doc, args),
        "preview_merge_record" => preview(doc, args),
        _ => return None,
    })
}

fn main_type(word: &str) -> Result<&'static str, String> {
    Ok(match word {
        "" | "letters" | "formLetters" => "formLetters",
        "labels" | "mailingLabels" => "mailingLabels",
        "envelopes" => "envelopes",
        "directory" | "catalog" => "catalog",
        other => return Err(format!("unknown kind '{other}'; use letters, labels, envelopes or directory")),
    })
}

fn info(doc: &Document) -> String {
    let i = doc.merge_info();
    let mut out = String::new();
    if !i.active {
        out.push_str("The document is not a mail merge. Use start_mail_merge.\n");
    } else {
        let _ = writeln!(out, "Mail merge: {} (main document type {}).", if i.has_recipients { "with recipients" } else { "no recipients yet" }, i.main_type);
        if let Some(n) = &i.source_name {
            let _ = writeln!(out, "Source: {n}");
        }
    }
    if let Some(d) = doc.merge_data() {
        let _ = writeln!(out, "Columns: {}", d.columns.join(", "));
        let _ = writeln!(out, "{} records, {} in the merge.", i.records, i.included);
        for (k, &row) in d.order.iter().take(3).enumerate() {
            let _ = writeln!(out, "  {}. {}", k + 1, d.rows[row].join(" | "));
        }
    }
    for f in doc.list_fields().into_iter().filter(|f| crate::merge::instr::is_merge_instr(&f.instr)) {
        let _ = writeln!(out, "p{} @{}: {} => {:?}", f.p, f.o, f.instr.trim(), f.result);
    }
    out
}

fn start(doc: &mut Document, args: &Value) -> Result<String, String> {
    let kind = main_type(args.get("type").and_then(|v| v.as_str()).unwrap_or(""))?;
    let csv_text = args.get("csv").and_then(|v| v.as_str());
    let def = match csv_text {
        Some(text) => {
            let (table, delim) = csv::parse(text, None)?;
            let name = args.get("name").and_then(|v| v.as_str()).filter(|n| !n.is_empty()).unwrap_or("recipients.csv").to_string();
            MailMergeDef { main_type: kind.into(), source: Some(SourceInfo { kind: "csv".into(), name, table: None, delimiter: Some(delim) }), columns: table.columns, rows: table.rows, ..Default::default() }
        }
        None => MailMergeDef { main_type: kind.into(), ..Default::default() },
    };
    doc.apply_batch(&[Op::SetMailMerge { merge: Some(def) }], false).map_err(|e| e.to_string())?;
    let i = doc.merge_info();
    Ok(format!("Mail merge started ({}): {} recipients, columns {}.", i.main_type, i.records, i.columns.join(", ")))
}

fn insert(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id: Id = serde_json::from_value(args.get("id").cloned().ok_or("id is required")?).map_err(|e| e.to_string())?;
    let spec: MergeFieldSpec = serde_json::from_value(args.get("field").cloned().ok_or("field is required")?).map_err(|e| format!("field: {e}"))?;
    let offset = match args.get("offset").and_then(|v| v.as_u64()) {
        Some(o) => o as usize,
        None => crate::model::find_para(&doc.body, id.0).map(|p| p.len16()).unwrap_or(0),
    };
    doc.apply_batch(&[Op::InsertMergeField { at: Pos { p: id, o: offset }, field: spec }], false).map_err(|e| e.to_string())?;
    Ok(format!("Merge field inserted:\n{}", crate::ai::script_for_paragraphs(doc, &[id.0])))
}

fn preview(doc: &mut Document, args: &Value) -> Result<String, String> {
    let record = args.get("record").and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize - 1;
    let answers = Answers::default();
    let shown = doc.merge_preview(Some(record), &answers)?;
    let text = crate::export::to_text(doc);
    doc.merge_preview(None, &answers)?;
    let n = shown.map(|s| s.position + 1).unwrap_or(record + 1);
    Ok(format!("Record {n}:\n{text}"))
}
