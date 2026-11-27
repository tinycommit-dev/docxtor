//! Editing the workbook behind a chart in place: only the cells the chart uses change, everything else in the
//! workbook (other cells, sheets, formulas, styles, tables, defined names) stays byte-identical.

use docxtor_core::chart::xlsx::{self, Cell, SeriesRefs};
use docxtor_core::chart::xlsx_patch::{patch, Patched, SheetEdit};
use docxtor_core::chart::ChartSeries;
use std::io::{Cursor, Read, Write};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";
const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const RELNS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// A zip with these entries, in order. `docProps/app.xml` is stored, the rest deflated.
fn make_zip(entries: &[(String, String)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in entries {
        let method = if name == "docProps/app.xml" { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        let opts = zip::write::SimpleFileOptions::default().compression_method(method);
        w.start_file(name.as_str(), opts).unwrap();
        w.write_all(body.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// (name, compression, raw bytes, decompressed text) of every entry, in order.
fn entries(bytes: &[u8]) -> Vec<(String, zip::CompressionMethod, Vec<u8>, String)> {
    let mut ar = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    for i in 0..ar.len() {
        let (name, method, raw) = {
            let mut f = ar.by_index_raw(i).unwrap();
            let mut raw = Vec::new();
            f.read_to_end(&mut raw).unwrap();
            (f.name().unwrap().to_string(), f.compression(), raw)
        };
        let mut f = ar.by_index(i).unwrap();
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        out.push((name, method, raw, String::from_utf8(data).unwrap()));
    }
    out
}

fn part(bytes: &[u8], name: &str) -> String {
    entries(bytes).into_iter().find(|e| e.0 == name).unwrap_or_else(|| panic!("no {name}")).3
}

/// Every XML part parses with end-name checks on.
fn assert_well_formed(bytes: &[u8]) {
    for (name, _, _, text) in entries(bytes) {
        if !(name.ends_with(".xml") || name.ends_with(".rels")) {
            continue;
        }
        let mut r = quick_xml::Reader::from_str(&text);
        let mut depth = 0i32;
        loop {
            match r.read_event() {
                Ok(quick_xml::events::Event::Start(_)) => depth += 1,
                Ok(quick_xml::events::Event::End(_)) => depth -= 1,
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("{name} is not well-formed: {e}\n{text}"),
            }
        }
        assert_eq!(depth, 0, "{name} has unclosed elements");
    }
}

/// The entries other than `changed` are byte-identical (raw compressed bytes, method, order).
fn assert_untouched_except(before: &[u8], after: &[u8], changed: &[&str]) {
    let a = entries(before);
    let b = entries(after);
    assert_eq!(a.iter().map(|e| &e.0).collect::<Vec<_>>(), b.iter().map(|e| &e.0).collect::<Vec<_>>(), "entry order");
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.1, y.1, "{} compression", x.0);
        if !changed.contains(&x.0.as_str()) {
            assert_eq!(x.2, y.2, "{} raw bytes", x.0);
        } else {
            assert_ne!(x.3, y.3, "{} should have changed", x.0);
        }
    }
}

/// The span of `<c r="B3" ...>...</c>` (or `<c .../>`).
fn cell_span(text: &str, r: &str) -> Option<(usize, usize)> {
    let start = text.find(&format!("<c r=\"{r}\""))?;
    let gt = start + text[start..].find('>')?;
    if text.as_bytes()[gt - 1] == b'/' {
        return Some((start, gt + 1));
    }
    Some((start, gt + text[gt..].find("</c>")? + 4))
}

fn cell<'a>(text: &'a str, r: &str) -> &'a str {
    let (a, b) = cell_span(text, r).unwrap_or_else(|| panic!("no cell {r}"));
    &text[a..b]
}

fn without(text: &str, cells: &[&str]) -> String {
    let mut t = text.to_string();
    for r in cells {
        if let Some((a, b)) = cell_span(&t, r) {
            t.replace_range(a..b, "");
        }
    }
    t
}

fn ser(name: &str, values: &[Option<f64>]) -> ChartSeries {
    ChartSeries { name: name.into(), values: values.to_vec(), ..Default::default() }
}

fn nums(v: &[f64]) -> Vec<Option<f64>> {
    v.iter().map(|x| Some(*x)).collect()
}

fn refs(name: &str, cat: &str, val: &str) -> SeriesRefs {
    SeriesRefs { name: name.into(), cat: cat.into(), val: val.into() }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn run(book: &[u8], cats: &[String], series: &[ChartSeries], old: &[Option<SeriesRefs>]) -> Option<Patched> {
    patch(book, &SheetEdit { categories: cats, series, scatter: false, old_refs: old, old_size_refs: &[] })
}

fn t(s: &str) -> Cell {
    Cell::Text(s.into())
}
fn n(x: f64) -> Cell {
    Cell::Number(x)
}

// ---------------------------------------------------------------------------
// A workbook like Excel makes
// ---------------------------------------------------------------------------

const SST: [&str; 11] = ["Quarter", "Sales", "Costs", "Note", "Double", "Q1", "Q2", "Q3", "Q4", "good", "bad"];

fn sheet1_xml() -> String {
    let mut rows = String::from("<row r=\"1\" spans=\"1:5\" x14ac:dyDescent=\"0.25\">");
    for (i, c) in ["A", "B", "C", "D", "E"].iter().enumerate() {
        rows.push_str(&format!("<c r=\"{c}1\" t=\"s\"><v>{i}</v></c>"));
    }
    rows.push_str("</row>");
    let sales = [10, 20, 30, 40];
    let costs = [5, 6, 7, 8];
    for k in 0..4 {
        let r = k + 2;
        rows.push_str(&format!(
            "<row r=\"{r}\" spans=\"1:5\" x14ac:dyDescent=\"0.25\"><c r=\"A{r}\" t=\"s\"><v>{}</v></c><c r=\"B{r}\" s=\"1\"><v>{}</v></c><c r=\"C{r}\" s=\"1\"><v>{}</v></c><c r=\"D{r}\" t=\"s\"><v>{}</v></c><c r=\"E{r}\" s=\"1\"><f>B{r}*2</f><v>{}</v></c></row>",
            5 + k,
            sales[k],
            costs[k],
            9 + k % 2,
            sales[k] * 2
        ));
    }
    format!(
        "{DECL}<worksheet xmlns=\"{MAIN}\" xmlns:r=\"{RELNS}\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"x14ac\" xmlns:x14ac=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac\"><dimension ref=\"A1:E5\"/><sheetViews><sheetView tabSelected=\"1\" workbookViewId=\"0\"/></sheetViews><sheetFormatPr defaultRowHeight=\"15\" x14ac:dyDescent=\"0.25\"/><cols><col min=\"1\" max=\"1\" width=\"14\" customWidth=\"1\"/></cols><sheetData>{rows}</sheetData><conditionalFormatting sqref=\"B2:B5\"><cfRule type=\"cellIs\" dxfId=\"0\" priority=\"1\" operator=\"greaterThan\"><formula>100</formula></cfRule></conditionalFormatting><pageMargins left=\"0.7\" right=\"0.7\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/><tableParts count=\"1\"><tablePart r:id=\"rId1\"/></tableParts></worksheet>"
    )
}

fn table_xml(r: &str, cols: &[&str]) -> String {
    let cs: String = cols.iter().enumerate().map(|(i, c)| format!("<tableColumn id=\"{}\" name=\"{c}\"/>", i + 1)).collect();
    format!(
        "{DECL}<table xmlns=\"{MAIN}\" id=\"1\" name=\"Table1\" displayName=\"Table1\" ref=\"{r}\" totalsRowShown=\"0\"><autoFilter ref=\"{r}\"/><tableColumns count=\"{}\">{cs}</tableColumns><tableStyleInfo name=\"TableStyleMedium2\" showFirstColumn=\"0\" showLastColumn=\"0\" showRowStripes=\"1\" showColumnStripes=\"0\"/></table>",
        cols.len()
    )
}

fn excel_book_with(sheet1: String, table: String, calc_chain: Option<String>) -> Vec<u8> {
    let sst: String = SST.iter().map(|s| format!("<si><t>{s}</t></si>")).collect();
    let mut e: Vec<(String, String)> = vec![
        (
            "[Content_Types].xml".into(),
            format!("{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/worksheets/sheet2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/tables/table1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/><Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/><Override PartName=\"/xl/calcChain.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml\"/><Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/></Types>"),
        ),
        ("_rels/.rels".into(), format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/officeDocument\" Target=\"xl/workbook.xml\"/><Relationship Id=\"rId2\" Type=\"{RELNS}/extended-properties\" Target=\"docProps/app.xml\"/></Relationships>")),
        ("docProps/app.xml".into(), format!("{DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>Microsoft Excel</Application></Properties>")),
        (
            "xl/workbook.xml".into(),
            format!("{DECL}<workbook xmlns=\"{MAIN}\" xmlns:r=\"{RELNS}\"><bookViews><workbookView xWindow=\"0\" yWindow=\"0\" windowWidth=\"16000\" windowHeight=\"9000\"/></bookViews><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Other\" sheetId=\"2\" r:id=\"rId2\"/></sheets><definedNames><definedName name=\"Total\">Sheet1!$B$2:$B$5</definedName></definedNames><calcPr calcId=\"191029\"/></workbook>"),
        ),
        (
            "xl/_rels/workbook.xml.rels".into(),
            format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"{RELNS}/worksheet\" Target=\"worksheets/sheet2.xml\"/><Relationship Id=\"rId3\" Type=\"{RELNS}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId4\" Type=\"{RELNS}/sharedStrings\" Target=\"sharedStrings.xml\"/><Relationship Id=\"rId5\" Type=\"{RELNS}/calcChain\" Target=\"calcChain.xml\"/></Relationships>"),
        ),
        ("xl/worksheets/sheet1.xml".into(), sheet1),
        ("xl/worksheets/_rels/sheet1.xml.rels".into(), format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/table\" Target=\"../tables/table1.xml\"/></Relationships>")),
        ("xl/worksheets/sheet2.xml".into(), format!("{DECL}<worksheet xmlns=\"{MAIN}\"><dimension ref=\"A1\"/><sheetData><row r=\"1\"><c r=\"A1\"><v>42</v></c></row></sheetData></worksheet>")),
        ("xl/tables/table1.xml".into(), table),
        (
            "xl/styles.xml".into(),
            format!("{DECL}<styleSheet xmlns=\"{MAIN}\"><numFmts count=\"1\"><numFmt numFmtId=\"164\" formatCode=\"#,##0.0\"/></numFmts><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"2\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/><xf numFmtId=\"164\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/></cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles><dxfs count=\"1\"><dxf><font><b/></font></dxf></dxfs></styleSheet>"),
        ),
        ("xl/sharedStrings.xml".into(), format!("{DECL}<sst xmlns=\"{MAIN}\" count=\"13\" uniqueCount=\"11\">{sst}</sst>")),
    ];
    if let Some(cc) = calc_chain {
        e.push(("xl/calcChain.xml".into(), cc));
    }
    make_zip(&e)
}

fn calc_chain_e() -> String {
    format!("{DECL}<calcChain xmlns=\"{MAIN}\"><c r=\"E2\" i=\"1\"/><c r=\"E3\"/><c r=\"E4\"/><c r=\"E5\"/></calcChain>")
}

fn excel_book() -> Vec<u8> {
    excel_book_with(sheet1_xml(), table_xml("A1:C5", &["Quarter", "Sales", "Costs"]), Some(calc_chain_e()))
}

fn excel_refs() -> Vec<Option<SeriesRefs>> {
    vec![Some(refs("Sheet1!$B$1", "Sheet1!$A$2:$A$5", "Sheet1!$B$2:$B$5")), Some(refs("Sheet1!$C$1", "Sheet1!$A$2:$A$5", "Sheet1!$C$2:$C$5"))]
}

const SHEET: &str = "xl/worksheets/sheet1.xml";
const TABLE: &str = "xl/tables/table1.xml";

// ---------------------------------------------------------------------------
// (1) A small edit changes only the cells it names
// ---------------------------------------------------------------------------

#[test]
fn small_edit_keeps_everything_else() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3 (est)", "Q4"]);
    let series = [ser("Sales", &nums(&[10.0, 25.0, 30.0, 40.0])), ser("Expenses", &nums(&[5.0, 6.0, 7.0, 8.0]))];
    let p = run(&book, &cats, &series, &excel_refs()).expect("patched");
    assert_eq!(p.refs, excel_refs().into_iter().flatten().collect::<Vec<_>>());
    assert_well_formed(&p.bytes);
    // The sheet and the table (its column follows the renamed header) change; nothing else does.
    assert_untouched_except(&book, &p.bytes, &[SHEET, TABLE]);

    let before = part(&book, SHEET);
    let after = part(&p.bytes, SHEET);
    assert_eq!(without(&before, &["B3", "C1", "A4"]), without(&after, &["B3", "C1", "A4"]));
    assert_eq!(cell(&after, "B3"), "<c r=\"B3\" s=\"1\"><v>25</v></c>");
    assert_eq!(cell(&after, "A4"), "<c r=\"A4\" t=\"inlineStr\"><is><t xml:space=\"preserve\">Q3 (est)</t></is></c>");
    assert_eq!(cell(&after, "C1"), "<c r=\"C1\" t=\"inlineStr\"><is><t xml:space=\"preserve\">Expenses</t></is></c>");
    // The formula column is kept.
    assert_eq!(cell(&after, "E3"), "<c r=\"E3\" s=\"1\"><f>B3*2</f><v>40</v></c>");
    assert!(after.contains("<conditionalFormatting sqref=\"B2:B5\">"));

    let table = part(&p.bytes, TABLE);
    assert_eq!(table, part(&book, TABLE).replace("name=\"Costs\"", "name=\"Expenses\""));
}

#[test]
fn unchanged_data_returns_the_same_bytes() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &nums(&[10.0, 20.0, 30.0, 40.0])), ser("Costs", &nums(&[5.0, 6.0, 7.0, 8.0]))];
    let p = run(&book, &cats, &series, &excel_refs()).unwrap();
    assert_eq!(p.bytes, book);
}

#[test]
fn a_part_that_inflates_past_the_limit_is_refused_not_read() {
    // The workbook sits in the document, so it is untrusted: a small compressed part that inflates to tens of
    // megabytes is where the patch gives up (the caller then writes a fresh workbook) instead of reading it.
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &nums(&[1.0, 2.0, 3.0, 4.0])), ser("Costs", &nums(&[5.0, 6.0, 7.0, 8.0]))];
    let sst = format!("{DECL}<sst xmlns=\"{MAIN}\">{}</sst>", " ".repeat(40 << 20));
    let mut parts: Vec<(String, String)> = entries(&excel_book()).into_iter().map(|e| (e.0, e.3)).collect();
    for p in parts.iter_mut().filter(|p| p.0 == "xl/sharedStrings.xml") {
        p.1 = sst.clone();
    }
    let book = make_zip(&parts);
    assert!(book.len() < 1 << 20, "the bomb is small as stored: {}", book.len());
    assert!(run(&book, &cats, &series, &excel_refs()).is_none(), "the shared strings were read");
    // The same workbook without the bomb is patched.
    assert!(run(&excel_book(), &cats, &series, &excel_refs()).is_some());
}

// ---------------------------------------------------------------------------
// (2) Growing, (3) shrinking
// ---------------------------------------------------------------------------

#[test]
fn grow_by_two_rows() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3", "Q4", "Q5", "Q6"]);
    let series = [ser("Sales", &nums(&[10.0, 20.0, 30.0, 40.0, 50.0, 60.5])), ser("Costs", &nums(&[5.0, 6.0, 7.0, 8.0, 9.0, 10.0]))];
    let p = run(&book, &cats, &series, &excel_refs()).unwrap();
    assert_eq!(p.refs[0], refs("Sheet1!$B$1", "Sheet1!$A$2:$A$7", "Sheet1!$B$2:$B$7"));
    assert_eq!(p.refs[1], refs("Sheet1!$C$1", "Sheet1!$A$2:$A$7", "Sheet1!$C$2:$C$7"));
    assert_well_formed(&p.bytes);
    assert_untouched_except(&book, &p.bytes, &[SHEET, TABLE]);

    let before = part(&book, SHEET);
    let after = part(&p.bytes, SHEET);
    // Rows 1 to 5, D and E included, are untouched.
    let rows = |s: &str| s[s.find("<row r=\"1\"").unwrap()..s.find("<row r=\"5\"").map(|i| i + s[i..].find("</row>").unwrap() + 6).unwrap()].to_string();
    assert_eq!(rows(&before), rows(&after));
    assert!(after.contains("<dimension ref=\"A1:E7\"/>"));
    assert_eq!(cell(&after, "B6"), "<c r=\"B6\" s=\"1\"><v>50</v></c>");
    assert_eq!(cell(&after, "B7"), "<c r=\"B7\" s=\"1\"><v>60.5</v></c>");
    assert_eq!(cell(&after, "A7"), "<c r=\"A7\" t=\"inlineStr\"><is><t xml:space=\"preserve\">Q6</t></is></c>");
    assert!(after.contains("<row r=\"6\"><c r=\"A6\""));
    assert!(after.contains("</row></sheetData>"));
    assert_eq!(after.matches("<c r=\"D").count(), 5, "column D gets no new cells");

    let table = part(&p.bytes, TABLE);
    assert!(table.contains("ref=\"A1:C7\""));
    assert!(table.contains("<autoFilter ref=\"A1:C7\"/>"));
    assert!(table.contains("<tableColumns count=\"3\">"));

    let grid = read_sheet1(&p.bytes);
    assert_eq!(grid[6][0..3], [t("Q6"), n(60.5), n(10.0)]);
}

#[test]
fn shrink_leaves_the_leftover_cells() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2"]);
    let series = [ser("Sales", &nums(&[10.0, 21.0])), ser("Costs", &nums(&[5.0, 6.0]))];
    let p = run(&book, &cats, &series, &excel_refs()).unwrap();
    assert_eq!(p.refs[0], refs("Sheet1!$B$1", "Sheet1!$A$2:$A$3", "Sheet1!$B$2:$B$3"));
    assert_eq!(p.refs[1].val, "Sheet1!$C$2:$C$3");
    assert_untouched_except(&book, &p.bytes, &[SHEET]);
    let after = part(&p.bytes, SHEET);
    assert_eq!(without(&part(&book, SHEET), &["B3"]), without(&after, &["B3"]));
    assert_eq!(cell(&after, "B5"), "<c r=\"B5\" s=\"1\"><v>40</v></c>");
    // The table is never shrunk.
    assert_eq!(part(&p.bytes, TABLE), part(&book, TABLE));
}

// ---------------------------------------------------------------------------
// (4) Adding a series, (5) removing one
// ---------------------------------------------------------------------------

#[test]
fn add_series_skips_occupied_columns() {
    // The table covers A1:E5, so the first free column F is right at its edge.
    let book = excel_book_with(sheet1_xml(), table_xml("A1:E5", &["Quarter", "Sales", "Costs", "Note", "Double"]), Some(calc_chain_e()));
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &nums(&[10.0, 20.0, 30.0, 40.0])), ser("Costs", &nums(&[5.0, 6.0, 7.0, 8.0])), ser("Profit", &nums(&[5.0, 14.0, 23.0, 32.0]))];
    let mut old = excel_refs();
    old.push(None);
    let p = run(&book, &cats, &series, &old).unwrap();
    assert_eq!(p.refs[2], refs("Sheet1!$F$1", "Sheet1!$A$2:$A$5", "Sheet1!$F$2:$F$5"));
    assert_well_formed(&p.bytes);
    assert_untouched_except(&book, &p.bytes, &[SHEET, TABLE]);
    let after = part(&p.bytes, SHEET);
    assert_eq!(cell(&after, "F1"), "<c r=\"F1\" t=\"inlineStr\"><is><t xml:space=\"preserve\">Profit</t></is></c>");
    assert_eq!(cell(&after, "F3"), "<c r=\"F3\" s=\"1\"><v>14</v></c>");
    assert!(after.contains("<row r=\"2\" spans=\"1:6\""));
    assert!(after.contains("<dimension ref=\"A1:F5\"/>"));
    let table = part(&p.bytes, TABLE);
    assert!(table.contains("ref=\"A1:F5\""));
    assert!(table.contains("<autoFilter ref=\"A1:F5\"/>"));
    assert!(table.contains("<tableColumns count=\"6\">"));
    assert!(table.contains("<tableColumn id=\"5\" name=\"Double\"/><tableColumn id=\"6\" name=\"Profit\"/></tableColumns>"));
}

#[test]
fn add_series_beyond_other_data_leaves_the_table() {
    // Table over A1:C5 and other data in D and E: the new column F is not next to the table.
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &nums(&[10.0, 20.0, 30.0, 40.0])), ser("Costs", &nums(&[5.0, 6.0, 7.0, 8.0])), ser("Profit", &nums(&[1.0, 2.0, 3.0, 4.0]))];
    let mut old = excel_refs();
    old.push(None);
    let p = run(&book, &cats, &series, &old).unwrap();
    assert_eq!(p.refs[2].val, "Sheet1!$F$2:$F$5");
    assert_untouched_except(&book, &p.bytes, &[SHEET]);
}

#[test]
fn add_series_to_a_plain_block() {
    // No table, and an empty column right after the data.
    let mut sheet = sheet1_xml();
    sheet = sheet.replace("<tableParts count=\"1\"><tablePart r:id=\"rId1\"/></tableParts>", "");
    let p0 = excel_book_with(sheet, table_xml("A1:C5", &["Quarter", "Sales", "Costs"]), None);
    let cats = strings(&["Q1", "Q2", "Q3", "Q4", "Q5"]);
    let series = [ser("Sales", &nums(&[1.0, 2.0, 3.0, 4.0, 5.0])), ser("Costs", &nums(&[1.0, 1.0, 1.0, 1.0, 1.0])), ser("New", &nums(&[9.0, 9.0]))];
    let mut old = excel_refs();
    old.push(None);
    let p = run(&p0, &cats, &series, &old).unwrap();
    // D and E hold data in rows 1 to 5, so the new series goes to F, with five rows like the others.
    assert_eq!(p.refs[2], refs("Sheet1!$F$1", "Sheet1!$A$2:$A$6", "Sheet1!$F$2:$F$6"));
    let grid = read_sheet1(&p.bytes);
    assert_eq!(grid[0][5], t("New"));
    assert_eq!(grid[2][5], n(9.0));
    assert_eq!(grid[3][5], Cell::Empty);
    assert_eq!(grid[5][0..3], [t("Q5"), n(5.0), n(1.0)]);
    assert_well_formed(&p.bytes);
}

#[test]
fn remove_series_keeps_its_cells() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &nums(&[10.0, 20.0, 30.0, 40.0]))];
    let p = run(&book, &cats, &series, &excel_refs()[..1]).unwrap();
    assert_eq!(p.refs.len(), 1);
    assert_eq!(p.refs[0].val, "Sheet1!$B$2:$B$5");
    assert_eq!(p.bytes, book);
}

// ---------------------------------------------------------------------------
// (6) Series in rows
// ---------------------------------------------------------------------------

fn simple_book(sheet_name_xml: &str, sheet: &str) -> Vec<u8> {
    let e: Vec<(String, String)> = vec![
        ("[Content_Types].xml".into(), format!("{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>")),
        ("_rels/.rels".into(), format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>")),
        ("xl/workbook.xml".into(), format!("{DECL}<workbook xmlns=\"{MAIN}\" xmlns:r=\"{RELNS}\"><sheets><sheet name=\"{sheet_name_xml}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>")),
        ("xl/_rels/workbook.xml.rels".into(), format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/worksheet\" Target=\"/xl/worksheets/sheet1.xml\"/></Relationships>")),
        ("xl/worksheets/sheet1.xml".into(), format!("{DECL}<worksheet xmlns=\"{MAIN}\"><dimension ref=\"A1:D3\"/><sheetData>{sheet}</sheetData></worksheet>")),
    ];
    make_zip(&e)
}

fn read_sheet1(bytes: &[u8]) -> Vec<Vec<Cell>> {
    xlsx::read_cells(bytes).expect("readable")
}

#[test]
fn series_in_rows() {
    let rows = "<row r=\"1\"><c r=\"B1\" t=\"inlineStr\"><is><t>Jan</t></is></c><c r=\"C1\" t=\"inlineStr\"><is><t>Feb</t></is></c><c r=\"D1\" t=\"inlineStr\"><is><t>Mar</t></is></c></row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>North</t></is></c><c r=\"B2\" s=\"3\"><v>1</v></c><c r=\"C2\" s=\"3\"><v>2</v></c><c r=\"D2\" s=\"3\"><v>3</v></c></row><row r=\"3\"><c r=\"A3\" t=\"inlineStr\"><is><t>South</t></is></c><c r=\"B3\"><v>4</v></c><c r=\"C3\"><v>5</v></c><c r=\"D3\"><v>6</v></c></row>";
    let book = simple_book("Data", rows);
    let old = vec![Some(refs("Data!$A$2", "Data!$B$1:$D$1", "Data!$B$2:$D$2")), Some(refs("Data!$A$3", "Data!$B$1:$D$1", "Data!$B$3:$D$3"))];
    let cats = strings(&["Jan", "Feb", "Mar", "Apr"]);
    let series = [ser("North", &nums(&[1.0, 2.0, 3.0, 3.5])), ser("South", &nums(&[4.0, 5.0, 6.0, 6.5])), ser("East", &nums(&[7.0, 8.0, 9.0, 10.0]))];
    let mut old3 = old.clone();
    old3.push(None);
    let p = run(&book, &cats, &series, &old3).unwrap();
    assert_eq!(p.refs[0], refs("Data!$A$2", "Data!$B$1:$E$1", "Data!$B$2:$E$2"));
    assert_eq!(p.refs[1].val, "Data!$B$3:$E$3");
    assert_eq!(p.refs[2], refs("Data!$A$4", "Data!$B$1:$E$1", "Data!$B$4:$E$4"));
    assert_well_formed(&p.bytes);
    let after = part(&p.bytes, SHEET);
    assert_eq!(cell(&after, "E2"), "<c r=\"E2\" s=\"3\"><v>3.5</v></c>");
    assert!(after.contains("<dimension ref=\"A1:E4\"/>"));
    let grid = read_sheet1(&p.bytes);
    assert_eq!(grid[0], vec![Cell::Empty, t("Jan"), t("Feb"), t("Mar"), t("Apr")]);
    assert_eq!(grid[1], vec![t("North"), n(1.0), n(2.0), n(3.0), n(3.5)]);
    assert_eq!(grid[3], vec![t("East"), n(7.0), n(8.0), n(9.0), n(10.0)]);
    // The new row's cells copy the styles of the row above.
    assert_eq!(cell(&after, "B4"), "<c r=\"B4\"><v>7</v></c>");
    assert_eq!(cell(&after, "A4"), "<c r=\"A4\" t=\"inlineStr\"><is><t xml:space=\"preserve\">East</t></is></c>");
}

// ---------------------------------------------------------------------------
// (7) Quoted sheet names
// ---------------------------------------------------------------------------

#[test]
fn quoted_sheet_names() {
    let rows = "<row r=\"1\"><c r=\"B1\" t=\"inlineStr\"><is><t>A</t></is></c></row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>x</t></is></c><c r=\"B2\"><v>1</v></c></row><row r=\"3\"><c r=\"A3\" t=\"inlineStr\"><is><t>y</t></is></c><c r=\"B3\"><v>2</v></c></row>";
    let book = simple_book("Bob&apos;s data", rows);
    let old = vec![Some(refs("'Bob''s data'!$B$1", "'Bob''s data'!$A$2:$A$3", "'Bob''s data'!$B$2:$B$3")), None];
    let cats = strings(&["x", "y", "z"]);
    let series = [ser("A", &nums(&[1.0, 2.0, 3.0])), ser("B & C", &nums(&[4.0, 5.0, 6.0]))];
    let p = run(&book, &cats, &series, &old).unwrap();
    assert_eq!(p.refs[0], refs("'Bob''s data'!$B$1", "'Bob''s data'!$A$2:$A$4", "'Bob''s data'!$B$2:$B$4"));
    assert_eq!(p.refs[1], refs("'Bob''s data'!$C$1", "'Bob''s data'!$A$2:$A$4", "'Bob''s data'!$C$2:$C$4"));
    assert_well_formed(&p.bytes);
    let grid = read_sheet1(&p.bytes);
    assert_eq!(grid[0][2], t("B & C"));
    assert_eq!(grid[3], vec![t("z"), n(3.0), n(6.0)]);

    // A name with spaces, matched without regard to case.
    let book = simple_book("My Sheet", rows);
    let old = vec![Some(refs("'my sheet'!$B$1", "'my sheet'!$A$2:$A$3", "'my sheet'!$B$2:$B$3"))];
    let p = run(&book, &cats, &series[..1], &old).unwrap();
    assert_eq!(p.refs[0].val, "'my sheet'!$B$2:$B$4");
}

// ---------------------------------------------------------------------------
// (8) Formulas and the calculation chain
// ---------------------------------------------------------------------------

#[test]
fn formulas_and_calc_chain() {
    let mut rows = String::new();
    rows.push_str("<row r=\"1\"><c r=\"B1\" t=\"inlineStr\"><is><t>Twice</t></is></c><c r=\"C1\" t=\"inlineStr\"><is><t>Base</t></is></c><c r=\"D1\" t=\"inlineStr\"><is><t>Plus</t></is></c></row>");
    rows.push_str("<row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>a</t></is></c><c r=\"B2\"><f t=\"shared\" ref=\"B2:B4\" si=\"0\">C2*2</f><v>2</v></c><c r=\"C2\"><v>1</v></c><c r=\"D2\"><f>C2+1</f><v>2</v></c></row>");
    rows.push_str("<row r=\"3\"><c r=\"A3\" t=\"inlineStr\"><is><t>b</t></is></c><c r=\"B3\"><f t=\"shared\" si=\"0\"/><v>4</v></c><c r=\"C3\"><v>2</v></c><c r=\"D3\"><f>C3+1</f><v>3</v></c></row>");
    rows.push_str("<row r=\"4\"><c r=\"A4\" t=\"inlineStr\"><is><t>c</t></is></c><c r=\"B4\"><f t=\"shared\" si=\"0\"/><v>6</v></c><c r=\"C4\"><v>3</v></c><c r=\"D4\"><f>C4+1</f><v>4</v></c></row>");
    let sheet = format!("{DECL}<worksheet xmlns=\"{MAIN}\" xmlns:r=\"{RELNS}\"><dimension ref=\"A1:D4\"/><sheetData>{rows}</sheetData></worksheet>");
    let chain = format!("{DECL}<calcChain xmlns=\"{MAIN}\"><c r=\"B2\" i=\"1\"/><c r=\"B3\"/><c r=\"B4\"/><c r=\"D2\"/><c r=\"D3\"/><c r=\"D4\"/></calcChain>");
    let book = excel_book_with(sheet,table_xml("A1:C5", &["Quarter", "Sales", "Costs"]), Some(chain));
    let old = vec![Some(refs("Sheet1!$B$1", "Sheet1!$A$2:$A$4", "Sheet1!$B$2:$B$4")), Some(refs("Sheet1!$D$1", "Sheet1!$A$2:$A$4", "Sheet1!$D$2:$D$4"))];
    let cats = strings(&["a", "b", "c"]);
    let series = [ser("Twice", &nums(&[9.0, 4.0, 7.0])), ser("Plus", &nums(&[2.0, 30.0, 4.0]))];
    let p = run(&book, &cats, &series, &old).unwrap();
    assert_well_formed(&p.bytes);
    assert_untouched_except(&book, &p.bytes, &[SHEET, "xl/calcChain.xml"]);
    let after = part(&p.bytes, SHEET);
    // The shared formula master keeps its formula; only the cached value changes.
    assert_eq!(cell(&after, "B2"), "<c r=\"B2\"><f t=\"shared\" ref=\"B2:B4\" si=\"0\">C2*2</f><v>9</v></c>");
    // Unchanged formula cells stay formulas.
    assert_eq!(cell(&after, "B3"), "<c r=\"B3\"><f t=\"shared\" si=\"0\"/><v>4</v></c>");
    assert_eq!(cell(&after, "D2"), "<c r=\"D2\"><f>C2+1</f><v>2</v></c>");
    // Changed ones become constants.
    assert_eq!(cell(&after, "B4"), "<c r=\"B4\"><v>7</v></c>");
    assert_eq!(cell(&after, "D3"), "<c r=\"D3\"><v>30</v></c>");
    let chain = part(&p.bytes, "xl/calcChain.xml");
    assert!(chain.ends_with("<calcChain xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><c r=\"B2\" i=\"1\"/><c r=\"B3\"/><c r=\"D2\"/><c r=\"D4\"/></calcChain>"), "{chain}");
}

#[test]
fn calc_chain_keeps_the_sheet_of_the_next_entry() {
    let rows = "<row r=\"1\"><c r=\"B1\" t=\"inlineStr\"><is><t>S</t></is></c></row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>a</t></is></c><c r=\"B2\"><f>1+1</f><v>2</v></c></row><row r=\"3\"><c r=\"A3\" t=\"inlineStr\"><is><t>b</t></is></c><c r=\"B3\"><f>1+2</f><v>3</v></c></row>";
    let sheet = format!("{DECL}<worksheet xmlns=\"{MAIN}\"><dimension ref=\"A1:B3\"/><sheetData>{rows}</sheetData></worksheet>");
    let chain = format!("{DECL}<calcChain xmlns=\"{MAIN}\"><c r=\"B2\" i=\"1\"/><c r=\"B3\"/></calcChain>");
    let book = excel_book_with(sheet, table_xml("A1:C5", &["Quarter", "Sales", "Costs"]), Some(chain));
    let old = vec![Some(refs("Sheet1!$B$1", "Sheet1!$A$2:$A$3", "Sheet1!$B$2:$B$3"))];
    let p = run(&book, &strings(&["a", "b"]), &[ser("S", &nums(&[5.0, 3.0]))], &old).unwrap();
    let chain = part(&p.bytes, "xl/calcChain.xml");
    assert!(chain.contains("<calcChain xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><c i=\"1\" r=\"B3\"/></calcChain>"), "{chain}");
}

// ---------------------------------------------------------------------------
// (9) Gaps
// ---------------------------------------------------------------------------

#[test]
fn gap_clears_the_cell() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &[Some(10.0), None, Some(30.0), Some(40.0)]), ser("Costs", &nums(&[5.0, 6.0, 7.0, 8.0]))];
    let p = run(&book, &cats, &series, &excel_refs()).unwrap();
    let after = part(&p.bytes, SHEET);
    assert_eq!(cell(&after, "B3"), "<c r=\"B3\" s=\"1\"/>");
    assert_eq!(without(&part(&book, SHEET), &["B3"]), without(&after, &["B3"]));
    assert_eq!(read_sheet1(&p.bytes)[2][1], Cell::Empty);
    assert_well_formed(&p.bytes);
}

// ---------------------------------------------------------------------------
// (10) What it refuses
// ---------------------------------------------------------------------------

#[test]
fn unsupported_ranges_give_none() {
    let book = excel_book();
    let cats = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let series = [ser("Sales", &nums(&[1.0, 2.0, 3.0, 4.0]))];
    for val in [
        "(Sheet1!$B$2,Sheet1!$B$4)",
        "Sheet1!$B$2,Sheet1!$B$4",
        "Sheet1!$B$2:$C$5",
        "[1]Sheet1!$B$2:$B$5",
        "'[Book2.xlsx]Sheet1'!$B$2:$B$5",
        "Nope!$B$2:$B$5",
        "Total",
        "Sheet1!Total",
        "Sheet1:Other!$B$2:$B$5",
        "Sheet1!$B:$B",
        "Sheet1!#REF!",
    ] {
        let old = vec![Some(refs("Sheet1!$B$1", "Sheet1!$A$2:$A$5", val))];
        assert!(run(&book, &cats, &series, &old).is_none(), "{val}");
    }
    // A category range running the other way from the values.
    let old = vec![Some(refs("Sheet1!$B$1", "Sheet1!$A$1:$D$1", "Sheet1!$B$2:$B$5"))];
    assert!(run(&book, &cats, &series, &old).is_none());
    // Only new series: nothing to anchor them to.
    assert!(run(&book, &cats, &series, &[None]).is_none());
    // Not a zip.
    let old = excel_refs();
    assert!(run(b"not a zip", &cats, &series, &old[..1]).is_none());
    // A cell inside an array formula.
    let sheet = sheet1_xml().replace("<c r=\"B2\" s=\"1\"><v>10</v></c>", "<c r=\"B2\" s=\"1\"><f t=\"array\" ref=\"B2:B3\">{1;2}</f><v>10</v></c>");
    let arr = excel_book_with(sheet, table_xml("A1:C5", &["Quarter", "Sales", "Costs"]), Some(calc_chain_e()));
    assert!(run(&arr, &cats, &[ser("Sales", &nums(&[10.0, 99.0, 30.0, 40.0]))], &old[..1]).is_none());
}

// ---------------------------------------------------------------------------
// (11) Workbooks Docxtor made before
// ---------------------------------------------------------------------------

#[test]
fn docxtor_built_workbook() {
    let cats = strings(&["Q1", "Q2", "Q3"]);
    let first = [ser("Sales", &nums(&[1.0, 2.5, 3.0])), ser("Costs", &nums(&[0.5, 1.0, 1.5]))];
    let plan = xlsx::plan(&cats, &first, false);
    let book = xlsx::build(&plan);
    let old: Vec<Option<SeriesRefs>> = plan.refs.iter().cloned().map(Some).chain([None]).collect();
    let cats2 = strings(&["Q1", "Q2", "Q3", "Q4"]);
    let next = [ser("Sales", &nums(&[1.0, 2.0, 3.0, 4.0])), ser("Costs", &[Some(0.5), None, Some(1.5), Some(2.0)]), ser("Profit", &nums(&[0.1, 0.2, 0.3, 0.4]))];
    let p = run(&book, &cats2, &next, &old).unwrap();
    assert_eq!(p.refs, xlsx::plan(&cats2, &next, false).refs);
    assert_well_formed(&p.bytes);
    let grid = read_sheet1(&p.bytes);
    assert_eq!(
        grid,
        vec![
            vec![Cell::Empty, t("Sales"), t("Costs"), t("Profit")],
            vec![t("Q1"), n(1.0), n(0.5), n(0.1)],
            vec![t("Q2"), n(2.0), Cell::Empty, n(0.2)],
            vec![t("Q3"), n(3.0), n(1.5), n(0.3)],
            vec![t("Q4"), n(4.0), n(2.0), n(0.4)],
        ]
    );
    assert!(part(&p.bytes, SHEET).contains("<dimension ref=\"A1:D5\"/>"));
    assert_untouched_except(&book, &p.bytes, &[SHEET]);
}

// ---------------------------------------------------------------------------
// (12) Scatter, (13) bubble
// ---------------------------------------------------------------------------

fn scatter(name: &str, x: &[f64], y: &[f64]) -> ChartSeries {
    ChartSeries { name: name.into(), values: nums(y), x_values: Some(nums(x)), ..Default::default() }
}

#[test]
fn scatter_x_values() {
    let first = [scatter("A", &[1.0, 2.0, 3.0], &[10.0, 20.0, 30.0]), scatter("B", &[1.0, 2.0, 3.0], &[5.0, 6.0, 7.0])];
    let plan = xlsx::plan(&[], &first, true);
    let book = xlsx::build(&plan);
    let old: Vec<Option<SeriesRefs>> = plan.refs.iter().cloned().map(Some).collect();
    let next = [scatter("A", &[0.5, 2.0, 3.0, 4.0], &[10.0, 21.0, 30.0, 40.0]), scatter("B", &[0.5, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0])];
    let p = patch(&book, &SheetEdit { categories: &[], series: &next, scatter: true, old_refs: &old, old_size_refs: &[] }).unwrap();
    assert_eq!(p.refs[0], refs("Sheet1!$B$1", "Sheet1!$A$2:$A$5", "Sheet1!$B$2:$B$5"));
    let grid = read_sheet1(&p.bytes);
    assert_eq!(grid[1], vec![n(0.5), n(10.0), n(5.0)]);
    assert_eq!(grid[4], vec![n(4.0), n(40.0), n(8.0)]);
    // Series that disagree about shared X values can't be written.
    let bad = [scatter("A", &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]), scatter("B", &[7.0, 8.0, 9.0], &[1.0, 2.0, 3.0])];
    assert!(patch(&book, &SheetEdit { categories: &[], series: &bad, scatter: true, old_refs: &old, old_size_refs: &[] }).is_none());
}

#[test]
fn bubble_sizes() {
    let rows = "<row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>X</t></is></c><c r=\"B1\" t=\"inlineStr\"><is><t>Y</t></is></c><c r=\"C1\" t=\"inlineStr\"><is><t>Size</t></is></c></row><row r=\"2\"><c r=\"A2\"><v>1</v></c><c r=\"B2\"><v>10</v></c><c r=\"C2\" s=\"2\"><v>3</v></c></row><row r=\"3\"><c r=\"A3\"><v>2</v></c><c r=\"B3\"><v>20</v></c><c r=\"C3\" s=\"2\"><v>4</v></c></row>";
    let book = simple_book("Sheet1", rows);
    let old = vec![Some(refs("Sheet1!$B$1", "Sheet1!$A$2:$A$3", "Sheet1!$B$2:$B$3")), None];
    let old_sizes = vec![Some("Sheet1!$C$2:$C$3".to_string()), None];
    let mut a = scatter("Y", &[1.0, 2.0, 3.0], &[10.0, 20.0, 30.0]);
    a.sizes = Some(nums(&[3.0, 5.0, 6.0]));
    let mut b = scatter("Z", &[1.0, 2.0, 3.0], &[7.0, 8.0, 9.0]);
    b.sizes = Some(nums(&[1.0, 1.0, 2.0]));
    let series = [a, b];
    let p = patch(&book, &SheetEdit { categories: &[], series: &series, scatter: true, old_refs: &old, old_size_refs: &old_sizes }).unwrap();
    assert_eq!(p.refs[0], refs("Sheet1!$B$1", "Sheet1!$A$2:$A$4", "Sheet1!$B$2:$B$4"));
    assert_eq!(p.size_refs[0].as_deref(), Some("Sheet1!$C$2:$C$4"));
    // The new series takes the first free column, its sizes the next.
    assert_eq!(p.refs[1], refs("Sheet1!$D$1", "Sheet1!$A$2:$A$4", "Sheet1!$D$2:$D$4"));
    assert_eq!(p.size_refs[1].as_deref(), Some("Sheet1!$E$2:$E$4"));
    assert_well_formed(&p.bytes);
    let after = part(&p.bytes, SHEET);
    assert_eq!(cell(&after, "C4"), "<c r=\"C4\" s=\"2\"><v>6</v></c>");
    assert_eq!(cell(&after, "E2"), "<c r=\"E2\" s=\"2\"><v>1</v></c>");
    let grid = read_sheet1(&p.bytes);
    assert_eq!(grid[0], vec![t("X"), t("Y"), t("Size"), t("Z"), Cell::Empty]);
    assert_eq!(grid[3], vec![n(3.0), n(30.0), n(6.0), n(9.0), n(2.0)]);
    // Without sizes a series reports none.
    let plain = [scatter("Y", &[1.0, 2.0], &[10.0, 20.0])];
    let p = patch(&book, &SheetEdit { categories: &[], series: &plain, scatter: true, old_refs: &old[..1], old_size_refs: &old_sizes[..1] }).unwrap();
    assert_eq!(p.size_refs, vec![None]);
}

// ---------------------------------------------------------------------------
// Odd but valid sheets
// ---------------------------------------------------------------------------

#[test]
fn prefixed_sheet_and_numeric_categories() {
    // A sheet written with a namespace prefix, numeric category cells and an empty row.
    let sheet = format!(
        "{DECL}<x:worksheet xmlns:x=\"{MAIN}\"><x:dimension ref=\"A1:B3\"/><x:sheetData><x:row r=\"1\"><x:c r=\"B1\" t=\"inlineStr\"><x:is><x:t>V</x:t></x:is></x:c></x:row><x:row r=\"2\"><x:c r=\"A2\" s=\"4\"><x:v>2020</x:v></x:c><x:c r=\"B2\"><x:v>1</x:v></x:c></x:row><x:row r=\"3\"><x:c r=\"A3\" s=\"4\"><x:v>2021</x:v></x:c><x:c r=\"B3\"><x:v>2</x:v></x:c></x:row><x:row r=\"4\" customHeight=\"1\" ht=\"20\"/></x:sheetData></x:worksheet>"
    );
    let e: Vec<(String, String)> = vec![
        ("_rels/.rels".into(), format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>")),
        ("xl/workbook.xml".into(), format!("{DECL}<workbook xmlns=\"{MAIN}\" xmlns:r=\"{RELNS}\"><sheets><sheet name=\"S\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>")),
        ("xl/_rels/workbook.xml.rels".into(), format!("{DECL}<Relationships xmlns=\"{PKG_RELS}\"><Relationship Id=\"rId1\" Type=\"{RELNS}/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>")),
        ("xl/worksheets/sheet1.xml".into(), sheet),
    ];
    let book = make_zip(&e);
    let old = vec![Some(refs("S!$B$1", "S!$A$2:$A$3", "S!$B$2:$B$3"))];
    let p = run(&book, &strings(&["2020", "2021", "2022", "Later"]), &[ser("V", &nums(&[1.0, 2.0, 3.0, 4.0]))], &old).unwrap();
    assert_well_formed(&p.bytes);
    let after = part(&p.bytes, SHEET);
    // Unchanged numeric categories stay numbers; the new one is a number too, with the same style.
    assert!(after.contains("<x:c r=\"A2\" s=\"4\"><x:v>2020</x:v></x:c>"));
    assert!(after.contains("<x:row r=\"4\" customHeight=\"1\" ht=\"20\"><x:c r=\"A4\" s=\"4\"><x:v>2022</x:v></x:c><x:c r=\"B4\"><x:v>3</x:v></x:c></x:row>"), "{after}");
    assert!(after.contains("<x:row r=\"5\"><x:c r=\"A5\" s=\"4\" t=\"inlineStr\"><x:is><x:t xml:space=\"preserve\">Later</x:t></x:is></x:c>"), "{after}");
    assert!(after.contains("<x:dimension ref=\"A1:B5\"/>"));
}
