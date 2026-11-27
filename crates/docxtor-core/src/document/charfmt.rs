//! Character formatting ops that reach beyond a patch on a range: changing case, the document's default
//! character formatting, and character styles (found by name, linked to paragraph styles).

use super::*;
use crate::charfmt::{change_case, CaseMode};

/// The character style (`w:rStyle`) of the text at an offset: the run holding the character before it, or the one
/// that starts there.
pub(super) fn char_style_at(p: &Paragraph, off: usize) -> Option<String> {
    fn go(inlines: &[Inline], off: usize, start: &mut usize, found: &mut Option<Option<String>>, first: &mut Option<Option<String>>) {
        for i in inlines {
            match i {
                Inline::Run(r) => {
                    let len = r.len16();
                    let style = r.rpr.as_ref().and_then(|x| x.child("w:rStyle")).and_then(|s| s.attr("w:val")).map(String::from);
                    if len > 0 {
                        if first.is_none() {
                            *first = Some(style.clone());
                        }
                        if *start < off && off <= *start + len {
                            *found = Some(style);
                        }
                    }
                    *start += len;
                }
                Inline::Container(c) => go(&c.children, off, start, found, first),
                Inline::Opaque(_) => *start += i.len16(),
            }
        }
    }
    let (mut found, mut first) = (None, None);
    go(&p.inlines, off, &mut 0, &mut found, &mut first);
    found.or(if off == 0 { first } else { None }).flatten()
}

/// The text of a paragraph as one entry per character (None for tabs, breaks and objects), with the UTF-16 offset
/// where each starts.
fn flat_chars(inlines: &[Inline], chars: &mut Vec<Option<char>>, at: &mut Vec<usize>, pos: &mut usize) {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                for item in &r.items {
                    match item {
                        RunItem::Text(t) => {
                            for c in t.chars() {
                                chars.push(Some(c));
                                at.push(*pos);
                                *pos += c.len_utf16();
                            }
                        }
                        other => {
                            let l = other.len16();
                            if l > 0 {
                                chars.push(None);
                                at.push(*pos);
                                *pos += l;
                            }
                        }
                    }
                }
            }
            Inline::Container(c) => flat_chars(&c.children, chars, at, pos),
            Inline::Opaque(_) => {
                let l = i.len16();
                if l > 0 {
                    chars.push(None);
                    at.push(*pos);
                    *pos += l;
                }
            }
        }
    }
}

/// Rewrite the text of the runs inside `[s, e)` from `new` (one entry per character of the paragraph).
fn rewrite(inlines: &mut [Inline], base: usize, s: usize, e: usize, at: &[usize], new: &[Option<String>]) {
    let mut start = base;
    for inl in inlines.iter_mut() {
        let len = inl.len16();
        if len > 0 && start >= s && start + len <= e {
            match inl {
                Inline::Run(r) => {
                    let mut idx = at.binary_search(&start).unwrap_or_else(|i| i);
                    for item in r.items.iter_mut() {
                        match item {
                            RunItem::Text(t) => {
                                let mut out = String::with_capacity(t.len());
                                for c in t.chars() {
                                    match new.get(idx).and_then(|n| n.as_ref()) {
                                        Some(n) => out.push_str(n),
                                        None => out.push(c),
                                    }
                                    idx += 1;
                                }
                                *t = out;
                            }
                            other => {
                                if other.len16() > 0 {
                                    idx += 1;
                                }
                            }
                        }
                    }
                }
                Inline::Container(c) => rewrite(&mut c.children, start, s, e, at, new),
                Inline::Opaque(_) => {}
            }
        } else if len > 0 && start < e && start + len > s {
            if let Inline::Container(c) = inl {
                rewrite(&mut c.children, start, s, e, at, new);
            }
        }
        start += len;
    }
}

impl Document {
    /// A character style by id or name for `w:rStyle`. Word's built-in character styles are added when the
    /// file lacks them; a paragraph style that is linked to a character style stands for that one.
    pub(super) fn resolve_char_style(&mut self, name: &str) -> Result<String, EditError> {
        let id = self.resolve_style(name)?;
        match self.styles.get(&id).map(|i| i.kind.clone()).as_deref() {
            Some("character") => Ok(id),
            Some("paragraph") => self.styles.linked_char_style(&id).ok_or_else(|| EditError::Invalid(format!("'{name}' is a paragraph style, not a character style"))),
            _ => Err(EditError::Invalid(format!("'{name}' is not a character style"))),
        }
    }

    pub(super) fn op_change_case(&mut self, from: Pos, to: Pos, mode: CaseMode) -> Result<OpResult, EditError> {
        let (from, to) = self.ordered(from, to)?;
        let order = paragraph_order(&self.body);
        let a = order.iter().position(|x| *x == from.p.0).unwrap();
        let b = order.iter().position(|x| *x == to.p.0).unwrap();
        for (k, pid) in order[a..=b].iter().enumerate() {
            // Looked at first: a paragraph the case does not change is not copied from an undo step.
            let (s, e, at, new) = {
                let (blocks, part) = self.locate(*pid);
                let p = find_para(blocks, *pid).ok_or(EditError::NotFound(*pid))?;
                let mut ctx = self.render_ctx(part, &self.last_labels);
                let lang = ctx.para(p).lang;
                let len = p.len16();
                let s = if k == 0 { from.o.min(len) } else { 0 };
                let e = if a + k == b { to.o.min(len) } else { len };
                if e <= s {
                    continue;
                }
                let (mut chars, mut at, mut pos) = (Vec::new(), Vec::new(), 0);
                flat_chars(&p.inlines, &mut chars, &mut at, &mut pos);
                // The characters of the range, as entry indexes.
                let first = at.partition_point(|u| *u < s);
                let last = at.partition_point(|u| *u < e);
                let new = change_case(&chars, first, last, mode, lang.as_deref());
                (s, e, at, new)
            };
            if new.iter().all(|n| n.is_none()) {
                continue;
            }
            let p = self.para_mut(*pid)?;
            edit::split_at(&mut p.inlines, s);
            edit::split_at(&mut p.inlines, e);
            rewrite(&mut p.inlines, 0, s, e, &at, &new);
            self.touch(*pid);
        }
        Ok(OpResult::default())
    }

    pub(super) fn op_set_default_format(&mut self, patch: &RunPatch) -> Result<OpResult, EditError> {
        let mut patch = patch.clone();
        if patch.style.is_some() {
            return Err(EditError::Invalid("the default formatting can't name a character style".into()));
        }
        if patch.ext.fit_text.is_some_and(|w| w > 0) {
            return Err(EditError::Invalid("text can't be fitted to a width by default".into()));
        }
        patch.clear = false;
        let styles = self.styles.doc.get_or_insert_with(|| XmlDoc::new(Element::new("w:styles").with_attr("xmlns:w", NS_W)));
        let before = styles.clone();
        let root = &mut styles.root;
        if root.child("w:docDefaults").is_none() {
            // `w:docDefaults` is the first child of `w:styles`.
            let first = root.children.iter().position(|n| matches!(n, Node::Element(_))).unwrap_or(root.children.len());
            root.children.insert(first, Node::Element(Element::new("w:docDefaults")));
        }
        let dd = root.child_mut("w:docDefaults").expect("just created");
        if dd.child("w:rPrDefault").is_none() {
            crate::props::put_ordered(dd, Element::new("w:rPrDefault"), &["w:rPrDefault", "w:pPrDefault"]);
        }
        let def = dd.child_mut("w:rPrDefault").expect("just created");
        if def.child("w:rPr").is_none() {
            def.children.push(Node::Element(Element::new("w:rPr")));
        }
        let rpr = def.child_mut("w:rPr").expect("just created");
        let inherited = RunFmt::default();
        patch.apply(rpr, &inherited);
        if *styles != before {
            self.styles.reload();
            // Every paragraph that doesn't say otherwise looks different now.
            self.redraw_all();
        }
        Ok(OpResult::default())
    }
}
