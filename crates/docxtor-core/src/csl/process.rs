//! The processor: from the citations of a document, in reading order, to the text of each citation and
//! of the bibliography. It works out citation numbers, the order of the bibliography, the position of each
//! citation (first, subsequent, ibid), year suffixes and what a numeric citation collapses to.

use super::eval::{decorate, CiteCtx, Eval, Item, Mode, Position};
use super::locale::Locale;
use super::model::*;
use super::{text, vars};
use crate::cite_styles::{fold_char, Rich};
use crate::citations::Source;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// One source in a citation, with what the citation says about it.
#[derive(Debug, Clone)]
pub struct CiteItem<'a> {
    pub src: &'a Source,
    pub locator: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub suppress_author: bool,
    pub suppress_year: bool,
}

impl<'a> CiteItem<'a> {
    pub fn new(src: &'a Source) -> CiteItem<'a> {
        CiteItem { src, locator: None, prefix: None, suffix: None, suppress_author: false, suppress_year: false }
    }
}

/// A citation: one or more sources cited together. `note` is the number of the note it sits in, for styles
/// whose citations are notes.
#[derive(Debug, Clone)]
pub struct CiteSite<'a> {
    pub items: Vec<CiteItem<'a>>,
    pub note: Option<usize>,
}

/// One entry of the bibliography.
#[derive(Debug, Clone)]
pub struct Entry {
    pub tag: String,
    pub text: Rich,
}

#[derive(Debug, Clone, Default)]
pub struct Output {
    /// The text of each citation, in the order the sites were given. Empty when nothing could be formatted.
    pub citations: Vec<Rich>,
    pub bibliography: Vec<Entry>,
    /// The position each item of each citation got (for tests and for callers that want to show it).
    pub positions: Vec<Vec<Position>>,
}

impl Style {
    /// Whether citations are notes (footnotes).
    pub fn is_note(&self) -> bool {
        self.class == Class::Note
    }

    /// Whether a citation is written raised (superscript numbers), so it needs no space before it.
    pub fn cites_superscript(&self) -> bool {
        self.citation.layout.fmt.vertical == Some(Vertical::Sup)
    }

    /// The paragraph indent of a bibliography entry in twips: (left, hanging). (0, 0) is none.
    pub fn bibliography_indent(&self) -> (i64, i64) {
        match &self.bibliography {
            Some(b) if b.second_field_align.is_some() => (540, 540),
            Some(b) if b.hanging_indent => (720, 720),
            _ => (0, 0),
        }
    }

    /// The heading above the bibliography when the style doesn't name one.
    pub fn bibliography_heading(&self) -> &'static str {
        if self.is_note() {
            "Bibliography"
        } else {
            "References"
        }
    }
}

fn norm(s: &str) -> String {
    let t: String = s.chars().filter_map(fold_char).filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, PartialEq)]
enum Key {
    Text(String),
    Num(i64),
    Missing,
}

fn cmp_key(a: &Key, b: &Key, descending: bool) -> Ordering {
    let o = match (a, b) {
        (Key::Missing, Key::Missing) => return Ordering::Equal,
        // Items with nothing to sort on go last, whichever way the key runs.
        (Key::Missing, _) => return Ordering::Greater,
        (_, Key::Missing) => return Ordering::Less,
        (Key::Num(x), Key::Num(y)) => x.cmp(y),
        (Key::Text(x), Key::Text(y)) => x.cmp(y),
        (Key::Num(x), Key::Text(y)) => x.to_string().cmp(y),
        (Key::Text(x), Key::Num(y)) => x.cmp(&y.to_string()),
    };
    if descending {
        o.reverse()
    } else {
        o
    }
}

struct Run<'a> {
    style: &'a Style,
    loc: Locale<'a>,
}

impl<'a> Run<'a> {
    fn eval_macro_plain(&self, name: &str, item: &Item, mode: Mode) -> String {
        let mut ev = Eval::new(self.style, &self.loc, mode, item, CiteCtx::default());
        let nodes = [Node::Text(TextNode { fmt: Fmt::default(), src: TextSource::Macro(name.to_string()), form: Form::Long, plural: None })];
        ev.run(&nodes).plain()
    }

    fn key(&self, k: &SortKey, item: &Item, mode: Mode) -> Key {
        if let Some(m) = &k.macro_name {
            let t = norm(&self.eval_macro_plain(m, item, mode));
            return if t.is_empty() { Key::Missing } else { Key::Text(t) };
        }
        let Some(v) = k.variable.as_deref() else { return Key::Missing };
        match v {
            "citation-number" => item.citation_number.map(|n| Key::Num(n as i64)).unwrap_or(Key::Missing),
            "issued" | "accessed" => match vars::date(item.src, v).and_then(|d| d.year.map(|y| (y, d.month.unwrap_or(0), d.day.unwrap_or(0)))) {
                Some((y, m, d)) => Key::Num(y as i64 * 10_000 + m as i64 * 100 + d as i64),
                None => Key::Missing,
            },
            "author" | "editor" => {
                let t: Vec<String> = vars::names(item.src, v).iter().map(|n| format!("{} {}", n.family(), n.given().unwrap_or(""))).collect();
                let t = norm(&t.join(" "));
                if t.is_empty() {
                    Key::Missing
                } else {
                    Key::Text(t)
                }
            }
            other => match vars::text(item.src, other).map(|t| norm(&t)).filter(|t| !t.is_empty()) {
                Some(t) => Key::Text(t),
                None => Key::Missing,
            },
        }
    }

    fn sort_indices(&self, keys: &[SortKey], items: &[Item], mode: Mode) -> Vec<usize> {
        let mut order: Vec<usize> = (0..items.len()).collect();
        if keys.is_empty() {
            return order;
        }
        let table: Vec<Vec<Key>> = items.iter().map(|it| keys.iter().map(|k| self.key(k, it, mode)).collect()).collect();
        order.sort_by(|&a, &b| {
            for (i, k) in keys.iter().enumerate() {
                let o = cmp_key(&table[a][i], &table[b][i], k.descending);
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        order
    }

    /// One item in a citation, before the layout around the whole citation is applied.
    fn item_text(&self, item: &Item, ci: &CiteItem, position: Position) -> Rich {
        let cite = CiteCtx { position, locator: ci.locator.clone().filter(|l| !l.trim().is_empty()), suppress_author: ci.suppress_author, suppress_year: ci.suppress_year };
        let mut ev = Eval::new(self.style, &self.loc, Mode::Citation, item, cite);
        let body = ev.run(&self.style.citation.layout.body);
        if body.is_empty() {
            return body;
        }
        let mut out = Rich::new();
        if let Some(p) = ci.prefix.as_deref().filter(|p| !p.is_empty()) {
            out.t(p);
            if !p.ends_with(' ') {
                out.t(" ");
            }
        }
        out.append(&body);
        if let Some(s) = ci.suffix.as_deref().filter(|s| !s.is_empty()) {
            if !s.starts_with([',', ';', '.', ' ']) {
                out.t(" ");
            }
            out.t(s);
        }
        out
    }

    /// The pieces of a citation put together: collapsed numbers, the delimiter, the layout's own look.
    fn assemble(&self, parts: Vec<(Rich, Option<usize>, bool)>) -> Rich {
        let layout = &self.style.citation.layout;
        let collapse = self.style.citation.collapse.as_deref() == Some("citation-number");
        let mut segments: Vec<Rich> = Vec::new();
        let mut i = 0;
        while i < parts.len() {
            let mut j = i;
            if collapse {
                while j + 1 < parts.len() {
                    let (a, b) = (&parts[j], &parts[j + 1]);
                    let ok = matches!((a.1, b.1), (Some(x), Some(y)) if y == x + 1) && !a.2 && !b.2;
                    if !ok {
                        break;
                    }
                    j += 1;
                }
            }
            if j >= i + 2 {
                let mut r = Rich::new();
                r.append(&parts[i].0);
                r.t("\u{2013}");
                r.append(&parts[j].0);
                segments.push(r);
                i = j + 1;
            } else {
                segments.push(parts[i].0.clone());
                i += 1;
            }
        }
        let mut joined = text::join(segments, &layout.delimiter);
        // A note that starts with a term ("ibid.") starts with a capital.
        if self.style.is_note() {
            if let Some(t) = self.loc.term("ibid", Form::Long, false).filter(|t| !t.is_empty()) {
                if joined.plain().starts_with(&t) {
                    text::apply_case(&mut joined, TextCase::CapitalizeFirst);
                }
            }
        }
        joined = decorate(joined, &layout.fmt, &self.loc);
        text::finish(&mut joined);
        joined
    }
}

fn letters(n: usize) -> String {
    let mut s = String::new();
    let mut n = n;
    loop {
        s.insert(0, (b'a' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    s
}

fn ibid_position(items: &[CiteItem], i: usize, prev: &Option<(String, Option<String>, bool)>, tag: &str) -> Position {
    let Some((ptag, ploc, psingle)) = prev else { return Position::Subsequent };
    if items.len() != 1 || !psingle || ptag != tag {
        return Position::Subsequent;
    }
    let cur = items[i].locator.as_deref().filter(|l| !l.trim().is_empty());
    match (cur, ploc.as_deref()) {
        (None, None) => Position::Ibid,
        (None, Some(_)) => Position::Subsequent,
        (Some(a), Some(b)) if a == b => Position::Ibid,
        (Some(_), _) => Position::IbidWithLocator,
    }
}

/// Format the citations of a document and its bibliography. `sites` are in reading order; a source is
/// recognised by its tag.
pub fn process(style: &Style, sites: &[CiteSite]) -> Output {
    let run = Run { style, loc: Locale::new(style) };
    // The cited sources, in order of first citation.
    let mut order: Vec<&Source> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for site in sites {
        for it in &site.items {
            if !index.contains_key(it.src.tag.as_str()) {
                index.insert(it.src.tag.as_str(), order.len());
                order.push(it.src);
            }
        }
    }
    let mut items: Vec<Item> = order
        .iter()
        .enumerate()
        .map(|(i, s)| Item { src: s, citation_number: Some(i + 1), year_suffix: None, first_note: None, disambiguate: false })
        .collect();

    // The order of the bibliography, which also gives the numbers.
    let bib_sort = style.bibliography.as_ref().map(|b| b.sort.as_slice()).unwrap_or(&[]);
    let bib_order = run.sort_indices(bib_sort, &items, Mode::Bibliography);
    for (pos, &i) in bib_order.iter().enumerate() {
        items[i].citation_number = Some(pos + 1);
    }

    // Citations that read the same for different sources: a letter after the year, or more detail.
    let baseline = if style.is_note() { Position::Subsequent } else { Position::First };
    let text_of = |items: &[Item], i: usize| run.item_text(&items[i], &CiteItem::new(items[i].src), baseline).plain();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for i in 0..items.len() {
        groups.entry(text_of(&items, i)).or_default().push(i);
    }
    let mut ambiguous: Vec<Vec<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
    ambiguous.sort();
    if style.citation.add_year_suffix {
        for g in &ambiguous {
            let mut members = g.clone();
            members.sort_by_key(|&i| bib_order.iter().position(|&b| b == i));
            for (n, i) in members.into_iter().enumerate() {
                items[i].year_suffix = Some(letters(n));
            }
        }
        // Whatever still reads the same asks the style for more detail.
        let mut again: HashMap<String, Vec<usize>> = HashMap::new();
        for i in 0..items.len() {
            again.entry(text_of(&items, i)).or_default().push(i);
        }
        for g in again.into_values().filter(|g| g.len() > 1) {
            for i in g {
                items[i].disambiguate = true;
            }
        }
    } else {
        for g in &ambiguous {
            for &i in g {
                items[i].disambiguate = true;
            }
        }
    }

    // The citations, in order.
    let mut out = Output::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut first_note: HashMap<String, usize> = HashMap::new();
    let mut prev: Option<(String, Option<String>, bool)> = None;
    for site in sites {
        let mut positions = Vec::new();
        // Items are sorted inside a citation when the style says so (numeric styles by number).
        let keyed: Vec<Item> = site.items.iter().map(|it| items[index[it.src.tag.as_str()]].clone()).collect();
        let sorted = run.sort_indices(&style.citation.sort, &keyed, Mode::Citation);
        let mut parts: Vec<(Rich, Option<usize>, bool)> = Vec::new();
        for &k in &sorted {
            let ci = &site.items[k];
            let tag = ci.src.tag.as_str();
            let position = if !seen.contains(tag) { Position::First } else { ibid_position(&site.items, k, &prev, tag) };
            positions.push(position);
            if let Some(n) = site.note {
                first_note.entry(tag.to_string()).or_insert(n);
            }
            seen.insert(tag.to_string());
            let mut item = keyed[k].clone();
            item.first_note = first_note.get(tag).copied();
            let r = run.item_text(&item, ci, position);
            if !r.is_empty() {
                parts.push((r, item.citation_number, ci.locator.is_some() || ci.prefix.is_some() || ci.suffix.is_some()));
            }
        }
        prev = (site.items.len() == 1).then(|| (site.items[0].src.tag.clone(), site.items[0].locator.clone().filter(|l| !l.trim().is_empty()), true));
        out.positions.push(positions);
        out.citations.push(if parts.is_empty() { Rich::new() } else { run.assemble(parts) });
    }

    // The bibliography.
    if let Some(b) = &style.bibliography {
        let mut prev_authors: Option<(Vec<crate::citations::Name>, String)> = None;
        for &i in &bib_order {
            let item = &items[i];
            let mut ev = Eval::new(style, &run.loc, Mode::Bibliography, item, CiteCtx::default());
            let mut r = if b.second_field_align.is_some() {
                let mut parts = ev.run_each(&b.layout.body);
                if parts.is_empty() {
                    Rich::new()
                } else {
                    let head = parts.remove(0);
                    let mut r = Rich::new();
                    r.append(&head);
                    if !parts.is_empty() {
                        r.t("\t");
                        for p in &parts {
                            r.append(p);
                        }
                    }
                    r
                }
            } else {
                ev.run(&b.layout.body)
            };
            if r.is_empty() {
                continue;
            }
            r = decorate(r, &b.layout.fmt, &run.loc);
            text::finish(&mut r);
            if let Some(sub) = &b.subsequent_author_substitute {
                let authors = item.src.authors.clone();
                if !authors.is_empty() {
                    let lead = lead_text(style, &run.loc, item);
                    if let Some((pa, _)) = &prev_authors {
                        if *pa == authors && !lead.is_empty() && r.plain().starts_with(&lead) {
                            r = replace_prefix(&r, lead.chars().count(), sub);
                        }
                    }
                    prev_authors = Some((authors, lead));
                } else {
                    prev_authors = None;
                }
            }
            out.bibliography.push(Entry { tag: item.src.tag.clone(), text: r });
        }
    }
    out
}

/// The rendered author names of an item in the bibliography, found by evaluating the first `names`
/// element for `author` that the layout reaches.
fn lead_text(style: &Style, loc: &Locale, item: &Item) -> String {
    fn find<'a>(style: &'a Style, nodes: &'a [Node], seen: &mut Vec<&'a str>) -> Option<&'a NamesNode> {
        for n in nodes {
            match n {
                Node::Names(nn) if nn.variables.iter().any(|v| v == "author") => return Some(nn),
                Node::Group(g) => {
                    if let Some(f) = find(style, &g.children, seen) {
                        return Some(f);
                    }
                }
                Node::Choose(c) => {
                    for b in &c.branches {
                        if let Some(f) = find(style, &b.body, seen) {
                            return Some(f);
                        }
                    }
                }
                Node::Text(TextNode { src: TextSource::Macro(m), .. }) => {
                    if !seen.contains(&m.as_str()) {
                        seen.push(m.as_str());
                        if let Some(body) = style.macros.get(m) {
                            if let Some(f) = find(style, body, seen) {
                                return Some(f);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    let Some(b) = &style.bibliography else { return String::new() };
    let Some(names) = find(style, &b.layout.body, &mut Vec::new()) else { return String::new() };
    let mut ev = Eval::new(style, loc, Mode::Bibliography, item, CiteCtx::default());
    let mut only = names.clone();
    only.substitute.clear();
    let mut r = ev.node(&Node::Names(Box::new(only)));
    text::finish(&mut r);
    r.plain()
}

/// The text with its first `n` characters replaced by `with`.
fn replace_prefix(r: &Rich, n: usize, with: &str) -> Rich {
    let mut out = Rich::plain_of(with);
    let mut skip = n;
    for s in &r.0 {
        let len = s.text.chars().count();
        if skip >= len {
            skip -= len;
            continue;
        }
        let rest: String = s.text.chars().skip(skip).collect();
        skip = 0;
        let mut piece = s.clone();
        piece.text = rest;
        out.push_span(piece);
    }
    out
}
