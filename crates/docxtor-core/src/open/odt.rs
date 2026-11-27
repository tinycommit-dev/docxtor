//! OpenDocument text (.odt, and the single-file .fodt) into a fragment.
//!
//! The reader keeps text with its character and paragraph formatting, headings, lists (with their
//! bullets and number formats), tables (column widths, spans, cell shading and borders), links,
//! pictures, footnotes, the page size and margins, and the document's title, author, subject and
//! keywords. Styles are not copied as styles: each paragraph and run gets the formatting its style chain
//! gives it, and a paragraph in the headings, Title, Subtitle or Quotations styles uses Word's own
//! style of that name. Lost: headers and footers, text boxes' placement (their text is kept as ordinary
//! paragraphs), drawings, charts and formulas, tracked changes (the final text is kept), index and
//! table-of-contents formatting (the entries stay as text) and columns.

use super::text::{Converted, Meta, Page};
use super::zipscan::RawEntry;
use super::{Note, OpenError};
use crate::fragment::{inline_drawing, Builder, FragDefaults, FragNote, LevelSpec};
use crate::props::{put_ppr, put_rpr};
use crate::xml::{self, Element, Node};
use std::collections::HashMap;

const MAX_DEPTH: usize = crate::budget::caps::ODT_DEPTH;

/// An ODT zip: `content.xml`, `styles.xml`, `meta.xml` and the pictures.
pub fn read_zip(entries: &[RawEntry]) -> Result<Converted, OpenError> {
    let get = |n: &str| entries.iter().find(|e| e.name == n).map(|e| e.data.as_slice());
    let content = get("content.xml").ok_or_else(|| OpenError::new("no-text", "The file has no content.xml."))?;
    let content = parse_lenient(content)?;
    let styles = get("styles.xml").and_then(|d| parse_lenient(d).ok());
    let meta = get("meta.xml").and_then(|d| parse_lenient(d).ok());
    let pics: HashMap<String, &[u8]> = entries.iter().map(|e| (e.name.clone(), e.data.as_slice())).collect();
    convert(&content, styles.as_ref(), meta.as_ref(), &pics)
}

/// A flat ODT: one `office:document` with styles, meta and content together; pictures are inline base64.
pub fn read_flat(bytes: &[u8]) -> Result<Converted, OpenError> {
    let doc = parse_lenient(bytes)?;
    let mut pics: HashMap<String, Vec<u8>> = HashMap::new();
    collect_inline_pictures(&doc, &mut pics);
    let refs: HashMap<String, &[u8]> = pics.iter().map(|(k, v)| (k.clone(), v.as_slice())).collect();
    convert(&doc, Some(&doc), Some(&doc), &refs)
}

fn parse_lenient(bytes: &[u8]) -> Result<Element, OpenError> {
    match xml::parse(bytes) {
        Ok(d) => Ok(d.root),
        Err(_) => {
            let r = super::xmlfix::recover(bytes).ok_or_else(|| OpenError::new("damaged", "A part of the file can't be read."))?;
            Ok(xml::parse(&r.bytes).map_err(|e| OpenError::new("damaged", e.to_string()))?.root)
        }
    }
}

/// In a flat ODT a picture's bytes sit under the `draw:image` as `office:binary-data`; they are named
/// by a made-up address so the converter can find them like zip entries.
fn collect_inline_pictures(el: &Element, out: &mut HashMap<String, Vec<u8>>) {
    if el.name == "draw:image" {
        if let Some(bin) = el.child("office:binary-data") {
            if let Some(data) = crate::fragment::b64_decode(&bin.text()) {
                out.insert(format!("inline-{}", out.len()), data);
            }
        }
    }
    for c in el.elements() {
        collect_inline_pictures(c, out);
    }
}

// ---------------------------------------------------------------------------
// Lengths
// ---------------------------------------------------------------------------

/// A length such as `2.5cm`, `0.1in`, `12pt` or `3mm` in points.
fn points(v: &str) -> Option<f64> {
    let v = v.trim();
    let end = v.find(|c: char| c.is_alphabetic() || c == '%').unwrap_or(v.len());
    let n: f64 = v[..end].parse().ok()?;
    Some(match &v[end..] {
        "cm" => n * 28.3465,
        "mm" => n * 2.83465,
        "in" => n * 72.0,
        "pt" | "" => n,
        "pc" => n * 12.0,
        "px" => n * 0.75,
        _ => return None,
    })
}

fn twips(v: &str) -> Option<i64> {
    points(v).map(|p| (p * 20.0).round() as i64)
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Style {
    parent: Option<String>,
    display: String,
    list_style: Option<String>,
    text: HashMap<String, String>,
    para: HashMap<String, String>,
    cell: HashMap<String, String>,
    col: HashMap<String, String>,
}

#[derive(Default)]
struct Styles {
    by_name: HashMap<String, Style>,
    /// The default text and paragraph properties of paragraphs (`style:default-style`).
    default_text: HashMap<String, String>,
    default_para: HashMap<String, String>,
    list_styles: HashMap<String, Element>,
}

fn props(el: &Element, tag: &str) -> HashMap<String, String> {
    el.child(tag).map(|p| p.attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()).unwrap_or_default()
}

impl Styles {
    fn load(&mut self, root: &Element) {
        for group in root.elements() {
            if !matches!(group.name.as_str(), "office:styles" | "office:automatic-styles" | "office:master-styles") {
                continue;
            }
            for s in group.elements() {
                match s.name.as_str() {
                    "style:style" => {
                        let Some(name) = s.attr("style:name") else { continue };
                        let mut st = Style {
                            parent: s.attr("style:parent-style-name").map(String::from),
                            display: s.attr("style:display-name").unwrap_or(name).into(),
                            list_style: s.attr("style:list-style-name").map(String::from),
                            text: props(s, "style:text-properties"),
                            para: props(s, "style:paragraph-properties"),
                            cell: props(s, "style:table-cell-properties"),
                            col: props(s, "style:table-column-properties"),
                        };
                        // Properties of the paragraph style that a list or table style also names.
                        if st.display.is_empty() {
                            st.display = name.into();
                        }
                        self.by_name.insert(name.to_string(), st);
                    }
                    "style:default-style" if s.attr("style:family") == Some("paragraph") => {
                        self.default_text = props(s, "style:text-properties");
                        self.default_para = props(s, "style:paragraph-properties");
                    }
                    "text:list-style" => {
                        if let Some(name) = s.attr("style:name") {
                            self.list_styles.insert(name.to_string(), s.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// A style and its parents, the style itself first.
    fn chain(&self, name: &str) -> Vec<&Style> {
        let mut out = Vec::new();
        let mut cur = Some(name.to_string());
        while let Some(n) = cur {
            let Some(s) = self.by_name.get(&n) else { break };
            if out.len() > 20 {
                break;
            }
            out.push(s);
            cur = s.parent.clone();
        }
        out
    }

    /// The Word style a style chain stands for, and how many styles from the start of the chain are below it.
    fn word_style(&self, chain: &[&Style]) -> Option<(&'static str, usize)> {
        for (i, s) in chain.iter().enumerate() {
            let key = s.display.replace(['_', '-'], " ").to_lowercase();
            let key = key.replace("20", "").trim().to_string();
            let key = key.replace("  ", " ");
            let name = match key.as_str() {
                "heading 1" | "heading1" => "Heading 1",
                "heading 2" | "heading2" => "Heading 2",
                "heading 3" | "heading3" => "Heading 3",
                "heading 4" | "heading4" => "Heading 4",
                "heading 5" | "heading5" => "Heading 5",
                "heading 6" | "heading6" => "Heading 6",
                "title" => "Title",
                "subtitle" => "Subtitle",
                "quotations" | "quote" => "Quote",
                _ => continue,
            };
            return Some((name, i));
        }
        None
    }

    /// The merged text and paragraph properties of the first `n` styles of a chain (parents first, so a child wins).
    fn merged(&self, chain: &[&Style], n: usize, which: fn(&Style) -> &HashMap<String, String>) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for s in chain[..n.min(chain.len())].iter().rev() {
            for (k, v) in which(s) {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn hex(v: &str) -> Option<String> {
    let h = v.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_uppercase())
}

fn rpr_of(t: &HashMap<String, String>) -> Element {
    let mut r = Element::new("w:rPr");
    let get = |k: &str| t.get(k).map(String::as_str);
    if let Some(f) = get("style:font-name").or_else(|| get("fo:font-family")) {
        let f = f.trim_matches('\'');
        put_rpr(&mut r, Element::new("w:rFonts").with_attr("w:ascii", f).with_attr("w:hAnsi", f).with_attr("w:cs", f));
    }
    if get("fo:font-weight") == Some("bold") || get("fo:font-weight").and_then(|w| w.parse::<u32>().ok()).is_some_and(|w| w >= 600) {
        put_rpr(&mut r, Element::new("w:b"));
    }
    if matches!(get("fo:font-style"), Some("italic") | Some("oblique")) {
        put_rpr(&mut r, Element::new("w:i"));
    }
    if get("fo:text-transform") == Some("uppercase") {
        put_rpr(&mut r, Element::new("w:caps"));
    }
    if get("fo:font-variant") == Some("small-caps") {
        put_rpr(&mut r, Element::new("w:smallCaps"));
    }
    if get("style:text-line-through-style").is_some_and(|s| s != "none") {
        put_rpr(&mut r, Element::new("w:strike"));
    }
    if get("text:display") == Some("none") {
        put_rpr(&mut r, Element::new("w:vanish"));
    }
    if let Some(c) = get("fo:color").and_then(hex) {
        put_rpr(&mut r, Element::new("w:color").with_attr("w:val", c));
    }
    if let Some(sz) = get("fo:font-size").and_then(points) {
        let half = ((sz * 2.0).round() as i64).clamp(2, 3276);
        put_rpr(&mut r, Element::new("w:sz").with_attr("w:val", half.to_string()));
        put_rpr(&mut r, Element::new("w:szCs").with_attr("w:val", half.to_string()));
    }
    if let Some(c) = get("fo:background-color").and_then(hex) {
        put_rpr(&mut r, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", c));
    }
    if get("style:text-underline-style").is_some_and(|s| s != "none") {
        let val = match get("style:text-underline-type") {
            Some("double") => "double",
            _ => match get("style:text-underline-style") {
                Some("dotted") => "dotted",
                Some("dash") | Some("long-dash") => "dash",
                Some("wave") => "wave",
                _ => "single",
            },
        };
        let mut u = Element::new("w:u").with_attr("w:val", val);
        if let Some(c) = get("style:text-underline-color").and_then(hex) {
            u.set_attr("w:color", c);
        }
        put_rpr(&mut r, u);
    }
    if let Some(pos) = get("style:text-position") {
        let first = pos.split_whitespace().next().unwrap_or("");
        let v = if first.starts_with("super") || first.trim_end_matches('%').parse::<f64>().is_ok_and(|n| n > 0.0) {
            Some("superscript")
        } else if first.starts_with("sub") || first.trim_end_matches('%').parse::<f64>().is_ok_and(|n| n < 0.0) {
            Some("subscript")
        } else {
            None
        };
        if let Some(v) = v {
            put_rpr(&mut r, Element::new("w:vertAlign").with_attr("w:val", v));
        }
    }
    r
}

fn border(v: &str) -> Option<Element> {
    // `0.5pt solid #000000`
    let v = v.trim();
    if v == "none" || v.is_empty() {
        return None;
    }
    let mut width = 0.5;
    let mut style = "single";
    let mut color = "auto".to_string();
    for part in v.split_whitespace() {
        if let Some(p) = points(part) {
            width = p;
        } else if let Some(h) = hex(part) {
            color = h;
        } else if part == "double" {
            style = "double";
        } else if part == "dotted" {
            style = "dotted";
        } else if part == "dashed" {
            style = "dashed";
        }
    }
    let sz = ((width * 8.0).round() as i64).clamp(2, 96);
    Some(Element::new("w:top").with_attr("w:val", style).with_attr("w:sz", sz.to_string()).with_attr("w:space", "0").with_attr("w:color", color))
}

fn side(border: Option<Element>, name: &'static str) -> Option<Element> {
    border.map(|mut b| {
        b.name = name.into();
        b
    })
}

fn ppr_of(p: &HashMap<String, String>) -> Element {
    let mut r = Element::new("w:pPr");
    let get = |k: &str| p.get(k).map(String::as_str);
    if get("fo:break-before") == Some("page") {
        put_ppr(&mut r, Element::new("w:pageBreakBefore"));
    }
    if get("fo:keep-with-next") == Some("always") {
        put_ppr(&mut r, Element::new("w:keepNext"));
    }
    if get("fo:keep-together") == Some("always") {
        put_ppr(&mut r, Element::new("w:keepLines"));
    }
    let b = Element::new("w:pBdr");
    let mut b = b;
    for (key, name) in [("top", "w:top"), ("left", "w:left"), ("bottom", "w:bottom"), ("right", "w:right")] {
        let v = get(&format!("fo:border-{key}")).or_else(|| get("fo:border"));
        if let Some(e) = v.and_then(|v| side(border(v), name)) {
            b.children.push(Node::Element(e));
        }
    }
    if !b.children.is_empty() {
        // Sides come in the order top, left, bottom, right.
        put_ppr(&mut r, b);
    }
    if let Some(c) = get("fo:background-color").and_then(hex) {
        put_ppr(&mut r, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", c));
    }
    let mut spacing = Element::new("w:spacing");
    let before = get("fo:margin-top").and_then(twips);
    let after = get("fo:margin-bottom").and_then(twips);
    if let Some(v) = before {
        spacing.set_attr("w:before", v.max(0).to_string());
    }
    if let Some(v) = after {
        spacing.set_attr("w:after", v.max(0).to_string());
    }
    if let Some(lh) = get("fo:line-height") {
        if let Some(pct) = lh.strip_suffix('%').and_then(|n| n.parse::<f64>().ok()) {
            spacing.set_attr("w:line", ((pct * 2.4).round() as i64).to_string());
            spacing.set_attr("w:lineRule", "auto");
        } else if let Some(t) = twips(lh) {
            spacing.set_attr("w:line", t.to_string());
            spacing.set_attr("w:lineRule", "atLeast");
        }
    }
    if !spacing.attrs.is_empty() {
        put_ppr(&mut r, spacing);
    }
    let mut ind = Element::new("w:ind");
    let left = get("fo:margin-left").or_else(|| get("fo:margin")).and_then(twips);
    let right = get("fo:margin-right").or_else(|| get("fo:margin")).and_then(twips);
    if let Some(v) = left.filter(|v| *v != 0) {
        ind.set_attr("w:left", v.to_string());
    }
    if let Some(v) = right.filter(|v| *v != 0) {
        ind.set_attr("w:right", v.to_string());
    }
    if let Some(v) = get("fo:text-indent").and_then(twips).filter(|v| *v != 0) {
        ind.set_attr(if v < 0 { "w:hanging" } else { "w:firstLine" }, v.abs().to_string());
    }
    if !ind.attrs.is_empty() {
        put_ppr(&mut r, ind);
    }
    if let Some(a) = get("fo:text-align") {
        let jc = match a {
            "center" => Some("center"),
            "end" | "right" => Some("right"),
            "justify" => Some("both"),
            _ => None,
        };
        if let Some(jc) = jc {
            put_ppr(&mut r, Element::new("w:jc").with_attr("w:val", jc));
        }
    }
    r
}

// ---------------------------------------------------------------------------
// The converter
// ---------------------------------------------------------------------------

struct Conv<'a> {
    b: Builder,
    st: Styles,
    pics: &'a HashMap<String, &'a [u8]>,
    flat_pics: usize,
    lists: HashMap<String, Vec<LevelSpec>>,
    notes: Vec<(String, FragNote)>,
    pictures_lost: usize,
    objects_lost: usize,
    boxes: usize,
    changes: bool,
    /// Paragraphs from text boxes, to place after the paragraph that holds them.
    pending: Vec<Element>,
    text_width: i64,
    /// How deep the markup being read is nested; deeper than any real file goes is left out.
    depth: usize,
}

/// A paragraph's Word style id, run properties, paragraph properties and list style name.
type ParaStyles = (Option<String>, HashMap<String, String>, HashMap<String, String>, Option<String>);

/// What the text so far in a paragraph ended with, to collapse white space as ODF does.
struct Flow {
    space: bool,
}

fn convert(content: &Element, styles: Option<&Element>, meta: Option<&Element>, pics: &HashMap<String, &[u8]>) -> Result<Converted, OpenError> {
    let mut st = Styles::default();
    if let Some(s) = styles {
        st.load(s);
    }
    // Automatic styles of the document win over named ones with the same name.
    st.load(content);
    let body = content.child("office:body").and_then(|b| b.child("office:text")).ok_or_else(|| OpenError::new("no-text", "The file has no text."))?;
    let page = page_of(styles.unwrap_or(content));
    let text_width = page.map(|p| p.width - p.margins[1] - p.margins[3]).unwrap_or(9360);
    let mut cv = Conv { b: Builder::new(), st, pics, flat_pics: 0, lists: HashMap::new(), notes: Vec::new(), pictures_lost: 0, objects_lost: 0, boxes: 0, changes: false, pending: Vec::new(), text_width, depth: 0 };
    let mut out = Vec::new();
    cv.blocks(body, None, &mut out);
    for e in out {
        cv.b.block(e);
    }
    let mut notes = Vec::new();
    if cv.pictures_lost > 0 {
        notes.push(Note::count("picture-unreadable", cv.pictures_lost));
    }
    if cv.objects_lost > 0 {
        notes.push(Note::count("object-dropped", cv.objects_lost));
    }
    if cv.boxes > 0 {
        notes.push(Note::count("lost-text-boxes", cv.boxes));
    }
    if has_header_or_footer(styles.unwrap_or(content)) {
        notes.push(Note::new("lost-headers"));
    }
    let defaults = {
        let std = cv.st.by_name.get("Standard").cloned().unwrap_or_default();
        let mut t = cv.st.default_text.clone();
        t.extend(std.text);
        let font = t.get("style:font-name").or_else(|| t.get("fo:font-family")).map(|f| f.trim_matches('\'').to_string());
        let size = t.get("fo:font-size").and_then(|s| points(s));
        (font.is_some() || size.is_some()).then_some(FragDefaults { font, size })
    };
    let note_list = std::mem::take(&mut cv.notes);
    cv.b.set_block(true);
    let mut fragment = cv.b.finish();
    fragment.defaults = defaults;
    for (k, n) in note_list {
        fragment.notes.insert(k, n);
    }
    Ok(Converted { fragment, meta: meta_of(meta), page, notes })
}

fn has_header_or_footer(root: &Element) -> bool {
    let mut all = Vec::new();
    root.find_all("style:master-page", &mut all);
    all.iter().any(|m| ["style:header", "style:footer", "style:header-left", "style:footer-left", "style:header-first", "style:footer-first"].iter().any(|tag| m.child(tag).is_some_and(|h| !h.text().trim().is_empty())))
}

fn page_of(root: &Element) -> Option<Page> {
    let mut masters = Vec::new();
    root.find_all("style:master-page", &mut masters);
    let layout_name = masters.first().and_then(|m| m.attr("style:page-layout-name")).map(String::from);
    let mut layouts = Vec::new();
    root.find_all("style:page-layout", &mut layouts);
    let layout = layouts.iter().find(|l| layout_name.as_deref().is_none_or(|n| l.attr("style:name") == Some(n)))?;
    let p = layout.child("style:page-layout-properties")?;
    let t = |k: &str| p.attr(k).and_then(twips);
    let (w, h) = (t("fo:page-width")?, t("fo:page-height")?);
    let m = |k: &str| t(k).unwrap_or(1134);
    Some(Page { width: w, height: h, margins: [m("fo:margin-top"), m("fo:margin-right"), m("fo:margin-bottom"), m("fo:margin-left")] })
}

fn meta_of(root: Option<&Element>) -> Meta {
    let mut m = Meta::default();
    let Some(root) = root else { return m };
    let Some(meta) = root.find("office:meta") else { return m };
    let t = |n: &str| meta.child(n).map(|e| e.text().trim().to_string()).filter(|s| !s.is_empty());
    m.title = t("dc:title");
    m.author = t("dc:creator").or_else(|| t("meta:initial-creator"));
    m.subject = t("dc:subject");
    m.description = t("dc:description");
    let keys: Vec<String> = meta.children_named("meta:keyword").map(|k| k.text().trim().to_string()).filter(|s| !s.is_empty()).collect();
    if !keys.is_empty() {
        m.keywords = Some(keys.join(", "));
    }
    m
}

/// The list a `text:list` (or its style) describes: one `LevelSpec` per level.
fn list_levels(style: &Element) -> Vec<LevelSpec> {
    let mut levels: Vec<(usize, LevelSpec)> = Vec::new();
    for l in style.elements() {
        let Some(n) = l.attr("text:level").and_then(|v| v.parse::<usize>().ok()).filter(|n| (1..=10).contains(n)) else { continue };
        let props = l.child("style:list-level-properties");
        let before = props.and_then(|p| p.attr("text:space-before")).and_then(twips).unwrap_or(0);
        let label = props.and_then(|p| p.attr("text:min-label-width")).and_then(twips).unwrap_or(360);
        let (left, hanging) = match props.and_then(|p| p.attr("text:list-level-position-and-space-mode")) {
            Some("label-alignment") => {
                let al = props.and_then(|p| p.child("style:list-level-label-alignment"));
                let left = al.and_then(|a| a.attr("fo:margin-left")).and_then(twips).unwrap_or(720);
                let indent = al.and_then(|a| a.attr("fo:text-indent")).and_then(twips).unwrap_or(-360);
                (left, (-indent).max(0))
            }
            _ => (before + label, label),
        };
        let spec = match l.name.as_str() {
            "text:list-level-style-bullet" => {
                let ch = l.attr("text:bullet-char").unwrap_or("\u{2022}").to_string();
                LevelSpec { fmt: "bullet".into(), text: ch, start: 1, left, hanging, font: Some("Arial".into()) }
            }
            "text:list-level-style-number" => {
                let fmt = match l.attr("style:num-format").unwrap_or("1") {
                    "a" => "lowerLetter",
                    "A" => "upperLetter",
                    "i" => "lowerRoman",
                    "I" => "upperRoman",
                    "" => "none",
                    _ => "decimal",
                };
                let display = l.attr("text:display-levels").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).clamp(1, n);
                let mut text = l.attr("style:num-prefix").unwrap_or("").to_string();
                for k in (n + 1 - display)..=n {
                    if k > n + 1 - display {
                        text.push('.');
                    }
                    text.push_str(&format!("%{k}"));
                }
                text.push_str(l.attr("style:num-suffix").unwrap_or("."));
                LevelSpec { fmt: fmt.into(), text, start: l.attr("text:start-value").and_then(|v| v.parse().ok()).unwrap_or(1), left, hanging, font: None }
            }
            _ => continue,
        };
        levels.push((n - 1, spec));
    }
    levels.sort_by_key(|(i, _)| *i);
    levels.into_iter().map(|(_, s)| s).collect()
}

impl<'a> Conv<'a> {
    /// A top-level (or table-cell) run of blocks.
    fn blocks(&mut self, parent: &Element, list: Option<(i64, usize)>, out: &mut Vec<Element>) {
        if self.depth > MAX_DEPTH {
            return;
        }
        self.depth += 1;
        self.blocks_in(parent, list, out);
        self.depth -= 1;
    }

    fn blocks_in(&mut self, parent: &Element, list: Option<(i64, usize)>, out: &mut Vec<Element>) {
        for c in parent.elements() {
            match c.name.as_str() {
                "text:p" | "text:h" => {
                    out.push(self.paragraph(c, list));
                    out.append(&mut self.pending);
                }
                "text:list" => self.list(c, list, out),
                "table:table" => {
                    if let Some(t) = self.table(c) {
                        out.push(t);
                    }
                }
                "text:section" | "text:table-of-content" | "text:illustration-index" | "text:table-index" | "text:alphabetical-index" | "text:bibliography" | "text:index-body" | "text:user-index" | "text:object-index" => {
                    self.blocks(c, list, out)
                }
                "text:index-title" => self.blocks(c, list, out),
                "text:tracked-changes" | "text:soft-page-break" => {
                    self.changes |= c.name == "text:tracked-changes" && c.elements().next().is_some();
                }
                "draw:frame" | "draw:custom-shape" | "draw:g" => self.objects_lost += 1,
                _ => {}
            }
        }
    }

    fn list(&mut self, l: &Element, parent: Option<(i64, usize)>, out: &mut Vec<Element>) {
        let (num, level) = match parent {
            Some((n, lv)) => (n, lv + 1),
            None => {
                let name = l.attr("text:style-name").map(String::from);
                let levels = match name.as_ref().and_then(|n| self.st.list_styles.get(n)) {
                    Some(style) => list_levels(style),
                    None => Vec::new(),
                };
                let levels = if levels.is_empty() { vec![LevelSpec::bullet(0)] } else { levels };
                let key = name.clone().unwrap_or_default();
                // A list restarts its numbering: each list gets its own instance of the definition.
                self.lists.entry(key).or_insert_with(|| levels.clone());
                (self.b.list(&levels), 0)
            }
        };
        for item in l.elements() {
            if item.name != "text:list-item" && item.name != "text:list-header" {
                continue;
            }
            self.blocks(item, Some((num, level)), out);
        }
    }

    /// Word style and the properties a paragraph carries itself.
    fn para_styles(&mut self, p: &Element) -> ParaStyles {
        let name = p.attr("text:style-name").unwrap_or("Standard");
        let chain = self.st.chain(name);
        let mut word = self.st.word_style(&chain);
        if word.is_none() && p.name == "text:h" {
            let lv = p.attr("text:outline-level").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).clamp(1, 6);
            word = Some((["Heading 1", "Heading 2", "Heading 3", "Heading 4", "Heading 5", "Heading 6"][lv - 1], chain.len()));
        }
        let below = word.map(|(_, i)| i).unwrap_or(chain.len());
        let text = self.st.merged(&chain, below, |s| &s.text);
        let para = self.st.merged(&chain, below, |s| &s.para);
        let list_style = chain.iter().find_map(|s| s.list_style.clone());
        let id = word.and_then(|(n, _)| self.b.builtin_style(n));
        (id, text, para, list_style)
    }

    fn paragraph(&mut self, p: &Element, list: Option<(i64, usize)>) -> Element {
        let (style, text_props, para_props, list_style) = self.para_styles(p);
        let mut ppr = ppr_of(&para_props);
        if let Some(id) = &style {
            put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", id));
        }
        // A paragraph in a list item, or in a style that names a list, is numbered.
        let numbering = list.or_else(|| {
            let ls = list_style?;
            let levels = list_levels(self.st.list_styles.get(&ls)?);
            (!levels.is_empty()).then(|| (self.b.list(&levels), 0))
        });
        if let Some((num, level)) = numbering {
            put_ppr(&mut ppr, Element::new("w:numPr").with_child(Element::new("w:ilvl").with_attr("w:val", level.min(8).to_string())).with_child(Element::new("w:numId").with_attr("w:val", num.to_string())));
            // Indents come from the list level.
            ppr.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "w:ind"));
        }
        let rpr = rpr_of(&text_props);
        let mut out = Element::new("w:p");
        if !ppr.children.is_empty() {
            out.children.push(Node::Element(ppr));
        }
        let mut flow = Flow { space: true };
        self.inlines(p, &rpr, &mut out.children, &mut flow);
        out
    }

    fn run(&self, rpr: &Element, content: Vec<Element>) -> Element {
        let mut r = Element::new("w:r");
        if !rpr.children.is_empty() {
            r.children.push(Node::Element(rpr.clone()));
        }
        r.children.extend(content.into_iter().map(Node::Element));
        r
    }

    fn text_run(&self, rpr: &Element, s: &str) -> Element {
        self.run(rpr, vec![Element::new("w:t").with_attr("xml:space", "preserve").with_text(s)])
    }

    /// The text of ODF markup with white space collapsed as the format says.
    fn push_text(&self, s: &str, rpr: &Element, out: &mut Vec<Node>, flow: &mut Flow) {
        let mut buf = String::new();
        for ch in s.chars() {
            if ch.is_whitespace() {
                if !flow.space {
                    buf.push(' ');
                    flow.space = true;
                }
            } else {
                buf.push(ch);
                flow.space = false;
            }
        }
        if !buf.is_empty() {
            out.push(Node::Element(self.text_run(rpr, &buf)));
        }
    }

    fn inlines(&mut self, parent: &Element, rpr: &Element, out: &mut Vec<Node>, flow: &mut Flow) {
        if self.depth > MAX_DEPTH {
            return;
        }
        self.depth += 1;
        self.inlines_in(parent, rpr, out, flow);
        self.depth -= 1;
    }

    fn inlines_in(&mut self, parent: &Element, rpr: &Element, out: &mut Vec<Node>, flow: &mut Flow) {
        for n in &parent.children {
            match n {
                Node::Text(t) | Node::CData(t) => self.push_text(t, rpr, out, flow),
                Node::Element(e) => self.inline(e, rpr, out, flow),
                _ => {}
            }
        }
    }

    fn inline(&mut self, e: &Element, rpr: &Element, out: &mut Vec<Node>, flow: &mut Flow) {
        match e.name.as_str() {
            "text:span" => {
                let chain = e.attr("text:style-name").map(|n| self.st.chain(n)).unwrap_or_default();
                let mut r = rpr.clone();
                let own = rpr_of(&self.st.merged(&chain, chain.len(), |s| &s.text));
                for c in own.elements() {
                    put_rpr(&mut r, c.clone());
                }
                if chain.iter().any(|s| s.display.replace(['_', '-'], " ").trim().eq_ignore_ascii_case("internet link")) {
                    if let Some(id) = self.b.builtin_style("Hyperlink") {
                        put_rpr(&mut r, Element::new("w:rStyle").with_attr("w:val", id));
                    }
                }
                self.inlines(e, &r, out, flow);
            }
            "text:a" => {
                let href = e.attr("xlink:href").unwrap_or_default();
                let mut h = Element::new("w:hyperlink");
                if let Some(anchor) = href.strip_prefix('#') {
                    h.set_attr("w:anchor", anchor);
                } else if !href.is_empty() {
                    let key = self.b.link(href);
                    h.set_attr("r:id", key);
                }
                let mut r = rpr.clone();
                if let Some(id) = self.b.builtin_style("Hyperlink") {
                    put_rpr(&mut r, Element::new("w:rStyle").with_attr("w:val", id));
                }
                let mut inner = Vec::new();
                self.inlines(e, &r, &mut inner, flow);
                h.children = inner;
                if !h.children.is_empty() {
                    out.push(Node::Element(h));
                }
            }
            "text:s" => {
                let n = e.attr("text:c").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).min(500);
                out.push(Node::Element(self.text_run(rpr, &" ".repeat(n))));
                flow.space = true;
            }
            "text:tab" => {
                out.push(Node::Element(self.run(rpr, vec![Element::new("w:tab")])));
                flow.space = true;
            }
            "text:line-break" => {
                out.push(Node::Element(self.run(rpr, vec![Element::new("w:br")])));
                flow.space = true;
            }
            "text:soft-page-break" | "text:bookmark" | "text:bookmark-start" | "text:bookmark-end" | "text:reference-mark" | "text:reference-mark-start" | "text:reference-mark-end" | "office:annotation" | "office:annotation-end" | "text:change-start" | "text:change-end" | "text:change" => {}
            "text:note" => self.note(e, rpr, out),
            "draw:frame" => self.frame(e, rpr, out),
            "text:changed-region" | "text:tracked-changes" => {}
            "draw:custom-shape" | "draw:g" | "draw:line" | "draw:rect" | "draw:ellipse" => self.objects_lost += 1,
            // Fields and everything else with text inside: keep the text.
            _ => self.inlines(e, rpr, out, flow),
        }
    }

    fn note(&mut self, e: &Element, rpr: &Element, out: &mut Vec<Node>) {
        let endnote = e.attr("text:note-class") == Some("endnote");
        let key = format!("{}{}", if endnote { "e" } else { "n" }, self.notes.len() + 1);
        let _ = self.b.builtin_style(if endnote { "Endnote Reference" } else { "Footnote Reference" });
        let _ = self.b.builtin_style(if endnote { "Endnote Text" } else { "Footnote Text" });
        let tag = if endnote { "w:endnote" } else { "w:footnote" };
        let mark_style = if endnote { "EndnoteReference" } else { "FootnoteReference" };
        let mut note = Element::new(tag).with_attr("w:id", key.as_str());
        let mut paras = Vec::new();
        if let Some(body) = e.child("text:note-body") {
            let saved = std::mem::take(&mut self.pending);
            self.blocks(body, None, &mut paras);
            self.pending = saved;
        }
        if paras.is_empty() {
            paras.push(Element::new("w:p"));
        }
        // The note's own number mark leads its first paragraph.
        let mark = Element::new("w:r").with_child(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", mark_style))).with_child(Element::new(if endnote { "w:endnoteRef" } else { "w:footnoteRef" }));
        if let Some(first) = paras.iter_mut().find(|p| p.name == "w:p") {
            let at = first.children.iter().position(|n| !matches!(n, Node::Element(e) if e.name == "w:pPr")).unwrap_or(first.children.len());
            first.children.insert(at, Node::Element(mark));
            first.children.insert(at + 1, Node::Element(Element::new("w:r").with_child(Element::new("w:t").with_attr("xml:space", "preserve").with_text(" "))));
        }
        note.children = paras.into_iter().map(Node::Element).collect();
        self.notes.push((key.clone(), FragNote { kind: if endnote { "endnote" } else { "footnote" }.into(), xml: note.to_xml() }));
        let r = Element::new("w:r").with_child(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", mark_style))).with_child(Element::new(if endnote { "w:endnoteReference" } else { "w:footnoteReference" }).with_attr("w:id", key));
        out.push(Node::Element(r));
        let _ = rpr;
    }

    /// A picture frame, or a text box (its paragraphs follow the paragraph that holds it).
    fn frame(&mut self, f: &Element, rpr: &Element, out: &mut Vec<Node>) {
        if let Some(img) = f.child("draw:image") {
            let bytes: Option<Vec<u8>> = match img.attr("xlink:href") {
                Some(h) if !h.is_empty() && !h.starts_with("http") => self.pics.get(h.trim_start_matches("./")).map(|b| b.to_vec()),
                _ => img.child("office:binary-data").and_then(|b| crate::fragment::b64_decode(&b.text())),
            };
            let w = f.attr("svg:width").and_then(points).unwrap_or(100.0);
            let h = f.attr("svg:height").and_then(points).unwrap_or(75.0);
            match bytes.and_then(|b| self.b.media(&b)) {
                Some(key) => {
                    let id = self.b.drawing_id();
                    let alt = f.child("svg:desc").map(|d| d.text()).unwrap_or_default();
                    out.push(Node::Element(self.run(rpr, vec![inline_drawing(&key, w.max(1.0), h.max(1.0), id, &alt)])));
                }
                None => self.pictures_lost += 1,
            }
            self.flat_pics += 1;
        } else if let Some(tb) = f.child("draw:text-box") {
            self.boxes += 1;
            let mut paras = Vec::new();
            self.blocks(tb, None, &mut paras);
            self.pending.append(&mut paras);
        } else {
            self.objects_lost += 1;
        }
    }

    // -- tables -----------------------------------------------------------

    fn table(&mut self, t: &Element) -> Option<Element> {
        // Columns, with their widths.
        let mut widths: Vec<i64> = Vec::new();
        let mut cols = Vec::new();
        collect_columns(t, &mut cols);
        for c in &cols {
            let w = c.attr("table:style-name").map(|n| self.st.chain(n)).and_then(|ch| ch.iter().find_map(|s| s.col.get("style:column-width").cloned())).and_then(|v| twips(&v));
            let n = c.attr("table:number-columns-repeated").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).min(63);
            for _ in 0..n {
                widths.push(w.unwrap_or(0));
            }
        }
        let ncols = widths.len().max(1);
        let known: i64 = widths.iter().sum();
        let fill = if known == 0 { self.text_width / ncols as i64 } else { 0 };
        if widths.is_empty() {
            widths.push(0);
        }
        for w in widths.iter_mut() {
            if *w <= 0 {
                *w = fill.max(720);
            }
        }
        let total: i64 = widths.iter().sum();

        let mut tbl = Element::new("w:tbl");
        let mut tblpr = Element::new("w:tblPr");
        tblpr.children.push(Node::Element(Element::new("w:tblW").with_attr("w:w", total.to_string()).with_attr("w:type", "dxa")));
        tblpr.children.push(Node::Element(Element::new("w:tblLayout").with_attr("w:type", "fixed")));
        tblpr.children.push(Node::Element(Element::new("w:tblLook").with_attr("w:val", "0000")));
        tbl.children.push(Node::Element(tblpr));
        let mut grid = Element::new("w:tblGrid");
        for w in &widths {
            grid.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", w.to_string())));
        }
        tbl.children.push(Node::Element(grid));

        let mut rows = Vec::new();
        collect_rows(t, &mut rows);
        // Row spans in progress: for each column, rows still covered and the span's column count.
        let mut covered: Vec<(usize, usize)> = vec![(0, 1); ncols];
        let mut any = false;
        for (ri, row) in rows.iter().enumerate() {
            let header = ri < header_rows(t);
            let mut tr = Element::new("w:tr");
            if header {
                tr.children.push(Node::Element(Element::new("w:trPr").with_child(Element::new("w:tblHeader"))));
            }
            let mut col = 0usize;
            let mut skip = 0usize;
            for cell in row.elements() {
                let repeat = cell.attr("table:number-columns-repeated").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).min(ncols.saturating_sub(col).max(1));
                for _ in 0..repeat {
                    if col >= ncols {
                        break;
                    }
                    let covered_cell = cell.name == "table:covered-table-cell";
                    if covered_cell {
                        if skip > 0 {
                            skip -= 1;
                            col += 1;
                            continue;
                        }
                        let (left, span) = covered[col];
                        if left > 0 {
                            let mut tcpr = Element::new("w:tcPr");
                            let w: i64 = widths[col..(col + span).min(ncols)].iter().sum();
                            tcpr.children.push(Node::Element(Element::new("w:tcW").with_attr("w:w", w.to_string()).with_attr("w:type", "dxa")));
                            if span > 1 {
                                tcpr.children.push(Node::Element(Element::new("w:gridSpan").with_attr("w:val", span.to_string())));
                            }
                            tcpr.children.push(Node::Element(Element::new("w:vMerge")));
                            let tc = Element::new("w:tc").with_child(tcpr).with_child(Element::new("w:p"));
                            tr.children.push(Node::Element(tc));
                            for c in covered.iter_mut().take((col + span).min(ncols)).skip(col) {
                                c.0 -= 1;
                            }
                            skip = span - 1;
                        }
                        col += 1;
                        continue;
                    }
                    let colspan = cell.attr("table:number-columns-spanned").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).clamp(1, ncols - col);
                    let rowspan = cell.attr("table:number-rows-spanned").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).max(1);
                    let w: i64 = widths[col..col + colspan].iter().sum();
                    let tc = self.cell(cell, w, colspan, rowspan > 1);
                    tr.children.push(Node::Element(tc));
                    if rowspan > 1 {
                        for c in covered.iter_mut().skip(col).take(colspan) {
                            *c = (rowspan - 1, colspan);
                        }
                    }
                    skip = 0;
                    // The cells a column span covers come next as covered cells in the file; they are skipped there.
                    col += colspan;
                    any = true;
                }
            }
            if !tr.elements().any(|c| c.name == "w:tc") {
                continue;
            }
            tbl.children.push(Node::Element(tr));
        }
        any.then_some(tbl)
    }

    fn cell(&mut self, cell: &Element, width: i64, colspan: usize, restart: bool) -> Element {
        let cell_props: HashMap<String, String> = cell.attr("table:style-name").map(|n| self.st.chain(n)).map(|ch| self.st.merged(&ch, ch.len(), |s| &s.cell)).unwrap_or_default();
        let mut tcpr = Element::new("w:tcPr");
        tcpr.children.push(Node::Element(Element::new("w:tcW").with_attr("w:w", width.to_string()).with_attr("w:type", "dxa")));
        if colspan > 1 {
            tcpr.children.push(Node::Element(Element::new("w:gridSpan").with_attr("w:val", colspan.to_string())));
        }
        if restart {
            tcpr.children.push(Node::Element(Element::new("w:vMerge").with_attr("w:val", "restart")));
        }
        let mut b = Element::new("w:tcBorders");
        for (key, name) in [("top", "w:top"), ("left", "w:left"), ("bottom", "w:bottom"), ("right", "w:right")] {
            let v = cell_props.get(&format!("fo:border-{key}")).or_else(|| cell_props.get("fo:border"));
            if let Some(e) = v.and_then(|v| side(border(v), name)) {
                b.children.push(Node::Element(e));
            }
        }
        if !b.children.is_empty() {
            tcpr.children.push(Node::Element(b));
        }
        if let Some(c) = cell_props.get("fo:background-color").and_then(|c| hex(c)) {
            tcpr.children.push(Node::Element(Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", c)));
        }
        if let Some(a) = cell_props.get("style:vertical-align") {
            let v = match a.as_str() {
                "middle" => Some("center"),
                "bottom" => Some("bottom"),
                _ => None,
            };
            if let Some(v) = v {
                tcpr.children.push(Node::Element(Element::new("w:vAlign").with_attr("w:val", v)));
            }
        }
        let mut tc = Element::new("w:tc").with_child(tcpr);
        let mut blocks = Vec::new();
        self.blocks(cell, None, &mut blocks);
        if blocks.is_empty() || blocks.last().is_some_and(|e| e.name == "w:tbl") {
            blocks.push(Element::new("w:p"));
        }
        tc.children.extend(blocks.into_iter().map(Node::Element));
        tc
    }
}

fn collect_columns<'a>(t: &'a Element, out: &mut Vec<&'a Element>) {
    for c in t.elements() {
        match c.name.as_str() {
            "table:table-column" => out.push(c),
            "table:table-columns" | "table:table-header-columns" | "table:table-column-group" => collect_columns(c, out),
            _ => {}
        }
    }
}

fn collect_rows<'a>(t: &'a Element, out: &mut Vec<&'a Element>) {
    for c in t.elements() {
        match c.name.as_str() {
            "table:table-row" => out.push(c),
            "table:table-header-rows" | "table:table-rows" | "table:table-row-group" => collect_rows(c, out),
            _ => {}
        }
    }
}

fn header_rows(t: &Element) -> usize {
    t.child("table:table-header-rows").map(|h| h.children_named("table:table-row").count()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::walk_paragraphs;
    use crate::open::{text, Format};

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    pub(crate) const CONTENT: &str = r##"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0">
<office:automatic-styles>
<style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold" fo:color="#ff0000"/></style:style>
<style:style style:name="P1" style:family="paragraph" style:parent-style-name="Standard"><style:paragraph-properties fo:text-align="center" fo:margin-top="0.1in" fo:margin-bottom="0.2in"/></style:style>
<style:style style:name="Tab1.A" style:family="table-cell"><style:table-cell-properties fo:background-color="#dddddd" fo:border="0.5pt solid #000000"/></style:style>
<style:style style:name="Tab1.col" style:family="table-column"><style:table-column-properties style:column-width="2in"/></style:style>
<text:list-style style:name="L1"><text:list-level-style-bullet text:level="1" text:bullet-char="&#x2022;"><style:list-level-properties text:space-before="0.25in" text:min-label-width="0.25in"/></text:list-level-style-bullet></text:list-style>
<text:list-style style:name="L2"><text:list-level-style-number text:level="1" style:num-format="1" style:num-suffix="."><style:list-level-properties text:space-before="0.25in" text:min-label-width="0.25in"/></text:list-level-style-number></text:list-style>
</office:automatic-styles>
<office:body><office:text>
<text:h text:style-name="Heading_20_1" text:outline-level="1">Big   heading</text:h>
<text:p text:style-name="P1">Plain <text:span text:style-name="T1">bold red</text:span> and <text:a xlink:href="https://example.com">a link</text:a>.<text:note text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p>The note.</text:p></text:note-body></text:note></text:p>
<text:list text:style-name="L1"><text:list-item><text:p>One</text:p></text:list-item><text:list-item><text:p>Two<text:line-break/>lines</text:p><text:list><text:list-item><text:p>Nested</text:p></text:list-item></text:list></text:list-item></text:list>
<text:list text:style-name="L2"><text:list-item><text:p>First</text:p></text:list-item></text:list>
<table:table table:name="Table1"><table:table-column table:style-name="Tab1.col" table:number-columns-repeated="2"/>
<table:table-row><table:table-cell table:style-name="Tab1.A"><text:p>A1</text:p></table:table-cell><table:table-cell table:style-name="Tab1.A" table:number-rows-spanned="2"><text:p>B1</text:p></table:table-cell></table:table-row>
<table:table-row><table:table-cell><text:p>A2</text:p></table:table-cell><table:covered-table-cell/></table:table-row></table:table>
<text:p><draw:frame svg:width="1in" svg:height="0.5in"><draw:image xlink:href="Pictures/a.png"/></draw:frame></text:p>
</office:text></office:body></office:document-content>"##;

    const STYLES: &str = r##"<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
<office:styles><style:default-style style:family="paragraph"><style:text-properties style:font-name="Liberation Serif" fo:font-size="12pt"/></style:default-style>
<style:style style:name="Standard" style:family="paragraph"/>
<style:style style:name="Heading_20_1" style:display-name="Heading 1" style:family="paragraph" style:parent-style-name="Standard"><style:text-properties fo:font-size="130%" fo:font-weight="bold"/></style:style></office:styles>
<office:automatic-styles><style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="8.5in" fo:page-height="11in" fo:margin-top="1in" fo:margin-bottom="1in" fo:margin-left="0.8in" fo:margin-right="0.8in"/></style:page-layout></office:automatic-styles>
<office:master-styles><style:master-page style:name="Standard" style:page-layout-name="pm1"/></office:master-styles></office:document-styles>"##;

    const META: &str = r#"<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0"><office:meta><dc:title>ODT test</dc:title><dc:creator>Linus</dc:creator><meta:keyword>one</meta:keyword><meta:keyword>two</meta:keyword></office:meta></office:document-meta>"#;

    pub(crate) fn entries() -> Vec<RawEntry> {
        let e = |n: &str, d: &[u8]| RawEntry { name: n.into(), data: d.to_vec(), stored: false, damaged: false };
        vec![e("mimetype", b"application/vnd.oasis.opendocument.text"), e("content.xml", CONTENT.as_bytes()), e("styles.xml", STYLES.as_bytes()), e("meta.xml", META.as_bytes()), e("Pictures/a.png", &crate::fragment::b64_decode(PNG).unwrap())]
    }

    #[test]
    fn an_odt_opens_with_its_formatting() {
        let conv = read_zip(&entries()).unwrap();
        let o = text::finish(Format::Odt, conv).unwrap();
        let mut doc = o.doc;
        let mut texts = Vec::new();
        walk_paragraphs(&doc.body, &mut |p| texts.push(p.plain_text()));
        assert_eq!(texts[0], "Big heading");
        assert!(texts[1].starts_with("Plain bold red and a link."), "{texts:?}");
        assert!(texts.contains(&"One".to_string()) && texts.contains(&"Nested".to_string()) && texts.contains(&"First".to_string()));
        assert!(texts.contains(&"Two\nlines".to_string()) || texts.contains(&"Two\u{b}lines".to_string()) || texts.iter().any(|t| t.starts_with("Two")), "{texts:?}");
        let bytes = doc.save().unwrap();
        let pkg = crate::package::Package::open(&bytes).unwrap();
        let body = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
        assert!(body.contains(r#"w:val="Heading1""#), "{body}");
        assert!(body.contains("<w:b/>") && body.contains("FF0000"), "{body}");
        assert!(body.contains(r#"<w:jc w:val="center"/>"#), "{body}");
        assert!(body.contains("w:hyperlink") && body.contains("w:numId") && body.contains("w:footnoteReference") && body.contains("w:tbl"), "{body}");
        assert!(body.contains(r#"w:vMerge w:val="restart""#) && body.contains("<w:vMerge/>") || body.contains(r#"<w:vMerge w:val="restart"/>"#), "{body}");
        assert!(body.contains("<w:drawing>"), "{body}");
        assert!(body.contains("DDDDDD") || body.contains("dddddd".to_uppercase().as_str()), "{body}");
        let setup = doc.page_setup();
        assert_eq!((setup.width, setup.height), (12240, 15840));
        assert_eq!(setup.margin, [1440, 1152, 1440, 1152]);
        let core = String::from_utf8(pkg.get("docProps/core.xml").unwrap().to_vec()).unwrap();
        assert!(core.contains("ODT test") && core.contains("Linus"), "{core}");
    }

    #[test]
    fn a_flat_odt_opens_too() {
        let flat = r#"<?xml version="1.0" encoding="UTF-8"?><office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:dc="http://purl.org/dc/elements/1.1/" office:mimetype="application/vnd.oasis.opendocument.text"><office:meta><dc:title>Flat one</dc:title></office:meta><office:automatic-styles><style:style style:name="B" style:family="text"><style:text-properties fo:font-style="italic"/></style:style></office:automatic-styles><office:body><office:text><text:p>Hello <text:span text:style-name="B">flat</text:span> world</text:p></office:text></office:body></office:document>"#;
        assert_eq!(crate::open::sniff(flat.as_bytes()), crate::open::Kind::Odt);
        let o = text::finish(Format::Odt, read_flat(flat.as_bytes()).unwrap()).unwrap();
        let mut texts = Vec::new();
        walk_paragraphs(&o.doc.body, &mut |p| texts.push(p.plain_text()));
        assert_eq!(texts[0], "Hello flat world");
        let mut doc = o.doc;
        let body = String::from_utf8(crate::package::Package::open(&doc.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
        assert!(body.contains("<w:i/>"), "{body}");
    }

    #[test]
    fn lengths() {
        assert_eq!(twips("1in"), Some(1440));
        assert_eq!(twips("2.54cm"), Some(1440));
        assert_eq!(twips("72pt"), Some(1440));
        assert_eq!(points("50%"), None);
    }
}
