//! Arranging objects: the stacking order, behind and in front of the text, groups and their members, and
//! freehand drawings.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::render::RBlock;
use docxtor_core::stories::StoryKind;
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

const NS: &str = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" mc:Ignorable="w14 wp14""#;



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

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"/><a:ln w="12700"/><a:ln w="19050"/></a:lnStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

/// A package with `body` in place of the blank document's empty paragraph, as the bytes of a .docx.
fn package_with(body: &str, theme: bool) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    assert!(xml.contains(body), "the body was not replaced");
    pkg.set("word/document.xml", xml.into_bytes());
    if theme {
        let target = "theme/theme1.xml";
        pkg.add_rel("word/document.xml", docxtor_core::package::REL_THEME, target, false);
        pkg.set("word/theme/theme1.xml", THEME.as_bytes().to_vec());
        pkg.ensure_override("word/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml");
    }
    pkg.to_bytes().unwrap()
}

fn open_with(body: &str) -> Document {
    Document::open(&package_with(body, false)).unwrap()
}

fn doc_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    part(&bytes, "word/document.xml")
}

fn first_para(doc: &Document) -> u32 {
    doc.render_snapshot().order[0]
}

/// The shape at `{p, o}` as JSON, the way the renderer receives it.
fn shape(doc: &Document, p: u32, o: usize) -> Value {
    serde_json::to_value(doc.shape_at(Pos::new(p, o)).unwrap_or_else(|| panic!("no shape at {p}:{o}"))).unwrap()
}

/// Paragraphs inside text boxes: (id, text), in order.
fn box_paras(doc: &Document) -> Vec<(u32, String)> {
    let mut v = Vec::new();
    for s in doc.stories.iter().filter(|s| s.kind == StoryKind::TextBox) {
        walk_paragraphs(&s.blocks, &mut |p| v.push((p.id, p.plain_text())));
    }
    v
}

fn box_count(doc: &Document) -> usize {
    doc.stories.iter().filter(|s| s.kind == StoryKind::TextBox).count()
}

// ---------------------------------------------------------------------------
// Word's markup
// ---------------------------------------------------------------------------

const ELLIPSE_WITH_TEXT: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251660288" behindDoc="1" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:align>center</wp:align></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>914400</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="4" name="Oval 4"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"><a:alpha val="50000"/></a:srgbClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:ln><a:noFill/></a:ln></wps:spPr><wps:txbx><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello oval</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:oval id="Oval 4" o:spid="_x0000_s1028" style="position:absolute;margin-left:0;margin-top:72pt;width:100pt;height:50pt;z-index:-251656192;mso-position-horizontal:center;mso-position-horizontal-relative:page;mso-position-vertical-relative:page" stroked="f"><v:textbox inset="0,0,0,0"><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello oval</w:t></w:r></w:p></w:txbxContent></v:textbox></v:oval></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

const GROUP: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3048000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="5" name="Group 5"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3048000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="6096000" cy="2540000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:cNvPr id="6" name="Left box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="7" name="Right oval"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="3556000" y="1270000"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>In the group</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:group id="Group 5" o:spid="_x0000_s1030" style="width:240pt;height:100pt" coordsize="480,200"><v:rect id="Left box" style="position:absolute;width:200;height:100" fillcolor="red"/><v:oval id="Right oval" style="position:absolute;left:280;top:100;width:200;height:100" fillcolor="lime"><v:textbox><w:txbxContent><w:p><w:r><w:t>In the group</w:t></w:r></w:p></w:txbxContent></v:textbox></v:oval></v:group></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

const CANVAS: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpc"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="9" name="Canvas 9"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"><wpc:wpc><wpc:bg><a:noFill/></wpc:bg><wpc:whole><a:ln><a:noFill/></a:ln></wpc:whole><wps:wsp><wps:cNvPr id="10" name="Canvas box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="254000" y="254000"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FFC000"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>Canvas text</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="11" name="Canvas star"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="2540000" y="1270000"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="star5"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="70AD47"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp></wpc:wpc></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect id="Canvas" style="width:432pt;height:252pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

fn blank_with_text(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":text}));
    (doc, id)
}

/// Three floating shapes in front of the text, in one paragraph, at offsets 0, 1 and 2.
fn three_shapes() -> (Document, u32) {
    let (mut doc, p) = blank_with_text("Some text to float over");
    for i in 0..3 {
        ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":i},"geometry":"rect","w":80,"h":50,"fill":"4472C4","floating":true,"x":10.0 + 20.0 * i as f64,"y":5}));
        ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":i},"wrap":"front"}));
    }
    (doc, p)
}

fn zs(doc: &Document, p: u32, n: usize) -> Vec<u64> {
    (0..n).map(|o| shape(doc, p, o)["z"].as_u64().unwrap()).collect()
}

/// Every `relativeHeight` in the saved body, in document order.
fn saved_heights(doc: &mut Document) -> Vec<u64> {
    let xml = doc_xml(doc);
    xml.split("relativeHeight=\"").skip(1).map(|s| s[..s.find('"').unwrap()].parse().unwrap()).collect()
}

/// Every `z-index` in the saved body's VML, in document order.
fn saved_z_indexes(doc: &mut Document) -> Vec<i64> {
    let xml = doc_xml(doc);
    xml.split("z-index:").skip(1).map(|s| s[..s.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap()].parse().unwrap()).collect()
}

fn order_of(z: &[u64]) -> Vec<usize> {
    let mut s: Vec<usize> = (0..z.len()).collect();
    s.sort_by_key(|&i| (z[i], i));
    s
}

#[test]
fn shapes_report_their_stacking_value_and_where_they_are_placed_from() {
    let doc = open_with(ELLIPSE_WITH_TEXT);
    let s = shape(&doc, first_para(&doc), 0);
    assert_eq!(s["wrap"], "behind");
    assert_eq!(s["z"], 251660288u64);
    assert_eq!((s["relX"].as_str(), s["align"].as_str()), (Some("page"), Some("center")));
    assert_eq!(s["relY"], "page");
    assert_eq!(s["y"], 72.0);
    let (doc, p) = three_shapes();
    let z = zs(&doc, p, 3);
    assert!(z[0] < z[1] && z[1] < z[2], "{z:?}");
}

#[test]
fn bring_to_front_and_send_to_back_change_relative_height_and_the_fallbacks_z_index() {
    let (mut doc, p) = three_shapes();
    let before = zs(&doc, p, 3);
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"front"}));
    let z = zs(&doc, p, 3);
    assert_eq!(order_of(&z), vec![1, 2, 0]);
    assert_eq!((z[1], z[2]), (before[1], before[2]), "only the moved shape changes");
    // The saved file says the same in DrawingML and in VML.
    assert_eq!(saved_heights(&mut doc), z);
    assert_eq!(saved_z_indexes(&mut doc), z.iter().map(|v| *v as i64).collect::<Vec<_>>());
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":2},"order":"back"}));
    assert_eq!(order_of(&zs(&doc, p, 3)), vec![2, 1, 0]);
    // Undo, one step each.
    assert!(doc.undo());
    assert_eq!(zs(&doc, p, 3), z);
    assert!(doc.undo());
    assert_eq!(zs(&doc, p, 3), before);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn forward_and_backward_swap_neighbours_and_equal_values_are_numbered_again() {
    let (mut doc, p) = three_shapes();
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"forward"}));
    assert_eq!(order_of(&zs(&doc, p, 3)), vec![1, 0, 2]);
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":2},"order":"backward"}));
    assert_eq!(order_of(&zs(&doc, p, 3)), vec![1, 2, 0]);
    // Already on top: nothing changes.
    let z = zs(&doc, p, 3);
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"forward"}));
    assert_eq!(zs(&doc, p, 3), z);
    // Shapes written with the same value (as some programs do) can still be told apart.
    let xml = doc_xml(&mut doc);
    let same = xml.split("relativeHeight=\"").enumerate().map(|(i, s)| if i == 0 { s.to_string() } else { format!("251658240{}", &s[s.find('"').unwrap()..]) }).collect::<Vec<_>>().join("relativeHeight=\"");
    let mut again = open_with(&same[same.find("<w:p>").or(same.find("<w:p ")).unwrap()..same.rfind("</w:p>").unwrap() + 6]);
    let p2 = first_para(&again);
    assert_eq!(zs(&again, p2, 3), vec![251658240; 3]);
    ops(&mut again, json!({"op":"set_object_order","at":{"p":p2,"o":0},"order":"forward"}));
    assert_eq!(order_of(&zs(&again, p2, 3)), vec![1, 0, 2]);
}

#[test]
fn behind_and_in_front_of_the_text_are_two_stacks() {
    let (mut doc, p) = three_shapes();
    ops(&mut doc, json!({"op":"set_behind_text","at":{"p":p,"o":1},"behind":true}));
    assert_eq!(shape(&doc, p, 1)["wrap"], "behind");
    let behind_z = zs(&doc, p, 3)[1];
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"front"}));
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":2},"order":"back"}));
    // The shape behind the text kept its place in its own stack.
    assert_eq!(zs(&doc, p, 3)[1], behind_z);
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"behindDoc="1""#));
    // A shape's negative z-index in the fallback marks it as behind the text.
    assert!(saved_z_indexes(&mut doc).iter().any(|z| *z < 0));
}

#[test]
fn set_behind_text_keeps_the_position_and_works_for_text_that_wrapped_around() {
    let (mut doc, p) = blank_with_text("Words");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"ellipse","w":60,"h":40,"floating":true,"x":123.0,"y":17.0}));
    assert_eq!(shape(&doc, p, 0)["wrap"], "square");
    ops(&mut doc, json!({"op":"set_behind_text","at":{"p":p,"o":0},"behind":true}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["wrap"].as_str(), s["x"].as_f64(), s["y"].as_f64()), (Some("behind"), Some(123.0), Some(17.0)));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"behindDoc="1""#) && xml.contains("<wp:wrapNone/>") && !xml.contains("<wp:wrapSquare"));
    ops(&mut doc, json!({"op":"set_behind_text","at":{"p":p,"o":0},"behind":false}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["wrap"].as_str(), s["x"].as_f64()), (Some("front"), Some(123.0)));
    assert!(doc.undo() && doc.undo());
    assert_eq!(shape(&doc, p, 0)["wrap"], "square");
}

#[test]
fn ordering_needs_a_floating_object() {
    let (mut doc, p) = blank_with_text("Words");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":60,"h":40,"floating":false}));
    let e = err(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"front"}));
    assert!(e.contains("floats") || e.contains("wrapping"), "{e}");
    let e = err(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":3},"order":"front"}));
    assert!(e.contains("no object"), "{e}");
    let e = err(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"sideways"}));
    assert!(e.contains("unknown order"), "{e}");
    // A failed op leaves no marker behind.
    assert!(!doc_xml(&mut doc).contains("data-docxtor-pick"));
}

#[test]
fn grouping_joins_shapes_into_one_group_with_a_vml_fallback_and_ungrouping_puts_them_back() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":80,"h":50,"fill":"FF0000","floating":true,"x":10,"y":5}));
    ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":1},"w":100,"h":40,"text":"Boxed","floating":true,"x":120,"y":60}));
    let (box_id, _) = box_paras(&doc)[0].clone();
    ops(&mut doc, json!({"op":"group_shapes","at":[{"p":p,"o":0},{"p":p,"o":1}]}));
    // One object now, a group the size of the two shapes together.
    assert_eq!(doc.find_any_para(p).unwrap().text(), "\u{FFFC}Text");
    let g = shape(&doc, p, 0);
    assert_eq!((g["kind"].as_str(), g["children"].as_array().unwrap().len()), (Some("group"), 2));
    assert_eq!((g["w"].as_f64().unwrap(), g["h"].as_f64().unwrap()), (210.0, 95.0));
    assert_eq!((g["x"].as_f64().unwrap(), g["y"].as_f64().unwrap()), (10.0, 5.0));
    assert_eq!((g["children"][1]["childX"].as_f64().unwrap(), g["children"][1]["childY"].as_f64().unwrap()), (110.0, 55.0));
    assert_eq!(g["children"][1]["textBox"], true);
    let xml = doc_xml(&mut doc);
    for needle in ["<wpg:wgp>", r#"Requires="wpg""#, "<v:group ", "xmlns:wpg=", r#"coordsize="4200,1900""#] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
    // The text box kept its text, and typing in it reaches both copies of the group.
    assert_eq!(box_count(&doc), 1);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":box_id,"o":0},"text":"My "}));
    assert_eq!(doc_xml(&mut doc).matches("My Boxed").count(), 2);
    // Taking it apart gives the shapes back where they were.
    ops(&mut doc, json!({"op":"ungroup_shape","at":{"p":p,"o":0}}));
    assert_eq!(doc.find_any_para(p).unwrap().text(), "\u{FFFC}\u{FFFC}Text");
    let (a, b) = (shape(&doc, p, 0), shape(&doc, p, 1));
    assert_eq!((a["x"].as_f64().unwrap(), a["y"].as_f64().unwrap(), a["w"].as_f64().unwrap()), (10.0, 5.0, 80.0));
    assert_eq!((b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap(), b["textBox"].as_bool().unwrap()), (120.0, 60.0, true));
    assert_eq!(a["fill"]["color"], "FF0000");
    assert_eq!(b["text"][0]["segs"][0]["x"], "My Boxed");
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert!(!part(&out, "word/document.xml").contains("wpg:wgp"));
    // Each undo step brings one back.
    assert!(doc.undo());
    assert_eq!(shape(&doc, p, 0)["kind"], "group");
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(doc.find_any_para(p).unwrap().text(), "\u{FFFC}\u{FFFC}Text");
}

#[test]
fn grouping_rejects_what_cannot_be_grouped_and_changes_nothing() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":80,"h":50,"floating":true,"x":10,"y":5}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"rect","w":80,"h":50,"floating":false}));
    let e = err(&mut doc, json!({"op":"group_shapes","at":[{"p":p,"o":0}]}));
    assert!(e.contains("at least two"), "{e}");
    let e = err(&mut doc, json!({"op":"group_shapes","at":[{"p":p,"o":0},{"p":p,"o":1}]}));
    assert!(e.contains("in line") || e.contains("wrap"), "{e}");
    let e = err(&mut doc, json!({"op":"ungroup_shape","at":{"p":p,"o":0}}));
    assert!(e.contains("not a group"), "{e}");
    assert_eq!(doc.find_any_para(p).unwrap().text(), "\u{FFFC}\u{FFFC}Text");
}

#[test]
fn grouping_uses_the_given_offsets_when_shapes_are_placed_by_alignment() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":50,"h":50,"floating":true,"x":0,"y":0}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"ellipse","w":50,"h":50,"floating":true,"x":0,"y":0}));
    ops(&mut doc, json!({"op":"group_shapes","at":[{"p":p,"o":0},{"p":p,"o":1}],"offsets":[[100,40],[180,90]]}));
    let g = shape(&doc, p, 0);
    assert_eq!((g["x"].as_f64().unwrap(), g["y"].as_f64().unwrap(), g["w"].as_f64().unwrap(), g["h"].as_f64().unwrap()), (100.0, 40.0, 130.0, 100.0));
    assert_eq!(g["children"][1]["childX"], 80.0);
    ops(&mut doc, json!({"op":"ungroup_shape","at":{"p":p,"o":0}}));
    assert_eq!(shape(&doc, p, 1)["x"], 180.0);
}

#[test]
fn a_group_turned_a_quarter_is_taken_apart_with_each_member_turned_about_its_centre() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":40,"h":40,"floating":true,"x":0,"y":0}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"rect","w":40,"h":40,"floating":true,"x":100,"y":0}));
    ops(&mut doc, json!({"op":"group_shapes","at":[{"p":p,"o":0},{"p":p,"o":1}]}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"rot":90}));
    ops(&mut doc, json!({"op":"ungroup_shape","at":{"p":p,"o":0}}));
    // A quarter turn about the group's centre (70, 20) takes the left shape's centre (20, 20) to (70, -30).
    let a = shape(&doc, p, 0);
    assert!((a["x"].as_f64().unwrap() - 50.0).abs() < 0.01 && (a["y"].as_f64().unwrap() + 50.0).abs() < 0.01, "{a}");
    assert_eq!(a["rot"], 90.0);
}

#[test]
fn a_member_of_a_group_is_moved_sized_and_recoloured_in_the_groups_own_coordinates() {
    let mut doc = open_with(GROUP);
    let p = first_para(&doc);
    let g = shape(&doc, p, 0);
    assert_eq!((g["children"][1]["childX"].as_f64().unwrap(), g["children"][1]["childY"].as_f64().unwrap()), (140.0, 50.0));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[1],"x":100,"y":40,"w":60,"h":30,"fill":"FF00FF"}));
    let g = shape(&doc, p, 0);
    let m = &g["children"][1];
    assert_eq!((m["childX"].as_f64().unwrap(), m["childY"].as_f64().unwrap(), m["w"].as_f64().unwrap(), m["h"].as_f64().unwrap()), (100.0, 40.0, 60.0, 30.0));
    assert_eq!(m["fill"]["color"], "FF00FF");
    // The first member and the group itself are as they were.
    assert_eq!(g["children"][0]["fill"]["color"], "FF0000");
    assert_eq!((g["w"].as_f64().unwrap(), g["h"].as_f64().unwrap()), (240.0, 100.0));
    let xml = doc_xml(&mut doc);
    // In the group's child coordinates (the group is drawn at half their size).
    for needle in [r#"<a:off x="2540000" y="1016000"/>"#, r#"<a:ext cx="1524000" cy="762000"/>"#, "FF00FF"] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
    // The VML copy follows: the oval's colour and its place in a group counted in twentieths of a point.
    assert!(xml.contains("fillcolor=\"#FF00FF\""), "{xml}");
    assert!(xml.contains(r#"coordsize="9600,4000""#) && xml.contains("left:4000;top:1600;width:2400;height:1200"), "{xml}");
    // The text of the member is still its own story.
    assert_eq!(box_paras(&doc)[0].1, "In the group");
    assert!(doc.undo());
    assert_eq!(shape(&doc, p, 0)["children"][1]["childX"], 140.0);
}

#[test]
fn moving_a_member_past_the_edge_grows_the_group() {
    let mut doc = open_with(GROUP);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[1],"x":200}));
    let g = shape(&doc, p, 0);
    // The oval (100 wide) now ends at 300, the group's box was 240 wide: it grows to cover it.
    assert_eq!(g["w"].as_f64().unwrap(), 300.0);
    assert_eq!(g["children"][1]["childX"], 200.0);
    assert_eq!(g["children"][0]["childX"], 0.0);
    // Moving a member up and left grows the group on that side; the others keep their places inside it.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[0],"x":-20,"y":-10}));
    let g = shape(&doc, p, 0);
    assert_eq!((g["w"].as_f64().unwrap(), g["h"].as_f64().unwrap()), (320.0, 110.0));
    assert_eq!((g["children"][0]["childX"].as_f64().unwrap(), g["children"][0]["childY"].as_f64().unwrap()), (0.0, 0.0));
    assert_eq!(g["children"][1]["childX"], 220.0);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn members_of_a_canvas_and_bad_member_edits() {
    let mut doc = open_with(CANVAS);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[0],"x":30,"y":40,"rot":45,"alt":"A box","line":{"color":"112233","width_pt":3}}));
    let m = shape(&doc, p, 0)["children"][0].clone();
    assert_eq!((m["childX"].as_f64().unwrap(), m["childY"].as_f64().unwrap(), m["rot"].as_f64().unwrap(), m["alt"].as_str()), (30.0, 40.0, 45.0, None));
    assert_eq!((m["line"]["color"].as_str().unwrap(), m["line"]["widthPt"].as_f64().unwrap()), ("112233", 3.0));
    assert!(doc_xml(&mut doc).contains(r#"descr="A box""#));
    let e = err(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[5],"x":1}));
    assert!(e.contains("no such member"), "{e}");
    let e = err(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[0],"wrap":"behind"}));
    assert!(e.contains("wrapping of the group"), "{e}");
    let e = err(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[0,0],"x":1}));
    assert!(e.contains("not a group") || e.contains("no such member"), "{e}");
}

#[test]
fn freehand_strokes_make_one_smooth_shape_with_a_vml_path_that_survives_save_and_reopen() {
    let (mut doc, p) = blank_with_text("Draw here");
    let wave: Vec<[f64; 2]> = (0..40).map(|i| [20.0 + i as f64 * 3.0, 50.0 + (i as f64 / 4.0).sin() * 12.0]).collect();
    let dot = vec![[30.0, 90.0]];
    let r = ops(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":4},"strokes":[wave, dot, [[20,100],[140,100]]],"color":"#D93025","width_pt":3}));
    assert_eq!(r[0].caret.unwrap().o, 5);
    let s = shape(&doc, p, 4);
    assert_eq!((s["geometry"].as_str(), s["wrap"].as_str()), (Some("custom"), Some("front")));
    let paths = s["custom"]["paths"].as_array().unwrap();
    assert_eq!(paths.len(), 3, "a path for each stroke");
    assert!(paths.iter().all(|p| p["fill"] == false && p["stroke"] == true));
    assert!(paths[0]["d"].as_str().unwrap().contains('C'), "a smooth curve");
    assert!(paths[2]["d"].as_str().unwrap().contains('L'), "a straight stroke");
    assert!(s.get("fill").is_none());
    assert_eq!((s["line"]["color"].as_str().unwrap(), s["line"]["widthPt"].as_f64().unwrap()), ("D93025", 3.0));
    assert_eq!((s["line"]["cap"].as_str(), s["line"]["join"].as_str()), (Some("round"), Some("round")));
    // The box covers every stroke and the pen's width.
    let (w, h) = (s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap());
    assert!((w - 123.0).abs() < 1.0 && h > 60.0, "{w} x {h}");
    assert!((s["x"].as_f64().unwrap() - 18.5).abs() < 0.01);
    let out = doc.save().unwrap();
    assert_valid(&out);
    let xml = part(&out, "word/document.xml");
    for needle in ["<a:custGeom>", r#"fill="none""#, r#"cap="rnd""#, "<a:round/>", "<v:shape ", r#"path="m"#, "endcap=\"round\"", "<wp:wrapNone/>"] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
    let again = Document::open(&out).unwrap();
    assert_eq!(shape(&again, first_para(&again), 4)["custom"]["paths"].as_array().unwrap().len(), 3);
    // It is an ordinary shape: recolour, resize, delete.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":4},"line":{"color":"1A73E8"},"w":60}));
    let s = shape(&doc, p, 4);
    assert_eq!((s["line"]["color"].as_str().unwrap(), s["w"].as_f64().unwrap()), ("1A73E8", 60.0));
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":4},"to":{"p":p,"o":5}}));
    assert_eq!(doc.find_any_para(p).unwrap().text(), "Draw here");
    // Undo removes the drawing as one step.
    assert!(doc.undo() && doc.undo() && doc.undo());
    assert_eq!(doc.find_any_para(p).unwrap().text(), "Draw here");
}

#[test]
fn a_drawing_is_put_in_front_and_rejects_bad_input() {
    let (mut doc, p) = three_shapes();
    let top = *zs(&doc, p, 3).iter().max().unwrap();
    ops(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":3},"strokes":[[[0,0],[10,10],[20,5]]]}));
    assert!(shape(&doc, p, 3)["z"].as_u64().unwrap() > top);
    let e = err(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[]}));
    assert!(e.contains("nothing drawn"), "{e}");
    let e = err(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[[[0,0],[1,1]]],"width_pt":900}));
    assert!(e.contains("pen width"), "{e}");
    let e = err(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[[[0,0],[1,1]]],"color":"nope"}));
    assert!(e.contains("not a colour"), "{e}");
}

const VML_BOX: &str = r##"<w:p><w:r><w:pict><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype><v:shape id="_x0000_s1026" type="#_x0000_t202" style="position:absolute;margin-left:72pt;margin-top:36pt;width:144pt;height:72pt;z-index:251658240;mso-position-horizontal-relative:text;mso-position-vertical-relative:text" fillcolor="#ffffcc"><v:textbox><w:txbxContent><w:p><w:r><w:t>One</w:t></w:r></w:p></w:txbxContent></v:textbox><w10:wrap type="square"/></v:shape></w:pict></w:r><w:r><w:pict><v:shape id="_x0000_s1027" type="#_x0000_t202" style="position:absolute;margin-left:90pt;margin-top:50pt;width:144pt;height:72pt;z-index:251659264;mso-position-horizontal-relative:text;mso-position-vertical-relative:text" fillcolor="#ccffcc"><v:textbox><w:txbxContent><w:p><w:r><w:t>Two</w:t></w:r></w:p></w:txbxContent></v:textbox><w10:wrap type="square"/></v:shape></w:pict></w:r></w:p>"##;

#[test]
fn shapes_that_are_only_vml_are_ordered_by_their_z_index() {
    let mut doc = open_with(VML_BOX);
    let p = first_para(&doc);
    assert_eq!(zs(&doc, p, 2), vec![251658240, 251659264]);
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":p,"o":0},"order":"front"}));
    let z = zs(&doc, p, 2);
    assert!(z[0] > z[1], "{z:?}");
    assert_eq!(saved_z_indexes(&mut doc), z.iter().map(|v| *v as i64).collect::<Vec<_>>());
    ops(&mut doc, json!({"op":"set_behind_text","at":{"p":p,"o":1},"behind":true}));
    assert_eq!(shape(&doc, p, 1)["wrap"], "behind");
    assert!(saved_z_indexes(&mut doc)[1] < 0);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn shapes_in_a_header_can_be_ordered_and_edited() {
    let mut blank = Document::new_blank();
    ops(&mut blank, json!({"op":"add_header_footer","kind":"header"}));
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let hdr = String::from_utf8(pkg.get("word/header1.xml").unwrap().to_vec()).unwrap();
    let ell = &ELLIPSE_WITH_TEXT["<w:p>".len()..ELLIPSE_WITH_TEXT.len() - "</w:p>".len()];
    let hdr = hdr.replacen("<w:hdr ", &format!("<w:hdr {NS} "), 1).replacen("</w:pPr></w:p>", &format!("</w:pPr>{ell}</w:p>"), 1);
    pkg.set("word/header1.xml", hdr.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let host = {
        let ps = doc.page_setup();
        let RBlock::Para(hp) = &ps.headers["default"].blocks[0] else { panic!() };
        (hp.id, hp.segs.iter().find(|s| s.shape.is_some()).unwrap().o)
    };
    ops(&mut doc, json!({"op":"set_object_order","at":{"p":host.0,"o":host.1},"order":"back"}));
    ops(&mut doc, json!({"op":"set_behind_text","at":{"p":host.0,"o":host.1},"behind":false}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":host.0,"o":host.1},"fill":"00AA00"}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let header = part(&out, "word/header1.xml");
    assert!(header.contains(r#"behindDoc="0""#) && header.contains("00AA00"), "{header}");
}
