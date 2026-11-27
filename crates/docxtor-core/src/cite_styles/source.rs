//! Helpers shared by the styles that read a source: lead names, title kinds, sort keys and thesis labels.

use super::*;

/// Authors, or editors when there are none.
pub(super) fn lead_names(s: &Source) -> &[Name] {
    if s.authors.is_empty() {
        &s.editors
    } else {
        &s.authors
    }
}

/// Parts of a larger work (articles, chapters, papers, web pages) are quoted; whole works are italic.
pub(super) fn title_is_part(k: SourceType) -> bool {
    matches!(k, SourceType::Article | SourceType::Chapter | SourceType::Conference)
}

/// Whether MLA, Chicago and in-text citations put the title in quotes: parts of a larger work, web pages
/// within a site, and theses.
pub(super) fn quoted_title(s: &Source) -> bool {
    title_is_part(s.kind) || s.kind == SourceType::Thesis || (s.kind == SourceType::Website && nz(&s.container_title).is_some())
}

/// Strip a leading article so "The Rise" sorts under R.
pub(super) fn title_key(t: &str) -> &str {
    let t = t.trim();
    for a in ["the ", "a ", "an ", "The ", "A ", "An "] {
        if let Some(rest) = t.strip_prefix(a) {
            return rest;
        }
    }
    t
}

pub(super) fn author_sort_key(s: &Source) -> String {
    let names = lead_names(s);
    if names.is_empty() {
        return sort_fold(title_key(&s.title));
    }
    names.iter().map(|n| format!("{} {}", sort_fold(n.family()), sort_fold(given_full(n)))).collect::<Vec<_>>().join("|")
}

pub(super) fn year_key(s: &Source) -> u32 {
    nz(&s.year).map(|y| y.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).and_then(|y| y.parse().ok()).unwrap_or(0)
}

/// What a thesis is called in a style: a doctoral one by default, a master's one when the genre says so, any
/// other genre as it is written.
pub(super) fn thesis_label(s: &Source, style: Style) -> String {
    let genre = nz(&s.genre);
    let low = genre.map(str::to_lowercase).unwrap_or_default();
    let doctoral = genre.is_none() || low == "thesis" || ["doctor", "phd", "ph.d", "dissertation"].iter().any(|k| low.contains(k));
    let masters = !doctoral && ["master", "msc", "m.sc", "mphil", "m.s.", "ma thesis"].iter().any(|k| low.contains(k));
    match (doctoral, masters, style) {
        (false, false, _) => genre.unwrap_or_default().to_string(),
        (true, _, Style::Apa) => "Doctoral dissertation".into(),
        (true, _, Style::Mla) => "PhD dissertation".into(),
        (true, _, Style::Chicago) => "PhD diss.".into(),
        (true, _, Style::Ieee) => "Ph.D. dissertation".into(),
        (true, _, Style::Harvard) => "PhD thesis".into(),
        (_, true, Style::Apa) | (_, true, Style::Chicago) => "Master's thesis".into(),
        (_, true, Style::Mla) => "MA thesis".into(),
        (_, true, Style::Ieee) => "M.S. thesis".into(),
        (_, true, Style::Harvard) => "MSc thesis".into(),
    }
}
