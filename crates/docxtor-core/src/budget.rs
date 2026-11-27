//! The input budget: one limit every reader spends from, so no file from outside can crash, freeze or use up
//! the engine.
//!
//! A `Budget` is made where a file comes in (`open::open`, `Document::open`, a paste, a picture) and handed
//! down to every reader, nested ones included: the zip reader spends bytes and parts, a chunk that holds a
//! whole .docx spends from the same budget as the file around it, and slow paths spend work steps. When a
//! limit is hit the budget stays used up, and opening the file ends with one error: [`OverBudget`], whose
//! message a person can read.
//!
//! Two limits are not spent but fixed, because they keep the stack safe and every format passes through the
//! XML reader: how deep elements nest, and how deep blocks and inline containers nest (tables in cells, content controls, insertions). They are
//! enforced in `xml::parse` ([`XML_DEPTH`], [`NEST_LIMIT`]); the numbers are small enough that clone, drop,
//! render and save of a file at the limit fit in the 1 MB stack of WebAssembly (`tests/all/budget.rs` proves it).
//!
//! The per-format caps further down ([`caps`]) are the sizes the picture, paste and list readers already had,
//! kept in one place.

use std::cell::Cell;

/// How deep XML elements may nest. Real files go a few dozen levels (a text box in a table in a content
/// control); 256 leaves a wide margin and is still safe on a 1 MB stack.
pub const XML_DEPTH: usize = 256;

/// How much blocks and inline containers may nest inside each other, in weighted levels (see [`nest_cost`]): a
/// table in a cell in a table, a content control in a content control, a tracked insertion in a link in an
/// insertion. The reader, the renderer, the cloner and the writer recurse once per level, and a level of a table or
/// a text box costs far more stack than a content control, so the levels are weighed. 36 allows 12 tables in
/// tables, 3 text boxes in text boxes, or 18 content controls or insertions in each other: several times what
/// Word documents have, and measured to fit a 1 MB stack with room to spare in a build without optimising
/// (`tests/all/budget.rs`).
pub const NEST_LIMIT: usize = 36;

/// What one level of this element adds to the nesting (0 for an element that nothing recurses on).
pub fn nest_cost(name: &str) -> usize {
    match name {
        "w:txbxContent" => 12,
        "w:tbl" => 3,
        "w:sdt" | "w:ins" | "w:del" | "w:moveFrom" | "w:moveTo" | "w:hyperlink" | "w:smartTag" | "w:fldSimple" | "w:dir" | "w:bdo" => 2,
        "w:customXml" => 1,
        _ => 0,
    }
}

/// The sentence a person reads when a file runs the budget out.
pub const MESSAGE: &str = "This file is too large or too deeply nested to open.";

/// The limit that ran out. One error kind for every reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("This file is too large or too deeply nested to open.")]
pub struct OverBudget;

/// What a budget allows. `Limits::file()` is for a whole file; paste and pictures get smaller ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Bytes inflated from zips, in total, nested documents included.
    pub inflated: u64,
    /// Zip entries (parts) read, in total.
    pub parts: usize,
    /// Work steps: what slow paths spend, so no reader can run for minutes.
    pub work: u64,
    /// Documents inside documents (a .docx chunk of a .docx chunk), counting the file itself as one.
    pub nesting: usize,
    /// Imported chunks (`w:altChunk`) read, in total.
    pub chunks: usize,
    /// Tabs read from a file.
    pub tabs: usize,
}

impl Limits {
    /// A file the person opens. Sized well above anything a real document reaches: the longest fixture is a
    /// few hundred KB, and a book with many pictures inflates to tens of MB.
    pub const fn file() -> Limits {
        Limits { inflated: 512 << 20, parts: 20_000, work: 1 << 30, nesting: 3, chunks: 1_000, tabs: 500 }
    }

    /// Something pasted: a clipboard's HTML or RTF, an imported chunk's text.
    pub const fn paste() -> Limits {
        Limits { inflated: 64 << 20, parts: 2_000, work: 1 << 27, nesting: 2, chunks: 100, tabs: 500 }
    }

    /// One picture to convert (EMF, WMF, TIFF) or one small list (CSV, .xlsx, BibTeX, CSL).
    pub const fn small() -> Limits {
        Limits { inflated: 64 << 20, parts: 2_000, work: 1 << 27, nesting: 1, chunks: 0, tabs: 0 }
    }
}

thread_local! {
    /// Set by `xml::parse` when a part is nested too deeply. `xml::parse` has no budget in its signature (110
    /// call sites read parts after opening too), so it leaves this mark, and a budget that is asked whether it
    /// ran out looks at it too.
    static XML_OVERFLOW: Cell<bool> = const { Cell::new(false) };
}

/// Called by `xml::parse` when a part nests deeper than the limits allow.
pub(crate) fn mark_xml_overflow() {
    XML_OVERFLOW.with(|c| c.set(true));
}

/// What a budget has spent. Not `Sync`: a budget belongs to one open.
#[derive(Debug)]
pub struct Budget {
    limits: Limits,
    inflated: Cell<u64>,
    parts: Cell<usize>,
    work: Cell<u64>,
    chunks: Cell<usize>,
    nesting: Cell<usize>,
    out: Cell<bool>,
}

impl Default for Budget {
    /// The limits of a file, without starting a new open (see [`Budget::file`]).
    fn default() -> Budget {
        Budget::new(Limits::file())
    }
}

impl Budget {
    pub fn new(limits: Limits) -> Budget {
        Budget { limits, inflated: Cell::new(0), parts: Cell::new(0), work: Cell::new(0), chunks: Cell::new(0), nesting: Cell::new(1), out: Cell::new(false) }
    }

    /// A budget for a file the person opens. Starts a new open: clears the mark `xml::parse` may have left.
    pub fn file() -> Budget {
        XML_OVERFLOW.with(|c| c.set(false));
        Budget::new(Limits::file())
    }

    pub fn paste() -> Budget {
        Budget::new(Limits::paste())
    }

    pub fn small() -> Budget {
        Budget::new(Limits::small())
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Whether any limit has run out, here or in `xml::parse`.
    pub fn is_out(&self) -> bool {
        self.out.get() || XML_OVERFLOW.with(|c| c.get())
    }

    /// `Err` once any limit has run out. Readers check this before they return a document.
    pub fn check(&self) -> Result<(), OverBudget> {
        if self.is_out() {
            Err(OverBudget)
        } else {
            Ok(())
        }
    }

    fn run_out(&self) -> OverBudget {
        self.out.set(true);
        OverBudget
    }

    /// Bytes that can still be inflated.
    pub fn bytes_left(&self) -> u64 {
        self.limits.inflated.saturating_sub(self.inflated.get())
    }

    /// Spend `n` inflated bytes (or, for a reader that is handed bytes, the bytes it will read).
    pub fn spend_bytes(&self, n: u64) -> Result<(), OverBudget> {
        let now = self.inflated.get().saturating_add(n);
        self.inflated.set(now);
        if now > self.limits.inflated {
            return Err(self.run_out());
        }
        self.check()
    }

    /// Spend one zip entry.
    pub fn spend_part(&self) -> Result<(), OverBudget> {
        let now = self.parts.get() + 1;
        self.parts.set(now);
        if now > self.limits.parts {
            return Err(self.run_out());
        }
        Ok(())
    }

    /// Spend `n` work steps. A slow path calls this once for each unit of work it does, or once with the
    /// size of the work before it starts.
    pub fn spend_work(&self, n: u64) -> Result<(), OverBudget> {
        let now = self.work.get().saturating_add(n);
        self.work.set(now);
        if now > self.limits.work {
            return Err(self.run_out());
        }
        Ok(())
    }

    /// Spend one imported chunk. Unlike the other limits this does not use the budget up: a chunk that is
    /// over the limit stays a placeholder and the rest of the file opens.
    pub fn spend_chunk(&self) -> bool {
        let now = self.chunks.get() + 1;
        self.chunks.set(now);
        now <= self.limits.chunks
    }

    /// Go one document deeper (a .docx inside a .docx). `None` past the limit; this does not use the budget
    /// up, the inner document stays a placeholder. Dropping the guard comes back out.
    pub fn enter_document(&self) -> Option<Nesting<'_>> {
        if self.nesting.get() >= self.limits.nesting {
            return None;
        }
        self.nesting.set(self.nesting.get() + 1);
        Some(Nesting(self))
    }
}

/// Being inside one more document. Dropping it leaves that document.
pub struct Nesting<'a>(&'a Budget);

impl Drop for Nesting<'_> {
    fn drop(&mut self) {
        self.0.nesting.set(self.0.nesting.get() - 1);
    }
}

/// `Err` when an input of `len` bytes is longer than `cap`: the check every reader of a single input (a paste, a
/// picture, a list) makes first.
pub fn within(len: usize, cap: usize) -> Result<(), OverBudget> {
    if len > cap {
        Err(OverBudget)
    } else {
        Ok(())
    }
}

/// The sizes the picture, paste and list readers cap themselves at, kept in one place. They are about the size
/// of one input, not what a file spends in total.
pub mod caps {
    /// RTF groups nested.
    pub const RTF_DEPTH: usize = 200;
    /// One picture inside an RTF file.
    pub const RTF_PICT_BYTES: usize = 64 << 20;
    /// Cells in one RTF table row.
    pub const RTF_CELLS: usize = 256;
    /// OpenDocument elements nested.
    pub const ODT_DEPTH: usize = 100;
    /// Pixels of a TIFF (smaller in tests, which decode in a debug build).
    pub const TIFF_PIXELS: u64 = if cfg!(test) { 4_000_000 } else { 36_000_000 };
    /// Bytes a TIFF may decode to.
    pub const TIFF_BYTES: u64 = 1 << 30;
    /// Strips or tiles in one TIFF.
    pub const TIFF_BLOCKS: usize = 1 << 20;
    /// Rows of a recipient list.
    pub const LIST_ROWS: usize = 100_000;
    /// Columns of a recipient list.
    pub const LIST_COLS: usize = 16_384;
    /// Cells of a recipient list.
    pub const LIST_CELLS: usize = 4_000_000;
    /// The longest text the paste readers take (HTML and RTF).
    pub const PASTE_BYTES: usize = 64 << 20;
    /// The longest picture the converters take (EMF, WMF, TIFF).
    pub const PICTURE_BYTES: usize = 128 << 20;
    /// The longest list or bibliography file (CSV, BibTeX, CSL).
    pub const LIST_BYTES: usize = 64 << 20;
    /// The most a compressed metafile (.emz, .wmz) may unpack to.
    pub const METAFILE_BYTES: usize = 64 << 20;
    /// The longest op list `apply_json` takes.
    pub const OPS_BYTES: usize = 32 << 20;
}
