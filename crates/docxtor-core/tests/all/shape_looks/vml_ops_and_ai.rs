//! Tests of: VML-only shapes and members of groups and ops as json and nothing else changes and the ai tools.

use super::*;

const VML_ONLY: &str = r##"<w:p><w:r><w:pict><v:rect id="Old box" o:spid="_x0000_s1026" style="position:absolute;margin-left:10pt;margin-top:10pt;width:100pt;height:50pt;z-index:1" fillcolor="#ffffcc" strokecolor="red"><v:textbox><w:txbxContent><w:p><w:r><w:t>Old</w:t></w:r></w:p></w:txbxContent></v:textbox><w10:wrap type="square"/></v:rect></w:pict></w:r></w:p>"##;

#[test]
fn a_vml_only_shape_gets_gradient_pattern_effects_free_changes_in_place() {
    let mut doc = Document::open(&package_with(VML_ONLY, false, true)).unwrap();
    set(&mut doc, json!({"fill": {"kind": "gradient", "angle": 90, "stops": [{"pos": 0, "color": "FF0000"}, {"pos": 100, "color": "0000FF"}]}, "line": {"alpha": 0.5, "compound": "dbl"}, "body": {"ins_l": 1, "ins_t": 1, "ins_r": 1, "ins_b": 1, "auto_fit": "shape"}}));
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"type="gradient""#) && xml.contains(r##"color2="#0000FF""##), "{xml}");
    assert!(xml.contains(r##"fillcolor="#FF0000""##) && xml.contains(r#"linestyle="thinThin""#) && xml.contains(r#"inset="1pt,1pt,1pt,1pt""#) && xml.contains("mso-fit-shape-to-text:t"), "{xml}");
    assert!(xml.find("<v:fill").unwrap() < xml.find("<v:textbox").unwrap());
    let s = shape(&doc);
    assert_eq!(s["fill"]["kind"], "gradient");
    approx(&s["line"]["alpha"], 0.5);
    assert_eq!(shape(&Document::open(&bytes).unwrap())["fill"], s["fill"]);
    // VML has a shadow and no other effect.
    assert!(fail(&mut doc, json!({"effects": {"glow": {"radius_pt": 3}}})).contains("VML"));
    set(&mut doc, json!({"effects": {"shadow": {"dist_pt": 4, "angle": 90, "color": "333333", "alpha": 0.5}}}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r##"<v:shadow on="t" color="#333333" opacity="0.5" offset="0pt,4pt"/>"##), "{xml}");
    set(&mut doc, json!({"effects": "none"}));
    let (_, xml) = saved(&mut doc);
    assert!(!xml.contains("v:shadow"));
}

const GROUP_PIC: &str = r##"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3048000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="5" name="Group 5"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3048000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="3048000" cy="1270000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:cNvPr id="6" name="Left box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="7" name="Pic"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdImg"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="1778000" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:group id="Group 5" o:spid="_x0000_s1030" style="width:240pt;height:100pt" coordsize="4800,2000"><v:rect id="Left box" style="position:absolute;width:2000;height:2000" fillcolor="red"/></v:group></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"##;

#[test]
fn members_of_a_group_take_fills_effects_and_flips() {
    let mut doc = Document::open(&package_with(GROUP_PIC, false, true)).unwrap();
    let g = |doc: &Document| shape(doc);
    set(&mut doc, json!({"member": [0], "fill": {"kind": "gradient", "angle": 0, "stops": [{"pos": 0, "color": "FFFFFF"}, {"pos": 100, "color": "000000"}]}, "effects": {"shadow": {"dist_pt": 2}}, "body": {"v_anchor": "middle", "columns": 2}, "name": "Left"}));
    let s = g(&doc);
    let m = &s["children"][0];
    assert_eq!(m["fill"]["kind"], "gradient");
    approx(&m["effects"]["shadow"]["distPt"], 2.0);
    assert_eq!(m["vAnchor"], "middle");
    assert_eq!(m["columns"], 2);
    assert_eq!(m["name"], "Left");
    // The picture member flips (a flip is on its transform), and flips back.
    set(&mut doc, json!({"member": [1], "flip_h": true}));
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:xfrm flipH="1"><a:off x="1778000" y="0"/>"#), "{xml}");
    assert_eq!(g(&doc)["children"][1]["flipH"], true);
    assert_eq!(g(&Document::open(&bytes).unwrap())["children"][1]["flipH"], true);
    assert!(doc.undo());
    assert!(g(&doc)["children"][1].get("flipH").is_none());
    // A member that is not a shape can't have a fill.
    assert!(fail(&mut doc, json!({"member": [1], "fill": "FF0000"})).contains("only a shape"));
}

const FLIPPED_LINE: &str = r##"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="2540000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="8" name="Straight Arrow Connector 8"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvCnPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="635000"/></a:xfrm><a:prstGeom prst="straightConnector1"><a:avLst/></a:prstGeom><a:ln w="19050"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:tailEnd type="triangle"/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:line id="Straight Arrow Connector 8" o:spid="_x0000_s1029" style="position:absolute" from="0pt,0pt" to="200pt,50pt" strokecolor="black" strokeweight="1.5pt"><v:stroke endarrow="block"/></v:line></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"##;

#[test]
fn flipping_a_connector_flips_the_drawing_and_the_vml_line_and_undoes() {
    let mut doc = Document::open(&package_with(FLIPPED_LINE, false, false)).unwrap();
    let before = saved(&mut doc).1;
    set(&mut doc, json!({"flip_h": true}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"<a:xfrm flipH="1">"#), "{xml}");
    // The arrow stays on the end of the line: the line's two ends swap sides in the fallback.
    let fb = fallback_of(&xml);
    assert!(fb.contains(r#"from="200pt,0pt" to="0pt,50pt""#) && fb.contains("endarrow=\"block\""), "{fb}");
    assert_eq!(shape(&doc)["flipH"], true);
    set(&mut doc, json!({"flip_v": true}));
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(r#"flipV="1""#) && fallback_of(&xml).contains(r#"from="200pt,50pt" to="0pt,0pt""#), "{xml}");
    set(&mut doc, json!({"flip_h": false, "flip_v": false}));
    let (_, xml) = saved(&mut doc);
    assert!(fallback_of(&xml).contains(r#"from="0pt,0pt" to="200pt,50pt""#));
    assert!(shape(&doc).get("flipH").is_none());
    assert!(doc.undo() && doc.undo() && doc.undo());
    assert_eq!(saved(&mut doc).1, before);
}

#[test]
fn flipping_a_shape_with_text_sets_the_flag_and_leaves_the_text_alone() {
    let mut doc = Sh::new().text("Hello").open();
    set(&mut doc, json!({"flip_h": true, "flip_v": true}));
    let s = shape(&doc);
    assert_eq!(s["flipH"], true);
    assert_eq!(s["flipV"], true);
    assert_eq!(s["text"][0]["segs"][0]["x"], "Hello");
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains("<w:t>Hello</w:t>"));
    assert!(fallback_of(&xml).contains("flip:x y"), "{}", fallback_of(&xml));
}

#[test]
fn the_new_op_inputs_round_trip_through_json() {
    use docxtor_core::ops::Op;
    let j = json!({"op": "set_shape", "at": {"p": 1, "o": 0}, "fill": {"kind": "pattern", "pattern": "pct50", "fg": "000000", "bg": "FFFFFF"}, "line": {"color": "FF0000", "compound": "dbl", "head_w": "lg"}, "effects": {"shadow": {"blur_pt": 3}, "glow": "none", "soft_edge_pt": null}, "body": {"columns": 2, "auto_fit": "text"}, "name": "X"});
    let op: Op = serde_json::from_value(j).unwrap();
    let back = serde_json::to_value(&op).unwrap();
    let again: Op = serde_json::from_value(back.clone()).unwrap();
    assert_eq!(serde_json::to_value(&again).unwrap(), back);
    assert_eq!(back["fill"]["kind"], "pattern");
    assert_eq!(back["effects"]["glow"], "none");
    assert_eq!(back["effects"]["soft_edge_pt"], "none");
    let none: Op = serde_json::from_value(json!({"op": "set_shape", "at": {"p": 1, "o": 0}, "effects": "none", "fill": "none"})).unwrap();
    assert_eq!(serde_json::to_value(&none).unwrap()["effects"], "none");
    let plain: Op = serde_json::from_value(json!({"op": "set_shape", "at": {"p": 1, "o": 0}, "fill": "FF0000"})).unwrap();
    assert_eq!(serde_json::to_value(&plain).unwrap()["fill"], "FF0000");
}

#[test]
fn a_shape_with_looks_round_trips_byte_for_byte_when_nothing_is_set() {
    let sh = Sh::new().sppr(&format!(r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:path path="rect"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path></a:gradFill>{EFFECTS}"#));
    let mut doc = sh.open();
    let (_, xml) = saved(&mut doc);
    assert!(xml.contains(&sh.sppr));
    assert!(xml.contains(&sh.body_pr));
}

#[test]
fn the_ai_reads_a_shapes_look_and_sets_it_through_apply_ops() {
    use docxtor_core::ai::call_tool;
    let mut doc = Document::new_blank();
    let p = doc.render_snapshot().order[0];
    call_tool(&mut doc, "insert_shape", &json!({"id": p, "shape": "rect", "fill": "FFE699", "floating": false})).unwrap();
    let plain = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(plain.contains("[shape: rect]"), "{plain}");
    let ops = json!({"ops": [{"op": "set_shape", "at": {"p": p, "o": 0}, "fill": {"kind": "gradient", "stops": [{"pos": 0, "color": "FFFFFF"}, {"pos": 100, "color": "4472C4"}]}, "effects": {"shadow": {"blur_pt": 4}, "glow": {"radius_pt": 6}}}]});
    call_tool(&mut doc, "apply_ops", &ops).unwrap();
    let out = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(out.contains("[shape: rect (gradient fill, shadow, glow)]"), "{out}");
    let defs = docxtor_core::ai::tool_definitions().to_string();
    assert!(defs.contains("compound") && defs.contains("effects?"));
}

#[test]
fn a_picture_fill_in_a_header_is_related_from_the_header() {
    let mut blank = Document::new_blank();
    ops(&mut blank, json!({"op": "add_header_footer", "kind": "header"}));
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let hdr = String::from_utf8(pkg.get("word/header1.xml").unwrap().to_vec()).unwrap();
    let run = Sh::new().xml();
    let runs = &run[run.find("<w:r>").unwrap()..run.rfind("</w:r>").unwrap() + "</w:r>".len()];
    let hdr = hdr.replacen("<w:hdr ", &format!("<w:hdr {NS} "), 1).replacen("</w:pPr></w:p>", &format!("</w:pPr>{runs}</w:p>"), 1);
    assert!(hdr.contains("Rectangle 3"), "{hdr}");
    pkg.set("word/header1.xml", hdr.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let media = doc.add_media("png", PNG.to_vec());
    let host = {
        let ps = doc.page_setup();
        let docxtor_core::render::RBlock::Para(hp) = &ps.headers["default"].blocks[0] else { panic!() };
        (hp.id, hp.segs.iter().find(|s| s.shape.is_some()).unwrap().o)
    };
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": host.0, "o": host.1}, "fill": {"kind": "picture", "media": media}}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let header = part(&out, "word/header1.xml");
    let rels = part(&out, "word/_rels/header1.xml.rels");
    assert!(header.contains("<a:blipFill") && rels.contains(&media.replace("word/", "")), "{header}\n{rels}");
    let rid = header.split("r:embed=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    assert!(rels.contains(&format!(r#"Id="{rid}""#)), "the relationship is the header's own: {rels}");
    assert!(header.contains("xmlns:r="), "the header declares r");
}

/// The relationships of the main part, as (type, target) pairs.
pub(super) fn main_rels(doc: &Document) -> Vec<(String, String)> {
    doc.pkg.rels_for("word/document.xml").unwrap().into_iter().map(|r| (r.rel_type, r.target)).collect()
}

#[test]
fn a_picture_fills_relationship_is_given_back_when_the_op_is_refused_or_undone() {
    let mut doc = Sh::new().open();
    // Not the same bytes as a picture the file has: the same picture is stored (and related) once.
    let mut other = PNG.to_vec();
    other.push(0);
    let media = doc.add_media("png", other);
    let before = main_rels(&doc);
    // The picture is fine but the rest of the patch is not: nothing of it stays, the relationship included.
    let e = fail(&mut doc, json!({"fill": {"kind": "picture", "media": media}, "w": 5000}));
    assert!(!e.is_empty());
    assert_eq!(main_rels(&doc), before, "a refused op left a relationship");
    assert!(!doc.undo(), "a refused op is not an undo step");
    // A good one adds it; undo takes it away again, redo brings it back.
    set(&mut doc, json!({"fill": {"kind": "picture", "media": media}}));
    assert_eq!(main_rels(&doc).len(), before.len() + 1);
    assert!(doc.undo());
    assert_eq!(main_rels(&doc), before, "undo left the relationship");
    assert!(doc.redo());
    assert_eq!(main_rels(&doc).len(), before.len() + 1);
    let (bytes, xml) = saved(&mut doc);
    assert!(xml.contains("r:embed=\"rId"), "{xml}");
    assert_valid(&bytes);
}
