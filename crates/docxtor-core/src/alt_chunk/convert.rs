//! From the part an imported chunk points at to a `Fragment`: web pages, web page archives (MHT), plain text, RTF and
//! whole .docx files, each through the reader the clipboard or the file opener already has.

use super::mime;
use crate::fragment::{Builder, Fragment};
use crate::model::Block;
use crate::package::{self, Package};
use crate::xml::Element;
use std::collections::HashMap;

/// What the chunk's part holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Html,
    /// A web page archive: a MIME message with the page and its pictures.
    Mht,
    Text,
    Rtf,
    Docx,
}

impl Kind {
    /// The name the page and the AI tools use.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Html => "html",
            Kind::Mht => "mht",
            Kind::Text => "text",
            Kind::Rtf => "rtf",
            Kind::Docx => "docx",
        }
    }
}

/// The content type the package declares for a part: its override, else the default for its extension.
pub fn content_type_of(pkg: &Package, part: &str) -> Option<String> {
    let doc = pkg.xml("[Content_Types].xml").ok()?;
    let want = format!("/{}", part.trim_start_matches('/'));
    let ext = part.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    let mut default = None;
    for e in doc.root.elements() {
        match e.local() {
            "Override" if e.attr("PartName").is_some_and(|p| p.eq_ignore_ascii_case(&want)) => return e.attr("ContentType").map(String::from),
            "Default" if ext.is_some() && e.attr("Extension").map(|x| x.to_ascii_lowercase()) == ext => default = e.attr("ContentType").map(String::from),
            _ => {}
        }
    }
    default
}

/// What kind of content a chunk part is, from its declared type and, where that settles nothing, its first bytes.
pub fn detect(content_type: &str, part: &str, data: &[u8]) -> Option<Kind> {
    let ct = content_type.to_ascii_lowercase();
    let ext = part.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if data.starts_with(b"PK\x03\x04") || ct.contains("wordprocessingml.document") || ct.contains("wordprocessingml.template") || matches!(ext.as_str(), "docx" | "docm" | "dotx") {
        return data.starts_with(b"PK\x03\x04").then_some(Kind::Docx);
    }
    if data.starts_with(b"{\\rtf") || ct.contains("rtf") || ext == "rtf" {
        return Some(Kind::Rtf);
    }
    if mime::looks_like_mime(data) || ct.contains("message/rfc822") || ct.contains("mimearchive") || matches!(ext.as_str(), "mht" | "mhtml") {
        return Some(Kind::Mht);
    }
    let lower = String::from_utf8_lossy(&data[..data.len().min(2000)]).to_ascii_lowercase();
    let looks_html = lower.contains("<html") || lower.contains("<!doctype html") || lower.contains("<body") || lower.trim_start().starts_with("<p") || lower.trim_start().starts_with("<div");
    if ct.contains("html") || matches!(ext.as_str(), "htm" | "html" | "xhtml") || (ct.is_empty() && looks_html) {
        return Some(Kind::Html);
    }
    if ct.starts_with("text/plain") || ext == "txt" {
        return Some(Kind::Text);
    }
    // An unlabelled part is read as text when it is text (a byte order mark of UTF-16 counts).
    if ct.is_empty() || ct == "application/octet-stream" {
        let wide = data.starts_with(&[0xFF, 0xFE]) || data.starts_with(&[0xFE, 0xFF]);
        return (wide || !data.contains(&0)).then_some(if looks_html { Kind::Html } else { Kind::Text });
    }
    None
}

/// The content of a chunk as a fragment. `owner` is the part holding the `w:altChunk`; `part` the chunk's part.
/// Returns None when the bytes can't be read as the kind they claim.
pub fn to_fragment(pkg: &Package, part: &str, kind: Kind, data: &[u8], budget: &crate::budget::Budget) -> Option<Fragment> {
    let mut frag = match kind {
        Kind::Html => {
            let html = html_text(data);
            let images = images_for(&html, |src| from_part(pkg, part, src));
            crate::html_import::html_page_to_fragment(&html, &images)
        }
        Kind::Mht => {
            let page = mime::page(data)?;
            let images = images_for(&page.html, |src| from_resources(&page.resources, src));
            crate::html_import::html_page_to_fragment(&page.html, &images)
        }
        Kind::Rtf => {
            let src: String = match std::str::from_utf8(data) {
                Ok(s) => s.to_string(),
                Err(_) => data.iter().map(|&b| b as char).collect(),
            };
            crate::rtf::rtf_to_fragment(&src)
        }
        Kind::Text => text_fragment(&plain_text(data)),
        Kind::Docx => {
            // A .docx inside a .docx opens with the same budget, one level deeper; past the limit it stays a placeholder.
            let _inside = budget.enter_document()?;
            let inner = crate::document::Document::open_with(data, budget).ok()?;
            inner.whole_body_fragment()?
        }
    };
    // Nothing a chunk carries is this session's own: links to local files and the like stay out.
    frag.origin.clear();
    Some(frag)
}

fn html_text(data: &[u8]) -> String {
    let charset = mime::meta_charset(data);
    mime::decode_text(data, charset.as_deref())
}

fn plain_text(data: &[u8]) -> String {
    mime::decode_text(data, None)
}

/// A paragraph per line, as Word reads a text chunk.
pub fn text_fragment(text: &str) -> Fragment {
    let mut b = Builder::new();
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.strip_suffix('\n').unwrap_or(&normalized);
    for line in trimmed.split('\n') {
        let mut p = Element::new("w:p");
        if !line.is_empty() {
            let mut r = Element::new("w:r");
            for (i, piece) in line.split('\t').enumerate() {
                if i > 0 {
                    r = r.with_child(Element::new("w:tab"));
                }
                if !piece.is_empty() {
                    let clean: String = piece.chars().filter(|c| !c.is_control() || *c == '\u{0B}').collect();
                    r = r.with_child(Element::new("w:t").with_attr("xml:space", "preserve").with_text(&clean));
                }
            }
            p = p.with_child(r);
        }
        b.block(p);
    }
    b.set_block(true);
    b.finish()
}

/// A picture the page names, looked up among the parts around the chunk: related to it, or beside it.
fn from_part(pkg: &Package, part: &str, src: &str) -> Option<Vec<u8>> {
    if src.starts_with("http:") || src.starts_with("https:") {
        return None;
    }
    let rels = pkg.rels_for(part).unwrap_or_default();
    let wanted = src.strip_prefix("cid:").unwrap_or(src);
    let name = mime::basename(wanted);
    for r in rels.iter().filter(|r| !r.external) {
        let target = package::resolve_target(part, &r.target);
        let same = r.id == wanted || r.target == wanted || mime::percent_decode(&r.target) == mime::percent_decode(wanted) || target.eq_ignore_ascii_case(&package::resolve_target(part, wanted));
        if same || mime::basename(&r.target).eq_ignore_ascii_case(&name) {
            if let Some(d) = pkg.get(&target) {
                return Some(d.to_vec());
            }
        }
    }
    // A file next to the chunk's part.
    if !src.contains(':') {
        let near = package::resolve_target(part, &mime::percent_decode(src));
        if let Some(d) = pkg.get(&near) {
            return Some(d.to_vec());
        }
    }
    None
}

fn from_resources(res: &HashMap<String, Vec<u8>>, src: &str) -> Option<Vec<u8>> {
    res.get(src).or_else(|| res.get(&mime::percent_decode(src))).or_else(|| res.get(&mime::basename(src))).cloned()
}

/// The pictures an HTML page names (other than `data:` addresses) found by `lookup`, keyed by the address as written.
fn images_for(html: &str, lookup: impl Fn(&str) -> Option<Vec<u8>>) -> HashMap<String, Vec<u8>> {
    let mut out = HashMap::new();
    for src in crate::html_import::html_image_sources(html) {
        if let Some(d) = lookup(&src) {
            out.insert(src, d);
        }
    }
    out
}

impl crate::document::Document {
    /// Everything in the body as one fragment: whole blocks with the styles, lists and pictures they use.
    pub(crate) fn whole_body_fragment(&self) -> Option<Fragment> {
        fn collect<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut Vec<Element>, chunks: &mut Vec<(usize, usize, crate::model::NodeId)>) {
            for b in blocks {
                match b {
                    Block::Para(p) => out.push(crate::model::write_paragraph(p)),
                    Block::Table(t) => out.push(crate::model::write_table(t)),
                    // A chunk of this document (a .docx chunk with chunks of its own) is the chunk's content,
                    // gathered against the chunk's preview document, not this one.
                    Block::Container(c) if crate::model::is_alt_chunk(c) => {
                        let first = out.len();
                        collect(&c.blocks, out, &mut Vec::new());
                        if out.len() > first {
                            chunks.push((first, out.len(), c.id));
                        }
                    }
                    Block::Container(c) => collect(&c.blocks, out, chunks),
                    Block::Opaque(_) => {}
                }
            }
        }
        let mut els = Vec::new();
        let mut chunks = Vec::new();
        collect(&self.body, &mut els, &mut chunks);
        if els.is_empty() {
            return None;
        }
        Some(self.build_fragment_of_chunks(els, &chunks, &self.main_path.clone(), true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_by_type_and_by_look() {
        assert_eq!(detect("text/html", "word/afchunk.htm", b"<html></html>"), Some(Kind::Html));
        assert_eq!(detect("message/rfc822", "word/afchunk.mht", b"MIME-Version: 1.0\r\n"), Some(Kind::Mht));
        assert_eq!(detect("text/html", "word/a.htm", b"MIME-Version: 1.0\r\nContent-Type: multipart/related"), Some(Kind::Mht));
        assert_eq!(detect("text/plain", "word/a.txt", b"hello"), Some(Kind::Text));
        assert_eq!(detect("application/rtf", "word/a.rtf", b"{\\rtf1 hi}"), Some(Kind::Rtf));
        assert_eq!(detect("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "word/c.docx", b"PK\x03\x04"), Some(Kind::Docx));
        assert_eq!(detect("", "word/odd.bin", b"<p>hi</p>"), Some(Kind::Html));
        assert_eq!(detect("image/png", "word/x.png", b"\x89PNG"), None);
    }

    #[test]
    fn text_makes_a_paragraph_per_line() {
        let f = text_fragment("one\r\ntwo\tthree\r\n\r\nfour\r\n");
        assert_eq!(f.body.matches("<w:p>").count() + f.body.matches("<w:p/>").count(), 4, "{}", f.body);
        assert!(f.body.contains("<w:tab/>"));
        assert_eq!(f.plain_text(), "one\ntwo\tthree\n\nfour");
    }
}
