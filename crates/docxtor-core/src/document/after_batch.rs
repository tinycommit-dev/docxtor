//! What runs after every batch of ops, for every part of the document the batch touched.
//!
//! A batch touches the body and any number of stories (headers, footers, notes, text boxes). Stories are edited by
//! swapping their blocks into the body's place for one op (`Document::in_story_with`), which is the one door: it notes
//! each story it opens, what that story held before, and which of its blocks changed. `after_batch` then runs the steps
//! on each of those places. Every step is a function over a list of blocks (plus what it needs), so none of them can
//! see the body only by accident:
//!
//! - locked controls stay as they were (`controls::lock_state_of`, `controls::locks_hold`);
//! - control state is put right: a placeholder goes when text is typed, comes back when the control is emptied, and a
//!   bound control writes its value to its custom XML part or document property (`controls::fix_controls_in`);
//! - a bookmark that lost one end gets it back, shrunk to what is left (`bookmark_halves`);
//! - a table cell ends with a paragraph, as the file requires (`cells_end_in_paragraph`), so what the engine shows is
//!   what a saved and reopened file shows;
//! - connectors follow the shapes they are glued to (`connectors::connectors_after`);
//! - a shape that came in as markup gets the story of its text box (`adopt_text_boxes`), and text typed in a text box
//!   goes back into its shape (`sync_text_boxes`, which looks at every story).
//!
//! Three steps belong to the body by what they are, and say so here rather than by being forgotten:
//!
//! - Protection compares the body's signature. A story edit is refused at the door while editing is restricted
//!   (`apply_batch_core`), so no story can change under protection by an op that names a paragraph in it.
//! - Tabs are runs of body blocks between section markers.
//! - Imported chunks (`w:altChunk`) are body blocks.

use super::*;
use crate::stories::StoryKind;

/// A story named so that it can be found again after ops that add or remove stories: its kind, its part, the note in
/// it (footnotes and endnotes) and the text box it is (text boxes).
pub(super) type StoryKey = (StoryKind, String, Option<String>, Option<String>);

/// The key of a story.
pub(super) fn story_key(s: &Story) -> StoryKey {
    (s.kind, s.part.clone(), s.note_id.clone(), crate::shapes::story::key_of(s).map(String::from))
}

/// One story a batch edited: its changed blocks, and what it held before the batch first opened it.
pub(super) struct StoryTouch {
    key: StoryKey,
    changed: BTreeSet<NodeId>,
    locks_before: Vec<(i64, String, String)>,
    connectors_before: Option<connectors::ConnSnapshot>,
    bookmarks_before: bookmark_halves::Marks,
}

/// What was noted about the document before a batch ran, to compare with it afterwards.
pub(super) struct BatchWatch {
    mode: Option<protect::Mode>,
    signature: Option<String>,
    locks: Vec<(i64, String, String)>,
    tabs: bool,
    text_boxes: Vec<(String, u64)>,
    connectors: Option<connectors::ConnSnapshot>,
    /// The body's bookmark halves without their other half as they were (unless every op only looks), to mend halves
    /// the batch left.
    bookmarks: Option<HashSet<(bool, String)>>,
}

impl BatchWatch {
    /// The editing restriction in force (read only, comments, forms), if any.
    pub(super) fn mode(&self) -> Option<protect::Mode> {
        self.mode
    }
}

impl Document {
    /// Note the document before `ops` run (the stories are noted as the ops open them).
    pub(super) fn batch_watch(&mut self, ops: &[Op]) -> BatchWatch {
        self.touched_stories.clear();
        let mode = self.editing_mode();
        BatchWatch {
            mode,
            signature: mode.map(|m| self.protected_signature(m)),
            // Only a document with a locked control needs its controls compared after the batch.
            locks: if self.body_has_locks() { controls::lock_state_of(&self.body) } else { Vec::new() },
            tabs: self.tabs_may_break(ops) && self.tabs_structure_ok(),
            text_boxes: self.text_box_gens(),
            connectors: if self.body_has_shapes() { connectors::connectors_before(&self.body) } else { None },
            bookmarks: (!ops.iter().all(Op::is_view_only)).then(|| {
                let unpaired = self.body_unpaired_bookmarks();
                // Names that went before this batch (an undo) are not the batch's to answer for.
                self.gone_bookmarks();
                unpaired
            }),
        }
    }

    /// A story is about to be edited: note what it holds, the first time in this batch.
    pub(super) fn story_opening(&mut self, si: usize) {
        let s = &self.stories[si];
        let key = story_key(s);
        if self.touched_stories.iter().any(|t| t.key == key) {
            return;
        }
        let touch = StoryTouch { key, changed: BTreeSet::new(), locks_before: controls::lock_state_of(&s.blocks), connectors_before: connectors::connectors_before(&s.blocks), bookmarks_before: bookmark_halves::Marks::of(&s.blocks) };
        self.touched_stories.push(touch);
    }

    /// The op that opened story `si` is done: these blocks of it changed.
    pub(super) fn story_closed(&mut self, si: usize, changed: BTreeSet<NodeId>) {
        let key = story_key(&self.stories[si]);
        if let Some(t) = self.touched_stories.iter_mut().find(|t| t.key == key) {
            t.changed.extend(changed);
        }
    }

    /// An op changed these top-level blocks of story `si` in place, without opening it (updating fields, citations):
    /// the steps after the batch run on them, as for an edit in the story. What is noted before the edit (locks,
    /// connectors, bookmark halves) is noted now unless an earlier op of the batch opened the story: a field's result
    /// changing moves none of them.
    pub(super) fn story_edited(&mut self, si: usize, tops: BTreeSet<NodeId>) {
        self.story_opening(si);
        self.story_closed(si, tops);
    }

    /// A bookmark the batch took away (with the text that held it, a tab, a cover page, a header) takes its links with it: they become
    /// plain text, as deleting the bookmark itself does (a link must lead somewhere, E14).
    /// Only when a name left the body or a story the batch touched, so a key pays nothing.
    fn unlink_gone_bookmarks(&mut self, body_watched: bool, touched: &[StoryTouch]) {
        // Names the batch took out, and names new links lead to that are not there (a bookmark the batch made and took out again: a
        // table of contents bookmarks the headings and the same batch deletes one). A link the file came with that leads nowhere is
        // not new, and stays as it came.
        // The kept facts are read again first: mending a bookmark that lost one half (above) can take its other half out too.
        let (mut gone, led_to) = if body_watched {
            self.body_unpaired_bookmarks();
            self.gone_bookmarks()
        } else {
            (Vec::new(), Vec::new())
        };
        // In a story the batch touched: the names that left it, and the names new links in it lead to (it is small: it is read whole).
        let mut story_led: Vec<(usize, Vec<String>)> = Vec::new();
        for t in touched {
            let Some(si) = self.stories.iter().position(|s| story_key(s) == t.key) else { continue };
            let now = bookmark_halves::Marks::of(&self.stories[si].blocks);
            let names = now.names();
            gone.extend(t.bookmarks_before.names().into_iter().filter(|n| !names.contains(n)));
            let new: Vec<String> = now.anchors().iter().filter(|a| !t.bookmarks_before.anchors().contains(*a) && !names.contains(*a)).cloned().collect();
            if !new.is_empty() {
                story_led.push((si, new));
            }
        }
        if gone.is_empty() && led_to.is_empty() && story_led.is_empty() {
            return;
        }
        // A name still somewhere (another story, or the body for a story's) is not gone.
        let mut there = bookmark_halves::names_in(&self.body);
        for s in &self.stories {
            there.extend(bookmark_halves::names_in(&s.blocks));
        }
        let keep = |v: Vec<String>| -> Vec<String> { v.into_iter().filter(|n| !there.contains(n)).collect() };
        let (gone, led_to) = (keep(gone), keep(led_to));
        for (si, led) in story_led {
            let led = keep(led);
            if !led.is_empty() && !crate::marks::unlink_gone(&mut self.stories[si].blocks, &led).is_empty() {
                self.story_gen += 1;
                self.stories[si].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[si].part.clone());
                self.stories_touched = true;
            }
        }
        if !led_to.is_empty() {
            // Only in the blocks the batch changed: the new links are there.
            let since = &self.changed.since_copy;
            let hit = self.body.edit_where(|b| since.contains(&b.id()), |s| !crate::marks::unlink_gone(s, &led_to).is_empty());
            for id in hit {
                self.changed.insert(id);
                self.body_dirty = true;
            }
        }
        if gone.is_empty() {
            return;
        }
        for id in self.body.edit_where(crate::marks::may_link, |s| !crate::marks::unlink_gone(s, &gone).is_empty()) {
            self.changed.insert(id);
            self.body_dirty = true;
        }
        for si in 0..self.stories.len() {
            if !crate::marks::unlink_gone(&mut self.stories[si].blocks, &gone).is_empty() {
                self.story_gen += 1;
                self.stories[si].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[si].part.clone());
                self.stories_touched = true;
            }
        }
    }

    /// Check the batch against what was noted (protection, locks, tabs, chunks), then put everything right in the body
    /// and in every story it touched. An error means the batch must be undone.
    pub(super) fn after_batch(&mut self, watch: BatchWatch, before: &Snapshot, body_touched: bool) -> Result<(), EditError> {
        let touched = std::mem::take(&mut self.touched_stories);
        // The stories as they are now, by key (ops can add and remove stories).
        let place = |d: &Document, t: &StoryTouch| d.stories.iter().position(|s| story_key(s) == t.key);

        // Checks.
        if let (Some(m), Some(sig)) = (watch.mode, &watch.signature) {
            if self.protected_signature(m) != *sig {
                return Err(self.protection_error());
            }
        }
        if watch.locks.iter().any(|(_, l, _)| !l.is_empty()) {
            controls::locks_hold(&watch.locks, &controls::lock_state_of(&self.body))?;
        }
        for t in &touched {
            if t.locks_before.iter().any(|(_, l, _)| !l.is_empty()) {
                // A story that is gone was removed whole (a header replaced), which is not an edit of its controls.
                if let Some(si) = place(self, t) {
                    controls::locks_hold(&t.locks_before, &controls::lock_state_of(&self.stories[si].blocks))?;
                }
            }
        }
        // A bookmark that lost one end to an op gets it back, shrunk to what is left (see `bookmark_halves`).
        if let Some(before_unpaired) = &watch.bookmarks {
            let now = self.body_unpaired_bookmarks();
            let fresh: Vec<&(bool, String)> = now.iter().filter(|h| !before_unpaired.contains(*h)).collect();
            if !fresh.is_empty() {
                // The starts as they were, for an end that lost its start: from the copy of the body before the batch.
                let starts = bookmark_halves::starts_of(before.body.iter().map(|b| &**b));
                for id in bookmark_halves::mend_fresh_body(&mut self.body, &fresh, &starts) {
                    self.changed.insert(id);
                }
            }
        }
        for t in &touched {
            if let Some(si) = place(self, t) {
                bookmark_halves::mend(&mut self.stories[si].blocks, &t.bookmarks_before);
            }
        }
        if watch.tabs {
            self.tabs_hold(&before.body)?;
        }
        // Blocks nested deeper than a file may be would save but not open again (see `crate::budget`).
        fn too_deep<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> bool {
            crate::model::block_depth(blocks) > crate::budget::NEST_LIMIT
        }
        if (body_touched && too_deep(&self.body)) || touched.iter().any(|t| place(self, t).is_some_and(|si| too_deep(&self.stories[si].blocks))) {
            return Err(EditError::Invalid("Tables and content controls can't be nested that deeply.".into()));
        }
        // A change inside an imported chunk: undo the batch and run it again on real content.
        if !self.alt_chunks.is_empty() && self.alt_changed_since(before) {
            return Err(Self::alt_retry_error());
        }

        // Control state: the body's changed blocks, then each touched story's.
        let main = self.main_path.clone();
        let mut bound: Vec<(String, String, String, String)> = Vec::new();
        let mut placeholders = false;
        // The body's blocks this batch changed (marked since the batch's copy was made), not every block marked to be drawn again: a
        // change of the page or the styles draws every block again and changes none of them, and what the file holds in a block
        // the batch did not touch stays as it is.
        let batch: BTreeSet<NodeId> = self.changed.since_copy.clone();
        if body_touched {
            let fixed = controls::fix_controls_in(self.body.iter_mut_where(|b| batch.contains(&b.id())), &batch, &self.pkg, &main);
            bound.extend(fixed.values);
            placeholders |= fixed.placeholders;
        }
        for t in &touched {
            if let Some(si) = place(self, t) {
                let fixed = controls::fix_controls_in(&mut self.stories[si].blocks, &t.changed, &self.pkg, &main);
                bound.extend(fixed.values);
                placeholders |= fixed.placeholders;
            }
        }
        if placeholders {
            self.ensure_placeholder_style();
        }
        // What a feature wrote with Word's style id goes to the id the file defines for that style (see `style_refs`).
        if body_touched {
            super::style_refs::point_style_refs_body(&mut self.body, &batch, &self.styles);
            if placeholders {
                super::style_refs::point_all_style_refs_body(&mut self.body, &self.styles);
            }
        }
        for t in &touched {
            if let Some(si) = place(self, t) {
                super::style_refs::point_style_refs(&mut self.stories[si].blocks, &t.changed, &self.styles);
            }
        }
        // Splitting a run or paragraph copies its formatting-change mark with the same id: give the copy a new one.
        if body_touched {
            super::change_ids::unique_change_ids_body(&mut self.body, &batch);
        }
        for t in &touched {
            if let Some(si) = place(self, t) {
                super::change_ids::unique_change_ids(&mut self.stories[si].blocks, &t.changed);
            }
        }

        // A cell ends with a paragraph: a control or a table put last in a cell gets one after it, as the saved file has. Only in the
        // blocks the batch changed, which are marked again: a cell the batch did not touch stays as the file has it (it used to be
        // given its paragraph by any edit anywhere, unmarked, so the undo step's copy of that table was stale and undo took the
        // paragraph back out; and every key walked every table of the text).
        if body_touched {
            let mut fixed = Vec::new();
            for b in self.body.iter_mut_where(|b| batch.contains(&b.id())) {
                if cells_end_in_paragraph(std::slice::from_mut(b), &mut self.ids) {
                    fixed.push(b.id());
                }
            }
            if !fixed.is_empty() {
                self.body_dirty = true;
                self.changed.extend(fixed);
            }
        }
        for t in &touched {
            if let Some(si) = place(self, t) {
                cells_end_in_paragraph(&mut self.stories[si].blocks, &mut self.ids);
            }
        }
        if !bound.is_empty() {
            self.write_bindings(&bound);
        }

        // A shape that came in as markup gets its text box story; text typed in a text box goes back into its shape.
        if body_touched || !touched.is_empty() {
            self.adopt_text_boxes();
        }
        self.sync_text_boxes(&watch.text_boxes);
        // Links to a bookmark that went become text: after the text boxes are synced, since a text box whose shape went goes with it
        // (and the bookmarks in it), and synced again for a link in a text box.
        let gens = self.text_box_gens();
        self.unlink_gone_bookmarks(watch.bookmarks.is_some(), &touched);
        self.sync_text_boxes(&gens);

        // Connectors follow the shapes they are glued to.
        for pid in connectors::connectors_after_body(&mut self.body, watch.connectors) {
            self.touch(pid);
        }
        for t in touched {
            if let Some(si) = place(self, &t) {
                connectors::connectors_after(&mut self.stories[si].blocks, t.connectors_before);
            }
        }
        Ok(())
    }
}

/// Give every table cell that does not end with a paragraph an empty one (the file format wants one, and the writer
/// adds it, so the document must hold it too or a reopened file would show one block more). True when one was added.
pub(super) fn cells_end_in_paragraph(blocks: &mut [Block], ids: &mut IdGen) -> bool {
    let mut added = false;
    for b in blocks {
        match b {
            Block::Table(t) => {
                for r in t.rows_mut() {
                    for c in r.cells_mut() {
                        added |= cells_end_in_paragraph(&mut c.blocks, ids);
                        if !crate::model::ends_in_paragraph(&c.blocks) {
                            c.blocks.push(Block::Para(Paragraph::new(ids.fresh())));
                            added = true;
                        }
                    }
                }
            }
            Block::Container(c) => added |= cells_end_in_paragraph(&mut c.blocks, ids),
            _ => {}
        }
    }
    added
}
