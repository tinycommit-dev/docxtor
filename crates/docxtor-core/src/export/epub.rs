//! EPUB 3 export: a zip with the `mimetype` file first and stored, `META-INF/container.xml`, a package
//! document (title, author, language, identifier, modified date), a navigation document built from the
//! headings, one XHTML chapter for each Heading 1 (the web page writer does the bodies), the pictures as
//! files, and a stylesheet.

use super::common::*;
use super::html::{Mode, Writer};
use crate::document::Document;
use crate::render::RBlock;
use std::fmt::Write as _;

struct Chapter<'a> {
    file: String,
    blocks: Vec<&'a RBlock>,
    title: Option<String>,
}

struct NavEntry {
    level: u8,
    href: String,
    text: String,
}

struct NavNode<'a> {
    entry: &'a NavEntry,
    children: Vec<NavNode<'a>>,
}

/// The document as an EPUB 3 book. `modified` is the time to record (`2026-10-13T09:30:00Z`); without it
/// the document's own modified date is used.
pub fn to_epub(doc: &Document, modified: Option<&str>) -> Result<Vec<u8>, String> {
    let src = Source::new(doc);
    let blocks = trim_end(&src.blocks);

    // Chapters begin at each Heading 1 among the top-level blocks.
    let mut chapters: Vec<Chapter> = Vec::new();
    for b in blocks {
        let starts = matches!(b, RBlock::Para(p) if heading_level(p) == Some(1));
        if starts || chapters.is_empty() {
            let n = chapters.len() + 1;
            let title = match b {
                RBlock::Para(p) if starts => Some(plain_text(p).trim().to_string()).filter(|t| !t.is_empty()),
                _ => None,
            };
            chapters.push(Chapter { file: format!("ch{n:02}.xhtml"), blocks: Vec::new(), title });
        }
        chapters.last_mut().unwrap().blocks.push(b);
    }
    if chapters.is_empty() {
        chapters.push(Chapter { file: "ch01.xhtml".into(), blocks: Vec::new(), title: None });
    }

    let mut w = Writer::new(&src, Mode::Epub);
    for ch in &chapters {
        for b in &ch.blocks {
            collect_bookmarks(b, &ch.file, &mut w.anchors);
        }
    }

    let mut entries: Vec<NavEntry> = Vec::new();
    let mut files: Vec<ZipEntry> = Vec::new();
    let mut math_files: Vec<bool> = Vec::new();
    for (i, ch) in chapters.iter().enumerate() {
        w.begin_page();
        let first_heading = w.headings.len();
        let mut body = String::new();
        for b in &ch.blocks {
            w.blocks(std::slice::from_ref(*b), &mut body);
        }
        body.push_str(&w.take_notes());
        let has_math = w.take_math();
        let new_headings = &w.headings[first_heading..];
        // A chapter that does not open with a heading (the front of the book) still gets a contents entry.
        let opens_with_heading = new_headings.first().is_some_and(|h| ch.blocks.first().is_some_and(|b| matches!(b, RBlock::Para(p) if heading_level(p).is_some())) && h.level >= 1);
        if i == 0 && !opens_with_heading {
            entries.push(NavEntry { level: 1, href: ch.file.clone(), text: src.title.clone() });
        }
        for h in new_headings {
            let text = if h.text.is_empty() { format!("Section {}", entries.len() + 1) } else { h.text.clone() };
            entries.push(NavEntry { level: h.level, href: format!("{}#{}", ch.file, h.id), text });
        }
        let title = ch.title.clone().unwrap_or_else(|| src.title.clone());
        files.push(ZipEntry::new(format!("EPUB/{}", ch.file), xhtml(&src.lang, &title, &body).into_bytes()));
        math_files.push(has_math);
    }

    // Navigation document.
    let mut pos = 0;
    let tree = parse_nav(&entries, &mut pos, 0);
    let mut nav = String::from("<nav epub:type=\"toc\" id=\"toc\">\n<h1>Contents</h1>\n");
    write_nav(&tree, &mut nav);
    nav.push_str("</nav>\n");
    files.push(ZipEntry::new("EPUB/nav.xhtml", xhtml(&src.lang, "Contents", &nav).into_bytes()));
    files.push(ZipEntry::new("EPUB/style.css", w.stylesheet().into_bytes()));
    for img in &w.images {
        files.push(ZipEntry::new(format!("EPUB/{}", img.path), img.data.clone()));
    }

    // Package document.
    let text_seed: String = std::iter::once(src.title.clone()).chain(blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { Some(plain_text(p)) } else { None })).collect::<Vec<_>>().join("\n");
    let (created, doc_modified) = core_dates(doc);
    let modified = modified.and_then(timestamp).or_else(|| doc_modified.as_deref().and_then(timestamp)).or_else(|| created.as_deref().and_then(timestamp)).unwrap_or_else(|| "2000-01-01T00:00:00Z".into());
    let mut opf = String::new();
    let lang = esc_attr(&src.lang);
    let _ = write!(
        opf,
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\" xml:lang=\"{lang}\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n<dc:identifier id=\"book-id\">urn:uuid:{}</dc:identifier>\n<dc:title>{}</dc:title>\n<dc:language>{lang}</dc:language>\n",
        stable_uuid(&text_seed),
        esc(&src.title)
    );
    if let Some(a) = src.props.author.as_deref().filter(|a| !a.trim().is_empty()) {
        let _ = writeln!(opf, "<dc:creator>{}</dc:creator>", esc(a));
    }
    if let Some(s) = src.props.subject.as_deref().filter(|a| !a.trim().is_empty()) {
        let _ = writeln!(opf, "<dc:subject>{}</dc:subject>", esc(s));
    }
    if let Some(k) = &src.props.keywords {
        for word in k.split([',', ';']).map(str::trim).filter(|w| !w.is_empty()) {
            let _ = writeln!(opf, "<dc:subject>{}</dc:subject>", esc(word));
        }
    }
    if let Some(d) = src.props.description.as_deref().filter(|a| !a.trim().is_empty()) {
        let _ = writeln!(opf, "<dc:description>{}</dc:description>", esc(d));
    }
    let _ = writeln!(opf, "<meta property=\"dcterms:modified\">{modified}</meta>\n</metadata>\n<manifest>");
    opf.push_str("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\" />\n<item id=\"css\" href=\"style.css\" media-type=\"text/css\" />\n");
    for (i, ch) in chapters.iter().enumerate() {
        let props = if math_files[i] { " properties=\"mathml\"" } else { "" };
        let _ = writeln!(opf, "<item id=\"ch{:02}\" href=\"{}\" media-type=\"application/xhtml+xml\"{props} />", i + 1, ch.file);
    }
    for (i, img) in w.images.iter().enumerate() {
        let _ = writeln!(opf, "<item id=\"img{}\" href=\"{}\" media-type=\"{}\" />", i + 1, esc_attr(&img.path), img.mime);
    }
    opf.push_str("</manifest>\n<spine>\n");
    for i in 0..chapters.len() {
        let _ = writeln!(opf, "<itemref idref=\"ch{:02}\" />", i + 1);
    }
    opf.push_str("</spine>\n</package>\n");

    let container = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n<rootfiles>\n<rootfile full-path=\"EPUB/package.opf\" media-type=\"application/oebps-package+xml\" />\n</rootfiles>\n</container>\n";
    let mut entries_zip = vec![ZipEntry::stored("mimetype", "application/epub+zip"), ZipEntry::new("META-INF/container.xml", container), ZipEntry::new("EPUB/package.opf", opf)];
    entries_zip.extend(files);
    write_zip(&entries_zip)
}

fn collect_bookmarks(b: &RBlock, file: &str, out: &mut std::collections::HashMap<String, String>) {
    match b {
        RBlock::Para(p) => {
            for n in &p.bookmarks {
                out.entry(n.clone()).or_insert_with(|| file.to_string());
            }
        }
        RBlock::Table(t) => {
            for c in t.rows.iter().flat_map(|r| &r.cells) {
                for b in &c.blocks {
                    collect_bookmarks(b, file, out);
                }
            }
        }
        RBlock::Container { blocks, .. } => {
            for b in blocks {
                collect_bookmarks(b, file, out);
            }
        }
        RBlock::Opaque { .. } => {}
    }
}

fn xhtml(lang: &str, title: &str, body: &str) -> String {
    let lang = esc_attr(lang);
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{lang}\" xml:lang=\"{lang}\">\n<head>\n<title>{}</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\" />\n</head>\n<body>\n{body}</body>\n</html>\n",
        esc(title)
    )
}

fn parse_nav<'a>(entries: &'a [NavEntry], pos: &mut usize, above: u8) -> Vec<NavNode<'a>> {
    let mut out = Vec::new();
    while *pos < entries.len() && entries[*pos].level > above {
        let entry = &entries[*pos];
        *pos += 1;
        let children = parse_nav(entries, pos, entry.level);
        out.push(NavNode { entry, children });
    }
    out
}

fn write_nav(nodes: &[NavNode], out: &mut String) {
    if nodes.is_empty() {
        return;
    }
    out.push_str("<ol>\n");
    for n in nodes {
        let _ = write!(out, "<li><a href=\"{}\">{}</a>", esc_attr(&n.entry.href), esc(&n.entry.text));
        if !n.children.is_empty() {
            out.push('\n');
            write_nav(&n.children, out);
        }
        out.push_str("</li>\n");
    }
    out.push_str("</ol>\n");
}

