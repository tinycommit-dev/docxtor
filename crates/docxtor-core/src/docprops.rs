//! Document properties (`docProps/core.xml`) and the document language.

use crate::model::*;
use crate::props::{put_ordered, put_rpr};
use crate::xml::{Element, Node, XmlDoc};
use serde::Serialize;

const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The properties people edit: what `docProps/core.xml` says (absent ones are left out).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocProps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When the file was made (`dcterms:created`), as the file has it: a W3CDTF date such as `2026-10-13T09:30:00Z`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// When the file was last changed (`dcterms:modified`), as the file has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Who changed it last (`cp:lastModifiedBy`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by: Option<String>,
    /// Word's Category (`cp:category`) and Status (`cp:contentStatus`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The Company and Manager of `docProps/app.xml`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager: Option<String>,
    /// The custom properties (`docProps/custom.xml`), without the private ones whose names start with `_`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<crate::docmeta::CustomProp>,
}

/// An empty set of core properties (the file Word would write for a document with none).
pub fn new_core_doc() -> XmlDoc {
    XmlDoc::new(
        Element::new("cp:coreProperties")
            .with_attr("xmlns:cp", NS_CP)
            .with_attr("xmlns:dc", NS_DC)
            .with_attr("xmlns:dcterms", NS_DCTERMS)
            .with_attr("xmlns:xsi", NS_XSI),
    )
}

fn text_of(root: &Element, local: &str) -> Option<String> {
    root.elements().find(|e| e.local() == local).map(|e| e.text()).filter(|t| !t.trim().is_empty())
}

pub fn read(core: Option<&XmlDoc>) -> DocProps {
    let Some(c) = core else { return DocProps::default() };
    DocProps {
        title: text_of(&c.root, "title"),
        subject: text_of(&c.root, "subject"),
        author: text_of(&c.root, "creator"),
        keywords: text_of(&c.root, "keywords"),
        description: text_of(&c.root, "description"),
        created: text_of(&c.root, "created"),
        modified: text_of(&c.root, "modified"),
        last_modified_by: text_of(&c.root, "lastModifiedBy"),
        category: text_of(&c.root, "category"),
        status: text_of(&c.root, "contentStatus"),
        ..Default::default()
    }
}

/// Set a date property the way Word writes it: `<dcterms:modified xsi:type="dcterms:W3CDTF">…</dcterms:modified>`.
fn set_date(root: &mut Element, local: &str, value: &str) {
    let prefix = prefix_for(root, NS_DCTERMS, "dcterms");
    let name = format!("{prefix}:{local}");
    if let Some(Node::Element(e)) = root.children.iter_mut().find(|n| matches!(n, Node::Element(e) if e.name == name)) {
        e.children = vec![Node::Text(value.to_string())];
        return;
    }
    let xsi = prefix_for(root, NS_XSI, "xsi");
    root.children.push(Node::Element(Element::new(&name).with_attr(format!("{xsi}:type"), format!("{prefix}:W3CDTF")).with_text(value)));
}

/// Record a save: `dcterms:modified` becomes `now`, and `dcterms:created` too when `created` is set and the
/// file has no created date yet. Nothing else changes. `now` is an ISO 8601 UTC time such as `2026-10-13T09:30:00Z`.
pub fn stamp(core: &mut XmlDoc, now: &str, created: bool) {
    let root = &mut core.root;
    set_date(root, "modified", now);
    let has_created = root.elements().any(|e| e.local() == "created");
    if created && !has_created {
        set_date(root, "created", now);
    }
}

/// Set `cp:lastModifiedBy`, which Word writes on every save; a blank name leaves it as it is.
pub fn set_last_modified_by(core: &mut XmlDoc, name: &str) {
    if !name.trim().is_empty() {
        set_prop(&mut core.root, NS_CP, "cp", "lastModifiedBy", name, false);
    }
}

/// Whether `s` is a UTC time in the form `2026-10-13T09:30:00Z` (what Word writes).
pub fn is_w3cdtf(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && b.iter().enumerate().all(|(i, c)| matches!(i, 4 | 7 | 10 | 13 | 16 | 19) || c.is_ascii_digit())
}

/// The time `secs` seconds after 1970-01-01 as `2026-10-13T09:30:00Z`.
pub fn iso_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// The current time as `2026-10-13T09:30:00Z`. Not available in the browser build, which has no clock of its
/// own: the page passes the time in.
#[cfg(not(target_arch = "wasm32"))]
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    iso_from_unix(secs)
}

/// The prefix a document binds to a namespace, declaring the usual one when it is not bound yet.
fn prefix_for(root: &mut Element, uri: &str, usual: &str) -> String {
    if let Some((k, _)) = root.attrs.iter().find(|(k, v)| k.starts_with("xmlns:") && v == uri) {
        return k["xmlns:".len()..].to_string();
    }
    root.set_attr(format!("xmlns:{usual}"), uri);
    usual.to_string()
}

fn set_prop(root: &mut Element, uri: &str, usual: &str, local: &str, value: &str, exact: bool) {
    let prefix = prefix_for(root, uri, usual);
    let name = format!("{prefix}:{local}");
    // A property set from the details box is tidied; one a content control is bound to holds exactly what is typed in
    // the control, spaces included, so the control and the file agree (a title of one space is a title).
    let value = if exact { value } else { value.trim() };
    let at = root.children.iter().position(|n| matches!(n, Node::Element(e) if e.local() == local && e.name == name));
    match (value.is_empty(), at) {
        (true, Some(i)) => {
            root.children.remove(i);
        }
        (true, None) => {}
        (false, Some(i)) => {
            if let Node::Element(e) = &mut root.children[i] {
                e.children = vec![Node::Text(value.to_string())];
            }
        }
        (false, None) => root.children.push(Node::Element(Element::new(name).with_text(value))),
    }
}

/// The core properties a `set_doc_props` can change; `None` leaves a property alone and an empty string removes it.
#[derive(Debug, Clone, Default)]
pub struct CoreChanges<'a> {
    pub title: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub author: Option<&'a str>,
    pub keywords: Option<&'a str>,
    pub description: Option<&'a str>,
    pub category: Option<&'a str>,
    pub status: Option<&'a str>,
    /// Keep the values as given (no trimming): a content control bound to the property shows them as they are.
    pub exact: bool,
}

/// Apply a set of property changes to a core part.
pub fn apply(core: &mut XmlDoc, c: &CoreChanges) {
    let root = &mut core.root;
    for (v, uri, usual, local) in [
        (c.title, NS_DC, "dc", "title"),
        (c.subject, NS_DC, "dc", "subject"),
        (c.author, NS_DC, "dc", "creator"),
        (c.keywords, NS_CP, "cp", "keywords"),
        (c.description, NS_DC, "dc", "description"),
        (c.category, NS_CP, "cp", "category"),
        (c.status, NS_CP, "cp", "contentStatus"),
    ] {
        if let Some(v) = v {
            set_prop(root, uri, usual, local, v, c.exact);
        }
    }
}

// ---------------------------------------------------------------------------
// Language
// ---------------------------------------------------------------------------

/// Check and tidy a BCP 47 language tag (`en-us` becomes `en-US`).
pub fn normalize_lang(input: &str) -> Result<String, String> {
    let bad = || format!("'{}' is not a language tag; use one like en-US, es-ES or fr-FR", input.trim());
    let t = input.trim().replace('_', "-");
    let parts: Vec<&str> = t.split('-').collect();
    let ok_part = |p: &str, min: usize, max: usize| (min..=max).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric());
    if t.is_empty() || !parts[0].chars().all(|c| c.is_ascii_alphabetic()) || !ok_part(parts[0], 2, 3) || !parts[1..].iter().all(|p| ok_part(p, 1, 8)) {
        return Err(bad());
    }
    let mut out = vec![parts[0].to_ascii_lowercase()];
    for p in &parts[1..] {
        out.push(if p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()) {
            p.to_ascii_uppercase()
        } else if p.len() == 4 && p.chars().all(|c| c.is_ascii_alphabetic()) {
            let mut c = p.to_ascii_lowercase();
            c[..1].make_ascii_uppercase();
            c
        } else {
            p.to_string()
        });
    }
    Ok(out.join("-"))
}

/// Set `w:lang w:val` in a style defaults' run properties, creating `docDefaults/rPrDefault/rPr` as needed.
/// Other attributes of an existing `w:lang` (`w:eastAsia`, `w:bidi`) are kept.
pub fn set_default_lang(styles: &mut XmlDoc, lang: &str) {
    let root = &mut styles.root;
    if root.child("w:docDefaults").is_none() {
        // `w:docDefaults` is the first child of `w:styles`.
        let first = root.children.iter().position(|n| matches!(n, Node::Element(_))).unwrap_or(root.children.len());
        root.children.insert(first, Node::Element(Element::new("w:docDefaults")));
    }
    let dd = root.child_mut("w:docDefaults").expect("just created");
    if dd.child("w:rPrDefault").is_none() {
        put_ordered(dd, Element::new("w:rPrDefault"), &["w:rPrDefault", "w:pPrDefault"]);
    }
    let def = dd.child_mut("w:rPrDefault").expect("just created");
    if def.child("w:rPr").is_none() {
        def.children.push(Node::Element(Element::new("w:rPr")));
    }
    let rpr = def.child_mut("w:rPr").expect("just created");
    let mut el = rpr.child("w:lang").cloned().unwrap_or_else(|| Element::new("w:lang"));
    el.set_attr("w:val", lang);
    put_rpr(rpr, el);
}

/// Set `w:themeFontLang w:val` if the settings have one (they do not need one otherwise).
pub fn set_theme_font_lang(settings: &mut Element, lang: &str) {
    if let Some(t) = settings.child_mut("w:themeFontLang") {
        t.set_attr("w:val", lang);
    }
}

/// Set the language on the text of some inline content.
pub fn set_run_lang(inlines: &mut [Inline], lang: &str) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                if r.items.iter().any(|it| matches!(it, RunItem::Text(_))) {
                    let rpr = r.rpr.get_or_insert_with(|| Element::new("w:rPr"));
                    let mut el = rpr.child("w:lang").cloned().unwrap_or_else(|| Element::new("w:lang"));
                    el.set_attr("w:val", lang);
                    put_rpr(rpr, el);
                }
            }
            Inline::Container(c) => set_run_lang(&mut c.children, lang),
            Inline::Opaque(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_written_as_word_does() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_from_unix(1_760_347_800), "2025-10-13T09:30:00Z");
        assert_eq!(iso_from_unix(-1), "1969-12-31T23:59:59Z");
        assert!(is_w3cdtf("2026-10-13T09:30:00Z") && !is_w3cdtf("2026-10-13") && !is_w3cdtf("2026-10-13T09:30:00.5Z"));
    }

    #[test]
    fn stamping_sets_modified_and_created_once() {
        let mut core = new_core_doc();
        stamp(&mut core, "2026-10-13T09:30:00Z", true);
        let xml = String::from_utf8(core.to_bytes()).unwrap();
        assert!(xml.contains(r#"<dcterms:created xsi:type="dcterms:W3CDTF">2026-10-13T09:30:00Z</dcterms:created>"#), "{xml}");
        assert!(xml.contains(r#"<dcterms:modified xsi:type="dcterms:W3CDTF">2026-10-13T09:30:00Z</dcterms:modified>"#), "{xml}");
        stamp(&mut core, "2026-10-14T10:00:00Z", true);
        let props = read(Some(&core));
        assert_eq!((props.created.as_deref(), props.modified.as_deref()), (Some("2026-10-13T09:30:00Z"), Some("2026-10-14T10:00:00Z")));
    }

    #[test]
    fn language_tags() {
        assert_eq!(normalize_lang("en-us").unwrap(), "en-US");
        assert_eq!(normalize_lang(" es_ES ").unwrap(), "es-ES");
        assert_eq!(normalize_lang("zh-hant-TW").unwrap(), "zh-Hant-TW");
        assert_eq!(normalize_lang("fil").unwrap(), "fil");
        for bad in ["", "english", "e", "en-", "en--US", "12-US", "en-US-toolongsubtag1", "en US"] {
            assert!(normalize_lang(bad).is_err(), "{bad}");
        }
    }
}
