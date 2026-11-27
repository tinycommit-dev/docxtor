//! Picture watermarks: the op, how Word's markup is written, the image part and the header's relationship
//! to it, and what the saved file keeps when the watermark is replaced or removed.

use docxtor_core::ai::call_tool;
use docxtor_core::marks::{WatermarkKind, WatermarkLayout};
use docxtor_core::model::walk_paragraphs;
use docxtor_core::render::RBlock;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

/// Every XML part of a saved package must be namespace-well-formed.
fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn saved_part(doc: &mut Document, name: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

fn part_names(bytes: &[u8]) -> Vec<String> {
    Package::open(bytes).unwrap().parts.iter().map(|p| p.name.clone()).collect()
}

/// Reopen a document after editing its package.
fn rewrite(doc: &mut Document, f: impl FnOnce(&mut Package)) -> Document {
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    f(&mut pkg);
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

/// A 1x1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
    0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn picture(doc: &mut Document, extra: Value) {
    let media = doc.add_media("png", PNG.to_vec());
    let mut op = json!({"op":"set_picture_watermark","media":media,"width":200,"height":100});
    for (k, v) in extra.as_object().unwrap() {
        op[k] = v.clone();
    }
    ops(doc, op);
}

#[test]
fn a_picture_watermark_is_written_as_word_does() {
    let mut doc = doc_with("Alpha");
    assert!(doc.page_setup().headers.is_empty());
    picture(&mut doc, json!({}));
    let w = doc.page_setup().watermark.unwrap();
    assert_eq!((w.kind, w.layout, w.width, w.height, w.washout, w.text.clone()), (WatermarkKind::Picture, WatermarkLayout::Horizontal, Some(200.0), Some(100.0), Some(true), None));
    assert_eq!(w.media.as_deref(), Some("word/media/docxtor1.png"));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let header = String::from_utf8(pkg.get("word/header1.xml").unwrap().to_vec()).unwrap();
    for needle in [
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#,
        r#"xmlns:v="urn:schemas-microsoft-com:vml""#,
        r##"<v:shapetype id="_x0000_t75" coordsize="21600,21600" o:spt="75" o:preferrelative="t" path="m@4@5l@4@11@9@11@9@5xe" filled="f" stroked="f">"##,
        r#"<v:f eqn="prod @3 21600 pixelWidth"/>"#,
        r#"<o:lock v:ext="edit" aspectratio="t"/>"#,
        r#"<v:shape id="WordPictureWatermark"#,
        r##"type="#_x0000_t75""##,
        "width:200pt;height:100pt;z-index:-251657216;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin",
        r#"o:allowincell="f""#,
        r#"<v:imagedata r:id="rId1" o:title="docxtor1.png" gain="19661f" blacklevel="22938f"/>"#,
        r#"<w10:wrap anchorx="margin" anchory="margin"/>"#,
    ] {
        assert!(header.contains(needle), "missing {needle}\n{header}");
    }
    assert!(header.contains(r#"<w:pPr><w:pStyle w:val="Header"/></w:pPr><w:r><w:rPr><w:noProof/></w:rPr><w:pict>"#), "{header}");
    // The image part and the header's relationship to it.
    assert_eq!(pkg.get("word/media/docxtor1.png"), Some(PNG));
    let rels = String::from_utf8(pkg.get("word/_rels/header1.xml.rels").unwrap().to_vec()).unwrap();
    assert!(rels.contains(r#"Id="rId1""#) && rels.contains("relationships/image") && rels.contains(r#"Target="media/docxtor1.png""#), "{rels}");
    let types = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(types.contains(r#"Extension="png""#), "{types}");
    // It reads back after a reload, and the shape is no object in the header's text.
    let mut again = Document::open(&bytes).unwrap();
    assert_eq!(again.page_setup().watermark.unwrap(), w);
    let ps = again.page_setup();
    let RBlock::Para(p) = &ps.headers["default"].blocks[0] else { panic!() };
    assert!(p.segs.is_empty() && p.len == 0);
    assert_eq!(texts(&again), ["Alpha"]);
    let script = call_tool(&mut again, "read_document", &json!({})).unwrap();
    assert!(script.contains("# Watermark: picture (horizontal)"), "{script}");
}

#[test]
fn picture_watermark_size_and_washout() {
    let mut doc = doc_with("Alpha");
    // Auto: its own size when it fits, shrunk (not enlarged) when it doesn't.
    picture(&mut doc, json!({"washout":false}));
    let w = doc.page_setup().watermark.unwrap();
    assert_eq!((w.width, w.height, w.washout), (Some(200.0), Some(100.0), Some(false)));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains(r#"<v:imagedata r:id="rId1" o:title="docxtor1.png"/>"#) && !header.contains("gain="), "{header}");
    picture(&mut doc, json!({"width":1000,"height":500}));
    let w = doc.page_setup().watermark.unwrap();
    assert!((w.width.unwrap() - 468.0).abs() < 0.01 && (w.height.unwrap() - 234.0).abs() < 0.01, "{w:?}");
    // A scale is of the picture's own size and may pass the page.
    picture(&mut doc, json!({"scale":1.5}));
    let w = doc.page_setup().watermark.unwrap();
    assert_eq!((w.width, w.height), (Some(300.0), Some(150.0)));
    picture(&mut doc, json!({"scale":0.5}));
    assert_eq!(doc.page_setup().watermark.unwrap().width, Some(100.0));
    // Checked values.
    let media = doc.add_media("png", PNG.to_vec());
    assert!(err(&mut doc, json!({"op":"set_picture_watermark","media":media,"width":200,"height":100,"scale":0})).contains("scale"));
    assert!(err(&mut doc, json!({"op":"set_picture_watermark","media":media,"width":0,"height":100})).contains("width"));
    assert!(err(&mut doc, json!({"op":"set_picture_watermark","media":"word/media/none.png","width":10,"height":10})).contains("not found"));
}

#[test]
fn replacing_a_picture_watermark_with_text_or_another_picture_leaves_one_mark() {
    let mut doc = doc_with("Alpha");
    picture(&mut doc, json!({}));
    ops(&mut doc, json!({"op":"set_watermark","text":"DRAFT"}));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains("PowerPlusWaterMarkObject") && !header.contains("WordPictureWatermark") && !header.contains("v:imagedata"), "{header}");
    assert_eq!(doc.page_setup().watermark.as_ref().unwrap().kind, WatermarkKind::Text);
    // The picture is no longer used, so the file does not carry it.
    let names = part_names(&doc.save().unwrap());
    assert!(!names.iter().any(|n| n.contains("media/") || n.ends_with("header1.xml.rels")), "{names:?}");
    // And a picture over text.
    picture(&mut doc, json!({}));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(!header.contains("PowerPlusWaterMarkObject") && header.matches("WordPictureWatermark").count() == 1, "{header}");
    // A second picture takes the first one's place and the first one's image goes.
    picture(&mut doc, json!({"scale":2}));
    let names = part_names(&doc.save().unwrap());
    assert_eq!(names.iter().filter(|n| n.starts_with("word/media/")).count(), 1, "{names:?}");
}

#[test]
fn every_section_gets_the_picture_and_removing_it_cleans_up() {
    let mut doc = doc_with("Alpha\n\nBravo");
    let b = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    let b = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","section":b}));
    picture(&mut doc, json!({"scale":1}));
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| s.watermark.as_ref().is_some_and(|w| w.kind == WatermarkKind::Picture)));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let names = part_names(&bytes);
    let headers: Vec<&String> = names.iter().filter(|n| n.starts_with("word/header") && n.ends_with(".xml")).collect();
    assert_eq!(headers.len(), 2);
    let pkg = Package::open(&bytes).unwrap();
    let ids: Vec<String> = headers
        .iter()
        .map(|h| {
            let xml = String::from_utf8(pkg.get(h).unwrap().to_vec()).unwrap();
            let rels = String::from_utf8(pkg.get(&format!("word/_rels/{}.rels", &h["word/".len()..])).unwrap().to_vec()).unwrap();
            assert!(xml.contains(r#"r:id="rId1""#) && rels.contains("media/docxtor1.png"), "{h}\n{xml}\n{rels}");
            xml.split("WordPictureWatermark").nth(1).unwrap().split('"').next().unwrap().to_string()
        })
        .collect();
    assert_ne!(ids[0], ids[1], "each header's shape has its own id");
    assert_eq!(names.iter().filter(|n| n.starts_with("word/media/")).count(), 1, "one image part, shared");
    // Remove: the shapes, their relationships and the image all go from the saved file.
    ops(&mut doc, json!({"op":"set_watermark","text":null}));
    assert!(doc.page_setup().watermark.is_none());
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let names = part_names(&bytes);
    assert!(!names.iter().any(|n| n.starts_with("word/media/") || (n.contains("header") && n.ends_with(".rels"))), "{names:?}");
    let pkg = Package::open(&bytes).unwrap();
    for h in names.iter().filter(|n| n.starts_with("word/header")) {
        let xml = String::from_utf8(pkg.get(h).unwrap().to_vec()).unwrap();
        assert!(!xml.contains("v:shape") && !xml.contains("imagedata"), "{h}: {xml}");
    }
    // Undo brings the picture back, with its image, even after that save.
    assert!(doc.undo());
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert_eq!(part_names(&bytes).iter().filter(|n| n.starts_with("word/media/")).count(), 1);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(again.page_setup().watermark.unwrap().media.as_deref(), Some("word/media/docxtor1.png"));
}

#[test]
fn an_image_the_body_also_uses_stays_when_the_watermark_goes() {
    let mut doc = doc_with("Alpha");
    let media = doc.add_media("png", PNG.to_vec());
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_image","at":{"p":a,"o":0},"media":media,"width":10,"height":10}));
    ops(&mut doc, json!({"op":"set_picture_watermark","media":media,"width":200,"height":100}));
    ops(&mut doc, json!({"op":"set_watermark","text":null}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert!(part_names(&bytes).iter().any(|n| n == "word/media/docxtor1.png"), "{:?}", part_names(&bytes));
}

#[test]
fn a_word_made_picture_watermark_keeps_its_image_until_removed() {
    let mut doc = doc_with("Alpha");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    let mut doc = rewrite(&mut doc, |pkg| {
        let s = String::from_utf8(pkg.get("word/header1.xml").unwrap().to_vec()).unwrap();
        let s = s.replacen("xmlns:w=", r#"xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" xmlns:w="#, 1).replacen(
            "<w:p>",
            r##"<w:p><w:r><w:pict><v:shape id="WordPictureWatermark1" o:spid="_x0000_s2050" type="#_x0000_t75" style="position:absolute;width:300pt;height:150pt"><v:imagedata r:id="rId9" o:title="logo"/></v:shape></w:pict></w:r>"##,
            1,
        );
        pkg.set("word/header1.xml", s.into_bytes());
        pkg.set("word/media/image9.png", PNG.to_vec());
        pkg.set(
            "word/_rels/header1.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image9.png"/></Relationships>"#.to_vec(),
        );
    });
    // Untouched, the image and the relationship are as they were.
    let names = part_names(&doc.save().unwrap());
    assert!(names.iter().any(|n| n == "word/media/image9.png") && names.iter().any(|n| n == "word/_rels/header1.xml.rels"));
    assert_eq!(doc.page_setup().watermark.as_ref().unwrap().washout, Some(false));
    ops(&mut doc, json!({"op":"set_watermark","text":null}));
    let names = part_names(&doc.save().unwrap());
    assert!(!names.iter().any(|n| n == "word/media/image9.png" || n == "word/_rels/header1.xml.rels"), "{names:?}");
}
