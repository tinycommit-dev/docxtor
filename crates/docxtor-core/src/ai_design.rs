//! AI tools for styles, themes, style sets and paragraph spacing. They live apart from `ai` and are merged into its
//! tool list and executor.

use crate::document::Document;
use crate::ops::Op;
use crate::props::{ParaPatch, RunPatch};
use crate::style_edit::StyleDefinition;
use crate::style_sets;
use crate::themes::{self, ColorScheme, FontScheme};
use serde_json::{json, Value};
use std::fmt::Write as _;

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "define_style",
            "description": "Change what a style looks like (every paragraph or run that uses it follows), or make a new style. `style` is a style name or id from list_styles; Word's built-in styles (Title, Heading 1 to 9, Quote, Emphasis, Strong...) can always be used. If no such style exists a new one is made (give `kind`: paragraph, character or linked). Only what you give is changed. Sizes and spacing are in points; `line_spacing` is a multiple (1, 1.15, 1.5, 2). To restyle all headings, call this once per heading style instead of formatting each paragraph.",
            "input_schema": {"type": "object", "required": ["style"], "properties": {
                "style": {"type": "string"},
                "name": {"type": "string", "description": "A new name (styles you made only)"},
                "kind": {"type": "string", "enum": ["paragraph", "character", "linked"], "description": "For a new style"},
                "based_on": {"type": "string", "description": "The style it builds on; \"\" for none"},
                "next": {"type": "string", "description": "The style of the paragraph after it; \"\" for the same style"},
                "in_gallery": {"type": "boolean", "description": "Show in the style gallery"},
                "auto_update": {"type": "boolean"},
                "hidden": {"type": "boolean"},
                "font": {"type": "string"},
                "size_pt": {"type": "number"},
                "bold": {"type": "boolean"},
                "italic": {"type": "boolean"},
                "underline": {"type": "boolean"},
                "color": {"type": "string", "description": "Hex, e.g. 1F4E79"},
                "align": {"type": "string", "enum": ["left", "center", "right", "both"]},
                "space_before_pt": {"type": "number"},
                "space_after_pt": {"type": "number"},
                "line_spacing": {"type": "number"},
                "indent_left_pt": {"type": "number"},
                "indent_right_pt": {"type": "number"},
                "first_line_indent_pt": {"type": "number", "description": "Negative for a hanging indent"},
                "keep_with_next": {"type": "boolean"},
                "page_break_before": {"type": "boolean"}
            }}
        }),
        json!({
            "name": "create_style_from_selection",
            "description": "Make a style from how some text is formatted now (Word's Create a Style), and give that text the style. The style takes the paragraph's own formatting and what all the text in the range shares.",
            "input_schema": {"type": "object", "required": ["name", "id"], "properties": {
                "name": {"type": "string"},
                "id": {"type": "string", "description": "A paragraph id"},
                "from": {"type": "integer", "description": "Start offset in the paragraph (default 0)"},
                "to": {"type": "integer", "description": "End offset (default the end)"},
                "kind": {"type": "string", "enum": ["paragraph", "character", "linked"]}
            }}
        }),
        json!({
            "name": "delete_style",
            "description": "Delete a style you or the document made (Word's built-in styles stay). Paragraphs that used it fall back to the style it was based on.",
            "input_schema": {"type": "object", "required": ["style"], "properties": {"style": {"type": "string"}}}
        }),
        json!({
            "name": "list_design",
            "description": "Show the document's theme (colours and fonts) and what can be chosen: Word's themes, colour schemes, font schemes, style sets and paragraph spacing presets.",
            "input_schema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "set_theme",
            "description": "Change the document's theme (Word's Design tab): a whole theme (`theme`: Office, Facet, Integral, Ion, Ion Boardroom, Organic, Retrospect, Slice, Wisp, Basis, Berlin, Circuit, Damask, Depth, Droplet, Frame, Gallery, Main Event, Mesh, Parallax, Savon, View, Vapor Trail, Wood Type...), only its colours (`colors_scheme`, e.g. Blue Warm, or `colors` for single slots: dk1, lt1, dk2, lt2, accent1 to accent6, hlink, folHlink) or only its fonts (`fonts_scheme`, e.g. Garamond, or `heading_font` and `body_font`). Text, headings, tables and shapes that use theme colours or fonts follow. See list_design.",
            "input_schema": {"type": "object", "properties": {
                "theme": {"type": "string"},
                "colors_scheme": {"type": "string"},
                "colors": {"type": "object", "description": "Slot to hex colour, e.g. {\"accent1\": \"C00000\"}"},
                "fonts_scheme": {"type": "string"},
                "heading_font": {"type": "string"},
                "body_font": {"type": "string"}
            }}
        }),
        json!({
            "name": "set_style_set",
            "description": "Apply one of Word's style sets (Design, Style Set): Word 2013+, Word 2010, Word 2003, Basic (Elegant), Basic (Simple), Basic (Stylish), Black & White (Classic), Black & White (Modern), Black & White (Word 2003), Casual, Centered, Lines (Distinctive), Lines (Simple), Lines (Stylish), Minimalist, Shaded. It rewrites Normal, Title, Subtitle, Heading 1 to 9 and Quote and the default paragraph spacing; paragraphs keep their styles. The sets are Docxtor's approximations of Word's.",
            "input_schema": {"type": "object", "required": ["name"], "properties": {"name": {"type": "string"}}}
        }),
        json!({
            "name": "set_paragraph_spacing",
            "description": "Set the spacing between paragraphs and lines for the whole document (Design, Paragraph Spacing): default, none (No paragraph space), compact, tight, open, relaxed or double.",
            "input_schema": {"type": "object", "required": ["preset"], "properties": {"preset": {"type": "string"}}}
        }),
    ]
}

pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "define_style" => define_style(doc, args),
        "create_style_from_selection" => create_style(doc, args),
        "delete_style" => delete_style(doc, args),
        "list_design" => Ok(list_design(doc)),
        "set_theme" => set_theme(doc, args),
        "set_style_set" => set_style_set(doc, args),
        "set_paragraph_spacing" => set_spacing(doc, args),
        _ => return None,
    })
}

fn apply(doc: &mut Document, op: Op) -> Result<(), String> {
    doc.apply_batch(&[op], false).map(|_| ()).map_err(|e| e.to_string())
}

fn string(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}

fn num(args: &Value, k: &str) -> Option<f64> {
    args.get(k).and_then(|v| v.as_f64())
}

fn flag(args: &Value, k: &str) -> Option<bool> {
    args.get(k).and_then(|v| v.as_bool())
}

fn twips(pt: f64) -> i64 {
    (pt * 20.0).round() as i64
}

fn define_style(doc: &mut Document, args: &Value) -> Result<String, String> {
    let style = string(args, "style").ok_or("style is required")?;
    let mut ppr = ParaPatch {
        align: string(args, "align"),
        space_before: num(args, "space_before_pt").map(twips),
        space_after: num(args, "space_after_pt").map(twips),
        line_multiple: num(args, "line_spacing"),
        ind_left: num(args, "indent_left_pt").map(twips),
        ind_right: num(args, "indent_right_pt").map(twips),
        ind_first: num(args, "first_line_indent_pt").map(twips),
        keep_next: flag(args, "keep_with_next"),
        page_break_before: flag(args, "page_break_before"),
        ..Default::default()
    };
    if ppr == ParaPatch::default() {
        ppr.clear = false;
    }
    let rpr = RunPatch {
        font: string(args, "font"),
        size: num(args, "size_pt"),
        bold: flag(args, "bold"),
        italic: flag(args, "italic"),
        underline: flag(args, "underline").map(|u| if u { "single".to_string() } else { "none".to_string() }),
        color: string(args, "color"),
        ..Default::default()
    };
    let exists = doc.styles.lookup(&style).is_some() || crate::styles::builtin_style_xml(&style).is_some();
    let def = StyleDefinition {
        name: if exists { string(args, "name") } else { Some(string(args, "name").unwrap_or_else(|| style.clone())) },
        kind: string(args, "kind"),
        based_on: string(args, "based_on"),
        next: string(args, "next"),
        quick: flag(args, "in_gallery"),
        auto_update: flag(args, "auto_update"),
        hidden: flag(args, "hidden"),
        priority: None,
        ppr: (ppr != ParaPatch::default()).then_some(ppr),
        rpr: (rpr != RunPatch::default()).then_some(rpr),
        lang: None,
    };
    apply(doc, Op::SetStyleDefinition { style: exists.then(|| style.clone()), def })?;
    Ok(if exists { format!("Style \"{style}\" changed; everything that uses it follows.") } else { format!("Style \"{style}\" made.") })
}

fn create_style(doc: &mut Document, args: &Value) -> Result<String, String> {
    let name = string(args, "name").ok_or("name is required")?;
    let id = crate::ai::id_of(args.get("id").ok_or("id is required")?)?;
    let len = doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16();
    let from = args.get("from").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let to = args.get("to").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(len);
    apply(doc, Op::CreateStyle { name: name.clone(), kind: string(args, "kind"), from: crate::ops::Pos::new(id, from.min(len)), to: crate::ops::Pos::new(id, to.min(len)), def: None })?;
    Ok(format!("Style \"{name}\" made from the formatting, and the text now has it."))
}

fn delete_style(doc: &mut Document, args: &Value) -> Result<String, String> {
    let style = string(args, "style").ok_or("style is required")?;
    apply(doc, Op::DeleteStyle { style: style.clone() })?;
    Ok(format!("Style \"{style}\" deleted."))
}

fn list_design(doc: &Document) -> String {
    let info = doc.theme_info();
    let mut out = format!("Theme: {} (colours \"{}\", fonts \"{}\": headings {}, body {}).\n", info.name, info.colors.name, info.fonts.name, info.fonts.major, info.fonts.minor);
    for slot in themes::SLOTS {
        let _ = write!(out, "{slot} {}  ", info.colors.colors.get(slot).cloned().unwrap_or_default());
    }
    let names = |v: Vec<String>| v.join(", ");
    let _ = writeln!(out, "\nThemes: {}.", names(themes::themes().into_iter().map(|t| t.name).collect()));
    let _ = writeln!(out, "Colour schemes: {}.", names(themes::color_schemes().into_iter().map(|t| t.name).collect()));
    let _ = writeln!(out, "Font schemes: {}.", names(themes::font_schemes().into_iter().map(|f| format!("{} ({} / {})", f.name, f.major, f.minor)).collect()));
    let _ = writeln!(out, "Style sets: {}.", names(style_sets::style_set_infos().into_iter().map(|s| s.name).collect()));
    let _ = writeln!(out, "Paragraph spacing: {}.", names(style_sets::SPACING_PRESETS.iter().map(|p| p.0.to_string()).collect()));
    out
}

fn set_theme(doc: &mut Document, args: &Value) -> Result<String, String> {
    let theme = string(args, "theme");
    let colors = match (string(args, "colors_scheme"), args.get("colors").and_then(|v| v.as_object())) {
        (None, None) => None,
        (name, map) => {
            let mut scheme = ColorScheme { name: name.clone().unwrap_or_else(|| "Custom".into()), ..Default::default() };
            if let Some(m) = map {
                for (k, v) in m {
                    scheme.colors.insert(k.clone(), v.as_str().ok_or("colours are hex strings")?.to_string());
                }
            }
            Some(scheme)
        }
    };
    let fonts = match (string(args, "fonts_scheme"), string(args, "heading_font"), string(args, "body_font")) {
        (None, None, None) => None,
        (name, major, minor) => {
            if major.is_some() != minor.is_some() {
                return Err("give both heading_font and body_font".into());
            }
            Some(FontScheme { name: name.unwrap_or_else(|| "Custom".into()), major: major.unwrap_or_default(), minor: minor.unwrap_or_default() })
        }
    };
    if theme.is_none() && colors.is_none() && fonts.is_none() {
        return Err("give a theme, colours or fonts (see list_design)".into());
    }
    apply(doc, Op::SetTheme { theme, definition: None, colors, fonts })?;
    let t = doc.theme_info();
    Ok(format!("The theme is {} now (colours \"{}\", headings in {}, body in {}).", t.name, t.colors.name, t.fonts.major, t.fonts.minor))
}

fn set_style_set(doc: &mut Document, args: &Value) -> Result<String, String> {
    let name = string(args, "name").ok_or("name is required")?;
    apply(doc, Op::SetStyleSet { style_set: Some(name.clone()), definition: None })?;
    Ok(format!("Style set \"{name}\" applied: Normal, Title, Subtitle, Heading 1 to 9 and Quote are rewritten."))
}

fn set_spacing(doc: &mut Document, args: &Value) -> Result<String, String> {
    let preset = string(args, "preset").ok_or("preset is required")?;
    apply(doc, Op::SetParagraphSpacing { preset: Some(preset.clone()), spacing: None })?;
    Ok(format!("Paragraph spacing is \"{preset}\" now."))
}
