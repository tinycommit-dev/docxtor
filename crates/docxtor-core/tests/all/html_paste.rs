//! HTML from Word, Google Docs and web pages, turned into fragments and pasted.

use docxtor_core::fragment::{Fragment, PasteMode};
use docxtor_core::html_import::{html_image_sources, html_to_fragment};
use docxtor_core::model::walk_paragraphs;
use docxtor_core::{package::Package, Document, Op, Pos};
use std::collections::HashMap;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn frag(html: &str) -> Fragment {
    html_to_fragment(html, &HashMap::new())
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

/// Paste into a blank document and save; returns the document and its XML.
fn pasted(f: &Fragment) -> (Document, String) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply(&Op::InsertFragment { at: Pos::new(first, 0), fragment: f.clone(), mode: PasteMode::Keep }, false).unwrap();
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    (doc, xml)
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn well_formed(f: &Fragment) {
    let doc = docxtor_core::xml::parse(format!("<f>{}</f>", f.body).as_bytes()).unwrap_or_else(|e| panic!("body: {e}\n{}", f.body));
    assert!(f.body.is_empty() || doc.root.elements().count() > 0);
    docxtor_core::xml::parse(format!("<f>{}</f>", f.styles).as_bytes()).unwrap();
    docxtor_core::xml::parse(format!("<f>{}</f>", f.numbering).as_bytes()).unwrap();
}

const WORD: &str = r#"<html xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w="urn:schemas-microsoft-com:office:word" xmlns="http://www.w3.org/TR/REC-html40">
<head><meta http-equiv=Content-Type content="text/html; charset=utf-8">
<!--[if gte mso 9]><xml><w:WordDocument><w:View>Normal</w:View></w:WordDocument></xml><![endif]-->
<style>
<!--
 /* Style Definitions */
 p.MsoNormal, li.MsoNormal, div.MsoNormal
	{margin-top:0in;margin-right:0in;margin-bottom:8.0pt;margin-left:0in;line-height:107%;font-size:11.0pt;font-family:"Calibri",sans-serif;}
p.MsoListParagraph
	{margin-top:0in;margin-right:0in;margin-bottom:8.0pt;margin-left:.5in;line-height:107%;font-size:11.0pt;font-family:"Calibri",sans-serif;}
@list l0:level1
	{mso-level-number-format:bullet;mso-level-text:\F0B7;mso-level-tab-stop:.5in;mso-level-number-position:left;text-indent:-.25in;mso-ansi-font-size:10.0pt;font-family:Symbol;}
@list l1:level1
	{mso-level-start-at:1;mso-level-text:"%1\.";mso-level-tab-stop:.5in;mso-level-number-position:left;text-indent:-.25in;}
-->
</style></head>
<body lang=EN-US style='tab-interval:.5in'>
<!--StartFragment-->
<p class=MsoNormal><b><span style='font-size:14.0pt;line-height:107%;font-family:"Arial",sans-serif;color:#C00000'>Bold red Arial</span></b><o:p></o:p></p>
<p class=MsoNormal align=center style='text-align:center'><i>Centered</i> and <u>underlined</u> and <span style='mso-highlight:yellow'>highlighted</span>, x<sup>2</sup> and H<sub>2</sub>O<o:p></o:p></p>
<p class=MsoListParagraphCxSpFirst style='text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span style='font-family:Symbol;mso-fareast-font-family:Symbol;mso-bidi-font-family:Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><![endif]>First bullet<o:p></o:p></p>
<p class=MsoListParagraphCxSpLast style='text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span style='font-family:Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><![endif]>Second bullet<o:p></o:p></p>
<p class=MsoListParagraphCxSpFirst style='text-indent:-.25in;mso-list:l1 level1 lfo2'><!--[if !supportLists]--><span><span style='mso-list:Ignore'>1.<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><!--[endif]-->Numbered one<o:p></o:p></p>
<p class=MsoListParagraphCxSpLast style='text-indent:-.25in;mso-list:l1 level1 lfo2'><!--[if !supportLists]--><span><span style='mso-list:Ignore'>2.<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><!--[endif]-->Numbered two<o:p></o:p></p>
<table class=MsoTableGrid border=1 cellspacing=0 cellpadding=0 width=400 style='width:300.0pt;border-collapse:collapse;border:none'>
 <tr>
  <td width=200 valign=top style='width:150.0pt;border:solid windowtext 1.0pt;background:#D9E2F3;padding:0in 5.4pt 0in 5.4pt'><p class=MsoNormal><b>Head A</b></p></td>
  <td width=200 colspan=1 valign=top style='width:150.0pt;border:solid windowtext 1.0pt;border-left:none;padding:0in 5.4pt 0in 5.4pt'><p class=MsoNormal>Head B</p></td>
 </tr>
 <tr>
  <td colspan=2 style='width:300.0pt;border:solid windowtext 1.0pt;border-top:none;padding:0in 5.4pt'><p class=MsoNormal>Spans both</p></td>
 </tr>
</table>
<p class=MsoNormal><a href="https://example.com/page">A link</a> and a picture: <img width=96 height=48 src="file:///C:/Users/x/AppData/Local/Temp/msohtmlclip1/01/clip_image001.png" alt="Logo"></p>
<!--EndFragment-->
</body></html>"#;

#[test]
fn word_html_keeps_fonts_sizes_colours_lists_and_tables() {
    let f = frag(WORD);
    well_formed(&f);
    let b = &f.body;
    assert!(b.contains(r#"w:ascii="Arial""#), "{b}");
    assert!(b.contains(r#"<w:sz w:val="28"/>"#));
    assert!(b.contains(r#"<w:color w:val="C00000"/>"#));
    assert!(b.contains("<w:b/>"));
    assert!(b.contains(r#"<w:jc w:val="center"/>"#));
    assert!(b.contains("<w:i/>") && b.contains(r#"<w:u w:val="single"/>"#));
    assert!(b.contains(r#"<w:highlight w:val="yellow"/>"#));
    assert!(b.contains(r#"<w:vertAlign w:val="superscript"/>"#) && b.contains(r#"<w:vertAlign w:val="subscript"/>"#));
    // Word's Normal spacing travels as written.
    assert!(b.contains(r#"w:after="160""#), "{b}");
    assert!(b.contains(r#"w:line="257""#), "{b}");
    // Two lists: a bullet list and a numbered one. The hand-written markers are gone from the text.
    assert_eq!(b.matches("<w:numPr>").count(), 4);
    assert!(f.numbering.contains(r#"w:numFmt w:val="bullet""#));
    assert!(f.numbering.contains(r#"w:numFmt w:val="decimal""#));
    let text = f.plain_text();
    assert!(text.contains("First bullet") && !text.contains("·") && !text.contains("1."), "{text}");
    // The table keeps its widths, borders, shading and span.
    assert!(b.contains("<w:tbl>"));
    assert!(b.contains(r#"<w:gridSpan w:val="2"/>"#));
    assert!(b.contains(r#"w:fill="D9E2F3""#));
    assert!(b.contains("<w:tcBorders>"));
    assert!(b.contains(r#"<w:gridCol w:w="3000"/>"#), "{b}");
    // Link, and a picture the page can't read.
    assert_eq!(f.links.values().collect::<Vec<_>>(), ["https://example.com/page"]);
    assert!(b.contains("w:hyperlink"));
    assert!(f.warnings.iter().any(|w| w.kind == "picture-unreadable" && w.count == 1));
    assert!(f.block);
}

#[test]
fn word_html_pastes_into_a_document_and_saves_valid_xml() {
    let (doc, xml) = pasted(&frag(WORD));
    assert!(xml.contains("Bold red Arial"));
    assert!(xml.contains("<w:tbl>"));
    let t = texts(&doc);
    assert!(t.iter().any(|t| t == "Second bullet"));
}

const GDOCS: &str = r#"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-8b0c9b7c-7fff-0c0b-8b0e-6a2d1f0f5a11"><h1 dir="ltr" style="line-height:1.38;margin-top:20pt;margin-bottom:6pt;"><span style="font-size:20pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Big heading</span></h1><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#1155cc;background-color:#ffff00;font-weight:700;font-style:italic;text-decoration:underline;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre-wrap;">Bold italic link-colour on yellow</span></p><ul style="margin-top:0;margin-bottom:0;padding-inline-start:48px;"><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;">Item one</span></p></li><li dir="ltr" style="list-style-type:disc;" aria-level="1"><p dir="ltr" role="presentation"><span>Item two</span></p></li><ul style="margin-top:0;margin-bottom:0;"><li dir="ltr" style="list-style-type:circle;" aria-level="2"><p dir="ltr" role="presentation"><span>Nested</span></p></li></ul></ul><br /><div dir="ltr" style="margin-left:0pt;" align="left"><table style="border:none;border-collapse:collapse;"><colgroup><col width="200" /><col width="150" /></colgroup><tbody><tr style="height:0pt"><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;vertical-align:top;background-color:#cfe2f3;padding:5pt 5pt 5pt 5pt;overflow:hidden;overflow-wrap:break-word;"><p dir="ltr" style="line-height:1.2;text-align:center;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;">Cell A</span></p></td><td style="border-left:solid #000000 1pt;border-right:solid #000000 1pt;border-bottom:solid #000000 1pt;border-top:solid #000000 1pt;"><p dir="ltr"><span>Cell B</span></p></td></tr></tbody></table></div></b>"#;

#[test]
fn google_docs_html_keeps_headings_lists_tables_and_character_formatting() {
    let f = frag(GDOCS);
    well_formed(&f);
    let b = &f.body;
    // The wrapper's `font-weight: normal` does not make everything bold; the span's 700 does.
    assert_eq!(b.matches("<w:b/>").count(), 1, "{b}");
    assert!(b.contains(r#"w:pStyle w:val="Heading1""#));
    // Keep source formatting: the heading keeps the font, size and black Google Docs gave it (the style is blue and in
    // the theme's heading font), and it is still a Heading 1.
    let h = &b[..b.find("</w:p>").unwrap()];
    assert!(h.contains(r#"<w:sz w:val="40"/>"#) && h.contains("Arial") && h.contains(r#"<w:color w:val="000000"/>"#), "{h}");
    // Merge formatting takes them away: the heading looks like the document's headings.
    let merged = f.merged();
    let m = &merged.body[..merged.body.find("</w:p>").unwrap()];
    assert!(!m.contains("w:sz") && !m.contains("Arial") && !m.contains("w:color"), "{m}");
    assert!(f.styles.contains(r#"w:styleId="Heading1""#));
    assert!(b.contains(r#"<w:highlight w:val="yellow"/>"#));
    assert!(b.contains("<w:i/>"));
    assert_eq!(b.matches("<w:numPr>").count(), 3);
    assert!(b.contains(r#"<w:ilvl w:val="1"/>"#), "nested level");
    assert!(b.contains("<w:tbl>") && b.contains(r#"w:fill="CFE2F3""#));
    assert!(b.contains(r#"<w:gridCol w:w="3000"/>"#), "{b}");
    assert!(f.block);
    let (doc, _) = pasted(&f);
    let t = texts(&doc);
    assert!(t.contains(&"Nested".to_string()));
}

#[test]
fn google_docs_inline_selection_pastes_inline() {
    let f = frag(r#"<meta charset='utf-8'><b style="font-weight:normal;" id="docs-internal-guid-1"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#ff0000;font-weight:700;">some words</span></b><br class="Apple-interchange-newline">"#);
    assert!(!f.block);
    assert_eq!(f.plain_text(), "some words");
    assert!(f.body.contains(r#"<w:color w:val="FF0000"/>"#) && f.body.contains("<w:b/>"));
    // Pasted into the middle of a paragraph, it stays in that paragraph.
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&serde_json::json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Hello world"}).to_string(), false).unwrap();
    doc.apply(&Op::InsertFragment { at: Pos::new(first, 5), fragment: f, mode: PasteMode::Keep }, false).unwrap();
    assert_eq!(texts(&doc)[0], "Hellosome words world");
}

const WEB: &str = r##"<html><head><title>x</title><style>.note{color:#336699;font-style:italic} body{font-family:Georgia,serif}</style><script>var x = "<p>nope</p>";</script></head>
<body>
<h2>Section <em>two</em></h2>
<p>Plain paragraph with <a href="#top">an anchor</a>, <a href="mailto:a@b.co">mail</a>, <code>code</code> &amp; &quot;entities&quot; &mdash; done.</p>
<p class="note">Styled by class</p>
<blockquote>Quoted text</blockquote>
<pre>line one
  indented line two</pre>
<ol start="3"><li>Third</li><li>Fourth<ol type="a"><li>Sub a</li></ol></li></ol>
<p style="text-align:justify;margin-left:36pt;text-indent:18pt;border-bottom:1px solid #999;background-color:#eeeeee">Indented justified</p>
<hr>
<p><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==" width="40" height="20" alt="dot"></p>
<p><img src="https://example.com/a.png"> <img src="https://example.com/a.png"> <img src="https://example.com/b.png"></p>
<p>Equation: <span class="math" data-latex="x^2">x2</span></p>
</body></html>"##;

#[test]
fn web_page_html() {
    let f = frag(WEB);
    well_formed(&f);
    let b = &f.body;
    assert!(b.contains(r#"w:pStyle w:val="Heading2""#));
    assert!(f.links.values().any(|u| u == "mailto:a@b.co"));
    // A link to a place in the page: its anchors are not made into bookmarks, so the words stay as plain text, not a link to nothing.
    assert!(!b.contains("w:anchor="), "no link to a bookmark that does not exist");
    assert!(f.plain_text().contains("with an anchor, mail"));
    assert!(b.contains(r#"<w:color w:val="336699"/>"#) && b.contains("<w:i/>"));
    assert!(b.contains(r#"w:pStyle w:val="Code""#), "pre uses the Code style");
    assert_eq!(f.plain_text().matches("indented line two").count(), 1);
    assert!(f.plain_text().contains("  indented line two"), "pre keeps its spaces");
    assert!(b.contains(r#"w:start w:val="3""#) || f.numbering.contains(r#"<w:start w:val="3"/>"#));
    assert!(f.numbering.contains("lowerLetter"));
    assert!(b.contains(r#"<w:jc w:val="both"/>"#));
    assert!(b.contains("w:pBdr") && b.contains(r#"w:fill="EEEEEE""#));
    assert!(b.contains(r#"w:left="720""#) && b.contains(r#"w:firstLine="360""#));
    assert_eq!(f.media.len(), 1, "the data: picture");
    assert!(b.contains("a:blip") && b.contains(r#"descr="dot""#) && b.contains(r#"cx="381000""#));
    assert!(f.warnings.iter().any(|w| w.kind == "picture-unreadable" && w.count == 3));
    assert!(b.contains("m:oMath"));
    assert!(!f.plain_text().contains("nope"), "script is not text");
    assert_eq!(html_image_sources(WEB), ["https://example.com/a.png", "https://example.com/b.png"]);
    let (_doc, _xml) = pasted(&f);
}

#[test]
fn fetched_pictures_are_used() {
    let html = r#"<p>x<img src="https://example.com/a.png"></p>"#;
    let mut images = HashMap::new();
    images.insert("https://example.com/a.png".to_string(), docxtor_core::fragment::b64_decode(PNG).unwrap());
    let f = html_to_fragment(html, &images);
    assert_eq!(f.media.len(), 1);
    assert!(f.warnings.is_empty());
    let (mut doc, xml) = pasted(&f);
    assert!(xml.contains("w:drawing"));
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    assert!(pkg.parts.iter().any(|p| p.name.starts_with("word/media/")));
}

#[test]
fn rowspan_makes_vertical_merges() {
    let f = frag("<table><tr><td rowspan=2>A</td><td>B</td></tr><tr><td>C</td></tr></table>");
    well_formed(&f);
    assert!(f.body.contains(r#"<w:vMerge w:val="restart"/>"#) && f.body.contains("<w:vMerge/>"), "{}", f.body);
    assert_eq!(f.body.matches("<w:tc>").count(), 4);
}

#[test]
fn text_only_and_junk_do_not_panic() {
    for s in ["", "   ", "plain", "<", "<<<>>>", "<p", "<p><b><i>x", "</p></div>", "<table><td>a", "<ul><li>a<ul><li>b</ul>", "&#99999999; &#xZZ; &", "<![if", "<!--", "<style>p{", "<img>", "<a href>x</a>"] {
        let f = frag(s);
        well_formed(&f.clone()) ;
        let _ = f.plain_text();
    }
    let deep = "<div>".repeat(2000) + "x" + &"</div>".repeat(2000);
    let f = frag(&deep);
    assert!(f.plain_text().len() < 10);
}

#[test]
fn pasting_html_through_the_op() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&serde_json::json!({"op":"paste_html","at":{"p":first,"o":0},"html":"<h1>Title</h1><p>Body <b>bold</b></p>"}).to_string(), false).unwrap();
    let t = texts(&doc);
    assert_eq!(&t[..2], ["Title", "Body bold"]);
    assert!(doc.undo());
    assert_eq!(texts(&doc).iter().filter(|t| !t.is_empty()).count(), 0);
}

#[test]
fn the_insert_html_tool_adds_blocks_after_a_paragraph() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let out = docxtor_core::ai::call_tool(&mut doc, "insert_html", &serde_json::json!({"after": format!("p{first}"), "html": "<h1>Title</h1><ul><li>one</li><li>two</li></ul>"})).unwrap();
    assert!(out.contains("Title") && out.contains("one"), "{out}");
    let t = texts(&doc);
    assert_eq!(&t[1..4], ["Title", "one", "two"]);
}

/// The labels of a document's list items, by the item's text: (text, colour, font).
fn labels(doc: &mut Document) -> Vec<(String, Option<String>, Option<String>)> {
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push((p.id, p.plain_text())));
    let all = serde_json::to_value(doc.render_all()).unwrap();
    let mut out = Vec::new();
    for b in all["blocks"].as_array().unwrap() {
        let Some(l) = b.get("list").filter(|l| !l.is_null()) else { continue };
        let text = ids.iter().find(|(id, _)| b["id"].as_u64() == Some(*id as u64)).map(|(_, t)| t.clone()).unwrap_or_default();
        out.push((text, l["color"].as_str().map(str::to_string), l["font"].as_str().map(str::to_string)));
    }
    out
}

#[test]
fn google_docs_lists_keep_black_text_and_bullets_in_a_document_whose_list_style_is_blue() {
    let html = r#"<b id="docs-internal-guid-1"><ul><li style="list-style-type:disc;font-size:11pt;font-family:Calibri,sans-serif;color:#000000;"><p role="presentation"><span style="font-size:11pt;font-family:Calibri,sans-serif;color:#000000;">Bullet</span></p></li><ul><li style="list-style-type:circle;font-family:Calibri,sans-serif;color:#000000;"><p role="presentation"><span style="font-family:Calibri,sans-serif;color:#000000;">Nested</span></p></li></ul></ul><ol><li style="list-style-type:decimal;font-family:Calibri,sans-serif;color:#000000;"><p role="presentation"><span style="font-family:Calibri,sans-serif;color:#000000;">Numbered</span></p></li></ol></b>"#;
    let f = frag(html);
    // The item's font, size and colour go on its paragraph mark, which Word draws the bullet or number with.
    assert_eq!(f.body.matches(r#"<w:color w:val="000000"/>"#).count(), 6, "{}", f.body);
    // A document whose List Paragraph style is blue and in another font.
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&serde_json::json!({"op":"set_style_definition","style":"List Paragraph","def":{"rpr":{"color":"0000FF","font":"Corbel"}}}).to_string(), false).unwrap();
    doc.apply(&Op::InsertFragment { at: Pos::new(first, 0), fragment: f.clone(), mode: PasteMode::Keep }, false).unwrap();
    let xml = String::from_utf8(Package::open(&doc.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.matches(r#"<w:color w:val="000000"/>"#).count() >= 6, "{xml}");
    let l = labels(&mut doc);
    assert_eq!(l.len(), 3, "{l:?}");
    for (text, color, font) in &l {
        assert_eq!(color.as_deref(), Some("000000"), "{text}: {l:?}");
        // A bullet keeps its own glyph font; a number is in the item's font.
        if text == "Numbered" {
            assert_eq!(font.as_deref(), Some("Calibri"), "{l:?}");
        }
        assert_ne!(font.as_deref(), Some("Corbel"), "{text}: {l:?}");
    }
    // Merge formatting: the document's look, bullets included.
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply(&Op::InsertFragment { at: Pos::new(first, 0), fragment: f, mode: PasteMode::Merge }, false).unwrap();
    let xml = String::from_utf8(Package::open(&doc.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(!xml.contains("w:color") && !xml.contains("Calibri"), "{xml}");
}

#[test]
fn google_docs_checklist_pastes_as_a_checklist() {
    let img = format!(r#"<img width="11.37px" height="11.37px" alt="unchecked" aria-roledescription="checkbox" src="data:image/png;base64,{PNG}" style="margin-right:3px;" />"#);
    let html = format!(
        r#"<b id="docs-internal-guid-1"><ul class="lst-kix_x"><li role="checkbox" aria-checked="false" style="list-style-type:none;" aria-level="1">{img}<p style="display:inline-block;" role="presentation"><span>To do</span></p></li><li role="checkbox" aria-checked="true" style="list-style-type:none;" aria-level="1">{}<p style="display:inline-block;" role="presentation"><span style="text-decoration:line-through;">Done</span></p></li></ul></b>"#,
        img.replace("unchecked", "checked")
    );
    let f = frag(&html);
    let (mut doc, xml) = pasted(&f);
    assert_eq!(texts(&doc).iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>(), ["To do", "Done"]);
    assert!(!xml.contains("<w:drawing"), "the checkbox picture is the list's box, not a picture: {xml}");
    let all = serde_json::to_value(doc.render_all()).unwrap();
    let boxes: Vec<(bool, bool)> = all["blocks"].as_array().unwrap().iter().filter_map(|b| b.get("list").filter(|l| !l.is_null())).map(|l| (l["checklist"].as_bool().unwrap_or(false), l["checked"].as_bool().unwrap_or(false))).collect();
    assert_eq!(boxes, [(true, false), (true, true)]);
}

#[test]
fn google_docs_numbered_headings_keep_their_spacing_and_line_up_past_nine() {
    // Twelve 20 pt Heading 1 items in a numbered list, as Google Docs copies them, with its 1.15 line spacing.
    let item = |t: &str| format!(r#"<li style="list-style-type:decimal;font-size:20pt;font-family:Calibri,sans-serif;color:#000000;" aria-level="1"><h1 style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:20pt;font-family:Calibri,sans-serif;color:#000000;">{t}</span></h1></li>"#);
    let items: String = (1..=12).map(|i| item(&format!("Item {i}"))).collect();
    let f = frag(&format!(r#"<b id="docs-internal-guid-1"><ol style="margin-top:0;margin-bottom:0;padding-inline-start:48px;">{items}</ol></b>"#));
    let b = &f.body;
    assert_eq!(b.matches(r#"w:pStyle w:val="Heading1""#).count(), 12, "{b}");
    // The heading style's space above is not added: Google Docs said none.
    assert_eq!(b.matches(r#"<w:spacing w:before="0" w:after="0" w:line="276" w:lineRule="auto"/>"#).count(), 12, "{b}");
    // "12." at 20 pt is wider than the usual quarter inch for the number, so the number gets the room it needs and
    // every item's text starts at the same place.
    let numbering = &f.numbering;
    let ind = numbering.split("<w:lvl ").nth(1).unwrap();
    let hanging: i64 = ind.split(r#"w:hanging=""#).nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    let left: i64 = ind.split(r#"w:left=""#).nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    assert!(hanging >= 600 && left >= hanging, "{ind}");
    // Merge formatting: the document's spacing.
    assert!(!f.merged().body.contains("w:spacing"));
}

#[test]
fn a_pasted_heading_keeps_a_normal_weight_and_upright_text() {
    let html = r#"<h4 style="margin:0"><span style="font-size:14pt;font-weight:400;font-style:normal;color:#434343">Plain heading</span></h4><p><span style="font-weight:400">Body</span></p>"#;
    let frag = docxtor_core::html_import::html_to_fragment(html, &Default::default());
    assert!(frag.body.contains(r#"<w:b w:val="0"/>"#) && frag.body.contains(r#"<w:i w:val="0"/>"#), "{}", frag.body);
    // Body text is unchanged: nothing is written for a plain weight outside headings.
    let body_part = frag.body.split("Body").next().unwrap().rsplit("<w:p>").next().unwrap().to_string();
    assert!(!body_part.contains("w:val=\"0\""), "{body_part}");
    // And the engine pastes it into a document whose heading style is bold and italic, keeping what it said.
    let mut doc = docxtor_core::Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&serde_json::json!({"op":"insert_fragment","at":{"p":first,"o":0},"fragment":frag}).to_string(), false).unwrap();
    let xml = String::from_utf8(docxtor_core::package::Package::open(&doc.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#"<w:b w:val="0"/>"#), "{xml}");
}
