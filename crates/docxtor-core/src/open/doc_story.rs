//! The text of a legacy .doc as blocks: paragraphs, runs, fields, notes, pictures and tables. A child of
//! `doc_binary`, which holds the file structures this reads.

use super::*;

/// A field that has begun and not ended.
struct Field {
    instr: String,
    in_result: bool,
    /// Where the result starts among the paragraph's nodes, and the paragraph it started in.
    mark: usize,
    serial: usize,
}

/// A finished row: its cells (each a list of blocks) and the row's `sprmTDefTable` operand.
type RowData = (Vec<Vec<Element>>, Option<Vec<u8>>);

/// A table being collected: finished rows, and the cells of the row in progress.
#[derive(Default)]
struct TableAcc {
    rows: Vec<RowData>,
    cells: Vec<Vec<Element>>,
    cell: Vec<Element>,
}

impl TableAcc {
    fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.cells.is_empty() && self.cell.is_empty()
    }
}

struct Border {
    style: &'static str,
    sz: u8,
    color: String,
}

#[derive(Default, Clone)]
struct CellDef {
    left: i32,
    right: i32,
    vmerge: u8,
    valign: u8,
    borders: [Option<(&'static str, u8, String)>; 4],
}

impl<'a> Reader<'a> {
    /// Which formatting run covers a character (by address and length, to notice a change) and its sprms.
    fn chp_key(&self, fc: Option<usize>) -> (usize, usize) {
        let g = fc.and_then(|f| self.chp.find(f)).unwrap_or(&[]);
        (g.as_ptr() as usize, g.len())
    }

    /// Blocks for the characters in `[start, end)`.
    pub(super) fn story(&mut self, start: usize, end: usize) -> Vec<Element> {
        let end = end.min(self.text.len());
        let mut out: Vec<Element> = Vec::new();
        let mut tbl = TableAcc::default();
        let mut serial = 0;
        let mut cp = start;
        while cp < end {
            let mut mark = cp;
            while mark < end && !matches!(self.text[mark], 0x0D | 0x07) {
                mark += 1;
            }
            let mark_ch = self.text.get(mark).copied().unwrap_or(0x0D);
            let (p, pap) = self.paragraph(cp, mark.min(end), &mut serial);
            cp = mark + 1;
            if pap.in_table {
                if mark_ch == 0x07 && pap.ttp {
                    if !tbl.cell.is_empty() {
                        let c = std::mem::take(&mut tbl.cell);
                        tbl.cells.push(c);
                    }
                    let cells = std::mem::take(&mut tbl.cells);
                    tbl.rows.push((cells, pap.tdef.clone()));
                } else if mark_ch == 0x07 {
                    tbl.cell.push(p);
                    let c = std::mem::take(&mut tbl.cell);
                    tbl.cells.push(c);
                } else {
                    tbl.cell.push(p);
                }
            } else {
                if !tbl.is_empty() {
                    out.push(self.table(std::mem::take(&mut tbl)));
                }
                out.push(p);
            }
        }
        if !tbl.is_empty() {
            out.push(self.table(tbl));
        }
        out
    }

    fn paragraph(&mut self, cp0: usize, mark: usize, serial: &mut usize) -> (Element, Pap) {
        *serial += 1;
        let fc_mark = self.fc_of(mark);
        let direct: Vec<u8> = fc_mark.and_then(|fc| self.pap.find(fc)).map(|g| g.to_vec()).unwrap_or_default();
        let istd = if direct.len() >= 2 { u16_at(&direct, 0) } else { 0 };
        let mapped = self.word_style(istd);
        let mut pap = if mapped.is_some() { Pap::default() } else { self.style_pap(istd, 0) };
        let mut dp = Pap::default();
        dp.apply(direct.get(2..).unwrap_or(&[]));
        pap.overlay(&dp);
        pap.istd = istd;
        pap.in_table = dp.in_table;
        pap.ttp = dp.ttp;
        pap.tdef = dp.tdef.clone();

        let base = if mapped.is_some() { Chp::default() } else { self.style_chp(istd, 0) };
        let mut nodes: Vec<Node> = Vec::new();
        self.inline(cp0, mark, &base, *serial, &mut nodes);

        let mut ppr = Element::new("w:pPr");
        if let Some(id) = &mapped {
            put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", id));
        }
        let num = pap.ilfo.and_then(|i| self.list_num(i));
        if let Some(n) = num {
            put_ppr(&mut ppr, Element::new("w:numPr").with_child(Element::new("w:ilvl").with_attr("w:val", pap.ilvl.unwrap_or(0).min(8).to_string())).with_child(Element::new("w:numId").with_attr("w:val", n.to_string())));
        }
        if pap.keep_next == Some(true) {
            put_ppr(&mut ppr, Element::new("w:keepNext"));
        }
        if pap.keep_lines == Some(true) {
            put_ppr(&mut ppr, Element::new("w:keepLines"));
        }
        if pap.page_break == Some(true) {
            put_ppr(&mut ppr, Element::new("w:pageBreakBefore"));
        }
        if let Some(fill) = &pap.shade {
            put_ppr(&mut ppr, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", fill));
        }
        if !pap.tabs.is_empty() {
            let mut tabs = Element::new("w:tabs");
            for t in &pap.tabs {
                let val = ["left", "center", "right", "decimal", "bar"].get(t.kind as usize).copied().unwrap_or("left");
                let mut e = Element::new("w:tab").with_attr("w:val", val);
                if let Some(l) = [None, Some("dot"), Some("hyphen"), Some("underscore")].get(t.leader as usize).copied().flatten() {
                    e.set_attr("w:leader", l);
                }
                e.set_attr("w:pos", t.pos.to_string());
                tabs.children.push(Node::Element(e));
            }
            put_ppr(&mut ppr, tabs);
        }
        let mut spacing = Element::new("w:spacing");
        if let Some(v) = pap.before {
            spacing.set_attr("w:before", v.to_string());
        }
        if let Some(v) = pap.after {
            spacing.set_attr("w:after", v.to_string());
        }
        if let Some((line, mult)) = pap.line {
            if mult {
                spacing.set_attr("w:line", line.max(1).to_string());
                spacing.set_attr("w:lineRule", "auto");
            } else if line > 0 {
                spacing.set_attr("w:line", line.to_string());
                spacing.set_attr("w:lineRule", "atLeast");
            } else if line < 0 {
                spacing.set_attr("w:line", (-line).to_string());
                spacing.set_attr("w:lineRule", "exact");
            }
        }
        if !spacing.attrs.is_empty() {
            put_ppr(&mut ppr, spacing);
        }
        if num.is_none() {
            let mut ind = Element::new("w:ind");
            if let Some(v) = pap.left.filter(|v| *v != 0) {
                ind.set_attr("w:left", v.to_string());
            }
            if let Some(v) = pap.right.filter(|v| *v != 0) {
                ind.set_attr("w:right", v.to_string());
            }
            if let Some(v) = pap.first.filter(|v| *v != 0) {
                ind.set_attr(if v < 0 { "w:hanging" } else { "w:firstLine" }, v.abs().to_string());
            }
            if !ind.attrs.is_empty() {
                put_ppr(&mut ppr, ind);
            }
        }
        if let Some(jc) = pap.jc {
            let v = match jc {
                1 => Some("center"),
                2 => Some("right"),
                3 => Some("both"),
                4 => Some("distribute"),
                _ => None,
            };
            if let Some(v) = v {
                put_ppr(&mut ppr, Element::new("w:jc").with_attr("w:val", v));
            }
        }
        let mut p = Element::new("w:p");
        if !ppr.children.is_empty() {
            p.children.push(Node::Element(ppr));
        }
        p.children.extend(nodes);
        (p, pap)
    }

    fn rpr(&mut self, c: &Chp, cstyle: Option<usize>) -> Element {
        let mut r = Element::new("w:rPr");
        if let Some(id) = cstyle.and_then(|i| self.char_style_id(i)) {
            put_rpr(&mut r, Element::new("w:rStyle").with_attr("w:val", id));
        }
        if let Some(f) = c.font.and_then(|f| self.font_name(f)).map(String::from) {
            put_rpr(&mut r, Element::new("w:rFonts").with_attr("w:ascii", f.as_str()).with_attr("w:hAnsi", f.as_str()).with_attr("w:cs", f.as_str()));
        }
        let flag = |r: &mut Element, v: Option<bool>, name: &'static str| {
            if v == Some(true) {
                put_rpr(r, Element::new(name));
            }
        };
        flag(&mut r, c.bold, "w:b");
        flag(&mut r, c.italic, "w:i");
        flag(&mut r, c.caps, "w:caps");
        flag(&mut r, c.small_caps, "w:smallCaps");
        flag(&mut r, c.strike, "w:strike");
        flag(&mut r, c.hidden, "w:vanish");
        if let Some(col) = &c.color {
            put_rpr(&mut r, Element::new("w:color").with_attr("w:val", col));
        }
        if let Some(sz) = c.size.filter(|s| *s >= 2) {
            put_rpr(&mut r, Element::new("w:sz").with_attr("w:val", sz.to_string()));
            put_rpr(&mut r, Element::new("w:szCs").with_attr("w:val", sz.to_string()));
        }
        if let Some(h) = c.highlight.and_then(|h| HIGHLIGHT.get(h as usize)) {
            put_rpr(&mut r, Element::new("w:highlight").with_attr("w:val", *h));
        }
        if let Some(u) = c.underline.filter(|u| *u > 0) {
            let val = match u {
                2 => "words",
                3 => "double",
                4 => "dotted",
                6 => "thick",
                7 => "dash",
                9 => "dotDash",
                10 => "dotDotDash",
                11 => "wave",
                _ => "single",
            };
            put_rpr(&mut r, Element::new("w:u").with_attr("w:val", val));
        }
        if let Some(v) = c.vert {
            put_rpr(&mut r, Element::new("w:vertAlign").with_attr("w:val", if v == 1 { "superscript" } else { "subscript" }));
        }
        r
    }

    fn make_run(&mut self, c: &Chp, cstyle: Option<usize>, content: Vec<Element>) -> Element {
        let rpr = self.rpr(c, cstyle);
        let mut r = Element::new("w:r");
        if !rpr.children.is_empty() {
            r.children.push(Node::Element(rpr));
        }
        r.children.extend(content.into_iter().map(Node::Element));
        r
    }

    /// The characters of one paragraph as runs, fields and pictures.
    fn inline(&mut self, cp0: usize, cp1: usize, base: &Chp, serial: usize, nodes: &mut Vec<Node>) {
        let mut fields: Vec<Field> = Vec::new();
        let mut text = String::new();
        let mut cur: Option<(Chp, Option<usize>)> = None;
        let mut last_ptr: Option<(usize, usize)> = None;
        let mut eff = base.clone();
        let mut cstyle: Option<usize> = None;

        macro_rules! flush {
            () => {
                if !text.is_empty() {
                    if let Some((c, cs)) = &cur {
                        let run = self.make_run(c, *cs, vec![Element::new("w:t").with_attr("xml:space", "preserve").with_text(std::mem::take(&mut text))]);
                        nodes.push(Node::Element(run));
                    }
                    text.clear();
                }
            };
        }

        for cp in cp0..cp1 {
            let ch = self.text[cp];
            let fc = self.fc_of(cp);
            let key = self.chp_key(fc);
            if last_ptr != Some(key) {
                last_ptr = Some(key);
                let g: Vec<u8> = fc.and_then(|f| self.chp.find(f)).map(|g| g.to_vec()).unwrap_or_default();
                let mut direct = Chp::default();
                direct.apply(&g);
                eff = base.clone();
                cstyle = direct.istd.filter(|i| *i != 0);
                if let Some(cs) = cstyle {
                    let sc = self.style_chp(cs, 0);
                    eff.overlay(&sc);
                }
                // Direct formatting acts on the style's values (a toggle can flip them).
                let mut d = Chp { bold: eff.bold, italic: eff.italic, strike: eff.strike, caps: eff.caps, small_caps: eff.small_caps, hidden: eff.hidden, ..Default::default() };
                d.apply(&g);
                eff.overlay(&d);
                eff.deleted = d.deleted;
                eff.special = d.special;
                eff.pic = d.pic;
                eff.symbol = d.symbol;
            }
            if eff.deleted {
                continue;
            }
            match ch {
                0x13 => {
                    flush!();
                    fields.push(Field { instr: String::new(), in_result: false, mark: nodes.len(), serial });
                    continue;
                }
                0x14 => {
                    flush!();
                    if let Some(f) = fields.last_mut() {
                        f.in_result = true;
                        f.mark = nodes.len();
                        f.serial = serial;
                    }
                    continue;
                }
                0x15 => {
                    flush!();
                    if let Some(f) = fields.pop() {
                        self.finish_field(f, nodes, serial);
                    }
                    continue;
                }
                _ => {}
            }
            if let Some(f) = fields.last_mut().filter(|f| !f.in_result) {
                if let Some(c) = char::from_u32(ch as u32).filter(|c| !c.is_control()) {
                    f.instr.push(c);
                }
                continue;
            }
            let same = matches!(&cur, Some((c, cs)) if *c == eff && *cs == cstyle);
            let mut special: Option<Vec<Element>> = None;
            let mut plain: Option<char> = None;
            match ch {
                0x01 if eff.special => {
                    if let Some(loc) = eff.pic {
                        match self.picture(loc) {
                            Some(d) => special = Some(vec![d]),
                            None => self.lost_pictures += 1,
                        }
                    }
                }
                0x01 | 0x05 | 0x00 | 0x0A => {}
                0x02 => {
                    if let Some(r) = self.note_reference(cp) {
                        flush!();
                        nodes.push(Node::Element(r));
                    }
                }
                0x08 => self.lost_shapes += 1,
                0x09 => special = Some(vec![Element::new("w:tab")]),
                0x0B => special = Some(vec![Element::new("w:br")]),
                0x0C => special = Some(vec![Element::new("w:br").with_attr("w:type", "page")]),
                0x0E => special = Some(vec![Element::new("w:br").with_attr("w:type", "column")]),
                0x1E => plain = Some('\u{2011}'),
                0x1F => plain = Some('\u{00AD}'),
                u if u < 0x20 => {}
                u => {
                    if eff.special {
                        if let Some((ftc, code)) = eff.symbol {
                            if let Some(font) = self.font_name(ftc).map(String::from) {
                                special = Some(vec![Element::new("w:sym").with_attr("w:font", font).with_attr("w:char", format!("{code:04X}"))]);
                            }
                        }
                    }
                    if special.is_none() {
                        plain = char::from_u32(u as u32);
                        // Surrogate pairs arrive as two units.
                        if (0xD800..0xDC00).contains(&u) {
                            if let Some(&lo) = self.text.get(cp + 1).filter(|l| (0xDC00..0xE000).contains(*l)) {
                                plain = char::decode_utf16([u, lo]).next().and_then(|r| r.ok());
                            }
                        } else if (0xDC00..0xE000).contains(&u) {
                            plain = None;
                        }
                    }
                }
            }
            if let Some(content) = special {
                flush!();
                let run = self.make_run(&eff, cstyle, content);
                nodes.push(Node::Element(run));
            } else if let Some(c) = plain {
                if !same {
                    flush!();
                    cur = Some((eff.clone(), cstyle));
                }
                text.push(c);
            }
        }
        flush!();
        while let Some(f) = fields.pop() {
            self.finish_field(f, nodes, serial);
        }
    }

    fn finish_field(&mut self, f: Field, nodes: &mut Vec<Node>, serial: usize) {
        if !f.in_result || f.serial != serial || f.mark > nodes.len() {
            return;
        }
        let instr = f.instr.trim();
        let kind = instr.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        let result = nodes.split_off(f.mark);
        match kind.as_str() {
            "HYPERLINK" => {
                let quoted: Vec<&str> = instr.split('"').skip(1).step_by(2).collect();
                let mut h = Element::new("w:hyperlink");
                if instr.contains("\\l") {
                    if let Some(a) = quoted.last() {
                        h.set_attr("w:anchor", *a);
                    }
                } else if let Some(url) = quoted.first() {
                    let key = self.b.link(url);
                    h.set_attr("r:id", key);
                } else {
                    nodes.extend(result);
                    return;
                }
                h.children = result;
                nodes.push(Node::Element(h));
            }
            "PAGE" | "NUMPAGES" | "NUMWORDS" | "NUMCHARS" | "DATE" | "TIME" | "CREATEDATE" | "SAVEDATE" | "PRINTDATE" | "AUTHOR" | "TITLE" | "SUBJECT" | "FILENAME" | "SECTION" | "SECTIONPAGES" => {
                let mut e = Element::new("w:fldSimple").with_attr("w:instr", format!(" {instr} "));
                e.children = result;
                nodes.push(Node::Element(e));
            }
            _ => nodes.extend(result),
        }
    }

    // -- notes ----------------------------------------------------------------

    fn note_reference(&mut self, cp: usize) -> Option<Element> {
        let (end, idx) = if let Some(i) = self.note_refs.iter().position(|c| *c == cp) {
            (false, i)
        } else if let Some(i) = self.end_refs.iter().position(|c| *c == cp) {
            (true, i)
        } else {
            return None;
        };
        let key = format!("{}{}", if end { "e" } else { "n" }, idx + 1);
        if !self.notes.iter().any(|(k, _)| *k == key) {
            let (a, b) = if end { self.end_ranges.get(idx).copied()? } else { self.note_ranges.get(idx).copied()? };
            let _ = self.b.builtin_style(if end { "Endnote Reference" } else { "Footnote Reference" });
            let _ = self.b.builtin_style(if end { "Endnote Text" } else { "Footnote Text" });
            // The key is taken first, so a damaged file whose note points into itself doesn't go round for ever.
            let kind = if end { "endnote" } else { "footnote" };
            self.notes.push((key.clone(), FragNote { kind: kind.into(), xml: String::new() }));
            let mut paras = self.note_story(a, b, end);
            if paras.is_empty() {
                paras.push(Element::new("w:p"));
            }
            let mut note = Element::new(if end { "w:endnote" } else { "w:footnote" }).with_attr("w:id", key.as_str());
            note.children = paras.into_iter().map(Node::Element).collect();
            if let Some(slot) = self.notes.iter_mut().find(|(k, _)| *k == key) {
                slot.1.xml = note.to_xml();
            }
        }
        let style = if end { "EndnoteReference" } else { "FootnoteReference" };
        Some(Element::new("w:r").with_child(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", style))).with_child(Element::new(if end { "w:endnoteReference" } else { "w:footnoteReference" }).with_attr("w:id", key)))
    }

    /// A note's paragraphs. Its own number mark (character 0x02, which no reference names) becomes the
    /// number reference Word shows before the text.
    fn note_story(&mut self, a: usize, b: usize, end: bool) -> Vec<Element> {
        let mut paras = self.story(a, b);
        let style = if end { "EndnoteReference" } else { "FootnoteReference" };
        let mark = Element::new("w:r").with_child(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", style))).with_child(Element::new(if end { "w:endnoteRef" } else { "w:footnoteRef" }));
        if let Some(first) = paras.iter_mut().find(|p| p.name == "w:p") {
            let at = first.children.iter().position(|n| !matches!(n, Node::Element(e) if e.name == "w:pPr")).unwrap_or(first.children.len());
            first.children.insert(at, Node::Element(mark));
            first.children.insert(at + 1, Node::Element(Element::new("w:r").with_child(Element::new("w:t").with_attr("xml:space", "preserve").with_text(" "))));
        }
        paras
    }

    // -- pictures -------------------------------------------------------------

    fn picture(&mut self, loc: usize) -> Option<Element> {
        let d = self.data.as_ref()?;
        let lcb = u32_at(d, loc);
        let cb_header = u16_at(d, loc + 4);
        let mm = u16_at(d, loc + 6);
        let mut start = loc + cb_header;
        if mm == 0x66 {
            start += 1 + d.get(start).copied().unwrap_or(0) as usize;
        }
        let region = d.get(start..(loc + lcb).min(d.len()))?;
        let bytes = find_blip(region, 0)?;
        let (_, pw, ph) = sniff_image(&bytes)?;
        let (gw, gh) = (u16_at(d, loc + 28) as f64, u16_at(d, loc + 30) as f64);
        let (mx, my) = (u16_at(d, loc + 32) as f64, u16_at(d, loc + 34) as f64);
        let (w, h) = if gw > 0.0 && gh > 0.0 && mx > 0.0 && my > 0.0 { (gw * mx / 1000.0 / 20.0, gh * my / 1000.0 / 20.0) } else { (pw as f64 * 0.75, ph as f64 * 0.75) };
        let key = self.b.media(&bytes)?;
        let id = self.b.drawing_id();
        Some(inline_drawing(&key, w.max(1.0), h.max(1.0), id, ""))
    }

    // -- tables ---------------------------------------------------------------

    fn table(&mut self, t: TableAcc) -> Element {
        let mut t = t;
        if !t.cell.is_empty() {
            let c = std::mem::take(&mut t.cell);
            t.cells.push(c);
        }
        if !t.cells.is_empty() {
            let cells = std::mem::take(&mut t.cells);
            t.rows.push((cells, None));
        }
        // Each row's cells with their edges.
        let mut rows: Vec<(Vec<CellDef>, Vec<Vec<Element>>)> = Vec::new();
        for (cells, tdef) in t.rows {
            let defs = tdef.as_deref().map(parse_tdef).unwrap_or_default();
            rows.push((defs, cells));
        }
        // Grid: every distinct edge of every row, measured from the row's own left edge.
        let mut edges: Vec<i32> = vec![0];
        let mut fallback_total = 0;
        for (defs, _) in &rows {
            if let (Some(f), Some(l)) = (defs.first(), defs.last()) {
                fallback_total = fallback_total.max(l.right - f.left);
                for d in defs {
                    edges.push(d.left - f.left);
                    edges.push(d.right - f.left);
                }
            }
        }
        if fallback_total == 0 {
            fallback_total = 8640;
        }
        edges.sort();
        edges.dedup_by(|a, b| (*a - *b).abs() < 20);
        if edges.len() < 2 {
            edges = vec![0, fallback_total];
        }
        let ncols = edges.len() - 1;
        let col_at = |x: i32| -> usize { edges.iter().enumerate().min_by_key(|(_, e)| (**e - x).abs()).map(|(i, _)| i).unwrap_or(0) };

        let mut tbl = Element::new("w:tbl");
        let total = edges[ncols] - edges[0];
        tbl.children.push(Node::Element(
            Element::new("w:tblPr")
                .with_child(Element::new("w:tblW").with_attr("w:w", total.to_string()).with_attr("w:type", "dxa"))
                .with_child(Element::new("w:tblLayout").with_attr("w:type", "fixed"))
                .with_child(Element::new("w:tblLook").with_attr("w:val", "0000")),
        ));
        let mut grid = Element::new("w:tblGrid");
        for i in 0..ncols {
            grid.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", (edges[i + 1] - edges[i]).to_string())));
        }
        tbl.children.push(Node::Element(grid));
        for (defs, cells) in rows {
            let mut tr = Element::new("w:tr");
            let origin = defs.first().map(|d| d.left).unwrap_or(0);
            let mut col = 0usize;
            for (i, content) in cells.into_iter().enumerate() {
                let (c0, c1) = match defs.get(i) {
                    Some(d) => (col_at(d.left - origin), col_at(d.right - origin)),
                    None => (col.min(ncols - 1), ncols),
                };
                let c0 = c0.max(col.min(ncols - 1)).min(ncols - 1);
                let c1 = c1.max(c0 + 1).min(ncols);
                let mut tcpr = Element::new("w:tcPr");
                let w = edges[c1] - edges[c0];
                tcpr.children.push(Node::Element(Element::new("w:tcW").with_attr("w:w", w.to_string()).with_attr("w:type", "dxa")));
                if c1 - c0 > 1 {
                    tcpr.children.push(Node::Element(Element::new("w:gridSpan").with_attr("w:val", (c1 - c0).to_string())));
                }
                let d = defs.get(i).cloned().unwrap_or_default();
                match d.vmerge {
                    1 => tcpr.children.push(Node::Element(Element::new("w:vMerge"))),
                    2 => tcpr.children.push(Node::Element(Element::new("w:vMerge").with_attr("w:val", "restart"))),
                    _ => {}
                }
                let mut bd = Element::new("w:tcBorders");
                for (k, name) in ["w:top", "w:left", "w:bottom", "w:right"].iter().enumerate() {
                    if let Some((style, sz, color)) = &d.borders[k] {
                        bd.children.push(Node::Element(Element::new(*name).with_attr("w:val", *style).with_attr("w:sz", sz.to_string()).with_attr("w:space", "0").with_attr("w:color", color.as_str())));
                    }
                }
                if !bd.children.is_empty() {
                    tcpr.children.push(Node::Element(bd));
                }
                match d.valign {
                    1 => tcpr.children.push(Node::Element(Element::new("w:vAlign").with_attr("w:val", "center"))),
                    2 => tcpr.children.push(Node::Element(Element::new("w:vAlign").with_attr("w:val", "bottom"))),
                    _ => {}
                }
                let mut tc = Element::new("w:tc");
                tc.children.push(Node::Element(sorted_tcpr(tcpr)));
                let mut content = content;
                if content.is_empty() {
                    content.push(Element::new("w:p"));
                }
                tc.children.extend(content.into_iter().map(Node::Element));
                tr.children.push(Node::Element(tc));
                col = c1;
            }
            if tr.elements().any(|c| c.name == "w:tc") {
                tbl.children.push(Node::Element(tr));
            }
        }
        tbl
    }
}

fn sorted_tcpr(src: Element) -> Element {
    let mut out = Element::new("w:tcPr");
    for c in src.elements() {
        put_ordered(&mut out, c.clone(), TCPR_ORDER);
    }
    out
}

/// The cells a table row defines (`sprmTDefTable`): edges, merges, borders.
fn parse_tdef(t: &[u8]) -> Vec<CellDef> {
    let n = t.first().copied().unwrap_or(0) as usize;
    let centers: Vec<i32> = (0..=n).map(|i| i16_at(t, 1 + i * 2)).collect();
    let tc0 = 1 + (n + 1) * 2;
    let mut cells: Vec<CellDef> = Vec::new();
    let mut open: Option<usize> = None;
    for i in 0..n {
        let tc = t.get(tc0 + i * 20..tc0 + i * 20 + 20);
        let grf = tc.map(|b| u16_at(b, 0)).unwrap_or(0);
        let (first_merged, merged) = (grf & 1 != 0, grf & 2 != 0);
        if merged && !first_merged {
            if let Some(k) = open {
                cells[k].right = centers[i + 1];
                continue;
            }
        }
        let mut d = CellDef { left: centers[i], right: centers[i + 1], ..Default::default() };
        d.vmerge = if grf & 0x40 != 0 {
            2
        } else if grf & 0x20 != 0 {
            1
        } else {
            0
        };
        d.valign = ((grf >> 7) & 3) as u8;
        if let Some(b) = tc {
            for k in 0..4 {
                if let Some(brc) = brc80(&b[4 + k * 4..8 + k * 4]) {
                    d.borders[k] = Some((brc.style, brc.sz, brc.color));
                }
            }
        }
        open = if first_merged { Some(cells.len()) } else { None };
        cells.push(d);
    }
    cells
}

fn brc80(b: &[u8]) -> Option<Border> {
    let (width, kind, ico) = (b[0], b[1], b[2] as usize);
    if width == 0 || kind == 0 || kind == 0xFF {
        return None;
    }
    let style = match kind {
        3 => "double",
        6 => "dotted",
        7 => "dashed",
        8 => "dotDash",
        9 => "dotDotDash",
        10 => "triple",
        _ => "single",
    };
    let color = ICO.get(ico).filter(|c| !c.is_empty()).map(|c| c.to_string()).unwrap_or_else(|| "auto".into());
    Some(Border { style, sz: width.clamp(2, 96), color })
}

/// The first JPEG or PNG picture in some OfficeArt records.
fn find_blip(d: &[u8], depth: usize) -> Option<Vec<u8>> {
    if depth > 6 {
        return None;
    }
    let mut p = 0;
    while p + 8 <= d.len() {
        let vi = u16_at(d, p);
        let ty = u16_at(d, p + 2);
        let len = u32_at(d, p + 4);
        let body = p + 8;
        let end = (body + len).min(d.len());
        if vi & 0xF == 0xF && (0xF000..0xF008).contains(&ty) {
            if let Some(b) = find_blip(&d[body..end], depth + 1) {
                return Some(b);
            }
        } else if ty == 0xF007 {
            // A picture entry: a header, an optional name, then the picture record.
            let cb_name = d.get(body + 33).copied().unwrap_or(0) as usize;
            if let Some(b) = d.get(body + 36 + cb_name..end).and_then(|s| find_blip(s, depth + 1)) {
                return Some(b);
            }
        } else if ty == 0xF01D || ty == 0xF02A || ty == 0xF01E {
            let inst = vi >> 4;
            let two = matches!(inst, 0x46B | 0x6E3 | 0x6E1);
            let data = body + 16 + if two { 16 } else { 0 } + 1;
            if let Some(s) = d.get(data..end) {
                return Some(s.to_vec());
            }
        }
        p = end.max(p + 8);
    }
    None
}
