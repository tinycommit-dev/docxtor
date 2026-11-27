//! Web page, EPUB, OpenDocument and Rich Text exports: well-formed output, the known text, pictures, tables,
//! lists, notes and links, and a document that is the same afterwards.

use docxtor_core::export::{epub::to_epub, html::to_html, odt::to_odt, rtf::to_rtf};
use docxtor_core::xml;
use docxtor_core::Document;
use serde_json::{json, Value};
use std::io::Read;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}; run python3 fixtures/make_fixtures.py"))
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

/// A document with a bit of everything: headings, nested and numbered lists, a table, a link, an equation,
/// a picture, a footnote, an endnote, a chart, a bookmark and a link to it.
fn everything() -> Document {
    let md = "# Chapter one\n\nIntro with **bold**, *italic* and a [link](https://example.com/a?x=1&y=2). Equation $E=mc^2$ here.\n\n- First\n  - Nested\n- Second\n\n1. One\n2. Two\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nA note here and an endnote there.\n\nPicture: X\n\nChart: Y\n\n## Section\n\nA bookmark target.\n\n# Chapter two\n\nSecond chapter text <script>alert(1)</script> & more.";
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    let p = find_p(&doc, "A note here");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":6},"text":"The footnote text."}));
    let p = find_p(&doc, "A note here");
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":20},"text":"The endnote text."}));
    let p = find_p(&doc, "Picture:");
    let rich = Document::open(&fixture("rich.docx")).unwrap();
    let png = rich.media("word/media/image1.png").expect("fixture picture").to_vec();
    let media = doc.add_media("png", png);
    let p2 = find_p(&doc, "Picture:");
    ops(&mut doc, json!({"op":"insert_image","at":{"p":p2,"o":9},"media":media,"width":96,"height":48,"alt":"A small picture"}));
    let _ = p;
    let c = find_p(&doc, "Chart:");
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":c,"o":0},"kind":"column","title":"Sales","categories":["Q1","Q2"],"series":[{"name":"North","values":[3,5]}]}));
    let b = find_p(&doc, "A bookmark target");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":b,"o":2},"to":{"p":b,"o":10},"name":"Target"}));
    let i = find_p(&doc, "Intro with");
    ops(&mut doc, json!({"op":"set_link","from":{"p":i,"o":0},"to":{"p":i,"o":5},"url":"#Target"}));
    doc
}

fn zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>, bool)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip file");
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            (f.name().unwrap().to_string(), data, f.compression() == zip::CompressionMethod::Stored)
        })
        .collect()
}

fn entry<'a>(entries: &'a [(String, Vec<u8>, bool)], name: &str) -> &'a [u8] {
    &entries.iter().find(|e| e.0 == name).unwrap_or_else(|| panic!("no {name} in the zip")).1
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("UTF-8")
}

fn well_formed(name: &str, bytes: &[u8]) -> xml::XmlDoc {
    let doc = xml::parse(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{name}: {e}"));
    doc
}

fn all_fixtures() -> Vec<(String, Vec<u8>)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut out = Vec::new();
    for d in [dir.clone(), dir.join("textutil")] {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let path = e.path();
            if path.extension().is_some_and(|x| x == "docx") {
                out.push((path.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&path).unwrap()));
            }
        }
    }
    assert!(out.len() >= 4);
    out
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

#[test]
fn html_has_the_text_the_structure_and_the_picture() {
    let doc = Document::open(&fixture("rich.docx")).unwrap();
    let html = to_html(&doc);
    assert!(html.starts_with("<!doctype html>"), "{}", &html[..40]);
    assert!(html.contains("<html lang=\"en-US\">"));
    assert!(html.contains("<title>Docxtor fixture</title>"));
    well_formed("rich.html", html.as_bytes());
    for needle in ["Quarterly Product Report", "Revenue grew 18% year over year", "Launch AI panel", "Final paragraph on page two."] {
        assert!(html.contains(needle), "{needle}");
    }
    assert!(html.contains("<h1 class=\"s-Heading1\" id=\"sec-1\">Highlights</h1>"));
    assert!(html.contains("<h2 "));
    assert!(html.contains("<ul ") && html.contains("<ol "), "lists");
    assert!(html.contains("<table ") && html.contains("<colgroup>") && html.contains("colspan=\"2\""));
    assert!(html.contains("<img src=\"data:image/png;base64,iVBOR"), "the picture is inline");
    assert!(html.contains("<a href=\"https://example.com/report\">the dashboard</a>"));
    assert!(html.contains("<strong>bold progress</strong>") && html.contains("<em>italic caveats</em>"));
    assert!(html.contains("&amp; &lt;angle&gt;"), "text is escaped");
    assert!(html.contains("font-family:'Georgia',serif") || html.contains("font-family:'Georgia'"), "direct formatting becomes a class");
}

#[test]
fn html_has_notes_equations_charts_and_safe_links() {
    let doc = everything();
    let html = to_html(&doc);
    well_formed("everything.html", html.as_bytes());
    assert!(html.contains("<math xmlns=\"http://www.w3.org/1998/Math/MathML\""), "equation as MathML");
    assert!(html.contains("role=\"doc-noteref\"") && html.contains("role=\"doc-footnotes\"") && html.contains("The footnote text."));
    assert!(html.contains("role=\"doc-endnotes\"") && html.contains("The endnote text."));
    assert!(html.contains("role=\"doc-backlink\""), "notes link back");
    assert!(html.contains("<figure class=\"chart\"") && html.contains("<figcaption>Sales</figcaption>") && html.contains("North"));
    assert!(html.contains("alt=\"A small picture\""));
    assert!(html.contains("<a href=\"https://example.com/a?x=1&amp;y=2\"><span class=\"c1\">link</span></a>"), "links keep the link style");
    assert!(html.contains("href=\"#bm-Target\"") && html.contains("id=\"bm-Target\""), "bookmark and link to it");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; more."), "markup in text is escaped");
    assert!(!html.contains("<script"));
}

#[test]
fn html_drops_links_that_run_code() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"Click here now"}));
    let p = find_p(&doc, "Click here");
    ops(&mut doc, json!({"op":"set_link","from":{"p":p,"o":0},"to":{"p":p,"o":5},"url":"javascript:alert(1)"}));
    let html = to_html(&doc);
    assert!(html.contains("here now") && !html.contains("javascript") && !html.contains("<a "), "{html}");
}

// ---------------------------------------------------------------------------
// EPUB
// ---------------------------------------------------------------------------

#[test]
fn epub_is_a_valid_package() {
    let doc = everything();
    let bytes = to_epub(&doc, Some("2026-10-13T09:30:00Z")).unwrap();
    let z = zip_entries(&bytes);
    assert_eq!(z[0].0, "mimetype", "mimetype is first");
    assert!(z[0].2, "mimetype is stored");
    assert_eq!(z[0].1, b"application/epub+zip");
    let container = well_formed("container.xml", entry(&z, "META-INF/container.xml"));
    let root = container.root.find("rootfile").expect("rootfile");
    assert_eq!(root.attr("full-path"), Some("EPUB/package.opf"));
    let opf_text = text_of(entry(&z, "EPUB/package.opf"));
    let opf = well_formed("package.opf", opf_text.as_bytes());
    assert_eq!(opf.root.attr("version"), Some("3.0"));
    for needle in ["<dc:title>", "<dc:language>en-US</dc:language>", "<dc:identifier id=\"book-id\">urn:uuid:", "dcterms:modified\">2026-10-13T09:30:00Z<"] {
        assert!(opf_text.contains(needle), "{needle}\n{opf_text}");
    }
    // Every manifest item exists, every spine item is in the manifest, exactly one item is the navigation document.
    let manifest = opf.root.child("manifest").unwrap();
    let mut ids = Vec::new();
    let mut navs = 0;
    for item in manifest.elements() {
        let href = item.attr("href").unwrap();
        assert!(z.iter().any(|e| e.0 == format!("EPUB/{href}")), "manifest names missing {href}");
        ids.push(item.attr("id").unwrap().to_string());
        if item.attr("properties").is_some_and(|p| p.split(' ').any(|x| x == "nav")) {
            navs += 1;
        }
    }
    assert_eq!(navs, 1);
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "manifest ids are unique");
    let spine: Vec<&str> = opf.root.child("spine").unwrap().elements().map(|i| i.attr("idref").unwrap()).collect();
    assert!(!spine.is_empty());
    assert!(spine.iter().all(|s| ids.iter().any(|i| i == s)));
    // Two Heading 1s, so two chapters.
    assert_eq!(spine.len(), 2, "{spine:?}");
    assert!(opf_text.contains("properties=\"mathml\""), "the chapter with the equation says so");
    // Every file that is XML is well-formed; every picture is a manifest item.
    for (name, data, _) in &z {
        if name.ends_with(".xhtml") || name.ends_with(".opf") || name.ends_with(".xml") {
            well_formed(name, data);
        }
    }
    assert!(z.iter().any(|e| e.0 == "EPUB/images/img1.png"));
    assert!(opf_text.contains("href=\"images/img1.png\" media-type=\"image/png\""));
    // Navigation lists the headings, nested, pointing into the chapter files.
    let nav = text_of(entry(&z, "EPUB/nav.xhtml"));
    assert!(nav.contains("epub:type=\"toc\""));
    assert!(nav.contains("<a href=\"ch01.xhtml#sec-1\">Chapter one</a>"), "{nav}");
    assert!(nav.contains("Section") && nav.contains("<a href=\"ch02.xhtml#"), "{nav}");
    // Chapters.
    let ch1 = text_of(entry(&z, "EPUB/ch01.xhtml"));
    let ch2 = text_of(entry(&z, "EPUB/ch02.xhtml"));
    assert!(ch1.starts_with("<?xml") && ch1.contains("<!DOCTYPE html>") && ch1.contains("xmlns:epub="));
    assert!(ch1.contains("<img src=\"images/img1.png\" alt=\"A small picture\""));
    assert!(ch1.contains("The footnote text.") && ch1.contains("epub:type=\"noteref\"") && ch1.contains("epub:type=\"footnotes\""));
    assert!(ch1.contains("<math xmlns="));
    assert!(ch2.contains("Second chapter text") && !ch2.contains("The footnote text."));
    // The link to the bookmark names the file that holds it.
    assert!(ch1.contains("href=\"ch01.xhtml#bm-Target\""), "{ch1}");
    assert!(!ch1.contains("&nbsp;"));
}

#[test]
fn epub_without_headings_is_one_chapter() {
    let doc = Document::open(&fixture("simple.docx")).unwrap();
    let z = zip_entries(&to_epub(&doc, None).unwrap());
    assert!(z.iter().any(|e| e.0 == "EPUB/ch01.xhtml") && !z.iter().any(|e| e.0 == "EPUB/ch02.xhtml"));
    let nav = text_of(entry(&z, "EPUB/nav.xhtml"));
    assert!(nav.contains("<a href=\"ch01.xhtml\">"), "one entry so the contents is never empty: {nav}");
}

// ---------------------------------------------------------------------------
// OpenDocument
// ---------------------------------------------------------------------------

#[test]
fn odt_is_a_valid_package_with_the_content() {
    let doc = everything();
    let bytes = to_odt(&doc).unwrap();
    let z = zip_entries(&bytes);
    assert_eq!(z[0].0, "mimetype");
    assert!(z[0].2, "mimetype is stored");
    assert_eq!(z[0].1, b"application/vnd.oasis.opendocument.text");
    for name in ["META-INF/manifest.xml", "content.xml", "styles.xml", "meta.xml"] {
        well_formed(name, entry(&z, name));
    }
    let manifest = text_of(entry(&z, "META-INF/manifest.xml"));
    for (name, _, _) in &z {
        if name != "mimetype" && name != "META-INF/manifest.xml" {
            assert!(manifest.contains(&format!("manifest:full-path=\"{name}\"")), "{name} is in the manifest");
        }
    }
    assert!(manifest.contains("manifest:media-type=\"image/png\""));
    let content = text_of(entry(&z, "content.xml"));
    for needle in [
        "Chapter one",
        "<text:h ",
        "text:outline-level=\"1\"",
        "<text:list ",
        "<text:list-item",
        "<table:table ",
        "<table:table-column ",
        "<text:note text:id=\"ftn1\" text:note-class=\"footnote\">",
        "The footnote text.",
        "text:note-class=\"endnote\"",
        "<draw:image xlink:href=\"Pictures/image1.png\"",
        "<svg:desc>A small picture</svg:desc>",
        "<text:a xlink:type=\"simple\" xlink:href=\"https://example.com/a?x=1&amp;y=2\">",
        "<text:bookmark text:name=\"Target\"/>",
        "$E=mc^2$",
        "[Chart: Sales]",
    ] {
        assert!(content.contains(needle), "{needle}");
    }
    let styles = text_of(entry(&z, "styles.xml"));
    assert!(styles.contains("fo:page-width=\"612pt\"") && styles.contains("style:name=\"Heading_20_1\"") && styles.contains("style:default-outline-level=\"1\""));
    assert!(styles.contains("style:name=\"Standard\""));
    let meta = text_of(entry(&z, "meta.xml"));
    assert!(meta.contains("<dc:language>en-US</dc:language>") && meta.contains("<meta:generator>Docxtor</meta:generator>"));
    // Styles named in the body exist in one of the style parts.
    let all = format!("{content}{styles}");
    let mut rest = content.as_str();
    while let Some(i) = rest.find("text:style-name=\"") {
        rest = &rest[i + 17..];
        let name = &rest[..rest.find('"').unwrap()];
        assert!(all.contains(&format!("style:name=\"{name}\"")) || name == "Standard", "style {name} is defined");
    }
}

#[test]
fn odt_tables_keep_widths_borders_and_merges() {
    let doc = Document::open(&fixture("rich.docx")).unwrap();
    let z = zip_entries(&to_odt(&doc).unwrap());
    let content = text_of(entry(&z, "content.xml"));
    assert!(content.contains("style:column-width=\"144pt\""));
    assert!(content.contains("fo:border-top=\"0.5pt solid #000000\""));
    assert!(content.contains("table:number-columns-spanned=\"2\"") && content.contains("<table:covered-table-cell/>"));
    // The header and footer of the first section are in the master page.
    let styles = text_of(entry(&z, "styles.xml"));
    assert!(styles.contains("<style:header>") && styles.contains("<style:footer>") && styles.contains("<text:page-number"));
}

// ---------------------------------------------------------------------------
// RTF
// ---------------------------------------------------------------------------

/// Braces balance outside escapes (`\{`, `\}`, and the bytes of `\'hh`).
fn braces_balance(rtf: &str) -> bool {
    let b = rtf.as_bytes();
    let (mut depth, mut i) = (0i64, 0);
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    depth == 0
}

#[test]
fn rtf_has_the_header_the_text_and_the_parts() {
    let doc = everything();
    let rtf = to_rtf(&doc);
    assert!(rtf.starts_with("{\\rtf1\\ansi"), "{}", &rtf[..30]);
    assert!(braces_balance(&rtf), "braces balance");
    assert!(rtf.is_ascii(), "plain ASCII");
    for needle in ["{\\fonttbl", "{\\colortbl;", "{\\stylesheet", "\\paperw12240\\paperh15840", "{\\*\\listtable", "{\\*\\listoverridetable", "\\trowd", "\\cell", "\\row", "\\pngblip", "{\\footnote", "{\\footnote\\ftnalt", "The footnote text.", "HYPERLINK \"https://example.com/a?x=1&y=2\"", "HYPERLINK \\\\l \"Target\"", "{\\*\\bkmkstart Target}", "Chapter one", "E=mc^2", "[Chart: Sales]", "\\outlinelevel0", "\\b "] {
        assert!(rtf.contains(needle), "{needle}");
    }
    let rich = to_rtf(&Document::open(&fixture("rich.docx")).unwrap());
    assert!(braces_balance(&rich) && rich.is_ascii());
    assert!(rich.contains("\\u8212?"), "an em dash is a Unicode escape");
    assert!(rich.contains("caf\\'e9"), "Latin-1 is a hex escape");
    assert!(rich.contains("\\u-10179?\\u-8704?"), "characters beyond the BMP are surrogate pairs");
    assert!(rich.contains("{\\header ") && rich.contains("{\\footer ") && rich.contains("{\\*\\fldinst PAGE}"));
    assert!(rich.contains("\\clmgf") && rich.contains("\\clmrg"), "merged cells");
}

// ---------------------------------------------------------------------------
// All formats
// ---------------------------------------------------------------------------

#[test]
fn exporting_does_not_change_the_document() {
    for (name, bytes) in all_fixtures() {
        let mut doc = Document::open(&bytes).unwrap();
        let before = doc.save().unwrap();
        let _ = (to_html(&doc), to_epub(&doc, None).unwrap(), to_odt(&doc).unwrap(), to_rtf(&doc));
        assert_eq!(doc.save().unwrap(), before, "{name}");
        assert!(!doc.is_modified(), "{name}");
        assert!(!doc.can_undo(), "{name}");
    }
    let mut doc = everything();
    let before = doc.save().unwrap();
    let _ = (to_html(&doc), to_epub(&doc, None).unwrap(), to_odt(&doc).unwrap(), to_rtf(&doc));
    assert_eq!(doc.save().unwrap(), before);
}

#[test]
fn every_fixture_exports_in_every_format() {
    for (name, bytes) in all_fixtures() {
        let doc = Document::open(&bytes).unwrap();
        let html = to_html(&doc);
        well_formed(&format!("{name}.html"), html.as_bytes());
        let z = zip_entries(&to_epub(&doc, None).unwrap());
        for (n, data, _) in &z {
            if n.ends_with(".xhtml") || n.ends_with(".opf") {
                well_formed(&format!("{name}:{n}"), data);
            }
        }
        let z = zip_entries(&to_odt(&doc).unwrap());
        for n in ["content.xml", "styles.xml", "meta.xml", "META-INF/manifest.xml"] {
            well_formed(&format!("{name}:{n}"), entry(&z, n));
        }
        let rtf = to_rtf(&doc);
        assert!(braces_balance(&rtf) && rtf.is_ascii() && rtf.starts_with("{\\rtf1"), "{name}");
        // The words of the document are in all of them.
        let plain = docxtor_core::export::to_text(&doc);
        let word = plain.split_whitespace().find(|w| w.len() > 5 && w.chars().all(|c| c.is_ascii_alphabetic()));
        if let Some(w) = word {
            assert!(html.contains(w) && to_rtf(&doc).contains(w), "{name}: {w}");
        }
    }
}

#[test]
fn a_blank_document_exports() {
    let doc = Document::new_blank();
    assert!(to_html(&doc).contains("<title>Untitled</title>"));
    assert!(!zip_entries(&to_epub(&doc, None).unwrap()).is_empty());
    let z = zip_entries(&to_odt(&doc).unwrap());
    assert!(text_of(entry(&z, "content.xml")).contains("<office:text>"));
    assert!(braces_balance(&to_rtf(&doc)));
}

#[test]
fn text_with_characters_xml_cannot_hold_is_cleaned() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"a\u{1}b  c\u{FFFE}d \"q\" 'r'"}));
    let html = to_html(&doc);
    well_formed("ctl.html", html.as_bytes());
    assert!(html.contains("ab  c") || html.contains("ab  cd"), "{html}");
    let z = zip_entries(&to_odt(&doc).unwrap());
    let content = text_of(entry(&z, "content.xml"));
    well_formed("content.xml", content.as_bytes());
    assert!(content.contains("<text:s text:c=\"2\"/>"), "runs of spaces are text:s: {content}");
}

/// A document whose body is the given XML.
fn docx_from_body(body: &str) -> Document {
    let mut pkg = docxtor_core::package::Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

const UNUSUAL: &str = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Top</w:t></w:r></w:p>
<w:p><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="8000"/></w:tabs><w:jc w:val="both"/><w:ind w:left="720" w:hanging="360"/></w:pPr><w:r><w:t>Left</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>Right</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>After the break</w:t></w:r></w:p>
<w:p><w:pPr><w:bidi/></w:pPr><w:r><w:rPr><w:rtl/></w:rPr><w:t>שלום</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:jc w:val="center"/></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/></w:tblGrid>
<w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/><w:vMerge w:val="restart"/><w:shd w:val="clear" w:fill="FFFF00"/></w:tcPr><w:p><w:r><w:t>Tall</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Top right</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/><w:vMerge/></w:tcPr><w:p/></w:tc><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Nested cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/></w:tc></w:tr></w:tbl>
<w:p><w:r><w:t>End</w:t></w:r></w:p>"#;

#[test]
fn unusual_constructs_come_out_in_every_format() {
    let doc = docx_from_body(UNUSUAL);
    let html = to_html(&doc);
    well_formed("unusual.html", html.as_bytes());
    for needle in ["rowspan=\"2\"", "<thead>", "width:100%", "margin-left:auto", "direction:rtl", "text-align:justify", "class=\"page-break\"", "background-color:#FFFF00", "Nested cell", "שלום"] {
        assert!(html.contains(needle), "html: {needle}");
    }
    assert_eq!(html.matches("<table").count(), 2, "the nested table stays nested");
    let z = zip_entries(&to_odt(&doc).unwrap());
    let content = text_of(entry(&z, "content.xml"));
    let styles = text_of(entry(&z, "styles.xml"));
    well_formed("content.xml", content.as_bytes());
    for needle in ["table:number-rows-spanned=\"2\"", "<table:covered-table-cell/>", "fo:break-before=\"page\"", "style:leader-style=\"dotted\"", "style:position=\"400pt\" style:type=\"right\"", "style:writing-mode=\"rl-tb\"", "<table:table-header-rows>", "fo:background-color=\"#FFFF00\"", "<text:tab/>", "Nested cell"] {
        assert!(content.contains(needle), "odt: {needle}");
    }
    // Landscape page.
    assert!(styles.contains("fo:page-width=\"792pt\"") && styles.contains("style:print-orientation=\"landscape\""));
    let rtf = to_rtf(&doc);
    assert!(braces_balance(&rtf));
    for needle in ["\\clvmgf", "\\clvmrg", "\\trhdr", "\\rtlpar", "\\qj", "\\tqr\\tldot\\tx8000", "\\page ", "\\landscape", "\\paperw15840\\paperh12240", "\\clcbpat", "Nested cell", "\\u1513?"] {
        assert!(rtf.contains(needle), "rtf: {needle}");
    }
}

#[test]
fn text_boxes_and_shapes_come_out_as_boxes_or_placeholders() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"Before after"}));
    let p = find_p(&doc, "Before");
    ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":7},"w":144,"h":60,"text":"Inside the box","floating":false}));
    let html = to_html(&doc);
    well_formed("box.html", html.as_bytes());
    assert!(html.contains("<aside class=\"textbox\">") && html.contains("Inside the box"));
    let z = zip_entries(&to_odt(&doc).unwrap());
    let content = text_of(entry(&z, "content.xml"));
    assert!(content.contains("<draw:text-box>") && content.contains("Inside the box"), "{content}");
    assert!(to_rtf(&doc).contains("[Inside the box]"));
}

#[test]
fn line_spacing_survives_save_as_html_and_open() {
    for line in [240, 276, 360, 480] {
        let mut doc = Document::new_blank();
        let first = doc.render_all().order[0];
        doc.apply_json(&serde_json::json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Spaced"}).to_string(), false).unwrap();
        doc.apply_json(&serde_json::json!({"op":"format_paragraphs","ids":[first],"format":{"line":line,"lineRule":"auto"}}).to_string(), false).unwrap();
        let html = to_html(&doc);
        let mut again = docxtor_core::open::open(html.as_bytes(), "page.html").unwrap().doc;
        let v = serde_json::to_value(again.render_all()).unwrap();
        let got = v["blocks"].as_array().unwrap().iter().find(|b| b["segs"][0]["x"] == "Spaced").unwrap()["fmt"]["line"].as_i64().unwrap();
        assert_eq!(got, line, "{line} comes back as {got}: {html}");
    }
}
