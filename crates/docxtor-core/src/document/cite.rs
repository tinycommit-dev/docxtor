//! Citations and the bibliography as operations on a document: the list of sources (Word's Manage Sources),
//! citation fields and the bibliography field, kept in step with the chosen style.

use super::*;
use crate::cite_fields as cf;
use crate::cite_styles::{Item, Plan, Rich, Style};
use crate::citations::{guid_from, lcid_for, make_tag, valid_tag, CitationInfo, CiteInstr, Source, StyleId};
use crate::csl;
use std::sync::Arc;

fn bad(msg: impl Into<String>) -> EditError {
    EditError::Invalid(msg.into())
}

/// The citation style of a document, ready to format with.
enum Resolved {
    Builtin(Style),
    Csl(Arc<csl::Style>),
}

/// A citation in reading order: its paragraph, its number among the citations of that paragraph, its
/// field instruction and the footnote it sits in.
struct Site {
    p: NodeId,
    ord: usize,
    instr: String,
    note: Option<usize>,
}

/// A CSL style registered for a document, as the interface lists it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CslStyleInfo {
    pub id: String,
    pub title: String,
    /// Citations are footnotes.
    pub note: bool,
    /// Citations are numbers.
    pub numeric: bool,
}

// ---------------------------------------------------------------------------
// Reading citations from the model
// ---------------------------------------------------------------------------

/// Citation content controls in some inlines: (start offset, length, instruction).
fn scan_inlines(inlines: &[Inline], start: &mut usize, out: &mut Vec<(usize, usize, String)>) {
    for i in inlines {
        match i {
            Inline::Run(r) => *start += r.len16(),
            Inline::Container(c) if cf::is_citation_sdt(c) => {
                let len: usize = c.children.iter().map(|x| x.len16()).sum();
                if let Some(instr) = cf::first_instr(&c.children) {
                    out.push((*start, len, instr));
                }
                *start += len;
            }
            Inline::Container(c) => scan_inlines(&c.children, start, out),
            Inline::Opaque(_) => *start += i.len16(),
        }
    }
}

/// The instruction of every `CITATION` field in some inlines, in reading order.
fn instrs_in(inlines: &[Inline], out: &mut Vec<String>) {
    let mut items: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, inl) in inlines.iter().enumerate() {
        if let Inline::Container(c) = inl {
            let mut v = Vec::new();
            instrs_in(&c.children, &mut v);
            if !v.is_empty() {
                items.push((i, v));
            }
        }
    }
    for sp in cf::spans(&[inlines]) {
        if sp.name() == "CITATION" {
            items.push((sp.begin.inl, vec![sp.instr]));
        }
    }
    items.sort_by_key(|(i, _)| *i);
    for (_, v) in items {
        out.extend(v);
    }
}

fn shell_id(shell: &Element) -> Option<i64> {
    shell.child("w:sdtPr")?.child("w:id")?.attr("w:val")?.parse().ok()
}

fn sdt_ids_inl(inlines: &[Inline], out: &mut Vec<i64>) {
    for i in inlines {
        if let Inline::Container(c) = i {
            if c.kind == InlineKind::Sdt {
                out.extend(shell_id(&c.shell));
            }
            sdt_ids_inl(&c.children, out);
        }
    }
}

fn sdt_ids<'a>(blocks: impl IntoIterator<Item = &'a Block>, out: &mut Vec<i64>) {
    for b in blocks {
        match b {
            Block::Para(p) => sdt_ids_inl(&p.inlines, out),
            Block::Container(c) => {
                out.extend(shell_id(&c.shell));
                sdt_ids(&c.blocks, out);
            }
            Block::Table(t) => {
                for r in t.rows() {
                    for c in r.cells() {
                        sdt_ids(&c.blocks, out);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// The content control with this id, wherever it sits.
fn find_container<'a>(blocks: impl IntoIterator<Item = &'a Block>, id: i64) -> Option<&'a BlockContainer> {
    for b in blocks {
        match b {
            Block::Container(c) if shell_id(&c.shell) == Some(id) => return Some(c),
            Block::Container(c) => {
                if let Some(f) = find_container(&c.blocks, id) {
                    return Some(f);
                }
            }
            Block::Table(t) => {
                for r in t.rows() {
                    for cell in r.cells() {
                        if let Some(f) = find_container(&cell.blocks, id) {
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

fn trim_opt(o: &mut Option<String>) {
    if let Some(s) = o {
        let t = s.trim();
        *o = if t.is_empty() { None } else { Some(t.to_string()) };
    }
}

/// Tidy a source: trimmed text, no empty names or empty optional fields.
fn clean_source(s: &mut Source) {
    s.title = s.title.trim().to_string();
    s.tag = s.tag.trim().to_string();
    let keep = |n: &crate::citations::Name| !n.family().trim().is_empty();
    s.authors.retain(keep);
    s.editors.retain(keep);
    for o in [
        &mut s.container_title, &mut s.year, &mut s.month, &mut s.day, &mut s.publisher, &mut s.place, &mut s.volume, &mut s.issue,
        &mut s.pages, &mut s.edition, &mut s.url, &mut s.doi, &mut s.accessed, &mut s.genre, &mut s.guid,
    ] {
        trim_opt(o);
    }
}

fn check_has_content(s: &Source) -> Result<(), EditError> {
    if s.title.is_empty() && s.authors.is_empty() && s.editors.is_empty() {
        return Err(bad("A source needs at least a title or an author."));
    }
    Ok(())
}

fn check_new_tag(tag: &str, existing: &[Source]) -> Result<(), EditError> {
    if !valid_tag(tag) {
        return Err(bad(format!("'{tag}' can't be a source tag. Use letters and digits without spaces, like Smi20.")));
    }
    if existing.iter().any(|e| e.tag == tag) {
        return Err(bad(format!("A source with the tag '{tag}' already exists.")));
    }
    Ok(())
}

fn fresh_guid(existing: &[Source], seed: &str) -> String {
    let mut salt = existing.len() as u32 + 1;
    loop {
        let g = guid_from(seed, salt);
        if !existing.iter().any(|e| e.guid.as_deref() == Some(g.as_str())) {
            return g;
        }
        salt += 1;
    }
}

/// Make citations of only the removed source into plain text.
fn unwrap_inlines(inlines: &mut Vec<Inline>, tag: &str) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < inlines.len() {
        let replacement = match &mut inlines[i] {
            Inline::Container(c) if cf::is_citation_sdt(c) => {
                let only = cf::first_instr(&c.children).and_then(|s| CiteInstr::parse(&s)).is_some_and(|ci| ci.tags.iter().all(|t| t == tag));
                only.then(|| cf::result_inlines(&c.children))
            }
            Inline::Container(c) => {
                n += unwrap_inlines(&mut c.children, tag);
                None
            }
            _ => None,
        };
        match replacement {
            Some(rep) => {
                let k = rep.len();
                inlines.splice(i..=i, rep);
                i += k;
                n += 1;
            }
            None => i += 1,
        }
    }
    n
}

fn set_para_text(p: &mut Paragraph, text: &str) {
    let mut done = false;
    for inl in p.inlines.iter_mut() {
        if let Inline::Run(r) = inl {
            for it in r.items.iter_mut() {
                if let RunItem::Text(t) = it {
                    if done {
                        t.clear();
                    } else {
                        *t = text.to_string();
                        done = true;
                    }
                }
            }
        }
    }
}

/// Bibliography headings Docxtor or Word wrote by default: switching style changes them to the new style's.
const DEFAULT_HEADINGS: [&str; 3] = ["References", "Works Cited", "Bibliography"];

fn fix_headings(blocks: &mut [Block], heading: &str) -> usize {
    let mut n = 0;
    for b in blocks.iter_mut() {
        if let Block::Container(c) = b {
            if cf::has_bibliography(&c.blocks) {
                if let Some(Block::Para(p)) = c.blocks.first_mut() {
                    let is_heading = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).is_some_and(|s| s == "Heading1");
                    let text = p.plain_text();
                    if is_heading && DEFAULT_HEADINGS.contains(&text.trim()) && text.trim() != heading {
                        set_para_text(p, heading);
                        n += 1;
                    }
                }
            }
            n += fix_headings(&mut c.blocks, heading);
        }
    }
    n
}

/// Run `f` on the citation content control that starts at `target`.
fn edit_citation_at(inlines: &mut [Inline], start: &mut usize, target: usize, f: &mut dyn FnMut(&mut InlineContainer)) -> bool {
    for i in inlines.iter_mut() {
        match i {
            Inline::Run(r) => *start += r.len16(),
            Inline::Container(c) if cf::is_citation_sdt(c) => {
                if *start == target {
                    f(c);
                    return true;
                }
                *start += c.children.iter().map(|x| x.len16()).sum::<usize>();
            }
            Inline::Container(c) => {
                if edit_citation_at(&mut c.children, start, target, f) {
                    return true;
                }
            }
            Inline::Opaque(e) => *start += usize::from(crate::math::is_math(e)),
        }
    }
    false
}

/// Take the citation that starts at `target` out of some inlines.
fn remove_citation_at(inlines: &mut Vec<Inline>, start: &mut usize, target: usize) -> bool {
    let mut i = 0;
    while i < inlines.len() {
        match &mut inlines[i] {
            Inline::Run(r) => *start += r.len16(),
            Inline::Container(c) if cf::is_citation_sdt(c) => {
                if *start == target {
                    inlines.remove(i);
                    return true;
                }
                *start += c.children.iter().map(|x| x.len16()).sum::<usize>();
            }
            Inline::Container(c) => {
                if remove_citation_at(&mut c.children, start, target) {
                    return true;
                }
            }
            Inline::Opaque(e) => *start += usize::from(crate::math::is_math(e)),
        }
        i += 1;
    }
    false
}

/// Take the mark of a footnote out of the text. `off` ends up as the offset where it was.
fn remove_note_mark(inlines: &mut Vec<Inline>, id: &str, off: &mut usize) -> bool {
    let mut i = 0;
    while i < inlines.len() {
        if let Inline::Container(c) = &mut inlines[i] {
            if remove_note_mark(&mut c.children, id, off) {
                return true;
            }
        }
        let is_mark = matches!(&inlines[i], Inline::Run(r) if r.items.len() == 1 && matches!(&r.items[0], RunItem::Object(e) if e.name == "w:footnoteReference" && e.attr("w:id") == Some(id)));
        if is_mark {
            inlines.remove(i);
            return true;
        }
        *off += inlines[i].len16();
        i += 1;
    }
    false
}

/// Put the citation that was in a footnote where the note's mark is. A space goes before the citation when
/// it follows a word. Returns whether the mark was found.
fn swap_note_mark(inlines: &mut Vec<Inline>, id: &str, sdt: &mut Option<Inline>, space: bool) -> bool {
    for i in 0..inlines.len() {
        if let Inline::Container(c) = &mut inlines[i] {
            if swap_note_mark(&mut c.children, id, sdt, space) {
                return true;
            }
            continue;
        }
        let is_mark = matches!(&inlines[i], Inline::Run(r) if r.items.len() == 1 && matches!(&r.items[0], RunItem::Object(e) if e.name == "w:footnoteReference" && e.attr("w:id") == Some(id)));
        if !is_mark {
            continue;
        }
        let after_word = space && i > 0 && matches!(&inlines[i - 1], Inline::Run(p) if matches!(p.items.last(), Some(RunItem::Text(t)) if t.chars().last().is_some_and(|c| !c.is_whitespace() && c != '(')));
        let Some(sdt) = sdt.take() else { return false };
        let mut replacement = Vec::new();
        if after_word {
            replacement.push(Inline::Run(Run { items: vec![RunItem::Text(" ".into())], ..Default::default() }));
        }
        replacement.push(sdt);
        inlines.splice(i..=i, replacement);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Document API
// ---------------------------------------------------------------------------

impl Document {
    /// The document's list of sources, in Word's order.
    pub fn sources(&self) -> Vec<Source> {
        self.cites.sources()
    }

    /// The citation style: `apa`, `mla`, `chicago`, `ieee` or `harvard`, or the id of a CSL style.
    pub fn citation_style(&self) -> String {
        self.cites.style_id().id()
    }

    /// Make a CSL style available to this document under an id. The style is not saved in the file: a
    /// document remembers the id, and whoever opens it registers the style again. Registering an id again
    /// replaces the style. Fields already in the document are not rewritten.
    pub fn register_citation_style(&mut self, id: &str, csl_xml: &str) -> Result<CslStyleInfo, String> {
        let id = id.trim();
        if id.is_empty() || id.chars().any(|c| c.is_whitespace()) {
            return Err("A citation style needs an id without spaces.".into());
        }
        if Style::ALL.iter().any(|s| s.id() == id) {
            return Err(format!("'{id}' is one of Docxtor's own citation styles. Give the CSL style another id."));
        }
        let style = csl::Style::parse(csl_xml)?;
        let info = CslStyleInfo { id: id.to_string(), title: style.title.clone(), note: style.is_note(), numeric: style.is_numeric() };
        self.csl_styles.insert(id.to_string(), Arc::new(style));
        Ok(info)
    }

    /// The CSL styles registered for this document.
    pub fn citation_styles(&self) -> Vec<CslStyleInfo> {
        let mut v: Vec<CslStyleInfo> = self.csl_styles.iter().map(|(id, s)| CslStyleInfo { id: id.clone(), title: s.title.clone(), note: s.is_note(), numeric: s.is_numeric() }).collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Whether the document's citations are footnotes (its style is a CSL note style).
    pub fn citations_are_notes(&self) -> bool {
        self.notes_style()
    }

    /// Choose a style by id: one of Docxtor's five, or a registered CSL style.
    fn parse_style_id(&self, id: &str) -> Result<StyleId, EditError> {
        let id = id.trim();
        if self.csl_styles.contains_key(id) {
            return Ok(StyleId::Csl(id.to_string()));
        }
        if let Some(s) = Style::parse(id) {
            return Ok(StyleId::Builtin(s));
        }
        let mut csl: Vec<&String> = self.csl_styles.keys().collect();
        csl.sort();
        let more = if csl.is_empty() { String::new() } else { format!(", or a CSL style added here: {}", csl.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) };
        Err(bad(format!("'{id}' is not a citation style Docxtor knows. Use apa, mla, chicago, ieee or harvard{more}.")))
    }

    /// Where the citations are (content controls around `CITATION` fields), in reading order: the body, then
    /// headers, footers and notes.
    pub fn citations(&self) -> Vec<CitationInfo> {
        let mut out = Vec::new();
        for blocks in self.block_lists() {
            walk_paragraphs(blocks, &mut |p| {
                let mut found = Vec::new();
                scan_inlines(&p.inlines, &mut 0, &mut found);
                for (o, len, instr) in found {
                    if let Some(ci) = CiteInstr::parse(&instr) {
                        out.push(CitationInfo { p: p.id, o, len, tags: ci.tags, page: ci.page, suppress_author: ci.no_author, suppress_year: ci.no_year, suppress_title: ci.no_title });
                    }
                }
            });
        }
        out
    }

    /// How many `CITATION` fields use a source.
    fn uses_of(&self, tag: &str) -> usize {
        let mut instrs = Vec::new();
        for blocks in self.block_lists() {
            walk_paragraphs(blocks, &mut |p| instrs_in(&p.inlines, &mut instrs));
        }
        instrs.iter().filter_map(|i| CiteInstr::parse(i)).filter(|c| c.tags.iter().any(|t| t == tag)).count()
    }

    /// Write every citation and bibliography again from the sources, in the chosen style. Returns how many
    /// fields were rewritten. Meant for the main text: does nothing while an op runs in a header or note.
    pub fn update_citations(&mut self) -> usize {
        self.update_citations_with(None, None, false)
    }

    /// Run `f` on the body and each story, marking what it changed.
    fn edit_lists(&mut self, f: &mut dyn FnMut(&mut Vec<Block>, &mut IdGen) -> usize) -> usize {
        let mut total = 0;
        let mut n = 0;
        let ids = &mut self.ids;
        self.body.edit_as_vec(|v| {
            n = f(v, ids);
            n > 0
        });
        if n > 0 {
            self.body_dirty = true;
            self.touch_all();
            total += n;
        }
        for i in 0..self.stories.len() {
            let n = f(&mut self.stories[i].blocks, &mut self.ids);
            if n > 0 {
                let tops = self.stories[i].blocks.iter().map(|b| b.id()).collect();
                self.story_edited(i, tops);
                self.story_gen += 1;
                self.stories[i].gen = self.story_gen;
                self.dirty_parts.insert(self.stories[i].part.clone());
                self.stories_touched = true;
                total += n;
            }
        }
        total
    }

    fn update_citations_with(&mut self, rename: Option<(&str, &str)>, remove: Option<&str>, fix_heading: bool) -> usize {
        if self.parked.is_some() {
            return 0;
        }
        let sources = self.cites.sources();
        let mut total = 0;
        if let Some(tag) = remove {
            total += self.edit_lists(&mut |blocks, _| {
                let mut n = 0;
                walk_paragraphs_mut(blocks, &mut |p| n += unwrap_inlines(&mut p.inlines, tag));
                n
            });
        }
        match self.resolved_style() {
            Resolved::Csl(style) => total + self.update_csl(&style, &sources, rename, remove, fix_heading),
            Resolved::Builtin(style) => total + self.update_builtin(style, &sources, rename, remove, fix_heading),
        }
    }

    /// The style the document names, as something Docxtor can format with. A CSL style that was never registered
    /// here reads as the Word style closest to it, so the text already in the document stays as it is.
    fn resolved_style(&self) -> Resolved {
        match self.cites.style_id() {
            StyleId::Csl(id) => match self.csl_styles.get(&id) {
                Some(s) => Resolved::Csl(s.clone()),
                None => Resolved::Builtin(self.cites.style()),
            },
            StyleId::Builtin(s) => Resolved::Builtin(s),
        }
    }

    /// The hanging indent of bibliography entries and the heading above them, in the document's style.
    fn bibliography_layout(&self) -> ((i64, i64), String) {
        match self.resolved_style() {
            Resolved::Csl(s) => (s.bibliography_indent(), s.bibliography_heading().to_string()),
            Resolved::Builtin(s) => (s.indent(), s.heading().to_string()),
        }
    }

    /// Whether the document's citations are footnotes.
    pub(crate) fn notes_style(&self) -> bool {
        matches!(self.resolved_style(), Resolved::Csl(s) if s.is_note())
    }

    /// Where citations are in reading order, with the number of the footnote each sits in. The body comes
    /// first, with the notes it refers to read where they are referred to; headers, footers and notes nothing
    /// refers to come last.
    fn doc_order_sites(&self) -> Vec<Site> {
        fn walk<'a>(doc: &Document, blocks: impl IntoIterator<Item = &'a Block>, note: Option<usize>, notes: &mut usize, done: &mut Vec<usize>, out: &mut Vec<Site>) {
            let mut paras: Vec<&Paragraph> = Vec::new();
            walk_paragraphs(blocks, &mut |p| paras.push(p));
            for p in paras {
                let mut marks = Vec::new();
                cf::marks_in_order(&p.inlines, &mut marks);
                let mut ord = 0;
                for m in marks {
                    match m {
                        cf::Mark::Cite(instr) => {
                            out.push(Site { p: p.id, ord, instr, note });
                            ord += 1;
                        }
                        cf::Mark::Note(id) => {
                            *notes += 1;
                            let n = *notes;
                            let at = doc.stories.iter().position(|s| s.kind == StoryKind::Footnote && s.note_id.as_deref() == Some(id.as_str()));
                            if let Some(si) = at.filter(|si| !done.contains(si)) {
                                done.push(si);
                                walk(doc, &doc.stories[si].blocks, Some(n), notes, done, out);
                            }
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        let mut notes = 0;
        let mut done = Vec::new();
        walk(self, self.main_body(), None, &mut notes, &mut done, &mut out);
        for i in 0..self.stories.len() {
            if !done.contains(&i) {
                done.push(i);
                walk(self, &self.stories[i].blocks, None, &mut notes, &mut done, &mut out);
            }
        }
        out
    }

    /// Write citations and the bibliography again with a CSL style.
    fn update_csl(&mut self, style: &Arc<csl::Style>, sources: &[Source], rename: Option<(&str, &str)>, remove: Option<&str>, fix_heading: bool) -> usize {
        let mapped = |t: &str| -> String {
            match rename {
                Some((from, to)) if t == from => to.to_string(),
                _ => t.to_string(),
            }
        };
        let sites = self.doc_order_sites();
        let mut csl_sites: Vec<csl::CiteSite> = Vec::new();
        // (paragraph, number of the citation in it) to the new instruction, if any, and the formatted site.
        let mut by_site: HashMap<(NodeId, usize), (Option<String>, usize)> = HashMap::new();
        // Citations of placeholder sources: their text is the tag in brackets and they are not in the bibliography.
        let mut placeholders: HashMap<(NodeId, usize), (Option<String>, Rich)> = HashMap::new();
        for s in &sites {
            let Some(mut ci) = CiteInstr::parse(&s.instr) else { continue };
            let before = ci.tags.clone();
            ci.tags = ci.tags.iter().map(|t| mapped(t)).filter(|t| Some(t.as_str()) != remove).collect();
            if ci.tags.is_empty() {
                continue;
            }
            if ci.tags.iter().all(|t| sources.iter().any(|x| &x.tag == t && x.is_placeholder())) {
                placeholders.insert((s.p, s.ord), ((ci.tags != before).then(|| ci.build()), Rich::plain_of(&crate::citations::placeholder_text(&ci.tags))));
                continue;
            }
            let last = ci.tags.len() - 1;
            let mut items = Vec::new();
            for (i, t) in ci.tags.iter().enumerate() {
                let Some(src) = sources.iter().find(|x| &x.tag == t) else { break };
                let mut it = csl::CiteItem::new(src);
                if i == 0 {
                    it.locator = ci.page.clone();
                    it.prefix = ci.prefix.clone();
                }
                if i == last {
                    it.suffix = ci.suffix.clone();
                }
                it.suppress_author = ci.no_author;
                it.suppress_year = ci.no_year;
                items.push(it);
            }
            if items.len() != ci.tags.len() {
                continue;
            }
            by_site.insert((s.p, s.ord), ((ci.tags != before).then(|| ci.build()), csl_sites.len()));
            csl_sites.push(csl::CiteSite { items, note: s.note });
        }
        let out = csl::process(style, &csl_sites);
        let entries: Vec<Rich> = out.bibliography.iter().map(|e| e.text.clone()).collect();
        let indent = style.bibliography_indent();
        let heading = style.bibliography_heading();
        self.edit_lists(&mut |blocks, ids| {
            let mut n = 0;
            walk_paragraphs_mut(blocks, &mut |p| {
                let pid = p.id;
                n += cf::rewrite_citations(&mut p.inlines, &mut 0, &mut |ord, _| {
                    if let Some(ph) = placeholders.get(&(pid, ord)) {
                        return Some(ph.clone());
                    }
                    let (new_instr, idx) = by_site.get(&(pid, ord))?;
                    let rich = out.citations[*idx].clone();
                    (!rich.is_empty()).then(|| (new_instr.clone(), rich))
                });
            });
            n += cf::rewrite_bibliographies(blocks, &entries, indent, ids);
            if fix_heading {
                n += fix_headings(blocks, heading);
            }
            n
        })
    }

    fn update_builtin(&mut self, style: Style, sources: &[Source], rename: Option<(&str, &str)>, remove: Option<&str>, fix_heading: bool) -> usize {
        let mut total = 0;
        // The sources in the order they are first cited decide numbering and the letters after years.
        let mut instrs = Vec::new();
        for blocks in self.block_lists() {
            walk_paragraphs(blocks, &mut |p| instrs_in(&p.inlines, &mut instrs));
        }
        let mapped = |t: &str| -> String {
            match rename {
                Some((from, to)) if t == from => to.to_string(),
                _ => t.to_string(),
            }
        };
        let mut order: Vec<String> = Vec::new();
        for ins in &instrs {
            if let Some(ci) = CiteInstr::parse(ins) {
                for t in &ci.tags {
                    let t = mapped(t);
                    if Some(t.as_str()) != remove && !order.contains(&t) {
                        order.push(t);
                    }
                }
            }
        }
        let cited: Vec<&Source> = order.iter().filter_map(|t| sources.iter().find(|s| &s.tag == t)).filter(|s| !s.is_placeholder()).collect();
        let plan = Plan::new(style, &cited);
        let entries: Vec<Rich> = plan.entries().iter().map(|s| plan.reference(s)).collect();
        let mut render = |_ord: usize, instr: &str| -> Option<cf::Rendered> {
            let mut ci = CiteInstr::parse(instr)?;
            let before = ci.tags.clone();
            ci.tags = ci.tags.iter().map(|t| mapped(t)).filter(|t| Some(t.as_str()) != remove).collect();
            if ci.tags.is_empty() {
                return None;
            }
            if ci.tags.iter().all(|t| sources.iter().any(|s| &s.tag == t && s.is_placeholder())) {
                return Some(((ci.tags != before).then(|| ci.build()), Rich::plain_of(&crate::citations::placeholder_text(&ci.tags))));
            }
            let mut items: Vec<Item> = Vec::new();
            for (i, t) in ci.tags.iter().enumerate() {
                let src = cited.iter().find(|s| &s.tag == t)?;
                items.push(Item { src, page: if i == 0 { ci.page.as_deref() } else { None }, no_author: ci.no_author, no_year: ci.no_year, no_title: ci.no_title });
            }
            let rich = plan.in_text(&items);
            if rich.is_empty() {
                return None;
            }
            Some(((ci.tags != before).then(|| ci.build()), rich))
        };
        let indent = style.indent();
        let heading = style.heading();
        total += self.edit_lists(&mut |blocks, ids| {
            let mut n = 0;
            walk_paragraphs_mut(blocks, &mut |p| n += cf::rewrite_citations(&mut p.inlines, &mut 0, &mut render));
            n += cf::rewrite_bibliographies(blocks, &entries, indent, ids);
            if fix_heading {
                n += fix_headings(blocks, heading);
            }
            n
        });
        total
    }

    /// A new content control id, unused in the body and notes.
    pub(super) fn next_sdt_id(&self) -> i64 {
        let mut ids = Vec::new();
        for blocks in self.block_lists() {
            sdt_ids(blocks, &mut ids);
        }
        match ids.iter().max() {
            Some(&m) if m < i32::MAX as i64 && m >= 100_000_000 => m + 1,
            Some(&m) if m >= i32::MAX as i64 => ids.iter().min().copied().unwrap_or(0).min(0) - 1,
            _ => 1_000_000_001,
        }
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn apply_cite_op(&mut self, op: &Op) -> Result<OpResult, EditError> {
        match op {
            Op::AddSource { source } => self.op_add_source(source),
            Op::UpdateSource { tag, source } => self.op_update_source(tag, source),
            Op::InsertCitationPlaceholder { at, tag } => self.op_insert_placeholder(*at, tag.as_deref()),
            Op::DeleteSource { tag, force } => self.op_delete_source(tag, *force),
            Op::SetCitationStyle { style } => {
                if self.parked.is_some() {
                    return Err(bad("The citation style can only be changed from the main text."));
                }
                let new = self.parse_style_id(style)?;
                let was_notes = self.notes_style();
                // Word updates the fields in the closest style of its own: the one a style of this kind would use.
                let near = match &new {
                    StyleId::Builtin(s) => *s,
                    StyleId::Csl(id) => match self.csl_styles.get(id) {
                        Some(s) if s.is_note() => Style::Chicago,
                        Some(s) if s.is_numeric() => Style::Ieee,
                        _ => Style::Apa,
                    },
                };
                self.cites.set_style_id(&new, near);
                let now_notes = self.notes_style();
                // Citations become footnotes, or footnotes that hold only a citation become citations again.
                let mut n = 0;
                if now_notes && !was_notes {
                    n += self.citations_to_notes()?;
                } else if was_notes && !now_notes {
                    n += self.notes_to_citations();
                }
                n += self.update_citations_with(None, None, true);
                Ok(OpResult { count: Some(n), ..Default::default() })
            }
            Op::InsertCitation { at, tags, page, suppress_author, suppress_year, suppress_title } => self.op_insert_citation(*at, tags, page.as_deref(), [*suppress_author, *suppress_year, *suppress_title]),
            Op::EditCitation { at, tags, page, suppress_author, suppress_year, suppress_title } => self.op_edit_citation(*at, tags, page.as_deref(), [*suppress_author, *suppress_year, *suppress_title]),
            Op::RemoveCitation { at } => self.op_remove_citation(*at),
            Op::InsertBibliography { after, before, heading } => self.op_insert_bibliography(after.map(|i| i.0), before.map(|i| i.0), heading.as_deref()),
            Op::UpdateBibliography => {
                if self.parked.is_some() {
                    return Err(bad("Citations and the bibliography can only be updated from the main text."));
                }
                let n = self.update_citations();
                Ok(OpResult { count: Some(n), ..Default::default() })
            }
            _ => Err(bad("not a citation op")),
        }
    }

    fn op_add_source(&mut self, source: &Source) -> Result<OpResult, EditError> {
        let mut s = source.clone();
        clean_source(&mut s);
        check_has_content(&s)?;
        let existing = self.cites.sources();
        if s.tag.is_empty() {
            s.tag = make_tag(&s, &|t| existing.iter().any(|e| e.tag == t));
        } else {
            check_new_tag(&s.tag, &existing)?;
        }
        if s.guid.is_none() {
            s.guid = Some(fresh_guid(&existing, &format!("{}{}", s.tag, s.title)));
        }
        self.cites.add(&s);
        Ok(OpResult::default())
    }

    /// Word's "Add new placeholder": a citation of a source that has only a tag so far, to fill in later.
    fn op_insert_placeholder(&mut self, at: Pos, tag: Option<&str>) -> Result<OpResult, EditError> {
        let existing = self.cites.sources();
        let tag = match tag.map(str::trim).filter(|t| !t.is_empty()) {
            Some(t) => {
                check_new_tag(t, &existing)?;
                t.to_string()
            }
            None => (1..).map(|n| format!("Placeholder{n}")).find(|t| !existing.iter().any(|e| &e.tag == t)).unwrap_or_default(),
        };
        let src = Source { tag: tag.clone(), title: tag.clone(), guid: Some(fresh_guid(&existing, &tag)), ..Default::default() };
        self.cites.add(&src);
        self.op_insert_citation(at, &[tag], None, [None; 3])
    }

    fn op_update_source(&mut self, tag: &str, source: &Source) -> Result<OpResult, EditError> {
        let existing = self.cites.sources();
        let old = existing.iter().find(|s| s.tag == tag).ok_or_else(|| bad(format!("There is no source with the tag '{tag}'.")))?;
        let mut s = source.clone();
        clean_source(&mut s);
        check_has_content(&s)?;
        if s.tag.is_empty() {
            s.tag = tag.to_string();
        }
        if s.tag != tag {
            check_new_tag(&s.tag, &existing)?;
        }
        if s.guid.is_none() {
            s.guid = old.guid.clone();
        }
        let renamed = s.tag != tag;
        let new_tag = s.tag.clone();
        let used = self.uses_of(tag) > 0;
        self.cites.update(tag, &s);
        if used {
            let rename = renamed.then_some((tag, new_tag.as_str()));
            self.update_citations_with(rename, None, false);
        }
        Ok(OpResult::default())
    }

    fn op_delete_source(&mut self, tag: &str, force: bool) -> Result<OpResult, EditError> {
        if !self.cites.has(tag) {
            return Err(bad(format!("There is no source with the tag '{tag}'.")));
        }
        let uses = self.uses_of(tag);
        if uses > 0 && !force {
            return Err(bad(format!(
                "The source '{tag}' is cited {uses} time{} in the document. Remove those citations first, or delete the source anyway to turn them into plain text.",
                if uses == 1 { "" } else { "s" }
            )));
        }
        self.cites.delete(tag);
        if uses > 0 {
            self.update_citations_with(None, Some(tag), false);
        } else if self.block_lists().into_iter().any(cf::has_bibliography) {
            self.update_citations();
        }
        Ok(OpResult::default())
    }

    fn check_tags_exist(&self, tags: &[String]) -> Result<(), EditError> {
        let existing = self.cites.sources();
        for t in tags {
            if !existing.iter().any(|s| &s.tag == t) {
                return Err(bad(format!("There is no source with the tag '{t}'. Add the source first.")));
            }
        }
        Ok(())
    }

    fn op_insert_citation(&mut self, at: Pos, tags: &[String], page: Option<&str>, flags: [Option<bool>; 3]) -> Result<OpResult, EditError> {
        if self.parked.is_some() {
            return Err(bad("Citations can only be inserted in the main text."));
        }
        let tags: Vec<String> = tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        if tags.is_empty() {
            return Err(bad("Name at least one source to cite."));
        }
        self.check_tags_exist(&tags)?;
        let instr = CiteInstr {
            tags,
            page: page.map(str::trim).filter(|p| !p.is_empty()).map(String::from),
            no_author: flags[0] == Some(true),
            no_year: flags[1] == Some(true),
            no_title: flags[2] == Some(true),
            lcid: Some(lcid_for(self.language().as_deref()).to_string()),
            ..Default::default()
        }
        .build();
        let id = self.next_sdt_id();
        if self.notes_style() {
            return self.insert_note_citation(at, &instr, id);
        }
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        let mut found = Vec::new();
        scan_inlines(&p.inlines, &mut 0, &mut found);
        if found.iter().any(|(o, len, _)| *o < off && off < o + len) {
            return Err(bad("That position is inside another citation. Put the citation before or after it."));
        }
        let base = edit::rpr_at(&p.inlines, off).map(strip_link_style).map(|mut r| {
            crate::props::put_rpr(&mut r, Element::new("w:noProof"));
            r
        });
        edit::insert_inline_at(&mut p.inlines, off, cf::citation_sdt(&instr, &Rich::new(), id, base.as_ref()));
        self.update_citations();
        let caret = self.citation_end(at.p.0, off).unwrap_or(off);
        Ok(OpResult { caret: Some(Pos::new(at.p.0, caret)), ..Default::default() })
    }

    /// A citation in a note style: a footnote whose text is the citation. The caret stays in the text, after
    /// the note's mark.
    fn insert_note_citation(&mut self, at: Pos, instr: &str, id: i64) -> Result<OpResult, EditError> {
        let p = self.para_ref(at.p.0)?;
        let off = at.o.min(p.len16());
        let mut found = Vec::new();
        scan_inlines(&p.inlines, &mut 0, &mut found);
        if found.iter().any(|(o, len, _)| *o < off && off < o + len) {
            return Err(bad("That position is inside another citation. Put the citation before or after it."));
        }
        let made = self.op_insert_footnote(Pos::new(at.p.0, off), "")?;
        let note_p = made.caret.map(|c| c.p.0).ok_or_else(|| bad("The note for the citation could not be made."))?;
        self.append_to_story_para(note_p, cf::citation_sdt(instr, &Rich::new(), id, None))?;
        self.update_citations();
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }

    /// Add an inline at the end of a paragraph of a note or header.
    fn append_to_story_para(&mut self, pid: NodeId, inline: Inline) -> Result<(), EditError> {
        let si = self.story_index(pid).ok_or(EditError::NotFound(pid))?;
        let story = &mut self.stories[si];
        let Some((list, i)) = find_parent_mut(&mut story.blocks, pid) else { return Err(EditError::NotFound(pid)) };
        let Block::Para(p) = &mut list[i] else { return Err(EditError::NotParagraph(pid)) };
        let end = p.len16();
        edit::insert_inline_at(&mut p.inlines, end, inline);
        self.story_gen += 1;
        story.gen = self.story_gen;
        self.dirty_parts.insert(story.part.clone());
        self.stories_touched = true;
        Ok(())
    }

    /// A paragraph of the text or of a note, to change.
    fn any_para_mut(&mut self, pid: NodeId) -> Result<&mut Paragraph, EditError> {
        if find_block(&self.body, pid).is_some() {
            return self.para_mut(pid);
        }
        let si = self.story_index(pid).ok_or(EditError::NotFound(pid))?;
        self.story_gen += 1;
        let story = &mut self.stories[si];
        story.gen = self.story_gen;
        self.dirty_parts.insert(story.part.clone());
        self.stories_touched = true;
        let Some((list, i)) = find_parent_mut(&mut story.blocks, pid) else { return Err(EditError::NotFound(pid)) };
        match &mut list[i] {
            Block::Para(p) => Ok(p),
            _ => Err(EditError::NotParagraph(pid)),
        }
    }

    /// Citations in the text become footnotes, each holding its citation. A space before the citation goes,
    /// as the note's mark follows the word.
    fn citations_to_notes(&mut self) -> Result<usize, EditError> {
        let mut ids = Vec::new();
        walk_paragraphs(&self.body, &mut |p| ids.push(p.id));
        let mut n = 0;
        for pid in ids {
            loop {
                // Looked at first: a paragraph with no citation is not copied from an undo step.
                if !find_para(&self.body, pid).is_some_and(|p| p.inlines.iter().any(|i| matches!(i, Inline::Container(c) if cf::is_citation_sdt(c)))) {
                    break;
                }
                let (idx, sdt, off) = {
                    let p = self.para_mut(pid)?;
                    let Some(idx) = p.inlines.iter().rposition(|i| matches!(i, Inline::Container(c) if cf::is_citation_sdt(c))) else { break };
                    let sdt = p.inlines.remove(idx);
                    let mut at = idx;
                    // Drop one space just before the citation.
                    if idx > 0 {
                        if let Inline::Run(r) = &mut p.inlines[idx - 1] {
                            if let Some(RunItem::Text(t)) = r.items.last_mut() {
                                if t.ends_with(' ') {
                                    t.pop();
                                    if t.is_empty() {
                                        r.items.pop();
                                    }
                                }
                            }
                            if r.items.is_empty() {
                                p.inlines.remove(idx - 1);
                                at -= 1;
                            }
                        }
                    }
                    let off: usize = p.inlines[..at].iter().map(|i| i.len16()).sum();
                    (at, sdt, off)
                };
                let _ = idx;
                let made = self.op_insert_footnote(Pos::new(pid, off), "")?;
                let note_p = made.caret.map(|c| c.p.0).ok_or_else(|| bad("The note for the citation could not be made."))?;
                self.append_to_story_para(note_p, sdt)?;
                n += 1;
            }
        }
        if n > 0 {
            self.body_dirty = true;
            self.touch_all();
        }
        Ok(n)
    }

    /// Take a footnote out of the file (its mark in the text is dealt with by the caller).
    fn drop_note(&mut self, part: &str, id: &str) {
        self.stories.retain(|s| !(s.kind == StoryKind::Footnote && s.part == part && s.note_id.as_deref() == Some(id)));
        if let Some(doc) = self.story_parts.get_mut(part) {
            doc.root.children.retain(|c| !matches!(c, Node::Element(e) if e.name == "w:footnote" && e.attr("w:id") == Some(id)));
        }
        self.dirty_parts.insert(part.to_string());
        self.stories_touched = true;
    }

    /// Remove a citation. One that is all a footnote holds takes the footnote with it.
    fn op_remove_citation(&mut self, at: Pos) -> Result<OpResult, EditError> {
        if self.parked.is_some() {
            return Err(bad("Citations can only be removed from the main text."));
        }
        let p = self.find_any_para(at.p.0).ok_or(EditError::NotFound(at.p.0))?;
        let mut found = Vec::new();
        scan_inlines(&p.inlines, &mut 0, &mut found);
        let hit = found
            .iter()
            .find(|(o, len, _)| *o < at.o && at.o <= o + len)
            .or_else(|| found.iter().find(|(o, _, _)| *o == at.o))
            .cloned()
            .ok_or_else(|| bad("There is no citation at that position."))?;
        let in_body = find_block(&self.body, at.p.0).is_some();
        self.touch(at.p.0);
        let p = self.any_para_mut(at.p.0)?;
        remove_citation_at(&mut p.inlines, &mut 0, hit.0);
        let mut caret = hit.0;
        if !in_body {
            // A note with nothing left in it goes, and so does its mark in the text.
            let si = self.story_index(at.p.0).ok_or(EditError::NotFound(at.p.0))?;
            let story = &self.stories[si];
            let empty = story.kind == StoryKind::Footnote && matches!(story.blocks.as_slice(), [Block::Para(p)] if p.plain_text().trim().is_empty() && !p.inlines.iter().any(|i| matches!(i, Inline::Container(_))));
            if empty {
                let (part, id) = (story.part.clone(), story.note_id.clone().unwrap_or_default());
                let mut mark_in: Option<(NodeId, usize)> = None;
                self.body.edit_paragraphs(&mut |p| {
                    if mark_in.is_none() {
                        let mut off = 0;
                        if remove_note_mark(&mut p.inlines, &id, &mut off) {
                            mark_in = Some((p.id, off));
                            return true;
                        }
                    }
                    false
                });
                self.drop_note(&part, &id);
                self.body_dirty = true;
                self.touch_all();
                if let Some((pid, off)) = mark_in {
                    self.update_citations();
                    return Ok(OpResult { caret: Some(Pos::new(pid, off)), ..Default::default() });
                }
            }
            caret = 0;
        }
        self.update_citations();
        Ok(OpResult { caret: Some(Pos::new(at.p.0, caret)), ..Default::default() })
    }

    /// Footnotes that hold only a citation go back into the text as citations. Other notes stay.
    fn notes_to_citations(&mut self) -> usize {
        // A raised number sits against the word; other citations follow it after a space.
        let space = !matches!(self.resolved_style(), Resolved::Csl(s) if s.cites_superscript());
        let candidates: Vec<(String, String, Inline)> = self
            .stories
            .iter()
            .filter(|s| s.kind == StoryKind::Footnote)
            .filter_map(|s| {
                let id = s.note_id.clone()?;
                let [Block::Para(p)] = s.blocks.as_slice() else { return None };
                let mut sdt = None;
                for inl in &p.inlines {
                    match inl {
                        Inline::Container(c) if cf::is_citation_sdt(c) && sdt.is_none() => sdt = Some(inl.clone()),
                        // The note's own mark and spaces around the citation.
                        Inline::Run(r) if r.items.iter().all(|it| match it {
                            RunItem::Text(t) => t.trim().is_empty(),
                            RunItem::Object(e) => e.name == "w:footnoteRef",
                            _ => false,
                        }) => {}
                        _ => return None,
                    }
                }
                Some((id, s.part.clone(), sdt?))
            })
            .collect();
        let mut n = 0;
        for (id, part, sdt) in candidates {
            let mut slot = Some(sdt);
            let mut done = false;
            self.body.edit_paragraphs(&mut |p| {
                if !done {
                    done = swap_note_mark(&mut p.inlines, &id, &mut slot, space);
                    return done;
                }
                false
            });
            if !done {
                continue;
            }
            self.drop_note(&part, &id);
            n += 1;
        }
        if n > 0 {
            self.body_dirty = true;
            self.touch_all();
        }
        n
    }

    /// The offset just after the citation that starts at `o` in a paragraph.
    fn citation_end(&self, pid: NodeId, o: usize) -> Option<usize> {
        let p = self.find_any_para(pid)?;
        let mut found = Vec::new();
        scan_inlines(&p.inlines, &mut 0, &mut found);
        found.into_iter().find(|(s, _, _)| *s == o).map(|(s, len, _)| s + len)
    }

    fn op_edit_citation(&mut self, at: Pos, tags: &[String], page: Option<&str>, flags: [Option<bool>; 3]) -> Result<OpResult, EditError> {
        if self.parked.is_some() {
            return Err(bad("Citations can only be changed in the main text."));
        }
        let tags: Vec<String> = tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        self.check_tags_exist(&tags)?;
        let p = self.find_any_para(at.p.0).ok_or(EditError::NotFound(at.p.0))?;
        let mut found = Vec::new();
        scan_inlines(&p.inlines, &mut 0, &mut found);
        // The citation the position is inside of, or at the end of, or at the start of.
        let hit = found
            .iter()
            .find(|(o, len, _)| *o < at.o && at.o <= o + len)
            .or_else(|| found.iter().find(|(o, _, _)| *o == at.o))
            .cloned()
            .ok_or_else(|| bad("There is no citation at that position."))?;
        let mut ci = CiteInstr::parse(&hit.2).ok_or_else(|| bad("That citation's field could not be read."))?;
        if !tags.is_empty() {
            ci.tags = tags;
        }
        if let Some(pg) = page {
            ci.page = Some(pg.trim().to_string()).filter(|p| !p.is_empty());
        }
        if let Some(v) = flags[0] {
            ci.no_author = v;
        }
        if let Some(v) = flags[1] {
            ci.no_year = v;
        }
        if let Some(v) = flags[2] {
            ci.no_title = v;
        }
        let text = ci.build();
        self.touch(at.p.0);
        let p = self.any_para_mut(at.p.0)?;
        edit_citation_at(&mut p.inlines, &mut 0, hit.0, &mut |c| {
            cf::set_first_instr(&mut c.children, &text);
        });
        self.update_citations();
        let caret = self.citation_end(at.p.0, hit.0).unwrap_or(hit.0);
        Ok(OpResult { caret: Some(Pos::new(at.p.0, caret)), ..Default::default() })
    }

    fn op_insert_bibliography(&mut self, after: Option<NodeId>, before: Option<NodeId>, heading: Option<&str>) -> Result<OpResult, EditError> {
        if self.parked.is_some() {
            return Err(bad("A bibliography can only be inserted in the main text."));
        }
        if self.block_lists().into_iter().any(cf::has_bibliography) {
            return Err(bad("The document already has a bibliography. Update it instead."));
        }
        let (indent, default_heading) = self.bibliography_layout();
        let entry_style = self.styles.ensure_builtin("Bibliography").unwrap_or_else(|| "Bibliography".into());
        let heading = match heading {
            None => Some(default_heading),
            Some(h) => Some(h.trim().to_string()).filter(|h| !h.is_empty()),
        };
        if heading.is_some() {
            self.styles.ensure_builtin("Heading1");
        }
        let id = self.next_sdt_id();
        let ppr = cf::entry_ppr(Some(indent), &entry_style);
        let el = cf::new_bibliography(id, heading.as_deref(), &ppr);
        let block = parse_block(&el, &mut self.ids);
        self.insert_blocks(after, before, vec![block])?;
        // Fills in the entries; they are new paragraphs, so report every paragraph inside the control.
        self.update_citations();
        let mut created = Vec::new();
        if let Some(c) = find_container(&self.body, id) {
            walk_paragraphs(&c.blocks, &mut |p| created.push(p.id));
        }
        self.touch_all();
        Ok(OpResult { created, ..Default::default() })
    }
}
