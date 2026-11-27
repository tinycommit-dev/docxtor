//! What `render_changed` compares before and after a batch to know what the client must draw again.
//!
//! The blocks an op touched are marked as the op runs. Some things change how other blocks, and every header, footer
//! and note, look without any op naming them: the document defaults, a style that is added or changed (or comes back
//! by undo), the theme fonts, the paragraph that ends a section, the stories themselves (their paragraph ids, undo
//! and redo put them back). None of these is tracked op by op: `RenderKey` holds them all, `render_changed`
//! compares the key of the last render with the key now, and this file is the one place that says what follows from
//! a difference. A plain keystroke changes none of it, so it still draws one block and no stories.

use super::*;
use crate::stories::StoryKind;
use crate::xml::Element;
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct RenderKey {
    /// Each section's properties with the paragraph that carries them (`None`: the end of the body).
    sections: Vec<(Option<NodeId>, Element)>,
    /// The page background and the settings part.
    page: Vec<Element>,
    /// The style sheet: defaults, every style (language is a default too).
    styles: Option<Element>,
    /// The theme fonts the style sheet resolves names against.
    theme: String,
    /// The stories (headers, footers, notes): which there are, their paragraph ids and how often each was edited.
    stories: Vec<(String, u64, u64)>,
}

/// What a change of key means for drawing.
#[derive(Debug, Default)]
pub(super) struct KeyChange {
    /// Everything that is drawn depends on it: defaults, theme.
    pub all: bool,
    /// Headers, footers and notes (and so the page setup) must be read again.
    pub stories: bool,
    /// Styles added, changed or removed, with the styles based on them.
    pub styles: HashSet<String>,
}

impl Document {
    pub(super) fn render_key(&self) -> RenderKey {
        let sections = self.section_refs().into_iter().map(|(id, s)| (id, s.cloned().unwrap_or_else(|| Element::new("w:sectPr")))).collect();
        let mut page: Vec<Element> = self.doc_xml.root.child("w:background").cloned().into_iter().collect();
        page.extend(self.settings.as_ref().map(|d| d.root.clone()));
        let stories = self.stories_key();
        RenderKey { sections, page, styles: self.styles.doc.as_ref().map(|d| d.root.clone()), theme: format!("{:?}", self.styles.theme), stories }
    }

    fn stories_key(&self) -> Vec<(String, u64, u64)> {
        self.stories
            .iter()
            .filter(|s| s.kind != StoryKind::TextBox)
            .map(|s| {
                // FNV over the paragraph ids, in order.
                let mut h: u64 = 0xcbf29ce484222325;
                walk_paragraphs(&s.blocks, &mut |p| {
                    h = (h ^ u64::from(p.id)).wrapping_mul(0x100000001b3);
                });
                (format!("{}#{}", s.part, s.note_id.as_deref().unwrap_or("")), h, s.gen)
            })
            .collect()
    }

    /// Whether the key now equals `last`, compared in place: a keystroke asks this, so it copies nothing.
    pub(super) fn key_matches(&self, last: &RenderKey) -> bool {
        let refs = self.section_refs();
        let empty = Element::new("w:sectPr");
        let page: Vec<&Element> = self.doc_xml.root.child("w:background").into_iter().chain(self.settings.as_ref().map(|d| &d.root)).collect();
        refs.len() == last.sections.len()
            && refs.iter().zip(&last.sections).all(|((id, s), (lid, ls))| id == lid && s.unwrap_or(&empty) == ls)
            && page.len() == last.page.len()
            && page.iter().zip(&last.page).all(|(a, b)| *a == b)
            && self.styles.doc.as_ref().map(|d| &d.root) == last.styles.as_ref()
            && last.theme == format!("{:?}", self.styles.theme)
            && self.stories_key() == last.stories
    }

    /// Whether what the kept per-block facts depend on moved between two keys: `(lists, notes)`. The list labels read
    /// the styles and the theme; the note numbers read the settings and the section properties. A change in a story
    /// only (typing in a footnote or a header) moves neither.
    pub(super) fn facts_moved(before: &RenderKey, now: &RenderKey) -> (bool, bool) {
        (before.styles != now.styles || before.theme != now.theme, before.sections != now.sections || before.page != now.page)
    }

    /// What differs between the key of the last render and the key now.
    pub(super) fn key_change(&self, before: &RenderKey, now: &RenderKey) -> KeyChange {
        let mut out = KeyChange::default();
        if before.sections != now.sections || before.page != now.page || before.stories != now.stories {
            out.stories = true;
        }
        if before.theme != now.theme {
            out.all = true;
        }
        if before.styles != now.styles {
            match (&before.styles, &now.styles) {
                (Some(a), Some(b)) => {
                    if a.child("w:docDefaults") != b.child("w:docDefaults") {
                        out.all = true;
                    }
                    let by_id = |r: &Element| -> HashMap<String, Element> { r.children_named("w:style").filter_map(|s| s.attr("w:styleId").map(|i| (i.to_string(), s.clone()))).collect() };
                    let (old, new) = (by_id(a), by_id(b));
                    for (id, s) in &new {
                        if old.get(id) != Some(s) {
                            out.styles.insert(id.clone());
                        }
                    }
                    for id in old.keys().filter(|id| !new.contains_key(*id)) {
                        out.styles.insert(id.clone());
                    }
                    // A style that changes changes the styles built on it.
                    let based: Vec<(String, String)> = new.iter().chain(old.iter()).filter_map(|(id, s)| s.child("w:basedOn").and_then(|e| e.attr("w:val")).map(|p| (id.clone(), p.to_string()))).collect();
                    loop {
                        let n = out.styles.len();
                        for (id, parent) in &based {
                            if out.styles.contains(parent) {
                                out.styles.insert(id.clone());
                            }
                        }
                        if out.styles.len() == n {
                            break;
                        }
                    }
                }
                _ => out.all = true,
            }
        }
        if out.all || !out.styles.is_empty() {
            out.stories = true;
        }
        out
    }

    /// Mark the body blocks that use any of these styles (a paragraph, run or table style, anywhere inside the block,
    /// text boxes included) as changed.
    pub(super) fn touch_blocks_using(&mut self, styles: &HashSet<String>) {
        if styles.is_empty() {
            return;
        }
        let needles: Vec<String> = styles.iter().map(|s| format!("\"{s}\"")).collect();
        let ids: Vec<NodeId> = self
            .body
            .iter()
            .filter(|b| {
                let xml = crate::model::write_block(b).to_xml();
                needles.iter().any(|n| xml.contains(n.as_str()))
            })
            .map(|b| b.id())
            .collect();
        self.changed.extend(ids);
    }
}
