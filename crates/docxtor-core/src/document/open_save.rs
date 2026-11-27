//! Opening a document, saving it, and keeping the package in step with the parts the document holds.

use super::*;

impl Document {
    /// Open a Word file. A damaged package is repaired and Strict Open XML is read as the standard kind
    /// (`crate::open` says what was done); a package that is sound is read as it is, part for part.
    pub fn open(bytes: &[u8]) -> Result<Self, DocError> {
        Self::open_with(bytes, &crate::budget::Budget::file())
    }

    /// `open`, spending from `budget`: the one budget of a file, shared with every document nested inside it.
    pub fn open_with(bytes: &[u8], budget: &crate::budget::Budget) -> Result<Self, DocError> {
        let (pkg, _) = crate::open::open_package(bytes, budget).map_err(|e| DocError::Open(e.message))?;
        let doc = Self::from_package_with(pkg, budget)?;
        budget.check().map_err(|e| DocError::Open(e.to_string()))?;
        Ok(doc)
    }

    pub fn new_blank() -> Self {
        let mut pkg = Package::default();
        for (name, data) in blank_parts() {
            pkg.set(name, data.into_bytes());
        }
        let mut doc = Self::from_package(pkg).expect("blank template is valid");
        doc.fresh = true;
        doc
    }

    pub fn from_package(pkg: Package) -> Result<Self, DocError> {
        Self::from_package_with(pkg, &crate::budget::Budget::default())
    }

    pub fn from_package_with(pkg: Package, budget: &crate::budget::Budget) -> Result<Self, DocError> {
        let main_path = pkg.main_document_path();
        let mut doc_xml = pkg.xml(&main_path)?;
        let rels = pkg.rels_for(&main_path)?;
        let find = |t: &str| rels.iter().find(|r| r.rel_type == t).map(|r| package::resolve_target(&main_path, &r.target));
        let styles_path = find(package::REL_STYLES).unwrap_or_else(|| "word/styles.xml".into());
        let numbering_path = find(package::REL_NUMBERING).unwrap_or_else(|| "word/numbering.xml".into());
        let settings_path = find(package::REL_SETTINGS).unwrap_or_else(|| "word/settings.xml".into());
        let settings = pkg.xml(&settings_path).ok();
        let core_path = pkg
            .rels_for("")
            .unwrap_or_default()
            .iter()
            .find(|r| r.rel_type == package::REL_CORE)
            .map(|r| package::resolve_target("", &r.target))
            .unwrap_or_else(|| "docProps/core.xml".into());
        let core = pkg.xml(&core_path).ok();
        let custom_path = pkg
            .rels_for("")
            .unwrap_or_default()
            .iter()
            .find(|r| r.rel_type == crate::protection::REL_CUSTOM_PROPS)
            .map(|r| package::resolve_target("", &r.target))
            .unwrap_or_else(|| "docProps/custom.xml".into());
        let custom = pkg.xml(&custom_path).ok();
        let app_path = pkg
            .rels_for("")
            .unwrap_or_default()
            .iter()
            .find(|r| r.rel_type == docmeta::REL_APP)
            .map(|r| package::resolve_target("", &r.target))
            .unwrap_or_else(|| "docProps/app.xml".into());
        let app = pkg.xml(&app_path).ok();
        let theme = find(package::REL_THEME)
            .and_then(|p| pkg.xml(&p).ok())
            .map(|t| ThemeFonts::parse(&t))
            .unwrap_or_default();
        let styles = StyleSheet::parse(pkg.xml(&styles_path).ok(), theme);
        let mut numbering = Numbering::parse(pkg.xml(&numbering_path).ok());
        numbering.rels = lists::numbering_rels(&pkg, &numbering_path);

        let mut ids = IdGen(0);
        let body_el = doc_xml.root.child_mut("w:body").ok_or(DocError::NoBody)?;
        // Each block's XML is dropped as soon as it is model, so the whole tree and the whole model are never in memory
        // together: that was the tallest moment of opening a long document (7.4 KB a Word-style paragraph, 6.7 after).
        let mut body = crate::model::parse_blocks_taking(body_el, &mut ids);
        if !body.iter().any(|b| matches!(b, Block::Para(_) | Block::Table(_) | Block::Container(_))) {
            let pos = body.iter().position(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).unwrap_or(body.len());
            body.insert(pos, Block::Para(Paragraph::new(ids.fresh())));
        }
        let (mut stories, story_parts) = stories::load(&pkg, &main_path, &body, &mut ids);
        crate::shapes::story::load(&main_path, &mut body, &mut stories, &mut ids);
        let cites = crate::citations::CiteState::load(&pkg, &main_path);
        let merge = crate::merge::state::MergeState::load(&pkg, &main_path);
        let tabs = crate::tabs::TabsState::load(&pkg, &main_path, budget);
        let theme_part = Document::load_theme(&pkg, find(package::REL_THEME));
        let mut d = Document {
            cites,
            merge,
            merge_preview: None,
            access: Access::Edit,
            tabs,
            charts_dirty: false,
            shape_theme: Default::default(),
            theme: theme_part,
            theme_dirty: false,
            active_part: main_path.clone(),
            stories,
            story_parts,
            dirty_parts: BTreeSet::new(),
            watermark_headers: BTreeSet::new(),
            fresh: false,
            story_gen: 0,
            hf_candidates: BTreeSet::new(),
            baseline_orphans: None,
            parked: None,
            restyled_main: false,
            stories_touched: false,
            touched_stories: Vec::new(),
            last_notes: HashMap::new(),
            csl_styles: HashMap::new(),
            scrub: Default::default(),
            pkg,
            main_path,
            doc_xml,
            body: Body::from(body),
            styles,
            numbering,
            styles_path,
            numbering_path,
            settings,
            settings_path,
            settings_dirty: false,
            core,
            core_path,
            core_dirty: false,
            custom,
            custom_path,
            custom_dirty: false,
            app,
            app_path,
            app_dirty: false,
            save_info: SaveInfo::default(),
            user_groups: Vec::new(),
            ids,
            body_dirty: false,
            changed: Changed::default(),
            #[cfg(any(test, debug_assertions))]
            snap_op: String::new(),
            facts: Default::default(),
            shapes: Default::default(),
            words: Default::default(),
            locks: Default::default(),
            bookmarks: Default::default(),
            body_xml: Default::default(),
            last_labels: HashMap::new(),
            last_breaks: HashMap::new(),
            last_order: Vec::new(),
            last_key: Default::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            copies: PartCopies::default(),
            undo_budget: UNDO_BYTES,
            undo_labels: Vec::new(),
            revision: 0,
            generation: 0,
            opening: false,
            fonts_body: FontsKept::default(),
            fonts_stories: Default::default(),
            redo_labels: Vec::new(),
            undo_base: 0,
            pins: Vec::new(),
            drawing_id: 5000,
            alt_chunks: HashMap::new(),
            alt_dirty: Vec::new(),
            alt_stamp: None,
            inv: Default::default(),
        };
        d.load_alt_chunks(budget);
        // A cell ends with a paragraph, as every save writes it (`model::ends_in_paragraph`): a cell that ends with a nested table
        // gets the empty paragraph Word shows there, once, here, so what the engine shows is what a save writes and opens again
        // (an edit, or an undo, that wrote the table again used to add it to the file and not to the text). Not an edit: the
        // parts are saved as the file has them until something changes.
        d.body.edit_each(|s| after_batch::cells_end_in_paragraph(s, &mut d.ids));
        for s in d.stories.iter_mut() {
            after_batch::cells_end_in_paragraph(&mut s.blocks, &mut d.ids);
        }
        d.last_labels = d.numbering.labels(&d.body, &d.styles);
        d.last_breaks = d.section_breaks();
        d.last_key = d.render_key();
        d.last_notes = d.note_state();
        // Controls bound to custom XML show its values, as Word does when it opens the file. That is not an edit, so it adds no
        // placeholder style either (`opening`): a file with bound controls was saved with a style it did not have, or a styles part it
        // did not have, and an undo later wrote the styles part again.
        d.opening = true;
        d.sync_bound_controls();
        d.opening = false;
        d.changed.clear();
        // What opening showed in a header or footer is not an edit: the part is saved as the file had it.
        d.dirty_parts.clear();
        d.stories_touched = false;
        Ok(d)
    }

    /// Serialize to .docx bytes. Untouched parts are written back byte-for-byte.
    pub fn save(&mut self) -> Result<Vec<u8>, DocError> {
        self.save_as(Saving::Zip)
    }

    /// `save` without the compressing: the parts as `Package::pack_with` lays them out, for `package::zip_packed` to turn into the
    /// same bytes `save` returns (on another thread, or later).
    pub fn save_packed(&mut self) -> Result<Vec<u8>, DocError> {
        self.save_as(Saving::Packed)
    }

    /// `save_packed` handed on in pieces of at most `max` bytes (see `Package::pack_pieces`): what the browser gets, so the
    /// engine's memory, which never shrinks, does not hold a second copy of the whole package for it.
    pub fn save_packed_pieces(&mut self, max: usize, piece: &mut dyn FnMut(&[u8])) -> Result<(), DocError> {
        self.save_as(Saving::Pieces(max, piece)).map(|_| ())
    }

    pub(super) fn save_as(&mut self, how: Saving) -> Result<Vec<u8>, DocError> {
        self.flush_parts();
        // Images that no header uses any more (a removed picture watermark) are left out of the file, not
        // out of the package, so undo can still bring them back.
        let (mut skip, mut replace) = self.unused_header_images();
        // Header and footer parts no section refers to any more are left out too (see `header_link`).
        let mut pruned = self.pruned_package();
        // So are the parts Inspect document took out, and the properties it blanked (see `inspect`).
        if !self.scrub.is_empty() {
            let mut pkg = pruned.take().unwrap_or_else(|| self.pkg.clone());
            self.scrub.apply(&mut pkg);
            pruned = Some(pkg);
        }
        let pkg = pruned.as_ref().unwrap_or(&self.pkg);
        // Imported chunks the text no longer has (see `alt_chunks`).
        self.leave_out_gone_chunks(pkg, &mut skip, &mut replace);
        // And the pictures, charts and workbooks nothing uses any more (see `media_prune`).
        self.leave_out_unused_media(pkg, &mut skip, &mut replace);
        match how {
            Saving::Zip => {}
            Saving::Packed => {
                let bytes = pkg.pack_with(&skip, &replace);
                if self.inv_enabled() {
                    let zip = crate::package::zip_packed(&bytes)?;
                    self.inv_saved(&zip);
                }
                return Ok(bytes);
            }
            Saving::Pieces(max, piece) => {
                pkg.pack_pieces(&skip, &replace, max, piece);
                if self.inv_enabled() {
                    // The same bytes `zip_packed` makes of the pieces (checked in `tests/all/save_packed.rs`).
                    let zip = crate::perf::unmetered_check(|| pkg.to_bytes_with(&skip, &replace))?;
                    self.inv_saved(&zip);
                }
                return Ok(Vec::new());
            }
        }
        let bytes = pkg.to_bytes_with(&skip, &replace)?;
        self.inv_saved(&bytes);
        Ok(bytes)
    }

    /// Save as a program does: when the document changed since it was opened or last saved, `dcterms:modified`
    /// becomes `now` (an ISO 8601 UTC time such as `2026-10-13T09:30:00Z`, from the caller's clock), and a
    /// document made new gets `dcterms:created` too. An untouched document is saved byte for byte, as `save` does.
    pub fn save_stamped(&mut self, now: &str) -> Result<Vec<u8>, DocError> {
        self.stamp(now);
        self.save()
    }

    /// The first half of `save_stamped` (the dates and the statistics), for a save that goes on in steps.
    pub fn stamp(&mut self, now: &str) {
        if (self.is_modified() || self.fresh) && docprops::is_w3cdtf(now) {
            let personal_info_kept = !self.settings.as_ref().and_then(|s| s.root.child("w:removePersonalInformation")).is_some_and(|e| {
                !matches!(e.attr("w:val"), Some("0" | "false" | "off"))
            });
            let core = self.core.get_or_insert_with(docprops::new_core_doc);
            docprops::stamp(core, now, self.fresh);
            // Word keeps the name out when the file asks for personal information to be removed.
            if let Some(user) = personal_info_kept.then(|| self.save_info.user.clone()).flatten() {
                docprops::set_last_modified_by(core, &user);
            }
            self.core_dirty = true;
            self.fresh = false;
            // Word keeps its statistics in app.xml and rewrites them on every save.
            // The same counts of the main text that Document details shows.
            let main = self.main_counts();
            let stats = docmeta::Stats {
                pages: self.save_info.pages,
                words: main.words,
                characters: main.chars_no_spaces,
                characters_with_spaces: main.chars,
                paragraphs: main.paragraphs,
                lines: self.save_info.lines,
            };
            let app = self.app.get_or_insert_with(docmeta::new_app_doc);
            docmeta::write_stats(app, &stats);
            self.app_dirty = true;
        }
    }

    /// What the next `save_stamped` records besides the time: who is saving (`cp:lastModifiedBy`), and how many
    /// pages and lines the page laid the document out in (`Pages` and `Lines` of `app.xml`).
    pub fn set_save_info(&mut self, info: SaveInfo) {
        self.save_info = info;
    }

    /// An undo or redo that put back an older `document.xml` leaves nothing the last save wrote to copy from: the
    /// kept places point into a part that is no longer there (its length alone may not show it).
    pub(super) fn forget_body_xml_if_swapped(&mut self, delta: &[package::PartDelta]) {
        if delta.iter().any(|(n, _)| n.trim_start_matches('/') == self.main_path.trim_start_matches('/')) {
            self.body_xml = Default::default();
        }
    }

    /// Write modified in-memory structures back into the package.
    pub fn flush_parts(&mut self) {
        // A previewed record is not the document: the template is what gets written.
        if self.merge_preview.is_some() {
            // The blocks written are the template's, not the ones shown: nothing kept of the last write fits either.
            self.body_xml = Default::default();
            return self.with_template(|d| d.flush_parts());
        }
        if self.body_dirty {
            self.write_body_xml();
            self.body_dirty = false;
        }
        if self.styles.dirty {
            if let Some(s) = &self.styles.doc {
                let path = self.styles_path.clone();
                // A part without its relationship (an undo put back the relationships as they were) gets it again.
                if !self.main_links_to(&path) {
                    let target = relative_target(&self.main_path, &path);
                    self.pkg.add_rel(&self.main_path.clone(), package::REL_STYLES, &target, false);
                }
                self.pkg.ensure_override(&path, package::CT_STYLES);
                let mut s = s.clone();
                crate::charfmt::declare_w14(&mut s.root);
                self.pkg.set(&path, s.to_bytes());
            } else {
                let path = self.styles_path.clone();
                self.drop_main_part(&path);
            }
            self.styles.dirty = false;
        }
        if self.numbering.dirty {
            if let Some(n) = &self.numbering.doc {
                let path = self.numbering_path.clone();
                // A part without its relationship (an undo put back the relationships as they were) gets it again.
                if !self.main_links_to(&path) {
                    let target = relative_target(&self.main_path, &path);
                    self.pkg.add_rel(&self.main_path.clone(), package::REL_NUMBERING, &target, false);
                }
                self.pkg.ensure_override(&path, package::CT_NUMBERING);
                self.pkg.set(&path, n.to_bytes());
            } else {
                let path = self.numbering_path.clone();
                self.drop_main_part(&path);
            }
            self.numbering.dirty = false;
        }
        if self.settings_dirty {
            if let Some(st) = &self.settings {
                let path = self.settings_path.clone();
                // A part without its relationship (an undo put back the relationships as they were) gets it again.
                if !self.main_links_to(&path) {
                    let target = relative_target(&self.main_path, &path);
                    self.pkg.add_rel(&self.main_path.clone(), package::REL_SETTINGS, &target, false);
                }
                self.pkg.ensure_override(&path, package::CT_SETTINGS);
                self.pkg.set(&path, st.to_bytes());
            } else {
                let path = self.settings_path.clone();
                self.drop_main_part(&path);
            }
            self.settings_dirty = false;
        }
        if self.core_dirty {
            let path = self.core_path.clone();
            match &self.core {
                Some(c) => {
                    if !self.pkg.has(&path) {
                        self.pkg.add_rel("", package::REL_CORE, &path, false);
                        self.pkg.ensure_override(&path, package::CT_CORE);
                    }
                    self.pkg.set(&path, c.to_bytes());
                }
                // Undone back to a file without properties after a save wrote them: leave an empty set.
                None if self.pkg.has(&path) => self.pkg.set(&path, docprops::new_core_doc().to_bytes()),
                None => {}
            }
            self.core_dirty = false;
        }
        if self.custom_dirty {
            let path = self.custom_path.clone();
            if let Some(c) = &self.custom {
                if !self.pkg.has(&path) {
                    self.pkg.add_rel("", crate::protection::REL_CUSTOM_PROPS, &path, false);
                    self.pkg.ensure_override(&path, crate::protection::CT_CUSTOM_PROPS);
                }
                self.pkg.set(&path, c.to_bytes());
            }
            self.custom_dirty = false;
        }
        if self.app_dirty {
            let path = self.app_path.clone();
            if let Some(a) = &self.app {
                if !self.pkg.has(&path) {
                    self.pkg.add_rel("", docmeta::REL_APP, &path, false);
                    self.pkg.ensure_override(&path, docmeta::CT_APP);
                }
                self.pkg.set(&path, a.to_bytes());
            }
            self.app_dirty = false;
        }
        if !self.dirty_parts.is_empty() {
            stories::flush(&self.stories, &self.story_parts, &self.dirty_parts, &mut self.pkg);
            self.dirty_parts.clear();
        }
        let main = self.main_path.clone();
        self.cites.flush(&mut self.pkg, &main);
        self.merge.flush(&mut self.pkg, &main);
        self.tabs.flush(&mut self.pkg, &main);
        self.charts_dirty = false;
        // The theme part is written into the package as soon as it changes.
        self.theme_dirty = false;
    }

    /// Whether `doc` is what the package holds at `path` (both absent counts): an undo or redo that takes a part back to it leaves the
    /// part as the package has it.
    pub(super) fn as_package_holds(&self, path: &str, doc: Option<&XmlDoc>) -> bool {
        // The relationship counts too: an undo puts back the relationships part as it was when its step ran, and a save since can
        // have added or taken out the part it names.
        let linked = self.main_links_to(path);
        match (self.pkg.get(path), doc) {
            (Some(bytes), Some(d)) => linked && xml::parse(bytes).ok().as_ref() == Some(d),
            (None, None) => !linked,
            // A part that does not read as XML is one the text has none of (a damaged part the file came with): it stays.
            (Some(bytes), None) => xml::parse(bytes).is_err(),
            (None, Some(_)) => false,
        }
    }

    /// Whether the package has this part and the main part links to it just when the text has it (a damaged part the file came with,
    /// which the text cannot read, counts as none). Reads no part.
    pub(super) fn part_and_link_agree(&self, path: &str, in_text: bool) -> bool {
        let (linked, there) = (self.main_links_to(path), self.pkg.has(path));
        if in_text {
            linked && there
        } else {
            !linked || (there && self.pkg.get(path).is_some_and(|b| xml::parse(b).is_err()))
        }
    }

    /// Whether the main part has a relationship to this part.
    pub(super) fn main_links_to(&self, path: &str) -> bool {
        self.pkg.rels_for(&self.main_path).is_ok_and(|rels| rels.iter().any(|r| !r.external && package::resolve_target(&self.main_path, &r.target) == path))
    }

    /// A styles, numbering or settings part the text no longer has: an undo took it back to the file, which had none, after a save
    /// wrote one. The part goes, with its relationship and its content type, so that the file says what the text says (it kept the
    /// part the save wrote, and opened again with settings the text did not have).
    /// The relationship and content type go even when the part is already gone (an undo took back the package's own record of it): a
    /// relationship to a part that is not there makes Word repair the file.
    pub(super) fn drop_main_part(&mut self, path: &str) {
        if self.pkg.get(path).is_some_and(|b| xml::parse(b).is_err()) {
            return;
        }
        let main = self.main_path.clone();
        self.pkg.remove_rels_to(&main, path);
        self.pkg.remove_override(path);
        self.pkg.remove(path);
    }

    pub fn is_modified(&self) -> bool {
        self.charts_dirty || self.theme_dirty || self.body_dirty ||self.styles.dirty || self.numbering.dirty || self.settings_dirty || self.core_dirty || self.custom_dirty || self.app_dirty || self.cites.dirty || self.merge.dirty || self.tabs.dirty || !self.dirty_parts.is_empty()
    }

    // -----------------------------------------------------------------------
    // Undo
    // -----------------------------------------------------------------------
}

/// What a save gives: the zip, the parts not compressed yet, or those parts in pieces (see `Document::save_packed_pieces`).
pub(super) enum Saving<'a> {
    Zip,
    Packed,
    Pieces(usize, &'a mut dyn FnMut(&[u8])),
}

pub(super) fn blank_parts() -> Vec<(&'static str, String)> {
    vec![
        ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/><Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#.into()),
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#.into()),
        ("word/_rels/document.xml.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#.into()),
        ("word/theme/theme1.xml", crate::themes::theme_xml(&crate::themes::office_theme())),
        ("word/document.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{NS_W}" xmlns:r="{NS_R}" xmlns:wp="{NS_WP}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p/><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:cols w:space="720"/></w:sectPr></w:body></w:document>"#)),
        ("word/styles.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{NS_W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:asciiTheme="minorHAnsi" w:eastAsiaTheme="minorEastAsia" w:hAnsiTheme="minorHAnsi" w:cstheme="minorBidi"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="en-US"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="numbering" w:default="1" w:styleId="NoList"><w:name w:val="No List"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/></w:style></w:styles>"#)),
        ("word/settings.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="{NS_W}"><w:defaultTabStop w:val="720"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#)),
        ("docProps/core.xml", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title></dc:title><dc:creator>Docxtor</dc:creator></cp:coreProperties>"#.into()),
        ("docProps/app.xml", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Docxtor</Application></Properties>"#.into()),
    ]
}
