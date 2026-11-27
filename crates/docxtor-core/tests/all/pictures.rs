//! Pictures: crop, turn, outline, adjustments and effects, position and wrapping, linked pictures and SVG
//! pictures. Each edit is an op (`set_image`), so it also undoes and survives a save.

use docxtor_core::package::Package;
use docxtor_core::{Document, Pos};
use serde_json::{json, Value};

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

fn document_xml(bytes: &[u8]) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap()
}

/// A document with one inline picture (100 x 50 pt) in its first paragraph.
fn with_picture() -> (Document, u32) {
    let mut doc = Document::new_blank();
    let p = doc.render_snapshot().order[0];
    let media = doc.add_media("png", vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10]);
    ops(&mut doc, json!({"op": "insert_image", "at": {"p": p, "o": 0}, "media": media, "width": 100, "height": 50, "alt": "A test picture"}));
    (doc, p)
}

/// The picture at the start of the paragraph, as the view model hands it to the editor.
fn picture(doc: &Document, p: u32) -> Value {
    serde_json::to_value(doc.picture_at(Pos::new(p, 0)).expect("a picture")).unwrap()
}

#[test]
fn a_plain_picture_has_no_formatting() {
    let (doc, p) = with_picture();
    let v = picture(&doc, p);
    assert_eq!(v["w"], 100.0);
    assert_eq!(v["h"], 50.0);
    assert_eq!(v["wrap"], "inline");
    assert!(v.get("pic").is_none(), "{v}");
    assert!(v.get("place").is_none());
}

#[test]
fn crop_turn_outline_and_effects_are_written_the_way_word_writes_them_and_undo() {
    let (mut doc, p) = with_picture();
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "crop": [0.1, 0.0, 0.1, 0.2], "rot": 90, "flip_h": true, "geometry": "roundRect", "corner": 0.2}));
    ops(
        &mut doc,
        json!({"op": "set_image", "at": {"p": p, "o": 0}, "border": {"color": "FF4F00", "width_pt": 2, "dash": "dash"}, "shadow": {"blur_pt": 5, "dist_pt": 4, "angle": 45, "color": "000000", "alpha": 0.5}, "glow": {"radius_pt": 8, "color": "accent1", "alpha": 0.4}, "soft_edge": 3, "reflection": {"size": 0.5, "alpha": 0.4, "dist_pt": 2}, "opacity": 0.8, "brightness": 0.2, "contrast": -0.3, "recolor": {"kind": "grayscale"}}),
    );
    let v = picture(&doc, p);
    let pic = &v["pic"];
    // The size followed the crop: 100 x 50 shown was the whole picture, so 80% of its width and 80% of its height remain.
    assert_eq!(v["w"], 80.0);
    assert_eq!(v["h"], 40.0);
    assert_eq!(pic["crop"], json!([0.1, 0.0, 0.1, 0.2]));
    assert_eq!(pic["rot"], 90.0);
    assert_eq!(pic["flipH"], true);
    assert_eq!(pic["geom"], "roundRect");
    assert_eq!(pic["adj"], 0.2);
    assert_eq!(pic["line"], json!({"color": "FF4F00", "widthPt": 2.0, "dash": "dash"}));
    assert_eq!(pic["shadow"]["distPt"], 4.0);
    assert_eq!(pic["glow"]["radiusPt"], 8.0);
    assert_eq!(pic["softEdge"], 3.0);
    assert_eq!(pic["reflection"]["size"], 0.5);
    assert_eq!(pic["opacity"], 0.8);
    assert_eq!(pic["bright"], 0.2);
    assert_eq!(pic["contrast"], -0.3);
    assert_eq!(pic["recolor"]["kind"], "grayscale");
    // Room around the turned, outlined and shadowed picture.
    assert!(pic["extent"][0].as_f64().unwrap() > 0.0);

    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = document_xml(&bytes);
    for needle in [
        r#"<a:srcRect l="10000" r="10000" b="20000"/>"#,
        r#"rot="5400000""#,
        r#"flipH="1""#,
        r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 20000"/></a:avLst></a:prstGeom>"#,
        r#"<a:alphaModFix amt="80000"/>"#,
        r#"<a:lum bright="20000" contrast="-30000"/>"#,
        "<a:grayscl/>",
        r#"<a:ln w="25400"><a:solidFill><a:srgbClr val="FF4F00"/></a:solidFill><a:prstDash val="dash"/></a:ln>"#,
        r#"<a:softEdge rad="38100"/>"#,
    ] {
        assert!(xml.contains(needle), "missing {needle} in {xml}");
    }
    // The effects come in the schema's order.
    let fx = xml.find("<a:effectLst>").unwrap();
    let order: Vec<_> = ["<a:glow", "<a:outerShdw", "<a:reflection", "<a:softEdge"].iter().map(|t| xml[fx..].find(t).unwrap()).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");

    // It all comes back from the saved file.
    let again = Document::open(&bytes).unwrap();
    let w = picture(&again, again.render_snapshot().order[0]);
    assert_eq!(w["pic"], v["pic"]);

    // One undo step per op: the picture is back to plain, in two steps.
    assert!(doc.undo());
    let mid = picture(&doc, p);
    assert_eq!(mid["pic"]["rot"], 90.0);
    assert!(mid["pic"].get("shadow").is_none());
    assert!(doc.undo());
    let v = picture(&doc, p);
    assert!(v.get("pic").is_none(), "{v}");
    assert_eq!(v["w"], 100.0);
    assert!(doc.redo());
    assert_eq!(picture(&doc, p)["pic"]["rot"], 90.0);
}

#[test]
fn reset_picture_removes_every_adjustment_and_the_crop() {
    let (mut doc, p) = with_picture();
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "crop": [0.2, 0.2, 0.2, 0.2], "rot": 30, "border": {"color": "000000", "width_pt": 1}, "shadow": {}, "opacity": 0.5, "geometry": "ellipse"}));
    assert!(picture(&doc, p).get("pic").is_some());
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "reset": true}));
    let v = picture(&doc, p);
    assert!(v.get("pic").is_none(), "{v}");
    // The whole picture, at the scale it was shown at.
    assert_eq!(v["w"], 100.0);
    assert_eq!(v["h"], 50.0);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = document_xml(&bytes);
    assert!(!xml.contains("a:ln ") && !xml.contains("effectLst") && !xml.contains("alphaModFix") && !xml.contains(r#"rot=""#));
}

#[test]
fn bad_picture_values_are_refused_with_a_plain_message() {
    let (mut doc, p) = with_picture();
    assert!(err(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "geometry": "banana"})).contains("unknown picture shape"));
    assert!(err(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "recolor": {"kind": "rainbow"}})).contains("unknown recolour"));
    assert!(err(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap": "floaty"})).contains("unknown wrap"));
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap": "square"}));
    assert!(err(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "rel_x": "moon"})).contains("unknown horizontal reference"));
    assert!(err(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap_text": "up"})).contains("unknown text side"));
}

#[test]
fn position_wrapping_and_distances_are_numbers() {
    let (mut doc, p) = with_picture();
    // Giving a position makes an in-line picture float.
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "x": 72, "y": 36, "rel_x": "page", "rel_y": "margin", "dist": [3, 6, 9, 12], "wrap_text": "left", "lock_anchor": true, "allow_overlap": false}));
    let v = picture(&doc, p);
    assert_eq!(v["wrap"], "square");
    assert_eq!((v["x"].as_f64(), v["y"].as_f64()), (Some(72.0), Some(36.0)));
    assert_eq!((v["relX"].as_str(), v["relY"].as_str()), (Some("page"), Some("margin")));
    assert_eq!(v["place"]["dist"], json!([3.0, 6.0, 9.0, 12.0]));
    assert_eq!(v["place"]["wrapText"], "left");
    assert_eq!(v["place"]["locked"], true);
    assert_eq!(v["place"]["overlap"], false);
    let xml = document_xml(&doc.save().unwrap());
    assert!(xml.contains(r#"distT="38100" distB="76200" distL="114300" distR="152400""#), "{xml}");
    assert!(xml.contains(r#"locked="1""#) && xml.contains(r#"allowOverlap="0""#));
    assert!(xml.contains(r#"<wp:positionH relativeFrom="page"><wp:posOffset>914400</wp:posOffset></wp:positionH>"#));
    assert!(xml.contains(r#"<wp:wrapSquare wrapText="left"/>"#));

    // Aligned instead of offset, relative to something else.
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "align": "inside", "rel_x": "margin", "align_y": "bottom", "rel_y": "page"}));
    let v = picture(&doc, p);
    assert_eq!(v["place"]["alignH"], "inside");
    assert_eq!(v["alignY"], "bottom");
    assert_eq!(v["relY"], "page");
    assert!(v.get("x").is_none() && v.get("y").is_none());
    assert_valid(&doc.save().unwrap());

    // Tight and through wrapping carry the rectangle Word writes for a box, and keep which sides.
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap": "tight"}));
    let xml = document_xml(&doc.save().unwrap());
    assert!(xml.contains(r#"<wp:wrapTight wrapText="left"><wp:wrapPolygon edited="0"><wp:start x="0" y="0"/><wp:lineTo x="0" y="21600"/><wp:lineTo x="21600" y="21600"/><wp:lineTo x="21600" y="0"/><wp:lineTo x="0" y="0"/></wp:wrapPolygon></wp:wrapTight>"#), "{xml}");
    assert_eq!(picture(&doc, p)["wrap"], "tight");
    assert!(picture(&doc, p).get("place").unwrap().get("through").is_none());
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap": "through", "wrap_text": "bothSides"}));
    let xml = document_xml(&doc.save().unwrap());
    assert!(xml.contains("<wp:wrapThrough wrapText=\"bothSides\">") && !xml.contains("wrapTight"));
    let v = picture(&doc, p);
    assert_eq!(v["wrap"], "tight");
    assert_eq!(v["place"]["through"], true);
    // Changing only the wrap keeps the polygon and the place.
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap": "topBottom"}));
    assert_eq!(picture(&doc, p)["wrap"], "topBottom");
    assert_valid(&doc.save().unwrap());
    // In line again: the distances and position go; undo brings the floating picture back.
    ops(&mut doc, json!({"op": "set_image", "at": {"p": p, "o": 0}, "wrap": "inline"}));
    assert_eq!(picture(&doc, p)["wrap"], "inline");
    assert!(doc.undo());
    assert_eq!(picture(&doc, p)["wrap"], "topBottom");
}

#[test]
fn a_picture_that_is_only_linked_says_where_it_points() {
    let mut doc = Document::new_blank();
    let bytes = doc.save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    let rid = pkg.add_rel("word/document.xml", docxtor_core::package::REL_IMAGE, "file:///C:/Users/ana/Pictures/logo%20final.png", true);
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let drawing = format!(
        r#"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1270000" cy="635000"/><wp:docPr id="1" name="Linked"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="logo"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:link="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    );
    let xml = xml
        .replacen("<w:document ", r#"<w:document xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#, 1)
        .replacen("<w:body><w:p/>", &format!("<w:body>{drawing}"), 1);
    pkg.set("word/document.xml", xml.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let p = doc.render_snapshot().order[0];
    let v = picture(&doc, p);
    assert_eq!(v["pic"]["link"], "file:///C:/Users/ana/Pictures/logo%20final.png");
    assert_eq!(v["w"], 100.0);
}

#[test]
fn a_picture_with_an_svg_version_names_it() {
    let mut doc = Document::new_blank();
    let bytes = doc.save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    pkg.ensure_default("png", "image/png");
    pkg.ensure_default("svg", "image/svg+xml");
    pkg.set("word/media/image1.png", vec![0x89, b'P', b'N', b'G']);
    pkg.set("word/media/image2.svg", br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#.to_vec());
    let png = pkg.add_rel("word/document.xml", docxtor_core::package::REL_IMAGE, "media/image1.png", false);
    let svg = pkg.add_rel("word/document.xml", docxtor_core::package::REL_IMAGE, "media/image2.svg", false);
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let drawing = format!(
        r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="1270000" cy="635000"/><wp:docPr id="1" name="Vector"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="v"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{png}"><a:extLst><a:ext uri="{{96DAC541-7B7A-43D3-8B79-37D633B846F1}}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="{svg}"/></a:ext></a:extLst></a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    );
    let xml = xml
        .replacen("<w:document ", r#"<w:document xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#, 1)
        .replacen("<w:body><w:p/>", &format!("<w:body>{drawing}"), 1);
    pkg.set("word/document.xml", xml.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let p = doc.render_snapshot().order[0];
    let v = picture(&doc, p);
    assert_eq!(v["src"], "word/media/image1.png");
    assert_eq!(v["pic"]["svg"], "word/media/image2.svg");
    // The theme's colours are there for pickers.
    assert!(doc.theme_colors().iter().any(|(n, _)| n == "accent1"));
}

#[test]
fn the_ai_tools_know_the_new_fields() {
    let guide = docxtor_core::ai::tool_definitions().to_string();
    for word in ["crop", "flip_h", "geometry", "soft_edge", "wrap_text", "allow_overlap", "reset"] {
        assert!(guide.contains(word), "apply_ops should describe {word}");
    }
}
