//! Regression tests for bugs the real-world corpus found. Each test builds a
//! small file by hand that has the shape of the corpus file, never a copy of it.

use crate::common;

use common::{first_diff, normalise, render_model};
use docxtor_core::{package::Package, Document};
use std::collections::HashMap;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// A document whose body is the given XML (then a plain section).
#[allow(dead_code)]
fn docx_with_body(body: &str) -> Vec<u8> {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.to_bytes().unwrap()
}

/// E6 as the corpus run does it: the render model of the document equals that of the file it saves, reopened. The
/// body is forced to be written again (as `docxtor roundtrip` does), so the text really goes through the writer.
#[allow(dead_code)]
fn assert_same_after_reopen(bytes: &[u8]) -> Document {
    let mut doc = Document::open(bytes).unwrap();
    doc.touch_all();
    doc.apply(&docxtor_core::Op::DeleteBlocks { ids: vec![] }, false).unwrap();
    let saved = doc.save().unwrap();
    let again = Document::open(&saved).unwrap();
    if let Some(d) = first_diff(&normalise(&render_model(&doc)), &normalise(&render_model(&again))) {
        panic!("renders differently after save and reopen: {d}");
    }
    again
}

/// Item 1: The package's root relationships name the core properties part under a misspelt type (some producers do).
/// Docxtor's repair used to add a second relationship with the right type to the same part, and the Open XML SDK
/// refuses a part with two relationship types.
#[test]
fn a_misspelt_core_properties_relationship_is_not_duplicated() {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let rels = String::from_utf8(pkg.get("_rels/.rels").unwrap().to_vec()).unwrap();
    let bad = rels.replace("http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties", "http://schemas.openxmlformats.org/officedocument/2006/relationships/metadata/core-properties");
    assert_ne!(rels, bad, "the blank file has the standard relationship");
    pkg.set("_rels/.rels", bad.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let out = Package::open(&doc.save().unwrap()).unwrap();
    let mut by_target: HashMap<String, Vec<String>> = HashMap::new();
    for r in out.rels_for("").unwrap() {
        by_target.entry(r.target.clone()).or_default().push(r.rel_type.clone());
    }
    for (t, types) in &by_target {
        assert_eq!(types.len(), 1, "{t} is the target of {types:?}");
    }
}

/// Item 2: A content control marked as showing its placeholder that holds real text in two differently formatted runs
/// (LibreOffice writes these). The clean-up on save used to rebuild the text as one run: the second run's size was
/// lost and the runs were merged.
#[test]
fn a_control_with_text_and_a_stale_placeholder_flag_keeps_its_runs() {
    let body = r#"<w:p><w:sdt><w:sdtPr><w:alias w:val="a"/><w:id w:val="7"/><w:showingPlcHdr/></w:sdtPr><w:sdtContent><w:r><w:t>aaa</w:t></w:r><w:r><w:rPr><w:sz w:val="48"/></w:rPr><w:t>bbb</w:t></w:r></w:sdtContent></w:sdt></w:p>"#;
    let mut doc = Document::open(&docx_with_body(body)).unwrap();
    doc.touch_all();
    doc.apply(&docxtor_core::Op::DeleteBlocks { ids: vec![] }, false).unwrap();
    let out = Package::open(&doc.save().unwrap()).unwrap();
    let xml = String::from_utf8(out.get("word/document.xml").unwrap().to_vec()).unwrap();
    let a = xml.find("aaa").expect("aaa");
    let b = xml.find("bbb").expect("bbb");
    assert!(a < b, "order kept: {xml}");
    assert!(xml.contains(r#"<w:sz w:val="48"/>"#), "the second run keeps its size: {xml}");
    assert!(xml.matches("<w:t").count() >= 2, "two runs: {xml}");
}

/// Item 3: A line feed (or CRLF, or a tab) typed raw inside `w:t`, as some producers write it. The model kept it raw
/// while the writer turns it into `<w:br/>` (`<w:tab/>`), so the reopened document had the text in pieces.
#[test]
fn a_line_feed_inside_a_text_element_reads_as_a_line_break() {
    let body = "<w:p><w:r><w:t xml:space=\"preserve\">First (before CRLF).\r\nSecond (after).</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\">\nlead\tmid\nend</w:t></w:r></w:p>";
    let again = assert_same_after_reopen(&docx_with_body(body));
    let v = render_model(&again);
    let all = v["body"].to_string();
    assert!(all.contains("First (before CRLF)."), "{all}");
    assert!(all.contains("Second (after)."), "no text is lost: {all}");
    assert!(all.contains("lead") && all.contains("mid") && all.contains("end"), "{all}");
}

/// Item 4: A chart whose workbook is not in the file: its relationship dangles from the start, so it is not Docxtor's. When
/// an edit removed the chart (and its part with it) and undo brought it back, the check after saving called the
/// relationship new, because it compared with the previous save only.
#[test]
fn a_dangling_relationship_the_file_came_with_is_not_new_after_delete_and_undo() {
    let chart = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><c:chart><c:plotArea><c:layout/><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numLit><c:ptCount val="1"/><c:pt idx="0"><c:v>1</c:v></c:pt></c:numLit></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart></c:plotArea></c:chart><c:externalData r:id="rId1"/></c:chartSpace>"#;
    let mut pkg = Package::open(&docx_with_body("<w:p><w:r><w:t>x</w:t></w:r></w:p>")).unwrap();
    pkg.set("word/charts/chart1.xml", chart.as_bytes().to_vec());
    pkg.ensure_override("word/charts/chart1.xml", "application/vnd.openxmlformats-officedocument.drawingml.chart+xml");
    pkg.add_rel("word/charts/chart1.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package", "../embeddings/Missing.xlsx", false);
    let rid = pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart", "charts/chart1.xml", false);
    let drawing = format!(r#"<w:p><w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><wp:extent cx="5486400" cy="3200400"/><wp:docPr id="7" name="Chart 7"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="{rid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#);
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap().replacen("<w:body>", &format!("<w:body>{drawing}"), 1);
    pkg.set("word/document.xml", xml.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    doc.set_invariant_checks(true);
    let first = doc.render_all().order[0];
    let with = doc.save().unwrap();
    assert!(Package::open(&with).unwrap().has("word/charts/chart1.xml"));
    doc.apply(&docxtor_core::Op::DeleteBlocks { ids: vec![docxtor_core::Id(first)] }, false).unwrap();
    let without = doc.save().unwrap();
    assert!(!Package::open(&without).unwrap().has("word/charts/chart1.xml"), "the chart part goes with the chart");
    assert!(doc.undo());
    doc.save().unwrap(); // in a debug build the check panics when it calls the relationship new
    assert_eq!(doc.take_invariant_violations(), vec![]);
}

/// Item 6: a file with no styles part reads as Times New Roman 10 pt (Word's implicit defaults). The first op that
/// needs a style makes a styles part; it used to have no `w:docDefaults`, so the other paragraphs lost their font and
/// size. (The paragraph-mark font in the corpus report was this: `tdf108714.docx` has no styles part.)
#[test]
fn adding_a_style_to_a_file_without_styles_keeps_the_implicit_font() {
    let mut pkg = Package::open(&docx_with_body("<w:p><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r><w:t>two</w:t></w:r></w:p>")).unwrap();
    pkg.remove_rels_to("word/document.xml", "word/styles.xml");
    pkg.remove("word/styles.xml");
    pkg.remove_override("word/styles.xml");
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let ids = doc.render_all().order.clone();
    let font_of = |d: &Document, i: usize| serde_json::to_value(d.render_snapshot()).unwrap()["blocks"][i]["mark"].clone();
    assert_eq!(font_of(&doc, 1)["font"], "Times New Roman");
    doc.apply_json(&format!(r#"[{{"op":"set_style","ids":[{}],"style":"Heading2"}}]"#, ids[0]), false).unwrap();
    assert_eq!(font_of(&doc, 1)["font"], "Times New Roman", "the other paragraph keeps its font");
    assert_eq!(font_of(&doc, 1)["size"], 10.0);
    assert_same_after_reopen(&doc.save().unwrap());
}

/// Item 7: a styles part whose `w:docDefaults` has no size reads as 10 pt (Word's default), the same as a file with no
/// styles at all. The size used to be set only when there were no defaults, so a sheet that gained `w:docDefaults`
/// from an edit, or came back from undo and redo, read differently from the one before (a size went missing).
#[test]
fn a_styles_part_without_a_default_size_reads_as_ten_points_before_and_after_undo_and_redo() {
    let mut pkg = Package::open(&docx_with_body("<w:p><w:r><w:t>one</w:t></w:r></w:p>")).unwrap();
    let styles = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:lang w:val="en-US"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#);
    pkg.set("word/styles.xml", styles.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let size = |d: &Document| serde_json::to_value(d.render_snapshot()).unwrap()["blocks"][0]["mark"]["size"].clone();
    assert_eq!(size(&doc), 10.0);
    let id = doc.render_all().order[0];
    doc.apply_json(&format!(r#"[{{"op":"set_style","ids":[{id}],"style":"Heading1"}}]"#), false).unwrap();
    assert!(doc.undo());
    assert!(doc.redo());
    assert!(size(&doc).is_number(), "{:?}", size(&doc));
    assert_same_after_reopen(&doc.save().unwrap());
}

/// A file with the styles "Placeholder Text" and "toc 1" under other ids (a Spanish or German Word does this).
fn docx_with_localised_styles(body: &str) -> Document {
    let mut pkg = Package::open(&docx_with_body(body)).unwrap();
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/></w:style><w:style w:type="character" w:styleId="Textodelmarcador"><w:name w:val="Placeholder Text"/><w:basedOn w:val="DefaultParagraphFont"/></w:style><w:style w:type="paragraph" w:styleId="Contents1"><w:name w:val="toc 1"/><w:basedOn w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style></w:styles>"#
    );
    pkg.set("word/styles.xml", styles.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    doc.set_invariant_checks(true);
    doc
}

fn body_xml(doc: &mut Document) -> String {
    let pkg = Package::open(&doc.save().unwrap()).unwrap();
    String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap()
}

/// Item 8: features write Word's style ids (`PlaceholderText`, `TOC1`). A file that defines those styles under other
/// ids has them found by name, so nothing is added; the runs and paragraphs used to name the id the file does not
/// have (`style used but not defined`). They now name the file's own ids.
#[test]
fn a_content_control_placeholder_uses_the_id_the_file_gives_the_placeholder_style() {
    let mut doc = docx_with_localised_styles("<w:p><w:r><w:t>some text here</w:t></w:r></w:p>");
    let p = doc.render_all().order[0];
    doc.apply_json(&format!(r#"[{{"op":"control","action":"insert","at":{{"p":{p},"o":4}},"kind":"richText"}}]"#), false).unwrap();
    let xml = body_xml(&mut doc);
    assert!(xml.contains(r#"w:val="Textodelmarcador""#), "{xml}");
    assert!(!xml.contains(r#"w:val="PlaceholderText""#), "{xml}");
    assert_eq!(doc.take_invariant_violations(), vec![]);
}

#[test]
fn a_table_of_contents_uses_the_ids_the_file_gives_its_styles() {
    let mut doc = docx_with_localised_styles(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter</w:t></w:r></w:p>"#);
    doc.apply_json(r#"[{"op":"insert_toc","levels":2}]"#, false).unwrap();
    doc.apply_json(r#"[{"op":"update_toc"}]"#, false).unwrap();
    let xml = body_xml(&mut doc);
    assert!(xml.contains(r#"<w:pStyle w:val="Contents1"/>"#), "{xml}");
    assert!(!xml.contains(r#"w:val="TOC1""#), "{xml}");
    assert_eq!(doc.take_invariant_violations(), vec![]);
}

/// Item 9: Word's `_GoBack` bookmark sits directly under the body as two blocks of its own (start, end). Deleting one
/// of the two used to leave the other behind: a start with no end, or an end with no start. A half that stands alone
/// goes when its partner does. (Not in the original files: they pair the two; Docxtor's op created the lone half.)
#[test]
fn deleting_one_half_of_a_bookmark_that_stands_between_blocks_removes_the_other() {
    for first in [true, false] {
        let body = r#"<w:bookmarkStart w:id="0" w:name="_GoBack"/><w:bookmarkEnd w:id="0"/><w:p><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r><w:t>two</w:t></w:r></w:p>"#;
        let mut doc = Document::open(&docx_with_body(body)).unwrap();
        doc.set_invariant_checks(true);
        let halves: Vec<_> = doc.body.iter().filter(|b| matches!(b, docxtor_core::model::Block::Opaque(o) if o.el.name.starts_with("w:bookmark"))).map(|b| b.id()).collect();
        assert_eq!(halves.len(), 2, "the two halves are blocks of their own");
        let id = if first { halves[0] } else { halves[1] };
        doc.apply(&docxtor_core::Op::DeleteBlocks { ids: vec![docxtor_core::Id(id)] }, false).unwrap();
        let xml = body_xml(&mut doc);
        assert!(!xml.contains("bookmarkStart") && !xml.contains("bookmarkEnd"), "{xml}");
        assert!(xml.contains("one") && xml.contains("two"));
        assert_eq!(doc.take_invariant_violations(), vec![]);
    }
}

/// Item 9, the table case: a bookmark over a table column starts in a cell and ends after the rows, directly in the
/// table. Deleting the column took the start and left the end.
#[test]
fn deleting_the_column_that_holds_the_start_of_a_column_bookmark_removes_its_end() {
    let cell = |inner: &str| format!(r#"<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p>{inner}<w:r><w:t>x</w:t></w:r></w:p></w:tc>"#);
    let body = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr>{}{}</w:tr><w:bookmarkEnd w:id="0"/></w:tbl><w:p/>"#,
        cell(r#"<w:bookmarkStart w:id="0" w:name="_GoBack" w:colFirst="0" w:colLast="0"/>"#),
        cell("")
    );
    let mut doc = Document::open(&docx_with_body(&body)).unwrap();
    doc.set_invariant_checks(true);
    let first = doc.render_all().order[0];
    let first_para = serde_json::to_value(doc.render_snapshot()).unwrap()["blocks"][0]["rows"][0]["cells"][0]["blocks"][0]["id"].as_u64().unwrap_or(first as u64);
    doc.apply_json(&format!(r#"[{{"op":"table_edit","action":"delete_col","at":{first_para}}}]"#), false).unwrap();
    let xml = body_xml(&mut doc);
    assert!(!xml.contains("bookmarkEnd") && !xml.contains("bookmarkStart"), "{xml}");
    assert_eq!(doc.take_invariant_violations(), vec![]);
}

/// Item 9, the rename case: LibreOffice's field marks leave a bookmark start without an end in the file. Renaming
/// that bookmark is not a new problem; the check used to name the bookmark, so a new name looked like a new start.
#[test]
fn renaming_a_bookmark_the_file_left_without_an_end_is_not_a_new_problem() {
    let body = r#"<w:p><w:bookmarkStart w:id="3" w:name="Lonely"/><w:bookmarkStart w:id="4" w:name="Lonely"/><w:bookmarkEnd w:id="4"/><w:r><w:t>text</w:t></w:r></w:p>"#;
    let mut doc = Document::open(&docx_with_body(body)).unwrap();
    doc.set_invariant_checks(true);
    doc.apply_json(r#"[{"op":"rename_bookmark","from":"Lonely","to":"Found"}]"#, false).unwrap();
    assert_eq!(doc.take_invariant_violations(), vec![]);
}

/// Item 10, duplicate ids: a run that carries a tracked formatting change (`w:rPrChange`) is split by an edit, and
/// both halves kept the same change id; the Open XML SDK refuses the part (`fmt-twice.docx`, `tdf124491.docx`).
#[test]
fn splitting_a_run_with_a_formatting_change_gives_the_halves_different_change_ids() {
    let body = r#"<w:p><w:r><w:rPr><w:color w:val="FF0000"/><w:rPrChange w:id="1" w:author="a" w:date="2019-08-29T15:35:00Z"><w:rPr/></w:rPrChange></w:rPr><w:t>Lorem ipsum dolor</w:t></w:r></w:p>"#;
    let mut doc = Document::open(&docx_with_body(body)).unwrap();
    let p = doc.render_all().order[0];
    doc.apply_json(&format!(r#"[{{"op":"format_text","from":{{"p":{p},"o":6}},"to":{{"p":{p},"o":11}},"format":{{"bold":true}}}}]"#), false).unwrap();
    let xml = body_xml(&mut doc);
    let ids: Vec<&str> = xml.split("<w:rPrChange w:id=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    assert!(ids.len() >= 3, "the run was split into three: {xml}");
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "{ids:?}");
}

/// The fresh ids don't take one an insertion or deletion already has (they share the numbers).
#[test]
fn a_fresh_change_id_does_not_take_an_insertions_id() {
    let body = r#"<w:p><w:r><w:rPr><w:rPrChange w:id="1" w:author="a" w:date="2019-08-29T15:35:00Z"><w:rPr/></w:rPrChange></w:rPr><w:t>Lorem ipsum dolor</w:t></w:r><w:ins w:id="2" w:author="a" w:date="2019-08-29T15:35:00Z"><w:r><w:t> added</w:t></w:r></w:ins></w:p>"#;
    let mut doc = Document::open(&docx_with_body(body)).unwrap();
    let p = doc.render_all().order[0];
    doc.apply_json(&format!(r#"[{{"op":"format_text","from":{{"p":{p},"o":6}},"to":{{"p":{p},"o":11}},"format":{{"bold":true}}}}]"#), false).unwrap();
    let xml = body_xml(&mut doc);
    let ids: Vec<&str> = xml.split(" w:id=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "every revision id is its own: {ids:?}");
}

/// Item 10, `moveFrom` in run properties: the paragraph mark's properties may say the mark was moved
/// (`w:moveFrom`, as `w:ins` and `w:del` say it was inserted or deleted). Text typed into the empty paragraph took
/// the mark's properties, including that mark, and a run's properties may not hold it (`tdf104797.docx`,
/// `tdf150166.docx`).
#[test]
fn text_typed_into_a_paragraph_whose_mark_was_moved_gets_no_move_mark_in_its_run() {
    let body = r#"<w:p><w:pPr><w:rPr><w:b/><w:moveFrom w:id="0" w:author="a" w:date="2020-11-06T14:07:00Z"/></w:rPr></w:pPr></w:p>"#;
    let mut doc = Document::open(&docx_with_body(body)).unwrap();
    let p = doc.render_all().order[0];
    doc.apply_json(&format!(r#"[{{"op":"insert_text","at":{{"p":{p},"o":0}},"text":"typed"}}]"#), false).unwrap();
    let xml = body_xml(&mut doc);
    let run = xml.split("<w:r>").nth(1).expect("a run");
    assert!(!run.split("</w:r>").next().unwrap().contains("moveFrom"), "{xml}");
    assert!(xml.contains("<w:moveFrom"), "the mark keeps its own move mark: {xml}");
    assert!(run.contains("<w:b/>"), "the run keeps the mark's formatting: {xml}");
}

/// Found by a run of every real file (LibreOffice's tdf158349_SDTstreamStateStack.docx,
/// tdf153891.docx and fdo81946.docx). A save always ends a table cell with a paragraph; a cell the file ends with
/// something else got one in the saved file and not in the text whenever the table was written again (an edit, or an
/// undo, anywhere), so the reopened file had one more paragraph in that cell. A content control that ends with a
/// paragraph counts as one (Word writes a control around a cell's text so); a cell that ends with a nested table gets
/// its empty paragraph as the file opens, so the text and every save agree.
#[test]
fn cells_end_in_a_paragraph_the_same_way_in_the_text_and_in_every_save() {
    let cell = |inner: &str| format!(r#"<w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>{inner}</w:tc>"#);
    let nested = format!(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr>{}</w:tr></w:tbl>"#, cell("<w:p><w:r><w:t>inner</w:t></w:r></w:p>"));
    let boxed = r#"<w:sdt><w:sdtPr><w:id w:val="70001"/><w:group/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>In a box</w:t></w:r></w:p></w:sdtContent></w:sdt>"#;
    let body = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr>{}{}</w:tr></w:tbl><w:p><w:r><w:t>After</w:t></w:r></w:p>"#,
        cell(boxed),
        cell(&nested)
    );
    let bytes = docx_with_body(&body);
    // Opened and saved with no edit: the file stays as it was.
    let mut doc = Document::open(&bytes).unwrap();
    let untouched = doc.save().unwrap();
    assert_eq!(Package::open(&untouched).unwrap().get("word/document.xml"), Package::open(&bytes).unwrap().get("word/document.xml"));
    // Written again: the box's cell gets no paragraph after the box, the nested table's cell gets one; reopened alike.
    let again = assert_same_after_reopen(&bytes);
    let mut again = again;
    let xml = body_xml(&mut again);
    assert!(!xml.contains("</w:sdt><w:p/></w:tc>"), "no empty paragraph after a box that ends with one: {xml}");
    // An edit and its undo write the table again (as the corpus run found).
    let mut doc = Document::open(&bytes).unwrap();
    let last = doc.body.last().map(|b| b.id()).unwrap();
    doc.apply_json(&serde_json::json!({"op":"insert_paragraph","before":last,"text":"x"}).to_string(), false).unwrap();
    assert!(doc.undo());
    let saved = doc.save().unwrap();
    let reopened = Document::open(&saved).unwrap();
    if let Some(d) = first_diff(&normalise(&render_model(&doc)), &normalise(&render_model(&reopened))) {
        panic!("an edit and its undo, saved and reopened, renders differently: {d}");
    }
}

/// Found by a run of every real file (LibreOffice's tdf138093B.docx, content-control-date-data-binding.docx and
/// test_sdt_datepicker.docx): a date box bound to the cover page's publish date was shown again as the file opened
/// because the date Word stores (`2017-01-06T00:00:00`) is spelt otherwise than the box's own (`…Z`), and showing it
/// rebuilt the box as one paragraph. The same date is not a change: the box keeps what it holds, two paragraphs here.
#[test]
fn a_bound_date_box_showing_the_stored_date_is_kept_as_it_is_when_the_file_opens() {
    let store = "{55AF091B-3C7A-41E3-B477-F2FDAA23CFDA}";
    let body = format!(
        r#"<w:sdt><w:sdtPr><w:alias w:val="Year"/><w:id w:val="70002"/><w:dataBinding w:prefixMappings="xmlns:ns0='http://schemas.microsoft.com/office/2006/coverPageProps'" w:xpath="/ns0:CoverPageProperties[1]/ns0:PublishDate[1]" w:storeItemID="{store}"/><w:date w:fullDate="2017-01-06T00:00:00Z"><w:dateFormat w:val="yyyy"/><w:lid w:val="en-US"/><w:storeMappedDataAs w:val="dateTime"/><w:calendar w:val="gregorian"/></w:date></w:sdtPr><w:sdtContent><w:p><w:r><w:t>2017</w:t></w:r></w:p><w:p/></w:sdtContent></w:sdt><w:p><w:r><w:t>After</w:t></w:r></w:p>"#
    );
    let mut pkg = Package::open(&docx_with_body(&body)).unwrap();
    pkg.set("customXml/item1.xml", br#"<CoverPageProperties xmlns="http://schemas.microsoft.com/office/2006/coverPageProps"><PublishDate>2017-01-06T00:00:00</PublishDate></CoverPageProperties>"#.to_vec());
    pkg.set("customXml/itemProps1.xml", format!(r#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{store}"/>"#).into_bytes());
    pkg.add_rel("customXml/item1.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps", "itemProps1.xml", false);
    pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml", "../customXml/item1.xml", false);
    let bytes = pkg.to_bytes().unwrap();
    let doc = Document::open(&bytes).unwrap();
    let paras = match &doc.body[0] {
        docxtor_core::model::Block::Container(c) => c.blocks.len(),
        other => panic!("the box is the first block: {other:?}"),
    };
    assert_eq!(paras, 2, "the box keeps both its paragraphs");
    assert_same_after_reopen(&bytes);
}
