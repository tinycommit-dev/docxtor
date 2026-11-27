//! Table, row and cell properties as Word's Table Properties dialog sets them: size, alignment, indent, text
//! wrapping, autofit, default cell margins, row heights, rows that may not break, header rows, preferred column
//! and cell widths, vertical alignment, text direction, alt text, and borders per edge.
//!
//! Each `*Format` is a patch: a field left out stays as it is. They are the payload of `table_format`,
//! `table_rows` and the cell actions `format` and `borders`.

use crate::model::*;
use crate::props::{put_ordered, Border};
use crate::table::{set_tcpr, span, with_tblpr, TBLPR_ORDER};
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

const TRPR_ORDER: &[&str] = &[
    "w:cnfStyle", "w:divId", "w:gridBefore", "w:gridAfter", "w:wBefore", "w:wAfter", "w:cantSplit", "w:trHeight", "w:tblHeader",
    "w:tblCellSpacing", "w:jc", "w:hidden", "w:ins", "w:del", "w:trPrChange",
];
const TBLBORDERS_ORDER: &[&str] = &["w:top", "w:left", "w:start", "w:bottom", "w:right", "w:end", "w:insideH", "w:insideV"];
const TCBORDERS_ORDER: &[&str] = &["w:top", "w:left", "w:start", "w:bottom", "w:right", "w:end", "w:insideH", "w:insideV", "w:tl2br", "w:tr2bl"];
const CELLMAR_ORDER: &[&str] = &["w:top", "w:left", "w:start", "w:bottom", "w:right", "w:end"];

/// How a width is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidthKind {
    /// Whatever the content and the page need.
    #[default]
    Auto,
    /// Twips.
    Dxa,
    /// Fiftieths of a percent (5000 is the whole width).
    Pct,
}

/// A preferred width (`w:tblW`, `w:tcW`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Width {
    #[serde(rename = "type")]
    pub kind: WidthKind,
    #[serde(default)]
    pub value: i64,
}

impl Width {
    pub fn element(&self, name: &'static str) -> Element {
        match self.kind {
            WidthKind::Auto => Element::new(name).with_attr("w:w", "0").with_attr("w:type", "auto"),
            WidthKind::Dxa => Element::new(name).with_attr("w:w", self.value.max(0).to_string()).with_attr("w:type", "dxa"),
            WidthKind::Pct => Element::new(name).with_attr("w:w", self.value.max(0).to_string()).with_attr("w:type", "pct"),
        }
    }

    /// Read `w:tblW` or `w:tcW`; a missing one is auto. Percentages written as `50%` are read as fiftieths.
    pub fn read(el: Option<&Element>) -> Width {
        let Some(el) = el else { return Width::default() };
        let raw = el.attr("w:w").unwrap_or("0");
        match el.attr("w:type") {
            Some("dxa") => Width { kind: WidthKind::Dxa, value: raw.parse().unwrap_or(0) },
            Some("pct") => {
                let value = raw.strip_suffix('%').map(|p| (p.parse::<f64>().unwrap_or(0.0) * 50.0) as i64).unwrap_or_else(|| raw.parse().unwrap_or(0));
                Width { kind: WidthKind::Pct, value }
            }
            // `nil` and `auto` have no size.
            _ => Width::default(),
        }
    }
}

/// One edge of a table or cell. `none` (or `nil`) draws no line and hides one set further out (the table style's);
/// `clear` takes the setting off, so the table's own look shows again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub val: String,
    /// Eighths of a point.
    #[serde(default = "default_sz")]
    pub sz: i64,
    /// Hex, or `auto`.
    #[serde(default = "default_color")]
    pub color: String,
    /// Points.
    #[serde(default)]
    pub space: i64,
}

fn default_sz() -> i64 {
    4
}
fn default_color() -> String {
    "auto".into()
}

impl Edge {
    fn element(&self, name: &'static str) -> Option<Element> {
        match self.val.as_str() {
            "clear" => None,
            "none" | "nil" => Some(Element::new(name).with_attr("w:val", "nil")),
            val => {
                let color = self.color.trim_start_matches('#');
                let color = if color.eq_ignore_ascii_case("auto") || color.is_empty() { "auto".to_string() } else { color.to_uppercase() };
                Some(
                    Element::new(name)
                        .with_attr("w:val", val)
                        .with_attr("w:sz", self.sz.clamp(2, 96).to_string())
                        .with_attr("w:space", self.space.clamp(0, 31).to_string())
                        .with_attr("w:color", color),
                )
            }
        }
    }
}

/// Borders by edge. A field left out stays as it is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BorderEdges {
    pub top: Option<Edge>,
    pub bottom: Option<Edge>,
    pub left: Option<Edge>,
    pub right: Option<Edge>,
    /// Between rows (a table, or several selected cells).
    pub inside_h: Option<Edge>,
    /// Between columns.
    pub inside_v: Option<Edge>,
    /// Top left to bottom right (cells only).
    pub diag_down: Option<Edge>,
    /// Top right to bottom left (cells only).
    pub diag_up: Option<Edge>,
}

impl BorderEdges {
    pub fn is_empty(&self) -> bool {
        *self == BorderEdges::default()
    }
}

fn put_edge(parent: &mut Element, name: &'static str, edge: &Option<Edge>, order: &[&str]) {
    let Some(edge) = edge else { return };
    // `w:start` and `w:end` are the same edges as `w:left` and `w:right`; writing both would contradict.
    let alias = match name {
        "w:left" => Some("w:start"),
        "w:right" => Some("w:end"),
        _ => None,
    };
    parent.remove_children(name);
    if let Some(a) = alias {
        parent.remove_children(a);
    }
    if let Some(el) = edge.element(name) {
        put_ordered(parent, el, order);
    }
}

/// Write the edges into a `w:tblBorders` or `w:tcBorders` element.
fn write_edges(parent: &mut Element, e: &BorderEdges, order: &[&str]) {
    put_edge(parent, "w:top", &e.top, order);
    put_edge(parent, "w:left", &e.left, order);
    put_edge(parent, "w:bottom", &e.bottom, order);
    put_edge(parent, "w:right", &e.right, order);
    put_edge(parent, "w:insideH", &e.inside_h, order);
    put_edge(parent, "w:insideV", &e.inside_v, order);
    put_edge(parent, "w:tl2br", &e.diag_down, order);
    put_edge(parent, "w:tr2bl", &e.diag_up, order);
}

/// Table-wide properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TableFormat {
    pub width: Option<Width>,
    /// `left`, `center` or `right`.
    pub align: Option<String>,
    /// Twips from the margin.
    pub indent: Option<i64>,
    /// `none` or `around`: the text flows around a table that is `around`.
    pub wrap: Option<String>,
    /// Columns grow and shrink to their content (otherwise they keep the grid's widths).
    pub autofit: Option<bool>,
    /// Default cell margins in twips: top, right, bottom, left.
    pub cell_margins: Option<[i64; 4]>,
    /// Space between cells in twips; 0 removes it.
    pub spacing: Option<i64>,
    /// Alt text: a title and a description. An empty text removes it.
    pub title: Option<String>,
    pub description: Option<String>,
    pub borders: Option<BorderEdges>,
}

pub fn apply_table(t: &mut Table, f: &TableFormat) -> Result<(), String> {
    if let Some(a) = &f.align {
        if !matches!(a.as_str(), "left" | "center" | "right") {
            return Err(format!("alignment must be left, center or right, not \"{a}\""));
        }
    }
    if let Some(w) = &f.wrap {
        if !matches!(w.as_str(), "none" | "around") {
            return Err(format!("text wrapping must be none or around, not \"{w}\""));
        }
    }
    with_tblpr(t, |p| {
        if let Some(w) = f.width {
            put_ordered(p, w.element("w:tblW"), TBLPR_ORDER);
        }
        if let Some(fit) = f.autofit {
            p.remove_children("w:tblLayout");
            if !fit {
                put_ordered(p, Element::new("w:tblLayout").with_attr("w:type", "fixed"), TBLPR_ORDER);
            }
        }
        if let Some(a) = &f.align {
            p.remove_children("w:jc");
            if a != "left" {
                put_ordered(p, Element::new("w:jc").with_attr("w:val", a.as_str()), TBLPR_ORDER);
            }
        }
        if let Some(i) = f.indent {
            p.remove_children("w:tblInd");
            if i != 0 {
                put_ordered(p, Element::new("w:tblInd").with_attr("w:w", i.to_string()).with_attr("w:type", "dxa"), TBLPR_ORDER);
            }
        }
        if let Some(m) = f.cell_margins {
            let side = |n: &'static str, v: i64| Element::new(n).with_attr("w:w", v.max(0).to_string()).with_attr("w:type", "dxa");
            let mut mar = Element::new("w:tblCellMar");
            mar.children.push(Node::Element(side("w:top", m[0])));
            mar.children.push(Node::Element(side("w:left", m[3])));
            mar.children.push(Node::Element(side("w:bottom", m[2])));
            mar.children.push(Node::Element(side("w:right", m[1])));
            put_ordered(p, mar, TBLPR_ORDER);
        }
        if let Some(s) = f.spacing {
            p.remove_children("w:tblCellSpacing");
            if s > 0 {
                put_ordered(p, Element::new("w:tblCellSpacing").with_attr("w:w", s.to_string()).with_attr("w:type", "dxa"), TBLPR_ORDER);
            }
        }
        if let Some(title) = &f.title {
            p.remove_children("w:tblCaption");
            if !title.is_empty() {
                put_ordered(p, Element::new("w:tblCaption").with_attr("w:val", title.as_str()), TBLPR_ORDER);
            }
        }
        if let Some(d) = &f.description {
            p.remove_children("w:tblDescription");
            if !d.is_empty() {
                put_ordered(p, Element::new("w:tblDescription").with_attr("w:val", d.as_str()), TBLPR_ORDER);
            }
        }
        if let Some(b) = &f.borders {
            let mut el = p.child("w:tblBorders").cloned().unwrap_or_else(|| Element::new("w:tblBorders"));
            write_edges(&mut el, b, TBLBORDERS_ORDER);
            p.remove_children("w:tblBorders");
            if !el.children.is_empty() {
                put_ordered(p, el, TBLPR_ORDER);
            }
        }
        // Text around a table: the file places it from the text, a little below the line it is in, with Word's usual gap.
        match f.wrap.as_deref() {
            Some("around") if p.child("w:tblpPr").is_none() => {
                let spec = f.align.clone().or_else(|| p.child("w:jc").and_then(|j| j.attr("w:val")).map(String::from)).unwrap_or_else(|| "left".into());
                let el = Element::new("w:tblpPr")
                    .with_attr("w:leftFromText", "180")
                    .with_attr("w:rightFromText", "180")
                    .with_attr("w:vertAnchor", "text")
                    .with_attr("w:horzAnchor", "margin")
                    .with_attr("w:tblpXSpec", spec)
                    .with_attr("w:tblpY", "1");
                put_ordered(p, el, TBLPR_ORDER);
            }
            Some("none") => {
                p.remove_children("w:tblpPr");
                p.remove_children("w:tblOverlap");
            }
            _ => {}
        }
        // A table with text around it is placed by its anchor, so the alignment moves the anchor.
        if let (Some(a), Some(fp)) = (&f.align, p.child_mut("w:tblpPr")) {
            if fp.attr("w:tblpX").is_none() {
                fp.set_attr("w:horzAnchor", fp.attr("w:horzAnchor").unwrap_or("margin").to_string());
                fp.set_attr("w:tblpXSpec", a.as_str());
            }
        }
    });
    Ok(())
}

/// The row's `w:trPr`, made if the row has none.
fn trpr_mut(row: &mut Row) -> &mut Element {
    let idx = row.children.iter().position(|c| matches!(c, RowChild::Other(e) if e.name == "w:trPr"));
    let idx = idx.unwrap_or_else(|| {
        row.children.insert(0, RowChild::Other(Element::new("w:trPr")));
        0
    });
    match &mut row.children[idx] {
        RowChild::Other(e) => e,
        RowChild::Cell(_) | RowChild::Marker(..) => unreachable!(),
    }
}

pub fn trpr(row: &Row) -> Option<&Element> {
    row.children.iter().find_map(|c| match c {
        RowChild::Other(e) if e.name == "w:trPr" => Some(e),
        _ => None,
    })
}

/// A row's height in twips: `auto` (no setting), `atLeast` or `exact`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowHeight {
    #[serde(default)]
    pub value: i64,
    /// `auto`, `atLeast` or `exact`.
    pub rule: HeightRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeightRule {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "atLeast")]
    AtLeast,
    #[serde(rename = "exact")]
    Exact,
}

/// Row properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RowFormat {
    pub height: Option<RowHeight>,
    /// The row may not split across pages.
    pub cant_split: Option<bool>,
    /// Repeat the row at the top of every page. Header rows start at the top of the table.
    pub header: Option<bool>,
}

pub fn apply_row(row: &mut Row, f: &RowFormat) {
    let p = trpr_mut(row);
    if let Some(h) = f.height {
        p.remove_children("w:trHeight");
        if h.rule != HeightRule::Auto && h.value > 0 {
            let rule = if h.rule == HeightRule::Exact { "exact" } else { "atLeast" };
            put_ordered(p, Element::new("w:trHeight").with_attr("w:val", h.value.to_string()).with_attr("w:hRule", rule), TRPR_ORDER);
        }
    }
    if let Some(c) = f.cant_split {
        p.remove_children("w:cantSplit");
        if c {
            put_ordered(p, Element::new("w:cantSplit"), TRPR_ORDER);
        }
    }
    if let Some(on) = f.header {
        p.remove_children("w:tblHeader");
        if on {
            put_ordered(p, Element::new("w:tblHeader"), TRPR_ORDER);
        }
    }
    if p.children.is_empty() && p.attrs.is_empty() {
        row.children.retain(|c| !matches!(c, RowChild::Other(e) if e.name == "w:trPr" && e.children.is_empty()));
    }
}

/// Cell properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CellFormat {
    pub width: Option<Width>,
    /// `top`, `center` (or `middle`) or `bottom`.
    pub valign: Option<String>,
    /// The cell's own margins in twips: top, right, bottom, left.
    pub margins: Option<[i64; 4]>,
    /// Use the table's margins again.
    pub clear_margins: Option<bool>,
    /// Keep the text on one line.
    pub no_wrap: Option<bool>,
    /// Squeeze the text to the cell's width.
    pub fit_text: Option<bool>,
    /// `lrTb` (across), `tbRl` (down, lines from the right) or `btLr` (up).
    pub text_direction: Option<String>,
}

pub fn apply_cell(c: &mut Cell, f: &CellFormat) -> Result<(), String> {
    if let Some(d) = &f.text_direction {
        if !matches!(d.as_str(), "lrTb" | "tbRl" | "btLr" | "lrTbV" | "tbRlV" | "tbLrV") {
            return Err(format!("text direction must be lrTb, tbRl or btLr, not \"{d}\""));
        }
    }
    if let Some(v) = &f.valign {
        if !matches!(v.as_str(), "top" | "center" | "middle" | "bottom") {
            return Err(format!("vertical alignment must be top, center or bottom, not \"{v}\""));
        }
    }
    if let Some(w) = f.width {
        set_tcpr(c, Some(w.element("w:tcW")), "w:tcW");
    }
    if let Some(v) = &f.valign {
        let el = (v != "top").then(|| Element::new("w:vAlign").with_attr("w:val", if v == "middle" { "center" } else { v.as_str() }));
        set_tcpr(c, el, "w:vAlign");
    }
    if f.clear_margins == Some(true) {
        set_tcpr(c, None, "w:tcMar");
    }
    if let Some(m) = f.margins {
        let side = |n: &'static str, v: i64| Element::new(n).with_attr("w:w", v.max(0).to_string()).with_attr("w:type", "dxa");
        let mut mar = Element::new("w:tcMar");
        for (name, v) in [("w:top", m[0]), ("w:left", m[3]), ("w:bottom", m[2]), ("w:right", m[1])] {
            put_ordered(&mut mar, side(name, v), CELLMAR_ORDER);
        }
        set_tcpr(c, Some(mar), "w:tcMar");
    }
    if let Some(n) = f.no_wrap {
        set_tcpr(c, n.then(|| Element::new("w:noWrap")), "w:noWrap");
    }
    if let Some(n) = f.fit_text {
        set_tcpr(c, n.then(|| Element::new("w:tcFitText")), "w:tcFitText");
    }
    if let Some(d) = &f.text_direction {
        set_tcpr(c, (d != "lrTb").then(|| Element::new("w:textDirection").with_attr("w:val", d.as_str())), "w:textDirection");
    }
    Ok(())
}

/// Write border edges on a cell.
pub fn apply_cell_borders(c: &mut Cell, e: &BorderEdges) {
    let mut el = c.tcpr.as_ref().and_then(|p| p.child("w:tcBorders")).cloned().unwrap_or_else(|| Element::new("w:tcBorders"));
    write_edges(&mut el, e, TCBORDERS_ORDER);
    set_tcpr(c, (!el.children.is_empty()).then_some(el), "w:tcBorders");
}

/// What the dialog reads about a cell's own settings (not what the table's style adds).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellInfo {
    pub width: Width,
    pub valign: String,
    pub margins: Option<[i64; 4]>,
    pub no_wrap: bool,
    pub fit_text: bool,
    pub text_direction: String,
}

pub fn cell_info(c: &Cell) -> CellInfo {
    let p = c.tcpr.as_ref();
    let on = |n: &str| p.and_then(|p| p.child(n)).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")));
    let margins = p.and_then(|p| p.child("w:tcMar")).map(|m| {
        let w = |names: &[&str]| names.iter().find_map(|n| m.child(n)).and_then(|e| e.attr("w:w")).and_then(|v| v.parse().ok()).unwrap_or(0);
        [w(&["w:top"]), w(&["w:right", "w:end"]), w(&["w:bottom"]), w(&["w:left", "w:start"])]
    });
    CellInfo {
        width: Width::read(p.and_then(|p| p.child("w:tcW"))),
        valign: p.and_then(|p| p.child("w:vAlign")).and_then(|v| v.attr("w:val")).map(|v| if v == "both" { "top" } else { v }).unwrap_or("top").to_string(),
        margins,
        no_wrap: on("w:noWrap"),
        fit_text: on("w:tcFitText"),
        text_direction: p.and_then(|p| p.child("w:textDirection")).and_then(|v| v.attr("w:val")).unwrap_or("lrTb").to_string(),
    }
}

/// What the dialog reads about a row.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowInfo {
    pub height: i64,
    /// `auto`, `atLeast` or `exact`.
    pub rule: String,
    pub cant_split: bool,
    pub header: bool,
}

pub fn row_info(row: &Row) -> RowInfo {
    let p = trpr(row);
    let h = p.and_then(|p| p.child("w:trHeight"));
    let on = |n: &str| p.and_then(|p| p.child(n)).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")));
    RowInfo {
        height: h.and_then(|h| h.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(0),
        rule: h.map(|h| h.attr("w:hRule").unwrap_or("atLeast").to_string()).unwrap_or_else(|| "auto".into()),
        cant_split: on("w:cantSplit"),
        header: on("w:tblHeader"),
    }
}

/// What the dialog reads about the table's own settings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub width: Width,
    pub align: String,
    pub indent: i64,
    pub wrap: bool,
    pub autofit: bool,
    /// The default cell margins: top, right, bottom, left.
    pub cell_margins: [i64; 4],
    pub spacing: i64,
    pub title: String,
    pub description: String,
    pub style: Option<String>,
}

pub fn table_info(t: &Table, cell_margins: [i64; 4]) -> TableInfo {
    let p = t.tblpr();
    let get = |n: &str| p.and_then(|p| p.child(n));
    let val = |n: &str| get(n).and_then(|e| e.attr("w:val")).unwrap_or("").to_string();
    TableInfo {
        width: Width::read(get("w:tblW")),
        align: match val("w:jc").as_str() {
            "center" => "center",
            "right" | "end" => "right",
            _ => "left",
        }
        .into(),
        indent: get("w:tblInd").and_then(|e| e.attr("w:w")).and_then(|v| v.parse().ok()).unwrap_or(0),
        wrap: get("w:tblpPr").is_some(),
        autofit: get("w:tblLayout").and_then(|l| l.attr("w:type")) != Some("fixed"),
        cell_margins,
        spacing: get("w:tblCellSpacing").and_then(|e| e.attr("w:w")).and_then(|v| v.parse().ok()).unwrap_or(0),
        title: val("w:tblCaption"),
        description: val("w:tblDescription"),
        style: get("w:tblStyle").and_then(|e| e.attr("w:val")).map(String::from),
    }
}

/// A border as the dialog shows it: absent when the edge draws no line.
pub type Shown = Option<Border>;

/// Set the preferred width of a grid column: twips change the grid and the cells in it, a percentage or auto
/// sets the cells only.
pub fn set_column_width(t: &mut Table, col: usize, w: Width) -> Result<(), String> {
    let mut grid = t.grid();
    if col >= grid.len() {
        return Err("there is no such column".into());
    }
    if w.kind == WidthKind::Dxa {
        grid[col] = w.value.max(200);
        for c in t.children.iter_mut() {
            if let TableChild::Other(e) = c {
                if e.name == "w:tblGrid" {
                    e.children.retain(|n| !matches!(n, Node::Element(x) if x.name == "w:gridCol"));
                    for g in &grid {
                        e.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", g.to_string())));
                    }
                }
            }
        }
        crate::table::sync_widths(t);
        return Ok(());
    }
    for row in t.rows_mut() {
        let before = row.trpr_before();
        let mut g = before;
        for c in row.cells_mut() {
            let s = span(c);
            if g == col && s == 1 {
                set_tcpr(c, Some(w.element("w:tcW")), "w:tcW");
            }
            g += s;
        }
    }
    Ok(())
}

impl Row {
    /// Grid columns left empty before the first cell.
    pub fn trpr_before(&self) -> usize {
        trpr(self).and_then(|p| p.child("w:gridBefore")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn table(rows: usize, cols: usize) -> Table {
        let mut ids = IdGen(0);
        let mut src = String::from(r#"<w:tbl xmlns:w="w"><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid>"#);
        for _ in 0..cols {
            src.push_str(r#"<w:gridCol w:w="1000"/>"#);
        }
        src.push_str("</w:tblGrid>");
        for r in 0..rows {
            src.push_str("<w:tr>");
            for c in 0..cols {
                src.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"1000\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:t>{r}{c}</w:t></w:r></w:p></w:tc>"));
            }
            src.push_str("</w:tr>");
        }
        src.push_str("</w:tbl>");
        parse_table(&xml::parse(src.as_bytes()).unwrap().root, &mut ids)
    }

    #[test]
    fn table_properties_are_written_in_schema_order() {
        let mut t = table(2, 2);
        let f = TableFormat {
            width: Some(Width { kind: WidthKind::Pct, value: 5000 }),
            align: Some("center".into()),
            indent: Some(360),
            autofit: Some(false),
            cell_margins: Some([20, 120, 20, 100]),
            spacing: Some(30),
            title: Some("Sales".into()),
            description: Some("Sales by region".into()),
            ..Default::default()
        };
        apply_table(&mut t, &f).unwrap();
        let names: Vec<&str> = t.tblpr().unwrap().elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:tblW", "w:jc", "w:tblCellSpacing", "w:tblInd", "w:tblLayout", "w:tblCellMar", "w:tblCaption", "w:tblDescription"]);
        let info = table_info(&t, [20, 120, 20, 100]);
        assert_eq!(info.width, Width { kind: WidthKind::Pct, value: 5000 });
        assert_eq!((info.align.as_str(), info.indent, info.autofit, info.spacing), ("center", 360, false, 30));
        assert_eq!((info.title.as_str(), info.description.as_str()), ("Sales", "Sales by region"));
        // Autofit, left, no indent and no spacing take the settings off.
        let g = TableFormat { autofit: Some(true), align: Some("left".into()), indent: Some(0), spacing: Some(0), title: Some(String::new()), ..Default::default() };
        apply_table(&mut t, &g).unwrap();
        let names: Vec<&str> = t.tblpr().unwrap().elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:tblW", "w:tblCellMar", "w:tblDescription"]);
        assert!(apply_table(&mut t, &TableFormat { align: Some("middle".into()), ..Default::default() }).is_err());
    }

    #[test]
    fn text_wrapping_places_the_table_by_its_anchor() {
        let mut t = table(1, 1);
        apply_table(&mut t, &TableFormat { wrap: Some("around".into()), align: Some("right".into()), ..Default::default() }).unwrap();
        let f = t.tblpr().unwrap().child("w:tblpPr").unwrap();
        assert_eq!((f.attr("w:tblpXSpec"), f.attr("w:vertAnchor"), f.attr("w:leftFromText")), (Some("right"), Some("text"), Some("180")));
        apply_table(&mut t, &TableFormat { align: Some("center".into()), ..Default::default() }).unwrap();
        assert_eq!(t.tblpr().unwrap().child("w:tblpPr").unwrap().attr("w:tblpXSpec"), Some("center"));
        apply_table(&mut t, &TableFormat { wrap: Some("none".into()), ..Default::default() }).unwrap();
        assert!(t.tblpr().unwrap().child("w:tblpPr").is_none());
    }

    #[test]
    fn row_height_and_flags() {
        let mut t = table(2, 1);
        let row = t.rows_mut().next().unwrap();
        apply_row(row, &RowFormat { height: Some(RowHeight { value: 600, rule: HeightRule::Exact }), cant_split: Some(true), header: Some(true) });
        let i = row_info(row);
        assert_eq!((i.height, i.rule.as_str(), i.cant_split, i.header), (600, "exact", true, true));
        apply_row(row, &RowFormat { height: Some(RowHeight { value: 0, rule: HeightRule::Auto }), cant_split: Some(false), header: Some(false) });
        // Nothing left to say: the row has no properties.
        assert!(trpr(row).is_none());
        assert_eq!(row_info(row).rule, "auto");
    }

    #[test]
    fn cell_properties_and_borders() {
        let mut t = table(1, 2);
        let c = t.rows_mut().next().unwrap().cells_mut().next().unwrap();
        apply_cell(c, &CellFormat { valign: Some("middle".into()), margins: Some([10, 20, 30, 40]), no_wrap: Some(true), text_direction: Some("btLr".into()), width: Some(Width { kind: WidthKind::Dxa, value: 1500 }), ..Default::default() }).unwrap();
        let names: Vec<&str> = c.tcpr.as_ref().unwrap().elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:tcW", "w:noWrap", "w:tcMar", "w:textDirection", "w:vAlign"]);
        let i = cell_info(c);
        assert_eq!((i.valign.as_str(), i.margins, i.no_wrap, i.text_direction.as_str()), ("center", Some([10, 20, 30, 40]), true, "btLr"));
        apply_cell_borders(c, &BorderEdges { top: Some(Edge { val: "single".into(), sz: 12, color: "#ff0000".into(), space: 0 }), left: Some(Edge { val: "none".into(), sz: 4, color: "auto".into(), space: 0 }), ..Default::default() });
        let b = c.tcpr.as_ref().unwrap().child("w:tcBorders").unwrap();
        assert_eq!(b.elements().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["w:top", "w:left"]);
        assert_eq!(b.child("w:top").unwrap().attr("w:color"), Some("FF0000"));
        assert_eq!(b.child("w:left").unwrap().attr("w:val"), Some("nil"));
        // Clearing an edge takes the element out, and the last one takes the container out.
        let clear = |s: &str| Some(Edge { val: s.into(), sz: 4, color: "auto".into(), space: 0 });
        apply_cell_borders(c, &BorderEdges { top: clear("clear"), left: clear("clear"), ..Default::default() });
        assert!(c.tcpr.as_ref().unwrap().child("w:tcBorders").is_none());
        assert!(apply_cell(c, &CellFormat { text_direction: Some("sideways".into()), ..Default::default() }).is_err());
    }

    #[test]
    fn table_borders_replace_the_start_and_end_names() {
        let mut t = table(1, 1);
        with_tblpr(&mut t, |p| p.children.push(Node::Element(Element::new("w:tblBorders").with_child(Element::new("w:start").with_attr("w:val", "single")))));
        let e = Edge { val: "double".into(), sz: 6, color: "auto".into(), space: 0 };
        apply_table(&mut t, &TableFormat { borders: Some(BorderEdges { left: Some(e.clone()), inside_h: Some(e), ..Default::default() }), ..Default::default() }).unwrap();
        let b = t.tblpr().unwrap().child("w:tblBorders").unwrap();
        assert_eq!(b.elements().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["w:left", "w:insideH"]);
    }

    #[test]
    fn column_widths_in_twips_change_the_grid_and_in_percent_only_the_cells() {
        let mut t = table(2, 2);
        set_column_width(&mut t, 0, Width { kind: WidthKind::Dxa, value: 2400 }).unwrap();
        assert_eq!(t.grid(), [2400, 1000]);
        assert_eq!(cell_info(t.rows().next().unwrap().cells().next().unwrap()).width.value, 2400);
        set_column_width(&mut t, 1, Width { kind: WidthKind::Pct, value: 1500 }).unwrap();
        assert_eq!(t.grid(), [2400, 1000]);
        for r in t.rows() {
            assert_eq!(cell_info(r.cells().nth(1).unwrap()).width, Width { kind: WidthKind::Pct, value: 1500 });
        }
        assert!(set_column_width(&mut t, 5, Width::default()).is_err());
    }
}
