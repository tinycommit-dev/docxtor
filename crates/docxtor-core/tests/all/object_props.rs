//! Names, hiding and placing drawing objects, the list of all objects (the Selection pane), moving an object in
//! the stacking order to a place, and a pen's opacity and ends in freehand drawings.

use docxtor_core::render::{RBlock, RPara};
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

fn open_with(body: &str) -> Document {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    assert!(xml.contains(body), "the body was not replaced");
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn doc_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    part(&bytes, "word/document.xml")
}

fn first_para(doc: &Document) -> u32 {
    doc.render_snapshot().order[0]
}

fn rpara(doc: &Document, id: u32) -> RPara {
    doc.render_snapshot()
        .blocks
        .iter()
        .find_map(|b| match b {
            RBlock::Para(p) if p.id == id => Some(p.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no paragraph {id}"))
}

fn shape(doc: &Document, p: u32, o: usize) -> Value {
    serde_json::to_value(doc.shape_at(Pos::new(p, o)).unwrap_or_else(|| panic!("no shape at {p}:{o}"))).unwrap()
}

fn objects(doc: &Document) -> Vec<Value> {
    serde_json::to_value(doc.objects()).unwrap().as_array().unwrap().clone()
}

/// A paragraph with text and, at offsets 0 to 2, a picture, a chart and a floating shape.
fn mixed() -> (Document, u32) {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Words to float over"}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":80,"h":50,"fill":"4472C4","floating":true,"x":10,"y":5}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"wrap":"front"}));
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","title":"Sample","categories":["A","B"],"series":[{"name":"One","values":[1,2]}]}));
    let media = doc.add_media("png", vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
    ops(&mut doc, json!({"op":"insert_image","at":{"p":p,"o":0},"media":media,"width":40,"height":30,"alt":"Tiny"}));
    (doc, p)
}

const ANCHORED_OVAL: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251660288" behindDoc="1" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>127000</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>254000</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="4" name="Oval 4"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:oval id="Oval 4" o:spid="_x0000_s1028" style="position:absolute;margin-left:10pt;margin-top:20pt;width:100pt;height:50pt;z-index:-251656192;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text"/></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

/// A picture that is VML only and floats (no DrawingML copy).
const VML_ONLY: &str = r#"<w:p><w:r><w:pict><v:rect id="Rectangle 1" o:spid="_x0000_s1026" style="position:absolute;margin-left:5pt;margin-top:6pt;width:50pt;height:30pt;z-index:3;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text" fillcolor="red"/></w:pict></w:r></w:p>"#;

// ---------------------------------------------------------------------------
// The list of objects
// ---------------------------------------------------------------------------

#[test]
fn objects_lists_every_kind_in_document_order_with_names_and_defaults() {
    let (doc, p) = mixed();
    let list = objects(&doc);
    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().map(|o| o["kind"].as_str().unwrap()).collect::<Vec<_>>(), ["picture", "chart", "shape"]);
    assert_eq!(list.iter().map(|o| o["o"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(list.iter().all(|o| o["p"] == p));
    assert!(list[0]["name"].as_str().unwrap().starts_with("Picture "), "{}", list[0]["name"]);
    assert!(list[1]["name"].as_str().unwrap().starts_with("Chart "));
    // A picture and a chart sit in line; the shape floats in front of the text.
    assert_eq!((list[0]["wrap"].as_str(), list[0]["floating"].as_bool()), (Some("inline"), Some(false)));
    assert_eq!((list[2]["wrap"].as_str(), list[2]["floating"].as_bool()), (Some("front"), Some(true)));
    assert!(list[2]["z"].as_u64().unwrap() > 0);
    assert!(list[0].get("hidden").is_none() && list[0].get("z").is_none());
}

#[test]
fn objects_reads_word_markup_groups_and_vml_only_shapes() {
    let doc = open_with(ANCHORED_OVAL);
    let o = &objects(&doc)[0];
    assert_eq!((o["kind"].as_str(), o["name"].as_str(), o["id"].as_u64()), (Some("shape"), Some("Oval 4"), Some(4)));
    assert_eq!((o["wrap"].as_str(), o["behind"].as_bool(), o["z"].as_u64()), (Some("behind"), Some(true), Some(251660288)));
    let doc = open_with(VML_ONLY);
    let o = &objects(&doc)[0];
    assert_eq!((o["kind"].as_str(), o["name"].as_str(), o["wrap"].as_str(), o["z"].as_u64()), (Some("shape"), Some("Rectangle 1"), Some("front"), Some(3)));
}

#[test]
fn objects_counts_offsets_past_text_and_tables() {
    let (mut doc, p) = mixed();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":3},"text":"abc"}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":6},"geometry":"ellipse","w":30,"h":30,"floating":true}));
    let list = objects(&doc);
    assert_eq!(list.len(), 4);
    assert_eq!(list[3]["o"], 6);
    assert!(doc.shape_at(Pos::new(p, 6)).is_some());
}

// ---------------------------------------------------------------------------
// Names and hiding
// ---------------------------------------------------------------------------

#[test]
fn naming_and_hiding_an_anchored_shape_writes_docpr_and_the_vml_copy() {
    let mut doc = open_with(ANCHORED_OVAL);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":0},"name":"Logo","hidden":true}));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"<wp:docPr id="4" name="Logo" hidden="1"/>"#), "{xml}");
    assert!(xml.contains(r#"<v:oval id="Logo""#) && xml.contains("visibility:hidden"), "{xml}");
    let o = &objects(&doc)[0];
    assert_eq!((o["name"].as_str(), o["hidden"].as_bool()), (Some("Logo"), Some(true)));
    // The renderer marks the segment and the shape keeps its name.
    let seg = rpara(&doc, p).segs.into_iter().find(|s| s.k == "shape").unwrap();
    assert!(seg.hidden);
    assert_eq!(seg.shape.unwrap().name.as_deref(), Some("Logo"));
    // Show it again: the attribute goes.
    ops(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":0},"hidden":false}));
    let xml = doc_xml(&mut doc);
    assert!(!xml.contains("hidden=\"1\"") && !xml.contains("visibility:hidden"), "{xml}");
    assert!(!rpara(&doc, p).segs.iter().any(|s| s.hidden));
}

#[test]
fn hiding_keeps_the_object_in_the_file_and_round_trips() {
    let (mut doc, p) = mixed();
    for o in 0..3 {
        ops(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":o},"name":format!("Item {o}"),"hidden":true}));
    }
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    let list = objects(&again);
    assert_eq!(list.len(), 3, "hidden objects are still there");
    assert!(list.iter().all(|o| o["hidden"] == true));
    assert_eq!(list.iter().map(|o| o["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Item 0", "Item 1", "Item 2"]);
    let xml = part(&out, "word/document.xml");
    assert_eq!(xml.matches("hidden=\"1\"").count(), 3);
    // Pictures, charts and shapes mark their segments.
    let p2 = first_para(&again);
    assert_eq!(rpara(&again, p2).segs.iter().filter(|s| s.hidden).count(), 3);
    // The text between them is untouched and positions did not move.
    assert_eq!(again.find_any_para(p2).unwrap().plain_text(), "Words to float over");
}

#[test]
fn naming_a_group_and_a_vml_only_shape() {
    let mut doc = open_with(VML_ONLY);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":0},"name":"Frame","hidden":true}));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"<v:rect id="Frame""#) && xml.contains("visibility:hidden"), "{xml}");
    let o = &objects(&doc)[0];
    assert_eq!((o["name"].as_str(), o["hidden"].as_bool()), (Some("Frame"), Some(true)));
    assert!(rpara(&doc, p).segs.iter().any(|s| s.hidden));
    ops(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":0},"hidden":false}));
    assert!(!rpara(&doc, p).segs.iter().any(|s| s.hidden));
}

#[test]
fn name_and_hidden_are_undone_in_one_step() {
    let (mut doc, p) = mixed();
    let before = objects(&doc);
    ops(&mut doc, json!([
        {"op":"set_object_props","at":{"p":p,"o":0},"name":"A","hidden":true},
        {"op":"set_object_props","at":{"p":p,"o":2},"name":"B"}
    ]));
    assert_ne!(objects(&doc), before);
    assert!(doc.undo());
    assert_eq!(objects(&doc), before);
    assert!(doc.redo());
    assert_eq!(objects(&doc)[2]["name"], "B");
}

#[test]
fn naming_rejects_bad_input() {
    let (mut doc, p) = mixed();
    let e = err(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":2},"name":"  "}));
    assert!(e.contains("can't be empty"), "{e}");
    let e = err(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":9},"name":"x"}));
    assert!(e.contains("no object"), "{e}");
    // A character that is not an object.
    let e = err(&mut doc, json!({"op":"set_object_props","at":{"p":p,"o":5},"name":"x"}));
    assert!(e.contains("no object"), "{e}");
}

// ---------------------------------------------------------------------------
// Placing
// ---------------------------------------------------------------------------

#[test]
fn aligning_to_the_margin_or_page_writes_relative_from_and_align_as_word_does() {
    let mut doc = open_with(ANCHORED_OVAL);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":0},"rel_x":"margin","align":"center","rel_y":"page","align_y":"bottom"}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["relX"].as_str(), s["align"].as_str(), s["relY"].as_str(), s["alignY"].as_str()), (Some("margin"), Some("center"), Some("page"), Some("bottom")));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"<wp:positionH relativeFrom="margin"><wp:align>center</wp:align></wp:positionH>"#), "{xml}");
    assert!(xml.contains(r#"<wp:positionV relativeFrom="page"><wp:align>bottom</wp:align></wp:positionV>"#), "{xml}");
    // The wrapping stays, and the VML copy says the same.
    assert!(xml.contains(r#"behindDoc="1""#) && xml.contains("<wp:wrapNone/>"));
    assert!(xml.contains("mso-position-horizontal:center") && xml.contains("mso-position-horizontal-relative:margin"), "{xml}");
    assert!(xml.contains("mso-position-vertical:bottom") && xml.contains("mso-position-vertical-relative:page"), "{xml}");
    // A shape that wraps text keeps square wrapping.
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":80,"h":50,"floating":true,"x":10,"y":5}));
    ops(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":0},"rel_x":"page","align":"right"}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["wrap"].as_str(), s["relX"].as_str(), s["align"].as_str()), (Some("square"), Some("page"), Some("right")));
}

#[test]
fn placing_by_offset_replaces_an_alignment_and_works_for_pictures_and_charts() {
    let (mut doc, p) = mixed();
    for o in 0..2 {
        ops(&mut doc, json!({"op":"set_image","at":{"p":p,"o":o},"wrap":"square","align":"left"}));
    }
    ops(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":0},"rel_x":"page","x":36,"rel_y":"margin","y":72}));
    ops(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":1},"rel_x":"margin","align":"right","align_y":"center"}));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"<wp:positionH relativeFrom="page"><wp:posOffset>457200</wp:posOffset></wp:positionH>"#), "{xml}");
    assert!(xml.contains(r#"<wp:positionV relativeFrom="margin"><wp:posOffset>914400</wp:posOffset></wp:positionV>"#), "{xml}");
    assert!(xml.contains(r#"<wp:positionH relativeFrom="margin"><wp:align>right</wp:align></wp:positionH>"#), "{xml}");
    let pic = &objects(&doc)[0];
    assert_eq!(pic["wrap"], "square");
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(objects(&again).len(), 3);
}

#[test]
fn placing_needs_a_floating_object_and_a_valid_position() {
    let (mut doc, p) = mixed();
    // The picture is in line.
    let e = err(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":0},"align":"left"}));
    assert!(e.contains("floats") || e.contains("floating") || e.contains("in line"), "{e}");
    let e = err(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":2},"rel_x":"elsewhere"}));
    assert!(e.contains("unknown position base"), "{e}");
    let e = err(&mut doc, json!({"op":"set_object_position","at":{"p":p,"o":2},"align_y":"left"}));
    assert!(e.contains("unknown alignment"), "{e}");
}

#[test]
fn placing_many_objects_is_one_undo_step() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    for i in 0..3 {
        ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":i},"geometry":"rect","w":40,"h":40,"floating":true,"x":10.0 * i as f64,"y":0}));
    }
    let before = (0..3).map(|o| shape(&doc, p, o)).collect::<Vec<_>>();
    ops(&mut doc, json!((0..3).map(|o| json!({"op":"set_object_position","at":{"p":p,"o":o},"rel_x":"page","align":"left"})).collect::<Vec<_>>()));
    assert!((0..3).all(|o| shape(&doc, p, o)["align"] == "left"));
    assert!(doc.undo());
    assert_eq!((0..3).map(|o| shape(&doc, p, o)).collect::<Vec<_>>(), before);
}

// ---------------------------------------------------------------------------
// Moving in the stack
// ---------------------------------------------------------------------------

fn floating_three() -> (Document, u32) {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    for i in 0..3 {
        ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":i},"geometry":"rect","w":40,"h":40,"floating":true,"x":10.0 * i as f64,"y":0}));
        ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":i},"wrap":"front"}));
    }
    (doc, p)
}

fn zs(doc: &Document, p: u32) -> Vec<u64> {
    (0..3).map(|o| shape(doc, p, o)["z"].as_u64().unwrap()).collect()
}

fn order_of(z: &[u64]) -> Vec<usize> {
    let mut s: Vec<usize> = (0..z.len()).collect();
    s.sort_by_key(|&i| (z[i], i));
    s
}

#[test]
fn move_object_puts_it_directly_above_or_below_another() {
    let (mut doc, p) = floating_three();
    assert_eq!(order_of(&zs(&doc, p)), vec![0, 1, 2]);
    ops(&mut doc, json!({"op":"move_object","at":{"p":p,"o":0},"above":{"p":p,"o":2}}));
    assert_eq!(order_of(&zs(&doc, p)), vec![1, 2, 0]);
    ops(&mut doc, json!({"op":"move_object","at":{"p":p,"o":2},"below":{"p":p,"o":1}}));
    assert_eq!(order_of(&zs(&doc, p)), vec![2, 1, 0]);
    ops(&mut doc, json!({"op":"move_object","at":{"p":p,"o":2},"above":{"p":p,"o":1}}));
    assert_eq!(order_of(&zs(&doc, p)), vec![1, 2, 0]);
    // The saved file carries the new values, and the VML copy follows.
    let xml = doc_xml(&mut doc);
    let heights: Vec<u64> = xml.split("relativeHeight=\"").skip(1).map(|s| s[..s.find('"').unwrap()].parse().unwrap()).collect();
    assert_eq!(order_of(&heights), vec![1, 2, 0]);
    let z: Vec<i64> = xml.split("z-index:").skip(1).map(|s| s[..s.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap()].parse().unwrap()).collect();
    assert_eq!(z.iter().map(|v| v.unsigned_abs()).collect::<Vec<_>>(), heights);
}

#[test]
fn move_object_is_one_undo_step_and_a_no_op_when_already_there() {
    let (mut doc, p) = floating_three();
    let before = zs(&doc, p);
    ops(&mut doc, json!({"op":"move_object","at":{"p":p,"o":1},"above":{"p":p,"o":2}}));
    assert_ne!(zs(&doc, p), before);
    assert!(doc.undo());
    assert_eq!(zs(&doc, p), before);
    // Directly above its neighbour already: nothing changes.
    ops(&mut doc, json!({"op":"move_object","at":{"p":p,"o":1},"above":{"p":p,"o":0}}));
    assert_eq!(zs(&doc, p), before);
    ops(&mut doc, json!({"op":"move_object","at":{"p":p,"o":1},"above":{"p":p,"o":1}}));
    assert_eq!(zs(&doc, p), before);
}

#[test]
fn move_object_keeps_the_two_stacks_apart_and_rejects_bad_input() {
    let (mut doc, p) = floating_three();
    ops(&mut doc, json!({"op":"set_behind_text","at":{"p":p,"o":0},"behind":true}));
    let e = err(&mut doc, json!({"op":"move_object","at":{"p":p,"o":1},"above":{"p":p,"o":0}}));
    assert!(e.contains("same stack"), "{e}");
    let e = err(&mut doc, json!({"op":"move_object","at":{"p":p,"o":1}}));
    assert!(e.contains("above") && e.contains("below"), "{e}");
    let e = err(&mut doc, json!({"op":"move_object","at":{"p":p,"o":1},"above":{"p":p,"o":2},"below":{"p":p,"o":2}}));
    assert!(e.contains("above") && e.contains("below"), "{e}");
    let (mut doc, p) = mixed();
    let e = err(&mut doc, json!({"op":"move_object","at":{"p":p,"o":0},"above":{"p":p,"o":2}}));
    assert!(e.contains("float"), "{e}");
}

#[test]
fn move_object_numbers_a_crowded_stack_again() {
    let mut doc = open_with(&format!("{ANCHORED_OVAL}{ANCHORED_OVAL}{ANCHORED_OVAL}"));
    let ids: Vec<u32> = doc.render_snapshot().order.clone();
    // Equal stacking values: document order decides, and the move still works.
    ops(&mut doc, json!({"op":"move_object","at":{"p":ids[0],"o":0},"above":{"p":ids[2],"o":0}}));
    let z: Vec<u64> = ids.iter().map(|&p| shape(&doc, p, 0)["z"].as_u64().unwrap()).collect();
    assert_eq!(order_of(&z), vec![1, 2, 0], "{z:?}");
}

// ---------------------------------------------------------------------------
// Freehand drawing with a highlighter or a pencil
// ---------------------------------------------------------------------------

fn stroke() -> Vec<[f64; 2]> {
    (0..20).map(|i| [20.0 + i as f64 * 4.0, 40.0 + (i as f64 / 3.0).sin() * 10.0]).collect()
}

#[test]
fn ink_with_alpha_and_square_ends_is_written_as_a_highlighter() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[stroke()],"color":"FFEB3B","width_pt":15,"alpha":0.5,"cap":"square"}));
    let s = shape(&doc, p, 0);
    assert_eq!(s["line"]["color"], "FFEB3B");
    assert_eq!(s["line"]["alpha"].as_f64(), Some(0.5));
    assert_eq!(s["line"]["cap"], "square");
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"cap="sq""#), "{xml}");
    assert!(xml.contains(r#"<a:srgbClr val="FFEB3B"><a:alpha val="50000"/></a:srgbClr>"#), "{xml}");
    assert!(xml.contains(r#"<v:stroke endcap="square" joinstyle="round" opacity="0.5"/>"#), "{xml}");
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(shape(&again, first_para(&again), 0)["line"]["alpha"].as_f64(), Some(0.5));
    // One undo removes it.
    assert!(doc.undo());
    assert!(doc.objects().is_empty());
}

#[test]
fn ink_without_alpha_or_cap_is_written_as_before() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[stroke()],"color":"D93025","width_pt":2}));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"cap="rnd""#) && xml.contains(r#"<a:srgbClr val="D93025"/>"#), "{xml}");
    assert!(xml.contains(r#"<v:stroke endcap="round" joinstyle="round"/>"#), "{xml}");
    assert!(!xml.contains("<a:alpha"));
    let s = shape(&doc, p, 0);
    assert!(s["line"].get("alpha").is_none());
}

#[test]
fn ink_pens_in_a_batch_are_one_undo_step() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!([
        {"op":"insert_ink","at":{"p":p,"o":0},"strokes":[stroke()],"color":"000000","width_pt":1.5,"alpha":0.85},
        {"op":"insert_ink","at":{"p":p,"o":0},"strokes":[stroke()],"color":"FFEB3B","width_pt":15,"alpha":0.5,"cap":"square"}
    ]));
    assert_eq!(doc.objects().len(), 2);
    assert!(doc.undo());
    assert!(doc.objects().is_empty());
}

#[test]
fn ink_rejects_a_bad_alpha_or_cap() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    let e = err(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[stroke()],"alpha":1.5}));
    assert!(e.contains("alpha"), "{e}");
    let e = err(&mut doc, json!({"op":"insert_ink","at":{"p":p,"o":0},"strokes":[stroke()],"cap":"pointy"}));
    assert!(e.contains("unknown cap"), "{e}");
}

// ---------------------------------------------------------------------------
// The assistant
// ---------------------------------------------------------------------------

#[test]
fn the_list_objects_tool_gives_positions_the_ops_take() {
    let (mut doc, p) = mixed();
    let out = docxtor_core::ai::call_tool(&mut doc, "list_objects", &json!({})).unwrap();
    let list: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 3);
    assert_eq!(list[2]["kind"], "shape");
    assert_eq!(list[2]["p"], p);
    // The position it names is the one `set_object_props` takes.
    let at = json!({"p": list[2]["p"], "o": list[2]["o"]});
    ops(&mut doc, json!({"op":"set_object_props","at":at,"name":"From the tool"}));
    assert_eq!(objects(&doc)[2]["name"], "From the tool");
    let mut blank = Document::new_blank();
    assert!(docxtor_core::ai::call_tool(&mut blank, "list_objects", &json!({})).unwrap().starts_with("No shapes"));
}
