//! Tests of: Charts as Word writes them.

use super::*;

pub(super) fn word_chart_doc(plot_and_more: &str) -> (Document, Pos) {
    // A document of one chart whose part is written out by hand, with its workbook relationship.
    let (mut seed, _) = chart_doc("column", json!({}));
    let mut pkg = Package::open(&seed.save().unwrap()).unwrap();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><c:date1904 val=\"0\"/><c:roundedCorners val=\"0\"/>{plot_and_more}<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/></c:externalData></c:chartSpace>"
    );
    pkg.set("word/charts/chart1.xml", xml.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let (pos, _) = the_chart(&doc);
    (doc, pos)
}

pub(super) fn ser_xml(i: usize, name: &str, vals: &[f64], tag_val: &str, extra: &str) -> String {
    let pts: String = vals.iter().enumerate().map(|(k, v)| format!("<c:pt idx=\"{k}\"><c:v>{v}</c:v></c:pt>")).collect();
    let col = (b'B' + i as u8) as char;
    format!(
        "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/><c:tx><c:strRef><c:f>Sheet1!${col}$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx>{extra}<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$4</c:f><c:strCache><c:ptCount val=\"3\"/><c:pt idx=\"0\"><c:v>Mon</c:v></c:pt><c:pt idx=\"1\"><c:v>Tue</c:v></c:pt><c:pt idx=\"2\"><c:v>Wed</c:v></c:pt></c:strCache></c:strRef></c:cat><c:{tag_val}><c:numRef><c:f>Sheet1!${col}$2:${col}$4</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"3\"/>{pts}</c:numCache></c:numRef></c:{tag_val}></c:ser>"
    )
}

#[test]
fn a_volume_open_high_low_close_chart_from_word_is_read_and_edited() {
    let bar = format!("<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>{}<c:gapWidth val=\"150\"/><c:axId val=\"1\"/><c:axId val=\"2\"/></c:barChart>", ser_xml(0, "Volume", &[100.0, 200.0, 150.0], "val", "<c:invertIfNegative val=\"0\"/>"));
    let stock_sers: String = [("Open", [10.0, 11.0, 12.0]), ("High", [12.0, 13.0, 14.0]), ("Low", [9.0, 10.0, 11.0]), ("Close", [11.0, 12.0, 13.0])]
        .iter()
        .enumerate()
        .map(|(i, (n, v))| ser_xml(i + 1, n, v, "val", "<c:spPr><a:ln w=\"19050\"><a:noFill/></a:ln></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker>"))
        .collect();
    let stock = format!("<c:stockChart>{stock_sers}<c:hiLowLines/><c:upDownBars><c:gapWidth val=\"150\"/><c:upBars/><c:downBars/></c:upDownBars><c:axId val=\"3\"/><c:axId val=\"4\"/></c:stockChart>");
    let axes = "<c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/></c:catAx><c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/><c:crossAx val=\"1\"/><c:crosses val=\"autoZero\"/></c:valAx><c:valAx><c:axId val=\"4\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"r\"/><c:crossAx val=\"3\"/><c:crosses val=\"max\"/></c:valAx><c:catAx><c:axId val=\"3\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"1\"/><c:axPos val=\"b\"/><c:crossAx val=\"4\"/><c:crosses val=\"autoZero\"/></c:catAx>";
    let (mut doc, pos) = word_chart_doc(&format!("<c:chart><c:autoTitleDeleted val=\"1\"/><c:plotArea><c:layout/>{bar}{stock}{axes}</c:plotArea><c:plotVisOnly val=\"1\"/></c:chart>"));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Stock);
    assert_eq!(m.stock.as_deref(), Some("vohlc"));
    assert!(!m.combo);
    assert_eq!(m.series.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Volume", "Open", "High", "Low", "Close"]);
    assert!(!m.series[0].secondary && m.series[1].secondary);
    assert!(m.y2_axis.is_some());
    // Editing the title and a colour keeps the structure.
    set(&mut doc, pos, json!({"title":"Prices"}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"color":"336699"}],"y2Axis":{"title":"Price"}}));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:stockChart>") && xml.contains("<c:upDownBars>") && xml.contains("<c:barChart>"));
    assert_eq!(count(&xml, "<c:valAx>"), 2);
    assert!(xml.contains("<a:t>Price</a:t>"));
    // Values change in place.
    set(&mut doc, pos, json!({"categories":["Mon","Tue","Wed"],"series":[
        {"name":"Volume","values":[1,2,3]},{"name":"Open","values":[1,2,3]},{"name":"High","values":[2,3,4]},{"name":"Low","values":[0,1,2]},{"name":"Close","values":[1,2,3]}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[2].values, vec![Some(2.0), Some(3.0), Some(4.0)]);
    assert_eq!(m.stock.as_deref(), Some("vohlc"));
}

#[test]
fn a_surface_chart_from_word_with_band_formats_is_read_and_kept() {
    let sers: String = (0..3).map(|i| ser_xml(i, &format!("S{}", i + 1), &[1.0 + i as f64, 3.0, 2.0], "val", "")).collect();
    let plot = format!(
        "<c:chart><c:autoTitleDeleted val=\"1\"/><c:view3D><c:rotX val=\"15\"/><c:rotY val=\"20\"/><c:rAngAx val=\"0\"/><c:perspective val=\"30\"/></c:view3D><c:floor><c:thickness val=\"0\"/></c:floor><c:plotArea><c:layout/><c:surface3DChart><c:wireframe val=\"1\"/>{sers}<c:bandFmts><c:bandFmt><c:idx val=\"0\"/></c:bandFmt></c:bandFmts><c:axId val=\"1\"/><c:axId val=\"2\"/><c:axId val=\"3\"/></c:surface3DChart><c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/></c:catAx><c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/><c:crossAx val=\"1\"/><c:crosses val=\"autoZero\"/></c:valAx><c:serAx><c:axId val=\"3\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/></c:serAx></c:plotArea><c:plotVisOnly val=\"1\"/></c:chart>"
    );
    let (mut doc, pos) = word_chart_doc(&plot);
    let (_, m) = the_chart(&doc);
    assert_eq!((m.kind, m.surface.as_deref()), (ChartKind::Surface, Some("wireframe")));
    assert_eq!(m.view3d.as_ref().map(|v| (v.rot_x, v.rot_y, v.perspective)), Some((15, 20, 30)));
    assert_eq!(m.series.len(), 3);
    opts(&mut doc, pos, json!({"yAxis":{"title":"Height"}}));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:bandFmts>") && xml.contains("<c:serAx>") && xml.contains("<a:t>Height</a:t>"));
    // A data edit keeps the variant and the walls.
    set(&mut doc, pos, json!({"categories":["Mon","Tue","Wed"],"series":[{"name":"S1","values":[1,1,1]},{"name":"S2","values":[2,2,2]},{"name":"S3","values":[3,3,3]}]}));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:wireframe val=\"1\"/>") && xml.contains("<c:floor>") && xml.contains("<c:bandFmts>"));
}

#[test]
fn a_bubble_chart_from_word_with_bubble3d_and_a_negative_size_flag() {
    let ser = |name: &str| {
        format!(
            "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:tx><c:strRef><c:f>Sheet1!$B$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx><c:spPr><a:solidFill><a:schemeClr val=\"accent1\"><a:alpha val=\"50000\"/></a:schemeClr></a:solidFill><a:ln w=\"25400\"><a:solidFill><a:schemeClr val=\"accent1\"/></a:solidFill></a:ln></c:spPr><c:invertIfNegative val=\"0\"/><c:xVal><c:numRef><c:f>Sheet1!$A$2:$A$4</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"3\"/><c:pt idx=\"0\"><c:v>1</c:v></c:pt><c:pt idx=\"1\"><c:v>2</c:v></c:pt><c:pt idx=\"2\"><c:v>3</c:v></c:pt></c:numCache></c:numRef></c:xVal><c:yVal><c:numRef><c:f>Sheet1!$B$2:$B$4</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"3\"/><c:pt idx=\"0\"><c:v>5</c:v></c:pt><c:pt idx=\"1\"><c:v>6</c:v></c:pt><c:pt idx=\"2\"><c:v>7</c:v></c:pt></c:numCache></c:numRef></c:yVal><c:bubbleSize><c:numRef><c:f>Sheet1!$C$2:$C$4</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"3\"/><c:pt idx=\"0\"><c:v>10</c:v></c:pt><c:pt idx=\"1\"><c:v>20</c:v></c:pt><c:pt idx=\"2\"><c:v>30</c:v></c:pt></c:numCache></c:numRef></c:bubbleSize><c:bubble3D val=\"0\"/></c:ser>"
        )
    };
    let axes = "<c:valAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/></c:valAx><c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/><c:majorGridlines/><c:crossAx val=\"1\"/><c:crosses val=\"autoZero\"/></c:valAx>";
    let (mut doc, pos) = word_chart_doc(&format!("<c:chart><c:autoTitleDeleted val=\"1\"/><c:plotArea><c:layout/><c:bubbleChart><c:varyColors val=\"0\"/>{}<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"0\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls><c:bubbleScale val=\"100\"/><c:showNegBubbles val=\"0\"/><c:axId val=\"1\"/><c:axId val=\"2\"/></c:bubbleChart>{axes}</c:plotArea><c:plotVisOnly val=\"1\"/></c:chart>", ser("Trial")));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Bubble);
    assert_eq!(m.series[0].sizes, Some(vec![Some(10.0), Some(20.0), Some(30.0)]));
    assert_eq!(m.series[0].fill.as_deref(), Some("soft"), "the see-through fill Word gives bubbles");
    assert_eq!(m.series[0].x_values, Some(vec![Some(1.0), Some(2.0), Some(3.0)]));
    set(&mut doc, pos, json!({"series":[{"name":"Trial","values":[5,6,7],"x_values":[1,2,3],"sizes":[10,20,99]}]}));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:v>99</c:v>") && xml.contains("<c:bubble3D val=\"0\"/>") && xml.contains("<a:alpha val=\"50000\"/>"));
}

#[test]
fn the_set_chart_tool_takes_options_and_explains_a_wrong_one() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    let p = pos.p.0;
    let r = docxtor_core::ai::call_tool(&mut doc, "set_chart", &json!({"id": p, "options": {"y_axis": {"max": 50, "title": "Units"}, "series": [{"index": 0, "color": "scheme:accent4", "trendlines": [{"type": "linear", "equation": true}]}]}}));
    assert!(r.is_ok(), "{r:?}");
    let (_, m) = the_chart(&doc);
    assert_eq!((m.y_axis.max, m.y_axis.title.as_deref()), (Some(50.0), Some("Units")));
    assert!(m.series[0].trendlines[0].equation);
    let e = docxtor_core::ai::call_tool(&mut doc, "set_chart", &json!({"id": p, "options": {"yAxis": {"maxx": 1}}})).unwrap_err();
    assert!(e.contains("maxx"), "{e}");
    let e = docxtor_core::ai::call_tool(&mut doc, "set_chart", &json!({"id": p, "options": {"look": 12}})).unwrap_err();
    assert!(e.contains("look 12"), "{e}");
    // The tool list says what the options are.
    let defs = serde_json::to_string(&docxtor_core::ai::tool_definitions()).unwrap();
    assert!(defs.contains("trendlines") && defs.contains("errorBars") && defs.contains("colorful1") && defs.contains("bubble"));
}

#[test]
fn the_legend_parameter_can_say_top_right() {
    let (mut doc, pos) = chart_doc("column", json!({"legend":"top_right"}));
    assert_eq!(the_chart(&doc).1.legend.unwrap().position, "tr");
    set(&mut doc, pos, json!({"legend":"left"}));
    assert_eq!(the_chart(&doc).1.legend.unwrap().position, "l");
    set(&mut doc, pos, json!({"legend":"top_right"}));
    assert!(chart_xml(&mut doc).contains("<c:legendPos val=\"tr\"/>"));
}
