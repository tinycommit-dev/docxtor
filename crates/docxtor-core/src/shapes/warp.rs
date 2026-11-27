//! WordArt: text bent to a shape (`a:prstTxWarp`, and VML `v:textpath`).
//!
//! DrawingML WordArt is an ordinary shape with text (a `TextBox` story, editable as always) whose `wps:bodyPr`
//! holds a `a:prstTxWarp` and whose runs carry the text look in `w14:textFill`, `w14:textOutline`,
//! `w14:shadow`, `w14:glow` and `w14:reflection`. VML WordArt (Word 97 to 2007, and still the fallback of
//! DrawingML WordArt) is a `v:shape` whose `v:textpath` holds the text in its `string` attribute; the shape's
//! fill and stroke are the text's fill and outline.
//!
//! This module only reads: the view model carries the preset, its adjust values and the text paint; the
//! editor bends the text (see `editor/warp.ts`). The warp presets are the 40 of `ST_TextShapeType`; a VML
//! shape type number (`o:spt` 136 to 175) stands for the preset in the same position.

use super::read::Rd;
use super::*;
use crate::marks::{length_pt, parse_opacity, style_prop};
use crate::xml::Name;

/// The text of a shape bent to a preset.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RWarp {
    /// A DrawingML preset (`textArchUp`, `textWave1`, `textPlain`, ...).
    pub preset: String,
    /// The preset's adjust values in the file's units (`adj`, `adj1`, `adj2`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub adj: BTreeMap<String, f64>,
    /// `drawingml` (the text is the shape's story) or `vml` (the text is `text`).
    pub source: &'static str,
    /// VML: the text, and its font.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// The text fill; absent when the text has no fill of its own (it is then the text's colour).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<RFill>,
    /// The text outline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<RLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<RTextShadow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<RTextGlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<RTextReflection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RTextShadow {
    pub color: String,
    pub alpha: f64,
    pub blur_pt: f64,
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RTextGlow {
    pub color: String,
    pub alpha: f64,
    pub radius_pt: f64,
}

/// A reflection under the text: it starts at `start_alpha` and fades to `end_alpha` at `size` percent of the
/// text's height, `dist_pt` below it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RTextReflection {
    pub start_alpha: f64,
    pub end_alpha: f64,
    pub size: f64,
    pub dist_pt: f64,
}

/// Every `ST_TextShapeType` that bends text, in the order VML numbers them from 136.
pub const PRESETS: [&str; 40] = [
    "textPlain",
    "textStop",
    "textTriangle",
    "textTriangleInverted",
    "textChevron",
    "textChevronInverted",
    "textRingInside",
    "textRingOutside",
    "textArchUp",
    "textArchDown",
    "textCircle",
    "textButton",
    "textArchUpPour",
    "textArchDownPour",
    "textCirclePour",
    "textButtonPour",
    "textCurveUp",
    "textCurveDown",
    "textCascadeUp",
    "textCascadeDown",
    "textWave1",
    "textWave2",
    "textDoubleWave1",
    "textWave4",
    "textInflate",
    "textDeflate",
    "textInflateBottom",
    "textDeflateBottom",
    "textInflateTop",
    "textDeflateTop",
    "textDeflateInflate",
    "textDeflateInflateDeflate",
    "textFadeRight",
    "textFadeLeft",
    "textFadeUp",
    "textFadeDown",
    "textSlantUp",
    "textSlantDown",
    "textCanUp",
    "textCanDown",
];

/// The preset for a VML shape type number (`o:spt`, 136 to 175).
pub fn preset_for_spt(spt: i64) -> Option<&'static str> {
    PRESETS.get(usize::try_from(spt - 136).ok()?).copied()
}

/// The DrawingML name for a name or number that stands for a warp.
pub fn is_preset(name: &str) -> bool {
    PRESETS.contains(&name)
}

/// A copy of a `w14` element with the names of DrawingML (`w14:solidFill` to `a:solidFill`, `w14:val` to `val`),
/// so the readers of shape fills and colours read it.
fn as_drawingml(e: &Element) -> Element {
    let mut out = Element::new(format!("a:{}", e.local()));
    for (k, v) in &e.attrs {
        if k.starts_with("xmlns") {
            continue;
        }
        out.attrs.push((Name::intern(k.rsplit(':').next().unwrap_or(k)), v.clone()));
    }
    for c in e.elements() {
        out.children.push(crate::xml::Node::Element(as_drawingml(c)));
    }
    out
}

fn num(e: &Element, a: &str) -> Option<f64> {
    e.attr(a).and_then(|v| v.parse::<f64>().ok())
}

/// The adjust values in an `a:avLst`.
fn read_adj(av: Option<&Element>) -> BTreeMap<String, f64> {
    let mut adj = BTreeMap::new();
    for g in av.into_iter().flat_map(|a| a.children_named("a:gd")) {
        let v = g.attr("fmla").and_then(|f| f.strip_prefix("val ")).and_then(|v| v.trim().parse::<f64>().ok());
        if let (Some(n), Some(v)) = (g.attr("name"), v) {
            adj.insert(n.to_string(), v);
        }
    }
    adj
}

/// The first run with text in a text box's content, for its look.
fn first_rpr(content: &Element) -> Option<&Element> {
    let mut runs = Vec::new();
    content.find_all("w:r", &mut runs);
    runs.into_iter().find(|r| r.child("w:t").is_some_and(|t| !t.text().is_empty())).and_then(|r| r.child("w:rPr"))
}

impl Rd<'_, '_> {
    /// WordArt in a `wps:bodyPr`: the warp, and the look of the text from its first run.
    pub(super) fn warp_dml(&self, body_pr: &Element, txbx: Option<&Element>) -> Option<RWarp> {
        let tw = body_pr.child("a:prstTxWarp")?;
        let preset = tw.attr("prst")?;
        if !is_preset(preset) {
            return None;
        }
        let mut warp = RWarp { preset: preset.to_string(), adj: read_adj(tw.child("a:avLst")), source: "drawingml", ..Default::default() };
        if let Some(rpr) = txbx.and_then(|t| t.child("w:txbxContent")).and_then(first_rpr) {
            self.text_look(rpr, &mut warp);
        }
        Some(warp)
    }

    /// Fill, outline, shadow, glow and reflection from a run's properties.
    fn text_look(&self, rpr: &Element, warp: &mut RWarp) {
        let src = |rid: &str| self.cx.rel_src(rid);
        if let Some(fill) = rpr.child("w14:textFill") {
            match fill.elements().next() {
                Some(f) if f.local() == "noFill" => {}
                Some(f) => warp.fill = super::fill::read(self.th, &as_drawingml(f), None, &src),
                None => {}
            }
        } else if let Some(c) = rpr.child("w:color").and_then(|c| c.attr("w:val")).and_then(|v| if v == "auto" { None } else { hex6(v) }) {
            warp.fill = Some(RFill::solid(c, 1.0));
        }
        if let Some(o) = rpr.child("w14:textOutline") {
            let mut ln = as_drawingml(o);
            ln.name = "a:ln".into();
            let holder = Element::new("x").with_child(ln);
            warp.line = self.line_of(&holder, None);
        }
        if let Some(s) = rpr.child("w14:shadow") {
            let a = as_drawingml(s);
            if let Some((color, alpha)) = self.th.color_of(&a, None) {
                let (dist, dir) = (num(&a, "dist").unwrap_or(0.0) / EMU_PER_PT, num(&a, "dir").unwrap_or(0.0).to_radians() / 60000.0);
                warp.shadow = Some(RTextShadow { color, alpha, blur_pt: num(&a, "blurRad").unwrap_or(0.0) / EMU_PER_PT, dx: dist * dir.cos(), dy: dist * dir.sin() });
            }
        }
        if let Some(g) = rpr.child("w14:glow") {
            let a = as_drawingml(g);
            if let Some((color, alpha)) = self.th.color_of(&a, None) {
                warp.glow = Some(RTextGlow { color, alpha, radius_pt: num(&a, "rad").unwrap_or(0.0) / EMU_PER_PT });
            }
        }
        if let Some(r) = rpr.child("w14:reflection") {
            let a = as_drawingml(r);
            let pct = |n: &str, d: f64| num(&a, n).unwrap_or(d) / 100000.0;
            warp.reflection = Some(RTextReflection { start_alpha: pct("stA", 100000.0), end_alpha: pct("endA", 0.0), size: pct("endPos", 100000.0) * 100.0, dist_pt: num(&a, "dist").unwrap_or(0.0) / EMU_PER_PT });
        }
    }

    /// WordArt in VML: `shape` is the `v:shape` with a `v:textpath`; its fill and outline are the text's.
    pub(super) fn warp_vml(&self, shape: &Element, spt: Option<i64>, sh: &mut RShape) -> Option<RWarp> {
        let tp = shape.child("v:textpath")?;
        let preset = spt.and_then(preset_for_spt).unwrap_or("textPlain");
        let style = tp.attr("style").unwrap_or("");
        let mut warp = RWarp {
            preset: preset.to_string(),
            source: "vml",
            text: Some(tp.attr("string").unwrap_or("").to_string()),
            font: style_prop(style, "font-family").map(|f| f.trim_matches(|c| c == '"' || c == '\'' || c == '\u{201C}' || c == '\u{201D}').to_string()).filter(|f| !f.is_empty()),
            bold: matches!(style_prop(style, "font-weight"), Some("bold" | "700" | "800" | "900")),
            italic: style_prop(style, "font-style") == Some("italic"),
            ..Default::default()
        };
        // The shape's fill and outline are the text's.
        warp.fill = sh.fill.take();
        warp.line = sh.line.take();
        if let Some(s) = shape.child("v:shadow").filter(|s| s.attr("on") != Some("f") && s.attr("on") != Some("false")) {
            let color = s.attr("color").and_then(hex6).unwrap_or_else(|| "808080".into());
            let alpha = s.attr("opacity").and_then(parse_opacity).unwrap_or(1.0);
            let off: Vec<f64> = s.attr("offset").unwrap_or("2pt,2pt").split(',').map(|v| length_pt(v).unwrap_or(2.0)).collect();
            warp.shadow = Some(RTextShadow { color, alpha, blur_pt: 0.0, dx: off.first().copied().unwrap_or(2.0), dy: off.get(1).copied().unwrap_or(2.0) });
        }
        Some(warp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vml_shape_types_map_to_the_presets_in_order() {
        assert_eq!(preset_for_spt(136), Some("textPlain"));
        assert_eq!(preset_for_spt(144), Some("textArchUp"));
        assert_eq!(preset_for_spt(148), Some("textArchUpPour"));
        assert_eq!(preset_for_spt(156), Some("textWave1"));
        assert_eq!(preset_for_spt(158), Some("textDoubleWave1"));
        assert_eq!(preset_for_spt(160), Some("textInflate"));
        assert_eq!(preset_for_spt(168), Some("textFadeRight"));
        assert_eq!(preset_for_spt(172), Some("textSlantUp"));
        assert_eq!(preset_for_spt(175), Some("textCanDown"));
        assert_eq!(preset_for_spt(135), None);
        assert_eq!(preset_for_spt(176), None);
        assert_eq!(PRESETS.len(), 40);
    }

    #[test]
    fn w14_elements_read_as_drawingml() {
        let xml = r#"<w14:gradFill xmlns:w14="w14"><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="FF0000"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill>"#;
        let doc = crate::xml::parse(xml.as_bytes()).unwrap();
        let a = as_drawingml(&doc.root);
        assert_eq!(a.name, "a:gradFill");
        assert_eq!(a.child("a:gsLst").unwrap().child("a:gs").unwrap().attr("pos"), Some("0"));
        assert_eq!(a.child("a:lin").unwrap().attr("ang"), Some("5400000"));
    }
}
