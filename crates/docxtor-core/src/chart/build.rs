//! Writing a clean DrawingML chart the way Word writes one for each type. Used for new charts and when an
//! existing chart changes type.

use super::model::*;
use super::xlsx::SeriesRefs;
use crate::xml::{self, Element, XmlDoc};

pub const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// Parse an XML fragment that uses the `c:` and `a:` prefixes.
pub fn frag(s: &str) -> Element {
    let wrapped = format!("<w xmlns:c=\"{NS_C}\" xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\">{s}</w>");
    let doc = xml::parse(wrapped.as_bytes()).unwrap_or_else(|e| panic!("chart fragment: {e}: {s}"));
    let first = doc.root.elements().next().cloned();
    first.expect("fragment element")
}

const TEXT_COLOR: &str = r#"<a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill>"#;

pub fn txpr(sz: u32, bold: bool) -> String {
    format!(
        r#"<c:txPr><a:bodyPr rot="-60000000" spcFirstLastPara="1" vertOverflow="ellipsis" vert="horz" wrap="square" anchor="ctr" anchorCtr="1"/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="{sz}" b="{}" i="0" u="none" strike="noStrike" kern="1200" baseline="0">{TEXT_COLOR}<a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>"#,
        bold as u8
    )
}

/// A `c:title` with plain text.
pub fn title_xml(text: &str, sz: u32, rot: Option<i32>) -> String {
    let rot = rot.map(|r| format!(" rot=\"{r}\" vert=\"horz\"")).unwrap_or_else(|| " rot=\"0\" vert=\"horz\"".into());
    let paras: String = text
        .split('\n')
        .map(|line| {
            format!(
                r#"<a:p><a:pPr><a:defRPr sz="{sz}" b="0" i="0" u="none" strike="noStrike" kern="1200" baseline="0">{TEXT_COLOR}<a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:pPr><a:r><a:rPr lang="en-US"/><a:t>{}</a:t></a:r></a:p>"#,
                esc(line)
            )
        })
        .collect();
    format!(r#"<c:title><c:tx><c:rich><a:bodyPr spcFirstLastPara="1" vertOverflow="ellipsis"{rot} wrap="square" anchor="ctr" anchorCtr="1"/><a:lstStyle/>{paras}</c:rich></c:tx><c:overlay val="0"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr></c:title>"#)
}

pub fn str_ref_xml(f: &str, items: &[String]) -> String {
    let pts: String = items.iter().enumerate().map(|(i, v)| format!("<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", esc(v))).collect();
    format!("<c:strRef><c:f>{}</c:f><c:strCache><c:ptCount val=\"{}\"/>{pts}</c:strCache></c:strRef>", esc(f), items.len())
}

pub fn num_ref_xml(f: &str, values: &[Option<f64>], format_code: &str) -> String {
    let pts: String = values.iter().enumerate().filter_map(|(i, v)| v.map(|v| format!("<c:pt idx=\"{i}\"><c:v>{v}</c:v></c:pt>"))).collect();
    format!("<c:numRef><c:f>{}</c:f><c:numCache><c:formatCode>{}</c:formatCode><c:ptCount val=\"{}\"/>{pts}</c:numCache></c:numRef>", esc(f), esc(format_code), values.len())
}

fn dlbls_xml(percent: bool, pie: bool) -> String {
    format!(
        "<c:dLbls><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}<c:showLegendKey val=\"0\"/><c:showVal val=\"{}\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"{}\"/><c:showBubbleSize val=\"0\"/>{}</c:dLbls>",
        txpr(900, false),
        (!percent) as u8,
        percent as u8,
        if pie { "<c:showLeaderLines val=\"1\"/>" } else { "" }
    )
}

/// A colour element for a colour setting: `RRGGBB`, or `scheme:accent2` with optional modifiers
/// (`scheme:accent2:lumMod=60000:lumOff=40000`); nothing (or nonsense) means the theme's accent for the series'
/// place. `extra` is put inside the element (an alpha, a tint).
pub fn clr(color: Option<&str>, index: usize, extra: &str) -> String {
    let el = |name: &str, val: &str, inner: &str| if inner.is_empty() { format!("<a:{name} val=\"{val}\"/>") } else { format!("<a:{name} val=\"{val}\">{inner}</a:{name}>") };
    if let Some(c) = color {
        if let Some(rest) = c.strip_prefix("scheme:") {
            let mut parts = rest.split(':');
            let name = parts.next().unwrap_or("");
            if !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric()) {
                let mods: String = parts.filter_map(|p| p.split_once('=')).filter(|(k, v)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphabetic()) && !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())).map(|(k, v)| format!("<a:{k} val=\"{v}\"/>")).collect();
                return el("schemeClr", name, &format!("{mods}{extra}"));
            }
        } else if c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return el("srgbClr", &c.to_ascii_uppercase(), extra);
        }
    }
    let accent = format!("accent{}", index % 6 + 1);
    let mods = match index / 6 {
        0 => String::new(),
        1 => "<a:lumMod val=\"60000\"/>".to_string(),
        _ => "<a:lumMod val=\"80000\"/><a:lumOff val=\"20000\"/>".to_string(),
    };
    el("schemeClr", &accent, &format!("{mods}{extra}"))
}

pub fn color_xml(color: Option<&str>, index: usize) -> String {
    clr(color, index, "")
}

/// The fill of a series as its model says: plain, outline only, see-through or a soft gradient.
pub fn fill_xml(s: &ChartSeries, index: usize) -> String {
    let c = s.color.as_deref();
    match s.fill.as_deref() {
        Some("none") => "<a:noFill/>".to_string(),
        Some("soft") => format!("<a:solidFill>{}</a:solidFill>", clr(c, index, "<a:alpha val=\"70000\"/>")),
        Some("gradient") => format!(
            "<a:gradFill rotWithShape=\"1\"><a:gsLst><a:gs pos=\"0\">{}</a:gs><a:gs pos=\"100000\">{}</a:gs></a:gsLst><a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill>",
            clr(c, index, "<a:lumMod val=\"60000\"/><a:lumOff val=\"40000\"/>"),
            color_xml(c, index)
        ),
        _ => format!("<a:solidFill>{}</a:solidFill>", color_xml(c, index)),
    }
}

/// The outline of a bar, area or bubble: none, unless the series has a width (or has no fill).
pub fn outline_xml(s: &ChartSeries, index: usize) -> String {
    let w = s.line_width.or_else(|| (s.fill.as_deref() == Some("none")).then_some(2.25));
    match w {
        Some(w) => format!("<a:ln w=\"{}\"><a:solidFill>{}</a:solidFill></a:ln>", (w * 12700.0).round() as i64, color_xml(s.line_color.as_deref().or(s.color.as_deref()), index)),
        None => "<a:ln><a:noFill/></a:ln>".to_string(),
    }
}

pub fn marker_xml(s: &ChartSeries, index: usize) -> String {
    let color = color_xml(s.color.as_deref(), index);
    if s.marker {
        format!("<c:marker><c:symbol val=\"circle\"/><c:size val=\"5\"/><c:spPr><a:solidFill>{color}</a:solidFill><a:ln w=\"9525\"><a:solidFill>{color}</a:solidFill></a:ln></c:spPr></c:marker>")
    } else {
        "<c:marker><c:symbol val=\"none\"/></c:marker>".to_string()
    }
}

const INK_65: &str = "<a:schemeClr val=\"tx1\"><a:lumMod val=\"65000\"/><a:lumOff val=\"35000\"/></a:schemeClr>";

/// One `c:ser`. `role` is a stock series' place in its group (0 is the first of high-low-close or open-high-low-close).
pub fn ser_xml(kind: ChartKind, index: usize, s: &ChartSeries, refs: &SeriesRefs, categories: &[String]) -> String {
    ser_xml_role(kind, index, s, refs, categories, None)
}

pub fn ser_xml_role(kind: ChartKind, index: usize, s: &ChartSeries, refs: &SeriesRefs, categories: &[String], stock_close: Option<bool>) -> String {
    let name = format!("<c:tx>{}</c:tx>", str_ref_xml(&refs.name, std::slice::from_ref(&s.name)));
    let color = color_xml(s.color.as_deref(), index);
    let head = format!("<c:ser><c:idx val=\"{index}\"/><c:order val=\"{index}\"/>{name}");
    let xy = matches!(kind, ChartKind::Scatter | ChartKind::Bubble);
    let cat = if xy {
        let xs = s.x_values.clone().unwrap_or_else(|| (1..=s.values.len()).map(|i| Some(i as f64)).collect());
        format!("<c:xVal>{}</c:xVal>", num_ref_xml(&refs.cat, &xs, "General"))
    } else {
        format!("<c:cat>{}</c:cat>", str_ref_xml(&refs.cat, categories))
    };
    let val_tag = if xy { "yVal" } else { "val" };
    let val = format!("<c:{val_tag}>{}</c:{val_tag}>", num_ref_xml(&refs.val, &s.values, "General"));
    match kind {
        ChartKind::Column | ChartKind::Bar => format!("{head}<c:spPr>{}{}</c:spPr><c:invertIfNegative val=\"0\"/>{cat}{val}</c:ser>", fill_xml(s, index), outline_xml(s, index)),
        ChartKind::Area => format!("{head}<c:spPr>{}{}</c:spPr>{cat}{val}</c:ser>", fill_xml(s, index), outline_xml(s, index)),
        ChartKind::Pie | ChartKind::Doughnut => {
            let pts: String = (0..categories.len().max(s.values.len()))
                .map(|i| {
                    let c = s.point_colors.get(i).cloned().flatten();
                    format!("<c:dPt><c:idx val=\"{i}\"/><c:bubble3D val=\"0\"/><c:spPr><a:solidFill>{}</a:solidFill><a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"lt1\"/></a:solidFill></a:ln></c:spPr></c:dPt>", color_xml(c.as_deref(), i))
                })
                .collect();
            format!("{head}{pts}{cat}{val}</c:ser>")
        }
        ChartKind::Line | ChartKind::Radar => {
            let lw = (s.line_width.unwrap_or(2.25) * 12700.0).round() as i64;
            let smooth = if kind == ChartKind::Line { format!("<c:smooth val=\"{}\"/>", s.smooth as u8) } else { String::new() };
            format!("{head}<c:spPr><a:ln w=\"{lw}\" cap=\"rnd\"><a:solidFill>{color}</a:solidFill><a:round/></a:ln></c:spPr>{}{cat}{val}{smooth}</c:ser>", marker_xml(s, index))
        }
        ChartKind::Scatter => {
            let lw = (s.line_width.unwrap_or(1.5) * 12700.0).round() as i64;
            let line = if s.no_line { format!("<a:ln w=\"{lw}\" cap=\"rnd\"><a:noFill/><a:round/></a:ln>") } else { format!("<a:ln w=\"{lw}\" cap=\"rnd\"><a:solidFill>{color}</a:solidFill><a:round/></a:ln>") };
            format!("{head}<c:spPr>{line}</c:spPr>{}{cat}{val}<c:smooth val=\"{}\"/></c:ser>", marker_xml(s, index), s.smooth as u8)
        }
        ChartKind::Bubble => {
            let sizes = s.sizes.clone().unwrap_or_else(|| vec![Some(1.0); s.values.len()]);
            let size_f = s.size_ref.clone().unwrap_or_else(|| refs.val.clone());
            let fill = if s.fill.is_some() { fill_xml(s, index) } else { format!("<a:solidFill>{}</a:solidFill>", clr(s.color.as_deref(), index, "<a:alpha val=\"50000\"/>")) };
            let ln = format!("<a:ln w=\"{}\"><a:solidFill>{}</a:solidFill></a:ln>", ((s.line_width.unwrap_or(2.0)) * 12700.0).round() as i64, color_xml(s.line_color.as_deref().or(s.color.as_deref()), index));
            format!("{head}<c:spPr>{fill}{ln}</c:spPr><c:invertIfNegative val=\"0\"/>{cat}{val}<c:bubbleSize>{}</c:bubbleSize><c:bubble3D val=\"0\"/></c:ser>", num_ref_xml(&size_f, &sizes, "General"))
        }
        ChartKind::Stock => {
            let marker = match stock_close {
                Some(true) => format!("<c:marker><c:symbol val=\"dash\"/><c:size val=\"7\"/><c:spPr><a:solidFill>{INK_65}</a:solidFill><a:ln w=\"9525\"><a:solidFill>{INK_65}</a:solidFill></a:ln></c:spPr></c:marker>"),
                _ => "<c:marker><c:symbol val=\"none\"/></c:marker>".to_string(),
            };
            format!("{head}<c:spPr><a:ln w=\"19050\" cap=\"rnd\"><a:noFill/><a:round/></a:ln></c:spPr>{marker}{cat}{val}<c:smooth val=\"0\"/></c:ser>")
        }
        ChartKind::Surface => format!("{head}{cat}{val}</c:ser>"),
        ChartKind::Other => format!("{head}{cat}{val}</c:ser>"),
    }
}

/// One plot group of a chart: its kind, the series (indices into the model's series) and whether it uses the second axes.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub kind: ChartKind,
    pub series: Vec<usize>,
    pub secondary: bool,
}

/// Kinds that can share a plot area (a combination chart).
pub fn combinable(k: ChartKind) -> bool {
    matches!(k, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Scatter)
}

/// The groups a model needs, in the order Word writes them: areas, bars, lines, scatter; first on the main axes, then
/// on the second ones. A stock chart with volume is bars on the main axes and the prices on the second.
pub fn specs(m: &ChartModel) -> Vec<Spec> {
    let n = m.series.len();
    if m.kind == ChartKind::Stock {
        let variant = m.stock.as_deref().unwrap_or("hlc");
        let volume = variant.starts_with('v');
        let vol = usize::from(volume).min(n);
        let mut v = Vec::new();
        if volume {
            v.push(Spec { kind: ChartKind::Column, series: (0..vol).collect(), secondary: false });
        }
        v.push(Spec { kind: ChartKind::Stock, series: (vol..n).collect(), secondary: volume });
        return v;
    }
    let mixed = m.series.iter().any(|s| s.kind != m.series[0].kind || s.secondary != m.series[0].secondary) && m.series.iter().all(|s| combinable(s.kind)) && combinable(m.kind);
    if !mixed {
        return vec![Spec { kind: m.kind, series: (0..n).collect(), secondary: false }];
    }
    let mut out = Vec::new();
    for secondary in [false, true] {
        for kind in [ChartKind::Area, ChartKind::Column, ChartKind::Bar, ChartKind::Line, ChartKind::Scatter] {
            let idx: Vec<usize> = (0..n).filter(|&i| m.series[i].kind == kind && m.series[i].secondary == secondary).collect();
            if !idx.is_empty() {
                out.push(Spec { kind, series: idx, secondary });
            }
        }
    }
    out
}

fn hi_low_lines() -> String {
    r#"<c:hiLowLines><c:spPr><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="75000"/><a:lumOff val="25000"/></a:schemeClr></a:solidFill><a:round/></a:ln></c:spPr></c:hiLowLines>"#.to_string()
}

fn up_down_bars() -> String {
    r#"<c:upDownBars><c:gapWidth val="150"/><c:upBars><c:spPr><a:solidFill><a:schemeClr val="lt1"/></a:solidFill><a:ln w="9525"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill></a:ln></c:spPr></c:upBars><c:downBars><c:spPr><a:solidFill><a:schemeClr val="dk1"><a:lumMod val="75000"/><a:lumOff val="25000"/></a:schemeClr></a:solidFill><a:ln w="9525"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill></a:ln></c:spPr></c:downBars></c:upDownBars>"#.to_string()
}

/// A chart group (`c:barChart`, `c:lineChart`, ...) for series `series` (indices into the model's series).
pub fn group_xml(m: &ChartModel, kind: ChartKind, series: &[usize], refs: &[SeriesRefs], ax: (u32, u32)) -> String {
    let stock_n = if kind == ChartKind::Stock { series.len() } else { 0 };
    let sers: String = series
        .iter()
        .enumerate()
        .map(|(j, &i)| if kind == ChartKind::Stock { ser_xml_role(kind, i, &m.series[i], &refs[i], &m.categories, Some(j + 1 == stock_n && stock_n == 3)) } else { ser_xml(kind, i, &m.series[i], &refs[i], &m.categories) })
        .collect();
    let dl = if m.data_labels { dlbls_xml(m.labels_percent && matches!(kind, ChartKind::Pie | ChartKind::Doughnut), matches!(kind, ChartKind::Pie | ChartKind::Doughnut)) } else { String::new() };
    let axids = format!("<c:axId val=\"{}\"/><c:axId val=\"{}\"/>", ax.0, ax.1);
    let grouping = |stacked_name: &str| match m.grouping {
        Grouping::Clustered => stacked_name.to_string(),
        Grouping::Stacked => "stacked".into(),
        Grouping::Percent => "percentStacked".into(),
    };
    match kind {
        ChartKind::Column | ChartKind::Bar => {
            let (gap, overlap) = if m.grouping == Grouping::Clustered { (219, -27) } else { (150, 100) };
            let (gap, overlap) = (m.gap_width.unwrap_or(gap), m.overlap.unwrap_or(overlap));
            format!(
                "<c:barChart><c:barDir val=\"{}\"/><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>{sers}{dl}<c:gapWidth val=\"{gap}\"/><c:overlap val=\"{overlap}\"/>{axids}</c:barChart>",
                if kind == ChartKind::Bar { "bar" } else { "col" },
                grouping("clustered")
            )
        }
        ChartKind::Line => format!("<c:lineChart><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>{sers}{dl}<c:marker val=\"1\"/>{axids}</c:lineChart>", grouping("standard")),
        ChartKind::Area => format!("<c:areaChart><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>{sers}{dl}{axids}</c:areaChart>", grouping("standard")),
        ChartKind::Pie => format!("<c:pieChart><c:varyColors val=\"1\"/>{sers}{dl}<c:firstSliceAng val=\"0\"/></c:pieChart>"),
        ChartKind::Doughnut => format!("<c:doughnutChart><c:varyColors val=\"1\"/>{sers}{dl}<c:firstSliceAng val=\"0\"/><c:holeSize val=\"{}\"/></c:doughnutChart>", m.hole.unwrap_or(50)),
        ChartKind::Scatter => format!("<c:scatterChart><c:scatterStyle val=\"lineMarker\"/><c:varyColors val=\"0\"/>{sers}{dl}{axids}</c:scatterChart>"),
        ChartKind::Bubble => format!(
            "<c:bubbleChart><c:varyColors val=\"0\"/>{sers}{dl}<c:bubbleScale val=\"{}\"/><c:showNegBubbles val=\"{}\"/>{}{axids}</c:bubbleChart>",
            m.bubble_scale.unwrap_or(100),
            m.show_negative_bubbles as u8,
            match m.size_represents.as_deref() {
                Some("width") => "<c:sizeRepresents val=\"w\"/>",
                _ => "",
            }
        ),
        ChartKind::Stock => {
            let ohlc = m.stock.as_deref().is_some_and(|s| s.ends_with("ohlc"));
            format!("<c:stockChart>{sers}{dl}{}{}{axids}</c:stockChart>", hi_low_lines(), if ohlc { up_down_bars() } else { String::new() })
        }
        ChartKind::Surface => {
            let (three_d, wire) = surface_flags(m);
            let name = if three_d { "c:surface3DChart" } else { "c:surfaceChart" };
            format!("<{name}><c:wireframe val=\"{}\"/>{sers}<c:axId val=\"{}\"/><c:axId val=\"{}\"/><c:axId val=\"{}\"/></{name}>", wire as u8, ax.0, ax.1, ax.1 + 1)
        }
        ChartKind::Radar => format!("<c:radarChart><c:radarStyle val=\"{}\"/><c:varyColors val=\"0\"/>{sers}{dl}{axids}</c:radarChart>", if m.filled { "filled" } else { "marker" }),
        ChartKind::Other => String::new(),
    }
}

/// Whether a surface chart is drawn in 3-D, and whether it is a wireframe.
pub fn surface_flags(m: &ChartModel) -> (bool, bool) {
    match m.surface.as_deref() {
        Some("wireframe") => (true, true),
        Some("contour") => (false, false),
        Some("wireframe_contour") => (false, true),
        _ => (true, false),
    }
}

/// The `c:view3D`, floor and walls of a surface chart.
pub fn view3d_xml(m: &ChartModel) -> String {
    let (three_d, _) = surface_flags(m);
    let (rx, ry, per) = match (&m.view3d, three_d) {
        (Some(v), _) => (v.rot_x, v.rot_y, v.perspective),
        (None, true) => (15, 20, 30),
        (None, false) => (90, 0, 0),
    };
    format!("<c:view3D><c:rotX val=\"{rx}\"/><c:rotY val=\"{ry}\"/><c:depthPercent val=\"100\"/><c:rAngAx val=\"0\"/><c:perspective val=\"{per}\"/></c:view3D><c:floor><c:thickness val=\"0\"/></c:floor><c:sideWall><c:thickness val=\"0\"/></c:sideWall><c:backWall><c:thickness val=\"0\"/></c:backWall>")
}

const GRID: &str = r#"<c:majorGridlines><c:spPr><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="15000"/><a:lumOff val="85000"/></a:schemeClr></a:solidFill><a:round/></a:ln></c:spPr></c:majorGridlines>"#;
const AXIS_LINE: &str = r#"<c:spPr><a:noFill/><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="15000"/><a:lumOff val="85000"/></a:schemeClr></a:solidFill><a:round/></a:ln></c:spPr>"#;
const NO_LINE: &str = r#"<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>"#;

fn scaling(min: Option<f64>, max: Option<f64>) -> String {
    format!("<c:scaling><c:orientation val=\"minMax\"/>{}{}</c:scaling>", max.map(|v| format!("<c:max val=\"{v}\"/>")).unwrap_or_default(), min.map(|v| format!("<c:min val=\"{v}\"/>")).unwrap_or_default())
}

/// The axes a kind of chart needs, with ids `ax`.
pub fn axes_xml(m: &ChartModel, kind: ChartKind, ax: (u32, u32)) -> String {
    let a = &m.axes;
    let num_fmt = if m.grouping == Grouping::Percent && kind.can_stack() {
        "<c:numFmt formatCode=\"0%\" sourceLinked=\"0\"/>".to_string()
    } else {
        match &a.number_format {
            Some(f) => format!("<c:numFmt formatCode=\"{}\" sourceLinked=\"0\"/>", esc(f)),
            None => "<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/>".to_string(),
        }
    };
    let title = |t: &Option<String>, rot: Option<i32>| t.as_deref().filter(|t| !t.is_empty()).map(|t| title_xml(t, 1000, rot)).unwrap_or_default();
    match kind {
        ChartKind::Pie | ChartKind::Doughnut | ChartKind::Other => String::new(),
        ChartKind::Scatter | ChartKind::Bubble => {
            let xt = title(&a.x_title, None);
            let yt = title(&a.y_title, Some(-5400000));
            format!(
                "<c:valAx><c:axId val=\"{x}\"/>{sx}<c:delete val=\"0\"/><c:axPos val=\"b\"/>{xt}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{AXIS_LINE}{tx}<c:crossAx val=\"{y}\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"midCat\"/></c:valAx><c:valAx><c:axId val=\"{y}\"/>{sy}<c:delete val=\"0\"/><c:axPos val=\"l\"/>{GRID}{yt}{num_fmt}<c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{NO_LINE}{tx}<c:crossAx val=\"{x}\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"midCat\"/></c:valAx>",
                x = ax.0,
                y = ax.1,
                sx = scaling(a.x_min, a.x_max),
                sy = scaling(a.y_min, a.y_max),
                tx = txpr(900, false)
            )
        }
        _ => {
            let horizontal = kind == ChartKind::Bar;
            let (cat_pos, val_pos) = if horizontal { ("l", "b") } else { ("b", "l") };
            let xt = title(&a.x_title, if horizontal { Some(-5400000) } else { None });
            let yt = title(&a.y_title, if horizontal { None } else { Some(-5400000) });
            let cross_between = if kind == ChartKind::Area || kind == ChartKind::Surface { "midCat" } else { "between" };
            let orientation = if a.x_reverse { "maxMin" } else { "minMax" };
            let cat_line = if kind == ChartKind::Radar { NO_LINE } else { AXIS_LINE };
            let ser_ax = if kind == ChartKind::Surface {
                format!("<c:serAx><c:axId val=\"{}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{}<c:crossAx val=\"{}\"/><c:crosses val=\"autoZero\"/></c:serAx>", ax.1 + 1, txpr(900, false), ax.1)
            } else {
                String::new()
            };
            format!(
                "<c:catAx><c:axId val=\"{c}\"/><c:scaling><c:orientation val=\"{orientation}\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"{cat_pos}\"/>{xt}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{cat_line}{tx}<c:crossAx val=\"{v}\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/><c:noMultiLvlLbl val=\"0\"/></c:catAx><c:valAx><c:axId val=\"{v}\"/>{sv}<c:delete val=\"0\"/><c:axPos val=\"{val_pos}\"/>{GRID}{yt}{num_fmt}<c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{NO_LINE}{tx}<c:crossAx val=\"{c}\"/><c:crosses val=\"{crosses}\"/><c:crossBetween val=\"{cross_between}\"/></c:valAx>{ser_ax}",
                c = ax.0,
                v = ax.1,
                sv = scaling(a.y_min, a.y_max),
                tx = txpr(900, false),
                crosses = if a.x_reverse && horizontal { "max" } else { "autoZero" }
            )
        }
    }
}

/// The second pair of axes of a combination chart (or the price axes of a stock chart with volume): a value axis on
/// the right and a hidden category axis, as Word writes them.
pub fn secondary_axes_xml(ax: (u32, u32)) -> String {
    format!(
        "<c:valAx><c:axId val=\"{v}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"r\"/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{NO_LINE}{tx}<c:crossAx val=\"{c}\"/><c:crosses val=\"max\"/><c:crossBetween val=\"between\"/></c:valAx><c:catAx><c:axId val=\"{c}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"1\"/><c:axPos val=\"b\"/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"{v}\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/><c:noMultiLvlLbl val=\"0\"/></c:catAx>",
        c = ax.0,
        v = ax.1,
        tx = txpr(900, false)
    )
}

pub fn legend_xml(position: &str) -> String {
    format!("<c:legend><c:legendPos val=\"{}\"/><c:overlay val=\"0\"/>{NO_LINE}{}</c:legend>", esc(position), txpr(900, false))
}

pub const AX_IDS: (u32, u32) = (506_450_856, 506_452_096);
/// The ids of the second pair of axes.
pub const AX_IDS_2: (u32, u32) = (506_450_857, 506_452_097);

/// The groups and axes of a plot area for `m`, in the order Word writes them.
pub fn plot_body(m: &ChartModel, refs: &[SeriesRefs]) -> String {
    let sp = specs(m);
    let mut groups = String::new();
    for s in &sp {
        let ids = if s.secondary { AX_IDS_2 } else { AX_IDS };
        groups.push_str(&group_xml(m, s.kind, &s.series, refs, ids));
    }
    let main_kind = sp.iter().find(|s| !s.secondary).map(|s| s.kind).unwrap_or(m.kind);
    let main_kind = if main_kind == ChartKind::Stock { ChartKind::Column } else { main_kind };
    let mut axes = axes_xml(m, main_kind, AX_IDS);
    if sp.iter().any(|s| s.secondary) {
        axes.push_str(&secondary_axes_xml(AX_IDS_2));
    }
    format!("{groups}{axes}")
}

/// A whole chart part for `m`. `refs` has one entry per series; `external` is the relationship id of the workbook.
pub fn chart_space(m: &ChartModel, refs: &[SeriesRefs], external: Option<&str>) -> XmlDoc {
    let body_plot = plot_body(m, refs);
    let title = m.title.as_deref().filter(|t| !t.is_empty()).map(|t| title_xml(t, 1400, None)).unwrap_or_default();
    let legend = m.legend.as_ref().map(|l| legend_xml(&l.position)).unwrap_or_default();
    let view3d = if m.kind == ChartKind::Surface { view3d_xml(m) } else { String::new() };
    let ext = external.map(|rid| format!("<c:externalData r:id=\"{rid}\"><c:autoUpdate val=\"0\"/></c:externalData>")).unwrap_or_default();
    let body = format!(
        "<c:chartSpace xmlns:c=\"{NS_C}\" xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\"><c:date1904 val=\"0\"/><c:lang val=\"en-US\"/><c:roundedCorners val=\"0\"/><c:chart>{title}<c:autoTitleDeleted val=\"{}\"/>{view3d}<c:plotArea><c:layout/>{body_plot}<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr></c:plotArea>{legend}<c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart><c:spPr><a:solidFill><a:schemeClr val=\"bg1\"/></a:solidFill><a:ln w=\"9525\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:noFill/><a:round/></a:ln></c:spPr><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr/></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>{ext}</c:chartSpace>",
        title.is_empty() as u8
    );
    xml::parse(body.as_bytes()).unwrap_or_else(|e| panic!("generated chart: {e}"))
}
