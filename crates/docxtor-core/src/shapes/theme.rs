//! Theme colours and line widths, and DrawingML colour resolution (`a:srgbClr`, `a:schemeClr` with
//! `lumMod`, `lumOff`, `tint`, `shade`, `alpha`, ...).

use crate::package::{self, Package};
use crate::xml::Element;
use std::collections::HashMap;

/// The colour slots of a theme, in the order of Word's theme colour dialog.
pub const SLOTS: [&str; 12] = ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"];

#[derive(Debug, Clone)]
pub struct ShapeTheme {
    /// Colours by scheme name (`dk1`, `lt1`, `dk2`, `lt2`, `accent1`..`accent6`, `hlink`, `folHlink`), uppercase hex.
    colors: HashMap<String, String>,
    /// Widths in points of the theme's line styles (`a:lnStyleLst`), indexed from 0 (`lnRef idx=1` is the first).
    pub line_widths: Vec<f64>,
    /// The theme's fill styles (`a:fillStyleLst`): `fillRef idx` 1 to 3 pick from them.
    pub fill_styles: Vec<Element>,
    /// The theme's background fill styles (`a:bgFillStyleLst`): `fillRef idx` 1001 and up.
    pub bg_fill_styles: Vec<Element>,
    /// The theme's effect styles (`a:effectStyleLst`): `effectRef idx` 1 to 3 pick from them.
    pub effect_styles: Vec<Element>,
}

impl Default for ShapeTheme {
    /// The Office theme, for files without one.
    fn default() -> Self {
        let colors = [
            ("dk1", "000000"),
            ("lt1", "FFFFFF"),
            ("dk2", "44546A"),
            ("lt2", "E7E6E6"),
            ("accent1", "5B9BD5"),
            ("accent2", "ED7D31"),
            ("accent3", "A5A5A5"),
            ("accent4", "FFC000"),
            ("accent5", "4472C4"),
            ("accent6", "70AD47"),
            ("hlink", "0563C1"),
            ("folHlink", "954F72"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        ShapeTheme { colors, line_widths: vec![0.5, 1.0, 1.5], fill_styles: vec![], bg_fill_styles: vec![], effect_styles: vec![] }
    }
}

impl ShapeTheme {
    /// Read the document's theme (`word/theme/theme1.xml`); the Office theme where it is missing.
    pub fn load(pkg: &Package) -> ShapeTheme {
        let theme = ShapeTheme::default();
        let main = pkg.main_document_path();
        let Some(rel) = pkg.rels_for(&main).ok().and_then(|r| r.into_iter().find(|r| r.rel_type == package::REL_THEME)) else { return theme };
        let Ok(doc) = pkg.xml(&package::resolve_target(&main, &rel.target)) else { return theme };
        ShapeTheme::from_doc(&doc.root)
    }

    /// The theme in a theme part's bytes (`Document::shape_theme`); the Office theme where there is none or it can't be read.
    pub fn from_bytes(bytes: Option<&[u8]>) -> ShapeTheme {
        bytes.and_then(|b| crate::xml::parse(b).ok()).map(|d| ShapeTheme::from_doc(&d.root)).unwrap_or_default()
    }

    /// The colours and line widths of a theme document (`a:theme`); the Office theme's where it says nothing.
    pub fn from_doc(root: &Element) -> ShapeTheme {
        let mut theme = ShapeTheme::default();
        if let Some(scheme) = root.find("a:clrScheme") {
            for c in scheme.elements() {
                let name = c.local().to_string();
                let hex = c.elements().next().and_then(|v| match v.name.as_str() {
                    "a:srgbClr" => v.attr("val").and_then(crate::colour::hex),
                    "a:sysClr" => v.attr("lastClr").or(v.attr("val")).and_then(crate::colour::hex),
                    _ => None,
                });
                if let Some(h) = hex {
                    theme.colors.insert(name, h);
                }
            }
        }
        if let Some(lines) = root.find("a:lnStyleLst") {
            let widths: Vec<f64> = lines.children_named("a:ln").map(|l| l.attr("w").and_then(|w| w.parse::<f64>().ok()).unwrap_or(9525.0) / super::EMU_PER_PT).collect();
            if !widths.is_empty() {
                theme.line_widths = widths;
            }
        }
        let list = |name: &str| root.find(name).map(|l| l.elements().cloned().collect::<Vec<_>>()).unwrap_or_default();
        theme.fill_styles = list("a:fillStyleLst");
        theme.bg_fill_styles = list("a:bgFillStyleLst");
        theme.effect_styles = list("a:effectStyleLst");
        theme
    }

    /// The fill style of a `fillRef idx` (1-based; 1001 and up are background fills).
    pub fn fill_style(&self, idx: usize) -> Option<&Element> {
        if idx >= 1001 { self.bg_fill_styles.get(idx - 1001) } else { self.fill_styles.get(idx.checked_sub(1)?) }
    }

    /// The effect style of an `effectRef idx` (1-based): an `a:effectStyle`.
    pub fn effect_style(&self, idx: usize) -> Option<&Element> {
        self.effect_styles.get(idx.checked_sub(1)?)
    }

    /// A theme colour as WordprocessingML names it (`accent1`, `text1`, `background2`, `hyperlink`...) with Word's tint
    /// (towards white) and shade (towards black), both two hex digits. Both work on the lightness, as Word's do.
    pub fn word_color(&self, name: &str, tint: Option<&str>, shade: Option<&str>) -> Option<String> {
        let slot = match name {
            "dark1" | "text1" => "dk1",
            "light1" | "background1" => "lt1",
            "dark2" | "text2" => "dk2",
            "light2" | "background2" => "lt2",
            "hyperlink" => "hlink",
            "followedHyperlink" => "folHlink",
            other => other,
        };
        let base = self.colors.get(slot)?;
        if tint.is_none() && shade.is_none() {
            return Some(base.clone());
        }
        let (r, g, b) = parse_hex(base);
        let (h, s, mut l) = rgb_to_hsl(r, g, b);
        let byte = |v: &str| u8::from_str_radix(v.trim(), 16).ok().map(|n| n as f64 / 255.0);
        if let Some(t) = tint.and_then(byte) {
            l = l * t + (1.0 - t);
        }
        if let Some(sh) = shade.and_then(byte) {
            l *= sh;
        }
        // Word rounds a lightened colour down and a darkened one to the nearest (D9E2F3 and 843C0C for Office's accents).
        let (r, g, b) = if tint.is_none() { hsl_to_rgb(h, s, l.clamp(0.0, 1.0)) } else { hsl_to_rgb_floor(h, s, l.clamp(0.0, 1.0)) };
        Some(to_hex(r, g, b))
    }

    /// The twelve scheme colours in the order Word lists them.
    pub fn slots(&self) -> Vec<(&'static str, String)> {
        SLOTS.iter().map(|k| (*k, self.colors.get(*k).cloned().unwrap_or_default())).collect()
    }

    /// The default theme with some scheme colours replaced (`("accent1", "4472C4")`).
    pub fn with_scheme(mut self, colors: &[(&str, &str)]) -> ShapeTheme {
        for (k, v) in colors {
            self.colors.insert((*k).to_string(), v.to_uppercase());
        }
        self
    }

    /// A scheme colour by its DrawingML name (`accent1`, `tx1`, `bg1`, `dk2`, ...).
    pub fn scheme(&self, name: &str) -> Option<&str> {
        let key = match name {
            "tx1" => "dk1",
            "bg1" => "lt1",
            "tx2" => "dk2",
            "bg2" => "lt2",
            other => other,
        };
        self.colors.get(key).map(String::as_str)
    }

    /// The line width of `lnRef idx` (1-based), in points.
    pub fn line_width(&self, idx: usize) -> f64 {
        self.line_widths.get(idx.saturating_sub(1)).or(self.line_widths.last()).copied().unwrap_or(0.75)
    }

    /// The colour held by `parent` (`a:solidFill`, `a:lnRef`, `a:gs`, ...): its first colour child, resolved to
    /// uppercase hex and alpha (0 to 1). `ph` is what `phClr` stands for (the colour of a style reference).
    pub fn color_of(&self, parent: &Element, ph: Option<&(String, f64)>) -> Option<(String, f64)> {
        let c = parent.elements().find(|e| matches!(e.name.as_str(), "a:srgbClr" | "a:schemeClr" | "a:sysClr" | "a:prstClr" | "a:scrgbClr" | "a:hslClr"))?;
        let (base, mut alpha) = match c.name.as_str() {
            "a:srgbClr" => (c.attr("val").and_then(super::hex6)?, 1.0),
            "a:sysClr" => (c.attr("lastClr").or(c.attr("val")).and_then(super::hex6).or_else(|| match c.attr("val") {
                Some("windowText") => Some("000000".into()),
                Some("window") => Some("FFFFFF".into()),
                _ => None,
            })?, 1.0),
            "a:prstClr" => (preset_color(c.attr("val")?)?, 1.0),
            "a:scrgbClr" => {
                let ch = |a: &str| (c.attr(a).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100000.0).clamp(0.0, 1.0);
                (to_hex((ch("r") * 255.0).round() as u8, (ch("g") * 255.0).round() as u8, (ch("b") * 255.0).round() as u8), 1.0)
            }
            "a:hslClr" => {
                let h = c.attr("hue").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 60000.0;
                let s = c.attr("sat").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100000.0;
                let l = c.attr("lum").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100000.0;
                let (r, g, b) = hsl_to_rgb(h, s, l);
                (to_hex(r, g, b), 1.0)
            }
            _ => {
                let name = c.attr("val")?;
                if name == "phClr" {
                    let (h, a) = ph?.clone();
                    (h, a)
                } else {
                    (self.scheme(name)?.to_string(), 1.0)
                }
            }
        };
        let (mut r, mut g, mut b) = parse_hex(&base);
        for t in c.elements() {
            let v = t.attr("val").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100000.0;
            match t.local() {
                "alpha" => alpha = v.clamp(0.0, 1.0),
                "lumMod" | "lumOff" | "satMod" | "satOff" | "hueOff" => {
                    let (mut h, mut s, mut l) = rgb_to_hsl(r, g, b);
                    match t.local() {
                        "lumMod" => l *= v,
                        "lumOff" => l += v,
                        "satMod" => s *= v,
                        "satOff" => s += v,
                        _ => h = (h + t.attr("val").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 60000.0).rem_euclid(360.0),
                    }
                    (r, g, b) = hsl_to_rgb(h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                }
                "tint" => {
                    let mix = |c: u8| (c as f64 * v + 255.0 * (1.0 - v)).round().clamp(0.0, 255.0) as u8;
                    (r, g, b) = (mix(r), mix(g), mix(b));
                }
                "shade" => {
                    let mix = |c: u8| (c as f64 * v).round().clamp(0.0, 255.0) as u8;
                    (r, g, b) = (mix(r), mix(g), mix(b));
                }
                "comp" => {
                    let (h, s, l) = rgb_to_hsl(r, g, b);
                    (r, g, b) = hsl_to_rgb((h + 180.0).rem_euclid(360.0), s, l);
                }
                "inv" => (r, g, b) = (255 - r, 255 - g, 255 - b),
                "gray" => {
                    let y = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64).round() as u8;
                    (r, g, b) = (y, y, y);
                }
                _ => {}
            }
        }
        Some((to_hex(r, g, b), alpha))
    }
}

/// Whether a WordprocessingML element (or one inside it) names a theme colour (`w:themeColor`, `w:themeFill`).
pub fn has_theme_attrs(el: &Element) -> bool {
    el.attrs.iter().any(|(k, _)| k == "w:themeColor" || k == "w:themeFill") || el.elements().any(has_theme_attrs)
}

/// Replace the colours that name a theme colour by the theme's: `w:themeColor` into `w:color` (`w:val` on a `w:color`
/// element) and `w:themeFill` into `w:fill`, with their tints and shades. The theme attributes stay, so the file is
/// unchanged where it is saved; only the copy being read has the new colours.
pub fn resolve_theme_attrs(el: &mut Element, theme: &ShapeTheme) {
    let find = |el: &Element, name: &str, tint: &str, shade: &str| el.attr(name).filter(|n| *n != "none").and_then(|n| theme.word_color(n, el.attr(tint), el.attr(shade)));
    if let Some(hex) = find(el, "w:themeColor", "w:themeTint", "w:themeShade") {
        el.set_attr(if el.name == "w:color" { "w:val" } else { "w:color" }, hex);
    }
    if let Some(hex) = find(el, "w:themeFill", "w:themeFillTint", "w:themeFillShade") {
        el.set_attr("w:fill", hex);
    }
    for c in el.elements_mut() {
        resolve_theme_attrs(c, theme);
    }
}

fn preset_color(name: &str) -> Option<String> {
    Some(
        match name {
            "black" => "000000",
            "white" => "FFFFFF",
            "red" => "FF0000",
            "green" => "008000",
            "blue" => "0000FF",
            "yellow" => "FFFF00",
            "gray" | "grey" => "808080",
            "ltGray" | "lightGray" => "D3D3D3",
            "dkGray" | "darkGray" => "A9A9A9",
            "orange" => "FFA500",
            "purple" => "800080",
            "cyan" => "00FFFF",
            "magenta" => "FF00FF",
            "lime" => "00FF00",
            "navy" | "dkBlue" => "000080",
            "teal" => "008080",
            "maroon" => "800000",
            "olive" => "808000",
            "silver" => "C0C0C0",
            "brown" => "A52A2A",
            "pink" => "FFC0CB",
            "gold" => "FFD700",
            _ => return crate::marks::colour_hex(name),
        }
        .to_string(),
    )
}

fn parse_hex(h: &str) -> (u8, u8, u8) {
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    (p(0), p(2), p(4))
}

pub fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("{r:02X}{g:02X}{b:02X}")
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-12 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-12 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-12 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

/// The same, rounding each channel down.
fn hsl_to_rgb_floor(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0).floor() as i64 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let q = |v: f64| ((v + m) * 255.0 + 1e-6).floor().clamp(0.0, 255.0) as u8;
    (q(r), q(g), q(b))
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s <= 0.0 {
        let v = (l * 255.0).round().clamp(0.0, 255.0) as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let h = h / 360.0;
    (hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0))
}
