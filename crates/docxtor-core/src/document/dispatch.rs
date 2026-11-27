//! Applying ops: `apply`, the batch with its checks and undo step, JSON entry points, and the match that sends each op to its method.

use super::*;

impl Document {
    /// Apply one op. With `merge`, it joins the previous undo step (e.g. continued typing).
    pub fn apply(&mut self, op: &Op, merge: bool) -> Result<OpResult, EditError> {
        self.apply_batch(std::slice::from_ref(op), merge).map(|mut v| v.pop().unwrap_or_default())
    }

    /// Apply ops atomically as one undo step: on error nothing changes. (`apply_batch`, in `alt_chunks`, wraps this
    /// for documents with imported chunks.)
    pub(super) fn apply_batch_core(&mut self, ops: &[Op], merge: bool) -> Result<Vec<OpResult>, EditError> {
        if self.merge_preview.is_some() && !ops.is_empty() {
            return Err(EditError::Invalid("A merged record is shown. Stop previewing the results to change the document.".into()));
        }
        self.check_access(ops)?;
        let chart_ops = ops.iter().any(|o| o.class().touches_charts);
        // Before the first change that could leave a picture or chart unused, note which ones the file never used.
        if !ops.iter().all(|o| matches!(o, Op::UpdateFields { open: true, .. })) {
            self.ensure_media_baseline();
        }
        // Protection and locked controls: what they cover is compared before and after (see `document/protect.rs`).
        self.check_formatting_allowed(ops)?;
        self.check_layout_allowed(ops)?;
        // What the steps after the batch compare with (see `document/after_batch.rs`).
        let watch = self.batch_watch(ops);
        crate::perf::plant_edit();
        #[cfg(any(test, debug_assertions))]
        {
            self.snap_op = ops.first().map(|o| o.name().to_string()).unwrap_or_default();
        }
        let mut before = self.snapshot();
        let marks = self.change_marks();
        // Relationships and parts an op adds to the package are recorded, so a refused batch or an undo gives them back.
        self.pkg.begin_journal();
        let mut results = Vec::with_capacity(ops.len());
        let mut body_touched = false;
        for (i, op) in ops.iter().enumerate() {
            // Editing a citation inside a note runs on the whole document, so the other citations follow it.
            let story = op.anchor().filter(|id| find_block(&self.body, *id).is_none() && !op.class().whole_document).and_then(|id| self.story_index(id));
            if story.is_some() && watch.mode().is_some() {
                self.refuse_batch(before, marks);
                return Err(self.protection_error());
            }
            body_touched |= story.is_none() && !op.leaves_body_alone();
            let r = match story {
                Some(si) => self.in_story(si, op),
                // Replacing with no paragraphs named goes through the text and the stories.
                None if matches!(op, Op::ReplaceText { ids: None, .. }) => self.replace_everywhere(op),
                None => self.apply_inner(op),
            };
            match r {
                Ok(r) => results.push(r),
                Err(e) => {
                    self.refuse_batch(before, marks);
                    let msg = if ops.len() > 1 { format!("op #{i}: {e}") } else { e.to_string() };
                    return Err(EditError::Invalid(msg));
                }
            }
        }
        if chart_ops {
            self.charts_dirty = true;
        }
        // An op that by its kind leaves the body alone can still change blocks in it: updating fields, a bound control
        // showing a new property, a bibliography built again. What the batch marked decides, so the steps after it
        // (a placeholder typed over, style ids, cells ending in a paragraph) run on those blocks too.
        if !body_touched && !self.changed.since_copy.is_empty() {
            body_touched = self.body.iter().any(|b| self.changed.since_copy.contains(&b.id()));
        }
        self.keep_closing_sect_last();
        // Protection, locks, tabs and chunks are checked, and controls, text boxes and connectors put right, in the
        // body and in every story the batch touched.
        if let Err(e) = self.after_batch(watch, &before, body_touched) {
            self.refuse_batch(before, marks);
            return Err(e);
        }
        // Nothing is recorded for undo when the batch only changes what nobody wrote: updating as the file opens, opening
        // a tab, or anything a read-only document lets through. Updating fields before printing is one undoable step in a
        // document that can be edited, and none when nothing changed.
        let printing_update = |o: &Op| matches!(o, Op::UpdateFields { quiet: true, open: false, .. });
        let skip_undo = !ops.is_empty()
            && (self.access == Access::ViewOnly
                || ops.iter().all(|o| o.is_view_only() && !printing_update(o))
                || ops.iter().zip(&results).all(|(o, r)| printing_update(o) && r.count == Some(0)));
        let pkg_delta = self.pkg.end_journal();
        if skip_undo {
            // Nothing to record.
        } else if merge && !self.undo.is_empty() {
            // Continued typing joins the last step: what that step recorded is older, so it wins.
            if let Some(last) = self.undo.last_mut() {
                for d in pkg_delta {
                    if !last.pkg_delta.iter().any(|(n, _)| *n == d.0) {
                        last.pkg_delta.push(d);
                    }
                }
            }
        } else {
            before.pkg_delta = pkg_delta;
            self.undo.push(before.weigh());
            self.undo_labels.resize(self.undo.len() - 1, String::new());
            self.undo_labels
                .push(ops.first().map(|o| o.name().to_string()).unwrap_or_default());
            self.trim_undo();
        }
        if !skip_undo {
            self.redo.clear();
            self.redo_labels.clear();
        }
        self.body_dirty |= body_touched;
        // A batch that changed nothing (no ops, the field update at open, a quiet update that found nothing) is no change.
        if !ops.is_empty() && !skip_undo {
            self.revision += 1;
        }
        // Any batch may have changed what was kept of the stories (a quiet one too).
        if !ops.is_empty() {
            self.generation += 1;
        }
        Ok(results)
    }

    pub fn apply_json(&mut self, json: &str, merge: bool) -> Result<Vec<OpResult>, EditError> {
        self.apply_json_inner(json, merge).map_err(EditError::shortened)
    }

    pub(super) fn apply_json_inner(&mut self, json: &str, merge: bool) -> Result<Vec<OpResult>, EditError> {
        // Ops from an assistant or an extension are input from outside too.
        crate::budget::within(json.len(), crate::budget::caps::OPS_BYTES).map_err(|e| EditError::Invalid(e.to_string()))?;
        let v: serde_json::Value = serde_json::from_str(json).map_err(|e| EditError::Json(e.to_string()))?;
        let ops: Vec<Op> = if v.is_array() {
            serde_json::from_value(v).map_err(|e| EditError::Json(e.to_string()))?
        } else {
            vec![serde_json::from_value(v).map_err(|e| EditError::Json(e.to_string()))?]
        };
        self.apply_batch(&ops, merge)
    }

    // -----------------------------------------------------------------------
    // Stories (headers, footers, footnotes)
    // -----------------------------------------------------------------------

    pub(super) fn apply_inner(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::InsertText { at, text, format } => self.op_insert_text(*at, text, format.as_ref()),
            Op::Delete { from, to } => self.op_delete(*from, *to),
            Op::SplitParagraph { at } => self.op_split(*at),
            Op::FormatText { from, to, format } => self.op_format_text(*from, *to, format),
            Op::FormatParagraphs { ids, format } => {
                let mut patch = format.clone();
                if let Some(s) = &patch.style {
                    if !s.is_empty() {
                        patch.style = Some(self.resolve_style(s)?);
                    }
                }
                for id in ids {
                    self.touch(id.0);
                    // What the paragraph inherits, so switching a setting off can override a style that turns it on.
                    let style = patch.style.clone().or_else(|| self.para_style_of(id.0)).filter(|s| !s.is_empty());
                    let inherited = self.styles.doc_ppr.overlay(&self.styles.para_style(style.as_deref()).0);
                    let p = self.para_mut(id.0)?;
                    patch.apply_with(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), &inherited);
                }
                Ok(OpResult::default())
            }
            Op::SetStyle { ids, style } => {
                let sid = if style.is_empty() { String::new() } else { self.resolve_style(style)? };
                for id in ids {
                    self.touch(id.0);
                    let p = self.para_mut(id.0)?;
                    let ppr = p.ppr.get_or_insert_with(|| Element::new("w:pPr"));
                    ParaPatch { style: Some(sid.clone()), ..Default::default() }.apply(ppr);
                }
                Ok(OpResult::default())
            }
            Op::ToggleList { ids, kind } => self.op_toggle_list(ids, *kind),
            Op::SetListStyle { ids, style } => self.op_set_list_style(ids, style),
            // Styles, themes and style sets: see `design`.
            Op::SetStyleDefinition { style, def } => self.op_set_style_definition(style.as_deref(), def),
            Op::CreateStyle { name, kind, from, to, def } => self.op_create_style(name, kind.as_deref(), *from, *to, def.as_ref()),
            Op::UpdateStyle { style, from, to } => self.op_update_style(style, *from, *to),
            Op::DeleteStyle { style } => self.op_delete_style(style),
            Op::SetTheme { theme, definition, colors, fonts } => self.op_set_theme(theme.as_deref(), definition.as_ref(), colors.as_ref(), fonts.as_ref()),
            Op::SetStyleSet { style_set, definition } => self.op_set_style_set(style_set.as_deref(), definition.as_ref()),
            Op::SetParagraphSpacing { preset, spacing } => self.op_set_paragraph_spacing(preset.as_deref(), spacing.as_ref()),
            Op::RestartNumbering { id, start } => self.op_restart_numbering(id.0, start.unwrap_or(1)),
            Op::ContinueNumbering { id } => self.op_continue_numbering(id.0),
            Op::DefineList { def, ids } => self.op_define_list(def, ids),
            Op::ApplyList { ids, num_id } => self.op_apply_list(ids, *num_id),
            Op::SetNumberingValue { id, value, continue_previous } => self.op_set_numbering_value(id.0, *value, *continue_previous),
            Op::SortParagraphs { ids, options } => self.op_sort_paragraphs(ids, options),
            Op::SetChecked { ids, checked } => self.op_set_checked(ids, *checked),
            Op::SetOutlineLevel { ids, level, delta } => self.op_set_outline_level(ids, *level, *delta),
            Op::MoveBlocks { ids, after, before } => self.op_move_blocks(ids, *after, *before),
            Op::Indent { ids, delta } => self.op_indent(ids, *delta),
            Op::InsertParagraph { after, before, text, style } => {
                let id = self.ids.fresh();
                let mut p = Paragraph::new(id);
                if let Some(s) = style {
                    let sid = self.resolve_style(s)?;
                    let mut ppr = Element::new("w:pPr");
                    put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", sid));
                    p.ppr = Some(ppr);
                }
                if !text.is_empty() {
                    edit::insert_text(&mut p.inlines, 0, text, None);
                }
                let len = p.len16();
                self.insert_blocks(after.map(|i| i.0), before.map(|i| i.0), vec![Block::Para(p)])?;
                Ok(OpResult { caret: Some(Pos::new(id, len)), created: vec![id], count: None })
            }
            Op::InsertMarkdown { after, before, markdown } => self.op_insert_markdown(after.map(|i| i.0), before.map(|i| i.0), markdown),
            Op::SetParagraphText { id, markdown } => self.op_set_paragraph_text(id.0, markdown),
            Op::PasteMarkdown { at, markdown } => self.op_paste(*at, markdown),
            Op::InsertFragment { at, fragment, mode } => self.op_insert_fragment(*at, fragment, *mode),
            Op::PasteHtml { at, html, mode } => {
                let fragment = crate::html_import::html_to_fragment(html, &Default::default());
                self.op_insert_fragment(*at, &fragment, *mode)
            }
            Op::ReplaceText { find, replace, ids, all, options, format, para_format, language } => {
                let r = self.replace_in_body(find, replace, ids.as_deref(), *all, options, format.as_ref(), para_format.as_ref(), language.as_deref())?;
                if r.count == Some(0) {
                    return Err(EditError::Invalid(format!("text not found: {find:?}")));
                }
                Ok(r)
            }
            Op::DeleteBlocks { ids } => {
                for id in ids {
                    self.touch(id.0);
                    let (mut list, i) = self.body.parent_mut(id.0).ok_or(EditError::NotFound(id.0))?;
                    list.remove(i);
                    if list.is_empty() || !list.iter().any(|b| matches!(b, Block::Para(_) | Block::Table(_))) {
                        let pos = list.iter().position(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).unwrap_or(list.len());
                        let nid = self.ids.fresh();
                        // `list` borrow ended above; re-find is unnecessary since we hold it.
                        list.insert(pos, Block::Para(Paragraph::new(nid)));
                        self.changed.insert(nid);
                    }
                }
                Ok(OpResult::default())
            }
            Op::InsertTable { after, rows, cols, data, autofit, column_width } => {
                let rows = (*rows).clamp(1, 200);
                let cols = (*cols).clamp(1, 50);
                let cells: Vec<Vec<Vec<Inline>>> = (0..rows)
                    .map(|r| {
                        (0..cols)
                            .map(|c| {
                                let mut v = Vec::new();
                                if let Some(t) = data.get(r).and_then(|row| row.get(c)) {
                                    edit::insert_text(&mut v, 0, t, None);
                                }
                                v
                            })
                            .collect()
                    })
                    .collect();
                let mut t = self.build_table(cells, false)?;
                self.size_new_table(&mut t, autofit.as_deref(), *column_width)?;
                let first = first_para_in(&t);
                let tid = t.id;
                self.insert_blocks(Some(after.0), None, vec![Block::Table(t)])?;
                self.ensure_para_after(tid);
                Ok(OpResult { caret: first.map(|p| Pos::new(p, 0)), created: vec![tid], count: None })
            }
            Op::TableEdit { at, action } => self.op_table(at.0, *action),
            Op::TableCells { from, to, action } => self.op_table_cells(from.0, to.0, action),
            Op::TableFormat { at, format } => self.op_table_format(at.0, format),
            Op::TableRows { from, to, all, format } => self.op_table_rows(from.0, to.map(|i| i.0), *all, format),
            Op::TableColumn { at, col, width } => self.op_table_column(at.0, *col, *width),
            Op::SplitTable { at } => self.op_split_table(at.0),
            Op::TableCell { at, action, shift } => self.op_table_cell(at.0, *action, *shift),
            Op::SortTable { from, to, options } => self.op_sort_table(from.0, to.map(|i| i.0), options),
            Op::ConvertToTable { ids, separator, separator_char, columns, autofit } => self.op_convert_to_table(ids, *separator, *separator_char, *columns, autofit.as_deref()),
            Op::ConvertToText { at, separator, separator_char } => self.op_convert_to_text(at.0, *separator, *separator_char),
            Op::TableProps { at, widths, borders, header } => {
                use crate::table;
                self.touch(at.0);
                with_table_body(&mut self.body, at.0, &mut |t: &mut Table, _, _| {
                    if let Some(w) = widths {
                        table::set_widths(t, w);
                    }
                    if let Some(b) = borders {
                        table::set_borders(t, *b);
                    }
                    if let Some(h) = header {
                        table::set_header_row(t, *h);
                    }
                })
                .ok_or_else(|| EditError::Invalid(format!("paragraph {} is not inside a table", at.0)))?;
                Ok(OpResult::default())
            }
            Op::TableDesign { at, style, definition, look } => self.op_table_design(at.0, style.as_deref(), definition.as_deref(), look.as_ref()),
            Op::Control(c) => self.op_control(c),
            Op::FormField(f) => self.op_form(f),
            Op::Protection(p) => self.op_protection(p),
            Op::EastAsian(e) => self.op_east_asian(e),
            Op::Tab(t) => self.op_tab(t),
            Op::Toa(t) => self.op_toa(t),
            Op::SetLink { from, to, url } => self.op_set_link(*from, *to, url.as_deref()),
            Op::SetPageSetup { section, width, height, margins } => {
                self.edit_sects(*section, &mut |sect| {
                    if width.is_some() || height.is_some() {
                        let mut sz = sect.child("w:pgSz").cloned().unwrap_or_else(|| Element::new("w:pgSz"));
                        let w = width.or_else(|| sz.attr("w:w").and_then(|v| v.parse().ok())).unwrap_or(12240);
                        let h = height.or_else(|| sz.attr("w:h").and_then(|v| v.parse().ok())).unwrap_or(15840);
                        sz.set_attr("w:w", w.to_string());
                        sz.set_attr("w:h", h.to_string());
                        if w > h {
                            sz.set_attr("w:orient", "landscape");
                        } else {
                            sz.remove_attr("w:orient");
                        }
                        crate::props::put_ordered(sect, sz, SECT_ORDER);
                    }
                    if let Some([t, r, b, l]) = margins {
                        let mut mar = sect.child("w:pgMar").cloned().unwrap_or_else(|| {
                            Element::new("w:pgMar").with_attr("w:header", "720").with_attr("w:footer", "720").with_attr("w:gutter", "0")
                        });
                        for (k, v) in [("w:top", *t), ("w:right", *r), ("w:bottom", *b), ("w:left", *l)] {
                            mar.set_attr(k, v.max(0).to_string());
                        }
                        crate::props::put_ordered(sect, mar, SECT_ORDER);
                    }
                })?;
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::SetColumns { section, count, space, sep, widths, spaces } => {
                if !(1..=6).contains(count) {
                    return Err(EditError::Invalid("columns must be between 1 and 6".into()));
                }
                if let Some(w) = widths {
                    if w.len() != usize::from(*count) || w.iter().any(|v| *v < 360) {
                        return Err(EditError::Invalid("Give one width for each column, each at least a quarter of an inch (360 twips).".into()));
                    }
                }
                self.edit_sects(*section, &mut |sect| {
                    let mut cols = sect.child("w:cols").cloned().unwrap_or_else(|| Element::new("w:cols"));
                    if *count == 1 {
                        cols.remove_attr("w:num");
                    } else {
                        cols.set_attr("w:num", count.to_string());
                    }
                    match (space, cols.attr("w:space")) {
                        (Some(v), _) => cols.set_attr("w:space", (*v).max(0).to_string()),
                        (None, None) => cols.set_attr("w:space", "720"),
                        _ => {}
                    }
                    match sep {
                        Some(true) => cols.set_attr("w:sep", "1"),
                        Some(false) => {
                            cols.remove_attr("w:sep");
                        }
                        None => {}
                    }
                    cols.remove_attr("w:equalWidth");
                    cols.remove_children("w:col");
                    // Unequal columns: each `w:col` has its width and the space after it.
                    if let Some(w) = widths.as_ref().filter(|w| w.len() > 1) {
                        cols.set_attr("w:equalWidth", "0");
                        let gap = space.or_else(|| cols.attr("w:space").and_then(|v| v.parse().ok())).unwrap_or(720);
                        for (i, width) in w.iter().enumerate() {
                            let mut col = Element::new("w:col").with_attr("w:w", width.to_string());
                            if i + 1 < w.len() {
                                col.set_attr("w:space", spaces.as_ref().and_then(|s| s.get(i)).copied().unwrap_or(gap).max(0).to_string());
                            }
                            cols.children.push(Node::Element(col));
                        }
                    }
                    crate::props::put_ordered(sect, cols, SECT_ORDER);
                })?;
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::SetLineNumbers { section, on, count_by, restart } => {
                if let Some(r) = restart {
                    if !matches!(r.as_str(), "newPage" | "newSection" | "continuous") {
                        return Err(EditError::Invalid(format!("unknown line number restart '{r}'; use newPage, newSection or continuous")));
                    }
                }
                self.edit_sects(*section, &mut |sect| {
                    if !*on {
                        sect.remove_children("w:lnNumType");
                        return;
                    }
                    let mut ln = sect.child("w:lnNumType").cloned().unwrap_or_else(|| Element::new("w:lnNumType"));
                    let by = count_by.or_else(|| ln.attr("w:countBy").and_then(|v| v.parse().ok())).unwrap_or(1).max(1);
                    ln.set_attr("w:countBy", by.to_string());
                    match restart.as_deref() {
                        Some("newPage") => {
                            ln.remove_attr("w:restart");
                        }
                        Some(r) => ln.set_attr("w:restart", r),
                        None => {}
                    }
                    crate::props::put_ordered(sect, ln, SECT_ORDER);
                })?;
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::SetPageColor { color } => {
                match color {
                    Some(c) => {
                        let hex = c.trim().trim_start_matches('#').to_ascii_uppercase();
                        if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                            return Err(EditError::Invalid(format!("'{c}' is not a colour; use six hex digits like FFF2CC")));
                        }
                        self.set_background(Some(Element::new("w:background").with_attr("w:color", hex)));
                        // Word only shows the page colour when the document asks for it.
                        self.settings_edit(true, &mut |r| crate::props::put_ordered(r, Element::new("w:displayBackgroundShape"), SETTINGS_ORDER));
                    }
                    None => {
                        self.set_background(None);
                        self.settings_edit(false, &mut |r| {
                            r.remove_children("w:displayBackgroundShape");
                        });
                    }
                }
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::SetDefaultTabStop { twips } => {
                if *twips < 20 || *twips > 31680 {
                    return Err(EditError::Invalid("the default tab stop must be between 20 and 31680 twips".into()));
                }
                let v = twips.to_string();
                self.settings_edit(true, &mut |r| crate::props::put_ordered(r, Element::new("w:defaultTabStop").with_attr("w:val", &v), SETTINGS_ORDER));
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::SetHyphenation { on } => {
                if *on {
                    self.settings_edit(true, &mut |r| crate::props::put_ordered(r, Element::new("w:autoHyphenation"), SETTINGS_ORDER));
                } else {
                    self.settings_edit(false, &mut |r| {
                        r.remove_children("w:autoHyphenation");
                    });
                }
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::SetImage { at, patch } => {
                self.ensure_ns("wp", NS_WP);
                self.ensure_ns("a", "http://schemas.openxmlformats.org/drawingml/2006/main");
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                let el = edit::object_at_mut(&mut p.inlines, at.o).ok_or_else(|| EditError::Invalid("no image at that position".into()))?;
                if el.find("a:blip").is_none() && !crate::chart::is_chart_drawing(el) && !crate::chart::chartex::is_chartex_drawing(el) {
                    return Err(EditError::Invalid("that object isn't a picture or a chart".into()));
                }
                crate::image::apply(el, patch).map_err(EditError::Invalid)?;
                Ok(OpResult { caret: Some(*at), ..Default::default() })
            }
            Op::InsertPageBreak { at } => self.op_inline_break(*at, "page"),
            Op::InsertBreak { at, kind } => match kind.section_type() {
                None => self.op_inline_break(*at, if *kind == BreakKind::Column { "column" } else { "page" }),
                Some(ty) => self.op_section_break(*at, ty),
            },
            Op::InsertImage { at, media, width, height, alt } => {
                if !self.pkg.has(media) {
                    return Err(EditError::Invalid(format!("media '{media}' not found; add it first")));
                }
                let target = relative_target(&self.active_part, media);
                let main = self.active_part.clone();
                let rid = self.pkg.add_rel(&main, package::REL_IMAGE, &target, false);
                self.ensure_ns("r", NS_R);
                self.ensure_ns("wp", NS_WP);
                self.drawing_id += 1;
                let el = drawing_xml(&rid, *width, *height, self.drawing_id, alt.as_deref().unwrap_or(""));
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                let off = at.o.min(p.len16());
                let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
                edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
                Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
            }
            Op::InsertChart { .. } => self.op_insert_chart(op),
            Op::SetChart { .. } => self.op_set_chart(op),
            Op::AddHeaderFooter { kind, variant, section } => self.op_add_header_footer(*kind, variant, *section),
            Op::LinkHeaderFooter { kind, variant, section, linked } => self.op_link_header_footer(*kind, variant, *section, *linked),
            Op::RemoveHeaderFooter { kind, variant, section } => {
                let tag = if *kind == HeaderKind::Header { "w:headerReference" } else { "w:footerReference" };
                let mut removed = false;
                self.with_sect(*section, &mut |s| {
                    let before = s.children.len();
                    s.children.retain(|n| !matches!(n, Node::Element(e) if e.name == tag && e.attr("w:type").unwrap_or("default") == variant));
                    removed |= s.children.len() != before;
                })?;
                if !removed {
                    return Err(EditError::Invalid(format!("there is no {variant} {} to remove", if *kind == HeaderKind::Header { "header" } else { "footer" })));
                }
                self.stories_touched = true;
                Ok(OpResult::default())
            }
            Op::SetTitlePage { on, section } => {
                self.with_sect(*section, &mut |s| {
                    if *on {
                        crate::props::put_ordered(s, Element::new("w:titlePg"), SECT_ORDER);
                    } else {
                        s.remove_children("w:titlePg");
                    }
                })?;
                self.stories_touched = true;
                Ok(OpResult::default())
            }
            Op::SetPageNumbering { section, start, format, chap_style, chap_sep } => {
                if let Some(c) = chap_style {
                    if !(0..=9).contains(c) {
                        return Err(EditError::Invalid("The chapter heading level is 1 to 9; 0 removes chapter numbers.".into()));
                    }
                }
                if let Some(sep) = chap_sep {
                    if !matches!(sep.as_str(), "hyphen" | "period" | "colon" | "emDash" | "enDash") {
                        return Err(EditError::Invalid(format!("The chapter separator is hyphen, period, colon, emDash or enDash, not '{sep}'.")));
                    }
                }
                if let Some(f) = format {
                    if !matches!(f.as_str(), "decimal" | "lowerRoman" | "upperRoman" | "lowerLetter" | "upperLetter") {
                        return Err(EditError::Invalid(format!("unknown page number format '{f}'; use decimal, lowerRoman, upperRoman, lowerLetter or upperLetter")));
                    }
                }
                self.edit_sects(*section, &mut |s| {
                    let mut n = s.child("w:pgNumType").cloned().unwrap_or_else(|| Element::new("w:pgNumType"));
                    match start {
                        Some(v) => n.set_attr("w:start", (*v).max(0).to_string()),
                        None => {
                            n.remove_attr("w:start");
                        }
                    }
                    match format.as_deref() {
                        Some("decimal") => {
                            n.remove_attr("w:fmt");
                        }
                        Some(f) => n.set_attr("w:fmt", f),
                        None => {}
                    }
                    match chap_style {
                        Some(0) => {
                            n.remove_attr("w:chapStyle");
                            n.remove_attr("w:chapSep");
                        }
                        Some(c) => n.set_attr("w:chapStyle", c.to_string()),
                        None => {}
                    }
                    if let (Some(sep), true) = (chap_sep, n.attr("w:chapStyle").is_some()) {
                        if sep == "hyphen" {
                            n.remove_attr("w:chapSep");
                        } else {
                            n.set_attr("w:chapSep", sep);
                        }
                    }
                    if n.attrs.is_empty() && n.children.is_empty() {
                        s.remove_children("w:pgNumType");
                    } else {
                        crate::props::put_ordered(s, n, SECT_ORDER);
                    }
                })?;
                self.redraw_all();
                Ok(OpResult::default())
            }
            Op::InsertField { at, field, result, input } => self.op_insert_field(*at, field, result.as_deref(), input.as_ref()),
            Op::InsertFootnote { at, text } => self.op_insert_footnote(*at, text),
            Op::InsertEquation { at, latex, display } => self.op_insert_equation(*at, latex, *display),
            Op::SetEquation { at, latex, display } => self.op_set_equation(*at, latex, *display),
            Op::ReplaceFont { from, to } => self.op_replace_font(from, to),
            Op::ChangeCase { from, to, mode } => self.op_change_case(*from, *to, *mode),
            Op::SetDefaultFormat { format } => self.op_set_default_format(format),
            Op::InsertToc { after, before, levels, title, pages, options } => self.op_insert_toc(after.map(|i| i.0), before.map(|i| i.0), *levels, options.as_ref(), title.as_deref(), pages.as_ref()),
            Op::UpdateToc { pages, pages_only } => self.op_update_toc(pages.as_ref(), *pages_only),
            Op::SetToc { at, options, pages } => self.op_set_toc(at.map(|i| i.0), options, pages.as_ref()),
            Op::RemoveToc { at } => self.op_remove_toc(at.map(|i| i.0)),
            Op::ExpandAltChunk { id } => self.op_expand_alt_chunk(id.0),
            Op::InsertTocEntry { at, text, level, id, no_page } => self.op_insert_toc_entry(*at, text, *level, id.as_deref(), *no_page),
            Op::InsertBookmark { from, to, name } => self.op_insert_bookmark(*from, *to, name),
            Op::DeleteBookmark { name } => self.op_delete_bookmark(name),
            Op::RenameBookmark { from, to } => self.op_rename_bookmark(from, to),
            Op::InsertHorizontalLine { at } => self.op_insert_horizontal_line(*at),
            Op::SetWatermark {
                text,
                layout,
                color,
                font,
                opacity,
            } => self.op_set_watermark(
                text.as_deref(),
                *layout,
                color.as_deref(),
                font.as_deref(),
                *opacity,
            ),
            Op::SetPictureWatermark {
                media,
                width,
                height,
                scale,
                washout,
            } => self.op_set_picture_watermark(media, *width, *height, *scale, *washout),
            Op::SetDocProps {
                title,
                subject,
                author,
                keywords,
                description,
                category,
                status,
                company,
                manager,
            } => {
                let core = self.core.get_or_insert_with(docprops::new_core_doc);
                let before = core.clone();
                fn o(v: &Option<String>) -> Option<&str> {
                    v.as_deref()
                }
                let changes = docprops::CoreChanges {
                    title: o(title),
                    subject: o(subject),
                    author: o(author),
                    keywords: o(keywords),
                    description: o(description),
                    category: o(category),
                    status: o(status),
                    exact: false,
                };
                docprops::apply(core, &changes);
                if *core != before || self.pkg.get(&self.core_path).is_none() {
                    self.core_dirty = true;
                }
                if company.is_some() || manager.is_some() {
                    let app = self.app.get_or_insert_with(docmeta::new_app_doc);
                    let before = app.clone();
                    for (v, local) in [(company, "Company"), (manager, "Manager")] {
                        if let Some(v) = v {
                            docmeta::set_app_text(app, local, v);
                        }
                    }
                    if *app != before || self.pkg.get(&self.app_path).is_none() {
                        self.app_dirty = true;
                    }
                }
                // Cover pages, headers and footers that show these properties follow them.
                self.refresh_property_controls();
                Ok(OpResult::default())
            }
            Op::SetCustomProps { set, remove } => {
                if set.is_empty() && remove.is_empty() {
                    return Ok(OpResult::default());
                }
                self.ensure_custom_part();
                let custom = self.custom.as_mut().expect("just made");
                let before = custom.clone();
                for name in remove {
                    docmeta::remove_custom(custom, name);
                }
                for prop in set {
                    docmeta::set_custom(custom, prop).map_err(EditError::Invalid)?;
                }
                if *custom != before {
                    self.custom_dirty = true;
                }
                Ok(OpResult::default())
            }
            Op::SetLanguage { lang, ids } => self.op_set_language(lang, ids.as_deref()),
            Op::RemoveHiddenData { what } => self.op_remove_hidden_data(what),
            // Notes, captions, cross-references, fields, the index: see `references` and `updating`.
            Op::InsertEndnote { at, text } => self.op_insert_note(notes::NoteKind::Endnote, *at, text),
            Op::ConvertNotes { to, ids } => self.op_convert_notes(*to, ids.as_deref()),
            Op::SetNoteFormat { kind, format, start } => self.op_set_note_format(*kind, format, *start),
            Op::InsertPageNumber { section, spec } => self.op_insert_page_number(*section, spec),
            Op::InsertCoverPage { design, fragment, labels } => self.op_insert_cover_page(design.as_deref(), fragment.as_ref(), labels),
            Op::RemoveCoverPage => self.op_remove_cover_page(),
            Op::InsertPagePart { kind, design, section, labels } => self.op_insert_page_part(*kind, design, *section, labels),
            Op::RemovePageNumbers => self.op_remove_page_numbers(),
            Op::SetSection { section, from, patch } => self.op_set_section(*section, *from, patch),
            Op::SetPageBorders { section, from, borders } => self.op_set_page_borders(*section, *from, borders.as_ref()),
            Op::SetNoteProps { kind, section, from, patch } => self.op_set_note_props(*kind, *section, *from, patch),
            Op::SetLayoutSettings { patch } => self.op_set_layout_settings(patch),
            Op::SetPageBackground { fill } => self.op_set_page_background(fill.as_ref()),
            Op::SetFrame { ids, frame } => self.op_set_frame(ids, frame.as_ref()),
            Op::SetDropCap { id, options } => self.op_set_drop_cap(id.0, options),
            Op::InsertCustomNote { at, kind, mark, text } => self.op_insert_custom_note(*at, *kind, mark, text),
            Op::InsertCaption { after, before, label, text, position, separator, chapter } => self.op_insert_caption(after.map(|i| i.0), before.map(|i| i.0), label, text, *position, separator.as_deref(), chapter.as_ref()),
            Op::InsertCrossReference { at, target, show, link, above_below, pages } => self.op_insert_cross_reference(*at, target, *show, *link, *above_below, pages.as_ref()),
            Op::UpdateFields { pages, scope, open, only, input, .. } => self.op_update_fields(pages.as_ref(), scope.as_deref(), &field_ops::UpdateExtra { open: *open, only: only.clone(), input: input.clone() }),
            Op::SetFieldCode { at, instr, input, pages } => self.op_set_field_code(*at, instr, input.as_ref(), pages.as_ref()),
            Op::SetFieldResult { at, text } => self.op_set_field_result(*at, text),
            Op::MarkIndexEntry { at, entry, sub, see, bold, italic } => self.op_mark_index_entry(*at, entry, sub.as_deref(), see.as_deref(), *bold, *italic),
            Op::MarkAllIndexEntries { text, entry, sub, see, bold, italic, ignore_case, whole_word } => {
                self.op_mark_all_index_entries(text, entry.as_deref(), sub.as_deref(), see.as_deref(), *bold, *italic, *ignore_case, *whole_word)
            }
            Op::InsertIndex { after, before, columns, run_in, headings, pages, options } => self.op_insert_index(after.map(|i| i.0), before.map(|i| i.0), *columns, *run_in, *headings, options.as_ref(), pages.as_ref()),
            Op::SetIndex { at, options, pages } => self.op_set_index(at.map(|i| i.0), options, pages.as_ref()),
            Op::AutoMarkIndex { entries } => self.op_auto_mark(entries),
            Op::InsertBuildingBlock { at, index, guid, name } => self.op_insert_building_block(*at, *index, guid.as_deref(), name.as_deref()),
            Op::InsertTableOfFigures { after, before, label, pages } => self.op_insert_table_of_figures(after.map(|i| i.0), before.map(|i| i.0), label, pages.as_ref()),
            Op::AddSource { .. }
            | Op::UpdateSource { .. }
            | Op::DeleteSource { .. }
            | Op::SetCitationStyle { .. }
            | Op::InsertCitation { .. }
            | Op::InsertCitationPlaceholder { .. }
            | Op::EditCitation { .. }
            | Op::RemoveCitation { .. }
            | Op::InsertBibliography { .. }
            | Op::UpdateBibliography => self.apply_cite_op(op),
            // Shapes and text boxes: see `shapes`.
            Op::InsertShape { .. } | Op::InsertTextBox { .. } | Op::SetShape { .. } => self.apply_shape_op(op),
            // Arranging objects: see `arrange`.
            Op::SetObjectOrder { .. } | Op::SetBehindText { .. } | Op::GroupShapes { .. } | Op::UngroupShape { .. } | Op::InsertInk { .. } => self.apply_arrange_op(op),
            // Mail merge, labels and envelopes: see `mail_merge`.
            Op::SetMailMerge { .. } | Op::SetMergeRecipients { .. } | Op::InsertMergeField { .. } | Op::UpdateLabels { .. } | Op::MakeLabels { .. } | Op::InsertEnvelope { .. } => self.apply_merge_op(op),
            // Names, hiding, placing and stacking any drawing object: see `object_props`.
            Op::SetObjectProps { .. } | Op::SetObjectPosition { .. } | Op::MoveObject { .. } => self.apply_object_op(op),
            // Curves, freeforms and the points of a shape: see `geometry`. Connectors: see `connectors`.
            Op::InsertFreeform { .. } | Op::SetShapePath { .. } => self.apply_geometry_op(op),
            Op::InsertConnector { .. } | Op::SetConnector { .. } => self.apply_connector_op(op),
        }
    }
}
