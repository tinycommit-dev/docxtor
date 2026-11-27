//! Table structure beyond rows and columns: split a cell into any number of cells, split a table in two, insert and
//! delete single cells with the others shifting, sort the rows, and turn text into a table and back.
//!
//! Everything works on the model of a table: rows of cells with `gridSpan` and `vMerge`, over a `tblGrid`.

use crate::edit;
use crate::model::*;
use crate::props::put_ordered;
use crate::sort::{self, SortOptions, Separator};
use crate::table::{empty_para, set_tcpr, span, sync_widths, vmerge};
use crate::table_format::trpr;
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

/// A cell's place in the grid: its index among the row's cells, the grid column it starts at, and its span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub cell: usize,
    pub start: usize,
    pub span: usize,
}

/// Where each cell of a row sits in the grid (a row that starts short begins further in).
pub fn layout(row: &Row) -> Vec<Placed> {
    let mut g = row.trpr_before();
    row.cells()
        .enumerate()
        .map(|(cell, c)| {
            let s = span(c);
            let p = Placed { cell, start: g, span: s };
            g += s;
            p
        })
        .collect()
}

fn grid_after(row: &Row) -> usize {
    trpr(row).and_then(|p| p.child("w:gridAfter")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn set_grid_after(row: &mut Row, n: usize) {
    let idx = row.children.iter().position(|c| matches!(c, RowChild::Other(e) if e.name == "w:trPr"));
    let idx = idx.unwrap_or_else(|| {
        row.children.insert(0, RowChild::Other(Element::new("w:trPr")));
        0
    });
    if let RowChild::Other(p) = &mut row.children[idx] {
        p.remove_children("w:gridAfter");
        p.remove_children("w:wAfter");
        if n > 0 {
            const ORDER: &[&str] = &["w:cnfStyle", "w:divId", "w:gridBefore", "w:gridAfter", "w:wBefore", "w:wAfter", "w:cantSplit", "w:trHeight", "w:tblHeader"];
            put_ordered(p, Element::new("w:gridAfter").with_attr("w:val", n.to_string()), ORDER);
        } else if p.children.is_empty() {
            row.children.remove(idx);
        }
    }
}

fn grid_len(t: &Table) -> usize {
    t.grid().len().max(t.rows().map(|r| r.trpr_before() + r.cells().map(span).sum::<usize>() + grid_after(r)).max().unwrap_or(0))
}

fn set_grid(t: &mut Table, grid: &[i64]) {
    for c in t.children.iter_mut() {
        if let TableChild::Other(e) = c {
            if e.name == "w:tblGrid" {
                e.children.retain(|n| !matches!(n, Node::Element(x) if x.name == "w:gridCol"));
                for w in grid {
                    e.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", (*w).max(1).to_string())));
                }
            }
        }
    }
}

fn first_para(blocks: &[Block]) -> Option<NodeId> {
    let mut first = None;
    walk_paragraphs(blocks, &mut |p| {
        first.get_or_insert(p.id);
    });
    first
}

/// The cell properties a new cell next to an old one starts from: fill, borders, alignment, margins, but no merge.
fn template_of(c: &Cell) -> Option<Element> {
    let mut tp = c.tcpr.clone()?;
    for n in ["w:gridSpan", "w:vMerge", "w:hMerge", "w:tcW"] {
        tp.remove_children(n);
    }
    Some(tp)
}

fn fresh_cell(template: &Option<Element>, span: usize, merge: Option<&str>, ids: &mut IdGen) -> Cell {
    let mut c = Cell { attrs: vec![], tcpr: template.clone(), blocks: vec![empty_para(ids)], sdts: vec![] };
    if span > 1 {
        set_tcpr(&mut c, Some(Element::new("w:gridSpan").with_attr("w:val", span.to_string())), "w:gridSpan");
    }
    match merge {
        Some("restart") => set_tcpr(&mut c, Some(Element::new("w:vMerge").with_attr("w:val", "restart")), "w:vMerge"),
        Some(_) => set_tcpr(&mut c, Some(Element::new("w:vMerge")), "w:vMerge"),
        None => {}
    }
    c
}

/// The cell of a row that covers grid column `g`.
fn covering(row: &Row, g: usize) -> Option<Placed> {
    layout(row).into_iter().find(|p| p.start <= g && g < p.start + p.span)
}

/// The rows (first, last) a cell takes up when it is merged vertically, for the cell at grid column `g` of row `ri`.
fn vertical_extent(t: &Table, ri: usize, g: usize) -> (usize, usize) {
    let rows: Vec<&Row> = t.rows().collect();
    let is_continue = |r: usize| covering(rows[r], g).is_some_and(|p| vmerge(rows[r].cells().nth(p.cell).unwrap()) == Some("continue"));
    let mut first = ri;
    while first > 0 && is_continue(first) {
        first -= 1;
    }
    let mut last = first;
    while last + 1 < rows.len() && is_continue(last + 1) {
        last += 1;
    }
    (first, last.max(ri))
}

/// Divide `n` into `parts` shares as even as possible, the larger ones first.
fn shares(n: usize, parts: usize) -> Vec<usize> {
    (0..parts).map(|i| n / parts + usize::from(i < n % parts)).collect()
}

/// Split the cell holding grid column `g` of row `ri` into `cols` by `rows` cells. A cell that was merged is divided
/// where it was joined; when more cells are asked for than it covers, columns and rows are added to the table
/// around it, and the other cells stretch across them. Returns the first paragraph of the first piece.
pub fn split_cell(t: &mut Table, ri: usize, g: usize, cols: usize, rows: usize, ids: &mut IdGen) -> Result<NodeId, String> {
    if cols == 0 || rows == 0 || cols > 63 || rows > 32 {
        return Err("a cell can be split into 1 to 63 columns and 1 to 32 rows".into());
    }
    if cols == 1 && rows == 1 {
        return Err("choose more than one column or row".into());
    }
    let row_count = t.rows().count();
    let here = t.rows().nth(ri).and_then(|r| covering(r, g)).ok_or("there is no cell there")?;
    let (c0, mut w) = (here.start, here.span);
    let (r0, r1) = vertical_extent(t, ri, c0);
    let mut h = r1 - r0 + 1;

    // More columns than the cell covers: the grid columns under it are replaced by that many.
    if cols > w {
        for (k, r) in t.rows().enumerate() {
            if (r0..=r1).contains(&k) {
                continue;
            }
            for p in layout(r) {
                let inside = |x: usize| x > c0 && x < c0 + w;
                if inside(p.start) || inside(p.start + p.span) {
                    return Err("split into fewer columns, or unmerge the neighbouring cells first".into());
                }
            }
        }
        let extra = cols - w;
        let mut grid = t.grid();
        if grid.len() < c0 + w {
            return Err("the table has no grid to split".into());
        }
        let total: i64 = grid[c0..c0 + w].iter().sum();
        let each = (total / cols as i64).max(1);
        let mut parts = vec![each; cols];
        parts[cols - 1] = (total - each * (cols as i64 - 1)).max(1);
        grid.splice(c0..c0 + w, parts);
        set_grid(t, &grid);
        for (k, r) in t.rows_mut().enumerate() {
            let place = layout(r);
            for p in place {
                if p.start <= c0 && p.start + p.span >= c0 + w {
                    let c = r.cells_mut().nth(p.cell).unwrap();
                    set_tcpr(c, Some(Element::new("w:gridSpan").with_attr("w:val", (p.span + extra).to_string())), "w:gridSpan");
                    if p.span + extra == 1 {
                        set_tcpr(c, None, "w:gridSpan");
                    }
                }
            }
            let _ = k;
        }
        w = cols;
    }

    // More rows than the cell covers: rows are added below it, and the cells beside it carry on through them.
    if rows > h {
        let add = rows - h;
        let src = t.rows().nth(r1).unwrap().clone();
        let place = layout(&src);
        let template = template_of(src.cells().nth(covering(&src, c0).unwrap().cell).unwrap());
        for k in 0..add {
            let mut nr = Row { attrs: vec![], children: vec![], sdts: src.sdts.clone() };
            if let Some(p) = trpr(&src) {
                let mut p = p.clone();
                p.remove_children("w:tblHeader");
                nr.children.push(RowChild::Other(p));
            }
            let mut emitted = false;
            for pl in &place {
                let sc = src.cells().nth(pl.cell).unwrap();
                if pl.start >= c0 && pl.start < c0 + w {
                    if !emitted {
                        nr.children.push(RowChild::Cell(fresh_cell(&template, w, None, ids)));
                        emitted = true;
                    }
                    continue;
                }
                let tp = template_of(sc);
                nr.children.push(RowChild::Cell(fresh_cell(&tp, pl.span, Some("continue"), ids)));
            }
            let at = t.children.iter().enumerate().filter(|(_, c)| matches!(c, TableChild::Row(_))).map(|(i, _)| i).nth(r1).unwrap() + 1 + k;
            t.children.insert(at, TableChild::Row(nr));
        }
        // The cells beside the target start a vertical merge in the row it ended at.
        {
            let row = crate::table::row_mut(t, r1);
            let place = layout(row);
            for pl in place {
                if pl.start >= c0 && pl.start < c0 + w {
                    continue;
                }
                let c = row.cells_mut().nth(pl.cell).unwrap();
                if vmerge(c).is_none() {
                    set_tcpr(c, Some(Element::new("w:vMerge").with_attr("w:val", "restart")), "w:vMerge");
                }
            }
        }
        h = rows;
        let _ = row_count;
    }

    // Now the cell's area is h rows by w grid columns: cut it into the pieces.
    let col_spans = shares(w, cols);
    let row_heights = shares(h, rows);
    let original = {
        let row = t.rows().nth(r0).unwrap();
        let p = covering(row, c0).unwrap();
        row.cells().nth(p.cell).unwrap().clone()
    };
    let template = template_of(&original);
    let mut first = None;
    let mut piece_of_row = Vec::new();
    for (i, hh) in row_heights.iter().enumerate() {
        for k in 0..*hh {
            piece_of_row.push((i, k, *hh));
        }
    }
    for (k, &(_, offset, height)) in piece_of_row.iter().enumerate() {
        let row = crate::table::row_mut(t, r0 + k);
        let place = covering(row, c0).ok_or("the cell's rows do not line up")?;
        let merge = if height == 1 {
            None
        } else if offset == 0 {
            Some("restart")
        } else {
            Some("continue")
        };
        let mut new_cells = Vec::new();
        for (j, s) in col_spans.iter().enumerate() {
            let mut c = fresh_cell(&template, *s, merge, ids);
            if k == 0 && j == 0 {
                c.blocks = original.blocks.clone();
                c.attrs = original.attrs.clone();
                c.sdts = original.sdts.clone();
                // The first piece keeps all the cell had, its own properties included.
                c.tcpr = original.tcpr.clone();
                set_tcpr(&mut c, (*s > 1).then(|| Element::new("w:gridSpan").with_attr("w:val", s.to_string())), "w:gridSpan");
                match merge {
                    Some("restart") => set_tcpr(&mut c, Some(Element::new("w:vMerge").with_attr("w:val", "restart")), "w:vMerge"),
                    Some(_) => set_tcpr(&mut c, Some(Element::new("w:vMerge")), "w:vMerge"),
                    None => set_tcpr(&mut c, None, "w:vMerge"),
                }
                first = first_para(&c.blocks);
            }
            new_cells.push(c);
        }
        // Put them where the old cell was.
        let mut at = 0;
        let mut idx = None;
        for (n, child) in row.children.iter().enumerate() {
            if let RowChild::Cell(_) = child {
                if at == place.cell {
                    idx = Some(n);
                    break;
                }
                at += 1;
            }
        }
        let idx = idx.unwrap();
        row.children.remove(idx);
        for (j, c) in new_cells.into_iter().enumerate() {
            row.children.insert(idx + j, RowChild::Cell(c));
        }
    }
    sync_widths(t);
    first.ok_or_else(|| "the split produced no paragraph".into())
}

/// Where the table breaks in two.
pub struct SplitTable {
    pub first: Table,
    pub second: Table,
}

/// Cut a table before row `ri` (not the first). The second table gets the same table properties and grid; cells that
/// were merged down from the first start again.
pub fn split_table(t: &Table, ri: usize, second_id: NodeId) -> Result<SplitTable, String> {
    let rows = t.rows().count();
    if ri == 0 || ri >= rows {
        return Err("a table can be split before any row but the first".into());
    }
    let mut first = Table { id: t.id, attrs: t.attrs.clone(), children: vec![] };
    let mut second = Table { id: second_id, attrs: t.attrs.clone(), children: vec![] };
    let mut seen = 0;
    for c in &t.children {
        match c {
            TableChild::Other(e) => {
                first.children.push(TableChild::Other(e.clone()));
                second.children.push(TableChild::Other(e.clone()));
            }
            TableChild::Marker(..) => {
                // A range marker stays with the rows around it: before the cut it is in the first table.
                if seen < ri {
                    first.children.push(c.clone());
                } else {
                    second.children.push(c.clone());
                }
            }
            TableChild::Row(r) => {
                if seen < ri {
                    first.children.push(TableChild::Row(r.clone()));
                } else {
                    let mut r = r.clone();
                    if seen == ri {
                        for c in r.cells_mut() {
                            if vmerge(c) == Some("continue") {
                                set_tcpr(c, Some(Element::new("w:vMerge").with_attr("w:val", "restart")), "w:vMerge");
                            }
                        }
                    }
                    second.children.push(TableChild::Row(r));
                }
                seen += 1;
            }
        }
    }
    Ok(SplitTable { first, second })
}

/// How the cells around an inserted or deleted cell move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shift {
    Right,
    Down,
    Left,
    Up,
}

fn plain_cell(t: &Table, ri: usize, ci: usize) -> Result<(), String> {
    let c = t.rows().nth(ri).and_then(|r| r.cells().nth(ci)).ok_or("there is no cell there")?;
    if vmerge(c).is_some() || span(c) > 1 {
        return Err("cells that are merged can't be shifted; unmerge them first".into());
    }
    Ok(())
}

/// The cells of the column under a cell, from its row down: each must be a plain cell of that column.
fn column_below(t: &Table, ri: usize, g: usize) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for r in t.rows().skip(ri) {
        let p = covering(r, g).filter(|p| p.start == g && p.span == 1).ok_or("the cells below are not in line; unmerge them first")?;
        if r.cells().nth(p.cell).is_some_and(|c| vmerge(c).is_some()) {
            return Err("cells that are merged can't be shifted; unmerge them first".into());
        }
        out.push(p.cell);
    }
    Ok(out)
}

/// Insert a cell before the cell `ci` of row `ri`, and shift the others right (the row grows by a cell and the table by a
/// column when it was full) or down (the content of the column moves down a row and the table gets a row).
pub fn insert_cell(t: &mut Table, ri: usize, ci: usize, shift: Shift, ids: &mut IdGen) -> Result<NodeId, String> {
    plain_cell(t, ri, ci)?;
    let target = t.rows().nth(ri).unwrap().cells().nth(ci).unwrap().clone();
    let template = template_of(&target);
    match shift {
        Shift::Right => {
            let before = grid_len(t);
            let row = crate::table::row_mut(t, ri);
            let fresh = fresh_cell(&template, 1, None, ids);
            let id = first_para(&fresh.blocks);
            let at = row.children.iter().enumerate().filter(|(_, c)| matches!(c, RowChild::Cell(_))).map(|(i, _)| i).nth(ci).unwrap();
            row.children.insert(at, RowChild::Cell(fresh));
            // A row that stopped short uses up one of its empty places; a full one needs a new column.
            let short = grid_after(row);
            if short > 0 {
                set_grid_after(row, short - 1);
            } else {
                let mut grid = t.grid();
                let width = grid.last().copied().unwrap_or(1440);
                grid.push(width);
                set_grid(t, &grid);
                let len = before + 1;
                for (k, r) in t.rows_mut().enumerate() {
                    if k == ri {
                        continue;
                    }
                    let used = r.trpr_before() + r.cells().map(span).sum::<usize>() + grid_after(r);
                    if used < len {
                        let a = grid_after(r) + (len - used);
                        set_grid_after(r, a);
                    }
                }
            }
            sync_widths(t);
            id.ok_or_else(|| "no paragraph".into())
        }
        Shift::Down => {
            let g = layout(t.rows().nth(ri).unwrap())[ci].start;
            let column = column_below(t, ri, g)?;
            // The content of each cell moves to the one below; the last goes to a row added at the bottom.
            let blocks: Vec<Vec<Block>> = t.rows().skip(ri).zip(&column).map(|(r, &c)| r.cells().nth(c).unwrap().blocks.clone()).collect();
            let last = t.rows().last().unwrap().clone();
            let mut nr = Row { attrs: vec![], children: vec![], sdts: last.sdts.clone() };
            if let Some(p) = trpr(&last) {
                let mut p = p.clone();
                p.remove_children("w:tblHeader");
                nr.children.push(RowChild::Other(p));
            }
            for c in last.cells() {
                let mut nc = fresh_cell(&template_of(c), span(c), None, ids);
                nc.attrs = vec![];
                nr.children.push(RowChild::Cell(nc));
            }
            t.children.push(TableChild::Row(nr));
            let total = t.rows().count();
            let mut id = None;
            for (k, r) in t.rows_mut().enumerate().skip(ri) {
                let place = covering(r, g).filter(|p| p.start == g && p.span == 1).ok_or("the cells below are not in line")?;
                let cell = r.cells_mut().nth(place.cell).unwrap();
                if k == ri {
                    cell.blocks = vec![empty_para(ids)];
                    id = first_para(&cell.blocks);
                } else {
                    cell.blocks = blocks[k - ri - 1].clone();
                }
                let _ = total;
            }
            id.ok_or_else(|| "no paragraph".into())
        }
        _ => Err("insert cells by shifting right or down".into()),
    }
}

/// Delete the cell `ci` of row `ri` and shift the others left (the row ends one place short) or up (the content below
/// moves up and the last cell of the column is left empty).
pub fn delete_cell(t: &mut Table, ri: usize, ci: usize, shift: Shift, ids: &mut IdGen) -> Result<NodeId, String> {
    plain_cell(t, ri, ci)?;
    match shift {
        Shift::Left => {
            let row = crate::table::row_mut(t, ri);
            if row.cells().count() < 2 {
                return Err("a row needs at least one cell; delete the row instead".into());
            }
            let at = row.children.iter().enumerate().filter(|(_, c)| matches!(c, RowChild::Cell(_))).map(|(i, _)| i).nth(ci).unwrap();
            row.children.remove(at);
            let a = grid_after(row) + 1;
            set_grid_after(row, a);
            sync_widths(t);
            let row = t.rows().nth(ri).unwrap();
            let keep = ci.min(row.cells().count() - 1);
            first_para(&row.cells().nth(keep).unwrap().blocks).ok_or_else(|| "no paragraph".into())
        }
        Shift::Up => {
            let g = layout(t.rows().nth(ri).unwrap())[ci].start;
            let column = column_below(t, ri, g)?;
            let blocks: Vec<Vec<Block>> = t.rows().skip(ri).zip(&column).map(|(r, &c)| r.cells().nth(c).unwrap().blocks.clone()).collect();
            let n = blocks.len();
            let mut id = None;
            for (k, r) in t.rows_mut().enumerate().skip(ri) {
                let place = covering(r, g).filter(|p| p.start == g && p.span == 1).ok_or("the cells below are not in line")?;
                let cell = r.cells_mut().nth(place.cell).unwrap();
                let i = k - ri;
                cell.blocks = if i + 1 < n { blocks[i + 1].clone() } else { vec![empty_para(ids)] };
                if i == 0 {
                    id = first_para(&cell.blocks);
                }
            }
            id.ok_or_else(|| "no paragraph".into())
        }
        _ => Err("delete cells by shifting left or up".into()),
    }
}

fn cell_text(c: &Cell) -> String {
    let mut parts = Vec::new();
    walk_paragraphs(&c.blocks, &mut |p| parts.push(p.plain_text().replace(['\t', '\n'], " ")));
    parts.join(" ")
}

/// Sort the rows `first..=last` by the text of their cells. Each key's `field` is the column, counting from 1; the first
/// row stays where it is when `header` is on. Rows that are merged down can't be moved.
pub fn sort_rows(t: &mut Table, first: usize, last: usize, opts: &SortOptions) -> Result<(), String> {
    let rows = t.rows().count();
    if last >= rows || first > last {
        return Err("there are no such rows".into());
    }
    let start = if opts.header { first + 1 } else { first };
    if start >= last {
        return Ok(());
    }
    let cols = grid_len(t);
    let mut lines = Vec::new();
    for r in t.rows().skip(start).take(last - start + 1) {
        if r.cells().any(|c| vmerge(c).is_some()) {
            return Err("rows with cells merged down can't be sorted; unmerge them first".into());
        }
        let mut fields = vec![String::new(); cols.max(1)];
        let mut text = r.cells().map(cell_text);
        for p in layout(r) {
            if let Some(f) = fields.get_mut(p.start) {
                *f = text.next().unwrap_or_default();
            } else {
                text.next();
            }
        }
        lines.push(fields.join("\t"));
    }
    let o = SortOptions { separator: Separator::Tabs, ..opts.clone() };
    let order = sort::order(&lines, &o);
    let all: Vec<usize> = t.children.iter().enumerate().filter(|(_, c)| matches!(c, TableChild::Row(_))).map(|(i, _)| i).collect();
    let slots = &all[start..=last];
    let moved: Vec<TableChild> = slots.iter().map(|&i| t.children[i].clone()).collect();
    for (k, &i) in slots.iter().enumerate() {
        t.children[i] = moved[order[k]].clone();
    }
    Ok(())
}

/// What separates the pieces of text that become cells (and what is put between cells when a table becomes text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSeparator {
    /// Each paragraph is a cell.
    Paragraphs,
    #[default]
    Tabs,
    Commas,
    /// Another character.
    Other,
}

impl TextSeparator {
    pub fn char(self, other: Option<char>) -> Option<char> {
        match self {
            TextSeparator::Paragraphs => None,
            TextSeparator::Tabs => Some('\t'),
            TextSeparator::Commas => Some(','),
            TextSeparator::Other => Some(other.unwrap_or(';')),
        }
    }
}

/// Cut a paragraph's content at every `sep`, dropping the separators: the content of each cell, in order.
pub fn split_inlines(p: &Paragraph, sep: char) -> Vec<Vec<Inline>> {
    let text = p.text();
    let mut cuts = Vec::new();
    let mut off = 0;
    for ch in text.chars() {
        if ch == sep {
            cuts.push(off);
        }
        off += ch.len_utf16();
    }
    let mut rest = p.inlines.clone();
    let mut out = Vec::new();
    let mut done = 0;
    for cut in cuts {
        let (left, right) = edit::partition(rest, cut - done);
        out.push(left);
        let mut right = right;
        edit::delete_range(&mut right, 0, 1);
        rest = right;
        done = cut + 1;
    }
    out.push(rest);
    out
}

/// A table as text: the paragraphs in the cells, with the cells of a row joined by the separator in one paragraph
/// (`Paragraphs` leaves each cell's paragraphs as they are).
pub fn table_to_blocks(t: &Table, sep: TextSeparator, other: Option<char>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let join = sep.char(other);
    for r in t.rows() {
        let mut current: Option<Paragraph> = None;
        for (ci, c) in r.cells().enumerate() {
            let mut first_in_cell = true;
            for b in &c.blocks {
                match (b, &mut current, join) {
                    (Block::Para(p), Some(cur), Some(ch)) if first_in_cell && ci > 0 => {
                        let end = cur.len16();
                        edit::insert_text(&mut cur.inlines, end, &ch.to_string(), None);
                        cur.inlines.extend(p.inlines.clone());
                    }
                    (Block::Para(p), _, _) => {
                        if let Some(cur) = current.take() {
                            out.push(Block::Para(cur));
                        }
                        current = Some(p.clone());
                    }
                    (other_block, _, _) => {
                        if let Some(cur) = current.take() {
                            out.push(Block::Para(cur));
                        }
                        out.push(other_block.clone());
                    }
                }
                first_in_cell = false;
            }
        }
        if let Some(cur) = current.take() {
            out.push(Block::Para(cur));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml;

    fn table(src_rows: &[&[&str]]) -> (Table, IdGen) {
        let mut ids = IdGen(0);
        let cols = src_rows.iter().map(|r| r.len()).max().unwrap();
        let mut src = String::from(r#"<w:tbl xmlns:w="w"><w:tblPr/><w:tblGrid>"#);
        for _ in 0..cols {
            src.push_str(r#"<w:gridCol w:w="1200"/>"#);
        }
        src.push_str("</w:tblGrid>");
        for r in src_rows {
            src.push_str("<w:tr>");
            for c in *r {
                src.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"1200\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:t>{c}</w:t></w:r></w:p></w:tc>"));
            }
            src.push_str("</w:tr>");
        }
        src.push_str("</w:tbl>");
        (parse_table(&xml::parse(src.as_bytes()).unwrap().root, &mut ids), ids)
    }

    fn texts(t: &Table) -> Vec<Vec<String>> {
        t.rows().map(|r| r.cells().map(cell_text).collect()).collect()
    }

    fn want(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
    }

    fn shape(t: &Table) -> Vec<Vec<(usize, Option<String>)>> {
        t.rows().map(|r| r.cells().map(|c| (span(c), vmerge(c).map(String::from))).collect()).collect()
    }

    #[test]
    fn a_cell_splits_into_columns_and_the_table_grows_a_column() {
        let (mut t, mut ids) = table(&[&["a", "b"], &["c", "d"]]);
        split_cell(&mut t, 0, 0, 3, 1, &mut ids).unwrap();
        assert_eq!(t.grid(), [400, 400, 400, 1200]);
        assert_eq!(texts(&t), want(&[&["a", "", "", "b"], &["c", "d"]]));
        // The other row's cell stretches across the new columns, with its width kept.
        assert_eq!(shape(&t)[1], [(3, None), (1, None)]);
        let widths: Vec<_> = t.rows().next().unwrap().cells().map(|c| crate::table_format::cell_info(c).width.value).collect();
        assert_eq!(widths, [400, 400, 400, 1200]);
    }

    #[test]
    fn a_cell_splits_into_rows_and_the_cells_beside_it_carry_on() {
        let (mut t, mut ids) = table(&[&["a", "b"], &["c", "d"]]);
        split_cell(&mut t, 0, 0, 1, 3, &mut ids).unwrap();
        assert_eq!(texts(&t), want(&[&["a", "b"], &["", ""], &["", ""], &["c", "d"]]));
        // "b" now runs down three rows; the new rows' left cells are separate cells.
        assert_eq!(shape(&t)[0][1], (1, Some("restart".into())));
        assert_eq!(shape(&t)[1], [(1, None), (1, Some("continue".into()))]);
        assert_eq!(shape(&t)[3], [(1, None), (1, None)]);
    }

    #[test]
    fn a_merged_cell_splits_where_it_was_joined_and_rows_by_columns_work_together() {
        let (mut t, mut ids) = table(&[&["a", "b", "c"], &["d", "e", "f"]]);
        crate::table::merge(&mut t, (0, 0, 1, 1), &mut ids).unwrap();
        // Two columns by two rows from a cell two by two.
        split_cell(&mut t, 0, 0, 2, 2, &mut ids).unwrap();
        assert_eq!(shape(&t), [[(1, None), (1, None), (1, None)], [(1, None), (1, None), (1, None)]]);
        // Content stayed in the first piece.
        assert_eq!(texts(&t)[0][0], "a b d e");
        assert_eq!(texts(&t)[0][1], "");
        assert!(split_cell(&mut t, 0, 0, 1, 1, &mut ids).is_err());
    }

    #[test]
    fn a_table_splits_in_two_with_the_same_properties() {
        let (t, _) = table(&[&["a"], &["b"], &["c"]]);
        let s = split_table(&t, 1, 99).unwrap();
        assert_eq!(texts(&s.first), want(&[&["a"]]));
        assert_eq!(texts(&s.second), want(&[&["b"], &["c"]]));
        assert_eq!(s.second.grid(), s.first.grid());
        assert_eq!(s.second.id, 99);
        assert!(split_table(&t, 0, 99).is_err());
        assert!(split_table(&t, 3, 99).is_err());
    }

    #[test]
    fn cells_shift_right_and_a_column_is_added() {
        let (mut t, mut ids) = table(&[&["a", "b"], &["c", "d"]]);
        insert_cell(&mut t, 0, 1, Shift::Right, &mut ids).unwrap();
        assert_eq!(texts(&t), want(&[&["a", "", "b"], &["c", "d"]]));
        assert_eq!(t.grid().len(), 3);
        // The other row stops one place short.
        assert_eq!(grid_after(t.rows().nth(1).unwrap()), 1);
        // The shifted row's last cell is in the new column; deleting a cell and shifting left takes it back.
        delete_cell(&mut t, 0, 1, Shift::Left, &mut ids).unwrap();
        assert_eq!(texts(&t), want(&[&["a", "b"], &["c", "d"]]));
        assert_eq!(grid_after(t.rows().next().unwrap()), 1);
    }

    #[test]
    fn cells_shift_down_and_up() {
        let (mut t, mut ids) = table(&[&["a", "b"], &["c", "d"]]);
        insert_cell(&mut t, 0, 0, Shift::Down, &mut ids).unwrap();
        assert_eq!(texts(&t), want(&[&["", "b"], &["a", "d"], &["c", ""]]));
        delete_cell(&mut t, 0, 0, Shift::Up, &mut ids).unwrap();
        assert_eq!(texts(&t), want(&[&["a", "b"], &["c", "d"], &["", ""]]));
        crate::table::merge(&mut t, (0, 0, 0, 1), &mut ids).unwrap();
        assert!(insert_cell(&mut t, 0, 0, Shift::Right, &mut ids).is_err());
    }

    #[test]
    fn rows_sort_by_a_column_with_a_header() {
        let (mut t, _) = table(&[&["Name", "Qty"], &["pear", "10"], &["apple", "9"], &["fig", "100"]]);
        let by_name = SortOptions { keys: vec![sort::SortKey { field: 1, ..Default::default() }], header: true, ..Default::default() };
        sort_rows(&mut t, 0, 3, &by_name).unwrap();
        assert_eq!(texts(&t).iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["Name", "apple", "fig", "pear"]);
        let by_qty = SortOptions { keys: vec![sort::SortKey { field: 2, kind: sort::SortType::Number, descending: true }], header: true, ..Default::default() };
        sort_rows(&mut t, 0, 3, &by_qty).unwrap();
        assert_eq!(texts(&t).iter().map(|r| r[1].as_str()).collect::<Vec<_>>(), ["Qty", "100", "10", "9"]);
        assert!(sort_rows(&mut t, 0, 9, &by_qty).is_err());
    }

    #[test]
    fn text_splits_into_cells_keeping_the_formatting() {
        let mut ids = IdGen(0);
        let mut p = Paragraph::new(ids.fresh());
        edit::insert_text(&mut p.inlines, 0, "one\ttwo\t\tfour", None);
        let bold = Element::new("w:rPr").with_child(Element::new("w:b"));
        edit::format_range(&mut p.inlines, 4, 7, &mut |r| r.rpr = Some(bold.clone()));
        let cells = split_inlines(&p, '\t');
        let text = |c: &[Inline]| c.iter().map(|i| i.text()).collect::<String>();
        assert_eq!(cells.iter().map(|c| text(c)).collect::<Vec<_>>(), ["one", "two", "", "four"]);
        assert!(matches!(&cells[1][0], Inline::Run(r) if r.rpr.is_some()));
    }

    #[test]
    fn a_table_becomes_text_with_a_separator_or_as_paragraphs() {
        let (t, _) = table(&[&["a", "b"], &["c", "d"]]);
        let texts = |blocks: &[Block]| blocks.iter().map(|b| if let Block::Para(p) = b { p.text() } else { String::new() }).collect::<Vec<_>>();
        assert_eq!(texts(&table_to_blocks(&t, TextSeparator::Tabs, None)), ["a\tb", "c\td"]);
        assert_eq!(texts(&table_to_blocks(&t, TextSeparator::Commas, None)), ["a,b", "c,d"]);
        assert_eq!(texts(&table_to_blocks(&t, TextSeparator::Other, Some('|'))), ["a|b", "c|d"]);
        assert_eq!(texts(&table_to_blocks(&t, TextSeparator::Paragraphs, None)), ["a", "b", "c", "d"]);
    }
}
