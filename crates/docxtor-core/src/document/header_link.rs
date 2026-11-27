//! "Link to previous" for headers and footers. A section without a reference of its own shows the one
//! before it; unlinking gives it a real copy (a new part), linking drops that copy again.

use super::*;

impl Document {
    pub(super) fn op_link_header_footer(&mut self, kind: HeaderKind, variant: &str, section: Option<Id>, linked: bool) -> Result<OpResult, EditError> {
        if !matches!(variant, "default" | "first" | "even") {
            return Err(EditError::Invalid(format!("unsupported header/footer variant '{variant}' (use default, first or even)")));
        }
        let (tag, skind, stem, ct, rel_type, name) = match kind {
            HeaderKind::Header => ("w:headerReference", StoryKind::Header, "header", package::CT_HEADER, package::REL_HEADER, "header"),
            HeaderKind::Footer => ("w:footerReference", StoryKind::Footer, "footer", package::CT_FOOTER, package::REL_FOOTER, "footer"),
        };
        // This section's own reference and the part it shows from the sections before.
        let rid_of = |e: Option<&Element>| e.and_then(|e| e.children_named(tag).find(|r| r.attr("w:type").unwrap_or("default") == variant).and_then(|r| r.attr("r:id")).map(String::from));
        let (idx, own, inherited) = {
            let refs = self.section_refs();
            let idx = match section {
                Some(id) => {
                    self.para_ref(id.0)?;
                    self.section_of(id.0)
                }
                None => refs.len() - 1,
            };
            let own = rid_of(refs.get(idx).and_then(|r| r.1));
            let inherited = (0..idx).rev().find_map(|i| rid_of(refs[i].1));
            (idx, own, inherited)
        };
        let part_of = |this: &Self, rid: &str| this.pkg.resolve_rel(&this.main_path, rid).map(|r| r.target);
        let first_para = |blocks: &[Block]| paragraph_order(blocks).first().map(|id| Pos::new(*id, 0));
        if idx == 0 {
            return Err(EditError::Invalid(format!("The first section has no section before it to link its {name} to.")));
        }
        if linked {
            let Some(rid) = own else { return Ok(OpResult::default()) };
            let part = part_of(self, &rid);
            self.with_sect(section, &mut |s| {
                s.children.retain(|n| !matches!(n, Node::Element(e) if e.name == tag && e.attr("w:type").unwrap_or("default") == variant));
            })?;
            // Gone with the reference, unless another section still uses the same part.
            if let Some(part) = part {
                let used = self.section_refs().iter().any(|(_, e)| ["w:headerReference", "w:footerReference"].iter().any(|t| e.is_some_and(|e| e.children_named(t).any(|r| r.attr("r:id").and_then(|id| part_of(self, id)).as_deref() == Some(part.as_str())))));
                if !used {
                    self.stories.retain(|s| s.part != part);
                    self.story_parts.parts.retain(|(p, _)| *p != part);
                    self.hf_candidates.insert(part);
                }
            }
            self.stories_touched = true;
            return Ok(OpResult::default());
        }
        if let Some(rid) = own {
            let caret = part_of(self, &rid).and_then(|part| self.stories.iter().find(|s| s.part == part)).and_then(|s| first_para(&s.blocks));
            return Ok(OpResult { caret, ..Default::default() });
        }
        let Some(source) = inherited.and_then(|rid| part_of(self, &rid)) else {
            // Nothing before it to copy: an empty one.
            return self.add_header_footer_variant(kind, variant, section);
        };
        let Some(blocks_src) = self.stories.iter().find(|s| s.part == source).map(|s| s.blocks.clone()) else {
            return self.add_header_footer_variant(kind, variant, section);
        };
        // A real copy: the same root (namespaces), the story's current content and its relationships.
        let mut doc = self.story_parts.get(&source).cloned().unwrap_or_else(|| stories::header_part(skind, if skind == StoryKind::Header { "Header" } else { "Footer" }));
        doc.root.children.clear();
        write_blocks(&blocks_src, &mut doc.root.children);
        let part = self.free_part(stem);
        let mut blocks = parse_blocks(&doc.root, &mut self.ids);
        // Bookmark names are unique in a document, so the copy leaves them with the original.
        if marks::strip_bookmarks(&mut blocks) {
            doc.root.children.clear();
            write_blocks(&blocks, &mut doc.root.children);
        }
        self.pkg.set(&part, doc.to_bytes());
        self.pkg.ensure_override(&part, ct);
        if let Some(rels) = self.pkg.get(&package::rels_path(&source)).map(|d| d.to_vec()) {
            self.pkg.set(&package::rels_path(&part), rels);
        }
        let main = self.main_path.clone();
        let rid = self.pkg.add_rel(&main, rel_type, &relative_target(&main, &part), false);
        self.ensure_ns("r", NS_R);
        self.with_sect(section, &mut |s| stories::add_reference(s, tag, variant, &rid))?;
        let caret = first_para(&blocks);
        let created = blocks.iter().map(|b| b.id()).collect();
        self.story_gen += 1;
        self.hf_candidates.insert(part.clone());
        self.stories.push(Story { kind: skind, part: part.clone(), note_id: None, blocks, gen: self.story_gen });
        self.story_parts.parts.push((part, doc));
        self.stories_touched = true;
        Ok(OpResult { caret, created, ..Default::default() })
    }

    /// The header and footer parts some section still refers to.
    pub(super) fn referenced_hf_parts(&self) -> BTreeSet<String> {
        let mut used = BTreeSet::new();
        for (_, e) in self.section_refs() {
            for tag in ["w:headerReference", "w:footerReference"] {
                for r in e.into_iter().flat_map(|e| e.children_named(tag)) {
                    if let Some(rel) = r.attr("r:id").and_then(|id| self.pkg.resolve_rel(&self.main_path, id)) {
                        used.insert(rel.target);
                    }
                }
            }
        }
        // Whatever the body's shape, a section properties block anywhere in the main text counts: a header it names is never dropped.
        for b in self.main_body() {
            if let Block::Opaque(o) = b {
                if o.el.name == "w:sectPr" {
                    for tag in ["w:headerReference", "w:footerReference"] {
                        for r in o.el.children_named(tag) {
                            if let Some(rel) = r.attr("r:id").and_then(|id| self.pkg.resolve_rel(&self.main_path, id)) {
                                used.insert(rel.target);
                            }
                        }
                    }
                }
            }
        }
        used
    }

    /// The package as it should be saved: without header and footer parts that no section refers to any more
    /// (made and then undone, or let go by linking to the previous section). None when nothing needs to go.
    pub(super) fn pruned_package(&self) -> Option<Package> {
        if self.hf_candidates.is_empty() {
            return None;
        }
        let used = self.referenced_hf_parts();
        let orphans: Vec<&String> = self.hf_candidates.iter().filter(|p| !used.contains(*p) && self.pkg.has(p)).collect();
        if orphans.is_empty() {
            return None;
        }
        let mut pkg = self.pkg.clone();
        for part in orphans {
            pkg.remove_rels_to(&self.main_path, part);
            pkg.remove(&package::rels_path(part));
            pkg.remove(part);
            pkg.remove_override(part);
        }
        Some(pkg)
    }
}
