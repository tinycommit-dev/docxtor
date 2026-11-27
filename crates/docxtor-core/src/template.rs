//! Word templates (`.dotx`, and `.dotm` with macros). A template is a `.docx` package whose main part has another
//! content type; everything else in it is the same. These functions switch between the two without touching any other
//! part, so styles, headers, page setup, building blocks and macros come across as they are.

use crate::docprops;
use crate::package::{Package, PackageError};
use crate::xml::{Node, XmlDoc};

pub const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
pub const CT_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml";
pub const CT_DOCUMENT_MACRO: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";
pub const CT_TEMPLATE_MACRO: &str =
    "application/vnd.ms-word.template.macroEnabledTemplate.main+xml";

/// What a package is: a document or a template, with or without macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    pub template: bool,
    pub macros: bool,
}

fn main_override(pkg: &Package) -> Option<(XmlDoc, usize)> {
    let doc = pkg.xml("[Content_Types].xml").ok()?;
    let main = format!("/{}", pkg.main_document_path());
    let at = doc.root.children.iter().position(|n| matches!(n, Node::Element(e) if e.local() == "Override" && e.attr("PartName") == Some(main.as_str())))?;
    Some((doc, at))
}

/// Whether the package is a document, a template or a macro-enabled one, from the content type of its main part.
pub fn kind_of(bytes: &[u8]) -> Result<Kind, PackageError> {
    let pkg = Package::open(bytes)?;
    let ct = main_override(&pkg)
        .and_then(|(doc, at)| {
            if let Node::Element(e) = &doc.root.children[at] {
                e.attr("ContentType").map(String::from)
            } else {
                None
            }
        })
        .unwrap_or_default();
    Ok(match ct.as_str() {
        CT_TEMPLATE => Kind {
            template: true,
            macros: false,
        },
        CT_TEMPLATE_MACRO => Kind {
            template: true,
            macros: true,
        },
        CT_DOCUMENT_MACRO => Kind {
            template: false,
            macros: true,
        },
        _ => Kind {
            template: false,
            macros: false,
        },
    })
}

fn convert(bytes: &[u8], template: bool) -> Result<(Package, bool), PackageError> {
    let mut pkg = Package::open(bytes)?;
    let Some((mut doc, at)) = main_override(&pkg) else {
        return Ok((pkg, false));
    };
    let macros = kind_of(bytes)?.macros;
    let want = match (template, macros) {
        (true, false) => CT_TEMPLATE,
        (true, true) => CT_TEMPLATE_MACRO,
        (false, false) => CT_DOCUMENT,
        (false, true) => CT_DOCUMENT_MACRO,
    };
    let Node::Element(e) = &mut doc.root.children[at] else {
        return Ok((pkg, false));
    };
    if e.attr("ContentType") == Some(want) {
        return Ok((pkg, false));
    }
    e.set_attr("ContentType", want);
    pkg.set("[Content_Types].xml", doc.to_bytes());
    Ok((pkg, true))
}

/// The same package as a template (`.dotx`, or `.dotm` when it has macros). Bytes that are one already come back
/// unchanged.
pub fn as_template(bytes: &[u8]) -> Result<Vec<u8>, PackageError> {
    match convert(bytes, true)? {
        (pkg, true) => pkg.to_bytes(),
        _ => Ok(bytes.to_vec()),
    }
}

/// The same package as a document (`.docx`, or `.docm` when it has macros). With `now` (an ISO 8601 UTC time) it is
/// also a new document: made at that time, with no record of who changed it last or how often, as Word makes a
/// document from a template.
pub fn as_document(bytes: &[u8], now: Option<&str>) -> Result<Vec<u8>, PackageError> {
    let (mut pkg, changed) = convert(bytes, false)?;
    let mut touched = changed;
    if let Some(now) = now.filter(|n| docprops::is_w3cdtf(n)) {
        let core_path = "docProps/core.xml";
        if let Ok(mut core) = pkg.xml(core_path) {
            core.root.children.retain(|n| !matches!(n, Node::Element(e) if matches!(e.local(), "created" | "modified" | "lastModifiedBy" | "revision" | "lastPrinted")));
            docprops::stamp(&mut core, now, true);
            core.root
                .children
                .retain(|n| !matches!(n, Node::Element(e) if e.local() == "modified"));
            pkg.set(core_path, core.to_bytes());
            touched = true;
        }
    }
    if touched {
        pkg.to_bytes()
    } else {
        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> Vec<u8> {
        let mut d = crate::Document::new_blank();
        d.save().unwrap()
    }

    #[test]
    fn a_document_becomes_a_template_and_back_changing_only_its_content_type() {
        let doc = blank();
        assert_eq!(
            kind_of(&doc).unwrap(),
            Kind {
                template: false,
                macros: false
            }
        );
        let tmpl = as_template(&doc).unwrap();
        assert_eq!(
            kind_of(&tmpl).unwrap(),
            Kind {
                template: true,
                macros: false
            }
        );
        assert_eq!(as_template(&tmpl).unwrap(), tmpl);
        let (a, b) = (Package::open(&doc).unwrap(), Package::open(&tmpl).unwrap());
        assert_eq!(a.parts.len(), b.parts.len());
        for p in &a.parts {
            if p.name != "[Content_Types].xml" {
                assert_eq!(
                    b.get(&p.name),
                    Some(p.data.as_slice()),
                    "{} changed",
                    p.name
                );
            }
        }
        let ct = String::from_utf8(b.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
        assert!(
            ct.contains(CT_TEMPLATE) && !ct.contains(CT_DOCUMENT),
            "{ct}"
        );
        let back = as_document(&tmpl, None).unwrap();
        assert_eq!(
            kind_of(&back).unwrap(),
            Kind {
                template: false,
                macros: false
            }
        );
        // The template opens and edits like any document and stays a template when saved.
        let mut d = crate::Document::open(&tmpl).unwrap();
        let out = d.save().unwrap();
        assert!(kind_of(&out).unwrap().template);
    }

    #[test]
    fn macro_enabled_packages_keep_their_macros_flag() {
        let mut pkg = Package::open(&blank()).unwrap();
        let mut ct = pkg.xml("[Content_Types].xml").unwrap();
        for n in ct.root.children.iter_mut() {
            if let Node::Element(e) = n {
                if e.attr("ContentType") == Some(CT_DOCUMENT) {
                    e.set_attr("ContentType", CT_DOCUMENT_MACRO);
                }
            }
        }
        pkg.set("[Content_Types].xml", ct.to_bytes());
        let docm = pkg.to_bytes().unwrap();
        assert_eq!(
            kind_of(&docm).unwrap(),
            Kind {
                template: false,
                macros: true
            }
        );
        let dotm = as_template(&docm).unwrap();
        assert_eq!(
            kind_of(&dotm).unwrap(),
            Kind {
                template: true,
                macros: true
            }
        );
        assert_eq!(
            kind_of(&as_document(&dotm, None).unwrap()).unwrap(),
            Kind {
                template: false,
                macros: true
            }
        );
    }

    #[test]
    fn a_new_document_from_a_template_starts_a_fresh_history() {
        let mut d = crate::Document::new_blank();
        let first = d.render_all().order[0];
        d.apply_json(
            &serde_json::json!({"op": "set_doc_props", "author": "Ana", "title": "Letter"})
                .to_string(),
            false,
        )
        .unwrap();
        d.apply_json(
            &serde_json::json!({"op": "insert_markdown", "before": first, "markdown": "Dear"})
                .to_string(),
            false,
        )
        .unwrap();
        d.set_save_info(crate::document::SaveInfo {
            user: Some("Ben".into()),
            pages: None,
            lines: None,
        });
        let tmpl = as_template(&d.save_stamped("2024-01-02T03:04:05Z").unwrap()).unwrap();
        let made = as_document(&tmpl, Some("2026-10-14T09:00:00Z")).unwrap();
        let props = crate::Document::open(&made).unwrap().doc_props();
        assert_eq!(
            (
                props.created.as_deref(),
                props.modified,
                props.last_modified_by
            ),
            (Some("2026-10-14T09:00:00Z"), None, None)
        );
        assert_eq!(
            (props.author.as_deref(), props.title.as_deref()),
            (Some("Ana"), Some("Letter"))
        );
        let pkg = Package::open(&made).unwrap();
        for part in &pkg.parts {
            if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
                let doc = crate::xml::parse(&part.data).unwrap();
                crate::xml::check_namespaces(&doc).unwrap();
            }
        }
    }
}
