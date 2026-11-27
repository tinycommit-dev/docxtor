//! The ops for page layout beyond size and margins: section layout, page borders, note options, layout settings,
//! the page background, frames and drop caps, and notes with a custom mark. Types and XML are in `layout_props`.

use super::*;
use crate::layout_props::*;
use crate::notes::NoteKind;

const NS_V: &str = "urn:schemas-microsoft-com:vml";
const NS_O: &str = "urn:schemas-microsoft-com:office:office";

/// The separators of one kind of note, where the notes part has its own.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindSeps {
    /// The line above the notes of a page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<NoteSep>,
    /// The line above notes that carry on from the page before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<NoteSep>,
    /// The text below notes that go on to the next page ("continued on the next page").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<NoteSep>,
}

/// One separator note: whether it still draws Word's line, and what else it holds.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSep {
    pub line: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<RBlock>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSeps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnote: Option<KindSeps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endnote: Option<KindSeps>,
}

fn runs_in(inlines: &mut [Inline], f: &mut dyn FnMut(&mut Run)) {
    for i in inlines {
        match i {
            Inline::Run(r) => f(r),
            Inline::Container(c) => runs_in(&mut c.children, f),
            Inline::Opaque(_) => {}
        }
    }
}

/// The drop cap kind of a paragraph (`drop` or `margin`), if it is a drop cap's frame.
fn drop_cap_of(p: &Paragraph) -> Option<&str> {
    p.ppr.as_ref()?.child("w:framePr")?.attr("w:dropCap").filter(|v| *v != "none")
}

/// Word's ratio of a single line to the font size, roughly (the editor uses the same numbers).
fn line_factor(font: Option<&str>) -> f64 {
    let k = font.unwrap_or("").to_lowercase();
    if k.starts_with("calibri") || k.starts_with("carlito") {
        1.22
    } else if k.starts_with("cambria") {
        1.17
    } else if k.starts_with("aptos") {
        1.2
    } else if k.contains("times") {
        1.15
    } else if k.starts_with("georgia") {
        1.14
    } else if k.starts_with("arial") || k.starts_with("helvetica") {
        1.15
    } else {
        1.17
    }
}

/// A paragraph's font size and the height of one of its lines, in points.
fn drop_metrics(styles: &StyleSheet, p: &Paragraph) -> (f64, f64) {
    let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val"));
    let (pfmt, rfmt) = styles.para_style(style);
    let mut direct = RunFmt::default();
    let mut found = false;
    walk_runs(&p.inlines, &mut 0, &mut |r, _, _| {
        if !found && r.items.iter().any(|i| matches!(i, RunItem::Text(t) if !t.is_empty())) {
            found = true;
            if let Some(rpr) = &r.rpr {
                direct = RunFmt::from_rpr(rpr);
            }
        }
    });
    let size = direct.size.or(rfmt.size).or(styles.doc_rpr.size).unwrap_or(11.0);
    let font = direct.font.clone().or(rfmt.font.clone()).or(styles.doc_rpr.font.clone());
    let own = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
    let line = own.line.or(pfmt.line).or(styles.doc_ppr.line);
    let rule = own.line_rule.clone().or(pfmt.line_rule.clone()).or(styles.doc_ppr.line_rule.clone());
    let mult = match (line, rule.as_deref()) {
        (Some(l), None | Some("auto")) => l as f64 / 240.0,
        _ => 1.0,
    };
    (size, size * line_factor(font.as_deref()) * mult)
}

impl Document {
    /// Run `f` on the section properties of the section holding paragraph `scope` (with `from`, and every section
    /// after it), or of every section when there is no scope.
    fn edit_sects_range(&mut self, scope: Option<Id>, from: bool, f: &mut dyn FnMut(&mut Element)) -> Result<(), EditError> {
        let target = match scope {
            Some(id) => {
                self.para_ref(id.0)?;
                Some(self.section_of(id.0))
            }
            None => None,
        };
        self.final_sect_mut();
        let mut n = 0;
        let hit = |n: usize| target.is_none_or(|t| if from { n >= t } else { n == t });
        let mut edited = Vec::new();
        self.body.edit_paragraphs(&mut |p| {
            let mut changed = false;
            if let Some(s) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:sectPr")) {
                if hit(n) {
                    f(s);
                    edited.push(p.id);
                    changed = true;
                }
                n += 1;
            }
            changed
        });
        // The paragraphs whose section properties changed are changed blocks; the rest are only drawn again.
        for id in edited {
            self.touch(id);
        }
        if hit(n) {
            f(&mut self.final_sect_mut().el);
        }
        Ok(())
    }

    pub(super) fn op_set_section(&mut self, section: Option<Id>, from: bool, patch: &SectionPatch) -> Result<OpResult, EditError> {
        patch.validate().map_err(EditError::Invalid)?;
        if patch.gutter.is_some_and(|g| g < 0) || patch.header.is_some_and(|g| g < 0) || patch.footer.is_some_and(|g| g < 0) {
            return Err(EditError::Invalid("The gutter and the header and footer distances can't be negative.".into()));
        }
        self.edit_sects_range(section, from, &mut |sect| {
            if patch.gutter.is_some() || patch.header.is_some() || patch.footer.is_some() {
                let mut mar = sect.child("w:pgMar").cloned().unwrap_or_else(|| {
                    Element::new("w:pgMar").with_attr("w:top", "1440").with_attr("w:right", "1440").with_attr("w:bottom", "1440").with_attr("w:left", "1440").with_attr("w:header", "720").with_attr("w:footer", "720").with_attr("w:gutter", "0")
                });
                if let Some(v) = patch.gutter {
                    mar.set_attr("w:gutter", v.to_string());
                }
                if let Some(v) = patch.header {
                    mar.set_attr("w:header", v.to_string());
                }
                if let Some(v) = patch.footer {
                    mar.set_attr("w:footer", v.to_string());
                }
                crate::props::put_ordered(sect, mar, SECT_ORDER);
            }
            match patch.rtl_gutter {
                Some(true) => crate::props::put_ordered(sect, Element::new("w:rtlGutter"), SECT_ORDER),
                Some(false) => {
                    sect.remove_children("w:rtlGutter");
                }
                None => {}
            }
            if let Some(v) = &patch.v_align {
                if v == "top" {
                    sect.remove_children("w:vAlign");
                } else {
                    crate::props::put_ordered(sect, Element::new("w:vAlign").with_attr("w:val", v), SECT_ORDER);
                }
            }
            if let Some(v) = &patch.text_direction {
                if v == "lrTb" {
                    sect.remove_children("w:textDirection");
                } else {
                    crate::props::put_ordered(sect, Element::new("w:textDirection").with_attr("w:val", v), SECT_ORDER);
                }
            }
            if let Some(v) = &patch.start {
                set_sect_type(sect, v);
            }
            if let Some(g) = &patch.doc_grid {
                g.write(sect, SECT_ORDER);
            }
            match patch.no_endnote {
                Some(true) => crate::props::put_ordered(sect, Element::new("w:noEndnote"), SECT_ORDER),
                Some(false) => {
                    sect.remove_children("w:noEndnote");
                }
                None => {}
            }
        })?;
        self.redraw_all();
        Ok(OpResult::default())
    }

    pub(super) fn op_set_page_borders(&mut self, section: Option<Id>, from: bool, borders: Option<&PageBorders>) -> Result<OpResult, EditError> {
        if let Some(b) = borders {
            b.validate().map_err(EditError::Invalid)?;
        }
        let el = borders.map(PageBorders::element).filter(|e| e.elements().next().is_some());
        self.edit_sects_range(section, from, &mut |sect| match &el {
            Some(e) => crate::props::put_ordered(sect, e.clone(), SECT_ORDER),
            None => {
                sect.remove_children("w:pgBorders");
            }
        })?;
        self.redraw_all();
        Ok(OpResult::default())
    }

    pub(super) fn op_set_note_props(&mut self, kind: NoteKind, section: Option<Id>, from: bool, patch: &NotePatch) -> Result<OpResult, EditError> {
        patch.validate(kind).map_err(EditError::Invalid)?;
        let tag = kind.pr_tag();
        if section.is_none() && !from {
            // The whole document: the settings, and every section that says its own (it would hide the change).
            self.settings_edit(true, &mut |root| {
                let mut pr = root.child(tag).cloned().unwrap_or_else(|| Element::new(tag));
                patch.write(&mut pr);
                crate::props::put_ordered(root, pr, SETTINGS_ORDER);
            });
            self.edit_sects_range(None, false, &mut |sect| {
                if let Some(pr) = sect.child_mut(tag) {
                    patch.write(pr);
                }
            })?;
        } else {
            self.edit_sects_range(section, from, &mut |sect| {
                let mut pr = sect.child(tag).cloned().unwrap_or_else(|| Element::new(tag));
                patch.write(&mut pr);
                crate::props::put_ordered(sect, pr, SECT_ORDER);
            })?;
        }
        self.redraw_all();
        self.stories_touched = true;
        Ok(OpResult::default())
    }

    pub(super) fn op_set_layout_settings(&mut self, patch: &LayoutPatch) -> Result<OpResult, EditError> {
        patch.validate().map_err(EditError::Invalid)?;
        for (name, el) in patch.changes() {
            match el {
                Some(e) => self.settings_edit(true, &mut |root| crate::props::put_ordered(root, e.clone(), SETTINGS_ORDER)),
                None => self.settings_edit(false, &mut |root| {
                    root.remove_children(name);
                }),
            }
        }
        self.redraw_all();
        self.stories_touched = true;
        Ok(OpResult::default())
    }

    /// The page background as a `PageFill`, if the document has one.
    pub(super) fn page_fill(&self) -> Option<PageFill> {
        let bg = self.doc_xml.root.child("w:background")?;
        let rel = |rid: &str| self.pkg.resolve_rel(&self.main_path, rid).map(|r| r.target);
        Some(PageFill::read(bg, &rel))
    }

    pub(super) fn op_set_page_background(&mut self, fill: Option<&PageFill>) -> Result<OpResult, EditError> {
        match fill {
            None => {
                self.set_background(None);
                self.settings_edit(false, &mut |r| {
                    r.remove_children("w:displayBackgroundShape");
                });
            }
            Some(f) => {
                f.validate().map_err(EditError::Invalid)?;
                let mut rid = None;
                if matches!(f.kind.as_str(), "pattern" | "texture" | "picture") {
                    let media = f.media.as_deref().unwrap_or_default();
                    if !self.pkg.has(media) {
                        return Err(EditError::Invalid(format!("media '{media}' not found; add it first")));
                    }
                    let main = self.main_path.clone();
                    rid = Some(self.pkg.add_rel(&main, package::REL_IMAGE, &relative_target(&main, media), false));
                    self.ensure_ns("r", NS_R);
                }
                if f.kind != "solid" {
                    self.ensure_ns("v", NS_V);
                    self.ensure_ns("o", NS_O);
                }
                self.set_background(Some(f.element(rid.as_deref())));
                // Word only shows the background when the document asks for it.
                self.settings_edit(true, &mut |r| crate::props::put_ordered(r, Element::new("w:displayBackgroundShape"), SETTINGS_ORDER));
            }
        }
        self.redraw_all();
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Notes
    // -----------------------------------------------------------------------

    /// The note references and section ends of the main text, in order (see `facts.rs`, which keeps them per block).
    pub(super) fn note_events(&self) -> Vec<NoteEv> {
        self.main_body().iter().flat_map(facts::block_events).collect()
    }

    /// Where each note is referenced in the main text, in order (by id; `e:` and the id for an endnote). Notes
    /// with a mark of their own are here, though they have no number.
    pub(super) fn note_order(&self) -> HashMap<String, usize> {
        facts::order_of(&self.note_events().iter().collect::<Vec<_>>())
    }

    /// The labels footnotes and endnotes show, where a section numbers them its own way or starts them again
    /// (by id; `e:` and the id for an endnote). Restarting on each page is up to the page layout.
    pub(super) fn note_labels(&self) -> HashMap<String, String> {
        self.note_labels_from(&self.note_events().iter().collect::<Vec<_>>())
    }

    pub(super) fn note_labels_from(&self, evs: &[&NoteEv]) -> HashMap<String, String> {
        let settings = self.settings.as_ref().map(|d| &d.root);
        let prs: Vec<[NotePr; 2]> = self.section_refs().iter().map(|(_, s)| [NotePr::read(NoteKind::Footnote, settings, *s), NotePr::read(NoteKind::Endnote, settings, *s)]).collect();
        let mut out = HashMap::new();
        // Per kind: how many notes so far, where numbering began, the section of the last one.
        let mut state: [(i64, i64, usize); 2] = [(0, 0, 0); 2];
        let mut sec = 0usize;
        for ev in evs {
            let (k, id) = match ev {
                NoteEv::SectEnd(_) => {
                    sec += 1;
                    continue;
                }
                NoteEv::Ref { custom: true, .. } => continue,
                NoteEv::Ref { end, id, .. } => (usize::from(*end), id),
            };
            let at = sec.min(prs.len().saturating_sub(1));
            let key = if k == 0 { id.clone() } else { format!("e:{id}") };
            if out.contains_key(&key) {
                continue;
            }
            let Some(pr) = prs.get(at).map(|p| &p[k]) else { continue };
            let (count, base, last) = &mut state[k];
            if *count == 0 || (pr.restart == "eachSect" && *last != at) {
                *base = pr.start;
                *count = 0;
            }
            let n = *base + *count;
            *count += 1;
            *last = at;
            out.insert(key, notes::format_number(n, &pr.fmt));
        }
        out
    }

    /// The separator notes of the footnotes and endnotes parts that differ from Word's own.
    pub(super) fn note_seps(&self) -> Option<NoteSeps> {
        let mut out = NoteSeps::default();
        for kind in [NoteKind::Footnote, NoteKind::Endnote] {
            let root = if kind == NoteKind::Footnote { "w:footnotes" } else { "w:endnotes" };
            let Some((part, doc)) = self.story_parts.parts.iter().find(|(_, d)| d.root.name == root) else { continue };
            let mut seps = KindSeps::default();
            let mut any = false;
            for n in doc.root.children_named(kind.note_tag()) {
                let ty = n.attr("w:type").unwrap_or("normal");
                if !matches!(ty, "separator" | "continuationSeparator" | "continuationNotice") {
                    continue;
                }
                let mark = if ty == "separator" { "w:separator" } else { "w:continuationSeparator" };
                let line = n.find(mark).is_some();
                let mut ids = IdGen(2_000_000_000);
                let blocks = parse_blocks(n, &mut ids);
                let labels = self.numbering.labels(&blocks, &self.styles);
                let mut ctx = self.render_ctx(part, &labels);
                let rendered = ctx.blocks(&blocks);
                let visible = rendered.iter().any(|b| matches!(b, RBlock::Para(p) if p.len > 0 || p.hr) || matches!(b, RBlock::Table(_)));
                // Word's own: just the mark. A notice that is missing or empty draws nothing.
                let custom = visible || (ty != "continuationNotice" && !line);
                if !custom {
                    continue;
                }
                let sep = NoteSep { line, blocks: if visible { rendered } else { Vec::new() } };
                any = true;
                match ty {
                    "separator" => seps.separator = Some(sep),
                    "continuationSeparator" => seps.continuation = Some(sep),
                    _ => seps.notice = Some(sep),
                }
            }
            if any {
                if kind == NoteKind::Footnote {
                    out.footnote = Some(seps);
                } else {
                    out.endnote = Some(seps);
                }
            }
        }
        (out.footnote.is_some() || out.endnote.is_some()).then_some(out)
    }

    pub(super) fn op_insert_custom_note(&mut self, at: Pos, kind: NoteKind, mark: &str, text: &str) -> Result<OpResult, EditError> {
        let mark: String = mark.chars().filter(|c| !c.is_control()).collect();
        if mark.trim().is_empty() {
            return Err(EditError::Invalid("Give the mark to show, such as * or a symbol.".into()));
        }
        if mark.chars().count() > 8 {
            return Err(EditError::Invalid("A note mark is a few characters at most.".into()));
        }
        let off = at.o.min(self.para_ref(at.p.0)?.len16());
        let res = match kind {
            NoteKind::Footnote => self.op_insert_footnote(at, text)?,
            NoteKind::Endnote => self.op_insert_note(kind, at, text)?,
        };
        let note_pid = res.caret.map(|c| c.p.0).ok_or_else(|| EditError::Invalid("the note was not made".into()))?;
        let si = self
            .stories
            .iter()
            .position(|s| find_para(&s.blocks, note_pid).is_some())
            .ok_or_else(|| EditError::Invalid("the note was not made".into()))?;
        let id = self.stories[si].note_id.clone().unwrap_or_default();
        let ref_style = self.styles.ensure_builtin(kind.ref_style()).unwrap_or_else(|| kind.ref_style().into());
        // In the note: the mark replaces the number.
        let mut caret = res.caret;
        self.story_gen += 1;
        let gen = self.story_gen;
        let mark_text = mark.clone();
        let story = &mut self.stories[si];
        walk_paragraphs_mut(&mut story.blocks, &mut |p| {
            if p.id != note_pid {
                return;
            }
            runs_in(&mut p.inlines, &mut |r| {
                if r.items.iter().any(|i| matches!(i, RunItem::Object(e) if e.name == kind.mark_tag())) {
                    r.items = text_items(&mark_text);
                }
            });
            caret = Some(Pos::new(p.id, p.len16()));
        });
        story.gen = gen;
        let part = story.part.clone();
        self.dirty_parts.insert(part);
        // In the text: the reference says its mark follows, and the mark comes right after it.
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        runs_in(&mut p.inlines, &mut |r| {
            for it in r.items.iter_mut() {
                if let RunItem::Object(e) = it {
                    if e.name == kind.reference_tag() && e.attr("w:id") == Some(id.as_str()) {
                        e.set_attr("w:customMarkFollows", "1");
                    }
                }
            }
        });
        let rpr = Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", &ref_style));
        let run = Run::of_text(Some(rpr), &mark);
        edit::insert_inline_at(&mut p.inlines, off + 1, Inline::Run(run));
        self.stories_touched = true;
        Ok(OpResult { caret, ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Page numbers
    // -----------------------------------------------------------------------

    pub(super) fn op_insert_page_number(&mut self, section: Option<Id>, spec: &PageNumberSpec) -> Result<OpResult, EditError> {
        spec.validate().map_err(EditError::Invalid)?;
        let kind = if spec.place == "bottom" { HeaderKind::Footer } else { HeaderKind::Header };
        let variant = spec.variant.as_deref().unwrap_or("default");
        let made = self.add_header_footer_variant(kind, variant, section)?;
        let pid = made.caret.map(|c| c.p.0).ok_or_else(|| EditError::Invalid("the header or footer was not made".into()))?;
        let si = self
            .stories
            .iter()
            .position(|s| find_para(&s.blocks, pid).is_some())
            .ok_or_else(|| EditError::Invalid("the header or footer was not found".into()))?;
        let style = match kind {
            HeaderKind::Header => self.styles.ensure_builtin("Header").unwrap_or_else(|| "Header".into()),
            HeaderKind::Footer => self.styles.ensure_builtin("Footer").unwrap_or_else(|| "Footer".into()),
        };
        let margin = spec.place.starts_with("margin");
        let sect = if margin { self.sect_of(section)? } else { None };
        let align = spec.align.clone().unwrap_or_else(|| if spec.place == "margin_right" { "right".into() } else if spec.place == "margin_left" { "left".into() } else { "center".into() });
        let mut inlines: Vec<Inline> = Vec::new();
        let words = spec.style.as_deref() == Some("x_of_y");
        let word = |w: &Option<String>, d: &str| w.clone().filter(|w| !w.trim().is_empty()).unwrap_or_else(|| d.to_string());
        if words {
            let page = word(&spec.page_word, "Page");
            inlines.push(Inline::Run(Run { t_attrs: Some(vec![("xml:space".into(), "preserve".into())]), ..Run::of_text(None, &format!("{page} ")) }));
        }
        inlines.extend(crate::fields::complex(" PAGE ", "1", Some(crate::fields::no_proof()), false));
        if words {
            let of = word(&spec.of_word, "of");
            inlines.push(Inline::Run(Run { t_attrs: Some(vec![("xml:space".into(), "preserve".into())]), ..Run::of_text(None, &format!(" {of} ")) }));
            inlines.extend(crate::fields::complex(" NUMPAGES ", "1", Some(crate::fields::no_proof()), false));
        }
        let id = self.ids.fresh();
        self.story_gen += 1;
        let gen = self.story_gen;
        let story = &mut self.stories[si];
        // A page number replaces the ones already there.
        strip_page_numbers(&mut story.blocks);
        let mut ppr = Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", &style));
        if margin {
            // A frame in the margin, centred in it (the way Word's gallery does) and not against the paper's edge.
            let (page_w, left, right) = match &sect {
                Some(s) => {
                    let n = |tag: &str, key: &str, d: i64| s.child(tag).and_then(|e| e.attr(key)).and_then(|v| v.parse::<i64>().ok()).unwrap_or(d);
                    (n("w:pgSz", "w:w", 12240), n("w:pgMar", "w:left", 1440), n("w:pgMar", "w:right", 1440))
                }
                None => (12240, 1440, 1440),
            };
            let (margin_w, margin_x) = if spec.place == "margin_right" { (right, page_w - right) } else { (left, 0) };
            let w = margin_w.clamp(1, 720);
            let fr = FrameFmt {
                w: Some(w),
                wrap: Some("around".into()),
                h_anchor: Some("page".into()),
                v_anchor: Some("margin".into()),
                y_align: Some("center".into()),
                x: Some(margin_x + (margin_w - w) / 2),
                ..Default::default()
            };
            put_ppr(&mut ppr, fr.element());
        }
        if align != "left" || margin {
            put_ppr(&mut ppr, Element::new("w:jc").with_attr("w:val", if margin { "center" } else { &align }));
        }
        let mut p = Paragraph::new(id);
        p.ppr = Some(ppr);
        p.inlines = inlines;
        let caret = Pos::new(id, p.len16());
        // After what the header or footer already holds; an empty one is used up.
        let only_empty = story.blocks.iter().all(|b| matches!(b, Block::Para(q) if q.len16() == 0));
        if only_empty {
            story.blocks.clear();
        }
        story.blocks.push(Block::Para(p));
        story.gen = gen;
        let part = story.part.clone();
        self.dirty_parts.insert(part);
        self.stories_touched = true;
        Ok(OpResult { caret: Some(caret), created: vec![id], ..Default::default() })
    }

    pub(super) fn op_remove_page_numbers(&mut self) -> Result<OpResult, EditError> {
        let mut count = 0;
        let mut dirty = Vec::new();
        self.story_gen += 1;
        let gen = self.story_gen;
        for s in self.stories.iter_mut().filter(|s| matches!(s.kind, StoryKind::Header | StoryKind::Footer)) {
            let n = strip_page_numbers(&mut s.blocks);
            if n > 0 {
                count += n;
                // A header or footer always keeps one paragraph.
                if s.blocks.is_empty() {
                    let style = if s.kind == StoryKind::Header { "Header" } else { "Footer" };
                    let mut p = Paragraph::new(self.ids.fresh());
                    p.ppr = Some(Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style)));
                    s.blocks.push(Block::Para(p));
                }
                s.gen = gen;
                dirty.push(s.part.clone());
            }
        }
        if count == 0 {
            return Err(EditError::Invalid("There are no page numbers in the headers and footers.".into()));
        }
        self.dirty_parts.extend(dirty);
        self.stories_touched = true;
        Ok(OpResult { count: Some(count), ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Frames and drop caps
    // -----------------------------------------------------------------------

    pub(super) fn op_set_frame(&mut self, ids: &[Id], frame: Option<&FrameFmt>) -> Result<OpResult, EditError> {
        if let Some(f) = frame {
            f.validate().map_err(EditError::Invalid)?;
        }
        for id in ids {
            self.touch(id.0);
            let p = self.para_mut(id.0)?;
            match frame {
                Some(f) => crate::props::put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), f.element()),
                None => {
                    if let Some(pp) = p.ppr.as_mut() {
                        pp.remove_children("w:framePr");
                    }
                }
            }
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_set_drop_cap(&mut self, id: NodeId, o: &DropCapOptions) -> Result<OpResult, EditError> {
        let kind = o.kind.as_str();
        if !matches!(kind, "none" | "drop" | "margin") {
            return Err(EditError::Invalid(format!("A drop cap is 'none', 'drop' or 'margin', not '{kind}'.")));
        }
        let lines = o.lines.unwrap_or(3);
        if !(1..=10).contains(&lines) {
            return Err(EditError::Invalid("A drop cap spans 1 to 10 lines.".into()));
        }
        if o.distance.is_some_and(|d| !(0..=5760).contains(&d)) {
            return Err(EditError::Invalid("The distance from the text is between 0 and 4 inches.".into()));
        }
        let new_id = self.ids.fresh();
        // Looked at first: taking a drop cap off a paragraph that has none changes nothing, and copies nothing from an undo step.
        if kind == "none" {
            let (list, i) = self.body.parent_of(id).ok_or(EditError::NotFound(id))?;
            let is_frame = |b: Option<&Block>| matches!(b, Some(Block::Para(p)) if drop_cap_of(p).is_some());
            let (framed, text_ix) = if is_frame(list.get(i)) {
                (true, i + 1)
            } else if i > 0 && is_frame(list.get(i - 1)) {
                (true, i)
            } else {
                (false, i)
            };
            if !matches!(list.get(text_ix), Some(Block::Para(_))) {
                return Err(EditError::Invalid("A drop cap needs the paragraph of text it sits in.".into()));
            }
            if !framed {
                return Ok(OpResult::default());
            }
        }
        let styles = &self.styles;
        let (mut list, i) = self.body.parent_mut(id).ok_or(EditError::NotFound(id))?;
        let is_frame = |b: &Block| matches!(b, Block::Para(p) if drop_cap_of(p).is_some());
        let (frame_ix, text_ix) = if is_frame(&list[i]) {
            (Some(i), i + 1)
        } else if i > 0 && is_frame(&list[i - 1]) {
            (Some(i - 1), i)
        } else {
            (None, i)
        };
        if !matches!(list.get(text_ix), Some(Block::Para(_))) {
            return Err(EditError::Invalid("A drop cap needs the paragraph of text it sits in.".into()));
        }
        let mut touched = vec![id];
        let caret;
        match (kind, frame_ix) {
            ("none", None) => return Ok(OpResult::default()),
            ("none", Some(f)) => {
                // Put the letter back in front of the text.
                let Block::Para(fp) = list.remove(f) else { unreachable!() };
                let ti = if f < text_ix { text_ix - 1 } else { text_ix };
                let Block::Para(tp) = &mut list[ti] else { unreachable!() };
                let mut letters = fp.inlines;
                runs_in(&mut letters, &mut |r| {
                    if let Some(rpr) = r.rpr.as_mut() {
                        for n in ["w:position", "w:sz", "w:szCs"] {
                            rpr.remove_children(n);
                        }
                        if rpr.children.is_empty() {
                            r.rpr = None;
                        }
                    }
                });
                letters.append(&mut tp.inlines);
                tp.inlines = letters;
                edit::cleanup(&mut tp.inlines);
                if fp.ppr.as_ref().is_some_and(|pp| pp.child("w:pageBreakBefore").is_some()) {
                    put_ppr(tp.ppr.get_or_insert_with(|| Element::new("w:pPr")), Element::new("w:pageBreakBefore"));
                }
                caret = Pos::new(tp.id, 0);
                touched.push(tp.id);
            }
            (_, Some(f)) => {
                let Block::Para(tp) = &list[text_ix] else { unreachable!() };
                let (size, pitch) = drop_metrics(styles, tp);
                let tid = tp.id;
                let Block::Para(fp) = &mut list[f] else { unreachable!() };
                let mut fr = fp.ppr.as_ref().and_then(|pp| pp.child("w:framePr")).map(FrameFmt::from_el).unwrap_or_default();
                fr.drop_cap = Some(kind.to_string());
                fr.lines = Some(lines);
                if let Some(d) = o.distance {
                    fr.h_space = (d > 0).then_some(d);
                }
                style_drop_cap(fp, &fr, size, pitch, o.font.as_deref());
                // The frame paragraph is restyled whichever of the two the op named.
                touched.push(fp.id);
                caret = Pos::new(tid, 0);
                touched.push(tid);
            }
            (_, None) => {
                let Block::Para(tp) = &mut list[text_ix] else { unreachable!() };
                let text = tp.text();
                let first = text.chars().next().filter(|c| *c != '\u{FFFC}' && !c.is_whitespace());
                let Some(first) = first else {
                    return Err(EditError::Invalid("A drop cap needs a letter at the start of the paragraph.".into()));
                };
                let (size, pitch) = drop_metrics(styles, tp);
                let (left, right) = edit::partition(std::mem::take(&mut tp.inlines), first.len_utf16());
                tp.inlines = right;
                let mut fp = Paragraph::new(new_id);
                fp.attrs = tp.attrs.iter().filter(|(k, _)| !k.starts_with("w14:")).cloned().collect();
                fp.inlines = left;
                // The frame keeps the paragraph's style; what belongs to the text beside it stays with the text.
                let mut ppr = tp.ppr.clone().unwrap_or_else(|| Element::new("w:pPr"));
                let moved_break = ppr.remove_children("w:pageBreakBefore") > 0;
                for n in ["w:sectPr", "w:numPr", "w:ind", "w:jc", "w:pBdr", "w:shd", "w:tabs", "w:outlineLvl", "w:rPr", "w:mirrorIndents", "w:framePr"] {
                    ppr.remove_children(n);
                }
                if let Some(tpp) = tp.ppr.as_mut() {
                    tpp.remove_children("w:pageBreakBefore");
                }
                if moved_break {
                    put_ppr(&mut ppr, Element::new("w:pageBreakBefore"));
                }
                put_ppr(&mut ppr, Element::new("w:keepNext"));
                fp.ppr = Some(ppr);
                let fr = FrameFmt {
                    drop_cap: Some(kind.to_string()),
                    lines: Some(lines),
                    wrap: Some("around".into()),
                    v_anchor: Some("text".into()),
                    h_anchor: Some("text".into()),
                    h_space: o.distance.filter(|d| *d > 0),
                    ..Default::default()
                };
                style_drop_cap(&mut fp, &fr, size, pitch, o.font.as_deref());
                caret = Pos::new(tp.id, 0);
                touched.push(tp.id);
                touched.push(new_id);
                list.insert(text_ix, Block::Para(fp));
            }
        }
        for t in touched {
            self.touch(t);
        }
        Ok(OpResult { caret: Some(caret), created: if kind != "none" && frame_ix.is_none() { vec![new_id] } else { vec![] }, ..Default::default() })
    }
}

/// Give a drop cap's paragraph its frame, its exact line height and its big letter, the way Word writes them.
fn style_drop_cap(fp: &mut Paragraph, fr: &FrameFmt, size: f64, pitch: f64, font: Option<&str>) {
    let lines = fr.lines.unwrap_or(3) as f64;
    // N lines of the text beside it, and a letter a little smaller than that (its cap height is what fills the lines).
    let height = (lines * pitch).max(pitch);
    let letter_hp = ((height / 0.75) * 2.0).round().max(size * 2.0);
    let drop_hp = (letter_hp * 0.0727).round();
    let ppr = fp.ppr.get_or_insert_with(|| Element::new("w:pPr"));
    put_ppr(ppr, fr.element());
    put_ppr(ppr, Element::new("w:spacing").with_attr("w:after", "0").with_attr("w:line", ((height * 20.0).round() as i64).to_string()).with_attr("w:lineRule", "exact"));
    put_ppr(ppr, Element::new("w:textAlignment").with_attr("w:val", "baseline"));
    runs_in(&mut fp.inlines, &mut |r| {
        let rpr = r.rpr.get_or_insert_with(|| Element::new("w:rPr"));
        crate::props::put_rpr(rpr, Element::new("w:position").with_attr("w:val", format!("-{}", drop_hp as i64)));
        crate::props::put_rpr(rpr, Element::new("w:sz").with_attr("w:val", (letter_hp as i64).to_string()));
        crate::props::put_rpr(rpr, Element::new("w:szCs").with_attr("w:val", (letter_hp as i64).to_string()));
        if let Some(f) = font {
            crate::props::put_rpr(rpr, Element::new("w:rFonts").with_attr("w:ascii", f).with_attr("w:hAnsi", f).with_attr("w:cs", f));
        }
    });
}

/// Is this paragraph a page number, the kind the gallery writes: a PAGE field alone (with any dashes or brackets
/// around it), or "Page { PAGE } of { NUMPAGES }" in some language? A line that holds anything else as well, such as
/// a company name beside the number, is the person's own and is never one.
fn is_page_number(p: &Paragraph) -> bool {
    // The text outside the PAGE and NUMPAGES fields: before the first, between them, after the last.
    let text: Vec<u16> = p.text().encode_utf16().collect();
    let mut fields: Vec<(usize, usize, bool)> = Vec::new(); // (start, end, is NUMPAGES)
    let mut other_field = false;
    crate::fields::scan_inlines(&p.inlines, 0, &mut |s| {
        let code = crate::fields::Instr::parse(s.instr).code;
        if code == "PAGE" || code == "NUMPAGES" {
            fields.push((s.offset, s.offset + s.result.encode_utf16().count(), code == "NUMPAGES"));
        } else {
            other_field = true;
        }
    });
    if other_field || fields.is_empty() || fields.iter().filter(|f| !f.2).count() != 1 || fields.iter().filter(|f| f.2).count() > 1 {
        return false;
    }
    fields.sort();
    let mut outside: Vec<String> = Vec::new();
    let mut at = 0;
    for (s, e, _) in &fields {
        if *s < at || *e > text.len() {
            return false;
        }
        outside.push(String::from_utf16_lossy(&text[at..*s]));
        at = *e;
    }
    outside.push(String::from_utf16_lossy(&text[at..]));
    // Decoration is spaces and punctuation; the words are the gallery's "Page" and "of", one or two each, before the
    // number and between the two.
    let words = |s: &str| -> Option<usize> {
        if s.chars().any(|c| c == '\t' || c == '\n' || c == '\u{FFFC}' || c.is_numeric()) {
            return None;
        }
        Some(s.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).count())
    };
    let counts: Option<Vec<usize>> = outside.iter().map(|s| words(s)).collect();
    let Some(counts) = counts else { return false };
    if fields.len() == 1 {
        counts.iter().all(|n| *n == 0)
    } else {
        // Page { PAGE } of { NUMPAGES }: some words in front, some between, none after.
        let (page, of, after) = (counts[0], counts[1], counts[2]);
        page <= 2 && (1..=2).contains(&of) && after == 0 || page == 0 && of == 0 && after == 0
    }
}

/// Take page numbers out of a header or footer. Returns how many went.
fn strip_page_numbers(blocks: &mut Vec<Block>) -> usize {
    let mut n = 0;
    blocks.retain(|b| {
        let gone = matches!(b, Block::Para(p) if is_page_number(p));
        n += usize::from(gone);
        !gone
    });
    n
}
