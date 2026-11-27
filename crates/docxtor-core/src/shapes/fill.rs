//! Fills of shapes: reading `a:solidFill`, `a:gradFill`, `a:blipFill`, `a:pattFill` and VML `v:fill` into the
//! view model, and writing a fill from an op (DrawingML for the `mc:Choice`, VML attributes for the fallback).

use super::appearance::*;
use super::theme::ShapeTheme;
use super::{hex6, RFill};
use crate::xml::Element;

/// Every DrawingML preset pattern (`ST_PresetPatternVal`).
pub const PATTERNS: &[&str] = &[
    "pct5", "pct10", "pct20", "pct25", "pct30", "pct40", "pct50", "pct60", "pct70", "pct75", "pct80", "pct90", "horz", "vert", "ltHorz", "ltVert", "dkHorz",
    "dkVert", "narHorz", "narVert", "dashHorz", "dashVert", "cross", "dnDiag", "upDiag", "ltDnDiag", "ltUpDiag", "dkDnDiag", "dkUpDiag", "wdDnDiag",
    "wdUpDiag", "dashDnDiag", "dashUpDiag", "diagCross", "smCheck", "lgCheck", "smGrid", "lgGrid", "dotGrid", "smConfetti", "lgConfetti", "horzBrick",
    "diagBrick", "solidDmnd", "openDmnd", "dotDmnd", "plaid", "sphere", "weave", "divot", "shingle", "wave", "trellis", "zigZag",
];

const EMU_PER_PT: f64 = super::EMU_PER_PT;

fn num(e: &Element, a: &str) -> Option<f64> {
    e.attr(a).and_then(|v| v.parse::<f64>().ok())
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on" | "t"))
}

fn rect4(e: &Element) -> Rect4 {
    let p = |a: &str| num(e, a).unwrap_or(0.0) / 1000.0;
    Rect4 { l: p("l"), t: p("t"), r: p("r"), b: p("b") }
}

// ---------------------------------------------------------------------------
// Reading DrawingML
// ---------------------------------------------------------------------------

/// Whether an element is one of the fill elements.
pub fn is_fill(name: &str) -> bool {
    matches!(name, "a:noFill" | "a:solidFill" | "a:gradFill" | "a:blipFill" | "a:pattFill" | "a:grpFill")
}

/// A fill element as the view model. `ph` is what `phClr` stands for (a theme style's colour); `src_of` turns a
/// relationship id into a part name. None when the element can't be read (a picture fill without its picture).
pub fn read(th: &ShapeTheme, c: &Element, ph: Option<&(String, f64)>, src_of: &dyn Fn(&str) -> Option<String>) -> Option<RFill> {
    match c.name.as_str() {
        "a:solidFill" => th.color_of(c, ph).map(|(color, alpha)| RFill::solid(color, alpha)),
        "a:gradFill" => {
            let mut stops: Vec<RStop> = c
                .child("a:gsLst")
                .map(|l| l.children_named("a:gs").filter_map(|g| th.color_of(g, ph).map(|(color, alpha)| RStop { pos: (num(g, "pos").unwrap_or(0.0) / 1000.0).clamp(0.0, 100.0), color, alpha })).collect())
                .unwrap_or_default();
            stops.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
            let (first, last) = (stops.first()?.clone(), stops.last()?.clone());
            let lin = c.child("a:lin");
            let path = c.child("a:path").map(|p| match p.attr("path") {
                Some("rect") => "rect",
                Some("shape") => "shape",
                _ => "circle",
            });
            let mut f = RFill::new("gradient", first.color.clone(), first.alpha);
            f.color2 = Some(last.color.clone());
            f.stops = stops;
            if let Some(p) = path {
                f.path = Some(p);
                f.fill_to = Some(c.child("a:path").and_then(|p| p.child("a:fillToRect")).map(rect4).unwrap_or(Rect4 { l: 50.0, t: 50.0, r: 50.0, b: 50.0 }));
            } else {
                f.angle = Some(lin.and_then(|l| num(l, "ang")).unwrap_or(0.0) / 60000.0);
                f.scaled = Some(lin.is_some_and(|l| truthy(l.attr("scaled"))));
            }
            f.rot_with_shape = c.attr("rotWithShape").map(|v| truthy(Some(v)));
            Some(f)
        }
        "a:blipFill" => {
            let blip = c.child("a:blip")?;
            let src = src_of(blip.attr("r:embed")?)?;
            let alpha = blip.child("a:alphaModFix").and_then(|a| num(a, "amt")).map(|v| (v / 100000.0).clamp(0.0, 1.0)).unwrap_or(1.0);
            let mut f = RFill::new("picture", "FFFFFF".into(), alpha);
            f.src = Some(src);
            f.crop = c.child("a:srcRect").map(rect4);
            if let Some(t) = c.child("a:tile") {
                f.tile = Some(RTile {
                    tx: num(t, "tx").unwrap_or(0.0) / EMU_PER_PT,
                    ty: num(t, "ty").unwrap_or(0.0) / EMU_PER_PT,
                    sx: num(t, "sx").unwrap_or(100000.0) / 1000.0,
                    sy: num(t, "sy").unwrap_or(100000.0) / 1000.0,
                    align: t.attr("algn").unwrap_or("tl").to_string(),
                    flip: match t.attr("flip") {
                        Some("x") => "x",
                        Some("y") => "y",
                        Some("xy") => "xy",
                        _ => "none",
                    }
                    .to_string(),
                });
            } else if let Some(s) = c.child("a:stretch") {
                f.stretch = Some(s.child("a:fillRect").map(rect4).unwrap_or_default());
            }
            Some(f)
        }
        "a:pattFill" => {
            let fg = c.child("a:fgClr").and_then(|f| th.color_of(f, ph)).unwrap_or(("000000".into(), 1.0));
            let bg = c.child("a:bgClr").and_then(|f| th.color_of(f, ph)).unwrap_or(("FFFFFF".into(), 1.0));
            let mut f = RFill::new("pattern", fg.0, fg.1);
            f.color2 = Some(bg.0);
            f.pattern = Some(c.attr("prst").unwrap_or("pct50").to_string());
            Some(f)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Reading VML
// ---------------------------------------------------------------------------

/// The VML `colors` list (`0 #fff;.5 red;1 blue`, positions as fractions or `n f` fixed point) as stops.
fn vml_stops(colors: &str) -> Vec<RStop> {
    colors
        .split(';')
        .filter_map(|part| {
            let (p, c) = part.trim().split_once(char::is_whitespace)?;
            let pos = if let Some(fixed) = p.strip_suffix('f') { fixed.parse::<f64>().ok()? / 65536.0 } else if let Some(pc) = p.strip_suffix('%') { pc.parse::<f64>().ok()? / 100.0 } else { p.parse::<f64>().ok()? };
            // A colour can carry a modifier (`red [3]`, `fill darken(118)`): the base is what comes first.
            let color = hex6(c.split_whitespace().next()?)?;
            Some(RStop { pos: (pos * 100.0).clamp(0.0, 100.0), color, alpha: 1.0 })
        })
        .collect()
}

/// A VML `v:fill` as the view model. `shape_color` is the shape's `fillcolor`; `src_of` resolves `r:id`.
pub fn read_vml(fill: Option<&Element>, shape_color: Option<&str>, src_of: &dyn Fn(&str) -> Option<String>, parse_opacity: &dyn Fn(&str) -> Option<f64>) -> RFill {
    let color = fill.and_then(|f| f.attr("color")).or(shape_color).and_then(hex6).unwrap_or_else(|| "FFFFFF".into());
    let alpha = fill.and_then(|f| f.attr("opacity")).and_then(parse_opacity).unwrap_or(1.0);
    let ty = fill.and_then(|f| f.attr("type")).unwrap_or("solid");
    let Some(f) = fill else { return RFill::solid(color, alpha) };
    match ty {
        "gradient" | "gradientRadial" | "gradientradial" => {
            let color2 = f.attr("color2").and_then(hex6).unwrap_or_else(|| "FFFFFF".into());
            let alpha2 = f.attr("o:opacity2").and_then(parse_opacity).unwrap_or(alpha);
            let mut stops = f.attr("colors").map(vml_stops).unwrap_or_default();
            if stops.len() < 2 {
                stops = vec![RStop { pos: 0.0, color: color.clone(), alpha }, RStop { pos: 100.0, color: color2.clone(), alpha: alpha2 }];
            } else {
                let n = stops.len();
                stops[0].alpha = alpha;
                stops[n - 1].alpha = alpha2;
            }
            stops.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
            let (first, last) = (stops[0].clone(), stops[stops.len() - 1].clone());
            let mut g = RFill::new("gradient", first.color, first.alpha);
            g.color2 = Some(last.color);
            g.stops = stops;
            if ty == "gradient" {
                g.angle = Some(f.attr("angle").and_then(|a| a.parse::<f64>().ok()).map(|a| (90.0 - a).rem_euclid(360.0)).unwrap_or(90.0));
                g.scaled = Some(false);
            } else {
                g.path = Some("circle");
                let pos: Vec<f64> = f.attr("focusposition").map(|p| p.split(',').map(|v| v.trim().trim_end_matches('%').parse::<f64>().unwrap_or(50.0) * if v.contains('%') { 1.0 } else { 100.0 }).collect()).unwrap_or_default();
                let (fx, fy) = (pos.first().copied().unwrap_or(50.0), pos.get(1).copied().unwrap_or(50.0));
                g.fill_to = Some(Rect4 { l: fx, t: fy, r: 100.0 - fx, b: 100.0 - fy });
            }
            g
        }
        "tile" | "frame" | "picture" | "pattern" => {
            let src = f.attr("r:id").or(f.attr("o:relid")).and_then(src_of);
            match src {
                Some(src) if ty != "pattern" => {
                    let mut p = RFill::new("picture", "FFFFFF".into(), alpha);
                    p.src = Some(src);
                    if ty == "tile" {
                        p.tile = Some(RTile { tx: 0.0, ty: 0.0, sx: 100.0, sy: 100.0, align: "tl".into(), flip: "none".into() });
                    } else {
                        p.stretch = Some(Rect4::default());
                    }
                    p
                }
                // A pattern is a bitmap in VML: its colours stand for it.
                _ => RFill::solid(color, alpha),
            }
        }
        _ => RFill::solid(color, alpha),
    }
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

/// A fill ready to write: the DrawingML element and the VML that says the same as far as VML can.
#[derive(Debug, Clone)]
pub struct FillWrite {
    pub dml: String,
    /// The shape's `fillcolor`.
    pub color: String,
    /// A `v:fill` element, when the fill is more than a flat colour.
    pub vml: Option<Element>,
}

/// `<a:srgbClr val="..."/>`, with an `a:alpha` when translucent.
pub fn clr(hex: &str, alpha: f64) -> String {
    if (alpha - 1.0).abs() < 1e-6 {
        format!(r#"<a:srgbClr val="{hex}"/>"#)
    } else {
        format!(r#"<a:srgbClr val="{hex}"><a:alpha val="{}"/></a:srgbClr>"#, (alpha * 100000.0).round() as i64)
    }
}

pub fn check_alpha(a: Option<f64>) -> Result<f64, String> {
    let a = a.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&a) {
        return Err("alpha must be between 0 (clear) and 1 (opaque)".into());
    }
    Ok(a)
}

pub fn colour(v: &str) -> Result<String, String> {
    hex6(v).ok_or_else(|| format!("'{v}' is not a colour; use six hex digits like FFF2CC"))
}

fn pct(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

fn rect_attrs(r: &Rect4) -> String {
    format!(r#" l="{}" t="{}" r="{}" b="{}""#, pct(r.l), pct(r.t), pct(r.r), pct(r.b))
}

fn vml_num(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let s = format!("{r}");
    s.strip_prefix("0.").map(|f| format!(".{f}")).unwrap_or(s)
}

/// Resolve a fill from an op. `rid` is the relationship id of the picture (the caller adds the relationship for
/// `media`). `Ok(None)` is no fill.
pub fn resolve(spec: &FillSpec, rid: Option<&str>) -> Result<Option<FillWrite>, String> {
    let solid = |hex: String, alpha: f64| {
        let mut vml = None;
        if alpha < 1.0 {
            vml = Some(Element::new("v:fill").with_attr("opacity", vml_num(alpha)));
        }
        Ok(Some(FillWrite { dml: format!("<a:solidFill>{}</a:solidFill>", clr(&hex, alpha)), color: hex, vml }))
    };
    match spec {
        FillSpec::None => Ok(None),
        FillSpec::Hex(h) => solid(colour(h)?, 1.0),
        FillSpec::Obj(FillObj::Solid { color, alpha }) => solid(colour(color)?, check_alpha(*alpha)?),
        FillSpec::Obj(FillObj::Gradient { stops, angle, path, fill_to, scaled, rot_with_shape }) => {
            if stops.len() < 2 {
                return Err("a gradient needs at least two stops".into());
            }
            if stops.len() > 10 {
                return Err("a gradient can have at most ten stops".into());
            }
            let mut st: Vec<(f64, String, f64)> = Vec::new();
            for s in stops {
                if !(0.0..=100.0).contains(&s.pos) {
                    return Err("a gradient stop's position must be between 0 and 100".into());
                }
                st.push((s.pos, colour(&s.color)?, check_alpha(s.alpha)?));
            }
            st.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let gs: String = st.iter().map(|(p, c, a)| format!(r#"<a:gs pos="{}">{}</a:gs>"#, pct(*p), clr(c, *a))).collect();
            let rot = rot_with_shape.map(|r| format!(r#" rotWithShape="{}""#, r as u8)).unwrap_or_else(|| r#" rotWithShape="1""#.into());
            let dir = match path.as_deref() {
                Some(p @ ("circle" | "rect" | "shape")) => {
                    let r = fill_to.unwrap_or(Rect4 { l: 50.0, t: 50.0, r: 50.0, b: 50.0 });
                    format!(r#"<a:path path="{p}"><a:fillToRect{}/></a:path>"#, rect_attrs(&r))
                }
                Some(other) => return Err(format!("unknown gradient path '{other}'; use circle, rect or shape")),
                None => format!(r#"<a:lin ang="{}" scaled="{}"/>"#, ((angle.unwrap_or(90.0).rem_euclid(360.0)) * 60000.0).round() as i64, scaled.unwrap_or(false) as u8),
            };
            let dml = format!("<a:gradFill{rot}><a:gsLst>{gs}</a:gsLst>{dir}</a:gradFill>");
            // VML: `colors` lists the stops, `angle` runs the other way round, a path gradient is radial.
            let first = &st[0];
            let last = &st[st.len() - 1];
            let mut v = Element::new("v:fill").with_attr("color2", format!("#{}", last.1));
            if first.2 < 1.0 {
                v.set_attr("opacity", vml_num(first.2));
            }
            if last.2 < 1.0 {
                v.set_attr("o:opacity2", vml_num(last.2));
            }
            if st.len() > 2 {
                v.set_attr("colors", st.iter().map(|(p, c, _)| format!("{} #{c}", vml_num(p / 100.0))).collect::<Vec<_>>().join(";"));
            }
            if path.is_some() {
                let r = fill_to.unwrap_or(Rect4 { l: 50.0, t: 50.0, r: 50.0, b: 50.0 });
                v.set_attr("type", "gradientRadial");
                v.set_attr("focusposition", format!("{},{}", vml_num(r.l / 100.0), vml_num(r.t / 100.0)));
            } else {
                v.set_attr("type", "gradient");
                v.set_attr("angle", format!("{}", ((90.0 - angle.unwrap_or(90.0)).rem_euclid(360.0) * 100.0).round() / 100.0));
                v.set_attr("rotate", "t");
            }
            Ok(Some(FillWrite { dml, color: first.1.clone(), vml: Some(v) }))
        }
        FillSpec::Obj(FillObj::Picture { media, tile, stretch, crop, alpha }) => {
            let rid = rid.ok_or_else(|| format!("picture {media} isn't in the document (add it with add_media)"))?;
            let alpha = check_alpha(*alpha)?;
            let blip = if alpha < 1.0 { format!(r#"<a:blip r:embed="{rid}"><a:alphaModFix amt="{}"/></a:blip>"#, (alpha * 100000.0).round() as i64) } else { format!(r#"<a:blip r:embed="{rid}"/>"#) };
            let src = crop.map(|c| format!("<a:srcRect{}/>", rect_attrs(&c))).unwrap_or_default();
            let mode = match tile {
                Some(t) => {
                    let flip = match t.flip.as_deref() {
                        None | Some("none") => "none",
                        Some(f @ ("x" | "y" | "xy")) => f,
                        Some(o) => return Err(format!("unknown tile flip '{o}'; use none, x, y or xy")),
                    };
                    let align = match t.align.as_deref() {
                        None => "tl",
                        Some(a @ ("tl" | "t" | "tr" | "l" | "ctr" | "r" | "bl" | "b" | "br")) => a,
                        Some(o) => return Err(format!("unknown tile alignment '{o}'; use tl, t, tr, l, ctr, r, bl, b or br")),
                    };
                    format!(r#"<a:tile tx="{}" ty="{}" sx="{}" sy="{}" flip="{flip}" algn="{align}"/>"#, (t.tx * EMU_PER_PT).round() as i64, (t.ty * EMU_PER_PT).round() as i64, pct(t.sx), pct(t.sy))
                }
                None => format!("<a:stretch><a:fillRect{}/></a:stretch>", stretch.map(|r| rect_attrs(&r)).unwrap_or_default()),
            };
            let dml = format!(r#"<a:blipFill dpi="0" rotWithShape="1">{blip}{src}{mode}</a:blipFill>"#);
            let mut v = Element::new("v:fill").with_attr("r:id", rid).with_attr("type", if tile.is_some() { "tile" } else { "frame" }).with_attr("rotate", "t");
            if alpha < 1.0 {
                v.set_attr("opacity", vml_num(alpha));
            }
            Ok(Some(FillWrite { dml, color: "FFFFFF".into(), vml: Some(v) }))
        }
        FillSpec::Obj(FillObj::Pattern { pattern, fg, bg, alpha }) => {
            let p = PATTERNS.iter().find(|p| p.eq_ignore_ascii_case(pattern)).ok_or_else(|| format!("unknown pattern '{pattern}'; use one of {}", PATTERNS.join(", ")))?;
            let (fg, bg, a) = (colour(fg)?, colour(bg)?, check_alpha(*alpha)?);
            let dml = format!(r#"<a:pattFill prst="{p}"><a:fgClr>{}</a:fgClr><a:bgClr>{}</a:bgClr></a:pattFill>"#, clr(&fg, a), clr(&bg, a));
            // VML keeps patterns as bitmaps: the foreground stands for it.
            Ok(Some(FillWrite { dml, color: fg, vml: None }))
        }
    }
}
