//! The table of authorities in a document: marking citations (`TA` fields), building the table (`TOA`), its
//! styles and category names, and finding the next likely citation. See `toa` for the field syntax.

use super::toc_ops::{collect_fields, put_style_child, set_list_instr};
use super::updating::BlockKinds;
use super::*;
use crate::fields;
use crate::toa::{self, Citation, Fmt, Piece, ToaOp, ToaOptions};
use crate::toc::Leader;
use std::collections::HashSet;

/// Paragraph styles of the lists built from fields: their text is never a citation.
fn is_list_style(style: &str) -> bool {
    style.starts_with("TOC") || style.starts_with("Index") || style == "TableofFigures" || style == "TableofAuthorities" || style == "TOAHeading"
}

fn style_of(p: &Paragraph) -> String {
    p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).unwrap_or_default().to_string()
}

/// The text of `inlines` as pieces with the formatting of the runs they came from.
fn pieces_of(inlines: &[Inline], out: &mut Vec<Piece>) -> bool {
    for i in inlines {
        match i {
            Inline::Run(r) => {
                let fmt = Fmt::of_rpr(r.rpr.as_ref());
                for item in &r.items {
                    match item {
                        RunItem::Text(t) => match out.last_mut() {
                            Some(p) if p.fmt == fmt => p.text.push_str(t),
                            _ => out.push(Piece { text: t.clone(), fmt }),
                        },
                        RunItem::Hidden(_) => {}
                        _ => return false,
                    }
                }
            }
            Inline::Container(c) => {
                if !pieces_of(&c.children, out) {
                    return false;
                }
            }
            Inline::Opaque(_) => {}
        }
    }
    true
}

/// `find_text` with whitespace folded: a run of spaces, tabs or no-break spaces in the text matches the single space the
/// citation was typed with (the dialog collapses them). Returns (start, end) in UTF-16 units.
fn find_folded(hay: &str, needle: &str, from: usize, ignore_case: bool) -> Option<(usize, usize)> {
    let tokens: Vec<Vec<char>> = needle.split_whitespace().map(|t| t.chars().collect()).collect();
    if tokens.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = {
        let mut off = 0;
        hay.chars().map(|c| { let o = off; off += c.len_utf16(); (o, c) }).collect()
    };
    let eq = |a: char, b: char| if ignore_case { a == b || a.to_lowercase().eq(b.to_lowercase()) } else { a == b };
    'start: for i in 0..chars.len() {
        if chars[i].0 < from {
            continue;
        }
        let mut at = i;
        for (k, tok) in tokens.iter().enumerate() {
            if k > 0 {
                let before = at;
                while at < chars.len() && chars[at].1.is_whitespace() {
                    at += 1;
                }
                if at == before {
                    continue 'start;
                }
            }
            for c in tok {
                if at >= chars.len() || !eq(chars[at].1, *c) {
                    continue 'start;
                }
                at += 1;
            }
        }
        let (lo, lc) = chars[at - 1];
        return Some((chars[i].0, lo + lc.len_utf16()));
    }
    None
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Document {
    // -----------------------------------------------------------------------
    // Names, options and the table's look
    // -----------------------------------------------------------------------

    fn toa_names(&self) -> HashMap<u8, String> {
        toa::read_names(self.custom.as_ref())
    }

    /// The names of the 16 categories as they head the table: Cases, Statutes, ..., then the names given to 8 to 16.
    pub fn toa_category_names(&self) -> Vec<String> {
        let names = self.toa_names();
        (1..=toa::LAST_CATEGORY).map(|n| names.get(&n).cloned().unwrap_or_else(|| toa::default_name(n))).collect()
    }

    fn find_toa(&self, at: Option<NodeId>) -> Result<super::toc_ops::ListField, EditError> {
        let mut found = Vec::new();
        collect_fields(&self.body, &toa::is_toa, &mut found);
        let pick = match at {
            Some(p) => found.into_iter().find(|f| f.paras.contains(&p)),
            None => found.into_iter().next(),
        };
        pick.ok_or_else(|| EditError::Invalid("the document has no table of authorities".into()))
    }

    /// The options of the table of authorities holding a paragraph (the first one without `at`), for the dialog.
    pub fn toa_options(&self, at: Option<NodeId>) -> Option<ToaOptions> {
        let f = self.find_toa(at).ok()?;
        let leader = f.paras.iter().find_map(|id| find_para(&self.body, *id).and_then(toc::leader_of)).unwrap_or_else(|| self.style_leader("TableofAuthorities").unwrap_or_default());
        Some(ToaOptions::from_instr(&f.instr, leader))
    }

    /// The first paragraph of the table of authorities holding a paragraph (the first table without `at`), if any.
    pub fn toa_at(&self, at: Option<NodeId>) -> Option<NodeId> {
        self.find_toa(at).ok().and_then(|f| f.paras.first().copied())
    }

    /// The leader for a table: the one asked for, else the one its old result uses, else the style's.
    fn toa_leader(&self, instr: &str, forced: Option<Leader>) -> Leader {
        forced
            .or_else(|| {
                let mut found = Vec::new();
                collect_fields(&self.body, &toa::is_toa, &mut found);
                let f = found.iter().find(|f| f.instr == instr).or(found.first())?;
                // The first paragraph that has a leader: the heading of a table has none.
                f.paras.iter().find_map(|id| find_para(&self.body, *id).and_then(toc::leader_of))
            })
            .or_else(|| self.style_leader("TableofAuthorities"))
            .unwrap_or_default()
    }

    /// Write a format into the `Table of Authorities` and `TOA Heading` styles (`None` leaves the formatting alone)
    /// and the leader into the entry style, as Word's dialog does, so a table Word updates later looks the same.
    pub(super) fn set_toa_styles(&mut self, look: Option<&str>, leader: Option<Leader>) {
        let tab_pos = self.text_width();
        let Some(doc) = self.styles.doc.as_mut() else { return };
        let mut changed = false;
        for id in ["TableofAuthorities", "TOAHeading"] {
            let Some(style) = doc.root.children.iter_mut().find_map(|c| match c {
                Node::Element(e) if e.name == "w:style" && e.attr("w:styleId") == Some(id) => Some(e),
                _ => None,
            }) else {
                continue;
            };
            let mut ppr = style.child("w:pPr").cloned().unwrap_or_else(|| Element::new("w:pPr"));
            if id == "TableofAuthorities" {
                if let Some(l) = leader {
                    ppr.remove_children("w:tabs");
                    let tabs = Element::new("w:tabs").with_child(Element::new("w:tab").with_attr("w:val", "right").with_attr("w:leader", l.val()).with_attr("w:pos", tab_pos.to_string()));
                    crate::props::put_ppr(&mut ppr, tabs);
                    changed = true;
                }
                if let Some((left, hanging)) = look.and_then(entry_indent) {
                    ppr.remove_children("w:ind");
                    if left > 0 {
                        crate::props::put_ppr(&mut ppr, Element::new("w:ind").with_attr("w:left", left.to_string()).with_attr("w:hanging", hanging.to_string()));
                    }
                    changed = true;
                }
                put_style_child(style, ppr);
            } else if let Some(h) = look.and_then(heading_look) {
                ppr.remove_children("w:spacing");
                ppr.remove_children("w:jc");
                crate::props::put_ppr(&mut ppr, Element::new("w:spacing").with_attr("w:before", h.before.to_string()).with_attr("w:after", h.after.to_string()));
                if h.center {
                    crate::props::put_ppr(&mut ppr, Element::new("w:jc").with_attr("w:val", "center"));
                }
                put_style_child(style, ppr);
                style.remove_children("w:rPr");
                let mut r = Element::new("w:rPr");
                crate::props::put_rpr(&mut r, Element::new("w:b"));
                crate::props::put_rpr(&mut r, Element::new("w:bCs"));
                if h.caps {
                    crate::props::put_rpr(&mut r, Element::new("w:caps"));
                }
                if h.small_caps {
                    crate::props::put_rpr(&mut r, Element::new("w:smallCaps"));
                }
                put_style_child(style, r);
                changed = true;
            }
        }
        if changed {
            self.styles.reload();
        }
    }

    // -----------------------------------------------------------------------
    // Building the table
    // -----------------------------------------------------------------------

    /// The `TA` fields in footnotes and endnotes (where authorities are usually cited), each standing at the paragraph of
    /// the text that holds the note's mark: the page of the mark is the page of the entry.
    fn note_citations(&self, seq: Option<&str>) -> Vec<Citation> {
        fn marks(inlines: &[Inline], out: &mut Vec<(bool, String)>) {
            for i in inlines {
                match i {
                    Inline::Run(r) => {
                        for item in &r.items {
                            if let RunItem::Object(e) = item {
                                if let (true, Some(id)) = (matches!(e.name.as_str(), "w:footnoteReference" | "w:endnoteReference"), e.attr("w:id")) {
                                    out.push((e.name == "w:footnoteReference", id.to_string()));
                                }
                            }
                        }
                    }
                    Inline::Container(c) => marks(&c.children, out),
                    Inline::Opaque(_) => {}
                }
            }
        }
        let mut refs: Vec<(NodeId, bool, String)> = Vec::new();
        walk_paragraphs(self.main_body(), &mut |p| {
            let mut m = Vec::new();
            marks(&p.inlines, &mut m);
            refs.extend(m.into_iter().map(|(f, id)| (p.id, f, id)));
        });
        let mut out = Vec::new();
        for (para, foot, id) in refs {
            let want = if foot { StoryKind::Footnote } else { StoryKind::Endnote };
            let Some(story) = self.stories.iter().find(|s| s.kind == want && s.note_id.as_deref() == Some(id.as_str())) else { continue };
            for mut c in toa::collect(&story.blocks, seq) {
                c.para = para;
                out.push(c);
            }
        }
        out
    }

    /// The citations a table with these options lists, each with the pages it is on.
    fn toa_citations(&self, pages: &HashMap<NodeId, u32>, opts: &toa::Options) -> Vec<Citation> {
        let mut cites = toa::collect(self.main_body(), opts.seq.as_deref());
        cites.extend(self.note_citations(opts.seq.as_deref()));
        let labels = self.page_labels();
        let bms = if opts.bookmark.is_some() || cites.iter().any(|c| c.range.is_some()) { self.bookmark_infos() } else { HashMap::new() };
        // Only the part of the text a bookmark covers.
        let scope: Option<HashSet<NodeId>> = opts.bookmark.as_ref().map(|name| {
            let order = paragraph_order(self.main_body());
            match bms.get(&name.to_lowercase()) {
                Some(b) => {
                    let (from, to) = (order.iter().position(|p| *p == b.p), order.iter().position(|p| *p == b.end_p));
                    match (from, to) {
                        (Some(a), Some(z)) if a <= z => order[a..=z].iter().copied().collect(),
                        _ => HashSet::new(),
                    }
                }
                None => HashSet::new(),
            }
        });
        cites.retain(|c| scope.as_ref().is_none_or(|s| s.contains(&c.para)));
        for c in cites.iter_mut() {
            let Some(&n) = pages.get(&c.para) else { continue };
            let mut span = toa::PageSpan { from: n, to: n, from_label: labels.label(c.para, n), to_label: labels.label(c.para, n) };
            if let Some(b) = c.range.as_ref().and_then(|r| bms.get(&r.to_lowercase())) {
                if let Some(&m) = pages.get(&b.end_p).filter(|m| **m > n) {
                    span.to = m;
                    span.to_label = labels.label(b.end_p, m);
                }
            }
            c.pages = Some(span);
        }
        cites
    }

    /// The paragraphs of a table of authorities result for a `TOA` instruction.
    pub(super) fn toa_paragraphs(&mut self, instr: &str, pages: &HashMap<NodeId, u32>, leader: Option<Leader>) -> Vec<Element> {
        let opts = toa::Options::parse(instr);
        let cites = self.toa_citations(pages, &opts);
        let entries = toa::build(&cites, &opts);
        self.styles.ensure_builtin("TableofAuthorities");
        if opts.headings {
            self.styles.ensure_builtin("TOAHeading");
        }
        let names = self.toa_names();
        let name_of = |c: u8| names.get(&c).cloned().unwrap_or_else(|| toa::default_name(c));
        let leader = self.toa_leader(instr, leader);
        toa::paragraphs(&entries, instr, &opts, &name_of, self.text_width(), leader)
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn op_toa(&mut self, op: &ToaOp) -> Result<OpResult, EditError> {
        match op {
            ToaOp::MarkCitation { at, from, long, short, category, bold, italic } => self.op_mark_citation(*at, *from, long, short.as_deref(), *category, *bold, *italic),
            ToaOp::MarkAll { from, to, long, short, category, bold, italic, ignore_case } => self.op_mark_all_citations(*from, *to, long, short.as_deref(), *category, *bold, *italic, *ignore_case),
            ToaOp::Insert { after, before, options, pages } => self.op_insert_toa(after.map(|i| i.0), before.map(|i| i.0), options, pages.as_ref()),
            ToaOp::Set { at, options, pages } => self.op_set_toa(at.map(|i| i.0), options, pages.as_ref()),
            ToaOp::Update { at, pages } => self.op_update_toa(at.map(|i| i.0), pages.as_ref()),
            ToaOp::SetCategoryNames { names } => self.op_set_toa_names(names),
        }
    }

    fn check_category(category: u8) -> Result<(), EditError> {
        if (1..=toa::LAST_CATEGORY).contains(&category) {
            Ok(())
        } else {
            Err(EditError::Invalid("The category is 1 to 16: Cases, Statutes, Other authorities, Rules, Treatises, Regulations, Constitutional provisions, then 8 to 16 of your own.".into()))
        }
    }

    /// The long citation with the formatting of the text it was taken from (when `from..to` is that text).
    fn long_pieces(&self, from: Option<Pos>, to: Pos, long: &str) -> Vec<Piece> {
        if let Some(f) = from.filter(|f| f.p == to.p && f.o < to.o) {
            if let Ok(p) = self.para_ref(to.p.0) {
                let mut pieces = Vec::new();
                if pieces_of(&edit::slice(&p.inlines, f.o.min(p.len16()), to.o.min(p.len16())), &mut pieces) && squash(&toa::pieces_text(&pieces)) == squash(long) {
                    // Spaces at the ends are not part of the citation.
                    if let Some(first) = pieces.first_mut() {
                        first.text = first.text.trim_start().to_string();
                    }
                    if let Some(last) = pieces.last_mut() {
                        last.text = last.text.trim_end().to_string();
                    }
                    pieces.retain(|p| !p.text.is_empty());
                    if !pieces.is_empty() {
                        return pieces;
                    }
                }
            }
        }
        toa::plain(long.trim())
    }

    #[allow(clippy::too_many_arguments)]
    fn op_mark_citation(&mut self, at: Pos, from: Option<Pos>, long: &str, short: Option<&str>, category: u8, bold: bool, italic: bool) -> Result<OpResult, EditError> {
        Self::check_category(category)?;
        let short = short.map(str::trim).filter(|s| !s.is_empty());
        if long.trim().is_empty() && short.is_none() {
            return Err(EditError::Invalid("Give the citation some text.".into()));
        }
        let pieces = (!long.trim().is_empty()).then(|| self.long_pieces(from, at, long));
        let mut c = Citation::new(pieces, short.map(String::from), category);
        c.bold = bold;
        c.italic = italic;
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        // Marking the same citation twice at one place (the dialog stays open) writes one field, not two.
        let mut twice = false;
        fields::scan_inlines(&p.inlines, 0, &mut |s| {
            if s.offset == off {
                if let Some(m) = toa::parse_ta(s.instr) {
                    twice |= m.category == c.category && m.short == c.short && m.long_text().map(|l| squash(&l)) == c.long_text().map(|l| squash(&l));
                }
            }
        });
        if twice {
            return Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() });
        }
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        fields::insert_inlines_at(&mut p.inlines, off, toa::ta_inlines(&c));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() })
    }

    /// Mark the long citation and the short one wherever they are in the main text. A short citation inside a long
    /// one is part of it, and one that follows a mark of the same entry is left alone.
    #[allow(clippy::too_many_arguments)]
    fn op_mark_all_citations(&mut self, from: Option<Pos>, to: Option<Pos>, long: &str, short: Option<&str>, category: u8, bold: bool, italic: bool, ignore_case: bool) -> Result<OpResult, EditError> {
        Self::check_category(category)?;
        let long = long.trim();
        let short = short.map(str::trim).filter(|s| !s.is_empty());
        if long.is_empty() && short.is_none() {
            return Err(EditError::Invalid("Give the citation some text.".into()));
        }
        let pieces = (!long.is_empty()).then(|| match to {
            Some(t) => self.long_pieces(from, t, long),
            None => toa::plain(long),
        });
        let mut with_long = Citation::new(pieces, short.map(String::from), category);
        with_long.bold = bold;
        with_long.italic = italic;
        let mut short_only = Citation::new(None, short.map(String::from), category);
        short_only.bold = bold;
        short_only.italic = italic;
        // (paragraph, offset after the text, the mark to put there)
        let mut hits: Vec<(NodeId, usize, bool)> = Vec::new();
        walk_paragraphs(&self.body, &mut |p| {
            if is_list_style(&style_of(p)) {
                return;
            }
            let hay = p.text();
            let u: Vec<u16> = hay.encode_utf16().collect();
            // Marks already in the paragraph: where they are and what they mark.
            let mut marked: Vec<(usize, Citation)> = Vec::new();
            fields::scan_inlines(&p.inlines, 0, &mut |s| {
                if let Some(c) = toa::parse_ta(s.instr) {
                    marked.push((s.offset, c));
                }
            });
            let already = |end: usize, is_long: bool| {
                marked.iter().any(|(o, c)| {
                    *o == end
                        && c.category == category
                        && if is_long { long.is_empty() || c.long_text().as_deref().is_some_and(|l| squash(l) == squash(long)) } else { c.short.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(short.unwrap_or_default())) }
                })
            };
            let mut spans: Vec<(usize, usize)> = Vec::new();
            if !long.is_empty() {
                let mut at = 0;
                while let Some((start, end)) = find_folded(&hay, long, at, ignore_case) {
                    spans.push((start, end));
                    if !already(end, true) {
                        hits.push((p.id, end, true));
                    }
                    at = end.max(start + 1);
                }
            }
            // The selected citation is marked itself, whatever its text is (it may have been corrected in the box).
            if let (Some(t), false) = (to.filter(|t| t.p.0 == p.id), long.is_empty()) {
                if !spans.iter().any(|(_, e)| *e == t.o) && !already(t.o, true) {
                    hits.push((p.id, t.o, true));
                }
                if let Some(f) = from.filter(|f| f.p.0 == p.id && f.o < t.o) {
                    spans.push((f.o, t.o));
                }
            }
            if let Some(s) = short {
                let word = |i: usize| u.get(i).and_then(|c| char::from_u32(*c as u32)).is_some_and(|c| c.is_alphanumeric());
                let mut at = 0;
                while let Some((start, end, _)) = find_text(&hay, s, at, ignore_case) {
                    at = end.max(start + 1);
                    let inside = spans.iter().any(|(a, b)| start >= *a && end <= *b);
                    if inside || (start > 0 && word(start - 1)) || word(end) {
                        continue;
                    }
                    if !already(end, false) {
                        hits.push((p.id, end, false));
                    }
                }
            }
        });
        if hits.is_empty() {
            let any = {
                let mut seen = false;
                walk_paragraphs(&self.body, &mut |p| {
                    let h = p.text();
                    seen |= !is_list_style(&style_of(p)) && (long.is_empty() || find_folded(&h, long, 0, ignore_case).is_some() || short.is_some_and(|s| find_text(&h, s, 0, ignore_case).is_some()));
                });
                seen
            };
            return if any { Ok(OpResult { count: Some(0), ..Default::default() }) } else { Err(EditError::Invalid(format!("The citation {long:?} isn't in the document."))) };
        }
        // Later offsets first, so earlier ones stay where they are.
        hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        for (pid, end, is_long) in &hits {
            let c = if *is_long { &with_long } else { &short_only };
            self.touch(*pid);
            let p = self.para_mut(*pid)?;
            let off = (*end).min(p.len16());
            fields::insert_inlines_at(&mut p.inlines, off, toa::ta_inlines(c));
        }
        Ok(OpResult { count: Some(hits.len()), ..Default::default() })
    }

    fn op_insert_toa(&mut self, after: Option<NodeId>, before: Option<NodeId>, options: &ToaOptions, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        Self::check_category(options.category.max(1))?;
        let instr = options.instr(None);
        self.styles.ensure_builtin("TableofAuthorities");
        if options.headings {
            self.styles.ensure_builtin("TOAHeading");
        }
        // The style is written when the leader is not the default, and when the style still holds another one from an
        // earlier table (the update that follows reads the leader from it).
        if options.look.is_some() || options.leader != Leader::Dot || self.style_leader("TableofAuthorities").is_some_and(|l| l != options.leader) {
            self.set_toa_styles(options.look.as_deref(), Some(options.leader));
        }
        let page_map = self.page_map(pages);
        let els = self.toa_paragraphs(&instr, &page_map, Some(options.leader));
        let blocks: Vec<Block> = els.iter().map(|e| Block::Para(parse_paragraph(e, &mut self.ids))).collect();
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        self.insert_blocks(after, before, blocks)?;
        Ok(OpResult { created, ..Default::default() })
    }

    fn op_set_toa(&mut self, at: Option<NodeId>, options: &ToaOptions, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        Self::check_category(options.category.max(1))?;
        let f = self.find_toa(at)?;
        let instr = options.instr(Some(&f.instr));
        let first = f.paras[0];
        self.touch(first);
        if !set_list_instr(self.para_mut(first)?, &instr) {
            return Err(EditError::Invalid("could not change the table of authorities".into()));
        }
        self.styles.ensure_builtin("TableofAuthorities");
        if options.headings {
            self.styles.ensure_builtin("TOAHeading");
        }
        self.set_toa_styles(options.look.as_deref(), Some(options.leader));
        let scope: HashSet<NodeId> = f.paras.iter().copied().collect();
        let (created, count) = self.update_block_fields_with(pages, BlockKinds { toc: false, tof: false, index: true }, Some(&scope), Some(options.leader));
        Ok(OpResult { created, count: Some(count), ..Default::default() })
    }

    fn op_update_toa(&mut self, at: Option<NodeId>, pages: Option<&HashMap<String, u32>>) -> Result<OpResult, EditError> {
        let scope: HashSet<NodeId> = match at {
            Some(_) => self.find_toa(at)?.paras.into_iter().collect(),
            None => {
                let mut found = Vec::new();
                collect_fields(&self.body, &toa::is_toa, &mut found);
                if found.is_empty() {
                    return Err(EditError::Invalid("the document has no table of authorities".into()));
                }
                found.into_iter().flat_map(|f| f.paras).collect()
            }
        };
        let (created, count) = self.update_block_fields_with(pages, BlockKinds { toc: false, tof: false, index: true }, Some(&scope), None);
        Ok(OpResult { created, count: Some(count), ..Default::default() })
    }

    fn op_set_toa_names(&mut self, new: &[toa::CategoryName]) -> Result<OpResult, EditError> {
        for n in new {
            if !(toa::FIRST_CUSTOM..=toa::LAST_CATEGORY).contains(&n.category) {
                return Err(EditError::Invalid("Only categories 8 to 16 can be renamed.".into()));
            }
        }
        self.ensure_custom_part();
        let mut names = toa::read_names(self.custom.as_ref());
        for n in new {
            let name = n.name.trim();
            if name.is_empty() {
                names.remove(&n.category);
            } else {
                names.insert(n.category, name.to_string());
            }
        }
        let custom = self.custom.as_mut().expect("just made");
        let before = custom.clone();
        toa::write_names(custom, &names);
        if *custom != before {
            self.custom_dirty = true;
        }
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Next citation
    // -----------------------------------------------------------------------

    /// The next text after `from` that looks like a citation and isn't marked yet, as the positions of its start
    /// and end. The main text only.
    pub fn next_citation(&self, from: Pos) -> Option<(Pos, Pos)> {
        let mut started = false;
        let mut found: Option<(Pos, Pos)> = None;
        walk_paragraphs(self.main_body(), &mut |p| {
            if found.is_some() {
                return;
            }
            if !started {
                if p.id != from.p.0 {
                    return;
                }
                started = true;
            }
            if is_list_style(&style_of(p)) {
                return;
            }
            let mut marks: Vec<usize> = Vec::new();
            fields::scan_inlines(&p.inlines, 0, &mut |s| {
                if toa::is_ta(s.instr) {
                    marks.push(s.offset);
                }
            });
            let text = p.text();
            let mut at = if p.id == from.p.0 { from.o } else { 0 };
            while let Some((s, e)) = toa::find_citation(&text, at) {
                if !marks.contains(&e) {
                    found = Some((Pos::new(p.id, s), Pos::new(p.id, e)));
                    return;
                }
                at = e;
            }
        });
        found
    }
}

/// How an entry is indented in each format: the left indent and the hanging part, in twips.
fn entry_indent(look: &str) -> Option<(i64, i64)> {
    Some(match look {
        "classic" => (440, 440),
        "distinctive" => (220, 220),
        "formal" => (360, 360),
        "simple" => (0, 0),
        _ => return None,
    })
}

struct HeadingLook {
    before: i64,
    after: i64,
    center: bool,
    caps: bool,
    small_caps: bool,
}

fn heading_look(look: &str) -> Option<HeadingLook> {
    Some(match look {
        "classic" => HeadingLook { before: 240, after: 120, center: false, caps: false, small_caps: false },
        "distinctive" => HeadingLook { before: 240, after: 120, center: true, caps: false, small_caps: true },
        "formal" => HeadingLook { before: 240, after: 120, center: false, caps: true, small_caps: false },
        "simple" => HeadingLook { before: 120, after: 60, center: false, caps: false, small_caps: false },
        _ => return None,
    })
}
