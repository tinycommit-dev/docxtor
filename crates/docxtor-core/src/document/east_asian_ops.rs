//! The ops for East Asian typography: phonetic guides, enclosed characters and the document's line-breaking
//! settings (see `east_asian` and `ruby`). A guide stays the `w:ruby` element Word wrote, and an enclosure the
//! `EQ` field; editing replaces what the dialog sets and keeps the rest.

use super::*;
use crate::east_asian::{self, AsianSettings, EastAsianOp, ENCLOSURES};
use crate::ruby::{self, RRuby};

/// The text of `[from, to)` in a paragraph and the formatting of its first run; an error when the range holds
/// anything but text (a picture, a tab, a field).
fn range_text(inlines: &[Inline], from: usize, to: usize) -> Result<(String, Option<Element>), EditError> {
    fn go(inlines: &[Inline], text: &mut String, rpr: &mut Option<Option<Element>>) -> Result<(), EditError> {
        for i in inlines {
            match i {
                Inline::Run(r) => {
                    for item in &r.items {
                        match item {
                            RunItem::Text(t) => text.push_str(t),
                            RunItem::Hidden(_) => {}
                            _ => return Err(EditError::Invalid("A phonetic guide or enclosure goes over plain text only.".into())),
                        }
                    }
                    if r.len16() > 0 && rpr.is_none() {
                        *rpr = Some(r.rpr.clone());
                    }
                }
                Inline::Container(c) => go(&c.children, text, rpr)?,
                Inline::Opaque(_) if i.len16() > 0 => return Err(EditError::Invalid("A phonetic guide or enclosure goes over plain text only.".into())),
                Inline::Opaque(_) => {}
            }
        }
        Ok(())
    }
    let slice = edit::slice(inlines, from, to);
    let (mut text, mut rpr) = (String::new(), None);
    go(&slice, &mut text, &mut rpr)?;
    Ok((text, rpr.flatten()))
}

fn field_run(rpr: &Option<Element>, item: Element) -> Inline {
    Inline::Run(Run { attrs: Vec::new(), t_attrs: None, rpr: rpr.clone(), items: vec![RunItem::Hidden(item)] })
}

/// Escape what the `EQ` field reads as its own syntax.
fn eq_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if matches!(c, '\\' | ',' | '(' | ')') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn eq_unescape(s: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for c in s.chars() {
        if esc {
            out.push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// The characters an enclosure field holds: `EQ \o\ac(○,字)` gives `字`.
fn enclosed_text(instr: &str) -> Option<String> {
    let body = instr.trim().strip_prefix("EQ")?.trim().strip_prefix("\\o\\ac(")?.strip_suffix(')')?;
    let mut esc = false;
    for (i, c) in body.char_indices() {
        if esc {
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ',' {
            return Some(eq_unescape(&body[i + 1..]));
        }
    }
    None
}

impl Document {
    /// The phonetic guides in a paragraph (any story) with the offset of each.
    pub fn guides_in(&self, pid: NodeId) -> Vec<(usize, RRuby)> {
        fn go(inlines: &[Inline], start: &mut usize, out: &mut Vec<(usize, RRuby)>) {
            for i in inlines {
                match i {
                    Inline::Run(r) => {
                        for item in &r.items {
                            if let RunItem::Object(e) = item {
                                if ruby::is_ruby(e) {
                                    out.push((*start, ruby::read(e)));
                                }
                            }
                            *start += item.len16();
                        }
                    }
                    Inline::Container(c) => go(&c.children, start, out),
                    Inline::Opaque(_) => *start += i.len16(),
                }
            }
        }
        let mut out = Vec::new();
        if let Some(p) = self.find_any_para(pid) {
            go(&p.inlines, &mut 0, &mut out);
        }
        out
    }

    /// The phonetic guide at a position (the one character `[o, o + 1)`), in any story.
    pub fn ruby_at(&self, at: Pos) -> Option<RRuby> {
        self.guides_in(at.p.0).into_iter().find(|(o, _)| *o == at.o).map(|(_, r)| r)
    }

    /// The document's East Asian line-breaking settings.
    pub fn asian_settings(&self) -> AsianSettings {
        AsianSettings::read(self.settings.as_ref().map(|d| &d.root))
    }

    pub(super) fn op_east_asian(&mut self, op: &EastAsianOp) -> Result<OpResult, EditError> {
        match op {
            EastAsianOp::SetRuby { from, to, props } => self.op_set_ruby(*from, *to, props),
            EastAsianOp::ClearRuby { at } => self.op_clear_ruby(*at),
            EastAsianOp::Enclose { from, to, symbol } => self.op_enclose(*from, *to, symbol),
            EastAsianOp::ClearEnclose { at } => self.op_clear_enclose(*at),
            EastAsianOp::SetSettings { settings } => {
                settings.validate().map_err(EditError::Invalid)?;
                self.settings_edit(true, &mut |root| east_asian::write_settings(root, settings));
                self.redraw_all();
                self.stories_touched = true;
                Ok(OpResult::default())
            }
        }
    }

    fn op_set_ruby(&mut self, from: Pos, to: Pos, props: &ruby::RubyProps) -> Result<OpResult, EditError> {
        props.validate().map_err(EditError::Invalid)?;
        if from.p != to.p {
            return Err(EditError::Invalid("A phonetic guide goes over text in one paragraph.".into()));
        }
        self.touch(from.p.0);
        let p = self.para_mut(from.p.0)?;
        let len = p.len16();
        let (a, b) = (from.o.min(to.o).min(len), from.o.max(to.o).min(len));
        if b == a + 1 {
            if let Some(e) = edit::object_at_mut(&mut p.inlines, a) {
                if ruby::is_ruby(e) {
                    ruby::update(e, props);
                    return Ok(OpResult { caret: Some(Pos::new(from.p.0, a + 1)), ..Default::default() });
                }
            }
        }
        if b == a {
            return Err(EditError::Invalid("Select the text the phonetic guide goes over.".into()));
        }
        let (text, rpr) = range_text(&p.inlines, a, b)?;
        if text.is_empty() {
            return Err(EditError::Invalid("Select the text the phonetic guide goes over.".into()));
        }
        edit::delete_range(&mut p.inlines, a, b);
        let el = ruby::build(&text, rpr.as_ref(), props);
        let run = Run { attrs: Vec::new(), t_attrs: None, rpr: rpr.filter(|r| !r.children.is_empty()), items: vec![RunItem::Object(el)] };
        edit::insert_inline_at(&mut p.inlines, a, Inline::Run(run));
        Ok(OpResult { caret: Some(Pos::new(from.p.0, a + 1)), ..Default::default() })
    }

    fn op_clear_ruby(&mut self, at: Pos) -> Result<OpResult, EditError> {
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let e = edit::object_at_mut(&mut p.inlines, at.o).filter(|e| ruby::is_ruby(e)).ok_or_else(|| EditError::Invalid("There is no phonetic guide at that position.".into()))?;
        let base = e.child("w:rubyBase").map(ruby::text_of).unwrap_or_default();
        let rpr = e.child("w:rubyBase").and_then(ruby::first_rpr).cloned();
        edit::delete_range(&mut p.inlines, at.o, at.o + 1);
        edit::insert_text(&mut p.inlines, at.o, &base, Some(rpr.unwrap_or_else(|| Element::new("w:rPr"))));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, at.o + base.encode_utf16().count())), ..Default::default() })
    }

    fn op_enclose(&mut self, from: Pos, to: Pos, symbol: &str) -> Result<OpResult, EditError> {
        let shape = ENCLOSURES.iter().find(|(n, _)| *n == symbol).map(|(_, c)| *c).ok_or_else(|| EditError::Invalid(format!("An enclosure is circle, square, triangle or diamond, not '{symbol}'.")))?;
        if from.p != to.p {
            return Err(EditError::Invalid("Enclosed characters are in one paragraph.".into()));
        }
        self.touch(from.p.0);
        let p = self.para_mut(from.p.0)?;
        let len = p.len16();
        let (a, b) = (from.o.min(to.o).min(len), from.o.max(to.o).min(len));
        let (text, rpr) = range_text(&p.inlines, a, b)?;
        let n = text.chars().count();
        if !(1..=2).contains(&n) {
            return Err(EditError::Invalid("Enclose one or two characters.".into()));
        }
        let instr = format!(" EQ \\o\\ac({},{}) ", shape, eq_escape(&text));
        edit::delete_range(&mut p.inlines, a, b);
        let rpr = rpr.filter(|r| !r.children.is_empty());
        let mut instr_el = Element::new("w:instrText").with_attr("xml:space", "preserve");
        instr_el.children.push(Node::Text(instr));
        // The runs go in back to front, since each one is put before what is already at the position.
        for el in [
            Element::new("w:fldChar").with_attr("w:fldCharType", "end"),
            instr_el,
            Element::new("w:fldChar").with_attr("w:fldCharType", "begin"),
        ] {
            edit::insert_inline_at(&mut p.inlines, a, field_run(&rpr, el));
        }
        Ok(OpResult { caret: Some(Pos::new(from.p.0, a)), ..Default::default() })
    }

    fn op_clear_enclose(&mut self, at: Pos) -> Result<OpResult, EditError> {
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        // The field starts at the position with a `begin` mark; its instruction follows and an `end` mark closes it.
        let mut start = 0;
        let mut found: Option<(usize, usize, String, Option<Element>)> = None;
        for (i, inl) in p.inlines.iter().enumerate() {
            let len = inl.len16();
            if start == at.o && len == 0 {
                if let Inline::Run(r) = inl {
                    if r.items.iter().any(|it| matches!(it, RunItem::Hidden(e) if e.name == "w:fldChar" && e.attr("w:fldCharType") == Some("begin"))) {
                        let mut instr = String::new();
                        let mut j = i + 1;
                        while j < p.inlines.len() {
                            if let Inline::Run(r2) = &p.inlines[j] {
                                for it in &r2.items {
                                    if let RunItem::Hidden(e) = it {
                                        if e.name == "w:instrText" {
                                            instr.push_str(&e.text());
                                        }
                                        if e.name == "w:fldChar" && e.attr("w:fldCharType") == Some("end") {
                                            if let Some(t) = enclosed_text(&instr) {
                                                found = Some((i, j, t, r.rpr.clone()));
                                            }
                                            j = p.inlines.len();
                                            break;
                                        }
                                    }
                                }
                            }
                            j += 1;
                        }
                        if found.is_some() {
                            break;
                        }
                    }
                }
            }
            start += len;
        }
        let (i, j, text, rpr) = found.ok_or_else(|| EditError::Invalid("There are no enclosed characters at that position.".into()))?;
        p.inlines.drain(i..=j);
        edit::insert_text(&mut p.inlines, at.o, &text, Some(rpr.unwrap_or_else(|| Element::new("w:rPr"))));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, at.o + text.encode_utf16().count())), ..Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enclosed_text_reads_the_second_argument() {
        assert_eq!(enclosed_text(r" EQ \o\ac(○,字) ").as_deref(), Some("字"));
        assert_eq!(enclosed_text(r" EQ \o\ac(○,\,) ").as_deref(), Some(","));
        assert_eq!(enclosed_text(r" EQ \f(1,2) "), None);
    }
}
