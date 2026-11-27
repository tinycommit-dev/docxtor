//! Flat OPC: a whole Word package as one XML file (`pkg:package`, Word's "Word XML Document"). Each
//! `pkg:part` holds one part of the zip: XML parts inline, binary parts (pictures, fonts) as base64.
//! `package` turns it back into the parts of a .docx; the content types it carries become
//! `[Content_Types].xml`.

use super::OpenError;
use crate::fragment::b64_decode;
use crate::package::{Package, Part};
use crate::xml::{self, AttrValue, Element, Name, Node, XmlDoc};

const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

fn attr<'a>(e: &'a Element, name: &str) -> Option<&'a str> {
    e.attrs.iter().find(|(k, _)| xml::local_name(k) == name).map(|(_, v)| v.as_str())
}

/// The parts of a Flat OPC file as a package.
pub fn read(bytes: &[u8]) -> Result<Package, OpenError> {
    let doc = match xml::parse(bytes) {
        Ok(d) => d,
        Err(_) => {
            let r = super::xmlfix::recover(bytes).ok_or_else(|| OpenError::new("damaged", "The XML file can't be read."))?;
            xml::parse(&r.bytes).map_err(|e| OpenError::new("damaged", e.to_string()))?
        }
    };
    if xml::local_name(&doc.root.name) != "package" {
        return Err(OpenError::new("unknown", "This XML file is not a Word package."));
    }
    // Namespace declarations made on the package or part apply to the XML inside.
    let inherited = |a: &Element, b: &Element| -> Vec<(Name, AttrValue)> {
        a.attrs.iter().chain(b.attrs.iter()).filter(|(k, _)| k.starts_with("xmlns") && k != "xmlns:pkg").cloned().collect()
    };
    let mut pkg = Package::default();
    let mut types: Vec<(String, String)> = Vec::new();
    for part in doc.root.elements().filter(|e| xml::local_name(&e.name) == "part") {
        let Some(name) = attr(part, "name") else { continue };
        let name = name.trim_start_matches('/').to_string();
        // The content types are made from the parts' own attributes below.
        if name == "[Content_Types].xml" {
            continue;
        }
        let ctype = attr(part, "contentType").unwrap_or_default().to_string();
        let mut data: Option<Vec<u8>> = None;
        for c in part.elements() {
            match xml::local_name(&c.name) {
                "xmlData" => {
                    if let Some(inner) = c.elements().next() {
                        let mut inner = inner.clone();
                        for (k, v) in inherited(&doc.root, c) {
                            if inner.attr(&k).is_none() {
                                inner.attrs.insert(0, (k, v));
                            }
                        }
                        data = Some(XmlDoc::new(inner).to_bytes());
                    }
                }
                "binaryData" => {
                    let text: String = c.children.iter().filter_map(|n| if let Node::Text(t) | Node::CData(t) = n { Some(t.as_str()) } else { None }).collect();
                    data = b64_decode(&text);
                }
                _ => {}
            }
        }
        let Some(data) = data else { continue };
        let stored = attr(part, "compression") == Some("store") || name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg");
        types.push((name.clone(), ctype));
        pkg.parts.retain(|p| p.name != name);
        pkg.parts.push(Part { name, data, stored });
    }
    if pkg.parts.is_empty() {
        return Err(OpenError::new("no-text", "The package has no parts."));
    }
    let mut ct = Element::new("Types").with_attr("xmlns", CT_NS);
    ct = ct.with_child(Element::new("Default").with_attr("Extension", "rels").with_attr("ContentType", "application/vnd.openxmlformats-package.relationships+xml"));
    ct = ct.with_child(Element::new("Default").with_attr("Extension", "xml").with_attr("ContentType", "application/xml"));
    for (name, ctype) in types {
        if ctype.is_empty() || name.ends_with(".rels") {
            continue;
        }
        ct = ct.with_child(Element::new("Override").with_attr("PartName", format!("/{name}")).with_attr("ContentType", ctype));
    }
    pkg.parts.insert(0, Part { name: "[Content_Types].xml".into(), data: XmlDoc::new(ct).to_bytes(), stored: false });
    Ok(pkg)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> String {
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
        let doc = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Flat hello</w:t></w:r></w:p></w:body></w:document>"#;
        format!(
            r#"<?xml version="1.0" standalone="yes"?><?mso-application progid="Word.Document"?><pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage"><pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml" pkg:padding="512"><pkg:xmlData>{rels}</pkg:xmlData></pkg:part><pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"><pkg:xmlData>{doc}</pkg:xmlData></pkg:part><pkg:part pkg:name="/word/media/a.png" pkg:contentType="image/png" pkg:compression="store"><pkg:binaryData>iVBORw0KGgo=</pkg:binaryData></pkg:part></pkg:package>"#
        )
    }

    #[test]
    fn parts_come_out_as_a_package() {
        let pkg = read(sample().as_bytes()).unwrap();
        assert!(pkg.has("word/document.xml") && pkg.has("_rels/.rels") && pkg.has("[Content_Types].xml"));
        assert_eq!(pkg.get("word/media/a.png").unwrap(), &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
        assert!(ct.contains("/word/document.xml") && ct.contains("image/png"), "{ct}");
    }
}
