//! Bound controls: writing values to the custom data, showing them in the document, and the clean-up pass after an edit.

use super::*;

impl Document {
    /// Write the values of bound controls to their custom XML parts, then show the value in every control bound
    /// to the same node.
    pub(in crate::document) fn write_bindings(&mut self, bound: &[(String, String, String, String)]) {
        let mut changed_nodes: Vec<(String, String, String, String)> = Vec::new();
        // Only a control that differs from the node as it was is a source; the others still show the old value.
        let snapshot: Vec<Option<String>> = bound.iter().map(|(x, s, p, _)| self.bound_get(s, x, p)).collect();
        for ((xpath, store, prefixes, value), was) in bound.iter().zip(snapshot) {
            if !same_bound_value(was.as_deref(), value) && self.bound_set(store, xpath, prefixes, value) {
                changed_nodes.retain(|(x, s, _, _)| x != xpath || s != store);
                changed_nodes.push((xpath.clone(), store.clone(), prefixes.clone(), value.clone()));
            }
        }
        if changed_nodes.is_empty() {
            return;
        }
        // Other controls bound to the same node follow.
        self.show_bound_everywhere(&changed_nodes);
    }

    /// Show `nodes` in the controls bound to them, in the body and in the headers and footers.
    pub(in crate::document) fn show_bound_everywhere(&mut self, nodes: &[Bound]) -> bool {
        let main = self.main_path.clone();
        let pkg = &self.pkg;
        let mut touched = Vec::new();
        for i in 0..self.body.len() {
            let id = self.body[i].id();
            if self.body.try_edit(i, |b| show_bound(b, nodes, pkg, &main)) {
                touched.push(id);
            }
        }
        let body_changed = !touched.is_empty();
        self.changed.extend(touched);
        let mut dirty = Vec::new();
        for s in self.stories.iter_mut().filter(|s| matches!(s.kind, StoryKind::Header | StoryKind::Footer)) {
            let mut any = false;
            for b in s.blocks.iter_mut() {
                any |= show_bound(b, nodes, pkg, &main);
            }
            if any {
                dirty.push(s.part.clone());
            }
        }
        if !dirty.is_empty() {
            self.story_gen += 1;
            let gen = self.story_gen;
            for s in self.stories.iter_mut().filter(|s| dirty.contains(&s.part)) {
                s.gen = gen;
            }
            self.dirty_parts.extend(dirty.clone());
            self.stories_touched = true;
        }
        // What was written may be a placeholder, which carries the PlaceholderText style.
        if body_changed || !dirty.is_empty() {
            self.ensure_placeholder_style();
            let styles = &self.styles;
            if body_changed {
                let pointed = super::style_refs::point_all_style_refs_body(&mut self.body, styles);
                self.changed.extend(pointed);
            }
            for s in self.stories.iter_mut().filter(|s| dirty.contains(&s.part)) {
                super::style_refs::point_all_style_refs(&mut s.blocks, styles);
            }
        }
        body_changed
    }

    /// After undo or redo the controls are the truth: write what they show to their custom XML nodes. The body and
    /// the headers and footers all count.
    pub(in crate::document) fn export_bound_controls(&mut self) {
        let main = self.main_path.clone();
        let mut all: Vec<Bound> = Vec::new();
        {
            let pkg = &self.pkg;
            // Re-use the clean-up pass without touching placeholders: only read the values. The pass wants blocks it
            // could change, so a block with a bound control goes through it as a scratch copy (the body is not changed:
            // it is shared with the undo steps), and a block with none is not looked at.
            for b in self.body.iter() {
                let mut probe = Vec::new();
                collect_bound(b, &mut probe);
                if !probe.is_empty() {
                    collect_bound_values(&mut b.clone(), pkg, &main, &mut all);
                }
            }
            for s in self.stories.iter_mut().filter(|s| matches!(s.kind, StoryKind::Header | StoryKind::Footer)) {
                for b in s.blocks.iter_mut() {
                    collect_bound_values(b, pkg, &main, &mut all);
                }
            }
        }
        if !all.is_empty() {
            self.write_bindings(&all);
        }
    }

    /// Make every bound control show the value of its node: a custom XML part, or a document property (done when a
    /// document opens, and after the properties change).
    pub(in crate::document) fn sync_bound_controls(&mut self) -> bool {
        let mut all: Vec<Bound> = Vec::new();
        for b in self.body.iter() {
            collect_bound(b, &mut all);
        }
        for s in self.stories.iter().filter(|s| matches!(s.kind, StoryKind::Header | StoryKind::Footer)) {
            for b in &s.blocks {
                collect_bound(b, &mut all);
            }
        }
        if all.is_empty() {
            return false;
        }
        let nodes: Vec<Bound> = all.into_iter().filter_map(|(x, s, p, _)| self.bound_get(&s, &x, &p).map(|v| (x, s, p, v))).collect();
        if nodes.is_empty() {
            return false;
        }
        self.show_bound_everywhere(&nodes)
    }

    // -----------------------------------------------------------------------
    // Locks
    // -----------------------------------------------------------------------

}

/// The locked controls in a list of blocks (the body or a story): their id, lock and a signature of their content, to
/// compare after an op.
pub(in crate::document) fn lock_state_of<'a>(body: impl IntoIterator<Item = &'a Block>) -> Vec<(i64, String, String)> {
    let mut out = Vec::new();

    pub(super) fn pr_of(shell: &Element) -> Option<(i64, String)> {
        let pr = shell.child("w:sdtPr")?;
        let v = controls::read_props(pr);
        Some((v.id?, v.lock.unwrap_or_default()))
    }

    pub(super) fn inl(inlines: &[Inline], out: &mut Vec<(i64, String, String)>) {
        for i in inlines {
            if let Inline::Container(c) = i {
                if c.kind == InlineKind::Sdt {
                    if let Some((id, lock)) = pr_of(&c.shell) {
                        // Only locked controls need their content compared.
                        let sig: String = if lock.is_empty() { String::new() } else { c.children.iter().map(|x| write_inline(x).to_xml()).collect() };
                        out.push((id, lock, sig));
                    }
                }
                inl(&c.children, out);
            }
        }
    }

    pub(super) fn blocks<'a>(bs: impl IntoIterator<Item = &'a Block>, out: &mut Vec<(i64, String, String)>) {
        for b in bs {
            match b {
                Block::Para(p) => inl(&p.inlines, out),
                Block::Container(c) => {
                    if let Some((id, lock)) = pr_of(&c.shell) {
                        let sig: String = if lock.is_empty() {
                            String::new()
                        } else {
                            let mut nodes = Vec::new();
                            write_blocks(&c.blocks, &mut nodes);
                            nodes.iter().filter_map(|n| if let Node::Element(e) = n { Some(e.to_xml()) } else { None }).collect()
                        };
                        out.push((id, lock, sig));
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
    blocks(body, &mut out);
    // A file may give two controls the same id: the nth control with an id is told apart by its place among them.
    let mut seen: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for (id, _, _) in out.iter_mut() {
        let n = seen.entry(*id).or_insert(0);
        let key = *id * 1000 + *n;
        *n += 1;
        *id = key;
    }
    out
}

/// Whether the ops left every locked control as it was: not deleted when the lock keeps it, not edited when the lock
/// keeps its content. `before` and `after` are `lock_state_of` the same list of blocks, before and after.
pub(in crate::document) fn locks_hold(before: &[(i64, String, String)], after: &[(i64, String, String)]) -> Result<(), EditError> {
    for (id, lock, sig) in before.iter().filter(|(_, l, _)| !l.is_empty()) {
        match after.iter().find(|(i, _, _)| i == id) {
            None if controls::locks_control(Some(lock)) => {
                return Err(EditError::Invalid("a content control is locked and can't be deleted".into()));
            }
            // The whole control went, content and all: that is allowed when only the content is locked.
            None => {}
            Some((_, now_lock, now_sig)) if controls::locks_content(Some(lock)) && now_lock == lock && now_sig != sig => {
                return Err(EditError::Invalid("a content control is locked and its content can't be edited".into()));
            }
            _ => {}
        }
    }
    Ok(())
}

/// What `fix_controls_in` found.
#[derive(Default)]
pub(in crate::document) struct Fixed {
    /// The values of the bound controls, for `Document::write_bindings`.
    pub values: Vec<Bound>,
    /// A placeholder was written, so the document needs the `PlaceholderText` style.
    pub placeholders: bool,
}

/// Put control state right in a list of blocks (the body or a story) after ops changed the blocks in `changed`: a
/// control that was showing its placeholder and now holds other text stops showing it; one that is empty shows its
/// placeholder again; legacy text fields keep to their maximum length and format. Returns what it found. `main` is the main part, where the placeholder text is looked up.
pub(in crate::document) fn fix_controls_in<'a>(blocks: impl IntoIterator<Item = &'a mut Block>, changed: &BTreeSet<NodeId>, pkg: &crate::package::Package, main: &str) -> Fixed {
    let mut bound = Fixed::default();
    if changed.is_empty() {
        return bound;
    }
    for b in blocks {
        if changed.contains(&b.id()) {
            fix_block(b, pkg, main, &mut bound);
        }
    }
    bound
}

pub(super) type Bound = (String, String, String, String);

pub(super) fn fix_block(b: &mut Block, pkg: &crate::package::Package, main: &str, bound: &mut Fixed) {
    match b {
        Block::Para(p) => {
            fix_inlines(&mut p.inlines, pkg, main, bound);
            fix_forms(&mut p.inlines);
        }
        Block::Container(c) => {
            if c.shell.child("w:sdtPr").is_some() {
                fix_ctl(&mut Ctl::Block(c), pkg, main, bound);
            }
            for x in c.blocks.iter_mut() {
                fix_block(x, pkg, main, bound);
            }
        }
        Block::Table(t) => {
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    for x in c.blocks.iter_mut() {
                        fix_block(x, pkg, main, bound);
                    }
                }
            }
        }
        Block::Opaque(_) => {}
    }
}

pub(super) fn fix_inlines(inlines: &mut [Inline], pkg: &crate::package::Package, main: &str, bound: &mut Fixed) {
    for i in inlines.iter_mut() {
        if let Inline::Container(c) = i {
            if c.kind == InlineKind::Sdt {
                fix_ctl(&mut Ctl::Inline(c), pkg, main, bound);
            }
            if let Inline::Container(c) = i {
                fix_inlines(&mut c.children, pkg, main, bound);
            }
        }
    }
}

pub(super) fn fix_ctl(ctl: &mut Ctl, pkg: &crate::package::Package, main: &str, bound: &mut Fixed) {
    let v = ctl.view();
    if !controls::is_textual(&v.kind) {
        return;
    }
    let pr = match ctl.pr() {
        Some(p) => p.clone(),
        None => return,
    };
    let text = ctl.text();
    let ph_text = placeholder_for(pkg, main, &pr);
    let has_ph_part = controls::placeholder_name(&pr).is_some();
    if v.placeholder {
        if text.is_empty() {
            bound.placeholders = true;
            ctl.set_text(&ph_text, true, None);
        } else if text != ph_text {
            // Typed into a placeholder: the placeholder goes, what was typed stays.
            let typed = typed_into(&ph_text, &text);
            if typed.is_empty() {
                bound.placeholders = true;
            ctl.set_text(&ph_text, true, None);
            } else {
                if let Some(p) = ctl.pr() {
                    controls::set_showing(p, false);
                }
                ctl.clear_placeholder_style();
                // Only when part of the placeholder is still there does the text change, and then only that part
                // goes: what was put in stays as it is (its runs and formatting, a field, a picture). When all of it is
                // new (or a file holds text next to a stale placeholder flag), the runs stay as they are.
                if typed != text {
                    let (pre, suf) = placeholder_left(&ph_text, &text);
                    let units = |s: &[char]| s.iter().map(|c| c.len_utf16()).sum::<usize>();
                    let chars: Vec<char> = text.chars().collect();
                    ctl.cut_ends(units(&chars[..pre]), units(&chars[chars.len() - suf..]));
                    if ctl.text() != typed {
                        ctl.set_text(&typed, false, None);
                    }
                }
            }
        }
    } else if text.is_empty() && has_ph_part {
        if let Some(p) = ctl.pr() {
            controls::set_showing(p, true);
            if let Some(d) = p.child_mut("w:date") {
                d.remove_attr("w:fullDate");
            }
        }
        bound.placeholders = true;
            ctl.set_text(&ph_text, true, None);
    }
    if let Some((xpath, store, prefixes)) = pr_binding(ctl) {
        let now = ctl.view();
        bound.values.push((xpath, store, prefixes, bound_value(ctl, &now)));
    }
}

/// What a bound control hands to its node: its text, or for a date picker the date as the node keeps it
/// (`2026-10-14T00:00:00Z`, from `w:fullDate`), nothing while it shows its placeholder.
pub(super) fn bound_value(ctl: &mut Ctl, v: &RControl) -> String {
    if v.placeholder {
        return String::new();
    }
    if v.kind == "date" {
        if let Some(full) = ctl.pr().and_then(|p| p.child("w:date")).and_then(|d| d.attr("w:fullDate")).filter(|d| !d.is_empty()) {
            return full.to_string();
        }
    }
    ctl.text()
}

/// Whether the node already holds the value. Two spellings of one date (`2026-10-14T00:00:00` as Word writes the cover
/// date and `2026-10-14T00:00:00Z` as a date box's `fullDate`) are the same value.
pub(super) fn same_bound_value(was: Option<&str>, value: &str) -> bool {
    match was {
        Some(w) if w == value => true,
        Some(w) => matches!((controls::parse_date(w), controls::parse_date(value)), (Some(a), Some(b)) if a == b && date_part_only(w) && date_part_only(value)),
        None => false,
    }
}

/// A date with no time of day other than midnight (what a date box holds), so comparing the day is enough.
pub(super) fn date_part_only(s: &str) -> bool {
    s.get(10..).is_none_or(|rest| matches!(rest, "" | "T00:00:00" | "T00:00:00Z"))
}

/// What was typed into a placeholder: the new text without the stretches at its start and end that are still the
/// placeholder's.
pub(super) fn typed_into(old: &str, new: &str) -> String {
    let n: Vec<char> = new.chars().collect();
    let (pre, suf) = placeholder_left(old, new);
    n[pre..n.len() - suf].iter().collect()
}

/// How many characters at the start and at the end of `new` are still the placeholder `old`'s.
pub(super) fn placeholder_left(old: &str, new: &str) -> (usize, usize) {
    let (o, n): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let pre = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let suf = o.iter().rev().zip(n.iter().rev()).take_while(|(a, b)| a == b).count().min(o.len() - pre).min(n.len() - pre);
    (pre, suf)
}

pub(super) fn pr_binding(ctl: &mut Ctl) -> Option<(String, String, String)> {
    ctl.pr().and_then(|p| controls::binding_of(p))
}

/// The bound text controls under a block, with the text they show (nothing while they show a placeholder).
pub(super) fn collect_bound_values(b: &mut Block, _pkg: &crate::package::Package, _main: &str, out: &mut Vec<Bound>) {
    fn inl(inlines: &mut [Inline], out: &mut Vec<Bound>) {
        for i in inlines.iter_mut() {
            if let Inline::Container(c) = i {
                if c.kind == InlineKind::Sdt {
                    let mut ctl = Ctl::Inline(c);
                    push_bound_value(&mut ctl, out);
                }
                if let Inline::Container(c) = i {
                    inl(&mut c.children, out);
                }
            }
        }
    }
    match b {
        Block::Para(p) => inl(&mut p.inlines, out),
        Block::Container(c) => {
            if c.shell.child("w:sdtPr").is_some() {
                push_bound_value(&mut Ctl::Block(c), out);
            }
            for x in c.blocks.iter_mut() {
                collect_bound_values(x, _pkg, _main, out);
            }
        }
        Block::Table(t) => {
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    for x in c.blocks.iter_mut() {
                        collect_bound_values(x, _pkg, _main, out);
                    }
                }
            }
        }
        Block::Opaque(_) => {}
    }
}

pub(super) fn push_bound_value(ctl: &mut Ctl, out: &mut Vec<Bound>) {
    let v = ctl.view();
    if !controls::is_textual(&v.kind) {
        return;
    }
    if let Some((x, s, p)) = pr_binding(ctl) {
        out.push((x, s, p, bound_value(ctl, &v)));
    }
}

pub(super) fn collect_bound(b: &Block, out: &mut Vec<Bound>) {
    fn inl(inlines: &[Inline], out: &mut Vec<Bound>) {
        for i in inlines {
            if let Inline::Container(c) = i {
                if c.kind == InlineKind::Sdt {
                    if let Some((x, s, p)) = c.shell.child("w:sdtPr").and_then(controls::binding_of) {
                        out.push((x, s, p, String::new()));
                    }
                }
                inl(&c.children, out);
            }
        }
    }
    match b {
        Block::Para(p) => inl(&p.inlines, out),
        Block::Container(c) => {
            if let Some((x, s, p)) = c.shell.child("w:sdtPr").and_then(controls::binding_of) {
                out.push((x, s, p, String::new()));
            }
            for x in &c.blocks {
                collect_bound(x, out);
            }
        }
        Block::Table(t) => {
            for r in t.rows() {
                for c in r.cells() {
                    for x in &c.blocks {
                        collect_bound(x, out);
                    }
                }
            }
        }
        Block::Opaque(_) => {}
    }
}

/// Show `nodes` (xpath, store, prefixes, value) in the controls bound to them. Returns whether anything changed.
pub(super) fn show_bound(b: &mut Block, nodes: &[Bound], pkg: &crate::package::Package, main: &str) -> bool {
    fn one(ctl: &mut Ctl, nodes: &[Bound], pkg: &crate::package::Package, main: &str) -> bool {
        let Some((x, s, _)) = pr_binding(ctl) else { return false };
        let Some((_, _, _, value)) = nodes.iter().find(|(nx, ns, _, _)| *nx == x && *ns == s) else { return false };
        let v = ctl.view();
        let shown = bound_value(ctl, &v);
        // The same value spelt another way (a date as Word writes the cover date, `…T00:00:00`, and as a date box keeps
        // it, `…T00:00:00Z`) is not a change: the box keeps what it holds (rebuilding it keeps only its first paragraph).
        if same_bound_value(Some(&shown), value) {
            return false;
        }
        match v.kind.as_str() {
            // A date node holds `2026-10-14T00:00:00Z`; the control shows it in its own format.
            "date" if !value.is_empty() && controls::parse_date(value).is_some() => {
                let (y, m, d) = controls::parse_date(value).expect("checked");
                let fmt = v.date_format.clone().unwrap_or_else(|| "M/d/yyyy".into());
                let text = controls::format_date(&fmt, v.lid.as_deref().unwrap_or("en-US"), y, m, d);
                if let Some(pr) = ctl.pr() {
                    if let Some(de) = pr.child_mut("w:date") {
                        de.set_attr("w:fullDate", format!("{y:04}-{m:02}-{d:02}T00:00:00Z"));
                    }
                    controls::set_showing(pr, false);
                }
                ctl.clear_placeholder_style();
                ctl.set_text(&text, false, None);
            }
            "checkbox" => {
                let on = matches!(value.as_str(), "1" | "true" | "on");
                let sym = if on { v.checked_symbol.clone() } else { v.unchecked_symbol.clone() };
                if let Some(pr) = ctl.pr() {
                    controls::set_checked(pr, on);
                }
                if let Some(sym) = sym {
                    ctl.set_text(&sym.ch().to_string(), false, Some(&sym.font));
                }
            }
            "date" | "text" | "richText" | "dropDown" | "comboBox" => {
                let pr = ctl.pr().map(|p| p.clone());
                if value.is_empty() {
                    let t = pr.as_ref().map(|p| placeholder_for(pkg, main, p)).unwrap_or_default();
                    if let Some(pr) = ctl.pr() {
                        controls::set_showing(pr, true);
                    }
                    ctl.set_text(&t, true, None);
                } else {
                    if let Some(pr) = ctl.pr() {
                        controls::set_showing(pr, false);
                    }
                    ctl.clear_placeholder_style();
                    ctl.set_text(value, false, None);
                }
            }
            _ => return false,
        }
        true
    }
    fn inl(inlines: &mut [Inline], nodes: &[Bound], pkg: &crate::package::Package, main: &str) -> bool {
        let mut any = false;
        for i in inlines.iter_mut() {
            if let Inline::Container(c) = i {
                if c.kind == InlineKind::Sdt {
                    any |= one(&mut Ctl::Inline(c), nodes, pkg, main);
                }
                if let Inline::Container(c) = i {
                    any |= inl(&mut c.children, nodes, pkg, main);
                }
            }
        }
        any
    }
    match b {
        Block::Para(p) => inl(&mut p.inlines, nodes, pkg, main),
        Block::Container(c) => {
            let mut any = false;
            if c.shell.child("w:sdtPr").is_some() {
                any |= one(&mut Ctl::Block(c), nodes, pkg, main);
            }
            for x in c.blocks.iter_mut() {
                any |= show_bound(x, nodes, pkg, main);
            }
            any
        }
        Block::Table(t) => {
            let mut any = false;
            for r in t.rows_mut() {
                for c in r.cells_mut() {
                    for x in c.blocks.iter_mut() {
                        any |= show_bound(x, nodes, pkg, main);
                    }
                }
            }
            any
        }
        Block::Opaque(_) => false,
    }
}

/// Keep legacy text fields to their maximum length and format after typing.
pub(super) fn fix_forms(inlines: &mut Vec<Inline>) {
    let views = forms::views(inlines);
    // Last field first, so the offsets of the others stay valid.
    for f in views.iter().rev().filter(|f| f.kind == "text") {
        let current = text_between(inlines, f.start, f.end);
        let mut t = current.clone();
        if let Some(fm) = &f.format {
            t = forms::apply_format(&t, fm);
        }
        if let Some(max) = f.max_length {
            if t.chars().count() > max as usize {
                t = t.chars().take(max as usize).collect();
            }
        }
        // A blank stays a blank to fill in.
        if t.is_empty() {
            t = f.default.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| forms::EMPTY_RESULT.to_string());
        }
        if t != current {
            replace_form_text(inlines, f.at, &t);
        }
    }
}

pub(super) fn text_between(inlines: &[Inline], from: usize, to: usize) -> String {
    let mut out = String::new();
    let mut at = 0usize;
    for (r, _) in forms::runs_of(inlines) {
        for item in &r.items {
            let l = item.len16();
            if at >= from && at + l <= to {
                out.push_str(&item.flat());
            }
            at += l;
        }
    }
    out
}

/// Replace the result of the complex field that begins at `field_at`.
pub(super) fn replace_form_text(inlines: &mut Vec<Inline>, field_at: usize, text: &str) {
    crate::fields::update_inlines(inlines, 0, &mut |s| (s.offset == field_at && s.instr.trim_start().to_uppercase().starts_with("FORMTEXT")).then(|| crate::fields::Replace { text: text.to_string(), rstyle: None, ..Default::default() }));
}
