//! AI tools for content controls, form fields and document protection. They live apart from `ai` and are merged
//! into its tool list and executor. The assistant can fill a form in and look at what a document restricts; it
//! cannot set or remove a password.

use crate::ai::{find_in_para, id_of};
use crate::controls::{ControlOp, ControlProps, ControlRef};
use crate::document::Document;
use crate::forms::FormOp;
use crate::ops::{Id, Op, Pos};
use crate::protection::ProtectionOp;
use serde_json::{json, Value};

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_content_controls",
            "description": "List the content controls (check boxes, drop-down lists, date pickers, text and picture controls, repeating sections) and legacy form fields (text, check box, drop-down) in the document, with the paragraph id and number to address each by, its kind, name, current value and, for a list, the entries you can choose.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "fill_content_control",
            "description": "Fill in a content control or form field found with list_content_controls. Give the paragraph `id` and the control's `n` (and `form: true` for a legacy form field), then one of: `text` (text controls, combo boxes, text form fields; an empty string clears it), `checked` (check boxes), `choice` (the entry number of a list, from 0), `date` (2026-10-14, date pickers). Locked controls and protected documents refuse the edit.",
            "input_schema": {"type": "object", "required": ["id"], "properties": {
                "id": {"type": "string", "description": "Paragraph id"},
                "n": {"type": "integer", "description": "The control's number in the paragraph (default 0)"},
                "form": {"type": "boolean", "description": "A legacy form field rather than a content control"},
                "block": {"type": "string", "description": "Id of a block-level control, instead of id and n"},
                "text": {"type": "string"},
                "checked": {"type": "boolean"},
                "choice": {"type": "integer"},
                "date": {"type": "string"}
            }}
        }),
        json!({
            "name": "insert_content_control",
            "description": "Insert a content control after some text in a paragraph (`after_text`, else at the end): kind text, richText, picture, date, dropDown, comboBox, checkbox or group. Give `alias` (its title), `tag`, `placeholder` text and `items` (for lists).",
            "input_schema": {"type": "object", "required": ["id", "kind"], "properties": {
                "id": {"type": "string", "description": "Paragraph id"},
                "after_text": {"type": "string"},
                "kind": {"type": "string", "enum": ["text", "richText", "picture", "date", "dropDown", "comboBox", "checkbox", "group"]},
                "alias": {"type": "string"},
                "tag": {"type": "string"},
                "placeholder": {"type": "string"},
                "items": {"type": "array", "items": {"type": "string"}, "description": "The entries of a drop-down list or combo box"},
                "date_format": {"type": "string", "description": "A date picker's format, such as M/d/yyyy"}
            }}
        }),
        json!({
            "name": "get_document_protection",
            "description": "Say what the document restricts: the kind of protection (readOnly, forms, comments, trackedChanges), whether it is on, whether formatting is limited, whether it is marked as final and where the editable ranges are. Edits outside the editable ranges are refused while protection is on.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "set_document_protection",
            "description": "Restrict editing without a password (the person can set one in Review, Restrict editing): `edit` is none, readOnly, comments, trackedChanges or forms; `formatting` limits formatting to styles; `enforce` switches it on. Use `final: true` to mark the document as final. Do this only when asked.",
            "input_schema": {"type": "object", "properties": {
                "edit": {"type": "string", "enum": ["none", "readOnly", "comments", "trackedChanges", "forms"]},
                "formatting": {"type": "boolean"},
                "enforce": {"type": "boolean"},
                "final": {"type": "boolean"}
            }}
        }),
    ]
}

pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "list_content_controls" => Ok(list(doc)),
        "fill_content_control" => fill(doc, args),
        "insert_content_control" => insert(doc, args),
        "get_document_protection" => Ok(serde_json::to_string_pretty(&doc.protection_info()).unwrap_or_default()),
        "set_document_protection" => set_protection(doc, args),
        _ => return None,
    })
}

fn list(doc: &mut Document) -> String {
    let mut out = String::new();
    let mut n = 0;
    for (p, c, text) in doc.list_controls() {
        n += 1;
        let at = match (p, c.ctl) {
            (Some(p), _) => format!("p{p} n={}", c.n),
            (None, Some(b)) => format!("block {b}"),
            _ => String::new(),
        };
        let name = c.alias.clone().or(c.tag.clone()).map(|a| format!(" \"{a}\"")).unwrap_or_default();
        let value = if c.placeholder { "(empty)".to_string() } else { format!("\"{text}\"") };
        let state = match c.kind.as_str() {
            "checkbox" => format!(" {}", if c.checked == Some(true) { "checked" } else { "unchecked" }),
            _ => String::new(),
        };
        let items = if c.items.is_empty() { String::new() } else { format!(" choices: {}", c.items.iter().enumerate().map(|(i, it)| format!("{i}={}", it.display)).collect::<Vec<_>>().join(", ")) };
        let lock = c.lock.as_deref().map(|l| format!(" locked: {l}")).unwrap_or_default();
        let _ = std::fmt::Write::write_fmt(&mut out, format_args!("[{at}] {}{name}{state} = {value}{items}{lock}\n", c.kind));
    }
    fn forms(blocks: &[crate::render::RBlock], out: &mut String, n: &mut usize) {
        for b in blocks {
            match b {
                crate::render::RBlock::Para(p) => {
                    for f in &p.forms {
                        *n += 1;
                        let value = match f.kind.as_str() {
                            "checkbox" => if f.checked == Some(true) { "checked".to_string() } else { "unchecked".to_string() },
                            "dropdown" => f.items.get(f.selected.unwrap_or(0)).cloned().unwrap_or_default(),
                            _ => String::new(),
                        };
                        let items = if f.items.is_empty() { String::new() } else { format!(" choices: {}", f.items.iter().enumerate().map(|(i, it)| format!("{i}={it}")).collect::<Vec<_>>().join(", ")) };
                        let _ = std::fmt::Write::write_fmt(out, format_args!("[p{} n={} form] {}{} {value}{items}\n", p.id, f.n, f.kind, f.name.as_deref().map(|x| format!(" \"{x}\"")).unwrap_or_default()));
                    }
                }
                crate::render::RBlock::Container { blocks, .. } => forms(blocks, out, n),
                crate::render::RBlock::Table(t) => {
                    for r in &t.rows {
                        for c in &r.cells {
                            forms(&c.blocks, out, n);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    doc.render_snapshot_by_block(&mut |_, drawn| forms(drawn, &mut out, &mut n));
    if n == 0 {
        "The document has no content controls or form fields.".into()
    } else {
        out
    }
}


fn fill(doc: &mut Document, args: &Value) -> Result<String, String> {
    let text = args.get("text").and_then(|v| v.as_str()).map(String::from);
    let checked = args.get("checked").and_then(|v| v.as_bool());
    let choice = args.get("choice").and_then(|v| v.as_u64()).map(|v| v as usize);
    let date = args.get("date").and_then(|v| v.as_str()).map(String::from);
    let n = args.get("n").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let op = if args.get("form").and_then(|v| v.as_bool()) == Some(true) {
        let id = id_of(args.get("id").ok_or("id is required")?)?;
        Op::FormField(FormOp::Set { at: Pos::new(id, 0), n, checked, index: choice, text })
    } else {
        let target = match args.get("block") {
            Some(b) if !b.is_null() => ControlRef { block: Some(Id(id_of(b)?)), ..Default::default() },
            _ => ControlRef { p: Some(Id(id_of(args.get("id").ok_or("id is required")?)?)), n, ..Default::default() },
        };
        Op::Control(ControlOp::Set { target, text, checked, index: choice, date, picture: None })
    };
    doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    Ok("Filled in.".into())
}

fn insert(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let kind = args.get("kind").and_then(|v| v.as_str()).ok_or("kind is required")?;
    let at = match args.get("after_text").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => find_in_para(doc, id, t, false)?[0].1,
        _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
    };
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).map(String::from);
    let items = args.get("items").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).map(|t| crate::controls::ListItem { display: t.into(), value: t.into() }).collect());
    let props = ControlProps { alias: s("alias"), tag: s("tag"), placeholder: s("placeholder"), items, date_format: s("date_format"), ..Default::default() };
    doc.apply_batch(&[Op::Control(ControlOp::Insert { at: Pos::new(id, at), to: None, kind: kind.into(), props, block: false })], false).map_err(|e| e.to_string())?;
    Ok(format!("Inserted a {kind} control in p{id}."))
}

fn set_protection(doc: &mut Document, args: &Value) -> Result<String, String> {
    let mut ops = Vec::new();
    let edit = args.get("edit").and_then(|v| v.as_str()).map(String::from);
    let formatting = args.get("formatting").and_then(|v| v.as_bool());
    let enforce = args.get("enforce").and_then(|v| v.as_bool());
    if edit.is_some() || formatting.is_some() || enforce.is_some() {
        ops.push(Op::Protection(ProtectionOp::Set { settings: crate::protection::ProtectionSettings { edit, formatting, enforce, ..Default::default() } }));
    }
    if let Some(f) = args.get("final").and_then(|v| v.as_bool()) {
        ops.push(Op::Protection(ProtectionOp::MarkFinal { on: f }));
    }
    if ops.is_empty() {
        return Err("say what to set: edit, formatting, enforce or final".into());
    }
    doc.apply_batch(&ops, false).map_err(|e| e.to_string())?;
    Ok("Done.".into())
}
