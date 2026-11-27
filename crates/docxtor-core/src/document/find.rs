//! Find and replace as Word does it: plain text with special characters, wildcards or regular expressions,
//! whole words, prefixes and suffixes, ignored punctuation and white space, text that sounds alike, formatting to
//! find and formatting to replace with, and searching the headers, footers, notes and text boxes as well as the
//! text. The pattern matcher is in `crate::matcher`; this file turns the document into text it can search and
//! turns a match into edits.
//!
//! Searching is read-only (`Document::find`). Replacing is the op `replace_text`: every edit it makes goes
//! through the same ops as typing (`insert_text`, `delete`, `split_paragraph`, `format_text`), so a replacement
//! across paragraph marks joins and splits paragraphs, and the whole replace undoes in one step.

use super::*;
use crate::marks::BookmarkStory;
use crate::matcher::{self as mx, Flags, Hc, Matcher};
use crate::model::OBJ;
use serde::Deserialize;

/// The most matches one search returns.
pub const MAX_HITS: usize = 20_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// The whole text.
    #[default]
    All,
    /// From `from` to the end of the text it is in.
    Down,
    /// From the start of the text it is in to `from`.
    Up,
}

/// What to look for besides the text. Every field is off (or empty) by default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FindOptions {
    /// Letters match in either case ("Match case" off).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ignore_case: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub whole_word: bool,
    /// Word's wildcards: `?`, `*`, `[a-z]`, `[!x]`, `{n,m}`, `@`, `<`, `>`, `(...)` with `\1` in the replacement.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub wildcards: bool,
    /// JavaScript regular expression syntax, with `$1` in the replacement.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub regex: bool,
    /// Only at the start of a word.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub prefix: bool,
    /// Only at the end of a word.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suffix: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ignore_punct: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ignore_space: bool,
    /// é matches e, ü matches u.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ignore_marks: bool,
    /// Words that sound like the text (Soundex).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sounds_like: bool,
    /// Give a replacement the capitals of the text it replaces ("Apple" becomes "Pear", "APPLE" "PEAR"), as Word
    /// does when "Match case" is off. Only used when letters match in either case and the search is plain text.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub keep_case: bool,
    /// Search the main text only, not headers, footers, notes and text boxes.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub main_only: bool,
    #[serde(skip_serializing_if = "is_all")]
    pub direction: Direction,
    /// Where `direction` starts from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Pos>,
    /// Only text with this formatting. With no text to find, every stretch of text that has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find_format: Option<FindFormat>,
}

fn is_all(d: &Direction) -> bool {
    *d == Direction::All
}

/// Formatting to find. A field left out is not looked at.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FindFormat {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// Any underline (true) or none (false).
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    pub font: Option<String>,
    /// Points.
    pub size: Option<f64>,
    /// Hex, or `auto`.
    pub color: Option<String>,
    /// A highlight name, or `none`.
    pub highlight: Option<String>,
    /// `superscript`, `subscript` or `baseline`.
    pub vert_align: Option<String>,
    /// A character style id (`Emphasis`).
    pub style: Option<String>,
    /// A language tag set on the text (`en-GB`).
    pub lang: Option<String>,
    /// A paragraph style id; empty for the default style.
    pub para_style: Option<String>,
    /// `left`, `center`, `right` or `justify`.
    pub align: Option<String>,
}

impl FindFormat {
    fn has_run(&self) -> bool {
        self.bold.is_some()
            || self.italic.is_some()
            || self.underline.is_some()
            || self.strike.is_some()
            || self.caps.is_some()
            || self.small_caps.is_some()
            || self.font.is_some()
            || self.size.is_some()
            || self.color.is_some()
            || self.highlight.is_some()
            || self.vert_align.is_some()
            || self.style.is_some()
            || self.lang.is_some()
    }

    fn has_para(&self) -> bool {
        self.para_style.is_some() || self.align.is_some()
    }

    pub fn is_empty(&self) -> bool {
        !self.has_run() && !self.has_para()
    }
}

/// One match, as the interface lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindHit {
    pub p: NodeId,
    pub o: usize,
    pub end_p: NodeId,
    pub end_o: usize,
    /// The matched text, with a paragraph mark shown as ¶.
    pub text: String,
    /// About thirty characters on each side, in the same paragraph.
    pub before: String,
    pub after: String,
    /// Where it is when not in the main text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story: Option<BookmarkStory>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindResult {
    pub hits: Vec<FindHit>,
    /// More matches than `MAX_HITS`: the list stops there.
    pub truncated: bool,
}

/// What a replacement is made of once `^p`, `\1` and the like are worked out.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    Text(String),
    Para,
    Page,
    Column,
}

/// Consecutive paragraphs that share a block list (the body, a table cell, a content control) read as one text,
/// so a paragraph mark can be found and a match can run over it.
fn collect_streams<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut Vec<Vec<&'a Paragraph>>) {
    let mut cur: Vec<&Paragraph> = Vec::new();
    for b in blocks {
        match b {
            Block::Para(p) => cur.push(p),
            Block::Table(t) => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                for c in t.rows().flat_map(|r| r.cells()) {
                    collect_streams(&c.blocks, out);
                }
            }
            Block::Container(c) => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                collect_streams(&c.blocks, out);
            }
            Block::Opaque(_) => {}
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
}

fn norm_color(c: &str) -> String {
    let c = c.trim_start_matches('#').to_ascii_uppercase();
    if c.is_empty() {
        "AUTO".into()
    } else {
        c
    }
}

fn norm_align(a: &str) -> &str {
    match a {
        "both" | "distribute" => "justify",
        "start" => "left",
        "end" => "right",
        other => other,
    }
}

/// The capitals of `found` put on `rep` (ALL CAPS stays all caps, "Initial Caps" gets initial capitals).
fn keep_case(found: &str, rep: &str) -> String {
    let letters: Vec<char> = found.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return rep.to_string();
    }
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return rep.to_uppercase();
    }
    let cap = |w: &str| {
        let mut it = w.chars();
        match it.next() {
            Some(f) => f.to_uppercase().chain(it).collect::<String>(),
            None => String::new(),
        }
    };
    let words: Vec<&str> = found.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphabetic())).collect();
    let initial = |w: &str| w.chars().find(|c| c.is_alphabetic()).is_some_and(|c| c.is_uppercase());
    if words.len() >= 2 && words.iter().all(|w| initial(w)) {
        let mut out = String::new();
        let mut word = String::new();
        for c in rep.chars() {
            if c.is_whitespace() {
                out.push_str(&cap(&word));
                word.clear();
                out.push(c);
            } else {
                word.push(c);
            }
        }
        out.push_str(&cap(&word));
        return out;
    }
    if initial(found) {
        return cap(rep);
    }
    rep.to_string()
}

fn push_text(parts: &mut Vec<Part>, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(Part::Text(t)) = parts.last_mut() {
        t.push_str(s);
    } else {
        parts.push(Part::Text(s.to_string()));
    }
}

/// The matched characters as parts to put back (for `^&`, `\0` and `$&`). Returns whether the span holds an inline
/// object (a picture, note mark, equation), which a part can't carry.
fn found_parts(parts: &mut Vec<Part>, hay: &[Hc]) -> bool {
    let mut object = false;
    for h in hay {
        if h.kind & mx::PMARK != 0 {
            parts.push(Part::Para);
        } else if h.kind & mx::PAGE != 0 {
            parts.push(Part::Page);
        } else if h.kind & mx::COLUMN != 0 {
            parts.push(Part::Column);
        } else if h.kind & mx::LINE != 0 {
            push_text(parts, "\n");
        } else if h.c == OBJ {
            object = true;
        } else {
            let mut b = [0; 4];
            push_text(parts, h.c.encode_utf8(&mut b));
        }
    }
    object
}

enum Mode {
    Plain,
    Wild,
    Regex,
}

/// Work out what a match is replaced with.
fn expand(rep: &str, mode: &Mode, m: &Matcher, hay: &[Hc], f: &mx::Found, ignore_case_keep: bool) -> Result<Vec<Part>, String> {
    let mut parts: Vec<Part> = Vec::new();
    let cs: Vec<char> = rep.chars().collect();
    let reused_object = std::cell::Cell::new(false);
    let group = |parts: &mut Vec<Part>, n: usize| {
        let object = if n == 0 {
            found_parts(parts, &hay[f.start..f.end])
        } else if let Some(Some((a, b))) = f.caps.get(n) {
            found_parts(parts, &hay[*a..*b])
        } else {
            false
        };
        if object {
            reused_object.set(true);
        }
    };
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let n = cs.get(i + 1).copied();
        match (c, n) {
            ('^', Some(x)) if !matches!(mode, Mode::Regex) => {
                i += 2;
                match x {
                    'p' => parts.push(Part::Para),
                    't' => push_text(&mut parts, "\t"),
                    'l' => push_text(&mut parts, "\n"),
                    'm' => parts.push(Part::Page),
                    'n' => parts.push(Part::Column),
                    '^' => push_text(&mut parts, "^"),
                    '+' => push_text(&mut parts, "\u{2014}"),
                    '=' => push_text(&mut parts, "\u{2013}"),
                    's' => push_text(&mut parts, "\u{a0}"),
                    '-' => push_text(&mut parts, "\u{ad}"),
                    '~' => push_text(&mut parts, "\u{2011}"),
                    '&' => group(&mut parts, 0),
                    'b' => return Err("a section break can't be put in by replacing: use Insert, Break".into()),
                    other => {
                        push_text(&mut parts, "^");
                        push_text(&mut parts, &other.to_string());
                    }
                }
            }
            ('\\', Some(x)) if matches!(mode, Mode::Wild) => {
                i += 2;
                match x.to_digit(10) {
                    Some(d) => group(&mut parts, d as usize),
                    None if x == '\\' => push_text(&mut parts, "\\"),
                    None => {
                        push_text(&mut parts, "\\");
                        push_text(&mut parts, &x.to_string());
                    }
                }
            }
            ('$', Some(x)) if matches!(mode, Mode::Regex) => {
                if x == '$' {
                    push_text(&mut parts, "$");
                    i += 2;
                } else if x == '&' {
                    group(&mut parts, 0);
                    i += 2;
                } else if x.is_ascii_digit() {
                    let mut num = x.to_digit(10).unwrap() as usize;
                    let mut len = 2;
                    // Two digits when the group exists, as JavaScript does.
                    if let Some(d) = cs.get(i + 2).and_then(|d| d.to_digit(10)) {
                        let two = num * 10 + d as usize;
                        if two <= m.groups {
                            num = two;
                            len = 3;
                        }
                    }
                    if num >= 1 && num <= m.groups {
                        group(&mut parts, num);
                    } else {
                        let lit: String = cs[i..i + len].iter().collect();
                        push_text(&mut parts, &lit);
                    }
                    i += len;
                } else if x == '<' {
                    if let Some(close) = cs[i + 2..].iter().position(|c| *c == '>') {
                        let name: String = cs[i + 2..i + 2 + close].iter().collect();
                        if let Some((_, g)) = m.names.iter().find(|(k, _)| *k == name) {
                            group(&mut parts, *g);
                        }
                        i += close + 3;
                    } else {
                        push_text(&mut parts, "$");
                        i += 1;
                    }
                } else {
                    push_text(&mut parts, "$");
                    i += 1;
                }
            }
            _ => {
                let mut b = [0; 4];
                push_text(&mut parts, c.encode_utf8(&mut b));
                i += 1;
            }
        }
    }
    if reused_object.get() {
        // Putting the found text back would drop the picture, note mark or equation inside it.
        return Err("the replacement reuses found text that holds a picture, note mark or equation, which would be lost: leave the object out of the match".into());
    }
    if ignore_case_keep {
        let found: String = hay[f.start..f.end].iter().map(|h| h.c).collect();
        for p in parts.iter_mut() {
            if let Part::Text(t) = p {
                *t = keep_case(&found, t);
            }
        }
    }
    Ok(parts)
}

fn describe(h: &[Hc]) -> String {
    h.iter().map(|x| if x.kind & mx::PMARK != 0 { '¶' } else if x.kind & (mx::LINE | mx::PAGE | mx::COLUMN) != 0 { ' ' } else { x.c }).collect()
}

struct Compiled {
    m: Matcher,
    mode: Mode,
    /// Characters left out of the text searched, when punctuation or white space is ignored.
    skip: Option<fn(char) -> bool>,
}

fn skip_punct_space(o: &FindOptions) -> Option<fn(char) -> bool> {
    match (o.ignore_punct, o.ignore_space) {
        (true, true) => Some(|c| mx::is_punctuation(c) || c == ' ' || c == '\t' || c == '\u{a0}'),
        (true, false) => Some(mx::is_punctuation),
        (false, true) => Some(|c| c == ' ' || c == '\t' || c == '\u{a0}'),
        (false, false) => None,
    }
}

fn compile(find: &str, o: &FindOptions) -> Result<Compiled, String> {
    let fmt = o.find_format.as_ref().is_some_and(|f| !f.is_empty());
    let plain = !o.regex && !o.wildcards;
    let flags = Flags { ignore_case: o.ignore_case, ignore_marks: o.ignore_marks && plain, need_fmt: fmt };
    let skip = if plain { skip_punct_space(o) } else { None };
    let mut mode = Mode::Plain;
    let mut m = if find.is_empty() {
        if !fmt {
            return Err("the text to find is empty".into());
        }
        Matcher { root: mx::Node::Rep { node: Box::new(mx::Node::Dot), min: 1, max: usize::MAX, greedy: true }, flags, groups: 0, names: vec![] }
    } else if (o.regex || o.wildcards) && find.chars().count() > mx::MAX_FIND_CHARS {
        return Err(format!("the pattern is too long ({} characters at most)", mx::MAX_FIND_CHARS));
    } else if o.regex {
        mode = Mode::Regex;
        mx::compile_regex(find, flags)?
    } else if o.wildcards {
        mode = Mode::Wild;
        mx::compile_wildcards(find, flags)?
    } else {
        let sk = skip.unwrap_or(|_| false);
        mx::compile_plain(find, flags, &sk, o.sounds_like)?
    };
    let (start, end) = (o.whole_word || o.prefix, o.whole_word || o.suffix);
    if start || end {
        mx::wrap_words(&mut m, start, end);
    }
    Ok(Compiled { m, mode, skip })
}

/// Replace `[start, end)` of a paragraph with the parts (text, page and column breaks; no paragraph marks). The first
/// `keep` characters of the match stay until the new text is in, so the new text takes their formatting. Returns the
/// ranges the new pieces occupy and where the new text ends.
fn edit_inlines(p: &mut Paragraph, start: usize, end: usize, keep: usize, parts: &[Part]) -> (Vec<(usize, usize)>, usize) {
    let mut ranges = Vec::new();
    let mut off = start + keep;
    let len = p.len16();
    edit::delete_range(&mut p.inlines, off.min(len), end.min(len));
    for part in parts {
        match part {
            Part::Text(t) => {
                edit::insert_text(&mut p.inlines, off, t, None);
                let n = t.encode_utf16().count();
                ranges.push((off, off + n));
                off += n;
            }
            Part::Page | Part::Column => {
                let kind = if *part == Part::Page { "page" } else { "column" };
                let br = Element::new("w:br").with_attr("w:type", kind);
                edit::insert_inline_at(&mut p.inlines, off, Inline::Run(Run { items: vec![RunItem::Break(br)], ..Default::default() }));
                ranges.push((off, off + 1));
                off += 1;
            }
            Part::Para => {}
        }
    }
    if keep > 0 {
        let len = p.len16();
        edit::delete_range(&mut p.inlines, start.min(len), (start + keep).min(len));
        for r in ranges.iter_mut() {
            r.0 -= keep;
            r.1 -= keep;
        }
        off -= keep;
    }
    edit::cleanup(&mut p.inlines);
    (ranges, off)
}

impl Document {
    /// Whether a paragraph has the paragraph formatting being searched for.
    fn para_matches(&self, p: &Paragraph, f: &FindFormat) -> bool {
        if let Some(want) = &f.para_style {
            let have = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or("");
            let default = self.styles.lookup("Normal").unwrap_or_else(|| "Normal".into());
            let norm = |s: &str| if s.is_empty() { default.clone() } else { s.to_string() };
            if norm(have) != norm(want) {
                return false;
            }
        }
        if let Some(want) = &f.align {
            let direct = p.ppr.as_ref().and_then(|pp| pp.child("w:jc")).and_then(|j| j.attr("w:val")).map(String::from);
            let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val"));
            let have = direct.or_else(|| self.styles.para_style(style).0.align.clone()).unwrap_or_else(|| "left".into());
            if norm_align(&have) != norm_align(want) {
                return false;
            }
        }
        true
    }

    /// The formatting a run really has: its paragraph style, its character style and its own settings.
    fn resolved_run(&self, base: &RunFmt, run: &Run) -> RunFmt {
        let direct = run.rpr.as_ref().map(RunFmt::from_rpr).unwrap_or_default();
        let mut r = base.clone();
        if let Some(cs) = &direct.style {
            r = r.overlay(&self.styles.char_style(cs));
        }
        self.styles.finish_run(r.overlay(&direct))
    }

    fn run_matches(&self, f: &FindFormat, r: &RunFmt, run: &Run) -> bool {
        let flag = |want: Option<bool>, have: Option<bool>| want.is_none_or(|w| have.unwrap_or(false) == w);
        if !(flag(f.bold, r.bold) && flag(f.italic, r.italic) && flag(f.strike, r.strike) && flag(f.caps, r.caps) && flag(f.small_caps, r.small_caps)) {
            return false;
        }
        if let Some(w) = f.underline {
            let has = r.underline.as_deref().is_some_and(|u| u != "none");
            if has != w {
                return false;
            }
        }
        if let Some(font) = &f.font {
            if !r.font.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(font)) {
                return false;
            }
        }
        if let Some(size) = f.size {
            if !r.size.is_some_and(|s| (s - size).abs() < 0.01) {
                return false;
            }
        }
        if let Some(color) = &f.color {
            if norm_color(r.color.as_deref().unwrap_or("")) != norm_color(color) {
                return false;
            }
        }
        if let Some(h) = &f.highlight {
            let have = r.highlight.as_deref().filter(|h| *h != "none");
            let want = Some(h.as_str()).filter(|h| *h != "none");
            if have.map(str::to_ascii_lowercase) != want.map(str::to_ascii_lowercase) {
                return false;
            }
        }
        if let Some(v) = &f.vert_align {
            let have = r.vert_align.as_deref().unwrap_or("baseline");
            if have != v {
                return false;
            }
        }
        if let Some(s) = &f.style {
            let have = run.rpr.as_ref().and_then(|x| x.child("w:rStyle")).and_then(|x| x.attr("w:val")).unwrap_or("");
            if have != s {
                return false;
            }
        }
        if let Some(l) = &f.lang {
            let have = run.rpr.as_ref().and_then(crate::styles::lang_of).unwrap_or_default();
            if !have.eq_ignore_ascii_case(l) {
                return false;
            }
        }
        true
    }

    /// The characters of a run of paragraphs as one searchable text.
    fn hay_of(&self, paras: &[&Paragraph], fmt: Option<&FindFormat>) -> Vec<Hc> {
        struct W<'a> {
            doc: &'a Document,
            out: Vec<Hc>,
            pi: u32,
            off: usize,
            fmt: Option<&'a FindFormat>,
            para_ok: bool,
            base: RunFmt,
            fields: Vec<u16>,
            next_field: u16,
            simple: u16,
        }
        impl W<'_> {
            fn push(&mut self, c: char, kind: u16, len: usize, ok: bool) {
                let field = self.simple.max(self.fields.first().copied().unwrap_or(0));
                self.out.push(Hc { c, kind, field, ok, para: self.pi, off: self.off as u32, len: len as u8 });
                self.off += len;
            }

            fn walk(&mut self, inlines: &[Inline]) {
                for inl in inlines {
                    match inl {
                        Inline::Run(r) => self.run(r),
                        Inline::Container(c) => {
                            let simple = c.kind == InlineKind::Field && self.simple == 0;
                            if simple {
                                self.next_field += 1;
                                self.simple = self.next_field;
                            }
                            self.walk(&c.children);
                            if simple {
                                self.simple = 0;
                            }
                        }
                        Inline::Opaque(e) => {
                            if crate::math::is_math(e) {
                                self.push(OBJ, mx::EQUATION, 1, self.para_ok);
                            }
                        }
                    }
                }
            }

            fn run(&mut self, r: &Run) {
                let ok = self.para_ok
                    && match self.fmt {
                        Some(f) if f.has_run() => self.doc.run_matches(f, &self.doc.resolved_run(&self.base, r), r),
                        _ => true,
                    };
                for it in &r.items {
                    match it {
                        RunItem::Text(t) => {
                            for ch in t.chars() {
                                self.push(ch, 0, ch.len_utf16(), ok);
                            }
                        }
                        RunItem::Tab => self.push('\t', 0, 1, ok),
                        RunItem::Break(e) => match e.attr("w:type") {
                            Some("page") => self.push('\u{c}', mx::PAGE, 1, ok),
                            Some("column") => self.push('\u{e}', mx::COLUMN, 1, ok),
                            _ => self.push('\n', mx::LINE, 1, ok),
                        },
                        RunItem::Object(e) => {
                            if crate::marks::is_watermark_pict(e) {
                                continue;
                            }
                            let kind = match e.name.as_str() {
                                "w:footnoteReference" => mx::FOOTNOTE,
                                "w:endnoteReference" => mx::ENDNOTE,
                                "w:drawing" | "w:pict" | "w:object" | "mc:AlternateContent" => mx::GRAPHIC,
                                _ => 0,
                            };
                            self.push(OBJ, kind, 1, ok);
                        }
                        RunItem::Hidden(e) => {
                            if e.name == "w:fldChar" {
                                match e.attr("w:fldCharType") {
                                    Some("begin") => {
                                        self.next_field += 1;
                                        self.fields.push(self.next_field);
                                    }
                                    Some("end") => {
                                        self.fields.pop();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut w = W {
            doc: self,
            out: Vec::new(),
            pi: 0,
            off: 0,
            fmt,
            para_ok: true,
            base: RunFmt::default(),
            fields: Vec::new(),
            next_field: 0,
            simple: 0,
        };
        let last = paras.len().saturating_sub(1);
        for (pi, p) in paras.iter().enumerate() {
            w.pi = pi as u32;
            w.off = 0;
            w.fields.clear();
            w.next_field = 0;
            w.para_ok = fmt.is_none_or(|f| !f.has_para() || self.para_matches(p, f));
            if fmt.is_some_and(|f| f.has_run()) {
                let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val"));
                w.base = self.inherited_run(style);
            }
            w.walk(&p.inlines);
            if pi < last {
                let sect = p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some());
                let len = p.len16();
                w.out.push(Hc { c: '\r', kind: mx::PMARK | if sect { mx::SECT } else { 0 }, field: 0, ok: w.para_ok, para: pi as u32, off: len as u32, len: 0 });
            }
        }
        w.out
    }

    /// The matches in one stream, each as a range of `hay`.
    fn stream_matches(c: &Compiled, hay: &[Hc], o: &FindOptions, limit: usize) -> Vec<mx::Found> {
        match c.skip {
            None => c.m.find_all(hay, limit),
            Some(skip) => {
                // Search a copy without the ignored characters, then map the ranges back.
                let map: Vec<usize> = (0..hay.len()).filter(|i| hay[*i].kind != 0 || !skip(hay[*i].c)).collect();
                let kept: Vec<Hc> = map.iter().map(|i| hay[*i]).collect();
                let _ = o;
                c.m.find_all(&kept, limit)
                    .into_iter()
                    .map(|mut f| {
                        let remap = |k: usize, end: bool| if end { map[k - 1] + 1 } else { map[k] };
                        let (s, e) = (remap(f.start, false), remap(f.end, true));
                        for cap in f.caps.iter_mut().flatten() {
                            *cap = (if cap.0 < map.len() { map[cap.0] } else { hay.len() }, if cap.1 > 0 { map[cap.1 - 1] + 1 } else { 0 });
                        }
                        f.start = s;
                        f.end = e;
                        f
                    })
                    .collect()
            }
        }
    }

    /// Where a match starts and ends in the document.
    fn span(paras: &[&Paragraph], hay: &[Hc], s: usize, e: usize) -> (Pos, Pos) {
        let a = hay[s];
        let start = Pos::new(paras[a.para as usize].id, a.off as usize);
        let z = hay[e - 1];
        let end = if z.kind & mx::PMARK != 0 {
            let next = (z.para as usize + 1).min(paras.len() - 1);
            Pos::new(paras[next].id, 0)
        } else {
            Pos::new(paras[z.para as usize].id, z.off as usize + z.len as usize)
        };
        (start, end)
    }

    /// Streams of paragraphs of a block list, or of only the listed paragraphs.
    fn streams_of<'a>(&'a self, blocks: crate::body::BlocksRef<'a>, only: Option<&[Id]>) -> Result<Vec<Vec<&'a Paragraph>>, EditError> {
        let mut streams = Vec::new();
        collect_streams(blocks, &mut streams);
        if let Some(ids) = only {
            for id in ids {
                if find_para(blocks, id.0).is_none() {
                    return Err(EditError::NotFound(id.0));
                }
            }
            let keep: std::collections::HashSet<NodeId> = ids.iter().map(|i| i.0).collect();
            let mut out: Vec<Vec<&Paragraph>> = Vec::new();
            for s in streams {
                let mut cur: Vec<&Paragraph> = Vec::new();
                for p in s {
                    if keep.contains(&p.id) {
                        cur.push(p);
                    } else if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                if !cur.is_empty() {
                    out.push(cur);
                }
            }
            streams = out;
        }
        Ok(streams)
    }

    /// Whether a match at `start`..`end` is on the right side of `from` for the direction.
    fn in_direction(order: &HashMap<NodeId, usize>, o: &FindOptions, start: Pos, end: Pos) -> bool {
        let Some(from) = o.from else { return true };
        let key = |p: Pos| (order.get(&p.p.0).copied().unwrap_or(usize::MAX), p.o);
        match o.direction {
            Direction::All => true,
            Direction::Down => key(start) >= key(from),
            Direction::Up => key(end) <= key(from),
        }
    }

    /// Every match of `find`, in the text and (unless `main_only`) in headers, footers, notes and text boxes.
    pub fn find(&self, find: &str, o: &FindOptions) -> Result<FindResult, EditError> {
        let c = compile(find, o).map_err(EditError::Invalid)?;
        let fmt = o.find_format.as_ref().filter(|f| !f.is_empty());
        let mut result = FindResult::default();
        // Down and Up stay in the text the position is in.
        let from_story = o.from.and_then(|f| if o.direction == Direction::All { None } else { self.story_index(f.p.0) });
        let from_body = o.from.is_some_and(|f| o.direction != Direction::All && find_block(&self.body, f.p.0).is_some());
        let scoped = o.from.is_some() && o.direction != Direction::All;
        let mut places: Vec<(Option<usize>, crate::body::BlocksRef)> = Vec::new();
        if !scoped || from_body {
            places.push((None, crate::body::BlocksRef::Top(&self.body)));
        }
        if !o.main_only {
            for (si, s) in self.stories.iter().enumerate() {
                if scoped && from_story != Some(si) {
                    continue;
                }
                places.push((Some(si), crate::body::BlocksRef::List(&s.blocks)));
            }
        }
        for (si, blocks) in places {
            let order: HashMap<NodeId, usize> = paragraph_order(blocks).into_iter().enumerate().map(|(i, p)| (p, i)).collect();
            let mut streams = Vec::new();
            collect_streams(blocks, &mut streams);
            for paras in streams {
                let hay = self.hay_of(&paras, fmt);
                for f in Self::stream_matches(&c, &hay, o, MAX_HITS + 1) {
                    let (start, end) = Self::span(&paras, &hay, f.start, f.end);
                    if !Self::in_direction(&order, o, start, end) {
                        continue;
                    }
                    if result.hits.len() >= MAX_HITS {
                        result.truncated = true;
                        return Ok(result);
                    }
                    let near = |range: std::ops::Range<usize>| -> String {
                        let para = hay[f.start].para;
                        describe(&hay[range].iter().copied().filter(|h| h.para == para && h.kind & mx::PMARK == 0).collect::<Vec<_>>())
                    };
                    let b0 = f.start.saturating_sub(30);
                    let a1 = (f.end + 30).min(hay.len());
                    result.hits.push(FindHit {
                        p: start.p.0,
                        o: start.o,
                        end_p: end.p.0,
                        end_o: end.o,
                        text: describe(&hay[f.start..f.end]),
                        before: near(b0..f.start),
                        after: near(f.end..a1),
                        story: si.and_then(|i| self.story_info(i)),
                    });
                }
            }
        }
        Ok(result)
    }

    fn story_info(&self, si: usize) -> Option<BookmarkStory> {
        let s = self.stories.get(si)?;
        Some(BookmarkStory { kind: s.kind, part: s.part.clone(), note_id: s.note_id.clone() })
    }

    /// The character formatting at a position (the character before it, or the first at the start), as the
    /// settings "Select text with similar formatting" looks for.
    pub fn find_format_at(&self, p: NodeId, o: usize) -> Option<FindFormat> {
        let para = self.find_any_para(p)?;
        let style = para.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val"));
        let base = self.inherited_run(style);
        let want = o.saturating_sub(1);
        let mut found: Option<(&Run, RunFmt)> = None;
        let mut start = 0;
        walk_runs(&para.inlines, &mut start, &mut |r, at, _| {
            let len = r.len16();
            if found.is_none() && len > 0 && at <= want && want < at + len {
                found = Some((r, self.resolved_run(&base, r)));
            }
        });
        let empty = Run::default();
        let (run, r) = match found {
            Some(x) => x,
            None => (&empty, self.styles.finish_run(base)),
        };
        let direct = run.rpr.as_ref().map(RunFmt::from_rpr).unwrap_or_default();
        Some(FindFormat {
            bold: Some(r.bold.unwrap_or(false)),
            italic: Some(r.italic.unwrap_or(false)),
            underline: Some(r.underline.as_deref().is_some_and(|u| u != "none")),
            strike: Some(r.strike.unwrap_or(false)),
            caps: Some(r.caps.unwrap_or(false)),
            small_caps: Some(r.small_caps.unwrap_or(false)),
            font: r.font.clone(),
            size: r.size,
            color: Some(norm_color(r.color.as_deref().unwrap_or(""))),
            highlight: Some(r.highlight.clone().unwrap_or_else(|| "none".into())),
            vert_align: Some(r.vert_align.clone().unwrap_or_else(|| "baseline".into())),
            style: Some(direct.style.unwrap_or_default()),
            lang: run.rpr.as_ref().and_then(crate::styles::lang_of),
            para_style: None,
            align: None,
        })
    }

    /// Replace in the text being edited (the main text, or a story while an op runs in it). Returns how many
    /// were replaced (0 when nothing matched).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn replace_in_body(
        &mut self,
        find: &str,
        replace: &str,
        ids: Option<&[Id]>,
        all: bool,
        o: &FindOptions,
        format: Option<&RunPatch>,
        para_format: Option<&ParaPatch>,
        language: Option<&str>,
    ) -> Result<OpResult, EditError> {
        let c = compile(find, o).map_err(EditError::Invalid)?;
        let fmt = o.find_format.as_ref().filter(|f| !f.is_empty());
        let lang = language.map(docprops::normalize_lang).transpose().map_err(EditError::Invalid)?;
        // A character style named by its display name, or one of the built-in ones, is made if the file lacks it.
        let mut format = format.cloned();
        if let Some(st) = format.as_mut().and_then(|f| f.style.as_mut()) {
            if !st.is_empty() {
                *st = self.resolve_style(st)?;
            }
        }
        let format = format.as_ref();
        let styling = format.is_some() || para_format.is_some() || lang.is_some();
        // With formatting to put on and no replacement text, the text stays and only its look changes.
        let text_change = !(replace.is_empty() && styling);
        let keep = o.keep_case && o.ignore_case && matches!(c.mode, Mode::Plain) && !o.sounds_like;
        let order: HashMap<NodeId, usize> = paragraph_order(&self.body).into_iter().enumerate().map(|(i, p)| (p, i)).collect();
        // Work out every match and its replacement first; the edits then run from the end backwards.
        struct Job {
            start: Pos,
            end: Pos,
            first_len: usize,
            parts: Vec<Part>,
            matched: usize,
        }
        let mut jobs: Vec<Job> = Vec::new();
        {
            let streams = self.streams_of(crate::body::BlocksRef::Top(&self.body), ids)?;
            'streams: for paras in &streams {
                let hay = self.hay_of(paras, fmt);
                for f in Self::stream_matches(&c, &hay, o, usize::MAX) {
                    let (start, end) = Self::span(paras, &hay, f.start, f.end);
                    if !Self::in_direction(&order, o, start, end) {
                        continue;
                    }
                    let parts = if text_change { expand(replace, &c.mode, &c.m, &hay, &f, keep).map_err(EditError::Invalid)? } else { Vec::new() };
                    let matched = hay[f.start..f.end].iter().map(|h| h.len as usize).sum();
                    jobs.push(Job { start, end, first_len: hay[f.start].len as usize, parts, matched });
                    if !all {
                        break 'streams;
                    }
                }
            }
        }
        let count = jobs.len();
        let mut caret: Option<Pos> = None;
        // The caret ends after the last replacement; earlier ones in the same paragraph move it.
        let note = |caret: &mut Option<Pos>, job: &Job, inserted: Option<Pos>| match (caret.as_mut(), inserted) {
            (None, Some(at)) => *caret = Some(at),
            (Some(cr), Some(at)) if job.start.p == job.end.p && cr.p == job.start.p => {
                let delta = at.o as isize - (job.start.o + job.matched) as isize;
                cr.o = (cr.o as isize + delta).max(0) as usize;
            }
            _ => {}
        };
        // Plain text for plain text, within one paragraph each: all of a paragraph's matches in one go.
        let plain = text_change && format.is_none() && para_format.is_none() && lang.is_none();
        let simple = |j: &Job| j.start.p == j.end.p && !j.parts.iter().any(|p| matches!(p, Part::Para));
        let mut i = jobs.len();
        while i > 0 {
            i -= 1;
            if plain && simple(&jobs[i]) {
                let pid = jobs[i].start.p.0;
                let mut lo = i;
                while lo > 0 && jobs[lo - 1].start.p.0 == pid && simple(&jobs[lo - 1]) {
                    lo -= 1;
                }
                self.touch(pid);
                let p = self.para_mut(pid)?;
                for job in jobs[lo..=i].iter().rev() {
                    let (_, at) = edit_inlines(p, job.start.o, job.end.o, job.first_len, &job.parts);
                    note(&mut caret, job, Some(Pos::new(pid, at)));
                }
                i = lo;
                continue;
            }
            let job = &jobs[i];
            let inserted = self.apply_replacement(job.start, job.end, job.first_len, &job.parts, text_change, format, para_format, lang.as_deref())?;
            note(&mut caret, job, inserted);
        }
        Ok(OpResult { caret, count: Some(count), ..Default::default() })
    }

    /// Replace one match. Returns where the new text ends.
    #[allow(clippy::too_many_arguments)]
    fn apply_replacement(
        &mut self,
        start: Pos,
        end: Pos,
        first_len: usize,
        parts: &[Part],
        text_change: bool,
        format: Option<&RunPatch>,
        para_format: Option<&ParaPatch>,
        language: Option<&str>,
    ) -> Result<Option<Pos>, EditError> {
        let mut para_ids: Vec<NodeId> = vec![start.p.0];
        let mut ranges: Vec<(NodeId, usize, usize)> = Vec::new();
        let mut at_end = None;
        if text_change {
            // The first character stays until the new text is in, so the new text takes its formatting.
            let keep = first_len;
            let from = Pos::new(start.p.0, start.o + keep);
            let (mut pid, mut off) = (start.p.0, from.o);
            if start.p == end.p && parts.iter().all(|p| !matches!(p, Part::Para)) {
                // Within one paragraph: edit the inlines directly.
                self.touch(pid);
                let p = self.para_mut(pid)?;
                let (inserted, at) = edit_inlines(p, start.o, end.o, keep, parts);
                ranges.extend(inserted.into_iter().map(|(a, b)| (pid, a, b)));
                off = at;
            } else {
                if (end.p.0, end.o) != (from.p.0, from.o) {
                    self.apply_inner(&Op::Delete { from, to: end })?;
                }
                for part in parts {
                    match part {
                        Part::Text(t) => {
                            self.apply_inner(&Op::InsertText { at: Pos::new(pid, off), text: t.clone(), format: None })?;
                            let n = t.encode_utf16().count();
                            ranges.push((pid, off, off + n));
                            off += n;
                        }
                        Part::Page | Part::Column => {
                            let kind = if *part == Part::Page { "page" } else { "column" };
                            self.touch(pid);
                            let p = self.para_mut(pid)?;
                            let br = Element::new("w:br").with_attr("w:type", kind);
                            edit::insert_inline_at(&mut p.inlines, off, Inline::Run(Run { items: vec![RunItem::Break(br)], ..Default::default() }));
                            ranges.push((pid, off, off + 1));
                            off += 1;
                        }
                        Part::Para => {
                            let r = self.split_paragraph(Pos::new(pid, off))?;
                            pid = r.caret.map(|c| c.p.0).unwrap_or(pid);
                            para_ids.push(pid);
                            off = 0;
                        }
                    }
                }
                if keep > 0 {
                    self.apply_inner(&Op::Delete { from: start, to: Pos::new(start.p.0, start.o + keep) })?;
                    for r in ranges.iter_mut().filter(|r| r.0 == start.p.0) {
                        r.1 -= keep;
                        r.2 -= keep;
                    }
                    if pid == start.p.0 {
                        off -= keep;
                    }
                }
            }
            at_end = Some(Pos::new(pid, off));
        } else {
            // Formatting only: the matched text is the range to format.
            let order = paragraph_order(&self.body);
            let a = order.iter().position(|x| *x == start.p.0);
            let b = order.iter().position(|x| *x == end.p.0);
            if let (Some(a), Some(b)) = (a, b) {
                para_ids = order[a..=b.max(a)].to_vec();
                // A match that ends at the start of the next paragraph (a mark) doesn't format that paragraph.
                if end.o == 0 && para_ids.len() > 1 {
                    para_ids.pop();
                }
            }
        }
        if let Some(patch) = format {
            if text_change {
                for (pid, a, b) in &ranges {
                    self.apply_inner(&Op::FormatText { from: Pos::new(*pid, *a), to: Pos::new(*pid, *b), format: patch.clone() })?;
                }
            } else {
                self.apply_inner(&Op::FormatText { from: start, to: end, format: patch.clone() })?;
            }
        }
        if let Some(lang) = language {
            let spans: Vec<(NodeId, usize, usize)> = if text_change { ranges.clone() } else { Vec::new() };
            if text_change {
                for (pid, a, b) in spans {
                    self.set_lang_range(pid, a, b, lang)?;
                }
            } else {
                let order = paragraph_order(&self.body);
                let a = order.iter().position(|x| *x == start.p.0).unwrap_or(0);
                let b = order.iter().position(|x| *x == end.p.0).unwrap_or(a);
                for (k, pid) in order[a..=b.max(a)].iter().enumerate() {
                    let len = self.para_ref(*pid)?.len16();
                    let s = if k == 0 { start.o } else { 0 };
                    let e = if a + k == b { end.o } else { len };
                    self.set_lang_range(*pid, s, e, lang)?;
                }
            }
        }
        if let Some(pf) = para_format {
            let ids: Vec<Id> = para_ids.iter().map(|i| Id(*i)).collect();
            self.apply_inner(&Op::FormatParagraphs { ids, format: pf.clone() })?;
        }
        Ok(at_end)
    }

    /// Set the language of the text in `[s, e)` of a paragraph.
    fn set_lang_range(&mut self, pid: NodeId, s: usize, e: usize, lang: &str) -> Result<(), EditError> {
        self.touch(pid);
        let p = self.para_mut(pid)?;
        let len = p.len16();
        edit::format_range(&mut p.inlines, s.min(len), e.min(len), &mut |run| {
            let rpr = run.rpr.get_or_insert_with(|| Element::new("w:rPr"));
            let mut el = rpr.child("w:lang").cloned().unwrap_or_else(|| Element::new("w:lang"));
            el.set_attr("w:val", lang);
            crate::props::put_rpr(rpr, el);
        });
        Ok(())
    }

    /// `replace_text` with no paragraphs named: the main text, then each story, unless `main_only`.
    pub(super) fn replace_everywhere(&mut self, op: &Op) -> Result<OpResult, EditError> {
        let Op::ReplaceText { find, replace, all, options, format, para_format, language, .. } = op else {
            return Err(EditError::Invalid("not a replace".into()));
        };
        let run = |d: &mut Document| d.replace_in_body(find, replace, None, *all, options, format.as_ref(), para_format.as_ref(), language.as_deref());
        let mut total = 0;
        let mut caret = None;
        // A direction from a position stays in the text that position is in.
        let scoped = options.from.is_some() && options.direction != Direction::All;
        let from_story = if scoped { options.from.and_then(|f| self.story_index(f.p.0)) } else { None };
        if from_story.is_none() {
            let r = run(self)?;
            total += r.count.unwrap_or(0);
            caret = caret.or(r.caret);
        }
        if !options.main_only && (*all || total == 0) {
            let stories: Vec<usize> = if scoped { from_story.into_iter().collect() } else { (0..self.stories.len()).collect() };
            for si in stories {
                if total > 0 && !*all {
                    break;
                }
                let r = self.in_story_with(si, &mut |d| run(d))?;
                total += r.count.unwrap_or(0);
                caret = caret.or(r.caret);
            }
        }
        if total == 0 {
            return Err(EditError::Invalid(if find.is_empty() { "no text has that formatting".to_string() } else { format!("text not found: {find:?}") }));
        }
        Ok(OpResult { caret, count: Some(total), ..Default::default() })
    }
}
