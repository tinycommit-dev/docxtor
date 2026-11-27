//! `Body`: the document's top-level blocks, each one shared (copy on write) with the undo steps that hold it.
//!
//! An undo step keeps the body as it was before the step. Only the top level is shared: a block is an `Arc<Block>`, and the
//! step holds another count on the same block until the live body changes it. Reading is as easy as with a `Vec<Block>`
//! (`iter` and `for b in &body` give `&Block`, `body[i]` gives `&Block`). Writing copies only what it changes: `get_mut`
//! and `IndexMut` clone one block, and only when a step still shares it (`Arc::make_mut`). A whole-body `iter_mut` would
//! clone every shared block, so it is named `iter_mut_all` and is only for where every block really changes; to change the
//! few blocks a pass cares about, find them with a read and use `get_mut`, or `iter_mut_where`.
//!
//! Blocks inside a table, a cell or a container stay plain `Vec<Block>` inside their block.

use crate::model::Block;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct Body(Vec<Arc<Block>>);

/// The one place a shared block is copied for writing: counts the copy when a step still shares the block.
#[inline]
fn cow(a: &mut Arc<Block>) -> &mut Block {
    if Arc::strong_count(a) > 1 {
        crate::perf::blocks_cloned(1);
    }
    Arc::make_mut(a)
}

#[inline]
fn unwrap(a: Arc<Block>) -> Block {
    Arc::try_unwrap(a).unwrap_or_else(|a| (*a).clone())
}

/// `&Block`s of a body, in order.
pub type Iter<'a> = std::iter::Map<std::slice::Iter<'a, Arc<Block>>, fn(&'a Arc<Block>) -> &'a Block>;

impl Body {
    pub fn new() -> Body {
        Body(Vec::new())
    }

    pub fn with_capacity(n: usize) -> Body {
        Body(Vec::with_capacity(n))
    }

    /// A body over blocks already shared (what an undo step holds).
    pub fn from_arcs(v: Vec<Arc<Block>>) -> Body {
        Body(v)
    }

    /// The shared blocks, for putting them in an undo step (reference counts only).
    pub fn arcs(&self) -> &[Arc<Block>] {
        &self.0
    }

    /// Every block as a plain `Vec`: a block no step shares moves, a shared one is copied. For taking the whole body out
    /// to work on it, and giving it back with `Body::from`.
    pub fn into_vec(self) -> Vec<Block> {
        self.0.into_iter().map(unwrap).collect()
    }

    /// A copy of every block (for a derived document, never for a read).
    pub fn to_vec(&self) -> Vec<Block> {
        self.0.iter().map(|a| (**a).clone()).collect()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.0.iter().map(|a| &**a)
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<&Block> {
        self.0.get(i).map(|a| &**a)
    }

    #[inline]
    pub fn first(&self) -> Option<&Block> {
        self.0.first().map(|a| &**a)
    }

    #[inline]
    pub fn last(&self) -> Option<&Block> {
        self.0.last().map(|a| &**a)
    }

    pub fn position(&self, f: impl FnMut(&Block) -> bool) -> Option<usize> {
        self.iter().position(f)
    }

    pub fn rposition(&self, f: impl FnMut(&Block) -> bool) -> Option<usize> {
        self.iter().rposition(f)
    }

    /// The block `i` for changing: copied only if an undo step still shares it.
    #[inline]
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Block> {
        self.0.get_mut(i).map(cow)
    }

    pub fn first_mut(&mut self) -> Option<&mut Block> {
        self.0.first_mut().map(cow)
    }

    pub fn last_mut(&mut self) -> Option<&mut Block> {
        self.0.last_mut().map(cow)
    }

    /// Change block `i` through `f`, which says whether it changed anything. A block an undo step shares is changed on a
    /// scratch copy that is put in only when `f` says it changed (so a pass that finds nothing to do copies nothing
    /// for good). Returns what `f` said.
    pub fn try_edit(&mut self, i: usize, f: impl FnOnce(&mut Block) -> bool) -> bool {
        let a = &mut self.0[i];
        if let Some(b) = Arc::get_mut(a) {
            return f(b);
        }
        let mut scratch = (**a).clone();
        let changed = f(&mut scratch);
        if changed {
            crate::perf::blocks_cloned(1);
            *a = Arc::new(scratch);
        }
        changed
    }

    /// Run `f` on every top-level block, as a one-block slice, for passes written over `&mut [Block]`. A block an undo
    /// step shares is changed on a scratch copy that goes in only when `f` says it changed; the ids of the blocks that
    /// changed are returned. For rare passes that have to look at every block; a pass that knows which blocks it cares
    /// about uses `edit_where`.
    pub fn edit_each(&mut self, mut f: impl FnMut(&mut [Block]) -> bool) -> Vec<crate::model::NodeId> {
        let mut changed = Vec::new();
        for i in 0..self.0.len() {
            let id = self.0[i].id();
            if self.try_edit(i, |b| f(std::slice::from_mut(b))) {
                changed.push(id);
            }
        }
        changed
    }

    /// Like `edit_each`, for the blocks `keep` accepts: only those are looked at, and only those that change are copied.
    pub fn edit_where(&mut self, mut keep: impl FnMut(&Block) -> bool, mut f: impl FnMut(&mut [Block]) -> bool) -> Vec<crate::model::NodeId> {
        let mut changed = Vec::new();
        for i in 0..self.0.len() {
            if !keep(&self.0[i]) {
                continue;
            }
            let id = self.0[i].id();
            if self.try_edit(i, |b| f(std::slice::from_mut(b))) {
                changed.push(id);
            }
        }
        changed
    }

    /// `walk_paragraphs_mut` for the body: `f` is called on every paragraph and says whether it changed it. A block an
    /// undo step shares is walked on a scratch copy that goes in only when something in it changed. Returns the ids of
    /// the top-level blocks that changed. For rare passes that have to look at every paragraph.
    pub fn edit_paragraphs(&mut self, f: &mut dyn FnMut(&mut crate::model::Paragraph) -> bool) -> Vec<crate::model::NodeId> {
        self.edit_each(|s| {
            let mut any = false;
            crate::model::walk_paragraphs_mut(s, &mut |p| any |= f(p));
            any
        })
    }

    /// Change the body as a plain `Vec` of blocks. Every block is copied out, and every block goes back as a new one when
    /// `f` says it changed anything, so a block no longer shares anything with an undo step: only for a change that
    /// marks every block of the body as changed anyway (`touch_all`). Returns what `f` said.
    pub fn edit_as_vec(&mut self, f: impl FnOnce(&mut Vec<Block>) -> bool) -> bool {
        let mut v = self.to_vec();
        let changed = f(&mut v);
        if changed {
            *self = Body::from(v);
        }
        changed
    }

    /// The blocks `keep` accepts, for changing: only those are copied from a step that shares them.
    pub fn iter_mut_where<'a>(&'a mut self, mut keep: impl FnMut(&Block) -> bool + 'a) -> impl Iterator<Item = &'a mut Block> + 'a {
        self.0.iter_mut().filter(move |a| keep(a)).map(cow)
    }

    /// Every block for changing: copies every block an undo step shares. Only for a change that really touches every
    /// block; anything else finds its blocks first and uses `get_mut`.
    pub fn iter_mut_all(&mut self) -> impl DoubleEndedIterator<Item = &mut Block> {
        self.0.iter_mut().map(cow)
    }

    pub fn push(&mut self, b: Block) {
        self.0.push(Arc::new(b));
    }

    pub fn insert(&mut self, i: usize, b: Block) {
        self.0.insert(i, Arc::new(b));
    }

    pub fn remove(&mut self, i: usize) -> Block {
        unwrap(self.0.remove(i))
    }

    pub fn pop(&mut self) -> Option<Block> {
        self.0.pop().map(unwrap)
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.0.swap(a, b);
    }

    pub fn truncate(&mut self, n: usize) {
        self.0.truncate(n);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn retain(&mut self, mut f: impl FnMut(&Block) -> bool) {
        self.0.retain(|a| f(a));
    }

    /// Replace the blocks in `range` with `with`; returns the ones taken out.
    pub fn splice(&mut self, range: impl std::ops::RangeBounds<usize>, with: impl IntoIterator<Item = Block>) -> Vec<Block> {
        self.0.splice(range, with.into_iter().map(Arc::new)).map(unwrap).collect()
    }

    /// Take the blocks in `range` out.
    pub fn drain(&mut self, range: impl std::ops::RangeBounds<usize>) -> Vec<Block> {
        self.0.drain(range).map(unwrap).collect()
    }

    pub fn split_off(&mut self, at: usize) -> Body {
        Body(self.0.split_off(at))
    }

    pub fn extend(&mut self, with: impl IntoIterator<Item = Block>) {
        self.0.extend(with.into_iter().map(Arc::new));
    }

    pub fn append(&mut self, other: &mut Body) {
        self.0.append(&mut other.0);
    }

    pub fn reserve(&mut self, n: usize) {
        self.0.reserve(n);
    }
}

impl From<Vec<Block>> for Body {
    fn from(v: Vec<Block>) -> Body {
        // Not `collect`: from a `Vec` of blocks it would reuse the blocks' allocation for the pointers, and keep all of it.
        // Room to grow: a block added to a document of exactly this many would otherwise copy all the pointers.
        let mut out = Vec::with_capacity(v.len() + v.len() / 8 + 8);
        out.extend(v.into_iter().map(Arc::new));
        Body(out)
    }
}

impl FromIterator<Block> for Body {
    fn from_iter<I: IntoIterator<Item = Block>>(it: I) -> Body {
        let it = it.into_iter();
        let n = it.size_hint().0;
        let mut out = Vec::with_capacity(n + n / 8 + 8);
        out.extend(it.map(Arc::new));
        Body(out)
    }
}

impl<'a> IntoIterator for &'a Body {
    type Item = &'a Block;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl Index<usize> for Body {
    type Output = Block;
    #[inline]
    fn index(&self, i: usize) -> &Block {
        &self.0[i]
    }
}

impl IndexMut<usize> for Body {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut Block {
        cow(&mut self.0[i])
    }
}

/// A list of blocks that can be read by position: a slice, a `Vec`, the body. For functions that walk a list by index.
pub trait Seq {
    fn seq_len(&self) -> usize;
    fn seq_at(&self, i: usize) -> &Block;
}

impl Seq for [Block] {
    fn seq_len(&self) -> usize {
        self.len()
    }
    fn seq_at(&self, i: usize) -> &Block {
        &self[i]
    }
}

impl Seq for Vec<Block> {
    fn seq_len(&self) -> usize {
        self.len()
    }
    fn seq_at(&self, i: usize) -> &Block {
        &self[i]
    }
}

impl Seq for Body {
    fn seq_len(&self) -> usize {
        self.len()
    }
    fn seq_at(&self, i: usize) -> &Block {
        &self[i]
    }
}

impl Seq for crate::model::ListMut<'_> {
    fn seq_len(&self) -> usize {
        self.len()
    }
    fn seq_at(&self, i: usize) -> &Block {
        &self[i]
    }
}

/// A list of blocks to read, whether it is the body or a list of blocks inside a story or a cell.
#[derive(Clone, Copy)]
pub enum BlocksRef<'a> {
    Top(&'a Body),
    List(&'a [Block]),
}

impl<'a> BlocksRef<'a> {
    pub fn len(&self) -> usize {
        match self {
            BlocksRef::Top(b) => b.len(),
            BlocksRef::List(l) => l.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, i: usize) -> Option<&'a Block> {
        match *self {
            BlocksRef::Top(b) => b.get(i),
            BlocksRef::List(l) => l.get(i),
        }
    }
}

pub enum BlocksIter<'a> {
    Top(Iter<'a>),
    List(std::slice::Iter<'a, Block>),
}

impl<'a> Iterator for BlocksIter<'a> {
    type Item = &'a Block;
    fn next(&mut self) -> Option<&'a Block> {
        match self {
            BlocksIter::Top(i) => i.next(),
            BlocksIter::List(i) => i.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            BlocksIter::Top(i) => i.size_hint(),
            BlocksIter::List(i) => i.size_hint(),
        }
    }
}

impl<'a> IntoIterator for BlocksRef<'a> {
    type Item = &'a Block;
    type IntoIter = BlocksIter<'a>;
    fn into_iter(self) -> BlocksIter<'a> {
        match self {
            BlocksRef::Top(b) => BlocksIter::Top(b.iter()),
            BlocksRef::List(l) => BlocksIter::List(l.iter()),
        }
    }
}
