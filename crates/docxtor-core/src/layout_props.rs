//! Page layout properties beyond size and margins: page borders, vertical alignment, text direction, the note
//! options of a section (`w:footnotePr`, `w:endnotePr`), chapter page numbers, the document's layout settings
//! (mirrored margins, gutter at the top, hyphenation limits), the page background (colour, gradient, texture,
//! pattern or picture) and frames (`w:framePr`, drop caps).
//!
//! Everything here reads from and writes to the XML elements Word uses, so what Docxtor doesn't model stays put.
//! The ops that apply these live in `document/layout_ops.rs`.

use crate::notes::{NoteFormat, NoteKind};
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

fn num(e: &Element, attr: &str) -> Option<i64> {
    e.attr(attr).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64)
}

fn on_off(e: &Element) -> bool {
    !matches!(e.attr("w:val"), Some("0" | "false" | "off"))
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn default_auto() -> String {
    "auto".into()
}

// ---------------------------------------------------------------------------
// Page borders
// ---------------------------------------------------------------------------

/// The line styles of `ST_Border` that draw as lines; any other value is an art border.
pub const LINE_STYLES: &[&str] = &[
    "nil", "none", "single", "thick", "double", "dotted", "dashed", "dotDash", "dotDotDash", "triple", "thinThickSmallGap",
    "thickThinSmallGap", "thinThickThinSmallGap", "thinThickMediumGap", "thickThinMediumGap", "thinThickThinMediumGap",
    "thinThickLargeGap", "thickThinLargeGap", "thinThickThinLargeGap", "wave", "doubleWave", "dashSmallGap", "dashDotStroked",
    "threeDEmboss", "threeDEngrave", "outset", "inset",
];

/// One side of a page border.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageBorder {
    /// A line style (`single`, `double`, ...) or an art border name (`hearts`, `stars`, ...).
    pub val: String,
    /// Width in eighths of a point (for an art border, the art's size in points).
    #[serde(default = "four")]
    pub sz: i64,
    /// Distance from the text or the page edge, in points.
    #[serde(default)]
    pub space: i64,
    /// Hex colour or `auto`.
    #[serde(default = "default_auto")]
    pub color: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub shadow: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub frame: bool,
}

fn four() -> i64 {
    4
}

impl PageBorder {
    pub fn is_art(&self) -> bool {
        !LINE_STYLES.contains(&self.val.as_str())
    }

    fn read(e: &Element) -> Option<PageBorder> {
        let val = e.attr("w:val")?.to_string();
        if val == "nil" || val == "none" {
            return None;
        }
        Some(PageBorder {
            val,
            sz: num(e, "w:sz").unwrap_or(4),
            space: num(e, "w:space").unwrap_or(0),
            color: e.attr("w:color").and_then(crate::colour::auto_or_hex).unwrap_or_else(|| "auto".into()),
            shadow: e.attr("w:shadow").is_some_and(|v| matches!(v, "1" | "true" | "on")),
            frame: e.attr("w:frame").is_some_and(|v| matches!(v, "1" | "true" | "on")),
        })
    }

    fn element(&self, name: &'static str) -> Element {
        let mut e = Element::new(name).with_attr("w:val", &self.val).with_attr("w:sz", self.sz.clamp(0, 96).to_string()).with_attr("w:space", self.space.clamp(0, 31).to_string());
        e.set_attr("w:color", if self.color.is_empty() { "auto" } else { &self.color });
        if self.shadow {
            e.set_attr("w:shadow", "1");
        }
        if self.frame {
            e.set_attr("w:frame", "1");
        }
        e
    }
}

/// `w:pgBorders`: the four sides and where the borders count from and show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageBorders {
    /// `page` (the paper's edge) or `text` (the margins).
    #[serde(default = "page_edge")]
    pub offset_from: String,
    /// `allPages`, `firstPage` or `notFirstPage`.
    #[serde(default = "all_pages")]
    pub display: String,
    /// `front` or `back`.
    #[serde(default = "in_front")]
    pub z_order: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<PageBorder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<PageBorder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<PageBorder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<PageBorder>,
}

fn page_edge() -> String {
    "page".into()
}
fn all_pages() -> String {
    "allPages".into()
}
fn in_front() -> String {
    "front".into()
}

impl PageBorders {
    pub fn read(sect: &Element) -> Option<PageBorders> {
        let b = sect.child("w:pgBorders")?;
        let out = PageBorders {
            offset_from: b.attr("w:offsetFrom").unwrap_or("text").to_string(),
            display: b.attr("w:display").unwrap_or("allPages").to_string(),
            z_order: b.attr("w:zOrder").unwrap_or("front").to_string(),
            top: b.child("w:top").and_then(PageBorder::read),
            left: b.child("w:left").and_then(PageBorder::read),
            bottom: b.child("w:bottom").and_then(PageBorder::read),
            right: b.child("w:right").and_then(PageBorder::read),
        };
        (out.top.is_some() || out.left.is_some() || out.bottom.is_some() || out.right.is_some()).then_some(out)
    }

    /// The element Word writes: attributes for the placement, then top, left, bottom, right.
    pub fn element(&self) -> Element {
        let mut e = Element::new("w:pgBorders");
        if self.z_order == "back" {
            e.set_attr("w:zOrder", "back");
        }
        if self.display != "allPages" {
            e.set_attr("w:display", &self.display);
        }
        e.set_attr("w:offsetFrom", if self.offset_from == "text" { "text" } else { "page" });
        for (name, side) in [("w:top", &self.top), ("w:left", &self.left), ("w:bottom", &self.bottom), ("w:right", &self.right)] {
            if let Some(s) = side {
                e.children.push(Node::Element(s.element(name)));
            }
        }
        e
    }

    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.offset_from.as_str(), "page" | "text") {
            return Err(format!("Page borders count from 'page' or 'text', not '{}'.", self.offset_from));
        }
        if !matches!(self.display.as_str(), "allPages" | "firstPage" | "notFirstPage") {
            return Err(format!("Page borders show on 'allPages', 'firstPage' or 'notFirstPage', not '{}'.", self.display));
        }
        if !matches!(self.z_order.as_str(), "front" | "back") {
            return Err(format!("A page border sits in 'front' or at the 'back', not '{}'.", self.z_order));
        }
        for s in [&self.top, &self.left, &self.bottom, &self.right].into_iter().flatten() {
            if s.val.is_empty() || !s.val.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(format!("'{}' is not a border style.", s.val));
            }
            let c = s.color.trim_start_matches('#');
            if c != "auto" && !(c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit())) {
                return Err(format!("'{}' is not a colour; use six hex digits or auto.", s.color));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Notes per section
// ---------------------------------------------------------------------------

/// How a kind of note is numbered and where it goes in a section (or in the document, when the section says
/// nothing): the format and first number, when numbering starts again, and the position.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePr {
    pub fmt: String,
    pub start: i64,
    /// `continuous`, `eachSect` or `eachPage`.
    pub restart: String,
    /// Footnotes: `pageBottom` or `beneathText`. Endnotes: `sectEnd` or `docEnd`.
    pub pos: String,
}

impl NotePr {
    pub fn default_for(kind: NoteKind) -> NotePr {
        let f = NoteFormat::default_for(kind);
        NotePr { fmt: f.fmt, start: f.start, restart: "continuous".into(), pos: if kind == NoteKind::Footnote { "pageBottom" } else { "docEnd" }.into() }
    }

    /// The settings first, then the section on top of them, one property at a time.
    pub fn read(kind: NoteKind, settings: Option<&Element>, sect: Option<&Element>) -> NotePr {
        let mut out = NotePr::default_for(kind);
        for pr in [settings, sect].into_iter().flatten().filter_map(|e| e.child(kind.pr_tag())) {
            if let Some(f) = pr.child("w:numFmt").and_then(|n| n.attr("w:val")) {
                out.fmt = f.to_string();
            }
            if let Some(s) = pr.child("w:numStart").and_then(|n| num(n, "w:val")) {
                out.start = s;
            }
            if let Some(r) = pr.child("w:numRestart").and_then(|n| n.attr("w:val")) {
                out.restart = r.to_string();
            }
            if let Some(p) = pr.child("w:pos").and_then(|n| n.attr("w:val")) {
                out.pos = p.to_string();
            }
        }
        out
    }

    pub fn format(&self) -> NoteFormat {
        NoteFormat { fmt: self.fmt.clone(), start: self.start }
    }
}

/// A change to the note options: only the given properties change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NotePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
}

impl NotePatch {
    pub fn validate(&self, kind: NoteKind) -> Result<(), String> {
        if let Some(f) = &self.format {
            if crate::notes::ooxml_format(f.trim()).is_none() {
                return Err(format!("Unknown number format '{f}'. Use decimal, lowerRoman, upperRoman, lowerLetter, upperLetter or symbol."));
            }
        }
        if self.start.is_some_and(|s| s < 1) {
            return Err("The first note number has to be 1 or more.".into());
        }
        if let Some(r) = &self.restart {
            if !matches!(r.as_str(), "continuous" | "eachSect" | "eachPage") {
                return Err(format!("Notes restart 'continuous', 'eachSect' or 'eachPage', not '{r}'."));
            }
        }
        if let Some(p) = &self.pos {
            let ok = match kind {
                NoteKind::Footnote => matches!(p.as_str(), "pageBottom" | "beneathText"),
                NoteKind::Endnote => matches!(p.as_str(), "sectEnd" | "docEnd"),
            };
            if !ok {
                return Err(match kind {
                    NoteKind::Footnote => format!("Footnotes go at the 'pageBottom' or 'beneathText', not '{p}'."),
                    NoteKind::Endnote => format!("Endnotes go at the end of the section ('sectEnd') or the document ('docEnd'), not '{p}'."),
                });
            }
        }
        Ok(())
    }

    /// Write the given properties into a `w:footnotePr` / `w:endnotePr` element in schema order.
    pub fn write(&self, pr: &mut Element) {
        const ORDER: &[&str] = &["w:pos", "w:numFmt", "w:numStart", "w:numRestart", "w:footnote", "w:endnote"];
        if let Some(p) = &self.pos {
            crate::props::put_ordered(pr, Element::new("w:pos").with_attr("w:val", p), ORDER);
        }
        if let Some(f) = &self.format {
            let fmt = crate::notes::ooxml_format(f.trim()).unwrap_or("decimal");
            crate::props::put_ordered(pr, Element::new("w:numFmt").with_attr("w:val", fmt), ORDER);
        }
        if let Some(s) = self.start {
            crate::props::put_ordered(pr, Element::new("w:numStart").with_attr("w:val", s.to_string()), ORDER);
        }
        if let Some(r) = &self.restart {
            // Word leaves out the default (continuous).
            if r == "continuous" {
                pr.remove_children("w:numRestart");
            } else {
                crate::props::put_ordered(pr, Element::new("w:numRestart").with_attr("w:val", r), ORDER);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A section's extras
// ---------------------------------------------------------------------------

/// What a section says beyond its size, margins and columns.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionExtras {
    /// Where the text sits between the top and bottom margins: `center`, `both` (spread out) or `bottom`; absent is top.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v_align: Option<String>,
    /// `tbRl`, `btLr` and the other directions of `w:textDirection`; absent is left to right, top to bottom.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_borders: Option<PageBorders>,
    /// The Heading level (1 to 9) whose number goes before the page number, and the character between them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chap_style: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chap_sep: Option<String>,
    /// The section's own note options, settings and defaults applied.
    pub footnote_pr: NotePr,
    pub endnote_pr: NotePr,
    /// Endnotes are not collected in this section.
    #[serde(skip_serializing_if = "is_false")]
    pub no_endnote: bool,
    /// The document grid (`w:docGrid`): lines of a fixed pitch and, with characters, a fixed width per character.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_grid: Option<crate::east_asian::DocGrid>,
    /// The size of the font the grid's character width is added to (points): the default paragraph's font size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_base: Option<f64>,
}

impl SectionExtras {
    pub fn read(sect: &Element, settings: Option<&Element>) -> SectionExtras {
        let pg = sect.child("w:pgNumType");
        SectionExtras {
            v_align: sect.child("w:vAlign").and_then(|v| v.attr("w:val")).filter(|v| *v != "top").map(String::from),
            text_direction: sect.child("w:textDirection").and_then(|v| v.attr("w:val")).filter(|v| !matches!(*v, "lrTb" | "lr")).map(String::from),
            page_borders: PageBorders::read(sect),
            chap_style: pg.and_then(|n| num(n, "w:chapStyle")).filter(|v| (1..=9).contains(v)),
            chap_sep: pg.and_then(|n| n.attr("w:chapSep")).map(String::from),
            footnote_pr: NotePr::read(NoteKind::Footnote, settings, Some(sect)),
            endnote_pr: NotePr::read(NoteKind::Endnote, settings, Some(sect)),
            no_endnote: sect.child("w:noEndnote").is_some_and(on_off),
            doc_grid: crate::east_asian::DocGrid::read(sect),
            grid_base: None,
        }
    }
}

/// A change to a section's layout; only the given properties change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionPatch {
    /// Extra margin for binding (twips) and whether it sits on the right.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gutter: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtl_gutter: Option<bool>,
    /// Distance of the header and footer from the paper's edge (twips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<i64>,
    /// `top`, `center`, `both` or `bottom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_align: Option<String>,
    /// `lrTb`, `tbRl`, `btLr`, `lrTbV`, `tbRlV` or `tbLrV`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<String>,
    /// How the section starts: `nextPage`, `continuous`, `evenPage`, `oddPage`, `nextColumn`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_endnote: Option<bool>,
    /// The document grid; the kind `default` removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_grid: Option<crate::east_asian::DocGrid>,
}

impl SectionPatch {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(g) = &self.doc_grid {
            g.validate()?;
        }
        if let Some(v) = &self.v_align {
            if !matches!(v.as_str(), "top" | "center" | "both" | "bottom") {
                return Err(format!("Vertical alignment is 'top', 'center', 'both' or 'bottom', not '{v}'."));
            }
        }
        if let Some(v) = &self.text_direction {
            if !matches!(v.as_str(), "lrTb" | "tbRl" | "btLr" | "lrTbV" | "tbRlV" | "tbLrV") {
                return Err(format!("Text direction is lrTb, tbRl, btLr, lrTbV, tbRlV or tbLrV, not '{v}'."));
            }
        }
        if let Some(v) = &self.start {
            if !matches!(v.as_str(), "nextPage" | "continuous" | "evenPage" | "oddPage" | "nextColumn") {
                return Err(format!("A section starts 'nextPage', 'continuous', 'evenPage', 'oddPage' or 'nextColumn', not '{v}'."));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// The document-wide layout settings (`word/settings.xml`).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSettings {
    /// Inside and outside margins instead of left and right (`w:mirrorMargins`).
    #[serde(skip_serializing_if = "is_false")]
    pub mirror_margins: bool,
    /// The gutter sits at the top of the page (`w:gutterAtTop`).
    #[serde(skip_serializing_if = "is_false")]
    pub gutter_at_top: bool,
    /// `bookFold` or `bookFoldRev`: print as a booklet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_fold: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_fold_sheets: Option<i64>,
    /// Two pages on one sheet (`w:printTwoOnOne`).
    #[serde(skip_serializing_if = "is_false")]
    pub print_two_on_one: bool,
    /// Hyphenation: the zone (twips), the most hyphens in a row, and whether words in capitals are left alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphenation_zone: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consecutive_hyphen_limit: Option<i64>,
    #[serde(skip_serializing_if = "is_false")]
    pub do_not_hyphenate_caps: bool,
    /// The page background shows (`w:displayBackgroundShape`).
    #[serde(skip_serializing_if = "is_false")]
    pub display_background_shape: bool,
    /// Spelling and grammar underlines are hidden in this document (`w:hideSpellingErrors`, `w:hideGrammaticalErrors`).
    #[serde(skip_serializing_if = "is_false")]
    pub hide_spelling_errors: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub hide_grammatical_errors: bool,
    /// The document's own note options (before any section's).
    pub footnote_pr: NotePr,
    pub endnote_pr: NotePr,
}

impl LayoutSettings {
    pub fn read(settings: Option<&Element>) -> LayoutSettings {
        let flag = |n: &str| settings.and_then(|s| s.child(n)).is_some_and(on_off);
        let val = |n: &str| settings.and_then(|s| s.child(n)).and_then(|e| num(e, "w:val"));
        LayoutSettings {
            mirror_margins: flag("w:mirrorMargins"),
            gutter_at_top: flag("w:gutterAtTop"),
            book_fold: if flag("w:bookFoldPrinting") {
                Some("bookFold".into())
            } else if flag("w:bookFoldRevPrinting") {
                Some("bookFoldRev".into())
            } else {
                None
            },
            book_fold_sheets: val("w:bookFoldPrintingSheets"),
            print_two_on_one: flag("w:printTwoOnOne"),
            hyphenation_zone: val("w:hyphenationZone"),
            consecutive_hyphen_limit: val("w:consecutiveHyphenLimit"),
            do_not_hyphenate_caps: flag("w:doNotHyphenateCaps"),
            display_background_shape: flag("w:displayBackgroundShape"),
            hide_spelling_errors: flag("w:hideSpellingErrors"),
            hide_grammatical_errors: flag("w:hideGrammaticalErrors"),
            footnote_pr: NotePr::read(NoteKind::Footnote, settings, None),
            endnote_pr: NotePr::read(NoteKind::Endnote, settings, None),
        }
    }
}

/// A change to the layout settings; only the given ones change. A number of 0 removes a numeric setting.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_margins: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gutter_at_top: Option<bool>,
    /// Different headers and footers on odd and even pages (`w:evenAndOddHeaders`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_and_odd_headers: Option<bool>,
    /// `none`, `bookFold` or `bookFoldRev`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_fold: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_fold_sheets: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_two_on_one: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyphenation_zone: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consecutive_hyphen_limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub do_not_hyphenate_caps: Option<bool>,
    /// Hide the spelling or grammar underlines in this document only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_spelling_errors: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_grammatical_errors: Option<bool>,
}

impl LayoutPatch {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(b) = &self.book_fold {
            if !matches!(b.as_str(), "none" | "bookFold" | "bookFoldRev") {
                return Err(format!("Book fold is 'none', 'bookFold' or 'bookFoldRev', not '{b}'."));
            }
        }
        if self.hyphenation_zone.is_some_and(|z| !(0..=31680).contains(&z)) {
            return Err("The hyphenation zone is between 0 and 31680 twips.".into());
        }
        if self.consecutive_hyphen_limit.is_some_and(|z| !(0..=1000).contains(&z)) {
            return Err("The limit of hyphens in a row is a whole number; 0 means no limit.".into());
        }
        Ok(())
    }

    /// The `w:settings` children to set or remove: (name, Some(element) to put, None to remove).
    pub fn changes(&self) -> Vec<(&'static str, Option<Element>)> {
        let flag = |name: &'static str, on: bool| (name, on.then(|| Element::new(name)));
        let valued = |name: &'static str, v: i64| (name, (v > 0).then(|| Element::new(name).with_attr("w:val", v.to_string())));
        let mut out = Vec::new();
        if let Some(v) = self.mirror_margins {
            out.push(flag("w:mirrorMargins", v));
        }
        if let Some(v) = self.gutter_at_top {
            out.push(flag("w:gutterAtTop", v));
        }
        if let Some(v) = self.even_and_odd_headers {
            out.push(flag("w:evenAndOddHeaders", v));
        }
        if let Some(b) = &self.book_fold {
            out.push(flag("w:bookFoldPrinting", b == "bookFold"));
            out.push(flag("w:bookFoldRevPrinting", b == "bookFoldRev"));
            if b == "none" {
                out.push(("w:bookFoldPrintingSheets", None));
            }
        }
        if let Some(v) = self.book_fold_sheets {
            out.push(valued("w:bookFoldPrintingSheets", v));
        }
        if let Some(v) = self.print_two_on_one {
            out.push(flag("w:printTwoOnOne", v));
        }
        if let Some(v) = self.hyphenation_zone {
            out.push(valued("w:hyphenationZone", v));
        }
        if let Some(v) = self.consecutive_hyphen_limit {
            out.push(valued("w:consecutiveHyphenLimit", v));
        }
        if let Some(v) = self.do_not_hyphenate_caps {
            out.push(flag("w:doNotHyphenateCaps", v));
        }
        if let Some(v) = self.hide_spelling_errors {
            out.push(flag("w:hideSpellingErrors", v));
        }
        if let Some(v) = self.hide_grammatical_errors {
            out.push(flag("w:hideGrammaticalErrors", v));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The page background
// ---------------------------------------------------------------------------

/// The page background as the document has it: a colour, or a VML fill (`v:background` in `w:background`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageFill {
    /// `solid`, `gradient`, `radial` (a gradient from the middle or a corner), `pattern`, `texture` or `picture`.
    pub kind: String,
    /// The colour (hex) of a solid page, the first colour of a gradient, the foreground of a pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The second colour of a gradient, the background of a pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color2: Option<String>,
    /// The gradient's direction in VML degrees (0 changes from top to bottom, -90 from left to right).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// For a radial gradient, where it starts: `center`, `topLeft`, `topRight`, `bottomLeft` or `bottomRight`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// For a pattern, texture or picture: the image part (`word/media/image1.png`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    /// The name of a texture or pattern ("Paper bag").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl PageFill {
    /// Read `w:background`; `rel` turns a relationship id into the image part it names.
    pub fn read(bg: &Element, rel: &dyn Fn(&str) -> Option<String>) -> PageFill {
        let hex = |s: Option<&str>| s.and_then(crate::colour::hex_hash);
        let colour = hex(bg.attr("w:color"));
        let vbg = bg.child("v:background");
        let fill = vbg.and_then(|b| b.child("v:fill"));
        let (Some(vbg), Some(fill)) = (vbg, fill) else {
            return PageFill { kind: "solid".into(), color: colour, color2: None, angle: None, from: None, media: None, title: None };
        };
        let kind = match fill.attr("type") {
            Some("gradient") => "gradient",
            Some("gradientRadial") | Some("gradientradial") => "radial",
            Some("pattern") => "pattern",
            Some("tile") => "texture",
            Some("frame") => "picture",
            _ => "solid",
        };
        let vml_colour = |s: Option<&str>| hex(s.map(|c| c.split_whitespace().next().unwrap_or(c)));
        let media = fill.attr("r:id").and_then(rel);
        let from = match (fill.attr("focusposition"), fill.attr("focussize")) {
            (Some(p), _) => {
                let p = p.replace(' ', "");
                Some(match p.as_str() {
                    "0,0" => "topLeft",
                    "1,0" | ".5,0" => "topRight",
                    "0,1" => "bottomLeft",
                    "1,1" => "bottomRight",
                    _ => "center",
                })
            }
            _ => Some("center"),
        };
        PageFill {
            kind: kind.into(),
            color: vml_colour(vbg.attr("fillcolor")).or(colour),
            color2: vml_colour(fill.attr("color2")),
            angle: fill.attr("angle").and_then(|a| a.parse::<f64>().ok()),
            from: if kind == "radial" { from.map(String::from) } else { None },
            media,
            title: fill.attr("o:title").filter(|t| !t.is_empty()).map(String::from),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let ok = |c: &Option<String>| c.as_ref().is_none_or(|c| {
            let c = c.trim_start_matches('#');
            c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit())
        });
        if !ok(&self.color) || !ok(&self.color2) {
            return Err("A fill colour is six hex digits like FFF2CC.".into());
        }
        match self.kind.as_str() {
            "solid" => {
                if self.color.is_none() {
                    return Err("A solid page colour needs a colour.".into());
                }
            }
            "gradient" | "radial" => {
                if self.color.is_none() || self.color2.is_none() {
                    return Err("A gradient needs two colours.".into());
                }
            }
            "pattern" | "texture" | "picture" => {
                if self.media.is_none() {
                    return Err("A pattern, texture or picture needs the image to fill the page with; add it with add_media first.".into());
                }
            }
            other => return Err(format!("A page fill is solid, gradient, radial, pattern, texture or picture, not '{other}'.")),
        }
        Ok(())
    }

    /// The `w:background` element Word writes. `rid` is the relationship to the image, for the fills that use one.
    pub fn element(&self, rid: Option<&str>) -> Element {
        let hex = |c: &Option<String>| c.as_deref().map(|c| c.trim_start_matches('#').to_ascii_uppercase());
        let color = hex(&self.color);
        let mut bg = Element::new("w:background").with_attr("w:color", color.clone().unwrap_or_else(|| "FFFFFF".into()));
        if self.kind == "solid" {
            return bg;
        }
        let mut fill = Element::new("v:fill");
        match self.kind.as_str() {
            "gradient" => {
                fill.set_attr("type", "gradient");
                if let Some(c2) = hex(&self.color2) {
                    fill.set_attr("color2", format!("#{c2}"));
                }
                fill.set_attr("angle", format_angle(self.angle.unwrap_or(0.0)));
                fill.set_attr("focus", "100%");
                fill.set_attr("method", "none");
            }
            "radial" => {
                fill.set_attr("type", "gradientRadial");
                if let Some(c2) = hex(&self.color2) {
                    fill.set_attr("color2", format!("#{c2}"));
                }
                let pos = match self.from.as_deref() {
                    Some("topLeft") => "0,0",
                    Some("topRight") => "1,0",
                    Some("bottomLeft") => "0,1",
                    Some("bottomRight") => "1,1",
                    _ => ".5,.5",
                };
                fill.set_attr("focusposition", pos);
                fill.set_attr("focussize", "0,0");
                fill.set_attr("focus", "100%");
            }
            "pattern" | "texture" | "picture" => {
                if let Some(r) = rid {
                    fill.set_attr("r:id", r);
                }
                if let Some(t) = &self.title {
                    fill.set_attr("o:title", t);
                }
                fill.set_attr("type", match self.kind.as_str() {
                    "pattern" => "pattern",
                    "texture" => "tile",
                    _ => "frame",
                });
                if self.kind == "pattern" {
                    if let Some(c2) = hex(&self.color2) {
                        fill.set_attr("color2", format!("#{c2}"));
                    }
                } else {
                    fill.set_attr("recolor", "t");
                    fill.set_attr("rotate", "t");
                }
            }
            _ => {}
        }
        let mut v = Element::new("v:background").with_attr("id", "_x0000_s1025").with_attr("o:bwmode", "white");
        if let Some(c) = &color {
            v.set_attr("fillcolor", format!("#{c}"));
        }
        v.set_attr("o:targetscreensize", "1024,768");
        bg.children.push(Node::Element(v.with_child(fill)));
        bg
    }
}

fn format_angle(a: f64) -> String {
    if a.fract() == 0.0 {
        format!("{}", a as i64)
    } else {
        format!("{a}")
    }
}

// ---------------------------------------------------------------------------
// Frames and drop caps
// ---------------------------------------------------------------------------

/// `w:framePr`: a paragraph set apart as a positioned text block, or a drop cap.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameFmt {
    /// `drop` (inside the text) or `margin` (in the margin); absent for an ordinary frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_cap: Option<String>,
    /// For a drop cap: how many lines it spans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<i64>,
    /// Width and height (twips) and whether the height is `exact`, `atLeast` or `auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_rule: Option<String>,
    /// Space around the frame (twips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_space: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_space: Option<i64>,
    /// How text goes round it: `auto`, `notBeside`, `around`, `tight`, `through` or `none`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    /// What the position counts from: `text`, `margin` or `page`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v_anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i64>,
    /// `left`, `center`, `right`, `inside` or `outside`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_align: Option<String>,
    /// `inline`, `top`, `center`, `bottom`, `inside` or `outside`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_lock: Option<bool>,
}

impl FrameFmt {
    pub fn from_el(e: &Element) -> FrameFmt {
        let s = |a: &str| e.attr(a).map(String::from);
        FrameFmt {
            drop_cap: s("w:dropCap").filter(|v| v != "none"),
            lines: num(e, "w:lines"),
            w: num(e, "w:w"),
            h: num(e, "w:h"),
            h_rule: s("w:hRule"),
            h_space: num(e, "w:hSpace"),
            v_space: num(e, "w:vSpace"),
            wrap: s("w:wrap"),
            h_anchor: s("w:hAnchor"),
            v_anchor: s("w:vAnchor"),
            x: num(e, "w:x"),
            y: num(e, "w:y"),
            x_align: s("w:xAlign"),
            y_align: s("w:yAlign"),
            anchor_lock: e.attr("w:anchorLock").map(|v| matches!(v, "1" | "true" | "on")),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let one_of = |what: &str, v: &Option<String>, ok: &[&str]| match v {
            Some(v) if !ok.contains(&v.as_str()) => Err(format!("{what} is one of {}, not '{v}'.", ok.join(", "))),
            _ => Ok(()),
        };
        one_of("A drop cap", &self.drop_cap, &["none", "drop", "margin"])?;
        one_of("The height rule", &self.h_rule, &["auto", "exact", "atLeast"])?;
        one_of("Wrapping", &self.wrap, &["auto", "notBeside", "around", "tight", "through", "none"])?;
        one_of("The horizontal anchor", &self.h_anchor, &["text", "margin", "page"])?;
        one_of("The vertical anchor", &self.v_anchor, &["text", "margin", "page"])?;
        one_of("The horizontal alignment", &self.x_align, &["left", "center", "right", "inside", "outside"])?;
        one_of("The vertical alignment", &self.y_align, &["inline", "top", "center", "bottom", "inside", "outside"])?;
        if self.lines.is_some_and(|l| !(1..=10).contains(&l)) {
            return Err("A drop cap spans 1 to 10 lines.".into());
        }
        if self.w.is_some_and(|w| w < 0) || self.h.is_some_and(|h| h < 0) {
            return Err("A frame's width and height can't be negative.".into());
        }
        Ok(())
    }

    /// The `w:framePr` element, with the attributes in the order Word writes them.
    pub fn element(&self) -> Element {
        let mut e = Element::new("w:framePr");
        let mut s = |k: &'static str, v: Option<String>| {
            if let Some(v) = v {
                e.set_attr(k, v);
            }
        };
        s("w:dropCap", self.drop_cap.clone());
        s("w:lines", self.lines.map(|v| v.to_string()));
        s("w:w", self.w.map(|v| v.to_string()));
        s("w:h", self.h.map(|v| v.to_string()));
        s("w:vSpace", self.v_space.map(|v| v.to_string()));
        s("w:hSpace", self.h_space.map(|v| v.to_string()));
        s("w:wrap", self.wrap.clone());
        s("w:hAnchor", self.h_anchor.clone());
        s("w:vAnchor", self.v_anchor.clone());
        s("w:x", self.x.map(|v| v.to_string()));
        s("w:xAlign", self.x_align.clone());
        s("w:y", self.y.map(|v| v.to_string()));
        s("w:yAlign", self.y_align.clone());
        s("w:hRule", self.h_rule.clone());
        s("w:anchorLock", self.anchor_lock.filter(|v| *v).map(|_| "1".to_string()));
        e
    }
}

/// Options of a drop cap (Insert, Drop cap, Options).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropCapOptions {
    /// `none`, `drop` or `margin`.
    pub kind: String,
    /// Lines to drop, 1 to 10 (3 by default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<i64>,
    /// Distance from the text (twips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<i64>,
    /// The letter's font, if not the paragraph's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}

// ---------------------------------------------------------------------------
// Page numbers
// ---------------------------------------------------------------------------

/// Where a page number goes and how it looks (Insert, Page numbers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageNumberSpec {
    /// `top` (the header), `bottom` (the footer), `margin_left` or `margin_right` (a frame in the margin).
    pub place: String,
    /// `left`, `center` or `right`; centred by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// `plain` (1) or `x_of_y` (Page 1 of 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The words around the numbers of `x_of_y`, in the document's language ("Page", "of").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_word: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of_word: Option<String>,
    /// The header or footer to put it in: `default` (the usual), `first` or `even`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl PageNumberSpec {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.place.as_str(), "top" | "bottom" | "margin_left" | "margin_right") {
            return Err(format!("A page number goes at the 'top', the 'bottom' or in the 'margin_left' or 'margin_right', not '{}'.", self.place));
        }
        if let Some(a) = &self.align {
            if !matches!(a.as_str(), "left" | "center" | "right") {
                return Err(format!("A page number is aligned 'left', 'center' or 'right', not '{a}'."));
            }
        }
        if let Some(s) = &self.style {
            if !matches!(s.as_str(), "plain" | "x_of_y") {
                return Err(format!("A page number is 'plain' or 'x_of_y', not '{s}'."));
            }
        }
        if let Some(v) = &self.variant {
            if !matches!(v.as_str(), "default" | "first" | "even") {
                return Err(format!("The header or footer is 'default', 'first' or 'even', not '{v}'."));
            }
        }
        Ok(())
    }
}
