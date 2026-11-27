//! SmartArt drawn from the drawing Word saves with it (and laid out from the data model when it is missing),
//! and WordArt (DrawingML `a:prstTxWarp` and VML `v:textpath`) read into the view model.

use docxtor_core::ai::call_tool;
use docxtor_core::package::{Package, REL_THEME};
use docxtor_core::render::{RBlock, RSeg};
use docxtor_core::{Document, Pos};
use serde_json::{json, Value};

const NS: &str = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" mc:Ignorable="w14 wp14""#;

const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";
const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_DRAWING: &str = "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing";

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"/><a:ln w="12700"/><a:ln w="19050"/></a:lnStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    Package::open(bytes).unwrap().get(name).unwrap_or_else(|| panic!("no part {name}")).to_vec()
}

fn text(bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(bytes, name)).unwrap()
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn segs(doc: &Document) -> Vec<RSeg> {
    let mut out = Vec::new();
    for b in &doc.render_snapshot().blocks {
        if let RBlock::Para(p) = b {
            out.extend(p.segs.iter().cloned());
        }
    }
    out
}

fn first_para(doc: &Document) -> u32 {
    doc.render_snapshot().order[0]
}

fn shape(doc: &Document) -> Value {
    serde_json::to_value(doc.shape_at(Pos::new(first_para(doc), 0)).expect("a shape at the start")).unwrap()
}

// ---------------------------------------------------------------------------
// Building documents
// ---------------------------------------------------------------------------

fn package_with_body(body: &str) -> Package {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    assert!(xml.contains(body), "the body was not replaced");
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.add_rel("word/document.xml", REL_THEME, "theme/theme1.xml", false);
    pkg.set("word/theme/theme1.xml", THEME.as_bytes().to_vec());
    pkg.ensure_override("word/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml");
    pkg
}

fn open_body(body: &str) -> Document {
    Document::open(&package_with_body(body).to_bytes().unwrap()).unwrap()
}

/// A paragraph holding a SmartArt graphic the way Word writes one: an inline drawing with `dgm:relIds`.
fn diagram_paragraph(ids: [&str; 4], extent: (u32, u32), docpr: &str, anchor: bool) -> String {
    let [dm, lo, qs, cs] = ids;
    let rel_ids = format!(r#"<dgm:relIds xmlns:dgm="{DGM}" xmlns:r="{REL_BASE}" r:dm="{dm}" r:lo="{lo}" r:qs="{qs}" r:cs="{cs}"/>"#);
    let pre = format!(r#"<wp:extent cx="{}" cy="{}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>"#, extent.0, extent.1);
    let post = format!(r#"{docpr}<wp:cNvGraphicFramePr/><a:graphic xmlns:a="{A}"><a:graphicData uri="{DGM}">{rel_ids}</a:graphicData></a:graphic>"#);
    let drawing = if anchor {
        format!(r#"<wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>914400</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>182880</wp:posOffset></wp:positionV>{pre}<wp:wrapSquare wrapText="bothSides"/>{post}</wp:anchor>"#)
    } else {
        format!(r#"<wp:inline distT="0" distB="0" distL="0" distR="0">{pre}{post}</wp:inline>"#)
    };
    format!(r#"<w:p><w:r><w:rPr><w:noProof/></w:rPr><w:drawing>{drawing}</w:drawing></w:r></w:p><w:p><w:r><w:t>After the graphic</w:t></w:r></w:p>"#)
}

fn data_model(nodes: &[&str], layout: &str, colors: &str, drawing_rid: Option<&str>) -> String {
    let mut pts = format!(
        r#"<dgm:pt modelId="{{DOC}}" type="doc"><dgm:prSet loTypeId="urn:microsoft.com/office/officeart/2005/8/layout/{layout}" loCatId="list" qsTypeId="urn:microsoft.com/office/officeart/2005/8/quickstyle/simple1" qsCatId="simple" csTypeId="urn:microsoft.com/office/officeart/2005/8/colors/{colors}" csCatId="accent1" phldr="0"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></dgm:t></dgm:pt>"#
    );
    let mut cxns = String::new();
    for (i, n) in nodes.iter().enumerate() {
        pts.push_str(&format!(r#"<dgm:pt modelId="{{N{i}}}"><dgm:prSet phldrT="[Text]"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" dirty="0"/><a:t>{n}</a:t></a:r></a:p></dgm:t></dgm:pt>"#));
        pts.push_str(&format!(r#"<dgm:pt modelId="{{P{i}}}" type="parTrans" cxnId="{{C{i}}}"><dgm:prSet/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></dgm:t></dgm:pt>"#));
        cxns.push_str(&format!(r#"<dgm:cxn modelId="{{C{i}}}" srcId="{{DOC}}" destId="{{N{i}}}" srcOrd="{i}" destOrd="0" parTransId="{{P{i}}}" sibTransId="{{S{i}}}"/>"#));
    }
    let ext = drawing_rid
        .map(|r| format!(r#"<dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="{DSP}" relId="{r}" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst>"#))
        .unwrap_or_default();
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:dataModel xmlns:dgm="{DGM}" xmlns:a="{A}"><dgm:ptLst>{pts}</dgm:ptLst><dgm:cxnLst>{cxns}</dgm:cxnLst><dgm:bg/><dgm:whole/>{ext}</dgm:dataModel>"#)
}

fn layout_part(name: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:layoutDef xmlns:dgm="{DGM}" xmlns:a="{A}" uniqueId="urn:microsoft.com/office/officeart/2005/8/layout/{name}"><dgm:title val=""/><dgm:desc val=""/><dgm:layoutNode name="diagram"><dgm:alg type="snake"/></dgm:layoutNode></dgm:layoutDef>"#)
}

fn style_part() -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:styleDef xmlns:dgm="{DGM}" xmlns:a="{A}" uniqueId="urn:microsoft.com/office/officeart/2005/8/quickstyle/simple1"><dgm:title val=""/><dgm:desc val=""/></dgm:styleDef>"#)
}

fn colors_part(accent: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:colorsDef xmlns:dgm="{DGM}" xmlns:a="{A}" uniqueId="urn:microsoft.com/office/officeart/2005/8/colors/accent1_2"><dgm:title val=""/><dgm:desc val=""/><dgm:styleLbl name="node1"><dgm:fillClrLst meth="repeat"><a:schemeClr val="{accent}"/></dgm:fillClrLst><dgm:linClrLst meth="repeat"><a:schemeClr val="lt1"/></dgm:linClrLst><dgm:effectClrLst/><dgm:txLinClrLst/><dgm:txFillClrLst/><dgm:txEffectClrLst/></dgm:styleLbl></dgm:colorsDef>"#
    )
}

/// The drawing Word saves for a basic block list of three nodes in a 432 by 252 point frame.
fn block_list_drawing() -> String {
    let sp = |i: usize, x: i64, label: &str| {
        format!(
            r#"<dsp:sp modelId="{{S{i}}}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="{x}" y="1085850"/><a:ext cx="1714500" cy="1028700"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:schemeClr val="accent1"><a:hueOff val="0"/><a:satOff val="0"/><a:lumOff val="0"/><a:alphaOff val="0"/></a:schemeClr></a:solidFill><a:ln w="12700" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="lt1"><a:hueOff val="0"/><a:satOff val="0"/><a:lumOff val="0"/><a:alphaOff val="0"/></a:schemeClr></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln><a:effectLst/></dsp:spPr><dsp:style><a:lnRef idx="2"><a:scrgbClr r="0" g="0" b="0"/></a:lnRef><a:fillRef idx="1"><a:scrgbClr r="0" g="0" b="0"/></a:fillRef><a:effectRef idx="0"><a:scrgbClr r="0" g="0" b="0"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></dsp:style><dsp:txBody><a:bodyPr spcFirstLastPara="0" vert="horz" wrap="square" lIns="106680" tIns="106680" rIns="106680" bIns="106680" numCol="1" spcCol="1270" anchor="ctr" anchorCtr="0"><a:noAutofit/></a:bodyPr><a:lstStyle/><a:p><a:pPr marL="0" lvl="0" indent="0" algn="ctr" defTabSz="1244600"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPct val="0"/></a:spcBef><a:spcAft><a:spcPct val="35000"/></a:spcAft><a:buNone/></a:pPr><a:r><a:rPr lang="en-US" sz="2800" b="1" kern="1200" dirty="0"/><a:t>{label}</a:t></a:r></a:p></dsp:txBody><dsp:txXfrm><a:off x="{x}" y="1085850"/><a:ext cx="1714500" cy="1028700"/></dsp:txXfrm></dsp:sp>"#
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dsp:drawing xmlns:dgm="{DGM}" xmlns:dsp="{DSP}" xmlns:a="{A}"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>{}{}{}</dsp:spTree></dsp:drawing>"#,
        sp(0, 0, "Alpha"),
        sp(1, 1885950, "Beta"),
        sp(2, 3771900, "Gamma")
    )
}

struct Spec<'a> {
    nodes: &'a [&'a str],
    layout: &'a str,
    drawing: Option<String>,
    colors: &'a str,
    /// The accent the colours part gives nodes.
    accent: &'a str,
    extent: (u32, u32),
    anchor: bool,
}

impl<'a> Default for Spec<'a> {
    fn default() -> Self {
        Spec { nodes: &["Alpha", "Beta", "Gamma"], layout: "default", drawing: Some(block_list_drawing()), colors: "accent1_2", accent: "accent1", extent: (5486400, 3200400), anchor: false }
    }
}

fn smartart_docx(s: &Spec) -> Vec<u8> {
    let docpr = r#"<wp:docPr id="1" name="Diagram 1" descr="Three words in boxes"/>"#;
    let mut pkg = package_with_body("<w:p/>");
    let drawing_rid = s.drawing.as_ref().map(|d| {
        pkg.set("word/diagrams/drawing1.xml", d.as_bytes().to_vec());
        pkg.ensure_override("word/diagrams/drawing1.xml", "application/vnd.ms-office.drawingml.diagramDrawing+xml");
        pkg.add_rel("word/document.xml", REL_DRAWING, "diagrams/drawing1.xml", false)
    });
    pkg.set("word/diagrams/data1.xml", data_model(s.nodes, s.layout, s.colors, drawing_rid.as_deref()).into_bytes());
    pkg.set("word/diagrams/layout1.xml", layout_part(s.layout).into_bytes());
    pkg.set("word/diagrams/quickStyle1.xml", style_part().into_bytes());
    pkg.set("word/diagrams/colors1.xml", colors_part(s.accent).into_bytes());
    pkg.ensure_override("word/diagrams/data1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml");
    pkg.ensure_override("word/diagrams/layout1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramLayout+xml");
    pkg.ensure_override("word/diagrams/quickStyle1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramStyle+xml");
    pkg.ensure_override("word/diagrams/colors1.xml", "application/vnd.openxmlformats-officedocument.drawingml.diagramColors+xml");
    let dm = pkg.add_rel("word/document.xml", &format!("{REL_BASE}/diagramData"), "diagrams/data1.xml", false);
    let lo = pkg.add_rel("word/document.xml", &format!("{REL_BASE}/diagramLayout"), "diagrams/layout1.xml", false);
    let qs = pkg.add_rel("word/document.xml", &format!("{REL_BASE}/diagramQuickStyle"), "diagrams/quickStyle1.xml", false);
    let cs = pkg.add_rel("word/document.xml", &format!("{REL_BASE}/diagramColors"), "diagrams/colors1.xml", false);
    let body = diagram_paragraph([&dm, &lo, &qs, &cs], s.extent, docpr, s.anchor);
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap().replacen("<w:p/>", &body, 1);
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.to_bytes().unwrap()
}

fn open_smartart(s: &Spec) -> Document {
    Document::open(&smartart_docx(s)).unwrap()
}

fn num(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("no number {k} in {v}"))
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 0.05, "{a} is not {b}");
}

fn children(v: &Value) -> &Vec<Value> {
    v["children"].as_array().expect("children")
}

fn run_texts(c: &Value) -> Vec<String> {
    c["dtext"]["paras"].as_array().map(|ps| ps.iter().map(|p| p["runs"].as_array().unwrap().iter().map(|r| r["text"].as_str().unwrap_or("")).collect::<String>()).collect()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// SmartArt drawn from Word's drawing
// ---------------------------------------------------------------------------

#[test]
fn a_smartart_graphic_is_a_diagram_shape_with_the_shapes_of_its_drawing() {
    let doc = open_smartart(&Spec::default());
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "diagram");
    assert_eq!(sh["diagram"]["source"], "drawing");
    assert_eq!(sh["diagram"]["layout"], "default");
    approx(num(&sh, "w"), 432.0);
    approx(num(&sh, "h"), 252.0);
    assert_eq!(sh["alt"], "Three words in boxes");
    assert_eq!(sh["wrap"], "inline");
    let kids = children(&sh);
    assert_eq!(kids.len(), 3);
    // Positions and sizes are points inside the frame.
    approx(num(&kids[0], "childX"), 0.0);
    approx(num(&kids[1], "childX"), 148.5);
    approx(num(&kids[2], "childX"), 297.0);
    for k in kids {
        approx(num(k, "childY"), 85.5);
        approx(num(k, "w"), 135.0);
        approx(num(k, "h"), 81.0);
        assert_eq!(k["geometry"], "rect");
        // The scheme colour resolves against the theme; the outline is the style's white.
        assert_eq!(k["fill"]["color"], "4472C4");
        assert_eq!(k["line"]["color"], "FFFFFF");
        approx(num(&k["line"], "widthPt"), 1.0);
    }
    // The text is read only: size, weight, the colour from the style's font reference, and the box it sits in.
    assert_eq!(run_texts(&kids[0]), ["Alpha"]);
    assert_eq!(run_texts(&kids[2]), ["Gamma"]);
    let run = &kids[1]["dtext"]["paras"][0]["runs"][0];
    assert_eq!(run["text"], "Beta");
    approx(num(run, "size"), 28.0);
    assert_eq!(run["bold"], true);
    assert_eq!(run["color"], "FFFFFF");
    // The font is the theme's body font, which the shape's style refers to.
    assert_eq!(run["font"], "Calibri");
    assert_eq!(kids[1]["dtext"]["paras"][0]["align"], "center");
    assert_eq!(kids[1]["dtext"]["vAnchor"], "middle");
    approx(num(&kids[1]["dtext"]["insets"], "l"), 8.4);
    approx(num(&kids[1]["dtext"], "w"), 135.0);
    // The paragraph's segment is labelled for screen readers and the chip fallback.
    let seg = segs(&doc).into_iter().find(|s| s.k == "shape").unwrap();
    assert_eq!(seg.label.as_deref(), Some("SmartArt graphic"));
}

#[test]
fn a_floating_smartart_graphic_is_placed_like_a_picture() {
    let doc = open_smartart(&Spec { anchor: true, ..Default::default() });
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "diagram");
    assert_eq!(sh["wrap"], "square");
    approx(num(&sh, "x"), 72.0);
    approx(num(&sh, "y"), 14.4);
    assert_eq!(children(&sh).len(), 3);
}

#[test]
fn saving_keeps_every_diagram_part_byte_for_byte() {
    let bytes = smartart_docx(&Spec::default());
    let mut doc = Document::open(&bytes).unwrap();
    // An edit elsewhere in the document.
    let after = doc.render_snapshot().order[1];
    ops(&mut doc, json!({"op": "insert_text", "at": {"p": after, "o": 0}, "text": "Typed "}));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    for name in ["word/diagrams/data1.xml", "word/diagrams/layout1.xml", "word/diagrams/quickStyle1.xml", "word/diagrams/colors1.xml", "word/diagrams/drawing1.xml"] {
        assert_eq!(part(&saved, name), part(&bytes, name), "{name} changed");
    }
    let xml = text(&saved, "word/document.xml");
    assert!(xml.contains("dgm:relIds") && xml.contains("Typed "));
    // The relationships and content types of the parts are kept.
    let rels = text(&saved, "word/_rels/document.xml.rels");
    for t in ["diagramData", "diagramLayout", "diagramQuickStyle", "diagramColors", "diagramDrawing"] {
        assert!(rels.contains(t), "{t} relationship lost");
    }
    let types = text(&saved, "[Content_Types].xml");
    assert!(types.contains("diagramDrawing+xml"));
    // And an untouched document is written back as it was.
    let again = Document::open(&bytes).unwrap().save().unwrap();
    assert_eq!(part(&again, "word/document.xml"), part(&bytes, "word/document.xml"));
}

#[test]
fn resizing_scales_the_drawing_and_leaves_the_parts_alone() {
    let bytes = smartart_docx(&Spec::default());
    let mut doc = Document::open(&bytes).unwrap();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "w": 216, "h": 252}));
    let sh = shape(&doc);
    approx(num(&sh, "w"), 216.0);
    let kids = children(&sh);
    // Half the width: the second box starts at half of where it did, and is half as wide.
    approx(num(&kids[1], "childX"), 74.25);
    approx(num(&kids[1], "w"), 67.5);
    approx(num(&kids[1], "h"), 81.0);
    // A second resize counts from the size the drawing was made for, not from the last one.
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "w": 864, "h": 504}));
    let sh = shape(&doc);
    let kids = children(&sh);
    approx(num(&kids[1], "childX"), 297.0);
    approx(num(&kids[1], "w"), 270.0);
    approx(num(&kids[1], "h"), 162.0);
    approx(num(&kids[1], "childY"), 171.0);
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    for name in ["word/diagrams/data1.xml", "word/diagrams/drawing1.xml"] {
        assert_eq!(part(&saved, name), part(&bytes, name), "{name} changed");
    }
    let xml = text(&saved, "word/document.xml");
    assert!(xml.contains(r#"<wp:extent cx="10972800" cy="6400800"/>"#), "the extent is the new size");
    assert_eq!(xml.matches("https://docxtor.tinycommit.dev/ns/smartart").count(), 1, "the original size is noted once");
    // Undo puts the old size back (and the note with it).
    assert!(doc.undo());
    assert!(doc.undo());
    let sh = shape(&doc);
    approx(num(&sh, "w"), 432.0);
    approx(num(&children(&sh)[1], "childX"), 148.5);
    assert!(!text(&doc.save().unwrap(), "word/document.xml").contains("docxtor.dev"));
}

#[test]
fn a_size_noted_by_docxtor_is_dropped_when_word_resizes_the_graphic_afterwards() {
    let mut doc = open_smartart(&Spec::default());
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "w": 216, "h": 252}));
    let saved = doc.save().unwrap();
    // The note is there: the drawing is scaled to the 216 points.
    approx(num(&children(&shape(&Document::open(&saved).unwrap()))[1], "childX"), 74.25);
    // Word makes the graphic 288 points wide and writes the drawing again for that size; the drawing part here stands
    // for it, and the note in the file still says 432.
    let mut pkg = Package::open(&saved).unwrap();
    let xml = text(&saved, "word/document.xml");
    assert!(xml.contains(r#"<wp:extent cx="2743200" cy="3200400"/>"#), "{xml}");
    pkg.set("word/document.xml", xml.replace(r#"<wp:extent cx="2743200" cy="3200400"/>"#, r#"<wp:extent cx="3657600" cy="3200400"/>"#).into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let sh = shape(&doc);
    approx(num(&sh, "w"), 288.0);
    // The drawing is for 288 points: it is drawn as it is, not scaled by 288 / 432 again.
    approx(num(&children(&sh)[1], "childX"), 148.5);
    approx(num(&children(&sh)[1], "w"), 135.0);
    // Resizing from here counts from the drawing as Word made it.
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "w": 144, "h": 252}));
    approx(num(&children(&shape(&doc))[1], "childX"), 74.25);
}

#[test]
fn a_smartart_model_whose_connections_loop_or_fan_out_is_read_within_limits() {
    // Every node lists every node as a child: 20 nodes and 400 connections. Read without bounds this makes billions of nodes.
    let ids: Vec<String> = (0..20).map(|i| format!("{{N{i}}}")).collect();
    let pts: String = ids.iter().enumerate().map(|(i, id)| format!(r#"<dgm:pt modelId="{id}"><dgm:prSet/><dgm:spPr/><dgm:t><a:bodyPr/><a:p><a:r><a:t>Node {i}</a:t></a:r></a:p></dgm:t></dgm:pt>"#)).collect();
    let cxns: String = ids.iter().flat_map(|a| ids.iter().map(move |b| format!(r#"<dgm:cxn modelId="{{C{a}{b}}}" srcId="{a}" destId="{b}" srcOrd="0" destOrd="0"/>"#))).collect();
    let data = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><dgm:dataModel xmlns:dgm="{DGM}" xmlns:a="{A}"><dgm:ptLst><dgm:pt modelId="{{DOC}}" type="doc"><dgm:prSet loTypeId="urn:microsoft.com/office/officeart/2005/8/layout/default"/><dgm:spPr/><dgm:t><a:bodyPr/><a:p><a:endParaRPr lang="en-US"/></a:p></dgm:t></dgm:pt>{pts}</dgm:ptLst><dgm:cxnLst>{cxns}{}</dgm:cxnLst></dgm:dataModel>"#, ids.iter().map(|id| format!(r#"<dgm:cxn modelId="{{D{id}}}" srcId="{{DOC}}" destId="{id}" srcOrd="0" destOrd="0"/>"#)).collect::<String>());
    // No drawing part: the shapes are laid out from the data model, which is where the nodes are walked.
    let mut pkg = Package::open(&smartart_docx(&Spec { drawing: None, ..Default::default() })).unwrap();
    pkg.set("word/diagrams/data1.xml", data.into_bytes());
    let started = std::time::Instant::now();
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "diagram");
    assert!(started.elapsed().as_secs() < 20, "reading the model took {:?}", started.elapsed());
}

#[test]
fn a_smartart_graphic_can_be_moved_and_deleted_like_a_picture() {
    let mut doc = open_smartart(&Spec::default());
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "wrap": "square", "x": 36, "y": 10}));
    let sh = shape(&doc);
    assert_eq!(sh["wrap"], "square");
    approx(num(&sh, "x"), 36.0);
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "alt": "A process of three steps"}));
    assert_eq!(shape(&doc)["alt"], "A process of three steps");
    ops(&mut doc, json!({"op": "delete", "from": {"p": p, "o": 0}, "to": {"p": p, "o": 1}}));
    assert!(doc.shape_at(Pos::new(p, 0)).is_none());
    assert_valid(&doc.save().unwrap());
}

// ---------------------------------------------------------------------------
// SmartArt laid out from the data model
// ---------------------------------------------------------------------------

fn texts_of(sh: &Value) -> Vec<String> {
    children(sh).iter().flat_map(run_texts).collect()
}

#[test]
fn without_a_drawing_a_basic_block_list_is_laid_out_from_the_nodes() {
    let doc = open_smartart(&Spec { drawing: None, nodes: &["One", "Two", "Three", "Four", "Five"], ..Default::default() });
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "diagram");
    assert_eq!(sh["diagram"]["source"], "layout");
    let kids = children(&sh);
    assert_eq!(kids.len(), 5);
    assert_eq!(texts_of(&sh), ["One", "Two", "Three", "Four", "Five"]);
    // Boxes 0.6 as tall as wide, three on the first row, the last two centred.
    let w = num(&kids[0], "w");
    approx(num(&kids[0], "h"), w * 0.6);
    approx(num(&kids[0], "childY"), num(&kids[2], "childY"));
    assert!(num(&kids[3], "childY") > num(&kids[0], "childY"));
    approx(num(&kids[3], "childY"), num(&kids[4], "childY"));
    let row = 2.0 * w + 0.1 * w;
    approx(num(&kids[3], "childX"), (432.0 - row) / 2.0);
    // Colours: the colours part's accent, white text, white outline.
    assert_eq!(kids[0]["fill"]["color"], "4472C4");
    assert_eq!(kids[0]["dtext"]["paras"][0]["runs"][0]["color"], "FFFFFF");
    for k in kids {
        assert!(num(k, "childX") >= -0.01 && num(k, "childX") + num(k, "w") <= 432.01);
        assert!(num(k, "childY") >= -0.01 && num(k, "childY") + num(k, "h") <= 252.01);
    }
}

#[test]
fn the_colours_part_gives_the_node_fill_when_the_graphic_is_laid_out_here() {
    let doc = open_smartart(&Spec { drawing: None, accent: "accent2", ..Default::default() });
    let sh = shape(&doc);
    assert!(children(&sh).iter().all(|k| k["fill"]["color"] == "ED7D31"));
    assert_eq!(children(&sh)[0]["dtext"]["paras"][0]["runs"][0]["font"], "Calibri");
}

#[test]
fn a_picture_in_the_drawing_is_read_with_the_drawing_parts_own_relationship() {
    let drawing = block_list_drawing().replace(
        "</dsp:spTree>",
        r#"<dsp:pic><dsp:nvPicPr><dsp:cNvPr id="0" name=""/><dsp:cNvPicPr/></dsp:nvPicPr><dsp:blipFill><a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></dsp:blipFill><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="635000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></dsp:spPr></dsp:pic></dsp:spTree>"#,
    );
    let mut pkg = Package::open(&smartart_docx(&Spec { drawing: Some(drawing), ..Default::default() })).unwrap();
    pkg.set("word/media/image9.png", vec![0x89, b'P', b'N', b'G']);
    pkg.add_rel("word/diagrams/drawing1.xml", &format!("{REL_BASE}/image"), "../media/image9.png", false);
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let kids = children(&shape(&doc)).clone();
    assert_eq!(kids.len(), 4);
    assert_eq!(kids[3]["kind"], "picture");
    assert_eq!(kids[3]["src"], "word/media/image9.png");
    approx(num(&kids[3], "w"), 50.0);
}

#[test]
fn shapes_of_a_drawing_can_have_their_own_outline_and_a_rotated_text_rectangle() {
    let custom = r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="1000" h="500"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="1000" y="0"/></a:lnTo><a:lnTo><a:pt x="500" y="500"/></a:lnTo><a:close/></a:path></a:pathLst></a:custGeom>"#;
    let drawing = block_list_drawing().replacen(r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#, custom, 1).replacen("<dsp:txXfrm><a:off", r#"<dsp:txXfrm rot="5400000"><a:off"#, 1);
    let doc = open_smartart(&Spec { drawing: Some(drawing), ..Default::default() });
    let kids = children(&shape(&doc)).clone();
    assert_eq!(kids[0]["geometry"], "custom");
    assert_eq!(kids[0]["custom"]["paths"][0]["d"], "M0 0 L1000 0 L500 500 Z");
    approx(num(&kids[0]["dtext"], "rot"), 90.0);
    assert!(kids[1]["dtext"]["rot"].is_null());
}

#[test]
fn a_basic_process_has_arrows_between_the_boxes() {
    let doc = open_smartart(&Spec { drawing: None, layout: "process1", ..Default::default() });
    let sh = shape(&doc);
    assert_eq!(sh["diagram"]["layout"], "process1");
    let kids = children(&sh);
    let boxes: Vec<&Value> = kids.iter().filter(|k| k["geometry"] == "roundRect").collect();
    let arrows: Vec<&Value> = kids.iter().filter(|k| k["geometry"] == "rightArrow").collect();
    assert_eq!((boxes.len(), arrows.len()), (3, 2));
    approx(num(boxes[0], "childY"), num(boxes[2], "childY"));
    assert!(num(arrows[0], "childX") > num(boxes[0], "childX") + num(boxes[0], "w"));
    assert!(num(arrows[0], "childX") + num(arrows[0], "w") < num(boxes[1], "childX"));
    // The arrow is a lighter tint of the accent.
    assert_ne!(arrows[0]["fill"]["color"], boxes[0]["fill"]["color"]);
}

#[test]
fn a_basic_cycle_puts_the_nodes_on_a_circle_with_arrows() {
    let doc = open_smartart(&Spec { drawing: None, layout: "cycle2", nodes: &["a", "b", "c", "d"], extent: (3810000, 3810000), ..Default::default() });
    let sh = shape(&doc);
    let kids = children(&sh);
    let nodes: Vec<&Value> = kids.iter().filter(|k| k["geometry"] == "ellipse").collect();
    assert_eq!(nodes.len(), 4);
    assert_eq!(kids.iter().filter(|k| k["geometry"] == "rightArrow").count(), 4);
    let centre = |k: &Value| (num(k, "childX") + num(k, "w") / 2.0, num(k, "childY") + num(k, "h") / 2.0);
    let d: Vec<f64> = nodes.iter().map(|k| ((centre(k).0 - 150.0).powi(2) + (centre(k).1 - 150.0).powi(2)).sqrt()).collect();
    assert!(d.iter().all(|v| (v - d[0]).abs() < 0.1));
    // The first node is at the top.
    approx(centre(nodes[0]).0, 150.0);
    assert!(centre(nodes[0]).1 < 150.0);
}

#[test]
fn a_vertical_list_and_a_hierarchy_have_their_own_shapes() {
    let doc = open_smartart(&Spec { drawing: None, layout: "vList2", ..Default::default() });
    let kids_v = children(&shape(&doc)).clone();
    assert_eq!(kids_v.len(), 3);
    assert!(kids_v.iter().all(|k| k["geometry"] == "roundRect" && approx_eq(num(k, "w"), 432.0)));
    assert!(num(&kids_v[1], "childY") > num(&kids_v[0], "childY") + num(&kids_v[0], "h") - 0.01);

    let doc = open_smartart(&Spec { drawing: None, layout: "orgChart1", ..Default::default() });
    let sh = shape(&doc);
    let kids = children(&sh);
    // Three top-level nodes are three roots side by side: boxes in one row.
    assert_eq!(kids.iter().filter(|k| k["geometry"] == "rect").count(), 3);
    approx(num(&kids[0], "childY"), num(&kids[2], "childY"));
    assert!(num(&kids[2], "childX") > num(&kids[0], "childX"));
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.05
}

#[test]
fn a_layout_word_has_but_docxtor_does_not_draw_falls_back_to_the_block_list() {
    let doc = open_smartart(&Spec { drawing: None, layout: "pyramid1", ..Default::default() });
    let sh = shape(&doc);
    assert_eq!(sh["diagram"]["source"], "layout");
    assert_eq!(sh["diagram"]["layout"], "pyramid1");
    let kids = children(&sh);
    assert_eq!(kids.len(), 3);
    assert!(kids.iter().all(|k| k["geometry"] == "rect"));
}

#[test]
fn a_graphic_with_nothing_to_draw_is_a_placeholder_that_lists_its_texts() {
    // No drawing and no nodes.
    let doc = open_smartart(&Spec { drawing: None, nodes: &[], ..Default::default() });
    let sh = shape(&doc);
    assert_eq!(sh["diagram"]["source"], "placeholder");
    let kids = children(&sh);
    assert_eq!(kids.len(), 1);
    assert_eq!(run_texts(&kids[0]), ["SmartArt graphic"]);
    // Empty nodes count as nothing too.
    let doc = open_smartart(&Spec { drawing: None, nodes: &["", ""], ..Default::default() });
    assert_eq!(shape(&doc)["diagram"]["source"], "placeholder");
}

#[test]
fn a_graphic_whose_parts_are_gone_still_shows_as_a_placeholder() {
    let bytes = smartart_docx(&Spec::default());
    let mut pkg = Package::open(&bytes).unwrap();
    for n in ["word/diagrams/data1.xml", "word/diagrams/layout1.xml", "word/diagrams/drawing1.xml"] {
        pkg.remove(n);
    }
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "diagram");
    assert_eq!(sh["diagram"]["source"], "placeholder");
    assert_valid(&Document::open(&pkg.to_bytes().unwrap()).unwrap().save().unwrap());
}

// ---------------------------------------------------------------------------
// WordArt
// ---------------------------------------------------------------------------

fn wordart_paragraph(prst: &str, av: &str, rpr: &str, shape_pr: &str) -> String {
    format!(
        r##"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3810000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="2" name="WordArt 2" descr="Curved heading"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3810000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{shape_pr}</wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:rPr><w:b/><w:sz w:val="72"/>{rpr}</w:rPr><w:t>Hello WordArt</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr rot="0" spcFirstLastPara="0" vertOverflow="overflow" horzOverflow="overflow" vert="horz" wrap="none" lIns="91440" tIns="45720" rIns="91440" bIns="45720" numCol="1" spcCol="0" rtlCol="0" fromWordArt="1" anchor="ctr" anchorCtr="0" forceAA="0" compatLnSpc="1"><a:prstTxWarp prst="{prst}">{av}</a:prstTxWarp><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape id="WordArt 2" o:spid="_x0000_s1026" type="#_x0000_t144" style="width:300pt;height:100pt" fillcolor="#4472c4" stroked="f"><v:textpath style="font-family:&quot;Calibri&quot;;font-size:36pt;font-weight:bold" string="Hello WordArt"/></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"##
    )
}

#[test]
fn drawingml_wordart_reads_the_warp_and_the_look_of_the_text() {
    let rpr = r#"<w14:textOutline w14:w="9525" w14:cap="flat" w14:cmpd="sng" w14:algn="ctr"><w14:solidFill><w14:srgbClr w14:val="1F3864"/></w14:solidFill><w14:prstDash w14:val="solid"/><w14:round/></w14:textOutline><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="FF0000"/></w14:gs><w14:gs w14:pos="100000"><w14:schemeClr w14:val="accent1"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill></w14:textFill><w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl" w14:rotWithShape="0"><w14:srgbClr w14:val="000000"><w14:alpha w14:val="40000"/></w14:srgbClr></w14:shadow><w14:glow w14:rad="101600"><w14:schemeClr w14:val="accent1"><w14:alpha w14:val="40000"/></w14:schemeClr></w14:glow><w14:reflection w14:blurRad="6350" w14:stA="50000" w14:stPos="0" w14:endA="300" w14:endPos="35000" w14:dist="0" w14:dir="5400000" w14:fadeDir="5400000" w14:sx="100000" w14:sy="-100000" w14:kx="0" w14:ky="0" w14:algn="bl" w14:rotWithShape="0"/>"#;
    let doc = open_body(&wordart_paragraph("textArchUp", r#"<a:avLst><a:gd name="adj" fmla="val 10800000"/></a:avLst>"#, rpr, "<a:noFill/><a:ln><a:noFill/></a:ln>"));
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "shape");
    let w = &sh["warp"];
    assert_eq!(w["preset"], "textArchUp");
    assert_eq!(w["source"], "drawingml");
    approx(num(&w["adj"], "adj"), 10800000.0);
    assert_eq!(w["fill"]["kind"], "gradient");
    assert_eq!(w["fill"]["color"], "FF0000");
    assert_eq!(w["fill"]["color2"], "4472C4");
    assert_eq!(w["line"]["color"], "1F3864");
    approx(num(&w["line"], "widthPt"), 0.75);
    assert_eq!(w["shadow"]["color"], "000000");
    approx(num(&w["shadow"], "alpha"), 0.4);
    approx(num(&w["shadow"], "blurPt"), 4.0);
    assert!(num(&w["shadow"], "dx") > 2.0 && num(&w["shadow"], "dy") > 2.0);
    approx(num(&w["glow"], "radiusPt"), 8.0);
    approx(num(&w["reflection"], "startAlpha"), 0.5);
    approx(num(&w["reflection"], "size"), 35.0);
    // The shape itself has no fill or outline; the text is its story.
    assert!(sh["fill"].is_null() && sh["line"].is_null());
    assert_eq!(sh["text"][0]["segs"][0]["x"], "Hello WordArt");
    assert_eq!(sh["alt"], "Curved heading");
    let seg = segs(&doc).into_iter().find(|s| s.k == "shape").unwrap();
    assert_eq!(seg.label.as_deref(), Some("WordArt"));
}

#[test]
fn a_plain_text_color_is_the_text_fill_of_wordart() {
    let doc = open_body(&wordart_paragraph("textWave1", r#"<a:avLst/>"#, r#"<w:color w:val="C00000"/>"#, "<a:noFill/>"));
    let w = &shape(&doc)["warp"];
    assert_eq!(w["preset"], "textWave1");
    assert_eq!(w["fill"]["kind"], "solid");
    assert_eq!(w["fill"]["color"], "C00000");
    assert!(w["line"].is_null() && w["shadow"].is_null());
}

#[test]
fn a_shape_without_a_warp_has_none() {
    for prst in ["textNoShape", "notAPreset"] {
        let doc = open_body(&wordart_paragraph(prst, "<a:avLst/>", "", "<a:noFill/>"));
        assert!(shape(&doc)["warp"].is_null(), "{prst}");
    }
}

#[test]
fn the_text_of_drawingml_wordart_stays_editable_and_the_warp_stays() {
    let mut doc = open_body(&wordart_paragraph("textInflate", "<a:avLst/>", "", "<a:noFill/>"));
    let p = first_para(&doc);
    let sh = shape(&doc);
    let inner = sh["text"][0]["id"].as_u64().unwrap();
    ops(&mut doc, json!({"op": "insert_text", "at": {"p": inner, "o": 5}, "text": " big"}));
    let sh = shape(&doc);
    assert_eq!(sh["text"][0]["segs"][0]["x"], "Hello big WordArt");
    assert_eq!(sh["warp"]["preset"], "textInflate");
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = text(&saved, "word/document.xml");
    assert!(xml.contains(r#"<a:prstTxWarp prst="textInflate">"#));
    assert!(xml.contains("Hello big WordArt"));
    // The VML fallback, which Word 2007 reads, says the same.
    assert!(xml.contains(r#"string="Hello big WordArt""#), "the fallback textpath follows the text");
    ops(&mut doc, json!({"op": "set_shape", "at": {"p": p, "o": 0}, "w": 400, "h": 120}));
    assert_eq!(shape(&doc)["warp"]["preset"], "textInflate");
}

fn vml_wordart(spt: u32, shape_attrs: &str, inner: &str) -> String {
    format!(
        r##"<w:p><w:r><w:pict><v:shapetype id="_x0000_t{spt}" coordsize="21600,21600" o:spt="{spt}" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/><o:lock v:ext="edit" text="t" shapetype="t"/></v:shapetype><v:shape id="WordArt1" o:spid="_x0000_s1026" type="#_x0000_t{spt}" style="position:absolute;margin-left:36pt;margin-top:10pt;width:300pt;height:80pt;rotation:5;z-index:251658240;mso-position-horizontal-relative:text;mso-position-vertical-relative:text" {shape_attrs}>{inner}<w10:wrap type="square"/></v:shape></w:pict></w:r></w:p>"##
    )
}

#[test]
fn vml_wordart_is_a_shape_with_a_warp_from_its_shape_type() {
    let inner = r##"<v:fill color2="#f90" type="gradient" angle="0"/><v:stroke color="#600" weight="1pt"/><v:shadow on="t" color="silver" offset="3pt,3pt"/><v:textpath style="font-family:&quot;Impact&quot;;font-size:36pt;font-style:italic;font-weight:bold;v-text-kern:t" string="Sale Today"/>"##;
    let doc = open_body(&vml_wordart(148, r##"fillcolor="#c00" strokecolor="#600""##, inner));
    let sh = shape(&doc);
    assert_eq!(sh["kind"], "shape");
    assert_eq!(sh["source"], "vml");
    approx(num(&sh, "w"), 300.0);
    approx(num(&sh, "h"), 80.0);
    approx(num(&sh, "rot"), 5.0);
    assert_eq!(sh["wrap"], "square");
    let w = &sh["warp"];
    assert_eq!(w["preset"], "textArchUpPour");
    assert_eq!(w["source"], "vml");
    assert_eq!(w["text"], "Sale Today");
    assert_eq!(w["font"], "Impact");
    assert_eq!(w["bold"], true);
    assert_eq!(w["italic"], true);
    // The shape's fill and stroke are the text's.
    assert_eq!(w["fill"]["kind"], "gradient");
    assert_eq!(w["fill"]["color"], "CC0000");
    assert_eq!(w["line"]["color"], "660000");
    assert_eq!(w["shadow"]["color"], "C0C0C0");
    approx(num(&w["shadow"], "dx"), 3.0);
    assert!(sh["fill"].is_null() && sh["line"].is_null());
    let seg = segs(&doc).into_iter().find(|s| s.k == "shape").unwrap();
    assert_eq!(seg.label.as_deref(), Some("WordArt"));
    // Saving keeps the markup as it was.
    let mut doc = doc;
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert!(text(&saved, "word/document.xml").contains(r#"string="Sale Today""#));
}

#[test]
fn every_vml_wordart_shape_type_reads_as_its_preset() {
    for (spt, preset) in [(136, "textPlain"), (140, "textChevron"), (144, "textArchUp"), (156, "textWave1"), (160, "textInflate"), (170, "textFadeUp"), (172, "textSlantUp"), (175, "textCanDown")] {
        let doc = open_body(&vml_wordart(spt, r##"fillcolor="#000""##, r#"<v:textpath string="Hi"/>"#));
        assert_eq!(shape(&doc)["warp"]["preset"], preset, "o:spt {spt}");
    }
}

#[test]
fn a_text_watermark_is_still_not_a_shape_in_the_text() {
    let mark = r##"<w:p><w:r><w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="PowerPlusWaterMarkObject1" o:spid="_x0000_s2049" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:412pt;height:247pt;rotation:315;z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" fillcolor="silver" stroked="f"><v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="DRAFT"/></v:shape></w:pict></w:r><w:r><w:t>Body</w:t></w:r></w:p>"##;
    let doc = open_body(mark);
    assert!(segs(&doc).iter().all(|s| s.k != "shape" && s.k != "obj"));
}

// ---------------------------------------------------------------------------
// What the assistant reads
// ---------------------------------------------------------------------------

#[test]
fn the_assistant_reads_the_texts_of_a_smartart_graphic_and_of_wordart() {
    let mut doc = open_smartart(&Spec::default());
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains(r#"[SmartArt: "Alpha; Beta; Gamma"]"#), "{script}");
    let mut doc = open_body(&vml_wordart(148, r##"fillcolor="#c00""##, r#"<v:textpath string="Sale Today"/>"#));
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains(r#"[shape: wordart textArchUpPour "Sale Today"]"#), "{script}");
}
