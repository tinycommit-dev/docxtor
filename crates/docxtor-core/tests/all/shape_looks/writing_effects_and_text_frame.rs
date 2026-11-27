//! Tests of: Writing effects and writing the text frame.

use super::*;

#[test]
fn writes_each_effect_as_word_does_and_reads_it_back() {
    let mut doc = Sh::new().open();
    set(
        &mut doc,
        json!({"effects": {
            "shadow": {"color": "112233", "alpha": 0.5, "blur_pt": 6, "dist_pt": 4, "angle": 60, "sx": 95, "sy": 95},
            "inner_shadow": {"blur_pt": 3, "dist_pt": 2, "angle": 135},
            "glow": {"color": "FFC000", "alpha": 0.4, "radius_pt": 10},
            "soft_edge_pt": 5,
            "reflection": {"blur_pt": 1, "start_alpha": 0.6, "end_alpha": 0, "dist_pt": 3, "end_pos": 50},
            "bevel": {"preset": "circle", "w_pt": 4, "h_pt": 4},
            "extrusion": {"height_pt": 12, "color": "666666"}
        }}),
    );
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:outerShdw blurRad="76200" dist="50800" dir="3600000" algn="tl" rotWithShape="0" sx="95000" sy="95000"><a:srgbClr val="112233"><a:alpha val="50000"/></a:srgbClr></a:outerShdw>"#) || xml.contains("<a:outerShdw"), "{xml}");
    assert!(fallback_of(&xml).contains(r##"<v:shadow on="t" color="#112233" opacity="0.5" offset="2pt,3.46pt"/>"##), "{}", fallback_of(&xml));
    for needle in ["<a:innerShdw", r#"<a:glow rad="127000">"#, r#"<a:softEdge rad="63500"/>"#, "<a:reflection", r#"<a:sp3d extrusionH="152400">"#, r#"<a:bevelT w="50800" h="50800" prst="circle"/>"#, "<a:scene3d><a:camera prst=\"obliqueTopRight\">", r#"<a:extrusionClr><a:srgbClr val="666666"/>"#] {
        assert!(xml.contains(needle), "{needle} missing: {xml}");
    }
    // Word's order inside the shape properties and the effect list.
    let order = ["<a:ln", "<a:effectLst>", "<a:scene3d>", "<a:sp3d"];
    let pos: Vec<usize> = order.iter().filter_map(|n| xml.find(n)).collect();
    if pos.len() == order.len() {
        assert!(pos.windows(2).all(|w| w[0] < w[1]), "{pos:?}");
    }
    let lst = &xml[xml.find("<a:effectLst>").unwrap()..xml.find("</a:effectLst>").unwrap()];
    let idx = |n: &str| lst.find(n).unwrap_or_else(|| panic!("{n} in {lst}"));
    assert!(idx("<a:glow") < idx("<a:innerShdw") && idx("<a:innerShdw") < idx("<a:outerShdw") && idx("<a:outerShdw") < idx("<a:reflection") && idx("<a:reflection") < idx("<a:softEdge"));
    // Reading gives back what was set.
    let e = shape(&doc)["effects"].clone();
    assert_eq!(e["shadow"]["color"], "112233");
    approx(&e["shadow"]["alpha"], 0.5);
    approx(&e["shadow"]["blurPt"], 6.0);
    approx(&e["shadow"]["distPt"], 4.0);
    approx(&e["shadow"]["angle"], 60.0);
    approx(&e["shadow"]["sx"], 95.0);
    approx(&e["glow"]["radiusPt"], 10.0);
    approx(&e["softEdgePt"], 5.0);
    approx(&e["reflection"]["startAlpha"], 0.6);
    approx(&e["reflection"]["endPos"], 50.0);
    assert_eq!(e["bevel"]["preset"], "circle");
    approx(&e["extrusion"]["heightPt"], 12.0);
    assert!(e["extrusion"]["dir"].is_array(), "a side camera gives the depth a direction");
    assert_eq!(shape(&Document::open(&bytes).unwrap())["effects"], e);
}

#[test]
fn changing_one_effect_keeps_the_others_and_none_takes_them_away() {
    let mut doc = Sh::new().sppr(EFFECTS).open();
    set(&mut doc, json!({"effects": {"shadow": {"blur_pt": 10}, "glow": "none"}}));
    let e = shape(&doc)["effects"].clone();
    approx(&e["shadow"]["blurPt"], 10.0);
    approx(&e["shadow"]["distPt"], 3.0);
    approx(&e["shadow"]["alpha"], 0.4);
    assert_eq!(e["shadow"]["color"], "000000");
    assert!(e.get("glow").is_none());
    assert!(e.get("innerShadow").is_some() && e.get("reflection").is_some() && e.get("bevel").is_some());
    set(&mut doc, json!({"effects": {"shadow": {"alpha": 0.9}, "soft_edge_pt": null, "bevel": "none", "extrusion": "none"}}));
    let e = shape(&doc)["effects"].clone();
    approx(&e["shadow"]["alpha"], 0.9);
    assert_eq!(e["shadow"]["color"], "000000");
    assert!(e.get("softEdgePt").is_none() && e.get("bevel").is_none() && e.get("extrusion").is_none());
    let (_, xml) = saved(&mut doc);
    assert!(!xml.contains("<a:bevelT") && !xml.contains("extrusionH") && xml.contains("prstMaterial"), "{xml}");
    assert!(xml.contains("<a:scene3d>"), "the 3-D rotation stays");
    set(&mut doc, json!({"effects": "none"}));
    assert!(shape(&doc).get("effects").is_none());
    let (_, xml) = saved(&mut doc);
    assert!(!xml.contains("<a:effectLst"), "{xml}");
    // One undo brings the effects back.
    assert!(doc.undo());
    assert!(shape(&doc)["effects"]["shadow"].is_object());
}

#[test]
fn effects_the_theme_gives_are_written_out_before_one_of_them_changes() {
    let style = r#"<wps:style><a:lnRef idx="0"><a:scrgbClr r="0" g="0" b="0"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="2"><a:scrgbClr r="0" g="0" b="0"/></a:effectRef><a:fontRef idx="minor"/></wps:style>"#;
    let mut doc = Sh::new().style(style).open_themed();
    approx(&shape(&doc)["effects"]["shadow"]["distPt"], 1.5);
    // Adding a glow must not lose the theme's shadow, which an own list would otherwise replace.
    set(&mut doc, json!({"effects": {"glow": {"radius_pt": 6, "color": "FF0000"}}}));
    let e = shape(&doc)["effects"].clone();
    approx(&e["shadow"]["blurPt"], 4.5);
    approx(&e["glow"]["radiusPt"], 6.0);
    // "none" on a themed shape writes an empty list so the theme's shadow no longer shows.
    set(&mut doc, json!({"effects": "none"}));
    assert!(shape(&doc).get("effects").is_none());
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains("<a:effectLst/>"), "{xml}");
    assert!(shape(&Document::open(&bytes).unwrap()).get("effects").is_none());
}

#[test]
fn an_extrusion_with_a_front_camera_turns_the_camera_to_the_side() {
    let mut doc = Sh::new().sppr(r#"<a:scene3d><a:camera prst="orthographicFront"/><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:sp3d><a:bevelT w="38100" h="38100"/></a:sp3d>"#).open();
    assert!(shape(&doc)["effects"].get("extrusion").is_none());
    set(&mut doc, json!({"effects": {"extrusion": {"height_pt": 8}}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"prst="obliqueTopRight""#), "{xml}");
    assert!(xml.contains("<a:bevelT"), "the bevel stays");
    assert!(shape(&doc)["effects"]["extrusion"]["dir"].is_array());
}

#[test]
fn effects_are_checked_before_anything_is_changed() {
    let mut doc = Sh::new().open();
    let before = saved(&mut doc).1;
    for bad in [json!({"shadow": {"blur_pt": 500}}), json!({"glow": {"radius_pt": -1}}), json!({"bevel": {"preset": "pointy"}}), json!({"reflection": {"start_alpha": 2}}), json!({"shadow": {"color": "nope"}})] {
        let e = fail(&mut doc, json!({"fill": "FF0000", "effects": bad}));
        assert!(!e.is_empty());
    }
    assert_eq!(saved(&mut doc).1, before);
}

#[test]
fn writes_the_text_frame_settings_to_the_body_and_the_vml_text_box() {
    let fallback = r##"<w:pict><v:rect id="Rectangle 1" o:spid="_x0000_s1026" style="width:100pt;height:50pt;v-text-anchor:top" fillcolor="#4472c4"><v:textbox><w:txbxContent><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:txbxContent></v:textbox></v:rect></w:pict>"##;
    let mut doc = Sh::new().text("Hi").fallback(fallback).open();
    set(&mut doc, json!({"body": {"ins_l": 2, "ins_t": 3, "ins_r": 4, "ins_b": 5, "v_anchor": "bottom", "anchor_center": true, "wrap_text": false, "vert": "vert270", "columns": 2, "column_gap_pt": 12, "auto_fit": "shape", "upright": true}}));
    let (bytes, xml) = saved(&mut doc);
    let bp = &xml[xml.find("<wps:bodyPr").unwrap()..xml.find("</wps:bodyPr>").unwrap()];
    for needle in [r#"lIns="25400""#, r#"tIns="38100""#, r#"rIns="50800""#, r#"bIns="63500""#, r#"anchor="b""#, r#"anchorCtr="1""#, r#"wrap="none""#, r#"vert="vert270""#, r#"numCol="2""#, r#"spcCol="152400""#, r#"upright="1""#, "<a:spAutoFit/>"] {
        assert!(bp.contains(needle), "{needle} missing in {bp}");
    }
    assert!(!bp.contains("noAutofit"));
    let fb = fallback_of(&xml);
    assert!(fb.contains("v-text-anchor:bottom-center") && fb.contains("mso-wrap-style:none"), "{fb}");
    assert!(fb.contains(r#"inset="2pt,3pt,4pt,5pt""#), "{fb}");
    assert!(fb.contains("mso-fit-shape-to-text:t") && fb.contains("layout-flow:vertical") && fb.contains("mso-layout-flow-alt:bottom-to-top"), "{fb}");
    let s = shape(&doc);
    assert_eq!(s["vAnchor"], "bottom");
    assert_eq!(s["anchorCenter"], true);
    assert_eq!(s["vert"], "vert270");
    assert_eq!(s["columns"], 2);
    assert_eq!(s["autoFitMode"], "shape");
    assert_eq!(shape(&Document::open(&bytes).unwrap()), s);
    // Text autofit with a scale.
    set(&mut doc, json!({"body": {"auto_fit": "text", "font_scale": 80, "line_spacing_reduction": 20, "vert": "horz", "columns": 1, "upright": false}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:normAutofit fontScale="80000" lnSpcReduction="20000"/>"#) && !xml.contains("upright"), "{xml}");
    let s = shape(&doc);
    approx(&s["fontScale"], 80.0);
    assert!(s.get("vert").is_none() && s.get("columns").is_none());
    // None puts the explicit "do not autofit" back.
    set(&mut doc, json!({"body": {"auto_fit": "none"}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains("<a:noAutofit/>") && !xml.contains("normAutofit"));
    assert!(shape(&doc).get("autoFitMode").is_none());
}

#[test]
fn v_anchor_alone_still_works_in_a_shape_with_vml() {
    let mut doc = Sh::new().open();
    set(&mut doc, json!({"v_anchor": "middle"}));
    assert_eq!(shape(&doc)["vAnchor"], "middle");
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"anchor="ctr""#) && fallback_of(&xml).contains("v-text-anchor:middle"));
    assert!(fail(&mut doc, json!({"body": {"vert": "sideways"}})).contains("unknown vert"));
    assert!(fail(&mut doc, json!({"body": {"columns": 40}})).contains("columns"));
}

#[test]
fn renames_a_shape() {
    let mut doc = Sh::new().open();
    set(&mut doc, json!({"name": "Title banner"}));
    assert_eq!(shape(&doc)["name"], "Title banner");
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<wp:docPr id="3" name="Title banner"/>"#), "{xml}");
    assert!(fallback_of(&xml).contains(r#"id="Title banner""#));
    assert!(doc.undo());
    assert_eq!(shape(&doc)["name"], "Rectangle 3");
}
