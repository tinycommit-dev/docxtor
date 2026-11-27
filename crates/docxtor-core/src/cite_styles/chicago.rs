//! Chicago 17 (author-date): the in-text citation and the reference entry.

use super::*;

impl<'a> Plan<'a> {
    pub(super) fn chicago_cite(&self, items: &[Item]) -> Rich {
        let mut out = Rich::new();
        let mut first = true;
        out.t("(");
        for it in items {
            let mut part = Rich::new();
            if !it.no_author {
                if let Some(n) = self.cite_names(it.src) {
                    part.t(&n);
                } else if !it.no_title {
                    part.append(&self.cite_title(it.src, "\""));
                }
            }
            if !it.no_year {
                part.sp().t(&self.year_label(it.src));
            }
            if let Some(p) = it.page.map(str::trim).filter(|p| !p.is_empty()) {
                if !part.is_empty() {
                    part.t(", ");
                }
                part.t(&pages_with(p, "\u{2013}"));
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

    pub(super) fn chicago_ref(&self, s: &Source) -> Rich {
        let names = lead_names(s);
        let ed_only = s.authors.is_empty() && !s.editors.is_empty();
        let title = s.title.trim();
        let dt = pub_dt(s);
        let year = self.year_label(s);
        let mut r = Rich::new();
        let quoted = quoted_title(s);
        let place_pub = |s: &Source| -> String {
            match (nz(&s.place), nz(&s.publisher)) {
                (Some(pl), Some(pu)) => format!("{pl}: {pu}"),
                (None, Some(pu)) => pu.to_string(),
                (Some(pl), None) => pl.to_string(),
                _ => String::new(),
            }
        };
        let put_title = |r: &mut Rich| {
            if title.is_empty() {
                return;
            }
            if quoted {
                let mut t = title.to_string();
                if !t.ends_with(['.', '?', '!']) {
                    t.push('.');
                }
                r.sp().t(&format!("\"{t}\""));
            } else {
                r.sp().i(title);
                r.stop();
            }
        };
        if names.is_empty() {
            put_title(&mut r);
            r.sp().t(&year);
            r.stop();
        } else {
            r.t(&chicago_authors(names));
            if ed_only {
                r.t(if names.len() > 1 { ", eds" } else { ", ed" });
            }
            r.stop();
            r.sp().t(&year);
            r.stop();
            put_title(&mut r);
        }
        match s.kind {
            SourceType::Book | SourceType::Misc => {
                if let Some(v) = nz(&s.volume) {
                    r.sp().t(&format!("Vol. {v}."));
                }
                if let Some(e) = nz(&s.edition) {
                    r.sp().t(&format!("{} ed.", edition_base(e)));
                }
                let pp = place_pub(s);
                if !pp.is_empty() {
                    r.sp().t(&pp);
                    r.stop();
                }
            }
            SourceType::Report => {
                if let Some(g) = nz(&s.genre) {
                    r.sp().t(g);
                    r.stop();
                }
                let pp = place_pub(s);
                if !pp.is_empty() {
                    r.sp().t(&pp);
                    r.stop();
                }
            }
            SourceType::Article => {
                if let Some(j) = nz(&s.container_title) {
                    r.sp().i(j);
                    if let Some(v) = nz(&s.volume) {
                        r.t(&format!(" {v}"));
                    }
                    if let Some(i) = nz(&s.issue) {
                        r.t(&format!(" ({i})"));
                    }
                    if let Some(p) = nz(&s.pages) {
                        r.t(&format!(": {}", pages_with(p, "\u{2013}")));
                    }
                    r.stop();
                } else if let Some(p) = nz(&s.pages) {
                    r.sp().t(&pages_with(p, "\u{2013}"));
                    r.stop();
                }
            }
            SourceType::Chapter | SourceType::Conference => {
                if let Some(c) = nz(&s.container_title) {
                    r.sp().t("In ").i(c);
                    if s.kind == SourceType::Chapter && !s.editors.is_empty() && !ed_only {
                        r.t(&format!(", edited by {}", chicago_editors(&s.editors)));
                    }
                    if let Some(v) = nz(&s.volume) {
                        r.t(&format!(", vol. {v}"));
                    }
                    if let Some(p) = nz(&s.pages) {
                        r.t(&format!(", {}", pages_with(p, "\u{2013}")));
                    }
                    r.stop();
                } else if let Some(p) = nz(&s.pages) {
                    r.sp().t(&pages_with(p, "\u{2013}"));
                    r.stop();
                }
                let pp = place_pub(s);
                if !pp.is_empty() {
                    r.sp().t(&pp);
                    r.stop();
                }
            }
            SourceType::Website => {
                if let Some(site) = nz(&s.container_title) {
                    r.sp().t(site);
                    r.stop();
                }
                if dt.m.is_some() {
                    if let Some(d) = mdy_date(&dt) {
                        r.sp().t(&format!("{d}."));
                    }
                }
                if let Some(a) = acc_dt(s).and_then(|a| mdy_date(&a)) {
                    r.sp().t(&format!("Accessed {a}."));
                }
            }
            SourceType::Thesis => {
                let mut bits = vec![thesis_label(s, Style::Chicago)];
                if let Some(p) = nz(&s.publisher) {
                    bits.push(p.to_string());
                }
                r.sp().t(&bits.join(", "));
                r.stop();
            }
        }
        if let Some(d) = nz(&s.doi) {
            r.sp().t(&doi_link(d));
            r.stop();
        } else if let Some(u) = nz(&s.url) {
            r.sp().t(u);
            r.stop();
        }
        r
    }

    // ----- IEEE -----------------------------------------------------------
}
