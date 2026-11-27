//! The content control and form field ops: listing, inserting, changing and removing controls.

use super::*;

impl Document {
    /// The content controls of the document, for the AI tools and scripts: paragraph, number, kind, name,
    /// value and state.
    pub fn list_controls(&self) -> Vec<(Option<NodeId>, RControl, String)> {
        let mut out = Vec::new();
        fn blocks<'a>(bs: impl IntoIterator<Item = &'a Block>, out: &mut Vec<(Option<NodeId>, RControl, String)>) {
            for b in bs {
                match b {
                    Block::Para(p) => {
                        for (n, (c, s, e)) in controls::sdts_in(&p.inlines).into_iter().enumerate() {
                            let v = controls::inline_view(c, n, s, e);
                            if v.kind != "citation" {
                                out.push((Some(p.id), v, controls::inline_text(&c.children)));
                            }
                        }
                    }
                    Block::Container(c) => {
                        if let Some(v) = controls::block_view(c) {
                            out.push((None, v, controls::blocks_text(&c.blocks)));
                        }
                        blocks(&c.blocks, out);
                    }
                    Block::Table(t) => {
                        for r in t.rows() {
                            for c in r.cells() {
                                blocks(&c.blocks, out);
                            }
                        }
                    }
                    Block::Opaque(_) => {}
                }
            }
        }
        blocks(&self.body, &mut out);
        out
    }

    /// Declare the namespace of Word 2010/2013 elements on the part's root (and in `mc:Ignorable`).
    pub(super) fn declare_word_ns(&mut self, prefix: &str, uri: &str) {
        self.ensure_ns(prefix, uri);
        if self.active_part != self.main_path {
            return;
        }
        let root = &mut self.doc_xml.root;
        if root.attr("xmlns:mc").is_none() {
            return;
        }
        let ign = root.attr("mc:Ignorable").unwrap_or("").to_string();
        if !ign.split_whitespace().any(|p| p == prefix) {
            let v = if ign.is_empty() { prefix.to_string() } else { format!("{ign} {prefix}") };
            root.set_attr("mc:Ignorable", v);
        }
    }

    // -----------------------------------------------------------------------
    // The control ops
    // -----------------------------------------------------------------------

    /// Placeholder text carries the `PlaceholderText` character style: make sure the document defines it wherever a
    /// placeholder is written (the control ops, the gallery designs, the clean-up after an edit).
    pub(in crate::document) fn ensure_placeholder_style(&mut self) {
        // Not while a document opens: what opening shows is not an edit, and the styles stay as the file has them.
        if self.opening {
            return;
        }
        let _ = self.styles.ensure_builtin("PlaceholderText");
    }

    pub(in crate::document) fn op_control(&mut self, op: &ControlOp) -> Result<OpResult, EditError> {
        if !matches!(op, ControlOp::Remove { .. }) {
            self.ensure_placeholder_style();
        }
        match op {
            ControlOp::Insert { at, to, kind, props, block } => self.control_insert(*at, *to, kind, props, *block),
            ControlOp::Props { target, props } => self.control_props(target, props),
            ControlOp::Set { target, text, checked, index, date, picture } => self.control_set(target, text.as_deref(), *checked, *index, date.as_deref(), picture.as_ref()),
            ControlOp::Remove { target, delete_content } => self.control_remove(target, *delete_content),
            ControlOp::Repeat { target, remove } => self.control_repeat(target, *remove),
        }
    }

    pub(super) fn control_insert(&mut self, at: Pos, to: Option<Pos>, kind: &str, props: &ControlProps, block: bool) -> Result<OpResult, EditError> {
        if !controls::KINDS.contains(&kind) {
            return Err(EditError::Invalid(format!("unknown content control kind '{kind}'")));
        }
        let local_ns = self.active_part != self.main_path;
        if kind == "checkbox" {
            self.declare_word_ns("w14", controls::NS_W14);
        }
        if kind == "repeating" || props.color.is_some() || props.appearance.as_deref().is_some_and(|a| a != "boundingBox") {
            self.declare_word_ns("w15", controls::NS_W15);
        }
        let id = self.next_sdt_id();
        let mut shell = controls::new_shell(kind, id, props, props.placeholder.as_deref(), local_ns).map_err(EditError::Invalid)?;
        let main = self.main_path.clone();
        // Placeholder text, and the building block that holds it.
        let (def_name, def_text) = default_placeholder(kind);
        let text = props.placeholder.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| def_text.to_string());
        let preferred = (text == def_text).then_some(def_name);
        let ph_name = if kind == "checkbox" || kind == "repeating" || kind == "group" { None } else { Some(controls::ensure_placeholder(&mut self.pkg, &main, &text, preferred)) };
        self.touch(at.p.0);

        if kind == "repeating" {
            return self.insert_repeating(at, shell, id);
        }
        // Content: the selection, a ticked-off box, or the placeholder.
        let wraps_selection = to.is_some_and(|t| t.p == at.p && t.o != at.o) && matches!(kind, "text" | "richText" | "group");
        if block {
            let (mut list, i) = self.body.parent_mut(at.p.0).ok_or(EditError::NotFound(at.p.0))?;
            if !matches!(list[i], Block::Para(_)) {
                return Err(EditError::NotParagraph(at.p.0));
            }
            let para = list.remove(i);
            let mut blocks = vec![para];
            if !wraps_selection && ph_name.is_some() {
                let Block::Para(p) = &mut blocks[0] else { unreachable!() };
                if p.len16() == 0 {
                    p.inlines = vec![controls::text_run(&text, None, true)];
                    if let Some(n) = &ph_name {
                        controls::set_placeholder(shell.child_mut("w:sdtPr").expect("pr"), n, true);
                    }
                }
            }
            list.insert(i, Block::Container(BlockContainer { id: self.ids.fresh(), shell, content_in: Some("w:sdtContent".into()), blocks }));
            // The paragraph is inside a new block now: draw them all again.
            self.touch_all();
            return Ok(OpResult { caret: Some(at), ..Default::default() });
        }
        let children: Vec<Inline>;
        let pr = shell.child_mut("w:sdtPr").expect("pr");
        let mut selection: Option<(usize, usize)> = None;
        if kind == "checkbox" {
            let v = controls::read_props(pr);
            let sym = v.unchecked_symbol.unwrap_or(controls::Symbol { val: "2610".into(), font: "MS Gothic".into() });
            children = vec![controls::text_run(&sym.ch().to_string(), Some(controls::with_font(None, &sym.font)), false)];
        } else if wraps_selection {
            let t = to.expect("checked");
            let (s, e) = (at.o.min(t.o), at.o.max(t.o));
            selection = Some((s, e));
            children = Vec::new();
        } else if kind == "group" {
            children = vec![controls::text_run("", None, false)];
        } else {
            if let Some(n) = &ph_name {
                controls::set_placeholder(pr, n, true);
            }
            children = vec![controls::text_run(&text, None, true)];
        }
        let para = self.para_mut(at.p.0)?;
        let len = para.len16();
        if let Some((s, e)) = selection {
            let (s, e) = (s.min(len), e.min(len));
            edit::split_at(&mut para.inlines, s);
            edit::split_at(&mut para.inlines, e);
            let (mut first, mut last, mut pos, mut covered) = (None, 0, 0, 0);
            for (i, inl) in para.inlines.iter().enumerate() {
                let l = inl.len16();
                if pos >= s && pos + l <= e && l > 0 {
                    first.get_or_insert(i);
                    last = i;
                    covered += l;
                }
                pos += l;
            }
            if covered != e - s || first.is_none() {
                return Err(EditError::Invalid("a content control can't start inside another one or a link here".into()));
            }
            let kids: Vec<Inline> = para.inlines.drain(first.unwrap()..=last).collect();
            para.inlines.insert(first.unwrap(), Inline::Container(InlineContainer { kind: InlineKind::Sdt, shell, content_in: Some("w:sdtContent".into()), children: kids }));
            return Ok(OpResult { caret: Some(Pos::new(at.p.0, s)), ..Default::default() });
        }
        let off = at.o.min(len);
        edit::insert_inline_at(&mut para.inlines, off, Inline::Container(InlineContainer { kind: InlineKind::Sdt, shell, content_in: Some("w:sdtContent".into()), children }));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, if kind == "checkbox" { off + 1 } else { off })), ..Default::default() })
    }

    /// A repeating section around the paragraph at `at`, with one item holding it.
    pub(super) fn insert_repeating(&mut self, at: Pos, shell: Element, id: i64) -> Result<OpResult, EditError> {
        let local_ns = self.active_part != self.main_path;
        let item_shell = {
            let mut pr = Element::new("w:sdtPr").with_child(Element::new("w:id").with_attr("w:val", (id + 1).to_string()));
            let mut ri = Element::new("w15:repeatingSectionItem");
            if local_ns {
                ri.set_attr("xmlns:w15", controls::NS_W15);
            }
            crate::props::put_ordered(&mut pr, ri, controls::SDTPR_ORDER);
            Element::new("w:sdt").with_child(pr).with_child(Element::new("w:sdtContent"))
        };
        let (mut list, i) = self.body.parent_mut(at.p.0).ok_or(EditError::NotFound(at.p.0))?;
        if !matches!(list[i], Block::Para(_)) {
            return Err(EditError::NotParagraph(at.p.0));
        }
        let para = list.remove(i);
        let item = Block::Container(BlockContainer { id: self.ids.fresh(), shell: item_shell, content_in: Some("w:sdtContent".into()), blocks: vec![para] });
        list.insert(i, Block::Container(BlockContainer { id: self.ids.fresh(), shell, content_in: Some("w:sdtContent".into()), blocks: vec![item] }));
        self.touch_all();
        Ok(OpResult { caret: Some(at), ..Default::default() })
    }

    pub(super) fn control_props(&mut self, target: &ControlRef, props: &ControlProps) -> Result<OpResult, EditError> {
        let local_ns = self.active_part != self.main_path;
        if props.color.is_some() || props.appearance.is_some() || props.section_title.is_some() {
            self.declare_word_ns("w15", controls::NS_W15);
        }
        if props.checked_symbol.is_some() || props.unchecked_symbol.is_some() {
            self.declare_word_ns("w14", controls::NS_W14);
        }
        // A new placeholder text goes into the glossary first.
        let main = self.main_path.clone();
        let ph = props.placeholder.as_ref().map(|t| controls::ensure_placeholder(&mut self.pkg, &main, t, None));
        let ph_text = props.placeholder.clone();
        let pkg = &self.pkg;
        let main2 = self.main_path.clone();
        let props = props.clone();
        // Rows have their own addressing; everything else is a block or inline control.
        if target.wrap.is_some() {
            return self.wrap_props(target, &props);
        }
        let r = {
            let mut tmp = Document::detached_control(&mut self.body);
            tmp.apply_props(target, &props, ph.as_deref(), ph_text.as_deref(), local_ns, pkg, &main2)
        };
        if let Some(id) = target.p.or(target.block) {
            self.touch(id.0);
        }
        r.map(|_| OpResult::default())
    }

    /// A stand-in that lets a control be changed while `pkg` is read: it only needs the blocks.
    pub(super) fn detached_control(body: &mut Body) -> Detached<'_> {
        Detached { body }
    }

    pub(super) fn control_set(&mut self, target: &ControlRef, text: Option<&str>, checked: Option<bool>, index: Option<usize>, date: Option<&str>, picture: Option<&controls::PictureValue>) -> Result<OpResult, EditError> {
        // A picture needs the relationship and a drawing before the control is touched.
        let drawing = match picture {
            Some(p) => {
                if !self.pkg.has(&p.media) {
                    return Err(EditError::Invalid(format!("media '{}' not found; add it first", p.media)));
                }
                let target_rel = relative_target(&self.active_part, &p.media);
                let part = self.active_part.clone();
                let rid = self.pkg.add_rel(&part, package::REL_IMAGE, &target_rel, false);
                self.ensure_ns("r", NS_R);
                self.ensure_ns("wp", NS_WP);
                self.drawing_id += 1;
                Some(drawing_xml(&rid, p.width, p.height, self.drawing_id, p.alt.as_deref().unwrap_or("")))
            }
            None => None,
        };
        let pkg = &self.pkg;
        let main = self.main_path.clone();
        let r = {
            let mut tmp = Document::detached_control(&mut self.body);
            tmp.apply_set(target, text, checked, index, date, drawing, pkg, &main)
        };
        if let Some(id) = target.p.or(target.block) {
            self.touch(id.0);
        }
        r.map(|_| OpResult::default())
    }

    pub(super) fn control_remove(&mut self, target: &ControlRef, delete_content: bool) -> Result<OpResult, EditError> {
        if let Some(id) = target.block {
            self.touch(id.0);
            let (mut list, i) = self.body.parent_mut(id.0).ok_or(EditError::NotFound(id.0))?;
            let Block::Container(c) = &list[i] else { return Err(EditError::Invalid("not a content control".into())) };
            let v = c.shell.child("w:sdtPr").map(controls::read_props).unwrap_or_default();
            if controls::locks_control(v.lock.as_deref()) {
                return Err(EditError::Invalid("this content control is locked and can't be deleted".into()));
            }
            let Block::Container(c) = list.remove(i) else { unreachable!() };
            if !delete_content {
                for (k, b) in c.blocks.into_iter().enumerate() {
                    list.insert(i + k, b);
                }
            } else if list.is_empty() {
                list.push(Block::Para(Paragraph::new(self.ids.fresh())));
            }
            // Its blocks stand on their own now: draw them all again.
            self.touch_all();
            return Ok(OpResult::default());
        }
        let p = target.p.ok_or_else(|| EditError::Invalid("say which content control".into()))?;
        self.touch(p.0);
        let para = self.para_mut(p.0)?;
        let mut seen = 0usize;
        let ok = remove_sdt(&mut para.inlines, target.n, &mut seen, delete_content)?;
        if !ok {
            return Err(EditError::Invalid(format!("paragraph {} has no content control number {}", p.0, target.n)));
        }
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Repeating sections
    // -----------------------------------------------------------------------

    pub(in crate::document) fn op_form(&mut self, op: &FormOp) -> Result<OpResult, EditError> {
        match op {
            FormOp::Insert { at, kind, props } => {
                let n = forms::find(&self.para_ref(at.p.0)?.inlines).len() + 1;
                let name = props.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| match kind.as_str() {
                    "text" => format!("Text{n}"),
                    "checkbox" => format!("Check{n}"),
                    _ => format!("Dropdown{n}"),
                });
                let inlines = forms::build(kind, props, &name).map_err(EditError::Invalid)?;
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                let off = at.o.min(p.len16());
                forms::insert_at(&mut p.inlines, off, inlines);
                Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() })
            }
            FormOp::Props { at, n, props } => {
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                let ff = forms::ff_mut(&mut p.inlines, *n).ok_or_else(|| EditError::Invalid(format!("paragraph {} has no form field number {n}", at.p.0)))?;
                forms::patch(ff, props).map_err(EditError::Invalid)?;
                // A new default shows in an empty text field.
                if let Some(d) = &props.default {
                    let v = forms::views(&p.inlines);
                    if let Some(f) = v.get(*n).filter(|f| f.kind == "text") {
                        let current: String = text_between(&p.inlines, f.start, f.end);
                        if current.trim_matches('\u{2002}').is_empty() {
                            let text = if d.is_empty() { forms::EMPTY_RESULT.to_string() } else { d.clone() };
                            replace_form_text(&mut p.inlines, f.at, &text);
                        }
                    }
                }
                Ok(OpResult::default())
            }
            FormOp::Set { at, n, checked, index, text } => {
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                let views = forms::views(&p.inlines);
                let f = views.get(*n).ok_or_else(|| EditError::Invalid(format!("paragraph {} has no form field number {n}", at.p.0)))?.clone();
                if f.disabled {
                    return Err(EditError::Invalid("this form field is switched off".into()));
                }
                match f.kind.as_str() {
                    "checkbox" => {
                        let on = checked.ok_or_else(|| EditError::Invalid("a check box needs checked".into()))?;
                        forms::set_checked(forms::ff_mut(&mut p.inlines, *n).expect("found"), on);
                    }
                    "dropdown" => {
                        let i = index.ok_or_else(|| EditError::Invalid("a drop-down needs an entry number".into()))?;
                        if i >= f.items.len() {
                            return Err(EditError::Invalid(format!("the drop-down has {} entries", f.items.len())));
                        }
                        forms::set_selected(forms::ff_mut(&mut p.inlines, *n).expect("found"), i);
                    }
                    _ => {
                        let t = text.as_ref().ok_or_else(|| EditError::Invalid("a text field needs text".into()))?;
                        let mut t = match &f.format {
                            Some(fm) => forms::apply_format(t, fm),
                            None => t.clone(),
                        };
                        if let Some(max) = f.max_length {
                            t = t.chars().take(max as usize).collect();
                        }
                        if t.is_empty() {
                            t = forms::EMPTY_RESULT.to_string();
                        }
                        replace_form_text(&mut p.inlines, f.at, &t);
                    }
                }
                Ok(OpResult::default())
            }
        }
    }

    // -----------------------------------------------------------------------
    // After any edit
    // -----------------------------------------------------------------------

    pub(super) fn wrap_props(&mut self, target: &ControlRef, props: &ControlProps) -> Result<OpResult, EditError> {
        let wrap = target.wrap.expect("checked").0;
        let local_ns = self.active_part != self.main_path;
        let mut done = false;
        let mut result = Ok(());
        let mut tid = None;
        // `f` says whether it changed the table; true when any did.
        fn each_table(blocks: &mut [Block], f: &mut dyn FnMut(&mut Table) -> bool) -> bool {
            let mut any = false;
            for b in blocks.iter_mut() {
                match b {
                    Block::Table(t) => {
                        any |= f(t);
                        for r in t.rows_mut() {
                            for c in r.cells_mut() {
                                any |= each_table(&mut c.blocks, f);
                            }
                        }
                    }
                    Block::Container(c) => any |= each_table(&mut c.blocks, f),
                    _ => {}
                }
            }
            any
        }
        // A block that holds a table is copied from an undo step only when a table in it is changed.
        let body = &mut self.body;
        for i in 0..body.len() {
            body.try_edit(i, |b| {
                each_table(std::slice::from_mut(b), &mut |t| {
                    let mut hit = false;
                    let mut one = |w: &mut SdtWrap| {
                        if w.id == wrap {
                            if let Some(pr) = w.shell.child_mut("w:sdtPr") {
                                if let Err(e) = controls::patch_props(pr, props, local_ns) {
                                    result = Err(e);
                                }
                                hit = true;
                            }
                        }
                    };
                    for r in t.rows_mut() {
                        r.sdts.iter_mut().for_each(&mut one);
                        for c in r.cells_mut() {
                            c.sdts.iter_mut().for_each(&mut one);
                        }
                    }
                    if hit {
                        done = true;
                        tid = Some(t.id);
                    }
                    hit
                })
            });
        }
        result.map_err(EditError::Invalid)?;
        if !done {
            return Err(EditError::Invalid("no table row or cell has that content control".into()));
        }
        if let Some(t) = tid {
            self.touch(t);
        }
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Legacy form fields
    // -----------------------------------------------------------------------
}
