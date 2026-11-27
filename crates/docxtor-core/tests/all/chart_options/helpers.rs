//! Tests of: Helpers.

use super::*;

/// Every XML part must be namespace-well-formed, every relationship must point at a part that exists, and every
/// part must have a content type.
pub(super) fn assert_valid(bytes: &[u8]) {
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
        if p.name.starts_with("word/charts/") && p.name.ends_with(".xml") && !p.name.contains("_rels") {
            assert!(ct.root.elements().any(|e| e.local() == "Override" && e.attr("PartName") == Some(format!("/{}", p.name).as_str())), "{} has no content type", p.name);
            if !p.name.contains("colors") && !p.name.contains("style") {
                check_order(&p.name, &p.data);
            }
        }
    }
}

const CHART_ORDER: &[&str] = &["c:title", "c:autoTitleDeleted", "c:pivotFmts", "c:view3D", "c:floor", "c:sideWall", "c:backWall", "c:plotArea", "c:legend", "c:plotVisOnly", "c:dispBlanksAs", "c:showDLblsOverMax", "c:extLst"];
const SPACE_ORDER: &[&str] = &["c:date1904", "c:lang", "c:roundedCorners", "mc:AlternateContent", "c:style", "c:clrMapOvr", "c:pivotSource", "c:protection", "c:chart", "c:spPr", "c:txPr", "c:externalData", "c:printSettings", "c:userShapes", "c:extLst"];
const LEGEND_ORDER: &[&str] = &["c:legendPos", "c:legendEntry", "c:layout", "c:overlay", "c:spPr", "c:txPr", "c:extLst"];
const SCALING_ORDER: &[&str] = &["c:logBase", "c:orientation", "c:max", "c:min", "c:extLst"];
const TITLE_ORDER: &[&str] = &["c:tx", "c:layout", "c:overlay", "c:spPr", "c:txPr", "c:extLst"];
const PLOT_ORDER: &[&str] = &["c:layout", "GROUP", "AXIS", "c:dTable", "c:spPr", "c:extLst"];

pub(super) fn order_of(e: &Element) -> Option<&'static [&'static str]> {
    Some(match e.name.as_str() {
        "c:ser" => SER_ORDER,
        "c:catAx" | "c:valAx" | "c:dateAx" | "c:serAx" => AX_ORDER,
        "c:dLbls" => DLBLS_ORDER,
        "c:trendline" => TREND_ORDER,
        "c:errBars" => ERR_ORDER,
        "c:barChart" | "c:lineChart" | "c:areaChart" | "c:scatterChart" | "c:bubbleChart" | "c:stockChart" | "c:pieChart" | "c:doughnutChart" | "c:radarChart" | "c:surfaceChart" | "c:surface3DChart" => GROUP_ORDER,
        "c:legend" => LEGEND_ORDER,
        "c:chart" => CHART_ORDER,
        "c:chartSpace" => SPACE_ORDER,
        "c:scaling" => SCALING_ORDER,
        "c:title" => TITLE_ORDER,
        "c:spPr" => SPPR_ORDER,
        "c:plotArea" => PLOT_ORDER,
        _ => return None,
    })
}

pub(super) fn check_order(name: &str, data: &[u8]) {
    let doc = docxtor_core::xml::parse(data).unwrap();
    fn walk(name: &str, e: &Element) {
        if let Some(order) = order_of(e) {
            let rank = |n: &str| -> Option<usize> {
                if e.name == "c:plotArea" {
                    return match n {
                        "c:layout" => Some(0),
                        n if n.ends_with("Chart") || n.ends_with("3DChart") => Some(1),
                        "c:catAx" | "c:valAx" | "c:dateAx" | "c:serAx" => Some(2),
                        "c:dTable" => Some(3),
                        "c:spPr" => Some(4),
                        "c:extLst" => Some(5),
                        _ => None,
                    };
                }
                order.iter().position(|o| *o == n)
            };
            let mut last = 0;
            for c in e.elements() {
                if let Some(r) = rank(&c.name) {
                    assert!(r >= last, "{name}: <{}> is out of order inside <{}> (children: {:?})", c.name, e.name, e.elements().map(|c| c.name.as_str()).collect::<Vec<_>>());
                    last = r;
                }
            }
        }
        for c in e.elements() {
            walk(name, c);
        }
    }
    walk(name, &doc.root);
}

pub(super) fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

pub(super) fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

pub(super) fn saved(doc: &mut Document) -> Package {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    Package::open(&bytes).unwrap()
}

pub(super) fn text(pkg: &Package, name: &str) -> String {
    String::from_utf8(pkg.get(name).unwrap_or_else(|| panic!("no part {name}")).to_vec()).unwrap()
}

pub(super) fn chart_xml(doc: &mut Document) -> String {
    text(&saved(doc), "word/charts/chart1.xml")
}

pub(super) fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

pub(super) fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

pub(super) fn the_chart(doc: &Document) -> (Pos, ChartModel) {
    let mut ids = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| ids.push(p.id));
    let mut all: Vec<(u32, usize, ChartModel)> = ids.into_iter().flat_map(|p| doc.charts_in(p).into_iter().map(move |(o, m)| (p, o, m))).collect();
    assert_eq!(all.len(), 1, "expected one chart");
    let (p, o, m) = all.remove(0);
    (Pos::new(p, o), m)
}

pub(super) fn at(pos: Pos) -> Value {
    json!({"p": pos.p.0, "o": pos.o})
}

/// A document with one chart of `kind` and the usual two series over three categories; `extra` is merged into the op.
pub(super) fn chart_doc(kind: &str, extra: Value) -> (Document, Pos) {
    let mut doc = doc_with("Intro\n\nBody");
    let p = find_p(&doc, "Body");
    let mut op = json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":kind,"title":"Sample","categories":["A","B","C"],"series":[{"name":"One","values":[1,2,3]},{"name":"Two","values":[3,2,1]}]});
    if kind == "scatter" {
        op["series"] = json!([{"name":"One","values":[1,4,9],"x_values":[1,2,3]},{"name":"Two","values":[2,3,5],"x_values":[1,2,3]}]);
        op["categories"] = json!([]);
    }
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        op[k] = v;
    }
    ops(&mut doc, op);
    let (pos, _) = the_chart(&doc);
    (doc, pos)
}

pub(super) fn set(doc: &mut Document, pos: Pos, extra: Value) {
    let mut op = json!({"op":"set_chart","at":at(pos)});
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        op[k] = v;
    }
    ops(doc, op);
}

pub(super) fn opts(doc: &mut Document, pos: Pos, options: Value) {
    set(doc, pos, json!({"options": options}));
}

pub(super) fn count(xml: &str, needle: &str) -> usize {
    xml.matches(needle).count()
}
