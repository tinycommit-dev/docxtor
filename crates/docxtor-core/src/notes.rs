//! Footnote and endnote numbering: the number format (`w:footnotePr` / `w:endnotePr` in `settings.xml` and
//! in the last section) and the labels it produces. Word numbers footnotes 1, 2, 3 and endnotes i, ii, iii
//! unless the document says otherwise.

use crate::xml::Element;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Footnote,
    Endnote,
}

impl NoteKind {
    /// `w:footnotePr` or `w:endnotePr`.
    pub fn pr_tag(self) -> &'static str {
        match self {
            NoteKind::Footnote => "w:footnotePr",
            NoteKind::Endnote => "w:endnotePr",
        }
    }
    /// `w:footnote` or `w:endnote`: the notes in their part.
    pub fn note_tag(self) -> &'static str {
        match self {
            NoteKind::Footnote => "w:footnote",
            NoteKind::Endnote => "w:endnote",
        }
    }
    /// `w:footnoteReference` or `w:endnoteReference`: the mark in the text.
    pub fn reference_tag(self) -> &'static str {
        match self {
            NoteKind::Footnote => "w:footnoteReference",
            NoteKind::Endnote => "w:endnoteReference",
        }
    }
    /// `w:footnoteRef` or `w:endnoteRef`: the mark at the start of the note itself.
    pub fn mark_tag(self) -> &'static str {
        match self {
            NoteKind::Footnote => "w:footnoteRef",
            NoteKind::Endnote => "w:endnoteRef",
        }
    }
    pub fn text_style(self) -> &'static str {
        match self {
            NoteKind::Footnote => "FootnoteText",
            NoteKind::Endnote => "EndnoteText",
        }
    }
    pub fn ref_style(self) -> &'static str {
        match self {
            NoteKind::Footnote => "FootnoteReference",
            NoteKind::Endnote => "EndnoteReference",
        }
    }
}

/// How a kind of note is numbered: an OOXML number format (`decimal`, `lowerRoman`, `chicago`, ...) and the
/// number of the first note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFormat {
    pub fmt: String,
    pub start: i64,
}

impl NoteFormat {
    /// What Word does when nothing says otherwise.
    pub fn default_for(kind: NoteKind) -> Self {
        NoteFormat { fmt: if kind == NoteKind::Endnote { "lowerRoman" } else { "decimal" }.into(), start: 1 }
    }

    /// Read the format from the settings (`w:settings`) and the last section (`w:sectPr`), the section winning.
    pub fn read(kind: NoteKind, settings: Option<&Element>, last_sect: Option<&Element>) -> Self {
        let mut out = Self::default_for(kind);
        for pr in [settings, last_sect].into_iter().flatten().filter_map(|e| e.child(kind.pr_tag())) {
            if let Some(f) = pr.child("w:numFmt").and_then(|n| n.attr("w:val")) {
                out.fmt = f.to_string();
            }
            if let Some(s) = pr.child("w:numStart").and_then(|n| n.attr("w:val")).and_then(|v| v.parse::<i64>().ok()) {
                out.start = s;
            }
        }
        out
    }

    /// The label of the `index`th note (1-based) of this kind.
    pub fn label(&self, index: u32) -> String {
        format_number(self.start + index as i64 - 1, &self.fmt)
    }

    /// A small number that changes whenever the format does (to notice that labels moved).
    pub fn code(&self) -> u32 {
        let mut h: u32 = 2166136261;
        for b in self.fmt.bytes().chain(self.start.to_le_bytes()) {
            h = (h ^ b as u32).wrapping_mul(16777619);
        }
        h
    }
}

/// The OOXML `w:numFmt` value for a format name people use (`symbol` is Word's `chicago`).
pub fn ooxml_format(name: &str) -> Option<&'static str> {
    Some(match name {
        "decimal" => "decimal",
        "lowerRoman" => "lowerRoman",
        "upperRoman" => "upperRoman",
        "lowerLetter" => "lowerLetter",
        "upperLetter" => "upperLetter",
        "symbol" | "chicago" => "chicago",
        _ => return None,
    })
}

/// Format a number the way Word's number formats do. Anything it doesn't know is decimal.
pub fn format_number(n: i64, fmt: &str) -> String {
    match fmt {
        "lowerRoman" => roman(n).map(|r| r.to_lowercase()).unwrap_or_else(|| n.to_string()),
        "upperRoman" => roman(n).unwrap_or_else(|| n.to_string()),
        "lowerLetter" => letters(n, false),
        "upperLetter" => letters(n, true),
        "chicago" => symbols(n),
        "decimalZero" if (0..10).contains(&n) => format!("0{n}"),
        _ => n.to_string(),
    }
}

fn roman(n: i64) -> Option<String> {
    if !(1..4000).contains(&n) {
        return None;
    }
    let table = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut n = n;
    let mut out = String::new();
    for (v, s) in table {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    Some(out)
}

/// a, b, ... z, aa, bb, ... zz, aaa: Word repeats the letter rather than counting in base 26.
fn letters(n: i64, upper: bool) -> String {
    if n < 1 {
        return n.to_string();
    }
    let k = ((n - 1) % 26) as u8;
    let times = ((n - 1) / 26 + 1) as usize;
    let c = (if upper { b'A' } else { b'a' } + k) as char;
    std::iter::repeat_n(c, times).collect()
}

/// *, †, ‡, §, then the same doubled (**, ††, ‡‡, §§), tripled, and so on.
fn symbols(n: i64) -> String {
    if n < 1 {
        return n.to_string();
    }
    const SYMS: [char; 4] = ['*', '\u{2020}', '\u{2021}', '\u{00A7}'];
    let c = SYMS[((n - 1) % 4) as usize];
    let times = ((n - 1) / 4 + 1) as usize;
    std::iter::repeat_n(c, times).collect()
}

/// Write the format into a `w:footnotePr` / `w:endnotePr` element: `w:numFmt` and `w:numStart` go after
/// `w:pos` and before `w:numRestart` and the separator references, as the schema orders them.
pub fn write_format(pr: &mut Element, fmt: Option<&str>, start: Option<i64>) {
    const ORDER: &[&str] = &["w:pos", "w:numFmt", "w:numStart", "w:numRestart", "w:footnote", "w:endnote"];
    if let Some(f) = fmt {
        crate::props::put_ordered(pr, Element::new("w:numFmt").with_attr("w:val", f), ORDER);
    }
    if let Some(s) = start {
        crate::props::put_ordered(pr, Element::new("w:numStart").with_attr("w:val", s.to_string()), ORDER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_number(4, "lowerRoman"), "iv");
        assert_eq!(format_number(14, "upperRoman"), "XIV");
        assert_eq!(format_number(27, "lowerLetter"), "aa");
        assert_eq!(format_number(5, "chicago"), "**");
        assert_eq!(format_number(3, "decimal"), "3");
        assert_eq!(NoteFormat { fmt: "decimal".into(), start: 5 }.label(2), "6");
    }
}
