//! Putting shapes together into a group (`wpg:wgp`) and taking a group apart again. A group is written as Word
//! writes it: an `mc:AlternateContent` whose `mc:Choice Requires="wpg"` holds the DrawingML and whose
//! `mc:Fallback` holds a VML `v:group` built from the members' own VML. When a member has no VML, the group is
//! written with the Choice only.

use super::order::stack_of;
use super::patch::{drop_key, pt, put, style_pairs, style_string};
use super::story::MARK;
use super::{read, EMU_PER_PT};
use crate::xml::{self, Element, Node};

pub const NS_WPG: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup";

const VML_SHAPES: &[&str] = &["v:rect", "v:roundrect", "v:oval", "v:line", "v:shape"];

/// The VML copy of a shape: the shape types it uses and the shape itself.
#[derive(Debug, Clone)]
pub struct Vml {
    pub types: Vec<Element>,
    pub shape: Element,
}

/// A shape that is about to join a group.
#[derive(Debug, Clone)]
pub struct Part {
    pub wsp: Element,
    pub id: String,
    pub name: String,
    pub descr: Option<String>,
    /// Top-left corner and size of the unrotated box, in points, in the group's frame.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rot: f64,
    pub z: u64,
    pub vml: Option<Vml>,
    /// The keys of the text-box stories this shape carries (see `story`).
    pub keys: Vec<String>,
}

fn emu(pt: f64) -> i64 {
    (pt * EMU_PER_PT).round() as i64
}

fn num(e: Option<&Element>, a: &str) -> f64 {
    e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

fn pos_offset(anchor: &Element, name: &str) -> Option<f64> {
    anchor.child(name)?.child("wp:posOffset")?.text().trim().parse::<f64>().ok().map(|v| v / EMU_PER_PT)
}

/// The VML shape (and the shape types it needs) of an object's fallback.
fn vml_of(obj: &Element) -> Option<Vml> {
    let pict = obj.child("mc:Fallback")?.child("w:pict")?;
    let shape = pict.elements().find(|c| VML_SHAPES.contains(&c.name.as_str()))?.clone();
    let types = pict.children_named("v:shapetype").cloned().collect();
    Some(Vml { types, shape })
}

fn parse_with(body: &str) -> Element {
    let mut decls: String = super::build::ROOT_NS.iter().map(|(p, u)| format!(r#" xmlns:{p}="{u}""#)).collect();
    decls.push_str(&format!(r#" xmlns:wpg="{NS_WPG}" xmlns:w="{}""#, super::build::NS_W));
    let src = body.replacen('>', &format!("{decls}>"), 1);
    let mut el = xml::parse(src.as_bytes()).expect("valid shape xml").root;
    el.attrs.retain(|(k, _)| !k.starts_with("xmlns:"));
    el
}

/// Read a shape object so it can join a group. `at` overrides the position (top-left of the unrotated box, in
/// points, in the frame of the first shape of the group).
pub fn take_part(obj: &Element, at: Option<(f64, f64)>, first_frame: Option<(&str, &str)>) -> Result<Part, String> {
    let d = match read::primary(obj) {
        Some(read::Src::Drawing(d)) => d,
        Some(read::Src::Vml(_)) => return Err("a shape from an old file (VML only) can't be grouped".into()),
        None => return Err("only shapes and text boxes can be grouped".into()),
    };
    let anchor = d.child("wp:anchor").ok_or("a shape in line with the text can't be grouped; wrap it first")?;
    let content = anchor.find("a:graphicData").and_then(|g| g.elements().next()).ok_or("not a shape")?;
    if content.name != "wps:wsp" {
        return Err("only single shapes and text boxes can be grouped; a group can't go into another group".into());
    }
    let ext = anchor.child("wp:extent");
    let (w, h) = (num(ext, "cx") / EMU_PER_PT, num(ext, "cy") / EMU_PER_PT);
    let rel = |name: &str| anchor.child(name).and_then(|p| p.attr("relativeFrom")).unwrap_or("").to_string();
    let (x, y) = match at {
        Some(p) => p,
        None => {
            let (rh, rv) = first_frame.unwrap_or(("", ""));
            if (rh != rel("wp:positionH") || rv != rel("wp:positionV")) && first_frame.is_some() {
                return Err("these shapes are placed from different points on the page; move one of them first".into());
            }
            (
                pos_offset(anchor, "wp:positionH").ok_or("a shape placed by alignment needs its position; move it first")?,
                pos_offset(anchor, "wp:positionV").ok_or("a shape placed by alignment needs its position; move it first")?,
            )
        }
    };
    let rot = num(content.child("wps:spPr").and_then(|s| s.child("a:xfrm")), "rot") / 60000.0;
    let pr = anchor.child("wp:docPr");
    Ok(Part {
        wsp: content.clone(),
        id: pr.and_then(|p| p.attr("id")).unwrap_or("0").to_string(),
        name: pr.and_then(|p| p.attr("name")).unwrap_or("Shape").to_string(),
        descr: pr.and_then(|p| p.attr("descr")).filter(|s| !s.is_empty()).map(String::from),
        x,
        y,
        w,
        h,
        rot,
        z: stack_of(obj).map(|s| s.z).unwrap_or(0),
        vml: vml_of(obj),
        keys: super::story::keys_of(obj),
    })
}

/// The box a part takes once rotated: left, top, right, bottom.
fn rotated_bounds(p: &Part) -> (f64, f64, f64, f64) {
    let a = p.rot.to_radians();
    let (bw, bh) = (p.w * a.cos().abs() + p.h * a.sin().abs(), p.w * a.sin().abs() + p.h * a.cos().abs());
    let (cx, cy) = (p.x + p.w / 2.0, p.y + p.h / 2.0);
    (cx - bw / 2.0, cy - bh / 2.0, cx + bw / 2.0, cy + bh / 2.0)
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

fn wrap_type(wrap: &str) -> Option<&'static str> {
    match wrap {
        "wp:wrapSquare" => Some("square"),
        "wp:wrapTight" | "wp:wrapThrough" => Some("tight"),
        "wp:wrapTopAndBottom" => Some("topAndBottom"),
        _ => None,
    }
}

const POSITION_KEYS: &[&str] = &[
    "margin-left", "margin-top", "z-index", "left", "top", "width", "height", "mso-position-horizontal", "mso-position-horizontal-relative", "mso-position-vertical",
    "mso-position-vertical-relative", "mso-wrap-distance-left", "mso-wrap-distance-top", "mso-wrap-distance-right", "mso-wrap-distance-bottom", "rotation",
];

/// A member's VML shape placed inside a group, in the group's own units.
fn vml_in_group(v: &Vml, x: i64, y: i64, w: i64, h: i64, rot: f64) -> Element {
    let mut sh = v.shape.clone();
    let mut style = style_pairs(sh.attr("style").unwrap_or(""));
    for k in POSITION_KEYS {
        drop_key(&mut style, k);
    }
    put(&mut style, "position", "absolute");
    put(&mut style, "left", x.to_string());
    put(&mut style, "top", y.to_string());
    put(&mut style, "width", w.to_string());
    put(&mut style, "height", h.to_string());
    if rot != 0.0 {
        put(&mut style, "rotation", format!("{}", (rot * 100.0).round() / 100.0));
    }
    sh.set_attr("style", style_string(&style));
    sh.remove_children("w10:wrap");
    if sh.name == "v:line" {
        sh.set_attr("from", format!("{x},{y}"));
        sh.set_attr("to", format!("{},{}", x + w, y + h));
    }
    sh
}

/// A group of the parts. `anchor` is the first part's anchor (its wrap, distances and what its position counts
/// from carry over); `z` is the stacking value of the group.
pub fn build_group(parts: &[Part], anchor: &Element, id: u32, name: &str, z: u64) -> Element {
    let (mut x1, mut y1, mut x2, mut y2) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in parts {
        let b = rotated_bounds(p);
        x1 = x1.min(b.0);
        y1 = y1.min(b.1);
        x2 = x2.max(b.2);
        y2 = y2.max(b.3);
    }
    let (w, h) = (x2 - x1, y2 - y1);
    let (cx, cy) = (emu(w), emu(h));
    let attr = |a: &str, d: &str| anchor.attr(a).unwrap_or(d).to_string();
    let rel = |n: &str, d: &str| anchor.child(n).and_then(|p| p.attr("relativeFrom")).unwrap_or(d).to_string();
    let (rel_h, rel_v) = (rel("wp:positionH", "column"), rel("wp:positionV", "paragraph"));
    let wrap_el = anchor
        .elements()
        .find(|e| e.name.starts_with("wp:wrap"))
        .cloned()
        .unwrap_or_else(|| Element::new("wp:wrapNone"));
    let uid = 1024 + id;
    let src = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:anchor distT="{}" distB="{}" distL="{}" distR="{}" simplePos="0" relativeHeight="{z}" behindDoc="{}" locked="0" layoutInCell="{}" allowOverlap="{}"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="{rel_h}"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="{rel_v}"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{}<wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{NS_WPG}"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/><a:chOff x="0" y="0"/><a:chExt cx="{cx}" cy="{cy}"/></a:xfrm></wpg:grpSpPr></wpg:wgp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice></mc:AlternateContent>"#,
        attr("distT", "0"),
        attr("distB", "0"),
        attr("distL", "114300"),
        attr("distR", "114300"),
        attr("behindDoc", "0"),
        attr("layoutInCell", "1"),
        attr("allowOverlap", "1"),
        emu(x1),
        emu(y1),
        wrap_el.to_xml(),
    );
    let mut el = parse_with(&src);
    // The members.
    let mut members = Vec::new();
    for p in parts {
        let mut m = p.wsp.clone();
        if let Some(x) = m.child_mut("wps:spPr").and_then(|s| s.child_mut("a:xfrm")) {
            if let Some(o) = x.child_mut("a:off") {
                o.set_attr("x", emu(p.x - x1).to_string());
                o.set_attr("y", emu(p.y - y1).to_string());
            }
        }
        m.remove_children("wps:cNvPr");
        let mut pr = Element::new("wps:cNvPr").with_attr("id", p.id.as_str()).with_attr("name", p.name.as_str());
        if let Some(d) = &p.descr {
            pr.set_attr("descr", d.as_str());
        }
        m.children.insert(0, Node::Element(pr));
        members.push(m);
    }
    if let Some(wgp) = find_in(&mut el, "wpg:wgp") {
        wgp.children.extend(members.into_iter().map(Node::Element));
    }
    // The VML copy, when every member has one.
    if parts.iter().all(|p| p.vml.is_some()) {
        let tw = |v: f64| (v * 20.0).round() as i64;
        let mut group = Element::new("v:group")
            .with_attr("id", name)
            .with_attr("o:spid", format!("_x0000_s{uid}"))
            .with_attr(
                "style",
                format!(
                    "position:absolute;margin-left:{};margin-top:{};width:{};height:{};z-index:{z};mso-position-horizontal:absolute;mso-position-horizontal-relative:{};mso-position-vertical:absolute;mso-position-vertical-relative:{}",
                    pt(x1),
                    pt(y1),
                    pt(w),
                    pt(h),
                    vml_rel(&rel_h),
                    vml_rel(&rel_v)
                ),
            )
            .with_attr("coordsize", format!("{},{}", tw(w), tw(h)))
            .with_attr("o:allowincell", "f");
        let mut seen: Vec<String> = Vec::new();
        for p in parts {
            for t in &p.vml.as_ref().unwrap().types {
                let tid = t.attr("id").unwrap_or("").to_string();
                if !seen.contains(&tid) {
                    seen.push(tid);
                    group.children.push(Node::Element(t.clone()));
                }
            }
        }
        for p in parts {
            let v = p.vml.as_ref().unwrap();
            group.children.push(Node::Element(vml_in_group(v, tw(p.x - x1), tw(p.y - y1), tw(p.w), tw(p.h), p.rot)));
        }
        if let Some(t) = wrap_type(&wrap_el.name) {
            group.children.push(Node::Element(Element::new("w10:wrap").with_attr("type", t)));
        }
        let fb = Element::new("mc:Fallback").with_child(Element::new("w:pict").with_child(group));
        el.children.push(Node::Element(fb));
    }
    let keys: Vec<&str> = parts.iter().flat_map(|p| p.keys.iter().map(|k| k.as_str())).collect();
    if !keys.is_empty() {
        el.set_attr(MARK, keys.join(" "));
    }
    el
}

fn find_in<'a>(e: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    super::patch::find_mut(e, name)
}

// ---------------------------------------------------------------------------
// Taking a group apart
// ---------------------------------------------------------------------------

fn count_text_boxes(e: &Element) -> usize {
    e.elements().map(|c| if c.name == "w:txbxContent" { 1 } else { count_text_boxes(c) }).sum()
}

/// The shapes a group is made of, each as an object of its own, placed where it was on the page. `at` is the
/// position of the group's top-left corner in its own frame when the group is placed by alignment.
pub fn ungroup(obj: &Element, at: Option<(f64, f64)>, rel_default: (&str, &str)) -> Result<Vec<Element>, String> {
    let Some(read::Src::Drawing(d)) = read::primary(obj) else { return Err("that is not a group".into()) };
    let c = d.child("wp:anchor").or_else(|| d.child("wp:inline")).ok_or("that is not a group")?;
    let wgp = c.find("wpg:wgp").ok_or("that is not a group")?;
    let x = wgp.child("wpg:grpSpPr").and_then(|p| p.child("a:xfrm")).ok_or("the group has no size")?;
    if x.attr("flipH").is_some() || x.attr("flipV").is_some() {
        return Err("flip the group back before taking it apart".into());
    }
    let members: Vec<&Element> = wgp.elements().filter(|e| super::member::is_member(e)).collect();
    if members.is_empty() || members.iter().any(|m| m.name != "wps:wsp") {
        return Err("only a group of shapes and text boxes can be taken apart".into());
    }
    let (ch_off, ch_ext, ext) = (x.child("a:chOff"), x.child("a:chExt"), x.child("a:ext"));
    let (gw, gh) = (num(ext, "cx"), num(ext, "cy"));
    let (cw, chh) = (num(ch_ext, "cx"), num(ch_ext, "cy"));
    let (sx, sy) = (if cw > 0.0 { gw / cw } else { 1.0 }, if chh > 0.0 { gh / chh } else { 1.0 });
    let theta = num(Some(x), "rot") / 60000.0;
    let rel = |name: &str, d: &str| c.child(name).and_then(|p| p.attr("relativeFrom")).unwrap_or(d).to_string();
    let anchored = c.name == "wp:anchor";
    let (rel_h, rel_v) = if anchored { (rel("wp:positionH", rel_default.0), rel("wp:positionV", rel_default.1)) } else { (rel_default.0.to_string(), rel_default.1.to_string()) };
    let (x0, y0) = match (at, anchored) {
        (Some(p), _) => p,
        (None, true) => (
            pos_offset(c, "wp:positionH").ok_or("a group placed by alignment needs its position; move it first")?,
            pos_offset(c, "wp:positionV").ok_or("a group placed by alignment needs its position; move it first")?,
        ),
        (None, false) => (0.0, 0.0),
    };
    let (gwp, ghp) = (gw / EMU_PER_PT, gh / EMU_PER_PT);
    let wrap_el = if anchored {
        c.elements().find(|e| e.name.starts_with("wp:wrap")).cloned().unwrap_or_else(|| Element::new("wp:wrapNone"))
    } else {
        Element::new("wp:wrapSquare").with_attr("wrapText", "bothSides")
    };
    let z = stack_of(obj).map(|s| s.z).unwrap_or(super::order::Z_BASE);
    let behind = anchored && matches!(c.attr("behindDoc"), Some("1" | "true"));
    let keys = super::story::keys_of(obj);
    let vml_members: Option<Vec<Element>> = obj.child("mc:Fallback").and_then(|f| f.child("w:pict")).and_then(|p| p.child("v:group")).map(|g| g.elements().filter(|e| VML_SHAPES.contains(&e.name.as_str())).cloned().collect());
    let vml_types: Vec<Element> = obj.child("mc:Fallback").and_then(|f| f.child("w:pict")).and_then(|p| p.child("v:group")).map(|g| g.children_named("v:shapetype").cloned().collect()).unwrap_or_default();
    let vml_members = vml_members.filter(|v| v.len() == members.len());

    let mut out = Vec::new();
    let mut key_at = 0;
    for (i, m) in members.iter().enumerate() {
        let mx = m.child("wps:spPr").and_then(|s| s.child("a:xfrm"));
        let (off, mext) = (mx.and_then(|x| x.child("a:off")), mx.and_then(|x| x.child("a:ext")));
        let (bx, by) = ((num(off, "x") - num(ch_off, "x")) * sx / EMU_PER_PT, (num(off, "y") - num(ch_off, "y")) * sy / EMU_PER_PT);
        let (bw, bh) = (num(mext, "cx") * sx / EMU_PER_PT, num(mext, "cy") * sy / EMU_PER_PT);
        let rot_m = num(mx, "rot") / 60000.0;
        // A turned group turns each member about the group's centre.
        let (px, py, rot) = if theta == 0.0 {
            (x0 + bx, y0 + by, rot_m)
        } else {
            let (dx, dy) = (bx + bw / 2.0 - gwp / 2.0, by + bh / 2.0 - ghp / 2.0);
            let (s, co) = theta.to_radians().sin_cos();
            let (rx, ry) = (dx * co - dy * s, dx * s + dy * co);
            (x0 + gwp / 2.0 + rx - bw / 2.0, y0 + ghp / 2.0 + ry - bh / 2.0, (rot_m + theta).rem_euclid(360.0))
        };
        let n_boxes = count_text_boxes(m);
        let my_keys: Vec<String> = keys.iter().skip(key_at).take(n_boxes).cloned().collect();
        key_at += n_boxes;
        let pr = m.child("wps:cNvPr");
        let id = pr.and_then(|p| p.attr("id")).unwrap_or("0");
        let name = pr.and_then(|p| p.attr("name")).unwrap_or("Shape");
        let (w_emu, h_emu) = (emu(bw), emu(bh));
        let mz = z + i as u64;
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
        let descr = pr.and_then(|p| p.attr("descr")).map(|d| format!(r#" descr="{}""#, esc(d))).unwrap_or_default();
        let src = format!(
            r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="{}" distB="{}" distL="{}" distR="{}" simplePos="0" relativeHeight="{mz}" behindDoc="{}" locked="0" layoutInCell="{}" allowOverlap="{}"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="{rel_h}"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="{rel_v}"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{w_emu}" cy="{h_emu}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{}<wp:docPr id="{}" name="{}"{descr}/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{}"></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice></mc:AlternateContent>"#,
            if anchored { c.attr("distT").unwrap_or("0") } else { "0" },
            if anchored { c.attr("distB").unwrap_or("0") } else { "0" },
            if anchored { c.attr("distL").unwrap_or("114300") } else { "114300" },
            if anchored { c.attr("distR").unwrap_or("114300") } else { "114300" },
            if behind { "1" } else { "0" },
            if anchored { c.attr("layoutInCell").unwrap_or("1") } else { "1" },
            if anchored { c.attr("allowOverlap").unwrap_or("1") } else { "1" },
            emu(px),
            emu(py),
            wrap_el.to_xml(),
            esc(id),
            esc(name),
            super::build::NS_WPS,
        );
        let mut el = parse_with(&src);
        let mut shape = (*m).clone();
        shape.remove_children("wps:cNvPr");
        if let Some(x) = shape.child_mut("wps:spPr").and_then(|s| s.child_mut("a:xfrm")) {
            if let Some(o) = x.child_mut("a:off") {
                o.set_attr("x", "0");
                o.set_attr("y", "0");
            }
            if let Some(e) = x.child_mut("a:ext") {
                e.set_attr("cx", w_emu.to_string());
                e.set_attr("cy", h_emu.to_string());
            }
            if rot == 0.0 {
                x.remove_attr("rot");
            } else {
                x.set_attr("rot", ((rot * 60000.0).round() as i64).to_string());
            }
        }
        if let Some(gd) = find_in(&mut el, "a:graphicData") {
            gd.children.push(Node::Element(shape));
        }
        if let Some(vm) = vml_members.as_ref().map(|v| &v[i]) {
            let vml = Vml { types: vml_types.clone(), shape: vm.clone() };
            let mut sh = vml_anchored(&vml, px, py, bw, bh, rot, if behind { -(mz as i64) } else { mz as i64 }, &rel_h, &rel_v);
            if let Some(t) = wrap_type(&wrap_el.name) {
                sh.children.push(Node::Element(Element::new("w10:wrap").with_attr("type", t)));
            }
            let mut pict = Element::new("w:pict");
            let mut seen: Vec<String> = Vec::new();
            for t in &vml.types {
                let tid = t.attr("id").unwrap_or("").to_string();
                let used = sh.attr("type").is_some_and(|ty| ty.trim_start_matches('#') == tid);
                if used && !seen.contains(&tid) {
                    seen.push(tid);
                    pict.children.push(Node::Element(t.clone()));
                }
            }
            pict.children.push(Node::Element(sh));
            el.children.push(Node::Element(Element::new("mc:Fallback").with_child(pict)));
        }
        if !my_keys.is_empty() {
            el.set_attr(MARK, my_keys.join(" "));
        }
        out.push(el);
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn vml_anchored(v: &Vml, x: f64, y: f64, w: f64, h: f64, rot: f64, z: i64, rel_h: &str, rel_v: &str) -> Element {
    let mut sh = v.shape.clone();
    let mut style = style_pairs(sh.attr("style").unwrap_or(""));
    for k in POSITION_KEYS {
        drop_key(&mut style, k);
    }
    put(&mut style, "position", "absolute");
    put(&mut style, "margin-left", pt(x));
    put(&mut style, "margin-top", pt(y));
    put(&mut style, "width", pt(w));
    put(&mut style, "height", pt(h));
    if rot != 0.0 {
        put(&mut style, "rotation", format!("{}", (rot * 100.0).round() / 100.0));
    }
    put(&mut style, "z-index", z.to_string());
    put(&mut style, "mso-position-horizontal", "absolute");
    put(&mut style, "mso-position-horizontal-relative", vml_rel(rel_h));
    put(&mut style, "mso-position-vertical", "absolute");
    put(&mut style, "mso-position-vertical-relative", vml_rel(rel_v));
    sh.set_attr("style", style_string(&style));
    sh.remove_children("w10:wrap");
    if sh.name == "v:line" {
        sh.set_attr("from", "0,0");
        sh.set_attr("to", format!("{},{}", pt(w), pt(h)));
    }
    sh
}

// ---------------------------------------------------------------------------
// Freehand drawing
// ---------------------------------------------------------------------------

/// A freehand drawing: strokes of points (points, in the frame of the object's position), one pen.
#[derive(Debug, Clone)]
pub struct Ink {
    pub strokes: Vec<Vec<(f64, f64)>>,
    pub color: String,
    pub width_pt: f64,
    /// How much of the colour shows (0 to 1); None for opaque.
    pub alpha: Option<f64>,
    pub cap: InkCap,
    pub id: u32,
    pub z: u64,
}

/// How a pen ends its line: a round pen, a marker or highlighter with square ends, or flat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkCap {
    Round,
    Square,
    Flat,
}

impl InkCap {
    /// `a:ln cap`.
    fn ooxml(self) -> &'static str {
        match self {
            InkCap::Round => "rnd",
            InkCap::Square => "sq",
            InkCap::Flat => "flat",
        }
    }

    /// `v:stroke endcap`.
    fn vml(self) -> &'static str {
        match self {
            InkCap::Round => "round",
            InkCap::Square => "square",
            InkCap::Flat => "flat",
        }
    }
}

/// Remove points that add nothing to the line (Ramer, Douglas and Peucker), to the given tolerance in points.
pub fn simplify(points: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    *keep.last_mut().unwrap() = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (points[a], points[b]);
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = dx.hypot(dy);
        let mut far = (0.0, a);
        for (i, p) in points.iter().enumerate().take(b).skip(a + 1) {
            let d = if len == 0.0 { (p.0 - pa.0).hypot(p.1 - pa.1) } else { ((p.0 - pa.0) * dy - (p.1 - pa.1) * dx).abs() / len };
            if d > far.0 {
                far = (d, i);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            stack.push((a, far.1));
            stack.push((far.1, b));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// The two control points and the end point of a cubic Bézier segment.
pub type Bezier = [(f64, f64); 3];

/// A smooth curve through the points as cubic Bézier segments (Catmull-Rom): the start, then control points and
/// end point of each segment. Two points make a straight segment, one a dot.
pub fn smooth(points: &[(f64, f64)]) -> ((f64, f64), Vec<Bezier>) {
    let start = points[0];
    if points.len() == 1 {
        let p = (start.0 + 0.01, start.1);
        return (start, vec![[start, p, p]]);
    }
    let at = |i: isize| points[i.clamp(0, points.len() as isize - 1) as usize];
    let mut segs = Vec::new();
    for i in 0..points.len() as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        segs.push([c1, c2, p2]);
    }
    (start, segs)
}

/// The shape of a drawing: `(markup, x, y)` where `x` and `y` are where its top-left corner goes in the frame
/// the strokes were given in.
pub fn build_ink(ink: &Ink) -> Result<(Element, f64, f64, f64, f64), String> {
    let strokes: Vec<Vec<(f64, f64)>> = ink.strokes.iter().filter(|s| !s.is_empty()).map(|s| simplify(s, 0.35)).collect();
    if strokes.is_empty() {
        return Err("there is nothing drawn".into());
    }
    let pad = (ink.width_pt / 2.0).max(0.5);
    let (mut x1, mut y1, mut x2, mut y2) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in strokes.iter().flatten() {
        x1 = x1.min(p.0);
        y1 = y1.min(p.1);
        x2 = x2.max(p.0);
        y2 = y2.max(p.1);
    }
    let (ox, oy) = (x1 - pad, y1 - pad);
    let (w, h) = ((x2 - x1) + 2.0 * pad, (y2 - y1) + 2.0 * pad);
    let (cx, cy) = (emu(w), emu(h));
    let rel = |p: (f64, f64)| (p.0 - ox, p.1 - oy);
    let mut paths = String::new();
    let mut vml_path = String::new();
    let pt_xml = |p: (f64, f64)| format!(r#"<a:pt x="{}" y="{}"/>"#, emu(p.0), emu(p.1));
    let tw = |v: f64| (v * 20.0).round() as i64;
    for s in &strokes {
        let pts: Vec<(f64, f64)> = s.iter().map(|p| rel(*p)).collect();
        let (start, segs) = smooth(&pts);
        paths.push_str(&format!(r#"<a:path w="{cx}" h="{cy}" fill="none"><a:moveTo>{}</a:moveTo>"#, pt_xml(start)));
        vml_path.push_str(&format!("m{},{}", tw(start.0), tw(start.1)));
        let straight = pts.len() == 2;
        for seg in &segs {
            if straight {
                paths.push_str(&format!("<a:lnTo>{}</a:lnTo>", pt_xml(seg[2])));
                vml_path.push_str(&format!("l{},{}", tw(seg[2].0), tw(seg[2].1)));
            } else {
                paths.push_str(&format!("<a:cubicBezTo>{}{}{}</a:cubicBezTo>", pt_xml(seg[0]), pt_xml(seg[1]), pt_xml(seg[2])));
                vml_path.push_str(&format!("c{},{},{},{},{},{}", tw(seg[0].0), tw(seg[0].1), tw(seg[1].0), tw(seg[1].1), tw(seg[2].0), tw(seg[2].1)));
            }
        }
        paths.push_str("</a:path>");
    }
    vml_path.push('e');
    let id = ink.id;
    let name = format!("Freeform: Shape {id}");
    let color = &ink.color;
    let src = format!(
        r##"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="{z}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>{px}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>{py}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{wps}"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst>{paths}</a:pathLst></a:custGeom><a:noFill/><a:ln w="{lw}" cap="{cap}"><a:solidFill>{color_xml}</a:solidFill><a:round/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape id="{name}" o:spid="_x0000_s{uid}" style="position:absolute;margin-left:{mx};margin-top:{my};width:{wp};height:{hp};z-index:{z};visibility:visible;mso-wrap-style:square;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text" coordsize="{tw_w},{tw_h}" o:spt="100" path="{vml_path}" filled="f" strokecolor="#{color}" strokeweight="{sw}"><v:stroke endcap="{vcap}" joinstyle="round"{vopacity}/><w10:wrap type="none"/></v:shape></w:pict></mc:Fallback></mc:AlternateContent>"##,
        z = ink.z,
        cap = ink.cap.ooxml(),
        vcap = ink.cap.vml(),
        vopacity = ink.alpha.map(|a| format!(r#" opacity="{}""#, (a * 1000.0).round() / 1000.0)).unwrap_or_default(),
        color_xml = match ink.alpha {
            Some(a) => format!(r#"<a:srgbClr val="{color}"><a:alpha val="{}"/></a:srgbClr>"#, (a * 100_000.0).round() as i64),
            None => format!(r#"<a:srgbClr val="{color}"/>"#),
        },
        px = emu(ox),
        py = emu(oy),
        wps = super::build::NS_WPS,
        lw = emu(ink.width_pt),
        uid = 1024 + id,
        mx = pt(ox),
        my = pt(oy),
        wp = pt(w),
        hp = pt(h),
        tw_w = tw(w),
        tw_h = tw(h),
        sw = pt(ink.width_pt),
    );
    Ok((parse_with(&src), ox, oy, w, h))
}
