//! Connectors: lines that stay attached to shapes.
//!
//! A connector is a `wps:wsp` with `wps:cNvCnPr`. `a:stCxn` and `a:endCxn` there name the shape (its drawing id)
//! and the connection site (an index into that shape's sites) each end is glued to. The line itself is one of
//! Word's connector presets (`straightConnector1`, `bentConnector2` to `5`, `curvedConnector2` to `5`) in a box
//! whose turn and mirroring make it leave the first shape and arrive at the second the way Word draws it.
//!
//! This module knows the sites of the shapes, how to pick the preset for two sites, and how to write that into
//! the markup. `crate::document::connectors` keeps connectors attached while ops move the shapes.

use super::freeform::{vml_set_path, Seg, P};
use super::patch::{drop_key, parse, put, style_pairs, style_string};
use super::{build::LineLook, EMU_PER_PT};
use crate::ops::Pos;
use crate::props::put_ordered;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The `w:drawing` of a shape object (for code outside `shapes`).
pub fn drawing_mut(obj: &mut Element) -> Option<&mut Element> {
    super::patch::drawing_mut(obj)
}

/// The first element of this name below `el`.
pub fn find_mut<'a>(el: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    super::patch::find_mut(el, name)
}

// ---------------------------------------------------------------------------
// Connection sites
// ---------------------------------------------------------------------------

/// A place on a shape where a connector can be glued: in points from the top-left of the shape's own box, with
/// the direction a connector leaves it in (degrees: 0 right, 90 down, 180 left, 270 up).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Site {
    pub x: f64,
    pub y: f64,
    pub ang: f64,
}

const T: f64 = 270.0;
const L: f64 = 180.0;
const B: f64 = 90.0;
const R: f64 = 0.0;

fn four(w: f64, h: f64) -> Vec<Site> {
    vec![Site { x: w / 2.0, y: 0.0, ang: T }, Site { x: 0.0, y: h / 2.0, ang: L }, Site { x: w / 2.0, y: h, ang: B }, Site { x: w, y: h / 2.0, ang: R }]
}

fn eight(w: f64, h: f64) -> Vec<Site> {
    let (hc, vc) = (w / 2.0, h / 2.0);
    let k = std::f64::consts::FRAC_1_SQRT_2;
    let (il, ir, it, ib) = (hc - hc * k, hc + hc * k, vc - vc * k, vc + vc * k);
    vec![
        Site { x: hc, y: 0.0, ang: T },
        Site { x: il, y: it, ang: L },
        Site { x: 0.0, y: vc, ang: L },
        Site { x: il, y: ib, ang: B },
        Site { x: hc, y: h, ang: B },
        Site { x: ir, y: ib, ang: R },
        Site { x: w, y: vc, ang: R },
        Site { x: ir, y: it, ang: T },
    ]
}

/// The connection sites of a preset in a `w` by `h` box, numbered as Word numbers them (the order of `cxnLst` in
/// the preset definitions). Presets not listed here get four sites at the middles of the sides.
pub fn preset_sites(prst: &str, adj: &BTreeMap<String, f64>, w: f64, h: f64) -> Vec<Site> {
    let a = |k: &str, d: f64| adj.get(k).copied().unwrap_or(d);
    let (hc, vc) = (w / 2.0, h / 2.0);
    let ss = w.min(h);
    let s = |x: f64, y: f64, ang: f64| Site { x, y, ang };
    match prst {
        "ellipse" | "flowChartConnector" | "flowChartOr" | "flowChartSummingJunction" | "donut" | "noSmoking" => eight(w, h),
        "wedgeEllipseCallout" => {
            let mut v = eight(w, h);
            v.push(s(hc + w * a("adj1", -20833.0) / 100000.0, vc + h * a("adj2", 62500.0) / 100000.0, B));
            v
        }
        "wedgeRectCallout" | "wedgeRoundRectCallout" => {
            let mut v = four(w, h);
            v.push(s(hc + w * a("adj1", -20833.0) / 100000.0, vc + h * a("adj2", 62500.0) / 100000.0, B));
            v
        }
        "triangle" => {
            let x2 = w * a("adj", 50000.0).clamp(0.0, 100000.0) / 100000.0;
            let x1 = x2 / 2.0;
            vec![s(x2, 0.0, T), s(x1, vc, L), s(0.0, h, B), s(x2, h, B), s(w, h, B), s(x1 + hc, vc, R)]
        }
        "rtTriangle" => vec![s(0.0, 0.0, T), s(0.0, vc, L), s(0.0, h, B), s(hc, h, B), s(w, h, B), s(hc, vc, R)],
        "trapezoid" => {
            let x1 = ss * a("adj", 25000.0).clamp(0.0, 50000.0 * w / ss.max(1e-9)) / 200000.0;
            vec![s(hc, 0.0, T), s(x1, vc, L), s(hc, h, B), s(w - x1, vc, R)]
        }
        "pentagon" => {
            // The corners of the regular pentagon that fills the box.
            let pts: Vec<(f64, f64)> = (0..5)
                .map(|i| {
                    let t = (-90.0 + 72.0 * i as f64).to_radians();
                    (t.cos(), t.sin())
                })
                .collect();
            let (x0, x1) = pts.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0)));
            let (y0, y1) = pts.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.1), m.1.max(p.1)));
            let at = |i: usize| ((pts[i].0 - x0) / (x1 - x0) * w, (pts[i].1 - y0) / (y1 - y0) * h);
            // Top, upper left, lower left, lower right, upper right.
            vec![s(at(0).0, at(0).1, T), s(at(4).0, at(4).1, L), s(at(3).0, at(3).1, B), s(at(2).0, at(2).1, B), s(at(1).0, at(1).1, R)]
        }
        "hexagon" => {
            let x1 = ss * a("adj", 25000.0).clamp(0.0, 50000.0 * w / ss.max(1e-9)) / 100000.0;
            vec![s(w, vc, R), s(w - x1, h, B), s(x1, h, B), s(0.0, vc, L), s(x1, 0.0, T), s(w - x1, 0.0, T)]
        }
        "octagon" => {
            let x1 = ss * a("adj", 29289.0).clamp(0.0, 50000.0) / 100000.0;
            vec![s(w, x1, R), s(w, h - x1, R), s(w - x1, h, B), s(x1, h, B), s(0.0, h - x1, L), s(0.0, x1, L), s(x1, 0.0, T), s(w - x1, 0.0, T)]
        }
        "rightArrow" => {
            let a1 = a("adj1", 50000.0).clamp(0.0, 100000.0);
            let dx1 = ss * a("adj2", 50000.0).clamp(0.0, 100000.0 * w / ss.max(1e-9)) / 100000.0;
            let x1 = w - dx1;
            let y1 = vc - h * a1 / 200000.0;
            let x2 = x1 + if hc > 0.0 { y1 * dx1 / hc } else { 0.0 };
            vec![s(x2, 0.0, T), s(0.0, vc, L), s(x2, h, B), s(w, vc, R)]
        }
        "leftArrow" => {
            let a1 = a("adj1", 50000.0).clamp(0.0, 100000.0);
            let dx1 = ss * a("adj2", 50000.0).clamp(0.0, 100000.0 * w / ss.max(1e-9)) / 100000.0;
            let y1 = vc - h * a1 / 200000.0;
            let x2 = dx1 - if hc > 0.0 { y1 * dx1 / hc } else { 0.0 };
            vec![s(x2, 0.0, T), s(0.0, vc, L), s(x2, h, B), s(w, vc, R)]
        }
        "upArrow" | "downArrow" => {
            let a1 = a("adj1", 50000.0).clamp(0.0, 100000.0);
            let dy1 = ss * a("adj2", 50000.0).clamp(0.0, 100000.0 * h / ss.max(1e-9)) / 100000.0;
            let x1 = hc - w * a1 / 200000.0;
            let dy2 = if hc > 0.0 { x1 * dy1 / hc } else { 0.0 };
            let y2 = if prst == "upArrow" { dy1 - dy2 } else { h - dy1 + dy2 };
            vec![s(hc, 0.0, T), s(0.0, y2, L), s(hc, h, B), s(w, y2, R)]
        }
        "homePlate" => {
            let x1 = w - ss * a("adj", 50000.0).clamp(0.0, 100000.0 * w / ss.max(1e-9)) / 100000.0;
            vec![s(x1 / 2.0, 0.0, T), s(0.0, vc, L), s(x1 / 2.0, h, B), s(w, vc, R)]
        }
        "chevron" => {
            let x1 = ss * a("adj", 50000.0).clamp(0.0, 100000.0 * w / ss.max(1e-9)) / 100000.0;
            vec![s(w - x1, 0.0, T), s(x1, vc, L), s(w - x1, h, B), s(w, vc, R)]
        }
        "flowChartInputOutput" => vec![s(hc, 0.0, T), s(w * 2115.0 / 21600.0, vc, L), s(hc, h, B), s(w * 19485.0 / 21600.0, vc, R)],
        "flowChartDocument" => vec![s(hc, 0.0, T), s(0.0, vc, L), s(hc, h * 17322.0 / 21600.0, B), s(w, vc, R)],
        _ => four(w, h),
    }
}

/// The adjust values of a `wps:wsp`'s preset (`adj`, `adj1`, ...).
pub fn adjusts(wsp: &Element) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    if let Some(av) = wsp.child("wps:spPr").and_then(|s| s.child("a:prstGeom")).and_then(|g| g.child("a:avLst")) {
        for g in av.children_named("a:gd") {
            let v = g.attr("fmla").and_then(|f| f.strip_prefix("val ")).and_then(|v| v.trim().parse::<f64>().ok());
            if let (Some(n), Some(v)) = (g.attr("name"), v) {
                out.insert(n.to_string(), v);
            }
        }
    }
    out
}

/// The connection sites of a shape element (`wps:wsp`) drawn in a `w` by `h` box.
pub fn wsp_sites(wsp: &Element, w: f64, h: f64) -> Vec<Site> {
    let Some(sp) = wsp.child("wps:spPr") else { return four(w, h) };
    if let Some(p) = sp.child("a:prstGeom") {
        let prst = p.attr("prst").unwrap_or("rect");
        if prst == "line" || prst == "lineInv" || prst.contains("Connector") {
            return vec![];
        }
        return preset_sites(prst, &adjusts(wsp), w, h);
    }
    if let Some(cg) = sp.child("a:custGeom") {
        // Word writes a site for each corner of a freeform; they count in the path's own space.
        let (pw, ph) = cg.child("a:pathLst").and_then(|l| l.child("a:path")).map(|p| (num(p.attr("w")), num(p.attr("h")))).unwrap_or((0.0, 0.0));
        let (kx, ky) = (if pw > 0.0 { w / pw } else { 1.0 / EMU_PER_PT }, if ph > 0.0 { h / ph } else { 1.0 / EMU_PER_PT });
        return cg
            .child("a:cxnLst")
            .map(|l| {
                l.children_named("a:cxn")
                    .filter_map(|c| {
                        let p = c.child("a:pos")?;
                        let ang = num(c.attr("ang")) / 60000.0;
                        Some(Site { x: num(p.attr("x")) * kx, y: num(p.attr("y")) * ky, ang })
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    four(w, h)
}

fn num(v: Option<&str>) -> f64 {
    v.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// Boxes on the page
// ---------------------------------------------------------------------------

/// A shape's box: the unrotated top-left corner and size in points, its turn (degrees clockwise) and mirroring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geo {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rot: f64,
    pub flip_h: bool,
    pub flip_v: bool,
}

pub type Dir = (i32, i32);

impl Geo {
    /// A point of the box (points from its unrotated top-left corner) as it is drawn on the page.
    pub fn to_page(&self, lx: f64, ly: f64) -> P {
        let x = if self.flip_h { self.w - lx } else { lx };
        let y = if self.flip_v { self.h - ly } else { ly };
        let (dx, dy) = (x - self.w / 2.0, y - self.h / 2.0);
        let (s, c) = self.rot.to_radians().sin_cos();
        (self.x + self.w / 2.0 + dx * c - dy * s, self.y + self.h / 2.0 + dx * s + dy * c)
    }

    /// A direction in the box (degrees) as it points on the page, snapped to an axis.
    pub fn dir_to_page(&self, ang: f64) -> Dir {
        let (s, c) = ang.to_radians().sin_cos();
        let (mut vx, mut vy) = (c, s);
        if self.flip_h {
            vx = -vx;
        }
        if self.flip_v {
            vy = -vy;
        }
        let (s, c) = self.rot.to_radians().sin_cos();
        axis((vx * c - vy * s, vx * s + vy * c))
    }

    /// The site on the page: where it is and which way a connector leaves it.
    pub fn site(&self, s: &Site) -> (P, Dir) {
        (self.to_page(s.x, s.y), self.dir_to_page(s.ang))
    }
}

/// The axis direction a vector points along most.
pub fn axis(v: P) -> Dir {
    if v.0.abs() >= v.1.abs() {
        (if v.0 >= 0.0 { 1 } else { -1 }, 0)
    } else {
        (0, if v.1 >= 0.0 { 1 } else { -1 })
    }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Straight,
    Elbow,
    Curved,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind, String> {
        match s.to_ascii_lowercase().as_str() {
            "straight" | "line" => Ok(Kind::Straight),
            "elbow" | "bent" => Ok(Kind::Elbow),
            "curved" | "curve" => Ok(Kind::Curved),
            other => Err(format!("unknown connector kind '{other}'; use straight, elbow or curved")),
        }
    }

    /// The kind a preset belongs to.
    pub fn of(prst: &str) -> Option<Kind> {
        if prst.starts_with("bentConnector") {
            Some(Kind::Elbow)
        } else if prst.starts_with("curvedConnector") {
            Some(Kind::Curved)
        } else if prst == "straightConnector1" || prst == "line" || prst == "lineInv" {
            Some(Kind::Straight)
        } else {
            None
        }
    }
}

/// How far a connector runs out of a site before it turns, in points.
pub const STUB: f64 = 18.0;

/// A connector's preset and box.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub prst: String,
    pub geo: Geo,
    /// Adjust values that differ from the preset's 50000 default.
    pub adj: Vec<(&'static str, f64)>,
}

fn dominant(dx: f64, dy: f64) -> Dir {
    axis((dx, dy))
}

fn angle_of(d: Dir) -> f64 {
    match d {
        (1, 0) => 0.0,
        (0, 1) => 90.0,
        (-1, 0) => 180.0,
        _ => 270.0,
    }
}

/// The connector between two points. `ds` is the way the first end leaves its site and `de` the way the second
/// end's site faces (a connector arrives against it); None for an end that is not on a shape.
pub fn route(kind: Kind, straight_prst: &str, s: P, ds: Option<Dir>, e: P, de: Option<Dir>) -> Route {
    let (dx, dy) = (e.0 - s.0, e.1 - s.1);
    if kind == Kind::Straight {
        let (w, h) = (dx.abs(), dy.abs());
        let c = ((s.0 + e.0) / 2.0, (s.1 + e.1) / 2.0);
        return Route { prst: straight_prst.into(), geo: Geo { x: c.0 - w / 2.0, y: c.1 - h / 2.0, w, h, rot: 0.0, flip_h: dx < 0.0, flip_v: dy < 0.0 }, adj: vec![] };
    }
    let a: Dir = ds.unwrap_or_else(|| dominant(dx, dy));
    let along = dx * a.0 as f64 + dy * a.1 as f64;
    let n: Dir = (-a.1, a.0);
    let across_n = dx * n.0 as f64 + dy * n.1 as f64;
    let b: Dir = match de {
        Some(d) => (-d.0, -d.1),
        None if along > 0.5 => a,
        None => {
            if across_n >= 0.0 {
                n
            } else {
                (-n.0, -n.1)
            }
        }
    };
    let sigma: i32 = if along >= 0.0 { 1 } else { -1 };
    let ex: Dir = (a.0 * sigma, a.1 * sigma);
    let ey0: Dir = (-ex.1, ex.0);
    let h_signed = dx * ey0.0 as f64 + dy * ey0.1 as f64;
    let ey: Dir = if h_signed >= 0.0 { ey0 } else { (-ey0.0, -ey0.1) };
    let (mut w, mut h) = (along.abs(), h_signed.abs());
    let bx = b.0 * ex.0 + b.1 * ex.1;
    let by = b.0 * ey.0 + b.1 * ey.1;
    let name = |n: u8| format!("{}{}", if kind == Kind::Elbow { "bentConnector" } else { "curvedConnector" }, n);
    let ratio = |v: f64, of: f64| (v / of * 100000.0).round();
    let (prst, adj): (String, Vec<(&'static str, f64)>) = if by == 0 {
        match (sigma, bx) {
            (1, 1) => (name(3), vec![]),
            (1, _) => {
                w = w.max(1.0);
                (name(3), vec![("adj1", ratio(w + STUB, w))])
            }
            (_, -1) => {
                w = w.max(1.0);
                h = h.max(1.0);
                (name(5), vec![("adj1", ratio(-STUB, w)), ("adj3", ratio(w + STUB, w))])
            }
            _ => {
                w = w.max(1.0);
                (name(3), vec![("adj1", ratio(-STUB, w))])
            }
        }
    } else {
        match (sigma, by) {
            (1, 1) => (name(2), vec![]),
            (1, _) => {
                w = w.max(1.0);
                h = h.max(1.0);
                let x1 = if w >= 2.0 * STUB { w / 2.0 } else { STUB };
                (name(4), vec![("adj1", ratio(x1, w)), ("adj2", ratio(h + STUB, h))])
            }
            (_, 1) => {
                w = w.max(1.0);
                h = h.max(1.0);
                let y2 = if h > 2.0 { h / 2.0 } else { -STUB };
                (name(4), vec![("adj1", ratio(-STUB, w)), ("adj2", ratio(y2, h))])
            }
            _ => {
                w = w.max(1.0);
                h = h.max(1.0);
                (name(4), vec![("adj1", ratio(-STUB, w)), ("adj2", ratio(h + STUB, h))])
            }
        }
    };
    let adj = adj.into_iter().filter(|(_, v)| (*v - 50000.0).abs() > 0.5).collect();
    // The turn and mirroring that carry the box's right and down onto `ex` and `ey`. A mirrored box is written the
    // way Word writes it: flipped down when it is not turned, else flipped across and turned half a turn further.
    let mut rot = angle_of(ex);
    let mirrored = ey != (-ex.1, ex.0);
    let flip_v = mirrored && rot == 0.0;
    if mirrored && !flip_v {
        rot = (rot + 180.0) % 360.0;
    }
    let end = (s.0 + ex.0 as f64 * w + ey.0 as f64 * h, s.1 + ex.1 as f64 * w + ey.1 as f64 * h);
    let c = ((s.0 + end.0) / 2.0, (s.1 + end.1) / 2.0);
    Route { prst, geo: Geo { x: c.0 - w / 2.0, y: c.1 - h / 2.0, w, h, rot, flip_h: mirrored && !flip_v, flip_v }, adj }
}

/// `route` for the editor's live preview: the preset, box and adjust values as JSON. A direction of `[0, 0]` means
/// the end is not on a shape.
pub fn route_json(kind: &str, s: P, ds: Dir, e: P, de: Dir) -> Result<serde_json::Value, String> {
    let kind = Kind::parse(kind)?;
    let dir = |d: Dir| (d != (0, 0)).then_some(d);
    let r = route(kind, "straightConnector1", s, dir(ds), e, dir(de));
    let adj: BTreeMap<&str, f64> = r.adj.iter().map(|(k, v)| (*k, *v)).collect();
    Ok(serde_json::json!({"prst": r.prst, "x": r.geo.x, "y": r.geo.y, "w": r.geo.w, "h": r.geo.h, "rot": r.geo.rot, "flipH": r.geo.flip_h, "flipV": r.geo.flip_v, "adj": adj}))
}

/// Where the two ends of a connector are on the page.
pub fn ends(geo: &Geo) -> (P, P) {
    (geo.to_page(0.0, 0.0), geo.to_page(geo.w, geo.h))
}

/// The ways a connector at `geo` of preset `prst` leaves its start and arrives at its end, as directions on the
/// page (the second is the way the end's site faces, pointing away from the line).
pub fn directions(prst: &str, adj: &BTreeMap<String, f64>, geo: &Geo) -> (Dir, Dir) {
    let a = |k: &str| adj.get(k).copied().unwrap_or(50000.0) / 100000.0;
    let sgn = |v: f64| if v < 0.0 { -1 } else { 1 };
    let (w, h) = (geo.w, geo.h);
    let (start, end): (Dir, Dir) = match prst {
        "bentConnector2" | "curvedConnector2" => ((1, 0), (0, 1)),
        "bentConnector3" | "curvedConnector3" => ((sgn(a("adj1")), 0), (if a("adj1") > 1.0 && w > 0.0 { -1 } else { 1 }, 0)),
        "bentConnector4" | "curvedConnector4" => ((sgn(a("adj1")), 0), (0, if a("adj2") > 1.0 && h > 0.0 { -1 } else { 1 })),
        "bentConnector5" | "curvedConnector5" => ((sgn(a("adj1")), 0), (if a("adj3") > 1.0 && w > 0.0 { -1 } else { 1 }, 0)),
        // A straight line runs along the longer side of its box.
        _ => {
            let d = if w >= h { (1, 0) } else { (0, 1) };
            (d, d)
        }
    };
    let page = |d: Dir| geo.dir_to_page(angle_of(d));
    let arrive = page(end);
    (page(start), (-arrive.0, -arrive.1))
}

// ---------------------------------------------------------------------------
// The line a preset draws
// ---------------------------------------------------------------------------

/// The path of a connector preset in a `w` by `h` box (DrawingML's `presetShapeDefinitions`).
pub fn preset_segs(prst: &str, adj: &BTreeMap<String, f64>, w: f64, h: f64) -> Vec<Seg> {
    let a = |k: &str| adj.get(k).copied().unwrap_or(50000.0);
    let (hc, vc) = (w / 2.0, h / 2.0);
    match prst {
        "bentConnector2" => vec![Seg::M((0.0, 0.0)), Seg::L((w, 0.0)), Seg::L((w, h))],
        "bentConnector3" => {
            let x1 = w * a("adj1") / 100000.0;
            vec![Seg::M((0.0, 0.0)), Seg::L((x1, 0.0)), Seg::L((x1, h)), Seg::L((w, h))]
        }
        "bentConnector4" => {
            let (x1, y2) = (w * a("adj1") / 100000.0, h * a("adj2") / 100000.0);
            vec![Seg::M((0.0, 0.0)), Seg::L((x1, 0.0)), Seg::L((x1, y2)), Seg::L((w, y2)), Seg::L((w, h))]
        }
        "bentConnector5" => {
            let (x1, y2, x3) = (w * a("adj1") / 100000.0, h * a("adj2") / 100000.0, w * a("adj3") / 100000.0);
            vec![Seg::M((0.0, 0.0)), Seg::L((x1, 0.0)), Seg::L((x1, y2)), Seg::L((x3, y2)), Seg::L((x3, h)), Seg::L((w, h))]
        }
        "curvedConnector2" => vec![Seg::M((0.0, 0.0)), Seg::C((hc, 0.0), (w, vc), (w, h))],
        "curvedConnector3" => {
            let x2 = w * a("adj1") / 100000.0;
            let (x1, x3, y3) = (x2 / 2.0, (w + x2) / 2.0, h * 3.0 / 4.0);
            vec![Seg::M((0.0, 0.0)), Seg::C((x1, 0.0), (x2, h / 4.0), (x2, vc)), Seg::C((x2, y3), (x3, h), (w, h))]
        }
        "curvedConnector4" => {
            let x2 = w * a("adj1") / 100000.0;
            let y4 = h * a("adj2") / 100000.0;
            let (x1, x3) = (x2 / 2.0, (w + x2) / 2.0);
            let (x4, x5) = ((x2 + x3) / 2.0, (x3 + w) / 2.0);
            let (y1, y5) = (y4 / 2.0, (h + y4) / 2.0);
            let (y2, y3) = (y1 / 2.0, (y1 + y4) / 2.0);
            vec![Seg::M((0.0, 0.0)), Seg::C((x1, 0.0), (x2, y2), (x2, y1)), Seg::C((x2, y3), (x4, y4), (x3, y4)), Seg::C((x5, y4), (w, y5), (w, h))]
        }
        "curvedConnector5" => {
            let x3 = w * a("adj1") / 100000.0;
            let y4 = h * a("adj2") / 100000.0;
            let x6 = w * a("adj3") / 100000.0;
            let (x1, x2) = ((x3 + x6) / 2.0, x3 / 2.0);
            let (x4, x5, x7) = ((x3 + x1) / 2.0, (x6 + x1) / 2.0, (x6 + w) / 2.0);
            let (y1, y5) = (y4 / 2.0, (h + y4) / 2.0);
            let (y2, y3) = (y1 / 2.0, (y1 + y4) / 2.0);
            let (y6, y7) = ((y5 + y4) / 2.0, (y5 + h) / 2.0);
            vec![
                Seg::M((0.0, 0.0)),
                Seg::C((x2, 0.0), (x3, y2), (x3, y1)),
                Seg::C((x3, y3), (x4, y4), (x1, y4)),
                Seg::C((x5, y4), (x6, y6), (x6, y5)),
                Seg::C((x6, y7), (x7, h), (w, h)),
            ]
        }
        _ => vec![Seg::M((0.0, 0.0)), Seg::L((w, h))],
    }
}

// ---------------------------------------------------------------------------
// Connections in the markup
// ---------------------------------------------------------------------------

/// What an end of a connector is glued to in an op: `"free"` (nothing), or `{shape, idx}` (the shape's position
/// in the text and the index of the site on it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndSpec {
    Free,
    To { shape: Pos, idx: usize },
}

#[derive(Serialize, Deserialize)]
struct EndTo {
    shape: Pos,
    idx: usize,
}

impl Serialize for EndSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            EndSpec::Free => s.serialize_str("free"),
            EndSpec::To { shape, idx } => EndTo { shape: *shape, idx: *idx }.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for EndSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("free") || s.eq_ignore_ascii_case("none") => Ok(EndSpec::Free),
            serde_json::Value::Object(_) => serde_json::from_value::<EndTo>(v).map(|e| EndSpec::To { shape: e.shape, idx: e.idx }).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("an end must be \"free\" or {shape, idx}")),
        }
    }
}

/// A connection: the drawing id of a shape and the index of one of its sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attach {
    pub id: u32,
    pub idx: usize,
}

fn attach_of(e: Option<&Element>) -> Option<Attach> {
    let e = e?;
    Some(Attach { id: e.attr("id")?.parse().ok()?, idx: e.attr("idx")?.parse().ok()? })
}

/// Whether the shape element is a connector (a line or connector shape: it has `wps:cNvCnPr`).
pub fn is_connector(wsp: &Element) -> bool {
    wsp.child("wps:cNvCnPr").is_some()
}

/// What the ends of a connector are glued to.
pub fn attachments(wsp: &Element) -> (Option<Attach>, Option<Attach>) {
    match wsp.child("wps:cNvCnPr") {
        Some(c) => (attach_of(c.child("a:stCxn")), attach_of(c.child("a:endCxn"))),
        None => (None, None),
    }
}

/// What an end of a connector is glued to, as the renderer receives it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RAttach {
    /// The drawing id of the shape.
    pub id: u32,
    /// Which of the shape's sites.
    pub idx: usize,
}

/// The glued ends of a connector, as the renderer receives them.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RConnect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<RAttach>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<RAttach>,
}

/// The glued ends of a connector shape (`{}` when it is glued to nothing); None for a shape that is not a connector.
pub fn connect_of(wsp: &Element) -> Option<RConnect> {
    if !is_connector(wsp) {
        return None;
    }
    let (start, end) = attachments(wsp);
    let r = |a: Attach| RAttach { id: a.id, idx: a.idx };
    Some(RConnect { start: start.map(r), end: end.map(r) })
}

const CNV_ORDER: &[&str] = &["a:cxnSpLocks", "a:stCxn", "a:endCxn", "a:extLst"];

/// Set what the ends are glued to. `None` leaves an end as it is, `Some(None)` unglues it.
pub fn set_attachments(wsp: &mut Element, start: Option<Option<Attach>>, end: Option<Option<Attach>>) {
    let Some(c) = wsp.child_mut("wps:cNvCnPr") else { return };
    for (name, v) in [("a:stCxn", start), ("a:endCxn", end)] {
        match v {
            None => {}
            Some(None) => {
                c.remove_children(name);
            }
            Some(Some(a)) => put_ordered(c, Element::new(name).with_attr("id", a.id.to_string()).with_attr("idx", a.idx.to_string()), CNV_ORDER),
        }
    }
}

// ---------------------------------------------------------------------------
// Writing a route into a connector
// ---------------------------------------------------------------------------

fn emu(v: f64) -> i64 {
    (v * EMU_PER_PT).round() as i64
}

fn trim(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// Where the connector is written: a shape on its own in the text (its position is in `wp:anchor`) or a member of
/// a group or canvas (its position is in its `a:xfrm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Top,
    Member,
}

fn av_list(route: &Route) -> Element {
    let mut av = Element::new("a:avLst");
    for (k, v) in &route.adj {
        av.children.push(Node::Element(Element::new("a:gd").with_attr("name", *k).with_attr("fmla", format!("val {}", *v as i64))));
    }
    av
}

fn write_xfrm(x: &mut Element, g: &Geo, off: bool) {
    if g.rot == 0.0 {
        x.remove_attr("rot");
    } else {
        x.set_attr("rot", ((g.rot * 60000.0).round() as i64).to_string());
    }
    for (name, on) in [("flipH", g.flip_h), ("flipV", g.flip_v)] {
        if on {
            x.set_attr(name, "1");
        } else {
            x.remove_attr(name);
        }
    }
    if let Some(e) = x.child_mut("a:ext") {
        e.set_attr("cx", emu(g.w).to_string());
        e.set_attr("cy", emu(g.h).to_string());
    }
    if off {
        if let Some(o) = x.child_mut("a:off") {
            o.set_attr("x", emu(g.x).to_string());
            o.set_attr("y", emu(g.y).to_string());
        }
    }
}

const SPPR_ORDER: &[&str] = &["a:xfrm", "a:custGeom", "a:prstGeom", "a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill", "a:ln", "a:effectLst", "a:effectDag", "a:scene3d", "a:sp3d", "a:extLst"];

/// Write the preset, box and adjust values into the shape element of a connector.
fn write_wsp(wsp: &mut Element, route: &Route, place: Where) {
    if let Some(pr) = wsp.child_mut("wps:spPr") {
        pr.remove_children("a:custGeom");
        let mut g = Element::new("a:prstGeom").with_attr("prst", route.prst.as_str());
        g.children.push(Node::Element(av_list(route)));
        put_ordered(pr, g, SPPR_ORDER);
        if pr.child("a:xfrm").is_none() {
            put_ordered(pr, parse(r#"<a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm>"#), SPPR_ORDER);
        }
        if let Some(x) = pr.child_mut("a:xfrm") {
            write_xfrm(x, &route.geo, place == Where::Member);
        }
    }
}

/// The VML copy of a connector: its box, turn and mirroring in the style, and the line it draws as a path.
fn write_vml(sh: &mut Element, route: &Route, place: Where) {
    let g = &route.geo;
    let adj: BTreeMap<String, f64> = route.adj.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    let segs = preset_segs(&route.prst, &adj, emu(g.w) as f64, emu(g.h) as f64);
    let path: String = {
        let r = |p: P| (p.0.round() as i64, p.1.round() as i64);
        let mut s = String::new();
        for seg in &segs {
            match *seg {
                Seg::M(a) => s.push_str(&format!("m{},{}", r(a).0, r(a).1)),
                Seg::L(a) => s.push_str(&format!("l{},{}", r(a).0, r(a).1)),
                Seg::C(a, b, c) => s.push_str(&format!("c{},{},{},{},{},{}", r(a).0, r(a).1, r(b).0, r(b).1, r(c).0, r(c).1)),
                Seg::Z => s.push('x'),
            }
        }
        s.push('e');
        s
    };
    vml_set_path(sh, &path, (emu(g.w).max(1), emu(g.h).max(1)));
    let kind = match Kind::of(&route.prst) {
        Some(Kind::Elbow) => "elbow",
        Some(Kind::Curved) => "curved",
        _ => "straight",
    };
    sh.set_attr("o:connectortype", kind);
    let mut style = style_pairs(sh.attr("style").unwrap_or(""));
    put(&mut style, "width", format!("{}pt", trim(g.w)));
    put(&mut style, "height", format!("{}pt", trim(g.h)));
    if place == Where::Top {
        put(&mut style, "margin-left", format!("{}pt", trim(g.x)));
        put(&mut style, "margin-top", format!("{}pt", trim(g.y)));
    }
    if g.rot == 0.0 {
        drop_key(&mut style, "rotation");
    } else {
        put(&mut style, "rotation", trim(g.rot));
    }
    match (g.flip_h, g.flip_v) {
        (false, false) => drop_key(&mut style, "flip"),
        (true, false) => put(&mut style, "flip", "x"),
        (false, true) => put(&mut style, "flip", "y"),
        (true, true) => put(&mut style, "flip", "x y"),
    }
    sh.set_attr("style", style_string(&style));
}

/// Write a route into a connector object. `obj` is the object in the text (`mc:AlternateContent`, `w:drawing`) for
/// `Where::Top`, the `wps:wsp` for `Where::Member`. The geometry is in the frame the position counts from.
pub fn write(obj: &mut Element, route: &Route, place: Where) -> Result<(), String> {
    let g = route.geo;
    if place == Where::Member {
        write_wsp(obj, route, place);
        return Ok(());
    }
    let drawing = drawing_mut(obj).ok_or("not a connector")?;
    {
        let c = find_mut(drawing, "wp:anchor").ok_or("a connector in line with the text can't be attached to shapes; wrap it first")?;
        if let Some(e) = c.child_mut("wp:extent") {
            e.set_attr("cx", emu(g.w).to_string());
            e.set_attr("cy", emu(g.h).to_string());
        }
        // The room the turned box takes outside the unturned one.
        let a = g.rot.to_radians();
        let (bw, bh) = (g.w * a.cos().abs() + g.h * a.sin().abs(), g.w * a.sin().abs() + g.h * a.cos().abs());
        let (ex, ey) = (emu(((bw - g.w) / 2.0).max(0.0)), emu(((bh - g.h) / 2.0).max(0.0)));
        if let Some(e) = c.child_mut("wp:effectExtent") {
            for (k, v) in [("l", ex), ("r", ex), ("t", ey), ("b", ey)] {
                e.set_attr(k, v.to_string());
            }
        }
        for (name, v) in [("wp:positionH", g.x), ("wp:positionV", g.y)] {
            if let Some(pos) = c.child_mut(name) {
                pos.children = vec![Node::Element(Element::new("wp:posOffset").with_text(emu(v).to_string()))];
            }
        }
        if let Some(wsp) = find_mut(c, "wps:wsp") {
            write_wsp(wsp, route, place);
        }
    }
    let vml = |root: &mut Element| {
        let picts: Vec<&mut Element> = if root.name == "w:pict" { vec![root] } else { root.elements_mut().filter(|e| e.name == "w:pict").collect() };
        for pict in picts {
            for sh in pict.elements_mut().filter(|e| matches!(e.name.as_str(), "v:line" | "v:shape")) {
                write_vml(sh, route, place);
            }
        }
    };
    if obj.name == "mc:AlternateContent" {
        if let Some(fb) = obj.child_mut("mc:Fallback") {
            vml(fb);
        }
    } else if obj.name == "w:pict" {
        vml(obj);
    }
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

/// A new connector.
#[derive(Debug, Clone)]
pub struct Spec {
    pub route: Route,
    pub line: Option<LineLook>,
    pub start: Option<Attach>,
    pub end: Option<Attach>,
    pub id: u32,
}

/// The name Word gives a new connector.
pub fn default_name(kind: Kind, arrows: bool, id: u32) -> String {
    match (kind, arrows) {
        (Kind::Straight, false) => format!("Straight Connector {id}"),
        (Kind::Straight, true) => format!("Straight Arrow Connector {id}"),
        (Kind::Elbow, _) => format!("Connector: Elbow {id}"),
        (Kind::Curved, _) => format!("Connector: Curved {id}"),
    }
}

/// A connector as Word writes it, in front of the text, with its VML copy.
pub fn build(spec: &Spec) -> Element {
    let id = spec.id;
    let kind = Kind::of(&spec.route.prst).unwrap_or(Kind::Straight);
    let arrows = spec.line.as_ref().is_some_and(|l| l.head.is_some() || l.tail.is_some());
    let name = esc(&default_name(kind, arrows, id));
    let cxn: String = [("a:stCxn", spec.start), ("a:endCxn", spec.end)].iter().filter_map(|(n, a)| a.map(|a| format!(r#"<{n} id="{}" idx="{}"/>"#, a.id, a.idx))).collect();
    let ln = super::build::ln_xml(&spec.line);
    let stroke = match &spec.line {
        Some(l) => format!(r##" strokecolor="#{}" strokeweight="{}pt""##, l.color, trim(l.width_pt)),
        None => r#" stroked="f""#.into(),
    };
    let stroke_child = spec.line.as_ref().map(super::freeform::vml_stroke_children).unwrap_or_default();
    let src = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="{}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="0" cy="0"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{}"><wps:wsp><wps:cNvCnPr>{cxn}</wps:cNvCnPr><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm><a:prstGeom prst="straightConnector1"><a:avLst/></a:prstGeom>{ln}</wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic><wp14:sizeRelH relativeFrom="margin"><wp14:pctWidth>0</wp14:pctWidth></wp14:sizeRelH><wp14:sizeRelV relativeFrom="margin"><wp14:pctHeight>0</wp14:pctHeight></wp14:sizeRelV></wp:anchor></w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape id="{name}" o:spid="_x0000_s{}" style="position:absolute;margin-left:0;margin-top:0;width:0;height:0;z-index:{};visibility:visible;mso-wrap-style:square;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text" coordsize="1,1" o:spt="100" path="m0,0l1,1e" filled="f"{stroke}>{stroke_child}<w10:wrap type="none"/></v:shape></w:pict></mc:Fallback></mc:AlternateContent>"#,
        251658240 + id as u64,
        super::build::NS_WPS,
        1024 + id,
        251658240 + id as u64,
    );
    let mut el = super::freeform::parse_fragment(&src);
    write(&mut el, &spec.route, Where::Top).expect("a new connector is anchored");
    el
}
