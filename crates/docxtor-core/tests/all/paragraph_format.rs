//! Paragraph formatting for the Paragraph and Tabs dialogs, and for right-to-left text: tab stops and the
//! default tab width, borders between paragraphs, patterned and theme shading, spacing in lines, indents in
//! characters, mirrored indents, complex-script formatting, right-to-left tables and the right-to-left gutter.

use docxtor_core::model::{find_block, walk_paragraphs, write_block};
use docxtor_core::props::ParaFmt;
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

/// A document whose body is the given XML (before the section properties) and whose styles get `extra_styles`.
fn docx(body: &str, extra_styles: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    if !extra_styles.is_empty() {
        let s = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/styles.xml", s.replace("</w:styles>", &format!("{extra_styles}</w:styles>")).into_bytes());
    }
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
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

fn fmt_at(doc: &Document, id: u32) -> ParaFmt {
    doc.format_at(Pos::new(id, 0)).unwrap().para
}

fn rendered(doc: &Document) -> Value {
    serde_json::to_value(doc.render_snapshot()).unwrap()
}

fn rendered_para(doc: &Document, id: u32) -> Value {
    rendered(doc)["blocks"].as_array().unwrap().iter().find(|b| b["t"] == "p" && b["id"] == id).cloned().expect("paragraph")
}

fn saved(doc: &mut Document) -> Vec<u8> {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    bytes
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

#[test]
fn reads_chars_lines_mirror_and_between_borders() {
    let doc = docx(
        r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="1" w:color="auto"/><w:between w:val="dashed" w:sz="6" w:space="2" w:color="FF0000"/><w:bar w:val="thick" w:sz="12" w:space="0" w:color="0000FF"/></w:pBdr><w:spacing w:beforeLines="50" w:afterLines="25" w:before="100"/><w:ind w:leftChars="200" w:firstLineChars="100" w:right="360"/><w:mirrorIndents/></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"#,
        "",
    );
    let f = fmt_at(&doc, first_para(&doc));
    assert_eq!((f.space_before_lines, f.space_after_lines, f.space_before), (Some(50), Some(25), Some(100)));
    assert_eq!((f.ind_left_chars, f.ind_first_line_chars, f.ind_right), (Some(200), Some(100), Some(360)));
    assert_eq!(f.mirror_indents, Some(true));
    let b = f.borders.unwrap();
    assert_eq!(b.between.as_ref().map(|x| (x.val.as_str(), x.color.as_str())), Some(("dashed", "FF0000")));
    assert_eq!(b.bar.as_ref().map(|x| (x.val.as_str(), x.sz)), Some(("thick", 12)));
}

#[test]
fn shading_patterns_and_theme_colours_are_resolved() {
    let doc = docx(
        r#"<w:p><w:pPr><w:shd w:val="pct25" w:color="FF0000" w:fill="FFFF00"/></w:pPr><w:r><w:t>a</w:t></w:r></w:p>
<w:p><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="FFFFFF" w:themeFill="accent1" w:themeFillTint="33"/></w:pPr><w:r><w:t>b</w:t></w:r></w:p>
<w:p><w:pPr><w:shd w:val="solid" w:color="000000" w:fill="auto" w:themeColor="accent2" w:themeShade="80"/></w:pPr><w:r><w:t>c</w:t></w:r></w:p>"#,
        "",
    );
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    let a = rendered_para(&doc, ids[0]);
    assert_eq!(a["fmt"]["shd"]["val"], "pct25");
    assert_eq!((a["fmt"]["shd"]["color"].as_str(), a["fmt"]["shd"]["fill"].as_str()), (Some("FF0000"), Some("FFFF00")));
    // A pattern keeps its fill as the flat colour a simple renderer can use.
    assert_eq!(a["fmt"]["shading"], "FFFF00");
    // The Office theme's accent1 (4472C4) at tint 0x33: its lightness scaled to a fifth and the rest white (Word shows D9E2F3).
    let b = rendered_para(&doc, ids[1]);
    assert_eq!(b["fmt"]["shd"]["fill"], "D9E2F3", "{b}");
    assert!(b["fmt"]["shd"].get("themeFill").is_none(), "theme references are resolved away: {b}");
    // Solid shading shows its colour: accent2 (ED7D31) at shade 0x80 (half as light).
    let c = rendered_para(&doc, ids[2]);
    assert_eq!(c["fmt"]["shd"]["color"], "843C0C", "{c}");
    assert_eq!(c["fmt"]["shading"], "843C0C");
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

#[test]
fn line_spacing_rules_auto_spacing_and_toggles() {
    let mut doc = docx("<w:p><w:r><w:t>alpha</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"line":360,"lineRule":"exact","spaceBefore":120,"beforeAuto":false,"afterAuto":true,"contextualSpacing":true,"mirrorIndents":true,"outlineLvl":2,"bidi":true}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"w:line="360""#) && xml.contains(r#"w:lineRule="exact""#), "{xml}");
    assert!(xml.contains(r#"w:afterAutospacing="1""#) && xml.contains(r#"w:after="280""#), "{xml}");
    assert!(!xml.contains("beforeAutospacing"), "{xml}");
    assert!(xml.contains("<w:contextualSpacing/>") && xml.contains("<w:mirrorIndents/>") && xml.contains("<w:bidi/>"), "{xml}");
    assert!(xml.contains(r#"<w:outlineLvl w:val="2"/>"#), "{xml}");
    // Schema order: bidi, spacing, ind-less, contextualSpacing, mirrorIndents, outlineLvl.
    let at: Vec<usize> = ["<w:bidi", "<w:spacing", "<w:contextualSpacing", "<w:mirrorIndents", "<w:outlineLvl"].iter().map(|t| xml.find(t).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{xml}");
    let f = fmt_at(&doc, p);
    assert_eq!((f.line, f.line_rule.as_deref(), f.after_auto, f.bidi, f.outline_lvl), (Some(360), Some("exact"), Some(true), Some(true), Some(2)));
    let again = Document::open(&saved(&mut doc)).unwrap();
    assert_eq!(fmt_at(&again, p).line_rule.as_deref(), Some("exact"));

    // Off again: toggles disappear, auto spacing goes, the outline level can be removed or set to body text.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"afterAuto":false,"contextualSpacing":false,"mirrorIndents":false,"bidi":false,"outlineLvl":-1,"line":2,"lineRule":"atLeast"}}));
    let xml = para_xml(&doc, p);
    assert!(!xml.contains("Autospacing") && !xml.contains("contextualSpacing") && !xml.contains("mirrorIndents") && !xml.contains("<w:bidi") && !xml.contains("outlineLvl"), "{xml}");
    assert!(xml.contains(r#"w:lineRule="atLeast""#), "{xml}");
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"outlineLvl":9}}));
    assert!(para_xml(&doc, p).contains(r#"<w:outlineLvl w:val="9"/>"#));

    // Undo restores the paragraph.
    assert!(doc.undo());
    assert!(!para_xml(&doc, p).contains("outlineLvl"));
}

#[test]
fn editing_an_indent_in_twips_drops_the_character_units() {
    let mut doc = docx(r#"<w:p><w:pPr><w:ind w:leftChars="200" w:left="420" w:rightChars="100" w:right="210" w:firstLineChars="100"/></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"indLeft":720,"indRight":360,"indFirst":-360}}));
    let xml = para_xml(&doc, p);
    assert!(!xml.contains("Chars=\"200\"") && !xml.contains("rightChars") && !xml.contains("firstLineChars"), "{xml}");
    assert!(xml.contains(r#"w:left="720""#) && xml.contains(r#"w:right="360""#) && xml.contains(r#"w:hanging="360""#), "{xml}");
    saved(&mut doc);
}

#[test]
fn tab_stops_are_written_with_clears_for_inherited_ones() {
    let styles = r#"<w:style w:type="paragraph" w:customStyle="1" w:styleId="Tabbed"><w:name w:val="Tabbed"/><w:basedOn w:val="Normal"/><w:pPr><w:tabs><w:tab w:val="left" w:pos="1440"/><w:tab w:val="right" w:leader="dot" w:pos="9360"/></w:tabs></w:pPr></w:style>"#;
    let mut doc = docx(r#"<w:p><w:pPr><w:pStyle w:val="Tabbed"/></w:pPr><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t></w:r></w:p>"#, styles);
    let p = first_para(&doc);
    let f = fmt_at(&doc, p);
    assert_eq!(f.tabs.as_ref().map(|t| t.len()), Some(2));

    // Keep the right stop, drop the left one, add a decimal stop with a leader and a bar.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"tabs":[
        {"val":"decimal","pos":4320,"leader":"hyphen"},
        {"val":"bar","pos":5000},
        {"val":"right","pos":9360,"leader":"dot"}
    ]}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:tab w:val="clear" w:pos="1440"/>"#), "{xml}");
    assert!(xml.contains(r#"<w:tab w:val="decimal" w:leader="hyphen" w:pos="4320"/>"#) && xml.contains(r#"<w:tab w:val="bar" w:pos="5000"/>"#), "{xml}");
    // The right stop is the style's own: not written again.
    assert!(!xml.contains(r#"w:pos="9360""#), "{xml}");
    let f = fmt_at(&doc, p);
    let got: Vec<(String, i64, Option<String>)> = f.tabs.unwrap().into_iter().map(|t| (t.val, t.pos, t.leader)).collect();
    assert_eq!(got, vec![("decimal".into(), 4320, Some("hyphen".into())), ("bar".into(), 5000, None), ("right".into(), 9360, Some("dot".into()))]);
    // schema order: tabs before spacing/jc, after pStyle
    assert!(xml.find("<w:pStyle").unwrap() < xml.find("<w:tabs").unwrap());
    let again = Document::open(&saved(&mut doc)).unwrap();
    assert_eq!(fmt_at(&again, p).tabs.unwrap().len(), 3);

    // Clearing every stop leaves no stops (the style's two are cleared), and putting the style's back removes the element.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"tabs":[]}}));
    let xml = para_xml(&doc, p);
    assert_eq!(xml.matches(r#"w:val="clear""#).count(), 2, "{xml}");
    assert!(fmt_at(&doc, p).tabs.unwrap().is_empty());
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"tabs":[{"val":"left","pos":1440},{"val":"right","pos":9360,"leader":"dot"}]}}));
    assert!(!para_xml(&doc, p).contains("<w:tabs"), "{}", para_xml(&doc, p));
    assert!(doc.undo());
    assert!(para_xml(&doc, p).contains(r#"w:val="clear""#));
}

#[test]
fn a_paragraphs_own_num_tab_stop_survives_tab_edits_that_leave_it_out() {
    let mut doc = docx(r#"<w:p><w:pPr><w:tabs><w:tab w:val="num" w:pos="1080"/></w:tabs><w:ind w:left="720" w:hanging="360"/></w:pPr><w:r><w:t>a</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    // The ruler and the Tabs dialog never show a num stop, so they never send it.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"tabs":[{"val":"left","pos":4320}]}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:tab w:val="num" w:pos="1080"/>"#) && xml.contains(r#"<w:tab w:val="left" w:pos="4320"/>"#), "{xml}");
    assert!(xml.find(r#"w:pos="1080""#).unwrap() < xml.find(r#"w:pos="4320""#).unwrap(), "stops stay in order: {xml}");
    // Clearing the list the person can see leaves the one they cannot.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"tabs":[]}}));
    assert!(para_xml(&doc, p).contains(r#"<w:tab w:val="num" w:pos="1080"/>"#), "{}", para_xml(&doc, p));
    // A stop the person puts at the same place replaces it.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"tabs":[{"val":"left","pos":1080}]}}));
    let xml = para_xml(&doc, p);
    assert!(!xml.contains(r#"w:val="num""#) && xml.contains(r#"<w:tab w:val="left" w:pos="1080"/>"#), "{xml}");
    assert_valid(&saved(&mut doc));
}

#[test]
fn default_tab_stop_is_read_set_and_undone() {
    let mut doc = docx("<w:p><w:r><w:t>x</w:t></w:r></w:p>", "");
    assert_eq!(doc.page_setup().default_tab, 720);
    ops(&mut doc, json!({"op":"set_default_tab_stop","twips":1080}));
    assert_eq!(doc.page_setup().default_tab, 1080);
    let bytes = saved(&mut doc);
    let pkg = Package::open(&bytes).unwrap();
    let settings = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
    assert!(settings.contains(r#"<w:defaultTabStop w:val="1080"/>"#) && settings.matches("defaultTabStop").count() == 1, "{settings}");
    assert_eq!(Document::open(&bytes).unwrap().page_setup().default_tab, 1080);
    assert!(doc.undo());
    assert_eq!(doc.page_setup().default_tab, 720);
    assert!(doc.apply_json(&json!({"op":"set_default_tab_stop","twips":0}).to_string(), false).is_err());
}

#[test]
fn between_and_bar_borders_are_set_kept_and_removed() {
    let mut doc = docx(r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="4" w:space="1" w:color="auto"/><w:between w:val="single" w:sz="4" w:space="1" w:color="auto"/></w:pBdr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    let line = |val: &str| json!({"val":val,"sz":8,"color":"FF0000","space":2});
    // Setting the sides keeps the between border, as before.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"borders":{"top":line("single"),"bottom":line("single")}}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains("<w:between") && xml.find("<w:bottom").unwrap() < xml.find("<w:between").unwrap(), "{xml}");
    // Both can be set, with the spec's order: top, left, bottom, right, between, bar.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"borders":{"top":line("single"),"between":line("dotted"),"bar":line("thick")}}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:between w:val="dotted""#) && xml.contains(r#"<w:bar w:val="thick""#) && xml.find("<w:between").unwrap() < xml.find("<w:bar").unwrap(), "{xml}");
    let f = fmt_at(&doc, p).borders.unwrap();
    assert_eq!((f.between.unwrap().val, f.bar.unwrap().val), ("dotted".to_string(), "thick".to_string()));
    // A border with the style none removes it.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"borders":{"top":line("single"),"between":line("none")}}}));
    let xml = para_xml(&doc, p);
    assert!(!xml.contains("<w:between") && xml.contains("<w:bar"), "{xml}");
    saved(&mut doc);
}

#[test]
fn patterned_shading_is_written_with_its_colours() {
    let mut doc = docx("<w:p><w:r><w:t>x</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"shd":{"val":"diagStripe","color":"336699","fill":"FFEEAA"}}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:shd w:val="diagStripe" w:color="336699" w:fill="FFEEAA"/>"#), "{xml}");
    let again = Document::open(&saved(&mut doc)).unwrap();
    let shd = fmt_at(&again, p).shd.unwrap();
    assert_eq!((shd.val.as_str(), shd.color.as_deref(), shd.fill.as_deref()), ("diagStripe", Some("336699"), Some("FFEEAA")));
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"shd":{"val":"nil"}}}));
    assert!(!para_xml(&doc, p).contains("<w:shd"));
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[test]
fn absolute_position_tabs_say_where_they_align() {
    let doc = docx(
        r#"<w:p><w:r><w:t>left</w:t></w:r><w:r><w:ptab w:relativeTo="margin" w:alignment="center" w:leader="none"/></w:r><w:r><w:t>mid</w:t></w:r><w:r><w:ptab w:relativeTo="indent" w:alignment="right" w:leader="dot"/></w:r><w:r><w:t>end</w:t></w:r></w:p>"#,
        "",
    );
    let p = rendered_para(&doc, first_para(&doc));
    let ptabs: Vec<&str> = p["segs"].as_array().unwrap().iter().filter(|s| s["k"] == "obj").filter_map(|s| s["x"].as_str()).collect();
    assert_eq!(ptabs, ["ptab:center:margin:none", "ptab:right:indent:dot"]);
    // Each takes one position, as a tab does; an export still sees a tab character.
    assert_eq!(p["len"], 4 + 1 + 3 + 1 + 3);
}

#[test]
fn complex_script_text_takes_the_complex_script_size_bold_and_italic() {
    let doc = docx(
        r#"<w:p><w:r><w:rPr><w:b/><w:i/><w:sz w:val="24"/><w:szCs w:val="40"/><w:rFonts w:ascii="Calibri" w:cs="Arial"/></w:rPr><w:t xml:space="preserve">Hi שלום there</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:rtl/><w:b/><w:bCs/><w:iCs/><w:sz w:val="24"/><w:szCs w:val="30"/></w:rPr><w:t>مرحبا 123</w:t></w:r></w:p>"#,
        "",
    );
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    let a = rendered_para(&doc, ids[0]);
    let segs = a["segs"].as_array().unwrap();
    assert_eq!(segs.len(), 3, "{segs:?}");
    // Latin text keeps b, i and sz; the Hebrew word has no bCs or iCs, so it is plain, at szCs, in the cs font.
    assert_eq!((segs[0]["x"].as_str(), segs[0]["f"]["bold"].as_bool(), segs[0]["f"]["size"].as_f64()), (Some("Hi "), Some(true), Some(12.0)));
    assert_eq!((segs[1]["x"].as_str(), segs[1]["f"]["bold"].as_bool(), segs[1]["f"]["italic"].as_bool(), segs[1]["f"]["size"].as_f64(), segs[1]["f"]["font"].as_str()), (Some("שלום "), Some(false), Some(false), Some(20.0), Some("Arial")));
    assert_eq!((segs[1]["o"].as_u64(), segs[2]["o"].as_u64(), segs[2]["x"].as_str()), (Some(3), Some(8), Some("there")));
    // A right-to-left run is complex script throughout, digits included.
    let b = rendered_para(&doc, ids[1]);
    let s = &b["segs"].as_array().unwrap()[0];
    assert_eq!((s["x"].as_str(), s["f"]["bold"].as_bool(), s["f"]["italic"].as_bool(), s["f"]["size"].as_f64()), (Some("مرحبا 123"), Some(true), Some(true), Some(15.0)));
}

#[test]
fn right_to_left_table_and_gutter_are_reported() {
    let mut doc = docx(
        r#"<w:tbl><w:tblPr><w:bidiVisual/><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#,
        "",
    );
    let tbl = rendered(&doc)["blocks"].as_array().unwrap().iter().find(|b| b["t"] == "tbl").cloned().unwrap();
    assert_eq!(tbl["bidiVisual"], true);
    // The gutter and which side it is on come from the section properties.
    let mut pkg = Package::open(&saved(&mut doc)).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    pkg.set("word/document.xml", xml.replace(r#"w:gutter="0"/>"#, r#"w:gutter="360"/>"#).replace("</w:sectPr>", "<w:rtlGutter/></w:sectPr>").into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let s = &doc.page_setup().sections[0];
    assert_eq!((s.gutter, s.rtl_gutter), (360, true));
}

#[test]
fn the_format_paragraphs_tool_reaches_the_new_settings() {
    use docxtor_core::ai::call_tool;
    let mut doc = docx("<w:p><w:r><w:t>alpha</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    call_tool(
        &mut doc,
        "format_paragraphs",
        &json!({"ids":[p],"align":"distribute","direction":"rtl","line_spacing_exactly_pt":18,"indent_right_pt":36,"mirror_indents":true,"outline_level":2,
                "dont_add_space_between_same_style":true,"dont_hyphenate":true,
                "tab_stops":[{"position_pt":72,"kind":"decimal","leader":"dot"},{"position_pt":432,"kind":"right"}]}),
    )
    .unwrap();
    let xml = para_xml(&doc, p);
    for want in [r#"<w:jc w:val="distribute"/>"#, "<w:bidi/>", r#"w:line="360" w:lineRule="exact""#, r#"w:right="720""#, "<w:mirrorIndents/>", r#"<w:outlineLvl w:val="1"/>"#, "<w:contextualSpacing/>", "<w:suppressAutoHyphens/>", r#"<w:tab w:val="decimal" w:leader="dot" w:pos="1440"/>"#, r#"<w:tab w:val="right" w:pos="8640"/>"#] {
        assert!(xml.contains(want), "{want} in {xml}");
    }
    assert!(call_tool(&mut doc, "format_paragraphs", &json!({"ids":[p],"direction":"up"})).is_err());
    assert!(call_tool(&mut doc, "format_paragraphs", &json!({"ids":[p],"tab_stops":[{"position_pt":10,"kind":"wavy"}]})).is_err());
    call_tool(&mut doc, "format_paragraphs", &json!({"ids":[p],"tab_stops":[],"outline_level":0})).unwrap();
    let xml = para_xml(&doc, p);
    assert!(!xml.contains("<w:tabs") && xml.contains(r#"<w:outlineLvl w:val="9"/>"#), "{xml}");
    saved(&mut doc);
}
