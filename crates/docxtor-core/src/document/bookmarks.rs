//! Bookmarks: listing them and the ops that insert, delete and rename them.

use super::*;

impl Document {
    /// The main body and every story, whichever of them an op is running in.
    pub(super) fn block_lists(&self) -> Vec<crate::body::BlocksRef<'_>> {
        use crate::body::BlocksRef;
        let mut v = vec![BlocksRef::Top(self.main_body())];
        if self.parked.is_some() {
            v.push(BlocksRef::Top(&self.body));
        }
        v.extend(self.stories.iter().map(|s| BlocksRef::List(&s.blocks)));
        v
    }

    /// Every bookmark id and name in the document and its stories (hidden ones included).
    pub(super) fn bookmark_ids_and_names(&self) -> (Vec<i64>, Vec<String>) {
        marks::ids_and_names(&self.block_lists())
    }

    /// The bookmarks people can see, in document order: the body first, then headers, footers and notes.
    /// Hidden ones (names starting with `_`, like the table of contents' `_Toc...`) are left out.
    /// Whether the document has a bookmark of this name (in its text or a story; Word matches names without regard to case).
    pub(super) fn has_bookmark(&self, name: &str) -> bool {
        // Every bookmark counts, the hidden ones too (`_Toc…` on a heading a table of contents lists, which the link editor offers):
        // `marks::list` leaves those out.
        let k = name.to_lowercase();
        // While an op runs in a story the main text is set aside (`parked`): its bookmarks count as well.
        bookmark_halves::names_in(&self.body).contains(&k)
            || self.parked.as_ref().is_some_and(|b| bookmark_halves::names_in(b).contains(&k))
            || self.stories.iter().any(|s| bookmark_halves::names_in(&s.blocks).contains(&k))
    }

    pub fn bookmarks(&self) -> Vec<BookmarkInfo> {
        let mut list = marks::list(&marks::scan(&self.block_lists()));
        for b in &mut list {
            if let Some(s) = self.stories.iter().find(|s| s.kind != StoryKind::TextBox && find_block(&s.blocks, b.p).is_some()) {
                b.story = Some(marks::BookmarkStory { kind: s.kind, part: s.part.clone(), note_id: s.note_id.clone() });
            }
        }
        list
    }

    /// Run `f` on the body and every story; stories it changes are rewritten on save. Only for ops that
    /// run in the main body.
    pub(super) fn edit_everywhere(&mut self, f: &mut dyn FnMut(&mut Vec<Block>) -> bool) -> bool {
        let mut any = false;
        // Every block goes back as a new one when `f` changes anything, and every block is marked changed with it.
        if self.body.edit_as_vec(|v| f(v)) {
            any = true;
            self.touch_all();
        }
        for i in 0..self.stories.len() {
            if f(&mut self.stories[i].blocks) {
                any = true;
                self.story_gen += 1;
                self.stories[i].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[i].part.clone());
                self.stories_touched = true;
            }
        }
        any
    }

    pub(super) fn op_insert_bookmark(&mut self, from: Pos, to: Pos, name: &str) -> Result<OpResult, EditError> {
        let name = marks::validate_name(name).map_err(EditError::Invalid)?;
        let (ids, names) = self.bookmark_ids_and_names();
        if names.iter().any(|n| n.to_lowercase() == name.to_lowercase()) {
            return Err(EditError::Invalid(format!("There is already a bookmark called {name}. Pick another name, or delete or rename that one first.")));
        }
        let (from, to) = self.ordered(from, to)?;
        let id = (ids.iter().max().copied().unwrap_or(0) + 1).to_string();
        // The end goes in first, so a collapsed range ends up as start, end.
        for (pos, el) in [(to, Element::new("w:bookmarkEnd").with_attr("w:id", &id)), (from, Element::new("w:bookmarkStart").with_attr("w:id", &id).with_attr("w:name", &name))] {
            self.touch(pos.p.0);
            let p = self.para_mut(pos.p.0)?;
            let off = pos.o.min(p.len16());
            edit::insert_inline_at(&mut p.inlines, off, Inline::Opaque(el));
        }
        Ok(OpResult { caret: Some(to), ..Default::default() })
    }

    pub(super) fn op_delete_bookmark(&mut self, name: &str) -> Result<OpResult, EditError> {
        let lc = name.trim().to_lowercase();
        let scan = marks::scan(&self.block_lists());
        let ids: std::collections::HashSet<String> = scan
            .events
            .iter()
            .filter_map(|e| match &e.kind {
                marks::Kind::Start { id, name } if name.to_lowercase() == lc => Some(id.clone()),
                _ => None,
            })
            .collect();
        if ids.is_empty() && !self.bookmark_ids_and_names().1.iter().any(|n| n.to_lowercase() == lc) {
            return Err(EditError::Invalid(format!("There is no bookmark called {}.", name.trim())));
        }
        self.edit_everywhere(&mut |blocks| marks::delete_bookmark(blocks, &lc, &ids));
        Ok(OpResult::default())
    }

    pub(super) fn op_rename_bookmark(&mut self, from: &str, to: &str) -> Result<OpResult, EditError> {
        let to = marks::validate_name(to).map_err(EditError::Invalid)?;
        let from = from.trim();
        let names = self.bookmark_ids_and_names().1;
        let (old_lc, new_lc) = (from.to_lowercase(), to.to_lowercase());
        if !names.iter().any(|n| n.to_lowercase() == old_lc) {
            return Err(EditError::Invalid(format!("There is no bookmark called {from}.")));
        }
        if old_lc != new_lc && names.iter().any(|n| n.to_lowercase() == new_lc) {
            return Err(EditError::Invalid(format!("There is already a bookmark called {to}. Pick another name, or delete or rename that one first.")));
        }
        self.edit_everywhere(&mut |blocks| marks::rename_bookmark(blocks, &old_lc, &to));
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Horizontal lines
    // -----------------------------------------------------------------------
}
