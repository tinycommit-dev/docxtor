//! Text helpers for the processor: text case, page ranges, joining pieces with the punctuation rules
//! a style relies on, and the clean-up of the finished text.

use super::model::TextCase;
use crate::cite_styles::{Rich, Span};

#[derive(Clone, Copy, PartialEq)]
enum Act {
    Keep,
    Upper,
    Lower,
}

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "but", "by", "down", "for", "from", "in", "into", "nor", "of", "on", "onto", "or", "over", "so", "the", "till", "to", "up", "via", "with", "yet",
];

fn first_alpha(word: &[char]) -> Option<usize> {
    word.iter().position(|c| c.is_alphabetic())
}

fn decide(chars: &[char], case: TextCase) -> Vec<Act> {
    let mut acts = vec![Act::Keep; chars.len()];
    match case {
        TextCase::Lowercase => acts.iter_mut().for_each(|a| *a = Act::Lower),
        TextCase::Uppercase => acts.iter_mut().for_each(|a| *a = Act::Upper),
        TextCase::CapitalizeFirst | TextCase::Sentence => {
            if let Some(i) = first_alpha(chars) {
                acts[i] = Act::Upper;
            }
        }
        TextCase::CapitalizeAll | TextCase::Title => {
            // Words are runs of non-space characters.
            let mut words: Vec<(usize, usize)> = Vec::new();
            let mut i = 0;
            while i < chars.len() {
                if chars[i].is_whitespace() {
                    i += 1;
                    continue;
                }
                let start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                words.push((start, i));
            }
            let last = words.len().saturating_sub(1);
            let mut after_colon = true;
            for (n, (s, e)) in words.iter().copied().enumerate() {
                let w = &chars[s..e];
                let Some(fa) = first_alpha(w) else {
                    after_colon = false;
                    continue;
                };
                let first_of_text = n == 0 || after_colon;
                after_colon = w.last() == Some(&':');
                if case == TextCase::CapitalizeAll {
                    acts[s + fa] = Act::Upper;
                    continue;
                }
                // A word with a capital after its first letter (iPhone, DNA) is left as it is.
                if w.iter().skip(fa + 1).any(|c| c.is_uppercase()) {
                    continue;
                }
                let core: String = w.iter().filter(|c| c.is_alphabetic()).collect::<String>().to_lowercase();
                if STOP_WORDS.contains(&core.as_str()) && !first_of_text && n != last {
                    acts[s + fa] = Act::Lower;
                } else {
                    acts[s + fa] = Act::Upper;
                }
            }
        }
    }
    acts
}

pub fn apply_case(r: &mut Rich, case: TextCase) {
    let chars: Vec<char> = r.0.iter().flat_map(|s| s.text.chars()).collect();
    let acts = decide(&chars, case);
    let mut k = 0;
    for span in r.0.iter_mut() {
        let mut out = String::with_capacity(span.text.len());
        for c in span.text.chars() {
            match acts[k] {
                Act::Keep => out.push(c),
                Act::Upper => out.extend(c.to_uppercase()),
                Act::Lower => out.extend(c.to_lowercase()),
            }
            k += 1;
        }
        span.text = out;
    }
}

pub fn strip_periods(r: &mut Rich) {
    for s in r.0.iter_mut() {
        s.text = s.text.replace('.', "");
    }
    r.0.retain(|s| !s.text.is_empty());
}

/// Apply a look to every run: `Some(true)` sets it, `Some(false)` clears it.
pub fn set_look(r: &mut Rich, italic: Option<bool>, bold: Option<bool>, small_caps: Option<bool>, sup: Option<bool>) {
    for s in r.0.iter_mut() {
        if let Some(v) = italic {
            s.italic = v;
        }
        if let Some(v) = bold {
            s.bold = v;
        }
        if let Some(v) = small_caps {
            s.small_caps = v;
        }
        if let Some(v) = sup {
            s.sup = v;
        }
    }
    // Neighbours that now look alike become one run.
    let spans = std::mem::take(&mut r.0);
    for s in spans {
        r.push_span(s);
    }
}

// ---------------------------------------------------------------------------
// Page ranges
// ---------------------------------------------------------------------------

fn expand(a: &str, b: &str) -> String {
    if b.len() < a.len() {
        format!("{}{}", &a[..a.len() - b.len()], b)
    } else {
        b.to_string()
    }
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

fn shorten(a: &str, b: &str, keep: usize) -> String {
    let p = common_prefix(a, b).min(b.len().saturating_sub(keep));
    b[p..].to_string()
}

fn chicago(a: &str, b: &str) -> String {
    let (Ok(av), Ok(bv)) = (a.parse::<u64>(), b.parse::<u64>()) else { return b.to_string() };
    if av < 100 || av % 100 == 0 || bv < av {
        return b.to_string();
    }
    if av / 100 != bv / 100 && av % 100 >= 10 {
        // The second number runs into the next hundred (1496-1500): both in full.
        return b.to_string();
    }
    if (1..=9).contains(&(av % 100)) {
        return shorten(a, b, 1);
    }
    shorten(a, b, 2)
}

/// A page value with its ranges written in the style's range format (`expanded`, `minimal`, `minimal-two`,
/// `chicago`) and `delimiter` between the two ends. Other text stays as it is.
pub fn page_range(value: &str, format: Option<&str>, delimiter: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            let s = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let a: String = chars[s..i].iter().collect();
            // A range: digits, a hyphen or dash, digits.
            let mut j = i;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            if j < chars.len() && matches!(chars[j], '-' | '\u{2013}' | '\u{2014}') {
                let mut k = j + 1;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                let bs = k;
                while k < chars.len() && chars[k].is_ascii_digit() {
                    k += 1;
                }
                if k > bs {
                    let b: String = chars[bs..k].iter().collect();
                    let full = expand(&a, &b);
                    let end = match format.unwrap_or("") {
                        "expanded" => full,
                        "minimal" => shorten(&a, &full, 1),
                        "minimal-two" => shorten(&a, &full, 2),
                        "chicago" | "chicago-15" | "chicago-16" => chicago(&a, &full),
                        _ => b,
                    };
                    out.push_str(&a);
                    out.push_str(delimiter);
                    out.push_str(&end);
                    i = k;
                    continue;
                }
            }
            out.push_str(&a);
            continue;
        }
        if matches!(chars[i], '-' | '\u{2013}') && i > 0 && i + 1 < chars.len() && chars[i - 1].is_alphanumeric() && chars[i + 1].is_alphanumeric() {
            out.push_str(delimiter);
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Joining
// ---------------------------------------------------------------------------

fn sentence_end(c: char) -> bool {
    matches!(c, '.' | '?' | '!')
}

/// Add a piece of punctuation or spacing (a prefix, a suffix, a delimiter) after the text so far, without
/// doubling what is already there: a full stop after a full stop, a space after a space.
pub fn push_chunk(out: &mut Rich, chunk: &str) {
    if chunk.is_empty() {
        return;
    }
    let mut chunk = chunk;
    if let Some(last) = out.last_char() {
        let first = chunk.chars().next().unwrap_or(' ');
        if first == '.' && sentence_end(last) {
            chunk = &chunk[1..];
        } else if first == '.' && matches!(last, '\u{201d}' | '"') {
            // `."` or `?"` already ends the sentence inside the quotes.
            let plain = out.plain();
            let before = plain.trim_end_matches(['\u{201d}', '"']);
            if before.chars().last().is_some_and(sentence_end) {
                chunk = &chunk[1..];
            }
        } else if first == ' ' && last == ' ' {
            chunk = &chunk[1..];
        }
    }
    if !chunk.is_empty() {
        out.t(chunk);
    }
}

/// The pieces that have something in them, joined with a delimiter.
pub fn join(parts: Vec<Rich>, delimiter: &str) -> Rich {
    let mut out = Rich::new();
    let mut first = true;
    for p in parts {
        if p.is_empty() {
            continue;
        }
        if !first {
            push_chunk(&mut out, delimiter);
        }
        first = false;
        out.append(&p);
    }
    out
}

/// Clean-up of finished text: a comma or full stop after a closing quote goes inside it, doubled full stops
/// and spaces collapse.
pub fn finish(r: &mut Rich) {
    // Flatten to characters with the run each came from.
    let mut cs: Vec<(char, usize)> = Vec::new();
    for (i, s) in r.0.iter().enumerate() {
        cs.extend(s.text.chars().map(|c| (c, i)));
    }
    let mut out: Vec<(char, usize)> = Vec::with_capacity(cs.len());
    let mut i = 0;
    while i < cs.len() {
        let (c, run) = cs[i];
        // Closing quote then , or . : put the mark inside.
        if c == '\u{201d}' && i + 1 < cs.len() && matches!(cs[i + 1].0, ',' | '.') {
            let mark = cs[i + 1].0;
            // After a sentence mark inside the quote, a full stop is not needed.
            let inside_ends = out.last().is_some_and(|(p, _)| sentence_end(*p));
            // A comma inside the quote gives way to the full stop that ends the text.
            if mark == '.' && out.last().is_some_and(|(p, _)| *p == ',') {
                out.pop();
            }
            if !(mark == '.' && inside_ends) {
                out.push((mark, run));
            }
            out.push((c, run));
            i += 2;
            continue;
        }
        // Two full stops in a row (but not an ellipsis of three).
        if c == '.' && i + 1 < cs.len() && cs[i + 1].0 == '.' && (i + 2 >= cs.len() || cs[i + 2].0 != '.') && out.last().map(|(p, _)| *p) != Some('.') {
            out.push((c, run));
            i += 2;
            continue;
        }
        // A full stop after ? or !
        if c == '.' && out.last().is_some_and(|(p, _)| matches!(*p, '?' | '!')) {
            i += 1;
            continue;
        }
        if c == ' ' && out.last().is_some_and(|(p, _)| *p == ' ') {
            i += 1;
            continue;
        }
        out.push((c, run));
        i += 1;
    }
    let templates: Vec<Span> = r.0.clone();
    let mut rebuilt = Rich::new();
    let mut cur: Option<(usize, String)> = None;
    for (c, run) in out {
        match &mut cur {
            Some((r0, s)) if *r0 == run => s.push(c),
            _ => {
                if let Some((r0, s)) = cur.take() {
                    rebuilt.push_span(Span { text: s, ..templates[r0].clone() });
                }
                cur = Some((run, c.to_string()));
            }
        }
    }
    if let Some((r0, s)) = cur {
        rebuilt.push_span(Span { text: s, ..templates[r0].clone() });
    }
    *r = rebuilt;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges() {
        assert_eq!(page_range("321-28", Some("expanded"), "\u{2013}"), "321\u{2013}328");
        assert_eq!(page_range("321-328", Some("minimal"), "\u{2013}"), "321\u{2013}8");
        assert_eq!(page_range("321-328", Some("minimal-two"), "\u{2013}"), "321\u{2013}28");
        assert_eq!(page_range("42-45", Some("minimal"), "\u{2013}"), "42\u{2013}5");
        assert_eq!(page_range("42-45", Some("minimal-two"), "\u{2013}"), "42\u{2013}45");
        assert_eq!(page_range("321-328", Some("chicago"), "\u{2013}"), "321\u{2013}28");
        assert_eq!(page_range("101-108", Some("chicago"), "\u{2013}"), "101\u{2013}8");
        assert_eq!(page_range("3-10", Some("chicago"), "\u{2013}"), "3\u{2013}10");
        assert_eq!(page_range("1496-1500", Some("chicago"), "\u{2013}"), "1496\u{2013}1500");
        assert_eq!(page_range("100-104", Some("chicago"), "\u{2013}"), "100\u{2013}104");
        assert_eq!(page_range("45-67", None, "\u{2013}"), "45\u{2013}67");
        assert_eq!(page_range("e123", None, "\u{2013}"), "e123");
        assert_eq!(page_range("S1-S3", None, "\u{2013}"), "S1\u{2013}S3");
        assert_eq!(page_range("12, 15-17", Some("expanded"), "-"), "12, 15-17");
    }

    #[test]
    fn cases() {
        let mut r = Rich::plain_of("the art of citing: a guide for the new reader");
        apply_case(&mut r, TextCase::Title);
        assert_eq!(r.plain(), "The Art of Citing: A Guide for the New Reader");
        let mut r = Rich::plain_of("a study of the iPhone and DNA");
        apply_case(&mut r, TextCase::Title);
        assert_eq!(r.plain(), "A Study of the iPhone and DNA");
        let mut r = Rich::plain_of("page");
        apply_case(&mut r, TextCase::CapitalizeFirst);
        assert_eq!(r.plain(), "Page");
    }

    #[test]
    fn punctuation_is_not_doubled() {
        let mut r = Rich::plain_of("Smith et al.");
        push_chunk(&mut r, ". ");
        assert_eq!(r.plain(), "Smith et al. ");
        let mut r = Rich::plain_of("Title");
        r.t("\u{201d}");
        let mut q = Rich::plain_of("\u{201c}Title,");
        q.t("\u{201d}.");
        finish(&mut q);
        assert_eq!(q.plain(), "\u{201c}Title.\u{201d}");
        let mut q = Rich::plain_of("\u{201c}Title\u{201d}, 23.");
        finish(&mut q);
        assert_eq!(q.plain(), "\u{201c}Title,\u{201d} 23.");
        let mut q = Rich::plain_of("A  b..");
        finish(&mut q);
        assert_eq!(q.plain(), "A b.");
    }
}
