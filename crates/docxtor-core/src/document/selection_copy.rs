//! A copy of the document that holds only a selection, with the same styles, headers, footers and page setup, for
//! printing the selection (Word's "Print selection"). The copy is a separate document: the open one is not touched.

use super::*;

impl Document {
    /// The document as .docx bytes with only the text from `from` to `to` in its body (positions in the main text).
    ///
    /// Whole blocks outside the selection are dropped; the paragraphs at its ends keep only the selected text. A table
    /// or content control that holds either end of the selection is kept whole. The last section's page setup is the
    /// one the selection is in, so the selection prints on the paper it was written for. List numbers and the like are
    /// counted again in the copy, because the items before the selection are gone.
    pub fn selection_copy(&self, from: Pos, to: Pos) -> Result<Vec<u8>, EditError> {
        let (from, to) = self.ordered(from, to)?;
        crate::perf::body_cloned();
        let mut copy = self.fork_with_body(Vec::new());
        let mut all = self.body.to_vec();
        let holds = |b: &Block, id: NodeId| find_block(std::slice::from_ref(b), id).is_some();
        let first = all
            .iter()
            .position(|b| holds(b, from.p.0))
            .ok_or(EditError::NotFound(from.p.0))?;
        let last = all
            .iter()
            .position(|b| holds(b, to.p.0))
            .ok_or(EditError::NotFound(to.p.0))?;

        // The section the end of the selection is in: its properties are in the last block's own section break, or in
        // the first section break after it, or are the body's own at the end. They become the copy's final section, so
        // the selection prints on its own paper and no empty section follows it.
        let own_break = |b: &Block| match b {
            Block::Para(p) => p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")).cloned(),
            _ => None,
        };
        let governing = all[last..].iter().find_map(own_break);
        let ends_with_break = own_break(&all[last]).is_some();

        let mut kept: Vec<Block> = Vec::new();
        let tail = all.split_off(last + 1);
        let mut head = all;
        let selected = head.split_off(first);
        kept.extend(selected);
        // The body-level section properties stay at the end.
        let mut final_sect = tail
            .into_iter()
            .chain(head)
            .rev()
            .find(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr"));
        if let (Some(Block::Opaque(o)), Some(sect)) = (final_sect.as_mut(), governing) {
            o.el = sect;
        }
        if ends_with_break {
            if let Some(Block::Para(p)) = kept.last_mut() {
                if let Some(pp) = p.ppr.as_mut() {
                    pp.remove_children("w:sectPr");
                }
            }
        }

        // Trim the text at both ends.
        if let Some(Block::Para(p)) = kept.last_mut().filter(|b| b.id() == to.p.0) {
            let len = p.len16();
            edit::delete_range(&mut p.inlines, to.o.min(len), len);
        }
        if let Some(Block::Para(p)) = kept.first_mut().filter(|b| b.id() == from.p.0) {
            let len = p.len16();
            edit::delete_range(&mut p.inlines, 0, from.o.min(len));
        }
        if kept.is_empty() {
            let id = copy.ids.fresh();
            kept.push(Block::Para(Paragraph::new(id)));
        }
        if let Some(s) = final_sect {
            kept.push(s);
        }
        copy.body = Body::from(kept);
        copy.body_dirty = true;
        copy.touch_all();
        copy.save().map_err(|e| EditError::Invalid(e.to_string()))
    }
}
