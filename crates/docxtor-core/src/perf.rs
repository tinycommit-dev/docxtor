//! Counters and test switches for the performance harness (see docs/performance/index.md).
//!
//! `BODY_CLONES` counts every copy of the whole document body (a selection copy, a comparison); an undo snapshot copies
//! only the blocks an edit touched and counts them in `BLOCK_CLONES`. Both are always on. The harness reads them through `perfCounters` and gates on them,
//! because a count does not move with how busy the machine is.
//!
//! The "plants" exist only with the `perf-plant` feature. They are deliberate regressions the harness must catch
//! (docs/performance/index.md, "Planted regressions"): a busy loop on every edit, an allocation on open, a second copy of
//! the body on every edit. A normal build has none of this code.

use std::cell::Cell;
#[cfg(feature = "perf-plant")]
use std::sync::atomic::Ordering::Relaxed;

// Per thread, so a test that reads them is not thrown off by tests running beside it (the program the harness measures
// runs on one thread).
thread_local! {
    static BODY_CLONES: Cell<u64> = const { Cell::new(0) };
    static BLOCK_CLONES: Cell<u64> = const { Cell::new(0) };
    static PARAGRAPHS_WALKED: Cell<u64> = const { Cell::new(0) };
    static BLOCKS_WRITTEN: Cell<u64> = const { Cell::new(0) };
    static PARTS_JOINED: Cell<u64> = const { Cell::new(0) };
    #[cfg(any(test, debug_assertions))]
    static UNMETERED: Cell<bool> = const { Cell::new(false) };
}

/// Run a check that walks the document again (the debug comparisons of `document/facts.rs`) without counting what it
/// walks: the count is about the work the engine does, not about what verifies it.
#[cfg(any(test, debug_assertions))]
pub fn unmetered<T>(f: impl FnOnce() -> T) -> T {
    let was = UNMETERED.with(|u| u.replace(true));
    let out = f();
    UNMETERED.with(|u| u.set(was));
    out
}

/// `unmetered` where it exists, and just the call where it does not (release builds): for the checks the invariants run.
#[inline]
pub fn unmetered_check<T>(f: impl FnOnce() -> T) -> T {
    #[cfg(any(test, debug_assertions))]
    return unmetered(f);
    #[cfg(not(any(test, debug_assertions)))]
    f()
}

/// Note that a walk over the document (`walk_paragraphs` and its variants) visited a paragraph.
#[inline]
pub fn paragraph_walked() {
    #[cfg(any(test, debug_assertions))]
    if UNMETERED.with(Cell::get) {
        return;
    }
    PARAGRAPHS_WALKED.with(|c| c.set(c.get() + 1));
}

/// Paragraphs visited by walks over the document since the program started. Typing should not make this grow with the
/// length of the document.
pub fn paragraphs_walked() -> u64 {
    PARAGRAPHS_WALKED.with(Cell::get)
}

/// Note that `n` top-level blocks were written out as XML for a save (the others were copied from the last save).
#[inline]
pub fn blocks_written(n: usize) {
    BLOCKS_WRITTEN.with(|c| c.set(c.get() + n as u64));
}

/// Top-level blocks written out as XML by saves since the program started: one key, one block.
pub fn blocks_written_total() -> u64 {
    BLOCKS_WRITTEN.with(Cell::get)
}

/// Note that a part kept as pieces was joined into one buffer (a rare path: saves, edits and idle work never do).
#[inline]
pub fn part_joined() {
    #[cfg(any(test, debug_assertions))]
    if UNMETERED.with(Cell::get) {
        return;
    }
    PARTS_JOINED.with(|c| c.set(c.get() + 1));
}

/// Times a part kept as pieces was joined into one buffer since the program started.
pub fn parts_joined() -> u64 {
    PARTS_JOINED.with(Cell::get)
}

/// Note that the whole body was copied.
#[inline]
pub fn body_cloned() {
    BODY_CLONES.with(|c| c.set(c.get() + 1));
}

/// Note that `n` top-level blocks were copied for an undo step (a snapshot copies only the blocks an edit touched).
#[inline]
pub fn blocks_cloned(n: usize) {
    BLOCK_CLONES.with(|c| c.set(c.get() + n as u64));
}

/// Top-level blocks copied for undo steps since the program started.
pub fn block_clones() -> u64 {
    BLOCK_CLONES.with(Cell::get)
}

/// Whole-body copies since the program started.
pub fn body_clones() -> u64 {
    BODY_CLONES.with(Cell::get)
}

#[cfg(feature = "perf-plant")]
mod plant_on {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    pub static BUSY: AtomicU64 = AtomicU64::new(0);
    pub static ALLOC_MB: AtomicU64 = AtomicU64::new(0);
    pub static DOUBLE_CLONE: AtomicU64 = AtomicU64::new(0);
    pub static INIT: std::sync::Once = std::sync::Once::new();

    /// Natively the plants are read once from `DOCXTOR_PLANT_BUSY` (loop iterations per edit), `DOCXTOR_PLANT_ALLOC_MB`
    /// (megabytes allocated and freed on open) and `DOCXTOR_PLANT_DOUBLE_CLONE` (1: a second body copy per edit).
    pub fn init() {
        INIT.call_once(|| {
            let get = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            BUSY.fetch_max(get("DOCXTOR_PLANT_BUSY"), Relaxed);
            ALLOC_MB.fetch_max(get("DOCXTOR_PLANT_ALLOC_MB"), Relaxed);
            DOUBLE_CLONE.fetch_max(get("DOCXTOR_PLANT_DOUBLE_CLONE"), Relaxed);
        });
    }
}

/// Set a plant by name (`busy`, `alloc_mb`, `double_clone`); for the browser and Node, where there are no environment variables.
#[cfg(feature = "perf-plant")]
pub fn set_plant(name: &str, value: u64) {
    plant_on::INIT.call_once(|| {});
    match name {
        "busy" => plant_on::BUSY.store(value, Relaxed),
        "alloc_mb" => plant_on::ALLOC_MB.store(value, Relaxed),
        "double_clone" => plant_on::DOUBLE_CLONE.store(value, Relaxed),
        _ => {}
    }
}

/// Planted: spin for a while on every edit.
#[inline]
pub fn plant_edit() {
    #[cfg(feature = "perf-plant")]
    {
        plant_on::init();
        let n = plant_on::BUSY.load(Relaxed);
        let mut x = 0u64;
        for i in 0..n {
            x = std::hint::black_box(x.wrapping_add(i).rotate_left(1));
        }
        std::hint::black_box(x);
    }
}

/// Planted: allocate and free memory when a document opens.
#[inline]
pub fn plant_open() {
    #[cfg(feature = "perf-plant")]
    {
        plant_on::init();
        let mb = plant_on::ALLOC_MB.load(Relaxed) as usize;
        if mb > 0 {
            let v = std::hint::black_box(vec![1u8; mb << 20]);
            drop(v);
        }
    }
}

/// Planted: whether to copy the body a second time on every edit.
#[inline]
pub fn plant_double_clone() -> bool {
    #[cfg(feature = "perf-plant")]
    {
        plant_on::init();
        return plant_on::DOUBLE_CLONE.load(Relaxed) > 0;
    }
    #[cfg(not(feature = "perf-plant"))]
    false
}
