//! Shapes and text boxes: reading Word's markup into the view model, editing the text inside them (it is a
//! story), inserting and changing shapes, the AI tools and DocScript.

use docxtor_core::ai::call_tool;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::render::{RBlock, RPara};
use docxtor_core::stories::StoryKind;
use docxtor_core::{package::Package, Document, Pos};
use serde_json::{json, Value};

const NS: &str = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" mc:Ignorable="w14 wp14""#;

const DRAWING_ML: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";

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
    // A blank document carries Office's theme; these tests choose whether the file has one and what it says.
    pkg.remove_rels_to("word/document.xml", "word/theme/theme1.xml");
    pkg.remove_override("word/theme/theme1.xml");
    pkg.remove("word/theme/theme1.xml");
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

/// The rendered paragraph with this id (body only).
fn rpara(doc: &Document, id: u32) -> RPara {
    fn find(blocks: &[RBlock], id: u32) -> Option<RPara> {
        blocks.iter().find_map(|b| match b {
            RBlock::Para(p) if p.id == id => Some(p.clone()),
            _ => None,
        })
    }
    find(&doc.render_snapshot().blocks, id).unwrap_or_else(|| panic!("no paragraph {id}"))
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

fn text_box_xml(lines: &[&str]) -> String {
    let content: String = lines.iter().map(|l| format!("<w:p><w:r><w:t>{l}</w:t></w:r></w:p>")).collect();
    format!(
        r##"<w:p><w:r><w:t xml:space="preserve">Before </w:t></w:r><w:r><w:rPr><w:noProof/></w:rPr><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1" wp14:anchorId="1A2B3C4D" wp14:editId="5E6F7A8B"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>2540000</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>127000</wp:posOffset></wp:positionV><wp:extent cx="2540000" cy="1016000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapSquare wrapText="bothSides"/><wp:docPr id="2" name="Text Box 2"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{DRAWING_ML}"><wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="1016000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:ln w="9525"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:miter lim="800000"/><a:headEnd/><a:tailEnd/></a:ln></wps:spPr><wps:txbx><w:txbxContent>{content}</w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="t" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic><wp14:sizeRelH relativeFrom="margin"><wp14:pctWidth>0</wp14:pctWidth></wp14:sizeRelH><wp14:sizeRelV relativeFrom="margin"><wp14:pctHeight>0</wp14:pctHeight></wp14:sizeRelV></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype><v:shape id="Text Box 2" o:spid="_x0000_s1026" type="#_x0000_t202" style="position:absolute;margin-left:200pt;margin-top:10pt;width:200pt;height:80pt;z-index:251659264;visibility:visible;mso-wrap-style:square;mso-wrap-distance-left:9pt;mso-wrap-distance-top:0;mso-wrap-distance-right:9pt;mso-wrap-distance-bottom:0;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text;v-text-anchor:top"><v:textbox><w:txbxContent>{content}</w:txbxContent></v:textbox><w10:wrap type="square"/></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t xml:space="preserve"> after</w:t></w:r></w:p><w:p><w:r><w:t>Next paragraph</w:t></w:r></w:p>"##
    )
}

/// A rectangle whose line and fill come from `wps:style` and the theme, with a lightened colour override.
const STYLED_RECT: &str = r##"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1524000" cy="762000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="3" name="Rectangle 3" descr="A blue rectangle"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm rot="1800000"><a:off x="0" y="0"/><a:ext cx="1524000" cy="762000"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 25000"/></a:avLst></a:prstGeom></wps:spPr><wps:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></wps:style><wps:bodyPr rot="0" spcFirstLastPara="0" vertOverflow="overflow" horzOverflow="overflow" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" numCol="1" spcCol="0" rtlCol="0" fromWordArt="0" anchor="ctr" anchorCtr="0" forceAA="0" compatLnSpc="1"><a:prstTxWarp prst="textNoShape"><a:avLst/></a:prstTxWarp><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:roundrect id="Rectangle 3" o:spid="_x0000_s1027" style="width:120pt;height:60pt;rotation:30" arcsize="16384f" fillcolor="#4472c4" strokecolor="#223962" strokeweight="1pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"##;

const ELLIPSE_WITH_TEXT: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251660288" behindDoc="1" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:align>center</wp:align></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>914400</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="4" name="Oval 4"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"><a:alpha val="50000"/></a:srgbClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:ln><a:noFill/></a:ln></wps:spPr><wps:txbx><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello oval</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:oval id="Oval 4" o:spid="_x0000_s1028" style="position:absolute;margin-left:0;margin-top:72pt;width:100pt;height:50pt;z-index:-251656192;mso-position-horizontal:center;mso-position-horizontal-relative:page;mso-position-vertical-relative:page" stroked="f"><v:textbox inset="0,0,0,0"><w:txbxContent><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Hello oval</w:t></w:r></w:p></w:txbxContent></v:textbox></v:oval></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

const ARROW_LINE: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="2540000" cy="0"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="8" name="Straight Arrow Connector 8"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvCnPr/><wps:spPr><a:xfrm flipH="1"><a:off x="0" y="0"/><a:ext cx="2540000" cy="0"/></a:xfrm><a:prstGeom prst="straightConnector1"><a:avLst/></a:prstGeom><a:ln w="19050"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:prstDash val="dash"/><a:tailEnd type="triangle"/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:line id="Straight Arrow Connector 8" o:spid="_x0000_s1029" style="position:absolute;flip:x;z-index:251661312" from="0,0" to="200pt,0" strokecolor="black" strokeweight="1.5pt"><v:stroke endarrow="block" dashstyle="dash"/></v:line></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

const GROUP: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="3048000" cy="1270000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="5" name="Group 5"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="3048000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="6096000" cy="2540000"/></a:xfrm></wpg:grpSpPr><wps:wsp><wps:cNvPr id="6" name="Left box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="7" name="Right oval"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="3556000" y="1270000"/><a:ext cx="2540000" cy="1270000"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>In the group</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:group id="Group 5" o:spid="_x0000_s1030" style="width:240pt;height:100pt" coordsize="480,200"><v:rect id="Left box" style="position:absolute;width:200;height:100" fillcolor="red"/><v:oval id="Right oval" style="position:absolute;left:280;top:100;width:200;height:100" fillcolor="lime"><v:textbox><w:txbxContent><w:p><w:r><w:t>In the group</w:t></w:r></w:p></w:txbxContent></v:textbox></v:oval></v:group></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

const CANVAS: &str = r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wpc"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="9" name="Canvas 9"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"><wpc:wpc><wpc:bg><a:noFill/></wpc:bg><wpc:whole><a:ln><a:noFill/></a:ln></wpc:whole><wps:wsp><wps:cNvPr id="10" name="Canvas box"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="254000" y="254000"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FFC000"/></a:solidFill></wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>Canvas text</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp><wps:wsp><wps:cNvPr id="11" name="Canvas star"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="2540000" y="1270000"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="star5"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="70AD47"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp></wpc:wpc></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect id="Canvas" style="width:432pt;height:252pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;

/// A text box from a file made before DrawingML shapes: VML only.
const VML_BOX: &str = r##"<w:p><w:r><w:t xml:space="preserve">Old: </w:t></w:r><w:r><w:pict><v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe"><v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype><v:shape id="_x0000_s1026" type="#_x0000_t202" style="position:absolute;margin-left:72pt;margin-top:36pt;width:144pt;height:72pt;z-index:251658240;mso-position-horizontal-relative:text;mso-position-vertical-relative:text;v-text-anchor:middle" fillcolor="#ffffcc" strokecolor="red" strokeweight="2pt"><v:textbox inset="3.6pt,1.8pt,3.6pt,1.8pt"><w:txbxContent><w:p><w:r><w:t>Old text box</w:t></w:r></w:p></w:txbxContent></v:textbox><w10:wrap type="square"/></v:shape></w:pict></w:r></w:p>"##;

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

#[test]
fn a_text_box_is_read_with_geometry_position_style_and_text() {
    let doc = open_with(&text_box_xml(&["First line", "Second line"]));
    let p = first_para(&doc);
    let para = rpara(&doc, p);
    // The text box is one object of length 1 between the two runs of text.
    assert_eq!(para.len, "Before ".len() + 1 + " after".len());
    let seg = para.segs.iter().find(|s| s.k == "shape").expect("a shape segment");
    assert_eq!(seg.o, 7);
    let s = shape(&doc, p, 7);
    assert_eq!(s["kind"], "shape");
    assert_eq!(s["source"], "drawingml");
    assert_eq!(s["geometry"], "rect");
    assert_eq!(s["id"], 2);
    assert_eq!(s["name"], "Text Box 2");
    assert_eq!((s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap()), (200.0, 80.0));
    assert_eq!(s["textBox"], true);
    assert_eq!(s["wrap"], "square");
    assert_eq!((s["x"].as_f64().unwrap(), s["y"].as_f64().unwrap()), (200.0, 10.0));
    assert_eq!((s["relX"].as_str(), s["relY"].as_str()), (Some("column"), Some("paragraph")));
    assert_eq!(s["fill"]["color"], "FFFFFF");
    assert_eq!((s["line"]["color"].as_str().unwrap(), s["line"]["widthPt"].as_f64().unwrap(), s["line"]["dash"].as_str().unwrap()), ("000000", 0.75, "solid"));
    assert_eq!((s["insets"]["l"].as_f64().unwrap(), s["insets"]["t"].as_f64().unwrap()), (7.2, 3.6));
    assert_eq!(s["vAnchor"], "top");
    // The text is rendered with the ids the text ops use.
    let boxes = box_paras(&doc);
    assert_eq!(boxes.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["First line", "Second line"]);
    let ids: Vec<u64> = s["text"].as_array().unwrap().iter().map(|b| b["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, boxes.iter().map(|(i, _)| *i as u64).collect::<Vec<_>>());
    assert_eq!(s["text"][0]["segs"][0]["x"], "First line");
    // The paragraph ids of the body don't clash with the ones in the box.
    assert!(boxes.iter().all(|(i, _)| *i != p));
    // The box's text is not part of the host paragraph's text.
    assert_eq!(doc.find_any_para(p).unwrap().plain_text(), "Before  after");
}

#[test]
fn theme_colours_and_the_shape_style_resolve_to_hex() {
    let doc = Document::open(&package_with(STYLED_RECT, true)).unwrap();
    let p = first_para(&doc);
    let s = shape(&doc, p, 0);
    assert_eq!(s["geometry"], "roundRect");
    assert_eq!(s["adj"]["adj"], 25000.0);
    assert_eq!(s["rot"], 30.0);
    assert_eq!(s["alt"], "A blue rectangle");
    // fillRef idx 1 with accent1; lnRef idx 2 (the theme's second line, 1pt) with accent1 shaded by half.
    assert_eq!(s["fill"]["color"], "4472C4");
    assert_eq!(s["line"]["color"], "223962");
    assert_eq!(s["line"]["widthPt"], 1.0);
    assert_eq!(s["wrap"], "inline");
    assert_eq!(s["vAnchor"], "middle");
    assert!(s.get("text").is_none(), "a shape without a text frame has no text");
    assert_eq!(box_count(&doc), 0);
}

#[test]
fn colour_transforms_and_a_missing_theme_use_the_office_colours() {
    let body = STYLED_RECT.replace(
        r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 25000"/></a:avLst></a:prstGeom></wps:spPr>"#,
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:schemeClr val="accent1"><a:lumMod val="20000"/><a:lumOff val="80000"/></a:schemeClr></a:solidFill></wps:spPr>"#,
    );
    let doc = Document::open(&package_with(&body, true)).unwrap();
    let c = shape(&doc, first_para(&doc), 0)["fill"]["color"].as_str().unwrap().to_string();
    // Word's "Lighter 80%" of 4472C4 is D9E2F3.
    let hex = |s: &str, i: usize| i64::from_str_radix(&s[i..i + 2], 16).unwrap();
    for i in [0, 2, 4] {
        assert!((hex(&c, i) - hex("D9E2F3", i)).abs() <= 2, "{c}");
    }
    // Without a theme part, accent1 is the Office colour.
    let doc = open_with(STYLED_RECT);
    assert_eq!(shape(&doc, first_para(&doc), 0)["fill"]["color"], "5B9BD5");
}

#[test]
fn an_ellipse_with_text_reads_its_gradient_alpha_and_frame() {
    let doc = open_with(ELLIPSE_WITH_TEXT);
    let p = first_para(&doc);
    let s = shape(&doc, p, 0);
    assert_eq!(s["geometry"], "ellipse");
    assert_eq!(s["textBox"], false);
    assert_eq!(s["fill"]["kind"], "gradient");
    assert_eq!((s["fill"]["color"].as_str(), s["fill"]["color2"].as_str(), s["fill"]["angle"].as_f64()), (Some("FF0000"), Some("0000FF"), Some(90.0)));
    assert!(s.get("line").is_none(), "an explicit noFill outline is no outline");
    assert_eq!(s["wrap"], "behind");
    assert_eq!((s["align"].as_str(), s["relX"].as_str(), s["relY"].as_str(), s["y"].as_f64()), (Some("center"), Some("page"), Some("page"), Some(72.0)));
    assert_eq!(s["vAnchor"], "middle");
    assert_eq!(s["insets"]["l"], 0.0);
    assert_eq!(box_paras(&doc).iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Hello oval"]);
    assert_eq!(s["text"][0]["fmt"]["align"], "center");
}

#[test]
fn an_arrow_line_reads_dash_flip_and_arrow() {
    let doc = Document::open(&package_with(ARROW_LINE, true)).unwrap();
    let s = shape(&doc, first_para(&doc), 0);
    assert_eq!(s["geometry"], "straightConnector1");
    assert_eq!((s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap()), (200.0, 0.0));
    assert_eq!(s["flipH"], true);
    assert!(s.get("flipV").is_none());
    assert_eq!(s["line"]["color"], "000000");
    assert_eq!((s["line"]["widthPt"].as_f64(), s["line"]["dash"].as_str(), s["line"]["tailArrow"].as_str()), (Some(1.5), Some("dash"), Some("triangle")));
    assert!(s["line"].get("headArrow").is_none());
    assert_eq!(s["textBox"], false);
}

#[test]
fn a_group_reads_its_members_scaled_into_its_box() {
    let doc = open_with(GROUP);
    let p = first_para(&doc);
    let s = shape(&doc, p, 0);
    assert_eq!(s["kind"], "group");
    assert_eq!((s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap()), (240.0, 100.0));
    let kids = s["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    // The group shows its child space (480 x 200 pt) at half size.
    assert_eq!((kids[0]["geometry"].as_str(), kids[0]["childX"].as_f64(), kids[0]["childY"].as_f64()), (Some("rect"), Some(0.0), Some(0.0)));
    assert_eq!((kids[0]["w"].as_f64(), kids[0]["h"].as_f64()), (Some(100.0), Some(50.0)));
    assert_eq!(kids[0]["fill"]["color"], "FF0000");
    assert_eq!((kids[1]["geometry"].as_str(), kids[1]["childX"].as_f64(), kids[1]["childY"].as_f64()), (Some("ellipse"), Some(140.0), Some(50.0)));
    assert_eq!(kids[1]["name"], "Right oval");
    assert_eq!(kids[1]["text"][0]["segs"][0]["x"], "In the group");
    assert_eq!(box_paras(&doc).len(), 1);
}

#[test]
fn a_drawing_canvas_reads_its_members() {
    let doc = open_with(CANVAS);
    let s = shape(&doc, first_para(&doc), 0);
    assert_eq!(s["kind"], "canvas");
    assert_eq!((s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap()), (432.0, 252.0));
    let kids = s["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    assert_eq!((kids[0]["childX"].as_f64(), kids[0]["childY"].as_f64(), kids[0]["w"].as_f64(), kids[0]["h"].as_f64()), (Some(20.0), Some(20.0), Some(100.0), Some(50.0)));
    assert_eq!(kids[1]["geometry"], "star5");
    assert_eq!((kids[1]["childX"].as_f64(), kids[1]["childY"].as_f64()), (Some(200.0), Some(100.0)));
    assert!(s.get("fill").is_none() && s.get("line").is_none());
    assert_eq!(box_paras(&doc).iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Canvas text"]);
}

#[test]
fn custom_geometry_is_simplified_to_svg_paths() {
    let body = ELLIPSE_WITH_TEXT.replace(
        r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#,
        r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="100" h="50"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="0"/></a:lnTo><a:cubicBezTo><a:pt x="100" y="20"/><a:pt x="80" y="50"/><a:pt x="50" y="50"/></a:cubicBezTo><a:arcTo wR="25" hR="25" stAng="5400000" swAng="5400000"/><a:close/></a:path><a:path w="10" h="10" fill="none"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="r" y="b"/></a:lnTo></a:path></a:pathLst></a:custGeom>"#,
    );
    let doc = open_with(&body);
    let s = shape(&doc, first_para(&doc), 0);
    assert_eq!(s["geometry"], "custom");
    let paths = s["custom"]["paths"].as_array().unwrap();
    assert_eq!(paths.len(), 2);
    assert_eq!((paths[0]["w"].as_f64(), paths[0]["h"].as_f64(), paths[0]["fill"].as_bool()), (Some(100.0), Some(50.0), Some(true)));
    let d = paths[0]["d"].as_str().unwrap();
    assert!(d.starts_with("M0 0 L100 0 C100 20 80 50 50 50 A25 25 0 0 1 "), "{d}");
    assert!(d.ends_with(" Z"), "{d}");
    // The arc runs a quarter turn clockwise from the bottom of its circle: it ends at the left of the centre.
    assert!(d.contains("A25 25 0 0 1 25 25"), "{d}");
    assert_eq!((paths[1]["d"].as_str(), paths[1]["fill"].as_bool()), (Some("M0 0 L10 10"), Some(false)));
}

#[test]
fn a_vml_only_text_box_from_an_old_file_is_read() {
    let doc = open_with(VML_BOX);
    let p = first_para(&doc);
    let s = shape(&doc, p, 5);
    assert_eq!(s["source"], "vml");
    assert_eq!((s["geometry"].as_str(), s["textBox"].as_bool()), (Some("rect"), Some(true)));
    assert_eq!((s["w"].as_f64(), s["h"].as_f64()), (Some(144.0), Some(72.0)));
    assert_eq!((s["wrap"].as_str(), s["x"].as_f64(), s["y"].as_f64(), s["relX"].as_str(), s["relY"].as_str()), (Some("square"), Some(72.0), Some(36.0), Some("column"), Some("paragraph")));
    assert_eq!(s["fill"]["color"], "FFFFCC");
    assert_eq!((s["line"]["color"].as_str(), s["line"]["widthPt"].as_f64()), (Some("FF0000"), Some(2.0)));
    assert_eq!((s["insets"]["l"].as_f64(), s["insets"]["t"].as_f64()), (Some(3.6), Some(1.8)));
    assert_eq!(s["vAnchor"], "middle");
    assert_eq!(box_paras(&doc).iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Old text box"]);
    // "Old: " then the box.
    assert_eq!(rpara(&doc, p).segs.iter().find(|x| x.k == "shape").unwrap().o, 5);
}

#[test]
fn a_plain_vml_rectangle_defaults_to_white_with_a_black_line() {
    let body = r#"<w:p><w:r><w:pict><v:rect id="Rectangle 1" style="width:50pt;height:20pt"/></w:pict></w:r></w:p>"#;
    let doc = open_with(body);
    let s = shape(&doc, first_para(&doc), 0);
    assert_eq!((s["fill"]["color"].as_str(), s["line"]["color"].as_str(), s["line"]["widthPt"].as_f64()), (Some("FFFFFF"), Some("000000"), Some(0.75)));
    assert_eq!(s["wrap"], "inline");
}

#[test]
fn pictures_and_watermarks_are_not_shapes() {
    let body = r##"<w:p><w:r><w:pict><v:shape id="WordPictureWatermark1" style="position:absolute;width:100pt;height:50pt;z-index:-1"><v:imagedata r:id="rId9"/></v:shape></w:pict></w:r><w:r><w:pict><v:shape id="pic" type="#_x0000_t75" style="width:10pt;height:10pt"><v:imagedata r:id="rId8"/></v:shape></w:pict></w:r></w:p>"##;
    let doc = open_with(body);
    let p = first_para(&doc);
    assert!(rpara(&doc, p).segs.iter().all(|s| s.k != "shape"));
    assert!(doc.shape_at(Pos::new(p, 0)).is_none());
}

// ---------------------------------------------------------------------------
// Lossless
// ---------------------------------------------------------------------------

#[test]
fn untouched_shapes_round_trip_byte_identical() {
    for body in [text_box_xml(&["A", "B"]), STYLED_RECT.to_string(), ELLIPSE_WITH_TEXT.to_string(), ARROW_LINE.to_string(), GROUP.to_string(), CANVAS.to_string(), VML_BOX.to_string()] {
        let bytes = package_with(&body, true);
        let mut doc = Document::open(&bytes).unwrap();
        let saved = doc.save().unwrap();
        assert_eq!(part(&saved, "word/document.xml"), part(&bytes, "word/document.xml"));
        // Even when the body is rewritten for another reason, every shape is written as it was read.
        let mut doc = Document::open(&bytes).unwrap();
        let id = first_para(&doc);
        let before = part(&bytes, "word/document.xml");
        ops(&mut doc, json!({"op":"insert_paragraph","after":id,"text":"Elsewhere"}));
        let after = doc_xml(&mut doc);
        assert!(!after.contains("data-docxtor-tb"), "the in-memory marker never reaches the file");
        // The re-serialised body differs only by the new paragraph.
        let canon = docxtor_core::xml::parse(before.as_bytes()).unwrap().to_bytes();
        let canon = String::from_utf8(canon).unwrap();
        assert_eq!(after.replace("<w:p><w:r><w:t>Elsewhere</w:t></w:r></w:p>", ""), canon);
    }
}

#[test]
fn typing_in_a_text_box_changes_only_the_text_of_its_choice_and_fallback() {
    let original = text_box_xml(&["First line", "Second line"]);
    let bytes = package_with(&original, false);
    let mut doc = Document::open(&bytes).unwrap();
    let canon = String::from_utf8(docxtor_core::xml::parse(&part(&bytes, "word/document.xml").into_bytes()).unwrap().to_bytes()).unwrap();
    let (first, _) = box_paras(&doc)[0].clone();
    let host = first_para(&doc);
    let r = ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":10},"text":"!"}));
    assert!(r[0].caret.is_none() || r[0].caret.unwrap().p.0 == first);
    let saved = doc_xml(&mut doc);
    // Exactly two places changed: the DrawingML text box and the VML copy.
    assert_eq!(saved.matches("First line!").count(), 2);
    assert_eq!(saved, canon.replace("First line", "First line!"));
    // The host paragraph and everything else keep their text.
    assert_eq!(doc.find_any_para(host).unwrap().plain_text(), "Before  after");
    assert_eq!(box_paras(&doc)[0].1, "First line!");
    // Only the document part was written.
    let out = doc.save().unwrap();
    for name in ["word/styles.xml", "word/settings.xml", "[Content_Types].xml"] {
        assert_eq!(part(&out, name), part(&bytes, name), "{name}");
    }
}

#[test]
fn typing_in_a_text_box_renders_in_the_host_paragraph_and_undoes() {
    let mut doc = open_with(&text_box_xml(&["First line", "Second line"]));
    let host = first_para(&doc);
    doc.render_all();
    let (first, _) = box_paras(&doc)[0].clone();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"NEW "}));
    // The host paragraph is what re-renders, with the new text inside its shape.
    let r = doc.render_changed();
    assert!(!r.full && r.blocks.len() == 1, "{} blocks", r.blocks.len());
    let RBlock::Para(p) = &r.blocks[0] else { panic!() };
    assert_eq!(p.id, host);
    let sh = p.segs.iter().find_map(|s| s.shape.as_ref()).unwrap();
    let RBlock::Para(inner) = &sh.text.as_ref().unwrap()[0] else { panic!() };
    assert_eq!(inner.segs[0].x.as_deref(), Some("NEW First line"));
    assert!(doc.undo());
    assert_eq!(box_paras(&doc)[0].1, "First line");
    let saved = doc_xml(&mut doc);
    assert!(saved.contains("<w:t>First line</w:t>") && !saved.contains("NEW "));
    assert!(doc.redo());
    assert_eq!(box_paras(&doc)[0].1, "NEW First line");
    assert_eq!(doc_xml(&mut doc).matches("NEW First line").count(), 2);
}

#[test]
fn paragraphs_split_and_removed_in_a_text_box_reach_both_copies() {
    let mut doc = open_with(&text_box_xml(&["First line", "Second line"]));
    let (first, _) = box_paras(&doc)[0].clone();
    let r = ops(&mut doc, json!({"op":"split_paragraph","at":{"p":first,"o":5}}));
    let new = r[0].caret.unwrap().p.0;
    assert_ne!(new, first);
    assert_eq!(box_paras(&doc).iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["First", " line", "Second line"]);
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches("<w:t>First</w:t>").count(), 2);
    assert_eq!(xml.matches("<w:t xml:space=\"preserve\"> line</w:t>").count(), 2);
    let (last, _) = box_paras(&doc)[2].clone();
    ops(&mut doc, json!({"op":"delete_blocks","ids":[last]}));
    assert_eq!(box_paras(&doc).len(), 2);
    assert_eq!(doc_xml(&mut doc).matches("Second line").count(), 0);
    // A box can't be left without a paragraph.
    let ids: Vec<u32> = box_paras(&doc).iter().map(|(i, _)| *i).collect();
    ops(&mut doc, json!({"op":"delete_blocks","ids":ids}));
    assert_eq!(box_paras(&doc).len(), 1);
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches("<w:txbxContent><w:p/></w:txbxContent>").count(), 2);
}

#[test]
fn typing_in_a_vml_only_text_box_and_in_a_group_updates_the_markup() {
    let mut doc = open_with(VML_BOX);
    let (id, _) = box_paras(&doc)[0].clone();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":3},"text":"-"}));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains("<w:t>Old- text box</w:t>"), "{xml}");
    assert_eq!(xml.matches("txbxContent>").count(), 2);

    let mut doc = open_with(GROUP);
    let (id, _) = box_paras(&doc)[0].clone();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":"Hi "}));
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches("Hi In the group").count(), 2, "the child's text and its VML copy");
    assert_eq!(shape(&doc, first_para(&doc), 0)["children"][1]["text"][0]["segs"][0]["x"], "Hi In the group");
}

#[test]
fn formatting_and_lists_work_inside_a_text_box() {
    let mut doc = open_with(&text_box_xml(&["Plain words"]));
    let (id, _) = box_paras(&doc)[0].clone();
    ops(&mut doc, json!({"op":"format_text","from":{"p":id,"o":0},"to":{"p":id,"o":5},"format":{"bold":true}}));
    ops(&mut doc, json!({"op":"toggle_list","ids":[id],"kind":"bullet"}));
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches("<w:b/>").count(), 2);
    assert_eq!(xml.matches("<w:numPr>").count(), 2);
    // The list label is rendered inside the shape.
    let sh = shape(&doc, first_para(&doc), 7);
    assert_eq!(sh["text"][0]["list"]["bullet"], true);
    assert_eq!(sh["text"][0]["segs"][0]["f"]["bold"], true);
}

#[test]
fn deleting_the_object_removes_the_shape_and_its_text_and_undo_brings_it_back() {
    let mut doc = open_with(&text_box_xml(&["First line", "Second line"]));
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":7},"to":{"p":p,"o":8}}));
    assert_eq!(doc.find_any_para(p).unwrap().plain_text(), "Before  after");
    assert_eq!(box_count(&doc), 0);
    assert!(box_paras(&doc).is_empty());
    let xml = doc_xml(&mut doc);
    assert!(!xml.contains("AlternateContent") && !xml.contains("txbxContent"), "{xml}");
    assert!(doc.undo());
    assert_eq!(box_count(&doc), 1);
    assert_eq!(box_paras(&doc).len(), 2);
    assert_eq!(doc_xml(&mut doc).matches("Second line").count(), 2);
}

// ---------------------------------------------------------------------------
// Inserting
// ---------------------------------------------------------------------------

fn blank_with_text(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let id = first_para(&doc);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":text}));
    (doc, id)
}

#[test]
fn insert_text_box_writes_word_markup_and_puts_the_caret_inside() {
    let (mut doc, p) = blank_with_text("Hello world");
    let r = ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":5},"w":180,"h":60,"text":"First\nSecond"}));
    assert_eq!(r[0].created.len(), 2);
    assert_eq!(r[0].caret.unwrap().p.0, r[0].created[0]);
    assert_eq!(r[0].caret.unwrap().o, 0);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml");
    // Word's structure: Choice with the DrawingML text box, Fallback with the VML one.
    for needle in [
        r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor"#,
        r#"<wps:cNvSpPr txBox="1"/>"#,
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#,
        r#"<a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill>"#,
        r#"<a:ln w="9525"><a:solidFill><a:srgbClr val="000000"/></a:solidFill>"#,
        r#"<wp:extent cx="2286000" cy="762000"/>"#,
        r#"<wp:wrapSquare wrapText="bothSides"/>"#,
        "<mc:Fallback><w:pict>",
        r#"<v:shapetype id="_x0000_t202""#,
        "<v:textbox><w:txbxContent>",
        r#"<w10:wrap type="square"/>"#,
        "<wps:bodyPr",
    ] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
    assert_eq!(xml.matches("<w:t>First</w:t>").count(), 2);
    assert_eq!(xml.matches("<w:t>Second</w:t>").count(), 2);
    // Namespaces are declared on the root, and wp14 is ignorable, as in Word's files.
    let root = &xml[xml.find("<w:document").unwrap()..xml.find('>').map(|_| xml.find("<w:body>").unwrap()).unwrap()];
    for ns in ["xmlns:mc=", "xmlns:wps=", "xmlns:wp14=", "xmlns:v=", "xmlns:o=", "xmlns:w10=", "xmlns:wp=", "xmlns:a="] {
        assert!(root.contains(ns), "{ns} in {root}");
    }
    assert!(root.contains(r#"mc:Ignorable="wp14""#), "{root}");
    // The text is in the host paragraph's flattened text as one object after "Hello".
    assert_eq!(doc.find_any_para(p).unwrap().text(), "Hello\u{FFFC} world");
    // Reading it back (also after a save and reopen).
    let s = shape(&doc, p, 5);
    assert_eq!((s["textBox"].as_bool(), s["wrap"].as_str(), s["w"].as_f64(), s["h"].as_f64()), (Some(true), Some("square"), Some(180.0), Some(60.0)));
    let again = Document::open(&bytes).unwrap();
    let s2 = shape(&again, first_para(&again), 5);
    assert_eq!(s2["geometry"], "rect");
    assert_eq!(box_paras(&again).iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["First", "Second"]);
    // Typing at the caret lands in the box and in both copies.
    let caret = r[0].caret.unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":caret.p.0,"o":5},"text":" one"}));
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches("First one").count(), 2);
    // One undo step per op: the typing, then the insert.
    assert!(doc.undo());
    assert_eq!(box_paras(&doc)[0].1, "First");
    assert!(doc.undo());
    assert_eq!(box_count(&doc), 0);
    assert!(!doc_xml(&mut doc).contains("AlternateContent"));
    assert_eq!(doc.find_any_para(p).unwrap().text(), "Hello world");
}

#[test]
fn insert_shape_writes_fill_outline_text_and_a_matching_vml_fallback() {
    let (mut doc, p) = blank_with_text("Shape here");
    let r = ops(
        &mut doc,
        json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"ellipse","w":120,"h":60,"fill":"#ffe699","line":{"color":"C00000","width_pt":2.5,"dash":"dash"},"text":"Hi\nthere","floating":true,"x":10,"y":20}),
    );
    assert_eq!(r[0].created.len(), 2);
    assert_eq!(r[0].caret.unwrap(), Pos::new(p, 1), "the caret lands after a shape");
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml");
    for needle in [
        r#"<wps:cNvSpPr/>"#,
        r#"<a:prstGeom prst="ellipse">"#,
        r#"<a:solidFill><a:srgbClr val="FFE699"/></a:solidFill>"#,
        r#"<a:ln w="31750"><a:solidFill><a:srgbClr val="C00000"/></a:solidFill><a:prstDash val="dash"/>"#,
        r#"<wp:posOffset>127000</wp:posOffset>"#,
        r#"<wp:posOffset>254000</wp:posOffset>"#,
        r#"anchor="ctr""#,
        r##"<v:oval"##,
        r##"fillcolor="#FFE699""##,
        r##"strokecolor="#C00000" strokeweight="2.5pt""##,
        r#"<w:jc w:val="center"/>"#,
    ] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
    let s = shape(&doc, p, 0);
    assert_eq!((s["geometry"].as_str(), s["textBox"].as_bool(), s["x"].as_f64(), s["y"].as_f64()), (Some("ellipse"), Some(false), Some(10.0), Some(20.0)));
    assert_eq!((s["fill"]["color"].as_str(), s["line"]["color"].as_str(), s["line"]["widthPt"].as_f64(), s["line"]["dash"].as_str()), (Some("FFE699"), Some("C00000"), Some(2.5), Some("dash")));
    assert_eq!(box_paras(&doc).iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Hi", "there"]);
    // Reopened, the same shape.
    let again = Document::open(&bytes).unwrap();
    assert_eq!(shape(&again, first_para(&again), 0)["geometry"], "ellipse");
}

#[test]
fn insert_shape_inline_without_fill_or_text_and_lines_with_arrows() {
    let (mut doc, p) = blank_with_text("ab");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":1},"geometry":"rect","w":40,"h":20,"fill":"none","line":"none","floating":false}));
    let s = shape(&doc, p, 1);
    assert_eq!(s["wrap"], "inline");
    assert!(s.get("fill").is_none() && s.get("line").is_none() && s.get("text").is_none());
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"line","w":100,"h":0,"line":{"color":"FF0000","width_pt":3,"tail_arrow":"triangle","head_arrow":"oval"}}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["geometry"].as_str(), s["h"].as_f64()), (Some("line"), Some(0.0)));
    assert_eq!((s["line"]["tailArrow"].as_str(), s["line"]["headArrow"].as_str()), (Some("triangle"), Some("oval")));
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"<wps:cNvCnPr/>"#) && xml.contains(r#"<a:tailEnd type="triangle"/>"#) && xml.contains("<v:line"), "{xml}");
    assert!(xml.contains(r#"endarrow="block""#) && xml.contains(r#"startarrow="oval""#), "{xml}");
    assert_eq!(box_count(&doc), 0);
}

#[test]
fn insert_shape_rejects_bad_input_and_changes_nothing() {
    let (mut doc, p) = blank_with_text("x");
    assert!(err(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"blob","w":10,"h":10})).contains("unknown shape"));
    assert!(err(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":10,"h":10,"fill":"zzz"})).contains("not a colour"));
    assert!(err(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":0,"h":10})).contains("between"));
    assert_eq!(box_count(&doc), 0);
    // Plain-language names work.
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"Rounded Rectangle","w":40,"h":20}));
    assert_eq!(shape(&doc, p, 0)["geometry"], "roundRect");
    // Shapes can't go inside a text box.
    ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":0},"w":100,"h":50,"text":"in"}));
    let (inner, _) = box_paras(&doc)[0].clone();
    assert!(err(&mut doc, json!({"op":"insert_shape","at":{"p":inner,"o":0},"geometry":"rect","w":10,"h":10})).contains("main text"));
}

// ---------------------------------------------------------------------------
// Changing
// ---------------------------------------------------------------------------

#[test]
fn set_shape_resizes_moves_and_recolours_the_choice_and_the_fallback() {
    let (mut doc, p) = blank_with_text("Move me");
    ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":0},"w":100,"h":50,"text":"Box"}));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"w":150,"h":75,"x":30,"y":40,"fill":"FFF2CC","line":{"color":"0000FF","width_pt":2},"rot":15,"v_anchor":"middle"}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["w"].as_f64(), s["h"].as_f64(), s["x"].as_f64(), s["y"].as_f64(), s["rot"].as_f64()), (Some(150.0), Some(75.0), Some(30.0), Some(40.0), Some(15.0)));
    assert_eq!((s["fill"]["color"].as_str(), s["line"]["color"].as_str(), s["line"]["widthPt"].as_f64(), s["vAnchor"].as_str()), (Some("FFF2CC"), Some("0000FF"), Some(2.0), Some("middle")));
    let xml = doc_xml(&mut doc);
    for needle in [
        r#"<wp:extent cx="1905000" cy="952500"/>"#,
        r#"<a:ext cx="1905000" cy="952500"/>"#,
        r#"<wp:posOffset>381000</wp:posOffset>"#,
        r#"<wp:posOffset>508000</wp:posOffset>"#,
        r#"rot="900000""#,
        r#"<a:srgbClr val="FFF2CC"/>"#,
        r#"<a:ln w="25400"><a:solidFill><a:srgbClr val="0000FF"/>"#,
        r#"anchor="ctr""#,
        // The VML copy follows.
        "width:150pt;height:75pt",
        "margin-left:30pt",
        "margin-top:40pt",
        "rotation:15",
        "v-text-anchor:middle",
        r##"fillcolor="#FFF2CC""##,
        r##"strokecolor="#0000FF" strokeweight="2pt""##,
    ] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
    // The text, its story and the rest stay.
    assert_eq!(xml.matches("<w:t>Box</w:t>").count(), 2);
    assert_eq!(box_paras(&doc)[0].1, "Box");
    // Undo restores the old look.
    assert!(doc.undo());
    let s = shape(&doc, p, 0);
    assert_eq!((s["w"].as_f64(), s["fill"]["color"].as_str(), s["x"].as_f64()), (Some(100.0), Some("FFFFFF"), Some(0.0)));
}

#[test]
fn set_shape_changes_wrap_geometry_and_flips_and_can_take_a_shape_out_of_the_line() {
    let (mut doc, p) = blank_with_text("w");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":60,"h":30,"floating":false}));
    assert_eq!(shape(&doc, p, 0)["wrap"], "inline");
    // Setting a position floats an inline shape.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"x":5,"y":6,"geometry":"rightArrow","flip_h":true}));
    let s = shape(&doc, p, 0);
    assert_eq!((s["wrap"].as_str(), s["x"].as_f64(), s["y"].as_f64(), s["geometry"].as_str(), s["flipH"].as_bool()), (Some("square"), Some(5.0), Some(6.0), Some("rightArrow"), Some(true)));
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"wrap":"behind"}));
    assert_eq!(shape(&doc, p, 0)["wrap"], "behind");
    let xml = doc_xml(&mut doc);
    assert!(xml.contains(r#"behindDoc="1""#) && xml.contains("<wp:wrapNone/>") && xml.contains(r#"flipH="1""#), "{xml}");
    assert!(xml.contains("z-index:-"), "the VML copy goes behind the text too: {xml}");
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"wrap":"inline"}));
    assert_eq!(shape(&doc, p, 0)["wrap"], "inline");
    let xml = doc_xml(&mut doc);
    assert!(xml.contains("<wp:inline") && !xml.contains("<wp:anchor") && !xml.contains("position:absolute"), "{xml}");
    // Fill and outline removed.
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"fill":"none","line":"none"}));
    let s = shape(&doc, p, 0);
    assert!(s.get("fill").is_none() && s.get("line").is_none());
    let xml = doc_xml(&mut doc);
    assert!(xml.contains("<a:noFill/>") && xml.contains(r#"filled="f""#) && xml.contains(r#"stroked="f""#), "{xml}");
}

#[test]
fn set_shape_works_on_shapes_read_from_files_and_leaves_their_other_markup_alone() {
    let body = text_box_xml(&["First line", "Second line"]);
    let bytes = package_with(&body, false);
    let mut doc = Document::open(&bytes).unwrap();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":7},"w":300,"line":{"width_pt":3}}));
    let s = shape(&doc, p, 7);
    assert_eq!((s["w"].as_f64(), s["h"].as_f64(), s["line"]["color"].as_str(), s["line"]["widthPt"].as_f64()), (Some(300.0), Some(80.0), Some("000000"), Some(3.0)));
    let xml = doc_xml(&mut doc);
    let canon = String::from_utf8(docxtor_core::xml::parse(&part(&bytes, "word/document.xml").into_bytes()).unwrap().to_bytes()).unwrap();
    // Only the size, the outline's width and the VML copy's size and stroke weight changed; the outline keeps its
    // join and arrow elements, and the shape keeps its ids, its wp14 attributes and its text.
    let expected = canon
        .replace(r#"cx="2540000" cy="1016000""#, r#"cx="3810000" cy="1016000""#)
        .replace(r#"<a:ln w="9525">"#, r#"<a:ln w="38100">"#)
        .replace("width:200pt;height:80pt", "width:300pt;height:80pt")
        .replace(r#"v-text-anchor:top">"#, r##"v-text-anchor:top" strokecolor="#000000" strokeweight="3pt">"##);
    assert_eq!(xml, expected);
    assert!(err(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"w":10})).contains("no shape"));
}

#[test]
fn set_shape_edits_a_vml_only_shape_in_place() {
    let mut doc = open_with(VML_BOX);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":5},"w":200,"h":100,"fill":"none","line":{"color":"0000FF","width_pt":1,"dash":"dash","tail_arrow":"triangle"},"x":10,"y":20,"rot":45,"geometry":"ellipse"}));
    let s = shape(&doc, p, 5);
    assert_eq!((s["w"].as_f64(), s["h"].as_f64(), s["x"].as_f64(), s["y"].as_f64(), s["rot"].as_f64()), (Some(200.0), Some(100.0), Some(10.0), Some(20.0), Some(45.0)));
    assert!(s.get("fill").is_none());
    assert_eq!((s["line"]["color"].as_str(), s["line"]["dash"].as_str(), s["line"]["tailArrow"].as_str()), (Some("0000FF"), Some("dash"), Some("triangle")));
    let xml = doc_xml(&mut doc);
    // A v:shape keeps its type (only the plain VML shapes change preset); the text is untouched.
    assert!(xml.contains("<v:shape ") && xml.contains("width:200pt;height:100pt") && xml.contains("rotation:45") && xml.contains(r#"filled="f""#), "{xml}");
    assert!(xml.contains("<v:stroke") && xml.contains(r#"endarrow="block""#) && xml.contains(r#"dashstyle="dash""#), "{xml}");
    assert!(xml.contains("<w:t>Old text box</w:t>"));
    // A plain VML rectangle changes shape.
    let mut doc = open_with(r#"<w:p><w:r><w:pict><v:rect id="R" style="width:50pt;height:20pt"/></w:pict></w:r></w:p>"#);
    let p = first_para(&doc);
    ops(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":0},"geometry":"roundRect"}));
    assert_eq!(shape(&doc, p, 0)["geometry"], "roundRect");
}

#[test]
fn set_shape_rejects_bad_input() {
    let (mut doc, p) = blank_with_text("x");
    ops(&mut doc, json!({"op":"insert_shape","at":{"p":p,"o":0},"geometry":"rect","w":40,"h":20}));
    for bad in [json!({"wrap":"floaty"}), json!({"fill":"red-ish"}), json!({"geometry":"nope"}), json!({"v_anchor":"sideways"}), json!({"w":5000.0}), json!({"h":0.0}), json!({"line":{"color":"x"}})] {
        let mut op = json!({"op":"set_shape","at":{"p":p,"o":0}});
        op.as_object_mut().unwrap().extend(bad.as_object().unwrap().clone());
        err(&mut doc, op);
    }
    assert!(err(&mut doc, json!({"op":"set_shape","at":{"p":p,"o":3},"w":5})).contains("no shape"));
    assert_eq!(shape(&doc, p, 0)["w"], 40.0);
}

// ---------------------------------------------------------------------------
// Shapes in headers
// ---------------------------------------------------------------------------

#[test]
fn a_text_box_in_a_header_is_editable_and_rewrites_only_the_header() {
    let mut blank = Document::new_blank();
    ops(&mut blank, json!({"op":"add_header_footer","kind":"header"}));
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let hdr = String::from_utf8(pkg.get("word/header1.xml").unwrap().to_vec()).unwrap();
    // The text box paragraph of the helper, without its surrounding text.
    let tb = text_box_xml(&["Header box"]);
    let runs = &tb[tb.find("<w:r><w:rPr><w:noProof/>").unwrap()..tb.find("</w:r><w:r><w:t xml:space=\"preserve\"> after").unwrap() + "</w:r>".len()];
    let hdr = hdr.replacen("<w:hdr ", &format!("<w:hdr {NS} "), 1).replacen("</w:pPr></w:p>", &format!("</w:pPr>{runs}</w:p>"), 1);
    assert!(hdr.contains("Header box"), "{hdr}");
    pkg.set("word/header1.xml", hdr.into_bytes());
    let before_main = part(&pkg.to_bytes().unwrap(), "word/document.xml");
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let boxes = box_paras(&doc);
    assert_eq!(boxes.len(), 1, "the header's text box has a story");
    let (id, _) = boxes[0].clone();
    // The page setup shows the shape, with its text, in the header.
    let ps = doc.page_setup();
    let RBlock::Para(hp) = &ps.headers["default"].blocks[0] else { panic!() };
    let sh = hp.segs.iter().find_map(|s| s.shape.as_ref()).expect("a shape in the header");
    assert_eq!(sh.text.as_ref().unwrap().len(), 1);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":"My "}));
    assert!(doc.render_changed().stories, "the header changed");
    let out = doc.save().unwrap();
    assert_valid(&out);
    let header = part(&out, "word/header1.xml");
    assert_eq!(header.matches("My Header box").count(), 2, "{header}");
    assert_eq!(part(&out, "word/document.xml"), before_main, "the body is not rewritten");
    // Moving the shape in the header works too.
    let host = {
        let ps = doc.page_setup();
        let RBlock::Para(hp) = &ps.headers["default"].blocks[0] else { panic!() };
        (hp.id, hp.segs.iter().find(|s| s.shape.is_some()).unwrap().o)
    };
    ops(&mut doc, json!({"op":"set_shape","at":{"p":host.0,"o":host.1},"w":90}));
    assert!(part(&doc.save().unwrap(), "word/header1.xml").contains(r#"cx="1143000""#));
}

#[test]
fn replacing_a_font_reaches_text_in_text_boxes_and_both_copies() {
    let body = text_box_xml(&["Plain"]).replace("<w:r><w:t>Plain</w:t></w:r>", r#"<w:r><w:rPr><w:rFonts w:ascii="Papyrus" w:hAnsi="Papyrus"/></w:rPr><w:t>Plain</w:t></w:r>"#);
    let mut doc = open_with(&body);
    ops(&mut doc, json!({"op":"replace_font","from":"Papyrus","to":"Arial"}));
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches(r#"w:ascii="Arial""#).count(), 2, "{xml}");
    assert!(!xml.contains("Papyrus"));
    assert!(doc.undo());
    assert_eq!(doc_xml(&mut doc).matches("Papyrus").count(), 4, "ascii and hAnsi, in both copies");
}

// ---------------------------------------------------------------------------
// AI tools and DocScript
// ---------------------------------------------------------------------------

#[test]
fn docscript_shows_shapes_and_the_text_inside_text_boxes_with_ids() {
    let mut doc = open_with(&text_box_xml(&["First line", "Second line"]));
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    let (a, b) = (box_paras(&doc)[0].0, box_paras(&doc)[1].0);
    assert!(script.contains(r#"Before [shape: textbox "First line Second line"] after"#), "{script}");
    assert!(script.contains(&format!("\n  [p{a}] First line\n  [p{b}] Second line\n")), "{script}");
    // A shape without text.
    let mut doc = open_with(STYLED_RECT);
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains("[shape: roundRect]"), "{script}");
    let mut doc = open_with(ELLIPSE_WITH_TEXT);
    assert!(call_tool(&mut doc, "read_document", &json!({})).unwrap().contains(r#"[shape: ellipse (gradient fill) "Hello oval"]"#));
    let mut doc = open_with(GROUP);
    assert!(call_tool(&mut doc, "read_document", &json!({})).unwrap().contains("[shape: group of 2]"));
}

#[test]
fn ai_tools_insert_text_boxes_and_shapes_and_edit_their_text() {
    let (mut doc, p) = blank_with_text("Intro text");
    let out = call_tool(&mut doc, "insert_text_box", &json!({"id":p,"after_text":"Intro","text":"Note one\nNote two","width":150})).unwrap();
    let boxes = box_paras(&doc);
    assert_eq!(boxes.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(), ["Note one", "Note two"]);
    assert!(out.contains(&format!("p{}", boxes[0].0)) && out.contains(r#"[shape: textbox "Note one Note two"]"#), "{out}");
    // The ordinary text tools reach the text inside.
    let id = boxes[1].0;
    let out = call_tool(&mut doc, "replace_text", &json!({"ids":[id],"find":"two","replace":"2"})).unwrap();
    assert!(out.to_lowercase().contains("replaced") || out.contains("2"), "{out}");
    assert_eq!(box_paras(&doc)[1].1, "Note 2");
    let out = call_tool(&mut doc, "rewrite_paragraph", &json!({"id":id,"markdown":"**Bold** note"})).unwrap();
    assert!(out.contains("Bold"), "{out}");
    assert_eq!(box_paras(&doc)[1].1, "Bold note");
    let xml = doc_xml(&mut doc);
    assert_eq!(xml.matches("Bold").count(), 2);

    let out = call_tool(&mut doc, "insert_shape", &json!({"id":p,"shape":"oval","text":"Round","fill":"FFE699","width":90,"height":40})).unwrap();
    assert!(out.contains(r#"[shape: ellipse "Round"]"#), "{out}");
    let out = call_tool(&mut doc, "insert_shape", &json!({"id":p,"shape":"rightArrow","floating":false})).unwrap();
    assert!(out.contains("[shape: rightArrow]"), "{out}");
    assert!(call_tool(&mut doc, "insert_shape", &json!({"id":p,"shape":"blob"})).unwrap_err().contains("unknown shape"));
    assert!(call_tool(&mut doc, "insert_text_box", &json!({"id":p})).is_err());
    // The tools are listed.
    let defs = docxtor_core::ai::tool_definitions();
    let names: Vec<&str> = defs.as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"insert_text_box") && names.contains(&"insert_shape"));
}

#[test]
fn search_and_diff_reach_text_in_text_boxes() {
    let mut doc = open_with(&text_box_xml(&["Needle in the box"]));
    let out = call_tool(&mut doc, "search", &json!({"query":"Needle"})).unwrap();
    assert!(out.contains("Needle in the box"), "{out}");
    let cp = doc.checkpoint();
    let (id, _) = box_paras(&doc)[0].clone();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":id,"o":0},"text":"A "}));
    let d = doc.diff_since(cp);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].2.as_deref(), Some("A Needle in the box"));
}

#[test]
fn op_json_round_trips() {
    for j in [
        json!({"op":"insert_shape","at":{"p":1,"o":0},"geometry":"rect","w":10.0,"h":20.0,"fill":"none","line":"none","floating":true}),
        json!({"op":"insert_shape","at":{"p":1,"o":0},"geometry":"line","w":10.0,"h":0.0,"line":{"color":"FF0000","width_pt":2.0,"tail_arrow":"triangle"},"text":"t","floating":false,"x":1.0,"y":2.0}),
        json!({"op":"insert_text_box","at":{"p":1,"o":3},"w":10.0,"h":20.0,"text":"a","floating":true}),
        json!({"op":"set_shape","at":{"p":1,"o":3},"w":5.0,"rot":10.0,"fill":"none","line":"none","v_anchor":"middle"}),
    ] {
        let op: docxtor_core::Op = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(serde_json::to_value(&op).unwrap(), j);
    }
}
