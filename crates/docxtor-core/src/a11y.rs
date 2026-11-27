//! The accessibility check: problems that make a document hard to use with a screen reader or low
//! vision, found from the document alone. Every issue has a stable `kind`; the wording of `message`
//! depends only on the kind, and `detail` carries the variable part (the link text, the colour pair).

use crate::document::Document;
use crate::marks;
use crate::model::*;
use crate::render::{RBlock, RPara};
use crate::stories::StoryKind;
use crate::xml::Element;
use serde::Serialize;
use std::collections::HashMap;

/// A document with more words than this and no headings is hard to navigate (about three pages).
const LONG_WORDS: usize = 1500;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct A11yIssue {
    /// Stable key, in snake_case: `missing_alt`, `heading_skip`, `no_headings`, `empty_heading`, `no_title`,
    /// `table_header`, `merged_cells`, `link_text`, `contrast`, `fake_list`, `blank_lines` or `language`.
    pub kind: &'static str,
    /// `error`, `warning` or `tip`.
    pub severity: &'static str,
    /// The paragraph to go to (for a table its first paragraph, for an image the paragraph holding it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<NodeId>,
    /// Offset (UTF-16) of the image, link or text inside the paragraph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub o: Option<usize>,
    /// What is wrong and how to fix it; depends only on `kind`.
    pub message: String,
    /// The variable part: the link text, the colours and their ratio, a count, the heading levels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The fixed message of each issue kind.
pub fn message_for(kind: &str) -> &'static str {
    match kind {
        "missing_alt" => "This image has no alt text. Describe it for people who can't see it, or mark it as decorative.",
        "heading_skip" => "A heading level is skipped. Use the next level down so the outline stays in order.",
        "no_headings" => "This long document has no headings. Add headings so readers can find their way around.",
        "empty_heading" => "This heading has no text. Add text or change it to a normal paragraph.",
        "no_title" => "The document has no title. Add one in the document properties.",
        "table_header" => "This table has no header row. Mark the first row as a header so it repeats on every page and screen readers announce it.",
        "merged_cells" => "This table has merged cells, which screen readers handle poorly. Split them if you can.",
        "link_text" => "This link doesn't say where it goes. Use words that describe the destination.",
        "contrast" => "This text is hard to read against its background. Use a darker or lighter colour.",
        "fake_list" => "This looks like a typed list. Use real bullets or numbering instead.",
        "blank_lines" => "Blank lines are used for spacing. Use paragraph spacing or a page break instead.",
        "language" => "No document language is set. Set one so screen readers pronounce the text correctly.",
        _ => "",
    }
}

fn severity_of(kind: &str) -> &'static str {
    match kind {
        "missing_alt" | "contrast" => "error",
        "heading_skip" | "empty_heading" | "table_header" | "merged_cells" | "link_text" => "warning",
        _ => "tip",
    }
}

fn issue(kind: &'static str, p: Option<NodeId>, o: Option<usize>, detail: Option<String>) -> A11yIssue {
    A11yIssue { kind, severity: severity_of(kind), p, o, message: message_for(kind).to_string(), detail }
}

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

/// Parse six hex digits (with or without `#`) into red, green and blue.
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((v(0)?, v(2)?, v(4)?))
}

/// WCAG relative luminance of an sRGB colour.
pub fn luminance((r, g, b): (u8, u8, u8)) -> f64 {
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio between two colours, from 1 to 21.
pub fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn highlight_hex(name: &str) -> Option<&'static str> {
    Some(match name {
        "yellow" => "FFFF00",
        "green" => "00FF00",
        "cyan" => "00FFFF",
        "magenta" => "FF00FF",
        "blue" => "0000FF",
        "red" => "FF0000",
        "darkBlue" => "000080",
        "darkCyan" => "008080",
        "darkGreen" => "008000",
        "darkMagenta" => "800080",
        "darkRed" => "800000",
        "darkYellow" => "808000",
        "darkGray" => "808080",
        "lightGray" => "C0C0C0",
        "black" => "000000",
        "white" => "FFFFFF",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// What surrounds a paragraph: the fill behind it and whether the background is unknown.
#[derive(Clone, Default)]
struct Cx {
    shade: Option<String>,
    /// A table style that shades cells (banded rows, a coloured header) sits behind this text and is not resolved.
    unknown_bg: bool,
}

struct Checker<'a> {
    doc: &'a Document,
    rparas: HashMap<NodeId, RPara>,
    page_bg: Option<String>,
    issues: Vec<A11yIssue>,
    prev_heading: Option<i64>,
    headings: usize,
    words: usize,
}

/// Runs of blank paragraphs and of typed-list paragraphs in the list of blocks being checked.
#[derive(Default)]
struct Runs {
    blank: Vec<NodeId>,
    fake: Vec<NodeId>,
}

enum Found {
    Image { o: usize, ok: bool },
    Link { o: usize, text: String },
}

fn collect_rparas(blocks: &[RBlock], out: &mut HashMap<NodeId, RPara>) {
    for b in blocks {
        match b {
            RBlock::Para(p) => {
                out.insert(p.id, p.clone());
            }
            RBlock::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        collect_rparas(&c.blocks, out);
                    }
                }
            }
            RBlock::Container { blocks, .. } => collect_rparas(blocks, out),
            RBlock::Opaque { .. } => {}
        }
    }
}

/// Whether a drawing or picture counts as described: it has alt text, or the file marks it decorative.
fn image_described(e: &Element) -> bool {
    let non_empty = |v: Option<&str>| v.is_some_and(|s| !s.trim().is_empty());
    if let Some(doc_pr) = e.find("wp:docPr") {
        // The description lives on the drawing's `docPr`; some producers set it on the picture's `cNvPr` only.
        if non_empty(doc_pr.attr("descr")) || e.find("pic:cNvPr").is_some_and(|c| non_empty(c.attr("descr"))) {
            return true;
        }
        // Marked decorative (`adec:decorative` in an extension list of the drawing).
        let mut all = Vec::new();
        collect_all(e, &mut all);
        return all.iter().any(|d| d.local() == "decorative" && !matches!(d.attr("val"), Some("0" | "false")));
    }
    // VML pictures: `alt` on the shape.
    let mut shapes = Vec::new();
    e.find_all("v:shape", &mut shapes);
    shapes.iter().any(|s| non_empty(s.attr("alt")))
}

fn collect_all<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
    for c in e.elements() {
        out.push(c);
        collect_all(c, out);
    }
}

fn is_picture(e: &Element) -> bool {
    e.name != "w:object" && !marks::is_watermark_pict(e) && (e.find("a:blip").is_some() || e.find("v:imagedata").is_some() || crate::chart::is_chart_drawing(e))
}

fn scan(inlines: &[Inline], off: &mut usize, out: &mut Vec<Found>) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                for item in &r.items {
                    if let RunItem::Object(e) = item {
                        if is_picture(e) {
                            out.push(Found::Image { o: *off, ok: image_described(e) });
                        }
                    }
                    *off += item.len16();
                }
            }
            Inline::Container(c) => {
                if c.kind == InlineKind::Hyperlink {
                    out.push(Found::Link { o: *off, text: c.children.iter().map(|i| i.text()).collect() });
                }
                scan(&c.children, off, out);
            }
            Inline::Opaque(_) => *off += i.len16(),
        }
    }
}

fn is_vague_link(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    if ["http://", "https://", "ftp://", "www.", "mailto:"].iter().any(|p| t.starts_with(p)) {
        return true;
    }
    let core = t.trim_matches(|c: char| !c.is_alphanumeric());
    matches!(core, "click here" | "here" | "link" | "read more" | "more")
}

/// The level of a heading (1 is the highest), or None for other paragraphs.
fn heading_level(doc: &Document, rp: &RPara) -> Option<i64> {
    if let Some(l) = rp.fmt.outline_lvl {
        return (0..9).contains(&l).then_some(l + 1);
    }
    let id = rp.style.as_deref()?;
    let name = doc.styles.get(id).map(|s| s.name.to_lowercase()).unwrap_or_else(|| id.to_lowercase());
    let n = name.strip_prefix("heading")?.trim().parse::<i64>().ok()?;
    (1..=9).contains(&n).then_some(n)
}

fn style_has_shading(doc: &Document, style_id: &str) -> bool {
    let Some(sd) = &doc.styles.doc else { return false };
    let mut cur = Some(style_id.to_string());
    for _ in 0..12 {
        let Some(id) = cur else { break };
        let Some(st) = sd.root.children_named("w:style").find(|s| s.attr("w:styleId") == Some(id.as_str())) else { break };
        if st.find("w:shd").is_some() {
            return true;
        }
        cur = st.child("w:basedOn").and_then(|b| b.attr("w:val")).map(String::from);
    }
    false
}

fn fill_of(e: Option<&Element>) -> Option<String> {
    e.and_then(|e| e.child("w:shd")).and_then(|s| s.attr("w:fill")).and_then(crate::colour::hex)
}

fn first_para(blocks: &[Block]) -> Option<NodeId> {
    let mut id = None;
    walk_paragraphs(blocks, &mut |p| {
        if id.is_none() {
            id = Some(p.id);
        }
    });
    id
}

fn table_first_para(t: &Table) -> Option<NodeId> {
    t.rows().flat_map(|r| r.cells()).find_map(|c| first_para(&c.blocks))
}

impl<'a> Checker<'a> {
    fn blocks<'b>(&mut self, blocks: impl IntoIterator<Item = &'b Block>, cx: &Cx, in_body: bool) {
        // Runs of blank paragraphs and of typed-list paragraphs inside this list of blocks.
        let mut runs = Runs::default();
        for b in blocks {
            self.step(b, cx, in_body, &mut runs);
        }
        self.end(&mut runs);
    }

    /// One block of a list of blocks (see `blocks`).
    fn step(&mut self, b: &Block, cx: &Cx, in_body: bool, runs: &mut Runs) {
        let mut is_blank = false;
        let mut is_fake = false;
        match b {
            Block::Para(p) => {
                (is_blank, is_fake) = self.para(p, cx, in_body);
            }
            Block::Table(t) => self.table(t, cx, in_body),
            Block::Container(c) => self.blocks(&c.blocks, cx, in_body),
            Block::Opaque(_) => {
                // Markers (a section's properties, bookmarks) don't end a run of paragraphs.
                return;
            }
        }
        if is_blank {
            runs.blank.push(b.id());
        } else {
            self.flush_blank(&mut runs.blank);
        }
        if is_fake {
            runs.fake.push(b.id());
        } else {
            self.flush_fake(&mut runs.fake);
        }
    }

    fn end(&mut self, runs: &mut Runs) {
        self.flush_blank(&mut runs.blank);
        self.flush_fake(&mut runs.fake);
    }

    fn flush_blank(&mut self, run: &mut Vec<NodeId>) {
        if run.len() >= 3 {
            self.issues.push(issue("blank_lines", Some(run[0]), None, Some(run.len().to_string())));
        }
        run.clear();
    }

    fn flush_fake(&mut self, run: &mut Vec<NodeId>) {
        if let Some(first) = run.first() {
            let detail = (run.len() > 1).then(|| run.len().to_string());
            self.issues.push(issue("fake_list", Some(*first), None, detail));
        }
        run.clear();
    }

    /// Check one paragraph; returns whether it is a blank line and whether it is a typed list item.
    fn para(&mut self, p: &Paragraph, cx: &Cx, in_body: bool) -> (bool, bool) {
        let Some(rp) = self.rparas.get(&p.id).cloned() else { return (false, false) };
        let text = p.plain_text();
        let trimmed = text.trim();

        // Headings.
        let mut heading = None;
        if in_body {
            self.words += text.split_whitespace().count();
            heading = heading_level(self.doc, &rp);
            if let Some(level) = heading {
                self.headings += 1;
                if trimmed.is_empty() && !p.text().contains(OBJ) {
                    self.issues.push(issue("empty_heading", Some(p.id), None, None));
                } else if let Some(prev) = self.prev_heading.filter(|prev| level > prev + 1) {
                    self.issues.push(issue("heading_skip", Some(p.id), None, Some(format!("Heading {prev} to Heading {level}"))));
                }
                self.prev_heading = Some(level);
            }
        }

        // Images and links.
        let mut found = Vec::new();
        scan(&p.inlines, &mut 0, &mut found);
        for f in found {
            match f {
                Found::Image { o, ok: false } => self.issues.push(issue("missing_alt", Some(p.id), Some(o), None)),
                Found::Link { o, text } if in_body && !text.trim().is_empty() && is_vague_link(&text) => {
                    self.issues.push(issue("link_text", Some(p.id), Some(o), Some(text.trim().to_string())));
                }
                _ => {}
            }
        }

        self.contrast(&rp, cx);

        // Typed lists and blank lines.
        let mut is_fake = false;
        let mut is_blank = false;
        if in_body && heading.is_none() && rp.list.is_none() && looks_like_list_item(trimmed) {
            is_fake = true;
        }
        if in_body && trimmed.is_empty() && !p.text().contains(OBJ) && !rp.hr && !has_break_or_section(p) {
            is_blank = true;
        }
        (is_blank, is_fake)
    }

    fn contrast(&mut self, rp: &RPara, cx: &Cx) {
        struct Group {
            o: usize,
            fg: String,
            bg: String,
            ratio: f64,
        }
        let mut group: Option<Group> = None;
        let mut out: Vec<Group> = Vec::new();
        for s in &rp.segs {
            if s.k != "t" {
                continue;
            }
            let Some(text) = s.x.as_deref() else { continue };
            if text.trim().is_empty() || s.f.vanish == Some(true) {
                continue;
            }
            let Some(fg) = s.f.color.as_deref().filter(|c| !c.eq_ignore_ascii_case("auto")).and_then(parse_hex) else {
                if let Some(g) = group.take() {
                    out.push(g);
                }
                continue;
            };
            let highlight = s.f.highlight.as_deref().and_then(highlight_hex).map(String::from);
            let explicit = highlight.clone().or_else(|| s.f.shading.clone()).or_else(|| rp.fmt.shading.clone()).or_else(|| cx.shade.clone());
            if explicit.is_none() && cx.unknown_bg {
                if let Some(g) = group.take() {
                    out.push(g);
                }
                continue;
            }
            let bg_hex = explicit.or_else(|| self.page_bg.clone()).unwrap_or_else(|| "FFFFFF".into());
            let bg = parse_hex(&bg_hex).unwrap_or((255, 255, 255));
            let ratio = contrast_ratio(fg, bg);
            let size = s.f.size.unwrap_or(11.0);
            let large = size >= 18.0 || (size >= 14.0 && s.f.bold == Some(true));
            let needed = if large { 3.0 } else { 4.5 };
            if ratio >= needed {
                if let Some(g) = group.take() {
                    out.push(g);
                }
                continue;
            }
            let fg_hex = format!("{:02X}{:02X}{:02X}", fg.0, fg.1, fg.2);
            let bg_hex = format!("{:02X}{:02X}{:02X}", bg.0, bg.1, bg.2);
            match &mut group {
                Some(g) if g.fg == fg_hex && g.bg == bg_hex => {}
                _ => {
                    if let Some(g) = group.take() {
                        out.push(g);
                    }
                    group = Some(Group { o: s.o, fg: fg_hex, bg: bg_hex, ratio });
                }
            }
        }
        out.extend(group);
        for g in out {
            // Rounded down, so a pair that fails never reads as passing.
            let shown = (g.ratio * 10.0).floor() / 10.0;
            self.issues.push(issue("contrast", Some(rp.id), Some(g.o), Some(format!("#{} on #{} ({shown:.1}:1)", g.fg, g.bg))));
        }
    }

    fn table(&mut self, t: &Table, cx: &Cx, in_body: bool) {
        let first = table_first_para(t);
        let rows: Vec<&Row> = t.rows().collect();
        if in_body && rows.len() > 1 {
            let is_header = |r: &Row| {
                r.children.iter().any(|c| matches!(c, RowChild::Other(e) if e.name == "w:trPr" && e.child("w:tblHeader").is_some_and(|h| !matches!(h.attr("w:val"), Some("0" | "false" | "off")))))
            };
            if !is_header(rows[0]) {
                self.issues.push(issue("table_header", first, None, None));
            }
        }
        let merged = rows.iter().flat_map(|r| r.cells()).any(|c| {
            c.tcpr.as_ref().is_some_and(|p| {
                p.child("w:gridSpan").and_then(|g| g.attr("w:val")).and_then(|v| v.parse::<i64>().ok()).is_some_and(|n| n > 1)
                    || p.child("w:vMerge").is_some()
                    || p.child("w:hMerge").is_some()
            })
        });
        if in_body && merged {
            self.issues.push(issue("merged_cells", first, None, None));
        }
        let tblpr = t.tblpr();
        let table_fill = fill_of(tblpr);
        let shaded_style = tblpr.and_then(|p| p.child("w:tblStyle")).and_then(|s| s.attr("w:val")).is_some_and(|id| style_has_shading(self.doc, id));
        for r in rows {
            for c in r.cells() {
                let cell_fill = fill_of(c.tcpr.as_ref());
                let explicit = cell_fill.clone().or_else(|| table_fill.clone());
                let inner = Cx {
                    shade: explicit.clone().or_else(|| cx.shade.clone()),
                    unknown_bg: cx.unknown_bg || (shaded_style && explicit.is_none()),
                };
                self.blocks(&c.blocks, &inner, in_body);
            }
        }
    }
}

/// Text that starts like a typed list item: a dash, bullet or star, or a number with a dot or bracket.
fn looks_like_list_item(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some('-' | '\u{2022}' | '*' | '\u{2013}') => chars.next().is_some_and(char::is_whitespace) && !chars.as_str().trim().is_empty(),
        Some(c) if c.is_ascii_digit() => {
            let rest = text.trim_start_matches(|c: char| c.is_ascii_digit());
            let digits = text.len() - rest.len();
            let mut r = rest.chars();
            digits <= 2 && matches!(r.next(), Some('.' | ')')) && r.next().is_some_and(char::is_whitespace) && !r.as_str().trim().is_empty()
        }
        _ => false,
    }
}

/// A page, column or section break makes a paragraph more than a blank line.
fn has_break_or_section(p: &Paragraph) -> bool {
    fn runs_break(inlines: &[Inline]) -> bool {
        inlines.iter().any(|i| match i {
            Inline::Run(r) => r.items.iter().any(|it| matches!(it, RunItem::Break(e) if matches!(e.attr("w:type"), Some("page" | "column")))),
            Inline::Container(c) => runs_break(&c.children),
            Inline::Opaque(_) => false,
        })
    }
    p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some() || pp.child("w:pBdr").is_some()) || runs_break(&p.inlines)
}

impl Document {
    /// Accessibility problems in the document, in the order they appear in the text, followed by the ones
    /// about the document as a whole (`no_title`, `language`, `no_headings`). Headers and footers are checked for
    /// missing alt text and low contrast.
    pub fn accessibility(&self) -> Vec<A11yIssue> {
        let mut c = Checker { doc: self, rparas: HashMap::new(), page_bg: self.page_background(), issues: Vec::new(), prev_heading: None, headings: 0, words: 0 };
        // The body is drawn a block at a time and each block checked with its drawing, which is dropped after: the drawing of a
        // whole long document is several kilobytes a paragraph (War and Peace grew the browser's engine by 100 MB for it).
        let mut runs = Runs::default();
        self.render_snapshot_by_block(&mut |b, drawn| {
            collect_rparas(drawn, &mut c.rparas);
            c.step(b, &Cx::default(), true, &mut runs);
            c.rparas.clear();
        });
        c.end(&mut runs);
        let labels = HashMap::new();
        for s in self.stories.iter().filter(|s| matches!(s.kind, StoryKind::Header | StoryKind::Footer)) {
            let mut ctx = self.render_ctx(&s.part, &labels);
            collect_rparas(&ctx.blocks(&s.blocks), &mut c.rparas);
            c.blocks(&s.blocks, &Cx::default(), false);
            c.rparas.clear();
        }
        if self.doc_props().title.is_none() {
            c.issues.push(issue("no_title", None, None, None));
        }
        if self.language().is_none_or(|l| l.trim().is_empty()) {
            c.issues.push(issue("language", None, None, None));
        }
        if c.headings == 0 && c.words > LONG_WORDS {
            c.issues.push(issue("no_headings", None, None, Some(format!("{} words", c.words))));
        }
        c.issues
    }

    /// The accessibility check as text for people and models: one line per issue with the paragraph id.
    pub fn accessibility_report(&self) -> String {
        let issues = self.accessibility();
        if issues.is_empty() {
            return "No accessibility problems found.".into();
        }
        let count = |s: &str| issues.iter().filter(|i| i.severity == s).count();
        let (e, w, t) = (count("error"), count("warning"), count("tip"));
        let mut out = format!("{} accessibility issue(s): {e} error(s), {w} warning(s), {t} tip(s).\n", issues.len());
        for i in &issues {
            let at = match (i.p, i.o) {
                (Some(p), Some(o)) => format!(" [p{p} at {o}]"),
                (Some(p), None) => format!(" [p{p}]"),
                _ => String::new(),
            };
            let detail = i.detail.as_ref().map(|d| format!(" ({d})")).unwrap_or_default();
            out.push_str(&format!("- {} {}{at}: {}{detail}\n", i.severity, i.kind, i.message));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_math() {
        let white = (255, 255, 255);
        let r = |h: &str| contrast_ratio(parse_hex(h).unwrap(), white);
        assert!((r("000000") - 21.0).abs() < 1e-9);
        assert!((r("777777") - 4.478).abs() < 0.01);
        assert!(r("777777") < 4.5 && r("767676") >= 4.5);
        assert!(looks_like_list_item("- one") && looks_like_list_item("1. one") && looks_like_list_item("2) two"));
        assert!(!looks_like_list_item("-one") && !looks_like_list_item("2023. A year") && !looks_like_list_item("- "));
    }
}
