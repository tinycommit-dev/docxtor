//! Tests of: Combination charts and bubble.

use super::*;

pub(super) fn combo_doc() -> (Document, Pos) {
    chart_doc(
        "column",
        json!({"series":[{"name":"Revenue","values":[10,20,30]},{"name":"Margin","values":[0.1,0.2,0.15],"kind":"line","secondary":true}]}),
    )
}

#[test]
fn a_combination_chart_is_written_with_a_group_per_type_and_a_second_pair_of_axes() {
    let (mut doc, _) = combo_doc();
    let (_, m) = the_chart(&doc);
    assert!(m.combo);
    assert_eq!(m.kind, ChartKind::Column);
    assert_eq!((m.series[0].kind, m.series[0].secondary), (ChartKind::Column, false));
    assert_eq!((m.series[1].kind, m.series[1].secondary), (ChartKind::Line, true));
    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.find("<c:barChart>").unwrap() < xml.find("<c:lineChart>").unwrap());
    assert_eq!(count(&xml, "<c:valAx>"), 2);
    assert_eq!(count(&xml, "<c:catAx>"), 2);
    // The line group uses the second pair; the second category axis is hidden.
    let line = &xml[xml.find("<c:lineChart>").unwrap()..xml.find("</c:lineChart>").unwrap()];
    assert!(line.contains("<c:axId val=\"506450857\"/><c:axId val=\"506452097\"/>"));
    assert!(xml.contains("<c:delete val=\"1\"/>"));
    // Word's order of axes: category, value, second value, second category.
    let mut found: Vec<(usize, &str)> = Vec::new();
    for n in ["c:catAx", "c:valAx"] {
        for (i, _) in xml.match_indices(&format!("<{n}>")) {
            found.push((i, n));
        }
    }
    found.sort();
    let order: Vec<&str> = found.into_iter().map(|(_, n)| n).collect();
    assert_eq!(order, ["c:catAx", "c:valAx", "c:valAx", "c:catAx"]);
    // The workbook has both columns, and the formulas follow.
    assert!(xml.contains("<c:f>Sheet1!$B$2:$B$4</c:f>") && xml.contains("<c:f>Sheet1!$C$2:$C$4</c:f>"));
    // Reading it back is the same chart.
    let again = Document::open(&doc.save().unwrap()).unwrap();
    let (_, m2) = the_chart(&again);
    assert_eq!(serde_json::to_value(&m).unwrap(), serde_json::to_value(&m2).unwrap());
}

#[test]
fn a_combination_chart_edits_properly() {
    let (mut doc, pos) = combo_doc();
    // The data changes and the types stay.
    set(&mut doc, pos, json!({"categories":["A","B","C","D"],"series":[{"name":"Revenue","values":[1,2,3,4]},{"name":"Margin","values":[0.1,0.2,0.3,0.4]}]}));
    let (_, m) = the_chart(&doc);
    assert!(m.combo && m.series[1].kind == ChartKind::Line && m.series[1].secondary);
    assert_eq!(m.categories.len(), 4);
    assert_eq!(m.series[1].values.len(), 4);
    chart_xml(&mut doc);

    // A third series joins the last group; it can move to another type and the first axes.
    set(&mut doc, pos, json!({"series":[{"name":"Revenue","values":[1,2,3,4]},{"name":"Margin","values":[0.1,0.2,0.3,0.4]},{"name":"Plan","values":[2,2,2,2]}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series.len(), 3);
    opts(&mut doc, pos, json!({"series":[{"index":2,"kind":"area","secondary":false}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!((m.series[2].kind, m.series[2].secondary), (ChartKind::Area, false));
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:areaChart>"), 1);
    assert!(xml.find("<c:areaChart>").unwrap() < xml.find("<c:barChart>").unwrap(), "areas first, as Word writes them");

    // Names of the series stay with their data.
    assert_eq!(m.series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Revenue", "Margin", "Plan"]);

    // Everything on the first axes again: the second pair goes away.
    opts(&mut doc, pos, json!({"series":[{"index":1,"secondary":false}]}));
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:valAx>"), 1);
    assert_eq!(count(&xml, "<c:catAx>"), 1);
    assert!(!the_chart(&doc).1.series.iter().any(|s| s.secondary));

    // Back to one type with the kind parameter.
    set(&mut doc, pos, json!({"kind":"line"}));
    let (_, m) = the_chart(&doc);
    assert!(!m.combo && m.series.iter().all(|s| s.kind == ChartKind::Line));
    let xml = chart_xml(&mut doc);
    assert!(!xml.contains("<c:barChart>") && !xml.contains("<c:areaChart>"));
    doc.undo();
    assert!(the_chart(&doc).1.series.iter().any(|s| s.kind == ChartKind::Area));
}

#[test]
fn a_combination_with_formatting_keeps_it_when_a_series_changes_type() {
    let (mut doc, pos) = combo_doc();
    opts(&mut doc, pos, json!({"series":[{"index":1,"color":"CC3300","labels":{"value":true,"position":"t"},"trendlines":[{"type":"linear"}]}]}));
    // Margin becomes columns on the second axis.
    opts(&mut doc, pos, json!({"series":[{"index":1,"kind":"column"}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[1].color.as_deref(), Some("CC3300"));
    assert_eq!(m.series[1].kind, ChartKind::Column);
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:barChart>"), 2, "one group on each pair of axes");
    // The position "t" is not allowed on columns, so it was dropped rather than written.
    assert!(!xml.contains("<c:dLblPos val=\"t\"/>"));
}

#[test]
fn column_and_scatter_share_a_chart() {
    let (mut doc, _) = chart_doc(
        "column",
        json!({"series":[{"name":"Sales","values":[3,4,5]},{"name":"Trend","values":[2,4,6],"kind":"scatter","x_values":[1,2,3]}]}),
    );
    let (_, m) = the_chart(&doc);
    assert!(m.combo && m.series[1].kind == ChartKind::Scatter);
    assert_eq!(m.series[1].x_values, Some(vec![Some(1.0), Some(2.0), Some(3.0)]));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:scatterChart>") && xml.contains("<c:barChart>"));
    assert_eq!(count(&xml, "<c:valAx>"), 1);
}

#[test]
fn the_ai_tools_build_a_combination_chart() {
    let mut doc = doc_with("Intro\n\nBody");
    let p = find_p(&doc, "Body");
    let r = docxtor_core::ai::call_tool(
        &mut doc,
        "insert_chart",
        &json!({"id": p, "kind":"column","categories":["A","B"],"series":[{"name":"Sales","values":[1,2]},{"name":"Rate","values":[0.5,0.7],"kind":"line","secondary":true}],"options":{"yAxis":{"title":"Units"},"y2Axis":{"title":"Share","numberFormat":"0%"},"look":1}}),
    );
    assert!(r.is_ok(), "{r:?}");
    let (_, m) = the_chart(&doc);
    assert!(m.combo);
    assert_eq!(m.y2_axis.as_ref().and_then(|a| a.number_format.as_deref()), Some("0%"));
    assert_eq!(m.y_axis.title.as_deref(), Some("Units"));
    let script = docxtor_core::ai::docscript(&mut doc, &docxtor_core::ai::ScriptOpts::default());
    assert!(script.contains("combination of column and line"), "{script}");
}

pub(super) fn bubble_doc() -> (Document, Pos) {
    chart_doc(
        "bubble",
        json!({"categories":[],"series":[{"name":"North","values":[3,5,7],"x_values":[1,2,3],"sizes":[10,20,30]},{"name":"South","values":[2,4,1],"x_values":[1,2,3],"sizes":[5,5,15]}]}),
    )
}

#[test]
fn a_bubble_chart_has_sizes_and_two_value_axes() {
    let (mut doc, _) = bubble_doc();
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Bubble);
    assert_eq!(m.series[0].sizes, Some(vec![Some(10.0), Some(20.0), Some(30.0)]));
    assert_eq!(m.series[1].x_values, Some(vec![Some(1.0), Some(2.0), Some(3.0)]));
    assert!(m.categories.is_empty());
    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.contains("<c:bubbleChart>") && xml.contains("<c:bubbleSize>") && xml.contains("<c:bubbleScale val=\"100\"/><c:showNegBubbles val=\"0\"/>"));
    assert_eq!(count(&xml, "<c:valAx>"), 2);
    // Word's order inside a bubble series.
    let ser = &xml[xml.find("<c:ser>").unwrap()..xml.find("</c:ser>").unwrap()];
    let at = |n: &str| ser.find(n).unwrap_or_else(|| panic!("{n}"));
    assert!(at("<c:xVal>") < at("<c:yVal>") && at("<c:yVal>") < at("<c:bubbleSize>") && at("<c:bubbleSize>") < at("<c:bubble3D"));
    // The workbook holds the sizes in a column of their own, and the formula points there.
    let cells = chart::xlsx::read_cells(pkg.get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx").unwrap()).unwrap();
    assert_eq!(cells[0][0], chart::xlsx::Cell::Text("X-Values".into()));
    assert_eq!(cells[3][2], chart::xlsx::Cell::Number(30.0));
    assert!(xml.contains("<c:bubbleSize><c:numRef><c:f>Sheet1!$C$2:$C$4</c:f>"), "{xml}");
    assert!(xml.contains("<c:f>Sheet1!$E$2:$E$4</c:f>"));
}

#[test]
fn bubble_data_and_options_edit_in_place() {
    let (mut doc, pos) = bubble_doc();
    set(&mut doc, pos, json!({"series":[{"name":"North","values":[3,5,7,9],"x_values":[1,2,3,4],"sizes":[10,20,30,40]},{"name":"South","values":[2,4,1,2],"x_values":[1,2,3,4],"sizes":[5,5,15,25]}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].sizes.as_ref().unwrap().len(), 4);
    assert_eq!(m.series[1].sizes.as_ref().unwrap()[3], Some(25.0));
    opts(&mut doc, pos, json!({"bubbleScale":60,"sizeRepresents":"width","showNegativeBubbles":true,"series":[{"index":0,"fill":"soft","color":"3366CC"}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!((m.bubble_scale, m.size_represents.as_deref(), m.show_negative_bubbles), (Some(60), Some("width"), true));
    assert_eq!(m.series[0].color.as_deref(), Some("3366CC"));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:bubbleScale val=\"60\"/><c:showNegBubbles val=\"1\"/><c:sizeRepresents val=\"w\"/>"));
    // Moving to another type keeps the points, and back.
    set(&mut doc, pos, json!({"kind":"scatter"}));
    assert_eq!(the_chart(&doc).1.kind, ChartKind::Scatter);
    set(&mut doc, pos, json!({"kind":"bubble"}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Bubble);
    assert!(m.series[0].sizes.is_some());
    chart_xml(&mut doc);
}

#[test]
fn bubble_error_bars_take_both_directions() {
    let (mut doc, pos) = bubble_doc();
    opts(&mut doc, pos, json!({"series":[{"index":0,"errorBars":[{"dir":"x","valueType":"fixed","value":0.2}]}]}));
    assert_eq!(the_chart(&doc).1.series[0].error_bars[0].dir, "x");
    chart_xml(&mut doc);
}
