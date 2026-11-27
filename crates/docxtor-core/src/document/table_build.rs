//! Building tables and the table op, with the helpers that find the table around a paragraph.

use super::*;

impl Document {
    pub(super) fn build_table(&mut self, cells: Vec<Vec<Vec<Inline>>>, header: bool) -> Result<Table, EditError> {
        let style = self.styles.ensure_builtin("TableGrid");
        let cols = cells.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
        let width = self.text_width();
        let colw = width / cols as i64;
        let mut tblpr = Element::new("w:tblPr");
        if let Some(s) = style {
            tblpr.children.push(Node::Element(Element::new("w:tblStyle").with_attr("w:val", s)));
        }
        tblpr.children.push(Node::Element(Element::new("w:tblW").with_attr("w:w", "0").with_attr("w:type", "auto")));
        tblpr.children.push(Node::Element(
            Element::new("w:tblLook")
                .with_attr("w:val", "04A0")
                .with_attr("w:firstRow", "1")
                .with_attr("w:lastRow", "0")
                .with_attr("w:firstColumn", "1")
                .with_attr("w:lastColumn", "0")
                .with_attr("w:noHBand", "0")
                .with_attr("w:noVBand", "1"),
        ));
        let mut grid = Element::new("w:tblGrid");
        for _ in 0..cols {
            grid.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", colw.to_string())));
        }
        let id = self.ids.fresh();
        let mut children = vec![TableChild::Other(tblpr), TableChild::Other(grid)];
        for (ri, row) in cells.into_iter().enumerate() {
            let mut r = Row { attrs: vec![], children: vec![], sdts: vec![] };
            if header && ri == 0 {
                r.children.push(RowChild::Other(Element::new("w:trPr").with_child(Element::new("w:tblHeader"))));
            }
            let n = row.len();
            for (ci, inl) in row.into_iter().chain((n..cols).map(|_| Vec::new())).enumerate() {
                let _ = ci;
                let mut p = Paragraph::new(self.ids.fresh());
                p.inlines = inl;
                if header && ri == 0 {
                    let len = p.len16();
                    edit::format_range(&mut p.inlines, 0, len, &mut |run| {
                        RunPatch { bold: Some(true), ..Default::default() }.apply(run.rpr.get_or_insert_with(|| Element::new("w:rPr")), &RunFmt::default());
                    });
                }
                let tcpr = Element::new("w:tcPr").with_child(Element::new("w:tcW").with_attr("w:w", colw.to_string()).with_attr("w:type", "dxa"));
                r.children.push(RowChild::Cell(Cell { attrs: vec![], tcpr: Some(tcpr), blocks: vec![Block::Para(p)], sdts: vec![] }));
            }
            children.push(TableChild::Row(r));
        }
        Ok(Table { id, attrs: vec![], children })
    }

    pub(super) fn op_table(&mut self, pid: NodeId, action: TableAction) -> Result<OpResult, EditError> {
        self.touch(pid);
        let ids = &mut self.ids;
        let mut caret = None;
        let mut delete_table: Option<NodeId> = None;
        let found = with_table_body(&mut self.body, pid, &mut |t: &mut Table, ri: usize, ci: usize| {
            let row_positions: Vec<usize> = t.children.iter().enumerate().filter(|(_, c)| matches!(c, TableChild::Row(_))).map(|(i, _)| i).collect();
            let row_at = row_positions[ri];
            let grid_col = {
                let TableChild::Row(r) = &t.children[row_at] else { unreachable!() };
                r.cells().take(ci).map(cell_span).sum::<i64>()
            };
            match action {
                TableAction::RowAbove | TableAction::RowBelow => {
                    let TableChild::Row(src) = &t.children[row_at] else { unreachable!() };
                    let mut nr = Row { attrs: vec![], children: vec![], sdts: src.sdts.clone() };
                    for c in &src.children {
                        match c {
                            RowChild::Other(e) if e.name == "w:trPr" => {
                                let mut e = e.clone();
                                e.remove_children("w:tblHeader");
                                nr.children.push(RowChild::Other(e));
                            }
                            RowChild::Other(_) | RowChild::Marker(..) => {}
                            RowChild::Cell(cell) => {
                                let mut tcpr = cell.tcpr.clone();
                                if let Some(tp) = tcpr.as_mut() {
                                    tp.remove_children("w:vMerge");
                                }
                                let mut p = Paragraph::new(ids.fresh());
                                if let Some(Block::Para(src_p)) = cell.blocks.first() {
                                    p.ppr = src_p.ppr.clone();
                                }
                                nr.children.push(RowChild::Cell(Cell { attrs: vec![], tcpr, blocks: vec![Block::Para(p)], sdts: cell.sdts.clone() }));
                            }
                        }
                    }
                    let target = nr.cells().nth(ci).and_then(|c| first_para_blocks(&c.blocks));
                    caret = target.map(|p| Pos::new(p, 0));
                    let at = if action == TableAction::RowAbove { row_at } else { row_at + 1 };
                    t.children.insert(at, TableChild::Row(nr));
                }
                TableAction::DeleteRow => {
                    t.children.remove(row_at);
                    if t.rows().count() == 0 {
                        delete_table = Some(t.id);
                    } else {
                        let r = t.rows().nth(ri.min(t.rows().count() - 1)).unwrap();
                        caret = r.cells().next().and_then(|c| first_para_blocks(&c.blocks)).map(|p| Pos::new(p, 0));
                    }
                }
                TableAction::ColLeft | TableAction::ColRight => {
                    let insert_at = if action == TableAction::ColLeft { grid_col } else { grid_col + {
                        let TableChild::Row(r) = &t.children[row_at] else { unreachable!() };
                        r.cells().nth(ci).map(cell_span).unwrap_or(1)
                    } };
                    let mut grid = t.grid();
                    let w = grid.get(grid_col as usize).copied().unwrap_or(1440);
                    grid.insert((insert_at as usize).min(grid.len()), w);
                    for r in t.rows_mut() {
                        // find child index for grid column `insert_at`
                        let mut g = 0;
                        let mut idx = None;
                        let mut template = None;
                        let mut straddle = None;
                        for (k, c) in r.children.iter().enumerate() {
                            if let RowChild::Cell(cell) = c {
                                let span = cell_span(cell);
                                if g < insert_at && insert_at < g + span {
                                    straddle = Some((k, span));
                                }
                                if g >= insert_at && idx.is_none() {
                                    idx = Some(k);
                                }
                                if template.is_none() || g <= insert_at {
                                    template = Some(cell.clone());
                                }
                                g += span;
                            }
                        }
                        // A merged cell spanning the insertion point just grows by one column.
                        if let Some((k, span)) = straddle {
                            if let RowChild::Cell(cell) = &mut r.children[k] {
                                let tp = cell.tcpr.get_or_insert_with(|| Element::new("w:tcPr"));
                                crate::props::put_ordered(tp, Element::new("w:gridSpan").with_attr("w:val", (span + 1).to_string()), &["w:cnfStyle", "w:tcW", "w:gridSpan"]);
                            }
                            continue;
                        }
                        let idx = idx.unwrap_or(r.children.len());
                        let mut tcpr = template.and_then(|c| c.tcpr).unwrap_or_else(|| Element::new("w:tcPr"));
                        tcpr.remove_children("w:gridSpan");
                        tcpr.remove_children("w:vMerge");
                        let p = Paragraph::new(ids.fresh());
                        if caret.is_none() {
                            caret = Some(Pos::new(p.id, 0));
                        }
                        // Between two cells that sit in the same content control, the new one sits in it too.
                        let sdts = match (r.children[..idx].iter().rev().find_map(|c| if let RowChild::Cell(c) = c { Some(c) } else { None }), r.children.get(idx)) {
                            (Some(left), Some(RowChild::Cell(right))) => left.sdts.iter().zip(&right.sdts).take_while(|(l, r)| l.id == r.id).map(|(l, _)| l.clone()).collect(),
                            _ => Vec::new(),
                        };
                        r.children.insert(idx, RowChild::Cell(Cell { attrs: vec![], tcpr: Some(tcpr), blocks: vec![Block::Para(p)], sdts }));
                    }
                    set_grid(t, grid);
                }
                TableAction::DeleteCol => {
                    let mut grid = t.grid();
                    if (grid_col as usize) < grid.len() {
                        grid.remove(grid_col as usize);
                    }
                    let mut empty = true;
                    for r in t.rows_mut() {
                        let mut g = 0;
                        let mut remove = None;
                        for (k, c) in r.children.iter_mut().enumerate() {
                            if let RowChild::Cell(cell) = c {
                                let span = cell_span(cell);
                                if g <= grid_col && grid_col < g + span {
                                    if span > 1 {
                                        if let Some(tp) = cell.tcpr.as_mut() {
                                            tp.remove_children("w:gridSpan");
                                            if span - 1 > 1 {
                                                crate::props::put_ordered(tp, Element::new("w:gridSpan").with_attr("w:val", (span - 1).to_string()), &["w:cnfStyle", "w:tcW", "w:gridSpan"]);
                                            }
                                        }
                                    } else {
                                        remove = Some(k);
                                    }
                                    break;
                                }
                                g += span;
                            }
                        }
                        if let Some(k) = remove {
                            r.children.remove(k);
                        }
                        if r.cells().count() > 0 {
                            empty = false;
                        }
                    }
                    if empty || grid.is_empty() {
                        delete_table = Some(t.id);
                    } else {
                        set_grid(t, grid);
                        let r = t.rows().nth(ri).unwrap();
                        caret = r.cells().nth(ci.saturating_sub(1)).or(r.cells().next()).and_then(|c| first_para_blocks(&c.blocks)).map(|p| Pos::new(p, 0));
                    }
                }
                TableAction::DeleteTable => delete_table = Some(t.id),
            }
        });
        if found.is_none() {
            return Err(EditError::Invalid(format!("paragraph {pid} is not inside a table")));
        }
        if let Some(tid) = delete_table {
            let next = {
                let (mut list, i) = self.body.parent_mut(tid).ok_or(EditError::NotFound(tid))?;
                list.remove(i);
                list.get(i).or(list.get(i.wrapping_sub(1))).map(|b| b.id())
            };
            if let Some(n) = next {
                self.touch(n);
                caret = find_para(&self.body, n).map(|p| Pos::new(p.id, 0));
            }
            self.ensure_nonempty_body();
        }
        Ok(OpResult { caret, ..Default::default() })
    }
}

pub(super) fn cell_span(c: &Cell) -> i64 {
    c.tcpr.as_ref().and_then(|p| p.child("w:gridSpan")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(1)
}

pub(super) fn first_para_blocks(blocks: &[Block]) -> Option<NodeId> {
    let mut first = None;
    walk_paragraphs(blocks, &mut |p| {
        first.get_or_insert(p.id);
    });
    first
}

pub(super) fn first_para_in(t: &Table) -> Option<NodeId> {
    t.rows().next().and_then(|r| r.cells().next()).and_then(|c| first_para_blocks(&c.blocks))
}

/// Replace the grid, keeping the table's total width, and sync cell widths.
pub(super) fn set_grid(t: &mut Table, mut grid: Vec<i64>) {
    let old_total: i64 = t.grid().iter().sum();
    let new_total: i64 = grid.iter().sum();
    if old_total > 0 && new_total > 0 {
        for g in grid.iter_mut() {
            *g = *g * old_total / new_total;
        }
    }
    for c in t.children.iter_mut() {
        if let TableChild::Other(e) = c {
            if e.name == "w:tblGrid" {
                e.children.retain(|n| !matches!(n, Node::Element(x) if x.name == "w:gridCol"));
                for w in &grid {
                    e.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", w.to_string())));
                }
            }
        }
    }
    for r in t.rows_mut() {
        let mut g = 0usize;
        for cell in r.cells_mut() {
            let span = cell_span(cell) as usize;
            let w: i64 = grid.iter().skip(g).take(span).sum();
            g += span;
            let tcpr = cell.tcpr.get_or_insert_with(|| Element::new("w:tcPr"));
            crate::props::put_ordered(
                tcpr,
                Element::new("w:tcW").with_attr("w:w", w.to_string()).with_attr("w:type", "dxa"),
                &["w:cnfStyle", "w:tcW", "w:gridSpan", "w:hMerge", "w:vMerge", "w:tcBorders", "w:shd", "w:noWrap", "w:tcMar", "w:textDirection", "w:tcFitText", "w:vAlign", "w:hideMark"],
            );
        }
    }
}

/// `with_table` on the body: only the top-level block that holds `pid` is copied from an undo step that shares it.
pub(super) fn with_table_body<R>(body: &mut Body, pid: NodeId, f: &mut dyn FnMut(&mut Table, usize, usize) -> R) -> Option<R> {
    let t = body.iter().position(|b| find_block(std::slice::from_ref(b), pid).is_some())?;
    with_table(std::slice::from_mut(&mut body[t]), pid, f)
}

/// Run `f` on the innermost table containing paragraph `pid`, with (row, cell) indices.
pub(super) fn with_table<R>(blocks: &mut [Block], pid: NodeId, f: &mut dyn FnMut(&mut Table, usize, usize) -> R) -> Option<R> {
    for b in blocks.iter_mut() {
        match b {
            Block::Table(t) => {
                let mut hit = None;
                'rows: for (ri, row) in t.rows_mut().enumerate() {
                    for (ci, cell) in row.cells_mut().enumerate() {
                        if let Some(r) = with_table(&mut cell.blocks, pid, f) {
                            return Some(r);
                        }
                        if find_block(&cell.blocks, pid).is_some() {
                            hit = Some((ri, ci));
                            break 'rows;
                        }
                    }
                }
                if let Some((ri, ci)) = hit {
                    return Some(f(t, ri, ci));
                }
            }
            Block::Container(c) => {
                if let Some(r) = with_table(&mut c.blocks, pid, f) {
                    return Some(r);
                }
            }
            _ => {}
        }
    }
    None
}
