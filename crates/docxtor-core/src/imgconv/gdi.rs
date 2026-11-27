//! What EMF and WMF share: a GDI device context (mapping modes, world transform, selected pen,
//! brush and font, clip), the object table, and the SVG that the drawing records turn into. The
//! record parsers live in `emf.rs` and `wmf.rs`; the drawing itself is in `gdi_draw.rs`.

use std::collections::HashMap;
use std::fmt::Write;

/// An affine matrix in SVG order `[a b c d e f]`: x' = a x + c y + e, y' = b x + d y + f.
pub type Mat = [f64; 6];
pub const IDENT: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `a` applied after `b`.
pub fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

pub fn mat_apply(m: &Mat, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

pub fn mat_det(m: &Mat) -> f64 {
    m[0] * m[3] - m[1] * m[2]
}

/// How much the matrix scales lengths, on average.
pub fn mat_scale(m: &Mat) -> f64 {
    mat_det(m).abs().sqrt()
}

/// A number for SVG: at most three decimals, no trailing zeros, never NaN or infinite.
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 {
        return "0".into();
    }
    let mut s = format!("{r:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// XML-escape text for an attribute or element body, dropping characters XML cannot hold.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' || (0xD800..0xE000).contains(&(c as u32)) => {}
            c => out.push(c),
        }
    }
    out
}

/// Windows-1252 text as a string.
pub fn ansi_to_string(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘',
        '’', '“', '”', '•', '\u{2013}', '\u{2014}', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|&b| if (0x80..0xA0).contains(&b) { HIGH[(b - 0x80) as usize] } else { b as char })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// A Windows COLORREF (0x00BBGGRR).
    pub fn from_colorref(c: u32) -> Rgb {
        Rgb(c as u8, (c >> 8) as u8, (c >> 16) as u8)
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// What a pen's dash lengths are measured in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DashUnit {
    /// Device pixels (cosmetic pens).
    Pixels,
    /// Multiples of the pen width.
    Widths,
    /// Logical units (user-defined styles).
    Logical,
}

#[derive(Clone, Debug)]
pub struct Pen {
    pub null: bool,
    pub color: Rgb,
    pub width: f64,
    /// One device pixel wide whatever the mapping.
    pub cosmetic: bool,
    pub dash: Vec<f64>,
    pub dash_unit: DashUnit,
    pub cap: &'static str,
    pub join: &'static str,
}

impl Pen {
    /// A pen from a Windows pen style (PS_*), width and colour. `user_dash` is used for PS_USERSTYLE.
    pub fn from_style(style: u32, width: f64, color: Rgb, user_dash: Vec<f64>) -> Pen {
        let geometric = style & 0x0001_0000 != 0;
        let cosmetic = !geometric && width <= 1.0;
        let (dash, dash_unit): (Vec<f64>, DashUnit) = match (style & 0xF, cosmetic) {
            (1, true) => (vec![18.0, 6.0], DashUnit::Pixels),
            (2, true) => (vec![3.0, 3.0], DashUnit::Pixels),
            (3, true) => (vec![9.0, 3.0, 3.0, 3.0], DashUnit::Pixels),
            (4, true) => (vec![9.0, 3.0, 3.0, 3.0, 3.0, 3.0], DashUnit::Pixels),
            (1, false) => (vec![6.0, 2.0], DashUnit::Widths),
            (2, false) => (vec![1.0, 1.0], DashUnit::Widths),
            (3, false) => (vec![4.0, 2.0, 1.0, 2.0], DashUnit::Widths),
            (4, false) => (vec![4.0, 2.0, 1.0, 2.0, 1.0, 2.0], DashUnit::Widths),
            (7, _) if !user_dash.is_empty() => (user_dash, DashUnit::Logical),
            _ => (vec![], DashUnit::Logical),
        };
        let (cap, join) = if geometric {
            (
                match style & 0xF00 {
                    0x100 => "square",
                    0x200 => "butt",
                    _ => "round",
                },
                match style & 0xF000 {
                    0x1000 => "bevel",
                    0x2000 => "miter",
                    _ => "round",
                },
            )
        } else {
            ("round", "round")
        };
        Pen { null: style & 0xF == 5, color, width, cosmetic, dash, dash_unit, cap, join }
    }
}

#[derive(Clone, Debug)]
pub enum Brush {
    Solid(Rgb),
    Hollow,
    /// Hatched fills are drawn as a light wash of the hatch colour.
    Hatch(Rgb),
}

#[derive(Clone, Debug)]
pub struct Font {
    pub face: String,
    pub generic: &'static str,
    /// The em size in logical units; 0 means the device default.
    pub size: f64,
    pub weight: u32,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// Tenths of a degree, counterclockwise.
    pub escapement: f64,
}

impl Font {
    /// A font from the fields of a LOGFONT.
    pub fn from_logfont(
        height: i32,
        escapement: i32,
        weight: u32,
        flags: (bool, bool, bool),
        pitch_family: u8,
        face: &str,
    ) -> Font {
        let face = face.trim().to_string();
        let lower = face.to_lowercase();
        let by_name = if ["courier", "consolas", "mono", "lucida console", "terminal"].iter().any(|k| lower.contains(k)) {
            "monospace"
        } else if ["times", "georgia", "garamond", "cambria", "palatino", "book", "minion"].iter().any(|k| lower.contains(k))
            || (lower.contains("serif") && !lower.contains("sans"))
        {
            "serif"
        } else {
            "sans-serif"
        };
        let generic = match pitch_family & 0xF0 {
            0x10 => "serif",
            0x20 => "sans-serif",
            0x30 => "monospace",
            0x40 => "cursive",
            _ => by_name,
        };
        Font {
            face: if face.is_empty() { "Arial".into() } else { face },
            generic,
            // A positive height is the whole cell (ascent and descent); the em is a little smaller.
            size: if height < 0 { -(height as f64) } else { height as f64 * 0.87 },
            weight: if weight == 0 { 400 } else { weight },
            italic: flags.0,
            underline: flags.1,
            strike: flags.2,
            escapement: escapement as f64,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Object {
    Pen(Pen),
    Brush(Brush),
    Font(Font),
    /// Palettes, regions and the rest: they take a slot in the table but draw nothing.
    Other,
}

/// A stock object by its GetStockObject index.
pub fn stock_object(index: u32) -> Option<Object> {
    let grey = |v: u8| Object::Brush(Brush::Solid(Rgb(v, v, v)));
    Some(match index {
        0 => grey(255),
        1 => grey(192),
        2 => grey(128),
        3 => grey(64),
        4 => grey(0),
        5 => Object::Brush(Brush::Hollow),
        6 => Object::Pen(Pen::from_style(0, 1.0, Rgb(255, 255, 255), vec![])),
        7 => Object::Pen(Pen::from_style(0, 1.0, Rgb(0, 0, 0), vec![])),
        8 => Object::Pen(Pen::from_style(5, 1.0, Rgb(0, 0, 0), vec![])),
        10 | 11 | 16 => Object::Font(Font::from_logfont(0, 0, 400, (false, false, false), 0x30, "Courier New")),
        12..=14 | 17 => Object::Font(Font::from_logfont(0, 0, 400, (false, false, false), 0x20, "Arial")),
        15 => Object::Other,
        18 => grey(255),
        19 => Object::Pen(Pen::from_style(0, 1.0, Rgb(0, 0, 0), vec![])),
        _ => return None,
    })
}

/// The state SaveDC and RestoreDC keep.
#[derive(Clone, Debug)]
pub struct Dc {
    pub pen: Pen,
    pub brush: Brush,
    pub font: Font,
    pub text_color: Rgb,
    pub bk_color: Rgb,
    /// 1 transparent, 2 opaque.
    pub bk_mode: u32,
    pub text_align: u32,
    /// 1 alternate (even-odd), 2 winding.
    pub poly_fill: u32,
    pub map_mode: u32,
    pub win_org: (f64, f64),
    pub win_ext: (f64, f64),
    pub vp_org: (f64, f64),
    pub vp_ext: (f64, f64),
    pub world: Mat,
    /// Current position, in logical coordinates.
    pub pos: (f64, f64),
    /// The active clip (0 for none), an index into the clip paths written so far.
    pub clip: usize,
    pub arc_cw: bool,
}

impl Dc {
    fn new() -> Dc {
        let pen = match stock_object(7) {
            Some(Object::Pen(p)) => p,
            _ => Pen::from_style(0, 1.0, Rgb(0, 0, 0), vec![]),
        };
        let font = match stock_object(13) {
            Some(Object::Font(f)) => f,
            _ => Font::from_logfont(0, 0, 400, (false, false, false), 0x20, "Arial"),
        };
        Dc {
            pen,
            brush: Brush::Solid(Rgb(255, 255, 255)),
            font,
            text_color: Rgb(0, 0, 0),
            bk_color: Rgb(255, 255, 255),
            bk_mode: 2,
            text_align: 0,
            poly_fill: 1,
            map_mode: 1,
            win_org: (0.0, 0.0),
            win_ext: (1.0, 1.0),
            vp_org: (0.0, 0.0),
            vp_ext: (1.0, 1.0),
            world: IDENT,
            pos: (0.0, 0.0),
            clip: 0,
            arc_cw: false,
        }
    }
}

/// The largest SVG body written before drawing stops.
const MAX_SVG: usize = 256 << 20;

/// The playback state and the SVG written so far.
pub struct Gdi {
    pub dc: Dc,
    stack: Vec<Dc>,
    pub objects: HashMap<u32, Object>,
    /// Freed object indexes (WMF hands out the lowest free one), so creating is not a scan.
    free_objects: std::collections::BTreeSet<u32>,
    /// Device pixels per millimetre, for the fixed mapping modes.
    pub px_per_mm: f64,
    /// Output (CSS) pixels per device unit, so cosmetic pens come out one pixel wide.
    pub px_per_dev: f64,
    /// WMF files set extents freely whatever the mapping mode says.
    pub lenient_ext: bool,
    /// The open path bracket, as SVG path data in logical coordinates.
    pub path: Option<String>,
    /// Whether the open path has a current point to continue from.
    pub path_cur: bool,
    /// The last finished path, waiting for a fill, stroke or clip.
    pub saved_path: Option<String>,
    pub(crate) defs: String,
    pub(crate) body: String,
    pub(crate) clip_seq: usize,
    pub(crate) cur_clip: usize,
    pub(crate) cur_mat: Option<Mat>,
    /// Device-space extent of everything drawn.
    pub min: (f64, f64),
    pub max: (f64, f64),
}

/// The most objects a metafile may hold at once (WMF indexes are 16-bit).
pub const MAX_OBJECTS: usize = 65_535;

impl Gdi {
    /// Add an object at the lowest free index, as WMF numbers them; creates past the cap are ignored.
    pub fn add_object(&mut self, obj: Object) {
        if self.objects.len() >= MAX_OBJECTS {
            return;
        }
        let i = self.free_objects.pop_first().unwrap_or((self.objects.len() + self.free_objects.len()) as u32);
        self.objects.insert(i, obj);
    }

    /// Delete an object by index, freeing its slot.
    pub fn delete_object(&mut self, i: u32) {
        if self.objects.remove(&i).is_some() {
            self.free_objects.insert(i);
        }
    }

    pub fn new(px_per_mm: f64, px_per_dev: f64) -> Gdi {
        Gdi {
            dc: Dc::new(),
            stack: Vec::new(),
            objects: HashMap::new(),
            free_objects: std::collections::BTreeSet::new(),
            px_per_mm: if px_per_mm.is_finite() && px_per_mm > 0.0 { px_per_mm } else { 96.0 / 25.4 },
            px_per_dev: if px_per_dev.is_finite() && px_per_dev > 0.0 { px_per_dev } else { 1.0 },
            lenient_ext: false,
            path: None,
            path_cur: false,
            saved_path: None,
            defs: String::new(),
            body: String::new(),
            clip_seq: 0,
            cur_clip: 0,
            cur_mat: None,
            min: (f64::MAX, f64::MAX),
            max: (f64::MIN, f64::MIN),
        }
    }

    /// Window-to-viewport mapping as a matrix.
    fn window_to_viewport(&self) -> Mat {
        let d = &self.dc;
        let sx = if d.win_ext.0 != 0.0 { d.vp_ext.0 / d.win_ext.0 } else { 1.0 };
        let sy = if d.win_ext.1 != 0.0 { d.vp_ext.1 / d.win_ext.1 } else { 1.0 };
        [sx, 0.0, 0.0, sy, d.vp_org.0 - d.win_org.0 * sx, d.vp_org.1 - d.win_org.1 * sy]
    }

    /// Logical coordinates to device coordinates: the world transform, then the mapping mode.
    pub fn matrix(&self) -> Mat {
        mat_mul(&self.window_to_viewport(), &self.dc.world)
    }

    fn ext_is_fixed(&self) -> bool {
        !self.lenient_ext && (1..=6).contains(&self.dc.map_mode)
    }

    pub fn set_map_mode(&mut self, mode: u32) {
        self.dc.map_mode = mode;
        if self.lenient_ext {
            return;
        }
        let mm = self.px_per_mm;
        let unit = match mode {
            1 => Some(1.0),
            2 => Some(mm * 0.1),
            3 => Some(mm * 0.01),
            4 => Some(mm * 0.254),
            5 => Some(mm * 0.0254),
            6 => Some(mm * 25.4 / 1440.0),
            _ => None,
        };
        if let Some(u) = unit {
            self.dc.win_ext = (1.0, 1.0);
            self.dc.vp_ext = (u, if mode == 1 { u } else { -u });
        }
    }

    /// MM_ISOTROPIC keeps a square aspect: the longer viewport side shrinks to fit.
    fn iso_fix(&mut self) {
        if self.lenient_ext || self.dc.map_mode != 7 {
            return;
        }
        let (we, ve) = (self.dc.win_ext, self.dc.vp_ext);
        if we.0 == 0.0 || we.1 == 0.0 || ve.0 == 0.0 || ve.1 == 0.0 {
            return;
        }
        let (xd, yd) = ((ve.0 / we.0).abs(), (ve.1 / we.1).abs());
        if xd > yd {
            self.dc.vp_ext.0 = ve.0.signum() * we.0.abs() * yd;
        } else if yd > xd {
            self.dc.vp_ext.1 = ve.1.signum() * we.1.abs() * xd;
        }
    }

    pub fn set_window_ext(&mut self, x: f64, y: f64) {
        if !self.ext_is_fixed() && x != 0.0 && y != 0.0 {
            self.dc.win_ext = (x, y);
            self.iso_fix();
        }
    }

    pub fn set_viewport_ext(&mut self, x: f64, y: f64) {
        if !self.ext_is_fixed() && x != 0.0 && y != 0.0 {
            self.dc.vp_ext = (x, y);
            self.iso_fix();
        }
    }

    pub fn set_window_org(&mut self, x: f64, y: f64) {
        self.dc.win_org = (x, y);
    }

    pub fn set_viewport_org(&mut self, x: f64, y: f64) {
        self.dc.vp_org = (x, y);
    }

    pub fn scale_window_ext(&mut self, xn: f64, xd: f64, yn: f64, yd: f64) {
        if xd != 0.0 && yd != 0.0 {
            let e = self.dc.win_ext;
            self.set_window_ext(e.0 * xn / xd, e.1 * yn / yd);
        }
    }

    pub fn scale_viewport_ext(&mut self, xn: f64, xd: f64, yn: f64, yd: f64) {
        if xd != 0.0 && yd != 0.0 {
            let e = self.dc.vp_ext;
            self.set_viewport_ext(e.0 * xn / xd, e.1 * yn / yd);
        }
    }

    /// SetWorldTransform from the six XFORM floats (eM11 eM12 eM21 eM22 eDx eDy).
    pub fn set_world(&mut self, x: [f64; 6]) {
        self.dc.world = x;
    }

    /// ModifyWorldTransform: 1 identity, 2 left multiply (apply `x` first), 3 right multiply, 4 set.
    pub fn modify_world(&mut self, x: [f64; 6], mode: u32) {
        let cur = self.dc.world;
        self.dc.world = match mode {
            1 => IDENT,
            2 => mat_mul(&cur, &x),
            3 => mat_mul(&x, &cur),
            4 => x,
            _ => cur,
        };
    }

    pub fn save_dc(&mut self) {
        if self.stack.len() < 10_000 {
            self.stack.push(self.dc.clone());
        }
    }

    /// RestoreDC: negative counts back from the last save, positive is an absolute save level.
    pub fn restore_dc(&mut self, n: i32) {
        let len = self.stack.len();
        let target = match n {
            0 => return,
            n if n < 0 => match len.checked_sub(n.unsigned_abs() as usize) {
                Some(t) => t,
                None => return,
            },
            n => (n as usize).saturating_sub(1),
        };
        if target < len {
            self.dc = self.stack[target].clone();
            self.stack.truncate(target);
        }
    }

    pub fn select_object(&mut self, index: u32) {
        let obj = if index & 0x8000_0000 != 0 { stock_object(index & 0x7FFF_FFFF) } else { self.objects.get(&index).cloned() };
        match obj {
            Some(Object::Pen(p)) => self.dc.pen = p,
            Some(Object::Brush(b)) => self.dc.brush = b,
            Some(Object::Font(f)) => self.dc.font = f,
            _ => {}
        }
    }

    pub(crate) fn full(&self) -> bool {
        // The definitions count too: a gradient is written to them before anything is drawn with it.
        self.body.len() + self.defs.len() > MAX_SVG
    }

    /// The finished SVG. `vb` is the view box (x, y, width, height) in device units and `size` the
    /// width and height in pixels.
    pub fn finish(mut self, vb: (f64, f64, f64, f64), size: (f64, f64)) -> String {
        self.close_groups();
        let pos = |v: f64, fallback: f64| if v.is_finite() && v > 0.0 { v } else { fallback };
        let (vw, vh) = (pos(vb.2, 1.0), pos(vb.3, 1.0));
        let (w, h) = (pos(size.0, vw).min(1e6), pos(size.1, vh).min(1e6));
        let mut svg = String::with_capacity(self.body.len() + self.defs.len() + 256);
        let _ = write!(
            svg,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
            num(w),
            num(h),
            num(vb.0),
            num(vb.1),
            num(vw),
            num(vh)
        );
        if !self.defs.is_empty() {
            svg.push_str("<defs>");
            svg.push_str(&self.defs);
            svg.push_str("</defs>");
        }
        svg.push_str(&self.body);
        svg.push_str("</svg>");
        svg
    }

    /// The bounding box of what was drawn, if anything was.
    pub fn drawn_extent(&self) -> Option<(f64, f64, f64, f64)> {
        (self.min.0 <= self.max.0 && self.min.1 <= self.max.1)
            .then_some((self.min.0, self.min.1, self.max.0 - self.min.0, self.max.1 - self.min.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.0004), "0");
        assert_eq!(num(1.23456), "1.235");
        assert_eq!(num(f64::NAN), "0");
        assert_eq!(num(-2.5), "-2.5");
        assert_eq!(num(100.0), "100");
    }

    #[test]
    fn escaping_drops_what_xml_cannot_hold() {
        assert_eq!(esc("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
        assert_eq!(esc("x\u{0}y\u{1}z"), "xyz");
        assert_eq!(ansi_to_string(&[0x41, 0x80, 0xE9, 0x93]), "A€é“");
    }

    #[test]
    fn matrices_compose_right_to_left() {
        let scale = [2.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        let shift = [1.0, 0.0, 0.0, 1.0, 10.0, 0.0];
        // shift applied after scale.
        assert_eq!(mat_apply(&mat_mul(&shift, &scale), 1.0, 1.0), (12.0, 2.0));
        assert_eq!(mat_apply(&mat_mul(&scale, &shift), 1.0, 1.0), (22.0, 2.0));
        assert_eq!(mat_scale(&scale), 2.0);
    }

    #[test]
    fn map_modes_set_extents() {
        let mut g = Gdi::new(4.0, 1.0);
        g.set_map_mode(2);
        let m = g.matrix();
        assert!((m[0] - 0.4).abs() < 1e-9 && (m[3] + 0.4).abs() < 1e-9);
        g.set_window_ext(5.0, 5.0); // ignored in a fixed mode
        assert_eq!(g.dc.win_ext, (1.0, 1.0));
        g.set_map_mode(8);
        g.set_window_ext(100.0, 50.0);
        g.set_viewport_ext(200.0, 200.0);
        g.set_window_org(10.0, 10.0);
        let m = g.matrix();
        assert_eq!(mat_apply(&m, 10.0, 10.0), (0.0, 0.0));
        assert_eq!(mat_apply(&m, 110.0, 60.0), (200.0, 200.0));
    }

    #[test]
    fn isotropic_mode_keeps_a_square_aspect() {
        let mut g = Gdi::new(4.0, 1.0);
        g.set_map_mode(7);
        g.set_window_ext(100.0, 100.0);
        g.set_viewport_ext(400.0, 200.0);
        let m = g.matrix();
        assert_eq!((m[0], m[3]), (2.0, 2.0));
    }

    #[test]
    fn world_transform_modes() {
        let mut g = Gdi::new(1.0, 1.0);
        g.set_world([1.0, 0.0, 0.0, 1.0, 5.0, 0.0]);
        // Left multiply: the new transform is applied to points first.
        g.modify_world([2.0, 0.0, 0.0, 2.0, 0.0, 0.0], 2);
        assert_eq!(mat_apply(&g.matrix(), 1.0, 1.0), (7.0, 2.0));
        g.set_world([1.0, 0.0, 0.0, 1.0, 5.0, 0.0]);
        g.modify_world([2.0, 0.0, 0.0, 2.0, 0.0, 0.0], 3);
        assert_eq!(mat_apply(&g.matrix(), 1.0, 1.0), (12.0, 2.0));
        g.modify_world([0.0; 6], 1);
        assert_eq!(g.matrix(), IDENT);
    }

    #[test]
    fn save_and_restore_levels() {
        let mut g = Gdi::new(1.0, 1.0);
        g.dc.text_color = Rgb(1, 1, 1);
        g.save_dc();
        g.dc.text_color = Rgb(2, 2, 2);
        g.save_dc();
        g.dc.text_color = Rgb(3, 3, 3);
        g.restore_dc(-1);
        assert_eq!(g.dc.text_color, Rgb(2, 2, 2));
        g.restore_dc(1);
        assert_eq!(g.dc.text_color, Rgb(1, 1, 1));
        g.restore_dc(-5); // too far: ignored
        g.restore_dc(0);
        assert_eq!(g.dc.text_color, Rgb(1, 1, 1));
    }

    #[test]
    fn pens_and_fonts_from_styles() {
        let dash = Pen::from_style(1, 1.0, Rgb(0, 0, 0), vec![]);
        assert!(dash.cosmetic && dash.dash_unit == DashUnit::Pixels);
        let wide = Pen::from_style(0x0001_0000 | 0x200 | 0x2000, 4.0, Rgb(0, 0, 0), vec![]);
        assert_eq!((wide.cap, wide.join, wide.cosmetic), ("butt", "miter", false));
        assert!(Pen::from_style(5, 1.0, Rgb(0, 0, 0), vec![]).null);
        let f = Font::from_logfont(-16, 900, 700, (true, false, false), 0x10, "");
        assert_eq!((f.size, f.generic, f.face.as_str(), f.weight), (16.0, "serif", "Arial", 700));
        assert_eq!(Font::from_logfont(0, 0, 0, (false, false, false), 0, "Courier New").generic, "monospace");
    }
}
