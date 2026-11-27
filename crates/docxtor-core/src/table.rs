//! Table structure editing on a grid model: merge/split cells, column widths,
//! cell shading and alignment, border presets and header rows.

use crate::model::*;
use crate::props::put_ordered;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

pub(crate) const TCPR_ORDER: &[&str] = &[
    "w:cnfStyle", "w:tcW", "w:gridSpan", "w:hMerge", "w:vMerge", "w:tcBorders", "w:shd", "w:noWrap", "w:tcMar", "w:textDirection",
    "w:tcFitText", "w:vAlign", "w:hideMark", "w:headers", "w:cellIns", "w:cellDel", "w:cellMerge", "w:tcPrChange",
];
pub(crate) const TBLPR_ORDER: &[&str] = &[
    "w:tblStyle", "w:tblpPr", "w:tblOverlap", "w:bidiVisual", "w:tblStyleRowBandSize", "w:tblStyleColBandSize", "w:tblW", "w:jc",
    "w:tblCellSpacing", "w:tblInd", "w:tblBorders", "w:shd", "w:tblLayout", "w:tblCellMar", "w:tblLook", "w:tblCaption",
    "w:tblDescription", "w:tblPrChange",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderPreset {
    /// All outer and inner borders.
    Grid,
    /// Outer box only.
    Outer,
    /// Horizontal lines between rows only.
    Horizontal,
    None,
}

/// One grid slot: which physical cell covers it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub row: usize,
    /// Index among the row's cells.
    pub cell: usize,
    /// Row where the vertical merge (if any) starts.
    pub origin_row: usize,
}

pub(crate) fn span(c: &Cell) -> usize {
    c.tcpr.as_ref().and_then(|p| p.child("w:gridSpan")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(1).max(1)
}

pub(crate) fn vmerge(c: &Cell) -> Option<&str> {
    c.tcpr.as_ref().and_then(|p| p.child("w:vMerge")).map(|v| v.attr("w:val").unwrap_or("continue"))
}

/// Grid of slots [row][grid column].
pub fn grid_map(t: &Table) -> Vec<Vec<Option<Slot>>> {
    let cols = t.grid().len().max(t.rows().map(|r| r.trpr_before() + r.cells().map(span).sum::<usize>()).max().unwrap_or(0));
    let mut out: Vec<Vec<Option<Slot>>> = Vec::new();
    for (ri, r) in t.rows().enumerate() {
        let mut row = vec![None; cols];
        // A row that starts short (`w:gridBefore`) begins further in.
        let mut g = r.trpr_before();
        for (ci, c) in r.cells().enumerate() {
            let s = span(c);
            let origin_row = if vmerge(c) == Some("continue") && ri > 0 {
                out[ri - 1].get(g).and_then(|x| x.as_ref()).map(|x| x.origin_row).unwrap_or(ri)
            } else {
                ri
            };
            for slot in row.iter_mut().take((g + s).min(cols)).skip(g) {
                *slot = Some(Slot { row: ri, cell: ci, origin_row });
            }
            g += s;
        }
        out.push(row);
    }
    out
}

/// Grid position (row, first column, span) of the cell containing paragraph `pid`.
pub fn cell_of(t: &Table, pid: NodeId) -> Option<(usize, usize, usize)> {
    for (ri, r) in t.rows().enumerate() {
        let mut g = r.trpr_before();
        for c in r.cells() {
            let s = span(c);
            if find_block(&c.blocks, pid).is_some() {
                return Some((ri, g, s));
            }
            g += s;
        }
    }
    None
}

/// Grow a rectangle until no merged cell crosses its edges.
pub fn normalize_rect(t: &Table, mut r0: usize, mut c0: usize, mut r1: usize, mut c1: usize) -> (usize, usize, usize, usize) {
    let map = grid_map(t);
    loop {
        let (or0, oc0, or1, oc1) = (r0, c0, r1, c1);
        // Grow from the rectangle as it was at the start of this pass; the loop repeats until stable.
        for r in or0..=or1.min(map.len().saturating_sub(1)) {
            for c in oc0..=oc1 {
                let Some(Some(s)) = map[r].get(c) else { continue };
                // horizontal extent of this cell
                let row = &map[s.row];
                let first = row.iter().position(|x| x.as_ref().is_some_and(|x| x.cell == s.cell)).unwrap_or(c);
                let last = row.iter().rposition(|x| x.as_ref().is_some_and(|x| x.cell == s.cell)).unwrap_or(c);
                c0 = c0.min(first);
                c1 = c1.max(last);
                // vertical extent of the merge this slot belongs to
                r0 = r0.min(s.origin_row);
                let mut end = r;
                while end + 1 < map.len() && map[end + 1].get(c).and_then(|x| x.as_ref()).is_some_and(|x| x.origin_row == s.origin_row && x.row != s.origin_row) {
                    end += 1;
                }
                r1 = r1.max(end);
            }
        }
        if (or0, oc0, or1, oc1) == (r0, c0, r1, c1) {
            return (r0, c0, r1, c1);
        }
    }
}

pub(crate) fn row_mut(t: &mut Table, ri: usize) -> &mut Row {
    t.rows_mut().nth(ri).expect("row index")
}

pub(crate) fn set_tcpr(cell: &mut Cell, el: Option<Element>, name: &str) {
    let tp = cell.tcpr.get_or_insert_with(|| Element::new("w:tcPr"));
    tp.remove_children(name);
    if let Some(e) = el {
        put_ordered(tp, e, TCPR_ORDER);
    }
}

pub(crate) fn empty_para(ids: &mut IdGen) -> Block {
    Block::Para(Paragraph::new(ids.fresh()))
}

fn has_text(b: &Block) -> bool {
    match b {
        Block::Para(p) => p.len16() > 0,
        _ => true,
    }
}

/// Recompute each cell's preferred width from the grid.
pub fn sync_widths(t: &mut Table) {
    let grid = t.grid();
    if grid.is_empty() {
        return;
    }
    for r in t.rows_mut() {
        let mut g = r.trpr_before();
        for c in r.cells_mut() {
            let s = span(c);
            let w: i64 = grid.iter().skip(g).take(s).sum();
            g += s;
            set_tcpr(c, Some(Element::new("w:tcW").with_attr("w:w", w.to_string()).with_attr("w:type", "dxa")), "w:tcW");
        }
    }
}

/// Merge the rectangle (grid coordinates, inclusive) into its top-left cell.
pub fn merge(t: &mut Table, rect: (usize, usize, usize, usize), ids: &mut IdGen) -> Result<NodeId, String> {
    let (r0, c0, r1, c1) = normalize_rect(t, rect.0, rect.1, rect.2, rect.3);
    if r0 == r1 && c0 == c1 {
        return Err("select more than one cell to merge".into());
    }
    let width = c1 - c0 + 1;
    // Collect content in reading order.
    let mut content: Vec<Block> = Vec::new();
    for ri in r0..=r1 {
        let row = t.rows().nth(ri).unwrap();
        let mut g = row.trpr_before();
        for c in row.cells() {
            let s = span(c);
            if g >= c0 && g + s - 1 <= c1 && vmerge(c) != Some("continue") {
                content.extend(c.blocks.iter().filter(|b| has_text(b)).cloned());
            }
            g += s;
        }
    }
    let mut first_para = None;
    for ri in r0..=r1 {
        let row = row_mut(t, ri);
        let mut g = row.trpr_before();
        let mut keep: Option<usize> = None;
        let mut remove = Vec::new();
        for (k, rc) in row.children.iter().enumerate() {
            if let RowChild::Cell(c) = rc {
                let s = span(c);
                if g >= c0 && g + s - 1 <= c1 {
                    if keep.is_none() {
                        keep = Some(k);
                    } else {
                        remove.push(k);
                    }
                }
                g += s;
            }
        }
        for k in remove.into_iter().rev() {
            row.children.remove(k);
        }
        let Some(k) = keep else { return Err("selection doesn't form a rectangle of cells".into()) };
        let RowChild::Cell(cell) = &mut row.children[k] else { unreachable!() };
        set_tcpr(cell, (width > 1).then(|| Element::new("w:gridSpan").with_attr("w:val", width.to_string())), "w:gridSpan");
        if r1 > r0 {
            let v = if ri == r0 { Element::new("w:vMerge").with_attr("w:val", "restart") } else { Element::new("w:vMerge") };
            set_tcpr(cell, Some(v), "w:vMerge");
        } else {
            set_tcpr(cell, None, "w:vMerge");
        }
        if ri == r0 {
            let mut blocks = std::mem::take(&mut content);
            if blocks.is_empty() || !matches!(blocks.last(), Some(Block::Para(_))) {
                blocks.push(empty_para(ids));
            }
            first_para = blocks.iter().find_map(|b| if let Block::Para(p) = b { Some(p.id) } else { None });
            cell.blocks = blocks;
        } else {
            cell.blocks = vec![empty_para(ids)];
        }
    }
    sync_widths(t);
    first_para.ok_or_else(|| "merge produced no paragraph".into())
}

/// Undo merges on the cell at grid position (row, col).
pub fn split(t: &mut Table, ri: usize, col: usize, ids: &mut IdGen) -> Result<(), String> {
    let map = grid_map(t);
    let Some(Some(slot)) = map.get(ri).and_then(|r| r.get(col)).cloned() else { return Err("no cell there".into()) };
    let origin = slot.origin_row;
    let start_col = map[slot.row].iter().position(|x| x.as_ref().is_some_and(|x| x.cell == slot.cell)).unwrap_or(col);
    let mut changed = false;
    // Vertical: clear the merge in this column range.
    let mut r = origin;
    while r < map.len() && map[r].get(start_col).and_then(|x| x.as_ref()).is_some_and(|x| x.origin_row == origin) {
        let cell_idx = map[r][start_col].unwrap().cell;
        let row = row_mut(t, r);
        if let Some(cell) = row.cells_mut().nth(cell_idx) {
            if vmerge(cell).is_some() {
                set_tcpr(cell, None, "w:vMerge");
                changed = true;
            }
        }
        r += 1;
    }
    // Horizontal: break a spanned cell into single-column cells, in every row it covered.
    for rr in origin..r.max(origin + 1) {
        let row = row_mut(t, rr);
        let mut g = row.trpr_before();
        let mut at = None;
        for (k, rc) in row.children.iter().enumerate() {
            if let RowChild::Cell(c) = rc {
                let s = span(c);
                if g == start_col && s > 1 {
                    at = Some((k, s));
                }
                g += s;
            }
        }
        if let Some((k, s)) = at {
            let RowChild::Cell(cell) = &mut row.children[k] else { unreachable!() };
            set_tcpr(cell, None, "w:gridSpan");
            let mut template = cell.tcpr.clone();
            let sdts = cell.sdts.clone();
            if let Some(tp) = template.as_mut() {
                tp.remove_children("w:vMerge");
            }
            for j in 1..s {
                let c = Cell { attrs: vec![], tcpr: template.clone(), blocks: vec![empty_para(ids)], sdts: sdts.clone() };
                row.children.insert(k + j, RowChild::Cell(c));
            }
            changed = true;
        }
    }
    if !changed {
        return Err("this cell isn't merged".into());
    }
    sync_widths(t);
    Ok(())
}

/// Set grid column widths exactly (twips) and keep cell widths in sync.
pub fn set_widths(t: &mut Table, widths: &[i64]) {
    for c in t.children.iter_mut() {
        if let TableChild::Other(e) = c {
            if e.name == "w:tblGrid" {
                e.children.retain(|n| !matches!(n, Node::Element(x) if x.name == "w:gridCol"));
                for w in widths {
                    e.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", (*w).max(200).to_string())));
                }
            }
        }
    }
    // Fixed layout honours the grid exactly, like Word after a manual resize.
    let total: i64 = widths.iter().sum();
    with_tblpr(t, |p| {
        p.remove_children("w:tblW");
        put_ordered(p, Element::new("w:tblW").with_attr("w:w", total.to_string()).with_attr("w:type", "dxa"), TBLPR_ORDER);
        p.remove_children("w:tblLayout");
        put_ordered(p, Element::new("w:tblLayout").with_attr("w:type", "fixed"), TBLPR_ORDER);
    });
    sync_widths(t);
}

pub(crate) fn with_tblpr(t: &mut Table, f: impl FnOnce(&mut Element)) {
    let idx = t.children.iter().position(|c| matches!(c, TableChild::Other(e) if e.name == "w:tblPr"));
    let idx = match idx {
        Some(i) => i,
        None => {
            t.children.insert(0, TableChild::Other(Element::new("w:tblPr")));
            0
        }
    };
    if let TableChild::Other(e) = &mut t.children[idx] {
        f(e);
    }
}

/// Apply `f` to every cell whose origin lies in the rectangle.
pub fn for_cells(t: &mut Table, rect: (usize, usize, usize, usize), f: &mut dyn FnMut(&mut Cell)) {
    let (r0, c0, r1, c1) = rect;
    for (ri, row) in t.rows_mut().enumerate() {
        if ri < r0 || ri > r1 {
            continue;
        }
        let mut g = row.trpr_before();
        for c in row.cells_mut() {
            let s = span(c);
            if g + s > c0 && g <= c1 {
                f(c);
            }
            g += s;
        }
    }
}

pub fn set_shading(t: &mut Table, rect: (usize, usize, usize, usize), fill: Option<&str>) {
    for_cells(t, rect, &mut |c| {
        let el = fill.map(|f| Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", f.trim_start_matches('#').to_uppercase()));
        set_tcpr(c, el, "w:shd");
    });
}

pub fn set_valign(t: &mut Table, rect: (usize, usize, usize, usize), v: &str) {
    for_cells(t, rect, &mut |c| {
        let el = (v != "top").then(|| Element::new("w:vAlign").with_attr("w:val", if v == "middle" { "center" } else { v }));
        set_tcpr(c, el, "w:vAlign");
    });
}

pub fn set_borders(t: &mut Table, preset: BorderPreset) {
    let line = |n: &'static str| Element::new(n).with_attr("w:val", "single").with_attr("w:sz", "4").with_attr("w:space", "0").with_attr("w:color", "auto");
    let nil = |n: &'static str| Element::new(n).with_attr("w:val", "nil");
    let sides = ["w:top", "w:left", "w:bottom", "w:right", "w:insideH", "w:insideV"];
    let mut b = Element::new("w:tblBorders");
    for s in sides {
        let on = match preset {
            BorderPreset::Grid => true,
            BorderPreset::Outer => !s.starts_with("w:inside"),
            BorderPreset::Horizontal => matches!(s, "w:top" | "w:bottom" | "w:insideH"),
            BorderPreset::None => false,
        };
        b.children.push(Node::Element(if on { line(s) } else { nil(s) }));
    }
    with_tblpr(t, |p| {
        p.remove_children("w:tblBorders");
        put_ordered(p, b, TBLPR_ORDER);
    });
    // A preset replaces per-cell overrides so the result is uniform.
    for row in t.rows_mut() {
        for c in row.cells_mut() {
            set_tcpr(c, None, "w:tcBorders");
        }
    }
}

pub fn set_header_row(t: &mut Table, on: bool) {
    let Some(row) = t.rows_mut().next() else { return };
    let idx = row.children.iter().position(|c| matches!(c, RowChild::Other(e) if e.name == "w:trPr"));
    let idx = match idx {
        Some(i) => i,
        None => {
            row.children.insert(0, RowChild::Other(Element::new("w:trPr")));
            0
        }
    };
    if let RowChild::Other(p) = &mut row.children[idx] {
        p.remove_children("w:tblHeader");
        if on {
            p.children.push(Node::Element(Element::new("w:tblHeader")));
        }
    }
}

/// The table's design: its table style and which parts of it show (`w:tblLook`). The table's own borders and
/// fill are dropped, since they would hide the design's; what is set on single cells stays.
pub fn set_design(t: &mut Table, style: Option<&str>, look: Option<crate::table_style::TableLook>) {
    with_tblpr(t, |p| {
        if let Some(s) = style {
            put_ordered(p, Element::new("w:tblStyle").with_attr("w:val", s), TBLPR_ORDER);
            p.remove_children("w:tblBorders");
            p.remove_children("w:shd");
        }
        if let Some(l) = look {
            put_ordered(p, l.element(), TBLPR_ORDER);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn table(rows: usize, cols: usize) -> (Table, IdGen) {
        let mut ids = IdGen(0);
        let mut src = String::from(r#"<w:tbl xmlns:w="w"><w:tblPr/><w:tblGrid>"#);
        for _ in 0..cols {
            src.push_str(r#"<w:gridCol w:w="1000"/>"#);
        }
        src.push_str("</w:tblGrid>");
        for r in 0..rows {
            src.push_str("<w:tr>");
            for c in 0..cols {
                src.push_str(&format!("<w:tc><w:p><w:r><w:t>{r}{c}</w:t></w:r></w:p></w:tc>"));
            }
            src.push_str("</w:tr>");
        }
        src.push_str("</w:tbl>");
        let el = xml::parse(src.as_bytes()).unwrap().root;
        (parse_table(&el, &mut ids), ids)
    }

    fn texts(t: &Table) -> Vec<Vec<String>> {
        t.rows().map(|r| r.cells().map(|c| c.blocks.iter().map(|b| if let Block::Para(p) = b { p.text() } else { String::new() }).collect::<Vec<_>>().join("|")).collect()).collect()
    }

    #[test]
    fn merge_and_split() {
        let (mut t, mut ids) = table(3, 3);
        merge(&mut t, (0, 0, 1, 1), &mut ids).unwrap();
        let tx = texts(&t);
        assert_eq!(tx[0], ["00|01|10|11", "02"]);
        assert_eq!(tx[1], ["", "12"]);
        let map = grid_map(&t);
        assert_eq!(map[1][1].unwrap().origin_row, 0);
        // Selecting a single slot inside the merge expands to the whole merged block.
        assert_eq!(normalize_rect(&t, 1, 1, 1, 1), (0, 0, 1, 1));
        split(&mut t, 1, 0, &mut ids).unwrap();
        assert_eq!(t.rows().map(|r| r.cells().count()).collect::<Vec<_>>(), [3, 3, 3]);
        assert!(grid_map(&t).iter().flatten().all(|s| s.unwrap().origin_row == s.unwrap().row));
    }

    #[test]
    fn widths_and_borders() {
        let (mut t, _) = table(2, 2);
        set_widths(&mut t, &[3000, 1500]);
        assert_eq!(t.grid(), [3000, 1500]);
        set_borders(&mut t, BorderPreset::Horizontal);
        let b = t.tblpr().unwrap().child("w:tblBorders").unwrap();
        assert_eq!(b.child("w:insideV").unwrap().attr("w:val"), Some("nil"));
        assert_eq!(b.child("w:insideH").unwrap().attr("w:val"), Some("single"));
        set_header_row(&mut t, true);
        set_shading(&mut t, (0, 0, 0, 1), Some("#eeeeee"));
        let first = t.rows().next().unwrap().cells().next().unwrap();
        assert_eq!(first.tcpr.as_ref().unwrap().child("w:shd").unwrap().attr("w:fill"), Some("EEEEEE"));
    }
}
