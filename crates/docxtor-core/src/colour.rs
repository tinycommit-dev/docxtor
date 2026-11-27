//! The one place a colour from a file becomes a colour the engine hands out.
//!
//! A colour in a .docx (`w:color w:val`, `w:shd w:fill`, `w:highlight`, DrawingML `a:srgbClr val`, a theme's colours, a chart's, a control's,
//! the page's, a border's, a shape's, WordArt's) is a string the file chooses: `FF0000;background:url(http://...)` is as good a value for it
//! as `FF0000`. Whatever the engine hands the page (the render model, the shape, chart, picture and page set-up views, the styles, the
//! lists, the exports) passes a function here first, where it is read from the file, so a value that is not a colour never leaves the engine:
//!
//! - [`hex`]: six hex digits, as `RRGGBB` in capitals (an `auto`, a name, or anything else is none);
//! - [`auto_or_hex`]: the same, or the literal `auto` (a text colour, a border's colour);
//! - [`highlight`]: one of Word's sixteen highlight names, or `none`;
//! - [`theme_name`] and [`two_hex`]: the name of a theme colour from Word's list, and a tint or a shade (`00` to `FF`).
//!
//! What is saved is another matter and is left alone: these read the file, nothing here writes to it (a value the file has stays in its XML,
//! byte for byte, whether or not it is a colour). The guard on the page's side is `hex()` in `packages/editor/src/editor/color.ts`, and the
//! test that holds the engine to this is `tests/all/hostile_colours.rs`.

/// Six hex digits, as capitals: `ff4f00` is `FF4F00`. Anything else (`auto`, `red`, `#FF4F00`, `FF4F0`, `FF0000;background:url(...)`) is none.
pub fn hex(v: &str) -> Option<String> {
    (v.len() == 6 && v.bytes().all(|c| c.is_ascii_hexdigit())).then(|| v.to_ascii_uppercase())
}

/// `hex`, with a leading `#` accepted (VML and CSS write it).
pub fn hex_hash(v: &str) -> Option<String> {
    hex(v.trim().trim_start_matches('#'))
}

/// A text colour or a line's: six hex digits, or `auto` (Word's automatic colour).
pub fn auto_or_hex(v: &str) -> Option<String> {
    if v.eq_ignore_ascii_case("auto") {
        return Some("auto".into());
    }
    hex(v)
}

/// Word's highlight colours, and `none`: the closed list (ST_HighlightColor).
const HIGHLIGHTS: [&str; 17] = [
    "black", "blue", "cyan", "green", "magenta", "red", "yellow", "white", "darkBlue", "darkCyan", "darkGreen", "darkMagenta", "darkRed", "darkYellow", "darkGray", "lightGray", "none",
];

/// A highlight: one of Word's names (`yellow`, `darkBlue`, ...) or `none`; anything else is none.
pub fn highlight(v: &str) -> Option<String> {
    HIGHLIGHTS.iter().find(|h| **h == v).map(|h| (*h).to_string())
}

/// The names a theme colour has in WordprocessingML (ST_ThemeColor) and in DrawingML's colour scheme.
const THEME_NAMES: [&str; 27] = [
    "dark1", "light1", "dark2", "light2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hyperlink", "followedHyperlink", "none", "text1", "text2", "background1", "background2", "dk1", "lt1",
    "dk2", "lt2", "hlink", "folHlink", "tx1", "tx2", "bg1", "bg2",
];

/// A theme colour's name from the lists Word uses (`accent1`, `text2`, `hlink`, ...); anything else is none.
pub fn theme_name(v: &str) -> Option<String> {
    THEME_NAMES.contains(&v).then(|| v.to_string())
}

/// A theme colour's tint or shade: two hex digits (`80`), as capitals.
pub fn two_hex(v: &str) -> Option<String> {
    (v.len() == 2 && v.bytes().all(|c| c.is_ascii_hexdigit())).then(|| v.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_six_digits_and_nothing_else() {
        assert_eq!(hex("ff4f00").as_deref(), Some("FF4F00"));
        assert_eq!(hex("FF4F00").as_deref(), Some("FF4F00"));
        for bad in ["", "auto", "red", "#FF4F00", "FF4F0", "FF4F000", "a;b:c)", "red;ba", "FF0000;background:url(http://evil.test/x)", "FF0000\" onload=\"x", "FF0000\n", " FF0000", "ＦＦ００００", "ééé"] {
            assert_eq!(hex(bad), None, "{bad:?}");
        }
        assert_eq!(hex_hash("#ff4f00").as_deref(), Some("FF4F00"));
        assert_eq!(hex_hash("red"), None);
    }

    #[test]
    fn auto_is_the_only_word() {
        assert_eq!(auto_or_hex("auto").as_deref(), Some("auto"));
        assert_eq!(auto_or_hex("AUTO").as_deref(), Some("auto"));
        assert_eq!(auto_or_hex("00ff00").as_deref(), Some("00FF00"));
        assert_eq!(auto_or_hex("none"), None);
        assert_eq!(auto_or_hex("auto;x:y"), None);
    }

    #[test]
    fn highlights_are_a_closed_list() {
        for name in HIGHLIGHTS {
            assert_eq!(highlight(name).as_deref(), Some(name));
        }
        for bad in ["", "Yellow", "FF0000", "yellow;background:url(http://evil.test/x)", "constructor", "__proto__", "orange"] {
            assert_eq!(highlight(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn theme_names_and_tints() {
        for ok in ["accent1", "text2", "background2", "bg2", "hlink", "followedHyperlink", "none"] {
            assert!(theme_name(ok).is_some(), "{ok}");
        }
        for bad in ["", "accent7", "accent1;x:y", "constructor", "FF0000"] {
            assert_eq!(theme_name(bad), None, "{bad:?}");
        }
        assert_eq!(two_hex("bf").as_deref(), Some("BF"));
        for bad in ["", "b", "bfa", "80;x", "zz"] {
            assert_eq!(two_hex(bad), None, "{bad:?}");
        }
    }
}
