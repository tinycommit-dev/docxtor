//! The little CSS the importer needs: style attributes, `<style>` rules (tag, class and tag.class selectors),
//! Word's `@list` definitions, and value parsing (colours, lengths, borders).

use super::dom::{HEl, HNode};
use std::collections::HashMap;

pub type Decls = Vec<(String, String)>;

/// The value of a property (the last declaration wins).
pub fn get<'a>(d: &'a Decls, k: &str) -> Option<&'a str> {
    d.iter().rev().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

/// Declarations from a style attribute or a rule body: `a: b; c: d`.
pub fn parse_decls(s: &str) -> Decls {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let flush = |cur: &mut String, out: &mut Decls| {
        if let Some((k, v)) = cur.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().trim_end_matches("!important").trim();
            if !k.is_empty() && !v.is_empty() {
                out.push((k, v.to_string()));
            }
        }
        cur.clear();
    };
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                cur.push(c);
            }
            (Some(_), c) => cur.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                cur.push(c);
            }
            (None, '(') => {
                depth += 1;
                cur.push(c);
            }
            (None, ')') => {
                depth -= 1;
                cur.push(c);
            }
            (None, ';') if depth <= 0 => flush(&mut cur, &mut out),
            (None, c) => cur.push(c),
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[derive(Debug, Clone)]
struct Rule {
    tag: Option<String>,
    class: Option<String>,
    decls: Decls,
}

#[derive(Debug, Default)]
pub struct Sheet {
    rules: Vec<Rule>,
    /// Word's `@list l0:level1 { ... }` definitions by `l0:level1`.
    pub lists: HashMap<String, Decls>,
}

impl Sheet {
    pub fn add(&mut self, css: &str) {
        let css = strip_comments(css);
        let b = css.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let Some(open) = css[i..].find('{').map(|k| i + k) else { break };
            let selector = css[i..open].trim().trim_start_matches("<!--").trim_start_matches("-->").trim();
            // The matching close, allowing nested blocks.
            let mut depth = 1;
            let mut j = open + 1;
            while j < b.len() && depth > 0 {
                match b[j] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let body = &css[open + 1..j.saturating_sub(1).max(open + 1)];
            i = j;
            if let Some(name) = selector.strip_prefix("@list") {
                self.lists.insert(name.trim().to_string(), parse_decls(body));
                continue;
            }
            if selector.starts_with('@') {
                continue;
            }
            let decls = parse_decls(body);
            for sel in selector.split(',') {
                let sel = sel.trim();
                // Only simple selectors: tag, .class, tag.class.
                if sel.is_empty() || sel.contains([' ', '>', '+', '~', ':', '[', '#', '*']) {
                    continue;
                }
                let (tag, class) = match sel.split_once('.') {
                    Some((t, c)) => (Some(t.to_ascii_lowercase()).filter(|t| !t.is_empty()), Some(c.to_string())),
                    None => (Some(sel.to_ascii_lowercase()), None),
                };
                if class.as_deref().is_some_and(|c| c.contains('.')) {
                    continue;
                }
                self.rules.push(Rule { tag, class, decls: decls.clone() });
            }
        }
    }

    /// The declarations that apply to an element: rules in order, then its own style attribute.
    pub fn computed(&self, el: &HEl) -> Decls {
        let mut out = Decls::new();
        for r in &self.rules {
            if r.tag.as_deref().is_some_and(|t| t != el.tag) {
                continue;
            }
            if let Some(c) = &r.class {
                if !el.classes().any(|k| k == c) {
                    continue;
                }
            }
            out.extend(r.decls.iter().cloned());
        }
        if let Some(s) = el.attr("style") {
            out.extend(parse_decls(s));
        }
        out
    }
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(e) => rest = &rest[i + 2 + e + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Every `<style>` in the tree, as sheet text.
pub fn collect_styles(nodes: &[HNode], sheet: &mut Sheet) {
    for n in nodes {
        if let HNode::El(e) = n {
            if e.tag == "style" {
                sheet.add(&e.text());
            } else {
                collect_styles(&e.children, sheet);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

const NAMED: &[(&str, &str)] = &[
    ("black", "000000"),
    ("white", "FFFFFF"),
    ("red", "FF0000"),
    ("green", "008000"),
    ("lime", "00FF00"),
    ("blue", "0000FF"),
    ("yellow", "FFFF00"),
    ("cyan", "00FFFF"),
    ("aqua", "00FFFF"),
    ("magenta", "FF00FF"),
    ("fuchsia", "FF00FF"),
    ("gray", "808080"),
    ("grey", "808080"),
    ("silver", "C0C0C0"),
    ("maroon", "800000"),
    ("olive", "808000"),
    ("navy", "000080"),
    ("purple", "800080"),
    ("teal", "008080"),
    ("orange", "FFA500"),
    ("pink", "FFC0CB"),
    ("brown", "A52A2A"),
    ("gold", "FFD700"),
    ("darkgray", "A9A9A9"),
    ("darkgrey", "A9A9A9"),
    ("lightgray", "D3D3D3"),
    ("lightgrey", "D3D3D3"),
    ("darkblue", "00008B"),
    ("darkred", "8B0000"),
    ("darkgreen", "006400"),
    ("lightblue", "ADD8E6"),
    ("lightgreen", "90EE90"),
    ("lightyellow", "FFFFE0"),
    ("whitesmoke", "F5F5F5"),
    ("crimson", "DC143C"),
    ("coral", "FF7F50"),
    ("indigo", "4B0082"),
    ("violet", "EE82EE"),
    ("tomato", "FF6347"),
    ("steelblue", "4682B4"),
    ("royalblue", "4169E1"),
    ("dodgerblue", "1E90FF"),
    ("slategray", "708090"),
    ("slategrey", "708090"),
];

/// A colour as six hex digits; None for `transparent`, `inherit`, `windowtext`, `auto` and the like.
pub fn parse_color(s: &str) -> Option<String> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        return match h.len() {
            3 if h.chars().all(|c| c.is_ascii_hexdigit()) => Some(h.chars().flat_map(|c| [c, c]).collect::<String>().to_ascii_uppercase()),
            6 if h.chars().all(|c| c.is_ascii_hexdigit()) => Some(h.to_ascii_uppercase()),
            8 if h.chars().all(|c| c.is_ascii_hexdigit()) => (u8::from_str_radix(&h[6..8], 16).ok()? > 0).then(|| h[..6].to_ascii_uppercase()),
            _ => None,
        };
    }
    if let Some(args) = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb(")) {
        let args = args.trim_end_matches(')');
        let parts: Vec<&str> = args.split([',', ' ', '/']).filter(|p| !p.is_empty()).collect();
        if parts.len() < 3 {
            return None;
        }
        let ch = |p: &str| -> Option<u8> {
            let v = if let Some(pc) = p.strip_suffix('%') { pc.parse::<f64>().ok()? * 2.55 } else { p.parse::<f64>().ok()? };
            Some(v.round().clamp(0.0, 255.0) as u8)
        };
        if let Some(a) = parts.get(3).and_then(|a| a.trim_end_matches('%').parse::<f64>().ok()) {
            if a <= 0.0 {
                return None;
            }
        }
        return Some(format!("{:02X}{:02X}{:02X}", ch(parts[0])?, ch(parts[1])?, ch(parts[2])?));
    }
    NAMED.iter().find(|(n, _)| *n == s).map(|(_, h)| h.to_string())
}

/// The highlight name Word uses for a colour, when it is exactly one of Word's sixteen.
pub fn highlight_name(hex: &str) -> Option<&'static str> {
    Some(match hex.to_ascii_uppercase().as_str() {
        "FFFF00" => "yellow",
        "00FF00" => "green",
        "00FFFF" => "cyan",
        "FF00FF" => "magenta",
        "0000FF" => "blue",
        "FF0000" => "red",
        "000080" => "darkBlue",
        "008080" => "darkCyan",
        "008000" => "darkGreen",
        "800080" => "darkMagenta",
        "800000" => "darkRed",
        "808000" => "darkYellow",
        "808080" => "darkGray",
        "C0C0C0" => "lightGray",
        "000000" => "black",
        _ => return None,
    })
}

/// A highlight name from `mso-highlight`.
pub fn mso_highlight(name: &str) -> Option<&'static str> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "yellow" => "yellow",
        "lime" | "bright-green" | "green" => "green",
        "aqua" | "cyan" | "turquoise" => "cyan",
        "fuchsia" | "magenta" | "pink" => "magenta",
        "blue" => "blue",
        "red" => "red",
        "navy" | "dark-blue" => "darkBlue",
        "teal" | "dark-cyan" => "darkCyan",
        "dark-green" => "darkGreen",
        "purple" | "violet" | "dark-magenta" => "darkMagenta",
        "maroon" | "dark-red" => "darkRed",
        "olive" | "dark-yellow" => "darkYellow",
        "gray" | "grey" | "dark-gray" => "darkGray",
        "silver" | "light-gray" => "lightGray",
        "black" => "black",
        _ => return None,
    })
}

/// A length in points. `font` is the current font size in points (for em and rem); `base` what percentages refer to.
pub fn length_pt(s: &str, font: f64, base: Option<f64>) -> Option<f64> {
    let s = s.trim().to_ascii_lowercase();
    if s == "0" {
        return Some(0.0);
    }
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(s.len());
    let n: f64 = s[..split].parse().ok()?;
    let unit = s[split..].trim();
    Some(match unit {
        "pt" => n,
        "px" | "" => n * 0.75,
        "in" => n * 72.0,
        "cm" => n * 72.0 / 2.54,
        "mm" => n * 72.0 / 25.4,
        "pc" => n * 12.0,
        "q" => n * 72.0 / 101.6,
        "em" | "rem" => n * font,
        "ex" => n * font * 0.5,
        "%" => n / 100.0 * base?,
        _ => return None,
    })
}

pub fn length_twips(s: &str, font: f64, base: Option<f64>) -> Option<i64> {
    length_pt(s, font, base).map(|p| (p * 20.0).round() as i64)
}

/// `margin: a b c d` style shorthand as top, right, bottom, left.
pub fn box4(v: &str) -> [Option<String>; 4] {
    let p: Vec<&str> = v.split_whitespace().collect();
    let g = |i: usize| p.get(i).map(|s| s.to_string());
    match p.len() {
        0 => [None, None, None, None],
        1 => [g(0), g(0), g(0), g(0)],
        2 => [g(0), g(1), g(0), g(1)],
        3 => [g(0), g(1), g(2), g(1)],
        _ => [g(0), g(1), g(2), g(3)],
    }
}

/// One side of a box property, from the longhand or the shorthand: `side` is 0 top, 1 right, 2 bottom, 3 left.
pub fn side(d: &Decls, prop: &str, side: usize) -> Option<String> {
    const NAMES: [&str; 4] = ["top", "right", "bottom", "left"];
    // Later declarations win, whichever form they are in.
    for (k, v) in d.iter().rev() {
        if *k == format!("{prop}-{}", NAMES[side]) {
            return Some(v.clone());
        }
        if k == prop {
            return box4(v)[side].clone();
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bd {
    /// Word's `w:val`: `single`, `double`, `dotted`, `dashed`.
    pub val: &'static str,
    /// Eighths of a point.
    pub sz: i64,
    /// Hex or `auto`.
    pub color: String,
}

/// A border from `1px solid #ccc` (or `none`).
pub fn parse_border(s: &str) -> Option<Bd> {
    let mut val = None;
    let mut width = None;
    let mut color = None;
    for tok in split_top(s) {
        let t = tok.to_ascii_lowercase();
        match t.as_str() {
            "none" | "hidden" => return None,
            "solid" | "inset" | "outset" | "groove" | "ridge" => val = Some("single"),
            "double" => val = Some("double"),
            "dotted" => val = Some("dotted"),
            "dashed" => val = Some("dashed"),
            "thin" => width = Some(0.75),
            "medium" => width = Some(2.25),
            "thick" => width = Some(3.75),
            _ => {
                if let Some(pt) = length_pt(&t, 12.0, None) {
                    width = Some(pt);
                } else if let Some(c) = parse_color(&t) {
                    color = Some(c);
                }
            }
        }
    }
    let val = val?;
    let pt = width.unwrap_or(0.75);
    if pt <= 0.0 {
        return None;
    }
    Some(Bd { val, sz: ((pt * 8.0).round() as i64).clamp(2, 96), color: color.unwrap_or_else(|| "auto".into()) })
}

/// Split on whitespace outside parentheses.
fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            c if c.is_whitespace() && depth <= 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations() {
        let d = parse_decls("font-family: \"Segoe UI\", sans-serif; color:#F00 !important;background:url(data:image/png;base64,AAA) ; ;");
        assert_eq!(get(&d, "font-family"), Some("\"Segoe UI\", sans-serif"));
        assert_eq!(get(&d, "color"), Some("#F00"));
        assert!(get(&d, "background").unwrap().starts_with("url("));
    }

    #[test]
    fn colours_and_lengths() {
        assert_eq!(parse_color("#abc").as_deref(), Some("AABBCC"));
        assert_eq!(parse_color("rgb(255, 0, 128)").as_deref(), Some("FF0080"));
        assert_eq!(parse_color("rgba(0,0,0,0)"), None);
        assert_eq!(parse_color("windowtext"), None);
        assert_eq!(length_twips("0.5in", 11.0, None), Some(720));
        assert_eq!(length_twips("12pt", 11.0, None), Some(240));
        assert_eq!(length_twips("16px", 11.0, None), Some(240));
        assert_eq!(length_twips("2em", 10.0, None), Some(400));
        assert_eq!(highlight_name("FFFF00"), Some("yellow"));
    }

    #[test]
    fn sheet_rules_and_lists() {
        let mut s = Sheet::default();
        s.add("p.MsoNormal, li.MsoNormal { margin:0in; font-size:11.0pt }\n.x { color: red } @page Section1 { size: 8.5in }\n@list l0:level1 { mso-level-number-format:bullet; mso-level-text:\"\\F0B7\" }\ndiv p { color: blue }");
        let el = HEl { tag: "p".into(), attrs: vec![("class".into(), "MsoNormal x".into()), ("style".into(), "color: green".into())], children: vec![] };
        let d = s.computed(&el);
        assert_eq!(get(&d, "font-size"), Some("11.0pt"));
        assert_eq!(get(&d, "color"), Some("green"));
        assert_eq!(get(s.lists.get("l0:level1").unwrap(), "mso-level-number-format"), Some("bullet"));
        let other = HEl { tag: "span".into(), ..Default::default() };
        assert!(s.computed(&other).is_empty());
    }

    #[test]
    fn borders_and_boxes() {
        let b = parse_border("1px solid #ccc").unwrap();
        assert_eq!((b.val, b.sz, b.color.as_str()), ("single", 6, "CCCCCC"));
        assert_eq!(parse_border("none"), None);
        let d = parse_decls("margin: 0 1in 2pt; margin-left: 5pt");
        assert_eq!(side(&d, "margin", 0).as_deref(), Some("0"));
        assert_eq!(side(&d, "margin", 1).as_deref(), Some("1in"));
        assert_eq!(side(&d, "margin", 3).as_deref(), Some("5pt"));
    }
}
