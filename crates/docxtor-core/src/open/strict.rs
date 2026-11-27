//! Strict Open XML (ISO 29500 Strict): the same markup as the standard Transitional kind, under other
//! namespaces (`purl.oclc.org/ooxml/...`), with a few spellings changed.
//!
//! Docxtor reads it by mapping the namespaces to the Transitional ones in every XML part, and the few
//! spellings that differ (`start` and `end` where Transitional says `left` and `right`) too. The whole
//! package is rewritten, so what is saved is a consistent Transitional file that Word opens.

use crate::package::Package;
use crate::xml::{self, Node};

const STRICT: &str = "http://purl.oclc.org/ooxml/";

/// Strict namespace, Transitional namespace. More specific names come before `officeDocument/relationships`.
const MAP: &[(&str, &str)] = &[
    ("http://purl.oclc.org/ooxml/wordprocessingml/main", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"),
    ("http://purl.oclc.org/ooxml/officeDocument/relationships", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
    ("http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
    ("http://purl.oclc.org/ooxml/drawingml/main", "http://schemas.openxmlformats.org/drawingml/2006/main"),
    ("http://purl.oclc.org/ooxml/drawingml/picture", "http://schemas.openxmlformats.org/drawingml/2006/picture"),
    ("http://purl.oclc.org/ooxml/drawingml/chartDrawing", "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing"),
    ("http://purl.oclc.org/ooxml/drawingml/chart", "http://schemas.openxmlformats.org/drawingml/2006/chart"),
    ("http://purl.oclc.org/ooxml/drawingml/diagram", "http://schemas.openxmlformats.org/drawingml/2006/diagram"),
    ("http://purl.oclc.org/ooxml/drawingml/lockedCanvas", "http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas"),
    ("http://purl.oclc.org/ooxml/officeDocument/math", "http://schemas.openxmlformats.org/officeDocument/2006/math"),
    ("http://purl.oclc.org/ooxml/officeDocument/sharedTypes", "http://schemas.openxmlformats.org/officeDocument/2006/sharedTypes"),
    ("http://purl.oclc.org/ooxml/officeDocument/extendedProperties", "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"),
    ("http://purl.oclc.org/ooxml/officeDocument/customProperties", "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"),
    ("http://purl.oclc.org/ooxml/officeDocument/docPropsVTypes", "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"),
    ("http://purl.oclc.org/ooxml/officeDocument/bibliography", "http://schemas.openxmlformats.org/officeDocument/2006/bibliography"),
    ("http://purl.oclc.org/ooxml/officeDocument/characteristics", "http://schemas.openxmlformats.org/officeDocument/2006/characteristics"),
    ("http://purl.oclc.org/ooxml/officeDocument/additionalCharacteristics", "http://schemas.openxmlformats.org/officeDocument/2006/additional-characteristics"),
];

/// Whether the package is Strict Open XML: its relationships use the `purl.oclc.org` namespaces.
pub fn is_strict(pkg: &Package) -> bool {
    pkg.get("_rels/.rels").is_some_and(|r| contains(r, STRICT))
        || pkg.parts.iter().any(|p| (p.name == "word/document.xml" || p.name.ends_with("/document.xml")) && contains(&p.data[..p.data.len().min(4096)], "purl.oclc.org/ooxml/wordprocessingml"))
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Map every XML part to the Transitional namespaces and spellings. Returns how many parts changed.
pub fn to_transitional(pkg: &mut Package) -> usize {
    let mut changed = 0;
    for part in pkg.parts.iter_mut() {
        if !(part.name.ends_with(".xml") || part.name.ends_with(".rels")) || !contains(&part.data, STRICT) && !is_word_part(&part.name) {
            continue;
        }
        let mut text = super::xmlfix::decode(&part.data);
        let mut touched = text.contains(STRICT);
        if touched {
            for (from, to) in MAP {
                text = text.replace(from, to);
            }
        }
        if is_word_part(&part.name) && needs_spelling(&text) {
            if let Ok(mut doc) = xml::parse(text.as_bytes()) {
                if respell(&mut doc.root) {
                    text = String::from_utf8_lossy(&doc.to_bytes()).into_owned();
                    touched = true;
                }
            }
        }
        if touched {
            part.data = text.into_bytes();
            changed += 1;
        }
    }
    changed
}

fn is_word_part(name: &str) -> bool {
    name.starts_with("word/") && name.ends_with(".xml") && !name.starts_with("word/theme/") && !name.starts_with("word/charts/") && !name.starts_with("word/diagrams/")
}

fn needs_spelling(text: &str) -> bool {
    text.contains("w:start") || text.contains("w:end") || text.contains("=\"start\"") || text.contains("=\"end\"") || ["pt\"", "mm\"", "cm\"", "in\"", "pc\"", "pi\""].iter().any(|u| text.contains(u))
}

/// The attributes of the Word namespace that are measures in twips: Strict lets them say `5.40pt` or `1.5cm`, Transitional
/// wants a whole number of twips. (Found by the Open XML validator on the Strict files of the corpus: `w:w="5.40pt"` in a
/// table style's cell margins was saved in a file that is otherwise Transitional.) Measures in other units (half-points,
/// eighths of a point, 240ths of a line) are left alone.
const TWIPS: &[&str] = &["w:w", "w:h", "w:left", "w:right", "w:top", "w:bottom", "w:start", "w:end", "w:hanging", "w:firstLine", "w:before", "w:after", "w:header", "w:footer", "w:gutter", "w:pos", "w:x", "w:y", "w:width", "w:height"];

/// `5.40pt`, `-1in`, `1.5cm`, `3mm`, `1pc` as twips, or None when it is not a measure with a unit.
fn twips_of(v: &str) -> Option<String> {
    let v = v.trim();
    let (num, per) = [("pt", 20.0), ("in", 1440.0), ("cm", 1440.0 / 2.54), ("mm", 144.0 / 2.54), ("pc", 240.0), ("pi", 240.0)].iter().find_map(|(u, k)| v.strip_suffix(u).map(|n| (n, *k)))?;
    let n: f64 = num.trim().parse().ok()?;
    n.is_finite().then(|| format!("{}", (n * per).round() as i64))
}

/// Parents whose `start` and `end` children are sides, not a number.
const SIDES: &[&str] = &["w:pBdr", "w:tcBorders", "w:tblBorders", "w:tcMar", "w:tblCellMar", "w:pgBorders", "w:tblPrEx"];

fn respell(el: &mut xml::Element) -> bool {
    let mut changed = false;
    let parent_sides = SIDES.contains(&el.name.as_str()) || el.name == "w:tblPrEx";
    // `w:jc`, `w:lvlJc` and `w:tab` say start and end where Transitional says left and right.
    if matches!(el.name.as_str(), "w:jc" | "w:lvlJc" | "w:tab") {
        if let Some((_, v)) = el.attrs.iter_mut().find(|(k, _)| k == "w:val") {
            if v == "start" {
                *v = "left".into();
                changed = true;
            } else if v == "end" {
                *v = "right".into();
                changed = true;
            }
        }
    }
    for (k, v) in el.attrs.iter_mut() {
        if TWIPS.contains(&k.as_str()) {
            if let Some(t) = twips_of(v) {
                *v = t.into();
                changed = true;
            }
        }
    }
    if el.name == "w:ind" {
        for (k, _) in el.attrs.iter_mut() {
            if k == "w:start" {
                *k = "w:left".into();
                changed = true;
            } else if k == "w:end" {
                *k = "w:right".into();
                changed = true;
            }
        }
    }
    for child in el.children.iter_mut() {
        if let Node::Element(c) = child {
            if parent_sides {
                if c.name == "w:start" {
                    c.name = "w:left".into();
                    changed = true;
                } else if c.name == "w:end" {
                    c.name = "w:right".into();
                    changed = true;
                }
            }
            changed |= respell(c);
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spellings_are_mapped_only_where_they_are_sides() {
        let src = r#"<w:p xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main"><w:pPr><w:jc w:val="start"/><w:ind w:start="720" w:end="0"/><w:pBdr><w:start w:val="single"/></w:pBdr></w:pPr><w:lvl><w:start w:val="1"/></w:lvl></w:p>"#;
        let mut d = xml::parse(src.as_bytes()).unwrap();
        assert!(respell(&mut d.root));
        let out = d.root.to_xml();
        assert!(out.contains(r#"<w:jc w:val="left"/>"#) && out.contains(r#"w:left="720""#) && out.contains(r#"w:right="0""#), "{out}");
        assert!(out.contains("<w:pBdr><w:left"), "{out}");
        // A start number stays a start number.
        assert!(out.contains(r#"<w:lvl><w:start w:val="1"/>"#), "{out}");
    }

    #[test]
    fn measures_with_a_unit_become_twips() {
        assert_eq!(twips_of("5.40pt").as_deref(), Some("108"));
        assert_eq!(twips_of("1in").as_deref(), Some("1440"));
        assert_eq!(twips_of("-0.5in").as_deref(), Some("-720"));
        assert_eq!(twips_of("2.54cm").as_deref(), Some("1440"));
        assert_eq!(twips_of("25.4mm").as_deref(), Some("1440"));
        assert_eq!(twips_of("1pc").as_deref(), Some("240"));
        assert_eq!(twips_of("720"), None);
        assert_eq!(twips_of("auto"), None);
        let src = r#"<w:tblCellMar xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main"><w:left w:w="5.40pt" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar>"#;
        let mut d = xml::parse(src.as_bytes()).unwrap();
        assert!(respell(&mut d.root));
        let out = d.root.to_xml();
        assert!(out.contains(r#"<w:left w:w="108" w:type="dxa"/>"#) && out.contains(r#"<w:right w:w="108" w:type="dxa"/>"#), "{out}");
    }
}
