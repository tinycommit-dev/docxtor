//! The fonts a document uses: counting them per block and story, and replacing one font with another.

use super::*;

impl Document {
    pub(super) fn op_replace_font(&mut self, from: &str, to: &str) -> Result<OpResult, EditError> {
        let (from, to) = (from.trim(), to.trim());
        if from.is_empty() || to.is_empty() {
            return Err(EditError::Invalid("name the font to replace and the font to use instead".into()));
        }
        let theme = self.styles.theme.clone();
        let styles = self.styles.replace_font(from, to);
        let body = !self.body.edit_each(|s| crate::fonts::replace_in_blocks(s, from, to, &theme)).is_empty();
        let mut stories = false;
        for i in 0..self.stories.len() {
            if crate::fonts::replace_in_blocks(&mut self.stories[i].blocks, from, to, &theme) {
                self.story_gen += 1;
                self.stories[i].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[i].part.clone());
                stories = true;
            }
        }
        if !(styles || body || stories) {
            return Err(EditError::Invalid(format!("no text uses the font '{from}'")));
        }
        // A style change can reach any paragraph, in the body or a story.
        if styles || body {
            self.touch_all();
        }
        self.stories_touched |= styles || stories;
        Ok(OpResult::default())
    }

    /// Fonts the text uses (body, headers, footers and footnotes), most used first.
    pub fn fonts_used(&self) -> Vec<crate::fonts::FontUse> {
        // The body from what the last render drew of each block, when nothing has changed since it; the headers, footers and notes from
        // the last page setup, when the document is as it was then. Either way the same list a render of everything gives (checked in
        // tests and debug builds), without the render: after an open that is half a second in a long book.
        let mut uses: Vec<crate::fonts::FontUse> = Vec::new();
        let mut add = |family: &str, chars: usize| add_font_use(&mut uses, family, chars);
        if !self.body_fonts_kept(&mut add) {
            // A block at a time: the whole drawing of a long document is not held at once for its fonts.
            self.render_snapshot_each(&mut |b| walk_fonts(std::slice::from_ref(&b), &mut add));
        }
        for (family, chars) in self.story_fonts() {
            add(&family, chars);
        }
        uses.retain(|u| u.chars > 0);
        uses.sort_by(|a, b| b.chars.cmp(&a.chars).then_with(|| a.family.cmp(&b.family)));
        #[cfg(any(test, debug_assertions))]
        {
            let walked = crate::perf::unmetered(|| self.fonts_used_walked());
            assert!(walked == uses, "the kept fonts differ from a render of everything: {uses:?} against {walked:?}");
        }
        uses
    }

    /// `fonts_used` from a render of everything: what it was before the fonts were kept, and what it is checked against.
    #[cfg(any(test, debug_assertions))]
    pub(super) fn fonts_used_walked(&self) -> Vec<crate::fonts::FontUse> {
        let mut uses: Vec<crate::fonts::FontUse> = Vec::new();
        let mut add = |family: &str, chars: usize| add_font_use(&mut uses, family, chars);
        self.render_snapshot_each(&mut |b| walk_fonts(std::slice::from_ref(&b), &mut add));
        let setup = self.page_setup_now();
        for st in setup.headers.values().chain(setup.footers.values()) {
            walk_fonts(&st.blocks, &mut add);
        }
        for n in &setup.notes {
            walk_fonts(&n.blocks, &mut add);
        }
        uses.retain(|u| u.chars > 0);
        uses.sort_by(|a, b| b.chars.cmp(&a.chars).then_with(|| a.family.cmp(&b.family)));
        uses
    }

    /// The body's fonts from what the last render drew of each block: false (and nothing added) when something changed since that render
    /// could not have drawn yet.
    pub(super) fn body_fonts_kept(&self, add: &mut dyn FnMut(&str, usize)) -> bool {
        if !self.fonts_body.ok || !self.changed.now.is_empty() || self.stories_touched || !self.key_matches(&self.last_key) {
            return false;
        }
        self.fonts_body.add_in_order(&self.last_order, add)
    }

    /// The fonts of the headers, footers and footnotes, from the last page setup while the document is as it was then.
    pub(super) fn story_fonts(&self) -> Vec<(String, usize)> {
        if let Some((generation, fonts)) = self.fonts_stories.lock().ok().and_then(|g| g.clone()) {
            if generation == self.generation {
                return fonts;
            }
        }
        let setup = self.page_setup();
        story_fonts_of(&setup)
    }
}

/// Add `chars` characters in `family` to a list of fonts used (families compared without case).
pub(super) fn add_font_use(uses: &mut Vec<crate::fonts::FontUse>, family: &str, chars: usize) {
    match uses.iter_mut().find(|u| u.family.eq_ignore_ascii_case(family)) {
        Some(u) => u.chars += chars,
        None => uses.push(crate::fonts::FontUse { family: family.to_string(), chars }),
    }
}

/// The fonts of the text runs in rendered blocks, with the characters in each.
pub(super) fn walk_fonts(blocks: &[RBlock], add: &mut dyn FnMut(&str, usize)) {
    for b in blocks {
        match b {
            RBlock::Para(p) => {
                for s in p.segs.iter().filter(|s| s.k == "t") {
                    if let Some(f) = &s.f.font {
                        add(f, s.x.as_deref().map_or(0, |x| x.chars().count()));
                    }
                }
            }
            RBlock::Table(t) => {
                for c in t.rows.iter().flat_map(|r| &r.cells) {
                    walk_fonts(&c.blocks, add);
                }
            }
            RBlock::Container { blocks, .. } => walk_fonts(blocks, add),
            RBlock::Opaque { .. } => {}
        }
    }
}

/// The fonts each top-level block was drawn with, kept between renders without an allocation a block: the names once (`names`), and for
/// each block a stretch of `arena` (font, characters), in the order the block uses them. A block drawn again gets a new stretch at the end;
/// the arena is packed again when stale stretches are half of it.
#[derive(Default)]
pub(super) struct FontsKept {
    /// Filled by a render of everything.
    ok: bool,
    names: Vec<String>,
    arena: Vec<(u32, u32)>,
    by_block: HashMap<NodeId, (u32, u32)>,
    live: usize,
    /// The block being read (kept so that a key allocates nothing).
    scratch: Vec<(u32, u32)>,
}

impl FontsKept {
    pub(super) fn with_capacity(n: usize) -> FontsKept {
        FontsKept { ok: true, by_block: HashMap::with_capacity(n), ..Default::default() }
    }

    /// Note the fonts of a block just drawn. A block drawn again takes its old stretch back when its fonts fit there (typing changes the
    /// characters, not the fonts), so a key writes in place and the arena does not grow.
    pub(super) fn set(&mut self, b: &RBlock) {
        let mut mine = std::mem::take(&mut self.scratch);
        mine.clear();
        let names = &mut self.names;
        walk_fonts(std::slice::from_ref(b), &mut |family, chars| {
            let i = match names.iter().position(|n| n == family) {
                Some(i) => i as u32,
                None => {
                    names.push(family.to_string());
                    (names.len() - 1) as u32
                }
            };
            match mine.iter_mut().find(|e| e.0 == i) {
                Some(e) => e.1 += chars as u32,
                None => mine.push((i, chars as u32)),
            }
        });
        let id = rblock_id(b);
        let len = mine.len() as u32;
        match self.by_block.get_mut(&id) {
            Some((s, l)) if len <= *l => {
                self.arena[*s as usize..(*s + len) as usize].copy_from_slice(&mine);
                self.live -= (*l - len) as usize;
                *l = len;
            }
            old => {
                let start = self.arena.len() as u32;
                if let Some((_, l)) = old {
                    self.live -= *l as usize;
                }
                self.arena.extend_from_slice(&mine);
                self.by_block.insert(id, (start, len));
                self.live += len as usize;
            }
        }
        self.scratch = mine;
        if self.arena.len() > 1024 && self.arena.len() > 2 * self.live {
            let old = std::mem::take(&mut self.arena);
            for (s, l) in self.by_block.values_mut() {
                let at = self.arena.len() as u32;
                self.arena.extend_from_slice(&old[*s as usize..(*s + *l) as usize]);
                *s = at;
            }
        }
    }

    /// The fonts of these blocks, in their order: false when one of them was never drawn.
    pub(super) fn add_in_order(&self, order: &[NodeId], add: &mut dyn FnMut(&str, usize)) -> bool {
        if order.iter().any(|id| !self.by_block.contains_key(id)) {
            return false;
        }
        for id in order {
            let (s, l) = self.by_block[id];
            for (i, chars) in &self.arena[s as usize..(s + l) as usize] {
                add(&self.names[*i as usize], *chars as usize);
            }
        }
        true
    }
}

/// The headers', footers' and footnotes' fonts at a `generation` of the document.
pub(super) type StoryFonts = (u64, Vec<(String, usize)>);

/// The fonts of a page setup's headers, footers and footnotes (what `fonts_used` counts of the stories).
pub(super) fn story_fonts_of(setup: &PageSetup) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut add = |family: &str, chars: usize| match out.iter_mut().find(|(f, _)| f == family) {
        Some(e) => e.1 += chars,
        None => out.push((family.to_string(), chars)),
    };
    for st in setup.headers.values().chain(setup.footers.values()) {
        walk_fonts(&st.blocks, &mut add);
    }
    for n in &setup.notes {
        walk_fonts(&n.blocks, &mut add);
    }
    out
}
