//! What every drawing object has in common: its name, whether it is hidden, where it sits and how it is listed.
//!
//! Word keeps the name in `wp:docPr name` and hides an object with `hidden="1"` on the same element (the
//! Selection pane's eye). The VML copy keeps the name in `id` and hides with `visibility:hidden`. Positions are
//! `wp:positionH` and `wp:positionV` (`relativeFrom` plus a `wp:align` or a `wp:posOffset`), with the same facts
//! in the VML `style`. Pure functions on the object's element; the ops are in `document/object_props.rs`.

use super::order;
use super::patch::{put, style_pairs, style_string};
use super::EMU_PER_PT;
use crate::xml::{Element, Node};
use serde::Serialize;

/// One drawing object of the text, as the Selection pane lists it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    /// The paragraph and the offset of the object (it occupies `[o, o+1)`).
    pub p: u32,
    pub o: usize,
    /// `shape`, `textbox`, `picture`, `chart`, `group` or `canvas`.
    pub kind: &'static str,
    /// The name Word shows in the Selection pane; a default such as `Picture 2` when the file gives none.
    pub name: String,
    /// The drawing's id (`wp:docPr`), 0 when there is none.
    pub id: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// `inline`, `square`, `tight`, `topBottom`, `behind` or `front`.
    pub wrap: String,
    /// Whether the object floats over the text, so it has a place in the stacking order.
    pub floating: bool,
    /// The stacking value (`relativeHeight`) of a floating object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<u64>,
    /// The object is drawn behind the text (a different stack from the objects in front of it).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub behind: bool,
    /// Where a floating object sits: what the position counts from, an alignment from that, or an offset in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel_x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel_y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on"))
}

const VML_SHAPES: &[&str] = &["v:rect", "v:roundrect", "v:oval", "v:line", "v:shape", "v:group", "v:image"];

/// What kind of drawing the object is, or None when it is something else (a note mark, a tab, a watermark).
pub fn kind_of(e: &Element) -> Option<&'static str> {
    if !matches!(e.name.as_str(), "w:drawing" | "mc:AlternateContent" | "w:pict") || crate::marks::is_watermark_pict(e) {
        return None;
    }
    if crate::chart::chart_rid(e).is_some() {
        return Some("chart");
    }
    if let Some(src) = super::read::primary(e) {
        let d = match src {
            super::read::Src::Drawing(d) => d,
            super::read::Src::Vml(p) => {
                if p.find("v:group").is_some() {
                    return Some("group");
                }
                return Some(if p.find("v:textbox").is_some() { "textbox" } else { "shape" });
            }
        };
        if d.find("wpg:wgp").is_some() {
            return Some("group");
        }
        if d.find("wpc:wpc").is_some() {
            return Some("canvas");
        }
        return Some(if d.find("wps:cNvSpPr").is_some_and(|n| truthy(n.attr("txBox"))) || d.find("wps:txbx").is_some() { "textbox" } else { "shape" });
    }
    if e.find("a:blip").is_some() || e.find("v:imagedata").is_some() {
        return Some("picture");
    }
    None
}

/// Word's own default name for a kind of object, used when the file gives none.
fn default_name(kind: &str, id: u32) -> String {
    let label = match kind {
        "textbox" => "Text Box",
        "picture" => "Picture",
        "chart" => "Chart",
        "group" => "Group",
        "canvas" => "Drawing Canvas",
        _ => "Shape",
    };
    format!("{label} {id}")
}

/// The VML shapes of the object: in the fallback of an alternate content, or the object itself when it is a `w:pict`.
fn vml_shapes(e: &Element) -> Vec<&Element> {
    let picts: Vec<&Element> = match e.name.as_str() {
        "w:pict" => vec![e],
        "mc:AlternateContent" => e.child("mc:Fallback").map(|f| f.children_named("w:pict").collect()).unwrap_or_default(),
        _ => vec![],
    };
    picts.into_iter().flat_map(|p| p.elements().filter(|c| VML_SHAPES.contains(&c.name.as_str()))).collect()
}

fn vml_shapes_mut(e: &mut Element) -> Vec<&mut Element> {
    let picts: Vec<&mut Element> = match e.name.as_str() {
        "w:pict" => vec![e],
        "mc:AlternateContent" => e.child_mut("mc:Fallback").map(|f| f.elements_mut().filter(|c| c.name == "w:pict").collect()).unwrap_or_default(),
        _ => vec![],
    };
    picts.into_iter().flat_map(|p| p.elements_mut().filter(|c| VML_SHAPES.contains(&c.name.as_str()))).collect()
}

/// The drawing's id and the name the file gives it (empty when it gives none).
fn raw_name(e: &Element) -> (u32, String) {
    if let Some(d) = e.find("wp:docPr") {
        return (d.attr("id").and_then(|v| v.parse().ok()).unwrap_or(0), d.attr("name").unwrap_or("").to_string());
    }
    // VML only: the shape's id is its name.
    let name = vml_shapes(e).first().and_then(|s| s.attr("id")).unwrap_or("").to_string();
    (0, name)
}

/// The name of an object as listed: its own, or a default.
pub fn name_of(e: &Element, kind: &str) -> String {
    let (id, name) = raw_name(e);
    if name.trim().is_empty() {
        default_name(kind, id)
    } else {
        name
    }
}

pub fn id_of(e: &Element) -> u32 {
    raw_name(e).0
}

fn vml_hidden(s: &Element) -> bool {
    s.attr("style").is_some_and(|st| style_pairs(st).iter().any(|(k, v)| k == "visibility" && v == "hidden") || style_pairs(st).iter().any(|(k, v)| k == "display" && v == "none"))
}

/// Whether the object is hidden (`hidden="1"` on its `wp:docPr`, or `visibility:hidden` in a VML-only shape).
pub fn hidden_of(e: &Element) -> bool {
    match e.find("wp:docPr") {
        Some(d) => truthy(d.attr("hidden")),
        None => vml_shapes(e).first().is_some_and(|s| vml_hidden(s)),
    }
}

/// Whether the object is a drawing that is hidden (what the renderer marks on the object's segment).
pub fn is_hidden_drawing(e: &Element) -> bool {
    matches!(e.name.as_str(), "w:drawing" | "mc:AlternateContent" | "w:pict") && hidden_of(e) && kind_of(e).is_some()
}

/// The name in a drawing's `wp:docPr`, when the file gives one.
pub fn docpr_name(e: &Element) -> Option<String> {
    e.find("wp:docPr").and_then(|d| d.attr("name")).filter(|n| !n.is_empty()).map(String::from)
}

/// Give the object a name, in the DrawingML and in the VML copy.
pub fn set_name(e: &mut Element, name: &str) {
    let old = raw_name(e).1;
    if let Some(d) = find_mut(e, "wp:docPr") {
        d.set_attr("name", name);
    }
    let has_docpr = e.find("wp:docPr").is_some();
    for s in vml_shapes_mut(e) {
        // The copy next to a DrawingML shape is renamed with it when it had the same name; a VML-only shape's id is its name.
        if !has_docpr || s.attr("id") == Some(old.as_str()) {
            s.set_attr("id", name);
        }
    }
}

/// Hide the object or show it again.
pub fn set_hidden(e: &mut Element, hidden: bool) {
    if let Some(d) = find_mut(e, "wp:docPr") {
        if hidden {
            d.set_attr("hidden", "1");
        } else {
            d.remove_attr("hidden");
        }
    }
    for s in vml_shapes_mut(e) {
        let mut style = style_pairs(s.attr("style").unwrap_or(""));
        if hidden {
            put(&mut style, "visibility", "hidden");
        } else if style.iter().any(|(k, v)| k == "visibility" && v == "hidden") {
            put(&mut style, "visibility", "visible");
        }
        style.retain(|(k, v)| !(k == "display" && v == "none" && !hidden));
        if !style.is_empty() || s.attr("style").is_some() {
            s.set_attr("style", style_string(&style));
        }
    }
}

fn find_mut<'a>(el: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    for c in el.elements_mut() {
        if c.name == name {
            return Some(c);
        }
        if let Some(f) = find_mut(c, name) {
            return Some(f);
        }
    }
    None
}

/// The wrap of a floating or inline object: DrawingML's, or for a VML-only shape what its style says.
pub fn wrap_of(e: &Element) -> String {
    let w = crate::image::layout(e).wrap;
    if w != "inline" || e.find("wp:inline").is_some() {
        return w;
    }
    match order::stack_of(e) {
        Some(s) => if s.behind { "behind" } else { "front" }.into(),
        None => "inline".into(),
    }
}

/// How the object is listed, or None when it is not a drawing.
pub fn info(e: &Element, p: u32, o: usize) -> Option<ObjectInfo> {
    let kind = kind_of(e)?;
    let stack = order::stack_of(e);
    let layout = crate::image::layout(e);
    Some(ObjectInfo {
        p,
        o,
        kind,
        name: name_of(e, kind),
        id: id_of(e),
        hidden: hidden_of(e),
        wrap: wrap_of(e),
        floating: stack.is_some(),
        z: stack.map(|s| s.z),
        behind: stack.is_some_and(|s| s.behind),
        rel_x: layout.rel_x,
        rel_y: layout.rel_y,
        align: layout.align,
        align_y: layout.align_y,
        x: layout.x,
        y: layout.y,
    })
}

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// A new position on one axis. `align` is a `wp:align` word (`left`, `center`, `right`, `inside`, `outside` across;
/// `top`, `center`, `bottom`, `inside`, `outside` down); otherwise `offset` is points from `rel`.
#[derive(Debug, Clone, Default)]
pub struct AxisPos {
    pub rel: Option<String>,
    pub align: Option<String>,
    pub offset: Option<f64>,
}

impl AxisPos {
    pub fn is_empty(&self) -> bool {
        self.rel.is_none() && self.align.is_none() && self.offset.is_none()
    }
}

const REL_H: &[&str] = &["margin", "page", "column", "character", "leftMargin", "rightMargin", "insideMargin", "outsideMargin"];
const REL_V: &[&str] = &["margin", "page", "paragraph", "line", "topMargin", "bottomMargin", "insideMargin", "outsideMargin"];
const ALIGN_H: &[&str] = &["left", "center", "right", "inside", "outside"];
const ALIGN_V: &[&str] = &["top", "center", "bottom", "inside", "outside"];

/// Check a position before anything is changed.
pub fn check_axis(a: &AxisPos, horizontal: bool) -> Result<(), String> {
    let (rels, aligns) = if horizontal { (REL_H, ALIGN_H) } else { (REL_V, ALIGN_V) };
    if let Some(r) = &a.rel {
        if !rels.contains(&r.as_str()) {
            return Err(format!("unknown position base '{r}'; use {}", rels.join(", ")));
        }
    }
    if let Some(al) = &a.align {
        if !aligns.contains(&al.as_str()) {
            return Err(format!("unknown alignment '{al}'; use {}", aligns.join(", ")));
        }
    }
    if a.offset.is_some_and(|v| !v.is_finite() || v.abs() > 1e5) {
        return Err("the offset is off the page".into());
    }
    Ok(())
}

fn vml_rel(rel: &str) -> &'static str {
    match rel {
        "page" => "page",
        "margin" => "margin",
        "leftMargin" => "left-margin-area",
        "rightMargin" => "right-margin-area",
        "topMargin" => "top-margin-area",
        "bottomMargin" => "bottom-margin-area",
        "insideMargin" => "inner-margin-area",
        "outsideMargin" => "outer-margin-area",
        "character" => "char",
        "line" => "line",
        _ => "text",
    }
}

fn set_axis(anchor: &mut Element, name: &str, a: &AxisPos, horizontal: bool) -> Result<(), String> {
    let default_rel = if horizontal { "column" } else { "paragraph" };
    let pos = anchor.child_mut(name).ok_or("the object does not float; give it a wrapping other than in line first")?;
    let rel = a.rel.clone().or_else(|| pos.attr("relativeFrom").map(String::from)).unwrap_or_else(|| default_rel.to_string());
    pos.set_attr("relativeFrom", rel);
    if let Some(al) = &a.align {
        pos.children = vec![Node::Element(Element::new("wp:align").with_text(al.as_str()))];
    } else if let Some(v) = a.offset {
        pos.children = vec![Node::Element(Element::new("wp:posOffset").with_text(((v * EMU_PER_PT).round() as i64).to_string()))];
    }
    Ok(())
}

fn set_axis_vml(s: &mut Element, a: &AxisPos, horizontal: bool, rel_now: &str) {
    let mut style = style_pairs(s.attr("style").unwrap_or(""));
    let (margin, mode, mode_rel) = if horizontal { ("margin-left", "mso-position-horizontal", "mso-position-horizontal-relative") } else { ("margin-top", "mso-position-vertical", "mso-position-vertical-relative") };
    put(&mut style, mode_rel, vml_rel(rel_now));
    if let Some(al) = &a.align {
        put(&mut style, mode, al.as_str());
        put(&mut style, margin, "0");
    } else if let Some(v) = a.offset {
        put(&mut style, mode, "absolute");
        put(&mut style, margin, super::patch::pt(v));
    }
    s.set_attr("style", style_string(&style));
}

/// Move a floating object: DrawingML `wp:positionH` and `wp:positionV`, and the VML copy in step.
pub fn set_position(e: &mut Element, x: &AxisPos, y: &AxisPos) -> Result<(), String> {
    check_axis(x, true)?;
    check_axis(y, false)?;
    if order::stack_of(e).is_none() {
        return Err("only an object that floats over the text can be placed on the page; give it a wrapping other than in line first".into());
    }
    let rel_now = |anchor: Option<&Element>, name: &str, a: &AxisPos, d: &str| a.rel.clone().or_else(|| anchor.and_then(|n| n.child(name)).and_then(|p| p.attr("relativeFrom")).map(String::from)).unwrap_or_else(|| d.to_string());
    let (rel_h, rel_v) = {
        let anchor = e.find("wp:anchor");
        (rel_now(anchor, "wp:positionH", x, "column"), rel_now(anchor, "wp:positionV", y, "paragraph"))
    };
    if let Some(anchor) = find_mut(e, "wp:anchor") {
        if !x.is_empty() {
            set_axis(anchor, "wp:positionH", x, true)?;
        }
        if !y.is_empty() {
            set_axis(anchor, "wp:positionV", y, false)?;
        }
    }
    for s in vml_shapes_mut(e) {
        if !x.is_empty() {
            set_axis_vml(s, x, true, &rel_h);
        }
        if !y.is_empty() {
            set_axis_vml(s, y, false, &rel_v);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn parse(src: &str) -> Element {
        xml::parse(src.as_bytes()).unwrap().root
    }

    const ANCHORED: &str = r#"<mc:AlternateContent xmlns:mc="mc" xmlns:wp="wp" xmlns:w="w" xmlns:a="a" xmlns:wps="wps" xmlns:v="v"><mc:Choice Requires="wps"><w:drawing><wp:anchor behindDoc="0" relativeHeight="5"><wp:positionH relativeFrom="column"><wp:posOffset>12700</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:wrapNone/><wp:docPr id="7" name="Rectangle 7"/><a:graphic><a:graphicData><wps:wsp><wps:cNvSpPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect id="Rectangle 7" style="position:absolute;margin-left:1pt;margin-top:0;width:100pt;height:50pt;z-index:5;visibility:visible;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text"/></w:pict></mc:Fallback></mc:AlternateContent>"#;

    #[test]
    fn name_and_hidden_in_both_forms() {
        let mut e = parse(ANCHORED);
        assert_eq!(kind_of(&e), Some("shape"));
        assert_eq!(name_of(&e, "shape"), "Rectangle 7");
        assert!(!hidden_of(&e));
        set_name(&mut e, "Logo");
        set_hidden(&mut e, true);
        assert_eq!(name_of(&e, "shape"), "Logo");
        assert!(hidden_of(&e));
        let docpr = e.find("wp:docPr").unwrap();
        assert_eq!(docpr.attr("hidden"), Some("1"));
        let rect = e.find("v:rect").unwrap();
        assert_eq!(rect.attr("id"), Some("Logo"));
        assert!(rect.attr("style").unwrap().contains("visibility:hidden"));
        set_hidden(&mut e, false);
        assert_eq!(e.find("wp:docPr").unwrap().attr("hidden"), None);
        assert!(e.find("v:rect").unwrap().attr("style").unwrap().contains("visibility:visible"));
    }

    #[test]
    fn default_names_follow_the_kind() {
        assert_eq!(default_name("picture", 2), "Picture 2");
        assert_eq!(default_name("textbox", 4), "Text Box 4");
        assert_eq!(default_name("shape", 3), "Shape 3");
    }

    #[test]
    fn position_by_alignment_and_by_offset() {
        let mut e = parse(ANCHORED);
        set_position(&mut e, &AxisPos { rel: Some("margin".into()), align: Some("center".into()), offset: None }, &AxisPos { rel: Some("page".into()), align: Some("bottom".into()), offset: None }).unwrap();
        let a = e.find("wp:anchor").unwrap();
        let h = a.child("wp:positionH").unwrap();
        assert_eq!(h.attr("relativeFrom"), Some("margin"));
        assert_eq!(h.child("wp:align").unwrap().text(), "center");
        assert!(h.child("wp:posOffset").is_none());
        let v = a.child("wp:positionV").unwrap();
        assert_eq!((v.attr("relativeFrom"), v.child("wp:align").unwrap().text().as_str()), (Some("page"), "bottom"));
        let style = e.find("v:rect").unwrap().attr("style").unwrap().to_string();
        assert!(style.contains("mso-position-horizontal:center") && style.contains("mso-position-horizontal-relative:margin"), "{style}");
        assert!(style.contains("mso-position-vertical:bottom") && style.contains("mso-position-vertical-relative:page"), "{style}");
        // Back to an offset: the alignment goes.
        set_position(&mut e, &AxisPos { rel: None, align: None, offset: Some(36.0) }, &AxisPos::default()).unwrap();
        let h = e.find("wp:positionH").unwrap();
        assert_eq!(h.child("wp:posOffset").unwrap().text(), "457200");
        assert!(h.child("wp:align").is_none());
        assert!(e.find("v:rect").unwrap().attr("style").unwrap().contains("margin-left:36pt"));
    }

    #[test]
    fn an_object_in_line_has_no_position() {
        let mut e = parse(r#"<w:drawing xmlns:w="w" xmlns:wp="wp"><wp:inline><wp:docPr id="1" name="Picture 1"/></wp:inline></w:drawing>"#);
        let err = set_position(&mut e, &AxisPos { rel: None, align: Some("left".into()), offset: None }, &AxisPos::default()).unwrap_err();
        assert!(err.contains("floats"), "{err}");
        let bad = set_position(&mut parse(ANCHORED), &AxisPos { rel: Some("elsewhere".into()), ..Default::default() }, &AxisPos::default()).unwrap_err();
        assert!(bad.contains("unknown position base"), "{bad}");
    }
}
