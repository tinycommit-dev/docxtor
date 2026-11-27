//! RTF reader: a whole RTF document, or a clipboard snippet, into a `Fragment`.
//!
//! The reader walks the token stream once. Groups save and restore the formatting state, the header
//! tables (fonts, colours, styles, lists) are read as they go by, and body text becomes `w:p` and `w:tbl`
//! elements built with `fragment::Builder`. It never panics and never loops: groups nest at most
//! `MAX_DEPTH` deep, unbalanced braces and truncated input are closed at the end of the text.
//!
//! Limits:
//! - Text bytes (`\'hh`) are decoded with Windows code pages 1250, 1251, 1252, 1253 and 1254. Other
//!   code pages fall back to Latin-1. Double-byte code pages (`\fcharset134` and the like) are kept as
//!   Latin-1 characters, so East Asian text that is not also given as `\uN` shows as mojibake.
//! - Nested tables are flattened: the cells of an inner table become paragraphs of the outer cell.
//! - Pictures: PNG, JPEG, GIF, BMP and WebP are kept. EMF, WMF, PICT and DIB cannot be drawn by a
//!   browser, so they are dropped with a `picture-unsupported` warning.
//! - Header and footer groups, the page setup, sections, text boxes and drawing objects are skipped;
//!   `\sect` only ends the current paragraph.
//! - Character styles are kept when they are used with `\csN`; table styles are ignored.

use crate::fragment::Fragment;

const MAX_DEPTH: usize = crate::budget::caps::RTF_DEPTH;
const MAX_PICT_BYTES: usize = crate::budget::caps::RTF_PICT_BYTES;
const MAX_CELLS: usize = crate::budget::caps::RTF_CELLS;

/// Whether text looks like RTF: it starts with `{\rtf`, after optional whitespace or a byte order mark.
pub fn looks_like_rtf(text: &str) -> bool {
    text.trim_start_matches(|c: char| c == '\u{feff}' || c.is_whitespace()).starts_with("{\\rtf")
}

/// Read an RTF file's bytes: UTF-8 when they are valid UTF-8, otherwise Latin-1 (each byte is the
/// character with that code).
pub fn rtf_bytes_to_fragment(bytes: &[u8]) -> Fragment {
    match std::str::from_utf8(bytes) {
        Ok(s) => rtf_to_fragment(s),
        Err(_) => {
            let s: String = bytes.iter().map(|&b| b as char).collect();
            rtf_to_fragment(&s)
        }
    }
}

/// Read an RTF document into a fragment.
pub fn rtf_to_fragment(rtf: &str) -> Fragment {
    // Too long to be something pasted: nothing is read (the budget's paste size, see `crate::budget`).
    if crate::budget::within(rtf.len(), crate::budget::caps::PASTE_BYTES).is_err() {
        return Fragment::default();
    }
    let mut it = Interp::new();
    it.run(rtf);
    it.finish()
}

mod codepages;
mod groups;
mod lexer;
mod notes_pictures;
mod state;
mod styles;
mod tables;
mod text;
mod words;

use state::Interp;
