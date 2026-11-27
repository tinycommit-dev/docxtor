//! Editing an existing shape's markup in place: size, rotation, fill, outline, effects, text frame, preset,
//! position and wrap.
//! The DrawingML in the `mc:Choice` is changed and the VML in the `mc:Fallback` is kept in step; nothing else
//! in the element is touched.

use super::appearance::{BodyPatch, EffectsSpec, FillSpec};
use super::build::{fill_xml, line_look, ln_xml, preset, LineLook};
use super::fill::FillWrite;
use super::read::primary_in_choice;
use super::{LineSpec, RLine, RShape, EMU_PER_PT};
use crate::image::{self, ImagePatch};
use crate::props::put_ordered;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

/// Changes to a shape. Every field is optional; sizes and offsets are points, angles degrees.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rot: Option<f64>,
    /// `none`, a hex colour, or `{kind: solid | gradient | picture | pattern, ...}` (see `appearance::FillObj`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<FillSpec>,
    /// `none`, or `{color, width_pt, dash, head_arrow, tail_arrow, alpha, compound, cap, join, miter, head_w, ...}`;
    /// fields left out stay as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<LineSpec>,
    /// `none` (take every effect away), or the effects to change (shadow, glow, soft edges, reflection, bevel,
    /// extrusion); an effect left out stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<EffectsSpec>,
    /// The text frame: insets, anchor, autofit, wrap, vertical text, columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyPatch>,
    /// The drawing's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A preset name (`rect`, `ellipse`, `rightArrow`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    /// Offset from the column (horizontal) and the paragraph (vertical) for a floating shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// `inline`, `square`, `tight`, `topBottom`, `behind` or `front`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    /// Where the text sits in the shape: `top`, `middle` or `bottom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_h: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_v: Option<bool>,
    /// Alternative text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// Which member of a group or canvas the change is for: the index of the member, then of its member if that
    /// is a group too. Sizes and offsets are then points inside the box of the group that holds it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Vec<usize>>,
}

const SPPR_ORDER: &[&str] = &["a:xfrm", "a:custGeom", "a:prstGeom", "a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill", "a:ln", "a:effectLst", "a:effectDag", "a:scene3d", "a:sp3d", "a:extLst"];
const FILLS: &[&str] = &["a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill"];
const VML_SHAPES: &[&str] = &["v:rect", "v:roundrect", "v:oval", "v:line", "v:shape"];

pub(super) fn find_mut<'a>(el: &'a mut Element, name: &str) -> Option<&'a mut Element> {
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

/// The parts of a patch that change a `wps:wsp`, checked and ready to write.
pub(super) struct Look {
    /// `Some(None)` is no fill.
    pub fill: Option<Option<FillWrite>>,
    /// `Some(None)` is no outline.
    pub line: Option<Option<LineLook>>,
    pub geometry: Option<&'static str>,
    pub effects: Option<EffectsSpec>,
    /// The text frame settings, with the patch's `v_anchor` folded in.
    pub body: Option<BodyPatch>,
    pub name: Option<String>,
    /// The outer shadow as the VML copy says it (`Some(None)` takes it away).
    pub vml_shadow: Option<Option<super::appearance::RShadow>>,
}

impl Look {
    fn wsp_changes(&self) -> bool {
        self.fill.is_some() || self.line.is_some() || self.geometry.is_some() || self.effects.is_some() || self.body.is_some()
    }
}

/// The outline a shape has now, as the starting point of a partial change.
pub(super) fn line_base(l: &RLine) -> LineLook {
    LineLook {
        color: l.color.clone(),
        width_pt: l.width_pt,
        dash: (l.dash != "solid").then(|| l.dash.clone()),
        head: l.head_arrow.clone(),
        tail: l.tail_arrow.clone(),
        alpha: (l.alpha < 1.0).then_some(l.alpha),
        compound: l.compound.clone(),
        cap: l.cap.map(String::from),
        join: l.join.map(String::from),
        miter: l.miter,
        head_w: l.head_w.map(String::from),
        head_len: l.head_len.map(String::from),
        tail_w: l.tail_w.map(String::from),
        tail_len: l.tail_len.map(String::from),
        recolor: false,
        set: Default::default(),
    }
}

/// Check the appearance parts of a patch against the shape they are for (`target`: the shape, or the member of
/// a group). `rid` is the relationship id of the picture a picture fill uses.
pub(super) fn resolve(p: &ShapePatch, target: &RShape, rid: Option<&str>) -> Result<Look, String> {
    let fill = p.fill.as_ref().map(|f| super::fill::resolve(f, rid)).transpose()?;
    let geometry = p.geometry.as_deref().map(preset).transpose()?;
    let line = match &p.line {
        None => None,
        Some(spec) => Some(line_look(Some(spec), Some(target.line.as_ref().map(line_base).unwrap_or_else(|| LineLook::default_for(target.text_box))))?),
    };
    if let Some(a) = &p.v_anchor {
        if !matches!(a.as_str(), "top" | "middle" | "bottom") {
            return Err(format!("unknown v_anchor '{a}'; use top, middle or bottom"));
        }
    }
    let mut body = p.body.clone();
    if let Some(b) = &body {
        super::body::check(b)?;
    }
    if p.v_anchor.is_some() {
        let b = body.get_or_insert_with(BodyPatch::default);
        if b.v_anchor.is_none() {
            b.v_anchor = p.v_anchor.clone();
        }
    }
    if let Some(e) = &p.effects {
        super::effects::check(e)?;
    }
    if let Some(n) = &p.name {
        if n.chars().count() > 255 {
            return Err("a name can have at most 255 characters".into());
        }
    }
    let vml_shadow = p.effects.as_ref().and_then(|e| super::effects::final_shadow(e, target.effects.as_ref()));
    Ok(Look { fill, line, geometry, effects: p.effects.clone(), body, name: p.name.clone(), vml_shadow })
}

/// Apply a patch to a shape object. `cur` is the shape as read now: it supplies the values a partial change
/// builds on (an outline's colour, the current wrap).
pub fn apply(obj: &mut Element, p: &ShapePatch, cur: &RShape) -> Result<(), String> {
    apply_with(obj, p, cur, None)
}

/// As `apply`, with the relationship id of the picture a picture fill uses (the caller adds the relationship).
pub fn apply_with(obj: &mut Element, p: &ShapePatch, cur: &RShape, rid: Option<&str>) -> Result<(), String> {
    if let Some(path) = &p.member {
        return super::member::apply(obj, path, p, cur, rid);
    }
    // Check everything before changing anything.
    let look = resolve(p, cur, rid)?;
    let geometry = look.geometry;
    if let Some(w) = &p.wrap {
        if !matches!(w.as_str(), "inline" | "square" | "tight" | "topBottom" | "behind" | "front") {
            return Err(format!("unknown wrap '{w}'; use inline, square, tight, topBottom, behind or front"));
        }
    }
    for (name, v) in [("w", p.w), ("h", p.h)] {
        if let Some(v) = v {
            if !(0.0..=1584.0).contains(&v) {
                return Err(format!("{name} must be between 0 and 1584 points"));
            }
        }
    }
    let is_line_shape = super::build::is_line(geometry.unwrap_or(&cur.geometry));
    if (p.w == Some(0.0) || p.h == Some(0.0)) && !is_line_shape {
        return Err("only a line can have no width or height".into());
    }

    // VML has a shadow of its own and nothing else of the effects.
    if matches!(&look.effects, Some(EffectsSpec::Set(e)) if (super::appearance::EffectsIn { shadow: None, ..e.clone() }) != Default::default()) && drawing_mut(obj).is_none() {
        return Err("only a shadow can be saved in a shape from a file made before Word 2010 (VML only); fill, outline and text frame can too".into());
    }
    let final_w = p.w.unwrap_or(cur.w);
    let final_h = p.h.unwrap_or(cur.h);
    let rot = p.rot.map(|r| r.rem_euclid(360.0));

    // DrawingML.
    if let Some(drawing) = drawing_mut(obj) {
        // Wrap first: it may turn an inline shape into an anchored one.
        let mut wrap = p.wrap.clone();
        if wrap.is_none() && (p.x.is_some() || p.y.is_some()) && drawing.find("wp:inline").is_some() {
            wrap = Some("square".into());
        }
        if let Some(w) = &wrap {
            if image::layout(drawing).wrap != *w {
                image::apply(drawing, &ImagePatch { wrap: Some(w.clone()), ..Default::default() })?;
            }
        }
        let c = if drawing.find("wp:anchor").is_some() { find_mut(drawing, "wp:anchor") } else { find_mut(drawing, "wp:inline") }.ok_or("not a shape")?;
        // A SmartArt graphic's drawing scales with its extent: note the size it was made for first.
        if cur.kind == "diagram" && (p.w.is_some() || p.h.is_some()) {
            super::smartart::remember_size(c);
        }
        if p.w.is_some() || p.h.is_some() || rot.is_some() {
            let (cx, cy) = ((final_w * EMU_PER_PT).round() as i64, (final_h * EMU_PER_PT).round() as i64);
            if let Some(e) = c.child_mut("wp:extent") {
                e.set_attr("cx", cx.to_string());
                e.set_attr("cy", cy.to_string());
            }
            if cur.kind == "diagram" {
                super::smartart::note_extent(c);
            }
            if p.w.is_some() || p.h.is_some() {
                if let Some(pr) = main_pr(c) {
                    if let Some(x) = pr.child_mut("a:xfrm") {
                        if let Some(ext) = x.child_mut("a:ext") {
                            ext.set_attr("cx", cx.to_string());
                            ext.set_attr("cy", cy.to_string());
                        }
                    }
                }
            }
            // The room the rotated box takes outside the unrotated one.
            let angle = rot.unwrap_or(cur.rot).to_radians();
            let (bw, bh) = (final_w * angle.cos().abs() + final_h * angle.sin().abs(), final_w * angle.sin().abs() + final_h * angle.cos().abs());
            let (ex, ey) = ((((bw - final_w) / 2.0).max(0.0) * EMU_PER_PT).round() as i64, (((bh - final_h) / 2.0).max(0.0) * EMU_PER_PT).round() as i64);
            if let Some(e) = c.child_mut("wp:effectExtent") {
                for (k, v) in [("l", ex), ("r", ex), ("t", ey), ("b", ey)] {
                    e.set_attr(k, v.to_string());
                }
            }
        }
        if rot.is_some() || p.flip_h.is_some() || p.flip_v.is_some() {
            let pr = main_pr(c).ok_or("this shape can't be rotated")?;
            let x = pr.child_mut("a:xfrm").ok_or("this shape has no transform")?;
            if let Some(r) = rot {
                if r == 0.0 {
                    x.remove_attr("rot");
                } else {
                    x.set_attr("rot", ((r * 60000.0).round() as i64).to_string());
                }
            }
            for (attr, v) in [("flipH", p.flip_h), ("flipV", p.flip_v)] {
                match v {
                    Some(true) => x.set_attr(attr, "1"),
                    Some(false) => {
                        x.remove_attr(attr);
                    }
                    None => {}
                }
            }
        }
        if p.x.is_some() || p.y.is_some() {
            for (name, v, rel) in [("wp:positionH", p.x, "column"), ("wp:positionV", p.y, "paragraph")] {
                if let Some(v) = v {
                    let pos = c.child_mut(name).ok_or("the shape is not floating")?;
                    if pos.attr("relativeFrom").is_none() {
                        pos.set_attr("relativeFrom", rel);
                    }
                    pos.children = vec![Node::Element(Element::new("wp:posOffset").with_text(((v * EMU_PER_PT).round() as i64).to_string()))];
                }
            }
        }
        if let Some(alt) = &p.alt {
            if let Some(d) = c.child_mut("wp:docPr") {
                d.set_attr("descr", alt);
            }
        }
        if let Some(name) = &look.name {
            if let Some(d) = c.child_mut("wp:docPr") {
                d.set_attr("name", name);
            }
        }
        if look.wsp_changes() {
            let wsp = find_mut(c, "wps:wsp").ok_or("fill, outline, effects, text frame and preset can only be changed on a single shape, not a group")?;
            patch_wsp(wsp, &look, cur)?;
        }
    }

    // VML: the fallback of a shape that has one, or the shape itself when it is VML only.
    let vml = VmlChange { p, look: &look, cur, size: (p.w.is_some() || p.h.is_some()).then_some((final_w, final_h)), rot };
    if obj.name == "mc:AlternateContent" {
        if let Some(fb) = obj.child_mut("mc:Fallback") {
            patch_vml(fb, &vml);
        }
    } else if obj.name == "w:pict" {
        patch_vml(obj, &vml);
    }
    Ok(())
}

/// Fill, outline, effects, text frame and preset on a `wps:wsp` (a single shape, or a member of a group).
/// `cur` is the shape as it reads now.
pub(super) fn patch_wsp(wsp: &mut Element, look: &Look, cur: &RShape) -> Result<(), String> {
    if look.fill.is_some() || look.line.is_some() || look.geometry.is_some() || look.effects.is_some() {
        let themed = wsp.child("wps:style").and_then(|s| s.child("a:effectRef")).and_then(|r| r.attr("idx")).is_some_and(|i| i != "0");
        let pr = wsp.child_mut("wps:spPr").ok_or("the shape has no properties")?;
        if let Some(f) = &look.fill {
            pr.children.retain(|n| !matches!(n, Node::Element(e) if FILLS.contains(&e.name.as_str())));
            let xml = match f {
                Some(f) => f.dml.clone(),
                None => fill_xml(&None),
            };
            put_ordered(pr, parse(&xml), SPPR_ORDER);
        }
        if let Some(l) = &look.line {
            set_line(pr, l);
        }
        if let Some(g) = look.geometry {
            pr.remove_children("a:custGeom");
            put_ordered(pr, parse(&format!(r#"<a:prstGeom prst="{g}"><a:avLst/></a:prstGeom>"#)), SPPR_ORDER);
        }
        if let Some(e) = &look.effects {
            super::effects::apply(pr, e, cur.effects.as_ref(), themed)?;
        }
    }
    if let Some(b) = &look.body {
        super::body::apply(wsp, b);
    }
    Ok(())
}

const LN_ORDER: &[&str] = &["a:noFill", "a:solidFill", "a:gradFill", "a:pattFill", "a:prstDash", "a:custDash", "a:round", "a:bevel", "a:miter", "a:headEnd", "a:tailEnd", "a:extLst"];

/// Set the outline in the shape properties. An outline the file already has is changed in place, so its cap,
/// join and anything else it says stay; otherwise a new one is written.
fn set_line(pr: &mut Element, look: &Option<LineLook>) {
    let (Some(look), Some(ln)) = (look, pr.child_mut("a:ln")) else {
        put_ordered(pr, parse(&ln_xml(look)), SPPR_ORDER);
        return;
    };
    ln.set_attr("w", ((look.width_pt * EMU_PER_PT).round() as i64).to_string());
    let has_fill = ln.children.iter().any(|n| matches!(n, Node::Element(e) if FILLS.contains(&e.name.as_str())));
    let has_line_fill = ln.child("a:solidFill").is_some();
    if look.set.color.is_some() || !has_fill || (look.recolor && !has_line_fill) {
        ln.children.retain(|n| !matches!(n, Node::Element(e) if FILLS.contains(&e.name.as_str())));
        let fill = format!("<a:solidFill>{}</a:solidFill>", super::fill::clr(&look.color, look.alpha.unwrap_or(1.0)));
        put_ordered(ln, parse(&fill), LN_ORDER);
    } else if let Some(a) = look.set.alpha {
        // Only the opacity changes: the colour (maybe a theme colour with changes) stays.
        if let Some(c) = ln.child_mut("a:solidFill").and_then(|f| f.elements_mut().next()) {
            c.remove_children("a:alpha");
            if a < 1.0 {
                c.children.push(Node::Element(Element::new("a:alpha").with_attr("val", ((a * 100000.0).round() as i64).to_string())));
            }
        }
    }
    ln.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "a:prstDash" || e.name == "a:custDash"));
    if let Some(d) = &look.dash {
        put_ordered(ln, Element::new("a:prstDash").with_attr("val", d.as_str()), LN_ORDER);
    }
    let s = &look.set;
    if let Some(c) = &s.cap {
        ln.set_attr("cap", match c.as_str() {
            "round" => "rnd",
            "square" => "sq",
            _ => "flat",
        });
    }
    if let Some(c) = &s.compound {
        if c == "single" {
            ln.remove_attr("cmpd");
        } else {
            ln.set_attr("cmpd", c.as_str());
        }
    }
    if s.join.is_some() || s.miter.is_some() {
        let old_lim = ln.child("a:miter").and_then(|m| m.attr("lim")).map(String::from);
        let kind = s.join.clone().unwrap_or_else(|| "miter".into());
        for n in ["a:round", "a:bevel", "a:miter"] {
            ln.remove_children(n);
        }
        let el = match kind.as_str() {
            "round" => Element::new("a:round"),
            "bevel" => Element::new("a:bevel"),
            _ => Element::new("a:miter").with_attr("lim", s.miter.map(|m| ((m * 1000.0).round() as i64).to_string()).or(old_lim).unwrap_or_else(|| "800000".into())),
        };
        put_ordered(ln, el, LN_ORDER);
    }
    for (tag, v, w, len) in [("a:headEnd", &look.head, &look.head_w, &look.head_len), ("a:tailEnd", &look.tail, &look.tail_w, &look.tail_len)] {
        match (ln.child_mut(tag), v) {
            (Some(e), Some(t)) => e.set_attr("type", t.as_str()),
            (Some(e), None) => {
                e.remove_attr("type");
            }
            (None, Some(t)) => put_ordered(ln, Element::new(tag).with_attr("type", t.as_str()), LN_ORDER),
            (None, None) => {}
        }
        if let Some(e) = ln.child_mut(tag) {
            for (attr, val) in [("w", w), ("len", len)] {
                let touched = if attr == "w" { if tag == "a:headEnd" { s.head_w.is_some() } else { s.tail_w.is_some() } } else if tag == "a:headEnd" { s.head_len.is_some() } else { s.tail_len.is_some() };
                if touched {
                    match val {
                        Some(x) => e.set_attr(attr, x.as_str()),
                        None => {
                            e.remove_attr(attr);
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn parse(xml: &str) -> Element {
    let src = format!(r#"<w:x xmlns:a="{}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">{xml}</w:x>"#, super::build::NS_A);
    let mut root = crate::xml::parse(src.as_bytes()).expect("valid xml").root;
    let mut child = match root.children.remove(0) {
        Node::Element(e) => e,
        _ => unreachable!(),
    };
    child.attrs.retain(|(k, _)| !k.starts_with("xmlns:"));
    child
}

pub(super) fn drawing_mut(obj: &mut Element) -> Option<&mut Element> {
    match obj.name.as_str() {
        "w:drawing" => Some(obj),
        "mc:AlternateContent" => obj
            .elements_mut()
            .find(|c| c.name == "mc:Choice" && primary_in_choice(c).is_some())
            .and_then(|c| c.child_mut("w:drawing")),
        _ => None,
    }
}

/// The shape properties of the drawing's content: `wps:spPr` of a shape, `wpg:grpSpPr` of a group.
pub(super) fn main_pr(c: &mut Element) -> Option<&mut Element> {
    let gd = find_mut(c, "a:graphicData")?;
    let content = gd.elements_mut().next()?;
    match content.name.as_str() {
        "wps:wsp" => content.child_mut("wps:spPr"),
        "wpg:wgp" => content.child_mut("wpg:grpSpPr"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// VML
// ---------------------------------------------------------------------------

pub(super) struct VmlChange<'a> {
    pub p: &'a ShapePatch,
    pub look: &'a Look,
    /// The shape as it reads now (what a partial text frame change builds on).
    pub cur: &'a RShape,
    pub size: Option<(f64, f64)>,
    pub rot: Option<f64>,
}

pub(super) fn style_pairs(s: &str) -> Vec<(String, String)> {
    s.split(';').filter_map(|kv| kv.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect()
}

pub(super) fn style_string(v: &[(String, String)]) -> String {
    v.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(";")
}

pub(super) fn put(v: &mut Vec<(String, String)>, k: &str, val: impl Into<String>) {
    let val = val.into();
    match v.iter_mut().find(|(ek, _)| ek == k) {
        Some(slot) => slot.1 = val,
        None => v.push((k.to_string(), val)),
    }
}

pub(super) fn drop_key(v: &mut Vec<(String, String)>, k: &str) {
    v.retain(|(ek, _)| ek != k);
}

pub(super) fn pt(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}pt", r as i64)
    } else {
        format!("{r}pt")
    }
}

pub(super) fn patch_vml(root: &mut Element, ch: &VmlChange) {
    let picts: Vec<&mut Element> = if root.name == "w:pict" { vec![root] } else { root.elements_mut().filter(|e| e.name == "w:pict").collect() };
    for pict in picts {
        for sh in pict.elements_mut().filter(|e| VML_SHAPES.contains(&e.name.as_str())) {
            patch_vml_shape(sh, ch);
        }
    }
}

pub(super) fn patch_vml_shape(sh: &mut Element, ch: &VmlChange) {
    let p = ch.p;
    let mut style = style_pairs(sh.attr("style").unwrap_or(""));
    if let Some((w, h)) = ch.size {
        put(&mut style, "width", pt(w));
        put(&mut style, "height", pt(h));
        if sh.name == "v:line" {
            let from: Vec<f64> = sh.attr("from").unwrap_or("0,0").split(',').map(|s| crate::marks::length_pt(s).unwrap_or(0.0)).collect();
            let (fx, fy) = (from.first().copied().unwrap_or(0.0), from.get(1).copied().unwrap_or(0.0));
            sh.set_attr("to", format!("{},{}", pt(fx + w), pt(fy + h)));
        }
    }
    if let Some(r) = ch.rot {
        if r == 0.0 {
            drop_key(&mut style, "rotation");
        } else {
            put(&mut style, "rotation", format!("{}", (r * 100.0).round() / 100.0));
        }
    }
    if p.flip_h.is_some() || p.flip_v.is_some() {
        let h = p.flip_h.unwrap_or(ch.cur.flip_h);
        let v = p.flip_v.unwrap_or(ch.cur.flip_v);
        if sh.name == "v:line" {
            // A line says its direction with `from` and `to`: the flip is the order of its ends.
            drop_key(&mut style, "flip");
            let pt_of = |a: &str| -> (f64, f64) {
                let v: Vec<f64> = sh.attr(a).unwrap_or("0,0").split(',').map(|s| crate::marks::length_pt(s).unwrap_or(0.0)).collect();
                (v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0))
            };
            let (f, t) = (pt_of("from"), pt_of("to"));
            let (w, hh) = ((t.0 - f.0).abs(), (t.1 - f.1).abs());
            let (x0, y0) = (f.0.min(t.0), f.1.min(t.1));
            let (fx, tx) = if h { (x0 + w, x0) } else { (x0, x0 + w) };
            let (fy, ty) = if v { (y0 + hh, y0) } else { (y0, y0 + hh) };
            sh.set_attr("from", format!("{},{}", pt(fx), pt(fy)));
            sh.set_attr("to", format!("{},{}", pt(tx), pt(ty)));
        } else {
            match (h, v) {
                (false, false) => drop_key(&mut style, "flip"),
                (true, false) => put(&mut style, "flip", "x"),
                (false, true) => put(&mut style, "flip", "y"),
                (true, true) => put(&mut style, "flip", "x y"),
            }
        }
    }
    if let Some(w) = &p.wrap {
        if w == "inline" {
            for k in ["position", "margin-left", "margin-top", "z-index", "mso-position-horizontal", "mso-position-horizontal-relative", "mso-position-vertical", "mso-position-vertical-relative", "mso-wrap-distance-left", "mso-wrap-distance-top", "mso-wrap-distance-right", "mso-wrap-distance-bottom"] {
                drop_key(&mut style, k);
            }
            sh.remove_children("w10:wrap");
        } else {
            put(&mut style, "position", "absolute");
            if !style.iter().any(|(k, _)| k == "margin-left") {
                put(&mut style, "margin-left", "0");
            }
            if !style.iter().any(|(k, _)| k == "margin-top") {
                put(&mut style, "margin-top", "0");
            }
            let z = style.iter().find(|(k, _)| k == "z-index").and_then(|(_, v)| v.parse::<i64>().ok()).map(|z| z.abs()).unwrap_or(251659264);
            put(&mut style, "z-index", if w == "behind" { -z } else { z }.to_string());
            for (k, v) in [("mso-position-horizontal", "absolute"), ("mso-position-horizontal-relative", "text"), ("mso-position-vertical", "absolute"), ("mso-position-vertical-relative", "text")] {
                if !style.iter().any(|(ek, _)| ek == k) {
                    put(&mut style, k, v);
                }
            }
            sh.remove_children("w10:wrap");
            let kind = match w.as_str() {
                "square" => Some("square"),
                "tight" => Some("tight"),
                "topBottom" => Some("topAndBottom"),
                _ => None,
            };
            if let Some(k) = kind {
                sh.children.push(Node::Element(Element::new("w10:wrap").with_attr("type", k)));
            }
        }
    }
    if p.x.is_some() || p.y.is_some() {
        if let Some(x) = p.x {
            put(&mut style, "margin-left", pt(x));
            put(&mut style, "mso-position-horizontal", "absolute");
        }
        if let Some(y) = p.y {
            put(&mut style, "margin-top", pt(y));
            put(&mut style, "mso-position-vertical", "absolute");
        }
    }
    sh.set_attr("style", style_string(&style));
    if let Some(n) = &ch.look.name {
        sh.set_attr("id", n.as_str());
    }
    if let Some(f) = &ch.look.fill {
        sh.remove_attr("fillcolor");
        sh.remove_attr("filled");
        sh.remove_children("v:fill");
        match f {
            Some(f) => {
                sh.set_attr("fillcolor", format!("#{}", f.color));
                if let Some(v) = &f.vml {
                    let at = sh.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "v:stroke" || e.name == "v:textbox" || e.name.starts_with("w10:"))).unwrap_or(sh.children.len());
                    sh.children.insert(at, Node::Element(v.clone()));
                }
            }
            None => sh.set_attr("filled", "f"),
        }
    }
    if let Some(l) = &ch.look.line {
        sh.remove_attr("strokecolor");
        sh.remove_attr("strokeweight");
        sh.remove_attr("stroked");
        sh.remove_children("v:stroke");
        match l {
            Some(l) => {
                sh.set_attr("strokecolor", format!("#{}", l.color));
                sh.set_attr("strokeweight", pt(l.width_pt));
                let arrow = |a: &Option<String>| a.as_deref().map(|t| match t {
                    "triangle" => "block",
                    "stealth" => "classic",
                    "diamond" => "diamond",
                    "oval" => "oval",
                    _ => "open",
                });
                let dash = l.dash.as_deref().map(|d| match d {
                    "dot" | "sysDot" => "dot",
                    "lgDash" => "longdash",
                    "dashDot" | "sysDashDot" => "dashdot",
                    "lgDashDot" => "longdashdot",
                    "lgDashDotDot" | "sysDashDotDot" => "longdashdotdot",
                    _ => "dash",
                });
                let size = |v: &Option<String>, narrow: &'static str, wide: &'static str| v.as_deref().map(|v| match v {
                    "sm" => narrow,
                    "lg" => wide,
                    _ => "medium",
                });
                let extras = l.alpha.is_some() || l.compound.is_some() || l.cap.is_some() || l.join.is_some() || l.miter.is_some() || l.head_w.is_some() || l.head_len.is_some() || l.tail_w.is_some() || l.tail_len.is_some();
                if dash.is_some() || l.head.is_some() || l.tail.is_some() || extras {
                    let mut s = Element::new("v:stroke");
                    if let Some(d) = dash {
                        s.set_attr("dashstyle", d);
                    }
                    if let Some(a) = l.alpha {
                        s.set_attr("opacity", format!("{}", (a * 1000.0).round() / 1000.0));
                    }
                    if let Some(c) = &l.compound {
                        s.set_attr("linestyle", match c.as_str() {
                            "dbl" => "thinThin",
                            "tri" => "thickBetweenThin",
                            other => other,
                        });
                    }
                    if let Some(c) = &l.cap {
                        s.set_attr("endcap", c.as_str());
                    }
                    match l.join.as_deref() {
                        Some(j) => s.set_attr("joinstyle", j),
                        None if l.miter.is_some() => s.set_attr("joinstyle", "miter"),
                        None => {}
                    }
                    if let Some(m) = l.miter {
                        s.set_attr("miterlimit", format!("{}", m / 100.0));
                    }
                    if l.head.is_some() {
                        if let Some(w) = size(&l.head_w, "narrow", "wide") {
                            s.set_attr("startarrowwidth", w);
                        }
                        if let Some(w) = size(&l.head_len, "short", "long") {
                            s.set_attr("startarrowlength", w);
                        }
                    }
                    if l.tail.is_some() {
                        if let Some(w) = size(&l.tail_w, "narrow", "wide") {
                            s.set_attr("endarrowwidth", w);
                        }
                        if let Some(w) = size(&l.tail_len, "short", "long") {
                            s.set_attr("endarrowlength", w);
                        }
                    }
                    if let Some(a) = arrow(&l.head) {
                        s.set_attr("startarrow", a);
                    }
                    if let Some(a) = arrow(&l.tail) {
                        s.set_attr("endarrow", a);
                    }
                    // Before the text box, as Word orders a shape's children.
                    let at = sh.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "v:textbox" || e.name.starts_with("w10:"))).unwrap_or(sh.children.len());
                    sh.children.insert(at, Node::Element(s));
                }
            }
            None => sh.set_attr("stroked", "f"),
        }
    }
    if let Some(shadow) = &ch.look.vml_shadow {
        sh.remove_children("v:shadow");
        if let Some(s) = shadow {
            let at = sh.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "v:textbox" || e.name.starts_with("w10:"))).unwrap_or(sh.children.len());
            sh.children.insert(at, Node::Element(super::effects::vml_shadow(s)));
        }
    }
    if let Some(b) = &ch.look.body {
        super::body::apply_vml(sh, b, ch.cur);
    }
    if let Some(g) = ch.look.geometry {
        // Only the three plain VML shapes can change preset in place.
        if matches!(sh.name.as_str(), "v:rect" | "v:roundrect" | "v:oval") {
            let (name, arc) = match g {
                "roundRect" => ("v:roundrect", true),
                "ellipse" => ("v:oval", false),
                _ => ("v:rect", false),
            };
            sh.name = name.into();
            if arc {
                if sh.attr("arcsize").is_none() {
                    sh.set_attr("arcsize", "10923f");
                }
            } else {
                sh.remove_attr("arcsize");
            }
        }
    }
}
