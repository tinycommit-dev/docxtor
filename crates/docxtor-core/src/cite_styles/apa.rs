//! APA 7: the reference entry.

use super::*;

impl<'a> Plan<'a> {
    pub(super) fn apa_title_piece(&self, s: &Source) -> Rich {
        let mut r = Rich::new();
        let title = s.title.trim();
        if title.is_empty() {
            return r;
        }
        match s.kind {
            SourceType::Article | SourceType::Chapter | SourceType::Conference => {
                r.t(title);
            }
            SourceType::Book => {
                r.i(title);
                if let Some(e) = nz(&s.edition) {
                    r.t(&format!(" ({} ed.)", edition_base(e)));
                }
            }
            SourceType::Report => {
                r.i(title);
                if let Some(g) = nz(&s.genre) {
                    r.t(&format!(" ({g})"));
                }
            }
            SourceType::Thesis => {
                r.i(title);
                let g = thesis_label(s, Style::Apa);
                match nz(&s.publisher) {
                    Some(p) => r.t(&format!(" [{g}, {p}]")),
                    None => r.t(&format!(" [{g}]")),
                };
            }
            SourceType::Website | SourceType::Misc => {
                r.i(title);
            }
        }
        r
    }

    pub(super) fn apa_ref(&self, s: &Source) -> Rich {
        let ed_only = s.authors.is_empty() && !s.editors.is_empty();
        let names = lead_names(s);
        let dt = pub_dt(s);
        let mut date = self.year_label(s);
        if matches!(s.kind, SourceType::Website | SourceType::Misc) {
            if let Some(m) = dt.m.filter(|_| dt.y.is_some()) {
                date = match dt.d {
                    Some(d) => format!("{date}, {} {d}", month_full(m)),
                    None => format!("{date}, {}", month_full(m)),
                };
            }
        }
        let title = self.apa_title_piece(s);
        let mut r = Rich::new();
        if names.is_empty() {
            r.append(&title);
            r.stop();
            r.sp().t(&format!("({date})."));
        } else {
            r.t(&apa_authors(names));
            if ed_only {
                r.t(if names.len() > 1 { " (Eds.)" } else { " (Ed.)" });
            }
            r.stop();
            r.sp().t(&format!("({date})."));
            if !title.is_empty() {
                r.sp().append(&title);
                r.stop();
            }
        }
        let author_text = apa_authors(names);
        match s.kind {
            SourceType::Book | SourceType::Misc => {
                if let Some(p) = nz(&s.publisher) {
                    r.sp().t(p);
                    r.stop();
                }
            }
            SourceType::Article => {
                let mut src = Rich::new();
                if let Some(j) = nz(&s.container_title) {
                    src.i(j);
                }
                if let Some(v) = nz(&s.volume) {
                    if !src.is_empty() {
                        src.i(", ");
                    }
                    src.i(v);
                    if let Some(i) = nz(&s.issue) {
                        src.t(&format!("({i})"));
                    }
                } else if let Some(i) = nz(&s.issue) {
                    if !src.is_empty() {
                        src.t(", ");
                    }
                    src.t(&format!("({i})"));
                }
                if let Some(p) = nz(&s.pages) {
                    if !src.is_empty() {
                        src.t(", ");
                    }
                    src.t(&pages_with(p, "\u{2013}"));
                }
                if !src.is_empty() {
                    r.sp().append(&src);
                    r.stop();
                }
            }
            SourceType::Chapter | SourceType::Conference => {
                let mut src = Rich::new();
                src.t("In ");
                if !s.editors.is_empty() && !ed_only {
                    src.t(&apa_editors(&s.editors));
                    src.t(if s.editors.len() > 1 { " (Eds.), " } else { " (Ed.), " });
                }
                match nz(&s.container_title) {
                    Some(c) => {
                        src.i(c);
                    }
                    None => {
                        src = Rich::new();
                    }
                }
                if !src.is_empty() {
                    let mut paren: Vec<String> = Vec::new();
                    if s.kind == SourceType::Chapter {
                        if let Some(e) = nz(&s.edition) {
                            paren.push(format!("{} ed.", edition_base(e)));
                        }
                    }
                    if let Some(p) = nz(&s.pages) {
                        paren.push(p_label(p, "\u{2013}"));
                    }
                    if !paren.is_empty() {
                        src.t(&format!(" ({})", paren.join(", ")));
                    }
                    r.sp().append(&src);
                    r.stop();
                }
                if let Some(p) = nz(&s.publisher) {
                    r.sp().t(p);
                    r.stop();
                }
            }
            SourceType::Report => {
                if let Some(p) = nz(&s.publisher).filter(|p| *p != author_text) {
                    r.sp().t(p);
                    r.stop();
                }
            }
            SourceType::Website => {
                if let Some(site) = nz(&s.container_title).filter(|c| *c != author_text) {
                    r.sp().t(site);
                    r.stop();
                }
            }
            SourceType::Thesis => {}
        }
        // Where to find it: a DOI wins over a URL; an undated web page says when it was read.
        if let Some(d) = nz(&s.doi) {
            r.sp().t(&doi_link(d));
        } else if let Some(u) = nz(&s.url) {
            match acc_dt(s).and_then(|a| mdy_date(&a)).filter(|_| s.kind == SourceType::Website && nz(&s.year).is_none()) {
                Some(a) => r.sp().t(&format!("Retrieved {a}, from {u}")),
                None => r.sp().t(u),
            };
        }
        r
    }

    // ----- Harvard --------------------------------------------------------
}
