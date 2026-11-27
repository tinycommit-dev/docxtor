//! List ops: toggling, styles, restarting and continuing numbering, checkboxes and indentation.

use super::*;

impl Document {
    pub(super) fn op_toggle_list(&mut self, ids: &[Id], kind: ListKind) -> Result<OpResult, EditError> {
        if ids.is_empty() {
            return Ok(OpResult::default());
        }
        let labels = self.numbering.labels(&self.body, &self.styles);
        // Leaving a checklist drops the strikethrough that checked items carry.
        let was_checked: Vec<NodeId> = ids.iter().map(|i| i.0).filter(|id| labels.get(id).is_some_and(|l| l.checked)).collect();
        self.set_strike(&was_checked, false)?;
        let all_in_kind = ids.iter().all(|id| labels.get(&id.0).is_some_and(|l| l.bullet == (kind == ListKind::Bullet)));
        if all_in_kind {
            for id in ids {
                self.remove_list(id.0)?;
            }
            return Ok(OpResult::default());
        }
        // Continue the list directly above if it is the same kind; otherwise start a new one.
        let order = paragraph_order(&self.body);
        let first = order.iter().position(|x| *x == ids[0].0).ok_or(EditError::NotFound(ids[0].0))?;
        let prev = if first > 0 { labels.get(&order[first - 1]) } else { None };
        let num_id = match prev.filter(|l| !l.checklist && l.bullet == (kind == ListKind::Bullet)) {
            Some(l) => l.num_id,
            None => self.numbering.create_list(kind),
        };
        let list_style = self.styles.ensure_builtin("ListParagraph");
        for id in ids {
            let level = labels.get(&id.0).map(|l| l.level).unwrap_or(0);
            let cur_style = self.para_style_of(id.0);
            self.touch(id.0);
            let p = self.para_mut(id.0)?;
            let pp = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
            ParaPatch { num: Some((num_id, level)), ..Default::default() }.apply(pp);
            if cur_style.is_none() || cur_style.as_deref() == Some("Normal") {
                if let Some(ls) = &list_style {
                    put_ppr(pp, Element::new("w:pStyle").with_attr("w:val", ls));
                }
            }
            // A direct indent would fight the list's own indentation.
            pp.remove_children("w:ind");
        }
        Ok(OpResult::default())
    }

    /// The list a paragraph is in, directly or through its style: `(numId, level)`.
    pub(super) fn list_of(&self, pid: NodeId) -> Option<(i64, i64)> {
        let p = find_para(&self.body, pid)?;
        let direct = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
        let (style, _) = self.styles.para_style(direct.style.as_deref());
        let f = style.overlay(&direct);
        let num = f.num_id.filter(|n| *n > 0)?;
        Some((num, f.ilvl.unwrap_or(0).clamp(0, 8)))
    }

    /// Put a paragraph in a list at a level.
    pub(super) fn set_num(&mut self, pid: NodeId, num: i64, level: i64) -> Result<(), EditError> {
        self.touch(pid);
        let p = self.para_mut(pid)?;
        ParaPatch { num: Some((num, level)), ..Default::default() }.apply(p.ppr.get_or_insert_with(|| Element::new("w:pPr")));
        Ok(())
    }

    /// Strike through (or restore) all the text of paragraphs; the paragraph mark is left alone so list labels stay clean.
    pub(super) fn set_strike(&mut self, ids: &[NodeId], on: bool) -> Result<(), EditError> {
        if ids.is_empty() {
            return Ok(());
        }
        let styles = self.styles.clone();
        let patch = RunPatch { strike: Some(on), ..Default::default() };
        for pid in ids {
            let inherited = self.inherited_run(self.para_style_of(*pid).as_deref());
            self.touch(*pid);
            let p = self.para_mut(*pid)?;
            let len = p.len16();
            edit::format_range(&mut p.inlines, 0, len, &mut |run| {
                let rpr = run.rpr.get_or_insert_with(|| Element::new("w:rPr"));
                let inh = match rpr.child("w:rStyle").and_then(|s| s.attr("w:val")) {
                    Some(cs) => inherited.overlay(&styles.char_style(cs)),
                    None => inherited.clone(),
                };
                patch.apply(rpr, &inh);
                if rpr.children.is_empty() {
                    run.rpr = None;
                }
            });
        }
        Ok(())
    }

    pub(super) fn op_set_list_style(&mut self, ids: &[Id], style: &str) -> Result<OpResult, EditError> {
        if style != "none" && !numbering::is_list_style(style) {
            return Err(EditError::Invalid(format!(
                "unknown list style '{style}'; use one of: none, {}",
                numbering::LIST_STYLES.join(", ")
            )));
        }
        for id in ids {
            self.para_ref(id.0)?;
        }
        if ids.is_empty() {
            return Ok(OpResult::default());
        }
        let labels = self.numbering.labels(&self.body, &self.styles);
        // Leaving a checklist drops the strikethrough that checked items carry.
        let leaving_checklist = |id: &NodeId| style != "checklist" && labels.get(id).is_some_and(|l| l.checked);
        if style == "none" {
            let was_checked: Vec<NodeId> = ids.iter().map(|i| i.0).filter(leaving_checklist).collect();
            self.set_strike(&was_checked, false)?;
            for id in ids {
                if labels.contains_key(&id.0) {
                    self.remove_list(id.0)?;
                }
            }
            return Ok(OpResult::default());
        }
        let numbered = numbering::is_numbered_style(style);

        // Lists the targets already belong to: the whole list changes style.
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
            let current = self.numbering.preset_of(x);
            // Already this style (a checked checklist counts as a checklist).
            if current == Some(style) || (style == "checklist" && current == Some("checklist_checked")) {
                continue;
            }
            let y = if numbered {
                let starts = self.numbering.start_overrides(x);
                self.numbering.fresh_num(style, &starts)
            } else {
                self.numbering.shared_num(style)
            };
            remap.insert(x, y.ok_or_else(|| EditError::Invalid(format!("unknown list style '{style}'")))?);
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
            let was_checked: Vec<NodeId> = moved.iter().map(|m| m.0).filter(leaving_checklist).collect();
            for (pid, y, level) in moved {
                self.set_num(pid, y, level)?;
            }
            self.set_strike(&was_checked, false)?;
        }

        // Paragraphs that aren't in a list yet start one.
        let fresh: Vec<NodeId> = ids.iter().map(|i| i.0).filter(|id| !labels.contains_key(id)).collect();
        if !fresh.is_empty() {
            let num = if numbered { self.numbering.fresh_num(style, &[]) } else { self.numbering.shared_num(style) };
            let num = num.ok_or_else(|| EditError::Invalid(format!("unknown list style '{style}'")))?;
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
                // A direct indent would fight the list's own indentation.
                pp.remove_children("w:ind");
            }
        }
        Ok(OpResult::default())
    }

    /// The level a paragraph that is not in a list would take if it joined one: its `ilvl`, or 0.
    pub(super) fn list_level_hint(&self, pid: NodeId) -> i64 {
        let Some(p) = find_para(&self.body, pid) else { return 0 };
        let direct = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
        let (style, _) = self.styles.para_style(direct.style.as_deref());
        style.overlay(&direct).ilvl.unwrap_or(0).clamp(0, 8)
    }

    pub(super) fn op_restart_numbering(&mut self, id: NodeId, start: i64) -> Result<OpResult, EditError> {
        self.para_ref(id)?;
        let labels = self.numbering.labels(&self.body, &self.styles);
        let here = labels.get(&id).ok_or_else(|| EditError::Invalid("that paragraph isn't in a list".into()))?;
        let (x, level) = (here.num_id, here.level);
        let order = paragraph_order(&self.body);
        let at = order.iter().position(|p| *p == id).ok_or(EditError::NotFound(id))?;
        let moved: Vec<(NodeId, i64)> = order[at..].iter().filter_map(|p| labels.get(p).filter(|l| l.num_id == x).map(|l| (*p, l.level))).collect();
        let y = self.numbering.restarted_num(x, level, start).ok_or_else(|| EditError::Invalid("that list can't be restarted".into()))?;
        for (pid, lvl) in moved {
            self.set_num(pid, y, lvl)?;
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_continue_numbering(&mut self, id: NodeId) -> Result<OpResult, EditError> {
        self.para_ref(id)?;
        let labels = self.numbering.labels(&self.body, &self.styles);
        let here = labels.get(&id).ok_or_else(|| EditError::Invalid("that paragraph isn't in a list".into()))?;
        let x = here.num_id;
        let order = paragraph_order(&self.body);
        let at = order.iter().position(|p| *p == id).ok_or(EditError::NotFound(id))?;
        let earlier: Vec<i64> = order[..at].iter().rev().filter_map(|p| labels.get(p).map(|l| l.num_id)).filter(|n| *n != x).collect();
        // The closest earlier list on the same definition; failing that, the same Docxtor preset.
        let abs = self.numbering.abstract_of(x);
        let preset = self.numbering.preset_of(x);
        let target = earlier
            .iter()
            .find(|n| abs.is_some() && self.numbering.abstract_of(**n) == abs)
            .or_else(|| earlier.iter().find(|n| preset.is_some() && self.numbering.preset_of(**n) == preset))
            .copied()
            .ok_or_else(|| EditError::Invalid("there's no earlier list to continue".into()))?;
        let moved: Vec<(NodeId, i64)> = order[at..].iter().filter_map(|p| labels.get(p).filter(|l| l.num_id == x).map(|l| (*p, l.level))).collect();
        for (pid, lvl) in moved {
            self.set_num(pid, target, lvl)?;
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_set_checked(&mut self, ids: &[Id], checked: bool) -> Result<OpResult, EditError> {
        for id in ids {
            self.para_ref(id.0)?;
        }
        if ids.is_empty() {
            return Ok(OpResult::default());
        }
        let labels = self.numbering.labels(&self.body, &self.styles);
        let mut items = Vec::new();
        for id in ids {
            match labels.get(&id.0).filter(|l| l.checklist) {
                Some(l) => items.push((id.0, l.level)),
                None => return Err(EditError::Invalid("that item isn't a checklist item".into())),
            }
        }
        let num = self.numbering.shared_num(if checked { "checklist_checked" } else { "checklist" }).ok_or_else(|| EditError::Invalid("can't make a checklist".into()))?;
        let mut pids = Vec::new();
        for (pid, level) in items {
            self.set_num(pid, num, level)?;
            pids.push(pid);
        }
        self.set_strike(&pids, checked)?;
        Ok(OpResult::default())
    }

    pub(super) fn op_indent(&mut self, ids: &[Id], delta: i64) -> Result<OpResult, EditError> {
        let labels = self.numbering.labels(&self.body, &self.styles);
        for id in ids {
            let fmt = {
                let p = self.para_ref(id.0)?;
                let labels_ref = &labels;
                let ctx = self.render_ctx(&self.active_part, labels_ref);
                ctx.para_fmt(p).0
            };
            self.touch(id.0);
            let p = self.para_mut(id.0)?;
            let pp = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
            if let Some(l) = labels.get(&id.0) {
                let lvl = (l.level + delta).clamp(0, 8);
                ParaPatch { num: Some((l.num_id, lvl)), ..Default::default() }.apply(pp);
            } else {
                let left = (fmt.ind_left.unwrap_or(0) + 720 * delta).max(0);
                ParaPatch { ind_left: Some(left), ..Default::default() }.apply(pp);
            }
        }
        Ok(OpResult::default())
    }
}
