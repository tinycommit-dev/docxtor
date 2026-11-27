//! Imported content chunks (`w:altChunk`).
//!
//! Many tools that write .docx files (report generators, content systems, mail systems, libraries for Python,
//! .NET and Java) put HTML, a web page archive (MHT), plain text, RTF or a whole .docx inside a document as a
//! chunk: a relationship of type `aFChunk` to a part with a content type of its own. Word turns a chunk into
//! ordinary content when it opens the file. Docxtor shows it the same way and keeps the file as it came until the
//! chunk is changed.
//!
//! How it works:
//! - On open, each chunk is read by the converter for its kind (`convert`: the clipboard's HTML and RTF readers,
//!   the MIME reader in `mime`, plain text, or the .docx reader with the way a paste carries styles, lists and
//!   pictures). The result is a `Fragment`, brought into a private preview document (`AltChunk::doc`) that has this
//!   document's styles and lists plus whatever the chunk adds. The converted blocks sit in the body in a
//!   `Block::Container` whose shell is the original `w:altChunk` element.
//! - The container is saved as that element alone, so opening and saving leaves the element and its part byte
//!   for byte as they were. Reading it (find, word count, the AI tools, export, compare) sees the blocks.
//! - It is drawn in place, with the preview's styles, lists and pictures, as if its blocks were the document's own
//!   (`RenderCtx::alt_blocks`): they page, wrap and select like any other text.
//! - The first op that changes anything inside a chunk makes it real: `expand_alt_chunk` does the same conversion
//!   into the document itself (styles and lists merged as a paste does), replaces the container with the blocks
//!   (their ids stay), and drops the relationship and the part when nothing else uses them. The batch that
//!   asked for the change then runs on the real content, in the same undo step. The op is also a command ("Make
//!   editable") for doing it without a change.
//!
//! Chunks are read in the body, table cells and content controls. Headers, footers and notes keep theirs as an
//! "Embedded content" placeholder.

pub mod convert;
pub mod mime;

use crate::document::Document;
use crate::fragment::Fragment;
use crate::model::{Block, BlockContainer, NodeId};
use crate::render::{RBlock, RenderCtx};

pub use convert::Kind;

/// An imported chunk as it is shown: what it holds, and the preview document the converted blocks use.
pub struct AltChunk {
    pub kind: Kind,
    /// Styles, lists and pictures of the converted blocks (never saved).
    pub(crate) doc: Document,
    /// The converted content, as a fragment, for `expand_alt_chunk`.
    pub(crate) fragment: Fragment,
    /// The part holding the `w:altChunk` and the relationship id it names.
    pub(crate) owner: String,
    pub(crate) rid: String,
    /// The ids the converted blocks were given: `first..=last`.
    pub(crate) first: NodeId,
    pub(crate) last: NodeId,
}

impl AltChunk {
    /// Whether `id` names a block inside this chunk.
    pub fn holds(&self, id: NodeId) -> bool {
        id >= self.first && id <= self.last
    }
}

impl RenderCtx<'_> {
    /// Draw a chunk's blocks in place with its own styles, lists and pictures, marking each as part of the chunk.
    pub(crate) fn alt_blocks(&mut self, c: &BlockContainer, out: &mut Vec<RBlock>) {
        let alts = self.alts;
        let Some(alt) = alts.and_then(|m| m.get(&c.id)) else {
            out.extend(self.blocks(&c.blocks));
            return;
        };
        let labels = alt.doc.numbering.labels(&c.blocks, &alt.doc.styles);
        let mut sub = RenderCtx::new(&alt.doc.styles, &alt.doc.numbering, &alt.doc.pkg, &alt.doc.main_path, &labels);
        for mut r in sub.blocks(&c.blocks) {
            match &mut r {
                RBlock::Para(p) => p.chunk = Some(c.id),
                RBlock::Table(t) => t.chunk = Some(c.id),
                _ => {}
            }
            out.push(r);
        }
    }
}

/// Whether some block, in a table cell or a content control too, is a chunk element not yet converted.
pub(crate) fn has_unloaded<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> bool {
    blocks.into_iter().any(|b| match b {
        Block::Opaque(o) => o.el.name == "w:altChunk",
        Block::Table(t) => t.rows().any(|r| r.cells().any(|c| has_unloaded(&c.blocks))),
        Block::Container(c) => has_unloaded(&c.blocks),
        _ => false,
    })
}
