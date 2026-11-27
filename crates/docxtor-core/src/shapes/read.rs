//! Reading shapes into the view model: DrawingML (`wps:wsp`, `wpg:wgp`, `wpc:wpc`) and VML-only shapes.

use super::theme::ShapeTheme;
use super::*;
use crate::image::ImageLayout;
use crate::marks::{length_pt, parse_opacity, style_prop};
use crate::render::RenderCtx;
use std::cell::Cell;

/// The markup a shape is read from.
pub enum Src<'a> {
    /// A `w:drawing` holding a DrawingML shape, group or canvas.
    Drawing(&'a Element),
    /// A `w:pict` holding VML shapes.
    Vml(&'a Element),
}

/// Where the shape's markup is, if the object is a shape this module reads.
pub fn primary(e: &Element) -> Option<Src<'_>> {
    match e.name.as_str() {
        "mc:AlternateContent" => e.children_named("mc:Choice").find_map(primary_in_choice),
        "w:drawing" => drawing_is_shape(e).then_some(Src::Drawing(e)),
        "w:pict" => vml_is_shape(e).then_some(Src::Vml(e)),
        _ => None,
    }
}

pub fn primary_in_choice(choice: &Element) -> Option<Src<'_>> {
    if let Some(d) = choice.child("w:drawing").filter(|d| drawing_is_shape(d)) {
        return Some(Src::Drawing(d));
    }
    choice.child("w:pict").filter(|p| vml_is_shape(p)).map(Src::Vml)
}

fn drawing_is_shape(d: &Element) -> bool {
    super::smartart::is_diagram(d) || d.find("wps:wsp").is_some() || d.find("wpg:wgp").is_some() || d.find("wpc:wpc").is_some()
}

const VML_SHAPES: &[&str] = &["v:rect", "v:roundrect", "v:oval", "v:line", "v:shape", "v:group"];

fn vml_is_shape(p: &Element) -> bool {
    if crate::marks::is_watermark_pict(p) {
        return false;
    }
    let mut any = false;
    for s in p.elements().filter(|c| VML_SHAPES.contains(&c.name.as_str())) {
        // Pictures and horizontal rules are other kinds of object (WordArt is a shape with a warp).
        let picture = s.find("v:imagedata").is_some() && s.find("v:textbox").is_none();
        if picture || s.attr("o:hr").is_some() {
            return false;
        }
        any = true;
    }
    any
}

impl RenderCtx<'_> {
    /// The shape an object element is, with the text of its text boxes.
    pub fn shape(&self, e: &Element) -> Option<RShape> {
        let src = primary(e)?;
        let rd = Rd { cx: self, th: self.shapes.theme(self.pkg), keys: super::story::keys_of(e), next: Cell::new(0) };
        match src {
            Src::Drawing(d) => rd.drawing(d),
            Src::Vml(p) => rd.vml(p),
        }
    }
}

pub(super) struct Rd<'c, 'a> {
    pub(super) cx: &'c RenderCtx<'a>,
    pub(super) th: &'c ShapeTheme,
    keys: Vec<String>,
    /// Number of `w:txbxContent` met so far: picks the story of the next text box.
    next: Cell<usize>,
}

pub(super) fn emu(e: Option<&Element>, a: &str) -> f64 {
    e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

pub(super) fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on" | "t"))
}

impl Rd<'_, '_> {
    /// The blocks of the next text box met, rendered.
    fn take_text(&self, has_content: bool) -> Option<Vec<RBlock>> {
        if !has_content {
            return None;
        }
        let i = self.next.get();
        self.next.set(i + 1);
        let key = self.keys.get(i)?;
        let blocks = self.cx.shapes.texts.get(key)?;
        Some(self.cx.render_story_blocks(blocks))
    }

    // ---- DrawingML ---------------------------------------------------------

    fn drawing(&self, d: &Element) -> Option<RShape> {
        let c = d.child("wp:anchor").or_else(|| d.child("wp:inline"))?;
        let ext = c.child("wp:extent");
        let (w, h) = (emu(ext, "cx") / EMU_PER_PT, emu(ext, "cy") / EMU_PER_PT);
        let content = c.find("a:graphicData")?.elements().next()?;
        let mut sh = match content.name.as_str() {
            "wps:wsp" => self.wsp(content, w, h, 0),
            "wpg:wgp" => self.group(content, w, h, 0),
            "wpc:wpc" => self.canvas(content, w, h),
            // SmartArt: drawn from the drawing Word saves with it (see `smartart`).
            "dgm:relIds" => self.diagram(c, content, w, h),
            _ => return None,
        };
        let pr = c.child("wp:docPr");
        sh.id = pr.and_then(|p| p.attr("id")).and_then(|v| v.parse().ok()).unwrap_or(0);
        sh.name = pr.and_then(|p| p.attr("name")).filter(|s| !s.is_empty()).map(String::from);
        sh.alt = pr.and_then(|p| p.attr("descr")).filter(|s| !s.is_empty()).map(String::from);
        sh.layout = crate::image::layout(d);
        Some(sh)
    }

    pub(super) fn base(&self, w: f64, h: f64) -> RShape {
        RShape { kind: "shape", source: "drawingml", geometry: "rect".into(), w, h, insets: RInsets::default(), v_anchor: "top", wrap_text: true, layout: ImageLayout { wrap: "inline".into(), ..Default::default() }, ..Default::default() }
    }

    fn wsp(&self, wsp: &Element, w: f64, h: f64, depth: usize) -> RShape {
        let mut sh = self.base(w, h);
        let sppr = wsp.child("wps:spPr");
        let style = wsp.child("wps:style");
        if let Some(x) = sppr.and_then(|s| s.child("a:xfrm")) {
            sh.rot = emu(Some(x), "rot") / 60000.0;
            sh.flip_h = truthy(x.attr("flipH"));
            sh.flip_v = truthy(x.attr("flipV"));
        }
        if let Some(sp) = sppr {
            self.geometry(sp, &mut sh);
            sh.fill = self.fill_of(sp, style);
            sh.line = self.line_of(sp, style);
        } else {
            sh.fill = self.fill_of(&Element::new("x"), style);
            sh.line = self.line_of(&Element::new("x"), style);
        }
        sh.text_box = truthy(wsp.child("wps:cNvSpPr").and_then(|c| c.attr("txBox")));
        if sh.geometry == "line" || sh.geometry.contains("Connector") {
            sh.text_box = false;
        }
        sh.connect = super::connector::connect_of(wsp);
        if let Some(b) = wsp.child("wps:bodyPr") {
            super::body::read(b, &mut sh);
        }
        sh.effects = super::effects::read(self.th, sppr, style);
        sh.warp = wsp.child("wps:bodyPr").and_then(|b| self.warp_dml(b, wsp.child("wps:txbx")));
        if let Some(tx) = wsp.child("wps:txbx") {
            let has = tx.child("w:txbxContent").is_some();
            sh.text = self.take_text(has);
        }
        let _ = depth;
        sh
    }

    pub(super) fn geometry(&self, sp: &Element, sh: &mut RShape) {
        if let Some(p) = sp.child("a:prstGeom") {
            sh.geometry = p.attr("prst").unwrap_or("rect").to_string();
            if let Some(av) = p.child("a:avLst") {
                for g in av.children_named("a:gd") {
                    let v = g.attr("fmla").and_then(|f| f.strip_prefix("val ")).and_then(|v| v.trim().parse::<f64>().ok());
                    if let (Some(n), Some(v)) = (g.attr("name"), v) {
                        sh.adj.insert(n.to_string(), v);
                    }
                }
            }
        } else if let Some(cg) = sp.child("a:custGeom") {
            sh.geometry = "custom".into();
            sh.custom = Some(self.custom(cg, sh.w, sh.h));
        }
    }

    /// The fill of a shape: its own, else the one its style refers to.
    pub(super) fn fill_of(&self, sp: &Element, style: Option<&Element>) -> Option<RFill> {
        if let Some(f) = self.fill_children(sp) {
            return f;
        }
        let r = style?.child("a:fillRef")?;
        if r.attr("idx").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) == 0 {
            return None;
        }
        let (color, alpha) = self.th.color_of(r, None)?;
        let ph = (color.clone(), alpha);
        let idx = r.attr("idx").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        if let Some(f) = self.th.fill_style(idx).and_then(|st| super::fill::read(self.th, st, Some(&ph), &|rid| self.cx.rel_src(rid))) {
            return Some(f);
        }
        Some(RFill::solid(color, alpha))
    }

    /// `Some(None)` when the element says no fill, `None` when it says nothing about fill.
    fn fill_children(&self, parent: &Element) -> Option<Option<RFill>> {
        for c in parent.elements() {
            match c.name.as_str() {
                "a:noFill" => return Some(None),
                // A group fill takes the group's: not drawn here.
                "a:grpFill" => return Some(None),
                "a:solidFill" | "a:gradFill" | "a:blipFill" | "a:pattFill" => {
                    return Some(super::fill::read(self.th, c, None, &|rid| self.cx.rel_src(rid)));
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn line_of(&self, sp: &Element, style: Option<&Element>) -> Option<RLine> {
        let ln = sp.child("a:ln");
        let lref = style.and_then(|s| s.child("a:lnRef"));
        let idx = lref.and_then(|r| r.attr("idx")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        let style_color = if idx > 0 { lref.and_then(|r| self.th.color_of(r, None)) } else { None };
        let style_width = if idx > 0 { Some(self.th.line_width(idx)) } else { None };
        let Some(ln) = ln else {
            let (color, alpha) = style_color?;
            return Some(RLine { color, alpha, width_pt: style_width.unwrap_or(0.75), dash: "solid".into(), head_arrow: None, tail_arrow: None, cap: None, join: None, compound: None, head_w: None, head_len: None, tail_w: None, tail_len: None, miter: None });
        };
        let (color, alpha) = match self.fill_children(ln) {
            Some(None) => return None,
            Some(Some(f)) => (f.color, f.alpha),
            None => style_color?,
        };
        let width_pt = ln.attr("w").and_then(|v| v.parse::<f64>().ok()).map(|v| v / EMU_PER_PT).or(style_width).unwrap_or(0.75);
        let dash = ln.child("a:prstDash").and_then(|d| d.attr("val")).map(String::from).or_else(|| ln.child("a:custDash").map(|_| "dash".to_string())).unwrap_or_else(|| "solid".into());
        let arrow = |n: &str| ln.child(n).and_then(|e| e.attr("type")).filter(|t| *t != "none").map(String::from);
        let cap = match ln.attr("cap") {
            Some("rnd") => Some("round"),
            Some("sq") => Some("square"),
            _ => None,
        };
        let join = if ln.child("a:round").is_some() {
            Some("round")
        } else if ln.child("a:bevel").is_some() {
            Some("bevel")
        } else {
            None
        };
        let size = |n: &str, a: &str| -> Option<&'static str> {
            match ln.child(n).and_then(|e| e.attr(a)) {
                Some("sm") => Some("sm"),
                Some("lg") => Some("lg"),
                _ => None,
            }
        };
        let compound = ln.attr("cmpd").filter(|c| matches!(*c, "dbl" | "thickThin" | "thinThick" | "tri")).map(String::from);
        let miter = ln.child("a:miter").and_then(|m| m.attr("lim")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 1000.0).filter(|v| (v - 800.0).abs() > 1e-6);
        Some(RLine {
            color,
            alpha,
            width_pt,
            dash,
            head_arrow: arrow("a:headEnd"),
            tail_arrow: arrow("a:tailEnd"),
            cap,
            join,
            compound,
            head_w: size("a:headEnd", "w"),
            head_len: size("a:headEnd", "len"),
            tail_w: size("a:tailEnd", "w"),
            tail_len: size("a:tailEnd", "len"),
            miter,
        })
    }

    fn group(&self, g: &Element, w: f64, h: f64, depth: usize) -> RShape {
        let mut sh = self.base(w, h);
        sh.kind = "group";
        sh.geometry = "group".into();
        let x = g.child("wpg:grpSpPr").and_then(|p| p.child("a:xfrm"));
        if let Some(x) = x {
            sh.rot = emu(Some(x), "rot") / 60000.0;
            sh.flip_h = truthy(x.attr("flipH"));
            sh.flip_v = truthy(x.attr("flipV"));
        }
        let ch_off = x.and_then(|x| x.child("a:chOff"));
        let ch_ext = x.and_then(|x| x.child("a:chExt"));
        let (cox, coy) = (emu(ch_off, "x"), emu(ch_off, "y"));
        let (cew, ceh) = (emu(ch_ext, "cx"), emu(ch_ext, "cy"));
        let sx = if cew > 0.0 { w * EMU_PER_PT / cew } else { 1.0 };
        let sy = if ceh > 0.0 { h * EMU_PER_PT / ceh } else { 1.0 };
        if depth < 6 {
            for c in g.elements() {
                if let Some(child) = self.member(c, depth + 1, &|ox, oy, cw, chh| ((ox - cox) * sx / EMU_PER_PT, (oy - coy) * sy / EMU_PER_PT, cw * sx / EMU_PER_PT, chh * sy / EMU_PER_PT)) {
                    sh.children.push(child);
                }
            }
        }
        sh
    }

    fn canvas(&self, c: &Element, w: f64, h: f64) -> RShape {
        let mut sh = self.base(w, h);
        sh.kind = "canvas";
        sh.geometry = "rect".into();
        if let Some(bg) = c.child("wpc:bg") {
            if let Some(f) = self.fill_children(bg) {
                sh.fill = f;
            }
        }
        if let Some(whole) = c.child("wpc:whole") {
            let holder = Element::new("x").with_child(whole.child("a:ln").cloned().unwrap_or_else(|| Element::new("a:ln")));
            sh.line = self.line_of(&holder, None);
            if whole.child("a:ln").is_none() {
                sh.line = None;
            }
        }
        for m in c.elements() {
            if let Some(child) = self.member(m, 1, &|ox, oy, cw, chh| (ox / EMU_PER_PT, oy / EMU_PER_PT, cw / EMU_PER_PT, chh / EMU_PER_PT)) {
                sh.children.push(child);
            }
        }
        sh
    }

    /// A member of a group or canvas. `place` maps its offset and size in the parent's coordinates (EMU) to
    /// points inside the parent's box.
    fn member(&self, m: &Element, depth: usize, place: &dyn Fn(f64, f64, f64, f64) -> (f64, f64, f64, f64)) -> Option<RShape> {
        let (xfrm, kind) = match m.name.as_str() {
            "wps:wsp" => (m.child("wps:spPr")?.child("a:xfrm"), 0),
            "wpg:grpSp" | "wpg:wgp" => (m.child("wpg:grpSpPr")?.child("a:xfrm"), 1),
            "pic:pic" => (m.child("pic:spPr")?.child("a:xfrm"), 2),
            _ => return None,
        };
        let (off, ext) = (xfrm.and_then(|x| x.child("a:off")), xfrm.and_then(|x| x.child("a:ext")));
        let (x, y, w, h) = place(emu(off, "x"), emu(off, "y"), emu(ext, "cx"), emu(ext, "cy"));
        let mut sh = match kind {
            0 => self.wsp(m, w, h, depth),
            1 => self.group(m, w, h, depth),
            _ => {
                let mut sh = self.base(w, h);
                sh.kind = "picture";
                sh.geometry = "rect".into();
                if let Some(x) = xfrm {
                    sh.rot = emu(Some(x), "rot") / 60000.0;
                    sh.flip_h = truthy(x.attr("flipH"));
                    sh.flip_v = truthy(x.attr("flipV"));
                }
                sh.src = m.find("a:blip").and_then(|b| b.attr("r:embed")).and_then(|rid| self.cx.rel_src(rid));
                sh
            }
        };
        let pr = m.find("wps:cNvPr").or_else(|| m.find("pic:cNvPr")).or_else(|| m.find("wpg:cNvPr"));
        sh.id = pr.and_then(|p| p.attr("id")).and_then(|v| v.parse().ok()).unwrap_or(0);
        sh.name = pr.and_then(|p| p.attr("name")).filter(|s| !s.is_empty()).map(String::from);
        sh.child_x = Some(x);
        sh.child_y = Some(y);
        Some(sh)
    }

    // ---- custom geometry ---------------------------------------------------

    fn custom(&self, cg: &Element, w: f64, h: f64) -> RCustom {
        let mut paths = Vec::new();
        for p in cg.child("a:pathLst").map(|l| l.children_named("a:path").collect::<Vec<_>>()).unwrap_or_default() {
            let pw = emu(Some(p), "w");
            let ph = emu(Some(p), "h");
            let (pw, ph) = (if pw > 0.0 { pw } else { w * EMU_PER_PT }, if ph > 0.0 { ph } else { h * EMU_PER_PT });
            let coord = |e: &Element, a: &str| -> f64 {
                match e.attr(a) {
                    Some(v) => v.parse::<f64>().unwrap_or_else(|_| match v {
                        "r" | "w" => pw,
                        "b" | "h" => ph,
                        "hc" | "wd2" => pw / 2.0,
                        "vc" | "hd2" => ph / 2.0,
                        "wd4" => pw / 4.0,
                        "hd4" => ph / 4.0,
                        _ => 0.0,
                    }),
                    None => 0.0,
                }
            };
            let mut d = String::new();
            let mut cmds: Vec<RCmd> = Vec::new();
            let mut cur = (0.0f64, 0.0f64);
            let pts =|c: &Element| -> Vec<(f64, f64)> { c.children_named("a:pt").map(|t| (coord(t, "x"), coord(t, "y"))).collect() };
            for c in p.elements() {
                match c.name.as_str() {
                    "a:moveTo" => {
                        if let Some(&(x, y)) = pts(c).first() {
                            d.push_str(&format!("M{} {} ", n(x), n(y)));
                            cmds.push(RCmd::new("M", &[(x, y)]));
                            cur = (x, y);
                        }
                    }
                    "a:lnTo" => {
                        if let Some(&(x, y)) = pts(c).first() {
                            d.push_str(&format!("L{} {} ", n(x), n(y)));
                            cmds.push(RCmd::new("L", &[(x, y)]));
                            cur = (x, y);
                        }
                    }
                    "a:cubicBezTo" => {
                        let v = pts(c);
                        if v.len() == 3 {
                            d.push_str(&format!("C{} {} {} {} {} {} ", n(v[0].0), n(v[0].1), n(v[1].0), n(v[1].1), n(v[2].0), n(v[2].1)));
                            cmds.push(RCmd::new("C", &v));
                            cur = v[2];
                        }
                    }
                    "a:quadBezTo" => {
                        let v = pts(c);
                        if v.len() == 2 {
                            d.push_str(&format!("Q{} {} {} {} ", n(v[0].0), n(v[0].1), n(v[1].0), n(v[1].1)));
                            // A quadratic curve is a cubic one with its handles two thirds of the way.
                            let third = |a: (f64, f64), b: (f64, f64)| (a.0 + 2.0 / 3.0 * (b.0 - a.0), a.1 + 2.0 / 3.0 * (b.1 - a.1));
                            cmds.push(RCmd::new("C", &[third(cur, v[0]), third(v[1], v[0]), v[1]]));
                            cur = v[1];
                        }
                    }
                    "a:arcTo" => {
                        let (wr, hr) = (coord(c, "wR"), coord(c, "hR"));
                        let (st, sw) = (coord(c, "stAng") / 60000.0, coord(c, "swAng") / 60000.0);
                        if wr > 0.0 && hr > 0.0 {
                            for b in super::freeform::arc_beziers(cur, wr, hr, st, sw) {
                                cmds.push(RCmd::new("C", &b));
                            }
                            cur = arc(&mut d, cur, wr, hr, st, sw);
                        }
                    }
                    "a:close" => {
                        d.push_str("Z ");
                        cmds.push(RCmd::new("Z", &[]));
                    }
                    _ => {}
                }
            }
            paths.push(RPath {
                w: pw,
                h: ph,
                d: d.trim_end().to_string(),
                cmds,
                fill: p.attr("fill") != Some("none"),
                stroke: !matches!(p.attr("stroke"), Some("0" | "false")),
            });
        }
        RCustom { paths }
    }

    // ---- VML ---------------------------------------------------------------

    fn vml(&self, pict: &Element) -> Option<RShape> {
        let shapes: Vec<&Element> = pict.elements().filter(|c| VML_SHAPES.contains(&c.name.as_str())).collect();
        let types: Vec<(&str, i64)> = pict
            .children_named("v:shapetype")
            .filter_map(|t| Some((t.attr("id")?, t.attr("o:spt")?.parse().ok()?)))
            .collect();
        match shapes.len() {
            0 => None,
            1 => Some(self.vml_shape(shapes[0], &types, 0)),
            _ => {
                let mut group = self.base(0.0, 0.0);
                group.source = "vml";
                group.kind = "group";
                group.geometry = "group".into();
                let (mut x1, mut y1) = (f64::MAX, f64::MAX);
                let (mut x2, mut y2) = (0.0f64, 0.0f64);
                for s in shapes {
                    let mut child = self.vml_shape(s, &types, 1);
                    let (x, y) = (child.layout.x.unwrap_or(0.0), child.layout.y.unwrap_or(0.0));
                    x1 = x1.min(x);
                    y1 = y1.min(y);
                    x2 = x2.max(x + child.w);
                    y2 = y2.max(y + child.h);
                    child.child_x = Some(x);
                    child.child_y = Some(y);
                    group.children.push(child);
                }
                for c in &mut group.children {
                    c.child_x = c.child_x.map(|v| v - x1);
                    c.child_y = c.child_y.map(|v| v - y1);
                    c.layout = ImageLayout { wrap: "inline".into(), ..Default::default() };
                }
                group.w = x2 - x1;
                group.h = y2 - y1;
                group.layout = ImageLayout { wrap: "front".into(), x: Some(x1), y: Some(y1), ..Default::default() };
                Some(group)
            }
        }
    }

    fn vml_shape(&self, el: &Element, types: &[(&str, i64)], depth: usize) -> RShape {
        let style = el.attr("style").unwrap_or("");
        let prop = |k: &str| style_prop(style, k);
        let len = |k: &str| prop(k).and_then(length_pt);
        let mut sh = self.base(0.0, 0.0);
        sh.source = "vml";
        let spt = el.attr("type").and_then(|t| types.iter().find(|(id, _)| t.trim_start_matches('#') == *id).map(|(_, n)| *n)).or_else(|| el.attr("o:spt").and_then(|v| v.parse().ok()));
        sh.geometry = match el.name.as_str() {
            "v:rect" => "rect".into(),
            "v:roundrect" => {
                let arc = el.attr("arcsize").map(|a| if let Some(f) = a.strip_suffix('f') { f.parse::<f64>().unwrap_or(10923.0) / 65536.0 } else { a.trim_end_matches('%').parse::<f64>().unwrap_or(16.667) / if a.ends_with('%') { 100.0 } else { 1.0 } }).unwrap_or(0.16667);
                sh.adj.insert("adj".into(), (arc * 100000.0).round());
                "roundRect".into()
            }
            "v:oval" => "ellipse".into(),
            "v:line" => "line".into(),
            "v:group" => "group".into(),
            _ => vml_preset(spt.or_else(|| el.attr("type").and_then(vml_builtin_type))).into(),
        };
        sh.text_box = spt == Some(202) || el.attr("type").is_some_and(|t| t.contains("_x0000_t202"));
        // Size and position.
        let (mut x, mut y) = (len("margin-left").or(len("left")), len("margin-top").or(len("top")));
        if el.name == "v:line" {
            let pt = |a: &str| -> (f64, f64) {
                let v: Vec<f64> = el.attr(a).unwrap_or("0,0").split(',').map(|s| length_pt(s).unwrap_or(0.0)).collect();
                (v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0))
            };
            let (f, t) = (pt("from"), pt("to"));
            sh.w = (t.0 - f.0).abs();
            sh.h = (t.1 - f.1).abs();
            sh.flip_h = t.0 < f.0;
            sh.flip_v = t.1 < f.1;
            x = x.or(Some(f.0.min(t.0)));
            y = y.or(Some(f.1.min(t.1)));
        } else {
            sh.w = len("width").unwrap_or(0.0);
            sh.h = len("height").unwrap_or(0.0);
        }
        sh.rot = prop("rotation").and_then(|v| v.trim().trim_end_matches("deg").parse::<f64>().ok()).unwrap_or(0.0);
        if let Some(f) = prop("flip") {
            sh.flip_h |= f.contains('x');
            sh.flip_v |= f.contains('y');
        }
        let absolute = prop("position") == Some("absolute");
        if absolute && depth == 0 {
            let z = prop("z-index").and_then(|v| v.parse::<i64>().ok()).unwrap_or(1);
            let wrap = match el.child("w10:wrap").and_then(|w| w.attr("type")) {
                Some("square") => "square",
                Some("tight") | Some("through") => "tight",
                Some("topAndBottom") => "topBottom",
                _ if z < 0 => "behind",
                _ => "front",
            };
            let rel = |v: Option<&str>| {
                v.map(|r| match r {
                    "text" => "column".to_string(),
                    "char" => "character".to_string(),
                    other => other.to_string(),
                })
            };
            let align = prop("mso-position-horizontal").filter(|a| matches!(*a, "left" | "center" | "right" | "inside" | "outside")).map(|a| match a {
                "inside" => "left".to_string(),
                "outside" => "right".to_string(),
                other => other.to_string(),
            });
            let rel_y = prop("mso-position-vertical-relative").map(|r| match r {
                "text" => "paragraph".to_string(),
                other => other.to_string(),
            });
            let align_y = prop("mso-position-vertical").filter(|a| matches!(*a, "top" | "center" | "bottom")).map(String::from);
            sh.layout = ImageLayout { wrap: wrap.into(), align, x, y, rel_x: rel(prop("mso-position-horizontal-relative")), rel_y, align_y, z: prop("z-index").and_then(|v| v.parse::<i64>().ok()).map(|z| z.unsigned_abs()) };
        } else {
            sh.layout = ImageLayout { wrap: "inline".into(), x, y, ..Default::default() };
        }
        // Fill and outline (VML defaults: white fill, black 0.75pt line).
        let fill_el = el.child("v:fill");
        let filled = el.attr("filled") != Some("f") && el.attr("filled") != Some("false") && fill_el.and_then(|f| f.attr("on")) != Some("f");
        if filled {
            sh.fill = Some(super::fill::read_vml(fill_el, el.attr("fillcolor"), &|rid| self.cx.rel_src(rid), &parse_opacity));
        }
        let stroke_el = el.child("v:stroke");
        let stroked = el.attr("stroked") != Some("f") && el.attr("stroked") != Some("false") && stroke_el.and_then(|s| s.attr("on")) != Some("f");
        if stroked {
            let color = stroke_el.and_then(|s| s.attr("color")).or(el.attr("strokecolor")).and_then(super::hex6).unwrap_or_else(|| "000000".into());
            let width_pt = el.attr("strokeweight").or(stroke_el.and_then(|s| s.attr("weight"))).and_then(length_pt).unwrap_or(0.75);
            let dash = match stroke_el.and_then(|s| s.attr("dashstyle")) {
                None | Some("solid") => "solid",
                Some("dot") | Some("1 1") => "sysDot",
                Some("dash") | Some("shortdash") => "dash",
                Some("longdash") => "lgDash",
                Some("dashdot") => "dashDot",
                Some("longdashdot") => "lgDashDot",
                Some("longdashdotdot") => "lgDashDotDot",
                Some(_) => "dash",
            };
            let arrow = |a: Option<&str>| {
                a.and_then(|a| match a {
                    "block" => Some("triangle"),
                    "classic" => Some("stealth"),
                    "diamond" => Some("diamond"),
                    "oval" => Some("oval"),
                    "open" => Some("arrow"),
                    _ => None,
                })
                .map(String::from)
            };
            let st = |a: &str| stroke_el.and_then(|s| s.attr(a));
            let wide = |a: &str| match st(a) {
                Some("narrow" | "short") => Some("sm"),
                Some("wide" | "long") => Some("lg"),
                _ => None,
            };
            sh.line = Some(RLine {
                color,
                alpha: st("opacity").and_then(parse_opacity).unwrap_or(1.0),
                width_pt,
                dash: dash.into(),
                head_arrow: arrow(st("startarrow")),
                tail_arrow: arrow(st("endarrow")),
                cap: match st("endcap") {
                    Some("round") => Some("round"),
                    Some("square") => Some("square"),
                    _ => None,
                },
                join: match st("joinstyle") {
                    Some("round") => Some("round"),
                    Some("bevel") => Some("bevel"),
                    _ => None,
                },
                compound: match st("linestyle") {
                    Some("thinThin") => Some("dbl".to_string()),
                    Some("thickThin") => Some("thickThin".to_string()),
                    Some("thinThick") => Some("thinThick".to_string()),
                    Some("thickBetweenThin") => Some("tri".to_string()),
                    _ => None,
                },
                head_w: wide("startarrowwidth"),
                head_len: wide("startarrowlength"),
                tail_w: wide("endarrowwidth"),
                tail_len: wide("endarrowlength"),
                miter: st("miterlimit").and_then(|v| v.parse::<f64>().ok()).map(|v| v * 100.0).filter(|v| (v - 800.0).abs() > 1e-6),
            });
        }
        // A shadow: VML's own, with an offset and no blur.
        if let Some(sd) = el.child("v:shadow").filter(|d| matches!(d.attr("on"), Some("t" | "true"))) {
            let off: Vec<f64> = sd.attr("offset").unwrap_or("2pt,2pt").split(',').map(|v| length_pt(v).unwrap_or(0.0)).collect();
            let (dx, dy) = (off.first().copied().unwrap_or(2.0), off.get(1).copied().unwrap_or(2.0));
            sh.effects = Some(super::REffects {
                shadow: Some(super::appearance::RShadow {
                    color: sd.attr("color").and_then(super::hex6).unwrap_or_else(|| "808080".into()),
                    alpha: sd.attr("opacity").and_then(parse_opacity).unwrap_or(1.0),
                    blur_pt: 2.0,
                    dist_pt: (dx * dx + dy * dy).sqrt(),
                    angle: dy.atan2(dx).to_degrees().rem_euclid(360.0),
                    sx: None,
                    sy: None,
                    align: None,
                    rot_with_shape: None,
                }),
                ..Default::default()
            });
        }
        // The text frame.
        if let Some(tb) = el.child("v:textbox") {
            if let Some(inset) = tb.attr("inset") {
                let parts: Vec<Option<f64>> = inset.split(',').map(length_pt).collect();
                let at = |i: usize, d: f64| parts.get(i).copied().flatten().unwrap_or(d);
                sh.insets = RInsets { l: at(0, 7.2), t: at(1, 3.6), r: at(2, 7.2), b: at(3, 3.6) };
            }
            sh.text = self.take_text(tb.child("w:txbxContent").is_some());
        }
        super::body::read_vml(style, el.child("v:textbox"), &mut sh);
        let id = el.attr("id").unwrap_or("");
        if !id.starts_with('_') && !id.is_empty() {
            sh.name = Some(id.to_string());
        }
        sh.id = el.attr("o:spid").or(Some(id)).and_then(|s| s.rsplit('s').next().and_then(|d| d.parse::<u32>().ok())).unwrap_or(0);
        sh.alt = el.attr("alt").filter(|s| !s.is_empty()).map(String::from);
        sh.warp = self.warp_vml(el, spt.or_else(|| el.attr("type").and_then(vml_builtin_type)), &mut sh);
        sh
    }
}

fn n(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// Append an SVG arc for DrawingML's `arcTo` and return the new current point.
fn arc(d: &mut String, cur: (f64, f64), wr: f64, hr: f64, st: f64, sw: f64) -> (f64, f64) {
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
    let point = |t: f64| (cx + wr * t.cos(), cy + hr * t.sin());
    let sweep = if sw > 0.0 { 1 } else { 0 };
    let mut at = cur;
    // An SVG arc cannot span a full turn: split into halves.
    let steps = if delta.abs() > std::f64::consts::PI + 1e-9 { 2 } else { 1 };
    for i in 1..=steps {
        let t = t0 + delta * i as f64 / steps as f64;
        let (x, y) = point(t);
        d.push_str(&format!("A{} {} 0 0 {sweep} {} {} ", n(wr), n(hr), n(x), n(y)));
        at = (x, y);
    }
    at
}

/// Word's VML shape types (`o:spt`) and the presets they stand for.
fn vml_preset(spt: Option<i64>) -> &'static str {
    match spt {
        Some(1) | Some(202) | Some(75) => "rect",
        Some(2) => "roundRect",
        Some(3) => "ellipse",
        Some(4) => "diamond",
        Some(5) => "triangle",
        Some(6) => "rtTriangle",
        Some(7) => "parallelogram",
        Some(8) => "trapezoid",
        Some(9) => "hexagon",
        Some(10) => "octagon",
        Some(11) => "plus",
        Some(12) => "star5",
        Some(13) => "rightArrow",
        Some(15) => "homePlate",
        Some(16) => "cube",
        Some(19) => "can",
        Some(20) => "line",
        Some(56) => "pentagon",
        Some(55) => "chevron",
        Some(66) => "leftArrow",
        Some(67) => "downArrow",
        Some(68) => "upArrow",
        Some(74) => "heart",
        Some(92) => "star5",
        _ => "rect",
    }
}

fn vml_builtin_type(t: &str) -> Option<i64> {
    t.trim_start_matches('#').strip_prefix("_x0000_t")?.parse().ok()
}
