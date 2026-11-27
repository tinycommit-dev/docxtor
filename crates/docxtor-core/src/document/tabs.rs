//! Document tabs on the document: the ops that add, rename, delete, copy and move tabs, and the checks that keep the
//! body in step with the tab list (see `crate::tabs` for how a tabbed document is stored).
//!
//! To change the tabs the body is cut into one chunk per tab, the chunks are changed, and the body is joined again. The
//! section properties that end a tab (the last paragraph of the tab carries them, the last tab's are the body's own)
//! travel with their chunk, so a tab keeps its page setup when it moves.

use super::*;
use crate::marks;
use crate::tabs::{self, marker_name, Tab, TabInfo, TabList, TabOp, TabsInfo, MAX_DEPTH, MAX_NAME};
use crate::wordcount::{Counts, TextStats};
use std::collections::HashSet;
use std::ops::Range;

struct Chunk {
    tab: Tab,
    blocks: Vec<Block>,
    /// The section properties that end the tab.
    sect: Option<Element>,
}

/// The body cut into tabs.
struct Split {
    chunks: Vec<Chunk>,
    /// The blocks after the content (the body's own section properties).
    tail: Vec<Block>,
    /// The id the next new tab takes.
    next: u32,
    /// The largest bookmark id in the document.
    bookmark: i64,
}

fn is_sect(b: &Block) -> bool {
    matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")
}

/// How many blocks the content has: all but the closing section properties.
fn content_len(body: &Body) -> usize {
    body.iter().rposition(|b| !is_sect(b)).map_or(0, |i| i + 1)
}

fn is_content(b: &Block) -> bool {
    matches!(b, Block::Para(_) | Block::Table(_) | Block::Container(_))
}

fn marker_in_inlines(inlines: &[Inline]) -> Option<String> {
    for i in inlines {
        match i {
            Inline::Opaque(e) if e.name == "w:bookmarkStart" => {
                if let Some(id) = e.attr("w:name").and_then(tabs::marker_id) {
                    return Some(id.to_string());
                }
            }
            Inline::Container(c) => {
                if let Some(id) = marker_in_inlines(&c.children) {
                    return Some(id);
                }
            }
            _ => {}
        }
    }
    None
}

/// The tab whose start marker a block holds, wherever inside the block it is.
fn marker_in_block(b: &Block) -> Option<String> {
    match b {
        Block::Para(p) => marker_in_inlines(&p.inlines),
        Block::Table(t) => t.rows().flat_map(|r| r.cells()).find_map(|c| c.blocks.iter().find_map(marker_in_block)),
        Block::Container(c) => c.blocks.iter().find_map(marker_in_block),
        Block::Opaque(_) => None,
    }
}

/// The last content block before position `at`: the one that ends the tab before a tab starting there.
fn closing_before(body: &Body, at: usize) -> Option<usize> {
    body.iter().take(at).rposition(is_content)
}

fn first_para_mut(b: &mut Block) -> Option<&mut Paragraph> {
    match b {
        Block::Para(p) => Some(p),
        Block::Table(t) => t.rows_mut().flat_map(|r| r.cells_mut()).find_map(|c| c.blocks.iter_mut().find_map(first_para_mut)),
        Block::Container(c) => c.blocks.iter_mut().find_map(first_para_mut),
        Block::Opaque(_) => None,
    }
}

fn para_sect(p: &Paragraph) -> Option<&Element> {
    p.ppr.as_ref().and_then(|pp| pp.child("w:sectPr"))
}

fn take_para_sect(p: &mut Paragraph) -> Option<Element> {
    let pp = p.ppr.as_mut()?;
    let at = pp.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "w:sectPr"))?;
    let Node::Element(sect) = pp.children.remove(at) else { return None };
    if pp.children.is_empty() && pp.attrs.is_empty() {
        p.ppr = None;
    }
    Some(sect)
}

fn put_para_sect(p: &mut Paragraph, sect: Element) {
    put_ppr(p.ppr.get_or_insert_with(|| Element::new("w:pPr")), sect);
}

/// Take the start markers of one tab (`id`) out of these blocks, with the ends that belong to them.
fn remove_markers(blocks: &mut Vec<Block>, id: &str) {
    let name = marker_name(id);
    fn strip_starts(list: &mut Vec<Inline>, name: &str, ends: &mut HashSet<String>) {
        list.retain(|i| match i {
            Inline::Opaque(e) if e.name == "w:bookmarkStart" && e.attr("w:name") == Some(name) => {
                ends.extend(e.attr("w:id").map(String::from));
                false
            }
            _ => true,
        });
        for i in list.iter_mut() {
            if let Inline::Container(c) = i {
                strip_starts(&mut c.children, name, ends);
            }
        }
    }
    fn strip_ends(list: &mut Vec<Inline>, ends: &HashSet<String>) {
        list.retain(|i| !matches!(i, Inline::Opaque(e) if e.name == "w:bookmarkEnd" && e.attr("w:id").is_some_and(|x| ends.contains(x))));
        for i in list.iter_mut() {
            if let Inline::Container(c) = i {
                strip_ends(&mut c.children, ends);
            }
        }
    }
    let mut ends = HashSet::new();
    marks::each_list_mut(blocks, &mut |list| {
        for b in list.iter_mut() {
            if let Block::Para(p) = b {
                strip_starts(&mut p.inlines, &name, &mut ends);
            }
        }
    });
    if ends.is_empty() {
        return;
    }
    marks::each_list_mut(blocks, &mut |list| {
        for b in list.iter_mut() {
            if let Block::Para(p) = b {
                strip_ends(&mut p.inlines, &ends);
            }
        }
    });
}

/// Put a tab's start marker (a point bookmark) at the start of a paragraph.
fn add_marker(p: &mut Paragraph, id: &str, bookmark: i64) {
    let start = Element::new("w:bookmarkStart").with_attr("w:id", bookmark.to_string()).with_attr("w:name", marker_name(id));
    let end = Element::new("w:bookmarkEnd").with_attr("w:id", bookmark.to_string());
    p.inlines.insert(0, Inline::Opaque(end));
    p.inlines.insert(0, Inline::Opaque(start));
}

/// Section properties for a tab made from a neighbour's: the same page, headers and footers, but not a title page,
/// a restarted page number or a section type other than the next page.
fn blank_sect(from: Option<&Element>) -> Element {
    let mut e = from.cloned().unwrap_or_else(|| Element::new("w:sectPr"));
    e.remove_children("w:type");
    e.remove_children("w:titlePg");
    if let Some(n) = e.child_mut("w:pgNumType") {
        n.remove_attr("w:start");
    }
    e
}

fn name_of(raw: &str) -> Result<String, EditError> {
    let name = raw.trim();
    if name.chars().count() > MAX_NAME {
        return Err(EditError::Invalid(format!("A tab name can be at most {MAX_NAME} characters long.")));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(EditError::Invalid("A tab name can't have line breaks or other control characters.".into()));
    }
    Ok(name.to_string())
}

fn icon_of(raw: Option<&str>) -> Result<Option<String>, EditError> {
    let icon = raw.map(str::trim).filter(|i| !i.is_empty());
    if let Some(i) = icon {
        if i.chars().count() > 16 || i.chars().any(|c| c.is_control() || c.is_ascii_alphanumeric()) {
            return Err(EditError::Invalid("A tab's icon is one emoji.".into()));
        }
    }
    Ok(icon.map(String::from))
}

fn no_tab(id: &str) -> EditError {
    EditError::Invalid(format!("There is no tab '{id}'."))
}

fn list_of(split: &Split) -> TabList {
    TabList { tabs: split.chunks.iter().map(|c| c.tab.clone()).collect(), next: split.next }
}

impl Document {
    // -----------------------------------------------------------------------
    // Looking at the tabs
    // -----------------------------------------------------------------------

    /// The tabs that have a start marker in the body, in order, as (position in the list, first block). The first tab
    /// always starts the body.
    fn tab_layout(&self) -> Vec<(usize, usize)> {
        let Some(list) = self.tabs.list.as_ref() else { return Vec::new() };
        let body = self.main_body();
        let n = content_len(body);
        let mut at: HashMap<String, usize> = HashMap::new();
        for (i, b) in body.iter().take(n).enumerate() {
            if let Some(id) = marker_in_block(b) {
                at.entry(id).or_insert(i);
            }
        }
        let mut out: Vec<(usize, usize)> = Vec::new();
        for (k, t) in list.tabs.iter().enumerate() {
            let start = if k == 0 {
                0
            } else {
                match at.get(&t.id) {
                    Some(&i) if out.last().is_none_or(|(_, l)| i > *l) => i,
                    _ => continue,
                }
            };
            out.push((k, start));
        }
        out
    }

    /// Say which tab to write into the file at the next save: the one the editor shows. Not an edit (no op, no undo step,
    /// no change to what is saved except that one attribute). A tab the document does not have is refused.
    pub fn set_open_tab(&mut self, id: &str) -> Result<(), EditError> {
        let list = self.tabs.list.as_ref().ok_or_else(|| EditError::Invalid("This document has no tabs.".into()))?;
        if list.index(id).is_none() {
            return Err(no_tab(id));
        }
        self.tabs.set_open(id);
        Ok(())
    }

    /// The tabs, with where each starts, and `active`: the tab the file was saved on (or the last one `set_open_tab`
    /// named), which a pane shows when it has no tab of its own yet. No tabs for a document that has none.
    pub fn tabs_info(&self) -> TabsInfo {
        let Some(list) = self.tabs.list.as_ref() else { return TabsInfo::default() };
        let body = self.main_body();
        let n = content_len(body);
        let layout = self.tab_layout();
        let depths = list.depths();
        let tabs: Vec<TabInfo> = layout
            .iter()
            .enumerate()
            .map(|(i, &(k, start))| {
                let end = layout.get(i + 1).map_or(n, |&(_, s)| s);
                let t = &list.tabs[k];
                // A tab whose parent has no marker any more is shown at the top level.
                let parent = t.parent.clone().filter(|p| layout.iter().any(|&(pk, _)| list.tabs[pk].id == *p));
                let mut ids: Vec<NodeId> = Vec::new();
                for b in body.iter().skip(start.min(end)).take(end - start.min(end)) {
                    match b {
                        _ if !crate::render::is_visible(b) => {}
                        Block::Container(c) if is_alt_chunk(c) => ids.extend(c.blocks.iter().map(|x| x.id())),
                        _ => ids.push(b.id()),
                    }
                }
                TabInfo { id: t.id.clone(), name: t.name.clone(), icon: t.icon.clone(), depth: if parent.is_some() { depths[k] } else { 0 }, parent, start: body.get(start).map_or(0, |b| b.id()), blocks: end.saturating_sub(start), ids }
            })
            .collect();
        // The tab asked for, else the first as the tabs stand in the text (not as the list has them: the two orders differ after a move,
        // and a reopened file must give the same answer).
        let active = self.tabs.asked().filter(|a| tabs.iter().any(|t| t.id == *a)).or_else(|| tabs.first().map(|t| t.id.as_str())).map(String::from);
        TabsInfo { tabs, active }
    }

    /// The positions in the body of the blocks of a tab.
    pub fn tab_range(&self, id: &str) -> Option<Range<usize>> {
        let list = self.tabs.list.as_ref()?;
        let layout = self.tab_layout();
        let n = content_len(self.main_body());
        let i = layout.iter().position(|&(k, _)| list.tabs[k].id == id)?;
        Some(layout[i].1..layout.get(i + 1).map_or(n, |&(_, s)| s))
    }

    /// The tab a top-level block is in.
    pub fn tab_of_block(&self, block: NodeId) -> Option<String> {
        let list = self.tabs.list.as_ref()?;
        let pos = self.main_body().iter().position(|b| b.id() == block)?;
        let layout = self.tab_layout();
        let i = layout.iter().rposition(|&(_, s)| s <= pos)?;
        Some(list.tabs[layout[i].0].id.clone())
    }

    /// Paragraphs, words and characters of one tab's text, and of the notes its text points at. Text boxes are left
    /// out. None when the tab isn't there.
    pub fn text_stats_of_tab(&self, id: &str) -> Option<TextStats> {
        let range = self.tab_range(id)?;
        let blocks = self.main_body().iter().skip(range.start).take(range.len());
        let mut out = TextStats::default();
        let mut refs: Vec<(bool, String)> = Vec::new();
        fn note_refs(inlines: &[Inline], out: &mut Vec<(bool, String)>) {
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
                    Inline::Container(c) => note_refs(&c.children, out),
                    Inline::Opaque(_) => {}
                }
            }
        }
        let mut count = Counts::default();
        walk_paragraphs(blocks, &mut |p| {
            let text = p.plain_text();
            count = count.plus(&Counts { paragraphs: 1, words: text.split_whitespace().count(), chars: text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\u{b}')).count(), chars_no_spaces: text.chars().filter(|c| !c.is_whitespace()).count() });
            note_refs(&p.inlines, &mut refs);
        });
        out.main = count;
        for s in &self.stories {
            let kind_match = refs.iter().any(|(foot, id)| s.note_id.as_deref() == Some(id.as_str()) && ((*foot && s.kind == StoryKind::Footnote) || (!*foot && s.kind == StoryKind::Endnote)));
            if kind_match {
                let mut c = Counts::default();
                walk_paragraphs(&s.blocks, &mut |p| {
                    let text = p.plain_text();
                    c = c.plus(&Counts { paragraphs: 1, words: text.split_whitespace().count(), chars: text.chars().filter(|c| !matches!(c, '\r' | '\n' | '\u{b}')).count(), chars_no_spaces: text.chars().filter(|c| !c.is_whitespace()).count() });
                });
                out.notes = out.notes.plus(&c);
            }
        }
        Some(out)
    }

    // -----------------------------------------------------------------------
    // Cutting the body into tabs and joining it again
    // -----------------------------------------------------------------------

    fn max_bookmark_id(&self) -> i64 {
        self.bookmark_ids_and_names().0.into_iter().max().unwrap_or(0)
    }

    /// Cut the body into one chunk per tab. A tab whose start marker is gone is dropped from the list (its text is
    /// part of the tab before it).
    fn split_body(&mut self) -> Result<Split, EditError> {
        let list = self.tabs.list.clone().ok_or_else(|| EditError::Invalid("This document has no tabs.".into()))?;
        let layout = self.tab_layout();
        let bookmark = self.max_bookmark_id();
        let mut body = std::mem::take(&mut self.body);
        let n = content_len(&body);
        let mut tail = body.split_off(n).into_vec();
        let mut body = body.into_vec();
        let mut rest: Vec<(Tab, Vec<Block>)> = Vec::new();
        for &(k, start) in layout.iter().rev() {
            rest.push((list.tabs[k].clone(), body.split_off(start)));
        }
        rest.reverse();
        let last = rest.len().saturating_sub(1);
        let mut chunks: Vec<Chunk> = Vec::new();
        for (i, (tab, mut blocks)) in rest.into_iter().enumerate() {
            let sect = if i < last {
                // The paragraph that ends the tab is its last content block: a bookmark end or another invisible
                // element after it (Word writes these) is not the end of the tab.
                match blocks.iter_mut().rev().find(|b| is_content(b)) {
                    Some(Block::Para(p)) => take_para_sect(p),
                    _ => None,
                }
            } else {
                let at = tail.iter().position(is_sect);
                at.and_then(|a| match &tail[a] {
                    Block::Opaque(o) => Some(o.el.clone()),
                    _ => None,
                })
            };
            chunks.push(Chunk { tab, blocks, sect });
        }
        // The closing section properties are written back from the last chunk.
        tail.retain(|b| !is_sect(b));
        Ok(Split { chunks, tail, next: list.next, bookmark })
    }

    /// A document without tabs as one chunk, for adding the first.
    fn whole_body_split(&mut self, name: &str) -> Split {
        let bookmark = self.max_bookmark_id();
        let mut list = TabList::default();
        let id = list.fresh_id();
        let mut body = std::mem::take(&mut self.body);
        let n = content_len(&body);
        let mut tail = body.split_off(n).into_vec();
        let body = body.into_vec();
        let sect = tail.iter().find_map(|b| match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => Some(o.el.clone()),
            _ => None,
        });
        tail.retain(|b| !is_sect(b));
        let name = if name.trim().is_empty() { "Tab 1".to_string() } else { name.trim().to_string() };
        Split { chunks: vec![Chunk { tab: Tab { id, name, icon: None, parent: None }, blocks: body, sect }], tail, next: list.next, bookmark }
    }

    /// Put the body together from the chunks, and keep the tab list in step. Every tab but the last ends with a
    /// paragraph that carries its section properties; each starts on a new page and has its marker on its first block.
    fn join_body(&mut self, split: Split) -> Result<(), EditError> {
        let Split { mut chunks, mut tail, next, mut bookmark } = split;
        if chunks.is_empty() {
            return Err(EditError::Invalid("A document needs at least one tab.".into()));
        }
        // Parents that are gone, and nesting that goes too deep, become top-level tabs.
        let mut seen: Vec<String> = Vec::new();
        let mut depths: Vec<usize> = Vec::new();
        for c in chunks.iter_mut() {
            let p = c.tab.parent.clone().and_then(|p| seen.iter().position(|s| *s == p));
            let d = p.map_or(0, |i| depths[i] + 1);
            if p.is_none() || d >= MAX_DEPTH {
                c.tab.parent = None;
            }
            depths.push(if c.tab.parent.is_some() { d } else { 0 });
            seen.push(c.tab.id.clone());
        }
        let tab_list: Vec<Tab> = chunks.iter().map(|c| c.tab.clone()).collect();
        let template = chunks.last().and_then(|c| c.sect.clone()).or_else(|| chunks.iter().find_map(|c| c.sect.clone()));
        let single = chunks.len() == 1;
        let n = chunks.len();
        let mut body: Vec<Block> = Vec::new();
        let mut closing: Option<Element> = None;
        for (i, mut c) in chunks.drain(..).enumerate() {
            if c.blocks.iter().all(|b| !is_content(b)) {
                c.blocks.push(Block::Para(Paragraph::new(self.ids.fresh())));
            }
            if single {
                remove_markers(&mut c.blocks, &c.tab.id);
            } else {
                let first = c.blocks.iter().position(is_content).unwrap_or(0);
                if marker_in_block(&c.blocks[first]).as_deref() != Some(c.tab.id.as_str()) {
                    remove_markers(&mut c.blocks, &c.tab.id);
                    bookmark += 1;
                    if let Some(p) = first_para_mut(&mut c.blocks[first]) {
                        add_marker(p, &c.tab.id, bookmark);
                    }
                }
            }
            let had_sect = c.sect.is_some();
            let mut sect = c.sect.take().unwrap_or_else(|| blank_sect(template.as_ref()));
            if i + 1 < n {
                // The tab ends with a paragraph that carries the section break.
                let last = c.blocks.iter().rposition(is_content);
                if !matches!(last.map(|l| &c.blocks[l]), Some(Block::Para(_))) {
                    c.blocks.push(Block::Para(Paragraph::new(self.ids.fresh())));
                }
            }
            if i > 0 {
                // The section that starts the tab starts on a new page: its own break, or the tab's closing one.
                let inner = c.blocks.iter_mut().find_map(|b| match b {
                    Block::Para(p) if para_sect(p).is_some() => p.ppr.as_mut().and_then(|pp| pp.child_mut("w:sectPr")),
                    _ => None,
                });
                match inner {
                    Some(s) => {
                        s.remove_children("w:type");
                    }
                    None => {
                        sect.remove_children("w:type");
                    }
                }
            }
            if i + 1 < n {
                if let Some(Block::Para(p)) = c.blocks.iter_mut().rev().find(|b| is_content(b)) {
                    // Section properties the closing paragraph already has are the tab's own: not replaced by a made-up copy.
                    if had_sect || para_sect(p).is_none() {
                        put_para_sect(p, sect);
                    }
                }
            } else {
                closing = Some(sect);
            }
            body.append(&mut c.blocks);
        }
        let closing = closing.unwrap_or_else(|| Element::new("w:sectPr"));
        let sect_id = tail.iter().find(|b| is_sect(b)).map(|b| b.id()).unwrap_or_else(|| self.ids.fresh());
        body.append(&mut tail);
        body.push(Block::Opaque(Opaque { id: sect_id, el: closing }));
        self.body = Body::from(body);
        self.touch_all();
        // A document with one tab left is a plain document again.
        self.tabs.set(if tab_list.len() <= 1 { None } else { Some(TabList { tabs: tab_list, next }) });
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Ops
    // -----------------------------------------------------------------------

    pub(super) fn op_tab(&mut self, op: &TabOp) -> Result<OpResult, EditError> {
        if self.parked.is_some() {
            return Err(EditError::Invalid("Tabs can only be changed from the main text.".into()));
        }
        match op {
            TabOp::Add { name, icon, parent, after, first_name, open: _ } => self.tab_add(name.as_deref(), icon.as_deref(), parent.as_deref(), after.as_deref(), first_name.as_deref()),
            TabOp::Rename { id, name } => {
                let name = name_of(name)?;
                self.edit_tab_list(id, |t| t.name = name)
            }
            TabOp::SetIcon { id, icon } => {
                let icon = icon_of(icon.as_deref())?;
                self.edit_tab_list(id, |t| t.icon = icon)
            }
            TabOp::Delete { id } => self.tab_delete(id),
            TabOp::Duplicate { id, name } => self.tab_duplicate(id, name.as_deref()),
            TabOp::Move { id, parent, before } => self.tab_move(id, parent.as_deref(), before.as_deref()),
        }
    }

    fn edit_tab_list(&mut self, id: &str, f: impl FnOnce(&mut Tab)) -> Result<OpResult, EditError> {
        let mut list = (**self.tabs.list.as_ref().ok_or_else(|| EditError::Invalid("This document has no tabs.".into()))?).clone();
        let i = list.index(id).ok_or_else(|| no_tab(id))?;
        f(&mut list.tabs[i]);
        self.tabs.set(Some(list));
        Ok(OpResult::default())
    }

    fn tab_add(&mut self, name: Option<&str>, icon: Option<&str>, parent: Option<&str>, after: Option<&str>, first_name: Option<&str>) -> Result<OpResult, EditError> {
        let icon = icon_of(icon)?;
        let name = name.map(name_of).transpose()?.filter(|n| !n.is_empty());
        let first_name = first_name.map(name_of).transpose()?.unwrap_or_default();
        // Check what is asked before the body is taken apart.
        if let Some(list) = self.tabs.list.as_ref() {
            for t in [parent, after].into_iter().flatten() {
                list.index(t).ok_or_else(|| no_tab(t))?;
            }
        } else if parent.is_some() || after.is_some() {
            return Err(EditError::Invalid("This document has no tabs yet; add one without a parent.".into()));
        }
        let mut split = if self.tabs.list.is_some() { self.split_body()? } else { self.whole_body_split(&first_name) };
        let mut list = list_of(&split);
        let depths = list.depths();
        let (pos, parent_id) = match (parent, after) {
            (Some(p), _) => {
                let pi = list.index(p).ok_or_else(|| no_tab(p))?;
                if depths[pi] + 1 >= MAX_DEPTH {
                    return Err(EditError::Invalid(format!("Tabs nest at most {MAX_DEPTH} levels deep.")));
                }
                (list.subtree_end(pi), Some(p.to_string()))
            }
            (None, Some(a)) => {
                let ai = list.index(a).ok_or_else(|| no_tab(a))?;
                (list.subtree_end(ai), list.tabs[ai].parent.clone())
            }
            (None, None) => (list.tabs.len(), None),
        };
        let id = list.fresh_id();
        split.next = list.next;
        let name = name.unwrap_or_else(|| format!("Tab {}", list.tabs.len() + 1));
        let sect = blank_sect(split.chunks[pos - 1].sect.as_ref());
        let para = Paragraph::new(self.ids.fresh());
        let pid = para.id;
        split.chunks.insert(pos, Chunk { tab: Tab { id: id.clone(), name, icon, parent: parent_id }, blocks: vec![Block::Para(para)], sect: Some(sect) });
        self.join_body(split)?;
        Ok(OpResult { caret: Some(Pos::new(pid, 0)), created: vec![pid], ..Default::default() })
    }

    fn tab_delete(&mut self, id: &str) -> Result<OpResult, EditError> {
        let list = self.tabs.list.clone().ok_or_else(|| EditError::Invalid("This document has no tabs.".into()))?;
        let i = list.index(id).ok_or_else(|| no_tab(id))?;
        let end = list.subtree_end(i);
        if end - i >= list.tabs.len() {
            return Err(EditError::Invalid("A document needs at least one tab. Delete the text instead.".into()));
        }
        let mut split = self.split_body()?;
        if !split.chunks.iter().any(|c| c.tab.id == id) {
            return Err(no_tab(id));
        }
        let gone: HashSet<&str> = list.tabs[i..end].iter().map(|t| t.id.as_str()).collect();
        split.chunks.retain(|c| !gone.contains(c.tab.id.as_str()));
        if split.chunks.is_empty() {
            return Err(EditError::Invalid("A document needs at least one tab. Delete the text instead.".into()));
        }
        self.join_body(split)?;
        Ok(OpResult::default())
    }

    fn tab_duplicate(&mut self, id: &str, name: Option<&str>) -> Result<OpResult, EditError> {
        let list = self.tabs.list.clone().ok_or_else(|| EditError::Invalid("This document has no tabs.".into()))?;
        let i = list.index(id).ok_or_else(|| no_tab(id))?;
        let range = self.tab_range(id).ok_or_else(|| no_tab(id))?;
        // The tab's text as a fragment, which brings its styles, lists, pictures and notes along.
        let order = paragraph_order(self.body.iter().skip(range.start).take(range.len()));
        let frag = match (order.first(), order.last()) {
            (Some(&a), Some(&b)) => {
                let len = self.para_ref(b)?.len16();
                self.copy_fragment(Pos::new(a, 0), Pos::new(b, len))
            }
            _ => None,
        };
        let mut blocks = match frag {
            Some(f) => self.fragment_blocks(&f)?,
            None => Vec::new(),
        };
        if blocks.is_empty() {
            blocks.push(Block::Para(Paragraph::new(self.ids.fresh())));
        }
        let name = match name.map(name_of).transpose()?.filter(|n| !n.is_empty()) {
            Some(n) => n,
            // The default name is cut to the limit a name may have (a tab named with 100 characters can be copied).
            None => format!("Copy of {}", list.tabs[i].name).chars().take(MAX_NAME).collect(),
        };
        let mut split = self.split_body()?;
        let mut list = list_of(&split);
        let at = list.index(id).ok_or_else(|| no_tab(id))?;
        let pos = list.subtree_end(at);
        let new_id = list.fresh_id();
        split.next = list.next;
        let src = &list.tabs[at];
        let tab = Tab { id: new_id.clone(), name, icon: src.icon.clone(), parent: src.parent.clone() };
        let sect = blank_sect(split.chunks[at].sect.as_ref());
        let created: Vec<NodeId> = blocks.iter().map(|b| b.id()).collect();
        split.chunks.insert(pos, Chunk { tab, blocks, sect: Some(sect) });
        self.join_body(split)?;
        Ok(OpResult { created, ..Default::default() })
    }

    fn tab_move(&mut self, id: &str, parent: Option<&str>, before: Option<&str>) -> Result<OpResult, EditError> {
        let mut split = self.split_body()?;
        let list = list_of(&split);
        let i = list.index(id).ok_or_else(|| no_tab(id))?;
        let end = list.subtree_end(i);
        let depths = list.depths();
        if let Some(p) = parent {
            let pi = list.index(p).ok_or_else(|| no_tab(p))?;
            if (i..end).contains(&pi) {
                return Err(EditError::Invalid("A tab can't be moved into itself or its own subtabs.".into()));
            }
            if depths[pi] + 1 + list.height(i) >= MAX_DEPTH {
                return Err(EditError::Invalid(format!("Tabs nest at most {MAX_DEPTH} levels deep.")));
            }
        } else if list.height(i) >= MAX_DEPTH {
            return Err(EditError::Invalid(format!("Tabs nest at most {MAX_DEPTH} levels deep.")));
        }
        if let Some(b) = before {
            let bi = list.index(b).ok_or_else(|| no_tab(b))?;
            if (i..end).contains(&bi) {
                return Err(EditError::Invalid("A tab can't be moved before itself.".into()));
            }
            if list.tabs[bi].parent.as_deref() != parent {
                return Err(EditError::Invalid(format!("'{b}' isn't one of the tabs in the place you are moving '{id}' to.")));
            }
        }
        let mut moved: Vec<Chunk> = split.chunks.drain(i..end).collect();
        moved[0].tab.parent = parent.map(String::from);
        let rest = list_of(&split);
        let pos = match before {
            Some(b) => rest.index(b).ok_or_else(|| no_tab(b))?,
            None => match parent {
                Some(p) => rest.subtree_end(rest.index(p).ok_or_else(|| no_tab(p))?),
                None => rest.tabs.len(),
            },
        };
        for (k, c) in moved.into_iter().enumerate() {
            split.chunks.insert(pos + k, c);
        }
        self.join_body(split)?;
        Ok(OpResult::default())
    }

    // -----------------------------------------------------------------------
    // Keeping the body and the list in step
    // -----------------------------------------------------------------------

    /// Does the body still match the tab list: every tab's marker is there, in order, and every tab after the first
    /// starts right after a paragraph that ends a section?
    pub(super) fn tabs_structure_ok(&self) -> bool {
        let Some(list) = self.tabs.list.as_ref() else { return true };
        let body = self.main_body();
        let n = content_len(body);
        let layout = self.tab_layout();
        if layout.len() != list.tabs.len() {
            return false;
        }
        // The first tab starts where the body does, so its marker (when it has one) must sit on the first block: a marker
        // left in the middle of another tab would be found first, and the tab list would start there.
        let first_marker_ok = list.tabs.len() < 2 || {
            let first = body.iter().position(is_content);
            body.iter().take(n).position(|b| marker_in_block(b).as_deref() == Some(list.tabs[0].id.as_str())).is_none_or(|at| Some(at) == first)
        };
        // The paragraph that ends the tab before is the last content block ahead of the start (invisible elements such as a
        // bookmark end may sit between it and the first block).
        first_marker_ok && layout.iter().all(|&(k, start)| k == 0 || (start > 0 && start < n && closing_before(body, start).is_some_and(|j| matches!(&body[j], Block::Para(p) if para_sect(p).is_some()))))
    }

    /// A tab whose first block was deleted or replaced took its marker with it. The tab is still there (its section
    /// break is), so the marker goes on the first block of the tab that is left: the first block after the old start
    /// that is still in the body, before the next tab begins. When nothing of the tab is left (its whole text, break
    /// included, went) there is nothing to put it on and the batch is refused as before.
    fn tabs_restore_markers(&mut self, list: &TabList, before: &[Block]) {
        let layout = self.tab_layout();
        let present: HashSet<usize> = layout.iter().map(|&(k, _)| k).collect();
        if present.len() == list.tabs.len() {
            return;
        }
        for (k, tab) in list.tabs.iter().enumerate() {
            if k == 0 || present.contains(&k) {
                continue;
            }
            let Some(old) = before.iter().position(|b| marker_in_block(b).as_deref() == Some(tab.id.as_str())) else { continue };
            // Where the tabs around it start now: the survivor has to be after the one before and ahead of the one after.
            let layout = self.tab_layout();
            let after_prev = layout.iter().rev().find(|&&(pk, _)| pk < k).map_or(0, |&(_, s)| s);
            let next_start = layout.iter().find(|&&(nk, _)| nk > k).map_or(content_len(&self.body), |&(_, s)| s);
            let spot = before[old + 1..].iter().find_map(|b| {
                let j = self.body.iter().position(|x| x.id() == b.id())?;
                (j > after_prev && j < next_start && is_content(&self.body[j])).then_some(j)
            });
            let Some(j) = spot else { continue };
            let bookmark = self.max_bookmark_id() + 1;
            if let Some(p) = first_para_mut(&mut self.body[j]) {
                add_marker(p, &tab.id, bookmark);
                let id = self.body[j].id();
                self.changed.insert(id);
            }
        }
    }

    /// A tab's marker can drift when text is split or joined at the start of its first paragraph: move it back to the
    /// first block after the section break that ends the tab before.
    fn tabs_repair(&mut self, before: Option<&[Block]>) {
        let Some(list) = self.tabs.list.clone() else { return };
        if let Some(before) = before {
            self.tabs_restore_markers(&list, before);
        }
        let layout = self.tab_layout();
        let mut moves: Vec<(usize, usize, String)> = Vec::new();
        for &(k, start) in &layout {
            let want = if k == 0 {
                self.body.iter().position(is_content).unwrap_or(0)
            } else {
                match self.body.iter().take(start).rposition(|b| matches!(b, Block::Para(p) if para_sect(p).is_some())) {
                    // The first content block after the break (invisible elements may sit between).
                    Some(j) => self.body.iter().skip(j + 1).position(is_content).map_or(j + 1, |d| j + 1 + d),
                    None => continue,
                }
            };
            if want < start && self.body.get(want).is_some_and(is_content) {
                moves.push((start, want, list.tabs[k].id.clone()));
            }
        }
        // The first tab's marker went along with a block that moved into another tab: put it on the first block again.
        if list.tabs.len() > 1 {
            let id = list.tabs[0].id.clone();
            let first = self.body.iter().position(is_content);
            let at = self.body.iter().position(|b| marker_in_block(b).as_deref() == Some(id.as_str()));
            if let (Some(at), Some(first)) = (at, first) {
                if at != first {
                    if let Some(bookmark) = marks_of(&self.body[at], &id) {
                        let mut one = vec![self.body.remove(at)];
                        remove_markers(&mut one, &id);
                        if let Some(b) = one.pop() {
                            self.body.insert(at, b);
                        }
                        if let Some(p) = first_para_mut(&mut self.body[first]) {
                            add_marker(p, &id, bookmark);
                        }
                        let (f, t) = (self.body[at].id(), self.body[first].id());
                        self.changed.insert(f);
                        self.changed.insert(t);
                    }
                }
            }
        }
        for (from, to, id) in moves {
            let Some(bookmark) = marks_of(&self.body[from], &id) else { continue };
            let mut one = vec![self.body.remove(from)];
            remove_markers(&mut one, &id);
            if let Some(b) = one.pop() {
                self.body.insert(from, b);
            }
            if let Some(p) = first_para_mut(&mut self.body[to]) {
                add_marker(p, &id, bookmark);
            }
            let (f, t) = (self.body[from].id(), self.body[to].id());
            self.changed.insert(f);
            self.changed.insert(t);
        }
    }

    /// After a batch of ops: the body must still match the tab list. Small drifts are put right.
    pub(super) fn tabs_hold(&mut self, before: &[std::sync::Arc<Block>]) -> Result<(), EditError> {
        if self.tabs_structure_ok() {
            return Ok(());
        }
        let before: Vec<Block> = before.iter().map(|b| (**b).clone()).collect();
        self.tabs_repair(Some(&before));
        if self.tabs_structure_ok() {
            return Ok(());
        }
        Err(EditError::Invalid("That would join two tabs or remove one with all its text. Delete the tab from the tab list instead.".into()))
    }

    /// Is the structure worth checking after these ops: they could move or remove a tab's first block or its break.
    pub(super) fn tabs_may_break(&self, ops: &[Op]) -> bool {
        // A batch that adds the first tab can break it later in the same batch (add a tab, then delete its first block).
        (self.tabs.list.is_some() || ops.iter().any(|o| matches!(o, Op::Tab(_))))
            && ops.iter().any(|o| match o {
                Op::Delete { from, to } => from.p != to.p,
                _ => !o.class().within_paragraph,
            })
    }

    /// Blocks put in right after a paragraph that ends a tab belong to that tab: the break moves to the last of them.
    pub(super) fn tabs_keep_boundary(&mut self, after: NodeId, inserted: &[NodeId]) {
        if self.tabs.list.is_none() || inserted.is_empty() {
            return;
        }
        let Some(i) = self.body.iter().position(|b| b.id() == after) else { return };
        let (Some(first), Some(last)) = (inserted.first(), inserted.last()) else { return };
        let ends_tab = matches!(&self.body[i], Block::Para(p) if para_sect(p).is_some());
        let after_run = i + inserted.len();
        let in_place = self.body.get(i + 1).is_some_and(|b| b.id() == *first) && self.body.get(after_run).is_some_and(|b| b.id() == *last);
        if !ends_tab || !in_place || self.body.get(after_run + 1).is_none_or(|b| marker_in_block(b).is_none()) {
            return;
        }
        let sect = match &mut self.body[i] {
            Block::Para(p) => take_para_sect(p),
            _ => None,
        };
        let Some(sect) = sect else { return };
        match &mut self.body[after_run] {
            Block::Para(p) => put_para_sect(p, sect),
            _ => {
                let mut p = Paragraph::new(self.ids.fresh());
                put_para_sect(&mut p, sect);
                self.changed.insert(p.id);
                self.body.insert(after_run + 1, Block::Para(p));
            }
        }
        self.changed.insert(after);
    }
}

/// The id of the bookmark that marks a tab's start in a block.
fn marks_of(b: &Block, tab: &str) -> Option<i64> {
    fn in_inlines(list: &[Inline], name: &str) -> Option<i64> {
        for i in list {
            match i {
                Inline::Opaque(e) if e.name == "w:bookmarkStart" && e.attr("w:name") == Some(name) => return e.attr("w:id").and_then(|v| v.parse().ok()),
                Inline::Container(c) => {
                    if let Some(v) = in_inlines(&c.children, name) {
                        return Some(v);
                    }
                }
                _ => {}
            }
        }
        None
    }
    let name = marker_name(tab);
    match b {
        Block::Para(p) => in_inlines(&p.inlines, &name),
        Block::Table(t) => t.rows().flat_map(|r| r.cells()).find_map(|c| c.blocks.iter().find_map(|b| marks_of(b, tab))),
        Block::Container(c) => c.blocks.iter().find_map(|b| marks_of(b, tab)),
        Block::Opaque(_) => None,
    }
}
