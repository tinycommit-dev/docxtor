//! Text-editing primitives over inline content.
//!
//! All offsets are UTF-16 code units into a paragraph's flattened text (see
//! [`Paragraph::text`]), matching what browsers report for DOM selections.

use crate::model::*;
use crate::xml::Element;

pub fn utf16_to_byte(s: &str, k: usize) -> usize {
    let mut n = 0;
    for (b, ch) in s.char_indices() {
        if n >= k {
            return b;
        }
        n += ch.len_utf16();
    }
    s.len()
}

fn split_run(r: &Run, k: usize) -> (Run, Run) {
    let mut left = Run { t_attrs: r.t_attrs.clone(), attrs: r.attrs.clone(), rpr: r.rpr.clone(), items: Vec::new() };
    let mut right = Run { t_attrs: r.t_attrs.clone(), attrs: r.attrs.clone(), rpr: r.rpr.clone(), items: Vec::new() };
    let mut pos = 0;
    for item in &r.items {
        let l = item.len16();
        if pos + l <= k && (l > 0 || pos < k) {
            left.items.push(item.clone());
        } else if pos >= k {
            right.items.push(item.clone());
        } else if let RunItem::Text(t) = item {
            let b = utf16_to_byte(t, k - pos);
            left.items.push(RunItem::Text(t[..b].to_string()));
            right.items.push(RunItem::Text(t[b..].to_string()));
        } else {
            left.items.push(item.clone());
        }
        pos += l;
    }
    (left, right)
}

/// Ensure there is a run boundary at `off` (splitting a run if needed).
pub fn split_at(inlines: &mut Vec<Inline>, off: usize) {
    let mut start = 0;
    let mut i = 0;
    while i < inlines.len() {
        let len = inlines[i].len16();
        if start < off && off < start + len {
            match &mut inlines[i] {
                Inline::Run(r) => {
                    let (a, b) = split_run(r, off - start);
                    inlines[i] = Inline::Run(a);
                    inlines.insert(i + 1, Inline::Run(b));
                }
                Inline::Container(c) => split_at(&mut c.children, off - start),
                Inline::Opaque(_) => {}
            }
            return;
        }
        start += len;
        i += 1;
    }
}

fn run_is_textual(r: &Run) -> bool {
    r.items.is_empty() || r.items.iter().any(|i| matches!(i, RunItem::Text(_) | RunItem::Tab | RunItem::Break(_)))
        && !r.items.iter().any(|i| matches!(i, RunItem::Hidden(e) if e.name == "w:fldChar" || e.name == "w:instrText"))
}

/// `object_at_mut`, for reading.
pub fn object_at(inlines: &[Inline], off: usize) -> Option<&Element> {
    fn go<'a>(inlines: &'a [Inline], off: usize, start: &mut usize) -> Option<&'a Element> {
        for inl in inlines.iter() {
            let len = inl.len16();
            if off >= *start && off < *start + len {
                return match inl {
                    Inline::Run(r) => {
                        let mut pos = *start;
                        for item in r.items.iter() {
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

/// The inline object (image, shape, ...) occupying `[off, off+1)`, if any.
pub fn object_at_mut(inlines: &mut [Inline], off: usize) -> Option<&mut Element> {
    fn go<'a>(inlines: &'a mut [Inline], off: usize, start: &mut usize) -> Option<&'a mut Element> {
        for inl in inlines.iter_mut() {
            let len = inl.len16();
            if off >= *start && off < *start + len {
                return match inl {
                    Inline::Run(r) => {
                        let mut pos = *start;
                        for item in r.items.iter_mut() {
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
                    Inline::Container(c) => go(&mut c.children, off, start),
                    Inline::Opaque(_) => None,
                };
            }
            *start += len;
        }
        None
    }
    go(inlines, off, &mut 0)
}

/// Path (indices through containers) to the run that should receive text typed at `off`.
fn locate_run(inlines: &[Inline], off: usize) -> Option<(Vec<usize>, usize)> {
    fn before(inlines: &[Inline], off: usize, base: usize, path: &mut Vec<usize>) -> Option<(Vec<usize>, usize)> {
        let mut start = base;
        for (i, inl) in inlines.iter().enumerate() {
            let len = inl.len16();
            match inl {
                Inline::Run(r) => {
                    if start < off && off <= start + len && run_is_textual(r) {
                        let mut p = path.clone();
                        p.push(i);
                        return Some((p, off - start));
                    }
                }
                Inline::Container(c) => {
                    let ends_here = off == start + len;
                    // Text typed right after a link, a field result or a citation goes outside it.
                    let skip = ends_here && (matches!(c.kind, InlineKind::Hyperlink | InlineKind::Field) || crate::cite_fields::is_citation_sdt(c));
                    if start < off && off <= start + len && !skip {
                        path.push(i);
                        let r = before(&c.children, off, start, path);
                        path.pop();
                        if r.is_some() {
                            return r;
                        }
                    }
                }
                Inline::Opaque(_) => {}
            }
            start += len;
        }
        None
    }
    fn at_start(inlines: &[Inline], off: usize, base: usize, path: &mut Vec<usize>) -> Option<(Vec<usize>, usize)> {
        let mut start = base;
        for (i, inl) in inlines.iter().enumerate() {
            let len = inl.len16();
            match inl {
                Inline::Run(r) if start == off && run_is_textual(r) => {
                    let mut p = path.clone();
                    p.push(i);
                    return Some((p, 0));
                }
                Inline::Container(c) if start == off && c.kind != InlineKind::Hyperlink && !crate::cite_fields::is_citation_sdt(c) => {
                    path.push(i);
                    let r = at_start(&c.children, off, start, path);
                    path.pop();
                    if r.is_some() {
                        return r;
                    }
                }
                _ => {}
            }
            if start > off {
                break;
            }
            start += len;
        }
        None
    }
    // Text typed where an empty content control stands goes into the control, not before it.
    fn in_empty_control(inlines: &[Inline], off: usize, base: usize, path: &mut Vec<usize>) -> Option<(Vec<usize>, usize)> {
        let mut start = base;
        for (i, inl) in inlines.iter().enumerate() {
            let len = inl.len16();
            if let Inline::Container(c) = inl {
                if start <= off && off <= start + len {
                    path.push(i);
                    let found = if len == 0 && c.kind == InlineKind::Sdt && !crate::cite_fields::is_citation_sdt(c) {
                        c.children.iter().position(|ch| matches!(ch, Inline::Run(r) if run_is_textual(r))).map(|j| {
                            let mut p = path.clone();
                            p.push(j);
                            (p, 0)
                        })
                    } else {
                        in_empty_control(&c.children, off, start, path)
                    };
                    path.pop();
                    if found.is_some() {
                        return found;
                    }
                }
            }
            start += len;
        }
        None
    }
    in_empty_control(inlines, off, 0, &mut Vec::new())
        .or_else(|| before(inlines, off, 0, &mut Vec::new()))
        .or_else(|| at_start(inlines, off, 0, &mut Vec::new()))
}

fn run_by_path<'a>(inlines: &'a mut [Inline], path: &[usize]) -> &'a mut Run {
    match &mut inlines[path[0]] {
        Inline::Run(r) => r,
        Inline::Container(c) => run_by_path(&mut c.children, &path[1..]),
        Inline::Opaque(_) => unreachable!("path points at opaque"),
    }
}

fn insert_into_run(r: &mut Run, k: usize, text: &str) {
    let new_items = text_items(text);
    if new_items.len() == 1 {
        if let RunItem::Text(t) = &new_items[0] {
            // Fast path: extend a text item that touches k.
            let mut pos = 0;
            for item in r.items.iter_mut() {
                let l = item.len16();
                if let RunItem::Text(s) = item {
                    if pos <= k && k <= pos + l {
                        let b = utf16_to_byte(s, k - pos);
                        s.insert_str(b, t);
                        return;
                    }
                }
                pos += l;
            }
        }
    }
    // General path: split the item list at k and splice.
    let (mut left, right) = split_run(r, k);
    left.items.extend(new_items);
    left.items.extend(right.items);
    r.items = left.items;
}

/// Find the item index in `inlines` where an inline inserted at `off` belongs, recursing into containers.
pub fn insert_inline_at(inlines: &mut Vec<Inline>, off: usize, new: Inline) {
    split_at(inlines, off);
    let mut start = 0;
    for i in 0..inlines.len() {
        let len = inlines[i].len16();
        if start == off {
            inlines.insert(i, new);
            return;
        }
        if start < off && off < start + len {
            if let Inline::Container(c) = &mut inlines[i] {
                insert_inline_at(&mut c.children, off - start, new);
                return;
            }
        }
        start += len;
    }
    inlines.push(new);
}

/// Formatting (`w:rPr`) of the run just before `off` (or just after it at offset 0).
pub fn rpr_at(inlines: &[Inline], off: usize) -> Option<Element> {
    let mut best: Option<Element> = None;
    let mut first: Option<Element> = None;
    let mut start = 0;
    walk_runs(inlines, &mut start, &mut |r, s, stack| {
        let len = r.len16();
        if len == 0 {
            return;
        }
        // A citation's formatting (no proofing) belongs to the citation, not to the text typed next to it.
        if stack.iter().any(|c| crate::cite_fields::is_citation_sdt(c)) {
            return;
        }
        if first.is_none() {
            first = r.rpr.clone().or(Some(Element::new("w:rPr")));
        }
        // The run ending at (or containing) the caret wins; at offset 0, the first run.
        if (s < off && off <= s + len) || (s == off && best.is_none() && off == 0) {
            best = r.rpr.clone().or(Some(Element::new("w:rPr")));
        }
    });
    best.or(first)
}

fn insert_at_path(list: &mut Vec<Inline>, path: &[usize], k: usize, new: Inline) {
    match path.split_first() {
        None => list.insert(k.min(list.len()), new),
        Some((&p, rest)) => {
            if let Some(Inline::Container(c)) = list.get_mut(p) {
                insert_at_path(&mut c.children, rest, k, new);
            }
        }
    }
}

fn field_char(r: &Run, kind: &str) -> bool {
    r.items.iter().any(|it| matches!(it, RunItem::Hidden(e) if e.name == "w:fldChar" && e.attr("w:fldCharType") == Some(kind)))
}

/// Whether the runs hold the start of a legacy form field (`FORMTEXT`, `FORMCHECKBOX`, `FORMDROPDOWN`).
fn is_form_field(runs: &[Inline]) -> bool {
    let instr: String = runs
        .iter()
        .filter_map(|i| if let Inline::Run(r) = i { Some(r) } else { None })
        .flat_map(|r| r.items.iter())
        .filter_map(|it| if let RunItem::Hidden(e) = it { (e.name == "w:instrText").then(|| e.text()) } else { None })
        .collect();
    instr.trim_start().to_uppercase().starts_with("FORM") && !instr.trim_start().to_uppercase().starts_with("FORMULA")
}

/// When `off` is where a field with a visible result ends: the container path and index to insert at
/// (after the field's end), and the formatting of the text before the field.
fn after_field(inlines: &[Inline], off: usize) -> Option<(Vec<usize>, usize, Option<Element>)> {
    fn go(inlines: &[Inline], off: usize, base: usize, path: &mut Vec<usize>) -> Option<(Vec<usize>, usize, Option<Element>)> {
        let mut start = base;
        for (i, inl) in inlines.iter().enumerate() {
            let len = inl.len16();
            match inl {
                Inline::Run(r) if start == off && len == 0 && field_char(r, "end") => {
                    // Walk back to the field's begin.
                    let (mut depth, mut begin) = (0i32, None);
                    for j in (0..=i).rev() {
                        if let Inline::Run(q) = &inlines[j] {
                            for it in q.items.iter().rev() {
                                if let RunItem::Hidden(e) = it {
                                    match (e.name.as_str(), e.attr("w:fldCharType")) {
                                        ("w:fldChar", Some("end")) => depth += 1,
                                        ("w:fldChar", Some("begin")) => {
                                            depth -= 1;
                                            if depth == 0 {
                                                begin = Some(j);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        if begin.is_some() {
                            break;
                        }
                    }
                    if let Some(b) = begin {
                        // A form field's blank is typed into; only other fields are stepped over.
                        if inlines[b..i].iter().map(|x| x.len16()).sum::<usize>() > 0 && !is_form_field(&inlines[b..i]) {
                            let mut k = i + 1;
                            while matches!(inlines.get(k), Some(Inline::Run(q)) if q.len16() == 0 && field_char(q, "end")) {
                                k += 1;
                            }
                            let rpr = match b.checked_sub(1).map(|p| &inlines[p]) {
                                Some(Inline::Run(q)) if q.len16() > 0 => q.rpr.clone(),
                                _ => None,
                            };
                            return Some((path.clone(), k, rpr));
                        }
                    }
                }
                // A citation is one unit: text typed at its end goes after it, not after the field inside it.
                Inline::Container(c) if start < off && off <= start + len && matches!(c.kind, InlineKind::Insertion | InlineKind::Sdt | InlineKind::Other) && !(off == start + len && crate::cite_fields::is_citation_sdt(c)) => {
                    path.push(i);
                    let r = go(&c.children, off, start, path);
                    path.pop();
                    if r.is_some() {
                        return r;
                    }
                }
                _ => {}
            }
            start += len;
        }
        None
    }
    go(inlines, off, 0, &mut Vec::new())
}

/// Insert text at `off`. With `rpr` the text gets exactly that formatting in a new run;
/// otherwise it inherits the formatting of the surrounding run.
pub fn insert_text(inlines: &mut Vec<Inline>, off: usize, text: &str, rpr: Option<Element>) {
    if text.is_empty() {
        return;
    }
    if rpr.is_none() {
        // Right after a field's result, text goes after the field, not into the result.
        if let Some((path, k, rpr)) = after_field(inlines, off) {
            let run = Run { t_attrs: None, attrs: Vec::new(), rpr: rpr.filter(|e| !e.children.is_empty()), items: text_items(text) };
            insert_at_path(inlines, &path, k, Inline::Run(run));
            return;
        }
        if let Some((path, k)) = locate_run(inlines, off) {
            insert_into_run(run_by_path(inlines, &path), k, text);
            return;
        }
    }
    let rpr = rpr.or_else(|| {
        rpr_at(inlines, off).map(|mut e| {
            // Don't carry hyperlink character style outside of the link.
            if e.child("w:rStyle").and_then(|s| s.attr("w:val")) == Some("Hyperlink") {
                e.remove_children("w:rStyle");
                e.remove_children("w:color");
                e.remove_children("w:u");
            }
            e
        })
    });
    let run = Run { t_attrs: None, attrs: Vec::new(), rpr: rpr.filter(|e| !e.children.is_empty()), items: text_items(text) };
    insert_inline_at(inlines, off, Inline::Run(run));
}

/// Delete `[from, to)`.
pub fn delete_range(inlines: &mut Vec<Inline>, from: usize, to: usize) {
    if to <= from {
        return;
    }
    fn go(inlines: &mut [Inline], from: usize, to: usize, base: usize) {
        let mut start = base;
        for inl in inlines.iter_mut() {
            let len = inl.len16();
            let (s, e) = (from.max(start), to.min(start + len));
            if s < e {
                match inl {
                    Inline::Run(r) => delete_in_run(r, s - start, e - start),
                    Inline::Container(c) => go(&mut c.children, from, to, start),
                    // An equation (the only opaque inline with a length) is one character: it goes whole. An empty
                    // run holds its place so indices stay valid until `cleanup`.
                    Inline::Opaque(_) => *inl = Inline::Run(Run::default()),
                }
            }
            start += len;
        }
    }
    let fields_before = crate::fields::visible_fields(inlines);
    // A content control inside the range goes with it, unless the range is just its content (then it is emptied).
    let dropped: Vec<usize> = crate::controls::sdts_in(inlines)
        .into_iter()
        .enumerate()
        .filter(|(_, (c, s, e))| !crate::cite_fields::is_citation_sdt(c) && from <= *s && *e <= to && (from < *s || *e < to))
        .map(|(n, _)| n)
        .collect();
    go(inlines, from, to, 0);
    if !dropped.is_empty() {
        drop_sdts(inlines, &dropped, &mut 0);
    }
    // A field whose whole result was deleted goes with it (Word treats a field as one piece), so no
    // empty shell is left to show a number again when fields are updated.
    let mut gone: Vec<(usize, usize)> = fields_before.into_iter().filter(|(b, e, _)| inlines[*b..=*e].iter().all(|i| i.len16() == 0)).map(|(b, e, _)| (b, e)).collect();
    // Fields inside another one that goes are covered by it.
    let all = gone.clone();
    gone.retain(|(b, e)| !all.iter().any(|(b2, e2)| (b2, e2) != (b, e) && b2 <= b && e <= e2));
    gone.sort_unstable_by(|x, y| y.cmp(x));
    for (b, e) in gone {
        inlines.drain(b..=e);
    }
    cleanup(inlines);
}

/// Remove the content controls with these numbers (in the order `controls::sdts_in` counts them). Each leaves an empty
/// run in its place, so no position shifts.
fn drop_sdts(inlines: &mut [Inline], which: &[usize], n: &mut usize) {
    let mut i = 0;
    while i < inlines.len() {
        if let Inline::Container(c) = &mut inlines[i] {
            if c.kind == InlineKind::Sdt {
                let mine = *n;
                *n += 1;
                if which.contains(&mine) {
                    // Count the controls inside it too, so the numbers after it stay right.
                    let inner = crate::controls::sdts_in(&c.children).len();
                    *n += inner;
                    // It is swapped for an empty run, not removed: `delete_range` holds positions into `inlines`
                    // (the fields it may drop) and they must stay valid. `cleanup` removes the empty run.
                    inlines[i] = Inline::Run(Run::default());
                    i += 1;
                    continue;
                }
            }
            drop_sdts(&mut c.children, which, n);
        }
        i += 1;
    }
}

fn delete_in_run(r: &mut Run, from: usize, to: usize) {
    let mut pos = 0;
    let mut out = Vec::with_capacity(r.items.len());
    for item in r.items.drain(..) {
        let l = item.len16();
        let (s, e) = (from.max(pos), to.min(pos + l));
        if s >= e {
            out.push(item);
        } else if let RunItem::Text(t) = &item {
            let b1 = utf16_to_byte(t, s - pos);
            let b2 = utf16_to_byte(t, e - pos);
            let mut nt = String::with_capacity(t.len());
            nt.push_str(&t[..b1]);
            nt.push_str(&t[b2..]);
            if !nt.is_empty() {
                out.push(RunItem::Text(nt));
            }
        }
        // Non-text visible items fully covered are dropped.
        pos += l;
    }
    r.items = out;
}

/// Remove empty runs/containers and merge adjacent runs with identical formatting.
pub fn cleanup(inlines: &mut Vec<Inline>) {
    inlines.retain_mut(|inl| match inl {
        Inline::Run(r) => !r.items.is_empty(),
        Inline::Container(c) => {
            // An emptied content control keeps an empty run, so text typed there goes inside it.
            let keep_rpr = c.children.iter().find_map(|i| if let Inline::Run(r) = i { Some(r.rpr.clone()) } else { None });
            cleanup(&mut c.children);
            if c.kind == InlineKind::Sdt && c.children.is_empty() && !crate::cite_fields::is_citation_sdt(c) {
                if let Some(rpr) = keep_rpr {
                    c.children.push(Inline::Run(Run { rpr, ..Default::default() }));
                }
            }
            // A citation whose text was all deleted goes with it (its field would bring the text back on update).
            let empty_citation = c.kind == InlineKind::Sdt && c.shell.child("w:sdtPr").is_some_and(|p| p.child("w:citation").is_some()) && c.children.iter().all(|i| i.len16() == 0);
            !(empty_citation || (c.children.is_empty() && matches!(c.kind, InlineKind::Hyperlink | InlineKind::Insertion | InlineKind::Other)))
        }
        Inline::Opaque(_) => true,
    });
    let mut i = 0;
    while i + 1 < inlines.len() {
        let mergeable = match (&inlines[i], &inlines[i + 1]) {
            (Inline::Run(a), Inline::Run(b)) => a.rpr == b.rpr && run_is_plain(a) && run_is_plain(b),
            _ => false,
        };
        if mergeable {
            let Inline::Run(b) = inlines.remove(i + 1) else { unreachable!() };
            if let Inline::Run(a) = &mut inlines[i] {
                for item in b.items {
                    match (a.items.last_mut(), item) {
                        (Some(RunItem::Text(x)), RunItem::Text(y)) => x.push_str(&y),
                        (_, it) => a.items.push(it),
                    }
                }
            }
        } else {
            i += 1;
        }
    }
}

fn run_is_plain(r: &Run) -> bool {
    r.items.iter().all(|i| matches!(i, RunItem::Text(_) | RunItem::Tab))
}

/// Apply `f` to every run fully inside `[from, to)` (splitting at the edges first).
pub fn format_range(inlines: &mut Vec<Inline>, from: usize, to: usize, f: &mut dyn FnMut(&mut Run)) {
    if to <= from {
        return;
    }
    split_at(inlines, from);
    split_at(inlines, to);
    fn go(inlines: &mut [Inline], from: usize, to: usize, base: usize, f: &mut dyn FnMut(&mut Run)) {
        let mut start = base;
        for inl in inlines.iter_mut() {
            let len = inl.len16();
            if len > 0 && start >= from && start + len <= to {
                match inl {
                    Inline::Run(r) => f(r),
                    Inline::Container(c) => go(&mut c.children, from, to, start, f),
                    Inline::Opaque(_) => {}
                }
            } else if len > 0 && start < to && start + len > from {
                if let Inline::Container(c) = inl {
                    go(&mut c.children, from, to, start, f);
                }
            }
            start += len;
        }
    }
    go(inlines, from, to, 0, f);
    cleanup(inlines);
}

/// Split inline content at `off` into (before, after). Containers that straddle are cloned.
pub fn partition(mut inlines: Vec<Inline>, off: usize) -> (Vec<Inline>, Vec<Inline>) {
    split_at(&mut inlines, off);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut start = 0;
    for inl in inlines {
        let len = inl.len16();
        let end = start + len;
        if len == 0 {
            // Zero-width markers stay with the preceding text, except at the very start.
            if start == off && off == 0 {
                right.push(inl);
            } else if start <= off {
                left.push(inl);
            } else {
                right.push(inl);
            }
        } else if end <= off {
            left.push(inl);
        } else if start >= off {
            right.push(inl);
        } else {
            match inl {
                Inline::Container(c) => {
                    let (a, b) = partition(c.children, off - start);
                    left.push(Inline::Container(InlineContainer { kind: c.kind, shell: c.shell.clone(), content_in: c.content_in.clone(), children: a }));
                    right.push(Inline::Container(InlineContainer { kind: c.kind, shell: c.shell, content_in: c.content_in, children: b }));
                }
                other => left.push(other),
            }
        }
        start = end;
    }
    (left, right)
}

/// Extract a copy of `[from, to)` as standalone inline content (for copy/paste and moves).
pub fn slice(inlines: &[Inline], from: usize, to: usize) -> Vec<Inline> {
    let (_, rest) = partition(inlines.to_vec(), from);
    let (mid, _) = partition(rest, to - from);
    mid
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(t: &str, bold: bool) -> Inline {
        let mut rpr = Element::new("w:rPr");
        if bold {
            rpr.children.push(crate::xml::Node::Element(Element::new("w:b")));
        }
        Inline::Run(Run { t_attrs: None, attrs: vec![], rpr: Some(rpr), items: vec![RunItem::Text(t.into())] })
    }

    fn text(inl: &[Inline]) -> String {
        inl.iter().map(|i| i.text()).collect()
    }

    #[test]
    fn insert_delete_format() {
        let mut v = vec![run("Hello ", false), run("world", true)];
        insert_text(&mut v, 6, "big ", None);
        assert_eq!(text(&v), "Hello big world");
        // typed at end of first run -> inherits non-bold
        if let Inline::Run(r) = &v[0] {
            assert_eq!(r.text(), "Hello big ");
        }
        delete_range(&mut v, 2, 8);
        assert_eq!(text(&v), "Heg world");
        format_range(&mut v, 0, 3, &mut |r| {
            r.rpr = Some(Element::new("w:rPr").with_child(Element::new("w:b")));
        });
        assert_eq!(text(&v), "Heg world");
        // "Heg" now bold and merges with bold " world"? no: " world" was non-bold? It was "world" bold
        let (a, b) = partition(v.clone(), 3);
        assert_eq!(text(&a), "Heg");
        assert_eq!(text(&b), " world");
    }

    #[test]
    fn emoji_offsets() {
        let mut v = vec![run("a😀b", false)];
        insert_text(&mut v, 3, "X", None);
        assert_eq!(text(&v), "a😀Xb");
        delete_range(&mut v, 1, 3);
        assert_eq!(text(&v), "aXb");
    }

    #[test]
    fn hyperlink_boundary() {
        let link = Inline::Container(InlineContainer {
            kind: InlineKind::Hyperlink,
            shell: Element::new("w:hyperlink"),
            content_in: None,
            children: vec![run("link", false)],
        });
        let mut v = vec![run("a ", false), link];
        insert_text(&mut v, 6, "!", None);
        assert_eq!(v.len(), 3, "typing after a link must not extend it");
        insert_text(&mut v, 4, "X", None);
        if let Inline::Container(c) = &v[1] {
            assert_eq!(c.children.iter().map(|i| i.text()).collect::<String>(), "liXnk");
        }
    }
}
