//! Sections and page setup: reading and editing section properties, section breaks, headers and footers, page background.

use super::*;

impl Document {
    pub fn page_setup(&self) -> PageSetup {
        let setup = self.page_setup_now();
        // What `fonts_used` reads of it, kept for this state of the document.
        if let Ok(mut kept) = self.fonts_stories.lock() {
            *kept = Some((self.generation, story_fonts_of(&setup)));
        }
        setup
    }

    pub(super) fn page_setup_now(&self) -> PageSetup {
        let refs = self.section_refs();
        let sect = refs.last().and_then(|r| r.1);
        let num = |e: Option<&Element>, a: &str, d: i64| e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64).unwrap_or(d);
        let pgsz = sect.and_then(|s| s.child("w:pgSz"));
        let mar = sect.and_then(|s| s.child("w:pgMar"));
        let cols = sect.and_then(|s| s.child("w:cols"));
        let note_nums = self.note_state();
        // Header and footer stories used by any section, and each section's references with Word's
        // inheritance: a section without a reference of some variant uses the previous section's.
        let mut parts: HashMap<String, RStory> = HashMap::new();
        let mut sections = Vec::new();
        let (mut cur_h, mut cur_f) = (HashMap::new(), HashMap::new());
        let mut marks_by_part: HashMap<String, Option<Watermark>> = HashMap::new();
        let empty = Element::new("w:sectPr");
        for (end, el) in &refs {
            let el = el.unwrap_or(&empty);
            for (tag, cur) in [("w:headerReference", &mut cur_h), ("w:footerReference", &mut cur_f)] {
                for r in el.children_named(tag) {
                    let kind = r.attr("w:type").unwrap_or("default").to_string();
                    let Some(rid) = r.attr("r:id") else { continue };
                    let Some(rel) = self.pkg.resolve_rel(&self.main_path, rid) else { continue };
                    let Some(story) = self.stories.iter().find(|s| s.part == rel.target) else { continue };
                    if !parts.contains_key(&story.part) {
                        // A list in a header or footer counts on its own, as in Word.
                        let labels = self.numbering.labels(&story.blocks, &self.styles);
                        let mut ctx = self.story_ctx(&story.part, &labels, &note_nums);
                        parts.insert(story.part.clone(), RStory { part: story.part.clone(), blocks: ctx.blocks(&story.blocks) });
                    }
                    cur.insert(kind, story.part.clone());
                }
            }
            let (pgsz, mar) = (el.child("w:pgSz"), el.child("w:pgMar"));
            let watermark = ["default", "first", "even"].iter().find_map(|v| {
                let part = cur_h.get(*v)?;
                marks_by_part.entry(part.clone()).or_insert_with(|| self.watermark_of(part)).clone()
            });
            let linked = |tag: &str| -> Vec<String> {
                if sections.is_empty() {
                    return Vec::new();
                }
                let own: Vec<&str> = el.children_named(tag).map(|r| r.attr("w:type").unwrap_or("default")).collect();
                ["default", "first", "even"].iter().filter(|v| !own.contains(v)).map(|v| v.to_string()).collect()
            };
            let (linked_headers, linked_footers) = (linked("w:headerReference"), linked("w:footerReference"));
            sections.push(SectionInfo {
                end: *end,
                start: section_type(Some(el)),
                width: num(pgsz, "w:w", 12240),
                height: num(pgsz, "w:h", 15840),
                margin: [num(mar, "w:top", 1440), num(mar, "w:right", 1440), num(mar, "w:bottom", 1440), num(mar, "w:left", 1440)],
                header: num(mar, "w:header", 720),
                footer: num(mar, "w:footer", 720),
                gutter: num(mar, "w:gutter", 0),
                rtl_gutter: el.child("w:rtlGutter").is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off"))),
                cols: read_columns(el),
                line_numbers: read_line_numbers(el),
                title_page: el.child("w:titlePg").is_some_and(|t| t.attr("w:val") != Some("0")),
                page_start: el.child("w:pgNumType").and_then(|n| n.attr("w:start")).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64),
                page_format: el.child("w:pgNumType").and_then(|n| n.attr("w:fmt")).filter(|f| *f != "decimal").map(String::from),
                headers: cur_h.clone(),
                footers: cur_f.clone(),
                linked_headers,
                linked_footers,
                watermark,
                extras: {
                    let mut x = crate::layout_props::SectionExtras::read(el, self.settings.as_ref().map(|d| &d.root));
                    // The size a grid added later would count characters from.
                    x.grid_base = self.inherited_run(None).size;
                    x
                },
            });
        }
        // The final section's own references.
        let mut headers = HashMap::new();
        let mut footers = HashMap::new();
        if let Some(s) = sect {
            for (tag, map) in [("w:headerReference", &mut headers), ("w:footerReference", &mut footers)] {
                for r in s.children_named(tag) {
                    let kind = r.attr("w:type").unwrap_or("default").to_string();
                    let Some(rid) = r.attr("r:id") else { continue };
                    let Some(rel) = self.pkg.resolve_rel(&self.main_path, rid) else { continue };
                    if let Some(st) = parts.get(&rel.target) {
                        map.insert(kind, st.clone());
                    }
                }
            }
        }
        let (foot_fmt, end_fmt) = self.note_formats();
        let note_labels = self.note_labels();
        let note_order = self.note_order();
        let notes_of = |kind: StoryKind, fmt: &notes::NoteFormat, prefix: &str| -> Vec<RNote> {
            let mut out: Vec<(usize, RNote)> = self
                .stories
                .iter()
                .filter(|s| s.kind == kind)
                .filter_map(|s| {
                    let id = s.note_id.clone()?;
                    let key = format!("{prefix}{id}");
                    let at = *note_order.get(&key)?;
                    // A note with a mark of its own has no number.
                    let num = note_nums.get(&key).copied().unwrap_or(0);
                    // A list in a note counts on its own.
                    let labels = self.numbering.labels(&s.blocks, &self.styles);
                    let mut ctx = self.story_ctx(&s.part, &labels, &note_nums);
                    ctx.current_note = Some(num);
                    ctx.current_endnote = kind == StoryKind::Endnote;
                    let label = if num == 0 { String::new() } else { note_labels.get(&key).cloned().unwrap_or_else(|| fmt.label(num)) };
                    ctx.current_note_label = Some(label.clone());
                    Some((at, RNote { id, num, label, blocks: ctx.blocks(&s.blocks) }))
                })
                .collect();
            out.sort_by_key(|(at, _)| *at);
            out.into_iter().map(|(_, n)| n).collect()
        };
        let notes = notes_of(StoryKind::Footnote, &foot_fmt, "");
        let endnotes = notes_of(StoryKind::Endnote, &end_fmt, "e:");
        let watermark = sections.last().and_then(|s| s.watermark.clone()).or_else(|| sections.iter().find_map(|s| s.watermark.clone()));
        let setting = |name: &str| self.settings.as_ref().and_then(|d| d.root.child(name)).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")));
        PageSetup {
            width: num(pgsz, "w:w", 12240),
            height: num(pgsz, "w:h", 15840),
            margin: [num(mar, "w:top", 1440), num(mar, "w:right", 1440), num(mar, "w:bottom", 1440), num(mar, "w:left", 1440)],
            header: num(mar, "w:header", 720),
            footer: num(mar, "w:footer", 720),
            cols: num(cols, "w:num", 1),
            title_page: sect.is_some_and(|s| s.child("w:titlePg").is_some_and(|t| t.attr("w:val") != Some("0"))),
            headers,
            footers,
            notes,
            endnotes,
            sections,
            parts,
            page_color: self.page_background(),
            hyphenation: setting("w:autoHyphenation"),
            default_tab: self.settings.as_ref().and_then(|d| d.root.child("w:defaultTabStop")).and_then(|e| e.attr("w:val")).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64).filter(|v| *v > 0).unwrap_or(720),
            even_and_odd_headers: setting("w:evenAndOddHeaders"),
            lang: self.styles.doc.as_ref().and_then(|d| d.root.path(&["w:docDefaults", "w:rPrDefault", "w:rPr", "w:lang"])).and_then(|l| l.attr("w:val")).map(String::from),
            watermark,
            layout: crate::layout_props::LayoutSettings::read(self.settings.as_ref().map(|d| &d.root)),
            page_fill: self.page_fill(),
            note_seps: self.note_seps(),
        }
    }

    /// Section properties in document order, each with the paragraph that ends it (None for the
    /// body-level `w:sectPr` of the final section, which is missing from a few odd files).
    pub(super) fn section_refs(&self) -> Vec<(Option<NodeId>, Option<&Element>)> {
        if !self.facts_fresh() {
            return self.section_refs_walked();
        }
        let kept = self.section_refs_kept();
        #[cfg(any(test, debug_assertions))]
        assert!(kept == crate::perf::unmetered(|| self.section_refs_walked()), "section properties built from the kept facts differ from the document's");
        kept
    }

    pub(super) fn section_refs_walked(&self) -> Vec<(Option<NodeId>, Option<&Element>)> {
        let body = self.main_body();
        let mut out = Vec::new();
        walk_paragraphs(body, &mut |p| {
            if let Some(s) = p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr")) {
                out.push((Some(p.id), Some(s)));
            }
        });
        let last = body.iter().rev().find_map(|b| match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => Some(&o.el),
            _ => None,
        });
        out.push((None, last));
        out
    }

    /// Section breaks by paragraph id: how the section that follows the one ending at that paragraph starts.
    pub fn section_breaks(&self) -> HashMap<NodeId, String> {
        let refs = self.section_refs();
        refs.windows(2).filter_map(|w| w[0].0.map(|id| (id, section_type(w[1].1)))).collect()
    }

    /// Index (into `PageSetup.sections`) of the section holding a paragraph; the last one for story paragraphs.
    pub fn section_of(&self, pid: NodeId) -> usize {
        let mut n = 0;
        let mut found = None;
        walk_paragraphs(self.main_body(), &mut |p| {
            if found.is_none() {
                if p.id == pid {
                    found = Some(n);
                } else if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) {
                    n += 1;
                }
            }
        });
        found.unwrap_or(n)
    }

    /// Run `f` on the section properties of the section holding paragraph `scope`, or of every section.
    pub(super) fn edit_sects(&mut self, scope: Option<Id>, f: &mut dyn FnMut(&mut Element)) -> Result<(), EditError> {
        let target = match scope {
            Some(id) => {
                self.para_ref(id.0)?;
                Some(self.section_of(id.0))
            }
            None => None,
        };
        self.final_sect_mut();
        let mut n = 0;
        let mut edited = Vec::new();
        self.body.edit_paragraphs(&mut |p| {
            let mut changed = false;
            if let Some(s) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:sectPr")) {
                if target.is_none_or(|t| t == n) {
                    f(s);
                    edited.push(p.id);
                    changed = true;
                }
                n += 1;
            }
            changed
        });
        for id in edited {
            self.touch(id);
        }
        if target.is_none_or(|t| t == n) {
            f(&mut self.final_sect_mut().el);
        }
        Ok(())
    }

    /// Edit `settings.xml` (created if `create` and missing); marks it for saving only when it changed.
    pub(super) fn settings_edit(&mut self, create: bool, f: &mut dyn FnMut(&mut Element)) {
        if self.settings.is_none() {
            if !create {
                return;
            }
            self.settings = Some(XmlDoc::new(Element::new("w:settings").with_attr("xmlns:w", NS_W)));
            self.settings_dirty = true;
        }
        let Some(st) = self.settings.as_mut() else { return };
        let before = st.root.clone();
        f(&mut st.root);
        if st.root != before {
            self.settings_dirty = true;
        }
    }

    /// Set or remove the page background element (always the first element of `w:document`).
    pub(super) fn set_background(&mut self, bg: Option<Element>) {
        let root = &mut self.doc_xml.root;
        let at = root.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "w:background"));
        match (bg, at) {
            (Some(b), Some(i)) => root.children[i] = Node::Element(b),
            (Some(b), None) => {
                let first = root.children.iter().position(|n| matches!(n, Node::Element(_))).unwrap_or(root.children.len());
                root.children.insert(first, Node::Element(b));
            }
            (None, Some(i)) => {
                root.children.remove(i);
            }
            (None, None) => {}
        }
    }

    pub(super) fn final_sect_pr(&self) -> Option<Element> {
        self.main_body().iter().rev().find_map(|b| match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => Some(o.el.clone()),
            _ => None,
        })
    }

    /// Usable text width in twips.
    pub fn text_width(&self) -> i64 {
        let p = self.page_setup_light();
        (p.0 - p.1 - p.2).max(1440)
    }

    pub(super) fn page_setup_light(&self) -> (i64, i64, i64) {
        let sect = self.final_sect_pr();
        let n = |path: &str, a: &str, d: i64| {
            sect.as_ref().and_then(|s| s.child(path)).and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64).unwrap_or(d)
        };
        (n("w:pgSz", "w:w", 12240), n("w:pgMar", "w:left", 1440), n("w:pgMar", "w:right", 1440))
    }

    /// The last section's properties (created if the body has none).
    pub(super) fn final_sect_mut(&mut self) -> &mut Opaque {
        let idx = match self.body.iter().rposition(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")) {
            Some(i) => i,
            None => {
                let id = self.ids.fresh();
                self.body.push(Block::Opaque(Opaque { id, el: Element::new("w:sectPr") }));
                self.body.len() - 1
            }
        };
        // Handing out the properties to change: the block is marked as changed (for drawing and for undo's copies).
        let id = self.body[idx].id();
        self.changed.insert(id);
        let Block::Opaque(o) = &mut self.body[idx] else { unreachable!() };
        o
    }

    /// A section break: the paragraph splits at `at` and its first half ends a new section holding the
    /// governing section's old properties; the governing section now starts the way `ty` says.
    pub(super) fn op_section_break(&mut self, at: Pos, ty: &str) -> Result<OpResult, EditError> {
        if !self.body.iter().any(|b| matches!(b, Block::Para(p) if p.id == at.p.0)) {
            self.para_ref(at.p.0)?;
            return Err(EditError::Invalid("Section breaks can only go in the main text, outside tables and content controls. Move to a regular paragraph first.".into()));
        }
        self.final_sect_mut();
        let idx = self.section_of(at.p.0);
        let first = self.section_refs().get(idx).and_then(|r| r.1).cloned().unwrap_or_else(|| Element::new("w:sectPr"));
        self.edit_sects(Some(at.p), &mut |sect| {
            set_sect_type(sect, ty);
            // A page number restart belongs to the start of the section, which the first half now holds.
            if let Some(n) = sect.child_mut("w:pgNumType") {
                n.remove_attr("w:start");
                if n.attrs.is_empty() {
                    sect.remove_children("w:pgNumType");
                }
            }
        })?;
        let r = self.split_paragraph(at)?;
        let p = self.para_mut(at.p.0)?;
        put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), first);
        self.touch_all();
        Ok(r)
    }

    /// Run `f` on the section properties of the section holding paragraph `scope`, or of the last section.
    pub(super) fn with_sect(&mut self, scope: Option<Id>, f: &mut dyn FnMut(&mut Element)) -> Result<(), EditError> {
        match scope {
            Some(_) => self.edit_sects(scope, f),
            None => {
                f(&mut self.final_sect_mut().el);
                Ok(())
            }
        }
    }

    /// The section properties of the section holding paragraph `scope`, or of the last section.
    pub(super) fn sect_of(&self, scope: Option<Id>) -> Result<Option<Element>, EditError> {
        match scope {
            Some(id) => {
                self.para_ref(id.0)?;
                Ok(self.section_refs().get(self.section_of(id.0)).and_then(|r| r.1).cloned())
            }
            None => Ok(self.final_sect_pr()),
        }
    }

    /// A free part name like `word/header3.xml` next to the main part.
    pub(super) fn free_part(&self, stem: &str) -> String {
        let dir = self.main_path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
        let mut n = 1;
        loop {
            let name = format!("{dir}{stem}{n}.xml");
            if !self.pkg.has(&name) {
                return name;
            }
            n += 1;
        }
    }

    pub(super) fn op_add_header_footer(&mut self, kind: HeaderKind, variant: &str, section: Option<Id>) -> Result<OpResult, EditError> {
        if !matches!(variant, "default" | "first" | "even") {
            return Err(EditError::Invalid(format!("unsupported header/footer variant '{variant}' (use default, first or even)")));
        }
        self.add_header_footer_variant(kind, variant, self.past_cover(section))
    }

    /// Find or create a header/footer of a section; `variant` may also be `even`.
    pub(super) fn add_header_footer_variant(&mut self, kind: HeaderKind, variant: &str, section: Option<Id>) -> Result<OpResult, EditError> {
        let (tag, skind, style, ct, rel_type, stem) = match kind {
            HeaderKind::Header => ("w:headerReference", StoryKind::Header, "Header", package::CT_HEADER, package::REL_HEADER, "header"),
            HeaderKind::Footer => ("w:footerReference", StoryKind::Footer, "Footer", package::CT_FOOTER, package::REL_FOOTER, "footer"),
        };
        let first_para = |blocks: &[Block]| paragraph_order(blocks).first().map(|id| Pos::new(*id, 0));
        // Already there: just put the caret in it.
        let existing = self
            .sect_of(section)?
            .and_then(|s| s.children_named(tag).find(|r| r.attr("w:type").unwrap_or("default") == variant).and_then(|r| r.attr("r:id")).map(String::from))
            .and_then(|rid| self.pkg.resolve_rel(&self.main_path, &rid))
            .and_then(|rel| self.stories.iter().find(|s| s.part == rel.target));
        if let Some(st) = existing {
            return Ok(OpResult { caret: first_para(&st.blocks), ..Default::default() });
        }
        let style_id = self.styles.ensure_builtin(style).unwrap_or_else(|| style.to_string());
        let part = self.free_part(stem);
        let doc = stories::header_part(skind, &style_id);
        let blocks = parse_blocks(&doc.root, &mut self.ids);
        self.pkg.set(&part, doc.to_bytes());
        self.pkg.ensure_override(&part, ct);
        let main = self.main_path.clone();
        let rid = self.pkg.add_rel(&main, rel_type, &relative_target(&main, &part), false);
        self.ensure_ns("r", NS_R);
        self.with_sect(section, &mut |s| {
            stories::add_reference(s, tag, variant, &rid);
            if variant == "first" {
                crate::props::put_ordered(s, Element::new("w:titlePg"), SECT_ORDER);
            }
        })?;
        let caret = first_para(&blocks);
        let created = blocks.iter().map(|b| b.id()).collect();
        self.story_gen += 1;
        self.hf_candidates.insert(part.clone());
        self.stories.push(Story { kind: skind, part: part.clone(), note_id: None, blocks, gen: self.story_gen });
        self.story_parts.parts.push((part, doc));
        self.stories_touched = true;
        Ok(OpResult { caret, created, ..Default::default() })
    }

    /// Header part names by variant for each section, with Word's inheritance (a section without a
    /// reference of some variant uses the previous section's), plus the section's properties.
    pub(super) fn section_headers(&self) -> Vec<SectionHeaders> {
        let mut cur: HashMap<String, String> = HashMap::new();
        let mut out = Vec::new();
        for (_, el) in self.section_refs() {
            if let Some(el) = el {
                for r in el.children_named("w:headerReference") {
                    let (Some(rid), kind) = (r.attr("r:id"), r.attr("w:type").unwrap_or("default")) else { continue };
                    let Some(rel) = self.pkg.resolve_rel(&self.main_path, rid) else { continue };
                    if self.stories.iter().any(|s| s.part == rel.target) {
                        cur.insert(kind.to_string(), rel.target);
                    }
                }
            }
            out.push(SectionHeaders { sect: el.cloned(), headers: cur.clone() });
        }
        out
    }
}

/// How a section starts (`w:type`); `nextPage` when the section doesn't say.
/// A section's properties and the header parts it shows by variant, with inheritance applied.
pub(super) struct SectionHeaders {
    pub(super) sect: Option<Element>,
    pub(super) headers: HashMap<String, String>,
}

/// The text area of a section in points (width, height), where a watermark has to fit.
pub(super) fn text_area(sect: Option<&Element>) -> (f64, f64) {
    let n = |c: &str, a: &str, d: f64| sect.and_then(|s| s.child(c)).and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let (w, h) = (n("w:pgSz", "w:w", 12240.0), n("w:pgSz", "w:h", 15840.0));
    let (t, r, b, l) = (n("w:pgMar", "w:top", 1440.0), n("w:pgMar", "w:right", 1440.0), n("w:pgMar", "w:bottom", 1440.0), n("w:pgMar", "w:left", 1440.0));
    (((w - l - r) / 20.0).max(72.0), ((h - t - b) / 20.0).max(72.0))
}

pub(super) fn section_type(sect: Option<&Element>) -> String {
    sect.and_then(|s| s.child("w:type")).and_then(|t| t.attr("w:val")).unwrap_or("nextPage").to_string()
}

pub(super) fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on"))
}

pub(super) fn read_columns(sect: &Element) -> Columns {
    let c = sect.child("w:cols");
    let num = |a: &str| c.and_then(|c| c.attr(a)).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64);
    let unequal = c.is_some_and(|c| matches!(c.attr("w:equalWidth"), Some("0" | "false" | "off")));
    let widths: Vec<i64> = match c {
        Some(c) if unequal => c.children_named("w:col").filter_map(|k| k.attr("w:w")?.parse::<f64>().ok().map(|v| v as i64)).collect(),
        _ => Vec::new(),
    };
    let spaces = match c {
        Some(c) if unequal => c
            .children_named("w:col")
            .take(widths.len().saturating_sub(1))
            .map(|k| k.attr("w:space").and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64).unwrap_or(720))
            .collect(),
        _ => Vec::new(),
    };
    Columns { num: num("w:num").unwrap_or(1), space: num("w:space").unwrap_or(720), sep: truthy(c.and_then(|c| c.attr("w:sep"))), widths, spaces }
}

pub(super) fn read_line_numbers(sect: &Element) -> Option<LineNumbers> {
    let l = sect.child("w:lnNumType")?;
    let num = |a: &str| l.attr(a).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64);
    let count_by = num("w:countBy").filter(|c| *c >= 1)?;
    Some(LineNumbers { count_by, start: num("w:start").unwrap_or(1), restart: l.attr("w:restart").unwrap_or("newPage").to_string(), distance: num("w:distance") })
}

/// Set a section's start type (`nextPage`, the default, is written as no `w:type`).
pub(super) fn set_sect_type(sect: &mut Element, ty: &str) {
    if ty == "nextPage" {
        sect.remove_children("w:type");
    } else {
        crate::props::put_ordered(sect, Element::new("w:type").with_attr("w:val", ty), SECT_ORDER);
    }
}
