//! Other parts of the workbook: tables, the calculation chain and the zip.

use super::*;

/// Whether a cell already holds what `put` would write.
pub(super) fn same(cell: &CellInfo, put: &Put, sst: &[String]) -> bool {
    let v = value_of(cell, sst);
    match &put.val {
        NewVal::Gap => !cell.has_content(),
        NewVal::Num(x) => matches!(v, Val::Num(y) if y == *x),
        NewVal::Label(s) => match v {
            Val::Text(t) => t == *s,
            Val::Num(y) => !put.force_text && parse_num(s) == Some(y),
            _ => false,
        },
    }
}

/// A `<c>` element. `keep` is the raw `<f>` of a shared formula master, kept as it is.
pub(super) fn cell_xml(p: &str, attrs: &[(String, String)], put: &Put, keep: Option<&str>) -> String {
    let mut head = format!("<{p}c");
    for (k, v) in attrs {
        head.push_str(&format!(" {k}=\"{}\"", v.replace('"', "&quot;")));
    }
    let f = keep.unwrap_or("");
    if let Some(x) = put.number() {
        return format!("{head}>{f}<{p}v>{x}</{p}v></{p}c>");
    }
    match &put.val {
        NewVal::Label(s) if keep.is_some() => format!("{head} t=\"str\">{f}<{p}v>{}</{p}v></{p}c>", esc_text(s)),
        NewVal::Label(s) => format!("{head} t=\"inlineStr\"><{p}is><{p}t xml:space=\"preserve\">{}</{p}t></{p}is></{p}c>", esc_text(s)),
        _ if keep.is_some() => format!("{head}>{f}</{p}c>"),
        _ => format!("{head}/>"),
    }
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

pub(super) struct TableXml {
    pub(super) part: String,
    pub(super) text: String,
    pub(super) prefix: String,
    pub(super) area: Area,
    pub(super) ref_span: (usize, usize),
    pub(super) header: bool,
    pub(super) totals: u32,
    pub(super) auto_filter: Option<(String, (usize, usize))>,
    /// The `count` attribute's span and where `</tableColumns>` starts.
    pub(super) cols_tag: Option<(Option<(usize, usize)>, usize)>,
    /// id, name, span of the name's value.
    pub(super) cols: Vec<(u32, String, (usize, usize))>,
}

pub(super) fn parse_table(part: String, text: String) -> Option<TableXml> {
    let s = text.as_str();
    let mut pos = 0;
    let root = loop {
        let t = next_tag(s, pos)??;
        pos = t.end;
        if t.kind == Kind::Open {
            break t;
        }
    };
    if root.local() != "table" {
        return None;
    }
    let rv = root.attr("ref")?;
    let area = parse_area(rv)?;
    let ref_span = (off(s, rv), off(s, rv) + rv.len());
    let header = root.attr("headerRowCount").is_none_or(|v| v.trim() != "0");
    let totals = root.attr("totalsRowCount").map_or(Some(0), |v| v.trim().parse().ok())?;
    let prefix = root.prefix().to_string();
    let mut auto_filter = None;
    let mut cols_tag = None;
    let mut cols = Vec::new();
    let mut count_span = None;
    let mut depth = 1usize;
    while let Some(t) = next_tag(s, pos)? {
        pos = t.end;
        match (depth, t.local(), t.kind) {
            (_, _, Kind::Close) => {
                if depth == 2 && t.local() == "tableColumns" {
                    cols_tag = Some((count_span, t.start));
                }
                depth = depth.checked_sub(1)?;
                continue;
            }
            (1, "autoFilter", _) => {
                if let Some(v) = t.attr("ref") {
                    auto_filter = Some((v.to_string(), (off(s, v), off(s, v) + v.len())));
                }
            }
            (1, "tableColumns", Kind::Open) => {
                count_span = t.attr("count").map(|v| (off(s, v), off(s, v) + v.len()));
            }
            (2, "tableColumn", _) => {
                let id: u32 = t.attr("id")?.trim().parse().ok()?;
                let v = t.attr("name")?;
                cols.push((id, unescape(v)?, (off(s, v), off(s, v) + v.len())));
            }
            _ => {}
        }
        if t.kind == Kind::Open {
            depth += 1;
        }
    }
    Some(TableXml { part, prefix, area, ref_span, header, totals, auto_filter, cols_tag, cols, text })
}

// ---------------------------------------------------------------------------
// calcChain
// ---------------------------------------------------------------------------

/// Drop the entries of cells whose formula was removed. `removed` maps a sheet's `i` value (its id and its
/// position; dropping an extra entry is harmless, a stale one is not) to cell names. `Some(None)` when nothing
/// changes; `None` when the chain would be left empty.
pub(super) fn edit_calc_chain(text: &str, removed: &HashMap<String, HashSet<String>>) -> Option<Option<String>> {
    let mut edits = Vec::new();
    let mut pos = 0;
    let mut cur_i: Option<String> = None;
    let mut last_kept_i: Option<String> = None;
    let mut kept = 0usize;
    let mut dropped = false;
    while let Some(t) = next_tag(text, pos)? {
        pos = t.end;
        if t.local() != "c" || t.kind == Kind::Close {
            continue;
        }
        if t.kind == Kind::Open {
            return None;
        }
        if let Some(i) = t.attr("i") {
            cur_i = Some(i.trim().to_string());
        }
        let r = t.attr("r")?.trim().replace('$', "").to_ascii_uppercase();
        let gone = cur_i.as_ref().and_then(|i| removed.get(i)).is_some_and(|set| set.contains(&r));
        if gone {
            edits.push((t.start, t.end, String::new()));
            dropped = true;
            continue;
        }
        kept += 1;
        if t.attr("i").is_none() && last_kept_i != cur_i {
            let at = t.start + 1 + t.name.len();
            edits.push((at, at, format!(" i=\"{}\"", cur_i.as_ref()?)));
        }
        last_kept_i = cur_i.clone();
    }
    if !dropped {
        return Some(None);
    }
    if kept == 0 {
        return None;
    }
    Some(Some(splice(text, edits)?))
}

// ---------------------------------------------------------------------------
// The zip
// ---------------------------------------------------------------------------

/// The workbook with `changed` parts replaced; every other entry is copied raw, in order.
pub(super) fn rezip(book: &[u8], changed: &HashMap<String, Vec<u8>>) -> Option<Vec<u8>> {
    let mut ar = zip::ZipArchive::new(Cursor::new(book)).ok()?;
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut done = 0;
    for i in 0..ar.len() {
        let f = ar.by_index_raw(i).ok()?;
        let name = f.name().ok()?.into_owned();
        match changed.get(&name) {
            Some(data) => {
                let method = if f.compression() == zip::CompressionMethod::Stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
                let opts = zip::write::SimpleFileOptions::default().compression_method(method).last_modified_time(f.last_modified().unwrap_or_default());
                drop(f);
                w.start_file(name.as_str(), opts).ok()?;
                w.write_all(data).ok()?;
                done += 1;
            }
            None => w.raw_copy_file(f).ok()?,
        }
    }
    if done != changed.len() {
        return None;
    }
    Some(w.finish().ok()?.into_inner())
}
