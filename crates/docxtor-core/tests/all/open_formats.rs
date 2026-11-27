//! Opening other formats and damaged files: ODT, RTF, HTML, legacy .doc, Word 2003 XML, Flat OPC, Strict
//! Open XML, repair of broken packages, and clear errors for files that are not Word documents.
//!
//! The .doc, .odt, .rtf, Word 2003 XML and .html samples in fixtures/formats/ were made from sample.html with
//! macOS textutil:
//!   textutil -convert doc|odt|rtf|wordml fixtures/formats/sample.html -output fixtures/formats/sample.<ext>

use docxtor_core::export::{html, odt, rtf};
use docxtor_core::fragment::b64_encode;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::open::{self, Format, Opened};
use docxtor_core::package::Package;
use docxtor_core::{Document, Op, Pos};
use std::io::{Cursor, Write};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

/// Open, save, check the saved file is valid and opens again with the same text.
fn opens(bytes: &[u8], name: &str) -> Opened {
    let mut o = open::open(bytes, name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let saved = o.doc.save().unwrap();
    assert_valid(&saved);
    let again = Document::open(&saved).unwrap();
    assert_eq!(texts(&again), texts(&o.doc), "{name}: text changed on save");
    o
}

fn has(o: &Opened, code: &str) -> bool {
    o.report.has(code)
}

/// A document with a heading, formatted text, a list, a table, a footnote and a header.
fn sample_doc() -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let md = "# Annual report\n\nSome **bold** and *italic* text with a [link](https://example.com).\n\n- One\n- Two\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nClosing words.";
    doc.apply(&Op::InsertMarkdown { after: None, before: Some(docxtor_core::Id(first)), markdown: md.into() }, false).unwrap();
    doc
}

// ---------------------------------------------------------------------------
// Other formats
// ---------------------------------------------------------------------------

#[test]
fn rich_text_written_by_docxtor_opens_again() {
    let src = sample_doc();
    let want: Vec<String> = texts(&src).into_iter().filter(|t| !t.is_empty()).collect();
    let o = opens(rtf::to_rtf(&src).as_bytes(), "x.rtf");
    assert_eq!(o.report.format, Format::Rtf);
    assert!(o.report.converted);
    let got: Vec<String> = texts(&o.doc).into_iter().filter(|t| !t.is_empty()).collect();
    assert_eq!(got, want);
}

#[test]
fn web_page_written_by_docxtor_opens_again() {
    let src = sample_doc();
    let want: Vec<String> = texts(&src).into_iter().filter(|t| !t.is_empty()).collect();
    let o = opens(html::to_html(&src).as_bytes(), "x.html");
    assert_eq!(o.report.format, Format::Html);
    let got: Vec<String> = texts(&o.doc).into_iter().filter(|t| !t.is_empty()).collect();
    for w in &want {
        assert!(got.iter().any(|g| g == w), "missing {w:?} in {got:?}");
    }
}

#[test]
fn opendocument_written_by_docxtor_opens_again() {
    let src = sample_doc();
    let want: Vec<String> = texts(&src).into_iter().filter(|t| !t.is_empty()).collect();
    let bytes = odt::to_odt(&src).unwrap();
    let o = opens(&bytes, "x.odt");
    assert_eq!(o.report.format, Format::Odt);
    let got: Vec<String> = texts(&o.doc).into_iter().filter(|t| !t.is_empty()).collect();
    assert_eq!(got, want);
    // Bold, the heading and the list survive.
    let mut doc = o.doc;
    let saved = doc.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    let body = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(body.contains("<w:b/>") && body.contains("w:numPr") && body.contains("<w:tbl>"), "{body}");
}

#[test]
fn a_textutil_odt_keeps_headings_lists_tables_and_the_page() {
    let o = opens(&fixture("formats/sample.odt"), "sample.odt");
    let t = texts(&o.doc);
    for want in ["Quarterly report", "Highlights", "First point", "Second point", "Name", "Score", "Ann", "90", "Centered line"] {
        assert!(t.iter().any(|x| x == want), "{want} not in {t:?}");
    }
    assert!(t.iter().any(|x| x.starts_with("This is bold, italic, underlined and red text with a link.")));
    let mut doc = o.doc;
    let body = String::from_utf8(Package::open(&doc.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(body.contains("w:tbl>") && body.contains("w:hyperlink") && body.contains(r#"w:jc w:val="center""#), "{body}");
}

#[test]
fn a_textutil_rtf_and_html_open() {
    for (name, file) in [("sample.rtf", "formats/sample.rtf"), ("sample.html", "formats/sample.html")] {
        let o = opens(&fixture(file), name);
        let t = texts(&o.doc);
        assert!(t.iter().any(|x| x == "Quarterly report"), "{name}: {t:?}");
        assert!(t.iter().any(|x| x == "Ann"), "{name}: {t:?}");
        assert!(t.iter().any(|x| x == "Plain closing paragraph."), "{name}: {t:?}");
    }
}

#[test]
fn a_textutil_word_2003_xml_opens() {
    let o = opens(&fixture("formats/sample-word2003.xml"), "sample.xml");
    assert_eq!(o.report.format, Format::Word2003);
    let t = texts(&o.doc);
    assert!(t.iter().any(|x| x == "Quarterly report") && t.iter().any(|x| x == "Plain closing paragraph."), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("This is bold, italic")), "{t:?}");
}

#[test]
fn a_legacy_doc_opens_with_its_text_and_formatting() {
    let o = opens(&fixture("formats/sample.doc"), "sample.doc");
    assert_eq!(o.report.format, Format::Doc);
    assert!(o.report.converted);
    let t = texts(&o.doc);
    for want in ["Quarterly report", "Highlights", "Name", "Score", "Ann", "90", "Bob", "85", "Centered line", "Plain closing paragraph."] {
        assert!(t.iter().any(|x| x == want), "{want} not in {t:?}");
    }
    let mut doc = o.doc;
    let saved = doc.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    let body = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    // Bold, italic, underline and a colour, a table with a grid, centred text.
    assert!(body.contains("<w:b/>") && body.contains("<w:i/>") && body.contains("<w:u ") && body.contains("w:color"), "{body}");
    assert!(body.contains("<w:tbl>") && body.contains("<w:gridCol") && body.contains(r#"w:jc w:val="center""#), "{body}");
    let core = String::from_utf8(pkg.get("docProps/core.xml").unwrap().to_vec()).unwrap();
    assert!(core.contains("Ada Lovelace"), "{core}");
}

#[test]
fn legacy_doc_errors_are_clear() {
    // A password-protected .doc has the encrypted flag in its header: flip it in a copy.
    let mut bytes = fixture("formats/sample.doc");
    let cfb = docxtor_core::cfb::CfbReader::new(&bytes).unwrap();
    let mut wd = cfb.stream("WordDocument").unwrap().unwrap();
    let table1 = cfb.stream("1Table").unwrap();
    let table0 = cfb.stream("0Table").unwrap();
    drop(cfb);
    wd[11] |= 0x01; // fEncrypted (bit 8 of the flags word at 10)
    let mut streams: Vec<(&str, Vec<u8>)> = vec![("WordDocument", wd)];
    if let Some(t) = table1 {
        streams.push(("1Table", t));
    }
    if let Some(t) = table0 {
        streams.push(("0Table", t));
    }
    bytes = docxtor_core::cfb::write(&streams);
    let e = open::open(&bytes, "locked.doc").err().unwrap();
    assert_eq!(e.code, "doc-encrypted");
    // A compound file that is not Word.
    let other = docxtor_core::cfb::write(&[("Workbook", vec![0; 600])]);
    let e = open::open(&other, "book.xls").err().unwrap();
    assert_eq!((e.code, e.detail.as_str()), ("not-word", "spreadsheet"));
}

// ---------------------------------------------------------------------------
// Flat OPC
// ---------------------------------------------------------------------------

fn flat_opc(docx: &[u8]) -> String {
    let pkg = Package::open(docx).unwrap();
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    let ctype = |name: &str| -> String {
        let doc = docxtor_core::xml::parse(ct.as_bytes()).unwrap();
        if let Some(o) = doc.root.elements().find(|e| e.local() == "Override" && e.attr("PartName") == Some(format!("/{name}").as_str())) {
            return o.attr("ContentType").unwrap().to_string();
        }
        let ext = name.rsplit('.').next().unwrap();
        let found = doc.root.elements().find(|e| e.local() == "Default" && e.attr("Extension") == Some(ext)).and_then(|d| d.attr("ContentType")).unwrap_or("application/octet-stream").to_string();
        found
    };
    let mut out = String::from(r#"<?xml version="1.0" standalone="yes"?><?mso-application progid="Word.Document"?><pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">"#);
    for p in &pkg.parts {
        let head = format!(r#"<pkg:part pkg:name="/{}" pkg:contentType="{}""#, p.name, ctype(&p.name));
        if p.name.ends_with(".xml") || p.name.ends_with(".rels") {
            let text = String::from_utf8_lossy(&p.data);
            let body = text.split_once("?>").map(|x| x.1).unwrap_or(&text);
            out.push_str(&format!("{head}><pkg:xmlData>{}</pkg:xmlData></pkg:part>", body.trim()));
        } else {
            out.push_str(&format!(r#"{head} pkg:compression="store"><pkg:binaryData>{}</pkg:binaryData></pkg:part>"#, b64_encode(&p.data)));
        }
    }
    out.push_str("</pkg:package>");
    out
}

#[test]
fn flat_opc_opens_with_everything_in_it() {
    let docx = fixture("rich.docx");
    let flat = flat_opc(&docx);
    let src = Document::open(&docx).unwrap();
    let o = opens(flat.as_bytes(), "rich.xml");
    assert_eq!(o.report.format, Format::FlatOpc);
    assert!(o.report.converted);
    assert_eq!(texts(&o.doc), texts(&src));
    let mut doc = o.doc;
    let pkg = Package::open(&doc.save().unwrap()).unwrap();
    assert!(pkg.parts.iter().any(|p| p.name.starts_with("word/media/")), "the picture is kept");
    assert!(pkg.has("word/header1.xml") && pkg.has("word/footer1.xml"));
}

// ---------------------------------------------------------------------------
// Strict Open XML
// ---------------------------------------------------------------------------

const STRICT_MAP: &[(&str, &str)] = &[
    ("http://schemas.openxmlformats.org/wordprocessingml/2006/main", "http://purl.oclc.org/ooxml/wordprocessingml/main"),
    ("http://schemas.openxmlformats.org/officeDocument/2006/relationships", "http://purl.oclc.org/ooxml/officeDocument/relationships"),
    ("http://schemas.openxmlformats.org/drawingml/2006/main", "http://purl.oclc.org/ooxml/drawingml/main"),
    ("http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing", "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing"),
    ("http://schemas.openxmlformats.org/drawingml/2006/picture", "http://purl.oclc.org/ooxml/drawingml/picture"),
];

/// A copy of a package with the Strict namespaces and spellings.
fn strict(docx: &[u8]) -> Vec<u8> {
    let pkg = Package::open(docx).unwrap();
    let mut out = Package::default();
    for p in &pkg.parts {
        let mut data = p.data.clone();
        if p.name.ends_with(".xml") || p.name.ends_with(".rels") {
            let mut s = String::from_utf8(data).unwrap();
            for (from, to) in STRICT_MAP {
                s = s.replace(from, to);
            }
            if p.name == "word/document.xml" {
                s = s.replace(r#"w:jc w:val="center""#, r#"w:jc w:val="start""#);
            }
            data = s.into_bytes();
        }
        out.parts.push(docxtor_core::package::Part { name: p.name.clone(), data, stored: p.stored });
    }
    out.to_bytes().unwrap()
}

#[test]
fn strict_open_xml_keeps_notes_and_theme_and_saves_as_the_standard_kind() {
    let mut doc = sample_doc();
    let p = doc.render_all().order[1];
    doc.apply(&Op::InsertFootnote { at: Pos::new(p, 4), text: "A footnote in a strict file.".into() }, false).unwrap();
    let plain = doc.save().unwrap();
    let want = texts(&Document::open(&plain).unwrap());

    let bytes = strict(&plain);
    assert!(String::from_utf8_lossy(Package::open(&bytes).unwrap().get("word/document.xml").unwrap()).contains("purl.oclc.org"));
    let o = opens(&bytes, "strict.docx");
    assert!(has(&o, "strict"));
    assert!(!o.report.converted);
    assert_eq!(texts(&o.doc), want);
    // The footnote and the theme are there.
    assert!(o.doc.stories.iter().any(|s| {
        let mut t = String::new();
        walk_paragraphs(&s.blocks, &mut |p| t.push_str(&p.plain_text()));
        t.contains("A footnote in a strict file.")
    }));
    let mut d = o.doc;
    let saved = d.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            assert!(!String::from_utf8_lossy(&part.data).contains("purl.oclc.org"), "{} is still strict", part.name);
        }
    }
    assert!(pkg.has("word/footnotes.xml") && pkg.has("word/theme/theme1.xml"));
    let body = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(!body.contains(r#"w:val="start""#));
}

// ---------------------------------------------------------------------------
// Repair
// ---------------------------------------------------------------------------

fn zip_of(parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, d) in parts {
        w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(d).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn parts_of(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    Package::open(bytes).unwrap().parts.into_iter().map(|p| (p.name, p.data)).collect()
}

/// A long document, so cuts fall inside the text.
fn long_docx() -> Vec<u8> {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let md: String = (1..=120).map(|i| format!("Paragraph number {i} has some words in it so that the file is not tiny.\n\n")).collect();
    doc.apply(&Op::InsertMarkdown { after: None, before: Some(docxtor_core::Id(first)), markdown: md }, false).unwrap();
    doc.save().unwrap()
}

#[test]
fn a_sound_file_gets_no_notes_and_is_unchanged() {
    for f in ["rich.docx", "simple.docx", "complex.docx", "long.docx"] {
        let bytes = fixture(f);
        let o = open::open(&bytes, f).unwrap();
        assert!(o.report.notes.is_empty(), "{f}: {:?}", o.report.notes);
        assert!(!o.report.converted);
    }
}

#[test]
fn a_truncated_file_opens_with_the_text_it_has_and_says_so() {
    let full = long_docx();
    let all = texts(&Document::open(&full).unwrap());
    // Cut the file in the middle of the text part: the end of the text and the central directory are gone.
    let (start, size) = {
        let mut z = zip::ZipArchive::new(Cursor::new(&full)).unwrap();
        let f = z.by_name("word/document.xml").unwrap();
        (f.data_start().unwrap() as usize, f.compressed_size() as usize)
    };
    let cut = &full[..start + size * 2 / 3];
    assert!(Package::open(cut).is_err(), "the cut file is not a valid zip any more");
    let o = open::open(cut, "cut.docx").unwrap();
    assert!(has(&o, "zip-directory"), "{:?}", o.report.notes);
    let t = texts(&o.doc);
    assert!(t.len() > 5 && t.len() < all.len(), "{} of {}", t.len(), all.len());
    assert_eq!(&t[..5], &all[..5]);
    // It saves as a valid file.
    let mut doc = o.doc;
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_file_without_its_central_directory_is_rebuilt() {
    let full = long_docx();
    // Wipe the end-of-central-directory signature.
    let mut broken = full.clone();
    let at = broken.windows(4).rposition(|w| w == [0x50, 0x4B, 0x05, 0x06]).unwrap();
    broken[at] = 0;
    let o = open::open(&broken, "broken.docx").unwrap();
    assert!(has(&o, "zip-directory"));
    assert_eq!(texts(&o.doc), texts(&Document::open(&full).unwrap()));
}

#[test]
fn damaged_xml_keeps_what_parses() {
    let full = long_docx();
    let mut parts = parts_of(&full);
    for (n, d) in parts.iter_mut() {
        if n == "word/document.xml" {
            let s = String::from_utf8(d.clone()).unwrap();
            // Break a tag in the middle and cut the end off.
            let mid = s.find("Paragraph number 60 ").unwrap();
            let broken_at = s[..mid].rfind("<w:r>").unwrap();
            let mut s2 = String::new();
            s2.push_str(&s[..broken_at]);
            s2.push_str("<w:r <<>");
            s2.push_str(&s[broken_at + 5..s.len() - 400]);
            *d = s2.into_bytes();
        }
    }
    let refs: Vec<(&str, Vec<u8>)> = parts.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let bytes = zip_of(&refs);
    let o = open::open(&bytes, "bad.docx").unwrap();
    assert!(has(&o, "xml-recovered") && has(&o, "text-cut"), "{:?}", o.report.notes);
    let t = texts(&o.doc);
    assert!(t.iter().any(|x| x.starts_with("Paragraph number 59 ")), "{}", t.len());
    assert!(t.iter().any(|x| x.starts_with("Paragraph number 100 ")), "{} paragraphs, last: {:?}", t.len(), t.iter().rev().take(3).collect::<Vec<_>>());
}

#[test]
fn missing_relationships_and_content_types_are_put_back() {
    let full = fixture("rich.docx");
    let parts: Vec<(&str, Vec<u8>)> = parts_of(&full).into_iter().filter(|(n, _)| n != "[Content_Types].xml" && n != "_rels/.rels" && n != "word/_rels/document.xml.rels").map(|(n, d)| (Box::leak(n.into_boxed_str()) as &str, d)).collect();
    let bytes = zip_of(&parts);
    let o = open::open(&bytes, "bare.docx").unwrap();
    assert!(has(&o, "rels-restored") && has(&o, "types-restored"), "{:?}", o.report.notes);
    let src = Document::open(&full).unwrap();
    assert_eq!(texts(&o.doc), texts(&src));
    let mut doc = o.doc;
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let pkg = Package::open(&saved).unwrap();
    assert!(pkg.rels_for("word/document.xml").unwrap().iter().any(|r| r.rel_type.ends_with("/styles")));
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(ct.contains("/word/document.xml") && ct.contains("/word/styles.xml"));
}

#[test]
fn a_part_that_cannot_be_read_is_left_as_it_is_and_reported() {
    let full = fixture("rich.docx");
    let mut parts = parts_of(&full);
    for (n, d) in parts.iter_mut() {
        if n == "word/numbering.xml" {
            *d = b"this is not xml at all".to_vec();
        }
    }
    let refs: Vec<(&str, Vec<u8>)> = parts.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let o = open::open(&zip_of(&refs), "bad.docx").unwrap();
    assert!(has(&o, "xml-unreadable"), "{:?}", o.report.notes);
    assert_eq!(o.report.notes.iter().find(|n| n.code == "xml-unreadable").unwrap().detail.as_deref(), Some("word/numbering.xml"));
    // The text is all there; the part is kept, byte for byte, as it was.
    assert_eq!(texts(&o.doc), texts(&Document::open(&full).unwrap()));
    let mut doc = o.doc;
    let saved = doc.save().unwrap();
    assert_eq!(Package::open(&saved).unwrap().get("word/numbering.xml").unwrap(), b"this is not xml at all");
}

#[test]
fn a_relationship_to_a_missing_picture_is_reported_and_kept() {
    let full = fixture("rich.docx");
    let mut parts = parts_of(&full);
    parts.retain(|(n, _)| !n.starts_with("word/media/"));
    let refs: Vec<(&str, Vec<u8>)> = parts.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let o = open::open(&zip_of(&refs), "nopic.docx").unwrap();
    assert!(has(&o, "parts-missing"), "{:?}", o.report.notes);
    let mut doc = o.doc;
    let saved = doc.save().unwrap();
    let pkg = Package::open(&saved).unwrap();
    assert!(pkg.rels_for("word/document.xml").unwrap().iter().any(|r| r.target.contains("media/")));
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

fn error(bytes: &[u8]) -> (String, String) {
    let e = open::open(bytes, "file").err().expect("should not open");
    (e.code.to_string(), e.detail)
}

#[test]
fn files_that_are_not_word_documents_say_what_they_are() {
    assert_eq!(error(b"").0, "empty");
    assert_eq!(error(b"%PDF-1.7 ...").1, "pdf");
    let xlsx = zip_of(&[("[Content_Types].xml", b"<Types><Override ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>".to_vec()), ("xl/workbook.xml", b"<w/>".to_vec())]);
    assert_eq!(error(&xlsx), ("not-word".into(), "spreadsheet".into()));
    let pptx = zip_of(&[("ppt/presentation.xml", b"<p/>".to_vec())]);
    assert_eq!(error(&pptx).1, "presentation");
    let epub = zip_of(&[("mimetype", b"application/epub+zip".to_vec()), ("META-INF/container.xml", b"<c/>".to_vec())]);
    assert_eq!(error(&epub).1, "epub");
    let ods = zip_of(&[("mimetype", b"application/vnd.oasis.opendocument.spreadsheet".to_vec()), ("content.xml", b"<c/>".to_vec())]);
    assert_eq!(error(&ods).1, "spreadsheet");
    assert_eq!(error(&[0x89, b'P', b'N', b'G', 1, 2, 3, 4]).1, "image");
    assert_eq!(error(b"\x00\x01\x02 just some bytes that are nothing").0, "unknown");
    let plain_zip = zip_of(&[("notes.txt", b"hello".to_vec())]);
    assert_eq!(error(&plain_zip), ("not-word".into(), "archive".into()));
    // A password-protected .docx is not this module's job: it says so.
    let enc = fixture("encrypted/encrypted-agile.docx");
    assert_eq!(error(&enc).0, "encrypted");
}

#[test]
fn garbage_that_looks_like_a_zip_is_a_clear_error() {
    let mut z = b"PK\x03\x04".to_vec();
    z.extend_from_slice(&[0xFF; 40]);
    let (code, _) = error(&z);
    assert!(code == "damaged" || code == "not-word", "{code}");
}

#[test]
fn misnamed_files_open_by_what_they_are() {
    // An RTF file called .docx, and the other way round.
    let rtf = fixture("formats/sample.rtf");
    assert_eq!(open::open(&rtf, "report.docx").unwrap().report.format, Format::Rtf);
    let docx = fixture("simple.docx");
    assert_eq!(open::open(&docx, "report.rtf").unwrap().report.format, Format::Docx);
}

#[test]
fn convert_writes_a_docx_and_a_report() {
    let (bytes, report) = open::to_docx(&fixture("formats/sample.odt"), "sample.odt").unwrap();
    assert_eq!(report.format, Format::Odt);
    assert_valid(&bytes);
    let doc = Document::open(&bytes).unwrap();
    assert!(texts(&doc).iter().any(|t| t == "Quarterly report"));
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["format"], "odt");
    assert_eq!(json["converted"], true);
}

/// Found by the Open XML validator (scripts/validate.mjs, on the feature files): Word 2003 XML spells `w:sz-cs`,
/// `w:first-line` and `w:fareast` with hyphens; the converted .docx kept them, and Word 2007 and later has `w:szCs`,
/// `w:firstLine` and `w:eastAsia`.
#[test]
fn word_2003_xml_names_become_the_names_open_xml_has() {
    let (docx, _) = open::to_docx(&fixture("formats/sample-word2003.xml"), "sample.xml").unwrap();
    let pkg = Package::open(&docx).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    for bad in ["w:sz-cs", "w:b-cs", "w:i-cs", "w:first-line", "w:line-rule", "w:fareast"] {
        assert!(!xml.contains(bad), "{bad} is still in the converted document");
    }
    assert!(xml.contains("<w:szCs "), "{xml}");
    // A negative first line is a hanging indent in Open XML.
    assert!(xml.contains("w:hanging=\"720\"") && !xml.contains("w:firstLine=\"-"), "{xml}");
}
