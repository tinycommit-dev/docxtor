//! Fonts embedded in a .docx (Word: Options > Save > Embed fonts). They live in `word/fonts/*.odttf`,
//! listed in the font table, with their first 32 bytes XOR-ed with a key taken from a GUID
//! (ECMA-376 Part 1, 17.8.1). Reading them lets a document show in its own fonts; the package
//! itself is never changed.

use crate::model::{walk_paragraphs_mut, Block, Inline};
use crate::package::Package;
use crate::styles::ThemeFonts;
use crate::xml::Element;

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddedFont {
    /// The family name documents use, e.g. `Lato`.
    pub family: String,
    /// `regular`, `bold`, `italic` or `boldItalic`.
    pub style: &'static str,
    /// The font file (TrueType or OpenType), ready to load.
    pub data: Vec<u8>,
}

/// The 16 key bytes of a font key like `{1A2B3C4D-…}`: the GUID's bytes as written, in reverse.
fn key_bytes(guid: &str) -> Option<[u8; 16]> {
    let hex: String = guid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return None;
    }
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        let j = 15 - i;
        *k = u8::from_str_radix(&hex[j * 2..j * 2 + 2], 16).ok()?;
    }
    Some(key)
}

/// Undo (or apply: it's symmetric) the obfuscation of an embedded font.
pub fn deobfuscate(data: &mut [u8], guid: &str) -> bool {
    let Some(key) = key_bytes(guid) else { return false };
    if data.len() < 32 {
        return false;
    }
    for i in 0..32 {
        data[i] ^= key[i % 16];
    }
    true
}

/// Every embedded font the font table lists, deobfuscated.
pub fn embedded_fonts(pkg: &Package, main: &str) -> Vec<EmbeddedFont> {
    let Ok(rels) = pkg.rels_for(main) else { return Vec::new() };
    let Some(table) = rels.iter().find(|r| r.rel_type.ends_with("/fontTable")).and_then(|r| pkg.resolve_rel(main, &r.id)) else {
        return Vec::new();
    };
    let Ok(doc) = pkg.xml(&table.target) else { return Vec::new() };
    let mut out = Vec::new();
    for font in doc.root.elements().filter(|e| e.local() == "font") {
        let Some(family) = font.attr("w:name") else { continue };
        for (tag, style) in [("w:embedRegular", "regular"), ("w:embedBold", "bold"), ("w:embedItalic", "italic"), ("w:embedBoldItalic", "boldItalic")] {
            let Some(e) = font.child(tag) else { continue };
            let Some(rel) = e.attr("r:id").and_then(|id| pkg.resolve_rel(&table.target, id)) else { continue };
            let Some(bytes) = pkg.get(&rel.target) else { continue };
            let mut data = bytes.to_vec();
            // Fonts saved without a key are stored as they are.
            if let Some(guid) = e.attr("w:fontKey") {
                if !deobfuscate(&mut data, guid) {
                    continue;
                }
            }
            out.push(EmbeddedFont { family: family.to_string(), style, data });
        }
    }
    out
}

/// The font families a document names without rendering it: the font table's (Word lists every font the file uses there), the theme's
/// heading and body fonts, and the fonts its styles name. For loading faces before the first layout; `Document::fonts_used` is the exact list
/// of what the text uses, and costs a render.
pub fn named_families(pkg: &Package, main: &str, styles: &crate::styles::StyleSheet) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        let name = name.trim();
        if !name.is_empty() && !out.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            out.push(name.to_string());
        }
    };
    if let Ok(rels) = pkg.rels_for(main) {
        if let Some(table) = rels.iter().find(|r| r.rel_type.ends_with("/fontTable")).and_then(|r| pkg.resolve_rel(main, &r.id)) {
            if let Ok(doc) = pkg.xml(&table.target) {
                for font in doc.root.elements().filter(|e| e.local() == "font") {
                    if let Some(name) = font.attr("w:name") {
                        add(name);
                    }
                }
            }
        }
    }
    for name in [&styles.theme.minor, &styles.theme.major].into_iter().flatten() {
        add(name);
    }
    if let Some(doc) = &styles.doc {
        fn walk(e: &Element, add: &mut dyn FnMut(&str)) {
            if e.name == "w:rFonts" {
                for a in ["w:ascii", "w:hAnsi"] {
                    if let Some(v) = e.attr(a) {
                        add(v);
                    }
                }
            }
            for c in e.elements() {
                walk(c, add);
            }
        }
        walk(&doc.root, &mut add);
    }
    out
}

/// A font family the document's text uses, and how many characters use it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FontUse {
    pub family: String,
    pub chars: usize,
}

/// Point every `w:rFonts` in `el` (and below it) that names `from` at `to`, including theme
/// references that resolve to `from` (they become explicit, the way Word applies a font).
/// Recorded formatting changes (`w:rPrChange`) keep the old names. Returns whether anything changed.
pub(crate) fn replace_in(el: &mut Element, from: &str, to: &str, theme: &ThemeFonts) -> bool {
    if el.name == "w:rPrChange" || el.name == "w:pPrChange" {
        return false;
    }
    let mut changed = false;
    if el.name == "w:rFonts" {
        let same = |v: Option<&str>| v.is_some_and(|v| v.eq_ignore_ascii_case(from));
        let themed = el.attr("w:asciiTheme").or(el.attr("w:hAnsiTheme")).map(String::from);
        if let Some(t) = themed {
            if same(theme.resolve(&t).as_deref()) {
                el.remove_attr("w:asciiTheme");
                el.remove_attr("w:hAnsiTheme");
                el.set_attr("w:ascii", to);
                el.set_attr("w:hAnsi", to);
                changed = true;
            }
        }
        for a in ["w:ascii", "w:hAnsi", "w:cs", "w:eastAsia"] {
            if same(el.attr(a)) {
                el.set_attr(a, to);
                changed = true;
            }
        }
    }
    for c in el.elements_mut() {
        changed |= replace_in(c, from, to, theme);
    }
    changed
}

/// The same, for paragraphs: their marks and every run, link and field inside them.
pub(crate) fn replace_in_blocks(blocks: &mut [Block], from: &str, to: &str, theme: &ThemeFonts) -> bool {
    fn inlines(list: &mut [Inline], from: &str, to: &str, theme: &ThemeFonts) -> bool {
        let mut changed = false;
        for i in list {
            changed |= match i {
                Inline::Run(r) => r.rpr.as_mut().is_some_and(|e| replace_in(e, from, to, theme)),
                Inline::Container(c) => inlines(&mut c.children, from, to, theme),
                Inline::Opaque(e) => replace_in(e, from, to, theme),
            };
        }
        changed
    }
    let mut changed = false;
    walk_paragraphs_mut(blocks, &mut |p| {
        if let Some(e) = p.ppr.as_mut() {
            changed |= replace_in(e, from, to, theme);
        }
        changed |= inlines(&mut p.inlines, from, to, theme);
    });
    changed
}
