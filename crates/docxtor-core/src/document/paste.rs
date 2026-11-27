//! Copy and paste of rich content. `copy_fragment` turns a range into a `Fragment` (runs, paragraph
//! properties, styles, lists, tables, pictures, links, notes, equations); `op_insert_fragment` brings one in
//! (from this or another document, or from the HTML and RTF readers). See `fragment` for the format.

use super::*;
use crate::fragment::{self, FragDefaults, FragMedia, FragNote, FragWarning, Fragment, PasteMode};
use crate::notes::NoteKind;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Copy
// ---------------------------------------------------------------------------

struct CopyState {
    a: Pos,
    b: Pos,
    inside: bool,
    done: bool,
    /// The first block starts at its beginning and the last ends at its end.
    first_whole: Option<bool>,
    last_whole: bool,
    /// Which of the copied elements came from an imported chunk: (first, end, the chunk's container id). Their
    /// pictures, links, lists and styles are the chunk's own, not the document's.
    chunks: Vec<(usize, usize, NodeId)>,
}

#[derive(Default)]
pub(super) struct Gather {
    pub(super) styles: BTreeSet<String>,
    pub(super) nums: BTreeSet<i64>,
    pub(super) media: BTreeMap<String, FragMedia>,
    pub(super) media_keys: HashMap<String, String>,
    pub(super) links: BTreeMap<String, String>,
    pub(super) notes: BTreeMap<String, FragNote>,
    pub(super) warnings: Vec<FragWarning>,
    /// Set while reading something that can't travel; the run holding it is dropped.
    pub(super) bad: bool,
}

impl Gather {
    fn warn(&mut self, kind: &str) {
        match self.warnings.iter_mut().find(|w| w.kind == kind) {
            Some(w) => w.count += 1,
            None => self.warnings.push(FragWarning { kind: kind.into(), count: 1 }),
        }
    }
}

fn table_has(t: &Table, id: NodeId) -> bool {
    let mut found = false;
    for r in t.rows() {
        for c in r.cells() {
            walk_paragraphs(&c.blocks, &mut |p| found |= p.id == id);
        }
    }
    found
}

fn cell_of(t: &Table, id: NodeId) -> Option<(usize, usize)> {
    for (ri, r) in t.rows().enumerate() {
        for (ci, c) in r.cells().enumerate() {
            let mut hit = false;
            walk_paragraphs(&c.blocks, &mut |p| hit |= p.id == id);
            if hit {
                return Some((ri, ci));
            }
        }
    }
    None
}

fn cell_blocks(t: &Table, ri: usize, ci: usize) -> &[Block] {
    t.rows().nth(ri).and_then(|r| r.cells().nth(ci)).map(|c| c.blocks.as_slice()).unwrap_or(&[])
}

/// A table keeping the rows `r0..=r1` and, when `cols` is given, only those cells of each row.
fn table_part(t: &Table, r0: usize, r1: usize, cols: Option<(usize, usize)>) -> Element {
    let mut el = write_table(t);
    let mut ri = 0;
    el.children.retain_mut(|n| {
        let Node::Element(row) = n else { return true };
        if row.name != "w:tr" {
            return true;
        }
        let keep = ri >= r0 && ri <= r1;
        ri += 1;
        if keep {
            if let Some((c0, c1)) = cols {
                let mut ci = 0;
                row.children.retain(|n| {
                    let Node::Element(c) = n else { return true };
                    if c.name != "w:tc" {
                        return true;
                    }
                    let k = ci >= c0 && ci <= c1;
                    ci += 1;
                    k
                });
            }
        }
        keep
    });
    // The grid has to match the cells that are left.
    if let Some((c0, c1)) = cols {
        if let Some(grid) = el.child_mut("w:tblGrid") {
            let mut ci = 0;
            grid.children.retain(|n| {
                let Node::Element(_) = n else { return true };
                let k = ci >= c0 && ci <= c1;
                ci += 1;
                k
            });
        }
    }
    el
}

fn extract<'a>(blocks: impl IntoIterator<Item = &'a Block>, st: &mut CopyState, out: &mut Vec<Element>) {
    for b in blocks {
        if st.done {
            return;
        }
        match b {
            Block::Para(p) => {
                let is_b = p.id == st.b.p.0;
                if !st.inside && p.id != st.a.p.0 {
                    continue;
                }
                let len = p.len16();
                let start = if st.inside { 0 } else { st.a.o.min(len) };
                let end = if is_b { st.b.o.min(len) } else { len };
                if st.first_whole.is_none() {
                    st.first_whole = Some(start == 0);
                }
                st.inside = true;
                st.last_whole = end == len;
                let mut np = Paragraph::new(0);
                np.attrs = p.attrs.clone();
                np.ppr = p.ppr.clone();
                np.inlines = edit::slice(&p.inlines, start, end.max(start));
                out.push(write_paragraph(&np));
                if is_b {
                    st.done = true;
                }
            }
            Block::Table(t) => {
                let has_a = table_has(t, st.a.p.0);
                let has_b = table_has(t, st.b.p.0);
                if !st.inside && !has_a {
                    continue;
                }
                if !st.inside && has_a && has_b {
                    // The range lies inside one table: a piece of one cell is just its paragraphs; across cells, whole cells.
                    let (Some((ra, ca)), Some((rb, cb))) = (cell_of(t, st.a.p.0), cell_of(t, st.b.p.0)) else { continue };
                    let ((ra, ca), (rb, cb)) = if (ra, ca) <= (rb, cb) { ((ra, ca), (rb, cb)) } else { ((rb, cb), (ra, ca)) };
                    if (ra, ca) == (rb, cb) {
                        extract(cell_blocks(t, ra, ca), st, out);
                        st.done = true;
                        continue;
                    }
                    let cols = if ra == rb { Some((ca, cb)) } else { None };
                    out.push(table_part(t, ra, rb, cols));
                    st.first_whole = Some(true);
                    st.last_whole = true;
                    st.done = true;
                    continue;
                }
                if st.first_whole.is_none() {
                    st.first_whole = Some(true);
                }
                st.inside = true;
                st.last_whole = true;
                out.push(write_table(t));
                if has_b {
                    st.done = true;
                }
            }
            Block::Container(c) if is_alt_chunk(c) => {
                let first = out.len();
                extract(&c.blocks, st, out);
                if out.len() > first {
                    st.chunks.push((first, out.len(), c.id));
                }
            }
            Block::Container(c) => extract(&c.blocks, st, out),
            Block::Opaque(_) => {}
        }
    }
}

/// Make the fields in a paragraph balanced: a copy that cuts a field in two keeps the result text only.
fn balance_fields(p: &mut Element) {
    let mut depth = 0i32;
    let mut broken = false;
    walk(p, &mut |e| {
        if e.name == "w:fldChar" {
            match e.attr("w:fldCharType") {
                Some("begin") => depth += 1,
                Some("end") => {
                    depth -= 1;
                    if depth < 0 {
                        broken = true;
                    }
                }
                _ => {}
            }
        }
    });
    if depth == 0 && !broken {
        return;
    }
    fn strip(e: &mut Element) {
        e.children.retain(|n| !matches!(n, Node::Element(c) if matches!(c.name.as_str(), "w:fldChar" | "w:instrText")));
        for c in e.elements_mut() {
            strip(c);
        }
    }
    strip(p);
}

use crate::fragment::walk;

impl Document {
    /// A copy of the content between two positions, with everything it needs to live in another document.
    pub fn copy_fragment(&self, from: Pos, to: Pos) -> Option<Fragment> {
        let (blocks, part) = self.locate(from.p.0);
        let part = part.to_string();
        let order = paragraph_order(blocks);
        let ia = order.iter().position(|x| *x == from.p.0)?;
        let ib = order.iter().position(|x| *x == to.p.0)?;
        let (a, b) = if (ia, from.o) <= (ib, to.o) { (from, to) } else { (to, from) };
        if a == b {
            return None;
        }
        let mut st = CopyState { a, b, inside: false, done: false, first_whole: None, last_whole: false, chunks: Vec::new() };
        let mut els = Vec::new();
        extract(blocks, &mut st, &mut els);
        if els.is_empty() {
            return None;
        }
        Some(self.build_fragment_of_chunks(els, &st.chunks, &part, st.first_whole == Some(true) && st.last_whole))
    }

    /// Like `build_fragment`, for elements some of which came from imported chunks (`chunks`: ranges of `els` and the
    /// chunk each is from): a chunk's elements name pictures, links, lists and styles of the chunk's own preview
    /// document, so they are gathered there and the pieces joined.
    pub(crate) fn build_fragment_of_chunks(&self, els: Vec<Element>, chunks: &[(usize, usize, NodeId)], part: &str, block: bool) -> Fragment {
        if chunks.is_empty() {
            return self.build_fragment(els, part, block);
        }
        let mut pieces: Vec<(Option<NodeId>, Vec<Element>)> = Vec::new();
        for (i, e) in els.into_iter().enumerate() {
            let src = chunks.iter().find(|(a, b, _)| i >= *a && i < *b).map(|c| c.2);
            match pieces.last_mut() {
                Some((s, v)) if *s == src => v.push(e),
                _ => pieces.push((src, vec![e])),
            }
        }
        let mut out: Option<Fragment> = None;
        for (n, (src, els)) in pieces.into_iter().enumerate() {
            let frag = match src.and_then(|id| self.alt_chunks.get(&id)) {
                Some(alt) => alt.doc.build_fragment(els, &alt.doc.main_path, block),
                None => self.build_fragment(els, part, block),
            };
            match out.as_mut() {
                Some(o) => o.append(frag, &format!("c{n}")),
                None => out = Some(frag),
            }
        }
        out.unwrap_or_default()
    }

    /// A fragment of the given blocks (`w:p` and `w:tbl` elements of the part `part`) with the styles, lists and
    /// pictures they use. `block` says they are whole paragraphs.
    pub(crate) fn build_fragment(&self, els: Vec<Element>, part: &str, block: bool) -> Fragment {
        let mut g = Gather::default();
        let mut kept = Vec::new();
        for mut e in els {
            if e.name == "w:p" {
                balance_fields(&mut e);
            }
            if self.gather(&mut e, part, &mut g) {
                kept.push(e);
            }
        }
        // Styles the content uses, with what they build on.
        let mut styles: Vec<String> = Vec::new();
        let mut queue: Vec<String> = g.styles.iter().cloned().collect();
        while let Some(id) = queue.pop() {
            if styles.contains(&id) || self.styles.style_element(&id).is_none() {
                continue;
            }
            queue.extend(self.styles.style_deps(&id));
            styles.push(id);
        }
        styles.sort();
        let mut style_els: Vec<Element> = Vec::new();
        for id in &styles {
            let Some(el) = self.styles.style_element(id) else { continue };
            walk(el, &mut |e| {
                if e.name == "w:numId" {
                    if let Some(n) = e.attr("w:val").and_then(|v| v.parse::<i64>().ok()) {
                        g.nums.insert(n);
                    }
                }
            });
            style_els.push(el.clone());
        }
        let mut abstracts: Vec<Element> = Vec::new();
        let mut nums: Vec<Element> = Vec::new();
        for n in &g.nums {
            if let Some((num, abs)) = self.numbering.list_elements(*n) {
                if !abstracts.iter().any(|a| a.attr("w:abstractNumId") == abs.attr("w:abstractNumId")) {
                    abstracts.push(abs);
                }
                nums.push(num);
            }
        }
        abstracts.extend(nums);
        let run = self.styles.finish_run(self.inherited_run(None));
        let source_ns: Vec<(String, String)> = self.doc_xml.root.attrs.iter().filter_map(|(k, v)| k.strip_prefix("xmlns:").map(|p| (p.to_string(), v.to_string()))).collect();
        let mut frag = Fragment {
            version: 1,
            body: fragment::join_xml(&kept),
            styles: fragment::join_xml(&style_els),
            numbering: fragment::join_xml(&abstracts),
            media: g.media,
            links: g.links,
            notes: g.notes,
            block,
            reuse_lists: true,
            defaults: Some(FragDefaults { font: run.font.clone(), size: run.size }),
            warnings: g.warnings,
            origin: fragment::session_nonce().unwrap_or_default().to_string(),
            ..Default::default()
        };
        frag.set_namespaces(&source_ns);
        frag
    }

    /// Walk copied content: note what it uses, swap relationship ids for keys, drop what can't travel.
    /// Returns false when the element itself should go.
    pub(super) fn gather(&self, el: &mut Element, part: &str, g: &mut Gather) -> bool {
        match el.name.as_str() {
            "w:bookmarkStart" | "w:bookmarkEnd" | "w:commentRangeStart" | "w:commentRangeEnd" | "w:commentReference" | "w:proofErr" | "w:permStart" | "w:permEnd" | "w:sectPr" | "w:lastRenderedPageBreak" => return false,
            "w:pStyle" | "w:rStyle" | "w:tblStyle" => {
                if let Some(v) = el.attr("w:val") {
                    g.styles.insert(v.to_string());
                }
            }
            "w:numId" => {
                if let Some(n) = el.attr("w:val").and_then(|v| v.parse::<i64>().ok()) {
                    if n > 0 {
                        g.nums.insert(n);
                    }
                }
            }
            "w:footnoteReference" | "w:endnoteReference" => {
                let kind = if el.name == "w:footnoteReference" { NoteKind::Footnote } else { NoteKind::Endnote };
                let key = self.gather_note(kind, el.attr("w:id").unwrap_or_default(), g);
                match key {
                    Some(k) => el.set_attr("w:id", k),
                    None => g.bad = true,
                }
            }
            "w:p" => {
                el.attrs.retain(|(k, _)| !k.starts_with("w14:") && !k.starts_with("w:rsid"));
            }
            _ => {}
        }
        let rel_attrs: Vec<(Name, AttrValue)> = el.attrs.iter().filter(|(k, _)| k.starts_with("r:") || k == "o:relid").cloned().collect();
        for (k, v) in rel_attrs {
            let v = v.to_string();
            let Some(rel) = self.pkg.resolve_rel(part, &v) else {
                if el.name == "w:hyperlink" {
                    el.remove_attr(&k);
                } else {
                    g.bad = true;
                }
                continue;
            };
            if rel.rel_type.ends_with("/image") && !rel.external {
                let key = match g.media_keys.get(&rel.target) {
                    Some(key) => key.clone(),
                    None => {
                        let Some(bytes) = self.pkg.get(&rel.target) else {
                            g.bad = true;
                            continue;
                        };
                        let ext = rel.target.rsplit('.').next().unwrap_or("png").to_ascii_lowercase();
                        let key = format!("m{}", g.media.len() + 1);
                        g.media.insert(key.clone(), FragMedia { ext, data: fragment::b64_encode(bytes) });
                        g.media_keys.insert(rel.target.clone(), key.clone());
                        key
                    }
                };
                el.set_attr(&k, key);
            } else if rel.rel_type.ends_with("/hyperlink") && rel.external {
                let key = match g.links.iter().find(|(_, u)| **u == rel.target) {
                    Some((key, _)) => key.clone(),
                    None => {
                        let key = format!("l{}", g.links.len() + 1);
                        g.links.insert(key.clone(), rel.target.clone());
                        key
                    }
                };
                el.set_attr(&k, key);
            } else {
                g.bad = true;
                g.warn("object-dropped");
            }
        }
        let mut kept = Vec::with_capacity(el.children.len());
        for n in std::mem::take(&mut el.children) {
            match n {
                Node::Element(mut c) => {
                    let outer_bad = std::mem::replace(&mut g.bad, false);
                    let keep = self.gather(&mut c, part, g);
                    let bad = g.bad;
                    g.bad = outer_bad;
                    if bad {
                        if c.name == "w:r" {
                            continue;
                        }
                        g.bad = true;
                    }
                    if keep {
                        kept.push(Node::Element(c));
                    }
                }
                other => kept.push(other),
            }
        }
        el.children = kept;
        true
    }

    /// Put a note in the fragment's notes and return its key.
    fn gather_note(&self, kind: NoteKind, id: &str, g: &mut Gather) -> Option<String> {
        let want = if kind == NoteKind::Footnote { StoryKind::Footnote } else { StoryKind::Endnote };
        let key = format!("{}{id}", if kind == NoteKind::Footnote { "n" } else { "e" });
        if g.notes.contains_key(&key) {
            return Some(key);
        }
        let story = self.stories.iter().find(|s| s.kind == want && s.note_id.as_deref() == Some(id))?;
        let mut note = Element::new(kind.note_tag()).with_attr("w:id", &key);
        let mut paras = Vec::new();
        write_blocks(&story.blocks, &mut paras);
        note.children = paras;
        let outer = std::mem::replace(&mut g.bad, false);
        let mut kept = Vec::new();
        for n in std::mem::take(&mut note.children) {
            if let Node::Element(mut e) = n {
                if self.gather(&mut e, &story.part, g) {
                    kept.push(Node::Element(e));
                }
            }
        }
        g.bad = outer;
        note.children = kept;
        g.notes.insert(key.clone(), FragNote { kind: if kind == NoteKind::Footnote { "footnote" } else { "endnote" }.into(), xml: note.to_xml() });
        Some(key)
    }
}

// ---------------------------------------------------------------------------
// Paste
// ---------------------------------------------------------------------------

/// What a paste maps from the fragment's keys and ids to this document's.
struct Mapping {
    styles: HashMap<String, String>,
    nums: HashMap<i64, i64>,
    media: HashMap<String, String>,
    links: HashMap<String, String>,
    notes: HashMap<String, String>,
    bad: bool,
    in_story: bool,
    /// A preview (see `fragment_elements`): a note reference stays as an inert mark of the same width.
    preview: bool,
    /// The fragment was copied by this Docxtor session (see `Fragment::is_own`): `file:` links may stay.
    own: bool,
}

impl Document {
    pub(super) fn op_insert_fragment(&mut self, at: Pos, frag: &Fragment, mode: PasteMode) -> Result<OpResult, EditError> {
        let merged;
        let frag = if mode == PasteMode::Merge {
            merged = frag.merged();
            &merged
        } else {
            frag
        };
        let at = Pos::new(at.p.0, at.o.min(self.para_ref(at.p.0)?.len16()));
        if frag.is_empty() {
            return Ok(OpResult { caret: Some(at), ..Default::default() });
        }
        let mut blocks = self.fragment_blocks(frag)?;
        if blocks.is_empty() {
            return Ok(OpResult { caret: Some(at), ..Default::default() });
        }
        self.insert_blocks_at(at, &mut blocks, frag.block)
    }

    /// The blocks of a fragment made over for this document: its styles, lists, pictures, links and notes brought in,
    /// and the blocks given fresh ids. Nothing is put in the text.
    pub(super) fn fragment_blocks(&mut self, frag: &Fragment) -> Result<Vec<Block>, EditError> {
        let kept = self.fragment_elements(frag, false);
        Ok(kept.iter().map(|e| parse_block(e, &mut self.ids)).filter(|b| matches!(b, Block::Para(_) | Block::Table(_))).collect())
    }

    /// The blocks of a fragment as elements for this document: its styles and lists brought in (ids mapped, missing
    /// definitions added), its pictures and links related to the part being edited, drawing ids renewed. With
    /// `drop_notes`, runs that hold a footnote or endnote reference are left out (a preview has no notes).
    pub(crate) fn fragment_elements(&mut self, frag: &Fragment, drop_notes: bool) -> Vec<Element> {
        for (p, u) in &frag.ns {
            self.ensure_ns(p, u);
        }
        let mut parsed = frag.parse();
        // The fragment may have come from a web page's clipboard data: keep only what a paste may carry.
        let own = frag.is_own();
        parsed.body.retain_mut(|e| super::paste_safe::sanitize(e, own));

        // Lists first (styles can name them), then styles.
        let mut used = BTreeSet::new();
        for e in &parsed.body {
            walk(e, &mut |e| {
                if e.name == "w:numId" {
                    if let Some(n) = e.attr("w:val").and_then(|v| v.parse::<i64>().ok()) {
                        used.insert(n);
                    }
                }
            });
        }
        for e in &parsed.styles {
            walk(e, &mut |e| {
                if e.name == "w:numId" {
                    if let Some(n) = e.attr("w:val").and_then(|v| v.parse::<i64>().ok()) {
                        used.insert(n);
                    }
                }
            });
        }
        let mut nums = HashMap::new();
        for n in used {
            if n <= 0 {
                continue;
            }
            let Some(num) = parsed.numbering.iter().find(|e| e.name == "w:num" && e.attr("w:numId").and_then(|v| v.parse::<i64>().ok()) == Some(n)) else { continue };
            let abs_id = num.child("w:abstractNumId").and_then(|e| e.attr("w:val"));
            let Some(abs) = parsed.numbering.iter().find(|e| e.name == "w:abstractNum" && e.attr("w:abstractNumId") == abs_id) else { continue };
            let bullet = abs.children_named("w:lvl").next().and_then(|l| l.child("w:numFmt")).and_then(|f| f.attr("w:val")) == Some("bullet");
            nums.insert(n, self.numbering.import_list(abs, num, frag.reuse_lists || bullet));
        }
        let styles = self.styles.import_styles(&parsed.styles, &nums);

        let mut map = Mapping { styles, nums, media: HashMap::new(), links: HashMap::new(), notes: HashMap::new(), bad: false, in_story: self.parked.is_some() || drop_notes, preview: drop_notes, own };
        let dest_run = self.styles.finish_run(self.inherited_run(None));
        let bake = frag.defaults.as_ref().map(|d| (d.font.clone().filter(|f| Some(f) != dest_run.font.as_ref()), d.size.filter(|s| Some(*s) != dest_run.size))).filter(|(f, s)| f.is_some() || s.is_some());
        let default_para = self.styles.default_para.clone();
        let mut kept = Vec::new();
        for mut e in std::mem::take(&mut parsed.body) {
            if self.remap(&mut e, frag, &mut map) {
                if let Some((font, size)) = &bake {
                    bake_defaults(&mut e, font.as_deref(), *size, default_para.as_deref());
                }
                kept.push(e);
            }
        }
        kept
    }

    /// Put parsed blocks in the text at a position (see the rules in the module docs of `fragment`).
    fn insert_blocks_at(&mut self, at: Pos, blocks: &mut Vec<Block>, whole: bool) -> Result<OpResult, EditError> {
        let dest_len = self.para_ref(at.p.0)?.len16();
        let mut created = Vec::new();
        // One paragraph goes into the text where the caret is.
        if blocks.len() == 1 {
            if let Block::Para(src) = &blocks[0] {
                let src = src.clone();
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                if dest_len == 0 && whole {
                    p.ppr = src.ppr.clone();
                }
                let mut off = at.o.min(p.len16());
                for i in src.inlines {
                    let l = i.len16();
                    edit::insert_inline_at(&mut p.inlines, off, i);
                    off += l;
                }
                edit::cleanup(&mut p.inlines);
                return Ok(OpResult { caret: Some(Pos::new(at.p.0, off)), ..Default::default() });
            }
        }
        // Into an empty paragraph: the blocks go before it and the last paragraph takes its place.
        if dest_len == 0 {
            let last_para = matches!(blocks.last(), Some(Block::Para(_)));
            let mut pre: Vec<Block> = std::mem::take(blocks);
            let tail = if last_para { pre.pop() } else { None };
            created.extend(pre.iter().map(|b| b.id()));
            if !pre.is_empty() {
                self.insert_blocks(None, Some(at.p.0), pre)?;
            }
            let mut caret = Pos::new(at.p.0, 0);
            if let Some(Block::Para(src)) = tail {
                self.touch(at.p.0);
                let p = self.para_mut(at.p.0)?;
                p.ppr = src.ppr.clone();
                let mut off = 0;
                for i in src.inlines {
                    let l = i.len16();
                    edit::insert_inline_at(&mut p.inlines, off, i);
                    off += l;
                }
                edit::cleanup(&mut p.inlines);
                caret = Pos::new(at.p.0, off);
            }
            return Ok(OpResult { caret: Some(caret), created, count: None });
        }
        // Several blocks into text: split the paragraph, the first and last paragraphs join the halves.
        let split = self.split_paragraph(at)?;
        let right = split.created.first().copied().ok_or_else(|| EditError::Invalid("could not split the paragraph".into()))?;
        created.push(right);
        let mut caret = Pos::new(right, 0);
        let first_is_para = matches!(blocks.first(), Some(Block::Para(_)));
        if first_is_para {
            let Block::Para(src) = blocks.remove(0) else { unreachable!() };
            self.touch(at.p.0);
            let p = self.para_mut(at.p.0)?;
            if p.len16() == 0 {
                p.ppr = src.ppr.clone();
            }
            let mut off = p.len16();
            for i in src.inlines {
                let l = i.len16();
                edit::insert_inline_at(&mut p.inlines, off, i);
                off += l;
            }
            edit::cleanup(&mut p.inlines);
        }
        if matches!(blocks.last(), Some(Block::Para(_))) {
            let Block::Para(src) = blocks.pop().unwrap() else { unreachable!() };
            self.touch(right);
            let p = self.para_mut(right)?;
            if p.len16() == 0 {
                p.ppr = src.ppr.clone();
            }
            let mut off = 0;
            for i in src.inlines {
                let l = i.len16();
                edit::insert_inline_at(&mut p.inlines, off, i);
                off += l;
            }
            edit::cleanup(&mut p.inlines);
            caret = Pos::new(right, off);
        }
        if !blocks.is_empty() {
            created.extend(blocks.iter().map(|b| b.id()));
            self.insert_blocks(Some(at.p.0), None, std::mem::take(blocks))?;
        }
        Ok(OpResult { caret: Some(caret), created, count: None })
    }

    /// Rewrite a fragment element for this document: styles, lists, relationships, drawing ids and notes.
    /// Returns false when the element should be left out.
    fn remap(&mut self, el: &mut Element, frag: &Fragment, m: &mut Mapping) -> bool {
        match el.name.as_str() {
            "w:bookmarkStart" | "w:bookmarkEnd" | "w:commentRangeStart" | "w:commentRangeEnd" | "w:commentReference" | "w:sectPr" => return false,
            "w:pStyle" | "w:rStyle" | "w:tblStyle" => {
                let v = el.attr("w:val").unwrap_or_default().to_string();
                match m.styles.get(&v).cloned().or_else(|| if self.styles.exists(&v) { Some(v.clone()) } else { self.styles.ensure_builtin(&v) }) {
                    Some(to) => el.set_attr("w:val", to),
                    None => return false,
                }
            }
            "w:numPr" => {
                if let Some(n) = el.child("w:numId").and_then(|e| e.attr("w:val")).and_then(|v| v.parse::<i64>().ok()) {
                    if n > 0 {
                        match m.nums.get(&n) {
                            Some(to) => {
                                if let Some(e) = el.child_mut("w:numId") {
                                    e.set_attr("w:val", to.to_string());
                                }
                            }
                            None => return false,
                        }
                    }
                }
            }
            "w:p" => el.attrs.retain(|(k, _)| !k.starts_with("w14:") && !k.starts_with("w:rsid")),
            "wp:docPr" => {
                self.drawing_id += 1;
                el.set_attr("id", self.drawing_id.to_string());
            }
            "w:footnoteReference" | "w:endnoteReference" => {
                if m.preview {
                    // Shown as nothing, but it counts one, as it does once the chunk is real (positions agree).
                    el.set_attr("w:customMarkFollows", "1");
                } else if m.in_story {
                    m.bad = true;
                } else {
                    let key = el.attr("w:id").unwrap_or_default().to_string();
                    let kind = if el.name == "w:footnoteReference" { NoteKind::Footnote } else { NoteKind::Endnote };
                    match self.import_note(&key, kind, frag, m) {
                        Some(id) => el.set_attr("w:id", id),
                        None => m.bad = true,
                    }
                }
            }
            _ => {}
        }
        let rel_attrs: Vec<(Name, AttrValue)> = el.attrs.iter().filter(|(k, _)| k.starts_with("r:") || k == "o:relid").cloned().collect();
        for (k, v) in rel_attrs {
            let v = v.to_string();
            let rid = if let Some(media) = frag.media.get(&v) {
                match m.media.get(&v) {
                    Some(r) => Some(r.clone()),
                    None => {
                        let r = self.import_media(media);
                        if let Some(r) = &r {
                            m.media.insert(v.clone(), r.clone());
                        }
                        r
                    }
                }
            } else if let Some(url) = frag.links.get(&v).filter(|u| super::paste_safe::safe_url_for(u, m.own)) {
                match m.links.get(&v) {
                    Some(r) => Some(r.clone()),
                    None => {
                        let main = self.active_part.clone();
                        let r = self.pkg.add_rel(&main, package::REL_HYPERLINK, url, true);
                        m.links.insert(v.clone(), r.clone());
                        Some(r)
                    }
                }
            } else {
                None
            };
            match rid {
                Some(r) => {
                    self.ensure_ns("r", NS_R);
                    el.set_attr(&k, r);
                }
                None => {
                    if el.name == "w:hyperlink" {
                        el.remove_attr(&k);
                    } else {
                        m.bad = true;
                    }
                }
            }
        }
        let mut kept = Vec::with_capacity(el.children.len());
        for n in std::mem::take(&mut el.children) {
            match n {
                Node::Element(mut c) => {
                    let outer = std::mem::replace(&mut m.bad, false);
                    let keep = self.remap(&mut c, frag, m);
                    let bad = m.bad;
                    m.bad = outer;
                    if bad {
                        if c.name == "w:r" {
                            continue;
                        }
                        m.bad = true;
                    }
                    if keep {
                        kept.push(Node::Element(c));
                    }
                }
                other => kept.push(other),
            }
        }
        el.children = kept;
        true
    }

    /// Add a picture to the package and relate it to the part being edited; returns the relationship id.
    fn import_media(&mut self, media: &FragMedia) -> Option<String> {
        // The extension names the file in the package: only picture types, so nothing odd lands in the archive.
        // These are the picture types Word stores (`emz` and `wmz` are gzipped metafiles).
        const PICTURES: &[&str] = &["png", "jpg", "jpeg", "jpe", "jfif", "gif", "bmp", "dib", "tif", "tiff", "webp", "svg", "emf", "wmf", "emz", "wmz", "ico", "tga", "pcz", "wdp"];
        if !PICTURES.contains(&media.ext.trim_start_matches('.').to_ascii_lowercase().as_str()) {
            return None;
        }
        let bytes = fragment::b64_decode(&media.data)?;
        // The same picture pasted twice is stored once.
        let name = self
            .pkg
            .parts
            .iter()
            .find(|p| p.name.starts_with("word/media/") && p.data == bytes)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| self.add_media(&media.ext, bytes));
        let target = relative_target(&self.active_part, &name);
        let part = self.active_part.clone();
        let rid = self.pkg.add_rel(&part, package::REL_IMAGE, &target, false);
        self.ensure_ns("r", NS_R);
        self.ensure_ns("wp", NS_WP);
        Some(rid)
    }

    /// Make a note from a fragment's copy of it; returns the new note's id.
    fn import_note(&mut self, key: &str, kind: NoteKind, frag: &Fragment, m: &mut Mapping) -> Option<String> {
        if let Some(id) = m.notes.get(key) {
            return Some(id.clone());
        }
        let src = frag.notes.get(key)?;
        let parsed = xml::parse(src.xml.as_bytes()).ok()?.root;
        let part = self.ensure_notes_part(kind);
        let outer_part = std::mem::replace(&mut self.active_part, part.clone());
        let mut inner = Mapping { styles: m.styles.clone(), nums: m.nums.clone(), media: HashMap::new(), links: HashMap::new(), notes: HashMap::new(), bad: false, in_story: true, preview: false, own: m.own };
        let mut paras = Vec::new();
        for n in parsed.children {
            if let Node::Element(mut e) = n {
                if super::paste_safe::sanitize(&mut e, m.own) && self.remap(&mut e, frag, &mut inner) {
                    paras.push(Node::Element(e));
                }
            }
        }
        self.active_part = outer_part;
        let doc = self.story_parts.get_mut(&part)?;
        let id = doc.root.children_named(kind.note_tag()).filter_map(|n| n.attr("w:id")?.parse::<i64>().ok()).max().unwrap_or(0).max(0) + 1;
        let id = id.to_string();
        let mut el = Element::new(kind.note_tag()).with_attr("w:id", &id);
        el.children = paras;
        let blocks = parse_blocks(&el, &mut self.ids);
        doc.root.children.push(Node::Element(el));
        self.story_gen += 1;
        let skind = if kind == NoteKind::Endnote { StoryKind::Endnote } else { StoryKind::Footnote };
        self.stories.push(Story { kind: skind, part: part.clone(), note_id: Some(id.clone()), blocks, gen: self.story_gen });
        self.dirty_parts.insert(part);
        self.stories_touched = true;
        m.notes.insert(key.to_string(), id.clone());
        Some(id)
    }
}

/// Give runs the source document's font and size when the destination's default differs, so pasted text looks as it did.
/// Only paragraphs in the default style are touched: other styles carry their own.
fn bake_defaults(p: &mut Element, font: Option<&str>, size: Option<f64>, default_style: Option<&str>) {
    if p.name == "w:tbl" {
        for c in p.elements_mut() {
            bake_defaults(c, font, size, default_style);
        }
        return;
    }
    if p.name != "w:p" {
        for c in p.elements_mut() {
            if matches!(c.name.as_str(), "w:tr" | "w:tc" | "w:p" | "w:tbl") {
                bake_defaults(c, font, size, default_style);
            }
        }
        return;
    }
    let style = p.child("w:pPr").and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val"));
    if style.is_some() && style != default_style {
        return;
    }
    fn runs(e: &mut Element, f: &mut dyn FnMut(&mut Element)) {
        if e.name == "w:r" {
            f(e);
            return;
        }
        for c in e.elements_mut() {
            if matches!(c.name.as_str(), "w:r" | "w:hyperlink" | "w:ins" | "w:smartTag" | "w:fldSimple" | "w:sdt" | "w:sdtContent") {
                runs(c, f);
            }
        }
    }
    runs(p, &mut |r| {
        if r.child("w:rPr").is_none() {
            r.children.insert(0, Node::Element(Element::new("w:rPr")));
        }
        let rpr = r.child_mut("w:rPr").unwrap();
        if let Some(f) = font {
            if rpr.child("w:rFonts").is_none() {
                crate::props::put_rpr(rpr, Element::new("w:rFonts").with_attr("w:ascii", f).with_attr("w:hAnsi", f).with_attr("w:cs", f));
            }
        }
        if let Some(s) = size {
            if rpr.child("w:sz").is_none() {
                let hp = ((s * 2.0).round() as i64).to_string();
                crate::props::put_rpr(rpr, Element::new("w:sz").with_attr("w:val", &hp));
                crate::props::put_rpr(rpr, Element::new("w:szCs").with_attr("w:val", &hp));
            }
        }
    });
}
