//! The new chart types (chartex): waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst and
//! region map are read from Word's markup into a model the editor draws, shown from the fallback picture when
//! Docxtor can't draw them, kept byte-for-byte on save, and resized with their drawing.

use docxtor_core::ai::{call_tool, docscript, ScriptOpts};
use docxtor_core::chart::chartex::{self, ChartExModel, ExKind};
use docxtor_core::chart::xlsx;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Every XML part of a saved package must be namespace-well-formed, every relationship must point at a part
/// that exists, and every part must have a content type.
fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
        if part.name.ends_with(".rels") {
            let owner = part.name.replace("_rels/", "").trim_end_matches(".rels").to_string();
            let owner = if part.name == "_rels/.rels" { String::new() } else { owner };
            for r in pkg.rels_for(&owner).unwrap() {
                if !r.external {
                    let target = docxtor_core::package::resolve_target(&owner, &r.target);
                    assert!(pkg.has(&target), "{} points at missing part {target}", part.name);
                }
            }
        }
    }
    let ct = pkg.xml("[Content_Types].xml").unwrap();
    for o in ct.root.elements().filter(|e| e.local() == "Override") {
        let name = o.attr("PartName").unwrap().trim_start_matches('/');
        assert!(pkg.has(name), "content types override for missing part {name}");
    }
    for p in &pkg.parts {
        if p.name.starts_with("word/charts/chartEx") && p.name.ends_with(".xml") {
            assert!(
                ct.root.elements().any(|e| e.local() == "Override" && e.attr("PartName") == Some(format!("/{}", p.name).as_str()) && e.attr("ContentType") == Some(chartex::CT_CHARTEX)),
                "{} has no chartex content type",
                p.name
            );
        }
    }
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn text(pkg: &Package, name: &str) -> String {
    String::from_utf8(pkg.get(name).unwrap_or_else(|| panic!("no part {name}")).to_vec()).unwrap()
}

fn saved(doc: &mut Document) -> Package {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    Package::open(&bytes).unwrap()
}

/// The chart segments of the body: paragraph id and segment as the editor receives it.
fn segs(doc: &Document, kind: &str) -> Vec<(u32, Value)> {
    let json = serde_json::to_value(doc.render_snapshot()).unwrap();
    json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|b| b["segs"].as_array().into_iter().flatten().map(move |s| (b["id"].as_u64().unwrap_or(0) as u32, s.clone())))
        .filter(|(_, s)| s["k"] == kind)
        .collect()
}

fn model_of(xml: &str) -> ChartExModel {
    chartex::read(xml.as_bytes(), &[]).expect("a readable chartex part")
}

// ---------------------------------------------------------------------------
// Word's markup
// ---------------------------------------------------------------------------

const HEAD: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cx:chartSpace xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex">"##;

fn title(t: &str) -> String {
    format!(r#"<cx:title pos="t" align="ctr" overlay="0"><cx:tx><cx:txData><cx:f>_xlchart.v1.0</cx:f><cx:v>{t}</cx:v></cx:txData></cx:tx></cx:title>"#)
}

/// `<cx:strDim>` with one or more levels; each level lists its cells (index and text).
fn str_dim(f: &str, count: usize, levels: &[&[(usize, &str)]]) -> String {
    let lvls: String = levels
        .iter()
        .map(|pts| {
            let cells: String = pts.iter().map(|(i, t)| format!(r#"<cx:pt idx="{i}">{t}</cx:pt>"#)).collect();
            format!(r#"<cx:lvl ptCount="{count}">{cells}</cx:lvl>"#)
        })
        .collect();
    format!(r#"<cx:strDim type="cat"><cx:f>{f}</cx:f>{lvls}</cx:strDim>"#)
}

fn num_dim(ty: &str, f: &str, vals: &[f64]) -> String {
    let cells: String = vals.iter().enumerate().map(|(i, v)| format!(r#"<cx:pt idx="{i}">{v}</cx:pt>"#)).collect();
    format!(r#"<cx:numDim type="{ty}"><cx:f>{f}</cx:f><cx:lvl ptCount="{}" formatCode="General">{cells}</cx:lvl></cx:numDim>"#, vals.len())
}

fn series_tx(f: &str, name: &str) -> String {
    format!(r#"<cx:tx><cx:txData><cx:f>{f}</cx:f><cx:v>{name}</cx:v></cx:txData></cx:tx>"#)
}

fn axes(extra_val: &str) -> String {
    format!(r#"<cx:axis id="0"><cx:catScaling gapWidth="0.5"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/>{extra_val}<cx:tickLabels/></cx:axis>"#)
}

fn wrap(data: &str, title_xml: &str, series: &str, axes_xml: &str, legend: &str) -> String {
    format!(
        r#"{HEAD}<cx:chartData><cx:externalData r:id="rId1" cx:autoUpdate="0"/>{data}</cx:chartData><cx:chart>{title_xml}<cx:plotArea><cx:plotAreaRegion>{series}</cx:plotAreaRegion>{axes_xml}</cx:plotArea>{legend}</cx:chart></cx:chartSpace>"#
    )
}

fn waterfall_xml() -> String {
    let data = format!(
        r#"<cx:data id="0">{}{}</cx:data>"#,
        str_dim("Sheet1!$A$2:$A$7", 6, &[&[(0, "Start"), (1, "Sales"), (2, "Returns"), (3, "Costs"), (4, "Tax"), (5, "Profit")]]),
        num_dim("val", "Sheet1!$B$2:$B$7", &[100.0, 80.0, -20.0, -35.0, -10.0, 115.0])
    );
    let ser = format!(
        r#"<cx:series layoutId="waterfall" uniqueId="{{00000000-0000-0000-0000-000000000001}}">{}<cx:dataPt idx="2"><cx:spPr><a:solidFill><a:srgbClr val="C00000"/></a:solidFill></cx:spPr></cx:dataPt><cx:dataLabels pos="outEnd"><cx:visibility seriesName="0" categoryName="0" value="1"/></cx:dataLabels><cx:dataId val="0"/><cx:layoutPr><cx:subtotals><cx:idx val="0"/><cx:idx val="5"/></cx:subtotals></cx:layoutPr></cx:series>"#,
        series_tx("Sheet1!$B$1", "Amount")
    );
    wrap(&data, &title("Cash flow"), &ser, &axes(""), r#"<cx:legend pos="t" align="ctr" overlay="0"/>"#)
}

const SAMPLE: [f64; 20] = [1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0, 5.0, 6.0, 7.0, 8.0, 9.0, 9.0, 10.0, 11.0, 12.0, 14.0, 20.0];

fn histogram_xml(binning: &str) -> String {
    let data = format!(r#"<cx:data id="0">{}</cx:data>"#, num_dim("val", "Sheet1!$A$2:$A$21", &SAMPLE));
    let ser = format!(r#"<cx:series layoutId="clusteredColumn" uniqueId="{{00000000-0000-0000-0000-000000000002}}"><cx:dataId val="0"/><cx:layoutPr>{binning}</cx:layoutPr></cx:series>"#);
    wrap(&data, &title("Scores"), &ser, r#"<cx:axis id="0"><cx:catScaling gapWidth="0"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis>"#, "")
}

fn pareto_xml() -> String {
    let data = format!(
        r#"<cx:data id="0">{}{}</cx:data>"#,
        str_dim("Sheet1!$A$2:$A$6", 5, &[&[(0, "Scratch"), (1, "Dent"), (2, "Crack"), (3, "Stain"), (4, "Other")]]),
        num_dim("val", "Sheet1!$B$2:$B$6", &[12.0, 40.0, 8.0, 25.0, 15.0])
    );
    let ser = format!(
        r#"<cx:series layoutId="clusteredColumn" uniqueId="{{1}}">{}<cx:dataId val="0"/><cx:layoutPr><cx:aggregation/></cx:layoutPr><cx:axisId val="1"/></cx:series><cx:series layoutId="paretoLine" uniqueId="{{2}}"><cx:spPr><a:ln w="28575"><a:solidFill><a:srgbClr val="ED7D31"/></a:solidFill></a:ln></cx:spPr><cx:axisId val="2"/><cx:dataId val="0"/></cx:series>"#,
        series_tx("Sheet1!$B$1", "Defects")
    );
    let ax = r#"<cx:axis id="0"><cx:catScaling gapWidth="0"/><cx:tickLabels/></cx:axis><cx:axis id="1"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis><cx:axis id="2"><cx:valScaling max="1" min="0"/><cx:units unit="percentage"/><cx:tickLabels/></cx:axis>"#;
    wrap(&data, &title("Defects"), &ser, ax, r#"<cx:legend pos="b" align="ctr" overlay="0"/>"#)
}

fn box_xml(method: &str, vis: &str) -> String {
    let a = [4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 30.0];
    let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let both: Vec<f64> = a.iter().chain(b.iter()).copied().collect();
    let labels: Vec<(usize, &str)> = (0..both.len()).map(|i| (i, if i < a.len() { "Ward A" } else { "Ward B" })).collect();
    let data = format!(r#"<cx:data id="0">{}{}</cx:data>"#, str_dim("Sheet1!$A$2:$A$18", both.len(), &[&labels]), num_dim("val", "Sheet1!$B$2:$B$18", &both));
    let ser = format!(
        r#"<cx:series layoutId="boxWhisker" uniqueId="{{3}}">{}<cx:dataId val="0"/><cx:layoutPr>{vis}<cx:statistics quartileMethod="{method}"/></cx:layoutPr></cx:series>"#,
        series_tx("Sheet1!$B$1", "Wait")
    );
    wrap(&data, &title("Waiting time"), &ser, &axes(""), "")
}

fn funnel_xml() -> String {
    let data = format!(
        r#"<cx:data id="0">{}{}</cx:data>"#,
        str_dim("Sheet1!$A$2:$A$5", 4, &[&[(0, "Visits"), (1, "Sign-ups"), (2, "Trials"), (3, "Paid")]]),
        num_dim("val", "Sheet1!$B$2:$B$5", &[1000.0, 400.0, 150.0, 60.0])
    );
    let ser = format!(r#"<cx:series layoutId="funnel" uniqueId="{{4}}">{}<cx:dataLabels pos="inEnd"><cx:visibility seriesName="0" categoryName="0" value="1"/></cx:dataLabels><cx:dataId val="0"/></cx:series>"#, series_tx("Sheet1!$B$1", "Count"));
    wrap(&data, &title("Funnel"), &ser, r#"<cx:axis id="0"><cx:catScaling gapWidth="0.06"/><cx:tickLabels/></cx:axis>"#, "")
}

/// Fruit: Apple 4, Pear 3, Plum 1; Veg: Kale 2, Leek 5. The parent level lists a name only at the first row of
/// its group, as Excel does. `leaf_first` puts the leaf level first, as the classic charts' caches do.
fn tree_xml(layout_id: &str, leaf_first: bool, extra: &str) -> String {
    let parents: &[(usize, &str)] = &[(0, "Fruit"), (3, "Veg")];
    let leaves: &[(usize, &str)] = &[(0, "Apple"), (1, "Pear"), (2, "Plum"), (3, "Kale"), (4, "Leek")];
    let levels: Vec<&[(usize, &str)]> = if leaf_first { vec![leaves, parents] } else { vec![parents, leaves] };
    let data = format!(r#"<cx:data id="0">{}{}</cx:data>"#, str_dim("Sheet1!$A$2:$B$6", 5, &levels), num_dim("size", "Sheet1!$C$2:$C$6", &[4.0, 3.0, 1.0, 2.0, 5.0]));
    let ser = format!(r#"<cx:series layoutId="{layout_id}" uniqueId="{{5}}">{}<cx:dataLabels pos="inEnd"><cx:visibility seriesName="0" categoryName="1" value="0"/></cx:dataLabels><cx:dataId val="0"/><cx:layoutPr>{extra}</cx:layoutPr></cx:series>"#, series_tx("Sheet1!$C$1", "Size"));
    wrap(&data, &title("Produce"), &ser, "", "")
}

fn region_xml() -> String {
    let data = format!(r#"<cx:data id="0">{}{}</cx:data>"#, str_dim("Sheet1!$A$2:$A$4", 3, &[&[(0, "France"), (1, "Spain"), (2, "Italy")]]), num_dim("colorVal", "Sheet1!$B$2:$B$4", &[5.0, 3.0, 8.0]));
    let ser = format!(r#"<cx:series layoutId="regionMap" uniqueId="{{6}}">{}<cx:dataId val="0"/><cx:layoutPr><cx:geography cultureLanguage="en-US" cultureRegion="US" attribution="Powered by Bing"><cx:geoCache provider="{{E9337A44-BEBE-4D9F-B70C-5C5E7DAFC167}}"/></cx:geography></cx:layoutPr></cx:series>"#, series_tx("Sheet1!$B$1", "Sales"));
    wrap(&data, &title("Sales by country"), &ser, "", r#"<cx:legend pos="r" align="ctr" overlay="0"/>"#)
}

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xff, 0xff, 0x3f, 0x00, 0x05, 0xfe, 0x02, 0xfe, 0xa7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Clone, Copy, PartialEq)]
enum Fallback {
    None,
    Drawing,
    Pict,
}

fn chartex_drawing(rid: &str, img_rid: &str, fallback: Fallback) -> String {
    let choice = format!(
        r#"<mc:Choice Requires="cx1" xmlns:cx1="http://schemas.microsoft.com/office/drawing/2015/9/8/chartex"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="7" name="Chart 7"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="{}"><cx:chart xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{rid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice>"#,
        chartex::URI_CHARTEX
    );
    let fb = match fallback {
        Fallback::None => String::new(),
        Fallback::Drawing => format!(
            r#"<mc:Fallback><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="7" name="Chart 7"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="0" name="Chart 7"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{img_rid}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="5486400" cy="3200400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Fallback>"#
        ),
        Fallback::Pict => format!(
            r##"<mc:Fallback><w:pict><v:shape id="Chart 7" o:spid="_x0000_i1025" type="#_x0000_t75" style="width:432pt;height:252pt" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><v:imagedata r:id="{img_rid}" o:title=""/></v:shape></w:pict></mc:Fallback>"##
        ),
    };
    format!(r#"<w:r><mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">{choice}{fb}</mc:AlternateContent></w:r>"#)
}

const CHARTEX_PART: &str = "word/charts/chartEx1.xml";

/// A document the way Word saves one with a new-style chart: the chartex part, its workbook, style and colour
/// parts, and the picture that stands in for it.
fn docx_with_chartex(chart_xml: &str, fallback: Fallback) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    pkg.set(CHARTEX_PART, chart_xml.as_bytes().to_vec());
    pkg.set("word/charts/style1.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cs:chartStyle xmlns:cs="http://schemas.microsoft.com/office/drawing/2012/chartStyle" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" id="395"/>"#.to_vec());
    pkg.set("word/charts/colors1.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cs:colorStyle xmlns:cs="http://schemas.microsoft.com/office/drawing/2012/chartStyle" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" meth="cycle" id="10"><a:schemeClr val="accent1"/></cs:colorStyle>"#.to_vec());
    let plan = xlsx::plan(&["a".to_string()], &[], false);
    pkg.set("word/embeddings/Microsoft_Excel_Sheet1.xlsx", xlsx::build(&plan));
    pkg.ensure_override(CHARTEX_PART, chartex::CT_CHARTEX);
    pkg.ensure_override("word/charts/style1.xml", "application/vnd.ms-office.chartstyle+xml");
    pkg.ensure_override("word/charts/colors1.xml", "application/vnd.ms-office.chartcolorstyle+xml");
    pkg.ensure_default("xlsx", docxtor_core::chart::CT_XLSX);
    pkg.ensure_default("png", "image/png");
    pkg.add_rel(CHARTEX_PART, docxtor_core::chart::REL_PACKAGE, "../embeddings/Microsoft_Excel_Sheet1.xlsx", false);
    pkg.add_rel(CHARTEX_PART, "http://schemas.microsoft.com/office/2011/relationships/chartStyle", "style1.xml", false);
    pkg.add_rel(CHARTEX_PART, "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle", "colors1.xml", false);
    let rid = pkg.add_rel("word/document.xml", chartex::REL_CHARTEX, "charts/chartEx1.xml", false);
    let img_rid = if fallback == Fallback::None {
        String::new()
    } else {
        pkg.set("word/media/image1.png", PNG.to_vec());
        pkg.add_rel("word/document.xml", docxtor_core::package::REL_IMAGE, "media/image1.png", false)
    };
    let doc = text(&pkg, "word/document.xml");
    let doc = doc.replacen("<w:p/>", &format!("<w:p><w:r><w:t>Before the chart</w:t></w:r></w:p><w:p>{}</w:p><w:p><w:r><w:t>After the chart</w:t></w:r></w:p>", chartex_drawing(&rid, &img_rid, fallback)), 1);
    pkg.set("word/document.xml", doc.into_bytes());
    pkg.to_bytes().unwrap()
}

fn open_chartex(chart_xml: &str, fallback: Fallback) -> Document {
    Document::open(&docx_with_chartex(chart_xml, fallback)).unwrap()
}

// ---------------------------------------------------------------------------
// Reading each kind
// ---------------------------------------------------------------------------

#[test]
fn reads_a_waterfall_with_running_totals_and_subtotals() {
    let m = model_of(&waterfall_xml());
    assert_eq!(m.kind, ExKind::Waterfall);
    assert_eq!(m.title.as_deref(), Some("Cash flow"));
    assert_eq!(m.name.as_deref(), Some("Amount"));
    assert_eq!(m.legend.as_ref().map(|l| l.position.as_str()), Some("t"));
    assert!(m.data_labels);
    assert!(m.axes.grid);
    assert_eq!(m.axes.gap_width, Some(0.5));
    let w = m.waterfall.as_ref().unwrap();
    assert!(w.connectors);
    let rows: Vec<(&str, f64, f64, &str)> = w.points.iter().map(|p| (p.label.as_str(), p.start, p.end, p.kind)).collect();
    assert_eq!(
        rows,
        vec![
            ("Start", 0.0, 100.0, "total"),
            ("Sales", 100.0, 180.0, "increase"),
            ("Returns", 180.0, 160.0, "decrease"),
            ("Costs", 160.0, 125.0, "decrease"),
            ("Tax", 125.0, 115.0, "decrease"),
            ("Profit", 0.0, 115.0, "total"),
        ]
    );
    // A colour set on one point (cx:dataPt) is kept for that point.
    assert_eq!(m.point_colors.get(2).cloned().flatten().as_deref(), Some("C00000"));
    assert!(m.point_colors.get(1).cloned().flatten().is_none());
}

#[test]
fn a_waterfall_can_turn_its_connector_lines_off() {
    let xml = waterfall_xml().replace("</cx:subtotals>", r#"</cx:subtotals><cx:visibility connectorLines="0"/>"#);
    assert!(!model_of(&xml).waterfall.unwrap().connectors);
}

#[test]
fn reads_a_histogram_with_a_bin_width_a_bin_count_or_none() {
    let by_size = model_of(&histogram_xml(r#"<cx:binning intervalClosed="r"><cx:binSize val="5"/></cx:binning>"#));
    assert_eq!(by_size.kind, ExKind::Histogram);
    let h = by_size.histogram.as_ref().unwrap();
    assert!(!h.by_category);
    // Edges 1, 6, 11, 16, 21: [1,6] (6,11] (11,16] (16,21].
    let edges: Vec<(f64, f64)> = h.bins.iter().map(|b| (b.lo.unwrap(), b.hi.unwrap())).collect();
    assert_eq!(edges, vec![(1.0, 6.0), (6.0, 11.0), (11.0, 16.0), (16.0, 21.0)]);
    let counts: Vec<f64> = h.bins.iter().map(|b| b.count).collect();
    assert_eq!(counts, vec![11.0, 6.0, 2.0, 1.0]);
    assert_eq!(counts.iter().sum::<f64>(), SAMPLE.len() as f64);
    assert_eq!(by_size.axes.gap_width, Some(0.0));

    let by_count = model_of(&histogram_xml(r#"<cx:binning intervalClosed="r"><cx:binCount val="4"/></cx:binning>"#));
    let h = by_count.histogram.as_ref().unwrap();
    assert_eq!(h.bins.len(), 4);
    // The span 1 to 20 in four: width 4.75.
    assert_eq!(h.bins[0].lo, Some(1.0));
    assert_eq!(h.bins[0].hi, Some(5.75));
    assert_eq!(h.bins[3].hi, Some(20.0));
    assert_eq!(h.bins.iter().map(|b| b.count).sum::<f64>(), 20.0);

    let auto = model_of(&histogram_xml(r#"<cx:binning intervalClosed="r"/>"#));
    let h = auto.histogram.as_ref().unwrap();
    // Scott's rule: 3.49 s / n^(1/3) with s = 4.8 and n = 20 gives a width near 6.2, so four bins.
    assert_eq!(h.bins.len(), 4);
    assert_eq!(h.bins.iter().map(|b| b.count).sum::<f64>(), 20.0);
    let w = h.bins[0].hi.unwrap() - h.bins[0].lo.unwrap();
    assert!((w - 6.17).abs() < 0.05, "{w}");
}

#[test]
fn a_histogram_closed_on_the_left_and_with_overflow_and_underflow_bins() {
    let left = model_of(&histogram_xml(r#"<cx:binning intervalClosed="l"><cx:binSize val="5"/></cx:binning>"#));
    let h = left.histogram.unwrap();
    // [1,6) [6,11) [11,16) [16,21]: the values 5 and 5 stay in the first bin, the 6 moves on.
    assert_eq!(h.bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![10.0, 6.0, 3.0, 1.0]);
    assert!(h.bins[0].closed_lo && !h.bins[0].closed_hi);
    assert!(h.bins[3].closed_lo && h.bins[3].closed_hi);

    let ends = model_of(&histogram_xml(r#"<cx:binning intervalClosed="r" underflow="3" overflow="12"><cx:binSize val="3"/></cx:binning>"#));
    let h = ends.histogram.unwrap();
    let kinds: Vec<&str> = h.bins.iter().map(|b| b.kind).collect();
    assert_eq!(kinds.first(), Some(&"underflow"));
    assert_eq!(kinds.last(), Some(&"overflow"));
    assert_eq!(h.bins[0].hi, Some(3.0));
    // 1, 2, 2, 3, 3, 3 are at most 3; 14 and 20 are above 12.
    assert_eq!(h.bins[0].count, 6.0);
    assert_eq!(h.bins.last().unwrap().count, 2.0);
    assert_eq!(h.bins.last().unwrap().lo, Some(12.0));
    assert_eq!(h.bins.iter().map(|b| b.count).sum::<f64>(), 20.0);

    // "auto" for either end means no end bin.
    let auto = model_of(&histogram_xml(r#"<cx:binning intervalClosed="r" underflow="auto" overflow="auto"><cx:binSize val="5"/></cx:binning>"#));
    assert!(auto.histogram.unwrap().bins.iter().all(|b| b.kind == "bin"));
}

#[test]
fn reads_a_pareto_with_its_bars_sorted_and_the_cumulative_line() {
    let m = model_of(&pareto_xml());
    assert_eq!(m.kind, ExKind::Pareto);
    assert_eq!(m.name.as_deref(), Some("Defects"));
    let h = m.histogram.as_ref().unwrap();
    assert!(h.by_category);
    let names: Vec<&str> = h.bins.iter().map(|b| b.label.as_deref().unwrap()).collect();
    assert_eq!(names, vec!["Dent", "Stain", "Other", "Scratch", "Crack"]);
    assert_eq!(h.bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![40.0, 25.0, 15.0, 12.0, 8.0]);
    let cum = h.cumulative.as_ref().unwrap();
    assert_eq!(cum.len(), 5);
    assert!((cum[0] - 0.4).abs() < 1e-12);
    assert!((cum[1] - 0.65).abs() < 1e-12);
    assert!((cum[4] - 1.0).abs() < 1e-12);
    assert_eq!(h.line_color.as_deref(), Some("ED7D31"));
    // The percentage axis is not the one the bars use.
    assert_eq!(m.axes.val_max, None);
}

#[test]
fn reads_box_and_whisker_with_either_quartile_method() {
    let vis = r#"<cx:visibility meanLine="1" meanMarker="1" nonoutliers="0" outliers="1"/>"#;
    let m = model_of(&box_xml("exclusive", vis));
    assert_eq!(m.kind, ExKind::BoxWhisker);
    let b = m.boxes.as_ref().unwrap();
    assert_eq!(b.categories, vec!["Ward A", "Ward B"]);
    assert_eq!(b.quartile, "exclusive");
    assert!(b.mean_line && b.mean_marker && b.outliers && !b.points);
    assert_eq!(b.series.len(), 1);
    assert_eq!(b.series[0].name, "Wait");
    let a = b.series[0].boxes[0].as_ref().unwrap();
    // 4 5 6 7 8 9 10 11 30: QUARTILE.EXC positions 2.5 and 7.5.
    assert_eq!((a.q1, a.median, a.q3), (5.5, 8.0, 10.5));
    assert_eq!(a.outliers, vec![30.0]);
    assert_eq!(a.whisker_lo, 4.0);
    assert_eq!(a.whisker_hi, 11.0);
    assert_eq!(a.n, 9);
    let bb = b.series[0].boxes[1].as_ref().unwrap();
    // 1..8: QUARTILE.EXC positions 2.25 and 6.75.
    assert_eq!((bb.q1, bb.median, bb.q3), (2.25, 4.5, 6.75));
    assert!(bb.outliers.is_empty());
    assert!((bb.mean - 4.5).abs() < 1e-12);

    let inc = model_of(&box_xml("inclusive", ""));
    let b = inc.boxes.as_ref().unwrap();
    assert_eq!(b.quartile, "inclusive");
    // Without a visibility element Word shows the mean marker and the outliers, not the line or the points.
    assert!(b.mean_marker && b.outliers && !b.mean_line && !b.points);
    let a = b.series[0].boxes[0].as_ref().unwrap();
    // QUARTILE.INC positions 2 and 6.
    assert_eq!((a.q1, a.median, a.q3), (6.0, 8.0, 10.0));
    let bb = b.series[0].boxes[1].as_ref().unwrap();
    assert_eq!((bb.q1, bb.q3), (2.75, 6.25));
}

#[test]
fn reads_a_funnel() {
    let m = model_of(&funnel_xml());
    assert_eq!(m.kind, ExKind::Funnel);
    assert!(m.data_labels);
    let f = m.funnel.unwrap();
    assert_eq!(f.iter().map(|p| (p.label.as_str(), p.value)).collect::<Vec<_>>(), vec![("Visits", 1000.0), ("Sign-ups", 400.0), ("Trials", 150.0), ("Paid", 60.0)]);
}

#[test]
fn reads_a_treemap_and_a_sunburst_whichever_way_the_levels_are_listed() {
    for leaf_first in [false, true] {
        for (id, kind) in [("treemap", ExKind::Treemap), ("sunburst", ExKind::Sunburst)] {
            let m = model_of(&tree_xml(id, leaf_first, ""));
            assert_eq!(m.kind, kind);
            let t = m.tree.as_ref().unwrap();
            assert_eq!(t.value, 15.0, "{id} leaf_first={leaf_first}");
            assert_eq!(t.children.iter().map(|c| (c.name.as_str(), c.value)).collect::<Vec<_>>(), vec![("Fruit", 8.0), ("Veg", 7.0)]);
            assert_eq!(t.children[0].children.iter().map(|c| (c.name.as_str(), c.value)).collect::<Vec<_>>(), vec![("Apple", 4.0), ("Pear", 3.0), ("Plum", 1.0)]);
            assert_eq!(t.children[1].children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Kale", "Leek"]);
            assert_eq!(t.depth(), 3);
            assert_eq!(t.leaves(), 5);
        }
    }
    let m = model_of(&tree_xml("treemap", false, r#"<cx:parentLabelLayout val="overlapping"/>"#));
    assert_eq!(m.parent_labels.as_deref(), Some("overlapping"));
}

#[test]
fn a_tree_without_sizes_counts_each_row_as_one() {
    let xml = tree_xml("sunburst", false, "").replace(&num_dim("size", "Sheet1!$C$2:$C$6", &[4.0, 3.0, 1.0, 2.0, 5.0]), "");
    let t = model_of(&xml).tree.unwrap();
    assert_eq!(t.value, 5.0);
    assert_eq!(t.children[0].value, 3.0);
}

#[test]
fn data_with_gaps_and_cells_holding_only_some_rows_reads_without_error() {
    // The Pareto's value column misses a cell: the row doesn't count. The names list has a hole too.
    let xml = pareto_xml().replace(r#"<cx:pt idx="2">8</cx:pt>"#, "").replace(r#"<cx:pt idx="3">Stain</cx:pt>"#, "");
    let h = model_of(&xml).histogram.unwrap();
    assert_eq!(h.bins.len(), 3);
    assert_eq!(h.bins.iter().map(|b| b.count).sum::<f64>(), 12.0 + 40.0 + 15.0);
    // A title in rich text, as Word writes one when it was edited.
    let rich = waterfall_xml().replace(&title("Cash flow"), r#"<cx:title pos="t" align="ctr" overlay="0"><cx:tx><cx:rich><a:bodyPr/><a:p><a:r><a:rPr lang="en-US"/><a:t>Money </a:t></a:r><a:r><a:t>in and out</a:t></a:r></a:p></cx:rich></cx:tx></cx:title>"#);
    assert_eq!(model_of(&rich).title.as_deref(), Some("Money in and out"));
    // A point count in the billions is a count, not an allocation: the part still reads, with the rows it really holds.
    let huge = waterfall_xml().replace(r#"ptCount="6""#, r#"ptCount="4000000000""#);
    assert!(huge.contains("4000000000"));
    let m = model_of(&huge);
    assert_eq!(m.title.as_deref(), Some("Cash flow"));
    // Not a chartex part.
    assert!(chartex::read(b"<c:chartSpace xmlns:c=\"x\"/>", &[]).is_none());
    assert!(chartex::read(b"nonsense", &[]).is_none());
}

#[test]
fn series_colours_resolve_against_the_theme() {
    let xml = waterfall_xml().replace("<cx:dataId val=\"0\"/>", r#"<cx:spPr><a:solidFill><a:schemeClr val="accent2"/></a:solidFill></cx:spPr><cx:dataId val="0"/>"#);
    let theme = vec![("accent1".to_string(), "112233".to_string()), ("accent2".to_string(), "445566".to_string())];
    let m = chartex::read(xml.as_bytes(), &theme).unwrap();
    assert_eq!(m.color.as_deref(), Some("445566"));
    assert_eq!(m.accents, vec!["112233", "445566"]);
}

// ---------------------------------------------------------------------------
// In a document
// ---------------------------------------------------------------------------

#[test]
fn a_document_with_a_waterfall_gives_a_chart_segment_with_the_model_and_the_size() {
    for fb in [Fallback::Drawing, Fallback::Pict, Fallback::None] {
        let doc = open_chartex(&waterfall_xml(), fb);
        let found = segs(&doc, "chart");
        assert_eq!(found.len(), 1);
        let (_, seg) = &found[0];
        assert_eq!(seg["chartex"]["kind"], "waterfall");
        assert_eq!(seg["chartex"]["waterfall"]["points"][1]["end"], 180.0);
        assert!(seg.get("chart").is_none(), "a new-style chart has no classic model");
        assert_eq!(seg["img"]["w"], 432.0);
        assert_eq!(seg["img"]["h"], 252.0);
        assert_eq!(seg["img"]["wrap"], "inline");
        assert_eq!(seg["img"]["src"], CHARTEX_PART);
        assert_eq!(seg["img"]["name"], "Chart 7");
        // The classic chart tools find nothing to edit.
        assert!(segs(&doc, "img").is_empty());
    }
}

#[test]
fn every_kind_is_drawn_natively_even_with_a_picture_to_fall_back_on() {
    let kinds = [
        (waterfall_xml(), "waterfall"),
        (histogram_xml(r#"<cx:binning intervalClosed="r"/>"#), "histogram"),
        (pareto_xml(), "pareto"),
        (box_xml("exclusive", ""), "boxWhisker"),
        (funnel_xml(), "funnel"),
        (tree_xml("treemap", false, ""), "treemap"),
        (tree_xml("sunburst", true, ""), "sunburst"),
    ];
    for (xml, kind) in kinds {
        let doc = open_chartex(&xml, Fallback::Drawing);
        let found = segs(&doc, "chart");
        assert_eq!(found.len(), 1, "{kind}");
        assert_eq!(found[0].1["chartex"]["kind"], kind);
    }
}

#[test]
fn a_region_map_shows_its_picture_or_else_a_placeholder_with_the_title() {
    let with = open_chartex(&region_xml(), Fallback::Drawing);
    assert!(segs(&with, "chart").is_empty(), "the picture is shown instead");
    let pics = segs(&with, "img");
    assert_eq!(pics.len(), 1);
    assert_eq!(pics[0].1["img"]["src"], "word/media/image1.png");
    assert_eq!(pics[0].1["img"]["w"], 432.0);

    let without = open_chartex(&region_xml(), Fallback::None);
    let found = segs(&without, "chart");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1["chartex"]["kind"], "regionMap");
    assert_eq!(found[0].1["chartex"]["title"], "Sales by country");
    assert!(found[0].1["chartex"].get("waterfall").is_none());
}

#[test]
fn unreadable_chart_data_falls_back_to_the_picture() {
    // The part is damaged.
    let doc = open_chartex("<cx:chartSpace", Fallback::Pict);
    assert!(segs(&doc, "chart").is_empty());
    assert_eq!(segs(&doc, "img").len(), 1);
    // The part is missing altogether.
    let mut pkg = Package::open(&docx_with_chartex(&waterfall_xml(), Fallback::Drawing)).unwrap();
    pkg.remove(CHARTEX_PART);
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert!(segs(&doc, "chart").is_empty());
    assert_eq!(segs(&doc, "img").len(), 1);
    // A chart with no data in it, and a picture.
    let empty = waterfall_xml().replace(&num_dim("val", "Sheet1!$B$2:$B$7", &[100.0, 80.0, -20.0, -35.0, -10.0, 115.0]), "");
    let doc = open_chartex(&empty, Fallback::Drawing);
    assert!(segs(&doc, "chart").is_empty());
    assert_eq!(segs(&doc, "img").len(), 1);
    // Without a picture it is a placeholder, not a crash.
    let doc = open_chartex("<cx:chartSpace", Fallback::None);
    let found = segs(&doc, "chart");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1["chartex"]["kind"], "other");
}

#[test]
fn an_untouched_chartex_round_trips_byte_for_byte() {
    for (xml, fb) in [(waterfall_xml(), Fallback::Drawing), (pareto_xml(), Fallback::Pict), (tree_xml("treemap", false, ""), Fallback::None), (region_xml(), Fallback::Drawing)] {
        let original = docx_with_chartex(&xml, fb);
        let before = Package::open(&original).unwrap();
        let mut doc = Document::open(&original).unwrap();
        // Edit elsewhere in the document.
        let first = doc.render_all().order[0];
        ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Hello "}));
        let after = saved(&mut doc);
        for part in &before.parts {
            if part.name != "word/document.xml" {
                assert_eq!(after.get(&part.name), Some(part.data.as_slice()), "{} changed", part.name);
            }
        }
        assert_eq!(before.rels_for(CHARTEX_PART).unwrap(), after.rels_for(CHARTEX_PART).unwrap());
        // The drawing is still the same alternate content with its choice and fallback.
        let body = text(&after, "word/document.xml");
        assert!(body.contains("<mc:AlternateContent"), "{body}");
        assert!(body.contains(r#"Requires="cx1""#));
        assert_eq!(body.contains("<mc:Fallback>"), fb != Fallback::None);
        assert!(body.contains(r#"uri="http://schemas.microsoft.com/office/drawing/2014/chartex""#));
        // And with no edit at all, every part is the same.
        let mut doc = Document::open(&original).unwrap();
        let again = saved(&mut doc);
        for part in &before.parts {
            assert_eq!(again.get(&part.name), Some(part.data.as_slice()), "{} changed on a plain save", part.name);
        }
    }
}

#[test]
fn resizing_changes_the_extent_of_the_choice_and_of_the_picture_and_keeps_the_rest() {
    for fb in [Fallback::Drawing, Fallback::Pict, Fallback::None] {
        let original = docx_with_chartex(&waterfall_xml(), fb);
        let before = Package::open(&original).unwrap();
        let mut doc = Document::open(&original).unwrap();
        let (p, _) = segs(&doc, "chart").remove(0);
        ops(&mut doc, json!({"op":"set_image","at":{"p":p,"o":0},"width":300,"height":150,"alt":"Cash flow by step"}));
        let found = segs(&doc, "chart");
        assert_eq!(found[0].1["img"]["w"], 300.0);
        assert_eq!(found[0].1["img"]["h"], 150.0);
        assert_eq!(found[0].1["img"]["alt"], "Cash flow by step");
        assert_eq!(found[0].1["chartex"]["kind"], "waterfall");
        let after = saved(&mut doc);
        for part in &before.parts {
            if part.name != "word/document.xml" {
                assert_eq!(after.get(&part.name), Some(part.data.as_slice()), "{} changed", part.name);
            }
        }
        let body = text(&after, "word/document.xml");
        // Still alternate content, with the chart choice.
        assert!(body.contains("<mc:AlternateContent") && body.contains(r#"Requires="cx1""#), "{body}");
        assert!(body.contains(r#"<wp:extent cx="3810000" cy="1905000"/>"#));
        assert!(!body.contains(r#"cx="5486400""#), "the old size is left somewhere: {body}");
        if fb == Fallback::Pict {
            assert!(body.contains("width:300pt;height:150pt"), "{body}");
        }
        if fb != Fallback::None {
            assert!(body.contains("<mc:Fallback>"));
        }
        // Undo puts the old size back.
        assert!(doc.undo());
        assert_eq!(segs(&doc, "chart")[0].1["img"]["w"], 432.0);
    }
}

#[test]
fn a_new_style_chart_can_float_and_come_back_inline() {
    let mut doc = open_chartex(&tree_xml("treemap", false, ""), Fallback::Drawing);
    let (p, _) = segs(&doc, "chart").remove(0);
    ops(&mut doc, json!({"op":"set_image","at":{"p":p,"o":0},"wrap":"square","align":"right"}));
    let seg = segs(&doc, "chart").remove(0).1;
    assert_eq!(seg["img"]["wrap"], "square");
    assert_eq!(seg["img"]["align"], "right");
    assert_eq!(seg["chartex"]["kind"], "treemap");
    let pkg = saved(&mut doc);
    assert!(text(&pkg, "word/document.xml").contains("<mc:AlternateContent"));
    ops(&mut doc, json!({"op":"set_image","at":{"p":p,"o":0},"wrap":"inline"}));
    assert_eq!(segs(&doc, "chart")[0].1["img"]["wrap"], "inline");
    saved(&mut doc);
}

#[test]
fn deleting_the_chart_and_undoing_it_brings_back_the_drawing() {
    let mut doc = open_chartex(&funnel_xml(), Fallback::Drawing);
    let (p, _) = segs(&doc, "chart").remove(0);
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":1}}));
    assert!(segs(&doc, "chart").is_empty());
    assert!(doc.undo());
    let found = segs(&doc, "chart");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1["chartex"]["kind"], "funnel");
    saved(&mut doc);
}

#[test]
fn a_chart_that_is_not_the_new_kind_has_no_chartex() {
    let doc = Document::new_blank();
    assert!(segs(&doc, "chart").is_empty());
    assert!(!chartex::is_chartex_drawing(&docxtor_core::xml::Element::new("w:drawing")));
}

#[test]
fn the_assistant_sees_a_new_style_chart_and_the_chart_tools_leave_it_alone() {
    let mut doc = open_chartex(&waterfall_xml(), Fallback::Drawing);
    let script = docscript(&mut doc, &ScriptOpts::default());
    assert!(script.contains("![Waterfall chart \"Cash flow\": 6 points]"), "{script}");
    let (p, _) = segs(&doc, "chart").remove(0);
    // set_chart needs a classic chart; the document stays as it was.
    let before = doc.save().unwrap();
    let err = call_tool(&mut doc, "set_chart", &json!({"id": p, "title": "New title"})).expect_err("set_chart on a chartex chart");
    assert!(err.contains("chart"), "{err}");
    assert_eq!(doc.save().unwrap(), before);
    assert_eq!(model_of(&waterfall_xml()).summary(), "Waterfall chart \"Cash flow\": 6 points");
    assert_eq!(model_of(&tree_xml("sunburst", false, "")).summary(), "Sunburst chart \"Produce\": 2 groups");
    assert_eq!(ChartExModel::default().summary(), "Chart");
}

#[test]
fn exports_name_a_new_style_chart_instead_of_dropping_it() {
    let doc = open_chartex(&waterfall_xml(), Fallback::Drawing);
    let html = docxtor_core::export::html::to_html(&doc);
    assert!(html.contains("<figure class=\"chart\"") && html.contains("Waterfall chart &quot;Cash flow&quot;: 6 points") || html.contains("Waterfall chart \"Cash flow\": 6 points"), "{html}");
    let rtf = docxtor_core::export::rtf::to_rtf(&doc);
    assert!(rtf.contains("[Chart: Cash flow]"), "{rtf}");
}
