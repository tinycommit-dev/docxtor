//! Content that moves between places: the Docxtor clipboard format, and what the HTML and RTF readers produce.
//!
//! A `Fragment` is a run of blocks as Word writes them (`w:p` and `w:tbl` elements, kept as XML text) plus
//! the parts they need: style definitions, list definitions, pictures and link targets. Inside the body,
//! pictures and links are named by keys (`m1`, `l1`) instead of relationship ids, and notes by their own
//! keys, so a fragment means the same in any document. `Document::insert_fragment` brings one in (mapping
//! styles by id, adding the missing definitions, making new relationships); `Document::copy_fragment`
//! makes one from a range. The HTML reader (`html_import`) and the RTF reader (`rtf`) build fragments with
//! `Builder`.

use crate::xml::{self, AttrValue, Element, Name, Node};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// The namespace prefixes fragments use, with their URIs.
pub const NAMESPACES: &[(&str, &str)] = &[
    ("w", NS_W),
    ("r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
    ("wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
    ("a", "http://schemas.openxmlformats.org/drawingml/2006/main"),
    ("pic", "http://schemas.openxmlformats.org/drawingml/2006/picture"),
    ("m", "http://schemas.openxmlformats.org/officeDocument/2006/math"),
    ("mc", "http://schemas.openxmlformats.org/markup-compatibility/2006"),
    ("v", "urn:schemas-microsoft-com:vml"),
    ("o", "urn:schemas-microsoft-com:office:office"),
    ("w10", "urn:schemas-microsoft-com:office:word"),
    ("wps", "http://schemas.microsoft.com/office/word/2010/wordprocessingShape"),
    ("wpg", "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"),
    ("wpc", "http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"),
    ("w14", "http://schemas.microsoft.com/office/word/2010/wordml"),
    ("wp14", "http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing"),
    ("c", "http://schemas.openxmlformats.org/drawingml/2006/chart"),
];

/// A picture carried by a fragment.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FragMedia {
    /// File extension without the dot (`png`, `jpg`, `gif`).
    pub ext: String,
    /// The bytes, base64.
    pub data: String,
}

/// A footnote or endnote carried by a fragment: its `w:footnote` or `w:endnote` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FragNote {
    /// `footnote` or `endnote`.
    pub kind: String,
    pub xml: String,
}

/// Something the person should hear about after a paste.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FragWarning {
    /// `picture-unreadable` (a picture the page can't reach), `picture-unsupported` (a format the editor can't draw)
    /// or `object-dropped` (a chart or embedded object that doesn't travel).
    pub kind: String,
    pub count: usize,
}

/// What the document body is made of, when the source's own defaults differ from the destination's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FragDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// Points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fragment {
    pub version: u32,
    /// Namespace prefixes the XML uses, with their URIs (the destination declares the ones it lacks).
    pub ns: Vec<(String, String)>,
    /// `w:p` and `w:tbl` elements, one after the other.
    pub body: String,
    /// `w:style` elements the body uses (and the ones they build on).
    pub styles: String,
    /// `w:abstractNum` and `w:num` elements for the lists the body uses.
    pub numbering: String,
    pub media: BTreeMap<String, FragMedia>,
    /// Link targets by key; the body's `w:hyperlink r:id` names them.
    pub links: BTreeMap<String, String>,
    pub notes: BTreeMap<String, FragNote>,
    /// The content is whole paragraphs (a heading, a list item), not a run of text inside one.
    pub block: bool,
    /// A list in the fragment may continue a list of the same definition in the destination (a copy within
    /// one document, or between documents with the same list). Readers leave this off: each pasted list counts on its own.
    pub reuse_lists: bool,
    pub defaults: Option<FragDefaults>,
    pub warnings: Vec<FragWarning>,
    /// The session nonce of the Docxtor that copied this (see `set_session_nonce`); empty for anything else.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub origin: String,
}

static SESSION_NONCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Give this engine instance a secret that only it knows (at least 16 characters, random per page load). Fragments
/// it copies carry the secret as `origin`, and a paste whose fragment has it knows Docxtor made the fragment, so it
/// may keep what a stranger's clipboard may not (links to `file:` addresses). The first call wins; returns whether
/// this one was taken. Nothing else is trusted: HTML, RTF and any other page's clipboard stay strict.
pub fn set_session_nonce(nonce: &str) -> bool {
    nonce.len() >= 16 && SESSION_NONCE.set(nonce.to_string()).is_ok()
}

pub(crate) fn session_nonce() -> Option<&'static str> {
    SESSION_NONCE.get().map(|s| s.as_str())
}

impl Fragment {
    /// Whether this session's Docxtor copied the fragment.
    pub fn is_own(&self) -> bool {
        !self.origin.is_empty() && session_nonce() == Some(self.origin.as_str())
    }
}

/// The text of one block element: its paragraphs on lines, table cells separated by tabs.
pub fn element_text(el: &Element) -> String {
    let mut lines = Vec::new();
    collect_text(el, &mut lines);
    lines.join("\n")
}

/// How a paste takes the source's formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PasteMode {
    /// Keep everything the source has.
    #[default]
    Keep,
    /// Keep bold, italic, underline, lists and links; take the destination's fonts and paragraph styles.
    Merge,
}

/// The pieces of a fragment as elements.
pub struct Parsed {
    pub body: Vec<Element>,
    pub styles: Vec<Element>,
    pub numbering: Vec<Element>,
}

fn parse_list(src: &str) -> Vec<Element> {
    if src.trim().is_empty() {
        return Vec::new();
    }
    match xml::parse(format!("<f>{src}</f>").as_bytes()) {
        Ok(d) => d.root.elements().cloned().collect(),
        Err(_) => Vec::new(),
    }
}

pub(crate) fn join_xml(els: &[Element]) -> String {
    els.iter().map(|e| e.to_xml()).collect()
}

impl Fragment {
    pub fn parse(&self) -> Parsed {
        Parsed { body: parse_list(&self.body), styles: parse_list(&self.styles), numbering: parse_list(&self.numbering) }
    }

    /// Fragment from JSON (what the clipboard carries).
    pub fn from_json(json: &str) -> Result<Fragment, String> {
        serde_json::from_str(json).map_err(|e| format!("not a fragment: {e}"))
    }

    pub fn is_empty(&self) -> bool {
        self.body.trim().is_empty()
    }

    /// The text of the fragment: paragraphs on lines, table cells separated by tabs.
    pub fn plain_text(&self) -> String {
        let mut lines = Vec::new();
        for el in parse_list(&self.body) {
            collect_text(&el, &mut lines);
        }
        lines.join("\n")
    }

    /// The fragment with only what Merge formatting keeps (see `PasteMode::Merge`).
    pub fn merged(&self) -> Fragment {
        let mut out = self.clone();
        let mut body = parse_list(&self.body);
        // A list that comes from a paragraph style stays a list when the style goes.
        let styles = parse_list(&self.styles);
        for el in body.iter_mut() {
            style_lists_to_direct(el, &styles);
        }
        merge_formatting(&mut body);
        out.body = join_xml(&body);
        out.styles = String::new();
        out.defaults = None;
        out
    }

    /// Add the content of another fragment after this one's, so a copy that crosses documents (a range over a
    /// chunk and the text around it) is one fragment. `tag` makes the other's picture, link and note keys different
    /// from this one's; its lists get numbers this one doesn't use; styles this one lacks come along.
    pub(crate) fn append(&mut self, other: Fragment, tag: &str) {
        let mut parsed = other.parse();
        let rekey = |k: &str| format!("{tag}{k}");
        // Lists: other numbers move past the highest this fragment has.
        let mine = parse_list(&self.numbering);
        let top = |els: &[Element], tag: &str, attr: &str| -> i64 { els.iter().filter(|e| e.name == tag).filter_map(|e| e.attr(attr).and_then(|v| v.parse::<i64>().ok())).max().unwrap_or(0) };
        let (num_off, abs_off) = (top(&mine, "w:num", "w:numId").max(top(&parsed.numbering, "w:num", "w:numId")) + 1, top(&mine, "w:abstractNum", "w:abstractNumId").max(top(&parsed.numbering, "w:abstractNum", "w:abstractNumId")) + 1);
        // `numId` 0 means "no list" and stays; an abstract list may be numbered 0.
        let shift = |e: &mut Element, attr: &'static str, off: i64, zero_ok: bool| {
            if let Some(n) = e.attr(attr).and_then(|v| v.parse::<i64>().ok()) {
                if n > 0 || zero_ok {
                    e.set_attr(attr, (n + off).to_string());
                }
            }
        };
        let renumber = |e: &mut Element| match e.name.as_str() {
            "w:numId" => shift(e, "w:val", num_off, false),
            "w:num" => shift(e, "w:numId", num_off, false),
            "w:abstractNumId" => shift(e, "w:val", abs_off, true),
            "w:abstractNum" => shift(e, "w:abstractNumId", abs_off, true),
            _ => {}
        };
        let (media_keys, link_keys, note_keys): (Vec<String>, Vec<String>, Vec<String>) = (other.media.keys().cloned().collect(), other.links.keys().cloned().collect(), other.notes.keys().cloned().collect());
        let keyed = |e: &mut Element| {
            let rel_attrs: Vec<(Name, AttrValue)> = e.attrs.iter().filter(|(k, _)| k.starts_with("r:") || k == "o:relid").cloned().collect();
            for (k, v) in rel_attrs {
                let v = v.to_string();
                if media_keys.contains(&v) || link_keys.contains(&v) {
                    e.set_attr(&k, rekey(&v));
                }
            }
            if matches!(e.name.as_str(), "w:footnoteReference" | "w:endnoteReference") {
                if let Some(id) = e.attr("w:id").filter(|id| note_keys.iter().any(|k| k == id)).map(String::from) {
                    e.set_attr("w:id", rekey(&id));
                }
            }
        };
        for e in parsed.body.iter_mut().chain(parsed.styles.iter_mut()).chain(parsed.numbering.iter_mut()) {
            walk_mut(e, &mut |x| renumber(x));
        }
        for e in parsed.body.iter_mut() {
            walk_mut(e, &mut |x| keyed(x));
        }
        // Styles the fragment already has keep their first definition.
        let have: Vec<String> = parse_list(&self.styles).iter().filter_map(|e| e.attr("w:styleId").map(String::from)).collect();
        parsed.styles.retain(|e| e.attr("w:styleId").is_none_or(|id| !have.contains(&id.to_string())));
        self.body.push_str(&join_xml(&parsed.body));
        self.styles.push_str(&join_xml(&parsed.styles));
        // Numbering lists the definitions before the numberings that name them.
        let mut numbering = mine;
        numbering.extend(parsed.numbering);
        numbering.sort_by_key(|e| e.name == "w:num");
        self.numbering = join_xml(&numbering);
        for (k, v) in other.media {
            self.media.insert(rekey(&k), v);
        }
        for (k, v) in other.links {
            self.links.insert(rekey(&k), v);
        }
        for (k, n) in other.notes {
            let mut els = parse_list(&n.xml);
            for e in els.iter_mut() {
                walk_mut(e, &mut |x| {
                    keyed(x);
                    if matches!(x.name.as_str(), "w:footnote" | "w:endnote") {
                        x.set_attr("w:id", rekey(&k));
                    }
                });
            }
            self.notes.insert(rekey(&k), FragNote { kind: n.kind, xml: join_xml(&els) });
        }
        for w in other.warnings {
            match self.warnings.iter_mut().find(|x| x.kind == w.kind) {
                Some(x) => x.count += w.count,
                None => self.warnings.push(w),
            }
        }
        for (p, u) in other.ns {
            if !self.ns.iter().any(|(q, _)| *q == p) {
                self.ns.push((p, u));
            }
        }
    }

    /// Declare which namespace prefixes the XML uses, from the known table plus `extra` (from a source document).
    pub fn set_namespaces(&mut self, extra: &[(String, String)]) {
        let all = format!("{}{}{}", self.body, self.styles, self.numbering);
        let mut ns: Vec<(String, String)> = Vec::new();
        let uses = |p: &str| all.contains(&format!("<{p}:")) || all.contains(&format!(" {p}:"));
        for (p, u) in NAMESPACES {
            if uses(p) {
                ns.push((p.to_string(), u.to_string()));
            }
        }
        for (p, u) in extra {
            if uses(p) && !ns.iter().any(|(q, _)| q == p) {
                ns.push((p.clone(), u.clone()));
            }
        }
        self.ns = ns;
    }
}

fn collect_text(el: &Element, lines: &mut Vec<String>) {
    match el.name.as_str() {
        "w:p" => {
            let mut t = String::new();
            para_text(el, &mut t);
            lines.push(t);
        }
        "w:tbl" => {
            for tr in el.children_named("w:tr") {
                let cells: Vec<String> = tr
                    .children_named("w:tc")
                    .map(|tc| {
                        let mut inner = Vec::new();
                        for c in tc.elements() {
                            collect_text(c, &mut inner);
                        }
                        inner.join(" ")
                    })
                    .collect();
                lines.push(cells.join("\t"));
            }
        }
        _ => {
            for c in el.elements() {
                collect_text(c, lines);
            }
        }
    }
}

fn para_text(el: &Element, out: &mut String) {
    for c in el.elements() {
        match c.name.as_str() {
            "w:t" => out.push_str(&c.text()),
            "w:tab" => out.push('\t'),
            "w:br" | "w:cr" if c.attr("w:type").is_none_or(|t| t == "textWrapping") => out.push('\n'),
            "w:pPr" | "w:rPr" | "w:instrText" | "w:delText" => {}
            "m:oMath" | "m:oMathPara" => out.push_str(&crate::math::omml_to_latex(c)),
            _ => para_text(c, out),
        }
    }
}

// ---------------------------------------------------------------------------
// Merge formatting
// ---------------------------------------------------------------------------

/// The list a paragraph style puts its paragraphs in, following `basedOn`.
fn style_numpr(styles: &[Element], id: &str) -> Option<Element> {
    let mut cur = id.to_string();
    for _ in 0..16 {
        let s = styles.iter().find(|s| s.attr("w:styleId") == Some(cur.as_str()))?;
        if let Some(n) = s.child("w:pPr").and_then(|p| p.child("w:numPr")) {
            return Some(n.clone());
        }
        cur = s.child("w:basedOn")?.attr("w:val")?.to_string();
    }
    None
}

fn style_lists_to_direct(el: &mut Element, styles: &[Element]) {
    if el.name == "w:p" {
        let style = el.child("w:pPr").and_then(|p| p.child("w:pStyle")).and_then(|s| s.attr("w:val")).map(String::from);
        let has_own = el.child("w:pPr").is_some_and(|p| p.child("w:numPr").is_some());
        if let (Some(style), false) = (style, has_own) {
            if let Some(n) = style_numpr(styles, &style) {
                if let Some(Node::Element(ppr)) = el.children.iter_mut().find(|c| matches!(c, Node::Element(e) if e.name == "w:pPr")) {
                    crate::props::put_ppr(ppr, n);
                }
            }
        }
        return;
    }
    for c in el.elements_mut() {
        style_lists_to_direct(c, styles);
    }
}

const KEEP_RUN: &[&str] = &["w:rStyle", "w:b", "w:bCs", "w:i", "w:iCs", "w:u", "w:strike", "w:dstrike", "w:vertAlign"];
const KEEP_PARA: &[&str] = &["w:numPr", "w:bidi"];

/// Strip fonts, sizes, colours, highlights, alignment, indents, spacing and paragraph styles; keep bold,
/// italic, underline, strike, sub- and superscript, links, lists and tables.
pub fn merge_formatting(els: &mut [Element]) {
    for el in els.iter_mut() {
        merge_el(el);
    }
}

fn merge_el(el: &mut Element) {
    match el.name.as_str() {
        "w:pPr" => el.children.retain(|n| matches!(n, Node::Element(e) if KEEP_PARA.contains(&e.name.as_str()))),
        "w:rPr" => el.children.retain(|n| {
            matches!(n, Node::Element(e) if KEEP_RUN.contains(&e.name.as_str()) && (e.name != "w:rStyle" || e.attr("w:val") == Some("Hyperlink")))
        }),
        "w:tcPr" => el.children.retain(|n| matches!(n, Node::Element(e) if e.name != "w:shd")),
        _ => {}
    }
    for c in el.elements_mut() {
        merge_el(c);
    }
}

// ---------------------------------------------------------------------------
// Walking
// ---------------------------------------------------------------------------

/// Visit an element and everything under it.
pub fn walk(el: &Element, f: &mut dyn FnMut(&Element)) {
    f(el);
    for c in el.elements() {
        walk(c, f);
    }
}

/// Visit an element and everything under it, mutably.
pub fn walk_mut(el: &mut Element, f: &mut dyn FnMut(&mut Element)) {
    f(el);
    for c in el.elements_mut() {
        walk_mut(c, f);
    }
}

// ---------------------------------------------------------------------------
// Base64
// ---------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Decode base64 (standard or URL-safe alphabet), ignoring whitespace and padding.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Pictures
// ---------------------------------------------------------------------------

/// What a picture's bytes are, from their first bytes: the file extension and the size in pixels.
pub fn sniff_image(data: &[u8]) -> Option<(&'static str, u32, u32)> {
    if data.len() > 24 && data.starts_with(&[0x89, b'P', b'N', b'G']) {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some(("png", w, h));
    }
    if data.len() > 10 && data.starts_with(b"GIF8") {
        return Some(("gif", u16::from_le_bytes([data[6], data[7]]) as u32, u16::from_le_bytes([data[8], data[9]]) as u32));
    }
    if data.len() > 4 && data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < data.len() {
            if data[i] != 0xFF {
                i += 1;
                continue;
            }
            let m = data[i + 1];
            if m == 0xFF {
                i += 1;
                continue;
            }
            if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
                let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                return Some(("jpg", w, h));
            }
            if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) {
                i += 2;
                continue;
            }
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            i += 2 + len;
        }
        return Some(("jpg", 0, 0));
    }
    if data.len() > 26 && data.starts_with(b"BM") {
        let w = i32::from_le_bytes([data[18], data[19], data[20], data[21]]).unsigned_abs();
        let h = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).unsigned_abs();
        return Some(("bmp", w, h));
    }
    if data.len() > 30 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        return Some(("webp", 0, 0));
    }
    None
}

/// A picture as an inline `w:drawing` run content, named by a fragment key. Sizes are in points.
pub fn inline_drawing(key: &str, w_pt: f64, h_pt: f64, id: u32, alt: &str) -> Element {
    let cx = (w_pt * 12700.0).round() as i64;
    let cy = (h_pt * 12700.0).round() as i64;
    let alt = alt.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    let src = format!(
        r#"<w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Picture {id}" descr="{alt}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="Picture {id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{key}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#
    );
    xml::parse(src.as_bytes()).expect("valid drawing").root
}

// ---------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------

/// One level of a list definition.
#[derive(Debug, Clone)]
pub struct LevelSpec {
    /// `bullet`, `decimal`, `lowerLetter`, `upperLetter`, `lowerRoman`, `upperRoman`...
    pub fmt: String,
    /// `%1.` or a bullet character.
    pub text: String,
    pub start: i64,
    pub left: i64,
    pub hanging: i64,
    /// Font for the bullet glyph (Symbol, Wingdings...).
    pub font: Option<String>,
}

impl LevelSpec {
    pub fn bullet(level: usize) -> LevelSpec {
        let glyph = ["\u{2022}", "o", "\u{25AA}"][level % 3];
        let font = if level % 3 == 1 { "Courier New" } else { "Arial" };
        LevelSpec { fmt: "bullet".into(), text: glyph.into(), start: 1, left: 720 * (level as i64 + 1), hanging: 360, font: Some(font.into()) }
    }

    pub fn numbered(level: usize, fmt: &str) -> LevelSpec {
        LevelSpec { fmt: fmt.into(), text: format!("%{}.", level + 1), start: 1, left: 720 * (level as i64 + 1), hanging: 360, font: None }
    }
}

/// Builds a fragment: blocks, styles, lists, pictures, links.
#[derive(Default)]
pub struct Builder {
    frag: Fragment,
    styles: Vec<Element>,
    numbering: Vec<Element>,
    next_num: i64,
    link_keys: BTreeMap<String, String>,
    drawing_id: u32,
}

impl Builder {
    pub fn new() -> Builder {
        Builder { frag: Fragment { version: 1, ..Default::default() }, next_num: 1, drawing_id: 1, ..Default::default() }
    }

    /// Add a block (`w:p` or `w:tbl`).
    pub fn block(&mut self, el: Element) {
        self.frag.body.push_str(&el.to_xml());
    }

    /// Whether any block was added.
    pub fn has_content(&self) -> bool {
        !self.frag.body.is_empty()
    }

    pub fn set_block(&mut self, block: bool) {
        self.frag.block = block;
    }

    /// Add a picture; returns the key to use in `r:embed`. `None` when the bytes aren't a picture Word can show.
    pub fn media(&mut self, data: &[u8]) -> Option<String> {
        let (ext, _, _) = sniff_image(data)?;
        let key = format!("m{}", self.frag.media.len() + 1);
        self.frag.media.insert(key.clone(), FragMedia { ext: ext.into(), data: b64_encode(data) });
        Some(key)
    }

    /// A key for a link target (the same target gets the same key).
    pub fn link(&mut self, url: &str) -> String {
        if let Some(k) = self.link_keys.get(url) {
            return k.clone();
        }
        let key = format!("l{}", self.frag.links.len() + 1);
        self.frag.links.insert(key.clone(), url.to_string());
        self.link_keys.insert(url.to_string(), key.clone());
        key
    }

    /// A fresh id for a drawing.
    pub fn drawing_id(&mut self) -> u32 {
        self.drawing_id += 1;
        self.drawing_id
    }

    /// Word's definition of a built-in style (`Heading 1`, `Quote`, `Hyperlink`, `List Paragraph`, `Code`...), added once.
    /// Returns its style id.
    pub fn builtin_style(&mut self, name: &str) -> Option<String> {
        let src = crate::styles::builtin_style_xml(name)?;
        let parsed = xml::parse(format!("<x>{src}</x>").as_bytes()).ok()?;
        let mut first = None;
        for e in parsed.root.elements() {
            let id = e.attr("w:styleId")?.to_string();
            first.get_or_insert(id.clone());
            self.style(e.clone());
        }
        first
    }

    /// Add a style definition (a style with the same id is kept as it is).
    pub fn style(&mut self, el: Element) {
        let id = el.attr("w:styleId").unwrap_or_default();
        if !self.styles.iter().any(|s| s.attr("w:styleId") == Some(id)) {
            self.styles.push(el);
        }
    }

    /// A new list definition; returns its numId. Levels not given repeat the pattern of the last given.
    pub fn list(&mut self, levels: &[LevelSpec]) -> i64 {
        let abs_id = self.numbering.iter().filter(|e| e.name == "w:abstractNum").count() as i64;
        let num_id = self.next_num;
        self.next_num += 1;
        let mut abs = Element::new("w:abstractNum").with_attr("w:abstractNumId", abs_id.to_string()).with_child(Element::new("w:multiLevelType").with_attr("w:val", "hybridMultilevel"));
        for l in 0..9usize {
            let spec = levels.get(l).cloned().unwrap_or_else(|| match levels.last() {
                Some(last) if last.fmt == "bullet" => LevelSpec::bullet(l),
                _ => LevelSpec::numbered(l, ["decimal", "lowerLetter", "lowerRoman"][l % 3]),
            });
            let mut lvl = Element::new("w:lvl")
                .with_attr("w:ilvl", l.to_string())
                .with_child(Element::new("w:start").with_attr("w:val", spec.start.to_string()))
                .with_child(Element::new("w:numFmt").with_attr("w:val", &spec.fmt))
                .with_child(Element::new("w:lvlText").with_attr("w:val", &spec.text))
                .with_child(Element::new("w:lvlJc").with_attr("w:val", if spec.fmt == "lowerRoman" { "right" } else { "left" }))
                .with_child(Element::new("w:pPr").with_child(Element::new("w:ind").with_attr("w:left", spec.left.to_string()).with_attr("w:hanging", spec.hanging.to_string())));
            if let Some(f) = &spec.font {
                lvl = lvl.with_child(Element::new("w:rPr").with_child(Element::new("w:rFonts").with_attr("w:ascii", f).with_attr("w:hAnsi", f).with_attr("w:hint", "default")));
            }
            abs = abs.with_child(lvl);
        }
        // All abstractNums come before all nums.
        let at = self.numbering.iter().rposition(|e| e.name == "w:abstractNum").map(|i| i + 1).unwrap_or(0);
        self.numbering.insert(at, abs);
        self.numbering.push(Element::new("w:num").with_attr("w:numId", num_id.to_string()).with_child(Element::new("w:abstractNumId").with_attr("w:val", abs_id.to_string())));
        num_id
    }

    pub fn warn(&mut self, kind: &str) {
        match self.frag.warnings.iter_mut().find(|w| w.kind == kind) {
            Some(w) => w.count += 1,
            None => self.frag.warnings.push(FragWarning { kind: kind.into(), count: 1 }),
        }
    }

    pub fn finish(mut self) -> Fragment {
        self.frag.styles = join_xml(&self.styles);
        self.frag.numbering = join_xml(&self.numbering);
        self.frag.set_namespaces(&[]);
        self.frag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for data in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0, 255, 128, 7, 9]] {
            assert_eq!(b64_decode(&b64_encode(data)).unwrap(), data);
        }
        assert_eq!(b64_decode("aGk=").unwrap(), b"hi");
    }

    #[test]
    fn image_sizes() {
        // 1x1 PNG
        let png = b64_decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==").unwrap();
        assert_eq!(sniff_image(&png), Some(("png", 1, 1)));
        assert_eq!(sniff_image(b"nope"), None);
    }

    #[test]
    fn builder_and_text() {
        let mut b = Builder::new();
        let h = b.builtin_style("Heading 1").unwrap();
        assert_eq!(h, "Heading1");
        let n = b.list(&[LevelSpec::bullet(0)]);
        assert_eq!(n, 1);
        let p = xml::parse(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Hi</w:t></w:r><w:r><w:tab/><w:t>there</w:t></w:r></w:p>"#.as_bytes()).unwrap().root;
        b.block(p);
        let f = b.finish();
        assert_eq!(f.plain_text(), "Hi\tthere");
        assert!(f.ns.iter().any(|(p, _)| p == "w"));
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(Fragment::from_json(&json).unwrap(), f);
    }

    #[test]
    fn merge_keeps_emphasis_and_lists_only() {
        let p = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:jc w:val="center"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Comic Sans MS"/><w:b/><w:color w:val="FF0000"/><w:sz w:val="40"/></w:rPr><w:t>x</w:t></w:r></w:p>"#;
        let f = Fragment { body: p.into(), ..Default::default() };
        let m = f.merged();
        assert!(m.body.contains("<w:b/>"));
        assert!(m.body.contains("w:numId"));
        for gone in ["Comic", "FF0000", "w:sz", "w:jc", "Heading1"] {
            assert!(!m.body.contains(gone), "{gone} should be gone: {}", m.body);
        }
    }
}
