//! Repeating section controls: adding and removing repeated items and table rows.

use super::*;

impl Document {
    pub(super) fn control_repeat(&mut self, target: &ControlRef, remove: bool) -> Result<OpResult, EditError> {
        if let Some(w) = target.wrap {
            return self.repeat_rows(w.0, remove);
        }
        let id = target.block.ok_or_else(|| EditError::Invalid("a repeating section is a block-level control: say which with block".into()))?;
        self.touch(id.0);
        let pkg = &self.pkg;
        let main = self.main_path.clone();
        let (mut list, i) = self.body.parent_mut(id.0).ok_or(EditError::NotFound(id.0))?;
        let kind_of = |b: &Block| match b {
            Block::Container(c) => c.shell.child("w:sdtPr").map(controls::read_props).unwrap_or_default().kind,
            _ => String::new(),
        };
        let kind = kind_of(&list[i]);
        // The section and the item the op means.
        if kind == "repeating" {
            let mut fresh = IdGen(self.ids.0);
            let Block::Container(sec) = &mut list[i] else { unreachable!() };
            if remove {
                return Err(EditError::Invalid("say which item to remove".into()));
            }
            let last = sec.blocks.iter().rposition(|b| kind_of(b) == "repeatingItem").ok_or_else(|| EditError::Invalid("the repeating section has no item to copy".into()))?;
            let mut copy = sec.blocks[last].clone();
            reset_for_copy(&mut copy, &mut fresh, pkg, &main);
            self.ids.0 = fresh.0;
            sec.blocks.insert(last + 1, copy);
            return Ok(OpResult::default());
        }
        if kind != "repeatingItem" {
            return Err(EditError::Invalid("that is not a repeating section or one of its items".into()));
        }
        // An item: its section is the container around it.
        let parent_id = {
            let mut found = None;
            fn find_parent_of<'a>(blocks: impl IntoIterator<Item = &'a Block>, id: NodeId) -> Option<NodeId> {
                for b in blocks {
                    if let Block::Container(c) = b {
                        if c.blocks.iter().any(|x| x.id() == id) {
                            return Some(c.id);
                        }
                        if let Some(f) = find_parent_of(&c.blocks, id) {
                            return Some(f);
                        }
                    }
                }
                None
            }
            if let Some(f) = find_parent_of(&self.body, id.0) {
                found = Some(f);
            }
            found
        };
        let sec_id = parent_id.ok_or_else(|| EditError::Invalid("the item is not inside a repeating section".into()))?;
        let mut fresh = IdGen(self.ids.0);
        let (mut slist, si) = self.body.parent_mut(sec_id).ok_or(EditError::NotFound(sec_id))?;
        let Block::Container(sec) = &mut slist[si] else { return Err(EditError::NotFound(sec_id)) };
        let at = sec.blocks.iter().position(|b| b.id() == id.0).ok_or(EditError::NotFound(id.0))?;
        if remove {
            let items = sec.blocks.iter().filter(|b| kind_of(b) == "repeatingItem").count();
            if items <= 1 {
                return Err(EditError::Invalid("a repeating section keeps at least one item".into()));
            }
            sec.blocks.remove(at);
        } else {
            let mut copy = sec.blocks[at].clone();
            reset_for_copy(&mut copy, &mut fresh, &self.pkg, &main);
            self.ids.0 = fresh.0;
            sec.blocks.insert(at + 1, copy);
        }
        self.touch(sec_id);
        Ok(OpResult::default())
    }

    /// A repeating section whose items are table rows: copy or remove the rows under the item control `wrap`.
    pub(super) fn repeat_rows(&mut self, wrap: NodeId, remove: bool) -> Result<OpResult, EditError> {
        let mut fresh = IdGen(self.ids.0);
        let main = self.main_path.clone();
        let pkg = &self.pkg;
        let mut done = false;
        let mut tid = None;
        fn tables_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Table) -> bool) -> bool {
            for b in blocks.iter_mut() {
                match b {
                    Block::Table(t) => {
                        if f(t) {
                            return true;
                        }
                        for r in t.rows_mut() {
                            for c in r.cells_mut() {
                                if tables_mut(&mut c.blocks, f) {
                                    return true;
                                }
                            }
                        }
                    }
                    Block::Container(c) => {
                        if tables_mut(&mut c.blocks, f) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        let err = std::cell::RefCell::new(None::<String>);
        // The block that holds the table is copied from an undo step only when the table is changed.
        let body = &mut self.body;
        let mut find_in_body = |f: &mut dyn FnMut(&mut Table) -> bool| -> bool {
            (0..body.len()).any(|i| body.try_edit(i, |b| tables_mut(std::slice::from_mut(b), f)))
        };
        find_in_body(&mut |t: &mut Table| {
            let idx: Vec<usize> = t.children.iter().enumerate().filter_map(|(i, c)| matches!(c, TableChild::Row(r) if r.sdts.iter().any(|w| w.id == wrap)).then_some(i)).collect();
            if idx.is_empty() {
                return false;
            }
            tid = Some(t.id);
            // How many items the section has: the rows' siblings under the same outer control.
            let (first, last) = (idx[0], *idx.last().unwrap());
            if remove {
                let outer = match &t.children[first] {
                    TableChild::Row(r) => r.sdts.iter().position(|w| w.id == wrap).and_then(|d| d.checked_sub(1)).and_then(|d| r.sdts.get(d)).map(|w| w.id),
                    _ => None,
                };
                let siblings = t.children.iter().filter(|c| matches!(c, TableChild::Row(r) if outer.is_some_and(|o| r.sdts.iter().any(|w| w.id == o)))).count();
                if outer.is_some() && siblings <= idx.len() {
                    *err.borrow_mut() = Some("a repeating section keeps at least one item".into());
                    return true;
                }
                for i in idx.iter().rev() {
                    t.children.remove(*i);
                }
                // Range markers (a bookmark around the row, say) inside the item's control go with it, as they do
                // in Word when a bookmarked row is deleted; left behind they would keep an empty item.
                t.children.retain(|c| !matches!(c, TableChild::Marker(_, w) if w.iter().any(|w| w.id == wrap)));
            } else {
                let mut copies = Vec::new();
                for i in &idx {
                    let TableChild::Row(r) = &t.children[*i] else { continue };
                    let mut row = r.clone();
                    for w in row.sdts.iter_mut() {
                        if w.id == wrap {
                            w.id = fresh.fresh();
                        }
                    }
                    for c in row.cells_mut() {
                        for w in c.sdts.iter_mut() {
                            w.id = fresh.fresh();
                        }
                        reid_blocks(&mut c.blocks, &mut fresh);
                        reset_controls(&mut c.blocks, pkg, &main);
                    }
                    copies.push(TableChild::Row(row));
                }
                for (k, c) in copies.into_iter().enumerate() {
                    t.children.insert(last + 1 + k, c);
                }
            }
            done = true;
            true
        });
        if let Some(e) = err.into_inner() {
            return Err(EditError::Invalid(e));
        }
        if !done {
            return Err(EditError::Invalid("no table row has that content control".into()));
        }
        self.ids.0 = fresh.0;
        if let Some(t) = tid {
            self.touch(t);
        }
        Ok(OpResult::default())
    }
}
