//! Reading a list of recipients from an Excel workbook (.xlsx): the first sheet, or the one asked for, with the
//! first row as the header. The workbook is a zip of XML parts, which Docxtor's package reader and XML reader open;
//! no spreadsheet library is needed. Values come out as the text the cell holds: numbers as written, dates as
//! `M/D/YYYY`, booleans as TRUE and FALSE. Formulas give their last calculated value. Number formats such as currency
//! and percent are not applied, which is what Word's own mail merge does with the OLE DB reader too.

use super::Table;
use crate::field_fmt::Dt;
use crate::package::{self, Package};
use crate::xml::Element;

/// Rows beyond this are not read.
pub const MAX_ROWS: usize = crate::budget::caps::LIST_ROWS;
/// Columns beyond this are not read (Excel's own limit is 16,384, XFD).
const MAX_COLS: usize = crate::budget::caps::LIST_COLS;
/// The most cells (rows times the width of the header) the table may hold; rows beyond that are left out.
const MAX_CELLS: usize = crate::budget::caps::LIST_CELLS;

/// What `read` found.
#[derive(Debug, Clone)]
pub struct Workbook {
    pub table: Table,
    /// The names of all sheets, in order.
    pub sheets: Vec<String>,
    /// The sheet the table came from.
    pub sheet: String,
    /// Whether rows beyond `MAX_ROWS` were left out.
    pub truncated: bool,
}

fn text_of_si(si: &Element) -> String {
    // A shared string is plain (`t`) or rich (`r` runs, each with a `t`); phonetic runs (`rPh`) are not part of it.
    fn walk(e: &Element, out: &mut String) {
        for c in e.elements() {
            match c.local() {
                "t" => out.push_str(&c.text()),
                "r" => walk(c, out),
                _ => {}
            }
        }
    }
    let mut out = String::new();
    walk(si, &mut out);
    out
}

/// `A` is 0, `B` is 1, `AA` is 26. A reference of more than three letters is not a column (the last is XFD). The one
/// column reader for workbooks: recipient lists and chart data both use it.
pub(crate) fn column_of(reference: &str) -> Option<usize> {
    let mut n = 0usize;
    let mut letters = 0;
    for c in reference.chars().take_while(|c| c.is_ascii_alphabetic()) {
        letters += 1;
        if letters > 3 {
            return None;
        }
        n = n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    n.checked_sub(1)
}

/// Whether a number format code is a date or time (Excel's built-in 14 to 22 and 45 to 47, or a custom code).
fn is_date_format(id: u32, code: Option<&str>) -> bool {
    if let Some(code) = code {
        // Drop quoted text, escaped characters and colour or condition brackets; what is left says what the number is.
        let mut plain = String::new();
        let mut it = code.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '"' => {
                    for q in it.by_ref() {
                        if q == '"' {
                            break;
                        }
                    }
                }
                '\\' | '_' | '*' => {
                    it.next();
                }
                '[' => {
                    let mut inner = String::new();
                    for q in it.by_ref() {
                        if q == ']' {
                            break;
                        }
                        inner.push(q);
                    }
                    // Elapsed time (`[h]`) is a duration, shown as a date-time here.
                    if matches!(inner.to_ascii_lowercase().as_str(), "h" | "m" | "s" | "hh" | "mm" | "ss") {
                        plain.push('h');
                    }
                }
                c => plain.push(c.to_ascii_lowercase()),
            }
        }
        return plain.chars().any(|c| matches!(c, 'y' | 'd' | 'h' | 's')) || (plain.contains('m') && !plain.contains('0') && !plain.contains('#'));
    }
    matches!(id, 14..=22 | 45..=47)
}

fn date_text(serial: f64, date1904: bool) -> String {
    // Excel's dates end on 31 December 9999 (serial 2,958,465); anything else is not a date.
    if !serial.is_finite() || !(0.0..=2_958_466.0).contains(&serial) {
        return serial.to_string();
    }
    let days = serial.floor();
    let frac = serial - days;
    // Excel's 1900 system counts a 29 February 1900 that never was; from 1 March on, day 25569 is 1 January 1970.
    let unix_days = if date1904 {
        days - 24107.0
    } else if days < 61.0 {
        days - 25568.0
    } else {
        days - 25569.0
    };
    let secs = unix_days as i64 * 86_400 + (frac * 86_400.0).round() as i64;
    let dt = Dt::from_unix(secs);
    if frac.abs() < 1e-9 {
        format!("{}/{}/{}", dt.month, dt.day, dt.year)
    } else {
        format!("{}/{}/{} {}:{:02}", dt.month, dt.day, dt.year, dt.hour, dt.minute)
    }
}

fn number_text(v: &str) -> String {
    let t = v.trim();
    if t.contains(['e', 'E']) {
        if let Ok(x) = t.parse::<f64>() {
            return format!("{x}");
        }
    }
    t.to_string()
}

fn cell_value(c: &Element, shared: &[String], date_styles: &[bool], date1904: bool) -> String {
    let kind = c.attr("t").unwrap_or("n");
    let v = c.child("v").map(|v| v.text());
    match kind {
        "s" => v.and_then(|i| i.trim().parse::<usize>().ok()).and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
        "inlineStr" => c.child("is").map(text_of_si).unwrap_or_default(),
        "str" => v.unwrap_or_default(),
        "b" => match v.as_deref().map(str::trim) {
            Some("1") => "TRUE".into(),
            Some(_) => "FALSE".into(),
            None => String::new(),
        },
        "e" => v.unwrap_or_default(),
        "d" => v
            .and_then(|s| Dt::parse_iso(&s))
            .map(|d| format!("{}/{}/{}", d.month, d.day, d.year))
            .unwrap_or_default(),
        _ => {
            let Some(v) = v else { return String::new() };
            let style = c.attr("s").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            if date_styles.get(style).copied().unwrap_or(false) {
                if let Ok(x) = v.trim().parse::<f64>() {
                    return date_text(x, date1904);
                }
            }
            number_text(&v)
        }
    }
}

/// Read a workbook. `sheet` names the sheet to read; without it the first one is.
pub fn read(bytes: &[u8], sheet: Option<&str>) -> Result<Workbook, String> {
    crate::budget::within(bytes.len(), crate::budget::caps::LIST_BYTES).map_err(|e| e.to_string())?;
    // A list is small: it gets the small budget for what it inflates.
    let pkg = Package::open_with(bytes, &crate::budget::Budget::small()).map_err(|_| "This isn't an Excel (.xlsx) file.".to_string())?;
    let wb_path = pkg
        .rels_for("")
        .ok()
        .and_then(|rels| rels.into_iter().find(|r| r.rel_type == package::REL_OFFICE_DOCUMENT))
        .map(|r| package::resolve_target("", &r.target))
        .unwrap_or_else(|| "xl/workbook.xml".into());
    let wb = pkg.xml(&wb_path).map_err(|_| "This isn't an Excel (.xlsx) file.".to_string())?;
    let rels = pkg.rels_for(&wb_path).unwrap_or_default();
    let date1904 = wb.root.elements().find(|e| e.local() == "workbookPr").and_then(|e| e.attr("date1904")).is_some_and(|v| matches!(v, "1" | "true"));
    let mut sheets: Vec<(String, String)> = Vec::new();
    for s in wb.root.elements().filter(|e| e.local() == "sheets").flat_map(|e| e.elements()) {
        let name = s.attr("name").unwrap_or_default().to_string();
        let rid = s.attr("r:id").or_else(|| s.attrs.iter().find(|(k, _)| k.ends_with(":id")).map(|(_, v)| v.as_str())).unwrap_or_default();
        if let Some(rel) = rels.iter().find(|r| r.id == rid) {
            sheets.push((name, package::resolve_target(&wb_path, &rel.target)));
        }
    }
    if sheets.is_empty() {
        return Err("The workbook has no sheets.".into());
    }
    let names: Vec<String> = sheets.iter().map(|(n, _)| n.clone()).collect();
    let (name, path) = match sheet {
        Some(want) => sheets.iter().find(|(n, _)| n.eq_ignore_ascii_case(want)).cloned().ok_or_else(|| format!("The workbook has no sheet called {want}."))?,
        None => sheets[0].clone(),
    };
    // Shared strings and styles are optional parts.
    let shared: Vec<String> = pkg
        .rels_for(&wb_path)
        .unwrap_or_default()
        .iter()
        .find(|r| r.rel_type.ends_with("/sharedStrings"))
        .map(|r| package::resolve_target(&wb_path, &r.target))
        .and_then(|p| pkg.xml(&p).ok())
        .map(|d| d.root.elements().filter(|e| e.local() == "si").map(text_of_si).collect())
        .unwrap_or_default();
    let date_styles: Vec<bool> = rels
        .iter()
        .find(|r| r.rel_type.ends_with("/styles"))
        .map(|r| package::resolve_target(&wb_path, &r.target))
        .and_then(|p| pkg.xml(&p).ok())
        .map(|d| {
            let custom: Vec<(u32, String)> = d
                .root
                .elements()
                .filter(|e| e.local() == "numFmts")
                .flat_map(|e| e.elements())
                .filter_map(|e| Some((e.attr("numFmtId")?.parse().ok()?, e.attr("formatCode")?.to_string())))
                .collect();
            d.root
                .elements()
                .filter(|e| e.local() == "cellXfs")
                .flat_map(|e| e.elements())
                .map(|xf| {
                    let id: u32 = xf.attr("numFmtId").and_then(|v| v.parse().ok()).unwrap_or(0);
                    is_date_format(id, custom.iter().find(|(i, _)| *i == id).map(|(_, c)| c.as_str()))
                })
                .collect()
        })
        .unwrap_or_default();
    let doc = pkg.xml(&path).map_err(|_| format!("The sheet {name} could not be read."))?;
    // Cells are kept sparse while reading: a row with one cell at XFD1 is one cell, not 16,384.
    let mut grid: Vec<Vec<(usize, String)>> = Vec::new();
    let mut truncated = false;
    for data in doc.root.elements().filter(|e| e.local() == "sheetData") {
        for (n, row) in data.elements().filter(|e| e.local() == "row").enumerate() {
            let at = row.attr("r").and_then(|r| r.parse::<usize>().ok()).map(|r| r.saturating_sub(1)).unwrap_or(n);
            if at >= MAX_ROWS {
                truncated = true;
                break;
            }
            let mut cells: Vec<(usize, String)> = Vec::new();
            let mut next = 0usize;
            for c in row.elements().filter(|e| e.local() == "c") {
                let col = match c.attr("r") {
                    Some(r) => match column_of(r) {
                        Some(col) => col,
                        None => continue,
                    },
                    None => next,
                };
                if col >= MAX_COLS {
                    continue;
                }
                let v = cell_value(c, &shared, &date_styles, date1904);
                match cells.iter_mut().find(|(i, _)| *i == col) {
                    Some(slot) => slot.1 = v,
                    None => cells.push((col, v)),
                }
                next = col + 1;
            }
            if grid.len() <= at {
                grid.resize(at + 1, Vec::new());
            }
            grid[at] = cells;
        }
    }
    grid.retain(|r| r.iter().any(|(_, v)| !v.trim().is_empty()));
    if grid.is_empty() {
        return Err(format!("The sheet {name} has no rows."));
    }
    // The header row sets the width: cells to its right have no column to go to.
    let width = grid[0].iter().filter(|(_, v)| !v.trim().is_empty()).map(|(i, _)| i + 1).max().unwrap_or(0);
    if grid.len() * width > MAX_CELLS {
        grid.truncate((MAX_CELLS / width.max(1)).max(2));
        truncated = true;
    }
    let mut grid: Vec<Vec<String>> = grid
        .into_iter()
        .map(|cells| {
            let mut row = vec![String::new(); width];
            for (i, v) in cells {
                if i < width {
                    row[i] = v;
                }
            }
            row
        })
        .collect();
    let header = grid.remove(0);
    Ok(Workbook { table: Table::from_rows(header, grid), sheets: names, sheet: name, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, data) in parts {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    /// A workbook with two sheets, shared strings, an inline string, a boolean, a date and a number.
    pub(crate) fn sample() -> Vec<u8> {
        zip(&[
            ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#),
            ("_rels/.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#),
            ("xl/workbook.xml", r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="People" sheetId="1" r:id="rId1"/><sheet name="Other" sheetId="2" r:id="rId2"/></sheets></workbook>"#),
            ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#),
            ("xl/sharedStrings.xml", r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>First Name</t></si><si><t>Joined</t></si><si><t>Active</t></si><si><r><t>Ad</t></r><r><t>a</t></r></si><si><t>Score</t></si></sst>"#),
            ("xl/styles.xml", r#"<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="yyyy\-mm\-dd"/></numFmts><cellXfs count="3"><xf numFmtId="0"/><xf numFmtId="14"/><xf numFmtId="164"/></cellXfs></styleSheet>"#),
            (
                "xl/worksheets/sheet1.xml",
                r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c><c r="E1" t="s"><v>4</v></c></row>
<row r="2"><c r="A2" t="s"><v>3</v></c><c r="B2" s="1"><v>46308</v></c><c r="C2" t="b"><v>1</v></c><c r="E2"><v>12.50</v></c></row>
<row r="3"><c r="A3" t="inlineStr"><is><t>Grace</t></is></c><c r="B3" s="2"><v>45000.75</v></c><c r="C3" t="b"><v>0</v></c><c r="E3"><v>1.5E-2</v></c></row>
</sheetData></worksheet>"#,
            ),
            ("xl/worksheets/sheet2.xml", r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>City</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Paris</t></is></c></row></sheetData></worksheet>"#),
        ])
    }

    #[test]
    fn reads_the_first_sheet() {
        let wb = read(&sample(), None).unwrap();
        assert_eq!(wb.sheets, ["People", "Other"]);
        assert_eq!(wb.sheet, "People");
        // The empty column D becomes a named column, so the gap keeps its place.
        assert_eq!(wb.table.columns, ["First Name", "Joined", "Active", "Column 4", "Score"]);
        assert_eq!(wb.table.rows[0], ["Ada", "10/13/2026", "TRUE", "", "12.50"]);
        assert_eq!(wb.table.rows[1], ["Grace", "3/15/2023 18:00", "FALSE", "", "0.015"]);
        assert!(!wb.truncated);
    }

    /// A workbook with one sheet holding the rows given as XML.
    fn with_rows(rows: &str, styles: bool) -> Vec<u8> {
        let ct = r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#;
        let root = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
        let wb = r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
        let st = r#"<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14"/></cellXfs></styleSheet>"#;
        let sheet = format!(r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#);
        let mut parts = vec![("[Content_Types].xml", ct), ("_rels/.rels", root), ("xl/workbook.xml", wb), ("xl/_rels/workbook.xml.rels", rels), ("xl/worksheets/sheet1.xml", sheet.as_str())];
        if styles {
            parts.push(("xl/styles.xml", st));
        }
        zip(&parts)
    }

    #[test]
    fn a_far_cell_in_every_row_does_not_pad_the_rows() {
        // Each row holds one cell at XFD; the header is one column wide, so the table is too.
        let mut rows = String::from(r#"<row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c></row>"#);
        for r in 2..2000 {
            rows.push_str(&format!(r#"<row r="{r}"><c r="XFD{r}"><v>1</v></c><c r="A{r}" t="inlineStr"><is><t>n{r}</t></is></c></row>"#));
        }
        let wb = read(&with_rows(&rows, false), None).unwrap();
        assert_eq!(wb.table.columns, ["Name"]);
        assert_eq!(wb.table.rows.len(), 1998);
        assert_eq!(wb.table.rows[0], ["n2"]);
    }

    #[test]
    fn crafted_references_and_dates_are_data_not_crashes() {
        let rows = r#"<row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>When</t></is></c></row>
<row r="2"><c r="ZZZZZZZZZZZZZZZZ2" t="inlineStr"><is><t>lost</t></is></c><c r="A2" t="inlineStr"><is><t>Ada</t></is></c><c r="B2" s="1"><v>1e300</v></c></row>"#;
        let wb = read(&with_rows(rows, true), None).unwrap();
        assert_eq!(wb.table.rows.len(), 1);
        assert_eq!(wb.table.rows[0][0], "Ada");
        assert!(!wb.table.rows[0][1].contains('/'));
    }

    #[test]
    fn reads_a_named_sheet_and_refuses_the_rest() {
        let wb = read(&sample(), Some("other")).unwrap();
        assert_eq!((wb.table.columns.clone(), wb.table.rows.clone()), (vec!["City".to_string()], vec![vec!["Paris".to_string()]]));
        assert!(read(&sample(), Some("Nope")).is_err());
        assert!(read(b"not a zip", None).is_err());
    }

    #[test]
    fn dates() {
        assert_eq!(date_text(1.0, false), "1/1/1900");
        assert_eq!(date_text(61.0, false), "3/1/1900");
        assert_eq!(date_text(25569.0, false), "1/1/1970");
        assert_eq!(date_text(0.0, true), "1/1/1904");
        assert!(is_date_format(14, None) && !is_date_format(0, None));
        assert!(is_date_format(164, Some("yyyy\\-mm\\-dd")) && is_date_format(165, Some("[$-409]d-mmm-yy")) && !is_date_format(166, Some("#,##0.00")) && !is_date_format(167, Some("\"d\"0")));
        assert_eq!(column_of("AB12"), Some(27));
        assert_eq!(column_of("XFD1"), Some(16_383));
        assert_eq!(column_of("ZZZZZZZZZZZZZZZZ1"), None);
        // Dates out of Excel's range are left as the number they are, not turned into garbage or a panic.
        assert!(!date_text(1e300, false).contains('/') && !date_text(f64::NAN, true).contains('/') && !date_text(-5.0, false).contains('/'));
    }
}
