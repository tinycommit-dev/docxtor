//! The Tools menu's engine: the accessibility check, comparing documents into a redline, document
//! properties (the title) and the document language.

use docxtor_core::a11y::{contrast_ratio, parse_hex, A11yIssue};
use docxtor_core::ai::call_tool;
use docxtor_core::compare::{compare_view, redline};
use docxtor_core::model::{walk_paragraphs, write_block, find_block};
use docxtor_core::render::{RBlock, RSeg};
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

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

/// A document holding Markdown (headings, lists, tables, links, **bold**), with a title so the check is quiet.
fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

/// A blank document whose body is the given XML (a section is added).
fn docx_from_body(body: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn titled(mut doc: Document) -> Document {
    ops(&mut doc, json!({"op":"set_doc_props","title":"A title"}));
    doc
}

fn issues(doc: &Document) -> Vec<A11yIssue> {
    doc.accessibility()
}

fn of_kind(doc: &Document, kind: &str) -> Vec<A11yIssue> {
    issues(doc).into_iter().filter(|i| i.kind == kind).collect()
}

fn kinds(doc: &Document) -> Vec<&'static str> {
    issues(doc).iter().map(|i| i.kind).collect()
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

fn saved_part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

fn run(text: &str, props: &str) -> String {
    format!(r#"<w:r><w:rPr>{props}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>"#)
}

fn para(inner: &str) -> String {
    format!("<w:p>{inner}</w:p>")
}

fn picture(descr: Option<&str>, in_doc_pr: &str, anchor: bool) -> String {
    let d = descr.map(|d| format!(r#" descr="{d}""#)).unwrap_or_default();
    let (open, close) = if anchor { ("wp:anchor", "wp:anchor") } else { ("wp:inline", "wp:inline") };
    format!(
        r#"<w:r><w:drawing><{open}><wp:extent cx="914400" cy="914400"/><wp:docPr id="7" name="Picture 7"{d}>{in_doc_pr}</wp:docPr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rId99"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></{close}></w:drawing></w:r>"#
    )
}

// ---------------------------------------------------------------------------
// Accessibility
// ---------------------------------------------------------------------------

#[test]
fn a_tidy_document_has_no_issues() {
    let doc = titled(doc_with("# Report\n\nSome text with a [link to the budget](https://example.com/budget).\n\n## Details\n\nMore text."));
    assert_eq!(issues(&doc), []);
    assert!(doc.accessibility_report().contains("No accessibility problems"));
}

#[test]
fn images_need_alt_text_unless_decorative() {
    let decorative = r#"<a:extLst><a:ext uri="{C183D7F6-B498-43B3-948B-1728B52AA6E4}"><adec:decorative xmlns:adec="http://schemas.microsoft.com/office/drawing/2017/decorative" val="1"/></a:ext></a:extLst>"#;
    let body = [
        para(&format!("{}{}", run("Before ", ""), picture(None, "", false))),
        para(&picture(Some("A bar chart of sales"), "", false)),
        para(&picture(Some("   "), "", false)),
        para(&picture(None, decorative, false)),
        para(&picture(None, "", true)),
    ]
    .concat();
    let doc = titled(docx_from_body(&body));
    let found = of_kind(&doc, "missing_alt");
    assert_eq!(found.len(), 3, "{found:?}");
    let ids: Vec<u32> = found.iter().map(|i| i.p.unwrap()).collect();
    let all = docxtor_core::model::paragraph_order(&doc.body);
    assert_eq!(ids, [all[0], all[2], all[4]], "unmarked, blank-description and floating pictures");
    // The image sits after "Before " (7 characters).
    assert_eq!((found[0].severity, found[0].o), ("error", Some(7)));
    assert_eq!(found[1].o, Some(0));
    assert!(found[0].message.contains("alt text"));
}

#[test]
fn a_fix_for_missing_alt_text_is_an_op() {
    let mut doc = titled(docx_from_body(&para(&picture(None, "", false))));
    let img = of_kind(&doc, "missing_alt").remove(0);
    ops(&mut doc, json!({"op":"set_image","at":{"p":img.p.unwrap(),"o":img.o.unwrap()},"alt":"A red bicycle"}));
    assert!(of_kind(&doc, "missing_alt").is_empty());
    assert!(doc.undo());
    assert_eq!(of_kind(&doc, "missing_alt").len(), 1);
}

#[test]
fn images_in_headers_are_checked() {
    let mut doc = titled(doc_with("# Title\n\nBody."));
    let r = call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","text":"Company"})).unwrap();
    assert!(r.contains("Company") || !r.is_empty());
    let header = doc.stories.iter().find(|s| s.kind == docxtor_core::stories::StoryKind::Header).unwrap();
    let hp = docxtor_core::model::paragraph_order(&header.blocks)[0];
    // Add an image run to the header paragraph through the package: reopen with edited XML.
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let part = pkg.parts.iter().find(|p| p.name.starts_with("word/header")).unwrap().name.clone();
    let xml = saved_part(&pkg.to_bytes().unwrap(), &part);
    let xml = xml.replacen("</w:p>", &format!("{}</w:p>", picture(None, "", false)), 1);
    let xml = xml.replacen("<w:hdr ", r#"<w:hdr xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#, 1);
    pkg.set(&part, xml.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let found = of_kind(&doc, "missing_alt");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(doc.stories.iter().any(|s| docxtor_core::model::find_block(&s.blocks, found[0].p.unwrap()).is_some()));
    let _ = hp;
}

#[test]
fn heading_levels_must_not_skip() {
    let doc = titled(doc_with("# One\n\n### Three\n\n## Two\n\n#### Four"));
    let found = of_kind(&doc, "heading_skip");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].detail.as_deref(), Some("Heading 1 to Heading 3"));
    assert_eq!(found[0].p, Some(find_p(&doc, "Three")));
    assert_eq!(found[1].detail.as_deref(), Some("Heading 2 to Heading 4"));
    assert_eq!(found[0].severity, "warning");
    // Going back up, or down a level at a time, is fine.
    let ok = titled(doc_with("# One\n\n## Two\n\n### Three\n\n# Again\n\n## Sub"));
    assert!(of_kind(&ok, "heading_skip").is_empty());
}

#[test]
fn empty_headings_and_headingless_long_documents() {
    let mut doc = titled(doc_with("# Intro\n\nText"));
    let head = find_p(&doc, "Intro");
    ops(&mut doc, json!({"op":"delete","from":{"p":head,"o":0},"to":{"p":head,"o":5}}));
    let found = of_kind(&doc, "empty_heading");
    assert_eq!((found.len(), found[0].p), (1, Some(head)));

    let long = format!("{}\n", "word ".repeat(1600));
    let doc = titled(doc_with(&long));
    let found = of_kind(&doc, "no_headings");
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].severity, found[0].p), ("tip", None));
    // Short documents, and long ones with a heading, are fine.
    assert!(of_kind(&titled(doc_with(&"word ".repeat(100))), "no_headings").is_empty());
    assert!(of_kind(&titled(doc_with(&format!("# Head\n\n{long}"))), "no_headings").is_empty());
}

#[test]
fn the_title_is_a_document_property_you_can_set() {
    let mut doc = doc_with("# Report\n\nText");
    assert_eq!(of_kind(&doc, "no_title").len(), 1);
    assert_eq!(of_kind(&doc, "no_title")[0].severity, "tip");
    ops(&mut doc, json!({"op":"set_doc_props","title":"Quarterly report"}));
    assert!(of_kind(&doc, "no_title").is_empty());
    assert_eq!(doc.doc_props().title.as_deref(), Some("Quarterly report"));
    // An empty title counts as none.
    ops(&mut doc, json!({"op":"set_doc_props","title":"  "}));
    assert_eq!(of_kind(&doc, "no_title").len(), 1);
}

#[test]
fn tables_need_a_header_row_and_no_merged_cells() {
    let mut doc = titled(doc_with("# T\n\n| Name | Qty |\n| --- | --- |\n| Apple | 3 |\n| Pear | 2 |"));
    // Tables made from Markdown have a header row already; take it off first.
    let first = find_p(&doc, "Name");
    assert!(of_kind(&doc, "table_header").is_empty());
    ops(&mut doc, json!({"op":"table_props","at":first,"header":false}));
    let found = of_kind(&doc, "table_header");
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].severity, found[0].p), ("warning", Some(find_p(&doc, "Name"))));
    let name = find_p(&doc, "Name");
    ops(&mut doc, json!({"op":"table_props","at":name,"header":true}));
    assert!(of_kind(&doc, "table_header").is_empty());

    // A one-row table needs no header; merged cells are flagged.
    let one = titled(doc_with("# T\n\n| Only | Row |\n| --- | --- |"));
    assert!(of_kind(&one, "table_header").is_empty());
    let cell = |span: &str, t: &str| format!(r#"<w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/>{span}</w:tcPr>{}</w:tc>"#, para(&run(t, "")));
    let tbl = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:trPr><w:tblHeader/></w:trPr>{}</w:tr><w:tr>{}{}</w:tr></w:tbl>"#,
        cell(r#"<w:gridSpan w:val="2"/>"#, "Wide heading"),
        cell("", "a"),
        cell("", "b")
    );
    let doc = titled(docx_from_body(&format!("{tbl}{}", para(""))));
    assert!(of_kind(&doc, "table_header").is_empty(), "the first row repeats as a header");
    let merged = of_kind(&doc, "merged_cells");
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].p, Some(find_p(&doc, "Wide heading")));
}

#[test]
fn vague_link_text_is_flagged() {
    let doc = titled(doc_with(
        "# L\n\nSee [click here](https://a.example), [Here](https://b.example), [https://c.example/page](https://c.example/page), [Read more.](https://d.example), [the annual report](https://e.example), [link](https://f.example) and [www.g.example](https://g.example).",
    ));
    let found = of_kind(&doc, "link_text");
    let details: Vec<_> = found.iter().map(|i| i.detail.clone().unwrap()).collect();
    assert_eq!(details, ["click here", "Here", "https://c.example/page", "Read more.", "link", "www.g.example"]);
    assert!(found.iter().all(|i| i.severity == "warning" && i.p == Some(find_p(&doc, "See")) && i.o.is_some()));
    assert_eq!(found[0].o, Some(4));
}

#[test]
fn contrast_math_matches_wcag() {
    let white = (255, 255, 255);
    let ratio = |hex: &str| contrast_ratio(parse_hex(hex).unwrap(), white);
    assert!((ratio("000000") - 21.0).abs() < 1e-9);
    assert!(ratio("777777") < 4.5, "#777777 on white is 4.48:1 and fails");
    assert!((ratio("777777") - 4.48).abs() < 0.01);
    assert!(ratio("767676") >= 4.5, "#767676 on white passes");
    assert!((ratio("999999") - 2.85).abs() < 0.01);
}

fn coloured(color: &str, size_half_points: u32, bold: bool, text: &str) -> String {
    let b = if bold { "<w:b/>" } else { "" };
    run(text, &format!(r#"{b}<w:color w:val="{color}"/><w:sz w:val="{size_half_points}"/>"#))
}

#[test]
fn low_contrast_text_is_an_error_with_the_colours() {
    let body = [
        para(&coloured("777777", 22, false, "Fails")),
        para(&coloured("767676", 22, false, "Passes")),
        para(&coloured("999999", 22, false, "Light grey")),
        // Large text needs only 3:1: 18 pt, or 14 pt bold.
        para(&coloured("777777", 36, false, "Large")),
        para(&coloured("999999", 36, false, "Large but too pale")),
        para(&coloured("777777", 28, true, "Bold 14")),
        para(&coloured("777777", 28, false, "Plain 14")),
        // Automatic colour is ignored.
        para(&run("Auto", r#"<w:color w:val="auto"/>"#)),
    ]
    .concat();
    let doc = titled(docx_from_body(&body));
    let found = of_kind(&doc, "contrast");
    let got: Vec<(String, String)> = found.iter().map(|i| (i.detail.clone().unwrap(), i.severity.to_string())).collect();
    assert_eq!(
        got,
        [
            ("#777777 on #FFFFFF (4.4:1)".to_string(), "error".to_string()),
            ("#999999 on #FFFFFF (2.8:1)".to_string(), "error".to_string()),
            ("#999999 on #FFFFFF (2.8:1)".to_string(), "error".to_string()),
            ("#777777 on #FFFFFF (4.4:1)".to_string(), "error".to_string()),
        ]
    );
    assert_eq!(found[0].p, Some(find_p(&doc, "Fails")));
    assert_eq!(found[2].p, Some(find_p(&doc, "Large but")));
    assert_eq!(found[3].p, Some(find_p(&doc, "Plain 14")));
}

#[test]
fn contrast_uses_the_shading_behind_the_text_and_groups_runs() {
    let shaded_cell = |fill: &str, content: String| {
        format!(
            r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="{fill}"/></w:tcPr>{content}</w:tc></w:tr></w:tbl>"#
        )
    };
    let body = [
        shaded_cell("333333", para(&coloured("FFFFFF", 22, false, "White on dark"))),
        para(""),
        shaded_cell("333333", para(&coloured("555555", 22, false, "Grey on dark"))),
        para(""),
        // Paragraph shading and a highlight.
        format!(r#"<w:p><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="000000"/></w:pPr>{}</w:p>"#, coloured("222222", 22, false, "Dark on black")),
        para(&run("Yellow", r#"<w:color w:val="FFFF99"/><w:highlight w:val="yellow"/>"#)),
        // Several runs of the same failing colour are one issue; a passing run in between splits them.
        para(&[coloured("999999", 22, false, "one "), coloured("999999", 22, true, "two "), coloured("000000", 22, false, "ok "), coloured("999999", 22, false, "three")].concat()),
    ]
    .concat();
    let doc = titled(docx_from_body(&body));
    let got: Vec<(String, Option<usize>)> = of_kind(&doc, "contrast").iter().map(|i| (i.detail.clone().unwrap(), i.o)).collect();
    assert_eq!(
        got,
        [
            ("#555555 on #333333 (1.6:1)".to_string(), Some(0)),
            ("#222222 on #000000 (1.3:1)".to_string(), Some(0)),
            ("#FFFF99 on #FFFF00 (1.0:1)".to_string(), Some(0)),
            ("#999999 on #FFFFFF (2.8:1)".to_string(), Some(0)),
            ("#999999 on #FFFFFF (2.8:1)".to_string(), Some(11)),
        ]
    );
}

#[test]
fn contrast_uses_the_page_colour() {
    let mut doc = titled(docx_from_body(&para(&coloured("333333", 22, false, "Dark grey"))));
    assert!(of_kind(&doc, "contrast").is_empty());
    ops(&mut doc, json!({"op":"set_page_color","color":"222222"}));
    assert_eq!(of_kind(&doc, "contrast").len(), 1);
    assert!(of_kind(&doc, "contrast")[0].detail.as_deref().unwrap().starts_with("#333333 on #222222"));
}

#[test]
fn typed_lists_and_blank_lines_are_tips() {
    let body = [
        para(&run("- first", "")),
        para(&run("- second", "")),
        para(&run("• bullet", "")),
        para(&run("A normal paragraph.", "")),
        para(&run("1. Numbered", "")),
        para(&run("2) Also", "")),
        para(&run("2023. A year to remember", "")),
        para(&run("Text before", "")),
        para(""),
        para(""),
        para(""),
        para(&run("Text after two blanks", "")),
        para(""),
        para(""),
        para(&run("End", "")),
    ]
    .concat();
    let doc = titled(docx_from_body(&body));
    let fake = of_kind(&doc, "fake_list");
    assert_eq!(fake.len(), 2, "{fake:?}");
    // Consecutive typed items are one issue at the first, counting them.
    assert_eq!((fake[0].p, fake[0].detail.as_deref(), fake[0].severity), (Some(find_p(&doc, "first")), Some("3"), "tip"));
    assert_eq!((fake[1].p, fake[1].detail.as_deref()), (Some(find_p(&doc, "Numbered")), Some("2")));
    let blank = of_kind(&doc, "blank_lines");
    assert_eq!(blank.len(), 1, "two blank lines are not enough");
    assert_eq!(blank[0].detail.as_deref(), Some("3"));
    assert_eq!(blank[0].p, Some(docxtor_core::model::paragraph_order(&doc.body)[8]));

    // Real lists and page breaks are not typed lists or blank lines.
    let real = titled(doc_with("# H\n\n- one\n- two\n\n1. a\n2. b"));
    assert!(of_kind(&real, "fake_list").is_empty());
    let breaks = titled(docx_from_body(&[para(&run("x", "")), "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>".repeat(3), para(&run("y", ""))].concat()));
    assert!(of_kind(&breaks, "blank_lines").is_empty());
}

#[test]
fn language_is_a_tip_until_set() {
    let mut doc = titled(doc_with("# H\n\nText"));
    assert!(of_kind(&doc, "language").is_empty(), "the blank template has a language");
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let styles = saved_part(&pkg.to_bytes().unwrap(), "word/styles.xml").replace(r#"<w:lang w:val="en-US"/>"#, "");
    pkg.set("word/styles.xml", styles.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let found = of_kind(&doc, "language");
    assert_eq!((found.len(), found[0].severity, found[0].p), (1, "tip", None));
    ops(&mut doc, json!({"op":"set_language","lang":"fr-FR"}));
    assert!(of_kind(&doc, "language").is_empty());
}

#[test]
fn issues_are_in_document_order_and_serialize_as_json() {
    let body = [para(&picture(None, "", false)), para(&coloured("999999", 22, false, "Pale")), para(&picture(None, "", false))].concat();
    let doc = docx_from_body(&body);
    let k = kinds(&doc);
    assert_eq!(k, ["missing_alt", "contrast", "missing_alt", "no_title"]);
    let v = serde_json::to_value(issues(&doc)).unwrap();
    assert_eq!(v[0]["kind"], "missing_alt");
    assert_eq!(v[0]["severity"], "error");
    assert!(v[0]["p"].is_number() && v[0]["o"].is_number() && v[0].get("detail").is_none());
    assert!(v[3].get("p").is_none());
    let report = doc.accessibility_report();
    assert!(report.contains("error missing_alt [p") && report.contains("(#999999 on #FFFFFF"), "{report}");
}

#[test]
fn the_ai_tool_reports_issues_with_ids() {
    let mut doc = docx_from_body(&para(&picture(None, "", false)));
    let r = call_tool(&mut doc, "check_accessibility", &json!({})).unwrap();
    assert!(r.contains("missing_alt") && r.contains("[p") && r.contains("no_title"), "{r}");
    call_tool(&mut doc, "set_document_properties", &json!({"title":"Photos"})).unwrap();
    let r = call_tool(&mut doc, "check_accessibility", &json!({})).unwrap();
    assert!(!r.contains("no_title"), "{r}");
    assert!(call_tool(&mut doc, "set_document_properties", &json!({})).is_err());
    assert!(docxtor_core::ai::tool_definitions().as_array().unwrap().iter().any(|t| t["name"] == "check_accessibility"));
}

// ---------------------------------------------------------------------------
// Document properties
// ---------------------------------------------------------------------------

fn core_xml(bytes: &[u8]) -> Option<String> {
    Package::open(bytes).unwrap().get("docProps/core.xml").map(|b| String::from_utf8(b.to_vec()).unwrap())
}

#[test]
fn setting_the_title_changes_only_the_title() {
    let mut original = doc_with("# T\n\nBody");
    let bytes = original.save().unwrap();
    let before = Package::open(&bytes).unwrap();
    let mut doc = Document::open(&bytes).unwrap();
    ops(&mut doc, json!({"op":"set_doc_props","title":"My <title> & more"}));
    assert!(doc.is_modified());
    let out = doc.save().unwrap();
    assert_valid(&out);
    let after = Package::open(&out).unwrap();
    for part in &before.parts {
        if part.name != "docProps/core.xml" {
            assert_eq!(after.get(&part.name), Some(part.data.as_slice()), "{} must stay byte-identical", part.name);
        }
    }
    assert_eq!(after.parts.len(), before.parts.len());
    let core = core_xml(&out).unwrap();
    assert!(core.contains("<dc:title>My &lt;title&gt; &amp; more</dc:title>") && core.contains("<dc:creator>Docxtor</dc:creator>"), "{core}");
    assert_eq!(Document::open(&out).unwrap().doc_props().title.as_deref(), Some("My <title> & more"));
    // Other properties, and removing one.
    ops(&mut doc, json!({"op":"set_doc_props","subject":"Q3","keywords":"a, b","author":"Ana","description":"Notes"}));
    let p = doc.doc_props();
    assert_eq!((p.title.as_deref(), p.subject.as_deref(), p.author.as_deref(), p.keywords.as_deref(), p.description.as_deref()), (Some("My <title> & more"), Some("Q3"), Some("Ana"), Some("a, b"), Some("Notes")));
    ops(&mut doc, json!({"op":"set_doc_props","title":""}));
    assert!(doc.doc_props().title.is_none());
    assert!(!core_xml(&doc.save().unwrap()).unwrap().contains("dc:title"));
}

#[test]
fn the_title_is_undoable() {
    let mut doc = doc_with("Body");
    let was = doc.doc_props().title;
    ops(&mut doc, json!({"op":"set_doc_props","title":"First"}));
    ops(&mut doc, json!({"op":"set_doc_props","title":"Second"}));
    assert!(doc.undo());
    assert_eq!(doc.doc_props().title.as_deref(), Some("First"));
    assert!(core_xml(&doc.save().unwrap()).unwrap().contains("First"));
    assert!(doc.undo());
    assert_eq!(doc.doc_props().title, was);
    assert!(doc.redo());
    assert_eq!(doc.doc_props().title.as_deref(), Some("First"));
}

#[test]
fn a_file_without_core_properties_gets_them() {
    let bytes = Document::new_blank().save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    pkg.parts.retain(|p| p.name != "docProps/core.xml");
    let rels = saved_part(&pkg.to_bytes().unwrap(), "_rels/.rels");
    let rels = rels.replace(r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#, "");
    pkg.set("_rels/.rels", rels.into_bytes());
    let types = saved_part(&pkg.to_bytes().unwrap(), "[Content_Types].xml").replace(r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#, "");
    pkg.set("[Content_Types].xml", types.into_bytes());
    let stripped = pkg.to_bytes().unwrap();
    assert!(core_xml(&stripped).is_none());
    assert!(!saved_part(&stripped, "[Content_Types].xml").contains("core.xml"));

    let mut doc = Document::open(&stripped).unwrap();
    assert!(doc.doc_props().title.is_none());
    assert_eq!(of_kind(&doc, "no_title").len(), 1);
    ops(&mut doc, json!({"op":"set_doc_props","title":"Brand new"}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert!(core_xml(&out).unwrap().contains("<dc:title>Brand new</dc:title>"));
    assert!(saved_part(&out, "[Content_Types].xml").contains(r#"PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml""#));
    let rels = saved_part(&out, "_rels/.rels");
    assert!(rels.contains("metadata/core-properties") && rels.contains(r#"Target="docProps/core.xml""#), "{rels}");
    assert_eq!(Document::open(&out).unwrap().doc_props().title.as_deref(), Some("Brand new"));
    // Undo before saving leaves no part behind.
    let mut doc = Document::open(&stripped).unwrap();
    ops(&mut doc, json!({"op":"set_doc_props","title":"x"}));
    assert!(doc.undo());
    assert!(core_xml(&doc.save().unwrap()).is_none());
}

#[test]
fn existing_core_properties_and_other_namespaces_are_kept() {
    let bytes = Document::new_blank().save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    let core = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:x="http://purl.org/dc/elements/1.1/"><x:creator>Ana</x:creator><dcterms:created>2024-01-02T03:04:05Z</dcterms:created><cp:lastModifiedBy>Ana</cp:lastModifiedBy></cp:coreProperties>"#;
    pkg.set("docProps/core.xml", core.as_bytes().to_vec());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert_eq!(doc.doc_props().author.as_deref(), Some("Ana"));
    ops(&mut doc, json!({"op":"set_doc_props","title":"Hi"}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let core = core_xml(&out).unwrap();
    // The document binds the Dublin Core namespace to `x`; the title uses that prefix.
    assert!(core.contains("<x:title>Hi</x:title>") && core.contains("<dcterms:created>2024-01-02T03:04:05Z</dcterms:created>") && core.contains("<cp:lastModifiedBy>Ana</cp:lastModifiedBy>"), "{core}");
}

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

#[test]
fn the_document_language_is_written_in_schema_order() {
    let mut doc = doc_with("Hola");
    assert_eq!(doc.page_setup().lang.as_deref(), Some("en-US"));
    ops(&mut doc, json!({"op":"set_language","lang":"es-es"}));
    assert_eq!(doc.page_setup().lang.as_deref(), Some("es-ES"));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let styles = saved_part(&out, "word/styles.xml");
    assert!(styles.contains(r#"<w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="es-ES"/></w:rPr></w:rPrDefault>"#), "{styles}");
    assert_eq!(Document::open(&out).unwrap().page_setup().lang.as_deref(), Some("es-ES"));
    // The body and everything else are untouched.
    let before = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    assert_eq!(Package::open(&out).unwrap().get("word/settings.xml"), before.get("word/settings.xml"));
    assert!(doc.undo());
    assert_eq!(doc.page_setup().lang.as_deref(), Some("en-US"));
    assert!(doc.redo());
    assert_eq!(doc.page_setup().lang.as_deref(), Some("es-ES"));
    // A bad tag changes nothing.
    assert!(err(&mut doc, json!({"op":"set_language","lang":"english"})).contains("language tag"));
    assert_eq!(doc.page_setup().lang.as_deref(), Some("es-ES"));
}

#[test]
fn the_language_is_created_in_order_and_keeps_other_attributes() {
    let bytes = Document::new_blank().save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    // No lang at all, and a defaults block without run properties.
    let styles = saved_part(&bytes, "word/styles.xml").replace(r#"<w:lang w:val="en-US"/>"#, "");
    pkg.set("word/styles.xml", styles.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert_eq!(doc.page_setup().lang, None);
    ops(&mut doc, json!({"op":"set_language","lang":"fr-FR"}));
    let out = doc.save().unwrap();
    assert!(saved_part(&out, "word/styles.xml").contains(r#"<w:szCs w:val="22"/><w:lang w:val="fr-FR"/></w:rPr>"#));

    let styles = saved_part(&bytes, "word/styles.xml").replace(r#"<w:lang w:val="en-US"/>"#, r#"<w:lang w:val="en-US" w:eastAsia="ja-JP" w:bidi="ar-SA"/>"#);
    pkg.set("word/styles.xml", styles.into_bytes());
    let mut settings = saved_part(&bytes, "word/settings.xml");
    settings = settings.replace("</w:settings>", r#"<w:themeFontLang w:val="en-US" w:eastAsia="ja-JP"/></w:settings>"#);
    pkg.set("word/settings.xml", settings.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    ops(&mut doc, json!({"op":"set_language","lang":"de-DE"}));
    let out = doc.save().unwrap();
    assert!(saved_part(&out, "word/styles.xml").contains(r#"<w:lang w:val="de-DE" w:eastAsia="ja-JP" w:bidi="ar-SA"/>"#));
    assert!(saved_part(&out, "word/settings.xml").contains(r#"<w:themeFontLang w:val="de-DE" w:eastAsia="ja-JP"/>"#));

    // No styles part at all.
    let mut p2 = Package::open(&bytes).unwrap();
    p2.parts.retain(|p| p.name != "word/styles.xml");
    let mut doc = Document::open(&p2.to_bytes().unwrap()).unwrap();
    ops(&mut doc, json!({"op":"set_language","lang":"it-IT"}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert_eq!(Document::open(&out).unwrap().page_setup().lang.as_deref(), Some("it-IT"));
}

#[test]
fn a_passage_can_have_its_own_language() {
    let mut doc = doc_with("English text\n\nTexto en español\n\nMore English");
    let es = find_p(&doc, "Texto");
    ops(&mut doc, json!({"op":"set_language","lang":"es-ES","ids":[es]}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let reopened = Document::open(&out).unwrap();
    let xml = |id| write_block(find_block(&reopened.body, id).unwrap()).to_xml();
    assert!(xml(find_p(&reopened, "Texto")).contains(r#"<w:lang w:val="es-ES"/>"#), "{}", xml(find_p(&reopened, "Texto")));
    assert!(!xml(find_p(&reopened, "English text")).contains("w:lang"));
    // The document default is unchanged.
    assert_eq!(reopened.page_setup().lang.as_deref(), Some("en-US"));
    assert!(doc.undo());
    assert!(!write_block(find_block(&doc.body, es).unwrap()).to_xml().contains("w:lang"));
    // The AI tool.
    let r = call_tool(&mut doc, "set_language", &json!({"lang":"fr-FR"})).unwrap();
    assert!(r.contains("fr-FR"));
    assert_eq!(doc.page_setup().lang.as_deref(), Some("fr-FR"));
    let r = call_tool(&mut doc, "set_language", &json!({"lang":"es-ES","ids":[format!("p{es}")]})).unwrap();
    assert!(r.contains("1 paragraph"));
    assert!(call_tool(&mut doc, "set_language", &json!({"lang":"??"})).is_err());
}

// ---------------------------------------------------------------------------
// Compare and redline
// ---------------------------------------------------------------------------

const AUTHOR: &str = "Tester";
const DATE: &str = "2026-10-12T09:30:00Z";

fn redlined(old: &mut Document, new: &mut Document) -> (Vec<u8>, String) {
    let _ = (&old, &new);
    let bytes = redline(old, new, AUTHOR, DATE).unwrap();
    assert_valid(&bytes);
    let xml = saved_part(&bytes, "word/document.xml");
    (bytes, xml)
}

fn count(xml: &str, needle: &str) -> usize {
    xml.matches(needle).count()
}

/// The `w:id` of every revision, in order.
fn rev_ids(xml: &str) -> Vec<u32> {
    let mut v = Vec::new();
    for tag in ["<w:ins ", "<w:del "] {
        for part in xml.split(tag).skip(1) {
            let id = part.split("w:id=\"").nth(1).unwrap().split('"').next().unwrap();
            v.push(id.parse().unwrap());
        }
    }
    v
}

#[test]
fn a_changed_word_is_one_deletion_and_one_insertion() {
    let mut old = doc_with("The quick brown fox jumps over the lazy dog.");
    let mut new = doc_with("The slow brown fox jumps over the lazy dog.");
    let (_, xml) = redlined(&mut old, &mut new);
    assert_eq!(count(&xml, "<w:del "), 1, "{xml}");
    assert_eq!(count(&xml, "<w:ins "), 1);
    assert!(xml.contains(r#"<w:del w:id="#) && xml.contains(&format!(r#"w:author="{AUTHOR}" w:date="{DATE}">"#)));
    assert!(xml.contains(r#"<w:delText xml:space="preserve">quick</w:delText>"#), "{xml}");
    let ins = xml.split("<w:ins ").nth(1).unwrap().split("</w:ins>").next().unwrap();
    assert!(ins.contains(&format!(r#"w:author="{AUTHOR}" w:date="{DATE}""#)) && ins.contains("<w:t>slow</w:t>"), "{ins}");
    // The deletion comes before the insertion, in the middle of the sentence.
    let (d, i) = (xml.find("<w:del ").unwrap(), xml.find("<w:ins ").unwrap());
    assert!(xml.find("The ").unwrap() < d && d < i && i < xml.find(" brown fox").unwrap());
    let ids = rev_ids(&xml);
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn unchanged_documents_have_no_revisions() {
    let mut old = doc_with("# Same\n\nExactly the same text.\n\n- a\n- b");
    let mut new = doc_with("# Same\n\nExactly the same text.\n\n- a\n- b");
    let (bytes, xml) = redlined(&mut old, &mut new);
    assert_eq!(count(&xml, "<w:ins ") + count(&xml, "<w:del "), 0, "{xml}");
    assert_eq!(texts(&Document::open(&bytes).unwrap()), texts(&new));
}

#[test]
fn added_and_removed_paragraphs_are_tracked_with_their_marks() {
    let mut old = doc_with("Alpha\n\nBeta\n\nGamma");
    let mut new = doc_with("Alpha\n\nGamma\n\nDelta");
    let (bytes, xml) = redlined(&mut old, &mut new);
    // "Beta" comes back between Alpha and Gamma as a deleted paragraph: deleted runs and a deleted mark.
    let beta = xml.find(">Beta<").unwrap();
    assert!(xml.find(">Alpha<").unwrap() < beta && beta < xml.find(">Gamma<").unwrap());
    let beta_p = xml[..beta].rfind("<w:p>").or_else(|| xml[..beta].rfind("<w:p ")).unwrap();
    let beta_p = &xml[beta_p..xml[beta..].find("</w:p>").unwrap() + beta];
    assert!(beta_p.contains("<w:pPr><w:rPr><w:del ") && beta_p.contains("<w:delText xml:space=\"preserve\">Beta</w:delText>"), "{beta_p}");
    assert!(!beta_p.contains("<w:t>") && !beta_p.contains("<w:ins "));
    // "Delta" is an inserted paragraph: inserted runs and an inserted mark.
    let delta = xml.find(">Delta<").unwrap();
    let delta_p = xml[..delta].rfind("<w:p>").or_else(|| xml[..delta].rfind("<w:p ")).unwrap();
    let delta_p = &xml[delta_p..xml[delta..].find("</w:p>").unwrap() + delta];
    assert!(delta_p.contains("<w:pPr><w:rPr><w:ins ") && delta_p.contains("<w:ins w:id=") && delta_p.contains("<w:t>Delta</w:t>"), "{delta_p}");
    assert_eq!(count(&xml, "<w:ins "), 2, "the mark and the run");
    assert_eq!(count(&xml, "<w:del "), 2);
    // Opened in Docxtor, the redline's text is the new document's, with the deletion as a ghost.
    let red = Document::open(&bytes).unwrap();
    let non_empty: Vec<String> = texts(&red).into_iter().filter(|t| !t.is_empty()).collect();
    assert_eq!(non_empty, ["Alpha", "Gamma", "Delta"]);
    let r = red.render_snapshot();
    let segs: Vec<&RSeg> = r.blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { Some(p) } else { None }).flat_map(|p| p.segs.iter()).collect();
    assert!(segs.iter().any(|s| s.k == "ghost" && s.x.as_deref() == Some("Beta") && s.label.as_deref() == Some("del")));
    assert!(segs.iter().any(|s| s.ins && s.x.as_deref() == Some("Delta")));
    assert!(!segs.iter().any(|s| s.ins && s.x.as_deref() == Some("Gamma")));
}

#[test]
fn formatting_is_kept_for_kept_inserted_and_deleted_text() {
    let mut old = doc_with("Plain and **bold old** words here");
    let mut new = doc_with("Plain and *italic new* words here");
    let (bytes, xml) = redlined(&mut old, &mut new);
    // The deleted words keep the old bold formatting.
    let del = xml.split("<w:del ").nth(1).unwrap().split("</w:del>").next().unwrap();
    assert!(del.contains("<w:b/>") && del.contains("bold old"), "{del}");
    // The inserted words keep the new italic formatting.
    let ins = xml.split("<w:ins ").nth(1).unwrap().split("</w:ins>").next().unwrap();
    assert!(ins.contains("<w:i/>") && ins.contains("italic new") && !ins.contains("<w:b/>"), "{ins}");
    // Kept text keeps its (here: no) formatting and sits outside revisions.
    let red = Document::open(&bytes).unwrap();
    assert_eq!(texts(&red), ["Plain and italic new words here"]);
}

#[test]
fn a_redline_renders_insertions_and_ghost_deletions() {
    let mut old = doc_with("One two three four");
    let mut new = doc_with("One 2 three four five");
    let (bytes, _) = redlined(&mut old, &mut new);
    let red = Document::open(&bytes).unwrap();
    let out = red.render_snapshot();
    let RBlock::Para(p) = &out.blocks[0] else { panic!() };
    let ins: Vec<_> = p.segs.iter().filter(|s| s.ins).map(|s| s.x.clone().unwrap()).collect();
    let ghost: Vec<_> = p.segs.iter().filter(|s| s.k == "ghost").map(|s| (s.x.clone().unwrap(), s.o)).collect();
    assert_eq!(ins, ["2", " five"]);
    assert_eq!(ghost, [("two".to_string(), 4)]);
    assert_eq!(texts(&red), ["One 2 three four five"]);
    assert_eq!(p.len, "One 2 three four five".len());
}

#[test]
fn objects_are_not_broken_when_their_paragraph_changes() {
    let img = picture(Some("A cat"), "", false);
    let old = docx_from_body(&para(&format!("{}{}", run("Old caption ", ""), run("text", ""))));
    let new = docx_from_body(&para(&format!("{}{}", run("New caption ", ""), img)));
    let (bytes, xml) = redlined(&mut { old }, &mut { new });
    // The picture is still in the paragraph, outside any revision, exactly once.
    assert_eq!(count(&xml, "<w:drawing>"), 1, "{xml}");
    let drawing = xml.find("<w:drawing>").unwrap();
    let last_ins_end = xml.rfind("</w:ins>").unwrap();
    assert!(drawing > last_ins_end, "the image comes after the inserted text and is not wrapped: {xml}");
    assert!(xml.contains(r#"<w:delText xml:space="preserve">Old caption </w:delText>"#) && xml.contains(r#"<w:delText xml:space="preserve">text</w:delText>"#));
    assert!(xml.contains("New caption "));
    let red = Document::open(&bytes).unwrap();
    assert_eq!(texts(&red), ["New caption "]);
}

#[test]
fn a_changed_table_cell_and_an_added_row() {
    let mut old = doc_with("| Name | Qty |\n| --- | --- |\n| Apple | 3 |");
    let mut new = doc_with("| Name | Qty |\n| --- | --- |\n| Apple | 5 |\n| Pear | 2 |");
    let (bytes, xml) = redlined(&mut old, &mut new);
    // The cell "3" became "5" inside its cell.
    assert!(xml.contains(r#"<w:delText xml:space="preserve">3</w:delText>"#) && xml.contains("<w:t>5</w:t>"), "{xml}");
    // The Pear row is an inserted row; the end-of-cell marks are not tracked on their own.
    assert_eq!(count(&xml, "<w:trPr><w:ins "), 1, "{xml}");
    assert!(!xml.contains("<w:pPr><w:rPr><w:ins "), "{xml}");
    let ins: Vec<_> = xml.split("<w:ins ").skip(1).map(|s| s.split("</w:ins>").next().unwrap()).collect();
    assert!(ins.iter().any(|s| s.contains("Pear")) && ins.iter().any(|s| s.contains("<w:t>2</w:t>")));
    let red = Document::open(&bytes).unwrap();
    assert_eq!(texts(&red).into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>(), ["Name", "Qty", "Apple", "5", "Pear", "2"]);
}

#[test]
fn links_around_changed_words_stay_valid() {
    let mut old = doc_with("Read the [old report](https://example.com/r) today");
    let mut new = doc_with("Read the [new report](https://example.com/r) today");
    let (bytes, xml) = redlined(&mut old, &mut new);
    // Revisions never wrap a hyperlink (the schema doesn't allow it); the insertion sits inside it.
    let link = xml.split("<w:hyperlink ").nth(1).unwrap().split("</w:hyperlink>").next().unwrap();
    assert!(link.contains("<w:ins ") && link.contains("<w:t>new</w:t>"), "{link}");
    assert!(xml.contains(r#"<w:delText xml:space="preserve">old</w:delText>"#));
    assert!(!xml.contains("<w:ins><w:hyperlink") && !xml.split("<w:ins ").skip(1).any(|s| s.split("</w:ins>").next().unwrap().contains("<w:hyperlink")));
    assert_eq!(texts(&Document::open(&bytes).unwrap()), ["Read the new report today"]);
}

#[test]
fn revision_ids_are_unique_and_clear_of_existing_ones() {
    let mut old = doc_with("Intro paragraph\n\nOne\n\nTwo three four five six\n\nSeven");
    let mut new = doc_with("Intro paragraph\n\nOne\n\nTwo 3 four 5 six\n\nEight\n\nNine");
    let eight = find_p(&new, "Eight");
    ops(&mut new, json!({"op":"insert_bookmark","from":{"p":eight,"o":0},"to":{"p":eight,"o":2},"name":"Mark"}));
    let (_, xml) = redlined(&mut old, &mut new);
    let mut ids = rev_ids(&xml);
    assert!(ids.len() >= 6, "{xml}");
    let bookmark: u32 = xml.split("<w:bookmarkStart w:id=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    assert!(!ids.contains(&bookmark));
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), rev_ids(&xml).len());
}

#[test]
fn redlining_leaves_both_documents_alone() {
    let mut old = doc_with("A\n\nB");
    let mut new = doc_with("A\n\nC");
    let (old_before, new_before) = (old.save().unwrap(), new.save().unwrap());
    let _ = redlined(&mut old, &mut new);
    assert_eq!(old.save().unwrap(), old_before);
    assert_eq!(new.save().unwrap(), new_before);
    assert!(!new.is_modified() || new.save().is_ok());
}

#[test]
fn the_redline_keeps_headers_and_untouched_parts_of_the_new_document() {
    let mut old = doc_with("Body one");
    let mut new = doc_with("Body two");
    call_tool(&mut new, "set_header_footer", &json!({"kind":"header","text":"Header text"})).unwrap();
    let new_bytes = new.save().unwrap();
    let (bytes, _) = redlined(&mut old, &mut Document::open(&new_bytes).unwrap());
    let before = Package::open(&new_bytes).unwrap();
    let after = Package::open(&bytes).unwrap();
    for part in &before.parts {
        if part.name != "word/document.xml" {
            assert_eq!(after.get(&part.name), Some(part.data.as_slice()), "{} must be the new document's", part.name);
        }
    }
}

#[test]
fn a_big_rewrite_pairs_paragraphs_by_similarity() {
    let mut old = doc_with("Keep this line\n\nThe budget for the marketing team is large\n\nCompletely unrelated sentence about weather\n\nEnd");
    let mut new = doc_with("Keep this line\n\nA brand new paragraph on something else entirely\n\nThe budget for the marketing group is large\n\nEnd");
    let (bytes, xml) = redlined(&mut old, &mut new);
    // The marketing paragraph is edited in place (team -> group); the other two are a removal and an addition.
    assert!(xml.contains(r#"<w:delText xml:space="preserve">team</w:delText>"#), "{xml}");
    assert!(xml.contains(r#"<w:delText xml:space="preserve">Completely unrelated sentence about weather</w:delText>"#));
    assert_eq!(count(&xml, "<w:pPr><w:rPr><w:del "), 1);
    assert_eq!(count(&xml, "<w:pPr><w:rPr><w:ins "), 1);
    let red = Document::open(&bytes).unwrap();
    assert_eq!(texts(&red).into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>(), texts(&new));
}

#[test]
fn compare_view_lists_every_paragraph_with_word_level_parts() {
    let old = doc_with("Same line\n\nThe quick brown fox\n\nGone paragraph\n\nTail");
    let new = doc_with("Same line\n\nThe slow brown fox\n\nTail\n\nBrand new");
    let view = compare_view(&old, &new);
    let kinds: Vec<_> = view.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, ["same", "changed", "removed", "same", "added"]);
    let parts = |b: &docxtor_core::compare::CompareBlock| b.parts.iter().map(|p| format!("{}:{}", p.t, p.text)).collect::<Vec<_>>();
    assert_eq!(parts(&view[0]), ["same:Same line"]);
    assert_eq!(parts(&view[1]), ["same:The ", "del:quick", "ins:slow", "same: brown fox"]);
    assert_eq!(parts(&view[2]), ["del:Gone paragraph"]);
    assert_eq!(parts(&view[4]), ["ins:Brand new"]);
    assert_eq!((view[1].p, view[2].p), (Some(find_p(&new, "slow")), None));
    assert_eq!((view[1].index, view[2].index, view[3].index, view[4].index), (1, 2, 2, 3));
    let v = serde_json::to_value(&view).unwrap();
    assert_eq!(v[1]["parts"][1], json!({"t":"del","text":"quick"}));
    assert!(v[2].get("p").is_none() && v[0].get("table").is_none());
    // The methods on Document agree.
    assert_eq!(new.compare_view(&old), view);
    assert_eq!(new.compare_summary(&old).changed, 1);
}

/// The text of a redline's first paragraph with every change rejected: kept and deleted text, not inserted text.
fn rejected_text(xml: &str) -> String {
    let mut s = xml.to_string();
    while let Some(a) = s.find("<w:ins ") {
        let end = s[a..].find("</w:ins>").map(|e| a + e + "</w:ins>".len()).unwrap();
        s.replace_range(a..end, "");
    }
    let body = &s[s.find("<w:body>").unwrap()..];
    let mut out = String::new();
    let mut i = 0;
    while i < body.len() {
        let rest = &body[i..];
        let tag = if rest.starts_with("<w:t>") || rest.starts_with("<w:t ") {
            Some("</w:t>")
        } else if rest.starts_with("<w:delText") {
            Some("</w:delText>")
        } else {
            None
        };
        if let Some(close) = tag {
            let start = i + rest.find('>').unwrap() + 1;
            let end = start + body[start..].find(close).unwrap();
            out.push_str(&body[start..end].replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"));
            i = end + close.len();
        } else {
            i += rest.chars().next().unwrap().len_utf8();
        }
    }
    out
}

#[test]
fn rejecting_a_redline_gives_the_old_text_and_accepting_it_the_new() {
    let vocab = ["alpha", "beta", "gamma", "delta", "épsilon", "😀", "a-b", "don't", ",", ".", "x", "The", "of", "1,200"];
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut next = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    for round in 0..250 {
        let mut words: Vec<String> = (0..2 + next(9)).map(|_| vocab[next(vocab.len())].to_string()).collect();
        let before = words.clone();
        for _ in 0..1 + next(4) {
            match next(3) {
                0 if !words.is_empty() => {
                    words.remove(next(words.len()));
                }
                1 => words.insert(next(words.len() + 1), vocab[next(vocab.len())].to_string()),
                _ if !words.is_empty() => {
                    let k = next(words.len());
                    words[k] = vocab[next(vocab.len())].to_string();
                }
                _ => {}
            }
        }
        // Some words bold, so the paragraph has several runs.
        let md = |ws: &[String], salt: usize| ws.iter().enumerate().map(|(i, w)| if (i + salt).is_multiple_of(3) && w.chars().all(char::is_alphanumeric) { format!("**{w}**") } else { w.clone() }).collect::<Vec<_>>().join(" ");
        let (old_md, new_md) = (md(&before, round % 3), md(&words, (round / 3) % 3));
        let (old, new) = (doc_with(&old_md), doc_with(&new_md));
        let bytes = redline(&old, &new, AUTHOR, DATE).unwrap();
        assert_valid(&bytes);
        let xml = saved_part(&bytes, "word/document.xml");
        let red = Document::open(&bytes).unwrap();
        assert_eq!(texts(&red), texts(&new), "accepting: {old_md:?} -> {new_md:?}");
        assert_eq!(rejected_text(&xml), texts(&old)[0], "rejecting: {old_md:?} -> {new_md:?}\n{xml}");
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}; run python3 fixtures/make_fixtures.py"))
}

const FIXTURES: [&str; 4] = ["simple.docx", "rich.docx", "complex.docx", "long.docx"];

#[test]
fn redlines_of_real_documents_are_valid_and_read_as_the_new_document() {
    for a in FIXTURES {
        for b in FIXTURES {
            let (old, new) = (Document::open(&fixture(a)).unwrap(), Document::open(&fixture(b)).unwrap());
            let bytes = redline(&old, &new, AUTHOR, DATE).unwrap_or_else(|e| panic!("{a} vs {b}: {e}"));
            assert_valid(&bytes);
            let red = Document::open(&bytes).unwrap();
            let strip = |d: &Document| texts(d).into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>();
            assert_eq!(strip(&red), strip(&new), "{a} vs {b}");
            let xml = saved_part(&bytes, "word/document.xml");
            if a == b {
                // Tracked changes the file already has stay; nothing is added.
                let own = saved_part(&fixture(a), "word/document.xml");
                assert_eq!(count(&xml, "<w:ins ") + count(&xml, "<w:del "), count(&own, "<w:ins ") + count(&own, "<w:del "), "{a} against itself");
            }
            let mut ids = rev_ids(&xml);
            let n = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), n, "{a} vs {b}: revision ids must be unique");
        }
    }
}

#[test]
fn accessibility_runs_on_real_documents() {
    for name in FIXTURES {
        let doc = Document::open(&fixture(name)).unwrap();
        let found = doc.accessibility();
        assert!(found.iter().all(|i| ["error", "warning", "tip"].contains(&i.severity) && !i.message.is_empty()), "{name}");
        // Every issue points at a paragraph that exists.
        for p in found.iter().filter_map(|i| i.p) {
            assert!(doc.find_any_para(p).is_some(), "{name}: p{p}");
        }
    }
}
