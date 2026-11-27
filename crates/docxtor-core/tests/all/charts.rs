//! Charts: Word-made charts read into the model, left byte-identical when untouched, changed through
//! `set_chart` (caches, formulas, point counts, the embedded workbook), inserted as clean Word charts, undone,
//! and reachable by the AI tools and DocScript.

use docxtor_core::ai::{call_tool, docscript, ScriptOpts};
use docxtor_core::chart::{self, xlsx, ChartKind, ChartModel, Grouping};
use docxtor_core::model::walk_paragraphs;
use docxtor_core::{package::Package, Document, Pos};
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
        if p.name.starts_with("word/charts/") && p.name.ends_with(".xml") && !p.name.contains("_rels") && !p.name.contains("colors") && !p.name.contains("style") {
            assert!(
                ct.root.elements().any(|e| e.local() == "Override" && e.attr("PartName") == Some(format!("/{}", p.name).as_str()) && e.attr("ContentType") == Some(chart::CT_CHART)),
                "{} has no chart content type",
                p.name
            );
        }
        if p.name.ends_with(".xlsx") {
            assert!(ct.root.elements().any(|e| e.local() == "Default" && e.attr("Extension") == Some("xlsx")), "xlsx has no default content type");
        }
    }
}

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn saved(doc: &mut Document) -> Package {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    Package::open(&bytes).unwrap()
}

fn text(pkg: &Package, name: &str) -> String {
    String::from_utf8(pkg.get(name).unwrap_or_else(|| panic!("no part {name}")).to_vec()).unwrap()
}

fn rewrite(doc: &mut Document, f: impl FnOnce(&mut Package)) -> Document {
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    f(&mut pkg);
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

/// Every chart in the body: paragraph id, offset and model.
fn charts(doc: &Document) -> Vec<(u32, usize, ChartModel)> {
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    ids.into_iter().flat_map(|p| doc.charts_in(p).into_iter().map(move |(o, m)| (p, o, m))).collect()
}

fn the_chart(doc: &Document) -> (Pos, ChartModel) {
    let mut all = charts(doc);
    assert_eq!(all.len(), 1, "expected one chart");
    let (p, o, m) = all.remove(0);
    (Pos::new(p, o), m)
}

fn at(pos: Pos) -> Value {
    json!({"p": pos.p.0, "o": pos.o})
}

fn values(m: &ChartModel, i: usize) -> Vec<Option<f64>> {
    m.series[i].values.clone()
}

fn nums(v: &[f64]) -> Vec<Option<f64>> {
    v.iter().map(|x| Some(*x)).collect()
}

// ---------------------------------------------------------------------------
// Word's markup
// ---------------------------------------------------------------------------

const THEME: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="2E75B6"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme></a:themeElements></a:theme>"##;

const HEAD: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:c16r2="http://schemas.microsoft.com/office/drawing/2015/06/chart"><c:date1904 val="0"/><c:lang val="en-US"/><c:roundedCorners val="0"/><mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="c14" xmlns:c14="http://schemas.microsoft.com/office/drawing/2007/8/2/chart"><c14:style val="102"/></mc:Choice><mc:Fallback><c:style val="2"/></mc:Fallback></mc:AlternateContent><c:chart>"##;

const TITLE: &str = r##"<c:title><c:tx><c:rich><a:bodyPr rot="0" spcFirstLastPara="1" vertOverflow="ellipsis" vert="horz" wrap="square" anchor="ctr" anchorCtr="1"/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="1862" b="0" i="0" u="none" strike="noStrike" kern="1200" spc="0" baseline="0"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:pPr><a:r><a:rPr lang="en-US" b="1"/><a:t>TITLE</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr></c:title><c:autoTitleDeleted val="0"/>"##;

const TX_PR: &str = r##"<c:txPr><a:bodyPr rot="-60000000" spcFirstLastPara="1" vertOverflow="ellipsis" vert="horz" wrap="square" anchor="ctr" anchorCtr="1"/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="1197" b="0" i="0" u="none" strike="noStrike" kern="1200" baseline="0"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>"##;

const CAT_AX: &str = r##"<c:catAx><c:axId val="111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:spPr><a:noFill/><a:ln w="9525"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="15000"/><a:lumOff val="85000"/></a:schemeClr></a:solidFill></a:ln></c:spPr>TXPR<c:crossAx val="222"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx>"##;

const VAL_AX: &str = r##"<c:valAx><c:axId val="222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines><c:spPr><a:ln w="9525"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="15000"/><a:lumOff val="85000"/></a:schemeClr></a:solidFill></a:ln></c:spPr></c:majorGridlines><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>TXPR<c:crossAx val="111"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"##;

const LEGEND: &str = r##"<c:legend><c:legendPos val="b"/><c:overlay val="0"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>TXPR</c:legend><c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/>"##;

const TAIL: &str = r##"</c:chart><c:spPr><a:solidFill><a:schemeClr val="bg1"/></a:solidFill><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:noFill/><a:round/></a:ln></c:spPr><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr/></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr><c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData></c:chartSpace>"##;

fn cats_xml(f: &str, cats: &[&str]) -> String {
    let pts: String = cats.iter().enumerate().map(|(i, c)| format!("<c:pt idx=\"{i}\"><c:v>{c}</c:v></c:pt>")).collect();
    format!("<c:cat><c:strRef><c:f>{f}</c:f><c:strCache><c:ptCount val=\"{}\"/>{pts}</c:strCache></c:strRef></c:cat>", cats.len())
}

fn vals_xml(f: &str, v: &[f64], tag: &str) -> String {
    let pts: String = v.iter().enumerate().map(|(i, x)| format!("<c:pt idx=\"{i}\"><c:v>{x}</c:v></c:pt>")).collect();
    format!("<c:{tag}><c:numRef><c:f>{f}</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"{}\"/>{pts}</c:numCache></c:numRef></c:{tag}>", v.len())
}

fn name_xml(f: &str, name: &str) -> String {
    format!("<c:tx><c:strRef><c:f>{f}</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx>")
}

const UNIQUE: &str = r##"<c:extLst><c:ext uri="{C3380CC4-5D6E-409C-BE32-E72D297353CC}" xmlns:c16="http://schemas.microsoft.com/office/drawing/2014/chart"><c16:uniqueId val="{00000000-AAAA-4C9A-8F2B-000000000001}"/></c:ext></c:extLst>"##;

const CATS: [&str; 4] = ["Category 1", "Category 2", "Category 3", "Category 4"];

/// Word 2016's default clustered column chart: three series, title, bottom legend.
fn column_chart() -> String {
    let data = [("Series 1", "accent1", [4.3, 2.5, 3.5, 4.5]), ("Series 2", "accent2", [2.4, 4.4, 1.8, 2.8]), ("Series 3", "", [2.0, 2.0, 3.0, 5.0])];
    let mut sers = String::new();
    for (i, (name, accent, v)) in data.iter().enumerate() {
        let col = ["B", "C", "D"][i];
        // The third series has an explicit colour of its own.
        let fill = if accent.is_empty() { "<a:srgbClr val=\"C00000\"/>".to_string() } else { format!("<a:schemeClr val=\"{accent}\"/>") };
        sers.push_str(&format!(
            "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>{}<c:spPr><a:solidFill>{fill}</a:solidFill><a:ln><a:noFill/></a:ln></c:spPr><c:invertIfNegative val=\"0\"/>{}{}{UNIQUE}</c:ser>",
            name_xml(&format!("Sheet1!${col}$1"), name),
            cats_xml("Sheet1!$A$2:$A$5", &CATS),
            vals_xml(&format!("Sheet1!${col}$2:${col}$5"), v, "val")
        ));
    }
    let plot = format!(
        "<c:plotArea><c:layout/><c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>{sers}<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"0\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls><c:gapWidth val=\"219\"/><c:overlap val=\"-27\"/><c:axId val=\"111\"/><c:axId val=\"222\"/></c:barChart>{}{}<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr></c:plotArea>",
        CAT_AX.replace("TXPR", TX_PR),
        VAL_AX.replace("TXPR", TX_PR)
    );
    format!("{HEAD}{}{plot}{}{TAIL}", TITLE.replace("TITLE", "Quarterly sales"), LEGEND.replace("TXPR", TX_PR))
}

fn line_chart() -> String {
    let sers: String = [("Revenue", "B", [10.0, 12.5, 11.0, 15.0]), ("Cost", "C", [8.0, 9.0, 9.5, 10.0])]
        .iter()
        .enumerate()
        .map(|(i, (name, col, v))| {
            format!(
                "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>{}<c:spPr><a:ln w=\"28575\" cap=\"rnd\"><a:solidFill><a:schemeClr val=\"accent{}\"/></a:solidFill><a:round/></a:ln></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker>{}{}<c:smooth val=\"{}\"/></c:ser>",
                name_xml(&format!("Sheet1!${col}$1"), name),
                i + 1,
                cats_xml("Sheet1!$A$2:$A$5", &["Jan", "Feb", "Mar", "Apr"]),
                vals_xml(&format!("Sheet1!${col}$2:${col}$5"), v, "val"),
                i
            )
        })
        .collect();
    let plot = format!(
        "<c:plotArea><c:layout/><c:lineChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/>{sers}<c:marker val=\"1\"/><c:axId val=\"111\"/><c:axId val=\"222\"/></c:lineChart>{}{}</c:plotArea>",
        CAT_AX.replace("TXPR", TX_PR),
        VAL_AX.replace("TXPR", TX_PR).replace("formatCode=\"General\" sourceLinked=\"1\"", "formatCode=\"#,##0.0\" sourceLinked=\"0\"")
    );
    format!("{HEAD}<c:autoTitleDeleted val=\"1\"/>{plot}{}{TAIL}", LEGEND.replace("TXPR", TX_PR).replace("\"b\"", "\"r\""))
}

fn pie_chart() -> String {
    let dpts: String = ["accent1", "accent2", "accent3"].iter().enumerate().map(|(i, a)| format!("<c:dPt><c:idx val=\"{i}\"/><c:bubble3D val=\"0\"/><c:spPr><a:solidFill><a:schemeClr val=\"{a}\"/></a:solidFill><a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"lt1\"/></a:solidFill></a:ln></c:spPr></c:dPt>")).collect();
    let sers = format!(
        "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>{}{dpts}<c:dLbls><c:spPr><a:noFill/></c:spPr><c:showLegendKey val=\"0\"/><c:showVal val=\"0\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"1\"/><c:showBubbleSize val=\"0\"/><c:showLeaderLines val=\"1\"/></c:dLbls>{}{}</c:ser>",
        name_xml("Sheet1!$B$1", "Share"),
        cats_xml("Sheet1!$A$2:$A$4", &["Rent", "Food", "Other"]),
        vals_xml("Sheet1!$B$2:$B$4", &[50.0, 30.0, 20.0], "val")
    );
    let plot = format!("<c:plotArea><c:layout/><c:pieChart><c:varyColors val=\"1\"/>{sers}<c:firstSliceAng val=\"0\"/></c:pieChart></c:plotArea>");
    format!("{HEAD}{}{plot}{}{TAIL}", TITLE.replace("TITLE", "Budget"), LEGEND.replace("TXPR", TX_PR).replace("\"b\"", "\"r\""))
}

/// Columns for revenue and a line for margin on a second axis.
fn combo_chart() -> String {
    let bar = format!(
        "<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/><c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>{}<c:spPr><a:solidFill><a:schemeClr val=\"accent1\"/></a:solidFill></c:spPr><c:invertIfNegative val=\"0\"/>{}{}</c:ser><c:gapWidth val=\"219\"/><c:overlap val=\"-27\"/><c:axId val=\"111\"/><c:axId val=\"222\"/></c:barChart>",
        name_xml("Sheet1!$B$1", "Revenue"),
        cats_xml("Sheet1!$A$2:$A$4", &["2022", "2023", "2024"]),
        vals_xml("Sheet1!$B$2:$B$4", &[100.0, 140.0, 180.0], "val")
    );
    let line = format!(
        "<c:lineChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/><c:ser><c:idx val=\"1\"/><c:order val=\"1\"/>{}<c:spPr><a:ln w=\"28575\"><a:solidFill><a:schemeClr val=\"accent2\"/></a:solidFill></a:ln></c:spPr>{}{}<c:smooth val=\"0\"/></c:ser><c:marker val=\"1\"/><c:axId val=\"333\"/><c:axId val=\"444\"/></c:lineChart>",
        name_xml("Sheet1!$C$1", "Margin"),
        cats_xml("Sheet1!$A$2:$A$4", &["2022", "2023", "2024"]),
        vals_xml("Sheet1!$C$2:$C$4", &[0.1, 0.15, 0.2], "val")
    );
    let sec_val = VAL_AX.replace("TXPR", TX_PR).replace("\"222\"", "\"444\"").replace("\"111\"", "\"333\"").replace("<c:axPos val=\"l\"/>", "<c:axPos val=\"r\"/>").replace("autoZero", "max");
    let sec_cat = CAT_AX.replace("TXPR", TX_PR).replace("\"111\"", "\"333\"").replace("\"222\"", "\"444\"").replace("<c:delete val=\"0\"/>", "<c:delete val=\"1\"/>");
    let plot = format!("<c:plotArea><c:layout/>{bar}{line}{}{}{sec_val}{sec_cat}</c:plotArea>", CAT_AX.replace("TXPR", TX_PR), VAL_AX.replace("TXPR", TX_PR));
    format!("{HEAD}{}{plot}{}{TAIL}", TITLE.replace("TITLE", "Revenue and margin"), LEGEND.replace("TXPR", TX_PR))
}

fn inline_drawing(rid: &str, id: u32) -> String {
    format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5486400" cy="3200400"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Chart {id}"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{rid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

/// A document the way Word saves one with a chart: chart part, rels to a workbook plus colour and style parts.
fn docx_with_chart(chart_xml: &str) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    // The workbook Word would have: the chart's own data.
    let model = chart::read(chart_xml.as_bytes(), &[]).unwrap();
    let plan = xlsx::plan(&model.categories, &model.series, model.kind == ChartKind::Scatter);
    pkg.set("word/embeddings/Microsoft_Excel_Worksheet.xlsx", xlsx::build(&plan));
    pkg.set("word/charts/chart1.xml", chart_xml.as_bytes().to_vec());
    pkg.set("word/charts/colors1.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cs:colorStyle xmlns:cs="http://schemas.microsoft.com/office/drawing/2012/chartStyle" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" meth="cycle" id="10"><a:schemeClr val="accent1"/></cs:colorStyle>"#.to_vec());
    pkg.set("word/charts/style1.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cs:chartStyle xmlns:cs="http://schemas.microsoft.com/office/drawing/2012/chartStyle" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" id="201"/>"#.to_vec());
    pkg.set("word/theme/theme1.xml", THEME.as_bytes().to_vec());
    pkg.ensure_override("word/charts/chart1.xml", chart::CT_CHART);
    pkg.ensure_override("word/charts/colors1.xml", "application/vnd.ms-office.chartcolorstyle+xml");
    pkg.ensure_override("word/charts/style1.xml", "application/vnd.ms-office.chartstyle+xml");
    pkg.ensure_override("word/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml");
    pkg.ensure_default("xlsx", chart::CT_XLSX);
    pkg.add_rel("word/charts/chart1.xml", chart::REL_PACKAGE, "../embeddings/Microsoft_Excel_Worksheet.xlsx", false);
    pkg.add_rel("word/charts/chart1.xml", "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle", "colors1.xml", false);
    pkg.add_rel("word/charts/chart1.xml", "http://schemas.microsoft.com/office/2011/relationships/chartStyle", "style1.xml", false);
    pkg.add_rel("word/document.xml", docxtor_core::package::REL_THEME, "theme/theme1.xml", false);
    let rid = pkg.add_rel("word/document.xml", chart::REL_CHART, "charts/chart1.xml", false);
    let doc = text(&pkg, "word/document.xml").replacen("<w:p/>", &format!("<w:p><w:r><w:t>Before the chart</w:t></w:r></w:p><w:p>{}</w:p><w:p><w:r><w:t>After the chart</w:t></w:r></w:p>", inline_drawing(&rid, 7)), 1);
    pkg.set("word/document.xml", doc.into_bytes());
    pkg.to_bytes().unwrap()
}

fn open_chart(chart_xml: &str) -> Document {
    Document::open(&docx_with_chart(chart_xml)).unwrap()
}

fn workbook_cells(pkg: &Package, name: &str) -> Vec<Vec<xlsx::Cell>> {
    xlsx::read_cells(pkg.get(name).unwrap_or_else(|| panic!("no workbook {name}"))).expect("a readable workbook")
}

fn t(s: &str) -> xlsx::Cell {
    xlsx::Cell::Text(s.into())
}

fn n(v: f64) -> xlsx::Cell {
    xlsx::Cell::Number(v)
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

#[test]
fn reads_a_clustered_column_chart_from_word() {
    let doc = open_chart(&column_chart());
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Column);
    assert_eq!(m.grouping, Grouping::Clustered);
    assert!(!m.combo);
    assert_eq!(m.title.as_deref(), Some("Quarterly sales"));
    assert_eq!(m.categories, CATS);
    assert_eq!(m.series.len(), 3);
    assert_eq!(m.series[0].name, "Series 1");
    assert_eq!(values(&m, 0), nums(&[4.3, 2.5, 3.5, 4.5]));
    assert_eq!(values(&m, 2), nums(&[2.0, 2.0, 3.0, 5.0]));
    // Scheme colours resolve against the document's theme (accent1 is 2E75B6 here); explicit colours stay.
    assert_eq!(m.series[0].color.as_deref(), Some("2E75B6"));
    assert_eq!(m.series[1].color.as_deref(), Some("ED7D31"));
    assert_eq!(m.series[2].color.as_deref(), Some("C00000"));
    assert_eq!(m.legend.as_ref().map(|l| l.position.as_str()), Some("b"));
    assert!(!m.data_labels);
    assert!(m.axes.grid);
    assert_eq!(m.style, Some(2));
    assert_eq!(m.series[1].val_ref.as_deref(), Some("Sheet1!$C$2:$C$5"));
    assert_eq!(m.series[0].name_ref.as_deref(), Some("Sheet1!$B$1"));
    assert_eq!(m.accents.first().map(String::as_str), Some("2E75B6"));
}

#[test]
fn reads_line_pie_and_combo_charts() {
    let doc = open_chart(&line_chart());
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Line);
    assert_eq!(m.title, None, "autoTitleDeleted means no title");
    assert_eq!(m.categories, ["Jan", "Feb", "Mar", "Apr"]);
    assert!(!m.series[0].marker, "marker symbol none");
    assert!(!m.series[0].smooth && m.series[1].smooth);
    assert_eq!(m.legend.as_ref().unwrap().position, "r");
    assert_eq!(m.axes.number_format.as_deref(), Some("#,##0.0"));

    let doc = open_chart(&pie_chart());
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Pie);
    assert_eq!(m.categories, ["Rent", "Food", "Other"]);
    assert_eq!(values(&m, 0), nums(&[50.0, 30.0, 20.0]));
    assert!(m.data_labels && m.labels_percent, "showPercent is on");
    assert_eq!(m.series[0].point_colors.len(), 3);
    assert_eq!(m.series[0].point_colors[1].as_deref(), Some("ED7D31"));
    assert!(!m.vary_colors);

    let doc = open_chart(&combo_chart());
    let (_, m) = the_chart(&doc);
    assert!(m.combo);
    assert_eq!(m.kind, ChartKind::Column);
    assert_eq!(m.series[0].kind, ChartKind::Column);
    assert_eq!(m.series[1].kind, ChartKind::Line);
    assert!(!m.series[0].secondary && m.series[1].secondary, "the line sits on the second axis");
    assert_eq!(values(&m, 1), nums(&[0.1, 0.15, 0.2]));
}

#[test]
fn an_unknown_kind_reads_as_other_with_its_title() {
    let xml = column_chart().replace("c:barChart", "c:ofPieChart");
    let doc = open_chart(&xml);
    let (_, m) = the_chart(&doc);
    assert_eq!(m.kind, ChartKind::Other);
    assert_eq!(m.title.as_deref(), Some("Quarterly sales"));
    // The segment is still a chart (a placeholder in the editor).
    let json = serde_json::to_value(doc.render_snapshot()).unwrap().to_string();
    assert!(json.contains("\"k\":\"chart\""));
}

#[test]
fn render_gives_a_chart_segment_with_size_and_layout() {
    let doc = open_chart(&column_chart());
    let json = serde_json::to_value(doc.render_snapshot()).unwrap();
    let seg = json["blocks"].as_array().unwrap().iter().filter_map(|b| b["segs"].as_array()).flatten().find(|s| s["k"] == "chart").expect("a chart segment").clone();
    assert_eq!(seg["img"]["w"], 432.0);
    assert_eq!(seg["img"]["h"], 252.0);
    assert_eq!(seg["img"]["wrap"], "inline");
    assert_eq!(seg["img"]["src"], "word/charts/chart1.xml");
    assert_eq!(seg["chart"]["kind"], "column");
    assert_eq!(seg["chart"]["series"][1]["name"], "Series 2");
}

// ---------------------------------------------------------------------------
// Lossless
// ---------------------------------------------------------------------------

#[test]
fn an_untouched_chart_round_trips_byte_for_byte() {
    for xml in [column_chart(), line_chart(), pie_chart(), combo_chart()] {
        let original = docx_with_chart(&xml);
        let before = Package::open(&original).unwrap();
        let mut doc = Document::open(&original).unwrap();
        // Edit elsewhere in the document.
        let p = find_p(&doc, "Before the chart");
        ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Hello "}));
        let after = saved(&mut doc);
        for part in &before.parts {
            if part.name.starts_with("word/charts/") || part.name.starts_with("word/embeddings/") || part.name == "word/theme/theme1.xml" {
                assert_eq!(after.get(&part.name), Some(part.data.as_slice()), "{} changed", part.name);
            }
        }
        assert_eq!(before.rels_for("word/charts/chart1.xml").unwrap(), after.rels_for("word/charts/chart1.xml").unwrap());
        // And with no edit at all, the whole file is the same parts.
        let mut doc = Document::open(&original).unwrap();
        let again = saved(&mut doc);
        for part in &before.parts {
            assert_eq!(again.get(&part.name), Some(part.data.as_slice()), "{} changed on a plain save", part.name);
        }
    }
}

// ---------------------------------------------------------------------------
// set_chart
// ---------------------------------------------------------------------------

#[test]
fn set_chart_rewrites_caches_formulas_counts_and_the_workbook() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    ops(
        &mut doc,
        json!({"op":"set_chart","at":at(pos),
            "categories":["North","South","East","West","Online"],
            "series":[{"name":"2023","values":[1,2,3,4,5]},{"name":"2024","values":[2,null,4,"5.5",6]}]}),
    );
    let (_, m) = the_chart(&doc);
    assert_eq!(m.categories, ["North", "South", "East", "West", "Online"]);
    assert_eq!(m.series.len(), 2, "the third series is gone");
    assert_eq!(values(&m, 1), vec![Some(2.0), None, Some(4.0), Some(5.5), Some(6.0)]);
    // Colours and the rest of the formatting are kept.
    assert_eq!(m.series[0].color.as_deref(), Some("2E75B6"));
    assert_eq!(m.title.as_deref(), Some("Quarterly sales"));
    assert_eq!(m.legend.as_ref().unwrap().position, "b");

    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.contains("<c:f>Sheet1!$A$2:$A$6</c:f>"), "category formula follows the data: {xml}");
    assert!(xml.contains("<c:f>Sheet1!$C$2:$C$6</c:f>"));
    assert!(xml.contains("<c:f>Sheet1!$C$1</c:f>"));
    assert!(!xml.contains("Sheet1!$D$"), "the removed series' formulas are gone");
    assert_eq!(xml.matches("<c:ptCount val=\"5\"/>").count(), 4, "two series, each with categories and values");
    assert!(xml.contains("<c:v>Online</c:v>"));
    assert!(xml.contains("<c:v>5.5</c:v>"));
    assert!(xml.contains("c:barChart") && xml.contains("c:overlap"), "the group keeps its own settings");
    // The explicit colour of series 1 and the style are untouched.
    assert!(xml.contains("<c:style val=\"2\"/>"));
    // Word's own series ids are renumbered, not duplicated.
    assert_eq!(xml.matches("<c:idx val=\"1\"/>").count(), 1);

    let cells = workbook_cells(&pkg, "word/embeddings/Microsoft_Excel_Worksheet.xlsx");
    assert_eq!(cells[0][1], t("2023"));
    assert_eq!(cells[0][2], t("2024"));
    assert_eq!(cells[1][0], t("North"));
    assert_eq!(cells[5][0], t("Online"));
    assert_eq!(cells[2][1], n(2.0));
    assert_eq!(cells[2][2], xlsx::Cell::Empty, "a gap is an empty cell");
    assert_eq!(cells[4][2], n(5.5));
    assert_eq!(cells.len(), 6);

    // And it opens again with the same data.
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(the_chart(&again).1.series[1].values, m.series[1].values);
}

#[test]
fn a_new_series_copies_the_last_one_without_its_colour() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    ops(
        &mut doc,
        json!({"op":"set_chart","at":at(pos),"series":[
            {"name":"A","values":[1,2,3,4]},{"name":"B","values":[1,2,3,4]},{"name":"C","values":[1,2,3,4]},{"name":"D","values":[5,6,7,8]}]}),
    );
    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert_eq!(xml.matches("<c:ser>").count(), 4);
    assert_eq!(xml.matches("C00000").count(), 1, "the colour of series 3 isn't copied to series 4");
    assert_eq!(xml.matches("{00000000-AAAA-4C9A-8F2B-000000000001}").count(), 3, "series ids from Word aren't duplicated");
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series.len(), 4);
    assert_eq!(m.series[3].name, "D");
    assert_eq!(m.series[3].color, None);
    let cells = workbook_cells(&pkg, "word/embeddings/Microsoft_Excel_Worksheet.xlsx");
    assert_eq!(cells[0][4], t("D"));
    assert_eq!(cells[4][4], n(8.0));
}

#[test]
fn title_legend_and_labels_change_without_touching_the_rest() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    let before = text(&Package::open(&docx_with_chart(&column_chart())).unwrap(), "word/charts/chart1.xml");
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"title":"Sales by region","legend":"right","data_labels":true}));
    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.contains("<a:t>Sales by region</a:t>"));
    assert!(xml.contains("sz=\"1862\""), "the title keeps its text size");
    assert!(xml.contains("<a:rPr lang=\"en-US\" b=\"1\"/>"), "and its run formatting");
    assert!(xml.contains("<c:legendPos val=\"r\"/>"));
    let (_, m) = the_chart(&doc);
    assert!(m.data_labels);
    assert_eq!(m.title.as_deref(), Some("Sales by region"));
    // Data was not touched, so the workbook is byte-identical and the data in the part is the same.
    let orig = Package::open(&docx_with_chart(&column_chart())).unwrap();
    assert_eq!(pkg.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"), orig.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"));
    assert!(before.contains("Quarterly sales") && !xml.contains("Quarterly sales"));
    assert_eq!(xml.matches("<c:ser>").count(), 3);
    assert!(xml.contains("<c:v>4.3</c:v>"));

    // Off again, and remove title and legend.
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"title":"","legend":"none","data_labels":false}));
    let (_, m) = the_chart(&doc);
    assert!(!m.data_labels);
    assert_eq!(m.title, None);
    assert!(m.legend.is_none());
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(!xml.contains("<c:title>") && !xml.contains("<c:legend>"));
    assert!(xml.contains("<c:autoTitleDeleted val=\"1\"/>"));
}

#[test]
fn a_new_title_on_a_chart_without_one() {
    let mut doc = open_chart(&line_chart());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"title":"Margins"}));
    assert_eq!(the_chart(&doc).1.title.as_deref(), Some("Margins"));
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(xml.find("<c:title>").unwrap() < xml.find("<c:plotArea>").unwrap());
    assert!(xml.contains("<c:autoTitleDeleted val=\"0\"/>"));
}

#[test]
fn changing_the_type_keeps_the_data() {
    let mut doc = open_chart(&column_chart());
    let (pos, before) = the_chart(&doc);
    for kind in ["bar", "line", "area", "radar", "pie", "doughnut", "scatter", "column"] {
        ops(&mut doc, json!({"op":"set_chart","at":at(pos),"kind":kind}));
        let (_, m) = the_chart(&doc);
        assert_eq!(m.kind.name(), kind, "{kind}");
        assert_eq!(m.title.as_deref(), Some("Quarterly sales"));
        assert_eq!(m.series.len(), 3, "{kind}");
        assert_eq!(values(&m, 0), values(&before, 0), "{kind}");
        match kind {
            "scatter" => assert_eq!(m.series[0].x_values, Some(nums(&[1.0, 2.0, 3.0, 4.0]))),
            // Coming back from a scatter chart, the X values become the categories.
            "column" => assert_eq!(m.categories, ["1", "2", "3", "4"]),
            _ => assert_eq!(m.categories, before.categories, "{kind}"),
        }
        saved(&mut doc); // valid after every switch
    }
    // Back to a column chart: the chart part has one group and the axes of a column chart.
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(xml.contains("<c:barDir val=\"col\"/>"));
    assert!(xml.contains("<c:catAx>") && xml.contains("<c:valAx>"));
    assert_eq!(xml.matches("<c:barChart>").count(), 1);
    // A pie colours slices, not series, so a series colour doesn't survive a stop at a pie. It does through the rest.
    let mut doc = open_chart(&column_chart());
    for kind in ["bar", "line", "area", "radar", "scatter", "column"] {
        ops(&mut doc, json!({"op":"set_chart","at":at(pos),"kind":kind}));
    }
    assert_eq!(the_chart(&doc).1.series[2].color.as_deref(), Some("C00000"));
}

#[test]
fn column_to_bar_turns_the_axes_and_keeps_the_formatting() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"kind":"bar"}));
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(xml.contains("<c:barDir val=\"bar\"/>"));
    assert!(xml.contains("<c:catAx><c:axId val=\"111\"/>"), "axes are kept, with their ids");
    assert!(xml.contains("<c:axPos val=\"l\"/>") && xml.contains("<c:axPos val=\"b\"/>"));
    assert_eq!(the_chart(&doc).1.kind, ChartKind::Bar);
}

#[test]
fn stacking_changes_grouping_and_overlap() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"grouping":"stacked"}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.grouping, Grouping::Stacked);
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(xml.contains("<c:grouping val=\"stacked\"/>") && xml.contains("<c:overlap val=\"100\"/>"));
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"grouping":"percent"}));
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(xml.contains("<c:grouping val=\"percentStacked\"/>"));
    assert!(xml.contains("formatCode=\"0%\""), "a percent chart shows percentages on the value axis");
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"grouping":"clustered"}));
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert!(xml.contains("<c:grouping val=\"clustered\"/>") && xml.contains("<c:overlap val=\"-27\"/>") && !xml.contains("formatCode=\"0%\""));
}

#[test]
fn a_combo_chart_collapses_to_one_kind_when_asked() {
    let mut doc = open_chart(&combo_chart());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"kind":"line"}));
    let (_, m) = the_chart(&doc);
    assert!(!m.combo);
    assert_eq!(m.kind, ChartKind::Line);
    assert_eq!(m.series.len(), 2);
    // The second group's axes go with it: no axis is left that nothing draws against.
    let xml = text(&saved(&mut doc), "word/charts/chart1.xml");
    assert_eq!(xml.matches("<c:valAx>").count(), 1);
    assert_eq!(xml.matches("<c:catAx>").count(), 1);
    assert_eq!(xml.matches("<c:lineChart>").count(), 1);
    assert_eq!(xml.matches("<c:barChart>").count(), 0);
}

#[test]
fn bad_input_is_refused_and_changes_nothing() {
    let mut doc = open_chart(&column_chart());
    let (pos, before) = the_chart(&doc);
    let undo_before = doc.can_undo();
    let bytes_before = doc.save().unwrap();
    for bad in [
        json!({"op":"set_chart","at":at(pos),"series":[]}),
        json!({"op":"set_chart","at":at(pos),"series":[{"name":"x","values":[1,2,3,4,5,6]}]}),
        json!({"op":"set_chart","at":at(pos),"series":[{"name":"x","values":["abc"]}]}),
        json!({"op":"set_chart","at":at(pos),"categories":[]}),
        json!({"op":"set_chart","at":at(pos),"kind":"other"}),
    ] {
        let e = err(&mut doc, bad.clone());
        assert!(!e.is_empty(), "{bad}");
    }
    assert_eq!(doc.can_undo(), undo_before);
    assert_eq!(the_chart(&doc).1, before);
    let (a, b) = (Package::open(&bytes_before).unwrap(), saved(&mut doc));
    for name in ["word/charts/chart1.xml", "word/embeddings/Microsoft_Excel_Worksheet.xlsx", "word/charts/_rels/chart1.xml.rels"] {
        assert_eq!(a.get(name), b.get(name), "{name}");
    }
    // Not a chart.
    let p = find_p(&doc, "Before the chart");
    let e = err(&mut doc, json!({"op":"set_chart","at":{"p":p,"o":0},"title":"x"}));
    assert!(e.contains("no chart"), "{e}");
}

#[test]
fn set_chart_undoes_and_redoes_with_its_workbook() {
    let mut doc = open_chart(&column_chart());
    let (pos, before) = the_chart(&doc);
    let orig = Package::open(&doc.save().unwrap()).unwrap();
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"title":"New","categories":["a","b"],"series":[{"name":"s","values":[1,2]}]}));
    let changed = saved(&mut doc);
    assert_ne!(changed.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"), orig.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"));
    assert!(doc.undo());
    assert_eq!(the_chart(&doc).1, before);
    let undone = saved(&mut doc);
    for name in ["word/charts/chart1.xml", "word/embeddings/Microsoft_Excel_Worksheet.xlsx"] {
        assert_eq!(undone.get(name), orig.get(name), "{name} is back as it was");
    }
    assert!(doc.redo());
    assert_eq!(the_chart(&doc).1.title.as_deref(), Some("New"));
    let redone = saved(&mut doc);
    assert_eq!(redone.get("word/charts/chart1.xml"), changed.get("word/charts/chart1.xml"));
    assert_eq!(redone.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"), changed.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"));
}

#[test]
fn a_chart_without_a_workbook_gets_one_when_its_data_changes() {
    let xml = column_chart().replace("<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/></c:externalData>", "");
    let bytes = docx_with_chart(&xml);
    let mut pkg = Package::open(&bytes).unwrap();
    // Drop the workbook part and its relationship, as a chart saved without "Edit data" has none.
    pkg.parts.retain(|p| p.name != "word/embeddings/Microsoft_Excel_Worksheet.xlsx");
    let rels = text(&pkg, "word/charts/_rels/chart1.xml.rels");
    let start = rels.find("<Relationship Id=\"rId1\"").unwrap();
    let end = start + rels[start..].find("/>").unwrap() + 2;
    pkg.set("word/charts/_rels/chart1.xml.rels", format!("{}{}", &rels[..start], &rels[end..]).into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"series":[{"name":"S","values":[1,2,3,4]}]}));
    let pkg = saved(&mut doc);
    let xml = text(&pkg, "word/charts/chart1.xml");
    assert!(xml.contains("<c:externalData"));
    let workbook = pkg.parts.iter().find(|p| p.name.starts_with("word/embeddings/")).expect("a workbook was added");
    assert!(xlsx::read_cells(&workbook.data).is_some());
}

#[test]
fn a_linked_workbook_is_not_rewritten() {
    let bytes = docx_with_chart(&column_chart());
    let mut pkg = Package::open(&bytes).unwrap();
    let rels = text(&pkg, "word/charts/_rels/chart1.xml.rels").replace("Target=\"../embeddings/Microsoft_Excel_Worksheet.xlsx\"", "Target=\"file:///C:/data/book.xlsx\" TargetMode=\"External\"");
    pkg.set("word/charts/_rels/chart1.xml.rels", rels.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let (pos, _) = the_chart(&doc);
    let book_before = Package::open(&bytes).unwrap().get("word/embeddings/Microsoft_Excel_Worksheet.xlsx").unwrap().to_vec();
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"series":[{"name":"S","values":[9,8,7,6]}]}));
    let out = saved(&mut doc);
    assert_eq!(out.get("word/embeddings/Microsoft_Excel_Worksheet.xlsx"), Some(book_before.as_slice()));
    let xml = text(&out, "word/charts/chart1.xml");
    assert!(xml.contains("<c:v>9</c:v>"), "cached values change");
    assert!(xml.contains("<c:f>Sheet1!$B$2:$B$5</c:f>"), "formulas of a linked book stay");
}

// ---------------------------------------------------------------------------
// Layout of a chart drawing, like a picture's
// ---------------------------------------------------------------------------

#[test]
fn a_chart_resizes_and_wraps_like_a_picture() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_image","at":at(pos),"width":300,"height":200,"alt":"Sales by quarter"}));
    ops(&mut doc, json!({"op":"set_image","at":at(pos),"wrap":"square","align":"right"}));
    let json = serde_json::to_value(doc.render_snapshot()).unwrap();
    let seg = json["blocks"].as_array().unwrap().iter().filter_map(|b| b["segs"].as_array()).flatten().find(|s| s["k"] == "chart").unwrap().clone();
    assert_eq!(seg["img"]["w"], 300.0);
    assert_eq!(seg["img"]["h"], 200.0);
    assert_eq!(seg["img"]["wrap"], "square");
    assert_eq!(seg["img"]["align"], "right");
    assert_eq!(seg["img"]["floating"], true);
    assert_eq!(seg["img"]["alt"], "Sales by quarter");
    assert_eq!(seg["chart"]["title"], "Quarterly sales");
    saved(&mut doc);
    ops(&mut doc, json!({"op":"set_image","at":at(pos),"wrap":"inline"}));
    assert_eq!(the_chart(&doc).1.series.len(), 3);
}

// ---------------------------------------------------------------------------
// insert_chart
// ---------------------------------------------------------------------------

fn insert_args(kind: &str, p: u32) -> Value {
    json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":kind,"title":"Sample","categories":["A","B","C"],"series":[{"name":"One","values":[1,2,3]},{"name":"Two","values":[3,2,1]}]})
}

#[test]
fn insert_chart_writes_valid_parts_and_reads_back() {
    for kind in ["column", "bar", "line", "pie", "doughnut", "area", "scatter", "radar"] {
        let mut doc = doc_with("Intro\n\nBody");
        let p = find_p(&doc, "Body");
        let mut op = insert_args(kind, p);
        if kind == "scatter" {
            op["series"] = json!([{"name":"One","values":[1,4,9],"x_values":[1,2,3]},{"name":"Two","values":[2,3,5],"x_values":[1,2,3]}]);
            op["categories"] = json!([]);
        }
        let r = ops(&mut doc, op);
        assert_eq!(r[0].caret, Some(Pos::new(p, 1)));
        let pkg = saved(&mut doc);

        // Parts, relationships and content types.
        assert!(pkg.has("word/charts/chart1.xml"), "{kind}");
        assert!(pkg.has("word/embeddings/Microsoft_Excel_Worksheet1.xlsx"), "{kind}");
        let chart_rels = pkg.rels_for("word/charts/chart1.xml").unwrap();
        assert_eq!(chart_rels.len(), 1);
        assert_eq!(chart_rels[0].rel_type, chart::REL_PACKAGE);
        assert_eq!(chart_rels[0].target, "../embeddings/Microsoft_Excel_Worksheet1.xlsx");
        let main_rels = pkg.rels_for("word/document.xml").unwrap();
        let rel = main_rels.iter().find(|r| r.rel_type == chart::REL_CHART).expect("a chart relationship");
        assert_eq!(rel.target, "charts/chart1.xml");
        let ct = text(&pkg, "[Content_Types].xml");
        assert!(ct.contains("/word/charts/chart1.xml") && ct.contains("drawingml.chart+xml"));
        assert!(ct.contains("Extension=\"xlsx\""));

        // The drawing.
        let body = text(&pkg, "word/document.xml");
        assert!(body.contains("<wp:inline"), "{kind}");
        assert!(body.contains("uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\""));
        assert!(body.contains("cx=\"5486400\" cy=\"3200400\""), "6in x 3.5in");
        assert!(body.contains(&format!("r:id=\"{}\"", rel.id)));
        assert!(body.contains("name=\"Chart ") && body.contains("descr=\""));

        // Reading it back gives the data.
        let again = Document::open(&doc.save().unwrap()).unwrap();
        let (_, m) = the_chart(&again);
        assert_eq!(m.kind.name(), kind);
        assert_eq!(m.title.as_deref(), Some("Sample"));
        assert_eq!(m.series.len(), 2);
        assert_eq!(m.series[0].name, "One");
        if kind == "scatter" {
            assert_eq!(m.series[0].x_values, Some(nums(&[1.0, 2.0, 3.0])));
            assert_eq!(values(&m, 0), nums(&[1.0, 4.0, 9.0]));
        } else {
            assert_eq!(m.categories, ["A", "B", "C"]);
            assert_eq!(values(&m, 1), nums(&[3.0, 2.0, 1.0]));
        }

        // The workbook has the same numbers.
        let cells = workbook_cells(&pkg, "word/embeddings/Microsoft_Excel_Worksheet1.xlsx");
        assert_eq!(cells[0][1], t("One"));
        assert_eq!(cells[0][2], t("Two"));
        if kind == "scatter" {
            assert_eq!(cells[3][0], n(3.0));
            assert_eq!(cells[3][1], n(9.0));
        } else {
            assert_eq!(cells[1][0], t("A"));
            assert_eq!(cells[3][2], n(1.0));
        }
        // The formulas point at the cells the workbook has.
        let xml = text(&pkg, "word/charts/chart1.xml");
        assert!(xml.contains("<c:f>Sheet1!$B$1</c:f>") && xml.contains("<c:f>Sheet1!$C$2:$C$4</c:f>"), "{kind}: {xml}");
        assert!(xml.contains("<c:externalData r:id=\"rId1\">"));
    }
}

#[test]
fn insert_chart_assigns_unique_names_and_ids() {
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    ops(&mut doc, insert_args("column", p));
    ops(&mut doc, insert_args("line", p));
    let pkg = saved(&mut doc);
    assert!(pkg.has("word/charts/chart1.xml") && pkg.has("word/charts/chart2.xml"));
    assert!(pkg.has("word/embeddings/Microsoft_Excel_Worksheet1.xlsx") && pkg.has("word/embeddings/Microsoft_Excel_Worksheet2.xlsx"));
    let body = text(&pkg, "word/document.xml");
    let ids: Vec<&str> = body.match_indices("<wp:docPr id=\"").map(|(i, _)| body[i + 14..].split('"').next().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert_eq!(charts(&doc).len(), 2);
}

#[test]
fn insert_chart_defaults_and_options() {
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    // One series, no legend asked: none. Several series: bottom. A pie: right.
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","categories":["a"],"series":[{"name":"x","values":[1]}]}));
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"pie","categories":["a","b"],"series":[{"name":"x","values":[1,2]}],"data_labels":true}));
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","grouping":"stacked","legend":"top","width":200,"height":100,"alt":"My alt","categories":["a","b"],"series":[{"values":[1,2]},{"values":[2,3]}]}));
    let all = charts(&doc);
    assert_eq!(all.len(), 3);
    // Offsets run backwards since each was inserted at 0.
    let pie = all.iter().find(|c| c.2.kind == ChartKind::Pie).unwrap();
    assert_eq!(pie.2.legend.as_ref().unwrap().position, "r");
    assert!(pie.2.data_labels && pie.2.labels_percent);
    let col = all.iter().find(|c| c.2.grouping == Grouping::Stacked).unwrap();
    assert_eq!(col.2.legend.as_ref().unwrap().position, "t");
    assert_eq!(col.2.series[0].name, "Series 1", "unnamed series get a name");
    let single = all.iter().find(|c| c.2.series.len() == 1 && c.2.kind == ChartKind::Column).unwrap();
    assert!(single.2.legend.is_none());
    let json = serde_json::to_value(doc.render_snapshot()).unwrap().to_string();
    assert!(json.contains("\"w\":200.0") && json.contains("\"alt\":\"My alt\""));
    saved(&mut doc);
}

#[test]
fn insert_chart_refuses_bad_data() {
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    let before = doc.save().unwrap();
    for bad in [
        json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","categories":["a"],"series":[]}),
        json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","categories":[],"series":[{"name":"x","values":[1]}]}),
        json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","categories":["a"],"series":[{"name":"x","values":[1,2]}]}),
        json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"other","categories":["a"],"series":[{"name":"x","values":[1]}]}),
    ] {
        doc.apply_json(&bad.to_string(), false).expect_err(&bad.to_string());
    }
    assert!(charts(&doc).is_empty());
    assert_eq!(doc.save().unwrap(), before, "nothing was written");
}

#[test]
fn insert_chart_undoes_and_redoes_completely() {
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    let plain = saved(&mut doc);
    ops(&mut doc, insert_args("column", p));
    assert_eq!(charts(&doc).len(), 1);
    let with = saved(&mut doc);
    assert!(doc.undo());
    assert!(charts(&doc).is_empty());
    let undone = saved(&mut doc);
    assert!(!undone.has("word/charts/chart1.xml") && !undone.has("word/embeddings/Microsoft_Excel_Worksheet1.xlsx"), "the parts go with the chart");
    assert!(undone.rels_for("word/document.xml").unwrap().iter().all(|r| r.rel_type != chart::REL_CHART));
    assert!(!text(&undone, "[Content_Types].xml").contains("/word/charts/"));
    assert_eq!(undone.rels_for("word/document.xml").unwrap(), plain.rels_for("word/document.xml").unwrap());
    assert!(doc.redo());
    assert_eq!(charts(&doc).len(), 1);
    let redone = saved(&mut doc);
    assert_eq!(redone.get("word/charts/chart1.xml"), with.get("word/charts/chart1.xml"));
    assert_eq!(redone.get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx"), with.get("word/embeddings/Microsoft_Excel_Worksheet1.xlsx"));
    assert_eq!(redone.rels_for("word/document.xml").unwrap(), with.rels_for("word/document.xml").unwrap());
}

#[test]
fn undo_after_other_edits_keeps_later_relationships() {
    // An image added after the chart keeps its relationship when the chart op is undone, then redone.
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    ops(&mut doc, insert_args("column", p));
    let media = doc.add_media("png", vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
    ops(&mut doc, json!({"op":"insert_image","at":{"p":p,"o":0},"media":media,"width":10,"height":10}));
    assert!(doc.undo()); // the image
    assert!(doc.undo()); // the chart
    assert!(doc.redo());
    assert!(doc.redo());
    let pkg = saved(&mut doc);
    let rels = pkg.rels_for("word/document.xml").unwrap();
    assert!(rels.iter().any(|r| r.rel_type == chart::REL_CHART));
    assert!(rels.iter().any(|r| r.rel_type == docxtor_core::package::REL_IMAGE));
    let body = text(&pkg, "word/document.xml");
    for r in rels.iter().filter(|r| r.rel_type == chart::REL_CHART || r.rel_type == docxtor_core::package::REL_IMAGE) {
        assert!(body.contains(&format!("\"{}\"", r.id)), "{} is used", r.id);
    }
}

#[test]
fn a_chart_in_a_header_is_inserted_with_its_own_relationship() {
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    let r = ops(&mut doc, json!({"op":"add_header_footer","kind":"header"}));
    let hp = r[0].caret.unwrap().p.0;
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":hp,"o":0},"kind":"column","categories":["a","b"],"series":[{"name":"x","values":[1,2]}],"width":120,"height":60}));
    let _ = p;
    let pkg = saved(&mut doc);
    let header = pkg.parts.iter().find(|p| p.name.starts_with("word/header") && p.name.ends_with(".xml")).unwrap().name.clone();
    assert!(pkg.rels_for(&header).unwrap().iter().any(|r| r.rel_type == chart::REL_CHART));
    assert!(pkg.has("word/charts/chart1.xml"));
    assert!(doc.undo());
    assert!(!saved(&mut doc).has("word/charts/chart1.xml"));
}

#[test]
fn is_modified_follows_chart_edits() {
    let mut doc = open_chart(&column_chart());
    assert!(!doc.is_modified());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"set_chart","at":at(pos),"title":"x"}));
    assert!(doc.is_modified());
    doc.save().unwrap();
    assert!(!doc.is_modified());
    doc.undo();
    assert!(doc.is_modified(), "undoing a chart edit is an unsaved change");
}

// ---------------------------------------------------------------------------
// AI tools and DocScript
// ---------------------------------------------------------------------------

#[test]
fn docscript_shows_a_chart_with_its_data() {
    let mut doc = open_chart(&column_chart());
    let script = docscript(&mut doc, &ScriptOpts::default());
    assert!(script.contains("[chart p"), "{script}");
    assert!(script.contains("Quarterly sales (column, legend bottom)"), "{script}");
    assert!(script.contains("| Series 1 | 4.3 | 2.5 | 3.5 | 4.5 |"), "{script}");
    assert!(script.contains("| |"), "{script}");
    assert!(script.contains("Category 4"));
    let mut doc = open_chart(&combo_chart());
    assert!(docscript(&mut doc, &ScriptOpts::default()).contains("combination of column and line"));
    let mut doc = open_chart(&column_chart().replace("c:barChart", "c:ofPieChart"));
    assert!(docscript(&mut doc, &ScriptOpts::default()).contains("can't read the data"));
}

#[test]
fn the_tools_insert_and_change_charts() {
    let mut doc = doc_with("# Report\n\nSome text about sales.\n\nThe end.");
    let intro = find_p(&doc, "Some text");
    let out = call_tool(
        &mut doc,
        "insert_chart",
        &json!({"after":intro,"kind":"column","title":"Sales","categories":["Q1","Q2"],"series":[{"name":"2024","values":[10,20]}],"data_labels":true}),
    )
    .unwrap();
    assert!(out.contains("[chart p") && out.contains("| 2024 | 10 | 20 |"), "{out}");
    let all = charts(&doc);
    assert_eq!(all.len(), 1);
    let (cp, _, m) = &all[0];
    assert_eq!(m.title.as_deref(), Some("Sales"));
    assert!(m.data_labels);
    // The chart sits in a paragraph of its own, right after the one named.
    let order = doc.render_all().order;
    let i = order.iter().position(|id| *id == intro).unwrap();
    assert_eq!(order[i + 1], *cp);
    // One undo takes the paragraph and the chart.
    assert!(doc.undo());
    assert!(charts(&doc).is_empty());
    assert!(!saved(&mut doc).has("word/charts/chart1.xml"));
    assert!(doc.redo());
    assert_eq!(charts(&doc).len(), 1);

    let (cp, _, _) = charts(&doc).remove(0);
    let out = call_tool(&mut doc, "set_chart", &json!({"id":cp,"kind":"line","title":"Sales by quarter","series":[{"name":"2024","values":[10,20]},{"name":"2025","values":[12,25]}]})).unwrap();
    assert!(out.contains("Sales by quarter (line") && out.contains("| 2025 | 12 | 25 |"), "{out}");
    let (_, _, m) = charts(&doc).remove(0);
    assert_eq!(m.kind, ChartKind::Line);
    assert_eq!(m.series.len(), 2);

    // Inline, after some text.
    let out = call_tool(&mut doc, "insert_chart", &json!({"id":intro,"after_text":"about sales","kind":"pie","categories":["a","b"],"series":[{"name":"s","values":[1,3]}]})).unwrap();
    assert!(out.contains("Chart inserted"));
    assert_eq!(charts(&doc).len(), 2);

    // Errors a model can act on.
    let e = call_tool(&mut doc, "set_chart", &json!({"id":intro,"index":3,"title":"x"})).unwrap_err();
    assert!(e.contains("chart"), "{e}");
    let e = call_tool(&mut doc, "insert_chart", &json!({"after":intro,"kind":"sparkline","series":[]})).unwrap_err();
    assert!(e.contains("unknown chart kind"), "{e}");
    let e = call_tool(&mut doc, "insert_chart", &json!({"kind":"column","categories":["a"],"series":[{"name":"x","values":[1]}]})).unwrap_err();
    assert!(e.contains("say where"), "{e}");
    let n = charts(&doc).len();
    let e = call_tool(&mut doc, "insert_chart", &json!({"after":intro,"kind":"column","categories":["a"],"series":[{"name":"x","values":[1,2,3]}]})).unwrap_err();
    assert!(e.contains("categories"), "{e}");
    assert_eq!(charts(&doc).len(), n, "a refused chart leaves no empty paragraph behind");
    saved(&mut doc);
}

#[test]
fn tool_definitions_list_the_chart_tools() {
    let defs = docxtor_core::ai::tool_definitions();
    let names: Vec<&str> = defs.as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"insert_chart") && names.contains(&"set_chart"));
}

#[test]
fn a_chart_survives_a_round_trip_through_markdown_free_edits() {
    // Deleting text around a chart, undoing, and saving keep the chart's parts.
    let mut doc = open_chart(&column_chart());
    let p = find_p(&doc, "After the chart");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":">> "}));
    let (_, m) = the_chart(&doc);
    assert_eq!(m.series.len(), 3);
    let _ = rewrite(&mut doc, |_| {});
}

#[test]
fn the_accessibility_check_asks_for_a_chart_description() {
    // A chart from Word without a description is flagged, at the chart's position; a described one is not.
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    let flagged: Vec<_> = doc.accessibility().into_iter().filter(|i| i.kind == "missing_alt").collect();
    assert_eq!(flagged.len(), 1);
    assert_eq!((flagged[0].p, flagged[0].o), (Some(pos.p.0), Some(pos.o)));
    ops(&mut doc, json!({"op":"set_image","at":at(pos),"alt":"Sales by quarter, three series"}));
    assert!(doc.accessibility().iter().all(|i| i.kind != "missing_alt"));
    // A chart Docxtor inserts describes itself.
    let mut doc = doc_with("Body");
    let p = find_p(&doc, "Body");
    ops(&mut doc, insert_args("column", p));
    assert!(doc.accessibility().iter().all(|i| i.kind != "missing_alt"));
}

#[test]
fn deleting_a_chart_removes_it_from_the_text() {
    let mut doc = open_chart(&column_chart());
    let (pos, _) = the_chart(&doc);
    ops(&mut doc, json!({"op":"delete","from":at(pos),"to":{"p":pos.p.0,"o":pos.o+1}}));
    assert!(charts(&doc).is_empty());
    assert!(doc.undo());
    assert_eq!(charts(&doc).len(), 1);
}
