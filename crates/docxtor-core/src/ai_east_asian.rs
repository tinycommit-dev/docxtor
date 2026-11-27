//! AI tools for East Asian typography: phonetic guides, the Asian typography switches of paragraphs, the East Asian
//! font and layout of text, the document grid and the line-breaking rules. They live apart from `ai` and are merged
//! into its tool list and executor.

use crate::ai::{find_in_para, id_of};
use crate::document::Document;
use crate::east_asian::{AsianSettings, DocGrid, EaLayoutPatch, EastAsianOp, LineBreakChars, ParaEaPatch, RunEaPatch};
use crate::layout_props::SectionPatch;
use crate::ops::{Id, Op, Pos};
use crate::props::{ParaPatch, RunPatch};
use crate::ruby::RubyProps;
use serde_json::{json, Value};

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "set_phonetic_guide",
            "description": "Put a phonetic guide (ruby: the reading of a kanji, in small text above it) over text in a paragraph, or change the guide that is already there. `text` is the base text to put it over (exact, first occurrence).",
            "input_schema": {"type": "object", "required": ["id", "text", "guide"], "properties": {
                "id": {"type": "string", "description": "Paragraph id"},
                "text": {"type": "string", "description": "The base text, or the base of an existing guide"},
                "guide": {"type": "string", "description": "The guide text, such as the hiragana reading"},
                "align": {"type": "string", "enum": ["center", "left", "right", "distributeSpace", "distributeLetter"]},
                "size_pt": {"type": "number", "description": "Size of the guide in points (default half the base text)"},
                "offset_pt": {"type": "number", "description": "How far above the baseline the guide sits, in points"}
            }}
        }),
        json!({
            "name": "remove_phonetic_guide",
            "description": "Take the phonetic guide over some base text away; the base text stays.",
            "input_schema": {"type": "object", "required": ["id", "text"], "properties": {
                "id": {"type": "string", "description": "Paragraph id"},
                "text": {"type": "string", "description": "The base text of the guide"}
            }}
        }),
        json!({
            "name": "set_east_asian_text",
            "description": "East Asian formatting of text in a paragraph: its East Asian font, two lines in one (with brackets), text upright in a vertical line (horizontal in vertical) and whether it snaps to the document grid. Matched by exact text, or the whole paragraph when `text` is omitted.",
            "input_schema": {"type": "object", "required": ["id"], "properties": {
                "id": {"type": "string"},
                "text": {"type": "string"},
                "all": {"type": "boolean"},
                "east_asian_font": {"type": "string", "description": "A font such as MS Mincho or Yu Gothic; empty removes it"},
                "two_lines_in_one": {"type": "boolean"},
                "brackets": {"type": "string", "enum": ["none", "round", "square", "angle", "curly"], "description": "Around two lines in one"},
                "horizontal_in_vertical": {"type": "boolean", "description": "Upright across a vertical line, fitted into the line"},
                "snap_to_grid": {"type": "boolean"}
            }}
        }),
        json!({
            "name": "set_asian_typography",
            "description": "The Asian typography switches of paragraphs: kinsoku (the East Asian rules for the first and last character of a line), word_wrap (false lets a Latin word break across lines), overflow_punct (punctuation may hang past the margin), top_line_punct (compress punctuation at the start of a line), auto_space_de (space between Asian and Latin text), auto_space_dn (between Asian text and numbers), snap_to_grid and a text_direction for the paragraph (tbRl, btLr, lrTb).",
            "input_schema": {"type": "object", "required": ["ids"], "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "kinsoku": {"type": "boolean"}, "word_wrap": {"type": "boolean"}, "overflow_punct": {"type": "boolean"}, "top_line_punct": {"type": "boolean"},
                "auto_space_de": {"type": "boolean"}, "auto_space_dn": {"type": "boolean"}, "snap_to_grid": {"type": "boolean"},
                "text_direction": {"type": "string", "enum": ["lrTb", "tbRl", "btLr"]}
            }}
        }),
        json!({
            "name": "set_document_grid",
            "description": "The document grid of a section: lines of a fixed pitch, and with characters a fixed width per character. `type` default removes the grid. The grid applies to the section holding paragraph `section`, else the whole document.",
            "input_schema": {"type": "object", "required": ["type"], "properties": {
                "type": {"type": "string", "enum": ["default", "lines", "linesAndChars", "snapToChars"]},
                "line_pitch_pt": {"type": "number", "description": "Distance between lines in points"},
                "char_space_pt": {"type": "number", "description": "Added to the font size to get a character's width, in points (negative packs them closer)"},
                "section": {"type": "string", "description": "A paragraph id in the section"}
            }}
        }),
        json!({
            "name": "set_line_breaking_rules",
            "description": "The document's East Asian line-breaking rules: `strict` for the strict level, and for a language (ja-JP, ko-KR, zh-CN, zh-TW) the characters that may not start a line (cannot_start) or end one (cannot_end). An empty string empties a list. Without arguments the current rules are reported.",
            "input_schema": {"type": "object", "properties": {
                "strict": {"type": "boolean"},
                "language": {"type": "string", "enum": ["ja-JP", "ko-KR", "zh-CN", "zh-TW"]},
                "cannot_start": {"type": "string"},
                "cannot_end": {"type": "string"}
            }}
        }),
    ]
}

pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "set_phonetic_guide" => set_guide(doc, args),
        "remove_phonetic_guide" => remove_guide(doc, args),
        "set_east_asian_text" => east_asian_text(doc, args),
        "set_asian_typography" => asian_typography(doc, args),
        "set_document_grid" => document_grid(doc, args),
        "set_line_breaking_rules" => line_breaking(doc, args),
        _ => return None,
    })
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn bool_arg(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn run(doc: &mut Document, ops: Vec<Op>) -> Result<String, String> {
    doc.apply_batch(&ops, false).map_err(|e| e.to_string())?;
    Ok("Done.".into())
}

fn set_guide(doc: &mut Document, args: &Value) -> Result<String, String> {
    let pid = id_of(args.get("id").ok_or("id is required")?)?;
    let text = str_arg(args, "text").ok_or("text is required")?;
    let guide = str_arg(args, "guide").ok_or("guide is required")?;
    let props = RubyProps {
        text: guide.to_string(),
        align: str_arg(args, "align").map(String::from),
        size: args.get("size_pt").and_then(|v| v.as_f64()),
        raise: args.get("offset_pt").and_then(|v| v.as_f64()),
        ..Default::default()
    };
    // A guide that is already over this text is changed; otherwise the text gets a new one.
    if let Some((o, _)) = doc.guides_in(pid).into_iter().find(|(_, g)| g.base == text) {
        return run(doc, vec![Op::EastAsian(EastAsianOp::SetRuby { from: Pos::new(pid, o), to: Pos::new(pid, o + 1), props })]);
    }
    let (from, to) = find_in_para(doc, pid, text, false)?[0];
    run(doc, vec![Op::EastAsian(EastAsianOp::SetRuby { from: Pos::new(pid, from), to: Pos::new(pid, to), props })])
}

fn remove_guide(doc: &mut Document, args: &Value) -> Result<String, String> {
    let pid = id_of(args.get("id").ok_or("id is required")?)?;
    let text = str_arg(args, "text").ok_or("text is required")?;
    let (o, _) = doc.guides_in(pid).into_iter().find(|(_, g)| g.base == text).ok_or_else(|| format!("no phonetic guide over {text:?} in p{pid}"))?;
    run(doc, vec![Op::EastAsian(EastAsianOp::ClearRuby { at: Pos::new(pid, o) })])
}

fn east_asian_text(doc: &mut Document, args: &Value) -> Result<String, String> {
    let pid = id_of(args.get("id").ok_or("id is required")?)?;
    let mut ea = RunEaPatch::default();
    if let Some(f) = str_arg(args, "east_asian_font") {
        ea.font_ea = Some(f.to_string());
    }
    let mut layout = EaLayoutPatch { combine: bool_arg(args, "two_lines_in_one"), combine_brackets: str_arg(args, "brackets").map(String::from), ..Default::default() };
    if let Some(v) = bool_arg(args, "horizontal_in_vertical") {
        layout.vert = Some(v);
        layout.vert_compress = Some(v);
    }
    if layout != EaLayoutPatch::default() {
        ea.ea_layout = Some(layout);
    }
    ea.snap_to_grid = bool_arg(args, "snap_to_grid");
    if ea == RunEaPatch::default() {
        return Err("say what to set: east_asian_font, two_lines_in_one, brackets, horizontal_in_vertical or snap_to_grid".into());
    }
    let ranges = match str_arg(args, "text").filter(|t| !t.is_empty()) {
        Some(t) => find_in_para(doc, pid, t, bool_arg(args, "all").unwrap_or(false))?,
        None => vec![(0, doc.find_any_para(pid).ok_or(format!("paragraph p{pid} not found"))?.len16())],
    };
    let patch = RunPatch { ext: crate::charfmt::RunExtPatch { ea, ..Default::default() }, ..Default::default() };
    let ops = ranges.iter().map(|(a, b)| Op::FormatText { from: Pos::new(pid, *a), to: Pos::new(pid, *b), format: patch.clone() }).collect();
    run(doc, ops)
}

fn asian_typography(doc: &mut Document, args: &Value) -> Result<String, String> {
    let ids: Vec<Id> = match args.get("ids") {
        Some(Value::Array(a)) => a.iter().map(|v| id_of(v).map(Id)).collect::<Result<_, _>>()?,
        Some(other) => vec![Id(id_of(other)?)],
        None => return Err("ids is required".into()),
    };
    let ea = ParaEaPatch {
        kinsoku: bool_arg(args, "kinsoku"),
        word_wrap: bool_arg(args, "word_wrap"),
        overflow_punct: bool_arg(args, "overflow_punct"),
        top_line_punct: bool_arg(args, "top_line_punct"),
        auto_space_de: bool_arg(args, "auto_space_de"),
        auto_space_dn: bool_arg(args, "auto_space_dn"),
        snap_to_grid: bool_arg(args, "snap_to_grid"),
        text_direction: str_arg(args, "text_direction").map(String::from),
        text_alignment: None,
    };
    if ea.is_empty() {
        return Err("say what to set: kinsoku, word_wrap, overflow_punct, top_line_punct, auto_space_de, auto_space_dn, snap_to_grid or text_direction".into());
    }
    ea.validate()?;
    run(doc, vec![Op::FormatParagraphs { ids, format: ParaPatch { ea, ..Default::default() } }])
}

fn document_grid(doc: &mut Document, args: &Value) -> Result<String, String> {
    let kind = str_arg(args, "type").ok_or("type is required")?.to_string();
    let grid = DocGrid {
        kind,
        line_pitch: args.get("line_pitch_pt").and_then(|v| v.as_f64()).map(|p| (p * 20.0).round() as i64),
        char_space: args.get("char_space_pt").and_then(|v| v.as_f64()).map(|p| (p * 4096.0).round() as i64),
    };
    let section = match args.get("section") {
        Some(v) if !v.is_null() => Some(Id(id_of(v)?)),
        _ => None,
    };
    let patch = SectionPatch { doc_grid: Some(grid), ..Default::default() };
    run(doc, vec![Op::SetSection { section, from: false, patch }])
}

fn line_breaking(doc: &mut Document, args: &Value) -> Result<String, String> {
    let mut s: AsianSettings = doc.asian_settings();
    let touched = args.get("strict").is_some() || args.get("language").is_some();
    if !touched {
        return Ok(serde_json::to_string_pretty(&s).unwrap_or_default());
    }
    if let Some(b) = bool_arg(args, "strict") {
        s.strict = b;
    }
    if let Some(lang) = str_arg(args, "language") {
        let entry = match s.custom.iter().position(|c| c.lang == lang) {
            Some(i) => i,
            None => {
                s.custom.push(LineBreakChars { lang: lang.to_string(), ..Default::default() });
                s.custom.len() - 1
            }
        };
        if let Some(v) = str_arg(args, "cannot_start") {
            s.custom[entry].before = v.to_string();
        }
        if let Some(v) = str_arg(args, "cannot_end") {
            s.custom[entry].after = v.to_string();
        }
        s.custom.retain(|c| !c.before.is_empty() || !c.after.is_empty());
    }
    run(doc, vec![Op::EastAsian(EastAsianOp::SetSettings { settings: s })])
}
