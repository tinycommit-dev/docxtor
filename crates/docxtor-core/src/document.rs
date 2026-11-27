//! `Document`: the package, the editable body, styles and lists, plus undo history.

use crate::docmeta;
use crate::docprops;
use crate::edit;
use crate::markdown::{self, MdBlock, MdSpan};
use crate::marks::{self, BookmarkInfo, Watermark, WatermarkLayout};
use crate::model::*;
use crate::notes;
use crate::numbering::{self, ListKind, ListLabel, Numbering};
use crate::ops::*;
use crate::package::{self, Package, PackageError};
use crate::props::{put_ppr, ParaFmt, ParaPatch, RunFmt, RunPatch};
use crate::render::{RBlock, RenderCtx};
use crate::stories::{self, Story, StoryKind, StoryParts};
use crate::styles::{StyleInfo, StyleSheet, ThemeFonts};
use crate::toc;
use facts::NoteEv;
use crate::xml::{self, AttrValue, Element, Name, Node, XmlDoc};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};

/// What the page tells the engine for the next save (see `Document::set_save_info`).
#[derive(Debug, Clone, Default)]
pub struct SaveInfo {
    /// Who is saving: becomes `cp:lastModifiedBy`.
    pub user: Option<String>,
    /// How many pages and lines the document lays out in, for `Pages` and `Lines` of `app.xml`.
    pub pages: Option<u32>,
    pub lines: Option<u32>,
}

// Citations and the bibliography (the sources part, citation fields); see `citations.rs`.
mod cite;
mod style_refs;
mod change_ids;

// Charts: inserting and changing them (see `chart`).
mod charts;
mod charfmt;

mod equations;
mod paste;
mod alt_chunks;
// The rules checked after every edit. A child of this module because it reads fields the document keeps private;
// the file sits beside `lib.rs`, which re-exports it as `docxtor_core::invariants`.
#[path = "invariants.rs"]
pub mod invariants;
mod paste_safe;
mod picture_watermark;
mod media_prune;
mod pictures;
mod header_link;
mod page_labels;
mod references;
mod arrange;
mod object_props;
mod geometry;
mod connectors;
mod shapes;
mod updating;
mod inspect;
mod field_ops;
pub use field_ops::FieldInfo;
mod table_design;
// Content controls, legacy form fields and document protection.
mod controls;
mod after_batch;
mod render_key;
mod facts;
mod body_xml;
#[doc(hidden)]
pub use body_xml::set_piece_limit;
mod bookmark_halves;
pub(crate) mod prop_binding;
mod cover_ops;
pub use cover_ops::{GalleryDesign, GalleryLabels};
mod protect;
mod selection_copy;
// Phonetic guides, enclosed characters and East Asian line-breaking settings.
mod east_asian_ops;
// Document tabs: the ops, and keeping the body in step with the tab list.
mod tabs;

pub use protect::{EditRange, ProtectionInfo};
mod table_ops;
pub use table_ops::TableDetails;
mod lists;
mod layout_ops;
mod find;
mod goto;
mod toc_ops;
mod toa_ops;
mod chapters;
mod glossary;
pub use glossary::{BuildingBlock, DocPartControl};
pub use chapters::NumberedItem;
pub use goto::GotoTarget;
mod outline;
pub use outline::OutlineEntry;
pub use find::{Direction, FindFormat, FindHit, FindOptions, FindResult};
// The theme, style sets, paragraph spacing and editing styles.
mod design;
// Mail merge, labels and envelopes.
mod mail_merge;
pub use mail_merge::{MergeData, MergeInfo, MergeRange, MergeResult, PreviewInfo};
mod reveal;
pub use reveal::{Reveal, RevealRow};

pub use cite::CslStyleInfo;
pub use equations::EquationAt;
pub use inspect::{Finding, CATEGORIES};
pub use references::{XrefTargets, XHeading, XBookmark, XCaption, XNote};

// The methods of `Document`, by topic: opening and saving, undo, applying ops, rendering, and one file per kind of edit.
mod open_save;
mod undo;
mod dispatch;
mod lookup;
mod render_out;
pub use render_out::{render_json, render_json_pieces};
mod sections;
mod watermarks;
mod doc_settings;
mod inserts;
mod bookmarks;
mod fonts;
mod toc_edit;
mod text_ops;
mod list_ops;
mod table_build;
mod paste_ops;
pub(crate) use paste_ops::strip_link_style;
use fonts::{story_fonts_of, FontsKept, StoryFonts};
use paste_ops::{drawing_xml, relative_target};
use sections::{set_sect_type, text_area};
use table_build::{first_para_blocks, first_para_in, with_table_body};

const NS_W: &str ="http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const SECT_ORDER: &[&str] = &[
    "w:headerReference", "w:footerReference", "w:footnotePr", "w:endnotePr", "w:type", "w:pgSz", "w:pgMar",
    "w:paperSrc", "w:pgBorders", "w:lnNumType", "w:pgNumType", "w:cols", "w:formProt", "w:vAlign", "w:noEndnote",
    "w:titlePg", "w:textDirection", "w:bidi", "w:rtlGutter", "w:docGrid", "w:printerSettings", "w:sectPrChange",
];

fn is_zero(v: &i64) -> bool {
    *v == 0
}

/// Schema order of the start of `w:settings`; elements not listed (and anything after `w:rsids`) stay after these.
pub(crate) const SETTINGS_ORDER: &[&str] = &[
    "w:writeProtection", "w:view", "w:zoom", "w:removePersonalInformation", "w:removeDateAndTime", "w:doNotDisplayPageBoundaries",
    "w:displayBackgroundShape", "w:printPostScriptOverText", "w:printFractionalCharacterWidth", "w:printFormsData",
    "w:embedTrueTypeFonts", "w:embedSystemFonts", "w:saveSubsetFonts", "w:saveFormsData", "w:mirrorMargins",
    "w:alignBordersAndEdges", "w:bordersDoNotSurroundHeader", "w:bordersDoNotSurroundFooter", "w:gutterAtTop",
    "w:hideSpellingErrors", "w:hideGrammaticalErrors", "w:activeWritingStyle", "w:proofState", "w:formsDesign",
    "w:attachedTemplate", "w:linkStyles", "w:stylePaneFormatFilter", "w:stylePaneSortMethod", "w:documentType",
    "w:mailMerge", "w:revisionView", "w:trackRevisions", "w:doNotTrackMoves", "w:doNotTrackFormatting",
    "w:documentProtection", "w:autoFormatOverride", "w:styleLockTheme", "w:styleLockQFSet", "w:defaultTabStop",
    "w:autoHyphenation", "w:consecutiveHyphenLimit", "w:hyphenationZone", "w:doNotHyphenateCaps", "w:showEnvelope",
    "w:summaryLength", "w:clickAndTypeStyle", "w:defaultTableStyle", "w:evenAndOddHeaders", "w:bookFoldRevPrinting",
    "w:bookFoldPrinting", "w:bookFoldPrintingSheets", "w:drawingGridHorizontalSpacing", "w:drawingGridVerticalSpacing",
    "w:displayHorizontalDrawingGridEvery", "w:displayVerticalDrawingGridEvery", "w:doNotUseMarginsForDrawingGridOrigin",
    "w:drawingGridHorizontalOrigin", "w:drawingGridVerticalOrigin", "w:doNotShadeFormData", "w:noPunctuationKerning",
    "w:characterSpacingControl", "w:printTwoOnOne", "w:strictFirstAndLastChars", "w:noLineBreaksAfter",
    "w:noLineBreaksBefore", "w:savePreviewPicture", "w:doNotValidateAgainstSchema", "w:saveInvalidXml",
    "w:ignoreMixedContent", "w:alwaysShowPlaceholderText", "w:doNotDemarcateInvalidXml", "w:saveXmlDataOnly",
    "w:useXSLTWhenSaving", "w:saveThroughXslt", "w:showXMLTags", "w:alwaysMergeEmptyNamespace", "w:updateFields",
    "w:hdrShapeDefaults", "w:footnotePr", "w:endnotePr", "w:compat", "w:docVars", "w:rsids",
];

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error(transparent)]
    Package(#[from] PackageError),
    #[error(transparent)]
    Xml(#[from] xml::XmlError),
    #[error("the main document part has no w:body")]
    NoBody,
    #[error("{0}")]
    Open(String),
}

#[derive(Clone)]
struct Snapshot {
    /// The top-level blocks, each shared with the steps around it while it stays as it was (see `Document::shared_body`).
    body: std::sync::Arc<Vec<std::sync::Arc<Block>>>,
    styles: Option<std::sync::Arc<XmlDoc>>,
    numbering: Option<std::sync::Arc<XmlDoc>>,
    stories: Vec<Story>,
    /// The parsed story parts (footnotes, headers...), shared with the steps around while unchanged.
    story_parts: Vec<(String, std::sync::Arc<XmlDoc>)>,
    /// The page background (`w:background`) of the main part.
    background: Option<Element>,
    /// The attributes of the main part's root (the namespace declarations ops add with `ensure_ns`).
    root_attrs: Vec<(Name, AttrValue)>,
    settings: Option<std::sync::Arc<XmlDoc>>,
    settings_dirty: bool,
    core: Option<XmlDoc>,
    core_dirty: bool,
    /// `docProps/custom.xml` (where `_MarkAsFinal` lives).
    custom: Option<XmlDoc>,
    custom_dirty: bool,
    /// `docProps/app.xml` (company and manager).
    app: Option<XmlDoc>,
    app_dirty: bool,
    cites: crate::citations::CiteState,
    /// The recipients of a mail merge.
    merge: crate::merge::state::MergeState,
    /// The document's tabs.
    tabs: crate::tabs::TabsState,
    /// The theme part (`word/theme/theme1.xml`).
    theme: std::sync::Arc<design::ThemePart>,
    scrub: inspect::Scrub,
    /// The package's relationship parts, content types and added parts as they were before the step (see `Package::begin_journal`).
    pkg_delta: Vec<package::PartDelta>,
    /// About how much memory the step takes (see `Snapshot::weigh`); set when it goes on a stack.
    bytes: usize,
    /// The size of the part copies this step made fresh instead of sharing them with the step before.
    fresh_bytes: usize,
    /// The size of the blocks this step copied fresh instead of sharing them with the step before.
    fresh_block_bytes: usize,
}

impl Snapshot {
    /// A block anywhere in the step's body.
    fn find_block(&self, id: NodeId) -> Option<&Block> {
        self.body.iter().find_map(|b| find_block(std::slice::from_ref(&**b), id))
    }

    /// Work out `bytes`: what the step owns. The blocks it shares with other steps are counted once, by the step that
    /// copied them, so a long run of one-character edits in a big document takes a few kilobytes a step.
    fn weigh(mut self) -> Snapshot {
        self.bytes = self.fresh_block_bytes
            + self.stories.iter().map(|st| crate::model::blocks_bytes(&st.blocks)).sum::<usize>()
            + self.pkg_delta.iter().map(|(n, p)| n.len() + p.as_ref().map_or(0, |p| p.data.len())).sum::<usize>()
            + self.fresh_bytes;
        self
    }
}

/// How many undo steps are kept, and about how much memory they may take together. Each step holds a copy of the
/// body, so a long document's steps are big: the oldest go first when either limit is passed.
const UNDO_STEPS: usize = 500;
const UNDO_BYTES: usize = 128 << 20;
/// Steps kept whatever they weigh, so undo works in a document too big for the budget.
const UNDO_LEAST: usize = 10;

/// The big XML parts as the last undo step copied them. A step that leaves a part as it was shares that copy: a
/// Word file's `styles.xml` alone is hundreds of kilobytes, which would be cloned again for every keystroke.
#[derive(Default)]
struct PartCopies {
    styles: Option<std::sync::Arc<XmlDoc>>,
    numbering: Option<std::sync::Arc<XmlDoc>>,
    /// `Numbering::gen` when `numbering` was copied: while it is the same, the part has not changed, so a snapshot
    /// shares the copy without comparing (the part can be megabytes in a long document with many lists).
    numbering_gen: u64,
    settings: Option<std::sync::Arc<XmlDoc>>,
    story_parts: Vec<(String, std::sync::Arc<XmlDoc>)>,
    /// The top-level blocks as the last snapshot (or restore) left them, in order. A block whose id has not been touched
    /// since is the same as its copy here, and a snapshot shares the copy instead of cloning the block. The list is the
    /// last snapshot's own (shared with it), so a step does not make a second list of the same pointers.
    blocks: Option<std::sync::Arc<Vec<std::sync::Arc<Block>>>>,
}

/// The top-level blocks that changed and have to be drawn again, and, separately, those that changed since the undo
/// copies were last made (`PartCopies::blocks`). Rendering empties the first; only a snapshot or a restore empties the
/// second, so every place that marks a block as changed for drawing also marks it for copying.
#[derive(Default)]
struct Changed {
    now: BTreeSet<NodeId>,
    since_copy: BTreeSet<NodeId>,
    /// Changed since `refresh_facts` last read the blocks (see `facts.rs`).
    facts: BTreeSet<NodeId>,
    /// Changed since `body_has_shapes` last looked (see `facts.rs`).
    shapes: BTreeSet<NodeId>,
    /// Changed since `word_count` last counted (see `facts.rs`).
    words: BTreeSet<NodeId>,
    /// Changed since `body_has_locks` last looked (see `facts.rs`).
    locks: BTreeSet<NodeId>,
    /// Changed since `body_unpaired_bookmarks` last looked (see `facts.rs`).
    bookmarks: BTreeSet<NodeId>,
    /// Changed since `flush_parts` last wrote the body (see `body_xml.rs`).
    write: BTreeSet<NodeId>,
}

impl std::ops::Deref for Changed {
    type Target = BTreeSet<NodeId>;
    fn deref(&self) -> &BTreeSet<NodeId> {
        &self.now
    }
}

impl Changed {
    fn insert(&mut self, id: NodeId) -> bool {
        self.since_copy.insert(id);
        self.facts.insert(id);
        self.shapes.insert(id);
        self.words.insert(id);
        self.locks.insert(id);
        self.bookmarks.insert(id);
        self.write.insert(id);
        self.now.insert(id)
    }
    /// Mark a block for drawing again although what it holds did not change (its list label or the notes before it
    /// did): the copies and the kept facts stay as they are.
    fn insert_redraw(&mut self, id: NodeId) {
        self.now.insert(id);
    }
    fn extend(&mut self, ids: impl IntoIterator<Item = NodeId>) {
        for id in ids {
            self.insert(id);
        }
    }
    /// Forget that the block has to be drawn again. It stays marked as changed for copying.
    fn remove(&mut self, id: &NodeId) -> bool {
        self.now.remove(id)
    }
    /// Forget what has to be drawn again (the copies are not touched).
    fn clear(&mut self) {
        self.now.clear();
    }
    /// Hand out what has to be drawn again; the copies' marks stay.
    fn take_now(&mut self) -> BTreeSet<NodeId> {
        std::mem::take(&mut self.now)
    }
    fn replace_now(&mut self, now: BTreeSet<NodeId>) -> BTreeSet<NodeId> {
        std::mem::replace(&mut self.now, now)
    }
}

/// A copy of `now` for a snapshot: the one kept from the step before when `now` has not changed since. `fresh`
/// grows by the size of a new copy.
fn shared_copy(kept: &mut Option<std::sync::Arc<XmlDoc>>, now: &Option<XmlDoc>, fresh: &mut usize) -> Option<std::sync::Arc<XmlDoc>> {
    let doc = now.as_ref()?;
    if let Some(k) = kept {
        if **k == *doc {
            return Some(k.clone());
        }
    }
    *fresh += crate::model::xml_doc_bytes(doc);
    let k = std::sync::Arc::new(doc.clone());
    *kept = Some(k.clone());
    Some(k)
}

/// The story parts for a snapshot, sharing each part's copy from the step before while its content is unchanged.
fn shared_story_parts(kept: &mut Vec<(String, std::sync::Arc<XmlDoc>)>, now: &StoryParts, fresh: &mut usize) -> Vec<(String, std::sync::Arc<XmlDoc>)> {
    let next: Vec<(String, std::sync::Arc<XmlDoc>)> = now
        .parts
        .iter()
        .map(|(name, doc)| match kept.iter().find(|(n, d)| n == name && **d == *doc) {
            Some((_, d)) => (name.clone(), d.clone()),
            None => {
                *fresh += crate::model::xml_doc_bytes(doc);
                (name.clone(), std::sync::Arc::new(doc.clone()))
            }
        })
        .collect();
    *kept = next.clone();
    next
}

/// What `Document::change_marks` keeps.
struct ChangeMarks {
    body: bool,
    styles: bool,
    numbering: bool,
    settings: bool,
    core: bool,
    custom: bool,
    app: bool,
    charts: bool,
    theme: bool,
    cites: bool,
    merge: bool,
    tabs: bool,
    parts: BTreeSet<String>,
}

/// Who may change the document, set by the host (the page, a server, the CLI). It is checked in one place, where edits
/// are applied: `apply_batch`, `undo`, `redo` and what builds on them, and `try_add_media`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    /// Everything is allowed (the default).
    #[default]
    Edit,
    /// Nothing a person wrote may change: only ops where `Op::is_view_only` holds are applied.
    ViewOnly,
}

pub struct Document {
    pub pkg: Package,
    pub main_path: String,
    doc_xml: XmlDoc,
    pub body: Body,
    pub styles: StyleSheet,
    pub numbering: Numbering,
    styles_path: String,
    numbering_path: String,
    /// `word/settings.xml`, loaded when the package has one (created by the ops that need it).
    settings: Option<XmlDoc>,
    settings_path: String,
    settings_dirty: bool,
    /// `docProps/core.xml` (title and other properties), loaded when the package has one.
    core: Option<XmlDoc>,
    core_path: String,
    core_dirty: bool,
    /// `docProps/custom.xml`, loaded when the package has one (created when the document is marked as final).
    custom: Option<XmlDoc>,
    custom_path: String,
    custom_dirty: bool,
    /// `docProps/app.xml` (company, manager and the statistics Word keeps), loaded when the package has one.
    app: Option<XmlDoc>,
    app_path: String,
    app_dirty: bool,
    /// What the page tells the engine for the next save: who is saving, and the page and line counts.
    save_info: SaveInfo,
    /// The groups the user counts as when editable ranges name one (`everyone` always applies).
    user_groups: Vec<String>,
    /// The list of sources citations refer to (`customXml/item1.xml`).
    cites: crate::citations::CiteState,
    /// The recipients of a mail merge (a custom XML part of Docxtor's own), and the record shown in place of the template.
    merge: crate::merge::state::MergeState,
    merge_preview: Option<mail_merge::Previewing>,
    /// Who may change the document (see `Access`).
    access: Access,
    /// The tabs (a custom XML part of Docxtor's own) and the tab that is open.
    tabs: crate::tabs::TabsState,
    /// Chart parts changed since the last save.
    charts_dirty: bool,
    /// The theme part as of the last theme op (see `Snapshot::theme`); changed since the last save when `theme_dirty`.
    theme: std::sync::Arc<design::ThemePart>,
    /// The colours and lines of `theme`, read on the first render after it changes (`write_theme` empties it) and given to
    /// every render (see `ShapeEnv::known_theme`). Not read when the file opens, where it would add to the peak of memory.
    shape_theme: std::sync::OnceLock<crate::shapes::theme::ShapeTheme>,
    theme_dirty: bool,
    ids: IdGen,
    body_dirty: bool,
    changed: Changed,
    /// The name of the first op of the batch being applied, for the check in `shared_body`.
    #[cfg(any(test, debug_assertions))]
    snap_op: String,
    /// Per block, what the whole-document passes read (see `facts.rs`).
    facts: facts::Facts,
    shapes: facts::ShapeFacts,
    words: facts::WordFacts,
    locks: facts::LockFacts,
    bookmarks: facts::BookmarkFacts,
    /// Where each top-level block is in the `document.xml` the last save wrote (see `body_xml.rs`).
    body_xml: body_xml::BodyXml,
    last_labels: HashMap<NodeId, ListLabel>,
    last_breaks: HashMap<NodeId, String>,
    /// The visible block order as the last render gave it (`render_changed` sends it again only when it moved).
    last_order: Vec<NodeId>,
    /// What the drawing of headers, footers and other blocks depends on, as of the last render (see `render_key.rs`).
    last_key: render_key::RenderKey,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// What the undo steps share of the big parts (see `PartCopies`).
    copies: PartCopies,
    /// About how much memory the undo steps may take together.
    undo_budget: usize,
    /// What each step did (the name of its first op), in step with `undo` and `redo`, for the undo list.
    undo_labels: Vec<String>,
    /// Goes up by one each time the document changes (a batch that changed something, undo, redo, revert). Not saved.
    revision: u64,
    /// Goes up with every change, the quiet ones too (a batch that keeps no undo step): what `fonts_stories` was read at.
    generation: u64,
    /// Set while `open` shows the values of bound controls: nothing done then is an edit (see `ensure_placeholder_style`).
    opening: bool,
    /// The fonts each top-level block was last drawn with (`render_all`, `render_changed`), for `fonts_used`.
    fonts_body: FontsKept,
    /// The fonts of the headers, footers and footnotes as the last `page_setup` drew them, and the `generation` it was at.
    fonts_stories: std::sync::Mutex<Option<StoryFonts>>,
    redo_labels: Vec<String>,
    /// Undo steps evicted by the history cap, so checkpoints stay monotonic.
    undo_base: usize,
    /// Checkpoints held by callers (see `pin`): the budget never lets go of the steps from the oldest of them on.
    pins: Vec<usize>,
    drawing_id: u32,
    /// Headers, footers and footnotes: separately stored bodies addressed by the same ids.
    pub stories: Vec<Story>,
    story_parts: StoryParts,
    /// Story parts that must be rewritten on save.
    dirty_parts: BTreeSet<String>,
    /// Header parts whose watermark was set or removed: their unused image relationships are dropped on save.
    watermark_headers: BTreeSet<String>,
    /// Made by `new_blank` and not saved yet: the first `save_stamped` records its creation date.
    fresh: bool,
    story_gen: u64,
    /// Header and footer parts that may end up with no reference (new ones, or ones a link to previous let go);
    /// the saved file leaves them out when nothing refers to them (see `header_link`).
    hf_candidates: BTreeSet<String>,
    /// Pictures, charts and workbooks nothing used when the file came in, found before the first edit (None until
    /// then); saving keeps them and leaves out the others that nothing uses (see `media_prune`).
    baseline_orphans: Option<BTreeSet<String>>,
    /// While an op runs inside a story, the main body is parked here and `body` holds the story.
    parked: Option<Body>,
    /// Styles changed while an op ran in a story: the main text is drawn again when it returns.
    restyled_main: bool,
    /// Part whose relationships new links/images go into (the main part or the active story's).
    active_part: String,
    /// Set when a story changed since the last render (the UI re-fetches them).
    stories_touched: bool,
    /// The stories the batch now running has edited, and what they were before (see `after_batch`).
    touched_stories: Vec<after_batch::StoryTouch>,
    last_notes: HashMap<String, u32>,
    /// CSL citation styles registered for this document, by id (see `document/cite.rs`). Not saved.
    csl_styles: HashMap<String, std::sync::Arc<crate::csl::Style>>,
    /// Parts the saved file leaves out or rewrites (Inspect document, Remove all); part of undo.
    scrub: inspect::Scrub,
    /// The imported chunks (`w:altChunk`) of the body by container id: what they hold and their preview document.
    /// Not part of undo: the entries never change, and an undone expansion brings the container back.
    alt_chunks: HashMap<NodeId, std::sync::Arc<crate::alt_chunk::AltChunk>>,
    /// Chunks a batch changed, found at its end (see `alt_changed_since`).
    alt_dirty: Vec<NodeId>,
    /// The styles, lists and theme the chunk previews were drawn with (see `sync_alt_previews`).
    alt_stamp: Option<alt_chunks::PreviewStamp>,
    /// The invariant check after every edit (see `invariants`).
    inv: invariants::Checker,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSetup {
    pub width: i64,
    pub height: i64,
    /// top, right, bottom, left (twips)
    pub margin: [i64; 4],
    pub header: i64,
    pub footer: i64,
    pub cols: i64,
    pub title_page: bool,
    /// The final section's own headers and footers by variant (`default`, `first`, `even`).
    pub headers: HashMap<String, RStory>,
    pub footers: HashMap<String, RStory>,
    /// Footnotes in reference order.
    pub notes: Vec<RNote>,
    /// Endnotes in reference order, for the end of the document.
    pub endnotes: Vec<RNote>,
    /// Every section in document order; the fields above describe the last one.
    pub sections: Vec<SectionInfo>,
    /// Every header and footer story by part name (the values of `sections[].headers` and `footers`).
    pub parts: HashMap<String, RStory>,
    /// Page background colour (hex), if the document sets one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_color: Option<String>,
    pub hyphenation: bool,
    /// The default tab stop width in twips (`w:defaultTabStop`, 720 when the file doesn't say).
    pub default_tab: i64,
    pub even_and_odd_headers: bool,
    /// The document's default language (`en-US`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// The watermark: the last section's, else the first section's that has one (see `sections[].watermark`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark: Option<Watermark>,
    /// Mirrored margins, gutter at the top, hyphenation limits and the like (see `layout_props`).
    #[serde(flatten)]
    pub layout: crate::layout_props::LayoutSettings,
    /// The page background when the document has one (a colour, gradient, pattern, texture or picture).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_fill: Option<crate::layout_props::PageFill>,
    /// The separators of footnotes and endnotes as the notes parts have them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_seps: Option<layout_ops::NoteSeps>,
}

/// One section: where it ends, how it starts and its page layout.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionInfo {
    /// The paragraph carrying the section's properties; None for the final section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<NodeId>,
    /// How the section starts: `nextPage`, `continuous`, `evenPage`, `oddPage` or `nextColumn`.
    pub start: String,
    pub width: i64,
    pub height: i64,
    /// top, right, bottom, left (twips)
    pub margin: [i64; 4],
    pub header: i64,
    pub footer: i64,
    /// Extra margin for binding (twips, `w:gutter`), and whether it sits on the right (`w:rtlGutter`).
    #[serde(skip_serializing_if = "is_zero")]
    pub gutter: i64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rtl_gutter: bool,
    pub cols: Columns,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_numbers: Option<LineNumbers>,
    pub title_page: bool,
    /// The page number this section's first page gets, when it restarts the numbering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_start: Option<i64>,
    /// Page number format (`lowerRoman`, `upperRoman`, `lowerLetter`, `upperLetter`); None is 1, 2, 3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_format: Option<String>,
    /// Header part names by variant, including those inherited from earlier sections.
    pub headers: HashMap<String, String>,
    pub footers: HashMap<String, String>,
    /// The header and footer variants this section shows from the section before ("Link to previous"): those it
    /// has no reference of its own for. Always empty for the first section, which has nothing to link to.
    pub linked_headers: Vec<String>,
    pub linked_footers: Vec<String>,
    /// The watermark in the headers this section shows (default, then first-page, then even-page header).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark: Option<Watermark>,
    /// Vertical alignment, text direction, page borders, chapter numbers and note options.
    #[serde(flatten)]
    pub extras: crate::layout_props::SectionExtras,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Columns {
    pub num: i64,
    /// Gap between columns (twips).
    pub space: i64,
    /// A line between columns.
    pub sep: bool,
    /// Column widths (twips) when the columns are not equal; empty otherwise.
    pub widths: Vec<i64>,
    /// The space after each of those columns (twips), one for each but the last.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spaces: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineNumbers {
    pub count_by: i64,
    pub start: i64,
    /// `newPage`, `newSection` or `continuous`.
    pub restart: String,
    /// Distance from the text (twips).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RStory {
    pub part: String,
    pub blocks: Vec<RBlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RNote {
    pub id: String,
    pub num: u32,
    /// The number as the document formats it: `1`, `iv`, `c`, `**`.
    pub label: String,
    pub blocks: Vec<RBlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOut {
    /// Visible top-level block ids in order. Left out of the JSON when `order_kept` (and, as a full render's empty list, when there is nothing visible: an absent `order` without `orderKept` is an empty list).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<NodeId>,
    /// `render_changed` only: the order is the one the last render gave (`order` is empty then). Typing a character changes no block's place, so it does not send the list again; a client keeps the one it has.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub order_kept: bool,
    /// Rendered blocks (all on a full render, only changed ones otherwise).
    pub blocks: Vec<RBlock>,
    pub full: bool,
    /// Headers, footers, notes or the page layout changed: re-fetch `page_setup`.
    pub stories: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatState {
    pub run: RunFmt,
    pub para: ParaFmt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// The character style (`w:rStyle`) of the text at the position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<FormatList>,
    /// The list style preset (`disc`, `decimal`, `checklist`, ...) when the list was made by Docxtor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_style: Option<String>,
    /// For checklist items: whether the item is checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// The list the paragraph is in (its numId) and the level it is at, for the list dialogs and galleries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_num: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_level: Option<i64>,
    /// The count the paragraph's label shows (3 for "c)").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    pub in_table: bool,
    /// Index into `PageSetup.sections` of the section holding the position.
    pub section: usize,
}

/// What kind of list a paragraph is in, for toolbar state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatList {
    Bullet,
    Decimal,
    Checklist,
}

impl Document {
    fn ensure_nonempty_body(&mut self) {
        if !self.body.iter().any(|b| matches!(b, Block::Para(_) | Block::Table(_) | Block::Container(_))) {
            let nid = self.ids.fresh();
            let pos = self.body.iter().position(|b| matches!(b, Block::Opaque(o) if o.el.name == "w:sectPr")).unwrap_or(self.body.len());
            self.body.insert(pos, Block::Para(Paragraph::new(nid)));
            self.changed.insert(nid);
        }
    }

    fn ensure_ns(&mut self, prefix: &str, uri: &str) {
        let key = format!("xmlns:{prefix}");
        let root = if self.active_part == self.main_path {
            &mut self.doc_xml.root
        } else {
            match self.story_parts.get_mut(&self.active_part) {
                Some(d) => &mut d.root,
                None => return,
            }
        };
        if root.attr(&key).is_none() {
            root.set_attr(key, uri);
        }
    }
}

fn rblock_id(b: &RBlock) -> NodeId {
    match b {
        RBlock::Para(p) => p.id,
        RBlock::Table(t) => t.id,
        RBlock::Container { id, .. } | RBlock::Opaque { id, .. } => *id,
    }
}

/// Hard page breaks inside an inline (`w:br w:type="page"`).
fn inline_page_breaks(i: &Inline) -> u32 {
    match i {
        Inline::Run(r) => r.items.iter().filter(|it| matches!(it, RunItem::Break(e) if e.attr("w:type") == Some("page"))).count() as u32,
        Inline::Container(c) => c.children.iter().map(inline_page_breaks).sum(),
        Inline::Opaque(_) => 0,
    }
}

/// Find `needle` in `hay` at or after UTF-16 offset `from`, comparing per character so
/// offsets always refer to the original text (lowercasing can change lengths).
/// Returns (start, end, length of the first matched character), all in UTF-16 units.
fn find_text(hay: &str, needle: &str, from: usize, ignore_case: bool) -> Option<(usize, usize, usize)> {
    let hay: Vec<(usize, char)> = {
        let mut off = 0;
        hay.chars()
            .map(|c| {
                let o = off;
                off += c.len_utf16();
                (o, c)
            })
            .collect()
    };
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() {
        return None;
    }
    let eq = |a: char, b: char| if ignore_case { a == b || a.to_lowercase().eq(b.to_lowercase()) } else { a == b };
    for i in 0..hay.len() {
        if hay[i].0 < from || i + needle.len() > hay.len() {
            continue;
        }
        if needle.iter().enumerate().all(|(k, n)| eq(hay[i + k].1, *n)) {
            let (last_off, last_c) = hay[i + needle.len() - 1];
            return Some((hay[i].0, last_off + last_c.len_utf16(), hay[i].1.len_utf16()));
        }
    }
    None
}
