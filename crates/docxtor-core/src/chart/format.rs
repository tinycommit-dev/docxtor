//! Chart formatting written in place: axes, gridlines, labels, trendlines, error bars, colours, legend and the
//! chart style. `merge` turns an `options` object into the model the chart should have; `write` compares that
//! model with what the chart part says now and edits only the elements that differ, in the order the schema wants
//! them, so everything else in the part stays as it is.

use super::build::{self, frag};
use super::model::*;
use super::read::{locate_axes, series_order};
use crate::xml::{Element, Node, XmlDoc};
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// Schema order
// ---------------------------------------------------------------------------

pub const SER_ORDER: &[&str] = &["c:idx", "c:order", "c:tx", "c:spPr", "c:invertIfNegative", "c:pictureOptions", "c:marker", "c:explosion", "c:dPt", "c:dLbls", "c:trendline", "c:errBars", "c:cat", "c:val", "c:xVal", "c:yVal", "c:shape", "c:smooth", "c:bubbleSize", "c:bubble3D", "c:extLst"];
pub const SPPR_ORDER: &[&str] = &["a:xfrm", "a:custGeom", "a:prstGeom", "a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill", "a:ln", "a:effectLst", "a:effectDag", "a:scene3d", "a:sp3d", "a:extLst"];
const FILLS: &[&str] = &["a:noFill", "a:solidFill", "a:gradFill", "a:blipFill", "a:pattFill", "a:grpFill"];
pub const DLBLS_ORDER: &[&str] = &["c:dLbl", "c:delete", "c:numFmt", "c:spPr", "c:txPr", "c:dLblPos", "c:showLegendKey", "c:showVal", "c:showCatName", "c:showSerName", "c:showPercent", "c:showBubbleSize", "c:separator", "c:showLeaderLines", "c:leaderLines", "c:extLst"];
pub const TREND_ORDER: &[&str] = &["c:name", "c:spPr", "c:trendlineType", "c:order", "c:period", "c:forward", "c:backward", "c:intercept", "c:dispRSqr", "c:dispEq", "c:trendlineLbl", "c:extLst"];
pub const ERR_ORDER: &[&str] = &["c:errDir", "c:errBarType", "c:errValType", "c:noEndCap", "c:plus", "c:minus", "c:val", "c:spPr", "c:extLst"];
pub const GROUP_ORDER: &[&str] = &["c:barDir", "c:grouping", "c:scatterStyle", "c:radarStyle", "c:wireframe", "c:varyColors", "c:ser", "c:dLbls", "c:dropLines", "c:hiLowLines", "c:upDownBars", "c:marker", "c:smooth", "c:bubble3D", "c:gapWidth", "c:overlap", "c:serLines", "c:firstSliceAng", "c:holeSize", "c:bubbleScale", "c:showNegBubbles", "c:sizeRepresents", "c:bandFmts", "c:axId", "c:extLst"];
pub const AX_ORDER: &[&str] = &["c:axId", "c:scaling", "c:delete", "c:axPos", "c:majorGridlines", "c:minorGridlines", "c:title", "c:numFmt", "c:majorTickMark", "c:minorTickMark", "c:tickLblPos", "c:spPr", "c:txPr", "c:crossAx", "c:crosses", "c:crossesAt", "c:crossBetween", "c:auto", "c:lblAlgn", "c:lblOffset", "c:tickLblSkip", "c:tickMarkSkip", "c:noMultiLvlLbl", "c:majorUnit", "c:minorUnit", "c:dispUnits", "c:extLst"];
const SCALING_ORDER: &[&str] = &["c:logBase", "c:orientation", "c:max", "c:min", "c:extLst"];
const LEGEND_ORDER: &[&str] = &["c:legendPos", "c:legendEntry", "c:layout", "c:overlay", "c:spPr", "c:txPr", "c:extLst"];
const TITLE_ORDER: &[&str] = &["c:tx", "c:layout", "c:overlay", "c:spPr", "c:txPr", "c:extLst"];
const CHART_ORDER: &[&str] = &["c:title", "c:autoTitleDeleted", "c:pivotFmts", "c:view3D", "c:floor", "c:sideWall", "c:backWall", "c:plotArea", "c:legend", "c:plotVisOnly", "c:dispBlanksAs", "c:showDLblsOverMax", "c:extLst"];
const DPT_ORDER: &[&str] = &["c:idx", "c:invertIfNegative", "c:marker", "c:bubble3D", "c:explosion", "c:spPr", "c:pictureOptions", "c:extLst"];
const MARKER_ORDER: &[&str] = &["c:symbol", "c:size", "c:spPr", "c:extLst"];

fn rank(order: &[&str], name: &str) -> usize {
    order.iter().position(|n| *n == name).unwrap_or(order.len())
}

/// Put `new` into `parent`: in place of a child with the same name, else where the schema order says.
pub fn put_in(parent: &mut Element, new: Element, order: &[&str]) {
    if let Some(i) = parent.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == new.name)) {
        parent.children[i] = Node::Element(new);
        return;
    }
    let r = rank(order, &new.name);
    match parent.children.iter().position(|n| matches!(n, Node::Element(e) if rank(order, &e.name) > r)) {
        Some(i) => parent.children.insert(i, Node::Element(new)),
        None => parent.children.push(Node::Element(new)),
    }
}

fn set_val(parent: &mut Element, name: &'static str, v: &str, order: &[&str]) {
    match parent.child_mut(name) {
        Some(e) => e.set_attr("val", v),
        None => put_in(parent, Element::new(name).with_attr("val", v), order),
    }
}

fn num_str(v: f64) -> String {
    format!("{v}")
}

// ---------------------------------------------------------------------------
// Options: the model a chart should have
// ---------------------------------------------------------------------------

const TOP_KEYS: &[&str] = &["legend", "legendOverlay", "titleOverlay", "xAxis", "yAxis", "y2Axis", "gapWidth", "overlap", "bubbleScale", "sizeRepresents", "showNegativeBubbles", "stock", "surface", "view3d", "hole", "style", "palette", "look", "varyColors", "series", "dataLabels"];
const SERIES_KEYS: &[&str] = &["index", "color", "pointColors", "fill", "lineWidth", "lineColor", "labels", "trendlines", "errorBars", "marker", "smooth", "noLine", "kind", "secondary"];

fn camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = false;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// `snake_case` keys become `camelCase`, all the way down, so options can be written either way.
pub fn camel_keys(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (camel(k), camel_keys(v))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(camel_keys).collect()),
        other => other.clone(),
    }
}

/// RFC 7386: objects merge, `null` removes, anything else replaces.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(p) => {
            if !target.is_object() {
                *target = Value::Object(Map::new());
            }
            let t = target.as_object_mut().unwrap();
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else {
                    merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        other => *target = other.clone(),
    }
}

/// The chart `current` with the changes `options` asks for. Errors name what is wrong.
pub fn merge(current: &ChartModel, options: &Value) -> Result<ChartModel, String> {
    let opts = camel_keys(options);
    let Some(obj) = opts.as_object() else { return Err("options must be an object".into()) };
    // A look and a palette go first; the settings named next to them win over what they set.
    let mut start = current.clone();
    if let Some(n) = obj.get("look").and_then(|l| l.as_u64().map(|n| n as u32).or_else(|| l.as_str().and_then(|s| s.parse().ok()))) {
        super::palette::apply_look(&mut start, n)?;
    }
    if let Some(p) = obj.get("palette").and_then(|p| p.as_str()) {
        super::palette::apply_palette(&mut start, p)?;
    }
    let mut v = serde_json::to_value(&start).map_err(|e| e.to_string())?;
    for (k, pv) in obj {
        if !TOP_KEYS.contains(&k.as_str()) {
            return Err(format!("unknown chart option \"{k}\"; the options are {}", TOP_KEYS.join(", ")));
        }
        match k.as_str() {
            "look" | "palette" | "dataLabels" => {}
            "series" => {
                let Some(list) = pv.as_array() else { return Err("options.series must be a list of {index, ...}".into()) };
                for item in list {
                    let Some(io) = item.as_object() else { return Err("each entry of options.series must be an object".into()) };
                    let Some(idx) = io.get("index").and_then(|i| i.as_u64()).map(|i| i as usize) else { return Err("each entry of options.series needs an \"index\" (from 0)".into()) };
                    for key in io.keys() {
                        if !SERIES_KEYS.contains(&key.as_str()) {
                            return Err(format!("unknown series option \"{key}\"; the options are {}", SERIES_KEYS.join(", ")));
                        }
                    }
                    let Some(slot) = v.get_mut("series").and_then(|s| s.get_mut(idx)) else { return Err(format!("the chart has no series {idx}")) };
                    let mut patch = item.clone();
                    patch.as_object_mut().unwrap().remove("index");
                    merge_patch(slot, &patch);
                }
            }
            _ => {
                if pv.is_null() {
                    v.as_object_mut().unwrap().remove(k);
                } else {
                    let slot = v.as_object_mut().unwrap().entry(k.clone()).or_insert(Value::Null);
                    merge_patch(slot, pv);
                }
            }
        }
    }
    if let Some(d) = obj.get("dataLabels").and_then(|d| d.as_bool()) {
        v.as_object_mut().unwrap().insert("dataLabels".into(), Value::Bool(d));
    }
    let m: ChartModel = serde_json::from_value(v).map_err(|e| format!("options: {e}"))?;
    check(&m)?;
    Ok(m)
}

const LABEL_POS: &[&str] = &["ctr", "inEnd", "inBase", "outEnd", "bestFit", "t", "b", "l", "r"];
const TICKS: &[&str] = &["none", "in", "out", "cross"];
const TICK_LABELS: &[&str] = &["nextTo", "high", "low", "none"];
const UNITS: &[&str] = &["hundreds", "thousands", "tenThousands", "hundredThousands", "millions", "tenMillions", "hundredMillions", "billions", "trillions"];
const TRENDS: &[&str] = &["linear", "exp", "log", "poly", "power", "movingAvg"];

fn one_of(what: &str, v: &str, list: &[&str]) -> Result<(), String> {
    if list.contains(&v) {
        Ok(())
    } else {
        Err(format!("{what} \"{v}\" is not one of {}", list.join(", ")))
    }
}

fn check_axis(name: &str, a: &Axis) -> Result<(), String> {
    if let (Some(lo), Some(hi)) = (a.min, a.max) {
        if lo >= hi {
            return Err(format!("{name}: the minimum must be less than the maximum"));
        }
    }
    for (what, v) in [("major unit", a.major_unit), ("minor unit", a.minor_unit)] {
        if v.is_some_and(|v| v <= 0.0 || !v.is_finite()) {
            return Err(format!("{name}: the {what} must be more than 0"));
        }
    }
    if a.log_base.is_some_and(|b| !(2.0..=1000.0).contains(&b)) {
        return Err(format!("{name}: the logarithmic base must be between 2 and 1000"));
    }
    for t in [&a.major_tick, &a.minor_tick].into_iter().flatten() {
        one_of("tick mark", t, TICKS)?;
    }
    if let Some(p) = &a.label_pos {
        one_of("label position", p, TICK_LABELS)?;
    }
    if let Some(u) = &a.display_units {
        one_of("display unit", u, UNITS)?;
    }
    Ok(())
}

/// Reject settings the file format has no way to say.
pub fn check(m: &ChartModel) -> Result<(), String> {
    if let Some(l) = &m.legend {
        one_of("legend position", &l.position, &["r", "l", "t", "b", "tr"])?;
    }
    check_axis("category axis", &m.x_axis)?;
    check_axis("value axis", &m.y_axis)?;
    if let Some(a) = &m.y2_axis {
        check_axis("second value axis", a)?;
    }
    if let Some(s) = &m.stock {
        one_of("stock chart type", s, &["hlc", "ohlc", "vhlc", "vohlc"])?;
    }
    if let Some(s) = &m.surface {
        one_of("surface type", s, &["surface", "wireframe", "contour", "wireframe_contour"])?;
    }
    if let Some(s) = &m.size_represents {
        one_of("bubble size meaning", s, &["area", "width"])?;
    }
    if m.gap_width.is_some_and(|g| g > 500) {
        return Err("gap width is 0 to 500".into());
    }
    if m.overlap.is_some_and(|o| !(-100..=100).contains(&o)) {
        return Err("overlap is -100 to 100".into());
    }
    if m.bubble_scale.is_some_and(|b| b > 300) {
        return Err("bubble scale is 0 to 300".into());
    }
    for (i, s) in m.series.iter().enumerate() {
        let n = i + 1;
        if let Some(f) = &s.fill {
            one_of("fill", f, &["none", "soft", "gradient"])?;
        }
        if s.line_width.is_some_and(|w| !(0.0..=100.0).contains(&w)) {
            return Err(format!("series {n}: the line width is 0 to 100 points"));
        }
        if let Some(l) = &s.labels {
            if let Some(p) = &l.position {
                one_of("label position", p, LABEL_POS)?;
            }
        }
        for t in &s.trendlines {
            one_of("trendline type", &t.kind, TRENDS)?;
            if t.kind == "poly" && !t.order.is_none_or(|o| (2..=6).contains(&o)) {
                return Err(format!("series {n}: a polynomial trendline has an order of 2 to 6"));
            }
            if t.kind == "movingAvg" && t.period.is_some_and(|p| p < 2) {
                return Err(format!("series {n}: a moving average needs a period of at least 2"));
            }
        }
        for e in &s.error_bars {
            one_of("error bar direction", &e.dir, &["x", "y"])?;
            one_of("error bar type", &e.bar_type, &["both", "plus", "minus"])?;
            one_of("error value type", &e.value_type, &["fixed", "percentage", "stdDev", "stdErr", "custom"])?;
            if e.value_type == "custom" && e.plus.is_none() && e.minus.is_none() {
                return Err(format!("series {n}: custom error bars need plus or minus amounts"));
            }
            if e.value.is_some_and(|v| v < 0.0 || !v.is_finite()) {
                return Err(format!("series {n}: an error amount can't be negative"));
            }
        }
        if s.error_bars.len() > 2 {
            return Err(format!("series {n}: at most two sets of error bars"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Writing the differences
// ---------------------------------------------------------------------------

/// Edit the chart part so it says what `target` says, touching only what differs from `now` (the chart as read
/// from the part).
pub fn write(doc: &mut XmlDoc, target: &ChartModel, now: &ChartModel) -> Result<(), String> {
    let root = &mut doc.root;
    if target.style != now.style {
        if let Some(n) = target.style {
            set_style(root, n);
        }
    }
    let chart = root.child_mut("c:chart").ok_or("the chart has no c:chart")?;
    if target.title_overlay != now.title_overlay {
        if let Some(t) = chart.child_mut("c:title") {
            set_val(t, "c:overlay", if target.title_overlay { "1" } else { "0" }, TITLE_ORDER);
        }
    }
    write_legend(chart, target, now);
    let plot = chart.child_mut("c:plotArea").ok_or("the chart has no plot area")?;
    write_axes(plot, target, now);
    write_groups(plot, target, now);
    write_series(plot, target, now);
    Ok(())
}

/// Word's `c:style` (inside `mc:AlternateContent` with `c14:style` 100 higher), or a bare `c:style`.
pub fn set_style(root: &mut Element, n: i64) {
    let n = n.clamp(1, 48);
    if let Some(ac) = root.elements_mut().find(|e| e.name == "mc:AlternateContent") {
        let mut found = false;
        for branch in ac.elements_mut() {
            for s in branch.elements_mut() {
                match s.name.as_str() {
                    "c14:style" => {
                        s.set_attr("val", (100 + n).to_string());
                        found = true;
                    }
                    "c:style" => {
                        s.set_attr("val", n.to_string());
                        found = true;
                    }
                    _ => {}
                }
            }
        }
        if found {
            return;
        }
    }
    if let Some(s) = root.child_mut("c:style") {
        s.set_attr("val", n.to_string());
        return;
    }
    let ac = Element::new("mc:AlternateContent")
        .with_attr("xmlns:mc", "http://schemas.openxmlformats.org/markup-compatibility/2006")
        .with_child(Element::new("mc:Choice").with_attr("Requires", "c14").with_attr("xmlns:c14", "http://schemas.microsoft.com/office/drawing/2007/8/2/chart").with_child(Element::new("c14:style").with_attr("val", (100 + n).to_string())))
        .with_child(Element::new("mc:Fallback").with_child(Element::new("c:style").with_attr("val", n.to_string())));
    let order: &[&str] = &["c:date1904", "c:lang", "c:roundedCorners", "mc:AlternateContent", "c:style", "c:clrMapOvr", "c:pivotSource", "c:protection", "c:chart", "c:spPr", "c:txPr", "c:externalData", "c:printSettings", "c:userShapes", "c:extLst"];
    put_in(root, ac, order);
}

// -- legend ------------------------------------------------------------------------------------

fn write_legend(chart: &mut Element, target: &ChartModel, now: &ChartModel) {
    if target.legend == now.legend && target.legend_overlay == now.legend_overlay {
        return;
    }
    match &target.legend {
        None => {
            chart.remove_children("c:legend");
        }
        Some(l) => {
            if chart.child("c:legend").is_none() {
                put_in(chart, frag(&build::legend_xml(&l.position)), CHART_ORDER);
            }
            let Some(leg) = chart.child_mut("c:legend") else { return };
            set_val(leg, "c:legendPos", &l.position, LEGEND_ORDER);
            if target.legend_overlay != now.legend_overlay || now.legend.is_none() {
                set_val(leg, "c:overlay", if target.legend_overlay { "1" } else { "0" }, LEGEND_ORDER);
            }
        }
    }
}

// -- axes --------------------------------------------------------------------------------------

fn write_axes(plot: &mut Element, target: &ChartModel, now: &ChartModel) {
    let at = locate_axes(plot, now.kind);
    let pairs = [(at.x, Some(&target.x_axis), Some(&now.x_axis)), (at.y, Some(&target.y_axis), Some(&now.y_axis)), (at.y2, target.y2_axis.as_ref(), now.y2_axis.as_ref())];
    for (idx, t, n) in pairs {
        let (Some(i), Some(t), Some(n)) = (idx, t, n) else { continue };
        if t == n {
            continue;
        }
        if let Some(Node::Element(ax)) = plot.children.get_mut(i) {
            write_axis(ax, t, n);
        }
    }
}

const GRID_MAJOR: &str = r#"<c:majorGridlines><c:spPr><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="15000"/><a:lumOff val="85000"/></a:schemeClr></a:solidFill><a:round/></a:ln></c:spPr></c:majorGridlines>"#;
const GRID_MINOR: &str = r#"<c:minorGridlines><c:spPr><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="5000"/><a:lumOff val="95000"/></a:schemeClr></a:solidFill><a:round/></a:ln></c:spPr></c:minorGridlines>"#;

fn write_axis(ax: &mut Element, t: &Axis, n: &Axis) {
    let is_value = ax.name == "c:valAx";
    if t.hidden != n.hidden {
        set_val(ax, "c:delete", if t.hidden { "1" } else { "0" }, AX_ORDER);
    }
    // Scaling: log base, direction, ends.
    if t.reverse != n.reverse || t.min != n.min || t.max != n.max || t.log_base != n.log_base {
        let mut sc = ax.child("c:scaling").cloned().unwrap_or_else(|| Element::new("c:scaling").with_child(Element::new("c:orientation").with_attr("val", "minMax")));
        if t.reverse != n.reverse {
            set_val(&mut sc, "c:orientation", if t.reverse { "maxMin" } else { "minMax" }, SCALING_ORDER);
        }
        for (name, tv, nv) in [("c:min", t.min, n.min), ("c:max", t.max, n.max), ("c:logBase", t.log_base, n.log_base)] {
            if tv != nv {
                match tv {
                    Some(v) if is_value => set_val(&mut sc, name, &num_str(v), SCALING_ORDER),
                    _ => {
                        sc.remove_children(name);
                    }
                }
            }
        }
        put_in(ax, sc, AX_ORDER);
    }
    if is_value {
        for (name, tv, nv) in [("c:majorUnit", t.major_unit, n.major_unit), ("c:minorUnit", t.minor_unit, n.minor_unit)] {
            if tv != nv {
                match tv {
                    Some(v) => set_val(ax, name, &num_str(v), AX_ORDER),
                    None => {
                        ax.remove_children(name);
                    }
                }
            }
        }
        if t.display_units != n.display_units {
            match &t.display_units {
                Some(u) => put_in(ax, Element::new("c:dispUnits").with_child(Element::new("c:builtInUnit").with_attr("val", u.as_str())), AX_ORDER),
                None => {
                    ax.remove_children("c:dispUnits");
                }
            }
        }
    }
    if t.number_format != n.number_format {
        let el = match &t.number_format {
            Some(code) => Element::new("c:numFmt").with_attr("formatCode", code.as_str()).with_attr("sourceLinked", "0"),
            None => Element::new("c:numFmt").with_attr("formatCode", "General").with_attr("sourceLinked", "1"),
        };
        put_in(ax, el, AX_ORDER);
    }
    if t.major_grid != n.major_grid {
        if t.major_grid {
            put_in(ax, frag(GRID_MAJOR), AX_ORDER);
        } else {
            ax.remove_children("c:majorGridlines");
        }
    }
    if t.minor_grid != n.minor_grid {
        if t.minor_grid {
            put_in(ax, frag(GRID_MINOR), AX_ORDER);
        } else {
            ax.remove_children("c:minorGridlines");
        }
    }
    for (name, tv, nv) in [("c:majorTickMark", &t.major_tick, &n.major_tick), ("c:minorTickMark", &t.minor_tick, &n.minor_tick), ("c:tickLblPos", &t.label_pos, &n.label_pos)] {
        if tv != nv {
            match tv {
                Some(v) => set_val(ax, name, v, AX_ORDER),
                None => set_val(ax, name, if name == "c:tickLblPos" { "nextTo" } else { "none" }, AX_ORDER),
            }
        }
    }
    if t.title != n.title {
        set_axis_title(ax, t.title.as_deref().unwrap_or(""));
    }
}

/// An axis title: new words keep the existing title's formatting; a missing title is added in Word's style.
pub fn set_axis_title(ax: &mut Element, text: &str) {
    if text.is_empty() {
        ax.remove_children("c:title");
        return;
    }
    let vertical = matches!(ax.child("c:axPos").and_then(|p| p.attr("val")), Some("l") | Some("r"));
    match ax.child_mut("c:title") {
        Some(old) => {
            let mut tmp = Element::new("c:chart");
            tmp.children.push(Node::Element(old.clone()));
            super::write::set_title(&mut tmp, text);
            if let Some(Node::Element(new)) = tmp.children.into_iter().find(|n| matches!(n, Node::Element(e) if e.name == "c:title")) {
                *old = new;
            }
        }
        None => put_in(ax, frag(&build::title_xml(text, 1000, if vertical { Some(-5400000) } else { None })), AX_ORDER),
    }
}

// -- groups ------------------------------------------------------------------------------------

fn write_groups(plot: &mut Element, target: &ChartModel, now: &ChartModel) {
    for g in plot.elements_mut() {
        match g.name.as_str() {
            "c:barChart" | "c:bar3DChart" => {
                if target.gap_width != now.gap_width {
                    if let Some(v) = target.gap_width {
                        set_val(g, "c:gapWidth", &v.to_string(), GROUP_ORDER);
                    }
                }
                if target.overlap != now.overlap {
                    if let Some(v) = target.overlap {
                        set_val(g, "c:overlap", &v.to_string(), GROUP_ORDER);
                    }
                }
            }
            "c:bubbleChart" => {
                if target.bubble_scale != now.bubble_scale {
                    if let Some(v) = target.bubble_scale {
                        set_val(g, "c:bubbleScale", &v.to_string(), GROUP_ORDER);
                    }
                }
                if target.show_negative_bubbles != now.show_negative_bubbles {
                    set_val(g, "c:showNegBubbles", if target.show_negative_bubbles { "1" } else { "0" }, GROUP_ORDER);
                }
                if target.size_represents != now.size_represents {
                    match target.size_represents.as_deref() {
                        Some("width") => set_val(g, "c:sizeRepresents", "w", GROUP_ORDER),
                        _ => {
                            g.remove_children("c:sizeRepresents");
                        }
                    }
                }
            }
            "c:doughnutChart" => {
                if target.hole != now.hole {
                    if let Some(h) = target.hole {
                        set_val(g, "c:holeSize", &h.clamp(10, 90).to_string(), GROUP_ORDER);
                    }
                }
            }
            _ => {}
        }
        if target.vary_colors != now.vary_colors && !matches!(g.name.as_str(), "c:pieChart" | "c:pie3DChart" | "c:doughnutChart") && g.child("c:varyColors").is_some() {
            set_val(g, "c:varyColors", if target.vary_colors { "1" } else { "0" }, GROUP_ORDER);
        }
    }
}

// -- series ------------------------------------------------------------------------------------

fn write_series(plot: &mut Element, target: &ChartModel, now: &ChartModel) {
    for (i, (gi, si)) in series_order(plot).into_iter().enumerate() {
        let (Some(t), Some(n)) = (target.series.get(i), now.series.get(i)) else { continue };
        let grouping = target.grouping;
        let Some(Node::Element(g)) = plot.children.get_mut(gi) else { continue };
        let Some(Node::Element(ser)) = g.children.get_mut(si) else { continue };
        write_one_series(ser, i, t, n, grouping);
    }
}

fn trim(list: &[Option<String>]) -> &[Option<String>] {
    let mut end = list.len();
    while end > 0 && list[end - 1].is_none() {
        end -= 1;
    }
    &list[..end]
}

fn write_one_series(ser: &mut Element, i: usize, t: &ChartSeries, n: &ChartSeries, grouping: Grouping) {
    let kind = n.kind;
    if (&t.color, &t.fill, t.line_width, &t.line_color) != (&n.color, &n.fill, n.line_width, &n.line_color) {
        write_look(ser, kind, i, t);
    }
    if trim(&t.point_colors) != trim(&n.point_colors) {
        write_point_colors(ser, kind, i, t, n);
    }
    if t.marker != n.marker && matches!(kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) {
        write_marker(ser, i, t);
    }
    if t.smooth != n.smooth && matches!(kind, ChartKind::Line | ChartKind::Scatter) {
        set_val(ser, "c:smooth", if t.smooth { "1" } else { "0" }, SER_ORDER);
    }
    if t.no_line != n.no_line && matches!(kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) {
        write_no_line(ser, i, t);
    }
    if t.labels != n.labels {
        write_labels(ser, kind, grouping, t.labels.as_ref(), n.labels.as_ref(), matches!(kind, ChartKind::Pie | ChartKind::Doughnut));
    }
    if t.trendlines != n.trendlines {
        let trend_ok = matches!(kind, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble) && (grouping == Grouping::Clustered || matches!(kind, ChartKind::Scatter | ChartKind::Bubble));
        write_trendlines(ser, if trend_ok { &t.trendlines } else { &[] }, i, t);
    }
    if t.error_bars != n.error_bars {
        let ok = matches!(kind, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble);
        write_error_bars(ser, if ok { &t.error_bars } else { &[] }, matches!(kind, ChartKind::Scatter | ChartKind::Bubble));
    }
}

fn ensure_sppr(ser: &mut Element) -> &mut Element {
    if ser.child("c:spPr").is_none() {
        put_in(ser, Element::new("c:spPr"), SER_ORDER);
    }
    ser.child_mut("c:spPr").unwrap()
}

/// Replace the fill of a shape-properties element.
fn set_fill(sp: &mut Element, fill: Element) {
    sp.children.retain(|n| !matches!(n, Node::Element(e) if FILLS.contains(&e.name.as_str())));
    put_in(sp, fill, SPPR_ORDER);
}

/// Bring an `a:ln` to say `want`: its width, and its fill. Other things in the line (dashes, joins) stay.
fn merge_line(sp: &mut Element, want: &Element) {
    match sp.child_mut("a:ln") {
        Some(ln) => {
            match want.attr("w") {
                Some(w) => ln.set_attr("w", w),
                None => {
                    ln.remove_attr("w");
                }
            }
            ln.children.retain(|n| !matches!(n, Node::Element(e) if FILLS.contains(&e.name.as_str())));
            for f in want.elements().filter(|e| FILLS.contains(&e.name.as_str())).cloned() {
                ln.children.insert(0, Node::Element(f));
            }
        }
        None => put_in(sp, want.clone(), SPPR_ORDER),
    }
}

/// The colour, fill and outline of a series.
fn write_look(ser: &mut Element, kind: ChartKind, i: usize, s: &ChartSeries) {
    match kind {
        ChartKind::Column | ChartKind::Bar | ChartKind::Area | ChartKind::Bubble => {
            let sp = ensure_sppr(ser);
            let fill = if kind == ChartKind::Bubble && s.fill.is_none() { frag(&format!("<a:solidFill>{}</a:solidFill>", build::clr(s.color.as_deref(), i, "<a:alpha val=\"50000\"/>"))) } else { frag(&build::fill_xml(s, i)) };
            set_fill(sp, fill);
            let ln = if kind == ChartKind::Bubble { frag(&format!("<a:ln w=\"{}\"><a:solidFill>{}</a:solidFill></a:ln>", (s.line_width.unwrap_or(2.0) * 12700.0).round() as i64, build::color_xml(s.line_color.as_deref().or(s.color.as_deref()), i))) } else { frag(&build::outline_xml(s, i)) };
            merge_line(sp, &ln);
        }
        ChartKind::Line | ChartKind::Scatter | ChartKind::Radar | ChartKind::Stock => {
            let marker_color = build::color_xml(s.color.as_deref(), i);
            let sp = ensure_sppr(ser);
            let w = s.line_width.map(|w| format!(" w=\"{}\"", (w * 12700.0).round() as i64)).unwrap_or_default();
            let fill = if s.no_line { "<a:noFill/>".to_string() } else { format!("<a:solidFill>{marker_color}</a:solidFill>") };
            merge_line(sp, &frag(&format!("<a:ln{w}>{fill}</a:ln>")));
            if let Some(mk) = ser.child_mut("c:marker") {
                if mk.child("c:symbol").and_then(|s| s.attr("val")) != Some("none") {
                    let msp = frag(&format!("<c:spPr><a:solidFill>{marker_color}</a:solidFill><a:ln w=\"9525\"><a:solidFill>{marker_color}</a:solidFill></a:ln></c:spPr>"));
                    put_in(mk, msp, MARKER_ORDER);
                }
            }
        }
        _ => {}
    }
}

fn point_fill(kind: ChartKind, color: &str, i: usize) -> Element {
    match kind {
        ChartKind::Pie | ChartKind::Doughnut => frag(&format!("<c:spPr><a:solidFill>{}</a:solidFill><a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"lt1\"/></a:solidFill></a:ln></c:spPr>", build::clr(Some(color), i, ""))),
        _ => frag(&format!("<c:spPr><a:solidFill>{}</a:solidFill><a:ln><a:noFill/></a:ln></c:spPr>", build::clr(Some(color), i, ""))),
    }
}

/// Colours of single points (slices, varied bars).
fn write_point_colors(ser: &mut Element, kind: ChartKind, _series: usize, t: &ChartSeries, n: &ChartSeries) {
    let count = t.point_colors.len().max(n.point_colors.len());
    for idx in 0..count {
        let want = t.point_colors.get(idx).cloned().flatten();
        let had = n.point_colors.get(idx).cloned().flatten();
        if want == had {
            continue;
        }
        let pos = ser.children.iter().position(|nd| matches!(nd, Node::Element(e) if e.name == "c:dPt" && e.child("c:idx").and_then(|x| x.attr("val")) == Some(idx.to_string().as_str())));
        match (want, pos) {
            (Some(c), Some(p)) => {
                if let Node::Element(d) = &mut ser.children[p] {
                    d.children.retain(|nd| !matches!(nd, Node::Element(e) if e.name == "c:spPr"));
                    put_in(d, point_fill(kind, &c, idx), DPT_ORDER);
                }
            }
            (Some(c), None) => {
                let mut d = Element::new("c:dPt").with_child(Element::new("c:idx").with_attr("val", idx.to_string()));
                if matches!(kind, ChartKind::Column | ChartKind::Bar) {
                    d.children.push(Node::Element(Element::new("c:invertIfNegative").with_attr("val", "0")));
                }
                d.children.push(Node::Element(Element::new("c:bubble3D").with_attr("val", "0")));
                put_in(&mut d, point_fill(kind, &c, idx), DPT_ORDER);
                // Among the other points, by index.
                let at = ser.children.iter().position(|nd| matches!(nd, Node::Element(e) if e.name == "c:dPt" && e.child("c:idx").and_then(|x| x.attr("val")).and_then(|v| v.parse::<usize>().ok()).is_some_and(|v| v > idx)));
                match at {
                    Some(a) => ser.children.insert(a, Node::Element(d)),
                    None => put_dpt_last(ser, d),
                }
            }
            (None, Some(p)) => {
                ser.children.remove(p);
            }
            (None, None) => {}
        }
    }
}

fn put_dpt_last(ser: &mut Element, d: Element) {
    let last = ser.children.iter().rposition(|nd| matches!(nd, Node::Element(e) if e.name == "c:dPt"));
    match last {
        Some(l) => ser.children.insert(l + 1, Node::Element(d)),
        None => put_in(ser, d, SER_ORDER),
    }
}

fn write_marker(ser: &mut Element, i: usize, s: &ChartSeries) {
    if s.marker {
        let color = build::color_xml(s.color.as_deref(), i);
        match ser.child_mut("c:marker") {
            Some(mk) => {
                set_val(mk, "c:symbol", "circle", MARKER_ORDER);
                if mk.child("c:size").is_none() {
                    set_val(mk, "c:size", "5", MARKER_ORDER);
                }
                if mk.child("c:spPr").is_none() {
                    put_in(mk, frag(&format!("<c:spPr><a:solidFill>{color}</a:solidFill><a:ln w=\"9525\"><a:solidFill>{color}</a:solidFill></a:ln></c:spPr>")), MARKER_ORDER);
                }
            }
            None => put_in(ser, frag(&build::marker_xml(s, i)), SER_ORDER),
        }
    } else {
        put_in(ser, frag("<c:marker><c:symbol val=\"none\"/></c:marker>"), SER_ORDER);
    }
}

fn write_no_line(ser: &mut Element, i: usize, s: &ChartSeries) {
    let color = build::color_xml(s.color.as_deref(), i);
    let sp = ensure_sppr(ser);
    let fill = if s.no_line { "<a:noFill/>".to_string() } else { format!("<a:solidFill>{color}</a:solidFill>") };
    merge_line(sp, &frag(&format!("<a:ln>{fill}</a:ln>")));
}

// -- data labels -------------------------------------------------------------------------------

/// The label positions the file format allows for a kind of series; anything else makes Word refuse the file.
pub fn label_positions(kind: ChartKind, grouping: Grouping) -> &'static [&'static str] {
    match kind {
        ChartKind::Column | ChartKind::Bar => {
            if grouping == Grouping::Clustered {
                &["ctr", "inEnd", "inBase", "outEnd"]
            } else {
                &["ctr", "inEnd", "inBase"]
            }
        }
        ChartKind::Line | ChartKind::Scatter | ChartKind::Radar | ChartKind::Bubble => &["t", "b", "l", "r", "ctr"],
        ChartKind::Pie => &["bestFit", "ctr", "inEnd", "outEnd"],
        _ => &[],
    }
}

fn flag_el(name: &'static str, on: bool) -> Element {
    Element::new(name).with_attr("val", if on { "1" } else { "0" })
}

fn write_labels(ser: &mut Element, kind: ChartKind, grouping: Grouping, t: Option<&DataLabels>, n: Option<&DataLabels>, pie: bool) {
    let Some(t) = t else {
        // Off for this series, whatever the group says.
        put_in(ser, frag("<c:dLbls><c:delete val=\"1\"/></c:dLbls>"), SER_ORDER);
        return;
    };
    if ser.child("c:dLbls").is_none_or(|d| d.child("c:delete").is_some()) {
        put_in(ser, frag(&format!("<c:dLbls><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}</c:dLbls>", build::txpr(900, false))), SER_ORDER);
    }
    let d = ser.child_mut("c:dLbls").unwrap();
    d.remove_children("c:delete");
    let def = DataLabels::default();
    let n = n.unwrap_or(&def);
    let flags = [("c:showLegendKey", t.legend_key, n.legend_key), ("c:showVal", t.value, n.value), ("c:showCatName", t.category, n.category), ("c:showSerName", t.series, n.series), ("c:showPercent", t.percent, n.percent), ("c:showBubbleSize", t.bubble_size, n.bubble_size)];
    for (name, tv, nv) in flags {
        if tv != nv || d.child(name).is_none() {
            put_in(d, flag_el(name, tv), DLBLS_ORDER);
        }
    }
    if t.position != n.position || d.child("c:dLblPos").is_some_and(|p| !label_positions(kind, grouping).contains(&p.attr("val").unwrap_or(""))) {
        match t.position.as_deref().filter(|p| label_positions(kind, grouping).contains(p)) {
            Some(p) => set_val(d, "c:dLblPos", p, DLBLS_ORDER),
            None => {
                d.remove_children("c:dLblPos");
            }
        }
    }
    if t.separator != n.separator {
        match &t.separator {
            Some(s) => put_in(d, Element::new("c:separator").with_text(s.as_str()), DLBLS_ORDER),
            None => {
                d.remove_children("c:separator");
            }
        }
    }
    if t.number_format != n.number_format {
        match &t.number_format {
            Some(code) => put_in(d, Element::new("c:numFmt").with_attr("formatCode", code.as_str()).with_attr("sourceLinked", "0"), DLBLS_ORDER),
            None => {
                d.remove_children("c:numFmt");
            }
        }
    }
    if pie && d.child("c:showLeaderLines").is_none() {
        put_in(d, flag_el("c:showLeaderLines", true), DLBLS_ORDER);
    }
    // Individual labels that were set by hand follow the series' settings now.
    if t != n {
        d.remove_children("c:dLbl");
    }
}

// -- trendlines --------------------------------------------------------------------------------

fn write_trendlines(ser: &mut Element, want: &[Trendline], i: usize, s: &ChartSeries) {
    let old: Vec<Element> = ser.children_named("c:trendline").cloned().collect();
    ser.remove_children("c:trendline");
    for (j, t) in want.iter().enumerate() {
        let mut el = old.get(j).cloned().unwrap_or_else(|| {
            let color = build::color_xml(s.color.as_deref(), i);
            frag(&format!("<c:trendline><c:spPr><a:ln w=\"19050\" cap=\"rnd\"><a:solidFill>{color}</a:solidFill><a:prstDash val=\"sysDot\"/></a:ln></c:spPr><c:trendlineType val=\"linear\"/><c:dispRSqr val=\"0\"/><c:dispEq val=\"0\"/></c:trendline>"))
        });
        match &t.name {
            Some(name) => put_in(&mut el, Element::new("c:name").with_text(name.as_str()), TREND_ORDER),
            None => {
                el.remove_children("c:name");
            }
        }
        set_val(&mut el, "c:trendlineType", &t.kind, TREND_ORDER);
        for (name, v) in [("c:order", t.order.filter(|_| t.kind == "poly").map(f64::from)), ("c:period", t.period.filter(|_| t.kind == "movingAvg").map(f64::from)), ("c:forward", t.forward), ("c:backward", t.backward), ("c:intercept", t.intercept)] {
            match v {
                Some(v) => set_val(&mut el, name, &num_str(v), TREND_ORDER),
                None => {
                    el.remove_children(name);
                }
            }
        }
        if t.kind == "poly" && t.order.is_none() {
            set_val(&mut el, "c:order", "2", TREND_ORDER);
        }
        if t.kind == "movingAvg" && t.period.is_none() {
            set_val(&mut el, "c:period", "2", TREND_ORDER);
        }
        set_val(&mut el, "c:dispRSqr", if t.r_squared { "1" } else { "0" }, TREND_ORDER);
        set_val(&mut el, "c:dispEq", if t.equation { "1" } else { "0" }, TREND_ORDER);
        if !t.equation && !t.r_squared {
            el.remove_children("c:trendlineLbl");
        }
        put_multi(ser, el, SER_ORDER);
    }
}

/// Like `put_in` for elements that repeat (trendlines, error bars): after the ones already there.
fn put_multi(parent: &mut Element, el: Element, order: &[&str]) {
    if let Some(last) = parent.children.iter().rposition(|n| matches!(n, Node::Element(e) if e.name == el.name)) {
        parent.children.insert(last + 1, Node::Element(el));
        return;
    }
    let r = rank(order, &el.name);
    match parent.children.iter().position(|n| matches!(n, Node::Element(e) if rank(order, &e.name) > r)) {
        Some(i) => parent.children.insert(i, Node::Element(el)),
        None => parent.children.push(Node::Element(el)),
    }
}

// -- error bars --------------------------------------------------------------------------------

fn lit(values: &[f64]) -> String {
    let pts: String = values.iter().enumerate().map(|(i, v)| format!("<c:pt idx=\"{i}\"><c:v>{v}</c:v></c:pt>")).collect();
    format!("<c:numLit><c:formatCode>General</c:formatCode><c:ptCount val=\"{}\"/>{pts}</c:numLit>", values.len())
}

fn write_error_bars(ser: &mut Element, want: &[ErrorBars], xy: bool) {
    let old: Vec<Element> = ser.children_named("c:errBars").cloned().collect();
    ser.remove_children("c:errBars");
    for (j, e) in want.iter().take(if xy { 2 } else { 1 }).enumerate() {
        let mut el = old.get(j).cloned().unwrap_or_else(|| {
            frag(r#"<c:errBars><c:errDir val="y"/><c:errBarType val="both"/><c:errValType val="fixedVal"/><c:noEndCap val="0"/><c:val val="1"/><c:spPr><a:noFill/><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill><a:round/></a:ln></c:spPr></c:errBars>"#)
        });
        set_val(&mut el, "c:errDir", if xy { &e.dir } else { "y" }, ERR_ORDER);
        set_val(&mut el, "c:errBarType", &e.bar_type, ERR_ORDER);
        let vt = match e.value_type.as_str() {
            "custom" => "cust",
            "fixed" => "fixedVal",
            other => other,
        };
        set_val(&mut el, "c:errValType", vt, ERR_ORDER);
        set_val(&mut el, "c:noEndCap", if e.no_cap { "1" } else { "0" }, ERR_ORDER);
        match e.value_type.as_str() {
            "custom" => {
                el.remove_children("c:val");
                let zeros = |n: usize| vec![0.0; n];
                let len = e.plus.as_ref().map(|p| p.len()).max(e.minus.as_ref().map(|m| m.len())).unwrap_or(0);
                let plus = e.plus.clone().unwrap_or_else(|| zeros(len));
                let minus = e.minus.clone().unwrap_or_else(|| zeros(len));
                if e.bar_type != "minus" {
                    put_in(&mut el, frag(&format!("<c:plus>{}</c:plus>", lit(&plus))), ERR_ORDER);
                } else {
                    el.remove_children("c:plus");
                }
                if e.bar_type != "plus" {
                    put_in(&mut el, frag(&format!("<c:minus>{}</c:minus>", lit(&minus))), ERR_ORDER);
                } else {
                    el.remove_children("c:minus");
                }
            }
            "stdDev" | "stdErr" => {
                el.remove_children("c:plus");
                el.remove_children("c:minus");
                match e.value {
                    Some(v) if e.value_type == "stdDev" => set_val(&mut el, "c:val", &num_str(v), ERR_ORDER),
                    _ => {
                        el.remove_children("c:val");
                    }
                }
            }
            _ => {
                el.remove_children("c:plus");
                el.remove_children("c:minus");
                set_val(&mut el, "c:val", &num_str(e.value.unwrap_or(if e.value_type == "percentage" { 5.0 } else { 1.0 })), ERR_ORDER);
            }
        }
        put_multi(ser, el, SER_ORDER);
    }
}
