//! Making a new .docx from what the text-format readers produce.
//!
//! RTF, HTML, OpenDocument text, Word 2003 XML and the old binary Word format are each read into a
//! `Fragment` (the same blocks, styles, lists and pictures a paste carries), plus the page setup and
//! document properties the fragment can't hold. `finish` pastes the fragment into a blank document, so a
//! converted file gets Word's default styles and every edit afterwards is an ordinary op.

use super::{Format, Note, OpenError, Opened, Report};
use crate::document::Document;
use crate::fragment::Fragment;
use crate::ops::{Op, Pos};

/// Page size and margins in twips (top, right, bottom, left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Page {
    pub width: i64,
    pub height: i64,
    pub margins: [i64; 4],
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub description: Option<String>,
}

impl Meta {
    fn is_empty(&self) -> bool {
        self.title.is_none() && self.author.is_none() && self.subject.is_none() && self.keywords.is_none() && self.description.is_none()
    }
}

/// What a reader makes of a file.
#[derive(Debug, Clone, Default)]
pub struct Converted {
    pub fragment: Fragment,
    pub meta: Meta,
    pub page: Option<Page>,
    pub notes: Vec<Note>,
}

/// Paste the fragment into a blank document, set the page and properties, and start with no undo history
/// (the document is written and read back).
pub fn finish(format: Format, mut conv: Converted) -> Result<Opened, OpenError> {
    let mut doc = Document::new_blank();
    let mut report = Report::new(format);
    report.notes = conv.notes;
    for w in &conv.fragment.warnings {
        report.notes.push(Note::count(&w.kind, w.count));
    }
    if conv.fragment.is_empty() && !conv.fragment.media.values().any(|_| true) {
        report.notes.push(Note::new("nothing-found"));
    }
    let first = doc.render_all().order[0];
    // The font and size text has when nothing says otherwise are the file's own, so they become the document's defaults.
    let defaults = conv.fragment.defaults.take();
    let mut ops = Vec::new();
    if let Some(d) = defaults.filter(|d| d.font.is_some() || d.size.is_some()) {
        ops.push(Op::SetDefaultFormat { format: crate::props::RunPatch { font: d.font, size: d.size, ..Default::default() } });
    }
    ops.push(Op::InsertFragment { at: Pos::new(first, 0), fragment: conv.fragment, mode: crate::fragment::PasteMode::Keep });
    if let Some(p) = conv.page {
        ops.push(Op::SetPageSetup { section: None, width: Some(p.width), height: Some(p.height), margins: Some(p.margins) });
    }
    if !conv.meta.is_empty() {
        let m = conv.meta;
        ops.push(Op::SetDocProps { title: m.title, subject: m.subject, author: m.author, keywords: m.keywords, description: m.description, category: None, status: None, company: None, manager: None });
    }
    doc.apply_batch(&ops, false).map_err(|e| OpenError::new("damaged", e.to_string()))?;
    let bytes = doc.save().map_err(|e| OpenError::new("damaged", e.to_string()))?;
    let doc = Document::open(&bytes).map_err(|e| OpenError::new("damaged", e.to_string()))?;
    Ok(Opened { doc, report })
}

// ---------------------------------------------------------------------------
// RTF
// ---------------------------------------------------------------------------

/// An RTF file: its text and formatting through the reader the clipboard uses, plus the page setup and the
/// document information from its header.
pub fn from_rtf(bytes: &[u8]) -> Converted {
    let src: String = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    let fragment = crate::rtf::rtf_to_fragment(&src);
    let mut notes = Vec::new();
    let has = |w: &str| src.contains(w);
    if has("{\\header") || has("{\\footer") || has("{\\headerl") || has("{\\footerl") {
        notes.push(Note::new("lost-headers"));
    }
    if has("{\\shp") || has("\\shpinst") || has("\\do") && has("\\dpx") {
        notes.push(Note::new("lost-drawings"));
    }
    Converted { fragment, meta: rtf_info(&src), page: rtf_page(&src), notes }
}

fn rtf_number(src: &str, word: &str) -> Option<i64> {
    let at = src.find(&format!("\\{word}"))?;
    let rest = &src[at + 1 + word.len()..];
    let digits: String = rest.chars().enumerate().take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && *c == '-')).map(|(_, c)| c).collect();
    // Don't mistake `\margl` for the start of `\marglsxn` and the like: the word must end here.
    digits.parse().ok()
}

/// The page setup in an RTF header (before the first paragraph).
fn rtf_page(src: &str) -> Option<Page> {
    let head_end = src.find("\\pard").or_else(|| src.find("\\sectd")).unwrap_or(src.len()).min(60_000);
    let head = &src[..head_end];
    let w = rtf_number(head, "paperw");
    let h = rtf_number(head, "paperh");
    let (Some(w), Some(h)) = (w, h) else { return None };
    if w < 1000 || h < 1000 {
        return None;
    }
    let landscape = head.contains("\\landscape");
    let m = |word: &str, default: i64| rtf_number(head, word).filter(|v| *v >= 0).unwrap_or(default);
    // RTF's `\paperw` is the paper's width as printed; in landscape Word writes the long side there already.
    let (w, h) = if landscape && w < h { (h, w) } else { (w, h) };
    Some(Page { width: w, height: h, margins: [m("margt", 1440), m("margr", 1800), m("margb", 1440), m("margl", 1800)] })
}

/// `{\info{\title ...}{\author ...}}`.
fn rtf_info(src: &str) -> Meta {
    let mut meta = Meta::default();
    let Some(at) = src.find("{\\info") else { return meta };
    let tail = &src[at..src.len().min(at + 4000)];
    let group = |name: &str| -> Option<String> {
        let start = tail.find(&format!("{{\\{name} "))? + name.len() + 3;
        let end = tail[start..].find('}')? + start;
        let v = tail[start..end].trim().to_string();
        (!v.is_empty() && !v.contains('\\')).then_some(v)
    };
    meta.title = group("title");
    meta.author = group("author");
    meta.subject = group("subject");
    meta.keywords = group("keywords");
    meta
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

/// The text of an HTML file's bytes: UTF-8 when they are, else the charset the page names, else Windows-1252.
fn html_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return super::xmlfix::decode(bytes);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

const CP1252_HI: [u16; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161,
    0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

pub(super) fn cp1252(b: u8) -> char {
    match b {
        0x80..=0x9F => char::from_u32(CP1252_HI[(b - 0x80) as usize] as u32).unwrap_or('?'),
        _ => b as char,
    }
}

/// An HTML file: the reader the clipboard uses, and the page's title. Pictures that are not `data:` addresses
/// can't be fetched from a file, so they are counted and left out.
pub fn from_html(bytes: &[u8]) -> Converted {
    let html = html_text(bytes);
    let fragment = crate::html_import::html_page_to_fragment(&html, &Default::default());
    let mut notes = Vec::new();
    let outside = crate::html_import::html_image_sources(&html).len();
    if outside > 0 {
        notes.push(Note::count("pictures-linked", outside));
    }
    let title = between(&html, "<title", "</title>").map(|t| t.split_once('>').map(|x| x.1).unwrap_or("").trim().to_string()).filter(|t| !t.is_empty());
    let meta = Meta { title: title.map(|t| decode_entities(&t)), ..Default::default() };
    Converted { fragment, meta, page: None, notes }
}

fn between<'a>(hay: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let lower = hay.to_ascii_lowercase();
    let a = lower.find(open)?;
    let b = lower[a..].find(close)? + a;
    hay.get(a..b)
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::walk_paragraphs;

    fn texts(doc: &Document) -> Vec<String> {
        let mut v = Vec::new();
        walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
        v
    }

    #[test]
    fn rtf_becomes_a_document_with_its_page_and_title() {
        let rtf = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}{\info{\title My report}{\author Ada}}\paperw15840\paperh12240\landscape\margl1000\margr1000\margt900\margb900\pard\b Bold\b0  and plain\par Second\par}";
        let o = finish(Format::Rtf, from_rtf(rtf.as_bytes())).unwrap();
        assert_eq!(texts(&o.doc), ["Bold and plain", "Second"]);
        assert!(o.report.converted);
        let mut doc = o.doc;
        let setup = doc.page_setup();
        assert_eq!((setup.width, setup.height), (15840, 12240));
        let bytes = doc.save().unwrap();
        let pkg = crate::package::Package::open(&bytes).unwrap();
        let core = String::from_utf8(pkg.get("docProps/core.xml").unwrap().to_vec()).unwrap();
        assert!(core.contains("My report") && core.contains("Ada"), "{core}");
    }

    #[test]
    fn html_becomes_a_document_with_its_title() {
        let html = "<html><head><title>Hello &amp; welcome</title></head><body><h1>Heading</h1><p>One <b>two</b></p><img src=\"pic.png\"></body></html>";
        let o = finish(Format::Html, from_html(html.as_bytes())).unwrap();
        assert_eq!(texts(&o.doc)[..2], ["Heading", "One two"]);
        assert!(o.report.has("pictures-linked"));
        let mut doc = o.doc;
        let bytes = doc.save().unwrap();
        let pkg = crate::package::Package::open(&bytes).unwrap();
        let core = String::from_utf8(pkg.get("docProps/core.xml").unwrap().to_vec()).unwrap();
        assert!(core.contains("Hello &amp; welcome"), "{core}");
    }

    #[test]
    fn windows_1252_html_is_read() {
        let o = finish(Format::Html, from_html(b"<p>caf\xe9 \x93quoted\x94</p>")).unwrap();
        assert_eq!(texts(&o.doc)[0], "caf\u{e9} \u{201c}quoted\u{201d}");
    }
}
