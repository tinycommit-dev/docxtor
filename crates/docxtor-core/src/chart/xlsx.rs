//! The workbook behind a chart: a minimal valid `.xlsx` holding the chart's data (categories in column A, one
//! series per column from B, names in row 1), so Word's "Edit Data" shows the numbers the chart draws.

use super::model::ChartSeries;
use crate::xml::{self, Element};
use std::io::{Cursor, Read, Write};

pub const SHEET: &str = "Sheet1";

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    Number(f64),
}

/// The spreadsheet ranges one series takes its data from.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesRefs {
    pub name: String,
    pub cat: String,
    pub val: String,
}

/// Column letters: 0 is `A`, 26 is `AA`.
pub fn col_name(mut n: usize) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s
}


fn range(col: &str, first: usize, last: usize) -> String {
    format!("{SHEET}!${col}${first}:${col}${last}")
}

/// Where the data goes in the sheet, and the cells.
pub struct Plan {
    pub cells: Vec<Vec<Cell>>,
    pub refs: Vec<SeriesRefs>,
}

/// Lay out a chart's data. Category charts: categories in A, series in B, C, ... Scatter charts: X values in A
/// and Y values in B, C, ... when every series shares the X values, else an X and a Y column per series.
pub fn plan(categories: &[String], series: &[ChartSeries], scatter: bool) -> Plan {
    let rows = if scatter { series.iter().map(|s| s.values.len().max(s.x_values.as_ref().map_or(0, |x| x.len()))).max().unwrap_or(0) } else { categories.len().max(series.iter().map(|s| s.values.len()).max().unwrap_or(0)) };
    let rows = rows.max(1);
    let shared_x = scatter && series.windows(2).all(|w| w[0].x_values == w[1].x_values);
    let width = if scatter && !shared_x { series.len() * 2 } else { series.len() + 1 };
    let mut cells = vec![vec![Cell::Empty; width.max(1)]; rows + 1];
    let mut refs = Vec::new();
    let last = rows + 1;
    if !scatter {
        for (i, c) in categories.iter().enumerate() {
            cells[i + 1][0] = Cell::Text(c.clone());
        }
        for (si, s) in series.iter().enumerate() {
            let col = si + 1;
            cells[0][col] = Cell::Text(s.name.clone());
            for (i, v) in s.values.iter().enumerate() {
                if let Some(v) = v {
                    cells[i + 1][col] = Cell::Number(*v);
                }
            }
            let cn = col_name(col);
            refs.push(SeriesRefs { name: format!("{SHEET}!${cn}$1"), cat: range("A", 2, last), val: range(&cn, 2, last) });
        }
    } else if shared_x {
        cells[0][0] = Cell::Text("X".into());
        if let Some(x) = series.first().and_then(|s| s.x_values.as_ref()) {
            for (i, v) in x.iter().enumerate() {
                if let Some(v) = v {
                    cells[i + 1][0] = Cell::Number(*v);
                }
            }
        }
        for (si, s) in series.iter().enumerate() {
            let col = si + 1;
            cells[0][col] = Cell::Text(s.name.clone());
            for (i, v) in s.values.iter().enumerate() {
                if let Some(v) = v {
                    cells[i + 1][col] = Cell::Number(*v);
                }
            }
            let cn = col_name(col);
            refs.push(SeriesRefs { name: format!("{SHEET}!${cn}$1"), cat: range("A", 2, last), val: range(&cn, 2, last) });
        }
    } else {
        for (si, s) in series.iter().enumerate() {
            let (xc, yc) = (si * 2, si * 2 + 1);
            cells[0][xc] = Cell::Text(format!("{} (X)", s.name));
            cells[0][yc] = Cell::Text(s.name.clone());
            for (i, v) in s.x_values.iter().flatten().enumerate() {
                if let Some(v) = v {
                    cells[i + 1][xc] = Cell::Number(*v);
                }
            }
            for (i, v) in s.values.iter().enumerate() {
                if let Some(v) = v {
                    cells[i + 1][yc] = Cell::Number(*v);
                }
            }
            let (xn, yn) = (col_name(xc), col_name(yc));
            refs.push(SeriesRefs { name: format!("{SHEET}!${yn}$1"), cat: range(&xn, 2, last), val: range(&yn, 2, last) });
        }
    }
    Plan { cells, refs }
}

/// Lay out a bubble chart's data: X values in A (shared) or one X column per series, then a Y and a size column
/// per series. Returns the plan and the range of each series' sizes.
pub fn plan_bubble(series: &[ChartSeries]) -> (Plan, Vec<Option<String>>) {
    let rows = series.iter().map(|s| s.values.len().max(s.x_values.as_ref().map_or(0, |x| x.len())).max(s.sizes.as_ref().map_or(0, |x| x.len()))).max().unwrap_or(0).max(1);
    let shared_x = series.windows(2).all(|w| w[0].x_values == w[1].x_values);
    let width = if shared_x { 1 + series.len() * 2 } else { series.len() * 3 };
    let mut cells = vec![vec![Cell::Empty; width.max(1)]; rows + 1];
    let (mut refs, mut sizes) = (Vec::new(), Vec::new());
    let last = rows + 1;
    let put = |cells: &mut Vec<Vec<Cell>>, col: usize, head: String, vals: &[Option<f64>]| {
        cells[0][col] = Cell::Text(head);
        for (i, v) in vals.iter().enumerate() {
            if let Some(v) = v {
                cells[i + 1][col] = Cell::Number(*v);
            }
        }
    };
    if shared_x {
        put(&mut cells, 0, "X-Values".into(), series.first().and_then(|s| s.x_values.as_deref()).unwrap_or(&[]));
    }
    for (si, s) in series.iter().enumerate() {
        let (xc, yc, zc) = if shared_x { (0, 1 + si * 2, 2 + si * 2) } else { (si * 3, si * 3 + 1, si * 3 + 2) };
        if !shared_x {
            put(&mut cells, xc, format!("{} (X)", s.name), s.x_values.as_deref().unwrap_or(&[]));
        }
        put(&mut cells, yc, s.name.clone(), &s.values);
        put(&mut cells, zc, format!("{} (size)", s.name), s.sizes.as_deref().unwrap_or(&[]));
        let (xn, yn, zn) = (col_name(xc), col_name(yc), col_name(zc));
        refs.push(SeriesRefs { name: format!("{SHEET}!${yn}$1"), cat: range(&xn, 2, last), val: range(&yn, 2, last) });
        sizes.push(Some(range(&zn, 2, last)));
    }
    (Plan { cells, refs }, sizes)
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";

/// The bytes of a minimal workbook holding `plan.cells` in `Sheet1`.
pub fn build(plan: &Plan) -> Vec<u8> {
    let mut strings: Vec<String> = Vec::new();
    let mut rows = String::new();
    let width = plan.cells.iter().map(|r| r.len()).max().unwrap_or(1);
    for (ri, row) in plan.cells.iter().enumerate() {
        let mut cells = String::new();
        for (ci, c) in row.iter().enumerate() {
            let r = format!("{}{}", col_name(ci), ri + 1);
            match c {
                Cell::Empty => {}
                Cell::Text(t) => {
                    let idx = strings.iter().position(|s| s == t).unwrap_or_else(|| {
                        strings.push(t.clone());
                        strings.len() - 1
                    });
                    cells.push_str(&format!("<c r=\"{r}\" t=\"s\"><v>{idx}</v></c>"));
                }
                Cell::Number(n) => cells.push_str(&format!("<c r=\"{r}\"><v>{n}</v></c>")),
            }
        }
        if !cells.is_empty() {
            rows.push_str(&format!("<row r=\"{}\">{cells}</row>", ri + 1));
        }
    }
    let dim = format!("A1:{}{}", col_name(width.saturating_sub(1)), plan.cells.len().max(1));
    let count: usize = plan.cells.iter().flatten().filter(|c| matches!(c, Cell::Text(_))).count();
    let sst: String = strings.iter().map(|s| format!("<si><t xml:space=\"preserve\">{}</t></si>", esc(s))).collect();

    let parts: Vec<(&str, String)> = vec![
        (
            "[Content_Types].xml",
            format!(
                "{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/></Types>"
            ),
        ),
        (
            "_rels/.rels",
            format!("{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL_BASE}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>"),
        ),
        (
            "xl/workbook.xml",
            format!("{DECL}<workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\"><bookViews><workbookView xWindow=\"0\" yWindow=\"0\" windowWidth=\"16000\" windowHeight=\"9000\"/></bookViews><sheets><sheet name=\"{SHEET}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            format!("{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL_BASE}/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"{REL_BASE}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId3\" Type=\"{REL_BASE}/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>"),
        ),
        (
            "xl/worksheets/sheet1.xml",
            format!("{DECL}<worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\"><dimension ref=\"{dim}\"/><sheetViews><sheetView tabSelected=\"1\" workbookViewId=\"0\"/></sheetViews><sheetFormatPr defaultRowHeight=\"15\"/><sheetData>{rows}</sheetData><pageMargins left=\"0.7\" right=\"0.7\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/></worksheet>"),
        ),
        (
            "xl/styles.xml",
            format!("{DECL}<styleSheet xmlns=\"{NS_MAIN}\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>"),
        ),
        ("xl/sharedStrings.xml", format!("{DECL}<sst xmlns=\"{NS_MAIN}\" count=\"{count}\" uniqueCount=\"{}\">{sst}</sst>", strings.len())),
    ];

    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in parts {
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).last_modified_time(zip::DateTime::default());
        w.start_file(name, opts).expect("zip entry");
        w.write_all(body.as_bytes()).expect("zip write");
    }
    w.finish().expect("zip finish").into_inner()
}

/// The cells of the first sheet of a workbook (`xl/worksheets/sheet1.xml`), row by row.
pub fn read_cells(bytes: &[u8]) -> Option<Vec<Vec<Cell>>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut f = zip.by_name(name).ok()?;
        let mut v = Vec::new();
        f.read_to_end(&mut v).ok()?;
        Some(v)
    };
    let strings: Vec<String> = read("xl/sharedStrings.xml")
        .and_then(|b| xml::parse(&b).ok())
        .map(|d| d.root.children_named("si").map(|si| si.text()).collect())
        .unwrap_or_default();
    let sheet = xml::parse(&read("xl/worksheets/sheet1.xml")?).ok()?;
    let data = sheet.root.child("sheetData")?;
    let mut grid: Vec<Vec<Cell>> = Vec::new();
    // A chart's data is a small table: rows, columns and cells past the limits a recipient list has are left out, so a
    // damaged workbook can't make the engine allocate billions of empty cells.
    use crate::budget::caps::{LIST_CELLS, LIST_COLS, LIST_ROWS};
    let mut cells = 0usize;
    for (ri, row) in data.children_named("row").enumerate() {
        let r = match row.attr("r") {
            Some(v) => match v.parse::<usize>().ok().and_then(|v| v.checked_sub(1)) {
                Some(r) => r,
                None => continue,
            },
            None => ri,
        };
        if r >= LIST_ROWS {
            continue;
        }
        while grid.len() <= r {
            grid.push(Vec::new());
        }
        for c in row.children_named("c") {
            let col = match c.attr("r") {
                Some(name) => match crate::merge::xlsx::column_of(name) {
                    Some(col) => col,
                    None => continue,
                },
                None => grid[r].len(),
            };
            if col >= LIST_COLS || cells >= LIST_CELLS {
                continue;
            }
            cells += 1;
            let cell = cell_of(c, &strings);
            if grid[r].len() <= col {
                grid[r].resize(col + 1, Cell::Empty);
            }
            grid[r][col] = cell;
        }
    }
    let width = grid.iter().map(|r| r.len()).max().unwrap_or(0);
    for row in &mut grid {
        row.resize(width, Cell::Empty);
    }
    Some(grid)
}

fn cell_of(c: &Element, strings: &[String]) -> Cell {
    let v = c.child("v").map(|v| v.text());
    match c.attr("t") {
        Some("s") => v.and_then(|i| i.trim().parse::<usize>().ok()).and_then(|i| strings.get(i).cloned()).map(Cell::Text).unwrap_or(Cell::Empty),
        Some("inlineStr") => Cell::Text(c.child("is").map(|i| i.text()).unwrap_or_default()),
        Some("str") => v.map(Cell::Text).unwrap_or(Cell::Empty),
        _ => v.and_then(|v| v.trim().parse::<f64>().ok()).map(Cell::Number).unwrap_or(Cell::Empty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_names() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(27), "AB");
        assert_eq!(crate::merge::xlsx::column_of("AB12"), Some(27));
        // A damaged name is not a column, and does not overflow.
        assert_eq!(crate::merge::xlsx::column_of("ZZZZZZZZZZZZZZZZZZZZ1"), None);
        assert_eq!(crate::merge::xlsx::column_of("1"), None);
    }

    #[test]
    fn build_and_read_back() {
        let s = |n: &str, v: &[f64]| ChartSeries { name: n.into(), values: v.iter().map(|x| Some(*x)).collect(), ..Default::default() };
        let cats: Vec<String> = ["Q1", "Q2", "Q3"].iter().map(|c| c.to_string()).collect();
        let p = plan(&cats, &[s("Sales", &[1.0, 2.5, 3.0]), s("Costs", &[0.5, 1.0, 1.5])], false);
        assert_eq!(p.refs[0].val, "Sheet1!$B$2:$B$4");
        assert_eq!(p.refs[1].name, "Sheet1!$C$1");
        assert_eq!(p.refs[1].cat, "Sheet1!$A$2:$A$4");
        let bytes = build(&p);
        let grid = read_cells(&bytes).unwrap();
        assert_eq!(grid[0][1], Cell::Text("Sales".into()));
        assert_eq!(grid[2][0], Cell::Text("Q2".into()));
        assert_eq!(grid[2][1], Cell::Number(2.5));
        assert_eq!(grid[3][2], Cell::Number(1.5));
    }
}
