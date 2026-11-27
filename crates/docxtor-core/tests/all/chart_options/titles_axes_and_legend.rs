//! Tests of: Titles, gridlines, axes and title and legend.

use super::*;

#[test]
fn axis_titles_for_category_value_and_second_value_axis() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(&mut doc, pos, json!({"xAxis":{"title":"Region"},"yAxis":{"title":"Revenue"}}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.x_axis.title.as_deref(), Some("Region"));
    assert_eq!(m.y_axis.title.as_deref(), Some("Revenue"));
    assert_eq!(m.axes.x_title.as_deref(), Some("Region"), "the older axes fields follow");
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<a:t>Region</a:t>") && xml.contains("<a:t>Revenue</a:t>"));
    // The value axis title runs up the axis, as Word writes it.
    assert!(xml.contains("rot=\"-5400000\""));

    // A second value axis: a series on it gets one, with its own title.
    set(&mut doc, pos, json!({"options": {"series":[{"index":1,"kind":"line","secondary":true}], "y2Axis":{"title":"Margin"}}}));
    let (_, m) = the_chart(&doc);
    assert!(m.combo);
    assert!(m.series[1].secondary && m.series[1].kind == ChartKind::Line);
    assert_eq!(m.y2_axis.as_ref().and_then(|a| a.title.as_deref()), Some("Margin"));
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:valAx>"), 2);
    assert_eq!(count(&xml, "<c:catAx>"), 2);
    assert!(xml.contains("<c:axPos val=\"r\"/>") && xml.contains("<c:crosses val=\"max\"/>"));

    // Titles come off again.
    opts(&mut doc, pos, json!({"xAxis":{"title":null}}));
    assert_eq!(the_chart(&doc).1.x_axis.title, None);
}

#[test]
fn gridlines_major_and_minor_for_each_axis() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    let (_, m) = the_chart(&doc);
    assert!(m.y_axis.major_grid && !m.x_axis.major_grid);
    opts(&mut doc, pos, json!({"yAxis":{"majorGrid":false,"minorGrid":true},"xAxis":{"majorGrid":true,"minorGrid":true}}));
    let (_, m) = the_chart(&doc);
    assert!(!m.y_axis.major_grid && m.y_axis.minor_grid && m.x_axis.major_grid && m.x_axis.minor_grid);
    assert!(!m.axes.grid, "the older field follows the value axis' major gridlines");
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:majorGridlines>"), 1);
    assert_eq!(count(&xml, "<c:minorGridlines>"), 2);
    opts(&mut doc, pos, json!({"yAxis":{"majorGrid":true,"minorGrid":false},"xAxis":{"majorGrid":false,"minorGrid":false}}));
    let xml = chart_xml(&mut doc);
    assert_eq!(count(&xml, "<c:majorGridlines>"), 1);
    assert_eq!(count(&xml, "<c:minorGridlines>"), 0);
}

#[test]
fn axis_scale_units_ticks_labels_number_format_and_direction() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(
        &mut doc,
        pos,
        json!({"yAxis":{"min":0,"max":10,"majorUnit":2,"minorUnit":0.5,"numberFormat":"#,##0.00","majorTick":"out","minorTick":"in","labelPos":"high","displayUnits":"thousands"},"xAxis":{"reverse":true,"majorTick":"cross","labelPos":"low"}}),
    );
    let (_, m) = the_chart(&doc);
    let y = &m.y_axis;
    assert_eq!((y.min, y.max, y.major_unit, y.minor_unit), (Some(0.0), Some(10.0), Some(2.0), Some(0.5)));
    assert_eq!(y.number_format.as_deref(), Some("#,##0.00"));
    assert_eq!((y.major_tick.as_deref(), y.minor_tick.as_deref(), y.label_pos.as_deref()), (Some("out"), Some("in"), Some("high")));
    assert_eq!(y.display_units.as_deref(), Some("thousands"));
    assert!(m.x_axis.reverse);
    assert_eq!(m.x_axis.label_pos.as_deref(), Some("low"));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:max val=\"10\"/><c:min val=\"0\"/>"), "max before min inside scaling: {xml}");
    assert!(xml.contains("<c:majorUnit val=\"2\"/><c:minorUnit val=\"0.5\"/><c:dispUnits><c:builtInUnit val=\"thousands\"/></c:dispUnits>"));
    assert!(xml.contains("<c:orientation val=\"maxMin\"/>"));
    assert!(xml.contains("formatCode=\"#,##0.00\" sourceLinked=\"0\""));

    // Back to automatic, linked to the data, hidden.
    opts(&mut doc, pos, json!({"yAxis":{"min":null,"max":null,"majorUnit":null,"minorUnit":null,"numberFormat":null,"displayUnits":null,"hidden":true}}));
    let (_, m) = the_chart(&doc);
    assert_eq!((m.y_axis.min, m.y_axis.max, m.y_axis.major_unit), (None, None, None));
    assert!(m.y_axis.number_format.is_none() && m.y_axis.display_units.is_none() && m.y_axis.hidden);
    let xml = chart_xml(&mut doc);
    assert!(!xml.contains("<c:dispUnits>") && !xml.contains("<c:majorUnit"));
}

#[test]
fn a_logarithmic_scale_and_percent_numbers() {
    let (mut doc, pos) = chart_doc("line", json!({}));
    opts(&mut doc, pos, json!({"yAxis":{"logBase":10,"numberFormat":"0%"}}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.y_axis.log_base, Some(10.0));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:logBase val=\"10\"/><c:orientation"), "logBase first inside scaling: {xml}");
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"yAxis":{"logBase":1}}})).contains("logarithmic base"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"yAxis":{"min":5,"max":2}}})).contains("minimum must be less"));
}

#[test]
fn a_scatter_chart_has_two_value_axes_with_their_own_scales() {
    let (mut doc, pos) = chart_doc("scatter", json!({}));
    opts(&mut doc, pos, json!({"xAxis":{"min":0,"max":4,"majorUnit":1,"title":"Time"},"yAxis":{"min":0,"max":10,"minorGrid":true}}));
    let (_, m) = the_chart(&doc);
    assert_eq!((m.x_axis.min, m.x_axis.max, m.x_axis.major_unit), (Some(0.0), Some(4.0), Some(1.0)));
    assert_eq!((m.y_axis.min, m.y_axis.max), (Some(0.0), Some(10.0)));
    assert_eq!(m.axes.x_min, Some(0.0));
    assert_eq!(m.x_axis.title.as_deref(), Some("Time"));
    chart_xml(&mut doc);
}

#[test]
fn title_overlay_and_legend_position_overlay_and_top_right() {
    let (mut doc, pos) = chart_doc("column", json!({"legend":"bottom"}));
    opts(&mut doc, pos, json!({"titleOverlay":true,"legend":{"position":"tr"},"legendOverlay":true}));
    let (_, m) = the_chart(&doc);
    assert!(m.title_overlay && m.legend_overlay);
    assert_eq!(m.legend.as_ref().unwrap().position, "tr");
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:legendPos val=\"tr\"/><c:overlay val=\"1\"/>"));
    assert!(xml.contains("<c:overlay val=\"1\"/>"));
    for pos_name in ["r", "l", "t", "b"] {
        opts(&mut doc, pos, json!({"legend":{"position":pos_name}}));
        assert_eq!(the_chart(&doc).1.legend.unwrap().position, pos_name);
    }
    opts(&mut doc, pos, json!({"legend":null}));
    assert!(the_chart(&doc).1.legend.is_none());
    assert!(!chart_xml(&mut doc).contains("<c:legend>"));
    opts(&mut doc, pos, json!({"legend":{"position":"b"}}));
    assert_eq!(the_chart(&doc).1.legend.unwrap().position, "b");
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"legend":{"position":"x"}}})).contains("legend position"));
}
