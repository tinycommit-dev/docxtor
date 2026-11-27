//! Word's built-in table designs: Table Grid, Plain Table 1 to 5, Grid Table 1 to 7 and List Table 1 to 7 (the
//! last two in grey and in each of the six accent colours). A design is a table style in `styles.xml`; the first time
//! one is used its definition is written as Word writes it, with the colours both as the theme's values and as
//! theme references (`w:themeColor`, `w:themeFill`, tint and shade), so a document theme can recolour it.
//!
//! The definitions are built from a few rules per family (which lines, which fills, what the header does) and not
//! kept as 100 strings.

use crate::shapes::theme::ShapeTheme;
use crate::styles::{StyleSheet, ThemeFonts};
use crate::table_style::{CellPos, Sides, TableLook, TableStyler};
use serde::Serialize;

/// The six accent colours' number or the neutral grey of a design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Grey,
    Accent(u8),
}

/// One design of the gallery.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Design {
    /// The style id Word uses (`GridTable4-Accent1`).
    pub id: String,
    /// The name people see (`Grid Table 4 Accent 1`).
    pub name: String,
    /// `plain`, `grid` or `list` (Table Grid is in `plain`).
    pub group: &'static str,
    /// 1 to 6, or None for the grey one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Plain(u8),
    Grid(u8),
    List(u8),
}

const GRID_NAMES: [&str; 7] = ["Light", "", "", "", "Dark", "Colorful", "Colorful"];

fn family_name(f: Family) -> (String, String) {
    let (kind, n) = match f {
        Family::Plain(n) => ("Plain Table", n),
        Family::Grid(n) => ("Grid Table", n),
        Family::List(n) => ("List Table", n),
    };
    let word = match f {
        Family::Plain(_) => "",
        _ => GRID_NAMES[n as usize - 1],
    };
    let name = if word.is_empty() { format!("{kind} {n}") } else { format!("{kind} {n} {word}") };
    (name.replace(' ', ""), name)
}

/// Every design, in the order of the gallery.
pub fn designs() -> Vec<Design> {
    let mut out = vec![Design { id: "TableGrid".into(), name: "Table Grid".into(), group: "plain", accent: None }];
    for n in 1..=5 {
        let (id, name) = family_name(Family::Plain(n));
        out.push(Design { id, name, group: "plain", accent: None });
    }
    for (group, make) in [("grid", Family::Grid as fn(u8) -> Family), ("list", Family::List as fn(u8) -> Family)] {
        for n in 1..=7 {
            let (id, name) = family_name(make(n));
            out.push(Design { id: id.clone(), name: name.clone(), group, accent: None });
            for a in 1..=6u8 {
                out.push(Design { id: format!("{id}-Accent{a}"), name: format!("{name} Accent {a}"), group, accent: Some(a) });
            }
        }
    }
    out
}

/// A design or a table style the document has, by id or by name (case, spaces and hyphens do not matter).
pub fn find(name: &str, styles: &StyleSheet) -> Option<String> {
    if let Some(id) = styles.lookup(name) {
        if styles.get(&id).is_some_and(|i| i.kind == "table") {
            return Some(id);
        }
    }
    let norm = |s: &str| s.to_lowercase().replace([' ', '-', '_'], "");
    let n = norm(name);
    designs().into_iter().find(|d| norm(&d.id) == n || norm(&d.name) == n).map(|d| d.id)
}

fn parse_id(id: &str) -> Option<(Family, Tone)> {
    let (base, tone) = match id.split_once("-Accent") {
        Some((b, a)) => (b, Tone::Accent(a.parse::<u8>().ok().filter(|a| (1..=6).contains(a))?)),
        None => (id, Tone::Grey),
    };
    for n in 1..=7u8 {
        for f in [Family::Plain(n), Family::Grid(n), Family::List(n)] {
            if matches!(f, Family::Plain(_)) && n > 5 {
                continue;
            }
            if family_name(f).0 == base {
                // Plain tables have no accents.
                if matches!(f, Family::Plain(_)) && tone != Tone::Grey {
                    return None;
                }
                return Some((f, tone));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------

/// A theme colour with Word's tint (towards white) or shade (towards black), as two hex digits.
#[derive(Debug, Clone, Copy)]
struct Col {
    /// `text1`, `background1` or `accent1` to `accent6`.
    theme: &'static str,
    tint: Option<u8>,
    shade: Option<u8>,
}

const ACCENTS: [&str; 6] = ["accent1", "accent2", "accent3", "accent4", "accent5", "accent6"];

impl Col {
    fn text() -> Col {
        Col { theme: "text1", tint: None, shade: None }
    }
    fn bg() -> Col {
        Col { theme: "background1", tint: None, shade: None }
    }
    fn of(tone: Tone) -> Col {
        match tone {
            Tone::Grey => Col::text(),
            Tone::Accent(a) => Col { theme: ACCENTS[a as usize - 1], tint: None, shade: None },
        }
    }
    fn tint(mut self, t: u8) -> Col {
        self.tint = Some(t);
        self
    }
    fn shade(mut self, s: u8) -> Col {
        self.shade = Some(s);
        self
    }

    /// The colour as the theme gives it, with the tint or shade applied.
    fn hex(&self, theme: &ShapeTheme) -> String {
        let slot = match self.theme {
            "text1" => "dk1",
            "background1" => "lt1",
            other => other,
        };
        let base = theme.scheme(slot).unwrap_or(if slot == "lt1" { "FFFFFF" } else { "000000" }).to_string();
        let (r, g, b) = (u8::from_str_radix(&base[0..2], 16).unwrap_or(0), u8::from_str_radix(&base[2..4], 16).unwrap_or(0), u8::from_str_radix(&base[4..6], 16).unwrap_or(0));
        let (h, s, mut l) = rgb_to_hsl(r, g, b);
        if let Some(t) = self.tint {
            let t = t as f64 / 255.0;
            l = l * t + (1.0 - t);
        }
        if let Some(sh) = self.shade {
            l *= sh as f64 / 255.0;
        }
        let (r, g, b) = hsl_to_rgb(h, s, l);
        format!("{r:02X}{g:02X}{b:02X}")
    }

    /// `w:color="…" w:themeColor="…" w:themeTint="…"`.
    fn color_attrs(&self, theme: &ShapeTheme) -> String {
        let mut a = format!(r#"w:color="{}" w:themeColor="{}""#, self.hex(theme), self.theme);
        if let Some(t) = self.tint {
            a.push_str(&format!(r#" w:themeTint="{t:02X}""#));
        }
        if let Some(s) = self.shade {
            a.push_str(&format!(r#" w:themeShade="{s:02X}""#));
        }
        a
    }

    /// `w:fill="…" w:themeFill="…" w:themeFillTint="…"`.
    fn fill_attrs(&self, theme: &ShapeTheme) -> String {
        let mut a = format!(r#"w:fill="{}" w:themeFill="{}""#, self.hex(theme), self.theme);
        if let Some(t) = self.tint {
            a.push_str(&format!(r#" w:themeFillTint="{t:02X}""#));
        }
        if let Some(s) = self.shade {
            a.push_str(&format!(r#" w:themeFillShade="{s:02X}""#));
        }
        a
    }
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-12 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0).floor() as i64 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    // Word rounds down.
    let q = |v: f64| ((v + m) * 255.0 + 1e-6).floor().clamp(0.0, 255.0) as u8;
    (q(r), q(g), q(b))
}

// ---------------------------------------------------------------------------
// Writing the definitions
// ---------------------------------------------------------------------------

fn line(name: &str, val: &str, sz: u32, c: Col, theme: &ShapeTheme) -> String {
    format!(r#"<w:{name} w:val="{val}" w:sz="{sz}" w:space="0" {}/>"#, c.color_attrs(theme))
}

fn nil(name: &str) -> String {
    format!(r#"<w:{name} w:val="nil"/>"#)
}

fn shd(c: Col, theme: &ShapeTheme) -> String {
    format!(r#"<w:shd w:val="clear" w:color="auto" {}/>"#, c.fill_attrs(theme))
}

const BOLD: &str = "<w:b/><w:bCs/>";

/// One `w:tblStylePr`: run properties, the borders of its cells, and a fill.
struct Cond {
    kind: &'static str,
    rpr: String,
    borders: String,
    fill: Option<String>,
}

impl Cond {
    fn new(kind: &'static str) -> Cond {
        Cond { kind, rpr: String::new(), borders: String::new(), fill: None }
    }
    fn rpr(mut self, r: &str) -> Cond {
        self.rpr = r.into();
        self
    }
    fn borders(mut self, b: String) -> Cond {
        self.borders = b;
        self
    }
    fn fill(mut self, f: String) -> Cond {
        self.fill = Some(f);
        self
    }
    fn xml(&self) -> String {
        let mut s = format!(r#"<w:tblStylePr w:type="{}">"#, self.kind);
        if !self.rpr.is_empty() {
            s.push_str(&format!("<w:rPr>{}</w:rPr>", self.rpr));
        }
        if !self.borders.is_empty() || self.fill.is_some() {
            // Word writes an empty table-properties element before the cell properties.
            s.push_str("<w:tblPr/><w:tcPr>");
            if !self.borders.is_empty() {
                s.push_str(&format!("<w:tcBorders>{}</w:tcBorders>", self.borders));
            }
            if let Some(f) = &self.fill {
                s.push_str(f);
            }
            s.push_str("</w:tcPr>");
        }
        s.push_str("</w:tblStylePr>");
        s
    }
}

struct Def {
    root_rpr: String,
    borders: String,
    root_fill: Option<String>,
    conds: Vec<Cond>,
}

fn cond_order(kind: &str) -> usize {
    ["firstRow", "lastRow", "firstCol", "lastCol", "band1Vert", "band2Vert", "band1Horz", "band2Horz", "neCell", "nwCell", "seCell", "swCell"]
        .iter()
        .position(|k| *k == kind)
        .unwrap_or(99)
}

/// The rules of each family. `m` is the design's own colour (black, or an accent).
fn definition(f: Family, tone: Tone, theme: &ShapeTheme) -> Def {
    let m = Col::of(tone);
    let t = theme;
    let sides = |names: &[&str], val: &str, sz: u32, c: Col| names.iter().map(|n| line(n, val, sz, c, t)).collect::<String>();
    let all6 = ["top", "left", "bottom", "right", "insideH", "insideV"];
    let box4 = ["top", "left", "bottom", "right"];
    let bold_cols = |conds: &mut Vec<Cond>, extra: &str| {
        conds.push(Cond::new("firstCol").rpr(&format!("{BOLD}{extra}")));
        conds.push(Cond::new("lastCol").rpr(&format!("{BOLD}{extra}")));
    };
    match f {
        Family::Plain(1) => {
            let c = Col::bg().shade(0xBF);
            let fill = shd(Col::bg().shade(0xF2), t);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "single", 4, c, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").fill(fill.clone()));
            conds.push(Cond::new("band1Horz").fill(fill));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom"], "single", 4, c), root_fill: None, conds }
        }
        Family::Plain(2) => {
            let c = Col::text().tint(0x80);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "single", 4, c, t)),
                Cond::new("firstCol").rpr(BOLD).borders(line("right", "single", 4, c, t)),
                Cond::new("lastCol").rpr(BOLD).borders(line("left", "single", 4, c, t)),
            ];
            conds.push(Cond::new("band1Vert").borders(sides(&["left", "right"], "single", 4, c)));
            conds.push(Cond::new("band1Horz").borders(sides(&["top", "bottom"], "single", 4, c)));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom"], "single", 4, c), root_fill: None, conds }
        }
        Family::Plain(3) => {
            let c = Col::text().tint(0x80);
            let fill = shd(Col::bg().shade(0xF2), t);
            let caps = format!("{BOLD}<w:caps/>");
            let mut conds = vec![
                Cond::new("firstRow").rpr(&caps).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(&caps).borders(line("top", "single", 4, c, t)),
                Cond::new("firstCol").rpr(&caps),
                Cond::new("lastCol").rpr(&caps),
            ];
            conds.push(Cond::new("band1Vert").fill(fill.clone()));
            conds.push(Cond::new("band1Horz").fill(fill));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom"], "single", 4, c), root_fill: None, conds }
        }
        Family::Plain(4) => {
            let fill = shd(Col::bg().shade(0xF2), t);
            let mut conds = vec![Cond::new("firstRow").rpr(BOLD), Cond::new("lastRow").rpr(BOLD)];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").fill(fill.clone()));
            conds.push(Cond::new("band1Horz").fill(fill));
            Def { root_rpr: String::new(), borders: String::new(), root_fill: None, conds }
        }
        Family::Plain(_) => {
            let c = Col::bg().shade(0xA6);
            let grey = shd(Col::bg().shade(0xD9), t);
            let italic = "<w:i/><w:iCs/>";
            let conds = vec![
                Cond::new("firstRow").rpr(&format!("{italic}<w:sz w:val=\"36\"/><w:szCs w:val=\"36\"/>")).borders(line("bottom", "single", 4, c, t)).fill(grey.clone()),
                Cond::new("lastRow").rpr(&format!("{italic}<w:sz w:val=\"36\"/><w:szCs w:val=\"36\"/>")).borders(line("top", "single", 4, c, t)).fill(grey.clone()),
                Cond::new("firstCol").rpr(&format!("{italic}<w:caps/>")).fill(grey.clone()),
                Cond::new("lastCol").rpr(&format!("{italic}<w:caps/>")).fill(grey),
                Cond::new("band1Horz").fill(shd(Col::bg().shade(0xF2), t)),
            ];
            Def { root_rpr: String::new(), borders: String::new(), root_fill: None, conds }
        }
        Family::Grid(1) => {
            let line_c = m.tint(0x66);
            let rule = m.tint(0x99);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 12, rule, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, rule, t)),
            ];
            bold_cols(&mut conds, "");
            Def { root_rpr: String::new(), borders: sides(&all6, "single", 4, line_c), root_fill: None, conds }
        }
        Family::Grid(2) => {
            let c = m.tint(0x99);
            let band = shd(m.tint(0x33), t);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(format!("{}{}", nil("top"), line("bottom", "single", 12, c, t))),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, c, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").fill(band.clone()));
            conds.push(Cond::new("band1Horz").fill(band));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom", "insideH", "insideV"], "single", 4, c), root_fill: None, conds }
        }
        Family::Grid(3) => {
            let c = m.tint(0x99);
            let caps = format!("{BOLD}<w:caps/>");
            let band = shd(m.tint(0x33), t);
            let conds = vec![
                Cond::new("firstRow").rpr(&caps).borders(line("bottom", "single", 12, c, t)),
                Cond::new("lastRow").rpr(&caps).borders(line("top", "double", 6, c, t)),
                Cond::new("firstCol").rpr(&caps),
                Cond::new("lastCol").rpr(&caps),
                Cond::new("band1Vert").fill(band.clone()),
                Cond::new("band1Horz").fill(band),
            ];
            Def { root_rpr: String::new(), borders: sides(&all6, "single", 4, c), root_fill: None, conds }
        }
        Family::Grid(4) => {
            let c = m.tint(0x99);
            let band = shd(m.tint(0x33), t);
            let white = format!(r#"{BOLD}<w:color w:val="FFFFFF" w:themeColor="background1"/>"#);
            let mut conds = vec![
                Cond::new("firstRow").rpr(&white).borders(sides(&box4, "single", 4, m)).fill(shd(m, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, m, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").fill(band.clone()));
            conds.push(Cond::new("band1Horz").fill(band));
            Def { root_rpr: String::new(), borders: sides(&all6, "single", 4, c), root_fill: None, conds }
        }
        Family::Grid(5) => {
            let white = Col::bg();
            let on_dark = format!(r#"{BOLD}<w:color w:val="FFFFFF" w:themeColor="background1"/>"#);
            let dark = shd(m, t);
            let conds = vec![
                Cond::new("firstRow").rpr(&on_dark).borders(line("bottom", "single", 12, white, t)).fill(dark.clone()),
                Cond::new("lastRow").rpr(&on_dark).borders(line("top", "single", 12, white, t)).fill(dark.clone()),
                Cond::new("firstCol").rpr(&on_dark).fill(dark.clone()),
                Cond::new("lastCol").rpr(&on_dark).fill(dark),
                Cond::new("band1Vert").fill(shd(m.tint(0x66), t)),
                Cond::new("band1Horz").fill(shd(m.tint(0x66), t)),
            ];
            Def { root_rpr: String::new(), borders: sides(&all6, "single", 4, white), root_fill: Some(shd(m.tint(0x33), t)), conds }
        }
        Family::Grid(6) => {
            let c = m.tint(0x99);
            let band = shd(m.tint(0x33), t);
            let ring = sides(&box4, "single", 4, c);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 12, c, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, c, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").borders(ring.clone()).fill(band.clone()));
            conds.push(Cond::new("band1Horz").borders(ring).fill(band));
            Def { root_rpr: format!(r#"<w:color {}/>"#, m.shade(0xBF).color_attrs(t)), borders: sides(&all6, "single", 4, c), root_fill: None, conds }
        }
        Family::Grid(_) => {
            let c = m.tint(0x99);
            let ring = sides(&box4, "single", 4, c);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(ring.clone()),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, c, t)),
                Cond::new("firstCol").rpr(BOLD).borders(ring.clone()),
                Cond::new("lastCol").rpr(BOLD).borders(ring.clone()),
            ];
            conds.push(Cond::new("band1Vert").borders(ring.clone()));
            conds.push(Cond::new("band1Horz").borders(ring));
            Def { root_rpr: format!(r#"<w:color {}/>"#, m.shade(0xBF).color_attrs(t)), borders: sides(&all6, "single", 4, c), root_fill: None, conds }
        }
        Family::List(1) => {
            let c = if tone == Tone::Grey { Col::bg().shade(0xBF) } else { m.tint(0x66) };
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "single", 4, c, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").borders(sides(&["left", "right"], "single", 4, c)));
            conds.push(Cond::new("band1Horz").borders(sides(&["top", "bottom"], "single", 4, c)));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom"], "single", 4, c), root_fill: None, conds }
        }
        Family::List(2) => {
            let c = m.tint(0x99);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "single", 4, c, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").borders(sides(&["left", "right"], "single", 4, c)));
            conds.push(Cond::new("band1Horz").borders(sides(&["top", "bottom"], "single", 4, c)));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom", "insideH"], "single", 4, c), root_fill: None, conds }
        }
        Family::List(3) => {
            let white = format!(r#"{BOLD}<w:color w:val="FFFFFF" w:themeColor="background1"/>"#);
            let mut conds = vec![
                Cond::new("firstRow").rpr(&white).fill(shd(m, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, m, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").borders(sides(&["left", "right"], "single", 4, m)));
            conds.push(Cond::new("band1Horz").borders(sides(&["top", "bottom"], "single", 4, m)));
            Def { root_rpr: String::new(), borders: sides(&box4, "single", 4, m), root_fill: None, conds }
        }
        Family::List(4) => {
            let c = m.tint(0x99);
            let white = format!(r#"{BOLD}<w:color w:val="FFFFFF" w:themeColor="background1"/>"#);
            let band = shd(m.tint(0x33), t);
            let mut conds = vec![
                Cond::new("firstRow").rpr(&white).fill(shd(m, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, m, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").fill(band.clone()));
            conds.push(Cond::new("band1Horz").fill(band));
            Def { root_rpr: String::new(), borders: sides(&["top", "bottom", "insideH"], "single", 4, c), root_fill: None, conds }
        }
        Family::List(5) => {
            let white = Col::bg();
            let on_dark = format!(r#"{BOLD}<w:color w:val="FFFFFF" w:themeColor="background1"/>"#);
            let dark = shd(m, t);
            let conds = vec![
                Cond::new("firstRow").rpr(&on_dark).fill(dark.clone()),
                Cond::new("lastRow").rpr(&on_dark).borders(line("top", "single", 12, white, t)).fill(dark.clone()),
                Cond::new("firstCol").rpr(&on_dark).fill(dark.clone()),
                Cond::new("lastCol").rpr(&on_dark).fill(dark),
                Cond::new("band1Horz").fill(shd(m.tint(0x66), t)),
                Cond::new("band1Vert").fill(shd(m.tint(0x66), t)),
            ];
            Def { root_rpr: String::new(), borders: sides(&box4, "single", 4, m), root_fill: Some(shd(m.tint(0x33), t)), conds }
        }
        Family::List(6) => {
            let c = m.tint(0x99);
            let mut conds = vec![
                Cond::new("firstRow").rpr(BOLD).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(BOLD).borders(line("top", "double", 6, c, t)),
            ];
            bold_cols(&mut conds, "");
            conds.push(Cond::new("band1Vert").borders(sides(&["left", "right"], "single", 4, c)));
            conds.push(Cond::new("band1Horz").borders(sides(&["top", "bottom"], "single", 4, c)));
            Def { root_rpr: format!(r#"<w:color {}/>"#, m.shade(0xBF).color_attrs(t)), borders: sides(&["top", "bottom"], "single", 4, c), root_fill: None, conds }
        }
        Family::List(_) => {
            let c = m.tint(0x99);
            let it = format!("{BOLD}<w:i/><w:iCs/>");
            let conds = vec![
                Cond::new("firstRow").rpr(&it).borders(line("bottom", "single", 4, c, t)),
                Cond::new("lastRow").rpr(&it).borders(line("top", "single", 4, c, t)),
                Cond::new("firstCol").rpr(&it),
                Cond::new("lastCol").rpr(&it),
                Cond::new("band1Horz").borders(sides(&["top", "bottom"], "single", 4, c)),
            ];
            Def { root_rpr: format!(r#"<w:color {}/>"#, m.shade(0xBF).color_attrs(t)), borders: String::new(), root_fill: None, conds }
        }
    }
}

fn priority(f: Family) -> u32 {
    match f {
        Family::Plain(n) => 40 + n as u32,
        Family::Grid(n) | Family::List(n) => 45 + n as u32,
    }
}

/// The `w:style` of a design as Word writes it. Table Grid is not here (it is one of the styles Docxtor always had).
pub fn style_xml(id: &str, theme: &ShapeTheme) -> Option<String> {
    let (f, tone) = parse_id(id)?;
    let (sid, name) = family_name(f);
    let (sid, name) = match tone {
        Tone::Grey => (sid, name),
        Tone::Accent(a) => (format!("{sid}-Accent{a}"), format!("{name} Accent {a}")),
    };
    let mut d = definition(f, tone, theme);
    d.conds.sort_by_key(|c| cond_order(c.kind));
    let mut s = format!(
        r#"<w:style w:type="table" w:styleId="{sid}"><w:name w:val="{name}"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="{}"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>"#,
        priority(f)
    );
    if !d.root_rpr.is_empty() {
        s.push_str(&format!("<w:rPr>{}</w:rPr>", d.root_rpr));
    }
    s.push_str(r#"<w:tblPr><w:tblStyleRowBandSize w:val="1"/><w:tblStyleColBandSize w:val="1"/>"#);
    if !d.borders.is_empty() {
        s.push_str(&format!("<w:tblBorders>{}</w:tblBorders>", d.borders));
    }
    s.push_str("</w:tblPr>");
    if let Some(f) = &d.root_fill {
        s.push_str(&format!("<w:tcPr>{f}</w:tcPr>"));
    }
    for c in &d.conds {
        s.push_str(&c.xml());
    }
    s.push_str("</w:style>");
    Some(s)
}

/// Make sure the design (and `TableNormal` it is based on) is in the style sheet. Returns the style id.
pub fn ensure(styles: &mut StyleSheet, id: &str, theme: &ShapeTheme) -> Option<String> {
    if id == "TableGrid" || id.eq_ignore_ascii_case("table grid") {
        return styles.ensure_builtin("TableGrid");
    }
    if styles.exists(id) {
        return Some(id.to_string());
    }
    let xml = style_xml(id, theme)?;
    styles.ensure_builtin("TableNormal");
    styles.insert_style_xml(id, &xml)
}

// ---------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------

/// One cell of a design drawn small: what the gallery needs to paint it.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCell {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<(String, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<(String, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<(String, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<(String, f64)>,
    pub bold: bool,
    /// The text colour as hex (the line drawn for text takes it).
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub id: String,
    pub name: String,
    pub group: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<u8>,
    /// Rows of cells (five rows of four).
    pub cells: Vec<Vec<PreviewCell>>,
}

fn side(b: &Option<crate::props::Border>, theme: &ShapeTheme) -> Option<(String, f64)> {
    let b = b.as_ref().filter(|b| !b.is_nil())?;
    let color = if b.color.len() == 6 { b.color.to_uppercase() } else { theme.scheme("dk1").unwrap_or("000000").to_string() };
    Some((color, (b.sz as f64 / 8.0).max(0.5)))
}

/// Every design drawn as five rows of four cells with the given look.
pub fn previews(look: &TableLook, theme: &ShapeTheme) -> Vec<Preview> {
    designs().into_iter().filter_map(|d| preview(&d, look, theme)).collect()
}

pub fn preview(d: &Design, look: &TableLook, theme: &ShapeTheme) -> Option<Preview> {
    let mut styles = StyleSheet::parse(None, ThemeFonts::default());
    let sid = ensure(&mut styles, &d.id, theme)?;
    Some(Preview { id: d.id.clone(), name: d.name.clone(), group: d.group, accent: d.accent, cells: draw(&styles, &sid, look, theme) })
}

/// The id and name of the one table style in a design's XML (what an extension offers as a design): the
/// definition must be well formed and use only declared prefixes, and be a single `w:style` of type `table`.
pub fn parse_definition(xml: &str) -> Result<(String, String), String> {
    let parsed = crate::xml::parse(format!(r#"<x xmlns:w="w">{xml}</x>"#).as_bytes()).map_err(|_| "the table design is not well formed XML".to_string())?;
    crate::xml::check_namespaces(&parsed)?;
    let mut styles = parsed.root.elements();
    let style = styles.next().filter(|e| e.name == "w:style").ok_or("a table design is one w:style element")?;
    if styles.next().is_some() {
        return Err("a table design is one w:style element".into());
    }
    if style.attr("w:type") != Some("table") {
        return Err("the style of a table design has the type table".into());
    }
    let id = style.attr("w:styleId").filter(|i| !i.is_empty()).ok_or("the style of a table design needs a styleId")?.to_string();
    let name = style.child("w:name").and_then(|n| n.attr("w:val")).unwrap_or(&id).to_string();
    Ok((id, name))
}

/// A design given as XML (see `parse_definition`) drawn small. It may be based on `TableNormal`.
pub fn preview_definition(xml: &str, look: &TableLook, theme: &ShapeTheme) -> Result<Preview, String> {
    let (id, name) = parse_definition(xml)?;
    let mut styles = StyleSheet::parse(None, ThemeFonts::default());
    styles.ensure_builtin("TableNormal");
    let sid = styles.insert_style_xml(&id, xml).ok_or("the table design could not be read")?;
    Ok(Preview { id, name, group: "custom", accent: None, cells: draw(&styles, &sid, look, theme) })
}

fn draw(styles: &StyleSheet, sid: &str, look: &TableLook, theme: &ShapeTheme) -> Vec<Vec<PreviewCell>> {
    let tblpr = crate::xml::Element::new("w:tblPr").with_child(look.element());
    let styler = TableStyler::new(styles, Some(sid), Some(&tblpr));
    let (rows, cols) = (5usize, 4usize);
    let mut out = Vec::new();
    for r in 0..rows {
        let mut row = Vec::new();
        for c in 0..cols {
            let pos = CellPos { row: r, rows, col: c, cols, grid_from: c, grid_to: c + 1, grid_cols: cols };
            let cs = styler.cell(&pos, None, None);
            let Sides { top, bottom, left, right, .. } = &cs.borders;
            row.push(PreviewCell {
                fill: cs.shading.clone(),
                top: side(top, theme),
                bottom: side(bottom, theme),
                left: side(left, theme),
                right: side(right, theme),
                bold: cs.run.bold == Some(true),
                color: cs.run.color.clone().filter(|c| c.len() == 6).unwrap_or_else(|| "000000".into()),
            });
        }
        out.push(row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn office_2016() -> ShapeTheme {
        // The theme Word has used since 2016 (the crate's default is the 2013 one).
        ShapeTheme::default().with_scheme(&[("accent1", "4472C4"), ("accent5", "5B9BD5")])
    }

    #[test]
    fn tints_and_shades_match_words_palette() {
        let t = office_2016();
        let hex = |theme: &'static str, tint: Option<u8>, shade: Option<u8>| Col { theme, tint, shade }.hex(&t);
        // Word rounds a little differently in places: one step in a channel is the same colour.
        let near = |a: String, b: &str| {
            let v = |s: &str, i: usize| i32::from_str_radix(&s[i..i + 2], 16).unwrap();
            (0..3).all(|k| (v(&a, k * 2) - v(b, k * 2)).abs() <= 1)
        };
        // The standard "lighter 80, 60, 40 percent" and "darker 25 percent" colours of each accent.
        for (acc, light, mid, strong, dark) in [
            ("accent1", "D9E2F3", "B4C6E7", "8EAADB", "2F5496"),
            ("accent2", "FBE4D5", "F7CBAC", "F4B183", "C55A11"),
            ("accent3", "EDEDED", "DBDBDB", "C9C9C9", "7C7C7C"),
            ("accent4", "FFF2CC", "FFE599", "FFD966", "BF9000"),
            ("accent5", "DEEBF7", "BDD7EE", "9DC3E6", "2E75B6"),
            ("accent6", "E2F0D9", "C5E0B4", "A9D18E", "538135"),
        ] {
            assert!(near(hex(acc, Some(0x33), None), light), "{acc} 33 {}", hex(acc, Some(0x33), None));
            assert!(near(hex(acc, Some(0x66), None), mid), "{acc} 66 {}", hex(acc, Some(0x66), None));
            assert!(near(hex(acc, Some(0x99), None), strong), "{acc} 99 {}", hex(acc, Some(0x99), None));
            assert!(near(hex(acc, None, Some(0xBF)), dark), "{acc} BF {}", hex(acc, None, Some(0xBF)));
        }
        assert_eq!(hex("text1", Some(0x99), None), "666666");
        assert_eq!(hex("text1", Some(0x66), None), "999999");
        assert_eq!(hex("background1", None, Some(0xBF)), "BFBFBF");
        assert_eq!(hex("background1", None, Some(0xF2)), "F2F2F2");
    }

    #[test]
    fn the_gallery_has_every_design_once() {
        let all = designs();
        // Table Grid, five plain, and seven grid and seven list families in grey and six accents.
        assert_eq!(all.len(), 1 + 5 + 14 * 7);
        let mut ids: Vec<&str> = all.iter().map(|d| d.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for want in ["PlainTable1", "GridTable1Light", "GridTable1Light-Accent1", "GridTable4-Accent1", "GridTable5Dark", "GridTable6Colorful-Accent6", "ListTable3", "ListTable7Colorful-Accent2"] {
            assert!(all.iter().any(|d| d.id == want), "{want}");
        }
        assert_eq!(all.iter().find(|d| d.id == "GridTable4-Accent1").unwrap().name, "Grid Table 4 Accent 1");
        assert_eq!(all.iter().find(|d| d.id == "ListTable1Light").unwrap().name, "List Table 1 Light");
    }

    #[test]
    fn every_design_is_well_formed_and_resolves() {
        let t = office_2016();
        for d in designs() {
            if d.id == "TableGrid" {
                continue;
            }
            let xml = style_xml(&d.id, &t).unwrap_or_else(|| panic!("{}", d.id));
            let parsed = crate::xml::parse(format!(r#"<x xmlns:w="w">{xml}</x>"#).as_bytes()).unwrap_or_else(|e| panic!("{}: {e:?}", d.id));
            let style = parsed.root.child("w:style").unwrap();
            assert_eq!(style.attr("w:styleId"), Some(d.id.as_str()));
            assert_eq!(style.child("w:name").unwrap().attr("w:val"), Some(d.name.as_str()));
            // Children in the order the schema wants.
            let order = ["w:name", "w:basedOn", "w:uiPriority", "w:pPr", "w:rPr", "w:tblPr", "w:tcPr", "w:tblStylePr"];
            let mut last = 0;
            for c in style.elements() {
                let at = order.iter().position(|o| *o == c.name).unwrap_or_else(|| panic!("{} {}", d.id, c.name));
                assert!(at >= last, "{}: {} out of order", d.id, c.name);
                last = at;
            }
        }
    }

    #[test]
    fn grid_table_4_accent_1_draws_like_word() {
        let t = office_2016();
        let p = preview(&designs().into_iter().find(|d| d.id == "GridTable4-Accent1").unwrap(), &TableLook::default(), &t).unwrap();
        // Header row: accent fill, white bold text.
        let head = &p.cells[0][1];
        assert_eq!(head.fill.as_deref(), Some("4472C4"));
        assert!(head.bold);
        assert_eq!(head.color, "FFFFFF");
        // First column is bold, body rows are banded with the light tint, white between.
        assert!(p.cells[2][0].bold);
        assert_eq!(p.cells[1][1].fill.as_deref(), Some("D9E2F3"));
        assert_eq!(p.cells[2][1].fill, None);
        assert_eq!(p.cells[3][1].fill.as_deref(), Some("D9E2F3"));
        // Light blue grid lines.
        assert_eq!(p.cells[2][2].top.as_ref().unwrap().0, "8EAADB");
        // Without the header row option nothing is filled solid.
        let off = TableLook { first_row: false, ..TableLook::default() };
        let p = preview(&designs().into_iter().find(|d| d.id == "GridTable4-Accent1").unwrap(), &off, &t).unwrap();
        assert_eq!(p.cells[0][1].fill.as_deref(), Some("D9E2F3"));
        assert!(!p.cells[0][1].bold);
    }

    #[test]
    fn the_total_row_and_last_column_follow_the_look() {
        let t = office_2016();
        let look = TableLook { last_row: true, last_col: true, ..TableLook::default() };
        let d = designs().into_iter().find(|d| d.id == "GridTable4").unwrap();
        let p = preview(&d, &look, &t).unwrap();
        assert!(p.cells[4][1].bold);
        assert!(p.cells[2][3].bold);
        assert_eq!(p.cells[4][1].top.as_ref().unwrap().1, 0.75);
        let none = preview(&d, &TableLook { last_row: false, last_col: false, ..TableLook::default() }, &t).unwrap();
        assert!(!none.cells[4][1].bold);
    }
}
