//! Bullet characters written in symbol fonts, mapped to the Unicode characters that look the same.
//!
//! Word stores a bullet as a private-use character (`U+F0B7`) or a plain letter (`l`) that only means
//! something in Symbol, Wingdings and similar fonts. Browsers on other systems don't have those fonts, so a
//! bullet is drawn as the Unicode character with the same look. Other agents map symbol fonts in running
//! text; this table is only for list bullets.

/// Whether `font` is a font whose characters mean something else than their Unicode value.
pub fn is_symbol_font(font: Option<&str>) -> bool {
    matches!(font.map(|f| f.trim().to_ascii_lowercase()).as_deref(), Some("symbol" | "wingdings" | "wingdings 2" | "wingdings 3" | "webdings" | "zapfdingbats" | "courier new"))
}

/// The character with the same look as bullet `c` in `font`; the bullet itself when it needs no mapping.
fn map_char(c: char, font: Option<&str>) -> char {
    let u = c as u32;
    let family = font.map(|f| f.trim().to_ascii_lowercase());
    let private = (0xF020..=0xF0FF).contains(&u);
    let low = if private { u - 0xF000 } else { u };
    let symbol_family = matches!(family.as_deref(), Some("symbol" | "wingdings" | "wingdings 2" | "wingdings 3" | "webdings" | "zapfdingbats"));
    // A plain character only means something else in a symbol font. Courier New has the one circle.
    if !private && !symbol_family {
        return if family.as_deref() == Some("courier new") && c == 'o' { '\u{25E6}' } else { c };
    }
    let mapped = match (family.as_deref(), low) {
        (Some("symbol"), 0xB7) => '\u{2022}',
        (Some("symbol"), 0xA8) => '\u{2666}',
        (Some("symbol"), 0xA7) => '\u{2663}',
        (Some("symbol"), 0xA9) => '\u{2665}',
        (Some("symbol"), 0xAA) => '\u{2660}',
        (Some("symbol"), 0xAE) => '\u{2192}',
        (Some("symbol"), 0xB0) => '\u{25E6}',
        (Some("wingdings 3"), 0x75) => '\u{25B2}',
        (Some("wingdings 3"), 0x76) => '\u{25BC}',
        (Some("wingdings 3"), 0x7D) => '\u{25B6}',
        (Some("wingdings 3"), 0x7B) => '\u{25C0}',
        (Some("wingdings 2"), 0x97 | 0x98) => '\u{25CF}',
        (Some("wingdings 2"), 0xA2) => '\u{25CF}',
        (Some("wingdings 2"), 0xA3) => '\u{25A0}',
        (Some("wingdings 2"), 0xA1) => '\u{25CB}',
        (Some("wingdings 2"), 0x50) => '\u{2713}',
        (Some("wingdings 2"), 0x52) => '\u{2717}',
        (_, 0xB7) => '\u{2022}',
        (_, 0xA7) => '\u{25AA}',
        (_, 0x6C) => '\u{25CF}',
        (_, 0x6D) => '\u{25CB}',
        (_, 0x6E) => '\u{25A0}',
        (_, 0x6F) => '\u{25A1}',
        (_, 0x70) => '\u{274F}',
        (_, 0x71) => '\u{2751}',
        (_, 0x72) => '\u{2752}',
        (_, 0x73..=0x75 | 0x77) => '\u{25C6}',
        (_, 0x76) => '\u{2756}',
        (_, 0x78) => '\u{2327}',
        (_, 0x4A) => '\u{263A}',
        (_, 0xA1) => '\u{25CB}',
        (_, 0xA8) => '\u{25FB}',
        (_, 0xAB) => '\u{2605}',
        (_, 0x9E | 0x9F) => '\u{2022}',
        (_, 0xD8) => '\u{27A2}',
        (_, 0xE0 | 0xE8) => '\u{2794}',
        (_, 0xF0) => '\u{21E8}',
        (_, 0xFB) => '\u{2717}',
        (_, 0xFC) => '\u{2714}',
        (_, 0xFE) => '\u{2611}',
        _ if private => '\u{2022}',
        _ => c,
    };
    mapped
}

/// The text of a bullet label, with symbol-font characters replaced; a plain bullet when the level has none.
pub fn map_bullet(text: &str, font: Option<&str>) -> String {
    let mapped: String = text.chars().map(|c| map_char(c, font)).collect();
    if mapped.is_empty() {
        "\u{2022}".into()
    } else {
        mapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_and_wingdings_bullets() {
        assert_eq!(map_bullet("\u{F0B7}", Some("Symbol")), "\u{2022}");
        assert_eq!(map_bullet("\u{F0B7}", None), "\u{2022}");
        assert_eq!(map_bullet("\u{B7}", Some("Symbol")), "\u{2022}");
        assert_eq!(map_bullet("\u{F0A7}", Some("Wingdings")), "\u{25AA}");
        assert_eq!(map_bullet("\u{F0D8}", Some("Wingdings")), "\u{27A2}");
        assert_eq!(map_bullet("\u{F0FC}", Some("Wingdings")), "\u{2714}");
        assert_eq!(map_bullet("\u{F076}", Some("Wingdings")), "\u{2756}");
        assert_eq!(map_bullet("\u{F06E}", Some("Wingdings")), "\u{25A0}");
        assert_eq!(map_bullet("l", Some("Wingdings")), "\u{25CF}");
        assert_eq!(map_bullet("\u{F0A8}", Some("Symbol")), "\u{2666}");
        assert_eq!(map_bullet("\u{F075}", Some("Wingdings 3")), "\u{25B2}");
        // A character in an ordinary font is left alone, and an unknown private-use one becomes a bullet.
        assert_eq!(map_bullet("-", Some("Calibri")), "-");
        assert_eq!(map_bullet("o", Some("Courier New")), "\u{25E6}");
        assert_eq!(map_bullet("\u{F0FF}", Some("MT Extra")), "\u{2022}");
        assert_eq!(map_bullet("", None), "\u{2022}");
    }
}
