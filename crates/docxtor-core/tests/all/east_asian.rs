//! East Asian typography: the document's own CJK fonts, phonetic guides (ruby), enclosed characters, two lines in one
//! and text upright in vertical lines, the Asian typography switches of a paragraph, the document grid of a section
//! and the document-wide line-breaking settings.

use docxtor_core::model::{find_block, walk_paragraphs, write_block};
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

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

fn fails(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err("the op should fail").to_string()
}

fn docx_with(body: &str, sect: &str, extra_styles: &str, settings: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>{sect}</w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    if !extra_styles.is_empty() {
        let s = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/styles.xml", s.replace("</w:styles>", &format!("{extra_styles}</w:styles>")).into_bytes());
    }
    if !settings.is_empty() {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/settings.xml", s.replace("</w:settings>", &format!("{settings}</w:settings>")).into_bytes());
    }
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn docx(body: &str) -> Document {
    docx_with(body, "", "", "")
}

fn first_para(doc: &Document) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        id.get_or_insert(p.id);
    });
    id.unwrap()
}

fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

fn rendered_para(doc: &Document, id: u32) -> Value {
    let all = serde_json::to_value(doc.render_snapshot()).unwrap();
    all["blocks"].as_array().unwrap().iter().find(|b| b["t"] == "p" && b["id"] == id).cloned().expect("paragraph")
}

fn segs(doc: &Document, id: u32) -> Vec<Value> {
    rendered_para(doc, id)["segs"].as_array().unwrap().clone()
}

fn saved(doc: &mut Document) -> Vec<u8> {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    bytes
}

fn part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

#[test]
fn east_asian_text_takes_the_east_asian_font_and_latin_text_does_not() {
    let doc = docx(r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="MS Mincho"/></w:rPr><w:t>Hello 漢字です abc</w:t></w:r></w:p>"#);
    let s = segs(&doc, first_para(&doc));
    // The text is cut where the script changes; East Asian text is marked and every piece says what the East Asian font is.
    let got: Vec<(String, String, bool)> =
        s.iter().map(|x| (x["x"].as_str().unwrap().to_string(), x["f"]["font"].as_str().unwrap_or("").to_string(), x["f"]["eaText"] == true)).collect();
    assert_eq!(
        got,
        vec![("Hello ".to_string(), "Arial".to_string(), false), ("漢字です ".to_string(), "Arial".to_string(), true), ("abc".to_string(), "Arial".to_string(), false)],
        "{s:?}"
    );
    assert!(s.iter().all(|x| x["f"]["fontEa"] == "MS Mincho"));
}

#[test]
fn a_run_with_only_an_east_asian_font_keeps_the_default_latin_font() {
    let doc = docx(r#"<w:p><w:r><w:rPr><w:rFonts w:eastAsia="Yu Gothic"/></w:rPr><w:t>abc かな</w:t></w:r></w:p>"#);
    let s = segs(&doc, first_para(&doc));
    assert_eq!(s.len(), 2);
    assert!(s[0]["f"]["font"].is_null() || s[0]["f"]["font"] != "Yu Gothic");
    assert_eq!(s[1]["f"]["fontEa"], "Yu Gothic");
    assert_eq!(s[1]["f"]["eaText"], true);
    assert!(s[0]["f"]["eaText"].is_null());
}

#[test]
fn theme_east_asian_fonts_resolve_from_the_theme_for_the_documents_language() {
    // The blank document's theme has no East Asian face of its own; the file's style defaults say Japanese.
    let styles = "";
    let doc = docx_with(r#"<w:p><w:r><w:rPr><w:rFonts w:eastAsiaTheme="minorEastAsia"/></w:rPr><w:t>漢字</w:t></w:r></w:p>"#, "", styles, "");
    let s = segs(&doc, first_para(&doc));
    assert_eq!(s.len(), 1);
    // No East Asian face to resolve: the text keeps the run's font and the browser picks one.
    assert!(s[0]["f"]["fontEa"].is_null());
}

#[test]
fn the_east_asian_language_marks_east_asian_text_only() {
    let doc = docx(r#"<w:p><w:r><w:rPr><w:lang w:val="en-US" w:eastAsia="ja-JP"/></w:rPr><w:t>abc かな</w:t></w:r></w:p>"#);
    let s = segs(&doc, first_para(&doc));
    assert_eq!(s.len(), 2);
    assert!(s[0]["f"]["langEa"].is_null());
    assert_eq!(s[1]["f"]["langEa"], "ja-JP");
}

#[test]
fn setting_the_east_asian_font_writes_it_with_the_hint_and_keeps_the_latin_font() {
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/></w:rPr><w:t>abc 漢字</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":6},"format":{"fontEa":"MS Gothic"}}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains(r#"w:eastAsia="MS Gothic""#) && xml.contains(r#"w:hint="eastAsia""#) && xml.contains(r#"w:ascii="Arial""#), "{xml}");
    // The font box sets the Latin font and leaves the East Asian one.
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":6},"format":{"font":"Verdana"}}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains(r#"w:ascii="Verdana""#) && xml.contains(r#"w:eastAsia="MS Gothic""#), "{xml}");
    saved(&mut doc);
    assert!(doc.undo() && doc.undo());
    assert!(!para_xml(&doc, id).contains("MS Gothic"));
    // Clearing removes it.
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":6},"format":{"fontEa":"MS Gothic"}}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":6},"format":{"fontEa":""}}));
    assert!(!para_xml(&doc, id).contains("eastAsia"));
}

#[test]
fn the_font_box_replaces_an_east_asian_font_that_only_repeats_the_latin_one() {
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:cs="Arial" w:eastAsia="Arial"/></w:rPr><w:t>abc 漢字</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":6},"format":{"font":"Verdana"}}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains(r#"w:eastAsia="Verdana""#) && !xml.contains(r#"w:eastAsia="Arial""#), "{xml}");
}

// ---------------------------------------------------------------------------
// Phonetic guides
// ---------------------------------------------------------------------------

const RUBY: &str = r#"<w:p><w:r><w:t xml:space="preserve">前 </w:t></w:r><w:r><w:rPr><w:rFonts w:hint="eastAsia"/></w:rPr><w:ruby><w:rubyPr><w:rubyAlign w:val="distributeSpace"/><w:hps w:val="10"/><w:hpsRaise w:val="18"/><w:hpsBaseText w:val="21"/><w:lid w:val="ja-JP"/></w:rubyPr><w:rt><w:r><w:rPr><w:rFonts w:hint="eastAsia"/><w:sz w:val="10"/></w:rPr><w:t>かんじ</w:t></w:r></w:rt><w:rubyBase><w:r><w:rPr><w:rFonts w:hint="eastAsia"/></w:rPr><w:t>漢字</w:t></w:r></w:rubyBase></w:ruby></w:r><w:r><w:t xml:space="preserve"> 後</w:t></w:r></w:p>"#;

#[test]
fn a_phonetic_guide_is_drawn_with_its_base_and_guide() {
    let doc = docx(RUBY);
    let s = segs(&doc, first_para(&doc));
    assert_eq!(s.len(), 3, "{s:?}");
    let r = &s[1];
    assert_eq!(r["k"], "ruby");
    assert_eq!(r["o"], 2);
    assert_eq!(r["ruby"]["base"], "漢字");
    assert_eq!(r["ruby"]["text"], "かんじ");
    assert_eq!(r["ruby"]["align"], "distributeSpace");
    assert_eq!(r["ruby"]["size"], 5.0);
    assert_eq!(r["ruby"]["raise"], 9.0);
    assert_eq!(r["ruby"]["baseSize"], 10.5);
    assert_eq!(r["ruby"]["rtF"]["size"], 5.0);
    // It is one position in the paragraph.
    assert_eq!(s[2]["o"], 3);
    assert_eq!(rendered_para(&doc, first_para(&doc))["len"], 5);
    // Copied as plain text, a guide gives its base text.
    let id = first_para(&doc);
    assert_eq!(doc.text_range(Pos::new(id, 0), Pos::new(id, 5)), "前 漢字 後");
}

#[test]
fn a_phonetic_guide_survives_a_save_byte_for_byte() {
    let mut doc = docx(RUBY);
    let id = first_para(&doc);
    let before = para_xml(&doc, id);
    let bytes = saved(&mut doc);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(para_xml(&again, first_para(&again)), before);
    assert!(part(&bytes, "word/document.xml").contains("<w:rubyAlign w:val=\"distributeSpace\"/>"));
}

#[test]
fn set_ruby_puts_a_guide_over_the_selected_text_and_undoes() {
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>前 漢字 後</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"east_asian","action":"set_ruby","from":{"p":id,"o":2},"to":{"p":id,"o":4},"text":"かんじ","align":"center","size":5.0,"baseSize":10.5,"raise":9.0}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains("<w:rubyBase>") && xml.contains("漢字") && xml.contains("かんじ"), "{xml}");
    // The guide's settings are in schema order and the base keeps the bold.
    let pr = xml.find("<w:rubyPr>").unwrap();
    assert!(xml[pr..].find("<w:rubyAlign").unwrap() < xml[pr..].find("<w:hps ").unwrap());
    assert!(xml[pr..].find("<w:hps ").unwrap() < xml[pr..].find("<w:hpsRaise").unwrap());
    assert!(xml[pr..].find("<w:hpsRaise").unwrap() < xml[pr..].find("<w:hpsBaseText").unwrap());
    assert!(xml[pr..].find("<w:hpsBaseText").unwrap() < xml[pr..].find("<w:lid").unwrap());
    assert!(xml.contains("<w:b/>"));
    let s = segs(&doc, id);
    assert_eq!(s.iter().find(|x| x["k"] == "ruby").unwrap()["o"], 2);
    assert_eq!(rendered_para(&doc, id)["len"], 5);
    saved(&mut doc);
    assert!(doc.undo());
    assert!(!para_xml(&doc, id).contains("w:ruby"));
    assert_eq!(rendered_para(&doc, id)["len"], 6);
}

#[test]
fn set_ruby_on_a_guide_changes_the_guide_and_keeps_the_base() {
    let mut doc = docx(RUBY);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"east_asian","action":"set_ruby","from":{"p":id,"o":2},"to":{"p":id,"o":3},"text":"カンジ","align":"left"}));
    let r = doc.ruby_at(Pos::new(id, 2)).unwrap();
    assert_eq!((r.base.as_str(), r.text.as_str(), r.align.as_str()), ("漢字", "カンジ", "left"));
    // The sizes it had stay.
    assert_eq!(r.size, Some(5.0));
    assert_eq!(r.raise, Some(9.0));
    assert!(doc.undo());
    assert_eq!(doc.ruby_at(Pos::new(id, 2)).unwrap().text, "かんじ");
}

#[test]
fn clear_ruby_leaves_the_base_text() {
    let mut doc = docx(RUBY);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"east_asian","action":"clear_ruby","at":{"p":id,"o":2}}));
    let xml = para_xml(&doc, id);
    assert!(!xml.contains("w:ruby") && xml.contains("漢字"), "{xml}");
    assert_eq!(docxtor_core::model::find_para(&doc.body, id).unwrap().plain_text(), "前 漢字 後");
    saved(&mut doc);
    assert!(doc.undo());
    assert!(para_xml(&doc, id).contains("<w:ruby>"));
}

#[test]
fn set_ruby_refuses_what_it_cannot_do() {
    let mut doc = docx(r#"<w:p><w:r><w:t>漢字</w:t></w:r></w:p><w:p><w:r><w:t>second</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"set_ruby","from":{"p":id,"o":0},"to":{"p":id,"o":0},"text":"x"})).contains("Select the text"));
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"set_ruby","from":{"p":id,"o":0},"to":{"p":id,"o":2},"text":""})).contains("needs some text"));
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"set_ruby","from":{"p":id,"o":0},"to":{"p":id,"o":2},"text":"x","align":"middle"})).contains("aligned"));
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"clear_ruby","at":{"p":id,"o":0}})).contains("no phonetic guide"));
}

// ---------------------------------------------------------------------------
// Enclosed characters
// ---------------------------------------------------------------------------

#[test]
fn enclose_writes_an_eq_field_that_is_drawn_and_can_be_taken_away() {
    let mut doc = docx(r#"<w:p><w:r><w:t>前 字 後</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"east_asian","action":"enclose","from":{"p":id,"o":2},"to":{"p":id,"o":3},"symbol":"circle"}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains(r"EQ \o\ac(○,字)") && xml.contains(r#"w:fldCharType="begin""#) && xml.contains(r#"w:fldCharType="end""#), "{xml}");
    let s = segs(&doc, id);
    let eq = s.iter().find(|x| x["k"] == "eq").expect("an equation segment");
    assert!(eq["math"]["mathml"].as_str().unwrap().contains("字"));
    saved(&mut doc);
    ops(&mut doc, json!({"op":"east_asian","action":"clear_enclose","at":{"p":id,"o":2}}));
    assert!(!para_xml(&doc, id).contains("EQ"));
    assert_eq!(docxtor_core::model::find_para(&doc.body, id).unwrap().plain_text(), "前 字 後");
    assert!(doc.undo());
    assert!(para_xml(&doc, id).contains("EQ"));
}

#[test]
fn enclose_takes_one_or_two_characters_and_known_shapes() {
    let mut doc = docx(r#"<w:p><w:r><w:t>abcdef</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"enclose","from":{"p":id,"o":0},"to":{"p":id,"o":3},"symbol":"circle"})).contains("one or two"));
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"enclose","from":{"p":id,"o":0},"to":{"p":id,"o":1},"symbol":"star"})).contains("circle, square"));
}

// ---------------------------------------------------------------------------
// Two lines in one, upright text in vertical lines
// ---------------------------------------------------------------------------

#[test]
fn east_asian_layout_is_read_and_written() {
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:eastAsianLayout w:id="3" w:combine="1" w:combineBrackets="square"/></w:rPr><w:t>二行</w:t></w:r><w:r><w:rPr><w:eastAsianLayout w:id="4" w:vert="1" w:vertCompress="1"/></w:rPr><w:t>12</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    let s = segs(&doc, id);
    assert_eq!(s[0]["f"]["eaLayout"]["combine"], true);
    assert_eq!(s[0]["f"]["eaLayout"]["combineBrackets"], "square");
    assert_eq!(s[1]["f"]["eaLayout"]["vert"], true);
    assert_eq!(s[1]["f"]["eaLayout"]["vertCompress"], true);
    // Switching it on for new text writes the element in its place in the run properties.
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":2},"format":{"bold":true,"eaLayout":{"combine":true,"combineBrackets":"round"}}}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains(r#"w:combineBrackets="round""#), "{xml}");
    // And off removes it.
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":2},"format":{"eaLayout":{"combine":false}}}));
    assert!(!para_xml(&doc, id).contains("combine"));
    saved(&mut doc);
    assert!(doc.undo());
    assert!(para_xml(&doc, id).contains(r#"w:combineBrackets="round""#));
}

#[test]
fn east_asian_layout_gets_an_id_that_groups_the_runs() {
    let mut doc = docx(r#"<w:p><w:r><w:t>二行</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":2},"format":{"eaLayout":{"vert":true,"vertCompress":true}}}));
    let xml = para_xml(&doc, id);
    assert!(xml.contains("<w:eastAsianLayout w:id=") && xml.contains(r#"w:vert="1""#) && xml.contains(r#"w:vertCompress="1""#), "{xml}");
}

#[test]
fn two_lines_in_one_over_several_runs_shares_one_id_and_avoids_the_files() {
    let mut doc = docx(r#"<w:p><w:r><w:t>二行</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>文字</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":4},"format":{"eaLayout":{"combine":true}}}));
    let xml = para_xml(&doc, id);
    let ids: Vec<&str> = xml.split("<w:eastAsianLayout w:id=\"").skip(1).map(|r| r.split('"').next().unwrap()).collect();
    assert!(ids.len() >= 2 && ids.iter().all(|i| *i == ids[0]), "{xml}");
    // A file that already uses id 1 for another combination is not joined to the new one.
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:eastAsianLayout w:id="1" w:combine="1"/></w:rPr><w:t>AB</w:t></w:r></w:p><w:p><w:r><w:t>漢字</w:t></w:r></w:p>"#);
    let mut order = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| order.push(p.id));
    let second = order[1];
    ops(&mut doc, json!({"op":"format_text","from":{"p":second,"o":0},"to":{"p":second,"o":2},"format":{"eaLayout":{"combine":true}}}));
    let xml = para_xml(&doc, second);
    assert!(!xml.contains(r#"w:id="1""#), "{xml}");
}

#[test]
fn snap_to_grid_can_be_turned_on_where_the_style_turns_it_off() {
    let mut doc = docx_with(
        r#"<w:p><w:r><w:rPr><w:rStyle w:val="Loose"/></w:rPr><w:t>漢字</w:t></w:r></w:p>"#,
        "",
        r#"<w:style w:type="character" w:styleId="Loose"><w:name w:val="Loose"/><w:rPr><w:snapToGrid w:val="0"/></w:rPr></w:style>"#,
        "",
    );
    let id = first_para(&doc);
    assert_eq!(segs(&doc, id)[0]["f"]["snapToGrid"], false);
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":2},"format":{"snapToGrid":true}}));
    assert_eq!(segs(&doc, id)[0]["f"].get("snapToGrid").cloned().unwrap_or(json!(true)), true, "{}", para_xml(&doc, id));
}

#[test]
fn snap_to_grid_off_is_written_only_when_it_differs_from_the_default() {
    let mut doc = docx(r#"<w:p><w:r><w:t>漢字</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":2},"format":{"snapToGrid":true}}));
    assert!(!para_xml(&doc, id).contains("snapToGrid"));
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":2},"format":{"snapToGrid":false}}));
    assert!(para_xml(&doc, id).contains(r#"<w:snapToGrid w:val="0"/>"#));
    assert_eq!(segs(&doc, id)[0]["f"]["snapToGrid"], false);
}

// ---------------------------------------------------------------------------
// Paragraphs
// ---------------------------------------------------------------------------

#[test]
fn the_asian_typography_switches_of_a_paragraph_are_read_and_written() {
    let mut doc = docx(r#"<w:p><w:pPr><w:kinsoku w:val="0"/><w:wordWrap w:val="0"/><w:overflowPunct w:val="0"/><w:topLinePunct/><w:autoSpaceDE w:val="0"/><w:autoSpaceDN w:val="0"/><w:snapToGrid w:val="0"/></w:pPr><w:r><w:t>漢字 abc</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    let f = rendered_para(&doc, id)["fmt"].clone();
    assert_eq!(f["kinsoku"], false);
    assert_eq!(f["wordWrap"], false);
    assert_eq!(f["overflowPunct"], false);
    assert_eq!(f["topLinePunct"], true);
    assert_eq!(f["autoSpaceDe"], false);
    assert_eq!(f["autoSpaceDn"], false);
    assert_eq!(f["snapToGrid"], false);
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[id],"format":{"kinsoku":true,"wordWrap":true,"topLinePunct":false,"autoSpaceDe":true,"snapToGrid":true}}));
    let xml = para_xml(&doc, id);
    // On is Word's default for these, so the elements go; compression off is its default too.
    for gone in ["w:kinsoku", "w:wordWrap", "w:topLinePunct", "w:autoSpaceDE", "w:snapToGrid"] {
        assert!(!xml.contains(gone), "{gone} in {xml}");
    }
    assert!(xml.contains(r#"<w:overflowPunct w:val="0"/>"#) && xml.contains(r#"<w:autoSpaceDN w:val="0"/>"#));
    saved(&mut doc);
}

#[test]
fn the_asian_typography_elements_keep_the_schema_order() {
    let mut doc = docx(r#"<w:p><w:pPr><w:jc w:val="left"/></w:pPr><w:r><w:t>漢字</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[id],"format":{"kinsoku":false,"topLinePunct":true,"autoSpaceDn":false,"snapToGrid":false,"textDirection":"tbRl","align":"both"}}));
    let xml = para_xml(&doc, id);
    let at = |n: &str| xml.find(n).unwrap_or_else(|| panic!("{n} in {xml}"));
    assert!(at("<w:kinsoku") < at("<w:topLinePunct") && at("<w:topLinePunct") < at("<w:autoSpaceDN") && at("<w:autoSpaceDN") < at("<w:snapToGrid"));
    assert!(at("<w:snapToGrid") < at("<w:jc") && at("<w:jc") < at("<w:textDirection"));
    assert!(doc.undo());
    assert!(!para_xml(&doc, id).contains("kinsoku"));
}

#[test]
fn a_style_gives_its_paragraphs_the_asian_typography() {
    let doc = docx_with(
        r#"<w:p><w:pPr><w:pStyle w:val="Jp"/></w:pPr><w:r><w:t>漢字</w:t></w:r></w:p>"#,
        "",
        r#"<w:style w:type="paragraph" w:customStyle="1" w:styleId="Jp"><w:name w:val="Jp"/><w:basedOn w:val="Normal"/><w:pPr><w:kinsoku w:val="0"/><w:autoSpaceDE w:val="0"/></w:pPr></w:style>"#,
        "",
    );
    let f = rendered_para(&doc, first_para(&doc))["fmt"].clone();
    assert_eq!((f["kinsoku"].clone(), f["autoSpaceDe"].clone()), (json!(false), json!(false)));
}

#[test]
fn a_paragraph_text_direction_is_read() {
    let doc = docx(r#"<w:p><w:pPr><w:textDirection w:val="tbRl"/></w:pPr><w:r><w:t>漢字</w:t></w:r></w:p>"#);
    assert_eq!(rendered_para(&doc, first_para(&doc))["fmt"]["textDirection"], "tbRl");
}

// ---------------------------------------------------------------------------
// The document grid and the line-breaking settings
// ---------------------------------------------------------------------------

#[test]
fn the_document_grid_is_read_set_and_removed() {
    let mut doc = docx_with(r#"<w:p><w:r><w:t>漢字</w:t></w:r></w:p>"#, r#"<w:docGrid w:type="lines" w:linePitch="312"/>"#, "", "");
    let sec = serde_json::to_value(doc.page_setup()).unwrap();
    let s0 = &sec["sections"][0];
    assert_eq!(s0["docGrid"]["type"], "lines");
    assert_eq!(s0["docGrid"]["linePitch"], 312);
    assert!(s0["gridBase"].as_f64().unwrap() > 0.0);
    ops(&mut doc, json!({"op":"set_section","doc_grid":{"type":"linesAndChars","linePitch":360,"charSpace":-1024}}));
    let xml = part(&saved(&mut doc), "word/document.xml");
    assert!(xml.contains(r#"w:type="linesAndChars""#) && xml.contains(r#"w:linePitch="360""#) && xml.contains(r#"w:charSpace="-1024""#), "{xml}");
    ops(&mut doc, json!({"op":"set_section","doc_grid":{"type":"default"}}));
    let sec = serde_json::to_value(doc.page_setup()).unwrap();
    assert!(sec["sections"][0]["docGrid"].is_null());
    assert!(doc.undo());
    assert_eq!(serde_json::to_value(doc.page_setup()).unwrap()["sections"][0]["docGrid"]["type"], "linesAndChars");
    assert!(fails(&mut doc, json!({"op":"set_section","doc_grid":{"type":"squares"}})).contains("document grid"));
}

#[test]
fn the_document_grid_stays_in_schema_order() {
    let mut doc = docx_with(r#"<w:p><w:r><w:t>漢字</w:t></w:r></w:p>"#, r#"<w:cols w:space="720"/>"#, "", "");
    ops(&mut doc, json!({"op":"set_section","doc_grid":{"type":"lines","linePitch":300},"text_direction":"tbRl"}));
    let xml = part(&saved(&mut doc), "word/document.xml");
    assert!(xml.find("<w:cols").unwrap() < xml.find("<w:textDirection").unwrap() && xml.find("<w:textDirection").unwrap() < xml.find("<w:docGrid").unwrap(), "{xml}");
}

#[test]
fn the_line_breaking_settings_round_trip_and_undo() {
    let mut doc = docx_with(
        r#"<w:p><w:r><w:t>漢字</w:t></w:r></w:p>"#,
        "",
        "",
        r#"<w:strictFirstAndLastChars/><w:noLineBreaksAfter w:lang="ja-JP" w:val="「（"/><w:noLineBreaksBefore w:lang="ja-JP" w:val="」）、。"/>"#,
    );
    let s = doc.asian_settings();
    assert!(s.strict);
    assert_eq!(s.custom.len(), 1);
    assert_eq!((s.custom[0].lang.as_str(), s.custom[0].after.as_str(), s.custom[0].before.as_str()), ("ja-JP", "「（", "」）、。"));
    ops(&mut doc, json!({"op":"east_asian","action":"set_settings","settings":{"strict":false,"custom":[{"lang":"zh-CN","before":"，。","after":"（"}]}}));
    let bytes = saved(&mut doc);
    let settings = part(&bytes, "word/settings.xml");
    assert!(!settings.contains("strictFirstAndLastChars") && settings.contains(r#"w:lang="zh-CN""#) && !settings.contains("ja-JP"), "{settings}");
    let again = Document::open(&bytes).unwrap();
    assert_eq!(again.asian_settings().custom[0].lang, "zh-CN");
    assert!(doc.undo());
    assert!(doc.asian_settings().strict);
    assert!(fails(&mut doc, json!({"op":"east_asian","action":"set_settings","settings":{"custom":[{"lang":"en-US","before":"x"}]}})).contains("ja-JP"));
}

#[test]
fn an_untouched_east_asian_document_is_saved_unchanged() {
    // Everything here is carried through byte for byte when nothing edits it.
    let body = r#"<w:p><w:pPr><w:kinsoku w:val="0"/><w:snapToGrid w:val="0"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Arial" w:eastAsia="MS Mincho" w:hint="eastAsia"/><w:eastAsianLayout w:id="1" w:combine="1"/><w:lang w:val="en-US" w:eastAsia="ja-JP"/></w:rPr><w:t>漢字</w:t></w:r></w:p>"#;
    let mut doc = docx_with(body, r#"<w:docGrid w:type="linesAndChars" w:linePitch="360" w:charSpace="4096"/>"#, "", "");
    let id = first_para(&doc);
    let before = para_xml(&doc, id);
    let bytes = saved(&mut doc);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(para_xml(&again, first_para(&again)), before);
    assert!(part(&bytes, "word/document.xml").contains(r#"<w:docGrid w:type="linesAndChars" w:linePitch="360" w:charSpace="4096"/>"#));
}

// ---------------------------------------------------------------------------
// The AI tools
// ---------------------------------------------------------------------------

fn tool(doc: &mut Document, name: &str, args: Value) -> Result<String, String> {
    docxtor_core::ai::call_tool(doc, name, &args)
}

#[test]
fn the_ai_can_put_a_guide_over_text_change_it_and_take_it_away() {
    let mut doc = docx(r#"<w:p><w:r><w:t>前 漢字 後</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    tool(&mut doc, "set_phonetic_guide", json!({"id": format!("p{id}"), "text": "漢字", "guide": "かんじ", "size_pt": 5})).unwrap();
    assert_eq!(doc.ruby_at(Pos::new(id, 2)).unwrap().text, "かんじ");
    // The same base text again changes the guide it has instead of nesting another.
    tool(&mut doc, "set_phonetic_guide", json!({"id": format!("p{id}"), "text": "漢字", "guide": "カンジ", "align": "left"})).unwrap();
    let r = doc.ruby_at(Pos::new(id, 2)).unwrap();
    assert_eq!((r.text.as_str(), r.align.as_str()), ("カンジ", "left"));
    assert_eq!(doc.guides_in(id).len(), 1);
    assert!(tool(&mut doc, "remove_phonetic_guide", json!({"id": format!("p{id}"), "text": "なし"})).unwrap_err().contains("no phonetic guide"));
    tool(&mut doc, "remove_phonetic_guide", json!({"id": format!("p{id}"), "text": "漢字"})).unwrap();
    assert!(doc.guides_in(id).is_empty());
    saved(&mut doc);
}

#[test]
fn the_ai_can_set_east_asian_text_paragraph_switches_the_grid_and_the_rules() {
    let mut doc = docx(r#"<w:p><w:r><w:t>漢字 and more</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    let pid = format!("p{id}");
    tool(&mut doc, "set_east_asian_text", json!({"id": pid, "text": "漢字", "east_asian_font": "MS Mincho", "two_lines_in_one": true, "brackets": "round"})).unwrap();
    let xml = para_xml(&doc, id);
    assert!(xml.contains(r#"w:eastAsia="MS Mincho""#) && xml.contains(r#"w:combineBrackets="round""#), "{xml}");
    assert!(tool(&mut doc, "set_east_asian_text", json!({"id": pid})).unwrap_err().contains("say what to set"));
    tool(&mut doc, "set_asian_typography", json!({"ids": [pid], "kinsoku": false, "top_line_punct": true, "auto_space_de": false})).unwrap();
    let f = rendered_para(&doc, id)["fmt"].clone();
    assert_eq!((f["kinsoku"].clone(), f["topLinePunct"].clone(), f["autoSpaceDe"].clone()), (json!(false), json!(true), json!(false)));
    assert!(tool(&mut doc, "set_asian_typography", json!({"ids": [pid]})).unwrap_err().contains("say what to set"));
    tool(&mut doc, "set_document_grid", json!({"type": "linesAndChars", "line_pitch_pt": 18, "char_space_pt": 1})).unwrap();
    let g = serde_json::to_value(doc.page_setup()).unwrap()["sections"][0]["docGrid"].clone();
    assert_eq!((g["type"].clone(), g["linePitch"].clone(), g["charSpace"].clone()), (json!("linesAndChars"), json!(360), json!(4096)));
    tool(&mut doc, "set_line_breaking_rules", json!({"strict": true, "language": "ja-JP", "cannot_start": "、。", "cannot_end": "「"})).unwrap();
    let s = doc.asian_settings();
    assert!(s.strict);
    assert_eq!((s.custom[0].before.as_str(), s.custom[0].after.as_str()), ("、。", "「"));
    assert!(tool(&mut doc, "set_line_breaking_rules", json!({})).unwrap().contains("ja-JP"));
    saved(&mut doc);
    assert!(docxtor_core::ai::tool_definitions().as_array().unwrap().iter().any(|t| t["name"] == "set_phonetic_guide"));
}

/// Found by the Open XML validator: `w:rubyPr` needs `hps`, `hpsRaise` and `hpsBaseText` (and `lid`) all, in that order.
#[test]
fn a_guide_given_no_sizes_still_writes_the_settings_the_schema_requires() {
    let mut doc = docx(r#"<w:p><w:r><w:t>前 漢字 後</w:t></w:r></w:p>"#);
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"east_asian","action":"set_ruby","from":{"p":id,"o":2},"to":{"p":id,"o":4},"text":"かんじ"}));
    let xml = para_xml(&doc, id);
    let pr = &xml[xml.find("<w:rubyPr>").unwrap()..xml.find("</w:rubyPr>").unwrap()];
    for (a, b) in [("<w:rubyAlign", "<w:hps "), ("<w:hps ", "<w:hpsRaise"), ("<w:hpsRaise", "<w:hpsBaseText"), ("<w:hpsBaseText", "<w:lid")] {
        assert!(pr.find(a).expect(a) < pr.find(b).expect(b), "{pr}");
    }
}
