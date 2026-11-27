//! The values ops carry that are not ops themselves: defaults, cross-reference targets, break and header kinds.

use super::*;

pub(super) fn default_toc_levels() -> u8 {
    3
}

pub(super) fn default_envelope() -> String {
    "10".into()
}

pub(super) fn default_tc_level() -> u8 {
    1
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_connector_kind() -> String {
    "straight".into()
}

pub(super) fn default_index_columns() -> u8 {
    2
}

pub(super) fn default_figure_label() -> String {
    "Figure".into()
}

/// One line of an AutoMark file: the text to mark and the index entry for it (`Fruit:Apple` is the sub-entry
/// Apple under Fruit; an empty entry is the text itself).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoMarkEntry {
    pub text: String,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

/// Chapter numbers in captions: the heading level they follow and what goes between the chapter and the number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChapterNumbering {
    /// The heading level (1 to 9) whose number is the chapter number.
    pub level: u8,
    /// Between the chapter number and the caption number (default `-`; Word offers hyphen, period, colon, em dash and en dash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
}

/// Where a caption goes relative to the thing it describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptionPosition {
    Above,
    Below,
}

pub(super) fn string_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(serde::de::Error::custom(format!("expected a note id, got {other}"))),
    }
}

/// What a cross-reference points at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XrefTarget {
    /// A heading paragraph.
    Heading { p: Id },
    /// A numbered paragraph (a list item): its text, its number with or without the levels above, or its page.
    Numbered { p: Id },
    /// A bookmark by name.
    Bookmark { name: String },
    /// A caption paragraph (one with a `SEQ` field).
    Caption { p: Id },
    /// A footnote by its id (`RNote.id`).
    Footnote {
        #[serde(deserialize_with = "string_or_number")]
        id: String,
    },
    /// An endnote by its id.
    Endnote {
        #[serde(deserialize_with = "string_or_number")]
        id: String,
    },
}

/// What a cross-reference shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XrefShow {
    /// The target's text: the heading, the bookmarked text, the whole caption.
    Text,
    /// The paragraph number of a heading or list item (`\r`), the number of a caption (just the number) or of a note.
    Number,
    /// The paragraph number without context (`\n`).
    NumberNoContext,
    /// The paragraph number with all its levels (`\w`).
    NumberFullContext,
    /// The page the target is on (`PAGEREF`).
    Page,
    /// A caption's label and number: "Figure 1".
    LabelNumber,
    /// A caption's text after its number.
    CaptionText,
    /// A note's number formatted as its mark (superscript, `NOTEREF \f`).
    NumberFormatted,
}

/// What `InsertBreak` inserts. The section breaks name how the new section starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakKind {
    Page,
    Column,
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
}

impl BreakKind {
    /// The `w:type` of a section break (`nextPage`, `continuous`, ...); None for page and column breaks.
    pub fn section_type(self) -> Option<&'static str> {
        match self {
            BreakKind::Page | BreakKind::Column => None,
            BreakKind::NextPage => Some("nextPage"),
            BreakKind::Continuous => Some("continuous"),
            BreakKind::EvenPage => Some("evenPage"),
            BreakKind::OddPage => Some("oddPage"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderKind {
    Header,
    Footer,
}

pub(super) fn default_variant() -> String {
    "default".into()
}
