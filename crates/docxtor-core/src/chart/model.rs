//! What Docxtor knows about a chart: the type, the title, the data and the few options the editor draws.
//! Everything else in the chart part stays as it is.

use serde::{Deserialize, Serialize};

/// The kind of chart (or of one series, in a combo chart). `Bar` is horizontal bars, `Column` vertical ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    #[default]
    Column,
    Bar,
    Line,
    Pie,
    Doughnut,
    Area,
    Scatter,
    Radar,
    /// Scatter with a third value per point: the size of the bubble.
    Bubble,
    /// High, low and close (and open, and volume) over time.
    Stock,
    /// Values over a grid of categories and series: 3-D surface, wireframe, contour (see `ChartModel::surface`).
    Surface,
    /// A kind Docxtor doesn't draw (of pie, ...). The editor shows a placeholder with the title.
    Other,
}

impl ChartKind {
    pub fn name(self) -> &'static str {
        match self {
            ChartKind::Column => "column",
            ChartKind::Bar => "bar",
            ChartKind::Line => "line",
            ChartKind::Pie => "pie",
            ChartKind::Doughnut => "doughnut",
            ChartKind::Area => "area",
            ChartKind::Scatter => "scatter",
            ChartKind::Radar => "radar",
            ChartKind::Bubble => "bubble",
            ChartKind::Stock => "stock",
            ChartKind::Surface => "surface",
            ChartKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<ChartKind> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "column" | "col" | "columns" | "vertical" | "vertical_bar" => ChartKind::Column,
            "bar" | "bars" | "horizontal" | "horizontal_bar" => ChartKind::Bar,
            "line" | "lines" => ChartKind::Line,
            "pie" => ChartKind::Pie,
            "doughnut" | "donut" => ChartKind::Doughnut,
            "area" => ChartKind::Area,
            "scatter" | "xy" => ChartKind::Scatter,
            "radar" | "spider" => ChartKind::Radar,
            "bubble" => ChartKind::Bubble,
            "stock" => ChartKind::Stock,
            "surface" | "contour" | "wireframe" => ChartKind::Surface,
            _ => return None,
        })
    }

    /// Charts of this kind have a category axis and a value axis.
    pub fn has_axes(self) -> bool {
        matches!(self, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Radar | ChartKind::Scatter | ChartKind::Bubble | ChartKind::Stock | ChartKind::Surface)
    }

    /// Kinds whose bars, lines or areas can be stacked.
    pub fn can_stack(self) -> bool {
        matches!(self, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area)
    }
}

/// How series of one group combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Grouping {
    /// Side by side (bars) or independent (lines, areas).
    #[default]
    Clustered,
    Stacked,
    /// Stacked, each category scaled to 100%.
    Percent,
}

impl Grouping {
    pub fn parse(s: &str) -> Option<Grouping> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "clustered" | "standard" | "grouped" | "normal" => Grouping::Clustered,
            "stacked" | "stack" => Grouping::Stacked,
            "percent" | "percentstacked" | "percent_stacked" | "100%" => Grouping::Percent,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChartSeries {
    pub name: String,
    /// One value per category (or per point, for a scatter chart's Y values); `None` is a gap.
    pub values: Vec<Option<f64>>,
    /// Hex colour (`4472C4`) when the file sets one, resolved against the theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Scatter charts: the X value of each point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_values: Option<Vec<Option<f64>>>,
    /// In a combo chart each series has its own kind; otherwise it repeats the chart's.
    #[serde(default)]
    pub kind: ChartKind,
    /// Drawn against the second value axis.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secondary: bool,
    /// Line and radar series: points have markers.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub marker: bool,
    /// Line series drawn as a smooth curve.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub smooth: bool,
    /// Scatter and line series drawn without the connecting line (markers only).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_line: bool,
    /// Pie and doughnut slices (and varied bars): the colour of each point when the file sets it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub point_colors: Vec<Option<String>>,
    /// The spreadsheet ranges the data comes from (`Sheet1!$B$2:$B$5`), kept for reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cat_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub val_ref: Option<String>,
    /// Bubble charts: the size of each bubble.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<Option<f64>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_ref: Option<String>,
    /// The data labels of this series (its own `c:dLbls`, else the group's); absent when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<DataLabels>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trendlines: Vec<Trendline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_bars: Vec<ErrorBars>,
    /// How the series is filled when not plainly: `none` (outline only), `soft` (see-through) or `gradient`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Outline or line width in points, and the outline colour when it differs from the fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
}

/// What a data label shows and where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct DataLabels {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub value: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub category: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub series: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub percent: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub legend_key: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bubble_size: bool,
    /// `ctr`, `inEnd`, `inBase`, `outEnd`, `bestFit`, `t`, `b`, `l` or `r`; absent is the chart's default place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// Between the parts of one label (`, ` `; ` a space or a new line).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    /// Excel number format of the numbers in the labels; absent follows the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
}

impl DataLabels {
    /// Whether the labels show anything.
    pub fn any(&self) -> bool {
        self.value || self.category || self.series || self.percent || self.bubble_size
    }
}

/// A trendline: `linear`, `exp`, `log`, `poly` (with `order`), `power` or `movingAvg` (with `period`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Trendline {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u32>,
    /// How far the line runs past the last point and before the first, in X units (periods for category charts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backward: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intercept: Option<f64>,
    /// Show the equation / R squared on the chart.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub equation: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub r_squared: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Default for Trendline {
    fn default() -> Self {
        Trendline { kind: "linear".into(), order: None, period: None, forward: None, backward: None, intercept: None, equation: false, r_squared: false, name: None }
    }
}

/// Error bars of a series: `dir` is `y` (or `x` for scatter and bubble charts), `bar_type` `both`, `plus` or `minus`,
/// `value_type` `fixed`, `percentage`, `stdDev`, `stdErr` or `custom` (then `plus` / `minus` list the amounts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ErrorBars {
    pub dir: String,
    pub bar_type: String,
    pub value_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plus: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minus: Option<Vec<f64>>,
    /// No cap at the end of the bars.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_cap: bool,
}

impl Default for ErrorBars {
    fn default() -> Self {
        ErrorBars { dir: "y".into(), bar_type: "both".into(), value_type: "fixed".into(), value: Some(1.0), plus: None, minus: None, no_cap: false }
    }
}

/// One axis of the chart as the dialog shows it. Absent numbers are automatic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Axis {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The axis is not shown (`c:delete`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// Runs backwards.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reverse: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_unit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_unit: Option<f64>,
    /// Logarithmic scale with this base (value axes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_base: Option<f64>,
    /// Excel number format of the tick labels; absent follows the data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub major_grid: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub minor_grid: bool,
    /// `none`, `in`, `out` or `cross`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_tick: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_tick: Option<String>,
    /// `nextTo` (the default), `low`, `high` or `none`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_pos: Option<String>,
    /// Value axes: `hundreds`, `thousands`, `millions`, `billions`, ... (`c:dispUnits`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_units: Option<String>,
}

/// The 3-D view of a surface chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct View3d {
    pub rot_x: i32,
    pub rot_y: i32,
    pub perspective: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Legend {
    /// `r`, `l`, `t`, `b` or `tr` (top right).
    pub position: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChartAxes {
    /// The category axis title (the horizontal axis for columns, the vertical one for bars; X for scatter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_title: Option<String>,
    /// The value axis title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_major: Option<f64>,
    /// Excel number format of the value axis (`General`, `0%`, `#,##0.00`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    /// The number format the data carries (`numCache` format code), used when the axis follows the data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_format: Option<String>,
    /// X axis hidden / value axis hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub x_hidden: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub y_hidden: bool,
    /// The value axis has major gridlines.
    #[serde(default)]
    pub grid: bool,
    /// The category axis runs backwards (first category last / at the top for bars).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub x_reverse: bool,
    /// X axis of a scatter chart: min and max.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChartModel {
    pub kind: ChartKind,
    #[serde(default)]
    pub grouping: Grouping,
    /// More than one kind of series in the plot area.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub combo: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<Legend>,
    #[serde(default)]
    pub axes: ChartAxes,
    /// Data labels are shown.
    #[serde(default)]
    pub data_labels: bool,
    /// The labels show percentages (pie and doughnut) instead of values.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub labels_percent: bool,
    /// Each point of a single-series chart has its own colour.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vary_colors: bool,
    /// Doughnut hole size in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole: Option<u32>,
    /// Radar chart drawn filled.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub filled: bool,
    /// Word's chart style number (`c:style`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<i64>,
    /// The legend is drawn over the plot instead of beside it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub legend_overlay: bool,
    /// The title is drawn over the plot instead of above it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub title_overlay: bool,
    /// The category axis (the X axis of scatter and bubble charts), the value axis, and the second value axis.
    #[serde(default)]
    pub x_axis: Axis,
    #[serde(default)]
    pub y_axis: Axis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2_axis: Option<Axis>,
    /// Bars: the gap between groups and how far bars of one group overlap (percent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlap: Option<i32>,
    /// Bubble charts: size of the biggest bubble in percent of the default, whether size means `area` or `width`, negatives shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bubble_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_represents: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_negative_bubbles: bool,
    /// Stock charts: `hlc`, `ohlc`, `vhlc` or `vohlc` (volume, open, high, low, close; series come in that order).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<String>,
    /// Surface charts: `surface`, `wireframe`, `contour` or `wireframe_contour`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view3d: Option<View3d>,
    /// The colour palette of `colorsN.xml` when the chart has one: `colorful1` to `colorful4`, `mono1` to `mono13`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// The theme's colours by name (`dk1`, `lt1`, `dk2`, `lt2`, `accent1` ...) as `RRGGBB`.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub theme: std::collections::BTreeMap<String, String>,
    /// The theme's accent colours, so a series without a colour of its own can be drawn as Word draws it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accents: Vec<String>,
}

impl ChartModel {
    /// A short sentence for screen readers and AI tools: `Column chart "Revenue": 3 series over 4 categories`.
    pub fn summary(&self) -> String {
        let kind = match self.kind {
            ChartKind::Other => "chart".to_string(),
            k => format!("{} chart", k.name()),
        };
        let title = self.title.as_deref().filter(|t| !t.is_empty()).map(|t| format!(" \"{t}\"")).unwrap_or_default();
        let n = self.series.len();
        let c = self.categories.len().max(self.series.iter().map(|s| s.values.len()).max().unwrap_or(0));
        let head = format!("{}{kind}", if self.combo { "combination " } else { "" });
        let mut cs = head.chars();
        let head = cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default();
        format!("{head}{title}: {n} series, {c} points")
    }
}
