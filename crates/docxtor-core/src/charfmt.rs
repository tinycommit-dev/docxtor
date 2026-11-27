//! Character formatting beyond the basics: underline colours, raised and lowered text, character scale, kerning,
//! outline, shadow, emboss and imprint, text borders, patterned and theme shading, theme colours, emphasis marks,
//! fit text and Word 2010's text effects (`w14:`).
//!
//! `RunExt` is flattened into `RunFmt` (see props.rs), so the view model and `FormatState` carry these fields
//! beside the old ones. `RunExtPatch` is flattened into `RunPatch` the same way. Reading takes the file as it is;
//! `resolve_run` turns theme references into hex colours once a theme is at hand. Writing patches the raw `w:rPr`
//! in place and touches only the elements a patch names, so effects Docxtor doesn't model (glow shape, 3-D) survive.

use crate::props::{put_rpr, Border, RunFmt};
use crate::shading::{theme_hex, Shd};
use crate::shapes::theme::ShapeTheme;
use crate::xml::Element;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};

/// A theme colour reference as `w:color` and `w:u` write it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeRef {
    pub name: String,
    /// Hex 00 to FF, towards white.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    /// Hex 00 to FF, towards black.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<String>,
}

impl ThemeRef {
    fn from_attrs(e: &Element, name: &str, tint: &str, shade: &str) -> Option<ThemeRef> {
        let n = crate::colour::theme_name(e.attr(name)?)?;
        Some(ThemeRef { name: n, tint: e.attr(tint).and_then(crate::colour::two_hex), shade: e.attr(shade).and_then(crate::colour::two_hex) })
    }

    pub fn hex(&self, theme: &ShapeTheme) -> Option<String> {
        theme_hex(theme, Some(self.name.clone()), self.tint.clone(), self.shade.clone())
    }
}

/// A colour element as the file has it: kind, value and transforms.
type RawColor = (String, String, Vec<(String, f64)>);

/// A colour inside a `w14:` effect: hex and alpha once resolved against the theme.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxColor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
    /// 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    /// As the file has it: element kind (`srgbClr`, `schemeClr`, ...), its value, and its transforms (`lumMod`, `alpha`).
    #[serde(skip)]
    raw: Option<RawColor>,
}

impl FxColor {
    fn from_el(c: &Element) -> Option<FxColor> {
        let kind = c.local().to_string();
        if !matches!(kind.as_str(), "srgbClr" | "schemeClr" | "sysClr" | "prstClr" | "scrgbClr" | "hslClr") {
            return None;
        }
        let val = c.attr("w14:val").or(c.attr("w14:lastClr")).unwrap_or("").to_string();
        let mods = c.elements().filter_map(|m| m.attr("w14:val").and_then(|v| v.parse::<f64>().ok()).map(|v| (m.local().to_string(), v / 100000.0))).collect::<Vec<_>>();
        let mut f = FxColor { raw: Some((kind.clone(), val.clone(), mods.clone())), ..Default::default() };
        // A plain hex colour is usable before the theme is read.
        if let Some(h) = Some(kind.as_str()).filter(|k| *k == "srgbClr").and(crate::colour::hex(&val)) {
            f.hex = Some(h);
            f.alpha = mods.iter().find(|(n, _)| n == "alpha").map(|(_, v)| *v);
        }
        Some(f)
    }

    fn resolve(&mut self, theme: &ShapeTheme) {
        let Some((kind, val, mods)) = self.raw.take() else { return };
        let mut e = Element::new(format!("a:{kind}"));
        e.set_attr("val", &val);
        if kind == "sysClr" {
            e.set_attr("lastClr", &val);
        }
        for (n, v) in &mods {
            e.children.push(crate::xml::Node::Element(Element::new(format!("a:{n}")).with_attr("val", format!("{}", (v * 100000.0).round() as i64))));
        }
        let wrap = Element::new("a:solidFill").with_child(e);
        if let Some((hex, alpha)) = theme.color_of(&wrap, None) {
            self.hex = Some(hex);
            self.alpha = (alpha < 0.999).then_some(alpha);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxStop {
    /// 0 to 1.
    pub pos: f64,
    pub color: FxColor,
}

/// A text fill (`w14:textFill`): one colour, or a gradient.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxFill {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solid: Option<FxColor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stops: Vec<FxStop>,
    /// Degrees, clockwise from the x axis (as DrawingML counts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxGlow {
    /// Radius in points.
    pub radius: f64,
    pub color: Option<FxColor>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxShadow {
    /// Points.
    pub blur: f64,
    pub dist: f64,
    /// Degrees.
    pub dir: f64,
    pub color: Option<FxColor>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxReflection {
    pub blur: f64,
    pub dist: f64,
    /// 0 to 1: how visible the reflection starts and ends.
    pub start_alpha: f64,
    pub end_alpha: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FxOutline {
    /// Points.
    pub width: f64,
    pub color: Option<FxColor>,
}

/// Word 2010 text effects and OpenType features, from the `w14:` elements of `w:rPr`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFx {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<FxGlow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<FxShadow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection: Option<FxReflection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<FxOutline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<FxFill>,
    /// `none`, `standard`, `contextual`, `historical`, `discretional` and their combinations, `all`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<String>,
    /// `default`, `lining`, `oldStyle`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_form: Option<String>,
    /// `default`, `proportional`, `tabular`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_spacing: Option<String>,
    /// Stylistic sets that are on (1 to 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_sets: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cntxt_alts: Option<bool>,
}

impl TextFx {
    fn is_empty(&self) -> bool {
        *self == TextFx::default()
    }
}

/// Formatting `RunFmt` carries besides bold, italic, size and the like (see the module comment).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunExt {
    /// The theme colour behind `color` (`w:color w:themeColor`); `color` holds the hex once resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_theme: Option<ThemeRef>,
    /// The underline's own colour (hex, or `auto`: the text colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_theme: Option<ThemeRef>,
    /// Raised (positive) or lowered (negative) text, in half-points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    /// Character width as a percentage (`w:w`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<i64>,
    /// Kern text from this size up, in half-points; 0 is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kern: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emboss: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imprint: Option<bool>,
    /// A box around the text (`w:bdr`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    /// The shading as the file has it (a pattern, theme colours); `shading` is the flat fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shd: Option<Shd>,
    /// Emphasis mark: `dot`, `comma`, `circle`, `underDot`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub em: Option<String>,
    /// Fit the text into this width, in twips (`w:fitText`), and the id that groups runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_text: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_text_id: Option<i64>,
    /// Hidden by the text style rather than `w:vanish` (kept for completeness: Word's `w:specVanish`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_vanish: Option<bool>,
    /// Spelling and grammar are not checked in this text (`w:noProof`, Word's "Do not check spelling or grammar").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_proof: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx: Option<Box<TextFx>>,
    /// East Asian layout, grid snapping and language (see east_asian.rs).
    #[serde(flatten)]
    pub ea: crate::east_asian::RunEa,
}

fn on_off(e: &Element) -> bool {
    !matches!(e.attr("w:val"), Some("0") | Some("false") | Some("off"))
}

fn int(e: &Element, a: &str) -> Option<i64> {
    e.attr(a).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64)
}

fn flt(e: &Element, a: &str) -> Option<f64> {
    e.attr(a).and_then(|v| v.parse::<f64>().ok())
}

const EMU_PT: f64 = 12700.0;

fn first_color(e: &Element) -> Option<FxColor> {
    e.elements().find_map(FxColor::from_el)
}

fn read_fill(e: &Element) -> Option<FxFill> {
    for c in e.elements() {
        match c.name.as_str() {
            "w14:solidFill" => return first_color(c).map(|solid| FxFill { solid: Some(solid), ..Default::default() }),
            "w14:gradFill" => {
                let stops: Vec<FxStop> = c
                    .child("w14:gsLst")
                    .map(|l| l.children_named("w14:gs").filter_map(|g| Some(FxStop { pos: flt(g, "w14:pos")? / 100000.0, color: first_color(g)? })).collect())
                    .unwrap_or_default();
                let angle = c.child("w14:lin").and_then(|l| flt(l, "w14:ang")).map(|a| a / 60000.0);
                return Some(FxFill { solid: None, stops, angle });
            }
            _ => {}
        }
    }
    None
}

impl RunExt {
    /// Take in one child of a `w:rPr` (the ones this module doesn't know are ignored).
    pub fn read(&mut self, c: &Element) {
        self.ea.read(c);
        match c.name.as_str() {
            "w:color" => self.color_theme = ThemeRef::from_attrs(c, "w:themeColor", "w:themeTint", "w:themeShade").filter(|t| t.name != "none"),
            "w:u" => {
                self.underline_color = c.attr("w:color").and_then(crate::colour::auto_or_hex);
                self.underline_theme = ThemeRef::from_attrs(c, "w:themeColor", "w:themeTint", "w:themeShade");
            }
            "w:position" => self.position = int(c, "w:val"),
            "w:w" => self.scale = int(c, "w:val"),
            "w:kern" => self.kern = int(c, "w:val"),
            "w:outline" => self.outline = Some(on_off(c)),
            "w:shadow" => self.shadow = Some(on_off(c)),
            "w:emboss" => self.emboss = Some(on_off(c)),
            "w:imprint" => self.imprint = Some(on_off(c)),
            "w:bdr" => {
                self.border = c.attr("w:val").map(|v| {
                    if v == "nil" || v == "none" {
                        Border { val: "none".into(), ..Default::default() }
                    } else {
                        Border { val: v.to_string(), sz: int(c, "w:sz").unwrap_or(4), color: c.attr("w:color").and_then(crate::colour::auto_or_hex).unwrap_or_else(|| "auto".into()), space: int(c, "w:space").unwrap_or(0) }
                    }
                })
            }
            "w:shd" => self.shd = Some(Shd::from_el(c)),
            "w:em" => self.em = c.attr("w:val").map(String::from),
            "w:fitText" => {
                self.fit_text = int(c, "w:val");
                self.fit_text_id = int(c, "w:id");
            }
            "w:specVanish" => self.spec_vanish = Some(on_off(c)),
            "w:noProof" => self.no_proof = Some(on_off(c)),
            "w14:glow" => self.fx().glow = Some(FxGlow { radius: flt(c, "w14:rad").unwrap_or(0.0) / EMU_PT, color: first_color(c) }),
            "w14:shadow" => {
                self.fx().shadow = Some(FxShadow {
                    blur: flt(c, "w14:blurRad").unwrap_or(0.0) / EMU_PT,
                    dist: flt(c, "w14:dist").unwrap_or(0.0) / EMU_PT,
                    dir: flt(c, "w14:dir").unwrap_or(0.0) / 60000.0,
                    color: first_color(c),
                })
            }
            "w14:reflection" => {
                self.fx().reflection = Some(FxReflection {
                    blur: flt(c, "w14:blurRad").unwrap_or(0.0) / EMU_PT,
                    dist: flt(c, "w14:dist").unwrap_or(0.0) / EMU_PT,
                    start_alpha: flt(c, "w14:stA").unwrap_or(100000.0) / 100000.0,
                    end_alpha: flt(c, "w14:endA").unwrap_or(0.0) / 100000.0,
                })
            }
            "w14:textOutline" => {
                let color = c.child("w14:solidFill").and_then(first_color);
                self.fx().outline = Some(FxOutline { width: flt(c, "w14:w").unwrap_or(9525.0) / EMU_PT, color });
            }
            "w14:textFill" => self.fx().fill = read_fill(c),
            "w14:ligatures" => self.fx().ligatures = c.attr("w14:val").map(String::from),
            "w14:numForm" => self.fx().num_form = c.attr("w14:val").map(String::from),
            "w14:numSpacing" => self.fx().num_spacing = c.attr("w14:val").map(String::from),
            "w14:stylisticSets" => {
                let on: Vec<u8> = c
                    .children_named("w14:styleSet")
                    .filter(|s| !matches!(s.attr("w14:val"), Some("0") | Some("false")))
                    .filter_map(|s| int(s, "w14:id").and_then(|n| u8::try_from(n).ok()))
                    .collect();
                self.fx().style_sets = Some(on);
            }
            "w14:cntxtAlts" => self.fx().cntxt_alts = Some(!matches!(c.attr("w14:val"), Some("0") | Some("false"))),
            _ => {}
        }
    }

    fn fx(&mut self) -> &mut TextFx {
        self.fx.get_or_insert_with(Default::default)
    }

    /// `top` over `self`, field by field. The effects layer one by one too.
    pub fn overlay(&self, top: &RunExt) -> RunExt {
        macro_rules! pick {
            ($($f:ident),*) => { RunExt { $($f: top.$f.clone().or_else(|| self.$f.clone()),)* fx: overlay_fx(&self.fx, &top.fx), ea: self.ea.overlay(&top.ea) } };
        }
        pick!(
            color_theme, underline_color, underline_theme, position, scale, kern, outline, shadow, emboss, imprint, border, shd, em, fit_text, fit_text_id, spec_vanish, no_proof
        )
    }

    /// Replace theme references by the hex colours they stand for.
    pub fn resolve(&mut self, theme: &ShapeTheme) {
        if let Some(s) = &mut self.shd {
            s.resolve(theme);
        }
        if let Some(fx) = &mut self.fx {
            let one = |c: &mut Option<FxColor>| {
                if let Some(c) = c {
                    c.resolve(theme);
                }
            };
            if let Some(g) = &mut fx.glow {
                one(&mut g.color);
            }
            if let Some(s) = &mut fx.shadow {
                one(&mut s.color);
            }
            if let Some(o) = &mut fx.outline {
                one(&mut o.color);
            }
            if let Some(f) = &mut fx.fill {
                one(&mut f.solid);
                for s in &mut f.stops {
                    s.color.resolve(theme);
                }
            }
        }
    }
}

fn overlay_fx(base: &Option<Box<TextFx>>, top: &Option<Box<TextFx>>) -> Option<Box<TextFx>> {
    match (base, top) {
        (None, None) => None,
        (Some(b), None) => Some(b.clone()),
        (None, Some(t)) => Some(t.clone()),
        (Some(b), Some(t)) => {
            macro_rules! pick {
                ($($f:ident),*) => { TextFx { $($f: t.$f.clone().or_else(|| b.$f.clone()),)* } };
            }
            Some(Box::new(pick!(glow, shadow, reflection, outline, fill, ligatures, num_form, num_spacing, style_sets, cntxt_alts)))
        }
    }
}

/// Turn the theme references of a run's formatting into hex colours, so a renderer sees only colours. Theme wins
/// over the hex fallback beside it, as in Word.
pub fn resolve_run(f: &mut RunFmt, theme: &ShapeTheme) {
    if let Some(c) = f.ext.color_theme.as_ref().and_then(|t| t.hex(theme)) {
        f.color = Some(c);
    }
    if let Some(c) = f.ext.underline_theme.as_ref().and_then(|t| t.hex(theme)) {
        f.ext.underline_color = Some(c);
    }
    f.ext.resolve(theme);
    if let Some(shd) = &f.ext.shd {
        f.shading = shd.flat();
    }
    if f.ext.fx.as_ref().is_some_and(|fx| fx.is_empty()) {
        f.ext.fx = None;
    }
}

// ---------------------------------------------------------------------------
// Patching
// ---------------------------------------------------------------------------

/// The ids that group runs under `w:fitText` (runs with the same id and the same width are fitted together).
static FIT_ID: AtomicI64 = AtomicI64::new(1_000_000);

pub fn next_fit_id() -> i64 {
    FIT_ID.fetch_add(1, Ordering::Relaxed)
}

/// The part of a `RunPatch` that deals with `RunExt`. `Some` means "set this"; the sentinels are the usual ones:
/// `false` for toggles, 0 for lengths, `none` for emphasis marks and `default` for the OpenType settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunExtPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dstrike: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vanish: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emboss: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imprint: Option<bool>,
    /// Do not check spelling or grammar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_proof: Option<bool>,
    /// Expanded (positive) or condensed (negative) spacing, in twentieths of a point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<i64>,
    /// Raised (positive) or lowered (negative), in half-points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    /// Percent; 100 is normal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<i64>,
    /// Half-points; 0 is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kern: Option<i64>,
    /// The theme colour behind `color` (written with `color` as its hex fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_tint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_shade: Option<String>,
    /// The underline's colour: hex, or `auto` for the text colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_tint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline_shade: Option<String>,
    /// A flat shading colour (hex); empty, `none` or `auto` removes the shading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    /// A whole shading, pattern and all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shd: Option<Shd>,
    /// A box around the text; a border whose `val` is `none` removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub em: Option<String>,
    /// Twips; 0 removes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_text: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_text_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_form: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_spacing: Option<String>,
    /// The stylistic sets that are on; an empty list turns them off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_sets: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cntxt_alts: Option<bool>,
    /// The East Asian font, layout and grid snapping (see east_asian.rs).
    #[serde(flatten)]
    pub ea: crate::east_asian::RunEaPatch,
}

fn toggle(rpr: &mut Element, name: &'static str, on: bool, inherited: Option<bool>) {
    rpr.remove_children(name);
    if on {
        put_rpr(rpr, Element::new(name));
    } else if inherited == Some(true) {
        put_rpr(rpr, Element::new(name).with_attr("w:val", "0"));
    }
}

/// A numeric property: removed when it is `normal` and nothing inherited says otherwise.
fn number(rpr: &mut Element, name: &'static str, v: i64, normal: i64, inherited: Option<i64>) {
    rpr.remove_children(name);
    if v != normal || inherited.is_some_and(|i| i != normal) {
        put_rpr(rpr, Element::new(name).with_attr("w:val", v.to_string()));
    }
}

fn w14(name: &'static str) -> Element {
    Element::new(name)
}

fn put_w14_val(rpr: &mut Element, name: &'static str, val: &str, none: &[&str]) {
    rpr.remove_children(name);
    if !val.is_empty() && !none.contains(&val) {
        put_rpr(rpr, w14(name).with_attr("w14:val", val));
    }
}

impl RunExtPatch {
    /// True when the patch changes a `w14:` effect (the part then needs the `w14` namespace declared).
    pub fn needs_w14(&self) -> bool {
        self.ligatures.is_some() || self.num_form.is_some() || self.num_spacing.is_some() || self.style_sets.is_some() || self.cntxt_alts.is_some()
    }

    pub fn apply(&self, rpr: &mut Element, inherited: &RunFmt) {
        let inh = &inherited.ext;
        self.ea.apply(rpr, inherited);
        if let Some(b) = self.dstrike {
            toggle(rpr, "w:dstrike", b, inherited.dstrike);
            if b {
                rpr.remove_children("w:strike");
            }
        }
        if let Some(b) = self.vanish {
            toggle(rpr, "w:vanish", b, inherited.vanish);
        }
        if let Some(b) = self.outline {
            toggle(rpr, "w:outline", b, inh.outline);
        }
        if let Some(b) = self.shadow {
            toggle(rpr, "w:shadow", b, inh.shadow);
        }
        if let Some(b) = self.emboss {
            toggle(rpr, "w:emboss", b, inh.emboss);
            if b {
                rpr.remove_children("w:imprint");
            }
        }
        if let Some(b) = self.imprint {
            toggle(rpr, "w:imprint", b, inh.imprint);
            if b {
                rpr.remove_children("w:emboss");
            }
        }
        if let Some(b) = self.no_proof {
            toggle(rpr, "w:noProof", b, inh.no_proof);
        }
        if let Some(v) = self.spacing {
            number(rpr, "w:spacing", v, 0, inherited.spacing);
        }
        if let Some(v) = self.position {
            number(rpr, "w:position", v, 0, inh.position);
        }
        if let Some(v) = self.scale {
            number(rpr, "w:w", v, 100, inh.scale);
        }
        if let Some(v) = self.kern {
            number(rpr, "w:kern", v, 0, inh.kern);
        }
        // A theme colour goes beside the hex that `RunPatch` wrote: `val` is what it looks like, the theme says why.
        if let Some(name) = self.theme_color.as_deref().filter(|n| !n.is_empty()) {
            if let Some(c) = rpr.child_mut("w:color") {
                c.set_attr("w:themeColor", name);
                c.remove_attr("w:themeTint");
                c.remove_attr("w:themeShade");
                if let Some(t) = &self.theme_tint {
                    c.set_attr("w:themeTint", t);
                }
                if let Some(s) = &self.theme_shade {
                    c.set_attr("w:themeShade", s);
                }
            }
        }
        if self.underline_color.is_some() || self.underline_theme.is_some() {
            if let Some(u) = rpr.child_mut("w:u") {
                for a in ["w:color", "w:themeColor", "w:themeTint", "w:themeShade"] {
                    u.remove_attr(a);
                }
                match (self.underline_theme.as_deref().filter(|n| !n.is_empty()), self.underline_color.as_deref()) {
                    (Some(name), c) => {
                        u.set_attr("w:color", c.map(|c| c.trim_start_matches('#').to_uppercase()).unwrap_or_else(|| "auto".into()));
                        u.set_attr("w:themeColor", name);
                        if let Some(t) = &self.underline_tint {
                            u.set_attr("w:themeTint", t);
                        }
                        if let Some(s) = &self.underline_shade {
                            u.set_attr("w:themeShade", s);
                        }
                    }
                    (None, Some(c)) if !c.is_empty() && !c.eq_ignore_ascii_case("auto") => u.set_attr("w:color", c.trim_start_matches('#').to_uppercase()),
                    _ => {}
                }
            }
        }
        if let Some(shd) = &self.shd {
            rpr.remove_children("w:shd");
            if shd.val != "nil" && shd.val != "none" {
                put_rpr(rpr, shd.to_el());
            } else if inherited.shading.is_some() || inh.shd.is_some() {
                put_rpr(rpr, Shd { val: "clear".into(), ..Default::default() }.to_el());
            }
        } else if let Some(fill) = &self.shading {
            rpr.remove_children("w:shd");
            let fill = fill.trim_start_matches('#');
            if fill.is_empty() || fill.eq_ignore_ascii_case("none") || fill.eq_ignore_ascii_case("auto") {
                if inherited.shading.is_some() || inh.shd.is_some() {
                    put_rpr(rpr, Shd { val: "clear".into(), ..Default::default() }.to_el());
                }
            } else {
                put_rpr(rpr, Shd { val: "clear".into(), fill: Some(fill.to_uppercase()), ..Default::default() }.to_el());
            }
        }
        if let Some(b) = &self.border {
            rpr.remove_children("w:bdr");
            if b.val != "none" && b.val != "nil" && !b.val.is_empty() {
                let color = if b.color.eq_ignore_ascii_case("auto") { "auto".to_string() } else { b.color.trim_start_matches('#').to_uppercase() };
                put_rpr(
                    rpr,
                    Element::new("w:bdr").with_attr("w:val", &b.val).with_attr("w:sz", b.sz.to_string()).with_attr("w:space", b.space.to_string()).with_attr("w:color", color),
                );
            } else if inh.border.as_ref().is_some_and(|i| i.val != "none") {
                put_rpr(rpr, Element::new("w:bdr").with_attr("w:val", "nil"));
            }
        }
        if let Some(em) = &self.em {
            rpr.remove_children("w:em");
            if em != "none" && !em.is_empty() {
                put_rpr(rpr, Element::new("w:em").with_attr("w:val", em));
            } else if inh.em.as_deref().is_some_and(|e| e != "none") {
                put_rpr(rpr, Element::new("w:em").with_attr("w:val", "none"));
            }
        }
        if let Some(w) = self.fit_text {
            rpr.remove_children("w:fitText");
            if w > 0 {
                let id = self.fit_text_id.unwrap_or_else(next_fit_id);
                put_rpr(rpr, Element::new("w:fitText").with_attr("w:val", w.to_string()).with_attr("w:id", id.to_string()));
            }
        }
        let fx_inh = inh.fx.clone().unwrap_or_default();
        if let Some(v) = &self.ligatures {
            put_w14_val(rpr, "w14:ligatures", v, &["default"]);
            // Turning ligatures back to the default over an inherited setting needs an explicit one.
            if (v.is_empty() || v == "default") && fx_inh.ligatures.as_deref().is_some_and(|l| l != "none") {
                put_rpr(rpr, w14("w14:ligatures").with_attr("w14:val", "none"));
            }
        }
        if let Some(v) = &self.num_form {
            put_w14_val(rpr, "w14:numForm", v, &["default"]);
        }
        if let Some(v) = &self.num_spacing {
            put_w14_val(rpr, "w14:numSpacing", v, &["default"]);
        }
        if let Some(sets) = &self.style_sets {
            rpr.remove_children("w14:stylisticSets");
            if !sets.is_empty() {
                let mut e = w14("w14:stylisticSets");
                for s in sets {
                    e.children.push(crate::xml::Node::Element(w14("w14:styleSet").with_attr("w14:id", s.to_string())));
                }
                put_rpr(rpr, e);
            }
        }
        if let Some(b) = self.cntxt_alts {
            rpr.remove_children("w14:cntxtAlts");
            if b {
                put_rpr(rpr, w14("w14:cntxtAlts"));
            } else if fx_inh.cntxt_alts == Some(true) {
                put_rpr(rpr, w14("w14:cntxtAlts").with_attr("w14:val", "0"));
            }
        }
    }
}

/// Whether any `w14:` element is used under `e`.
pub fn uses_w14(e: &Element) -> bool {
    e.name.starts_with("w14:") || e.elements().any(uses_w14)
}

/// Declare the `w14` namespace on a part that uses it and doesn't yet.
pub fn declare_w14(root: &mut Element) {
    if root.attr("xmlns:w14").is_none() && uses_w14(root) {
        ensure_w14(root);
    }
}

/// Make sure a part's root declares `xmlns:w14` and lists `w14` as ignorable, as Word writes it.
pub fn ensure_w14(root: &mut Element) {
    if root.attr("xmlns:w14").is_none() {
        root.set_attr("xmlns:w14", "http://schemas.microsoft.com/office/word/2010/wordml");
    }
    if root.attr("xmlns:mc").is_none() {
        root.set_attr("xmlns:mc", "http://schemas.openxmlformats.org/markup-compatibility/2006");
    }
    let mut tokens: Vec<String> = root.attr("mc:Ignorable").map(|v| v.split_whitespace().map(String::from).collect()).unwrap_or_default();
    if !tokens.iter().any(|t| t == "w14") {
        tokens.push("w14".into());
    }
    root.set_attr("mc:Ignorable", tokens.join(" "));
}

// ---------------------------------------------------------------------------
// Change case
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseMode {
    /// The first letter of each sentence in capitals, the rest in lower case.
    Sentence,
    Lower,
    Upper,
    /// The first letter of each word in capitals; the other letters stay as they are.
    Title,
    /// Capitals become small letters and the other way round.
    Toggle,
}

fn turkic(lang: Option<&str>) -> bool {
    lang.is_some_and(|l| {
        let p = l.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
        p == "tr" || p == "az" || p == "crh"
    })
}

fn upper(c: char, tr: bool) -> String {
    match (c, tr) {
        ('i', true) => "\u{130}".into(),
        ('\u{131}', true) => "I".into(),
        _ => c.to_uppercase().collect(),
    }
}

fn lower(c: char, tr: bool) -> String {
    match (c, tr) {
        ('I', true) => "\u{131}".into(),
        ('\u{130}', true) => "i".into(),
        _ => c.to_lowercase().collect(),
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}' || c == '_'
}

/// The new text for each character of `chars`. `start` is where the range begins (characters before it give
/// context for words and sentences). `None` for a character that is not text (an object, a tab) keeps it.
pub fn change_case(chars: &[Option<char>], start: usize, end: usize, mode: CaseMode, lang: Option<&str>) -> Vec<Option<String>> {
    let tr = turkic(lang);
    let mut out: Vec<Option<String>> = vec![None; chars.len()];
    // Sentence state: at the start of a sentence until its first letter. Word state: at the start of a word.
    let mut sentence_start = true;
    let mut word_start = true;
    for i in 0..end {
        let Some(c) = chars[i] else {
            // A tab, break or object ends a word; a break ends nothing else.
            word_start = true;
            continue;
        };
        let in_range = i >= start;
        let new: Option<String> = match mode {
            CaseMode::Lower => in_range.then(|| {
                // A capital sigma at the end of a Greek word is the final sigma.
                if c == '\u{3a3}' && i > 0 && chars[i - 1].is_some_and(|p| p.is_alphabetic()) && !chars.get(i + 1).copied().flatten().is_some_and(|n| n.is_alphabetic()) {
                    "\u{3c2}".to_string()
                } else {
                    lower(c, tr)
                }
            }),
            CaseMode::Upper => in_range.then(|| upper(c, tr)),
            CaseMode::Toggle => in_range.then(|| if c.is_uppercase() { lower(c, tr) } else if c.is_lowercase() { upper(c, tr) } else { c.to_string() }),
            CaseMode::Title => (in_range && word_start && c.is_alphabetic()).then(|| title(c, tr)),
            CaseMode::Sentence => {
                if !in_range {
                    None
                } else if c.is_alphabetic() && sentence_start {
                    Some(upper(c, tr))
                } else if c.is_alphabetic() {
                    Some(lower(c, tr))
                } else {
                    None
                }
            }
        };
        out[i] = new.filter(|n| n.chars().count() != 1 || !n.starts_with(c));
        // Update the states for the next character.
        word_start = !is_word_char(c);
        if c.is_alphanumeric() {
            sentence_start = false;
        } else if matches!(c, '.' | '!' | '?' | '\u{2026}' | '\u{3002}' | '\u{ff01}' | '\u{ff1f}') {
            // The next letter after the stop (and a space) starts a sentence.
            sentence_start = true;
        }
    }
    out
}

fn title(c: char, tr: bool) -> String {
    // The titlecase form of a few letters differs from the capital (the Croatian digraphs).
    match c {
        '\u{1c6}' | '\u{1c4}' | '\u{1c5}' => "\u{1c5}".into(),
        '\u{1c9}' | '\u{1c7}' | '\u{1c8}' => "\u{1c8}".into(),
        '\u{1cc}' | '\u{1ca}' | '\u{1cb}' => "\u{1cb}".into(),
        _ => upper(c, tr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, mode: CaseMode, lang: Option<&str>) -> String {
        let chars: Vec<Option<char>> = text.chars().map(Some).collect();
        let out = change_case(&chars, 0, chars.len(), mode, lang);
        text.chars().zip(out).map(|(c, n)| n.unwrap_or_else(|| c.to_string())).collect()
    }

    #[test]
    fn the_five_cases() {
        assert_eq!(run("hello WORLD. this is IT! ok", CaseMode::Sentence, None), "Hello world. This is it! Ok");
        assert_eq!(run("Hello World", CaseMode::Lower, None), "hello world");
        assert_eq!(run("Hello World", CaseMode::Upper, None), "HELLO WORLD");
        assert_eq!(run("hello wORLD don't well-known", CaseMode::Title, None), "Hello WORLD Don't Well-Known");
        assert_eq!(run("Hello World", CaseMode::Toggle, None), "hELLO wORLD");
    }

    #[test]
    fn turkish_has_a_dotted_and_a_dotless_i() {
        assert_eq!(run("istanbul ILIK", CaseMode::Upper, Some("tr-TR")), "\u{130}STANBUL ILIK");
        assert_eq!(run("ISTANBUL Iğdır", CaseMode::Lower, Some("tr-TR")), "\u{131}stanbul \u{131}ğd\u{131}r");
        assert_eq!(run("istanbul", CaseMode::Upper, Some("en-US")), "ISTANBUL");
        assert_eq!(run("ISTANBUL", CaseMode::Lower, Some("en-US")), "istanbul");
    }

    #[test]
    fn sharp_s_and_final_sigma() {
        assert_eq!(run("straße", CaseMode::Upper, Some("de-DE")), "STRASSE");
        assert_eq!(run("ΟΔΟΣ ΟΔΟΙ", CaseMode::Lower, None), "οδος οδοι");
    }

    #[test]
    fn context_before_the_range_decides_word_starts() {
        let chars: Vec<Option<char>> = "abc def".chars().map(Some).collect();
        // Only "bc def" is in the range: "b" is mid-word, so it stays; "d" starts a word.
        let out = change_case(&chars, 1, chars.len(), CaseMode::Title, None);
        assert_eq!(out[1], None);
        assert_eq!(out[4].as_deref(), Some("D"));
    }
}
