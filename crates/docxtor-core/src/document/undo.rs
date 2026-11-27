//! Undo and redo: the snapshots taken before an edit, restoring them, the undo budget, pinned versions, and the read-only access check.

use super::*;

impl Document {
    /// Every "changed since the last save" mark, so something that writes parts out without saving can put them back.
    pub(super) fn change_marks(&self) -> ChangeMarks {
        ChangeMarks {
            body: self.body_dirty,
            styles: self.styles.dirty,
            numbering: self.numbering.dirty,
            settings: self.settings_dirty,
            core: self.core_dirty,
            custom: self.custom_dirty,
            app: self.app_dirty,
            charts: self.charts_dirty,
            theme: self.theme_dirty,
            cites: self.cites.dirty,
            merge: self.merge.dirty,
            tabs: self.tabs.dirty,
            parts: self.dirty_parts.clone(),
        }
    }

    pub(super) fn restore_change_marks(&mut self, m: ChangeMarks) {
        self.body_dirty = m.body;
        self.styles.dirty = m.styles;
        self.numbering.dirty = m.numbering;
        self.settings_dirty = m.settings;
        self.core_dirty = m.core;
        self.custom_dirty = m.custom;
        self.app_dirty = m.app;
        self.charts_dirty = m.charts;
        self.theme_dirty = m.theme;
        self.cites.dirty = m.cites;
        self.merge.dirty = m.merge;
        self.tabs.dirty = m.tabs;
        self.dirty_parts = m.parts;
    }

    pub(super) fn snapshot(&mut self) -> Snapshot {
        let mut fresh_bytes = 0;
        if crate::perf::plant_double_clone() {
            crate::perf::body_cloned();
            std::hint::black_box(self.body.to_vec());
        }
        let mut fresh_block_bytes = 0;
        let body = self.shared_body(&mut fresh_block_bytes);
        Snapshot {
            body,
            styles: shared_copy(&mut self.copies.styles, &self.styles.doc, &mut fresh_bytes),
            numbering: self.shared_numbering(&mut fresh_bytes),
            stories: self.stories.clone(),
            story_parts: shared_story_parts(&mut self.copies.story_parts, &self.story_parts, &mut fresh_bytes),
            background: self.doc_xml.root.child("w:background").cloned(),
            root_attrs: self.doc_xml.root.attrs.clone(),
            settings: shared_copy(&mut self.copies.settings, &self.settings, &mut fresh_bytes),
            settings_dirty: self.settings_dirty,
            core: self.core.clone(),
            core_dirty: self.core_dirty,
            custom: self.custom.clone(),
            custom_dirty: self.custom_dirty,
            app: self.app.clone(),
            app_dirty: self.app_dirty,
            cites: self.cites.clone(),
            merge: self.merge.clone(),
            tabs: self.tabs.clone(),
            theme: self.theme.clone(),
            scrub: self.scrub.clone(),
            pkg_delta: Vec::new(),
            bytes: 0,
            fresh_bytes,
            fresh_block_bytes,
        }
    }

    pub(super) fn shared_numbering(&mut self, fresh: &mut usize) -> Option<std::sync::Arc<XmlDoc>> {
        if self.copies.numbering_gen == self.numbering.gen && self.copies.numbering.is_some() == self.numbering.doc.is_some() {
            #[cfg(any(test, debug_assertions))]
            assert!(self.copies.numbering.as_deref() == self.numbering.doc.as_ref(), "the numbering part changed without `Numbering::touch`: the op `{}`", self.snap_op);
            return self.copies.numbering.clone();
        }
        let out = shared_copy(&mut self.copies.numbering, &self.numbering.doc, fresh);
        self.copies.numbering_gen = self.numbering.gen;
        out
    }

    /// The copy of the top-level block `id` in `prev` (the blocks as the last snapshot left them): the one at position `i`
    /// when it is there, else found by id through `by_id`, which is built the first time it is needed.
    pub(super) fn prev_copy<'a>(prev: &'a [std::sync::Arc<Block>], by_id: &mut Option<HashMap<NodeId, usize>>, i: usize, id: NodeId) -> Option<&'a std::sync::Arc<Block>> {
        match prev.get(i) {
            Some(a) if a.id() == id => Some(a),
            _ => {
                let m = by_id.get_or_insert_with(|| prev.iter().enumerate().map(|(i, a)| (a.id(), i)).collect());
                m.get(&id).map(|&j| &prev[j])
            }
        }
    }

    /// The body for a snapshot: every top-level block as a shared copy. The live body shares its blocks with the steps
    /// (see `Body`), so this only counts references. A block the live body changed since the last snapshot was copied
    /// for writing then (`Body::get_mut`), so it is not the last snapshot's copy any more: that is what makes it fresh,
    /// and `fresh` grows by its size. Typing in a 9,000-paragraph novel copies one paragraph, not the book.
    ///
    /// In debug builds and tests a block that is not the last snapshot's copy and was not marked since panics naming
    /// the op: it means something changed it without marking it (`touch`), which would also leave it stale on screen, or
    /// copied it for writing without changing it.
    pub(super) fn shared_body(&mut self, fresh: &mut usize) -> std::sync::Arc<Vec<std::sync::Arc<Block>>> {
        let _dirty = std::mem::take(&mut self.changed.since_copy);
        let prev = self.copies.blocks.take();
        let mut by_id = None;
        let out = std::sync::Arc::new(self.body.arcs().to_vec());
        for (i, a) in out.iter().enumerate() {
            let id = a.id();
            match Self::prev_copy(prev.as_deref().map_or(&[], |v| v.as_slice()), &mut by_id, i, id) {
                Some(p) if std::sync::Arc::ptr_eq(p, a) => {}
                _last => {
                    #[cfg(any(test, debug_assertions))]
                    if let Some(p) = _last {
                        assert!(_dirty.contains(&id), "undo copy of block {id} is stale: the op `{}` {} without touching it", self.snap_op, if **p == **a { "copied it" } else { "changed it" });
                    }
                    *fresh += crate::model::blocks_bytes(std::slice::from_ref(&**a));
                }
            }
        }
        self.copies.blocks = Some(out.clone());
        out
    }

    /// Put the body back as `then` has it: the steps' own blocks go in as the body, and only the top-level blocks that
    /// are not the live ones (not the same shared copy) are marked as changed for drawing. A block the live body still
    /// shares with `then` is the same as `then`'s without looking at it. Blocks added, removed and moved are handled by id.
    pub(super) fn restore_body(&mut self, then: std::sync::Arc<Vec<std::sync::Arc<Block>>>) {
        self.changed.since_copy.clear();
        let old = std::mem::take(&mut self.body);
        let old = old.arcs();
        let same_shape = old.len() == then.len() && old.iter().zip(then.iter()).all(|(a, b)| a.id() == b.id());
        let by_id: HashMap<NodeId, &std::sync::Arc<Block>> = if same_shape { HashMap::new() } else { old.iter().map(|a| (a.id(), a)).collect() };
        for (i, t) in then.iter().enumerate() {
            let live = if same_shape { Some(&old[i]) } else { by_id.get(&t.id()).copied() };
            if !live.is_some_and(|l| std::sync::Arc::ptr_eq(l, t)) {
                self.changed.insert(t.id());
            }
        }
        // Blocks that were there and are not any more: nothing to draw, the order says they are gone.
        self.body = Body::from_arcs((*then).clone());
        self.copies.blocks = Some(then);
        // What was marked while putting back is equal to its copy.
        self.changed.since_copy.clear();
    }

    /// Give back everything a refused batch touched, including what counts as changed: a refused edit leaves no trace,
    /// so the document is not "modified" by it and the next save writes the same bytes.
    pub(super) fn refuse_batch(&mut self, before: Snapshot, marks: ChangeMarks) {
        self.pkg.rollback_journal();
        self.restore(before);
        self.restore_change_marks(marks);
    }

    /// Whether an undo or redo's package parts include the main part's relationships or its styles, numbering or settings part.
    pub(super) fn puts_back_main_parts(&self, swapped: &[package::PartDelta]) -> bool {
        // Most steps put back no part at all: nothing is built for them.
        swapped.iter().any(|(n, _)| {
            *n == self.styles_path
                || *n == self.numbering_path
                || *n == self.settings_path
                || n.starts_with("customXml/")
                || (n.ends_with(".rels") && *n == package::rels_path(&self.main_path))
        })
    }

    /// After an undo or redo that put back the main part's relationships (or one of these parts) as they were when its step ran: a
    /// save since can have added or taken out a part they name. Whether the text has the part, the package has it and the main part
    /// links to it must agree, or the save writes the part again (or takes it out). Nothing is read unless the step put one back.
    pub(super) fn parts_agree_after(&mut self) {
        // A custom XML part (the document tabs, the citation sources, mail merge's state, the cover page's properties) that a save took
        // out while the relationships put back still name it: the relationship goes. Nothing in the text refers to one of these.
        let main = self.main_path.clone();
        let gone: Vec<String> = self
            .pkg
            .rels_for(&main)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| !r.external && r.rel_type == crate::citations::REL_CUSTOM_XML)
            .map(|r| package::resolve_target(&main, &r.target))
            .filter(|t| !self.pkg.has(t))
            .collect();
        for t in &gone {
            self.pkg.remove_rels_to(&main, t);
        }
        // Document tabs whose part or its relationship the step did not put back are written again.
        if self.tabs.list.is_some() && !self.tabs.path.as_deref().is_some_and(|p| self.pkg.has(p) && self.main_links_to(p)) {
            self.tabs.dirty = true;
        }
        let styles_path = self.styles_path.clone();
        if !self.part_and_link_agree(&styles_path, self.styles.doc.is_some()) {
            self.styles.dirty = true;
        }
        let numbering_path = self.numbering_path.clone();
        if !self.part_and_link_agree(&numbering_path, self.numbering.doc.is_some()) {
            self.numbering.dirty = true;
        }
        let settings_path = self.settings_path.clone();
        if !self.part_and_link_agree(&settings_path, self.settings.is_some()) {
            self.settings_dirty = true;
        }
    }

    pub(super) fn restore(&mut self, s: Snapshot) {
        // Parts a block's drawing may depend on: when any of them differs, every block is drawn again, as the blocks that
        // differ are not the only ones that look different.
        let looks_differ = s.styles.as_deref() != self.styles.doc.as_ref()
            || s.numbering.as_deref() != self.numbering.doc.as_ref()
            || s.settings.as_deref() != self.settings.as_ref()
            || s.cites != self.cites
            || s.merge != self.merge
            || s.tabs != self.tabs
            || s.scrub != self.scrub
            || s.background != self.doc_xml.root.child("w:background").cloned()
            || s.root_attrs != self.doc_xml.root.attrs;
        self.restore_body(s.body);
        self.restore_theme(s.theme);
        // Rewrite every story part whose content differs from the restored state.
        let key = |st: &Story| (st.part.clone(), st.note_id.clone(), st.gen);
        let now: BTreeSet<_> = self.stories.iter().map(key).collect();
        let then: BTreeSet<_> = s.stories.iter().map(key).collect();
        for (part, _, _) in now.symmetric_difference(&then) {
            self.dirty_parts.insert(part.clone());
        }
        let mut parts_changed = s.story_parts.len() != self.story_parts.parts.len();
        for (part, doc) in &s.story_parts {
            if self.story_parts.get(part) != Some(&**doc) {
                self.dirty_parts.insert(part.clone());
                parts_changed = true;
            }
        }
        if now != then || parts_changed {
            self.stories_touched = true;
        }
        self.stories = s.stories;
        if parts_changed {
            self.story_parts = StoryParts { parts: s.story_parts.iter().map(|(n, d)| (n.clone(), (**d).clone())).collect() };
        }
        // A part the step takes back to what the package holds (the file as opened, or as last saved) is left as it is there: the save
        // copies its bytes, as for a part never edited, instead of writing it again (the lossless rule).
        if s.styles.as_deref() != self.styles.doc.as_ref() {
            self.styles = StyleSheet::parse(s.styles.as_deref().cloned(), self.styles.theme.clone());
            self.styles.dirty = !self.as_package_holds(&self.styles_path, self.styles.doc.as_ref());
        }
        if s.numbering.as_deref() != self.numbering.doc.as_ref() {
            let rels = std::mem::take(&mut self.numbering.rels);
            self.numbering = Numbering::parse(s.numbering.as_deref().cloned());
            self.numbering.rels = rels;
            self.numbering.dirty = !self.as_package_holds(&self.numbering_path, self.numbering.doc.as_ref());
        }
        if s.settings.as_deref() != self.settings.as_ref() {
            self.settings_dirty = !self.as_package_holds(&self.settings_path, s.settings.as_deref());
        } else {
            self.settings_dirty |= s.settings_dirty;
        }
        self.settings = s.settings.as_deref().cloned();
        if s.core != self.core {
            self.core_dirty = true;
        } else {
            self.core_dirty |= s.core_dirty;
        }
        self.core = s.core;
        if s.custom != self.custom {
            self.custom_dirty = true;
        } else {
            self.custom_dirty |= s.custom_dirty;
        }
        self.custom = s.custom;
        if s.app != self.app {
            self.app_dirty = true;
        } else {
            self.app_dirty |= s.app_dirty;
        }
        self.app = s.app;
        self.cites.restore(s.cites);
        self.scrub = s.scrub;
        self.merge.restore(s.merge);
        self.tabs.restore(s.tabs);
        self.set_background(s.background);
        self.doc_xml.root.attrs = s.root_attrs;
        self.body_dirty = true;
        if looks_differ || now != then || parts_changed {
            self.touch_all();
        }
        // Everything marked above is equal to its copy: the copies are what was put back.
        self.changed.since_copy.clear();
    }

    /// Let the oldest undo steps go while there are too many, or they take too much memory.
    pub(super) fn trim_undo(&mut self) {
        let mut total: usize = self.undo.iter().map(|s| s.bytes).sum();
        // The step a pinned checkpoint names, and every one after it, stay: the limits are soft while a pin is held.
        let oldest_pin = self.pins.iter().copied().min().unwrap_or(usize::MAX);
        while self.undo.len() > UNDO_LEAST && (self.undo.len() > UNDO_STEPS || total > self.undo_budget) && self.undo_base < oldest_pin {
            total -= self.undo.remove(0).bytes;
            self.undo_labels.remove(0);
            self.undo_base += 1;
        }
    }

    /// Change how much memory the undo steps may take together (the default is 128 MB). Older steps go when it is
    /// passed; the last few stay whatever they weigh.
    pub fn set_undo_budget(&mut self, bytes: usize) {
        self.undo_budget = bytes;
        self.trim_undo();
    }

    /// About how much memory the undo and redo steps take, and how many steps there are: (bytes, undo, redo).
    pub fn undo_weight(&self) -> (usize, usize, usize) {
        let bytes = self.undo.iter().chain(&self.redo).map(|s| s.bytes).sum();
        (bytes, self.undo.len(), self.redo.len())
    }

    pub fn undo(&mut self) -> bool {
        self.inv_begin();
        let done = self.undo_core();
        if done {
            self.sync_alt_previews();
            self.revision += 1;
            self.generation += 1;
            self.inv_end("undo", None);
        }
        done
    }

    pub(super) fn undo_core(&mut self) -> bool {
        if self.merge_preview.is_some() || self.access != Access::Edit {
            return false;
        }
        let Some(mut s) = self.undo.pop() else { return false };
        let mut cur = self.snapshot();
        cur.pkg_delta = self.pkg.swap_delta(std::mem::take(&mut s.pkg_delta));
        self.forget_body_xml_if_swapped(&cur.pkg_delta);
        let parts_back = self.puts_back_main_parts(&cur.pkg_delta);
        self.redo.push(cur.weigh());
        let label = self.undo_labels.pop().unwrap_or_default();
        self.redo_labels.push(label);
        self.restore(s);
        if parts_back {
            self.parts_agree_after();
        }
        self.export_bound_controls();
        true
    }

    pub fn redo(&mut self) -> bool {
        self.inv_begin();
        let done = self.redo_core();
        if done {
            self.sync_alt_previews();
            self.revision += 1;
            self.generation += 1;
            self.inv_end("redo", None);
        }
        done
    }

    pub(super) fn redo_core(&mut self) -> bool {
        if self.merge_preview.is_some() || self.access != Access::Edit {
            return false;
        }
        let Some(mut s) = self.redo.pop() else { return false };
        let mut cur = self.snapshot();
        cur.pkg_delta = self.pkg.swap_delta(std::mem::take(&mut s.pkg_delta));
        self.forget_body_xml_if_swapped(&cur.pkg_delta);
        let parts_back = self.puts_back_main_parts(&cur.pkg_delta);
        self.undo.push(cur.weigh());
        let label = self.redo_labels.pop().unwrap_or_default();
        self.undo_labels.push(label);
        self.restore(s);
        if parts_back {
            self.parts_agree_after();
        }
        self.export_bound_controls();
        true
    }

    /// What each undo step did, oldest first: the name of the step's first op (`insert_text`, `set_format`...).
    /// The last one is what Undo would take back.
    pub fn undo_labels(&self) -> Vec<String> {
        let mut v = self.undo_labels.clone();
        v.resize(self.undo.len(), String::new());
        v
    }

    /// What each redo step did, the next one to redo first.
    pub fn redo_labels(&self) -> Vec<String> {
        let mut v = self.redo_labels.clone();
        v.resize(self.redo.len(), String::new());
        v.reverse();
        v
    }

    /// Undo up to `n` steps (fewer when the history is shorter); true when any was undone.
    pub fn undo_steps(&mut self, n: usize) -> bool {
        let mut any = false;
        for _ in 0..n {
            if !self.undo() {
                break;
            }
            any = true;
        }
        any
    }

    /// Redo up to `n` steps.
    pub fn redo_steps(&mut self, n: usize) -> bool {
        let mut any = false;
        for _ in 0..n {
            if !self.redo() {
                break;
            }
            any = true;
        }
        any
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undo depth; pass to `revert_to` / `diff_since` (e.g. around an AI turn).
    pub fn checkpoint(&self) -> usize {
        self.undo_base + self.undo.len()
    }

    /// Keep every step after `checkpoint` whatever the memory budget says, until `unpin` is called with it (an AI turn
    /// and its review hold one, so `diff_since` and `revert_to` can always answer for it). Each `pin` needs an `unpin`.
    pub fn pin(&mut self, checkpoint: usize) {
        self.pins.push(checkpoint);
    }

    /// Let go of one `pin` of `checkpoint`; the steps it held back may go at the next edit.
    pub fn unpin(&mut self, checkpoint: usize) {
        if let Some(i) = self.pins.iter().position(|c| *c == checkpoint) {
            self.pins.swap_remove(i);
        }
        self.trim_undo();
    }

    /// Undo back to a checkpoint (the redo stack keeps the reverted steps). Does nothing, and returns false, when the
    /// checkpoint's steps were let go of by the budget: undoing "back to it" would stop part-way.
    pub fn revert_to(&mut self, checkpoint: usize) -> bool {
        if checkpoint < self.undo_base || self.access != Access::Edit {
            return false;
        }
        let mut any = false;
        while self.undo_base + self.undo.len() > checkpoint && !self.undo.is_empty() {
            any |= self.undo();
        }
        any
    }

    /// Paragraphs whose text changed since a checkpoint: (id, before, after).
    /// `before` is None for new paragraphs and `after` None for deleted ones.
    pub fn diff_since(&self, checkpoint: usize) -> Vec<(NodeId, Option<String>, Option<String>)> {
        let Some(snap) = checkpoint.checked_sub(self.undo_base).and_then(|i| self.undo.get(i)) else { return Vec::new() };
        let mut before = HashMap::new();
        let walk_all = |body: &mut dyn Iterator<Item = &Block>, stories: &[Story], f: &mut dyn FnMut(&Paragraph)| {
            for b in body {
                walk_paragraphs(std::slice::from_ref(b), f);
            }
            for s in stories {
                walk_paragraphs(&s.blocks, f);
            }
        };
        walk_all(&mut snap.body.iter().map(|a| &**a), &snap.stories, &mut |p| {
            before.insert(p.id, p.plain_text());
        });
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        walk_all(&mut self.body.iter(), &self.stories, &mut |p| {
            seen.insert(p.id);
            let now = p.plain_text();
            match before.get(&p.id) {
                Some(b) if *b == now => {}
                Some(b) => out.push((p.id, Some(b.clone()), Some(now))),
                None => out.push((p.id, None, Some(now))),
            }
        });
        walk_all(&mut snap.body.iter().map(|a| &**a), &snap.stories, &mut |p| {
            if !seen.contains(&p.id) {
                out.push((p.id, Some(p.plain_text()), None));
            }
        });
        out
    }

    /// How many times the document has changed since it was opened: up by one for each batch that changed something,
    /// and for each undo, redo and revert step. A refused batch, or one that changes nothing, leaves it alone. It is not
    /// saved in the file. "Saved" means the revision whose bytes were written is the revision now.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Who may change the document now.
    pub fn access(&self) -> Access {
        self.access
    }

    /// Set who may change the document. Nothing is applied or undone to the document itself; the next edit is checked.
    pub fn set_access(&mut self, access: Access) {
        self.access = access;
    }

    /// The one refusal for a view-only document: every way in (ops, the assistant's tools, undo, media) comes here.
    pub(super) fn read_only_error() -> EditError {
        EditError::Invalid("This document is read-only.".into())
    }

    /// Refuse `ops` while the document is view only, unless all of them are ops that change nothing a person wrote.
    pub(super) fn check_access(&self, ops: &[Op]) -> Result<(), EditError> {
        if self.access == Access::Edit || ops.iter().all(Op::is_view_only) {
            return Ok(());
        }
        Err(Self::read_only_error())
    }
}
