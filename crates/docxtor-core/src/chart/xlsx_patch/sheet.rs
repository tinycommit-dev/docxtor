//! The sheet XML: its rows and cells, and the values they hold.

use super::*;

#[derive(Debug)]
pub(super) struct FormulaInfo {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) t: Option<String>,
    pub(super) ref_: Option<String>,
}

#[derive(Debug)]
pub(super) struct CellInfo {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) col: u32,
    pub(super) attrs: Vec<(String, String)>,
    pub(super) t: Option<String>,
    pub(super) s: Option<String>,
    pub(super) f: Option<FormulaInfo>,
    pub(super) v: Option<String>,
    pub(super) inline: Option<String>,
}

impl CellInfo {
    pub(super) fn has_content(&self) -> bool {
        self.f.is_some() || self.v.is_some() || self.inline.is_some()
    }
    /// A shared formula other cells take their formula from.
    pub(super) fn shared_master(&self) -> bool {
        self.f.as_ref().is_some_and(|f| f.t.as_deref() == Some("shared") && f.ref_.as_deref().and_then(parse_area).is_some_and(|a| a.cells() > 1))
    }
}

#[derive(Debug)]
pub(super) struct RowInfo {
    pub(super) start: usize,
    pub(super) close_start: usize,
    pub(super) end: usize,
    pub(super) r: u32,
    pub(super) empty: bool,
    pub(super) spans: Option<(usize, usize)>,
    pub(super) cells: Vec<CellInfo>,
}

#[derive(Debug)]
pub(super) enum DataPos {
    Open { close: usize },
    Empty { start: usize, end: usize },
}

#[derive(Debug)]
pub(super) struct SheetXml {
    pub(super) text: String,
    /// The namespace prefix of the sheet's elements (`x:`), usually empty.
    pub(super) prefix: String,
    pub(super) data: DataPos,
    pub(super) rows: Vec<RowInfo>,
    pub(super) index: HashMap<(u32, u32), (usize, usize)>,
    pub(super) dimension: Option<(usize, usize)>,
    pub(super) arrays: Vec<Area>,
    pub(super) table_rids: Vec<String>,
}

pub(super) fn parse_sheet(text: String) -> Option<SheetXml> {
    let s = text.as_str();
    let mut pos = 0;
    let mut depth = 0usize;
    let mut dimension = None;
    let mut data = None;
    let mut rows = Vec::new();
    let mut prefix = String::new();
    let mut arrays = Vec::new();
    let mut table_rids = Vec::new();
    while let Some(tag) = next_tag(s, pos)? {
        pos = tag.end;
        if tag.kind == Kind::Close {
            depth = depth.checked_sub(1)?;
            continue;
        }
        if depth == 0 {
            if tag.local() != "worksheet" || tag.kind != Kind::Open {
                return None;
            }
            depth = 1;
            continue;
        }
        match (depth, tag.local(), tag.kind) {
            (1, "dimension", _) => {
                if let Some(v) = tag.attr("ref") {
                    let o = off(s, v);
                    dimension = Some((o, o + v.len()));
                }
            }
            (1, "sheetData", Kind::Empty) => {
                if data.is_some() {
                    return None;
                }
                prefix = tag.prefix().to_string();
                data = Some(DataPos::Empty { start: tag.start, end: tag.end });
                continue;
            }
            (1, "sheetData", Kind::Open) => {
                if data.is_some() {
                    return None;
                }
                prefix = tag.prefix().to_string();
                let close = parse_rows(s, &mut pos, &mut rows, &mut arrays)?;
                data = Some(DataPos::Open { close });
                continue;
            }
            (_, "tablePart", _) => {
                if let Some((_, v)) = tag.attrs.iter().find(|(k, _)| k.contains(':') && local(k) == "id") {
                    table_rids.push(v.to_string());
                }
            }
            _ => {}
        }
        if tag.kind == Kind::Open {
            depth += 1;
        }
    }
    if depth != 0 {
        return None;
    }
    let mut index = HashMap::new();
    for (ri, row) in rows.iter().enumerate() {
        for (ci, c) in row.cells.iter().enumerate() {
            index.insert((row.r, c.col), (ri, ci));
        }
    }
    Some(SheetXml { prefix, data: data?, rows, index, dimension, arrays, table_rids, text })
}

pub(super) fn parse_rows(s: &str, pos: &mut usize, rows: &mut Vec<RowInfo>, arrays: &mut Vec<Area>) -> Option<usize> {
    loop {
        let tag = next_tag(s, *pos)??;
        *pos = tag.end;
        match (tag.local(), tag.kind) {
            ("sheetData", Kind::Close) => return Some(tag.start),
            ("row", k @ (Kind::Open | Kind::Empty)) => {
                let r: u32 = tag.attr("r")?.trim().parse().ok()?;
                if r == 0 || r > MAX_ROW || rows.last().is_some_and(|p| p.r >= r) {
                    return None;
                }
                let spans = tag.attr("spans").map(|v| {
                    let o = off(s, v);
                    (o, o + v.len())
                });
                let mut row = RowInfo { start: tag.start, close_start: tag.end, end: tag.end, r, empty: k == Kind::Empty, spans, cells: Vec::new() };
                if k == Kind::Open {
                    loop {
                        let t = next_tag(s, *pos)??;
                        *pos = t.end;
                        match (t.local(), t.kind) {
                            ("row", Kind::Close) => {
                                row.close_start = t.start;
                                row.end = t.end;
                                break;
                            }
                            ("c", Kind::Open | Kind::Empty) => {
                                let cell = parse_cell_el(s, pos, &t, r, arrays)?;
                                if row.cells.last().is_some_and(|p| p.col >= cell.col) {
                                    return None;
                                }
                                row.cells.push(cell);
                            }
                            _ => return None,
                        }
                    }
                }
                rows.push(row);
            }
            _ => return None,
        }
    }
}

pub(super) fn parse_cell_el(s: &str, pos: &mut usize, tag: &Tag, row: u32, arrays: &mut Vec<Area>) -> Option<CellInfo> {
    let (r, c) = parse_cell(tag.attr("r")?)?;
    if r != row {
        return None;
    }
    let mut cell = CellInfo {
        start: tag.start,
        end: tag.end,
        col: c,
        attrs: tag.attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        t: tag.attr("t").map(String::from),
        s: tag.attr("s").map(String::from),
        f: None,
        v: None,
        inline: None,
    };
    if tag.kind == Kind::Empty {
        return Some(cell);
    }
    loop {
        let t = next_tag(s, *pos)??;
        *pos = t.end;
        match (t.local(), t.kind) {
            ("c", Kind::Close) => {
                cell.end = t.end;
                return Some(cell);
            }
            ("f", k @ (Kind::Open | Kind::Empty)) => {
                let end = if k == Kind::Open {
                    let cl = next_tag(s, *pos)??;
                    if cl.kind != Kind::Close || cl.local() != "f" {
                        return None;
                    }
                    *pos = cl.end;
                    cl.end
                } else {
                    t.end
                };
                let ft = t.attr("t").map(String::from);
                let fref = t.attr("ref").map(String::from);
                if matches!(ft.as_deref(), Some("array") | Some("dataTable")) {
                    match fref.as_deref().and_then(parse_area) {
                        Some(a) if a.cells() > 1 => arrays.push(a),
                        Some(_) => {}
                        None => return None,
                    }
                }
                cell.f = Some(FormulaInfo { start: t.start, end, t: ft, ref_: fref });
            }
            ("v", Kind::Open) => {
                let cl = next_tag(s, *pos)??;
                if cl.kind != Kind::Close || cl.local() != "v" {
                    return None;
                }
                *pos = cl.end;
                cell.v = Some(unescape(&s[t.end..cl.start])?);
            }
            ("v", Kind::Empty) => cell.v = Some(String::new()),
            ("is", Kind::Open) => cell.inline = Some(read_inline(s, pos)?),
            ("is", Kind::Empty) => cell.inline = Some(String::new()),
            (_, Kind::Open) => skip_subtree(s, pos)?,
            (_, Kind::Empty) => {}
            _ => return None,
        }
    }
}

/// The text of an `<is>` (or `<si>`) element just opened: its `t` runs, without phonetic runs.
pub(super) fn read_inline(s: &str, pos: &mut usize) -> Option<String> {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut phonetic: Option<usize> = None;
    loop {
        let t = next_tag(s, *pos)??;
        *pos = t.end;
        match t.kind {
            Kind::Close => {
                if depth == 0 {
                    return Some(out);
                }
                depth -= 1;
                if phonetic == Some(depth) {
                    phonetic = None;
                }
            }
            Kind::Empty => {}
            Kind::Open => {
                if t.local() == "t" && phonetic.is_none() {
                    let cl = next_tag(s, *pos)??;
                    if cl.kind != Kind::Close || cl.local() != "t" {
                        return None;
                    }
                    *pos = cl.end;
                    out.push_str(&unescape(&s[t.end..cl.start])?);
                    continue;
                }
                if t.local() == "rPh" && phonetic.is_none() {
                    phonetic = Some(depth);
                }
                depth += 1;
            }
        }
    }
}

/// The strings of `sharedStrings.xml`.
pub(super) fn shared_strings(text: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(t) = next_tag(text, pos)? {
        pos = t.end;
        if t.local() == "si" {
            match t.kind {
                Kind::Open => out.push(read_inline(text, &mut pos)?),
                Kind::Empty => out.push(String::new()),
                Kind::Close => {}
            }
        }
    }
    Some(out)
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Val {
    Empty,
    Num(f64),
    Text(String),
    Other,
}

pub(super) fn value_of(c: &CellInfo, sst: &[String]) -> Val {
    match c.t.as_deref() {
        Some("s") => c.v.as_deref().and_then(|v| v.trim().parse::<usize>().ok()).and_then(|i| sst.get(i).cloned()).map(Val::Text).unwrap_or(Val::Other),
        Some("inlineStr") => c.inline.clone().map(Val::Text).unwrap_or(Val::Empty),
        Some("str") => c.v.clone().map(Val::Text).unwrap_or(Val::Empty),
        None | Some("n") => match &c.v {
            None => Val::Empty,
            Some(v) => v.trim().parse::<f64>().ok().filter(|x| x.is_finite()).map(Val::Num).unwrap_or(Val::Other),
        },
        _ => Val::Other,
    }
}
