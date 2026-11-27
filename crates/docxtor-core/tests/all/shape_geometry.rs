//! Curves, freeforms and scribbles, "Edit points" (a new outline for a shape), and connectors that stay attached.

use docxtor_core::{package::Package, Document, Pos};
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

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn doc_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap()
}

fn first_para(doc: &Document) -> u32 {
    doc.render_snapshot().order[0]
}

fn shape(doc: &Document, p: u32, o: usize) -> Value {
    serde_json::to_value(doc.shape_at(Pos::new(p, o)).unwrap_or_else(|| panic!("no shape at {p}:{o}"))).unwrap()
}

fn blank_with_text(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":text}));
    (doc, id)
}

/// The commands of the first path of a shape, as `t` letters.
fn letters(s: &Value) -> String {
    s["custom"]["paths"][0]["cmds"].as_array().unwrap().iter().map(|c| c["t"].as_str().unwrap().to_string()).collect()
}

// ---------------------------------------------------------------------------
// Curves, freeforms and scribbles
// ---------------------------------------------------------------------------

#[test]
fn a_curve_through_points_is_smooth_and_open() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0],[40,-30],[80,10],[120,-20]],"smooth":true,"x":10,"y":50}));
    let s = shape(&doc, p, 0);
    assert_eq!(s["geometry"], "custom");
    // A move and a Bézier curve for each segment, no close, and no fill for an open curve.
    assert_eq!(letters(&s), "MCCC");
    assert!(s.get("fill").is_none() || s["fill"].is_null());
    assert_eq!(s["line"]["color"], "2F528F");
    // The box is the curve's bounds, and the shape sits where the first point was asked for.
    assert!(s["w"].as_f64().unwrap() >= 119.9);
    assert!(s["h"].as_f64().unwrap() > 20.0);
    let xml = doc_xml(&mut doc);
    assert!(xml.contains("<a:custGeom>"));
    assert!(xml.contains("<a:cubicBezTo>"));
    assert!(!xml.contains("<a:close/>"));
    // The VML copy has the same path.
    assert!(xml.contains(r#"o:spt="100""#));
    assert!(xml.contains(r#"path="m"#));
}

#[test]
fn a_closed_freeform_with_mixed_segments_is_filled_and_closed() {
    let (mut doc, p) = blank_with_text("Text");
    ops(
        &mut doc,
        json!({"op":"insert_freeform","at":{"p":p,"o":2},"cmds":[
            {"t":"M","pts":[[0,0]]},
            {"t":"L","pts":[[60,0],[60,40]]},
            {"t":"S","pts":[[40,60],[20,50],[0,60]]},
            {"t":"Z"}
        ]}),
    );
    let s = shape(&doc, p, 2);
    assert_eq!(s["geometry"], "custom");
    let l = letters(&s);
    assert!(l.starts_with("MLL") && l.ends_with('Z'), "{l}");
    assert!(l.contains('C'));
    assert_eq!(s["fill"]["color"], "DAE3F3");
    let path = &s["custom"]["paths"][0];
    assert!(!path["d"].as_str().unwrap().is_empty());
    let xml = doc_xml(&mut doc);
    assert!(xml.contains("<a:close/>"));
    assert!(xml.contains("<a:lnTo>"));
    assert!(xml.contains("<a:cubicBezTo>"));
}

#[test]
fn a_scribble_is_simplified_and_smoothed() {
    let (mut doc, p) = blank_with_text("Text");
    // A hundred points along a wave: far fewer curves remain.
    let pts: Vec<Value> = (0..100).map(|i| json!([i as f64, (i as f64 / 8.0).sin() * 20.0])).collect();
    ops(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":pts,"smooth":true,"label":"Freeform: Scribble"}));
    let s = shape(&doc, p, 0);
    let n = letters(&s).len();
    assert!(n > 3 && n < 40, "{n} commands");
    assert_eq!(s["name"], "Freeform: Scribble 5001");
}

#[test]
fn a_closed_smooth_curve_wraps_round() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0],[50,0],[50,50],[0,50]],"smooth":true,"closed":true}));
    let s = shape(&doc, p, 0);
    assert_eq!(letters(&s), "MCCCCZ");
}

#[test]
fn freeform_ops_are_checked() {
    let (mut doc, p) = blank_with_text("Text");
    assert!(err(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0]]})).contains("two points"));
    assert!(err(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"cmds":[{"t":"L","pts":[[0,0]]}]})).contains("start with a move"));
    assert!(err(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"cmds":[{"t":"M","pts":[[0,0]]},{"t":"C","pts":[[1,1]]}]})).contains("three points"));
    assert!(err(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0],[1e9,0]]})).contains("off the page"));
}

#[test]
fn a_freeform_can_be_undone_and_restyled_like_any_shape() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0],[50,0],[50,50]],"closed":true}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"fill":"FF0000","line":{"color":"00FF00","width_pt":3},"w":100,"h":100}));
    let s = shape(&doc, p, 0);
    assert_eq!(s["fill"]["color"], "FF0000");
    assert_eq!(s["line"]["color"], "00FF00");
    assert_eq!(s["w"], 100.0);
    // Still a freeform after the restyle.
    assert_eq!(s["geometry"], "custom");
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.shape_at(Pos::new(p, 0)).is_none());
}

// ---------------------------------------------------------------------------
// Edit points
// ---------------------------------------------------------------------------

#[test]
fn edit_points_replaces_a_preset_by_its_outline() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":100,"h":60,"floating":true,"x":20,"y":10}));
    // Word's "Edit points" on a rectangle: its four corners as a custom outline, here with the top right corner pulled out.
    ops(
        &mut doc,
        json!({"op":"set_shape_path","at":{"p":p,"o":0},"paths":[{"w":100,"h":60,"cmds":[
            {"t":"M","pts":[[0,0]]},{"t":"L","pts":[[130,0]]},{"t":"L","pts":[[100,60]]},{"t":"L","pts":[[0,60]]},{"t":"Z"}]}]}),
    );
    let s = shape(&doc, p, 0);
    assert_eq!(s["geometry"], "custom");
    assert_eq!(letters(&s), "MLLLZ");
    // The box follows the outline: 130 wide now, and the shape stays where it was.
    assert!((s["w"].as_f64().unwrap() - 130.0).abs() < 0.01);
    assert!((s["h"].as_f64().unwrap() - 60.0).abs() < 0.01);
    assert!((s["x"].as_f64().unwrap() - 20.0).abs() < 0.01);
    let xml = doc_xml(&mut doc);
    assert!(!xml.contains(r#"prst="rect""#));
    assert!(xml.contains("<a:custGeom>"));
    // The VML copy was a v:rect; it is a shape with the path now.
    assert!(!xml.contains("<v:rect"));
    assert!(xml.contains(r#"o:spt="100""#));
    assert!(doc.undo());
    assert_eq!(shape(&doc, p, 0)["geometry"], "rect");
}

#[test]
fn edit_points_follows_the_bounds_when_the_outline_moves_up_and_left() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0],[100,0],[100,50]],"closed":true,"x":50,"y":40}));
    let before = shape(&doc, p, 0);
    let (x0, y0) = (before["x"].as_f64().unwrap(), before["y"].as_f64().unwrap());
    let w0 = before["custom"]["paths"][0]["w"].as_f64().unwrap();
    let h0 = before["custom"]["paths"][0]["h"].as_f64().unwrap();
    let k = w0 / before["w"].as_f64().unwrap();
    // Move the first vertex 10 points left and 5 up (path units are EMU).
    let emu = 12700.0;
    let cmds = json!([
        {"t":"M","pts":[[-10.0 * emu, -5.0 * emu]]},
        {"t":"L","pts":[[100.0 * emu, 0.0]]},
        {"t":"L","pts":[[100.0 * emu, 50.0 * emu]]},
        {"t":"Z"}
    ]);
    ops(&mut doc, json!({"op":"set_shape_path","at":{"p":p,"o":0},"paths":[{"w":w0,"h":h0,"cmds":cmds}]}));
    let after = shape(&doc, p, 0);
    assert!((after["w"].as_f64().unwrap() - 110.0).abs() < 0.01, "{}", after["w"]);
    assert!((after["h"].as_f64().unwrap() - 55.0).abs() < 0.01, "{}", after["h"]);
    // The box grew to the left and up, so its corner moved by the same amount.
    assert!((after["x"].as_f64().unwrap() - (x0 - 10.0)).abs() < 0.01);
    assert!((after["y"].as_f64().unwrap() - (y0 - 5.0)).abs() < 0.01);
    assert!(k > 0.0);
    doc_xml(&mut doc);
}

#[test]
fn edit_points_on_a_turned_shape_keeps_the_drawing_in_place() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":100,"h":40,"floating":true,"x":100,"y":100}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"rot":90}));
    // The right edge moves out by 50 (in the shape's own direction).
    ops(&mut doc, json!({"op":"set_shape_path","at":{"p":p,"o":0},"paths":[{"w":100,"h":40,"cmds":[
        {"t":"M","pts":[[0,0]]},{"t":"L","pts":[[150,0]]},{"t":"L","pts":[[150,40]]},{"t":"L","pts":[[0,40]]},{"t":"Z"}]}]}));
    let s = shape(&doc, p, 0);
    assert!((s["w"].as_f64().unwrap() - 150.0).abs() < 0.01);
    assert_eq!(s["rot"], 90.0);
    // The old box was centred at (150, 120); a turn by 90 degrees sends the box's right (its +x) down the page,
    // so the new centre is 25 points further down: (150, 145).
    let (cx, cy) = (s["x"].as_f64().unwrap() + 75.0, s["y"].as_f64().unwrap() + 20.0);
    assert!((cx - 150.0).abs() < 0.05 && (cy - 145.0).abs() < 0.05, "{cx} {cy}");
}

#[test]
fn edit_points_keeps_the_text_of_the_shape() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"ellipse","w":100,"h":60,"text":"Inside"}));
    ops(&mut doc, json!({"op":"set_shape_path","at":{"p":p,"o":0},"paths":[{"w":100,"h":60,"cmds":[
        {"t":"M","pts":[[0,30]]},{"t":"C","pts":[[0,0],[100,0],[100,30]]},{"t":"C","pts":[[100,60],[0,60],[0,30]]},{"t":"Z"}]}]}));
    let s = shape(&doc, p, 0);
    assert_eq!(s["geometry"], "custom");
    assert_eq!(s["text"][0]["t"], "p");
    assert!(doc_xml(&mut doc).contains("Inside"));
}

#[test]
fn edit_points_is_checked() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":100,"h":60}));
    assert!(err(&mut doc, json!({"op":"set_shape_path","at":{"p":p,"o":0},"paths":[]})).contains("at least one path"));
    assert!(err(&mut doc, json!({"op":"set_shape_path","at":{"p":p,"o":0},"paths":[{"cmds":[{"t":"Q","pts":[[1,1]]}]}]})).contains("unknown path command"));
    assert!(err(&mut doc, json!({"op":"set_shape_path","at":{"p":p,"o":5},"paths":[{"cmds":[{"t":"M","pts":[[0,0]]},{"t":"L","pts":[[1,1]]}]}]})).contains("no shape"));
}

#[test]
fn quadratic_curves_and_arcs_read_back_as_cubic_commands() {
    let body = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1270000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="3" name="Pie"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="100" h="100"><a:moveTo><a:pt x="50" y="50"/></a:moveTo><a:lnTo><a:pt x="100" y="50"/></a:lnTo><a:arcTo wR="50" hR="50" stAng="0" swAng="5400000"/><a:quadBezTo><a:pt x="50" y="100"/><a:pt x="0" y="50"/></a:quadBezTo><a:close/></a:path></a:pathLst></a:custGeom></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice></mc:AlternateContent></w:r></w:p>"#;
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let ns = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word""#;
    let xml = xml.replacen("<w:document ", &format!("<w:document {ns} "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    pkg.set("word/document.xml", xml.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let p = first_para(&doc);
    let s = shape(&doc, p, 0);
    // M, L, then the quarter arc as one cubic curve, then the quadratic curve as a cubic one, then a close.
    assert_eq!(letters(&s), "MLCCZ");
    let arc = &s["custom"]["paths"][0]["cmds"][2]["pts"];
    // The arc starts at (100, 50) and ends at (50, 100), with handles a bit over half way round.
    assert!((arc[2][0].as_f64().unwrap() - 50.0).abs() < 1e-6 && (arc[2][1].as_f64().unwrap() - 100.0).abs() < 1e-6);
    assert!((arc[0][0].as_f64().unwrap() - 100.0).abs() < 1e-6 && arc[0][1].as_f64().unwrap() > 50.0);
    // The SVG data the renderer draws from is unchanged.
    assert!(s["custom"]["paths"][0]["d"].as_str().unwrap().starts_with("M50 50 L100 50 A"));
}

// ---------------------------------------------------------------------------
// Connectors
// ---------------------------------------------------------------------------

/// Two floating rectangles (100 by 60) in one paragraph, A at (0, 0) and B at (200, 100), at offsets 0 and 1.
fn two_boxes() -> (Document, u32) {
    let (mut doc, p) = blank_with_text("Hello");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":100,"h":60,"floating":true,"x":0,"y":0}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"rect","w":100,"h":60,"floating":true,"x":200,"y":100}));
    (doc, p)
}

fn near(a: &Value, b: f64) {
    let a = a.as_f64().unwrap_or_else(|| panic!("{a} is not a number"));
    assert!((a - b).abs() < 0.05, "{a} is not {b}");
}

/// The box of a connector: x, y, w, h.
fn boxed(s: &Value) -> (f64, f64, f64, f64) {
    (s["x"].as_f64().unwrap(), s["y"].as_f64().unwrap(), s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap())
}

fn connect_boxes(doc: &mut Document, p: u32, kind: &str, from_idx: usize, to_idx: usize) {
    ops(doc, json!({"op":"insert_connector","at":{"p":p,"o":2},"kind":kind,
        "start":{"shape":{"p":p,"o":0},"idx":from_idx},"end":{"shape":{"p":p,"o":1},"idx":to_idx}}));
}

#[test]
fn a_connector_between_two_shapes_is_glued_with_word_numbering() {
    let (mut doc, p) = two_boxes();
    let (a, b) = (shape(&doc, p, 0)["id"].as_u64().unwrap(), shape(&doc, p, 1)["id"].as_u64().unwrap());
    // A rectangle's sites are the top (0), left (1), bottom (2) and right (3): from the right of A to the left of B.
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector3");
    assert_eq!(c["connect"]["start"], json!({"id": a, "idx": 3}));
    assert_eq!(c["connect"]["end"], json!({"id": b, "idx": 1}));
    // Starts at A's right middle (100, 30) and ends at B's left middle (200, 130).
    assert_eq!(boxed(&c), (100.0, 30.0, 100.0, 100.0));
    assert!(c.get("rot").is_none() && c.get("flipH").is_none());
    assert_eq!(c["wrap"], "front");
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(&format!(r#"<a:stCxn id="{a}" idx="3"/>"#)));
    assert!(xml.contains(&format!(r#"<a:endCxn id="{b}" idx="1"/>"#)));
    assert!(xml.contains("<wps:cNvCnPr>"));
    assert!(xml.contains(r#"prst="bentConnector3""#));
    // The VML copy draws the same elbow.
    assert!(xml.contains(r#"o:connectortype="elbow""#));
}

#[test]
fn each_kind_of_connector_picks_its_preset_from_the_sides_it_joins() {
    let cases = [
        // Right of A to the left of B, straight on.
        ("straight", 3, 1, "straightConnector1"),
        ("elbow", 3, 1, "bentConnector3"),
        ("curved", 3, 1, "curvedConnector3"),
        // Right of A to the top of B: leave to the right, arrive going down: one bend.
        ("elbow", 3, 0, "bentConnector2"),
        ("curved", 3, 0, "curvedConnector2"),
        // Bottom of A to the top of B: down, across, down.
        ("elbow", 2, 0, "bentConnector3"),
        // Right of A to the right of B: round the far side.
        ("elbow", 3, 3, "bentConnector3"),
        // Bottom of A to the right of B: leaves down, arrives going left: a loop round B.
        ("elbow", 2, 3, "bentConnector4"),
        // Left of A to the left of B: out to the left and back in.
        ("elbow", 1, 1, "bentConnector3"),
        // Top of A to the left of B: it has to come back.
        ("elbow", 0, 1, "bentConnector4"),
        // Left of A to the right of B: round both.
        ("elbow", 1, 3, "bentConnector5"),
    ];
    for (kind, from, to, want) in cases {
        let (mut doc, p) = two_boxes();
        connect_boxes(&mut doc, p, kind, from, to);
        let c = shape(&doc, p, 2);
        assert_eq!(c["geometry"], want, "{kind} {from} to {to}");
        doc_xml(&mut doc);
    }
}

#[test]
fn a_vertical_elbow_is_turned_and_mirrored_the_way_word_writes_it() {
    let (mut doc, p) = two_boxes();
    // Bottom of A (50, 60) to the top of B (250, 100): down, across, down.
    connect_boxes(&mut doc, p, "elbow", 2, 0);
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector3");
    // The box is turned a quarter: 40 along (its width) and 200 across (its height), centred between the ends.
    near(&c["rot"], 270.0);
    assert_eq!(c["flipH"], true);
    let (x, y, w, h) = boxed(&c);
    assert!((w - 40.0).abs() < 0.05 && (h - 200.0).abs() < 0.05);
    assert!((x + w / 2.0 - 150.0).abs() < 0.05 && (y + h / 2.0 - 80.0).abs() < 0.05);
}

#[test]
fn an_elbow_that_climbs_is_flipped_down_as_word_writes_it() {
    let (mut doc, p) = blank_with_text("Hello");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":100,"h":60,"floating":true,"x":0,"y":100}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"rect","w":100,"h":60,"floating":true,"x":200,"y":0}));
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector3");
    // From A's right middle (100, 130) to B's left middle (200, 30): not turned, flipped down.
    assert_eq!(boxed(&c), (100.0, 30.0, 100.0, 100.0));
    assert_eq!(c["flipV"], true);
    assert!(c.get("rot").is_none() && c.get("flipH").is_none());
    assert!(doc_xml(&mut doc).contains(r#"<a:xfrm flipV="1">"#));
    // Move B so low that the connector no longer climbs: the flip goes.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"y":200}));
    let c = shape(&doc, p, 2);
    assert!(c.get("flipV").is_none(), "{c}");
}

#[test]
fn moving_a_shape_moves_the_end_of_its_connector_in_the_same_undo_step() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"x":300,"y":150}));
    let c = shape(&doc, p, 2);
    // B's left middle is now (300, 180); the start is still (100, 30).
    assert_eq!(boxed(&c), (100.0, 30.0, 200.0, 150.0));
    assert!(c.get("connect").is_some());
    // One undo takes back the move and the re-routing together.
    assert!(doc.undo());
    assert_eq!(boxed(&shape(&doc, p, 2)), (100.0, 30.0, 100.0, 100.0));
    assert_eq!(shape(&doc, p, 1)["x"], 200.0);
    // Redo puts both back.
    assert!(doc.redo());
    assert_eq!(boxed(&shape(&doc, p, 2)), (100.0, 30.0, 200.0, 150.0));
    doc_xml(&mut doc);
}

#[test]
fn resizing_turning_and_mirroring_a_shape_keep_the_connector_on_its_site() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    // Taller: the left middle of B is 20 lower.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"h":100}));
    assert_eq!(boxed(&shape(&doc, p, 2)), (100.0, 30.0, 100.0, 120.0));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"h":60}));
    // Turned a quarter: B's left side now faces up, and its middle is at (250, 80).
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"rot":90}));
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector2");
    near(&c["w"], 150.0);
    near(&c["h"], 50.0);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"rot":0}));
    // Mirrored: the left side is the right side, and the connector goes past B to reach it (past the end of its
    // box: the adjust value is over 100000).
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":1},"flip_h":true}));
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector3");
    assert!(c["adj"]["adj1"].as_f64().unwrap() > 100000.0, "{c}");
    near(&c["w"], 200.0);
    assert!(doc_xml(&mut doc).contains(r#"<a:gd name="adj1" fmla="val "#));
}

#[test]
fn deleting_a_shape_lets_go_of_its_connector_and_undo_glues_it_again() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    let a = shape(&doc, p, 0)["id"].as_u64().unwrap();
    let before = boxed(&shape(&doc, p, 2));
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":1},"to":{"p":p,"o":2}}));
    let c = shape(&doc, p, 1);
    // The connector stays where it was, glued only to A.
    assert_eq!(c["connect"], json!({"start": {"id": a, "idx": 3}}));
    assert_eq!(boxed(&c), before);
    let xml = doc_xml(&mut doc);
    assert!(xml.contains("<a:stCxn"));
    assert!(!xml.contains("<a:endCxn"));
    assert!(doc.undo());
    assert!(shape(&doc, p, 2)["connect"]["end"].is_object());
}

#[test]
fn grouping_and_ungrouping_the_shapes_keeps_the_connector_glued() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    // The connector is first in the paragraph now (offset 0), then the boxes: group the boxes.
    let (a, b) = (shape(&doc, p, 0)["id"].as_u64().unwrap(), shape(&doc, p, 1)["id"].as_u64().unwrap());
    assert!(a > 0 && b > 0);
    let before = boxed(&shape(&doc, p, 2));
    ops(&mut doc, json!({"op":"group_shapes","at":[{"p":p,"o":0},{"p":p,"o":1}]}));
    // Grouping alone moves nothing on the page: the connector is as it was, still glued to the ids the members kept.
    let c = shape(&doc, p, 1);
    assert_eq!(c["connect"]["start"]["id"].as_u64().unwrap(), a);
    assert_eq!(boxed(&c), before);
    let g = shape(&doc, p, 0);
    assert_eq!(g["kind"], "group");
    // Moving the group 50 points right moves both sites, and so both ends of the connector.
    let gx = g["x"].as_f64().unwrap();
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"x":gx + 50.0}));
    let moved = boxed(&shape(&doc, p, 1));
    assert!((moved.0 - (before.0 + 50.0)).abs() < 0.05, "{moved:?} {before:?}");
    assert!((moved.2 - before.2).abs() < 0.05 && (moved.3 - before.3).abs() < 0.05, "{moved:?} {before:?}");
    // Taking the group apart keeps the ids and the glue.
    ops(&mut doc, json!({"op":"ungroup_shape","at":{"p":p,"o":0}}));
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector3");
    assert!(c["connect"]["end"].is_object());
    doc_xml(&mut doc);
}

#[test]
fn moving_the_connector_itself_lets_go_of_the_shapes() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":2},"x":120,"y":50}));
    let c = shape(&doc, p, 2);
    assert_eq!(c["connect"], json!({}), "{c}");
    near(&c["x"], 120.0);
}

#[test]
fn a_connector_end_can_be_dragged_to_another_site_or_to_free_space() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    let b = shape(&doc, p, 1)["id"].as_u64().unwrap();
    // The end moves to B's top (250, 100).
    ops(&mut doc, json!({"op":"set_connector","at":{"p":p,"o":2},"end":{"shape":{"p":p,"o":1},"idx":0}}));
    let c = shape(&doc, p, 2);
    assert_eq!(c["geometry"], "bentConnector2");
    assert_eq!(c["connect"]["end"], json!({"id": b, "idx": 0}));
    // Dragged off the shape to a free point: it lets go.
    ops(&mut doc, json!({"op":"set_connector","at":{"p":p,"o":2},"end":"free","to":[400,200]}));
    let c = shape(&doc, p, 2);
    assert!(c["connect"].get("end").is_none());
    assert!(c["connect"].get("start").is_some());
    // Moving B no longer matters to the end, but the start still follows A.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"x":0,"y":20}));
    let moved = shape(&doc, p, 2);
    assert_ne!(boxed(&moved), boxed(&c));
    doc_xml(&mut doc);
}

#[test]
fn a_free_connector_between_points_and_its_flips() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_connector","at":{"p":p,"o":0},"kind":"straight","from":[100,100],"to":[40,50],"line":{"color":"FF0000","width_pt":2,"tail_arrow":"triangle"}}));
    let c = shape(&doc, p, 0);
    assert_eq!(c["geometry"], "straightConnector1");
    assert_eq!(boxed(&c), (40.0, 50.0, 60.0, 50.0));
    assert_eq!(c["flipH"], true);
    assert_eq!(c["flipV"], true);
    assert_eq!(c["line"]["tailArrow"], "triangle");
    assert_eq!(c["connect"], json!({}));
    assert!(err(&mut doc, json!({"op":"insert_connector","at":{"p":p,"o":0},"kind":"zigzag","from":[0,0],"to":[1,1]})).contains("unknown connector kind"));
    assert!(err(&mut doc, json!({"op":"insert_connector","at":{"p":p,"o":0},"kind":"elbow","from":[0,0]})).contains("where the connector ends"));
}

#[test]
fn connections_survive_a_save_and_open() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "curved", 3, 1);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let mut again = Document::open(&bytes).unwrap();
    let p2 = first_para(&again);
    let c = shape(&again, p2, 2);
    assert_eq!(c["geometry"], "curvedConnector3");
    assert!(c["connect"]["start"].is_object() && c["connect"]["end"].is_object());
    // The opened file still keeps the connector on its shape.
    ops(&mut again, json!({"op":"set_shape","at":{"p":p2,"o":1},"x":260,"y":140}));
    assert_eq!(boxed(&shape(&again, p2, 2)), (100.0, 30.0, 160.0, 140.0));
}

#[test]
fn a_connector_that_nothing_touched_is_not_rewritten() {
    let (mut doc, p) = two_boxes();
    connect_boxes(&mut doc, p, "elbow", 3, 1);
    let find = |xml: &str| {
        let i = xml.find("<wps:cNvCnPr>").unwrap();
        let start = xml[..i].rfind("<mc:AlternateContent>").unwrap();
        let end = i + xml[i..].find("</mc:AlternateContent>").unwrap();
        xml[start..end].to_string()
    };
    let before = find(&doc_xml(&mut doc));
    // Typing, and moving a shape that no connector holds, leave it byte for byte.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":3},"text":" more"}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"ellipse","w":50,"h":50,"floating":true,"x":400,"y":300}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"x":420,"y":310}));
    assert_eq!(find(&doc_xml(&mut doc)), before);
}

#[test]
fn connection_sites_of_the_presets_follow_word() {
    let (mut doc, p) = blank_with_text("Text");
    let cases = [("rect", 4), ("roundRect", 4), ("ellipse", 8), ("triangle", 6), ("diamond", 4), ("pentagon", 5), ("hexagon", 6), ("octagon", 8), ("rightArrow", 4), ("flowChartProcess", 4), ("flowChartDecision", 4), ("wedgeRectCallout", 5)];
    for (i, (prst, n)) in cases.iter().enumerate() {
        ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":i},"geometry":prst,"w":100,"h":60}));
        let sites = doc.shape_sites(Pos::new(p, i));
        assert_eq!(sites.len(), *n, "{prst}");
    }
    // A rectangle: top, left, bottom, right, with the way a connector leaves each.
    let rect = doc.shape_sites(Pos::new(p, 0));
    let got: Vec<(f64, f64, f64)> = rect.iter().map(|s| (s.x, s.y, s.ang)).collect();
    assert_eq!(got, vec![(50.0, 0.0, 270.0), (0.0, 30.0, 180.0), (50.0, 60.0, 90.0), (100.0, 30.0, 0.0)]);
    // A triangle: apex, left slope, bottom left, bottom middle, bottom right, right slope.
    let tri = doc.shape_sites(Pos::new(p, 3));
    let got: Vec<(f64, f64)> = tri.iter().map(|s| (s.x, s.y)).collect();
    assert_eq!(got, vec![(50.0, 0.0), (25.0, 30.0), (0.0, 60.0), (50.0, 60.0), (100.0, 60.0), (75.0, 30.0)]);
}

#[test]
fn the_assistant_can_draw_a_freeform_and_a_connector_through_ops() {
    let (mut doc, p) = two_boxes();
    let res = docxtor_core::ai::call_tool(&mut doc, "insert_freeform", &json!({"id": format!("p{p}"), "points": [[0, 0], [60, 0], [30, 50]], "closed": true, "fill": "FFF2CC", "x": 10, "y": 90})).unwrap();
    assert!(res.contains("Freeform inserted"), "{res}");
    // It went at the end of the paragraph, after "Hello" and the two boxes.
    let s = shape(&doc, p, 7);
    assert_eq!(s["geometry"], "custom");
    assert_eq!(s["fill"]["color"], "FFF2CC");
    assert!(docxtor_core::ai::call_tool(&mut doc, "insert_freeform", &json!({"id": format!("p{p}"), "points": [[0, 0]]})).is_err());
    // A connector goes through apply_ops with positions.
    let res = docxtor_core::ai::call_tool(
        &mut doc,
        "apply_ops",
        &json!({"ops": [{"op": "insert_connector", "at": {"p": p, "o": 2}, "kind": "straight", "start": {"shape": {"p": p, "o": 0}, "idx": 3}, "end": {"shape": {"p": p, "o": 1}, "idx": 1}}]}),
    );
    assert!(res.is_ok(), "{res:?}");
    assert!(shape(&doc, p, 2)["connect"]["start"].is_object());
}

#[test]
fn the_connector_presets_draw_the_same_lines_as_the_editor() {
    use docxtor_core::shapes::connector::preset_segs;
    use docxtor_core::shapes::freeform::Seg;
    use std::collections::BTreeMap;
    let adj = |v: &[(&str, f64)]| -> BTreeMap<String, f64> { v.iter().map(|(k, x)| (k.to_string(), *x)).collect() };
    // The same numbers packages/editor/test/path-edit.test.ts checks the TypeScript drawing against.
    assert_eq!(preset_segs("bentConnector2", &adj(&[]), 100.0, 60.0), vec![Seg::M((0.0, 0.0)), Seg::L((100.0, 0.0)), Seg::L((100.0, 60.0))]);
    assert_eq!(preset_segs("bentConnector3", &adj(&[("adj1", 118000.0)]), 100.0, 60.0), vec![Seg::M((0.0, 0.0)), Seg::L((118.0, 0.0)), Seg::L((118.0, 60.0)), Seg::L((100.0, 60.0))]);
    assert_eq!(
        preset_segs("bentConnector4", &adj(&[("adj2", 130000.0)]), 100.0, 60.0),
        vec![Seg::M((0.0, 0.0)), Seg::L((50.0, 0.0)), Seg::L((50.0, 78.0)), Seg::L((100.0, 78.0)), Seg::L((100.0, 60.0))]
    );
    assert_eq!(
        preset_segs("bentConnector5", &adj(&[("adj1", -18000.0), ("adj3", 118000.0)]), 100.0, 60.0).len(),
        6
    );
    assert_eq!(
        preset_segs("curvedConnector3", &adj(&[]), 100.0, 60.0),
        vec![Seg::M((0.0, 0.0)), Seg::C((25.0, 0.0), (50.0, 15.0), (50.0, 30.0)), Seg::C((50.0, 45.0), (75.0, 60.0), (100.0, 60.0))]
    );
    assert_eq!(preset_segs("curvedConnector2", &adj(&[]), 100.0, 60.0), vec![Seg::M((0.0, 0.0)), Seg::C((50.0, 0.0), (100.0, 30.0), (100.0, 60.0))]);
}

#[test]
fn a_freeform_has_a_site_at_each_corner() {
    let (mut doc, p) = blank_with_text("Text");
    ops(&mut doc, json!({"op":"insert_freeform","at":{"p":p,"o":0},"points":[[0,0],[100,0],[100,50]],"closed":true,"x":0,"y":0}));
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"rect","w":40,"h":40,"floating":true,"x":300,"y":0}));
    let sites = doc.shape_sites(Pos::new(p, 0));
    assert_eq!(sites.len(), 3);
    assert!((sites[1].x - 100.0).abs() < 0.01 && sites[1].y.abs() < 0.01);
    ops(&mut doc, json!({"op":"insert_connector","at":{"p":p,"o":2},"kind":"straight","start":{"shape":{"p":p,"o":0},"idx":2},"end":{"shape":{"p":p,"o":1},"idx":1}}));
    let c = shape(&doc, p, 2);
    assert!(c["connect"]["start"].is_object());
    // From the bottom corner (100, 50) of the triangle to B's left middle (300, 20).
    assert_eq!(boxed(&c), (100.0, 20.0, 200.0, 30.0));
    assert_eq!(c["flipV"], true);
}

// ---------------------------------------------------------------------------
// Connectors Word made inside a canvas or a group
// ---------------------------------------------------------------------------

const NS: &str = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" mc:Ignorable="wp14""#;

fn open_with(body: &str) -> Document {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    assert!(xml.contains(body), "the body was not replaced");
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

/// A member shape: its id, name, box in the container's own units (EMU), and what a connector there is glued to.
fn member(id: u32, name: &str, off: (i64, i64), ext: (i64, i64), prst: &str, glue: Option<((u32, u32), (u32, u32))>) -> String {
    let cnv = match glue {
        Some(((a, ai), (b, bi))) => format!(r#"<wps:cNvCnPr><a:stCxn id="{a}" idx="{ai}"/><a:endCxn id="{b}" idx="{bi}"/></wps:cNvCnPr>"#),
        None => "<wps:cNvSpPr/>".into(),
    };
    format!(
        r#"<wps:wsp><wps:cNvPr id="{id}" name="{name}"/>{cnv}<wps:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="{prst}"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp>"#,
        off.0, off.1, ext.0, ext.1
    )
}

/// A canvas in line with the text: A (20, 20) and B (240, 120), 100 by 50 points each, and a connector between them.
fn canvas_with_connector() -> Document {
    let pt = |v: i64| v * 12700;
    let members = member(10, "A", (pt(20), pt(20)), (pt(100), pt(50)), "rect", None)
        + &member(11, "B", (pt(240), pt(120)), (pt(100), pt(50)), "rect", None)
        + &member(12, "C", (pt(120), pt(45)), (pt(120), pt(100)), "bentConnector3", Some(((10, 3), (11, 1))));
    open_with(&format!(
        r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpc"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="9" name="Canvas 9"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"><wpc:wpc><wpc:bg><a:noFill/></wpc:bg><wpc:whole><a:ln><a:noFill/></a:ln></wpc:whole>{members}</wpc:wpc></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice></mc:AlternateContent></w:r></w:p>"#
    ))
}

#[test]
fn a_connector_between_shapes_of_a_canvas_follows_them() {
    let mut doc = canvas_with_connector();
    let p = first_para(&doc);
    let canvas = shape(&doc, p, 0);
    assert_eq!(canvas["children"][2]["connect"], json!({"start": {"id": 10, "idx": 3}, "end": {"id": 11, "idx": 1}}));
    assert_eq!(canvas["children"][2]["geometry"], "bentConnector3");
    // Move B 30 points right and 20 down, inside the canvas.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[1],"x":270,"y":140}));
    let canvas = shape(&doc, p, 0);
    let c = &canvas["children"][2];
    assert_eq!((c["childX"].as_f64().unwrap(), c["childY"].as_f64().unwrap()), (120.0, 45.0));
    assert!((c["w"].as_f64().unwrap() - 150.0).abs() < 0.05, "{c}");
    assert!((c["h"].as_f64().unwrap() - 120.0).abs() < 0.05, "{c}");
    assert!(c["connect"]["end"].is_object());
    // The shape in the middle of the same canvas, which no connector holds, is not touched by re-routing.
    assert_eq!(canvas["children"][0]["w"], 100.0);
    doc_xml(&mut doc);
    assert!(doc.undo());
    assert_eq!(shape(&doc, p, 0)["children"][2]["w"], 120.0);
}

/// A group in line with the text, at half scale (the members count in twice the group's size): A, B and a connector.
fn group_with_connector() -> Document {
    let pt = |v: i64| v * 12700;
    // In the group's own units (twice the points): A at (0, 0) 200 by 100, B at (280, 100) 200 by 100.
    let members = member(6, "A", (0, 0), (pt(200), pt(100)), "rect", None)
        + &member(7, "B", (pt(280), pt(100)), (pt(200), pt(100)), "rect", None)
        + &member(8, "C", (pt(200), pt(50)), (pt(80), pt(100)), "bentConnector3", Some(((6, 3), (7, 1))));
    open_with(&format!(
        r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3048000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="5" name="Group 5"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3048000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="6096000" cy="2540000"/></a:xfrm></wpg:grpSpPr>{members}</wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice></mc:AlternateContent></w:r></w:p>"#
    ))
}

#[test]
fn a_connector_inside_a_scaled_group_follows_its_members_in_the_groups_units() {
    let mut doc = group_with_connector();
    let p = first_para(&doc);
    let g = shape(&doc, p, 0);
    // The group draws its members at half size: B is 100 by 50 points, 140 right of the group's corner.
    assert_eq!(g["children"][1]["childX"], 140.0);
    assert_eq!(g["children"][2]["connect"]["start"], json!({"id": 6, "idx": 3}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"member":[1],"x":170,"y":60}));
    let g = shape(&doc, p, 0);
    let c = &g["children"][2];
    // A's right middle is (100, 25) on the page of the group; B's left middle is now (170, 85).
    assert!((c["w"].as_f64().unwrap() - 70.0).abs() < 0.1, "{c}");
    assert!((c["h"].as_f64().unwrap() - 60.0).abs() < 0.1, "{c}");
    assert_eq!(c["geometry"], "bentConnector3");
    doc_xml(&mut doc);
}

#[test]
fn a_connector_on_its_own_follows_a_member_of_a_group_that_moves() {
    // A floating group with two shapes, and a floating connector glued to one of them.
    let pt = |v: i64| v * 12700;
    let members = member(6, "A", (0, 0), (pt(100), pt(50)), "rect", None) + &member(7, "B", (pt(140), pt(60)), (pt(100), pt(50)), "rect", None);
    let group = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{}" cy="{}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="5" name="Group 5"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{}" cy="{}"/><a:chOff x="0" y="0"/><a:chExt cx="{}" cy="{}"/></a:xfrm></wpg:grpSpPr>{members}</wpg:wgp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice></mc:AlternateContent>"#,
        pt(100),
        pt(300),
        pt(240),
        pt(110),
        pt(240),
        pt(110),
        pt(240),
        pt(110)
    );
    // A connector on the page from B's left middle (240, 385) to a point at (400, 385), glued to B only.
    let conn = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="251660288" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{}" cy="0"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="20" name="Connector 20"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvCnPr><a:stCxn id="7" idx="1"/></wps:cNvCnPr><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{}" cy="0"/></a:xfrm><a:prstGeom prst="straightConnector1"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice></mc:AlternateContent>"#,
        pt(240),
        pt(385),
        pt(160),
        pt(160)
    );
    let mut doc = open_with(&format!("<w:p><w:r>{group}</w:r><w:r>{conn}</w:r></w:p>"));
    let p = first_para(&doc);
    assert!(shape(&doc, p, 1)["connect"]["start"].is_object());
    // Moving the whole group 50 points right and 20 down takes B's site with it: the glued start moves, the free end stays.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"x":150,"y":320}));
    let c = shape(&doc, p, 1);
    // The start is B's left middle, now (290, 405); the far end is still at (400, 385): a line up and to the right.
    assert_eq!(boxed(&c), (290.0, 385.0, 110.0, 20.0));
    assert_eq!(c["flipV"], true);
    assert!(c["connect"]["start"].is_object());
    doc_xml(&mut doc);
}

#[test]
fn two_connectors_sharing_a_drawing_id_stay_glued_through_an_unrelated_edit() {
    let pt = |v: i64| v * 12700;
    let anchor = |id: u32, name: &str, x: i64, y: i64, w: i64, inner: &str| {
        format!(
            r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="251660288" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{}" cy="{}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">{inner}</a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice></mc:AlternateContent>"#,
            pt(x),
            pt(y),
            pt(w),
            pt(if inner.contains("cNvCnPr") { 0 } else { 50 }),
        )
    };
    let shape_xml = |w: i64| format!(r#"<wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp>"#, pt(w), pt(50));
    let conn_xml = |w: i64| format!(r#"<wps:wsp><wps:cNvCnPr><a:stCxn id="7" idx="1"/></wps:cNvCnPr><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{}" cy="0"/></a:xfrm><a:prstGeom prst="straightConnector1"><a:avLst/></a:prstGeom></wps:spPr><wps:bodyPr/></wps:wsp>"#, pt(w));
    // A shape (7) and two connectors that both carry drawing id 5, as files from other producers do, both glued to it.
    let body = format!(
        "<w:p><w:r>{}</w:r><w:r>{}</w:r><w:r>{}</w:r></w:p><w:p><w:r><w:t>Text</w:t></w:r></w:p>",
        anchor(7, "Box", 100, 100, 100, &shape_xml(100)),
        anchor(5, "Line A", 100, 125, 80, &conn_xml(80)),
        anchor(5, "Line B", 100, 300, 160, &conn_xml(160))
    );
    let mut doc = open_with(&body);
    let before = doc_xml(&mut doc);
    assert_eq!(before.matches("<a:stCxn").count(), 2);
    // Typing elsewhere changes nothing about the shapes.
    let second = doc.render_snapshot().order[1];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":second,"o":4},"text":" more"}));
    let after = doc_xml(&mut doc);
    assert_eq!(after.matches("<a:stCxn").count(), 2, "an unrelated edit let go of a connector: {after}");
}
