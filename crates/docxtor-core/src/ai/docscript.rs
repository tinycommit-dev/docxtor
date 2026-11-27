//! DocScript: the ID-addressed text view of the document, its Markdown flavours, and the outline and summary views.

use crate::document::Document;
use crate::markdown::escape;
use crate::model::*;
use crate::numbering::ListLabel;
use crate::ops::Pos;
use crate::render::{RBlock, RPara, RSeg};
use std::fmt::Write as _;

/// DocScript shows every object so models don't drop them; plain Markdown keeps only what readers see.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Flavor {
    DocScript,
    Markdown,
}

/// Text in Markdown that is read as text and nothing else: the characters that are Markdown syntax, and `<`, which would
/// start a raw HTML tag (`<script>`, `<img onerror=...>`) or an autolink (`<javascript:...>`) in a renderer that allows them.
pub(super) fn escape_md(s: &str) -> String {
    escape(s).replace('<', "\\<")
}

/// The address inside `(...)` of a Markdown link or image, with the characters that would end it written as `%` escapes.
pub(super) fn md_dest(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() || matches!(c, ' ' | '(' | ')' | '<' | '>' | '"' | '\\' | '[' | ']') {
            let mut b = [0u8; 4];
            for byte in c.encode_utf8(&mut b).bytes() {
                let _ = write!(o, "%{byte:02X}");
            }
        } else {
            o.push(c);
        }
    }
    o
}

pub(super) fn seg_md(s: &RSeg, base: &crate::props::RunFmt, flavor: Flavor) -> String {
    let md = flavor == Flavor::Markdown;
    let mut t = match s.k {
        "t" if md => escape_md(s.x.as_deref().unwrap_or("")),
        "t" => escape(s.x.as_deref().unwrap_or("")),
        "tab" => "\t".into(),
        "br" if md => "\\\n".into(),
        "br" => "\\n".into(),
        "pbr" | "cbr" if md => String::new(),
        "pbr" => "<pagebreak>".into(),
        "cbr" => "<columnbreak>".into(),
        "img" if md => {
            let i = s.img.as_ref().unwrap();
            // A picture in the file, or on the web. A picture linked by another kind of address (`file:`, `data:`, `javascript:`) is not one a
            // reader of the Markdown should be sent to.
            let scheme = i.src.split(['/', '?', '#']).next().is_some_and(|first| first.contains(':'));
            let web = i.src.starts_with("http://") || i.src.starts_with("https://");
            if scheme && !web {
                escape_md(i.alt.as_deref().unwrap_or(""))
            } else {
                format!("![{}]({})", escape_md(i.alt.as_deref().unwrap_or("")), md_dest(i.src.strip_prefix("word/").unwrap_or(&i.src)))
            }
        }
        "img" => {
            let i = s.img.as_ref().unwrap();
            format!("![image {}x{}pt{}]", i.w.round(), i.h.round(), i.alt.as_ref().map(|a| format!(" \"{a}\"")).unwrap_or_default())
        }
        "shape" if !md => shape_md(s.shape.as_deref()),
        "note" if s.note == Some("endnote") => format!("[^end:{}]", s.label.as_deref().unwrap_or("")),
        "note" => format!("[^{}]", s.label.as_deref().unwrap_or("")),
        // An equation is `$latex$` inline and `$$latex$$` on its own line, in DocScript and in Markdown alike.
        "math" => {
            let m = s.math.as_ref();
            let latex = m.map(|m| m.latex.as_str()).unwrap_or("");
            if m.is_some_and(|m| m.display) {
                format!("$${latex}$$")
            } else {
                format!("${latex}$")
            }
        }
        _ if md => String::new(),
        "ghost" => format!("[tracked deletion: {:?}]", s.x.as_deref().unwrap_or("")),
        _ => format!("![{}]", s.label.as_deref().unwrap_or("object")),
    };
    if s.k != "t" {
        return t;
    }
    let f = &s.f;
    let ws_l = t.len() - t.trim_start().len();
    let ws_r = t.len() - t.trim_end().len();
    if t.trim().is_empty() {
        return t;
    }
    let (lead, core, trail) = (t[..ws_l].to_string(), t[ws_l..t.len() - ws_r].to_string(), t[t.len() - ws_r..].to_string());
    let mut core = core;
    // Only mark formatting that differs from the paragraph style, so headings don't read as **bold**.
    let on = |v: Option<bool>| v == Some(true);
    let ul = |v: &Option<String>| v.as_deref().is_some_and(|u| u != "none");
    if on(f.strike) && !on(base.strike) {
        core = format!("~~{core}~~");
    }
    if ul(&f.underline) && !ul(&base.underline) && s.link.is_none() {
        core = format!("<u>{core}</u>");
    }
    if on(f.italic) && !on(base.italic) {
        core = format!("*{core}*");
    }
    if on(f.bold) && !on(base.bold) {
        core = format!("**{core}**");
    }
    t = format!("{lead}{core}{trail}");
    t
}

/// A shape or text box in DocScript: `[shape: rect "text…"]`. The text itself follows the paragraph as an indented block.
pub(super) fn shape_md(sh: Option<&crate::shapes::RShape>) -> String {
    let Some(sh) = sh else { return "[shape]".into() };
    // SmartArt: the texts of its shapes (a model can read them, not edit them).
    if sh.kind == "diagram" {
        let texts: Vec<String> = sh
            .children
            .iter()
            .filter_map(|c| c.dtext.as_ref())
            .map(|t| t.paras.iter().map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>()).collect::<Vec<_>>().join(" "))
            .filter(|t| !t.trim().is_empty())
            .collect();
        let joined = texts.join("; ");
        let mut snippet: String = joined.chars().take(80).collect();
        if joined.chars().count() > 80 {
            snippet.push('\u{2026}');
        }
        return if snippet.is_empty() { "[SmartArt]".into() } else { format!("[SmartArt: {snippet:?}]") };
    }
    let mut name = if sh.text_box { "textbox".to_string() } else { sh.geometry.clone() };
    if let Some(w) = &sh.warp {
        name = format!("wordart {}", w.preset);
    }
    let looks = crate::shapes::appearance::looks_of(sh);
    if !looks.is_empty() {
        name = format!("{name} ({looks})");
    }
    let mut text = String::new();
    if let Some(blocks) = &sh.text {
        shape_plain(blocks, &mut text);
    }
    // VML WordArt keeps its text in the warp.
    if text.trim().is_empty() {
        text = sh.warp.as_ref().and_then(|w| w.text.clone()).unwrap_or_default();
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return match sh.kind {
            "group" | "canvas" => format!("[shape: {} of {}]", sh.kind, sh.children.len()),
            _ => format!("[shape: {name}]"),
        };
    }
    let mut snippet: String = text.chars().take(40).collect();
    if text.chars().count() > 40 {
        snippet.push('…');
    }
    format!("[shape: {name} {snippet:?}]")
}

pub(super) fn shape_plain(blocks: &[RBlock], out: &mut String) {
    for b in blocks {
        match b {
            RBlock::Para(p) => {
                for s in &p.segs {
                    if s.k == "t" {
                        out.push_str(s.x.as_deref().unwrap_or(""));
                    }
                }
                out.push(' ');
            }
            RBlock::Table(t) => t.rows.iter().flat_map(|r| &r.cells).for_each(|c| shape_plain(&c.blocks, out)),
            RBlock::Container { blocks, .. } => shape_plain(blocks, out),
            RBlock::Opaque { .. } => {}
        }
    }
}

/// The text inside the shapes of a paragraph, as indented blocks with their paragraph ids.
pub(super) fn shape_blocks(p: &RPara, indent: usize, out: &mut String) {
    fn go(sh: &crate::shapes::RShape, indent: usize, out: &mut String) {
        if let Some(blocks) = &sh.text {
            for b in blocks {
                block_script(b, indent, out);
            }
        }
        for c in &sh.children {
            go(c, indent, out);
        }
    }
    for s in &p.segs {
        if let Some(sh) = &s.shape {
            go(sh, indent + 1, out);
        }
    }
}

pub(super) fn para_md(p: &RPara) -> String {
    para_md_as(p, Flavor::DocScript)
}

pub(crate) fn para_md_as(p: &RPara, flavor: Flavor) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < p.segs.len() {
        let s = &p.segs[i];
        // The result of a cross-reference is text; the hidden bookmark it links to isn't something to show.
        let hidden_ref = |s: &RSeg| s.link.as_deref().is_some_and(|l| l.starts_with("#_Ref")) && s.fld.is_some();
        if hidden_ref(s) {
            while i < p.segs.len() && hidden_ref(&p.segs[i]) {
                let mut seg = p.segs[i].clone();
                seg.link = None;
                out.push_str(&seg_md(&seg, &p.mark, flavor));
                i += 1;
            }
            continue;
        }
        // A citation is one unit: its sources, not the text Word formats for it.
        if let (Flavor::DocScript, Some(tags)) = (flavor, &s.cite) {
            let mut j = i;
            while j < p.segs.len() && p.segs[j].cite.as_ref() == Some(tags) {
                j += 1;
            }
            let _ = write!(out, "[{}]", tags.iter().map(|t| format!("@{t}")).collect::<Vec<_>>().join("; "));
            i = j;
            continue;
        }
        if let Some(link) = &s.link {
            let mut inner = String::new();
            while i < p.segs.len() && p.segs[i].link.as_ref() == Some(link) {
                inner.push_str(&seg_md(&p.segs[i], &p.mark, flavor));
                i += 1;
            }
            if flavor == Flavor::Markdown {
                // A script address (`javascript:`, `data:`, `file:`) is not a link a reader may follow: the text stays, the link goes.
                match crate::export::common::safe_link(link) {
                    Some(l) => {
                        let _ = write!(out, "[{inner}]({})", md_dest(l));
                    }
                    None => out.push_str(&inner),
                }
            } else {
                let _ = write!(out, "[{inner}]({link})");
            }
            continue;
        }
        if let (Flavor::DocScript, Some(model)) = (flavor, &s.chart) {
            out.push_str(&crate::ai_charts::script(p.id, s.o, model));
            i += 1;
            continue;
        }
        out.push_str(&seg_md(s, &p.mark, flavor));
        i += 1;
    }
    out
}

pub(super) fn para_tag(p: &RPara, extra: &str) -> String {
    let mut tag = format!("p{}", p.id);
    if let Some(s) = &p.style {
        if s != "Normal" {
            let _ = write!(tag, " {s}");
        }
    }
    if let Some(l) = &p.list {
        let _ = write!(tag, " list:{:?}", list_label(l));
        if l.level > 0 {
            let _ = write!(tag, " lvl{}", l.level);
        }
    }
    if let Some(a) = &p.fmt.align {
        if !matches!(a.as_str(), "left" | "start") {
            let _ = write!(tag, " align:{}", if a == "both" { "justify" } else { a });
        }
    }
    if let Some(k) = &p.sect_break {
        let _ = write!(tag, " section_break={}", snake_case(k));
    }
    // Hidden bookmarks (`_Toc...`) belong to Word and the table of contents.
    let marks: Vec<&str> = p.bookmarks.iter().map(String::as_str).filter(|n| !crate::marks::is_hidden(n)).collect();
    if !marks.is_empty() {
        let _ = write!(tag, " bookmarks={}", marks.join(","));
    }
    if p.hr {
        tag.push_str(" hr");
    }
    tag.push_str(extra);
    tag
}

/// `nextPage` -> `next_page`.
pub(super) fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

pub(super) fn list_label(l: &ListLabel) -> String {
    // Checkboxes show their state (☐ or ☑).
    if l.bullet && !l.checklist {
        "•".into()
    } else {
        l.text.clone()
    }
}

pub(super) fn block_script(b: &RBlock, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match b {
        RBlock::Para(p) => {
            let _ = writeln!(out, "{pad}[{}] {}", para_tag(p, ""), para_md(p));
            shape_blocks(p, indent, out);
        }
        RBlock::Table(t) => {
            let cols = t.rows.iter().map(|r| r.cells.iter().map(|c| c.span).sum::<i64>()).max().unwrap_or(0);
            let _ = writeln!(out, "{pad}[t{} table {}x{}]", t.id, t.rows.len(), cols);
            for (ri, r) in t.rows.iter().enumerate() {
                let mut col = 1;
                for c in &r.cells {
                    if c.vmerge.as_deref() == Some("continue") {
                        col += c.span;
                        continue;
                    }
                    for cb in &c.blocks {
                        match cb {
                            RBlock::Para(p) => {
                                let span = if c.span > 1 { format!(" span{}", c.span) } else { String::new() };
                                let extra = format!(" r{}c{}{}", ri + 1, col, span);
                                let _ = writeln!(out, "{pad}  [{}] {}", para_tag(p, &extra), para_md(p));
                                shape_blocks(p, indent + 1, out);
                            }
                            other => block_script(other, indent + 2, out),
                        }
                    }
                    col += c.span;
                }
            }
            let _ = writeln!(out, "{pad}[/t{}]", t.id);
        }
        RBlock::Container { id, label, blocks, .. } => {
            let _ = writeln!(out, "{pad}[c{id} container{}]", label.as_ref().map(|l| format!(" \"{l}\"")).unwrap_or_default());
            for b in blocks {
                block_script(b, indent + 1, out);
            }
            let _ = writeln!(out, "{pad}[/c{id}]");
        }
        RBlock::Opaque { id, label } => {
            let _ = writeln!(out, "{pad}[x{id} {}]", label.as_deref().unwrap_or("unsupported"));
        }
    }
}

pub struct ScriptOpts {
    /// Start at this top-level block id.
    pub start: Option<NodeId>,
    /// Maximum top-level blocks to include.
    pub limit: usize,
}

impl Default for ScriptOpts {
    fn default() -> Self {
        ScriptOpts { start: None, limit: 400 }
    }
}

/// Render the document (or a window of it) as DocScript. Drawn a block at a time: only the text of the window is kept.
pub fn docscript(doc: &mut Document, opts: &ScriptOpts) -> String {
    // The window starts at the block with the `start` id, or at the first block when there is none such; which of the two is only
    // known at the end, so the first `limit` blocks are kept as text beside the window (a few hundred blocks' text at most).
    struct Window {
        text: String,
        next: Option<String>,
    }
    let mut head = Window { text: String::new(), next: None };
    let mut win = Window { text: String::new(), next: None };
    let mut start_idx: Option<usize> = None;
    let mut total = 0usize;
    let limit = opts.limit;
    doc.render_snapshot_by_block(&mut |_, drawn| {
        for b in drawn.iter() {
            let idx = total;
            total += 1;
            if start_idx.is_none() && opts.start == Some(block_id(b)) {
                start_idx = Some(idx);
            }
            if idx < limit {
                block_script(b, 0, &mut head.text);
            } else if idx == limit {
                head.next = Some(block_ref(block_id(b), b));
            }
            if let Some(s) = start_idx {
                if idx < s + limit {
                    block_script(b, 0, &mut win.text);
                } else if idx == s + limit {
                    win.next = Some(block_ref(block_id(b), b));
                }
            }
        }
    });
    let (start_idx, w) = match start_idx {
        Some(s) => (s, win),
        None => (0, head),
    };
    let mut out = String::new();
    let end_idx = (start_idx + limit).min(total);
    let _ = writeln!(out, "# {} blocks total; showing {}..{}", total, start_idx + 1, end_idx);
    out.push_str(&w.text);
    if end_idx < total {
        let _ = writeln!(out, "# … {} more blocks. Continue with read_document {{\"start\": \"{}\"}}", total - end_idx, w.next.unwrap_or_default());
    } else {
        stories_script(doc, &mut out);
    }
    out
}

/// The first `limit` blocks as DocScript, without rendering the rest of the document (the Library's page previews).
pub fn docscript_head(doc: &Document, limit: usize) -> String {
    let r = doc.render_head(limit);
    let mut out = String::new();
    let _ = writeln!(out, "# first {} blocks", r.blocks.len());
    for b in &r.blocks {
        block_script(b, 0, &mut out);
    }
    out
}

/// Headers, footers and footnotes, after the body.
pub(super) fn stories_script(doc: &Document, out: &mut String) {
    let ps = doc.page_setup();
    for (what, map) in [("Header", &ps.headers), ("Footer", &ps.footers)] {
        let mut keys: Vec<_> = map.keys().collect();
        keys.sort();
        for k in keys {
            let _ = writeln!(out, "# {what} ({k})");
            for b in &map[k].blocks {
                block_script(b, 0, out);
            }
        }
    }
    if let Some(w) = &ps.watermark {
        let layout = if w.layout == crate::marks::WatermarkLayout::Horizontal { "horizontal" } else { "diagonal" };
        match (&w.text, w.kind) {
            (Some(t), _) => {
                let _ = writeln!(out, "# Watermark: {t:?} ({layout}{})", w.color.as_ref().map(|c| format!(", {c}")).unwrap_or_default());
            }
            _ => {
                let _ = writeln!(out, "# Watermark: picture ({layout})");
            }
        }
    }
    if !ps.notes.is_empty() {
        let _ = writeln!(out, "# Footnotes");
        for n in &ps.notes {
            for b in &n.blocks {
                block_script(b, 0, out);
            }
        }
    }
    if !ps.endnotes.is_empty() {
        let _ = writeln!(out, "# Endnotes");
        for n in &ps.endnotes {
            for b in &n.blocks {
                block_script(b, 0, out);
            }
        }
    }
}

pub(super) fn block_ref(id: NodeId, b: &RBlock) -> String {
    let p = match b {
        RBlock::Para(_) => "p",
        RBlock::Table(_) => "t",
        RBlock::Container { .. } => "c",
        RBlock::Opaque { .. } => "x",
    };
    format!("{p}{id}")
}

/// The paragraph with this id among `blocks` (also one in a text box or shape).
pub(super) fn find_para(blocks: &[RBlock], id: NodeId) -> Option<&RPara> {
    fn in_shape(sh: &crate::shapes::RShape, id: NodeId) -> Option<&RPara> {
        sh.text.as_deref().and_then(|t| find_para(t, id)).or_else(|| sh.children.iter().find_map(|c| in_shape(c, id)))
    }
    for b in blocks {
        match b {
            RBlock::Para(p) if p.id == id => return Some(p),
            // The text of a text box or shape.
            RBlock::Para(p) => {
                if let Some(f) = p.segs.iter().filter_map(|s| s.shape.as_deref()).find_map(|sh| in_shape(sh, id)) {
                    return Some(f);
                }
            }
            RBlock::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        if let Some(p) = find_para(&c.blocks, id) {
                            return Some(p);
                        }
                    }
                }
            }
            RBlock::Container { blocks, .. } => {
                if let Some(p) = find_para(blocks, id) {
                    return Some(p);
                }
            }
            _ => {}
        }
    }
    None
}

/// The lines for one paragraph found by id.
pub(super) fn para_lines(p: &RPara) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "[{}] {}", para_tag(p, ""), para_md(p));
    shape_blocks(p, 0, &mut out);
    out
}

/// DocScript for specific paragraphs (used to echo the result of an edit). The body is drawn a block at a time, keeping the lines
/// of the paragraphs asked for and nothing else.
pub fn script_for_paragraphs(doc: &mut Document, ids: &[NodeId]) -> String {
    use std::collections::HashMap;
    // Per id: the lines of the first paragraph in the body with it, and (the fallback) the script of the first top-level block with it.
    let mut in_body: HashMap<NodeId, String> = HashMap::new();
    let mut as_block: HashMap<NodeId, String> = HashMap::new();
    doc.render_snapshot_by_block(&mut |_, drawn| {
        for id in ids {
            if !in_body.contains_key(id) {
                if let Some(p) = find_para(drawn, *id) {
                    in_body.insert(*id, para_lines(p));
                }
            }
            if !as_block.contains_key(id) {
                if let Some(b) = drawn.iter().find(|b| block_id(b) == *id) {
                    let mut s = String::new();
                    block_script(b, 0, &mut s);
                    as_block.insert(*id, s);
                }
            }
        }
    });
    let mut out = String::new();
    let ps = doc.page_setup();
    let story_blocks: Vec<&RBlock> =
        ps.headers.values().chain(ps.footers.values()).flat_map(|s| s.blocks.iter()).chain(ps.notes.iter().flat_map(|n| n.blocks.iter())).chain(ps.endnotes.iter().flat_map(|n| n.blocks.iter())).collect();
    let find_story = |id: NodeId| story_blocks.iter().find_map(|b| find_para(std::slice::from_ref(*b), id));
    for id in ids {
        if let Some(s) = in_body.get(id) {
            out.push_str(s);
        } else if let Some(p) = find_story(*id) {
            out.push_str(&para_lines(p));
        } else if let Some(s) = as_block.get(id) {
            out.push_str(s);
        }
    }
    out
}

pub(super) fn block_id(b: &RBlock) -> NodeId {
    match b {
        RBlock::Para(p) => p.id,
        RBlock::Table(t) => t.id,
        RBlock::Container { id, .. } | RBlock::Opaque { id, .. } => *id,
    }
}

pub fn outline(doc: &mut Document) -> String {
    let tabs = doc.tabs_info();
    let mut out = String::new();
    // A document with tabs says where each one starts, with its id (the `tab` op takes it) and how much text it holds.
    let mut current: Option<String> = None;
    // Drawn a block at a time: only the heading lines are kept.
    doc.render_snapshot_by_block(&mut |_, drawn| {
        for b in drawn.iter() {
            let id = block_id(b);
            if let Some(t) = tabs.tabs.iter().find(|t| t.ids.contains(&id)) {
                if current.as_deref() != Some(t.id.as_str()) {
                    current = Some(t.id.clone());
                    let c = doc.text_stats_of_tab(&t.id).map(|s| s.main).unwrap_or_default();
                    let _ = writeln!(out, "{}[tab {}] {} ({} paragraphs, {} words)", "  ".repeat(t.depth), t.id, if t.name.is_empty() { "(unnamed)" } else { &t.name }, c.paragraphs, c.words);
                }
            }
            if let RBlock::Para(p) = b {
                let lvl = p.fmt.outline_lvl.or_else(|| p.style.as_deref().and_then(|s| s.strip_prefix("Heading")).and_then(|n| n.parse::<i64>().ok()).map(|n| n - 1));
                let is_title = p.style.as_deref() == Some("Title");
                if let Some(l) = lvl.filter(|l| (0..9).contains(l)) {
                    let _ = writeln!(out, "{}[p{}] {}", "  ".repeat(l as usize), p.id, para_md(p));
                } else if is_title {
                    let _ = writeln!(out, "[p{} Title] {}", p.id, para_md(p));
                }
            }
        }
    });
    if out.is_empty() || (!tabs.tabs.is_empty() && !out.contains("[p")) {
        out.push_str("(no headings)\n");
    }
    out
}

/// One-paragraph summary of the document for the model's context.
pub fn summary(doc: &mut Document) -> String {
    let mut paras = 0;
    let mut words = 0;
    walk_paragraphs(&doc.body, &mut |p| {
        paras += 1;
        words += p.plain_text().split_whitespace().count();
    });
    let tables = doc.body.iter().filter(|b| matches!(b, Block::Table(_))).count();
    let page = doc.page_setup();
    let mut out = format!(
        "{paras} paragraphs, ~{words} words (~{} pages), {tables} top-level tables. Page {:.1}x{:.1}in.",
        (words / 450).max(1),
        page.width as f64 / 1440.0,
        page.height as f64 / 1440.0
    );
    if page.sections.len() > 1 {
        let _ = write!(out, " {} sections.", page.sections.len());
    }
    let cols = page.sections.iter().map(|s| s.cols.num).max().unwrap_or(1);
    if cols > 1 {
        let _ = write!(out, " Up to {cols} columns.");
    }
    out
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

/// Text of the current selection as DocScript, for giving models context about what the user means by "this".
pub fn selection_context(doc: &mut Document, from: Pos, to: Pos) -> String {
    let order = paragraph_order(&doc.body);
    let a = order.iter().position(|x| *x == from.p.0);
    let b = order.iter().position(|x| *x == to.p.0);
    let (Some(a), Some(b)) = (a, b) else { return String::new() };
    let (a, b) = (a.min(b), a.max(b));
    let ids: Vec<NodeId> = order[a..=b.min(a + 60)].to_vec();
    let text = doc.text_range(from, to);
    let mut out = String::new();
    if !text.is_empty() {
        let _ = writeln!(out, "Selected text: {text:?}");
    }
    let _ = write!(out, "Selected paragraph(s):\n{}", script_for_paragraphs(doc, &ids));
    out
}
