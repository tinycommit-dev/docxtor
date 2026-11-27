//! AI tools for shapes and text boxes, merged into `ai`'s tool list and executor.

use crate::ai::{find_in_para, id_of, script_for_paragraphs};
use crate::document::Document;
use crate::ops::{Op, Pos};
use serde_json::{json, Value};

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_objects",
            "description": "List every drawing object of the document (shapes, text boxes, pictures, charts, groups) with its position `{p, o}` (the paragraph and the offset of the object), kind, name, wrapping, stacking value `z`, whether it is hidden, and where a floating one is placed. Use the positions in apply_ops: `set_object_props` (`at`, `name?`, `hidden?`) names or hides an object, `set_object_position` (`at`, `rel_x?`, `align?`, `x?`, `rel_y?`, `align_y?`, `y?`) places a floating object as Word does (for example `rel_x: \"margin\", align: \"center\"` centres it between the margins), and `move_object` (`at`, `above?` or `below?`: another object's position) puts it directly above or below another object of the same stack.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "insert_text_box",
            "description": "Insert a text box (a rectangle with a thin black outline and white fill that floats beside the paragraph's text, as in Word) right after some text in a paragraph. `text` is its content, one paragraph per line; the result lists the new paragraph ids inside the box, which you can edit with the ordinary text tools (insert_text, set_paragraph_text, replace_text, ...).",
            "input_schema": {"type": "object", "required": ["id", "text"], "properties": {
                "id": {"type": "string", "description": "Paragraph to anchor the text box to"},
                "after_text": {"type": "string", "description": "Exact text the box is anchored after (first occurrence); default is the end of the paragraph"},
                "text": {"type": "string", "description": "The box's text; use \\n between paragraphs"},
                "width": {"type": "number", "description": "Points (default 200)"},
                "height": {"type": "number", "description": "Points (default fits the text)"},
                "floating": {"type": "boolean", "description": "Float beside the text with square wrapping (default true); false puts the box in the line of text"},
                "x": {"type": "number", "description": "Points from the column to the box (floating only, default 0)"},
                "y": {"type": "number", "description": "Points from the paragraph to the box (floating only, default 0)"}
            }}
        }),
        json!({
            "name": "insert_shape",
            "description": "Insert a drawing shape (rectangle, rounded rectangle, ellipse, triangle, arrow, star, line, ...) right after some text in a paragraph. `shape` is a Word preset such as rect, roundRect, ellipse, triangle, rtTriangle, diamond, pentagon, hexagon, star5, rightArrow, leftArrow, upArrow, downArrow, chevron, cloud, heart or line (plain names like rectangle, oval and arrow work too). The shape floats beside the text unless floating is false. Optional `text` goes inside it (and can be edited later with the ordinary text tools using the paragraph ids in the result).",
            "input_schema": {"type": "object", "required": ["id", "shape"], "properties": {
                "id": {"type": "string", "description": "Paragraph to anchor the shape to"},
                "after_text": {"type": "string", "description": "Exact text the shape is anchored after (first occurrence); default is the end of the paragraph"},
                "shape": {"type": "string", "description": "Preset name"},
                "text": {"type": "string", "description": "Text inside the shape"},
                "fill": {"type": "string", "description": "Hex colour such as FFF2CC, or none"},
                "line": {"type": "string", "description": "Outline colour as hex, or none"},
                "line_width": {"type": "number", "description": "Outline width in points"},
                "width": {"type": "number", "description": "Points (default 100)"},
                "height": {"type": "number", "description": "Points (default 60; 0 for a horizontal line)"},
                "floating": {"type": "boolean", "description": "Float beside the text (default true)"},
                "x": {"type": "number"}, "y": {"type": "number"}
            }}
        }),
        json!({
            "name": "insert_freeform",
            "description": "Draw a curve or a freeform (a shape with its own outline, as Word's Curve, Freeform and Scribble tools make) right after some text in a paragraph. `points` are [x, y] pairs in points; with `smooth` the line is a curve through them, otherwise straight segments; `closed` joins the last point to the first (a closed shape gets a fill, an open one does not). The first point sits `x`, `y` points from the column and the paragraph, so a shape can be placed exactly. Connectors that join shapes are `insert_connector` and `set_connector` in apply_ops.",
            "input_schema": {"type": "object", "required": ["id", "points"], "properties": {
                "id": {"type": "string", "description": "Paragraph to anchor the drawing to"},
                "after_text": {"type": "string", "description": "Exact text the drawing is anchored after (first occurrence); default is the end of the paragraph"},
                "points": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2}, "description": "[x, y] points, at least two"},
                "smooth": {"type": "boolean", "description": "A smooth curve through the points (default false: straight segments)"},
                "closed": {"type": "boolean", "description": "Join the last point to the first (default false)"},
                "fill": {"type": "string", "description": "Hex colour such as FFF2CC, or none (default: a light blue for a closed shape, none for an open one)"},
                "line": {"type": "string", "description": "Outline colour as hex, or none"},
                "line_width": {"type": "number", "description": "Outline width in points"},
                "floating": {"type": "boolean", "description": "Float beside the text (default true)"},
                "x": {"type": "number"}, "y": {"type": "number"}
            }}
        }),
    ]
}

/// Run one of these tools; None when the name isn't one of them.
pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "list_objects" => Ok(list_objects(doc)),
        "insert_text_box" => insert_text_box(doc, args),
        "insert_shape" => insert_shape(doc, args),
        "insert_freeform" => insert_freeform(doc, args),
        _ => return None,
    })
}

fn list_objects(doc: &Document) -> String {
    let list = doc.objects();
    if list.is_empty() {
        return "No shapes, text boxes, pictures or charts in the document.".into();
    }
    serde_json::to_string(&list).unwrap_or_default()
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}

fn n(args: &Value, k: &str) -> Option<f64> {
    args.get(k).and_then(|v| v.as_f64())
}

fn flag(args: &Value, k: &str, default: bool) -> bool {
    args.get(k).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// Where a shape is anchored: after `after_text`, else at the end of the paragraph.
fn anchor(doc: &Document, args: &Value) -> Result<Pos, String> {
    let id = id_of(args.get("id").ok_or("id is required")?)?;
    let o = match s(args, "after_text") {
        Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
        _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
    };
    Ok(Pos::new(id, o))
}

fn insert_text_box(doc: &mut Document, args: &Value) -> Result<String, String> {
    let at = anchor(doc, args)?;
    let text = s(args, "text").ok_or("text is required")?;
    let lines = text.split('\n').count() as f64;
    let op = Op::InsertTextBox {
        at,
        w: n(args, "width").unwrap_or(200.0),
        h: n(args, "height").unwrap_or((lines * 18.0 + 14.0).max(40.0)),
        text,
        floating: flag(args, "floating", true),
        x: n(args, "x"),
        y: n(args, "y"),
    };
    let r = doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    let created = r[0].created.clone();
    let mut ids = vec![at.p.0];
    ids.extend(&created);
    Ok(format!("Text box inserted; its paragraphs are {}:\n{}", created.iter().map(|c| format!("p{c}")).collect::<Vec<_>>().join(", "), script_for_paragraphs(doc, &ids)))
}

fn insert_freeform(doc: &mut Document, args: &Value) -> Result<String, String> {
    let at = anchor(doc, args)?;
    let points: Vec<[f64; 2]> = args
        .get("points")
        .and_then(|v| v.as_array())
        .ok_or("points is required")?
        .iter()
        .map(|p| {
            let a = p.as_array().filter(|a| a.len() == 2).ok_or("each point is [x, y]")?;
            Ok([a[0].as_f64().ok_or("a point needs numbers")?, a[1].as_f64().ok_or("a point needs numbers")?])
        })
        .collect::<Result<_, &str>>()?;
    let line = match s(args, "line") {
        Some(l) if l.eq_ignore_ascii_case("none") => Some(super::LineSpec::None),
        Some(color) => Some(super::LineSpec::Set(super::LineProps { color: Some(color), width_pt: n(args, "line_width"), ..Default::default() })),
        None => n(args, "line_width").map(|w| super::LineSpec::Set(super::LineProps { width_pt: Some(w), ..Default::default() })),
    };
    let op = Op::InsertFreeform {
        at,
        points,
        cmds: vec![],
        smooth: flag(args, "smooth", false),
        closed: flag(args, "closed", false),
        fill: s(args, "fill"),
        line,
        floating: flag(args, "floating", true),
        x: n(args, "x"),
        y: n(args, "y"),
        label: None,
    };
    doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    Ok(format!("Freeform inserted:\n{}", script_for_paragraphs(doc, &[at.p.0])))
}

fn insert_shape(doc: &mut Document, args: &Value) -> Result<String, String> {
    let at = anchor(doc, args)?;
    let shape = s(args, "shape").ok_or("shape is required")?;
    let preset = super::build::preset(&shape)?;
    let line = match s(args, "line") {
        Some(l) if l.eq_ignore_ascii_case("none") => Some(super::LineSpec::None),
        Some(color) => Some(super::LineSpec::Set(super::LineProps { color: Some(color), width_pt: n(args, "line_width"), ..Default::default() })),
        None => n(args, "line_width").map(|w| super::LineSpec::Set(super::LineProps { width_pt: Some(w), ..Default::default() })),
    };
    let default_h = if super::build::is_line(preset) { 0.0 } else { 60.0 };
    let op = Op::InsertShape {
        at,
        geometry: preset.to_string(),
        w: n(args, "width").unwrap_or(100.0),
        h: n(args, "height").unwrap_or(default_h),
        fill: s(args, "fill"),
        line,
        text: s(args, "text"),
        floating: flag(args, "floating", true),
        x: n(args, "x"),
        y: n(args, "y"),
    };
    let r = doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    let created = r[0].created.clone();
    let mut ids = vec![at.p.0];
    ids.extend(&created);
    let note = if created.is_empty() { String::new() } else { format!("; the text inside is {}", created.iter().map(|c| format!("p{c}")).collect::<Vec<_>>().join(", ")) };
    Ok(format!("Shape inserted{note}:\n{}", script_for_paragraphs(doc, &ids)))
}
