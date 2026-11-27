//! Keeping a mail merge in the file.
//!
//! Word keeps the settings of a merge in `word/settings.xml` (`w:mailMerge`: the kind of document, the data source it is
//! linked to and how it reads it) and the recipients in the file the source points at, outside the .docx. Docxtor
//! writes the settings as Word does, and also keeps the recipients (with the ticks, the filter and the sort) in a
//! custom XML part of its own, so a document that is opened again still has its list. Word ignores the part.

use super::address::STANDARD_FIELDS;
use super::{norm, FilterRule, MailMerge, SortKey, SourceInfo, Table};
use crate::citations::{relative_to, CT_CUSTOM_XML_PROPS, REL_CUSTOM_XML, REL_CUSTOM_XML_PROPS};
use crate::package::{self, Package};
use crate::xml::{self, AttrValue, Element, Node, XmlDoc};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const NS_MM: &str = "https://docxtor.tinycommit.dev/ns/mail-merge/1";
const NS_DS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/customXml";
pub const REL_MAIL_MERGE_SOURCE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/mailMergeSource";

// ---------------------------------------------------------------------------
// Docxtor's part
// ---------------------------------------------------------------------------

fn el(name: &str) -> Element {
    Element::new(format!("mm:{name}"))
}

/// The part's XML for a merge.
pub fn to_xml(mm: &MailMerge) -> XmlDoc {
    let mut root = el("mailMerge").with_attr("xmlns:mm", NS_MM).with_attr("version", "1").with_attr("kind", mm.source.kind.clone()).with_attr("name", mm.source.name.clone());
    if let Some(t) = &mm.source.table {
        root.set_attr("table", t.clone());
    }
    if let Some(d) = mm.source.delimiter {
        root.set_attr("delimiter", d.to_string());
    }
    let mut cols = el("columns");
    for c in &mm.table.columns {
        cols.children.push(Node::Element(el("c").with_text(c.clone())));
    }
    root.children.push(Node::Element(cols));
    let mut recs = el("records");
    for (i, row) in mm.table.rows.iter().enumerate() {
        let mut r = el("r");
        if !mm.is_active(i) {
            r.set_attr("a", "0");
        }
        for v in row {
            let mut cell = el("v");
            if !v.is_empty() {
                cell.children.push(Node::Text(v.clone()));
            }
            r.children.push(Node::Element(cell));
        }
        recs.children.push(Node::Element(r));
    }
    root.children.push(Node::Element(recs));
    for f in &mm.filter {
        let mut e = el("filter").with_attr("field", f.field.clone()).with_attr("op", f.op.clone()).with_attr("value", f.value.clone());
        if let Some(j) = &f.join {
            e.set_attr("join", j.clone());
        }
        root.children.push(Node::Element(e));
    }
    for s in &mm.sort {
        root.children.push(Node::Element(el("sort").with_attr("field", s.field.clone()).with_attr("desc", if s.desc { "1" } else { "0" })));
    }
    for (k, v) in &mm.mapping {
        root.children.push(Node::Element(el("map").with_attr("field", k.clone()).with_attr("column", v.clone())));
    }
    XmlDoc::new(root)
}

/// The merge a part holds, or none when it isn't Docxtor's.
pub fn from_xml(doc: &XmlDoc) -> Option<MailMerge> {
    let root = &doc.root;
    if root.local() != "mailMerge" || !root.attrs.iter().any(|(_, v)| v == NS_MM) {
        return None;
    }
    let columns: Vec<String> = root.elements().find(|e| e.local() == "columns")?.elements().map(|c| c.text()).collect();
    let mut rows = Vec::new();
    let mut active = Vec::new();
    for r in root.elements().find(|e| e.local() == "records")?.elements() {
        let mut row: Vec<String> = r.elements().map(|v| v.text()).collect();
        row.resize(columns.len(), String::new());
        rows.push(row);
        active.push(r.attr("a") != Some("0"));
    }
    if active.iter().all(|a| *a) {
        active.clear();
    }
    let mut mm = MailMerge {
        source: SourceInfo {
            kind: root.attr("kind").unwrap_or_default().to_string(),
            name: root.attr("name").unwrap_or_default().to_string(),
            table: root.attr("table").map(String::from),
            delimiter: root.attr("delimiter").and_then(|d| d.chars().next()),
        },
        table: Arc::new(Table { columns, rows }),
        active,
        ..Default::default()
    };
    for e in root.elements() {
        match e.local() {
            "filter" => mm.filter.push(FilterRule {
                field: e.attr("field").unwrap_or_default().into(),
                op: e.attr("op").unwrap_or_default().into(),
                value: e.attr("value").unwrap_or_default().into(),
                join: e.attr("join").map(String::from),
            }),
            "sort" => mm.sort.push(SortKey { field: e.attr("field").unwrap_or_default().into(), desc: e.attr("desc") == Some("1") }),
            "map" => {
                mm.mapping.insert(e.attr("field").unwrap_or_default().into(), e.attr("column").unwrap_or_default().into());
            }
            _ => {}
        }
    }
    Some(mm)
}

/// The recipients of a document, with the part they live in. Like the sources of a bibliography, it keeps a
/// snapshot for undo and writes the part when the file is saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MergeState {
    /// Part name once it exists in the package.
    pub path: Option<String>,
    pub cfg: Option<Arc<MailMerge>>,
    pub dirty: bool,
}

impl MergeState {
    /// Find Docxtor's part by what it holds (a `mailMerge` root in Docxtor's namespace), not by its name.
    pub fn load(pkg: &Package, main_path: &str) -> MergeState {
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
            let Some(data) = pkg.get(&name) else { continue };
            if !data.windows(NS_MM.len()).any(|w| w == NS_MM.as_bytes()) {
                continue;
            }
            if let Some(mm) = xml::parse(data).ok().and_then(|d| from_xml(&d)) {
                return MergeState { path: Some(name), cfg: Some(Arc::new(mm)), dirty: false };
            }
        }
        MergeState::default()
    }

    pub fn set(&mut self, cfg: Option<MailMerge>) {
        self.cfg = cfg.map(Arc::new);
        self.dirty = true;
    }

    pub fn restore(&mut self, snap: MergeState) {
        if snap.cfg != self.cfg {
            self.dirty = true;
        } else {
            self.dirty |= snap.dirty;
        }
        self.cfg = snap.cfg;
        if self.path.is_none() {
            self.path = snap.path;
        }
    }

    /// Write the part into the package when the recipients changed: created with its properties part, relationship and
    /// content type the first time, as Word lays out custom XML; taken out again (with all of that) when the merge has
    /// ended, so a document that is no longer a merge keeps no list of people.
    pub fn flush(&mut self, pkg: &mut Package, main_path: &str) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let Some(cfg) = self.cfg.clone() else {
            if let Some(path) = self.path.take() {
                pkg.remove_rels_to(main_path, &path);
                let props = pkg.rels_for(&path).unwrap_or_default().into_iter().find(|r| r.rel_type == REL_CUSTOM_XML_PROPS).map(|r| package::resolve_target(&path, &r.target));
                if let Some(p) = props {
                    pkg.remove(&p);
                    pkg.remove_override(&p);
                }
                pkg.remove(&package::rels_path(&path));
                pkg.remove(&path);
            }
            return;
        };
        let doc = to_xml(&cfg);
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
                let ds = XmlDoc::new(
                    Element::new("ds:datastoreItem")
                        .with_attr("ds:itemID", format!("{{F0110000-{n:04X}-4D41-8C00-{:012X}}}", cfg.table.rows.len() as u64 + 0xF011))
                        .with_attr("xmlns:ds", NS_DS)
                        .with_child(Element::new("ds:schemaRefs").with_child(Element::new("ds:schemaRef").with_attr("ds:uri", NS_MM))),
                );
                pkg.set(&props, ds.to_bytes());
                pkg.add_rel(&path, REL_CUSTOM_XML_PROPS, &format!("itemProps{n}.xml"), false);
                pkg.ensure_override(&props, CT_CUSTOM_XML_PROPS);
                pkg.ensure_default("xml", "application/xml");
                pkg.add_rel(main_path, REL_CUSTOM_XML, &relative_to(main_path, &path), false);
                self.path = Some(path.clone());
                path
            }
        };
        pkg.set(&path, doc.to_bytes());
    }
}

// ---------------------------------------------------------------------------
// Word's settings
// ---------------------------------------------------------------------------

const MAIL_MERGE_ORDER: &[&str] = &[
    "w:mainDocumentType", "w:linkToQuery", "w:dataType", "w:connectString", "w:query", "w:dataSource", "w:headerSource", "w:doNotSuppressBlankLines",
    "w:destination", "w:addressFieldName", "w:mailSubject", "w:mailAsAttachment", "w:viewMergedData", "w:activeRecord", "w:checkErrors", "w:odso",
];

fn val(name: &'static str, v: impl Into<AttrValue>) -> Element {
    Element::new(name).with_attr("w:val", v)
}

/// A relationship target for a file name: `file:///people.csv`.
pub fn file_target(name: &str) -> String {
    let mut out = String::from("file:///");
    for c in name.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '#' => out.push_str("%23"),
            c => out.push(c),
        }
    }
    out
}

/// The sheet of a workbook as Word's queries name it: `Sheet1$`.
fn table_name(sheet: &str) -> String {
    format!("{sheet}$")
}

fn provider(name: &str) -> String {
    format!("Provider=Microsoft.ACE.OLEDB.12.0;User ID=Admin;Data Source={name};Mode=Read;Extended Properties=\"HDR=YES;IMEX=1;\";")
}

/// The mapped address fields as Word's `w:fieldMapData`.
fn field_map(mm: &MailMerge) -> Vec<Element> {
    let mut out = Vec::new();
    for (name, _) in STANDARD_FIELDS {
        let Some(col) = super::address::standard_column(mm, name) else { continue };
        let mapped = mm.table.columns.get(col).cloned().unwrap_or_default();
        // Only a field Word has no name for beyond its own is skipped; the rest are all address fields.
        out.push(
            Element::new("w:fieldMapData")
                .with_child(val("w:type", "dbColumn"))
                .with_child(val("w:name", *name))
                .with_child(val("w:mappedName", mapped))
                .with_child(val("w:column", col.to_string()))
                .with_child(val("w:lid", "en-US"))
                .with_child(Element::new("w:dynamicAddress")),
        );
    }
    out
}

/// Build or update `w:mailMerge`. What Word wrote that Docxtor doesn't manage (the destination, the subject, `w:viewMergedData`...)
/// stays as it was; the document type, the data source and `w:odso` are set. `src` is the relationship id of the data source file.
pub fn mail_merge_element(existing: Option<&Element>, main_type: &str, mm: Option<&MailMerge>, src: Option<&str>) -> Element {
    let mut el = existing.cloned().unwrap_or_else(|| Element::new("w:mailMerge"));
    for n in ["w:linkToQuery", "w:dataType", "w:connectString", "w:query", "w:dataSource", "w:odso"] {
        el.remove_children(n);
    }
    crate::props::put_ordered(&mut el, val("w:mainDocumentType", main_type), MAIL_MERGE_ORDER);
    let Some(mm) = mm else {
        crate::props::put_ordered(&mut el, val("w:dataType", "textFile"), MAIL_MERGE_ORDER);
        return el;
    };
    let name = &mm.source.name;
    let kind = mm.source.kind.as_str();
    let linked = src.is_some() && matches!(kind, "csv" | "xlsx");
    if !linked {
        crate::props::put_ordered(&mut el, val("w:dataType", "textFile"), MAIL_MERGE_ORDER);
        return el;
    }
    let sheet = mm.source.table.clone().unwrap_or_else(|| "Sheet1".into());
    let (data_type, connect, query) = if kind == "csv" {
        ("textFile", String::new(), format!("SELECT * FROM {name}"))
    } else {
        ("native", provider(name), format!("SELECT * FROM `{}`", table_name(&sheet)))
    };
    crate::props::put_ordered(&mut el, Element::new("w:linkToQuery"), MAIL_MERGE_ORDER);
    crate::props::put_ordered(&mut el, val("w:dataType", data_type), MAIL_MERGE_ORDER);
    crate::props::put_ordered(&mut el, val("w:connectString", connect), MAIL_MERGE_ORDER);
    crate::props::put_ordered(&mut el, val("w:query", query), MAIL_MERGE_ORDER);
    let rid = src.unwrap_or_default();
    crate::props::put_ordered(&mut el, Element::new("w:dataSource").with_attr("r:id", rid), MAIL_MERGE_ORDER);
    let mut odso = Element::new("w:odso").with_child(val("w:udl", provider(name)));
    if kind == "xlsx" {
        odso.children.push(Node::Element(val("w:table", table_name(&sheet))));
    }
    odso.children.push(Node::Element(Element::new("w:src").with_attr("r:id", rid)));
    if kind == "csv" {
        odso.children.push(Node::Element(val("w:colDelim", (mm.source.delimiter.unwrap_or(',') as u32).to_string())));
        odso.children.push(Node::Element(val("w:type", "text")));
    } else {
        odso.children.push(Node::Element(val("w:type", "database")));
    }
    odso.children.push(Node::Element(Element::new("w:fHdr")));
    for f in field_map(mm) {
        odso.children.push(Node::Element(f));
    }
    crate::props::put_ordered(&mut el, odso, MAIL_MERGE_ORDER);
    el
}

/// What the settings say about the merge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsInfo {
    pub main_type: String,
    /// The file the data source names (the last part of the path).
    pub source_name: Option<String>,
    /// Word shows the merged results when the file opens.
    pub view_merged: bool,
    pub destination: Option<String>,
    /// The address fields Word matched to columns: Word's name to the column's name.
    pub mapping: BTreeMap<String, String>,
}

fn last_segment(path: &str) -> String {
    let p = path.trim_start_matches("file:///").trim_start_matches("file://");
    let p = p.rsplit(['/', '\\']).next().unwrap_or(p);
    p.replace("%20", " ").replace("%23", "#").replace("%25", "%")
}

/// Read `w:mailMerge`. `target_of` gives the target of a relationship id of the settings part.
pub fn read_settings(el: &Element, target_of: &dyn Fn(&str) -> Option<String>) -> SettingsInfo {
    let mut info = SettingsInfo {
        main_type: el.child("w:mainDocumentType").and_then(|e| e.attr("w:val")).unwrap_or("formLetters").to_string(),
        view_merged: el.child("w:viewMergedData").is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off"))),
        destination: el.child("w:destination").and_then(|e| e.attr("w:val")).map(String::from),
        ..Default::default()
    };
    info.source_name = el
        .child("w:dataSource")
        .and_then(|e| e.attr("r:id"))
        .and_then(target_of)
        .map(|t| last_segment(&t))
        .or_else(|| el.child("w:odso").and_then(|o| o.child("w:src")).and_then(|e| e.attr("r:id")).and_then(target_of).map(|t| last_segment(&t)))
        .or_else(|| el.child("w:query").and_then(|e| e.attr("w:val")).and_then(|q| q.strip_prefix("SELECT * FROM ")).map(last_segment).filter(|n| !n.starts_with('`')));
    if let Some(o) = el.child("w:odso") {
        for m in o.children_named("w:fieldMapData") {
            let name = m.child("w:name").and_then(|e| e.attr("w:val"));
            let mapped = m.child("w:mappedName").and_then(|e| e.attr("w:val"));
            if let (Some(n), Some(c)) = (name, mapped) {
                if !c.is_empty() && STANDARD_FIELDS.iter().any(|(s, _)| norm(s) == norm(n)) {
                    info.mapping.insert(n.to_string(), c.to_string());
                }
            }
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MailMerge {
        MailMerge {
            source: SourceInfo { kind: "csv".into(), name: "my people.csv".into(), table: None, delimiter: Some(';') },
            table: Arc::new(Table::from_rows(vec!["First Name".into(), "Given".into()], vec![vec!["Ada ".into(), "a\nb  c".into()], vec![String::new(), " lead".into()]])),
            active: vec![true, false],
            filter: vec![FilterRule { field: "First Name".into(), op: "ne".into(), value: "x".into(), join: Some("or".into()) }],
            sort: vec![SortKey { field: "Given".into(), desc: true }],
            mapping: [("Last Name".to_string(), "Given".to_string())].into(),
        }
    }

    #[test]
    fn the_part_keeps_everything() {
        let mm = sample();
        let bytes = to_xml(&mm).to_bytes();
        let back = from_xml(&xml::parse(&bytes).unwrap()).unwrap();
        // Spaces and line breaks inside a value, empty values and the ticks all come back.
        assert_eq!(back.table.rows[0], ["Ada", "a\nb  c"]);
        assert_eq!(back.table.rows[1], ["", " lead"]);
        assert_eq!((back.active.clone(), back.filter.clone(), back.sort.clone(), back.mapping.clone()), (mm.active, mm.filter, mm.sort, mm.mapping));
        assert_eq!(back.source, mm.source);
    }

    #[test]
    fn settings_are_written_as_word_does() {
        let mm = sample();
        let el = mail_merge_element(None, "formLetters", Some(&mm), Some("rId7"));
        let names: Vec<&str> = el.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:mainDocumentType", "w:linkToQuery", "w:dataType", "w:connectString", "w:query", "w:dataSource", "w:odso"]);
        assert_eq!(el.child("w:dataType").unwrap().attr("w:val"), Some("textFile"));
        assert_eq!(el.child("w:query").unwrap().attr("w:val"), Some("SELECT * FROM my people.csv"));
        let odso = el.child("w:odso").unwrap();
        assert_eq!(odso.child("w:colDelim").unwrap().attr("w:val"), Some("59"));
        assert_eq!(odso.child("w:type").unwrap().attr("w:val"), Some("text"));
        assert!(odso.child("w:fHdr").is_some());
        // The mapped field is written with its column.
        let ln = odso.children_named("w:fieldMapData").find(|m| m.child("w:name").unwrap().attr("w:val") == Some("Last Name")).unwrap();
        assert_eq!((ln.child("w:mappedName").unwrap().attr("w:val"), ln.child("w:column").unwrap().attr("w:val")), (Some("Given"), Some("1")));
        let info = read_settings(&el, &|id| (id == "rId7").then(|| file_target("my people.csv")));
        assert_eq!((info.main_type.as_str(), info.source_name.as_deref()), ("formLetters", Some("my people.csv")));
        assert_eq!(info.mapping.get("Last Name").map(String::as_str), Some("Given"));
        // A workbook is read through the OLE DB provider and names its sheet.
        let mut book = sample();
        book.source = SourceInfo { kind: "xlsx".into(), name: "list.xlsx".into(), table: Some("People".into()), delimiter: None };
        let el = mail_merge_element(None, "mailingLabels", Some(&book), Some("rId2"));
        assert_eq!(el.child("w:dataType").unwrap().attr("w:val"), Some("native"));
        assert_eq!(el.child("w:query").unwrap().attr("w:val"), Some("SELECT * FROM `People$`"));
        assert_eq!(el.child("w:odso").unwrap().child("w:table").unwrap().attr("w:val"), Some("People$"));
    }

    #[test]
    fn what_word_wrote_is_kept() {
        let existing = Element::new("w:mailMerge")
            .with_child(val("w:mainDocumentType", "envelopes"))
            .with_child(val("w:dataType", "textFile"))
            .with_child(val("w:mailSubject", "Hello"))
            .with_child(Element::new("w:viewMergedData"));
        let el = mail_merge_element(Some(&existing), "formLetters", Some(&sample()), Some("rId1"));
        let names: Vec<&str> = el.elements().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["w:mainDocumentType", "w:linkToQuery", "w:dataType", "w:connectString", "w:query", "w:dataSource", "w:mailSubject", "w:viewMergedData", "w:odso"]);
        assert_eq!(el.child("w:mainDocumentType").unwrap().attr("w:val"), Some("formLetters"));
        // No data source: just the type.
        let bare = mail_merge_element(None, "catalog", None, None);
        assert_eq!(bare.elements().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["w:mainDocumentType", "w:dataType"]);
    }
}
