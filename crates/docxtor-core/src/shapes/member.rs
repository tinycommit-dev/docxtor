//! Changing one member of a group or canvas in place: its box (in the coordinates of the group, `a:chOff` and
//! `a:chExt`), rotation, colours, outline and preset. A group's box grows when a member is moved or sized past
//! its edge, and the VML copy of the group is written again from the DrawingML so both say the same.

use super::patch::{drawing_mut, find_mut, patch_vml_shape, patch_wsp, put, resolve, style_pairs, style_string, Look, ShapePatch, VmlChange};
use super::{RShape, EMU_PER_PT};
use crate::xml::Element;

/// EMU in a twentieth of a point: the unit the VML copy of a group counts in.
const EMU_PER_TWIP: f64 = 635.0;

/// The names a group's or canvas's members have.
pub fn is_member(e: &Element) -> bool {
    matches!(e.name.as_str(), "wps:wsp" | "wpg:grpSp" | "wpg:wgp" | "pic:pic")
}

fn is_group(e: &Element) -> bool {
    matches!(e.name.as_str(), "wpg:grpSp" | "wpg:wgp")
}

fn num(e: Option<&Element>, a: &str) -> f64 {
    e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

fn xfrm_of(m: &Element) -> Option<&Element> {
    let pr = match m.name.as_str() {
        "wps:wsp" => m.child("wps:spPr"),
        "wpg:grpSp" | "wpg:wgp" => m.child("wpg:grpSpPr"),
        "pic:pic" => m.child("pic:spPr"),
        _ => None,
    }?;
    pr.child("a:xfrm")
}

fn xfrm_mut(m: &mut Element) -> Option<&mut Element> {
    let pr = match m.name.as_str() {
        "wps:wsp" => m.child_mut("wps:spPr"),
        "wpg:grpSp" | "wpg:wgp" => m.child_mut("wpg:grpSpPr"),
        "pic:pic" => m.child_mut("pic:spPr"),
        _ => None,
    }?;
    pr.child_mut("a:xfrm")
}

/// How points inside the parent's box turn into the child coordinates of its members: `emu = o + pt * k`.
struct Space {
    ox: f64,
    oy: f64,
    kx: f64,
    ky: f64,
}

fn space_of(container: &Element, w: f64, h: f64) -> Space {
    if container.name == "wpg:wgp" || container.name == "wpg:grpSp" {
        if let Some(x) = container.child("wpg:grpSpPr").and_then(|p| p.child("a:xfrm")) {
            let (off, ext) = (x.child("a:chOff"), x.child("a:chExt"));
            let (cw, ch) = (num(ext, "cx"), num(ext, "cy"));
            return Space {
                ox: num(off, "x"),
                oy: num(off, "y"),
                kx: if cw > 0.0 && w > 0.0 { cw / w } else { EMU_PER_PT },
                ky: if ch > 0.0 && h > 0.0 { ch / h } else { EMU_PER_PT },
            };
        }
    }
    Space { ox: 0.0, oy: 0.0, kx: EMU_PER_PT, ky: EMU_PER_PT }
}

/// The `wpg:wgp` or `wpc:wpc` of a shape object.
fn container_mut(obj: &mut Element) -> Option<&mut Element> {
    let drawing = drawing_mut(obj)?;
    if drawing.find("wpg:wgp").is_some() {
        find_mut(drawing, "wpg:wgp")
    } else {
        find_mut(drawing, "wpc:wpc")
    }
}

fn set_name_attr(m: &mut Element, attr: &'static str, value: &str) {
    for tag in ["wps:cNvPr", "wpg:cNvPr", "pic:cNvPr"] {
        if let Some(d) = find_mut(m, tag) {
            d.set_attr(attr, value);
            return;
        }
    }
}

/// Apply a patch to a member of the group or canvas in `obj`. `cur` is the whole shape as it reads now.
pub fn apply(obj: &mut Element, path: &[usize], p: &ShapePatch, cur: &RShape, rid: Option<&str>) -> Result<(), String> {
    if p.wrap.is_some() {
        return Err("a member of a group can't be wrapped by itself; change the wrapping of the group".into());
    }
    if path.is_empty() {
        return Err("name the member to change".into());
    }
    // Check everything before changing anything.
    let mut member = cur;
    for &i in path {
        member = member.children.get(i).ok_or("there is no such member in that group")?;
    }
    let look = resolve(p, member, rid)?;
    for (name, v) in [("w", p.w), ("h", p.h)] {
        if let Some(v) = v {
            if !(0.0..=1584.0).contains(&v) {
                return Err(format!("{name} must be between 0 and 1584 points"));
            }
        }
    }
    let container = container_mut(obj).ok_or("this shape is not a group")?;
    walk(container, path, cur, p, &look)?;
    // The group's box follows its members; the VML copy follows the group.
    if let Some(drawing) = drawing_mut(obj) {
        fit_group(drawing);
    }
    resync_vml(obj, p, &look, member, path);
    Ok(())
}

fn walk(container: &mut Element, path: &[usize], parent: &RShape, p: &ShapePatch, look: &Look) -> Result<(), String> {
    let space = space_of(container, parent.w, parent.h);
    let i = path[0];
    let shape = parent.children.get(i).ok_or("there is no such member in that group")?;
    let m = container.elements_mut().filter(|e| is_member(e)).nth(i).ok_or("there is no such member in that group")?;
    if path.len() > 1 {
        if !is_group(m) {
            return Err("that member is not a group".into());
        }
        return walk(m, &path[1..], shape, p, look);
    }
    if let Some(x) = xfrm_mut(m) {
        let set = |x: &mut Element, child: &str, a: &'static str, v: f64| {
            if let Some(e) = x.child_mut(child) {
                e.set_attr(a, (v.round() as i64).to_string());
            }
        };
        if let Some(v) = p.x {
            set(x, "a:off", "x", space.ox + v * space.kx);
        }
        if let Some(v) = p.y {
            set(x, "a:off", "y", space.oy + v * space.ky);
        }
        if let Some(v) = p.w {
            set(x, "a:ext", "cx", v * space.kx);
        }
        if let Some(v) = p.h {
            set(x, "a:ext", "cy", v * space.ky);
        }
        if let Some(r) = p.rot {
            let r = r.rem_euclid(360.0);
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
    } else if p.x.is_some() || p.y.is_some() || p.w.is_some() || p.h.is_some() || p.rot.is_some() {
        return Err("this member has no position in its group".into());
    }
    if m.name == "wps:wsp" {
        patch_wsp(m, look, shape)?;
    } else if look.fill.is_some() || look.line.is_some() || look.geometry.is_some() || look.effects.is_some() || look.body.is_some() {
        return Err("only a shape has a fill, an outline, effects, a text frame or a preset".into());
    }
    if let Some(alt) = &p.alt {
        set_name_attr(m, "descr", alt);
    }
    if let Some(name) = &look.name {
        set_name_attr(m, "name", name);
    }
    Ok(())
}

/// Rotated bounds of a member in child coordinates: left, top, right, bottom.
fn bounds(m: &Element) -> Option<(f64, f64, f64, f64)> {
    let x = xfrm_of(m)?;
    let (off, ext) = (x.child("a:off"), x.child("a:ext"));
    let (ox, oy, w, h) = (num(off, "x"), num(off, "y"), num(ext, "cx"), num(ext, "cy"));
    let a = (num(Some(x), "rot") / 60000.0).to_radians();
    let (bw, bh) = (w * a.cos().abs() + h * a.sin().abs(), w * a.sin().abs() + h * a.cos().abs());
    let (cx, cy) = (ox + w / 2.0, oy + h / 2.0);
    Some((cx - bw / 2.0, cy - bh / 2.0, cx + bw / 2.0, cy + bh / 2.0))
}

/// Make a group's box cover all its members. The scale of its members stays as it was; when the left or top
/// edge moves out, an anchored group moves with it so the other members stay where they were on the page.
fn fit_group(drawing: &mut Element) {
    let Some(c) = (if drawing.find("wp:anchor").is_some() { find_mut(drawing, "wp:anchor") } else { find_mut(drawing, "wp:inline") }) else { return };
    let Some(wgp) = find_mut(c, "wpg:wgp") else { return };
    let (members, xfrm) = {
        let Some(x) = wgp.child("wpg:grpSpPr").and_then(|p| p.child("a:xfrm")) else { return };
        let (off, ext) = (x.child("a:chOff"), x.child("a:chExt"));
        let ch = (num(off, "x"), num(off, "y"), num(ext, "cx"), num(ext, "cy"));
        let size = (num(x.child("a:ext"), "cx"), num(x.child("a:ext"), "cy"));
        (wgp.elements().filter(|e| is_member(e)).filter_map(bounds).collect::<Vec<_>>(), (ch, size))
    };
    let ((cox, coy, cw, chh), (w, h)) = xfrm;
    if cw <= 0.0 || chh <= 0.0 || members.is_empty() {
        return;
    }
    let minx = members.iter().map(|b| b.0).fold(cox, f64::min);
    let miny = members.iter().map(|b| b.1).fold(coy, f64::min);
    let maxx = members.iter().map(|b| b.2).fold(cox + cw, f64::max);
    let maxy = members.iter().map(|b| b.3).fold(coy + chh, f64::max);
    if minx >= cox - 1.0 && miny >= coy - 1.0 && maxx <= cox + cw + 1.0 && maxy <= coy + chh + 1.0 {
        return;
    }
    let (sx, sy) = (w / cw, h / chh);
    let (ncw, nch) = (maxx - minx, maxy - miny);
    let (nw, nh) = ((ncw * sx).round(), (nch * sy).round());
    let (dx, dy) = (((cox - minx) * sx).round() as i64, ((coy - miny) * sy).round() as i64);
    if let Some(x) = wgp.child_mut("wpg:grpSpPr").and_then(|p| p.child_mut("a:xfrm")) {
        for (child, a, v) in [("a:chOff", "x", minx), ("a:chOff", "y", miny), ("a:chExt", "cx", ncw), ("a:chExt", "cy", nch), ("a:ext", "cx", nw), ("a:ext", "cy", nh)] {
            if let Some(e) = x.child_mut(child) {
                e.set_attr(a, (v.round() as i64).to_string());
            }
        }
    }
    if let Some(e) = c.child_mut("wp:extent") {
        e.set_attr("cx", (nw as i64).to_string());
        e.set_attr("cy", (nh as i64).to_string());
    }
    for (name, d) in [("wp:positionH", dx), ("wp:positionV", dy)] {
        if let Some(off) = c.child_mut(name).and_then(|p| p.child_mut("wp:posOffset")) {
            let now = off.text().trim().parse::<i64>().unwrap_or(0);
            *off = Element::new("wp:posOffset").with_text((now - d).to_string());
        }
    }
}

/// Write the VML copy of a group again from its DrawingML members (when both have the same members).
fn resync_vml(obj: &mut Element, p: &ShapePatch, look: &Look, member: &RShape, path: &[usize]) {
    if obj.name != "mc:AlternateContent" {
        return;
    }
    // What the DrawingML group says.
    let Some(choice) = obj.elements().find(|c| c.name == "mc:Choice") else { return };
    let Some(wgp) = choice.find("wpg:wgp") else { return };
    let Some(x) = wgp.child("wpg:grpSpPr").and_then(|p| p.child("a:xfrm")) else { return };
    let (cox, coy) = (num(x.child("a:chOff"), "x"), num(x.child("a:chOff"), "y"));
    let (cw, chh) = (num(x.child("a:chExt"), "cx"), num(x.child("a:chExt"), "cy"));
    let (w, h) = (num(x.child("a:ext"), "cx"), num(x.child("a:ext"), "cy"));
    let boxes: Vec<(f64, f64, f64, f64, f64)> = wgp
        .elements()
        .filter(|e| is_member(e))
        .map(|m| {
            let x = xfrm_of(m);
            let (off, ext) = (x.and_then(|x| x.child("a:off")), x.and_then(|x| x.child("a:ext")));
            (num(off, "x"), num(off, "y"), num(ext, "cx"), num(ext, "cy"), num(x, "rot") / 60000.0)
        })
        .collect();
    let anchor_pos = |name: &str| choice.find(name).and_then(|p| p.child("wp:posOffset")).and_then(|o| o.text().trim().parse::<f64>().ok());
    let (px, py) = (anchor_pos("wp:positionH"), anchor_pos("wp:positionV"));
    let Some(fb) = obj.child_mut("mc:Fallback") else { return };
    let Some(pict) = fb.child_mut("w:pict") else { return };
    let Some(group) = pict.child_mut("v:group") else { return };
    let shapes: Vec<&mut Element> = group.elements_mut().filter(|e| matches!(e.name.as_str(), "v:rect" | "v:roundrect" | "v:oval" | "v:line" | "v:shape" | "v:group")).collect();
    if shapes.len() != boxes.len() || cw <= 0.0 || chh <= 0.0 {
        return;
    }
    // The member the patch was for takes the new colours.
    let only = ShapePatch { flip_h: p.flip_h, flip_v: p.flip_v, ..Default::default() };
    let ch = VmlChange { p: &only, look, cur: member, size: None, rot: None };
    let twip = |v: f64| (v / EMU_PER_TWIP).round() as i64;
    for (i, sh) in shapes.into_iter().enumerate() {
        if path.first() == Some(&i) && path.len() == 1 {
            patch_vml_shape(sh, &ch);
        }
        if sh.name == "v:line" {
            continue;
        }
        let (bx, by, bw, bh, rot) = boxes[i];
        let mut style = style_pairs(sh.attr("style").unwrap_or(""));
        put(&mut style, "position", "absolute");
        put(&mut style, "left", twip(bx - cox).to_string());
        put(&mut style, "top", twip(by - coy).to_string());
        put(&mut style, "width", twip(bw).to_string());
        put(&mut style, "height", twip(bh).to_string());
        if rot != 0.0 {
            put(&mut style, "rotation", format!("{}", (rot * 100.0).round() / 100.0));
        }
        sh.set_attr("style", style_string(&style));
    }
    let mut style = style_pairs(group.attr("style").unwrap_or(""));
    put(&mut style, "width", super::patch::pt(w / EMU_PER_PT));
    put(&mut style, "height", super::patch::pt(h / EMU_PER_PT));
    if let Some(v) = px.filter(|_| style.iter().any(|(k, _)| k == "margin-left")) {
        put(&mut style, "margin-left", super::patch::pt(v / EMU_PER_PT));
    }
    if let Some(v) = py.filter(|_| style.iter().any(|(k, _)| k == "margin-top")) {
        put(&mut style, "margin-top", super::patch::pt(v / EMU_PER_PT));
    }
    group.set_attr("style", style_string(&style));
    group.set_attr("coordsize", format!("{},{}", twip(cw), twip(chh)));
    group.set_attr("coordorigin", "0,0");
}
