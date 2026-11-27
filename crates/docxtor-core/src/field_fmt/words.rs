//! `\*` formats: case changes and numbers in words.

use super::*;

pub(super) fn upper_first_alnum(s: &str) -> String {
    let mut done = false;
    let mut out = String::new();
    for c in s.chars() {
        if !done && c.is_alphanumeric() {
            out.extend(c.to_uppercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// The `\*` case formats: `Upper`, `Lower`, `FirstCap` (the first letter of the text capital, the rest lowercase)
/// and `Caps` (the first letter of every word capital, the rest lowercase). The name is matched without regard to
/// case. `None` for names that are not case formats.
pub fn apply_case(text: &str, how: &str) -> Option<String> {
    match how.to_ascii_lowercase().as_str() {
        "upper" => Some(text.to_uppercase()),
        "lower" => Some(text.to_lowercase()),
        "firstcap" => Some(upper_first_alnum(&text.to_lowercase())),
        "caps" => {
            let mut out = String::new();
            let mut start = true;
            for c in text.chars() {
                if c.is_alphanumeric() {
                    if start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    start = false;
                } else {
                    // An apostrophe inside a word does not start a new one.
                    if !matches!(c, '\'' | '\u{2019}') || start {
                        start = true;
                    }
                    out.push(c);
                }
            }
            Some(out)
        }
        _ => None,
    }
}

pub(super) const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen",
    "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];

pub(super) const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

pub(super) const SCALES: [&str; 7] = ["", "thousand", "million", "billion", "trillion", "quadrillion", "quintillion"];

pub(super) fn below_thousand(n: u64) -> String {
    let mut parts: Vec<String> = Vec::new();
    if n >= 100 {
        parts.push(format!("{} hundred", ONES[(n / 100) as usize]));
    }
    let r = n % 100;
    if r >= 20 {
        let t = TENS[(r / 10) as usize];
        if !r.is_multiple_of(10) {
            parts.push(format!("{t}-{}", ONES[(r % 10) as usize]));
        } else {
            parts.push(t.to_string());
        }
    } else if r > 0 {
        parts.push(ONES[r as usize].to_string());
    }
    parts.join(" ")
}

/// English cardinal words with no "and": 123 is "one hundred twenty-three".
pub(super) fn cardinal(n: u64) -> String {
    if n == 0 {
        return "zero".to_string();
    }
    let mut groups: Vec<u64> = Vec::new();
    let mut r = n;
    while r > 0 {
        groups.push(r % 1000);
        r /= 1000;
    }
    let mut parts: Vec<String> = Vec::new();
    for (i, g) in groups.iter().enumerate().rev() {
        if *g == 0 {
            continue;
        }
        let words = below_thousand(*g);
        if i == 0 {
            parts.push(words);
        } else {
            parts.push(format!("{words} {}", SCALES[i]));
        }
    }
    parts.join(" ")
}

/// Turns the last word of cardinal text into its ordinal: "twenty-three" becomes "twenty-third".
pub(super) fn ordinal_words(card: &str) -> String {
    let split = card.rfind([' ', '-']).map_or(0, |i| i + 1);
    let (head, last) = card.split_at(split);
    let ord = match last {
        "one" => "first".to_string(),
        "two" => "second".to_string(),
        "three" => "third".to_string(),
        "five" => "fifth".to_string(),
        "eight" => "eighth".to_string(),
        "nine" => "ninth".to_string(),
        "twelve" => "twelfth".to_string(),
        w if w.ends_with('y') => format!("{}ieth", &w[..w.len() - 1]),
        w => format!("{w}th"),
    };
    format!("{head}{ord}")
}

/// The `\*` number formats applied to a number, which is rounded to the nearest integer except for `DollarText`.
///
/// `Arabic`, `roman`, `ROMAN`, `alphabetic`, `ALPHABETIC`, `Ordinal` (1st, 2nd, 11th), `CardText` ("one hundred
/// twenty-three"), `OrdText` ("twenty-third"), `Hex` (uppercase) and `DollarText` ("one hundred twenty-three and
/// 45/100"). `roman`, `ROMAN`, `alphabetic` and `ALPHABETIC` are case-sensitive and the others are not.
/// `None` for names that are not number formats, such as `CHARFORMAT` and `MERGEFORMAT`.
pub fn number_format(value: f64, how: &str) -> Option<String> {
    let n = value.round() as i64;
    match how {
        "roman" => return Some(format_number(n, "lowerRoman")),
        "ROMAN" => return Some(format_number(n, "upperRoman")),
        "alphabetic" => return Some(format_number(n, "lowerLetter")),
        "ALPHABETIC" => return Some(format_number(n, "upperLetter")),
        _ => {}
    }
    let sign = if n < 0 { "-" } else { "" };
    let mag = n.unsigned_abs();
    match how.to_ascii_lowercase().as_str() {
        "arabic" => Some(n.to_string()),
        "ordinal" => Some(format!("{sign}{}", format_number(mag as i64, "ordinal"))),
        "cardtext" | "ordtext" => {
            let mut words = cardinal(mag);
            if how.eq_ignore_ascii_case("ordtext") {
                words = ordinal_words(&words);
            }
            Some(if n < 0 { format!("minus {words}") } else { words })
        }
        "hex" => Some(format!("{sign}{mag:X}")),
        "dollartext" => {
            if !value.is_finite() {
                return Some("zero and 00/100".to_string());
            }
            let (int, frac) = decimal_parts(value.abs());
            let (int, frac) = round_parts(int, frac, 2);
            let whole = int.iter().fold(0u64, |a, d| a.saturating_mul(10).saturating_add(u64::from(*d)));
            let cents = u32::from(frac[0]) * 10 + u32::from(frac[1]);
            let words = cardinal(whole);
            let prefix = if value < 0.0 && (whole > 0 || cents > 0) { "minus " } else { "" };
            Some(format!("{prefix}{words} and {cents:02}/100"))
        }
        _ => None,
    }
}
