//! Character formatting for the Font dialog and for display: underline styles and colours, raised and lowered text,
//! scale, kerning, outline and the like, text borders, patterned and theme shading, theme colours, emphasis marks,
//! fit text, Word 2010 text effects, change case, character styles (linked ones too) and the default formatting.

use docxtor_core::model::{find_block, walk_paragraphs, write_block};
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";

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
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:w14="{W14}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
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

fn rendered_para(doc: &Document, id: u32) -> Value {
    let all = serde_json::to_value(doc.render_snapshot()).unwrap();
    all["blocks"].as_array().unwrap().iter().find(|b| b["t"] == "p" && b["id"] == id).cloned().expect("paragraph")
}

/// The formatting of the first text segment of a paragraph, as the renderer reports it.
fn run_of(doc: &Document, id: u32) -> Value {
    rendered_para(doc, id)["segs"][0]["f"].clone()
}

fn text_of(doc: &Document, id: u32) -> String {
    docxtor_core::model::find_para(&doc.body, id).unwrap().plain_text()
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
fn reads_underline_position_scale_kerning_and_effects() {
    let doc = docx(
        r#"<w:p><w:r><w:rPr><w:u w:val="wavyHeavy" w:color="FF0000"/><w:position w:val="-6"/><w:w w:val="150"/><w:kern w:val="28"/><w:outline/><w:shadow/><w:emboss/><w:em w:val="underDot"/><w:fitText w:val="1440" w:id="77"/></w:rPr><w:t>abc</w:t></w:r></w:p>"#,
        "",
    );
    let f = run_of(&doc, first_para(&doc));
    assert_eq!((f["underline"].as_str(), f["underlineColor"].as_str()), (Some("wavyHeavy"), Some("FF0000")), "{f}");
    assert_eq!((f["position"].as_i64(), f["scale"].as_i64(), f["kern"].as_i64()), (Some(-6), Some(150), Some(28)));
    assert_eq!((f["outline"].as_bool(), f["shadow"].as_bool(), f["emboss"].as_bool()), (Some(true), Some(true), Some(true)));
    assert_eq!((f["em"].as_str(), f["fitText"].as_i64(), f["fitTextId"].as_i64()), (Some("underDot"), Some(1440), Some(77)));
}

#[test]
fn theme_colours_are_resolved_for_text_underline_and_shading() {
    // The blank document's theme is Office's: accent1 is 4472C4; the tint and shade are Word's.
    let doc = docx(
        r#"<w:p><w:r><w:rPr><w:color w:val="000000" w:themeColor="accent1" w:themeTint="33"/><w:u w:val="single" w:color="000000" w:themeColor="accent2" w:themeShade="80"/><w:shd w:val="pct25" w:color="FF0000" w:fill="FFFF00" w:themeFill="accent1" w:themeFillTint="33"/></w:rPr><w:t>abc</w:t></w:r></w:p>"#,
        "",
    );
    let f = run_of(&doc, first_para(&doc));
    assert_eq!(f["color"], "D9E2F3", "{f}");
    assert_eq!(f["colorTheme"]["name"], "accent1");
    assert_eq!(f["underlineColor"], "843C0C", "{f}");
    assert_eq!(f["shd"]["val"], "pct25");
    assert_eq!(f["shd"]["fill"], "D9E2F3", "{f}");
    assert_eq!(f["shading"], "D9E2F3");
}

#[test]
fn theme_colours_follow_a_change_of_theme_and_its_undo_and_redo() {
    // The document reads its theme once and keeps it: a change of theme, and undo and redo of one, must read it again,
    // both for a whole render and for the changed blocks drawn after a key.
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:color w:val="000000" w:themeColor="accent1"/></w:rPr><w:t>abc</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    let office = "4472C4";
    let facet = docxtor_core::themes::find_theme("facet").unwrap().colors.colors["accent1"].clone();
    assert_ne!(facet, office);
    let changed = |doc: &mut Document| {
        ops(doc, json!({"op":"insert_text","at":{"p":p,"o":3},"text":"d"}));
        let out = serde_json::to_value(doc.render_changed()).unwrap();
        out["blocks"].as_array().unwrap().iter().find(|b| b["id"] == p).expect("drawn again")["segs"][0]["f"]["color"].as_str().unwrap().to_string()
    };
    assert_eq!(run_of(&doc, p)["color"], office);
    assert_eq!(changed(&mut doc), office);
    ops(&mut doc, json!({"op":"set_theme","theme":"facet"}));
    assert_eq!(run_of(&doc, p)["color"], facet.as_str());
    assert_eq!(changed(&mut doc), facet);
    assert!(doc.undo()); // the key
    assert!(doc.undo()); // the theme
    assert_eq!(run_of(&doc, p)["color"], office);
    assert!(doc.redo()); // the theme again (a key typed here would end the redo)
    assert_eq!(run_of(&doc, p)["color"], facet.as_str());
    assert_eq!(changed(&mut doc), facet);
    assert!(doc.undo()); // that key
    assert!(doc.undo()); // the theme
    assert_eq!(changed(&mut doc), office);
}

#[test]
fn text_borders_and_effects_are_read() {
    let doc = docx(
        r#"<w:p><w:r><w:rPr><w:bdr w:val="single" w:sz="8" w:space="2" w:color="0000FF"/><w14:glow w14:rad="63500"><w14:schemeClr w14:val="accent1"><w14:alpha w14:val="60000"/></w14:schemeClr></w14:glow><w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000"><w14:srgbClr w14:val="000000"><w14:alpha w14:val="40000"/></w14:srgbClr></w14:shadow><w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr w14:val="FF0000"/></w14:solidFill></w14:textOutline><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="FF0000"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="0000FF"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000"/></w14:gradFill></w14:textFill><w14:ligatures w14:val="standardContextual"/><w14:numForm w14:val="oldStyle"/><w14:numSpacing w14:val="tabular"/><w14:stylisticSets><w14:styleSet w14:id="2"/></w14:stylisticSets><w14:cntxtAlts/></w:rPr><w:t>abc</w:t></w:r></w:p>"#,
        "",
    );
    let f = run_of(&doc, first_para(&doc));
    assert_eq!((f["border"]["val"].as_str(), f["border"]["sz"].as_i64(), f["border"]["space"].as_i64()), (Some("single"), Some(8), Some(2)), "{f}");
    let fx = &f["fx"];
    assert_eq!(fx["glow"]["radius"], 5.0, "{f}");
    assert_eq!(fx["glow"]["color"]["hex"], "4472C4");
    assert_eq!(fx["glow"]["color"]["alpha"], 0.6);
    assert_eq!((fx["shadow"]["dist"].as_f64(), fx["shadow"]["dir"].as_f64(), fx["shadow"]["blur"].as_f64()), (Some(3.0), Some(45.0), Some(4.0)));
    assert_eq!((fx["outline"]["width"].as_f64(), fx["outline"]["color"]["hex"].as_str()), (Some(0.75), Some("FF0000")));
    assert_eq!(fx["fill"]["stops"].as_array().map(|s| s.len()), Some(2));
    assert_eq!(fx["fill"]["angle"], 90.0);
    assert_eq!((fx["ligatures"].as_str(), fx["numForm"].as_str(), fx["numSpacing"].as_str()), (Some("standardContextual"), Some("oldStyle"), Some("tabular")));
    assert_eq!((fx["styleSets"][0].as_i64(), fx["cntxtAlts"].as_bool()), (Some(2), Some(true)));
}

#[test]
fn a_style_gives_the_character_formatting_and_the_run_overrides_it() {
    let styles = r#"<w:style w:type="character" w:customStyle="1" w:styleId="Fancy"><w:name w:val="Fancy"/><w:rPr><w:u w:val="dotted" w:color="00FF00"/><w:position w:val="4"/></w:rPr></w:style>"#;
    let doc = docx(
        r#"<w:p><w:r><w:rPr><w:rStyle w:val="Fancy"/></w:rPr><w:t>a</w:t></w:r><w:r><w:rPr><w:rStyle w:val="Fancy"/><w:u w:val="double"/></w:rPr><w:t>b</w:t></w:r></w:p>"#,
        styles,
    );
    let p = rendered_para(&doc, first_para(&doc));
    assert_eq!((p["segs"][0]["f"]["underline"].as_str(), p["segs"][0]["f"]["underlineColor"].as_str(), p["segs"][0]["f"]["position"].as_i64()), (Some("dotted"), Some("00FF00"), Some(4)), "{p}");
    // The run's own underline has no colour: the style's colour does not carry over to it.
    assert_eq!((p["segs"][1]["f"]["underline"].as_str(), p["segs"][1]["f"]["underlineColor"].as_str()), (Some("double"), None), "{p}");
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

#[test]
fn the_font_dialog_settings_are_written_in_schema_order_and_undone() {
    let mut doc = docx("<w:p><w:r><w:t>alpha beta</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    let at = |o| json!({"p": p, "o": o});
    ops(
        &mut doc,
        json!({"op":"format_text","from":at(0),"to":at(5),"format":{
            "underline":"dashLongHeavy","underlineColor":"C00000","dstrike":true,"vanish":true,"outline":true,"shadow":true,"emboss":true,
            "spacing":40,"position":-4,"scale":120,"kern":24,"color":"5B9BD5","themeColor":"accent1","themeShade":"BF",
            "em":"dot","border":{"val":"single","sz":4,"color":"auto","space":1},"shading":"FFFF00",
            "ligatures":"standard","numForm":"oldStyle","numSpacing":"tabular","styleSets":[3],"cntxtAlts":true
        }}),
    );
    let xml = para_xml(&doc, p);
    for needle in [
        r#"<w:u w:val="dashLongHeavy" w:color="C00000"/>"#,
        "<w:dstrike/>",
        "<w:vanish/>",
        "<w:outline/>",
        "<w:shadow/>",
        "<w:emboss/>",
        r#"<w:spacing w:val="40"/>"#,
        r#"<w:position w:val="-4"/>"#,
        r#"<w:w w:val="120"/>"#,
        r#"<w:kern w:val="24"/>"#,
        r#"<w:color w:val="5B9BD5" w:themeColor="accent1" w:themeShade="BF"/>"#,
        r#"<w:em w:val="dot"/>"#,
        r#"<w:bdr w:val="single" w:sz="4" w:space="1" w:color="auto"/>"#,
        r#"<w:shd w:val="clear" w:color="auto" w:fill="FFFF00"/>"#,
        r#"<w14:ligatures w14:val="standard"/>"#,
        r#"<w14:numForm w14:val="oldStyle"/>"#,
        r#"<w14:numSpacing w14:val="tabular"/>"#,
        r#"<w14:styleSet w14:id="3"/>"#,
        "<w14:cntxtAlts/>",
    ] {
        assert!(xml.contains(needle), "missing {needle} in {xml}");
    }
    // Word's order: effects first, then spacing, scale, kerning and position, then the underline, border, shading and the rest.
    let order: Vec<usize> = ["<w:dstrike", "<w:outline", "<w:shadow", "<w:emboss", "<w:vanish", "<w:color", "<w:spacing", "<w:w ", "<w:kern", "<w:position", "<w:u ", "<w:bdr", "<w:shd", "<w:em ", "<w14:ligatures", "<w14:numForm", "<w14:numSpacing", "<w14:stylisticSets", "<w14:cntxtAlts"]
        .iter()
        .map(|t| xml.find(t).unwrap_or_else(|| panic!("{t} in {xml}")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{xml}");
    // The rest of the text keeps what it had.
    assert!(xml.contains("beta"));

    let f = run_of(&doc, p);
    assert_eq!((f["underline"].as_str(), f["underlineColor"].as_str(), f["position"].as_i64(), f["scale"].as_i64(), f["spacing"].as_i64()), (Some("dashLongHeavy"), Some("C00000"), Some(-4), Some(120), Some(40)), "{f}");
    // The theme colour wins over the hex beside it: accent1 at shade BF.
    assert_eq!(f["colorTheme"]["name"], "accent1");
    assert_eq!(f["color"], "2F5496", "{f}");

    // The file is valid and declares w14 as Word does.
    let bytes = saved(&mut doc);
    let pkg = Package::open(&bytes).unwrap();
    let main = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(main.contains("xmlns:w14="), "{main}");
    let again = Document::open(&bytes).unwrap();
    assert_eq!(run_of(&again, p)["underlineColor"], "C00000");

    assert!(doc.undo());
    assert!(!para_xml(&doc, p).contains("w:position"));
}

#[test]
fn turning_a_setting_off_removes_it_or_overrides_what_a_style_gives() {
    let styles = r#"<w:style w:type="character" w:customStyle="1" w:styleId="Spread"><w:name w:val="Spread"/><w:rPr><w:spacing w:val="30"/><w:position w:val="6"/><w:w w:val="80"/><w:outline/></w:rPr></w:style>"#;
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:rStyle w:val="Spread"/><w:kern w:val="20"/><w:vanish/></w:rPr><w:t>abc</w:t></w:r></w:p>"#, styles);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":3},"format":{"spacing":0,"position":0,"scale":100,"outline":false,"kern":0,"vanish":false}}));
    let xml = para_xml(&doc, p);
    // What the style gives needs an explicit "normal"; what the run alone gave just goes.
    assert!(xml.contains(r#"<w:spacing w:val="0"/>"#) && xml.contains(r#"<w:position w:val="0"/>"#) && xml.contains(r#"<w:w w:val="100"/>"#), "{xml}");
    assert!(xml.contains(r#"<w:outline w:val="0"/>"#), "{xml}");
    assert!(!xml.contains("w:kern") && !xml.contains("vanish"), "{xml}");
    saved(&mut doc);
}

#[test]
fn changing_the_underline_style_keeps_its_colour_and_none_removes_it() {
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:u w:val="single" w:color="FF0000" w:themeColor="accent1"/></w:rPr><w:t>abc</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    let range = |fmt: Value| json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":3},"format":fmt});
    ops(&mut doc, range(json!({"underline":"double"})));
    assert!(para_xml(&doc, p).contains(r#"<w:u w:val="double" w:color="FF0000" w:themeColor="accent1"/>"#), "{}", para_xml(&doc, p));
    ops(&mut doc, range(json!({"underlineColor":"auto"})));
    assert!(para_xml(&doc, p).contains(r#"<w:u w:val="double"/>"#), "{}", para_xml(&doc, p));
    ops(&mut doc, range(json!({"underline":"none"})));
    assert!(!para_xml(&doc, p).contains("<w:u"));
}

#[test]
fn automatic_colour_is_written_as_auto() {
    let mut doc = docx(r#"<w:p><w:r><w:rPr><w:color w:val="FF0000"/></w:rPr><w:t>abc</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":3},"format":{"color":"auto"}}));
    assert!(para_xml(&doc, p).contains(r#"<w:color w:val="auto"/>"#), "{}", para_xml(&doc, p));
}

#[test]
fn a_whole_shading_with_a_pattern_and_theme_colours_is_written() {
    let mut doc = docx("<w:p><w:r><w:t>abc</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":3},"format":{"shd":{"val":"diagStripe","color":"FF0000","fill":"FFFF00","themeFill":"accent2"}}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:shd w:val="diagStripe" w:color="FF0000" w:fill="FFFF00" w:themeFill="accent2"/>"#), "{xml}");
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":3},"format":{"shading":"none"}}));
    assert!(!para_xml(&doc, p).contains("w:shd"));
    saved(&mut doc);
}

#[test]
fn fit_text_runs_get_one_id_and_are_removed_with_zero() {
    let mut doc = docx(r#"<w:p><w:r><w:t>one</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>two</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":6},"format":{"fitText":1440}}));
    let xml = para_xml(&doc, p);
    let ids: Vec<&str> = xml.match_indices(r#"<w:fitText w:val="1440" w:id=""#).map(|(i, m)| &xml[i + m.len()..i + m.len() + 7]).collect();
    assert_eq!(ids.len(), 2, "{xml}");
    assert_eq!(ids[0], ids[1], "{xml}");
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":6},"format":{"fitText":0}}));
    assert!(!para_xml(&doc, p).contains("fitText"));
}

#[test]
fn the_default_formatting_is_set_in_doc_defaults_and_undone() {
    let mut doc = docx(r#"<w:p><w:r><w:t>abc</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    let before = run_of(&doc, p);
    ops(&mut doc, json!({"op":"set_default_format","format":{"font":"Georgia","size":14,"color":"336699","ligatures":"standard","spacing":10}}));
    let f = run_of(&doc, p);
    assert_eq!((f["font"].as_str(), f["size"].as_f64(), f["color"].as_str(), f["spacing"].as_i64()), (Some("Georgia"), Some(14.0), Some("336699"), Some(10)), "{f}");
    let bytes = saved(&mut doc);
    let styles = String::from_utf8(Package::open(&bytes).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains("<w:rPrDefault>") && styles.contains(r#"w:ascii="Georgia""#) && styles.contains("<w14:ligatures"), "{styles}");
    assert!(styles.contains("xmlns:w14="), "{styles}");
    assert!(doc.undo());
    assert_eq!(run_of(&doc, p)["font"], before["font"]);
}

// ---------------------------------------------------------------------------
// Change case
// ---------------------------------------------------------------------------

#[test]
fn change_case_keeps_the_formatting_and_is_one_undo_step() {
    let mut doc = docx(
        r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">hello </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>wORLD. second one</w:t></w:r></w:p><w:p><w:r><w:t>next paragraph</w:t></w:r></w:p>"#,
        "",
    );
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    let at = |p: u32, o: usize| json!({"p": p, "o": o});
    ops(&mut doc, json!({"op":"change_case","from":at(ids[0],0),"to":at(ids[1],4),"mode":"upper"}));
    assert_eq!(text_of(&doc, ids[0]), "HELLO WORLD. SECOND ONE");
    assert_eq!(text_of(&doc, ids[1]), "NEXT paragraph");
    let xml = para_xml(&doc, ids[0]);
    assert!(xml.contains("<w:b/>") && xml.contains("<w:i/>"), "{xml}");
    ops(&mut doc, json!({"op":"change_case","from":at(ids[0],0),"to":at(ids[0],23),"mode":"sentence"}));
    assert_eq!(text_of(&doc, ids[0]), "Hello world. Second one");
    ops(&mut doc, json!({"op":"change_case","from":at(ids[0],0),"to":at(ids[0],23),"mode":"title"}));
    assert_eq!(text_of(&doc, ids[0]), "Hello World. Second One");
    ops(&mut doc, json!({"op":"change_case","from":at(ids[0],0),"to":at(ids[0],23),"mode":"toggle"}));
    assert_eq!(text_of(&doc, ids[0]), "hELLO wORLD. sECOND oNE");
    ops(&mut doc, json!({"op":"change_case","from":at(ids[0],0),"to":at(ids[0],23),"mode":"lower"}));
    assert_eq!(text_of(&doc, ids[0]), "hello world. second one");
    saved(&mut doc);
    assert!(doc.undo());
    assert_eq!(text_of(&doc, ids[0]), "hELLO wORLD. sECOND oNE");
}

#[test]
fn change_case_follows_the_paragraphs_language() {
    let mut doc = docx(
        r#"<w:p><w:r><w:rPr><w:lang w:val="tr-TR"/></w:rPr><w:t>istanbul</w:t></w:r></w:p><w:p><w:r><w:rPr><w:lang w:val="en-US"/></w:rPr><w:t>istanbul</w:t></w:r></w:p>"#,
        "",
    );
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    ops(&mut doc, json!({"op":"change_case","from":{"p":ids[0],"o":0},"to":{"p":ids[1],"o":9},"mode":"upper"}));
    assert_eq!(text_of(&doc, ids[0]), "\u{130}STANBUL");
    assert_eq!(text_of(&doc, ids[1]), "ISTANBUL");
}

#[test]
fn change_case_leaves_a_tab_and_unchanged_text_alone() {
    let mut doc = docx(r#"<w:p><w:r><w:t>ab</w:t><w:tab/><w:t>cd</w:t></w:r></w:p>"#, "");
    let p = first_para(&doc);
    let before = para_xml(&doc, p);
    ops(&mut doc, json!({"op":"change_case","from":{"p":p,"o":0},"to":{"p":p,"o":5},"mode":"lower"}));
    // Nothing changed: no runs were split.
    assert_eq!(para_xml(&doc, p), before);
    ops(&mut doc, json!({"op":"change_case","from":{"p":p,"o":0},"to":{"p":p,"o":5},"mode":"upper"}));
    assert_eq!(text_of(&doc, p), "AB\tCD");
    assert!(para_xml(&doc, p).contains("<w:tab/>"));
}

// ---------------------------------------------------------------------------
// Character styles
// ---------------------------------------------------------------------------

#[test]
fn a_character_style_is_applied_found_by_name_and_cleared() {
    let mut doc = docx("<w:p><w:r><w:t>alpha beta</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":5},"format":{"style":"Strong"}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:rStyle w:val="Strong"/>"#), "{xml}");
    // Word's own definition was added to the file.
    let bytes = saved(&mut doc);
    let styles = String::from_utf8(Package::open(&bytes).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:styleId="Strong""#) && styles.contains(r#"w:type="character""#), "{styles}");
    let st = doc.format_at(Pos::new(p, 3)).unwrap();
    assert_eq!(st.char_style.as_deref(), Some("Strong"));
    assert_eq!(st.run.bold, Some(true));
    assert_eq!(doc.format_at(Pos::new(p, 8)).unwrap().char_style, None);
    // Clearing it.
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":5},"format":{"style":""}}));
    assert!(!para_xml(&doc, p).contains("rStyle"));
    assert_eq!(doc.format_at(Pos::new(p, 3)).unwrap().char_style, None);
    // A paragraph style that isn't linked (Caption, not Title or a heading) can't be applied to a range.
    let err = doc.apply_json(&json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":5},"format":{"style":"Caption"}}).to_string(), false);
    assert!(err.is_err());
}

#[test]
fn a_linked_paragraph_style_applies_to_a_range_as_its_character_style() {
    let mut doc = docx("<w:p><w:r><w:t>alpha beta</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    // Heading 1 is added with a link to Heading1Char, which the file does not have yet.
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":5},"format":{"style":"Heading 1"}}));
    assert!(para_xml(&doc, p).contains(r#"<w:rStyle w:val="Heading1Char"/>"#), "{}", para_xml(&doc, p));
    let bytes = saved(&mut doc);
    let styles = String::from_utf8(Package::open(&bytes).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:styleId="Heading1Char""#) && styles.contains(r#"<w:link w:val="Heading1"/>"#), "{styles}");
    // The text takes the heading's size and colour.
    let f = run_of(&doc, p);
    assert_eq!(f["size"], 20.0, "{f}");
    // The styles list says which is linked to which.
    let list = serde_json::to_value(doc.style_list()).unwrap();
    let h1 = list.as_array().unwrap().iter().find(|s| s["id"] == "Heading1").unwrap();
    assert_eq!(h1["link"], "Heading1Char");
}

#[test]
fn old_files_still_open_and_character_styles_list_with_their_kind() {
    let doc = docx("<w:p><w:r><w:t>x</w:t></w:r></w:p>", r#"<w:style w:type="character" w:styleId="Mine"><w:name w:val="Mine"/><w:qFormat/></w:style>"#);
    let list = serde_json::to_value(doc.style_list()).unwrap();
    let mine = list.as_array().unwrap().iter().find(|s| s["id"] == "Mine").unwrap();
    assert_eq!((mine["kind"].as_str(), mine["quick"].as_bool()), (Some("character"), Some(true)));
}

// ---------------------------------------------------------------------------
// The assistant's tools
// ---------------------------------------------------------------------------

#[test]
fn the_assistant_can_change_case_and_format_with_the_new_options() {
    use docxtor_core::ai::call_tool;
    let mut doc = docx("<w:p><w:r><w:t>the quick brown fox. jumps over</w:t></w:r></w:p>", "");
    let p = first_para(&doc);
    call_tool(&mut doc, "change_case", &json!({"id": format!("p{p}"), "mode": "sentence"})).unwrap();
    assert_eq!(text_of(&doc, p), "The quick brown fox. Jumps over");
    call_tool(&mut doc, "change_case", &json!({"id": format!("p{p}"), "text": "quick", "mode": "upper"})).unwrap();
    assert_eq!(text_of(&doc, p), "The QUICK brown fox. Jumps over");
    call_tool(&mut doc, "format_text", &json!({"id": format!("p{p}"), "text": "brown", "format": {"underline": "wave", "underlineColor": "FF0000", "position": 4, "dstrike": true}})).unwrap();
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:u w:val="wave" w:color="FF0000"/>"#) && xml.contains(r#"<w:position w:val="4"/>"#) && xml.contains("<w:dstrike/>"), "{xml}");
    assert!(call_tool(&mut doc, "change_case", &json!({"id": format!("p{p}"), "mode": "shout"})).is_err());
}
