//! Curves, freeforms and scribbles: shapes that carry their own outline (`a:custGeom`), written as Word writes
//! them with the VML copy in the fallback, and changing the outline of any shape ("Edit points", `plan`).
//!
//! An outline is a list of path commands. The ops carry them as `{t, pts}`: `M` (move to one point), `L` (line
//! to each point), `C` (cubic Bézier, three points per curve), `Z` (close) and `S`, which is only for input: a
//! smooth curve through the points, starting at the current point. The file only ever holds `M`, `L`, `C` and `Z`.

use super::build::{fill_xml, ln_xml, LineLook, NS_W, NS_WPS, ROOT_NS};
use super::group::{simplify, smooth};
use super::patch::parse;
use super::EMU_PER_PT;
use crate::props::put_ordered;
use crate::xml::{self, Element, Node};
use serde::{Deserialize, Serialize};

pub type P = (f64, f64);

/// One path command as an op carries it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CmdIn {
    /// `M`, `L`, `C`, `Z` or `S`.
    pub t: String,
    #[serde(default)]
    pub pts: Vec<[f64; 2]>,
}

fn yes() -> bool {
    true
}

/// One path of an outline as an op carries it: commands in a `w` by `h` space (0: points of the shape's box).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathIn {
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub h: f64,
    pub cmds: Vec<CmdIn>,
    #[serde(default = "yes")]
    pub fill: bool,
    #[serde(default = "yes")]
    pub stroke: bool,
}

/// A path command with its points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    M(P),
    L(P),
    C(P, P, P),
    Z,
}

const MAX_POINTS: usize = 200_000;
const MAX_ABS: f64 = 1e7;

fn point(p: [f64; 2]) -> Result<P, String> {
    if !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > MAX_ABS || p[1].abs() > MAX_ABS {
        return Err("a point of the outline is off the page".into());
    }
    Ok((p[0], p[1]))
}

/// The commands of an op as segments (`S` becomes smooth curves). The first command must be a move.
pub fn segs(cmds: &[CmdIn]) -> Result<Vec<Seg>, String> {
    if cmds.iter().map(|c| c.pts.len()).sum::<usize>() > MAX_POINTS {
        return Err("the outline has too many points".into());
    }
    let mut out: Vec<Seg> = Vec::new();
    let mut cur: Option<P> = None;
    let mut start: Option<P> = None;
    for c in cmds {
        let pts: Vec<P> = c.pts.iter().map(|p| point(*p)).collect::<Result<_, _>>()?;
        match c.t.to_ascii_uppercase().as_str() {
            "M" => {
                let first = *pts.first().ok_or("a move needs a point")?;
                out.push(Seg::M(first));
                cur = Some(first);
                start = Some(first);
                for p in &pts[1..] {
                    out.push(Seg::L(*p));
                    cur = Some(*p);
                }
            }
            "L" => {
                if cur.is_none() {
                    return Err("the outline must start with a move".into());
                }
                for p in pts {
                    out.push(Seg::L(p));
                    cur = Some(p);
                }
            }
            "C" => {
                if cur.is_none() {
                    return Err("the outline must start with a move".into());
                }
                if pts.is_empty() || !pts.len().is_multiple_of(3) {
                    return Err("a curve needs three points: two handles and the end".into());
                }
                for t in pts.chunks(3) {
                    out.push(Seg::C(t[0], t[1], t[2]));
                    cur = Some(t[2]);
                }
            }
            "S" => {
                let from = cur.ok_or("the outline must start with a move")?;
                if pts.is_empty() {
                    continue;
                }
                let mut line = vec![from];
                line.extend(pts);
                for s in smooth_open(&line) {
                    out.push(s);
                }
                cur = Some(*line.last().unwrap());
            }
            "Z" => {
                if cur.is_none() {
                    return Err("the outline must start with a move".into());
                }
                out.push(Seg::Z);
                cur = start;
            }
            other => return Err(format!("unknown path command '{other}'; use M, L, C, S or Z")),
        }
    }
    if out.is_empty() {
        return Err("the outline is empty".into());
    }
    Ok(out)
}

/// A smooth open curve through the points (the first is where it starts): Bézier segments after simplifying.
pub fn smooth_open(points: &[P]) -> Vec<Seg> {
    let pts = simplify(points, 0.35);
    if pts.len() < 2 {
        return vec![];
    }
    if pts.len() == 2 {
        return vec![Seg::L(pts[1])];
    }
    let (_, beziers) = smooth(&pts);
    beziers.into_iter().map(|b| Seg::C(b[0], b[1], b[2])).collect()
}

/// A smooth closed curve through the points: segments that wrap round to the first point, then a close.
pub fn smooth_closed(points: &[P]) -> Vec<Seg> {
    let n = points.len();
    let mut out = vec![Seg::M(points[0])];
    if n < 3 {
        for p in &points[1..] {
            out.push(Seg::L(*p));
        }
        out.push(Seg::Z);
        return out;
    }
    for i in 0..n {
        let (p0, p1, p2, p3) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n], points[(i + 2) % n]);
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        out.push(Seg::C(c1, c2, p2));
    }
    out.push(Seg::Z);
    out
}

/// The segments of a curve, freeform or scribble from the points the op gave.
pub fn from_points(points: &[P], smooth: bool, closed: bool) -> Vec<Seg> {
    let mut pts: Vec<P> = Vec::new();
    for p in points {
        if pts.last().is_none_or(|l| (l.0 - p.0).abs() > 1e-9 || (l.1 - p.1).abs() > 1e-9) {
            pts.push(*p);
        }
    }
    if closed && pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    if smooth && closed && pts.len() >= 3 {
        return smooth_closed(&pts);
    }
    let mut out = vec![Seg::M(pts[0])];
    if smooth && pts.len() >= 3 {
        out.extend(smooth_open(&pts));
    } else {
        out.extend(pts[1..].iter().map(|p| Seg::L(*p)));
    }
    if closed {
        out.push(Seg::Z);
    }
    out
}

// ---------------------------------------------------------------------------
// Bounds
// ---------------------------------------------------------------------------

/// The extent of a cubic Bézier curve on one axis (its end points and where it turns).
fn axis_extent(p0: f64, c1: f64, c2: f64, p3: f64) -> (f64, f64) {
    let (mut lo, mut hi) = (p0.min(p3), p0.max(p3));
    let (d0, d1, d2) = (c1 - p0, c2 - c1, p3 - c2);
    let (a, b, c) = (d0 - 2.0 * d1 + d2, 2.0 * (d1 - d0), d0);
    let mut roots = Vec::new();
    if a.abs() < 1e-12 {
        if b.abs() > 1e-12 {
            roots.push(-c / b);
        }
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let s = disc.sqrt();
            roots.push((-b + s) / (2.0 * a));
            roots.push((-b - s) / (2.0 * a));
        }
    }
    for t in roots {
        if t > 0.0 && t < 1.0 {
            let u = 1.0 - t;
            let v = u * u * u * p0 + 3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t * p3;
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    (lo, hi)
}

/// Left, top, right and bottom of the segments (a curve counts where it really goes, not its handles).
pub fn bounds(segs: &[Seg]) -> Option<(f64, f64, f64, f64)> {
    let (mut x1, mut y1, mut x2, mut y2) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut cur: Option<P> = None;
    let mut grow = |p: P| {
        x1 = x1.min(p.0);
        y1 = y1.min(p.1);
        x2 = x2.max(p.0);
        y2 = y2.max(p.1);
    };
    for s in segs {
        match *s {
            Seg::M(p) | Seg::L(p) => {
                grow(p);
                cur = Some(p);
            }
            Seg::C(c1, c2, p) => {
                let from = cur.unwrap_or(c1);
                let (lx, hx) = axis_extent(from.0, c1.0, c2.0, p.0);
                let (ly, hy) = axis_extent(from.1, c1.1, c2.1, p.1);
                grow((lx, ly));
                grow((hx, hy));
                cur = Some(p);
            }
            Seg::Z => {}
        }
    }
    (x1 <= x2).then_some((x1, y1, x2, y2))
}

/// The points a connector can attach to: every end point of a move, line or curve.
fn vertices(segs: &[Seg]) -> Vec<P> {
    segs.iter()
        .filter_map(|s| match *s {
            Seg::M(p) | Seg::L(p) => Some(p),
            Seg::C(_, _, p) => Some(p),
            Seg::Z => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Arcs, for reading `a:arcTo` as curves
// ---------------------------------------------------------------------------

/// DrawingML's `arcTo` (radii, start angle and sweep in degrees, from the current point) as cubic Béziers.
pub fn arc_beziers(cur: P, wr: f64, hr: f64, st: f64, sw: f64) -> Vec<[P; 3]> {
    let param = |deg: f64| {
        let r = deg.to_radians();
        (wr * r.sin()).atan2(hr * r.cos())
    };
    let t0 = param(st);
    let mut delta = param(st + sw) - t0;
    if sw.abs() >= 360.0 {
        delta = if sw > 0.0 { std::f64::consts::TAU } else { -std::f64::consts::TAU };
    } else {
        while sw > 0.0 && delta < 0.0 {
            delta += std::f64::consts::TAU;
        }
        while sw < 0.0 && delta > 0.0 {
            delta -= std::f64::consts::TAU;
        }
    }
    let (cx, cy) = (cur.0 - wr * t0.cos(), cur.1 - hr * t0.sin());
    let steps = ((delta.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
    let step = delta / steps as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let at = |t: f64| (cx + wr * t.cos(), cy + hr * t.sin());
    let tangent = |t: f64| (-wr * t.sin(), hr * t.cos());
    (0..steps)
        .map(|i| {
            let (a, b) = (t0 + step * i as f64, t0 + step * (i + 1) as f64);
            let (pa, pb, da, db) = (at(a), at(b), tangent(a), tangent(b));
            [(pa.0 + k * da.0, pa.1 + k * da.1), (pb.0 - k * db.0, pb.1 - k * db.1), pb]
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Writing the outline
// ---------------------------------------------------------------------------

fn emu(v: f64) -> i64 {
    (v * EMU_PER_PT).round() as i64
}

/// One path in EMU, relative to the shape's box.
struct Placed {
    segs: Vec<Seg>,
    fill: bool,
    stroke: bool,
}

fn rounded(p: P) -> (i64, i64) {
    (p.0.round() as i64, p.1.round() as i64)
}

fn pt_xml(p: P) -> String {
    let (x, y) = rounded(p);
    format!(r#"<a:pt x="{x}" y="{y}"/>"#)
}

fn path_xml(p: &Placed, w: i64, h: i64) -> String {
    let mut s = format!(r#"<a:path w="{}" h="{}""#, w.max(1), h.max(1));
    if !p.fill {
        s.push_str(r#" fill="none""#);
    }
    if !p.stroke {
        s.push_str(r#" stroke="0""#);
    }
    s.push('>');
    for seg in &p.segs {
        match *seg {
            Seg::M(a) => s.push_str(&format!("<a:moveTo>{}</a:moveTo>", pt_xml(a))),
            Seg::L(a) => s.push_str(&format!("<a:lnTo>{}</a:lnTo>", pt_xml(a))),
            Seg::C(a, b, c) => s.push_str(&format!("<a:cubicBezTo>{}{}{}</a:cubicBezTo>", pt_xml(a), pt_xml(b), pt_xml(c))),
            Seg::Z => s.push_str("<a:close/>"),
        }
    }
    s.push_str("</a:path>");
    s
}

/// VML path text for the paths, in the units of the shape's `coordsize`.
fn vml_path(paths: &[Placed]) -> String {
    let mut s = String::new();
    for p in paths {
        for seg in &p.segs {
            match *seg {
                Seg::M(a) => {
                    let (x, y) = rounded(a);
                    s.push_str(&format!("m{x},{y}"));
                }
                Seg::L(a) => {
                    let (x, y) = rounded(a);
                    s.push_str(&format!("l{x},{y}"));
                }
                Seg::C(a, b, c) => {
                    let (a, b, c) = (rounded(a), rounded(b), rounded(c));
                    s.push_str(&format!("c{},{},{},{},{},{}", a.0, a.1, b.0, b.1, c.0, c.1));
                }
                Seg::Z => s.push('x'),
            }
        }
    }
    s.push('e');
    s
}

/// Connection sites Word writes for a freeform: one at each vertex (none for a very long outline).
fn cxn_xml(paths: &[Placed]) -> String {
    let all: Vec<P> = paths.iter().flat_map(|p| vertices(&p.segs)).collect();
    if all.len() > 100 {
        return String::new();
    }
    all.into_iter().map(|p| format!(r#"<a:cxn ang="0"><a:pos x="{}" y="{}"/></a:cxn>"#, rounded(p).0, rounded(p).1)).collect()
}

/// The `a:custGeom` of the paths, and the VML for them: `(geometry markup, vml path, coordsize)`.
struct Geom {
    xml: String,
    vml: String,
    coord: (i64, i64),
}

fn geometry(paths: &[Placed], w: i64, h: i64) -> Geom {
    let list: String = paths.iter().map(|p| path_xml(p, w, h)).collect();
    let xml = format!(
        r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst>{}</a:cxnLst><a:rect l="0" t="0" r="r" b="b"/><a:pathLst>{list}</a:pathLst></a:custGeom>"#,
        cxn_xml(paths)
    );
    Geom { xml, vml: vml_path(paths), coord: (w.max(1), h.max(1)) }
}

/// Move the paths so the box starts at 0,0 and turn the points into EMU: `(paths, width, height)`.
fn place(all: Vec<(Vec<Seg>, bool, bool)>, origin: P, scale: &dyn Fn(usize) -> (f64, f64)) -> Option<(Vec<Placed>, i64, i64, P)> {
    // Everything in EMU first, so the bounds are the shape's real box.
    let scaled: Vec<(Vec<Seg>, bool, bool)> = all
        .into_iter()
        .enumerate()
        .map(|(i, (segs, f, s))| {
            let (kx, ky) = scale(i);
            let m = |p: P| ((p.0 - origin.0) * kx, (p.1 - origin.1) * ky);
            let segs = segs
                .into_iter()
                .map(|s| match s {
                    Seg::M(a) => Seg::M(m(a)),
                    Seg::L(a) => Seg::L(m(a)),
                    Seg::C(a, b, c) => Seg::C(m(a), m(b), m(c)),
                    Seg::Z => Seg::Z,
                })
                .collect();
            (segs, f, s)
        })
        .collect();
    let (mut x1, mut y1, mut x2, mut y2) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (s, _, _) in &scaled {
        let (a, b, c, d) = bounds(s)?;
        x1 = x1.min(a);
        y1 = y1.min(b);
        x2 = x2.max(c);
        y2 = y2.max(d);
    }
    let shift = |p: P| (p.0 - x1, p.1 - y1);
    let placed = scaled
        .into_iter()
        .map(|(segs, fill, stroke)| Placed {
            segs: segs
                .into_iter()
                .map(|s| match s {
                    Seg::M(a) => Seg::M(shift(a)),
                    Seg::L(a) => Seg::L(shift(a)),
                    Seg::C(a, b, c) => Seg::C(shift(a), shift(b), shift(c)),
                    Seg::Z => Seg::Z,
                })
                .collect(),
            fill,
            stroke,
        })
        .collect();
    Some((placed, (x2 - x1).round() as i64, (y2 - y1).round() as i64, (x1, y1)))
}

// ---------------------------------------------------------------------------
// A new freeform
// ---------------------------------------------------------------------------

/// A new curve, freeform or scribble. Points are in points; the first point of the outline sits at `x`, `y`.
#[derive(Debug, Clone)]
pub struct FreeSpec {
    pub segs: Vec<Seg>,
    pub closed: bool,
    pub fill: Option<String>,
    pub line: Option<LineLook>,
    pub floating: bool,
    pub x: f64,
    pub y: f64,
    pub id: u32,
    pub label: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

fn trim(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// Parse markup that uses the shape namespaces, then drop the declarations (they live on the document root).
pub fn parse_fragment(body: &str) -> Element {
    let decls: String = ROOT_NS.iter().map(|(p, u)| format!(r#" xmlns:{p}="{u}""#)).collect::<String>() + &format!(r#" xmlns:w="{NS_W}""#);
    let src = body.replacen('>', &format!("{decls}>"), 1);
    let mut el = xml::parse(src.as_bytes()).expect("valid shape xml").root;
    el.attrs.retain(|(k, _)| !k.starts_with("xmlns:"));
    el
}

/// The `v:stroke` of an outline: its dash and arrow heads, joined square.
pub fn vml_stroke_children(l: &LineLook) -> String {
    let arrow = |a: &Option<String>| {
        a.as_deref().map(|t| match t {
            "triangle" => "block",
            "stealth" => "classic",
            "diamond" => "diamond",
            "oval" => "oval",
            _ => "open",
        })
    };
    let dash = l.dash.as_deref().map(|d| match d {
        "dot" | "sysDot" => "dot",
        "lgDash" => "longdash",
        "dashDot" | "sysDashDot" => "dashdot",
        "lgDashDot" => "longdashdot",
        "lgDashDotDot" | "sysDashDotDot" => "longdashdotdot",
        _ => "dash",
    });
    let mut s = String::from("<v:stroke");
    if let Some(d) = dash {
        s.push_str(&format!(r#" dashstyle="{d}""#));
    }
    if let Some(a) = arrow(&l.head) {
        s.push_str(&format!(r#" startarrow="{a}""#));
    }
    if let Some(a) = arrow(&l.tail) {
        s.push_str(&format!(r#" endarrow="{a}""#));
    }
    s.push_str(r#" joinstyle="miter"/>"#);
    s
}

/// The freeform's markup and its size in points.
pub fn build_free(spec: &FreeSpec) -> Result<(Element, f64, f64), String> {
    let first = match spec.segs.first() {
        Some(Seg::M(p)) => *p,
        _ => return Err("the outline must start with a move".into()),
    };
    // An open outline is still a path that takes a fill (Word fills it as if it were closed), so a fill can be added later.
    let (paths, w, h, min) = place(vec![(spec.segs.clone(), true, true)], (0.0, 0.0), &|_| (EMU_PER_PT, EMU_PER_PT)).ok_or("the outline has no points")?;
    let geom = geometry(&paths, w, h);
    let (w_pt, h_pt) = (w as f64 / EMU_PER_PT, h as f64 / EMU_PER_PT);
    let (min_x, min_y) = (min.0 / EMU_PER_PT, min.1 / EMU_PER_PT);
    let (px, py) = (spec.x + (min_x - first.0), spec.y + (min_y - first.1));
    let id = spec.id;
    let name = esc(&format!("{} {id}", spec.label));
    let uid = 1024 + id;
    let fill = fill_xml(&spec.fill);
    let ln = ln_xml(&spec.line);
    let (cx, cy) = (w, h);
    let frame = if spec.floating {
        format!(
            r#"<wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="{}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapSquare wrapText="bothSides"/>"#,
            251658240 + id as u64,
            emu(px),
            emu(py)
        )
    } else {
        format!(r#"<wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>"#)
    };
    let close = if spec.floating {
        r#"<wp14:sizeRelH relativeFrom="margin"><wp14:pctWidth>0</wp14:pctWidth></wp14:sizeRelH><wp14:sizeRelV relativeFrom="margin"><wp14:pctHeight>0</wp14:pctHeight></wp14:sizeRelV></wp:anchor>"#
    } else {
        "</wp:inline>"
    };
    let style = if spec.floating {
        format!(
            "position:absolute;margin-left:{}pt;margin-top:{}pt;width:{}pt;height:{}pt;z-index:{};visibility:visible;mso-wrap-style:square;mso-wrap-distance-left:9pt;mso-wrap-distance-top:0;mso-wrap-distance-right:9pt;mso-wrap-distance-bottom:0;mso-position-horizontal:absolute;mso-position-horizontal-relative:text;mso-position-vertical:absolute;mso-position-vertical-relative:text;v-text-anchor:middle",
            trim(px),
            trim(py),
            trim(w_pt),
            trim(h_pt),
            251658240 + id as u64
        )
    } else {
        format!("width:{}pt;height:{}pt;visibility:visible;mso-wrap-style:square;v-text-anchor:middle", trim(w_pt), trim(h_pt))
    };
    let wrap = if spec.floating { r#"<w10:wrap type="square"/>"# } else { "" };
    let vml_fill = match &spec.fill {
        Some(h) => format!(r##" fillcolor="#{h}""##),
        None => r#" filled="f""#.into(),
    };
    let vml_stroke = match &spec.line {
        Some(l) => format!(r##" strokecolor="#{}" strokeweight="{}pt""##, l.color, trim(l.width_pt)),
        None => r#" stroked="f""#.into(),
    };
    let vml_children = match &spec.line {
        Some(l) => vml_stroke_children(l),
        None => String::new(),
    };
    let src = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="wps"><w:drawing>{frame}<wp:docPr id="{id}" name="{name}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{NS_WPS}"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>{}{fill}{ln}</wps:spPr><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic>{close}</w:drawing></mc:Choice><mc:Fallback><w:pict><v:shape id="{name}" o:spid="_x0000_s{uid}" style="{style}" coordsize="{},{}" o:spt="100" adj="0,,0" path="{}"{vml_fill}{vml_stroke}>{vml_children}<v:path arrowok="t" o:connecttype="custom"/>{wrap}</v:shape></w:pict></mc:Fallback></mc:AlternateContent>"#,
        geom.xml,
        geom.coord.0,
        geom.coord.1,
        geom.vml,
    );
    Ok((parse_fragment(&src), w_pt, h_pt))
}

// ---------------------------------------------------------------------------
// Changing the outline of a shape
// ---------------------------------------------------------------------------

/// What `plan` works out for a new outline: its markup and how the shape's box changes.
pub struct Plan {
    geom: Geom,
    /// The new box in points.
    pub w: f64,
    pub h: f64,
    /// Where the new box's top-left is, in points from the old box's top-left (before any turn or flip).
    pub shift: P,
    /// Number of points a connector could attach to.
    pub vertices: usize,
}

/// Plan an outline for a shape that is `cur_w` by `cur_h` points. Each path's commands are in its own `w` by `h`
/// space (points of the box when it says 0).
pub fn plan(paths: &[PathIn], cur_w: f64, cur_h: f64) -> Result<Plan, String> {
    if paths.is_empty() {
        return Err("give at least one path".into());
    }
    let mut all = Vec::new();
    for p in paths {
        all.push((segs(&p.cmds)?, p.fill, p.stroke));
    }
    let scale = |i: usize| {
        let p = &paths[i];
        (if p.w > 0.0 { cur_w * EMU_PER_PT / p.w } else { EMU_PER_PT }, if p.h > 0.0 { cur_h * EMU_PER_PT / p.h } else { EMU_PER_PT })
    };
    let (placed, w, h, min) = place(all, (0.0, 0.0), &scale).ok_or("the outline has no points")?;
    let geom = geometry(&placed, w, h);
    let vertices = placed.iter().map(|p| vertices(&p.segs).len()).sum();
    Ok(Plan { geom, w: w as f64 / EMU_PER_PT, h: h as f64 / EMU_PER_PT, shift: (min.0 / EMU_PER_PT, min.1 / EMU_PER_PT), vertices })
}

const SPPR_ORDER: &[&str] = &["a:xfrm", "a:custGeom", "a:prstGeom", "a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill", "a:ln", "a:effectLst", "a:effectDag", "a:scene3d", "a:sp3d", "a:extLst"];
const VML_SHAPES: &[&str] = &["v:rect", "v:roundrect", "v:oval", "v:line", "v:shape"];

/// Make a VML shape draw `path` in a `coord` space: it becomes a plain `v:shape` with its own path.
pub fn vml_set_path(sh: &mut Element, path: &str, coord: (i64, i64)) {
    sh.name = "v:shape".into();
    for a in ["arcsize", "type", "from", "to", "o:connectortype", "o:oned"] {
        sh.remove_attr(a);
    }
    sh.set_attr("coordsize", format!("{},{}", coord.0.max(1), coord.1.max(1)));
    sh.set_attr("o:spt", "100");
    sh.set_attr("path", path);
    if sh.child("v:path").is_none() {
        let at = sh.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "v:textbox" || e.name.starts_with("w10:"))).unwrap_or(sh.children.len());
        sh.children.insert(at, Node::Element(Element::new("v:path").with_attr("arrowok", "t").with_attr("o:connecttype", "custom")));
    }
}

/// Write the planned outline into a shape object (the DrawingML and the VML copy). The box is set separately.
pub fn write(obj: &mut Element, plan: &Plan) -> Result<(), String> {
    let drawing = super::patch::drawing_mut(obj);
    if let Some(drawing) = drawing {
        let wsp = super::patch::find_mut(drawing, "wps:wsp").ok_or("only a single shape has points to edit, not a group")?;
        let pr = wsp.child_mut("wps:spPr").ok_or("the shape has no properties")?;
        pr.remove_children("a:prstGeom");
        pr.remove_children("a:custGeom");
        put_ordered(pr, parse(&plan.geom.xml), SPPR_ORDER);
        // A connector that is turned into a freeform is no longer attached to anything.
        if let Some(cnv) = wsp.child_mut("wps:cNvCnPr") {
            cnv.remove_children("a:stCxn");
            cnv.remove_children("a:endCxn");
        }
    }
    let vml = |root: &mut Element| {
        let picts: Vec<&mut Element> = if root.name == "w:pict" { vec![root] } else { root.elements_mut().filter(|e| e.name == "w:pict").collect() };
        for pict in picts {
            for sh in pict.elements_mut().filter(|e| VML_SHAPES.contains(&e.name.as_str())) {
                vml_set_path(sh, &plan.geom.vml, plan.geom.coord);
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

/// Where the new box's top-left goes, in points, for a shape whose box is at `(x, y)` with size `(w, h)`, turned by
/// `rot` degrees and mirrored as `flip` says: the box's centre moves as the outline's bounds did.
pub fn new_corner(old: (f64, f64, f64, f64), rot: f64, flip: (bool, bool), plan: &Plan) -> P {
    let (x, y, w, h) = old;
    // The bounds as the box shows them (mirrored shapes show the outline reversed).
    let (bx0, bx1) = (plan.shift.0, plan.shift.0 + plan.w);
    let (by0, by1) = (plan.shift.1, plan.shift.1 + plan.h);
    let (lx0, lx1) = if flip.0 { (w - bx1, w - bx0) } else { (bx0, bx1) };
    let (ly0, ly1) = if flip.1 { (h - by1, h - by0) } else { (by0, by1) };
    let (ux, uy) = ((lx0 + lx1) / 2.0 - w / 2.0, (ly0 + ly1) / 2.0 - h / 2.0);
    let (s, c) = rot.to_radians().sin_cos();
    let (cx, cy) = (x + w / 2.0 + ux * c - uy * s, y + h / 2.0 + ux * s + uy * c);
    (cx - plan.w / 2.0, cy - plan.h / 2.0)
}
