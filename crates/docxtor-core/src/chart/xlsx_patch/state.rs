//! The planned writes and the workbook being patched: loading its parts, planning each cell and writing the result.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum NewVal {
    Gap,
    Num(f64),
    /// A name or category: written as a number when the cell (or its neighbour) holds numbers and the text is
    /// one, else as text.
    Label(String),
}

impl NewVal {
    pub(super) fn norm(self) -> NewVal {
        match self {
            NewVal::Label(s) if s.is_empty() => NewVal::Gap,
            NewVal::Num(x) if !x.is_finite() => NewVal::Gap,
            v => v,
        }
    }
}

pub(super) fn parse_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

#[derive(Debug)]
pub(super) struct Put {
    pub(super) val: NewVal,
    /// For a cell that doesn't exist yet: the style to give it.
    pub(super) style: Option<String>,
    /// Labels: the cell or its neighbour holds a number.
    pub(super) numeric: bool,
    /// Table headers are always text.
    pub(super) force_text: bool,
}

impl Put {
    /// The number this write puts in the cell, if it writes one.
    pub(super) fn number(&self) -> Option<f64> {
        match &self.val {
            NewVal::Num(x) => Some(*x),
            NewVal::Label(s) if self.numeric && !self.force_text => parse_num(s),
            _ => None,
        }
    }
}

pub(super) struct SheetInfo {
    pub(super) name: String,
    pub(super) id: Option<String>,
    pub(super) part: String,
}

pub(super) struct Loaded {
    pub(super) info: usize,
    pub(super) xml: SheetXml,
}

/// The most one part of a workbook may inflate to, and all the parts read together. The workbook comes from the
/// document, so it is untrusted: a few kilobytes can inflate to gigabytes. Chart sheets are far below these.
pub(super) const MAX_PART_BYTES: usize = 32 << 20;

pub(super) const MAX_TOTAL_BYTES: usize = 96 << 20;

pub(super) struct Zip<'a> {
    pub(super) ar: zip::ZipArchive<Cursor<&'a [u8]>>,
    /// How much has been inflated so far.
    pub(super) spent: usize,
    /// A part was too big to read: whatever was built from the rest is not to be trusted.
    pub(super) over: bool,
}

impl<'a> Zip<'a> {
    /// A part's bytes; None when it is missing or inflates past the limits (the caller then builds a new workbook).
    pub(super) fn bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        let f = self.ar.by_name(name).ok()?;
        if f.size() > MAX_PART_BYTES as u64 {
            self.over = true;
            return None;
        }
        let mut v = Vec::new();
        // The declared size can lie: stop reading one byte past the limit.
        f.take(MAX_PART_BYTES as u64 + 1).read_to_end(&mut v).ok()?;
        self.spent += v.len();
        if v.len() > MAX_PART_BYTES || self.spent > MAX_TOTAL_BYTES {
            self.over = true;
            return None;
        }
        Some(v)
    }
    pub(super) fn text(&mut self, name: &str) -> Option<String> {
        String::from_utf8(self.bytes(name)?).ok()
    }
    /// The relationships of a part: (id, type, absolute target). External targets are left out.
    pub(super) fn rels(&mut self, part: &str) -> Vec<(String, String, String)> {
        let rels_part = match part.rfind('/') {
            Some(i) => format!("{}/_rels/{}.rels", &part[..i], &part[i + 1..]),
            None if part.is_empty() => "_rels/.rels".to_string(),
            None => format!("_rels/{part}.rels"),
        };
        let Some(text) = self.text(&rels_part) else { return Vec::new() };
        let mut out = Vec::new();
        let mut pos = 0;
        while let Some(Some(t)) = next_tag(&text, pos) {
            pos = t.end;
            if t.local() == "Relationship" && t.kind != Kind::Close && t.attr("TargetMode") != Some("External") {
                if let (Some(id), Some(ty), Some(target)) = (t.attr("Id"), t.attr("Type"), t.attr("Target")) {
                    let target = unescape(target).unwrap_or_default();
                    out.push((id.to_string(), ty.to_string(), resolve(part, &target)));
                }
            }
        }
        out
    }
}

pub(super) fn resolve(base: &str, target: &str) -> String {
    let joined = if let Some(t) = target.strip_prefix('/') {
        t.to_string()
    } else {
        match base.rfind('/') {
            Some(i) => format!("{}/{}", &base[..i], target),
            None => target.to_string(),
        }
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub(super) struct State<'a> {
    pub(super) zip: Zip<'a>,
    pub(super) infos: Vec<SheetInfo>,
    pub(super) loaded: Vec<Loaded>,
    pub(super) sst: Vec<String>,
    pub(super) calc_chain: Option<String>,
    pub(super) writes: BTreeMap<(usize, u32, u32), Put>,
}

impl<'a> State<'a> {
    pub(super) fn open(book: &'a [u8]) -> Option<State<'a>> {
        let ar = zip::ZipArchive::new(Cursor::new(book)).ok()?;
        let mut zip = Zip { ar, spent: 0, over: false };
        let wb_part = zip.rels("").into_iter().find(|r| r.1.ends_with("/officeDocument")).map(|r| r.2).unwrap_or_else(|| "xl/workbook.xml".into());
        let wb = zip.text(&wb_part)?;
        let rels = zip.rels(&wb_part);
        let mut infos = Vec::new();
        let mut pos = 0;
        while let Some(t) = next_tag(&wb, pos)? {
            pos = t.end;
            if t.local() == "sheet" && t.kind != Kind::Close {
                let name = unescape(t.attr("name")?)?;
                let rid = t.attrs.iter().find(|(k, _)| k.contains(':') && local(k) == "id").map(|(_, v)| *v)?;
                let part = rels.iter().find(|r| r.0 == rid).map(|r| r.2.clone()).unwrap_or_default();
                infos.push(SheetInfo { name, id: t.attr("sheetId").map(String::from), part });
            }
        }
        let sst = match rels.iter().find(|r| r.1.ends_with("/sharedStrings")) {
            Some(r) => match zip.text(&r.2) {
                Some(t) => shared_strings(&t)?,
                None => Vec::new(),
            },
            None => Vec::new(),
        };
        let calc_chain = rels.iter().find(|r| r.1.ends_with("/calcChain")).map(|r| r.2.clone());
        Some(State { zip, infos, loaded: Vec::new(), sst, calc_chain, writes: BTreeMap::new() })
    }

    /// The loaded sheet named `name` (case does not matter, as in Excel).
    pub(super) fn sheet(&mut self, name: &str) -> Option<usize> {
        let lname = name.to_lowercase();
        let info = self.infos.iter().position(|s| s.name.to_lowercase() == lname)?;
        if let Some(i) = self.loaded.iter().position(|l| l.info == info) {
            return Some(i);
        }
        let part = self.infos[info].part.clone();
        if part.is_empty() {
            return None;
        }
        let text = self.zip.text(&part)?;
        let xml = parse_sheet(text)?;
        self.loaded.push(Loaded { info, xml });
        Some(self.loaded.len() - 1)
    }

    pub(super) fn cell(&self, sh: usize, r: u32, c: u32) -> Option<&CellInfo> {
        let x = &self.loaded[sh].xml;
        x.index.get(&(r, c)).map(|&(ri, ci)| &x.rows[ri].cells[ci])
    }

    /// The style a cell has (or will have) and whether it holds a number.
    pub(super) fn template(&self, sh: usize, r: u32, c: u32) -> (Option<String>, bool) {
        let cell = self.cell(sh, r, c);
        if let Some(p) = self.writes.get(&(sh, r, c)) {
            let style = cell.map(|c| c.s.clone()).unwrap_or_else(|| p.style.clone());
            return (style, p.number().is_some());
        }
        match cell {
            Some(cell) => (cell.s.clone(), matches!(value_of(cell, &self.sst), Val::Num(_))),
            None => (None, false),
        }
    }

    /// Plan writing `val` to a cell. `style_from` is the cell a new cell copies its style from. Two writes to
    /// one cell must agree.
    pub(super) fn put(&mut self, sh: usize, r: u32, c: u32, val: NewVal, style_from: Option<(u32, u32)>) -> Option<()> {
        if r == 0 || c == 0 || r > MAX_ROW || c > MAX_COL {
            return None;
        }
        let val = val.norm();
        if let Some(p) = self.writes.get(&(sh, r, c)) {
            return (p.val == val).then_some(());
        }
        let src = style_from.map(|(sr, sc)| self.template(sh, sr, sc)).unwrap_or((None, false));
        let (style, numeric) = match self.cell(sh, r, c) {
            Some(cell) if cell.has_content() => (None, matches!(value_of(cell, &self.sst), Val::Num(_))),
            Some(_) => (None, src.1),
            None => src,
        };
        self.writes.insert((sh, r, c), Put { val, style, numeric, force_text: false });
        Some(())
    }

    /// Write `cells` along an old range, keeping its start; returns the new range.
    pub(super) fn write_line(&mut self, sh: usize, o: Orient, old: &Area, cells: &[NewVal]) -> Option<Area> {
        let start = o.along(old.r1, old.c1);
        let across = o.across(old.r1, old.c1);
        let old_len = old.cells() as u32;
        let n = cells.len().max(1) as u32;
        if start.checked_add(n - 1)? > o.max_along() {
            return None;
        }
        let last = o.rc(start + old_len - 1, across);
        for k in 0..n {
            let (r, c) = o.rc(start + k, across);
            let v = cells.get(k as usize).cloned().unwrap_or(NewVal::Gap);
            self.put(sh, r, c, v, (k >= old_len).then_some(last))?;
        }
        Some(o.line(start, across, n))
    }

    /// Write a new range of `n` cells at `across`, copying styles from the cells at `style_across`.
    pub(super) fn write_new_line(&mut self, sh: usize, o: Orient, (start, across): (u32, u32), n: u32, style_across: u32, val: impl Fn(usize) -> NewVal) -> Option<Area> {
        if start.checked_add(n - 1)? > o.max_along() || across > o.max_across() {
            return None;
        }
        for k in 0..n {
            let (r, c) = o.rc(start + k, across);
            self.put(sh, r, c, val(k as usize), Some(o.rc(start + k, style_across)))?;
        }
        Some(o.line(start, across, n))
    }

    /// The first column (row, for series in rows) at or after `from` with no data between `lo` and `hi`.
    pub(super) fn free_across(&self, sh: usize, o: Orient, lo: u32, hi: u32, from: u32) -> Option<u32> {
        let mut used: HashSet<u32> = HashSet::new();
        for row in &self.loaded[sh].xml.rows {
            for c in &row.cells {
                if c.has_content() && (lo..=hi).contains(&o.along(row.r, c.col)) {
                    used.insert(o.across(row.r, c.col));
                }
            }
        }
        for (&(s, r, c), p) in &self.writes {
            if s == sh && p.val != NewVal::Gap && (lo..=hi).contains(&o.along(r, c)) {
                used.insert(o.across(r, c));
            }
        }
        let mut a = from;
        while used.contains(&a) {
            a += 1;
        }
        (a <= o.max_across()).then_some(a)
    }

    /// The text a header cell will hold, if it is text.
    pub(super) fn header_text(&self, sh: usize, r: u32, c: u32) -> Option<String> {
        if let Some(p) = self.writes.get(&(sh, r, c)) {
            return match &p.val {
                NewVal::Label(s) => Some(s.clone()),
                _ => None,
            };
        }
        match value_of(self.cell(sh, r, c)?, &self.sst) {
            Val::Text(t) if !t.is_empty() => Some(t),
            _ => None,
        }
    }

    // ---- tables ------------------------------------------------------------------------------------

    /// Grow the tables of a sheet that hold the chart's block, and rename columns whose header changes.
    /// Returns the changed table parts.
    pub(super) fn edit_tables(&mut self, sh: usize, old_b: Option<Area>, new_b: Option<Area>) -> Option<Vec<(String, String)>> {
        let info = self.loaded[sh].info;
        let rids = self.loaded[sh].xml.table_rids.clone();
        if rids.is_empty() {
            return Some(Vec::new());
        }
        let sheet_part = self.infos[info].part.clone();
        let rels = self.zip.rels(&sheet_part);
        let mut tables = Vec::new();
        for rid in &rids {
            let part = rels.iter().find(|r| &r.0 == rid && r.1.ends_with("/table")).map(|r| r.2.clone())?;
            let text = self.zip.text(&part)?;
            tables.push(parse_table(part, text)?);
        }
        let mut out = Vec::new();
        for ti in 0..tables.len() {
            let t = &tables[ti];
            let mut nb = t.area;
            if let (Some(ob), Some(nw)) = (old_b, new_b) {
                if t.area.contains(&ob) {
                    nb = t.area.union(&nw);
                }
            }
            // A table only grows into columns (rows) the chart writes, so it never takes in unrelated data
            // between its edge and a new series placed further out.
            let writes_in = |rows: (u32, u32), cols: (u32, u32)| self.writes.iter().any(|(&(s, r, c), p)| s == sh && p.val != NewVal::Gap && (rows.0..=rows.1).contains(&r) && (cols.0..=cols.1).contains(&c));
            if nb.c2 > t.area.c2 && !(t.area.c2 + 1..=nb.c2).all(|c| writes_in((nb.r1, nb.r2), (c, c))) {
                nb.c2 = t.area.c2;
            }
            if nb.r2 > t.area.r2 && !(t.area.r2 + 1..=nb.r2).all(|r| writes_in((r, r), (nb.c1, nb.c2))) {
                nb.r2 = t.area.r2;
            }
            let grow = nb != t.area;
            if grow {
                if t.totals > 0 || nb.r2 > MAX_ROW || nb.c2 > MAX_COL {
                    return None;
                }
                if tables.iter().enumerate().any(|(j, o)| j != ti && o.area.intersects(&nb)) {
                    return None;
                }
            }
            let width = (t.area.c2 - t.area.c1 + 1) as usize;
            let header_written = t.header && (nb.c1..=nb.c2).any(|c| self.writes.contains_key(&(sh, t.area.r1, c)));
            if !grow && !header_written {
                continue;
            }
            if t.cols.len() != width {
                return None;
            }
            let mut names: Vec<String> = t.cols.iter().map(|c| c.1.clone()).collect();
            let mut renames: Vec<(usize, String)> = Vec::new();
            let mut added: Vec<String> = Vec::new();
            if t.header {
                let hr = t.area.r1;
                for c in nb.c1..=nb.c2 {
                    let j = (c - t.area.c1) as usize;
                    if let Some(p) = self.writes.get_mut(&(sh, hr, c)) {
                        p.force_text = true;
                        let NewVal::Label(text) = &p.val else { return None };
                        if text.chars().any(|ch| (ch as u32) < 0x20) {
                            return None;
                        }
                        if j < width && names[j] != *text {
                            // Formulas that name the old column would break.
                            let old = format!("[{}]", esc_text(&names[j]));
                            if t.text.contains(&old) || self.loaded[sh].xml.text.contains(&old) {
                                return None;
                            }
                            renames.push((j, text.clone()));
                            names[j] = text.clone();
                        }
                    }
                }
                for c in t.area.c2 + 1..=nb.c2 {
                    let text = self.header_text(sh, hr, c)?;
                    if text.chars().any(|ch| (ch as u32) < 0x20) {
                        return None;
                    }
                    if let Some(p) = self.writes.get_mut(&(sh, hr, c)) {
                        p.force_text = true;
                    }
                    names.push(text.clone());
                    added.push(text);
                }
            } else if nb.c2 > t.area.c2 {
                return None;
            }
            let mut seen = HashSet::new();
            if !names.iter().all(|n| seen.insert(n.to_lowercase())) {
                return None;
            }
            if !grow && renames.is_empty() {
                continue;
            }
            let mut edits: Vec<(usize, usize, String)> = Vec::new();
            if grow {
                edits.push((t.ref_span.0, t.ref_span.1, nb.plain()));
                if let Some((v, span)) = &t.auto_filter {
                    if !v.eq_ignore_ascii_case(&t.area.plain()) {
                        return None;
                    }
                    edits.push((span.0, span.1, nb.plain()));
                }
            }
            for (j, name) in &renames {
                let span = t.cols[*j].2;
                edits.push((span.0, span.1, esc_attr(name)));
            }
            if !added.is_empty() {
                let (count_span, close) = t.cols_tag?;
                if let Some(cs) = count_span {
                    edits.push((cs.0, cs.1, names.len().to_string()));
                }
                let mut id = t.cols.iter().map(|c| c.0).max().unwrap_or(0);
                let mut xml = String::new();
                for name in &added {
                    id += 1;
                    xml.push_str(&format!("<{}tableColumn id=\"{id}\" name=\"{}\"/>", t.prefix, esc_attr(name)));
                }
                edits.push((close, close, xml));
            }
            out.push((t.part.clone(), splice(&t.text, edits)?));
        }
        Some(out)
    }

    // ---- writing the sheet ---------------------------------------------------------------------------

    /// The sheet's new text (`None` when nothing changes) and the cells whose formula was removed.
    pub(super) fn render_sheet(&self, sh: usize) -> Option<(Option<String>, Vec<String>)> {
        let x = &self.loaded[sh].xml;
        let s = x.text.as_str();
        let p = x.prefix.as_str();
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let mut removed = Vec::new();
        let mut new_cells: BTreeMap<u32, Vec<(u32, String)>> = BTreeMap::new();
        let mut grown: Option<Area> = None;
        for (&(_, r, c), put) in self.writes.range((sh, 0, 0)..=(sh, u32::MAX, u32::MAX)) {
            match self.cell(sh, r, c) {
                Some(cell) => {
                    if same(cell, put, &self.sst) {
                        continue;
                    }
                    if x.arrays.iter().any(|a| a.contains_cell(r, c)) {
                        return None;
                    }
                    let keep = cell.shared_master().then(|| &s[cell.f.as_ref().map_or(0, |f| f.start)..cell.f.as_ref().map_or(0, |f| f.end)]);
                    if cell.f.is_some() && keep.is_none() {
                        removed.push(cell_name(r, c));
                    }
                    let attrs: Vec<(String, String)> = cell.attrs.iter().filter(|(k, _)| !matches!(k.as_str(), "t" | "cm" | "vm")).cloned().collect();
                    edits.push((cell.start, cell.end, cell_xml(p, &attrs, put, keep)));
                }
                None => {
                    if put.val == NewVal::Gap {
                        continue;
                    }
                    if x.arrays.iter().any(|a| a.contains_cell(r, c)) {
                        return None;
                    }
                    let mut attrs = vec![("r".to_string(), cell_name(r, c))];
                    if let Some(st) = &put.style {
                        attrs.push(("s".to_string(), st.clone()));
                    }
                    new_cells.entry(r).or_default().push((c, cell_xml(p, &attrs, put, None)));
                    let a = Area::cell(r, c);
                    grown = Some(grown.map_or(a, |g| g.union(&a)));
                }
            }
        }
        // New cells go into their rows; missing rows are made.
        let row_pos = |r: u32| x.rows.iter().position(|row| row.r >= r);
        let mut fresh_rows = String::new();
        for (r, cells) in &new_cells {
            let (lo, hi) = (cells.first()?.0, cells.last()?.0);
            match row_pos(*r).filter(|&i| x.rows[i].r == *r) {
                Some(i) => {
                    let row = &x.rows[i];
                    let spans = row.spans.and_then(|(a, b)| {
                        let v = &s[a..b];
                        let (f, l) = v.split_once(':')?;
                        let (f, l): (u32, u32) = (f.trim().parse().ok()?, l.trim().parse().ok()?);
                        (lo < f || hi > l).then(|| (a, b, format!("{}:{}", f.min(lo), l.max(hi))))
                    });
                    if row.empty {
                        let tag = &s[row.start..row.end];
                        let mut open = tag.strip_suffix("/>")?.trim_end().to_string();
                        if let Some((a, b, v)) = &spans {
                            open = format!("{}{}{}", &s[row.start..*a], v, &s[*b..row.end - 2].trim_end());
                        }
                        let body: String = cells.iter().map(|c| c.1.as_str()).collect();
                        edits.push((row.start, row.end, format!("{open}>{body}</{p}row>")));
                    } else {
                        if let Some((a, b, v)) = spans {
                            edits.push((a, b, v));
                        }
                        for (c, xml) in cells {
                            let at = row.cells.iter().find(|cell| cell.col > *c).map_or(row.close_start, |cell| cell.start);
                            edits.push((at, at, xml.clone()));
                        }
                    }
                }
                None => {
                    let body: String = cells.iter().map(|c| c.1.as_str()).collect();
                    let xml = format!("<{p}row r=\"{r}\">{body}</{p}row>");
                    match (&x.data, row_pos(*r)) {
                        (_, Some(i)) => edits.push((x.rows[i].start, x.rows[i].start, xml)),
                        (DataPos::Open { close }, None) => edits.push((*close, *close, xml)),
                        (DataPos::Empty { .. }, None) => fresh_rows.push_str(&xml),
                    }
                }
            }
        }
        if let DataPos::Empty { start, end } = x.data {
            if !fresh_rows.is_empty() {
                edits.push((start, end, format!("<{p}sheetData>{fresh_rows}</{p}sheetData>")));
            }
        }
        if let (Some((a, b)), Some(g)) = (x.dimension, grown) {
            let old = parse_area(&s[a..b]);
            let next = old.map_or(g, |o| o.union(&g));
            if old != Some(next) && old.is_some() {
                edits.push((a, b, next.plain()));
            }
        }
        if edits.is_empty() {
            return Some((None, removed));
        }
        Some((Some(splice(s, edits)?), removed))
    }
}
