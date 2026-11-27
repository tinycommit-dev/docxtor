//! What a `TOC` instruction asks for, and the options the "Table of contents" dialog edits. Switches are read
//! and written the way Word does: `\o "1-3"` heading levels, `\u` outline levels, `\t "Style,1"` styles,
//! `\f` and `\l` table entry (`TC`) fields, `\h` links, `\z` hidden in web layout, `\n` no page numbers,
//! `\p` the text between an entry and its page number. Switches Docxtor doesn't model are kept as they were.

use crate::fields::{Instr, Tok};
use crate::toc::Leader;
use serde::{Deserialize, Serialize};

/// Which page numbers a table leaves out (`\n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoPages {
    #[default]
    None,
    All,
    Levels(u8, u8),
}

/// The table entry fields a table lists (`\f`, `\l`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcSpec {
    /// The entry type letter (`C` unless the field says another).
    pub id: String,
    pub levels: (u8, u8),
}

/// A parsed `TOC` instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// Built-in heading styles, from `\o`.
    pub headings: Option<(u8, u8)>,
    /// Paragraphs with an outline level (`\u`).
    pub outline: bool,
    /// Other styles and the level each one makes (`\t`).
    pub styles: Vec<(String, u8)>,
    pub links: bool,
    pub web_hide: bool,
    pub no_pages: NoPages,
    pub sep: Option<String>,
    pub tc: Option<TcSpec>,
}

fn range(s: &str) -> Option<(u8, u8)> {
    let mut parts = s.trim().trim_matches('"').split('-').filter_map(|p| p.trim().parse::<u8>().ok());
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) if a >= 1 && b >= a => Some((a, b.min(9))),
        (Some(a), None) if a >= 1 => Some((a, a.min(9))),
        _ => None,
    }
}

impl Spec {
    pub fn parse(instr: &str) -> Spec {
        let i = Instr::parse(instr);
        let mut headings = i.value("o").and_then(|v| range(&v));
        let outline = i.has("u");
        let styles: Vec<(String, u8)> = i
            .value("t")
            .map(|v| {
                let parts: Vec<&str> = v.split([',', ';']).collect();
                parts.chunks(2).filter_map(|c| Some((c.first()?.trim().to_string(), c.get(1)?.trim().parse::<u8>().ok()?.clamp(1, 9)))).filter(|(s, _)| !s.is_empty()).collect()
            })
            .unwrap_or_default();
        let tc = if i.has("f") || i.has("l") {
            Some(TcSpec { id: i.value("f").map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| "C".into()), levels: i.value("l").and_then(|v| range(&v)).unwrap_or((1, 9)) })
        } else {
            None
        };
        // A bare field lists the headings, as Word does when the dialog's defaults are used.
        let mut outline = outline;
        if headings.is_none() && !outline && styles.is_empty() && tc.is_none() {
            headings = Some((1, 3));
            outline = true;
        }
        let no_pages = if i.has("n") {
            match i.value("n").and_then(|v| range(&v)) {
                Some((a, b)) => NoPages::Levels(a, b),
                None => NoPages::All,
            }
        } else {
            NoPages::None
        };
        Spec { headings, outline, styles, links: i.has("h"), web_hide: i.has("z"), no_pages, sep: i.value("p"), tc }
    }

    /// The deepest level the table lists.
    pub fn max_level(&self) -> u8 {
        let h = self.headings.map(|h| h.1).unwrap_or(0);
        let s = self.styles.iter().map(|s| s.1).max().unwrap_or(0);
        let t = self.tc.as_ref().map(|t| t.levels.1).unwrap_or(0);
        let o = if self.outline { 9 } else { 0 };
        h.max(s).max(t).max(o).max(1)
    }

    /// Whether an entry at this level has a page number.
    pub fn shows_page(&self, level: u8) -> bool {
        match self.no_pages {
            NoPages::None => true,
            NoPages::All => false,
            NoPages::Levels(a, b) => !(a..=b).contains(&level),
        }
    }
}

/// One style that makes entries at a level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleLevel {
    pub style: String,
    pub level: u8,
}

fn yes() -> bool {
    true
}

fn three() -> u8 {
    3
}

/// What the "Table of contents" dialog sets, as data. `instr` turns it into the field instruction and `Document`
/// writes the leader and the look into the tab stops and the `TOC` styles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TocOptions {
    /// Deepest built-in heading level to list; 0 lists none.
    #[serde(default = "three")]
    pub levels: u8,
    /// Also list paragraphs that have an outline level.
    #[serde(default = "yes")]
    pub outline: bool,
    /// Other styles and the level each one makes.
    #[serde(default)]
    pub styles: Vec<StyleLevel>,
    /// List table entry fields (`TC`), marked with Insert, Add text.
    #[serde(default)]
    pub entries: bool,
    /// The entry type of those fields (default `C`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<String>,
    #[serde(default = "yes")]
    pub links: bool,
    #[serde(default = "yes")]
    pub page_numbers: bool,
    #[serde(default = "yes")]
    pub right_align: bool,
    #[serde(default)]
    pub leader: Leader,
    /// Levels without a page number (`\n`), as `[first, last]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_pages: Option<[u8; 2]>,
    /// One of the looks (`classic`, `formal`, `modern`, `simple`); None leaves the styles as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
}

impl Default for TocOptions {
    fn default() -> Self {
        TocOptions { levels: 3, outline: true, styles: vec![], entries: false, entry_id: None, links: true, page_numbers: true, right_align: true, leader: Leader::Dot, hide_pages: None, look: None }
    }
}

fn quoted(t: &Tok) -> String {
    if t.quoted {
        format!("\"{}\"", t.text)
    } else {
        t.text.clone()
    }
}

impl TocOptions {
    /// The options an instruction stands for. The leader and the look aren't in it (Word keeps them in the
    /// styles); `leader` is the one the existing result uses.
    pub fn from_instr(instr: &str, leader: Leader) -> TocOptions {
        let s = Spec::parse(instr);
        let i = Instr::parse(instr);
        TocOptions {
            levels: s.headings.map(|h| h.1).unwrap_or(0),
            outline: i.has("u"),
            styles: s.styles.iter().map(|(style, level)| StyleLevel { style: style.clone(), level: *level }).collect(),
            entries: s.tc.is_some(),
            entry_id: s.tc.as_ref().map(|t| t.id.clone()).filter(|id| !id.eq_ignore_ascii_case("c")),
            links: s.links,
            page_numbers: s.no_pages != NoPages::All,
            right_align: !matches!(&s.sep, Some(p) if !p.contains('\t')),
            leader,
            hide_pages: if let NoPages::Levels(a, b) = s.no_pages { Some([a, b]) } else { None },
            look: None,
        }
    }

    /// The `TOC` instruction for these options. Switches the dialog doesn't set (`\b`, `\s`, `\w`, ...) are
    /// carried over from `old`, the instruction being replaced.
    pub fn instr(&self, old: Option<&str>) -> String {
        let mut out = String::from(" TOC");
        if self.levels > 0 {
            out.push_str(&format!(" \\o \"1-{}\"", self.levels.clamp(1, 9)));
        }
        if !self.styles.is_empty() {
            let list: Vec<String> = self.styles.iter().filter(|s| !s.style.trim().is_empty()).map(|s| format!("{},{}", s.style.trim(), s.level.clamp(1, 9))).collect();
            out.push_str(&format!(" \\t \"{}\"", crate::fields::escape(&list.join(","), false)));
        }
        if self.entries {
            match self.entry_id.as_deref().map(str::trim).filter(|id| !id.is_empty() && !id.eq_ignore_ascii_case("c")) {
                Some(id) => out.push_str(&format!(" \\f \"{}\"", crate::fields::escape(id, false))),
                None => out.push_str(" \\f"),
            }
        }
        if self.links {
            out.push_str(" \\h");
        }
        out.push_str(" \\z");
        if self.outline || (self.levels == 0 && self.styles.is_empty() && !self.entries) {
            out.push_str(" \\u");
        }
        if !self.page_numbers {
            out.push_str(" \\n");
        } else if let Some([a, b]) = self.hide_pages {
            out.push_str(&format!(" \\n \"{}-{}\"", a.clamp(1, 9), b.clamp(a, 9)));
        }
        if !self.right_align && self.page_numbers {
            out.push_str(" \\p \" \"");
        }
        // Carry over what the dialog doesn't know.
        if let Some(old) = old {
            let parsed = Instr::parse(old);
            let mut k = 0;
            while k < parsed.toks.len() {
                let t = &parsed.toks[k];
                if t.is_switch() {
                    let name = t.text[1..].to_lowercase();
                    let takes_value = parsed.toks.get(k + 1).is_some_and(|n| !n.is_switch());
                    if !matches!(name.as_str(), "o" | "u" | "t" | "f" | "l" | "h" | "z" | "n" | "p") {
                        out.push(' ');
                        out.push_str(&t.text);
                        if takes_value {
                            out.push(' ');
                            out.push_str(&quoted(&parsed.toks[k + 1]));
                        }
                    }
                    k += if takes_value { 2 } else { 1 };
                } else {
                    k += 1;
                }
            }
        }
        out.push(' ');
        out
    }
}

/// A `TC` field: one line for the table of contents, whatever the text it sits in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcMark {
    pub text: String,
    /// The entry type letter, `C` by default.
    pub id: String,
    pub level: u8,
    /// `\n`: no page number for this entry.
    pub no_page: bool,
}

/// Read a `TC` instruction: ` TC "Chapter one" \f C \l 2 \n `.
pub fn parse_tc(instr: &str) -> Option<TcMark> {
    let i = Instr::parse(instr);
    if i.code != "TC" {
        return None;
    }
    let text = crate::fields::unescape(i.first_arg()?).trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(TcMark {
        text,
        id: i.value("f").map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| "C".into()),
        level: i.value("l").and_then(|v| v.trim().parse::<u8>().ok()).unwrap_or(1).clamp(1, 9),
        no_page: i.has("n"),
    })
}

/// The instruction of a new `TC` field.
pub fn tc_instr(text: &str, level: u8, id: Option<&str>, no_page: bool) -> String {
    let mut out = format!(" TC \"{}\"", crate::fields::escape(text.trim(), false));
    if let Some(id) = id.map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("c")) {
        out.push_str(&format!(" \\f \"{}\"", crate::fields::escape(id, false)));
    }
    if level != 1 {
        out.push_str(&format!(" \\l {}", level.clamp(1, 9)));
    }
    if no_page {
        out.push_str(" \\n");
    }
    out.push(' ');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_word_writes() {
        let s = Spec::parse(r#" TOC \o "1-3" \h \z \u "#);
        assert_eq!(s.headings, Some((1, 3)));
        assert!(s.outline && s.links && s.web_hide);
        assert_eq!(s.no_pages, NoPages::None);
        let s = Spec::parse(r#" TOC \o "2-4" \h \z \t "Quote,1,Note;2" \n "3-4" \p " " "#);
        assert_eq!(s.headings, Some((2, 4)));
        assert_eq!(s.styles, [("Quote".to_string(), 1), ("Note".to_string(), 2)]);
        assert_eq!(s.no_pages, NoPages::Levels(3, 4));
        assert_eq!(s.sep.as_deref(), Some(" "));
        assert!(!s.outline);
        let s = Spec::parse(r#" TOC \f A \l "1-2" "#);
        assert_eq!(s.tc, Some(TcSpec { id: "A".into(), levels: (1, 2) }));
        assert_eq!(s.headings, None);
        assert_eq!(Spec::parse(" TOC ").headings, Some((1, 3)));
    }

    #[test]
    fn options_write_the_switches_in_words_shape() {
        let o = TocOptions::default();
        assert_eq!(o.instr(None), r#" TOC \o "1-3" \h \z \u "#);
        let o = TocOptions { levels: 2, outline: false, styles: vec![StyleLevel { style: "Quote".into(), level: 3 }], entries: true, page_numbers: false, ..Default::default() };
        let i = o.instr(Some(r#" TOC \o "1-3" \h \z \u \b Chapter \w "#));
        assert!(i.contains(r#"\o "1-2""#) && i.contains(r#"\t "Quote,3""#) && i.contains(" \\f") && i.contains(" \\n"));
        assert!(i.contains("\\b Chapter") && i.contains("\\w"), "{i}");
        assert!(!i.contains("\\u"));
        let back = TocOptions::from_instr(&i, Leader::Dot);
        assert_eq!(back.levels, 2);
        assert!(back.entries && !back.page_numbers && !back.outline);
        assert_eq!(back.styles, o.styles);
    }

    #[test]
    fn tc_fields() {
        let i = tc_instr("A \"quoted\" entry", 2, Some("D"), true);
        let m = parse_tc(&i).unwrap();
        assert_eq!(m, TcMark { text: "A \"quoted\" entry".into(), id: "D".into(), level: 2, no_page: true });
        assert_eq!(parse_tc(&tc_instr("Plain", 1, None, false)).unwrap().id, "C");
    }
}
