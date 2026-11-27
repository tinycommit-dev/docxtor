//! Harvard (Cite Them Right): the reference entry.

use super::*;

impl<'a> Plan<'a> {
    pub(super) fn harvard_ref(&self, s: &Source) -> Rich {
        let names = lead_names(s);
        let ed_only = s.authors.is_empty() && !s.editors.is_empty();
        let year = self.year_label(s);
        let title = s.title.trim();
        let part_title = |r: &mut Rich| {
            if !title.is_empty() {
                r.t(&format!("'{title}'"));
            }
        };
        let place_pub = |s: &Source| -> String {
            match (nz(&s.place), nz(&s.publisher)) {
                (Some(pl), Some(pu)) => format!("{pl}: {pu}"),
                (None, Some(pu)) => pu.to_string(),
                (Some(pl), None) => pl.to_string(),
                _ => String::new(),
            }
        };
        let mut r = Rich::new();
        let own = |r: &mut Rich| {
            if !title.is_empty() {
                r.i(title);
            }
        };
        if names.is_empty() {
            if title_is_part(s.kind) {
                part_title(&mut r);
            } else {
                own(&mut r);
            }
            r.sp().t(&format!("({year})"));
        } else {
            r.t(&harvard_authors(names));
            if ed_only {
                r.t(if names.len() > 1 { " (eds.)" } else { " (ed.)" });
            }
            r.sp().t(&format!("({year})"));
        }
        let titled = !names.is_empty();
        match s.kind {
            SourceType::Book | SourceType::Misc | SourceType::Report => {
                if titled {
                    r.sp();
                    own(&mut r);
                }
                if s.kind == SourceType::Report {
                    r.stop();
                    if let Some(g) = nz(&s.genre) {
                        r.sp().t(g);
                    }
                }
                r.stop();
                if let Some(e) = nz(&s.edition) {
                    r.sp().t(&format!("{} edn.", edition_base(e)));
                }
                let pp = place_pub(s);
                if !pp.is_empty() {
                    r.stop();
                    r.sp().t(&pp);
                }
                r.stop();
            }
            SourceType::Article => {
                if titled {
                    r.sp();
                    part_title(&mut r);
                }
                if let Some(j) = nz(&s.container_title) {
                    if titled || !title.is_empty() {
                        r.t(",");
                    }
                    r.sp().i(j);
                }
                if let Some(v) = nz(&s.volume) {
                    r.t(", ").t(v);
                    if let Some(i) = nz(&s.issue) {
                        r.t(&format!("({i})"));
                    }
                } else if let Some(i) = nz(&s.issue) {
                    r.t(&format!(", ({i})"));
                }
                if let Some(p) = nz(&s.pages) {
                    r.t(", ").t(&p_label(p, "\u{2013}"));
                }
                r.stop();
            }
            SourceType::Chapter | SourceType::Conference => {
                if titled {
                    r.sp();
                    part_title(&mut r);
                }
                if let Some(c) = nz(&s.container_title) {
                    r.t(", in");
                    if !s.editors.is_empty() && s.kind == SourceType::Chapter {
                        r.sp().t(&harvard_editors(&s.editors));
                        r.t(if s.editors.len() > 1 { " (eds.)" } else { " (ed.)" });
                    }
                    r.sp().i(c);
                    if let Some(e) = nz(&s.edition) {
                        r.t(&format!(". {} edn", edition_base(e)));
                    }
                }
                r.stop();
                let pp = place_pub(s);
                let mut tail = pp.clone();
                if let Some(p) = nz(&s.pages) {
                    if !tail.is_empty() {
                        tail.push_str(", ");
                    }
                    tail.push_str(&p_label(p, "\u{2013}"));
                }
                if !tail.is_empty() {
                    r.sp().t(&tail);
                    r.stop();
                }
            }
            SourceType::Website => {
                if titled {
                    r.sp();
                    own(&mut r);
                }
                r.stop();
                if let Some(site) = nz(&s.container_title) {
                    r.sp().t(site);
                    r.stop();
                }
            }
            SourceType::Thesis => {
                if titled {
                    r.sp();
                    own(&mut r);
                }
                r.stop();
                r.sp().t(&thesis_label(s, Style::Harvard));
                r.stop();
                if let Some(p) = nz(&s.publisher) {
                    r.sp().t(p);
                    r.stop();
                }
            }
        }
        // Links: "doi:" for a DOI, "Available at:" for a URL, with the date it was read.
        if let Some(d) = nz(&s.doi) {
            r.sp().t(&format!("doi: {}", doi_of(d)));
            r.stop();
        } else if let Some(u) = nz(&s.url) {
            r.sp().t(&format!("Available at: {u}"));
            if let Some(a) = acc_dt(s).and_then(|a| dmy_date(&a)) {
                r.t(&format!(" (Accessed: {a})"));
            }
            r.stop();
        }
        r
    }

    // ----- MLA ------------------------------------------------------------
}
