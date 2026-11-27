//! Effects of shapes: reading `a:effectLst`, `a:scene3d` and `a:sp3d` (a shape's own, else the theme style it
//! refers to) into the view model, and writing them from an op the way Word does.

use super::appearance::*;
use super::fill::{check_alpha, colour};
use super::theme::ShapeTheme;
use crate::props::put_ordered;
use crate::xml::{Element, Node};

const EMU_PER_PT: f64 = super::EMU_PER_PT;

fn num(e: &Element, a: &str) -> Option<f64> {
    e.attr(a).and_then(|v| v.parse::<f64>().ok())
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// The effects of a shape: from its `spPr` (`a:effectLst`, `a:scene3d`, `a:sp3d`), and for what `spPr` says
/// nothing about, from the theme style `wps:style` refers to with `a:effectRef`.
pub fn read(th: &ShapeTheme, sppr: Option<&Element>, style: Option<&Element>) -> Option<REffects> {
    let eref = style.and_then(|s| s.child("a:effectRef"));
    let idx = eref.and_then(|r| r.attr("idx")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let ph = eref.and_then(|r| th.color_of(r, None));
    let themed = if idx > 0 { th.effect_style(idx) } else { None };
    let pick = |name: &str| sppr.and_then(|p| p.child(name)).or_else(|| themed.and_then(|t| t.child(name)));
    let mut out = REffects::default();
    if let Some(lst) = pick("a:effectLst") {
        read_list(th, lst, ph.as_ref(), &mut out);
    }
    let scene = pick("a:scene3d");
    if let Some(sp) = pick("a:sp3d") {
        read_3d(th, sp, scene, ph.as_ref(), &mut out);
    }
    (!out.is_empty()).then_some(out)
}

fn shadow(th: &ShapeTheme, e: &Element, ph: Option<&(String, f64)>, outer: bool) -> Option<RShadow> {
    let (color, alpha) = th.color_of(e, ph)?;
    let scale = |a: &str| num(e, a).map(|v| v / 1000.0).filter(|v| (v - 100.0).abs() > 1e-6);
    Some(RShadow {
        color,
        alpha,
        blur_pt: num(e, "blurRad").unwrap_or(0.0) / EMU_PER_PT,
        dist_pt: num(e, "dist").unwrap_or(0.0) / EMU_PER_PT,
        angle: num(e, "dir").unwrap_or(0.0) / 60000.0,
        sx: if outer { scale("sx") } else { None },
        sy: if outer { scale("sy") } else { None },
        align: if outer { e.attr("algn").filter(|a| *a != "b").map(String::from) } else { None },
        rot_with_shape: if outer { e.attr("rotWithShape").map(|v| matches!(v, "1" | "true")) } else { None },
    })
}

fn read_list(th: &ShapeTheme, lst: &Element, ph: Option<&(String, f64)>, out: &mut REffects) {
    for e in lst.elements() {
        match e.name.as_str() {
            "a:outerShdw" => out.shadow = shadow(th, e, ph, true),
            "a:innerShdw" => out.inner_shadow = shadow(th, e, ph, false),
            "a:glow" => {
                if let Some((color, alpha)) = th.color_of(e, ph) {
                    out.glow = Some(RGlow { color, alpha, radius_pt: num(e, "rad").unwrap_or(0.0) / EMU_PER_PT });
                }
            }
            "a:softEdge" => out.soft_edge_pt = Some(num(e, "rad").unwrap_or(0.0) / EMU_PER_PT),
            "a:reflection" => {
                let scale = |a: &str| num(e, a).map(|v| v / 1000.0).filter(|v| (v - 100.0).abs() > 1e-6);
                out.reflection = Some(RReflection {
                    blur_pt: num(e, "blurRad").unwrap_or(0.0) / EMU_PER_PT,
                    start_alpha: num(e, "stA").unwrap_or(100000.0) / 100000.0,
                    end_alpha: num(e, "endA").unwrap_or(0.0) / 100000.0,
                    dist_pt: num(e, "dist").unwrap_or(0.0) / EMU_PER_PT,
                    direction: num(e, "dir").unwrap_or(0.0) / 60000.0,
                    fade_direction: num(e, "fadeDir").unwrap_or(5400000.0) / 60000.0,
                    sx: scale("sx"),
                    sy: scale("sy"),
                    start_pos: num(e, "stPos").map(|v| v / 1000.0).filter(|v| *v != 0.0),
                    end_pos: num(e, "endPos").map(|v| v / 1000.0).filter(|v| (v - 100.0).abs() > 1e-6),
                    rot_with_shape: e.attr("rotWithShape").map(|v| matches!(v, "1" | "true")),
                });
            }
            _ => {}
        }
    }
}

fn read_3d(th: &ShapeTheme, sp: &Element, scene: Option<&Element>, ph: Option<&(String, f64)>, out: &mut REffects) {
    if let Some(b) = sp.child("a:bevelT").or_else(|| sp.child("a:bevelB")) {
        out.bevel = Some(RBevel { preset: b.attr("prst").unwrap_or("circle").to_string(), w_pt: num(b, "w").unwrap_or(76200.0) / EMU_PER_PT, h_pt: num(b, "h").unwrap_or(76200.0) / EMU_PER_PT });
    }
    let h = num(sp, "extrusionH").unwrap_or(0.0) / EMU_PER_PT;
    if h > 0.0 {
        out.extrusion = Some(RExtrusion { height_pt: h, color: sp.child("a:extrusionClr").and_then(|c| th.color_of(c, ph)).map(|c| c.0), dir: camera_dir(scene) });
    }
}

/// Where depth goes on the page for the scene's camera: a step `[dx, dy]` per point of depth (y down). A camera
/// that looks straight at the shape shows no depth. Approximate: Word draws a real projection.
pub fn camera_dir(scene: Option<&Element>) -> Option<[f64; 2]> {
    let cam = scene?.child("a:camera")?;
    let prst = cam.attr("prst").unwrap_or("orthographicFront");
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let mut dx = 0.0;
    let mut dy = 0.0;
    if prst.contains("Left") {
        dx = -1.0;
    }
    if prst.contains("Right") {
        dx = 1.0;
    }
    if prst.contains("Top") || prst.contains("Above") || prst.contains("Up") {
        dy = -1.0;
    }
    if prst.contains("Bottom") || prst.contains("Below") || prst.contains("Down") {
        dy = 1.0;
    }
    if dx != 0.0 && dy != 0.0 {
        dx *= s;
        dy *= s;
    }
    if dx == 0.0 && dy == 0.0 {
        // A free rotation: latitude tilts the top towards the viewer, longitude turns the right side away.
        if let Some(r) = cam.child("a:rot") {
            let lat = num(r, "lat").unwrap_or(0.0) / 60000.0;
            let lon = num(r, "lon").unwrap_or(0.0) / 60000.0;
            dx = (lon.to_radians()).sin();
            dy = -(lat.to_radians()).sin();
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.05 {
                return None;
            }
            if len > 1.0 {
                dx /= len;
                dy /= len;
            }
        } else {
            return None;
        }
    }
    Some([(dx * 1000.0).round() / 1000.0, (dy * 1000.0).round() / 1000.0])
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

const LST_ORDER: &[&str] = &["a:blur", "a:fillOverlay", "a:glow", "a:innerShdw", "a:outerShdw", "a:prstShdw", "a:reflection", "a:softEdge"];
const SP3D_ORDER: &[&str] = &["a:bevelT", "a:bevelB", "a:extrusionClr", "a:contourClr", "a:extLst"];
const SPPR_ORDER: &[&str] = &["a:xfrm", "a:custGeom", "a:prstGeom", "a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill", "a:ln", "a:effectLst", "a:effectDag", "a:scene3d", "a:sp3d", "a:extLst"];
const COLOR_NAMES: &[&str] = &["a:srgbClr", "a:schemeClr", "a:sysClr", "a:prstClr", "a:scrgbClr", "a:hslClr"];

pub const BEVELS: &[&str] = &["circle", "relaxedInset", "cross", "coolSlant", "angle", "softRound", "convex", "slope", "divot", "riblet", "hardEdge", "artDeco"];

fn emu(pt: f64) -> String {
    ((pt * EMU_PER_PT).round() as i64).to_string()
}

fn angle(deg: f64) -> String {
    ((deg.rem_euclid(360.0) * 60000.0).round() as i64).to_string()
}

fn pct(v: f64) -> String {
    ((v * 1000.0).round() as i64).to_string()
}

fn range(name: &str, v: Option<f64>, lo: f64, hi: f64) -> Result<(), String> {
    match v {
        Some(v) if !(lo..=hi).contains(&v) => Err(format!("{name} must be between {lo} and {hi}")),
        _ => Ok(()),
    }
}

/// Check an op's `effects` before anything is changed.
pub fn check(spec: &EffectsSpec) -> Result<(), String> {
    let EffectsSpec::Set(e) = spec else { return Ok(()) };
    for sh in [&e.shadow, &e.inner_shadow].into_iter().flatten() {
        if let Opt::Set(s) = sh {
            range("blur_pt", s.blur_pt, 0.0, 100.0)?;
            range("dist_pt", s.dist_pt, 0.0, 200.0)?;
            range("sx", s.sx, -1000.0, 1000.0)?;
            range("sy", s.sy, -1000.0, 1000.0)?;
            if let Some(c) = &s.color {
                colour(c)?;
            }
            check_alpha(s.alpha)?;
            if let Some(a) = &s.align {
                if !matches!(a.as_str(), "tl" | "t" | "tr" | "l" | "ctr" | "r" | "bl" | "b" | "br") {
                    return Err(format!("unknown shadow alignment '{a}'"));
                }
            }
        }
    }
    if let Some(Opt::Set(g)) = &e.glow {
        range("radius_pt", g.radius_pt, 0.0, 150.0)?;
        if let Some(c) = &g.color {
            colour(c)?;
        }
        check_alpha(g.alpha)?;
    }
    if let Some(Opt::Set(v)) = &e.soft_edge_pt {
        range("soft_edge_pt", Some(*v), 0.0, 100.0)?;
    }
    if let Some(Opt::Set(r)) = &e.reflection {
        range("blur_pt", r.blur_pt, 0.0, 100.0)?;
        range("dist_pt", r.dist_pt, 0.0, 200.0)?;
        range("start_alpha", r.start_alpha, 0.0, 1.0)?;
        range("end_alpha", r.end_alpha, 0.0, 1.0)?;
        range("start_pos", r.start_pos, 0.0, 100.0)?;
        range("end_pos", r.end_pos, 0.0, 100.0)?;
    }
    if let Some(Opt::Set(b)) = &e.bevel {
        range("w_pt", b.w_pt, 0.0, 1584.0)?;
        range("h_pt", b.h_pt, 0.0, 1584.0)?;
        if let Some(p) = &b.preset {
            if !BEVELS.contains(&p.as_str()) {
                return Err(format!("unknown bevel '{p}'; use {}", BEVELS.join(", ")));
            }
        }
    }
    if let Some(Opt::Set(x)) = &e.extrusion {
        range("height_pt", x.height_pt, 0.0, 1584.0)?;
        if let Some(c) = &x.color {
            colour(c)?;
        }
    }
    Ok(())
}

fn alpha_child(alpha: f64) -> Element {
    Element::new("a:alpha").with_attr("val", ((alpha * 100000.0).round() as i64).to_string())
}

fn srgb(hex: &str, alpha: f64) -> Element {
    let mut c = Element::new("a:srgbClr").with_attr("val", hex);
    if (alpha - 1.0).abs() > 1e-6 {
        c.children.push(Node::Element(alpha_child(alpha)));
    }
    c
}

/// Set the colour (and alpha) an effect element holds. A colour given replaces it, keeping the alpha unless
/// one is given; an alpha alone changes only the alpha of the colour already there.
fn set_color(el: &mut Element, color: Option<&str>, alpha: Option<f64>) {
    let idx = el.children.iter().position(|n| matches!(n, Node::Element(e) if COLOR_NAMES.contains(&e.name.as_str())));
    match (color, idx) {
        (Some(c), idx) => {
            let old = idx.and_then(|i| match &el.children[i] {
                Node::Element(e) => e.child("a:alpha").and_then(|a| num(a, "val")).map(|v| v / 100000.0),
                _ => None,
            });
            let new = Node::Element(srgb(c, alpha.or(old).unwrap_or(1.0)));
            match idx {
                Some(i) => el.children[i] = new,
                None => el.children.insert(0, new),
            }
        }
        (None, Some(i)) => {
            if let (Some(a), Node::Element(c)) = (alpha, &mut el.children[i]) {
                c.remove_children("a:alpha");
                if (a - 1.0).abs() > 1e-6 {
                    c.children.push(Node::Element(alpha_child(a)));
                }
            }
        }
        (None, None) => {}
    }
}

fn set_pct_attr(el: &mut Element, name: &'static str, v: Option<f64>, default: f64) {
    if let Some(v) = v {
        if (v - default).abs() < 1e-9 && default == 100.0 {
            el.remove_attr(name);
        } else {
            el.set_attr(name, pct(v));
        }
    }
}

fn put_shadow(lst: &mut Element, tag: &'static str, v: &Opt<ShadowIn>) {
    let s = match v {
        Opt::Remove => {
            lst.remove_children(tag);
            return;
        }
        Opt::Set(s) => s,
    };
    let outer = tag == "a:outerShdw";
    if lst.child(tag).is_none() {
        let mut e = Element::new(tag);
        let (blur, dist) = if outer { (4.0, 3.0) } else { (5.0, 4.0) };
        e.set_attr("blurRad", emu(blur));
        e.set_attr("dist", emu(dist));
        e.set_attr("dir", angle(45.0));
        if outer {
            e.set_attr("algn", "tl");
            e.set_attr("rotWithShape", "0");
        }
        e.children.push(Node::Element(srgb("000000", if outer { 0.4 } else { 0.5 })));
        put_ordered(lst, e, LST_ORDER);
    }
    let e = lst.child_mut(tag).expect("just added");
    if let Some(v) = s.blur_pt {
        e.set_attr("blurRad", emu(v));
    }
    if let Some(v) = s.dist_pt {
        e.set_attr("dist", emu(v));
    }
    if let Some(v) = s.angle {
        e.set_attr("dir", angle(v));
    }
    if outer {
        set_pct_attr(e, "sx", s.sx, 100.0);
        set_pct_attr(e, "sy", s.sy, 100.0);
        if let Some(a) = &s.align {
            e.set_attr("algn", a.as_str());
        }
    }
    set_color(e, s.color.as_deref().map(|c| c.trim_start_matches('#')).and_then(super::hex6).as_deref(), s.alpha);
}

fn put_glow(lst: &mut Element, v: &Opt<GlowIn>) {
    let g = match v {
        Opt::Remove => {
            lst.remove_children("a:glow");
            return;
        }
        Opt::Set(g) => g,
    };
    if lst.child("a:glow").is_none() {
        let mut e = Element::new("a:glow").with_attr("rad", emu(5.0));
        e.children.push(Node::Element(Element::new("a:schemeClr").with_attr("val", "accent1").with_child(alpha_child(0.4))));
        put_ordered(lst, e, LST_ORDER);
    }
    let e = lst.child_mut("a:glow").expect("just added");
    if let Some(r) = g.radius_pt {
        e.set_attr("rad", emu(r));
    }
    set_color(e, g.color.as_deref().and_then(super::hex6).as_deref(), g.alpha);
}

fn put_reflection(lst: &mut Element, v: &Opt<ReflectionIn>) {
    let r = match v {
        Opt::Remove => {
            lst.remove_children("a:reflection");
            return;
        }
        Opt::Set(r) => r,
    };
    if lst.child("a:reflection").is_none() {
        // Word's "Half reflection, touching".
        let e = Element::new("a:reflection")
            .with_attr("blurRad", "6350")
            .with_attr("stA", "50000")
            .with_attr("stPos", "0")
            .with_attr("endA", "300")
            .with_attr("endPos", "55000")
            .with_attr("dist", "0")
            .with_attr("dir", "5400000")
            .with_attr("sy", "-100000")
            .with_attr("algn", "bl")
            .with_attr("rotWithShape", "0");
        put_ordered(lst, e, LST_ORDER);
    }
    let e = lst.child_mut("a:reflection").expect("just added");
    if let Some(v) = r.blur_pt {
        e.set_attr("blurRad", emu(v));
    }
    if let Some(v) = r.start_alpha {
        e.set_attr("stA", pct(v * 100.0));
    }
    if let Some(v) = r.end_alpha {
        e.set_attr("endA", pct(v * 100.0));
    }
    if let Some(v) = r.dist_pt {
        e.set_attr("dist", emu(v));
    }
    if let Some(v) = r.direction {
        e.set_attr("dir", angle(v));
    }
    if let Some(v) = r.fade_direction {
        e.set_attr("fadeDir", angle(v));
    }
    if let Some(v) = r.sx {
        e.set_attr("sx", pct(v));
    }
    if let Some(v) = r.sy {
        e.set_attr("sy", pct(v));
    }
    if let Some(v) = r.start_pos {
        e.set_attr("stPos", pct(v));
    }
    if let Some(v) = r.end_pos {
        e.set_attr("endPos", pct(v));
    }
}

/// Write the effect lists of a shape's `spPr` from an op. `cur` is what the shape shows now (its own effects or
/// those of its theme style): an own list replaces the style's, so what the style gave is written out first.
/// `themed` says the shape's style refers to a theme effect style.
pub fn apply(pr: &mut Element, spec: &EffectsSpec, cur: Option<&REffects>, themed: bool) -> Result<(), String> {
    check(spec)?;
    let had_lst = pr.child("a:effectLst").is_some();
    let had_sp3d = pr.child("a:sp3d").is_some();
    match spec {
        EffectsSpec::None => {
            if let Some(l) = pr.child_mut("a:effectLst") {
                l.children.retain(|n| matches!(n, Node::Element(e) if !LST_ORDER[2..].contains(&e.name.as_str())));
            } else if themed {
                put_ordered(pr, Element::new("a:effectLst"), SPPR_ORDER);
            }
            if let Some(s) = pr.child_mut("a:sp3d") {
                s.remove_attr("extrusionH");
                s.children.retain(|n| matches!(n, Node::Element(e) if !matches!(e.name.as_str(), "a:bevelT" | "a:bevelB" | "a:extrusionClr")));
            } else if themed && cur.is_some_and(|c| c.bevel.is_some() || c.extrusion.is_some()) {
                put_ordered(pr, Element::new("a:sp3d"), SPPR_ORDER);
            }
            drop_empty(pr, themed);
            return Ok(());
        }
        EffectsSpec::Set(_) => {}
    }
    let EffectsSpec::Set(e) = spec else { unreachable!() };
    if pr.child("a:effectLst").is_none() {
        put_ordered(pr, Element::new("a:effectLst"), SPPR_ORDER);
    }
    // Carry over what the theme style gave, for the parts the op leaves alone.
    if !had_lst {
        if let Some(c) = cur {
            let lst = pr.child_mut("a:effectLst").expect("just added");
            if e.shadow.is_none() {
                if let Some(s) = &c.shadow {
                    put_shadow(lst, "a:outerShdw", &Opt::Set(from_shadow(s)));
                }
            }
            if e.inner_shadow.is_none() {
                if let Some(s) = &c.inner_shadow {
                    put_shadow(lst, "a:innerShdw", &Opt::Set(from_shadow(s)));
                }
            }
            if e.glow.is_none() {
                if let Some(g) = &c.glow {
                    put_glow(lst, &Opt::Set(GlowIn { color: Some(g.color.clone()), alpha: Some(g.alpha), radius_pt: Some(g.radius_pt) }));
                }
            }
            if e.soft_edge_pt.is_none() {
                if let Some(r) = c.soft_edge_pt {
                    lst.children.push(Node::Element(Element::new("a:softEdge").with_attr("rad", emu(r))));
                }
            }
            if e.reflection.is_none() {
                if let Some(r) = &c.reflection {
                    put_reflection(lst, &Opt::Set(from_reflection(r)));
                }
            }
        }
    }
    if !had_sp3d && (e.bevel.is_none() || e.extrusion.is_none()) {
        if let Some(c) = cur.filter(|c| c.bevel.is_some() || c.extrusion.is_some()) {
            put_ordered(pr, Element::new("a:sp3d"), SPPR_ORDER);
            let sp = pr.child_mut("a:sp3d").expect("just added");
            if e.bevel.is_none() {
                if let Some(b) = &c.bevel {
                    put_bevel(sp, &Opt::Set(BevelIn { preset: Some(b.preset.clone()), w_pt: Some(b.w_pt), h_pt: Some(b.h_pt) }));
                }
            }
            if e.extrusion.is_none() {
                if let Some(x) = &c.extrusion {
                    put_extrusion(sp, &Opt::Set(ExtrusionIn { height_pt: Some(x.height_pt), color: x.color.clone() }));
                }
            }
        }
    }
    let lst = pr.child_mut("a:effectLst").expect("just added");
    if let Some(v) = &e.shadow {
        put_shadow(lst, "a:outerShdw", v);
    }
    if let Some(v) = &e.inner_shadow {
        put_shadow(lst, "a:innerShdw", v);
    }
    if let Some(v) = &e.glow {
        put_glow(lst, v);
    }
    match &e.soft_edge_pt {
        Some(Opt::Remove) => {
            lst.remove_children("a:softEdge");
        }
        Some(Opt::Set(r)) => put_ordered(lst, Element::new("a:softEdge").with_attr("rad", emu(*r)), LST_ORDER),
        None => {}
    }
    if let Some(v) = &e.reflection {
        put_reflection(lst, v);
    }
    if e.bevel.is_some() || e.extrusion.is_some() {
        let adding_extrusion = matches!(e.extrusion, Some(Opt::Set(_)));
        let adding_3d = adding_extrusion || matches!(e.bevel, Some(Opt::Set(_)));
        if pr.child("a:sp3d").is_none() && adding_3d {
            put_ordered(pr, Element::new("a:sp3d"), SPPR_ORDER);
        }
        if adding_3d {
            ensure_scene(pr, adding_extrusion);
        }
        if let Some(sp) = pr.child_mut("a:sp3d") {
            if let Some(v) = &e.bevel {
                put_bevel(sp, v);
            }
            if let Some(v) = &e.extrusion {
                put_extrusion(sp, v);
            }
        }
    }
    drop_empty(pr, themed);
    Ok(())
}

/// A scene is needed for the 3-D format to count. An extrusion needs a camera that looks at it from the side.
fn ensure_scene(pr: &mut Element, extrusion: bool) {
    if pr.child("a:scene3d").is_none() {
        let prst = if extrusion { "obliqueTopRight" } else { "orthographicFront" };
        let scene = Element::new("a:scene3d")
            .with_child(Element::new("a:camera").with_attr("prst", prst).with_child(Element::new("a:rot").with_attr("lat", "0").with_attr("lon", "0").with_attr("rev", "0")))
            .with_child(Element::new("a:lightRig").with_attr("rig", "threePt").with_attr("dir", "t"));
        put_ordered(pr, scene, SPPR_ORDER);
    } else if extrusion {
        if let Some(cam) = pr.child_mut("a:scene3d").and_then(|s| s.child_mut("a:camera")) {
            if camera_dir(Some(&Element::new("a:scene3d").with_child(cam.clone()))).is_none() {
                cam.set_attr("prst", "obliqueTopRight");
                cam.remove_children("a:rot");
            }
        }
    }
}

fn put_bevel(sp: &mut Element, v: &Opt<BevelIn>) {
    let b = match v {
        Opt::Remove => {
            sp.remove_children("a:bevelT");
            sp.remove_children("a:bevelB");
            return;
        }
        Opt::Set(b) => b,
    };
    if sp.child("a:bevelT").is_none() {
        put_ordered(sp, Element::new("a:bevelT").with_attr("w", "76200").with_attr("h", "76200").with_attr("prst", "circle"), SP3D_ORDER);
    }
    let e = sp.child_mut("a:bevelT").expect("just added");
    if let Some(v) = b.w_pt {
        e.set_attr("w", emu(v));
    }
    if let Some(v) = b.h_pt {
        e.set_attr("h", emu(v));
    }
    if let Some(p) = &b.preset {
        e.set_attr("prst", p.as_str());
    }
}

fn put_extrusion(sp: &mut Element, v: &Opt<ExtrusionIn>) {
    let x = match v {
        Opt::Remove => {
            sp.remove_attr("extrusionH");
            sp.remove_children("a:extrusionClr");
            return;
        }
        Opt::Set(x) => x,
    };
    if let Some(h) = x.height_pt {
        sp.set_attr("extrusionH", emu(h));
    } else if sp.attr("extrusionH").is_none() {
        sp.set_attr("extrusionH", emu(10.0));
    }
    if let Some(c) = x.color.as_deref().and_then(super::hex6) {
        put_ordered(sp, Element::new("a:extrusionClr").with_child(srgb(&c, 1.0)), SP3D_ORDER);
    }
}

/// Take out the lists that are left empty (an empty `a:effectLst` stays when it overrides a theme style).
fn drop_empty(pr: &mut Element, themed: bool) {
    if !themed && pr.child("a:effectLst").is_some_and(|l| l.children.is_empty()) {
        pr.remove_children("a:effectLst");
    }
    if pr.child("a:sp3d").is_some_and(|s| s.children.is_empty() && s.attrs.is_empty()) {
        pr.remove_children("a:sp3d");
    }
}

fn from_shadow(s: &RShadow) -> ShadowIn {
    ShadowIn { color: Some(s.color.clone()), alpha: Some(s.alpha), blur_pt: Some(s.blur_pt), dist_pt: Some(s.dist_pt), angle: Some(s.angle), sx: s.sx, sy: s.sy, align: s.align.clone() }
}

fn from_reflection(r: &RReflection) -> ReflectionIn {
    ReflectionIn {
        blur_pt: Some(r.blur_pt),
        start_alpha: Some(r.start_alpha),
        end_alpha: Some(r.end_alpha),
        dist_pt: Some(r.dist_pt),
        direction: Some(r.direction),
        fade_direction: Some(r.fade_direction),
        sx: r.sx,
        sy: r.sy,
        start_pos: r.start_pos,
        end_pos: r.end_pos,
    }
}

/// What the outer shadow of a shape is once an op has been applied, for the VML copy: `Some(None)` when it is
/// taken away, `Some(Some(..))` when there is one, `None` when the op leaves it alone.
pub fn final_shadow(spec: &EffectsSpec, cur: Option<&REffects>) -> Option<Option<RShadow>> {
    match spec {
        EffectsSpec::None => Some(None),
        EffectsSpec::Set(e) => match e.shadow.as_ref()? {
            Opt::Remove => Some(None),
            Opt::Set(s) => {
                let old = cur.and_then(|c| c.shadow.clone());
                let base = old.unwrap_or(RShadow { color: "000000".into(), alpha: 0.4, blur_pt: 4.0, dist_pt: 3.0, angle: 45.0, sx: None, sy: None, align: None, rot_with_shape: Some(false) });
                Some(Some(RShadow {
                    color: s.color.as_deref().and_then(super::hex6).unwrap_or(base.color),
                    alpha: s.alpha.unwrap_or(base.alpha),
                    blur_pt: s.blur_pt.unwrap_or(base.blur_pt),
                    dist_pt: s.dist_pt.unwrap_or(base.dist_pt),
                    angle: s.angle.unwrap_or(base.angle),
                    sx: s.sx.or(base.sx),
                    sy: s.sy.or(base.sy),
                    align: s.align.clone().or(base.align),
                    rot_with_shape: base.rot_with_shape,
                }))
            }
        },
    }
}

/// The VML `v:shadow` for an outer shadow.
pub fn vml_shadow(s: &RShadow) -> Element {
    let n = |v: f64| {
        let r = (v * 100.0).round() / 100.0;
        if r.fract() == 0.0 { format!("{}pt", r as i64) } else { format!("{r}pt") }
    };
    let a = s.angle.to_radians();
    Element::new("v:shadow")
        .with_attr("on", "t")
        .with_attr("color", format!("#{}", s.color))
        .with_attr("opacity", format!("{}", (s.alpha * 1000.0).round() / 1000.0))
        .with_attr("offset", format!("{},{}", n(s.dist_pt * a.cos()), n(s.dist_pt * a.sin())))
}
