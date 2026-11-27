//! Editing controls on blocks that are not in the document yet (a glossary entry, a copy), and removing a control.

use super::*;

/// The part of `Document` that edits controls while a package is only read: just the blocks.
pub(super) struct Detached<'a> {
    pub(super) body: &'a mut Body,
}

impl Detached<'_> {
    pub(super) fn find<'b>(&'b mut self, r: &ControlRef) -> Result<Ctl<'b>, EditError> {
        if let Some(id) = r.block {
            let (list, i) = self.body.parent_mut(id.0).ok_or(EditError::NotFound(id.0))?;
            return match list.into_get_mut(i) {
                Some(Block::Container(c)) if c.shell.child("w:sdtPr").is_some() => Ok(Ctl::Block(c)),
                _ => Err(EditError::Invalid(format!("block {} is not a content control", id.0))),
            };
        }
        let p = r.p.ok_or_else(|| EditError::Invalid("say which content control: p and n, or block".into()))?;
        let para = self.body.para_mut(p.0).ok_or(EditError::NotFound(p.0))?;
        let c = controls::sdt_mut(&mut para.inlines, r.n).ok_or_else(|| EditError::Invalid(format!("paragraph {} has no content control number {}", p.0, r.n)))?;
        Ok(Ctl::Inline(c))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_props(&mut self, r: &ControlRef, props: &ControlProps, ph_name: Option<&str>, ph_text: Option<&str>, local_ns: bool, _pkg: &crate::package::Package, _main: &str) -> Result<(), EditError> {
        let mut ctl = self.find(r)?;
        let mut p = props.clone();
        p.placeholder = None;
        {
            let pr = ctl.pr().ok_or_else(|| EditError::Invalid("not a content control".into()))?;
            controls::patch_props(pr, &p, local_ns).map_err(EditError::Invalid)?;
        }
        let v = ctl.view();
        if let (Some(name), Some(text)) = (ph_name, ph_text) {
            if let Some(pr) = ctl.pr() {
                controls::set_placeholder(pr, name, v.placeholder);
            }
            if v.placeholder {
                let t = text.to_string();
                ctl.set_text(&t, true, None);
            }
        }
        // A check box shows the symbol of its state.
        if v.kind == "checkbox" && (props.checked_symbol.is_some() || props.unchecked_symbol.is_some()) {
            let sym = if v.checked == Some(true) { v.checked_symbol.clone() } else { v.unchecked_symbol.clone() };
            if let Some(s) = sym {
                ctl.set_text(&s.ch().to_string(), false, Some(&s.font));
            }
        }
        // A date format shows the date again.
        if v.kind == "date" && (props.date_format.is_some() || props.lid.is_some()) && !v.placeholder {
            if let (Some(d), Some(fmt)) = (v.full_date.as_deref().and_then(controls::parse_date), v.date_format.as_deref()) {
                ctl.set_text(&controls::format_date(fmt, v.lid.as_deref().unwrap_or("en-US"), d.0, d.1, d.2), false, None);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_set(&mut self, r: &ControlRef, text: Option<&str>, checked: Option<bool>, index: Option<usize>, date: Option<&str>, drawing: Option<Element>, pkg: &crate::package::Package, main: &str) -> Result<(), EditError> {
        let mut ctl = self.find(r)?;
        let v = ctl.view();
        if controls::locks_content(v.lock.as_deref()) {
            return Err(EditError::Invalid("this content control is locked and its content can't be edited".into()));
        }
        if let Some(on) = checked {
            if v.kind != "checkbox" {
                return Err(EditError::Invalid("only a check box can be ticked".into()));
            }
            let sym = if on { v.checked_symbol.clone() } else { v.unchecked_symbol.clone() }.unwrap_or(controls::Symbol { val: if on { "2612" } else { "2610" }.into(), font: "MS Gothic".into() });
            if let Some(pr) = ctl.pr() {
                controls::set_checked(pr, on);
            }
            ctl.set_text(&sym.ch().to_string(), false, Some(&sym.font));
            return Ok(());
        }
        if let Some(i) = index {
            if !matches!(v.kind.as_str(), "dropDown" | "comboBox") {
                return Err(EditError::Invalid("only a drop-down list or combo box has entries".into()));
            }
            let item = v.items.get(i).ok_or_else(|| EditError::Invalid(format!("the list has {} entries", v.items.len())))?.clone();
            let pr = ctl.pr().map(|p| p.clone()).expect("pr");
            let prompt = placeholder_for(pkg, main, &pr);
            // Choosing Word's first entry (the prompt) shows the placeholder again.
            if i == 0 && (item.display == prompt || item.value.is_empty()) {
                if let Some(pr) = ctl.pr() {
                    controls::set_showing(pr, true);
                }
                ctl.set_text(&prompt, true, None);
            } else {
                if let Some(pr) = ctl.pr() {
                    controls::set_showing(pr, false);
                }
                ctl.clear_placeholder_style();
                ctl.set_text(&item.display, false, None);
            }
            return Ok(());
        }
        if let Some(d) = date {
            if v.kind != "date" {
                return Err(EditError::Invalid("only a date picker has a date".into()));
            }
            if d.is_empty() {
                if let Some(pr) = ctl.pr() {
                    if let Some(de) = pr.child_mut("w:date") {
                        de.remove_attr("w:fullDate");
                    }
                    controls::set_showing(pr, true);
                }
                let pr = ctl.pr().map(|p| p.clone()).expect("pr");
                let t = placeholder_for(pkg, main, &pr);
                ctl.set_text(&t, true, None);
                return Ok(());
            }
            let (y, m, dd) = controls::parse_date(d).ok_or_else(|| EditError::Invalid(format!("'{d}' is not a date (use 2026-10-14)")))?;
            if let Some(pr) = ctl.pr() {
                if let Some(de) = pr.child_mut("w:date") {
                    de.set_attr("w:fullDate", format!("{y:04}-{m:02}-{dd:02}T00:00:00Z"));
                }
                controls::set_showing(pr, false);
            }
            let fmt = v.date_format.clone().unwrap_or_else(|| "M/d/yyyy".into());
            let t = controls::format_date(&fmt, v.lid.as_deref().unwrap_or("en-US"), y, m, dd);
            ctl.clear_placeholder_style();
            ctl.set_text(&t, false, None);
            return Ok(());
        }
        if let Some(el) = drawing {
            if v.kind != "picture" {
                return Err(EditError::Invalid("only a picture control takes a picture".into()));
            }
            if let Some(pr) = ctl.pr() {
                controls::set_showing(pr, false);
            }
            let run = Run { t_attrs: None, attrs: vec![], rpr: None, items: vec![RunItem::Object(el)] };
            match &mut ctl {
                Ctl::Inline(c) => c.children = vec![Inline::Run(run)],
                Ctl::Block(c) => {
                    if let Some(Block::Para(p)) = c.blocks.iter_mut().find(|b| matches!(b, Block::Para(_))) {
                        p.inlines = vec![Inline::Run(run)];
                    }
                }
            }
            return Ok(());
        }
        if let Some(t) = text {
            if !matches!(v.kind.as_str(), "text" | "richText" | "comboBox" | "dropDown" | "date" | "group") {
                return Err(EditError::Invalid(format!("a {} control doesn't take text", v.kind)));
            }
            if t.is_empty() && controls::is_textual(&v.kind) {
                let pr = ctl.pr().map(|p| p.clone()).expect("pr");
                let ph = placeholder_for(pkg, main, &pr);
                if let Some(pr) = ctl.pr() {
                    controls::set_showing(pr, true);
                }
                ctl.set_text(&ph, true, None);
            } else {
                if let Some(pr) = ctl.pr() {
                    controls::set_showing(pr, false);
                }
                ctl.clear_placeholder_style();
                let t = if v.kind == "text" && !v.multiline { t.replace(['\n', '\r'], " ") } else { t.to_string() };
                ctl.set_text(&t, false, None);
            }
            return Ok(());
        }
        Err(EditError::Invalid("say what to set: text, checked, index, date or picture".into()))
    }
}

pub(super) fn remove_sdt(inlines: &mut Vec<Inline>, n: usize, seen: &mut usize, delete_content: bool) -> Result<bool, EditError> {
    let mut i = 0;
    while i < inlines.len() {
        if let Inline::Container(c) = &mut inlines[i] {
            if c.kind == InlineKind::Sdt {
                if *seen == n {
                    let v = c.shell.child("w:sdtPr").map(controls::read_props).unwrap_or_default();
                    if controls::locks_control(v.lock.as_deref()) {
                        return Err(EditError::Invalid("this content control is locked and can't be deleted".into()));
                    }
                    if delete_content && controls::locks_content(v.lock.as_deref()) {
                        return Err(EditError::Invalid("this content control is locked and its content can't be deleted".into()));
                    }
                    let Inline::Container(c) = inlines.remove(i) else { unreachable!() };
                    if !delete_content {
                        for (k, ch) in c.children.into_iter().enumerate() {
                            inlines.insert(i + k, ch);
                        }
                    }
                    return Ok(true);
                }
                *seen += 1;
            }
            if remove_sdt(&mut c.children, n, seen, delete_content)? {
                return Ok(true);
            }
        }
        i += 1;
    }
    Ok(false)
}

/// A copy of a repeating item with fresh ids and its controls back at their placeholders.
pub(super) fn reset_for_copy(item: &mut Block, ids: &mut IdGen, pkg: &crate::package::Package, main: &str) {
    reid_blocks(std::slice::from_mut(item), ids);
    // Controls inside carry their own `w:id`; the copy gets numbers of its own too.
    fn renumber(blocks: &mut [Block], next: &mut i64) {
        fn inl(inlines: &mut [Inline], next: &mut i64) {
            for i in inlines.iter_mut() {
                if let Inline::Container(c) = i {
                    if c.kind == InlineKind::Sdt {
                        if let Some(e) = c.shell.child_mut("w:sdtPr").and_then(|p| p.child_mut("w:id")) {
                            *next += 1;
                            e.set_attr("w:val", next.to_string());
                        }
                    }
                    inl(&mut c.children, next);
                }
            }
        }
        for b in blocks.iter_mut() {
            match b {
                Block::Para(p) => inl(&mut p.inlines, next),
                Block::Container(c) => {
                    if let Some(e) = c.shell.child_mut("w:sdtPr").and_then(|p| p.child_mut("w:id")) {
                        *next += 1;
                        e.set_attr("w:val", next.to_string());
                    }
                    renumber(&mut c.blocks, next);
                }
                Block::Table(t) => {
                    for r in t.rows_mut() {
                        for c in r.cells_mut() {
                            renumber(&mut c.blocks, next);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    // A big start keeps the numbers clear of the ones in the document.
    let mut next = 2_000_000_000i64 - (ids.0 as i64 % 1_000_000) * 100;
    renumber(std::slice::from_mut(item), &mut next);
    if let Block::Container(c) = item {
        reset_controls(&mut c.blocks, pkg, main);
    }
}
