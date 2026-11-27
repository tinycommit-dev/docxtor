//! Building blocks: the quick parts a .docx or .dotx keeps in its glossary document (`word/glossary/document.xml`),
//! read without touching the part (it stays byte for byte as it was), and the content controls that came from a
//! gallery (`w:docPartObj`, like Word's built-in tables of contents and cover pages).

use super::paste::Gather;
use super::*;
use crate::fragment::{self, FragDefaults, Fragment};

const REL_GLOSSARY: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/glossaryDocument";

/// A building block of the document's glossary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingBlock {
    /// Its place among the blocks (what `Document::building_block_fragment` takes).
    pub index: usize,
    pub name: String,
    /// The gallery it sits in: `autoText`, `docParts`, `tblOfContents`, `coverPg`, `custQuickParts`...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gallery: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    /// The start of its text.
    pub text: String,
    /// It is a run of text that goes in at the caret, not whole paragraphs.
    pub inline: bool,
}

/// A content control made from a gallery (`w:docPartObj`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocPartControl {
    /// The first paragraph inside it.
    pub p: NodeId,
    /// Its gallery, such as `Table of Contents`, `Cover Pages` or `Bibliographies`.
    pub gallery: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// The title or alias Word shows on its tab.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Word keeps it unique: inserting the same one again replaces it.
    pub unique: bool,
}

fn val(e: &Element, child: &str) -> Option<String> {
    e.child(child).and_then(|c| c.attr("w:val")).map(String::from).filter(|v| !v.is_empty())
}

/// The block-level content of a part of a docPart body: paragraphs and tables, looking into content controls.
fn body_elements(body: &Element, out: &mut Vec<Element>) {
    for c in body.elements() {
        match c.name.as_str() {
            "w:p" | "w:tbl" => out.push(c.clone()),
            "w:sdt" => {
                if let Some(content) = c.child("w:sdtContent") {
                    body_elements(content, out);
                }
            }
            _ => {}
        }
    }
}

impl Document {
    fn glossary_path(&self) -> Option<String> {
        let rels = self.pkg.rels_for(&self.main_path).ok()?;
        let r = rels.into_iter().find(|r| r.rel_type == REL_GLOSSARY)?;
        Some(package::resolve_target(&self.main_path, &r.target))
    }

    fn glossary_doc(&self) -> Option<(String, XmlDoc)> {
        let path = self.glossary_path()?;
        let doc = xml::parse(self.pkg.get(&path)?).ok()?;
        Some((path, doc))
    }

    /// The building blocks of the glossary document, in the order the file lists them.
    pub fn building_blocks(&self) -> Vec<BuildingBlock> {
        let Some((_, doc)) = self.glossary_doc() else { return Vec::new() };
        let Some(parts) = doc.root.child("w:docParts") else { return Vec::new() };
        parts
            .children_named("w:docPart")
            .enumerate()
            .filter_map(|(index, dp)| {
                let pr = dp.child("w:docPartPr")?;
                let name = val(pr, "w:name")?;
                let mut els = Vec::new();
                if let Some(b) = dp.child("w:docPartBody") {
                    body_elements(b, &mut els);
                }
                let text: String = els.iter().map(fragment::element_text).collect::<Vec<_>>().join(" ");
                let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let text = if text.chars().count() > 80 { text.chars().take(80).collect::<String>() + "\u{2026}" } else { text };
                let content_only = pr.child("w:behaviors").is_some_and(|b| b.children_named("w:behavior").any(|x| x.attr("w:val") == Some("content")));
                Some(BuildingBlock {
                    index,
                    name,
                    gallery: pr.child("w:category").and_then(|c| val(c, "w:gallery")).or_else(|| pr.child("w:types").and_then(|t| t.child("w:type")).and_then(|t| t.attr("w:val")).map(String::from)),
                    category: pr.child("w:category").and_then(|c| val(c, "w:name")),
                    description: val(pr, "w:description"),
                    guid: val(pr, "w:guid"),
                    text,
                    inline: content_only && els.len() == 1 && els[0].name == "w:p",
                })
            })
            .collect()
    }

    /// A building block as content to insert: its paragraphs and tables with the styles, lists, pictures and links they
    /// use, from the glossary document's own parts.
    pub fn building_block_fragment(&self, index: usize) -> Option<Fragment> {
        let (path, doc) = self.glossary_doc()?;
        let dp = doc.root.child("w:docParts")?.children_named("w:docPart").nth(index)?;
        let inline = self.building_blocks().into_iter().find(|b| b.index == index)?.inline;
        let mut els = Vec::new();
        body_elements(dp.child("w:docPartBody")?, &mut els);
        if els.is_empty() {
            return None;
        }
        let mut g = Gather::default();
        let mut kept = Vec::new();
        for mut e in els {
            if self.gather(&mut e, &path, &mut g) {
                kept.push(e);
            }
        }
        // The glossary's own styles and lists, for what the content uses.
        let part_of = |suffix: &str| -> Option<XmlDoc> {
            let rel = self.pkg.rels_for(&path).ok()?.into_iter().find(|r| r.rel_type.ends_with(suffix))?;
            xml::parse(self.pkg.get(&package::resolve_target(&path, &rel.target))?).ok()
        };
        let styles_doc = part_of("/styles");
        let mut style_els: Vec<Element> = Vec::new();
        if let Some(sd) = &styles_doc {
            let mut queue: Vec<String> = g.styles.iter().cloned().collect();
            let mut seen: Vec<String> = Vec::new();
            while let Some(id) = queue.pop() {
                if seen.contains(&id) {
                    continue;
                }
                seen.push(id.clone());
                let Some(s) = sd.root.children_named("w:style").find(|s| s.attr("w:styleId") == Some(id.as_str())) else { continue };
                for dep in ["w:basedOn", "w:link", "w:next"] {
                    if let Some(d) = s.child(dep).and_then(|d| d.attr("w:val")) {
                        queue.push(d.to_string());
                    }
                }
                fragment::walk(s, &mut |e| {
                    if e.name == "w:numId" {
                        if let Some(n) = e.attr("w:val").and_then(|v| v.parse::<i64>().ok()) {
                            g.nums.insert(n);
                        }
                    }
                });
                style_els.push(s.clone());
            }
            style_els.sort_by_key(|s| s.attr("w:styleId").unwrap_or_default().to_string());
        }
        let mut lists: Vec<Element> = Vec::new();
        if let Some(nd) = part_of("/numbering") {
            let mut nums = Vec::new();
            for n in &g.nums {
                let Some(num) = nd.root.children_named("w:num").find(|x| x.attr("w:numId") == Some(n.to_string().as_str())) else { continue };
                if let Some(abs_id) = num.child("w:abstractNumId").and_then(|a| a.attr("w:val")) {
                    if let Some(abs) = nd.root.children_named("w:abstractNum").find(|a| a.attr("w:abstractNumId") == Some(abs_id)) {
                        if !lists.iter().any(|a| a.attr("w:abstractNumId") == Some(abs_id)) {
                            lists.push(abs.clone());
                        }
                    }
                }
                nums.push(num.clone());
            }
            lists.extend(nums);
        } else {
            // Without the glossary's lists the numbering can't be kept: the paragraphs go in unnumbered.
            for e in kept.iter_mut() {
                fragment::walk_mut(e, &mut |x| x.children.retain(|c| !matches!(c, Node::Element(n) if n.name == "w:numPr")));
            }
        }
        let source_ns: Vec<(String, String)> = doc.root.attrs.iter().filter_map(|(k, v)| k.strip_prefix("xmlns:").map(|p| (p.to_string(), v.to_string()))).collect();
        let mut frag = Fragment {
            version: 1,
            body: fragment::join_xml(&kept),
            styles: fragment::join_xml(&style_els),
            numbering: fragment::join_xml(&lists),
            media: g.media,
            links: g.links,
            notes: g.notes,
            block: !(inline && kept.len() == 1),
            reuse_lists: false,
            defaults: Some(FragDefaults { font: None, size: None }),
            warnings: g.warnings,
            ..Default::default()
        };
        frag.set_namespaces(&source_ns);
        Some(frag)
    }

    /// Insert a building block of the glossary at a position (by its index, else its guid, else its name).
    pub(super) fn op_insert_building_block(&mut self, at: Pos, index: Option<usize>, guid: Option<&str>, name: Option<&str>) -> Result<OpResult, EditError> {
        let blocks = self.building_blocks();
        let pick = match (index, guid, name) {
            (Some(i), _, _) => blocks.iter().find(|b| b.index == i),
            (None, Some(g), _) => blocks.iter().find(|b| b.guid.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(g))),
            (None, None, Some(n)) => blocks.iter().find(|b| b.name.eq_ignore_ascii_case(n)),
            _ => None,
        }
        .ok_or_else(|| EditError::Invalid("There is no such building block in this document.".into()))?;
        let frag = self.building_block_fragment(pick.index).ok_or_else(|| EditError::Invalid("That building block has nothing to insert.".into()))?;
        self.op_insert_fragment(at, &frag, fragment::PasteMode::Keep)
    }

    /// The content controls that came from a gallery, in order.
    pub fn doc_part_controls(&self) -> Vec<DocPartControl> {
        fn walk<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut Vec<DocPartControl>) {
            for b in blocks {
                if let Block::Container(c) = b {
                    if let Some(g) = c.shell.find("w:docPartGallery") {
                        let mut first = None;
                        walk_paragraphs(&c.blocks, &mut |p| {
                            if first.is_none() {
                                first = Some(p.id);
                            }
                        });
                        if let (Some(p), Some(gallery)) = (first, g.attr("w:val")) {
                            out.push(DocPartControl {
                                p,
                                gallery: gallery.to_string(),
                                category: c.shell.find("w:docPartCategory").and_then(|x| x.attr("w:val")).map(String::from),
                                title: c.shell.find("w:alias").and_then(|x| x.attr("w:val")).map(String::from),
                                unique: c.shell.find("w:docPartUnique").is_some(),
                            });
                        }
                    }
                    walk(&c.blocks, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.body, &mut out);
        out
    }
}
