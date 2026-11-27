//! The table of contents as the "Table of contents" dialog edits it: entries from heading styles, outline
//! levels, other styles and table entry (`TC`) fields; the options written as Word's switches; the leader and
//! the look kept in the `TOC` styles the way Word keeps them; update page numbers only; remove.

use super::*;
use crate::fields::{self, Instr};
use crate::index::{self, IndexOptions};
use crate::toc::{Layout, Leader};
use crate::toc_spec::{self, Spec, TocOptions};

/// A list field found in the text: its paragraphs, in order, and its instruction.
pub(super) struct ListField {
    pub paras: Vec<NodeId>,
    pub instr: String,
}

pub(super) fn collect_fields<S: crate::body::Seq + ?Sized>(blocks: &S, want: &dyn Fn(&str) -> bool, out: &mut Vec<ListField>) {
    for f in toc::find_where(blocks, want) {
        out.push(ListField { paras: (f.start..=f.end).map(|k| blocks.seq_at(k).id()).collect(), instr: f.instr });
    }
    for i in 0..blocks.seq_len() {
        if let Block::Container(c) = blocks.seq_at(i) {
            collect_fields(&c.blocks, want, out);
        }
    }
}

/// Whether an instruction is a table of contents (a `TOC` field that doesn't list captions).
pub(super) fn is_toc(instr: &str) -> bool {
    let i = Instr::parse(instr);
    i.code == "TOC" && i.value("c").is_none()
}

fn flatten(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

/// How a look sets one level of the contents: left indent, space after, and bold, caps and italic.
struct LookLevel {
    indent: i64,
    before: i64,
    after: i64,
    bold: bool,
    caps: bool,
    italic: bool,
}

/// The four looks. `From template` (None) leaves the styles alone.
fn look_level(look: &str, level: i64) -> Option<LookLevel> {
    let n = level.clamp(1, 9);
    Some(match look {
        "classic" => LookLevel { indent: (n - 1) * 220, before: 0, after: 100, bold: n == 1, caps: false, italic: false },
        "formal" => LookLevel { indent: (n - 1) * 360, before: if n == 1 { 120 } else { 0 }, after: 60, bold: n <= 2, caps: n == 1, italic: n >= 3 },
        "modern" => LookLevel { indent: 0, before: if n == 1 { 160 } else { 0 }, after: 40, bold: n == 1, caps: false, italic: false },
        "simple" => LookLevel { indent: (n - 1) * 200, before: 0, after: 0, bold: false, caps: false, italic: false },
        _ => return None,
    })
}

impl Document {
    // -----------------------------------------------------------------------
    // Entries
    // -----------------------------------------------------------------------

    /// The built-in heading level a paragraph's style stands for (`Heading 2` is 2), whatever its outline level.
    pub(super) fn heading_style_level(&self, p: &Paragraph) -> Option<u8> {
        let id = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val"))?;
        let name = self.styles.get(id).map(|s| s.name.clone()).unwrap_or_default();
        [id.to_string(), name].iter().find_map(|s| {
            let f = flatten(s);
            let n: u8 = f.strip_prefix("heading")?.parse().ok()?;
            (1..=9).contains(&n).then_some(n)
        })
    }

    /// The level a table of contents gives a paragraph, if it lists it: by a `\t` style, a heading style (`\o`) or
    /// an outline level (`\u`).
    fn toc_level_of(&self, p: &Paragraph, spec: &Spec) -> Option<u8> {
        let id = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or_default();
        if !spec.styles.is_empty() {
            let name = self.styles.get(id).map(|s| s.name.clone()).unwrap_or_default();
            if let Some((_, l)) = spec.styles.iter().find(|(s, _)| flatten(s) == flatten(id) || (!name.is_empty() && flatten(s) == flatten(&name))) {
                return Some(*l);
            }
        }
        if let Some((lo, hi)) = spec.headings {
            if let Some(l) = self.heading_style_level(p).filter(|l| (lo..=hi).contains(l)) {
                return Some(l);
            }
        }
        if spec.outline {
            let range = spec.headings.unwrap_or((1, 9));
            return self.heading_level(p).filter(|l| (range.0..=range.1).contains(l));
        }
        None
    }

    /// The entries a table of contents lists, in document order, each with a `_Toc` bookmark (added where missing).
    pub(super) fn toc_entries_for(&mut self, spec: &Spec) -> Vec<toc::Entry> {
        let mut tcs: HashMap<NodeId, Vec<toc_spec::TcMark>> = HashMap::new();
        if let Some(tc) = &spec.tc {
            fields::scan_blocks(&self.body, &mut |pid, seen| {
                if let Some(m) = toc_spec::parse_tc(seen.instr) {
                    if m.id.eq_ignore_ascii_case(&tc.id) && (tc.levels.0..=tc.levels.1).contains(&m.level) {
                        tcs.entry(pid).or_default().push(m);
                    }
                }
            });
        }
        let mut ids = Vec::new();
        walk_paragraphs(&self.body, &mut |p| ids.push(p.id));
        // What a table of contents, of figures or an index shows is not listed, whatever its style: updating replaces it (with any
        // bookmark put on it), so an entry would lead nowhere, and Word leaves it out too.
        let inside = lists_shown_by_fields(&self.body);
        // A heading's list number goes before its text (bullets don't).
        let labels = self.numbering.labels(&self.body, &self.styles);
        let number = |id: NodeId| labels.get(&id).filter(|l| !l.bullet && !l.text.trim().is_empty()).map(|l| l.text.trim().to_string());
        struct Head {
            id: NodeId,
            level: u8,
            text: String,
            label: Option<String>,
            no_page: bool,
            existing: Option<String>,
        }
        let mut heads: Vec<Head> = Vec::new();
        for id in ids {
            if inside.contains(&id) {
                continue;
            }
            let Some(p) = find_para(&self.body, id) else { continue };
            let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or_default();
            if style.starts_with("TOC") || style == "TableofFigures" {
                continue;
            }
            let existing = toc::toc_bookmark(p);
            if let Some(level) = self.toc_level_of(p, spec) {
                let text = p.plain_text().replace(['\t', '\n'], " ").trim().to_string();
                if !text.is_empty() {
                    heads.push(Head { id, level, text, label: number(id), no_page: !spec.shows_page(level), existing: existing.clone() });
                }
            }
            for m in tcs.get(&id).into_iter().flatten() {
                heads.push(Head { id, level: m.level, text: m.text.clone(), label: None, no_page: m.no_page || !spec.shows_page(m.level), existing: existing.clone() });
            }
        }
        let (bm_ids, names) = self.bookmark_ids_and_names();
        let mut names: std::collections::HashSet<String> = names.into_iter().collect();
        let mut next_id = bm_ids.iter().max().copied().unwrap_or(0) + 1;
        let mut seq: u64 = 100_000_000 + next_id as u64;
        let mut made: HashMap<NodeId, String> = HashMap::new();
        let mut out = Vec::new();
        for Head { id, level, text, label, no_page, existing } in heads {
            let bookmark = match existing.or_else(|| made.get(&id).cloned()) {
                Some(b) => b,
                None => {
                    let mut name = format!("_Toc{seq}");
                    while names.contains(&name) {
                        seq += 1;
                        name = format!("_Toc{seq}");
                    }
                    seq += 1;
                    names.insert(name.clone());
                    let bid = next_id.to_string();
                    next_id += 1;
                    self.touch(id);
                    if let Ok(p) = self.para_mut(id) {
                        p.inlines.insert(0, Inline::Opaque(Element::new("w:bookmarkStart").with_attr("w:id", &bid).with_attr("w:name", &name)));
                        p.inlines.push(Inline::Opaque(Element::new("w:bookmarkEnd").with_attr("w:id", &bid)));
                    }
                    made.insert(id, name.clone());
                    name
                }
            };
            out.push(toc::Entry { para: id, level, text, label, bookmark, no_page });
        }
        out
    }

    // -----------------------------------------------------------------------
    // The TOC styles: leader and look
    // -----------------------------------------------------------------------

    /// The leader of the right tab stop in a paragraph style, if it has one.
    pub(super) fn style_leader(&self, style: &str) -> Option<Leader> {
        let tabs = self.styles.style_element(style)?.path(&["w:pPr", "w:tabs"])?;
        let tab = tabs.children_named("w:tab").find(|t| t.attr("w:val") == Some("right"))?;
        Some(tab.attr("w:leader").and_then(Leader::parse).unwrap_or(Leader::None))
    }

    /// Write the leader (and, with a look, the formatting) into the styles `prefix`1 to `prefix`9, the way Word's
    /// dialog does, so a table Word updates later looks the same. `look` is None to keep the formatting.
    pub(super) fn set_list_styles(&mut self, prefix: &str, levels: u8, leader: Option<Leader>, look: Option<&str>, tab_pos: i64) {
        let Some(doc) = self.styles.doc.as_mut() else { return };
        let mut changed = false;
        for n in 1..=levels.clamp(1, 9) as i64 {
            let id = format!("{prefix}{n}");
            let Some(style) = doc.root.children.iter_mut().find_map(|c| match c {
                Node::Element(e) if e.name == "w:style" && e.attr("w:styleId") == Some(id.as_str()) => Some(e),
                _ => None,
            }) else {
                continue;
            };
            let lv = look.and_then(|l| look_level(l, n));
            let mut ppr = style.child("w:pPr").cloned().unwrap_or_else(|| Element::new("w:pPr"));
            if let Some(l) = leader {
                ppr.remove_children("w:tabs");
                let tabs = Element::new("w:tabs").with_child(Element::new("w:tab").with_attr("w:val", "right").with_attr("w:leader", l.val()).with_attr("w:pos", tab_pos.to_string()));
                crate::props::put_ppr(&mut ppr, tabs);
            }
            if let Some(lv) = &lv {
                ppr.remove_children("w:spacing");
                ppr.remove_children("w:ind");
                let mut sp = Element::new("w:spacing").with_attr("w:after", lv.after.to_string());
                if lv.before > 0 {
                    sp = sp.with_attr("w:before", lv.before.to_string());
                }
                crate::props::put_ppr(&mut ppr, sp);
                if lv.indent > 0 {
                    crate::props::put_ppr(&mut ppr, Element::new("w:ind").with_attr("w:left", lv.indent.to_string()));
                }
            }
            put_style_child(style, ppr);
            if let Some(lv) = lv {
                style.remove_children("w:rPr");
                if lv.bold || lv.caps || lv.italic {
                    let mut r = Element::new("w:rPr");
                    if lv.bold {
                        r = r.with_child(Element::new("w:b")).with_child(Element::new("w:bCs"));
                    }
                    if lv.italic {
                        r = r.with_child(Element::new("w:i")).with_child(Element::new("w:iCs"));
                    }
                    if lv.caps {
                        r = r.with_child(Element::new("w:caps"));
                    }
                    put_style_child(style, r);
                }
            }
            changed = true;
        }
        if changed {
            self.styles.reload();
        }
    }

    /// The leader for an index: the one asked for, else the one the existing index uses, else the `Index 1` style's.
    pub(super) fn index_leader(&self, instr: &str, forced: Option<Leader>) -> Leader {
        forced
            .or_else(|| {
                let mut found = Vec::new();
                collect_fields(&self.body, &|i| crate::index::is_index(i), &mut found);
                let f = found.iter().find(|f| f.instr == instr).or(found.first())?;
                find_para(&self.body, *f.paras.first()?).and_then(toc::leader_of)
            })
            .or_else(|| self.style_leader("Index1"))
            .unwrap_or_default()
    }

    /// The leader a new or rebuilt table of contents uses when nothing else says: the `TOC 1` style's, else dots.
    pub(super) fn default_toc_leader(&self) -> Leader {
        self.style_leader("TOC1").unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn op_insert_toc_with(&mut self, after: Option<NodeId>, before: Option<NodeId>, options: &TocOptions, title: Option<&str>, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let instr = options.instr(None);
        let spec = Spec::parse(&instr);
        let entries = self.toc_entries_for(&spec);
        let deepest = entries.iter().map(|e| e.level).max().unwrap_or(1).max(spec.max_level().min(3));
        self.ensure_toc_styles(deepest);
        let width = self.text_width();
        if options.look.is_some() || options.leader != Leader::Dot {
            self.set_list_styles("TOC", deepest, Some(options.leader), options.look.as_deref(), width);
        }
        let page = self.toc_page_fn(pages);
        let mut els = Vec::new();
        if let Some(t) = title.filter(|t| !t.trim().is_empty()) {
            let style = self.styles.ensure_builtin("TOCHeading").unwrap_or_else(|| "TOCHeading".into());
            els.push(
                Element::new("w:p")
                    .with_child(Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style)))
                    .with_child(Element::new("w:r").with_child(Element::new("w:t").with_attr("xml:space", "preserve").with_text(t))),
            );
        }
        let lay = Layout::of(&spec, width, options.leader);
        let toc_ids = self.toc_style_ids();
        els.extend(toc::paragraphs_with(&entries, &instr, &lay, &page, &|e| toc_ids[(e.level as usize).clamp(1, 9) - 1].clone(), "No table of contents entries found."));
        let blocks: Vec<Block> = els.iter().map(|e| Block::Para(parse_paragraph(e, &mut self.ids))).collect();
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        let before = if after.is_none() && before.is_none() { self.body.iter().find(|b| matches!(b, Block::Para(_) | Block::Table(_))).map(|b| b.id()) } else { before };
        self.insert_blocks(after, before, blocks)?;
        Ok(OpResult { created, ..Default::default() })
    }

    /// Find the table of contents holding a paragraph (or the first one in the document).
    fn find_toc(&self, at: Option<NodeId>) -> Result<ListField, EditError> {
        let mut found = Vec::new();
        collect_fields(&self.body, &is_toc, &mut found);
        let pick = match at {
            Some(p) => found.into_iter().find(|f| f.paras.contains(&p)),
            None => found.into_iter().next(),
        };
        pick.ok_or_else(|| EditError::Invalid("the document has no table of contents".into()))
    }

    /// The options of the table of contents holding a paragraph (the first one without `at`), for the dialog.
    pub fn toc_options(&self, at: Option<NodeId>) -> Option<TocOptions> {
        let f = self.find_toc(at).ok()?;
        let leader = f.paras.first().and_then(|id| find_para(&self.body, *id)).and_then(toc::leader_of).unwrap_or_else(|| self.default_toc_leader());
        Some(TocOptions::from_instr(&f.instr, leader))
    }

    /// Whether the document has a table of contents, and the id of its first paragraph.
    pub fn toc_at(&self, at: Option<NodeId>) -> Option<NodeId> {
        self.find_toc(at).ok().and_then(|f| f.paras.first().copied())
    }

    /// Change the options of a table of contents (the instruction, the leader, the look) and rebuild it.
    pub(super) fn op_set_toc(&mut self, at: Option<NodeId>, options: &TocOptions, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let f = self.find_toc(at)?;
        let first = f.paras[0];
        let instr = options.instr(Some(&f.instr));
        self.touch(first);
        let ok = set_list_instr(self.para_mut(first)?, &instr);
        if !ok {
            return Err(EditError::Invalid("could not change the table of contents".into()));
        }
        let spec = Spec::parse(&instr);
        let width = self.text_width();
        self.ensure_toc_styles(spec.max_level().clamp(3, 9));
        self.set_list_styles("TOC", spec.max_level().clamp(3, 9), Some(options.leader), options.look.as_deref(), width);
        let scope: std::collections::HashSet<NodeId> = f.paras.iter().copied().collect();
        let (created, count) = self.update_block_fields_with(pages, updating::BlockKinds { toc: true, tof: false, index: false }, Some(&scope), Some(options.leader));
        Ok(OpResult { created, count: Some(count), ..Default::default() })
    }

    /// Remove a table of contents (the one holding a paragraph, else the first), with the "Contents" title right
    /// above it. The `_Toc` bookmarks stay, as in Word.
    pub(super) fn op_remove_toc(&mut self, at: Option<NodeId>) -> Result<OpResult, EditError> {
        let f = self.find_toc(at)?;
        let mut gone: std::collections::HashSet<NodeId> = f.paras.iter().copied().collect();
        // The title: a TOC Heading paragraph just before the table.
        if let Some((list, i)) = self.body.parent_of(f.paras[0]) {
            if i > 0 {
                if let Some(Block::Para(prev)) = list.get(i - 1) {
                    let heading = prev.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).is_some_and(|s| s == "TOCHeading");
                    if heading {
                        gone.insert(prev.id);
                    }
                }
            }
        }
        for id in &gone {
            self.touch(*id);
        }
        self.remove_blocks(&gone);
        self.body_dirty = true;
        Ok(OpResult { count: Some(gone.len()), ..Default::default() })
    }

    /// Take these blocks out of the text (anywhere in it), and the content controls that end up empty. The document
    /// keeps a paragraph if nothing else is left.
    fn remove_blocks(&mut self, gone: &std::collections::HashSet<NodeId>) {
        fn remove(list: &mut ListMut, gone: &std::collections::HashSet<NodeId>) {
            list.retain(|b| !gone.contains(&b.id()));
            // A content control that only held the table goes with it. One that was already empty stays. Only a control
            // that holds one of the blocks is changed (and copied from an undo step).
            let mut emptied = std::collections::HashSet::new();
            for i in 0..list.len() {
                let holds = matches!(&list[i], Block::Container(c) if gone.iter().any(|g| find_block(&c.blocks, *g).is_some()));
                if !holds {
                    continue;
                }
                if let Some(Block::Container(c)) = list.get_mut(i) {
                    let had = !c.blocks.is_empty();
                    remove(&mut ListMut::Inner(&mut c.blocks), gone);
                    if had && c.blocks.is_empty() {
                        emptied.insert(c.id);
                    }
                }
            }
            list.retain(|b| !matches!(b, Block::Container(c) if emptied.contains(&c.id)));
        }
        remove(&mut ListMut::Top(&mut self.body), gone);
        if !self.body.iter().any(|b| matches!(b, Block::Para(_) | Block::Table(_))) {
            let id = self.ids.fresh();
            let at = self.body.len().saturating_sub(usize::from(matches!(self.body.last(), Some(Block::Opaque(o)) if o.el.name == "w:sectPr")));
            self.body.insert(at, Block::Para(Paragraph::new(id)));
        }
        self.body_dirty = true;
    }

    /// "Update page numbers only": recompute the page of every entry in the tables of contents and figures, leaving
    /// their entries as they are.
    pub(super) fn op_update_toc_pages(&mut self, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let mut found = Vec::new();
        collect_fields(&self.body, &|i| is_toc(i) || Instr::parse(i).code == "TOC", &mut found);
        if found.is_empty() {
            return Err(EditError::Invalid("the document has no table of contents".into()));
        }
        let scope: std::collections::HashSet<NodeId> = found.iter().flat_map(|f| f.paras.iter().copied()).collect();
        let changed = self.update_reference_fields(pages, Some(&scope));
        if !changed.is_empty() {
            self.body_dirty = true;
        }
        Ok(OpResult { count: Some(changed.len()), ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // The index
    // -----------------------------------------------------------------------

    /// Write a look into the `Index 1` to `Index 9` styles (`None` leaves them alone): how far each level is
    /// indented, with a hanging first line, and whether the main entries are bold.
    pub(super) fn set_index_styles(&mut self, look: Option<&str>) {
        let Some(look) = look else { return };
        let Some(doc) = self.styles.doc.as_mut() else { return };
        let mut changed = false;
        for n in 1..=9i64 {
            let id = format!("Index{n}");
            let Some(style) = doc.root.children.iter_mut().find_map(|c| match c {
                Node::Element(e) if e.name == "w:style" && e.attr("w:styleId") == Some(id.as_str()) => Some(e),
                _ => None,
            }) else {
                continue;
            };
            let (left, hanging, bold) = match look {
                "classic" => (n * 220, 220, false),
                "modern" => (n * 260, 260, n == 1),
                "simple" => (0, 0, false),
                _ => continue,
            };
            let mut ppr = style.child("w:pPr").cloned().unwrap_or_else(|| Element::new("w:pPr"));
            ppr.remove_children("w:ind");
            if left > 0 {
                crate::props::put_ppr(&mut ppr, Element::new("w:ind").with_attr("w:left", left.to_string()).with_attr("w:hanging", hanging.to_string()));
            }
            put_style_child(style, ppr);
            style.remove_children("w:rPr");
            if bold {
                put_style_child(style, Element::new("w:rPr").with_child(Element::new("w:b")).with_child(Element::new("w:bCs")));
            }
            changed = true;
        }
        if changed {
            self.styles.reload();
        }
    }

    fn find_index(&self, at: Option<NodeId>) -> Result<ListField, EditError> {
        let mut found = Vec::new();
        collect_fields(&self.body, &|i| index::is_index(i), &mut found);
        let pick = match at {
            Some(p) => found.into_iter().find(|f| f.paras.contains(&p)),
            None => found.into_iter().next(),
        };
        pick.ok_or_else(|| EditError::Invalid("the document has no index".into()))
    }

    /// The options of the index holding a paragraph (the first one without `at`), for the dialog.
    pub fn index_options(&self, at: Option<NodeId>) -> Option<IndexOptions> {
        let f = self.find_index(at).ok()?;
        let leader = f.paras.first().and_then(|id| find_para(&self.body, *id)).and_then(toc::leader_of).unwrap_or_else(|| self.style_leader("Index1").unwrap_or_default());
        Some(IndexOptions::from_instr(&f.instr, leader))
    }

    /// Change an index and build it again. With another number of columns the index is made again in a section of
    /// its own, where it was.
    pub(super) fn op_set_index(&mut self, at: Option<NodeId>, options: &IndexOptions, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let f = self.find_index(at)?;
        let old = index::Options::parse(&f.instr);
        let lcid = updating::lcid_of(self.language().as_deref());
        if options.columns.clamp(1, 4) == old.columns {
            let instr = options.instr(lcid, Some(&f.instr));
            let first = f.paras[0];
            self.touch(first);
            if !set_list_instr(self.para_mut(first)?, &instr) {
                return Err(EditError::Invalid("could not change the index".into()));
            }
            self.set_index_styles(options.look.as_deref());
            let scope: std::collections::HashSet<NodeId> = f.paras.iter().copied().collect();
            let (created, count) = self.update_block_fields_with(pages, updating::BlockKinds { toc: false, tof: false, index: true }, Some(&scope), Some(options.leader));
            return Ok(OpResult { created, count: Some(count), ..Default::default() });
        }
        // Where the index was: after the block before it, else before the block after it.
        let prev = self.body.parent_of(f.paras[0]).and_then(|(list, i)| i.checked_sub(1).and_then(|k| list.get(k)).map(|b| b.id()));
        let next = self.body.parent_of(*f.paras.last().unwrap_or(&f.paras[0])).and_then(|(list, i)| list.get(i + 1).map(|b| b.id()));
        let gone: std::collections::HashSet<NodeId> = f.paras.iter().copied().collect();
        for id in &gone {
            self.touch(*id);
        }
        self.remove_blocks(&gone);
        let (after, before) = match prev {
            Some(p) => (Some(p), None),
            None => (None, next),
        };
        self.op_insert_index(after, before, options.columns, options.run_in, options.headings, Some(options), pages)
    }

    /// Word's AutoMark: mark the first occurrence of each listed text in every paragraph, matching case. Texts the
    /// document doesn't have are skipped.
    pub(super) fn op_auto_mark(&mut self, entries: &[AutoMarkEntry]) -> Result<OpResult, EditError> {
        if entries.iter().all(|e| e.text.trim().is_empty()) {
            return Err(EditError::Invalid("The AutoMark list has no text to mark.".into()));
        }
        let mut total = 0;
        for e in entries.iter().filter(|e| !e.text.trim().is_empty()) {
            let levels = index::split_entry(if e.entry.trim().is_empty() { &e.text } else { &e.entry });
            if levels.is_empty() {
                continue;
            }
            let path: Vec<&str> = levels.iter().map(String::as_str).collect();
            let instr = index::xe_instr_path(&path, None, e.bold, e.italic);
            if let Some(n) = self.mark_all_with(e.text.trim(), &instr, false, false)? {
                total += n;
            }
        }
        Ok(OpResult { count: Some(total), ..Default::default() })
    }

    /// Add a table of contents entry (`TC` field, hidden) at a position.
    pub(super) fn op_insert_toc_entry(&mut self, at: Pos, text: &str, level: u8, id: Option<&str>, no_page: bool) -> Result<OpResult, EditError> {
        if text.trim().is_empty() {
            return Err(EditError::Invalid("Give the entry some text.".into()));
        }
        let instr = toc_spec::tc_instr(text, level, id, no_page);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        fields::insert_inlines_at(&mut p.inlines, off, fields::complex(&instr, "", None, true));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() })
    }
}

/// Give a list field (a table of contents, figures or index) a new instruction. The field's begin is in this
/// paragraph, but its end is in a later one, so the field helpers that need a closed field can't be used.
pub(super) fn set_list_instr(p: &mut Paragraph, instr: &str) -> bool {
    let mut seen_begin = false;
    let mut replaced = false;
    for inl in p.inlines.iter_mut() {
        let Inline::Run(r) = inl else { continue };
        let mut drop = Vec::new();
        let mut stop = false;
        for (k, it) in r.items.iter_mut().enumerate() {
            let RunItem::Hidden(e) = it else {
                // A tab inside the instruction (an index separator) belongs to the old text.
                if replaced && matches!(it, RunItem::Tab) {
                    drop.push(k);
                }
                continue;
            };
            match (e.name.as_str(), e.attr("w:fldCharType")) {
                ("w:fldChar", Some("begin")) if !seen_begin => seen_begin = true,
                ("w:fldChar", Some("separate")) if seen_begin => {
                    stop = true;
                    break;
                }
                ("w:instrText", _) if seen_begin => {
                    if replaced {
                        drop.push(k);
                    } else {
                        *e = Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(instr);
                        replaced = true;
                    }
                }
                _ => {}
            }
        }
        for k in drop.into_iter().rev() {
            r.items.remove(k);
        }
        if stop {
            break;
        }
    }
    p.inlines.retain(|i| !matches!(i, Inline::Run(r) if r.items.is_empty()));
    replaced
}

/// Put `pPr` or `rPr` into a `w:style` where the schema wants it: `pPr` before `rPr`, both before the table
/// properties. Anything of the same name is replaced.
pub(super) fn put_style_child(style: &mut Element, el: Element) {
    let name = el.name.clone();
    style.remove_children(&name);
    let after: &[&str] = if name == "w:pPr" { &["w:rPr", "w:tblPr", "w:trPr", "w:tcPr", "w:tblStylePr"] } else { &["w:tblPr", "w:trPr", "w:tcPr", "w:tblStylePr"] };
    let at = style.children.iter().position(|n| matches!(n, Node::Element(e) if after.contains(&e.name.as_str()))).unwrap_or(style.children.len());
    style.children.insert(at, Node::Element(el));
}

/// The paragraphs that tables of contents, tables of figures, indexes and tables of authorities show (their results), in these
/// blocks and the containers in them.
fn lists_shown_by_fields<S: crate::body::Seq + ?Sized>(blocks: &S) -> std::collections::HashSet<NodeId> {
    fn go<S: crate::body::Seq + ?Sized>(list: &S, out: &mut std::collections::HashSet<NodeId>) {
        let shows_a_list = |instr: &str| matches!(crate::fields::Instr::parse(instr).code.as_str(), "TOC" | "INDEX" | "TOA");
        for f in toc::find_where(list, &shows_a_list) {
            for k in f.start..=f.end {
                walk_paragraphs(std::slice::from_ref(list.seq_at(k)), &mut |p| {
                    out.insert(p.id);
                });
            }
        }
        for i in 0..list.seq_len() {
            if let Block::Container(c) = list.seq_at(i) {
                go(&c.blocks, out);
            }
        }
    }
    let mut out = std::collections::HashSet::new();
    go(blocks, &mut out);
    out
}
