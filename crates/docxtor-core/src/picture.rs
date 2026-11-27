//! How a picture is formatted: crop, rotation and flips, the outline, brightness and contrast, transparency,
//! recolouring, the picture's shape, and its shadow, glow, reflection and soft edges. Reading (for the view
//! model, `PicFmt`) and writing (the `set_image` patch) live here; `image` keeps size, wrapping and placement.
//!
//! Everything is DrawingML on the picture (`pic:pic`): `a:srcRect` and the `a:blip` transforms in
//! `pic:blipFill`, `a:xfrm`, `a:prstGeom`, `a:ln` and `a:effectLst` in `pic:spPr`.

use crate::image::ImagePatch;
use crate::props::put_ordered;
use crate::shapes::theme::ShapeTheme;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

const EMU_PER_PT: f64 = 12700.0;

/// A value in a patch that can also say "remove it": the JSON string `"none"`, or the value itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Switch<T> {
    Off,
    On(T),
}

impl<T: Serialize> Serialize for Switch<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Switch::Off => s.serialize_str("none"),
            Switch::On(v) => v.serialize(s),
        }
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Switch<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("none") => Ok(Switch::Off),
            serde_json::Value::Null => Ok(Switch::Off),
            _ => serde_json::from_value(v).map(Switch::On).map_err(serde::de::Error::custom),
        }
    }
}

// ---------------------------------------------------------------------------
// The view
// ---------------------------------------------------------------------------

fn not(v: &bool) -> bool {
    !*v
}

/// The outline of a picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PicLine {
    /// Hex, no `#`.
    #[serde(default = "black")]
    pub color: String,
    /// Width in points.
    #[serde(default = "one", alias = "width_pt")]
    pub width_pt: f64,
    /// `solid`, `dot`, `dash`, `lgDash`, `dashDot`, `lgDashDot`, `lgDashDotDot`, `sysDash`, `sysDot`, ...
    #[serde(default = "solid")]
    pub dash: String,
}

fn black() -> String {
    "000000".into()
}
fn one() -> f64 {
    1.0
}
fn solid() -> String {
    "solid".into()
}

/// Recolouring: `grayscale`, `blackWhite` (with `threshold`, 0 to 1) or `duotone` (`color1` for the dark end, `color2` for the light).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recolor {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Hex, or a theme colour name (`accent1`, `dk2`, ...) when writing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color2: Option<String>,
}

/// One colour turned into another (Word's Set Transparent Color has `to` absent: the colour becomes see-through).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorChange {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    #[serde(default = "d_blur", alias = "blur_pt")]
    pub blur_pt: f64,
    #[serde(default = "d_dist", alias = "dist_pt")]
    pub dist_pt: f64,
    /// Degrees clockwise from the right (45 is down and to the right).
    #[serde(default = "d_angle")]
    pub angle: f64,
    #[serde(default = "black")]
    pub color: String,
    /// 0 to 1.
    #[serde(default = "d_alpha")]
    pub alpha: f64,
}

fn d_blur() -> f64 {
    4.0
}
fn d_dist() -> f64 {
    3.0
}
fn d_angle() -> f64 {
    45.0
}
fn d_alpha() -> f64 {
    0.4
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Glow {
    #[serde(default = "d_glow", alias = "radius_pt")]
    pub radius_pt: f64,
    #[serde(default = "d_glow_color")]
    pub color: String,
    #[serde(default = "d_alpha")]
    pub alpha: f64,
}

fn d_glow() -> f64 {
    8.0
}
fn d_glow_color() -> String {
    "5B9BD5".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reflection {
    /// How much of the picture is mirrored, 0 to 1.
    #[serde(default = "d_size")]
    pub size: f64,
    /// How strong the mirror is where it starts, 0 to 1.
    #[serde(default = "d_refl_alpha")]
    pub alpha: f64,
    /// Gap between the picture and its reflection.
    #[serde(default, alias = "dist_pt")]
    pub dist_pt: f64,
    #[serde(default, alias = "blur_pt")]
    pub blur_pt: f64,
}

fn d_size() -> f64 {
    0.5
}
fn d_refl_alpha() -> f64 {
    0.5
}

/// A picture's formatting as the view draws it. Fractions are of the picture's own size; angles are degrees.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PicFmt {
    /// Left, top, right and bottom crop as fractions of the whole picture; negative values are padding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot: Option<f64>,
    #[serde(skip_serializing_if = "not")]
    pub flip_h: bool,
    #[serde(skip_serializing_if = "not")]
    pub flip_v: bool,
    /// The picture's shape (`ellipse`, `roundRect`, ...); absent for a rectangle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geom: Option<String>,
    /// A rounded rectangle's corner, as a fraction of the shorter side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adj: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<PicLine>,
    /// 0 (invisible) to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// -1 to 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bright: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recolor: Option<Recolor>,
    /// With `to` absent the colour is made transparent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clr_change: Option<ColorChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<Glow>,
    /// Soft edge radius in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_edge: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<Reflection>,
    /// An artistic effect Word applied (`artisticBlur`, `artisticPencilGrayscale`, ...), with its radius where it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub art: Option<(String, f64)>,
    /// The media part of the picture's SVG version (`asvg:svgBlip`); `src` is then its PNG stand-in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    /// The picture is linked, not stored in the file: where it points (a path or an address).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// Space Word keeps around the picture for its rotation, outline and effects (`wp:effectExtent`): left, top, right, bottom in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<[f64; 4]>,
}

impl PicFmt {
    pub fn is_empty(&self) -> bool {
        *self == PicFmt::default()
    }
}

fn num(e: &Element, a: &str) -> Option<f64> {
    e.attr(a).and_then(|v| v.trim().parse::<f64>().ok())
}

/// A colour element's colour as hex, resolving theme colours and the transforms on them.
fn color_hex(theme: &ShapeTheme, color_el: &Element) -> Option<(String, f64)> {
    let holder = Element { name: "holder".into(), attrs: vec![], children: vec![Node::Element(color_el.clone())] };
    theme.color_of(&holder, None)
}

fn is_color_el(e: &Element) -> bool {
    matches!(e.name.as_str(), "a:srgbClr" | "a:schemeClr" | "a:sysClr" | "a:prstClr" | "a:scrgbClr" | "a:hslClr")
}

/// The first colour inside an element (`a:solidFill`, `a:clrFrom`, `a:glow`, ...).
fn first_color(theme: &ShapeTheme, parent: &Element) -> Option<(String, f64)> {
    parent.elements().find(|e| is_color_el(e)).and_then(|c| color_hex(theme, c))
}

/// Read a picture's formatting from the drawing that holds it. `rel` turns a relationship id into the part it
/// points to and whether that is outside the file.
pub fn read(drawing: &Element, theme: &ShapeTheme, rel: &dyn Fn(&str) -> Option<(String, bool)>) -> PicFmt {
    let mut f = PicFmt::default();
    let Some(pic) = drawing.find("pic:pic") else { return f };
    let container = drawing.find("wp:inline").or_else(|| drawing.find("wp:anchor"));
    if let Some(ee) = container.and_then(|c| c.child("wp:effectExtent")) {
        let v = |a: &str| (num(ee, a).unwrap_or(0.0) / EMU_PER_PT * 100.0).round() / 100.0;
        let ext = [v("l"), v("t"), v("r"), v("b")];
        if ext.iter().any(|x| *x > 0.0) {
            f.extent = Some(ext);
        }
    }
    if let Some(fill) = pic.child("pic:blipFill") {
        if let Some(sr) = fill.child("a:srcRect") {
            let g = |a: &str| num(sr, a).unwrap_or(0.0) / 100000.0;
            let c = [g("l"), g("t"), g("r"), g("b")];
            if c.iter().any(|x| x.abs() > 1e-9) {
                f.crop = Some(c);
            }
        }
        if let Some(blip) = fill.child("a:blip") {
            read_blip(&mut f, blip, theme, rel);
        }
    }
    if let Some(sp) = pic.child("pic:spPr") {
        if let Some(x) = sp.child("a:xfrm") {
            let r = num(x, "rot").unwrap_or(0.0) / 60000.0;
            if r.rem_euclid(360.0).abs() > 1e-6 {
                f.rot = Some((r.rem_euclid(360.0) * 100.0).round() / 100.0);
            }
            f.flip_h = matches!(x.attr("flipH"), Some("1") | Some("true"));
            f.flip_v = matches!(x.attr("flipV"), Some("1") | Some("true"));
        }
        if let Some(g) = sp.child("a:prstGeom") {
            let name = g.attr("prst").unwrap_or("rect");
            if name != "rect" {
                f.geom = Some(name.to_string());
                if let Some(adj) = g.child("a:avLst").and_then(|l| l.children_named("a:gd").find(|d| matches!(d.attr("name"), Some("adj") | Some("adj1")))) {
                    f.adj = adj.attr("fmla").and_then(|v| v.strip_prefix("val ")).and_then(|v| v.trim().parse::<f64>().ok()).map(|v| v / 100000.0);
                }
            }
        }
        if let Some(ln) = sp.child("a:ln") {
            if ln.child("a:noFill").is_none() {
                if let Some(fill) = ln.child("a:solidFill") {
                    if let Some((color, alpha)) = first_color(theme, fill) {
                        let _ = alpha;
                        f.line = Some(PicLine {
                            color,
                            width_pt: num(ln, "w").map(|w| w / EMU_PER_PT).unwrap_or(0.75),
                            dash: ln.child("a:prstDash").and_then(|d| d.attr("val")).unwrap_or("solid").to_string(),
                        });
                    }
                }
            }
        }
        if let Some(fx) = sp.child("a:effectLst") {
            read_effects(&mut f, fx, theme);
        }
    }
    f
}

fn read_blip(f: &mut PicFmt, blip: &Element, theme: &ShapeTheme, rel: &dyn Fn(&str) -> Option<(String, bool)>) {
    for e in blip.elements() {
        match e.name.as_str() {
            "a:alphaModFix" => {
                let a = num(e, "amt").unwrap_or(100000.0) / 100000.0;
                if a < 0.999 {
                    f.opacity = Some(a.clamp(0.0, 1.0));
                }
            }
            "a:lum" => {
                let b = num(e, "bright").unwrap_or(0.0) / 100000.0;
                let c = num(e, "contrast").unwrap_or(0.0) / 100000.0;
                if b != 0.0 {
                    f.bright = Some(b);
                }
                if c != 0.0 {
                    f.contrast = Some(c);
                }
            }
            "a:grayscl" => f.recolor = Some(Recolor { kind: "grayscale".into(), threshold: None, color1: None, color2: None }),
            "a:biLevel" => {
                f.recolor = Some(Recolor { kind: "blackWhite".into(), threshold: Some(num(e, "thresh").unwrap_or(50000.0) / 100000.0), color1: None, color2: None })
            }
            "a:duotone" => {
                let cols: Vec<String> = e.elements().filter(|c| is_color_el(c)).filter_map(|c| color_hex(theme, c)).map(|(h, _)| h).collect();
                if cols.len() >= 2 {
                    f.recolor = Some(Recolor { kind: "duotone".into(), threshold: None, color1: Some(cols[0].clone()), color2: Some(cols[1].clone()) });
                }
            }
            "a:clrChange" => {
                let from = e.child("a:clrFrom").and_then(|c| first_color(theme, c));
                let to = e.child("a:clrTo").and_then(|c| first_color(theme, c));
                if let Some((from, _)) = from {
                    // A target with no alpha at all is a solid colour; alpha 0 is Word's "Set Transparent Color".
                    let to_clear = e.child("a:clrTo").and_then(|c| c.elements().next()).and_then(|c| c.child("a:alpha")).and_then(|a| num(a, "val")).is_some_and(|a| a < 1000.0);
                    f.clr_change = Some(ColorChange { from, to: if to_clear { None } else { to.map(|(h, _)| h) } });
                }
            }
            _ => {}
        }
    }
    if let Some(svg) = blip.find("asvg:svgBlip").and_then(|s| s.attr("r:embed")).and_then(rel) {
        f.svg = Some(svg.0);
    }
    // Word 2010 and later keep the picture corrections in an extension next to the original picture, and put the
    // corrected picture in the blip itself. Corrections are only drawn here when the blip is the original.
    if let Some(layer) = blip.find("a14:imgLayer") {
        let baked = layer.attr("r:embed").is_some() && layer.attr("r:embed") != blip.attr("r:embed");
        if !baked {
            if let Some(bc) = layer.find("a14:brightnessContrast") {
                f.bright = f.bright.or(Some(num(bc, "bright").unwrap_or(0.0) / 100000.0)).filter(|v| *v != 0.0);
                f.contrast = f.contrast.or(Some(num(bc, "contrast").unwrap_or(0.0) / 100000.0)).filter(|v| *v != 0.0);
            }
            if let Some(s) = layer.find("a14:saturation") {
                f.saturation = Some(num(s, "sat").unwrap_or(100000.0) / 100000.0).filter(|v| (*v - 1.0).abs() > 1e-6);
            }
        }
        if let Some(art) = layer.children_named("a14:imgEffect").find_map(|e| e.elements().find(|c| c.local().starts_with("artistic"))) {
            f.art = Some((art.local().to_string(), num(art, "radius").unwrap_or(0.0)));
        }
    }
}

fn read_effects(f: &mut PicFmt, fx: &Element, theme: &ShapeTheme) {
    if let Some(s) = fx.child("a:outerShdw") {
        let (color, alpha) = first_color(theme, s).unwrap_or(("000000".into(), 0.4));
        f.shadow = Some(Shadow {
            blur_pt: num(s, "blurRad").unwrap_or(0.0) / EMU_PER_PT,
            dist_pt: num(s, "dist").unwrap_or(0.0) / EMU_PER_PT,
            angle: num(s, "dir").unwrap_or(0.0) / 60000.0,
            color,
            alpha,
        });
    }
    if let Some(g) = fx.child("a:glow") {
        let (color, alpha) = first_color(theme, g).unwrap_or(("5B9BD5".into(), 0.4));
        f.glow = Some(Glow { radius_pt: num(g, "rad").unwrap_or(0.0) / EMU_PER_PT, color, alpha });
    }
    if let Some(s) = fx.child("a:softEdge") {
        f.soft_edge = Some(num(s, "rad").unwrap_or(0.0) / EMU_PER_PT);
    }
    if let Some(r) = fx.child("a:reflection") {
        let start = num(r, "stA").unwrap_or(100000.0) / 100000.0;
        let end_pos = num(r, "endPos").unwrap_or(100000.0) / 100000.0;
        f.reflection = Some(Reflection { size: end_pos.clamp(0.0, 1.0), alpha: start.clamp(0.0, 1.0), dist_pt: num(r, "dist").unwrap_or(0.0) / EMU_PER_PT, blur_pt: num(r, "blurRad").unwrap_or(0.0) / EMU_PER_PT });
    }
}

/// The crop of an old-style (VML) picture: `cropleft`, `croptop`, `cropright` and `cropbottom` on `v:imagedata`,
/// each a percentage (`10%`) or a fraction in 65536ths (`6554f`).
pub fn read_vml_crop(imagedata: &Element) -> Option<[f64; 4]> {
    let one = |a: &str| -> f64 {
        let Some(v) = imagedata.attr(a) else { return 0.0 };
        let v = v.trim();
        if let Some(p) = v.strip_suffix('%') {
            p.trim().parse::<f64>().map(|x| x / 100.0).unwrap_or(0.0)
        } else if let Some(p) = v.strip_suffix('f') {
            p.trim().parse::<f64>().map(|x| x / 65536.0).unwrap_or(0.0)
        } else {
            v.parse::<f64>().map(|x| x / 65536.0).unwrap_or(0.0)
        }
    };
    let c = [one("cropleft"), one("croptop"), one("cropright"), one("cropbottom")];
    c.iter().any(|x| x.abs() > 1e-9).then_some(c)
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

fn find_mut<'a>(el: &'a mut Element, name: &str) -> Option<&'a mut Element> {
    for c in el.elements_mut() {
        if c.name == name {
            return Some(c);
        }
        if let Some(f) = find_mut(c, name) {
            return Some(f);
        }
    }
    None
}

const SPPR_ORDER: &[&str] = &["a:xfrm", "a:custGeom", "a:prstGeom", "a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill", "a:ln", "a:effectLst", "a:effectDag", "a:scene3d", "a:sp3d", "a:extLst"];
const FX_ORDER: &[&str] = &["a:blur", "a:fillOverlay", "a:glow", "a:innerShdw", "a:outerShdw", "a:prstShdw", "a:reflection", "a:softEdge"];
const BLIP_COLOR: &[&str] = &["a:grayscl", "a:biLevel", "a:duotone"];

const SCHEME: &[&str] = &["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "tx1", "tx2", "bg1", "bg2", "hlink", "folHlink"];

/// A colour element for a hex colour or a theme colour name, with an optional alpha.
fn color_el(c: &str, alpha: Option<f64>) -> Element {
    let c = c.trim().trim_start_matches('#');
    let mut e = if SCHEME.contains(&c) {
        Element::new("a:schemeClr").with_attr("val", c)
    } else {
        let hex = crate::shapes::hex6(c).unwrap_or_else(|| "000000".into());
        Element::new("a:srgbClr").with_attr("val", hex)
    };
    if let Some(a) = alpha.filter(|a| *a < 0.999) {
        e = e.with_child(Element::new("a:alpha").with_attr("val", ((a.clamp(0.0, 1.0)) * 100000.0).round().to_string()));
    }
    e
}

fn emu(pt: f64) -> String {
    (pt * EMU_PER_PT).round().max(0.0).to_string()
}

const GEOMETRIES: &[&str] = &[
    "rect", "roundRect", "round1Rect", "round2SameRect", "round2DiagRect", "snip1Rect", "snip2SameRect", "snip2DiagRect", "ellipse", "triangle", "rtTriangle", "diamond", "parallelogram", "trapezoid", "pentagon",
    "hexagon", "octagon", "star5", "plus", "rightArrow", "leftArrow", "upArrow", "downArrow", "heart", "chevron", "homePlate", "teardrop", "plaque", "flowChartTerminator",
];

fn pic_mut(drawing: &mut Element) -> Result<&mut Element, String> {
    find_mut(drawing, "pic:pic").ok_or_else(|| "not a picture".to_string())
}

/// The sizes `wp:extent` holds, in points, of the drawing's container.
fn extent_pt(c: &Element) -> (f64, f64) {
    let e = c.child("wp:extent");
    let g = |a: &str| e.and_then(|x| num(x, a)).unwrap_or(914400.0) / EMU_PER_PT;
    (g("cx"), g("cy"))
}

/// Apply the picture-formatting part of a patch to a drawing (`w:drawing`, already holding a `wp:inline` or `wp:anchor`).
pub fn apply(drawing: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    if !patch.touches_picture() {
        return Ok(());
    }
    if drawing.find("pic:pic").is_none() {
        return Err("that object isn't a picture".into());
    }
    if patch.reset == Some(true) {
        reset(drawing);
    }
    if let Some(crop) = patch.crop {
        set_crop(drawing, crop, patch.width.is_some() || patch.height.is_some())?;
    }
    {
        let pic = pic_mut(drawing)?;
        if patch.rot.is_some() || patch.flip_h.is_some() || patch.flip_v.is_some() || patch.geometry.is_some() || patch.border.is_some() || patch.shadow.is_some() || patch.glow.is_some() || patch.soft_edge.is_some() || patch.reflection.is_some() {
            if pic.child("pic:spPr").is_none() {
                pic.children.push(Node::Element(Element::new("pic:spPr")));
            }
            let sp = pic.child_mut("pic:spPr").ok_or("no picture properties")?;
            set_shape_props(sp, patch)?;
        }
        if patch.opacity.is_some() || patch.brightness.is_some() || patch.contrast.is_some() || patch.recolor.is_some() || patch.color_change.is_some() {
            let fill = pic.child_mut("pic:blipFill").ok_or("no picture fill")?;
            let blip = fill.child_mut("a:blip").ok_or("no picture")?;
            set_blip(blip, patch)?;
        }
    }
    update_effect_extent(drawing);
    Ok(())
}

fn reset(drawing: &mut Element) {
    // Back to the whole picture at the scale it is shown at.
    let _ = set_crop(drawing, [0.0; 4], false);
    if let Some(pic) = find_mut(drawing, "pic:pic") {
        if let Some(blip) = pic.child_mut("pic:blipFill").and_then(|f| f.child_mut("a:blip")) {
            blip.children.retain(|n| !matches!(n, Node::Element(e) if matches!(e.name.as_str(), "a:alphaModFix" | "a:lum" | "a:grayscl" | "a:biLevel" | "a:duotone" | "a:clrChange" | "a:alphaBiLevel" | "a:tint" | "a:hsl")));
            // Word's own corrections and artistic effects.
            if let Some(ext_lst) = blip.child_mut("a:extLst") {
                ext_lst.children.retain(|n| !matches!(n, Node::Element(e) if e.find("a14:imgProps").is_some()));
            }
            if blip.child("a:extLst").is_some_and(|l| l.children.is_empty()) {
                blip.remove_children("a:extLst");
            }
        }
        if let Some(sp) = pic.child_mut("pic:spPr") {
            if let Some(x) = sp.child_mut("a:xfrm") {
                x.remove_attr("rot");
                x.remove_attr("flipH");
                x.remove_attr("flipV");
            }
            sp.remove_children("a:prstGeom");
            sp.remove_children("a:ln");
            sp.remove_children("a:effectLst");
            put_ordered(sp, Element::new("a:prstGeom").with_attr("prst", "rect").with_child(Element::new("a:avLst")), SPPR_ORDER);
        }
    }
    update_effect_extent(drawing);
}

/// The container (`wp:inline` or `wp:anchor`) of a drawing.
fn container_mut(drawing: &mut Element) -> Option<&mut Element> {
    if drawing.find("wp:inline").is_some() {
        find_mut(drawing, "wp:inline")
    } else {
        find_mut(drawing, "wp:anchor")
    }
}

/// Set the crop. Without a new size in the patch the visible size follows, so the picture keeps its scale; an
/// object placed by an offset moves with the edge that was cropped.
fn set_crop(drawing: &mut Element, crop: [f64; 4], size_given: bool) -> Result<(), String> {
    let mut c = crop;
    for v in &mut c {
        *v = v.clamp(-4.0, 0.98);
    }
    if c[0] + c[2] > 0.98 {
        let k = 0.98 / (c[0] + c[2]);
        c[0] *= k;
        c[2] *= k;
    }
    if c[1] + c[3] > 0.98 {
        let k = 0.98 / (c[1] + c[3]);
        c[1] *= k;
        c[3] *= k;
    }
    let old = {
        let pic = pic_mut(drawing)?;
        let sr = pic.child("pic:blipFill").and_then(|f| f.child("a:srcRect"));
        let g = |a: &str| sr.and_then(|s| num(s, a)).unwrap_or(0.0) / 100000.0;
        [g("l"), g("t"), g("r"), g("b")]
    };
    {
        let pic = pic_mut(drawing)?;
        let fill = pic.child_mut("pic:blipFill").ok_or("no picture fill")?;
        let mut sr = Element::new("a:srcRect");
        for (k, v) in ["l", "t", "r", "b"].iter().zip(c) {
            if v.abs() > 1e-9 {
                sr.set_attr(*k, (v * 100000.0).round().to_string());
            }
        }
        fill.remove_children("a:srcRect");
        // After the blip, before the way it fills the shape.
        let at = fill.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "a:blip")).map(|i| i + 1).unwrap_or(0);
        fill.children.insert(at, Node::Element(sr));
    }
    if size_given {
        return Ok(());
    }
    let (w, h, is_anchor) = {
        let cont = container_mut(drawing).ok_or("not a picture")?;
        let (w, h) = extent_pt(cont);
        (w, h, cont.name == "wp:anchor")
    };
    let full_w = w / (1.0 - old[0] - old[2]).max(0.02);
    let full_h = h / (1.0 - old[1] - old[3]).max(0.02);
    let nw = (full_w * (1.0 - c[0] - c[2])).max(1.0);
    let nh = (full_h * (1.0 - c[1] - c[3])).max(1.0);
    let (cx, cy) = ((nw * EMU_PER_PT).round() as i64, (nh * EMU_PER_PT).round() as i64);
    {
        let cont = container_mut(drawing).ok_or("not a picture")?;
        if let Some(e) = cont.child_mut("wp:extent") {
            e.set_attr("cx", cx.to_string());
            e.set_attr("cy", cy.to_string());
        }
        if is_anchor {
            let shift = |axis: &str, by: f64, cont: &mut Element| {
                if let Some(off) = cont.child_mut(axis).and_then(|p| p.child_mut("wp:posOffset")) {
                    let cur: f64 = off.text().trim().parse().unwrap_or(0.0);
                    off.children = vec![Node::Text(((cur + by * EMU_PER_PT).round() as i64).to_string())];
                }
            };
            shift("wp:positionH", (c[0] - old[0]) * full_w, cont);
            shift("wp:positionV", (c[1] - old[1]) * full_h, cont);
        }
    }
    if let Some(x) = find_mut(drawing, "a:xfrm").and_then(|x| x.child_mut("a:ext")) {
        x.set_attr("cx", cx.to_string());
        x.set_attr("cy", cy.to_string());
    }
    Ok(())
}

fn set_shape_props(sp: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    if patch.rot.is_some() || patch.flip_h.is_some() || patch.flip_v.is_some() {
        if sp.child("a:xfrm").is_none() {
            put_ordered(sp, Element::new("a:xfrm").with_child(Element::new("a:off").with_attr("x", "0").with_attr("y", "0")).with_child(Element::new("a:ext").with_attr("cx", "914400").with_attr("cy", "914400")), SPPR_ORDER);
        }
        let x = sp.child_mut("a:xfrm").ok_or("no transform")?;
        if let Some(r) = patch.rot {
            let r = r.rem_euclid(360.0);
            if r.abs() < 1e-6 {
                x.remove_attr("rot");
            } else {
                x.set_attr("rot", (r * 60000.0).round().to_string());
            }
        }
        for (key, v) in [("flipH", patch.flip_h), ("flipV", patch.flip_v)] {
            match v {
                Some(true) => x.set_attr(key, "1"),
                Some(false) => {
                    x.remove_attr(key);
                }
                None => {}
            }
        }
    }
    if let Some(g) = &patch.geometry {
        if !GEOMETRIES.contains(&g.as_str()) {
            return Err(format!("unknown picture shape '{g}'; use one of {}", GEOMETRIES.join(", ")));
        }
        let mut av = Element::new("a:avLst");
        if let (Some(adj), "roundRect") = (patch.corner, g.as_str()) {
            av = av.with_child(Element::new("a:gd").with_attr("name", "adj").with_attr("fmla", format!("val {}", (adj.clamp(0.0, 0.5) * 100000.0).round() as i64)));
        }
        put_ordered(sp, Element::new("a:prstGeom").with_attr("prst", g.clone()).with_child(av), SPPR_ORDER);
    } else if let (Some(adj), Some(g)) = (patch.corner, sp.child_mut("a:prstGeom")) {
        if g.attr("prst") == Some("roundRect") {
            g.remove_children("a:avLst");
            g.children.push(Node::Element(Element::new("a:avLst").with_child(Element::new("a:gd").with_attr("name", "adj").with_attr("fmla", format!("val {}", (adj.clamp(0.0, 0.5) * 100000.0).round() as i64)))));
        }
    }
    match &patch.border {
        Some(Switch::Off) => {
            sp.remove_children("a:ln");
        }
        Some(Switch::On(l)) => {
            let mut ln = sp.child("a:ln").cloned().unwrap_or_else(|| Element::new("a:ln"));
            ln.remove_children("a:noFill");
            ln.remove_children("a:solidFill");
            ln.remove_children("a:prstDash");
            ln.set_attr("w", emu(l.width_pt.max(0.0)));
            let fill = Element::new("a:solidFill").with_child(color_el(&l.color, None));
            // Order inside a:ln: fill, dash, join.
            ln.children.insert(0, Node::Element(fill));
            if l.dash != "solid" {
                let pos = if ln.child("a:solidFill").is_some() { 1 } else { 0 };
                ln.children.insert(pos, Node::Element(Element::new("a:prstDash").with_attr("val", l.dash.clone())));
            }
            put_ordered(sp, ln, SPPR_ORDER);
        }
        None => {}
    }
    if patch.shadow.is_some() || patch.glow.is_some() || patch.soft_edge.is_some() || patch.reflection.is_some() {
        let mut fx = sp.child("a:effectLst").cloned().unwrap_or_else(|| Element::new("a:effectLst"));
        match &patch.shadow {
            Some(Switch::Off) => {
                fx.remove_children("a:outerShdw");
            }
            Some(Switch::On(s)) => {
                let e = Element::new("a:outerShdw")
                    .with_attr("blurRad", emu(s.blur_pt))
                    .with_attr("dist", emu(s.dist_pt))
                    .with_attr("dir", ((s.angle.rem_euclid(360.0)) * 60000.0).round().to_string())
                    .with_attr("algn", "tl")
                    .with_attr("rotWithShape", "0")
                    .with_child(color_el(&s.color, Some(s.alpha)));
                put_ordered(&mut fx, e, FX_ORDER);
            }
            None => {}
        }
        match &patch.glow {
            Some(Switch::Off) => {
                fx.remove_children("a:glow");
            }
            Some(Switch::On(g)) => put_ordered(&mut fx, Element::new("a:glow").with_attr("rad", emu(g.radius_pt)).with_child(color_el(&g.color, Some(g.alpha))), FX_ORDER),
            None => {}
        }
        match patch.soft_edge {
            Some(r) if r > 0.0 => put_ordered(&mut fx, Element::new("a:softEdge").with_attr("rad", emu(r)), FX_ORDER),
            Some(_) => {
                fx.remove_children("a:softEdge");
            }
            None => {}
        }
        match &patch.reflection {
            Some(Switch::Off) => {
                fx.remove_children("a:reflection");
            }
            Some(Switch::On(r)) => {
                let e = Element::new("a:reflection")
                    .with_attr("blurRad", emu(r.blur_pt))
                    .with_attr("stA", ((r.alpha.clamp(0.0, 1.0)) * 100000.0).round().to_string())
                    .with_attr("stPos", "0")
                    .with_attr("endA", "300")
                    .with_attr("endPos", ((r.size.clamp(0.0, 1.0)) * 100000.0).round().to_string())
                    .with_attr("dist", emu(r.dist_pt))
                    .with_attr("dir", "5400000")
                    .with_attr("sy", "-100000")
                    .with_attr("algn", "bl")
                    .with_attr("rotWithShape", "0");
                put_ordered(&mut fx, e, FX_ORDER);
            }
            None => {}
        }
        if fx.children.is_empty() {
            sp.remove_children("a:effectLst");
        } else {
            put_ordered(sp, fx, SPPR_ORDER);
        }
    }
    Ok(())
}

/// A blip child goes before the extension list, which is always last.
fn blip_put(blip: &mut Element, e: Element) {
    let at = blip.children.iter().position(|n| matches!(n, Node::Element(x) if x.name == "a:extLst")).unwrap_or(blip.children.len());
    blip.children.insert(at, Node::Element(e));
}

fn set_blip(blip: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    if let Some(o) = patch.opacity {
        blip.remove_children("a:alphaModFix");
        if o < 0.999 {
            blip_put(blip, Element::new("a:alphaModFix").with_attr("amt", ((o.clamp(0.0, 1.0)) * 100000.0).round().to_string()));
        }
    }
    if patch.brightness.is_some() || patch.contrast.is_some() {
        let cur = blip.child("a:lum").cloned();
        let g = |a: &str| cur.as_ref().and_then(|l| num(l, a)).unwrap_or(0.0) / 100000.0;
        let b = patch.brightness.unwrap_or_else(|| g("bright")).clamp(-1.0, 1.0);
        let c = patch.contrast.unwrap_or_else(|| g("contrast")).clamp(-1.0, 1.0);
        blip.remove_children("a:lum");
        if b.abs() > 1e-9 || c.abs() > 1e-9 {
            let mut l = Element::new("a:lum");
            if b.abs() > 1e-9 {
                l.set_attr("bright", (b * 100000.0).round().to_string());
            }
            if c.abs() > 1e-9 {
                l.set_attr("contrast", (c * 100000.0).round().to_string());
            }
            blip_put(blip, l);
        }
    }
    match &patch.recolor {
        Some(Switch::Off) => {
            for n in BLIP_COLOR {
                blip.remove_children(n);
            }
        }
        Some(Switch::On(r)) => {
            for n in BLIP_COLOR {
                blip.remove_children(n);
            }
            let e = match r.kind.as_str() {
                "grayscale" => Element::new("a:grayscl"),
                "blackWhite" => Element::new("a:biLevel").with_attr("thresh", ((r.threshold.unwrap_or(0.5).clamp(0.0, 1.0)) * 100000.0).round().to_string()),
                "sepia" => Element::new("a:duotone")
                    .with_child(Element::new("a:prstClr").with_attr("val", "black"))
                    .with_child(Element::new("a:srgbClr").with_attr("val", "D9C3A5").with_child(Element::new("a:tint").with_attr("val", "50000")).with_child(Element::new("a:satMod").with_attr("val", "180000"))),
                "duotone" => Element::new("a:duotone")
                    .with_child(color_el(r.color1.as_deref().unwrap_or("000000"), None))
                    .with_child(color_el(r.color2.as_deref().unwrap_or("FFFFFF"), None)),
                other => return Err(format!("unknown recolour '{other}'; use grayscale, blackWhite, sepia or duotone")),
            };
            blip_put(blip, e);
        }
        None => {}
    }
    match &patch.color_change {
        Some(Switch::Off) => {
            blip.remove_children("a:clrChange");
        }
        Some(Switch::On(cc)) => {
            blip.remove_children("a:clrChange");
            let to = match &cc.to {
                Some(t) => color_el(t, None),
                None => color_el(&cc.from, Some(0.0)),
            };
            blip_put(blip, Element::new("a:clrChange").with_child(Element::new("a:clrFrom").with_child(color_el(&cc.from, None))).with_child(Element::new("a:clrTo").with_child(to)));
        }
        None => {}
    }
    Ok(())
}

/// Bring `wp:effectExtent` up to date after a resize, for a picture whose turn, outline or effects need room.
pub fn refresh_effect_extent(drawing: &mut Element) {
    let f = read(drawing, &ShapeTheme::default(), &|_| None);
    if f.rot.is_some() || f.line.is_some() || f.shadow.is_some() || f.glow.is_some() || f.reflection.is_some() {
        update_effect_extent(drawing);
    }
}

/// How a floating picture is placed, beyond what `ImageLayout` says.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceInfo {
    /// Distance of the text from the picture: top, bottom, left, right in points.
    pub dist: [f64; 4],
    /// `bothSides`, `left`, `right` or `largest`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap_text: Option<String>,
    /// The wrap is Word's "through" (the same as "tight" here: the text follows the box).
    #[serde(skip_serializing_if = "not")]
    pub through: bool,
    #[serde(skip_serializing_if = "not")]
    pub locked: bool,
    /// Word's "Allow overlap" (on unless the file says otherwise).
    pub overlap: bool,
    /// The horizontal alignment as written (`left`, `center`, `right`, `inside`, `outside`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_h: Option<String>,
}

/// The placement details of a floating picture; None for one in line with the text.
pub fn read_place(drawing: &Element) -> Option<PlaceInfo> {
    let a = drawing.find("wp:anchor")?;
    let d = |k: &str| num(a, k).unwrap_or(0.0) / EMU_PER_PT;
    let wrap = ["wp:wrapSquare", "wp:wrapTight", "wp:wrapThrough"].iter().find_map(|n| a.child(n));
    let on = |v: Option<&str>, default: bool| match v {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => default,
    };
    Some(PlaceInfo {
        dist: [d("distT"), d("distB"), d("distL"), d("distR")],
        wrap_text: wrap.and_then(|w| w.attr("wrapText")).map(String::from),
        through: a.child("wp:wrapThrough").is_some(),
        locked: on(a.attr("locked"), false),
        overlap: on(a.attr("allowOverlap"), true),
        align_h: a.child("wp:positionH").and_then(|p| p.child("wp:align")).map(|x| x.text().trim().to_string()),
    })
}

/// Write `wp:effectExtent` to hold what the picture's rotation, outline and effects reach beyond its box.
fn update_effect_extent(drawing: &mut Element) {
    let (w, h) = match container_mut(drawing) {
        Some(c) => extent_pt(c),
        None => return,
    };
    let f = read(drawing, &ShapeTheme::default(), &|_| None);
    let (rot, ln, shadow, glow, reflection) = (f.rot.unwrap_or(0.0), f.line.as_ref().map(|l| l.width_pt).unwrap_or(0.0), f.shadow.clone(), f.glow.clone(), f.reflection.clone());
    // The turned box, then the outline (half of it lies outside), the glow and the shadow.
    let a = rot.to_radians();
    let bw = (w * a.cos().abs() + h * a.sin().abs()).max(w);
    let bh = (w * a.sin().abs() + h * a.cos().abs()).max(h);
    let mut l = (bw - w) / 2.0;
    let mut r = l;
    let mut t = (bh - h) / 2.0;
    let mut b = t;
    let grow = ln / 2.0 + glow.map(|g| g.radius_pt).unwrap_or(0.0);
    l += grow;
    r += grow;
    t += grow;
    b += grow;
    if let Some(s) = shadow {
        let rad = s.angle.to_radians();
        let (dx, dy) = (s.dist_pt * rad.cos(), s.dist_pt * rad.sin());
        l = l.max(s.blur_pt - dx + grow);
        r = r.max(s.blur_pt + dx + grow);
        t = t.max(s.blur_pt - dy + grow);
        b = b.max(s.blur_pt + dy + grow);
    }
    // The mirror image below the picture.
    if let Some(r) = reflection {
        b = b.max(r.dist_pt + h * r.size + r.blur_pt);
    }
    let Some(cont) = container_mut(drawing) else { return };
    let ext = |v: f64| emu(v.max(0.0));
    let mut ee = Element::new("wp:effectExtent");
    ee.set_attr("l", ext(l));
    ee.set_attr("t", ext(t));
    ee.set_attr("r", ext(r));
    ee.set_attr("b", ext(b));
    if let Some(slot) = cont.child_mut("wp:effectExtent") {
        *slot = ee;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn drawing(anchor: bool) -> Element {
        let inner = r#"<wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="1" name="P"/><a:graphic><a:graphicData><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="P"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId5"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>"#;
        let xml_s = if anchor {
            format!(r#"<w:drawing xmlns:w="w" xmlns:wp="wp" xmlns:a="a" xmlns:pic="pic" xmlns:r="r"><wp:anchor distT="0"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>127000</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>254000</wp:posOffset></wp:positionV>{inner}</wp:anchor></w:drawing>"#)
        } else {
            format!(r#"<w:drawing xmlns:w="w" xmlns:wp="wp" xmlns:a="a" xmlns:pic="pic" xmlns:r="r"><wp:inline>{inner}</wp:inline></w:drawing>"#)
        };
        xml::parse(xml_s.as_bytes()).unwrap().root
    }

    fn view(d: &Element) -> PicFmt {
        read(d, &ShapeTheme::default(), &|id| Some((format!("word/media/{id}.svg"), false)))
    }

    fn patch(json: &str) -> ImagePatch {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn crop_keeps_the_scale_and_the_offset() {
        let mut d = drawing(true);
        apply(&mut d, &patch(r#"{"crop":[0.1,0.2,0.0,0.0]}"#)).unwrap();
        let f = view(&d);
        assert_eq!(f.crop, Some([0.1, 0.2, 0.0, 0.0]));
        // 100 x 50 pt shown, now without 10% of the width and 20% of the height of the whole picture.
        let ext = d.find("wp:extent").unwrap();
        assert_eq!(ext.attr("cx"), Some("1143000"));
        assert_eq!(ext.attr("cy"), Some("508000"));
        assert_eq!(d.find("a:ext").unwrap().attr("cx"), Some("1143000"));
        // The left and top edges moved with the crop.
        assert_eq!(d.find("wp:positionH").unwrap().child("wp:posOffset").unwrap().text(), "254000");
        assert_eq!(d.find("wp:positionV").unwrap().child("wp:posOffset").unwrap().text(), "381000");
        // Cropping again from the other side keeps the scale from the first crop.
        apply(&mut d, &patch(r#"{"crop":[0.1,0.2,0.1,0.0]}"#)).unwrap();
        let ext = d.find("wp:extent").unwrap();
        assert_eq!(ext.attr("cx"), Some("1016000"));
        // A negative crop pads.
        apply(&mut d, &patch(r#"{"crop":[-0.1,0,0,0]}"#)).unwrap();
        assert_eq!(view(&d).crop, Some([-0.1, 0.0, 0.0, 0.0]));
        assert!(d.find("a:srcRect").unwrap().attr("l") == Some("-10000"));
        // The crop sits between the blip and the stretch.
        let fill = d.find("pic:blipFill").unwrap();
        let names: Vec<_> = fill.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a:blip", "a:srcRect", "a:stretch"]);
    }

    #[test]
    fn crop_with_a_size_leaves_the_size_to_the_caller() {
        let mut d = drawing(false);
        apply(&mut d, &patch(r#"{"crop":[0.25,0,0.25,0],"width":50,"height":50}"#)).unwrap();
        assert_eq!(view(&d).crop, Some([0.25, 0.0, 0.25, 0.0]));
    }

    #[test]
    fn rotation_flips_shape_and_extent() {
        let mut d = drawing(false);
        apply(&mut d, &patch(r#"{"rot":90,"flip_h":true,"geometry":"ellipse"}"#)).unwrap();
        let f = view(&d);
        assert_eq!((f.rot, f.flip_h, f.flip_v, f.geom.as_deref()), (Some(90.0), true, false, Some("ellipse")));
        let x = d.find("a:xfrm").unwrap();
        assert_eq!(x.attr("rot"), Some("5400000"));
        assert_eq!(x.attr("flipH"), Some("1"));
        // 100 x 50 turned a quarter is 50 wide and 100 tall: 25 pt more room above and below, none at the sides.
        let ee = d.find("wp:effectExtent").unwrap();
        assert_eq!((ee.attr("l"), ee.attr("t"), ee.attr("r"), ee.attr("b")), (Some("0"), Some("317500"), Some("0"), Some("317500")));
        apply(&mut d, &patch(r#"{"rot":0,"flip_h":false,"geometry":"rect"}"#)).unwrap();
        let f = view(&d);
        assert_eq!((f.rot, f.flip_h, f.geom), (None, false, None));
        assert!(d.find("a:xfrm").unwrap().attr("rot").is_none());
        let ee = d.find("wp:effectExtent").unwrap();
        assert_eq!(ee.attr("l"), Some("0"));
        assert!(apply(&mut d, &patch(r#"{"geometry":"banana"}"#)).is_err());
    }

    #[test]
    fn outline_and_effects_in_schema_order() {
        let mut d = drawing(false);
        apply(
            &mut d,
            &patch(r##"{"border":{"color":"#FF4F00","width_pt":2,"dash":"dash"},"shadow":{"blur_pt":5,"dist_pt":4,"angle":45,"color":"000000","alpha":0.5},"glow":{"radius_pt":6,"color":"accent1","alpha":0.4},"soft_edge":3,"reflection":{"size":0.5,"alpha":0.4,"dist_pt":1}}"##),
        )
        .unwrap();
        let sp = d.find("pic:spPr").unwrap();
        let names: Vec<_> = sp.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a:xfrm", "a:prstGeom", "a:ln", "a:effectLst"]);
        let ln = sp.child("a:ln").unwrap();
        assert_eq!(ln.attr("w"), Some("25400"));
        assert_eq!(ln.child("a:solidFill").unwrap().child("a:srgbClr").unwrap().attr("val"), Some("FF4F00"));
        assert_eq!(ln.child("a:prstDash").unwrap().attr("val"), Some("dash"));
        let fx: Vec<_> = sp.child("a:effectLst").unwrap().elements().map(|e| e.name.as_str()).collect();
        assert_eq!(fx, ["a:glow", "a:outerShdw", "a:reflection", "a:softEdge"]);
        let f = view(&d);
        assert_eq!(f.line, Some(PicLine { color: "FF4F00".into(), width_pt: 2.0, dash: "dash".into() }));
        assert_eq!(f.shadow.as_ref().map(|s| (s.blur_pt, s.dist_pt, s.angle, s.alpha)), Some((5.0, 4.0, 45.0, 0.5)));
        assert_eq!(f.glow.as_ref().map(|g| (g.radius_pt, g.color.clone())), Some((6.0, "5B9BD5".to_string())));
        assert_eq!(f.soft_edge, Some(3.0));
        assert_eq!(f.reflection.as_ref().map(|r| r.size), Some(0.5));
        // Room for the outline, glow and shadow.
        let ee = d.find("wp:effectExtent").unwrap();
        assert!(ee.attr("r").unwrap().parse::<i64>().unwrap() > 76200);
        // Take them off again.
        apply(&mut d, &patch(r#"{"border":"none","shadow":"none","glow":"none","soft_edge":0,"reflection":"none"}"#)).unwrap();
        let sp = d.find("pic:spPr").unwrap();
        assert!(sp.child("a:ln").is_none() && sp.child("a:effectLst").is_none());
        assert!(view(&d).is_empty());
    }

    #[test]
    fn adjustments_are_blip_transforms() {
        let mut d = drawing(false);
        apply(&mut d, &patch(r#"{"opacity":0.5,"brightness":0.2,"contrast":-0.4,"recolor":{"kind":"grayscale"}}"#)).unwrap();
        let blip = d.find("a:blip").unwrap();
        assert_eq!(blip.child("a:alphaModFix").unwrap().attr("amt"), Some("50000"));
        assert_eq!(blip.child("a:lum").unwrap().attr("bright"), Some("20000"));
        assert_eq!(blip.child("a:lum").unwrap().attr("contrast"), Some("-40000"));
        assert!(blip.child("a:grayscl").is_some());
        let f = view(&d);
        assert_eq!((f.opacity, f.bright, f.contrast), (Some(0.5), Some(0.2), Some(-0.4)));
        assert_eq!(f.recolor.as_ref().map(|r| r.kind.as_str()), Some("grayscale"));
        // One at a time: the other stays.
        apply(&mut d, &patch(r#"{"brightness":0}"#)).unwrap();
        assert_eq!(view(&d).contrast, Some(-0.4));
        assert!(view(&d).bright.is_none());
        apply(&mut d, &patch(r##"{"recolor":{"kind":"duotone","color1":"#112233","color2":"accent2"}}"##)).unwrap();
        let f = view(&d);
        assert_eq!(f.recolor.as_ref().map(|r| (r.kind.as_str(), r.color1.clone().unwrap(), r.color2.clone().unwrap())), Some(("duotone", "112233".to_string(), "ED7D31".to_string())));
        assert!(d.find("a:grayscl").is_none() && d.find("a:duotone").is_some());
        apply(&mut d, &patch(r#"{"recolor":{"kind":"blackWhite","threshold":0.25}}"#)).unwrap();
        assert_eq!(view(&d).recolor.unwrap().threshold, Some(0.25));
        apply(&mut d, &patch(r#"{"recolor":{"kind":"sepia"}}"#)).unwrap();
        assert_eq!(view(&d).recolor.unwrap().kind, "duotone");
        apply(&mut d, &patch(r#"{"recolor":"none","opacity":1,"contrast":0}"#)).unwrap();
        assert!(view(&d).is_empty());
        assert!(d.find("a:alphaModFix").is_none());
        // The blip keeps its embed.
        assert_eq!(d.find("a:blip").unwrap().attr("r:embed"), Some("rId5"));
    }

    #[test]
    fn transparent_colour() {
        let mut d = drawing(false);
        apply(&mut d, &patch(r#"{"color_change":{"from":"FFFFFF"}}"#)).unwrap();
        assert_eq!(view(&d).clr_change, Some(ColorChange { from: "FFFFFF".into(), to: None }));
        apply(&mut d, &patch(r#"{"color_change":{"from":"FFFFFF","to":"00FF00"}}"#)).unwrap();
        assert_eq!(view(&d).clr_change, Some(ColorChange { from: "FFFFFF".into(), to: Some("00FF00".into()) }));
        apply(&mut d, &patch(r#"{"color_change":"none"}"#)).unwrap();
        assert!(view(&d).clr_change.is_none());
    }

    #[test]
    fn reset_removes_the_formatting_but_not_the_picture() {
        let mut d = drawing(false);
        apply(&mut d, &patch(r#"{"crop":[0.1,0.1,0.1,0.1],"rot":30,"opacity":0.4,"border":{"color":"000000","width_pt":1},"shadow":{},"recolor":{"kind":"grayscale"},"geometry":"ellipse"}"#)).unwrap();
        assert!(!view(&d).is_empty());
        apply(&mut d, &patch(r#"{"reset":true}"#)).unwrap();
        assert!(view(&d).is_empty(), "{:?}", view(&d));
        // The whole picture at the scale it was shown at.
        let ext = d.find("wp:extent").unwrap();
        assert_eq!(ext.attr("cx"), Some("1270000"));
        assert_eq!(d.find("a:blip").unwrap().attr("r:embed"), Some("rId5"));
    }

    #[test]
    fn reads_what_word_writes() {
        let d = xml::parse(
            br#"<w:drawing xmlns:w="w" xmlns:wp="wp" xmlns:a="a" xmlns:pic="pic" xmlns:r="r" xmlns:a14="a14" xmlns:asvg="asvg"><wp:inline><wp:extent cx="1270000" cy="635000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="1" name="P"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId5"><a:grayscl/><a:lum bright="70000" contrast="-70000"/><a:extLst><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip r:embed="rId9"/></a:ext></a:extLst></a:blip><a:srcRect l="10000" t="-5000" r="2500" b="0"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm rot="2700000" flipV="1"><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 25000"/></a:avLst></a:prstGeom><a:ln w="19050"><a:solidFill><a:schemeClr val="accent2"/></a:solidFill><a:prstDash val="sysDot"/></a:ln><a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000" algn="tl" rotWithShape="0"><a:prstClr val="black"><a:alpha val="40000"/></a:prstClr></a:outerShdw></a:effectLst></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#,
        )
        .unwrap()
        .root;
        let f = read(&d, &ShapeTheme::default(), &|id| Some((format!("word/media/{id}.svg"), false)));
        assert_eq!(f.crop, Some([0.1, -0.05, 0.025, 0.0]));
        assert_eq!((f.rot, f.flip_v, f.geom.as_deref(), f.adj), (Some(45.0), true, Some("roundRect"), Some(0.25)));
        assert_eq!(f.line, Some(PicLine { color: "ED7D31".into(), width_pt: 1.5, dash: "sysDot".into() }));
        assert_eq!((f.bright, f.contrast), (Some(0.7), Some(-0.7)));
        assert_eq!(f.recolor.as_ref().map(|r| r.kind.as_str()), Some("grayscale"));
        assert_eq!(f.shadow.as_ref().map(|s| (s.blur_pt, s.dist_pt, s.angle, s.color.as_str(), s.alpha)), Some((4.0, 3.0, 45.0, "000000", 0.4)));
        assert_eq!(f.svg.as_deref(), Some("word/media/rId9.svg"));
    }

    #[test]
    fn office_2010_corrections_are_not_applied_twice() {
        // The blip is a corrected copy: the layer's picture is another part.
        let baked = xml::parse(
            br#"<w:drawing xmlns:w="w" xmlns:wp="wp" xmlns:a="a" xmlns:pic="pic" xmlns:r="r" xmlns:a14="a14"><wp:inline><wp:extent cx="10" cy="10"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId5"><a:extLst><a:ext><a14:imgProps><a14:imgLayer r:embed="rId6"><a14:imgEffect><a14:brightnessContrast bright="20000" contrast="-40000"/></a14:imgEffect><a14:imgEffect><a14:artisticBlur radius="12"/></a14:imgEffect></a14:imgLayer></a14:imgProps></a:ext></a:extLst></a:blip></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#,
        )
        .unwrap()
        .root;
        let f = read(&baked, &ShapeTheme::default(), &|_| None);
        assert!(f.bright.is_none() && f.contrast.is_none());
        assert_eq!(f.art, Some(("artisticBlur".to_string(), 12.0)));
        let same = xml::parse(
            br#"<w:drawing xmlns:w="w" xmlns:wp="wp" xmlns:a="a" xmlns:pic="pic" xmlns:r="r" xmlns:a14="a14"><wp:inline><wp:extent cx="10" cy="10"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rId5"><a:extLst><a:ext><a14:imgProps><a14:imgLayer r:embed="rId5"><a14:imgEffect><a14:brightnessContrast bright="20000" contrast="-40000"/></a14:imgEffect></a14:imgLayer></a14:imgProps></a:ext></a:extLst></a:blip></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#,
        )
        .unwrap()
        .root;
        let f = read(&same, &ShapeTheme::default(), &|_| None);
        assert_eq!((f.bright, f.contrast), (Some(0.2), Some(-0.4)));
    }

    #[test]
    fn vml_crop() {
        let e = xml::parse(br#"<v:imagedata xmlns:v="v" cropleft="10%" croptop="16384f" cropright="0"/>"#).unwrap().root;
        assert_eq!(read_vml_crop(&e), Some([0.1, 0.25, 0.0, 0.0]));
    }
}
