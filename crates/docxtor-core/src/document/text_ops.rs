//! Editing text: inserting, deleting, splitting and formatting.

use super::*;

impl Document {
    pub(super) fn op_insert_text(&mut self, at: Pos, text: &str, format: Option<&RunPatch>) -> Result<OpResult, EditError> {
        // Typing right after choosing a character style: find it by name (or add Word's definition).
        let named;
        let format = match format {
            Some(f) if f.style.as_deref().is_some_and(|s| !s.is_empty()) => {
                let mut f = f.clone();
                f.style = Some(self.resolve_char_style(f.style.as_deref().unwrap_or_default())?);
                named = f;
                Some(&named)
            }
            other => other,
        };
        let inherited = self.inherited_run(self.para_style_of(at.p.0).as_deref());
        let char_styles = self.styles.clone();
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let has_text_runs = p.len16() > 0;
        let mut rpr = None;
        if let Some(patch) = format {
            let mut base = edit::rpr_at(&p.inlines, off).map(strip_link_style).or_else(|| Self::mark_rpr(p)).unwrap_or_else(|| Element::new("w:rPr"));
            let inh = match base.child("w:rStyle").and_then(|s| s.attr("w:val")) {
                Some(cs) => inherited.overlay(&char_styles.char_style(cs)),
                None => inherited.clone(),
            };
            patch.apply(&mut base, &inh);
            rpr = Some(base);
        } else if !has_text_runs {
            rpr = Self::mark_rpr(p);
        }
        edit::insert_text(&mut p.inlines, off, text, rpr);
        let len = text.replace('\r', "").encode_utf16().count();
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + len)), ..Default::default() })
    }

    pub(super) fn op_delete(&mut self, from: Pos, to: Pos) -> Result<OpResult, EditError> {
        let (from, to) = self.ordered(from, to)?;
        self.touch(from.p.0);
        self.touch(to.p.0);
        if from.p == to.p {
            let p = self.para_mut(from.p.0)?;
            let len = p.len16();
            edit::delete_range(&mut p.inlines, from.o.min(len), to.o.min(len));
            return Ok(OpResult { caret: Some(from), ..Default::default() });
        }
        let (p1, p2) = (from.p.0, to.p.0);
        let (mut list, i1) = self.body.parent_mut(p1).ok_or(EditError::NotFound(p1))?;
        let at2 = list.iter().position(|b| b.id() == p2);
        if let Some(i2) = at2 {
            if let Block::Para(b) = &mut list[i2] {
                let l = b.len16();
                edit::delete_range(&mut b.inlines, 0, to.o.min(l));
            }
            let removed: Vec<Block> = list.drain(i1 + 1..=i2);
            let Some(Block::Para(b)) = removed.into_iter().last() else { return Err(EditError::NotParagraph(p2)) };
            let Block::Para(a) = &mut list[i1] else { return Err(EditError::NotParagraph(p1)) };
            let l = a.len16();
            edit::delete_range(&mut a.inlines, from.o.min(l), l);
            let keep_b_props = a.len16() == 0 && from.o == 0 && b.len16() > 0;
            a.inlines.extend(b.inlines);
            edit::cleanup(&mut a.inlines);
            // Joining paragraphs deletes the section break between them: the later paragraph's section
            // properties (if it ends a section) govern the merged section.
            if keep_b_props {
                // Deleting a whole paragraph's text into the next: keep the surviving text's paragraph props.
                a.ppr = b.ppr.clone();
            } else {
                let b_sect = b.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).cloned();
                if let Some(pp) = a.ppr.as_mut() {
                    pp.remove_children("w:sectPr");
                }
                if let Some(sect) = b_sect {
                    put_ppr(a.ppr.get_or_insert_with(|| Element::new("w:pPr")), sect);
                }
            }
        } else {
            // Selection spans different containers (e.g. body into a table): clear text only.
            let order = paragraph_order(&self.body);
            let a = order.iter().position(|x| *x == p1).unwrap();
            let b = order.iter().position(|x| *x == p2).unwrap();
            for (k, pid) in order[a..=b].iter().enumerate() {
                self.touch(*pid);
                let p = self.para_mut(*pid)?;
                let len = p.len16();
                let s = if k == 0 { from.o.min(len) } else { 0 };
                let e = if a + k == b { to.o.min(len) } else { len };
                edit::delete_range(&mut p.inlines, s, e);
            }
        }
        Ok(OpResult { caret: Some(from), ..Default::default() })
    }

    /// Whether a paragraph is a list item (directly or through its style).
    pub(super) fn is_list_item(&self, pid: NodeId) -> bool {
        let Some(p) = find_para(&self.body, pid) else { return false };
        let direct = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
        let (style, _) = self.styles.para_style(direct.style.as_deref());
        style.overlay(&direct).num_id.is_some_and(|n| n > 0)
    }

    /// Take a paragraph out of its list, overriding numbering that comes from its style.
    pub(super) fn remove_list(&mut self, pid: NodeId) -> Result<(), EditError> {
        self.touch(pid);
        let p = self.para_mut(pid)?;
        let pp = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
        pp.remove_children("w:numPr");
        if pp.child("w:pStyle").and_then(|s| s.attr("w:val")) == Some("ListParagraph") {
            pp.remove_children("w:pStyle");
        }
        if self.is_list_item(pid) {
            let p = self.para_mut(pid)?;
            // numId 0 explicitly cancels numbering inherited from the paragraph style.
            ParaPatch { num: Some((0, 0)), ..Default::default() }.apply(p.ppr.get_or_insert_with(|| Element::new("w:pPr")));
        }
        Ok(())
    }

    pub(super) fn op_split(&mut self, at: Pos) -> Result<OpResult, EditError> {
        // Enter on an empty list item leaves the list instead of adding an item.
        if self.para_ref(at.p.0)?.len16() == 0 && self.is_list_item(at.p.0) {
            self.remove_list(at.p.0)?;
            return Ok(OpResult { caret: Some(at), ..Default::default() });
        }
        self.split_paragraph(at)
    }

    /// Split a paragraph at a position; the second half keeps any section break the paragraph carried.
    pub(super) fn split_paragraph(&mut self, at: Pos) -> Result<OpResult, EditError> {
        let style = self.para_style_of(at.p.0);
        let next_style = style.as_deref().and_then(|s| self.styles.get(s)).and_then(|s| s.next.clone()).filter(|n| Some(n) != style.as_ref());
        // Enter in a checked item starts an unchecked one.
        let checked_level = self.list_of(at.p.0).filter(|(n, l)| self.numbering.checkbox_at(*n, *l) == Some(true)).map(|(_, l)| l);
        let unchecked = if checked_level.is_some() { self.numbering.shared_num("checklist") } else { None };
        self.touch(at.p.0);
        let new_id = self.ids.fresh();
        let (mut list, i) = self.body.parent_mut(at.p.0).ok_or(EditError::NotFound(at.p.0))?;
        let Block::Para(p) = &mut list[i] else { return Err(EditError::NotParagraph(at.p.0)) };
        let len = p.len16();
        let off = at.o.min(len);
        let tail_rpr = edit::rpr_at(&p.inlines, off).map(strip_link_style);
        let (left, right) = edit::partition(std::mem::take(&mut p.inlines), off);
        p.inlines = left;
        let mut np = Paragraph::new(new_id);
        np.attrs = p.attrs.iter().filter(|(k, _)| !k.starts_with("w14:")).cloned().collect();
        np.ppr = p.ppr.clone();
        np.inlines = right;
        if let Some(pp) = p.ppr.as_mut() {
            pp.remove_children("w:sectPr");
        }
        if off == len {
            if let (Some(ns), Some(pp)) = (&next_style, np.ppr.as_mut()) {
                put_ppr(pp, Element::new("w:pStyle").with_attr("w:val", ns));
                pp.remove_children("w:numPr");
            }
        }
        // Carry the formatting at the split point onto the new paragraph's mark so typing continues it.
        if np.len16() == 0 && next_style.is_none() {
            if let Some(r) = tail_rpr.filter(|r| !r.children.is_empty()) {
                let pp = np.ppr.get_or_insert_with(|| Element::new("w:pPr"));
                put_ppr(pp, Element { name: "w:rPr".into(), ..r });
            }
        } else if let Some(pp) = np.ppr.as_mut() {
            if next_style.is_some() && off == len {
                pp.remove_children("w:rPr");
            }
        }
        if let (Some(level), Some(num)) = (checked_level, unchecked) {
            ParaPatch { num: Some((num, level)), ..Default::default() }.apply(np.ppr.get_or_insert_with(|| Element::new("w:pPr")));
            let pp = np.ppr.as_mut().unwrap();
            if let Some(mark) = pp.child_mut("w:rPr") {
                mark.remove_children("w:strike");
            }
            if pp.child("w:rPr").is_some_and(|r| r.children.is_empty()) {
                pp.remove_children("w:rPr");
            }
            let n = np.len16();
            edit::format_range(&mut np.inlines, 0, n, &mut |run| {
                if let Some(r) = run.rpr.as_mut() {
                    r.remove_children("w:strike");
                }
                if run.rpr.as_ref().is_some_and(|r| r.children.is_empty()) {
                    run.rpr = None;
                }
            });
        }
        list.insert(i + 1, Block::Para(np));
        if top_level_of(&self.body, new_id) == Some(new_id) {
            self.changed.insert(new_id);
        }
        Ok(OpResult { caret: Some(Pos::new(new_id, 0)), created: vec![new_id], count: None })
    }

    pub(super) fn op_format_text(&mut self, from: Pos, to: Pos, patch: &RunPatch) -> Result<OpResult, EditError> {
        let (from, to) = self.ordered(from, to)?;
        let mut patch = patch.clone();
        // A character style is found by id or name (Word's built-in ones are added); a linked paragraph style
        // applies as its character style.
        if let Some(s) = patch.style.clone().filter(|s| !s.is_empty()) {
            patch.style = Some(self.resolve_char_style(&s)?);
        }
        // Runs that are fitted together share one id.
        if patch.ext.fit_text.is_some_and(|w| w > 0) && patch.ext.fit_text_id.is_none() {
            patch.ext.fit_text_id = Some(crate::charfmt::next_fit_id());
        }
        // Runs that are combined or set upright together share one id, one past the largest the file already uses.
        if let Some(l) = patch.ext.ea.ea_layout.as_mut().filter(|l| l.id.is_none() && (l.combine == Some(true) || l.vert == Some(true) || l.vert_compress == Some(true))) {
            let mut max = 0i64;
            walk_paragraphs(&self.body, &mut |p| {
                walk_runs(&p.inlines, &mut 0, &mut |r, _, _| {
                    max = max.max(r.rpr.as_ref().and_then(|x| x.child("w:eastAsianLayout")).and_then(|e| e.attr("w:id")).and_then(|v| v.parse().ok()).unwrap_or(0));
                });
                max = max.max(p.ppr.as_ref().and_then(|x| x.child("w:rPr")).and_then(|x| x.child("w:eastAsianLayout")).and_then(|e| e.attr("w:id")).and_then(|v| v.parse().ok()).unwrap_or(0));
            });
            l.id = Some(max + 1);
        }
        let patch = &patch;
        let order = paragraph_order(&self.body);
        let a = order.iter().position(|x| *x == from.p.0).unwrap();
        let b = order.iter().position(|x| *x == to.p.0).unwrap();
        let styles = self.styles.clone();
        for (k, pid) in order[a..=b].iter().enumerate() {
            let inherited = self.inherited_run(self.para_style_of(*pid).as_deref());
            self.touch(*pid);
            let p = self.para_mut(*pid)?;
            let len = p.len16();
            let s = if k == 0 { from.o.min(len) } else { 0 };
            let e = if a + k == b { to.o.min(len) } else { len };
            edit::format_range(&mut p.inlines, s, e, &mut |run| {
                let rpr = run.rpr.get_or_insert_with(|| Element::new("w:rPr"));
                let inh = match rpr.child("w:rStyle").and_then(|s| s.attr("w:val")) {
                    Some(cs) => inherited.overlay(&styles.char_style(cs)),
                    None => inherited.clone(),
                };
                patch.apply(rpr, &inh);
            });
            // Whole-paragraph formatting also updates the paragraph mark (affects empty lines / new text).
            if s == 0 && e == len {
                let pp = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
                let mut mark = pp.child("w:rPr").cloned().unwrap_or_else(|| Element::new("w:rPr"));
                // Fitting text to a width is for the text, not the mark after it.
                let mut for_mark = patch.clone();
                for_mark.ext.fit_text = None;
                for_mark.apply(&mut mark, &inherited);
                if mark.children.is_empty() {
                    pp.remove_children("w:rPr");
                } else {
                    put_ppr(pp, mark);
                }
            }
        }
        Ok(OpResult::default())
    }

    /// Formatting at a position (for toolbar state).
    pub fn format_at(&self, at: Pos) -> Option<FormatState> {
        let (blocks, part) = self.locate(at.p.0);
        let p = find_para(blocks, at.p.0)?;
        let labels = &self.last_labels;
        let mut ctx = self.render_ctx(part, labels);
        let rp = ctx.para(p);
        let seg = rp
            .segs
            .iter()
            .rev()
            .find(|s| s.o < at.o || (at.o == 0 && s.o == 0))
            .or(rp.segs.first());
        let run = seg.map(|s| s.f.clone()).unwrap_or_else(|| rp.mark.clone());
        let list = rp.list.as_ref().map(|l| match (l.checklist, l.bullet) {
            (true, _) => FormatList::Checklist,
            (false, true) => FormatList::Bullet,
            _ => FormatList::Decimal,
        });
        // A checked item is the same style as an unchecked one: both are "checklist".
        let list_style = rp
            .list
            .as_ref()
            .and_then(|l| self.numbering.preset_of(l.num_id))
            .map(|p| if p == "checklist_checked" { "checklist" } else { p }.to_string());
        let checked = rp.list.as_ref().filter(|l| l.checklist).map(|l| l.checked);
        let in_table = !blocks.into_iter().any(|b| b.id() == at.p.0) && self.in_table(at.p.0);
        let section = self.section_of(at.p.0);
        let char_style = charfmt::char_style_at(p, at.o);
        let (list_num, list_level, list_value) = (rp.list.as_ref().map(|l| l.num_id), rp.list.as_ref().map(|l| l.level), rp.list.as_ref().map(|l| l.value));
        Some(FormatState { run, para: rp.fmt.clone(), style: rp.style.clone(), char_style, list, list_style, checked, list_num, list_level, list_value, link: seg.and_then(|s| s.link.clone()), in_table, section })
    }

    /// Plain text of a range (for clipboard).
    pub fn text_range(&self, from: Pos, to: Pos) -> String {
        let body = self.locate(from.p.0).0;
        let order = paragraph_order(body);
        let (Some(a), Some(b)) = (order.iter().position(|x| *x == from.p.0), order.iter().position(|x| *x == to.p.0)) else {
            return String::new();
        };
        let (a, b, from, to) = if (a, from.o) <= (b, to.o) { (a, b, from, to) } else { (b, a, to, from) };
        let mut out = Vec::new();
        for (i, pid) in order[a..=b].iter().enumerate() {
            let Some(p) = find_para(body, *pid) else { continue };
            let t: Vec<u16> = p.text().encode_utf16().collect();
            let s = if i == 0 { from.o.min(t.len()) } else { 0 };
            let e = if a + i == b { to.o.min(t.len()) } else { t.len() };
            // Objects drop out of plain text; an equation stays as its LaTeX in dollar signs, a phonetic guide as its base text.
            let mut line = String::new();
            let guides = self.guides_in(*pid);
            let mut from_u = s.min(e);
            for o in s.min(e)..e {
                if t[o] == OBJ as u16 {
                    line.push_str(&String::from_utf16_lossy(&t[from_u..o]));
                    from_u = o + 1;
                    if let Some(eq) = crate::math::at(&p.inlines, o) {
                        let latex = crate::math::omml_to_latex(eq);
                        line.push_str(&if eq.name == "m:oMathPara" { format!("$${latex}$$") } else { format!("${latex}$") });
                    } else if let Some((_, g)) = guides.iter().find(|(at, _)| *at == o) {
                        line.push_str(&g.base);
                    }
                }
            }
            line.push_str(&String::from_utf16_lossy(&t[from_u..e]));
            out.push(line.replace(OBJ, ""));
        }
        out.join("\n")
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn op_set_paragraph_text(&mut self, id: NodeId, md: &str) -> Result<OpResult, EditError> {
        let base = {
            let p = self.para_ref(id)?;
            edit::rpr_at(&p.inlines, 1).or_else(|| Self::mark_rpr(p))
        };
        let base = base.map(|mut b| {
            for n in ["w:b", "w:bCs", "w:i", "w:iCs", "w:u", "w:strike", "w:rStyle"] {
                b.remove_children(n);
            }
            b
        });
        let spans = markdown::parse_inline(md.trim_end_matches('\n'));
        let inl = self.md_inlines(&spans, base.as_ref());
        self.touch(id);
        let p = self.para_mut(id)?;
        let mut kept: Vec<Inline> = p.inlines.drain(..).filter(|i| matches!(i, Inline::Opaque(e) if !crate::math::is_math(e)) || matches!(i, Inline::Run(r) if marks::is_watermark_run(r))).collect();
        kept.extend(inl);
        p.inlines = kept;
        let len = p.len16();
        Ok(OpResult { caret: Some(Pos::new(id, len)), ..Default::default() })
    }
}
