//! IEEE: the numbered in-text citation and the reference entry.

use super::*;

impl<'a> Plan<'a> {
    pub(super) fn ieee_cite(&self, items: &[Item]) -> Rich {
        // Numbers in order, with runs of three or more written as a range: [1]–[3].
        let mut nums: Vec<(usize, Option<&str>)> = items.iter().filter_map(|it| self.number.get(&it.src.tag).map(|n| (*n, it.page.map(str::trim).filter(|p| !p.is_empty())))).collect();
        nums.sort_by_key(|(n, _)| *n);
        nums.dedup_by_key(|(n, _)| *n);
        let mut parts: Vec<String> = Vec::new();
        let mut i = 0;
        while i < nums.len() {
            let mut j = i;
            while j + 1 < nums.len() && nums[j + 1].0 == nums[j].0 + 1 && nums[j].1.is_none() && nums[j + 1].1.is_none() {
                j += 1;
            }
            if j >= i + 2 {
                parts.push(format!("[{}]\u{2013}[{}]", nums[i].0, nums[j].0));
                i = j + 1;
            } else {
                match nums[i].1 {
                    Some(p) => parts.push(format!("[{}, {}]", nums[i].0, p_label(p, "\u{2013}"))),
                    None => parts.push(format!("[{}]", nums[i].0)),
                }
                i += 1;
            }
        }
        Rich::plain_of(&parts.join(", "))
    }

    // ----- reference list ---------------------------------------------------

    pub(super) fn ieee_ref(&self, s: &Source) -> Rich {
        let names = lead_names(s);
        let ed_only = s.authors.is_empty() && !s.editors.is_empty();
        let title = s.title.trim();
        let dt = pub_dt(s);
        let mut r = Rich::new();
        if let Some(n) = self.number.get(&s.tag) {
            r.t(&format!("[{n}]\t"));
        }
        let quoted = title_is_part(s.kind) || matches!(s.kind, SourceType::Thesis | SourceType::Report | SourceType::Website);
        let place_pub = |s: &Source| -> String {
            match (nz(&s.place), nz(&s.publisher)) {
                (Some(pl), Some(pu)) => format!("{pl}: {pu}"),
                (None, Some(pu)) => pu.to_string(),
                (Some(pl), None) => pl.to_string(),
                _ => String::new(),
            }
        };
        let had_lead = !names.is_empty();
        if had_lead {
            r.t(&ieee_authors(names));
            if ed_only {
                r.t(if names.len() > 1 { ", Eds." } else { ", Ed." });
            }
        }
        // `Rich` pieces after the authors are joined with commas; the last one ends the entry.
        let comma_title = |r: &mut Rich| {
            if title.is_empty() {
                return;
            }
            if had_lead {
                r.t(", ");
            }
            if quoted {
                r.t(&format!("\"{title},\""));
            } else {
                r.i(title);
            }
        };
        match s.kind {
            SourceType::Book | SourceType::Misc => {
                comma_title(&mut r);
                if let Some(e) = nz(&s.edition) {
                    r.t(&format!(", {} ed", edition_base(e)));
                }
                let pp = place_pub(s);
                let mut tail = pp.clone();
                if let Some(y) = ieee_date(&dt, false) {
                    if !tail.is_empty() {
                        tail.push_str(", ");
                    }
                    tail.push_str(&y);
                }
                if pp.is_empty() && !tail.is_empty() {
                    // "Title, 2020." when there is no publisher to put in between.
                    r.t(", ").t(&tail);
                } else if !tail.is_empty() {
                    r.stop();
                    r.sp().t(&tail);
                }
                r.stop();
            }
            SourceType::Article => {
                comma_title(&mut r);
                let mut bits: Vec<String> = Vec::new();
                let mut src = Rich::new();
                if let Some(j) = nz(&s.container_title) {
                    src.i(j);
                }
                if let Some(v) = nz(&s.volume) {
                    bits.push(format!("vol. {v}"));
                }
                if let Some(i) = nz(&s.issue) {
                    bits.push(format!("no. {i}"));
                }
                if let Some(p) = nz(&s.pages) {
                    bits.push(p_label(p, "\u{2013}"));
                }
                if let Some(d) = ieee_date(&dt, false) {
                    bits.push(d);
                }
                if let Some(d) = nz(&s.doi) {
                    bits.push(format!("doi: {}", doi_of(d)));
                }
                let mut tail = Rich::new();
                tail.append(&src);
                for b in &bits {
                    if !tail.is_empty() {
                        tail.t(", ");
                    }
                    tail.t(b);
                }
                if !tail.is_empty() {
                    r.sp().append(&tail);
                    r.stop();
                }
                if nz(&s.doi).is_none() {
                    if let Some(u) = nz(&s.url) {
                        r.sp().t(&format!("[Online]. Available: {u}"));
                    }
                }
            }
            SourceType::Chapter | SourceType::Conference => {
                comma_title(&mut r);
                if let Some(c) = nz(&s.container_title) {
                    r.sp().t("in ").i(c);
                    if s.kind == SourceType::Chapter && !s.editors.is_empty() && !ed_only {
                        let eds: Vec<String> = s.editors.iter().map(init_fam).collect();
                        r.t(&format!(", {}, {}", join_and(&eds, "and", true), if eds.len() > 1 { "Eds" } else { "Ed" }));
                    }
                    if let Some(e) = nz(&s.edition) {
                        r.t(&format!(", {} ed", edition_base(e)));
                    }
                    // A paper reads "in Proc. X, City, 2018, pp. 1-10."; a chapter "in Book. City: Publisher, 2019".
                    if s.kind == SourceType::Conference || (place_pub(s).is_empty() && (ieee_date(&dt, false).is_some() || nz(&s.pages).is_some())) {
                        r.t(",");
                    } else {
                        r.stop();
                    }
                }
                let mut tail = if s.kind == SourceType::Conference { nz(&s.place).unwrap_or("").to_string() } else { place_pub(s) };
                if let Some(y) = ieee_date(&dt, false) {
                    if !tail.is_empty() {
                        tail.push_str(", ");
                    }
                    tail.push_str(&y);
                }
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
            SourceType::Report => {
                comma_title(&mut r);
                let mut bits: Vec<String> = Vec::new();
                if let Some(p) = nz(&s.publisher) {
                    bits.push(p.to_string());
                }
                if let Some(p) = nz(&s.place) {
                    bits.push(p.to_string());
                }
                if let Some(g) = nz(&s.genre) {
                    bits.push(g.to_string());
                }
                if let Some(d) = ieee_date(&dt, false) {
                    bits.push(d);
                }
                if !bits.is_empty() {
                    r.sp().t(&bits.join(", "));
                }
                r.stop();
            }
            SourceType::Website => {
                comma_title(&mut r);
                if let Some(site) = nz(&s.container_title) {
                    r.sp().t(site);
                    r.stop();
                }
                if let Some(u) = nz(&s.url) {
                    r.sp().t(&format!("[Online]. Available: {u}"));
                    if let Some(a) = acc_dt(s).and_then(|a| ieee_date(&a, true)) {
                        r.t(&format!(" (accessed {a})."));
                    }
                }
            }
            SourceType::Thesis => {
                comma_title(&mut r);
                let mut bits: Vec<String> = vec![thesis_label(s, Style::Ieee)];
                if let Some(p) = nz(&s.publisher) {
                    bits.push(p.to_string());
                }
                if let Some(p) = nz(&s.place) {
                    bits.push(p.to_string());
                }
                if let Some(y) = ieee_date(&dt, false) {
                    bits.push(y);
                }
                r.sp().t(&bits.join(", "));
                r.stop();
            }
        }
        if !matches!(s.kind, SourceType::Article | SourceType::Website) {
            if let Some(d) = nz(&s.doi) {
                r.sp().t(&format!("doi: {}.", doi_of(d)));
            } else if let Some(u) = nz(&s.url) {
                r.sp().t(&format!("[Online]. Available: {u}"));
            }
        }
        r
    }
}
