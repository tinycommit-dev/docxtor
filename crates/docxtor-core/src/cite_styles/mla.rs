//! MLA 9: the in-text citation and the works-cited entry.

use super::*;

impl<'a> Plan<'a> {
    pub(super) fn mla_cite(&self, items: &[Item]) -> Rich {
        let mut out = Rich::new();
        let mut first = true;
        out.t("(");
        for it in items {
            let mut part = Rich::new();
            let author = if it.no_author { None } else { self.cite_names(it.src) };
            // A source with no author is cited by title; one with an author only when its author has other works.
            let anonymous = lead_names(it.src).is_empty();
            let needs_title = !it.no_title && ((anonymous && !it.no_author) || (author.is_some() && self.same_author.contains(&it.src.tag)));
            if let Some(a) = &author {
                part.t(a);
            }
            if needs_title {
                let t = self.cite_title(it.src, "\"");
                if !t.is_empty() {
                    if author.is_some() {
                        part.t(", ");
                    }
                    part.append(&t);
                }
            }
            if let Some(p) = it.page.map(str::trim).filter(|p| !p.is_empty()) {
                part.sp().t(&pages_with(p, "-"));
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

    pub(super) fn mla_ref(&self, s: &Source) -> Rich {
        let names = lead_names(s);
        let ed_only = s.authors.is_empty() && !s.editors.is_empty();
        let title = s.title.trim();
        let dt = pub_dt(s);
        let mut r = Rich::new();
        if !names.is_empty() {
            r.t(&if ed_only { format!("{}, editor{}", mla_authors(names), if names.len() > 1 { "s" } else { "" }) } else { mla_authors(names) });
            r.stop();
        }
        // The title: a part in quotes, a whole work in italics.
        let quoted = quoted_title(s);
        if !title.is_empty() {
            r.sp();
            if quoted {
                let mut t = title.to_string();
                if !t.ends_with(['.', '?', '!']) {
                    t.push('.');
                }
                r.t(&format!("\"{t}\""));
            } else {
                r.i(title);
                r.stop();
            }
        }
        let mut pieces: Vec<Rich> = Vec::new();
        let mut piece = |s: String| {
            if !s.is_empty() {
                pieces.push(Rich::plain_of(&s));
            }
        };
        let mut container: Option<Rich> = None;
        match s.kind {
            SourceType::Book | SourceType::Misc | SourceType::Report => {
                if s.kind == SourceType::Report {
                    if let Some(g) = nz(&s.genre) {
                        piece(g.to_string());
                    }
                }
                if let Some(e) = nz(&s.edition) {
                    piece(format!("{} ed.", edition_base(e)));
                }
                if let Some(v) = nz(&s.volume) {
                    piece(format!("vol. {v}"));
                }
                if let Some(p) = nz(&s.publisher) {
                    piece(p.to_string());
                }
                if let Some(d) = mla_date(&dt) {
                    piece(d);
                }
            }
            SourceType::Article => {
                if let Some(c) = nz(&s.container_title) {
                    container = Some(Rich::new());
                    container.as_mut().unwrap().i(c);
                }
                if let Some(v) = nz(&s.volume) {
                    piece(format!("vol. {v}"));
                }
                if let Some(i) = nz(&s.issue) {
                    piece(format!("no. {i}"));
                }
                if let Some(d) = mla_date(&dt) {
                    piece(d);
                }
                if let Some(p) = nz(&s.pages) {
                    piece(p_label(p, "-"));
                }
            }
            SourceType::Chapter | SourceType::Conference => {
                if let Some(c) = nz(&s.container_title) {
                    container = Some(Rich::new());
                    container.as_mut().unwrap().i(c);
                }
                if !s.editors.is_empty() && !ed_only {
                    piece(format!("edited by {}", mla_editors(&s.editors)));
                }
                if let Some(e) = nz(&s.edition) {
                    piece(format!("{} ed.", edition_base(e)));
                }
                if let Some(v) = nz(&s.volume) {
                    piece(format!("vol. {v}"));
                }
                if let Some(p) = nz(&s.publisher) {
                    piece(p.to_string());
                }
                if let Some(d) = mla_date(&dt) {
                    piece(d);
                }
                if let Some(p) = nz(&s.pages) {
                    piece(p_label(p, "-"));
                }
            }
            SourceType::Website => {
                if let Some(c) = nz(&s.container_title) {
                    container = Some(Rich::new());
                    container.as_mut().unwrap().i(c);
                }
                if let Some(p) = nz(&s.publisher).filter(|p| Some(*p) != nz(&s.container_title)) {
                    piece(p.to_string());
                }
                if let Some(d) = mla_date(&dt) {
                    piece(d);
                }
            }
            SourceType::Thesis => {
                if let Some(d) = mla_date(&dt) {
                    // The year stands alone after the title.
                    r.sp().t(&format!("{d}."));
                }
                let mut tail = Vec::new();
                if let Some(p) = nz(&s.publisher) {
                    tail.push(p.to_string());
                }
                tail.push(thesis_label(s, Style::Mla));
                piece(tail.join(", "));
            }
        }
        // A web address or DOI ends the list of pieces.
        let link = if let Some(d) = nz(&s.doi) {
            Some(doi_link(d))
        } else {
            nz(&s.url).map(|u| strip_protocol(u).to_string())
        };
        if link.is_some() && s.kind != SourceType::Thesis {
            piece(link.clone().unwrap());
        }
        let has_pieces = !pieces.is_empty();
        let mut tail = Rich::new();
        if let Some(c) = &container {
            tail.append(c);
        }
        for (i, p) in pieces.iter().enumerate() {
            if i > 0 || container.is_some() {
                tail.t(", ");
            }
            tail.append(p);
        }
        if container.is_some() || has_pieces {
            r.sp().append(&tail);
            r.stop();
        }
        if s.kind == SourceType::Thesis {
            if let Some(l) = link {
                r.sp().t(&l);
                r.stop();
            }
        }
        if s.kind == SourceType::Website {
            if let Some(a) = acc_dt(s).and_then(|a| mla_date(&a)) {
                r.sp().t(&format!("Accessed {a}."));
            }
        }
        r
    }

    // ----- Chicago (author-date) -------------------------------------------
}
