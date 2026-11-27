//! Charts: inserting one (a chart part, its workbook, relationships and content types) and changing one's
//! type, title and data. The chart parts live in the package; the package journal gives them back on undo.

use super::*;
use crate::chart::{self, xlsx, ChartEdit, ChartKind, ChartModel, ChartSeries, Grouping, Legend, Workbook};

const DEFAULT_WIDTH: f64 = 432.0;
const DEFAULT_HEIGHT: f64 = 252.0;

fn legend_arg(s: &str) -> Option<String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "none" | "off" | "hide" | "" => None,
        "l" | "left" => Some("l".into()),
        "t" | "top" => Some("t".into()),
        "b" | "bottom" => Some("b".into()),
        "tr" | "top_right" | "top-right" => Some("tr".into()),
        _ => Some("r".into()),
    }
}

impl Document {
    /// The chart at a position (the object occupying `[o, o+1)`), as the editor draws it.
    pub fn chart_at(&self, at: Pos) -> Option<ChartModel> {
        let (blocks, part) = self.locate(at.p.0);
        let p = find_para(blocks, at.p.0)?;
        let labels = &self.last_labels;
        let mut ctx = self.render_ctx(part, labels);
        ctx.para(p).segs.into_iter().find(|s| s.k == "chart" && s.o == at.o).and_then(|s| s.chart).map(|c| *c)
    }

    /// The charts in a paragraph: the offset of each and what it shows.
    pub fn charts_in(&self, p: NodeId) -> Vec<(usize, ChartModel)> {
        let (blocks, part) = self.locate(p);
        let Some(para) = find_para(blocks, p) else { return Vec::new() };
        let labels = &self.last_labels;
        let mut ctx = self.render_ctx(part, labels);
        ctx.para(para).segs.into_iter().filter(|s| s.k == "chart").filter_map(|s| s.chart.map(|c| (s.o, *c))).collect()
    }

    /// Call before a series of edits that add a chart in several steps, so one undo takes them all back. (The
    /// package journal already joins the steps; kept for callers.)
    pub fn begin_chart_edit(&mut self) {}

    /// The part name of the chart at a position.
    fn chart_part_at(&mut self, at: Pos) -> Result<String, EditError> {
        let active = self.active_part.clone();
        let p = self.para_ref(at.p.0)?;
        let rid = edit::object_at(&p.inlines, at.o).and_then(|e| chart::chart_rid(e)).map(String::from).ok_or_else(|| EditError::Invalid("there is no chart at that position".into()))?;
        self.pkg.resolve_rel(&active, &rid).map(|r| r.target).ok_or_else(|| EditError::Invalid("the chart's part is missing".into()))
    }

    pub(super) fn op_set_chart(&mut self, op: &Op) -> Result<OpResult, EditError> {
        let Op::SetChart { at, kind, grouping, title, categories, series, legend, data_labels, x_title, y_title, options } = op else { unreachable!() };
        let part = self.chart_part_at(*at)?;
        let bytes = self.pkg.get(&part).ok_or_else(|| EditError::Invalid("the chart's part is missing".into()))?.to_vec();
        let current = chart::read_part(&self.pkg, &part).ok_or_else(|| EditError::Invalid("Docxtor can't read this chart".into()))?;
        let mut doc = xml::parse(&bytes).map_err(|e| EditError::Invalid(e.to_string()))?;
        let wb = chart::workbook_of(&self.pkg, &part);
        let book_bytes = match &wb {
            Workbook::Embedded { part: wb_part, .. } => self.pkg.get(wb_part).map(|b| b.to_vec()),
            _ => None,
        };
        let edit = ChartEdit {
            kind: *kind,
            grouping: *grouping,
            title: title.clone(),
            categories: categories.clone(),
            series: series.clone(),
            legend: legend.as_deref().map(legend_arg),
            data_labels: *data_labels,
            x_title: x_title.clone(),
            y_title: y_title.clone(),
            options: options.clone(),
        };
        let out = chart::write::apply(&mut doc, &edit, &current, matches!(wb, Workbook::Linked), book_bytes.as_deref()).map_err(EditError::Invalid)?;
        if let Some(book) = out.workbook {
            match wb {
                Workbook::Embedded { part: wb_part, rid } => {
                    self.pkg.set(&wb_part, book);
                    chart::write::ensure_external_data(&mut doc, &rid);
                }
                _ => {
                    let name = chart::free_name(&self.pkg, "word/embeddings", "Microsoft_Excel_Worksheet", "xlsx");
                    let rid = self.pkg.add_rel(&part, chart::REL_PACKAGE, &format!("../embeddings/{}", name.rsplit('/').next().unwrap_or("")), false);
                    self.pkg.ensure_default("xlsx", chart::CT_XLSX);
                    self.pkg.set(&name, book);
                    chart::write::ensure_external_data(&mut doc, &rid);
                }
            }
        }
        if let Some(p) = out.palette {
            chart::write_colors_part(&mut self.pkg, &part, &p);
        }
        self.pkg.set(&part, doc.to_bytes());
        self.touch(at.p.0);
        Ok(OpResult { caret: Some(*at), ..Default::default() })
    }

    pub(super) fn op_insert_chart(&mut self, op: &Op) -> Result<OpResult, EditError> {
        let Op::InsertChart { at, kind, title, categories, series, grouping, legend, data_labels, width, height, alt, options } = op else { unreachable!() };
        if *kind == ChartKind::Other {
            return Err(EditError::Invalid("pick a chart type: column, bar, line, pie, doughnut, area, scatter, radar, bubble, stock or surface".into()));
        }
        let pie = matches!(kind, ChartKind::Pie | ChartKind::Doughnut);
        let xy = matches!(kind, ChartKind::Scatter | ChartKind::Bubble);
        let bubble = *kind == ChartKind::Bubble;
        let labels_on = data_labels.unwrap_or(false);
        let mut model = ChartModel {
            kind: *kind,
            grouping: grouping.filter(|_| kind.can_stack()).unwrap_or(Grouping::Clustered),
            title: title.clone().filter(|t| !t.is_empty()),
            categories: categories.clone(),
            data_labels: labels_on,
            labels_percent: pie && labels_on,
            series: series
                .iter()
                .map(|s| ChartSeries {
                    name: s.name.clone(),
                    values: s.values.clone(),
                    kind: s.kind.unwrap_or(*kind),
                    secondary: s.secondary.unwrap_or(false),
                    marker: matches!(s.kind.unwrap_or(*kind), ChartKind::Line | ChartKind::Scatter),
                    x_values: xy.then(|| s.x_values.clone().unwrap_or_else(|| (1..=s.values.len()).map(|n| Some(n as f64)).collect())),
                    sizes: bubble.then(|| s.sizes.clone().unwrap_or_else(|| vec![Some(1.0); s.values.len()])),
                    labels: labels_on.then(|| chart::DataLabels { value: !pie, percent: pie, ..Default::default() }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        for (i, s) in model.series.iter_mut().enumerate() {
            if s.name.trim().is_empty() {
                s.name = format!("Series {}", i + 1);
            }
        }
        chart::write::normalise(&mut model).map_err(EditError::Invalid)?;
        model.legend = match legend.as_deref() {
            Some(l) => legend_arg(l).map(|position| Legend { position }),
            None if pie || model.series.len() > 1 => Some(Legend { position: if pie { "r".into() } else { "b".into() } }),
            None => None,
        };
        if let Some(o) = options {
            model = chart::format::merge(&model, o).map_err(EditError::Invalid)?;
        }
        chart::write::check_stock(&model).map_err(EditError::Invalid)?;
        let (plan, sizes) = if bubble { chart::xlsx::plan_bubble(&model.series) } else { (xlsx::plan(&model.categories, &model.series, xy), vec![None; model.series.len()]) };
        for (s, z) in model.series.iter_mut().zip(sizes) {
            s.size_ref = z;
        }

        // Parts: the chart, its workbook, and the relationships between them.
        let chart_part = chart::free_name(&self.pkg, "word/charts", "chart", "xml");
        let book_part = chart::free_name(&self.pkg, "word/embeddings", "Microsoft_Excel_Worksheet", "xlsx");
        let book_rid = self.pkg.add_rel(&chart_part, chart::REL_PACKAGE, &format!("../embeddings/{}", book_part.rsplit('/').next().unwrap_or("")), false);
        let mut space = chart::build::chart_space(&model, &plan.refs, Some(&book_rid));
        let palette = chart::write::finish_new(&mut space, options.as_ref()).map_err(EditError::Invalid)?;
        self.pkg.set(&chart_part, space.to_bytes());
        self.pkg.set(&book_part, xlsx::build(&plan));
        self.pkg.ensure_override(&chart_part, chart::CT_CHART);
        self.pkg.ensure_default("xlsx", chart::CT_XLSX);
        if let Some(p) = &palette {
            chart::write_colors_part(&mut self.pkg, &chart_part, p);
        }

        let main = self.active_part.clone();
        let rid = self.pkg.add_rel(&main, chart::REL_CHART, &relative_target(&main, &chart_part), false);
        self.ensure_ns("r", NS_R);
        self.ensure_ns("wp", NS_WP);
        self.drawing_id += 1;
        let descr = alt.clone().unwrap_or_else(|| model.summary());
        let el = chart_drawing_xml(&rid, width.unwrap_or(DEFAULT_WIDTH), height.unwrap_or(DEFAULT_HEIGHT), self.drawing_id, &descr);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }
}

/// A chart in an inline drawing, the way Word writes one.
fn chart_drawing_xml(rid: &str, w_pt: f64, h_pt: f64, id: u32, alt: &str) -> Element {
    let cx = (w_pt.clamp(36.0, 1584.0) * 12700.0).round() as i64;
    let cy = (h_pt.clamp(36.0, 1584.0) * 12700.0).round() as i64;
    let alt = chart::build::esc(alt);
    let src = format!(
        r#"<w:drawing xmlns:w="{NS_W}" xmlns:wp="{NS_WP}" xmlns:r="{NS_R}"><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Chart {id}" descr="{alt}"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="{uri}"><c:chart xmlns:c="{ns_c}" xmlns:r="{NS_R}" r:id="{rid}"/></a:graphicData></a:graphic></wp:inline></w:drawing>"#,
        uri = chart::URI_CHART,
        ns_c = chart::build::NS_C,
    );
    let mut el = xml::parse(src.as_bytes()).expect("valid drawing xml").root;
    el.attrs.retain(|(k, _)| !k.starts_with("xmlns:"));
    el
}
