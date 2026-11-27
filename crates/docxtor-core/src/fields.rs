//! Fields: reading their instructions, finding them inside paragraphs, building Word's markup for them and
//! replacing their results. Word keeps a field as `w:fldChar` begin, `w:instrText`, `w:fldChar` separate, the
//! result runs and `w:fldChar` end (or as a `w:fldSimple` element). Only the result runs are ever replaced,
//! the first result run's formatting is kept, and anything that isn't plain text is left alone.

use crate::edit;
use crate::model::*;
use crate::xml::Element;

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// One word of a field instruction. Quoted words are kept as written between the quotes (escapes included).
#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    pub text: String,
    pub quoted: bool,
}

impl Tok {
    pub fn is_switch(&self) -> bool {
        !self.quoted && self.text.len() >= 2 && self.text.starts_with('\\')
    }
}

/// A parsed field instruction: ` REF _Ref123 \h ` is the code `REF` and three words.
#[derive(Debug, Clone, PartialEq)]
pub struct Instr {
    /// The field name in capitals (`REF`, `SEQ`, `XE`, ...).
    pub code: String,
    /// The words after the name, in order.
    pub toks: Vec<Tok>,
}

impl Instr {
    pub fn parse(s: &str) -> Instr {
        let mut toks = tokenize(s);
        if toks.is_empty() {
            return Instr { code: String::new(), toks };
        }
        let first = toks.remove(0);
        // A formula can be written without a space after the equals sign: `=SUM(ABOVE)`.
        if !first.quoted && first.text.len() > 1 && first.text.starts_with('=') {
            toks.insert(0, Tok { text: first.text[1..].to_string(), quoted: false });
            return Instr { code: "=".into(), toks };
        }
        Instr { code: first.text.to_uppercase(), toks }
    }

    /// The first word that isn't a switch or a switch's value: a bookmark, a sequence name, an entry.
    pub fn first_arg(&self) -> Option<&str> {
        self.toks.first().filter(|t| !t.is_switch()).map(|t| t.text.as_str())
    }

    /// Whether the switch (given without its backslash, `h` for `\h`) is present. Case-insensitive.
    pub fn has(&self, switch: &str) -> bool {
        self.toks.iter().any(|t| t.is_switch() && t.text[1..].eq_ignore_ascii_case(switch))
    }

    /// The word after a switch that takes a value (`\r 5`, `\c "Figure"`), unescaped.
    pub fn value(&self, switch: &str) -> Option<String> {
        let i = self.toks.iter().position(|t| t.is_switch() && t.text[1..].eq_ignore_ascii_case(switch))?;
        let v = self.toks.get(i + 1).filter(|t| !t.is_switch())?;
        Some(unescape(&v.text))
    }

    /// The words that follow `\*` switches (`ARABIC`, `MERGEFORMAT`, `Upper`).
    pub fn formats(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, t) in self.toks.iter().enumerate() {
            if t.is_switch() && t.text == "\\*" {
                if let Some(v) = self.toks.get(i + 1).filter(|t| !t.is_switch()) {
                    out.push(v.text.clone());
                }
            }
        }
        out
    }
}

fn tokenize(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '"' || c == '\u{201C}' {
            // A value ends with the kind of quote that opened it, so a curly quote inside a straight-quoted value is text.
            let close = if c == '"' { '"' } else { '\u{201D}' };
            i += 1;
            let mut text = String::new();
            while i < chars.len() {
                match chars[i] {
                    '\\' if i + 1 < chars.len() => {
                        text.push('\\');
                        text.push(chars[i + 1]);
                        i += 2;
                    }
                    q if q == close => {
                        i += 1;
                        break;
                    }
                    ch => {
                        text.push(ch);
                        i += 1;
                    }
                }
            }
            out.push(Tok { text, quoted: true });
        } else {
            let mut text = String::new();
            while i < chars.len() && !chars[i].is_whitespace() {
                text.push(chars[i]);
                i += 1;
            }
            out.push(Tok { text, quoted: false });
        }
    }
    out
}

/// `\"` to `"`, `\\` to `\`, `\:` to `:`; other backslashes stay.
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            if let Some(&n) = it.peek() {
                if matches!(n, '"' | '\\' | ':') {
                    out.push(n);
                    it.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// Escape text for a quoted word of an instruction (`"` and `\`; `:` too when it must not split an index entry).
pub fn escape(s: &str, colon: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '"' || c == '\\' || (colon && c == ':') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The bookmark a `REF`, `PAGEREF` or `NOTEREF` field points at, and whether it is a link (`\h`).
pub fn ref_target(instr: &str) -> Option<(String, bool)> {
    let i = Instr::parse(instr);
    if !matches!(i.code.as_str(), "REF" | "PAGEREF" | "NOTEREF") {
        return None;
    }
    Some((unescape(i.first_arg()?), i.has("h")))
}

// ---------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------

fn fld(kind: &str) -> RunItem {
    RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", kind))
}

fn run_of(rpr: Option<Element>, item: RunItem) -> Inline {
    Inline::Run(Run { attrs: vec![], t_attrs: None, rpr, items: vec![item] })
}

/// A complex field as Word writes it, one run per part: begin, instruction, separate, result (if any), end.
/// `hidden` marks every run hidden (`w:vanish`), as Word does for index entries.
pub fn complex(instr: &str, result: &str, result_rpr: Option<Element>, hidden: bool) -> Vec<Inline> {
    let part_rpr = hidden.then(|| Element::new("w:rPr").with_child(Element::new("w:vanish")));
    let mut out = vec![
        run_of(part_rpr.clone(), fld("begin")),
        run_of(part_rpr.clone(), RunItem::Hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(instr))),
    ];
    if !hidden || !result.is_empty() {
        out.push(run_of(part_rpr.clone(), fld("separate")));
        if !result.is_empty() {
            out.push(Inline::Run(Run::of_text(result_rpr, result)));
        }
    }
    out.push(run_of(part_rpr, fld("end")));
    out
}

/// A run's `w:rPr` with `w:noProof`, which Word puts on number results.
pub fn no_proof() -> Element {
    Element::new("w:rPr").with_child(Element::new("w:noProof"))
}

/// Insert inlines at a UTF-16 offset, keeping their order (like `edit::insert_inline_at`, for several).
pub fn insert_inlines_at(inlines: &mut Vec<Inline>, off: usize, new: Vec<Inline>) {
    edit::split_at(inlines, off);
    let mut start = 0;
    for i in 0..inlines.len() {
        let len = inlines[i].len16();
        if start == off {
            // Right after a field's result the closing mark comes first: go past it, so the new content is
            // beside the field and not inside it.
            let mut i = i;
            while matches!(inlines.get(i), Some(Inline::Run(r)) if !r.items.is_empty() && r.items.iter().all(|it| matches!(it, RunItem::Hidden(e) if e.name == "w:fldChar" && e.attr("w:fldCharType") == Some("end")))) {
                i += 1;
            }
            for (k, n) in new.into_iter().enumerate() {
                inlines.insert(i + k, n);
            }
            return;
        }
        if start < off && off < start + len {
            if let Inline::Container(c) = &mut inlines[i] {
                insert_inlines_at(&mut c.children, off - start, new);
                return;
            }
        }
        start += len;
    }
    inlines.extend(new);
}

// ---------------------------------------------------------------------------
// Finding and updating
// ---------------------------------------------------------------------------

/// A field seen while walking a paragraph.
#[derive(Debug, Clone)]
pub struct Seen<'a> {
    pub instr: &'a str,
    /// The result as text (empty when the field has none).
    pub result: &'a str,
    /// UTF-16 offset of the field in the paragraph's text.
    pub offset: usize,
    /// The result is only text (runs of text, tabs, breaks), so it can be replaced.
    pub plain: bool,
    /// The field is locked (`w:fldLock`): it is never updated.
    pub locked: bool,
    /// The field is marked as needing an update (`w:dirty`).
    pub dirty: bool,
}

/// What to put in a field's result instead.
#[derive(Debug, Clone, Default)]
pub struct Replace {
    pub text: String,
    /// A character style for the new result (`FootnoteReference`), instead of the old result's formatting.
    pub rstyle: Option<String>,
    /// A font for the new result (a `SYMBOL` field's `\f`).
    pub font: Option<String>,
    /// A size in half-points for the new result (a `SYMBOL` field's `\s`).
    pub size: Option<u32>,
}

impl From<String> for Replace {
    fn from(text: String) -> Self {
        Replace { text, ..Default::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokKind {
    Begin,
    Sep,
    End,
}

struct Span {
    begin: (usize, usize),
    sep: Option<(usize, usize)>,
    end: (usize, usize),
    instr: String,
    result: String,
    plain: bool,
    locked: bool,
    dirty: bool,
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some("1" | "true" | "on"))
}

/// The complex fields whose parts all sit in this list of inlines (not looking inside links and other
/// wrappers), in the order they begin. Nested fields are listed too.
fn analyze(level: &[Inline]) -> Vec<Span> {
    struct Open {
        begin: (usize, usize),
        sep: Option<(usize, usize)>,
        instr: String,
        locked: bool,
        dirty: bool,
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut spans = Vec::new();
    for (i, inl) in level.iter().enumerate() {
        let Inline::Run(r) = inl else { continue };
        for (k, item) in r.items.iter().enumerate() {
            let tok = match item {
                RunItem::Hidden(e) if e.name == "w:fldChar" => match e.attr("w:fldCharType") {
                    Some("begin") => Some(TokKind::Begin),
                    Some("separate") => Some(TokKind::Sep),
                    Some("end") => Some(TokKind::End),
                    _ => None,
                },
                _ => None,
            };
            match (tok, item) {
                (Some(TokKind::Begin), RunItem::Hidden(b)) => {
                    stack.push(Open { begin: (i, k), sep: None, instr: String::new(), locked: truthy(b.attr("w:fldLock")), dirty: truthy(b.attr("w:dirty")) })
                }
                (Some(TokKind::Sep), _) => {
                    if let Some(top) = stack.last_mut() {
                        top.sep = Some((i, k));
                    }
                }
                (Some(TokKind::End), _) => {
                    if let Some(open) = stack.pop() {
                        let (result, plain) = match open.sep {
                            Some(sep) => result_of(level, sep, (i, k)),
                            None => (String::new(), true),
                        };
                        // A field inside another one's instruction (an `IF` with a nested `DATE`) puts its result there.
                        if let Some(parent) = stack.last_mut().filter(|p| p.sep.is_none()) {
                            parent.instr.push_str(&result);
                        }
                        spans.push(Span { begin: open.begin, sep: open.sep, end: (i, k), instr: open.instr, result, plain, locked: open.locked, dirty: open.dirty });
                    }
                }
                (None, RunItem::Hidden(e)) if e.name == "w:instrText" => {
                    if let Some(top) = stack.last_mut().filter(|t| t.sep.is_none()) {
                        top.instr.push_str(&e.text());
                    }
                }
                // Word writes a tab inside an index instruction (`INDEX \e "<tab>"`) as a tab run.
                (None, RunItem::Tab) => {
                    if let Some(top) = stack.last_mut().filter(|t| t.sep.is_none() && !t.instr.is_empty()) {
                        top.instr.push('\t');
                    }
                }
                _ => {}
            }
        }
    }
    spans.sort_by_key(|s| s.begin);
    spans
}

fn result_of(level: &[Inline], sep: (usize, usize), end: (usize, usize)) -> (String, bool) {
    let mut text = String::new();
    let mut plain = true;
    for (j, inl) in level.iter().enumerate().take(end.0 + 1).skip(sep.0) {
        match inl {
            Inline::Run(r) => {
                let from = if j == sep.0 { sep.1 + 1 } else { 0 };
                let to = if j == end.0 { end.1 } else { r.items.len() };
                for item in r.items.iter().take(to).skip(from) {
                    match item {
                        RunItem::Text(t) => text.push_str(t),
                        RunItem::Tab => text.push('\t'),
                        RunItem::Break(_) => {
                            text.push('\n');
                        }
                        RunItem::Hidden(e) if e.name == "w:lastRenderedPageBreak" => {}
                        RunItem::Hidden(_) | RunItem::Object(_) => plain = false,
                    }
                }
            }
            Inline::Opaque(e) if matches!(e.name.as_str(), "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr") => {}
            other => {
                text.push_str(&other.text());
                plain = false;
            }
        }
    }
    (text, plain)
}

fn new_result_run(rpr: Option<Element>, rep: &Replace) -> Option<Inline> {
    let text = rep.text.as_str();
    if text.is_empty() {
        return None;
    }
    let mut rpr = match rep.rstyle.as_deref() {
        Some(s) => Some(Element::new("w:rPr").with_child(Element::new("w:rStyle").with_attr("w:val", s))),
        None => rpr,
    };
    if rep.font.is_some() || rep.size.is_some() {
        let r = rpr.get_or_insert_with(|| Element::new("w:rPr"));
        if let Some(f) = &rep.font {
            let mut el = Element::new("w:rFonts");
            for a in ["w:ascii", "w:hAnsi", "w:cs"] {
                el.set_attr(a, f.clone());
            }
            crate::props::put_rpr(r, el);
        }
        if let Some(sz) = rep.size {
            crate::props::put_rpr(r, Element::new("w:sz").with_attr("w:val", sz.to_string()));
            crate::props::put_rpr(r, Element::new("w:szCs").with_attr("w:val", sz.to_string()));
        }
    }
    Some(Inline::Run(Run::of_text(rpr, text)))
}

/// Replace the result of a complex field in place.
fn apply_span(level: &mut Vec<Inline>, span: &Span, new: &Replace) {
    let Some(sep) = span.sep else { return };
    let end = span.end;
    // The new result looks like the first piece of the old one.
    let mut rpr = None;
    let mut found = false;
    for (j, inl) in level.iter().enumerate().take(end.0 + 1).skip(sep.0) {
        if let Inline::Run(r) = inl {
            let from = if j == sep.0 { sep.1 + 1 } else { 0 };
            let to = if j == end.0 { end.1 } else { r.items.len() };
            if r.items.iter().take(to).skip(from).any(|it| matches!(it, RunItem::Text(_))) {
                rpr = r.rpr.clone();
                found = true;
                break;
            }
        }
    }
    if !found {
        rpr = match &level[sep.0] {
            Inline::Run(r) => r.rpr.clone().map(|mut p| {
                p.remove_children("w:vanish");
                p
            }),
            _ => None,
        }
        .filter(|p| !p.children.is_empty());
    }
    let run = new_result_run(rpr, new);
    if sep.0 == end.0 {
        let Inline::Run(r) = &level[sep.0] else { return };
        let mut left = r.clone();
        let mut right = r.clone();
        left.items.truncate(sep.1 + 1);
        right.items.drain(..end.1);
        let mut rep = vec![Inline::Run(left)];
        rep.extend(run);
        rep.push(Inline::Run(right));
        level.splice(sep.0..=sep.0, rep);
        return;
    }
    let kept: Vec<Inline> = level[sep.0 + 1..end.0].iter().filter(|i| matches!(i, Inline::Opaque(_))).cloned().collect();
    if let Inline::Run(r) = &mut level[end.0] {
        r.items.drain(..end.1);
    }
    if let Inline::Run(r) = &mut level[sep.0] {
        r.items.truncate(sep.1 + 1);
    }
    let mut mid: Vec<Inline> = run.into_iter().collect();
    mid.extend(kept);
    level.splice(sep.0 + 1..end.0, mid);
}

/// The fields in this list of inlines that show something: the index of the inline where each starts, the
/// index of the one where it ends, and how much text (UTF-16 units) the whole field takes. A field whose
/// result is deleted should go with it, as in Word, which treats a field as one piece.
pub fn visible_fields(inlines: &[Inline]) -> Vec<(usize, usize, usize)> {
    let mut out: Vec<(usize, usize, usize)> = analyze(inlines)
        .iter()
        // A form field keeps its place when its text is deleted: it is a blank to fill in.
        .filter(|s| s.sep.is_some() && !matches!(s.instr.split_whitespace().next().map(str::to_uppercase).as_deref(), Some("FORMTEXT" | "FORMCHECKBOX" | "FORMDROPDOWN")))
        .map(|s| (s.begin.0, s.end.0, inlines[s.begin.0..=s.end.0].iter().map(|i| i.len16()).sum()))
        .collect();
    for (i, inl) in inlines.iter().enumerate() {
        if matches!(inl, Inline::Container(c) if c.kind == InlineKind::Field) {
            out.push((i, i, inl.len16()));
        }
    }
    out.retain(|(_, _, len)| *len > 0);
    out
}

/// Walk the fields of a paragraph's inlines in document order. `f` sees each field and may return a new
/// result for it; fields it returns nothing for, and fields whose result isn't plain text, are untouched.
/// Returns whether anything changed.
pub fn update_inlines(inlines: &mut Vec<Inline>, base: usize, f: &mut dyn FnMut(&Seen) -> Option<Replace>) -> bool {
    let spans = analyze(inlines);
    // Offsets of each inline.
    let mut starts = Vec::with_capacity(inlines.len());
    let mut at = base;
    for i in inlines.iter() {
        starts.push(at);
        at += i.len16();
    }
    // Document order: complex fields by where they begin, containers where they stand.
    enum Work {
        Span(usize),
        Container(usize),
    }
    let mut work: Vec<(usize, usize, Work)> = Vec::new();
    for (n, s) in spans.iter().enumerate() {
        work.push((s.begin.0, s.begin.1, Work::Span(n)));
    }
    for (i, inl) in inlines.iter().enumerate() {
        if matches!(inl, Inline::Container(_)) {
            work.push((i, 0, Work::Container(i)));
        }
    }
    work.sort_by_key(|(i, k, _)| (*i, *k));
    let mut changed = false;
    let mut pending: Vec<(usize, Replace)> = Vec::new();
    let mut clear_dirty: Vec<(usize, usize)> = Vec::new();
    for (_, _, w) in work {
        match w {
            Work::Span(n) => {
                let s = &spans[n];
                let within: usize = match &inlines[s.begin.0] {
                    Inline::Run(r) => r.items.iter().take(s.begin.1).map(|it| it.len16()).sum(),
                    _ => 0,
                };
                let seen = Seen { instr: &s.instr, result: &s.result, offset: starts[s.begin.0] + within, plain: s.plain, locked: s.locked, dirty: s.dirty };
                if let Some(rep) = f(&seen) {
                    // A locked field keeps its result.
                    if s.plain && s.sep.is_some() && !s.locked && (rep.text != s.result || rep.rstyle.is_some() || rep.font.is_some() || rep.size.is_some()) {
                        pending.push((n, rep));
                    } else if s.dirty && !s.locked {
                        clear_dirty.push(s.begin);
                    }
                }
            }
            Work::Container(i) => {
                let Inline::Container(c) = &mut inlines[i] else { continue };
                let off = starts[i];
                if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some() {
                    let instr = c.shell.attr("w:instr").unwrap_or_default().to_string();
                    let result: String = c.children.iter().map(|x| x.text()).collect();
                    let plain = c.children.iter().all(|x| matches!(x, Inline::Run(r) if r.items.iter().all(|it| matches!(it, RunItem::Text(_) | RunItem::Tab | RunItem::Break(_)))));
                    let locked = truthy(c.shell.attr("w:fldLock"));
                    let dirty = truthy(c.shell.attr("w:dirty"));
                    let seen = Seen { instr: &instr, result: &result, offset: off, plain, locked, dirty };
                    if let Some(rep) = f(&seen).filter(|_| !locked) {
                        if dirty {
                            c.shell.remove_attr("w:dirty");
                            changed = true;
                        }
                        if plain && (rep.text != result || rep.rstyle.is_some() || rep.font.is_some() || rep.size.is_some()) {
                            let rpr = c.children.iter().find_map(|x| match x {
                                Inline::Run(r) => r.rpr.clone(),
                                _ => None,
                            });
                            c.children = new_result_run(rpr, &rep).into_iter().collect();
                            changed = true;
                        }
                    }
                } else {
                    changed |= update_inlines(&mut c.children, off, f);
                }
            }
        }
    }
    // A field that holds another pending field waits for the next pass, which sees the inner result.
    let outer: Vec<usize> = pending.iter().map(|(n, _)| *n).filter(|n| pending.iter().any(|(m, _)| spans[*m].begin > spans[*n].begin && spans[*m].end < spans[*n].end)).collect();
    pending.retain(|(n, _)| !outer.contains(n));
    for (n, _) in &pending {
        if spans[*n].dirty {
            clear_dirty.push(spans[*n].begin);
        }
    }
    // Clear the marks first: replacing results moves the runs after them.
    for (i, k) in clear_dirty {
        if let Some(Inline::Run(r)) = inlines.get_mut(i) {
            if let Some(RunItem::Hidden(e)) = r.items.get_mut(k) {
                if e.attr("w:dirty").is_some() {
                    e.remove_attr("w:dirty");
                    changed = true;
                }
            }
        }
    }
    for (n, rep) in pending.into_iter().rev() {
        apply_span(inlines, &spans[n], &rep);
        changed = true;
    }
    changed
}

/// Walk the fields of a paragraph inner first: a field is visited after the fields inside its code, and its
/// instruction already holds their new results (the order a mail merge needs, where `IF { MERGEFIELD x } = "a"` reads
/// the value of `x`). Siblings keep document order. `f` may return a new result for a field; a field it returns
/// nothing for, a locked one, and one whose result isn't plain text, are left as they are. Returns whether anything
/// changed.
pub fn update_inner_first(inlines: &mut Vec<Inline>, f: &mut dyn FnMut(&Seen) -> Option<Replace>) -> bool {
    enum Next {
        Span(usize),
        Container(usize),
    }
    let mut changed = false;
    // Where the last field visited ended, in the current coordinates of the list.
    let mut last: Option<(usize, usize)> = None;
    // Each visit moves `last` forward, so this ends; the bound only guards against a bug.
    for _ in 0..100_000 {
        let spans = analyze(inlines);
        let after = |key: (usize, usize)| last.is_none_or(|l| key > l);
        let mut best: Option<((usize, usize), Next)> = None;
        for (n, s) in spans.iter().enumerate() {
            if after(s.end) && best.as_ref().is_none_or(|(k, _)| s.end < *k) {
                best = Some((s.end, Next::Span(n)));
            }
        }
        for (i, inl) in inlines.iter().enumerate() {
            if matches!(inl, Inline::Container(_)) && after((i, 0)) && best.as_ref().is_none_or(|(k, _)| (i, 0) < *k) {
                best = Some(((i, 0), Next::Container(i)));
            }
        }
        let Some((key, next)) = best else { break };
        last = Some(key);
        match next {
            Next::Span(n) => {
                let s = &spans[n];
                let within: usize = match &inlines[s.begin.0] {
                    Inline::Run(r) => r.items.iter().take(s.begin.1).map(|it| it.len16()).sum(),
                    _ => 0,
                };
                let offset: usize = inlines[..s.begin.0].iter().map(|i| i.len16()).sum::<usize>() + within;
                let seen = Seen { instr: &s.instr, result: &s.result, offset, plain: s.plain, locked: s.locked, dirty: s.dirty };
                if let Some(rep) = f(&seen) {
                    if s.plain && s.sep.is_some() && !s.locked && (rep.text != s.result || rep.rstyle.is_some() || rep.font.is_some() || rep.size.is_some()) {
                        apply_span(inlines, s, &rep);
                        changed = true;
                        // The result's runs changed, so the field ends somewhere else now.
                        if let Some(moved) = analyze(inlines).into_iter().find(|m| m.begin == s.begin) {
                            last = Some(moved.end);
                        }
                    }
                }
            }
            Next::Container(i) => {
                let Inline::Container(c) = &mut inlines[i] else { continue };
                if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some() {
                    let instr = c.shell.attr("w:instr").unwrap_or_default().to_string();
                    let result: String = c.children.iter().map(|x| x.text()).collect();
                    let plain = c.children.iter().all(|x| matches!(x, Inline::Run(r) if r.items.iter().all(|it| matches!(it, RunItem::Text(_) | RunItem::Tab | RunItem::Break(_)))));
                    let locked = truthy(c.shell.attr("w:fldLock"));
                    let seen = Seen { instr: &instr, result: &result, offset: 0, plain, locked, dirty: false };
                    if let Some(rep) = f(&seen).filter(|r| !locked && plain && r.text != result) {
                        let rpr = c.children.iter().find_map(|x| match x {
                            Inline::Run(r) => r.rpr.clone(),
                            _ => None,
                        });
                        c.children = new_result_run(rpr, &rep).into_iter().collect();
                        changed = true;
                    }
                } else {
                    changed |= update_inner_first(&mut c.children, f);
                }
            }
        }
    }
    changed
}

/// Read-only walk of a paragraph's fields in document order.
pub fn scan_inlines(inlines: &[Inline], base: usize, f: &mut dyn FnMut(&Seen)) {
    let spans = analyze(inlines);
    let mut starts = Vec::with_capacity(inlines.len());
    let mut at = base;
    for i in inlines {
        starts.push(at);
        at += i.len16();
    }
    let mut work: Vec<(usize, usize, Option<usize>)> = spans.iter().enumerate().map(|(n, s)| (s.begin.0, s.begin.1, Some(n))).collect();
    for (i, inl) in inlines.iter().enumerate() {
        if matches!(inl, Inline::Container(_)) {
            work.push((i, 0, None));
        }
    }
    work.sort_by_key(|(i, k, _)| (*i, *k));
    for (i, _, n) in work {
        match n {
            Some(n) => {
                let s = &spans[n];
                let within: usize = match &inlines[s.begin.0] {
                    Inline::Run(r) => r.items.iter().take(s.begin.1).map(|it| it.len16()).sum(),
                    _ => 0,
                };
                f(&Seen { instr: &s.instr, result: &s.result, offset: starts[s.begin.0] + within, plain: s.plain, locked: s.locked, dirty: s.dirty });
            }
            None => {
                let Inline::Container(c) = &inlines[i] else { continue };
                if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some() {
                    let instr = c.shell.attr("w:instr").unwrap_or_default();
                    let result: String = c.children.iter().map(|x| x.text()).collect();
                    f(&Seen { instr, result: &result, offset: starts[i], plain: true, locked: truthy(c.shell.attr("w:fldLock")), dirty: truthy(c.shell.attr("w:dirty")) });
                } else {
                    scan_inlines(&c.children, starts[i], f);
                }
            }
        }
    }
}

/// Walk the fields of every paragraph (tables and content controls included) in document order.
pub fn scan_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>, f: &mut dyn FnMut(NodeId, &Seen)) {
    walk_paragraphs(blocks, &mut |p| scan_inlines(&p.inlines, 0, &mut |s| f(p.id, s)));
}

/// Update the fields of every paragraph in document order. `f` also gets the paragraph's id. Returns the
/// ids of the paragraphs that changed.
pub fn update_blocks(blocks: &mut [Block], f: &mut dyn FnMut(NodeId, &Seen) -> Option<Replace>) -> Vec<NodeId> {
    let mut changed = Vec::new();
    walk_paragraphs_mut(blocks, &mut |p| {
        let id = p.id;
        if update_inlines(&mut p.inlines, 0, &mut |s| f(id, s)) {
            changed.push(id);
        }
    });
    changed
}

/// Whether a block has a field anywhere (read only: what `update_inlines` could change, `scan_inlines` sees).
pub fn block_has_field(b: &Block) -> bool {
    let mut any = false;
    scan_blocks(std::iter::once(b), &mut |_, _| any = true);
    any
}

/// `update_blocks` for the body: only the blocks with a field that changes are copied from an undo step that shares them,
/// and a block with no field is not even copied to look (most of a long document).
pub fn update_blocks_body(body: &mut crate::model::Body, f: &mut dyn FnMut(NodeId, &Seen) -> Option<Replace>) -> Vec<NodeId> {
    let mut changed = Vec::new();
    body.edit_where(block_has_field, |s| {
        let c = update_blocks(s, f);
        let any = !c.is_empty();
        changed.extend(c);
        any
    });
    changed
}

/// Whether a paragraph holds a field with this code (`XE`, `SEQ`, ...) anywhere in its text.
pub fn has_field(p: &Paragraph, code: &str) -> bool {
    let mut found = false;
    scan_inlines(&p.inlines, 0, &mut |s| found |= Instr::parse(s.instr).code == code);
    found
}

// ---------------------------------------------------------------------------
// Links, formulas, and editing a field's own markup
// ---------------------------------------------------------------------------

/// Where a `HYPERLINK` field points: an address, a place in the document (`\l "bookmark"`) or both
/// (`address#place`). Internal places come back as `#bookmark`.
pub fn hyperlink_target(instr: &str) -> Option<String> {
    let i = Instr::parse(instr);
    if i.code != "HYPERLINK" {
        return None;
    }
    let url = i.first_arg().map(unescape).filter(|u| !u.is_empty());
    let place = i.value("l").filter(|p| !p.is_empty());
    match (url, place) {
        (Some(u), Some(p)) => Some(format!("{u}#{p}")),
        (Some(u), None) => Some(u),
        (None, Some(p)) => Some(format!("#{p}")),
        (None, None) => None,
    }
}

/// The link a field's result makes: a `HYPERLINK`, or a cross-reference with `\h`.
pub fn link_of(instr: &str) -> Option<String> {
    hyperlink_target(instr).or_else(|| {
        let (name, hyper) = ref_target(instr)?;
        hyper.then(|| format!("#{name}"))
    })
}

/// The expression of a formula field (` = SUM(ABOVE) \# "0.00" ` gives `SUM(ABOVE)`): the text after the equals
/// sign, up to the first switch.
pub fn formula_expr(instr: &str) -> Option<String> {
    let t = instr.trim_start();
    let rest = t.strip_prefix('=')?;
    let mut out = String::new();
    let mut quoted = false;
    let mut prev_space = true;
    for c in rest.chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if c == '\\' && !quoted && prev_space {
            break;
        }
        prev_space = c.is_whitespace();
        out.push(c);
    }
    Some(out.trim().to_string())
}

/// Remove the field markup (begin, instruction, separate, end) of the complex fields in this list of inlines
/// that overlap the text from `from` to `to` and that `pick` accepts, keeping their results as plain text. Used
/// when a link is edited: a `HYPERLINK` field becomes ordinary runs that the link can wrap. Returns whether
/// anything changed.
pub fn unwrap_fields(inlines: &mut Vec<Inline>, from: usize, to: usize, pick: &dyn Fn(&str) -> bool) -> bool {
    let mut changed = false;
    // Each pass removes one field and looks again, because the positions of the others move.
    for _ in 0..64 {
        let spans = analyze(inlines);
        let mut starts = Vec::with_capacity(inlines.len() + 1);
        let mut at = 0;
        for i in inlines.iter() {
            starts.push(at);
            at += i.len16();
        }
        starts.push(at);
        let hit = spans.iter().rev().find(|s| {
            let a = starts[s.begin.0];
            let b = starts[s.end.0 + 1];
            pick(&s.instr) && a <= to.max(from + 1) && b >= from && (b > from || a == from)
        });
        let Some(s) = hit else { break };
        let (begin, sep, end) = (s.begin, s.sep, s.end);
        remove_items(inlines, end, end);
        match sep {
            Some(sep) => remove_items(inlines, begin, sep),
            None => remove_items(inlines, begin, (end.0, end.1.saturating_sub(1))),
        }
        changed = true;
    }
    changed
}

/// Remove the run items from `a` to `b` (inclusive), and the runs that end up empty.
fn remove_items(level: &mut Vec<Inline>, a: (usize, usize), b: (usize, usize)) {
    if b.0 < a.0 || (b.0 == a.0 && b.1 < a.1) {
        return;
    }
    for i in a.0..=b.0 {
        if let Some(Inline::Run(r)) = level.get_mut(i) {
            let lo = if i == a.0 { a.1 } else { 0 };
            let hi = if i == b.0 { b.1.min(r.items.len().saturating_sub(1)) } else { r.items.len().saturating_sub(1) };
            if lo <= hi && lo < r.items.len() {
                r.items.drain(lo..=hi);
            }
        }
    }
    for i in (a.0..=b.0).rev() {
        if matches!(level.get(i), Some(Inline::Run(r)) if r.items.is_empty()) {
            level.remove(i);
        }
    }
}

/// Give the field that starts at this offset a new instruction. Its result is left as it is (the caller updates
/// it). Returns whether a field was found.
pub fn set_instr(inlines: &mut Vec<Inline>, at: usize, base: usize, instr: &str) -> bool {
    let spans = analyze(inlines);
    let mut starts = Vec::with_capacity(inlines.len());
    let mut pos = base;
    for i in inlines.iter() {
        starts.push(pos);
        pos += i.len16();
    }
    for s in &spans {
        let within: usize = match &inlines[s.begin.0] {
            Inline::Run(r) => r.items.iter().take(s.begin.1).map(|it| it.len16()).sum(),
            _ => 0,
        };
        if starts[s.begin.0] + within != at {
            continue;
        }
        let stop = s.sep.unwrap_or(s.end);
        // The first instruction item takes the new text; the others (and tabs inside it) go.
        let mut first: Option<(usize, usize)> = None;
        let mut drop: Vec<(usize, usize)> = Vec::new();
        for (i, inl) in inlines.iter().enumerate().take(stop.0 + 1).skip(s.begin.0) {
            let Inline::Run(r) = inl else { continue };
            let lo = if i == s.begin.0 { s.begin.1 + 1 } else { 0 };
            let hi = if i == stop.0 { stop.1 } else { r.items.len() };
            for k in lo..hi.min(r.items.len()) {
                match &r.items[k] {
                    RunItem::Hidden(e) if e.name == "w:instrText" => {
                        if first.is_none() {
                            first = Some((i, k));
                        } else {
                            drop.push((i, k));
                        }
                    }
                    RunItem::Tab if first.is_some() => drop.push((i, k)),
                    _ => {}
                }
            }
        }
        let Some((fi, fk)) = first else { return false };
        // Items after the first one are removed from the back, so the first keeps its place.
        for (i, k) in drop.into_iter().rev() {
            remove_items(inlines, (i, k), (i, k));
        }
        if let Some(Inline::Run(r)) = inlines.get_mut(fi) {
            if let Some(RunItem::Hidden(e)) = r.items.get_mut(fk) {
                *e = Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(instr);
            }
        }
        return true;
    }
    for (i, inl) in inlines.iter_mut().enumerate() {
        if let Inline::Container(c) = inl {
            let off = starts[i];
            let len: usize = c.children.iter().map(|x| x.len16()).sum();
            if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some() {
                if off == at {
                    c.shell.set_attr("w:instr", instr);
                    return true;
                }
            } else if at >= off && at <= off + len && set_instr(&mut c.children, at, off, instr) {
                return true;
            }
        }
    }
    false
}

/// The field codes whose value depends on where the field is laid out, so the page layout supplies it.
pub fn is_page_dependent(code: &str) -> bool {
    matches!(code, "PAGE" | "NUMPAGES" | "SECTIONPAGES" | "STYLEREF")
}

// ---------------------------------------------------------------------------
// Sequence numbers
// ---------------------------------------------------------------------------

/// The number a `SEQ` field shows for a sequence: `\* ROMAN`, `\* alphabetic` and the like.
pub fn seq_text(n: i64, instr: &Instr) -> String {
    let fmt = instr.formats().into_iter().find_map(|f| match f.as_str() {
        "roman" => Some("lowerRoman"),
        "ROMAN" => Some("upperRoman"),
        "alphabetic" => Some("lowerLetter"),
        "ALPHABETIC" => Some("upperLetter"),
        _ => None,
    });
    crate::notes::format_number(n, fmt.unwrap_or("decimal"))
}

/// Keeps the count of every `SEQ` sequence while walking the document. Sequence names are case-insensitive.
#[derive(Default)]
pub struct SeqCounter {
    counts: std::collections::HashMap<String, i64>,
}

impl SeqCounter {
    /// The text of the next `SEQ` field of its sequence (`\r n` restarts at n, `\c` repeats the last number,
    /// `\h` hides the result), or None when the field isn't a usable `SEQ` field.
    pub fn next(&mut self, instr: &str) -> Option<String> {
        self.next_in(instr, &|_| 0)
    }

    /// Like [`next`](SeqCounter::next), for numbers that restart under a heading (`\s 1`, as in "Figure 2-1"):
    /// `chapter(level)` says which heading of that level the field is under, as any number that changes with it.
    pub fn next_in(&mut self, instr: &str, chapter: &dyn Fn(u8) -> usize) -> Option<String> {
        let i = Instr::parse(instr);
        if i.code != "SEQ" {
            return None;
        }
        let mut name = i.first_arg()?.to_lowercase();
        if let Some(level) = i.value("s").and_then(|v| v.trim().parse::<u8>().ok()) {
            name = format!("{name}\u{0}{level}:{}", chapter(level));
        }
        let slot = self.counts.entry(name).or_insert(0);
        if let Some(r) = i.value("r").and_then(|v| v.trim().parse::<i64>().ok()) {
            *slot = r;
        } else if !i.has("c") {
            *slot += 1;
        }
        if i.has("h") {
            return Some(String::new());
        }
        Some(seq_text(*slot, &i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions() {
        let i = Instr::parse(r#" SEQ Figure \* ARABIC \r 3 "#);
        assert_eq!(i.code, "SEQ");
        assert_eq!(i.first_arg(), Some("Figure"));
        assert_eq!(i.value("r").as_deref(), Some("3"));
        assert_eq!(i.formats(), ["ARABIC"]);
        let i = Instr::parse(r#" XE "Apple\: red:Gala" \b \t "See also Fruit" "#);
        assert_eq!(i.first_arg(), Some(r#"Apple\: red:Gala"#));
        assert!(i.has("b") && !i.has("i"));
        assert_eq!(i.value("t").as_deref(), Some("See also Fruit"));
        assert_eq!(ref_target(" REF _Ref100000001 \\h "), Some(("_Ref100000001".into(), true)));
        assert_eq!(ref_target(" PAGEREF \"My_Mark\" "), Some(("My_Mark".into(), false)));
        assert_eq!(ref_target(" HYPERLINK \"x\" "), None);
    }

    #[test]
    fn a_field_inserted_after_another_sits_beside_it() {
        // Right after a result the closing mark comes first: the new field goes past it, not inside the field.
        let mut inl = complex(" MERGEFIELD A ", "«A»", None, false);
        let at = inl.iter().map(|i| i.len16()).sum();
        insert_inlines_at(&mut inl, at, complex(" MERGEFIELD B ", "«B»", None, false));
        let mut seen = Vec::new();
        scan_inlines(&inl, 0, &mut |s| seen.push((s.instr.trim().to_string(), s.result.to_string(), s.offset)));
        assert_eq!(seen, [("MERGEFIELD A".to_string(), "«A»".to_string(), 0), ("MERGEFIELD B".to_string(), "«B»".to_string(), 3)]);
    }

    #[test]
    fn inner_fields_are_worked_out_before_the_field_that_holds_them() {
        // `IF "{ MERGEFIELD x }" = "a"`: the outer field is visited last and its instruction holds the inner result.
        let mut inl = vec![];
        inl.push(Inline::Run(Run { items: vec![RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", "begin"))], ..Default::default() }));
        inl.push(Inline::Run(Run { items: vec![RunItem::Hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(" IF \""))], ..Default::default() }));
        inl.extend(complex(" MERGEFIELD x ", "«x»", None, false));
        inl.push(Inline::Run(Run { items: vec![RunItem::Hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text("\" = \"a\" \"yes\" \"no\" "))], ..Default::default() }));
        inl.push(Inline::Run(Run { items: vec![RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", "separate"))], ..Default::default() }));
        inl.push(Inline::Run(Run { items: vec![RunItem::Text("no".into())], ..Default::default() }));
        inl.push(Inline::Run(Run { items: vec![RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", "end"))], ..Default::default() }));
        let mut order = Vec::new();
        update_inner_first(&mut inl, &mut |s| {
            order.push(s.instr.trim().to_string());
            Instr::parse(s.instr).code.eq("MERGEFIELD").then(|| Replace::from("a".to_string()))
        });
        assert_eq!(order, ["MERGEFIELD x", "IF \"a\" = \"a\" \"yes\" \"no\""]);
        assert_eq!(inl.iter().map(|i| i.text()).collect::<String>(), "ano");
    }

    #[test]
    fn replaces_results() {
        let mut inl = complex(" SEQ Figure ", "1", Some(no_proof()), false);
        let mut seen = Vec::new();
        scan_inlines(&inl, 0, &mut |s| seen.push((s.instr.to_string(), s.result.to_string(), s.plain)));
        assert_eq!(seen, [(" SEQ Figure ".to_string(), "1".to_string(), true)]);
        let changed = update_inlines(&mut inl, 0, &mut |_| Some("7".to_string().into()));
        assert!(changed);
        let text: String = inl.iter().map(|i| i.text()).collect();
        assert_eq!(text, "7");
        // The result keeps its formatting.
        assert!(matches!(&inl[3], Inline::Run(r) if r.rpr.is_some()));
    }
}
