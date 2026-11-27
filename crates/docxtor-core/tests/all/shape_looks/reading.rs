//! Tests of: Reading fills and reading outlines, effects and the text frame and reading vml.

use super::*;

#[test]
fn reads_a_solid_fill_with_alpha_and_a_scheme_colour() {
    let doc = Sh::new().sppr(r#"<a:solidFill><a:srgbClr val="ff4f00"><a:alpha val="40000"/></a:srgbClr></a:solidFill>"#).open();
    let f = &shape(&doc)["fill"];
    assert_eq!(f["kind"], "solid");
    assert_eq!(f["color"], "FF4F00");
    approx(&f["alpha"], 0.4);
    assert!(f.get("stops").is_none(), "a solid fill has no stops");
    let doc = Sh::new().sppr(r#"<a:solidFill><a:schemeClr val="accent1"><a:lumMod val="50000"/></a:schemeClr></a:solidFill>"#).open();
    assert_eq!(shape(&doc)["fill"]["color"].as_str().unwrap().len(), 6);
}

#[test]
fn reads_every_stop_of_a_linear_gradient_with_its_angle_and_flags() {
    let g = r#"<a:gradFill flip="none" rotWithShape="1"><a:gsLst><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs><a:gs pos="0"><a:srgbClr val="FF0000"><a:alpha val="80000"/></a:srgbClr></a:gs><a:gs pos="35000"><a:srgbClr val="00FF00"/></a:gs></a:gsLst><a:lin ang="2700000" scaled="1"/></a:gradFill>"#;
    let doc = Sh::new().sppr(g).open();
    let f = &shape(&doc)["fill"];
    assert_eq!(f["kind"], "gradient");
    // Sorted by position; first and last keep the old fields.
    let stops = f["stops"].as_array().unwrap();
    assert_eq!(stops.len(), 3);
    assert_eq!(stops[0]["color"], "FF0000");
    approx(&stops[0]["alpha"], 0.8);
    approx(&stops[1]["pos"], 35.0);
    assert_eq!(stops[2]["color"], "0000FF");
    assert_eq!(f["color"], "FF0000");
    assert_eq!(f["color2"], "0000FF");
    approx(&f["angle"], 45.0);
    assert_eq!(f["scaled"], true);
    assert_eq!(f["rotWithShape"], true);
    assert!(f.get("path").is_none());
}

#[test]
fn reads_path_gradients_with_their_focus_rectangle() {
    let g = |p: &str| format!(r#"<a:gradFill rotWithShape="0"><a:gsLst><a:gs pos="0"><a:srgbClr val="FFFFFF"/></a:gs><a:gs pos="100000"><a:srgbClr val="000000"/></a:gs></a:gsLst><a:path path="{p}"><a:fillToRect l="25000" t="75000" r="75000" b="25000"/></a:path></a:gradFill>"#);
    for p in ["circle", "rect", "shape"] {
        let doc = Sh::new().sppr(&g(p)).open();
        let f = &shape(&doc)["fill"];
        assert_eq!(f["path"], p);
        assert!(f.get("angle").is_none(), "a path gradient has no angle");
        approx(&f["fillTo"]["l"], 25.0);
        approx(&f["fillTo"]["t"], 75.0);
        approx(&f["fillTo"]["r"], 75.0);
        approx(&f["fillTo"]["b"], 25.0);
        assert_eq!(f["rotWithShape"], false);
    }
}

#[test]
fn reads_picture_fills_stretched_cropped_and_tiled() {
    let stretch = r#"<a:blipFill dpi="0" rotWithShape="1"><a:blip r:embed="rIdImg"><a:alphaModFix amt="60000"/></a:blip><a:srcRect l="10000" t="5000" r="0" b="20000"/><a:stretch><a:fillRect l="-5000" t="0" r="10000" b="0"/></a:stretch></a:blipFill>"#;
    let doc = Sh::new().sppr(stretch).open();
    let f = &shape(&doc)["fill"];
    assert_eq!(f["kind"], "picture");
    assert_eq!(f["color"], "FFFFFF");
    assert_eq!(f["src"], "word/media/img1.png");
    approx(&f["alpha"], 0.6);
    approx(&f["crop"]["l"], 10.0);
    approx(&f["crop"]["b"], 20.0);
    approx(&f["stretch"]["l"], -5.0);
    approx(&f["stretch"]["r"], 10.0);
    assert!(f.get("tile").is_none());

    let tile = r#"<a:blipFill><a:blip r:embed="rIdImg"/><a:tile tx="127000" ty="63500" sx="50000" sy="75000" flip="xy" algn="ctr"/></a:blipFill>"#;
    let doc = Sh::new().sppr(tile).open();
    let f = &shape(&doc)["fill"];
    let t = &f["tile"];
    approx(&t["tx"], 10.0);
    approx(&t["ty"], 5.0);
    approx(&t["sx"], 50.0);
    approx(&t["sy"], 75.0);
    assert_eq!(t["align"], "ctr");
    assert_eq!(t["flip"], "xy");
    assert!(f.get("stretch").is_none());
    // A picture that isn't in the package gives no fill rather than a broken one.
    let doc = Sh::new().sppr(r#"<a:blipFill><a:blip r:embed="rIdMissing"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>"#).open();
    assert!(shape(&doc).get("fill").is_none());
}

#[test]
fn reads_pattern_fills_with_foreground_and_background() {
    let doc = Sh::new().sppr(r#"<a:pattFill prst="ltDnDiag"><a:fgClr><a:srgbClr val="C00000"/></a:fgClr><a:bgClr><a:srgbClr val="FFFFFF"/></a:bgClr></a:pattFill>"#).open();
    let f = &shape(&doc)["fill"];
    assert_eq!(f["kind"], "pattern");
    assert_eq!(f["pattern"], "ltDnDiag");
    assert_eq!(f["color"], "C00000");
    assert_eq!(f["color2"], "FFFFFF");
}

#[test]
fn a_theme_fill_style_gives_a_gradient_and_an_effect_style_a_shadow() {
    let style = r#"<wps:style><a:lnRef idx="0"><a:scrgbClr r="0" g="0" b="0"/></a:lnRef><a:fillRef idx="2"><a:schemeClr val="accent2"/></a:fillRef><a:effectRef idx="2"><a:scrgbClr r="0" g="0" b="0"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></wps:style>"#;
    let doc = Sh::new().style(style).open_themed();
    let s = shape(&doc);
    assert_eq!(s["fill"]["kind"], "gradient");
    assert_eq!(s["fill"]["stops"].as_array().unwrap().len(), 3);
    approx(&s["fill"]["angle"], 90.0);
    let sh = &s["effects"]["shadow"];
    approx(&sh["blurPt"], 4.5);
    approx(&sh["distPt"], 1.5);
    approx(&sh["angle"], 90.0);
    approx(&sh["alpha"], 0.63);
    // The shape's own list wins over the theme's, even when it is empty.
    let doc = Sh::new().style(style).sppr("<a:effectLst/>").open_themed();
    assert!(shape(&doc).get("effects").is_none());
    // Index 1 of this theme is an empty effect style.
    let doc = Sh::new().style(&style.replace(r#"effectRef idx="2""#, r#"effectRef idx="1""#)).open_themed();
    assert!(shape(&doc).get("effects").is_none());
}

#[test]
fn a_shape_without_a_theme_keeps_a_solid_fill_from_its_style() {
    let style = r#"<wps:style><a:lnRef idx="0"><a:scrgbClr r="0" g="0" b="0"/></a:lnRef><a:fillRef idx="3"><a:schemeClr val="accent6"/></a:fillRef><a:effectRef idx="0"><a:scrgbClr r="0" g="0" b="0"/></a:effectRef><a:fontRef idx="minor"/></wps:style>"#;
    let doc = Sh::new().style(style).open();
    assert_eq!(shape(&doc)["fill"]["kind"], "solid");
    assert_eq!(shape(&doc)["fill"]["color"], "70AD47");
}

#[test]
fn reads_line_alpha_compound_cap_join_miter_and_arrowhead_sizes() {
    let ln = r#"<a:ln w="38100" cap="rnd" cmpd="thickThin"><a:solidFill><a:srgbClr val="112233"><a:alpha val="50000"/></a:srgbClr></a:solidFill><a:prstDash val="dash"/><a:miter lim="400000"/><a:headEnd type="triangle" w="lg" len="sm"/><a:tailEnd type="oval" w="sm" len="lg"/></a:ln>"#;
    let doc = Sh::new().sppr(ln).open();
    let l = &shape(&doc)["line"];
    assert_eq!(l["color"], "112233");
    approx(&l["alpha"], 0.5);
    approx(&l["widthPt"], 3.0);
    assert_eq!(l["cap"], "round");
    assert_eq!(l["compound"], "thickThin");
    approx(&l["miter"], 400.0);
    assert_eq!(l["headArrow"], "triangle");
    assert_eq!(l["headW"], "lg");
    assert_eq!(l["headLen"], "sm");
    assert_eq!(l["tailArrow"], "oval");
    assert_eq!(l["tailW"], "sm");
    assert_eq!(l["tailLen"], "lg");
    // The defaults are not reported.
    let doc = Sh::new().sppr(r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:miter lim="800000"/><a:headEnd type="triangle" w="med" len="med"/></a:ln>"#).open();
    let l = &shape(&doc)["line"];
    for k in ["compound", "miter", "headW", "headLen", "cap", "join"] {
        assert!(l.get(k).is_none(), "{k} should be absent: {l}");
    }
}

pub(super) const EFFECTS: &str = r#"<a:effectLst><a:outerShdw blurRad="63500" dist="38100" dir="2700000" sx="90000" sy="80000" algn="tl" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="40000"/></a:srgbClr></a:outerShdw><a:innerShdw blurRad="76200" dist="50800" dir="13500000"><a:srgbClr val="333333"><a:alpha val="60000"/></a:srgbClr></a:innerShdw><a:glow rad="101600"><a:srgbClr val="FFC000"><a:alpha val="35000"/></a:srgbClr></a:glow><a:softEdge rad="63500"/><a:reflection blurRad="6350" stA="50000" stPos="0" endA="300" endPos="55000" dist="25400" dir="5400000" fadeDir="5400000" sy="-100000" algn="bl" rotWithShape="0"/></a:effectLst><a:scene3d><a:camera prst="obliqueTopRight"><a:rot lat="0" lon="0" rev="0"/></a:camera><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:sp3d extrusionH="127000" prstMaterial="matte"><a:bevelT w="63500" h="38100" prst="relaxedInset"/><a:extrusionClr><a:srgbClr val="808080"/></a:extrusionClr></a:sp3d>"#;

#[test]
fn reads_every_effect() {
    let doc = Sh::new().sppr(EFFECTS).open();
    let e = &shape(&doc)["effects"];
    let s = &e["shadow"];
    assert_eq!(s["color"], "000000");
    approx(&s["alpha"], 0.4);
    approx(&s["blurPt"], 5.0);
    approx(&s["distPt"], 3.0);
    approx(&s["angle"], 45.0);
    approx(&s["sx"], 90.0);
    approx(&s["sy"], 80.0);
    assert_eq!(s["align"], "tl");
    assert_eq!(s["rotWithShape"], false);
    assert_eq!(e["reflection"]["rotWithShape"], false);
    let i = &e["innerShadow"];
    assert_eq!(i["color"], "333333");
    approx(&i["blurPt"], 6.0);
    approx(&i["distPt"], 4.0);
    approx(&i["angle"], 225.0);
    let g = &e["glow"];
    assert_eq!(g["color"], "FFC000");
    approx(&g["radiusPt"], 8.0);
    approx(&g["alpha"], 0.35);
    approx(&e["softEdgePt"], 5.0);
    let r = &e["reflection"];
    approx(&r["blurPt"], 0.5);
    approx(&r["startAlpha"], 0.5);
    approx(&r["endAlpha"], 0.003);
    approx(&r["distPt"], 2.0);
    approx(&r["direction"], 90.0);
    approx(&r["fadeDirection"], 90.0);
    approx(&r["sy"], -100.0);
    approx(&r["endPos"], 55.0);
    let b = &e["bevel"];
    assert_eq!(b["preset"], "relaxedInset");
    approx(&b["wPt"], 5.0);
    approx(&b["hPt"], 3.0);
    let x = &e["extrusion"];
    approx(&x["heightPt"], 10.0);
    assert_eq!(x["color"], "808080");
    // obliqueTopRight: depth goes up and to the right.
    let dir = x["dir"].as_array().unwrap();
    assert!(dir[0].as_f64().unwrap() > 0.0 && dir[1].as_f64().unwrap() < 0.0, "{dir:?}");
}

#[test]
fn a_front_camera_shows_no_extrusion_direction() {
    let doc = Sh::new().sppr(r#"<a:scene3d><a:camera prst="orthographicFront"/><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:sp3d extrusionH="63500"/>"#).open();
    let x = &shape(&doc)["effects"]["extrusion"];
    approx(&x["heightPt"], 5.0);
    assert!(x.get("dir").is_none());
}

#[test]
fn reads_the_text_frame() {
    let body = r#"<wps:bodyPr rot="0" vert="vert270" wrap="none" lIns="0" tIns="25400" rIns="127000" bIns="12700" numCol="3" spcCol="228600" anchor="b" anchorCtr="1" upright="1"><a:normAutofit fontScale="92500" lnSpcReduction="10000"/></wps:bodyPr>"#;
    let doc = Sh::new().body(body).text("Hi").open();
    let s = shape(&doc);
    assert_eq!(s["vert"], "vert270");
    assert_eq!(s["wrapText"], false);
    assert_eq!(s["vAnchor"], "bottom");
    assert_eq!(s["anchorCenter"], true);
    assert_eq!(s["upright"], true);
    assert_eq!(s["columns"], 3);
    approx(&s["columnGapPt"], 18.0);
    assert_eq!(s["autoFitMode"], "text");
    approx(&s["fontScale"], 92.5);
    approx(&s["lineSpacingReduction"], 10.0);
    assert!(s.get("autoFit").is_none());
    approx(&s["insets"]["r"], 10.0);
    let doc = Sh::new().body(r#"<wps:bodyPr vert="wordArtVert"><a:spAutoFit/></wps:bodyPr>"#).open();
    let s = shape(&doc);
    assert_eq!(s["vert"], "wordArtVert");
    assert_eq!(s["autoFit"], true);
    assert_eq!(s["autoFitMode"], "shape");
    assert!(s.get("columns").is_none());
}

#[test]
fn reads_vml_fills_strokes_and_text_frame() {
    let vml = |inner: &str, attrs: &str| format!(r##"<w:pict><v:rect id="Box" o:spid="_x0000_s1026" style="position:absolute;width:100pt;height:50pt;v-text-anchor:middle-center" {attrs}>{inner}<v:textbox style="mso-fit-shape-to-text:t;layout-flow:vertical;mso-layout-flow-alt:bottom-to-top" inset="1pt,2pt,3pt,4pt"><w:txbxContent><w:p><w:r><w:t>x</w:t></w:r></w:p></w:txbxContent></v:textbox></v:rect></w:pict>"##);
    let build = |inner: &str, attrs: &str| {
        let xml = format!("<w:p><w:r>{}</w:r></w:p>", vml(inner, attrs));
        Document::open(&package_with(&xml, false, true)).unwrap()
    };
    // Gradient with a colour list, an angle and an opacity at each end.
    let doc = build(r##"<v:fill type="gradient" color2="#0000ff" colors="0 #ff0000;.5 #00ff00;1 #0000ff" angle="180" opacity=".8" o:opacity2=".4"/>"##, r##"fillcolor="#ff0000""##);
    let s = serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap();
    let f = &s["fill"];
    assert_eq!(f["kind"], "gradient");
    assert_eq!(f["stops"].as_array().unwrap().len(), 3);
    approx(&f["stops"][1]["pos"], 50.0);
    approx(&f["stops"][0]["alpha"], 0.8);
    approx(&f["stops"][2]["alpha"], 0.4);
    assert_eq!(s["vAnchor"], "middle");
    assert_eq!(s["anchorCenter"], true);
    assert_eq!(s["vert"], "vert270");
    assert_eq!(s["autoFit"], true);
    approx(&s["insets"]["t"], 2.0);
    // A radial gradient is a path gradient.
    let doc = build(r##"<v:fill type="gradientRadial" color2="#0000ff" focusposition=".25,.75"/>"##, r##"fillcolor="#ffffff""##);
    let f = &serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap()["fill"];
    assert_eq!(f["path"], "circle");
    approx(&f["fillTo"]["l"], 25.0);
    approx(&f["fillTo"]["t"], 75.0);
    // A tiled picture and a stretched one.
    let doc = build(r#"<v:fill r:id="rIdImg" o:title="pic" type="tile"/>"#, "");
    let f = &serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap()["fill"];
    assert_eq!(f["kind"], "picture");
    assert_eq!(f["src"], "word/media/img1.png");
    assert!(f.get("tile").is_some());
    let doc = build(r#"<v:fill r:id="rIdImg" type="frame"/>"#, "");
    let f = &serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap()["fill"];
    assert!(f.get("tile").is_none() && f.get("stretch").is_some());
    // A pattern is a bitmap in VML: the colour stands for it.
    let doc = build(r##"<v:fill type="pattern" color="#112233" color2="#ffffff"/>"##, "");
    let f = &serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap()["fill"];
    assert_eq!(f["kind"], "solid");
    // A shadow.
    let doc = build(r##"<v:shadow on="t" color="#222222" opacity=".5" offset="3pt,3pt"/>"##, "");
    let e = &serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap()["effects"]["shadow"];
    assert_eq!(e["color"], "222222");
    approx(&e["alpha"], 0.5);
    approx(&e["angle"], 45.0);
    approx(&e["distPt"], (18.0f64).sqrt());
    // Stroke details.
    let doc = build(r##"<v:stroke linestyle="thinThin" endcap="round" joinstyle="bevel" miterlimit="4" opacity=".5" startarrow="block" startarrowwidth="wide" startarrowlength="short"/>"##, r##"strokecolor="#00ff00""##);
    let l = &serde_json::to_value(doc.shape_at(Pos::new(first_para(&doc), 0)).unwrap()).unwrap()["line"];
    assert_eq!(l["compound"], "dbl");
    assert_eq!(l["cap"], "round");
    assert_eq!(l["join"], "bevel");
    approx(&l["miter"], 400.0);
    approx(&l["alpha"], 0.5);
    assert_eq!(l["headW"], "lg");
    assert_eq!(l["headLen"], "sm");
}
