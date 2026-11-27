//! SmartArt graphics (`dgm`).
//!
//! Word saves a SmartArt graphic as four parts (data model, layout, quick style, colours) and a fifth that holds
//! the shapes it laid out: a `dsp:drawing` named by the data model (`dsp:dataModelExt`) and related to the
//! document part. Docxtor draws that drawing with the shape renderer: the graphic becomes an `RShape` of kind
//! `diagram` whose children are the `dsp:sp` shapes (fill, outline, geometry, and the text as read-only
//! `RDText`). Nothing is written back: the parts stay as they were.
//!
//! When the drawing part is missing the shapes are laid out from the data model (see `smartart_layout`), with
//! the colours of the colours part or the theme; and when there is nothing to lay out, a placeholder box names
//! the graphic and lists the texts it holds.
//!
//! A resize changes only `wp:extent`. The drawing is scaled by the ratio of the extent to the size the graphic
//! had when it was first resized, which `remember_size` records the first time in a Docxtor extension of the
//! picture's `wp:docPr` (Word ignores it); the drawing part itself is never touched. The note also holds the extent
//! Docxtor last gave the graphic: when the extent is something else, Word resized it and made a new drawing, and the
//! note is dropped.

use super::read::{emu, truthy, Rd};
use super::smartart_layout as lay;
use super::*;
use crate::xml::Node;
use std::collections::HashMap;

pub const URI_DIAGRAM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
/// The extension that remembers the size a graphic's drawing was made for.
const SIZE_EXT_URI: &str = "{4E1D5F0C-2A7B-4C69-9F3E-6B7A8D0C1F20}";
const SIZE_EXT_NS: &str = "https://docxtor.tinycommit.dev/ns/smartart";

/// What a SmartArt graphic was drawn from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RDiagram {
    /// `drawing` (the drawing Word saved), `layout` (laid out here from the data model) or `placeholder`.
    pub source: &'static str,
    /// The last part of the layout's id (`default`, `process1`, ...) when the file names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
}

/// The text of a shape in a diagram, drawn read only. Positions are points inside the shape's box.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RDText {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub insets: RInsets,
    /// `top`, `middle` or `bottom`.
    pub v_anchor: &'static str,
    /// Lines wrap at the text rectangle's width (false for `wrap="none"`).
    pub wrap: bool,
    /// Degrees; the text turns about the middle of its rectangle.
    #[serde(skip_serializing_if = "is_zero_f")]
    pub rot: f64,
    pub paras: Vec<RDPara>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RDPara {
    /// `left`, `center`, `right` or `justify`.
    pub align: &'static str,
    /// Line spacing as a percentage of the font's own (90 is tight); absent for single.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_pct: Option<f64>,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub before: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub after: f64,
    /// Left margin and the first line's extra indent (negative for a hanging bullet), in points.
    #[serde(skip_serializing_if = "is_zero_f")]
    pub margin: f64,
    #[serde(skip_serializing_if = "is_zero_f")]
    pub indent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullet: Option<String>,
    pub runs: Vec<RDRun>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RDRun {
    pub text: String,
    pub size: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    /// Hex; absent for the text's default (black).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

fn is_zero_f(v: &f64) -> bool {
    *v == 0.0
}

/// Whether a `w:drawing` holds a SmartArt graphic.
pub fn is_diagram(d: &Element) -> bool {
    d.find("a:graphicData").is_some_and(|g| g.attr("uri") == Some(URI_DIAGRAM) || g.child("dgm:relIds").is_some())
}

// ---------------------------------------------------------------------------
// The size a drawing was made for
// ---------------------------------------------------------------------------

/// The note of the size the drawing was made for, in the `wp:docPr` of the `wp:inline` or `wp:anchor` `c`.
fn size_note(c: &Element) -> Option<&Element> {
    c.child("wp:docPr").and_then(|d| d.child("a:extLst")).and_then(|l| l.children_named("a:ext").find(|e| e.attr("uri") == Some(SIZE_EXT_URI))).and_then(|e| e.elements().next())
}

/// The note, if it still describes the drawing. Docxtor writes the extent it gave the graphic into the note
/// (`fx`, `fy`); when the extent is something else, Word (or another program) resized the graphic since, and
/// regenerated the drawing for the new size, so the note is out of date. A note without them (an older file) is trusted.
fn current_note(c: &Element) -> Option<&Element> {
    let note = size_note(c)?;
    let ext = c.child("wp:extent");
    let same = |own: &str, name: &str| match note.attr(own).and_then(|v| v.parse::<f64>().ok()) {
        Some(v) => (v - emu(ext, name)).abs() <= 1.0,
        None => true,
    };
    (same("fx", "cx") && same("fy", "cy")).then_some(note)
}

/// The size in points the drawing's coordinates are for: what `remember_size` recorded, else the extent.
fn reference_size(c: &Element, w: f64, h: f64) -> (f64, f64) {
    let rec = current_note(c);
    let (cx, cy) = (emu(rec, "cx") / EMU_PER_PT, emu(rec, "cy") / EMU_PER_PT);
    if cx > 0.0 && cy > 0.0 {
        (cx, cy)
    } else {
        (w.max(1e-6), h.max(1e-6))
    }
}

/// Before a graphic is resized: note the size its drawing is for, unless that is already noted and still true. `c`
/// is the `wp:inline` or `wp:anchor` element.
pub fn remember_size(c: &mut Element) {
    let (cx, cy) = match c.child("wp:extent") {
        Some(e) => (e.attr("cx").unwrap_or("0").to_string(), e.attr("cy").unwrap_or("0").to_string()),
        None => return,
    };
    if current_note(c).is_some() {
        return;
    }
    let ext = Element::new("a:ext")
        .with_attr("uri", SIZE_EXT_URI)
        .with_child(Element::new("fo:diagram").with_attr("xmlns:fo", SIZE_EXT_NS).with_attr("cx", cx.clone()).with_attr("cy", cy.clone()).with_attr("fx", cx).with_attr("fy", cy));
    let Some(pr) = c.child_mut("wp:docPr") else { return };
    if let Some(list) = pr.child_mut("a:extLst") {
        // A note that no longer fits is replaced.
        list.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "a:ext" && e.attr("uri") == Some(SIZE_EXT_URI)));
        list.children.push(Node::Element(ext));
    } else {
        pr.children.push(Node::Element(Element::new("a:extLst").with_attr("xmlns:a", super::build::NS_A).with_child(ext)));
    }
}

/// After Docxtor changed a graphic's extent: the note records the extent Docxtor gave it, to tell a later resize by
/// another program from this one.
pub fn note_extent(c: &mut Element) {
    let Some((cx, cy)) = c.child("wp:extent").map(|e| (e.attr("cx").unwrap_or("0").to_string(), e.attr("cy").unwrap_or("0").to_string())) else { return };
    let Some(list) = c.child_mut("wp:docPr").and_then(|d| d.child_mut("a:extLst")) else { return };
    for ext in list.elements_mut().filter(|e| e.name == "a:ext" && e.attr("uri") == Some(SIZE_EXT_URI)) {
        if let Some(note) = ext.elements_mut().next() {
            note.set_attr("fx", cx.clone());
            note.set_attr("fy", cy.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------

/// The colours a laid out diagram is drawn in.
#[derive(Debug, Clone)]
struct Palette {
    /// Node fills, used in turn by top-level node.
    fills: Vec<String>,
    line: String,
}

fn mix_white(hex: &str, keep: f64) -> String {
    let p = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64;
    let m = |c: f64| (c * keep + 255.0 * (1.0 - keep)).round().clamp(0.0, 255.0) as u8;
    super::theme::to_hex(m(p(0)), m(p(2)), m(p(4)))
}

fn light(hex: &str) -> bool {
    let p = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64;
    (0.299 * p(0) + 0.587 * p(2) + 0.114 * p(4)) / 255.0 > 0.72
}

fn local<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    e.elements().find(|c| c.local() == name)
}

fn find_local<'a>(e: &'a Element, name: &str) -> Option<&'a Element> {
    for c in e.elements() {
        if c.local() == name {
            return Some(c);
        }
        if let Some(f) = find_local(c, name) {
            return Some(f);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The data model
// ---------------------------------------------------------------------------

struct Model {
    nodes: Vec<lay::Node>,
    layout_id: Option<String>,
    colors_id: Option<String>,
}

fn paragraph_texts(t: &Element) -> String {
    let mut lines = Vec::new();
    for p in t.elements().filter(|e| e.local() == "p") {
        let mut s = String::new();
        for r in p.elements() {
            match r.local() {
                "r" | "fld" => {
                    if let Some(t) = r.elements().find(|e| e.local() == "t") {
                        s.push_str(&t.text());
                    }
                }
                "br" => s.push(' '),
                _ => {}
            }
        }
        lines.push(s);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// The most nodes of a diagram that are read.
const MAX_NODES: usize = 2000;

/// The tree of nodes in a `dgm:dataModel`: top-level nodes (children of the document point) in order.
fn read_model(dm: &Element) -> Model {
    struct Pt {
        kind: String,
        text: String,
    }
    let mut pts: HashMap<String, Pt> = HashMap::new();
    let mut doc_id: Option<String> = None;
    let mut layout_id = None;
    let mut colors_id = None;
    if let Some(list) = local(dm, "ptLst") {
        for p in list.elements().filter(|e| e.local() == "pt") {
            let id = p.attr("modelId").unwrap_or("").to_string();
            let kind = p.attr("type").unwrap_or("node").to_string();
            if kind == "doc" {
                doc_id = Some(id.clone());
                if let Some(ps) = local(p, "prSet") {
                    layout_id = ps.attr("loTypeId").map(String::from);
                    colors_id = ps.attr("csTypeId").map(String::from);
                }
            }
            let text = local(p, "t").map(paragraph_texts).unwrap_or_default();
            pts.insert(id, Pt { kind, text });
        }
    }
    // Parent to children, in the order the connections give.
    let mut kids: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    let mut has_parent: Vec<String> = Vec::new();
    if let Some(list) = local(dm, "cxnLst") {
        for c in list.elements().filter(|e| e.local() == "cxn") {
            if c.attr("type").unwrap_or("parOf") != "parOf" {
                continue;
            }
            let (Some(src), Some(dst)) = (c.attr("srcId"), c.attr("destId")) else { continue };
            let ord = c.attr("srcOrd").and_then(|v| v.parse().ok()).unwrap_or(0);
            kids.entry(src.to_string()).or_default().push((ord, dst.to_string()));
            has_parent.push(dst.to_string());
        }
    }
    for v in kids.values_mut() {
        v.sort_by_key(|(o, _)| *o);
    }
    let is_node = |id: &str| pts.get(id).is_some_and(|p| matches!(p.kind.as_str(), "node" | "asst"));
    // A real model is a tree; a crafted one can reach a node through many parents or through itself. A node already on
    // the way down is skipped, and the whole diagram is limited to a few thousand nodes.
    struct Walk<'a> {
        pts: &'a HashMap<String, Pt>,
        kids: &'a HashMap<String, Vec<(i64, String)>>,
        is_node: &'a dyn Fn(&str) -> bool,
        left: usize,
        path: Vec<String>,
    }
    impl Walk<'_> {
        fn build(&mut self, id: &str, depth: usize) -> lay::Node {
            self.left = self.left.saturating_sub(1);
            let text = self.pts.get(id).map(|p| p.text.clone()).unwrap_or_default();
            let mut children = Vec::new();
            if depth < 6 {
                self.path.push(id.to_string());
                if let Some(v) = self.kids.get(id) {
                    for (_, d) in v {
                        if self.left == 0 {
                            break;
                        }
                        if (self.is_node)(d) && !self.path.contains(d) {
                            children.push(self.build(d, depth + 1));
                        }
                    }
                }
                self.path.pop();
            }
            lay::Node { text, children }
        }
    }
    let mut walk = Walk { pts: &pts, kids: &kids, is_node: &is_node, left: MAX_NODES, path: Vec::new() };
    let roots: Vec<String> = match &doc_id {
        Some(d) => kids.get(d).map(|v| v.iter().filter(|(_, c)| is_node(c)).map(|(_, c)| c.clone()).collect()).unwrap_or_default(),
        // No document point: the nodes nothing points at, in the file's order.
        None => {
            let mut ids: Vec<&String> = pts.iter().filter(|(id, p)| matches!(p.kind.as_str(), "node" | "asst") && !has_parent.contains(id)).map(|(id, _)| id).collect();
            ids.sort();
            ids.into_iter().cloned().collect()
        }
    };
    let mut nodes = Vec::new();
    for r in &roots {
        if walk.left == 0 {
            break;
        }
        nodes.push(walk.build(r, 0));
    }
    Model { nodes, layout_id, colors_id }
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// The theme's heading and body fonts (`+mj-lt` and `+mn-lt`).
#[derive(Default)]
struct ThemeFonts {
    major: Option<String>,
    minor: Option<String>,
}

fn theme_fonts(pkg: &crate::package::Package) -> ThemeFonts {
    let main = pkg.main_document_path();
    let Some(rel) = pkg.rels_for(&main).ok().and_then(|r| r.into_iter().find(|r| r.rel_type == crate::package::REL_THEME)) else { return ThemeFonts::default() };
    let Ok(doc) = pkg.xml(&crate::package::resolve_target(&main, &rel.target)) else { return ThemeFonts::default() };
    let face = |scheme: &str| find_local(&doc.root, scheme).and_then(|f| local(f, "latin")).and_then(|l| l.attr("typeface")).filter(|t| !t.is_empty()).map(String::from);
    ThemeFonts { major: face("majorFont"), minor: face("minorFont") }
}

/// A text style in effect for the runs of one shape.
struct TextStyle<'a> {
    color: Option<String>,
    /// The font the shape's style refers to (`fontRef`), when the runs name none.
    font: Option<String>,
    fonts: &'a ThemeFonts,
}

fn xfrm_of(sppr: &Element) -> Option<&Element> {
    local(sppr, "xfrm")
}

impl Rd<'_, '_> {
    /// A SmartArt graphic. `c` is the `wp:inline` or `wp:anchor`, `g` the `dgm:relIds`.
    pub(super) fn diagram(&self, c: &Element, g: &Element, w: f64, h: f64) -> RShape {
        let mut sh = self.base(w, h);
        sh.kind = "diagram";
        let pkg = self.cx.pkg;
        let part_of = |attr: &str| g.attr(attr).and_then(|r| self.cx.rel_src(r));
        let load = |name: Option<String>| name.and_then(|n| pkg.xml(&n).ok());
        let dm = load(part_of("r:dm"));
        let lo = load(part_of("r:lo"));
        let cs = load(part_of("r:cs"));
        let model = dm.as_ref().map(|d| read_model(&d.root));
        let layout_id = lo.as_ref().and_then(|l| find_local(&l.root, "layoutDef").or(Some(&l.root)).and_then(|e| e.attr("uniqueId"))).map(String::from).or_else(|| model.as_ref().and_then(|m| m.layout_id.clone()));
        let layout_name = layout_id.as_deref().map(|l| l.rsplit('/').next().unwrap_or(l).to_string());
        let (rw, rh) = reference_size(c, w, h);
        let k = (w / rw, h / rh);
        let fonts = theme_fonts(pkg);

        // Word's own drawing.
        let drawing_part = dm.as_ref().and_then(|d| find_local(&d.root, "dataModelExt")).and_then(|e| e.attr("relId")).and_then(|r| self.cx.rel_src(r));
        if let Some(part) = drawing_part {
            if let Ok(doc) = pkg.xml(&part) {
                let kids = self.dsp_shapes(&doc.root, &part, k, &fonts);
                if !kids.is_empty() {
                    sh.children = kids;
                    sh.diagram = Some(RDiagram { source: "drawing", layout: layout_name });
                    return sh;
                }
            }
        }

        // No drawing: lay the nodes out here.
        let nodes = model.as_ref().map(|m| m.nodes.clone()).unwrap_or_default();
        let kind = layout_id.as_deref().and_then(lay::native_kind).unwrap_or(lay::Kind::BlockList);
        let pieces = if nodes.iter().any(|n| !n.text.is_empty() || !n.children.is_empty()) { lay::layout(kind, &nodes, w, h) } else { Vec::new() };
        if !pieces.is_empty() {
            let colors_id = cs.as_ref().and_then(|c| find_local(&c.root, "colorsDef")).and_then(|e| e.attr("uniqueId")).map(String::from).or_else(|| model.as_ref().and_then(|m| m.colors_id.clone()));
            let pal = self.palette(cs.as_ref().map(|c| &c.root), colors_id.as_deref());
            sh.children = pieces.iter().map(|p| self.piece_shape(p, &pal, &fonts)).collect();
            sh.diagram = Some(RDiagram { source: "layout", layout: layout_name });
            return sh;
        }
        sh.children = vec![self.placeholder(&nodes, w, h, &fonts)];
        sh.diagram = Some(RDiagram { source: "placeholder", layout: layout_name });
        sh
    }

    // ---- Word's drawing ---------------------------------------------------

    fn dsp_shapes(&self, root: &Element, part: &str, k: (f64, f64), fonts: &ThemeFonts) -> Vec<RShape> {
        let rels: HashMap<String, String> = self
            .cx
            .pkg
            .rels_for(part)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| !r.external)
            .map(|r| (r.id, crate::package::resolve_target(part, &r.target)))
            .collect();
        let mut out = Vec::new();
        if let Some(tree) = find_local(root, "spTree") {
            self.dsp_walk(tree, k, &rels, fonts, &mut out, 0);
        }
        out
    }

    fn dsp_walk(&self, parent: &Element, k: (f64, f64), rels: &HashMap<String, String>, fonts: &ThemeFonts, out: &mut Vec<RShape>, depth: usize) {
        for c in parent.elements() {
            match c.local() {
                "sp" => out.extend(self.dsp_sp(c, k, fonts)),
                "pic" => out.extend(self.dsp_pic(c, k, rels)),
                // Word does not nest them in practice; the members are drawn where they say.
                "grpSp" if depth < 4 => self.dsp_walk(c, k, rels, fonts, out, depth + 1),
                _ => {}
            }
        }
    }

    fn frame_of(&self, sppr: &Element, k: (f64, f64)) -> Option<(RShape, (f64, f64))> {
        let x = xfrm_of(sppr)?;
        let (off, ext) = (local(x, "off"), local(x, "ext"));
        let (ox, oy) = (emu(off, "x"), emu(off, "y"));
        let mut sh = self.base(emu(ext, "cx") * k.0 / EMU_PER_PT, emu(ext, "cy") * k.1 / EMU_PER_PT);
        sh.child_x = Some(ox * k.0 / EMU_PER_PT);
        sh.child_y = Some(oy * k.1 / EMU_PER_PT);
        sh.rot = emu(Some(x), "rot") / 60000.0;
        sh.flip_h = truthy(x.attr("flipH"));
        sh.flip_v = truthy(x.attr("flipV"));
        Some((sh, (ox, oy)))
    }

    fn dsp_sp(&self, sp: &Element, k: (f64, f64), fonts: &ThemeFonts) -> Option<RShape> {
        let sppr = local(sp, "spPr")?;
        let (mut sh, origin) = self.frame_of(sppr, k)?;
        let style = local(sp, "style");
        self.geometry(sppr, &mut sh);
        sh.fill = self.fill_of(sppr, style);
        sh.line = self.line_of(sppr, style);
        sh.effects = super::effects::read(self.th, Some(sppr), style);
        sh.name = sp.attr("modelId").map(String::from);
        if let Some(body) = local(sp, "txBody") {
            let font_ref = style.and_then(|s| local(s, "fontRef"));
            let color = font_ref.and_then(|f| self.th.color_of(f, None)).map(|(c, _)| c);
            let font = font_ref.and_then(|f| f.attr("idx")).and_then(|i| if i == "major" { fonts.major.clone() } else { fonts.minor.clone() });
            sh.dtext = self.dtext(body, local(sp, "txXfrm"), origin, &sh, k, &TextStyle { color, font, fonts });
        }
        Some(sh)
    }

    fn dsp_pic(&self, pic: &Element, k: (f64, f64), rels: &HashMap<String, String>) -> Option<RShape> {
        let sppr = local(pic, "spPr")?;
        let (mut sh, _) = self.frame_of(sppr, k)?;
        sh.kind = "picture";
        sh.src = find_local(pic, "blip").and_then(|b| b.attr("r:embed")).and_then(|r| rels.get(r).cloned());
        sh.src.as_ref()?;
        Some(sh)
    }

    /// The text of a `dsp:txBody`. `origin` is the shape's offset in EMU (the text rectangle is in the drawing's
    /// own coordinates).
    fn dtext(&self, body: &Element, tx: Option<&Element>, origin: (f64, f64), sh: &RShape, k: (f64, f64), st: &TextStyle) -> Option<RDText> {
        let bp = local(body, "bodyPr");
        let attr = |n: &str| bp.and_then(|b| b.attr(n));
        let ins = |n: &str, d: f64| attr(n).and_then(|v| v.parse::<f64>().ok()).map(|v| v / EMU_PER_PT).unwrap_or(d);
        let scale = bp.and_then(|b| find_local(b, "normAutofit")).and_then(|n| n.attr("fontScale")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 100000.0).unwrap_or(1.0);
        let paras: Vec<RDPara> = body.elements().filter(|e| e.local() == "p").map(|p| self.dpara(p, st, scale)).collect();
        if paras.iter().all(|p| p.runs.iter().all(|r| r.text.is_empty())) {
            return None;
        }
        // The text rectangle: the shape's own box, or `dsp:txXfrm` (in the drawing's coordinates).
        let (x, y, w, h, rot) = match tx.and_then(|t| Some((local(t, "off")?, local(t, "ext")?, t))) {
            Some((off, ext, t)) => ((emu(Some(off), "x") - origin.0) * k.0 / EMU_PER_PT, (emu(Some(off), "y") - origin.1) * k.1 / EMU_PER_PT, emu(Some(ext), "cx") * k.0 / EMU_PER_PT, emu(Some(ext), "cy") * k.1 / EMU_PER_PT, emu(Some(t), "rot") / 60000.0),
            None => (0.0, 0.0, sh.w, sh.h, 0.0),
        };
        Some(RDText {
            x,
            y,
            w,
            h,
            insets: RInsets { l: ins("lIns", 7.2), t: ins("tIns", 3.6), r: ins("rIns", 7.2), b: ins("bIns", 3.6) },
            v_anchor: match attr("anchor") {
                Some("ctr") => "middle",
                Some("b") => "bottom",
                _ => "top",
            },
            wrap: attr("wrap") != Some("none"),
            rot,
            paras,
        })
    }

    fn dpara(&self, p: &Element, st: &TextStyle, scale: f64) -> RDPara {
        let ppr = local(p, "pPr");
        let pa = |n: &str| ppr.and_then(|e| e.attr(n));
        let mut para = RDPara {
            align: match pa("algn") {
                Some("ctr") => "center",
                Some("r") => "right",
                Some("just" | "justLow" | "dist") => "justify",
                _ => "left",
            },
            margin: pa("marL").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / EMU_PER_PT,
            indent: pa("indent").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / EMU_PER_PT,
            ..Default::default()
        };
        let val = |e: Option<&Element>, name: &str| e.and_then(|e| local(e, name)).and_then(|s| s.attr("val")).and_then(|v| v.parse::<f64>().ok());
        let spc = |n: &str| ppr.and_then(|e| local(e, n));
        para.line_pct = val(spc("lnSpc"), "spcPct").map(|v| v / 1000.0).filter(|v| (*v - 100.0).abs() > 0.5);
        para.before = val(spc("spcBef"), "spcPts").map(|v| v / 100.0).unwrap_or(0.0);
        para.after = val(spc("spcAft"), "spcPts").map(|v| v / 100.0).unwrap_or(0.0);
        para.bullet = ppr.and_then(|e| local(e, "buChar")).and_then(|b| b.attr("char")).map(String::from);
        let end_size = local(p, "endParaRPr").and_then(|r| r.attr("sz")).and_then(|v| v.parse::<f64>().ok());
        for r in p.elements() {
            match r.local() {
                "r" | "fld" => {
                    let rpr = local(r, "rPr");
                    let text = local(r, "t").map(|t| t.text()).unwrap_or_default();
                    para.runs.push(self.drun(rpr, text, st, scale, end_size));
                }
                "br" => para.runs.push(self.drun(local(r, "rPr"), "\n".into(), st, scale, end_size)),
                _ => {}
            }
        }
        if para.runs.is_empty() {
            // An empty paragraph still takes a line of its size.
            para.runs.push(self.drun(None, String::new(), st, scale, end_size));
        }
        para
    }

    fn drun(&self, rpr: Option<&Element>, text: String, st: &TextStyle, scale: f64, end_size: Option<f64>) -> RDRun {
        let attr = |n: &str| rpr.and_then(|r| r.attr(n));
        let size = attr("sz").and_then(|v| v.parse::<f64>().ok()).or(end_size).unwrap_or(1800.0) / 100.0 * scale;
        let color = rpr.and_then(|r| local(r, "solidFill")).and_then(|f| self.th.color_of(f, None)).map(|(c, _)| c).or_else(|| st.color.clone());
        let font = match rpr.and_then(|r| local(r, "latin")).and_then(|l| l.attr("typeface")).filter(|f| !f.is_empty()) {
            Some(f) if f.starts_with("+mj") => st.fonts.major.clone(),
            Some(f) if f.starts_with("+mn") => st.fonts.minor.clone(),
            Some(f) if f.starts_with('+') => st.font.clone(),
            Some(f) => Some(f.to_string()),
            None => st.font.clone(),
        };
        RDRun { text, size, bold: truthy(attr("b")), italic: truthy(attr("i")), underline: attr("u").is_some_and(|u| u != "none"), color, font }
    }

    // ---- Laid out here ----------------------------------------------------

    /// The colours of the colours part (`node1`), else the accent its id names, else the first accent.
    fn palette(&self, colors: Option<&Element>, id: Option<&str>) -> Palette {
        let accent = |n: usize| self.th.scheme(&format!("accent{n}")).unwrap_or("4472C4").to_string();
        let mut fills: Vec<String> = Vec::new();
        let mut line = "FFFFFF".to_string();
        if let Some(lbl) = colors.and_then(|c| find_style_label(c, "node1")) {
            if let Some(list) = local(lbl, "fillClrLst") {
                for c in list.elements() {
                    let holder = Element::new("x").with_child(c.clone());
                    if let Some((hex, _)) = self.th.color_of(&holder, None) {
                        fills.push(hex);
                    }
                }
            }
            if let Some(l) = local(lbl, "linClrLst").and_then(|l| self.th.color_of(l, None)) {
                line = l.0;
            }
        }
        if fills.is_empty() {
            // `accent2_1` and `colorful3`: the number after `accent` picks the colour; colourful ones cycle.
            let name = id.and_then(|i| i.rsplit('/').next()).unwrap_or("");
            if let Some(n) = name.strip_prefix("accent").and_then(|r| r.chars().next()).and_then(|c| c.to_digit(10)).filter(|n| (1..=6).contains(n)) {
                fills.push(accent(n as usize));
            } else if name.starts_with("colorful") {
                fills.extend((2..=6).map(accent));
            } else {
                fills.push(accent(1));
            }
        }
        Palette { fills, line }
    }

    fn solid_xml(&self, hex: &str, width_emu: u32) -> Element {
        let ln = Element::new("a:ln").with_attr("w", width_emu.to_string()).with_child(Element::new("a:solidFill").with_child(Element::new("a:srgbClr").with_attr("val", hex)));
        Element::new("x").with_child(ln)
    }

    fn piece_shape(&self, p: &lay::Piece, pal: &Palette, fonts: &ThemeFonts) -> RShape {
        let mut sh = self.base(p.w, p.h);
        sh.child_x = Some(p.x);
        sh.child_y = Some(p.y);
        sh.rot = p.rot;
        sh.geometry = p.geometry.to_string();
        for (n, v) in &p.adj {
            sh.adj.insert((*n).to_string(), *v);
        }
        let fill = pal.fills.get(p.index % pal.fills.len().max(1)).cloned().unwrap_or_else(|| "4472C4".into());
        let (fill_hex, text_hex) = match p.role {
            lay::Role::Node => {
                let text = if light(&fill) { "000000" } else { "FFFFFF" };
                (Some(fill.clone()), text.to_string())
            }
            lay::Role::Sub => (Some(mix_white(&fill, 0.2)), "000000".to_string()),
            lay::Role::Arrow => (Some(mix_white(&fill, 0.6)), "000000".to_string()),
            lay::Role::Line => (None, "000000".to_string()),
        };
        sh.fill = fill_hex.map(|h| RFill::solid(h, 1.0));
        match p.role {
            lay::Role::Node => sh.line = self.line_of(&self.solid_xml(&pal.line, 12700), None),
            lay::Role::Line => sh.line = self.line_of(&self.solid_xml(&mix_white(&pal.fills[0], 0.6), 19050), None),
            _ => {}
        }
        if !p.text.is_empty() {
            let paras = p
                .text
                .iter()
                .map(|l| RDPara {
                    align: if p.role == lay::Role::Sub { "left" } else { "center" },
                    line_pct: Some(90.0),
                    bullet: l.sub.then(|| "\u{2022}".to_string()),
                    margin: if l.sub { 12.0 } else { 0.0 },
                    indent: if l.sub { -12.0 } else { 0.0 },
                    runs: vec![RDRun { text: l.text.clone(), size: if l.sub { p.size * 0.8 } else { p.size }, color: Some(text_hex.clone()), font: fonts.minor.clone(), ..Default::default() }],
                    ..Default::default()
                })
                .collect();
            sh.dtext = Some(RDText { x: 0.0, y: 0.0, w: p.w, h: p.h, insets: RInsets { l: 4.0, t: 3.0, r: 4.0, b: 3.0 }, v_anchor: "middle", wrap: true, rot: 0.0, paras });
        }
        sh
    }

    /// A box that says what the graphic is and lists the texts it holds.
    fn placeholder(&self, nodes: &[lay::Node], w: f64, h: f64, fonts: &ThemeFonts) -> RShape {
        let mut sh = self.base(w, h);
        sh.child_x = Some(0.0);
        sh.child_y = Some(0.0);
        sh.fill = Some(RFill::solid("F2F2F2".into(), 1.0));
        sh.line = self.line_of(&self.solid_xml("A6A6A6", 9525), None);
        let run = |text: String, size: f64, bold: bool| RDRun { text, size, bold, color: Some("404040".into()), font: fonts.minor.clone(), ..Default::default() };
        let mut paras = vec![RDPara { align: "center", runs: vec![run("SmartArt graphic".into(), 14.0, true)], ..Default::default() }];
        fn flatten(n: &lay::Node, depth: usize, out: &mut Vec<(usize, String)>) {
            for line in n.text.split('\n').filter(|l| !l.is_empty()) {
                out.push((depth, line.to_string()));
            }
            for c in &n.children {
                flatten(c, depth + 1, out);
            }
        }
        let mut lines = Vec::new();
        for n in nodes {
            flatten(n, 0, &mut lines);
        }
        for (depth, text) in lines {
            paras.push(RDPara { align: "left", bullet: Some("\u{2022}".into()), margin: 12.0 + depth as f64 * 12.0, indent: -12.0, runs: vec![run(text, 11.0, false)], ..Default::default() });
        }
        sh.dtext = Some(RDText { x: 0.0, y: 0.0, w, h, insets: RInsets { l: 10.0, t: 8.0, r: 10.0, b: 8.0 }, v_anchor: "middle", wrap: true, rot: 0.0, paras });
        sh
    }
}

fn find_style_label<'a>(colors: &'a Element, name: &str) -> Option<&'a Element> {
    colors.elements().find(|e| e.local() == "styleLbl" && e.attr("name") == Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixing_with_white_keeps_the_hue() {
        assert_eq!(mix_white("4472C4", 1.0), "4472C4");
        assert_eq!(mix_white("000000", 0.0), "FFFFFF");
        assert_eq!(mix_white("000000", 0.5), "808080");
    }

    #[test]
    fn dark_fills_get_white_text() {
        assert!(light("FFFFFF"));
        assert!(!light("4472C4"));
    }

    #[test]
    fn the_data_model_gives_nodes_in_order() {
        let xml = r#"<dgm:dataModel xmlns:dgm="d" xmlns:a="a"><dgm:ptLst>
<dgm:pt modelId="{DOC}" type="doc"><dgm:prSet loTypeId="urn:microsoft.com/office/officeart/2005/8/layout/process1" csTypeId="urn:microsoft.com/office/officeart/2005/8/colors/accent2_1"/><dgm:t><a:p><a:endParaRPr/></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="{B}"><dgm:t><a:bodyPr/><a:p><a:r><a:t>Second</a:t></a:r></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="{A}"><dgm:t><a:p><a:r><a:t>First</a:t></a:r></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="{C}"><dgm:t><a:p><a:r><a:t>Child</a:t></a:r></a:p></dgm:t></dgm:pt>
<dgm:pt modelId="{P1}" type="pres"/><dgm:pt modelId="{T1}" type="parTrans"/>
</dgm:ptLst><dgm:cxnLst>
<dgm:cxn modelId="{1}" srcId="{DOC}" destId="{B}" srcOrd="1" destOrd="0"/>
<dgm:cxn modelId="{2}" srcId="{DOC}" destId="{A}" srcOrd="0" destOrd="0"/>
<dgm:cxn modelId="{3}" srcId="{A}" destId="{C}" srcOrd="0" destOrd="0"/>
<dgm:cxn modelId="{4}" type="presOf" srcId="{A}" destId="{P1}" srcOrd="0" destOrd="0"/>
</dgm:cxnLst></dgm:dataModel>"#;
        let doc = crate::xml::parse(xml.as_bytes()).unwrap();
        let m = read_model(&doc.root);
        assert_eq!(m.nodes.len(), 2);
        assert_eq!(m.nodes[0].text, "First");
        assert_eq!(m.nodes[0].children[0].text, "Child");
        assert_eq!(m.nodes[1].text, "Second");
        assert!(m.layout_id.unwrap().ends_with("process1"));
        assert!(m.colors_id.unwrap().ends_with("accent2_1"));
    }

    #[test]
    fn the_size_of_the_drawing_is_noted_once() {
        let mut inline = Element::new("wp:inline")
            .with_child(Element::new("wp:extent").with_attr("cx", "5486400").with_attr("cy", "3200400"))
            .with_child(Element::new("wp:docPr").with_attr("id", "1").with_attr("name", "Diagram 1"));
        assert_eq!(reference_size(&inline, 432.0, 252.0), (432.0, 252.0));
        remember_size(&mut inline);
        // Docxtor changes the extent after that, the note does not.
        inline.child_mut("wp:extent").unwrap().set_attr("cx", "2743200");
        note_extent(&mut inline);
        remember_size(&mut inline);
        assert_eq!(inline.child("wp:docPr").unwrap().child("a:extLst").unwrap().children_named("a:ext").count(), 1);
        assert_eq!(reference_size(&inline, 216.0, 252.0), (432.0, 252.0));
    }

    #[test]
    fn a_note_goes_when_another_program_resized_the_graphic() {
        let mut inline = Element::new("wp:inline")
            .with_child(Element::new("wp:extent").with_attr("cx", "5486400").with_attr("cy", "3200400"))
            .with_child(Element::new("wp:docPr").with_attr("id", "1").with_attr("name", "Diagram 1"));
        remember_size(&mut inline);
        inline.child_mut("wp:extent").unwrap().set_attr("cx", "2743200");
        note_extent(&mut inline);
        assert_eq!(reference_size(&inline, 216.0, 252.0), (432.0, 252.0));
        // Word makes it 4 inches wide and draws it again for that size: the note's 6 inches no longer apply.
        inline.child_mut("wp:extent").unwrap().set_attr("cx", "3657600");
        assert_eq!(reference_size(&inline, 288.0, 252.0), (288.0, 252.0));
        // The next resize by Docxtor notes the size the drawing is for now, and the one note is replaced.
        remember_size(&mut inline);
        assert_eq!(inline.child("wp:docPr").unwrap().child("a:extLst").unwrap().children_named("a:ext").count(), 1);
        inline.child_mut("wp:extent").unwrap().set_attr("cx", "1828800");
        note_extent(&mut inline);
        assert_eq!(reference_size(&inline, 144.0, 252.0), (288.0, 252.0));
        // A note from before the extent was tracked is still trusted.
        let mut old = Element::new("wp:inline").with_child(Element::new("wp:extent").with_attr("cx", "2743200").with_attr("cy", "3200400")).with_child(Element::new("wp:docPr").with_child(
            Element::new("a:extLst").with_child(Element::new("a:ext").with_attr("uri", SIZE_EXT_URI).with_child(Element::new("fo:diagram").with_attr("cx", "5486400").with_attr("cy", "3200400"))),
        ));
        assert_eq!(reference_size(&old, 216.0, 252.0), (432.0, 252.0));
        remember_size(&mut old);
        assert_eq!(reference_size(&old, 216.0, 252.0), (432.0, 252.0));
    }
}
