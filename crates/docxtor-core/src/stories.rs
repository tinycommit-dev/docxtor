//! Secondary stories: headers, footers and footnotes. Each is an editable block
//! list living in its own package part, addressed by the same paragraph ids as
//! the main body so every op works in them unchanged.

use crate::model::*;
use crate::package::{self, Package};
use crate::xml::{self, Element, Node, XmlDoc};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StoryKind {
    Header,
    Footer,
    Footnote,
    Endnote,
    /// The text of a shape or text box (see `shapes::story`). `part` is the part holding the shape; the text
    /// is saved inside the shape's own markup, not in a part of its own.
    TextBox,
}

#[derive(Debug, Clone)]
pub struct Story {
    pub kind: StoryKind,
    /// Package part holding the story (`word/header1.xml`, `word/footnotes.xml`).
    pub part: String,
    /// For footnotes: the `w:id` of the note inside the part.
    pub note_id: Option<String>,
    pub blocks: Vec<Block>,
    /// Changes on every edit; compared across undo to find what must be rewritten.
    pub gen: u64,
}

/// Parsed parts, kept so unmodelled content (namespaces, other notes) survives.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoryParts {
    pub parts: Vec<(String, XmlDoc)>,
}

impl StoryParts {
    pub fn get(&self, part: &str) -> Option<&XmlDoc> {
        self.parts.iter().find(|(p, _)| p == part).map(|(_, d)| d)
    }
    pub fn get_mut(&mut self, part: &str) -> Option<&mut XmlDoc> {
        self.parts.iter_mut().find(|(p, _)| p == part).map(|(_, d)| d)
    }
}

/// Load every header/footer referenced by a section, plus all footnotes.
pub fn load(pkg: &Package, main_path: &str, body: &[Block], ids: &mut IdGen) -> (Vec<Story>, StoryParts) {
    let mut stories = Vec::new();
    let mut parts = StoryParts::default();
    let mut sects: Vec<&Element> = Vec::new();
    collect_sect_prs(body, &mut sects);
    for s in sects {
        for (tag, kind) in [("w:headerReference", StoryKind::Header), ("w:footerReference", StoryKind::Footer)] {
            for r in s.children_named(tag) {
                let Some(rid) = r.attr("r:id") else { continue };
                let Some(rel) = pkg.resolve_rel(main_path, rid) else { continue };
                if parts.get(&rel.target).is_some() {
                    continue;
                }
                let Ok(doc) = pkg.xml(&rel.target) else { continue };
                let blocks = parse_blocks(&doc.root, ids);
                stories.push(Story { kind, part: rel.target.clone(), note_id: None, blocks, gen: 0 });
                parts.parts.push((rel.target, doc));
            }
        }
    }
    if let Some(rel) = pkg.rels_for(main_path).ok().and_then(|rels| rels.into_iter().find(|r| r.rel_type == package::REL_FOOTNOTES)) {
        let path = package::resolve_target(main_path, &rel.target);
        if let Ok(doc) = pkg.xml(&path) {
            for n in doc.root.children_named("w:footnote") {
                if n.attr("w:type").is_some_and(|t| t != "normal") {
                    continue; // separators
                }
                let Some(id) = n.attr("w:id") else { continue };
                let blocks = parse_blocks(n, ids);
                stories.push(Story { kind: StoryKind::Footnote, part: path.clone(), note_id: Some(id.to_string()), blocks, gen: 0 });
            }
            parts.parts.push((path, doc));
        }
    }
    if let Some(rel) = pkg.rels_for(main_path).ok().and_then(|rels| rels.into_iter().find(|r| r.rel_type == package::REL_ENDNOTES)) {
        let path = package::resolve_target(main_path, &rel.target);
        if parts.get(&path).is_none() {
            if let Ok(doc) = pkg.xml(&path) {
                for n in doc.root.children_named("w:endnote") {
                    if n.attr("w:type").is_some_and(|t| t != "normal") {
                        continue; // separators
                    }
                    let Some(id) = n.attr("w:id") else { continue };
                    let blocks = parse_blocks(n, ids);
                    stories.push(Story { kind: StoryKind::Endnote, part: path.clone(), note_id: Some(id.to_string()), blocks, gen: 0 });
                }
                parts.parts.push((path, doc));
            }
        }
    }
    (stories, parts)
}

pub fn collect_sect_prs<'a>(blocks: &'a [Block], out: &mut Vec<&'a Element>) {
    for b in blocks {
        match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => out.push(&o.el),
            Block::Para(p) => {
                if let Some(s) = p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")) {
                    out.push(s);
                }
            }
            Block::Container(c) => collect_sect_prs(&c.blocks, out),
            _ => {}
        }
    }
}

/// Write the given parts back from their stories.
pub fn flush(stories: &[Story], parts: &StoryParts, dirty: &BTreeSet<String>, pkg: &mut Package) {
    for part in dirty {
        let Some(doc) = parts.get(part) else {
            // A notes part whose notes were all undone: back to the separator notes it started with.
            match pkg.xml(part).map(|d| d.root.name) {
                Ok(n) if n == "w:footnotes" => pkg.set(part, footnotes_part().to_bytes()),
                Ok(n) if n == "w:endnotes" => pkg.set(part, endnotes_part().to_bytes()),
                _ => {}
            }
            continue;
        };
        let mut doc = doc.clone();
        for s in stories.iter().filter(|s| &s.part == part) {
            match s.kind {
                StoryKind::Header | StoryKind::Footer => {
                    doc.root.children.clear();
                    write_blocks(&s.blocks, &mut doc.root.children);
                }
                StoryKind::TextBox => {}
                StoryKind::Footnote | StoryKind::Endnote => {
                    let id = s.note_id.as_deref().unwrap_or_default();
                    let tag = if s.kind == StoryKind::Footnote { "w:footnote" } else { "w:endnote" };
                    if let Some(n) = doc.root.elements_mut().find(|e| e.name == tag && e.attr("w:id") == Some(id)) {
                        n.children.clear();
                        write_blocks(&s.blocks, &mut n.children);
                    }
                }
            }
        }
        crate::charfmt::declare_w14(&mut doc.root);
        pkg.set(part, doc.to_bytes());
    }
}

pub const NS_DECLS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#;

pub fn header_part(kind: StoryKind, style: &str) -> XmlDoc {
    let tag = if kind == StoryKind::Header { "w:hdr" } else { "w:ftr" };
    let src = format!(r#"<{tag} {NS_DECLS}><w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr></w:p></{tag}>"#);
    XmlDoc::new(xml::parse(src.as_bytes()).expect("valid header template").root)
}

pub fn footnotes_part() -> XmlDoc {
    let src = format!(
        r#"<w:footnotes {NS_DECLS}><w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote></w:footnotes>"#
    );
    XmlDoc::new(xml::parse(src.as_bytes()).expect("valid footnotes template").root)
}

/// The first paragraph of a new footnote: the note's own reference mark, then text.
pub fn footnote_paragraph(text_style: &str, ref_style: &str) -> Element {
    let src = format!(
        r#"<w:p xmlns:w="w"><w:pPr><w:pStyle w:val="{text_style}"/></w:pPr><w:r><w:rPr><w:rStyle w:val="{ref_style}"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r></w:p>"#
    );
    let mut el = xml::parse(src.as_bytes()).unwrap().root;
    el.attrs.clear();
    el
}

pub fn reference_run(ref_style: &str, id: &str) -> Run {
    let rpr = Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", ref_style));
    Run {
        attrs: vec![],
        t_attrs: None,
        rpr: Some(rpr),
        items: vec![RunItem::Object(Element::new("w:footnoteReference").with_attr("w:id", id))],
    }
}

/// Insert a header/footer reference into a sectPr, keeping references first.
pub fn add_reference(sect: &mut Element, tag: &'static str, variant: &str, rid: &str) {
    sect.children.retain(|n| !matches!(n, Node::Element(e) if e.name == tag && e.attr("w:type").unwrap_or("default") == variant));
    let pos = sect.children.iter().position(|n| matches!(n, Node::Element(e) if e.name != "w:headerReference" && e.name != "w:footerReference")).unwrap_or(sect.children.len());
    sect.children.insert(pos, Node::Element(Element::new(tag).with_attr("w:type", variant).with_attr("r:id", rid)));
}

/// A new `word/endnotes.xml`: the two separator notes, as Word writes them.
pub fn endnotes_part() -> XmlDoc {
    let src = format!(
        r#"<w:endnotes {NS_DECLS}><w:endnote w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:endnote><w:endnote w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:endnote></w:endnotes>"#
    );
    XmlDoc::new(xml::parse(src.as_bytes()).expect("valid endnotes template").root)
}

/// The first paragraph of a new endnote: the note's own reference mark, then text.
pub fn endnote_paragraph(text_style: &str, ref_style: &str) -> Element {
    let src = format!(
        r#"<w:p xmlns:w="w"><w:pPr><w:pStyle w:val="{text_style}"/></w:pPr><w:r><w:rPr><w:rStyle w:val="{ref_style}"/></w:rPr><w:endnoteRef/></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r></w:p>"#
    );
    let mut el = xml::parse(src.as_bytes()).unwrap().root;
    el.attrs.clear();
    el
}

/// A run holding a note reference (`w:footnoteReference` or `w:endnoteReference`) with the given character style.
pub fn note_reference_run(tag: &'static str, ref_style: &str, id: &str) -> Run {
    let rpr = Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", ref_style));
    Run { attrs: vec![], t_attrs: None, rpr: Some(rpr), items: vec![RunItem::Object(Element::new(tag).with_attr("w:id", id))] }
}
