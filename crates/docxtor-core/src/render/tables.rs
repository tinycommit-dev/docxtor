//! Rendering tables.

use super::*;

impl<'a> RenderCtx<'a> {
    pub fn table(&mut self, t: &Table) -> RTable {
        use crate::table_style::{is_nil, CellPos, Sides, TableStyler};
        let tblpr_themed = t.tblpr().map(|e| self.themed(e));
        let tblpr = tblpr_themed.as_ref();
        let style_id = tblpr.and_then(|p| p.child("w:tblStyle")).and_then(|s| s.attr("w:val")).map(String::from);
        let mut borders = TableBorders::default();
        let mut cell_mar = [0, 108, 0, 108];
        let apply_mar = |el: &Element, mar: &mut [i64; 4]| {
            if let Some(m) = el.child("w:tblCellMar") {
                let w = |n: &str| m.child(n).and_then(|e| e.attr("w:w")).and_then(|v| v.parse().ok());
                if let Some(v) = w("w:top") {
                    mar[0] = v;
                }
                if let Some(v) = w("w:right").or(w("w:end")) {
                    mar[1] = v;
                }
                if let Some(v) = w("w:bottom") {
                    mar[2] = v;
                }
                if let Some(v) = w("w:left").or(w("w:start")) {
                    mar[3] = v;
                }
            }
        };
        if let Some(sid) = &style_id {
            for p in self.styles.table_style_props(sid) {
                if let Some(b) = p.child("w:tblBorders") {
                    borders.apply(b);
                }
                apply_mar(p, &mut cell_mar);
            }
        }
        if let Some(p) = tblpr {
            if let Some(b) = p.child("w:tblBorders") {
                borders.apply(b);
            }
            apply_mar(p, &mut cell_mar);
        }
        let align = tblpr.and_then(|p| p.child("w:jc")).and_then(|j| j.attr("w:val")).map(String::from);
        let indent = tblpr.and_then(|p| p.child("w:tblInd")).and_then(|j| j.attr("w:w")).and_then(|v| v.parse().ok());
        let tblw = tblpr.and_then(|p| p.child("w:tblW"));
        let width_pct = tblw.and_then(|w| w.attr("w:type")) == Some("pct");
        let width = tblw.filter(|w| matches!(w.attr("w:type"), Some("dxa") | Some("pct"))).and_then(|w| {
            let v = w.attr("w:w")?;
            if let Some(p) = v.strip_suffix('%') {
                p.parse::<f64>().ok().map(|x| (x * 50.0) as i64)
            } else {
                v.parse().ok()
            }
        });
        let spacing = tblpr
            .and_then(|p| p.child("w:tblCellSpacing"))
            .filter(|s| matches!(s.attr("w:type"), None | Some("dxa")))
            .and_then(|s| s.attr("w:w"))
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0);
        let fixed = tblpr.and_then(|p| p.child("w:tblLayout")).and_then(|l| l.attr("w:type")) == Some("fixed");
        let float = tblpr.and_then(|p| p.child("w:tblpPr")).map(|f| {
            let n = |k: &str| f.attr(k).and_then(|v| v.parse::<i64>().ok());
            RTableFloat {
                horz_anchor: f.attr("w:horzAnchor").unwrap_or("text").to_string(),
                vert_anchor: f.attr("w:vertAnchor").unwrap_or("text").to_string(),
                x: n("w:tblpX"),
                y: n("w:tblpY"),
                x_align: f.attr("w:tblpXSpec").map(String::from),
                y_align: f.attr("w:tblpYSpec").map(String::from),
                left: n("w:leftFromText").unwrap_or(0),
                right: n("w:rightFromText").unwrap_or(0),
                top: n("w:topFromText").unwrap_or(0),
                bottom: n("w:bottomFromText").unwrap_or(0),
                never_overlap: tblpr.and_then(|p| p.child("w:tblOverlap")).and_then(|o| o.attr("w:val")) == Some("never"),
            }
        });

        let styler = TableStyler::new(self.styles, style_id.as_deref(), tblpr);
        let grid = t.grid();
        let rows_total = t.rows().count();
        let grid_cols = grid.len().max(t.rows().map(|r| r.cells().map(cell_span).sum::<usize>()).max().unwrap_or(0));
        let outer_text = self.table_text.take();
        let trpr_of = |r: &Row| {
            r.children.iter().find_map(|c| match c {
                RowChild::Other(e) if e.name == "w:trPr" => Some(e.clone()),
                _ => None,
            })
        };
        let mut rows = Vec::new();
        for (ri, r) in t.rows().enumerate() {
            let trpr = trpr_of(r);
            let trpr = trpr.as_ref();
            let height_el = trpr.and_then(|p| p.child("w:trHeight"));
            let height = height_el.and_then(|h| h.attr("w:val")).and_then(|v| v.parse().ok());
            let height_rule = height_el.and_then(|h| h.attr("w:hRule")).filter(|r| matches!(*r, "exact" | "atLeast")).map(String::from);
            let header = trpr.is_some_and(|p| p.child("w:tblHeader").is_some());
            let cant_split = trpr.is_some_and(|p| p.child("w:cantSplit").is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off"))));
            let grid_before = trpr.and_then(|p| p.child("w:gridBefore")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(0);
            let grid_after = trpr.and_then(|p| p.child("w:gridAfter")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(0);
            let row_cnf = trpr.and_then(|p| p.child("w:cnfStyle"));
            let ncells = r.cells().count();
            let mut cells: Vec<RCell> = Vec::new();
            let mut g = grid_before.max(0) as usize;
            let mut merge_into: Option<usize> = None;
            for (ci, c) in r.cells().enumerate() {
                let tcpr_themed = c.tcpr.as_ref().map(|e| self.themed(e));
                let tcpr = tcpr_themed.as_ref();
                let span = cell_span(c);
                let vmerge = tcpr.and_then(|p| p.child("w:vMerge")).map(|v| v.attr("w:val").unwrap_or("continue").to_string());
                let hmerge = tcpr.and_then(|p| p.child("w:hMerge")).map(|v| v.attr("w:val").unwrap_or("continue").to_string());
                let width = tcpr
                    .and_then(|p| p.child("w:tcW"))
                    .filter(|w| w.attr("w:type") != Some("pct") && w.attr("w:type") != Some("auto"))
                    .and_then(|w| w.attr("w:w"))
                    .and_then(|v| v.parse().ok());
                let pos = CellPos { row: ri, rows: rows_total, col: ci, cols: ncells, grid_from: g, grid_to: g + span, grid_cols };
                let cs = styler.cell(&pos, tcpr, tcpr.and_then(|p| p.child("w:cnfStyle")).or(row_cnf));
                let sides = |s: &Sides| TableBorders { top: s.top.clone(), bottom: s.bottom.clone(), left: s.left.clone(), right: s.right.clone(), inside_h: None, inside_v: None };
                let real = |b: &Option<Border>| b.clone().filter(|b| !is_nil(b));
                self.table_text = Some((cs.para.clone(), cs.run.clone()));
                let mar = cs.mar.iter().any(|m| m.is_some()).then(|| {
                    // The table's margins fill in the sides a cell does not set.
                    [cs.mar[0].unwrap_or(cell_mar[0]), cs.mar[1].unwrap_or(cell_mar[1]), cs.mar[2].unwrap_or(cell_mar[2]), cs.mar[3].unwrap_or(cell_mar[3])]
                });
                if hmerge.as_deref() == Some("continue") {
                    if let Some(first) = merge_into {
                        // A cell joined to the one before it is not drawn: that one is as wide as both.
                        cells[first].span += span as i64;
                        g += span;
                        continue;
                    }
                } else if hmerge.is_some() {
                    merge_into = Some(cells.len());
                } else {
                    merge_into = None;
                }
                cells.push(RCell {
                    span: span as i64,
                    vmerge,
                    width,
                    shading: cs.shading.clone(),
                    valign: cs.valign.clone(),
                    mar,
                    diag_down: real(&cs.borders.tl2br),
                    diag_up: real(&cs.borders.tr2bl),
                    sdt: sdt_label(&c.sdts),
                    text_dir: tcpr.and_then(|p| p.child("w:textDirection")).and_then(|d| d.attr("w:val")).and_then(|d| match d {
                        "tbRl" | "tbRlV" => Some("tbRl".to_string()),
                        "btLr" => Some("btLr".to_string()),
                        _ => None,
                    }),
                    no_wrap: tcpr.and_then(|p| p.child("w:noWrap")).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off"))),
                    borders: sides(&cs.borders),
                    blocks: self.blocks(&c.blocks),
                });
                g += span;
            }
            let (control, section) = row_controls(&r.sdts);
            rows.push(RRow { height, height_rule, header, cant_split, grid_before, grid_after, sdt: sdt_label(&r.sdts), control, section, cells });
        }
        self.table_text = outer_text;
        let look = styler.look().map(|l| RTableLook { first_row: l.first_row, last_row: l.last_row, first_column: l.first_col, last_column: l.last_col, banded_rows: l.h_band, banded_columns: l.v_band });
        let style_name = style_id.as_deref().and_then(|s| self.styles.get(s)).map(|s| s.name.clone());
        let bidi_visual = tblpr.and_then(|p| p.child("w:bidiVisual")).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")));
        RTable { id: t.id, grid, borders, align, indent, width, width_pct, cell_mar, spacing, fixed, float, style: style_id, style_name, look, bidi_visual, rows, chunk: None }
    }
}

pub(super) fn cell_span(c: &Cell) -> usize {
    c.tcpr.as_ref().and_then(|p| p.child("w:gridSpan")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(1).max(1)
}

/// The control around a row (its innermost one) and the repeating section it is an item of, if any.
pub(super) fn row_controls(wraps: &[SdtWrap]) -> (Option<crate::controls::RControl>, Option<crate::controls::RControl>) {
    let view = |w: &SdtWrap| {
        w.shell.child("w:sdtPr").map(|p| {
            let mut v = crate::controls::read_props(p);
            v.ctl = Some(w.id);
            v
        })
    };
    let inner = wraps.last().and_then(view);
    let section = match (&inner, wraps.len()) {
        (Some(i), n) if i.kind == "repeatingItem" && n >= 2 => view(&wraps[n - 2]).filter(|s| s.kind == "repeating"),
        _ => None,
    };
    (inner, section)
}

/// What to call the content control around a row or cell: its alias, else its tag, else nothing (the editor
/// then says "Content control").
pub(super) fn sdt_label(wraps: &[SdtWrap]) -> Option<String> {
    let w = wraps.last()?;
    let pr = w.shell.child("w:sdtPr");
    let name = pr.and_then(|p| p.child("w:alias").or(p.child("w:tag"))).and_then(|a| a.attr("w:val")).unwrap_or("");
    Some(name.to_string())
}
