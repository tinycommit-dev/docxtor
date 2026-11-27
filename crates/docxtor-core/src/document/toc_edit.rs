//! Inserting and updating a table of contents: heading levels, page estimates and the TOC styles.

use super::*;

impl Document {
    /// Outline level of a paragraph as a 1-based heading level (direct, then from its style).
    pub(super) fn heading_level(&self, p: &Paragraph) -> Option<u8> {
        let direct = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
        let style = direct.style.clone();
        let lvl = direct.outline_lvl.or_else(|| self.styles.para_style(style.as_deref()).0.outline_lvl).or_else(|| {
            let n: i64 = style.as_deref()?.strip_prefix("Heading")?.parse().ok()?;
            Some(n - 1)
        })?;
        (0..9).contains(&lvl).then_some(lvl as u8 + 1)
    }

    /// Page of each paragraph, counting explicit page breaks (for when no layout measured them).
    pub(super) fn page_estimates(&self) -> HashMap<NodeId, u32> {
        let mut out = HashMap::new();
        let mut page = 1u32;
        let breaks = self.section_breaks();
        walk_paragraphs(&self.body, &mut |p| {
            let fmt = p.ppr.as_ref().map(ParaFmt::from_ppr).unwrap_or_default();
            if fmt.page_break_before == Some(true) {
                page += 1;
            }
            out.insert(p.id, page);
            let pbreaks = p.inlines.iter().map(inline_page_breaks).sum::<u32>();
            let sect = breaks.get(&p.id).is_some_and(|k| matches!(k.as_str(), "nextPage" | "evenPage" | "oddPage"));
            page += pbreaks + u32::from(sect);
        });
        out
    }

    pub(super) fn toc_page_fn<'a>(&self, pages: Option<&'a HashMap<String, u32>>) -> impl Fn(&toc::Entry) -> String + 'a {
        let est = if pages.is_none() { self.page_estimates() } else { HashMap::new() };
        // A section that numbers its pages i, ii, iii shows that here too.
        let labels = self.page_labels();
        move |e: &toc::Entry| match pages {
            Some(m) => m.get(&e.para.to_string()).map(|n| labels.label(e.para, *n)).unwrap_or_default(),
            None => est.get(&e.para).map(|n| labels.label(e.para, *n)).unwrap_or_default(),
        }
    }

    /// The ids of the styles for the table of contents levels 1 to 9: Word's `TOC1`, or the id the file defines for
    /// the style named "toc 1" (a localised Word does).
    pub(super) fn toc_style_ids(&self) -> Vec<String> {
        (1..=9).map(|n| self.styles.lookup(&format!("TOC{n}")).unwrap_or_else(|| format!("TOC{n}"))).collect()
    }

    pub(super) fn ensure_toc_styles(&mut self, levels: u8) {
        for n in 1..=levels.clamp(1, 9) {
            self.styles.ensure_builtin(&format!("TOC{n}"));
        }
    }

    pub(super) fn op_insert_toc(&mut self, after: Option<NodeId>, before: Option<NodeId>, levels: u8, options: Option<&crate::toc_spec::TocOptions>, title: Option<&str>, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let options = options.cloned().unwrap_or_else(|| crate::toc_spec::TocOptions { levels: levels.clamp(1, 9), ..Default::default() });
        self.op_insert_toc_with(after, before, &options, title, pages)
    }

    /// Rebuild every table of contents and table of figures from the headings and captions (see `updating`).
    /// With `pages_only`, only the page numbers change.
    pub(super) fn op_update_toc(&mut self, pages: Option<&HashMap<String, u32>>, pages_only: bool) -> Result<OpResult, EditError> {
        if pages_only {
            return self.op_update_toc_pages(pages);
        }
        let (created, count) = self.update_block_fields(pages, updating::BlockKinds { toc: true, tof: true, index: false }, None);
        if count == 0 {
            return Err(EditError::Invalid("the document has no table of contents".into()));
        }
        Ok(OpResult { created, count: Some(count), ..Default::default() })
    }

    pub(super) fn mark_rpr(p: &Paragraph) -> Option<Element> {
        p.ppr.as_ref().and_then(|pp| pp.child("w:rPr")).map(|r| {
            let mut r = r.clone();
            r.remove_children("w:ins");
            r.remove_children("w:del");
            r.remove_children("w:moveFrom");
            r.remove_children("w:moveTo");
            r.remove_children("w:rPrChange");
            r
        })
    }
}
