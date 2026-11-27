//! Repairing a Word package: the parts a damaged zip gave, XML that doesn't parse, relationships and
//! content types that are missing or point nowhere. A package that is already sound is left exactly as
//! it was read, so saving keeps untouched parts byte for byte.

use super::{strict, xmlfix, zipscan, Note, OpenError};
use crate::package::{self, Package, Part};
use crate::xml::{self, Element, Node, XmlDoc};

const REL_PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const CT_MAIN: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";
const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const CT_FONTS: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml";
const CT_COMMENTS: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const CT_WEB: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.webSettings+xml";
const REL_FONTS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable";
const REL_WEB: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/webSettings";
const REL_APP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";

/// The parts every Word file may have, by their usual names: name, relationship type, content type.
const KNOWN: &[(&str, &str, &str)] = &[
    ("word/styles.xml", package::REL_STYLES, package::CT_STYLES),
    ("word/numbering.xml", package::REL_NUMBERING, package::CT_NUMBERING),
    ("word/settings.xml", package::REL_SETTINGS, package::CT_SETTINGS),
    ("word/fontTable.xml", REL_FONTS, CT_FONTS),
    ("word/webSettings.xml", REL_WEB, CT_WEB),
    ("word/footnotes.xml", package::REL_FOOTNOTES, package::CT_FOOTNOTES),
    ("word/endnotes.xml", package::REL_ENDNOTES, package::CT_ENDNOTES),
    ("word/comments.xml", package::REL_COMMENTS, CT_COMMENTS),
    ("word/theme/theme1.xml", package::REL_THEME, CT_THEME),
];

const MEDIA: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("bmp", "image/bmp"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("emf", "image/x-emf"),
    ("wmf", "image/x-wmf"),
    ("svg", "image/svg+xml"),
];

fn is_xml_part(name: &str) -> bool {
    (name.ends_with(".xml") || name.ends_with(".rels")) && !name.contains("/embeddings/")
}

/// Build a package from the zip's entries and repair it. Fails when no Word document part can be found.
pub fn load_package(z: zipscan::ZipRead) -> Result<(Package, Vec<Note>), OpenError> {
    let mut notes = Vec::new();
    if z.scanned {
        notes.push(Note::count("zip-directory", z.entries.len()));
    }
    let cut: Vec<String> = z.damaged().map(|e| e.name.clone()).collect();
    if !cut.is_empty() {
        notes.push(Note::count("zip-damaged", cut.len()).with_detail(cut.join(", ")));
    }
    let mut pkg = Package::from_parts(z.entries.into_iter().map(|e| Part { name: e.name, data: e.data, stored: e.stored }).collect());

    // Strict Open XML first, so the checks below see the standard names.
    let strict_file = strict::is_strict(&pkg);
    if strict_file {
        strict::to_transitional(&mut pkg);
        notes.push(Note::new("strict"));
    }

    // Malformed XML: keep what parses. A part nothing can be read from stays as it was (it may still mean something
    // to a program that knows it), and the person is told.
    let main_hint = find_main(&pkg);
    let mut recovered = Vec::new();
    let mut unreadable = Vec::new();
    let mut text_cut = false;
    for part in pkg.parts.iter_mut() {
        if !is_xml_part(&part.name) || xmlfix::well_formed(&part.data) {
            continue;
        }
        match xmlfix::recover(&part.data) {
            Some(r) => {
                if r.cut && Some(&part.name) == main_hint.as_ref() {
                    text_cut = true;
                }
                part.data = r.bytes;
                recovered.push(part.name.clone());
            }
            None => unreadable.push(part.name.clone()),
        }
    }
    if text_cut {
        notes.push(Note::new("text-cut"));
    }
    if !recovered.is_empty() {
        notes.push(Note::count("xml-recovered", recovered.len()).with_detail(recovered.join(", ")));
    }
    if !unreadable.is_empty() {
        notes.push(Note::count("xml-unreadable", unreadable.len()).with_detail(unreadable.join(", ")));
    }

    let Some(main) = find_main(&pkg) else {
        return Err(if pkg.parts.is_empty() { OpenError::new("damaged", "Nothing could be read from the file.") } else { OpenError::new("no-text", "The document has no text part Docxtor can read.") });
    };
    fix_structure(&mut pkg, &main, &mut notes);
    Ok((pkg, notes))
}

/// The main document part: the one the package relationships name, else `word/document.xml`, else any
/// part under `word/` whose root is a `document`.
fn find_main(pkg: &Package) -> Option<String> {
    if let Ok(rels) = pkg.rels_for("") {
        if let Some(r) = rels.iter().find(|r| r.rel_type == package::REL_OFFICE_DOCUMENT && !r.external) {
            let t = package::resolve_target("", &r.target);
            if pkg.has(&t) {
                return Some(t);
            }
        }
    }
    if pkg.has("word/document.xml") {
        return Some("word/document.xml".into());
    }
    pkg.parts
        .iter()
        .filter(|p| p.name.ends_with(".xml") && (p.name.starts_with("word/") || !p.name.contains('/')))
        .find(|p| {
            let head = String::from_utf8_lossy(&p.data[..p.data.len().min(1500)]);
            head.contains(":document ") || head.contains(":document>")
        })
        .map(|p| p.name.clone())
}

fn rels_doc() -> XmlDoc {
    XmlDoc::new(Element::new("Relationships").with_attr("xmlns", REL_PKG))
}

/// The part a `.rels` file belongs to (`""` for the package's own).
fn source_of(rels_name: &str) -> String {
    if rels_name == "_rels/.rels" {
        return String::new();
    }
    let (dir, file) = rels_name.rsplit_once('/').unwrap_or(("", rels_name));
    let base = dir.strip_suffix("_rels").unwrap_or(dir).trim_end_matches('/');
    let file = file.strip_suffix(".rels").unwrap_or(file);
    if base.is_empty() {
        file.to_string()
    } else {
        format!("{base}/{file}")
    }
}

/// Relationships and content types.
fn fix_structure(pkg: &mut Package, main: &str, notes: &mut Vec<Note>) {
    // The package's own relationship to the main part.
    let root_ok = pkg.rels_for("").map(|r| r.iter().any(|r| r.rel_type == package::REL_OFFICE_DOCUMENT && !r.external && package::resolve_target("", &r.target) == main)).unwrap_or(false);
    let mut restored = 0;
    if !root_ok {
        if pkg.get("_rels/.rels").and_then(|d| xml::parse(d).ok()).is_none() {
            pkg.set("_rels/.rels", rels_doc().to_bytes());
        } else {
            drop_rels(pkg, "_rels/.rels", |r| r.attr("Type") == Some(package::REL_OFFICE_DOCUMENT));
        }
        pkg.add_rel("", package::REL_OFFICE_DOCUMENT, main, false);
        restored += 1;
    }
    for (name, rel) in [("docProps/core.xml", package::REL_CORE), ("docProps/app.xml", REL_APP)] {
        // A relationship that already points at the part under a misspelt type is left alone: adding a second one
        // makes one part the target of two relationship types, which Word and the Open XML SDK refuse.
        let rels = pkg.rels_for("").unwrap_or_default();
        let targeted = rels.iter().any(|r| !r.external && package::resolve_target("", &r.target).eq_ignore_ascii_case(name));
        if pkg.has(name) && !targeted && !rels.iter().any(|r| r.rel_type == rel) {
            pkg.add_rel("", rel, name, false);
            restored += 1;
        }
    }

    // Relationships that point at parts the package doesn't have are counted and left as they are: a missing
    // picture shows as a placeholder, which is more than dropping the relationship would keep.
    let rels_names: Vec<String> = pkg.parts.iter().filter(|p| p.name.ends_with(".rels")).map(|p| p.name.clone()).collect();
    // A name counts as there when it matches exactly, with %-escapes undone, or without regard to case.
    let have: std::collections::HashSet<String> = pkg.parts.iter().map(|p| p.name.to_lowercase()).collect();
    let mut missing = 0;
    for rn in &rels_names {
        let src = source_of(rn);
        missing += count_rels(pkg, rn, |r| {
            r.attr("TargetMode") != Some("External") && {
                let t = r.attr("Target").unwrap_or_default();
                let name = package::resolve_target(&src, t);
                !t.contains('#') && !t.contains("://") && !t.starts_with("mailto:") && !have.contains(&name.to_lowercase()) && !have.contains(&unescape(&name).to_lowercase())
            }
        });
    }
    if missing > 0 {
        notes.push(Note::count("parts-missing", missing));
    }

    // The parts a document usually has, when they exist but nothing relates to them.
    for (name, rel, _) in KNOWN {
        if !pkg.has(name) {
            continue;
        }
        let rels = pkg.rels_for(main).unwrap_or_default();
        if rels.iter().any(|r| r.rel_type == *rel && package::resolve_target(main, &r.target) == *name) {
            continue;
        }
        if rels.iter().any(|r| r.rel_type == *rel) {
            continue;
        }
        let rel_main_dir = main.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let target = name.strip_prefix(&format!("{rel_main_dir}/")).filter(|_| !rel_main_dir.is_empty()).unwrap_or(name);
        pkg.add_rel(main, rel, target, false);
        restored += 1;
    }
    if restored > 0 {
        notes.push(Note::count("rels-restored", restored));
    }

    // Content types.
    let mut types = 0;
    if pkg.xml("[Content_Types].xml").is_err() {
        pkg.set("[Content_Types].xml", XmlDoc::new(Element::new("Types").with_attr("xmlns", "http://schemas.openxmlformats.org/package/2006/content-types")).to_bytes());
        types += 1;
    }
    let ct = pkg.xml("[Content_Types].xml").ok();
    let has_default = |ext: &str| ct.as_ref().is_some_and(|d| d.root.elements().any(|e| e.local() == "Default" && e.attr("Extension").is_some_and(|x| x.eq_ignore_ascii_case(ext))));
    let has_override = |part: &str| ct.as_ref().is_some_and(|d| d.root.elements().any(|e| e.local() == "Override" && e.attr("PartName") == Some(format!("/{part}").as_str())));
    let mut need_default: Vec<(&str, &str)> = Vec::new();
    let mut need_override: Vec<(String, &str)> = Vec::new();
    for (ext, c) in [("rels", CT_RELS), ("xml", "application/xml")] {
        if !has_default(ext) {
            need_default.push((ext, c));
        }
    }
    for (ext, c) in MEDIA {
        if !has_default(ext) && pkg.parts.iter().any(|p| p.name.to_ascii_lowercase().ends_with(&format!(".{ext}"))) {
            need_default.push((ext, c));
        }
    }
    let mut want: Vec<(String, &str)> = vec![(main.to_string(), CT_MAIN)];
    for (name, _, c) in KNOWN {
        if pkg.has(name) {
            want.push((name.to_string(), c));
        }
    }
    for (name, c) in [("docProps/core.xml", package::CT_CORE), ("docProps/app.xml", CT_APP)] {
        if pkg.has(name) {
            want.push((name.to_string(), c));
        }
    }
    for p in &pkg.parts {
        let n = p.name.as_str();
        let leaf = n.rsplit('/').next().unwrap_or(n);
        if n.starts_with("word/") && !n.contains("/_rels/") && n.ends_with(".xml") {
            if leaf.starts_with("header") {
                want.push((n.to_string(), package::CT_HEADER));
            } else if leaf.starts_with("footer") {
                want.push((n.to_string(), package::CT_FOOTER));
            }
        }
    }
    for (name, c) in want {
        if !has_override(&name) {
            need_override.push((name, c));
        }
    }
    for (ext, c) in need_default {
        pkg.ensure_default(ext, c);
        types += 1;
    }
    for (name, c) in need_override {
        pkg.ensure_override(&name, c);
        types += 1;
    }
    if types > 0 {
        notes.push(Note::count("types-restored", types));
    }
}

/// How many relationships of a `.rels` part `pred` says yes to.
fn count_rels(pkg: &Package, rels_name: &str, pred: impl Fn(&Element) -> bool) -> usize {
    let Some(doc) = pkg.get(rels_name).and_then(|d| xml::parse(d).ok()) else { return 0 };
    doc.root.elements().filter(|e| e.local() == "Relationship" && pred(e)).count()
}

/// A part name with its `%20`-style escapes undone.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'%'));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Remove the relationships of a `.rels` part for which `bad` says so; returns how many went.
fn drop_rels(pkg: &mut Package, rels_name: &str, bad: impl Fn(&Element) -> bool) -> usize {
    let Some(mut doc) = pkg.get(rels_name).and_then(|d| xml::parse(d).ok()) else { return 0 };
    let before = doc.root.children.len();
    doc.root.children.retain(|n| !matches!(n, Node::Element(e) if e.local() == "Relationship" && bad(e)));
    let gone = before - doc.root.children.len();
    if gone > 0 {
        pkg.set(rels_name, doc.to_bytes());
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, data: &str) -> zipscan::RawEntry {
        zipscan::RawEntry { name: name.into(), data: data.as_bytes().to_vec(), stored: false, damaged: false }
    }

    const DOC: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;

    #[test]
    fn source_names() {
        assert_eq!(source_of("_rels/.rels"), "");
        assert_eq!(source_of("word/_rels/document.xml.rels"), "word/document.xml");
    }

    #[test]
    fn a_sound_package_is_left_alone() {
        let bytes = crate::Document::new_blank().save().unwrap();
        let z = zipscan::read(&bytes, &crate::budget::Budget::default()).unwrap();
        let before: Vec<(String, Vec<u8>)> = z.entries.iter().map(|e| (e.name.clone(), e.data.clone())).collect();
        let (pkg, notes) = load_package(z).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        for (n, d) in before {
            assert_eq!(pkg.get(&n).unwrap(), &d[..], "{n}");
        }
    }

    #[test]
    fn a_bare_document_part_gets_its_relationships_and_types() {
        let z = zipscan::ZipRead { entries: vec![entry("word/document.xml", DOC), entry("word/styles.xml", r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#)], scanned: false };
        let (pkg, notes) = load_package(z).unwrap();
        assert!(notes.iter().any(|n| n.code == "rels-restored"), "{notes:?}");
        assert!(notes.iter().any(|n| n.code == "types-restored"), "{notes:?}");
        assert_eq!(pkg.main_document_path(), "word/document.xml");
        assert!(pkg.rels_for("word/document.xml").unwrap().iter().any(|r| r.rel_type == package::REL_STYLES));
        let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
        assert!(ct.contains("/word/document.xml") && ct.contains("/word/styles.xml"), "{ct}");
        let mut doc = crate::Document::from_package(pkg).unwrap();
        assert!(doc.render_all().order.len() == 1);
    }

    #[test]
    fn a_cut_document_part_keeps_its_text() {
        let cut = DOC.replace("</w:t></w:r></w:p></w:body></w:document>", "</w:t></w:r></w:p><w:p><w:r><w:t>Seco");
        let z = zipscan::ZipRead { entries: vec![entry("word/document.xml", &cut)], scanned: false };
        let (_, notes) = load_package(z).unwrap();
        assert!(notes.iter().any(|n| n.code == "text-cut"), "{notes:?}");
        assert!(notes.iter().any(|n| n.code == "xml-recovered"), "{notes:?}");
    }

    #[test]
    fn damage_in_the_middle_is_not_reported_as_cut_text() {
        // One broken tag, everything after it intact: the part is repaired, but no text is lost at the end.
        let broken = DOC.replace("</w:t></w:r>", "</w:t></w:x></w:r>");
        let z = zipscan::ZipRead { entries: vec![entry("word/document.xml", &broken)], scanned: false };
        let (_, notes) = load_package(z).unwrap();
        assert!(notes.iter().any(|n| n.code == "xml-recovered"), "{notes:?}");
        assert!(!notes.iter().any(|n| n.code == "text-cut"), "{notes:?}");
    }

    #[test]
    fn relationships_to_missing_parts_are_counted_and_kept() {
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/gone.png"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/></Relationships>"#;
        let z = zipscan::ZipRead { entries: vec![entry("word/document.xml", DOC), entry("word/_rels/document.xml.rels", rels)], scanned: false };
        let (pkg, notes) = load_package(z).unwrap();
        assert!(notes.iter().any(|n| n.code == "parts-missing" && n.count == 1), "{notes:?}");
        // The picture's relationship stays, so the page can show a placeholder for it.
        assert_eq!(pkg.rels_for("word/document.xml").unwrap().len(), 2);
    }

    #[test]
    fn relationships_to_escaped_or_differently_cased_names_are_kept() {
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/my%20picture.png"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/Other.PNG"/></Relationships>"#;
        let z = zipscan::ZipRead {
            entries: vec![entry("word/document.xml", DOC), entry("word/_rels/document.xml.rels", rels), entry("word/media/my picture.png", "x"), entry("word/media/other.png", "y")],
            scanned: false,
        };
        let (pkg, notes) = load_package(z).unwrap();
        assert!(!notes.iter().any(|n| n.code == "parts-missing"), "{notes:?}");
        assert_eq!(pkg.rels_for("word/document.xml").unwrap().len(), 2);
    }

    #[test]
    fn no_document_part_is_an_error() {
        let z = zipscan::ZipRead { entries: vec![entry("something.txt", "hi")], scanned: false };
        assert_eq!(load_package(z).err().unwrap().code, "no-text");
    }
}
