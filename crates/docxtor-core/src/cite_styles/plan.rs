//! The plan: what depends on the whole set of cited sources, and the in-text citation and reference entry for each source.

use super::*;

/// One source in a citation, with what the citation asks of it.
#[derive(Debug, Clone, Copy)]
pub struct Item<'a> {
    pub src: &'a Source,
    pub page: Option<&'a str>,
    pub no_author: bool,
    pub no_year: bool,
    pub no_title: bool,
}

impl<'a> Item<'a> {
    pub fn new(src: &'a Source) -> Item<'a> {
        Item { src, page: None, no_author: false, no_year: false, no_title: false }
    }
}

/// Everything that depends on the whole set of cited sources: the order of the list, the 2020a/2020b
/// letters, IEEE numbers and which MLA citations need a title.
pub struct Plan<'a> {
    pub style: Style,
    pub(super) entries: Vec<&'a Source>,
    pub(super) suffix: HashMap<String, String>,
    pub(super) number: HashMap<String, usize>,
    pub(super) same_author: HashSet<String>,
}

impl<'a> Plan<'a> {
    /// `cited` holds each cited source once, in the order of its first citation.
    pub fn new(style: Style, cited: &[&'a Source]) -> Plan<'a> {
        let mut entries: Vec<&'a Source> = cited.to_vec();
        let mut number = HashMap::new();
        for (i, s) in cited.iter().enumerate() {
            number.insert(s.tag.clone(), i + 1);
        }
        let mut suffix = HashMap::new();
        if matches!(style, Style::Apa | Style::Harvard | Style::Chicago) {
            let mut groups: HashMap<String, Vec<&Source>> = HashMap::new();
            for s in cited {
                let names = lead_names(s);
                let key = format!("{}|{}", families(names).join("|").to_lowercase(), nz(&s.year).unwrap_or("n.d.").to_lowercase());
                // Sources without an author are told apart by title, so they group by title and year too.
                let key = if names.is_empty() { format!("{key}|{}", sort_fold(&s.title)) } else { key };
                groups.entry(key).or_default().push(s);
            }
            for mut g in groups.into_values() {
                if g.len() > 1 {
                    g.sort_by_key(|s| sort_fold(title_key(&s.title)));
                    for (i, s) in g.iter().enumerate() {
                        suffix.insert(s.tag.clone(), ((b'a' + (i % 26) as u8) as char).to_string());
                    }
                }
            }
        }
        let mut same_author = HashSet::new();
        if style == Style::Mla {
            for s in cited {
                let fam = lead_names(s).first().map(|n| n.family().to_lowercase());
                if fam.is_some() && cited.iter().any(|o| o.tag != s.tag && lead_names(o).first().map(|n| n.family().to_lowercase()) == fam) {
                    same_author.insert(s.tag.clone());
                }
            }
        }
        if style != Style::Ieee {
            entries.sort_by(|a, b| {
                let ka = (author_sort_key(a), year_key(a), suffix.get(&a.tag).cloned().unwrap_or_default(), sort_fold(title_key(&a.title)));
                let kb = (author_sort_key(b), year_key(b), suffix.get(&b.tag).cloned().unwrap_or_default(), sort_fold(title_key(&b.title)));
                ka.cmp(&kb)
            });
        }
        Plan { style, entries, suffix, number, same_author }
    }

    /// The sources in bibliography order.
    pub fn entries(&self) -> &[&'a Source] {
        &self.entries
    }

    /// The number of a source in an IEEE list (order of first citation).
    pub fn number_of(&self, tag: &str) -> Option<usize> {
        self.number.get(tag).copied()
    }

    pub(super) fn year_label(&self, s: &Source) -> String {
        let base = nz(&s.year).unwrap_or("n.d.").to_string();
        match self.suffix.get(&s.tag) {
            Some(x) if nz(&s.year).is_some() => format!("{base}{x}"),
            Some(x) => format!("{base}-{x}"),
            None => base,
        }
    }

    // ----- in-text ---------------------------------------------------------

    /// A citation of one or more sources as it reads in the text, brackets included.
    pub fn in_text(&self, items: &[Item]) -> Rich {
        let r = match self.style {
            Style::Apa | Style::Harvard => self.author_year_cite(items),
            Style::Mla => self.mla_cite(items),
            Style::Chicago => self.chicago_cite(items),
            Style::Ieee => self.ieee_cite(items),
        };
        if r.is_empty() && !items.is_empty() {
            // Everything was suppressed: fall back to the author so the citation isn't empty.
            let plain: Vec<Item> = items.iter().map(|i| Item { no_author: false, no_year: false, no_title: false, ..*i }).collect();
            return self.in_text(&plain);
        }
        r
    }

    /// Author names for an in-text citation, or None when the source has none.
    pub(super) fn cite_names(&self, s: &Source) -> Option<String> {
        let names = lead_names(s);
        if names.is_empty() {
            return None;
        }
        let f = families(names);
        Some(match (self.style, f.len()) {
            (_, 1) => f[0].clone(),
            (Style::Apa, 2) => format!("{} & {}", f[0], f[1]),
            (Style::Apa, _) => format!("{} et al.", f[0]),
            (Style::Chicago, 2) | (Style::Mla, 2) | (Style::Harvard, 2) => format!("{} and {}", f[0], f[1]),
            (Style::Chicago, 3) => format!("{}, {}, and {}", f[0], f[1], f[2]),
            (Style::Harvard, 3) => format!("{}, {} and {}", f[0], f[1], f[2]),
            _ => format!("{} et al.", f[0]),
        })
    }

    /// The title standing in for the author: italic for books and the like, quoted for parts.
    pub(super) fn cite_title(&self, s: &Source, quote: &str) -> Rich {
        let mut r = Rich::new();
        let t = short_title(&s.title);
        if t.is_empty() {
            return r;
        }
        if quoted_title(s) {
            r.t(&format!("{quote}{t}{quote}"));
        } else {
            r.i(&t);
        }
        r
    }

    pub(super) fn author_year_cite(&self, items: &[Item]) -> Rich {
        let mut out = Rich::new();
        out.t("(");
        let mut first = true;
        for it in items {
            let mut part = Rich::new();
            let mut bits: Vec<Rich> = Vec::new();
            if !it.no_author {
                if let Some(n) = self.cite_names(it.src) {
                    bits.push(Rich::plain_of(&n));
                } else if !it.no_title {
                    let t = self.cite_title(it.src, "\"");
                    if !t.is_empty() {
                        bits.push(t);
                    }
                }
            }
            if !it.no_year {
                bits.push(Rich::plain_of(&self.year_label(it.src)));
            }
            if let Some(p) = it.page.map(str::trim).filter(|p| !p.is_empty()) {
                bits.push(Rich::plain_of(&p_label(p, "\u{2013}")));
            }
            for (k, b) in bits.iter().enumerate() {
                if k > 0 {
                    part.t(", ");
                }
                part.append(b);
            }
            if part.is_empty() {
                continue;
            }
            if !first {
                out.t("; ");
            }
            first = false;
            out.append(&part);
        }
        if first {
            return Rich::new();
        }
        out.t(")");
        out
    }

    /// One entry of the reference list. IEEE entries start with their number and a tab.
    pub fn reference(&self, s: &Source) -> Rich {
        let mut r = match self.style {
            Style::Apa => self.apa_ref(s),
            Style::Mla => self.mla_ref(s),
            Style::Chicago => self.chicago_ref(s),
            Style::Ieee => self.ieee_ref(s),
            Style::Harvard => self.harvard_ref(s),
        };
        r.trim_end();
        if self.style == Style::Ieee {
            r.end_quoted_title();
        }
        r
    }

    // ----- APA ------------------------------------------------------------
}
