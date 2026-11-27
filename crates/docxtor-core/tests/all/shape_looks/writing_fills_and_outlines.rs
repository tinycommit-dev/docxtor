//! Tests of: Writing fills and writing outlines.

use super::*;

#[test]
fn a_bare_hex_fill_still_works_and_alpha_is_written() {
    let mut doc = Sh::new().sppr(r#"<a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>"#).open();
    set(&mut doc, json!({"fill": "00ff00"}));
    assert_eq!(shape(&doc)["fill"]["color"], "00FF00".to_string().to_ascii_uppercase());
    set(&mut doc, json!({"fill": {"kind": "solid", "color": "336699", "alpha": 0.25}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:solidFill><a:srgbClr val="336699"><a:alpha val="25000"/></a:srgbClr></a:solidFill>"#), "{xml}");
    // The fallback has the colour and the opacity.
    let fb = fallback_of(&xml);
    assert!(fb.contains(r##"fillcolor="#336699""##) && fb.contains(r#"opacity=".25""#), "{fb}");
    let f = shape(&doc)["fill"].clone();
    approx(&f["alpha"], 0.25);
    set(&mut doc, json!({"fill": "none"}));
    assert!(shape(&doc).get("fill").is_none());
}

#[test]
fn writes_a_gradient_with_all_its_stops_and_keeps_the_vml_in_step() {
    let mut doc = Sh::new().sppr(r#"<a:solidFill><a:srgbClr val="FF0000"/></a:solidFill>"#).open();
    let before = saved(&mut doc).1;
    set(
        &mut doc,
        json!({"fill": {"kind": "gradient", "angle": 45, "scaled": true, "stops": [{"pos": 100, "color": "0000FF", "alpha": 0.5}, {"pos": 0, "color": "FF0000"}, {"pos": 40, "color": "FFFF00"}]}}),
    );
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="40000"><a:srgbClr val="FFFF00"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"><a:alpha val="50000"/></a:srgbClr></a:gs>"#), "{xml}");
    assert!(xml.contains(r#"<a:lin ang="2700000" scaled="1"/>"#));
    // Placed in the order DrawingML wants: after the geometry, before the outline.
    assert!(xml.find("<a:prstGeom").unwrap() < xml.find("<a:gradFill").unwrap());
    let fb = fallback_of(&xml);
    assert!(fb.contains(r#"type="gradient""#) && fb.contains(r##"fillcolor="#FF0000""##) && fb.contains(r##"color2="#0000FF""##), "{fb}");
    assert!(fb.contains(r##"colors="0 #FF0000;.4 #FFFF00;1 #0000FF""##), "{fb}");
    // Read back from the saved file: the same fill.
    let want = shape(&doc)["fill"].clone();
    assert_eq!(want["stops"].as_array().unwrap().len(), 3);
    approx(&want["angle"], 45.0);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(shape(&again)["fill"], want);
    // Undo.
    assert!(doc.undo());
    assert_eq!(saved(&mut doc).1, before);
}

#[test]
fn writes_path_gradients() {
    let mut doc = Sh::new().open();
    set(&mut doc, json!({"fill": {"kind": "gradient", "path": "circle", "fill_to": {"l": 30, "t": 40, "r": 70, "b": 60}, "rot_with_shape": false, "stops": [{"pos": 0, "color": "FFFFFF"}, {"pos": 100, "color": "000000"}]}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:gradFill rotWithShape="0">"#), "{xml}");
    assert!(xml.contains(r#"<a:path path="circle"><a:fillToRect l="30000" t="40000" r="70000" b="60000"/></a:path>"#), "{xml}");
    assert!(fallback_of(&xml).contains(r#"type="gradientRadial""#));
    let f = &shape(&doc)["fill"];
    assert_eq!(f["path"], "circle");
    assert_eq!(f["rotWithShape"], false);
}

#[test]
fn writes_pattern_fills_for_every_preset() {
    let presets = docxtor_core::shapes::fill::PATTERNS;
    assert_eq!(presets.len(), 54);
    let mut doc = Sh::new().open();
    for p in presets {
        set(&mut doc, json!({"fill": {"kind": "pattern", "pattern": p, "fg": "C00000", "bg": "FFE699"}}));
        let f = &shape(&doc)["fill"];
        assert_eq!(f["pattern"], *p);
        assert_eq!(f["color"], "C00000");
        assert_eq!(f["color2"], "FFE699");
    }
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:pattFill prst="zigZag"><a:fgClr><a:srgbClr val="C00000"/></a:fgClr><a:bgClr><a:srgbClr val="FFE699"/></a:bgClr></a:pattFill>"#));
    // VML keeps a bitmap for a pattern: the foreground colour stands for it.
    assert!(fallback_of(&xml).contains(r##"fillcolor="#C00000""##));
    assert!(fail(&mut doc, json!({"fill": {"kind": "pattern", "pattern": "nope", "fg": "000000", "bg": "FFFFFF"}})).contains("unknown pattern"));
}

#[test]
fn writes_picture_fills_with_a_relationship_stretched_cropped_or_tiled() {
    let mut doc = Sh::new().open();
    let media = doc.add_media("png", PNG.to_vec());
    set(&mut doc, json!({"fill": {"kind": "picture", "media": media, "crop": {"l": 10, "t": 0, "r": 0, "b": 20}, "stretch": {"l": 5, "t": 5, "r": 5, "b": 5}, "alpha": 0.5}}));
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:blipFill dpi="0" rotWithShape="1"><a:blip r:embed="rId"#), "{xml}");
    assert!(xml.contains(r#"<a:alphaModFix amt="50000"/></a:blip><a:srcRect l="10000" t="0" r="0" b="20000"/><a:stretch><a:fillRect l="5000" t="5000" r="5000" b="5000"/></a:stretch>"#), "{xml}");
    let rels = part(&bytes, "word/_rels/document.xml.rels");
    assert!(rels.contains(&media.replace("word/", "")), "{rels}");
    let fb = fallback_of(&xml);
    assert!(fb.contains(r#"type="frame""#) && fb.contains("r:id=\"rId"), "{fb}");
    let f = shape(&doc)["fill"].clone();
    assert_eq!(f["kind"], "picture");
    assert_eq!(f["src"], media);
    approx(&f["alpha"], 0.5);
    approx(&f["stretch"]["l"], 5.0);
    assert_eq!(shape(&Document::open(&bytes).unwrap())["fill"], f);
    // Tiled.
    set(&mut doc, json!({"fill": {"kind": "picture", "media": media, "tile": {"tx": 3, "ty": 4, "sx": 50, "sy": 60, "align": "ctr", "flip": "x"}}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:tile tx="38100" ty="50800" sx="50000" sy="60000" flip="x" algn="ctr"/>"#), "{xml}");
    assert!(fallback_of(&xml).contains(r#"type="tile""#));
    let f = &shape(&doc)["fill"];
    approx(&f["tile"]["sx"], 50.0);
    assert!(f.get("stretch").is_none());
    // Undo twice goes back to the plain fill.
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(shape(&doc).get("fill").is_none() || shape(&doc)["fill"]["kind"] == "solid");
    assert!(fail(&mut doc, json!({"fill": {"kind": "picture", "media": "word/media/nothing.png"}})).contains("add_media"));
}

#[test]
fn a_fill_replaces_the_old_fill_and_leaves_the_rest_of_the_shape_alone() {
    let sppr = r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:lin ang="0"/></a:gradFill><a:ln w="9525"><a:solidFill><a:schemeClr val="accent1"><a:lumMod val="50000"/></a:schemeClr></a:solidFill></a:ln><a:extLst><a:ext uri="{91240B29-F687-4F45-9708-019B960494DF}"><a14:hiddenLine xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main"/></a:ext></a:extLst>"#;
    let mut doc = Sh::new().sppr(sppr).open();
    set(&mut doc, json!({"fill": "112233"}));
    let (_, xml) = saved(&mut doc);
    assert!(!xml.contains("a:gradFill"));
    assert!(xml.contains(r#"<a:schemeClr val="accent1"><a:lumMod val="50000"/></a:schemeClr>"#), "the outline stays: {xml}");
    assert!(xml.contains("a14:hiddenLine"), "the extension list stays");
    assert!(xml.find("a:solidFill><a:srgbClr val=\"112233\"").unwrap() < xml.find("<a:ln ").unwrap());
}

#[test]
fn writes_line_alpha_compound_cap_join_miter_and_arrowhead_sizes() {
    let ln = r#"<a:ln w="25400"><a:solidFill><a:schemeClr val="accent1"><a:lumMod val="75000"/></a:schemeClr></a:solidFill><a:miter lim="800000"/><a:tailEnd type="triangle"/></a:ln>"#;
    let mut doc = Sh::new().sppr(ln).open();
    // Only the opacity changes: the theme colour stays as it was.
    set(&mut doc, json!({"line": {"alpha": 0.5}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:schemeClr val="accent1"><a:lumMod val="75000"/><a:alpha val="50000"/></a:schemeClr>"#), "{xml}");
    set(&mut doc, json!({"line": {"compound": "dbl", "cap": "round", "join": "bevel", "tail_w": "lg", "tail_len": "sm", "head_arrow": "oval", "head_w": "sm"}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:ln w="25400" cap="rnd" cmpd="dbl">"#), "{xml}");
    assert!(xml.contains("<a:bevel/>") && !xml.contains("<a:miter"), "{xml}");
    assert!(xml.contains(r#"<a:tailEnd type="triangle" w="lg" len="sm"/>"#), "{xml}");
    assert!(xml.contains(r#"<a:headEnd type="oval" w="sm"/>"#), "{xml}");
    let l = shape(&doc)["line"].clone();
    assert_eq!(l["compound"], "dbl");
    assert_eq!(l["join"], "bevel");
    assert_eq!(l["cap"], "round");
    assert_eq!(l["tailW"], "lg");
    approx(&l["alpha"], 0.5);
    // The fallback says what VML can.
    let fb = fallback_of(&xml);
    assert!(fb.contains(r#"linestyle="thinThin""#) && fb.contains(r#"endcap="round""#) && fb.contains(r#"joinstyle="bevel""#) && fb.contains(r#"opacity="0.5""#) && fb.contains(r#"endarrowwidth="wide""#), "{fb}");
    // Back to a single flat line with a miter limit.
    set(&mut doc, json!({"line": {"compound": "single", "cap": "flat", "join": "miter", "miter": 300}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"cap="flat""#) && !xml.contains("cmpd=") && xml.contains(r#"<a:miter lim="300000"/>"#), "{xml}");
    assert!(fail(&mut doc, json!({"line": {"cap": "pointy"}})).contains("unknown cap"));
    assert!(fail(&mut doc, json!({"line": {"alpha": 2}})).contains("alpha"));
}

#[test]
fn a_new_shape_takes_the_extra_line_properties_too() {
    let mut doc = Document::new_blank();
    let p = doc.render_snapshot().order[0];
    ops(&mut doc, json!({"op": "insert_shape", "at": {"p": p, "o": 0}, "geometry": "line", "w": 100, "h": 0, "line": {"color": "FF0000", "width_pt": 2, "alpha": 0.5, "compound": "tri", "tail_arrow": "triangle", "tail_w": "lg", "tail_len": "lg", "join": "round"}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"cmpd="tri""#) && xml.contains("<a:round/>") && xml.contains(r#"<a:tailEnd type="triangle" w="lg" len="lg"/>"#), "{xml}");
    assert!(xml.contains(r#"<a:alpha val="50000"/>"#), "{xml}");
}
