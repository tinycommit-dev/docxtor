//! AI tools for the outline: change heading levels and move a section. They live apart from `ai` and are merged into
//! its tool list and executor.

use crate::document::Document;
use crate::ops::{Id, Op};
use serde_json::{json, Value};

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "set_heading_level",
            "description": "Promote or demote headings, or turn paragraphs into headings or body text. Give `level` (1 to 9 for Heading 1 to 9, 0 for body text) or `delta` (-1 promotes one level, 1 demotes; body text promotes to a heading one level below the heading above it, a level 9 demotes to body text). With `with_subsections` the headings under each given heading move by the same amount, so the structure stays as it is.",
            "input_schema": {"type": "object", "required": ["ids"], "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "level": {"type": "integer", "minimum": 0, "maximum": 9},
                "delta": {"type": "integer", "minimum": -8, "maximum": 8},
                "with_subsections": {"type": "boolean", "description": "Also change the headings of the sections under each given heading"}
            }}
        }),
        json!({
            "name": "move_section",
            "description": "Move a heading together with everything under it (down to the next heading of the same or a higher level) to just after a block (`after`) or just before one (`before`). Ids come from get_outline or read_document.",
            "input_schema": {"type": "object", "required": ["id"], "properties": {
                "id": {"type": "string", "description": "The heading"},
                "after": {"type": "string"},
                "before": {"type": "string"}
            }}
        }),
    ]
}

pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "set_heading_level" => set_heading_level(doc, args),
        "move_section" => move_section(doc, args),
        _ => return None,
    })
}

fn id_list(v: Option<&Value>) -> Result<Vec<u32>, String> {
    match v {
        Some(Value::Array(a)) => a.iter().map(crate::ai::id_of).collect(),
        Some(Value::Null) | None => Ok(vec![]),
        Some(other) => Ok(vec![crate::ai::id_of(other)?]),
    }
}

fn set_heading_level(doc: &mut Document, args: &Value) -> Result<String, String> {
    let ids = id_list(args.get("ids"))?;
    if ids.is_empty() {
        return Err("ids is required".into());
    }
    let level = args.get("level").and_then(|v| v.as_u64()).map(|l| l.min(255) as u8);
    let delta = args.get("delta").and_then(|v| v.as_i64()).map(|d| d.clamp(-9, 9) as i8);
    if level.is_none() && delta.is_none() {
        return Err("give level or delta".into());
    }
    let mut all: Vec<u32> = Vec::new();
    for id in &ids {
        if !all.contains(id) {
            all.push(*id);
        }
        if args.get("with_subsections").and_then(|v| v.as_bool()).unwrap_or(false) {
            let outline = doc.outline();
            for sub in doc.section_ids(*id).into_iter().skip(1) {
                if outline.iter().any(|e| e.id == sub && e.level > 0) && !all.contains(&sub) {
                    all.push(sub);
                }
            }
        }
    }
    let n = all.len();
    doc.apply_batch(&[Op::SetOutlineLevel { ids: all.into_iter().map(Id).collect(), level, delta }], false).map_err(|e| e.to_string())?;
    Ok(format!("Changed the level of {n} paragraph(s).\n{}", crate::ai::outline(doc)))
}

fn move_section(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = crate::ai::id_of(args.get("id").ok_or("id is required")?)?;
    let after = args.get("after").filter(|v| !v.is_null()).map(crate::ai::id_of).transpose()?.map(Id);
    let before = args.get("before").filter(|v| !v.is_null()).map(crate::ai::id_of).transpose()?.map(Id);
    let ids: Vec<Id> = doc.section_ids(id).into_iter().map(Id).collect();
    if ids.is_empty() {
        return Err(format!("block {id} is not in the main text"));
    }
    let n = ids.len();
    doc.apply_batch(&[Op::MoveBlocks { ids, after, before }], false).map_err(|e| e.to_string())?;
    Ok(format!("Moved {n} block(s).\n{}", crate::ai::outline(doc)))
}
