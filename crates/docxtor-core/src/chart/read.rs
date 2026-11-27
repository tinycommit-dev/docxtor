//! Reading a chart part (`word/charts/chartN.xml`) into a `ChartModel` from its cached data.

use super::model::*;
use crate::xml::{self, Element, Node};

/// The theme's colours by name (`dk1`, `lt1`, `accent1`, ...) as `RRGGBB`.
pub fn theme_colors(theme_xml: &[u8]) -> Vec<(String, String)> {
    let Ok(doc) = xml::parse(theme_xml) else { return Vec::new() };
    let Some(scheme) = doc.root.find("a:clrScheme") else { return Vec::new() };
    scheme
        .elements()
        .filter_map(|c| {
            let hex = c.child("a:srgbClr").and_then(|e| e.attr("val")).or_else(|| c.child("a:sysClr").and_then(|e| e.attr("lastClr")))?;
            Some((c.local().to_string(), crate::colour::hex(hex)?))
        })
        .collect()
}

/// The six accent colours Word uses when a theme says nothing.
pub const OFFICE_PALETTE: [&str; 6] = ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"];

fn val<'a>(e: &'a Element, name: &str) -> Option<&'a str> {
    e.child(name).and_then(|c| c.attr("val"))
}

fn on(e: &Element, name: &str) -> Option<bool> {
    e.child(name).map(|c| !matches!(c.attr("val"), Some("0") | Some("false")))
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

// ---------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------

fn hex_to_rgb(h: &str) -> Option<(f64, f64, f64)> {
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(f64::from);
    Some((p(0)?, p(2)?, p(4)?))
}

fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let c = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    format!("{:02X}{:02X}{:02X}", c(r), c(g), c(b))
}

fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (r, g, b) = (r / 255.0, g / 255.0, b / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-9 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-9 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s.abs() < 1e-9 {
        let v = l * 255.0;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        v * 255.0
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/// A DrawingML colour element (`a:srgbClr`, `a:schemeClr`, ...) as `RRGGBB`, with its luminance and tint
/// modifiers applied.
pub fn color_of(c: &Element, theme: &[(String, String)]) -> Option<String> {
    let base = match c.name.as_str() {
        "a:srgbClr" => crate::colour::hex(c.attr("val")?)?,
        "a:sysClr" => c.attr("lastClr").and_then(crate::colour::hex).unwrap_or_else(|| match c.attr("val") { Some("window") => "FFFFFF", _ => "000000" }.into()),
        "a:prstClr" => match c.attr("val")? {
            "black" => "000000".into(),
            "white" => "FFFFFF".into(),
            "red" => "FF0000".into(),
            "green" => "008000".into(),
            "blue" => "0000FF".into(),
            "yellow" => "FFFF00".into(),
            "gray" | "grey" => "808080".into(),
            _ => return None,
        },
        "a:schemeClr" => {
            let name = match c.attr("val")? {
                "tx1" => "dk1",
                "bg1" => "lt1",
                "tx2" => "dk2",
                "bg2" => "lt2",
                other => other,
            };
            match theme.iter().find(|(k, _)| k == name) {
                Some((_, v)) => v.clone(),
                None => match name {
                    "accent1" => OFFICE_PALETTE[0].into(),
                    "accent2" => OFFICE_PALETTE[1].into(),
                    "accent3" => OFFICE_PALETTE[2].into(),
                    "accent4" => OFFICE_PALETTE[3].into(),
                    "accent5" => OFFICE_PALETTE[4].into(),
                    "accent6" => OFFICE_PALETTE[5].into(),
                    "dk1" | "dk2" => "000000".into(),
                    "lt1" | "lt2" => "FFFFFF".into(),
                    _ => return None,
                },
            }
        }
        _ => return None,
    };
    let (mut r, mut g, mut b) = hex_to_rgb(&base)?;
    let m = |name: &str| c.child(name).and_then(|e| e.attr("val")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 100000.0);
    let (lum_mod, lum_off) = (m("a:lumMod"), m("a:lumOff"));
    if lum_mod.is_some() || lum_off.is_some() {
        let (h, s, l) = rgb_to_hsl(r, g, b);
        let l = (l * lum_mod.unwrap_or(1.0) + lum_off.unwrap_or(0.0)).clamp(0.0, 1.0);
        (r, g, b) = hsl_to_rgb(h, s, l);
    }
    if let Some(t) = m("a:tint") {
        let f = |c: f64| c + (255.0 - c) * (1.0 - t);
        (r, g, b) = (f(r), f(g), f(b));
    }
    if let Some(sh) = m("a:shade") {
        (r, g, b) = (r * sh, g * sh, b * sh);
    }
    Some(rgb_to_hex(r, g, b))
}

fn fill_color(sp: &Element, theme: &[(String, String)]) -> Option<String> {
    let fill = sp.child("a:solidFill")?;
    fill.elements().find_map(|c| color_of(c, theme))
}

/// A series' own colour: its fill, else its line, else its marker.
fn series_color(ser: &Element, theme: &[(String, String)]) -> Option<String> {
    let sp = ser.child("c:spPr");
    sp.and_then(|s| fill_color(s, theme))
        .or_else(|| sp.and_then(|s| s.child("a:ln")).and_then(|l| fill_color(l, theme)))
        .or_else(|| ser.child("c:marker").and_then(|m| m.child("c:spPr")).and_then(|s| fill_color(s, theme)))
}

// ---------------------------------------------------------------------------
// Cached data
// ---------------------------------------------------------------------------

/// The points of a `c:strCache` / `c:numCache` / `c:strLit` / `c:numLit`: `ptCount` entries, gaps as `None`.
fn cache_points(cache: &Element) -> Vec<Option<String>> {
    let n = val(cache, "c:ptCount").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let mut out: Vec<Option<String>> = vec![None; n];
    for pt in cache.children_named("c:pt") {
        let Some(idx) = pt.attr("idx").and_then(|v| v.parse::<usize>().ok()) else { continue };
        if idx > 100_000 {
            continue;
        }
        if idx >= out.len() {
            out.resize(idx + 1, None);
        }
        out[idx] = pt.child("c:v").map(|v| v.text());
    }
    out
}

/// The cache element inside a reference holder (`c:cat`, `c:val`, `c:tx`, `c:xVal`, `c:yVal`), and its formula.
fn data_of(holder: &Element) -> Option<(Vec<Option<String>>, Option<String>)> {
    for r in ["c:strRef", "c:numRef", "c:multiLvlStrRef"] {
        if let Some(rf) = holder.child(r) {
            let f = rf.child("c:f").map(|f| f.text());
            let cache = rf.child("c:strCache").or_else(|| rf.child("c:numCache")).or_else(|| {
                // Multi-level categories: the innermost level comes first.
                rf.child("c:multiLvlStrCache").and_then(|m| m.child("c:lvl"))
            });
            return Some((cache.map(cache_points).unwrap_or_default(), f));
        }
    }
    for l in ["c:strLit", "c:numLit"] {
        if let Some(lit) = holder.child(l) {
            return Some((cache_points(lit), None));
        }
    }
    // `c:tx` can hold the name as a bare `c:v`.
    holder.child("c:v").map(|v| (vec![Some(v.text())], None))
}

fn numbers(pts: Vec<Option<String>>) -> Vec<Option<f64>> {
    pts.into_iter().map(|p| p.and_then(|s| num(&s))).collect()
}

/// The text of a title (rich text runs joined, paragraphs on separate lines).
fn title_text(title: &Element) -> Option<String> {
    let tx = title.child("c:tx")?;
    if let Some(rich) = tx.child("c:rich") {
        let paras: Vec<String> = rich.children_named("a:p").map(|p| p.elements().filter(|e| matches!(e.name.as_str(), "a:r" | "a:fld")).filter_map(|r| r.child("a:t")).map(|t| t.text()).collect::<String>()).collect();
        let text = paras.join("\n");
        return Some(text);
    }
    let (pts, _) = data_of(tx)?;
    pts.into_iter().next().flatten()
}

// ---------------------------------------------------------------------------
// Formatting: labels, trendlines, error bars, fills, axes
// ---------------------------------------------------------------------------

fn flag(e: &Element, name: &str) -> bool {
    on(e, name) == Some(true)
}

/// The labels a `c:dLbls` asks for; None when it shows nothing.
fn read_labels(d: &Element) -> Option<DataLabels> {
    if val(d, "c:delete") == Some("1") {
        return None;
    }
    let l = DataLabels {
        value: flag(d, "c:showVal"),
        category: flag(d, "c:showCatName"),
        series: flag(d, "c:showSerName"),
        percent: flag(d, "c:showPercent"),
        legend_key: flag(d, "c:showLegendKey"),
        bubble_size: flag(d, "c:showBubbleSize"),
        position: val(d, "c:dLblPos").map(String::from),
        separator: d.child("c:separator").map(|s| s.text()),
        number_format: d.child("c:numFmt").filter(|n| n.attr("sourceLinked") != Some("1")).and_then(|n| n.attr("formatCode")).map(String::from),
    };
    l.any().then_some(l)
}

fn read_trendline(t: &Element) -> Trendline {
    Trendline {
        kind: val(t, "c:trendlineType").unwrap_or("linear").to_string(),
        order: val(t, "c:order").and_then(|v| v.parse().ok()),
        period: val(t, "c:period").and_then(|v| v.parse().ok()),
        forward: val(t, "c:forward").and_then(num),
        backward: val(t, "c:backward").and_then(num),
        intercept: val(t, "c:intercept").and_then(num),
        equation: flag(t, "c:dispEq"),
        r_squared: flag(t, "c:dispRSqr"),
        name: t.child("c:name").map(|n| n.text()).filter(|n| !n.is_empty()),
    }
}

/// The amounts of a custom error bar side (`c:plus` / `c:minus`).
fn bar_amounts(side: Option<&Element>) -> Option<Vec<f64>> {
    let (pts, _) = side.and_then(data_of)?;
    Some(pts.into_iter().map(|p| p.and_then(|s| num(&s)).unwrap_or(0.0)).collect())
}

fn read_error_bars(e: &Element, bubble_like: bool) -> ErrorBars {
    let value_type = match val(e, "c:errValType") {
        Some("cust") => "custom",
        Some("percentage") => "percentage",
        Some("stdDev") => "stdDev",
        Some("stdErr") => "stdErr",
        _ => "fixed",
    };
    ErrorBars {
        dir: val(e, "c:errDir").unwrap_or(if bubble_like { "x" } else { "y" }).to_string(),
        bar_type: val(e, "c:errBarType").unwrap_or("both").to_string(),
        value_type: value_type.to_string(),
        value: val(e, "c:val").and_then(num),
        plus: bar_amounts(e.child("c:plus")),
        minus: bar_amounts(e.child("c:minus")),
        no_cap: flag(e, "c:noEndCap"),
    }
}

fn grad_color(sp: &Element, theme: &[(String, String)]) -> Option<String> {
    let gs = sp.child("a:gradFill")?.child("a:gsLst")?.child("a:gs")?;
    gs.elements().find_map(|c| color_of(c, theme))
}

struct Look {
    color: Option<String>,
    fill: Option<String>,
    line_width: Option<f64>,
    line_color: Option<String>,
}

/// How a series is filled and outlined.
fn series_look(ser: &Element, theme: &[(String, String)]) -> Look {
    let sp = ser.child("c:spPr");
    let ln = sp.and_then(|s| s.child("a:ln"));
    let solid = sp.and_then(|s| fill_color(s, theme));
    let ln_color = ln.and_then(|l| fill_color(l, theme));
    let grad = sp.and_then(|s| grad_color(s, theme));
    let color = series_color(ser, theme).or_else(|| grad.clone());
    let fill = match sp {
        Some(s) if s.child("a:noFill").is_some() => Some("none".to_string()),
        Some(s) if s.child("a:gradFill").is_some() => Some("gradient".to_string()),
        Some(s) if s.child("a:solidFill").is_some_and(|f| f.elements().any(|c| c.child("a:alpha").is_some_and(|a| a.attr("val").and_then(|v| v.parse::<u32>().ok()).is_some_and(|v| v < 100_000)))) => Some("soft".to_string()),
        _ => None,
    };
    let line_width = ln.filter(|l| l.child("a:noFill").is_none()).and_then(|l| l.attr("w")).and_then(|w| w.parse::<f64>().ok()).map(|w| (w / 12700.0 * 100.0).round() / 100.0);
    let line_color = match (&ln_color, &solid.clone().or(grad)) {
        (Some(l), Some(f)) if l != f => Some(l.clone()),
        _ => None,
    };
    Look { color, fill, line_width, line_color }
}

fn read_axis(ax: &Element) -> Axis {
    let sc = ax.child("c:scaling");
    let nf = ax.child("c:numFmt").filter(|n| n.attr("sourceLinked") != Some("1")).and_then(|n| n.attr("formatCode")).filter(|c| *c != "General");
    Axis {
        title: ax.child("c:title").and_then(title_text).filter(|t| !t.is_empty()),
        hidden: on(ax, "c:delete") == Some(true),
        reverse: sc.and_then(|s| val(s, "c:orientation")) == Some("maxMin"),
        min: sc.and_then(|s| val(s, "c:min")).and_then(num),
        max: sc.and_then(|s| val(s, "c:max")).and_then(num),
        major_unit: val(ax, "c:majorUnit").and_then(num),
        minor_unit: val(ax, "c:minorUnit").and_then(num),
        log_base: sc.and_then(|s| val(s, "c:logBase")).and_then(num),
        number_format: nf.map(String::from),
        major_grid: ax.child("c:majorGridlines").is_some(),
        minor_grid: ax.child("c:minorGridlines").is_some(),
        major_tick: val(ax, "c:majorTickMark").map(String::from),
        minor_tick: val(ax, "c:minorTickMark").map(String::from),
        label_pos: val(ax, "c:tickLblPos").map(String::from),
        display_units: ax.child("c:dispUnits").and_then(|d| val(d, "c:builtInUnit")).map(String::from),
    }
}

// ---------------------------------------------------------------------------
// The chart
// ---------------------------------------------------------------------------

struct Group<'a> {
    el: &'a Element,
    kind: ChartKind,
}

/// The kind a plot group draws; None for elements that aren't groups.
pub fn group_kind(g: &Element) -> Option<ChartKind> {
    Some(match g.name.as_str() {
        "c:barChart" | "c:bar3DChart" => {
            if val(g, "c:barDir") == Some("bar") {
                ChartKind::Bar
            } else {
                ChartKind::Column
            }
        }
        "c:lineChart" | "c:line3DChart" => ChartKind::Line,
        "c:areaChart" | "c:area3DChart" => ChartKind::Area,
        "c:pieChart" | "c:pie3DChart" => ChartKind::Pie,
        "c:doughnutChart" => ChartKind::Doughnut,
        "c:scatterChart" => ChartKind::Scatter,
        "c:radarChart" => ChartKind::Radar,
        "c:bubbleChart" => ChartKind::Bubble,
        "c:stockChart" => ChartKind::Stock,
        "c:surfaceChart" | "c:surface3DChart" => ChartKind::Surface,
        "c:ofPieChart" => ChartKind::Other,
        _ => return None,
    })
}

/// The series of a plot area in the order the chart lists them (`c:order`), as (index of the group among the
/// plot's children, index of the series among the group's children). The model's series, and the editor's
/// edits of them, follow this order.
pub fn series_order(plot: &Element) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize, i64)> = Vec::new();
    for (gi, n) in plot.children.iter().enumerate() {
        let Node::Element(g) = n else { continue };
        if group_kind(g).is_none() {
            continue;
        }
        for (si, sn) in g.children.iter().enumerate() {
            if let Node::Element(s) = sn {
                if s.name == "c:ser" {
                    let ord = s.child("c:order").and_then(|o| o.attr("val")).and_then(|v| v.parse().ok()).unwrap_or(out.len() as i64);
                    out.push((gi, si, ord));
                }
            }
        }
    }
    out.sort_by_key(|x| x.2);
    out.into_iter().map(|(g, s, _)| (g, s)).collect()
}

/// Where a plot area's axes are (indices among its children): the category (or X) axis and the value axis of the
/// first group, and the second value axis of a group on axes of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AxisIdx {
    pub x: Option<usize>,
    pub y: Option<usize>,
    pub y2: Option<usize>,
}

pub fn locate_axes(plot: &Element, kind: ChartKind) -> AxisIdx {
    let first: Vec<&str> = plot.elements().find(|g| group_kind(g).is_some()).map(|g| g.children_named("c:axId").filter_map(|a| a.attr("val")).collect()).unwrap_or_default();
    let xy = matches!(kind, ChartKind::Scatter | ChartKind::Bubble);
    let is_x_value = |ax: &Element| xy && matches!(val(ax, "c:axPos"), Some("b") | Some("t"));
    let mut out = AxisIdx::default();
    for (i, n) in plot.children.iter().enumerate() {
        let Node::Element(ax) = n else { continue };
        let id = val(ax, "c:axId").unwrap_or("");
        let primary = first.contains(&id) || first.is_empty();
        match ax.name.as_str() {
            "c:catAx" | "c:dateAx" if primary && out.x.is_none() => out.x = Some(i),
            "c:valAx" if primary => {
                if is_x_value(ax) {
                    out.x = Some(i);
                } else if out.y.is_none() {
                    out.y = Some(i);
                }
            }
            "c:valAx" if !is_x_value(ax) && out.y2.is_none() => out.y2 = Some(i),
            _ => {}
        }
    }
    out
}

fn grouping_of(g: &Element) -> Grouping {
    match val(g, "c:grouping") {
        Some("stacked") => Grouping::Stacked,
        Some("percentStacked") => Grouping::Percent,
        _ => Grouping::Clustered,
    }
}

fn has_labels(el: &Element) -> bool {
    let mut all = Vec::new();
    el.find_all("c:dLbls", &mut all);
    all.iter().any(|d| {
        if val(d, "c:delete") == Some("1") {
            return false;
        }
        ["c:showVal", "c:showPercent", "c:showCatName"].iter().any(|n| on(d, n) == Some(true))
    })
}

/// Read a chart part. None when the XML isn't a chart space.
pub fn read(xml_bytes: &[u8], theme: &[(String, String)]) -> Option<ChartModel> {
    let doc = xml::parse(xml_bytes).ok()?;
    let root = &doc.root;
    if root.local() != "chartSpace" {
        return None;
    }
    let chart = root.child("c:chart")?;
    let plot = chart.child("c:plotArea")?;
    let mut m = ChartModel { theme: theme.iter().cloned().collect(), ..Default::default() };

    // Title: absent when the chart says "no title" or has no title element.
    if val(chart, "c:autoTitleDeleted") != Some("1") {
        m.title = chart.child("c:title").and_then(title_text).filter(|t| !t.is_empty());
    }
    m.title_overlay = m.title.is_some() && chart.child("c:title").is_some_and(|t| on(t, "c:overlay") == Some(true));

    // Plot groups, in order. Each carries its own kind; the chart takes the first group's (a stock chart
    // with volume lists its bars first and is a stock chart all the same).
    let groups: Vec<Group> = plot.elements().filter_map(|g| group_kind(g).map(|kind| Group { el: g, kind })).collect();
    let Some(first) = groups.first() else {
        m.kind = ChartKind::Other;
        return Some(m);
    };
    let main = groups.iter().find(|g| matches!(g.kind, ChartKind::Stock | ChartKind::Bubble | ChartKind::Surface)).unwrap_or(first);
    m.kind = main.kind;
    m.grouping = grouping_of(first.el);
    m.vary_colors = on(first.el, "c:varyColors").unwrap_or(false) && first.kind != ChartKind::Pie && first.kind != ChartKind::Doughnut;
    m.hole = val(first.el, "c:holeSize").and_then(|v| v.parse().ok());
    m.filled = first.kind == ChartKind::Radar && val(first.el, "c:radarStyle") == Some("filled");
    m.data_labels = has_labels(plot);
    m.labels_percent = {
        let mut all = Vec::new();
        plot.find_all("c:dLbls", &mut all);
        all.iter().any(|d| on(d, "c:showPercent") == Some(true))
    };
    m.style = root.find("c:style").and_then(|s| s.attr("val")).and_then(|v| v.parse().ok());
    m.accents = (1..=6).filter_map(|i| theme.iter().find(|(k, _)| *k == format!("accent{i}")).map(|(_, v)| v.clone())).collect();
    if let Some(bar) = groups.iter().find(|g| matches!(g.el.name.as_str(), "c:barChart" | "c:bar3DChart")) {
        m.gap_width = val(bar.el, "c:gapWidth").and_then(|v| v.parse().ok());
        m.overlap = val(bar.el, "c:overlap").and_then(|v| v.parse().ok());
    }
    match m.kind {
        ChartKind::Bubble => {
            m.bubble_scale = val(main.el, "c:bubbleScale").and_then(|v| v.parse().ok());
            m.size_represents = val(main.el, "c:sizeRepresents").map(|v| if v == "w" { "width" } else { "area" }.to_string());
            m.show_negative_bubbles = on(main.el, "c:showNegBubbles") == Some(true);
        }
        ChartKind::Stock => {
            let volume = groups.iter().any(|g| matches!(g.el.name.as_str(), "c:barChart" | "c:bar3DChart"));
            let ohlc = main.el.child("c:upDownBars").is_some();
            m.stock = Some(format!("{}{}hlc", if volume { "v" } else { "" }, if ohlc { "o" } else { "" }));
        }
        ChartKind::Surface => {
            let three_d = main.el.name == "c:surface3DChart";
            let wire = on(main.el, "c:wireframe") == Some(true);
            m.surface = Some(match (three_d, wire) {
                (true, false) => "surface",
                (true, true) => "wireframe",
                (false, false) => "contour",
                (false, true) => "wireframe_contour",
            }.to_string());
            if let Some(v) = chart.child("c:view3D") {
                m.view3d = Some(View3d { rot_x: val(v, "c:rotX").and_then(|x| x.parse().ok()).unwrap_or(15), rot_y: val(v, "c:rotY").and_then(|x| x.parse().ok()).unwrap_or(20), perspective: val(v, "c:perspective").and_then(|x| x.parse().ok()).unwrap_or(30) });
            }
        }
        _ => {}
    }

    let primary_axes: Vec<&str> = first.el.children_named("c:axId").filter_map(|a| a.attr("val")).collect();
    let group_at = |gi: usize| -> Option<&Group> {
        let Node::Element(el) = &plot.children[gi] else { return None };
        groups.iter().find(|g| std::ptr::eq(g.el, el))
    };
    let bubble_like = |k: ChartKind| matches!(k, ChartKind::Scatter | ChartKind::Bubble);
    for (gi, si) in series_order(plot) {
        let Some(g) = group_at(gi) else { continue };
        let Node::Element(ser) = &g.el.children[si] else { continue };
        let g_axes: Vec<&str> = g.el.children_named("c:axId").filter_map(|a| a.attr("val")).collect();
        let secondary = !g_axes.is_empty() && !primary_axes.is_empty() && !g_axes.iter().any(|a| primary_axes.contains(a));
        let group_marker = on(g.el, "c:marker");
        let scatter_style = val(g.el, "c:scatterStyle");
        let radar_style = val(g.el, "c:radarStyle");
        let look = series_look(ser, theme);
        let mut s = ChartSeries { kind: g.kind, secondary, color: look.color, fill: look.fill, line_width: look.line_width, line_color: look.line_color, ..Default::default() };
        if let Some((pts, f)) = ser.child("c:tx").and_then(data_of) {
            s.name = pts.into_iter().next().flatten().unwrap_or_default();
            s.name_ref = f;
        }
        let (val_holder, cat_holder) = if bubble_like(g.kind) { (ser.child("c:yVal"), ser.child("c:xVal")) } else { (ser.child("c:val"), ser.child("c:cat")) };
        if let Some((pts, f)) = val_holder.and_then(data_of) {
            s.values = numbers(pts);
            s.val_ref = f;
        }
        if let Some((pts, f)) = cat_holder.and_then(data_of) {
            s.cat_ref = f;
            if bubble_like(g.kind) {
                s.x_values = Some(numbers(pts));
            } else if m.categories.is_empty() {
                m.categories = pts.into_iter().map(|p| p.unwrap_or_default()).collect();
            }
        }
        if g.kind == ChartKind::Bubble {
            if let Some((pts, f)) = ser.child("c:bubbleSize").and_then(data_of) {
                s.sizes = Some(numbers(pts));
                s.size_ref = f;
            }
        }
        // Markers and lines.
        let symbol_none = ser.child("c:marker").and_then(|mk| val(mk, "c:symbol")) == Some("none");
        s.marker = match g.kind {
            ChartKind::Line => group_marker.unwrap_or(true) && !symbol_none,
            ChartKind::Scatter => !symbol_none && scatter_style != Some("line") && scatter_style != Some("smooth"),
            ChartKind::Radar => radar_style == Some("marker") && !symbol_none,
            _ => false,
        };
        s.smooth = matches!(g.kind, ChartKind::Line | ChartKind::Scatter) && on(ser, "c:smooth") == Some(true);
        s.no_line = matches!(g.kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) && ser.child("c:spPr").and_then(|sp| sp.child("a:ln")).is_some_and(|l| l.child("a:noFill").is_some());
        // Slice colours.
        for d in ser.children_named("c:dPt") {
            if let (Some(idx), Some(sp)) = (val(d, "c:idx").and_then(|v| v.parse::<usize>().ok()), d.child("c:spPr")) {
                if idx > 10_000 {
                    continue;
                }
                if s.point_colors.len() <= idx {
                    s.point_colors.resize(idx + 1, None);
                }
                s.point_colors[idx] = fill_color(sp, theme).or_else(|| sp.child("a:ln").and_then(|l| fill_color(l, theme)));
            }
        }
        // Labels, trendlines and error bars.
        s.labels = ser.child("c:dLbls").or_else(|| g.el.child("c:dLbls")).and_then(read_labels);
        s.trendlines = ser.children_named("c:trendline").map(read_trendline).collect();
        s.error_bars = ser.children_named("c:errBars").map(|e| read_error_bars(e, bubble_like(g.kind))).collect();
        m.series.push(s);
    }
    m.combo = m.kind != ChartKind::Stock && m.series.iter().any(|s| s.kind != m.kind);
    if bubble_like(m.kind) && !m.series.is_empty() {
        m.categories.clear();
    }

    // Legend.
    if let Some(l) = chart.child("c:legend") {
        let pos = match val(l, "c:legendPos") {
            Some("l") => "l",
            Some("t") => "t",
            Some("b") => "b",
            Some("tr") => "tr",
            _ => "r",
        };
        m.legend = Some(Legend { position: pos.into() });
        m.legend_overlay = on(l, "c:overlay") == Some(true);
    }

    // Axes. The axes of the first group are the main ones; a group on axes of its own has the second value axis.
    let at = locate_axes(plot, m.kind);
    let axis_at = |i: Option<usize>| match i.and_then(|i| plot.children.get(i)) {
        Some(Node::Element(e)) => Some(read_axis(e)),
        _ => None,
    };
    m.x_axis = axis_at(at.x).unwrap_or_default();
    m.y_axis = axis_at(at.y).unwrap_or_default();
    m.y2_axis = axis_at(at.y2);
    let (x, y) = (&m.x_axis, &m.y_axis);
    m.axes = ChartAxes {
        x_title: x.title.clone(),
        y_title: y.title.clone(),
        y_min: y.min,
        y_max: y.max,
        y_major: y.major_unit,
        number_format: y.number_format.clone(),
        data_format: None,
        x_hidden: x.hidden,
        y_hidden: y.hidden,
        grid: y.major_grid,
        x_reverse: x.reverse,
        x_min: if bubble_like(m.kind) { x.min } else { None },
        x_max: if bubble_like(m.kind) { x.max } else { None },
    };
    // A number format that lives in the data.
    let mut caches = Vec::new();
    first.el.find_all("c:numCache", &mut caches);
    if let Some(code) = caches.iter().find_map(|c| c.child("c:formatCode").map(|f| f.text())) {
        if code != "General" && !code.is_empty() {
            m.axes.data_format = Some(code.clone());
            if m.axes.number_format.is_none() {
                m.axes.number_format = Some(code);
            }
        }
    }
    Some(m)
}
