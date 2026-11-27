//! Drawing for `Gdi`: shapes, paths, clipping, text and bitmaps become SVG elements. Shapes are
//! written in logical coordinates inside a group carrying the logical-to-device matrix, so pen
//! widths scale with the mapping; text is placed in device space so it stays upright.

use super::dib::{data_uri, Picture};
use super::gdi::*;
use std::f64::consts::{PI, TAU};
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq)]
pub enum PolyKind {
    Polygon,
    Polyline,
    PolylineTo,
    Bezier,
    BezierTo,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ArcKind {
    Arc,
    ArcTo,
    Chord,
    Pie,
}

/// A bitmap to place: `src` is the part of the bitmap to use as (x, y, width, height) in pixels
/// counted from the bottom row for bottom-up bitmaps, and `dest` the destination rectangle in
/// logical units. A negative destination width or height mirrors the picture.
pub struct ImageDraw {
    pub pic: Picture,
    pub src: Option<[usize; 4]>,
    pub dest: [f64; 4],
    pub blend: Option<&'static str>,
}

pub struct TextRun<'a> {
    pub x: f64,
    pub y: f64,
    pub text: &'a str,
    /// Advance widths per character in logical units, when the file gives them.
    pub dx: Option<&'a [f64]>,
    /// Fill this rectangle (l, t, r, b) with the background colour first.
    pub opaque: Option<[f64; 4]>,
}

const TA_UPDATECP: u32 = 1;

fn pt(p: (f64, f64)) -> String {
    format!("{} {}", num(p.0), num(p.1))
}

/// (left, top, right, bottom) put in order.
fn norm(b: [f64; 4]) -> [f64; 4] {
    [b[0].min(b[2]), b[1].min(b[3]), b[0].max(b[2]), b[1].max(b[3])]
}

fn rect_path(b: [f64; 4]) -> String {
    let b = norm(b);
    format!("M{} L{} L{} L{} Z", pt((b[0], b[1])), pt((b[2], b[1])), pt((b[2], b[3])), pt((b[0], b[3])))
}

impl Gdi {
    fn grow(&mut self, x: f64, y: f64) {
        let (dx, dy) = mat_apply(&self.matrix(), x, y);
        if dx.is_finite() && dy.is_finite() {
            self.min = (self.min.0.min(dx), self.min.1.min(dy));
            self.max = (self.max.0.max(dx), self.max.1.max(dy));
        }
    }

    fn grow_box(&mut self, b: [f64; 4]) {
        self.grow(b[0], b[1]);
        self.grow(b[2], b[3]);
        self.grow(b[0], b[3]);
        self.grow(b[2], b[1]);
    }

    pub(crate) fn close_groups(&mut self) {
        self.close_mat();
        if self.cur_clip != 0 {
            self.body.push_str("</g>");
            self.cur_clip = 0;
        }
    }

    fn close_mat(&mut self) {
        if let Some(m) = self.cur_mat.take() {
            if m != IDENT {
                self.body.push_str("</g>");
            }
        }
    }

    /// Append an element, opening or closing the clip and matrix groups it needs.
    pub(crate) fn emit(&mut self, mat: Mat, frag: &str) {
        if self.full() {
            return;
        }
        let clip = self.dc.clip;
        if clip != self.cur_clip {
            self.close_mat();
            if self.cur_clip != 0 {
                self.body.push_str("</g>");
            }
            if clip != 0 {
                let _ = write!(self.body, "<g clip-path=\"url(#c{clip})\">");
            }
            self.cur_clip = clip;
        }
        if self.cur_mat != Some(mat) {
            self.close_mat();
            if mat != IDENT {
                let _ = write!(
                    self.body,
                    "<g transform=\"matrix({} {} {} {} {} {})\">",
                    num(mat[0]),
                    num(mat[1]),
                    num(mat[2]),
                    num(mat[3]),
                    num(mat[4]),
                    num(mat[5])
                );
            }
            self.cur_mat = Some(mat);
        }
        self.body.push_str(frag);
    }

    /// Fill and stroke attributes for the selected brush and pen, or `None` when nothing would show.
    fn paint(&self, fill: bool, stroke: bool, m: &Mat) -> Option<String> {
        let mut s = String::new();
        let mut visible = false;
        match (&self.dc.brush, fill) {
            (Brush::Solid(c), true) => {
                let _ = write!(s, " fill=\"{}\"", c.hex());
                visible = true;
            }
            (Brush::Hatch(c), true) => {
                let _ = write!(s, " fill=\"{}\" fill-opacity=\"0.4\"", c.hex());
                visible = true;
            }
            _ => s.push_str(" fill=\"none\""),
        }
        if fill && self.dc.poly_fill == 1 && !matches!(self.dc.brush, Brush::Hollow) {
            s.push_str(" fill-rule=\"evenodd\"");
        }
        let pen = &self.dc.pen;
        if stroke && !pen.null {
            visible = true;
            let px_per_unit = (mat_scale(m) * self.px_per_dev).max(1e-9);
            let width = if pen.cosmetic || pen.width <= 0.0 { 1.0 / px_per_unit } else { pen.width };
            let _ = write!(s, " stroke=\"{}\" stroke-width=\"{}\"", pen.color.hex(), num(width));
            if pen.cap != "butt" {
                let _ = write!(s, " stroke-linecap=\"{}\"", pen.cap);
            }
            if pen.join != "miter" {
                let _ = write!(s, " stroke-linejoin=\"{}\"", pen.join);
            } else {
                s.push_str(" stroke-miterlimit=\"10\"");
            }
            if !pen.dash.is_empty() {
                let unit = match pen.dash_unit {
                    DashUnit::Pixels => 1.0 / px_per_unit,
                    DashUnit::Widths => width,
                    DashUnit::Logical => 1.0,
                };
                let list: Vec<String> = pen.dash.iter().map(|d| num((d * unit).max(1e-6))).collect();
                let _ = write!(s, " stroke-dasharray=\"{}\"", list.join(" "));
            }
        }
        visible.then_some(s)
    }

    /// Draw path data, or add it to the open path bracket.
    pub(crate) fn shape(&mut self, d: String, fill: bool, stroke: bool) {
        if let Some(p) = self.path.as_mut() {
            p.push_str(&d);
            p.push(' ');
            return;
        }
        let m = self.matrix();
        if let Some(attrs) = self.paint(fill, stroke, &m) {
            self.emit(m, &format!("<path d=\"{d}\"{attrs}/>"));
        }
    }

    // Lines and polygons.

    pub fn move_to(&mut self, x: f64, y: f64) {
        self.dc.pos = (x, y);
        if let Some(p) = self.path.as_mut() {
            let _ = write!(p, "M{} ", pt((x, y)));
            self.path_cur = true;
        }
    }

    /// `M cur` unless an open path already continues from the current point.
    fn start_from_cur(&self) -> String {
        if self.path.is_some() && self.path_cur { String::new() } else { format!("M{} ", pt(self.dc.pos)) }
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        let from = self.dc.pos;
        let d = format!("{}L{}", self.start_from_cur(), pt((x, y)));
        self.grow(from.0, from.1);
        self.grow(x, y);
        self.shape(d, false, true);
        self.dc.pos = (x, y);
        if self.path.is_some() {
            self.path_cur = true;
        }
    }

    pub fn poly(&mut self, kind: PolyKind, pts: &[(f64, f64)]) {
        let Some(&first) = pts.first() else { return };
        for &(x, y) in pts {
            self.grow(x, y);
        }
        match kind {
            PolyKind::Polygon | PolyKind::Polyline => {
                let mut d = format!("M{}", pt(first));
                for &p in &pts[1..] {
                    let _ = write!(d, " L{}", pt(p));
                }
                if kind == PolyKind::Polygon {
                    d.push_str(" Z");
                }
                self.shape(d, kind == PolyKind::Polygon, true);
                self.path_cur = false;
            }
            PolyKind::PolylineTo => {
                let mut d = self.start_from_cur();
                for &p in pts {
                    let _ = write!(d, "L{} ", pt(p));
                }
                self.grow(self.dc.pos.0, self.dc.pos.1);
                self.shape(d, false, true);
                self.dc.pos = pts[pts.len() - 1];
                self.path_cur = self.path.is_some();
            }
            PolyKind::Bezier => {
                let mut d = format!("M{}", pt(first));
                for c in pts[1..].chunks_exact(3) {
                    let _ = write!(d, " C{} {} {}", pt(c[0]), pt(c[1]), pt(c[2]));
                }
                self.shape(d, false, true);
                self.path_cur = false;
            }
            PolyKind::BezierTo => {
                let used = pts.len() / 3 * 3;
                if used == 0 {
                    return;
                }
                let mut d = self.start_from_cur();
                for c in pts[..used].chunks_exact(3) {
                    let _ = write!(d, "C{} {} {} ", pt(c[0]), pt(c[1]), pt(c[2]));
                }
                self.grow(self.dc.pos.0, self.dc.pos.1);
                self.shape(d, false, true);
                self.dc.pos = pts[used - 1];
                self.path_cur = self.path.is_some();
            }
        }
    }

    /// Several polygons (or polylines) drawn as one shape, so the fill rule spans them all.
    pub fn poly_poly(&mut self, polys: &[Vec<(f64, f64)>], closed: bool) {
        let mut d = String::new();
        for poly in polys.iter().filter(|p| p.len() >= 2) {
            for &(x, y) in poly {
                self.grow(x, y);
            }
            let _ = write!(d, "M{}", pt(poly[0]));
            for &p in &poly[1..] {
                let _ = write!(d, " L{}", pt(p));
            }
            d.push_str(if closed { " Z " } else { " " });
        }
        if !d.is_empty() {
            self.shape(d, closed, true);
            self.path_cur = false;
        }
    }

    /// POLYDRAW: points with PT_MOVETO (6), PT_LINETO (2), PT_BEZIERTO (4) and PT_CLOSEFIGURE (1) flags.
    pub fn poly_draw(&mut self, pts: &[(f64, f64)], types: &[u8]) {
        let mut d = String::new();
        let mut i = 0;
        while i < pts.len().min(types.len()) {
            let t = types[i];
            match t & !1 {
                6 => {
                    let _ = write!(d, "M{} ", pt(pts[i]));
                    self.dc.pos = pts[i];
                }
                2 => {
                    if d.is_empty() {
                        let _ = write!(d, "M{} ", pt(self.dc.pos));
                    }
                    let _ = write!(d, "L{} ", pt(pts[i]));
                    self.dc.pos = pts[i];
                }
                4 if i + 2 < pts.len().min(types.len()) => {
                    if d.is_empty() {
                        let _ = write!(d, "M{} ", pt(self.dc.pos));
                    }
                    let _ = write!(d, "C{} {} {} ", pt(pts[i]), pt(pts[i + 1]), pt(pts[i + 2]));
                    self.dc.pos = pts[i + 2];
                    i += 2;
                }
                _ => {}
            }
            if t & 1 != 0 {
                d.push_str("Z ");
            }
            self.grow(pts[i].0, pts[i].1);
            i += 1;
        }
        if !d.is_empty() {
            self.shape(d, false, true);
            self.path_cur = self.path.is_some();
        }
    }

    // Boxes and arcs.

    pub fn rect(&mut self, b: [f64; 4]) {
        self.grow_box(b);
        self.shape(rect_path(b), true, true);
        self.path_cur = false;
    }

    pub fn ellipse(&mut self, b: [f64; 4]) {
        let [l, t, r, bt] = norm(b);
        let (rx, ry) = ((r - l) / 2.0, (bt - t) / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        self.grow_box(b);
        let cy = (t + bt) / 2.0;
        let d = format!(
            "M{} A{} {} 0 1 1 {} A{} {} 0 1 1 {} Z",
            pt((l, cy)),
            num(rx),
            num(ry),
            pt((r, cy)),
            num(rx),
            num(ry),
            pt((l, cy))
        );
        self.shape(d, true, true);
        self.path_cur = false;
    }

    /// A rectangle with corners rounded by an ellipse `cw` wide and `ch` high.
    pub fn round_rect(&mut self, b: [f64; 4], cw: f64, ch: f64) {
        let [l, t, r, bt] = norm(b);
        let rx = (cw.abs() / 2.0).min((r - l) / 2.0);
        let ry = (ch.abs() / 2.0).min((bt - t) / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return self.rect(b);
        }
        self.grow_box(b);
        let (a, c) = (num(rx), num(ry));
        let d = format!(
            "M{} L{} A{a} {c} 0 0 1 {} L{} A{a} {c} 0 0 1 {} L{} A{a} {c} 0 0 1 {} L{} A{a} {c} 0 0 1 {} Z",
            pt((l + rx, t)),
            pt((r - rx, t)),
            pt((r, t + ry)),
            pt((r, bt - ry)),
            pt((r - rx, bt)),
            pt((l + rx, bt)),
            pt((l, bt - ry)),
            pt((l, t + ry)),
            pt((l + rx, t))
        );
        self.shape(d, true, true);
        self.path_cur = false;
    }

    /// Whether increasing the parametric angle of an ellipse in logical space turns visually
    /// counterclockwise (it does when the mapping flips the y axis).
    fn increasing_is_ccw(&self) -> bool {
        mat_det(&self.matrix()) < 0.0
    }

    /// ARC, ARCTO, CHORD and PIE between the radials through `start` and `end`, counterclockwise
    /// (or clockwise after SETARCDIRECTION).
    pub fn arc(&mut self, kind: ArcKind, b: [f64; 4], start: (f64, f64), end: (f64, f64)) {
        let [l, t, r, bt] = norm(b);
        let (cx, cy, rx, ry) = ((l + r) / 2.0, (t + bt) / 2.0, (r - l) / 2.0, (bt - t) / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        self.grow_box(b);
        let angle = |p: (f64, f64)| ((p.1 - cy) / ry).atan2((p.0 - cx) / rx);
        let (a0, a1) = (angle(start), angle(end));
        let sweep_pos = self.increasing_is_ccw() != self.dc.arc_cw;
        let mut delta = if sweep_pos { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
        if delta < 1e-9 {
            delta = TAU;
        }
        let on = |a: f64| (cx + rx * a.cos(), cy + ry * a.sin());
        let (sp, ep) = (on(a0), on(a1));
        let sw = u8::from(sweep_pos);
        let (a, c) = (num(rx), num(ry));
        let arc = if delta >= TAU - 1e-9 {
            let mid = on(a0 + if sweep_pos { PI } else { -PI });
            format!("A{a} {c} 0 0 {sw} {} A{a} {c} 0 0 {sw} {}", pt(mid), pt(ep))
        } else {
            format!("A{a} {c} 0 {} {sw} {}", u8::from(delta > PI), pt(ep))
        };
        let (d, fill) = match kind {
            ArcKind::Arc => (format!("M{} {arc}", pt(sp)), false),
            ArcKind::ArcTo => {
                let from = self.start_from_cur();
                self.grow(self.dc.pos.0, self.dc.pos.1);
                (format!("{from}L{} {arc}", pt(sp)), false)
            }
            ArcKind::Chord => (format!("M{} {arc} Z", pt(sp)), true),
            ArcKind::Pie => (format!("M{} L{} {arc} Z", pt((cx, cy)), pt(sp)), true),
        };
        self.shape(d, fill, true);
        if kind == ArcKind::ArcTo {
            self.dc.pos = ep;
            self.path_cur = self.path.is_some();
        } else {
            self.path_cur = false;
        }
    }

    /// ANGLEARC: a line to the start of an arc of `radius` around (cx, cy), then the arc itself.
    pub fn angle_arc(&mut self, cx: f64, cy: f64, radius: f64, start_deg: f64, sweep_deg: f64) {
        if radius <= 0.0 || !sweep_deg.is_finite() || !start_deg.is_finite() {
            return;
        }
        let up = if mat_det(&self.matrix()) >= 0.0 { 1.0 } else { -1.0 };
        let at = |deg: f64| {
            let a = deg.to_radians();
            (cx + radius * a.cos(), cy - up * radius * a.sin())
        };
        let (sp, ep) = (at(start_deg), at(start_deg + sweep_deg));
        let sw = u8::from(if sweep_deg > 0.0 { up < 0.0 } else { up > 0.0 });
        let r = num(radius);
        let from = self.start_from_cur();
        self.grow(self.dc.pos.0, self.dc.pos.1);
        self.grow_box([cx - radius, cy - radius, cx + radius, cy + radius]);
        let arc = if sweep_deg.abs() >= 360.0 {
            let mid = at(start_deg + 180.0 * sweep_deg.signum());
            format!("A{r} {r} 0 0 {sw} {} A{r} {r} 0 0 {sw} {}", pt(mid), pt(ep))
        } else {
            format!("A{r} {r} 0 {} {sw} {}", u8::from(sweep_deg.abs() > 180.0), pt(ep))
        };
        self.shape(format!("{from}L{} {arc}", pt(sp)), false, true);
        self.dc.pos = ep;
        self.path_cur = self.path.is_some();
    }

    // Path brackets.

    pub fn begin_path(&mut self) {
        self.path = Some(String::new());
        self.path_cur = false;
        self.saved_path = None;
    }

    pub fn end_path(&mut self) {
        if let Some(p) = self.path.take() {
            self.saved_path = Some(p);
        }
        self.path_cur = false;
    }

    pub fn abort_path(&mut self) {
        self.path = None;
        self.saved_path = None;
    }

    pub fn close_figure(&mut self) {
        if let Some(p) = self.path.as_mut() {
            p.push_str("Z ");
            self.path_cur = false;
        }
    }

    pub fn fill_path(&mut self, fill: bool, stroke: bool) {
        if let Some(d) = self.saved_path.take() {
            let m = self.matrix();
            if let Some(attrs) = self.paint(fill, stroke, &m) {
                self.emit(m, &format!("<path d=\"{}\"{attrs}/>", d.trim_end()));
            }
        }
    }

    // Clipping.

    fn add_clip(&mut self, d: &str, rule: &str, mode: u32) {
        let prev = match mode {
            5 => 0,
            1 => self.dc.clip,
            _ => return,
        };
        self.clip_seq += 1;
        let id = self.clip_seq;
        let m = self.matrix();
        let link = if prev != 0 { format!(" clip-path=\"url(#c{prev})\"") } else { String::new() };
        let _ = write!(
            self.defs,
            "<clipPath id=\"c{id}\"{link}><path d=\"{}\" clip-rule=\"{rule}\" transform=\"matrix({} {} {} {} {} {})\"/></clipPath>",
            d.trim_end(),
            num(m[0]),
            num(m[1]),
            num(m[2]),
            num(m[3]),
            num(m[4]),
            num(m[5])
        );
        self.dc.clip = id;
    }

    /// Clip to a rectangle: mode 1 intersects with the current clip, 5 replaces it.
    pub fn clip_rect(&mut self, b: [f64; 4], mode: u32) {
        self.add_clip(&rect_path(b), "nonzero", mode);
    }

    /// Clip to a set of rectangles (a region), same modes as `clip_rect`.
    pub fn clip_rects(&mut self, rects: &[[f64; 4]], mode: u32) {
        let d: Vec<String> = rects.iter().map(|r| rect_path(*r)).collect();
        self.add_clip(&d.join(" "), "nonzero", mode);
    }

    /// Remove any clip.
    pub fn clear_clip(&mut self) {
        self.dc.clip = 0;
    }

    /// SELECTCLIPPATH with the finished path.
    pub fn clip_to_path(&mut self, mode: u32) {
        if let Some(d) = self.saved_path.take() {
            let rule = if self.dc.poly_fill == 2 { "nonzero" } else { "evenodd" };
            self.add_clip(&d, rule, mode);
        }
    }

    // Text.

    pub fn text(&mut self, t: &TextRun) {
        if t.text.is_empty() || self.full() {
            return;
        }
        let m = self.matrix();
        let align = self.dc.text_align;
        let (x, y) = if align & TA_UPDATECP != 0 { self.dc.pos } else { (t.x, t.y) };
        if let Some(b) = t.opaque {
            let bk = self.dc.bk_color.hex();
            self.grow_box(b);
            self.emit(m, &format!("<path d=\"{}\" fill=\"{bk}\"/>", rect_path(b)));
        }
        let font = self.dc.font.clone();
        let scale_y = (m[2] * m[2] + m[3] * m[3]).sqrt();
        let scale_x = (m[0] * m[0] + m[1] * m[1]).sqrt();
        let em = if font.size > 0.0 { font.size * scale_y } else { 16.0 / self.px_per_dev };
        if em <= 0.0 || !em.is_finite() {
            return;
        }
        let (px, py) = mat_apply(&m, x, y);
        self.min = (self.min.0.min(px), self.min.1.min(py));
        self.max = (self.max.0.max(px), self.max.1.max(py));
        // Direction of the baseline on the page, from the font's escapement.
        let e = (font.escapement / 10.0).to_radians();
        let up = if mat_det(&m) >= 0.0 { 1.0 } else { -1.0 };
        let (vx, vy) = (e.cos(), -up * e.sin());
        let (dx, dy) = (m[0] * vx + m[2] * vy, m[1] * vx + m[3] * vy);
        let angle = dy.atan2(dx).to_degrees();
        let anchor = match align & 6 {
            2 => "end",
            6 => "middle",
            _ => "start",
        };
        let base = match align & 24 {
            0 => 0.9 * em,
            8 => -0.2 * em,
            _ => 0.0,
        };
        let mut attrs = format!(
            " y=\"{}\" font-family=\"'{}', {}\" font-size=\"{}\" fill=\"{}\"",
            num(base),
            esc(&font.face.replace(['\'', '"'], "")),
            font.generic,
            num(em),
            self.dc.text_color.hex()
        );
        if anchor != "start" {
            let _ = write!(attrs, " text-anchor=\"{anchor}\"");
        }
        if font.weight >= 600 {
            attrs.push_str(" font-weight=\"bold\"");
        }
        if font.italic {
            attrs.push_str(" font-style=\"italic\"");
        }
        match (font.underline, font.strike) {
            (true, true) => attrs.push_str(" text-decoration=\"underline line-through\""),
            (true, false) => attrs.push_str(" text-decoration=\"underline\""),
            (false, true) => attrs.push_str(" text-decoration=\"line-through\""),
            _ => {}
        }
        let count = t.text.chars().count();
        let advance: f64 = t.dx.map_or(0.0, |d| d.iter().take(count).sum());
        if advance > 0.0 {
            let len = advance * scale_x;
            let guess = count as f64 * em * 0.5;
            if len > guess * 0.25 && len < guess * 4.0 {
                let _ = write!(attrs, " textLength=\"{}\" lengthAdjust=\"spacing\"", num(len));
            }
        }
        let rot = if angle.abs() > 1e-6 { format!(" rotate({})", num(angle)) } else { String::new() };
        let frag = format!(
            "<text transform=\"translate({} {}){rot}\"{attrs} xml:space=\"preserve\">{}</text>",
            num(px),
            num(py),
            esc(t.text)
        );
        self.emit(IDENT, &frag);
        if align & TA_UPDATECP != 0 {
            let w = if advance > 0.0 { advance } else { count as f64 * font.size * 0.5 };
            self.dc.pos.0 = x + w;
        }
    }

    // Bitmaps.

    pub fn image(&mut self, im: &ImageDraw) {
        let [x, y, cx, cy] = im.dest;
        let pic = &im.pic;
        if cx == 0.0 || cy == 0.0 || pic.w == 0 || pic.h == 0 || !(cx.is_finite() && cy.is_finite()) {
            return;
        }
        let [sx, sy, sw, sh] = im.src.unwrap_or([0, 0, pic.w, pic.h]);
        let (sx, sw) = (sx.min(pic.w.saturating_sub(1)), sw.max(1));
        let sw = sw.min(pic.w - sx);
        let sh = sh.max(1).min(pic.h);
        // Source rows count from the bottom for bottom-up bitmaps.
        let top = if pic.top_down { sy.min(pic.h - 1) } else { pic.h.saturating_sub(sy + sh) };
        let sh = sh.min(pic.h - top);
        let image = format!(
            "<image width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>",
            pic.w,
            pic.h,
            data_uri(pic)
        );
        let whole = sx == 0 && top == 0 && sw == pic.w && sh == pic.h;
        let inner = if whole {
            image
        } else {
            format!(
                "<svg width=\"{sw}\" height=\"{sh}\" viewBox=\"{sx} {top} {sw} {sh}\" preserveAspectRatio=\"none\" overflow=\"hidden\">{image}</svg>"
            )
        };
        let blend = im.blend.map(|b| format!(" style=\"mix-blend-mode:{b}\"")).unwrap_or_default();
        let frag = format!(
            "<g transform=\"translate({} {}) scale({} {})\"{blend}>{inner}</g>",
            num(x),
            num(y),
            num(cx / sw as f64),
            num(cy / sh as f64)
        );
        self.grow_box([x, y, x + cx, y + cy]);
        let m = self.matrix();
        self.emit(m, &frag);
    }

    /// Fill a rectangle with the selected brush (PATCOPY and its relatives).
    pub fn fill_box(&mut self, b: [f64; 4]) {
        self.grow_box(b);
        let m = self.matrix();
        if let Some(attrs) = self.paint(true, false, &m) {
            self.emit(m, &format!("<path d=\"{}\"{attrs}/>", rect_path(b)));
        }
    }

    /// Fill a rectangle with a gradient from one colour to another, across (`vertical` false) or down.
    pub fn fill_box_gradient(&mut self, b: [f64; 4], from: Rgb, to: Rgb, vertical: bool) {
        if self.full() {
            return;
        }
        self.grow_box(b);
        let m = self.matrix();
        let id = format!("g{}", self.defs.len());
        let (x2, y2) = if vertical { ("0", "1") } else { ("1", "0") };
        self.defs.push_str(&format!(
            "<linearGradient id=\"{id}\" x1=\"0\" y1=\"0\" x2=\"{x2}\" y2=\"{y2}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>",
            from.hex(),
            to.hex()
        ));
        self.emit(m, &format!("<path d=\"{}\" fill=\"url(#{id})\"/>", rect_path(b)));
    }

    /// Fill a triangle with the average of its corners' colours (SVG has no three-colour gradient).
    pub fn fill_triangle(&mut self, pts: [(f64, f64); 3], c: Rgb) {
        let xs = pts.map(|p| p.0);
        let ys = pts.map(|p| p.1);
        self.grow_box([xs.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max), ys.iter().cloned().fold(f64::MIN, f64::max)]);
        let m = self.matrix();
        let d = format!("M{} {}L{} {}L{} {}Z", num(pts[0].0), num(pts[0].1), num(pts[1].0), num(pts[1].1), num(pts[2].0), num(pts[2].1));
        self.emit(m, &format!("<path d=\"{d}\" fill=\"{}\"/>", c.hex()));
    }

    /// Fill a rectangle with a plain colour (BLACKNESS and WHITENESS).
    pub fn fill_box_color(&mut self, b: [f64; 4], c: Rgb) {
        self.grow_box(b);
        let m = self.matrix();
        self.emit(m, &format!("<path d=\"{}\" fill=\"{}\"/>", rect_path(b), c.hex()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imgconv::testutil::{element, numbers};

    fn svg(g: Gdi) -> String {
        g.finish((0.0, 0.0, 100.0, 100.0), (100.0, 100.0))
    }

    #[test]
    fn rectangle_with_pen_and_brush() {
        let mut g = Gdi::new(1.0, 1.0);
        g.dc.pen = Pen::from_style(0, 3.0, Rgb(255, 0, 0), vec![]);
        g.dc.brush = Brush::Solid(Rgb(0, 0, 255));
        g.rect([10.0, 20.0, 60.0, 40.0]);
        let s = svg(g);
        let p = element(&s, "path").unwrap();
        assert!(p.contains("fill=\"#0000ff\""), "{p}");
        assert!(p.contains("stroke=\"#ff0000\" stroke-width=\"3\""), "{p}");
        assert_eq!(numbers(p.split("d=\"").nth(1).unwrap().split('"').next().unwrap())[..4], [10.0, 20.0, 60.0, 20.0]);
    }

    #[test]
    fn cosmetic_pen_is_one_pixel_whatever_the_scale() {
        let mut g = Gdi::new(1.0, 1.0);
        g.set_map_mode(8);
        g.set_window_ext(10.0, 10.0);
        g.set_viewport_ext(100.0, 100.0);
        g.line_to(5.0, 5.0);
        let s = svg(g);
        assert!(s.contains("stroke-width=\"0.1\""), "{s}");
        assert!(s.contains("matrix(10 0 0 10 0 0)"), "{s}");
    }

    #[test]
    fn ellipse_and_arc_directions() {
        let mut g = Gdi::new(1.0, 1.0);
        g.ellipse([0.0, 0.0, 20.0, 10.0]);
        // Counterclockwise from the right (angle 0) to the top (angle -90 in y-down space).
        g.arc(ArcKind::Arc, [0.0, 0.0, 20.0, 10.0], (20.0, 5.0), (10.0, 0.0));
        let s = svg(g);
        assert!(s.contains("A10 5 0 1 1"), "{s}");
        let arc = s.split("<path").nth(2).unwrap();
        // A quarter turn visually counterclockwise in y-down coordinates has sweep flag 0.
        assert!(arc.contains("A10 5 0 0 0 10 0"), "{arc}");
    }

    #[test]
    fn path_bracket_collects_then_fills() {
        let mut g = Gdi::new(1.0, 1.0);
        g.begin_path();
        g.move_to(0.0, 0.0);
        g.line_to(10.0, 0.0);
        g.line_to(10.0, 10.0);
        g.close_figure();
        g.end_path();
        assert!(g.body.is_empty());
        g.fill_path(true, false);
        let s = svg(g);
        assert!(s.contains("d=\"M0 0 L10 0 L10 10 Z\""), "{s}");
    }

    #[test]
    fn clips_nest_and_group_elements() {
        let mut g = Gdi::new(1.0, 1.0);
        g.clip_rect([0.0, 0.0, 50.0, 50.0], 5);
        g.clip_rect([10.0, 10.0, 60.0, 60.0], 1);
        g.rect([0.0, 0.0, 5.0, 5.0]);
        let s = svg(g);
        assert!(s.contains("<clipPath id=\"c2\" clip-path=\"url(#c1)\">"), "{s}");
        assert!(s.contains("<g clip-path=\"url(#c2)\">"), "{s}");
        assert!(s.contains("</g></svg>"), "{s}");
    }

    #[test]
    fn text_is_upright_and_aligned() {
        let mut g = Gdi::new(1.0, 1.0);
        g.set_map_mode(8);
        g.set_window_ext(100.0, -100.0); // y axis up
        g.set_viewport_ext(100.0, 100.0);
        g.dc.font = Font::from_logfont(-12, 900, 700, (true, true, false), 0x20, "Arial");
        g.dc.text_align = 6 | 24;
        g.text(&TextRun { x: 10.0, y: -20.0, text: "A<B", dx: None, opaque: None });
        let s = svg(g);
        let t = element(&s, "text").unwrap();
        assert!(t.contains("translate(10 20) rotate(-90)"), "{t}");
        assert!(t.contains("text-anchor=\"middle\"") && t.contains("font-weight=\"bold\""), "{t}");
        assert!(t.contains("font-style=\"italic\"") && t.contains("text-decoration=\"underline\""), "{t}");
        assert!(t.contains("font-size=\"12\""), "{t}");
        assert!(s.contains(">A&lt;B</text>"));
    }

    #[test]
    fn bitmaps_are_placed_cropped_and_flipped() {
        let dib = crate::imgconv::dib::tests::make_dib(4, 2, 24, &[], &[0; 24]);
        let hd_len = 40;
        let pic = crate::imgconv::dib::picture(&dib[..hd_len], &dib[hd_len..]).unwrap();
        let mut g = Gdi::new(1.0, 1.0);
        g.image(&ImageDraw { pic, src: Some([1, 0, 2, 1]), dest: [10.0, 20.0, 40.0, -10.0], blend: None });
        let s = svg(g);
        assert!(s.contains("translate(10 20) scale(20 -10)"), "{s}");
        // Bottom-up bitmap: row 0 of the source is the bottom row, so the top row to use is 1.
        assert!(s.contains("viewBox=\"1 1 2 1\""), "{s}");
        assert!(s.contains("href=\"data:image/bmp;base64,"));
    }
}
