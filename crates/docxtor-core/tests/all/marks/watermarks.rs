//! Tests of: Watermarks.

use super::*;

/// What Word writes into a header for a text watermark (Insert > Watermark > DRAFT).
const WORD_HEADER: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word"><w:p><w:pPr><w:pStyle w:val="Header"/></w:pPr><w:r><w:rPr><w:noProof/></w:rPr><w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:formulas><v:f eqn="sum #0 0 10800"/><v:f eqn="prod #0 2 1"/><v:f eqn="sum 21600 0 @1"/><v:f eqn="sum 0 0 @2"/><v:f eqn="sum 21600 0 @3"/><v:f eqn="if @0 @3 0"/><v:f eqn="if @0 21600 @1"/><v:f eqn="if @0 0 @2"/><v:f eqn="if @0 @4 21600"/><v:f eqn="mid @5 @6"/><v:f eqn="mid @8 @5"/><v:f eqn="mid @7 @8"/><v:f eqn="mid @6 @7"/><v:f eqn="sum @6 0 @5"/></v:formulas><v:path textpathok="t" o:connecttype="custom" o:connectlocs="@9,0;@10,10800;@11,21600;@12,10800" o:connectangles="270,180,90,0"/><v:textpath on="t" fitshape="t"/><v:handles><v:h position="#0,bottomRight" xrange="6629,14971"/></v:handles><o:lock v:ext="edit" text="t" shapetype="t"/></v:shapetype><v:shape id="PowerPlusWaterMarkObject357476642" o:spid="_x0000_s2049" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:412.4pt;height:247.45pt;rotation:315;z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="silver" stroked="f"><v:fill opacity=".5"/><v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="CONFIDENTIAL"/><w10:wrap anchorx="margin" anchory="margin"/></v:shape></w:pict></w:r><w:r><w:t>Quarterly report</w:t></w:r></w:p></w:hdr>"##;

/// A document whose default header is the Word-made one above.
pub(super) fn doc_with_word_watermark() -> Document {
    let mut doc = doc_with("Alpha\n\nBravo");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    rewrite(&mut doc, |pkg| set_part(pkg, "word/header1.xml", |_| WORD_HEADER.to_string()))
}

#[test]
fn a_word_made_watermark_is_read_and_hidden_from_the_text() {
    let mut doc = doc_with_word_watermark();
    let ps = doc.page_setup();
    let w = ps.watermark.clone().expect("watermark");
    assert_eq!(w.kind, WatermarkKind::Text);
    assert_eq!((w.text.as_deref(), w.font.as_deref(), w.color.as_deref(), w.opacity), (Some("CONFIDENTIAL"), Some("Calibri"), Some("C0C0C0"), Some(0.5)));
    assert_eq!((w.layout, w.angle, w.width, w.height, w.size), (WatermarkLayout::Diagonal, 315.0, Some(412.4), Some(247.45), Some(247.45)));
    assert_eq!(ps.sections[0].watermark.as_ref(), Some(&w));
    // The header's paragraph renders its text only: no object placeholder, and the text keeps its offsets.
    let story = &ps.headers["default"];
    let RBlock::Para(p) = &story.blocks[0] else { panic!() };
    assert_eq!(p.len, "Quarterly report".len());
    assert!(p.segs.iter().all(|s| s.k == "t") && p.segs[0].o == 0, "{:?}", p.segs.iter().map(|s| (s.k, s.o)).collect::<Vec<_>>());
    // DocScript shows the text and names the watermark.
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains("[p") && script.contains("] Quarterly report") && !script.contains("![Shape]"), "{script}");
    assert!(script.contains(r#"# Watermark: "CONFIDENTIAL" (diagonal, C0C0C0)"#), "{script}");
    // The header isn't rewritten unless it is edited.
    assert_eq!(saved_part(&mut doc, "word/header1.xml"), WORD_HEADER);
    // Typing in that paragraph leaves the shape alone.
    let h = match &ps.headers["default"].blocks[0] {
        RBlock::Para(p) => p.id,
        _ => unreachable!(),
    };
    ops(&mut doc, json!({"op":"insert_text","at":{"p":h,"o":0},"text":"Our "}));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains("PowerPlusWaterMarkObject357476642") && header.contains("Our Quarterly report"), "{header}");
}

#[test]
fn a_header_shape_that_is_not_a_watermark_is_still_an_object() {
    let mut doc = doc_with("Alpha");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    let mut doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/header1.xml", |s| {
            s.replacen("xmlns:w=", r#"xmlns:v="urn:schemas-microsoft-com:vml" xmlns:w="#, 1)
                .replacen("<w:p>", r#"<w:p><w:r><w:pict><v:shape id="Rectangle 1" style="position:absolute;width:20pt;height:10pt"/></w:pict></w:r>"#, 1)
        })
    });
    let ps = doc.page_setup();
    assert!(ps.watermark.is_none());
    let RBlock::Para(p) = &ps.headers["default"].blocks[0] else { panic!() };
    assert!(p.segs.iter().any(|s| s.k == "shape"), "{:?}", p.segs.iter().map(|s| s.k).collect::<Vec<_>>());
    assert!(err(&mut doc, json!({"op":"set_watermark","text":null})).contains("no watermark"));
}

#[test]
fn a_picture_watermark_is_reported() {
    let mut doc = doc_with("Alpha");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    let doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/header1.xml", |s| {
            s.replacen("xmlns:w=", r#"xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" xmlns:w="#, 1).replacen(
                "<w:p>",
                r##"<w:p><w:r><w:pict><v:shape id="WordPictureWatermark1" o:spid="_x0000_s2050" type="#_x0000_t75" style="position:absolute;margin-left:0;margin-top:0;width:300pt;height:150pt;z-index:-251657728;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f"><v:imagedata r:id="rId9" o:title="logo" gain="19661f" blacklevel="22938f"/><w10:wrap anchorx="margin" anchory="margin"/></v:shape></w:pict></w:r>"##,
                1,
            )
        });
        pkg.set("word/_rels/header1.xml.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image9.png"/></Relationships>"#.to_vec());
    });
    let w = doc.page_setup().watermark.unwrap();
    assert_eq!(w.kind, WatermarkKind::Picture);
    assert_eq!((w.media.as_deref(), w.layout, w.width, w.height, w.text), (Some("word/media/image9.png"), WatermarkLayout::Horizontal, Some(300.0), Some(150.0), None));
}

#[test]
fn a_watermark_on_a_document_without_headers_makes_one() {
    let mut doc = doc_with("Alpha");
    assert!(doc.page_setup().headers.is_empty() && doc.page_setup().watermark.is_none());
    ops(&mut doc, json!({"op":"set_watermark","text":"DRAFT"}));
    let ps = doc.page_setup();
    assert!(ps.headers.contains_key("default"));
    let w = ps.watermark.clone().unwrap();
    assert_eq!((w.kind, w.text.as_deref(), w.layout, w.color.as_deref(), w.opacity, w.angle), (WatermarkKind::Text, Some("DRAFT"), WatermarkLayout::Diagonal, Some("C0C0C0"), Some(0.5), 315.0));
    assert!(w.font.is_some());
    let (width, height) = (w.width.unwrap(), w.height.unwrap());
    assert!((width / height - 4.0).abs() < 0.05 && (width - 527.85).abs() < 3.0, "DRAFT is about 528x132pt like Word's: {width}x{height}");
    let header = saved_part(&mut doc, "word/header1.xml");
    for needle in [
        r#"xmlns:v="urn:schemas-microsoft-com:vml""#,
        r#"xmlns:o="urn:schemas-microsoft-com:office:office""#,
        r#"xmlns:w10="urn:schemas-microsoft-com:office:word""#,
        r#"<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e">"#,
        r#"<v:f eqn="sum #0 0 10800"/>"#,
        r#"<v:path textpathok="t" o:connecttype="custom" o:connectlocs="@9,0;@10,10800;@11,21600;@12,10800" o:connectangles="270,180,90,0"/>"#,
        r##"<v:h position="#0,bottomRight" xrange="6629,14971"/>"##,
        r#"<o:lock v:ext="edit" text="t" shapetype="t"/>"#,
        r#"<v:shape id="PowerPlusWaterMarkObject"#,
        r##"type="#_x0000_t136""##,
        "rotation:315;z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin",
        r#"o:allowincell="f" fillcolor="silver" stroked="f""#,
        r#"<v:fill opacity=".5"/>"#,
        r#"font-family:&quot;"#,
        r#";font-size:1pt" string="DRAFT"/>"#,
        r#"<w10:wrap anchorx="margin" anchory="margin"/>"#,
        r#"<w:rPr><w:noProof/></w:rPr><w:pict>"#,
    ] {
        assert!(header.contains(needle), "missing {needle}\n{header}");
    }
    // It lives in the header's own (first) paragraph, which keeps the Header style.
    assert!(header.contains(r#"<w:pPr><w:pStyle w:val="Header"/></w:pPr><w:r><w:rPr><w:noProof/></w:rPr><w:pict>"#), "{header}");
    assert_eq!(header.matches("<w:p>").count() + header.matches("<w:p ").count(), 1);
    // And it reads back after a reload.
    let again = reopen(&mut doc).page_setup().watermark.unwrap();
    assert_eq!(again, w);
    // The header paragraph is empty to the editor: the shape is not drawn as an object.
    let RBlock::Para(p) = &doc.page_setup().headers["default"].blocks[0] else { panic!() };
    assert!(p.segs.is_empty() && p.len == 0);
    // The body is untouched.
    assert_eq!(texts(&doc), ["Alpha"]);
    // One undo removes the watermark; the document is as before (the header is empty).
    assert!(doc.undo());
    assert!(doc.page_setup().watermark.is_none());
    assert!(!saved_part(&mut doc, "word/document.xml").contains("headerReference"));
    assert!(doc.redo());
    assert_eq!(doc.page_setup().watermark.unwrap().text.as_deref(), Some("DRAFT"));
    assert!(saved_part(&mut doc, "word/header1.xml").contains(r#"string="DRAFT""#));
}

#[test]
fn a_watermark_keeps_the_headers_text() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let before = doc.page_setup();
    let RBlock::Para(hp) = &before.headers["default"].blocks[0] else { panic!() };
    let (hid, hlen) = (hp.id, hp.len);
    ops(&mut doc, json!({"op":"set_watermark","text":"CONFIDENTIAL","color":"#C00000","opacity":0.3,"font":"Arial"}));
    let after = doc.page_setup();
    assert_eq!(after.headers.len(), 1);
    assert_eq!(after.sections[0].headers["default"], "word/header1.xml", "no new header part");
    let RBlock::Para(hp) = &after.headers["default"].blocks[0] else { panic!() };
    assert_eq!((hp.id, hp.len), (hid, hlen));
    assert_eq!(hp.segs[0].x.as_deref(), Some("Docxtor — confidential draft"));
    let w = after.watermark.unwrap();
    assert_eq!((w.text.as_deref(), w.color.as_deref(), w.opacity, w.font.as_deref()), (Some("CONFIDENTIAL"), Some("C00000"), Some(0.3), Some("Arial")));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains(r##"fillcolor="#c00000""##) && header.contains(r#"<v:fill opacity=".3"/>"#) && header.contains("Docxtor — confidential draft"), "{header}");
    assert!(header.contains(r#"font-family:&quot;Arial&quot;"#), "{header}");
    // Only the header (and what the new part needs) changed; the body is byte-identical.
    let report = fidelity::compare(&Package::open(&fixture("rich.docx")).unwrap(), &Package::open(&doc.save().unwrap()).unwrap());
    assert!(report.changed.iter().all(|c| c.name == "word/header1.xml"), "{:?}", report.changed);
}

#[test]
fn every_section_and_header_variant_gets_the_watermark() {
    let mut doc = doc_with("Alpha\n\nBravo\n\nCharlie");
    let b = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    let c = find_p(&doc, "Charlie");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":c,"o":0},"kind":"next_page"}));
    let (a, b) = (find_p(&doc, "Alpha"), find_p(&doc, "Bravo"));
    // The middle section has its own header, the first a different first page, the last inherits the middle's.
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","section":b}));
    ops(&mut doc, json!({"op":"set_title_page","on":true,"section":a}));
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","variant":"first","section":a}));
    assert!(!doc.page_setup().sections[0].headers.contains_key("default"));
    ops(&mut doc, json!({"op":"set_watermark","text":"SAMPLE","layout":"horizontal"}));
    let ps = doc.page_setup();
    assert_eq!(ps.sections.len(), 3);
    for (i, s) in ps.sections.iter().enumerate() {
        let w = s.watermark.as_ref().unwrap_or_else(|| panic!("section {i} has no watermark"));
        assert_eq!((w.text.as_deref(), w.layout, w.angle), (Some("SAMPLE"), WatermarkLayout::Horizontal, 0.0));
    }
    assert!(ps.sections[0].headers.contains_key("default") && ps.sections[0].headers.contains_key("first"));
    assert_eq!(ps.sections[2].headers["default"], ps.sections[1].headers["default"], "the last section still shares the middle's");
    // The last section's own list stays empty: nothing was added where inheritance does the job.
    assert!(ps.headers.is_empty());
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let mut parts = 0;
    for part in ps.parts.keys().filter(|p| p.contains("header")) {
        let xml = String::from_utf8(pkg.get(part).unwrap().to_vec()).unwrap();
        assert_eq!(xml.matches("PowerPlusWaterMarkObject").count(), 1, "{part}: {xml}");
        assert!(!xml.contains("rotation:315") && xml.contains(r#"string="SAMPLE""#), "{part}: {xml}");
        parts += 1;
    }
    assert_eq!(parts, 3, "first page, first section default and the middle section's");
    let mut ids: Vec<String> = Vec::new();
    for part in ps.parts.keys().filter(|p| p.contains("header")) {
        let xml = String::from_utf8(pkg.get(part).unwrap().to_vec()).unwrap();
        ids.push(xml.split("PowerPlusWaterMarkObject").nth(1).unwrap().split('"').next().unwrap().to_string());
    }
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "each header's shape has its own id");
    // Removing takes it from every header and leaves the parts.
    ops(&mut doc, json!({"op":"set_watermark","text":null}));
    let ps = doc.page_setup();
    assert!(ps.watermark.is_none() && ps.sections.iter().all(|s| s.watermark.is_none()));
    assert_eq!(ps.parts.keys().filter(|p| p.contains("header")).count(), 3);
}

#[test]
fn even_page_headers_get_the_watermark_too() {
    let mut doc = doc_with("Alpha");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    let mut doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/settings.xml", |s| s.replacen("<w:defaultTabStop", "<w:evenAndOddHeaders/><w:defaultTabStop", 1));
    });
    assert!(doc.page_setup().even_and_odd_headers);
    ops(&mut doc, json!({"op":"set_watermark","text":"DRAFT"}));
    let ps = doc.page_setup();
    let s = &ps.sections[0];
    assert!(s.headers.contains_key("default") && s.headers.contains_key("even"));
    assert_ne!(s.headers["default"], s.headers["even"]);
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"w:type="even""#), "{xml}");
    for part in s.headers.values() {
        assert!(saved_part(&mut doc, part).contains(r#"string="DRAFT""#), "{part}");
    }
}

#[test]
fn replacing_and_removing_a_watermark() {
    let mut doc = doc_with_word_watermark();
    ops(&mut doc, json!({"op":"set_watermark","text":"DRAFT","color":"red","opacity":1}));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert_eq!(header.matches("PowerPlusWaterMarkObject").count(), 1, "the old one is replaced: {header}");
    assert!(header.contains("DRAFT") && !header.contains("CONFIDENTIAL") && header.contains("Quarterly report"), "{header}");
    assert!(header.contains(r##"fillcolor="#ff0000""##) && !header.contains("<v:fill"), "{header}");
    assert_eq!(header.matches("_x0000_t136\" coordsize").count(), 1, "one shape type");
    let w = doc.page_setup().watermark.unwrap();
    assert_eq!((w.text.as_deref(), w.color.as_deref(), w.opacity), (Some("DRAFT"), Some("FF0000"), None));
    ops(&mut doc, json!({"op":"set_watermark","text":"  Confidential  ","layout":"horizontal"}));
    assert_eq!(doc.page_setup().watermark.unwrap().text.as_deref(), Some("Confidential"));
    // A mixed-case word is a little less wide than capitals.
    let w = doc.page_setup().watermark.unwrap();
    assert!(w.width.unwrap() <= 468.0 + 0.01 && w.angle == 0.0);
    // Remove.
    ops(&mut doc, json!({"op":"set_watermark","text":null}));
    assert!(doc.page_setup().watermark.is_none());
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(!header.contains("PowerPlus") && !header.contains("v:shape") && header.contains("Quarterly report"), "{header}");
    assert!(err(&mut doc, json!({"op":"set_watermark"})).contains("no watermark"));
    // Undo steps back through each change.
    assert!(doc.undo());
    assert_eq!(doc.page_setup().watermark.unwrap().text.as_deref(), Some("Confidential"));
    assert!(doc.undo() && doc.undo());
    assert_eq!(doc.page_setup().watermark.unwrap().text.as_deref(), Some("CONFIDENTIAL"));
    assert_eq!(saved_part(&mut doc, "word/header1.xml").replace("\r\n", "\n"), WORD_HEADER);
}

#[test]
fn watermark_values_are_checked() {
    let mut doc = doc_with("Alpha");
    assert!(err(&mut doc, json!({"op":"set_watermark","text":"   "})).contains("some text"));
    assert!(err(&mut doc, json!({"op":"set_watermark","text":"DRAFT","color":"fluorescent"})).contains("not a colour"));
    assert!(err(&mut doc, json!({"op":"set_watermark","text":"DRAFT","opacity":2})).contains("Opacity"));
    assert!(err(&mut doc, json!({"op":"set_watermark","text":"a\nb"})).contains("one line"));
    assert!(doc.page_setup().headers.is_empty(), "failed ops change nothing");
    ops(&mut doc, json!({"op":"set_watermark","text":"A \"quoted\" & <odd> one","font":"Bad\"Font"}));
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains(r#"string="A &quot;quoted&quot; &amp; &lt;odd&gt; one""#), "{header}");
    assert_eq!(doc.page_setup().watermark.unwrap().text.as_deref(), Some(r#"A "quoted" & <odd> one"#));
    // The shape types of colour names Word writes are understood.
    assert_eq!(doc.page_setup().watermark.unwrap().font.as_deref(), Some("BadFont"));
}

#[test]
fn rewriting_a_header_paragraph_keeps_its_watermark() {
    let mut doc = doc_with_word_watermark();
    call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","text":"**Acme** — Page {PAGE}"})).unwrap();
    let header = saved_part(&mut doc, "word/header1.xml");
    assert!(header.contains("PowerPlusWaterMarkObject357476642") && header.contains("Acme"), "{header}");
    assert!(!header.contains("Quarterly"), "{header}");
    assert_eq!(doc.page_setup().watermark.unwrap().text.as_deref(), Some("CONFIDENTIAL"));
}

#[test]
fn edits_next_to_the_watermark_do_not_move_offsets() {
    let mut doc = doc_with_word_watermark();
    let h = match &doc.page_setup().headers["default"].blocks[0] {
        RBlock::Para(p) => p.id,
        _ => unreachable!(),
    };
    let p = find_para(&doc.stories[0].blocks, h).unwrap();
    assert_eq!(p.plain_text(), "Quarterly report");
    assert_eq!(p.len16(), "Quarterly report".len());
    ops(&mut doc, json!({"op":"delete","from":{"p":h,"o":0},"to":{"p":h,"o":10}}));
    let p = find_para(&doc.stories[0].blocks, h).unwrap();
    assert_eq!(p.plain_text(), "report");
    assert!(saved_part(&mut doc, "word/header1.xml").contains("PowerPlusWaterMarkObject"));
}
