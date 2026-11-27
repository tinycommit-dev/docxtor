//! Lists a person defines: new bullets, number formats and multilevel lists, setting a list item's number,
//! and sorting paragraphs.

use super::*;
use crate::numbering::{ListDef, ListDefinition};
use crate::sort::{self, SortOptions};
use std::collections::HashSet;

/// The relationships of `numbering.xml` (picture bullets): id to the part each points at.
pub(super) fn numbering_rels(pkg: &Package, path: &str) -> HashMap<String, String> {
    pkg.rels_for(path).unwrap_or_default().into_iter().filter(|r| !r.external).map(|r| (r.id, package::resolve_target(path, &r.target))).collect()
}

impl Document {
    /// The lists the main text uses that Docxtor didn't make from a preset: a gallery's "In this document" entries.
    pub fn list_definitions(&self) -> Vec<ListDefinition> {
        let labels = self.numbering.labels(&self.body, &self.styles);
        let used: HashSet<i64> = labels.values().map(|l| l.num_id).collect();
        self.numbering.definitions_in_use(&used)
    }

    /// The definition of a list, to start the Define new list dialogs from.
    pub fn list_definition(&self, num_id: i64) -> Option<ListDef> {
        self.numbering.definition(num_id)
    }

    pub(super) fn op_define_list(&mut self, def: &ListDef, ids: &[Id]) -> Result<OpResult, EditError> {
        for id in ids {
            self.para_ref(id.0)?;
        }
        let mut def = def.clone();
        // Headings linked to levels: name them by style id, adding Word's built-in ones when the file lacks them.
        let mut links: Vec<(String, i64)> = Vec::new();
        for (i, l) in def.levels.iter_mut().enumerate() {
            match l.style.clone().filter(|s| !s.is_empty()) {
                Some(name) => {
                    let sid = self.resolve_style(&name)?;
                    l.style = Some(sid.clone());
                    links.push((sid, i as i64));
                }
                None => l.style = None,
            }
        }
        // Picture bullets: numbering.xml points at each picture with a relationship.
        let mut pictures = HashMap::new();
        for media in Numbering::picture_parts(&def) {
            if !self.pkg.has(&media) {
                return Err(EditError::Invalid(format!("picture {media} isn't in the document (add it with add_media)")));
            }
            let target = relative_target(&self.numbering_path, &media);
            let rid = self.pkg.add_rel(&self.numbering_path.clone(), package::REL_IMAGE, &target, false);
            self.numbering.rels.insert(rid.clone(), media.clone());
            self.numbering.touch();
            pictures.insert(media, rid);
        }
        let (_, num) = self.numbering.define(&def, &pictures).map_err(EditError::Invalid)?;
        for (sid, level) in links {
            self.styles.set_style_numbering(&sid, Some((num, level)));
        }
        let list_style = self.styles.ensure_builtin("ListParagraph");
        for id in ids {
            let level = self.list_level_hint(id.0);
            let cur_style = self.para_style_of(id.0);
            self.set_num(id.0, num, level)?;
            let p = self.para_mut(id.0)?;
            let pp = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
            if cur_style.is_none() || cur_style.as_deref() == Some("Normal") {
                if let Some(ls) = &list_style {
                    put_ppr(pp, Element::new("w:pStyle").with_attr("w:val", ls));
                }
            }
            // A direct indent would fight the list's own indentation.
            pp.remove_children("w:ind");
        }
        Ok(OpResult { count: Some(num as usize), ..Default::default() })
    }

    pub(super) fn op_apply_list(&mut self, ids: &[Id], num_id: i64) -> Result<OpResult, EditError> {
        for id in ids {
            self.para_ref(id.0)?;
        }
        let abs = self.numbering.abstract_of(num_id).ok_or_else(|| EditError::Invalid(format!("there is no list {num_id}")))?;
        if ids.is_empty() {
            return Ok(OpResult::default());
        }
        let numbered = !self.numbering.is_bullet(num_id);
        let labels = self.numbering.labels(&self.body, &self.styles);
        // The lists the paragraphs are in change to this definition, as a whole.
        let mut old: Vec<i64> = Vec::new();
        for id in ids {
            if let Some(l) = labels.get(&id.0) {
                if !old.contains(&l.num_id) {
                    old.push(l.num_id);
                }
            }
        }
        let mut remap: HashMap<i64, i64> = HashMap::new();
        for x in old {
            if self.numbering.abstract_of(x) == Some(abs) {
                continue;
            }
            let y = if numbered {
                let starts = self.numbering.start_overrides(x);
                self.numbering.fresh_num_on(abs, &starts)
            } else {
                self.numbering.shared_num_on(abs)
            };
            remap.insert(x, y);
        }
        if !remap.is_empty() {
            let mut moved: Vec<(NodeId, i64, i64)> = Vec::new();
            walk_paragraphs(&self.body, &mut |p| {
                if let Some(l) = labels.get(&p.id) {
                    if let Some(y) = remap.get(&l.num_id) {
                        moved.push((p.id, *y, l.level));
                    }
                }
            });
            // Checked items of a checklist are struck through; they aren't any more.
            let checked: Vec<NodeId> = moved.iter().map(|m| m.0).filter(|id| labels.get(id).is_some_and(|l| l.checked)).collect();
            for (pid, y, level) in moved {
                self.set_num(pid, y, level)?;
            }
            self.set_strike(&checked, false)?;
        }
        let fresh: Vec<NodeId> = ids.iter().map(|i| i.0).filter(|id| !labels.contains_key(id)).collect();
        if !fresh.is_empty() {
            let num = if numbered { self.numbering.fresh_num_on(abs, &[]) } else { self.numbering.shared_num_on(abs) };
            let list_style = self.styles.ensure_builtin("ListParagraph");
            for pid in fresh {
                let level = self.list_level_hint(pid);
                let cur_style = self.para_style_of(pid);
                self.set_num(pid, num, level)?;
                let p = self.para_mut(pid)?;
                let pp = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
                if cur_style.is_none() || cur_style.as_deref() == Some("Normal") {
                    if let Some(ls) = &list_style {
                        put_ppr(pp, Element::new("w:pStyle").with_attr("w:val", ls));
                    }
                }
                pp.remove_children("w:ind");
            }
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_set_numbering_value(&mut self, id: NodeId, value: i64, continue_previous: bool) -> Result<OpResult, EditError> {
        if value < 0 {
            return Err(EditError::Invalid("a list can't start below 0".into()));
        }
        if !continue_previous {
            return self.op_restart_numbering(id, value);
        }
        // Carry on the previous list (if this item had been split off it), then show the value: a value higher
        // than the next one skips numbers. Nothing to join is not an error.
        let _ = self.op_continue_numbering(id);
        let labels = self.numbering.labels(&self.body, &self.styles);
        let here = labels.get(&id).ok_or_else(|| EditError::Invalid("that paragraph isn't in a list".into()))?;
        if here.value != value {
            self.op_restart_numbering(id, value)?;
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_sort_paragraphs(&mut self, ids: &[Id], opts: &SortOptions) -> Result<OpResult, EditError> {
        for id in ids {
            self.para_ref(id.0)?;
        }
        if ids.len() < 2 {
            return Ok(OpResult::default());
        }
        // Looked at first: a sort with nothing to move copies nothing from an undo step.
        let slots = {
            let (places, _) = self.body.parent_of(ids[0].0).ok_or(EditError::NotFound(ids[0].0))?;
            let mut slots = Vec::new();
            for id in ids {
                let at = places.into_iter().position(|b| b.id() == id.0).ok_or_else(|| EditError::Invalid("the paragraphs to sort must be in the same place (not in different table cells)".into()))?;
                slots.push(at);
            }
            slots.sort_unstable();
            slots.dedup();
            slots
        };
        // A header line stays where it is.
        let movable: &[usize] = if opts.header { &slots[1..] } else { &slots[..] };
        if movable.len() < 2 {
            return Ok(OpResult::default());
        }
        let (mut list, _) = self.body.parent_mut(ids[0].0).ok_or(EditError::NotFound(ids[0].0))?;
        let lines: Vec<String> = movable
            .iter()
            .map(|&i| match &list[i] {
                Block::Para(p) => p.plain_text(),
                _ => String::new(),
            })
            .collect();
        let order = sort::order(&lines, opts);
        // A section break belongs to its place in the text, not to the paragraph that carries it.
        let mut sections: Vec<Option<Element>> = Vec::new();
        for &i in movable {
            let mut sect = None;
            if let Block::Para(p) = &mut list[i] {
                if let Some(pp) = p.ppr.as_mut() {
                    sect = pp.child("w:sectPr").cloned();
                    pp.remove_children("w:sectPr");
                }
            }
            sections.push(sect);
        }
        let blocks: Vec<Block> = movable.iter().map(|&i| list[i].clone()).collect();
        for (k, &i) in movable.iter().enumerate() {
            list[i] = blocks[order[k]].clone();
            if let (Some(sect), Block::Para(p)) = (sections[k].take(), &mut list[i]) {
                put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), sect);
            }
        }
        for id in ids {
            self.touch(id.0);
        }
        Ok(OpResult::default())
    }
}
