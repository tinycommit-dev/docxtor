//! Tests of: Stock and surface and the workbook and bars that become lines.

use super::*;

pub(super) fn stock_doc(variant: &str) -> (Document, Pos) {
    let names: &[&str] = match variant {
        "hlc" => &["High", "Low", "Close"],
        "ohlc" => &["Open", "High", "Low", "Close"],
        "vhlc" => &["Volume", "High", "Low", "Close"],
        _ => &["Volume", "Open", "High", "Low", "Close"],
    };
    let series: Vec<Value> = names.iter().enumerate().map(|(i, n)| json!({"name": n, "values": [10 + i, 12 + i, 11 + i, 14 + i]})).collect();
    chart_doc("stock", json!({"categories":["Mon","Tue","Wed","Thu"],"series":series,"options":{"stock":variant}}))
}

#[test]
fn the_four_stock_charts_are_written_and_read_back() {
    for variant in ["hlc", "ohlc", "vhlc", "vohlc"] {
        let (mut doc, _) = stock_doc(variant);
        let (_, m) = the_chart(&doc);
        assert_eq!(m.kind, ChartKind::Stock, "{variant}");
        assert_eq!(m.stock.as_deref(), Some(variant));
        assert!(!m.combo);
        let xml = chart_xml(&mut doc);
        assert!(xml.contains("<c:stockChart>") && xml.contains("<c:hiLowLines>"), "{variant}");
        assert_eq!(xml.contains("<c:upDownBars>"), variant.ends_with("ohlc"), "{variant}");
        assert_eq!(xml.contains("<c:barChart>"), variant.starts_with('v'), "{variant}");
        assert_eq!(count(&xml, "<c:valAx>"), if variant.starts_with('v') { 2 } else { 1 }, "{variant}");
        let again = Document::open(&doc.save().unwrap()).unwrap();
        assert_eq!(the_chart(&again).1.stock.as_deref(), Some(variant));
        if variant.starts_with('v') {
            let (_, m) = the_chart(&doc);
            assert!(!m.series[0].secondary && m.series[0].kind == ChartKind::Column);
            assert!(m.series[1].secondary && m.series[1].kind == ChartKind::Stock);
        }
    }
}

#[test]
fn a_stock_chart_needs_the_right_number_of_series() {
    let mut doc = doc_with("Intro\n\nBody");
    let p = find_p(&doc, "Body");
    let e = err(&mut doc, json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"stock","categories":["a","b"],"series":[{"name":"H","values":[1,2]},{"name":"L","values":[1,2]}]}));
    assert!(e.contains("needs 3 series (high, low, close)"), "{e}");
    let (mut doc, pos) = stock_doc("hlc");
    let e = err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"stock":"ohlc"}}));
    assert!(e.contains("needs 4 series"), "{e}");
}

#[test]
fn a_stock_chart_changes_variant_with_its_data_and_edits_values() {
    let (mut doc, pos) = stock_doc("hlc");
    let rows = |name: &str, base: i32| json!({"name": name, "values": [base, base + 1, base + 2, base + 3]});
    set(&mut doc, pos, json!({"series":[rows("Open", 9), rows("High", 12), rows("Low", 8), rows("Close", 11)],"options":{"stock":"ohlc"}}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.stock.as_deref(), Some("ohlc"));
    assert_eq!(m.series.len(), 4);
    assert_eq!(m.series[0].name, "Open");
    assert!(chart_xml(&mut doc).contains("<c:upDownBars>"));
    // Values only.
    set(&mut doc, pos, json!({"series":[rows("Open", 19), rows("High", 22), rows("Low", 18), rows("Close", 21)]}));
    assert_eq!(the_chart(&doc).1.series[3].values[0], Some(21.0));
    assert_eq!(the_chart(&doc).1.stock.as_deref(), Some("ohlc"));
    // And from a column chart to a stock chart.
    set(&mut doc, pos, json!({"kind":"column"}));
    assert_eq!(the_chart(&doc).1.kind, ChartKind::Column);
    set(&mut doc, pos, json!({"kind":"stock","options":{"stock":"ohlc"}}));
    assert_eq!(the_chart(&doc).1.stock.as_deref(), Some("ohlc"));
    chart_xml(&mut doc);
}

pub(super) fn surface_doc(variant: &str) -> (Document, Pos) {
    chart_doc(
        "surface",
        json!({"categories":["A","B","C","D"],"series":[{"name":"S1","values":[1,2,3,2]},{"name":"S2","values":[2,4,3,1]},{"name":"S3","values":[3,3,2,4]}],"options":{"surface":variant}}),
    )
}

#[test]
fn the_four_surface_charts_are_written_and_read_back() {
    for (variant, group, wire) in [("surface", "c:surface3DChart", "0"), ("wireframe", "c:surface3DChart", "1"), ("contour", "c:surfaceChart", "0"), ("wireframe_contour", "c:surfaceChart", "1")] {
        let (mut doc, _) = surface_doc(variant);
        let (_, m) = the_chart(&doc);
        assert_eq!(m.kind, ChartKind::Surface, "{variant}");
        assert_eq!(m.surface.as_deref(), Some(variant));
        assert!(m.view3d.is_some());
        assert_eq!(m.series.len(), 3);
        let xml = chart_xml(&mut doc);
        assert!(xml.contains(&format!("<{group}><c:wireframe val=\"{wire}\"/>")), "{variant}: {xml}");
        assert!(xml.contains("<c:view3D>") && xml.contains("<c:serAx>"));
        assert_eq!(xml.contains("<c:rotX val=\"90\"/>"), group == "c:surfaceChart");
        assert!(xml.find("<c:view3D>").unwrap() < xml.find("<c:plotArea>").unwrap());
        let again = Document::open(&doc.save().unwrap()).unwrap();
        assert_eq!(the_chart(&again).1.surface.as_deref(), Some(variant));
    }
}

#[test]
fn a_surface_chart_switches_between_its_variants_and_other_types() {
    let (mut doc, pos) = surface_doc("surface");
    opts(&mut doc, pos, json!({"surface":"contour"}));
    assert_eq!(the_chart(&doc).1.surface.as_deref(), Some("contour"));
    assert!(chart_xml(&mut doc).contains("<c:surfaceChart>"));
    set(&mut doc, pos, json!({"kind":"column"}));
    let xml = chart_xml(&mut doc);
    assert!(!xml.contains("<c:view3D>") && !xml.contains("<c:serAx>") && !xml.contains("surface"));
    set(&mut doc, pos, json!({"kind":"surface"}));
    assert_eq!(the_chart(&doc).1.kind, ChartKind::Surface);
    chart_xml(&mut doc);
    // Data edits keep the variant.
    opts(&mut doc, pos, json!({"surface":"wireframe"}));
    set(&mut doc, pos, json!({"series":[{"name":"S1","values":[1,1,1,1]},{"name":"S2","values":[2,2,2,2]},{"name":"S3","values":[3,3,3,3]}]}));
    assert_eq!(the_chart(&doc).1.surface.as_deref(), Some("wireframe"));
}

pub(super) fn with_workbook(doc: &mut Document, f: impl FnOnce(Vec<Vec<chart::xlsx::Cell>>) -> Vec<Vec<chart::xlsx::Cell>>) -> Document {
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let name = "word/embeddings/Microsoft_Excel_Worksheet1.xlsx";
    let cells = chart::xlsx::read_cells(pkg.get(name).unwrap()).unwrap();
    let cells = f(cells);
    pkg.set(name, chart::xlsx::build(&chart::xlsx::Plan { cells, refs: vec![] }));
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

#[test]
fn editing_the_data_keeps_the_columns_and_cells_added_in_excel() {
    use chart::xlsx::Cell;
    let (mut doc, _) = chart_doc("column", json!({}));
    // Extra columns the chart does not use, and a note below the data.
    let mut doc = with_workbook(&mut doc, |mut cells| {
        for (r, row) in cells.iter_mut().enumerate() {
            row.push(Cell::Empty);
            row.push(Cell::Text(format!("note {r}")));
            row.push(Cell::Number(100.0 + r as f64));
        }
        cells
    });
    let (pos, _) = the_chart(&doc);
    set(&mut doc, pos, json!({"categories":["A","B","C","D"],"series":[{"name":"One","values":[9,2,3,4]},{"name":"Two","values":[3,2,1,0]}]}));
    let pkg = saved(&mut doc);
    let cells = chart::xlsx::read_cells(pkg.get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx").unwrap()).unwrap();
    assert_eq!(cells[1][1], Cell::Number(9.0), "the changed value");
    assert_eq!(cells[4][0], Cell::Text("D".into()), "the new row");
    assert_eq!(cells[4][2], Cell::Number(0.0));
    // The columns next to the chart's are still there.
    assert_eq!(cells[0][4], Cell::Text("note 0".into()));
    assert_eq!(cells[2][5], Cell::Number(102.0));
    assert_eq!(cells[3][4], Cell::Text("note 3".into()));
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.contains("<c:f>Sheet1!$B$2:$B$5</c:f>"), "ranges grew: {xml}");
    // Undo puts the old workbook back.
    doc.undo();
    let pkg = saved(&mut doc);
    let cells = chart::xlsx::read_cells(pkg.get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx").unwrap()).unwrap();
    assert_eq!(cells[1][1], Cell::Number(1.0));
    assert_eq!(cells[0][4], Cell::Text("note 0".into()));
}

#[test]
fn a_new_series_goes_beside_the_columns_that_are_taken() {
    use chart::xlsx::Cell;
    let (mut doc, _) = chart_doc("column", json!({}));
    let mut doc = with_workbook(&mut doc, |mut cells| {
        for (r, row) in cells.iter_mut().enumerate() {
            row.push(Cell::Text(format!("mine {r}")));
        }
        cells
    });
    let (pos, _) = the_chart(&doc);
    set(&mut doc, pos, json!({"series":[{"name":"One","values":[1,2,3]},{"name":"Two","values":[3,2,1]},{"name":"Three","values":[7,8,9]}]}));
    let pkg = saved(&mut doc);
    let cells = chart::xlsx::read_cells(pkg.get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx").unwrap()).unwrap();
    assert_eq!(cells[0][3], Cell::Text("mine 0".into()), "column D is still the user's");
    assert_eq!(cells[0][4], Cell::Text("Three".into()));
    assert_eq!(cells[3][4], Cell::Number(9.0));
    assert!(text(&pkg, "word/charts/chart1.xml").contains("<c:f>Sheet1!$E$2:$E$4</c:f>"));
}

#[test]
fn a_bar_series_that_becomes_a_line_is_drawn_as_a_line() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    // Bars have no outline; that must not turn into "no line" for the line.
    assert!(!the_chart(&doc).1.series[0].no_line);
    opts(&mut doc, pos, json!({"series":[{"index":1,"kind":"line"}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[1].kind, ChartKind::Line);
    assert!(!m.series[1].no_line && m.series[1].marker);
    let xml = chart_xml(&mut doc);
    let line = &xml[xml.find("<c:lineChart>").unwrap()..xml.find("</c:lineChart>").unwrap()];
    assert!(line.contains("<a:ln w=\"28575\" cap=\"rnd\"><a:solidFill>"), "{line}");
}

#[test]
fn a_series_added_in_the_same_edit_can_have_its_own_type_and_formatting() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    set(
        &mut doc,
        pos,
        json!({"series":[{"name":"One","values":[1,2,3]},{"name":"Two","values":[3,2,1]},{"name":"Three","values":[5,5,5],"kind":"line","secondary":true}],"options":{"series":[{"index":2,"color":"AA3300","labels":{"value":true}}]}}),
    );
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series.len(), 3);
    assert_eq!((m.series[2].kind, m.series[2].secondary), (ChartKind::Line, true));
    assert_eq!(m.series[2].color.as_deref(), Some("AA3300"));
    assert!(m.series[2].labels.is_some() && m.series[0].labels.is_none());
    chart_xml(&mut doc);
}
