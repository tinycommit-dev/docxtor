//! The new chart types Word 2016 added (waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst,
//! region map), which Word writes as "chartex": a drawing in `mc:AlternateContent` whose `mc:Choice
//! Requires="cx1"` points (`cx:chart r:id`) at `word/charts/chartExN.xml`, and whose `mc:Fallback` holds a picture.
//!
//! Docxtor reads the part into a [`ChartExModel`] that already holds what the editor draws (the bins of a
//! histogram, the quartiles of a box, the running totals of a waterfall, the tree of a treemap), so the editor
//! only lays it out. Nothing here writes the part: it stays as it is on save. A resize changes the extent of the
//! drawing (and of its fallback picture), see [`apply_patch`].

use super::model::Legend;
use super::read::color_of;
use crate::image::{self, ImagePatch};
use crate::package::Package;
use crate::xml::{self, Element};
use serde::Serialize;

pub const URI_CHARTEX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
pub const REL_CHARTEX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
pub const CT_CHARTEX: &str = "application/vnd.ms-office.chartex+xml";

/// The relationship id of the chartex part a drawing shows (looking into `mc:AlternateContent`).
pub fn chartex_rid(drawing: &Element) -> Option<&str> {
    let mut all = Vec::new();
    drawing.find_all("a:graphicData", &mut all);
    all.into_iter().filter(|g| g.attr("uri") == Some(URI_CHARTEX)).find_map(|g| g.elements().find(|c| c.local() == "chart")).and_then(|c| c.attr("r:id"))
}

/// Whether a drawing (or an `mc:AlternateContent` holding one) shows a new-style chart.
pub fn is_chartex_drawing(e: &Element) -> bool {
    chartex_rid(e).is_some()
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// Which new chart type. `Pareto` is a histogram (or category counts) with a cumulative-percent line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ExKind {
    Waterfall,
    Histogram,
    Pareto,
    BoxWhisker,
    Funnel,
    Treemap,
    Sunburst,
    /// Needs map data Docxtor doesn't have: the fallback picture is shown, else a placeholder with the title.
    RegionMap,
    #[default]
    Other,
}

impl ExKind {
    pub fn from_layout(id: &str) -> ExKind {
        match id {
            "waterfall" => ExKind::Waterfall,
            "clusteredColumn" => ExKind::Histogram,
            "paretoLine" => ExKind::Pareto,
            "boxWhisker" => ExKind::BoxWhisker,
            "funnel" => ExKind::Funnel,
            "treemap" => ExKind::Treemap,
            "sunburst" => ExKind::Sunburst,
            "regionMap" => ExKind::RegionMap,
            _ => ExKind::Other,
        }
    }

    /// Docxtor draws this kind from its data.
    pub fn drawable(self) -> bool {
        !matches!(self, ExKind::RegionMap | ExKind::Other)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExAxes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cat_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub val_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub val_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub val_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub val_major: Option<f64>,
    /// The value axis has major gridlines.
    pub grid: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cat_hidden: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub val_hidden: bool,
    /// Space between bars as a fraction of a bar's width (`cx:catScaling gapWidth`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_width: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WaterfallPoint {
    pub label: String,
    /// The value in the data.
    pub value: f64,
    /// Where the bar starts and ends on the value axis.
    pub start: f64,
    pub end: f64,
    /// `increase`, `decrease` or `total`.
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Waterfall {
    pub points: Vec<WaterfallPoint>,
    /// Lines join the end of a bar to the start of the next.
    pub connectors: bool,
}

/// One bar of a histogram: a range of values, or a category.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bin {
    /// The range, for bins of numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lo: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hi: Option<f64>,
    /// The category, for bars by category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub count: f64,
    /// Whether the range includes its ends (`[lo, hi]` or `(lo, hi]`).
    pub closed_lo: bool,
    pub closed_hi: bool,
    /// `bin`, `underflow`, `overflow` or `category`.
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Histogram {
    pub bins: Vec<Bin>,
    /// Bars are categories, not ranges of numbers.
    pub by_category: bool,
    /// Pareto: the running share of the total after each bar, 0 to 1. The bars are sorted largest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BoxStat {
    pub n: usize,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    /// The ends of the whiskers: the lowest and highest values within 1.5 interquartile ranges of the box.
    pub whisker_lo: f64,
    pub whisker_hi: f64,
    pub mean: f64,
    pub outliers: Vec<f64>,
    /// The values inside the whiskers.
    pub points: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoxSeries {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// One box per category (none where the series has no values in it).
    pub boxes: Vec<Option<BoxStat>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxPlot {
    pub categories: Vec<String>,
    pub series: Vec<BoxSeries>,
    pub mean_marker: bool,
    pub mean_line: bool,
    pub outliers: bool,
    /// Every value inside the whiskers is drawn as a point.
    pub points: bool,
    /// `exclusive` or `inclusive`.
    pub quartile: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExPoint {
    pub label: String,
    pub value: f64,
}

/// A node of the hierarchy a treemap or sunburst shows. The root has no name.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct TreeNode {
    pub name: String,
    /// The sum of the leaves below it (or its own value, for a leaf).
    pub value: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// How many levels of names there are below the root.
    pub fn depth(&self) -> usize {
        1 + self.children.iter().map(TreeNode::depth).max().unwrap_or(0)
    }
    pub fn leaves(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            self.children.iter().map(TreeNode::leaves).sum()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChartExModel {
    pub kind: ExKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<Legend>,
    /// The theme's accent colours, for points without a colour of their own.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accents: Vec<String>,
    pub data_labels: bool,
    /// What a data label shows: the category's name, the value.
    pub label_names: bool,
    pub label_values: bool,
    /// Excel number format of the labels and the value axis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    pub axes: ExAxes,
    /// The first series' name and colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Colours of single points (`cx:dataPt`), by index.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub point_colors: Vec<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waterfall: Option<Waterfall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub histogram: Option<Histogram>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boxes: Option<BoxPlot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funnel: Option<Vec<ExPoint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<TreeNode>,
    /// Treemap: where the names of parents go (`banner`, `overlapping`, `none`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_labels: Option<String>,
}

impl ChartExModel {
    /// A short sentence for screen readers and AI tools: `Waterfall chart "Cash flow": 6 points`.
    pub fn summary(&self) -> String {
        let kind = match self.kind {
            ExKind::Waterfall => "Waterfall chart",
            ExKind::Histogram => "Histogram",
            ExKind::Pareto => "Pareto chart",
            ExKind::BoxWhisker => "Box and whisker chart",
            ExKind::Funnel => "Funnel chart",
            ExKind::Treemap => "Treemap chart",
            ExKind::Sunburst => "Sunburst chart",
            ExKind::RegionMap => "Region map",
            ExKind::Other => "Chart",
        };
        let title = self.title.as_deref().filter(|t| !t.is_empty()).map(|t| format!(" \"{t}\"")).unwrap_or_default();
        let count = match self.kind {
            ExKind::Waterfall => self.waterfall.as_ref().map(|w| (w.points.len(), "points")),
            ExKind::Histogram | ExKind::Pareto => self.histogram.as_ref().map(|h| (h.bins.len(), "bars")),
            ExKind::BoxWhisker => self.boxes.as_ref().map(|b| (b.categories.len(), "boxes")),
            ExKind::Funnel => self.funnel.as_ref().map(|f| (f.len(), "stages")),
            ExKind::Treemap | ExKind::Sunburst => self.tree.as_ref().map(|t| (t.children.len(), "groups")),
            _ => None,
        };
        match count {
            Some((n, what)) => format!("{kind}{title}: {n} {what}"),
            None => format!("{kind}{title}"),
        }
    }

    /// Whether the model holds something to draw.
    pub fn has_data(&self) -> bool {
        self.waterfall.as_ref().is_some_and(|w| !w.points.is_empty())
            || self.histogram.as_ref().is_some_and(|h| !h.bins.is_empty())
            || self.boxes.as_ref().is_some_and(|b| b.series.iter().any(|s| s.boxes.iter().any(Option::is_some)))
            || self.funnel.as_ref().is_some_and(|f| !f.is_empty())
            || self.tree.as_ref().is_some_and(|t| t.value > 0.0)
    }
}

// ---------------------------------------------------------------------------
// The maths
// ---------------------------------------------------------------------------

/// A percentile of sorted values the way Excel's `QUARTILE.INC` (`inclusive`) and `QUARTILE.EXC` (exclusive) do.
pub fn percentile(sorted: &[f64], p: f64, inclusive: bool) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    let pos = if inclusive { (n as f64 - 1.0) * p } else { (n as f64 + 1.0) * p - 1.0 };
    let pos = pos.clamp(0.0, n as f64 - 1.0);
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// The five numbers of a box, its mean and the points outside the whiskers.
pub fn box_stat(values: &[f64], inclusive: bool) -> Option<BoxStat> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let q1 = percentile(&v, 0.25, inclusive);
    let median = percentile(&v, 0.5, inclusive);
    let q3 = percentile(&v, 0.75, inclusive);
    let iqr = q3 - q1;
    let (lo_fence, hi_fence) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    let inside: Vec<f64> = v.iter().copied().filter(|x| *x >= lo_fence - 1e-12 && *x <= hi_fence + 1e-12).collect();
    let outliers: Vec<f64> = v.iter().copied().filter(|x| *x < lo_fence - 1e-12 || *x > hi_fence + 1e-12).collect();
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    Some(BoxStat {
        n: v.len(),
        q1,
        median,
        q3,
        whisker_lo: inside.first().copied().unwrap_or(q1),
        whisker_hi: inside.last().copied().unwrap_or(q3),
        mean,
        outliers,
        points: inside,
    })
}

/// Bars of a waterfall from the data: each value rises or falls from the running total; a subtotal point
/// starts at zero and takes the value in the data as the new running total.
pub fn waterfall(labels: &[String], values: &[Option<f64>], subtotals: &[usize]) -> Vec<WaterfallPoint> {
    let n = labels.len().max(values.len());
    let mut running = 0.0;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let v = values.get(i).copied().flatten().unwrap_or(0.0);
        let label = labels.get(i).cloned().unwrap_or_else(|| (i + 1).to_string());
        if subtotals.contains(&i) {
            running = v;
            out.push(WaterfallPoint { label, value: v, start: 0.0, end: v, kind: "total" });
        } else {
            let start = running;
            running += v;
            out.push(WaterfallPoint { label, value: v, start, end: running, kind: if v < 0.0 { "decrease" } else { "increase" } });
        }
    }
    out
}

/// How a histogram cuts the number line (`cx:binning`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BinSpec {
    pub size: Option<f64>,
    pub count: Option<usize>,
    pub underflow: Option<f64>,
    pub overflow: Option<f64>,
    /// Intervals are `(lo, hi]` (`r`, Excel's default) or `[lo, hi)` (`l`).
    pub closed_left: bool,
}

/// The bin width of Scott's normal reference rule, which Excel uses for automatic bins:
/// `3.49 s / n^(1/3)`, with `s` the sample standard deviation.
pub fn scott_width(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    let w = 3.49 * var.sqrt() / (n as f64).cbrt();
    (w.is_finite() && w > 0.0).then_some(w)
}

/// Count values into bins as Excel does: the first bin starts at the lowest value (or the underflow value), the
/// width is given, or comes from a bin count, or from Scott's rule; values at or below the underflow value and
/// above the overflow value get a bin of their own.
pub fn bin(values: &[f64], spec: &BinSpec) -> Vec<Bin> {
    let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return Vec::new();
    }
    let min = v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lo = spec.underflow.filter(|u| *u < max).unwrap_or(min);
    let hi = spec.overflow.filter(|o| *o > lo).unwrap_or(max);
    let span = hi - lo;
    let (width, nb) = if let Some(s) = spec.size.filter(|s| *s > 0.0) {
        (s, ((span / s) - 1e-9).ceil().max(1.0) as usize)
    } else if let Some(c) = spec.count.filter(|c| *c > 0) {
        (if span > 0.0 { span / c as f64 } else { 1.0 }, c)
    } else {
        match scott_width(&v) {
            Some(w) if span > 0.0 => (w, ((span / w) - 1e-9).ceil().max(1.0) as usize),
            _ => (if span > 0.0 { span } else { 1.0 }, 1),
        }
    };
    let nb = nb.min(1000);
    let right = !spec.closed_left;
    let mut counts = vec![0.0; nb];
    let (mut under, mut over) = (0.0, 0.0);
    for x in &v {
        if let Some(u) = spec.underflow.filter(|u| *u < max) {
            if (right && *x <= u) || (!right && *x < u) {
                under += 1.0;
                continue;
            }
        }
        if let Some(o) = spec.overflow.filter(|o| *o > lo) {
            if (right && *x > o) || (!right && *x >= o) {
                over += 1.0;
                continue;
            }
        }
        let t = (x - lo) / width;
        let idx = if right { (t - 1e-9).ceil() as i64 - 1 } else { (t + 1e-9).floor() as i64 };
        counts[idx.clamp(0, nb as i64 - 1) as usize] += 1.0;
    }
    let mut out = Vec::new();
    if spec.underflow.is_some_and(|u| u < max) {
        out.push(Bin { lo: None, hi: spec.underflow, label: None, count: under, closed_lo: false, closed_hi: right, kind: "underflow" });
    }
    for (i, c) in counts.iter().enumerate() {
        let a = lo + i as f64 * width;
        let b = lo + (i + 1) as f64 * width;
        let (closed_lo, closed_hi) = if right { (i == 0 && spec.underflow.is_none_or(|u| u >= max), true) } else { (true, i == nb - 1 && spec.overflow.is_none_or(|o| o <= lo)) };
        out.push(Bin { lo: Some(clean(a)), hi: Some(clean(b)), label: None, count: *c, closed_lo, closed_hi, kind: "bin" });
    }
    if spec.overflow.is_some_and(|o| o > lo) {
        out.push(Bin { lo: spec.overflow, hi: None, label: None, count: over, closed_lo: !right, closed_hi: false, kind: "overflow" });
    }
    out
}

/// Bars by category: the sum of the values in each (or a count, when the data has no values), in order of first
/// appearance.
pub fn bin_categories(labels: &[Option<String>], values: Option<&[Option<f64>]>) -> Vec<Bin> {
    let mut out: Vec<Bin> = Vec::new();
    for (i, l) in labels.iter().enumerate() {
        let Some(l) = l else { continue };
        let add = match values {
            Some(v) => match v.get(i).copied().flatten() {
                Some(x) => x,
                None => continue,
            },
            None => 1.0,
        };
        match out.iter_mut().find(|b| b.label.as_deref() == Some(l)) {
            Some(b) => b.count += add,
            None => out.push(Bin { lo: None, hi: None, label: Some(l.clone()), count: add, closed_lo: false, closed_hi: false, kind: "category" }),
        }
    }
    out
}

/// A Pareto: the bars largest first, and the running share of the total after each.
pub fn pareto(mut bins: Vec<Bin>) -> (Vec<Bin>, Vec<f64>) {
    bins.sort_by(|a, b| b.count.total_cmp(&a.count));
    let total: f64 = bins.iter().map(|b| b.count.max(0.0)).sum();
    let mut run = 0.0;
    let cum = bins
        .iter()
        .map(|b| {
            run += b.count.max(0.0);
            if total > 0.0 {
                (run / total).min(1.0)
            } else {
                0.0
            }
        })
        .collect();
    (bins, cum)
}

/// Round away floating-point noise from an edge (`0.30000000000000004` to `0.3`).
fn clean(v: f64) -> f64 {
    let r = (v * 1e9).round() / 1e9;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// The hierarchy of a treemap or sunburst from rows: `levels[l][row]` is the name at level `l` (root level
/// first; `None` where the file leaves a cell empty, which means "same as above" for the levels above the
/// leaf), `values[row]` its size.
pub fn build_tree(levels: &[Vec<Option<String>>], values: &[f64]) -> TreeNode {
    let mut root = TreeNode::default();
    let rows = levels.iter().map(Vec::len).max().unwrap_or(0).min(20_000);
    let last = levels.len().saturating_sub(1);
    let mut carry: Vec<Option<String>> = vec![None; levels.len()];
    for row in 0..rows {
        let mut path: Vec<String> = Vec::new();
        let mut reset = false;
        for (l, lvl) in levels.iter().enumerate() {
            let own = lvl.get(row).cloned().flatten().filter(|s| !s.is_empty());
            if l < last {
                if own.is_some() {
                    carry[l] = own;
                    reset = true;
                } else if reset {
                    // A new parent above: the levels under it start empty.
                    carry[l] = None;
                }
                match &carry[l] {
                    Some(n) => path.push(n.clone()),
                    None => break,
                }
            } else if let Some(n) = own {
                path.push(n);
            }
        }
        if path.is_empty() {
            continue;
        }
        let v = values.get(row).copied().unwrap_or(1.0);
        let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
        let mut node = &mut root;
        for name in &path {
            let at = match node.children.iter().position(|c| c.name == *name) {
                Some(i) => i,
                None => {
                    node.children.push(TreeNode { name: name.clone(), ..Default::default() });
                    node.children.len() - 1
                }
            };
            node = &mut node.children[at];
        }
        node.value += v;
    }
    fn total(n: &mut TreeNode) -> f64 {
        let kids: f64 = n.children.iter_mut().map(total).sum();
        n.value += kids;
        n.value
    }
    total(&mut root);
    root
}

// ---------------------------------------------------------------------------
// Reading the part
// ---------------------------------------------------------------------------

fn kid<'a>(e: &'a Element, local: &str) -> Option<&'a Element> {
    e.elements().find(|c| c.local() == local)
}

fn kids<'a>(e: &'a Element, local: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
    e.elements().filter(move |c| c.local() == local)
}

fn val<'a>(e: &'a Element, local: &str) -> Option<&'a str> {
    kid(e, local).and_then(|c| c.attr("val"))
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn flag(s: Option<&str>, default: bool) -> bool {
    match s {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => default,
    }
}

/// The cells of one level of a dimension.
struct Lvl {
    count: usize,
    pts: Vec<(usize, String)>,
    format: Option<String>,
}

impl Lvl {
    /// One cell per row; empty cells are `None`.
    fn dense(&self, rows: usize) -> Vec<Option<String>> {
        let mut out = vec![None; rows];
        for (i, s) in &self.pts {
            if *i < rows {
                out[*i] = Some(s.clone());
            }
        }
        out
    }
    fn filled(&self) -> usize {
        self.pts.len()
    }
}

/// A `cx:strDim` or `cx:numDim`.
struct Dim {
    ty: String,
    lvls: Vec<Lvl>,
}

/// The most rows of a chart's data that are read.
const MAX_CELLS: usize = 100_000;

fn read_dim(d: &Element) -> Dim {
    let lvls = kids(d, "lvl")
        .map(|l| Lvl {
            // The same cap as the cell indices below: a count is never allowed to size an allocation by itself.
            count: l.attr("ptCount").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(MAX_CELLS),
            pts: kids(l, "pt")
                .filter_map(|p| {
                    let i: usize = p.attr("idx")?.parse().ok()?;
                    (i < MAX_CELLS).then(|| (i, p.text()))
                })
                .collect(),
            format: l.attr("formatCode").map(String::from),
        })
        .collect();
    Dim { ty: d.attr("type").unwrap_or("").to_string(), lvls }
}

struct Data {
    id: String,
    strs: Vec<Dim>,
    nums: Vec<Dim>,
}

impl Data {
    fn rows(&self) -> usize {
        self.strs.iter().chain(&self.nums).flat_map(|d| &d.lvls).map(|l| l.count.max(l.pts.iter().map(|(i, _)| i + 1).max().unwrap_or(0))).max().unwrap_or(0)
    }
    /// The category dimension.
    fn cat(&self) -> Option<&Dim> {
        self.strs.iter().find(|d| d.ty == "cat").or(self.strs.first())
    }
    /// The value dimension (`val`, or `size` for hierarchies).
    fn val(&self) -> Option<&Dim> {
        self.nums.iter().find(|d| matches!(d.ty.as_str(), "val" | "size")).or(self.nums.first())
    }
    /// The category of each row: the level with the most cells (the leaves, when there are several).
    fn leaf_labels(&self) -> Vec<Option<String>> {
        let rows = self.rows();
        let Some(d) = self.cat() else { return Vec::new() };
        // The first of the fullest levels.
        let mut best: Option<&Lvl> = None;
        for l in &d.lvls {
            if best.is_none_or(|b| l.filled() > b.filled()) {
                best = Some(l);
            }
        }
        best.map(|l| l.dense(rows)).unwrap_or_default()
    }
    fn values(&self) -> Option<Vec<Option<f64>>> {
        let rows = self.rows();
        let l = self.val()?.lvls.first()?;
        Some(l.dense(rows).into_iter().map(|s| s.and_then(|t| num(&t))).collect())
    }
    fn format(&self) -> Option<String> {
        self.val().and_then(|d| d.lvls.first()).and_then(|l| l.format.clone()).filter(|f| f != "General" && !f.is_empty())
    }
    /// The category levels from the root down: the level with the fewest cells first.
    fn tree_levels(&self) -> Vec<Vec<Option<String>>> {
        let rows = self.rows();
        let Some(d) = self.cat() else { return Vec::new() };
        let mut order: Vec<usize> = (0..d.lvls.len()).collect();
        // Fewer cells means higher up; with equal counts the file's first level is the leaf, as in a classic chart.
        order.sort_by(|a, b| d.lvls[*a].filled().cmp(&d.lvls[*b].filled()).then(b.cmp(a)));
        order.into_iter().map(|i| d.lvls[i].dense(rows)).collect()
    }
}

fn read_data(d: &Element) -> Data {
    Data { id: d.attr("id").unwrap_or("0").to_string(), strs: kids(d, "strDim").map(read_dim).collect(), nums: kids(d, "numDim").map(read_dim).collect() }
}

/// The text of a `cx:title` or `cx:tx`: a cell's cached value, or rich text.
fn text_of(holder: &Element) -> Option<String> {
    let tx = if holder.local() == "tx" { holder } else { kid(holder, "tx")? };
    if let Some(d) = kid(tx, "txData") {
        return kid(d, "v").map(|v| v.text());
    }
    let rich = kid(tx, "rich")?;
    let paras: Vec<String> = rich.children_named("a:p").map(|p| p.elements().filter(|e| matches!(e.name.as_str(), "a:r" | "a:fld")).filter_map(|r| r.child("a:t")).map(|t| t.text()).collect::<String>()).collect();
    Some(paras.join("\n"))
}

fn fill_of(sp: &Element, theme: &[(String, String)]) -> Option<String> {
    let fill = sp.child("a:solidFill")?;
    fill.elements().find_map(|c| color_of(c, theme))
}

struct Series<'a> {
    el: &'a Element,
    layout: &'a str,
    data: Option<&'a str>,
}

/// A box series before its statistics: name, colour, and the values of each category (by category index).
type RawBox = (String, Option<String>, Vec<(usize, Vec<f64>)>);

/// Read a chartex part. None when it isn't one or holds no series.
pub fn read(xml_bytes: &[u8], theme: &[(String, String)]) -> Option<ChartExModel> {
    let doc = xml::parse(xml_bytes).ok()?;
    let root = &doc.root;
    if root.local() != "chartSpace" {
        return None;
    }
    let chart = kid(root, "chart")?;
    let plot = kid(chart, "plotArea")?;
    let region = kid(plot, "plotAreaRegion")?;
    let datas: Vec<Data> = kid(root, "chartData").map(|cd| kids(cd, "data").map(read_data).collect()).unwrap_or_default();
    let series: Vec<Series> = kids(region, "series").map(|s| Series { el: s, layout: s.attr("layoutId").unwrap_or(""), data: val(s, "dataId") }).collect();
    // The series that gives the chart its kind and data: a Pareto's line comes second, and has no data of its own.
    let first = series.iter().find(|s| s.layout != "paretoLine").or(series.first())?;
    let data_of = |s: &Series| -> Option<&Data> {
        match s.data {
            Some(id) => datas.iter().find(|d| d.id == id),
            None => datas.first(),
        }
    };

    let mut m = ChartExModel {
        kind: if series.iter().any(|s| s.layout == "paretoLine") { ExKind::Pareto } else { ExKind::from_layout(first.layout) },
        accents: (1..=6).filter_map(|i| theme.iter().find(|(k, _)| *k == format!("accent{i}")).map(|(_, v)| v.clone())).collect(),
        ..Default::default()
    };
    m.title = kid(chart, "title").and_then(text_of).filter(|t| !t.is_empty());
    if let Some(l) = kid(chart, "legend") {
        let pos = match l.attr("pos") {
            Some("l") => "l",
            Some("t") => "t",
            Some("b") => "b",
            _ => "r",
        };
        m.legend = Some(Legend { position: pos.into() });
    }

    // Series look: name, colour, point colours, labels.
    m.name = kid(first.el, "tx").and_then(text_of).filter(|t| !t.is_empty());
    m.color = kid(first.el, "spPr").and_then(|sp| fill_of(sp, theme));
    for dp in kids(first.el, "dataPt") {
        let Some(i) = dp.attr("idx").and_then(|v| v.parse::<usize>().ok()).filter(|i| *i < 10_000) else { continue };
        if m.point_colors.len() <= i {
            m.point_colors.resize(i + 1, None);
        }
        m.point_colors[i] = kid(dp, "spPr").and_then(|sp| fill_of(sp, theme));
    }
    m.data_labels = series.iter().any(|s| {
        kid(s.el, "dataLabels").is_some_and(|d| match kid(d, "visibility") {
            Some(v) => flag(v.attr("value"), true) || flag(v.attr("categoryName"), false),
            None => true,
        })
    });
    if let Some(d) = series.iter().find_map(|s| kid(s.el, "dataLabels")) {
        let vis = kid(d, "visibility");
        m.label_values = vis.is_none_or(|v| flag(v.attr("value"), true));
        m.label_names = vis.is_some_and(|v| flag(v.attr("categoryName"), false));
    }
    m.number_format = kid(first.el, "dataLabels")
        .and_then(|d| kid(d, "numFmt"))
        .filter(|n| n.attr("sourceLinked") != Some("1"))
        .and_then(|n| n.attr("formatCode"))
        .filter(|c| *c != "General")
        .map(String::from);

    // Axes.
    let pareto_axis = series.iter().find(|s| s.layout == "paretoLine").and_then(|s| val(s.el, "axisId")).map(String::from);
    let mut seen_val = false;
    for ax in kids(plot, "axis") {
        let hidden = flag(ax.attr("hidden"), false);
        let title = kid(ax, "title").and_then(text_of).filter(|t| !t.is_empty());
        if let Some(cs) = kid(ax, "catScaling") {
            m.axes.cat_title = title;
            m.axes.cat_hidden = hidden;
            m.axes.gap_width = cs.attr("gapWidth").and_then(num);
        } else if let Some(vs) = kid(ax, "valScaling") {
            // The secondary axis of a Pareto is the cumulative percent: not the one the bars use.
            if pareto_axis.is_some() && pareto_axis.as_deref() == ax.attr("id") {
                continue;
            }
            if seen_val {
                continue;
            }
            seen_val = true;
            m.axes.val_title = title;
            m.axes.val_hidden = hidden;
            m.axes.val_min = vs.attr("min").and_then(num);
            m.axes.val_max = vs.attr("max").and_then(num);
            m.axes.val_major = vs.attr("majorUnit").and_then(num);
            m.axes.grid = kid(ax, "majorGridlines").is_some();
            if let Some(nf) = kid(ax, "numFmt").filter(|n| n.attr("sourceLinked") != Some("1")) {
                if let Some(code) = nf.attr("formatCode").filter(|c| *c != "General") {
                    m.number_format.get_or_insert_with(|| code.to_string());
                }
            }
        }
    }

    // The data, by kind.
    let layout = kid(first.el, "layoutPr");
    let Some(data) = data_of(first) else { return Some(m) };
    if m.number_format.is_none() {
        m.number_format = data.format();
    }
    match m.kind {
        ExKind::Waterfall => {
            let labels: Vec<String> = data.leaf_labels().into_iter().map(Option::unwrap_or_default).collect();
            let values = data.values();
            let subtotals: Vec<usize> = layout.and_then(|l| kid(l, "subtotals")).map(|s| kids(s, "idx").filter_map(|i| i.attr("val").and_then(|v| v.parse().ok())).collect()).unwrap_or_default();
            let connectors = layout.and_then(|l| kid(l, "visibility")).is_none_or(|v| flag(v.attr("connectorLines"), true));
            if let Some(values) = values {
                m.waterfall = Some(Waterfall { points: waterfall(&labels, &values, &subtotals), connectors });
            }
        }
        ExKind::Histogram | ExKind::Pareto => {
            let binning = layout.and_then(|l| kid(l, "binning"));
            let by_category = layout.is_some_and(|l| kid(l, "aggregation").is_some()) || (binning.is_none() && data.cat().is_some_and(|d| d.lvls.iter().any(|l| l.filled() > 0)));
            let bins = if by_category {
                bin_categories(&data.leaf_labels(), data.values().as_deref())
            } else {
                let values: Vec<f64> = data.values().unwrap_or_default().into_iter().flatten().collect();
                let auto = |a: Option<&str>| a.filter(|s| *s != "auto").and_then(num);
                let spec = BinSpec {
                    size: binning.and_then(|b| val(b, "binSize")).and_then(num),
                    count: binning.and_then(|b| val(b, "binCount")).and_then(|v| v.parse().ok()),
                    underflow: binning.and_then(|b| auto(b.attr("underflow"))),
                    overflow: binning.and_then(|b| auto(b.attr("overflow"))),
                    closed_left: binning.and_then(|b| b.attr("intervalClosed")) == Some("l"),
                };
                bin(&values, &spec)
            };
            let mut h = Histogram { bins, by_category, cumulative: None, line_name: None, line_color: None };
            if m.kind == ExKind::Pareto {
                let (bins, cum) = pareto(std::mem::take(&mut h.bins));
                h.bins = bins;
                h.cumulative = Some(cum);
                if let Some(line) = series.iter().find(|s| s.layout == "paretoLine") {
                    h.line_name = kid(line.el, "tx").and_then(text_of).filter(|t| !t.is_empty());
                    h.line_color = kid(line.el, "spPr").and_then(|sp| kid(sp, "ln").and_then(|l| fill_of(l, theme)).or_else(|| fill_of(sp, theme)));
                }
            }
            m.histogram = Some(h);
        }
        ExKind::BoxWhisker => {
            let vis = layout.and_then(|l| kid(l, "visibility"));
            let inclusive = layout.and_then(|l| kid(l, "statistics").and_then(|s| s.attr("quartileMethod")).or_else(|| val(l, "quartileMethod"))) == Some("inclusive");
            let mut categories: Vec<String> = Vec::new();
            let mut per_series: Vec<RawBox> = Vec::new();
            let mut any_cat = false;
            for (si, s) in series.iter().filter(|s| s.layout == "boxWhisker").enumerate() {
                let Some(d) = data_of(s) else { continue };
                let labels = d.leaf_labels();
                any_cat |= labels.iter().any(Option::is_some);
                let values = d.values().unwrap_or_default();
                let mut groups: Vec<(usize, Vec<f64>)> = Vec::new();
                for (i, v) in values.iter().enumerate() {
                    let Some(v) = v else { continue };
                    let label = labels.get(i).cloned().flatten().unwrap_or_default();
                    let ci = match categories.iter().position(|c| *c == label) {
                        Some(c) => c,
                        None => {
                            categories.push(label);
                            categories.len() - 1
                        }
                    };
                    match groups.iter_mut().find(|(c, _)| *c == ci) {
                        Some((_, g)) => g.push(*v),
                        None => groups.push((ci, vec![*v])),
                    }
                }
                let name = kid(s.el, "tx").and_then(text_of).filter(|t| !t.is_empty()).unwrap_or_else(|| format!("Series {}", si + 1));
                per_series.push((name, kid(s.el, "spPr").and_then(|sp| fill_of(sp, theme)), groups));
            }
            let series_out = per_series
                .into_iter()
                .map(|(name, color, groups)| BoxSeries {
                    name,
                    color,
                    boxes: (0..categories.len()).map(|c| groups.iter().find(|(ci, _)| *ci == c).and_then(|(_, v)| box_stat(v, inclusive))).collect(),
                })
                .collect();
            if !any_cat {
                categories = vec![String::new(); categories.len()];
            }
            m.boxes = Some(BoxPlot {
                categories,
                series: series_out,
                mean_marker: vis.is_none_or(|v| flag(v.attr("meanMarker"), true)),
                mean_line: vis.is_some_and(|v| flag(v.attr("meanLine"), false)),
                outliers: vis.is_none_or(|v| flag(v.attr("outliers"), true)),
                points: vis.is_some_and(|v| flag(v.attr("nonoutliers"), false)),
                quartile: if inclusive { "inclusive" } else { "exclusive" },
            });
        }
        ExKind::Funnel => {
            let labels = data.leaf_labels();
            let values = data.values().unwrap_or_default();
            let n = labels.len().max(values.len());
            m.funnel = Some(
                (0..n)
                    .filter_map(|i| {
                        let v = values.get(i).copied().flatten()?;
                        Some(ExPoint { label: labels.get(i).cloned().flatten().unwrap_or_else(|| (i + 1).to_string()), value: v })
                    })
                    .collect(),
            );
        }
        ExKind::Treemap | ExKind::Sunburst => {
            let levels = data.tree_levels();
            let values: Vec<f64> = match data.values() {
                Some(v) => v.into_iter().map(|x| x.unwrap_or(0.0)).collect(),
                None => vec![1.0; data.rows()],
            };
            m.tree = Some(build_tree(&levels, &values));
            m.parent_labels = layout.and_then(|l| val(l, "parentLabelLayout")).map(String::from);
        }
        ExKind::RegionMap | ExKind::Other => {}
    }
    Some(m)
}

/// A chartex part read into a model.
pub fn read_part(pkg: &Package, part: &str) -> Option<ChartExModel> {
    let bytes = pkg.get(part)?;
    read(bytes, &super::theme_for(pkg))
}

/// Resize and describe a new-style chart: the change goes to the extent of the drawing in `mc:Choice` and to the
/// picture in `mc:Fallback`, and the drawing stays inside its `mc:AlternateContent`, so the chart part, its
/// workbook and the fallback are all kept.
pub fn apply_patch(obj: &mut Element, patch: &ImagePatch) -> Result<(), String> {
    for branch in obj.elements_mut() {
        for part in branch.elements_mut() {
            match part.name.as_str() {
                "w:drawing" => image::apply(part, patch)?,
                "w:pict" => resize_vml(part, patch),
                _ => {}
            }
        }
    }
    Ok(())
}

/// The size of a VML picture is in its `style`.
fn resize_vml(pict: &mut Element, patch: &ImagePatch) {
    fn set(style: &str, key: &str, pt: f64) -> String {
        let new = format!("{key}:{}pt", (pt * 100.0).round() / 100.0);
        let mut found = false;
        let mut parts: Vec<String> = style
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .map(|kv| {
                if kv.trim().starts_with(&format!("{key}:")) {
                    found = true;
                    new.clone()
                } else {
                    kv.trim().to_string()
                }
            })
            .collect();
        if !found {
            parts.push(new);
        }
        parts.join(";")
    }
    fn walk(e: &mut Element, patch: &ImagePatch) {
        if e.name == "v:shape" || e.name == "v:rect" || e.name == "v:image" {
            let mut style = e.attr("style").unwrap_or("").to_string();
            if let Some(w) = patch.width {
                style = set(&style, "width", w);
            }
            if let Some(h) = patch.height {
                style = set(&style, "height", h);
            }
            if patch.width.is_some() || patch.height.is_some() {
                e.set_attr("style", style);
            }
            return;
        }
        for c in e.elements_mut() {
            walk(c, patch);
        }
    }
    walk(pict, patch);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn percentiles_follow_excels_two_quartile_functions() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        // QUARTILE.INC: (n - 1) p; QUARTILE.EXC: (n + 1) p - 1.
        assert_eq!(percentile(&v, 0.25, true), 2.75);
        assert_eq!(percentile(&v, 0.75, true), 6.25);
        assert_eq!(percentile(&v, 0.25, false), 2.25);
        assert_eq!(percentile(&v, 0.75, false), 6.75);
        assert_eq!(percentile(&v, 0.5, true), 4.5);
        assert_eq!(percentile(&v, 0.5, false), 4.5);
        let odd = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        assert_eq!(percentile(&odd, 0.25, true), 3.0);
        assert_eq!(percentile(&odd, 0.25, false), 2.5);
    }

    #[test]
    fn a_box_has_whiskers_inside_the_fences_and_outliers_beyond() {
        let b = box_stat(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 30.0], true).unwrap();
        assert_eq!(b.n, 10);
        assert_eq!(b.median, 5.5);
        assert_eq!(b.q1, 3.25);
        assert_eq!(b.q3, 7.75);
        assert_eq!(b.whisker_lo, 1.0);
        // The fence is 7.75 + 1.5 * 4.5 = 14.5.
        assert_eq!(b.whisker_hi, 9.0);
        assert_eq!(b.outliers, vec![30.0]);
        assert_eq!(b.points.len(), 9);
        assert!((b.mean - 7.5).abs() < 1e-12);
        let x = box_stat(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 30.0], false).unwrap();
        assert_eq!(x.q1, 2.75);
        assert_eq!(x.q3, 8.25);
        assert!(box_stat(&[], true).is_none());
        let one = box_stat(&[4.0], false).unwrap();
        assert_eq!((one.q1, one.median, one.q3, one.whisker_lo, one.whisker_hi), (4.0, 4.0, 4.0, 4.0, 4.0));
    }

    #[test]
    fn a_waterfall_runs_a_total_and_resets_at_subtotals() {
        let w = waterfall(&s(&["Start", "Sales", "Costs", "Tax", "Net"]), &[Some(100.0), Some(50.0), Some(-30.0), Some(-20.0), Some(100.0)], &[0, 4]);
        let ends: Vec<(f64, f64, &str)> = w.iter().map(|p| (p.start, p.end, p.kind)).collect();
        assert_eq!(ends, vec![(0.0, 100.0, "total"), (100.0, 150.0, "increase"), (150.0, 120.0, "decrease"), (120.0, 100.0, "decrease"), (0.0, 100.0, "total")]);
        // A subtotal takes its own value as the new total, even when it differs from the running sum.
        let w = waterfall(&s(&["a", "b", "c"]), &[Some(10.0), Some(5.0), Some(1.0)], &[1]);
        assert_eq!((w[1].start, w[1].end), (0.0, 5.0));
        assert_eq!((w[2].start, w[2].end), (5.0, 6.0));
        // A gap counts as zero.
        let w = waterfall(&s(&["a", "b"]), &[Some(3.0), None], &[]);
        assert_eq!((w[1].start, w[1].end, w[1].kind), (3.0, 3.0, "increase"));
    }

    #[test]
    fn scotts_rule_gives_the_width_excel_starts_from() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let w = scott_width(&v).unwrap();
        // s = 29.011, n^(1/3) = 4.6416
        assert!((w - 21.816).abs() < 0.01, "{w}");
        assert!(scott_width(&[5.0, 5.0, 5.0]).is_none());
        assert!(scott_width(&[5.0]).is_none());
    }

    #[test]
    fn bins_by_size_close_on_the_right_by_default() {
        let v = [1.0, 2.0, 2.0, 3.0, 4.0, 5.0, 6.0, 6.0, 7.0, 9.0];
        let bins = bin(&v, &BinSpec { size: Some(2.0), ..Default::default() });
        // Edges 1, 3, 5, 7, 9: [1,3] (3,5] (5,7] (7,9].
        let edges: Vec<(Option<f64>, Option<f64>)> = bins.iter().map(|b| (b.lo, b.hi)).collect();
        assert_eq!(edges, vec![(Some(1.0), Some(3.0)), (Some(3.0), Some(5.0)), (Some(5.0), Some(7.0)), (Some(7.0), Some(9.0))]);
        let counts: Vec<f64> = bins.iter().map(|b| b.count).collect();
        assert_eq!(counts, vec![4.0, 2.0, 3.0, 1.0]);
        assert_eq!(counts.iter().sum::<f64>(), v.len() as f64);
        assert!(bins[0].closed_lo && bins[0].closed_hi);
        assert!(!bins[1].closed_lo && bins[1].closed_hi);
    }

    #[test]
    fn bins_closed_on_the_left_put_an_edge_in_the_next_bin() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        let bins = bin(&v, &BinSpec { size: Some(2.0), closed_left: true, ..Default::default() });
        // [1,3) [3,5]: the 5 stays in the last bin, the 3 starts the second.
        let counts: Vec<f64> = bins.iter().map(|b| b.count).collect();
        assert_eq!(counts, vec![2.0, 3.0]);
        assert!(bins[1].closed_lo && bins[1].closed_hi);
        assert!(bins[0].closed_lo && !bins[0].closed_hi);
    }

    #[test]
    fn bins_by_count_split_the_range_evenly() {
        let v = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let bins = bin(&v, &BinSpec { count: Some(5), ..Default::default() });
        assert_eq!(bins.len(), 5);
        assert_eq!(bins[0].lo, Some(0.0));
        assert_eq!(bins[4].hi, Some(10.0));
        assert_eq!(bins[1].hi, Some(4.0));
        assert_eq!(bins.iter().map(|b| b.count).sum::<f64>(), 11.0);
        assert_eq!(bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![3.0, 2.0, 2.0, 2.0, 2.0]);
    }

    #[test]
    fn automatic_bins_cover_every_value() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let bins = bin(&v, &BinSpec::default());
        // Width 21.8 over a span of 99: five bins.
        assert_eq!(bins.len(), 5);
        assert_eq!(bins.iter().map(|b| b.count).sum::<f64>(), 100.0);
        assert_eq!(bins[0].lo, Some(1.0));
        assert!(bins.last().unwrap().hi.unwrap() >= 100.0);
        // All the same value: one bin holds them.
        let same = bin(&[3.0, 3.0, 3.0], &BinSpec::default());
        assert_eq!(same.len(), 1);
        assert_eq!(same[0].count, 3.0);
    }

    #[test]
    fn underflow_and_overflow_bins_catch_the_ends() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let bins = bin(&v, &BinSpec { size: Some(2.0), underflow: Some(2.0), overflow: Some(8.0), ..Default::default() });
        let kinds: Vec<&str> = bins.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec!["underflow", "bin", "bin", "bin", "overflow"]);
        // <= 2: 1, 2. (2,4]: 3, 4. (4,6]: 5, 6. (6,8]: 7, 8. > 8: 9, 10.
        assert_eq!(bins.iter().map(|b| b.count).collect::<Vec<_>>(), vec![2.0, 2.0, 2.0, 2.0, 2.0]);
        assert_eq!(bins[0].hi, Some(2.0));
        assert_eq!(bins[4].lo, Some(8.0));
        assert_eq!(bins.iter().map(|b| b.count).sum::<f64>(), 10.0);
    }

    #[test]
    fn categories_add_up_and_a_pareto_sorts_them() {
        let labels: Vec<Option<String>> = ["Late", "Rude", "Late", "Wrong", "Late", "Rude"].iter().map(|x| Some(x.to_string())).collect();
        let bins = bin_categories(&labels, None);
        assert_eq!(bins.iter().map(|b| (b.label.clone().unwrap(), b.count)).collect::<Vec<_>>(), vec![("Late".into(), 3.0), ("Rude".into(), 2.0), ("Wrong".into(), 1.0)]);
        let vals: Vec<Option<f64>> = [5.0, 1.0, 2.0, 9.0, 3.0, 1.0].iter().map(|x| Some(*x)).collect();
        let sum = bin_categories(&labels, Some(&vals));
        assert_eq!(sum.iter().map(|b| b.count).collect::<Vec<_>>(), vec![10.0, 2.0, 9.0]);
        let (sorted, cum) = pareto(sum);
        assert_eq!(sorted.iter().map(|b| b.label.clone().unwrap()).collect::<Vec<_>>(), vec!["Late", "Wrong", "Rude"]);
        assert!((cum[0] - 10.0 / 21.0).abs() < 1e-12);
        assert!((cum[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_tree_sums_its_leaves_and_carries_parents_down() {
        let l0 = vec![Some("Fruit".to_string()), None, None, Some("Veg".to_string()), None];
        let l1 = vec![Some("Apple".to_string()), Some("Pear".to_string()), Some("Plum".to_string()), Some("Kale".to_string()), Some("Leek".to_string())];
        let t = build_tree(&[l0, l1], &[4.0, 3.0, 1.0, 2.0, 5.0]);
        assert_eq!(t.value, 15.0);
        assert_eq!(t.children.len(), 2);
        assert_eq!(t.children[0].name, "Fruit");
        assert_eq!(t.children[0].value, 8.0);
        assert_eq!(t.children[0].children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Apple", "Pear", "Plum"]);
        assert_eq!(t.children[1].value, 7.0);
        assert_eq!(t.depth(), 3);
        assert_eq!(t.leaves(), 5);
        // Each parent's value is the sum of its children's.
        fn check(n: &TreeNode) {
            if !n.children.is_empty() {
                assert!((n.value - n.children.iter().map(|c| c.value).sum::<f64>()).abs() < 1e-9);
            }
            n.children.iter().for_each(check);
        }
        check(&t);
    }

    #[test]
    fn a_row_may_end_above_the_leaf_level() {
        let l0 = vec![Some("A".to_string()), Some("B".to_string())];
        let l1 = vec![Some("a1".to_string()), None];
        let t = build_tree(&[l0, l1], &[2.0, 3.0]);
        assert_eq!(t.children[1].name, "B");
        assert_eq!(t.children[1].value, 3.0);
        assert!(t.children[1].children.is_empty());
        // Negative sizes count as nothing.
        let t = build_tree(&[vec![Some("x".to_string())]], &[-4.0]);
        assert_eq!(t.value, 0.0);
    }
}
