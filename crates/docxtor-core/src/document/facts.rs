//! What the whole-document passes of a render read from each top-level block, kept per block.
//!
//! Drawing a keystroke used to walk every paragraph of the document several times: for the note numbers, the note
//! labels, the order of the notes, the list labels and the section properties. Every one of those passes reads only a
//! few things from a block (the note references in it, the list a paragraph is in, the paragraph that ends a section),
//! and a keystroke changes one block. `Facts` keeps what each block holds of them; a block is read again only when
//! `Changed::facts` (fed by the same places that mark a block for drawing) names it, and the whole-document results
//! are built again only when some block's facts differ from before. Typing in a paragraph with no note and no list
//! therefore reads one paragraph, whatever the length of the document.
//!
//! The facts belong to the main body. They are used only inside `render_changed` and `render_all` (`Facts::fresh`),
//! right after `refresh_facts`; everywhere else the document is walked as before. What does not come from a block
//! (styles, numbering definitions, settings, section properties) is not held here: a change of any of these (the
//! `RenderKey`, the numbering part) throws the dependent results away.
//!
//! In debug builds and tests every result built from the facts is compared with the same result built by walking the
//! document, and a difference panics: it means something changed a block without marking it, or a result depends on
//! something that is not tracked.

use super::*;
use crate::notes::NoteKind;
use crate::numbering::ListInput;
use crate::wordcount::Counts;
use std::sync::Arc;

/// What a paragraph says about notes and sections, in document order.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum NoteEv {
    /// A reference to a note (`end`: an endnote) with its id; `custom`: the note has a mark of its own.
    Ref { end: bool, id: String, custom: bool },
    /// The paragraph ends a section: its id.
    SectEnd(NodeId),
}

type Notes = Option<Arc<Vec<NoteEv>>>;
type Lists = Option<Arc<Vec<ListInput>>>;

#[derive(Default)]
pub(super) struct Facts {
    built: bool,
    /// The ids of the body's top-level blocks as of the last refresh.
    order: Vec<NodeId>,
    /// Per block: the note and section events in it (`None` when it has none).
    notes: Vec<Notes>,
    /// Per block: its list paragraphs (`None` when it has none). Only meaningful while `lists_valid`.
    lists: Vec<Lists>,
    /// Whether `lists` was read with the styles and numbering of now.
    lists_valid: bool,
    /// The numbering definitions the lists were read against (`Numbering::gen`; in debug builds the part itself too, to
    /// check that the generation moves with it).
    numbering_gen: u64,
    #[cfg(any(test, debug_assertions))]
    numbering_seen: (Option<crate::xml::XmlDoc>, HashMap<String, String>),
    /// Set between `refresh_facts` and the end of the render that called it.
    fresh: bool,
    /// The note results below are out of date.
    notes_stale: bool,
    /// The list labels have to be counted again.
    labels_stale: bool,
    state: HashMap<String, u32>,
    note_labels: HashMap<String, String>,
}

impl Facts {
    /// The styles (`lists`) or the settings and section properties (`notes`) changed: what was counted from them is out
    /// of date.
    pub(super) fn key_changed(&mut self, lists: bool, notes: bool) {
        if lists {
            self.lists_valid = false;
            self.labels_stale = true;
        }
        if notes {
            self.notes_stale = true;
        }
    }

    pub(super) fn done(&mut self) {
        self.fresh = false;
    }

    pub(super) fn notes_fresh(&self) -> bool {
        self.fresh && !self.notes_stale
    }

    pub(super) fn note_labels(&self) -> &HashMap<String, String> {
        &self.note_labels
    }
}

/// The note and section events of a block, in document order.
pub(super) fn block_events(b: &Block) -> Vec<NoteEv> {
    let mut out = Vec::new();
    walk_paragraphs(std::slice::from_ref(b), &mut |p| {
        walk_runs(&p.inlines, &mut 0, &mut |r, _, _| {
            for it in &r.items {
                let RunItem::Object(e) = it else { continue };
                let end = match e.name.as_str() {
                    "w:footnoteReference" => false,
                    "w:endnoteReference" => true,
                    _ => continue,
                };
                let Some(id) = e.attr("w:id") else { continue };
                out.push(NoteEv::Ref { end, id: id.to_string(), custom: e.attr("w:customMarkFollows").is_some() });
            }
        });
        if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) {
            out.push(NoteEv::SectEnd(p.id));
        }
    });
    out
}

/// The ids of the blocks that hold something.
fn held<T>(order: &[NodeId], v: &[Option<Arc<Vec<T>>>]) -> Vec<NodeId> {
    order.iter().zip(v).filter(|(_, x)| x.is_some()).map(|(i, _)| *i).collect()
}

/// The facts of the blocks in the stretch that changed, found by id: a list while it is short (a split, a join, a
/// paste: one or a few blocks), a hash map when blocks moved far.
pub(super) struct Stayed<T>(Vec<(NodeId, T)>, Option<HashMap<NodeId, T>>);

impl<T> Stayed<T> {
    pub(super) fn new(items: impl Iterator<Item = (NodeId, T)>, len: usize) -> Self {
        if len <= 16 {
            Stayed(items.collect(), None)
        } else {
            Stayed(Vec::new(), Some(items.collect()))
        }
    }
    pub(super) fn take(&mut self, id: NodeId) -> Option<T> {
        match &mut self.1 {
            Some(m) => m.remove(&id),
            None => self.0.iter().position(|(i, _)| *i == id).map(|i| self.0.swap_remove(i).1),
        }
    }
    /// What was not taken: the blocks that went.
    pub(super) fn rest(self) -> Vec<T> {
        match self.1 {
            Some(m) => m.into_values().collect(),
            None => self.0.into_iter().map(|(_, v)| v).collect(),
        }
    }
}

fn some<T>(v: Vec<T>) -> Option<Arc<Vec<T>>> {
    (!v.is_empty()).then(|| Arc::new(v))
}

fn block_lists(b: &Block, styles: &crate::styles::StyleSheet) -> Lists {
    let mut out = Vec::new();
    // Imported chunks count their own lists (see `alt_chunk`).
    walk_paragraphs_local(std::slice::from_ref(b), &mut |p| out.extend(crate::numbering::Numbering::list_input(p, styles)));
    some(out)
}

impl Document {
    /// Read again the blocks that changed since the last refresh (all of them the first time). When blocks came, went
    /// or moved, what is known of the blocks that stayed is kept, found by id in the stretch between the blocks that
    /// kept their place at the start and at the end. Marks the results that depend on a block whose facts differ as
    /// out of date.
    pub(super) fn refresh_facts(&mut self) {
        let dirty = std::mem::take(&mut self.changed.facts);
        if self.parked.is_some() {
            self.facts = Facts::default();
            return;
        }
        let f = &mut self.facts;
        // Other lists, other counts: the numbering definitions and what they point at.
        #[cfg(any(test, debug_assertions))]
        {
            if f.numbering_gen == self.numbering.gen {
                let same = f.numbering_seen.0 == self.numbering.doc && f.numbering_seen.1 == self.numbering.rels;
                assert!(!f.built || same, "the numbering definitions changed without `Numbering::touch`");
            } else {
                f.numbering_seen = (self.numbering.doc.clone(), self.numbering.rels.clone());
            }
        }
        if f.numbering_gen != self.numbering.gen {
            f.numbering_gen = self.numbering.gen;
            f.lists_valid = false;
            f.labels_stale = true;
        }
        let body = &self.body;
        if !f.built {
            // Room to grow: a paragraph added to a document of exactly this many would otherwise double all three.
            let room = body.len() + body.len() / 8 + 8;
            f.order = Vec::with_capacity(room);
            f.order.extend(body.iter().map(Block::id));
            f.notes = Vec::with_capacity(room);
            f.notes.extend(body.iter().map(|b| some(block_events(b))));
            f.lists = Vec::with_capacity(room);
            f.lists.resize(body.len(), None);
            f.lists_valid = false;
            f.notes_stale = true;
            f.labels_stale = true;
            f.built = true;
        } else {
            let (old_n, n) = (f.order.len(), body.len());
            let (mut pre, mut suf) = (0, 0);
            while pre < old_n.min(n) && f.order[pre] == body[pre].id() {
                pre += 1;
            }
            while suf < old_n.min(n) - pre && f.order[old_n - 1 - suf] == body[n - 1 - suf].id() {
                suf += 1;
            }
            if pre + suf < old_n || pre + suf < n {
                // Blocks came, went or moved. The sequence of the blocks that hold something decides whether the results
                // change; what a block that stayed holds is kept.
                let notes_before = held(&f.order, &f.notes);
                let lists_before = if f.lists_valid { held(&f.order, &f.lists) } else { Vec::new() };
                let mut stayed = Stayed::new((pre..old_n - suf).map(|i| (f.order[i], (f.notes[i].take(), f.lists[i].take()))), old_n - suf - pre);
                let mut mid: Vec<(NodeId, Notes, Lists)> = Vec::with_capacity(n - suf - pre);
                for b in body.iter().skip(pre).take(n - suf - pre) {
                    let id = b.id();
                    // A new block was touched by whatever made it: the loop below reads it.
                    let (notes, lists) = stayed.take(id).unwrap_or_default();
                    mid.push((id, notes, lists));
                }
                let span = pre..old_n - suf;
                f.order.splice(span.clone(), mid.iter().map(|m| m.0));
                f.notes.splice(span.clone(), mid.iter().map(|m| m.1.clone()));
                f.lists.splice(span, mid.into_iter().map(|m| m.2));
                f.notes_stale |= notes_before != held(&f.order, &f.notes);
                if f.lists_valid {
                    f.labels_stale |= lists_before != held(&f.order, &f.lists);
                }
            }
            // The blocks an op touched.
            let at: Vec<usize> = if dirty.len() <= 16 {
                dirty.iter().filter_map(|id| f.order.iter().position(|o| o == id)).collect()
            } else {
                f.order.iter().enumerate().filter(|(_, id)| dirty.contains(id)).map(|(i, _)| i).collect()
            };
            for i in at {
                let notes = some(block_events(&body[i]));
                if notes != f.notes[i] {
                    f.notes[i] = notes;
                    f.notes_stale = true;
                }
                if f.lists_valid {
                    let lists = block_lists(&body[i], &self.styles);
                    if lists != f.lists[i] {
                        f.lists[i] = lists;
                        f.labels_stale = true;
                    }
                }
            }
        }
        f.fresh = true;
    }

    /// Read the lists of every block again when styles or numbering changed since they were read (after the
    /// `RenderKey` was compared, so the styles are those of the last render).
    pub(super) fn ensure_list_facts(&mut self) {
        if self.facts.fresh && !self.facts.lists_valid {
            let styles = &self.styles;
            let mut lists = Vec::with_capacity(self.body.len() + self.body.len() / 8 + 8);
            lists.extend(self.body.iter().map(|b| block_lists(b, styles)));
            self.facts.lists = lists;
            self.facts.lists_valid = true;
            self.facts.labels_stale = true;
        }
    }

    /// The list labels when they have to be counted again (`None`: as last time).
    pub(super) fn facts_labels(&mut self) -> Option<HashMap<NodeId, crate::numbering::ListLabel>> {
        if !self.facts.fresh {
            return Some(self.numbering.labels(&self.body, &self.styles));
        }
        let fresh = self.facts.labels_stale.then(|| {
            let inputs = self.facts.lists.iter().flatten().flat_map(|l| l.iter());
            self.numbering.labels_of(inputs, &self.styles)
        });
        #[cfg(any(test, debug_assertions))]
        {
            let walked = crate::perf::unmetered(|| self.numbering.labels(&self.body, &self.styles));
            let got = fresh.as_ref().unwrap_or(&self.last_labels);
            assert!(*got == walked, "list labels built from the kept facts differ from the document's (stale: {})", self.facts.labels_stale);
        }
        self.facts.labels_stale = false;
        fresh
    }

    /// The note numbers and formats (`note_state`) when they changed since last time (`None`: as last time).
    pub(super) fn facts_notes(&mut self) -> Option<HashMap<String, u32>> {
        if !self.facts.fresh {
            return Some(self.note_state());
        }
        let fresh = self.facts.notes_stale.then(|| {
            let evs: Vec<&NoteEv> = self.facts.notes.iter().flatten().flat_map(|v| v.iter()).collect();
            self.note_facts_from(&evs)
        });
        if let Some((state, labels)) = fresh.as_ref() {
            self.facts.state = state.clone();
            self.facts.note_labels = labels.clone();
        }
        self.facts.notes_stale = false;
        #[cfg(any(test, debug_assertions))]
        {
            crate::perf::unmetered(|| {
                assert!(self.facts.state == self.note_state(), "note numbers built from the kept facts differ from the document's");
                assert!(self.facts.note_labels == self.note_labels(), "note labels built from the kept facts differ from the document's");
            });
        }
        fresh.map(|f| f.0)
    }

    /// Section properties from the kept facts: the same as walking the body.
    pub(super) fn section_refs_kept(&self) -> Vec<(Option<NodeId>, Option<&Element>)> {
        let mut out = Vec::new();
        for (i, evs) in self.facts.notes.iter().enumerate() {
            let Some(evs) = evs else { continue };
            for ev in evs.iter() {
                if let NoteEv::SectEnd(id) = ev {
                    let s = find_para(std::slice::from_ref(&self.body[i]), *id).and_then(|p| p.ppr.as_ref()).and_then(|pp| pp.child("w:sectPr"));
                    out.push((Some(*id), s));
                }
            }
        }
        out.push((None, self.body.iter().rev().find_map(|b| match b {
            Block::Opaque(o) if o.el.name == "w:sectPr" => Some(&o.el),
            _ => None,
        })));
        out
    }

    pub(super) fn facts_fresh(&self) -> bool {
        self.facts.fresh
    }
}

// ---------------------------------------------------------------------------
// The note passes, over events
// ---------------------------------------------------------------------------

/// Note numbers by id, in order of reference (1-based, before formatting): notes with a mark of their own have none.
pub(super) fn nums_of(evs: &[&NoteEv], kind: NoteKind) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for ev in evs {
        if let NoteEv::Ref { end, id, custom: false } = ev {
            if *end == (kind == NoteKind::Endnote) {
                let n = out.len() as u32 + 1;
                out.entry(id.clone()).or_insert(n);
            }
        }
    }
    out
}

/// Where each note is referenced, in order (by id; `e:` and the id for an endnote). Notes with a mark of their own are
/// here, though they have no number.
pub(super) fn order_of(evs: &[&NoteEv]) -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for ev in evs {
        if let NoteEv::Ref { end, id, .. } = ev {
            let key = if *end { format!("e:{id}") } else { id.clone() };
            let n = out.len();
            out.entry(key).or_insert(n);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Values kept per block between edits
// ---------------------------------------------------------------------------

/// One value per top-level block of the body, kept between edits, so that a question about the whole body reads only
/// the blocks that changed since it was last asked: the one home of the bookkeeping behind `body_has_shapes`, the word
/// count, `body_has_locks` and the body's bookmarks. Kept apart from `Facts`: the editing ops ask these between renders,
/// and must not use up what the render still has to read. Each user has its own set in `Changed`, fed with every block
/// an op changes, and checks its result against a walk of the body in debug builds and tests.
pub(super) struct Kept<T> {
    built: bool,
    order: Vec<NodeId>,
    vals: Vec<T>,
}

impl<T> Default for Kept<T> {
    fn default() -> Self {
        Kept { built: false, order: Vec::new(), vals: Vec::new() }
    }
}

impl<T> Kept<T> {
    /// Bring the values up to date with `body`: the blocks named in `dirty` and the blocks that came are read again.
    /// Blocks that came, went or moved are found between the blocks that kept their place at the start and the end.
    /// `change(old, new)` hears of every value that goes (`new` is `None`), comes (`old` is `None`) or is read again:
    /// what a running total needs.
    pub(super) fn refresh(&mut self, body: &Body, dirty: &BTreeSet<NodeId>, read: impl Fn(&Block) -> T, mut change: impl FnMut(Option<&T>, Option<&T>)) {
        if !self.built {
            let room = body.len() + body.len() / 8 + 8;
            self.order = Vec::with_capacity(room);
            self.order.extend(body.iter().map(Block::id));
            self.vals = Vec::with_capacity(room);
            self.vals.extend(body.iter().map(&read));
            for v in &self.vals {
                change(None, Some(v));
            }
            self.built = true;
            return;
        }
        let (old_n, n) = (self.order.len(), body.len());
        let (pre, suf) = kept_ends(&self.order, body);
        // Blocks that came are read here, once: the loop over `dirty` below leaves them alone.
        let mut came: HashSet<NodeId> = HashSet::new();
        if pre + suf < old_n || pre + suf < n {
            let span = pre..old_n - suf;
            let ids: Vec<NodeId> = self.order.splice(span.clone(), body.iter().skip(pre).take(n - suf - pre).map(Block::id)).collect();
            let vals: Vec<T> = self.vals.drain(span).collect();
            let mut stayed = Stayed::new(ids.into_iter().zip(vals), old_n - suf - pre);
            let mut mid = Vec::with_capacity(n - suf - pre);
            for b in body.iter().skip(pre).take(n - suf - pre) {
                match stayed.take(b.id()) {
                    Some(v) => mid.push(v),
                    None => {
                        let v = read(b);
                        change(None, Some(&v));
                        mid.push(v);
                        came.insert(b.id());
                    }
                }
            }
            for v in stayed.rest() {
                change(Some(&v), None);
            }
            self.vals.splice(pre..pre, mid);
        }
        for i in dirty_places(&self.order, dirty) {
            if came.contains(&self.order[i]) {
                continue;
            }
            let v = read(&body[i]);
            change(Some(&self.vals[i]), Some(&v));
            self.vals[i] = v;
        }
    }
}

/// How many blocks kept their place at the start and at the end since `order` was taken (not generic: one copy for every `Kept`).
fn kept_ends(order: &[NodeId], body: &Body) -> (usize, usize) {
    let (old_n, n) = (order.len(), body.len());
    let (mut pre, mut suf) = (0, 0);
    while pre < old_n.min(n) && order[pre] == body[pre].id() {
        pre += 1;
    }
    while suf < old_n.min(n) - pre && order[old_n - 1 - suf] == body[n - 1 - suf].id() {
        suf += 1;
    }
    (pre, suf)
}

/// The places in `order` of the blocks named in `dirty`.
fn dirty_places(order: &[NodeId], dirty: &BTreeSet<NodeId>) -> Vec<usize> {
    if dirty.len() <= 16 {
        dirty.iter().filter_map(|id| order.iter().position(|o| o == id)).collect()
    } else {
        order.iter().enumerate().filter(|(_, id)| dirty.contains(id)).map(|(i, _)| i).collect()
    }
}

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

/// Which top-level blocks hold a shape (a drawing shape, group or canvas, a VML shape): a document with none has no
/// connector to keep glued and no text box to adopt, which is nearly every long text.
#[derive(Default)]
pub(super) struct ShapeFacts {
    kept: Kept<bool>,
    count: usize,
}

fn block_has_shape(b: &Block) -> bool {
    let mut any = false;
    crate::shapes::story::walk_objects(std::slice::from_ref(b), &mut |_, e| any |= crate::shapes::is_shape(e));
    any
}

/// Running count of the blocks for which a kept flag is set.
fn count_flags(count: &mut usize) -> impl FnMut(Option<&bool>, Option<&bool>) + '_ {
    move |old, new| {
        if old == Some(&true) {
            *count -= 1;
        }
        if new == Some(&true) {
            *count += 1;
        }
    }
}

impl Document {
    /// Whether the body holds any shape. Reads again only the blocks that changed since it was last asked.
    pub(super) fn body_has_shapes(&mut self) -> bool {
        let dirty = std::mem::take(&mut self.changed.shapes);
        if self.parked.is_some() {
            // The body is a story's while an op runs in it: no facts, and the question is answered by looking.
            self.shapes = ShapeFacts::default();
            return true;
        }
        let ShapeFacts { kept, count } = &mut self.shapes;
        kept.refresh(&self.body, &dirty, block_has_shape, count_flags(count));
        #[cfg(any(test, debug_assertions))]
        {
            let mut any = false;
            crate::perf::unmetered(|| crate::shapes::story::walk_objects(&self.body, &mut |_, e| any |= crate::shapes::is_shape(e)));
            assert!(any == (self.shapes.count > 0), "the kept shapes differ from the body's: a block changed without being marked");
        }
        self.shapes.count > 0
    }
}

// ---------------------------------------------------------------------------
// Locked controls
// ---------------------------------------------------------------------------

/// Which top-level blocks hold a locked content control: a document with none (nearly every one) needs no comparison of
/// its controls before and after an edit (`controls::lock_state_of`, `controls::locks_hold`).
#[derive(Default)]
pub(super) struct LockFacts {
    kept: Kept<bool>,
    count: usize,
}

fn block_has_lock(b: &Block) -> bool {
    controls::lock_state_of(std::slice::from_ref(b)).iter().any(|(_, lock, _)| !lock.is_empty())
}

impl Document {
    /// Whether the body holds a locked content control. Reads again only the blocks that changed since it was last asked.
    pub(super) fn body_has_locks(&mut self) -> bool {
        let dirty = std::mem::take(&mut self.changed.locks);
        if self.parked.is_some() {
            self.locks = LockFacts::default();
            return true;
        }
        let LockFacts { kept, count } = &mut self.locks;
        kept.refresh(&self.body, &dirty, block_has_lock, count_flags(count));
        #[cfg(any(test, debug_assertions))]
        {
            let walked = crate::perf::unmetered(|| controls::lock_state_of(&self.body).iter().any(|(_, lock, _)| !lock.is_empty()));
            assert!(walked == (self.locks.count > 0), "the kept locked controls differ from the body's: a block changed without being marked");
        }
        self.locks.count > 0
    }
}

// ---------------------------------------------------------------------------
// Bookmarks
// ---------------------------------------------------------------------------

/// The ids of the bookmark starts and ends in each top-level block, and how many of each there are in the body: what
/// `bookmark_halves` compares before and after a batch, without walking the body twice a key.
#[derive(Default)]
pub(super) struct BookmarkFacts {
    kept: Kept<Option<Arc<bookmark_halves::Halves>>>,
    counts: HashMap<String, (u32, u32)>,
    /// How many starts of the body have each name (lower case), in the first number (the type `counts` has, so no more code).
    names: HashMap<String, (u32, u32)>,
    /// Names whose last start went since `gone_bookmarks` was last asked.
    gone: Vec<String>,
    /// Bookmarks that links came to lead to (a block has a link to it that it did not have) since `gone_bookmarks` was last asked.
    led_to: Vec<String>,
}

impl Document {
    /// The bookmark names (lower case) whose last start left the body since this was last asked, and the names new links in the body
    /// lead to that the body has no bookmark of, as the kept facts last saw the body (`body_unpaired_bookmarks` reads it again). A name
    /// that went from one block and came in another is not gone.
    pub(super) fn gone_bookmarks(&mut self) -> (Vec<String>, Vec<String>) {
        let BookmarkFacts { names, gone, led_to, .. } = &mut self.bookmarks;
        let tidy = |v: Vec<String>| -> Vec<String> { v.into_iter().filter(|n| !names.contains_key(n)).collect() };
        (tidy(std::mem::take(gone)), tidy(std::mem::take(led_to)))
    }

    /// The bookmark halves of the body without their other half (`true`: a start), as `bookmark_halves::Marks::of`
    /// finds them. Reads again only the blocks that changed since it was last asked.
    pub(super) fn body_unpaired_bookmarks(&mut self) -> HashSet<(bool, String)> {
        let dirty = std::mem::take(&mut self.changed.bookmarks);
        if self.parked.is_some() {
            self.bookmarks = BookmarkFacts::default();
            return bookmark_halves::Marks::of(&self.body).unpaired().clone();
        }
        let BookmarkFacts { kept, counts, names, gone, led_to } = &mut self.bookmarks;
        kept.refresh(&self.body, &dirty, bookmark_halves::Halves::of, |old, new| {
            if let Some(Some(n)) = new {
                let had: &[String] = match old {
                    Some(Some(o)) => &o.anchors,
                    _ => &[],
                };
                led_to.extend(n.anchors.iter().filter(|a| !had.contains(a)).cloned());
            }
            for (h, step) in [(old, -1i32), (new, 1)] {
                let Some(Some(h)) = h else { continue };
                for name in &h.names {
                    let c = names.entry(name.clone()).or_default();
                    c.0 = c.0.checked_add_signed(step).expect("a bookmark name count went below zero");
                    if c.0 == 0 {
                        names.remove(name);
                        gone.push(name.clone());
                    }
                }
                for (ids, start) in [(&h.starts, true), (&h.ends, false)] {
                    for id in ids {
                        let c = counts.entry(id.clone()).or_default();
                        let n = if start { &mut c.0 } else { &mut c.1 };
                        *n = n.checked_add_signed(step).expect("a bookmark count went below zero");
                        if *c == (0, 0) {
                            counts.remove(id);
                        }
                    }
                }
            }
        });
        let mut out = HashSet::new();
        for (id, (s, e)) in &self.bookmarks.counts {
            if *s > 0 && *e == 0 {
                out.insert((true, id.clone()));
            } else if *e > 0 && *s == 0 {
                out.insert((false, id.clone()));
            }
        }
        #[cfg(any(test, debug_assertions))]
        {
            let walked = crate::perf::unmetered(|| bookmark_halves::Marks::of(&self.body));
            assert!(*walked.unpaired() == out, "the kept bookmarks differ from the body's: a block changed without being marked");
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Words
// ---------------------------------------------------------------------------

/// The words in the paragraphs of each top-level block, and their sum: the status bar's count. A block is counted again
/// only when `Changed::words` names it, so asking after a key counts one block, whatever the length of the document.
#[derive(Default)]
pub(super) struct WordFacts {
    kept: Kept<Counts>,
    total: Counts,
}

/// The paragraphs, words and characters in a block (inside tables and containers too): what the word count and the
/// document statistics count.
fn block_counts(b: &Block) -> Counts {
    crate::wordcount::count_blocks(std::slice::from_ref(b))
}

impl Document {
    /// The words in the main text, as the status bar shows them. Counts again only the blocks that changed since it was
    /// last asked.
    pub fn word_count(&mut self) -> usize {
        self.main_counts().words
    }

    /// The counts of the main text (what `text_stats().main` gives), kept per block like the word count: only the blocks
    /// that changed since either was last asked are counted again.
    pub fn main_counts(&mut self) -> Counts {
        let dirty = std::mem::take(&mut self.changed.words);
        if self.parked.is_some() {
            // The body is a story's while an op runs in it: no facts, and the question is answered by counting.
            self.words = WordFacts::default();
            return crate::wordcount::count_blocks(&self.body);
        }
        let WordFacts { kept, total } = &mut self.words;
        kept.refresh(&self.body, &dirty, block_counts, |old, new| {
            if let Some(o) = old {
                *total = total.minus(o);
            }
            if let Some(n) = new {
                *total = total.plus(n);
            }
        });
        #[cfg(any(test, debug_assertions))]
        {
            let walked = crate::perf::unmetered(|| crate::wordcount::count_blocks(&self.body));
            assert!(walked == self.words.total, "the kept counts differ from the body's: a block changed without being marked");
        }
        self.words.total
    }
}
