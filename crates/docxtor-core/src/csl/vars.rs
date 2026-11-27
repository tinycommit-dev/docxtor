//! Docxtor's source model seen as CSL variables: item types, text, names and dates.

use crate::citations::{month_number, nz, Name, Source, SourceType};

/// The CSL item type of a source.
pub fn csl_type(s: &Source) -> &'static str {
    match s.kind {
        SourceType::Book => "book",
        SourceType::Article => "article-journal",
        SourceType::Chapter => "chapter",
        SourceType::Conference => "paper-conference",
        SourceType::Report => "report",
        SourceType::Website => "webpage",
        SourceType::Thesis => "thesis",
        SourceType::Misc => "document",
    }
}

/// A text variable's value, or None when the source has none. Variables Docxtor doesn't keep are always None.
pub fn text(s: &Source, name: &str) -> Option<String> {
    let v = match name {
        "title" => Some(s.title.trim()).filter(|t| !t.is_empty()),
        "container-title" => nz(&s.container_title),
        "publisher" => nz(&s.publisher),
        "publisher-place" => nz(&s.place),
        "volume" => nz(&s.volume),
        "issue" => nz(&s.issue),
        "page" => nz(&s.pages),
        "edition" => nz(&s.edition),
        "URL" => nz(&s.url),
        "DOI" => nz(&s.doi).map(|d| d.trim_start_matches("https://doi.org/").trim_start_matches("http://dx.doi.org/").trim_start_matches("doi:")),
        "genre" => nz(&s.genre),
        "type" => Some(csl_type(s)),
        _ => None,
    };
    v.map(str::to_string)
}

/// The short form of a title: what comes before the first colon or question mark.
pub fn short_title(t: &str) -> String {
    let t = t.trim();
    match t.find([':', '?']) {
        Some(i) if i > 0 && t[..i].chars().any(|c| c.is_alphanumeric()) => {
            // A question mark stays with its question.
            let end = if t[i..].starts_with('?') { i + 1 } else { i };
            t[..end].to_string()
        }
        _ => t.to_string(),
    }
}

/// The names of a name variable.
pub fn names<'a>(s: &'a Source, name: &str) -> &'a [Name] {
    match name {
        "author" => &s.authors,
        "editor" => &s.editors,
        _ => &[],
    }
}

/// A date: numbers when the text parses, else the text as written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Date {
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
    pub literal: Option<String>,
    pub uncertain: bool,
}

fn parse_year(y: &str) -> Option<i32> {
    let t = y.trim();
    if !t.is_empty() && t.len() <= 4 && t.chars().all(|c| c.is_ascii_digit()) {
        t.parse().ok()
    } else {
        None
    }
}

/// `issued` or `accessed`.
pub fn date(s: &Source, name: &str) -> Option<Date> {
    match name {
        "issued" => {
            let y = nz(&s.year)?;
            let month = nz(&s.month).and_then(month_number);
            let day = nz(&s.day).and_then(|d| d.trim().parse::<u32>().ok()).filter(|d| (1..=31).contains(d));
            Some(match parse_year(y) {
                Some(year) => Date { year: Some(year), month, day: day.filter(|_| month.is_some()), ..Default::default() },
                None => {
                    let uncertain = y.contains('?') || y.starts_with("c.") || y.starts_with('~');
                    // "c. 1850" and "1850?" still have a year to show.
                    let digits: String = y.chars().filter(|c| c.is_ascii_digit()).collect();
                    match (uncertain, parse_year(&digits)) {
                        (true, Some(year)) => Date { year: Some(year), month, day: day.filter(|_| month.is_some()), uncertain: true, ..Default::default() },
                        _ => Date { literal: Some(y.to_string()), ..Default::default() },
                    }
                }
            })
        }
        "accessed" => {
            let a = nz(&s.accessed)?;
            let mut it = a.split('-');
            let year = it.next().and_then(parse_year)?;
            let month = it.next().and_then(|m| m.trim().parse::<u32>().ok()).filter(|m| (1..=12).contains(m));
            let day = it.next().and_then(|d| d.trim().parse::<u32>().ok()).filter(|d| (1..=31).contains(d));
            Some(Date { year: Some(year), month, day: day.filter(|_| month.is_some()), ..Default::default() })
        }
        _ => None,
    }
}

/// Whether a value counts as a number in CSL: digits, a range or list of them, with an optional letter
/// before or after each, or an ordinal ("2nd").
pub fn is_numeric(v: &str) -> bool {
    let v = v.trim();
    if v.is_empty() {
        return false;
    }
    let mut any = false;
    for part in v.split(['-', '\u{2013}', '\u{2014}', ',', '&']) {
        let p = part.trim();
        if p.is_empty() {
            return false;
        }
        let digits = p.chars().filter(|c| c.is_ascii_digit()).count();
        let others = p.chars().filter(|c| !c.is_ascii_digit()).count();
        if digits == 0 || others > 3 {
            return false;
        }
        // The letters sit at one end: "B1", "2a", "2nd".
        let lead = p.chars().take_while(|c| !c.is_ascii_digit()).count();
        let trail = p.chars().rev().take_while(|c| !c.is_ascii_digit()).count();
        if lead + trail != others || !p.chars().filter(|c| !c.is_ascii_digit()).all(|c| c.is_alphabetic() || c == '.') {
            return false;
        }
        any = true;
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_values() {
        for yes in ["2", "12", "2nd", "1-3", "S1", "3, 5", "2a", "10\u{2013}12"] {
            assert!(is_numeric(yes), "{yes}");
        }
        for no in ["", "Supplement", "two", "Spring", "2nd edition", "A-B"] {
            assert!(!is_numeric(no), "{no}");
        }
    }

    #[test]
    fn short_titles() {
        assert_eq!(short_title("The Art of Citing: A Guide"), "The Art of Citing");
        assert_eq!(short_title("Why cite? A guide"), "Why cite?");
        assert_eq!(short_title("Plain"), "Plain");
    }
}
