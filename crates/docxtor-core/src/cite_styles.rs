//! Citation styles: in-text citations and reference-list entries for APA 7, MLA 9, Chicago 17
//! (author-date), IEEE and Harvard (Cite Them Right), as formatted runs (plain and italic).
//!
//! Docxtor formats with the current editions; Word, asked to update, formats with its own (older) style
//! files, so a document may change slightly when it is updated there.

use crate::citations::{month_number, nz, Name, Source, SourceType, MONTH_NAMES};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Apa,
    Mla,
    Chicago,
    Ieee,
    Harvard,
}

impl Style {
    pub const ALL: [Style; 5] = [Style::Apa, Style::Mla, Style::Chicago, Style::Ieee, Style::Harvard];

    pub fn id(self) -> &'static str {
        match self {
            Style::Apa => "apa",
            Style::Mla => "mla",
            Style::Chicago => "chicago",
            Style::Ieee => "ieee",
            Style::Harvard => "harvard",
        }
    }

    pub fn parse(s: &str) -> Option<Style> {
        let t = s.trim().to_lowercase();
        let t = t.trim_start_matches('\\');
        Some(match t {
            t if t.starts_with("apa") => Style::Apa,
            t if t.starts_with("mla") => Style::Mla,
            t if t.starts_with("chicago") || t.starts_with("turabian") => Style::Chicago,
            t if t.starts_with("ieee") => Style::Ieee,
            t if t.starts_with("harvard") => Style::Harvard,
            _ => return None,
        })
    }

    /// Word's `SelectedStyle` and `StyleName` for the style, so Word re-renders with its own style file.
    pub fn word(self) -> (&'static str, &'static str) {
        match self {
            Style::Apa => ("\\APASixthEditionOfficeOnline.xsl", "APA"),
            Style::Mla => ("\\MLASeventhEditionOfficeOnline.xsl", "MLA"),
            Style::Chicago => ("\\ChicagoSixteenthEditionOfficeOnline.xsl", "Chicago"),
            Style::Ieee => ("\\IEEE2006OfficeOnline.xsl", "IEEE"),
            Style::Harvard => ("\\HarvardAnglia2008OfficeOnline.xsl", "Harvard - Anglia"),
        }
    }

    /// The style a document's `SelectedStyle`/`StyleName` names; other Word styles read as the closest of ours.
    pub fn from_word(selected: &str, name: &str) -> Style {
        let both = format!("{selected} {name}");
        let l = both.to_lowercase();
        if l.contains("turabian") || l.contains("chicago") {
            Style::Chicago
        } else if l.contains("mla") {
            Style::Mla
        } else if l.contains("ieee") || l.contains("numerical") || l.contains("gb7714") {
            Style::Ieee
        } else if l.contains("harvard") || l.contains("iso690") || l.contains("iso 690") {
            Style::Harvard
        } else {
            Style::Apa
        }
    }

    /// The paragraph indent of a bibliography entry in twips: (left, hanging).
    pub fn indent(self) -> (i64, i64) {
        match self {
            Style::Ieee => (540, 540),
            _ => (720, 720),
        }
    }

    /// The heading above a bibliography.
    pub fn heading(self) -> &'static str {
        match self {
            Style::Apa => "References",
            Style::Mla => "Works Cited",
            _ => "Bibliography",
        }
    }
}

// ---------------------------------------------------------------------------
// Formatted text
// ---------------------------------------------------------------------------

/// A run of text with its look. The built-in styles use plain and italic; CSL styles can also set bold,
/// small capitals and superscript.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub italic: bool,
    pub bold: bool,
    pub small_caps: bool,
    pub sup: bool,
}

impl Span {
    fn same_look(&self, o: &Span) -> bool {
        self.italic == o.italic && self.bold == o.bold && self.small_caps == o.small_caps && self.sup == o.sup
    }
}

/// Text made of runs that differ in look.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rich(pub Vec<Span>);

impl Rich {
    pub fn new() -> Rich {
        Rich(Vec::new())
    }

    pub fn plain_of(s: &str) -> Rich {
        let mut r = Rich::new();
        r.t(s);
        r
    }

    fn push(&mut self, s: &str, italic: bool) -> &mut Self {
        self.push_span(Span { text: s.to_string(), italic, ..Default::default() })
    }

    /// Add a run with any look; it joins the run before it when they look the same.
    pub fn push_span(&mut self, span: Span) -> &mut Self {
        if span.text.is_empty() {
            return self;
        }
        match self.0.last_mut() {
            Some(l) if l.same_look(&span) => l.text.push_str(&span.text),
            _ => self.0.push(span),
        }
        self
    }

    /// Add plain text.
    pub fn t(&mut self, s: &str) -> &mut Self {
        self.push(s, false)
    }

    /// Add italic text.
    pub fn i(&mut self, s: &str) -> &mut Self {
        self.push(s, true)
    }

    pub fn append(&mut self, other: &Rich) -> &mut Self {
        for s in &other.0 {
            self.push_span(s.clone());
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|s| s.text.is_empty())
    }

    pub fn plain(&self) -> String {
        self.0.iter().map(|s| s.text.as_str()).collect()
    }

    /// The text with italics marked `*like this*` (for tests and debugging). Bold is `**like this**`, small
    /// capitals `~like this~` and superscript `^like this^`.
    pub fn marked(&self) -> String {
        self.0
            .iter()
            .map(|s| {
                let mut t = s.text.clone();
                if s.small_caps {
                    t = format!("~{t}~");
                }
                if s.sup {
                    t = format!("^{t}^");
                }
                if s.bold {
                    t = format!("**{t}**");
                }
                if s.italic {
                    t = format!("*{t}*");
                }
                t
            })
            .collect()
    }

    pub fn last_char(&self) -> Option<char> {
        self.0.iter().rev().find_map(|s| s.text.chars().last())
    }

    /// A space, unless the text is empty or already ends in one.
    pub fn sp(&mut self) -> &mut Self {
        match self.last_char() {
            None | Some(' ') | Some('\t') => self,
            _ => self.t(" "),
        }
    }

    /// End a sentence: add a full stop unless the text already ends in `.`, `?` or `!` (a closing quote
    /// after one counts).
    pub fn stop(&mut self) -> &mut Self {
        let text = self.plain();
        let t = text.trim_end_matches(['"', '\'', '\u{201D}', '\u{2019}']);
        if t.is_empty() || t.ends_with(['.', '?', '!']) {
            return self;
        }
        self.t(".")
    }

    /// A quoted title that ends the text takes a full stop inside its quotes, not a comma: `"Title,"` and
    /// `"Title,".` become `"Title."`.
    fn end_quoted_title(&mut self) {
        if let Some(l) = self.0.last_mut() {
            if l.italic {
                return;
            }
            let t = l.text.trim_end().to_string();
            for bad in [",\" .", ",\".", ",\""] {
                if let Some(head) = t.strip_suffix(bad) {
                    l.text = format!("{head}.\"");
                    return;
                }
            }
        }
    }

    pub fn trim_end(&mut self) {
        while let Some(l) = self.0.last_mut() {
            let t = l.text.trim_end().len();
            l.text.truncate(t);
            if l.text.is_empty() {
                self.0.pop();
            } else {
                break;
            }
        }
    }

}

mod apa;
mod chicago;
mod harvard;
mod ieee;
mod mla;
mod names;
mod plan;
mod source;
mod text;
#[cfg(test)]
mod tests;

pub use plan::{Item, Plan};
pub use text::{fold_char, initials};
use names::*;
use source::*;
use text::*;
