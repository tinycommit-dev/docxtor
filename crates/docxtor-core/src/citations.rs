//! Citations and bibliography sources, kept the way Word keeps them so Word can keep editing them
//! (References > Manage Sources, Insert Citation, Bibliography).
//!
//! The sources live in a custom XML part (`customXml/item1.xml`, root `b:Sources`), described by a
//! properties part and reached through a `customXml` relationship of the main document. A citation is an
//! inline content control (`w:sdt` with `w:citation`) around a `CITATION` field; the bibliography is a block
//! content control (`docPartGallery` "Bibliographies") around a `BIBLIOGRAPHY` field. This module holds the
//! data types, the mapping to and from Word's `b:Source` markup, the field instruction parser and the
//! sources part. `cite_styles` formats; `document/cite.rs` applies it all to a document.

use crate::cite_styles::Style;
use crate::package::{self, Package};
use crate::xml::{self, Element, Node, XmlDoc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const NS_BIB: &str = "http://schemas.openxmlformats.org/officeDocument/2006/bibliography";
pub const NS_DS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/customXml";
pub const REL_CUSTOM_XML: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
pub const REL_CUSTOM_XML_PROPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
pub const CT_CUSTOM_XML_PROPS: &str = "application/vnd.openxmlformats-officedocument.customXmlProperties+xml";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// What a source is. Word's richer list (film, patent, interview, ...) reads as `misc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Book,
    #[serde(alias = "journal_article", alias = "journal")]
    Article,
    #[serde(alias = "book_section", alias = "incollection")]
    Chapter,
    #[serde(alias = "proceedings", alias = "inproceedings")]
    Conference,
    Report,
    #[serde(alias = "webpage", alias = "web", alias = "online")]
    Website,
    #[serde(alias = "dissertation")]
    Thesis,
    #[default]
    Misc,
}

impl SourceType {
    pub fn id(self) -> &'static str {
        match self {
            SourceType::Book => "book",
            SourceType::Article => "article",
            SourceType::Chapter => "chapter",
            SourceType::Conference => "conference",
            SourceType::Report => "report",
            SourceType::Website => "website",
            SourceType::Thesis => "thesis",
            SourceType::Misc => "misc",
        }
    }
}

/// An author or editor: a person (`family`, `given`) or an organisation (`literal`).
#[derive(Debug, Clone, PartialEq)]
pub enum Name {
    Person { family: String, given: Option<String> },
    Literal(String),
}

impl Name {
    pub fn person(family: &str, given: &str) -> Name {
        let given = given.trim();
        Name::Person { family: family.trim().to_string(), given: if given.is_empty() { None } else { Some(given.to_string()) } }
    }

    /// The surname, or the whole name of an organisation.
    pub fn family(&self) -> &str {
        match self {
            Name::Person { family, .. } => family,
            Name::Literal(l) => l,
        }
    }

    pub fn given(&self) -> Option<&str> {
        match self {
            Name::Person { given, .. } => given.as_deref().filter(|g| !g.trim().is_empty()),
            Name::Literal(_) => None,
        }
    }

    /// Parse a name written as people write it: "Smith, John", "John Q. Smith", "Ludwig van Beethoven".
    pub fn parse(s: &str) -> Option<Name> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        // An organisation is written in braces: {World Health Organization}.
        if let Some(org) = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
            let org = org.trim();
            return (!org.is_empty()).then(|| Name::Literal(org.to_string()));
        }
        if let Some((family, given)) = s.split_once(',') {
            // "Smith, Jr., John" keeps the suffix with the surname.
            let (family, given) = match given.trim().split_once(',') {
                Some((suffix, rest)) if matches!(suffix.trim().trim_end_matches('.').to_lowercase().as_str(), "jr" | "sr" | "ii" | "iii" | "iv") => {
                    (format!("{}, {}", family.trim(), suffix.trim()), rest.trim().to_string())
                }
                _ => (family.trim().to_string(), given.trim().to_string()),
            };
            return Some(Name::person(&family, &given));
        }
        let words: Vec<&str> = s.split_whitespace().collect();
        if words.len() == 1 {
            return Some(Name::person(words[0], ""));
        }
        // The surname is the last word plus any lowercase particles before it (van der Berg).
        let mut start = words.len() - 1;
        while start > 1 && is_particle(words[start - 1]) {
            start -= 1;
        }
        Some(Name::person(&words[start..].join(" "), &words[..start].join(" ")))
    }
}

fn is_particle(w: &str) -> bool {
    matches!(w, "van" | "von" | "de" | "der" | "den" | "ten" | "ter" | "da" | "di" | "del" | "della" | "du" | "la" | "le" | "bin" | "ibn" | "al" | "el" | "dos" | "das" | "y")
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Name::Person { family, given } => {
                let mut m = s.serialize_map(Some(1 + usize::from(given.is_some())))?;
                m.serialize_entry("family", family)?;
                if let Some(g) = given {
                    m.serialize_entry("given", g)?;
                }
                m.end()
            }
            Name::Literal(l) => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("literal", l)?;
                m.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) => Name::parse(s).ok_or_else(|| D::Error::custom("an author's name can't be empty")),
            serde_json::Value::Object(o) => {
                let text = |k: &str| o.get(k).and_then(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
                if let Some(f) = text("family") {
                    Ok(Name::Person { family: f, given: text("given") })
                } else if let Some(l) = text("literal").or_else(|| text("name")) {
                    Ok(Name::Literal(l))
                } else {
                    Err(D::Error::custom("an author needs a family name or a literal name"))
                }
            }
            _ => Err(D::Error::custom("an author is {family, given?}, {literal} or a string")),
        }
    }
}

fn flex<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    use serde::de::Error;
    let v = Option::<serde_json::Value>::deserialize(d)?;
    match v {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(_) => Err(D::Error::custom("expected text or a number")),
    }
}

/// One entry of the document's source list (Word's Manage Sources, "Current List").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// The short key citations use, e.g. `Smi20`. Generated when empty.
    #[serde(default)]
    pub tag: String,
    #[serde(default, rename = "type")]
    pub kind: SourceType,
    #[serde(default)]
    pub authors: Vec<Name>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub editors: Vec<Name>,
    #[serde(default)]
    pub title: String,
    /// Journal, book, proceedings or website the source sits in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_title: Option<String>,
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    /// A month number (`3`) or name (`March`).
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub month: Option<String>,
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub day: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub pages: Option<String>,
    #[serde(default, deserialize_with = "flex", skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    /// When a web source was read, `YYYY-MM-DD` (month and day optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accessed: Option<String>,
    /// What kind of report or thesis: "Doctoral dissertation", "Master's thesis", "Technical report".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
}

impl Source {
    /// A placeholder: a source that has a tag and nothing else yet (Word's "Add new placeholder"). Citations to it
    /// show the tag in brackets and the bibliography leaves it out until it is filled in.
    pub fn is_placeholder(&self) -> bool {
        let mut c = self.clone();
        if c.title == c.tag {
            c.title.clear();
        }
        c.tag.clear();
        c.guid = None;
        c == Source::default()
    }
}

/// What a citation of placeholder sources shows: `(Placeholder1; Placeholder2)`.
pub fn placeholder_text(tags: &[String]) -> String {
    format!("({})", tags.join("; "))
}

/// Where a citation sits in the text and what it cites (`Document::citations`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationInfo {
    /// The paragraph holding the citation and the offset where it starts.
    pub p: u32,
    pub o: usize,
    /// How many characters its text takes.
    pub len: usize,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suppress_author: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suppress_year: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suppress_title: bool,
}

/// A trimmed, non-empty value.
pub fn nz(o: &Option<String>) -> Option<&str> {
    o.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

pub fn month_number(m: &str) -> Option<u32> {
    let t = m.trim().trim_end_matches('.').to_lowercase();
    if let Ok(n) = t.parse::<u32>() {
        return (1..=12).contains(&n).then_some(n);
    }
    const NAMES: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
    if t.len() < 3 {
        return None;
    }
    NAMES.iter().position(|n| n.starts_with(&t) || (t == "sept" && *n == "september")).map(|i| i as u32 + 1)
}

pub const MONTH_NAMES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Generate a tag the way Word does: the first three letters of the first author's surname and the last two
/// digits of the year (`Smi20`). `taken` tells which tags exist; a clash gets `a`, `b`, ...
pub fn make_tag(s: &Source, taken: &dyn Fn(&str) -> bool) -> String {
    // Letters as typed (accents removed), the way Word takes the first three of a surname.
    let fold = |t: &str| -> String {
        t.chars()
            .filter_map(|c| crate::cite_styles::fold_char(c).filter(|f| f.is_ascii_alphabetic()).map(|f| if c.is_uppercase() { f.to_ascii_uppercase() } else { f }))
            .collect()
    };
    let mut base = s.authors.first().map(|a| fold(a.family())).unwrap_or_default();
    if base.is_empty() {
        base = s.editors.first().map(|a| fold(a.family())).unwrap_or_default();
    }
    if base.is_empty() {
        base = fold(&s.title);
    }
    let mut base: String = base.chars().take(3).collect();
    if base.is_empty() {
        base = "src".into();
    }
    base[..1].make_ascii_uppercase();
    let yy: String = nz(&s.year).map(|y| y.chars().filter(|c| c.is_ascii_digit()).collect::<String>()).filter(|y| y.len() >= 2).map(|y| y[y.len() - 2..].to_string()).unwrap_or_default();
    let root = format!("{base}{yy}");
    if !taken(&root) {
        return root;
    }
    for c in 'a'..='z' {
        let t = format!("{root}{c}");
        if !taken(&t) {
            return t;
        }
    }
    let mut n = 2;
    loop {
        let t = format!("{root}{n}");
        if !taken(&t) {
            return t;
        }
        n += 1;
    }
}

/// A tag must be usable inside a field instruction.
pub fn valid_tag(tag: &str) -> bool {
    !tag.is_empty() && tag.len() <= 64 && tag.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
}

/// A GUID-shaped id from some text, so new sources get stable ids without a random source.
pub fn guid_from(seed: &str, salt: u32) -> String {
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325 ^ salt as u64;
    let mut h2: u64 = 0x8422_2325_cbf2_9ce4 ^ ((salt as u64) << 17);
    for b in seed.bytes() {
        h1 = (h1 ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        h2 = (h2.rotate_left(5) ^ b as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    let a = (h1 >> 32) as u32;
    let b = (h1 >> 16) as u16;
    let c = ((h1 as u16) & 0x0fff) | 0x4000;
    let d = ((h2 >> 48) as u16 & 0x3fff) | 0x8000;
    let e = h2 & 0xffff_ffff_ffff;
    format!("{{{a:08X}-{b:04X}-{c:04X}-{d:04X}-{e:012X}}}")
}

// ---------------------------------------------------------------------------
// Word's b:Source markup
// ---------------------------------------------------------------------------

/// Children of `b:Source` that Docxtor reads and writes; anything else on a source is kept as it is.
const KNOWN: &[&str] = &[
    "Tag", "SourceType", "Guid", "Author", "Title", "Year", "Month", "Day", "Publisher", "Institution", "City", "JournalName",
    "PeriodicalTitle", "BookTitle", "ConferenceName", "InternetSiteTitle", "PublicationTitle", "Volume", "Issue", "Pages", "Edition",
    "URL", "DOI", "YearAccessed", "MonthAccessed", "DayAccessed", "ThesisType", "Type",
];

fn kind_from_word(word: &str, has_thesis_type: bool) -> SourceType {
    match word.trim() {
        "Book" => SourceType::Book,
        "JournalArticle" | "ArticleInAPeriodical" => SourceType::Article,
        "BookSection" => SourceType::Chapter,
        "ConferenceProceedings" => SourceType::Conference,
        "Report" if has_thesis_type => SourceType::Thesis,
        "Report" => SourceType::Report,
        "InternetSite" | "DocumentFromInternetSite" => SourceType::Website,
        _ => SourceType::Misc,
    }
}

fn kind_to_word(k: SourceType, has_container: bool) -> &'static str {
    match k {
        SourceType::Book => "Book",
        SourceType::Article => "JournalArticle",
        SourceType::Chapter => "BookSection",
        SourceType::Conference => "ConferenceProceedings",
        SourceType::Report | SourceType::Thesis => "Report",
        SourceType::Website if has_container => "DocumentFromInternetSite",
        SourceType::Website => "InternetSite",
        SourceType::Misc => "Misc",
    }
}

/// Names of the children that hold the container title for a type (the first is the one written to a new source).
fn container_names(k: SourceType) -> &'static [&'static str] {
    match k {
        SourceType::Article => &["JournalName", "PeriodicalTitle"],
        SourceType::Chapter => &["BookTitle"],
        SourceType::Conference => &["ConferenceName", "BookTitle"],
        SourceType::Website => &["InternetSiteTitle"],
        SourceType::Misc => &["PublicationTitle", "PeriodicalTitle"],
        SourceType::Book | SourceType::Report | SourceType::Thesis => &[],
    }
}

fn kid<'a>(e: &'a Element, local: &str) -> Option<&'a Element> {
    e.elements().find(|c| c.local() == local)
}

fn kid_text(e: &Element, local: &str) -> Option<String> {
    kid(e, local).map(|c| c.text().trim().to_string()).filter(|t| !t.is_empty())
}

fn read_names(list: &Element) -> Vec<Name> {
    let mut out = Vec::new();
    for c in list.elements() {
        match c.local() {
            "NameList" => {
                for p in c.elements().filter(|p| p.local() == "Person") {
                    let last = kid_text(p, "Last").unwrap_or_default();
                    let given = [kid_text(p, "First"), kid_text(p, "Middle")].into_iter().flatten().collect::<Vec<_>>().join(" ");
                    if !last.is_empty() || !given.is_empty() {
                        // A person with only a first name keeps it as the name.
                        if last.is_empty() {
                            out.push(Name::Literal(given));
                        } else {
                            out.push(Name::person(&last, &given));
                        }
                    }
                }
            }
            "Corporate" => {
                let t = c.text().trim().to_string();
                if !t.is_empty() {
                    out.push(Name::Literal(t));
                }
            }
            _ => {}
        }
    }
    out
}

/// Read one `b:Source` element.
pub fn source_from_xml(el: &Element) -> Source {
    let mut s = Source { tag: kid_text(el, "Tag").unwrap_or_default(), ..Default::default() };
    s.kind = kind_from_word(&kid_text(el, "SourceType").unwrap_or_default(), kid(el, "ThesisType").is_some());
    s.guid = kid_text(el, "Guid");
    if let Some(a) = kid(el, "Author") {
        if let Some(main) = kid(a, "Author") {
            s.authors = read_names(main);
        }
        if let Some(ed) = kid(a, "Editor") {
            s.editors = read_names(ed);
        }
    }
    s.title = kid_text(el, "Title").unwrap_or_default();
    s.year = kid_text(el, "Year");
    s.month = kid_text(el, "Month");
    s.day = kid_text(el, "Day");
    s.publisher = kid_text(el, "Publisher").or_else(|| kid_text(el, "Institution"));
    s.place = kid_text(el, "City");
    s.volume = kid_text(el, "Volume");
    s.issue = kid_text(el, "Issue");
    s.pages = kid_text(el, "Pages");
    s.edition = kid_text(el, "Edition");
    s.url = kid_text(el, "URL");
    s.doi = kid_text(el, "DOI");
    s.genre = if s.kind == SourceType::Thesis { kid_text(el, "ThesisType") } else { kid_text(el, "Type") };
    s.container_title = container_names(s.kind).iter().find_map(|n| kid_text(el, n));
    if s.container_title.is_none() && s.kind == SourceType::Conference {
        s.container_title = kid_text(el, "PublicationTitle");
    }
    let (y, m, d) = (kid_text(el, "YearAccessed"), kid_text(el, "MonthAccessed"), kid_text(el, "DayAccessed"));
    if let Some(y) = y {
        let mut acc = y;
        if let Some(mn) = m.as_deref().and_then(month_number) {
            acc.push_str(&format!("-{mn:02}"));
            if let Some(dn) = d.as_deref().and_then(|d| d.trim().parse::<u32>().ok()) {
                acc.push_str(&format!("-{dn:02}"));
            }
        }
        s.accessed = Some(acc);
    }
    s
}

/// The prefix of the sources part's elements (`b:`), or none when it uses a default namespace.
fn prefix_of(root: &Element) -> String {
    match root.name.split_once(':') {
        Some((p, _)) => format!("{p}:"),
        None => String::new(),
    }
}

fn mk(prefix: &str, local: &str) -> Element {
    Element::new(format!("{prefix}{local}"))
}

fn text_el(prefix: &str, local: &str, text: &str) -> Element {
    mk(prefix, local).with_text(text)
}

/// Set (or remove, for `None`) the first child with this local name, creating it when missing.
fn set_text(el: &mut Element, prefix: &str, local: &str, value: Option<&str>) {
    let at = el.children.iter().position(|n| matches!(n, Node::Element(c) if c.local() == local));
    match (value.map(str::trim).filter(|v| !v.is_empty()), at) {
        (Some(v), Some(i)) => {
            if let Node::Element(c) = &mut el.children[i] {
                c.children = vec![Node::Text(v.to_string())];
            }
        }
        (Some(v), None) => el.children.push(Node::Element(text_el(prefix, local, v))),
        (None, Some(i)) => {
            el.children.remove(i);
        }
        (None, None) => {}
    }
}

fn names_el(prefix: &str, wrapper: &str, names: &[Name]) -> Element {
    let mut w = mk(prefix, wrapper);
    let corporate: Vec<&Name> = names.iter().filter(|n| matches!(n, Name::Literal(_))).collect();
    // Word keeps people in a NameList and an organisation as `b:Corporate`; a mixed list becomes people only.
    if corporate.len() == names.len() && corporate.len() == 1 {
        return w.with_child(text_el(prefix, "Corporate", corporate[0].family()));
    }
    let mut list = mk(prefix, "NameList");
    for n in names {
        let mut p = mk(prefix, "Person");
        match n {
            Name::Person { family, given } => {
                p.children.push(Node::Element(text_el(prefix, "Last", family)));
                let g = given.as_deref().unwrap_or("").trim();
                if !g.is_empty() {
                    let (first, middle) = match g.split_once(char::is_whitespace) {
                        Some((f, m)) => (f, m.trim()),
                        None => (g, ""),
                    };
                    p.children.push(Node::Element(text_el(prefix, "First", first)));
                    if !middle.is_empty() {
                        p.children.push(Node::Element(text_el(prefix, "Middle", middle)));
                    }
                }
            }
            Name::Literal(l) => p.children.push(Node::Element(text_el(prefix, "Last", l))),
        }
        list.children.push(Node::Element(p));
    }
    w.children.push(Node::Element(list));
    w
}

fn set_names(el: &mut Element, prefix: &str, authors: &[Name], editors: &[Name]) {
    if kid(el, "Author").is_none() {
        if authors.is_empty() && editors.is_empty() {
            return;
        }
        el.children.push(Node::Element(mk(prefix, "Author")));
    }
    let wrapper = el.elements_mut().find(|c| c.local() == "Author").expect("just ensured");
    for (local, names) in [("Author", authors), ("Editor", editors)] {
        wrapper.children.retain(|n| !matches!(n, Node::Element(c) if c.local() == local));
        if !names.is_empty() {
            let pos = if local == "Author" { 0 } else { wrapper.children.len() };
            wrapper.children.insert(pos.min(wrapper.children.len()), Node::Element(names_el(prefix, local, names)));
        }
    }
    if wrapper.elements().next().is_none() {
        el.children.retain(|n| !matches!(n, Node::Element(c) if c.local() == "Author"));
    }
}

fn month_text(m: &Option<String>) -> Option<String> {
    nz(m).map(|m| month_number(m).map(|n| MONTH_NAMES[n as usize - 1].to_string()).unwrap_or_else(|| m.to_string()))
}

/// Write a source into a `b:Source` element: an existing one keeps the children Docxtor doesn't model.
pub fn write_source(el: &mut Element, s: &Source, prefix: &str) {
    let word_kind = {
        // Keep the exact Word type when it still means the same thing (ArticleInAPeriodical, DocumentFromInternetSite).
        let current = kid_text(el, "SourceType").unwrap_or_default();
        let has_container = nz(&s.container_title).is_some();
        if !current.is_empty() && kind_from_word(&current, s.kind == SourceType::Thesis) == s.kind && (s.kind != SourceType::Website || (current == "DocumentFromInternetSite") == has_container) {
            current
        } else {
            kind_to_word(s.kind, has_container).to_string()
        }
    };
    set_text(el, prefix, "Tag", Some(&s.tag));
    set_text(el, prefix, "SourceType", Some(&word_kind));
    set_text(el, prefix, "Guid", s.guid.as_deref());
    set_names(el, prefix, &s.authors, &s.editors);
    set_text(el, prefix, "Title", Some(&s.title).filter(|t| !t.trim().is_empty()).map(String::as_str));
    set_text(el, prefix, "Year", nz(&s.year));
    set_text(el, prefix, "Month", month_text(&s.month).as_deref());
    set_text(el, prefix, "Day", nz(&s.day));
    set_text(el, prefix, "City", nz(&s.place));
    // The publisher goes where the source already keeps it; a thesis names its institution.
    let pub_name = if kid(el, "Publisher").is_none() && (kid(el, "Institution").is_some() || s.kind == SourceType::Thesis) { "Institution" } else { "Publisher" };
    let other = if pub_name == "Publisher" { "Institution" } else { "Publisher" };
    set_text(el, prefix, other, None);
    set_text(el, prefix, pub_name, nz(&s.publisher));
    set_text(el, prefix, "Volume", nz(&s.volume));
    set_text(el, prefix, "Issue", nz(&s.issue));
    set_text(el, prefix, "Pages", nz(&s.pages));
    set_text(el, prefix, "Edition", nz(&s.edition));
    set_text(el, prefix, "URL", nz(&s.url));
    set_text(el, prefix, "DOI", nz(&s.doi));
    // A thesis is a Word report with a thesis type; the genre of other sources goes in `b:Type`.
    let thesis_type = if s.kind == SourceType::Thesis { Some(nz(&s.genre).unwrap_or("Thesis")) } else { None };
    set_text(el, prefix, "ThesisType", thesis_type);
    set_text(el, prefix, "Type", if s.kind == SourceType::Thesis { None } else { nz(&s.genre) });
    // The container title goes into the child that fits the type; others of the family are cleared.
    let names = container_names(s.kind);
    let all = ["JournalName", "PeriodicalTitle", "BookTitle", "ConferenceName", "InternetSiteTitle", "PublicationTitle"];
    let existing = names.iter().find(|n| kid(el, n).is_some()).copied().or(names.first().copied());
    for n in all {
        if Some(n) != existing {
            set_text(el, prefix, n, None);
        }
    }
    if let Some(n) = existing {
        set_text(el, prefix, n, nz(&s.container_title));
    }
    let (y, m, d) = match nz(&s.accessed) {
        Some(a) => {
            let mut it = a.split('-');
            (it.next().map(str::to_string), it.next().and_then(month_number).map(|n| MONTH_NAMES[n as usize - 1].to_string()), it.next().and_then(|d| d.parse::<u32>().ok()).map(|d| d.to_string()))
        }
        None => (None, None, None),
    };
    set_text(el, prefix, "YearAccessed", y.as_deref());
    set_text(el, prefix, "MonthAccessed", m.as_deref());
    set_text(el, prefix, "DayAccessed", d.as_deref());
}

/// A new `b:Source` element.
pub fn source_to_xml(s: &Source, prefix: &str, ref_order: usize) -> Element {
    let mut el = mk(prefix, "Source");
    write_source(&mut el, s, prefix);
    set_text(&mut el, prefix, "RefOrder", Some(&ref_order.to_string()));
    el
}

/// Whether a `b:Source` child is one Docxtor models (used by tests and when merging).
pub fn is_known_child(local: &str) -> bool {
    KNOWN.contains(&local)
}

// ---------------------------------------------------------------------------
// The sources part
// ---------------------------------------------------------------------------

/// `StyleName` of a document whose citations use a CSL style: this, then the style's id.
pub const CSL_PREFIX: &str = "CSL:";

/// The citation style of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleId {
    /// One of Docxtor's own five, formatted by Docxtor's own code.
    Builtin(Style),
    /// A CSL style, by the id it was registered with.
    Csl(String),
}

impl StyleId {
    pub fn id(&self) -> String {
        match self {
            StyleId::Builtin(s) => s.id().to_string(),
            StyleId::Csl(id) => id.clone(),
        }
    }
}

/// The document's list of sources: the parsed `b:Sources` part and where it lives.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CiteState {
    /// Part name of the sources part once it exists in the package.
    pub path: Option<String>,
    pub xml: Option<XmlDoc>,
    pub dirty: bool,
}

fn is_sources_root(root: &Element) -> bool {
    root.local() == "Sources" && root.attrs.iter().any(|(k, v)| (k == "xmlns" || k.starts_with("xmlns:")) && v == NS_BIB)
}

impl CiteState {
    /// Find the bibliography part by what it contains (root `Sources` in the bibliography namespace), not by its name.
    pub fn load(pkg: &Package, main_path: &str) -> CiteState {
        let mut candidates: Vec<String> = pkg
            .rels_for(main_path)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.rel_type == REL_CUSTOM_XML && !r.external)
            .map(|r| package::resolve_target(main_path, &r.target))
            .collect();
        for p in &pkg.parts {
            if p.name.starts_with("customXml/") && p.name.ends_with(".xml") && !p.name.contains("/_rels/") && !candidates.contains(&p.name) {
                candidates.push(p.name.clone());
            }
        }
        for name in candidates {
            if let Some(data) = pkg.get(&name) {
                // Skip big unrelated custom XML without parsing it (UTF-16 parts can't be searched this way).
                let utf16 = data.starts_with(&[0xFF, 0xFE]) || data.starts_with(&[0xFE, 0xFF]);
                if !utf16 && !data.windows(NS_BIB.len()).any(|w| w == NS_BIB.as_bytes()) {
                    continue;
                }
                if let Ok(doc) = xml::parse(data) {
                    if is_sources_root(&doc.root) {
                        return CiteState { path: Some(name), xml: Some(doc), dirty: false };
                    }
                }
            }
        }
        CiteState::default()
    }

    pub fn sources(&self) -> Vec<Source> {
        match &self.xml {
            Some(d) => d.root.elements().filter(|e| e.local() == "Source").map(source_from_xml).collect(),
            None => Vec::new(),
        }
    }

    pub fn style(&self) -> Style {
        match &self.xml {
            Some(d) => {
                // A CSL style names itself in StyleName; the Word style beside it is the closest one.
                let name = d.root.attr("StyleName").unwrap_or("");
                Style::from_word(d.root.attr("SelectedStyle").unwrap_or(""), if name.starts_with(CSL_PREFIX) { "" } else { name })
            }
            None => Style::Apa,
        }
    }

    fn fresh_doc(style: Style) -> XmlDoc {
        let (sel, name) = style.word();
        XmlDoc::new(
            Element::new("b:Sources")
                .with_attr("SelectedStyle", sel)
                .with_attr("StyleName", name)
                .with_attr("Version", "6")
                .with_attr("xmlns:b", NS_BIB)
                .with_attr("xmlns", NS_BIB),
        )
    }

    fn prefix(&self) -> String {
        self.xml.as_ref().map(|d| prefix_of(&d.root)).unwrap_or_else(|| "b:".into())
    }

    pub fn has(&self, tag: &str) -> bool {
        self.sources().iter().any(|s| s.tag == tag)
    }

    /// Add a source (its tag must be set and free). The first source creates the part.
    pub fn add(&mut self, s: &Source) {
        let prefix = self.prefix();
        let doc = self.xml.get_or_insert_with(|| CiteState::fresh_doc(Style::Apa));
        let order = doc.root.elements().filter(|e| e.local() == "Source").count() + 1;
        doc.root.children.push(Node::Element(source_to_xml(s, &prefix, order)));
        self.dirty = true;
    }

    /// Replace the source with this tag (keeping the children Docxtor doesn't model).
    pub fn update(&mut self, tag: &str, s: &Source) -> bool {
        let prefix = self.prefix();
        let Some(doc) = self.xml.as_mut() else { return false };
        let Some(el) = doc.root.elements_mut().find(|e| e.local() == "Source" && kid_text(e, "Tag").as_deref() == Some(tag)) else { return false };
        write_source(el, s, &prefix);
        self.dirty = true;
        true
    }

    pub fn delete(&mut self, tag: &str) -> bool {
        let Some(doc) = self.xml.as_mut() else { return false };
        let at = doc.root.children.iter().position(|n| matches!(n, Node::Element(e) if e.local() == "Source" && kid_text(e, "Tag").as_deref() == Some(tag)));
        match at {
            Some(i) => {
                doc.root.children.remove(i);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn set_style(&mut self, style: Style) {
        let doc = self.xml.get_or_insert_with(|| CiteState::fresh_doc(style));
        let (sel, name) = style.word();
        doc.root.set_attr("SelectedStyle", sel);
        doc.root.set_attr("StyleName", name);
        self.dirty = true;
    }

    /// The style as the document names it: one of Docxtor's five, or a CSL style by id.
    pub fn style_id(&self) -> StyleId {
        let name = self.xml.as_ref().and_then(|d| d.root.attr("StyleName")).unwrap_or("");
        match name.strip_prefix(CSL_PREFIX).map(str::trim).filter(|id| !id.is_empty()) {
            Some(id) => StyleId::Csl(id.to_string()),
            None => StyleId::Builtin(self.style()),
        }
    }

    /// Choose a style. A CSL style is written the way Word can live with: `SelectedStyle` names the Word style
    /// closest to it (`near`), so Word updates the fields in something sensible, and `StyleName` carries the
    /// CSL style's id.
    pub fn set_style_id(&mut self, id: &StyleId, near: Style) {
        match id {
            StyleId::Builtin(s) => self.set_style(*s),
            StyleId::Csl(csl) => {
                self.set_style(near);
                if let Some(d) = self.xml.as_mut() {
                    d.root.set_attr("StyleName", format!("{CSL_PREFIX}{csl}"));
                }
            }
        }
    }

    /// Take the state of an earlier snapshot (undo/redo). The part's name stays: it may be in the package by now.
    pub fn restore(&mut self, snap: CiteState) {
        if snap.xml != self.xml {
            self.dirty = true;
        } else {
            self.dirty |= snap.dirty;
        }
        self.xml = snap.xml;
        if self.path.is_none() {
            self.path = snap.path;
        }
    }

    /// Write the part into the package, creating it (with its properties part, relationships and content
    /// type) the first time, the way Word lays it out.
    pub fn flush(&mut self, pkg: &mut Package, main_path: &str) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let doc = match &self.xml {
            Some(d) => d.clone(),
            None => match &self.path {
                // Undone back to a file without sources after a save wrote them: leave an empty list.
                Some(p) if pkg.has(p) => CiteState::fresh_doc(Style::Apa),
                _ => return,
            },
        };
        let path = match &self.path {
            Some(p) => p.clone(),
            None => {
                let mut n = 1;
                let path = loop {
                    let name = format!("customXml/item{n}.xml");
                    if !pkg.has(&name) && !pkg.has(&format!("customXml/itemProps{n}.xml")) {
                        break name;
                    }
                    n += 1;
                };
                let props = format!("customXml/itemProps{n}.xml");
                let item_id = guid_from(&format!("{main_path}{n}{}", String::from_utf8_lossy(&doc.to_bytes())), n as u32);
                let ds = XmlDoc::new(
                    Element::new("ds:datastoreItem").with_attr("ds:itemID", item_id).with_attr("xmlns:ds", NS_DS).with_child(
                        Element::new("ds:schemaRefs").with_child(Element::new("ds:schemaRef").with_attr("ds:uri", NS_BIB)),
                    ),
                );
                pkg.set(&props, ds.to_bytes());
                pkg.add_rel(&path, REL_CUSTOM_XML_PROPS, &format!("itemProps{n}.xml"), false);
                pkg.ensure_override(&props, CT_CUSTOM_XML_PROPS);
                // The sources part itself is plain XML: the package's default for `.xml` must exist.
                pkg.ensure_default("xml", "application/xml");
                pkg.add_rel(main_path, REL_CUSTOM_XML, &relative_to(main_path, &path), false);
                self.path = Some(path.clone());
                path
            }
        };
        pkg.set(&path, doc.to_bytes());
    }
}

/// A relationship target for `target` seen from `source` (`../customXml/item1.xml` from `word/document.xml`).
pub fn relative_to(source: &str, target: &str) -> String {
    let src_dir: Vec<&str> = source.split('/').collect();
    let src_dir = &src_dir[..src_dir.len().saturating_sub(1)];
    let tgt: Vec<&str> = target.split('/').collect();
    let mut common = 0;
    while common < src_dir.len() && common + 1 < tgt.len() && src_dir[common] == tgt[common] {
        common += 1;
    }
    let mut out = vec![".."; src_dir.len() - common];
    out.extend(&tgt[common..]);
    out.join("/")
}

// ---------------------------------------------------------------------------
// Field instructions
// ---------------------------------------------------------------------------

/// A `CITATION` field instruction: which sources, and how the citation reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CiteInstr {
    pub tags: Vec<String>,
    pub page: Option<String>,
    pub no_author: bool,
    pub no_year: bool,
    pub no_title: bool,
    pub volume: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub lcid: Option<String>,
    /// Switches Docxtor doesn't know, kept as written.
    pub extra: Vec<String>,
}

fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_q = !in_q;
                quoted = true;
            }
            c if c.is_whitespace() && !in_q => {
                if !cur.is_empty() || quoted {
                    out.push(std::mem::take(&mut cur));
                    quoted = false;
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || quoted {
        out.push(cur);
    }
    out
}

fn quote_if_needed(v: &str) -> String {
    if v.is_empty() || v.chars().any(|c| c.is_whitespace() || c == '"' || c == '\\') {
        format!("\"{}\"", v.replace('"', "'"))
    } else {
        v.to_string()
    }
}

impl CiteInstr {
    /// Parse ` CITATION Smi20 \m Lee21 \p 23 \l 1033 `; None when the instruction isn't a citation.
    pub fn parse(instr: &str) -> Option<CiteInstr> {
        let toks = tokens(instr);
        if !toks.first()?.eq_ignore_ascii_case("CITATION") {
            return None;
        }
        let mut c = CiteInstr::default();
        let mut i = 1;
        let value = |i: &mut usize| -> Option<String> {
            let v = toks.get(*i + 1).filter(|t| !t.starts_with('\\')).cloned();
            if v.is_some() {
                *i += 1;
            }
            v
        };
        while i < toks.len() {
            let t = &toks[i];
            match t.as_str() {
                "\\l" => c.lcid = value(&mut i),
                "\\m" => {
                    if let Some(v) = value(&mut i) {
                        c.tags.push(v);
                    }
                }
                "\\p" => c.page = value(&mut i),
                "\\n" => c.no_author = true,
                "\\y" => c.no_year = true,
                "\\t" => c.no_title = true,
                "\\v" => c.volume = value(&mut i),
                "\\f" => c.prefix = value(&mut i),
                "\\s" => c.suffix = value(&mut i),
                s if s.starts_with('\\') => {
                    c.extra.push(s.to_string());
                    while let Some(v) = toks.get(i + 1).filter(|t| !t.starts_with('\\')) {
                        c.extra.push(quote_if_needed(v));
                        i += 1;
                    }
                }
                s if i == 1 => c.tags.insert(0, s.to_string()),
                _ => {}
            }
            i += 1;
        }
        if c.tags.is_empty() {
            return None;
        }
        Some(c)
    }

    /// The instruction as Word writes it: ` CITATION Smi20 \m Lee21 \p 23 \l 1033 `.
    pub fn build(&self) -> String {
        let mut s = String::from(" CITATION");
        for (i, t) in self.tags.iter().enumerate() {
            s.push_str(if i == 0 { " " } else { " \\m " });
            s.push_str(t);
        }
        for (flag, on) in [("\\n", self.no_author), ("\\y", self.no_year), ("\\t", self.no_title)] {
            if on {
                s.push(' ');
                s.push_str(flag);
            }
        }
        for (flag, v) in [("\\p", &self.page), ("\\v", &self.volume), ("\\f", &self.prefix), ("\\s", &self.suffix)] {
            if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
                s.push_str(&format!(" {flag} {}", quote_if_needed(v)));
            }
        }
        for e in &self.extra {
            s.push(' ');
            s.push_str(e);
        }
        s.push_str(&format!(" \\l {} ", self.lcid.as_deref().unwrap_or("1033")));
        s
    }
}

/// Word's locale id for a language tag (for the `\l` switch of a citation).
pub fn lcid_for(lang: Option<&str>) -> &'static str {
    match lang.unwrap_or("en-US") {
        "en-GB" => "2057",
        "es-ES" | "es" => "3082",
        "fr-FR" | "fr" => "1036",
        "de-DE" | "de" => "1031",
        "it-IT" | "it" => "1040",
        "pt-BR" => "1046",
        "pt-PT" | "pt" => "2070",
        "nl-NL" | "nl" => "1043",
        _ => "1033",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_round_trip() {
        let c = CiteInstr::parse(" CITATION Smi20 \\m Lee21 \\p 23 \\n \\l 1033 ").unwrap();
        assert_eq!(c.tags, ["Smi20", "Lee21"]);
        assert_eq!(c.page.as_deref(), Some("23"));
        assert!(c.no_author && !c.no_year);
        assert_eq!(c.lcid.as_deref(), Some("1033"));
        assert_eq!(CiteInstr::parse(&c.build()).unwrap(), c);
        assert_eq!(c.build(), " CITATION Smi20 \\m Lee21 \\n \\p 23 \\l 1033 ");
        assert!(CiteInstr::parse(" BIBLIOGRAPHY ").is_none());
        let q = CiteInstr::parse(" CITATION Smi20 \\p \"23-25\" \\x foo \\l 2057 ").unwrap();
        assert_eq!(q.page.as_deref(), Some("23-25"));
        assert_eq!(q.extra, ["\\x", "foo"]);
    }

    #[test]
    fn names() {
        assert_eq!(Name::parse("Smith, John Q."), Some(Name::person("Smith", "John Q.")));
        assert_eq!(Name::parse("John Q. Smith"), Some(Name::person("Smith", "John Q.")));
        assert_eq!(Name::parse("Ludwig van Beethoven"), Some(Name::person("van Beethoven", "Ludwig")));
        assert_eq!(Name::parse("Plato"), Some(Name::person("Plato", "")));
        assert_eq!(Name::parse("Smith, Jr., John"), Some(Name::person("Smith, Jr.", "John")));
        assert_eq!(Name::parse("  "), None);
        assert_eq!(Name::parse("{World Health Organization}"), Some(Name::Literal("World Health Organization".into())));
    }

    #[test]
    fn tags() {
        let s = Source { authors: vec![Name::person("Smith", "John")], year: Some("2020".into()), ..Default::default() };
        assert_eq!(make_tag(&s, &|_| false), "Smi20");
        assert_eq!(make_tag(&s, &|t| t == "Smi20"), "Smi20a");
        assert_eq!(make_tag(&s, &|t| t == "Smi20" || t == "Smi20a"), "Smi20b");
        let m = Source { authors: vec![Name::person("Müller", "Jo")], year: Some("1999".into()), ..Default::default() };
        assert_eq!(make_tag(&m, &|_| false), "Mul99");
        let t = Source { title: "The rise".into(), ..Default::default() };
        assert_eq!(make_tag(&t, &|_| false), "The");
        assert_eq!(make_tag(&Source::default(), &|_| false), "Src");
    }

    #[test]
    fn relative_targets() {
        assert_eq!(relative_to("word/document.xml", "customXml/item1.xml"), "../customXml/item1.xml");
        assert_eq!(relative_to("word/document.xml", "word/media/a.png"), "media/a.png");
        assert_eq!(relative_to("document.xml", "customXml/item1.xml"), "customXml/item1.xml");
    }

    #[test]
    fn source_xml_round_trip_keeps_unknown_children() {
        let src = r#"<b:Source xmlns:b="x"><b:Tag>Smi20</b:Tag><b:SourceType>Book</b:SourceType><b:Guid>{G}</b:Guid><b:LCID>1033</b:LCID><b:Author><b:Author><b:NameList><b:Person><b:Last>Smith</b:Last><b:First>John</b:First><b:Middle>Q.</b:Middle></b:Person></b:NameList></b:Author><b:Translator><b:NameList><b:Person><b:Last>Tr</b:Last></b:Person></b:NameList></b:Translator></b:Author><b:Title>A book</b:Title><b:Year>2020</b:Year><b:Publisher>Pub</b:Publisher><b:RefOrder>1</b:RefOrder></b:Source>"#;
        let doc = xml::parse(src.as_bytes()).unwrap();
        let mut el = doc.root.clone();
        let mut s = source_from_xml(&el);
        assert_eq!(s.authors, vec![Name::person("Smith", "John Q.")]);
        assert_eq!(s.title, "A book");
        s.title = "A better book".into();
        s.authors.push(Name::Literal("The Group".into()));
        write_source(&mut el, &s, "b:");
        let out = el.to_xml();
        assert!(out.contains("<b:LCID>1033</b:LCID>"), "{out}");
        assert!(out.contains("<b:Translator>"), "{out}");
        assert!(out.contains("<b:Title>A better book</b:Title>"), "{out}");
        assert!(out.contains("<b:RefOrder>1</b:RefOrder>"), "{out}");
        let back = source_from_xml(&el);
        assert_eq!(back.authors.len(), 2);
    }
}
