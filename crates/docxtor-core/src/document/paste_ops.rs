//! Putting blocks into the document: paste, Markdown, inserting blocks, and carrying relationships between parts.

use super::*;

impl Document {
    /// Insert blocks after/before an existing block (same container), or at the end of the body.
    pub(super) fn insert_blocks(&mut self, after: Option<NodeId>, before: Option<NodeId>, blocks: Vec<Block>) -> Result<(), EditError> {
        let new_ids: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        // What is drawn again: the new blocks themselves when they go in at the top level, or else the top-level block
        // that holds them (a table they go into). Worked out once, not once for each block: a converted file can put
        // in a hundred thousand.
        let top_level = if let Some(a) = after.or(before) {
            let top = top_level_of(&self.body, a);
            let (mut list, i) = self.body.parent_mut(a).ok_or(EditError::NotFound(a))?;
            // The closing section properties stay the last block of the body: "after" them means before them.
            let closing = matches!(&list[i], Block::Opaque(o) if o.el.name == "w:sectPr") && i + 1 == list.len();
            let at = if after.is_some() && !closing { i + 1 } else { i };
            list.splice(at..at, blocks);
            if let Some(t) = top {
                self.changed.insert(t);
            }
            top == Some(a)
        } else {
            let pos = self
                .body
                .iter()
                .rposition(|b| !matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr"))
                .map(|i| i + 1)
                .unwrap_or(0);
            self.body.splice(pos..pos, blocks);
            true
        };
        // Blocks put in after the paragraph that ends a tab stay in that tab.
        if let Some(a) = after {
            self.tabs_keep_boundary(a, &new_ids);
        }
        if top_level {
            self.changed.extend(new_ids);
        }
        Ok(())
    }

    pub(super) fn ensure_para_after(&mut self, id: NodeId) {
        let nid = self.ids.fresh();
        if let Some((mut list, i)) = self.body.parent_mut(id) {
            if !matches!(list.get(i + 1), Some(Block::Para(_))) {
                list.insert(i + 1, Block::Para(Paragraph::new(nid)));
                self.touch(nid);
            }
        }
    }

    /// The closing section properties are the last block of the body. An op that put blocks after them gets them
    /// moved back to the end, so the section list and the file agree whatever the op did.
    pub(super) fn keep_closing_sect_last(&mut self) {
        let is_sect = |b: &Block| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr");
        let Some(at) = self.body.iter().position(is_sect) else { return };
        if at + 1 == self.body.len() {
            return;
        }
        let sect = self.body.remove(at);
        self.body.push(sect);
        self.body_dirty = true;
    }

    /// Build runs from Markdown spans, starting from a base run format.
    pub(super) fn md_inlines(&mut self, spans: &[MdSpan], base: Option<&Element>) -> Vec<Inline> {
        let mut out: Vec<Inline> = Vec::new();
        for sp in spans {
            if let Some(display) = sp.math {
                if let Ok(omath) = crate::math::latex_to_omml(&sp.text) {
                    self.ensure_ns("m", crate::math::NS_M);
                    out.push(Inline::Opaque(if display { crate::math::display_wrap(omath) } else { omath }));
                    continue;
                }
            }
            let mut rpr = base.cloned().unwrap_or_else(|| Element::new("w:rPr"));
            let patch = RunPatch {
                bold: sp.bold.then_some(true),
                italic: sp.italic.then_some(true),
                strike: sp.strike.then_some(true),
                underline: sp.underline.then(|| "single".to_string()),
                font: sp.code.then(|| "Consolas".to_string()),
                ..Default::default()
            };
            patch.apply(&mut rpr, &RunFmt::default());
            if sp.code {
                crate::props::put_rpr(&mut rpr, Element::new("w:shd").with_attr("w:val", "clear").with_attr("w:color", "auto").with_attr("w:fill", "F3F4F6"));
            }
            let mut items_holder = Vec::new();
            edit::insert_text(&mut items_holder, 0, &sp.text, Some(Element::new("w:rPr")));
            let items = items_holder.into_iter().find_map(|i| if let Inline::Run(r) = i { Some(r.items) } else { None }).unwrap_or_default();
            let run = Run { t_attrs: None, attrs: vec![], rpr: Some(rpr).filter(|r| !r.children.is_empty()), items };
            // A link to a place in the document that has no bookmark of that name stays plain text, as deleting a bookmark leaves its links.
            let link = sp.link.as_ref().filter(|u| u.strip_prefix('#').is_none_or(|a| self.has_bookmark(a)));
            if let Some(url) = link {
                let link_style = self.styles.ensure_builtin("Hyperlink");
                let mut run = run;
                if let Some(ls) = link_style {
                    crate::props::put_rpr(run.rpr.get_or_insert_with(|| Element::new("w:rPr")), Element::new("w:rStyle").with_attr("w:val", ls));
                }
                // Merge into the previous link container if it points to the same URL.
                if let Some(Inline::Container(c)) = out.last_mut() {
                    if c.kind == InlineKind::Hyperlink && c.shell.attr("docxtor:url") == Some(url.as_str()) {
                        c.children.push(Inline::Run(run));
                        continue;
                    }
                }
                let shell = if let Some(anchor) = url.strip_prefix('#') {
                    Element::new("w:hyperlink").with_attr("w:anchor", anchor)
                } else {
                    let main = self.active_part.clone();
                    let rid = self.pkg.add_rel(&main, package::REL_HYPERLINK, url, true);
                    self.ensure_ns("r", NS_R);
                    Element::new("w:hyperlink").with_attr("r:id", rid)
                };
                out.push(Inline::Container(InlineContainer {
                    kind: InlineKind::Hyperlink,
                    shell: shell.with_attr("docxtor:url", url.as_str()),
                    content_in: None,
                    children: vec![Inline::Run(run)],
                }));
            } else {
                out.push(Inline::Run(run));
            }
        }
        // Strip the temporary url marker.
        for i in out.iter_mut() {
            if let Inline::Container(c) = i {
                c.shell.remove_attr("docxtor:url");
            }
        }
        edit::cleanup(&mut out);
        out
    }

    pub(super) fn op_paste(&mut self, at: Pos, md: &str) -> Result<OpResult, EditError> {
        let at = Pos::new(at.p.0, at.o.min(self.para_ref(at.p.0)?.len16()));
        let single_line = !md.trim_end_matches(['\n', '\r']).contains('\n');
        let looks_block = single_line && markdown::parse_blocks(md).iter().any(|b| !matches!(b, MdBlock::Para { style: None, .. }));
        let parsed = if single_line && !looks_block {
            // Keep leading/trailing whitespace of inline pastes.
            vec![MdBlock::Para { style: None, spans: markdown::parse_inline(md.trim_end_matches(['\n', '\r'])) }]
        } else {
            markdown::parse_blocks(md)
        };
        let mut parsed = parsed;
        // Block parsing trims lines; keep trailing spaces so the paste joins the following text naturally.
        let trailing: String = md.chars().rev().take_while(|c| *c == ' ' || *c == '\t').collect();
        if !single_line && !trailing.is_empty() {
            if let Some(MdBlock::Para { style: None, spans }) = parsed.last_mut() {
                if let Some(last) = spans.last_mut() {
                    last.text.push_str(&trailing);
                }
            }
        }
        let plain_para = |b: &MdBlock| matches!(b, MdBlock::Para { style: None, .. });
        if parsed.is_empty() {
            return Ok(OpResult { caret: Some(at), ..Default::default() });
        }
        if parsed.len() == 1 && plain_para(&parsed[0]) {
            let MdBlock::Para { spans, .. } = &parsed[0] else { unreachable!() };
            let base = {
                let p = self.para_ref(at.p.0)?;
                edit::rpr_at(&p.inlines, at.o).map(strip_link_style).or_else(|| Self::mark_rpr(p))
            };
            let inl = self.md_inlines(spans, base.as_ref());
            self.touch(at.p.0);
            let p = self.para_mut(at.p.0)?;
            let mut off = at.o.min(p.len16());
            for i in inl {
                let l = i.len16();
                edit::insert_inline_at(&mut p.inlines, off, i);
                off += l;
            }
            edit::cleanup(&mut p.inlines);
            return Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() });
        }
        // Multi-block: split, merge the first/last plain paragraphs into the halves, insert the rest between.
        let split = self.op_split(at)?;
        let right = match split.created.first() {
            Some(id) => *id,
            None => {
                // Splitting an empty list item only exits the list; make the right half explicitly.
                let id = self.ids.fresh();
                self.insert_blocks(Some(at.p.0), None, vec![Block::Para(Paragraph::new(id))])?;
                id
            }
        };
        let mut blocks = parsed;
        let mut caret = Pos::new(right, 0);
        if let Some(MdBlock::Para { style: None, spans }) = blocks.first().cloned() {
            blocks.remove(0);
            let inl = self.md_inlines(&spans, None);
            let p = self.para_mut(at.p.0)?;
            let mut off = p.len16();
            for i in inl {
                let l = i.len16();
                edit::insert_inline_at(&mut p.inlines, off, i);
                off += l;
            }
        }
        if !blocks.is_empty() {
            if let Some(MdBlock::Para { style: None, spans }) = blocks.last().cloned() {
                blocks.pop();
                let inl = self.md_inlines(&spans, None);
                let p = self.para_mut(right)?;
                let mut off = 0;
                for i in inl {
                    let l = i.len16();
                    edit::insert_inline_at(&mut p.inlines, off, i);
                    off += l;
                }
                caret = Pos::new(right, off);
            }
        }
        if !blocks.is_empty() {
            let md_rest = blocks_to_md(&blocks);
            let r = self.op_insert_markdown(Some(at.p.0), None, &md_rest)?;
            if caret.o == 0 {
                if let Some(c) = r.caret {
                    caret = c;
                }
            }
        }
        self.touch(at.p.0);
        self.touch(right);
        Ok(OpResult { caret: Some(caret), ..Default::default() })
    }

    pub(super) fn op_insert_markdown(&mut self, after: Option<NodeId>, before: Option<NodeId>, md: &str) -> Result<OpResult, EditError> {
        let parsed = markdown::parse_blocks(md);
        let mut blocks = Vec::new();
        let mut list_ids: HashMap<(bool, usize), i64> = HashMap::new();
        let mut prev_was_list = false;
        for b in parsed {
            match b {
                MdBlock::Para { style, spans } => {
                    prev_was_list = false;
                    let id = self.ids.fresh();
                    let mut p = Paragraph::new(id);
                    if let Some(s) = style {
                        let sid = self.resolve_style(&s)?;
                        let mut ppr = Element::new("w:pPr");
                        put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", sid));
                        p.ppr = Some(ppr);
                    }
                    p.inlines = self.md_inlines(&spans, None);
                    blocks.push(Block::Para(p));
                }
                MdBlock::ListItem { kind, level, check, mut spans } => {
                    if !prev_was_list {
                        list_ids.clear();
                    }
                    prev_was_list = true;
                    let bullet = kind == ListKind::Bullet;
                    let num_id = match check {
                        // Task items use the document's checklist (checked ones are struck through).
                        Some(checked) => {
                            if checked {
                                for sp in &mut spans {
                                    sp.strike = true;
                                }
                            }
                            self.numbering.shared_num(if checked { "checklist_checked" } else { "checklist" }).expect("checklist preset")
                        }
                        None => match list_ids.get(&(bullet, 0)) {
                            Some(n) => *n,
                            None => {
                                let n = self.numbering.create_list(kind);
                                list_ids.insert((bullet, 0), n);
                                n
                            }
                        },
                    };
                    let ls = self.styles.ensure_builtin("ListParagraph");
                    let id = self.ids.fresh();
                    let mut p = Paragraph::new(id);
                    let mut ppr = Element::new("w:pPr");
                    if let Some(ls) = ls {
                        put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", ls));
                    }
                    ParaPatch { num: Some((num_id, level.min(8) as i64)), ..Default::default() }.apply(&mut ppr);
                    p.ppr = Some(ppr);
                    p.inlines = self.md_inlines(&spans, None);
                    blocks.push(Block::Para(p));
                }
                MdBlock::Code { lines } => {
                    prev_was_list = false;
                    let sid = self.resolve_style("Code")?;
                    for line in lines {
                        let mut p = Paragraph::new(self.ids.fresh());
                        let mut ppr = Element::new("w:pPr");
                        put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", &sid));
                        p.ppr = Some(ppr);
                        edit::insert_text(&mut p.inlines, 0, &line, None);
                        blocks.push(Block::Para(p));
                    }
                }
                MdBlock::Table { rows } => {
                    prev_was_list = false;
                    let mut cells = Vec::new();
                    for r in rows {
                        let mut row = Vec::new();
                        for c in r {
                            row.push(self.md_inlines(&c, None));
                        }
                        cells.push(row);
                    }
                    let t = self.build_table(cells, true)?;
                    blocks.push(Block::Table(t));
                    // Word requires a paragraph between adjacent tables.
                    blocks.push(Block::Para(Paragraph::new(self.ids.fresh())));
                }
                MdBlock::PageBreak => {
                    prev_was_list = false;
                    let mut p = Paragraph::new(self.ids.fresh());
                    p.inlines.push(Inline::Run(Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Break(Element::new("w:br").with_attr("w:type", "page"))] }));
                    blocks.push(Block::Para(p));
                }
            }
        }
        // Drop a trailing empty spacer paragraph if the next block is already a paragraph.
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        let last_para = blocks.iter().rev().find_map(|b| if let Block::Para(p) = b { Some((p.id, p.len16())) } else { None });
        self.insert_blocks(after, before, blocks)?;
        Ok(OpResult { caret: last_para.map(|(id, l)| Pos::new(id, l)), created, count: None })
    }
}

/// Re-serialize parsed Markdown blocks (used to forward the middle of a paste).
pub(super) fn blocks_to_md(blocks: &[MdBlock]) -> String {
    fn spans(s: &[MdSpan]) -> String {
        s.iter()
            .map(|sp| {
                let mut t = markdown::escape(&sp.text);
                if sp.code {
                    t = format!("`{}`", sp.text);
                }
                if sp.italic {
                    t = format!("*{t}*");
                }
                if sp.bold {
                    t = format!("**{t}**");
                }
                if sp.strike {
                    t = format!("~~{t}~~");
                }
                if sp.underline {
                    t = format!("<u>{t}</u>");
                }
                if let Some(l) = &sp.link {
                    t = format!("[{t}]({l})");
                }
                t
            })
            .collect()
    }
    let mut out = String::new();
    for b in blocks {
        match b {
            MdBlock::Para { style, spans: sp } => {
                let prefix = match style.as_deref() {
                    Some("Title") => "#! ".to_string(),
                    Some(s) if s.starts_with("Heading") => format!("{} ", "#".repeat(s[7..].parse().unwrap_or(1))),
                    Some("Quote") => "> ".into(),
                    _ => String::new(),
                };
                out.push_str(&format!("{prefix}{}\n\n", spans(sp)));
            }
            MdBlock::ListItem { kind, level, check, spans: sp } => {
                let m = if *kind == ListKind::Bullet { "-" } else { "1." };
                let task = match check {
                    Some(true) => "[x] ",
                    Some(false) => "[ ] ",
                    None => "",
                };
                out.push_str(&format!("{}{m} {task}{}\n", "  ".repeat(*level), spans(sp)));
            }
            MdBlock::Code { lines } => out.push_str(&format!("```\n{}\n```\n\n", lines.join("\n"))),
            MdBlock::Table { rows } => {
                for (i, r) in rows.iter().enumerate() {
                    out.push_str(&format!("| {} |\n", r.iter().map(|c| spans(c)).collect::<Vec<_>>().join(" | ")));
                    if i == 0 {
                        out.push_str(&format!("|{}\n", "---|".repeat(r.len())));
                    }
                }
                out.push('\n');
            }
            MdBlock::PageBreak => out.push_str("<pagebreak>\n\n"),
        }
    }
    out
}

/// Remove hyperlink character styling so text typed next to a link doesn't look like one.
pub(crate) fn strip_link_style(mut rpr: Element) -> Element {
    if rpr.child("w:rStyle").and_then(|s| s.attr("w:val")) == Some("Hyperlink") {
        rpr.remove_children("w:rStyle");
        rpr.remove_children("w:color");
        rpr.remove_children("w:u");
    }
    rpr
}

impl Document {
    /// Content that moves from one part to another (a note from the endnotes to the footnotes) takes the relationships
    /// it uses with it: each `r:id`, `r:embed` or `r:link` in `blocks` that names a relationship of `from` is added to
    /// the relationships of `to` (the same one when `to` already has it) and the attribute is set to the id it has there.
    /// Namespace declarations the content may rely on are copied to the target's root. The one home for this, so no
    /// op that moves content between parts forgets its pictures, charts and links.
    pub(super) fn carry_relationships(&mut self, from: &str, to: &str, blocks: &mut [Block]) {
        if from == to {
            return;
        }
        let mut used = false;
        walk_elements_mut(blocks, &mut |e| {
            used |= e.attrs.iter().any(|(k, _)| k.starts_with("r:") || k == "o:relid");
        });
        if !used {
            return;
        }
        let mut seen: HashMap<String, String> = HashMap::new();
        {
            let pkg = &mut self.pkg;
            walk_elements_mut(blocks, &mut |e| {
                for (k, v) in e.attrs.iter_mut() {
                    if !(k.starts_with("r:") || k == "o:relid") {
                        continue;
                    }
                    if let Some(new) = seen.get(v.as_str()) {
                        *v = new.as_str().into();
                        continue;
                    }
                    let Some(rel) = pkg.resolve_rel(from, v) else { continue };
                    let target = if rel.external { rel.target.clone() } else { relative_target(to, &rel.target) };
                    let new = pkg.add_rel(to, &rel.rel_type, &target, rel.external);
                    seen.insert(v.to_string(), new.clone());
                    *v = new.into();
                }
            });
        }
        if seen.is_empty() {
            return;
        }
        let from_ns: Vec<(Name, AttrValue)> = match self.story_parts.get(from) {
            Some(d) => d.root.attrs.iter().filter(|(k, _)| k.starts_with("xmlns:")).cloned().collect(),
            None => Vec::new(),
        };
        if let Some(d) = self.story_parts.get_mut(to) {
            for (k, v) in from_ns {
                if d.root.attr(&k).is_none() {
                    d.root.set_attr(k, v);
                }
            }
        }
    }
}

pub(super) fn relative_target(source: &str, target: &str) -> String {
    let base = source.rsplit_once('/').map(|(b, _)| b).unwrap_or("");
    if !base.is_empty() {
        if let Some(rest) = target.strip_prefix(&format!("{base}/")) {
            return rest.to_string();
        }
    }
    format!("/{target}")
}

pub(super) fn drawing_xml(rid: &str, w_pt: f64, h_pt: f64, id: u32, alt: &str) -> Element {
    let cx = (w_pt * 12700.0).round() as i64;
    let cy = (h_pt * 12700.0).round() as i64;
    let alt = alt.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
    let src = format!(
        r#"<w:drawing xmlns:w="{NS_W}" xmlns:wp="{NS_WP}" xmlns:r="{NS_R}"><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Picture {id}" descr="{alt}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{id}" name="Picture {id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#
    );
    let mut el = xml::parse(src.as_bytes()).expect("valid drawing xml").root;
    // Namespaces are declared on the document root; drop the local declarations.
    el.attrs.retain(|(k, _)| !k.starts_with("xmlns:"));
    el
}
