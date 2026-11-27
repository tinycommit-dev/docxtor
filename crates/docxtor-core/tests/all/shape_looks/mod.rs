//! How shapes look: fills (gradients, pictures, patterns), outlines, effects and the text frame. Reading Word's
//! markup into the view model, and the `set_shape` ops that write them back (DrawingML and the VML fallback).

use docxtor_core::package::{Package, REL_IMAGE};
use docxtor_core::{Document, Pos};
use serde_json::{json, Value};

const NS: &str = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word" mc:Ignorable="w14 wp14""#;

pub(super) fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

pub(super) fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

pub(super) fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

/// The error a `set_shape` with these fields gives.
pub(super) fn fail(doc: &mut Document, patch: Value) -> String {
    let mut op = json!({"op": "set_shape", "at": at(doc)});
    for (k, v) in patch.as_object().unwrap() {
        op[k] = v.clone();
    }
    err(doc, op)
}

pub(super) fn part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:lumMod val="110000"/><a:satMod val="105000"/><a:tint val="67000"/></a:schemeClr></a:gs><a:gs pos="50000"><a:schemeClr val="phClr"><a:lumMod val="105000"/><a:satMod val="103000"/><a:tint val="73000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:lumMod val="105000"/><a:satMod val="109000"/><a:tint val="81000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"/><a:ln w="12700"/><a:ln w="19050"/></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst><a:outerShdw blurRad="57150" dist="19050" dir="5400000" algn="ctr" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="63000"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle></a:effectStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 10, b'I', b'D', b'A', b'T', 0x78, 0x9c, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0d, 0x0a, 0x2d, 0xb4, 0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82];

/// A package with `body` in place of the blank document's empty paragraph. `with_image` adds a picture part and
/// a relationship to it; the id of the relationship is `rIdImg`.
pub(super) fn package_with(body: &str, theme: bool, with_image: bool) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    assert!(xml.contains(body), "the body was not replaced");
    pkg.set("word/document.xml", xml.into_bytes());
    if !theme && pkg.has("word/theme/theme1.xml") {
        // A new blank document carries a theme; these tests want none.
        pkg.remove_rels_to("word/document.xml", "theme/theme1.xml");
        pkg.remove("word/theme/theme1.xml");
        pkg.remove_override("word/theme/theme1.xml");
    }
    if theme {
        if pkg.has("word/theme/theme1.xml") {
            pkg.remove_rels_to("word/document.xml", "theme/theme1.xml");
        }
        pkg.add_rel("word/document.xml", docxtor_core::package::REL_THEME, "theme/theme1.xml", false);
        pkg.set("word/theme/theme1.xml", THEME.as_bytes().to_vec());
        pkg.ensure_override("word/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml");
    }
    if with_image {
        pkg.ensure_default("png", "image/png");
        pkg.set("word/media/img1.png", PNG.to_vec());
        // Write the relationship with a known id.
        let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
        let rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="rIdImg" Type="{REL_IMAGE}" Target="media/img1.png"/></Relationships>"#));
        pkg.set("word/_rels/document.xml.rels", rels.into_bytes());
    }
    pkg.to_bytes().unwrap()
}

/// A paragraph holding one 100 by 50 point shape the way Word writes it. `sppr` goes after the geometry,
/// `fallback` is the VML shape.
struct Sh {
    sppr: String,
    style: String,
    body_pr: String,
    fallback: String,
    txbx: String,
}

impl Sh {
    fn new() -> Sh {
        Sh {
            sppr: String::new(),
            style: String::new(),
            body_pr: r#"<wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="t" anchorCtr="0"><a:noAutofit/></wps:bodyPr>"#.into(),
            fallback: r##"<w:pict><v:rect id="Rectangle 1" o:spid="_x0000_s1026" style="width:100pt;height:50pt" fillcolor="#4472c4" strokecolor="#223962" strokeweight="1pt"/></w:pict>"##.into(),
            txbx: String::new(),
        }
    }
    fn sppr(mut self, s: &str) -> Sh {
        self.sppr = s.into();
        self
    }
    fn style(mut self, s: &str) -> Sh {
        self.style = s.into();
        self
    }
    fn body(mut self, s: &str) -> Sh {
        self.body_pr = s.into();
        self
    }
    fn fallback(mut self, s: &str) -> Sh {
        self.fallback = s.into();
        self
    }
    fn text(mut self, t: &str) -> Sh {
        self.txbx = format!("<wps:txbx><w:txbxContent><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:txbxContent></wps:txbx>");
        self
    }
    fn xml(&self) -> String {
        format!(
            r##"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="3" name="Rectangle 3"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{}</wps:spPr>{}{}{}</wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent></w:r></w:p>"##,
            self.sppr, self.style, self.txbx, self.body_pr, self.fallback
        )
    }
    fn open(&self) -> Document {
        Document::open(&package_with(&self.xml(), false, true)).unwrap()
    }
    fn open_themed(&self) -> Document {
        Document::open(&package_with(&self.xml(), true, true)).unwrap()
    }
}

pub(super) fn first_para(doc: &Document) -> u32 {
    doc.render_snapshot().order[0]
}

pub(super) fn shape(doc: &Document) -> Value {
    let p = first_para(doc);
    serde_json::to_value(doc.shape_at(Pos::new(p, 0)).expect("a shape")).unwrap()
}

pub(super) fn at(doc: &Document) -> Value {
    json!({"p": first_para(doc), "o": 0})
}

pub(super) fn set(doc: &mut Document, patch: Value) {
    let mut op = json!({"op": "set_shape", "at": at(doc)});
    for (k, v) in patch.as_object().unwrap() {
        op[k] = v.clone();
    }
    ops(doc, op);
}

pub(super) fn saved(doc: &mut Document) -> (Vec<u8>, String) {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml").replace("\r\n", "\n");
    (bytes, xml)
}

/// The text of `<mc:Fallback>`.
pub(super) fn fallback_of(xml: &str) -> &str {
    let i = xml.find("<mc:Fallback>").expect("a fallback");
    &xml[i..xml[i..].find("</mc:Fallback>").map(|j| i + j).unwrap()]
}

pub(super) fn approx(v: &Value, want: f64) {
    let got = v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"));
    assert!((got - want).abs() < 1e-6, "{got} != {want}");
}

mod reading;
mod writing_fills_and_outlines;
mod writing_effects_and_text_frame;
mod vml_ops_and_ai;

#[allow(unused_imports)]
pub(super) use reading::*;
#[allow(unused_imports)]
pub(super) use writing_fills_and_outlines::*;
#[allow(unused_imports)]
pub(super) use writing_effects_and_text_frame::*;
#[allow(unused_imports)]
pub(super) use vml_ops_and_ai::*;
