//! Small text helpers: folding, initials, page ranges, DOIs, dates and joining names.

use super::*;

/// A lowercase base letter for sorting and tags: accents removed, combining marks dropped.
pub fn fold_char(c: char) -> Option<char> {
    let c = c.to_lowercase().next().unwrap_or(c);
    Some(match c {
        '\u{300}'..='\u{36F}' => return None,
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => 'c',
        'ď' | 'đ' | 'ð' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
        'ğ' | 'ĝ' | 'ģ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => 'i',
        'ł' | 'ľ' | 'ĺ' => 'l',
        'ñ' | 'ń' | 'ň' | 'ņ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => 'o',
        'ř' | 'ŕ' => 'r',
        'ś' | 'š' | 'ş' | 'ß' => 's',
        'ť' | 'ţ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => 'u',
        'ý' | 'ÿ' => 'y',
        'ź' | 'ž' | 'ż' => 'z',
        c => c,
    })
}

pub(super) fn sort_fold(s: &str) -> String {
    s.chars().filter_map(fold_char).filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>()
}

pub(super) fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split_whitespace()
}

/// The first few words of a title, cut at a colon or question mark (for in-text citations without an author).
pub(super) fn short_title(t: &str) -> String {
    let t = t.trim();
    let cut = t.find([':', '?', '.']).map(|i| &t[..i]).unwrap_or(t);
    let w: Vec<&str> = words(cut).collect();
    if w.len() > 4 {
        w[..4].join(" ")
    } else {
        w.join(" ")
    }
}

/// Initials of a given name: "John Q." gives "J. Q.", "Jean-Paul" gives "J.-P.".
pub fn initials(given: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for tok in words(given) {
        let parts: Vec<String> = tok
            .split('-')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let bare = p.trim_end_matches('.');
                let letters: Vec<char> = bare.chars().filter(|c| c.is_alphabetic()).collect();
                if letters.is_empty() {
                    return String::new();
                }
                // A lowercase word is a particle ("van"); a run of capitals ("JK") or dotted letters ("J.K.") are initials.
                if letters[0].is_lowercase() && letters.len() > 1 {
                    return p.to_string();
                }
                let each = bare.contains('.') || (letters.len() >= 2 && letters.len() <= 3 && letters.iter().all(|c| c.is_uppercase()));
                if each {
                    letters.iter().map(|c| format!("{}.", c.to_uppercase())).collect::<Vec<_>>().join(" ")
                } else {
                    format!("{}.", letters[0].to_uppercase())
                }
            })
            .filter(|p| !p.is_empty())
            .collect();
        if !parts.is_empty() {
            out.push(parts.join("-"));
        }
    }
    out.join(" ")
}

pub(super) fn ordinal(n: u32) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// An edition as it reads before "ed.": "2" and "2nd" give "2nd", "Revised edition" gives "Revised".
pub(super) fn edition_base(e: &str) -> String {
    let e = e.trim();
    if let Ok(n) = e.parse::<u32>() {
        return ordinal(n);
    }
    let lower = e.to_lowercase();
    for suf in [" edition", " edn.", " edn", " ed.", " ed"] {
        if lower.ends_with(suf) {
            return e[..e.len() - suf.len()].trim().to_string();
        }
    }
    e.to_string()
}

/// Page ranges with the right dash between numbers.
pub(super) fn pages_with(p: &str, dash: &str) -> String {
    let mut out = String::new();
    let mut chars = p.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c, '-' | '\u{2013}' | '\u{2014}') {
            while matches!(chars.peek(), Some('-' | '\u{2013}' | '\u{2014}')) {
                chars.next();
            }
            let t = out.trim_end().len();
            out.truncate(t);
            out.push_str(dash);
            while chars.peek() == Some(&' ') {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub(super) fn is_range(p: &str) -> bool {
    p.chars().any(|c| matches!(c, '-' | '\u{2013}' | '\u{2014}' | ','))
}

/// An article number such as `e1234` stands alone: no "p." in front.
pub(super) fn is_article_number(p: &str) -> bool {
    let p = p.trim();
    p.chars().next().is_some_and(|c| c.is_alphabetic()) && p.chars().any(|c| c.is_ascii_digit()) && !is_range(p) && !p.contains(' ')
}

pub(super) fn p_label(p: &str, dash: &str) -> String {
    let p = pages_with(p, dash);
    if is_article_number(&p) {
        p
    } else if is_range(&p) {
        format!("pp. {p}")
    } else {
        format!("p. {p}")
    }
}

pub(super) fn doi_of(d: &str) -> String {
    let d = d.trim();
    for pre in ["https://doi.org/", "http://doi.org/", "https://dx.doi.org/", "http://dx.doi.org/", "doi:", "DOI:", "doi: "] {
        if let Some(rest) = d.strip_prefix(pre) {
            return rest.trim().to_string();
        }
    }
    d.to_string()
}

pub(super) fn doi_link(d: &str) -> String {
    format!("https://doi.org/{}", doi_of(d))
}

pub(super) fn strip_protocol(u: &str) -> &str {
    let u = u.trim();
    u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(u)
}

#[derive(Clone, Default)]
pub(super) struct Dt {
    pub(super) y: Option<String>,
    pub(super) m: Option<u32>,
    pub(super) d: Option<u32>,
}

pub(super) fn day_of(s: &str) -> Option<u32> {
    s.trim().parse::<u32>().ok().filter(|d| (1..=31).contains(d))
}

pub(super) fn pub_dt(s: &Source) -> Dt {
    Dt { y: nz(&s.year).map(String::from), m: nz(&s.month).and_then(month_number), d: nz(&s.day).and_then(day_of) }
}

pub(super) fn acc_dt(s: &Source) -> Option<Dt> {
    let a = nz(&s.accessed)?;
    let mut it = a.split('-');
    let y = it.next()?.trim();
    if y.is_empty() {
        return None;
    }
    let m = it.next().and_then(month_number);
    let d = if m.is_some() { it.next().and_then(day_of) } else { None };
    Some(Dt { y: Some(y.to_string()), m, d })
}

pub(super) const MLA_MONTHS: [&str; 12] = ["Jan.", "Feb.", "Mar.", "Apr.", "May", "June", "July", "Aug.", "Sept.", "Oct.", "Nov.", "Dec."];

pub(super) const IEEE_MONTHS: [&str; 12] = ["Jan.", "Feb.", "Mar.", "Apr.", "May", "Jun.", "Jul.", "Aug.", "Sep.", "Oct.", "Nov.", "Dec."];

pub(super) fn month_full(m: u32) -> &'static str {
    MONTH_NAMES[m as usize - 1]
}

/// "5 Mar. 2020", "Mar. 2020" or "2020".
pub(super) fn mla_date(d: &Dt) -> Option<String> {
    let y = d.y.as_deref()?;
    Some(match (d.m, d.d) {
        (Some(m), Some(day)) => format!("{day} {} {y}", MLA_MONTHS[m as usize - 1]),
        (Some(m), None) => format!("{} {y}", MLA_MONTHS[m as usize - 1]),
        _ => y.to_string(),
    })
}

/// "March 5, 2020", "March 2020" or "2020".
pub(super) fn mdy_date(d: &Dt) -> Option<String> {
    let y = d.y.as_deref()?;
    Some(match (d.m, d.d) {
        (Some(m), Some(day)) => format!("{} {day}, {y}", month_full(m)),
        (Some(m), None) => format!("{} {y}", month_full(m)),
        _ => y.to_string(),
    })
}

/// "5 March 2021".
pub(super) fn dmy_date(d: &Dt) -> Option<String> {
    let y = d.y.as_deref()?;
    Some(match (d.m, d.d) {
        (Some(m), Some(day)) => format!("{day} {} {y}", month_full(m)),
        (Some(m), None) => format!("{} {y}", month_full(m)),
        _ => y.to_string(),
    })
}

pub(super) fn ieee_date(d: &Dt, with_day: bool) -> Option<String> {
    let y = d.y.as_deref()?;
    Some(match (d.m, d.d) {
        (Some(m), Some(day)) if with_day => format!("{} {day}, {y}", IEEE_MONTHS[m as usize - 1]),
        (Some(m), _) => format!("{} {y}", IEEE_MONTHS[m as usize - 1]),
        _ => y.to_string(),
    })
}

pub(super) fn join_and(items: &[String], and: &str, oxford: bool) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} {and} {}", items[0], items[1]),
        n => {
            let head = items[..n - 1].join(", ");
            format!("{head}{} {and} {}", if oxford { "," } else { "" }, items[n - 1])
        }
    }
}
