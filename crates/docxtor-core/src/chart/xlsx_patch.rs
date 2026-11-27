//! Changing the data of a chart whose workbook was made in Excel: only the cells the chart uses are edited,
//! in the sheet's XML text, so every other cell, sheet, formula, style, table and defined name stays as it was.
//!
//! How it works:
//! - The chart's old ranges (`Sheet1!$B$2:$B$5`) are parsed. Each must be one cell, one column or one row on a
//!   sheet of this workbook; anything else (several areas, 2-D blocks, 3-D or external refs, defined names)
//!   makes `patch` give up so the caller can rebuild the workbook.
//! - Each range keeps its start and takes the new length. Cells are written one by one; a cell that already
//!   holds the new value is left alone. Leftover cells of a shrunk range are left alone too.
//! - A new series goes to the first column (or row, for series laid out in rows) that is empty over the rows
//!   the chart uses.
//! - The sheet XML is edited as text: changed cells are replaced, new cells and rows are inserted in order,
//!   and `<dimension>` grows when needed. Tables over the chart's block grow with it and get the new header
//!   names. Stale `xl/calcChain.xml` entries for formulas that became constants are dropped.
//! - Zip entries that don't change are copied raw, in the same order.
//!
//! Limits:
//! - A shared formula master (`<f t="shared" ref=..>` over more than one cell) keeps its formula; only its
//!   cached value changes, so Excel shows the formula's result again after it recalculates.
//! - Writing into a cell inside a multi-cell array formula is refused (`None`).
//! - Growing a range overwrites what lies below (or right of) it, as typing in Excel would.
//! - Renaming a table column does not rewrite formulas on other sheets that use the old name; if the table or
//!   this sheet mentions `[old name]`, `patch` gives up instead.
//! - Text goes into inline string cells, so `sharedStrings.xml` is never edited.

use super::model::ChartSeries;
use super::xlsx::{col_name, SeriesRefs};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read, Write};

/// The data a chart needs its workbook to hold after an edit.
pub struct SheetEdit<'a> {
    /// The category labels (not used by scatter charts).
    pub categories: &'a [String],
    /// The series as they should be: name, values and (scatter) X values.
    pub series: &'a [ChartSeries],
    /// Scatter and bubble charts: `cat` holds the X values.
    pub scatter: bool,
    /// For each series in `series`, where the chart takes its data from now; `None` for a series added by this edit.
    /// An empty `name` or `cat` means the series has no such range; it stays empty in the result.
    pub old_refs: &'a [Option<SeriesRefs>],
    /// Bubble charts: for each series, the `c:bubbleSize` range (`Sheet1!$D$2:$D$5`); `None` for none or a new
    /// series. May be empty when no series has sizes.
    pub old_size_refs: &'a [Option<String>],
}

/// The edited workbook and the ranges each series should use afterwards (one per series in `SheetEdit::series`).
pub struct Patched {
    pub bytes: Vec<u8>,
    pub refs: Vec<SeriesRefs>,
    /// The bubble size range of each series that has sizes (`ChartSeries::sizes`), else `None`.
    pub size_refs: Vec<Option<String>>,
}

const MAX_ROW: u32 = 1_048_576;
const MAX_COL: u32 = 16_384;

/// Edit the workbook `book`. `None` when it can't be done safely (unusual ranges, a missing sheet, ...); the
/// caller then writes a fresh workbook as before.
pub fn patch(book: &[u8], edit: &SheetEdit) -> Option<Patched> {
    let n_series = edit.series.len();
    if n_series == 0 || edit.old_refs.len() != n_series || (!edit.old_size_refs.is_empty() && edit.old_size_refs.len() != n_series) {
        return None;
    }
    let mut st = State::open(book)?;

    // The old ranges.
    let mut olds: Vec<Option<Old>> = Vec::with_capacity(n_series);
    for (i, o) in edit.old_refs.iter().enumerate() {
        let size = edit.old_size_refs.get(i).cloned().flatten();
        match o {
            None => olds.push(None),
            Some(r) => olds.push(Some(Old::parse(r, size.as_deref())?)),
        }
    }
    let template = olds.iter().position(|o| o.is_some())?;
    let o = Orient(orientation(&olds)?);

    // How many points each series has.
    let shared_len = edit.categories.len().max(edit.series.iter().map(|s| s.values.len()).max().unwrap_or(0)).max(1);
    let lens: Vec<u32> = edit
        .series
        .iter()
        .map(|s| {
            let n = if edit.scatter {
                s.values.len().max(s.x_values.as_ref().map_or(0, |x| x.len())).max(s.sizes.as_ref().map_or(0, |x| x.len())).max(1)
            } else {
                shared_len.max(s.sizes.as_ref().map_or(0, |x| x.len()))
            };
            u32::try_from(n).ok()
        })
        .collect::<Option<_>>()?;

    let mut refs: Vec<Option<SeriesRefs>> = vec![None; n_series];
    let mut size_refs: Vec<Option<String>> = vec![None; n_series];
    // Per sheet: the bounding box of the old ranges and of the new ones (for tables).
    let mut blocks: HashMap<usize, (Option<Area>, Option<Area>)> = HashMap::new();
    let mut new_cat: Vec<Option<(usize, Area, String)>> = vec![None; n_series];
    // Series laid out in sizes that have no range yet: placed with the new series.
    let mut need_size: Vec<usize> = Vec::new();

    // ---- series that already have ranges ----------------------------------------------------------
    for (i, old) in olds.iter().enumerate() {
        let Some(old) = old else { continue };
        let s = &edit.series[i];
        let n = lens[i];
        let val_sh = st.sheet(&old.val.sheet)?;
        let vals: Vec<NewVal> = (0..n as usize).map(|k| num_val(s.values.get(k).copied().flatten())).collect();
        let val_area = st.write_line(val_sh, o, &old.val.area, &vals)?;
        grow_block(&mut blocks, val_sh, Some(old.val.area), val_area);
        let val_ref = format!("{}{}", old.val.prefix, val_area.abs());

        let name_ref = match &old.name {
            Some(r) => {
                let sh = st.sheet(&r.sheet)?;
                st.put(sh, r.area.r1, r.area.c1, NewVal::Label(s.name.clone()), None)?;
                grow_block(&mut blocks, sh, Some(r.area), r.area);
                format!("{}{}", r.prefix, r.area.abs())
            }
            None => String::new(),
        };
        let cat_ref = match &old.cat {
            Some(r) => {
                let sh = st.sheet(&r.sheet)?;
                let cells = cat_cells(edit, s, n);
                let area = st.write_line(sh, o, &r.area, &cells)?;
                grow_block(&mut blocks, sh, Some(r.area), area);
                new_cat[i] = Some((sh, area, r.prefix.clone()));
                format!("{}{}", r.prefix, area.abs())
            }
            None => String::new(),
        };
        if let Some(sizes) = &s.sizes {
            match &old.size {
                Some(r) => {
                    let sh = st.sheet(&r.sheet)?;
                    let cells: Vec<NewVal> = (0..n as usize).map(|k| num_val(sizes.get(k).copied().flatten())).collect();
                    let area = st.write_line(sh, o, &r.area, &cells)?;
                    grow_block(&mut blocks, sh, Some(r.area), area);
                    size_refs[i] = Some(format!("{}{}", r.prefix, area.abs()));
                }
                None => need_size.push(i),
            }
        }
        refs[i] = Some(SeriesRefs { name: name_ref, cat: cat_ref, val: val_ref });
    }

    // ---- new series, and sizes that need a place -------------------------------------------------
    let new_series: Vec<usize> = (0..n_series).filter(|i| olds[*i].is_none()).collect();
    if !new_series.is_empty() || !need_size.is_empty() {
        let t = olds[template].as_ref()?;
        let sh = st.sheet(&t.val.sheet)?;
        let same_sheet = |r: &Option<Ref>| r.as_ref().is_none_or(|r| r.sheet.to_lowercase() == t.val.sheet.to_lowercase());
        if !same_sheet(&t.name) || !same_sheet(&t.cat) || !same_sheet(&t.size) {
            return None;
        }
        // Every series uses the template's sheet here.
        for (i, old) in olds.iter().enumerate() {
            if let Some(old) = old {
                if need_size.contains(&i) && old.val.sheet.to_lowercase() != t.val.sheet.to_lowercase() {
                    return None;
                }
            }
        }
        let val_start = o.along(t.val.area.r1, t.val.area.c1);
        let name_along = t.name.as_ref().map(|r| o.along(r.area.r1, r.area.c1));
        // The rows (vertical layout) the chart uses: header through the longest range.
        let mut lo = val_start;
        let mut hi = 0u32;
        let mut across_max = 0u32;
        let mut neighbour = o.across(t.val.area.r1, t.val.area.c1);
        for (i, old) in olds.iter().enumerate() {
            let Some(old) = old else { continue };
            for r in [Some(&old.val), old.name.as_ref(), old.cat.as_ref(), old.size.as_ref()].into_iter().flatten() {
                if r.sheet.to_lowercase() == t.val.sheet.to_lowercase() {
                    lo = lo.min(o.along(r.area.r1, r.area.c1));
                    across_max = across_max.max(o.across(r.area.r2, r.area.c2));
                }
            }
            if old.val.sheet.to_lowercase() == t.val.sheet.to_lowercase() {
                neighbour = neighbour.max(o.across(old.val.area.r1, old.val.area.c1));
            }
            hi = hi.max(val_start.saturating_add(lens[i] - 1));
        }
        if let Some((_, a, _)) = &new_cat[template] {
            hi = hi.max(o.along(a.r2, a.c2));
        }
        for &i in &new_series {
            hi = hi.max(val_start.checked_add(lens[i] - 1)?);
        }
        if o.rc(hi, 1).0 > MAX_ROW || o.rc(1, hi).1 > MAX_COL {
            return None;
        }
        let mut cursor = across_max + 1;

        for &i in &need_size {
            let s = &edit.series[i];
            let old = olds[i].as_ref()?;
            let n = lens[i];
            let start = o.along(old.val.area.r1, old.val.area.c1);
            let a = st.free_across(sh, o, lo, hi, cursor)?;
            cursor = a + 1;
            let src = o.across(old.val.area.r1, old.val.area.c1);
            let area = st.write_new_line(sh, o, (start, a), n, src, |k| num_val(s.sizes.as_ref().and_then(|z| z.get(k).copied().flatten())))?;
            grow_block(&mut blocks, sh, None, area);
            size_refs[i] = Some(format!("{}{}", t.val.prefix, area.abs()));
        }

        for &i in &new_series {
            let s = &edit.series[i];
            let n = lens[i];
            let a = st.free_across(sh, o, lo, hi, cursor)?;
            cursor = a + 1;
            // The name, in the template's header row (column for series in rows).
            let name_ref = match name_along {
                Some(na) => {
                    let (r, c) = o.rc(na, a);
                    let src = o.rc(na, neighbour);
                    st.put(sh, r, c, NewVal::Label(s.name.clone()), Some(src))?;
                    let area = Area::cell(r, c);
                    grow_block(&mut blocks, sh, None, area);
                    format!("{}{}", t.val.prefix, area.abs())
                }
                None => return None,
            };
            let val_area = st.write_new_line(sh, o, (val_start, a), n, neighbour, |k| num_val(s.values.get(k).copied().flatten()))?;
            grow_block(&mut blocks, sh, None, val_area);
            // Categories (or X values) shared with the template series.
            let cat_ref = match &t.cat {
                None => String::new(),
                Some(r) => {
                    if edit.scatter {
                        let cells = cat_cells(edit, s, n);
                        let area = st.write_line(sh, o, &r.area, &cells)?;
                        grow_block(&mut blocks, sh, Some(r.area), area);
                        format!("{}{}", r.prefix, area.abs())
                    } else {
                        refs[template].as_ref()?.cat.clone()
                    }
                }
            };
            if s.sizes.is_some() {
                let b = st.free_across(sh, o, lo, hi, cursor)?;
                cursor = b + 1;
                let size_src = t.size.as_ref().map(|r| o.across(r.area.r1, r.area.c1)).unwrap_or(a);
                let area = st.write_new_line(sh, o, (val_start, b), n, size_src, |k| num_val(s.sizes.as_ref().and_then(|z| z.get(k).copied().flatten())))?;
                grow_block(&mut blocks, sh, None, area);
                size_refs[i] = Some(format!("{}{}", t.val.prefix, area.abs()));
            }
            neighbour = a;
            refs[i] = Some(SeriesRefs { name: name_ref, cat: cat_ref, val: format!("{}{}", t.val.prefix, val_area.abs()) });
        }
    }
    let refs: Vec<SeriesRefs> = refs.into_iter().collect::<Option<_>>()?;

    // ---- tables -----------------------------------------------------------------------------------
    let mut changed: HashMap<String, Vec<u8>> = HashMap::new();
    let touched: Vec<usize> = {
        let mut v: Vec<usize> = st.writes.keys().map(|k| k.0).collect();
        v.dedup();
        v
    };
    for &sh in &touched {
        let (old_b, new_b) = blocks.get(&sh).copied().unwrap_or((None, None));
        for (part, text) in st.edit_tables(sh, old_b, new_b)? {
            changed.insert(part, text.into_bytes());
        }
    }

    // ---- sheets -----------------------------------------------------------------------------------
    let mut removed: HashMap<String, HashSet<String>> = HashMap::new();
    for &sh in &touched {
        let (text, gone) = st.render_sheet(sh)?;
        if let Some(text) = text {
            let info = &st.infos[st.loaded[sh].info];
            if !gone.is_empty() {
                let mut keys = vec![(st.loaded[sh].info + 1).to_string()];
                keys.extend(info.id.clone());
                for k in keys {
                    removed.entry(k).or_default().extend(gone.iter().cloned());
                }
            }
            changed.insert(info.part.clone(), text.into_bytes());
        }
    }
    if !removed.is_empty() {
        if let Some(part) = st.calc_chain.clone() {
            if let Some(text) = st.zip.text(&part) {
                if let Some(t) = edit_calc_chain(&text, &removed)? {
                    changed.insert(part, t.into_bytes());
                }
            }
        }
    }

    if st.zip.over {
        return None;
    }
    let size_refs = size_refs.into_iter().enumerate().map(|(i, r)| if edit.series[i].sizes.is_some() { r } else { None }).collect();
    if changed.is_empty() {
        return Some(Patched { bytes: book.to_vec(), refs, size_refs });
    }
    let bytes = rezip(book, &changed)?;
    Some(Patched { bytes, refs, size_refs })
}

fn num_val(v: Option<f64>) -> NewVal {
    match v {
        Some(x) if x.is_finite() => NewVal::Num(x),
        _ => NewVal::Gap,
    }
}

/// The cells of a series' category (or X) range.
fn cat_cells(edit: &SheetEdit, s: &ChartSeries, n: u32) -> Vec<NewVal> {
    (0..n as usize)
        .map(|k| {
            if edit.scatter {
                match &s.x_values {
                    Some(x) => num_val(x.get(k).copied().flatten()),
                    None => {
                        if k < s.values.len() {
                            NewVal::Num((k + 1) as f64)
                        } else {
                            NewVal::Gap
                        }
                    }
                }
            } else {
                NewVal::Label(edit.categories.get(k).cloned().unwrap_or_default())
            }
        })
        .collect()
}

fn grow_block(blocks: &mut HashMap<usize, (Option<Area>, Option<Area>)>, sh: usize, old: Option<Area>, new: Area) {
    let e = blocks.entry(sh).or_insert((None, None));
    if let Some(old) = old {
        e.0 = Some(e.0.map_or(old, |b| b.union(&old)));
    }
    e.1 = Some(e.1.map_or(new, |b| b.union(&new)));
}

mod parts;
mod refs;
mod scan;
mod sheet;
mod state;

use parts::*;
use refs::*;
use scan::*;
use sheet::*;
use state::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs() {
        let r = parse_ref("Sheet1!$B$2:$B$5").unwrap();
        assert_eq!((r.sheet.as_str(), r.prefix.as_str()), ("Sheet1", "Sheet1!"));
        assert_eq!(r.area, Area { r1: 2, c1: 2, r2: 5, c2: 2 });
        let r = parse_ref("'Bob''s data'!$A$1").unwrap();
        assert_eq!(r.sheet, "Bob's data");
        assert_eq!(r.prefix, "'Bob''s data'!");
        assert_eq!(r.area.abs(), "$A$1");
        assert!(parse_ref("[1]Sheet1!$A$1").is_none());
        assert!(parse_ref("Sheet1:Sheet3!$A$1").is_none());
        assert!(parse_ref("(Sheet1!$A$1,Sheet1!$A$3)").is_none());
        assert!(parse_ref("Sheet1!$A:$A").is_none());
        assert!(parse_ref("Total").is_none());
        assert!(parse_ref("Sheet1!Total").is_none());
        assert_eq!(parse_cell("XFD1048576"), Some((1_048_576, 16_384)));
        assert_eq!(parse_cell("XFE1"), None);
        assert_eq!(Area { r1: 2, c1: 28, r2: 7, c2: 28 }.abs(), "$AB$2:$AB$7");
    }

    #[test]
    fn scanner() {
        let s = r#"<?xml version="1.0"?><!-- x --><a k='v>' j = "1"><b/></a>"#;
        let t = next_tag(s, 0).unwrap().unwrap();
        assert_eq!((t.name, t.kind), ("a", Kind::Open));
        assert_eq!(t.attr("k"), Some("v>"));
        assert_eq!(t.attr("j"), Some("1"));
        let b = next_tag(s, t.end).unwrap().unwrap();
        assert_eq!((b.name, b.kind), ("b", Kind::Empty));
        assert_eq!(unescape("a&amp;b&#x41;&#66;").unwrap(), "a&bAB");
        assert_eq!(splice("abcdef", vec![(2, 4, "X".into()), (2, 2, "<".into()), (6, 6, "!".into())]).unwrap(), "ab<Xef!");
        assert!(splice("abc", vec![(0, 2, "x".into()), (1, 3, "y".into())]).is_none());
    }

    #[test]
    fn sheet_parse() {
        let x = parse_sheet(r#"<worksheet xmlns="m"><dimension ref="A1:B2"/><sheetData><row r="1" spans="1:2"><c r="A1" t="inlineStr"><is><r><t>a</t></r><rPh><t>ph</t></rPh></is></c><c r="B1" s="2"><f>1+1</f><v>2</v></c></row><row r="3"/></sheetData></worksheet>"#.into()).unwrap();
        assert_eq!(x.rows.len(), 2);
        assert_eq!(x.rows[0].cells[0].inline.as_deref(), Some("a"));
        assert_eq!(x.rows[0].cells[1].v.as_deref(), Some("2"));
        assert!(x.rows[1].empty);
        assert!(parse_sheet(r#"<worksheet><sheetData><row r="2"/><row r="1"/></sheetData></worksheet>"#.into()).is_none());
        assert!(parse_sheet(r#"<worksheet><sheetData><row r="1"><c><v>1</v></c></row></sheetData></worksheet>"#.into()).is_none());
    }

    #[test]
    fn calc_chain() {
        let text = r#"<calcChain><c r="B2" i="1"/><c r="B3"/><c r="B4" i="2"/></calcChain>"#;
        let mut removed = HashMap::new();
        removed.insert("1".to_string(), HashSet::from(["B2".to_string()]));
        let out = edit_calc_chain(text, &removed).unwrap().unwrap();
        assert_eq!(out, r#"<calcChain><c i="1" r="B3"/><c r="B4" i="2"/></calcChain>"#);
        removed.insert("2".to_string(), HashSet::from(["B4".to_string()]));
        removed.get_mut("1").unwrap().insert("B3".into());
        assert!(edit_calc_chain(text, &removed).is_none());
    }
}
