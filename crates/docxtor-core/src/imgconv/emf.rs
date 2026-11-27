//! EMF (Enhanced Metafile) to SVG. The record stream is played into a `Gdi`, which tracks the
//! device context and writes the SVG; this file knows the record layouts.
//!
//! EMF+ drawing (records inside EMR_COMMENT) is not drawn: a "dual" file also carries plain EMF
//! records for the same picture, and those are used.

use super::dib;
use super::gdi::*;
use super::gdi_draw::*;

const PX_PER_MM_CSS: f64 = 96.0 / 25.4;

/// A bounds-checked view of one record. Reads outside it give zero.
struct Rec<'a>(&'a [u8]);

impl Rec<'_> {
    fn u32(&self, o: usize) -> u32 {
        self.0.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i32(&self, o: usize) -> i32 {
        self.u32(o) as i32
    }
    fn f(&self, o: usize) -> f64 {
        self.i32(o) as f64
    }
    fn f32(&self, o: usize) -> f64 {
        let v = f32::from_bits(self.u32(o)) as f64;
        if v.is_finite() { v } else { 0.0 }
    }
    fn u16(&self, o: usize) -> u16 {
        self.0.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn i16(&self, o: usize) -> i16 {
        self.u16(o) as i16
    }
    fn u8(&self, o: usize) -> u8 {
        self.0.get(o).copied().unwrap_or(0)
    }
    /// `len` bytes at `o`, or nothing when they do not fit.
    fn slice(&self, o: usize, len: usize) -> &[u8] {
        o.checked_add(len).and_then(|end| self.0.get(o..end)).unwrap_or(&[])
    }
    /// A rectangle (left, top, right, bottom) of four 32-bit values.
    fn rect(&self, o: usize) -> [f64; 4] {
        [self.f(o), self.f(o + 4), self.f(o + 8), self.f(o + 12)]
    }
    fn point(&self, o: usize) -> (f64, f64) {
        (self.f(o), self.f(o + 4))
    }
    /// `n` points of 32 or 16 bits per coordinate, as many as fit.
    fn points(&self, o: usize, n: usize, wide: bool) -> Vec<(f64, f64)> {
        let size = if wide { 8 } else { 4 };
        let n = n.min(self.0.len().saturating_sub(o) / size);
        (0..n)
            .map(|i| {
                let at = o + i * size;
                if wide {
                    (self.f(at), self.f(at + 4))
                } else {
                    (self.i16(at) as f64, self.i16(at + 2) as f64)
                }
            })
            .collect()
    }
    fn xform(&self, o: usize) -> [f64; 6] {
        [self.f32(o), self.f32(o + 4), self.f32(o + 8), self.f32(o + 12), self.f32(o + 16), self.f32(o + 20)]
    }
}

struct Header {
    bounds: [i32; 4],
    frame: [i32; 4],
    device: (i32, i32),
    mm: (i32, i32),
}

fn header(b: &[u8]) -> Result<Header, String> {
    let r = Rec(b);
    if r.u32(0) != 1 || b.get(40..44) != Some(b" EMF") || r.u32(4) < 88 {
        return Err("Not an EMF file".into());
    }
    let i = |o| r.i32(o);
    Ok(Header {
        bounds: [i(8), i(12), i(16), i(20)],
        frame: [i(24), i(28), i(32), i(36)],
        device: (i(72), i(76)),
        mm: (i(80), i(84)),
    })
}

fn logfont_face(r: &Rec, o: usize) -> String {
    let units: Vec<u16> = (0..32).map(|i| r.u16(o + i * 2)).take_while(|&c| c != 0).collect();
    String::from_utf16_lossy(&units)
}

fn colorref(r: &Rec, o: usize) -> Rgb {
    Rgb::from_colorref(r.u32(o))
}

/// The picture of a bitmap record, from its bitmap-info and bits offsets and sizes.
fn bitmap(r: &Rec, off_bmi: usize, off_bits: usize) -> Option<dib::Picture> {
    let bmi = r.slice(r.u32(off_bmi) as usize, r.u32(off_bmi + 4) as usize);
    let bits = r.slice(r.u32(off_bits) as usize, r.u32(off_bits + 4) as usize);
    dib::picture(bmi, bits)
}

fn abs_src(x: f64, y: f64, w: f64, h: f64) -> Option<[usize; 4]> {
    let (w, h) = (w.abs(), h.abs());
    (w >= 1.0 && h >= 1.0 && x >= 0.0 && y >= 0.0).then_some([x as usize, y as usize, w as usize, h as usize])
}

fn rop_blend(rop: u32) -> Option<&'static str> {
    match rop {
        0x0088_00C6 => Some("multiply"),
        0x00EE_0086 => Some("screen"),
        0x0066_0046 => Some("difference"),
        _ => None,
    }
}

/// BITBLT, STRETCHBLT, ALPHABLEND and TRANSPARENTBLT share a layout up to the bitmap fields.
fn blt(g: &mut Gdi, r: &Rec, stretch: bool) {
    let dest = [r.f(24), r.f(28), r.f(32), r.f(36)];
    let rop = r.u32(40);
    let src = if stretch {
        abs_src(r.f(44), r.f(48), r.f(100), r.f(104))
    } else {
        abs_src(r.f(44), r.f(48), dest[2], dest[3])
    };
    if r.u32(88) == 0 {
        // No source bitmap: the raster operation paints with the brush or a solid colour.
        match rop {
            0x00F0_0021 => g.fill_box([dest[0], dest[1], dest[0] + dest[2], dest[1] + dest[3]]),
            0x0000_0042 => g.fill_box_color([dest[0], dest[1], dest[0] + dest[2], dest[1] + dest[3]], Rgb(0, 0, 0)),
            0x00FF_0062 => g.fill_box_color([dest[0], dest[1], dest[0] + dest[2], dest[1] + dest[3]], Rgb(255, 255, 255)),
            _ => {}
        }
        return;
    }
    if let Some(pic) = bitmap(r, 84, 92) {
        g.image(&ImageDraw { pic, src, dest, blend: rop_blend(rop) });
    }
}

/// EMR_GRADIENTFILL: rectangles shaded across or down from one corner's colour to the other's, or triangles.
fn gradient_fill(g: &mut Gdi, r: &Rec) {
    let (n_ver, n_tri, mode) = (r.u32(24) as usize, r.u32(28) as usize, r.u32(32));
    if n_ver > 100_000 || n_tri > 100_000 {
        return;
    }
    // The counts are the file's word; what the record holds is what is there. Take only the vertices that fit, and
    // only the triangles (12 bytes each, or 8 for rectangles) that fit after them.
    let n_ver = n_ver.min(r.0.len().saturating_sub(36) / 16);
    let n_tri = n_tri.min(r.0.len().saturating_sub(36 + n_ver * 16) / if mode == 2 { 12 } else { 8 });
    // TRIVERTEX: x, y, then red, green, blue and alpha as 16-bit values whose high byte is the colour.
    let vertex = |k: usize| {
        let o = 36 + k * 16;
        (r.f(o), r.f(o + 4), Rgb((r.u16(o + 8) >> 8) as u8, (r.u16(o + 10) >> 8) as u8, (r.u16(o + 12) >> 8) as u8))
    };
    let base = 36 + n_ver * 16;
    for t in 0..n_tri {
        if mode == 2 {
            let o = base + t * 12;
            let ks = [r.u32(o) as usize, r.u32(o + 4) as usize, r.u32(o + 8) as usize];
            if ks.iter().any(|&k| k >= n_ver) {
                continue;
            }
            let v = ks.map(vertex);
            let avg = |f: fn(&Rgb) -> u8| ((f(&v[0].2) as u32 + f(&v[1].2) as u32 + f(&v[2].2) as u32) / 3) as u8;
            g.fill_triangle([(v[0].0, v[0].1), (v[1].0, v[1].1), (v[2].0, v[2].1)], Rgb(avg(|c| c.0), avg(|c| c.1), avg(|c| c.2)));
        } else {
            let o = base + t * 8;
            let (a, b) = (r.u32(o) as usize, r.u32(o + 4) as usize);
            if a >= n_ver || b >= n_ver {
                continue;
            }
            let (va, vb) = (vertex(a), vertex(b));
            // GDI shades by where the vertices are (the first colour at the left or the top), whichever index names which.
            let reversed = if mode == 1 { va.1 > vb.1 } else { va.0 > vb.0 };
            let (from, to) = if reversed { (vb.2, va.2) } else { (va.2, vb.2) };
            g.fill_box_gradient([va.0.min(vb.0), va.1.min(vb.1), va.0.max(vb.0), va.1.max(vb.1)], from, to, mode == 1);
        }
    }
}

fn text_record(g: &mut Gdi, r: &Rec, wide: bool) {
    let n = (r.u32(44) as usize).min(r.0.len());
    let off = r.u32(48) as usize;
    let options = r.u32(52);
    let text = if wide {
        let units: Vec<u16> = r.slice(off, n * 2).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        ansi_to_string(r.slice(off, n))
    };
    let off_dx = r.u32(72) as usize;
    let per = if options & 0x2000 != 0 { 2 } else { 1 };
    let dx: Vec<f64> = if off_dx == 0 {
        vec![]
    } else {
        let raw = r.slice(off_dx, n.saturating_mul(per).saturating_mul(4));
        raw.chunks_exact(4 * per).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64).collect()
    };
    let (x, y) = r.point(36);
    g.text(&TextRun {
        x,
        y,
        text: &text,
        dx: (!dx.is_empty()).then_some(&dx[..]),
        opaque: (options & 2 != 0).then(|| r.rect(56)),
    });
}

fn small_text(g: &mut Gdi, r: &Rec) {
    let n = (r.u32(16) as usize).min(r.0.len());
    let options = r.u32(20);
    let at = if options & 0x100 != 0 { 36 } else { 52 };
    let text = if options & 0x200 != 0 {
        ansi_to_string(r.slice(at, n))
    } else {
        let units: Vec<u16> = r.slice(at, n * 2).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    };
    g.text(&TextRun { x: r.f(8), y: r.f(12), text: &text, dx: None, opaque: None });
}

fn polypoly(g: &mut Gdi, r: &Rec, wide: bool, closed: bool) {
    let polys = (r.u32(24) as usize).min(r.0.len() / 4);
    let counts_at = 32;
    let pts_at = counts_at + polys * 4;
    let size = if wide { 8 } else { 4 };
    let total = r.0.len().saturating_sub(pts_at) / size;
    let pts = r.points(pts_at, total, wide);
    let mut list = Vec::new();
    let mut start = 0usize;
    for i in 0..polys {
        let n = (r.u32(counts_at + i * 4) as usize).min(pts.len() - start);
        list.push(pts[start..start + n].to_vec());
        start += n;
    }
    g.poly_poly(&list, closed);
}

fn set_object(g: &mut Gdi, handle: u32, obj: Object) {
    if handle != 0 && handle < 1 << 20 {
        g.objects.insert(handle, obj);
    }
}

fn record(g: &mut Gdi, ty: u32, r: &Rec) {
    match ty {
        2 | 3 | 4 | 5 | 6 | 85 | 86 | 87 | 88 | 89 => {
            let wide = ty < 10;
            let pts = r.points(28, r.u32(24) as usize, wide);
            let kind = match ty {
                2 | 85 => PolyKind::Bezier,
                3 | 86 => PolyKind::Polygon,
                4 | 87 => PolyKind::Polyline,
                5 | 88 => PolyKind::BezierTo,
                _ => PolyKind::PolylineTo,
            };
            g.poly(kind, &pts);
        }
        7 | 8 | 90 | 91 => polypoly(g, r, ty < 10, ty == 8 || ty == 91),
        56 | 92 => {
            let wide = ty == 56;
            let n = (r.u32(24) as usize).min(r.0.len());
            let pts = r.points(28, n, wide);
            let types = r.slice(28 + pts.len() * if wide { 8 } else { 4 }, pts.len());
            g.poly_draw(&pts, types);
        }
        9 => g.set_window_ext(r.f(8), r.f(12)),
        10 => g.set_window_org(r.f(8), r.f(12)),
        11 => g.set_viewport_ext(r.f(8), r.f(12)),
        12 => g.set_viewport_org(r.f(8), r.f(12)),
        31 => g.scale_viewport_ext(r.f(8), r.f(12), r.f(16), r.f(20)),
        32 => g.scale_window_ext(r.f(8), r.f(12), r.f(16), r.f(20)),
        17 => g.set_map_mode(r.u32(8)),
        18 => g.dc.bk_mode = r.u32(8),
        19 => g.dc.poly_fill = r.u32(8),
        22 => g.dc.text_align = r.u32(8),
        24 => g.dc.text_color = colorref(r, 8),
        25 => g.dc.bk_color = colorref(r, 8),
        57 => g.dc.arc_cw = r.u32(8) == 2,
        27 => g.move_to(r.f(8), r.f(12)),
        54 => g.line_to(r.f(8), r.f(12)),
        33 => g.save_dc(),
        34 => g.restore_dc(r.i32(8)),
        35 => g.set_world(r.xform(8)),
        36 => g.modify_world(r.xform(8), r.u32(32)),
        37 => g.select_object(r.u32(8)),
        40 => {
            g.objects.remove(&r.u32(8));
        }
        38 => {
            let pen = Pen::from_style(r.u32(12), r.f(16), colorref(r, 24), vec![]);
            set_object(g, r.u32(8), Object::Pen(pen));
        }
        95 => {
            let n = (r.u32(48) as usize).min(r.0.len() / 4);
            let dash = (0..n).map(|i| r.f(52 + i * 4)).collect();
            let pen = Pen::from_style(r.u32(28), r.f(32), colorref(r, 40), dash);
            set_object(g, r.u32(8), Object::Pen(pen));
        }
        39 => {
            let c = colorref(r, 16);
            let brush = match r.u32(12) {
                0 => Brush::Solid(c),
                1 => Brush::Hollow,
                2 => Brush::Hatch(c),
                _ => Brush::Solid(Rgb(128, 128, 128)),
            };
            set_object(g, r.u32(8), Object::Brush(brush));
        }
        93 | 94 => set_object(g, r.u32(8), Object::Brush(Brush::Solid(Rgb(128, 128, 128)))),
        49 | 99 | 122 => set_object(g, r.u32(8), Object::Other),
        82 => {
            let font = Font::from_logfont(
                r.i32(12),
                r.i32(20),
                r.u32(28),
                (r.u8(32) != 0, r.u8(33) != 0, r.u8(34) != 0),
                r.u8(39),
                &logfont_face(r, 40),
            );
            set_object(g, r.u32(8), Object::Font(font));
        }
        42 => g.ellipse(r.rect(8)),
        43 => g.rect(r.rect(8)),
        44 => g.round_rect(r.rect(8), r.f(24), r.f(28)),
        45 => g.arc(ArcKind::Arc, r.rect(8), r.point(24), r.point(32)),
        46 => g.arc(ArcKind::Chord, r.rect(8), r.point(24), r.point(32)),
        47 => g.arc(ArcKind::Pie, r.rect(8), r.point(24), r.point(32)),
        55 => g.arc(ArcKind::ArcTo, r.rect(8), r.point(24), r.point(32)),
        41 => g.angle_arc(r.f(8), r.f(12), r.u32(16) as f64, r.f32(20), r.f32(24)),
        59 => g.begin_path(),
        60 => g.end_path(),
        61 => g.close_figure(),
        62 => g.fill_path(true, false),
        63 => g.fill_path(true, true),
        64 => g.fill_path(false, true),
        68 => g.abort_path(),
        67 => g.clip_to_path(r.u32(8)),
        30 => g.clip_rect(r.rect(8), 1),
        75 => {
            let (bytes, mode) = (r.u32(8), r.u32(12));
            if bytes == 0 {
                if mode == 5 {
                    g.clear_clip();
                }
            } else {
                let count = (r.u32(24) as usize).min(r.0.len() / 16);
                let rects: Vec<[f64; 4]> = (0..count).map(|i| r.rect(48 + i * 16)).collect();
                g.clip_rects(&rects, mode);
            }
        }
        118 => gradient_fill(g, r),
        83 => text_record(g, r, false),
        84 => text_record(g, r, true),
        108 => small_text(g, r),
        76 => blt(g, r, false),
        77 | 114 | 116 => blt(g, r, true),
        81 => {
            if let Some(pic) = bitmap(r, 48, 56) {
                let src = abs_src(r.f(32), r.f(36), r.f(40), r.f(44));
                let dest = [r.f(24), r.f(28), r.f(72), r.f(76)];
                g.image(&ImageDraw { pic, src, dest, blend: rop_blend(r.u32(68)) });
            }
        }
        80 => {
            if let Some(pic) = bitmap(r, 48, 56) {
                let (w, h) = (r.f(40).abs(), r.f(44).abs());
                let dest = [r.f(24), r.f(28), w, h];
                g.image(&ImageDraw { pic, src: abs_src(r.f(32), r.f(36), w, h), dest, blend: None });
            }
        }
        _ => {}
    }
}

/// Convert an EMF file to an SVG document.
pub fn emf_to_svg(bytes: &[u8]) -> Result<String, String> {
    crate::budget::within(bytes.len(), crate::budget::caps::PICTURE_BYTES).map_err(|e| e.to_string())?;
    let h = header(bytes)?;
    let px_per_mm = if h.device.0 > 0 && h.mm.0 > 0 { h.device.0 as f64 / h.mm.0 as f64 } else { PX_PER_MM_CSS };
    let valid = |r: [i32; 4]| r[2] > r[0] && r[3] > r[1];
    // The view box is the bounds of the drawing in device units, or else the picture frame.
    let mut view = if valid(h.bounds) {
        Some((
            h.bounds[0] as f64,
            h.bounds[1] as f64,
            (h.bounds[2] - h.bounds[0]) as f64 + 1.0,
            (h.bounds[3] - h.bounds[1]) as f64 + 1.0,
        ))
    } else if valid(h.frame) {
        let k = px_per_mm / 100.0;
        Some((
            h.frame[0] as f64 * k,
            h.frame[1] as f64 * k,
            (h.frame[2] - h.frame[0]) as f64 * k,
            (h.frame[3] - h.frame[1]) as f64 * k,
        ))
    } else {
        None
    };
    // The size on the page comes from the frame (0.01 mm), if it agrees with the bounds.
    let css_per_dev = PX_PER_MM_CSS / px_per_mm;
    let mut size = view.map(|v| (v.2 * css_per_dev, v.3 * css_per_dev));
    if valid(h.frame) {
        let fw = (h.frame[2] - h.frame[0]) as f64 / 100.0 * PX_PER_MM_CSS;
        let fh = (h.frame[3] - h.frame[1]) as f64 / 100.0 * PX_PER_MM_CSS;
        let agrees = view.is_none_or(|v| ((v.2 / v.3) / (fw / fh) - 1.0).abs() < 0.2);
        if agrees {
            size = Some((fw, fh));
        }
    }
    let px_per_dev = match (size, view) {
        (Some(s), Some(v)) => s.0 / v.2,
        _ => css_per_dev,
    };
    let mut g = Gdi::new(px_per_mm, px_per_dev);
    let mut pos = 0usize;
    while pos + 8 <= bytes.len() {
        let r = Rec(&bytes[pos..]);
        let (ty, size_rec) = (r.u32(0), r.u32(4) as usize);
        if size_rec < 8 || size_rec % 4 != 0 || pos + size_rec > bytes.len() {
            break;
        }
        pos += size_rec;
        if ty == 14 {
            break;
        }
        if ty != 1 {
            record(&mut g, ty, &Rec(&r.0[..size_rec]));
        }
    }
    if view.is_none() {
        view = g.drawn_extent();
        if let Some(v) = view {
            size = Some((v.2 * css_per_dev, v.3 * css_per_dev));
        }
    }
    let view = view.unwrap_or((0.0, 0.0, 100.0, 100.0));
    let size = size.unwrap_or((view.2, view.3));
    Ok(g.finish(view, size))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::imgconv::testutil::{attr_numbers, element, numbers, W};

    /// Builds an EMF in memory, record by record.
    pub struct EmfBuilder {
        w: W,
        records: u32,
    }

    impl EmfBuilder {
        /// Bounds and frame as (l, t, r, b); a 1000x1000 pixel device on a 250 mm wide screen.
        pub fn new(bounds: [i32; 4], frame: [i32; 4]) -> EmfBuilder {
            let mut w = W::new();
            w.u32(1).u32(100);
            for v in bounds.iter().chain(frame.iter()) {
                w.i32(*v);
            }
            w.bytes(b" EMF").u32(0x10000).u32(0).u32(0).u16(8).u16(0).u32(0).u32(0).u32(0);
            w.i32(1000).i32(1000).i32(250).i32(250);
            w.u32(0).u32(0).u32(0);
            assert_eq!(w.len(), 100);
            EmfBuilder { w, records: 1 }
        }

        pub fn rec(&mut self, ty: u32, payload: &[u8]) -> &mut EmfBuilder {
            let mut p = payload.to_vec();
            while p.len() & 3 != 0 {
                p.push(0);
            }
            self.w.u32(ty).u32(8 + p.len() as u32).bytes(&p);
            self.records += 1;
            self
        }

        pub fn ints(&mut self, ty: u32, vals: &[i32]) -> &mut EmfBuilder {
            let mut p = W::new();
            for v in vals {
                p.i32(*v);
            }
            self.rec(ty, &p.b)
        }

        pub fn build(mut self) -> Vec<u8> {
            self.rec(14, &[0; 12]);
            let n = self.w.len() as u32;
            let mut b = self.w.b;
            b[48..52].copy_from_slice(&n.to_le_bytes());
            b[52..56].copy_from_slice(&self.records.to_le_bytes());
            b
        }
    }

    fn simple() -> EmfBuilder {
        EmfBuilder::new([0, 0, 199, 99], [0, 0, 5000, 2500])
    }

    #[test]
    fn view_box_and_size_come_from_bounds_and_frame() {
        let svg = emf_to_svg(&simple().build()).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("viewBox=\"0 0 200 100\""), "{svg}");
        // 5000 x 2500 hundredths of a millimetre at 96 dpi.
        assert!(svg.contains("width=\"188.976\" height=\"94.488\""), "{svg}");
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn empty_bounds_fall_back_to_the_frame() {
        let svg = emf_to_svg(&EmfBuilder::new([0, 0, -1, -1], [0, 0, 2500, 2500]).build()).unwrap();
        // 25 mm on a 4 px/mm device is 100 device units.
        assert!(svg.contains("viewBox=\"0 0 100 100\""), "{svg}");
    }

    #[test]
    fn polygon_with_selected_pen_and_brush() {
        let mut b = simple();
        b.ints(39, &[1, 0, 0x0000FF, 0]); // brush 1: solid red (COLORREF 0x00BBGGRR)
        b.ints(38, &[2, 0, 2, 0, 0x00FF00]); // pen 2: width 2, green
        b.ints(37, &[1]).ints(37, &[2]);
        let mut p = W::new();
        p.i32(0).i32(0).i32(0).i32(0).u32(3);
        for (x, y) in [(10, 10), (60, 10), (35, 50)] {
            p.i32(x).i32(y);
        }
        b.rec(3, &p.b);
        let svg = emf_to_svg(&b.build()).unwrap();
        let path = element(&svg, "path").unwrap();
        assert!(path.contains("d=\"M10 10 L60 10 L35 50 Z\""), "{path}");
        assert!(path.contains("fill=\"#ff0000\""), "{path}");
        assert!(path.contains("stroke=\"#00ff00\" stroke-width=\"2\""), "{path}");
    }

    #[test]
    fn sixteen_bit_polyline_and_current_position() {
        let mut b = simple();
        b.ints(27, &[5, 5]);
        let mut p = W::new();
        p.i32(0).i32(0).i32(0).i32(0).u32(2);
        p.i16(20).i16(5).i16(20).i16(40);
        b.rec(89, &p.b); // POLYLINETO16
        let svg = emf_to_svg(&b.build()).unwrap();
        let path = element(&svg, "path").unwrap();
        assert!(path.contains("d=\"M5 5 L20 5 L20 40 \""), "{path}");
        assert!(path.contains("fill=\"none\""));
    }

    #[test]
    fn rectangle_ellipse_and_stock_objects() {
        let mut b = simple();
        b.ints(37, &[0x8000_0005u32 as i32]); // NULL_BRUSH
        b.ints(37, &[0x8000_0007u32 as i32]); // BLACK_PEN
        b.ints(43, &[10, 20, 110, 70]);
        b.ints(37, &[0x8000_0004u32 as i32]); // BLACK_BRUSH
        b.ints(42, &[0, 0, 40, 20]);
        let svg = emf_to_svg(&b.build()).unwrap();
        let rect = element(&svg, "path").unwrap();
        assert!(rect.contains("M10 20 L110 20 L110 70 L10 70 Z") && rect.contains("fill=\"none\""), "{rect}");
        assert!(rect.contains("stroke=\"#000000\""), "{rect}");
        assert!(svg.contains("M0 10 A20 10 0 1 1 40 10 A20 10 0 1 1 0 10 Z"), "{svg}");
        assert!(svg.contains("fill=\"#000000\""), "{svg}");
    }

    #[test]
    fn mapping_modes_and_world_transform() {
        let mut b = simple();
        b.ints(17, &[8]); // MM_ANISOTROPIC
        b.ints(9, &[100, 100]).ints(11, &[200, 200]);
        let mut x = W::new();
        for v in [1.0f32, 0.0, 0.0, 1.0, 10.0, 0.0] {
            x.f32(v);
        }
        b.rec(35, &x.b);
        b.ints(43, &[0, 0, 5, 5]);
        let svg = emf_to_svg(&b.build()).unwrap();
        // Logical to device: translate by 10, then scale by 2.
        assert!(svg.contains("matrix(2 0 0 2 20 0)"), "{svg}");
    }

    #[test]
    fn modify_world_transform_multiplies() {
        let mut b = simple();
        let mut x = W::new();
        for v in [2.0f32, 0.0, 0.0, 2.0, 0.0, 0.0] {
            x.f32(v);
        }
        x.u32(4); // set
        b.rec(36, &x.b);
        let mut t = W::new();
        for v in [1.0f32, 0.0, 0.0, 1.0, 5.0, 0.0] {
            t.f32(v);
        }
        t.u32(2); // left multiply: translate first, then scale
        b.rec(36, &t.b);
        b.ints(43, &[0, 0, 1, 1]);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("matrix(2 0 0 2 10 0)"), "{svg}");
    }

    #[test]
    fn text_with_font_alignment_and_color() {
        let mut b = simple();
        let mut f = W::new();
        f.u32(3); // handle
        f.i32(-24).i32(0).i32(0).i32(0).i32(700); // height, width, escapement, orientation, weight
        f.u8(1).u8(0).u8(0).u8(0).u8(0).u8(0).u8(0).u8(0x20); // italic .. pitch/family (swiss)
        let mut face = [0u16; 32];
        for (i, c) in "Calibri".encode_utf16().enumerate() {
            face[i] = c;
        }
        for c in face {
            f.u16(c);
        }
        b.rec(82, &f.b);
        b.ints(37, &[3]);
        b.ints(24, &[0x0000_00FF]); // red text
        b.ints(22, &[6 | 24]); // centered on the baseline
        let text: Vec<u8> = "Hi & <you>".encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        let mut t = W::new();
        t.i32(0).i32(0).i32(0).i32(0); // bounds
        t.u32(1).u32(0).u32(0); // graphics mode, scales
        t.i32(100).i32(50); // reference
        t.u32(10).u32(76).u32(0); // chars, offString, options
        t.i32(0).i32(0).i32(0).i32(0); // rcl
        t.u32(0); // offDx
        t.bytes(&text);
        b.rec(84, &t.b);
        let svg = emf_to_svg(&b.build()).unwrap();
        let el = element(&svg, "text").unwrap();
        assert!(el.contains("translate(100 50)"), "{el}");
        assert!(el.contains("font-family=\"'Calibri', sans-serif\"") && el.contains("font-size=\"24\""), "{el}");
        assert!(el.contains("fill=\"#ff0000\"") && el.contains("text-anchor=\"middle\""), "{el}");
        assert!(el.contains("font-weight=\"bold\"") && el.contains("font-style=\"italic\""), "{el}");
        assert!(svg.contains(">Hi &amp; &lt;you&gt;</text>"), "{svg}");
    }

    #[test]
    fn rotated_text_and_ansi_text() {
        let mut b = simple();
        let mut f = W::new();
        f.u32(1).i32(-20).i32(0).i32(450).i32(0).i32(400);
        f.bytes(&[0; 8]);
        for _ in 0..32 {
            f.u16(0);
        }
        b.rec(82, &f.b);
        b.ints(37, &[1]);
        let mut t = W::new();
        t.i32(0).i32(0).i32(0).i32(0).u32(1).u32(0).u32(0).i32(10).i32(10);
        t.u32(3).u32(76).u32(0);
        t.i32(0).i32(0).i32(0).i32(0).u32(0);
        t.bytes(b"abc");
        b.rec(83, &t.b);
        let svg = emf_to_svg(&b.build()).unwrap();
        let el = element(&svg, "text").unwrap();
        assert!(el.contains("rotate(-45)"), "{el}");
        assert!(svg.contains(">abc</text>"));
    }

    #[test]
    fn bitmap_is_embedded_and_placed() {
        let dib = crate::imgconv::dib::tests::make_dib(2, 2, 24, &[], &[0x10; 16]);
        let mut p = W::new();
        p.i32(0).i32(0).i32(0).i32(0); // bounds
        p.i32(30).i32(40); // dest x, y
        p.i32(0).i32(0).i32(2).i32(2); // src x, y, cx, cy
        p.u32(80).u32(40).u32(120).u32(16); // offBmi, cbBmi, offBits, cbBits
        p.u32(0).u32(0x00CC0020); // usage, rop
        p.i32(60).i32(80); // dest cx, cy
        assert_eq!(p.len() + 8, 80);
        p.bytes(&dib);
        let mut b = simple();
        b.rec(81, &p.b);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("translate(30 40) scale(30 40)"), "{svg}");
        assert!(svg.contains("href=\"data:image/bmp;base64,Qk"), "{svg}");
        assert_eq!(attr_numbers(&svg, "width")[1], vec![2.0]);
    }

    #[test]
    fn clip_rect_becomes_a_clip_path() {
        let mut b = simple();
        b.ints(30, &[5, 5, 50, 50]);
        b.ints(43, &[0, 0, 100, 100]);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("<clipPath id=\"c1\"><path d=\"M5 5 L50 5 L50 50 L5 50 Z\""), "{svg}");
        assert!(svg.contains("<g clip-path=\"url(#c1)\">"), "{svg}");
    }

    #[test]
    fn path_bracket_fill_and_clip() {
        let mut b = simple();
        b.rec(59, &[]);
        b.ints(27, &[0, 0]).ints(54, &[40, 0]).ints(54, &[40, 40]);
        b.rec(61, &[]).rec(60, &[]);
        b.ints(62, &[0, 0, 0, 0]);
        b.rec(59, &[]);
        b.ints(43, &[0, 0, 30, 30]);
        b.rec(60, &[]);
        b.ints(67, &[5]);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("d=\"M0 0 L40 0 L40 40 Z\""), "{svg}");
        assert!(svg.contains("<clipPath id=\"c1\"><path d=\"M0 0 L30 0 L30 30 L0 30 Z\""), "{svg}");
    }

    #[test]
    fn dashed_extended_pen() {
        let mut b = simple();
        let mut p = W::new();
        p.u32(1).u32(0).u32(0).u32(0).u32(0); // handle, bmi/bits offsets and sizes
        p.u32(0x0001_0000 | 1).u32(4).u32(0).u32(0x0000_FF00).u32(0).u32(0); // geometric dash, width 4
        b.rec(95, &p.b);
        b.ints(37, &[1]);
        b.ints(27, &[0, 0]).ints(54, &[50, 0]);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("stroke-width=\"4\"") && svg.contains("stroke-dasharray=\"24 8\""), "{svg}");
        assert!(svg.contains("stroke=\"#00ff00\""), "{svg}");
    }

    #[test]
    fn arcs_pies_and_round_rects() {
        let mut b = simple();
        b.ints(45, &[0, 0, 20, 10, 20, 5, 10, 0]); // ARC: right to top
        b.ints(47, &[0, 0, 20, 20, 20, 10, 10, 0]); // PIE
        b.ints(44, &[0, 0, 40, 20, 10, 10]); // ROUNDRECT
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("M20 5 A10 5 0 0 0 10 0"), "{svg}");
        assert!(svg.contains("M10 10 L20 10 A10 10 0 0 0 10 0 Z"), "{svg}");
        assert!(svg.contains("M5 0 L35 0 A5 5 0 0 1 40 5"), "{svg}");
    }

    #[test]
    fn save_and_restore_device_context() {
        let mut b = simple();
        b.ints(24, &[0x0000FF]);
        b.rec(33, &[]);
        b.ints(24, &[0x00FF00]);
        b.ints(34, &[-1]);
        let mut t = W::new();
        t.i32(0).i32(0).i32(0).i32(0).u32(1).u32(0).u32(0).i32(10).i32(10);
        t.u32(1).u32(76).u32(0);
        t.i32(0).i32(0).i32(0).i32(0).u32(0);
        t.bytes(b"x");
        b.rec(83, &t.b);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(element(&svg, "text").unwrap().contains("fill=\"#ff0000\""), "{svg}");
    }

    #[test]
    fn emf_plus_comments_are_ignored_and_plain_records_drawn() {
        let mut b = simple();
        let mut c = W::new();
        c.u32(12).bytes(b"EMF+").u32(0xDEAD_BEEF);
        b.rec(70, &c.b);
        b.ints(43, &[1, 2, 3, 4]);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("M1 2 L3 2 L3 4 L1 4 Z"), "{svg}");
    }

    #[test]
    fn gradient_fills_are_drawn() {
        let mut b = EmfBuilder::new([0, 0, 100, 100], [0, 0, 2646, 2646]);
        // Bounds, 2 vertices, 1 rectangle, mode 0 (across): red at the top left, blue at the bottom right.
        let mut payload: Vec<u8> = Vec::new();
        for v in [0i32, 0, 100, 50] {
            payload.extend_from_slice(&v.to_le_bytes());
        }
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        for (x, y, rgb) in [(0i32, 0i32, [0xFF00u16, 0, 0]), (100, 50, [0, 0, 0xFF00])] {
            payload.extend_from_slice(&x.to_le_bytes());
            payload.extend_from_slice(&y.to_le_bytes());
            for c in rgb {
                payload.extend_from_slice(&c.to_le_bytes());
            }
            payload.extend_from_slice(&0u16.to_le_bytes());
        }
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        b.rec(118, &payload);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("<linearGradient"), "{svg}");
        assert!(svg.contains("#ff0000") && svg.contains("#0000ff"), "{svg}");
    }

    #[test]
    fn unknown_records_are_skipped_and_bad_files_rejected() {
        let mut b = simple();
        b.rec(9999, &[1, 2, 3, 4]).ints(43, &[0, 0, 5, 5]);
        assert!(emf_to_svg(&b.build()).unwrap().contains("L5 0"));
        assert!(emf_to_svg(b"nope").is_err());
        assert!(emf_to_svg(&[]).is_err());
        let mut bad = simple().build();
        bad[40] = b'X';
        assert!(emf_to_svg(&bad).is_err());
    }

    #[test]
    fn missing_bounds_and_frame_use_the_drawn_extent() {
        let mut b = EmfBuilder::new([0, 0, 0, 0], [0, 0, 0, 0]);
        b.ints(43, &[10, 10, 60, 40]);
        let svg = emf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("viewBox=\"10 10 50 30\""), "{svg}");
    }

    fn rich() -> Vec<u8> {
        let mut b = simple();
        b.ints(39, &[1, 0, 0x0000FF, 0]).ints(38, &[2, 0, 2, 0, 0xFF00]).ints(37, &[1]).ints(37, &[2]);
        let mut p = W::new();
        p.i32(0).i32(0).i32(0).i32(0).u32(3);
        for (x, y) in [(10, 10), (60, 10), (35, 50)] {
            p.i32(x).i32(y);
        }
        b.rec(3, &p.b);
        b.ints(30, &[0, 0, 50, 50]).ints(42, &[0, 0, 30, 30]);
        b.rec(59, &[]).ints(27, &[0, 0]).ints(54, &[9, 9]).rec(60, &[]).ints(62, &[0, 0, 0, 0]);
        let dib = crate::imgconv::dib::tests::make_dib(2, 2, 24, &[], &[0x10; 16]);
        let mut q = W::new();
        q.i32(0).i32(0).i32(0).i32(0).i32(30).i32(40).i32(0).i32(0).i32(2).i32(2);
        q.u32(80).u32(40).u32(120).u32(16).u32(0).u32(0x00CC0020).i32(60).i32(80);
        q.bytes(&dib);
        b.rec(81, &q.b);
        let mut f = W::new();
        f.u32(3).i32(-24).bytes(&[0; 16]).bytes(&[0; 8]);
        for _ in 0..32 {
            f.u16(65);
        }
        b.rec(82, &f.b).ints(37, &[3]);
        let mut t = W::new();
        t.i32(0).i32(0).i32(0).i32(0).u32(1).u32(0).u32(0).i32(10).i32(10).u32(2).u32(76).u32(0);
        t.i32(0).i32(0).i32(0).i32(0).u32(0);
        t.bytes(&[b'h', 0, b'i', 0]);
        b.rec(84, &t.b);
        b.build()
    }

    #[test]
    fn every_prefix_and_corruption_is_safe() {
        let full = rich();
        let svg = emf_to_svg(&full).unwrap();
        assert!(svg.contains("<image") && svg.contains("<text") && svg.contains("clip-path"));
        for n in 0..full.len() {
            let _ = emf_to_svg(&full[..n]);
        }
        for i in 0..full.len() {
            let mut g = full.clone();
            g[i] ^= 0xFF;
            let _ = emf_to_svg(&g);
            let mut g = full.clone();
            g[i] = 0x7F;
            let _ = emf_to_svg(&g);
        }
        // Garbage after a valid header.
        let mut junk = full[..100].to_vec();
        junk.extend((0..2000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8));
        let _ = emf_to_svg(&junk);
    }

    #[test]
    fn hostile_counts_do_not_allocate() {
        let mut b = simple();
        let mut p = W::new();
        p.i32(0).i32(0).i32(0).i32(0).u32(0xFFFF_FFFF);
        p.i32(1).i32(1);
        b.rec(3, &p.b);
        let mut q = W::new();
        q.i32(0).i32(0).i32(0).i32(0).u32(0xFFFF_FFFF).u32(0xFFFF_FFFF);
        b.rec(8, &q.b);
        b.ints(95, &[1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0x7FFF_FFFF]);
        assert!(emf_to_svg(&b.build()).is_ok());
        assert_eq!(numbers("M1 2 L-3.5 4"), vec![1.0, 2.0, -3.5, 4.0]);
    }
}
