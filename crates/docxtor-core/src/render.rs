//! View model: the document with all formatting resolved, ready for a renderer
//! (the web UI) to lay out without knowing anything about OOXML.

use crate::model::*;
use crate::notes::{NoteFormat, NoteKind};
use crate::numbering::{ListLabel, Numbering};
use crate::package::{Package, Relationship};
use crate::props::{parse_border, Border, ParaFmt, RunFmt};
use crate::styles::StyleSheet;
use crate::xml::Element;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t")]
#[allow(clippy::large_enum_variant)] // short-lived view model; boxing would only add indirection
pub enum RBlock {
    #[serde(rename = "p")]
    Para(RPara),
    #[serde(rename = "tbl")]
    Table(RTable),
    #[serde(rename = "c")]
    Container { id: NodeId, #[serde(skip_serializing_if = "Option::is_none")] label: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] gallery: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] control: Option<crate::controls::RControl>, blocks: Vec<RBlock> },
    #[serde(rename = "x")]
    Opaque { id: NodeId, #[serde(skip_serializing_if = "Option::is_none")] label: Option<String> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPara {
    pub id: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    pub fmt: ParaFmt,
    /// Formatting of the paragraph mark (sizes empty lines and list labels).
    pub mark: RunFmt,
    pub segs: Vec<RSeg>,
    pub len: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<ListLabel>,
    /// Section break after this paragraph: how the next section starts (`nextPage`, `continuous`,
    /// `evenPage`, `oddPage`, `nextColumn`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sect_break: Option<String>,
    /// Bookmarks that start in this paragraph (targets of internal links like a table of contents).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bookmarks: Vec<String>,
    /// An empty paragraph whose only border is one along the bottom: a horizontal line.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hr: bool,
    /// The language of the paragraph's text (BCP 47, as the file writes it): the first run with text, else the
    /// paragraph mark, else the paragraph style and the styles it is based on, else the document default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The content controls inside the paragraph (see `controls::RControl`), outer before inner.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<crate::controls::RControl>,
    /// Legacy form fields in the paragraph.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<crate::forms::RForm>,
    /// The imported chunk (`w:altChunk`) this paragraph is shown from, until it is made editable (see `alt_chunk`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk: Option<NodeId>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RSeg {
    /// Start offset (UTF-16) within the paragraph.
    pub o: usize,
    /// Kind: `t` text, `tab`, `br` line break, `pbr` page break, `cbr` column break, `img`, `shape` (a drawing shape or
    /// text box, see `shape`), `math` (an equation), `obj`, `sym` (one character of a symbol font, drawn as `x`),
    /// `val` (a page number or date element: one position whose text the layout fills in, `fld` says which),
    /// `eq` (an `EQ` field drawn as `math`, zero-length), `ff` (a legacy form field: `label` is `checkbox` and `x` is
    /// `1` when checked; zero-length) or `ghost` (read-only, zero-length: tracked deletions; `label` says which).
    pub k: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    pub f: RunFmt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub img: Option<Box<RImage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Field instruction for field results (`PAGE`, `NUMPAGES`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fld: Option<String>,
    /// The source tags of a citation (the result of a `CITATION` field), so a renderer can treat the text as one
    /// unit and offer to edit it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cite: Option<Vec<String>>,
    /// The whole instruction of a field, on the first piece of its result (the layout needs it for `STYLEREF`,
    /// and the editor shows it when field codes are shown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fi: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ins: bool,
    /// Text inside a field's code (the result of a field nested in another one's instruction): it keeps its
    /// place in the text but isn't shown.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hid: bool,
    /// For note marks (`k` is `note`): `endnote` for an endnote; absent for a footnote. `x` is the note's id and
    /// `label` its formatted number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'static str>,
    /// For shapes and text boxes (`k` is `shape`): the geometry, colours, position and the text inside.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Box<crate::shapes::RShape>>,
    /// For equations (`k` is `math`; one position, like an image): the MathML to draw, the LaTeX to edit and
    /// whether it sits on its own line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math: Option<crate::math::MathInfo>,
    /// For charts (`k` is `chart`): what to draw. `img` holds the size and placement (`src` is the chart part).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<Box<crate::chart::ChartModel>>,
    /// For the new chart types (waterfall, histogram, treemap, ...; `k` is `chart` too): what to draw. `img` holds
    /// the size and placement (`src` is the chartex part). Absent: the chart is not one of these.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chartex: Option<Box<crate::chart::chartex::ChartExModel>>,
    /// A drawing object that is hidden (`hidden="1"` on its `wp:docPr`): not drawn or printed, still in the file.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// For phonetic guides (`k` is `ruby`; one position, like an image): the base text, the guide and its settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby: Option<Box<crate::ruby::RRuby>>,
    /// For a horizontal line drawn as a VML rule (`k` is `hrule`): Word's Borders, Horizontal line and Google Docs'
    /// horizontal lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hrule: Option<RHrule>,
}

/// A horizontal line (a VML rectangle marked `o:hr`), as Word draws it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RHrule {
    /// Width in percent of the text column, or `None` with `w` in points.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    /// Thickness in points.
    pub h: f64,
    /// `left`, `center` or `right`.
    pub align: String,
    /// The fill colour (`A0A0A0`), or `None` for Word's grey.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Google Docs' code block marks (U+EC03 at the start, U+EC02 at the end) as zero-width spaces: same length, no glyph.
fn google_marks(t: String) -> String {
    if t.contains(['\u{EC02}', '\u{EC03}']) {
        t.replace(['\u{EC02}', '\u{EC03}'], "\u{200B}")
    } else {
        t
    }
}

/// The horizontal line a `w:pict` holds, if it is one.
pub fn vml_hrule(e: &Element) -> Option<RHrule> {
    if e.name != "w:pict" {
        return None;
    }
    let r = e.elements().find(|c| matches!(c.name.as_str(), "v:rect" | "v:shape") && c.attr("o:hr") == Some("t"))?;
    let style = r.attr("style").unwrap_or("");
    let pt = |k: &str| {
        style.split(';').find_map(|kv| {
            let (a, b) = kv.split_once(':')?;
            (a.trim() == k).then(|| b.trim().trim_end_matches("pt").parse::<f64>().ok()).flatten()
        })
    };
    // `o:hrpct` is in tenths of a percent; none (or 0) with no width is the whole column.
    let pct = r.attr("o:hrpct").and_then(|v| v.parse::<f64>().ok()).filter(|v| *v > 0.0).map(|v| v / 10.0);
    let w = pt("width").filter(|w| *w > 0.0 && pct.is_none());
    let color = r.attr("fillcolor").and_then(crate::colour::hex_hash);
    Some(RHrule { pct: if w.is_none() { Some(pct.unwrap_or(100.0)) } else { None }, w, h: pt("height").filter(|h| *h > 0.0).unwrap_or(1.5), align: r.attr("o:hralign").unwrap_or("center").to_string(), color })
}

#[derive(Debug, Clone, Serialize)]
pub struct RImage {
    /// Part name of the media (`word/media/image1.png`), or an external URL.
    pub src: String,
    /// Size in points.
    pub w: f64,
    pub h: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub floating: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// The drawing's name (`wp:docPr name`), as the Selection pane lists it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(flatten)]
    pub layout: crate::image::ImageLayout,
    /// How the picture is formatted (crop, turn, outline, adjustments, effects); absent for a plain one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic: Option<crate::picture::PicFmt>,
    /// Text distances and anchor behaviour of a floating object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<crate::picture::PlaceInfo>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TableBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Border>,
    #[serde(rename = "insideH", skip_serializing_if = "Option::is_none")]
    pub inside_h: Option<Border>,
    #[serde(rename = "insideV", skip_serializing_if = "Option::is_none")]
    pub inside_v: Option<Border>,
}

impl TableBorders {
    fn apply(&mut self, el: &Element) {
        let set = |names: &[&str], slot: &mut Option<Border>| {
            for n in names {
                if let Some(b) = el.child(n) {
                    *slot = parse_border(b);
                }
            }
        };
        set(&["w:top"], &mut self.top);
        set(&["w:bottom"], &mut self.bottom);
        set(&["w:left", "w:start"], &mut self.left);
        set(&["w:right", "w:end"], &mut self.right);
        set(&["w:insideH"], &mut self.inside_h);
        set(&["w:insideV"], &mut self.inside_v);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RTable {
    pub id: NodeId,
    /// Column widths in twips.
    pub grid: Vec<i64>,
    pub borders: TableBorders,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent: Option<i64>,
    /// Total width in twips if fixed, or percentage*50 when `width_pct`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub width_pct: bool,
    /// Default cell margins (twips): top, right, bottom, left.
    pub cell_mar: [i64; 4],
    /// Space between cells in twips (`w:tblCellSpacing`); the cells then each draw their own borders.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<i64>,
    /// `w:tblLayout w:type="fixed"`: the columns keep the grid's widths. Otherwise (autofit) a column grows
    /// to what its content needs.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fixed: bool,
    /// A table placed by the page with text wrapping around it (`w:tblpPr`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub float: Option<RTableFloat>,
    /// The table's style (its id) and its display name, and which parts of the style the table shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<RTableLook>,
    /// Right-to-left table (`w:bidiVisual`): the first column is on the right.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bidi_visual: bool,
    pub rows: Vec<RRow>,
    /// The imported chunk this table is shown from (see `RPara::chunk`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk: Option<NodeId>,
}

/// `w:tblLook`: the options of a table design.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RTableLook {
    pub first_row: bool,
    pub last_row: bool,
    pub first_column: bool,
    pub last_column: bool,
    pub banded_rows: bool,
    pub banded_columns: bool,
}

/// Where a floating table sits and how close the text comes (`w:tblpPr`, twips).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RTableFloat {
    /// `text`, `margin` or `page`.
    pub horz_anchor: String,
    pub vert_anchor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i64>,
    /// `left`, `center`, `right`, `inside` or `outside`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_align: Option<String>,
    /// `inline`, `top`, `center`, `bottom`, `inside` or `outside`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_align: Option<String>,
    pub left: i64,
    pub right: i64,
    pub top: i64,
    pub bottom: i64,
    /// `w:tblOverlap w:val="never"`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub never_overlap: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    /// `atLeast` or `exact` (an exact height clips what does not fit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_rule: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
    /// The row moves to the next page whole instead of splitting (`w:cantSplit`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cant_split: bool,
    /// Grid columns left empty before the first cell and after the last (`w:gridBefore`, `w:gridAfter`).
    #[serde(skip_serializing_if = "is_zero")]
    pub grid_before: i64,
    #[serde(skip_serializing_if = "is_zero")]
    pub grid_after: i64,
    /// The name of the content control this row is in, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdt: Option<String>,
    /// The innermost content control around the row (`ctl` is its id), and the repeating section it is an item of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<crate::controls::RControl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<crate::controls::RControl>,
    pub cells: Vec<RCell>,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RCell {
    pub span: i64,
    /// `restart` starts a vertical merge, `continue` is covered by the cell above.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vmerge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valign: Option<String>,
    /// The cell's own margins (top, right, bottom, left) where they differ from the table's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mar: Option<[i64; 4]>,
    /// Diagonal lines: top left to bottom right, and top right to bottom left.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diag_down: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diag_up: Option<Border>,
    /// The name of the content control this cell is in, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdt: Option<String>,
    /// The direction the text runs in: `tbRl` (down) or `btLr` (up); across is left out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_dir: Option<String>,
    /// The text stays on one line (`w:noWrap`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_wrap: bool,
    pub borders: TableBorders,
    pub blocks: Vec<RBlock>,
}

/// Whether a block produces any visible output (cheap; no rendering).
pub fn is_visible(b: &Block) -> bool {
    match b {
        Block::Opaque(o) => !matches!(
            o.el.name.as_str(),
            "w:sectPr" | "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr" | "w:permStart" | "w:permEnd" | "w:commentRangeStart"
                | "w:commentRangeEnd" | "w:moveFromRangeStart" | "w:moveFromRangeEnd" | "w:moveToRangeStart" | "w:moveToRangeEnd"
        ),
        _ => true,
    }
}

pub struct RenderCtx<'a> {
    pub styles: &'a StyleSheet,
    pub pkg: &'a Package,
    pub part: String,
    pub labels: &'a HashMap<NodeId, ListLabel>,
    pub numbering: &'a Numbering,
    rels: HashMap<String, Relationship>,
    /// Text formatting contributed by the enclosing table's style.
    table_text: Option<(ParaFmt, RunFmt)>,
    footnote_counter: std::cell::Cell<u32>,
    endnote_counter: std::cell::Cell<u32>,
    /// Footnote numbers by note id (in body reference order).
    pub notes: HashMap<String, u32>,
    /// Endnote numbers by note id (in body reference order).
    pub endnotes: HashMap<String, u32>,
    /// How footnotes and endnotes are numbered (their labels).
    pub footnote_fmt: NoteFormat,
    pub endnote_fmt: NoteFormat,
    /// Number of the note being rendered (for its own `w:footnoteRef` or `w:endnoteRef` mark).
    pub current_note: Option<u32>,
    /// Whether that note is an endnote.
    pub current_endnote: bool,
    /// The labels notes show by key (the id; `e:` and the id for an endnote), where sections number them their own
    /// way. A note without an entry gets its number in the document's format.
    pub note_labels: HashMap<String, String>,
    /// The label of the note being rendered, for its own mark.
    pub current_note_label: Option<String>,
    /// Section breaks by paragraph id (see `Document::section_breaks`); empty outside the main text.
    pub sect_breaks: HashMap<NodeId, String>,
    /// The text of text boxes and the theme, for drawing shapes (see `shapes`).
    pub shapes: crate::shapes::ShapeEnv<'a>,
    /// The imported chunks of the main text, by the id of their container (see `alt_chunk`); None where there are none.
    pub(crate) alts: Option<&'a HashMap<NodeId, std::sync::Arc<crate::alt_chunk::AltChunk>>>,
}

mod blocks;
mod inlines;
mod objects;
mod scripts;
mod tables;

pub use scripts::is_complex;
use scripts::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn pict(xml: &str) -> Element {
        crate::xml::parse(format!(r#"<w:pict xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office">{xml}</w:pict>"#).as_bytes()).unwrap().root
    }

    #[test]
    fn a_vml_rule_is_a_horizontal_line() {
        // Google Docs' horizontal line: the whole column, 1.5 pt, grey.
        let h = vml_hrule(&pict(r##"<v:rect style="width:0.0pt;height:1.5pt" o:hr="t" o:hrstd="t" o:hralign="center" fillcolor="#A0A0A0" stroked="f"/>"##)).unwrap();
        assert_eq!(h, RHrule { pct: Some(100.0), w: None, h: 1.5, align: "center".into(), color: Some("A0A0A0".into()) });
        // Word's, half the column on the right, 3 pt.
        let h = vml_hrule(&pict(r#"<v:rect style="width:0;height:3pt" o:hr="t" o:hrpct="500" o:hralign="right" o:hrstd="t" fillcolor="red" stroked="f"/>"#)).unwrap();
        assert_eq!((h.pct, h.h, h.align.as_str(), h.color), (Some(50.0), 3.0, "right", None));
        // A rectangle that isn't a rule is not one.
        assert!(vml_hrule(&pict(r#"<v:rect style="width:100pt;height:50pt"/>"#)).is_none());
    }

    #[test]
    fn google_code_block_marks_take_no_room() {
        let t = google_marks("\u{EC03}ticker,shares\u{EC02}".to_string());
        assert_eq!(t, "\u{200B}ticker,shares\u{200B}");
        assert_eq!(t.encode_utf16().count(), "\u{EC03}ticker,shares\u{EC02}".encode_utf16().count());
    }
}
