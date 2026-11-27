//! Tests of: Looks and palettes and things the model does not know stay.

use super::*;

#[test]
fn a_look_sets_fills_gaps_gridlines_and_the_chart_style() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(&mut doc, pos, json!({"look":3}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].fill.as_deref(), Some("none"));
    assert_eq!(m.gap_width, Some(150));
    assert_eq!(m.style, Some(18));
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<c:style val=\"18\"/>"));
    assert!(xml.contains("<c14:style val=\"118\"/>"), "Word's alternate content: {xml}");
    opts(&mut doc, pos, json!({"look":2}));
    let (_, m) = the_chart(&doc);
    assert!(!m.y_axis.major_grid && m.series[0].labels.as_ref().is_some_and(|l| l.value));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"look":99}})).contains("look 99"));
    opts(&mut doc, pos, json!({"look":1}));
    let (_, m) = the_chart(&doc);
    assert!(m.y_axis.major_grid && m.series[0].fill.is_none() && m.gap_width == Some(219));
}

#[test]
fn a_palette_colours_the_series_from_the_theme_and_writes_colors_xml() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(&mut doc, pos, json!({"palette":"colorful2"}));
    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.contains("<a:schemeClr val=\"accent2\"/>") && xml.contains("<a:schemeClr val=\"accent4\"/>"), "{xml}");
    let part = text(&pkg, "word/charts/colors1.xml");
    assert!(part.contains("cs:colorStyle") && part.contains("meth=\"cycle\"") && part.contains("id=\"11\""), "{part}");
    let rels = pkg.rels_for("word/charts/chart1.xml").unwrap();
    assert!(rels.iter().any(|r| r.rel_type == chart::REL_COLORS && r.target == "colors1.xml"));
    assert!(text(&pkg, "[Content_Types].xml").contains("application/vnd.ms-office.chartcolorstyle+xml"));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.palette.as_deref(), Some("colorful2"));

    // Another palette replaces the part rather than adding one.
    opts(&mut doc, pos, json!({"palette":"mono3"}));
    let pkg = saved(&mut doc);
    assert!(!pkg.has("word/charts/colors2.xml"));
    let part = text(&pkg, "word/charts/colors1.xml");
    assert!(part.contains("meth=\"withinLinear\"") && part.contains("id=\"16\""), "{part}");
    assert_eq!(the_chart(&doc).1.palette.as_deref(), Some("mono3"));
    // Monochromatic: shades of one accent.
    let (_, m) = the_chart(&doc);
    assert_ne!(m.series[0].color, m.series[1].color);
    assert!(chart_xml(&mut doc).contains("<a:schemeClr val=\"accent3\">"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"palette":"mono14"}})).contains("unknown palette"));

    // Undo takes the part and the colours away.
    doc.undo();
    assert_eq!(the_chart(&doc).1.palette.as_deref(), Some("colorful2"));
    doc.undo();
    let pkg = saved(&mut doc);
    assert!(!pkg.has("word/charts/colors1.xml"));
    assert!(the_chart(&doc).1.palette.is_none());
}

#[test]
fn every_palette_has_a_part_and_colours() {
    for id in chart::palette::palette_ids() {
        let xml = chart::palette::colors_part_xml(&id).unwrap();
        docxtor_core::xml::parse(xml.as_bytes()).unwrap();
        assert!(chart::palette::palette_color(&id, 0, 3).is_some(), "{id}");
        let n = chart::palette::palette_number(&id).unwrap();
        assert_eq!(chart::palette::palette_from_number(n).as_deref(), Some(id.as_str()));
    }
    assert_eq!(chart::palette::palette_ids().len(), 17);
}

#[test]
fn a_palette_on_a_pie_colours_its_slices() {
    let (mut doc, pos) = chart_doc("pie", json!({"series":[{"name":"S","values":[1,2,3]}]}));
    opts(&mut doc, pos, json!({"palette":"colorful1"}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series[0].point_colors.iter().flatten().count(), 3);
    let xml = chart_xml(&mut doc);
    assert!(xml.contains("<a:schemeClr val=\"accent3\"/>"));
}

#[test]
fn unmodelled_elements_survive_option_edits() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    // Put things Docxtor does not model into the part, as Word would.
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let xml = text(&pkg, "word/charts/chart1.xml").replace("</c:chart>", "<c:extLst><c:ext uri=\"{X}\"/></c:extLst></c:chart>").replace("</c:chartSpace>", "<c:extLst><c:ext uri=\"{Y}\"/></c:extLst></c:chartSpace>");
    let xml = xml.replacen("<c:valAx>", "<c:valAx><!--keep me-->", 1);
    pkg.set("word/charts/chart1.xml", xml.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let (pos2, _) = the_chart(&doc);
    let _ = pos;
    opts(&mut doc, pos2, json!({"yAxis":{"title":"Money","max":5},"series":[{"index":0,"color":"AA0000","labels":{"value":true}}]}));
    let out = chart_xml(&mut doc);
    assert!(out.contains("uri=\"{Y}\"") && out.contains("<!--keep me-->"));
}

#[test]
fn options_accept_snake_case_and_refuse_unknown_keys() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    opts(&mut doc, pos, json!({"y_axis":{"major_unit":5,"number_format":"0"},"legend_overlay":true,"series":[{"index":0,"point_colors":[]}]}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.y_axis.major_unit, Some(5.0));
    assert!(m.legend_overlay);
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"zAxis":{}}})).contains("unknown chart option"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"yAxis":{"majorUnits":5}}})).contains("majorUnits"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"series":[{"index":7,"color":"FF0000"}]}})).contains("no series 7"));
    assert!(err(&mut doc, json!({"op":"set_chart","at":at(pos),"options":{"series":[{"color":"FF0000"}]}})).contains("index"));
    // A refused edit changes nothing.
    assert_eq!(the_chart(&doc).1.y_axis.major_unit, Some(5.0));
}

#[test]
fn option_edits_undo_and_redo_in_one_step() {
    let (mut doc, pos) = chart_doc("column", json!({}));
    let before = chart_xml(&mut doc);
    opts(&mut doc, pos, json!({"yAxis":{"title":"Money","max":9},"series":[{"index":0,"color":"FF0000","trendlines":[{"type":"linear"}]}],"look":4,"palette":"mono1"}));
    let after = chart_xml(&mut doc);
    assert_ne!(before, after);
    doc.undo();
    assert_eq!(chart_xml(&mut doc), before);
    doc.redo();
    assert_eq!(chart_xml(&mut doc), after);
}

#[test]
fn options_on_insert_are_the_same_as_options_after() {
    let options = json!({"xAxis":{"title":"Q"},"yAxis":{"max":10,"majorUnit":5},"series":[{"index":0,"color":"00AA00","labels":{"value":true,"position":"outEnd"}}],"legendOverlay":true});
    let (mut a, _) = chart_doc("column", json!({"options": options}));
    let (mut b, pos) = chart_doc("column", json!({}));
    opts(&mut b, pos, options);
    let (_, ma) = the_chart(&a);
    let (_, mb) = the_chart(&b);
    assert_eq!(serde_json::to_value(&ma).unwrap(), serde_json::to_value(&mb).unwrap());
    chart_xml(&mut a);
}
