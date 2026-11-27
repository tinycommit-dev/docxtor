//! Changing an existing chart part: its type, title, data, legend, labels and, through `options`, its formatting.
//! Only what the edit names is rewritten; every other element of the part (colours, fonts, layout) stays as it is.
//!
//! An edit runs in two steps. First the classic parameters (type, data, title, legend, labels, axis titles) are
//! applied. Then `options` (see `format`) is merged into the chart as it is by then, the plot groups are
//! rebuilt or regrouped if the options change what is drawn (stock and surface types, a series' own type or axis),
//! and `format::write` edits the elements that still differ from the options.

use super::build::{self, frag};
use super::format::{self, put_in, SER_ORDER};
use super::model::*;
use super::read::{group_kind, series_order};
use super::xlsx::{self, Plan, SeriesRefs};
use super::xlsx_patch::{self, SheetEdit};
use crate::xml::{Element, Node, XmlDoc};

/// Numbers as people and models write them: numbers, numeric strings (`"1,200"`, `"3.5"`) and null for a gap.
fn lenient_numbers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Option<f64>>, D::Error> {
    use serde::Deserialize;
    let v = Vec::<serde_json::Value>::deserialize(d)?;
    v.into_iter()
        .map(|x| match x {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::Number(n) => n.as_f64().filter(|f| f.is_finite()).map(Some).ok_or_else(|| serde::de::Error::custom("chart values must be finite numbers")),
            serde_json::Value::String(s) if s.trim().is_empty() => Ok(None),
            serde_json::Value::String(s) => s.trim().replace([',', ' '], "").parse::<f64>().ok().filter(|f| f.is_finite()).map(Some).ok_or_else(|| serde::de::Error::custom(format!("\"{s}\" is not a number"))),
            other => Err(serde::de::Error::custom(format!("chart values must be numbers, got {other}"))),
        })
        .collect()
}

fn lenient_opt_numbers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Vec<Option<f64>>>, D::Error> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct W(#[serde(deserialize_with = "lenient_numbers")] Vec<Option<f64>>);
    Ok(Option::<W>::deserialize(d)?.map(|w| w.0))
}

/// A series as given to an edit: name and numbers (X values for scatter and bubble charts, sizes for bubble
/// charts), and optionally its own type and axis in a combination chart.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SeriesIn {
    #[serde(default)]
    pub name: String,
    #[serde(deserialize_with = "lenient_numbers")]
    pub values: Vec<Option<f64>>,
    #[serde(default, alias = "xValues", deserialize_with = "lenient_opt_numbers", skip_serializing_if = "Option::is_none")]
    pub x_values: Option<Vec<Option<f64>>>,
    #[serde(default, deserialize_with = "lenient_opt_numbers", skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<Option<f64>>>,
    /// This series' own type (a combination chart).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ChartKind>,
    /// Drawn against the second value axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct ChartEdit {
    pub kind: Option<ChartKind>,
    pub grouping: Option<Grouping>,
    /// `Some("")` removes the title.
    pub title: Option<String>,
    pub categories: Option<Vec<String>>,
    pub series: Option<Vec<SeriesIn>>,
    /// `Some(None)` removes the legend.
    pub legend: Option<Option<String>>,
    pub data_labels: Option<bool>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    /// Formatting, as a JSON object (see `format::merge`).
    pub options: Option<serde_json::Value>,
}

/// What an edit asks of the package around the chart part.
#[derive(Debug, Default)]
pub struct Outcome {
    /// New bytes for the embedded workbook (the data changed and the workbook is ours to change).
    pub workbook: Option<Vec<u8>>,
    /// The colour palette to write as the chart's `colorsN.xml`.
    pub palette: Option<String>,
}

pub const MAX_SERIES: usize = 64;
pub const MAX_POINTS: usize = 2000;

fn insert_before_any(parent: &mut Element, new: Element, names: &[&str]) {
    let pos = parent.children.iter().position(|n| matches!(n, Node::Element(e) if names.contains(&e.name.as_str())));
    match pos {
        Some(i) => parent.children.insert(i, Node::Element(new)),
        None => parent.children.push(Node::Element(new)),
    }
}

/// Replace the first child named like `new`, else insert it before the first of `after`.
fn put(parent: &mut Element, new: Element, after: &[&str]) {
    let pos = parent.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == new.name));
    match pos {
        Some(i) => parent.children[i] = Node::Element(new),
        None => insert_before_any(parent, new, after),
    }
}

fn set_val(parent: &mut Element, name: &'static str, v: &str, after: &[&str]) {
    match parent.child_mut(name) {
        Some(e) => e.set_attr("val", v),
        None => insert_before_any(parent, Element::new(name).with_attr("val", v), after),
    }
}

const GROUP_AFTER_DLBLS: &[&str] = &["c:dropLines", "c:hiLowLines", "c:upDownBars", "c:marker", "c:smooth", "c:gapWidth", "c:overlap", "c:serLines", "c:firstSliceAng", "c:holeSize", "c:bubbleScale", "c:axId", "c:extLst"];
const CHART_AFTER_LEGEND: &[&str] = &["c:plotVisOnly", "c:dispBlanksAs", "c:showDLblsOverMax", "c:extLst"];

fn is_group(name: &str) -> bool {
    matches!(name, "c:barChart" | "c:bar3DChart" | "c:lineChart" | "c:line3DChart" | "c:areaChart" | "c:area3DChart" | "c:pieChart" | "c:pie3DChart" | "c:doughnutChart" | "c:scatterChart" | "c:radarChart" | "c:bubbleChart" | "c:stockChart" | "c:surfaceChart" | "c:surface3DChart" | "c:ofPieChart")
}

fn is_axis(name: &str) -> bool {
    matches!(name, "c:catAx" | "c:valAx" | "c:dateAx" | "c:serAx")
}

fn xy(k: ChartKind) -> bool {
    matches!(k, ChartKind::Scatter | ChartKind::Bubble)
}

/// Axis families: category + value axes, none, two value axes, or a surface's three.
fn family(k: ChartKind) -> u8 {
    match k {
        ChartKind::Pie | ChartKind::Doughnut | ChartKind::Other => 1,
        ChartKind::Scatter | ChartKind::Bubble => 2,
        ChartKind::Surface => 3,
        _ => 0,
    }
}

fn existing_refs(s: &ChartSeries) -> Option<SeriesRefs> {
    match (&s.name_ref, &s.cat_ref, &s.val_ref) {
        (Some(n), Some(c), Some(v)) => Some(SeriesRefs { name: n.clone(), cat: c.clone(), val: v.clone() }),
        _ => None,
    }
}

/// The formulas to write for each series: the ones it has, else the layout of a fresh workbook.
fn refs_of(m: &ChartModel, plan: &Plan) -> Vec<SeriesRefs> {
    m.series.iter().enumerate().map(|(i, s)| existing_refs(s).unwrap_or_else(|| plan.refs[i].clone())).collect()
}

fn plan_for(m: &ChartModel) -> (Plan, Vec<Option<String>>) {
    if m.kind == ChartKind::Bubble {
        xlsx::plan_bubble(&m.series)
    } else {
        (xlsx::plan(&m.categories, &m.series, m.kind == ChartKind::Scatter), vec![None; m.series.len()])
    }
}

/// Apply `edit` to a chart part. `keep_refs` leaves the formulas alone (a linked workbook), updating only the
/// cached values; `workbook` is the embedded workbook, when there is one, so its cells can be edited in place.
pub fn apply(doc: &mut XmlDoc, edit: &ChartEdit, current: &ChartModel, keep_refs: bool, workbook: Option<&[u8]>) -> Result<Outcome, String> {
    let mut out = apply_classic(doc, edit, current, keep_refs, workbook)?;
    if let Some(opts) = &edit.options {
        out.palette = apply_options(doc, opts, current, edit.series.is_some() || edit.kind.is_some())?;
    }
    Ok(out)
}

fn apply_classic(doc: &mut XmlDoc, edit: &ChartEdit, current: &ChartModel, keep_refs: bool, workbook: Option<&[u8]>) -> Result<Outcome, String> {
    if doc.root.local() != "chartSpace" {
        return Err("that isn't a chart".into());
    }
    let mut target = current.clone();
    if let Some(k) = edit.kind {
        if k == ChartKind::Other {
            return Err("pick a chart type Docxtor can draw: column, bar, line, pie, doughnut, area, scatter, radar, bubble, stock or surface".into());
        }
        target.kind = k;
        target.combo = false;
        for s in &mut target.series {
            s.kind = k;
            s.secondary = false;
        }
    }
    if current.kind == ChartKind::Other && edit.kind.is_none() && (edit.series.is_some() || edit.categories.is_some() || edit.grouping.is_some()) {
        return Err("Docxtor can't change the data of this kind of chart yet. Pick a chart type first.".into());
    }
    if let Some(g) = edit.grouping {
        target.grouping = g;
    }
    if let Some(c) = &edit.categories {
        target.categories = c.clone();
    }
    if let Some(list) = &edit.series {
        if list.is_empty() {
            return Err("a chart needs at least one series".into());
        }
        if list.len() > MAX_SERIES {
            return Err(format!("a chart can have at most {MAX_SERIES} series"));
        }
        let mut next = Vec::with_capacity(list.len());
        for (i, s) in list.iter().enumerate() {
            let mut base = match current.series.get(i) {
                Some(b) => b.clone(),
                // A new series looks like the one before it, without what would repeat: colours, lines, ranges.
                None => {
                    let mut b = current.series.last().cloned().unwrap_or_else(|| ChartSeries { kind: target.kind, marker: matches!(target.kind, ChartKind::Line | ChartKind::Scatter), ..Default::default() });
                    b.color = None;
                    b.point_colors.clear();
                    b.trendlines.clear();
                    b.error_bars.clear();
                    b.name_ref = None;
                    b.cat_ref = None;
                    b.val_ref = None;
                    b.size_ref = None;
                    b
                }
            };
            base.kind = match (s.kind, edit.kind) {
                (Some(k), _) => k,
                (None, Some(_)) => target.kind,
                (None, None) => {
                    if current.combo {
                        base.kind
                    } else {
                        target.kind
                    }
                }
            };
            base.secondary = s.secondary.unwrap_or(if edit.kind.is_some() { false } else { base.secondary });
            base.name = s.name.clone();
            base.values = s.values.clone();
            if xy(target.kind) {
                base.x_values = Some(s.x_values.clone().unwrap_or_else(|| (1..=s.values.len()).map(|n| Some(n as f64)).collect()));
            }
            if target.kind == ChartKind::Bubble {
                base.sizes = Some(s.sizes.clone().or_else(|| base.sizes.clone()).unwrap_or_else(|| vec![Some(1.0); s.values.len()]));
            }
            next.push(base);
        }
        target.series = next;
    }
    let target_xy = xy(target.kind);
    // Moving between scatter-like charts and the other kinds changes what the first column means.
    if target_xy && !xy(current.kind) {
        for s in &mut target.series {
            s.x_values.get_or_insert_with(|| (1..=s.values.len()).map(|n| Some(n as f64)).collect());
            if target.kind == ChartKind::Bubble {
                s.sizes.get_or_insert_with(|| vec![Some(1.0); s.values.len()]);
            }
        }
    } else if !target_xy && xy(current.kind) && target.categories.is_empty() {
        let xs = target.series.first().and_then(|s| s.x_values.clone()).unwrap_or_default();
        target.categories = xs.iter().enumerate().map(|(i, x)| x.map(|x| x.to_string()).unwrap_or_else(|| (i + 1).to_string())).collect();
    }
    if target.kind == ChartKind::Bubble && current.kind == ChartKind::Scatter {
        for s in &mut target.series {
            s.sizes.get_or_insert_with(|| vec![Some(1.0); s.values.len()]);
        }
    }
    if edit.series.is_some() || edit.categories.is_some() || target_xy != xy(current.kind) {
        normalise(&mut target)?;
    }
    let data_changed = target.categories != current.categories
        || target.series.len() != current.series.len()
        || target.series.iter().zip(&current.series).any(|(a, b)| a.name != b.name || a.values != b.values || a.x_values != b.x_values || a.sizes != b.sizes);
    let kind_changed = target.kind != current.kind || (current.combo && edit.kind.is_some());

    // The workbook: edit its cells in place, else write a fresh one.
    let mut out = Outcome::default();
    let (plan, plan_sizes) = plan_for(&target);
    let mut refs: Vec<SeriesRefs> = refs_of(&target, &plan);
    if data_changed && !keep_refs {
        let old: Vec<Option<SeriesRefs>> = (0..target.series.len()).map(|i| current.series.get(i).and_then(existing_refs)).collect();
        let old_sizes: Vec<Option<String>> = (0..target.series.len()).map(|i| current.series.get(i).and_then(|s| s.size_ref.clone())).collect();
        let patched = workbook.and_then(|book| xlsx_patch::patch(book, &SheetEdit { categories: &target.categories, series: &target.series, scatter: target_xy, old_refs: &old, old_size_refs: &old_sizes }));
        match patched {
            Some(p) => {
                refs = p.refs;
                for (s, z) in target.series.iter_mut().zip(p.size_refs) {
                    s.size_ref = z;
                }
                out.workbook = Some(p.bytes);
            }
            None => {
                refs = plan.refs.clone();
                for (s, z) in target.series.iter_mut().zip(plan_sizes) {
                    s.size_ref = z;
                }
                out.workbook = Some(xlsx::build(&plan));
            }
        }
    } else if kind_changed || data_changed {
        for (s, z) in target.series.iter_mut().zip(&plan_sizes) {
            if s.size_ref.is_none() {
                s.size_ref = z.clone();
            }
        }
    }

    let chart = doc.root.child_mut("c:chart").ok_or("the chart has no c:chart")?;

    // ---- plot area -------------------------------------------------------------------------
    if kind_changed {
        rebuild(chart, &mut target, current, &refs)?;
    } else {
        let plot = chart.child_mut("c:plotArea").ok_or("the chart has no plot area")?;
        if edit.grouping.is_some() && target.grouping != current.grouping {
            regroup(plot, &target);
        }
        if data_changed {
            update_series(plot, &target, &refs, keep_refs)?;
        }
    }
    // Series that came with a type or axis of their own go to the group that has it.
    if !kind_changed && data_changed {
        let plot_now = chart.child_mut("c:plotArea").ok_or("the chart has no plot area")?;
        let kinds_now: Vec<(ChartKind, bool)> = {
            let order = series_order(plot_now);
            order
                .iter()
                .filter_map(|&(gi, _)| match &plot_now.children[gi] {
                    Node::Element(g) => group_kind(g),
                    _ => None,
                })
                .zip(series_secondary(plot_now))
                .collect()
        };
        if kinds_now.len() == target.series.len() && kinds_now != series_layout(&target) {
            for (i, s) in target.series.iter_mut().enumerate() {
                if matches!(s.kind, ChartKind::Line | ChartKind::Scatter) && !matches!(kinds_now[i].0, ChartKind::Line | ChartKind::Scatter) {
                    s.marker = true;
                }
            }
            regroup_series(plot_now, &target, &refs)?;
        }
    }
    {
        let plot = chart.child_mut("c:plotArea").ok_or("the chart has no plot area")?;
        if let Some(on) = edit.data_labels {
            set_data_labels(plot, on, &target);
        }
        if let Some(t) = &edit.x_title {
            set_axis_title(plot, true, target.kind, t);
        }
        if let Some(t) = &edit.y_title {
            set_axis_title(plot, false, target.kind, t);
        }
    }

    // ---- title, legend ----------------------------------------------------------------------
    if let Some(t) = &edit.title {
        set_title(chart, t);
    }
    if let Some(l) = &edit.legend {
        match l {
            None => {
                chart.remove_children("c:legend");
            }
            Some(pos) => {
                let pos = match pos.as_str() {
                    "l" | "left" => "l",
                    "t" | "top" => "t",
                    "b" | "bottom" => "b",
                    "tr" | "top_right" => "tr",
                    _ => "r",
                };
                if let Some(leg) = chart.child_mut("c:legend") {
                    set_val(leg, "c:legendPos", pos, &["c:legendEntry", "c:layout", "c:overlay", "c:spPr", "c:txPr"]);
                } else {
                    insert_before_any(chart, frag(&build::legend_xml(pos)), CHART_AFTER_LEGEND);
                }
            }
        }
    }
    Ok(out)
}

/// Whether each series of a plot area (in the chart's order) sits on axes of its own.
fn series_secondary(plot: &Element) -> Vec<bool> {
    let first: Vec<&str> = plot.elements().find(|g| group_kind(g).is_some()).map(|g| g.children_named("c:axId").filter_map(|a| a.attr("val")).collect()).unwrap_or_default();
    series_order(plot)
        .into_iter()
        .map(|(gi, _)| match &plot.children[gi] {
            Node::Element(g) => {
                let ids: Vec<&str> = g.children_named("c:axId").filter_map(|a| a.attr("val")).collect();
                !ids.is_empty() && !first.is_empty() && !ids.iter().any(|a| first.contains(a))
            }
            _ => false,
        })
        .collect()
}

fn series_layout(m: &ChartModel) -> Vec<(ChartKind, bool)> {
    m.series.iter().map(|s| (s.kind, s.secondary)).collect()
}

/// The `options` step of an edit: merge them into the chart as it is now, change the plot groups if the options
/// ask for a different layout, and write the rest. Returns the palette to write as `colorsN.xml`, if it changed.
fn apply_options(doc: &mut XmlDoc, opts: &serde_json::Value, before: &ChartModel, data_edit: bool) -> Result<Option<String>, String> {
    let theme: Vec<(String, String)> = before.theme.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let now = super::read::read(&doc.to_bytes(), &theme).ok_or("Docxtor can't read this chart")?;
    let mut target = format::merge(&now, opts)?;
    if data_edit || target.stock != now.stock {
        check_stock(&target)?;
    }
    let refs: Vec<SeriesRefs> = {
        let (plan, _) = plan_for(&target);
        refs_of(&target, &plan)
    };
    let chart = doc.root.child_mut("c:chart").ok_or("the chart has no c:chart")?;
    let variant_changed = target.stock != now.stock || target.surface != now.surface;
    if variant_changed {
        rebuild(chart, &mut target, &now, &refs)?;
    } else if series_layout(&target) != series_layout(&now) {
        // A series that becomes a line has markers, as a new line series in Word does.
        for (t, n) in target.series.iter_mut().zip(&now.series) {
            if matches!(t.kind, ChartKind::Line | ChartKind::Scatter) && !matches!(n.kind, ChartKind::Line | ChartKind::Scatter) {
                t.marker = true;
            }
        }
        let plot = chart.child_mut("c:plotArea").ok_or("the chart has no plot area")?;
        regroup_series(plot, &target, &refs)?;
    }
    let after = super::read::read(&doc.to_bytes(), &theme).ok_or("Docxtor can't read this chart")?;
    // A series of another type starts with that type's lines and fills.
    for (t, (n, old)) in target.series.iter_mut().zip(after.series.iter().zip(&now.series)) {
        if n.kind != old.kind {
            t.line_width = n.line_width;
            t.fill = n.fill.clone();
            t.line_color = n.line_color.clone();
        }
    }
    format::write(doc, &target, &after)?;
    Ok((target.palette != before.palette).then(|| target.palette.clone()).flatten())
}

/// A stock chart needs 3 to 5 series, as many as its type has roles.
pub fn check_stock(m: &ChartModel) -> Result<(), String> {
    if m.kind != ChartKind::Stock {
        return Ok(());
    }
    let variant = m.stock.as_deref().unwrap_or("hlc");
    let want = match variant {
        "hlc" => 3,
        "vhlc" | "ohlc" => 4,
        _ => 5,
    };
    if m.series.len() != want {
        return Err(format!("a {variant} stock chart needs {want} series ({}), this one has {}", stock_roles(variant), m.series.len()));
    }
    Ok(())
}

/// Apply `options` to a chart part that was just built, writing what `build::chart_space` could not say (axes, labels,
/// trendlines ...). Returns the palette to write as `colorsN.xml`.
pub fn finish_new(doc: &mut XmlDoc, options: Option<&serde_json::Value>) -> Result<Option<String>, String> {
    let Some(opts) = options else { return Ok(None) };
    let now = super::read::read(&doc.to_bytes(), &[]).ok_or("Docxtor can't read the chart it made")?;
    let target = format::merge(&now, opts)?;
    format::write(doc, &target, &now)?;
    Ok(target.palette)
}

/// What each series of a stock chart is, in order.
pub fn stock_roles(variant: &str) -> &'static str {
    match variant {
        "ohlc" => "open, high, low, close",
        "vhlc" => "volume, high, low, close",
        "vohlc" => "volume, open, high, low, close",
        _ => "high, low, close",
    }
}

// ---------------------------------------------------------------------------
// Type changes
// ---------------------------------------------------------------------------

fn axis_id_values(plot: &Element) -> Vec<u32> {
    plot.elements().filter(|e| is_group(&e.name)).flat_map(|g| g.children_named("c:axId").filter_map(|a| a.attr("val")?.parse::<u32>().ok()).collect::<Vec<_>>()).chain(plot.elements().filter(|e| is_axis(&e.name)).filter_map(|a| a.child("c:axId")?.attr("val")?.parse::<u32>().ok())).collect()
}

fn rebuild(chart: &mut Element, target: &mut ChartModel, current: &ChartModel, refs: &[SeriesRefs]) -> Result<(), String> {
    let old_family = family(current.kind);
    let new_family = family(target.kind);
    let plot = chart.child_mut("c:plotArea").ok_or("the chart has no plot area")?;
    let first_ids: Vec<u32> = plot.elements().find(|e| is_group(&e.name)).map(|g| g.children_named("c:axId").filter_map(|a| a.attr("val")?.parse().ok()).collect()).unwrap_or_default();
    let ax = if first_ids.len() >= 2 { (first_ids[0], first_ids[1]) } else { build::AX_IDS };
    let max_id = axis_id_values(plot).into_iter().chain([ax.0, ax.1]).max().unwrap_or(ax.1);
    let ax2 = (max_id + 1, max_id + 2);
    // Keep what the old series looked like where the new kind can use it.
    for s in &mut target.series {
        s.point_colors.clear();
        if matches!(target.kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) && !s.marker {
            s.marker = true;
        }
    }
    let specs = build::specs(target);
    let mut groups = Vec::new();
    for sp in &specs {
        let g = frag(&build::group_xml(target, sp.kind, &sp.series, refs, if sp.secondary { ax2 } else { ax }));
        if g.name == "c:chart" {
            return Err("that chart type can't be drawn".into());
        }
        groups.push(g);
    }
    let main_kind = specs.iter().find(|s| !s.secondary).map(|s| s.kind).unwrap_or(target.kind);
    let main_kind = if main_kind == ChartKind::Stock { ChartKind::Column } else { main_kind };
    let need_second = specs.iter().any(|s| s.secondary);
    let layout = if old_family == new_family { plot.child("c:layout").cloned() } else { None };
    // Only the axes the new groups use: a combination chart's second pair goes with its second group.
    let old_axes: Vec<Element> = plot.elements().filter(|e| is_axis(&e.name) && e.child("c:axId").and_then(|a| a.attr("val")).and_then(|v| v.parse::<u32>().ok()).is_some_and(|id| id == ax.0 || id == ax.1)).cloned().collect();
    let mut axes: Vec<Element> = Vec::new();
    if old_family == new_family && old_family == 0 && !old_axes.is_empty() {
        axes = old_axes;
        adjust_axes(&mut axes, target);
    } else if new_family != 1 {
        let xml = build::axes_xml(target, main_kind, ax);
        axes = frag(&format!("<c:x>{xml}</c:x>")).elements().cloned().collect();
    }
    if need_second && new_family != 1 {
        axes.extend(frag(&format!("<c:x>{}</c:x>", build::secondary_axes_xml(ax2))).elements().cloned());
    }
    let rest: Vec<Node> = plot.children.iter().filter(|n| !matches!(n, Node::Element(e) if is_group(&e.name) || is_axis(&e.name) || e.name == "c:layout")).cloned().collect();
    plot.children.clear();
    if let Some(l) = layout {
        plot.children.push(Node::Element(l));
    } else if new_family != old_family {
        plot.children.push(Node::Element(Element::new("c:layout")));
    }
    plot.children.extend(groups.into_iter().map(Node::Element));
    plot.children.extend(axes.into_iter().map(Node::Element));
    plot.children.extend(rest);
    // A surface chart's view, floor and walls go with it.
    if current.kind == ChartKind::Surface || target.kind == ChartKind::Surface {
        for n in ["c:view3D", "c:floor", "c:sideWall", "c:backWall"] {
            chart.remove_children(n);
        }
    }
    if target.kind == ChartKind::Surface {
        let at = chart.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "c:plotArea")).unwrap_or(chart.children.len());
        let els: Vec<Element> = frag(&format!("<c:x>{}</c:x>", build::view3d_xml(target))).elements().cloned().collect();
        for (k, e) in els.into_iter().enumerate() {
            chart.children.insert(at + k, Node::Element(e));
        }
    }
    Ok(())
}

/// Move series between groups after their types or axes changed: groups are kept, added and dropped as needed, the
/// second pair of axes comes and goes with the series that use it, and a series whose type changed is written afresh
/// (the formatting the model knows is written again afterwards).
fn regroup_series(plot: &mut Element, target: &ChartModel, refs: &[SeriesRefs]) -> Result<(), String> {
    let order = series_order(plot);
    let mut taken: Vec<(Element, ChartKind)> = Vec::new();
    for &(gi, si) in &order {
        let Node::Element(g) = &plot.children[gi] else { continue };
        let kind = group_kind(g).unwrap_or(ChartKind::Column);
        let Node::Element(s) = &g.children[si] else { continue };
        taken.push((s.clone(), kind));
    }
    if taken.len() != target.series.len() {
        return Err("the chart's series don't match its data".into());
    }
    let first_ids: Vec<String> = plot.elements().find(|e| is_group(&e.name)).map(|g| g.children_named("c:axId").filter_map(|a| a.attr("val").map(String::from)).collect()).unwrap_or_default();
    let old_groups: Vec<(ChartKind, Vec<String>, Element)> = plot
        .elements()
        .filter(|e| is_group(&e.name))
        .map(|g| {
            let mut c = g.clone();
            c.remove_children("c:ser");
            (group_kind(g).unwrap_or(ChartKind::Column), g.children_named("c:axId").filter_map(|a| a.attr("val").map(String::from)).collect(), c)
        })
        .collect();
    let second_old: Option<Vec<String>> = old_groups.iter().find(|(_, ids, _)| !ids.is_empty() && !ids.iter().any(|a| first_ids.contains(a))).map(|(_, ids, _)| ids.clone());
    let max_id = axis_id_values(plot).into_iter().max().unwrap_or(0);
    let specs = build::specs(target);
    let need_second = specs.iter().any(|s| s.secondary);
    let second: Vec<String> = second_old.clone().unwrap_or_else(|| vec![(max_id + 1).to_string(), (max_id + 2).to_string()]);
    let primary: Vec<String> = if first_ids.len() >= 2 { first_ids[..2].to_vec() } else { vec![build::AX_IDS.0.to_string(), build::AX_IDS.1.to_string()] };
    let mut new_groups: Vec<Element> = Vec::new();
    for sp in &specs {
        let ids: &Vec<String> = if sp.secondary { &second } else { &primary };
        let reuse = old_groups.iter().find(|(k, gids, _)| *k == sp.kind && gids.iter().any(|a| ids.contains(a))).map(|x| x.2.clone());
        let mut g = match reuse {
            Some(g) => g,
            None => {
                let pair = (ids[0].parse().unwrap_or(1), ids.get(1).and_then(|v| v.parse().ok()).unwrap_or(2));
                frag(&build::group_xml(target, sp.kind, &[], refs, pair))
            }
        };
        for &i in &sp.series {
            let (el, from) = &taken[i];
            let ser = if *from == sp.kind { el.clone() } else { frag(&build::ser_xml(sp.kind, i, &target.series[i], &refs[i], &target.categories)) };
            let mut ser = ser;
            set_val(&mut ser, "c:idx", &i.to_string(), &["c:order"]);
            set_val(&mut ser, "c:order", &i.to_string(), &["c:tx", "c:spPr"]);
            put_multi_ser(&mut g, ser);
        }
        new_groups.push(g);
    }
    // Axes: the second pair comes and goes.
    let mut axes: Vec<Element> = plot.elements().filter(|e| is_axis(&e.name)).cloned().collect();
    let is_second = |e: &Element| e.child("c:axId").and_then(|a| a.attr("val")).is_some_and(|v| second.iter().any(|s| s == v));
    if !need_second {
        axes.retain(|e| !is_second(e));
    } else if second_old.is_none() {
        let pair = (second[0].parse().unwrap_or(1), second[1].parse().unwrap_or(2));
        axes.extend(frag(&format!("<c:x>{}</c:x>", build::secondary_axes_xml(pair))).elements().cloned());
    }
    let layout = plot.child("c:layout").cloned();
    let rest: Vec<Node> = plot.children.iter().filter(|n| !matches!(n, Node::Element(e) if is_group(&e.name) || is_axis(&e.name) || e.name == "c:layout")).cloned().collect();
    plot.children.clear();
    if let Some(l) = layout {
        plot.children.push(Node::Element(l));
    }
    plot.children.extend(new_groups.into_iter().map(Node::Element));
    plot.children.extend(axes.into_iter().map(Node::Element));
    plot.children.extend(rest);
    Ok(())
}

/// Add a series to a group after the ones it has (or before the first element that follows series).
fn put_multi_ser(g: &mut Element, ser: Element) {
    let after = ["c:dLbls", "c:dropLines", "c:hiLowLines", "c:upDownBars", "c:marker", "c:smooth", "c:bubble3D", "c:gapWidth", "c:overlap", "c:serLines", "c:firstSliceAng", "c:holeSize", "c:bubbleScale", "c:showNegBubbles", "c:sizeRepresents", "c:bandFmts", "c:axId", "c:extLst"];
    if let Some(last) = g.children.iter().rposition(|n| matches!(n, Node::Element(e) if e.name == "c:ser")) {
        g.children.insert(last + 1, Node::Element(ser));
    } else {
        insert_before_any(g, ser, &after);
    }
}

/// Axes kept across a type change: bars lie on their side, areas start at the axis, percent charts show %.
fn adjust_axes(axes: &mut [Element], m: &ChartModel) {
    let horizontal = m.kind == ChartKind::Bar;
    for ax in axes.iter_mut() {
        match ax.name.as_str() {
            "c:catAx" => ax.set_child_val("c:axPos", if horizontal { "l" } else { "b" }),
            "c:valAx" => {
                ax.set_child_val("c:axPos", if horizontal { "b" } else { "l" });
                if let Some(cb) = ax.child_mut("c:crossBetween") {
                    cb.set_attr("val", if m.kind == ChartKind::Area { "midCat" } else { "between" });
                }
                set_percent_format(ax, m.grouping == Grouping::Percent && m.kind.can_stack());
            }
            _ => {}
        }
    }
}

trait SetChildVal {
    fn set_child_val(&mut self, name: &str, v: &str);
}
impl SetChildVal for Element {
    fn set_child_val(&mut self, name: &str, v: &str) {
        if let Some(c) = self.child_mut(name) {
            c.set_attr("val", v);
        }
    }
}

fn set_percent_format(val_ax: &mut Element, percent: bool) {
    if let Some(nf) = val_ax.child_mut("c:numFmt") {
        if percent {
            nf.set_attr("formatCode", "0%");
            nf.set_attr("sourceLinked", "0");
        } else if nf.attr("formatCode") == Some("0%") {
            nf.set_attr("formatCode", "General");
            nf.set_attr("sourceLinked", "1");
        }
    }
}

/// Stacking changed on a chart that keeps its type.
fn regroup(plot: &mut Element, m: &ChartModel) {
    for g in plot.elements_mut() {
        let named = |stack: &str| match m.grouping {
            Grouping::Clustered => stack.to_string(),
            Grouping::Stacked => "stacked".into(),
            Grouping::Percent => "percentStacked".into(),
        };
        match g.name.as_str() {
            "c:barChart" | "c:bar3DChart" => {
                set_val(g, "c:grouping", &named("clustered"), &["c:varyColors", "c:ser"]);
                let (gap, overlap) = if m.grouping == Grouping::Clustered { ("219", "-27") } else { ("150", "100") };
                set_val(g, "c:overlap", overlap, &["c:serLines", "c:axId"]);
                if g.child("c:gapWidth").is_none() {
                    set_val(g, "c:gapWidth", gap, &["c:overlap", "c:serLines", "c:axId"]);
                }
            }
            "c:lineChart" | "c:line3DChart" | "c:areaChart" | "c:area3DChart" => set_val(g, "c:grouping", &named("standard"), &["c:varyColors", "c:ser"]),
            _ => {}
        }
    }
    for ax in plot.elements_mut().filter(|e| e.name == "c:valAx") {
        set_percent_format(ax, m.grouping == Grouping::Percent && m.kind.can_stack());
    }
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

fn text_of_first_f(e: &Element) -> Option<String> {
    e.find("c:f").map(|f| f.text())
}

fn update_series(plot: &mut Element, m: &ChartModel, refs: &[SeriesRefs], keep_refs: bool) -> Result<(), String> {
    let want = m.series.len();
    let mut order = series_order(plot);
    // Fewer series: drop from the end.
    while order.len() > want {
        let (gi, si) = order.pop().unwrap();
        if let Node::Element(g) = &mut plot.children[gi] {
            g.children.remove(si);
        }
        for o in order.iter_mut().filter(|o| o.0 == gi && o.1 > si) {
            o.1 -= 1;
        }
    }
    // More: copy the last series, without the formatting that would repeat its colour or its lines.
    while order.len() < want {
        let Some(&(gi, si)) = order.last() else { return Err("this chart has no series to copy".into()) };
        let next_order = plot
            .elements()
            .filter(|e| is_group(&e.name))
            .flat_map(|g| g.children_named("c:ser"))
            .filter_map(|s| s.child("c:order")?.attr("val")?.parse::<i64>().ok())
            .max()
            .map_or(0, |v| v + 1);
        let Node::Element(g) = &mut plot.children[gi] else { break };
        let Node::Element(last) = &g.children[si] else { break };
        let mut copy = last.clone();
        for n in ["c:spPr", "c:dPt", "c:trendline", "c:errBars", "c:extLst"] {
            copy.remove_children(n);
        }
        if let Some(mk) = copy.child_mut("c:marker") {
            mk.remove_children("c:spPr");
        }
        set_val(&mut copy, "c:order", &next_order.to_string(), &["c:tx", "c:spPr"]);
        g.children.insert(si + 1, Node::Element(copy));
        for o in order.iter_mut().filter(|o| o.0 == gi && o.1 > si) {
            o.1 += 1;
        }
        order.push((gi, si + 1));
    }
    let n_cat = m.categories.len();
    for (i, &(gi, si)) in order.iter().enumerate() {
        let Node::Element(g) = &mut plot.children[gi] else { continue };
        let group_kind = group_kind(g).unwrap_or(m.kind);
        let Node::Element(ser) = &mut g.children[si] else { continue };
        let Some(s) = m.series.get(i) else { break };
        let r = &refs[i];
        set_val(ser, "c:idx", &i.to_string(), &["c:order"]);
        set_val(ser, "c:order", &i.to_string(), &["c:tx", "c:spPr"]);
        let keep = |tag: &str, new: &str, ser: &Element| -> String {
            if keep_refs {
                ser.child(tag).and_then(text_of_first_f).unwrap_or_else(|| new.to_string())
            } else {
                new.to_string()
            }
        };
        let name_f = keep("c:tx", &r.name, ser);
        put_in(ser, frag(&format!("<c:tx>{}</c:tx>", build::str_ref_xml(&name_f, std::slice::from_ref(&s.name)))), SER_ORDER);
        let fmt = |tag: &str, ser: &Element| ser.child(tag).and_then(|h| h.find("c:formatCode")).map(|f| f.text()).filter(|c| !c.is_empty()).unwrap_or_else(|| "General".into());
        if xy(group_kind) {
            let xs = s.x_values.clone().unwrap_or_default();
            let xf = keep("c:xVal", &r.cat, ser);
            let x_fmt = fmt("c:xVal", ser);
            put_in(ser, frag(&format!("<c:xVal>{}</c:xVal>", build::num_ref_xml(&xf, &xs, &x_fmt))), SER_ORDER);
            let yf = keep("c:yVal", &r.val, ser);
            let y_fmt = fmt("c:yVal", ser);
            put_in(ser, frag(&format!("<c:yVal>{}</c:yVal>", build::num_ref_xml(&yf, &s.values, &y_fmt))), SER_ORDER);
            if group_kind == ChartKind::Bubble {
                let sizes = s.sizes.clone().unwrap_or_default();
                let sf = keep("c:bubbleSize", s.size_ref.as_deref().unwrap_or(&r.val), ser);
                let z_fmt = fmt("c:bubbleSize", ser);
                put_in(ser, frag(&format!("<c:bubbleSize>{}</c:bubbleSize>", build::num_ref_xml(&sf, &sizes, &z_fmt))), SER_ORDER);
            }
        } else {
            let cf = keep("c:cat", &r.cat, ser);
            put_in(ser, frag(&format!("<c:cat>{}</c:cat>", build::str_ref_xml(&cf, &m.categories))), SER_ORDER);
            let vf = keep("c:val", &r.val, ser);
            let v_fmt = fmt("c:val", ser);
            put_in(ser, frag(&format!("<c:val>{}</c:val>", build::num_ref_xml(&vf, &s.values, &v_fmt))), SER_ORDER);
            // Per-point formatting for points that no longer exist.
            ser.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "c:dPt" && e.child("c:idx").and_then(|x| x.attr("val")).and_then(|v| v.parse::<usize>().ok()).is_some_and(|ix| ix >= n_cat)));
            if let Some(dl) = ser.child_mut("c:dLbls") {
                dl.children.retain(|n| !matches!(n, Node::Element(e) if e.name == "c:dLbl" && e.child("c:idx").and_then(|x| x.attr("val")).and_then(|v| v.parse::<usize>().ok()).is_some_and(|ix| ix >= n_cat)));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Labels, titles
// ---------------------------------------------------------------------------

const SHOW_FLAGS: [&str; 5] = ["c:showLegendKey", "c:showVal", "c:showCatName", "c:showSerName", "c:showPercent"];

fn set_data_labels(plot: &mut Element, on: bool, m: &ChartModel) {
    let pie = matches!(m.kind, ChartKind::Pie | ChartKind::Doughnut);
    for g in plot.elements_mut().filter(|e| is_group(&e.name)) {
        if on {
            let make = || frag(&format!("<c:x>{}</c:x>", dlbls_group(pie))).elements().next().cloned().unwrap();
            match g.child_mut("c:dLbls") {
                Some(d) => {
                    d.remove_children("c:delete");
                    if !SHOW_FLAGS.iter().any(|f| d.child(f).is_some()) {
                        *d = make();
                    } else if d.child("c:showVal").is_some_and(|v| v.attr("val") == Some("0")) && d.child("c:showPercent").is_some_and(|v| v.attr("val") == Some("0")) {
                        d.set_child_val(if pie { "c:showPercent" } else { "c:showVal" }, "1");
                    }
                }
                None => insert_before_any(g, make(), GROUP_AFTER_DLBLS),
            }
            // A series with its own labels turned off shows them now.
            for ser in g.elements_mut().filter(|e| e.name == "c:ser") {
                if let Some(d) = ser.child_mut("c:dLbls") {
                    d.remove_children("c:delete");
                    let any = ["c:showVal", "c:showPercent", "c:showCatName"].iter().any(|f| d.child(f).is_some_and(|v| v.attr("val") != Some("0")));
                    if !any {
                        d.set_child_val(if pie { "c:showPercent" } else { "c:showVal" }, "1");
                    }
                }
            }
        } else {
            let mut all: Vec<&mut Element> = Vec::new();
            fn collect<'a>(e: &'a mut Element, out: &mut Vec<&'a mut Element>) {
                for c in e.elements_mut() {
                    if c.name == "c:dLbls" {
                        out.push(c);
                    } else if c.name == "c:ser" {
                        collect(c, out);
                    }
                }
            }
            collect(g, &mut all);
            for d in all {
                for f in SHOW_FLAGS {
                    d.set_child_val(f, "0");
                }
                d.set_child_val("c:showBubbleSize", "0");
                d.remove_children("c:dLbl");
            }
        }
    }
}

fn dlbls_group(pie: bool) -> String {
    format!(
        "<c:dLbls><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr><c:showLegendKey val=\"0\"/><c:showVal val=\"{}\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"{}\"/><c:showBubbleSize val=\"0\"/>{}</c:dLbls>",
        (!pie) as u8,
        pie as u8,
        if pie { "<c:showLeaderLines val=\"1\"/>" } else { "" }
    )
}

/// The chart's title: keeps the title's formatting and changes the words.
pub(super) fn set_title(chart: &mut Element, text: &str) {
    if text.is_empty() {
        chart.remove_children("c:title");
        set_val(chart, "c:autoTitleDeleted", "1", &["c:pivotFmts", "c:view3D", "c:floor", "c:sideWall", "c:backWall", "c:plotArea"]);
        return;
    }
    set_val(chart, "c:autoTitleDeleted", "0", &["c:pivotFmts", "c:view3D", "c:floor", "c:sideWall", "c:backWall", "c:plotArea"]);
    let fresh = frag(&build::title_xml(text, 1400, None));
    match chart.child_mut("c:title") {
        Some(title) => {
            let rich = title.child("c:tx").and_then(|t| t.child("c:rich")).cloned();
            match rich {
                Some(mut rich) => {
                    let first_p = rich.child("a:p").cloned();
                    let ppr = first_p.as_ref().and_then(|p| p.child("a:pPr")).cloned();
                    let rpr = first_p.as_ref().and_then(|p| p.child("a:r")).and_then(|r| r.child("a:rPr")).cloned();
                    rich.remove_children("a:p");
                    for line in text.split('\n') {
                        let mut p = Element::new("a:p");
                        if let Some(ppr) = &ppr {
                            p.children.push(Node::Element(ppr.clone()));
                        }
                        let mut r = Element::new("a:r");
                        r.children.push(Node::Element(rpr.clone().unwrap_or_else(|| Element::new("a:rPr").with_attr("lang", "en-US"))));
                        r.children.push(Node::Element(Element::new("a:t").with_text(line)));
                        p.children.push(Node::Element(r));
                        rich.children.push(Node::Element(p));
                    }
                    if let Some(tx) = title.child_mut("c:tx") {
                        tx.children = vec![Node::Element(rich)];
                    }
                }
                None => {
                    let new_tx = fresh.child("c:tx").cloned().unwrap();
                    put(title, new_tx, &["c:layout", "c:overlay", "c:spPr", "c:txPr"]);
                }
            }
        }
        None => chart.children.insert(0, Node::Element(fresh)),
    }
}

fn set_axis_title(plot: &mut Element, x: bool, kind: ChartKind, text: &str) {
    let scatter = xy(kind);
    let pick = |e: &Element| -> bool {
        if scatter {
            let is_x = e.name == "c:valAx" && matches!(e.child("c:axPos").and_then(|p| p.attr("val")), Some("b") | Some("t"));
            e.name == "c:valAx" && (is_x == x)
        } else if x {
            matches!(e.name.as_str(), "c:catAx" | "c:dateAx")
        } else {
            e.name == "c:valAx"
        }
    };
    // The first axis that matches (the main axes come first).
    let Some(ax) = plot.elements_mut().find(|e| pick(e)) else { return };
    format::set_axis_title(ax, text);
}

/// Add `c:externalData` pointing at the workbook relationship when the chart has none.
pub fn ensure_external_data(doc: &mut XmlDoc, rid: &str) {
    if doc.root.child("c:externalData").is_some() {
        return;
    }
    if doc.root.attr("xmlns:r").is_none() {
        doc.root.set_attr("xmlns:r", build::NS_R);
    }
    let el = frag(&format!("<c:externalData r:id=\"{rid}\"><c:autoUpdate val=\"0\"/></c:externalData>"));
    insert_before_any(&mut doc.root, el, &["c:printSettings", "c:userShapes", "c:extLst"]);
}

/// Check a chart's data and make every series as long as the categories (or, for scatter, its X values).
pub fn normalise(m: &mut ChartModel) -> Result<(), String> {
    if m.series.is_empty() {
        return Err("a chart needs at least one series".into());
    }
    if m.series.len() > MAX_SERIES {
        return Err(format!("a chart can have at most {MAX_SERIES} series"));
    }
    if m.categories.len() > MAX_POINTS || m.series.iter().any(|s| s.values.len() > MAX_POINTS) {
        return Err(format!("a chart can have at most {MAX_POINTS} points per series"));
    }
    if xy(m.kind) {
        let bubble = m.kind == ChartKind::Bubble;
        for s in &mut m.series {
            let xs = s.x_values.get_or_insert_with(Vec::new);
            let n = s.values.len().max(xs.len());
            s.values.resize(n, None);
            xs.resize(n, None);
            if bubble {
                s.sizes.get_or_insert_with(Vec::new).resize(n, None);
            } else {
                s.sizes = None;
            }
        }
    } else {
        let n = m.categories.len();
        if n == 0 {
            return Err("a chart needs at least one category".into());
        }
        for s in &mut m.series {
            if s.values.len() > n {
                return Err(format!("series \"{}\" has {} values but there are {n} categories", s.name, s.values.len()));
            }
            s.values.resize(n, None);
            s.sizes = None;
        }
    }
    Ok(())
}
