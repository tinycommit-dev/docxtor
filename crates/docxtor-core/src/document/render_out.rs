//! Producing the rendering view model: full and partial renders, the dirty-block bookkeeping, and the JSON writers.

use super::*;

impl Document {
    pub(super) fn touch(&mut self, id: NodeId) {
        if let Some(t) = top_level_of(&self.body, id) {
            self.changed.insert(t);
        }
    }

    /// Draw the block holding `id` again though nothing in it changed (see `Changed::insert_redraw`).
    pub(super) fn redraw(&mut self, id: NodeId) {
        if let Some(t) = top_level_of(&self.body, id) {
            self.changed.insert_redraw(t);
        }
    }

    pub(super) fn redraw_all(&mut self) {
        for b in &self.body {
            self.changed.insert_redraw(b.id());
        }
    }

    pub fn touch_all(&mut self) {
        let ids: Vec<_> = self.body.iter().map(|b| b.id()).collect();
        self.changed.extend(ids);
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    pub fn render_all(&mut self) -> RenderOut {
        let mut sink = Sink::List(Vec::with_capacity(self.body.len()));
        let order = self.render_all_to(&mut sink);
        RenderOut { order, order_kept: false, blocks: sink.into_list(), full: true, stories: true }
    }

    /// `render_all`, with each block handed to `each` as soon as it is drawn instead of kept: the drawing of a long
    /// document is never all in memory at once (it was 5.5 KB a paragraph, 55 MB at 10,000, beside the opened document).
    /// The `RenderOut` has no blocks. See `render_json`, which the browser gets.
    pub fn render_all_each(&mut self, each: &mut dyn FnMut(RBlock)) -> RenderOut {
        let order = self.render_all_to(&mut Sink::Each(each));
        RenderOut { order, order_kept: false, blocks: Vec::new(), full: true, stories: true }
    }

    pub(super) fn render_all_to(&mut self, sink: &mut Sink) -> Vec<NodeId> {
        self.changed.clear();
        self.stories_touched = false;
        // Read every block again: nothing kept from before is trusted by a full render.
        self.facts = Default::default();
        self.refresh_facts();
        self.ensure_list_facts();
        self.last_labels = self.facts_labels().unwrap_or_default();
        self.last_breaks = self.section_breaks();
        self.last_key = self.render_key();
        self.last_notes = self.facts_notes().unwrap_or_default();
        // The fonts each block was drawn with, for `fonts_used`.
        let mut fonts = FontsKept::with_capacity(self.body.len());
        let mut order: Vec<NodeId> = Vec::with_capacity(self.body.len());
        {
            let mut ctx = self.story_ctx(&self.main_path, &self.last_labels, &self.last_notes);
            let mut emit = |rb: RBlock| {
                order.push(rblock_id(&rb));
                fonts.set(&rb);
                sink.push(rb);
            };
            // As `RenderCtx::blocks` does, a block at a time.
            let mut alt = Vec::new();
            for b in &self.body {
                match b {
                    // An imported chunk is drawn as its blocks, in place.
                    Block::Container(c) if is_alt_chunk(c) => {
                        ctx.alt_blocks(c, &mut alt);
                        alt.drain(..).for_each(&mut emit);
                    }
                    _ => ctx.block(b).into_iter().for_each(&mut emit),
                }
            }
        }
        self.last_order = order.clone();
        self.facts.done();
        self.fonts_body = fonts;
        order
    }

    /// Render everything without touching change tracking (for AI/CLI reads).
    pub fn render_snapshot(&self) -> RenderOut {
        self.render_blocks(&self.body, true)
    }

    /// The first `n` blocks of the body rendered like `render_snapshot`, without the rest: a preview that is made after every save
    /// must not cost what rendering the whole document costs.
    pub fn render_head(&self, n: usize) -> RenderOut {
        self.render_blocks(self.body.iter().take(n), false)
    }

    pub(super) fn render_blocks<'a>(&self, body: impl IntoIterator<Item = &'a Block> + Clone, full: bool) -> RenderOut {
        let labels = self.numbering.labels(body.clone(), &self.styles);
        let mut ctx = self.render_ctx(&self.main_path, &labels);
        self.set_note_ctx(&mut ctx, &self.note_state());
        self.attach_shape_texts(&mut ctx);
        ctx.sect_breaks = self.section_breaks();
        ctx.alts = Some(&self.alt_chunks);
        let blocks: Vec<RBlock> = ctx.blocks(body);
        let order = blocks.iter().map(rblock_id).collect();
        RenderOut { order, order_kept: false, blocks, full, stories: full }
    }

    /// `render_snapshot`, with each block handed to `each` as it is drawn instead of kept (see `render_all_each`).
    pub(crate) fn render_snapshot_each(&self, each: &mut dyn FnMut(RBlock)) {
        self.render_snapshot_by_block(&mut |_, drawn| drawn.drain(..).for_each(&mut *each));
    }

    /// `render_snapshot_each`, with each top-level block of the body handed over with what it drew (one block, or several for
    /// an imported chunk), so a pass over the model can read the drawing of each block as it comes and keep none of it.
    pub(crate) fn render_snapshot_by_block(&self, each: &mut dyn FnMut(&Block, &mut Vec<RBlock>)) {
        let labels = self.numbering.labels(&self.body, &self.styles);
        let mut ctx = self.render_ctx(&self.main_path, &labels);
        self.set_note_ctx(&mut ctx, &self.note_state());
        self.attach_shape_texts(&mut ctx);
        ctx.sect_breaks = self.section_breaks();
        ctx.alts = Some(&self.alt_chunks);
        // As `RenderCtx::blocks` does, a block at a time.
        let mut drawn = Vec::new();
        for b in &self.body {
            match b {
                Block::Container(c) if is_alt_chunk(c) => ctx.alt_blocks(c, &mut drawn),
                _ => drawn.extend(ctx.block(b)),
            }
            each(b, &mut drawn);
            drawn.clear();
        }
    }

    pub fn render_changed(&mut self) -> RenderOut {
        let mut sink = Sink::List(Vec::new());
        let (order, order_kept, stories) = self.render_changed_to(&mut sink);
        RenderOut { order, order_kept, blocks: sink.into_list(), full: false, stories }
    }

    /// `render_changed`, a block at a time (see `render_all_each`): a change of style draws every block again.
    pub fn render_changed_each(&mut self, each: &mut dyn FnMut(RBlock)) -> RenderOut {
        let (order, order_kept, stories) = self.render_changed_to(&mut Sink::Each(each));
        RenderOut { order, order_kept, blocks: Vec::new(), full: false, stories }
    }

    /// The order (empty when it is the one the last render gave), whether it is kept, and whether the stories changed.
    pub(super) fn render_changed_to(&mut self, sink: &mut Sink) -> (Vec<NodeId>, bool, bool) {
        // What each block holds of the notes, lists and sections: read again only where something changed.
        self.refresh_facts();
        // Section properties, defaults, styles, language or the stories themselves changed (an op, undo or redo): the
        // page setup is stale, and so is every block that uses a style that changed.
        if !self.key_matches(&self.last_key) {
            let key = self.render_key();
            let change = self.key_change(&self.last_key, &key);
            // Lists read styles, notes read settings and section properties: count again what moved.
            let (lists, notes) = Self::facts_moved(&self.last_key, &key);
            self.facts.key_changed(lists, notes);
            self.last_key = key;
            if change.all {
                self.touch_all();
            } else {
                self.touch_blocks_using(&change.styles);
            }
            if change.stories {
                self.stories_touched = true;
            }
        }
        self.ensure_list_facts();
        if let Some(labels) = self.facts_labels() {
            let mut relabeled = Vec::new();
            for (id, l) in &labels {
                if self.last_labels.get(id) != Some(l) {
                    relabeled.push(*id);
                }
            }
            for id in self.last_labels.keys() {
                if !labels.contains_key(id) {
                    relabeled.push(*id);
                }
            }
            for id in relabeled {
                self.redraw(id);
            }
            self.last_labels = labels;
        }
        // A section's type is shown on the paragraph that ends the section before it.
        let breaks = self.section_breaks();
        let moved: Vec<NodeId> = breaks.iter().filter(|(id, k)| self.last_breaks.get(id) != Some(k)).map(|(id, _)| *id).chain(self.last_breaks.keys().filter(|id| !breaks.contains_key(id)).copied()).collect();
        for id in moved {
            self.redraw(id);
        }
        self.last_breaks = breaks;
        if let Some(notes) = self.facts_notes() {
            if notes != self.last_notes {
                // Renumbered notes: every reference after the change moves.
                self.last_notes = notes;
                self.redraw_all();
                self.stories_touched = true;
            }
        }
        let changed = self.changed.take_now();
        let stories = std::mem::take(&mut self.stories_touched);
        let mut fonts = std::mem::take(&mut self.fonts_body);
        let mut emit = |rb: RBlock| {
            fonts.set(&rb);
            sink.push(rb);
        };
        let mut alt = Vec::new();
        let mut ctx = self.story_ctx(&self.main_path, &self.last_labels, &self.last_notes);
        // The order is compared with the last one as the blocks go by, and only built (and kept) when it differs.
        let kept = &self.last_order;
        let mut at = 0;
        let mut moved: Option<Vec<NodeId>> = None;
        let mut place = |id: NodeId| match &mut moved {
            Some(v) => v.push(id),
            None if kept.get(at) == Some(&id) => at += 1,
            None => {
                let mut v = Vec::with_capacity(kept.len() + 8);
                v.extend_from_slice(&kept[..at]);
                v.push(id);
                moved = Some(v);
            }
        };
        for b in &self.body {
            if !crate::render::is_visible(b) {
                continue;
            }
            // An imported chunk is drawn as its blocks, each with its own place in the order.
            if let Block::Container(c) = b {
                if is_alt_chunk(c) {
                    c.blocks.iter().for_each(|x| place(x.id()));
                    if changed.contains(&c.id) {
                        ctx.alt_blocks(c, &mut alt);
                        alt.drain(..).for_each(&mut emit);
                    }
                    continue;
                }
            }
            place(b.id());
            if changed.contains(&b.id()) {
                ctx.block(b).into_iter().for_each(&mut emit);
            }
        }
        let moved = match moved {
            Some(v) => Some(v),
            // The list ended sooner than the last one did.
            None if at < kept.len() => Some(kept[..at].to_vec()),
            None => None,
        };
        drop(ctx);
        self.facts.done();
        self.fonts_body = fonts;
        match moved {
            Some(order) => {
                self.last_order = order.clone();
                (order, false, stories)
            }
            None => (Vec::new(), true, stories),
        }
    }
}

/// Where a render puts the blocks it draws: kept in a list (`render_all`, `render_changed`), or handed on as each one
/// comes (`render_all_each`, `render_changed_each`).
pub(super) enum Sink<'a> {
    List(Vec<RBlock>),
    Each(&'a mut dyn FnMut(RBlock)),
}

impl Sink<'_> {
    fn push(&mut self, b: RBlock) {
        match self {
            Sink::List(v) => v.push(b),
            Sink::Each(f) => f(b),
        }
    }

    fn into_list(self) -> Vec<RBlock> {
        match self {
            Sink::List(v) => v,
            Sink::Each(_) => Vec::new(),
        }
    }
}

/// A render as the JSON of its `RenderOut`, each block written as `render` draws it and then dropped (`render` is
/// `render_all_each` or `render_changed_each`): the object `render_all` or `render_changed` gives, with `order` last.
/// Generic so that it is compiled where it is used, in the browser binding, which is built small (opt-level "s") and
/// already holds the code that writes blocks; compiled here, at opt-level 3, it added 107 KB to the engine.
pub fn render_json<F: FnOnce(&mut dyn FnMut(RBlock)) -> RenderOut>(render: F) -> String {
    let mut out = String::new();
    render_json_pieces(render, |piece| out.push_str(piece));
    out
}

/// `render_json` handed on in pieces of about 64 KB, each one whole blocks: the browser binding gives each piece to the
/// page as it comes, so the engine's memory, which never shrinks, holds one piece and not the whole render's JSON (a
/// 10,000-paragraph document grew it by 9.8 MB, for a 3.3 MB render, as the buffer doubled).
pub fn render_json_pieces<F: FnOnce(&mut dyn FnMut(RBlock)) -> RenderOut, P: FnMut(&str)>(render: F, mut piece: P) {
    const PIECE: usize = 64 << 10;
    let mut buf = Vec::with_capacity(PIECE + (PIECE >> 2));
    buf.extend_from_slice(b"{\"blocks\":[");
    let mut n = 0usize;
    let give = |buf: &mut Vec<u8>, piece: &mut P| {
        // serde_json writes UTF-8, and a piece ends between two blocks.
        piece(std::str::from_utf8(buf).unwrap_or_default());
        buf.clear();
    };
    let out = render(&mut |b: RBlock| {
        let len = buf.len();
        if n > 0 {
            buf.push(b',');
        }
        match serde_json::to_writer(&mut buf, &b) {
            Ok(()) => n += 1,
            // As `to_json` would have it, nothing half written: the block is left out.
            Err(_) => buf.truncate(len),
        }
        if buf.len() >= PIECE {
            give(&mut buf, &mut piece);
        }
    });
    buf.extend_from_slice(if out.full { b"],\"full\":true" } else { b"],\"full\":false" });
    buf.extend_from_slice(if out.stories { b",\"stories\":true" } else { b",\"stories\":false" });
    if out.order_kept {
        buf.extend_from_slice(b",\"orderKept\":true");
    }
    if !out.order.is_empty() {
        buf.extend_from_slice(b",\"order\":");
        let _ = serde_json::to_writer(&mut buf, &out.order);
    }
    buf.push(b'}');
    give(&mut buf, &mut piece);
}
