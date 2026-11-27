//! Chart looks and colour palettes, as Word's Chart Styles and Change Colors galleries offer them. A look is a
//! set of defaults for fills, outlines, gaps and gridlines (and Word's `c:style` number); a palette gives the
//! series theme colours. Both are applied to the model, and `format::write` turns the model into XML. A palette
//! is also written as Word writes it, in a `colorsN.xml` part (`cs:colorStyle`).

use super::model::*;

/// The accent numbers (1 to 6) of each colourful palette, in series order.
const COLORFUL: [[u8; 6]; 4] = [[1, 2, 3, 4, 5, 6], [2, 4, 6, 1, 3, 5], [3, 5, 1, 4, 6, 2], [4, 6, 2, 5, 1, 3]];

/// What Word does to a theme colour for the 7th, 8th, ... series of a colourful palette: `(lumMod, lumOff)`.
const VARIATIONS: [(u32, u32); 9] = [(100000, 0), (60000, 0), (80000, 20000), (80000, 0), (60000, 40000), (50000, 0), (70000, 30000), (70000, 0), (50000, 50000)];

/// Palette ids as the dialog and the options name them: `colorful1` to `colorful4`, `mono1` to `mono13`.
pub fn palette_ids() -> Vec<String> {
    (1..=4).map(|n| format!("colorful{n}")).chain((1..=13).map(|n| format!("mono{n}"))).collect()
}

/// Word's `cs:colorStyle id` for a palette (10 to 13 colourful, 14 to 26 monochromatic).
pub fn palette_number(id: &str) -> Option<u32> {
    if let Some(n) = id.strip_prefix("colorful").and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=4).contains(n)) {
        return Some(9 + n);
    }
    id.strip_prefix("mono").and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=13).contains(n)).map(|n| 13 + n)
}

pub fn palette_from_number(n: u32) -> Option<String> {
    match n {
        10..=13 => Some(format!("colorful{}", n - 9)),
        14..=26 => Some(format!("mono{}", n - 13)),
        _ => None,
    }
}

fn spec(base: &str, lum_mod: u32, lum_off: u32) -> String {
    let mut s = format!("scheme:{base}");
    if lum_mod != 100000 {
        s.push_str(&format!(":lumMod={lum_mod}"));
    }
    if lum_off != 0 {
        s.push_str(&format!(":lumOff={lum_off}"));
    }
    s
}

/// The colour (as a `scheme:` setting) of series `i` of `n` in a palette. Monochromatic palettes run from a shade to a
/// tint of one theme colour (7 to 12 the other way round); the last one is greys.
pub fn palette_color(id: &str, i: usize, n: usize) -> Option<String> {
    if let Some(k) = id.strip_prefix("colorful").and_then(|k| k.parse::<usize>().ok()).filter(|k| (1..=4).contains(k)) {
        let accent = COLORFUL[k - 1][i % 6];
        let (m, o) = VARIATIONS[(i / 6) % VARIATIONS.len()];
        return Some(spec(&format!("accent{accent}"), m, o));
    }
    let k = id.strip_prefix("mono").and_then(|k| k.parse::<usize>().ok()).filter(|k| (1..=13).contains(k))?;
    let n = n.max(1);
    let mut f = if n == 1 { 0.5 } else { i as f64 / (n - 1) as f64 };
    if (7..=12).contains(&k) {
        f = 1.0 - f;
    }
    // Shade (down to 50% of the colour's lightness) through the colour itself (at 0.5) to a tint (60% lighter).
    let (m, o) = if f < 0.5 { (0.5 + f, 0.0) } else { (1.0 - (f - 0.5) * 1.2, (f - 0.5) * 1.2) };
    let r = |v: f64| ((v * 100.0).round() as u32) * 1000;
    if k == 13 {
        // Greys of the text colour: from near black to light grey.
        let l = 0.15 + f * 0.65;
        return Some(spec("tx1", r(1.0 - l), r(l)));
    }
    let accent = if k <= 6 { k } else { k - 6 };
    Some(spec(&format!("accent{accent}"), r(m), r(o)))
}

/// Give the chart's series (or, for pies and varied bars, its points) the colours of a palette.
pub fn apply_palette(m: &mut ChartModel, id: &str) -> Result<(), String> {
    if palette_number(id).is_none() {
        return Err(format!("unknown palette \"{id}\": use colorful1 to colorful4 or mono1 to mono13"));
    }
    let pie = matches!(m.kind, ChartKind::Pie | ChartKind::Doughnut);
    let varied = (pie || m.vary_colors) && m.series.len() == 1;
    let n = if varied { m.categories.len().max(m.series[0].values.len()) } else { m.series.len() };
    for (si, s) in m.series.iter_mut().enumerate() {
        if varied {
            s.point_colors = (0..n).map(|i| palette_color(id, i, n)).collect();
        } else {
            s.color = palette_color(id, si, n);
            s.point_colors.clear();
        }
    }
    m.palette = Some(id.to_string());
    Ok(())
}

/// The `colorN.xml` part Word writes for a palette.
pub fn colors_part_xml(id: &str) -> Option<String> {
    let number = palette_number(id)?;
    let variations: String = VARIATIONS.iter().map(|(m, o)| match (*m, *o) {
        (100000, 0) => "<cs:variation/>".to_string(),
        (m, 0) => format!("<cs:variation><a:lumMod val=\"{m}\"/></cs:variation>"),
        (m, o) => format!("<cs:variation><a:lumMod val=\"{m}\"/><a:lumOff val=\"{o}\"/></cs:variation>"),
    }).collect();
    let (meth, colors) = if let Some(k) = id.strip_prefix("colorful").and_then(|k| k.parse::<usize>().ok()) {
        ("cycle", COLORFUL[k - 1].iter().map(|a| format!("<a:schemeClr val=\"accent{a}\"/>")).collect::<String>())
    } else {
        let k = id.strip_prefix("mono").and_then(|k| k.parse::<usize>().ok())?;
        let meth = if (7..=12).contains(&k) { "withinLinearReversed" } else { "withinLinear" };
        let base = if k == 13 { "<a:schemeClr val=\"tx1\"/>".to_string() } else { format!("<a:schemeClr val=\"accent{}\"/>", if k <= 6 { k } else { k - 6 }) };
        (meth, base)
    };
    Some(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cs:colorStyle xmlns:cs=\"http://schemas.microsoft.com/office/drawing/2012/chartStyle\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" meth=\"{meth}\" id=\"{number}\">{colors}{variations}</cs:colorStyle>"
    ))
}

/// The number of looks in the Style gallery.
pub const LOOKS: u32 = 8;

/// Word's `c:style` number each look writes (the older 1 to 48 styles; modern Word keeps the formatting in the
/// chart itself, so the number is a label for older readers).
const LOOK_STYLE: [i64; 8] = [2, 10, 18, 26, 34, 42, 6, 14];

/// Apply a look to the model: fills, outlines, bar gaps, gridlines and labels. Look 1 is Word's default.
pub fn apply_look(m: &mut ChartModel, n: u32) -> Result<(), String> {
    if !(1..=LOOKS).contains(&n) {
        return Err(format!("look {n} doesn't exist: the Style gallery has looks 1 to {LOOKS}"));
    }
    let clustered = m.grouping == Grouping::Clustered;
    let (fill, line_width, gap, overlap, grid, labels): (Option<&str>, Option<f64>, u32, i32, bool, Option<bool>) = match n {
        1 => (None, None, 219, -27, true, None),
        2 => (None, None, 100, -10, false, Some(true)),
        3 => (Some("none"), Some(2.25), 150, -10, true, None),
        4 => (Some("soft"), Some(1.5), 80, -10, true, None),
        5 => (Some("gradient"), None, 60, 0, false, None),
        6 => (None, Some(4.5), 40, 0, true, None),
        7 => (None, None, 150, 0, false, Some(false)),
        _ => (None, Some(0.75), 20, 0, true, None),
    };
    for s in &mut m.series {
        s.fill = fill.map(String::from);
        s.line_width = line_width;
        s.line_color = if n == 8 { Some("FFFFFF".into()) } else { None };
        if matches!(s.kind, ChartKind::Line | ChartKind::Scatter | ChartKind::Radar) && n == 6 {
            s.marker = true;
        }
        if let Some(on) = labels {
            let allowed = !matches!(s.kind, ChartKind::Surface | ChartKind::Stock);
            if on && allowed {
                let mut l = s.labels.take().unwrap_or_default();
                if !l.any() {
                    l.value = !matches!(m.kind, ChartKind::Pie | ChartKind::Doughnut);
                    l.percent = matches!(m.kind, ChartKind::Pie | ChartKind::Doughnut);
                }
                s.labels = Some(l);
            } else if !on {
                s.labels = None;
            }
        }
    }
    if matches!(m.kind, ChartKind::Column | ChartKind::Bar) || m.series.iter().any(|s| matches!(s.kind, ChartKind::Column | ChartKind::Bar)) {
        m.gap_width = Some(if clustered { gap } else { gap.min(150) });
        m.overlap = Some(if clustered { overlap } else { 100 });
    }
    m.y_axis.major_grid = grid;
    m.style = Some(LOOK_STYLE[(n - 1) as usize]);
    if let Some(on) = labels {
        m.data_labels = on;
    }
    Ok(())
}
