//! HTML tables to Word tables: widths, spans, borders, shading, header rows.

use super::convert::{Conv, Ctx, Run};
use super::css::{self, get, Decls};
use super::dom::{HEl, HNode};
use crate::xml::Element;

/// The width of the text area, in twips, when a table doesn't say.
const TEXT_WIDTH: i64 = 9360;

struct Cell<'a> {
    el: &'a HEl,
    cs: Decls,
    colspan: usize,
    rowspan: usize,
    header: bool,
}

#[derive(Clone, Copy)]
enum Width {
    Twips(i64),
    Percent(f64),
}

fn width_of(e: &HEl, cs: &Decls) -> Option<Width> {
    let v = get(cs, "width").or_else(|| e.attr("width"))?.trim();
    if v == "auto" {
        return None;
    }
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse::<f64>().ok().map(Width::Percent);
    }
    css::length_twips(v, 11.0, None).filter(|w| *w > 0).map(Width::Twips)
}

fn border_el(name: &'static str, b: &css::Bd) -> Element {
    Element::new(name).with_attr("w:val", b.val).with_attr("w:sz", b.sz.to_string()).with_attr("w:space", "0").with_attr("w:color", &b.color)
}

fn none_el(name: &'static str) -> Element {
    Element::new(name).with_attr("w:val", "nil")
}

impl Conv<'_> {
    pub(super) fn table(&mut self, e: &HEl, cs: &Decls, cx: &Ctx, rc: &Run, out: &mut Vec<Element>) {
        self.pending = None;
        // Rows, with the ones in <thead> marked.
        let mut rows: Vec<(&HEl, bool)> = Vec::new();
        let mut caption: Option<&HEl> = None;
        fn collect<'a>(nodes: &'a [HNode], head: bool, rows: &mut Vec<(&'a HEl, bool)>, caption: &mut Option<&'a HEl>) {
            for n in nodes {
                let HNode::El(k) = n else { continue };
                match k.tag.as_str() {
                    "tr" => rows.push((k, head)),
                    "thead" => collect(&k.children, true, rows, caption),
                    "tbody" | "tfoot" => collect(&k.children, false, rows, caption),
                    "caption" => *caption = Some(k),
                    _ => {}
                }
            }
        }
        collect(&e.children, false, &mut rows, &mut caption);
        if let Some(c) = caption {
            let cs2 = self.sheet.computed(c);
            self.block_el(c, &cs2, cx, rc, out);
        }
        if rows.is_empty() {
            return;
        }
        let table_border = get(cs, "border").and_then(css::parse_border).or_else(|| e.attr("border").and_then(|b| b.trim().parse::<f64>().ok()).filter(|b| *b > 0.0).map(|b| css::Bd { val: "single", sz: ((b * 6.0).round() as i64).clamp(2, 96), color: "auto".into() }));

        // Cells, with the grid they sit in.
        let mut grid_rows: Vec<Vec<(Cell, usize)>> = Vec::new(); // (cell, start column)
        let mut occupied: Vec<(usize, usize)> = Vec::new(); // per column: rows still covered, span of the cell that started there (0 = covered by a neighbour)
        let mut continuation: Vec<Vec<(usize, usize)>> = Vec::new(); // per row: (start column, span) of cells continued from above
        for (row, _) in &rows {
            let mut col = 0usize;
            let mut cells = Vec::new();
            let mut cont = Vec::new();
            let cell_els: Vec<&HEl> = row.children.iter().filter_map(|n| if let HNode::El(k) = n { Some(k) } else { None }).filter(|k| k.tag == "td" || k.tag == "th").collect();
            let mut ci = 0;
            loop {
                while col < occupied.len() && occupied[col].0 > 0 {
                    let span = occupied[col].1.max(1);
                    cont.push((col, span));
                    for k in col..(col + span).min(occupied.len()) {
                        occupied[k].0 -= 1;
                    }
                    col += span;
                }
                let Some(c) = cell_els.get(ci) else { break };
                ci += 1;
                let colspan = c.attr("colspan").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(1).clamp(1, 64);
                let rowspan = c.attr("rowspan").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(1).clamp(1, 500);
                let ccs = self.sheet.computed(c);
                if occupied.len() < col + colspan {
                    occupied.resize(col + colspan, (0, 0));
                }
                if rowspan > 1 {
                    for k in 0..colspan {
                        occupied[col + k] = (rowspan - 1, if k == 0 { colspan } else { 0 });
                    }
                }
                cells.push((Cell { el: c, cs: ccs, colspan, rowspan, header: c.tag == "th" }, col));
                col += colspan;
            }
            // Columns still covered from above at the end of the row.
            while col < occupied.len() {
                if occupied[col].0 > 0 {
                    let span = occupied[col].1.max(1);
                    cont.push((col, span));
                    for k in col..(col + span).min(occupied.len()) {
                        occupied[k].0 -= 1;
                    }
                    col += span;
                } else {
                    col += 1;
                }
            }
            grid_rows.push(cells);
            continuation.push(cont);
        }
        let ncols = grid_rows
            .iter()
            .zip(&continuation)
            .map(|(r, c)| r.iter().map(|(cell, s)| s + cell.colspan).chain(c.iter().map(|(s, n)| s + n)).max().unwrap_or(0))
            .max()
            .unwrap_or(1)
            .max(1);

        // Widths.
        let table_w = width_of(e, cs);
        let total = match table_w {
            Some(Width::Twips(t)) => t,
            Some(Width::Percent(p)) => (TEXT_WIDTH as f64 * p / 100.0).round() as i64,
            None => TEXT_WIDTH,
        };
        let mut col_w: Vec<Option<i64>> = vec![None; ncols];
        let col_els: Vec<&HEl> = e.children.iter().filter_map(|n| if let HNode::El(k) = n { Some(k) } else { None }).flat_map(|k| if k.tag == "colgroup" { k.children.iter().filter_map(|n| if let HNode::El(c) = n { Some(c) } else { None }).collect::<Vec<_>>() } else { vec![k] }).filter(|k| k.tag == "col").collect();
        let mut ci = 0;
        for c in &col_els {
            let span = c.attr("span").and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).max(1);
            let ccs = self.sheet.computed(c);
            if let Some(w) = width_of(c, &ccs) {
                for k in 0..span {
                    if ci + k < ncols {
                        col_w[ci + k] = Some(to_twips(w, total));
                    }
                }
            }
            ci += span;
        }
        for r in &grid_rows {
            for (cell, start) in r {
                if cell.colspan == 1 && col_w[*start].is_none() {
                    if let Some(w) = width_of(cell.el, &cell.cs) {
                        col_w[*start] = Some(to_twips(w, total));
                    }
                }
            }
        }
        let known: i64 = col_w.iter().flatten().sum();
        let unknown = col_w.iter().filter(|w| w.is_none()).count() as i64;
        let fill = if unknown > 0 { ((total - known) / unknown).max(720) } else { 0 };
        let widths: Vec<i64> = col_w.iter().map(|w| w.unwrap_or(fill)).collect();

        // The table itself.
        let mut tblpr = Element::new("w:tblPr");
        if table_border.is_some() {
            if let Some(id) = self.b.builtin_style("Table Grid") {
                tblpr = tblpr.with_child(Element::new("w:tblStyle").with_attr("w:val", id));
            }
        }
        tblpr = match table_w {
            Some(Width::Percent(p)) => tblpr.with_child(Element::new("w:tblW").with_attr("w:w", ((p * 50.0).round() as i64).to_string()).with_attr("w:type", "pct")),
            Some(Width::Twips(t)) => tblpr.with_child(Element::new("w:tblW").with_attr("w:w", t.to_string()).with_attr("w:type", "dxa")),
            None => tblpr.with_child(Element::new("w:tblW").with_attr("w:w", "0").with_attr("w:type", "auto")),
        };
        let talign = match e.attr("align").or_else(|| get(cs, "margin-left").filter(|v| *v == "auto" && get(cs, "margin-right") == Some("auto")).map(|_| "center")) {
            Some("center") => Some("center"),
            Some("right") => Some("right"),
            _ => None,
        };
        if let Some(a) = talign {
            tblpr = tblpr.with_child(Element::new("w:jc").with_attr("w:val", a));
        }
        if let Some(b) = &table_border {
            let mut bd = Element::new("w:tblBorders");
            for n in ["w:top", "w:left", "w:bottom", "w:right", "w:insideH", "w:insideV"] {
                bd = bd.with_child(border_el(n, b));
            }
            tblpr = tblpr.with_child(bd);
        }
        if let Some(pad) = e.attr("cellpadding").and_then(|v| v.trim().parse::<f64>().ok()) {
            let tw = (pad * 15.0).round() as i64;
            let mut mar = Element::new("w:tblCellMar");
            for (n, v) in [("w:top", tw), ("w:left", tw.max(60)), ("w:bottom", tw), ("w:right", tw.max(60))] {
                mar = mar.with_child(Element::new(n).with_attr("w:w", v.to_string()).with_attr("w:type", "dxa"));
            }
            tblpr = tblpr.with_child(mar);
        }
        tblpr = tblpr.with_child(Element::new("w:tblLook").with_attr("w:val", "04A0").with_attr("w:firstRow", "1").with_attr("w:lastRow", "0").with_attr("w:firstColumn", "1").with_attr("w:lastColumn", "0").with_attr("w:noHBand", "0").with_attr("w:noVBand", "1"));
        let mut tbl = Element::new("w:tbl").with_child(tblpr);
        let mut grid = Element::new("w:tblGrid");
        for w in &widths {
            grid = grid.with_child(Element::new("w:gridCol").with_attr("w:w", w.to_string()));
        }
        tbl = tbl.with_child(grid);

        let span_w = |start: usize, span: usize| -> i64 { widths.iter().skip(start).take(span).sum::<i64>().max(1) };
        let mut header_rows = true;
        for (ri, ((row_el, in_head), cells)) in rows.iter().zip(grid_rows).enumerate() {
            let mut tr = Element::new("w:tr");
            let all_th = !cells.is_empty() && cells.iter().all(|(c, _)| c.header);
            let is_header = *in_head || (header_rows && all_th);
            if !is_header {
                header_rows = false;
            }
            let rcs = self.sheet.computed(row_el);
            let mut trpr = Element::new("w:trPr");
            if let Some(h) = get(&rcs, "height").or_else(|| row_el.attr("height")).and_then(|v| css::length_twips(v, 11.0, None)).filter(|h| *h > 0) {
                trpr = trpr.with_child(Element::new("w:trHeight").with_attr("w:val", h.to_string()).with_attr("w:hRule", "atLeast"));
            }
            if is_header {
                trpr = trpr.with_child(Element::new("w:tblHeader"));
            }
            if !trpr.children.is_empty() {
                tr = tr.with_child(trpr);
            }
            // Cells in column order, with the continuation cells of spans from above where they belong.
            enum Slot<'x> {
                Cell(Cell<'x>, usize),
                Cont(usize, usize),
            }
            let mut slots: Vec<(usize, Slot)> = cells.into_iter().map(|(c, s)| (s, Slot::Cell(c, s))).collect();
            for (s, n) in &continuation[ri] {
                slots.push((*s, Slot::Cont(*s, *n)));
            }
            slots.sort_by_key(|(s, _)| *s);
            for (_, slot) in slots {
                match slot {
                    Slot::Cont(start, span) => {
                        let mut pr = Element::new("w:tcPr").with_child(Element::new("w:tcW").with_attr("w:w", span_w(start, span).to_string()).with_attr("w:type", "dxa"));
                        if span > 1 {
                            pr = pr.with_child(Element::new("w:gridSpan").with_attr("w:val", span.to_string()));
                        }
                        pr = pr.with_child(Element::new("w:vMerge"));
                        tr = tr.with_child(Element::new("w:tc").with_child(pr).with_child(Element::new("w:p")));
                    }
                    Slot::Cell(cell, start) => {
                        let tc = self.cell(&cell, start, span_w(start, cell.colspan), cx, rc);
                        tr = tr.with_child(tc);
                    }
                }
            }
            tbl = tbl.with_child(tr);
        }
        out.push(tbl);
        // Word wants a paragraph between two tables, and the editor wants one after.
        out.push(Element::new("w:p"));
    }

    fn cell(&mut self, cell: &Cell, _start: usize, width: i64, cx: &Ctx, rc: &Run) -> Element {
        let cs = &cell.cs;
        let mut pr = Element::new("w:tcPr").with_child(Element::new("w:tcW").with_attr("w:w", width.to_string()).with_attr("w:type", "dxa"));
        if cell.colspan > 1 {
            pr = pr.with_child(Element::new("w:gridSpan").with_attr("w:val", cell.colspan.to_string()));
        }
        if cell.rowspan > 1 {
            pr = pr.with_child(Element::new("w:vMerge").with_attr("w:val", "restart"));
        }
        // Borders: the cell's own, per side. A table border already covers the cells.
        let mut bd = Element::new("w:tcBorders");
        for (name, side) in [("w:top", 0usize), ("w:left", 3), ("w:bottom", 2), ("w:right", 1)] {
            if let Some(v) = css::side(cs, "border", side) {
                bd = match css::parse_border(&v) {
                    Some(b) => bd.with_child(border_el(name, &b)),
                    None => bd.with_child(none_el(name)),
                };
            }
        }
        if cs.iter().all(|(k, _)| !k.starts_with("border")) {
            if let Some(b) = cell.el.attr("border").and_then(|v| v.parse::<f64>().ok()).filter(|b| *b > 0.0) {
                let b = css::Bd { val: "single", sz: ((b * 6.0).round() as i64).clamp(2, 96), color: "auto".into() };
                for n in ["w:top", "w:left", "w:bottom", "w:right"] {
                    bd = bd.with_child(border_el(n, &b));
                }
            }
        }
        if !bd.children.is_empty() {
            pr = pr.with_child(bd);
        }
        let bg = cell.el.attr("bgcolor").and_then(css::parse_color).or_else(|| get(cs, "background-color").or_else(|| get(cs, "background")).and_then(|v| v.split_whitespace().find_map(css::parse_color)));
        if let Some(bg) = bg.filter(|b| !b.eq_ignore_ascii_case("FFFFFF")) {
            pr = pr.with_child(Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", bg));
        }
        if get(cs, "white-space") == Some("nowrap") || cell.el.attr("nowrap").is_some() {
            pr = pr.with_child(Element::new("w:noWrap"));
        }
        let va = get(cs, "vertical-align").or_else(|| cell.el.attr("valign")).unwrap_or("");
        match va {
            "middle" | "center" => pr = pr.with_child(Element::new("w:vAlign").with_attr("w:val", "center")),
            "bottom" => pr = pr.with_child(Element::new("w:vAlign").with_attr("w:val", "bottom")),
            _ => {}
        }
        // Contents. A header cell is bold and centred unless it says otherwise.
        let mut ccx = cx.clone();
        ccx.indent = 0;
        ccx.indent_right = 0;
        ccx.lists.clear();
        ccx.align = None;
        if cell.header {
            ccx.align = Some("center");
        }
        if let Some(a) = get(cs, "text-align").or_else(|| cell.el.attr("align")) {
            ccx.align = match a {
                "center" => Some("center"),
                "right" | "end" => Some("right"),
                "justify" => Some("both"),
                "left" | "start" => Some("left"),
                _ => ccx.align,
            };
        }
        let crc = self.run_from(cell.el, cs, rc);
        let mut blocks = Vec::new();
        self.blocks(&cell.el.children, &ccx, &crc, &mut blocks);
        if blocks.last().is_none_or(|b| b.name != "w:p") {
            blocks.push(Element::new("w:p"));
        }
        let mut tc = Element::new("w:tc").with_child(pr);
        for b in blocks {
            tc = tc.with_child(b);
        }
        tc
    }
}

fn to_twips(w: Width, total: i64) -> i64 {
    match w {
        Width::Twips(t) => t,
        Width::Percent(p) => (total as f64 * p / 100.0).round() as i64,
    }
}
