//! Tests of: Colours and data labels and trendlines and error bars.

use super::*;

#[test]
fn series_colours_hex_and_theme_and_per_point_colours() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"color":"FF0000"},{"index":1,"color":"scheme:accent2:lumMod=60000:lumOff=40000"}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].color.as_deref(), Some("FF0000"));
    let c1 = m.series[1].color.clone().unwrap();
    assert_eq!(c1.len(), 6);
    assert_ne!(c1, "FF0000");
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<a:srgbClr val=\"FF0000\"/>"));
    assert!(xml.contains("<a:schemeClr val=\"accent2\"><a:lumMod val=\"60000\"/><a:lumOff val=\"40000\"/></a:schemeClr>"), "theme colours stay theme colours: {xml}");

    // Varied bars: a colour per point of a single series.
    let (mut doc, pos) = chart_doc("column", json!({"series":[{"name":"One","values":[1,2,3]}]}));
    opts(&mut doc, pos, json!({"varyColors":true,"series":[{"index":0,"pointColors":["00FF00",null,"0000FF"]}]}));
    let (_, m) = the_chart(&doc);
    assert!(m.vary_colors);
    assert_eq!(m.series[0].point_colors, vec![Some("00FF00".to_string()), None, Some("0000FF".to_string())]);
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:dPt>"), 2);

    // Slices of a pie.
    let (mut doc, pos) = chart_doc("pie", json!({"series":[{"name":"Share","values":[1,2,3]}]}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"pointColors":["111111","222222","333333"]}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].point_colors.iter().flatten().count(), 3);
    assert_eq!(m.series[0].point_colors[1].as_deref(), Some("222222"));
    // Change one slice only.
    opts(&mut doc, pos, json!({"series":[{"index":0,"pointColors":["111111","AA0000","333333"]}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].point_colors[1].as_deref(), Some("AA0000"));
    assert_eq!(count(&chart_xml(&mut doc), "<c:dPt>"), 3);
}

#[test]
fn lines_and_scatter_colours_reach_the_line_and_the_marker() {
    let (mut doc, pos) = chart_doc("line", json!({}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"color":"123456","lineWidth":4.5}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].color.as_deref(), Some("123456"));
    assert_eq!(m.series[0].line_width, Some(4.5));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("w=\"57150\""));
    // The marker follows the line.
    assert!(count(&xml, "<a:srgbClr val=\"123456\"/>") >= 3, "line and marker fill and marker outline: {xml}");
    opts(&mut doc, pos, json!({"series":[{"index":0,"marker":false,"smooth":true}]}));
    let (_, m) = the_chart(&doc);
    assert!(!m.series[0].marker && m.series[0].smooth);
}

#[test]
fn bar_fills_outline_soft_and_gradient() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    for fill in ["none", "soft", "gradient"] {
        opts(&mut doc, pos, json!({"series":[{"index":0,"fill":fill,"color":"336699"}]}));
        let (_, m) = the_chart(&doc);
        assert_eq!(m.series[0].fill.as_deref(), Some(fill));
        assert_eq!(m.series[0].color.as_deref(), Some("336699"), "{fill}");
        chart_xml(&mut doc);
    }
    opts(&mut doc, pos, json!({"series":[{"index":0,"fill":null}]}));
    assert_eq!(the_chart(&doc).1.series[0].fill, None);
}

#[test]
fn data_labels_what_they_show_where_and_how() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(
        &mut doc,
        pos,
        json!({"series":[{"index":0,"labels":{"value":true,"category":true,"series":true,"legendKey":true,"position":"outEnd","separator":"; ","numberFormat":"0.0"}}]}),
    );
    let (_, m) = the_chart(&doc);
    let l = m.series[0].labels.clone().unwrap();
    assert!(l.value && l.category && l.series && l.legend_key && !l.percent);
    assert_eq!(l.position.as_deref(), Some("outEnd"));
    assert_eq!(l.separator.as_deref(), Some("; "));
    assert_eq!(l.number_format.as_deref(), Some("0.0"));
    assert!(m.series[1].labels.is_none(), "per series");
    assert!(m.data_labels);
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:dLblPos val=\"outEnd\"/>"));
    assert!(xml.contains("<c:separator>; </c:separator>"));

    // Off for one series.
    opts(&mut doc, pos, json!({"series":[{"index":0,"labels":null}]}));
    let (_, m) = the_chart(&doc);
    assert!(m.series[0].labels.is_none() && !m.data_labels);
}

#[test]
fn label_positions_the_chart_type_does_not_allow_are_not_written() {
    // Word refuses a file with an outside-end label on a stacked column.
    let (mut doc, pos) = chart_doc("column", json!({"grouping":"stacked"}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"labels":{"value":true,"position":"outEnd"}}]}));
    let xml = chart_xml(&mut doc);
    assert!(!xml.contains("outEnd"), "{xml}");
    opts(&mut doc, pos, json!({"series":[{"index":0,"labels":{"value":true,"position":"inBase"}}]}));
    assert!(chart_xml(&mut doc).contains("<c:dLblPos val=\"inBase\"/>"));
    // Area charts take no position at all, pies take best fit.
    let (mut doc, pos) = chart_doc("area", json!({}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"labels":{"value":true,"position":"t"}}]}));
    assert!(!chart_xml(&mut doc).contains("<c:dLblPos"));
    let (mut doc, pos) = chart_doc("pie", json!({"series":[{"name":"S","values":[1,2,3]}]}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"labels":{"percent":true,"category":true,"position":"bestFit"}}]}));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:dLblPos val=\"bestFit\"/>") && xml.contains("<c:showLeaderLines val=\"1\"/>"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"series":[{"index":0,"labels":{"position":"middle"}}]}})).contains("label position"));
}

#[test]
fn every_kind_of_trendline_is_written_and_read_back() {
    let (mut doc, pos) = chart_doc("scatter", json!({}));
    let lines = json!([
        {"type":"linear","forward":1,"backward":0.5,"equation":true,"rSquared":true,"name":"Fit"},
        {"type":"poly","order":3},
        {"type":"movingAvg","period":2},
    ]);
    opts(&mut doc, pos, json!({"series":[{"index":0,"trendlines":lines}]}));
    let (_, m) = the_chart(&doc);
    let t = &m.series[0].trendlines;
    assert_eq!(t.len(), 3);
    assert_eq!((t[0].kind.as_str(), t[0].forward, t[0].backward, t[0].equation, t[0].r_squared, t[0].name.as_deref()), ("linear", Some(1.0), Some(0.5), true, true, Some("Fit")));
    assert_eq!((t[1].kind.as_str(), t[1].order), ("poly", Some(3)));
    assert_eq!((t[2].kind.as_str(), t[2].period), ("movingAvg", Some(2)));
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:trendline>"), 3);
    assert!(xml.contains("<c:trendlineType val=\"poly\"/><c:order val=\"3\"/>"));
    assert!(xml.contains("<c:dispRSqr val=\"1\"/><c:dispEq val=\"1\"/>"));

    for kind in ["exp", "log", "power"] {
        opts(&mut doc, pos, json!({"series":[{"index":1,"trendlines":[{"type":kind}]}]}));
        assert_eq!(the_chart(&doc).1.series[1].trendlines[0].kind, kind);
    }
    // One removed.
    opts(&mut doc, pos, json!({"series":[{"index":0,"trendlines":[{"type":"poly","order":3}]}]}));
    assert_eq!(the_chart(&doc).1.series[0].trendlines.len(), 1);
    opts(&mut doc, pos, json!({"series":[{"index":0,"trendlines":[]}]}));
    assert!(the_chart(&doc).1.series[0].trendlines.is_empty());
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"series":[{"index":0,"trendlines":[{"type":"poly","order":9}]}]}})).contains("order of 2 to 6"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"series":[{"index":0,"trendlines":[{"type":"curvy"}]}]}})).contains("trendline type"));
}

#[test]
fn trendlines_on_a_stacked_chart_are_not_written() {
    let (mut doc, pos) = chart_doc("column", json!({"grouping":"stacked"}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"trendlines":[{"type":"linear"}]}]}));
    assert!(!chart_xml(&mut doc).contains("<c:trendline>"));
}

#[test]
fn error_bars_fixed_percentage_standard_and_custom() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"errorBars":[{"valueType":"fixed","value":2,"barType":"both"}]}]}));
    let e = the_chart(&doc).1.series[0].error_bars.clone();
    assert_eq!((e[0].value_type.as_str(), e[0].value, e[0].bar_type.as_str(), e[0].dir.as_str(), e[0].no_cap), ("fixed", Some(2.0), "both", "y", false));
    assert!(chart_xml(&mut doc).contains("<c:errValType val=\"fixedVal\"/>"));

    opts(&mut doc, pos, json!({"series":[{"index":0,"errorBars":[{"valueType":"percentage","value":10,"barType":"plus","noCap":true}]}]}));
    let e = the_chart(&doc).1.series[0].error_bars.clone();
    assert_eq!((e[0].value_type.as_str(), e[0].value, e[0].bar_type.as_str(), e[0].no_cap), ("percentage", Some(10.0), "plus", true));

    opts(&mut doc, pos, json!({"series":[{"index":0,"errorBars":[{"valueType":"stdDev","value":1.5,"barType":"minus"}]}]}));
    assert_eq!(the_chart(&doc).1.series[0].error_bars[0].value_type, "stdDev");
    opts(&mut doc, pos, json!({"series":[{"index":0,"errorBars":[{"valueType":"stdErr"}]}]}));
    assert_eq!(the_chart(&doc).1.series[0].error_bars[0].value_type, "stdErr");

    opts(&mut doc, pos, json!({"series":[{"index":1,"errorBars":[{"valueType":"custom","plus":[1,2,3],"minus":[0.5,0.5,0.5]}]}]}));
    let e = the_chart(&doc).1.series[1].error_bars.clone();
    assert_eq!((e[0].value_type.as_str(), e[0].plus.clone(), e[0].minus.clone()), ("custom", Some(vec![1.0, 2.0, 3.0]), Some(vec![0.5, 0.5, 0.5])));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:errValType val=\"cust\"/>") && xml.contains("<c:plus><c:numLit>"));

    opts(&mut doc, pos, json!({"series":[{"index":1,"errorBars":[]}]}));
    assert!(the_chart(&doc).1.series[1].error_bars.is_empty());
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"series":[{"index":0,"errorBars":[{"valueType":"custom"}]}]}})).contains("custom error bars"));
}

#[test]
fn scatter_charts_take_error_bars_in_both_directions() {
    let (mut doc, pos) = chart_doc("scatter", json!({}));
    opts(&mut doc, pos, json!({"series":[{"index":0,"errorBars":[{"dir":"x","valueType":"fixed","value":0.5},{"dir":"y","valueType":"percentage","value":5}]}]}));
    let e = the_chart(&doc).1.series[0].error_bars.clone();
    assert_eq!(e.len(), 2);
    assert_eq!((e[0].dir.as_str(), e[1].dir.as_str()), ("x", "y"));
    assert_eq!(count(&chart_xml(&mut doc), "<c:errBars>"), 2);
}
