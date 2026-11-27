//! HTML tree to Word blocks: paragraphs, runs, lists, links, pictures and equations. Tables are in `table.rs`.

use super::css::{self, get, Decls, Sheet};
use super::dom::{HEl, HNode};
use crate::fragment::{self, b64_decode, Builder, LevelSpec};
use crate::props::{put_ppr, put_rpr};
use crate::xml::Element;
use std::collections::HashMap;

pub(super) const BLOCK_TAGS: &[&str] = &[
    "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "table", "blockquote", "pre", "hr", "section", "article", "header", "footer", "nav", "aside", "main", "form", "fieldset", "figure", "figcaption", "address", "dl", "dt", "dd", "center",
    "details", "summary", "body", "html", "thead", "tbody", "tfoot", "tr", "td", "th", "caption", "colgroup", "col",
];

/// Elements whose content is never shown.
const SKIPPED: &[&str] = &["script", "style", "head", "title", "meta", "link", "noscript", "template", "iframe", "canvas", "svg", "video", "audio", "select", "option", "button", "input", "textarea", "o:p", "map", "object", "embed", "applet", "base"];

pub(super) fn is_block(e: &HEl) -> bool {
    BLOCK_TAGS.contains(&e.tag.as_str())
}

/// Character formatting that flows down the tree.
#[derive(Clone, Default, PartialEq, Debug)]
pub(super) struct Run {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: bool,
    pub strike: bool,
    pub vert: Option<&'static str>,
    pub color: Option<String>,
    pub font: Option<String>,
    pub size: Option<f64>,
    pub highlight: Option<&'static str>,
    pub shade: Option<String>,
    pub caps: bool,
    pub small_caps: bool,
    pub spacing: Option<i64>,
    /// Whitespace is kept as written.
    pub pre: bool,
    /// Inside a link: the Hyperlink style supplies the colour and underline.
    pub link: bool,
    /// Inside a heading: what the text doesn't set comes from the heading style.
    pub heading: bool,
}

/// Paragraph-level context that flows down the tree.
#[derive(Clone, Default)]
pub(super) struct Ctx {
    pub align: Option<&'static str>,
    pub indent: i64,
    pub indent_right: i64,
    pub lists: Vec<ListCtx>,
    pub bidi: bool,
}

#[derive(Clone)]
pub(super) struct ListCtx {
    pub num_id: i64,
    pub level: usize,
}

#[derive(Debug)]
pub(super) enum Seg {
    Text(Run, String),
    Raw(Element),
    Link(String, Vec<Seg>),
}

#[derive(Default)]
pub(super) struct Buf {
    /// Inline content on its own (a few words copied from a page): the spaces at its ends are kept.
    pub keep_edges: bool,
    pub segs: Vec<Seg>,
    /// The last thing written was a space (or the paragraph just started).
    pub ws: bool,
    pub any: bool,
}

impl Buf {
    pub fn new() -> Buf {
        Buf { ws: true, ..Default::default() }
    }

    pub fn edged() -> Buf {
        Buf { keep_edges: true, ..Default::default() }
    }

    fn add_text(&mut self, rc: &Run, s: String) {
        if let Some(Seg::Text(r, t)) = self.segs.last_mut() {
            if r == rc {
                t.push_str(&s);
                return;
            }
        }
        self.segs.push(Seg::Text(rc.clone(), s));
    }
}

pub(super) struct Conv<'a> {
    pub b: Builder,
    pub sheet: Sheet,
    pub images: &'a HashMap<String, Vec<u8>>,
    /// Lists Word wrote as `mso-list: l0 level1 lfo1`, by `l0:lfo1`.
    mso_nums: HashMap<String, i64>,
    /// The checklists made so far, unchecked and checked.
    checklists: [Option<i64>; 2],
    /// The list item the next paragraph belongs to.
    pub(super) pending: Option<(i64, usize)>,
    pub saw_block: bool,
    /// The content has no blocks: it is inline text, and its edge spaces count.
    pub inline_only: bool,
    depth: usize,
}

fn ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}')
}

fn generic_font(f: &str) -> bool {
    matches!(f.to_ascii_lowercase().as_str(), "serif" | "sans-serif" | "cursive" | "fantasy" | "system-ui" | "ui-sans-serif" | "ui-serif" | "ui-monospace" | "-apple-system" | "blinkmacsystemfont" | "inherit" | "initial" | "unset" | "emoji" | "math")
}

/// The first named family in a `font-family` list (`monospace` alone becomes Consolas).
fn pick_font(v: &str) -> Option<String> {
    let mut mono = false;
    for part in v.split(',') {
        let f = part.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        if f.is_empty() {
            continue;
        }
        if f.eq_ignore_ascii_case("monospace") {
            mono = true;
            continue;
        }
        if !generic_font(f) {
            return Some(f.to_string());
        }
    }
    mono.then(|| "Consolas".to_string())
}

fn keyword_size(v: &str, parent: f64) -> Option<f64> {
    Some(match v {
        "xx-small" => 7.5,
        "x-small" => 7.5,
        "small" => 10.0,
        "medium" => 12.0,
        "large" => 13.5,
        "x-large" => 18.0,
        "xx-large" => 24.0,
        "xxx-large" => 36.0,
        "smaller" => parent / 1.2,
        "larger" => parent * 1.2,
        _ => return None,
    })
}

impl<'a> Conv<'a> {
    pub fn new(sheet: Sheet, images: &'a HashMap<String, Vec<u8>>) -> Conv<'a> {
        Conv { b: Builder::new(), sheet, images, mso_nums: HashMap::new(), checklists: [None; 2], pending: None, saw_block: false, inline_only: false, depth: 0 }
    }

    pub fn finish(mut self, out: Vec<Element>) -> crate::fragment::Fragment {
        let mut out = out;
        // A last empty paragraph is the browser's trailing newline, not content.
        let empty = |p: &Element| p.name == "w:p" && p.children.iter().all(|c| matches!(c, crate::xml::Node::Element(e) if e.name == "w:pPr"));
        if out.len() > 1 && out.last().is_some_and(empty) && out[out.len() - 2].name != "w:tbl" {
            out.pop();
        }
        for e in out {
            self.b.block(e);
        }
        self.b.set_block(self.saw_block);
        self.b.finish()
    }

    // -- block level ---------------------------------------------------------------------------

    pub fn blocks(&mut self, nodes: &[HNode], cx: &Ctx, rc: &Run, out: &mut Vec<Element>) {
        if self.depth > 120 {
            return;
        }
        self.depth += 1;
        let mut buf: Option<Buf> = None;
        for n in nodes {
            match n {
                HNode::Text(t) => {
                    if buf.is_none() && !rc.pre && t.chars().all(ws) && !self.inline_only {
                        continue;
                    }
                    let fresh = if self.inline_only { Buf::edged } else { Buf::new };
                    self.text(buf.get_or_insert_with(fresh), t, rc);
                }
                HNode::El(e) => {
                    let cs = self.sheet.computed(e);
                    if self.hidden(e, &cs) {
                        continue;
                    }
                    if is_block(e) || (!is_block(e) && e.any(&is_block) && !matches!(e.tag.as_str(), "a")) {
                        self.flush(&mut buf, cx, rc, out, false);
                        if is_block(e) {
                            self.block_el(e, &cs, cx, rc, out);
                        } else {
                            // An inline wrapper around blocks (Google Docs wraps its paragraphs in <b>): pass its style down.
                            let rc2 = self.run_from(e, &cs, rc);
                            self.blocks(&e.children, cx, &rc2, out);
                        }
                    } else {
                        let fresh = if self.inline_only { Buf::edged } else { Buf::new };
                        self.inline(e, &cs, rc, buf.get_or_insert_with(fresh));
                    }
                }
            }
        }
        self.flush(&mut buf, cx, rc, out, false);
        self.depth -= 1;
    }

    fn hidden(&self, e: &HEl, cs: &Decls) -> bool {
        if SKIPPED.contains(&e.tag.as_str()) || e.tag.starts_with("v:") || e.tag.starts_with("w:") || e.tag.starts_with("xml") {
            return true;
        }
        if get(cs, "display") == Some("none") || get(cs, "mso-hide").is_some_and(|v| v == "all") || e.attr("hidden").is_some() {
            return true;
        }
        if get(cs, "mso-element").is_some_and(|v| v.starts_with("comment")) {
            return true;
        }
        e.classes().any(|c| c == "Apple-interchange-newline") && e.tag == "br"
    }

    /// Turn the loose inline content gathered so far into a paragraph.
    fn flush(&mut self, buf: &mut Option<Buf>, cx: &Ctx, rc: &Run, out: &mut Vec<Element>, keep_empty: bool) {
        let Some(b) = buf.take() else { return };
        if !b.any && !keep_empty {
            return;
        }
        let p = self.paragraph(b, &Decls::new(), None, cx, rc);
        out.push(p);
    }

    pub(super) fn block_el(&mut self, e: &HEl, cs: &Decls, cx: &Ctx, rc: &Run, out: &mut Vec<Element>) {
        self.saw_block = true;
        let tag = e.tag.as_str();
        let rc2 = self.run_from(e, cs, rc);
        match tag {
            "ul" | "ol" => self.list(e, cs, cx, &rc2, out),
            "li" => self.list_item(e, cs, cx, &rc2, out),
            "table" => self.table(e, cs, cx, &rc2, out),
            "hr" => {
                let ppr = Element::new("w:pPr").with_child(Element::new("w:pBdr").with_child(Element::new("w:bottom").with_attr("w:val", "single").with_attr("w:sz", "6").with_attr("w:space", "1").with_attr("w:color", "A6A6A6")));
                out.push(Element::new("w:p").with_child(ppr));
            }
            "pre" => self.pre(e, cs, cx, &rc2, out),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let mut rc3 = rc2;
                rc3.heading = true;
                let mut buf = Buf::new();
                for c in &e.children {
                    self.inline_node(c, &rc3, &mut buf);
                }
                let mut p = self.paragraph(buf, cs, Some(e), cx, &rc3);
                let level = &tag[1..];
                if let Some(id) = self.b.builtin_style(&format!("Heading {level}")) {
                    set_style(&mut p, &id);
                }
                out.push(p);
            }
            "blockquote" => {
                let mut cx2 = cx.clone();
                cx2.indent += 720;
                cx2.indent_right += 720;
                self.blocks(&e.children, &cx2, &rc2, out);
            }
            "center" => {
                let mut cx2 = cx.clone();
                cx2.align = Some("center");
                self.blocks(&e.children, &cx2, &rc2, out);
            }
            "p" | "div" | "dt" | "dd" | "figcaption" | "summary" | "address" | "caption" | "section" | "article" | "header" | "footer" | "nav" | "aside" | "main" | "form" | "fieldset" | "figure" | "dl" | "details" | "body" | "html" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "colgroup" | "col" => {
                let has_blocks = e.any(&is_block);
                let container = !matches!(tag, "p" | "dt" | "dd" | "figcaption" | "summary" | "address" | "caption") && has_blocks
                    || matches!(tag, "section" | "article" | "header" | "footer" | "nav" | "aside" | "main" | "form" | "fieldset" | "figure" | "dl" | "details" | "body" | "html" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "colgroup" | "col");
                if container || has_blocks {
                    let mut cx2 = cx.clone();
                    self.container_ctx(e, cs, &mut cx2);
                    self.blocks(&e.children, &cx2, &rc2, out);
                } else {
                    let mut buf = Buf::new();
                    for c in &e.children {
                        self.inline_node(c, &rc2, &mut buf);
                    }
                    if !buf.any && tag != "div" && tag != "p" {
                        return;
                    }
                    if !buf.any && e.children.is_empty() {
                        return;
                    }
                    let mut p = self.paragraph(buf, cs, Some(e), cx, &rc2);
                    match e.classes().find(|c| matches!(*c, "MsoTitle" | "MsoSubtitle")) {
                        Some("MsoTitle") => {
                            if let Some(id) = self.b.builtin_style("Title") {
                                set_style(&mut p, &id);
                            }
                        }
                        Some("MsoSubtitle") => {
                            if let Some(id) = self.b.builtin_style("Subtitle") {
                                set_style(&mut p, &id);
                            }
                        }
                        _ => {}
                    }
                    out.push(p);
                }
            }
            _ => self.blocks(&e.children, cx, &rc2, out),
        }
    }

    /// What a container passes down to the blocks inside it: alignment and the room its margins take.
    fn container_ctx(&self, e: &HEl, cs: &Decls, cx: &mut Ctx) {
        if let Some(a) = align_of(e, cs) {
            cx.align = Some(a);
        }
        let font = 11.0;
        if let Some(v) = css::side(cs, "margin", 3).and_then(|v| css::length_twips(&v, font, None)) {
            cx.indent += v.max(0);
        }
        if let Some(v) = css::side(cs, "padding", 3).and_then(|v| css::length_twips(&v, font, None)) {
            cx.indent += v.max(0);
        }
        if let Some(v) = css::side(cs, "margin", 1).and_then(|v| css::length_twips(&v, font, None)) {
            cx.indent_right += v.max(0);
        }
        if e.attr("dir") == Some("rtl") || get(cs, "direction") == Some("rtl") {
            cx.bidi = true;
        }
    }

    fn pre(&mut self, e: &HEl, cs: &Decls, cx: &Ctx, rc: &Run, out: &mut Vec<Element>) {
        let mut rc = rc.clone();
        rc.pre = true;
        rc.font = Some("Consolas".into());
        let text = e.text();
        let text = text.strip_prefix('\n').unwrap_or(&text).trim_end_matches('\n');
        let code = self.b.builtin_style("Code");
        for line in text.split('\n') {
            let mut buf = Buf::new();
            buf.any = true;
            buf.add_text(&rc, line.trim_end_matches('\r').to_string());
            let mut p = self.paragraph(buf, cs, Some(e), cx, &rc);
            if let Some(id) = &code {
                set_style(&mut p, id);
            }
            // The Code style sets the font and size.
            out.push(p);
        }
    }

    // -- paragraphs ----------------------------------------------------------------------------

    /// Build a `w:p` from gathered inline content, with the paragraph properties the element and context give.
    fn paragraph(&mut self, mut buf: Buf, cs: &Decls, el: Option<&HEl>, cx: &Ctx, rc: &Run) -> Element {
        if !buf.keep_edges {
            trim_end(&mut buf.segs);
        }
        // A lone line break is an empty paragraph.
        if buf.segs.len() == 1 && matches!(&buf.segs[0], Seg::Raw(e) if e.child("w:br").is_some_and(|b| b.attr("w:type").is_none())) {
            buf.segs.clear();
        }
        let list = self.pending.take().or_else(|| el.and_then(|e| self.mso_list(e, cs)));
        let mut ppr = Element::new("w:pPr");
        let heading = rc.heading;
        let font = rc.size.unwrap_or(11.0);
        if let Some((num, level)) = list {
            if let Some(ls) = self.b.builtin_style("List Paragraph") {
                put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", ls));
            }
            put_ppr(&mut ppr, Element::new("w:numPr").with_child(Element::new("w:ilvl").with_attr("w:val", level.min(8).to_string())).with_child(Element::new("w:numId").with_attr("w:val", num.to_string())));
            // The bullet or number is drawn in the item's font, size and colour (Google Docs sets them on the item),
            // which Word takes from the paragraph mark.
            let mark = rpr(&Run { font: rc.font.clone(), size: rc.size, color: rc.color.clone().filter(|_| !rc.link), bold: rc.bold, italic: rc.italic, ..Run::default() });
            if !mark.children.is_empty() {
                put_ppr(&mut ppr, mark);
            }
        }
        if get(cs, "page-break-before") == Some("always") || get(cs, "break-before") == Some("page") {
            put_ppr(&mut ppr, Element::new("w:pageBreakBefore"));
        }
        // Borders and shading.
        let mut bdr = Element::new("w:pBdr");
        for (i, name) in ["w:top", "w:left", "w:bottom", "w:right"].iter().enumerate() {
            let side = [0usize, 3, 2, 1][i];
            if let Some(b) = css::side(cs, "border", side).and_then(|v| css::parse_border(&v)) {
                bdr = bdr.with_child(Element::new(*name).with_attr("w:val", b.val).with_attr("w:sz", b.sz.to_string()).with_attr("w:space", "1").with_attr("w:color", b.color));
            }
        }
        if !heading && !bdr.children.is_empty() {
            put_ppr(&mut ppr, bdr);
        }
        if !heading {
            if let Some(bg) = block_background(cs) {
                put_ppr(&mut ppr, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", bg));
            }
        }
        if cx.bidi || el.is_some_and(|e| e.attr("dir") == Some("rtl")) || get(cs, "direction") == Some("rtl") {
            put_ppr(&mut ppr, Element::new("w:bidi"));
        }
        // Spacing: only what the markup says (Google Docs and Word say it for headings and list items too; Merge
        // formatting takes it away).
        {
            let mut sp = Element::new("w:spacing");
            if let Some(v) = css::side(cs, "margin", 0).and_then(|v| css::length_twips(&v, font, None)) {
                sp.set_attr("w:before", v.max(0).to_string());
            }
            if let Some(v) = css::side(cs, "margin", 2).and_then(|v| css::length_twips(&v, font, None)) {
                sp.set_attr("w:after", v.max(0).to_string());
            }
            if let Some(lh) = get(cs, "line-height") {
                if let Some((line, rule)) = line_height(lh, font) {
                    sp.set_attr("w:line", line.to_string());
                    sp.set_attr("w:lineRule", rule);
                }
            }
            if !sp.attrs.is_empty() {
                put_ppr(&mut ppr, sp);
            }
        }
        if list.is_none() && !heading {
            let mut ind = Element::new("w:ind");
            let mut left = cx.indent;
            let mut right = cx.indent_right;
            if let Some(v) = css::side(cs, "margin", 3).and_then(|v| css::length_twips(&v, font, None)) {
                left += v.max(0);
            }
            if let Some(v) = css::side(cs, "padding", 3).and_then(|v| css::length_twips(&v, font, None)) {
                left += v.max(0);
            }
            if let Some(v) = css::side(cs, "margin", 1).and_then(|v| css::length_twips(&v, font, None)) {
                right += v.max(0);
            }
            if left > 0 {
                ind.set_attr("w:left", left.to_string());
            }
            if right > 0 {
                ind.set_attr("w:right", right.to_string());
            }
            if let Some(v) = get(cs, "text-indent").and_then(|v| css::length_twips(v, font, None)) {
                if v > 0 {
                    ind.set_attr("w:firstLine", v.to_string());
                } else if v < 0 {
                    ind.set_attr("w:hanging", (-v).to_string());
                    if left == 0 {
                        ind.set_attr("w:left", (-v).to_string());
                    }
                }
            }
            if !ind.attrs.is_empty() {
                put_ppr(&mut ppr, ind);
            }
        }
        let align = el.and_then(|e| align_of(e, cs)).or(cx.align);
        if let Some(a) = align {
            put_ppr(&mut ppr, Element::new("w:jc").with_attr("w:val", a));
        }
        let mut p = Element::new("w:p");
        if !ppr.children.is_empty() {
            p = p.with_child(ppr);
        }
        for c in segs_to_elements(&mut self.b, buf.segs) {
            p = p.with_child(c);
        }
        p
    }

    /// The list a Word paragraph belongs to (`mso-list: l0 level1 lfo1`).
    fn mso_list(&mut self, e: &HEl, cs: &Decls) -> Option<(i64, usize)> {
        let v = get(cs, "mso-list")?;
        if v == "none" || v.starts_with("Ignore") {
            return None;
        }
        let toks: Vec<&str> = v.split_whitespace().collect();
        let list = toks.iter().find(|t| t.starts_with('l') && t[1..].chars().all(|c| c.is_ascii_digit()) && t.len() > 1)?;
        let level: usize = toks.iter().find_map(|t| t.strip_prefix("level")?.parse().ok()).unwrap_or(1);
        let lfo = toks.iter().find(|t| t.starts_with("lfo")).copied().unwrap_or("");
        let key = format!("{list}:{lfo}");
        if let Some(n) = self.mso_nums.get(&key) {
            return Some((*n, level.saturating_sub(1)));
        }
        // The first paragraph tells what the markers look like when the style sheet doesn't.
        let marker = marker_text(e);
        let bullet_guess = marker.as_deref().is_some_and(|m| !m.chars().any(|c| c.is_alphanumeric()));
        let mut levels = Vec::new();
        for k in 1..=9usize {
            let d = self.sheet.lists.get(&format!("{list}:level{k}"));
            let fmt = d.and_then(|d| get(d, "mso-level-number-format"));
            let bullet = match fmt {
                Some("bullet") => true,
                Some(_) => false,
                None => bullet_guess,
            };
            let mut spec = if bullet { LevelSpec::bullet(k - 1) } else { LevelSpec::numbered(k - 1, number_format(fmt.unwrap_or("arabic"))) };
            if let Some(start) = d.and_then(|d| get(d, "mso-level-start-at")).and_then(|s| s.parse().ok()) {
                spec.start = start;
            }
            if !bullet {
                if let Some(t) = d.and_then(|d| get(d, "mso-level-text")) {
                    let t = t.trim_matches('"');
                    if t.contains('%') {
                        spec.text = t.to_string();
                    }
                }
            }
            levels.push(spec);
        }
        let num = self.b.list(&levels);
        self.mso_nums.insert(key, num);
        Some((num, level.saturating_sub(1)))
    }

    // -- lists ---------------------------------------------------------------------------------

    fn list(&mut self, e: &HEl, cs: &Decls, cx: &Ctx, rc: &Run, out: &mut Vec<Element>) {
        let ordered = e.tag == "ol";
        let level = cx.lists.len();
        let kind = get(cs, "list-style-type").or_else(|| e.attr("type"));
        let mut spec = if ordered {
            LevelSpec::numbered(level, number_format(kind.unwrap_or("decimal")))
        } else {
            let mut s = LevelSpec::bullet(level);
            match kind {
                Some("circle") => {
                    s.text = "o".into();
                    s.font = Some("Courier New".into());
                }
                Some("square") => {
                    s.text = "\u{25AA}".into();
                    s.font = Some("Arial".into());
                }
                Some("disc") => {
                    s.text = "\u{2022}".into();
                    s.font = Some("Arial".into());
                }
                _ => {}
            }
            s
        };
        if ordered {
            if let Some(start) = e.attr("start").and_then(|s| s.trim().parse::<i64>().ok()) {
                spec.start = start;
            }
        }
        if kind == Some("none") {
            // A list without markers is just indented paragraphs.
            let mut cx2 = cx.clone();
            cx2.indent += 720;
            self.blocks(&e.children, &cx2, rc, out);
            return;
        }
        // Room for the widest label at the items' size, so "10." in a 20 pt list doesn't push its text to the next
        // tab stop: the text lines up as it did where it was copied.
        let items: Vec<&HEl> = e.children.iter().filter_map(|c| match c {
            HNode::El(k) if k.tag == "li" => Some(k),
            _ => None,
        }).collect();
        if let Some(first) = items.first() {
            let size = self.run_from(first, &self.sheet.computed(first), rc).size.unwrap_or(11.0);
            let widest = if ordered { spec.text.replace(&format!("%{}", level + 1), &crate::numfmt::format(spec.start + items.len() as i64 - 1, &spec.fmt, None)) } else { spec.text.clone() };
            let width: f64 = widest.chars().map(|c| match c {
                '.' | ',' | ')' | '(' | ':' | 'i' | 'l' | 'I' | 'j' | 't' | 'f' => 0.3,
                'm' | 'M' | 'w' | 'W' => 0.85,
                _ => 0.56,
            }).sum::<f64>() + 0.3;
            let need = (width * size).ceil() as i64 * 20;
            if need > spec.hanging {
                spec.hanging = need;
                spec.left = spec.left.max(need);
            }
        }
        let mut levels: Vec<LevelSpec> = (0..level).map(|l| if ordered { LevelSpec::numbered(l, "decimal") } else { LevelSpec::bullet(l) }).collect();
        levels.push(spec);
        let num_id = self.b.list(&levels);
        let mut cx2 = cx.clone();
        cx2.lists.push(ListCtx { num_id, level });
        self.pending = None;
        self.blocks(&e.children, &cx2, rc, out);
    }

    /// The checklist pasted items use, checked or not: a box at every level, as Docxtor's own checklist.
    fn checklist(&mut self, checked: bool) -> i64 {
        if let Some(n) = self.checklists[checked as usize] {
            return n;
        }
        let glyph = if checked { "\u{2611}" } else { "\u{2610}" };
        let levels: Vec<LevelSpec> = (0..9).map(|l| LevelSpec { text: glyph.into(), font: Some("Segoe UI Symbol".into()), ..LevelSpec::bullet(l) }).collect();
        let n = self.b.list(&levels);
        self.checklists[checked as usize] = Some(n);
        n
    }

    fn list_item(&mut self, e: &HEl, cs: &Decls, cx: &Ctx, rc: &Run, out: &mut Vec<Element>) {
        // A Google Docs checklist item: a checkbox picture, then the text in a paragraph laid out inline.
        if e.attr("role") == Some("checkbox") {
            let checked = e.attr("aria-checked") == Some("true");
            let num = self.checklist(checked);
            self.pending = Some((num, cx.lists.last().map_or(0, |l| l.level)));
            let mut buf = Buf::new();
            for c in &e.children {
                match c {
                    HNode::El(k) if k.tag == "img" && (k.attr("aria-roledescription") == Some("checkbox") || matches!(k.attr("alt"), Some("checked" | "unchecked"))) => {}
                    HNode::El(k) if k.tag == "p" || k.tag == "div" => {
                        let kcs = self.sheet.computed(k);
                        let krc = self.run_from(k, &kcs, rc);
                        for g in &k.children {
                            self.inline_node(g, &krc, &mut buf);
                        }
                    }
                    _ => self.inline_node(c, rc, &mut buf),
                }
            }
            // Spacing and alignment are the inner paragraph's.
            let inner = e.children.iter().find_map(|c| match c {
                HNode::El(k) if k.tag == "p" || k.tag == "div" => Some(k),
                _ => None,
            });
            let p = match inner {
                Some(k) => {
                    let kcs = self.sheet.computed(k);
                    self.paragraph(buf, &kcs, Some(k), cx, rc)
                }
                None => self.paragraph(buf, cs, Some(e), cx, rc),
            };
            out.push(p);
            self.pending = None;
            return;
        }
        let Some(l) = cx.lists.last().cloned() else {
            // A list item with no list around it: a plain paragraph.
            let mut buf = Buf::new();
            for c in &e.children {
                self.inline_node(c, rc, &mut buf);
            }
            let p = self.paragraph(buf, cs, Some(e), cx, rc);
            out.push(p);
            return;
        };
        let has_blocks = e.children.iter().any(|c| matches!(c, HNode::El(k) if is_block(k)));
        self.pending = Some((l.num_id, l.level));
        if !has_blocks {
            let mut buf = Buf::new();
            for c in &e.children {
                self.inline_node(c, rc, &mut buf);
            }
            let p = self.paragraph(buf, cs, Some(e), cx, rc);
            out.push(p);
        } else {
            // The text before the first block is the item; the blocks after it carry on, and a nested list is one level deeper.
            self.blocks(&e.children, cx, rc, out);
        }
        self.pending = None;
    }

    // -- inline --------------------------------------------------------------------------------

    pub fn inline_node(&mut self, n: &HNode, rc: &Run, buf: &mut Buf) {
        match n {
            HNode::Text(t) => self.text(buf, t, rc),
            HNode::El(e) => {
                let cs = self.sheet.computed(e);
                if self.hidden(e, &cs) {
                    return;
                }
                self.inline(e, &cs, rc, buf);
            }
        }
    }

    pub fn text(&mut self, buf: &mut Buf, t: &str, rc: &Run) {
        if rc.pre {
            if !t.is_empty() {
                buf.any = true;
                buf.ws = false;
                buf.add_text(rc, t.replace("\r\n", "\n"));
            }
            return;
        }
        let mut s = String::with_capacity(t.len());
        for c in t.chars() {
            if ws(c) {
                if !buf.ws {
                    s.push(' ');
                    buf.ws = true;
                }
            } else {
                s.push(c);
                buf.ws = false;
            }
        }
        if s.is_empty() {
            return;
        }
        if buf.keep_edges || s.chars().any(|c| c != ' ') {
            buf.any = true;
        }
        buf.add_text(rc, s);
    }

    fn inline(&mut self, e: &HEl, cs: &Decls, rc: &Run, buf: &mut Buf) {
        if self.depth > 150 {
            return;
        }
        self.depth += 1;
        self.inline_inner(e, cs, rc, buf);
        self.depth -= 1;
    }

    fn inline_inner(&mut self, e: &HEl, cs: &Decls, rc: &Run, buf: &mut Buf) {
        match e.tag.as_str() {
            "br" => {
                let page = get(cs, "page-break-before") == Some("always") || get(cs, "break-before") == Some("page") || (e.attr("clear") == Some("all") && get(cs, "page-break-before").is_some());
                let el = Element::new("w:r").with_child(if page { Element::new("w:br").with_attr("w:type", "page") } else { Element::new("w:br") });
                buf.segs.push(Seg::Raw(el));
                buf.ws = true;
                buf.any = true;
                return;
            }
            "img" => {
                self.image(e, cs, buf);
                return;
            }
            _ => {}
        }
        // An equation copied from a Docxtor page carries its LaTeX.
        if e.classes().any(|c| c == "math") {
            if let Some(latex) = e.attr("data-latex") {
                if let Ok(omath) = crate::math::latex_to_omml(latex) {
                    let display = e.classes().any(|c| c == "display");
                    buf.segs.push(Seg::Raw(if display { crate::math::display_wrap(omath) } else { omath }));
                    buf.any = true;
                    buf.ws = false;
                    return;
                }
            }
        }
        if e.classes().any(|c| c == "lbl" || c == "pgsp") {
            return;
        }
        if get(cs, "mso-list").is_some_and(|v| v.starts_with("Ignore")) {
            return;
        }
        if let Some(n) = get(cs, "mso-tab-count").and_then(|v| v.trim().parse::<usize>().ok()) {
            for _ in 0..n.min(10) {
                buf.segs.push(Seg::Raw(Element::new("w:r").with_child(Element::new("w:tab"))));
            }
            buf.ws = true;
            buf.any = true;
            return;
        }
        let rc2 = self.run_from(e, cs, rc);
        if e.tag == "a" {
            let href = e.attr("href").map(str::trim).filter(|h| !h.is_empty() && !h.to_ascii_lowercase().starts_with("javascript:"));
            if let Some(href) = href {
                let key = if href.starts_with('#') { href.to_string() } else { self.b.link(href) };
                let outer = std::mem::take(&mut buf.segs);
                for c in &e.children {
                    self.inline_node(c, &rc2, buf);
                }
                let inner = std::mem::replace(&mut buf.segs, outer);
                if !inner.is_empty() {
                    buf.segs.push(Seg::Link(key, inner));
                }
                return;
            }
        }
        for c in &e.children {
            self.inline_node(c, &rc2, buf);
        }
    }

    /// A picture: from a `data:` address, or from the pictures the caller fetched.
    fn image(&mut self, e: &HEl, cs: &Decls, buf: &mut Buf) {
        let Some(src) = e.attr("src").map(str::trim).filter(|s| !s.is_empty()) else { return };
        let bytes = if let Some(rest) = src.strip_prefix("data:") {
            let (meta, data) = rest.split_once(',').unwrap_or(("", ""));
            if meta.contains(";base64") {
                b64_decode(data)
            } else {
                Some(percent_decode(data))
            }
        } else {
            self.images.get(src).cloned()
        };
        let Some(bytes) = bytes else {
            self.b.warn("picture-unreadable");
            return;
        };
        let Some((_, px_w, px_h)) = fragment::sniff_image(&bytes) else {
            self.b.warn("picture-unsupported");
            return;
        };
        let Some(key) = self.b.media(&bytes) else {
            self.b.warn("picture-unsupported");
            return;
        };
        let dim = |css_name: &str, attr: &str| -> Option<f64> { get(cs, css_name).and_then(|v| css::length_pt(v, 11.0, None)).or_else(|| e.attr(attr).and_then(|v| css::length_pt(v, 11.0, None))).filter(|v| *v > 0.0) };
        let (nw, nh) = (px_w as f64 * 0.75, px_h as f64 * 0.75);
        let (mut w, mut h) = match (dim("width", "width"), dim("height", "height")) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) if nw > 0.0 => (w, w * nh / nw),
            (None, Some(h)) if nh > 0.0 => (h * nw / nh, h),
            _ => (if nw > 0.0 { nw } else { 144.0 }, if nh > 0.0 { nh } else { 108.0 }),
        };
        // Keep it on the page.
        if w > 468.0 {
            h *= 468.0 / w;
            w = 468.0;
        }
        let id = self.b.drawing_id();
        let drawing = fragment::inline_drawing(&key, w, h, id, e.attr("alt").unwrap_or(""));
        buf.segs.push(Seg::Raw(Element::new("w:r").with_child(drawing)));
        buf.any = true;
        buf.ws = false;
    }

    // -- character formatting ------------------------------------------------------------------

    pub(super) fn run_from(&self, e: &HEl, cs: &Decls, rc: &Run) -> Run {
        let mut r = rc.clone();
        let block = is_block(e);
        match e.tag.as_str() {
            "b" | "strong" => r.bold = Some(true),
            "i" | "em" | "cite" | "dfn" | "var" => r.italic = Some(true),
            "u" | "ins" => r.underline = true,
            "s" | "strike" | "del" => r.strike = true,
            "sup" => r.vert = Some("superscript"),
            "sub" => r.vert = Some("subscript"),
            "code" | "kbd" | "samp" | "tt" => {
                r.font = Some("Consolas".into());
                if e.tag == "code" && !r.pre {
                    r.shade = Some("F3F4F6".into());
                }
            }
            "mark" => r.highlight = Some("yellow"),
            "small" => r.size = Some(r.size.unwrap_or(11.0) / 1.2),
            "big" => r.size = Some(r.size.unwrap_or(11.0) * 1.2),
            "a" if e.attr("href").is_some() => r.link = true,
            "th" => r.bold = Some(true),
            "font" => {
                if let Some(f) = e.attr("face").and_then(pick_font) {
                    r.font = Some(f);
                }
                if let Some(c) = e.attr("color").and_then(css::parse_color) {
                    r.color = Some(c);
                }
                if let Some(s) = e.attr("size") {
                    let n: i32 = s.trim().trim_start_matches('+').parse().unwrap_or(3);
                    let n = if s.trim().starts_with(['+', '-']) { 3 + n } else { n };
                    r.size = Some(match n.clamp(1, 7) {
                        1 => 7.5,
                        2 => 10.0,
                        3 => 12.0,
                        4 => 13.5,
                        5 => 18.0,
                        6 => 24.0,
                        _ => 36.0,
                    });
                }
            }
            _ => {}
        }
        if let Some(w) = get(cs, "font-weight") {
            r.bold = Some(matches!(w, "bold" | "bolder") || w.parse::<u32>().is_ok_and(|n| n >= 600));
        }
        if let Some(s) = get(cs, "font-style") {
            r.italic = Some(matches!(s, "italic" | "oblique"));
        }
        for prop in ["text-decoration", "text-decoration-line"] {
            if let Some(d) = get(cs, prop) {
                if d.contains("underline") {
                    r.underline = true;
                }
                if d.contains("line-through") {
                    r.strike = true;
                }
                if d.trim() == "none" && e.tag != "a" {
                    r.underline = false;
                    r.strike = false;
                }
            }
        }
        if let Some(v) = get(cs, "vertical-align") {
            match v {
                "super" | "text-top" => r.vert = Some("superscript"),
                "sub" | "text-bottom" => r.vert = Some("subscript"),
                "baseline" => r.vert = None,
                _ => {}
            }
        }
        if let Some(c) = get(cs, "color").and_then(css::parse_color) {
            r.color = Some(c);
        }
        if let Some(f) = get(cs, "font-family").and_then(pick_font) {
            r.font = Some(f);
        }
        if let Some(v) = get(cs, "font-size") {
            let parent = r.size.unwrap_or(11.0);
            let v = v.trim().to_ascii_lowercase();
            if let Some(pt) = keyword_size(&v, parent).or_else(|| css::length_pt(&v, parent, Some(parent))) {
                if pt > 0.0 {
                    r.size = Some(pt);
                }
            }
        }
        if let Some(h) = get(cs, "mso-highlight").and_then(css::mso_highlight) {
            r.highlight = Some(h);
        }
        if !block {
            if let Some(bg) = get(cs, "background-color").or_else(|| get(cs, "background")).and_then(|v| v.split_whitespace().find_map(css::parse_color)) {
                if !bg.eq_ignore_ascii_case("FFFFFF") {
                    match css::highlight_name(&bg) {
                        Some(h) if get(cs, "mso-highlight").is_none() && bg != "000000" => r.highlight = Some(h),
                        _ => r.shade = Some(bg),
                    }
                }
            }
        }
        if let Some(t) = get(cs, "text-transform") {
            r.caps = t == "uppercase";
        }
        if let Some(v) = get(cs, "font-variant") {
            r.small_caps = v.contains("small-caps");
        }
        if let Some(v) = get(cs, "letter-spacing") {
            if let Some(tw) = css::length_twips(v, r.size.unwrap_or(11.0), None) {
                r.spacing = Some(tw).filter(|t| *t != 0);
            }
        }
        if let Some(v) = get(cs, "white-space") {
            r.pre = v.starts_with("pre");
        }
        // A heading keeps the font, size, weight and colour its text has (Google Docs and Word set them on every
        // heading), as Keep source formatting promises; Merge formatting takes them away and leaves the style's.
        if r.link {
            r.color = None;
            r.underline = false;
        }
        // Black stays black, as in Word: left to the document, the styles may draw it in a colour.
        r
    }
}

fn set_style(p: &mut Element, id: &str) {
    if !p.children.iter().any(|c| matches!(c, crate::xml::Node::Element(e) if e.name == "w:pPr")) {
        p.children.insert(0, crate::xml::Node::Element(Element::new("w:pPr")));
    }
    if let Some(ppr) = p.child_mut("w:pPr") {
        put_ppr(ppr, Element::new("w:pStyle").with_attr("w:val", id));
    }
}

fn align_of(e: &HEl, cs: &Decls) -> Option<&'static str> {
    let v = get(cs, "text-align").or_else(|| e.attr("align"))?;
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "center" | "-webkit-center" | "-moz-center" => "center",
        "right" | "end" => "right",
        "justify" => "both",
        "left" | "start" => "left",
        _ => return None,
    })
}

fn block_background(cs: &Decls) -> Option<String> {
    let bg = get(cs, "background-color").or_else(|| get(cs, "background")).or_else(|| get(cs, "mso-shading"))?;
    let c = bg.split_whitespace().find_map(css::parse_color)?;
    (!c.eq_ignore_ascii_case("FFFFFF")).then_some(c)
}

/// Word's `w:spacing` line and rule for a CSS line-height.
fn line_height(v: &str, font: f64) -> Option<(i64, &'static str)> {
    let v = v.trim();
    if v == "normal" || v == "inherit" {
        return None;
    }
    // A bare number is times the font size, and a single line is about 1.2 times it: Google Docs writes its 1.15
    // as 1.38 and single spacing as 1.2. (Word writes its own multiple as a percentage.)
    if let Ok(n) = v.parse::<f64>() {
        return Some(((n / 1.2 * 240.0).round() as i64, "auto")).filter(|(l, _)| *l > 0);
    }
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse::<f64>().ok().map(|p| ((p * 2.4).round() as i64, "auto")).filter(|(l, _)| *l > 0);
    }
    css::length_twips(v, font, None).filter(|l| *l > 0).map(|l| (l, "atLeast"))
}

/// Word's number format names from CSS and `mso-level-number-format` names.
fn number_format(v: &str) -> &'static str {
    match v {
        "lower-alpha" | "lower-latin" | "alpha-lower" | "a" => "lowerLetter",
        "upper-alpha" | "upper-latin" | "alpha-upper" | "A" => "upperLetter",
        "lower-roman" | "roman-lower" | "i" => "lowerRoman",
        "upper-roman" | "roman-upper" | "I" => "upperRoman",
        "decimal-leading-zero" => "decimalZero",
        _ => "decimal",
    }
}

/// The text of the number or bullet Word wrote by hand at the start of a list paragraph.
fn marker_text(e: &HEl) -> Option<String> {
    for c in &e.children {
        if let HNode::El(k) = c {
            if k.attr("style").is_some_and(|s| s.replace(' ', "").contains("mso-list:Ignore")) {
                return Some(k.text().trim().to_string());
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn trim_end(segs: &mut Vec<Seg>) {
    // A line break at the end of a paragraph is nothing; so is a space.
    loop {
        let n = segs.len();
        match segs.last_mut() {
            Some(Seg::Raw(e)) if n > 1 && e.child("w:br").is_some_and(|b| b.attr("w:type").is_none()) => {
                segs.pop();
            }
            Some(Seg::Text(_, t)) => {
                let trimmed = t.trim_end_matches(' ').len();
                t.truncate(trimmed);
                if t.is_empty() {
                    segs.pop();
                    continue;
                }
                return;
            }
            Some(Seg::Link(_, inner)) => {
                trim_end(inner);
                if inner.is_empty() {
                    segs.pop();
                    continue;
                }
                return;
            }
            _ => return,
        }
    }
}

/// The `w:rPr` for a run's formatting.
pub(super) fn rpr(rc: &Run) -> Element {
    let mut r = Element::new("w:rPr");
    if rc.link {
        put_rpr(&mut r, Element::new("w:rStyle").with_attr("w:val", "Hyperlink"));
    }
    if let Some(f) = &rc.font {
        put_rpr(&mut r, Element::new("w:rFonts").with_attr("w:ascii", f).with_attr("w:hAnsi", f).with_attr("w:cs", f).with_attr("w:eastAsia", f));
    }
    // In a heading a plain weight or an upright style is kept too: the heading style may be bold or italic, and
    // "not bold" has to say so (elsewhere the paragraph's own style is not bold or italic and nothing is written).
    match rc.bold {
        Some(true) => {
            put_rpr(&mut r, Element::new("w:b"));
            put_rpr(&mut r, Element::new("w:bCs"));
        }
        Some(false) if rc.heading => {
            put_rpr(&mut r, Element::new("w:b").with_attr("w:val", "0"));
            put_rpr(&mut r, Element::new("w:bCs").with_attr("w:val", "0"));
        }
        _ => {}
    }
    match rc.italic {
        Some(true) => {
            put_rpr(&mut r, Element::new("w:i"));
            put_rpr(&mut r, Element::new("w:iCs"));
        }
        Some(false) if rc.heading => {
            put_rpr(&mut r, Element::new("w:i").with_attr("w:val", "0"));
            put_rpr(&mut r, Element::new("w:iCs").with_attr("w:val", "0"));
        }
        _ => {}
    }
    if rc.caps {
        put_rpr(&mut r, Element::new("w:caps"));
    }
    if rc.small_caps {
        put_rpr(&mut r, Element::new("w:smallCaps"));
    }
    if rc.strike {
        put_rpr(&mut r, Element::new("w:strike"));
    }
    if let Some(c) = &rc.color {
        put_rpr(&mut r, Element::new("w:color").with_attr("w:val", c));
    }
    if let Some(s) = rc.spacing {
        put_rpr(&mut r, Element::new("w:spacing").with_attr("w:val", s.to_string()));
    }
    if let Some(s) = rc.size {
        let hp = ((s * 2.0).round() as i64).clamp(2, 3276).to_string();
        put_rpr(&mut r, Element::new("w:sz").with_attr("w:val", &hp));
        put_rpr(&mut r, Element::new("w:szCs").with_attr("w:val", &hp));
    }
    if let Some(h) = rc.highlight {
        put_rpr(&mut r, Element::new("w:highlight").with_attr("w:val", h));
    }
    if rc.underline {
        put_rpr(&mut r, Element::new("w:u").with_attr("w:val", "single"));
    }
    if let Some(s) = &rc.shade {
        put_rpr(&mut r, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", s));
    }
    if let Some(v) = rc.vert {
        put_rpr(&mut r, Element::new("w:vertAlign").with_attr("w:val", v));
    }
    r
}

/// The children of a `w:r` for a piece of text: text, tabs and line breaks.
fn run_children(text: &str, pre: bool) -> Vec<Element> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<Element>| {
        if !cur.is_empty() {
            out.push(Element::new("w:t").with_attr("xml:space", "preserve").with_text(std::mem::take(cur)));
        }
    };
    for c in text.chars() {
        match c {
            '\t' if pre => {
                flush(&mut cur, &mut out);
                out.push(Element::new("w:tab"));
            }
            '\n' if pre => {
                flush(&mut cur, &mut out);
                out.push(Element::new("w:br"));
            }
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' => {}
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut out);
    out
}

pub(super) fn segs_to_elements(b: &mut Builder, segs: Vec<Seg>) -> Vec<Element> {
    let mut out = Vec::new();
    for s in segs {
        match s {
            Seg::Text(rc, t) => {
                let kids = run_children(&t, rc.pre);
                if kids.is_empty() {
                    continue;
                }
                let mut r = Element::new("w:r");
                let p = rpr(&rc);
                if !p.children.is_empty() {
                    r = r.with_child(p);
                }
                for k in kids {
                    r = r.with_child(k);
                }
                out.push(r);
            }
            Seg::Raw(e) => out.push(e),
            Seg::Link(key, inner) => {
                if inner.is_empty() {
                    continue;
                }
                if b.builtin_style("Hyperlink").is_none() {
                    continue;
                }
                // A link to a place in the page: the page's anchors are not made into bookmarks, so it would point at nothing in the
                // document. Its text stays, as plain text.
                if key.starts_with('#') {
                    out.extend(segs_to_elements(b, inner));
                    continue;
                }
                let mut h = Element::new("w:hyperlink");
                h.set_attr("r:id", &key);
                h.set_attr("w:history", "1");
                for c in segs_to_elements(b, inner) {
                    h = h.with_child(c);
                }
                out.push(h);
            }
        }
    }
    out
}
