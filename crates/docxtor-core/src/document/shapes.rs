//! Shapes and text boxes: inserting and changing them, reading one, and keeping the text of text boxes (which
//! lives in stories) in step with the markup of the shapes that hold it. See `crate::shapes`.

use super::*;
use crate::shapes::build::{self, LineLook, Spec};
use crate::shapes::story;
use crate::shapes::RShape;
use std::collections::HashSet;

/// The inline object occupying `[off, off+1)`.
pub(super) fn object_at(inlines: &[Inline], off: usize) -> Option<&Element> {
    fn go<'a>(inlines: &'a [Inline], off: usize, start: &mut usize) -> Option<&'a Element> {
        for inl in inlines {
            let len = inl.len16();
            if off >= *start && off < *start + len {
                return match inl {
                    Inline::Run(r) => {
                        let mut pos = *start;
                        for item in &r.items {
                            let l = item.len16();
                            if pos == off && l == 1 {
                                if let RunItem::Object(e) = item {
                                    return Some(e);
                                }
                            }
                            pos += l;
                        }
                        None
                    }
                    Inline::Container(c) => go(&c.children, off, start),
                    Inline::Opaque(_) => None,
                };
            }
            *start += len;
        }
        None
    }
    go(inlines, off, &mut 0)
}

impl Document {
    /// The shape whose object sits at a position (the object occupies `[o, o+1)`), as the renderer sees it.
    pub fn shape_at(&self, at: Pos) -> Option<RShape> {
        let (blocks, part) = self.locate(at.p.0);
        let p = find_para(blocks, at.p.0)?;
        let e = object_at(&p.inlines, at.o)?;
        let labels = HashMap::new();
        let notes = self.note_state();
        let ctx = self.story_ctx(part, &labels, &notes);
        ctx.shape(e)
    }

    /// Hand a render context the text of every text box.
    pub(super) fn attach_shape_texts<'a>(&'a self, ctx: &mut RenderCtx<'a>) {
        for s in &self.stories {
            if let Some(k) = story::key_of(s) {
                ctx.shapes.texts.insert(k.to_string(), &s.blocks);
            }
        }
    }

    pub(super) fn apply_shape_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::InsertShape { at, geometry, w, h, fill, line, text, floating, x, y } => {
                self.op_insert_shape(*at, geometry, *w, *h, fill.as_deref(), line.as_ref(), text.as_deref(), *floating, *x, *y, false)
            }
            Op::InsertTextBox { at, w, h, text, floating, x, y } => self.op_insert_shape(*at, "rect", *w, *h, None, None, Some(text), *floating, *x, *y, true),
            Op::SetShape { at, patch } => self.op_set_shape(*at, patch),
            _ => unreachable!("not a shape op"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn op_insert_shape(
        &mut self,
        at: Pos,
        geometry: &str,
        w: f64,
        h: f64,
        fill: Option<&str>,
        line: Option<&crate::shapes::LineSpec>,
        text: Option<&str>,
        floating: bool,
        x: Option<f64>,
        y: Option<f64>,
        text_box: bool,
    ) -> Result<OpResult, EditError> {
        if self.active_part != self.main_path {
            return Err(EditError::Invalid("that can only be inserted in the main text".into()));
        }
        let invalid = EditError::Invalid;
        let geometry = if text_box { "rect" } else { build::preset(geometry).map_err(invalid)? };
        let is_line = build::is_line(geometry);
        if !(1.0..=1584.0).contains(&w) || !(if is_line { 0.0..=1584.0 } else { 1.0..=1584.0 }).contains(&h) {
            return Err(EditError::Invalid("a shape's width and height must be between 1 and 1584 points".into()));
        }
        // Word's looks: a text box is a white rectangle with a thin black line; a shape is a light fill with a darker line.
        let default_fill = if text_box { Some("FFFFFF".to_string()) } else if is_line { None } else { Some("DAE3F3".to_string()) };
        let fill = match fill {
            Some(f) => build::fill_hex(f).map_err(EditError::Invalid)?,
            None => default_fill,
        };
        let default_line = if text_box {
            Some(LineLook::default_for(true))
        } else if is_line {
            Some(LineLook { color: "000000".into(), width_pt: 1.5, ..Default::default() })
        } else {
            Some(LineLook { color: "2F528F".into(), width_pt: 1.0, ..Default::default() })
        };
        let line = build::line_look(line, default_line).map_err(EditError::Invalid)?;
        self.drawing_id += 1;
        let spec = Spec {
            geometry: geometry.to_string(),
            w,
            h,
            fill,
            line,
            text: if text_box { Some(text.unwrap_or_default().to_string()) } else { text.filter(|t| !t.is_empty()).map(String::from) },
            text_box,
            floating,
            x: x.unwrap_or(0.0),
            y: y.unwrap_or(0.0),
            id: self.drawing_id,
        };
        let mut el = build::build(&spec);
        build::ensure_root_ns(&mut self.doc_xml.root);
        let mut new = story::new_stories(&mut el, &self.main_path.clone(), &mut self.ids).unwrap_or_default();
        self.story_gen += 1;
        for s in &mut new {
            s.gen = self.story_gen;
        }
        let mut created = Vec::new();
        for s in &new {
            walk_paragraphs(&s.blocks, &mut |p| created.push(p.id));
        }
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
        edit::insert_inline_at(&mut p.inlines, off, Inline::Run(run));
        self.stories.extend(new);
        let caret = if text_box { created.first().map(|c| Pos::new(*c, 0)) } else { None }.unwrap_or_else(|| Pos::new(at.p.0, off + 1));
        Ok(OpResult { caret: Some(caret), created, ..Default::default() })
    }

    fn op_set_shape(&mut self, at: Pos, patch: &crate::shapes::patch::ShapePatch) -> Result<OpResult, EditError> {
        let cur = self.shape_at(at).ok_or_else(|| EditError::Invalid("there is no shape at that position".into()))?;
        // A picture fill points at the picture with a relationship of the part the shape is in.
        let mut rid = None;
        if let Some(crate::shapes::FillSpec::Obj(crate::shapes::appearance::FillObj::Picture { media, .. })) = &patch.fill {
            if !self.pkg.has(media) {
                return Err(EditError::Invalid(format!("picture {media} isn't in the document (add it with add_media)")));
            }
            // The part the shape is in: the body, or a header, footer or note.
            let part = self.locate(at.p.0).1.to_string();
            let target = relative_target(&part, media);
            rid = Some(self.pkg.add_rel(&part, package::REL_IMAGE, &target, false));
            let root = if part == self.main_path { Some(&mut self.doc_xml.root) } else { self.story_parts.get_mut(&part).map(|d| &mut d.root) };
            if let Some(root) = root.filter(|r| r.attr("xmlns:r").is_none()) {
                root.set_attr("xmlns:r", NS_R);
            }
        }
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let el = edit::object_at_mut(&mut p.inlines, at.o).ok_or_else(|| EditError::Invalid("there is no shape at that position".into()))?;
        crate::shapes::patch::apply_with(el, patch, &cur, rid.as_deref()).map_err(EditError::Invalid)?;
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }

    // -----------------------------------------------------------------------
    // Keeping text boxes and their shapes in step
    // -----------------------------------------------------------------------

    /// The generation of every text-box story, to tell afterwards which ones an op batch changed.
    pub(super) fn text_box_gens(&self) -> Vec<(String, u64)> {
        self.stories.iter().filter_map(|s| story::key_of(s).map(|k| (k.to_string(), s.gen))).collect()
    }

    /// Text boxes that came into the document as markup (a copied or duplicated shape, pasted text, a merged
    /// fragment) have no story yet: give each one, so what the engine shows is what the file will show.
    pub(super) fn adopt_text_boxes(&mut self) {
        let before = self.stories.len();
        let main = self.main_path.clone();
        // A body with no shape has no text box to adopt: only the stories are looked at.
        let body = self.body_has_shapes().then_some(&mut self.body);
        story::load_in_body(&main, body, &mut self.stories, &mut self.ids);
        if self.stories.len() > before {
            self.story_gen += 1;
            for s in &mut self.stories[before..] {
                s.gen = self.story_gen;
            }
            self.stories_touched = true;
        }
    }

    /// After a batch of ops: write the text of every text box that changed back into the markup of its shape
    /// (the DrawingML and the VML fallback), and forget text boxes whose shape is gone.
    pub(super) fn sync_text_boxes(&mut self, before: &[(String, u64)]) {
        if !self.stories.iter().any(|s| s.kind == StoryKind::TextBox) {
            return;
        }
        // A text box inside a text box takes another pass to reach the body.
        for _ in 0..4 {
            let jobs: Vec<(String, Vec<Node>)> = self
                .stories
                .iter()
                .filter_map(|s| {
                    let key = story::key_of(s)?;
                    let was = before.iter().find(|(k, _)| k == key).map(|(_, g)| *g);
                    if was == Some(s.gen) {
                        return None;
                    }
                    let mut nodes = Vec::new();
                    write_blocks(&s.blocks, &mut nodes);
                    Some((key.to_string(), nodes))
                })
                .collect();
            if jobs.is_empty() {
                break;
            }
            let mut nested = false;
            for (key, nodes) in &jobs {
                let (owners, changed) = story::write_back_body(&mut self.body, key, nodes);
                if changed {
                    self.body_dirty = true;
                }
                // The paragraph that holds the shape is drawn again even when the shape's own markup is the same: an
                // edit inside the text box can live in another part (a chart's data, a picture) that its markup only names.
                for pid in owners {
                    self.touch(pid);
                }
                for i in 0..self.stories.len() {
                    if story::key_of(&self.stories[i]) == Some(key.as_str()) {
                        continue;
                    }
                    let (owners, changed) = story::write_back(&mut self.stories[i].blocks, key, nodes);
                    if owners.is_empty() {
                        continue;
                    }
                    let is_box = self.stories[i].kind == StoryKind::TextBox;
                    // A box holding the shape is written into the part that holds it on the next pass, changed or not.
                    if changed || is_box {
                        self.story_gen += 1;
                        self.stories[i].gen = self.story_gen;
                    }
                    if is_box {
                        nested = true;
                    } else {
                        if changed {
                            self.dirty_parts.insert(self.stories[i].part.clone());
                        }
                        self.stories_touched = true;
                    }
                }
            }
            if !nested {
                break;
            }
        }
        // Forget text boxes no shape holds any more (their shape was deleted).
        let mut live = HashSet::new();
        story::live_keys(&self.body, &mut live);
        for s in &self.stories {
            story::live_keys(&s.blocks, &mut live);
        }
        self.stories.retain(|s| story::key_of(s).is_none_or(|k| live.contains(k)));
    }
}
