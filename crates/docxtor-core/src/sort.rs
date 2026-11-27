//! Sorting text by a key: as text, as a number or as a date, by the whole line or by a field in it.
//!
//! Paragraph sorting (`Op::SortParagraphs`) uses this, and so can table sorting: `order` takes the lines
//! (or the cells of a column) as plain strings and says in which order they go.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// How a key is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortType {
    /// Letter by letter, ignoring case and accents unless asked not to.
    #[default]
    Text,
    /// By the first number in the text (`$1,250.50`, `-3`, `(4)` is minus four); text without one sorts first.
    Number,
    /// By the date in the text (`2024-03-09`, `3/9/2024`, `9 March 2024`, `March 9, 2024`), then the time.
    Date,
}

/// Where a key reads its text from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Separator {
    /// Fields are separated by tabs.
    #[default]
    Tabs,
    Commas,
    /// Fields are separated by another character (`SortOptions::separator_char`).
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SortKey {
    #[serde(rename = "type")]
    pub kind: SortType,
    pub descending: bool,
    /// 0 sorts by the whole line; n sorts by the nth field of it (counting from 1).
    pub field: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SortOptions {
    /// Up to three keys; the next breaks ties in the one before. None sorts by the line, as text, ascending.
    pub keys: Vec<SortKey>,
    /// Capital letters sort apart from small ones (small first) and accents count.
    pub case_sensitive: bool,
    /// The first line is a header: it stays where it is.
    pub header: bool,
    pub separator: Separator,
    /// The character for `Separator::Other`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_char: Option<char>,
}

impl SortOptions {
    /// The keys to use: those given, at most three, or the line as text.
    pub fn effective_keys(&self) -> Vec<SortKey> {
        if self.keys.is_empty() {
            vec![SortKey::default()]
        } else {
            self.keys.iter().take(3).cloned().collect()
        }
    }
}

/// The text a key reads from a line: all of it, or one field.
pub fn field_of<'a>(line: &'a str, key: &SortKey, opts: &SortOptions) -> &'a str {
    if key.field == 0 {
        return line;
    }
    let sep = match opts.separator {
        Separator::Tabs => '\t',
        Separator::Commas => ',',
        Separator::Other => opts.separator_char.unwrap_or(';'),
    };
    line.split(sep).nth(key.field - 1).unwrap_or("").trim()
}

/// What a key read from a line, ready to compare.
#[derive(Debug, Clone, PartialEq)]
pub enum SortValue {
    Text(String, String),
    Number(f64),
    /// Seconds since 1 January 1970.
    Date(i64),
    /// No number or date in the text: sorts first.
    None,
}

pub fn value(text: &str, kind: SortType, case_sensitive: bool) -> SortValue {
    match kind {
        SortType::Text if case_sensitive => {
            // Accents count, and a small letter sorts before the same capital: swapping the case does that.
            let swapped = text.chars().map(|c| if c.is_lowercase() { c.to_uppercase().next().unwrap_or(c) } else { c.to_lowercase().next().unwrap_or(c) }).collect();
            SortValue::Text(text.to_lowercase(), swapped)
        }
        SortType::Text => SortValue::Text(fold(text), text.to_lowercase()),
        SortType::Number => first_number(text).map_or(SortValue::None, SortValue::Number),
        SortType::Date => parse_date(text).map_or(SortValue::None, SortValue::Date),
    }
}

fn cmp_values(a: &SortValue, b: &SortValue) -> Ordering {
    match (a, b) {
        (SortValue::Text(a1, a2), SortValue::Text(b1, b2)) => a1.cmp(b1).then_with(|| a2.cmp(b2)),
        (SortValue::Number(a), SortValue::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
        (SortValue::Date(a), SortValue::Date(b)) => a.cmp(b),
        (SortValue::None, SortValue::None) => Ordering::Equal,
        (SortValue::None, _) => Ordering::Less,
        (_, SortValue::None) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

/// The positions of `lines` in sorted order. Equal lines keep their order.
pub fn order(lines: &[String], opts: &SortOptions) -> Vec<usize> {
    let keys = opts.effective_keys();
    let values: Vec<Vec<SortValue>> = lines.iter().map(|l| keys.iter().map(|k| value(field_of(l, k, opts), k.kind, opts.case_sensitive)).collect()).collect();
    let mut idx: Vec<usize> = (0..lines.len()).collect();
    idx.sort_by(|&a, &b| {
        for (k, key) in keys.iter().enumerate() {
            let c = cmp_values(&values[a][k], &values[b][k]);
            let c = if key.descending { c.reverse() } else { c };
            if c != Ordering::Equal {
                return c;
            }
        }
        Ordering::Equal
    });
    idx
}

/// Letters compared without case and accents, so Émile sorts with the E's.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| {
            let base = match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
                'ç' | 'Ç' => 'c',
                'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
                'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
                'ñ' | 'Ñ' => 'n',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
                'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
                'ý' | 'ÿ' | 'Ý' => 'y',
                other => other,
            };
            base.to_lowercase().next().unwrap_or(base)
        })
        .collect()
}

/// The first number in a text: `12`, `-3.5`, `1,250.50`, `$7`, `(4)` for minus four.
pub fn first_number(text: &str) -> Option<f64> {
    let chars: Vec<char> = text.chars().collect();
    let start = chars.iter().position(|c| c.is_ascii_digit())?;
    let mut end = start;
    while end < chars.len() && (chars[end].is_ascii_digit() || ((chars[end] == ',' || chars[end] == '.') && chars.get(end + 1).is_some_and(|c| c.is_ascii_digit()))) {
        end += 1;
    }
    let token: String = chars[start..end].iter().collect();
    // Commas are thousands separators when they stand before three digits; otherwise a comma is the decimal mark.
    let cleaned = if token.contains('.') || (token.matches(',').count() > 1) || token.split(',').skip(1).all(|g| g.len() == 3) {
        token.replace(',', "")
    } else {
        token.replace(',', ".")
    };
    let n: f64 = cleaned.parse().ok()?;
    let before: String = chars[..start].iter().collect();
    let negative = before.trim_end_matches(['$', '€', '£', '¥', ' ']).ends_with(['-', '−', '(']);
    Some(if negative { -n } else { n })
}

const MONTHS: [(&str, &str); 12] = [
    ("jan", "january"),
    ("feb", "february"),
    ("mar", "march"),
    ("apr", "april"),
    ("may", "may"),
    ("jun", "june"),
    ("jul", "july"),
    ("aug", "august"),
    ("sep", "september"),
    ("oct", "october"),
    ("nov", "november"),
    ("dec", "december"),
];

/// Days since 1 January 1970 of a civil date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn valid(y: i64, m: i64, d: i64) -> bool {
    (1..=12).contains(&m) && (1..=31).contains(&d) && y > 0
}

/// A date (and a time after it) in a text, as seconds since 1970. A numeric date with the day first is
/// recognised when the first number can't be a month (`25/12/2024`); otherwise the month comes first.
pub fn parse_date(text: &str) -> Option<i64> {
    // Split into words and numbers.
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut kind = 0; // 1 digits, 2 letters
    for c in text.chars() {
        let k = if c.is_ascii_digit() { 1 } else if c.is_alphabetic() { 2 } else { 0 };
        if k != kind && !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
        kind = k;
        if k != 0 {
            cur.push(c);
        } else if matches!(c, '/' | '-' | '.' | ':') {
            tokens.push(c.to_string());
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    let num = |t: &String| t.parse::<i64>().ok().filter(|_| t.chars().all(|c| c.is_ascii_digit()));
    let month_of = |t: &String| {
        let t = t.to_lowercase();
        MONTHS.iter().position(|(short, full)| t == *short || t == *full || (t == "sept" && *short == "sep")).map(|i| i as i64 + 1)
    };
    let mut date = None;
    let mut used = 0;
    'scan: for i in 0..tokens.len() {
        // 2024-03-09, 3/9/2024, 25.12.2024
        if tokens.len() >= i + 5 {
            if let (Some(a), Some(b), Some(c)) = (num(&tokens[i]), num(&tokens[i + 2]), num(&tokens[i + 4])) {
                if matches!(tokens[i + 1].as_str(), "/" | "-" | ".") && tokens[i + 1] == tokens[i + 3] {
                    let (y, m, d) = if tokens[i].len() == 4 { (a, b, c) } else if a > 12 { (c, b, a) } else { (c, a, b) };
                    let y = if y < 100 { 2000 + y } else { y };
                    if valid(y, m, d) {
                        date = Some(days_from_civil(y, m, d));
                        used = i + 5;
                        break 'scan;
                    }
                }
            }
        }
        // 9 March 2024, March 9, 2024, March 9 2024
        if let Some(m) = month_of(&tokens[i]).filter(|_| tokens[i].chars().all(|c| c.is_alphabetic())) {
            let before = if i > 0 { num(&tokens[i - 1]) } else { None };
            let rest: Vec<i64> = tokens[i + 1..].iter().filter_map(num).take(2).collect();
            if let (Some(d), Some(y)) = (before.filter(|d| *d <= 31), rest.first()) {
                if valid(*y, m, d) {
                    date = Some(days_from_civil(*y, m, d));
                    used = i + 2;
                    break 'scan;
                }
            }
            if let [d, y] = rest[..] {
                if valid(y, m, d) {
                    date = Some(days_from_civil(y, m, d));
                    used = i + 2;
                    break 'scan;
                }
            }
        }
    }
    let days = date?;
    // A time of day after the date: 14:30 or 14:30:15, with am or pm.
    let mut secs = 0;
    let rest = &tokens[used.min(tokens.len())..];
    if let Some(at) = (0..rest.len().saturating_sub(2)).find(|&i| num(&rest[i]).is_some() && rest[i + 1] == ":" && num(&rest[i + 2]).is_some()) {
        let h = num(&rest[at]).unwrap_or(0);
        let m = num(&rest[at + 2]).unwrap_or(0);
        let s = if rest.get(at + 3).is_some_and(|t| t == ":") { rest.get(at + 4).and_then(num).unwrap_or(0) } else { 0 };
        let pm = rest.iter().any(|t| t.eq_ignore_ascii_case("pm"));
        let am = rest.iter().any(|t| t.eq_ignore_ascii_case("am"));
        let h = if pm && h < 12 { h + 12 } else if am && h == 12 { 0 } else { h };
        secs = h * 3600 + m * 60 + s;
    }
    Some(days * 86400 + secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn sorted(v: &[&str], opts: &SortOptions) -> Vec<String> {
        let l = lines(v);
        order(&l, opts).into_iter().map(|i| l[i].clone()).collect()
    }

    #[test]
    fn text_sorts_ignore_case_and_accents() {
        let o = SortOptions::default();
        assert_eq!(sorted(&["pear", "Apple", "émile", "banana", "Eric"], &o), ["Apple", "banana", "émile", "Eric", "pear"]);
        let desc = SortOptions { keys: vec![SortKey { descending: true, ..Default::default() }], ..Default::default() };
        assert_eq!(sorted(&["b", "a", "c"], &desc), ["c", "b", "a"]);
        // Case sensitive: a small letter sorts before the same capital.
        let cs = SortOptions { case_sensitive: true, ..Default::default() };
        assert_eq!(sorted(&["b", "B", "a", "A"], &cs), ["a", "A", "b", "B"]);
        // As characters, 10 comes before 2.
        assert_eq!(sorted(&["2", "10", "1"], &o), ["1", "10", "2"]);
        // Equal lines keep their order.
        let l = lines(&["x1", "X1"]);
        assert_eq!(order(&l, &o), [0, 1]);
    }

    #[test]
    fn numbers_are_read_from_the_text() {
        assert_eq!(first_number("Item 12"), Some(12.0));
        assert_eq!(first_number("$1,250.50 total"), Some(1250.5));
        assert_eq!(first_number("-3.5"), Some(-3.5));
        assert_eq!(first_number("owes (40)"), Some(-40.0));
        assert_eq!(first_number("3,5"), Some(3.5));
        assert_eq!(first_number("1,000"), Some(1000.0));
        assert_eq!(first_number("none"), None);
        let o = SortOptions { keys: vec![SortKey { kind: SortType::Number, ..Default::default() }], ..Default::default() };
        assert_eq!(sorted(&["10 apples", "2 pears", "no number", "-1 degrees"], &o), ["no number", "-1 degrees", "2 pears", "10 apples"]);
    }

    #[test]
    fn dates_are_read_in_common_forms() {
        let d = |s: &str| parse_date(s).unwrap();
        assert_eq!(d("1970-01-02"), 86400);
        assert_eq!(d("2024-03-09"), d("3/9/2024"));
        assert_eq!(d("2024-03-09"), d("9 March 2024"));
        assert_eq!(d("2024-03-09"), d("March 9, 2024"));
        assert_eq!(d("2024-03-09"), d("Mar 9 2024"));
        assert_eq!(d("2024-12-25"), d("25/12/2024"));
        assert_eq!(d("2024-03-09 14:30"), d("2024-03-09") + 14 * 3600 + 30 * 60);
        assert_eq!(d("2024-03-09 2:30 PM"), d("2024-03-09 14:30"));
        assert!(parse_date("no date here").is_none());
        let o = SortOptions { keys: vec![SortKey { kind: SortType::Date, ..Default::default() }], ..Default::default() };
        assert_eq!(sorted(&["Due 5 June 2024", "Due 1 May 2023", "later", "Due 2024-01-31"], &o), ["later", "Due 1 May 2023", "Due 2024-01-31", "Due 5 June 2024"]);
    }

    #[test]
    fn fields_and_second_keys() {
        let o = SortOptions {
            keys: vec![SortKey { field: 2, ..Default::default() }, SortKey { field: 1, descending: true, ..Default::default() }],
            ..Default::default()
        };
        assert_eq!(sorted(&["a\tx", "b\ty", "c\tx", "d\t"], &o), ["d\t", "c\tx", "a\tx", "b\ty"]);
        let commas = SortOptions { keys: vec![SortKey { field: 2, kind: SortType::Number, ..Default::default() }], separator: Separator::Commas, ..Default::default() };
        assert_eq!(sorted(&["a, 10", "b, 9", "c, 100"], &commas), ["b, 9", "a, 10", "c, 100"]);
        let other = SortOptions { keys: vec![SortKey { field: 2, ..Default::default() }], separator: Separator::Other, separator_char: Some('|'), ..Default::default() };
        assert_eq!(sorted(&["1|z", "2|a"], &other), ["2|a", "1|z"]);
        // More than three keys: the first three count.
        assert_eq!(SortOptions { keys: vec![SortKey::default(); 5], ..Default::default() }.effective_keys().len(), 3);
    }
}
