//! Finding things in the document: the story an op runs in, paragraphs by id, styles and media.

use super::*;

impl Document {
    pub(super) fn story_index(&self, id: NodeId) -> Option<usize> {
        self.stories.iter().position(|s| find_block(&s.blocks, id).is_some())
    }

    /// Run an op against a story: its blocks stand in for the body while the op runs.
    pub(super) fn in_story(&mut self, si: usize, op: &Op) -> Result<OpResult, EditError> {
        if op.main_text_only() {
            return Err(EditError::Invalid("that can only be inserted in the main text".into()));
        }
        self.in_story_with(si, &mut |d| d.apply_inner(op))
    }

    /// Run something against a story: its blocks stand in for the body while it runs.
    pub(super) fn in_story_with(&mut self, si: usize, f: &mut dyn FnMut(&mut Document) -> Result<OpResult, EditError>) -> Result<OpResult, EditError> {
        self.story_opening(si);
        let blocks = std::mem::take(&mut self.stories[si].blocks);
        self.parked = Some(std::mem::replace(&mut self.body, Body::from(blocks)));
        self.active_part = self.stories[si].part.clone();
        let changed = self.changed.take_now();
        let r = f(self);
        let story_changed = self.changed.replace_now(changed);
        let main = self.parked.take().expect("parked body");
        self.stories[si].blocks = std::mem::replace(&mut self.body, main).into_vec();
        let part = std::mem::replace(&mut self.active_part, self.main_path.clone());
        self.story_closed(si, story_changed);
        if std::mem::take(&mut self.restyled_main) {
            self.redraw_all();
        }
        if r.is_ok() {
            self.story_gen += 1;
            self.stories[si].gen = self.story_gen;
            // A text box is saved inside its shape (see `sync_text_boxes`), not as a part of its own.
            if self.stories[si].kind != StoryKind::TextBox {
                self.dirty_parts.insert(part);
                self.stories_touched = true;
            }
        }
        r
    }

    /// The document body, even while an op is running inside a story.
    pub(super) fn main_body(&self) -> &Body {
        self.parked.as_ref().unwrap_or(&self.body)
    }

    /// Blocks and part holding a paragraph (the body or a story).
    pub(super) fn locate(&self, id: NodeId) -> (crate::body::BlocksRef<'_>, &str) {
        if self.parked.is_none() && find_block(&self.body, id).is_none() {
            if let Some(s) = self.stories.iter().find(|s| find_block(&s.blocks, id).is_some()) {
                return (crate::body::BlocksRef::List(&s.blocks), &s.part);
            }
        }
        (crate::body::BlocksRef::Top(&self.body), &self.active_part)
    }

    /// A paragraph anywhere: the body, a header/footer or a footnote.
    pub fn find_any_para(&self, id: NodeId) -> Option<&Paragraph> {
        let (blocks, _) = self.locate(id);
        find_para(blocks, id)
    }

    /// A render of `part` with what the document keeps between renders (the theme, read once each time it changes).
    pub(crate) fn render_ctx<'a>(&'a self, part: &str, labels: &'a HashMap<NodeId, crate::numbering::ListLabel>) -> RenderCtx<'a> {
        let mut ctx = RenderCtx::new(&self.styles, &self.numbering, &self.pkg, part, labels);
        ctx.shapes.known_theme = Some(self.shape_theme.get_or_init(|| crate::shapes::theme::ShapeTheme::from_bytes(self.theme.bytes.as_deref())));
        ctx
    }

    pub(super) fn story_ctx<'a>(&'a self, part: &str, labels: &'a HashMap<NodeId, crate::numbering::ListLabel>, notes: &HashMap<String, u32>) -> RenderCtx<'a> {
        let mut ctx = self.render_ctx(part, labels);
        self.set_note_ctx(&mut ctx, notes);
        self.attach_shape_texts(&mut ctx);
        if part == self.main_path {
            ctx.sect_breaks = self.last_breaks.clone();
            ctx.alts = Some(&self.alt_chunks);
        }
        ctx
    }

    // -----------------------------------------------------------------------
    // Change tracking
    // -----------------------------------------------------------------------

    pub(super) fn para_mut(&mut self, id: NodeId) -> Result<&mut Paragraph, EditError> {
        match self.body.parent_mut(id) {
            None => Err(EditError::NotFound(id)),
            Some((list, i)) => match list.into_get_mut(i) {
                Some(Block::Para(p)) => Ok(p),
                _ => Err(EditError::NotParagraph(id)),
            },
        }
    }

    pub(super) fn para_ref(&self, id: NodeId) -> Result<&Paragraph, EditError> {
        match find_block(&self.body, id) {
            None => Err(EditError::NotFound(id)),
            Some(Block::Para(p)) => Ok(p),
            Some(_) => Err(EditError::NotParagraph(id)),
        }
    }

    /// Run formatting a paragraph's text inherits before direct formatting.
    pub(super) fn inherited_run(&self, para_style: Option<&str>) -> RunFmt {
        let (_, r) = self.styles.para_style(para_style);
        self.styles.doc_rpr.overlay(&r)
    }

    pub(super) fn para_style_of(&self, id: NodeId) -> Option<String> {
        find_para(&self.body, id).and_then(|p| p.ppr.as_ref()).and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).map(String::from)
    }

    pub(super) fn ordered(&self, a: Pos, b: Pos) -> Result<(Pos, Pos), EditError> {
        let order = paragraph_order(&self.body);
        let ia = order.iter().position(|x| *x == a.p.0).ok_or(EditError::NotFound(a.p.0))?;
        let ib = order.iter().position(|x| *x == b.p.0).ok_or(EditError::NotFound(b.p.0))?;
        Ok(if (ia, a.o) <= (ib, b.o) { (a, b) } else { (b, a) })
    }

    pub(super) fn resolve_style(&mut self, name: &str) -> Result<String, EditError> {
        self.styles
            .lookup(name)
            .or_else(|| self.styles.ensure_builtin(name))
            .ok_or_else(|| EditError::Invalid(format!("unknown style '{name}'")))
    }

    pub(super) fn in_table(&self, pid: NodeId) -> bool {
        fn go<'a>(blocks: impl IntoIterator<Item = &'a Block>, pid: NodeId) -> bool {
            blocks.into_iter().any(|b| match b {
                Block::Table(t) => t.rows().flat_map(|r| r.cells()).any(|c| find_block(&c.blocks, pid).is_some()),
                Block::Container(c) => go(&c.blocks, pid),
                _ => false,
            })
        }
        go(self.locate(pid).0, pid)
    }

    pub fn style_list(&self) -> Vec<StyleInfo> {
        self.styles.list()
    }

    pub fn media(&self, path: &str) -> Option<&[u8]> {
        self.pkg.get(path).or_else(|| self.alt_media(path))
    }

    /// A number that changes when the picture part `path` holds other bytes, is added or is taken out (see `Package::media_gen`).
    pub fn media_gen(&self, path: &str) -> u32 {
        self.pkg.media_gen(path)
    }

    /// `add_media` for hosts that must respect `Access`: refused while the document is view only.
    pub fn try_add_media(&mut self, ext: &str, bytes: Vec<u8>) -> Result<String, EditError> {
        if self.access != Access::Edit {
            return Err(Self::read_only_error());
        }
        Ok(self.add_media(ext, bytes))
    }

    /// Add an image to the package; returns its part name for `InsertImage`.
    pub fn add_media(&mut self, ext: &str, bytes: Vec<u8>) -> String {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        // The same picture added again is stored once: inserting, deleting and inserting it again adds nothing.
        self.ensure_media_baseline();
        let suffix = format!(".{ext}");
        if let Some(p) = self.pkg.parts.iter().find(|p| p.name.starts_with("word/media/") && p.name.to_ascii_lowercase().ends_with(&suffix) && p.data == bytes) {
            return p.name.clone();
        }
        let mut n = 1;
        while self.pkg.has(&format!("word/media/docxtor{n}.{ext}")) {
            n += 1;
        }
        let name = format!("word/media/docxtor{n}.{ext}");
        let ct = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            "tif" | "tiff" => "image/tiff",
            "emf" => "image/x-emf",
            "wmf" => "image/x-wmf",
            "emz" => "image/x-emz",
            "wmz" => "image/x-wmz",
            "jfif" | "jpe" => "image/jpeg",
            "dib" => "image/bmp",
            "ico" => "image/x-icon",
            "tga" => "image/x-tga",
            "pcz" => "image/x-pcz",
            "wdp" => "image/vnd.ms-photo",
            _ => "application/octet-stream",
        };
        self.pkg.ensure_default(&ext, ct);
        self.pkg.set(&name, bytes);
        name
    }
}
