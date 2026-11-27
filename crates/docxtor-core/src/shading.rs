//! `w:shd` shading: a pattern (`pct25`, `diagStripe`, ...) of a colour over a fill, with the theme colours
//! Word writes beside the hex ones (`themeFill`, `themeFillTint`, `themeFillShade`, `themeColor`, ...).
//!
//! `Shd::from_el` reads the element as the file has it. `Shd::resolve` turns the theme references into
//! hex colours (the theme wins over the hex fallback, as in Word), so a renderer only sees `val`, `color` and `fill`.

use crate::shapes::theme::ShapeTheme;
use crate::xml::Element;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shd {
    /// The pattern: `clear` (just the fill), `solid` (all colour), `pct25`, `horzStripe`, `diagCross`, ...
    pub val: String,
    /// The pattern's colour (hex); None or `auto` is the text colour (black).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The background (hex); None or `auto` is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_color_tint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_color_shade: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_fill_tint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_fill_shade: Option<String>,
}

fn hex6(v: &str) -> Option<String> {
    crate::colour::hex(v.trim_start_matches('#'))
}

/// The pattern of a shading (`clear`, `solid`, `pct25`, `diagStripe`, ...): letters and digits only, so what a file puts there cannot be more than a name.
fn shd_pattern(v: &str) -> String {
    if v.len() <= 24 && v.bytes().all(|c| c.is_ascii_alphanumeric()) { v.to_string() } else { "clear".into() }
}

impl Shd {
    pub fn from_el(e: &Element) -> Shd {
        // Read through `colour`: a colour is six hex digits or `auto`, a theme colour a name from Word's list, a tint or a shade two hex digits.
        let colour = |n: &str| e.attr(n).and_then(crate::colour::auto_or_hex);
        let theme = |n: &str| e.attr(n).and_then(crate::colour::theme_name);
        let amount = |n: &str| e.attr(n).and_then(crate::colour::two_hex);
        Shd {
            val: e.attr("w:val").map(shd_pattern).unwrap_or_else(|| "clear".into()),
            color: colour("w:color"),
            fill: colour("w:fill"),
            theme_color: theme("w:themeColor"),
            theme_color_tint: amount("w:themeTint"),
            theme_color_shade: amount("w:themeShade"),
            theme_fill: theme("w:themeFill"),
            theme_fill_tint: amount("w:themeFillTint"),
            theme_fill_shade: amount("w:themeFillShade"),
        }
    }

    /// Replace theme references by the hex colours they stand for, and tidy `auto` values away.
    pub fn resolve(&mut self, theme: &ShapeTheme) {
        if let Some(c) = theme_hex(theme, self.theme_color.take(), self.theme_color_tint.take(), self.theme_color_shade.take()) {
            self.color = Some(c);
        }
        if let Some(c) = theme_hex(theme, self.theme_fill.take(), self.theme_fill_tint.take(), self.theme_fill_shade.take()) {
            self.fill = Some(c);
        }
        self.color = self.color.as_deref().and_then(hex6);
        self.fill = self.fill.as_deref().and_then(hex6);
    }

    /// The colour a plain fill shows: the fill of a `clear` shading, the colour of a `solid` one, nothing for a pattern.
    pub fn flat(&self) -> Option<String> {
        match self.val.as_str() {
            "solid" => self.color.clone().or(Some("000000".into())),
            "nil" => None,
            _ => self.fill.clone(),
        }
    }

    /// The shading element as Word writes it: the pattern, its colour and the fill, with the theme references kept.
    pub fn to_el(&self) -> Element {
        let mut e = Element::new("w:shd").with_attr("w:val", &self.val).with_attr("w:color", self.color.as_deref().unwrap_or("auto"));
        if let Some(t) = &self.theme_color {
            e.set_attr("w:themeColor", t);
        }
        if let Some(t) = &self.theme_color_tint {
            e.set_attr("w:themeTint", t);
        }
        if let Some(t) = &self.theme_color_shade {
            e.set_attr("w:themeShade", t);
        }
        e.set_attr("w:fill", self.fill.as_deref().unwrap_or("auto"));
        if let Some(t) = &self.theme_fill {
            e.set_attr("w:themeFill", t);
        }
        if let Some(t) = &self.theme_fill_tint {
            e.set_attr("w:themeFillTint", t);
        }
        if let Some(t) = &self.theme_fill_shade {
            e.set_attr("w:themeFillShade", t);
        }
        e
    }
}

/// A theme colour with Word's tint (towards white) and shade (towards black), both given as hex 00 to FF.
pub fn theme_hex(theme: &ShapeTheme, name: Option<String>, tint: Option<String>, shade: Option<String>) -> Option<String> {
    let name = name.filter(|n| n != "none")?;
    theme.word_color(&name, tint.as_deref(), shade.as_deref())
}
