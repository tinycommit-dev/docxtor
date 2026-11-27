//! Updating fields, and the fields that make lists: the table of contents, tables of figures and the index.
//! `UpdateFields` renumbers `SEQ` fields, rebuilds the lists, then refreshes cross-references, in that order.

use super::references::{case_format, BmRange};
use super::*;
use crate::fields::{self, Instr, Replace};
use crate::index;
use crate::toc_spec::Spec;
use crate::notes::NoteKind;
use std::collections::HashSet;

/// Which kinds of list fields to rebuild.
#[derive(Debug, Clone, Copy)]
pub(super) struct BlockKinds {
    pub toc: bool,
    pub tof: bool,
    pub index: bool,
}

enum FieldList {
    Toc,
    /// A table of figures: the label of the captions it lists.
    Tof(String),
    Index,
    /// A table of authorities. It is built from marks like the index and updates with it.
    Toa,
}

fn classify(instr: &str) -> Option<FieldList> {
    if index::is_index(instr) {
        return Some(FieldList::Index);
    }
    if crate::toa::is_toa(instr) {
        return Some(FieldList::Toa);
    }
    let i = Instr::parse(instr);
    if i.code != "TOC" {
        return None;
    }
    Some(match i.value("c") {
        Some(label) => FieldList::Tof(label),
        None => FieldList::Toc,
    })
}

/// Caption labels compare ignoring case, and a space is the same as the underscore Word puts in the field.
fn norm(label: &str) -> String {
    label.split(|c: char| c.is_whitespace() || c == '_').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ").to_lowercase()
}

pub(super) fn lcid_of(lang: Option<&str>) -> u32 {
    match lang.map(|l| l.to_lowercase()).as_deref() {
        Some("en-gb") => 2057,
        Some("es-es") => 3082,
        Some("es-mx") => 2058,
        Some("fr-fr") => 1036,
        Some("fr-ca") => 3084,
        Some("de-de") => 1031,
        Some("it-it") => 1040,
        Some("pt-br") => 1046,
        Some("pt-pt") => 2070,
        Some("nl-nl") => 1043,
        _ => 1033,
    }
}

/// Page numbering that continues from the section before: no restart (the format stays).
fn strip_restart(s: &mut Element) {
    if let Some(n) = s.child_mut("w:pgNumType") {
        n.remove_attr("w:start");
        if n.attrs.is_empty() {
            s.remove_children("w:pgNumType");
        }
    }
}

/// Where the section that holds a point in the text ends.
enum SectEnd {
    Para(NodeId),
    Final,
}

impl Document {
    // -----------------------------------------------------------------------
    // Lists made by fields
    // -----------------------------------------------------------------------

    /// The captions with this label, as table-of-figures entries (each caption gets a `_Toc` bookmark).
    fn tof_entries(&mut self, label: &str) -> Vec<toc::Entry> {
        let want = norm(label);
        let caps: Vec<_> = self.caption_infos().into_iter().filter(|c| norm(&c.id) == want).collect();
        let mut out = Vec::new();
        for c in caps {
            if c.text.is_empty() {
                continue;
            }
            if let Ok(name) = self.ensure_hidden_bookmark(c.p, BmRange::Whole, "_Toc") {
                out.push(toc::Entry { para: c.p, level: 1, text: c.text, label: None, bookmark: name, no_page: false });
            }
        }
        out
    }

    /// Every `XE` field in the main text, in order.
    fn index_marks(&self) -> Vec<index::Mark> {
        let mut out = Vec::new();
        fields::scan_blocks(self.main_body(), &mut |pid, seen| {
            if let Some(mut m) = index::parse_xe(seen.instr) {
                m.para = pid;
                out.push(m);
            }
        });
        out
    }

    /// The paragraphs of an index result for an `INDEX` instruction.
    pub(super) fn index_paragraphs(&mut self, instr: &str, marks: &[index::Mark], pages: &HashMap<NodeId, u32>, leader: Option<toc::Leader>) -> Vec<Element> {
        let opts = index::Options::parse(instr);
        let items = index::build(marks, &|p| pages.get(&p).copied(), &opts);
        let levels = items.iter().filter_map(|i| if let index::Item::Entry { level, .. } = i { Some(*level) } else { None }).max().unwrap_or(0);
        for n in 1..=(levels + 1).min(9) {
            self.styles.ensure_builtin(&format!("Index{n}"));
        }
        if opts.headings {
            self.styles.ensure_builtin("IndexHeading");
        }
        let cols = opts.columns.max(1) as i64;
        let col_width = (self.text_width() - (cols - 1) * 720) / cols;
        let leader = self.index_leader(instr, leader);
        index::paragraphs(&items, instr, &opts, col_width, leader)
    }

    /// Rebuild the tables of contents, tables of figures and indexes (as `kinds` says) from the headings,
    /// captions and index entries. Returns the new paragraphs and how many fields were rebuilt. With `scope`,
    /// only fields that have a paragraph in it.
    pub(super) fn update_block_fields(&mut self, pages: Option<&HashMap<String, u32>>, kinds: BlockKinds, scope: Option<&HashSet<NodeId>>) -> (Vec<NodeId>, usize) {
        self.update_block_fields_with(pages, kinds, scope, None)
    }

    /// Like `update_block_fields`. `leader` is the tab leader to give the tables of contents and the index (else
    /// the one their old result used, else the one in their styles).
    pub(super) fn update_block_fields_with(&mut self, pages: Option<&HashMap<String, u32>>, kinds: BlockKinds, scope: Option<&HashSet<NodeId>>, leader: Option<toc::Leader>) -> (Vec<NodeId>, usize) {
        fn wanted(instr: &str, kinds: BlockKinds) -> bool {
            match classify(instr) {
                Some(FieldList::Toc) => kinds.toc,
                Some(FieldList::Tof(_)) => kinds.tof,
                Some(FieldList::Index | FieldList::Toa) => kinds.index,
                None => false,
            }
        }
        fn gather(blocks: &[Block], kinds: BlockKinds, out: &mut Vec<String>) {
            out.extend(toc::find_where(blocks, &|i| wanted(i, kinds)).into_iter().map(|f| f.instr));
            for b in blocks {
                if let Block::Container(c) = b {
                    gather(&c.blocks, kinds, out);
                }
            }
        }
        let mut found = Vec::new();
        found.extend(toc::find_where(&self.body, &|i| wanted(i, kinds)).into_iter().map(|f| f.instr));
        for b in &self.body {
            if let Block::Container(c) = b {
                gather(&c.blocks, kinds, &mut found);
            }
        }
        if found.is_empty() {
            return (vec![], 0);
        }
        // Building the lists adds bookmarks to the text, so the body has to be written.
        self.body_dirty = true;
        let kinds_found: Vec<FieldList> = found.iter().filter_map(|i| classify(i)).collect();
        // Entries for each table of contents (one list per distinct instruction).
        let mut toc_lists: HashMap<String, Vec<toc::Entry>> = HashMap::new();
        for (k, i) in kinds_found.iter().zip(&found) {
            if matches!(k, FieldList::Toc) && !toc_lists.contains_key(i) {
                let spec = Spec::parse(i);
                let list = self.toc_entries_for(&spec);
                let deepest = list.iter().map(|e| e.level).max().unwrap_or(1).max(spec.max_level().min(3));
                self.ensure_toc_styles(deepest);
                toc_lists.insert(i.clone(), list);
            }
        }
        let default_leader = self.default_toc_leader();
        // Captions for each table of figures.
        let mut figure_entries: HashMap<String, Vec<toc::Entry>> = HashMap::new();
        for k in &kinds_found {
            if let FieldList::Tof(label) = k {
                if let std::collections::hash_map::Entry::Vacant(slot) = figure_entries.entry(norm(label)) {
                    self.styles.ensure_builtin("TableofFigures");
                    slot.insert(self.tof_entries(label));
                }
            }
        }
        // Index entries and the page of everything.
        let marks = if kinds_found.iter().any(|k| matches!(k, FieldList::Index)) { self.index_marks() } else { vec![] };
        let page_map = self.page_map(pages);
        let page = self.toc_page_fn(pages);
        let width = self.text_width();
        // Index paragraphs depend on the styles, so build them before the body is taken apart.
        let mut index_results: HashMap<String, Vec<Element>> = HashMap::new();
        for (k, i) in kinds_found.iter().zip(&found) {
            if matches!(k, FieldList::Index) && !index_results.contains_key(i) {
                let els = self.index_paragraphs(i, &marks, &page_map, leader);
                index_results.insert(i.clone(), els);
            }
        }
        let mut toa_results: HashMap<String, Vec<Element>> = HashMap::new();
        for (k, i) in kinds_found.iter().zip(&found) {
            if matches!(k, FieldList::Toa) && !toa_results.contains_key(i) {
                let els = self.toa_paragraphs(i, &page_map, leader);
                toa_results.insert(i.clone(), els);
            }
        }
        let mut created = Vec::new();
        let mut count = 0usize;
        // A file may define the table of contents styles under other ids (a localised Word does).
        let toc_ids = self.toc_style_ids();
        let ids = &mut self.ids;
        let mut make = |instr: &str, old: Option<toc::Leader>| -> Vec<Element> {
            match classify(instr) {
                Some(FieldList::Toc) => {
                    let spec = Spec::parse(instr);
                    let mine = toc_lists.get(instr).cloned().unwrap_or_default();
                    let lay = toc::Layout::of(&spec, width, leader.or(old).unwrap_or(default_leader));
                    toc::paragraphs_with(&mine, instr, &lay, &page, &|e| toc_ids[(e.level as usize).clamp(1, 9) - 1].clone(), "No table of contents entries found.")
                }
                Some(FieldList::Tof(label)) => {
                    let mine = figure_entries.get(&norm(&label)).cloned().unwrap_or_default();
                    toc::paragraphs_styled(&mine, instr, width, &page, &|_| "TableofFigures".into(), "No table of figures entries found.")
                }
                Some(FieldList::Index) => index_results.get(instr).cloned().unwrap_or_default(),
                Some(FieldList::Toa) => toa_results.get(instr).cloned().unwrap_or_default(),
                None => vec![],
            }
        };
        /// Whether these blocks, or the content controls in them, hold a field of the kinds.
        fn any_wanted(blocks: &[Block], kinds: BlockKinds, scope: Option<&HashSet<NodeId>>) -> bool {
            toc::find_where(blocks, &|i| wanted(i, kinds)).iter().any(|f| scope.is_none_or(|s| blocks[f.start..=f.end].iter().any(|b| s.contains(&b.id()))))
                || blocks.iter().any(|b| matches!(b, Block::Container(c) if any_wanted(&c.blocks, kinds, scope)))
        }
        fn replace(
            list: &mut ListMut,
            kinds: BlockKinds,
            scope: Option<&HashSet<NodeId>>,
            make: &mut dyn FnMut(&str, Option<toc::Leader>) -> Vec<Element>,
            ids: &mut IdGen,
            created: &mut Vec<NodeId>,
            count: &mut usize,
        ) {
            for f in toc::find_where(&*list, &|i| wanted(i, kinds)).into_iter().rev() {
                let old: Vec<NodeId> = (f.start..=f.end).map(|k| list[k].id()).collect();
                if scope.is_some_and(|s| !old.iter().any(|id| s.contains(id))) {
                    continue;
                }
                // A section break can sit in the field's last paragraph (Word's index does that): keep it.
                let sect = (f.start..=f.end).rev().find_map(|k| match &list[k] {
                    Block::Para(p) => p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).cloned(),
                    _ => None,
                });
                let old_leader = match &list[f.start] {
                    Block::Para(p) => toc::leader_of(p),
                    _ => None,
                };
                let els = make(&f.instr, old_leader);
                let mut new: Vec<Block> = els.iter().map(|e| Block::Para(parse_paragraph(e, ids))).collect();
                if let (Some(s), Some(Block::Para(last))) = (sect, new.last_mut()) {
                    put_ppr(last.ppr.get_or_insert_with(|| Element::new("w:pPr")), s);
                }
                created.extend(new.iter().map(|b| b.id()));
                *count += 1;
                list.splice(f.start..f.end + 1, new);
            }
            // Only a content control that holds such a field is opened for changing (and copied from an undo step).
            for i in 0..list.len() {
                if !matches!(&list[i], Block::Container(c) if any_wanted(&c.blocks, kinds, scope)) {
                    continue;
                }
                if let Some(Block::Container(c)) = list.get_mut(i) {
                    replace(&mut ListMut::Inner(&mut c.blocks), kinds, scope, make, ids, created, count);
                }
            }
        }
        replace(&mut ListMut::Top(&mut self.body), kinds, scope, &mut make, ids, &mut created, &mut count);
        for id in &created {
            self.touch(*id);
        }
        (created, count)
    }

    /// Recompute `REF`, `PAGEREF` and `NOTEREF` results from the bookmarks they point at (main text, headers,
    /// footers and notes). A field whose bookmark is gone keeps its old result. Returns the paragraphs changed.
    pub(super) fn update_reference_fields(&mut self, pages: Option<&HashMap<String, u32>>, scope: Option<&HashSet<NodeId>>) -> Vec<NodeId> {
        let bms = self.bookmark_infos();
        let labels = self.numbering.labels(&self.body, &self.styles);
        let page_of = self.page_map(pages);
        let page_labels = self.page_labels();
        let order: HashMap<NodeId, usize> = paragraph_order(self.main_body()).into_iter().enumerate().map(|(i, p)| (p, i)).collect();
        // The note marks in the text, by paragraph.
        let mut note_marks: HashMap<NodeId, Vec<(usize, NoteKind, String)>> = HashMap::new();
        walk_paragraphs(self.main_body(), &mut |p| {
            walk_runs(&p.inlines, &mut 0, &mut |r, start, _| {
                let mut pos = start;
                for it in &r.items {
                    if let RunItem::Object(e) = it {
                        for kind in [NoteKind::Footnote, NoteKind::Endnote] {
                            if e.name == kind.reference_tag() {
                                if let Some(id) = e.attr("w:id") {
                                    note_marks.entry(p.id).or_default().push((pos, kind, id.to_string()));
                                }
                            }
                        }
                    }
                    pos += it.len16();
                }
            });
        });
        let (ff, ef) = self.note_formats();
        let (fnums, enums) = (self.note_nums(NoteKind::Footnote), self.note_nums(NoteKind::Endnote));
        let ref_style = |k: NoteKind| self.styles.lookup(k.ref_style()).unwrap_or_else(|| k.ref_style().to_string());
        let (fstyle, estyle) = (ref_style(NoteKind::Footnote), ref_style(NoteKind::Endnote));
        let chains = self.number_chains(&labels);
        let number_of = |pid: NodeId, host: NodeId, i: &Instr| -> Option<String> {
            let mode = if i.has("w") { 'w' } else if i.has("n") { 'n' } else { 'r' };
            Self::numbered_text(&labels, &chains, pid, Some(host), mode)
        };
        let mut compute = |pid: NodeId, seen: &fields::Seen| -> Option<Replace> {
            let i = Instr::parse(seen.instr);
            if !matches!(i.code.as_str(), "REF" | "PAGEREF" | "NOTEREF") || scope.is_some_and(|s| !s.contains(&pid)) {
                return None;
            }
            let bm = bms.get(&fields::unescape(i.first_arg()?).to_lowercase())?;
            let position = || {
                let before = match (order.get(&bm.p), order.get(&pid)) {
                    (Some(a), Some(b)) if a != b => a < b,
                    _ => bm.o <= seen.offset,
                };
                (if before { "above" } else { "below" }).to_string()
            };
            let has_number = i.has("r") || i.has("n") || i.has("w");
            match i.code.as_str() {
                "REF" => {
                    if i.has("p") {
                        return if has_number { None } else { Some(position().into()) };
                    }
                    let text = if has_number { number_of(bm.p, pid, &i)? } else if bm.name.starts_with('_') { bm.text.trim().to_string() } else { bm.text.clone() };
                    Some(case_format(&text, &i).into())
                }
                "PAGEREF" => {
                    if i.has("p") {
                        return Some(position().into());
                    }
                    page_of.get(&bm.p).map(|n| page_labels.label(bm.p, *n).into())
                }
                _ => {
                    if i.has("p") {
                        return Some(position().into());
                    }
                    let (_, kind, id) = note_marks.get(&bm.p)?.iter().find(|(o, _, _)| *o >= bm.o && (bm.end_p != bm.p || *o < bm.end_o.max(bm.o + 1)))?;
                    let (num, fmt) = if *kind == NoteKind::Endnote { (enums.get(id)?, &ef) } else { (fnums.get(id)?, &ff) };
                    Some(Replace { text: fmt.label(*num), rstyle: i.has("f").then(|| if *kind == NoteKind::Endnote { estyle.clone() } else { fstyle.clone() }), ..Default::default() })
                }
            }
        };
        let mut changed = fields::update_blocks_body(&mut self.body, &mut compute);
        for id in &changed {
            self.touch(*id);
        }
        for si in 0..self.stories.len() {
            let ch = fields::update_blocks(&mut self.stories[si].blocks, &mut compute);
            if !ch.is_empty() {
                let tops = ch.iter().filter_map(|id| crate::model::top_level_of(&self.stories[si].blocks, *id)).collect();
                self.story_edited(si, tops);
                self.story_gen += 1;
                self.stories[si].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[si].part.clone());
                self.stories_touched = true;
                changed.extend(ch);
            }
        }
        changed
    }

    pub(super) fn op_update_fields(&mut self, pages: Option<&HashMap<String, u32>>, scope: Option<&[Id]>, extra: &super::field_ops::UpdateExtra) -> Result<OpResult, EditError> {
        let scope: Option<HashSet<NodeId>> = scope.map(|s| s.iter().map(|i| i.0).collect());
        // Opening a file updates dates, and everything when the file asks for that.
        let all_on_open = extra.open && self.updates_fields_on_open();
        let numbered = (!extra.open || all_on_open) && extra.only.is_none();
        let mut seq = Vec::new();
        let mut lists = 0;
        let mut created = Vec::new();
        let mut refs = Vec::new();
        let mut cites = 0;
        if numbered {
            // Sequence numbers first: lists and references read them.
            seq = self.renumber_seq(scope.as_ref());
            (created, lists) = self.update_block_fields(pages, BlockKinds { toc: true, tof: true, index: true }, scope.as_ref());
            refs = self.update_reference_fields(pages, scope.as_ref());
            // Citations and bibliographies too, when the whole document is updated (they mark what they change).
            cites = if scope.is_none() { self.update_citations() } else { 0 };
        }
        // Dates, properties, page numbers, `IF`, formulas and the like.
        let input = extra.input.clone().unwrap_or_default();
        let values = self.update_value_fields(&super::field_ops::ValueOpts { scope: scope.as_ref(), pages, input: &input, open: extra.open && !all_on_open, only: extra.only.as_deref() });
        if all_on_open {
            self.clear_update_fields_setting();
        }
        let count = seq.len() + lists + refs.len() + cites + values.len();
        // A document with nothing to update stays as it was, byte for byte.
        if !seq.is_empty() || lists > 0 || refs.iter().any(|id| find_block(&self.body, *id).is_some()) {
            self.body_dirty = true;
        }
        Ok(OpResult { created, count: Some(count), ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Index entries
    // -----------------------------------------------------------------------

    pub(super) fn op_mark_index_entry(&mut self, at: Pos, entry: &str, sub: Option<&str>, see: Option<&str>, bold: bool, italic: bool) -> Result<OpResult, EditError> {
        if entry.trim().is_empty() {
            return Err(EditError::Invalid("Give the index entry some text.".into()));
        }
        let instr = index::xe_instr(entry, sub, see, bold, italic);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        fields::insert_inlines_at(&mut p.inlines, off, fields::complex(&instr, "", None, true));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() })
    }

    /// Mark the first occurrence of `text` in each paragraph of the main text (Word's Mark All).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn op_mark_all_index_entries(&mut self, text: &str, entry: Option<&str>, sub: Option<&str>, see: Option<&str>, bold: bool, italic: bool, ignore_case: bool, whole_word: bool) -> Result<OpResult, EditError> {
        if text.trim().is_empty() {
            return Err(EditError::Invalid("Give the text to mark.".into()));
        }
        let entry = entry.filter(|e| !e.trim().is_empty()).unwrap_or(text);
        let instr = index::xe_instr(entry, sub, see, bold, italic);
        match self.mark_all_with(text, &instr, ignore_case, whole_word)? {
            Some(n) => Ok(OpResult { count: Some(n), ..Default::default() }),
            None => Err(EditError::Invalid(format!("The text {text:?} isn't in the document."))),
        }
    }

    /// Put the `XE` field `instr` after the first occurrence of `text` in each paragraph that doesn't have the entry
    /// yet. None when no paragraph has the text at all.
    pub(super) fn mark_all_with(&mut self, text: &str, instr: &str, ignore_case: bool, whole_word: bool) -> Result<Option<usize>, EditError> {
        let wanted = index::parse_xe(instr).map(|m| m.levels);
        let mut hits: Vec<(NodeId, usize)> = Vec::new();
        let mut present = false;
        walk_paragraphs(&self.body, &mut |p| {
            let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or_default();
            if style.starts_with("TOC") || style.starts_with("Index") || style == "TableofFigures" {
                return;
            }
            let hay = p.text();
            let mut from = 0;
            while let Some((start, end, _)) = find_text(&hay, text, from, ignore_case) {
                let u: Vec<u16> = hay.encode_utf16().collect();
                let word = |i: usize| u.get(i).and_then(|c| char::from_u32(*c as u32)).is_some_and(|c| c.is_alphanumeric());
                if !whole_word || ((start == 0 || !word(start - 1)) && !word(end)) {
                    present = true;
                    // Already marked with this entry?
                    let mut marked = false;
                    fields::scan_inlines(&p.inlines, 0, &mut |s| marked |= index::parse_xe(s.instr).is_some_and(|m| Some(m.levels) == wanted));
                    if !marked {
                        hits.push((p.id, end));
                    }
                    return;
                }
                from = start + 1;
            }
        });
        if !present {
            return Ok(None);
        }
        for (pid, end) in &hits {
            self.touch(*pid);
            let p = self.para_mut(*pid)?;
            let off = (*end).min(p.len16());
            fields::insert_inlines_at(&mut p.inlines, off, fields::complex(instr, "", None, true));
        }
        Ok(Some(hits.len()))
    }

    // -----------------------------------------------------------------------
    // Inserting an index and a table of figures
    // -----------------------------------------------------------------------

    pub(super) fn op_insert_table_of_figures(&mut self, after: Option<NodeId>, before: Option<NodeId>, label: &str, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let label = label.trim();
        if label.is_empty() {
            return Err(EditError::Invalid("Say which captions to list, such as Figure or Table.".into()));
        }
        self.styles.ensure_builtin("TableofFigures");
        let entries = self.tof_entries(label);
        let page = self.toc_page_fn(pages);
        let instr = format!(" TOC \\h \\z \\c \"{}\" ", fields::escape(label, false));
        let els = toc::paragraphs_styled(&entries, &instr, self.text_width(), &page, &|_| "TableofFigures".into(), "No table of figures entries found.");
        let blocks: Vec<Block> = els.iter().map(|e| Block::Para(parse_paragraph(e, &mut self.ids))).collect();
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        self.insert_blocks(after, before, blocks)?;
        Ok(OpResult { created, ..Default::default() })
    }

    /// The `w:sectPr` that ends the section holding the top-level position `idx` (before the block at `idx`).
    fn section_end_from(&self, idx: usize) -> SectEnd {
        for b in self.body.iter().skip(idx) {
            if let Block::Para(p) = b {
                if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) {
                    return SectEnd::Para(p.id);
                }
            }
        }
        SectEnd::Final
    }

    fn sect_end_mut(&mut self, end: &SectEnd) -> Option<&mut Element> {
        match end {
            SectEnd::Final => Some(&mut self.final_sect_mut().el),
            SectEnd::Para(id) => self.body.para_mut(*id)?.ppr.as_mut()?.child_mut("w:sectPr"),
        }
    }

    /// Insert an index in a continuous section of its own with `columns` columns, as Word does: a section
    /// break before it (ending the text so far with the page setup it had), the index section, and the rest of
    /// the text continuing after it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn op_insert_index(&mut self, after: Option<NodeId>, before: Option<NodeId>, columns: u8, run_in: bool, headings: bool, options: Option<&index::IndexOptions>, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let opts = options.cloned().unwrap_or(index::IndexOptions { columns, run_in, headings, ..Default::default() });
        let columns = opts.columns.clamp(1, 4);
        // The top-level position to insert at.
        let idx = match after.or(before) {
            Some(a) => {
                let top = top_level_of(&self.body, a).ok_or(EditError::NotFound(a))?;
                let i = self.body.iter().position(|b| b.id() == top).ok_or(EditError::NotFound(a))?;
                // The closing section properties stay the last block of the body: "after" them means before them.
                let closing = matches!(&self.body[i], Block::Opaque(o) if o.el.name == "w:sectPr") && i + 1 == self.body.len();
                if after.is_some() && !closing {
                    i + 1
                } else {
                    i
                }
            }
            None => self.body.iter().rposition(|b| !matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).map(|i| i + 1).unwrap_or(0),
        };
        let instr = opts.instr(lcid_of(self.language().as_deref()), None);
        let marks = self.index_marks();
        let page_map = self.page_map(pages);
        let els = self.index_paragraphs(&instr, &marks, &page_map, Some(opts.leader));
        // After the paragraphs: building them adds the Index styles the look is written into.
        self.set_index_styles(opts.look.as_deref());
        let mut blocks: Vec<Block> = els.iter().map(|e| Block::Para(parse_paragraph(e, &mut self.ids))).collect();
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        let mut extra = Vec::new();
        if columns > 1 {
            let end = self.section_end_from(idx);
            let t = self.sect_end_mut(&end).map(|e| e.clone()).unwrap_or_else(|| Element::new("w:sectPr"));
            let prev_ends_section = idx > 0 && matches!(&self.body[idx - 1], Block::Para(p) if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()));
            let needs_prev = idx > 0 && !prev_ends_section;
            let original_type = t.child("w:type").and_then(|e| e.attr("w:val")).map(String::from);
            // What a later part of a section must not repeat: its headers, first-page setting and numbering restart.
            let strip = |s: &mut Element| {
                s.remove_children("w:headerReference");
                s.remove_children("w:footerReference");
                s.remove_children("w:titlePg");
                strip_restart(s);
            };
            let mut idx_sect = t.clone();
            let mut prev_sect = t.clone();
            if needs_prev {
                strip(&mut idx_sect);
                set_sect_type(&mut idx_sect, "continuous");
            } else if original_type.is_none() {
                idx_sect.remove_children("w:type");
            }
            let mut cols = idx_sect.child("w:cols").cloned().unwrap_or_else(|| Element::new("w:cols"));
            cols.set_attr("w:num", columns.to_string());
            if cols.attr("w:space").is_none() {
                cols.set_attr("w:space", "720");
            }
            cols.remove_attr("w:equalWidth");
            cols.remove_children("w:col");
            crate::props::put_ordered(&mut idx_sect, cols, SECT_ORDER);
            if !needs_prev {
                prev_sect = Element::new("w:sectPr");
            }
            // The rest of the old section now starts after the index.
            if let Some(rest) = self.sect_end_mut(&end) {
                if needs_prev {
                    strip(rest);
                } else {
                    rest.remove_children("w:titlePg");
                    strip_restart(rest);
                }
                set_sect_type(rest, "continuous");
            }
            if let Some(Block::Para(last)) = blocks.last_mut() {
                put_ppr(last.ppr.get_or_insert_with(|| Element::new("w:pPr")), idx_sect);
            }
            if needs_prev {
                match &mut self.body[idx - 1] {
                    Block::Para(p) => put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), prev_sect),
                    _ => {
                        // After a table or another block: Word puts the break in a paragraph of its own.
                        let id = self.ids.fresh();
                        let mut p = Paragraph::new(id);
                        put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), prev_sect);
                        extra.push(Block::Para(p));
                    }
                }
            }
        }
        let mut all = extra;
        all.extend(blocks);
        // The section after the index needs a paragraph to hold its content.
        let next_is_content = matches!(self.body.get(idx), Some(Block::Para(_) | Block::Table(_) | Block::Container(_)));
        if columns > 1 && !next_is_content {
            let id = self.ids.fresh();
            all.push(Block::Para(Paragraph::new(id)));
        }
        for (k, b) in all.into_iter().enumerate() {
            let id = b.id();
            self.body.insert(idx + k, b);
            self.touch(id);
        }
        if idx > 0 {
            let prev = self.body[idx - 1].id();
            self.touch(prev);
        }
        self.touch_all();
        Ok(OpResult { created, ..Default::default() })
    }
}
