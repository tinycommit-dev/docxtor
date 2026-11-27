//! Word count: paragraphs, words and characters of the main text, of the footnotes and endnotes, and of the text
//! boxes. Lines depend on the page layout, so the editor counts those on screen.
//!
//! Words are runs of non-space characters, the way Word counts them. Characters leave out paragraph marks and
//! line breaks, and objects (pictures, tabs, note marks) are not characters. Headers, footers and text boxes in
//! them are not counted, as in Word.

use crate::document::Document;
use crate::model::*;
use crate::stories::StoryKind;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub paragraphs: usize,
    pub words: usize,
    pub chars: usize,
    pub chars_no_spaces: usize,
}

impl Counts {
    fn add_text(&mut self, text: &str) {
        self.paragraphs += 1;
        self.words += text.split_whitespace().count();
        let mut chars = 0;
        let mut solid = 0;
        for c in text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\u{b}')) {
            chars += 1;
            if !c.is_whitespace() {
                solid += 1;
            }
        }
        self.chars += chars;
        self.chars_no_spaces += solid;
    }

    /// These counts without `other`'s (which must be part of them).
    pub fn minus(&self, other: &Counts) -> Counts {
        Counts {
            paragraphs: self.paragraphs - other.paragraphs,
            words: self.words - other.words,
            chars: self.chars - other.chars,
            chars_no_spaces: self.chars_no_spaces - other.chars_no_spaces,
        }
    }

    /// These counts and `other`'s together.
    pub fn plus(&self, other: &Counts) -> Counts {
        Counts {
            paragraphs: self.paragraphs + other.paragraphs,
            words: self.words + other.words,
            chars: self.chars + other.chars,
            chars_no_spaces: self.chars_no_spaces + other.chars_no_spaces,
        }
    }
}

/// The counts of the three kinds of text a document has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStats {
    /// The main text, tables included.
    pub main: Counts,
    /// Footnotes and endnotes.
    pub notes: Counts,
    /// Text boxes and shapes with text in the main text.
    pub boxes: Counts,
}

pub(crate) fn count_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Counts {
    let mut c = Counts::default();
    walk_paragraphs(blocks, &mut |p| c.add_text(&p.plain_text()));
    c
}

impl Document {
    /// Paragraphs, words and characters of the main text, the notes and the text boxes.
    pub fn text_stats(&self) -> TextStats {
        let mut out = TextStats { main: count_blocks(&self.body), ..Default::default() };
        for s in &self.stories {
            let c = count_blocks(&s.blocks);
            match s.kind {
                StoryKind::Footnote | StoryKind::Endnote => out.notes = out.notes.plus(&c),
                StoryKind::TextBox if s.part == self.main_path => out.boxes = out.boxes.plus(&c),
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_and_characters() {
        let mut c = Counts::default();
        c.add_text("Hello big  world");
        c.add_text("");
        assert_eq!(c, Counts { paragraphs: 2, words: 3, chars: 16, chars_no_spaces: 13 });
    }
}
