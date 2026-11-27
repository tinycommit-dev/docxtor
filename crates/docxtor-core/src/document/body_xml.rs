//! Writing `document.xml` for a save: what changed since the last save is written again, the rest is copied.
//!
//! The body used to be written out whole on every save: every block became an element tree, the tree was walked for
//! `w14` uses, and the tree was turned into text. `BodyXml` keeps, for each top-level block, where its text is in the
//! `document.xml` the last save put into the package, and whether the block uses `w14`. A save writes again only the
//! blocks `Changed::write` names (every block the first time, and after anything that swaps the body whole), copies the
//! text of the others from the last `document.xml`'s pieces, and declares `w14` on the root when any block uses it. The pieces
//! of a block are written the same whatever is around them, which is what makes the copy exact.
//!
//! The text is kept as pieces of about 1 MB, each holding whole blocks (the head before the first block and the tail after
//! the last are pieces of their own), and the package holds the very same pieces (`Package::set_pieces`): the part is never
//! one buffer after a save (only the zip route, `Package::to_bytes*`, joins it for the length of a write, to give the bytes it always gave: the compressor's output depends on how the input is cut into writes), so the memory a save for the browser needs is about one piece, and the engine's memory (which in the browser
//! never shrinks) does not climb from save to save. A piece whose blocks all come again unchanged and in order is kept
//! as it is (a reference count, no copy); the others are made again, copying the unchanged blocks' text from the old pieces.
//!
//! Memory: the cache is a few words per block; the text itself is the pieces the package already holds.
//!
//! In debug builds and tests every result is compared with the whole body written the old way (`body_xml_reference`),
//! and a difference panics: it means a block changed without being marked.

use super::*;
use crate::charfmt::{declare_w14, ensure_w14, uses_w14};
use crate::package::{Piece, PieceList};
use std::sync::Arc;

/// Stands in for the blocks while the rest of the part is written, to find where they go.
const MARK: &str = "\u{1}docxtor-body\u{1}";

/// About how big a piece of the part is: a piece holds whole blocks and is cut before it would pass this (a block bigger
/// than this is a piece of its own). `DOCXTOR_PIECE` (bytes) makes it smaller, for tests that need many pieces.
const PIECE: usize = 1 << 20;

static LIMIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn limit() -> usize {
    static ENV: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let set = LIMIT.load(std::sync::atomic::Ordering::Relaxed);
    if set != 0 {
        return set;
    }
    *ENV.get_or_init(|| std::env::var("DOCXTOR_PIECE").ok().and_then(|v| v.parse().ok()).filter(|n| *n > 0).unwrap_or(PIECE))
}

/// Make the pieces of the main part about `n` bytes (0 for the default), for tests. The saved bytes are the same whatever it is.
#[doc(hidden)]
pub fn set_piece_limit(n: usize) {
    LIMIT.store(n, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Clone, Copy)]
struct Entry {
    id: NodeId,
    /// The piece the block's text is in (an index into the list the last save wrote), and where: `start..end`.
    piece: usize,
    start: usize,
    end: usize,
    /// Whether any element in the block is a `w14:` one.
    w14: bool,
}

#[derive(Default)]
pub(super) struct BodyXml {
    built: bool,
    /// The pieces the entries point into: the very list the package holds, so a part that is not the one written is noticed.
    list: Option<Arc<PieceList>>,
    entries: Vec<Entry>,
}

/// Makes the pieces: whole blocks are added in order, and a piece is closed before it would pass the limit.
struct Builder {
    limit: usize,
    /// The part's size as the last save left it, to make room for a piece at once (not grow it), and what is made so far.
    est: usize,
    made: usize,
    /// Slot 0 is for the head, put in when the rest is known.
    pieces: Vec<Piece>,
    entries: Vec<Entry>,
    cur: Vec<u8>,
}

impl Builder {
    fn new(limit: usize, est: usize, blocks: usize) -> Self {
        let mut pieces: Vec<Piece> = Vec::with_capacity(2 + est / limit.max(1));
        pieces.push(Arc::new(Vec::new()));
        Builder { limit, est, made: 0, pieces, entries: Vec::with_capacity(blocks), cur: Vec::new() }
    }

    /// Close the piece being made.
    fn flush(&mut self) {
        if self.cur.is_empty() {
            return;
        }
        let mut v = std::mem::take(&mut self.cur);
        self.made += v.len();
        // A full piece keeps its room (a piece the same size fits the place it leaves); a short one gives it back.
        if v.capacity() - v.len() > self.limit / 8 {
            v.shrink_to_fit();
        }
        self.pieces.push(Arc::new(v));
    }

    fn add(&mut self, id: NodeId, w14: bool, text: &[u8]) {
        if !self.cur.is_empty() && self.cur.len() + text.len() > self.limit {
            self.flush();
        }
        if self.cur.capacity() == 0 {
            let left = self.est.saturating_sub(self.made);
            self.cur = Vec::with_capacity(self.limit.min(left + left / 8).max(text.len()));
        }
        let start = self.cur.len();
        self.cur.extend_from_slice(text);
        self.entries.push(Entry { id, piece: self.pieces.len(), start, end: self.cur.len(), w14 });
        if self.cur.len() >= self.limit {
            self.flush();
        }
    }

    /// Whether a whole old piece of `len` bytes is better taken as it is than copied block by block into the piece being made.
    fn takes_whole(&self, len: usize) -> bool {
        self.cur.is_empty() || self.cur.len() + len > self.limit
    }

    fn reuse(&mut self, piece: &Piece, run: &[Entry]) {
        self.flush();
        self.made += piece.len();
        let at = self.pieces.len();
        self.pieces.push(piece.clone());
        self.entries.extend(run.iter().map(|e| Entry { piece: at, ..*e }));
    }
}

impl Document {
    /// Write `document.xml` into the package as pieces, writing again only what changed since the last call.
    pub(super) fn write_body_xml(&mut self) {
        let dirty = std::mem::take(&mut self.changed.write);
        let main = self.main_path.clone();
        let usable = self.body_xml_usable();
        let est = self.pkg.part_len(&main).unwrap_or(0);
        if !usable && !self.body.is_empty() {
            // Nothing of the part written before is copied (every block is written): let it go first, so the pieces
            // can take the memory it leaves, and the engine's memory, which never shrinks, does not hold both.
            self.body_xml = BodyXml::default();
            self.pkg.release(&main);
        }
        let (list, entries) = match self.write_body_cached(&dirty, usable, est) {
            Some(r) => r,
            None => {
                self.body_xml = BodyXml::default();
                let whole = self.body_xml_reference();
                self.pkg.set(&main, whole);
                return;
            }
        };
        #[cfg(any(test, debug_assertions))]
        {
            let bytes: Vec<u8> = list.iter().flat_map(|p| p.iter().copied()).collect();
            let reference = crate::perf::unmetered(|| self.body_xml_reference());
            if reference != bytes {
                let at = reference.iter().zip(&bytes).position(|(a, b)| a != b).unwrap_or(reference.len().min(bytes.len()));
                let ctx = |v: &[u8]| String::from_utf8_lossy(&v[at.saturating_sub(60)..(at + 60).min(v.len())]).into_owned();
                panic!(
                    "the saved body differs from the whole body written again at byte {at}: a block changed without being marked.\nkept: {}\nwhole: {}",
                    ctx(&bytes),
                    ctx(&reference)
                );
            }
        }
        let list = Arc::new(list);
        self.pkg.set_pieces(&main, list.clone());
        self.body_xml = BodyXml { built: true, list: Some(list), entries };
    }

    /// Whether the entries point into the pieces the package holds now (the part is the one the last save wrote).
    fn body_xml_usable(&self) -> bool {
        let kept = &self.body_xml;
        kept.built && matches!((&kept.list, self.pkg.pieces_of(&self.main_path)), (Some(a), Some(b)) if Arc::ptr_eq(a, b))
    }

    fn write_body_cached(&self, dirty: &BTreeSet<NodeId>, usable: bool, est: usize) -> Option<(PieceList, Vec<Entry>)> {
        if self.body.is_empty() {
            return None;
        }
        let kept = &self.body_xml;
        let no_pieces: PieceList = Vec::new();
        let (old, prev): (&PieceList, &[Entry]) = if usable { (kept.list.as_deref().unwrap_or(&no_pieces), kept.entries.as_slice()) } else { (&no_pieces, &[]) };
        // Blocks that came, went or moved are found between the blocks that kept their place at the start and the end.
        let (old_n, n) = (prev.len(), self.body.len());
        let (mut pre, mut suf) = (0, 0);
        while pre < old_n.min(n) && prev[pre].id == self.body[pre].id() {
            pre += 1;
        }
        while suf < old_n.min(n) - pre && prev[old_n - 1 - suf].id == self.body[n - 1 - suf].id() {
            suf += 1;
        }
        let mut stayed = facts::Stayed::new((pre..old_n - suf).map(|i| (prev[i].id, prev[i])), old_n - suf - pre);
        // The blocks of each old piece: where its first entry is in `prev`, and how many there are.
        let mut runs: Vec<(usize, usize)> = vec![(0, 0); old.len()];
        for (j, e) in prev.iter().enumerate() {
            let r = &mut runs[e.piece];
            if r.1 == 0 {
                r.0 = j;
            }
            r.1 += 1;
        }
        let mut out = Builder::new(limit(), est, n);
        let mut scratch = String::new();
        let mut written = 0;
        let mut i = 0;
        while i < n {
            let b = &self.body[i];
            let id = b.id();
            let found = if i < pre {
                Some(prev[i])
            } else if i >= n - suf {
                Some(prev[old_n - (n - i)])
            } else {
                stayed.take(id)
            };
            match found {
                Some(e) if !dirty.contains(&id) => {
                    // A whole old piece whose blocks all come again, in order and unchanged, is kept as it is: no copy.
                    if e.start == 0 {
                        let (a, k) = runs[e.piece];
                        if k > 0 && i + k <= n && out.takes_whole(old[e.piece].len()) && (0..k).all(|t| {
                            let bid = self.body[i + t].id();
                            prev[a + t].id == bid && !dirty.contains(&bid)
                        }) {
                            out.reuse(&old[e.piece], &prev[a..a + k]);
                            i += k;
                            continue;
                        }
                    }
                    out.add(id, e.w14, &old[e.piece][e.start..e.end]);
                }
                _ => {
                    let el = write_block(b);
                    let w14 = uses_w14(&el);
                    scratch.clear();
                    xml::write_element_into(&el, &mut scratch);
                    out.add(id, w14, scratch.as_bytes());
                    written += 1;
                }
            }
            i += 1;
        }
        out.flush();
        crate::perf::blocks_written(written);
        let any_w14 = out.entries.iter().any(|e| e.w14);
        // Everything but the blocks, with a mark where they go.
        let mut doc = self.doc_xml.clone();
        doc.root.child_mut("w:body")?.children.push(Node::Text(MARK.into()));
        declare_w14(&mut doc.root);
        if any_w14 && doc.root.attr("xmlns:w14").is_none() {
            ensure_w14(&mut doc.root);
        }
        let shell = doc.to_bytes();
        let m = MARK.as_bytes();
        let at = shell.windows(m.len()).position(|w| w == m)?;
        let Builder { mut pieces, entries, .. } = out;
        pieces[0] = Arc::new(shell[..at].to_vec());
        pieces.push(Arc::new(shell[at + m.len()..].to_vec()));
        Some((pieces, entries))
    }

    /// `document.xml` written from the whole body: every block becomes elements, and the root declares `w14` when
    /// anything in it uses it. What a save did before the blocks were kept, and what each save is checked against in
    /// tests and debug builds.
    pub(super) fn body_xml_reference(&self) -> Vec<u8> {
        let mut doc = self.doc_xml.clone();
        if let Some(b) = doc.root.child_mut("w:body") {
            write_blocks(&self.body, &mut b.children);
        }
        declare_w14(&mut doc.root);
        doc.to_bytes()
    }

    /// The whole body written again, for tests that compare it with what a save wrote.
    #[doc(hidden)]
    pub fn document_xml_whole(&self) -> Vec<u8> {
        self.body_xml_reference()
    }
}
