//! AI tools for charts. They live apart from `ai` and are merged into its tool list and executor.

use crate::ai::{find_in_para, id_of, script_for_paragraphs};
use crate::chart::{ChartKind, Grouping, SeriesIn};
use crate::document::Document;
use crate::ops::{Id, Op, Pos};
use serde_json::{json, Value};

fn series_schema() -> Value {
    json!({
        "type": "array",
        "description": "One entry per series: {\"name\":\"Revenue\",\"values\":[10,12,null,15]} with one value per category (null is a gap). Scatter and bubble charts also take \"x_values\"; bubble charts \"sizes\" (the bubble sizes). In a combination chart a series can have its own \"kind\" (column, bar, line, area or scatter) and \"secondary\": true to use the second value axis. Stock charts take their series in the order of the type: hlc = high, low, close; ohlc = open, high, low, close; vhlc = volume, high, low, close; vohlc = volume, open, high, low, close. Surface charts: one series per row of the grid, one value per category.",
        "items": {"type": "object", "required": ["values"], "properties": {
            "name": {"type": "string"},
            "values": {"type": "array", "items": {"type": ["number", "null"]}},
            "x_values": {"type": "array", "items": {"type": ["number", "null"]}},
            "sizes": {"type": "array", "items": {"type": ["number", "null"]}},
            "kind": {"type": "string", "enum": ["column", "bar", "line", "area", "scatter"]},
            "secondary": {"type": "boolean"}
        }}
    })
}

/// What `options` can say. Shown in both tools.
const OPTIONS_DOC: &str = "Formatting, as an object that is merged into the chart's current formatting (null removes a setting; read the chart first). Keys (camelCase or snake_case): \
legend {position: r|l|t|b|tr} (null removes it), legendOverlay, titleOverlay; \
xAxis (the category axis; the X axis of scatter and bubble charts), yAxis (the value axis), y2Axis (the second value axis, when a series is secondary): each {title, hidden, reverse, min, max, majorUnit, minorUnit, logBase, numberFormat (Excel format code; absent follows the data), majorGrid, minorGrid, majorTick/minorTick (none|in|out|cross), labelPos (nextTo|low|high|none), displayUnits (hundreds|thousands|millions|billions|...)}; \
gapWidth, overlap (bars); bubbleScale, sizeRepresents (area|width), showNegativeBubbles; stock (hlc|ohlc|vhlc|vohlc); surface (surface|wireframe|contour|wireframe_contour); \
series: [{index, color (RRGGBB, or scheme:accent2 / scheme:accent2:lumMod=60000:lumOff=40000 for a theme colour), pointColors (slices or varied bars), fill (none|soft|gradient), lineWidth (points), lineColor, marker, smooth, noLine, kind, secondary, \
labels {value, category, series, percent, legendKey, bubbleSize, position (ctr|inEnd|inBase|outEnd|bestFit|t|b|l|r, as the chart type allows), separator, numberFormat} (null removes them), \
trendlines [{type: linear|exp|log|poly|power|movingAvg, order (poly 2-6), period (movingAvg), forward, backward, intercept, equation, rSquared, name}], \
errorBars [{dir: y|x, barType: both|plus|minus, valueType: fixed|percentage|stdDev|stdErr|custom, value, plus [..], minus [..], noCap}]}]; \
look 1-8 (the Style gallery: fills, outlines, gaps, gridlines); palette colorful1-colorful4 or mono1-mono13 (the Change colors gallery; sets every series' colour from the theme).";

/// Added to the tool list.
pub fn definitions() -> Vec<Value> {
    let kinds = json!(["column", "bar", "line", "pie", "doughnut", "area", "scatter", "radar", "bubble", "stock", "surface"]);
    vec![
        json!({
            "name": "insert_chart",
            "description": "Insert a real Word chart (it stays editable in Word, with its data in an embedded workbook). Besides the basic kinds there are bubble (x_values, values and sizes per series), stock (high-low-close and open-high-low-close, with or without volume: options.stock) and surface (options.surface), and combination charts (a kind and secondary axis per series). Put it after a block (`after`, in a paragraph of its own), before a block (`before`), or inline after some text (`id` + `after_text`). column = vertical bars, bar = horizontal bars. Give the data as categories (the labels along the axis, or the slices of a pie) and series. The chart appears in read_document as [chart p12@3: ...] with its data as a table.",
            "input_schema": {"type": "object", "required": ["kind", "series"], "properties": {
                "after": {"type": "string", "description": "Block id to put the chart after"},
                "before": {"type": "string", "description": "Block id to put the chart before"},
                "id": {"type": "string", "description": "Paragraph id for an inline chart (with after_text)"},
                "after_text": {"type": "string", "description": "Exact text the inline chart follows (first occurrence); default is the end of the paragraph"},
                "kind": {"type": "string", "enum": kinds},
                "title": {"type": "string"},
                "categories": {"type": "array", "items": {"type": "string"}, "description": "Not used by scatter charts"},
                "series": series_schema(),
                "grouping": {"type": "string", "enum": ["clustered", "stacked", "percent"], "description": "Column, bar, line and area charts (default clustered)"},
                "legend": {"type": "string", "enum": ["none", "right", "left", "top", "bottom", "top_right"], "description": "Default: bottom for several series, right for pie, none for one series"},
                "data_labels": {"type": "boolean", "description": "Show the values on the chart (percentages on a pie)"},
                "width": {"type": "number", "description": "Points (default 432, six inches)"},
                "height": {"type": "number", "description": "Points (default 252)"},
                "alt": {"type": "string", "description": "Alt text; by default a summary of the chart"},
                "options": {"type": "object", "description": OPTIONS_DOC}
            }}
        }),
        json!({
            "name": "set_chart",
            "description": "Change a chart: its type, title (\"\" removes it), data, legend, data labels, stacking, and with `options` its axes (titles, scale, gridlines, number formats), series colours, labels, trendlines, error bars and looks. Pass only what changes; passing `series` or `categories` replaces ALL the data, so send the full table (read it first in read_document). The chart's embedded workbook is updated to match (only the cells the chart uses change), and colours and fonts stay. Find the chart's paragraph id in read_document: [chart p12@3: ...].",
            "input_schema": {"type": "object", "required": ["id"], "properties": {
                "id": {"type": "string", "description": "Id of the paragraph that holds the chart"},
                "index": {"type": "integer", "description": "Which chart in the paragraph, from 0 (default 0)"},
                "kind": {"type": "string", "enum": kinds},
                "title": {"type": "string"},
                "categories": {"type": "array", "items": {"type": "string"}},
                "series": series_schema(),
                "grouping": {"type": "string", "enum": ["clustered", "stacked", "percent"]},
                "legend": {"type": "string", "enum": ["none", "right", "left", "top", "bottom", "top_right"]},
                "data_labels": {"type": "boolean"},
                "x_title": {"type": "string", "description": "Title of the category axis"},
                "y_title": {"type": "string", "description": "Title of the value axis"},
                "options": {"type": "object", "description": OPTIONS_DOC}
            }}
        }),
    ]
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}

fn kind_of(args: &Value) -> Result<Option<ChartKind>, String> {
    match s(args, "kind") {
        None => Ok(None),
        Some(k) => ChartKind::parse(&k).map(Some).ok_or_else(|| format!("unknown chart kind {k:?}: use column, bar, line, pie, doughnut, area, scatter, radar, bubble, stock or surface")),
    }
}

fn grouping_of(args: &Value) -> Result<Option<Grouping>, String> {
    match s(args, "grouping") {
        None => Ok(None),
        Some(g) => Grouping::parse(&g).map(Some).ok_or_else(|| format!("unknown grouping {g:?}: use clustered, stacked or percent")),
    }
}

fn series_of(args: &Value) -> Result<Option<Vec<SeriesIn>>, String> {
    match args.get("series") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|e| format!("series: {e}")),
    }
}

fn categories_of(args: &Value) -> Result<Option<Vec<String>>, String> {
    match args.get("categories") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => Ok(Some(
            a.iter()
                .map(|c| match c {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                })
                .collect(),
        )),
        Some(_) => Err("categories must be a list of text".into()),
    }
}

/// Run one of these tools; None when the name isn't one of them.
pub fn call(doc: &mut Document, name: &str, args: &Value) -> Option<Result<String, String>> {
    Some(match name {
        "insert_chart" => insert_chart(doc, args),
        "set_chart" => set_chart(doc, args),
        _ => return None,
    })
}

fn insert_chart(doc: &mut Document, args: &Value) -> Result<String, String> {
    let kind = kind_of(args)?.ok_or("kind is required: column, bar, line, pie, doughnut, area, scatter, radar, bubble, stock or surface")?;
    let series = series_of(args)?.ok_or("series is required")?;
    let mk = |at: Pos| Op::InsertChart {
        at,
        kind,
        title: s(args, "title"),
        categories: categories_of(args).ok().flatten().unwrap_or_default(),
        series: series.clone(),
        grouping: grouping_of(args).ok().flatten(),
        legend: s(args, "legend"),
        data_labels: args.get("data_labels").and_then(|v| v.as_bool()),
        width: args.get("width").and_then(|v| v.as_f64()),
        height: args.get("height").and_then(|v| v.as_f64()),
        alt: s(args, "alt"),
        options: args.get("options").filter(|v| !v.is_null()).cloned(),
    };
    categories_of(args)?;
    grouping_of(args)?;
    let opt_id = |k: &str| -> Result<Option<Id>, String> {
        match args.get(k) {
            Some(v) if !v.is_null() => Ok(Some(Id(id_of(v)?))),
            _ => Ok(None),
        }
    };
    let (after, before) = (opt_id("after")?, opt_id("before")?);
    let pid;
    if after.is_some() || before.is_some() {
        // A paragraph of its own, then the chart in it; the second step joins the first's undo step.
        doc.begin_chart_edit();
        let r = doc.apply_batch(&[Op::InsertParagraph { after, before, text: String::new(), style: None }], false).map_err(|e| e.to_string())?;
        pid = r[0].created.first().copied().ok_or("could not add a paragraph")?;
        if let Err(e) = doc.apply_batch(&[mk(Pos::new(pid, 0))], true) {
            doc.undo();
            return Err(e.to_string());
        }
    } else {
        let id = id_of(args.get("id").ok_or("say where: after, before, or id (with after_text)")?)?;
        let at = match s(args, "after_text") {
            Some(t) if !t.is_empty() => find_in_para(doc, id, &t, false)?[0].1,
            _ => doc.find_any_para(id).ok_or(format!("paragraph p{id} not found"))?.len16(),
        };
        doc.apply_batch(&[mk(Pos::new(id, at))], false).map_err(|e| e.to_string())?;
        pid = id;
    }
    Ok(format!("Chart inserted:\n{}", script_for_paragraphs(doc, &[pid])))
}

fn set_chart(doc: &mut Document, args: &Value) -> Result<String, String> {
    let id = id_of(args.get("id").ok_or("id is required: the paragraph that holds the chart")?)?;
    let index = args.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let charts = doc.charts_in(id);
    let (o, _) = charts.get(index).ok_or_else(|| {
        if charts.is_empty() {
            format!("p{id} has no chart. Charts show in read_document as [chart p12@3: ...].")
        } else {
            format!("p{id} has {} chart(s); index {index} is out of range", charts.len())
        }
    })?;
    let op = Op::SetChart {
        at: Pos::new(id, *o),
        kind: kind_of(args)?,
        grouping: grouping_of(args)?,
        title: s(args, "title"),
        categories: categories_of(args)?,
        series: series_of(args)?,
        legend: s(args, "legend"),
        data_labels: args.get("data_labels").and_then(|v| v.as_bool()),
        x_title: s(args, "x_title"),
        y_title: s(args, "y_title"),
        options: args.get("options").filter(|v| !v.is_null()).cloned(),
    };
    doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    Ok(format!("Chart updated:\n{}", script_for_paragraphs(doc, &[id])))
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn num(v: Option<f64>) -> String {
    match v {
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// How DocScript shows a chart: a line with its id, position and type, then its data as a small table
/// (series as rows, categories as columns).
pub fn script(pid: u32, o: usize, m: &crate::chart::ChartModel) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let title = m.title.as_deref().filter(|t| !t.is_empty()).unwrap_or("untitled");
    let mut kind = m.kind.name().to_string();
    if m.combo {
        let mut k: Vec<&str> = m.series.iter().map(|s| s.kind.name()).collect();
        k.dedup();
        kind = format!("combination of {}", k.join(" and "));
    } else if m.grouping != Grouping::Clustered && m.kind.can_stack() {
        kind = format!("{} {}", if m.grouping == Grouping::Stacked { "stacked" } else { "100% stacked" }, kind);
    }
    if let Some(v) = &m.stock {
        kind = format!("{v} stock");
    }
    if let Some(v) = &m.surface {
        kind = format!("{} surface", v.replace('_', " "));
    }
    let _ = write!(out, "[chart p{pid}@{o}: {title} ({kind}");
    if let Some(l) = &m.legend {
        let _ = write!(out, ", legend {}", match l.position.as_str() { "l" => "left", "t" => "top", "b" => "bottom", _ => "right" });
    }
    if m.data_labels {
        out.push_str(", data labels");
    }
    out.push(')');
    if m.kind == ChartKind::Other || m.series.is_empty() {
        out.push_str(" - Docxtor can't read the data of this kind of chart]");
        return out;
    }
    if matches!(m.kind, ChartKind::Scatter | ChartKind::Bubble) {
        for s in &m.series {
            let xs = s.x_values.clone().unwrap_or_default();
            let pts: Vec<String> = s
                .values
                .iter()
                .enumerate()
                .map(|(i, y)| match s.sizes.as_ref().and_then(|z| z.get(i).copied().flatten()) {
                    Some(z) => format!("({}, {}, size {})", num(xs.get(i).copied().flatten()), num(*y), z),
                    None => format!("({}, {})", num(xs.get(i).copied().flatten()), num(*y)),
                })
                .collect();
            let _ = write!(out, "\n  {}: {}", s.name, pts.join(" "));
        }
        out.push(']');
        return out;
    }
    out.push('\n');
    let _ = write!(out, "| |{}|\n|---|{}|\n", m.categories.iter().map(|c| format!(" {} ", cell(c))).collect::<Vec<_>>().join("|"), m.categories.iter().map(|_| "---").collect::<Vec<_>>().join("|"));
    for s in &m.series {
        let _ = writeln!(out, "| {} |{}|", cell(&s.name), (0..m.categories.len()).map(|i| format!(" {} ", num(s.values.get(i).copied().flatten()))).collect::<Vec<_>>().join("|"));
    }
    // The bracket closes on the last row.
    if out.ends_with('\n') {
        out.pop();
    }
    out.push(']');
    out
}
