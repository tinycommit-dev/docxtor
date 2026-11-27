//! Typed views over `w:rPr` / `w:pPr` elements.
//!
//! Reading produces sparse `RunFmt` / `ParaFmt` values (None = not specified at
//! this level) that can be layered for style inheritance. Writing patches the
//! raw element in place, inserting children at their schema-mandated position so
//! that Word accepts the output, and leaving unknown children untouched.

use crate::charfmt::{RunExt, RunExtPatch};
use crate::shading::Shd;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

const RPR_ORDER: &[&str] = &[
    "w:rStyle", "w:rFonts", "w:b", "w:bCs", "w:i", "w:iCs", "w:caps", "w:smallCaps", "w:strike", "w:dstrike", "w:outline",
    "w:shadow", "w:emboss", "w:imprint", "w:noProof", "w:snapToGrid", "w:vanish", "w:webHidden", "w:color", "w:spacing",
    "w:w", "w:kern", "w:position", "w:sz", "w:szCs", "w:highlight", "w:u", "w:effect", "w:bdr", "w:shd", "w:fitText",
    "w:vertAlign", "w:rtl", "w:cs", "w:em", "w:lang", "w:eastAsianLayout", "w:specVanish", "w:oMath", "w14:glow", "w14:shadow",
    "w14:reflection", "w14:textOutline", "w14:textFill", "w14:scene3d", "w14:props3d", "w14:ligatures", "w14:numForm", "w14:numSpacing",
    "w14:stylisticSets", "w14:cntxtAlts", "w:rPrChange",
];

const PPR_ORDER: &[&str] = &[
    "w:pStyle", "w:keepNext", "w:keepLines", "w:pageBreakBefore", "w:framePr", "w:widowControl", "w:numPr",
    "w:suppressLineNumbers", "w:pBdr", "w:shd", "w:tabs", "w:suppressAutoHyphens", "w:kinsoku", "w:wordWrap",
    "w:overflowPunct", "w:topLinePunct", "w:autoSpaceDE", "w:autoSpaceDN", "w:bidi", "w:adjustRightInd", "w:snapToGrid",
    "w:spacing", "w:ind", "w:contextualSpacing", "w:mirrorIndents", "w:suppressOverlap", "w:jc", "w:textDirection",
    "w:textAlignment", "w:textboxTightWrap", "w:outlineLvl", "w:divId", "w:cnfStyle", "w:rPr", "w:sectPr", "w:pPrChange",
];

/// Insert or replace `child` in `parent` respecting `order`.
pub fn put_ordered(parent: &mut Element, child: Element, order: &[&str]) {
    if let Some(slot) = parent.elements_mut().find(|e| e.name == child.name) {
        *slot = child;
        return;
    }
    let rank = |n: &str| order.iter().position(|o| *o == n).unwrap_or(usize::MAX);
    let my = rank(&child.name);
    let pos = parent
        .children
        .iter()
        .position(|n| matches!(n, Node::Element(e) if rank(&e.name) > my))
        .unwrap_or(parent.children.len());
    parent.children.insert(pos, Node::Element(child));
}

pub fn put_rpr(rpr: &mut Element, child: Element) {
    put_ordered(rpr, child, RPR_ORDER)
}

pub fn put_ppr(ppr: &mut Element, child: Element) {
    put_ordered(ppr, child, PPR_ORDER)
}

fn on_off(e: &Element) -> bool {
    !matches!(e.attr("w:val"), Some("0") | Some("false") | Some("off"))
}

fn num(e: &Element, attr: &str) -> Option<i64> {
    e.attr(attr).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64)
}

// ---------------------------------------------------------------------------
// Run formatting
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFmt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// Underline type (`single`, `double`, ...); `none` disables.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dstrike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vanish: Option<bool>,
    /// Font family. Theme references are resolved when a theme is supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_theme: Option<String>,
    /// Size in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// Hex RGB without `#`, or `auto`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Highlight colour name (`yellow`, `green`, ...) or `none`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    /// Background shading fill (hex).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    /// `superscript`, `subscript` or `baseline`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<String>,
    /// Character spacing in twentieths of a point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// Complex-script (Arabic, Hebrew, Thai, ...) bold, italic, size and font: `w:bCs`, `w:iCs`, `w:szCs` and the
    /// `w:cs` font. The renderer swaps them in for the text they apply to (see `RenderCtx::run`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold_cs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic_cs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_cs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_cs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_cs_theme: Option<String>,
    /// The East Asian font (`w:eastAsia`, `w:eastAsiaTheme`): the renderer gives it to the East Asian text of a run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_ea: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_ea_theme: Option<String>,
    /// Underline colour, position, scale, kerning, outline and the like (see charfmt.rs).
    #[serde(flatten)]
    pub ext: RunExt,
}

impl RunFmt {
    pub fn from_rpr(rpr: &Element) -> RunFmt {
        let mut f = RunFmt::default();
        for c in rpr.elements() {
            f.ext.read(c);
            match c.name.as_str() {
                "w:rStyle" => f.style = c.attr("w:val").map(String::from),
                "w:b" => f.bold = Some(on_off(c)),
                "w:i" => f.italic = Some(on_off(c)),
                "w:u" => f.underline = c.attr("w:val").map(String::from).or(Some("single".into())),
                "w:strike" => f.strike = Some(on_off(c)),
                "w:dstrike" => f.dstrike = Some(on_off(c)),
                "w:caps" => f.caps = Some(on_off(c)),
                "w:smallCaps" => f.small_caps = Some(on_off(c)),
                "w:vanish" => f.vanish = Some(on_off(c)),
                "w:rtl" => f.rtl = Some(on_off(c)),
                "w:bCs" => f.bold_cs = Some(on_off(c)),
                "w:iCs" => f.italic_cs = Some(on_off(c)),
                "w:szCs" => f.size_cs = num(c, "w:val").map(|v| v as f64 / 2.0),
                "w:rFonts" => {
                    f.font_cs = c.attr("w:cs").map(String::from);
                    f.font_cs_theme = c.attr("w:cstheme").map(String::from);
                    f.font_ea = c.attr("w:eastAsia").map(String::from);
                    f.font_ea_theme = c.attr("w:eastAsiaTheme").map(String::from);
                    if let Some(t) = c.attr("w:asciiTheme").or(c.attr("w:hAnsiTheme")) {
                        f.font_theme = Some(t.to_string());
                        f.font = None;
                    } else if let Some(v) = c.attr("w:ascii").or(c.attr("w:hAnsi")).or(c.attr("w:cs")) {
                        f.font = Some(v.to_string());
                        f.font_theme = None;
                    }
                }
                "w:sz" => f.size = num(c, "w:val").map(|v| v as f64 / 2.0),
                // A colour from the file is read through `colour`: six hex digits, `auto` or a name from Word's list, nothing else.
                "w:color" => f.color = c.attr("w:val").and_then(crate::colour::auto_or_hex),
                "w:highlight" => f.highlight = c.attr("w:val").and_then(crate::colour::highlight),
                "w:shd" => {
                    if let Some(fill) = c.attr("w:fill").and_then(crate::colour::hex) {
                        f.shading = Some(fill);
                    }
                }
                "w:vertAlign" => f.vert_align = c.attr("w:val").map(String::from),
                "w:spacing" => f.spacing = num(c, "w:val"),
                _ => {}
            }
        }
        f
    }

    /// Layer `top` over `self` (top wins where specified).
    pub fn overlay(&self, top: &RunFmt) -> RunFmt {
        macro_rules! pick {
            ($($f:ident),*) => { RunFmt { $($f: top.$f.clone().or_else(|| self.$f.clone()),)* ext: RunExt::default() } };
        }
        let mut r = pick!(
            style, bold, italic, underline, strike, dstrike, caps, small_caps, vanish, font, font_theme, size, color, highlight, shading, vert_align, spacing, rtl,
            bold_cs, italic_cs, size_cs, font_cs, font_cs_theme, font_ea, font_ea_theme
        );
        if top.font_ea.is_some() {
            r.font_ea_theme = None;
        } else if top.font_ea_theme.is_some() {
            r.font_ea = None;
        }
        r.ext = self.ext.overlay(&top.ext);
        // A plain colour or underline written on top replaces a theme colour below it.
        if top.color.is_some() {
            r.ext.color_theme = top.ext.color_theme.clone();
        }
        if top.underline.is_some() {
            r.ext.underline_color = top.ext.underline_color.clone();
            r.ext.underline_theme = top.ext.underline_theme.clone();
        }
        if top.font_cs.is_some() {
            r.font_cs_theme = None;
        } else if top.font_cs_theme.is_some() {
            r.font_cs = None;
        }
        // An explicit font overrides an inherited theme font and vice versa.
        if top.font.is_some() {
            r.font_theme = None;
        } else if top.font_theme.is_some() {
            r.font = None;
        }
        r
    }

    pub fn is_empty(&self) -> bool {
        *self == RunFmt::default()
    }
}

/// A change to run formatting. `Some(None)` style clearing is expressed with
/// sentinel values: `false` for toggles, `"none"` for underline/highlight,
/// `"auto"` for colour, and an empty string to remove fonts/styles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<String>,
    /// Remove all direct formatting before applying the rest of the patch.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    /// Double strikethrough, hidden, spacing, position, scale, kerning, outline and the like (see charfmt.rs).
    #[serde(flatten)]
    pub ext: RunExtPatch,
}

impl RunPatch {
    /// Apply to a raw `w:rPr`. `inherited` is the formatting the run would have
    /// without direct formatting, used to decide whether "off" needs an explicit
    /// `w:val="0"` or can simply drop the element.
    pub fn apply(&self, rpr: &mut Element, inherited: &RunFmt) {
        if self.clear {
            rpr.children.retain(|n| matches!(n, Node::Element(e) if e.name == "w:rPrChange" || e.name == "w:lang"));
        }
        if let Some(s) = &self.style {
            if s.is_empty() {
                rpr.remove_children("w:rStyle");
            } else {
                put_rpr(rpr, Element::new("w:rStyle").with_attr("w:val", s));
            }
        }
        let toggle = |rpr: &mut Element, name: &'static str, cs: Option<&'static str>, on: bool, inh: Option<bool>| {
            rpr.remove_children(name);
            if let Some(cs) = cs {
                rpr.remove_children(cs);
            }
            if on {
                put_rpr(rpr, Element::new(name));
                if let Some(cs) = cs {
                    put_rpr(rpr, Element::new(cs));
                }
            } else if inh == Some(true) {
                put_rpr(rpr, Element::new(name).with_attr("w:val", "0"));
                if let Some(cs) = cs {
                    put_rpr(rpr, Element::new(cs).with_attr("w:val", "0"));
                }
            }
        };
        if let Some(b) = self.bold {
            toggle(rpr, "w:b", Some("w:bCs"), b, inherited.bold);
        }
        if let Some(b) = self.italic {
            toggle(rpr, "w:i", Some("w:iCs"), b, inherited.italic);
        }
        if let Some(b) = self.strike {
            toggle(rpr, "w:strike", None, b, inherited.strike);
            if b {
                rpr.remove_children("w:dstrike");
            }
        }
        if let Some(b) = self.caps {
            toggle(rpr, "w:caps", None, b, inherited.caps);
        }
        if let Some(b) = self.small_caps {
            toggle(rpr, "w:smallCaps", None, b, inherited.small_caps);
        }
        if let Some(u) = &self.underline {
            let old = rpr.child("w:u").cloned();
            rpr.remove_children("w:u");
            let inherited_on = inherited.underline.as_deref().is_some_and(|v| v != "none");
            if u != "none" {
                // A new style keeps the colour the underline had.
                let mut e = Element::new("w:u").with_attr("w:val", u);
                for a in ["w:color", "w:themeColor", "w:themeTint", "w:themeShade"] {
                    if let Some(v) = old.as_ref().and_then(|o| o.attr(a)) {
                        e.set_attr(a, v);
                    }
                }
                put_rpr(rpr, e);
            } else if inherited_on {
                put_rpr(rpr, Element::new("w:u").with_attr("w:val", "none"));
            }
        }
        if let Some(font) = &self.font {
            // A run that has its own East Asian font keeps it: the font box sets the Latin font.
            let old = rpr.child("w:rFonts").cloned();
            rpr.remove_children("w:rFonts");
            if !font.is_empty() {
                let mut f = Element::new("w:rFonts").with_attr("w:ascii", font).with_attr("w:hAnsi", font).with_attr("w:cs", font).with_attr("w:eastAsia", font);
                if let Some(o) = &old {
                    // One that only repeats the Latin font (as this code used to write it) is replaced with it.
                    let repeats = o.attr("w:ascii").or(o.attr("w:hAnsi")) == o.attr("w:eastAsia");
                    let mut kept = false;
                    if let Some(ea) = o.attr("w:eastAsia").filter(|_| !repeats) {
                        f.set_attr("w:eastAsia", ea);
                        kept = true;
                    }
                    if let Some(t) = o.attr("w:eastAsiaTheme") {
                        f.remove_attr("w:eastAsia");
                        f.set_attr("w:eastAsiaTheme", t);
                        kept = true;
                    }
                    if kept && o.attr("w:hint") == Some("eastAsia") {
                        f.set_attr("w:hint", "eastAsia");
                    }
                }
                put_rpr(rpr, f);
            } else if let Some(o) = old.filter(|o| o.attr("w:eastAsia").is_some() || o.attr("w:eastAsiaTheme").is_some()) {
                // Clearing the font clears the Latin font only.
                let mut f = Element::new("w:rFonts");
                for a in ["w:eastAsia", "w:eastAsiaTheme", "w:hint"] {
                    if let Some(v) = o.attr(a) {
                        f.set_attr(a, v);
                    }
                }
                put_rpr(rpr, f);
            }
        }
        if let Some(sz) = self.size {
            rpr.remove_children("w:sz");
            rpr.remove_children("w:szCs");
            if sz > 0.0 {
                let hp = ((sz * 2.0).round() as i64).to_string();
                put_rpr(rpr, Element::new("w:sz").with_attr("w:val", &hp));
                put_rpr(rpr, Element::new("w:szCs").with_attr("w:val", &hp));
            }
        }
        if let Some(c) = &self.color {
            rpr.remove_children("w:color");
            if !c.is_empty() {
                let v = if c.eq_ignore_ascii_case("auto") { "auto".to_string() } else { c.trim_start_matches('#').to_uppercase() };
                put_rpr(rpr, Element::new("w:color").with_attr("w:val", v));
            }
        }
        if let Some(h) = &self.highlight {
            rpr.remove_children("w:highlight");
            if h != "none" && !h.is_empty() {
                put_rpr(rpr, Element::new("w:highlight").with_attr("w:val", h));
            }
        }
        if let Some(v) = &self.vert_align {
            rpr.remove_children("w:vertAlign");
            if v != "baseline" && !v.is_empty() {
                put_rpr(rpr, Element::new("w:vertAlign").with_attr("w:val", v));
            }
        }
        self.ext.apply(rpr, inherited);
    }
}

// ---------------------------------------------------------------------------
// Paragraph formatting
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Border {
    pub val: String,
    /// Eighths of a point.
    pub sz: i64,
    pub color: String,
    /// Points.
    pub space: i64,
}

impl Border {
    /// A border a table says is switched off (`nil` or `none`): it hides one set further out.
    pub fn is_nil(&self) -> bool {
        self.val == "nil" || self.val == "none"
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Border>,
    /// Drawn between paragraphs that share the other borders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<Border>,
    /// The bar border: a line along the outside edge of the paragraph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar: Option<Border>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaFmt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// `left`/`start`, `center`, `right`/`end`, `both` (justify), `distribute`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// Twips (1/20 pt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_auto: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_auto: Option<bool>,
    /// Line spacing: with rule `auto` it is in 240ths of a line, otherwise twips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_left: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_right: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_first_line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_hanging: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_next: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_break_before: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_spacing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ilvl: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_lvl: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borders: Option<ParaBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bidi: Option<bool>,
    /// Widow/orphan control (`w:widowControl`), as the XML says; Word treats an unspecified value as on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widow_control: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_line_numbers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_auto_hyphens: Option<bool>,
    /// Tab stops (style and direct ones merged), in position order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabStop>>,
    /// Indents in hundredths of a character (`w:leftChars` and the like); when present they win over the twips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_left_chars: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_right_chars: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_first_line_chars: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ind_hanging_chars: Option<i64>,
    /// Spacing in hundredths of a line (`w:beforeLines`, `w:afterLines`); when present it wins over the twips.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before_lines: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after_lines: Option<i64>,
    /// Left and right indents are inside and outside indents: they swap on even pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirror_indents: Option<bool>,
    /// The paragraph's shading as the file has it (a pattern, colours, theme colours resolved by the renderer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shd: Option<Shd>,
    /// The paragraph is set apart as a positioned frame, or is a drop cap (`w:framePr`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<crate::layout_props::FrameFmt>,
    /// The Asian typography tab: line-breaking rules, auto spacing, grid snapping, vertical text (see east_asian.rs).
    #[serde(flatten)]
    pub ea: crate::east_asian::ParaEa,
}

/// A tab stop: `left`, `center`, `right`, `decimal` or `bar` at `pos` twips, with an optional
/// leader (`dot`, `hyphen`, `underscore`, `middleDot`). `clear` removes an inherited stop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabStop {
    pub val: String,
    pub pos: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
}

pub fn parse_border(e: &Element) -> Option<Border> {
    let val = e.attr("w:val")?.to_string();
    if val == "nil" || val == "none" {
        return None;
    }
    Some(Border {
        val,
        sz: num(e, "w:sz").unwrap_or(4),
        color: e.attr("w:color").and_then(crate::colour::auto_or_hex).unwrap_or_else(|| "auto".into()),
        space: num(e, "w:space").unwrap_or(0),
    })
}

impl ParaFmt {
    pub fn from_ppr(ppr: &Element) -> ParaFmt {
        let mut f = ParaFmt::default();
        for c in ppr.elements() {
            f.ea.read(c);
            match c.name.as_str() {
                "w:pStyle" => f.style = c.attr("w:val").map(String::from),
                "w:jc" => f.align = c.attr("w:val").map(String::from),
                "w:spacing" => {
                    f.space_before = num(c, "w:before");
                    f.space_after = num(c, "w:after");
                    f.line = num(c, "w:line");
                    f.line_rule = c.attr("w:lineRule").map(String::from).or(f.line.map(|_| "auto".into()));
                    f.before_auto = c.attr("w:beforeAutospacing").map(|v| v == "1" || v == "true");
                    f.after_auto = c.attr("w:afterAutospacing").map(|v| v == "1" || v == "true");
                    f.space_before_lines = num(c, "w:beforeLines");
                    f.space_after_lines = num(c, "w:afterLines");
                }
                "w:ind" => {
                    f.ind_left = num(c, "w:left").or(num(c, "w:start"));
                    f.ind_right = num(c, "w:right").or(num(c, "w:end"));
                    f.ind_first_line = num(c, "w:firstLine");
                    f.ind_hanging = num(c, "w:hanging");
                    f.ind_left_chars = num(c, "w:leftChars").or(num(c, "w:startChars"));
                    f.ind_right_chars = num(c, "w:rightChars").or(num(c, "w:endChars"));
                    f.ind_first_line_chars = num(c, "w:firstLineChars");
                    f.ind_hanging_chars = num(c, "w:hangingChars");
                }
                "w:mirrorIndents" => f.mirror_indents = Some(on_off(c)),
                "w:framePr" => f.frame = Some(crate::layout_props::FrameFmt::from_el(c)),
                "w:keepNext" => f.keep_next = Some(on_off(c)),
                "w:keepLines" => f.keep_lines = Some(on_off(c)),
                "w:pageBreakBefore" => f.page_break_before = Some(on_off(c)),
                "w:contextualSpacing" => f.contextual_spacing = Some(on_off(c)),
                "w:bidi" => f.bidi = Some(on_off(c)),
                "w:widowControl" => f.widow_control = Some(on_off(c)),
                "w:suppressLineNumbers" => f.suppress_line_numbers = Some(on_off(c)),
                "w:suppressAutoHyphens" => f.suppress_auto_hyphens = Some(on_off(c)),
                "w:numPr" => {
                    f.num_id = c.child("w:numId").and_then(|e| num(e, "w:val"));
                    f.ilvl = c.child("w:ilvl").and_then(|e| num(e, "w:val"));
                }
                "w:outlineLvl" => f.outline_lvl = num(c, "w:val"),
                "w:tabs" => {
                    let mut stops: Vec<TabStop> = c
                        .children_named("w:tab")
                        .filter_map(|t| {
                            Some(TabStop {
                                val: t.attr("w:val")?.to_string(),
                                pos: num(t, "w:pos")?,
                                leader: t.attr("w:leader").filter(|l| *l != "none").map(String::from),
                            })
                        })
                        .collect();
                    // Files list them in any order; everything downstream reads them left to right.
                    stops.sort_by_key(|t| t.pos);
                    f.tabs = Some(stops);
                }
                "w:shd" => {
                    f.shading = c.attr("w:fill").and_then(crate::colour::hex);
                    f.shd = Some(Shd::from_el(c));
                }
                "w:pBdr" => {
                    let b = ParaBorders {
                        top: c.child("w:top").and_then(parse_border),
                        bottom: c.child("w:bottom").and_then(parse_border),
                        left: c.child("w:left").or(c.child("w:start")).and_then(parse_border),
                        right: c.child("w:right").or(c.child("w:end")).and_then(parse_border),
                        between: c.child("w:between").and_then(parse_border),
                        bar: c.child("w:bar").and_then(parse_border),
                    };
                    f.borders = Some(b);
                }
                _ => {}
            }
        }
        f
    }

    pub fn overlay(&self, top: &ParaFmt) -> ParaFmt {
        macro_rules! pick {
            ($($f:ident),*) => { ParaFmt { $($f: top.$f.clone().or_else(|| self.$f.clone()),)* ea: self.ea.overlay(&top.ea) } };
        }
        let mut r = pick!(
            style, align, space_before, space_after, before_auto, after_auto, line, line_rule, ind_left, ind_right,
            ind_first_line, ind_hanging, keep_next, keep_lines, page_break_before, contextual_spacing, num_id, ilvl,
            outline_lvl, shading, borders, bidi, widow_control, suppress_line_numbers, suppress_auto_hyphens, tabs,
            ind_left_chars, ind_right_chars, ind_first_line_chars, ind_hanging_chars, space_before_lines, space_after_lines,
            mirror_indents, shd, frame
        );
        // Tab stops accumulate down the style chain; `clear` removes an inherited one.
        if let (Some(base), Some(top_tabs)) = (&self.tabs, &top.tabs) {
            let cleared = |pos: i64| top_tabs.iter().any(|t| t.pos == pos);
            let mut all: Vec<TabStop> = base.iter().filter(|t| !cleared(t.pos)).cloned().collect();
            all.extend(top_tabs.iter().cloned());
            all.retain(|t| t.val != "clear");
            all.sort_by_key(|t| t.pos);
            r.tabs = Some(all);
        } else if let Some(t) = &mut r.tabs {
            t.retain(|t| t.val != "clear");
        }
        // firstLine and hanging are mutually exclusive.
        let first = top.ind_first_line.is_some() || top.ind_first_line_chars.is_some();
        let hang = top.ind_hanging.is_some() || top.ind_hanging_chars.is_some();
        if first && !hang {
            r.ind_hanging = None;
            r.ind_hanging_chars = None;
        }
        if hang && !first {
            r.ind_first_line = None;
            r.ind_first_line_chars = None;
        }
        r
    }
}

/// A change to paragraph properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaPatch {
    /// Paragraph style id; empty string removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_before: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_after: Option<i64>,
    /// Line spacing as a multiple (1.0, 1.15, 1.5, 2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_multiple: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ind_left: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ind_right: Option<i64>,
    /// Positive = first-line indent, negative = hanging indent (twips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ind_first: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_next: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_break_before: Option<bool>,
    /// Widow/orphan control; `false` writes an explicit off so it overrides a style that turns it on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widow_control: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_line_numbers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_auto_hyphens: Option<bool>,
    /// Background fill (hex); an empty string removes the shading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    /// Replaces the paragraph borders; all four sides `None` removes them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borders: Option<ParaBorders>,
    /// Set list membership: `Some((numId, ilvl))`; numId 0 removes numbering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num: Option<(i64, i64)>,
    /// Line spacing as Word stores it, with `line_rule`: `auto` is in 240ths of a line, `exact` and `atLeast` in twips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_rule: Option<String>,
    /// Automatic spacing before or after (what Word uses for HTML); `false` removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_auto: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_auto: Option<bool>,
    /// "Don't add space between paragraphs of the same style".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contextual_spacing: Option<bool>,
    /// Left and right indents are inside and outside indents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_indents: Option<bool>,
    /// Outline level 0 to 8 (levels 1 to 9), 9 is body text; a negative value removes the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_lvl: Option<i64>,
    /// Text direction: true is right to left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidi: Option<bool>,
    /// The tab stops the paragraph should have, inherited ones included. The paragraph keeps the ones its style
    /// doesn't give, and `clear` entries for the style's stops that aren't wanted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabStop>>,
    /// Shading with a pattern (`pct25`, `diagStripe`, ...): replaces `shading` when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shd: Option<Shd>,
    /// Remove all direct paragraph formatting (keeps section properties).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    /// The Asian typography tab (see east_asian.rs).
    #[serde(flatten)]
    pub ea: crate::east_asian::ParaEaPatch,
}

impl ParaPatch {
    pub fn apply(&self, ppr: &mut Element) {
        self.apply_with(ppr, &ParaFmt::default())
    }

    /// Apply to a raw `w:pPr`. `inherited` is what the paragraph gets from its style and the document
    /// defaults, so switching a toggle off can write an explicit `w:val="0"` only when needed.
    pub fn apply_with(&self, ppr: &mut Element, inherited: &ParaFmt) {
        if self.clear {
            ppr.children.retain(|n| matches!(n, Node::Element(e) if e.name == "w:sectPr" || e.name == "w:pPrChange" || e.name == "w:rPr"));
        }
        if let Some(s) = &self.style {
            if s.is_empty() {
                ppr.remove_children("w:pStyle");
            } else {
                put_ppr(ppr, Element::new("w:pStyle").with_attr("w:val", s));
            }
        }
        if let Some(a) = &self.align {
            ppr.remove_children("w:jc");
            if !a.is_empty() {
                put_ppr(ppr, Element::new("w:jc").with_attr("w:val", a));
            }
        }
        if self.space_before.is_some()
            || self.space_after.is_some()
            || self.line_multiple.is_some()
            || self.line.is_some()
            || self.before_auto.is_some()
            || self.after_auto.is_some()
        {
            let mut sp = ppr.child("w:spacing").cloned().unwrap_or_else(|| Element::new("w:spacing"));
            if let Some(v) = self.space_before {
                sp.set_attr("w:before", v.to_string());
                sp.remove_attr("w:beforeAutospacing");
                sp.remove_attr("w:beforeLines");
            }
            if let Some(v) = self.space_after {
                sp.set_attr("w:after", v.to_string());
                sp.remove_attr("w:afterAutospacing");
                sp.remove_attr("w:afterLines");
            }
            if let Some(m) = self.line_multiple {
                sp.set_attr("w:line", ((m * 240.0).round() as i64).to_string());
                sp.set_attr("w:lineRule", "auto");
            }
            if let Some(l) = self.line {
                sp.set_attr("w:line", l.max(0).to_string());
                sp.set_attr("w:lineRule", self.line_rule.as_deref().filter(|r| matches!(*r, "auto" | "exact" | "atLeast")).unwrap_or("auto"));
            }
            for (on, attr, lines, val) in [(self.before_auto, "w:beforeAutospacing", "w:beforeLines", "w:before"), (self.after_auto, "w:afterAutospacing", "w:afterLines", "w:after")] {
                match on {
                    Some(true) => {
                        sp.set_attr(attr, "1");
                        sp.remove_attr(lines);
                        // Word writes the 14 point it uses beside the flag.
                        if sp.attr(val).is_none() {
                            sp.set_attr(val, "280");
                        }
                    }
                    Some(false) => {
                        sp.remove_attr(attr);
                    }
                    None => {}
                }
            }
            put_ppr(ppr, sp);
        }
        if self.ind_left.is_some() || self.ind_right.is_some() || self.ind_first.is_some() {
            let mut ind = ppr.child("w:ind").cloned().unwrap_or_else(|| Element::new("w:ind"));
            if let Some(v) = self.ind_left {
                ind.remove_attr("w:start");
                ind.remove_attr("w:startChars");
                ind.remove_attr("w:leftChars");
                ind.set_attr("w:left", v.max(0).to_string());
            }
            if let Some(v) = self.ind_right {
                ind.remove_attr("w:end");
                ind.remove_attr("w:endChars");
                ind.remove_attr("w:rightChars");
                ind.set_attr("w:right", v.max(0).to_string());
            }
            if let Some(v) = self.ind_first {
                ind.remove_attr("w:firstLine");
                ind.remove_attr("w:hanging");
                ind.remove_attr("w:firstLineChars");
                ind.remove_attr("w:hangingChars");
                if v > 0 {
                    ind.set_attr("w:firstLine", v.to_string());
                } else if v < 0 {
                    ind.set_attr("w:hanging", (-v).to_string());
                }
            }
            put_ppr(ppr, ind);
        }
        let toggle = |ppr: &mut Element, name: &'static str, on: bool, inh: Option<bool>, default_on: bool| {
            ppr.remove_children(name);
            if on {
                put_ppr(ppr, Element::new(name));
            } else if inh.unwrap_or(default_on) {
                put_ppr(ppr, Element::new(name).with_attr("w:val", "0"));
            }
        };
        if let Some(b) = self.keep_next {
            toggle(ppr, "w:keepNext", b, inherited.keep_next, false);
        }
        if let Some(b) = self.keep_lines {
            toggle(ppr, "w:keepLines", b, inherited.keep_lines, false);
        }
        if let Some(b) = self.page_break_before {
            toggle(ppr, "w:pageBreakBefore", b, inherited.page_break_before, false);
        }
        if let Some(b) = self.widow_control {
            toggle(ppr, "w:widowControl", b, inherited.widow_control, true);
        }
        if let Some(b) = self.suppress_line_numbers {
            toggle(ppr, "w:suppressLineNumbers", b, inherited.suppress_line_numbers, false);
        }
        if let Some(b) = self.suppress_auto_hyphens {
            toggle(ppr, "w:suppressAutoHyphens", b, inherited.suppress_auto_hyphens, false);
        }
        if let Some(b) = self.contextual_spacing {
            toggle(ppr, "w:contextualSpacing", b, inherited.contextual_spacing, false);
        }
        if let Some(b) = self.mirror_indents {
            toggle(ppr, "w:mirrorIndents", b, inherited.mirror_indents, false);
        }
        if let Some(b) = self.bidi {
            toggle(ppr, "w:bidi", b, inherited.bidi, false);
        }
        if let Some(l) = self.outline_lvl {
            ppr.remove_children("w:outlineLvl");
            if l >= 0 {
                put_ppr(ppr, Element::new("w:outlineLvl").with_attr("w:val", l.min(9).to_string()));
            }
        }
        if let Some(wanted) = &self.tabs {
            // The paragraph's own `num` stops (older Word files write them for converted list paragraphs) are not
            // something the ruler or the Tabs dialog shows, so a list that leaves them out is not asking to drop them.
            // They stay unless the list says something about the same position.
            let own_num: Vec<TabStop> = ppr
                .child("w:tabs")
                .map(|e| {
                    e.children_named("w:tab")
                        .filter(|t| t.attr("w:val") == Some("num"))
                        .filter_map(|t| {
                            Some(TabStop { val: "num".into(), pos: t.attr("w:pos")?.parse().ok()?, leader: t.attr("w:leader").filter(|l| *l != "none").map(String::from) })
                        })
                        .filter(|t| !wanted.iter().any(|w| w.pos == t.pos))
                        .collect()
                })
                .unwrap_or_default();
            ppr.remove_children("w:tabs");
            let inherited_tabs: &[TabStop] = inherited.tabs.as_deref().unwrap_or(&[]);
            let mut direct: Vec<TabStop> = wanted.iter().filter(|t| t.val != "clear" && !inherited_tabs.contains(t)).cloned().collect();
            direct.extend(own_num);
            for t in inherited_tabs {
                if !wanted.iter().any(|w| w.pos == t.pos && w.val != "clear") {
                    direct.push(TabStop { val: "clear".into(), pos: t.pos, leader: None });
                }
            }
            direct.sort_by_key(|t| t.pos);
            if !direct.is_empty() {
                let mut tabs = Element::new("w:tabs");
                for t in &direct {
                    let mut e = Element::new("w:tab").with_attr("w:val", &t.val);
                    if let Some(l) = t.leader.as_deref().filter(|l| *l != "none") {
                        e.set_attr("w:leader", l);
                    }
                    e.set_attr("w:pos", t.pos.to_string());
                    tabs = tabs.with_child(e);
                }
                put_ppr(ppr, tabs);
            }
        }
        if let Some(shd) = &self.shd {
            ppr.remove_children("w:shd");
            if shd.val != "nil" && !shd.val.is_empty() {
                put_ppr(ppr, shd.to_el());
            }
        } else if let Some(fill) = &self.shading {
            ppr.remove_children("w:shd");
            let fill = fill.trim_start_matches('#');
            if !fill.is_empty() && fill != "none" {
                put_ppr(ppr, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", fill.to_uppercase()));
            }
        }
        if let Some(b) = &self.borders {
            let old = ppr.child("w:pBdr").cloned();
            ppr.remove_children("w:pBdr");
            if b.top.is_some() || b.left.is_some() || b.bottom.is_some() || b.right.is_some() || b.between.is_some() || b.bar.is_some() {
                let mut bdr = Element::new("w:pBdr");
                for (name, side) in [("w:top", &b.top), ("w:left", &b.left), ("w:bottom", &b.bottom), ("w:right", &b.right)] {
                    if let Some(s) = side {
                        bdr = bdr.with_child(
                            Element::new(name)
                                .with_attr("w:val", &s.val)
                                .with_attr("w:sz", s.sz.to_string())
                                .with_attr("w:space", s.space.to_string())
                                .with_attr("w:color", &s.color),
                        );
                    }
                }
                // The between and bar borders stay unless the patch says something about them; a border with
                // the style `none` removes one.
                for (name, side) in [("w:between", &b.between), ("w:bar", &b.bar)] {
                    match side {
                        Some(s) if s.val != "none" && s.val != "nil" => {
                            bdr = bdr.with_child(
                                Element::new(name)
                                    .with_attr("w:val", &s.val)
                                    .with_attr("w:sz", s.sz.to_string())
                                    .with_attr("w:space", s.space.to_string())
                                    .with_attr("w:color", &s.color),
                            );
                        }
                        Some(_) => {}
                        None => {
                            if let Some(e) = old.as_ref().and_then(|o| o.child(name)) {
                                bdr = bdr.with_child(e.clone());
                            }
                        }
                    }
                }
                put_ppr(ppr, bdr);
            }
        }
        self.ea.apply(ppr, &inherited.ea);
        if let Some((num_id, ilvl)) = self.num {
            ppr.remove_children("w:numPr");
            // numId 0 explicitly removes numbering inherited from a style.
            put_ppr(
                ppr,
                Element::new("w:numPr")
                    .with_child(Element::new("w:ilvl").with_attr("w:val", ilvl.clamp(0, 8).to_string()))
                    .with_child(Element::new("w:numId").with_attr("w:val", num_id.to_string())),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_insert() {
        let mut rpr = Element::new("w:rPr").with_child(Element::new("w:sz").with_attr("w:val", "24"));
        RunPatch { bold: Some(true), color: Some("#ff0000".into()), ..Default::default() }.apply(&mut rpr, &RunFmt::default());
        let names: Vec<_> = rpr.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:b", "w:bCs", "w:color", "w:sz"]);
        let f = RunFmt::from_rpr(&rpr);
        assert_eq!(f.bold, Some(true));
        assert_eq!(f.size, Some(12.0));
        assert_eq!(f.color.as_deref(), Some("FF0000"));
    }

    #[test]
    fn unbold_against_style() {
        let mut rpr = Element::new("w:rPr");
        let inherited = RunFmt { bold: Some(true), ..Default::default() };
        RunPatch { bold: Some(false), ..Default::default() }.apply(&mut rpr, &inherited);
        assert_eq!(RunFmt::from_rpr(&rpr).bold, Some(false));
    }
}
