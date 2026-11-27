//! Running a tool call: `call_tool` and the helpers its arms share.

use super::docscript::*;
use crate::document::Document;
use crate::model::*;
use crate::ops::{BreakKind, Id, Op, Pos, TableAction};
use crate::props::{Border, ParaBorders, ParaPatch, RunPatch};
use serde_json::{json, Value};
use std::fmt::Write as _;

pub(crate) fn id_of(v: &Value) -> Result<NodeId, String> {
    serde_json::from_value::<Id>(v.clone()).map(|i| i.0).map_err(|e| e.to_string())
}

pub(super) fn ids_of(v: &Value) -> Result<Vec<Id>, String> {
    match v {
        Value::Array(a) => a.iter().map(|x| id_of(x).map(Id)).collect(),
        Value::Null => Ok(vec![]),
        other => Ok(vec![Id(id_of(other)?)]),
    }
}

pub(crate) fn find_in_para(doc: &Document, pid: NodeId, text: &str, all: bool) -> Result<Vec<(usize, usize)>, String> {
    let p = doc.find_any_para(pid).ok_or(format!("paragraph p{pid} not found"))?;
    let hay: Vec<u16> = p.text().encode_utf16().collect();
    let needle: Vec<u16> = text.encode_utf16().collect();
    if needle.is_empty() {
        return Ok(vec![(0, hay.len())]);
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == needle[..] {
            out.push((i, i + needle.len()));
            if !all {
                break;
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
    if out.is_empty() {
        return Err(format!("text {text:?} not found in p{pid}. Current text: {:?}", p.plain_text()));
    }
    Ok(out)
}

/// A single thin line around a paragraph (`box`) or on one side; `none` removes the borders.
pub(super) fn para_borders(kind: &str) -> Result<ParaBorders, String> {
    let line = || Some(Border { val: "single".into(), sz: 4, color: "auto".into(), space: 1 });
    Ok(match kind {
        "none" => ParaBorders::default(),
        "box" => ParaBorders { top: line(), bottom: line(), left: line(), right: line(), ..Default::default() },
        "top" => ParaBorders { top: line(), ..Default::default() },
        "bottom" => ParaBorders { bottom: line(), ..Default::default() },
        "left" => ParaBorders { left: line(), ..Default::default() },
        "right" => ParaBorders { right: line(), ..Default::default() },
        other => return Err(format!("unknown borders '{other}'; use none, box, top, bottom, left or right")),
    })
}

pub(super) fn set_page_layout(doc: &mut Document, args: &Value) -> Result<String, String> {
    let section = match args.get("section") {
        Some(v) if !v.is_null() => Some(Id(id_of(v)?)),
        _ => None,
    };
    let setup = doc.page_setup();
    let order = paragraph_order(&doc.body);
    // The sections to change, each with a paragraph inside it.
    let targets: Vec<(usize, Id)> = match section {
        Some(id) => vec![(doc.section_of(id.0), id)],
        None => setup
            .sections
            .iter()
            .enumerate()
            .filter_map(|(i, sec)| sec.end.or_else(|| order.last().copied()).map(|p| (i, Id(p))))
            .collect(),
    };
    let tw = |v: f64| (v * 20.0).round() as i64;
    let mut ops = Vec::new();
    let orientation = args.get("orientation").and_then(|v| v.as_str());
    if let Some(o) = orientation {
        if !matches!(o, "portrait" | "landscape") {
            return Err("orientation must be portrait or landscape".into());
        }
    }
    let margins = args.get("margins_pt").filter(|m| m.is_object());
    if orientation.is_some() || margins.is_some() {
        for (i, id) in &targets {
            let sec = &setup.sections[*i];
            let (lo, hi) = (sec.width.min(sec.height), sec.width.max(sec.height));
            let size = orientation.map(|o| if o == "landscape" { (hi, lo) } else { (lo, hi) });
            let mut m = sec.margin;
            if let Some(mv) = margins {
                for (k, slot) in ["top", "right", "bottom", "left"].iter().zip(m.iter_mut()) {
                    if let Some(v) = mv.get(*k).and_then(|v| v.as_f64()) {
                        *slot = tw(v.max(0.0));
                    }
                }
            }
            ops.push(Op::SetPageSetup { section: Some(*id), width: size.map(|s| s.0), height: size.map(|s| s.1), margins: margins.map(|_| m) });
        }
    }
    let columns = args.get("columns").and_then(|v| v.as_i64());
    let spacing = args.get("column_spacing_pt").and_then(|v| v.as_f64()).map(tw);
    let line = args.get("column_line").and_then(|v| v.as_bool());
    if columns.is_some() || spacing.is_some() || line.is_some() {
        for (i, id) in &targets {
            let count = columns.unwrap_or(setup.sections[*i].cols.num).clamp(0, 255) as u8;
            ops.push(Op::SetColumns { section: Some(*id), count, space: spacing, sep: line, widths: None, spaces: None });
        }
    }
    if let Some(l) = args.get("line_numbers").and_then(|v| v.as_str()) {
        let (on, restart) = match l {
            "off" => (false, None),
            "each_page" => (true, Some("newPage")),
            "each_section" => (true, Some("newSection")),
            "continuous" => (true, Some("continuous")),
            _ => return Err("line_numbers must be off, each_page, each_section or continuous".into()),
        };
        for (_, id) in &targets {
            ops.push(Op::SetLineNumbers { section: Some(*id), on, count_by: None, restart: restart.map(String::from) });
        }
    }
    if let Some(c) = args.get("page_color").and_then(|v| v.as_str()) {
        ops.push(Op::SetPageColor { color: if c.eq_ignore_ascii_case("none") { None } else { Some(c.to_string()) } });
    }
    if let Some(h) = args.get("hyphenation").and_then(|v| v.as_bool()) {
        ops.push(Op::SetHyphenation { on: h });
    }
    let start = args.get("page_numbers_start").and_then(|v| v.as_i64());
    let format = args.get("page_number_format").and_then(|v| v.as_str()).map(String::from);
    if start.is_some() || format.is_some() {
        for (i, id) in &targets {
            // Without a new start, keep the section's own.
            let start = match start {
                Some(0) => None,
                Some(n) => Some(n),
                None => setup.sections[*i].page_start,
            };
            ops.push(Op::SetPageNumbering { section: Some(*id), start, format: format.clone(), chap_style: None, chap_sep: None });
        }
    }
    if ops.is_empty() {
        return Err("nothing to change: give orientation, margins_pt, columns, line_numbers, page_color, hyphenation or page numbering".into());
    }
    doc.apply_batch(&ops, false).map_err(|e| e.to_string())?;
    let mut out = String::from("Page layout updated.\n");
    let setup = doc.page_setup();
    for (i, sec) in setup.sections.iter().enumerate() {
        let _ = writeln!(
            out,
            "Section {} ({}): {:.1}x{:.1}in, margins {:.2}/{:.2}/{:.2}/{:.2}in, {} column(s){}",
            i + 1,
            snake_case(&sec.start),
            sec.width as f64 / 1440.0,
            sec.height as f64 / 1440.0,
            sec.margin[0] as f64 / 1440.0,
            sec.margin[1] as f64 / 1440.0,
            sec.margin[2] as f64 / 1440.0,
            sec.margin[3] as f64 / 1440.0,
            sec.cols.num,
            if sec.line_numbers.is_some() { ", line numbers" } else { "" }
        );
    }
    Ok(out)
}

/// A short line for a source: tag, type, authors, year, title.
pub(super) fn source_line(s: &crate::citations::Source) -> String {
    let names: Vec<String> = s.authors.iter().chain(if s.authors.is_empty() { s.editors.iter() } else { [].iter() }).map(|n| match n.given() {
        Some(g) => format!("{}, {g}", n.family()),
        None => n.family().to_string(),
    }).collect();
    let year = s.year.as_deref().filter(|y| !y.trim().is_empty()).unwrap_or("n.d.");
    format!("{} [{}] {} ({year}). {}", s.tag, s.kind.id(), if names.is_empty() { "(no author)".to_string() } else { names.join("; ") }, s.title)
}

pub(super) fn list_sources(doc: &Document) -> String {
    let sources = doc.sources();
    let cites = doc.citations();
    let mut out = format!("Citation style: {}. ", doc.citation_style());
    if sources.is_empty() {
        out.push_str("The document has no sources yet. Add one with add_source.");
        return out;
    }
    let _ = writeln!(out, "{} source(s):", sources.len());
    for s in &sources {
        let n = cites.iter().filter(|c| c.tags.contains(&s.tag)).count();
        let _ = writeln!(out, "{} — cited {n} time{}", source_line(s), if n == 1 { "" } else { "s" });
    }
    out
}

pub(super) fn add_source(doc: &mut Document, args: &Value) -> Result<String, String> {
    let new: Vec<crate::citations::Source> = match args.get("bibtex").and_then(|b| b.as_str()).filter(|b| !b.trim().is_empty()) {
        Some(text) => crate::bibtex::parse(text)?,
        None => {
            // The fields may also come wrapped in `source`, as in the raw op.
            let mut obj = args.get("source").and_then(|s| s.as_object()).or_else(|| args.as_object()).cloned().ok_or("give the source's fields or bibtex text")?;
            // Tool arguments are snake_case; the source's JSON is camelCase.
            if let Some(v) = obj.remove("container_title") {
                obj.insert("containerTitle".into(), v);
            }
            obj.remove("bibtex");
            vec![serde_json::from_value(Value::Object(obj)).map_err(|e| format!("Couldn't read the source: {e}"))?]
        }
    };
    let before = doc.sources().len();
    let ops: Vec<Op> = new.iter().map(|s| Op::AddSource { source: s.clone() }).collect();
    doc.apply_batch(&ops, false).map_err(|e| e.to_string())?;
    let all = doc.sources();
    let mut out = format!("Added {} source(s):\n", new.len());
    for s in &all[before..] {
        let _ = writeln!(out, "{}", source_line(s));
    }
    out.push_str("Cite them with the cite tool, using the tags.");
    Ok(out)
}

/// Execute a tool call. Returns text for the model (errors are returned as `Err` text
/// which callers should pass back to the model as a tool error).
pub fn call_tool(doc: &mut Document, name: &str, args: &Value) -> Result<String, String> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).map(String::from);
    let b = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    let apply = |doc: &mut Document, ops: Vec<Op>| -> Result<Vec<crate::ops::OpResult>, String> { doc.apply_batch(&ops, false).map_err(|e| e.to_string()) };
    match name {
        "read_document" => {
            let start = match args.get("start") {
                Some(v) if !v.is_null() => Some(id_of(v)?),
                _ => None,
            };
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(400) as usize;
            Ok(docscript(doc, &ScriptOpts { start, limit }))
        }
        "get_outline" => Ok(outline(doc)),
        "list_styles" => {
            let mut out = String::new();
            for st in doc.style_list() {
                if st.hidden && !st.quick {
                    continue;
                }
                let _ = writeln!(out, "{} ({}) — \"{}\"", st.id, st.kind, st.name);
            }
            out.push_str("Built-in styles that can always be used: Title, Subtitle, Heading1-9, Quote, Intense Quote, Emphasis, Strong, No Spacing, Code, ListParagraph. Change a style's look with define_style; the theme, style sets and paragraph spacing are in list_design.\n");
            Ok(out)
        }
        "search" => {
            let q = s("query").ok_or("query is required")?;
            let opts: crate::document::FindOptions = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            let found = doc.find(&q, &opts).map_err(|e| e.to_string())?;
            let mut hits: Vec<u32> = Vec::new();
            for h in &found.hits {
                if !hits.contains(&h.p) {
                    hits.push(h.p);
                }
            }
            if hits.is_empty() {
                return Ok(format!("No paragraphs contain {q:?}."));
            }
            let n = hits.len();
            hits.truncate(50);
            Ok(format!("{n} matching paragraph(s):\n{}", script_for_paragraphs(doc, &hits)))
        }
        "replace_text" => {
            let ids = match args.get("ids") {
                Some(v) if !v.is_null() => Some(ids_of(v)?),
                _ => None,
            };
            let options: crate::document::FindOptions = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            let part = |k: &str| args.get(k).filter(|v| !v.is_null());
            let op = Op::ReplaceText {
                find: s("find").unwrap_or_default(),
                replace: s("replace").unwrap_or_default(),
                ids,
                all: b("all"),
                options,
                format: part("format").map(|v| serde_json::from_value(v.clone()).map_err(|e| format!("format: {e}"))).transpose()?,
                para_format: part("para_format").map(|v| serde_json::from_value(v.clone()).map_err(|e| format!("para_format: {e}"))).transpose()?,
                language: s("language"),
            };
            let r = apply(doc, vec![op])?;
            let pid = r[0].caret.map(|c| c.p.0);
            let count = r[0].count.unwrap_or(0);
            Ok(format!("Replaced {count} occurrence(s).\n{}", pid.map(|p| script_for_paragraphs(doc, &[p])).unwrap_or_default()))
        }
        "rewrite_paragraph" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            apply(doc, vec![Op::SetParagraphText { id: Id(id), markdown: s("markdown").unwrap_or_default() }])?;
            Ok(format!("Updated:\n{}", script_for_paragraphs(doc, &[id])))
        }
        "insert_content" => {
            let after = args.get("after").filter(|v| !v.is_null()).map(id_of).transpose()?.map(Id);
            let before = args.get("before").filter(|v| !v.is_null()).map(id_of).transpose()?.map(Id);
            let r = apply(doc, vec![Op::InsertMarkdown { after, before, markdown: s("markdown").ok_or("markdown is required")? }])?;
            let created = r[0].created.clone();
            Ok(format!("Inserted {} block(s):\n{}", created.len(), script_for_paragraphs(doc, &created)))
        }
        "insert_html" => {
            let after = args.get("after").filter(|v| !v.is_null()).map(id_of).transpose()?.map(Id);
            let before = args.get("before").filter(|v| !v.is_null()).map(id_of).transpose()?.map(Id);
            let html = s("html").ok_or("html is required")?;
            // A fresh paragraph to paste into; the pasted blocks take its place.
            let r = apply(doc, vec![Op::InsertParagraph { after, before, text: String::new(), style: None }])?;
            let id = r[0].created.first().copied().ok_or("could not add a paragraph")?;
            let r = apply(doc, vec![Op::PasteHtml { at: crate::ops::Pos::new(id, 0), html, mode: crate::fragment::PasteMode::Keep }])?;
            let mut created = vec![id];
            created.extend(r[0].created.iter().copied());
            created.sort_unstable();
            Ok(format!("Inserted {} block(s):\n{}", created.len(), script_for_paragraphs(doc, &created)))
        }
        "delete_blocks" => {
            let ids = ids_of(args.get("ids").unwrap_or(&Value::Null))?;
            let n = ids.len();
            apply(doc, vec![Op::DeleteBlocks { ids }])?;
            Ok(format!("Deleted {n} block(s)."))
        }
        "set_header_footer" => {
            let kind = match s("kind").as_deref() {
                Some("header") => crate::ops::HeaderKind::Header,
                Some("footer") => crate::ops::HeaderKind::Footer,
                _ => return Err("kind must be header or footer".into()),
            };
            let variant = s("variant").unwrap_or_else(|| "default".into());
            let section = args.get("section").filter(|v| !v.is_null()).map(id_of).transpose()?.map(Id);
            let link = args.get("link_to_previous").and_then(Value::as_bool);
            if link == Some(true) {
                apply(doc, vec![Op::LinkHeaderFooter { kind, variant: variant.clone(), section, linked: true }])?;
                return Ok(format!("{} ({variant}) now shows the one of the previous section.", s("kind").unwrap_or_default()));
            }
            if link == Some(false) {
                apply(doc, vec![Op::LinkHeaderFooter { kind, variant: variant.clone(), section, linked: false }])?;
                if s("text").is_none() {
                    return Ok(format!("{} ({variant}) is now the section's own copy.", s("kind").unwrap_or_default()));
                }
            }
            let text = s("text").ok_or("text is required")?;
            let r = apply(doc, vec![Op::AddHeaderFooter { kind, variant: variant.clone(), section }])?;
            let pid = r.first().and_then(|r| r.caret).ok_or("could not create the header/footer")?.p.0;
            // Text runs between {FIELD} placeholders; fields become live.
            let mut parts: Vec<(bool, String)> = Vec::new();
            let mut rest = text.as_str();
            while let Some(i) = rest.find('{') {
                match rest[i..].find('}') {
                    Some(j) if ["PAGE", "NUMPAGES", "DATE"].contains(&&rest[i + 1..i + j]) => {
                        parts.push((false, rest[..i].to_string()));
                        parts.push((true, rest[i + 1..i + j].to_string()));
                        rest = &rest[i + j + 1..];
                    }
                    _ => {
                        parts.push((false, rest[..=i].to_string()));
                        rest = &rest[i + 1..];
                    }
                }
            }
            parts.push((false, rest.to_string()));
            let mut ops = vec![Op::SetParagraphText { id: Id(pid), markdown: String::new() }];
            for (is_field, t) in parts.into_iter().filter(|(_, t)| !t.is_empty()) {
                // Offsets are resolved as each op lands, so append at a large offset (clamped to the end).
                let at = Pos::new(pid, usize::MAX / 2);
                ops.push(if is_field { Op::InsertField { at, field: t, result: None, input: None } } else { Op::PasteMarkdown { at, markdown: t } });
            }
            apply(doc, ops)?;
            Ok(format!("{} ({variant}) set:\n{}", s("kind").unwrap_or_default(), script_for_paragraphs(doc, &[pid])))
        }
        "insert_footnote" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let after = s("after_text").ok_or("after_text is required")?;
            let ranges = find_in_para(doc, id, &after, false)?;
            let r = apply(doc, vec![Op::InsertFootnote { at: Pos::new(id, ranges[0].1), text: s("text").unwrap_or_default() }])?;
            let note = r.first().and_then(|r| r.caret).map(|c| c.p.0);
            let mut ids = vec![id];
            ids.extend(note);
            Ok(format!("Footnote added:\n{}", script_for_paragraphs(doc, &ids)))
        }
        "format_text" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let format: RunPatch = serde_json::from_value(args.get("format").cloned().unwrap_or(json!({}))).map_err(|e| e.to_string())?;
            let ranges = find_in_para(doc, id, &s("text").unwrap_or_default(), b("all"))?;
            let ops = ranges.iter().map(|(a, e)| Op::FormatText { from: Pos::new(id, *a), to: Pos::new(id, *e), format: format.clone() }).collect();
            apply(doc, ops)?;
            Ok(format!("Formatted {} range(s):\n{}", ranges.len(), script_for_paragraphs(doc, &[id])))
        }
        "change_case" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let mode: crate::charfmt::CaseMode = serde_json::from_value(args.get("mode").cloned().unwrap_or(Value::Null)).map_err(|_| "mode must be sentence, lower, upper, title or toggle".to_string())?;
            let ranges = find_in_para(doc, id, &s("text").unwrap_or_default(), b("all"))?;
            let ops = ranges.iter().map(|(a, e)| Op::ChangeCase { from: Pos::new(id, *a), to: Pos::new(id, *e), mode }).collect();
            apply(doc, ops)?;
            Ok(format!("Changed the case of {} range(s):\n{}", ranges.len(), script_for_paragraphs(doc, &[id])))
        }
        "set_link" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let ranges = find_in_para(doc, id, &s("text").ok_or("text is required")?, false)?;
            let url = s("url").filter(|u| !u.is_empty());
            apply(doc, vec![Op::SetLink { from: Pos::new(id, ranges[0].0), to: Pos::new(id, ranges[0].1), url }])?;
            Ok(script_for_paragraphs(doc, &[id]))
        }
        "set_paragraph_style" => {
            let ids = ids_of(args.get("ids").unwrap_or(&Value::Null))?;
            let raw: Vec<NodeId> = ids.iter().map(|i| i.0).collect();
            apply(doc, vec![Op::SetStyle { ids, style: s("style").ok_or("style is required")? }])?;
            Ok(script_for_paragraphs(doc, &raw))
        }
        "format_paragraphs" => {
            let ids = ids_of(args.get("ids").unwrap_or(&Value::Null))?;
            let raw: Vec<NodeId> = ids.iter().map(|i| i.0).collect();
            let pt = |k: &str| args.get(k).and_then(|v| v.as_f64()).map(|v| (v * 20.0).round() as i64);
            let flag = |k: &str| args.get(k).and_then(|v| v.as_bool());
            let patch = ParaPatch {
                align: s("align"),
                space_before: pt("space_before_pt"),
                space_after: pt("space_after_pt"),
                line_multiple: args.get("line_spacing").and_then(|v| v.as_f64()),
                line: pt("line_spacing_exactly_pt").or(pt("line_spacing_at_least_pt")),
                line_rule: if args.get("line_spacing_exactly_pt").is_some() {
                    Some("exact".into())
                } else if args.get("line_spacing_at_least_pt").is_some() {
                    Some("atLeast".into())
                } else {
                    None
                },
                ind_left: pt("indent_left_pt"),
                ind_right: pt("indent_right_pt"),
                ind_first: pt("first_line_pt"),
                mirror_indents: flag("mirror_indents"),
                contextual_spacing: flag("dont_add_space_between_same_style"),
                suppress_auto_hyphens: flag("dont_hyphenate"),
                bidi: match s("direction").as_deref() {
                    Some("rtl") => Some(true),
                    Some("ltr") => Some(false),
                    Some(_) => return Err("direction must be ltr or rtl".into()),
                    None => None,
                },
                outline_lvl: args.get("outline_level").and_then(|v| v.as_i64()).map(|l| if l <= 0 { 9 } else { (l - 1).min(8) }),
                tabs: args
                    .get("tab_stops")
                    .and_then(|v| v.as_array())
                    .map(|list| {
                        list.iter()
                            .map(|t| {
                                let pos = t.get("position_pt").and_then(|v| v.as_f64()).ok_or("each tab stop needs position_pt")?;
                                let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("left");
                                if !matches!(kind, "left" | "center" | "right" | "decimal" | "bar") {
                                    return Err(format!("tab stop kind {kind:?} is not one of left, center, right, decimal, bar"));
                                }
                                let leader = t.get("leader").and_then(|v| v.as_str()).filter(|l| *l != "none").map(String::from);
                                Ok(crate::props::TabStop { val: kind.into(), pos: (pos * 20.0).round() as i64, leader })
                            })
                            .collect::<Result<Vec<_>, String>>()
                    })
                    .transpose()?,
                keep_next: flag("keep_with_next"),
                keep_lines: flag("keep_lines_together"),
                page_break_before: flag("page_break_before"),
                widow_control: flag("prevent_single_lines"),
                shading: s("shading").map(|c| if c.eq_ignore_ascii_case("none") { String::new() } else { c }),
                borders: s("borders").map(|b| para_borders(&b)).transpose()?,
                ..Default::default()
            };
            let mut ops = vec![Op::FormatParagraphs { ids: ids.clone(), format: patch }];
            if let Some(style) = s("list_style") {
                ops.push(Op::SetListStyle { ids: ids.clone(), style });
            } else {
                match s("list").as_deref() {
                    Some("bullet") => ops.push(Op::ToggleList { ids: ids.clone(), kind: crate::numbering::ListKind::Bullet }),
                    Some("decimal") => ops.push(Op::ToggleList { ids: ids.clone(), kind: crate::numbering::ListKind::Decimal }),
                    Some("none") => {
                        ops.push(Op::FormatParagraphs { ids: ids.clone(), format: ParaPatch { num: Some((0, 0)), ..Default::default() } });
                    }
                    _ => {}
                }
            }
            if let Some(checked) = args.get("checked").and_then(|v| v.as_bool()) {
                ops.push(Op::SetChecked { ids: ids.clone(), checked });
            }
            apply(doc, ops)?;
            if let Some(lvl) = args.get("list_level").and_then(|v| v.as_i64()) {
                let labels = doc.numbering.labels(&doc.body, &doc.styles);
                let mut ops = Vec::new();
                for id in &ids {
                    if let Some(l) = labels.get(&id.0) {
                        ops.push(Op::Indent { ids: vec![*id], delta: lvl - l.level });
                    }
                }
                apply(doc, ops)?;
            }
            Ok(script_for_paragraphs(doc, &raw))
        }
        "insert_break" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let kind: BreakKind = serde_json::from_value(args.get("kind").cloned().unwrap_or(Value::Null))
                .map_err(|_| "kind must be one of page, column, next_page, continuous, even_page, odd_page".to_string())?;
            let end = match s("after_text") {
                Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
                _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
            };
            let r = apply(doc, vec![Op::InsertBreak { at: Pos::new(id, end), kind }])?;
            let mut ids = vec![id];
            ids.extend(r[0].created.iter().copied());
            Ok(format!("Inserted a {} break:\n{}", s("kind").unwrap_or_default().replace('_', " "), script_for_paragraphs(doc, &ids)))
        }
        "set_page_layout" => set_page_layout(doc, args),
        "add_bookmark" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let name = s("name").ok_or("name is required")?;
            let range = find_in_para(doc, id, &s("text").unwrap_or_default(), false)?[0];
            apply(doc, vec![Op::InsertBookmark { from: Pos::new(id, range.0), to: Pos::new(id, range.1), name: name.clone() }])?;
            Ok(format!("Bookmark {} added:\n{}", name.trim(), script_for_paragraphs(doc, &[id])))
        }
        "delete_bookmark" => {
            let name = s("name").ok_or("name is required")?;
            apply(doc, vec![Op::DeleteBookmark { name: name.clone() }])?;
            Ok(format!("Bookmark {} deleted.", name.trim()))
        }
        "insert_horizontal_line" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let end = match s("after_text") {
                Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
                _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
            };
            let r = apply(doc, vec![Op::InsertHorizontalLine { at: Pos::new(id, end) }])?;
            let mut ids = vec![id];
            ids.extend(r[0].created.iter().copied());
            Ok(format!("Horizontal line inserted:\n{}", script_for_paragraphs(doc, &ids)))
        }
        "set_watermark" => {
            let text = s("text").ok_or("text is required")?;
            let layout = match s("layout").as_deref() {
                None | Some("diagonal") => crate::marks::WatermarkLayout::Diagonal,
                Some("horizontal") => crate::marks::WatermarkLayout::Horizontal,
                Some(_) => return Err("layout must be diagonal or horizontal".into()),
            };
            if text.trim().eq_ignore_ascii_case("none") {
                apply(doc, vec![Op::SetWatermark { text: None, layout, color: None, font: None, opacity: None }])?;
                return Ok("Watermark removed.".into());
            }
            let opacity = args.get("opacity").and_then(|v| v.as_f64());
            apply(doc, vec![Op::SetWatermark { text: Some(text.clone()), layout, color: s("color"), font: s("font"), opacity }])?;
            let headers = doc.page_setup().parts.keys().filter(|p| p.contains("header")).count();
            Ok(format!("Watermark {:?} set ({}) in {headers} header(s).", text.trim(), if layout == crate::marks::WatermarkLayout::Horizontal { "horizontal" } else { "diagonal" }))
        }
        "check_accessibility" => Ok(doc.accessibility_report()),
        "set_document_properties" => {
            let known = [
                "title",
                "subject",
                "author",
                "keywords",
                "description",
                "category",
                "status",
                "company",
                "manager",
            ];
            let custom: Vec<crate::docmeta::CustomProp> = match args.get("custom") {
                Some(v) if !v.is_null() => v
                    .as_array()
                    .ok_or("custom must be a list")?
                    .iter()
                    .map(|c| {
                        let name = c
                            .get("name")
                            .and_then(|v| v.as_str())
                            .ok_or("each custom property needs a name")?
                            .to_string();
                        let value = match c.get("value") {
                            Some(serde_json::Value::String(v)) => v.clone(),
                            Some(v) if !v.is_null() => v.to_string(),
                            _ => return Err(format!("custom property {name} needs a value")),
                        };
                        let kind = match c.get("type").and_then(|v| v.as_str()) {
                            None | Some("text") => crate::docmeta::PropKind::Text,
                            Some("number") => crate::docmeta::PropKind::Number,
                            Some("date") => crate::docmeta::PropKind::Date,
                            Some("bool") => crate::docmeta::PropKind::Bool,
                            Some(other) => {
                                return Err(format!(
                                    "type must be text, number, date or bool, not {other}"
                                ))
                            }
                        };
                        Ok(crate::docmeta::CustomProp { name, kind, value })
                    })
                    .collect::<Result<_, String>>()?,
                _ => Vec::new(),
            };
            let remove: Vec<String> = args
                .get("remove_custom")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if !known
                .iter()
                .any(|k| args.get(*k).is_some_and(|v| v.is_string()))
                && custom.is_empty()
                && remove.is_empty()
            {
                return Err("give at least one of title, subject, author, keywords, description, category, status, company, manager, custom or remove_custom".into());
            }
            let mut ops = vec![Op::SetDocProps {
                title: s("title"),
                subject: s("subject"),
                author: s("author"),
                keywords: s("keywords"),
                description: s("description"),
                category: s("category"),
                status: s("status"),
                company: s("company"),
                manager: s("manager"),
            }];
            if !custom.is_empty() || !remove.is_empty() {
                ops.push(Op::SetCustomProps {
                    set: custom,
                    remove,
                });
            }
            apply(doc, ops)?;
            let p = doc.doc_props();
            let mut out = format!(
                "Document properties now: title {:?}, subject {:?}, author {:?}, keywords {:?}, description {:?}, category {:?}, status {:?}, company {:?}, manager {:?}.",
                p.title.unwrap_or_default(),
                p.subject.unwrap_or_default(),
                p.author.unwrap_or_default(),
                p.keywords.unwrap_or_default(),
                p.description.unwrap_or_default(),
                p.category.unwrap_or_default(),
                p.status.unwrap_or_default(),
                p.company.unwrap_or_default(),
                p.manager.unwrap_or_default()
            );
            if !p.custom.is_empty() {
                out.push_str(" Custom properties: ");
                out.push_str(
                    &p.custom
                        .iter()
                        .map(|c| format!("{} = {:?}", c.name, c.value))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push('.');
            }
            Ok(out)
        }
        "set_language" => {
            let lang = s("lang").ok_or("lang is required")?;
            let ids = match args.get("ids") {
                Some(v) if !v.is_null() => Some(ids_of(v)?),
                _ => None,
            };
            apply(doc, vec![Op::SetLanguage { lang: lang.clone(), ids: ids.clone() }])?;
            Ok(match ids {
                Some(ids) => format!("Language of {} paragraph(s) set to {}.", ids.len(), lang.trim()),
                None => format!("Document language set to {}.", doc.language().unwrap_or(lang)),
            })
        }
        "edit_table" => {
            let cell = id_of(args.get("cell").ok_or("cell is required")?)?;
            let action: TableAction = serde_json::from_value(args.get("action").cloned().unwrap_or(Value::Null)).map_err(|e| e.to_string())?;
            apply(doc, vec![Op::TableEdit { at: Id(cell), action }])?;
            Ok("Table updated. Use read_document to see new cell ids.".into())
        }
        "set_table_design" => {
            let cell = id_of(args.get("cell").ok_or("cell is required")?)?;
            let flag = |k: &str| args.get(k).and_then(|v| v.as_bool());
            let look = crate::ops::TableLookPatch {
                first_row: flag("header_row"),
                last_row: flag("total_row"),
                first_column: flag("first_column"),
                last_column: flag("last_column"),
                banded_rows: flag("banded_rows"),
                banded_columns: flag("banded_columns"),
            };
            let look = (look != Default::default()).then_some(look);
            let style = s("design");
            if style.is_none() && look.is_none() {
                return Err("give a design or at least one option".into());
            }
            apply(doc, vec![Op::TableDesign { at: Id(cell), style: style.clone(), definition: None, look }])?;
            Ok(match style {
                Some(d) => format!("The table now has the design \"{d}\"."),
                None => "The table's style options are changed.".into(),
            })
        }
        "list_sources" => Ok(list_sources(doc)),
        "add_source" => add_source(doc, args),
        "cite" => {
            let id = id_of(args.get("id").ok_or("id is required")?)?;
            let tags: Vec<String> = match args.get("tags") {
                Some(Value::Array(a)) => a.iter().filter_map(|t| t.as_str()).map(|t| t.trim().to_string()).collect(),
                Some(Value::String(t)) => t.split([',', ';']).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
                _ => return Err("tags is required: the tags of the sources to cite (see list_sources)".into()),
            };
            let at = match s("after_text") {
                Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
                _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
            };
            // A citation in the text is set off by a space, as Word writes "water (Okafor 2020)": text that ends right where the citation
            // goes gets one first. Not in a notes style (the citation is a footnote mark, which sits against the word), and not after a
            // space or an opening bracket.
            let before = doc.find_any_para(id).and_then(|p| at.checked_sub(1).and_then(|i| p.text().encode_utf16().nth(i))).and_then(|u| char::from_u32(u32::from(u)));
            let spaced = !doc.notes_style() && before.is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | '[' | '{' | '\u{201c}' | '"'));
            let mut ops = Vec::new();
            if spaced {
                ops.push(Op::InsertText { at: Pos::new(id, at), text: " ".into(), format: None });
            }
            let at = if spaced { at + 1 } else { at };
            ops.push(Op::InsertCitation { at: Pos::new(id, at), tags, page: s("page"), suppress_author: None, suppress_year: None, suppress_title: None });
            apply(doc, ops)?;
            Ok(format!("Citation inserted:\n{}", script_for_paragraphs(doc, &[id])))
        }
        "insert_bibliography" => {
            let after = args.get("after").filter(|v| !v.is_null()).map(id_of).transpose()?.map(Id);
            let cited = doc.citations().len();
            let r = apply(doc, vec![Op::InsertBibliography { after, before: None, heading: s("heading") }])?;
            let created = r[0].created.clone();
            let mut out = format!("Bibliography inserted ({} style):\n{}", doc.citation_style(), script_for_paragraphs(doc, &created));
            if cited == 0 {
                out.push_str("\nNothing is cited yet, so the list is empty: it lists only cited sources. Use `cite`, then apply_ops update_bibliography.");
            }
            Ok(out)
        }
        "set_citation_style" => {
            let style = s("style").ok_or("style is required: apa, mla, chicago, ieee or harvard, or a CSL style's id")?;
            let r = apply(doc, vec![Op::SetCitationStyle { style }])?;
            Ok(format!("Citation style set to {}; {} field(s) written again.", doc.citation_style(), r[0].count.unwrap_or(0)))
        }
        "apply_ops" => {
            let ops: Vec<Op> = serde_json::from_value(args.get("ops").cloned().unwrap_or(json!([]))).map_err(|e| e.to_string())?;
            let r = apply(doc, ops)?;
            Ok(serde_json::to_string(&r).unwrap_or_default())
        }
        other => crate::ai_refs::call(doc, other, args).or_else(|| crate::ai_lists::call(doc, other, args)).or_else(|| crate::shapes::ai::call(doc, other, args)).or_else(|| crate::math::call_tool(doc, other, args)).or_else(|| crate::ai_charts::call(doc, other, args)).or_else(|| crate::ai_forms::call(doc, other, args)).or_else(|| crate::ai_east_asian::call(doc, other, args)).or_else(|| crate::ai_design::call(doc, other, args)).or_else(|| crate::ai_outline::call(doc, other, args)).or_else(|| crate::ai_review::call(doc, other, args)).or_else(|| crate::ai_merge::call(doc, other, args)).unwrap_or_else(|| Err(format!("unknown tool {other}"))),
    }
}
