//! The text frame of a shape: `wps:bodyPr` (insets, anchor, wrap, autofit, vertical text, columns) and the
//! VML text box that says the same where VML can.

use super::appearance::BodyPatch;
use super::{RInsets, RShape, EMU_PER_PT};
use crate::marks::style_prop;
use crate::props::put_ordered;
use crate::xml::{Element, Node};

const BODY_ORDER: &[&str] = &["a:prstTxWarp", "a:noAutofit", "a:normAutofit", "a:spAutoFit", "a:scene3d", "a:sp3d", "a:flatTx", "a:extLst"];
pub const VERTS: &[&str] = &["horz", "vert", "vert270", "eaVert", "wordArtVert", "mongolianVert", "wordArtVertRtl"];

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on" | "t"))
}

/// Read a `wps:bodyPr` into the shape.
pub fn read(b: &Element, sh: &mut RShape) {
    let ins = |a: &str, d: f64| b.attr(a).and_then(|v| v.parse::<f64>().ok()).map(|v| v / EMU_PER_PT).unwrap_or(d);
    sh.insets = RInsets { l: ins("lIns", 7.2), t: ins("tIns", 3.6), r: ins("rIns", 7.2), b: ins("bIns", 3.6) };
    sh.v_anchor = match b.attr("anchor") {
        Some("ctr") => "middle",
        Some("b") => "bottom",
        _ => "top",
    };
    sh.anchor_center = truthy(b.attr("anchorCtr"));
    sh.wrap_text = b.attr("wrap") != Some("none");
    sh.vert = b.attr("vert").filter(|v| *v != "horz").map(String::from);
    sh.upright = truthy(b.attr("upright"));
    sh.auto_fit = b.child("a:spAutoFit").is_some();
    if sh.auto_fit {
        sh.auto_fit_mode = Some("shape");
    } else if let Some(n) = b.child("a:normAutofit") {
        sh.auto_fit_mode = Some("text");
        let pct = |a: &str| n.attr(a).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 1000.0);
        sh.font_scale = pct("fontScale").filter(|v| (v - 100.0).abs() > 1e-6);
        sh.line_spacing_reduction = pct("lnSpcReduction").filter(|v| *v != 0.0);
    }
    let cols = b.attr("numCol").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1);
    if cols > 1 {
        sh.columns = Some(cols);
        sh.column_gap_pt = b.attr("spcCol").and_then(|v| v.parse::<f64>().ok()).map(|v| v / EMU_PER_PT);
    }
}

/// Read the VML text box and the text settings in the shape's style.
pub fn read_vml(style: &str, tb: Option<&Element>, sh: &mut RShape) {
    if let Some(tb) = tb {
        let ts = tb.attr("style").unwrap_or("");
        sh.auto_fit = style_prop(ts, "mso-fit-shape-to-text") == Some("t");
        if sh.auto_fit {
            sh.auto_fit_mode = Some("shape");
        }
        sh.vert = match (style_prop(ts, "layout-flow"), style_prop(ts, "mso-layout-flow-alt")) {
            (Some("vertical"), Some("bottom-to-top")) => Some("vert270".to_string()),
            (Some("vertical"), _) => Some("vert".to_string()),
            (Some("vertical-ideographic"), _) => Some("eaVert".to_string()),
            _ => None,
        };
    }
    let anchor = style_prop(style, "v-text-anchor").unwrap_or("top");
    sh.anchor_center = anchor.ends_with("-center");
    sh.v_anchor = match anchor.split('-').next() {
        Some("middle") => "middle",
        Some("bottom") => "bottom",
        _ => "top",
    };
    sh.wrap_text = style_prop(style, "mso-wrap-style") != Some("none");
}

/// Check a body patch before anything is changed.
pub fn check(b: &BodyPatch) -> Result<(), String> {
    for (name, v) in [("ins_l", b.ins_l), ("ins_t", b.ins_t), ("ins_r", b.ins_r), ("ins_b", b.ins_b)] {
        if let Some(v) = v {
            if !(0.0..=1584.0).contains(&v) {
                return Err(format!("{name} must be between 0 and 1584 points"));
            }
        }
    }
    if let Some(a) = &b.v_anchor {
        if !matches!(a.as_str(), "top" | "middle" | "bottom") {
            return Err(format!("unknown v_anchor '{a}'; use top, middle or bottom"));
        }
    }
    if let Some(a) = &b.auto_fit {
        if !matches!(a.as_str(), "none" | "shape" | "text") {
            return Err(format!("unknown auto_fit '{a}'; use none, shape or text"));
        }
    }
    if let Some(v) = &b.vert {
        if !VERTS.contains(&v.as_str()) {
            return Err(format!("unknown vert '{v}'; use {}", VERTS.join(", ")));
        }
    }
    if let Some(c) = b.columns {
        if !(1..=16).contains(&c) {
            return Err("columns must be between 1 and 16".into());
        }
    }
    if let Some(g) = b.column_gap_pt {
        if !(0.0..=1584.0).contains(&g) {
            return Err("column_gap_pt must be between 0 and 1584 points".into());
        }
    }
    if let Some(f) = b.font_scale {
        if !(1.0..=100.0).contains(&f) {
            return Err("font_scale must be between 1 and 100 percent".into());
        }
    }
    if let Some(l) = b.line_spacing_reduction {
        if !(0.0..=100.0).contains(&l) {
            return Err("line_spacing_reduction must be between 0 and 100 percent".into());
        }
    }
    Ok(())
}

fn emu(pt: f64) -> String {
    ((pt * EMU_PER_PT).round() as i64).to_string()
}

/// Write the settings of a body patch into a `wps:bodyPr`.
pub fn apply(wsp: &mut Element, b: &BodyPatch) {
    if wsp.child("wps:bodyPr").is_none() {
        let at = wsp.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "wps:extLst")).unwrap_or(wsp.children.len());
        wsp.children.insert(at, Node::Element(Element::new("wps:bodyPr")));
    }
    let bp = wsp.child_mut("wps:bodyPr").expect("just added");
    for (attr, v) in [("lIns", b.ins_l), ("tIns", b.ins_t), ("rIns", b.ins_r), ("bIns", b.ins_b)] {
        if let Some(v) = v {
            bp.set_attr(attr, emu(v));
        }
    }
    if let Some(a) = &b.v_anchor {
        bp.set_attr("anchor", match a.as_str() {
            "middle" => "ctr",
            "bottom" => "b",
            _ => "t",
        });
    }
    if let Some(c) = b.anchor_center {
        bp.set_attr("anchorCtr", if c { "1" } else { "0" });
    }
    if let Some(w) = b.wrap_text {
        bp.set_attr("wrap", if w { "square" } else { "none" });
    }
    if let Some(v) = &b.vert {
        bp.set_attr("vert", v.as_str());
    }
    if let Some(c) = b.columns {
        bp.set_attr("numCol", c.to_string());
    }
    if let Some(g) = b.column_gap_pt {
        bp.set_attr("spcCol", emu(g));
    }
    if let Some(u) = b.upright {
        if u {
            bp.set_attr("upright", "1");
        } else {
            bp.remove_attr("upright");
        }
    }
    let fit = b.auto_fit.as_deref();
    if fit.is_some() || (b.font_scale.is_some() || b.line_spacing_reduction.is_some()) && bp.child("a:normAutofit").is_some() {
        let mode = fit.unwrap_or("text");
        let old = bp.child("a:normAutofit").cloned();
        for n in ["a:noAutofit", "a:normAutofit", "a:spAutoFit"] {
            bp.remove_children(n);
        }
        let el = match mode {
            "shape" => Element::new("a:spAutoFit"),
            "text" => {
                let mut n = Element::new("a:normAutofit");
                let scale = b.font_scale.or_else(|| old.as_ref().and_then(|o| o.attr("fontScale")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 1000.0));
                if let Some(s) = scale.filter(|s| (s - 100.0).abs() > 1e-6) {
                    n.set_attr("fontScale", ((s * 1000.0).round() as i64).to_string());
                }
                let red = b.line_spacing_reduction.or_else(|| old.as_ref().and_then(|o| o.attr("lnSpcReduction")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 1000.0));
                if let Some(r) = red.filter(|r| *r != 0.0) {
                    n.set_attr("lnSpcReduction", ((r * 1000.0).round() as i64).to_string());
                }
                n
            }
            _ => Element::new("a:noAutofit"),
        };
        put_ordered(bp, el, BODY_ORDER);
    }
}

fn pt_str(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}pt", r as i64)
    } else {
        format!("{r}pt")
    }
}

/// The VML side of a body patch: the style of the text box (`inset`, fit, flow) and the shape's text anchor and
/// wrap. `cur` supplies the insets a partial change builds on.
pub fn apply_vml(shape: &mut Element, b: &BodyPatch, cur: &RShape) {
    let mut style = super::patch::style_pairs(shape.attr("style").unwrap_or(""));
    if let Some(w) = b.wrap_text {
        super::patch::put(&mut style, "mso-wrap-style", if w { "square" } else { "none" });
    }
    if b.v_anchor.is_some() || b.anchor_center.is_some() {
        let a = b.v_anchor.clone().unwrap_or_else(|| cur.v_anchor.to_string());
        let centre = b.anchor_center.unwrap_or(cur.anchor_center);
        super::patch::put(&mut style, "v-text-anchor", if centre { format!("{a}-center") } else { a });
    }
    shape.set_attr("style", super::patch::style_string(&style));
    let Some(tb) = shape.child_mut("v:textbox") else { return };
    if b.ins_l.is_some() || b.ins_t.is_some() || b.ins_r.is_some() || b.ins_b.is_some() {
        let i = &cur.insets;
        let inset = format!("{},{},{},{}", pt_str(b.ins_l.unwrap_or(i.l)), pt_str(b.ins_t.unwrap_or(i.t)), pt_str(b.ins_r.unwrap_or(i.r)), pt_str(b.ins_b.unwrap_or(i.b)));
        tb.set_attr("inset", inset);
    }
    let mut ts = super::patch::style_pairs(tb.attr("style").unwrap_or(""));
    if let Some(a) = &b.auto_fit {
        if a == "shape" {
            super::patch::put(&mut ts, "mso-fit-shape-to-text", "t");
        } else {
            super::patch::drop_key(&mut ts, "mso-fit-shape-to-text");
        }
    }
    if let Some(v) = &b.vert {
        super::patch::drop_key(&mut ts, "layout-flow");
        super::patch::drop_key(&mut ts, "mso-layout-flow-alt");
        match v.as_str() {
            "vert" | "wordArtVert" | "mongolianVert" | "wordArtVertRtl" => super::patch::put(&mut ts, "layout-flow", "vertical"),
            "vert270" => {
                super::patch::put(&mut ts, "layout-flow", "vertical");
                super::patch::put(&mut ts, "mso-layout-flow-alt", "bottom-to-top");
            }
            "eaVert" => super::patch::put(&mut ts, "layout-flow", "vertical-ideographic"),
            _ => {}
        }
    }
    if ts.is_empty() {
        tb.remove_attr("style");
    } else {
        tb.set_attr("style", super::patch::style_string(&ts));
    }
}
