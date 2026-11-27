//! The rest of a document's properties: the extended ones in `docProps/app.xml` (company, manager and the
//! statistics Word keeps there) and the custom ones in `docProps/custom.xml` (name, type and value, as Word's
//! Properties dialog shows them on its Custom tab). `docprops.rs` has the core ones.

use crate::xml::{Element, Node, XmlDoc};
use serde::{Deserialize, Serialize};

pub const REL_APP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
pub const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";
const NS_APP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";
const NS_VT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes";
/// The format id every custom property of Office documents carries.
const FMTID: &str = "{D5CDD505-2E9C-101B-9397-08002B2CF9AE}";

// ---------------------------------------------------------------------------
// Extended properties (app.xml)
// ---------------------------------------------------------------------------

/// An empty extended-properties part.
pub fn new_app_doc() -> XmlDoc {
    XmlDoc::new(
        Element::new("Properties")
            .with_attr("xmlns", NS_APP)
            .with_attr("xmlns:vt", NS_VT),
    )
}

/// The text of one extended property (`Company`, `Words`...), when it is there and not blank.
pub fn app_text(app: Option<&XmlDoc>, local: &str) -> Option<String> {
    app?.root
        .elements()
        .find(|e| e.local() == local)
        .map(|e| e.text())
        .filter(|t| !t.trim().is_empty())
}

/// Set one extended property; an empty value removes it. Other properties are left as they are.
pub fn set_app_text(app: &mut XmlDoc, local: &str, value: &str) {
    set_app_value(app, local, value, false);
}

/// The same, and with `exact` the value is kept as given (no trimming): a content control bound to the property shows
/// exactly what the file holds, spaces and line breaks included (the same rule as `docprops::CoreChanges::exact`).
pub fn set_app_value(app: &mut XmlDoc, local: &str, value: &str, exact: bool) {
    let value = if exact { value } else { value.trim() };
    let at = app
        .root
        .children
        .iter()
        .position(|n| matches!(n, Node::Element(e) if e.local() == local));
    match (value.is_empty(), at) {
        (true, Some(i)) => {
            app.root.children.remove(i);
        }
        (true, None) => {}
        (false, Some(i)) => {
            if let Node::Element(e) = &mut app.root.children[i] {
                e.children = vec![Node::Text(value.to_string())];
            }
        }
        (false, None) => {
            // A part written as `<ap:Properties xmlns:ap="…">` has its properties in that prefix too; a new element with no prefix
            // there would be in no namespace at all.
            let name = match app.root.name.split_once(':') {
                Some((prefix, _)) => format!("{prefix}:{local}"),
                None => local.to_string(),
            };
            app.root
                .children
                .push(Node::Element(Element::new(name).with_text(value)));
        }
    }
}

/// What Word keeps in `app.xml` about the size of the document.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Known only to the page that lays the document out; left as the file has it when `None`.
    pub pages: Option<u32>,
    pub words: usize,
    /// Characters without spaces (`Characters`).
    pub characters: usize,
    pub characters_with_spaces: usize,
    pub paragraphs: usize,
    pub lines: Option<u32>,
}

/// Count words, characters and paragraphs of some paragraph texts the way Word's statistics do: words are runs
/// of non-blank characters, `Characters` leaves out spaces, and only paragraphs with text count.
pub fn count(texts: &[String]) -> Stats {
    Stats {
        pages: None,
        words: texts.iter().map(|t| t.split_whitespace().count()).sum(),
        characters: texts
            .iter()
            .map(|t| t.chars().filter(|c| !c.is_whitespace()).count())
            .sum(),
        characters_with_spaces: texts.iter().map(|t| t.chars().count()).sum(),
        paragraphs: texts.iter().filter(|t| !t.trim().is_empty()).count(),
        lines: None,
    }
}

/// Write the statistics into `app.xml`, the way Word does on every save. `Pages` and `Lines` are only touched
/// when the page knew them.
pub fn write_stats(app: &mut XmlDoc, s: &Stats) {
    if let Some(p) = s.pages {
        set_app_text(app, "Pages", &p.to_string());
    }
    set_app_text(app, "Words", &s.words.to_string());
    set_app_text(app, "Characters", &s.characters.to_string());
    set_app_text(
        app,
        "CharactersWithSpaces",
        &s.characters_with_spaces.to_string(),
    );
    set_app_text(app, "Paragraphs", &s.paragraphs.to_string());
    if let Some(l) = s.lines {
        set_app_text(app, "Lines", &l.to_string());
    }
}

// ---------------------------------------------------------------------------
// Custom properties (custom.xml)
// ---------------------------------------------------------------------------

/// The kinds of value Word's Custom tab offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropKind {
    Text,
    Number,
    Date,
    Bool,
}

/// A custom property: a name, a kind and the value as text (`true` or `false` for yes or no, a date as
/// `2026-10-14`, a number as written).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProp {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: PropKind,
    pub value: String,
}

/// The properties people see: those whose names do not start with an underscore (`_MarkAsFinal` and the other
/// private ones stay out of the list). Values of types the dialog does not know (blobs, vectors) are left out too.
pub fn read_custom(doc: Option<&XmlDoc>) -> Vec<CustomProp> {
    let Some(doc) = doc else { return Vec::new() };
    doc.root
        .elements()
        .filter(|p| p.local() == "property")
        .filter_map(|p| {
            let name = p.attr("name")?;
            if name.starts_with('_') {
                return None;
            }
            let v = p.elements().next()?;
            let text = v.text().trim().to_string();
            let (kind, value) = match v.local() {
                "lpwstr" | "lpstr" | "bstr" => (PropKind::Text, v.text()),
                "i1" | "i2" | "i4" | "i8" | "int" | "ui1" | "ui2" | "ui4" | "ui8" | "uint"
                | "r4" | "r8" | "decimal" => (PropKind::Number, text),
                "bool" => (
                    PropKind::Bool,
                    if matches!(text.as_str(), "true" | "1") {
                        "true".into()
                    } else {
                        "false".into()
                    },
                ),
                "filetime" | "date" => (
                    PropKind::Date,
                    text.split('T').next().unwrap_or("").to_string(),
                ),
                _ => return None,
            };
            Some(CustomProp {
                name: name.to_string(),
                kind,
                value,
            })
        })
        .collect()
}

fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && matches!(s[5..7].parse::<u32>(), Ok(1..=12))
        && matches!(s[8..10].parse::<u32>(), Ok(1..=31))
}

/// The element Word writes for a value, checking it fits its kind.
fn value_element(kind: PropKind, value: &str) -> Result<Element, String> {
    Ok(match kind {
        PropKind::Text => Element::new("vt:lpwstr").with_text(value),
        PropKind::Number => {
            let v = value.trim();
            if let Ok(i) = v.parse::<i32>() {
                Element::new("vt:i4").with_text(i.to_string())
            } else if v.parse::<f64>().is_ok_and(|f| f.is_finite()) {
                Element::new("vt:r8").with_text(v)
            } else {
                return Err(format!("'{v}' is not a number"));
            }
        }
        PropKind::Bool => match value.trim().to_lowercase().as_str() {
            "true" | "yes" | "1" => Element::new("vt:bool").with_text("true"),
            "false" | "no" | "0" => Element::new("vt:bool").with_text("false"),
            other => return Err(format!("'{other}' is not yes or no")),
        },
        PropKind::Date => {
            let day = value.trim().split('T').next().unwrap_or("");
            if !valid_date(day) {
                return Err(format!(
                    "'{}' is not a date; use a form like 2026-10-14",
                    value.trim()
                ));
            }
            Element::new("vt:filetime").with_text(format!("{day}T00:00:00Z"))
        }
    })
}

/// Add a custom property, or change the one with that name (names are compared ignoring case, as Word does).
pub fn set_custom(doc: &mut XmlDoc, prop: &CustomProp) -> Result<(), String> {
    let name = prop.name.trim();
    if name.is_empty() {
        return Err("a custom property needs a name".into());
    }
    if name.starts_with('_') {
        return Err("names that start with an underscore are kept for Word".into());
    }
    let value = value_element(prop.kind, &prop.value)?;
    let same = |n: &Node| matches!(n, Node::Element(p) if p.local() == "property" && p.attr("name").is_some_and(|x| x.eq_ignore_ascii_case(name)));
    if let Some(i) = doc.root.children.iter().position(same) {
        if let Node::Element(p) = &mut doc.root.children[i] {
            p.children = vec![Node::Element(value)];
            p.set_attr("name", name);
        }
        return Ok(());
    }
    let pid = doc
        .root
        .elements()
        .filter_map(|p| p.attr("pid"))
        .filter_map(|v| v.parse::<u32>().ok())
        .max()
        .unwrap_or(1)
        + 1;
    let el = Element::new("property")
        .with_attr("fmtid", FMTID)
        .with_attr("pid", pid.to_string())
        .with_attr("name", name)
        .with_child(value);
    doc.root.children.push(Node::Element(el));
    Ok(())
}

/// Remove the custom property with this name; false when there is none.
pub fn remove_custom(doc: &mut XmlDoc, name: &str) -> bool {
    let name = name.trim();
    if name.starts_with('_') {
        return false;
    }
    let before = doc.root.children.len();
    doc.root.children.retain(|n| !matches!(n, Node::Element(p) if p.local() == "property" && p.attr("name").is_some_and(|x| x.eq_ignore_ascii_case(name))));
    doc.root.children.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom() -> XmlDoc {
        crate::protection::new_custom_doc()
    }

    #[test]
    fn a_new_extended_property_takes_the_prefix_the_part_uses() {
        let ns = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";
        let prefixed = format!(r#"<ap:Properties xmlns:ap="{ns}"><ap:Application>Gen</ap:Application></ap:Properties>"#);
        let mut app = crate::xml::parse(prefixed.as_bytes()).unwrap();
        set_app_text(&mut app, "Company", "Acme");
        set_app_text(&mut app, "Words", "12");
        // An existing one keeps its name; reading it back works.
        set_app_text(&mut app, "Application", "Other");
        let xml = String::from_utf8(app.to_bytes()).unwrap();
        assert!(xml.contains("<ap:Company>Acme</ap:Company>"), "{xml}");
        assert!(xml.contains("<ap:Words>12</ap:Words>"), "{xml}");
        assert!(xml.contains("<ap:Application>Other</ap:Application>"), "{xml}");
        assert!(!xml.contains("<Company>") && !xml.contains("<Words>"), "{xml}");
        assert_eq!(app_text(Some(&app), "Company").as_deref(), Some("Acme"));
        // The usual form (Word's own) is as before.
        let mut plain = new_app_doc();
        set_app_text(&mut plain, "Company", "Acme");
        assert!(String::from_utf8(plain.to_bytes()).unwrap().contains("<Company>Acme</Company>"));
    }

    #[test]
    fn custom_properties_round_trip_as_word_writes_them() {
        let mut d = custom();
        for (name, kind, value) in [
            ("Project", PropKind::Text, "Babbage"),
            ("Revision", PropKind::Number, "7"),
            ("Ratio", PropKind::Number, "0.25"),
            ("Reviewed", PropKind::Bool, "yes"),
            ("Due", PropKind::Date, "2026-10-14"),
        ] {
            set_custom(
                &mut d,
                &CustomProp {
                    name: name.into(),
                    kind,
                    value: value.into(),
                },
            )
            .unwrap();
        }
        let xml = String::from_utf8(d.to_bytes()).unwrap();
        for want in [
            "<vt:lpwstr>Babbage</vt:lpwstr>",
            "<vt:i4>7</vt:i4>",
            "<vt:r8>0.25</vt:r8>",
            "<vt:bool>true</vt:bool>",
            "<vt:filetime>2026-10-14T00:00:00Z</vt:filetime>",
            r#"pid="2""#,
            r#"pid="6""#,
        ] {
            assert!(xml.contains(want), "{want} in {xml}");
        }
        let back = read_custom(Some(&d));
        assert_eq!(back.len(), 5);
        assert_eq!(
            back[3],
            CustomProp {
                name: "Reviewed".into(),
                kind: PropKind::Bool,
                value: "true".into()
            }
        );
        assert_eq!(back[4].value, "2026-10-14");
        // Changing one keeps its place and number; names compare ignoring case.
        set_custom(
            &mut d,
            &CustomProp {
                name: "project".into(),
                kind: PropKind::Text,
                value: "Engine".into(),
            },
        )
        .unwrap();
        let back = read_custom(Some(&d));
        assert_eq!(
            (back[0].name.as_str(), back[0].value.as_str()),
            ("project", "Engine")
        );
        assert!(remove_custom(&mut d, "REVISION") && !remove_custom(&mut d, "Revision"));
        assert_eq!(read_custom(Some(&d)).len(), 4);
    }

    #[test]
    fn bad_values_and_private_names_are_refused() {
        let mut d = custom();
        let set = |d: &mut XmlDoc, name: &str, kind, value: &str| {
            set_custom(
                d,
                &CustomProp {
                    name: name.into(),
                    kind,
                    value: value.into(),
                },
            )
        };
        assert!(set(&mut d, "", PropKind::Text, "x").is_err());
        assert!(set(&mut d, "_Hidden", PropKind::Text, "x").is_err());
        assert!(set(&mut d, "N", PropKind::Number, "twelve").is_err());
        assert!(set(&mut d, "B", PropKind::Bool, "maybe").is_err());
        assert!(set(&mut d, "D", PropKind::Date, "14/10/2026").is_err());
        assert!(set(&mut d, "D", PropKind::Date, "2026-13-01").is_err());
        assert!(read_custom(Some(&d)).is_empty());
    }

    #[test]
    fn private_properties_stay_out_of_the_list_and_survive() {
        let mut d = custom();
        crate::protection::set_marked_final(&mut d, true);
        set_custom(
            &mut d,
            &CustomProp {
                name: "A".into(),
                kind: PropKind::Text,
                value: "1".into(),
            },
        )
        .unwrap();
        assert_eq!(read_custom(Some(&d)).len(), 1);
        assert!(!remove_custom(&mut d, "_MarkAsFinal"));
        assert!(String::from_utf8(d.to_bytes())
            .unwrap()
            .contains("_MarkAsFinal"));
    }

    #[test]
    fn statistics_follow_words_characters_and_paragraphs() {
        let texts: Vec<String> = ["Hello big world", "", "  ", "Ünï cödé"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let s = count(&texts);
        assert_eq!(
            (
                s.words,
                s.characters,
                s.characters_with_spaces,
                s.paragraphs
            ),
            (5, 20, 25, 2)
        );
        let mut app = new_app_doc();
        set_app_text(&mut app, "Company", "Analytical");
        write_stats(
            &mut app,
            &Stats {
                pages: Some(3),
                lines: None,
                ..s
            },
        );
        assert_eq!(app_text(Some(&app), "Words").as_deref(), Some("5"));
        assert_eq!(app_text(Some(&app), "Pages").as_deref(), Some("3"));
        assert_eq!(app_text(Some(&app), "Lines"), None);
        assert_eq!(
            app_text(Some(&app), "Company").as_deref(),
            Some("Analytical")
        );
        set_app_text(&mut app, "Company", "");
        assert_eq!(app_text(Some(&app), "Company"), None);
    }
}
