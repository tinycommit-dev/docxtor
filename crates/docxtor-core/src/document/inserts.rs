//! Inserting footnotes, horizontal lines, line breaks and links.

use super::*;

impl Document {
    pub(super) fn op_insert_footnote(&mut self, at: Pos, text: &str) -> Result<OpResult, EditError> {
        let off = at.o.min(self.para_ref(at.p.0)?.len16());
        let part = match self.story_parts.parts.iter().find(|(_, d)| d.root.name == "w:footnotes") {
            Some((p, _)) => p.clone(),
            None => {
                let part = self.free_part("footnotes").replace("footnotes1.xml", "footnotes.xml");
                let part = if self.pkg.has(&part) { self.free_part("footnotes") } else { part };
                let doc = stories::footnotes_part();
                self.pkg.set(&part, doc.to_bytes());
                self.pkg.ensure_override(&part, package::CT_FOOTNOTES);
                let main = self.main_path.clone();
                self.pkg.add_rel(&main, package::REL_FOOTNOTES, &relative_target(&main, &part), false);
                self.story_parts.parts.push((part.clone(), doc));
                part
            }
        };
        let text_style = self.styles.ensure_builtin("FootnoteText").unwrap_or_else(|| "FootnoteText".into());
        let ref_style = self.styles.ensure_builtin("FootnoteReference").unwrap_or_else(|| "FootnoteReference".into());
        let doc = self.story_parts.get_mut(&part).expect("footnotes part");
        let id = doc.root.children_named("w:footnote").filter_map(|n| n.attr("w:id")?.parse::<i64>().ok()).max().unwrap_or(0).max(0) + 1;
        let id = id.to_string();
        let note = Element::new("w:footnote").with_attr("w:id", &id).with_child(stories::footnote_paragraph(&text_style, &ref_style));
        let mut blocks = parse_blocks(&note, &mut self.ids);
        doc.root.children.push(Node::Element(note));
        if !text.is_empty() {
            if let Some(Block::Para(p)) = blocks.first_mut() {
                let end = p.len16();
                edit::insert_text(&mut p.inlines, end, text, None);
            }
        }
        let caret = match blocks.first() {
            Some(Block::Para(p)) => Some(Pos::new(p.id, p.len16())),
            _ => None,
        };
        self.story_gen += 1;
        self.stories.push(Story { kind: StoryKind::Footnote, part: part.clone(), note_id: Some(id.clone()), blocks, gen: self.story_gen });
        self.dirty_parts.insert(part);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(stories::reference_run(&ref_style, &id)));
        self.stories_touched = true;
        Ok(OpResult { caret, ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Bookmarks
    // -----------------------------------------------------------------------

    pub(super) fn op_insert_horizontal_line(&mut self, at: Pos) -> Result<OpResult, EditError> {
        let pid = at.p.0;
        let len = self.para_ref(pid)?.len16();
        let off = at.o.min(len);
        let mut created = Vec::new();
        let line_id;
        if len == 0 {
            // The empty paragraph becomes the line (a section break it carries stays).
            self.touch(pid);
            let p = self.para_mut(pid)?;
            let sect = p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).cloned();
            p.ppr = Some(marks::line_ppr(sect));
            line_id = pid;
        } else if off == 0 {
            line_id = self.ids.fresh();
            let mut line = Paragraph::new(line_id);
            line.ppr = Some(marks::line_ppr(None));
            self.insert_blocks(None, Some(pid), vec![Block::Para(line)])?;
            created.push(line_id);
        } else {
            if off < len {
                let right = self.split_paragraph(Pos::new(pid, off))?;
                created.extend(right.created);
            }
            // After the text: a section break the paragraph ends moves to the line, which stays in that section.
            let sect = if off == len {
                let p = self.para_mut(pid)?;
                let sect = p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).cloned();
                if let Some(pp) = p.ppr.as_mut() {
                    pp.remove_children("w:sectPr");
                }
                sect
            } else {
                None
            };
            line_id = self.ids.fresh();
            let mut line = Paragraph::new(line_id);
            line.ppr = Some(marks::line_ppr(sect));
            self.insert_blocks(Some(pid), None, vec![Block::Para(line)])?;
            created.insert(0, line_id);
        }
        // The caret goes to the start of the paragraph after the line; make one if the line is last.
        let nid = self.ids.fresh();
        let (mut list, i) = self.body.parent_mut(line_id).ok_or(EditError::NotFound(line_id))?;
        let next = match list.get(i + 1) {
            Some(Block::Para(np)) => np.id,
            _ => {
                list.insert(i + 1, Block::Para(Paragraph::new(nid)));
                created.push(nid);
                nid
            }
        };
        self.touch(line_id);
        self.touch(next);
        Ok(OpResult { caret: Some(Pos::new(next, 0)), created, count: None })
    }

    // -----------------------------------------------------------------------
    // Watermarks
    // -----------------------------------------------------------------------

    /// A page or column break at a position.
    pub(super) fn op_inline_break(&mut self, at: Pos, ty: &str) -> Result<OpResult, EditError> {
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Break(Element::new("w:br").with_attr("w:type", ty))] };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }

    pub(super) fn op_set_link(&mut self, from: Pos, to: Pos, url: Option<&str>) -> Result<OpResult, EditError> {
        let (from, to) = self.ordered(from, to)?;
        if from.p != to.p {
            return Err(EditError::Invalid("links must stay within one paragraph".into()));
        }
        let shell = match url {
            // A link to a place in the document names a bookmark that is there (a link to nowhere does nothing in Word either).
            Some(u) if u.starts_with('#') && !self.has_bookmark(&u[1..]) => {
                return Err(EditError::Invalid(format!("There is no bookmark named “{}” in this document.", &u[1..])));
            }
            Some(u) if u.starts_with('#') => Some(Element::new("w:hyperlink").with_attr("w:anchor", &u[1..]).with_attr("w:history", "1")),
            Some(u) => {
                let main = self.active_part.clone();
                let rid = self.pkg.add_rel(&main, package::REL_HYPERLINK, u, true);
                self.ensure_ns("r", NS_R);
                Some(Element::new("w:hyperlink").with_attr("r:id", rid).with_attr("w:history", "1"))
            }
            None => None,
        };
        let link_style = if shell.is_some() { self.styles.ensure_builtin("Hyperlink") } else { None };
        self.touch(from.p.0);
        let p = self.para_mut(from.p.0)?;
        let len = p.len16();
        let (s, e) = (from.o.min(len), to.o.min(len));
        // A link written as a field becomes ordinary text first, so the link below can wrap or drop it.
        let unwrapped = crate::fields::unwrap_fields(&mut p.inlines, s, e, &|i| crate::fields::Instr::parse(i).code == "HYPERLINK");
        if unwrapped && url.is_none() {
            // Taking the link off also takes off the link look.
            let mut at = 0;
            for inl in p.inlines.iter_mut() {
                let l = inl.len16();
                if let Inline::Run(r) = inl {
                    if at < e.max(s + 1) && at + l > s {
                        if let Some(rpr) = r.rpr.as_mut() {
                            if rpr.child("w:rStyle").and_then(|x| x.attr("w:val")) == Some("Hyperlink") {
                                rpr.remove_children("w:rStyle");
                            }
                        }
                    }
                }
                at += l;
            }
        }
        // Unwrap existing hyperlinks overlapping the range.
        let mut start = 0;
        let mut flat = Vec::new();
        for inl in std::mem::take(&mut p.inlines) {
            let l = inl.len16();
            match inl {
                Inline::Container(c) if c.kind == InlineKind::Hyperlink && start < e.max(s + 1) && start + l > s => {
                    // Only the selected part loses the old link; text before/after keeps it.
                    let (ls, le) = if e > s { (s.saturating_sub(start).min(l), e.min(start + l) - start) } else { (0, l) };
                    let (left, rest) = edit::partition(c.children, ls);
                    let (mid, right) = edit::partition(rest, le - ls);
                    let wrap = |children: Vec<Inline>| InlineContainer { kind: c.kind, shell: c.shell.clone(), content_in: c.content_in.clone(), children };
                    if left.iter().any(|i| i.len16() > 0) {
                        flat.push(Inline::Container(wrap(left)));
                    } else {
                        flat.extend(left);
                    }
                    let keep_right = right.iter().any(|i| i.len16() > 0);
                    for mut ch in mid {
                        if let Inline::Run(r) = &mut ch {
                            if let Some(rpr) = r.rpr.as_mut() {
                                if rpr.child("w:rStyle").and_then(|x| x.attr("w:val")) == Some("Hyperlink") {
                                    rpr.remove_children("w:rStyle");
                                }
                            }
                        }
                        flat.push(ch);
                    }
                    if keep_right {
                        flat.push(Inline::Container(wrap(right)));
                    } else {
                        flat.extend(right);
                    }
                }
                other => flat.push(other),
            }
            start += l;
        }
        p.inlines = flat;
        if let Some(shell) = shell {
            if e > s {
                edit::split_at(&mut p.inlines, s);
                edit::split_at(&mut p.inlines, e);
                let mut pos = 0;
                let mut first = None;
                let mut last = 0;
                let mut covered = 0;
                for (i, inl) in p.inlines.iter().enumerate() {
                    let l = inl.len16();
                    if pos >= s && pos + l <= e && l > 0 {
                        first.get_or_insert(i);
                        last = i;
                        covered += l;
                    }
                    pos += l;
                }
                if covered != e - s {
                    // Part of the range sits inside a tracked change or content control.
                    return Err(EditError::Invalid("links can't yet span tracked changes or content controls".into()));
                }
                if let Some(f) = first {
                    let mut children: Vec<Inline> = p.inlines.drain(f..=last).collect();
                    for ch in children.iter_mut() {
                        if let Inline::Run(r) = ch {
                            let rpr = r.rpr.get_or_insert_with(|| Element::new("w:rPr"));
                            if let Some(ls) = &link_style {
                                crate::props::put_rpr(rpr, Element::new("w:rStyle").with_attr("w:val", ls));
                            }
                        }
                    }
                    p.inlines.insert(f, Inline::Container(InlineContainer { kind: InlineKind::Hyperlink, shell, content_in: None, children }));
                }
            }
        }
        Ok(OpResult { caret: Some(to), ..Default::default() })
    }
}
