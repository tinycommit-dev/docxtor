//! Document-wide settings: properties, statistics, language, the page background setting and derived copies.

use super::*;

impl Document {
    /// The document's properties (title, subject, author, keywords, description).
    pub fn doc_props(&self) -> docprops::DocProps {
        let mut p = docprops::read(self.core.as_ref());
        p.company = docmeta::app_text(self.app.as_ref(), "Company");
        p.manager = docmeta::app_text(self.app.as_ref(), "Manager");
        p.custom = docmeta::read_custom(self.custom.as_ref());
        p
    }

    /// The statistics of `docProps/app.xml` as the file has them (page, word and character counts as of the last
    /// save). The live counts are `docmeta::count` over the paragraph texts.
    pub fn app_stat(&self, local: &str) -> Option<String> {
        docmeta::app_text(self.app.as_ref(), local)
    }

    /// The page background colour (hex), if the document sets one.
    pub fn page_background(&self) -> Option<String> {
        self.doc_xml.root.child("w:background").and_then(|b| b.attr("w:color")).and_then(crate::colour::hex)
    }

    /// The document's default language (`en-US`), from the style defaults.
    pub fn language(&self) -> Option<String> {
        self.styles.doc.as_ref().and_then(|d| d.root.path(&["w:docDefaults", "w:rPrDefault", "w:rPr", "w:lang"])).and_then(|l| l.attr("w:val")).map(String::from)
    }

    pub(super) fn op_set_language(&mut self, lang: &str, ids: Option<&[Id]>) -> Result<OpResult, EditError> {
        let lang = docprops::normalize_lang(lang).map_err(EditError::Invalid)?;
        match ids {
            Some(ids) => {
                for id in ids {
                    self.touch(id.0);
                    let p = self.para_mut(id.0)?;
                    docprops::set_run_lang(&mut p.inlines, &lang);
                    // The paragraph mark's own run properties, so text typed at the end gets the language too.
                    if let Some(rpr) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:rPr")) {
                        let mut el = rpr.child("w:lang").cloned().unwrap_or_else(|| Element::new("w:lang"));
                        el.set_attr("w:val", &lang);
                        crate::props::put_rpr(rpr, el);
                    }
                }
            }
            None => {
                let styles = self.styles.doc.get_or_insert_with(|| XmlDoc::new(Element::new("w:styles").with_attr("xmlns:w", NS_W)));
                let before = styles.clone();
                docprops::set_default_lang(styles, &lang);
                if *styles != before {
                    self.styles.dirty = true;
                }
                self.settings_edit(false, &mut |r| docprops::set_theme_font_lang(r, &lang));
                // Paragraphs without a language of their own take the new default: draw them again.
                self.redraw_all();
            }
        }
        Ok(OpResult::default())
    }

    /// A copy of this document whose body is `body` (not the undo history), for producing derived files
    /// such as a redline without touching the document itself.
    pub(crate) fn fork_with_body(&self, body: Vec<Block>) -> Document {
        let body = Body::from(body);
        Document {
            pkg: self.pkg.clone(),
            main_path: self.main_path.clone(),
            doc_xml: self.doc_xml.clone(),
            body,
            styles: self.styles.clone(),
            numbering: self.numbering.clone(),
            styles_path: self.styles_path.clone(),
            numbering_path: self.numbering_path.clone(),
            settings: self.settings.clone(),
            settings_path: self.settings_path.clone(),
            settings_dirty: self.settings_dirty,
            core: self.core.clone(),
            core_path: self.core_path.clone(),
            core_dirty: self.core_dirty,
            custom: self.custom.clone(),
            custom_path: self.custom_path.clone(),
            custom_dirty: self.custom_dirty,
            app: self.app.clone(),
            app_path: self.app_path.clone(),
            app_dirty: self.app_dirty,
            save_info: self.save_info.clone(),
            user_groups: self.user_groups.clone(),
            cites: self.cites.clone(),
            merge: self.merge.clone(),
            merge_preview: None,
            access: self.access,
            tabs: self.tabs.clone(),
            charts_dirty: false,
            ids: self.ids.clone(),
            body_dirty: true,
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
            drawing_id: self.drawing_id,
            stories: self.stories.clone(),
            story_parts: self.story_parts.clone(),
            dirty_parts: self.dirty_parts.clone(),
            watermark_headers: self.watermark_headers.clone(),
            fresh: self.fresh,
            story_gen: self.story_gen,
            hf_candidates: self.hf_candidates.clone(),
            baseline_orphans: self.baseline_orphans.clone(),
            parked: None,
            restyled_main: false,
            active_part: self.main_path.clone(),
            stories_touched: false,
            touched_stories: Vec::new(),
            last_notes: HashMap::new(),
            csl_styles: self.csl_styles.clone(),
            scrub: self.scrub.clone(),
            theme: self.theme.clone(),
            shape_theme: self.shape_theme.clone(),
            theme_dirty: self.theme_dirty,
            alt_chunks: self.alt_chunks.clone(),
            alt_dirty: Vec::new(),
            alt_stamp: None,
            inv: Default::default(),
        }
    }
}
