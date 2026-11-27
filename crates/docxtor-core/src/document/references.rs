//! Notes (footnotes and endnotes), captions and cross-references: the ops, and the lookups the editor's
//! "Insert cross-reference" dialog needs. Fields are written the way Word writes them (see `fields`).

use super::*;
use crate::fields::{self, Instr};
use crate::notes::{NoteFormat, NoteKind};

// ---------------------------------------------------------------------------
// What the cross-reference dialog lists
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XHeading {
    pub p: NodeId,
    pub text: String,
    pub level: u8,
    /// The heading's list number (`1.2`), when it is numbered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XBookmark {
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XCaption {
    pub p: NodeId,
    /// The label before the number: `Figure`, `Table`.
    pub label: String,
    /// The number as shown: `1`.
    pub number: String,
    /// The whole caption.
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XNote {
    /// The note's id (what `footnote` and `endnote` targets take).
    pub id: String,
    pub num: u32,
    /// The number as the document formats it.
    pub label: String,
    /// The start of the note's text.
    pub text: String,
}

/// Everything a cross-reference can point at, in document order. See `Document::xref_targets`.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct XrefTargets {
    pub headings: Vec<XHeading>,
    pub bookmarks: Vec<XBookmark>,
    pub captions: Vec<XCaption>,
    pub footnotes: Vec<XNote>,
    pub endnotes: Vec<XNote>,
}

/// A caption found in the text: a paragraph with a `SEQ` field.
#[derive(Debug, Clone)]
pub(super) struct CaptionInfo {
    pub p: NodeId,
    /// The sequence name in the field (`Figure`); two captions with the same one are numbered together.
    pub id: String,
    /// The text before the number (`Figure`).
    pub label: String,
    pub number: String,
    pub text: String,
    /// Where the field starts, and how long its result is.
    pub seq_offset: usize,
    pub seq_len: usize,
}

/// A bookmark with the text it covers.
#[derive(Debug, Clone)]
pub(super) struct BmInfo {
    pub name: String,
    pub p: NodeId,
    pub o: usize,
    pub end_p: NodeId,
    pub end_o: usize,
    pub text: String,
}

/// The part of a paragraph a hidden bookmark covers.
#[derive(Debug, Clone, Copy)]
pub(super) enum BmRange {
    /// All of the paragraph's content.
    Whole,
    /// A caption's label and number.
    LabelNumber,
    /// A caption's number only.
    Number,
    /// A caption's text after its number.
    AfterNumber,
    Offsets(usize, usize),
}

fn runs_mut(inlines: &mut [Inline], f: &mut dyn FnMut(&mut Run)) {
    for i in inlines {
        match i {
            Inline::Run(r) => f(r),
            Inline::Container(c) => runs_mut(&mut c.children, f),
            Inline::Opaque(_) => {}
        }
    }
}

fn is_field_begin(i: &Inline) -> bool {
    matches!(i, Inline::Run(r) if r.items.iter().any(|it| matches!(it, RunItem::Hidden(e) if e.name == "w:fldChar" && e.attr("w:fldCharType") == Some("begin"))))
}

/// Put a bookmark marker at `off`, after the zero-width pieces that finish what comes before it (a field's
/// end), so a bookmark around "Figure 1" closes after the whole number field.
fn insert_marker(inlines: &mut Vec<Inline>, off: usize, el: Element, is_end: bool) {
    edit::split_at(inlines, off);
    let mut start = 0;
    let mut at = inlines.len();
    for (i, inl) in inlines.iter_mut().enumerate() {
        let len = inl.len16();
        if start == off {
            at = i;
            break;
        }
        if start < off && off < start + len {
            if let Inline::Container(c) = inl {
                insert_marker(&mut c.children, off - start, el, is_end);
                return;
            }
        }
        start += len;
    }
    while at < inlines.len() && inlines[at].len16() == 0 && !is_field_begin(&inlines[at]) && !(is_end && matches!(&inlines[at], Inline::Opaque(e) if e.name == "w:bookmarkStart")) {
        at += 1;
    }
    inlines.insert(at, Inline::Opaque(el));
}

fn utf16_slice(s: &str, from: usize, to: usize) -> String {
    let u: Vec<u16> = s.encode_utf16().collect();
    let (a, b) = (from.min(u.len()), to.min(u.len()));
    String::from_utf16_lossy(&u[a..b.max(a)])
}

/// Apply `\* Upper`, `\* Lower`, `\* Caps` and `\* FirstCap` to a field result.
pub(super) fn case_format(text: &str, instr: &Instr) -> String {
    let mut out = text.to_string();
    for f in instr.formats() {
        out = match f.to_lowercase().as_str() {
            "upper" => out.to_uppercase(),
            "lower" => out.to_lowercase(),
            "firstcap" => {
                let mut c = out.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
            }
            "caps" => out.split(' ').map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
            }).collect::<Vec<_>>().join(" "),
            _ => out,
        };
    }
    out
}

impl Document {
    // -----------------------------------------------------------------------
    // Note numbering
    // -----------------------------------------------------------------------

    /// How footnotes and endnotes are numbered: the settings, then the last section's own settings.
    pub(super) fn note_formats(&self) -> (NoteFormat, NoteFormat) {
        let settings = self.settings.as_ref().map(|d| &d.root);
        let sect = self.final_sect_pr();
        (NoteFormat::read(NoteKind::Footnote, settings, sect.as_ref()), NoteFormat::read(NoteKind::Endnote, settings, sect.as_ref()))
    }

    /// Note numbers by id, in order of reference in the main text (1-based, before formatting).
    pub(super) fn note_nums(&self, kind: NoteKind) -> HashMap<String, u32> {
        facts::nums_of(&self.note_events().iter().collect::<Vec<_>>(), kind)
    }

    /// Footnote numbers by id, endnote numbers by `e:` + id, and a code for each number format: when this
    /// changes, every note mark in the text has to be drawn again.
    pub(super) fn note_state(&self) -> HashMap<String, u32> {
        self.note_facts_from(&self.note_events().iter().collect::<Vec<_>>()).0
    }

    /// `note_state` and the note labels, from the notes the text refers to (see `facts.rs`).
    pub(super) fn note_facts_from(&self, evs: &[&NoteEv]) -> (HashMap<String, u32>, HashMap<String, String>) {
        let mut out = facts::nums_of(evs, NoteKind::Footnote);
        for (id, n) in facts::nums_of(evs, NoteKind::Endnote) {
            out.insert(format!("e:{id}"), n);
        }
        let (f, e) = self.note_formats();
        out.insert("~f".into(), f.code());
        out.insert("~e".into(), e.code());
        // Sections that number their notes another way change the labels without changing the format above.
        let mut h: u32 = 2166136261;
        let labels = self.note_labels_from(evs);
        let mut keys: Vec<&String> = labels.keys().collect();
        keys.sort();
        for k in keys {
            for b in k.bytes().chain(labels[k].bytes()).chain([0]) {
                h = (h ^ u32::from(b)).wrapping_mul(16777619);
            }
        }
        out.insert("~l".into(), h);
        // Which notes the text refers to, in order. A note with a mark of its own has no number, so the numbers above
        // do not see it come or go (a rebuilt contents entry can take its reference away); the page setup lists
        // exactly the notes the text refers to.
        let mut order: Vec<(usize, String)> = facts::order_of(evs).into_iter().map(|(k, at)| (at, k)).collect();
        order.sort();
        let mut o: u32 = 2166136261;
        for (_, k) in &order {
            for b in k.bytes().chain([0]) {
                o = (o ^ u32::from(b)).wrapping_mul(16777619);
            }
        }
        out.insert("~o".into(), o);
        (out, labels)
    }

    /// Give a render context the note numbers (see `note_state`) and formats.
    pub(super) fn set_note_ctx(&self, ctx: &mut RenderCtx, state: &HashMap<String, u32>) {
        for (k, v) in state {
            if let Some(id) = k.strip_prefix("e:") {
                ctx.endnotes.insert(id.to_string(), *v);
            } else if !k.starts_with('~') {
                ctx.notes.insert(k.clone(), *v);
            }
        }
        let (f, e) = self.note_formats();
        ctx.footnote_fmt = f;
        ctx.endnote_fmt = e;
        ctx.note_labels = if self.facts.notes_fresh() { self.facts.note_labels().clone() } else { self.note_labels() };
    }

    // -----------------------------------------------------------------------
    // Endnotes
    // -----------------------------------------------------------------------

    /// The part holding notes of this kind, made (with Word's separator notes) when the document has none.
    pub(super) fn ensure_notes_part(&mut self, kind: NoteKind) -> String {
        let (root, rel_type, ct, stem) = match kind {
            NoteKind::Footnote => ("w:footnotes", package::REL_FOOTNOTES, package::CT_FOOTNOTES, "footnotes"),
            NoteKind::Endnote => ("w:endnotes", package::REL_ENDNOTES, package::CT_ENDNOTES, "endnotes"),
        };
        if let Some((p, _)) = self.story_parts.parts.iter().find(|(_, d)| d.root.name == root) {
            return p.clone();
        }
        let main = self.main_path.clone();
        // Undo can take the part out of the parsed parts while the file stays in the package.
        let existing = self.pkg.rels_for(&main).ok().and_then(|rs| rs.into_iter().find(|r| r.rel_type == rel_type)).map(|r| package::resolve_target(&main, &r.target));
        if let Some(path) = existing {
            if let Ok(mut doc) = self.pkg.xml(&path) {
                doc.root.children.retain(|n| !matches!(n, Node::Element(e) if e.name == kind.note_tag() && e.attr("w:type").is_none_or(|t| t == "normal")));
                self.story_parts.parts.push((path.clone(), doc));
                self.dirty_parts.insert(path.clone());
                return path;
            }
        }
        let part = self.free_part(stem).replace(&format!("{stem}1.xml"), &format!("{stem}.xml"));
        let part = if self.pkg.has(&part) { self.free_part(stem) } else { part };
        let doc = match kind {
            NoteKind::Footnote => stories::footnotes_part(),
            NoteKind::Endnote => stories::endnotes_part(),
        };
        self.pkg.set(&part, doc.to_bytes());
        self.pkg.ensure_override(&part, ct);
        self.pkg.add_rel(&main, rel_type, &relative_target(&main, &part), false);
        self.story_parts.parts.push((part.clone(), doc));
        if kind == NoteKind::Endnote {
            // Word lists the separator notes in the settings too.
            self.settings_edit(true, &mut |r| {
                if r.child("w:endnotePr").is_none() {
                    let pr = Element::new("w:endnotePr").with_child(Element::new("w:endnote").with_attr("w:id", "-1")).with_child(Element::new("w:endnote").with_attr("w:id", "0"));
                    crate::props::put_ordered(r, pr, SETTINGS_ORDER);
                }
            });
        }
        part
    }

    pub(super) fn op_insert_note(&mut self, kind: NoteKind, at: Pos, text: &str) -> Result<OpResult, EditError> {
        let off = at.o.min(self.para_ref(at.p.0)?.len16());
        let part = self.ensure_notes_part(kind);
        let text_style = self.styles.ensure_builtin(kind.text_style()).unwrap_or_else(|| kind.text_style().into());
        let ref_style = self.styles.ensure_builtin(kind.ref_style()).unwrap_or_else(|| kind.ref_style().into());
        let doc = self.story_parts.get_mut(&part).expect("notes part");
        let id = doc.root.children_named(kind.note_tag()).filter_map(|n| n.attr("w:id")?.parse::<i64>().ok()).max().unwrap_or(0).max(0) + 1;
        let id = id.to_string();
        let para = match kind {
            NoteKind::Footnote => stories::footnote_paragraph(&text_style, &ref_style),
            NoteKind::Endnote => stories::endnote_paragraph(&text_style, &ref_style),
        };
        let note = Element::new(kind.note_tag()).with_attr("w:id", &id).with_child(para);
        let mut blocks = parse_blocks(&note, &mut self.ids);
        doc.root.children.push(Node::Element(note));
        if !text.is_empty() {
            if let Some(Block::Para(p)) = blocks.first_mut() {
                let end = p.len16();
                edit::insert_text(&mut p.inlines, end, text, None);
            }
        }
        let caret = match blocks.first() {
            Some(Block::Para(p)) => Some(Pos::new(p.id, p.len16())),
            _ => None,
        };
        self.story_gen += 1;
        let skind = if kind == NoteKind::Endnote { StoryKind::Endnote } else { StoryKind::Footnote };
        self.stories.push(Story { kind: skind, part: part.clone(), note_id: Some(id.clone()), blocks, gen: self.story_gen });
        self.dirty_parts.insert(part);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(stories::note_reference_run(kind.reference_tag(), &ref_style, &id)));
        self.stories_touched = true;
        Ok(OpResult { caret, ..Default::default() })
    }

    /// Move notes from one part to the other (Word's Convert): the notes keep their content and their paragraph
    /// ids, get the other kind's styles and marks, and their references in the text turn into the other kind.
    pub(super) fn op_convert_notes(&mut self, to: NoteKind, ids: Option<&[String]>) -> Result<OpResult, EditError> {
        let from = if to == NoteKind::Endnote { NoteKind::Footnote } else { NoteKind::Endnote };
        let (from_story, to_story) = if to == NoteKind::Endnote { (StoryKind::Footnote, StoryKind::Endnote) } else { (StoryKind::Endnote, StoryKind::Footnote) };
        let order = self.note_nums(from);
        let mut picks: Vec<(u32, usize)> = self
            .stories
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == from_story && s.note_id.as_ref().is_some_and(|id| ids.is_none_or(|l| l.contains(id))))
            .map(|(i, s)| (order.get(s.note_id.as_deref().unwrap_or_default()).copied().unwrap_or(u32::MAX), i))
            .collect();
        if picks.is_empty() {
            let what = if from == NoteKind::Footnote { "footnotes" } else { "endnotes" };
            return Err(EditError::Invalid(format!("There are no {what} to convert.")));
        }
        picks.sort();
        let dst = self.ensure_notes_part(to);
        let text_style = self.styles.ensure_builtin(to.text_style()).unwrap_or_else(|| to.text_style().into());
        let ref_style = self.styles.ensure_builtin(to.ref_style()).unwrap_or_else(|| to.ref_style().into());
        let mut next = self.story_parts.get(&dst).map(|d| d.root.children_named(to.note_tag()).filter_map(|n| n.attr("w:id")?.parse::<i64>().ok()).max().unwrap_or(0).max(0)).unwrap_or(0) + 1;
        let mut moved: HashMap<String, String> = HashMap::new();
        for (_, si) in &picks {
            let old = self.stories[*si].note_id.clone().unwrap_or_default();
            let src = self.stories[*si].part.clone();
            let new_id = next.to_string();
            next += 1;
            if let Some(d) = self.story_parts.get_mut(&dst) {
                d.root.children.push(Node::Element(Element::new(to.note_tag()).with_attr("w:id", &new_id)));
            }
            if let Some(d) = self.story_parts.get_mut(&src) {
                d.root.children.retain(|n| !matches!(n, Node::Element(e) if e.name == from.note_tag() && e.attr("w:id") == Some(old.as_str())));
            }
            let from_text = from.text_style();
            let from_ref = from.ref_style();
            // Its pictures, charts and links stay with it: the relationships belong to the part it leaves.
            let mut blocks = std::mem::take(&mut self.stories[*si].blocks);
            self.carry_relationships(&src, &dst, &mut blocks);
            self.stories[*si].blocks = blocks;
            walk_paragraphs_mut(&mut self.stories[*si].blocks, &mut |p| {
                if let Some(ps) = p.ppr.as_mut().and_then(|pp| pp.child_mut("w:pStyle")) {
                    if ps.attr("w:val") == Some(from_text) {
                        ps.set_attr("w:val", &text_style);
                    }
                }
                runs_mut(&mut p.inlines, &mut |r| {
                    for it in r.items.iter_mut() {
                        if let RunItem::Object(e) = it {
                            if e.name == from.mark_tag() {
                                e.name = Name::intern(to.mark_tag());
                            }
                        }
                    }
                    if let Some(rs) = r.rpr.as_mut().and_then(|x| x.child_mut("w:rStyle")) {
                        if rs.attr("w:val") == Some(from_ref) {
                            rs.set_attr("w:val", &ref_style);
                        }
                    }
                });
            });
            self.story_gen += 1;
            let s = &mut self.stories[*si];
            s.kind = to_story;
            s.part = dst.clone();
            s.note_id = Some(new_id.clone());
            s.gen = self.story_gen;
            self.dirty_parts.insert(src);
            self.dirty_parts.insert(dst.clone());
            moved.insert(old, new_id);
        }
        // The marks in the text.
        let from_ref = from.ref_style();
        self.body.edit_paragraphs(&mut |p| {
            let mut any = false;
            runs_mut(&mut p.inlines, &mut |r| {
                let mut hit = false;
                for it in r.items.iter_mut() {
                    if let RunItem::Object(e) = it {
                        if e.name == from.reference_tag() {
                            if let Some(new) = e.attr("w:id").and_then(|id| moved.get(id)).cloned() {
                                e.name = Name::intern(to.reference_tag());
                                e.set_attr("w:id", new);
                                hit = true;
                            }
                        }
                    }
                }
                if hit {
                    any = true;
                    if let Some(rs) = r.rpr.as_mut().and_then(|x| x.child_mut("w:rStyle")) {
                        if rs.attr("w:val") == Some(from_ref) {
                            rs.set_attr("w:val", &ref_style);
                        }
                    }
                }
            });
            any
        });
        self.touch_all();
        self.stories_touched = true;
        Ok(OpResult { count: Some(moved.len()), ..Default::default() })
    }

    /// Number footnotes or endnotes another way for the whole document.
    pub(super) fn op_set_note_format(&mut self, kind: NoteKind, format: &str, start: Option<i64>) -> Result<OpResult, EditError> {
        let fmt = notes::ooxml_format(format.trim())
            .ok_or_else(|| EditError::Invalid(format!("Unknown number format '{format}'. Use decimal, lowerRoman, upperRoman, lowerLetter, upperLetter or symbol.")))?;
        if start.is_some_and(|s| s < 1) {
            return Err(EditError::Invalid("The first note number has to be 1 or more.".into()));
        }
        let tag = kind.pr_tag();
        self.settings_edit(true, &mut |root| {
            let mut pr = root.child(tag).cloned().unwrap_or_else(|| Element::new(tag));
            notes::write_format(&mut pr, Some(fmt), start);
            crate::props::put_ordered(root, pr, SETTINGS_ORDER);
        });
        // A section that sets its own numbering would hide the change.
        if self.section_refs().iter().any(|(_, s)| s.is_some_and(|s| s.child(tag).is_some())) {
            self.edit_sects(None, &mut |sect| {
                if let Some(pr) = sect.child_mut(tag) {
                    notes::write_format(pr, Some(fmt), start);
                }
            })?;
        }
        self.touch_all();
        self.stories_touched = true;
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Captions
    // -----------------------------------------------------------------------

    /// Every caption in the main text, in order.
    pub(super) fn caption_infos(&self) -> Vec<CaptionInfo> {
        let mut out = Vec::new();
        walk_paragraphs(self.main_body(), &mut |p| {
            let mut first = None;
            // A chapter number (`STYLEREF 1 \s`) in front of the sequence number is part of the number.
            let mut chapter: Option<usize> = None;
            fields::scan_inlines(&p.inlines, 0, &mut |s| {
                if first.is_none() {
                    let i = Instr::parse(s.instr);
                    if i.code == "STYLEREF" && i.has("s") && chapter.is_none() {
                        chapter = Some(s.offset);
                    } else if i.code == "SEQ" && !i.has("h") {
                        if let Some(id) = i.first_arg() {
                            first = Some((id.to_string(), s.offset, s.result.to_string()));
                        }
                    }
                }
            });
            if let Some((id, seq_at, seq_result)) = first {
                let (offset, number) = match chapter {
                    Some(c) if c < seq_at => (c, utf16_slice(&p.text(), c, seq_at + seq_result.encode_utf16().count()).replace(OBJ, "")),
                    _ => (seq_at, seq_result),
                };
                let label = utf16_slice(&p.text(), 0, offset).replace(OBJ, "").trim().to_string();
                let label = if label.is_empty() { id.replace('_', " ") } else { label };
                out.push(CaptionInfo { p: p.id, id, label, seq_len: number.encode_utf16().count(), number, text: p.plain_text().trim().to_string(), seq_offset: offset });
            }
        });
        out
    }

    /// Renumber every `SEQ` field in document order. Returns the paragraphs that changed.
    pub(super) fn renumber_sequences(&mut self) -> Vec<NodeId> {
        self.renumber_seq(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn op_insert_caption(&mut self, after: Option<NodeId>, before: Option<NodeId>, label: &str, text: &str, position: Option<CaptionPosition>, separator: Option<&str>, chapter: Option<&ChapterNumbering>) -> Result<OpResult, EditError> {
        let label = label.trim();
        if label.is_empty() {
            return Err(EditError::Invalid("Give the caption a label, such as Figure or Table.".into()));
        }
        let (after, before) = match (position, after, before) {
            (Some(CaptionPosition::Above), Some(t), None) | (Some(CaptionPosition::Above), None, Some(t)) => (None, Some(t)),
            (Some(CaptionPosition::Below), Some(t), None) | (Some(CaptionPosition::Below), None, Some(t)) => (Some(t), None),
            (_, a, b) => (a, b),
        };
        let anchor = after.or(before).ok_or_else(|| EditError::Invalid("Say which block the caption goes next to (after or before).".into()))?;
        if find_block(&self.body, anchor).is_none() {
            return Err(EditError::NotFound(anchor));
        }
        let style = self.styles.ensure_builtin("Caption").unwrap_or_else(|| "Caption".into());
        let seq_id = label.split_whitespace().collect::<Vec<_>>().join("_");
        let id = self.ids.fresh();
        let mut p = Paragraph::new(id);
        let mut ppr = Element::new("w:pPr");
        put_ppr(&mut ppr, Element::new("w:pStyle").with_attr("w:val", style));
        p.ppr = Some(ppr);
        let text_run = |t: String| Inline::Run(Run::of_text(None, &t));
        p.inlines.push(text_run(format!("{label} ")));
        // With a chapter number: `Figure {STYLEREF 1 \s}-{SEQ Figure \s 1}`, as Word writes it.
        let mut restart = String::new();
        if let Some(c) = chapter {
            let level = c.level.clamp(1, 9);
            p.inlines.extend(fields::complex(&format!(" STYLEREF {level} \\s "), "0", Some(fields::no_proof()), false));
            p.inlines.push(text_run(c.separator.clone().unwrap_or_else(|| "-".into())));
            restart = format!(" \\s {level}");
        }
        p.inlines.extend(fields::complex(&format!(" SEQ {seq_id} \\* ARABIC{restart} "), "1", Some(fields::no_proof()), false));
        if !text.is_empty() {
            p.inlines.push(text_run(format!("{}{text}", separator.unwrap_or(": "))));
        }
        self.insert_blocks(after, before, vec![Block::Para(p)])?;
        self.renumber_sequences();
        if let Some(c) = chapter {
            // A heading above that has a number is what the chapter number comes from.
            let mut missing = false;
            fields::scan_inlines(&self.para_ref(id)?.inlines, 0, &mut |s| missing |= Instr::parse(s.instr).code == "STYLEREF" && s.result == "0");
            if missing {
                return Err(EditError::Invalid(format!("There is no numbered heading of level {} above the caption. Number the headings first.", c.level.clamp(1, 9))));
            }
        }
        let len = self.para_ref(id)?.len16();
        Ok(OpResult { caret: Some(Pos::new(id, len)), created: vec![id], count: None })
    }

    // -----------------------------------------------------------------------
    // Hidden bookmarks and what they cover
    // -----------------------------------------------------------------------

    /// The bookmarks with the text they cover, by lower-case name (hidden ones included).
    pub(super) fn bookmark_infos(&self) -> HashMap<String, BmInfo> {
        let scan = marks::scan(&self.block_lists());
        let mut out = HashMap::new();
        for (i, ev) in scan.events.iter().enumerate() {
            let marks::Kind::Start { id, name } = &ev.kind else { continue };
            let end = scan.events[i + 1..].iter().position(|e| matches!(&e.kind, marks::Kind::End { id: eid } if eid == id)).map(|k| i + 1 + k);
            let (end_p, end_o) = end.map(|k| (scan.events[k].p, scan.events[k].o)).unwrap_or((ev.p, ev.o));
            let mut text = String::new();
            if let Some(k) = end {
                let mut last = ev.p;
                for e in &scan.events[i + 1..k] {
                    if let marks::Kind::Text(t) = &e.kind {
                        if e.p != last && !text.is_empty() {
                            text.push(' ');
                        }
                        last = e.p;
                        text.push_str(t);
                    }
                }
            }
            out.entry(name.to_lowercase()).or_insert(BmInfo { name: name.clone(), p: ev.p, o: ev.o, end_p, end_o, text: text.replace(OBJ, "") });
        }
        out
    }

    /// Offsets of the parts of a caption paragraph.
    fn range_offsets(&self, pid: NodeId, range: BmRange) -> Result<(usize, usize), EditError> {
        let len = self.para_ref(pid)?.len16();
        Ok(match range {
            BmRange::Whole => (0, len),
            BmRange::Offsets(a, b) => (a, b),
            BmRange::LabelNumber | BmRange::Number | BmRange::AfterNumber => {
                let cap = self.caption_infos().into_iter().find(|c| c.p == pid).ok_or_else(|| EditError::Invalid("That paragraph isn't a caption.".into()))?;
                let end = cap.seq_offset + cap.seq_len;
                match range {
                    BmRange::LabelNumber => (0, end),
                    BmRange::Number => (cap.seq_offset, end),
                    _ => (end, len),
                }
            }
        })
    }

    /// A hidden bookmark (`_Ref…` for cross-references, `_Toc…` for tables of contents) around part of a
    /// paragraph: the one already there if it covers exactly that, else a new one.
    pub(super) fn ensure_hidden_bookmark(&mut self, pid: NodeId, range: BmRange, prefix: &str) -> Result<String, EditError> {
        // A table of contents or figures link needs any `_Toc` bookmark on the paragraph (cheap to look for).
        if prefix == "_Toc" {
            if let Some(n) = toc::toc_bookmark(self.para_ref(pid)?) {
                return Ok(n);
            }
        }
        let (from, to) = self.range_offsets(pid, range)?;
        if prefix != "_Toc" {
            let infos = self.bookmark_infos();
            let reuse = infos.values().filter(|b| b.name.starts_with(prefix) && b.p == pid && b.end_p == pid && b.o == from && b.end_o == to).map(|b| b.name.clone()).min();
            if let Some(name) = reuse {
                return Ok(name);
            }
        }
        let (ids, names) = self.bookmark_ids_and_names();
        let top = names.iter().filter_map(|n| n.strip_prefix(prefix)?.parse::<u64>().ok()).max().unwrap_or(0).max(100_000_000);
        let mut n = top + 1;
        let mut name = format!("{prefix}{n}");
        while names.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            n += 1;
            name = format!("{prefix}{n}");
        }
        let id = (ids.iter().max().copied().unwrap_or(0) + 1).to_string();
        self.touch(pid);
        let p = self.para_mut(pid)?;
        let to = to.min(p.len16());
        insert_marker(&mut p.inlines, to, Element::new("w:bookmarkEnd").with_attr("w:id", &id), true);
        let start = Element::new("w:bookmarkStart").with_attr("w:id", &id).with_attr("w:name", &name);
        if from == 0 {
            edit::insert_inline_at(&mut p.inlines, 0, Inline::Opaque(start));
        } else {
            // After a field that ends here, not inside its result.
            insert_marker(&mut p.inlines, from.min(to), start, false);
        }
        Ok(name)
    }

    // -----------------------------------------------------------------------
    // Cross-references
    // -----------------------------------------------------------------------

    /// Pages by paragraph id: the layout's (`pages`, keyed by id) or an estimate from page breaks.
    pub(super) fn page_map(&self, pages: Option<&HashMap<String, u32>>) -> HashMap<NodeId, u32> {
        match pages {
            Some(m) => m.iter().filter_map(|(k, v)| Some((k.trim_start_matches(|c: char| c.is_ascii_alphabetic()).parse::<NodeId>().ok()?, *v))).collect(),
            None => self.page_estimates(),
        }
    }

    /// The list number of a paragraph without its trailing period (`1.2`), if it has one.
    pub(super) fn para_number(labels: &HashMap<NodeId, ListLabel>, pid: NodeId) -> Option<String> {
        let l = labels.get(&pid)?;
        let t = l.text.trim().trim_end_matches('.').trim().to_string();
        (!l.bullet && !t.is_empty()).then_some(t)
    }

    /// Where a note's mark is in the main text: paragraph and offset.
    fn note_mark_at(&self, kind: NoteKind, id: &str) -> Option<(NodeId, usize)> {
        let mut found = None;
        walk_paragraphs(self.main_body(), &mut |p| {
            walk_runs(&p.inlines, &mut 0, &mut |r, start, _| {
                let mut pos = start;
                for it in &r.items {
                    if let RunItem::Object(e) = it {
                        if found.is_none() && e.name == kind.reference_tag() && e.attr("w:id") == Some(id) {
                            found = Some((p.id, pos));
                        }
                    }
                    pos += it.len16();
                }
            });
        });
        found
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn op_insert_cross_reference(&mut self, at: Pos, target: &XrefTarget, show: XrefShow, link: bool, above_below: bool, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let host = at.p.0;
        self.para_ref(host)?;
        let labels = self.numbering.labels(&self.body, &self.styles);
        let chains = self.number_chains(&labels);
        // The number of a paragraph with the context the field asks for (`\r` relative to this paragraph, `\n` alone, `\w` full).
        let number_of = |pid: NodeId, show: XrefShow| -> Option<String> {
            let mode = match show {
                XrefShow::NumberNoContext => 'n',
                XrefShow::NumberFullContext => 'w',
                _ => 'r',
            };
            Self::numbered_text(&labels, &chains, pid, Some(host), mode)
        };
        let page_of = |s: &Self, pid: NodeId| -> String { s.page_labels().label(pid, s.page_map(pages).get(&pid).copied().unwrap_or(1)) };
        let number_switch = |show: XrefShow| match show {
            XrefShow::Number => "\\r",
            XrefShow::NumberNoContext => "\\n",
            _ => "\\w",
        };
        let bad = |what: &str| EditError::Invalid(format!("A {what} can't be shown that way."));
        // The bookmark, what the field says and its result.
        let (name, code, switch, result, rstyle, target_para): (String, &str, Option<&str>, String, Option<String>, NodeId) = match target {
            XrefTarget::Heading { p } => {
                let para = self.para_ref(p.0)?;
                let text = para.plain_text().trim().to_string();
                if self.heading_level(para).is_none() || text.is_empty() {
                    return Err(EditError::Invalid("That paragraph isn't a heading with text.".into()));
                }
                match show {
                    XrefShow::Text => (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "REF", None, text, None, p.0),
                    XrefShow::Number | XrefShow::NumberNoContext | XrefShow::NumberFullContext => {
                        let n = number_of(p.0, show).ok_or_else(|| EditError::Invalid("That heading isn't numbered.".into()))?;
                        (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "REF", Some(number_switch(show)), n, None, p.0)
                    }
                    XrefShow::Page => (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "PAGEREF", None, page_of(self, p.0), None, p.0),
                    _ => return Err(bad("heading")),
                }
            }
            XrefTarget::Numbered { p } => {
                let para = self.para_ref(p.0)?;
                let text = para.plain_text().trim().to_string();
                if Self::para_number(&labels, p.0).is_none() {
                    return Err(EditError::Invalid("That paragraph isn't a numbered item.".into()));
                }
                match show {
                    XrefShow::Text => (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "REF", None, text, None, p.0),
                    XrefShow::Number | XrefShow::NumberNoContext | XrefShow::NumberFullContext => {
                        let n = number_of(p.0, show).ok_or_else(|| EditError::Invalid("That paragraph isn't a numbered item.".into()))?;
                        (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "REF", Some(number_switch(show)), n, None, p.0)
                    }
                    XrefShow::Page => (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "PAGEREF", None, page_of(self, p.0), None, p.0),
                    _ => return Err(bad("numbered item")),
                }
            }
            XrefTarget::Caption { p } => {
                let cap = self.caption_infos().into_iter().find(|c| c.p == p.0).ok_or_else(|| EditError::Invalid("That paragraph isn't a caption (it has no SEQ field).".into()))?;
                let full = self.para_ref(p.0)?.text();
                let end = cap.seq_offset + cap.seq_len;
                match show {
                    XrefShow::Text => (self.ensure_hidden_bookmark(p.0, BmRange::Whole, "_Ref")?, "REF", None, cap.text.clone(), None, p.0),
                    XrefShow::LabelNumber => (self.ensure_hidden_bookmark(p.0, BmRange::LabelNumber, "_Ref")?, "REF", None, utf16_slice(&full, 0, end).replace(OBJ, ""), None, p.0),
                    XrefShow::Number => (self.ensure_hidden_bookmark(p.0, BmRange::Number, "_Ref")?, "REF", None, cap.number.clone(), None, p.0),
                    XrefShow::CaptionText => {
                        let rest = utf16_slice(&full, end, usize::MAX).replace(OBJ, "");
                        (self.ensure_hidden_bookmark(p.0, BmRange::AfterNumber, "_Ref")?, "REF", None, rest, None, p.0)
                    }
                    XrefShow::Page => (self.ensure_hidden_bookmark(p.0, BmRange::LabelNumber, "_Ref")?, "PAGEREF", None, page_of(self, p.0), None, p.0),
                    _ => return Err(bad("caption")),
                }
            }
            XrefTarget::Bookmark { name } => {
                let info = self.bookmark_infos().get(&name.trim().to_lowercase()).cloned().ok_or_else(|| EditError::Invalid(format!("There is no bookmark called {}.", name.trim())))?;
                match show {
                    XrefShow::Text => (info.name.clone(), "REF", None, info.text.clone(), None, info.p),
                    XrefShow::Number | XrefShow::NumberNoContext | XrefShow::NumberFullContext => {
                        let n = number_of(info.p, show).ok_or_else(|| EditError::Invalid("That bookmark isn't in a numbered paragraph.".into()))?;
                        (info.name.clone(), "REF", Some(number_switch(show)), n, None, info.p)
                    }
                    XrefShow::Page => (info.name.clone(), "PAGEREF", None, page_of(self, info.p), None, info.p),
                    _ => return Err(bad("bookmark")),
                }
            }
            XrefTarget::Footnote { id } | XrefTarget::Endnote { id } => {
                let kind = if matches!(target, XrefTarget::Endnote { .. }) { NoteKind::Endnote } else { NoteKind::Footnote };
                let what = if kind == NoteKind::Endnote { "endnote" } else { "footnote" };
                let (pid, off) = self.note_mark_at(kind, id).ok_or_else(|| EditError::Invalid(format!("There is no {what} with id {id}.")))?;
                let num = self.note_nums(kind).get(id).copied().unwrap_or(1);
                let (ff, ef) = self.note_formats();
                let label = if kind == NoteKind::Endnote { ef.label(num) } else { ff.label(num) };
                let bm = self.ensure_hidden_bookmark(pid, BmRange::Offsets(off, off + 1), "_Ref")?;
                match show {
                    XrefShow::Number => (bm, "NOTEREF", None, label, None, pid),
                    XrefShow::NumberFormatted => {
                        let st = self.styles.ensure_builtin(kind.ref_style()).unwrap_or_else(|| kind.ref_style().into());
                        (bm, "NOTEREF", Some("\\f"), label, Some(st), pid)
                    }
                    XrefShow::Page => (bm, "PAGEREF", None, page_of(self, pid), None, pid),
                    _ => return Err(bad(what)),
                }
            }
        };
        let mut instr = format!(" {code} {name}");
        if let Some(s) = switch {
            instr.push(' ');
            instr.push_str(s);
        }
        if link {
            instr.push_str(" \\h");
        }
        instr.push(' ');
        // Above or below: "above" when the target comes first in the text.
        let order: HashMap<NodeId, usize> = paragraph_order(self.main_body()).into_iter().enumerate().map(|(i, p)| (p, i)).collect();
        let word = if order.get(&target_para).copied().unwrap_or(0) <= order.get(&host).copied().unwrap_or(0) { "above" } else { "below" };
        self.touch(host);
        let p = self.para_mut(host)?;
        let off = at.o.min(p.len16());
        let rpr = edit::rpr_at(&p.inlines, off).map(strip_link_style).filter(|r| !r.children.is_empty());
        // A formatted note number looks like the note's mark.
        let result_rpr = match &rstyle {
            Some(s) => Some(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", s))),
            None => rpr.clone(),
        };
        let mut inl = fields::complex(&instr, &result, result_rpr, false);
        let mut shown = result.encode_utf16().count();
        if above_below && code != "PAGEREF" {
            inl.push(Inline::Run(Run { attrs: vec![], t_attrs: None, rpr: rpr.clone(), items: vec![RunItem::Text(" ".into())] }));
            let second = format!(" REF {name} \\p{} ", if link { " \\h" } else { "" });
            inl.extend(fields::complex(&second, word, rpr, false));
            shown += 1 + word.len();
        }
        fields::insert_inlines_at(&mut p.inlines, off, inl);
        Ok(OpResult { caret: Some(Pos::new(host, off + shown)), ..Default::default() })
    }

    /// Everything a cross-reference can point at, in document order (for the "Insert cross-reference" dialog).
    pub fn xref_targets(&self) -> XrefTargets {
        let labels = self.numbering.labels(&self.body, &self.styles);
        let mut out = XrefTargets::default();
        walk_paragraphs(self.main_body(), &mut |p| {
            let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or_default();
            if style.starts_with("TOC") || style == "TableofFigures" {
                return;
            }
            if let Some(level) = self.heading_level(p) {
                let text = p.plain_text().trim().to_string();
                if !text.is_empty() {
                    out.headings.push(XHeading { p: p.id, text, level, number: Self::para_number(&labels, p.id) });
                }
            }
        });
        out.bookmarks = self.bookmarks().into_iter().map(|b| XBookmark { name: b.name, text: b.text }).collect();
        out.captions = self.caption_infos().into_iter().map(|c| XCaption { p: c.p, label: c.label, number: c.number, text: c.text }).collect();
        let (ff, ef) = self.note_formats();
        for (kind, fmt, list) in [(NoteKind::Footnote, &ff, &mut out.footnotes), (NoteKind::Endnote, &ef, &mut out.endnotes)] {
            let nums = self.note_nums(kind);
            let skind = if kind == NoteKind::Endnote { StoryKind::Endnote } else { StoryKind::Footnote };
            for s in self.stories.iter().filter(|s| s.kind == skind) {
                let Some(id) = s.note_id.clone() else { continue };
                let Some(&num) = nums.get(&id) else { continue };
                let mut text = String::new();
                walk_paragraphs(&s.blocks, &mut |p| {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(p.plain_text().trim());
                });
                let mut text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if text.chars().count() > 80 {
                    text = text.chars().take(80).collect::<String>().trim_end().to_string() + "\u{2026}";
                }
                list.push(XNote { id, num, label: fmt.label(num), text });
            }
            list.sort_by_key(|n| n.num);
        }
        out
    }
}
