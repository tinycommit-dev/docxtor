//! The index: `XE` fields mark entries in the text (zero-width, hidden), and an `INDEX` field lists them,
//! sorted, with their page numbers. The result is written the way Word writes it: one paragraph per entry in
//! styles `Index 1`, `Index 2`, ..., `Index Heading` for letter headings, page numbers as plain runs.

use crate::fields::{self, Instr};
use crate::model::NodeId;
use crate::toc::Leader;
use serde::{Deserialize, Serialize};
use crate::xml::Element;

/// One `XE` field in the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// The paragraph the field is in (the page of its entry comes from there).
    pub para: NodeId,
    /// The entry and its sub-entries (`Fruit:Apple` is `["Fruit", "Apple"]`).
    pub levels: Vec<String>,
    /// `\t "See Fruit"`: text shown instead of a page number.
    pub see: Option<String>,
    pub bold: bool,
    pub italic: bool,
    /// `\f "x"`: the entry type, for an index that lists only one type.
    pub kind: Option<String>,
}

/// Read an `XE` instruction: ` XE "Fruit:Apple" \b \i \t "See also Orchard" `.
pub fn parse_xe(instr: &str) -> Option<Mark> {
    let i = Instr::parse(instr);
    if i.code != "XE" {
        return None;
    }
    let raw = i.first_arg()?;
    let mut levels = Vec::new();
    let mut cur = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(n @ ('"' | '\\' | ':')) => cur.push(n),
                Some(n) => {
                    cur.push('\\');
                    cur.push(n);
                }
                None => cur.push('\\'),
            },
            ':' => {
                levels.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    levels.push(cur);
    let levels: Vec<String> = levels.into_iter().map(|l| l.trim().to_string()).collect();
    if levels.first().is_none_or(|l| l.is_empty()) {
        return None;
    }
    Some(Mark { para: 0, levels, see: i.value("t").filter(|s| !s.trim().is_empty()), bold: i.has("b"), italic: i.has("i"), kind: i.value("f") })
}

/// The instruction of an `XE` field.
pub fn xe_instr(entry: &str, sub: Option<&str>, see: Option<&str>, bold: bool, italic: bool) -> String {
    let mut path = vec![entry];
    path.extend(sub.map(str::trim).filter(|s| !s.is_empty()));
    xe_instr_path(&path, see, bold, italic)
}

/// The instruction of an `XE` field for an entry and any number of sub-entries.
pub fn xe_instr_path(path: &[&str], see: Option<&str>, bold: bool, italic: bool) -> String {
    let text = path.iter().map(|p| fields::escape(p.trim(), true)).collect::<Vec<_>>().join(":");
    let mut out = format!(" XE \"{text}\"");
    if bold {
        out.push_str(" \\b");
    }
    if italic {
        out.push_str(" \\i");
    }
    if let Some(s) = see.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str(&format!(" \\t \"{}\"", fields::escape(s, false)));
    }
    out.push(' ');
    out
}

/// Split an AutoMark entry such as `Fruit:Apple` into its levels (a backslash keeps a colon in the text).
pub fn split_entry(entry: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = entry.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&':') => out.last_mut().unwrap().push(chars.next().unwrap()),
            ':' => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// How an `INDEX` field asks for its result.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Text between an entry and its page numbers (`\e`).
    pub sep: String,
    /// Sub-entries run on in the main entry's paragraph (`\r`).
    pub run_in: bool,
    /// Letter headings (`\h "A"`).
    pub headings: bool,
    /// Text columns (`\c`).
    pub columns: u8,
    /// Only entries of this type (`\f`).
    pub kind: Option<String>,
    /// Between the page numbers of one entry (`\l`, default a comma and a space).
    pub page_sep: String,
    /// Between the first and last page of a range (`\g`, default an en dash).
    pub range_sep: String,
    /// The language, as a Windows locale id (`\z`), which decides how entries sort.
    pub lcid: Option<u32>,
}

impl Options {
    pub fn parse(instr: &str) -> Options {
        let i = Instr::parse(instr);
        let sep = i.value("e").map(|s| if s == "\\t" { "\t".to_string() } else { s });
        Options {
            sep: sep.unwrap_or_else(|| ", ".into()),
            run_in: i.has("r"),
            headings: i.has("h"),
            columns: i.value("c").and_then(|v| v.trim().parse().ok()).filter(|n: &u8| (1..=4).contains(n)).unwrap_or(1),
            kind: i.value("f"),
            page_sep: i.value("l").unwrap_or_else(|| ", ".into()),
            range_sep: i.value("g").unwrap_or_else(|| "\u{2013}".into()),
            lcid: i.value("z").and_then(|v| v.trim().parse().ok()),
        }
    }
}

/// The instruction Docxtor writes for a new index (Word's own shape: a tab before the page numbers).
pub fn index_instr(columns: u8, run_in: bool, headings: bool, lcid: u32) -> String {
    let sep = if run_in { ", " } else { "\t" };
    let mut s = format!(" INDEX \\e \"{sep}\"");
    if headings {
        s.push_str(" \\h \"A\"");
    }
    if run_in {
        s.push_str(" \\r");
    }
    s.push_str(&format!(" \\c \"{}\" \\z \"{lcid}\" ", columns.clamp(1, 4)));
    s
}

fn two() -> u8 {
    2
}

fn yes() -> bool {
    true
}

/// What the "Index" dialog sets, as data. `instr` turns it into the field instruction; `Document` writes the
/// leader and the look into the tab stops and the `Index` styles, as Word's dialog does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexOptions {
    /// Text columns, 1 to 4 (the index goes in a section of its own when it has more than one).
    #[serde(default = "two")]
    pub columns: u8,
    /// Sub-entries run on in the entry's paragraph, instead of being indented below it.
    #[serde(default)]
    pub run_in: bool,
    /// A letter heading before each letter.
    #[serde(default)]
    pub headings: bool,
    /// Page numbers sit at the right edge of the column (indented type only).
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
    /// The language as a Windows locale id (`\z`): 1033 English, 3082 Spanish... None is the document's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<u32>,
    /// List only entries of this type (`\f`, set with Mark index entry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// One of the looks (`classic`, `modern`, `simple`); None leaves the styles as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look: Option<String>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        IndexOptions { columns: 2, run_in: false, headings: false, right_align: true, leader: Leader::Dot, page_sep: None, range_sep: None, language: None, kind: None, look: None }
    }
}

impl IndexOptions {
    /// The options an instruction stands for. The leader and the look aren't in it (Word keeps them in the
    /// styles); `leader` is the one the existing result uses.
    pub fn from_instr(instr: &str, leader: Leader) -> IndexOptions {
        let o = Options::parse(instr);
        let i = Instr::parse(instr);
        IndexOptions {
            columns: o.columns,
            run_in: o.run_in,
            headings: o.headings,
            right_align: o.sep.contains('\t'),
            leader,
            page_sep: i.value("l"),
            range_sep: i.value("g"),
            language: o.lcid,
            kind: o.kind,
            look: None,
        }
    }

    /// The `INDEX` instruction for these options. Switches the dialog doesn't set (`\b`, `\k`, `\p`...) are carried
    /// over from `old`, the instruction being replaced. `lcid` is the document's language, for when none is chosen.
    pub fn instr(&self, lcid: u32, old: Option<&str>) -> String {
        let columns = self.columns.clamp(1, 4);
        let sep = if self.run_in || !self.right_align { ", " } else { "\t" };
        let mut out = format!(" INDEX \\e \"{sep}\"");
        if self.headings {
            out.push_str(" \\h \"A\"");
        }
        if self.run_in {
            out.push_str(" \\r");
        }
        if let Some(l) = self.page_sep.as_deref().filter(|l| !l.is_empty() && *l != ", ") {
            out.push_str(&format!(" \\l \"{}\"", fields::escape(l, false)));
        }
        if let Some(g) = self.range_sep.as_deref().filter(|g| !g.is_empty() && *g != "\u{2013}") {
            out.push_str(&format!(" \\g \"{}\"", fields::escape(g, false)));
        }
        if let Some(f) = self.kind.as_deref().filter(|f| !f.trim().is_empty()) {
            out.push_str(&format!(" \\f \"{}\"", fields::escape(f.trim(), false)));
        }
        out.push_str(&format!(" \\c \"{columns}\" \\z \"{}\"", self.language.unwrap_or(lcid)));
        if let Some(old) = old {
            keep_other_switches(&mut out, old, &["e", "h", "r", "l", "g", "f", "c", "z"]);
        }
        out.push(' ');
        out
    }
}

/// Add the switches of the instruction `old` that a dialog doesn't set (`handled` names the ones it does, without
/// the backslash), so a change made in the dialog keeps what Word or the file had there.
pub(crate) fn keep_other_switches(out: &mut String, old: &str, handled: &[&str]) {
    let parsed = Instr::parse(old);
    let mut k = 0;
    while k < parsed.toks.len() {
        let t = &parsed.toks[k];
        if t.is_switch() {
            let name = t.text[1..].to_lowercase();
            let takes_value = parsed.toks.get(k + 1).is_some_and(|n| !n.is_switch());
            if !handled.contains(&name.as_str()) {
                out.push(' ');
                out.push_str(&t.text);
                if takes_value {
                    let v = &parsed.toks[k + 1];
                    out.push(' ');
                    out.push_str(&if v.quoted { format!("\"{}\"", v.text) } else { v.text.clone() });
                }
            }
            k += if takes_value { 2 } else { 1 };
        } else {
            k += 1;
        }
    }
}

/// Whether an instruction is an `INDEX` field.
pub fn is_index(instr: &str) -> bool {
    instr.trim_start().to_uppercase().starts_with("INDEX")
}

/// A page number of an entry, with the formatting its mark asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct PageNo {
    pub page: u32,
    pub bold: bool,
    pub italic: bool,
}

/// What the index lists, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Heading(String),
    Entry { level: usize, text: String, pages: Vec<PageNo>, see: Option<String> },
}

struct Node {
    text: String,
    key: String,
    pages: Vec<PageNo>,
    see: Vec<String>,
    kids: Vec<Node>,
}

/// Lower case with accents removed, for sorting.
pub fn fold(s: &str) -> String {
    s.chars().flat_map(|c| c.to_lowercase()).map(fold_char).collect()
}

fn fold_char(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        c => c,
    }
}

/// Letters some languages sort as letters of their own, after the one they look like: Spanish ñ after n,
/// Swedish and Finnish å, ä, ö after z, Danish and Norwegian æ, ø, å after z.
fn special_letters(lcid: Option<u32>) -> &'static [(char, &'static str)] {
    match lcid.map(|l| l & 0x3ff) {
        Some(0x0a) => &[('\u{f1}', "n{")],
        Some(0x1d) | Some(0x0b) => &[('\u{e5}', "z{"), ('\u{e4}', "z|"), ('\u{f6}', "z}")],
        Some(0x06) | Some(0x14) => &[('\u{e6}', "z{"), ('\u{f8}', "z|"), ('\u{e5}', "z}")],
        _ => &[],
    }
}

/// Like [`fold`], keeping the letters the language sorts on their own.
fn fold_lang(s: &str, lcid: Option<u32>) -> String {
    let special = special_letters(lcid);
    if special.is_empty() {
        return fold(s);
    }
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match special.iter().find(|(l, _)| *l == c) {
            Some((_, key)) => key.to_string(),
            None => fold_char(c).to_string(),
        })
        .collect()
}

/// Symbols sort before numbers, numbers before letters, then by folded text.
pub(crate) fn sort_key(s: &str, lcid: Option<u32>) -> (u8, String) {
    let f = fold_lang(s.trim(), lcid);
    let class = match f.chars().next() {
        Some(c) if c.is_alphabetic() => 2,
        Some(c) if c.is_numeric() => 1,
        _ => 0,
    };
    (class, f)
}

fn add_page(pages: &mut Vec<PageNo>, p: PageNo) {
    match pages.iter_mut().find(|x| x.page == p.page) {
        Some(x) => {
            x.bold |= p.bold;
            x.italic |= p.italic;
        }
        None => pages.push(p),
    }
}

/// Sort the marks into the index: entries in order, sub-entries under their entries, page numbers merged
/// (`pages` gives the page of a paragraph), with a heading before each new letter when `headings`.
pub fn build(marks: &[Mark], page_of: &dyn Fn(NodeId) -> Option<u32>, opts: &Options) -> Vec<Item> {
    let mut roots: Vec<Node> = Vec::new();
    for m in marks.iter().filter(|m| opts.kind.as_ref().is_none_or(|k| m.kind.as_ref() == Some(k))) {
        if m.levels.is_empty() {
            continue;
        }
        let leaf = node_at(&mut roots, &m.levels, opts.lcid);
        match (&m.see, page_of(m.para)) {
            (Some(see), _) => {
                if !leaf.see.contains(see) {
                    leaf.see.push(see.clone());
                }
            }
            (None, Some(page)) => add_page(&mut leaf.pages, PageNo { page, bold: m.bold, italic: m.italic }),
            (None, None) => {}
        }
    }
    let mut items = Vec::new();
    let mut last_heading = String::new();
    sort(&mut roots, opts.lcid);
    for n in &roots {
        if opts.headings {
            let h = heading_of(&n.text, opts.lcid);
            if h != last_heading {
                items.push(Item::Heading(h.clone()));
                last_heading = h;
            }
        }
        flatten(n, 0, &mut items);
    }
    items
}

/// The node for an entry path (`["Fruit", "Apple"]`), made if it isn't there. Entries that differ only in
/// case or accents are the same entry.
fn node_at<'a>(level: &'a mut Vec<Node>, path: &[String], lcid: Option<u32>) -> &'a mut Node {
    let key = fold_lang(path[0].trim(), lcid);
    let idx = match level.iter().position(|n| n.key == key) {
        Some(i) => i,
        None => {
            level.push(Node { text: path[0].trim().to_string(), key, pages: vec![], see: vec![], kids: vec![] });
            level.len() - 1
        }
    };
    if path.len() == 1 {
        &mut level[idx]
    } else {
        node_at(&mut level[idx].kids, &path[1..], lcid)
    }
}

fn sort(nodes: &mut [Node], lcid: Option<u32>) {
    nodes.sort_by_key(|n| sort_key(&n.text, lcid));
    for n in nodes.iter_mut() {
        n.pages.sort_by_key(|p| p.page);
        sort(&mut n.kids, lcid);
    }
}

fn flatten(n: &Node, level: usize, out: &mut Vec<Item>) {
    out.push(Item::Entry { level, text: n.text.clone(), pages: n.pages.clone(), see: if n.see.is_empty() { None } else { Some(n.see.join("; ")) } });
    for k in &n.kids {
        flatten(k, level + 1, out);
    }
}

fn heading_of(text: &str, lcid: Option<u32>) -> String {
    // A letter the language sorts on its own heads its own group.
    if let Some(c) = text.trim().chars().next().and_then(|c| c.to_lowercase().next()) {
        if special_letters(lcid).iter().any(|(l, _)| *l == c) {
            return c.to_uppercase().to_string();
        }
    }
    match sort_key(text, lcid) {
        (2, f) => f.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        (1, _) => "Numbers".into(),
        _ => "Symbols".into(),
    }
}

/// Page numbers as the index prints them: consecutive pages with the same formatting run together as a
/// range (`4–6`), the rest are listed. Each piece is text plus its bold and italic.
pub fn page_runs(pages: &[PageNo], range_sep: &str) -> Vec<(String, bool, bool)> {
    let mut out: Vec<(String, bool, bool)> = Vec::new();
    let mut i = 0;
    while i < pages.len() {
        let mut j = i;
        while j + 1 < pages.len() && pages[j + 1].page == pages[j].page + 1 && pages[j + 1].bold == pages[i].bold && pages[j + 1].italic == pages[i].italic {
            j += 1;
        }
        let text = if j > i { format!("{}{range_sep}{}", pages[i].page, pages[j].page) } else { pages[i].page.to_string() };
        out.push((text, pages[i].bold, pages[i].italic));
        i = j + 1;
    }
    out
}

pub(crate) fn text_run(text: &str, bold: bool, italic: bool) -> Element {
    let mut rpr = None;
    if bold || italic {
        let mut p = Element::new("w:rPr");
        if bold {
            p = p.with_child(Element::new("w:b"));
        }
        if italic {
            p = p.with_child(Element::new("w:i"));
        }
        rpr = Some(p);
    }
    crate::model::text_run_element(rpr, text)
}

/// A separator as runs: a tab is a tab element.
pub(crate) fn sep_runs(sep: &str) -> Vec<Element> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for c in sep.chars() {
        if c == '\t' {
            if !buf.is_empty() {
                out.push(text_run(&std::mem::take(&mut buf), false, false));
            }
            out.push(Element::new("w:r").with_child(Element::new("w:tab")));
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        out.push(text_run(&buf, false, false));
    }
    out
}

/// "See also Fruit" with the "See" or "See also" in italics.
fn see_runs(see: &str) -> Vec<Element> {
    let lower = see.to_lowercase();
    for lead in ["see also", "see"] {
        if lower.starts_with(lead) && see.is_char_boundary(lead.len()) {
            let (a, b) = see.split_at(lead.len());
            let mut v = vec![text_run(a, false, true)];
            if !b.is_empty() {
                v.push(text_run(b, false, false));
            }
            return v;
        }
    }
    vec![text_run(see, false, true)]
}

fn entry_runs(text: &str, pages: &[PageNo], see: &Option<String>, opts: &Options, with_sep: bool) -> Vec<Element> {
    let sep = opts.sep.as_str();
    let mut out = vec![text_run(text, false, false)];
    let runs = page_runs(pages, &opts.range_sep);
    if runs.is_empty() && see.is_none() {
        return out;
    }
    if with_sep {
        out.extend(sep_runs(sep));
    } else {
        out.push(text_run(", ", false, false));
    }
    for (i, (t, b, it)) in runs.iter().enumerate() {
        if i > 0 {
            out.push(text_run(&opts.page_sep, false, false));
        }
        out.push(text_run(t, *b, *it));
    }
    if let Some(s) = see {
        if !runs.is_empty() {
            out.push(text_run(", ", false, false));
        }
        out.extend(see_runs(s));
    }
    out
}

pub(crate) fn fld(kind: &str) -> Element {
    Element::new("w:r").with_child(Element::new("w:fldChar").with_attr("w:fldCharType", kind))
}

/// The paragraphs of an `INDEX` field's result (the field's begin is in the first, its end in the last).
/// `tab_pos` is where page numbers align (the column's right edge, in twips).
pub fn paragraphs(items: &[Item], instr: &str, opts: &Options, tab_pos: i64, leader: Leader) -> Vec<Element> {
    let mut paras: Vec<Element> = Vec::new();
    let right_tab = opts.sep.contains('\t') && !opts.run_in;
    let mut p = |style: String, runs: Vec<Element>| {
        let mut ppr = Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", style));
        if right_tab {
            ppr = ppr.with_child(Element::new("w:tabs").with_child(Element::new("w:tab").with_attr("w:val", "right").with_attr("w:leader", leader.val()).with_attr("w:pos", tab_pos.to_string())));
        }
        let mut el = Element::new("w:p").with_child(ppr);
        for r in runs {
            el = el.with_child(r);
        }
        paras.push(el);
    };
    if opts.run_in {
        let mut i = 0;
        while i < items.len() {
            match &items[i] {
                Item::Heading(h) => {
                    p("IndexHeading".into(), vec![text_run(h, false, false)]);
                    i += 1;
                }
                Item::Entry { level, text, pages, see } => {
                    let mut runs = entry_runs(text, pages, see, opts, false);
                    let mut j = i + 1;
                    let mut first = true;
                    while j < items.len() {
                        let Item::Entry { level: l2, text: t2, pages: p2, see: s2 } = &items[j] else { break };
                        if *l2 <= *level {
                            break;
                        }
                        runs.push(text_run(if first { ": " } else { "; " }, false, false));
                        first = false;
                        runs.extend(entry_runs(t2, p2, s2, opts, false));
                        j += 1;
                    }
                    p("Index1".into(), runs);
                    i = j;
                }
            }
        }
    } else {
        for it in items {
            match it {
                Item::Heading(h) => p("IndexHeading".into(), vec![text_run(h, false, false)]),
                Item::Entry { level, text, pages, see } => p(format!("Index{}", (level + 1).min(9)), entry_runs(text, pages, see, opts, true)),
            }
        }
    }
    if paras.is_empty() {
        let el = Element::new("w:p")
            .with_child(Element::new("w:pPr").with_child(Element::new("w:pStyle").with_attr("w:val", "Index1")))
            .with_child(text_run("No index entries found.", false, false));
        paras.push(el);
    }
    wrap_field(&mut paras, instr);
    paras
}

/// Put a field around a result of several paragraphs: its begin, instruction and separator at the start of the
/// first, its end at the end of the last (how Word writes the index and the tables).
pub(crate) fn wrap_field(paras: &mut [Element], instr: &str) {
    let Some(first) = paras.first_mut() else { return };
    let at = first.children.iter().position(|n| matches!(n, crate::xml::Node::Element(e) if e.name != "w:pPr")).unwrap_or(first.children.len());
    let begin = [
        fld("begin"),
        Element::new("w:r").with_child(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(instr)),
        fld("separate"),
    ];
    for (k, r) in begin.into_iter().enumerate() {
        first.children.insert(at + k, crate::xml::Node::Element(r));
    }
    if let Some(last) = paras.last_mut() {
        last.children.push(crate::xml::Node::Element(fld("end")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xe_roundtrip() {
        let i = xe_instr("Apple: red", Some("Gala"), Some("See also Fruit"), true, true);
        let m = parse_xe(&i).unwrap();
        assert_eq!(m.levels, ["Apple: red", "Gala"]);
        assert_eq!(m.see.as_deref(), Some("See also Fruit"));
        assert!(m.bold && m.italic);
    }

    #[test]
    fn ranges() {
        let p = |n| PageNo { page: n, bold: false, italic: false };
        let runs = page_runs(&[p(2), p(4), p(5), p(6), p(9)], "\u{2013}");
        assert_eq!(runs.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["2", "4\u{2013}6", "9"]);
    }

    #[test]
    fn sorts_and_groups() {
        let mk = |levels: &[&str], para| Mark { para, levels: levels.iter().map(|s| s.to_string()).collect(), see: None, bold: false, italic: false, kind: None };
        let marks = [mk(&["banana"], 1), mk(&["Apple", "red"], 2), mk(&["apple"], 3), mk(&["\u{c9}clair"], 4)];
        let opts = Options::parse(" INDEX \\h \"A\" ");
        let items = build(&marks, &|p| Some(p), &opts);
        let names: Vec<String> = items
            .iter()
            .map(|i| match i {
                Item::Heading(h) => format!("#{h}"),
                Item::Entry { level, text, .. } => format!("{}{text}", "-".repeat(*level)),
            })
            .collect();
        assert_eq!(names, ["#A", "Apple", "-red", "#B", "banana", "#E", "\u{c9}clair"]);
    }
}
