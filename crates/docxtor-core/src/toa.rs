//! The table of authorities, as lawyers use it: `TA` fields mark citations in the text (zero-width and hidden, like
//! `XE` index entries), and a `TOA` field lists them by category, sorted, with their page numbers, as Word writes
//! it: a `TOA Heading` paragraph for each category, then one `Table of Authorities` paragraph for each entry.
//!
//! A `TA` field is ` TA \l "Smith v. Jones, 123 U.S. 456 (1990)" \s "Smith" \c 1 `: the long citation, the short
//! citation (a later `TA \s "Smith"` counts for the same entry) and the category, 1 to 16. The long citation keeps the
//! formatting of the runs of its instruction (case names in italics), which the table repeats when it keeps the
//! original formatting (`\f`).

use crate::fields::{Instr, SeqCounter};
use crate::index::{keep_other_switches, sep_runs, sort_key, text_run, wrap_field};
use crate::model::{walk_paragraphs, Block, Inline, InlineKind, NodeId, RunItem};
use crate::toc::Leader;
use crate::xml::{Element, Node, XmlDoc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

/// The first and last of the categories whose names can be changed.
pub const FIRST_CUSTOM: u8 = 8;
pub const LAST_CATEGORY: u8 = 16;

const NAMES: [&str; 7] = ["Cases", "Statutes", "Other Authorities", "Rules", "Treatises", "Regulations", "Constitutional Provisions"];

/// The name Word gives a category: seven fixed ones, then `Category 8` to `Category 16`.
pub fn default_name(category: u8) -> String {
    match category {
        1..=7 => NAMES[category as usize - 1].to_string(),
        n => format!("Category {}", n.clamp(FIRST_CUSTOM, LAST_CATEGORY)),
    }
}

// ---------------------------------------------------------------------------
// Formatting that a citation keeps
// ---------------------------------------------------------------------------

/// The formatting of a piece of a long citation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fmt {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub small_caps: bool,
}

impl Fmt {
    pub fn of_rpr(rpr: Option<&Element>) -> Fmt {
        let on = |name: &str| rpr.and_then(|r| r.child(name)).is_some_and(|e| !matches!(e.attr("w:val"), Some("0" | "false" | "off")));
        Fmt { bold: on("w:b"), italic: on("w:i"), underline: rpr.and_then(|r| r.child("w:u")).is_some_and(|e| e.attr("w:val") != Some("none")), small_caps: on("w:smallCaps") }
    }

    pub fn is_plain(&self) -> bool {
        *self == Fmt::default()
    }

    /// A run's `w:rPr` with this formatting, hidden when asked (the instruction of a `TA` field is).
    pub fn rpr(&self, hidden: bool) -> Option<Element> {
        let mut r = Element::new("w:rPr");
        if self.bold {
            crate::props::put_rpr(&mut r, Element::new("w:b"));
            crate::props::put_rpr(&mut r, Element::new("w:bCs"));
        }
        if self.italic {
            crate::props::put_rpr(&mut r, Element::new("w:i"));
            crate::props::put_rpr(&mut r, Element::new("w:iCs"));
        }
        if self.small_caps {
            crate::props::put_rpr(&mut r, Element::new("w:smallCaps"));
        }
        if hidden {
            crate::props::put_rpr(&mut r, Element::new("w:vanish"));
        }
        if self.underline {
            crate::props::put_rpr(&mut r, Element::new("w:u").with_attr("w:val", "single"));
        }
        (!r.children.is_empty()).then_some(r)
    }
}

/// A stretch of a long citation with one formatting.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub text: String,
    pub fmt: Fmt,
}

pub fn plain(text: &str) -> Vec<Piece> {
    vec![Piece { text: text.to_string(), fmt: Fmt::default() }]
}

pub fn pieces_text(pieces: &[Piece]) -> String {
    pieces.iter().map(|p| p.text.as_str()).collect()
}

type Chars = Vec<(char, Fmt)>;

fn merge(cs: &[(char, Fmt)]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for (c, f) in cs {
        match out.last_mut() {
            Some(p) if p.fmt == *f => p.text.push(*c),
            _ => out.push(Piece { text: c.to_string(), fmt: *f }),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Reading TA fields
// ---------------------------------------------------------------------------

/// The pages a citation is on: one page, or the pages from its place to the end of a bookmark (`\r`). The labels
/// are what the pages print as (a section can number its pages i, ii, iii).
#[derive(Debug, Clone, PartialEq)]
pub struct PageSpan {
    pub from: u32,
    pub to: u32,
    pub from_label: String,
    pub to_label: String,
}

/// One `TA` field in the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Citation {
    /// The paragraph the field is in (the page of its entry comes from there).
    pub para: NodeId,
    /// `\l`: the long citation, as listed in the table.
    pub long: Option<Vec<Piece>>,
    /// `\s`: the short citation, which finds the long one for a later mark.
    pub short: Option<String>,
    /// `\c`: 1 to 16.
    pub category: u8,
    /// Whether the field says its category. Word's later marks (` TA \s "short" `) don't: they are filed with the long
    /// citation that has that short form.
    pub own_category: bool,
    /// `\b` and `\i`: the page number is bold or italic.
    pub bold: bool,
    pub italic: bool,
    /// `\r`: a bookmark whose end is the last page of a range.
    pub range: Option<String>,
    /// The `SEQ` number in effect at the mark, when the table asks for it (`\s`).
    pub seq: Option<String>,
    /// Filled in by the document from its layout.
    pub pages: Option<PageSpan>,
}

impl Citation {
    pub fn new(long: Option<Vec<Piece>>, short: Option<String>, category: u8) -> Citation {
        Citation { para: 0, long, short, category: category.clamp(1, LAST_CATEGORY), own_category: true, bold: false, italic: false, range: None, seq: None, pages: None }
    }

    pub fn long_text(&self) -> Option<String> {
        self.long.as_ref().map(|l| pieces_text(l))
    }
}

struct Tok {
    quoted: bool,
    chars: Chars,
}

impl Tok {
    fn text(&self) -> String {
        self.chars.iter().map(|(c, _)| *c).collect()
    }
    fn is_switch(&self) -> bool {
        !self.quoted && self.chars.len() >= 2 && self.chars[0].0 == '\\'
    }
}

/// Words of an instruction, like `fields::Instr` reads them, keeping each character's formatting.
fn tokenize(cs: &[(char, Fmt)]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i].0;
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\u{201C}' {
            // A token ends with the kind of quote that opened it: a curly quote inside a straight-quoted citation
            // (“The Law of Things,”) is part of the text.
            let close = if c == '"' { '"' } else { '\u{201D}' };
            i += 1;
            let mut chars = Vec::new();
            while i < cs.len() {
                match cs[i].0 {
                    '\\' if i + 1 < cs.len() => {
                        chars.push(cs[i]);
                        chars.push(cs[i + 1]);
                        i += 2;
                    }
                    q if q == close => {
                        i += 1;
                        break;
                    }
                    _ => {
                        chars.push(cs[i]);
                        i += 1;
                    }
                }
            }
            out.push(Tok { quoted: true, chars });
        } else {
            let mut chars = Vec::new();
            while i < cs.len() && !cs[i].0.is_whitespace() {
                chars.push(cs[i]);
                i += 1;
            }
            out.push(Tok { quoted: false, chars });
        }
    }
    out
}

/// `\"` to `"`, `\\` to `\`, `\:` to `:`, keeping the formatting of the characters that stay.
fn unescape(cs: &[(char, Fmt)]) -> Chars {
    let mut out = Vec::with_capacity(cs.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i].0 == '\\' && i + 1 < cs.len() && matches!(cs[i + 1].0, '"' | '\\' | ':') {
            out.push(cs[i + 1]);
            i += 2;
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

/// Read a `TA` instruction written as characters with their formatting.
fn parse_chars(cs: &[(char, Fmt)]) -> Option<Citation> {
    let toks = tokenize(cs);
    let first = toks.first()?;
    if first.quoted || !first.text().eq_ignore_ascii_case("TA") {
        return None;
    }
    let mut c = Citation::new(None, None, 1);
    c.own_category = false;
    let mut k = 1;
    while k < toks.len() {
        let t = &toks[k];
        if !t.is_switch() {
            k += 1;
            continue;
        }
        let name = t.text()[1..].to_lowercase();
        let value = toks.get(k + 1).filter(|n| !n.is_switch());
        match (name.as_str(), value) {
            ("l", Some(v)) => {
                let pieces = merge(&unescape(&v.chars));
                if !pieces_text(&pieces).trim().is_empty() {
                    c.long = Some(pieces);
                }
            }
            ("s", Some(v)) => {
                let s: String = unescape(&v.chars).iter().map(|(c, _)| *c).collect();
                if !s.trim().is_empty() {
                    c.short = Some(s);
                }
            }
            ("c", Some(v)) => {
                c.category = v.text().trim().parse::<u8>().ok().filter(|n| (1..=LAST_CATEGORY).contains(n)).unwrap_or(1);
                c.own_category = true;
            }
            ("r", Some(v)) => c.range = Some(unescape(&v.chars).iter().map(|(c, _)| *c).collect()),
            ("b", _) => c.bold = true,
            ("i", _) => c.italic = true,
            _ => {}
        }
        k += if value.is_some() && matches!(name.as_str(), "l" | "s" | "c" | "r") { 2 } else { 1 };
    }
    (c.long.is_some() || c.short.is_some()).then_some(c)
}

/// Read a `TA` instruction (plain text, without formatting).
pub fn parse_ta(instr: &str) -> Option<Citation> {
    let cs: Chars = instr.chars().map(|c| (c, Fmt::default())).collect();
    parse_chars(&cs)
}

/// Whether an instruction is a `TA` field.
pub fn is_ta(instr: &str) -> bool {
    Instr::parse(instr).code == "TA"
}

/// The instruction of a `TA` field: ` TA \l "long" \s "short" \c 1 \b \i `. A `TA` for a later mention of a short
/// citation has no long one.
pub fn ta_instr(c: &Citation) -> String {
    let mut s = String::from(" TA");
    if let Some(l) = c.long_text() {
        s.push_str(&format!(" \\l \"{}\"", crate::fields::escape(&l, false)));
    }
    if let Some(sh) = &c.short {
        s.push_str(&format!(" \\s \"{}\"", crate::fields::escape(sh, false)));
    }
    // A later mark that didn't say its category keeps it that way (it follows its long citation).
    if c.own_category || c.long.is_some() {
        s.push_str(&format!(" \\c {}", c.category.clamp(1, LAST_CATEGORY)));
    }
    if c.bold {
        s.push_str(" \\b");
    }
    if c.italic {
        s.push_str(" \\i");
    }
    if let Some(r) = &c.range {
        s.push_str(&format!(" \\r \"{}\"", crate::fields::escape(r, false)));
    }
    s.push(' ');
    s
}

fn hidden_run(fmt: Fmt, item: RunItem) -> Inline {
    Inline::Run(crate::model::Run { attrs: vec![], t_attrs: None, rpr: fmt.rpr(true), items: vec![item] })
}

/// The runs of a `TA` field, hidden as Word hides them: the instruction is split wherever the long citation changes
/// formatting, so italic case names stay italic in the field's code.
pub fn ta_inlines(c: &Citation) -> Vec<Inline> {
    let plain_fmt = Fmt::default();
    let fld = |kind: &str| hidden_run(plain_fmt, RunItem::Hidden(Element::new("w:fldChar").with_attr("w:fldCharType", kind)));
    let instr = |text: &str, fmt: Fmt| hidden_run(fmt, RunItem::Hidden(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(text)));
    let mut out = vec![fld("begin")];
    match &c.long {
        Some(pieces) if pieces.iter().any(|p| !p.fmt.is_plain()) => {
            let mut rest = c.clone();
            rest.long = Some(plain("\u{0}"));
            let full = ta_instr(&rest);
            let (head, tail) = full.split_once('\u{0}').unwrap_or((&full, ""));
            out.push(instr(head, plain_fmt));
            for p in pieces {
                out.push(instr(&crate::fields::escape(&p.text, false), p.fmt));
            }
            out.push(instr(tail, plain_fmt));
        }
        _ => out.push(instr(&ta_instr(c), plain_fmt)),
    }
    out.push(fld("end"));
    out
}

/// The fields of one level of a paragraph, in the order they begin, as their instruction's characters with the
/// formatting of the run each came from.
fn level_fields(level: &[Inline], out: &mut Vec<Chars>) {
    let mut stack: Vec<(usize, bool)> = Vec::new();
    for inl in level {
        match inl {
            Inline::Run(r) => {
                let fmt = Fmt::of_rpr(r.rpr.as_ref());
                for item in &r.items {
                    match item {
                        RunItem::Hidden(e) if e.name == "w:fldChar" => match e.attr("w:fldCharType") {
                            Some("begin") => {
                                out.push(Vec::new());
                                stack.push((out.len() - 1, true));
                            }
                            Some("separate") => {
                                if let Some(t) = stack.last_mut() {
                                    t.1 = false;
                                }
                            }
                            Some("end") => {
                                stack.pop();
                            }
                            _ => {}
                        },
                        RunItem::Hidden(e) if e.name == "w:instrText" => {
                            if let Some(&(i, true)) = stack.last() {
                                out[i].extend(e.text().chars().map(|c| (c, fmt)));
                            }
                        }
                        RunItem::Tab => {
                            if let Some(&(i, true)) = stack.last() {
                                if !out[i].is_empty() {
                                    out[i].push(('\t', fmt));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Inline::Container(c) => {
                if c.kind == InlineKind::Field && c.shell.attr("w:instr").is_some() {
                    out.push(c.shell.attr("w:instr").unwrap_or_default().chars().map(|ch| (ch, Fmt::default())).collect());
                } else {
                    level_fields(&c.children, out);
                }
            }
            Inline::Opaque(_) => {}
        }
    }
}

/// Every `TA` field in these blocks, in order. With `seq`, the number of that sequence in effect at each one is
/// kept too (a table that adds sequence numbers to its page numbers).
pub fn collect<'a>(blocks: impl IntoIterator<Item = &'a Block>, seq: Option<&str>) -> Vec<Citation> {
    let mut out = Vec::new();
    let mut counter = SeqCounter::default();
    let mut current: Option<String> = None;
    walk_paragraphs(blocks, &mut |p| {
        let mut fields = Vec::new();
        level_fields(&p.inlines, &mut fields);
        for cs in fields {
            if let Some(name) = seq {
                let text: String = cs.iter().map(|(c, _)| *c).collect();
                let i = Instr::parse(&text);
                if i.code == "SEQ" {
                    if let Some(v) = counter.next(&text) {
                        if i.first_arg().is_some_and(|n| n.eq_ignore_ascii_case(name)) {
                            current = Some(v);
                        }
                    }
                    continue;
                }
            }
            if let Some(mut c) = parse_chars(&cs) {
                c.para = p.id;
                c.seq = current.clone();
                out.push(c);
            }
        }
    });
    out
}

// ---------------------------------------------------------------------------
// The TOA field
// ---------------------------------------------------------------------------

/// How a `TOA` field asks for its result.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// `\c`: one category, or 0 for all of them.
    pub category: u8,
    /// `\p`: five pages or more become "passim".
    pub passim: bool,
    /// The entries keep the formatting of their long citations: no `\f` (Word's `\f` removes it; see Microsoft's
    /// "Field codes: TOA" and ECMA-376 17.16.5.68).
    pub keep_format: bool,
    /// `\h`: a heading for each category.
    pub headings: bool,
    /// `\e`: between an entry and its page numbers (a tab is the default, and aligns them at the right edge).
    pub sep: String,
    /// `\l`: between the page numbers of an entry.
    pub page_sep: String,
    /// `\g`: between the first and last page of a range.
    pub range_sep: String,
    /// `\s` and `\d`: a sequence whose number goes before each page number, and what goes between them.
    pub seq: Option<String>,
    pub seq_sep: String,
    /// `\b`: only citations inside this bookmark.
    pub bookmark: Option<String>,
}

impl Options {
    pub fn parse(instr: &str) -> Options {
        let i = Instr::parse(instr);
        let sep = i.value("e");
        Options {
            category: i.value("c").and_then(|v| v.trim().parse::<u8>().ok()).filter(|n| *n <= LAST_CATEGORY).unwrap_or(0),
            passim: i.has("p"),
            keep_format: !i.has("f"),
            headings: i.has("h"),
            sep: match sep.as_deref() {
                None => "\t".into(),
                Some("\\t") => "\t".into(),
                Some(s) => s.to_string(),
            },
            page_sep: i.value("l").unwrap_or_else(|| ", ".into()),
            range_sep: i.value("g").unwrap_or_else(|| "\u{2013}".into()),
            seq: i.value("s").filter(|s| !s.trim().is_empty()),
            seq_sep: i.value("d").unwrap_or_else(|| "-".into()),
            bookmark: i.value("b").filter(|s| !s.trim().is_empty()),
        }
    }
}

/// Whether an instruction is a `TOA` field.
pub fn is_toa(instr: &str) -> bool {
    Instr::parse(instr).code == "TOA"
}

fn yes() -> bool {
    true
}

/// What the "Table of authorities" dialog sets, as data. `instr` turns it into the field's instruction; the
/// document writes the leader and the look into the `Table of Authorities` and `TOA Heading` styles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToaOptions {
    /// 0 lists every category, 1 to 16 one of them.
    #[serde(default)]
    pub category: u8,
    /// "passim" in place of five or more pages.
    #[serde(default)]
    pub passim: bool,
    /// Entries keep the formatting of their long citation (Word's "Keep original formatting", ticked by default). Off
    /// writes `\f`.
    #[serde(default = "yes")]
    pub keep_formatting: bool,
    /// A heading for each category.
    #[serde(default = "yes")]
    pub headings: bool,
    /// Page numbers at the right edge, after a leader.
    #[serde(default = "yes")]
    pub right_align: bool,
    #[serde(default)]
    pub leader: Leader,
    /// Between the page numbers of an entry (`\l`; default ", ").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_sep: Option<String>,
    /// Between the first and last page of a range (`\g`; default an en dash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_sep: Option<String>,
    /// A sequence (`SEQ` name) whose number goes before each page number (`\s`), and the text between (`\d`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq_sep: Option<String>,
    /// List only the citations inside this bookmark (`\b`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,
    /// One of the formats (`classic`, `distinctive`, `formal`, `simple`); None leaves the styles as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
}

impl Default for ToaOptions {
    fn default() -> Self {
        ToaOptions { category: 0, passim: false, keep_formatting: true, headings: true, right_align: true, leader: Leader::Dot, page_sep: None, range_sep: None, seq: None, seq_sep: None, bookmark: None, look: None }
    }
}

impl ToaOptions {
    /// The options an instruction stands for. The leader and the look are in the styles and the paragraphs, so the
    /// caller passes the leader the existing result uses.
    pub fn from_instr(instr: &str, leader: Leader) -> ToaOptions {
        let o = Options::parse(instr);
        let i = Instr::parse(instr);
        ToaOptions {
            category: o.category,
            passim: o.passim,
            keep_formatting: o.keep_format,
            headings: o.headings,
            right_align: o.sep.contains('\t'),
            leader,
            page_sep: i.value("l"),
            range_sep: i.value("g"),
            seq: o.seq,
            seq_sep: i.value("d"),
            bookmark: o.bookmark,
            look: None,
        }
    }

    /// The `TOA` instruction for these options, as Word writes it (` TOA \h \c "1" \e "<tab>" \p `). Switches the
    /// dialog doesn't set are carried over from `old`, the instruction being replaced.
    pub fn instr(&self, old: Option<&str>) -> String {
        let mut s = String::from(" TOA");
        if self.headings {
            s.push_str(" \\h");
        }
        s.push_str(&format!(" \\c \"{}\"", self.category.min(LAST_CATEGORY)));
        s.push_str(if self.right_align { " \\e \"\t\"" } else { " \\e \", \"" });
        if self.passim {
            s.push_str(" \\p");
        }
        if !self.keep_formatting {
            s.push_str(" \\f");
        }
        let flag = |v: &Option<String>, skip: &str| v.as_deref().filter(|v| !v.is_empty() && *v != skip).map(String::from);
        if let Some(l) = flag(&self.page_sep, ", ") {
            s.push_str(&format!(" \\l \"{}\"", crate::fields::escape(&l, false)));
        }
        if let Some(g) = flag(&self.range_sep, "\u{2013}") {
            s.push_str(&format!(" \\g \"{}\"", crate::fields::escape(&g, false)));
        }
        if let Some(q) = self.seq.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            s.push_str(&format!(" \\s \"{}\"", crate::fields::escape(q, false)));
            if let Some(d) = flag(&self.seq_sep, "-") {
                s.push_str(&format!(" \\d \"{}\"", crate::fields::escape(&d, false)));
            }
        }
        if let Some(b) = self.bookmark.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
            s.push_str(&format!(" \\b \"{}\"", crate::fields::escape(b, false)));
        }
        if let Some(old) = old {
            keep_other_switches(&mut s, old, &["h", "c", "e", "p", "f", "l", "g", "s", "d", "b"]);
        }
        s.push(' ');
        s
    }
}

// ---------------------------------------------------------------------------
// Building the table
// ---------------------------------------------------------------------------

/// A page number of an entry, with the formatting its mark asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub span: PageSpan,
    pub bold: bool,
    pub italic: bool,
    pub seq: Option<String>,
}

/// One authority in the table.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub category: u8,
    pub text: Vec<Piece>,
    pub pages: Vec<Page>,
}

/// Sort the citations into entries: a short citation without a long one joins the entry that has the same short
/// citation; entries are grouped by category, sorted without regard to case or accents, and their pages are merged.
pub fn build(cites: &[Citation], opts: &Options) -> Vec<Entry> {
    let mut by_short: HashMap<(u8, String), Vec<Piece>> = HashMap::new();
    let mut by_short_any: HashMap<String, Vec<Piece>> = HashMap::new();
    let mut category_of_short: HashMap<String, u8> = HashMap::new();
    for c in cites {
        if let (Some(l), Some(s)) = (&c.long, &c.short) {
            by_short.entry((c.category, crate::index::fold(s.trim()))).or_insert_with(|| l.clone());
            by_short_any.entry(crate::index::fold(s.trim())).or_insert_with(|| l.clone());
            category_of_short.entry(crate::index::fold(s.trim())).or_insert(c.category);
        }
    }
    // A later mark that doesn't say its category is filed with its long citation, before the table picks a category.
    let category = |c: &Citation| match (&c.long, &c.short) {
        (None, Some(s)) if !c.own_category => category_of_short.get(&crate::index::fold(s.trim())).copied().unwrap_or(c.category),
        _ => c.category,
    };
    let mut entries: Vec<Entry> = Vec::new();
    let mut slot: HashMap<(u8, String), usize> = HashMap::new();
    for c in cites.iter().filter(|c| opts.category == 0 || category(c) == opts.category) {
        let cat = category(c);
        let text = match (&c.long, &c.short) {
            (Some(l), _) => l.clone(),
            (None, Some(s)) => {
                let key = crate::index::fold(s.trim());
                by_short.get(&(cat, key.clone())).or_else(|| by_short_any.get(&key)).cloned().unwrap_or_else(|| plain(s.trim()))
            }
            (None, None) => continue,
        };
        let key = (cat, crate::index::fold(pieces_text(&text).trim()));
        let at = *slot.entry(key).or_insert_with(|| {
            entries.push(Entry { category: cat, text, pages: vec![] });
            entries.len() - 1
        });
        let Some(span) = &c.pages else { continue };
        let pages = &mut entries[at].pages;
        match pages.iter_mut().find(|p| p.span.from == span.from && p.span.to == span.to && p.seq == c.seq) {
            Some(p) => {
                p.bold |= c.bold;
                p.italic |= c.italic;
            }
            None => pages.push(Page { span: span.clone(), bold: c.bold, italic: c.italic, seq: c.seq.clone() }),
        }
    }
    for e in entries.iter_mut() {
        e.pages.sort_by_key(|p| (p.span.from, p.span.to));
    }
    entries.sort_by_cached_key(|e| (e.category, sort_key(pieces_text(&e.text).trim(), None)));
    entries
}

/// How many different pages an entry is on.
fn distinct_pages(e: &Entry) -> usize {
    let mut set = BTreeSet::new();
    for p in &e.pages {
        for n in p.span.from..=p.span.to.min(p.span.from.saturating_add(10_000)) {
            set.insert(n);
        }
    }
    set.len()
}

/// The page numbers of an entry as runs: `3, 5, 8-9`, each with the formatting its mark asked for, or "passim".
fn page_runs(e: &Entry, opts: &Options) -> Vec<Element> {
    if opts.passim && distinct_pages(e) >= 5 {
        return vec![text_run("passim", false, false)];
    }
    let mut out = Vec::new();
    for (i, p) in e.pages.iter().enumerate() {
        if i > 0 {
            out.push(text_run(&opts.page_sep, false, false));
        }
        let pages = if p.span.from == p.span.to { p.span.from_label.clone() } else { format!("{}{}{}", p.span.from_label, opts.range_sep, p.span.to_label) };
        let text = match (&opts.seq, &p.seq) {
            (Some(_), Some(s)) if !s.is_empty() => format!("{s}{}{pages}", opts.seq_sep),
            _ => pages,
        };
        out.push(text_run(&text, p.bold, p.italic));
    }
    out
}

fn fmt_run(text: &str, fmt: Fmt) -> Element {
    crate::model::text_run_element(fmt.rpr(false), text)
}

/// The paragraphs of a `TOA` field's result (the field's begin is in the first, its end in the last). `tab_pos` is
/// where page numbers align (the text's right edge, in twips); `name_of` gives a category's heading.
pub fn paragraphs(entries: &[Entry], instr: &str, opts: &Options, name_of: &dyn Fn(u8) -> String, tab_pos: i64, leader: Leader) -> Vec<Element> {
    let right_tab = opts.sep.contains('\t');
    let para = |style: &str, tabs: bool, runs: Vec<Element>| {
        let mut ppr = Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style));
        if tabs {
            ppr = ppr.with_child(Element::new("w:tabs").with_child(Element::new("w:tab").with_attr("w:val", "right").with_attr("w:leader", leader.val()).with_attr("w:pos", tab_pos.to_string())));
        }
        let mut el = Element::new("w:p").with_child(ppr);
        for r in runs {
            el = el.with_child(r);
        }
        el
    };
    let mut paras: Vec<Element> = Vec::new();
    let mut last: Option<u8> = None;
    for e in entries {
        if opts.headings && last != Some(e.category) {
            paras.push(para("TOAHeading", false, vec![text_run(&name_of(e.category), false, false)]));
        }
        last = Some(e.category);
        let mut runs: Vec<Element> = e.text.iter().map(|p| if opts.keep_format && !p.fmt.is_plain() { fmt_run(&p.text, p.fmt) } else { text_run(&p.text, false, false) }).collect();
        let pages = page_runs(e, opts);
        if !pages.is_empty() {
            runs.extend(sep_runs(&opts.sep));
            runs.extend(pages);
        }
        paras.push(para("TableofAuthorities", right_tab, runs));
    }
    if paras.is_empty() {
        paras.push(para("TableofAuthorities", false, vec![text_run("No table of authorities entries found.", false, false)]));
    }
    wrap_field(&mut paras, instr);
    paras
}

// ---------------------------------------------------------------------------
// The ops
// ---------------------------------------------------------------------------

fn one() -> u8 {
    1
}

/// A new name for one of the categories 8 to 16 (an empty name goes back to `Category n`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryName {
    pub category: u8,
    pub name: String,
}

/// Table of authorities ops. The wire form is `{"op": "toa", "action": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ToaOp {
    /// Mark a citation: a hidden `TA` field at `at` (after the cited text). `from` is where the cited text starts, so
    /// the long citation keeps its formatting (italic case names) when it is that text. `short` makes later mentions
    /// of the short form count for the same entry.
    MarkCitation {
        at: crate::ops::Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<crate::ops::Pos>,
        long: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        short: Option<String>,
        #[serde(default = "one")]
        category: u8,
        /// The page number is bold (`\b`) or italic (`\i`).
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
    },
    /// Mark every place the long citation (and the short one, as whole words) is in the main text. The result's
    /// `count` says how many marks were made. A place that has the mark already is left alone.
    MarkAll {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<crate::ops::Pos>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<crate::ops::Pos>,
        long: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        short: Option<String>,
        #[serde(default = "one")]
        category: u8,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        ignore_case: bool,
    },
    /// Insert a table of authorities after or before a block (default: the end).
    Insert {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<crate::ops::Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<crate::ops::Id>,
        #[serde(default)]
        options: ToaOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<HashMap<String, u32>>,
    },
    /// Change a table of authorities (the paragraph `at` is in it, else the first one) and build it again.
    Set {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<crate::ops::Id>,
        options: ToaOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<HashMap<String, u32>>,
    },
    /// Build the table again (Word's Update table): the one holding `at`, else every one.
    Update {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<crate::ops::Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<HashMap<String, u32>>,
    },
    /// Rename categories 8 to 16.
    SetCategoryNames { names: Vec<CategoryName> },
}

impl ToaOp {
    /// The flags of the op (see `OpClass`): marking and inserting happen in the main text; the category names are not
    /// in the body.
    pub fn class(&self) -> crate::ops::OpClass {
        use crate::ops::OpClass as C;
        match self {
            ToaOp::SetCategoryNames { .. } => C::BODY.main_text().leaves_body(),
            ToaOp::MarkCitation { .. } | ToaOp::MarkAll { .. } | ToaOp::Insert { .. } | ToaOp::Set { .. } | ToaOp::Update { .. } => C::BODY.main_text(),
        }
    }

    pub fn anchor(&self) -> Option<NodeId> {
        match self {
            ToaOp::MarkCitation { at, .. } => Some(at.p.0),
            ToaOp::Insert { after, before, .. } => after.or(*before).map(|i| i.0),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Category names
// ---------------------------------------------------------------------------

const NAMES_PROP: &str = "_DocxtorToaCategories";
const FMTID: &str = "{D5CDD505-2E9C-101B-9397-08002B2CF9AE}";

/// The names given to categories 8 to 16, kept in a private custom property of the document (Word keeps them
/// per user instead). Missing ones have their default names.
pub fn read_names(custom: Option<&XmlDoc>) -> HashMap<u8, String> {
    let mut out = HashMap::new();
    let Some(d) = custom else { return out };
    let Some(v) = d.root.elements().find(|p| p.attr("name") == Some(NAMES_PROP)).and_then(|p| p.elements().next()).map(|v| v.text()) else { return out };
    for line in v.lines() {
        if let Some((n, name)) = line.split_once('\t') {
            if let Ok(n) = n.trim().parse::<u8>() {
                if (FIRST_CUSTOM..=LAST_CATEGORY).contains(&n) && !name.trim().is_empty() {
                    out.insert(n, name.trim().to_string());
                }
            }
        }
    }
    out
}

/// Write the names (only those that differ from the defaults); none removes the property.
pub fn write_names(doc: &mut XmlDoc, names: &HashMap<u8, String>) {
    doc.root.children.retain(|n| !matches!(n, Node::Element(p) if p.attr("name") == Some(NAMES_PROP)));
    let mut keys: Vec<&u8> = names.keys().filter(|n| (FIRST_CUSTOM..=LAST_CATEGORY).contains(*n)).collect();
    keys.sort();
    let text: Vec<String> = keys.into_iter().filter(|n| names[n] != default_name(**n)).map(|n| format!("{n}\t{}", names[n])).collect();
    if text.is_empty() {
        return;
    }
    let pid = doc.root.elements().filter_map(|p| p.attr("pid")).filter_map(|v| v.parse::<u32>().ok()).max().unwrap_or(1) + 1;
    let prop = Element::new("property").with_attr("fmtid", FMTID).with_attr("pid", pid.to_string()).with_attr("name", NAMES_PROP).with_child(Element::new("vt:lpwstr").with_text(text.join("\n")));
    doc.root.children.push(Node::Element(prop));
}

// ---------------------------------------------------------------------------
// Next citation
// ---------------------------------------------------------------------------

struct Word {
    s: usize,
    e: usize,
    text: String,
}

fn words_of(chars: &[char]) -> Vec<Word> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let s = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        out.push(Word { s, e: i, text: chars[s..i].iter().collect() });
    }
    out
}

const REPORTERS: &[&str] = &[
    "U.S.", "S.Ct.", "L.Ed.", "L.Ed.2d", "F.", "F.2d", "F.3d", "F.4th", "F.Supp.", "F.Supp.2d", "F.Supp.3d", "U.S.C.", "U.S.C.A.", "C.F.R.", "Fed.", "N.E.", "N.E.2d", "N.E.3d", "N.W.", "N.W.2d", "P.", "P.2d",
    "P.3d", "A.", "A.2d", "A.3d", "S.E.", "S.E.2d", "S.W.", "S.W.2d", "S.W.3d", "So.", "So.2d", "So.3d", "Cal.Rptr.", "Cal.App.", "Stat.", "S.", "Ct.",
];

const ABBREVIATIONS: &[&str] = &["Inc.", "Corp.", "Co.", "Ltd.", "L.L.C.", "Bros.", "No.", "Dr.", "Mr.", "Ms.", "Mrs.", "Jr.", "Sr.", "St.", "Mt.", "Dep't", "Ass'n", "Assn.", "Cir.", "Civ.", "Rep.", "Reg.", "Stat.", "Fed.", "App.", "Ct.", "Cal.", "Sup."];

const SIGNALS: &[&str] = &["See", "see", "Cf.", "cf.", "Compare", "But", "Accord", "Contra", "E.g.,", "e.g.,", "Also", "And", "Under", "In", "The", "Thus,", "Here,", "Moreover,", "However,", "Hence,"];

const CONNECTORS: &[&str] = &["of", "the", "and", "for", "&", "de", "la", "del", "ex", "rel.", "re", "on", "in", "v.", "a/k/a"];

fn trim_punct(s: &str) -> &str {
    s.trim_end_matches([',', ';', ':'])
}

fn is_digits(s: &str) -> bool {
    let t = trim_punct(s).trim_end_matches('.');
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
}

/// A page, pin cite or section number: digits that may carry commas, dashes and subsections (`1983(a)(1)`, `12-15`).
fn is_numberish(s: &str) -> bool {
    let t = trim_punct(s).trim_end_matches('.');
    t.chars().next().is_some_and(|c| c.is_ascii_digit()) && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '(' | ')' | '-' | '\u{2013}' | '.' | ','))
}

fn is_reporter(s: &str) -> bool {
    REPORTERS.contains(&trim_punct(s))
}

fn is_name_word(s: &str) -> bool {
    let t = s.trim_start_matches(['(', '"', '\u{201C}']);
    t.chars().next().is_some_and(|c| c.is_uppercase()) || CONNECTORS.contains(&trim_punct(t))
}

/// The word ends a sentence: a period that isn't an abbreviation's, or a question or exclamation mark.
fn ends_sentence(s: &str) -> bool {
    if s.ends_with('?') || s.ends_with('!') {
        return true;
    }
    s.ends_with('.') && !ABBREVIATIONS.contains(&s) && !is_reporter(s) && s.chars().filter(|c| *c == '.').count() == 1 && s.chars().count() > 3
}

/// Include the words from `i` that finish a citation: a volume or page, a reporter, a pin cite and the
/// parenthetical with the court and year. Returns the index of the last word taken.
fn tail(words: &[Word], from: usize) -> usize {
    let mut last = from;
    let mut i = from + 1;
    let mut taken = 0;
    while i < words.len() && taken < 10 {
        let w = &words[i].text;
        let prev = &words[i - 1].text;
        let after_comma_or_cite = prev.ends_with(',') || is_numberish(prev) || is_reporter(prev);
        if w.starts_with('(') {
            // The parenthetical runs to the word that ends with a closing parenthesis.
            let mut j = i;
            while j < words.len() && j < i + 8 && !trim_punct(&words[j].text).trim_end_matches('.').ends_with(')') {
                j += 1;
            }
            if j < words.len() && j < i + 8 {
                last = j;
            }
            break;
        }
        if !after_comma_or_cite {
            break;
        }
        if is_numberish(w) || is_reporter(w) || (w == "at" && words.get(i + 1).is_some_and(|n| is_numberish(&n.text))) {
            last = i;
            taken += 1;
            i += 1;
        } else {
            break;
        }
        if words[last].text.ends_with(';') || (words[last].text.ends_with('.') && ends_sentence(&words[last].text)) {
            break;
        }
    }
    last
}

/// The words a citation takes in, as the first and last word index, around the word `i` that gave it away.
fn span_at(words: &[Word], i: usize) -> Option<(usize, usize)> {
    let w = trim_punct(&words[i].text);
    // Smith v. Jones, 123 U.S. 456 (1990)
    if (w == "v." || w == "vs.") && i > 0 && i + 1 < words.len() {
        let mut a = i;
        while a > 0 && i - a < 6 {
            let p = &words[a - 1].text;
            if SIGNALS.contains(&p.as_str()) && !(p == "In" && words[a].text == "re") {
                break;
            }
            if !is_name_word(p) || (a < i && words[a - 1].text.ends_with(',') && a - 1 + 1 != i) {
                break;
            }
            if ends_sentence(p) {
                break;
            }
            a -= 1;
        }
        // A name does not start with "of" or "the".
        while a < i && words[a].text.chars().next().is_some_and(|c| c.is_lowercase()) {
            a += 1;
        }
        let mut b = i;
        let mut names = 0;
        while b + 1 < words.len() && names < 6 && is_name_word(&words[b + 1].text) && !SIGNALS.contains(&words[b + 1].text.as_str()) {
            b += 1;
            names += 1;
            if words[b].text.ends_with(',') || words[b].text.ends_with(';') || ends_sentence(&words[b].text) {
                break;
            }
        }
        if b == i {
            return None;
        }
        while b > i && CONNECTORS.contains(&trim_punct(&words[b].text)) {
            b -= 1;
        }
        if a == i {
            return None;
        }
        if words[b].text.ends_with(',') {
            b = tail(words, b);
        }
        return Some((a, b));
    }
    // 42 U.S.C. § 1983 (2012), Cal. Civ. Code § 1714
    if words[i].text.starts_with('\u{a7}') || words[i].text.starts_with('\u{b6}') {
        let mut a = i;
        while a > 0 && i - a < 4 {
            let p = &words[a - 1].text;
            let part = is_numberish(p) || is_reporter(p) || ABBREVIATIONS.contains(&p.as_str()) || (p.chars().next().is_some_and(|c| c.is_uppercase()) && p.ends_with('.') && !ends_sentence(p));
            if !part || p.ends_with(',') || SIGNALS.contains(&p.as_str()) {
                break;
            }
            a -= 1;
        }
        let mut b = i;
        if trim_punct(&words[i].text).chars().count() <= 2 && i + 1 < words.len() && is_numberish(&words[i + 1].text) {
            b = i + 1;
        }
        let mut taken = 0;
        while b + 1 < words.len() && taken < 6 {
            let n = &words[b + 1].text;
            let cur = &words[b].text;
            if (cur.ends_with(',') && is_numberish(n)) || (matches!(cur.as_str(), "and" | "\u{a7}\u{a7}") && is_numberish(n)) || (is_numberish(cur) && matches!(n.as_str(), "and" | "et")) {
                b += 1;
                taken += 1;
            } else if trim_punct(n) == "seq." || (n == "seq" && words[b].text == "et") {
                b += 1;
                break;
            } else {
                break;
            }
        }
        if words[b].text == "et" && b + 1 < words.len() {
            b += 1;
        }
        if b + 1 < words.len() && words[b + 1].text.starts_with('(') {
            b = tail(words, b);
        }
        return Some((a, b));
    }
    // Id. at 12
    if matches!(w, "Id." | "Ibid.") {
        let mut b = i;
        if i + 2 < words.len() && words[i + 1].text == "at" && is_numberish(&words[i + 2].text) {
            b = i + 2;
        }
        return Some((i, b));
    }
    // et seq.
    if w == "et" && i + 1 < words.len() && trim_punct(&words[i + 1].text) == "seq." {
        return Some((i, i + 1));
    }
    // 123 U.S. 456 (1990)
    if is_reporter(w) && i > 0 && is_digits(&words[i - 1].text) && !words[i - 1].text.ends_with(',') && !words.get(i + 1).is_some_and(|n| n.text.starts_with(['\u{a7}', '\u{b6}'])) {
        let mut b = i;
        if i + 1 < words.len() && is_numberish(&words[i + 1].text) {
            b = i + 1;
        }
        b = tail(words, b);
        return Some((i - 1, b));
    }
    None
}

/// The next stretch of `text` at or after the UTF-16 offset `from` that looks like a citation, as UTF-16 offsets:
/// a case name with `v.`, a section sign, `Id.`, `et seq.`, or a volume with a reporter such as `U.S.` or `F.2d`,
/// with the page and the court and year after it.
pub fn find_citation(text: &str, from: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let words = words_of(&chars);
    // Character index of each UTF-16 offset.
    let mut u16_at = Vec::with_capacity(chars.len() + 1);
    let mut n = 0usize;
    for c in &chars {
        u16_at.push(n);
        n += c.len_utf16();
    }
    u16_at.push(n);
    for i in 0..words.len() {
        let Some((a, b)) = span_at(&words, i) else { continue };
        let start = words[a].s;
        if u16_at[start] < from {
            continue;
        }
        // Trailing punctuation and a closing period of the sentence stay out.
        let mut end = words[b].e;
        while end > start && matches!(chars[end - 1], ',' | ';' | ':') {
            end -= 1;
        }
        if end > start && chars[end - 1] == '.' {
            let before: String = chars[start..end - 1].iter().collect();
            let last = before.rsplit(char::is_whitespace).next().unwrap_or("");
            if last.chars().last().is_some_and(|c| c.is_ascii_digit() || c == ')') {
                end -= 1;
            }
        }
        return Some((u16_at[start], u16_at[end]));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Option<String> {
        find_citation(text, 0).map(|(s, e)| {
            let u: Vec<u16> = text.encode_utf16().collect();
            String::from_utf16(&u[s..e]).unwrap()
        })
    }

    #[test]
    fn ta_roundtrip() {
        let mut c = Citation::new(Some(plain("Smith v. Jones, 123 U.S. 456 \"1990\"")), Some("Smith".into()), 1);
        c.bold = true;
        let i = ta_instr(&c);
        assert_eq!(i, " TA \\l \"Smith v. Jones, 123 U.S. 456 \\\"1990\\\"\" \\s \"Smith\" \\c 1 \\b ");
        let back = parse_ta(&i).unwrap();
        assert_eq!(back.long_text().as_deref(), Some("Smith v. Jones, 123 U.S. 456 \"1990\""));
        assert_eq!(back.short.as_deref(), Some("Smith"));
        assert!(back.bold && !back.italic);
        assert_eq!(back.category, 1);
        let short = parse_ta(" TA \\s \"Smith\" \\c 3 ").unwrap();
        assert!(short.long.is_none() && short.category == 3);
    }

    #[test]
    fn curly_quotes_inside_a_citation_stay_in_it() {
        let c = Citation::new(Some(plain("John Doe, \u{201C}The Law of Things,\u{201D} 12 Harv. L. Rev. 1 (2000)")), Some("Doe, \u{201C}Things\u{201D}".into()), 3);
        let back = parse_ta(&ta_instr(&c)).unwrap();
        assert_eq!(back.long_text(), c.long_text());
        assert_eq!(back.short, c.short);
        assert_eq!(back.category, 3);
        // A value Word wrote with curly quotes round it still reads.
        let curly = parse_ta(" TA \\l \u{201C}Roe v. Wade\u{201D} \\c 1 ").unwrap();
        assert_eq!(curly.long_text().as_deref(), Some("Roe v. Wade"));
    }

    #[test]
    fn a_later_mark_without_a_category_is_filed_with_its_long_citation() {
        // Word: the first mention has \l, \s and \c; later ones only \s.
        let mut first = parse_ta(" TA \\l \"42 U.S.C. \u{a7} 1983\" \\s \"Section 1983\" \\c 2 ").unwrap();
        let mut later = parse_ta(" TA \\s \"Section 1983\" ").unwrap();
        assert!(!later.own_category);
        let span = |n: u32| Some(PageSpan { from: n, to: n, from_label: n.to_string(), to_label: n.to_string() });
        first.pages = span(1);
        later.pages = span(4);
        let mut again = later.clone();
        again.pages = span(7);
        let cites = vec![first, later.clone(), again];
        let all = build(&cites, &Options::parse(" TOA \\h \\c \"0\" "));
        assert_eq!(all.len(), 1, "one entry, under Statutes: {all:?}");
        assert_eq!(all[0].category, 2);
        assert_eq!(all[0].pages.iter().map(|p| p.span.from).collect::<Vec<_>>(), [1, 4, 7]);
        let statutes = build(&cites, &Options::parse(" TOA \\h \\c \"2\" "));
        assert_eq!(statutes.len(), 1);
        assert_eq!(statutes[0].pages.len(), 3);
        // Written again, the later mark still has no \c.
        assert!(!ta_instr(&later).contains("\\c"), "{}", ta_instr(&later));
    }

    #[test]
    fn formatted_long_citation_is_kept_in_the_field() {
        let mut c = Citation::new(Some(vec![Piece { text: "Smith v. Jones".into(), fmt: Fmt { italic: true, ..Default::default() } }, Piece { text: ", 123 U.S. 456".into(), fmt: Fmt::default() }]), None, 2);
        c.category = 2;
        let inl = ta_inlines(&c);
        let p = crate::model::Paragraph { inlines: inl, ..crate::model::Paragraph::new(1) };
        let blocks = vec![Block::Para(p)];
        let got = collect(&blocks, None);
        assert_eq!(got.len(), 1);
        let l = got[0].long.as_ref().unwrap();
        assert_eq!(l.len(), 2);
        assert!(l[0].fmt.italic && !l[1].fmt.italic);
        assert_eq!(pieces_text(l), "Smith v. Jones, 123 U.S. 456");
        assert_eq!(got[0].category, 2);
    }

    #[test]
    fn options_roundtrip() {
        let o = ToaOptions { category: 1, passim: true, keep_formatting: false, page_sep: Some("; ".into()), seq: Some("Chapter".into()), ..Default::default() };
        let i = o.instr(None);
        assert!(i.starts_with(" TOA \\h \\c \"1\" \\e \"\t\" \\p \\f"), "{i}");
        let back = ToaOptions::from_instr(&i, Leader::Dot);
        assert_eq!((back.category, back.passim, back.keep_formatting, back.headings, back.right_align), (1, true, false, true, true));
        // Keeping the formatting (Word's default) writes no \f, and a table Word made with the box ticked reads as keep.
        assert!(!ToaOptions::default().instr(None).contains("\\f"));
        assert!(ToaOptions::from_instr(" TOA \\h \\c \"1\" \\p ", Leader::Dot).keep_formatting);
        assert_eq!(back.page_sep.as_deref(), Some("; "));
        assert_eq!(back.seq.as_deref(), Some("Chapter"));
        // Switches the dialog doesn't know stay.
        let kept = ToaOptions::default().instr(Some(" TOA \\h \\c \"0\" \\x \"y\" "));
        assert!(kept.contains("\\x \"y\""), "{kept}");
    }

    fn cite(long: Option<&str>, short: Option<&str>, cat: u8, page: u32) -> Citation {
        let mut c = Citation::new(long.map(plain), short.map(String::from), cat);
        c.pages = Some(PageSpan { from: page, to: page, from_label: page.to_string(), to_label: page.to_string() });
        c
    }

    #[test]
    fn entries_are_grouped_sorted_and_joined_by_short_citation() {
        let cites = [
            cite(Some("Zeta v. Alpha, 1 U.S. 1"), Some("Zeta"), 1, 3),
            cite(Some("42 U.S.C. \u{a7} 1983"), None, 2, 1),
            cite(None, Some("Zeta"), 1, 7),
            cite(Some("Beta v. Gamma, 2 U.S. 2"), None, 1, 2),
            cite(Some("Zeta v. Alpha, 1 U.S. 1"), None, 1, 3),
        ];
        let e = build(&cites, &Options::parse(" TOA \\h \\c \"0\" "));
        let names: Vec<String> = e.iter().map(|e| format!("{}:{}", e.category, pieces_text(&e.text))).collect();
        assert_eq!(names, ["1:Beta v. Gamma, 2 U.S. 2", "1:Zeta v. Alpha, 1 U.S. 1", "2:42 U.S.C. \u{a7} 1983"]);
        let pages: Vec<u32> = e[1].pages.iter().map(|p| p.span.from).collect();
        assert_eq!(pages, [3, 7]);
        let only = build(&cites, &Options::parse(" TOA \\c \"2\" "));
        assert_eq!(only.len(), 1);
    }

    #[test]
    fn passim_replaces_five_pages() {
        let cites: Vec<Citation> = (1..=5).map(|n| cite(Some("Roe v. Wade"), None, 1, n * 2)).collect();
        let opts = Options::parse(" TOA \\h \\c \"1\" \\p ");
        let e = build(&cites, &opts);
        let paras = paragraphs(&e, " TOA \\h \\c \"1\" \\p ", &opts, &default_name, 9000, Leader::Dot);
        let xml: String = paras.iter().map(|p| p.to_xml()).collect();
        assert!(xml.contains("passim") && !xml.contains(">2<"), "{xml}");
        let none = Options::parse(" TOA \\h \\c \"1\" ");
        let xml: String = paragraphs(&build(&cites, &none), " TOA ", &none, &default_name, 9000, Leader::Dot).iter().map(|p| p.to_xml()).collect();
        assert!(xml.contains(">2<") && !xml.contains("passim"));
    }

    #[test]
    fn finds_likely_citations() {
        assert_eq!(found("The court in Marbury v. Madison, 5 U.S. 137 (1803), held that.").as_deref(), Some("Marbury v. Madison, 5 U.S. 137 (1803)"));
        assert_eq!(found("See Roe v. Wade for more.").as_deref(), Some("Roe v. Wade"));
        assert_eq!(found("Claims arise under 42 U.S.C. \u{a7} 1983 (2012).").as_deref(), Some("42 U.S.C. \u{a7} 1983 (2012)"));
        assert_eq!(found("He agreed. Id. at 12 says so.").as_deref(), Some("Id. at 12"));
        assert_eq!(found("Rights under 5 U.S.C. \u{a7} 551 et seq. apply.").as_deref(), Some("5 U.S.C. \u{a7} 551 et seq."));
        assert_eq!(found("Reported at 410 U.S. 113, 120 (1973).").as_deref(), Some("410 U.S. 113, 120 (1973)"));
        assert_eq!(found("Nothing here to see."), None);
        // Starts after the offset.
        let text = "A v. B, 1 U.S. 1 (1900) and C v. D, 2 U.S. 2 (1901).";
        let (_, e) = find_citation(text, 0).unwrap();
        let (s2, e2) = find_citation(text, e).unwrap();
        let u: Vec<u16> = text.encode_utf16().collect();
        assert_eq!(String::from_utf16(&u[s2..e2]).unwrap(), "C v. D, 2 U.S. 2 (1901)");
    }

    #[test]
    fn category_names_are_kept_in_the_document() {
        let mut d = crate::protection::new_custom_doc();
        let mut names = HashMap::new();
        names.insert(8, "Briefs".to_string());
        names.insert(9, "Category 9".to_string());
        write_names(&mut d, &names);
        let back = read_names(Some(&d));
        assert_eq!(back.get(&8).map(String::as_str), Some("Briefs"));
        assert!(!back.contains_key(&9));
        write_names(&mut d, &HashMap::new());
        assert!(read_names(Some(&d)).is_empty());
    }
}
