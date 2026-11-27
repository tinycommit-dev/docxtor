//! A pragmatic Markdown subset used as the content language for AI edits and paste.

use crate::numbering::ListKind;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MdSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    pub code: bool,
    pub link: Option<String>,
    /// An equation: `text` is its LaTeX and this says whether it is a display equation (`$$ ... $$`).
    pub math: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MdBlock {
    Para { style: Option<String>, spans: Vec<MdSpan> },
    /// `check` marks a GFM task item: `Some(false)` for `- [ ]`, `Some(true)` for `- [x]`.
    ListItem { kind: ListKind, level: usize, check: Option<bool>, spans: Vec<MdSpan> },
    Code { lines: Vec<String> },
    Table { rows: Vec<Vec<Vec<MdSpan>>> },
    PageBreak,
}

#[derive(Clone, Copy, Default)]
struct State {
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
}

pub fn parse_inline(s: &str) -> Vec<MdSpan> {
    let mut out = Vec::new();
    inline_into(s, State::default(), None, &mut out);
    // merge adjacent identical spans
    let mut merged: Vec<MdSpan> = Vec::new();
    for sp in out {
        if sp.text.is_empty() {
            continue;
        }
        if let Some(last) = merged.last_mut() {
            if last.math.is_none() && sp.math.is_none() && last.bold == sp.bold && last.italic == sp.italic && last.strike == sp.strike && last.underline == sp.underline && last.code == sp.code && last.link == sp.link {
                last.text.push_str(&sp.text);
                continue;
            }
        }
        merged.push(sp);
    }
    merged
}

fn inline_into(s: &str, mut st: State, link: Option<&str>, out: &mut Vec<MdSpan>) {
    let chars: Vec<char> = s.chars().collect();
    let mut buf = String::new();
    let flush = |buf: &mut String, st: &State, out: &mut Vec<MdSpan>| {
        if !buf.is_empty() {
            out.push(MdSpan {
                text: std::mem::take(buf),
                bold: st.bold,
                italic: st.italic,
                strike: st.strike,
                underline: st.underline,
                code: false,
                link: link.map(String::from),
                math: None,
            });
        }
    };
    let rest = |i: usize| -> String { chars[i..].iter().collect() };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // escapes
        if c == '\\' {
            if let Some(n) = next {
                if "\\*_~`[]()<>#|-.+!$".contains(n) {
                    buf.push(n);
                    i += 2;
                    continue;
                }
            }
        }
        // equation: $x^2$ or $$x^2$$
        if c == '$' {
            if let Some((latex, used, display)) = math_span(&chars[i..]) {
                flush(&mut buf, &st, out);
                out.push(MdSpan { text: latex, math: Some(display), ..Default::default() });
                i += used;
                continue;
            }
        }
        // code span
        if c == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '`') {
                flush(&mut buf, &st, out);
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                out.push(MdSpan { text: code, code: true, bold: st.bold, italic: st.italic, link: link.map(String::from), ..Default::default() });
                i += end + 2;
                continue;
            }
        }
        // link [text](url)
        if c == '[' && link.is_none() {
            if let Some(close) = find_matching(&chars, i, '[', ']') {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(pclose) = chars[close + 2..].iter().position(|&x| x == ')') {
                        flush(&mut buf, &st, out);
                        let text: String = chars[i + 1..close].iter().collect();
                        let url: String = chars[close + 2..close + 2 + pclose].iter().collect();
                        inline_into(&text, st, Some(url.trim()), out);
                        i = close + 3 + pclose;
                        continue;
                    }
                }
            }
        }
        // <u>...</u>
        if c == '<' {
            let r = rest(i);
            if r.starts_with("<u>") && r.contains("</u>") {
                flush(&mut buf, &st, out);
                st.underline = true;
                i += 3;
                continue;
            }
            if r.starts_with("</u>") && st.underline {
                flush(&mut buf, &st, out);
                st.underline = false;
                i += 4;
                continue;
            }
        }
        // ** / __ bold, ~~ strike
        if (c == '*' || c == '_' || c == '~') && next == Some(c) {
            let delim: String = [c, c].iter().collect();
            let on = match c {
                '~' => st.strike,
                _ => st.bold,
            };
            if on || rest(i + 2).contains(&delim) {
                flush(&mut buf, &st, out);
                match c {
                    '~' => st.strike = !st.strike,
                    _ => st.bold = !st.bold,
                }
                i += 2;
                continue;
            }
        }
        // * / _ italic
        if c == '*' || c == '_' {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let word_inner = c == '_' && prev.is_some_and(|p| p.is_alphanumeric()) && next.is_some_and(|n| n.is_alphanumeric());
            let opening_ok = next.is_some_and(|n| !n.is_whitespace());
            if !word_inner && (st.italic || (opening_ok && rest(i + 1).contains(c))) {
                flush(&mut buf, &st, out);
                st.italic = !st.italic;
                i += 1;
                continue;
            }
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut buf, &st, out);
}

/// An equation at the start of `s`: its LaTeX, the characters it used and whether it is a display equation. A single
/// dollar sign opens an equation only when no space follows it, and closes one only when no space comes before it and
/// no digit after it, so prices such as "$5 and $6" stay text.
fn math_span(s: &[char]) -> Option<(String, usize, bool)> {
    if s.get(1) == Some(&'$') {
        let mut j = 2;
        while j + 1 < s.len() {
            if s[j] == '$' && s[j + 1] == '$' && s[j - 1] != '\\' {
                let latex: String = s[2..j].iter().collect();
                return (!latex.trim().is_empty()).then(|| (latex.trim().to_string(), j + 2, true));
            }
            j += 1;
        }
        return None;
    }
    if s.len() < 3 || s[1].is_whitespace() {
        return None;
    }
    let mut j = 1;
    while j < s.len() {
        if s[j] == '\\' {
            j += 2;
            continue;
        }
        if s[j] == '$' {
            if s[j - 1].is_whitespace() || s.get(j + 1).is_some_and(|d| d.is_ascii_digit()) {
                return None;
            }
            let latex: String = s[1..j].iter().collect();
            return Some((latex, j + 1, false));
        }
        j += 1;
    }
    None
}

fn find_matching(chars: &[char], open_at: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (j, &ch) in chars.iter().enumerate().skip(open_at) {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(j);
            }
        }
    }
    None
}

pub fn parse_blocks(src: &str) -> Vec<MdBlock> {
    let mut out = Vec::new();
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    let mut para: Vec<String> = Vec::new();
    // Indents of the open list items, outermost first: nesting follows indentation like CommonMark,
    // so both two- and four-space styles work.
    let mut list_indents: Vec<usize> = Vec::new();
    let flush_para = |para: &mut Vec<String>, out: &mut Vec<MdBlock>| {
        if !para.is_empty() {
            let text = para.join(" ");
            out.push(MdBlock::Para { style: None, spans: parse_inline(text.trim()) });
            para.clear();
        }
    };
    while i < lines.len() {
        let raw = lines[i];
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if trimmed.is_empty() {
            flush_para(&mut para, &mut out);
            i += 1;
            continue;
        }
        if trimmed == "$$" {
            // A display equation over several lines.
            if let Some(end) = lines[i + 1..].iter().position(|l| l.trim() == "$$") {
                flush_para(&mut para, &mut out);
                let latex = lines[i + 1..i + 1 + end].join("\n").trim().to_string();
                if !latex.is_empty() {
                    out.push(MdBlock::Para { style: None, spans: vec![MdSpan { text: latex, math: Some(true), ..Default::default() }] });
                }
                i += end + 2;
                continue;
            }
        }
        if trimmed.starts_with("```") {
            flush_para(&mut para, &mut out);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push(lines[i].to_string());
                i += 1;
            }
            out.push(MdBlock::Code { lines: code });
            i += 1;
            continue;
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "<pagebreak>" || trimmed == "\\pagebreak" {
            flush_para(&mut para, &mut out);
            out.push(MdBlock::PageBreak);
            i += 1;
            continue;
        }
        if let Some(h) = heading_level(trimmed) {
            flush_para(&mut para, &mut out);
            let text = trimmed[if h == 0 { 2 } else { h }..].trim().trim_end_matches('#').trim();
            let style = if h == 0 { "Title".to_string() } else { format!("Heading{h}") };
            out.push(MdBlock::Para { style: Some(style), spans: parse_inline(text) });
            i += 1;
            continue;
        }
        if trimmed.starts_with('|') {
            flush_para(&mut para, &mut out);
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                let row = lines[i].trim();
                let cells: Vec<&str> = row.trim_matches('|').split('|').map(|c| c.trim()).collect();
                let is_sep = cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')));
                if !is_sep {
                    rows.push(cells.iter().map(|c| parse_inline(c)).collect());
                }
                i += 1;
            }
            out.push(MdBlock::Table { rows });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ").or(if trimmed == ">" { Some("") } else { None }) {
            flush_para(&mut para, &mut out);
            out.push(MdBlock::Para { style: Some("Quote".into()), spans: parse_inline(rest) });
            i += 1;
            continue;
        }
        if let Some((kind, check, rest)) = list_marker(trimmed) {
            flush_para(&mut para, &mut out);
            while list_indents.last().is_some_and(|&l| l > indent) {
                list_indents.pop();
            }
            if list_indents.last() != Some(&indent) {
                list_indents.push(indent);
            }
            out.push(MdBlock::ListItem { kind, level: list_indents.len() - 1, check, spans: parse_inline(rest) });
            i += 1;
            continue;
        }
        list_indents.clear();
        para.push(trimmed.to_string());
        i += 1;
    }
    flush_para(&mut para, &mut out);
    out
}

fn heading_level(s: &str) -> Option<usize> {
    // "#! Title" is our extension for the Title style.
    if s.starts_with("#! ") {
        return Some(0);
    }
    let n = s.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&n) && s[n..].starts_with(' ') {
        Some(n)
    } else {
        None
    }
}

fn list_marker(s: &str) -> Option<(ListKind, Option<bool>, &str)> {
    for m in ["- ", "* ", "+ ", "• "] {
        if let Some(r) = s.strip_prefix(m) {
            let (check, rest) = task_box(r);
            return Some((ListKind::Bullet, check, rest));
        }
    }
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 4 {
        let r = &s[digits..];
        if let Some(r) = r.strip_prefix(". ").or(r.strip_prefix(") ")) {
            return Some((ListKind::Decimal, None, r));
        }
    }
    None
}

/// A GFM task box (`[ ]`, `[x]`, `[X]`) opening a list item's text.
fn task_box(r: &str) -> (Option<bool>, &str) {
    for (b, checked) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
        if let Some(rest) = r.strip_prefix(b) {
            if rest.is_empty() || rest.starts_with(' ') {
                return (Some(checked), rest.trim_start());
            }
        }
    }
    (None, r)
}

/// Escape characters that would otherwise be read as Markdown syntax.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '*' | '_' | '~' | '`' | '[' | ']' | '\\' | '$') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline() {
        let s = parse_inline("Hello **bold *both*** and [a *link*](https://x.y) `code` 5 * 3");
        let texts: Vec<_> = s.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["Hello ", "bold ", "both", " and ", "a ", "link", " ", "code", " 5 * 3"]);
        assert!(s[2].bold && s[2].italic);
        assert_eq!(s[5].link.as_deref(), Some("https://x.y"));
        assert!(s[5].italic);
        assert!(s[7].code);
    }

    #[test]
    fn blocks() {
        let b = parse_blocks("# Title\n\nSome *text*\ncontinued.\n\n- a\n  - b\n1. one\n\n| A | B |\n|---|---|\n| 1 | 2 |\n");
        assert!(matches!(&b[0], MdBlock::Para { style: Some(s), .. } if s == "Heading1"));
        assert!(matches!(&b[1], MdBlock::Para { style: None, spans } if spans.len() == 3));
        assert!(matches!(&b[3], MdBlock::ListItem { level: 1, .. }));
        assert!(matches!(&b[4], MdBlock::ListItem { kind: ListKind::Decimal, .. }));
        assert!(matches!(&b[5], MdBlock::Table { rows } if rows.len() == 2));
    }

    #[test]
    fn task_items() {
        let b = parse_blocks("- [ ] todo\n* [x] done\n  - [X] nested\n- [x]\n- [link](https://x.y)\n- [ ]no space");
        let checks: Vec<_> = b.iter().map(|b| if let MdBlock::ListItem { check, .. } = b { *check } else { panic!("{b:?}") }).collect();
        assert_eq!(checks, [Some(false), Some(true), Some(true), Some(true), None, None]);
        assert!(matches!(&b[0], MdBlock::ListItem { spans, .. } if spans[0].text == "todo"));
        assert!(matches!(&b[2], MdBlock::ListItem { level: 1, .. }));
        assert!(matches!(&b[5], MdBlock::ListItem { spans, .. } if spans[0].text == "[ ]no space"));
    }
}
