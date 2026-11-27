//! WMF (Windows Metafile) to SVG. The 16-bit records are played into a `Gdi`, the same drawing
//! engine EMF uses. WMF writes call arguments in reverse order, so a rectangle is
//! (bottom, right, top, left) and a point is (y, x); the accessors below keep that straight.

use super::dib;
use super::gdi::*;
use super::gdi_draw::*;

const PX_PER_INCH: f64 = 96.0;

/// A bounds-checked view of one record: size (words), function, then the parameters from byte 6.
struct Rec<'a>(&'a [u8]);

impl Rec<'_> {
    fn u16(&self, o: usize) -> u16 {
        self.0.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn i16(&self, o: usize) -> i16 {
        self.u16(o) as i16
    }
    fn f(&self, o: usize) -> f64 {
        self.i16(o) as f64
    }
    fn u32(&self, o: usize) -> u32 {
        self.0.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u8(&self, o: usize) -> u8 {
        self.0.get(o).copied().unwrap_or(0)
    }
    fn func(&self) -> u16 {
        self.u16(4)
    }
    fn tail(&self, o: usize) -> &[u8] {
        self.0.get(o..).unwrap_or(&[])
    }
    /// A (y, x) pair at `o`, returned as a point.
    fn yx(&self, o: usize) -> (f64, f64) {
        (self.f(o + 2), self.f(o))
    }
    /// A rectangle written (bottom, right, top, left) at `o`, as (left, top, right, bottom).
    fn rect_brtl(&self, o: usize) -> [f64; 4] {
        [self.f(o + 6), self.f(o + 4), self.f(o + 2), self.f(o)]
    }
    /// `n` (x, y) points from `o`.
    fn points(&self, o: usize, n: usize) -> Vec<(f64, f64)> {
        let n = n.min(self.0.len().saturating_sub(o) / 4);
        (0..n).map(|i| (self.f(o + i * 4), self.f(o + i * 4 + 2))).collect()
    }
}

struct Layout {
    start: usize,
    /// Bounding box (left, top, right, bottom) and units per inch of a placeable file.
    placeable: Option<([i16; 4], u16)>,
}

fn layout(b: &[u8]) -> Result<Layout, String> {
    let r = Rec(b);
    let (start, placeable) = if r.u32(0) == 0x9AC6_CDD7 {
        let bbox = [r.i16(6), r.i16(8), r.i16(10), r.i16(12)];
        (22, Some((bbox, r.u16(14))))
    } else {
        (0, None)
    };
    let h = Rec(b.get(start..).unwrap_or(&[]));
    if !matches!(h.u16(0), 1 | 2) || h.u16(2) != 9 {
        return Err("Not a WMF file".into());
    }
    Ok(Layout { start: start + 18, placeable })
}

/// Walk the records, calling `f` with each (the end-of-file record excluded).
fn each_record(b: &[u8], start: usize, mut f: impl FnMut(&Rec)) {
    let mut pos = start;
    while pos + 6 <= b.len() {
        let words = Rec(&b[pos..]).u32(0) as usize;
        let size = words.saturating_mul(2);
        if words < 3 || pos.saturating_add(size) > b.len() {
            break;
        }
        let rec = Rec(&b[pos..pos + size]);
        pos += size;
        if rec.func() == 0 {
            break;
        }
        f(&rec);
    }
}

fn rop_blend(rop: u32) -> Option<&'static str> {
    match rop {
        0x0088_00C6 => Some("multiply"),
        0x00EE_0086 => Some("screen"),
        0x0066_0046 => Some("difference"),
        _ => None,
    }
}

fn src_rect(x: f64, y: f64, w: f64, h: f64) -> Option<[usize; 4]> {
    let (w, h) = (w.abs(), h.abs());
    (w >= 1.0 && h >= 1.0 && x >= 0.0 && y >= 0.0).then_some([x as usize, y as usize, w as usize, h as usize])
}

fn draw_dib(g: &mut Gdi, dib_bytes: &[u8], src: Option<[usize; 4]>, dest: [f64; 4], rop: u32) {
    if let Some(pic) = dib::picture_from_dib(dib_bytes) {
        g.image(&ImageDraw { pic, src, dest, blend: rop_blend(rop) });
    }
}

/// A pattern-only blit (no bitmap): fill with the brush or a solid colour.
fn fill_by_rop(g: &mut Gdi, b: [f64; 4], rop: u32) {
    let b = [b[0], b[1], b[0] + b[2], b[1] + b[3]];
    match rop {
        0x00F0_0021 => g.fill_box(b),
        0x0000_0042 => g.fill_box_color(b, Rgb(0, 0, 0)),
        0x00FF_0062 => g.fill_box_color(b, Rgb(255, 255, 255)),
        _ => {}
    }
}

fn text_record(g: &mut Gdi, r: &Rec, ext: bool) {
    let (x, y, n, opts, mut at) = if ext {
        let opts = r.u16(12);
        (r.f(8), r.f(6), r.u16(10) as usize, opts, if opts & 6 != 0 { 22 } else { 14 })
    } else {
        let n = r.u16(6) as usize;
        let after = 8 + n + n % 2;
        (r.f(after + 2), r.f(after), n, 0, 8)
    };
    let n = n.min(r.0.len());
    let bytes = r.tail(at).get(..n).unwrap_or(&[]);
    let text = ansi_to_string(bytes);
    at += n + n % 2;
    let dx: Vec<f64> = if ext {
        (0..n).map(|i| r.f(at + i * 2)).filter(|_| at + n * 2 <= r.0.len()).collect()
    } else {
        vec![]
    };
    g.text(&TextRun {
        x,
        y,
        text: &text,
        dx: (!dx.is_empty()).then_some(&dx[..]),
        opaque: (opts & 2 != 0).then(|| [r.f(14), r.f(16), r.f(18), r.f(20)]),
    });
}

fn record(g: &mut Gdi, r: &Rec) {
    match r.func() {
        0x001E => g.save_dc(),
        0x0127 => g.restore_dc(r.i16(6) as i32),
        0x0102 => g.dc.bk_mode = r.u16(6) as u32,
        0x0103 => g.set_map_mode(r.u16(6) as u32),
        0x0106 => g.dc.poly_fill = r.u16(6) as u32,
        0x012E => g.dc.text_align = r.u16(6) as u32,
        0x0209 => g.dc.text_color = Rgb::from_colorref(r.u32(6)),
        0x0201 => g.dc.bk_color = Rgb::from_colorref(r.u32(6)),
        0x020B => {
            let (x, y) = r.yx(6);
            g.set_window_org(x, y);
        }
        0x020C => {
            let (x, y) = r.yx(6);
            g.set_window_ext(x, y);
        }
        0x020D => {
            let (x, y) = r.yx(6);
            g.set_viewport_org(x, y);
        }
        0x020E => {
            let (x, y) = r.yx(6);
            g.set_viewport_ext(x, y);
        }
        0x020F => {
            let (dx, dy) = r.yx(6);
            g.set_window_org(g.dc.win_org.0 + dx, g.dc.win_org.1 + dy);
        }
        0x0211 => {
            let (dx, dy) = r.yx(6);
            g.set_viewport_org(g.dc.vp_org.0 + dx, g.dc.vp_org.1 + dy);
        }
        0x0410 => g.scale_window_ext(r.f(12), r.f(10), r.f(8), r.f(6)),
        0x0412 => g.scale_viewport_ext(r.f(12), r.f(10), r.f(8), r.f(6)),
        0x0214 => {
            let (x, y) = r.yx(6);
            g.move_to(x, y);
        }
        0x0213 => {
            let (x, y) = r.yx(6);
            g.line_to(x, y);
        }
        0x041B => g.rect(r.rect_brtl(6)),
        0x0418 => g.ellipse(r.rect_brtl(6)),
        0x061C => g.round_rect(r.rect_brtl(10), r.f(8), r.f(6)),
        0x0817 => g.arc(ArcKind::Arc, r.rect_brtl(14), r.yx(10), r.yx(6)),
        0x081A => g.arc(ArcKind::Pie, r.rect_brtl(14), r.yx(10), r.yx(6)),
        0x0830 => g.arc(ArcKind::Chord, r.rect_brtl(14), r.yx(10), r.yx(6)),
        0x0324 => g.poly(PolyKind::Polygon, &r.points(8, r.u16(6) as usize)),
        0x0325 => g.poly(PolyKind::Polyline, &r.points(8, r.u16(6) as usize)),
        0x0538 => {
            let polys = (r.u16(6) as usize).min(r.0.len() / 2);
            let pts_at = 8 + polys * 2;
            let pts = r.points(pts_at, r.0.len().saturating_sub(pts_at) / 4);
            let mut list = Vec::new();
            let mut start = 0usize;
            for i in 0..polys {
                let n = (r.u16(8 + i * 2) as usize).min(pts.len() - start);
                list.push(pts[start..start + n].to_vec());
                start += n;
            }
            g.poly_poly(&list, true);
        }
        0x0521 => text_record(g, r, false),
        0x0A32 => text_record(g, r, true),
        0x02FA => {
            let pen = Pen::from_style(r.u16(6) as u32, r.f(8), Rgb::from_colorref(r.u32(12)), vec![]);
            g.add_object(Object::Pen(pen));
        }
        0x02FC => {
            let c = Rgb::from_colorref(r.u32(8));
            let brush = match r.u16(6) {
                0 => Brush::Solid(c),
                1 => Brush::Hollow,
                2 => Brush::Hatch(c),
                _ => Brush::Solid(Rgb(128, 128, 128)),
            };
            g.add_object(Object::Brush(brush));
        }
        0x0142 | 0x01F9 => g.add_object(Object::Brush(Brush::Solid(Rgb(128, 128, 128)))),
        0x02FB => {
            let face: Vec<u8> = r.tail(24).iter().take(32).copied().take_while(|&b| b != 0).collect();
            let font = Font::from_logfont(
                r.i16(6) as i32,
                r.i16(10) as i32,
                r.u16(14) as u32,
                (r.u8(16) != 0, r.u8(17) != 0, r.u8(18) != 0),
                r.u8(23),
                &ansi_to_string(&face),
            );
            g.add_object(Object::Font(font));
        }
        0x00F7 | 0x06FF | 0x02FD | 0x02FE => g.add_object(Object::Other),
        0x012D => g.select_object(r.u16(6) as u32),
        0x01F0 => {
            g.delete_object(r.u16(6) as u32);
        }
        0x0416 => g.clip_rect(r.rect_brtl(6), 1),
        0x0F43 => {
            let dest = [r.f(26), r.f(24), r.f(22), r.f(20)];
            let src = src_rect(r.f(18), r.f(16), r.f(14), r.f(12));
            draw_dib(g, r.tail(28), src, dest, r.u32(6));
        }
        0x0B41 | 0x0B23 => {
            let dest = [r.f(24), r.f(22), r.f(20), r.f(18)];
            let src = src_rect(r.f(16), r.f(14), r.f(12), r.f(10));
            draw_dib(g, r.tail(26), src, dest, r.u32(6));
        }
        0x0940 => {
            let dest = [r.f(20), r.f(18), r.f(16), r.f(14)];
            if r.0.len() >= 22 + 12 {
                let src = src_rect(r.f(12), r.f(10), dest[2], dest[3]);
                draw_dib(g, r.tail(22), src, dest, r.u32(6));
            } else {
                fill_by_rop(g, dest, r.u32(6));
            }
        }
        0x0D33 => {
            let (w, h) = (r.f(18).abs(), r.f(16).abs());
            let dest = [r.f(22), r.f(20), w, h];
            draw_dib(g, r.tail(24), src_rect(r.f(14), r.f(12), w, h), dest, 0);
        }
        _ => {}
    }
}

/// Convert a WMF file to an SVG document.
pub fn wmf_to_svg(bytes: &[u8]) -> Result<String, String> {
    crate::budget::within(bytes.len(), crate::budget::caps::PICTURE_BYTES).map_err(|e| e.to_string())?;
    let lay = layout(bytes)?;
    // The first window origin and extent set the coordinate frame the whole picture is drawn in.
    let (mut win_org, mut win_ext) = (None, None);
    each_record(bytes, lay.start, |r| match r.func() {
        0x020B if win_org.is_none() => win_org = Some(r.yx(6)),
        0x020C if win_ext.is_none() => win_ext = Some(r.yx(6)),
        _ => {}
    });
    let bbox = lay.placeable.map(|(b, _)| b).filter(|b| b[2] != b[0] && b[3] != b[1]);
    let bbox_size = bbox.map(|b| ((b[2] as f64 - b[0] as f64).abs(), (b[3] as f64 - b[1] as f64).abs()));
    let mut g = Gdi::new(96.0 / 25.4, 1.0);
    g.lenient_ext = true;
    let mut view = None;
    if let Some(ext) = win_ext.filter(|e| e.0 != 0.0 && e.1 != 0.0) {
        let org = win_org.unwrap_or_else(|| bbox.map_or((0.0, 0.0), |b| (b[0] as f64, b[1] as f64)));
        g.dc.win_org = org;
        g.dc.win_ext = ext;
        g.dc.vp_ext = (ext.0.abs(), ext.1.abs());
        view = Some((0.0, 0.0, ext.0.abs(), ext.1.abs()));
    } else if let Some(b) = bbox {
        let (l, t) = (b[0].min(b[2]) as f64, b[1].min(b[3]) as f64);
        view = Some((l, t, bbox_size.map_or(1.0, |s| s.0), bbox_size.map_or(1.0, |s| s.1)));
    }
    // Page size from the placeable header's units per inch (1440 when it does not say).
    let units = lay.placeable.map_or(0, |p| p.1);
    let per_inch = if units == 0 { 1440.0 } else { units as f64 };
    let mut size = bbox_size.map(|s| (s.0 / per_inch * PX_PER_INCH, s.1 / per_inch * PX_PER_INCH));
    g.px_per_dev = match (size, view) {
        (Some(s), Some(v)) if v.2 > 0.0 => s.0 / v.2,
        _ => 1.0,
    };
    each_record(bytes, lay.start, |r| record(&mut g, r));
    if view.is_none() {
        view = g.drawn_extent();
    }
    let view = view.unwrap_or((0.0, 0.0, 100.0, 100.0));
    if size.is_none() {
        size = Some((view.2, view.3));
    }
    Ok(g.finish(view, size.unwrap_or((view.2, view.3))))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::imgconv::testutil::{element, W};

    /// Builds a WMF in memory: placeable header, standard header, then records.
    pub struct WmfBuilder {
        w: W,
        body_start: usize,
        max_record: u32,
    }

    impl WmfBuilder {
        /// Placeable file with the given bounding box and units per inch.
        pub fn placeable(bbox: [i16; 4], inch: u16) -> WmfBuilder {
            let mut w = W::new();
            w.u32(0x9AC6_CDD7).u16(0);
            for v in bbox {
                w.i16(v);
            }
            w.u16(inch).u32(0).u16(0);
            let mut b = WmfBuilder { w, body_start: 22, max_record: 0 };
            b.standard_header();
            b
        }

        pub fn plain() -> WmfBuilder {
            let mut b = WmfBuilder { w: W::new(), body_start: 0, max_record: 0 };
            b.standard_header();
            b
        }

        fn standard_header(&mut self) {
            self.w.u16(2).u16(9).u16(0x0300).u32(0).u16(4).u32(0).u16(0);
        }

        /// A record with 16-bit parameters.
        pub fn rec(&mut self, func: u16, params: &[i16]) -> &mut WmfBuilder {
            let mut bytes = W::new();
            for p in params {
                bytes.i16(*p);
            }
            self.raw(func, &bytes.b)
        }

        pub fn raw(&mut self, func: u16, params: &[u8]) -> &mut WmfBuilder {
            let mut p = params.to_vec();
            if p.len() % 2 == 1 {
                p.push(0);
            }
            let words = 3 + p.len() as u32 / 2;
            self.max_record = self.max_record.max(words);
            self.w.u32(words).u16(func).bytes(&p);
            self
        }

        pub fn build(mut self) -> Vec<u8> {
            self.raw(0, &[]);
            self.w.b
        }

        pub fn body_start(&self) -> usize {
            self.body_start
        }
    }

    fn colorref(c: u32) -> [i16; 2] {
        [(c & 0xFFFF) as u16 as i16, (c >> 16) as u16 as i16]
    }

    fn simple() -> WmfBuilder {
        // 100 x 50 units in a 100 per inch picture: 96 x 48 pixels.
        let mut b = WmfBuilder::placeable([0, 0, 100, 50], 100);
        b.rec(0x020C, &[50, 100]).rec(0x020B, &[0, 0]);
        b
    }

    #[test]
    fn placeable_header_gives_view_box_and_size() {
        let svg = wmf_to_svg(&simple().build()).unwrap();
        assert!(svg.contains("width=\"96\" height=\"48\" viewBox=\"0 0 100 50\""), "{svg}");
    }

    #[test]
    fn plain_header_without_window_uses_drawn_extent() {
        let mut b = WmfBuilder::plain();
        b.rec(0x041B, &[40, 60, 10, 20]); // bottom, right, top, left
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("viewBox=\"20 10 40 30\""), "{svg}");
    }

    #[test]
    fn rectangle_arguments_are_bottom_right_top_left() {
        let mut b = simple();
        b.rec(0x041B, &[40, 60, 10, 20]);
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("d=\"M20 10 L60 10 L60 40 L20 40 Z\""), "{svg}");
    }

    #[test]
    fn move_to_and_line_to_take_y_first() {
        let mut b = simple();
        b.rec(0x0214, &[10, 5]).rec(0x0213, &[30, 70]); // MoveTo(x=5,y=10), LineTo(x=70,y=30)
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("d=\"M5 10 L70 30\""), "{svg}");
    }

    #[test]
    fn objects_use_the_lowest_free_index() {
        let mut b = simple();
        let red = colorref(0x0000FF);
        let blue = colorref(0xFF0000);
        b.rec(0x02FA, &[0, 1, 1, red[0], red[1]]); // pen 0: red
        b.rec(0x02FC, &[0, blue[0], blue[1], 0]); // brush 1: solid blue
        b.rec(0x012D, &[0]).rec(0x012D, &[1]);
        b.rec(0x01F0, &[0]); // delete the pen
        let green = colorref(0x00FF00);
        b.rec(0x02FA, &[0, 1, 1, green[0], green[1]]); // new pen takes index 0
        b.rec(0x012D, &[0]);
        b.rec(0x041B, &[10, 10, 0, 0]);
        let svg = wmf_to_svg(&b.build()).unwrap();
        let p = element(&svg, "path").unwrap();
        assert!(p.contains("stroke=\"#00ff00\"") && p.contains("fill=\"#0000ff\""), "{p}");
    }

    #[test]
    fn a_flood_of_object_records_is_quick_and_bounded() {
        // 300,000 brush creations used to cost a scan each (billions of probes); the table is capped.
        let mut b = simple();
        for _ in 0..300_000 {
            b.rec(0x02FC, &[0, 0, 0, 0]);
        }
        b.rec(0x041B, &[10, 10, 0, 0]);
        let start = std::time::Instant::now();
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(start.elapsed().as_secs() < 5, "took {:?}", start.elapsed());
        assert!(svg.contains("<path"), "{svg}");
    }

    #[test]
    fn deleted_slots_are_reused_lowest_first() {
        let mut g = Gdi::new(1.0, 1.0);
        for _ in 0..5 {
            g.add_object(Object::Other);
        }
        g.delete_object(3);
        g.delete_object(1);
        g.add_object(Object::Other);
        g.add_object(Object::Other);
        g.add_object(Object::Other);
        let mut keys: Vec<u32> = g.objects.keys().copied().collect();
        keys.sort();
        assert_eq!(keys, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn polygon_polyline_and_polypolygon() {
        let mut b = simple();
        b.rec(0x0324, &[3, 0, 0, 10, 0, 5, 8]);
        b.rec(0x0325, &[2, 20, 20, 30, 25]);
        b.rec(0x0538, &[2, 3, 3, 40, 0, 50, 0, 45, 5, 60, 0, 70, 0, 65, 5]);
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("d=\"M0 0 L10 0 L5 8 Z\""), "{svg}");
        assert!(svg.contains("d=\"M20 20 L30 25\""), "{svg}");
        assert!(svg.contains("M40 0 L50 0 L45 5 Z M60 0 L70 0 L65 5 Z"), "{svg}");
    }

    #[test]
    fn ellipse_round_rect_and_arcs_use_wmf_order() {
        let mut b = simple();
        b.rec(0x0418, &[10, 20, 0, 0]);
        b.rec(0x061C, &[4, 6, 20, 40, 0, 0]); // corner height 4, width 6, then bottom right top left
        b.rec(0x0817, &[0, 10, 5, 20, 10, 20, 0, 0]); // ARC: yEnd xEnd yStart xStart then the box
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("M0 5 A10 5 0 1 1 20 5 A10 5 0 1 1 0 5 Z"), "{svg}");
        assert!(svg.contains("M3 0 L37 0 A3 2 0 0 1 40 2"), "{svg}");
        assert!(svg.contains("M20 5 A10 5 0 0 0 10 0"), "{svg}");
    }

    #[test]
    fn negative_window_extent_flips_the_picture() {
        let mut b = WmfBuilder::placeable([0, 0, 100, 100], 96);
        b.rec(0x020C, &[-100, 100]).rec(0x020B, &[100, 0]); // y runs up from the bottom
        b.rec(0x0214, &[0, 0]).rec(0x0213, &[100, 0]); // a line from (0,0) to (0,100) in logical space
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("viewBox=\"0 0 100 100\""), "{svg}");
        // Logical y 0 maps to the bottom of the view box.
        assert!(svg.contains("matrix(1 0 0 -1 0 100)"), "{svg}");
    }

    #[test]
    fn text_out_and_ext_text_out() {
        let mut b = simple();
        // Font: height -12, bold, face "Times New Roman" (roman family).
        let mut f = W::new();
        f.i16(-12).i16(0).i16(0).i16(0).i16(700);
        f.u8(0).u8(0).u8(0).u8(0).u8(0).u8(0).u8(0).u8(0x10);
        f.bytes(b"Times New Roman\0");
        b.raw(0x02FB, &f.b).rec(0x012D, &[0]);
        // TEXTOUT "Hey": length, string (padded), y, x.
        let mut t = W::new();
        t.u16(3).bytes(b"Hey\0").i16(30).i16(12);
        b.raw(0x0521, &t.b);
        // EXTTEXTOUT: y, x, length, options, string, then dx.
        let mut e = W::new();
        e.i16(40).i16(8).u16(2).u16(0).bytes(b"AB").i16(6).i16(6);
        b.raw(0x0A32, &e.b);
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains(">Hey</text>") && svg.contains("translate(12 30)"), "{svg}");
        assert!(svg.contains(">AB</text>") && svg.contains("translate(8 40)"), "{svg}");
        assert!(svg.contains("font-family=\"'Times New Roman', serif\"") && svg.contains("font-weight=\"bold\""), "{svg}");
        assert!(svg.contains("font-size=\"12\""), "{svg}");
    }

    #[test]
    fn bitmaps_from_stretch_dib_and_dib_bit_blt() {
        let dib = crate::imgconv::dib::tests::make_dib(2, 2, 24, &[], &[0x20; 16]);
        let mut b = simple();
        // STRETCHDIB: rop, usage, srcH, srcW, ySrc, xSrc, destH, destW, yDest, xDest, DIB.
        let mut s = W::new();
        s.u32(0x00CC0020).u16(0).i16(2).i16(2).i16(0).i16(0).i16(20).i16(30).i16(5).i16(10).bytes(&dib);
        b.raw(0x0F43, &s.b);
        // DIBBITBLT: rop, ySrc, xSrc, height, width, yDest, xDest, DIB.
        let mut d = W::new();
        d.u32(0x00CC0020).i16(0).i16(0).i16(2).i16(2).i16(25).i16(40).bytes(&dib);
        b.raw(0x0940, &d.b);
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("translate(10 5) scale(15 10)"), "{svg}");
        assert!(svg.contains("translate(40 25) scale(1 1)"), "{svg}");
        assert_eq!(svg.matches("href=\"data:image/bmp;base64,").count(), 2);
    }

    #[test]
    fn dib_bit_blt_without_a_bitmap_fills_with_the_brush() {
        let mut b = simple();
        let red = colorref(0x0000FF);
        b.rec(0x02FC, &[0, red[0], red[1], 0]).rec(0x012D, &[0]);
        let mut d = W::new();
        d.u32(0x00F00021).i16(0).i16(0).i16(10).i16(20).i16(5).i16(3).i16(0);
        b.raw(0x0940, &d.b);
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("M3 5 L23 5 L23 15 L3 15 Z") && svg.contains("fill=\"#ff0000\""), "{svg}");
    }

    #[test]
    fn clip_rect_and_save_restore() {
        let mut b = simple();
        b.rec(0x001E, &[]);
        b.rec(0x0416, &[40, 60, 10, 20]);
        b.rec(0x041B, &[5, 5, 0, 0]);
        b.rec(0x0127, &[-1]);
        b.rec(0x041B, &[9, 9, 4, 4]);
        let svg = wmf_to_svg(&b.build()).unwrap();
        assert!(svg.contains("<clipPath id=\"c1\"><path d=\"M20 10 L60 10 L60 40 L20 40 Z\""), "{svg}");
        // The second rectangle comes after the restore, outside the clip group.
        let after = svg.split("</g>").last().unwrap();
        assert!(after.contains("M4 4"), "{svg}");
    }

    #[test]
    fn bad_headers_are_rejected_and_unknown_records_skipped() {
        assert!(wmf_to_svg(&[]).is_err());
        assert!(wmf_to_svg(b"not a metafile at all").is_err());
        let mut b = simple();
        b.rec(0x7777, &[1, 2, 3]).rec(0x041B, &[5, 5, 0, 0]);
        assert!(wmf_to_svg(&b.build()).unwrap().contains("L5 0"));
    }

    fn rich() -> Vec<u8> {
        let mut b = simple();
        let red = colorref(0xFF);
        b.rec(0x02FA, &[0, 2, 2, red[0], red[1]]).rec(0x02FC, &[0, red[0], red[1], 0]);
        b.rec(0x012D, &[0]).rec(0x012D, &[1]);
        b.rec(0x0324, &[3, 0, 0, 10, 0, 5, 8]);
        b.rec(0x0538, &[2, 3, 3, 40, 0, 50, 0, 45, 5, 60, 0, 70, 0, 65, 5]);
        b.rec(0x0416, &[40, 60, 10, 20]);
        b.rec(0x0817, &[0, 10, 5, 20, 10, 20, 0, 0]);
        let mut f = W::new();
        f.i16(-12).i16(0).i16(0).i16(0).i16(400).bytes(&[0; 8]).bytes(b"Arial\0\0");
        b.raw(0x02FB, &f.b).rec(0x012D, &[2]);
        let mut t = W::new();
        t.u16(3).bytes(b"Hey\0").i16(30).i16(12);
        b.raw(0x0521, &t.b);
        let dib = crate::imgconv::dib::tests::make_dib(2, 2, 24, &[], &[0x20; 16]);
        let mut s = W::new();
        s.u32(0x00CC0020).u16(0).i16(2).i16(2).i16(0).i16(0).i16(20).i16(30).i16(5).i16(10).bytes(&dib);
        b.raw(0x0F43, &s.b);
        b.build()
    }

    #[test]
    fn every_prefix_and_corruption_is_safe() {
        let full = rich();
        let svg = wmf_to_svg(&full).unwrap();
        assert!(svg.contains("<image") && svg.contains("<text") && svg.contains("clip-path"), "{svg}");
        for n in 0..full.len() {
            let _ = wmf_to_svg(&full[..n]);
        }
        for i in 0..full.len() {
            let mut g = full.clone();
            g[i] ^= 0xFF;
            let _ = wmf_to_svg(&g);
            let mut g = full.clone();
            g[i] = 0x7F;
            let _ = wmf_to_svg(&g);
        }
        let b = WmfBuilder::plain();
        let start = b.body_start();
        let mut junk = b.build();
        junk.truncate(18 + start);
        junk.extend((0..3000u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8));
        let _ = wmf_to_svg(&junk);
    }
}
