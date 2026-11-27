//! Tables: collecting rows and cells and building the table element.

use super::*;
use crate::xml::{Element, Node};
use super::state::*;

impl Interp {
    pub(super) fn flush_table(&mut self) {
        let rows = std::mem::take(&mut self.story().tbl.rows);
        if rows.is_empty() {
            return;
        }
        if let Some(el) = self.build_table(rows) {
            self.story().blocks.push(el);
        }
    }

    pub(super) fn end_cell(&mut self) {
        if self.story().cur.is_some() {
            self.end_para(EndKind::Cell);
        }
        let st = self.story();
        let blocks = std::mem::take(&mut st.tbl.cell_blocks);
        if st.tbl.cells.len() < MAX_CELLS * 2 {
            st.tbl.cells.push(blocks);
        }
        st.tbl.row_open = true;
    }

    pub(super) fn end_row(&mut self) {
        if self.story().cur.is_some() || !self.story().tbl.cell_blocks.is_empty() {
            self.end_cell();
        }
        let st = self.story();
        let cells = std::mem::take(&mut st.tbl.cells);
        let def = st.tbl.def.clone();
        st.tbl.row_open = false;
        if cells.is_empty() && def.cells.is_empty() {
            return;
        }
        st.tbl.rows.push(RowOut { def, cells });
        self.g().pf.intbl = false;
    }

    pub(super) fn border_el(&self, tag: &'static str, b: &Brd) -> Element {
        let mut e = Element::new(tag).with_attr("w:val", b.style);
        if b.style != "nil" {
            let sz = (b.w * 2 / 5).clamp(2, 96);
            e.set_attr("w:sz", sz.to_string());
            e.set_attr("w:space", (b.space / 20).clamp(0, 31).to_string());
            e.set_attr("w:color", b.color.and_then(|c| self.color_hex(c)).unwrap_or_else(|| "auto".into()));
        }
        e
    }

    pub(super) fn shd_el(&self, fill: &str) -> Element {
        Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", fill)
    }

    pub(super) fn build_table(&self, rows: Vec<RowOut>) -> Option<Element> {
        struct TCell {
            left: i64,
            right: i64,
            def: CellDef,
            blocks: Vec<Element>,
        }
        let mut norm: Vec<(RowDef, Vec<TCell>)> = Vec::new();
        for row in rows {
            let RowOut { def, cells } = row;
            let n = def.cells.len().max(cells.len()).min(MAX_CELLS * 2);
            let mut out: Vec<TCell> = Vec::new();
            let mut prev = def.left;
            let mut it = cells.into_iter();
            for i in 0..n {
                let d = def.cells.get(i).cloned().unwrap_or_else(|| CellDef { right: prev + 1440, ..Default::default() });
                let right = d.right.max(prev + 20);
                let blocks = it.next().unwrap_or_default();
                if d.hmerge == 2 {
                    if let Some(last) = out.last_mut() {
                        last.right = right;
                        prev = right;
                        continue;
                    }
                }
                out.push(TCell { left: prev, right, def: d, blocks });
                prev = right;
            }
            if !out.is_empty() {
                norm.push((def, out));
            }
        }
        if norm.is_empty() {
            return None;
        }
        let mut all: Vec<i64> = norm.iter().flat_map(|(_, cs)| cs.iter().flat_map(|c| [c.left, c.right])).collect();
        all.sort_unstable();
        let mut bounds: Vec<i64> = Vec::new();
        for v in all {
            if bounds.last().is_none_or(|l| v - *l > 2) {
                bounds.push(v);
            }
        }
        if bounds.len() < 2 {
            let b0 = bounds.first().copied().unwrap_or(0);
            bounds = vec![b0, b0 + 1440];
        }
        let idx = |v: i64| -> usize {
            let p = bounds.partition_point(|x| *x < v);
            if p == 0 {
                0
            } else if p >= bounds.len() {
                bounds.len() - 1
            } else if bounds[p] - v < v - bounds[p - 1] {
                p
            } else {
                p - 1
            }
        };
        let ncols = bounds.len() - 1;
        let first = &norm[0].0;
        let total = bounds[ncols] - bounds[0];

        let mut pr = Element::new("w:tblPr");
        pr.children.push(Node::Element(Element::new("w:tblW").with_attr("w:w", total.to_string()).with_attr("w:type", "dxa")));
        if let Some(j) = first.jc.filter(|j| *j != "left") {
            pr.children.push(Node::Element(Element::new("w:jc").with_attr("w:val", j)));
        }
        let gap = first.gaph.unwrap_or(108);
        let ind = bounds[0] + gap;
        if ind != 0 {
            pr.children.push(Node::Element(Element::new("w:tblInd").with_attr("w:w", ind.to_string()).with_attr("w:type", "dxa")));
        }
        if first.borders.iter().any(|b| b.is_some()) {
            let mut tb = Element::new("w:tblBorders");
            for (i, tag) in ["w:top", "w:left", "w:bottom", "w:right", "w:insideH", "w:insideV"].iter().enumerate() {
                if let Some(b) = &first.borders[i] {
                    tb.children.push(Node::Element(self.border_el(tag, b)));
                }
            }
            pr.children.push(Node::Element(tb));
        }
        if gap != 108 {
            let mar = Element::new("w:tblCellMar")
                .with_child(Element::new("w:left").with_attr("w:w", gap.max(0).to_string()).with_attr("w:type", "dxa"))
                .with_child(Element::new("w:right").with_attr("w:w", gap.max(0).to_string()).with_attr("w:type", "dxa"));
            pr.children.push(Node::Element(mar));
        }

        let mut tbl = Element::new("w:tbl").with_child(pr);
        let mut grid = Element::new("w:tblGrid");
        for i in 0..ncols {
            grid.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", (bounds[i + 1] - bounds[i]).to_string())));
        }
        tbl.children.push(Node::Element(grid));

        for (def, cells) in norm {
            let mut tr = Element::new("w:tr");
            let before = idx(cells[0].left);
            let after = ncols.saturating_sub(idx(cells[cells.len() - 1].right));
            let mut trpr = Element::new("w:trPr");
            if before > 0 {
                trpr.children.push(Node::Element(Element::new("w:gridBefore").with_attr("w:val", before.to_string())));
            }
            if after > 0 {
                trpr.children.push(Node::Element(Element::new("w:gridAfter").with_attr("w:val", after.to_string())));
            }
            if def.keep {
                trpr.children.push(Node::Element(Element::new("w:cantSplit")));
            }
            if def.height != 0 {
                trpr.children.push(Node::Element(
                    Element::new("w:trHeight").with_attr("w:val", def.height.abs().to_string()).with_attr("w:hRule", if def.height < 0 { "exact" } else { "atLeast" }),
                ));
            }
            if def.header {
                trpr.children.push(Node::Element(Element::new("w:tblHeader")));
            }
            if !trpr.children.is_empty() {
                tr.children.push(Node::Element(trpr));
            }
            for c in cells {
                let il = idx(c.left);
                let ir = idx(c.right).max(il + 1);
                let mut pr = Element::new("w:tcPr");
                pr.children.push(Node::Element(Element::new("w:tcW").with_attr("w:w", (c.right - c.left).to_string()).with_attr("w:type", "dxa")));
                if ir - il > 1 {
                    pr.children.push(Node::Element(Element::new("w:gridSpan").with_attr("w:val", (ir - il).to_string())));
                }
                match c.def.vmerge {
                    1 => pr.children.push(Node::Element(Element::new("w:vMerge").with_attr("w:val", "restart"))),
                    2 => pr.children.push(Node::Element(Element::new("w:vMerge"))),
                    _ => {}
                }
                if c.def.borders.iter().any(|b| b.is_some()) {
                    let mut tb = Element::new("w:tcBorders");
                    for (i, tag) in ["w:top", "w:left", "w:bottom", "w:right"].iter().enumerate() {
                        if let Some(b) = &c.def.borders[i] {
                            tb.children.push(Node::Element(self.border_el(tag, b)));
                        }
                    }
                    pr.children.push(Node::Element(tb));
                }
                if let Some(fill) = c.def.shd.and_then(|i| self.color_hex(i)) {
                    pr.children.push(Node::Element(self.shd_el(&fill)));
                }
                if let Some(v) = c.def.valign.filter(|v| *v != "top") {
                    pr.children.push(Node::Element(Element::new("w:vAlign").with_attr("w:val", v)));
                }
                let mut tc = Element::new("w:tc").with_child(pr);
                let mut blocks = c.blocks;
                if !matches!(blocks.last(), Some(e) if e.name == "w:p") {
                    blocks.push(Element::new("w:p"));
                }
                tc.children.extend(blocks.into_iter().map(Node::Element));
                tr.children.push(Node::Element(tc));
            }
            tbl.children.push(Node::Element(tr));
        }
        Some(tbl)
    }

    // -----------------------------------------------------------------------
    // Notes and pictures
    // -----------------------------------------------------------------------
}
