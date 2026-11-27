//! The ops behind the Table properties dialog and the rest of the table commands: table, row, column and cell
//! properties, borders, splitting cells and tables, inserting and deleting single cells, sorting rows, turning text
//! into a table and back, and what the dialogs read (`table_details`).

use super::*;
use crate::formula;
use crate::sort::SortOptions;
use crate::render::TableBorders;
use crate::table;
use crate::table_edit::{self, Shift, TextSeparator};
use crate::table_format::{self, BorderEdges, CellFormat, CellInfo, Edge, RowFormat, RowInfo, TableFormat, TableInfo, Width, WidthKind};

/// What the Table properties dialog and the Borders and shading dialog read about the table at the caret.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDetails {
    pub table: TableInfo,
    /// The grid column widths in twips.
    pub grid: Vec<i64>,
    /// Every row's own settings.
    pub rows: Vec<RowInfo>,
    /// The caret's row and its first grid column (from 0), and the cell's span.
    pub row: usize,
    pub col: usize,
    pub span: usize,
    pub cell: CellInfo,
    /// The preferred width of the caret's column: what its single cells ask for, else its grid width.
    pub column_width: Width,
    /// The borders the table draws (its style and its own), and the ones the caret's cell draws.
    pub borders: TableBorders,
    pub cell_borders: TableBorders,
    pub diag_down: Option<crate::props::Border>,
    pub diag_up: Option<crate::props::Border>,
    pub shading: Option<String>,
}

/// The innermost table holding paragraph `pid`, with the row and cell it is in.
fn table_holding<'a>(blocks: impl IntoIterator<Item = &'a Block>, pid: NodeId) -> Option<(&'a Table, usize, usize)> {
    for b in blocks {
        match b {
            Block::Table(t) => {
                for (ri, row) in t.rows().enumerate() {
                    for (ci, cell) in row.cells().enumerate() {
                        if let Some(r) = table_holding(&cell.blocks, pid) {
                            return Some(r);
                        }
                        if find_block(&cell.blocks, pid).is_some() {
                            return Some((t, ri, ci));
                        }
                    }
                }
            }
            Block::Container(c) => {
                if let Some(r) = table_holding(&c.blocks, pid) {
                    return Some(r);
                }
            }
            _ => {}
        }
    }
    None
}

/// Size a table the way the Insert table and Convert text to table dialogs ask.
fn size_table(t: &mut Table, mode: Option<&str>, column_width: Option<i64>) -> Result<(), EditError> {
    let cols = t.grid().len().max(1) as i64;
    match mode {
        None => {}
        Some("contents") => {
            table_format::apply_table(t, &TableFormat { width: Some(Width::default()), autofit: Some(true), ..Default::default() }).map_err(EditError::Invalid)?;
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    table_format::apply_cell(c, &CellFormat { width: Some(Width::default()), ..Default::default() }).map_err(EditError::Invalid)?;
                }
            }
        }
        Some("window") => {
            table_format::apply_table(t, &TableFormat { width: Some(Width { kind: WidthKind::Pct, value: 5000 }), autofit: Some(true), ..Default::default() }).map_err(EditError::Invalid)?;
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    let s = table::span(c) as i64;
                    table_format::apply_cell(c, &CellFormat { width: Some(Width { kind: WidthKind::Pct, value: 5000 * s / cols }), ..Default::default() }).map_err(EditError::Invalid)?;
                }
            }
        }
        Some("fixed") => {
            let widths = match column_width {
                Some(w) => vec![w.max(200); cols as usize],
                None => t.grid(),
            };
            table::set_widths(t, &widths);
        }
        Some(other) => return Err(EditError::Invalid(format!("autofit must be fixed, contents or window, not \"{other}\""))),
    }
    Ok(())
}

impl Document {
    /// Size a table made by `insert_table` (see the op).
    pub(super) fn size_new_table(&self, t: &mut Table, autofit: Option<&str>, column_width: Option<i64>) -> Result<(), EditError> {
        size_table(t, autofit, column_width)
    }

    /// What the dialogs read about the table holding paragraph `pid` and the cell it is in.
    pub fn table_details(&self, pid: NodeId) -> Option<TableDetails> {
        let (blocks, part) = self.locate(pid);
        let (t, ri, ci) = table_holding(blocks, pid)?;
        let row = t.rows().nth(ri)?;
        let place = table_edit::layout(row).into_iter().find(|p| p.cell == ci)?;
        let cell = row.cells().nth(ci)?;
        let grid = t.grid();
        // The column's preferred width: a single cell in the column that says one, else the grid.
        let mut column_width = Width { kind: WidthKind::Dxa, value: grid.get(place.start).copied().unwrap_or(0) };
        if place.span == 1 {
            let asked = table_format::cell_info(cell).width;
            if asked.kind == WidthKind::Pct {
                column_width = asked;
            }
        }
        let mut ctx = self.render_ctx(part, &self.last_labels);
        let rt = ctx.table(t);
        // The rendering folds a cell joined to the one before it (`w:hMerge`) into that one, so its cells are fewer.
        let shown = {
            let (mut joined, mut n, mut at) = (false, 0usize, 0usize);
            for (k, c) in row.cells().enumerate() {
                let hm = c.tcpr.as_ref().and_then(|p| p.child("w:hMerge")).map(|v| v.attr("w:val").unwrap_or("continue"));
                if hm == Some("continue") && joined {
                    at = n.saturating_sub(1);
                } else {
                    if hm != Some("continue") {
                        joined = hm.is_some();
                    }
                    at = n;
                    n += 1;
                }
                if k == ci {
                    break;
                }
            }
            at
        };
        let rc = rt.rows.get(ri).and_then(|r| r.cells.get(shown));
        let margins = rt.cell_mar;
        Some(TableDetails {
            table: table_format::table_info(t, margins),
            grid,
            rows: t.rows().map(table_format::row_info).collect(),
            row: ri,
            col: place.start,
            span: place.span,
            cell: table_format::cell_info(cell),
            column_width,
            borders: rt.borders.clone(),
            cell_borders: rc.map(|c| c.borders.clone()).unwrap_or_default(),
            diag_down: rc.and_then(|c| c.diag_down.clone()),
            diag_up: rc.and_then(|c| c.diag_up.clone()),
            shading: rc.and_then(|c| c.shading.clone()),
        })
    }

    /// The formula Word suggests for the cell holding `pid`: the sum of the numbers above, or of the ones to the left.
    pub fn suggest_formula(&self, pid: NodeId) -> String {
        let Some((cells, (r, c))) = self.formula_context(pid) else { return "=SUM(ABOVE)".into() };
        let is_number = |x: Option<&formula::Cell>| matches!(x, Some(formula::Cell::Number(_)));
        let above = (0..r).any(|i| is_number(cells[i].get(c)));
        let left = (0..c).any(|j| is_number(cells[r].get(j)));
        if !above && left { "=SUM(LEFT)".into() } else { "=SUM(ABOVE)".into() }
    }

    pub(super) fn op_table_format(&mut self, at: NodeId, f: &TableFormat) -> Result<OpResult, EditError> {
        self.touch(at);
        let r = with_table_body(&mut self.body, at, &mut |t: &mut Table, _, _| -> Result<(), String> {
            table_format::apply_table(t, f)?;
            // The borders chosen for the table replace what single cells said about the same lines.
            if let Some(b) = &f.borders {
                clear_cell_borders(t, b);
            }
            Ok(())
        })
        .ok_or_else(|| EditError::Invalid(format!("paragraph {at} is not inside a table")))?;
        r.map_err(EditError::Invalid)?;
        Ok(OpResult::default())
    }

    pub(super) fn op_table_rows(&mut self, from: NodeId, to: Option<NodeId>, all: bool, f: &RowFormat) -> Result<OpResult, EditError> {
        self.touch(from);
        let to = to.unwrap_or(from);
        let r = with_table_body(&mut self.body, from, &mut |t: &mut Table, ri: usize, _| -> Result<(), String> {
            let rows = t.rows().count();
            let (r0, r1) = if all {
                (0, rows - 1)
            } else {
                let a = table::cell_of(t, from).map(|c| c.0).unwrap_or(ri);
                let b = table::cell_of(t, to).map(|c| c.0).ok_or("both ends of the selection must be in the same table")?;
                (a.min(b), a.max(b))
            };
            for row in t.rows_mut().skip(r0).take(r1 - r0 + 1) {
                table_format::apply_row(row, f);
            }
            if f.header.is_some() {
                // Rows that repeat on every page are the first ones of the table.
                let flags: Vec<bool> = t.rows().map(|r| table_format::row_info(r).header).collect();
                if flags.iter().skip_while(|h| **h).any(|h| *h) {
                    return Err("rows that repeat on every page must be at the top of the table".into());
                }
            }
            Ok(())
        })
        .ok_or_else(|| EditError::Invalid(format!("paragraph {from} is not inside a table")))?;
        r.map_err(EditError::Invalid)?;
        Ok(OpResult::default())
    }

    pub(super) fn op_table_column(&mut self, at: NodeId, col: Option<usize>, width: Width) -> Result<OpResult, EditError> {
        self.touch(at);
        let r = with_table_body(&mut self.body, at, &mut |t: &mut Table, _, _| -> Result<(), String> {
            let col = match col {
                Some(c) => c,
                None => table::cell_of(t, at).map(|c| c.1).ok_or("cell not found")?,
            };
            table_format::set_column_width(t, col, width)
        })
        .ok_or_else(|| EditError::Invalid(format!("paragraph {at} is not inside a table")))?;
        r.map_err(EditError::Invalid)?;
        Ok(OpResult::default())
    }

    /// Cell actions beyond merge, unmerge and shading (see `CellAction`).
    pub(super) fn op_table_cells(&mut self, from: NodeId, to: NodeId, action: &CellAction) -> Result<OpResult, EditError> {
        self.touch(from);
        let ids = &mut self.ids;
        let res = with_table_body(&mut self.body, from, &mut |t: &mut Table, _, _| -> Result<Option<NodeId>, String> {
            let a = table::cell_of(t, from).ok_or("cell not found")?;
            let b = table::cell_of(t, to).ok_or("both ends of the selection must be in the same table")?;
            let rect = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), (a.1 + a.2 - 1).max(b.1 + b.2 - 1));
            match action {
                CellAction::Merge => table::merge(t, rect, ids).map(Some),
                CellAction::Split { cols: None, rows: None } => table::split(t, a.0, a.1, ids).map(|_| None),
                CellAction::Split { cols, rows } => table_edit::split_cell(t, a.0, a.1, cols.unwrap_or(1), rows.unwrap_or(1), ids).map(Some),
                CellAction::Shading { color } => {
                    let r = table::normalize_rect(t, rect.0, rect.1, rect.2, rect.3);
                    table::set_shading(t, r, color.as_deref().filter(|c| !c.is_empty() && *c != "none"));
                    Ok(None)
                }
                CellAction::Valign { value } => {
                    let r = table::normalize_rect(t, rect.0, rect.1, rect.2, rect.3);
                    table::set_valign(t, r, value);
                    Ok(None)
                }
                CellAction::Format(f) => {
                    let r = table::normalize_rect(t, rect.0, rect.1, rect.2, rect.3);
                    let mut err = None;
                    table::for_cells(t, r, &mut |c| {
                        if let Err(e) = table_format::apply_cell(c, f) {
                            err = Some(e);
                        }
                    });
                    err.map_or(Ok(None), Err)
                }
                CellAction::Borders(e) => {
                    let r = table::normalize_rect(t, rect.0, rect.1, rect.2, rect.3);
                    set_rect_borders(t, r, e);
                    Ok(None)
                }
            }
        })
        .ok_or_else(|| EditError::Invalid(format!("paragraph {from} is not inside a table")))?;
        let caret = res.map_err(EditError::Invalid)?;
        Ok(OpResult { caret: caret.map(|p| Pos::new(p, 0)), ..Default::default() })
    }

    pub(super) fn op_split_table(&mut self, at: NodeId) -> Result<OpResult, EditError> {
        let (tid, ri) = with_table_body(&mut self.body, at, &mut |t: &mut Table, ri, _| (t.id, ri)).ok_or_else(|| EditError::Invalid(format!("paragraph {at} is not inside a table")))?;
        let second_id = self.ids.fresh();
        let para_id = self.ids.fresh();
        let (mut list, i) = self.body.parent_mut(tid).ok_or(EditError::NotFound(tid))?;
        let Block::Table(t) = &list[i] else { return Err(EditError::Invalid("not a table".into())) };
        if ri == 0 {
            // Before the first row there is nothing to split: the table just gets a paragraph above it, as in Word.
            list.insert(i, Block::Para(Paragraph::new(para_id)));
        } else {
            let split = table_edit::split_table(t, ri, second_id).map_err(EditError::Invalid)?;
            list.splice(i..=i, [Block::Table(split.first), Block::Para(Paragraph::new(para_id)), Block::Table(split.second)]);
        }
        self.touch(tid);
        self.touch(para_id);
        self.touch(second_id);
        let created = if ri == 0 { vec![para_id] } else { vec![second_id, para_id] };
        Ok(OpResult { caret: Some(Pos::new(para_id, 0)), created, count: None })
    }

    pub(super) fn op_table_cell(&mut self, at: NodeId, action: TableCellAction, shift: Shift) -> Result<OpResult, EditError> {
        self.touch(at);
        let ids = &mut self.ids;
        let r = with_table_body(&mut self.body, at, &mut |t: &mut Table, ri: usize, ci: usize| -> Result<NodeId, String> {
            match action {
                TableCellAction::Insert => table_edit::insert_cell(t, ri, ci, shift, ids),
                TableCellAction::Delete => table_edit::delete_cell(t, ri, ci, shift, ids),
            }
        })
        .ok_or_else(|| EditError::Invalid(format!("paragraph {at} is not inside a table")))?;
        let p = r.map_err(EditError::Invalid)?;
        Ok(OpResult { caret: Some(Pos::new(p, 0)), ..Default::default() })
    }

    pub(super) fn op_sort_table(&mut self, from: NodeId, to: Option<NodeId>, opts: &SortOptions) -> Result<OpResult, EditError> {
        self.touch(from);
        let r = with_table_body(&mut self.body, from, &mut |t: &mut Table, ri: usize, _| -> Result<(), String> {
            let rows = t.rows().count();
            let (r0, r1) = match to {
                Some(to) => {
                    let a = table::cell_of(t, from).map(|c| c.0).unwrap_or(ri);
                    let b = table::cell_of(t, to).map(|c| c.0).ok_or("both ends of the selection must be in the same table")?;
                    (a.min(b), a.max(b))
                }
                None => (0, rows - 1),
            };
            table_edit::sort_rows(t, r0, r1, opts)
        })
        .ok_or_else(|| EditError::Invalid(format!("paragraph {from} is not inside a table")))?;
        r.map_err(EditError::Invalid)?;
        Ok(OpResult::default())
    }

    pub(super) fn op_convert_to_table(&mut self, ids: &[Id], sep: TextSeparator, sep_char: Option<char>, columns: Option<usize>, autofit: Option<&str>) -> Result<OpResult, EditError> {
        if ids.is_empty() {
            return Err(EditError::Invalid("select the paragraphs to turn into a table".into()));
        }
        // The paragraphs must follow each other in one place.
        let (list, _) = self.body.parent_mut(ids[0].0).ok_or(EditError::NotFound(ids[0].0))?;
        let mut at: Vec<usize> = Vec::new();
        for id in ids {
            at.push(list.iter().position(|b| b.id() == id.0).ok_or_else(|| EditError::Invalid("the paragraphs must follow each other in the same place".into()))?);
        }
        at.sort_unstable();
        at.dedup();
        if at.windows(2).any(|w| w[1] != w[0] + 1) {
            return Err(EditError::Invalid("the paragraphs must follow each other".into()));
        }
        let mut paras: Vec<Paragraph> = Vec::new();
        for &i in &at {
            match &list[i] {
                Block::Para(p) if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) => return Err(EditError::Invalid("a paragraph that ends a section can't go into a table".into())),
                Block::Para(p) => paras.push(p.clone()),
                _ => return Err(EditError::Invalid("only paragraphs can be turned into a table".into())),
            }
        }
        // The cells: (content, the paragraph properties it had).
        let mut cells: Vec<Vec<(Vec<Inline>, Option<Element>)>> = Vec::new();
        match sep.char(sep_char) {
            None => {
                let cols = columns.unwrap_or(1).clamp(1, 50);
                for chunk in paras.chunks(cols) {
                    cells.push(chunk.iter().map(|p| (p.inlines.clone(), p.ppr.clone())).collect());
                }
            }
            Some(ch) => {
                for p in &paras {
                    cells.push(table_edit::split_inlines(p, ch).into_iter().map(|c| (c, p.ppr.clone())).collect());
                }
            }
        }
        let first_id = at[0];
        for id in ids {
            self.touch(id.0);
        }
        let mut t = self.build_table(cells.iter().map(|r| r.iter().map(|c| c.0.clone()).collect()).collect(), false)?;
        for (row, src) in t.rows_mut().zip(&cells) {
            for (cell, (_, ppr)) in row.cells_mut().zip(src) {
                if let Some(Block::Para(p)) = cell.blocks.first_mut() {
                    p.ppr = ppr.clone();
                }
            }
        }
        size_table(&mut t, autofit, None)?;
        let first = first_para_in(&t);
        let tid = t.id;
        let (mut list, _) = self.body.parent_mut(ids[0].0).ok_or(EditError::NotFound(ids[0].0))?;
        list.splice(at[0]..=*at.last().unwrap(), [Block::Table(t)]);
        let _ = first_id;
        self.touch(tid);
        self.ensure_para_after(tid);
        Ok(OpResult { caret: first.map(|p| Pos::new(p, 0)), created: vec![tid], count: None })
    }

    pub(super) fn op_convert_to_text(&mut self, at: NodeId, sep: TextSeparator, sep_char: Option<char>) -> Result<OpResult, EditError> {
        let tid = with_table_body(&mut self.body, at, &mut |t: &mut Table, _, _| t.id).ok_or_else(|| EditError::Invalid(format!("paragraph {at} is not inside a table")))?;
        self.touch(tid);
        let (mut list, i) = self.body.parent_mut(tid).ok_or(EditError::NotFound(tid))?;
        let Block::Table(t) = &list[i] else { return Err(EditError::Invalid("not a table".into())) };
        let blocks = table_edit::table_to_blocks(t, sep, sep_char);
        let new_ids: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        let caret = first_para_blocks(&blocks);
        if blocks.is_empty() {
            return Err(EditError::Invalid("the table has no text".into()));
        }
        list.splice(i..=i, blocks);
        for id in &new_ids {
            self.touch(*id);
        }
        Ok(OpResult { caret: caret.map(|p| Pos::new(p, 0)), created: new_ids, count: None })
    }
}

/// Set the edges of a rectangle of cells: the outside edges, the lines between the cells, the diagonals. The cells
/// next to the rectangle get the matching edge too, since two cells that share a line must agree about it.
pub fn set_rect_borders(t: &mut Table, rect: (usize, usize, usize, usize), e: &BorderEdges) {
    let (r0, c0, r1, c1) = rect;
    let row_count = t.rows().count();
    for ri in 0..row_count {
        let places = table_edit::layout(t.rows().nth(ri).unwrap());
        for p in places {
            let (start, end) = (p.start, p.start + p.span);
            let in_cols = start <= c1 && end > c0;
            let mut edges = BorderEdges::default();
            if (r0..=r1).contains(&ri) && in_cols {
                if ri == r0 {
                    edges.top = e.top.clone();
                } else {
                    edges.top = e.inside_h.clone();
                }
                if ri == r1 {
                    edges.bottom = e.bottom.clone();
                } else {
                    edges.bottom = e.inside_h.clone();
                }
                if start <= c0 {
                    edges.left = e.left.clone();
                } else {
                    edges.left = e.inside_v.clone();
                }
                if end > c1 {
                    edges.right = e.right.clone();
                } else {
                    edges.right = e.inside_v.clone();
                }
                edges.diag_down = e.diag_down.clone();
                edges.diag_up = e.diag_up.clone();
            } else {
                // A neighbour across the rectangle's outside edge.
                if ri + 1 == r0 && in_cols {
                    edges.bottom = e.top.clone();
                }
                if ri == r1 + 1 && in_cols {
                    edges.top = e.bottom.clone();
                }
                if (r0..=r1).contains(&ri) {
                    if end == c0 {
                        edges.right = e.left.clone();
                    }
                    if start == c1 + 1 {
                        edges.left = e.right.clone();
                    }
                }
            }
            if !edges.is_empty() {
                let row = table::row_mut(t, ri);
                if let Some(c) = row.cells_mut().nth(p.cell) {
                    table_format::apply_cell_borders(c, &edges);
                }
            }
        }
    }
}

/// Take what single cells say about the lines the table's own borders now set.
fn clear_cell_borders(t: &mut Table, e: &BorderEdges) {
    let rows = t.rows().count();
    let cols = t.grid().len();
    let clear = Some(Edge { val: "clear".into(), sz: 4, color: "auto".into(), space: 0 });
    for ri in 0..rows {
        let places = table_edit::layout(t.rows().nth(ri).unwrap());
        for p in places {
            let mut edges = BorderEdges::default();
            let first_row = ri == 0;
            let last_row = ri + 1 == rows;
            let first_col = p.start == 0;
            let last_col = p.start + p.span >= cols;
            if (first_row && e.top.is_some()) || (!first_row && e.inside_h.is_some()) {
                edges.top = clear.clone();
            }
            if (last_row && e.bottom.is_some()) || (!last_row && e.inside_h.is_some()) {
                edges.bottom = clear.clone();
            }
            if (first_col && e.left.is_some()) || (!first_col && e.inside_v.is_some()) {
                edges.left = clear.clone();
            }
            if (last_col && e.right.is_some()) || (!last_col && e.inside_v.is_some()) {
                edges.right = clear.clone();
            }
            if !edges.is_empty() {
                if let Some(c) = table::row_mut(t, ri).cells_mut().nth(p.cell) {
                    table_format::apply_cell_borders(c, &edges);
                }
            }
        }
    }
}
