//! Rendering blocks and paragraphs: the context, block dispatch, paragraph and run formatting.

use super::*;

impl<'a> RenderCtx<'a> {
    pub fn new(
        styles: &'a StyleSheet,
        numbering: &'a Numbering,
        pkg: &'a Package,
        part: &str,
        labels: &'a HashMap<NodeId, ListLabel>,
    ) -> Self {
        let rels = pkg
            .rels_for(part)
            .unwrap_or_default()
            .into_iter()
            .map(|mut r| {
                if !r.external {
                    r.target = crate::package::resolve_target(part, &r.target);
                }
                (r.id.clone(), r)
            })
            .collect();
        RenderCtx {
            styles,
            numbering,
            pkg,
            part: part.to_string(),
            labels,
            rels,
            table_text: None,
            footnote_counter: std::cell::Cell::new(0),
            endnote_counter: std::cell::Cell::new(0),
            notes: HashMap::new(),
            endnotes: HashMap::new(),
            footnote_fmt: NoteFormat::default_for(NoteKind::Footnote),
            endnote_fmt: NoteFormat::default_for(NoteKind::Endnote),
            current_note: None,
            current_endnote: false,
            note_labels: HashMap::new(),
            current_note_label: None,
            sect_breaks: HashMap::new(),
            shapes: Default::default(),
            alts: None,
        }
    }

    /// Render the blocks of a text box with a context of its own (a shape is drawn while its paragraph is borrowed).
    pub(crate) fn render_story_blocks(&self, blocks: &[Block]) -> Vec<RBlock> {
        let labels = self.numbering.labels(blocks, self.styles);
        let mut sub = RenderCtx::new(self.styles, self.numbering, self.pkg, &self.part, &labels);
        sub.shapes.texts = self.shapes.texts.clone();
        sub.shapes.known_theme = self.shapes.known_theme;
        sub.blocks(blocks)
    }

    /// The part name (or external URL) a relationship id of the part being rendered points at.
    pub(crate) fn rel_src(&self, rid: &str) -> Option<String> {
        self.rels.get(rid).map(|r| r.target.clone())
    }

    pub fn blocks<'b>(&mut self, blocks: impl IntoIterator<Item = &'b Block>) -> Vec<RBlock> {
        let blocks = blocks.into_iter();
        let mut out = Vec::with_capacity(blocks.size_hint().0);
        for b in blocks {
            match b {
                // An imported chunk is drawn as its blocks, in place.
                Block::Container(c) if crate::model::is_alt_chunk(c) => self.alt_blocks(c, &mut out),
                _ => out.extend(self.block(b)),
            }
        }
        out
    }

    pub fn block(&mut self, b: &Block) -> Option<RBlock> {
        Some(match b {
            Block::Para(p) => RBlock::Para(self.para(p)),
            Block::Table(t) => RBlock::Table(self.table(t)),
            Block::Container(c) => {
                let label = c.shell.find("w:docPartGallery").and_then(|g| g.attr("w:val")).map(String::from).or_else(|| {
                    c.shell.find("w:alias").and_then(|a| a.attr("w:val")).map(String::from)
                });
                // Word's gallery for a bibliography is "Bibliographies". The editor finds the block by the gallery,
                // since the label is words it may show in another language.
                let gallery = c.shell.find("w:docPartGallery").and_then(|g| g.attr("w:val")).map(String::from);
                let label = label.map(|l| if l == "Bibliographies" { "Bibliography".to_string() } else { l });
                let control = crate::controls::block_view(c);
                RBlock::Container { id: c.id, label, gallery, control, blocks: self.blocks(&c.blocks) }
            }
            Block::Opaque(o) => {
                let label = match o.el.name.as_str() {
                    "w:sectPr" | "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr" | "w:permStart" | "w:permEnd"
                    | "w:commentRangeStart" | "w:commentRangeEnd" | "w:moveFromRangeStart" | "w:moveFromRangeEnd"
                    | "w:moveToRangeStart" | "w:moveToRangeEnd" => return None,
                    "w:altChunk" => "Embedded content",
                    "m:oMathPara" => "Equation",
                    _ => "Unsupported content",
                };
                RBlock::Opaque { id: o.id, label: Some(label.into()) }
            }
        })
    }

    /// A copy of formatting elements with the colours that name a theme colour (borders, fills) taken from the theme.
    pub(super) fn themed(&self, el: &Element) -> Element {
        let mut c = el.clone();
        if let Some(t) = self.styles.theme.colors.as_ref().filter(|_| crate::shapes::theme::has_theme_attrs(el)) {
            crate::shapes::theme::resolve_theme_attrs(&mut c, t);
        }
        c
    }

    pub fn para_fmt(&self, p: &Paragraph) -> (ParaFmt, RunFmt) {
        let direct = p.ppr.as_ref().map(|e| ParaFmt::from_ppr(&self.themed(e))).unwrap_or_default();
        let (sp, sr) = self.styles.para_style(direct.style.as_deref());
        let mut pf = self.styles.doc_ppr.clone();
        let mut rf = self.styles.doc_rpr.clone();
        if let Some((tp, tr)) = &self.table_text {
            pf = pf.overlay(tp);
            rf = rf.overlay(tr);
        }
        rf = rf.overlay(&sr);
        let level = self.labels.get(&p.id).and_then(|l| self.numbering_level(l));
        match level {
            // Numbering the paragraph names itself: its indents win over the style's.
            Some(level) if direct.num_id.is_some() => pf = pf.overlay(&sp).overlay(&level),
            // Numbering that comes from a style: the style that names the list wins over the level's indents,
            // and the level wins over the styles it is based on (as Word does).
            Some(level) => {
                let chain = self.styles.para_chain(direct.style.as_deref());
                let at = chain.iter().rposition(|c| c.num_id.is_some_and(|n| n > 0));
                for (i, c) in chain.iter().enumerate() {
                    if Some(i) == at {
                        pf = pf.overlay(&level);
                    }
                    pf = pf.overlay(c);
                }
                if at.is_none() {
                    pf = pf.overlay(&level);
                }
            }
            None => pf = pf.overlay(&sp),
        }
        pf = pf.overlay(&direct);
        (pf, rf)
    }

    pub(super) fn numbering_level(&self, l: &ListLabel) -> Option<ParaFmt> {
        self.numbering.level(l.num_id, l.level).map(|lv| lv.ppr.clone())
    }

    pub fn para(&mut self, p: &Paragraph) -> RPara {
        let (mut fmt, base_run) = self.para_fmt(p);
        // Shading in the theme's colours becomes hex; `shading` is the flat colour a renderer can always use.
        if let Some(shd) = &mut fmt.shd {
            shd.resolve(self.shapes.theme(self.pkg));
            fmt.shading = shd.flat();
        }
        let mark_direct = p.ppr.as_ref().and_then(|pp| pp.child("w:rPr")).map(RunFmt::from_rpr).unwrap_or_default();
        let mut mark = strip_cs(self.styles.finish_run(self.run_fmt(&base_run, &mark_direct)));
        crate::charfmt::resolve_run(&mut mark, self.shapes.theme(self.pkg));
        let sect_break = self.sect_breaks.get(&p.id).cloned();
        let mut segs = Vec::new();
        let mut field = FieldState::default();
        let mut start = 0usize;
        self.inlines(&p.inlines, &base_run, &mut start, &mut segs, &mut field, None, false);
        let mut list = self.labels.get(&p.id).cloned();
        if let Some(l) = &mut list {
            if l.font.is_none() {
                l.font = mark.font.clone();
            }
            if l.size.is_none() {
                l.size = mark.size;
            }
            // Word draws the label in the paragraph mark's colour and weight too, unless the level sets its own.
            if l.color.is_none() {
                l.color = mark.color.clone().filter(|c| c != "auto");
            }
            if l.bold.is_none() {
                l.bold = mark.bold;
            }
            if l.italic.is_none() {
                l.italic = mark.italic;
            }
        }
        // Includes the hidden `_Toc…` and `_Ref…` bookmarks: internal links to them need a target.
        let mut bookmarks = Vec::new();
        crate::marks::starts_in(&p.inlines, &mut bookmarks);
        bookmarks.retain(|n| n != "_GoBack");
        let hr = start == 0 && fmt.borders.as_ref().is_some_and(|b| b.bottom.is_some() && b.top.is_none() && b.left.is_none() && b.right.is_none());
        let lang = self.para_lang(p, fmt.style.as_deref());
        let controls = crate::controls::sdts_in(&p.inlines)
            .into_iter()
            .enumerate()
            .map(|(n, (c, s, e))| crate::controls::inline_view(c, n, s, e))
            .filter(|c| c.kind != "citation")
            .collect();
        let forms = crate::forms::views(&p.inlines);
        RPara { id: p.id, style: fmt.style.clone(), fmt, mark, segs, len: start, list, sect_break, bookmarks, hr, lang, controls, forms, chunk: None }
    }

    /// The effective language of a paragraph (see `RPara::lang`).
    pub(super) fn para_lang(&self, p: &Paragraph, style: Option<&str>) -> Option<String> {
        fn first_text_run(inlines: &[Inline]) -> Option<&crate::model::Run> {
            inlines.iter().find_map(|i| match i {
                Inline::Run(r) if r.items.iter().any(|it| matches!(it, RunItem::Text(t) if !t.is_empty())) => Some(r),
                Inline::Container(c) => first_text_run(&c.children),
                _ => None,
            })
        }
        let run = first_text_run(&p.inlines).and_then(|r| r.rpr.as_ref()).and_then(crate::styles::lang_of);
        let mark = || p.ppr.as_ref().and_then(|pp| pp.child("w:rPr")).and_then(crate::styles::lang_of);
        run.or_else(mark).or_else(|| self.styles.para_lang(style))
    }

    pub(super) fn run_fmt(&self, base: &RunFmt, direct: &RunFmt) -> RunFmt {
        let mut f = base.clone();
        if let Some(cs) = &direct.style {
            f = f.overlay(&self.styles.char_style(cs));
        }
        f.overlay(direct)
    }

    /// A phonetic guide: the formatting of its base text (the run's, with what the base's own run adds) and what
    /// the editor needs to draw the guide, with the guide's own formatting.
    pub(super) fn ruby_info(&self, e: &Element, base: &RunFmt, direct: &RunFmt) -> (RunFmt, crate::ruby::RRuby) {
        let mut info = crate::ruby::read(e);
        let theme = self.shapes.theme(self.pkg);
        let make = |inner: Option<&Element>| {
            let inner = inner.map(RunFmt::from_rpr).unwrap_or_default();
            let mut f = self.styles.finish_run(self.run_fmt(base, &direct.overlay(&inner)));
            crate::charfmt::resolve_run(&mut f, theme);
            f.style = None;
            f.font_theme = None;
            ea_variant(&f)
        };
        let bf = make(e.child("w:rubyBase").and_then(crate::ruby::first_rpr));
        let mut rf = make(e.child("w:rt").and_then(crate::ruby::first_rpr));
        // The guide is half the size of its base unless the file gives a size.
        if let Some(sz) = info.size {
            rf.size = Some(sz);
        } else {
            rf.size = bf.size.map(|s| (s / 2.0).max(1.0)).or(rf.size.map(|s| s / 2.0));
            info.size = rf.size;
        }
        info.base_size = info.base_size.or(bf.size);
        info.rt_f = Some(rf);
        (bf, info)
    }
}
