//! Table styles as Word applies them: the conditional formatting of `w:tblStylePr` (header and total rows, first
//! and last column, banded rows and columns, corner cells), switched on by `w:tblLook` and laid over the style's
//! whole-table formatting. The order is Word's: whole table, vertical bands, horizontal bands, first and last
//! column, first and last row, corner cells, then the table's and the cell's direct formatting.
//!
//! `TableStyler` is built once for a table and answers for each cell: its fill, its four borders and two diagonals,
//! its margins and vertical alignment, and the paragraph and run formatting its text starts from.

use crate::props::{parse_border, Border, ParaFmt, RunFmt};
use crate::styles::StyleSheet;
use crate::xml::Element;

/// The six switches of `w:tblLook`: which parts of a table style the table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableLook {
    pub first_row: bool,
    pub last_row: bool,
    pub first_col: bool,
    pub last_col: bool,
    /// Rows are banded (`noHBand` off).
    pub h_band: bool,
    /// Columns are banded (`noVBand` off).
    pub v_band: bool,
}

impl Default for TableLook {
    /// What Word gives a new table: header row and first column, banded rows (`04A0`).
    fn default() -> Self {
        TableLook { first_row: true, last_row: false, first_col: true, last_col: false, h_band: true, v_band: false }
    }
}

const LOOK_FIRST_ROW: u32 = 0x0020;
const LOOK_LAST_ROW: u32 = 0x0040;
const LOOK_FIRST_COL: u32 = 0x0080;
const LOOK_LAST_COL: u32 = 0x0100;
const LOOK_NO_H_BAND: u32 = 0x0200;
const LOOK_NO_V_BAND: u32 = 0x0400;

fn flag(v: &str) -> bool {
    matches!(v, "1" | "true" | "on")
}

impl TableLook {
    /// The look a `w:tblPr` gives: the attribute form (`w:firstRow="1"`) where present, else the old hex `w:val`.
    /// None when the table has no `w:tblLook`.
    pub fn parse(tblpr: &Element) -> Option<TableLook> {
        let el = tblpr.child("w:tblLook")?;
        let hex = el.attr("w:val").and_then(|v| u32::from_str_radix(v.trim(), 16).ok()).unwrap_or(0);
        let pick = |name: &str, bit: u32| el.attr(name).map(flag).unwrap_or(hex & bit != 0);
        Some(TableLook {
            first_row: pick("w:firstRow", LOOK_FIRST_ROW),
            last_row: pick("w:lastRow", LOOK_LAST_ROW),
            first_col: pick("w:firstColumn", LOOK_FIRST_COL),
            last_col: pick("w:lastColumn", LOOK_LAST_COL),
            h_band: !pick("w:noHBand", LOOK_NO_H_BAND),
            v_band: !pick("w:noVBand", LOOK_NO_V_BAND),
        })
    }

    /// The look a `w:val` hex value stands for.
    pub fn from_bits(v: u32) -> TableLook {
        TableLook {
            first_row: v & LOOK_FIRST_ROW != 0,
            last_row: v & LOOK_LAST_ROW != 0,
            first_col: v & LOOK_FIRST_COL != 0,
            last_col: v & LOOK_LAST_COL != 0,
            h_band: v & LOOK_NO_H_BAND == 0,
            v_band: v & LOOK_NO_V_BAND == 0,
        }
    }

    /// The hex value of `w:val`.
    pub fn bits(&self) -> u32 {
        let mut v = 0;
        for (on, bit) in [
            (self.first_row, LOOK_FIRST_ROW),
            (self.last_row, LOOK_LAST_ROW),
            (self.first_col, LOOK_FIRST_COL),
            (self.last_col, LOOK_LAST_COL),
            (!self.h_band, LOOK_NO_H_BAND),
            (!self.v_band, LOOK_NO_V_BAND),
        ] {
            if on {
                v |= bit;
            }
        }
        v
    }

    /// `w:tblLook` as Word writes it: the hex value and the six attributes.
    pub fn element(&self) -> Element {
        let b = |on: bool| if on { "1" } else { "0" };
        Element::new("w:tblLook")
            .with_attr("w:val", format!("{:04X}", self.bits()))
            .with_attr("w:firstRow", b(self.first_row))
            .with_attr("w:lastRow", b(self.last_row))
            .with_attr("w:firstColumn", b(self.first_col))
            .with_attr("w:lastColumn", b(self.last_col))
            .with_attr("w:noHBand", b(!self.h_band))
            .with_attr("w:noVBand", b(!self.v_band))
    }
}

/// The parts of a table style, in the order they are applied (later ones win).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cond {
    Band1Vert,
    Band2Vert,
    Band1Horz,
    Band2Horz,
    FirstCol,
    LastCol,
    FirstRow,
    LastRow,
    NeCell,
    NwCell,
    SeCell,
    SwCell,
}

const CONDS: [Cond; 12] = [
    Cond::Band1Vert,
    Cond::Band2Vert,
    Cond::Band1Horz,
    Cond::Band2Horz,
    Cond::FirstCol,
    Cond::LastCol,
    Cond::FirstRow,
    Cond::LastRow,
    Cond::NeCell,
    Cond::NwCell,
    Cond::SeCell,
    Cond::SwCell,
];

impl Cond {
    pub fn parse(name: &str) -> Option<Cond> {
        Some(match name {
            "band1Vert" => Cond::Band1Vert,
            "band2Vert" => Cond::Band2Vert,
            "band1Horz" => Cond::Band1Horz,
            "band2Horz" => Cond::Band2Horz,
            "firstCol" => Cond::FirstCol,
            "lastCol" => Cond::LastCol,
            "firstRow" => Cond::FirstRow,
            "lastRow" => Cond::LastRow,
            "neCell" => Cond::NeCell,
            "nwCell" => Cond::NwCell,
            "seCell" => Cond::SeCell,
            "swCell" => Cond::SwCell,
            _ => return None,
        })
    }

    fn index(self) -> usize {
        CONDS.iter().position(|c| *c == self).unwrap()
    }
}

/// Where a cell sits: its row among all rows and its place among the cells of its row.
#[derive(Debug, Clone, Copy)]
pub struct CellPos {
    pub row: usize,
    pub rows: usize,
    pub col: usize,
    pub cols: usize,
    /// The first grid column the cell covers and the one after the last (for the table's outer edges).
    pub grid_from: usize,
    pub grid_to: usize,
    pub grid_cols: usize,
}

/// Which conditions apply to a cell.
pub fn conds_for(look: &TableLook, row_band: usize, col_band: usize, p: &CellPos) -> Vec<Cond> {
    let first_row = look.first_row && p.row == 0;
    let last_row = look.last_row && p.row + 1 == p.rows && p.rows > 1;
    let first_col = look.first_col && p.col == 0;
    let last_col = look.last_col && p.col + 1 == p.cols && p.cols > 1;
    let mut out = Vec::new();
    // Banding skips the header and total rows and the first and last column.
    if look.v_band && !first_col && !last_col {
        let k = p.col - usize::from(look.first_col);
        out.push(if (k / col_band.max(1)).is_multiple_of(2) { Cond::Band1Vert } else { Cond::Band2Vert });
    }
    if look.h_band && !first_row && !last_row {
        let k = p.row - usize::from(look.first_row);
        out.push(if (k / row_band.max(1)).is_multiple_of(2) { Cond::Band1Horz } else { Cond::Band2Horz });
    }
    if first_col {
        out.push(Cond::FirstCol);
    }
    if last_col {
        out.push(Cond::LastCol);
    }
    if first_row {
        out.push(Cond::FirstRow);
    }
    if last_row {
        out.push(Cond::LastRow);
    }
    if first_row && first_col {
        out.push(Cond::NwCell);
    }
    if first_row && last_col {
        out.push(Cond::NeCell);
    }
    if last_row && first_col {
        out.push(Cond::SwCell);
    }
    if last_row && last_col {
        out.push(Cond::SeCell);
    }
    out
}

/// The conditions in a `w:cnfStyle` (attribute form or the 12-digit binary `w:val`).
pub fn conds_from_cnf(cnf: &Element) -> Vec<Cond> {
    let bits: Vec<char> = cnf.attr("w:val").map(|v| v.chars().collect()).unwrap_or_default();
    let on = |name: &str, i: usize| cnf.attr(name).map(flag).unwrap_or_else(|| bits.get(i) == Some(&'1'));
    let mut out = Vec::new();
    if on("w:oddVBand", 4) {
        out.push(Cond::Band1Vert);
    }
    if on("w:evenVBand", 5) {
        out.push(Cond::Band2Vert);
    }
    if on("w:oddHBand", 6) {
        out.push(Cond::Band1Horz);
    }
    if on("w:evenHBand", 7) {
        out.push(Cond::Band2Horz);
    }
    if on("w:firstColumn", 2) {
        out.push(Cond::FirstCol);
    }
    if on("w:lastColumn", 3) {
        out.push(Cond::LastCol);
    }
    if on("w:firstRow", 0) {
        out.push(Cond::FirstRow);
    }
    if on("w:lastRow", 1) {
        out.push(Cond::LastRow);
    }
    if on("w:firstRowLastColumn", 9) {
        out.push(Cond::NeCell);
    }
    if on("w:firstRowFirstColumn", 8) {
        out.push(Cond::NwCell);
    }
    if on("w:lastRowLastColumn", 11) {
        out.push(Cond::SeCell);
    }
    if on("w:lastRowFirstColumn", 10) {
        out.push(Cond::SwCell);
    }
    out
}

/// A border that is switched off (`nil` or `none`), as opposed to one that is not mentioned.
pub fn nil_border() -> Border {
    Border { val: "nil".into(), sz: 0, color: "auto".into(), space: 0 }
}

pub fn is_nil(b: &Border) -> bool {
    b.val == "nil" || b.val == "none"
}

fn edge(e: &Element) -> Option<Border> {
    match e.attr("w:val") {
        Some("nil") | Some("none") => Some(nil_border()),
        _ => parse_border(e),
    }
}

/// Borders by side. `None` is "not mentioned" and a `nil` border is "no line".
#[derive(Debug, Clone, Default)]
pub struct Sides {
    pub top: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
    pub right: Option<Border>,
    pub inside_h: Option<Border>,
    pub inside_v: Option<Border>,
    /// Diagonals: top left to bottom right, top right to bottom left.
    pub tl2br: Option<Border>,
    pub tr2bl: Option<Border>,
}

impl Sides {
    /// Read `w:tblBorders` or `w:tcBorders`.
    pub fn from_el(el: &Element) -> Sides {
        let get = |names: &[&str]| names.iter().find_map(|n| el.child(n).and_then(edge));
        Sides {
            top: get(&["w:top"]),
            bottom: get(&["w:bottom"]),
            left: get(&["w:left", "w:start"]),
            right: get(&["w:right", "w:end"]),
            inside_h: get(&["w:insideH"]),
            inside_v: get(&["w:insideV"]),
            tl2br: get(&["w:tl2br"]),
            tr2bl: get(&["w:tr2bl"]),
        }
    }

    /// Put what `top` mentions over what `self` has.
    pub fn overlay(&mut self, top: &Sides) {
        macro_rules! lay {
            ($($f:ident),*) => { $(if top.$f.is_some() { self.$f = top.$f.clone(); })* };
        }
        lay!(top, bottom, left, right, inside_h, inside_v, tl2br, tr2bl);
    }
}

/// A fill in a `w:shd`: `Some(Some(hex))` a colour, `Some(None)` explicitly none, `None` not mentioned.
fn shd_fill(el: &Element) -> Option<Option<String>> {
    let s = el.child("w:shd")?;
    match s.attr("w:val") {
        Some("nil") | Some("none") => return Some(None),
        _ => {}
    }
    // A solid pattern is drawn in its colour; other patterns are approximated by the fill behind them.
    let pick = if s.attr("w:val") == Some("solid") { s.attr("w:color") } else { s.attr("w:fill") };
    match pick {
        Some(f) if f.len() == 6 && f.bytes().all(|b| b.is_ascii_hexdigit()) => Some(Some(f.to_uppercase())),
        Some(_) => Some(None),
        None => None,
    }
}

fn margins(m: &Element) -> [Option<i64>; 4] {
    let w = |names: &[&str]| names.iter().find_map(|n| m.child(n)).and_then(|e| e.attr("w:w")).and_then(|v| v.parse().ok());
    [w(&["w:top"]), w(&["w:right", "w:end"]), w(&["w:bottom"]), w(&["w:left", "w:start"])]
}

/// Everything one part of a style (or the style as a whole) says about a cell.
#[derive(Debug, Clone, Default)]
struct Part {
    para: ParaFmt,
    run: RunFmt,
    /// `w:tblBorders` of the part's `w:tblPr`: the part's own outline and inside lines.
    tbl_borders: Option<Sides>,
    /// `w:tcBorders` of the part's `w:tcPr`.
    tc_borders: Option<Sides>,
    fill: Option<Option<String>>,
    mar: [Option<i64>; 4],
    valign: Option<String>,
}

impl Part {
    fn layer(&mut self, o: &Part) {
        self.para = self.para.overlay(&o.para);
        self.run = self.run.overlay(&o.run);
        for (mine, theirs) in [(&mut self.tbl_borders, &o.tbl_borders), (&mut self.tc_borders, &o.tc_borders)] {
            if let Some(t) = theirs {
                mine.get_or_insert_with(Sides::default).overlay(t);
            }
        }
        if o.fill.is_some() {
            self.fill = o.fill.clone();
        }
        for i in 0..4 {
            if o.mar[i].is_some() {
                self.mar[i] = o.mar[i];
            }
        }
        if o.valign.is_some() {
            self.valign = o.valign.clone();
        }
    }

    /// Read a `w:tblStylePr` or the `w:style` itself: `w:pPr`, `w:rPr`, `w:tblPr` and `w:tcPr`.
    fn read(el: &Element) -> Part {
        let mut p = Part::default();
        if let Some(pp) = el.child("w:pPr") {
            p.para = ParaFmt::from_ppr(pp);
        }
        if let Some(rp) = el.child("w:rPr") {
            p.run = RunFmt::from_rpr(rp);
        }
        if let Some(tp) = el.child("w:tblPr") {
            p.tbl_borders = tp.child("w:tblBorders").map(Sides::from_el);
            if let Some(f) = shd_fill(tp) {
                p.fill = Some(f);
            }
            if let Some(m) = tp.child("w:tblCellMar") {
                p.mar = margins(m);
            }
        }
        if let Some(cp) = el.child("w:tcPr") {
            p.tc_borders = cp.child("w:tcBorders").map(Sides::from_el);
            if let Some(f) = shd_fill(cp) {
                p.fill = Some(f);
            }
            if let Some(m) = cp.child("w:tcMar") {
                for (i, v) in margins(m).into_iter().enumerate() {
                    if v.is_some() {
                        p.mar[i] = v;
                    }
                }
            }
            if let Some(v) = cp.child("w:vAlign").and_then(|v| v.attr("w:val")) {
                p.valign = Some(v.to_string());
            }
        }
        p
    }
}

/// What a table style and the table's own formatting make of one cell.
#[derive(Debug, Clone, Default)]
pub struct CellStyle {
    /// Fill colour as hex, or None for no fill.
    pub shading: Option<String>,
    /// The cell's borders, each side settled: a `nil` border is "no line".
    pub borders: Sides,
    /// Paragraph and run formatting for the text of the cell, from the table style.
    pub para: ParaFmt,
    pub run: RunFmt,
    /// Cell margins (top, right, bottom, left) where the style or the cell sets them.
    pub mar: [Option<i64>; 4],
    pub valign: Option<String>,
    pub conds: Vec<Cond>,
}

pub struct TableStyler {
    look: Option<TableLook>,
    row_band: usize,
    col_band: usize,
    whole: Part,
    conds: Vec<Option<Part>>,
    /// The table's own `w:tblBorders` and `w:shd`.
    direct_borders: Option<Sides>,
    direct_fill: Option<Option<String>>,
}

impl TableStyler {
    /// `style` is the table's style (if any) and `tblpr` its `w:tblPr`.
    pub fn new(styles: &StyleSheet, style: Option<&str>, tblpr: Option<&Element>) -> TableStyler {
        let look = tblpr.and_then(TableLook::parse);
        let mut row_band = 1;
        let mut col_band = 1;
        let mut whole = Part::default();
        let mut conds: Vec<Option<Part>> = vec![None; CONDS.len()];
        if let Some(id) = style {
            for p in styles.table_style_props(id) {
                if let Some(n) = p.child("w:tblStyleRowBandSize").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok()) {
                    row_band = n;
                }
                if let Some(n) = p.child("w:tblStyleColBandSize").and_then(|e| e.attr("w:val")).and_then(|v| v.parse().ok()) {
                    col_band = n;
                }
            }
            // The whole-table part: `w:pPr`/`w:rPr` and `w:tblPr` along the chain, and the cell properties.
            let (pf, rf) = styles.table_style_text(id);
            whole.para = pf;
            whole.run = rf;
            for p in styles.table_style_props(id) {
                if let Some(b) = p.child("w:tblBorders") {
                    whole.tbl_borders.get_or_insert_with(Sides::default).overlay(&Sides::from_el(b));
                }
                if let Some(f) = shd_fill(p) {
                    whole.fill = Some(f);
                }
                if let Some(m) = p.child("w:tblCellMar") {
                    for (i, v) in margins(m).into_iter().enumerate() {
                        if v.is_some() {
                            whole.mar[i] = v;
                        }
                    }
                }
            }
            for c in styles.table_style_cell_props(id) {
                let mut holder = Element::new("w:style");
                holder.children.push(crate::xml::Node::Element(c.clone()));
                whole.layer(&Part::read(&holder));
            }
            for el in styles.table_style_conds(id) {
                let Some(kind) = el.attr("w:type").and_then(Cond::parse) else { continue };
                let part = Part::read(el);
                match &mut conds[kind.index()] {
                    Some(have) => have.layer(&part),
                    slot => *slot = Some(part),
                }
            }
        }
        let mut direct_borders = None;
        let mut direct_fill = None;
        if let Some(tp) = tblpr {
            direct_borders = tp.child("w:tblBorders").map(Sides::from_el);
            direct_fill = shd_fill(tp);
        }
        TableStyler { look, row_band, col_band, whole, conds, direct_borders, direct_fill }
    }

    /// The table's own look, if it has a `w:tblLook`.
    pub fn look(&self) -> Option<TableLook> {
        self.look
    }

    /// The borders of the table as a whole (style, then direct), for its outer edge.
    pub fn table_borders(&self) -> Sides {
        let mut s = self.whole.tbl_borders.clone().unwrap_or_default();
        if let Some(d) = &self.direct_borders {
            s.overlay(d);
        }
        s
    }

    fn applying(&self, pos: &CellPos, cnf: Option<&Element>) -> Vec<Cond> {
        match (&self.look, cnf) {
            (Some(look), _) => conds_for(look, self.row_band, self.col_band, pos),
            // A table with no `w:tblLook` shows what its rows and cells say they are.
            (None, Some(c)) => {
                let mut v = conds_from_cnf(c);
                v.sort_by_key(|c| c.index());
                v
            }
            (None, None) => Vec::new(),
        }
    }

    /// The region of the table a condition covers, as (first row, last row, first cell, last cell).
    fn region(&self, c: Cond, pos: &CellPos) -> (usize, usize, usize, usize) {
        let look = self.look.unwrap_or_default();
        let last_row = pos.rows.saturating_sub(1);
        let last_col = pos.cols.saturating_sub(1);
        match c {
            Cond::FirstRow => (0, 0, 0, last_col),
            Cond::LastRow => (last_row, last_row, 0, last_col),
            Cond::FirstCol => (0, last_row, 0, 0),
            Cond::LastCol => (0, last_row, last_col, last_col),
            Cond::NwCell => (0, 0, 0, 0),
            Cond::NeCell => (0, 0, last_col, last_col),
            Cond::SwCell => (last_row, last_row, 0, 0),
            Cond::SeCell => (last_row, last_row, last_col, last_col),
            Cond::Band1Horz | Cond::Band2Horz => {
                let lead = usize::from(look.first_row);
                let size = self.row_band.max(1);
                let k = pos.row.saturating_sub(lead);
                let start = lead + (k / size) * size;
                let tail = usize::from(look.last_row);
                let end = (start + size - 1).min(last_row.saturating_sub(tail).max(start));
                (start, end, 0, last_col)
            }
            Cond::Band1Vert | Cond::Band2Vert => {
                let lead = usize::from(look.first_col);
                let size = self.col_band.max(1);
                let k = pos.col.saturating_sub(lead);
                let start = lead + (k / size) * size;
                let tail = usize::from(look.last_col);
                let end = (start + size - 1).min(last_col.saturating_sub(tail).max(start));
                (0, last_row, start, end)
            }
        }
    }

    /// Put the `w:tblBorders` of a region over the cell: the region's outer lines on its edges, its inside lines
    /// between its cells.
    fn region_sides(&self, b: &Sides, c: Cond, pos: &CellPos) -> Sides {
        let (r0, r1, c0, c1) = self.region(c, pos);
        Sides {
            top: if pos.row == r0 { b.top.clone() } else { b.inside_h.clone() },
            bottom: if pos.row == r1 { b.bottom.clone() } else { b.inside_h.clone() },
            left: if pos.col == c0 { b.left.clone() } else { b.inside_v.clone() },
            right: if pos.col == c1 { b.right.clone() } else { b.inside_v.clone() },
            ..Default::default()
        }
    }

    /// The table-level borders as they fall on one cell: the outer lines at the table's edge, the inside lines
    /// elsewhere.
    fn edge_sides(b: &Sides, pos: &CellPos) -> Sides {
        Sides {
            top: if pos.row == 0 { b.top.clone() } else { b.inside_h.clone() },
            bottom: if pos.row + 1 >= pos.rows { b.bottom.clone() } else { b.inside_h.clone() },
            left: if pos.grid_from == 0 { b.left.clone() } else { b.inside_v.clone() },
            right: if pos.grid_to >= pos.grid_cols { b.right.clone() } else { b.inside_v.clone() },
            ..Default::default()
        }
    }

    /// The style of the cell at `pos`. `tcpr` is the cell's own `w:tcPr`; `cnf` its row's or its own `w:cnfStyle`.
    pub fn cell(&self, pos: &CellPos, tcpr: Option<&Element>, cnf: Option<&Element>) -> CellStyle {
        let applying = self.applying(pos, cnf);
        let mut sum = self.whole.clone();
        let mut borders = Self::edge_sides(&self.whole.tbl_borders.clone().unwrap_or_default(), pos);
        if let Some(tc) = &self.whole.tc_borders {
            borders.overlay(tc);
        }
        for c in &applying {
            let Some(part) = &self.conds[c.index()] else { continue };
            let mut with_borders = part.clone();
            // The part's own text, fill and margins layer over the whole table's; its borders are placed below.
            with_borders.tbl_borders = None;
            with_borders.tc_borders = None;
            sum.layer(&with_borders);
            if let Some(tb) = &part.tbl_borders {
                borders.overlay(&self.region_sides(tb, *c, pos));
            }
            if let Some(tc) = &part.tc_borders {
                borders.overlay(tc);
            }
        }
        // Direct formatting: the table's, then the cell's.
        if let Some(d) = &self.direct_borders {
            borders.overlay(&Self::edge_sides(d, pos));
        }
        let mut fill = sum.fill.clone();
        if let Some(f) = &self.direct_fill {
            fill = Some(f.clone());
        }
        let mut mar = sum.mar;
        let mut valign = sum.valign.clone();
        if let Some(tc) = tcpr {
            if let Some(b) = tc.child("w:tcBorders") {
                borders.overlay(&Sides::from_el(b));
            }
            if let Some(f) = shd_fill(tc) {
                fill = Some(f);
            }
            if let Some(m) = tc.child("w:tcMar") {
                for (i, v) in margins(m).into_iter().enumerate() {
                    if v.is_some() {
                        mar[i] = v;
                    }
                }
            }
            if let Some(v) = tc.child("w:vAlign").and_then(|v| v.attr("w:val")) {
                valign = Some(v.to_string());
            }
        }
        // Inside lines do not belong to a cell.
        borders.inside_h = None;
        borders.inside_v = None;
        CellStyle { shading: fill.flatten(), borders, para: sum.para, run: sum.run, mar, valign, conds: applying }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::ThemeFonts;
    use crate::xml;

    fn look_el(src: &str) -> Element {
        xml::parse(format!(r#"<w:tblPr xmlns:w="w">{src}</w:tblPr>"#).as_bytes()).unwrap().root
    }

    #[test]
    fn look_reads_both_forms() {
        let l = TableLook::parse(&look_el(r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#)).unwrap();
        assert_eq!(l, TableLook::default());
        // The old form has only the hex value.
        let old = TableLook::parse(&look_el(r#"<w:tblLook w:val="0660"/>"#)).unwrap();
        assert!(old.first_row && old.last_row && !old.first_col && !old.last_col);
        assert!(!old.h_band && !old.v_band);
        assert!(TableLook::parse(&look_el("")).is_none());
        // Attributes win over the hex value.
        let both = TableLook::parse(&look_el(r#"<w:tblLook w:val="0000" w:firstRow="1"/>"#)).unwrap();
        assert!(both.first_row && both.h_band);
        let el = TableLook { first_row: true, last_row: true, first_col: false, last_col: true, h_band: true, v_band: true }.element();
        assert_eq!(el.attr("w:val"), Some("0160"));
        assert!(TableLook::parse(&look_el(r#"<w:tblLook w:val="0160"/>"#)).unwrap().last_col);
    }

    fn pos(row: usize, rows: usize, col: usize, cols: usize) -> CellPos {
        CellPos { row, rows, col, cols, grid_from: col, grid_to: col + 1, grid_cols: cols }
    }

    #[test]
    fn conditions_follow_the_look_and_bands_skip_the_edges() {
        let look = TableLook { first_row: true, last_row: true, first_col: true, last_col: false, h_band: true, v_band: false };
        assert_eq!(conds_for(&look, 1, 1, &pos(0, 5, 1, 3)), vec![Cond::FirstRow]);
        assert_eq!(conds_for(&look, 1, 1, &pos(0, 5, 0, 3)), vec![Cond::FirstCol, Cond::FirstRow, Cond::NwCell]);
        // The first row after the header is band 1, the next is band 2.
        assert_eq!(conds_for(&look, 1, 1, &pos(1, 5, 1, 3)), vec![Cond::Band1Horz]);
        assert_eq!(conds_for(&look, 1, 1, &pos(2, 5, 1, 3)), vec![Cond::Band2Horz]);
        assert_eq!(conds_for(&look, 1, 1, &pos(4, 5, 1, 3)), vec![Cond::LastRow]);
        // Bands of two rows.
        assert_eq!(conds_for(&look, 2, 1, &pos(2, 7, 1, 3)), vec![Cond::Band1Horz]);
        assert_eq!(conds_for(&look, 2, 1, &pos(3, 7, 1, 3)), vec![Cond::Band2Horz]);
        // No header: banding starts at the first row.
        let plain = TableLook { first_row: false, last_row: false, first_col: false, last_col: false, h_band: true, v_band: true };
        assert_eq!(conds_for(&plain, 1, 1, &pos(0, 3, 0, 3)), vec![Cond::Band1Vert, Cond::Band1Horz]);
        assert_eq!(conds_for(&plain, 1, 1, &pos(1, 3, 1, 3)), vec![Cond::Band2Vert, Cond::Band2Horz]);
    }

    #[test]
    fn cnf_reads_attributes_and_the_binary_value() {
        let a = xml::parse(br#"<w:cnfStyle xmlns:w="w" w:val="100000000000" w:firstRow="1"/>"#).unwrap().root;
        assert_eq!(conds_from_cnf(&a), vec![Cond::FirstRow]);
        let b = xml::parse(br#"<w:cnfStyle xmlns:w="w" w:val="001000100000"/>"#).unwrap().root;
        assert_eq!(conds_from_cnf(&b), vec![Cond::Band1Horz, Cond::FirstCol]);
    }

    const STYLES: &str = r#"<w:styles xmlns:w="w">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:tblPr><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
<w:style w:type="table" w:styleId="Demo"><w:name w:val="Demo"/><w:basedOn w:val="TableNormal"/>
<w:tblPr><w:tblStyleRowBandSize w:val="1"/><w:tblBorders><w:top w:val="single" w:sz="4" w:color="AAAAAA"/><w:insideH w:val="single" w:sz="4" w:color="BBBBBB"/></w:tblBorders></w:tblPr>
<w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="EEEEEE"/></w:tcPr>
<w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:color w:val="FFFFFF"/></w:rPr><w:tcPr><w:tcBorders><w:bottom w:val="single" w:sz="12" w:color="000000"/></w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="112233"/></w:tcPr></w:tblStylePr>
<w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="DDDDDD"/></w:tcPr></w:tblStylePr>
<w:tblStylePr w:type="firstCol"><w:rPr><w:b/></w:rPr></w:tblStylePr>
<w:tblStylePr w:type="nwCell"><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="FF0000"/></w:tcPr></w:tblStylePr>
</w:style></w:styles>"#;

    fn styler(tblpr: &str) -> TableStyler {
        let ss = StyleSheet::parse(Some(xml::parse(STYLES.as_bytes()).unwrap()), ThemeFonts::default());
        TableStyler::new(&ss, Some("Demo"), Some(&look_el(tblpr)))
    }

    #[test]
    fn a_cell_takes_the_layers_in_word_order() {
        let s = styler(r#"<w:tblStyle w:val="Demo"/><w:tblLook w:val="04A0"/>"#);
        // Header cell: the first row's fill and bold white text, over the whole table's.
        let head = s.cell(&pos(0, 4, 1, 3), None, None);
        assert_eq!(head.shading.as_deref(), Some("112233"));
        assert_eq!(head.run.bold, Some(true));
        assert_eq!(head.run.color.as_deref(), Some("FFFFFF"));
        assert_eq!(head.borders.bottom.as_ref().map(|b| b.sz), Some(12));
        // The corner cell wins over the header row and the first column.
        let corner = s.cell(&pos(0, 4, 0, 3), None, None);
        assert_eq!(corner.shading.as_deref(), Some("FF0000"));
        // Banded rows: the first body row is band 1 and carries its fill, the next has the whole table's.
        assert_eq!(s.cell(&pos(1, 4, 1, 3), None, None).shading.as_deref(), Some("DDDDDD"));
        assert_eq!(s.cell(&pos(2, 4, 1, 3), None, None).shading.as_deref(), Some("EEEEEE"));
        // First column is bold; other body cells are not.
        assert_eq!(s.cell(&pos(2, 4, 0, 3), None, None).run.bold, Some(true));
        assert_eq!(s.cell(&pos(2, 4, 1, 3), None, None).run.bold, None);
        // The table's outline and inside lines come from the whole table; the header's own bottom line wins.
        let body = s.cell(&pos(2, 4, 1, 3), None, None);
        assert_eq!(body.borders.top.as_ref().map(|b| b.color.as_str()), Some("BBBBBB"));
        assert_eq!(s.cell(&pos(0, 4, 1, 3), None, None).borders.top.as_ref().map(|b| b.color.as_str()), Some("AAAAAA"));
        // Direct formatting is last.
        let own = xml::parse(br#"<w:tcPr xmlns:w="w"><w:tcBorders><w:bottom w:val="nil"/></w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="00FF00"/></w:tcPr>"#).unwrap().root;
        let c = s.cell(&pos(0, 4, 1, 3), Some(&own), None);
        assert_eq!(c.shading.as_deref(), Some("00FF00"));
        assert!(is_nil(c.borders.bottom.as_ref().unwrap()));
    }

    #[test]
    fn the_look_switches_parts_off() {
        let s = styler(r#"<w:tblStyle w:val="Demo"/><w:tblLook w:val="0000" w:noHBand="1" w:noVBand="1"/>"#);
        let c = s.cell(&pos(0, 4, 0, 3), None, None);
        assert_eq!(c.shading.as_deref(), Some("EEEEEE"));
        assert_eq!(c.run.bold, None);
        assert!(c.conds.is_empty());
    }

    #[test]
    fn a_table_without_a_look_follows_cnf_style() {
        let ss = StyleSheet::parse(Some(xml::parse(STYLES.as_bytes()).unwrap()), ThemeFonts::default());
        let s = TableStyler::new(&ss, Some("Demo"), Some(&look_el(r#"<w:tblStyle w:val="Demo"/>"#)));
        let cnf = xml::parse(br#"<w:cnfStyle xmlns:w="w" w:firstRow="1"/>"#).unwrap().root;
        assert_eq!(s.cell(&pos(0, 4, 1, 3), None, Some(&cnf)).shading.as_deref(), Some("112233"));
        assert_eq!(s.cell(&pos(0, 4, 1, 3), None, None).shading.as_deref(), Some("EEEEEE"));
    }
}
