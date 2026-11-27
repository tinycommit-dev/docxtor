//! Watermarks: finding the header parts that carry one, putting one in, taking them out.

use super::*;

impl Document {
    /// The watermark in a header part, if it has one.
    pub(super) fn watermark_of(&self, part: &str) -> Option<Watermark> {
        let story = self.stories.iter().find(|s| s.part == part && s.kind == StoryKind::Header)?;
        let pict = marks::first_watermark(&story.blocks)?;
        let rels = self.pkg.rels_for(part).unwrap_or_default();
        marks::read_watermark(pict, &|rid| rels.iter().find(|r| r.id == rid).map(|r| package::resolve_target(part, &r.target)))
    }

    pub(super) fn op_set_watermark(&mut self, text: Option<&str>, layout: WatermarkLayout, color: Option<&str>, font: Option<&str>, opacity: Option<f64>) -> Result<OpResult, EditError> {
        let Some(text) = text else { return self.remove_watermarks() };
        let text = text.trim();
        if text.is_empty() {
            return Err(EditError::Invalid("Give the watermark some text, or remove it.".into()));
        }
        if text.chars().count() > 100 || text.chars().any(char::is_control) {
            return Err(EditError::Invalid("A watermark can be one line of up to 100 characters.".into()));
        }
        let color = match color.map(str::trim).filter(|c| !c.is_empty()) {
            Some(c) => marks::colour_hex(c).ok_or_else(|| EditError::Invalid(format!("'{c}' is not a colour; use six hex digits like C0C0C0")))?,
            None => "C0C0C0".to_string(),
        };
        let opacity = opacity.unwrap_or(0.5);
        if !(0.0..=1.0).contains(&opacity) {
            return Err(EditError::Invalid("Opacity is a number from 0 (clear) to 1 (solid).".into()));
        }
        let font = font
            .map(|f| f.replace(['"', '\\'], "").trim().to_string())
            .filter(|f| !f.is_empty())
            .or_else(|| self.styles.theme.minor.clone())
            .unwrap_or_else(|| "Calibri".into());
        let uses = self.watermark_targets()?;
        let header_style = self.styles.ensure_builtin("Header").unwrap_or_else(|| "Header".into());
        for (serial, (part, area)) in uses.iter().enumerate() {
            let pict = marks::text_pict(&marks::TextMark { text, font: &font, color: &color, opacity, layout, size: marks::watermark_size(text, layout, *area), serial: serial as u32 });
            self.put_watermark_in(part, pict, &header_style, false);
        }
        self.stories_touched = true;
        Ok(OpResult::default())
    }

    /// The header parts every section shows (default, first page and even pages as the section uses them),
    /// each with the text area of the first section that shows it. Missing headers are made.
    pub(super) fn watermark_targets(&mut self) -> Result<Vec<WatermarkTarget>, EditError> {
        // Every section shows a header of each kind it uses (default, first page, even pages); make the missing ones.
        let even = self.settings.as_ref().and_then(|d| d.root.child("w:evenAndOddHeaders")).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")));
        let ends: Vec<Option<NodeId>> = self.section_refs().iter().map(|r| r.0).collect();
        for (i, end) in ends.iter().enumerate() {
            let have: Vec<String> = self.section_headers()[i].headers.keys().cloned().collect();
            let title_page = self.section_refs()[i].1.is_some_and(|s| s.child("w:titlePg").is_some_and(|t| t.attr("w:val") != Some("0")));
            let mut need = vec!["default"];
            if title_page {
                need.push("first");
            }
            if even {
                need.push("even");
            }
            for variant in need {
                if !have.iter().any(|h| h == variant) {
                    self.add_header_footer_variant(HeaderKind::Header, variant, end.map(Id))?;
                }
            }
        }
        // The header parts the sections use, each with the text area of the first section that shows it.
        let mut uses: Vec<WatermarkTarget> = Vec::new();
        for sec in self.section_headers() {
            let area = text_area(sec.sect.as_ref());
            for v in ["default", "first", "even"] {
                if let Some(part) = sec.headers.get(v) {
                    if !uses.iter().any(|(p, _)| p == part) {
                        uses.push((part.clone(), area));
                    }
                }
            }
        }
        Ok(uses)
    }

    /// Replace the watermark of a header part with `pict` (in the paragraph that held the old one).
    pub(super) fn put_watermark_in(&mut self, part: &str, pict: Element, header_style: &str, needs_r: bool) {
        let Some(si) = self.stories.iter().position(|s| s.part == part && s.kind == StoryKind::Header) else { return };
        let (_, near) = marks::remove_watermarks(&mut self.stories[si].blocks);
        let new_id = self.ids.fresh();
        marks::put_watermark(&mut self.stories[si].blocks, near, marks::watermark_run(pict), new_id, header_style);
        if let Some(doc) = self.story_parts.get_mut(part) {
            for (k, uri) in marks::NAMESPACES.into_iter().chain(needs_r.then_some(("xmlns:r", NS_R))) {
                if doc.root.attr(k).is_none() {
                    doc.root.set_attr(k, uri);
                }
            }
        }
        self.story_gen += 1;
        self.stories[si].gen = self.story_gen;
        self.dirty_parts.insert(part.to_string());
        self.watermark_headers.insert(part.to_string());
    }

    pub(super) fn remove_watermarks(&mut self) -> Result<OpResult, EditError> {
        let mut any = false;
        for i in 0..self.stories.len() {
            if self.stories[i].kind == StoryKind::Header && marks::remove_watermarks(&mut self.stories[i].blocks).0 {
                any = true;
                self.story_gen += 1;
                self.stories[i].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[i].part.clone());
                self.watermark_headers.insert(self.stories[i].part.clone());
            }
        }
        if !any {
            return Err(EditError::Invalid("The document has no watermark to remove.".into()));
        }
        self.stories_touched = true;
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Tables of contents
    // -----------------------------------------------------------------------
}

/// A header part a watermark goes in, with the text area (width, height in points) it has to fit.
pub(super) type WatermarkTarget = (String, (f64, f64));
