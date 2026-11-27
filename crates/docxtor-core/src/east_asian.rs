//! East Asian typography: the paragraph switches of the "Asian typography" tab (line-breaking rules, hanging
//! punctuation, spacing between Asian and Latin text), the East Asian layout of a run (combined characters, text
//! upright in vertical lines), the document grid of a section, and the document-wide line-breaking settings.
//!
//! Like the other formatting modules these are views over the raw `w:pPr`, `w:rPr`, `w:sectPr` and `w:settings`
//! elements: reading takes the file as it is and writing touches only the elements a patch names, so everything
//! else survives. Phonetic guides (`w:ruby`) are in `ruby.rs`.

use crate::props::{put_ordered, put_ppr, put_rpr};
use crate::xml::Element;
use serde::{Deserialize, Serialize};

fn on_off(e: &Element) -> bool {
    !matches!(e.attr("w:val"), Some("0") | Some("false") | Some("off"))
}

fn int(e: &Element, a: &str) -> Option<i64> {
    e.attr(a).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64)
}

// ---------------------------------------------------------------------------
// Paragraphs
// ---------------------------------------------------------------------------

/// The paragraph properties of the Asian typography tab. `None` is "not specified here"; Word's defaults when
/// nothing says otherwise are kinsoku, word wrap, overflow punctuation, auto spacing and grid snapping on and
/// punctuation compression at the top of a line off.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaEa {
    /// Follow the rules for the first and last character of a line (`w:kinsoku`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinsoku: Option<bool>,
    /// Break Latin words only at their ends; off lets a line break inside a word (`w:wordWrap`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_wrap: Option<bool>,
    /// Punctuation may hang past the right margin (`w:overflowPunct`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_punct: Option<bool>,
    /// Compress punctuation at the start of a line (`w:topLinePunct`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_line_punct: Option<bool>,
    /// Space between Asian and Latin text (`w:autoSpaceDE`) and between Asian text and numbers (`w:autoSpaceDN`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_space_de: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_space_dn: Option<bool>,
    /// Lines sit on the section's document grid (`w:snapToGrid`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    /// Vertical text in this one paragraph (`w:textDirection`): `tbRl`, `btLr` and the like; absent is across.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<String>,
    /// The text is aligned to the top, centre, baseline or bottom of the line (`w:textAlignment`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_alignment: Option<String>,
}

impl ParaEa {
    /// Take in one child of a `w:pPr` (the ones this module doesn't know are ignored).
    pub fn read(&mut self, c: &Element) {
        match c.name.as_str() {
            "w:kinsoku" => self.kinsoku = Some(on_off(c)),
            "w:wordWrap" => self.word_wrap = Some(on_off(c)),
            "w:overflowPunct" => self.overflow_punct = Some(on_off(c)),
            "w:topLinePunct" => self.top_line_punct = Some(on_off(c)),
            "w:autoSpaceDE" => self.auto_space_de = Some(on_off(c)),
            "w:autoSpaceDN" => self.auto_space_dn = Some(on_off(c)),
            "w:snapToGrid" => self.snap_to_grid = Some(on_off(c)),
            "w:textDirection" => self.text_direction = c.attr("w:val").filter(|v| !matches!(*v, "lrTb" | "lr")).map(String::from),
            "w:textAlignment" => self.text_alignment = c.attr("w:val").filter(|v| *v != "auto").map(String::from),
            _ => {}
        }
    }

    /// `top` over `self`, field by field.
    pub fn overlay(&self, top: &ParaEa) -> ParaEa {
        macro_rules! pick {
            ($($f:ident),*) => { ParaEa { $($f: top.$f.clone().or_else(|| self.$f.clone()),)* } };
        }
        pick!(kinsoku, word_wrap, overflow_punct, top_line_punct, auto_space_de, auto_space_dn, snap_to_grid, text_direction, text_alignment)
    }
}

/// A change to the Asian typography of paragraphs; only the given properties change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaEaPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinsoku: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_punct: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_line_punct: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_space_de: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_space_dn: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    /// `lrTb` (across) removes the direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_alignment: Option<String>,
}

impl ParaEaPatch {
    pub fn is_empty(&self) -> bool {
        *self == ParaEaPatch::default()
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(v) = &self.text_direction {
            if !matches!(v.as_str(), "lrTb" | "tbRl" | "btLr" | "lrTbV" | "tbRlV" | "tbLrV") {
                return Err(format!("Text direction is lrTb, tbRl, btLr, lrTbV, tbRlV or tbLrV, not '{v}'."));
            }
        }
        if let Some(v) = &self.text_alignment {
            if !matches!(v.as_str(), "auto" | "top" | "center" | "baseline" | "bottom") {
                return Err(format!("Text alignment is auto, top, center, baseline or bottom, not '{v}'."));
            }
        }
        Ok(())
    }

    /// Apply to a raw `w:pPr`. `inherited` is what the paragraph gets from its style, so that switching a property
    /// off writes an explicit `w:val="0"` only when something on is inherited (or it is on by default).
    pub fn apply(&self, ppr: &mut Element, inherited: &ParaEa) {
        let toggle = |ppr: &mut Element, name: &'static str, on: Option<bool>, inh: Option<bool>, default_on: bool| {
            let Some(on) = on else { return };
            ppr.remove_children(name);
            // Without the element the property is what the style gives, else Word's default.
            if on != inh.unwrap_or(default_on) {
                put_ppr(ppr, if on { Element::new(name) } else { Element::new(name).with_attr("w:val", "0") });
            }
        };
        toggle(ppr, "w:kinsoku", self.kinsoku, inherited.kinsoku, true);
        toggle(ppr, "w:wordWrap", self.word_wrap, inherited.word_wrap, true);
        toggle(ppr, "w:overflowPunct", self.overflow_punct, inherited.overflow_punct, true);
        toggle(ppr, "w:topLinePunct", self.top_line_punct, inherited.top_line_punct, false);
        toggle(ppr, "w:autoSpaceDE", self.auto_space_de, inherited.auto_space_de, true);
        toggle(ppr, "w:autoSpaceDN", self.auto_space_dn, inherited.auto_space_dn, true);
        toggle(ppr, "w:snapToGrid", self.snap_to_grid, inherited.snap_to_grid, true);
        if let Some(v) = &self.text_direction {
            ppr.remove_children("w:textDirection");
            if v != "lrTb" && !v.is_empty() {
                put_ppr(ppr, Element::new("w:textDirection").with_attr("w:val", v));
            } else if inherited.text_direction.is_some() {
                put_ppr(ppr, Element::new("w:textDirection").with_attr("w:val", "lrTb"));
            }
        }
        if let Some(v) = &self.text_alignment {
            ppr.remove_children("w:textAlignment");
            if v == "auto" || v.is_empty() {
                if inherited.text_alignment.is_some() {
                    put_ppr(ppr, Element::new("w:textAlignment").with_attr("w:val", "auto"));
                }
            } else {
                put_ppr(ppr, Element::new("w:textAlignment").with_attr("w:val", v));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

/// `w:eastAsianLayout`: characters combined into one box ("two lines in one") and text set upright in a
/// vertical line.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EaLayout {
    /// Runs with the same id are one combined box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Two lines in one: the text is set in two half-height lines inside one character box.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub combine: bool,
    /// The brackets around combined text: `round`, `square`, `angle` or `curly`; absent is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combine_brackets: Option<String>,
    /// Horizontal in vertical: the text runs across in a vertical line.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vert: bool,
    /// Horizontal in vertical, fitted into the height of the line.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vert_compress: bool,
}

impl EaLayout {
    pub fn from_el(c: &Element) -> EaLayout {
        let on = |a: &str| matches!(c.attr(a), Some("1") | Some("true") | Some("on"));
        EaLayout {
            id: int(c, "w:id"),
            combine: on("w:combine"),
            combine_brackets: c.attr("w:combineBrackets").filter(|b| *b != "none").map(String::from),
            vert: on("w:vert"),
            vert_compress: on("w:vertCompress"),
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.combine && !self.vert && !self.vert_compress
    }
}

/// The run properties of East Asian text that are not fonts or sizes. They sit in `RunExt`, so the view model and
/// the formatting at the caret carry them beside the other character properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEa {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ea_layout: Option<EaLayout>,
    /// The text sits on the document grid (`w:snapToGrid`); off lets a run keep its own spacing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    /// The East Asian language (`w:lang w:eastAsia`), which picks the font Chrome falls back to for the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang_ea: Option<String>,
    /// Set by the renderer on text that is East Asian (ideographs, kana, Hangul and their punctuation), which the
    /// editor draws in the run's East Asian font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ea_text: Option<bool>,
}

impl RunEa {
    pub fn read(&mut self, c: &Element) {
        match c.name.as_str() {
            "w:eastAsianLayout" => self.ea_layout = Some(EaLayout::from_el(c)),
            "w:snapToGrid" => self.snap_to_grid = Some(on_off(c)),
            "w:lang" => self.lang_ea = c.attr("w:eastAsia").filter(|v| !v.is_empty()).map(String::from),
            _ => {}
        }
    }

    pub fn overlay(&self, top: &RunEa) -> RunEa {
        RunEa {
            ea_layout: top.ea_layout.clone().or_else(|| self.ea_layout.clone()),
            snap_to_grid: top.snap_to_grid.or(self.snap_to_grid),
            lang_ea: top.lang_ea.clone().or_else(|| self.lang_ea.clone()),
            ea_text: top.ea_text.or(self.ea_text),
        }
    }
}

/// A change to the East Asian layout of a run. Setting `combine`, `vert` or `vert_compress` to false for all three
/// removes the element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EaLayoutPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combine: Option<bool>,
    /// `none`, `round`, `square`, `angle` or `curly`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combine_brackets: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert_compress: Option<bool>,
    /// The id the runs of one edit share, minted by the op layer (see `Document::op_format_text`).
    #[serde(skip)]
    pub id: Option<i64>,
}

/// The part of a `RunPatch` that deals with East Asian text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEaPatch {
    /// The East Asian font (`w:rFonts w:eastAsia`); an empty string removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_ea: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ea_layout: Option<EaLayoutPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
}

impl RunEaPatch {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(b) = self.ea_layout.as_ref().and_then(|l| l.combine_brackets.as_deref()) {
            if !matches!(b, "none" | "round" | "square" | "angle" | "curly") {
                return Err(format!("Brackets are none, round, square, angle or curly, not '{b}'."));
            }
        }
        Ok(())
    }

    pub fn apply(&self, rpr: &mut Element, inherited: &crate::props::RunFmt) {
        if let Some(font) = &self.font_ea {
            if font.is_empty() {
                if let Some(f) = rpr.child_mut("w:rFonts") {
                    f.remove_attr("w:eastAsia");
                    f.remove_attr("w:eastAsiaTheme");
                    if f.attr("w:hint") == Some("eastAsia") {
                        f.remove_attr("w:hint");
                    }
                    if f.attrs.is_empty() {
                        rpr.remove_children("w:rFonts");
                    }
                }
            } else {
                let mut f = rpr.child("w:rFonts").cloned().unwrap_or_else(|| Element::new("w:rFonts"));
                f.remove_attr("w:eastAsiaTheme");
                f.set_attr("w:eastAsia", font);
                f.set_attr("w:hint", "eastAsia");
                put_rpr(rpr, f);
            }
        }
        if let Some(on) = self.snap_to_grid {
            rpr.remove_children("w:snapToGrid");
            let current = inherited.ext.ea.snap_to_grid.unwrap_or(true);
            if on && !current {
                put_rpr(rpr, Element::new("w:snapToGrid"));
            } else if !on && current {
                put_rpr(rpr, Element::new("w:snapToGrid").with_attr("w:val", "0"));
            }
        }
        if let Some(p) = &self.ea_layout {
            let old = rpr.child("w:eastAsianLayout").map(EaLayout::from_el).unwrap_or_default();
            let combine = p.combine.unwrap_or(old.combine);
            let vert = p.vert.unwrap_or(old.vert);
            let compress = p.vert_compress.unwrap_or(old.vert_compress);
            let brackets = match p.combine_brackets.as_deref() {
                Some("none") | Some("") => None,
                Some(b) => Some(b.to_string()),
                None => old.combine_brackets.clone(),
            };
            rpr.remove_children("w:eastAsianLayout");
            if combine || vert || compress {
                let mut e = Element::new("w:eastAsianLayout").with_attr("w:id", p.id.or(old.id).unwrap_or(1).to_string());
                if combine {
                    e.set_attr("w:combine", "1");
                    if let Some(b) = &brackets {
                        e.set_attr("w:combineBrackets", b);
                    }
                }
                if vert {
                    e.set_attr("w:vert", "1");
                }
                if compress {
                    e.set_attr("w:vertCompress", "1");
                }
                put_rpr(rpr, e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The document grid
// ---------------------------------------------------------------------------

/// A section's document grid (`w:docGrid`): lines of a fixed pitch and, with characters, a fixed width per
/// character.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocGrid {
    /// `default` (no grid), `lines`, `linesAndChars` or `snapToChars`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The distance between lines (twips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_pitch: Option<i64>,
    /// What is added to the base font size to get the width of a character, in 4096ths of a point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_space: Option<i64>,
}

impl DocGrid {
    pub fn read(sect: &Element) -> Option<DocGrid> {
        let g = sect.child("w:docGrid")?;
        let kind = g.attr("w:type").unwrap_or("default").to_string();
        if kind == "default" {
            return None;
        }
        Some(DocGrid { kind, line_pitch: int(g, "w:linePitch"), char_space: int(g, "w:charSpace") })
    }

    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.kind.as_str(), "default" | "lines" | "linesAndChars" | "snapToChars") {
            return Err(format!("The document grid is default, lines, linesAndChars or snapToChars, not '{}'.", self.kind));
        }
        if self.line_pitch.is_some_and(|p| !(1..=31680).contains(&p)) {
            return Err("The line pitch is between 1 and 31680 twips.".into());
        }
        Ok(())
    }

    /// Write the grid into a `w:sectPr`; `default` removes it (the grid's pitch is kept in the file for Word).
    pub fn write(&self, sect: &mut Element, order: &[&str]) {
        if self.kind == "default" {
            let keep = sect.child("w:docGrid").cloned();
            sect.remove_children("w:docGrid");
            // A section without a grid still has the element in Word's files, with its pitch.
            if let Some(mut k) = keep {
                k.remove_attr("w:type");
                k.remove_attr("w:charSpace");
                if k.attr("w:linePitch").is_some() {
                    put_ordered(sect, k, order);
                }
            }
            return;
        }
        let mut e = sect.child("w:docGrid").cloned().unwrap_or_else(|| Element::new("w:docGrid"));
        e.set_attr("w:type", &self.kind);
        if let Some(p) = self.line_pitch {
            e.set_attr("w:linePitch", p.to_string());
        } else if e.attr("w:linePitch").is_none() {
            e.set_attr("w:linePitch", "360");
        }
        match self.char_space {
            Some(c) if self.kind != "lines" => e.set_attr("w:charSpace", c.to_string()),
            _ => {
                e.remove_attr("w:charSpace");
            }
        }
        put_ordered(sect, e, order);
    }
}

// ---------------------------------------------------------------------------
// Document-wide line-breaking settings
// ---------------------------------------------------------------------------

/// The characters that may not start or end a line in one language (`w:noLineBreaksBefore`, `w:noLineBreaksAfter`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineBreakChars {
    /// `ja-JP`, `ko-KR`, `zh-CN` or `zh-TW`.
    pub lang: String,
    /// Characters that may not start a line.
    #[serde(default)]
    pub before: String,
    /// Characters that may not end a line.
    #[serde(default)]
    pub after: String,
}

/// The document's East Asian line-breaking settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsianSettings {
    /// The strict rules (level 2) for the first and last character of a line (`w:strictFirstAndLastChars`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strict: bool,
    /// The lists of characters the document sets itself, one per language.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<LineBreakChars>,
}

impl AsianSettings {
    pub fn read(settings: Option<&Element>) -> AsianSettings {
        let Some(s) = settings else { return AsianSettings::default() };
        let mut custom: Vec<LineBreakChars> = Vec::new();
        for (name, before) in [("w:noLineBreaksBefore", true), ("w:noLineBreaksAfter", false)] {
            for e in s.children_named(name) {
                let (Some(lang), Some(val)) = (e.attr("w:lang"), e.attr("w:val")) else { continue };
                let entry = match custom.iter().position(|c| c.lang == lang) {
                    Some(i) => &mut custom[i],
                    None => {
                        custom.push(LineBreakChars { lang: lang.to_string(), ..Default::default() });
                        custom.last_mut().unwrap()
                    }
                };
                if before {
                    entry.before = val.to_string();
                } else {
                    entry.after = val.to_string();
                }
            }
        }
        AsianSettings { strict: s.child("w:strictFirstAndLastChars").is_some_and(on_off), custom }
    }

    pub fn validate(&self) -> Result<(), String> {
        for c in &self.custom {
            if !matches!(c.lang.as_str(), "ja-JP" | "ko-KR" | "zh-CN" | "zh-TW") {
                return Err(format!("Line-breaking characters are set for ja-JP, ko-KR, zh-CN or zh-TW, not '{}'.", c.lang));
            }
            if c.before.chars().count() > 255 || c.after.chars().count() > 255 {
                return Err("A list of line-breaking characters has at most 255 characters.".into());
            }
        }
        Ok(())
    }
}

/// The order of the settings Docxtor writes among themselves, from the schema (`CT_Settings`).
const SETTINGS_ORDER: &[&str] = &[
    "w:strictFirstAndLastChars",
    "w:noLineBreaksAfter",
    "w:noLineBreaksBefore",
];

/// The place a new settings child goes: before the first child that comes after it in the schema. The settings part
/// is long, so only the neighbours of these three are listed.
const AFTER_ASIAN: &[&str] = &[
    "w:savePreviewPicture", "w:doNotValidateAgainstSchema", "w:saveInvalidXml", "w:ignoreMixedContent", "w:alwaysShowPlaceholderText",
    "w:doNotDemarcateInvalidXml", "w:saveXmlDataOnly", "w:useXSLTWhenSaving", "w:saveThroughXslt", "w:showXMLTags",
    "w:alwaysMergeEmptyNamespace", "w:updateFields", "w:hdrShapeDefaults", "w:footnotePr", "w:endnotePr", "w:compat", "w:docVars",
    "w:rsids", "m:mathPr", "w:attachedSchema", "w:themeFontLang", "w:clrSchemeMapping", "w:doNotIncludeSubdocsInStats",
    "w:doNotAutoCompressPictures", "w:forceUpgrade", "w:captions", "w:readModeInkLockDown", "w:smartTagType", "sl:schemaLibrary",
    "w:shapeDefaults", "w:doNotEmbedSmartTags", "w:decimalSymbol", "w:listSeparator",
];

/// Write the settings into a `w:settings` element: the strict switch and the custom lists replace what is there.
pub fn write_settings(root: &mut Element, s: &AsianSettings) {
    for n in SETTINGS_ORDER {
        root.remove_children(n);
    }
    let mut new: Vec<Element> = Vec::new();
    if s.strict {
        new.push(Element::new("w:strictFirstAndLastChars"));
    }
    for c in &s.custom {
        if !c.after.is_empty() {
            new.push(Element::new("w:noLineBreaksAfter").with_attr("w:lang", &c.lang).with_attr("w:val", &c.after));
        }
    }
    for c in &s.custom {
        if !c.before.is_empty() {
            new.push(Element::new("w:noLineBreaksBefore").with_attr("w:lang", &c.lang).with_attr("w:val", &c.before));
        }
    }
    let at = root
        .children
        .iter()
        .position(|n| matches!(n, crate::xml::Node::Element(e) if AFTER_ASIAN.contains(&e.name.as_str())))
        .unwrap_or(root.children.len());
    for (k, e) in new.into_iter().enumerate() {
        root.children.insert(at + k, crate::xml::Node::Element(e));
    }
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

/// East Asian typography ops. The wire form is `{"op": "east_asian", "action": "...", ...}`. Character and paragraph
/// switches (the East Asian font, two lines in one, line-breaking rules, grid snapping) go through `format_text`
/// and `format_paragraphs`; the section's document grid through `set_section`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum EastAsianOp {
    /// Put a phonetic guide over the text between two positions, or change the guide at a position (a range that is
    /// just one guide). The text becomes the guide's base.
    SetRuby {
        from: crate::ops::Pos,
        to: crate::ops::Pos,
        #[serde(flatten)]
        props: crate::ruby::RubyProps,
    },
    /// Take the guide at a position away; its base text stays as plain text.
    ClearRuby { at: crate::ops::Pos },
    /// Enclose the characters between two positions (one or two) in a circle, square, triangle or diamond: an `EQ`
    /// overstrike field, as Word's Enclose characters writes it.
    Enclose { from: crate::ops::Pos, to: crate::ops::Pos, symbol: String },
    /// Take the enclosure at a position away; its characters stay as plain text.
    ClearEnclose { at: crate::ops::Pos },
    /// The document's line-breaking rules for East Asian text.
    SetSettings { settings: AsianSettings },
}

impl EastAsianOp {
    /// The flags of the op (see `OpClass`): the settings are not in the body.
    pub fn class(&self) -> crate::ops::OpClass {
        use crate::ops::OpClass as C;
        match self {
            EastAsianOp::SetSettings { .. } => C::BODY.leaves_body(),
            EastAsianOp::SetRuby { .. } | EastAsianOp::ClearRuby { .. } | EastAsianOp::Enclose { .. } | EastAsianOp::ClearEnclose { .. } => C::BODY,
        }
    }

    pub fn anchor(&self) -> Option<crate::model::NodeId> {
        match self {
            EastAsianOp::SetRuby { from, .. } | EastAsianOp::Enclose { from, .. } => Some(from.p.0),
            EastAsianOp::ClearRuby { at } | EastAsianOp::ClearEnclose { at } => Some(at.p.0),
            EastAsianOp::SetSettings { .. } => None,
        }
    }
}

/// The shapes Enclose characters offers, with the character that draws each.
pub const ENCLOSURES: &[(&str, char)] = &[("circle", '\u{25CB}'), ("square", '\u{25A1}'), ("triangle", '\u{25B3}'), ("diamond", '\u{25C7}')];

// ---------------------------------------------------------------------------
// Which characters are East Asian
// ---------------------------------------------------------------------------

/// Whether a character is set in the East Asian font of a run: ideographs, kana, Hangul, and the East Asian
/// punctuation and full-width forms. Latin letters, digits and spaces are not, even beside them.
pub fn is_east_asian(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x2E80..=0x2FDF | 0x3000..=0x303F | 0x3040..=0x30FF | 0x3100..=0x312F | 0x3130..=0x318F | 0x3190..=0x31FF
        | 0x3200..=0x4DBF | 0x4E00..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F
        | 0xFF00..=0xFFEF | 0x1B000..=0x1B16F | 0x20000..=0x3FFFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn east_asian_ranges() {
        assert!(is_east_asian('漢') && is_east_asian('か') && is_east_asian('カ') && is_east_asian('한') && is_east_asian('。') && is_east_asian('Ａ'));
        assert!(!is_east_asian('a') && !is_east_asian('1') && !is_east_asian(' ') && !is_east_asian('é'));
    }

    #[test]
    fn para_toggles_write_only_what_differs_from_the_default() {
        let mut ppr = Element::new("w:pPr");
        ParaEaPatch { kinsoku: Some(true), word_wrap: Some(false), top_line_punct: Some(true), ..Default::default() }.apply(&mut ppr, &ParaEa::default());
        // Kinsoku is on by default, so nothing is written; word wrap off and compression on are.
        assert!(ppr.child("w:kinsoku").is_none());
        assert_eq!(ppr.child("w:wordWrap").and_then(|e| e.attr("w:val")), Some("0"));
        assert!(ppr.child("w:topLinePunct").is_some());
        let mut f = ParaEa::default();
        for c in ppr.elements() {
            f.read(c);
        }
        assert_eq!(f.word_wrap, Some(false));
        assert_eq!(f.top_line_punct, Some(true));
    }

    #[test]
    fn grid_reads_and_writes() {
        let mut sect = Element::new("w:sectPr");
        DocGrid { kind: "linesAndChars".into(), line_pitch: Some(360), char_space: Some(-1000) }.write(&mut sect, &["w:docGrid"]);
        let g = DocGrid::read(&sect).unwrap();
        assert_eq!((g.kind.as_str(), g.line_pitch, g.char_space), ("linesAndChars", Some(360), Some(-1000)));
        DocGrid { kind: "default".into(), ..Default::default() }.write(&mut sect, &["w:docGrid"]);
        assert!(DocGrid::read(&sect).is_none());
    }

    #[test]
    fn settings_round_trip() {
        let mut root = Element::new("w:settings").with_child(Element::new("w:zoom")).with_child(Element::new("w:compat"));
        let s = AsianSettings { strict: true, custom: vec![LineBreakChars { lang: "ja-JP".into(), before: "、。」".into(), after: "「".into() }] };
        write_settings(&mut root, &s);
        let names: Vec<_> = root.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:zoom", "w:strictFirstAndLastChars", "w:noLineBreaksAfter", "w:noLineBreaksBefore", "w:compat"]);
        assert_eq!(AsianSettings::read(Some(&root)), s);
    }
}
