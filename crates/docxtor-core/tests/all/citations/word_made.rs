//! Tests of: A document made by Word.

use super::*;

const SOURCES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?><b:Sources SelectedStyle="\APASixthEditionOfficeOnline.xsl" StyleName="APA" Version="6" xmlns:b="http://schemas.openxmlformats.org/officeDocument/2006/bibliography" xmlns="http://schemas.openxmlformats.org/officeDocument/2006/bibliography"><b:Source><b:Tag>Smi20</b:Tag><b:SourceType>Book</b:SourceType><b:Guid>{7D6F2D14-0B62-4D5B-A7C5-0DF3E8A21A11}</b:Guid><b:LCID>1033</b:LCID><b:Author><b:Author><b:NameList><b:Person><b:Last>Smith</b:Last><b:First>John</b:First></b:Person></b:NameList></b:Author></b:Author><b:Title>The Art of Citing</b:Title><b:Year>2020</b:Year><b:City>New York</b:City><b:Publisher>Penguin</b:Publisher><b:RefOrder>1</b:RefOrder></b:Source><b:Source><b:Tag>Lee21</b:Tag><b:SourceType>JournalArticle</b:SourceType><b:Guid>{0C3A1B6E-55A2-4E39-9C3B-3E2B1D2F8A22}</b:Guid><b:Author><b:Author><b:NameList><b:Person><b:Last>Lee</b:Last><b:First>Karen</b:First></b:Person><b:Person><b:Last>Kim</b:Last><b:First>Min-Jun</b:First></b:Person></b:NameList></b:Author></b:Author><b:Title>A study of things</b:Title><b:JournalName>Journal of Stuff</b:JournalName><b:Year>2021</b:Year><b:Month>March</b:Month><b:Pages>45-67</b:Pages><b:Volume>12</b:Volume><b:Issue>3</b:Issue><b:RefOrder>2</b:RefOrder></b:Source></b:Sources>"#;

pub(super) fn field(instr: &str, result: &str, id: i64) -> String {
    format!(
        r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/><w:citation/></w:sdtPr><w:sdtContent><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>{result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:sdtContent></w:sdt>"#
    )
}

pub(super) fn word_bibliography() -> String {
    r#"<w:sdt><w:sdtPr><w:id w:val="111145805"/><w:docPartObj><w:docPartGallery w:val="Bibliographies"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent><w:p w:rsidR="00B0381B" w:rsidRDefault="00B0381B" w:rsidP="00B0381B"><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>References</w:t></w:r></w:p><w:p w:rsidR="00B0381B" w:rsidRDefault="00B0381B" w:rsidP="00B0381B"><w:pPr><w:pStyle w:val="Bibliography"/><w:ind w:left="720" w:hanging="720"/><w:rPr><w:noProof/></w:rPr></w:pPr><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> BIBLIOGRAPHY </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t xml:space="preserve">Lee, K., &amp; Kim, M.-J. (2021). A study of things. </w:t></w:r><w:r><w:rPr><w:i/><w:iCs/><w:noProof/></w:rPr><w:t>Journal of Stuff</w:t></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>, 12(3), 45-67.</w:t></w:r></w:p><w:p w:rsidR="00B0381B" w:rsidRDefault="00B0381B" w:rsidP="00B0381B"><w:pPr><w:pStyle w:val="Bibliography"/><w:ind w:left="720" w:hanging="720"/><w:rPr><w:noProof/></w:rPr></w:pPr><w:r><w:rPr><w:noProof/></w:rPr><w:t xml:space="preserve">Smith, J. (2020). </w:t></w:r><w:r><w:rPr><w:i/><w:iCs/><w:noProof/></w:rPr><w:t>The Art of Citing</w:t></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>. New York: Penguin.</w:t></w:r></w:p><w:p w:rsidR="00B0381B" w:rsidRDefault="00B0381B" w:rsidP="00B0381B"><w:r><w:rPr><w:b/><w:bCs/><w:noProof/></w:rPr><w:fldChar w:fldCharType="end"/></w:r></w:p></w:sdtContent></w:sdt>"#
        .to_string()
}

/// What Word writes for a document with two sources, a citation of each and a bibliography.
pub(super) fn word_made() -> (Document, Vec<u8>) {
    word_package(&format!(
        r#"<w:p><w:r><w:t xml:space="preserve">Citing is hard </w:t></w:r>{}<w:r><w:t xml:space="preserve"> and more </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>{}"#,
        field("CITATION Smi20 \\l 1033", "(Smith, 2020)", -1234567),
        field("CITATION Lee21 \\m Smi20 \\p 5 \\l 1033", "(Lee &amp; Kim, 2021, p. 5; Smith, 2020)", 98765),
        word_bibliography()
    ))
}

/// A package with Word's two sources and the given body XML.
pub(super) fn word_package(body: &str) -> (Document, Vec<u8>) {
    word_package_with(body, SOURCES_XML)
}

pub(super) fn word_package_with(body: &str, sources: &str) -> (Document, Vec<u8>) {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    // Content types: the properties part.
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    let ct = ct.replace("</Types>", r#"<Override PartName="/customXml/itemProps1.xml" ContentType="application/vnd.openxmlformats-officedocument.customXmlProperties+xml"/></Types>"#);
    pkg.set("[Content_Types].xml", ct.into_bytes());
    pkg.set("customXml/item1.xml", sources.as_bytes().to_vec());
    pkg.set(
        "customXml/itemProps1.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="no"?><ds:datastoreItem ds:itemID="{3B4C1E77-0F2A-4D0E-8E0A-6B1B4D2C9F10}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs><ds:schemaRef ds:uri="http://schemas.openxmlformats.org/officeDocument/2006/bibliography"/></ds:schemaRefs></ds:datastoreItem>"#.to_vec(),
    );
    pkg.add_rel("customXml/item1.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps", "itemProps1.xml", false);
    pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml", "../customXml/item1.xml", false);
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    let bytes = pkg.to_bytes().unwrap();
    (Document::open(&bytes).unwrap(), bytes)
}

#[test]
fn reads_what_word_wrote() {
    let (doc, _) = word_made();
    let s = doc.sources();
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].tag.as_str(), s[0].kind.id(), s[0].title.as_str()), ("Smi20", "book", "The Art of Citing"));
    assert_eq!(s[0].authors, vec![docxtor_core::citations::Name::person("Smith", "John")]);
    assert_eq!((s[0].publisher.as_deref(), s[0].place.as_deref(), s[0].year.as_deref()), (Some("Penguin"), Some("New York"), Some("2020")));
    assert_eq!(s[0].guid.as_deref(), Some("{7D6F2D14-0B62-4D5B-A7C5-0DF3E8A21A11}"));
    assert_eq!(s[1].kind.id(), "article");
    assert_eq!(s[1].container_title.as_deref(), Some("Journal of Stuff"));
    assert_eq!((s[1].volume.as_deref(), s[1].issue.as_deref(), s[1].pages.as_deref(), s[1].month.as_deref()), (Some("12"), Some("3"), Some("45-67"), Some("March")));
    assert_eq!(doc.citation_style(), "apa");
    let c = doc.citations();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].tags, ["Smi20"]);
    assert_eq!((c[0].o, c[0].len), (15, 13), "'Citing is hard ' is 15 characters; '(Smith, 2020)' is 13");
    assert_eq!(c[1].tags, ["Lee21", "Smi20"]);
    assert_eq!(c[1].page.as_deref(), Some("5"));
}

#[test]
fn an_untouched_word_document_saves_byte_for_byte() {
    let (mut doc, bytes) = word_made();
    let out = doc.save().unwrap();
    let (a, b) = (Package::open(&bytes).unwrap(), Package::open(&out).unwrap());
    for p in &a.parts {
        assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
    }
    assert_eq!(a.parts.len(), b.parts.len());
}

#[test]
fn citations_and_bibliography_render_as_marked_units() {
    let (mut doc, _) = word_made();
    let r = doc.render_all();
    let mut cites: Vec<(String, Vec<String>)> = Vec::new();
    let mut label = None;
    for b in &r.blocks {
        match b {
            RBlock::Para(p) => {
                for s in p.segs.iter().filter(|s: &&RSeg| s.cite.is_some()) {
                    cites.push((s.x.clone().unwrap_or_default(), s.cite.clone().unwrap()));
                }
            }
            RBlock::Container { label: l, blocks, .. } => {
                label = l.clone();
                assert!(matches!(blocks[0], RBlock::Para(_)));
            }
            _ => {}
        }
    }
    assert_eq!(cites, [("(Smith, 2020)".to_string(), vec!["Smi20".to_string()]), ("(Lee & Kim, 2021, p. 5; Smith, 2020)".to_string(), vec!["Lee21".to_string(), "Smi20".to_string()])]);
    assert_eq!(label.as_deref(), Some("Bibliography"));
    // Models see the sources, not Word's formatted text.
    let script = docscript(&mut doc, &ScriptOpts::default());
    assert!(script.contains("Citing is hard [@Smi20] and more [@Lee21; @Smi20]."), "{script}");
    assert!(script.contains("container \"Bibliography\""), "{script}");
    assert!(script.contains("Lee, K., & Kim, M.-J. (2021). A study of things. *Journal of Stuff*"), "{script}");
}

#[test]
fn adding_a_source_leaves_every_other_part_alone() {
    let (mut doc, bytes) = word_made();
    ops(&mut doc, json!({"op":"add_source","source":{"type":"book","authors":["Roe, Jane"],"title":"Another book","year":"2019"}}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let (a, b) = (Package::open(&bytes).unwrap(), Package::open(&out).unwrap());
    for p in &a.parts {
        if p.name != "customXml/item1.xml" {
            assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
        }
    }
    let item = part(&out, "customXml/item1.xml");
    // Word's own sources are kept exactly (including the LCID Docxtor doesn't model), the new one follows.
    assert!(item.contains("<b:LCID>1033</b:LCID>"), "{item}");
    assert!(item.contains("<b:Tag>Roe19</b:Tag>"), "{item}");
    assert!(item.contains("<b:RefOrder>3</b:RefOrder>"), "{item}");
    assert!(item.contains(r#"SelectedStyle="\APASixthEditionOfficeOnline.xsl""#), "{item}");
    let again = Document::open(&out).unwrap();
    assert_eq!(again.sources().len(), 3);
    assert_eq!(again.sources()[2].authors, vec![docxtor_core::citations::Name::person("Roe", "Jane")]);
}

#[test]
fn changing_the_style_rewrites_citations_and_the_list_and_nothing_else() {
    let (mut doc, bytes) = word_made();
    ops(&mut doc, json!({"op":"set_citation_style","style":"mla"}));
    assert_eq!(doc.citation_style(), "mla");
    let t = texts(&doc);
    assert_eq!(t[0], "Citing is hard (Smith) and more (Lee and Kim 5; Smith).");
    assert_eq!(&t[1..], ["Works Cited", "Lee, Karen, and Min-Jun Kim. \"A study of things.\" Journal of Stuff, vol. 12, no. 3, Mar. 2021, pp. 45-67.", "Smith, John. The Art of Citing. Penguin, 2020.", ""]);
    let out = doc.save().unwrap();
    assert_valid(&out);
    let (a, b) = (Package::open(&bytes).unwrap(), Package::open(&out).unwrap());
    for p in &a.parts {
        if !matches!(p.name.as_str(), "customXml/item1.xml" | "word/document.xml") {
            assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
        }
    }
    let item = part(&out, "customXml/item1.xml");
    assert!(item.contains(r#"SelectedStyle="\MLASeventhEditionOfficeOnline.xsl""#) && item.contains(r#"StyleName="MLA""#), "{item}");
    assert!(item.contains("<b:LCID>1033</b:LCID>"), "the sources themselves are untouched");
    let xml = part(&out, "word/document.xml");
    // The citation controls and the bibliography field are still Word's structures.
    assert_eq!(xml.matches("<w:citation/>").count(), 2);
    assert!(xml.contains("CITATION Lee21 \\m Smi20 \\p 5 \\l 1033"), "the instruction is kept");
    assert!(xml.contains("w:docPartGallery w:val=\"Bibliographies\""));
    assert_eq!(xml.matches("fldCharType=\"begin\"").count(), 3);
    assert_eq!(xml.matches("fldCharType=\"end\"").count(), 3);
    // The first entry keeps its paragraph (and Word's revision ids); italics follow the style.
    assert!(xml.contains(r#"<w:p w:rsidR="00B0381B" w:rsidRDefault="00B0381B" w:rsidP="00B0381B"><w:pPr><w:pStyle w:val="Bibliography"/>"#), "{xml}");
    assert!(xml.contains("<w:i/><w:iCs/>"), "italic runs in the MLA entries");
}

#[test]
fn every_style_renders_the_word_made_document() {
    let expected = [
        ("apa", "(Smith, 2020)", "References", "Smith, J. (2020). The Art of Citing. Penguin."),
        ("mla", "(Smith)", "Works Cited", "Smith, John. The Art of Citing. Penguin, 2020."),
        ("chicago", "(Smith 2020)", "Bibliography", "Smith, John. 2020. The Art of Citing. New York: Penguin."),
        ("ieee", "[1]", "Bibliography", "[1]\tJ. Smith, The Art of Citing. New York: Penguin, 2020."),
        ("harvard", "(Smith, 2020)", "Bibliography", "Smith, J. (2020) The Art of Citing. New York: Penguin."),
    ];
    for (style, first, heading, entry) in expected {
        let (mut doc, _) = word_made();
        ops(&mut doc, json!({"op":"set_citation_style","style":style}));
        let t = texts(&doc);
        assert!(t[0].contains(first), "{style}: {}", t[0]);
        assert_eq!(t[1], heading, "{style}");
        // Apa sorts Lee before Smith; IEEE numbers by first citation, which is Smi20 in the text, but Lee21 is
        // cited first in its own citation... (Smi20 appears first in the paragraph).
        assert!(t.iter().any(|l| l == entry), "{style}: {t:?}");
        assert_valid(&doc.save().unwrap());
    }
}

#[test]
fn the_word_made_style_survives_a_round_trip_through_other_styles() {
    let (mut doc, _) = word_made();
    let before = texts(&doc);
    ops(&mut doc, json!({"op":"set_citation_style","style":"apa"}));
    // Docxtor's APA 7 differs from Word's APA 6 only in small ways: citations read the same.
    assert_eq!(texts(&doc)[0], before[0]);
    assert!(doc.undo());
    assert_eq!(texts(&doc), before);
    assert!(doc.redo());
}
