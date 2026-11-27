//! The operation language. Every change to a document — from a keystroke in the
//! UI to an edit proposed by an AI agent — is one of these serializable ops,
//! which makes edits loggable, replayable, reviewable and (later) syncable.

use crate::model::NodeId;
use crate::numbering::{ListDef, ListKind};
use crate::sort::SortOptions;
use crate::props::{ParaPatch, RunPatch};
use crate::style_edit::StyleDefinition;
use crate::style_sets::{Spacing, StyleSetDef};
use crate::themes::{ColorScheme, FontScheme, ThemeDef};
use serde::{Deserialize, Deserializer, Serialize};

// Everything that is not the `Op` enum itself, beside it.
mod class;
mod params;
mod pos;
mod result;

pub use class::OpClass;
pub use params::{AutoMarkEntry, BreakKind, CaptionPosition, ChapterNumbering, HeaderKind, XrefShow, XrefTarget};
pub use pos::{CellAction, Id, Pos, TableAction, TableCellAction, TableLookPatch};
pub use result::{EditError, OpResult};
// The serde attributes on `Op` name these by path.

use params::{default_connector_kind, default_envelope, default_figure_label, default_index_columns, default_tc_level, default_toc_levels, default_true, default_variant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // short-lived values; boxing the paragraph patch would only add indirection
pub enum Op {
    /// Insert text at a position. `\t` becomes a tab and `\n` a line break.
    InsertText {
        at: Pos,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<RunPatch>,
    },
    /// Delete everything between two positions (may span paragraphs).
    Delete { from: Pos, to: Pos },
    /// Split a paragraph (Enter).
    SplitParagraph { at: Pos },
    /// Apply character formatting to a range.
    FormatText { from: Pos, to: Pos, format: RunPatch },
    /// Apply paragraph formatting.
    FormatParagraphs { ids: Vec<Id>, format: ParaPatch },
    /// Set paragraph style by id or name (built-in styles are added if missing).
    SetStyle { ids: Vec<Id>, style: String },
    /// Change a style's definition, for every paragraph and run that uses it: its name (custom styles), what it is based on,
    /// the style for the following paragraph, whether it shows in the gallery or updates automatically, and its paragraph,
    /// character and language formatting, written into the style as Word writes it. `style` names the style (a built-in
    /// style the file lacks is added first); without it a new style is made from `def` (`name` is needed, `kind` is
    /// `paragraph`, `character` or `linked`). 
    SetStyleDefinition {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
        def: StyleDefinition,
    },
    /// Make a style from the formatting of a selection (Word's Create a Style). The style takes the paragraph's own
    /// formatting and the character formatting the selected text shares, builds on the style the paragraph has, and
    /// replaces that formatting on the selected text.
    CreateStyle {
        name: String,
        /// `paragraph` (default), `character` or `linked`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        from: Pos,
        to: Pos,
        /// More of the definition (`quick`, `next`, ...).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        def: Option<StyleDefinition>,
    },
    /// Update a style to match a selection (Word's Update Style to Match Selection): the style takes the formatting the
    /// selection has by hand, and the selection takes the style.
    UpdateStyle { style: String, from: Pos, to: Pos },
    /// Delete a style a person made. Paragraphs and text that used it fall back to the style it was based on.
    DeleteStyle { style: String },
    /// Change the document's theme (Design, Themes, Colors, Fonts). `theme` is one of Word's built-in themes by id or name
    /// (`facet`, `Ion Boardroom`), or `definition` brings one (a name, colours by slot and heading and body fonts). `colors`
    /// and `fonts` change those alone: a scheme named without colours is one of Word's (`Blue Warm`), and slots left out
    /// keep their colours. Everything that uses theme colours or fonts changes with it.
    SetTheme {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        theme: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        definition: Option<ThemeDef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        colors: Option<ColorScheme>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fonts: Option<FontScheme>,
    },
    /// Apply a style set (Design, Style Set): Normal, Title, Subtitle, Heading 1 to 9 and Quote are written again, and the
    /// paragraph spacing of the document defaults follows. `style_set` is a built-in set by id or name (`word-2010`,
    /// `Lines (Simple)`); `definition` brings one as `w:style` XML.
    SetStyleSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style_set: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        definition: Option<StyleSetDef>,
    },
    /// Set the paragraph spacing of the document (Design, Paragraph Spacing): a preset (`none`, `compact`, `tight`, `open`,
    /// `relaxed`, `double`, `default`), or `spacing` in twips (`before`, `after`) and 240ths of a line (`line`). It is written to
    /// the document defaults, and Normal follows them.
    SetParagraphSpacing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preset: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spacing: Option<Spacing>,
    },
    /// Turn paragraphs into a bullet/numbered list, or back to plain paragraphs.
    ToggleList { ids: Vec<Id>, kind: ListKind },
    /// Give paragraphs a list style: a bullet preset (`disc`, `diamond`, `arrow`, `star`, `arrowhead`, `box`),
    /// a numbered one (`decimal`, `paren`, `outline`, `upper`, `roman`, `zero`), `checklist`, or `none`.
    /// Paragraphs already in a list restyle the whole list; the others start a new list.
    SetListStyle { ids: Vec<Id>, style: String },
    /// Restart numbering at this list item (default 1): it and the items after it in its list count on their own.
    RestartNumbering {
        id: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<i64>,
    },
    /// Join this list item and the ones after it to the closest earlier list of the same kind.
    ContinueNumbering { id: Id },
    /// Define a new list: a bullet, a number format or a multilevel list (levels with their own formats, text with
    /// the earlier levels' counts in it, restart rules, indents, a picture bullet, and headings linked to levels). It is
    /// written as a new definition, as Word writes it, and the paragraphs in `ids` (if any) join it. The result's
    /// `count` is the numId of the new list.
    DefineList {
        def: ListDef,
        #[serde(default)]
        ids: Vec<Id>,
    },
    /// Put paragraphs in a list the document already has (`num_id`, from `list_definitions`): the lists they are in
    /// change to it, and paragraphs outside a list start one.
    ApplyList { ids: Vec<Id>, num_id: i64 },
    /// Give a list item a number (Word's Set Numbering Value). By default it starts a new list at `value`; with
    /// `continue_previous` it carries on the previous list and shows `value` there (skipping numbers when it is higher).
    SetNumberingValue {
        id: Id,
        value: i64,
        #[serde(default)]
        continue_previous: bool,
    },
    /// Sort paragraphs (or list items) by their text, as text, numbers or dates; see `SortOptions`. The paragraphs
    /// swap places among the places they occupy; with `header`, the first stays.
    SortParagraphs {
        ids: Vec<Id>,
        #[serde(flatten)]
        options: SortOptions,
    },
    /// Set the outline level of paragraphs (0 is body text, 1 to 9 are heading levels), or move each by `delta` levels
    /// (negative promotes). Headings switch between the Heading styles; a paragraph whose level is its own switches it directly.
    SetOutlineLevel {
        ids: Vec<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delta: Option<i8>,
    },
    /// Move top-level blocks (in their own order) to just after `after` or just before `before`. List a heading and
    /// everything under it to move a section.
    MoveBlocks {
        ids: Vec<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
    },
    /// Check or uncheck checklist items (checked items are struck through).
    SetChecked { ids: Vec<Id>, checked: bool },
    /// Indent (+1) or outdent (-1); changes list level for list items.
    Indent { ids: Vec<Id>, delta: i64 },
    /// Insert a new paragraph after/before a block (default: end of document).
    InsertParagraph {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// Insert Markdown content (headings, lists, quotes, code, tables, inline formatting).
    InsertMarkdown {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        markdown: String,
    },
    /// Paste Markdown at a position (inline if it is a single plain paragraph).
    PasteMarkdown { at: Pos, markdown: String },
    /// Insert a fragment (the Docxtor clipboard format, or what the HTML and RTF readers make) at a position,
    /// keeping its runs, paragraph properties, styles, lists, tables, pictures, links and notes.
    InsertFragment {
        at: Pos,
        fragment: crate::fragment::Fragment,
        #[serde(default)]
        mode: crate::fragment::PasteMode,
    },
    /// Paste HTML at a position (pictures only as `data:` addresses).
    PasteHtml {
        at: Pos,
        html: String,
        #[serde(default)]
        mode: crate::fragment::PasteMode,
    },
    /// Replace a paragraph's content with inline Markdown, keeping its paragraph style.
    SetParagraphText { id: Id, markdown: String },
    /// Find and replace text, preserving the formatting of the replaced text.
    ///
    /// By default it searches the text and then the headers, footers, notes and text boxes (`main_only` stops that),
    /// for plain text with Word's special characters (`^p`, `^t`, `^?` ...) or, with `wildcards` or `regex`, a pattern
    /// (`\1` or `$1` in `replace` stands for a group). A replacement can contain paragraph marks. `format`,
    /// `para_format` and `language` are put on the new text; with no replacement text they change the matched text
    /// itself. See `FindOptions` for the rest (`whole_word`, `ignore_case`, `keep_case`, `find_format` ...).
    ReplaceText {
        #[serde(default)]
        find: String,
        #[serde(default)]
        replace: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<Id>>,
        #[serde(default)]
        all: bool,
        #[serde(flatten)]
        options: crate::document::FindOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<RunPatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        para_format: Option<ParaPatch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    /// Remove blocks (paragraphs, tables, ...).
    DeleteBlocks { ids: Vec<Id> },
    /// Insert a table after a block.
    InsertTable {
        after: Id,
        rows: usize,
        cols: usize,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        data: Vec<Vec<String>>,
        /// How the columns are sized: `contents` (they grow to their text), `window` (the table fills the width and the
        /// columns share it) or `fixed` (each column is `column_width` twips, or an equal share of the width). Left out,
        /// the table is as Word makes one: equal columns across the page that grow to their text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        autofit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        column_width: Option<i64>,
    },
    /// Row/column operations, addressed by a paragraph inside the target cell.
    TableEdit { at: Id, action: TableAction },
    /// Merge/split/shade/align a rectangle of cells from the cell holding `from` to the one holding `to`.
    TableCells { from: Id, to: Id, action: CellAction },
    /// Table-wide properties (size, alignment, indent, text wrapping, autofit, cell margins, alt text, borders),
    /// addressed by a paragraph inside the table.
    TableFormat {
        at: Id,
        #[serde(flatten)]
        format: crate::table_format::TableFormat,
    },
    /// Row properties (height, may not break across pages, repeat as header row) for the rows from the one holding
    /// `from` to the one holding `to`, or `from`'s row alone, or every row with `all`.
    TableRows {
        from: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Id>,
        #[serde(default)]
        all: bool,
        #[serde(flatten)]
        format: crate::table_format::RowFormat,
    },
    /// The preferred width of the column holding `at` (or the column number `col`, from 0).
    TableColumn {
        at: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        col: Option<usize>,
        width: crate::table_format::Width,
    },
    /// Split the table in two before the row holding `at`, with a paragraph between.
    SplitTable { at: Id },
    /// Insert a cell before the one holding `at` (shift right or down), or delete it (shift left or up).
    TableCell { at: Id, action: TableCellAction, shift: crate::table_edit::Shift },
    /// Sort the rows from the one holding `from` to the one holding `to` (the whole table when `to` is left out) by
    /// columns: each key's `field` is a column number, from 1. With `header` the first row stays.
    SortTable {
        from: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<Id>,
        #[serde(flatten)]
        options: SortOptions,
    },
    /// Replace paragraphs with a table. Each paragraph is a row cut into cells at the separator, or with
    /// `paragraphs` every paragraph is a cell, `columns` to a row. The paragraphs must follow each other.
    ConvertToTable {
        ids: Vec<Id>,
        #[serde(default)]
        separator: crate::table_edit::TextSeparator,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator_char: Option<char>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<usize>,
        /// `fixed`, `contents` or `window`, as for `insert_table`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        autofit: Option<String>,
    },
    /// Replace the table holding `at` with its text: a paragraph per row with the cells cut at the separator, or the
    /// paragraphs of the cells as they are.
    ConvertToText {
        at: Id,
        #[serde(default)]
        separator: crate::table_edit::TextSeparator,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator_char: Option<char>,
    },
    /// Table-wide properties, addressed by a paragraph inside the table.
    TableProps {
        at: Id,
        /// Grid column widths in twips.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        widths: Option<Vec<i64>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        borders: Option<crate::table::BorderPreset>,
        /// Repeat the first row as a header on every page.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        header: Option<bool>,
    },
    /// Give a table one of Word's designs (a table style; its definition is added to the document the first time) and/or
    /// switch parts of the design on or off (`w:tblLook`). Addressed by a paragraph inside the table.
    TableDesign {
        at: Id,
        /// A design's style id (`GridTable4-Accent1`) or name (`Grid Table 4 Accent 1`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
        /// A design an extension offers: one `w:style` of type table as XML. It is added to the document if the document
        /// has no style with its id, and the table gets it (`style` is then not needed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        definition: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look: Option<TableLookPatch>,
    },
    /// Add/replace (`url: Some`) or remove (`url: None`) a hyperlink on a range within one paragraph.
    SetLink {
        from: Pos,
        to: Pos,
        #[serde(default)]
        url: Option<String>,
    },
    InsertPageBreak { at: Pos },
    /// Insert a break at a position: a page or column break inline, or a section break that splits the
    /// paragraph and starts a new section (main text only, not inside tables).
    InsertBreak { at: Pos, kind: BreakKind },
    /// Resize / re-wrap / align an image at a position (the image occupies `[o, o+1)`).
    SetImage {
        at: Pos,
        #[serde(flatten)]
        patch: crate::image::ImagePatch,
    },
    /// Page size/orientation/margins (twips) of the section holding paragraph `section`, or of every section.
    SetPageSetup {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<i64>,
        /// top, right, bottom, left
        #[serde(default, skip_serializing_if = "Option::is_none")]
        margins: Option<[i64; 4]>,
    },
    /// Text columns (1 to 6) for a section (the one holding paragraph `section`, or every section).
    /// `space` is the gap in twips; `sep` draws a line between columns.
    SetColumns {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        count: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        space: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sep: Option<bool>,
        /// Unequal columns: the width of each column in twips (one per column; `space` after each but the
        /// last is `spaces`, or `space`). Without it the columns are equal.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        widths: Option<Vec<i64>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spaces: Option<Vec<i64>>,
    },
    /// Line numbers for a section (or every section). `restart`: `newPage` (default), `newSection` or `continuous`.
    SetLineNumbers {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        on: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count_by: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restart: Option<String>,
    },
    /// Page background colour (hex, with or without `#`); None removes it.
    SetPageColor {
        #[serde(default)]
        color: Option<String>,
    },
    /// Turn automatic hyphenation on or off for the document.
    SetHyphenation { on: bool },
    /// The document's default tab stop width (twips, `w:defaultTabStop`): where tabs land past the last custom stop.
    SetDefaultTabStop { twips: i64 },
    /// Insert an image previously added with `add_media`. Size in points.
    InsertImage {
        at: Pos,
        media: String,
        width: f64,
        height: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
    },
    /// Insert a chart (a Word chart with its data workbook) at a position. Size in points, default 432 x 252.
    /// `series` is `[{name, values}]` (scatter: also `x_values`); `legend` is `none`, `right`, `left`, `top` or `bottom`.
    InsertChart {
        at: Pos,
        kind: crate::chart::ChartKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default)]
        categories: Vec<String>,
        series: Vec<crate::chart::SeriesIn>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grouping: Option<crate::chart::Grouping>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legend: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data_labels: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
        /// Formatting for the new chart: axes, series colours, labels, trendlines, error bars, stock and surface
        /// types ... (see `chart::format::merge`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<serde_json::Value>,
    },
    /// Change a chart (the object at `at`): type, title (`""` removes it), data (categories and series replace the
    /// old ones), legend (`none` removes it), data labels, stacking and axis titles. Everything else about the chart
    /// stays; the chart's workbook keeps its other cells and only the cells the chart uses change. `options` is a
    /// merge patch over the chart's formatting (`chart::format::merge`).
    SetChart {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<crate::chart::ChartKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grouping: Option<crate::chart::Grouping>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        categories: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        series: Option<Vec<crate::chart::SeriesIn>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legend: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data_labels: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x_title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y_title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<serde_json::Value>,
    },
    /// Create (or find) the header/footer of a section (the one holding paragraph `section`, else the last);
    /// the caret lands inside it. `variant`: `default` or `first` (which also turns on `set_title_page`).
    /// Later sections without their own header/footer show it too, as in Word.
    AddHeaderFooter {
        kind: HeaderKind,
        #[serde(default = "default_variant")]
        variant: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
    },
    /// Detach a header/footer from a section (the one holding paragraph `section`, else the last).
    RemoveHeaderFooter {
        kind: HeaderKind,
        #[serde(default = "default_variant")]
        variant: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
    },
    /// Word's "Link to previous" for a header or footer of a section (the one holding paragraph `section`, else
    /// the last). `linked: false` gives the section its own copy of what it shows now (a new part, so the two
    /// can differ); `true` drops the section's own header/footer so it shows the previous section's again.
    /// `variant`: `default`, `first` or `even`. The first section has nothing to link to.
    LinkHeaderFooter {
        kind: HeaderKind,
        #[serde(default = "default_variant")]
        variant: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        linked: bool,
    },
    /// Use a different header/footer on the first page of a section (the one holding paragraph `section`, else the last).
    SetTitlePage {
        on: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
    },
    /// Page numbering of a section (the one holding paragraph `section`, else every section): `start` restarts
    /// the numbers at that value (None continues from the section before); `format` is `decimal`,
    /// `lowerRoman`, `upperRoman`, `lowerLetter` or `upperLetter` (None leaves it as it is).
    SetPageNumbering {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        format: Option<String>,
        /// Chapter page numbers (2-1, 2-2): the Heading level (1 to 9) whose number goes before the page number;
        /// 0 removes it. `chap_sep` is `hyphen`, `period`, `colon`, `emDash` or `enDash`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chap_style: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chap_sep: Option<String>,
    },
    /// Insert a field (`PAGE`, `NUMPAGES`, `DATE`, `TIME`, `AUTHOR`, `TITLE`, ...).
    InsertField {
        at: Pos,
        field: String,
        /// Displayed result until the field is updated (e.g. today's date for `DATE`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        /// The time, the user and the file, which the engine can't know, for working out the result.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<crate::field_values::FieldInput>,
    },
    /// Insert an equation written in LaTeX at a position. It takes one position in the paragraph's text, like an
    /// image. With `display` it gets a paragraph of its own (the paragraph is split at the position) and is centred,
    /// as Word does.
    InsertEquation {
        at: Pos,
        latex: String,
        #[serde(default)]
        display: bool,
    },
    /// Replace the equation at a position (the character there must be an equation) with new LaTeX; `display`
    /// switches between a line of its own and inline.
    SetEquation {
        at: Pos,
        latex: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<bool>,
    },
    /// Insert a footnote reference at a position; the caret lands in the new note.
    InsertFootnote {
        at: Pos,
        #[serde(default)]
        text: String,
    },
    /// Insert a table of contents (a Word `TOC` field listing the headings), after or before a block,
    /// or at the start of the document.
    InsertToc {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        /// Deepest heading level to list (default 3).
        #[serde(default = "default_toc_levels")]
        levels: u8,
        /// A title above the table, e.g. "Contents". None when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Page number of each paragraph id, measured by a layout. Estimated from page breaks when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
        /// What the "Table of contents" dialog sets (levels, styles, links, page numbers, leader, look). Replaces `levels`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<crate::toc_spec::TocOptions>,
    },
    /// Rebuild every table of contents (including ones Word made) from the current headings. With `pages_only`
    /// only the page numbers are worked out again (Word's "Update page numbers only").
    UpdateToc {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
        #[serde(default)]
        pages_only: bool,
    },
    /// Change a table of contents: the paragraph `at` is in it (else the first one in the document). The
    /// instruction is written as Word writes it and the table is built again.
    SetToc {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Id>,
        options: crate::toc_spec::TocOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
    },
    /// Make an imported chunk (`w:altChunk`: HTML, text, RTF or another .docx a tool put in the file) ordinary content.
    /// `id` is the id of any block in the chunk. Editing inside a chunk does this first, by itself.
    ExpandAltChunk { id: Id },
    /// Remove a table of contents (the paragraph `at` is in it, else the first one) and the title above it.
    RemoveToc {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Id>,
    },
    /// Add a line to the table of contents at a position: a hidden `TC` field with the text, a level (1 to 9)
    /// and an entry type (`C` unless another is given). `no_page` leaves out its page number.
    InsertTocEntry {
        at: Pos,
        text: String,
        #[serde(default = "default_tc_level")]
        level: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(default)]
        no_page: bool,
    },
    /// Use font `to` everywhere the document uses `from`: text, headers, footers, footnotes and styles.
    ReplaceFont { from: String, to: String },
    /// Change the case of the text between two positions, keeping its formatting. Casing follows the paragraph's
    /// language (Turkish and Azeri have a dotted and a dotless i).
    ChangeCase { from: Pos, to: Pos, mode: crate::charfmt::CaseMode },
    /// Set the character formatting every paragraph starts from (`w:docDefaults`): font, size, colour and the rest.
    SetDefaultFormat { format: RunPatch },
    /// Bookmark the range `from`..`to` (collapsed for a point bookmark; it may span paragraphs). Names start with
    /// a letter, then letters, digits or underscores, at most 40 characters, and are unique ignoring case.
    InsertBookmark { from: Pos, to: Pos, name: String },
    /// Remove a bookmark (both markers). Links that point at it are left as they are, as in Word.
    DeleteBookmark { name: String },
    /// Rename a bookmark; internal links (`w:anchor`) and `REF`, `PAGEREF` and `NOTEREF` fields follow it.
    RenameBookmark { from: String, to: String },
    /// A horizontal line across the text width at a position: an empty Normal paragraph with a bottom border.
    /// An empty paragraph becomes the line; otherwise the line goes before, after or (splitting it) inside the
    /// paragraph. The caret lands at the start of the paragraph after the line.
    InsertHorizontalLine { at: Pos },
    /// Document properties in `docProps/core.xml` (created if the file has none): the title, subject, author,
    /// keywords and description. A property that is `None` stays as it is; an empty string removes it.
    /// Nothing else in the file changes.
    SetDocProps {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subject: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        author: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keywords: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Word's Category and Status (`cp:category`, `cp:contentStatus`) and the Company and Manager of
        /// `docProps/app.xml`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        company: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manager: Option<String>,
    },
    /// Custom document properties (`docProps/custom.xml`): add or change the ones in `set` (a name, a type of
    /// `text`, `number`, `date` or `bool`, and a value) and remove the names in `remove`. One undo step.
    SetCustomProps {
        #[serde(default)]
        set: Vec<crate::docmeta::CustomProp>,
        #[serde(default)]
        remove: Vec<String>,
    },
    /// The language of the text, as a BCP 47 tag such as `en-US`, `es-ES` or `fr-FR`. Without `ids` it is the
    /// document's default (`w:lang` in the style defaults, and `w:themeFontLang` in the settings when the file
    /// has one); with `ids` it is set on the text of those paragraphs only, for mixed-language documents.
    SetLanguage {
        lang: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<Id>>,
    },
    /// Take what a file carries besides what the page shows out of the document (Inspect document, Remove all).
    /// `what` names the categories: `properties` (author, last saved by, company, manager, custom properties, the
    /// preview picture), `authors` (names behind comments and tracked changes become "Author"), `comments`,
    /// `revisions` (tracked changes are accepted), `hiddenText`, `headersFooters` and `customXml`. Returns how many
    /// things went in `count`. `Document::inspect` says what there is.
    RemoveHiddenData { what: Vec<String> },
    /// A text watermark behind the text of every section (Word's VML WordArt in each header), or none to
    /// remove the watermark. `color` is hex (default `C0C0C0`), `font` defaults to the theme's body font and
    /// `opacity` (0 to 1) to 0.5.
    SetWatermark {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        layout: crate::marks::WatermarkLayout,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opacity: Option<f64>,
    },
    /// A picture watermark behind the text of every section (Word's `WordPictureWatermark` VML shape in each
    /// header). `media` is an image added with `add_media`; `width` and `height` are its size in points at
    /// 100%. Without `scale` the picture is its own size, shrunk to fit the text area; `scale` is 1 for 100%.
    /// `washout` (default true) lightens it as Word's Washout option does. Remove it with `set_watermark`
    /// and `text: null`.
    SetPictureWatermark {
        media: String,
        width: f64,
        height: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<f64>,
        #[serde(default = "default_true")]
        washout: bool,
    },
    /// Content controls: insert, set properties, fill in, remove, repeat (see `controls`).
    Control(crate::controls::ControlOp),
    /// Legacy form fields (`FORMTEXT`, `FORMCHECKBOX`, `FORMDROPDOWN`): insert, fill in, set properties.
    FormField(crate::forms::FormOp),
    /// Document protection: restrict editing, stop it, mark as final, editable ranges (see `protection`).
    Protection(crate::protection::ProtectionOp),
    /// Phonetic guides, enclosed characters and the East Asian line-breaking rules (see `east_asian`).
    EastAsian(crate::east_asian::EastAsianOp),
    /// Document tabs: add, rename, give an emoji, delete, duplicate, move and nest, open (see `tabs`).
    Tab(crate::tabs::TabOp),
    /// Table of authorities: mark citations, insert, change and update the table, name the categories (see `toa`).
    Toa(crate::toa::ToaOp),
    /// Insert an endnote reference at a position (the note is listed at the end of the document); the caret
    /// lands in the new note. Numbered i, ii, iii unless the document says otherwise.
    InsertEndnote {
        at: Pos,
        #[serde(default)]
        text: String,
    },
    /// Layout of a section beyond size and margins: the gutter, header and footer distance, vertical alignment,
    /// text direction and how it starts. `section` is a paragraph in the section, none for every section; with
    /// `from` the section and every one after it change.
    SetSection {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        from: bool,
        #[serde(flatten)]
        patch: crate::layout_props::SectionPatch,
    },
    /// Page borders for a section (see `SetSection` for `section` and `from`); none removes them.
    SetPageBorders {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        from: bool,
        #[serde(default)]
        borders: Option<crate::layout_props::PageBorders>,
    },
    /// Footnote or endnote options: numbering format, first number, when numbering starts again and the position.
    /// For the whole document by default; with `section` (a paragraph in it) only for that section, and with `from`
    /// for it and every one after it.
    SetNoteProps {
        kind: crate::notes::NoteKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        from: bool,
        #[serde(flatten)]
        patch: crate::layout_props::NotePatch,
    },
    /// Document-wide layout settings: mirrored margins, gutter at the top, odd and even headers, book fold,
    /// two pages on a sheet and the hyphenation limits.
    SetLayoutSettings {
        #[serde(flatten)]
        patch: crate::layout_props::LayoutPatch,
    },
    /// The page background: a colour, gradient, pattern, texture or picture; none removes it.
    SetPageBackground {
        #[serde(default)]
        fill: Option<crate::layout_props::PageFill>,
    },
    /// Set or remove the frame (`w:framePr`) of paragraphs.
    SetFrame {
        ids: Vec<Id>,
        #[serde(default)]
        frame: Option<crate::layout_props::FrameFmt>,
    },
    /// A drop cap on a paragraph (or none): the first letter moves into a frame of its own, as Word does it.
    /// `id` is the paragraph with the text, or the drop cap's frame.
    SetDropCap {
        id: Id,
        #[serde(flatten)]
        options: crate::layout_props::DropCapOptions,
    },
    /// A page number in the header or footer (or in the margin) of a section: just the number, or "Page 1 of 5", written
    /// with PAGE and NUMPAGES fields. Page numbers already there (short paragraphs with a PAGE field) are replaced.
    InsertPageNumber {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(flatten)]
        spec: crate::layout_props::PageNumberSpec,
    },
    /// Take the page numbers out of every header and footer (short paragraphs with a PAGE field).
    RemovePageNumbers,
    /// Insert, Cover page: one of Docxtor's designs (`design`, from `page_gallery("cover")`) or a saved cover page
    /// (`fragment`) at the start of the document, in place of the cover page there. Its boxes for the title, subtitle,
    /// author, company, date and abstract are content controls bound to the document's properties. `labels` gives the
    /// placeholder texts and words in the interface language (see `GalleryLabels`).
    InsertCoverPage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        design: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fragment: Option<crate::fragment::Fragment>,
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        labels: crate::document::GalleryLabels,
    },
    /// Remove current cover page.
    RemoveCoverPage,
    /// Insert, Header or Footer: one of Docxtor's designs (from `page_gallery("header")` or `("footer")`) in place of what
    /// the section's header or footer holds (the one holding paragraph `section`, else the last).
    InsertPagePart {
        kind: HeaderKind,
        design: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<Id>,
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        labels: crate::document::GalleryLabels,
    },
    /// Insert a footnote or endnote whose mark is text you choose (`*`, a symbol) instead of a number.
    InsertCustomNote {
        at: Pos,
        kind: crate::notes::NoteKind,
        mark: String,
        #[serde(default)]
        text: String,
    },
    /// Move footnotes to endnotes (`to` is `endnote`) or endnotes to footnotes (`to` is `footnote`), keeping their
    /// content. `ids` are the ids of the notes to move (`RNote.id`); without them every note of the other kind moves.
    ConvertNotes {
        to: crate::notes::NoteKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ids: Option<Vec<String>>,
    },
    /// How footnotes or endnotes are numbered, for the whole document: `format` is `decimal`, `lowerRoman`,
    /// `upperRoman`, `lowerLetter`, `upperLetter` or `symbol`; `start` is the number of the first note.
    SetNoteFormat {
        kind: crate::notes::NoteKind,
        format: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<i64>,
    },
    /// Insert a caption paragraph (style `Caption`) next to a block: `{label} {SEQ field}{separator}{text}`, such
    /// as "Figure 1: Revenue". The sequence numbers of that label are renumbered in document order. The caret
    /// lands at the end of the caption. With `position`, the one block given (`after` or `before`) is the thing
    /// being captioned and `position` says where the caption goes (`above` or `below` it).
    InsertCaption {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        label: String,
        #[serde(default)]
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<CaptionPosition>,
        /// Between the number and the text (default `": "` when there is text).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator: Option<String>,
        /// Number the caption with the chapter it is in ("Figure 2-1"): the numbers start again under each heading of
        /// this level.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chapter: Option<ChapterNumbering>,
    },
    /// Insert a cross-reference field (`REF`, `PAGEREF` or `NOTEREF`) at a position, with its result computed now.
    /// `link` (default true) makes it a link to the target; `above_below` adds "above" or "below" after it.
    /// `pages` maps paragraph ids to page numbers (from a layout) for page references.
    InsertCrossReference {
        at: Pos,
        target: XrefTarget,
        show: XrefShow,
        #[serde(default = "default_true")]
        link: bool,
        #[serde(default)]
        above_below: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
    },
    /// Recompute the results of `SEQ`, `REF`, `PAGEREF` and `NOTEREF` fields and rebuild tables of contents,
    /// tables of figures and indexes. Other fields (`DATE`, `PAGE`, ...) and unknown ones are left as they are.
    /// `scope` limits it to fields in those paragraphs.
    UpdateFields {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope: Option<Vec<Id>>,
        /// Leave no undo step when nothing changed (updating before printing or downloading).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        quiet: bool,
        /// As when the file opens: `DATE` and `TIME` fields and fields marked dirty, or every field when the file
        /// asks for that (`w:updateFields`, which is then cleared). Leaves no undo step.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        open: bool,
        /// Only fields with these codes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only: Option<Vec<String>>,
        /// The time, the user, the file and the page count, which the engine can't know.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<crate::field_values::FieldInput>,
    },
    /// Change the code of the field that starts at a position and work out its result again.
    SetFieldCode {
        at: Pos,
        instr: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<crate::field_values::FieldInput>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
    },
    /// Replace the plain-text result of the field that starts at a position (for field types an extension evaluates).
    SetFieldResult { at: Pos, text: String },
    /// Mark an index entry at a position: a hidden `XE` field. `sub` is a sub-entry; `see` replaces the page
    /// number with text such as "See Fruit".
    MarkIndexEntry {
        at: Pos,
        entry: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sub: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        see: Option<String>,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
    },
    /// Mark the first occurrence of `text` in every paragraph as an index entry (Word's Mark All).
    /// The entry is `entry`, or the text itself.
    MarkAllIndexEntries {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entry: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sub: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        see: Option<String>,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        ignore_case: bool,
        #[serde(default)]
        whole_word: bool,
    },
    /// Insert an index (an `INDEX` field) after or before a block (default: end of the document), in its own
    /// continuous section with `columns` columns. `run_in` puts sub-entries in their entry's paragraph;
    /// `headings` adds a letter heading before each letter. `pages` maps paragraph ids to page numbers.
    InsertIndex {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        #[serde(default = "default_index_columns")]
        columns: u8,
        #[serde(default)]
        run_in: bool,
        #[serde(default)]
        headings: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
        /// What the "Index" dialog sets (columns, type, right-aligned numbers, leader, separators, language, look).
        /// Replaces `columns`, `run_in` and `headings`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<crate::index::IndexOptions>,
    },
    /// Change an index (the paragraph `at` is in it, else the first one): the instruction is written as Word writes
    /// it and the index is built again. A different number of columns makes the index again in a section of its own.
    SetIndex {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Id>,
        options: crate::index::IndexOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
    },
    /// Mark index entries from a list, as Word's AutoMark does with its file of two columns: the first
    /// occurrence of each `text` in every paragraph (matching case) becomes an entry. The result's `count` is how
    /// many marks were made.
    AutoMarkIndex { entries: Vec<AutoMarkEntry> },
    /// Insert a building block (quick part) from the document's glossary (`word/glossary/document.xml`) at a
    /// position, found by its `index`, else its `guid`, else its `name`. Its paragraphs, tables, styles, lists and
    /// pictures come with it.
    InsertBuildingBlock {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Insert a table of figures (`TOC \c "Figure"`) listing the captions with this label and their pages.
    InsertTableOfFigures {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        #[serde(default = "default_figure_label")]
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pages: Option<std::collections::HashMap<String, u32>>,
    },
    /// Add a source to the document's list of sources (Word's Manage Sources). An empty `tag` is made the way
    /// Word does: three letters of the first author's surname and two digits of the year, with `a`, `b`, ... if
    /// taken (`Document::sources` shows which one it got).
    AddSource { source: crate::citations::Source },
    /// Replace the source with this tag. Citations and bibliographies that use it are written again; a new tag
    /// in `source.tag` renames the source in every citation.
    UpdateSource { tag: String, source: crate::citations::Source },
    /// Remove a source. It is an error while citations use it, unless `force`, which turns those citations into
    /// plain text.
    DeleteSource {
        tag: String,
        #[serde(default)]
        force: bool,
    },
    /// Choose the citation style: `apa`, `mla`, `chicago`, `ieee` or `harvard`, or the id of a CSL style the host
    /// has added. Every citation and bibliography is written again, and the choice is saved the way Word saves it,
    /// so Word re-renders with the closest style of its own. Switching to a note style turns the citations into
    /// footnotes; switching away turns footnotes that hold only a citation back into citations.
    SetCitationStyle { style: String },
    /// Insert a citation of one or more sources (a content control around a `CITATION` field, as Word writes
    /// it). `page` is the locator ("23" or "23-25"). The caret lands after the citation.
    InsertCitation {
        at: Pos,
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_author: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_year: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_title: Option<bool>,
    },
    /// Word's "Add new placeholder": add a source that has only a tag (`Placeholder1`, or `tag`) and cite it at a
    /// position. The citation shows the tag in brackets and the bibliography leaves the source out until it is
    /// filled in with `update_source`.
    InsertCitationPlaceholder {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
    },
    /// Change the citation at (or ending at) a position. Empty `tags` keep the sources; `page: None` keeps the
    /// locator and `Some("")` removes it.
    EditCitation {
        at: Pos,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_author: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_year: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_title: Option<bool>,
    },
    /// Remove the citation at (inside, at the end of, or at the start of) a position, and write the other
    /// citations and the bibliography again. In a footnote that holds only the citation, the whole note goes.
    RemoveCitation { at: Pos },
    /// Insert a bibliography (a content control around a `BIBLIOGRAPHY` field listing the cited sources in the
    /// chosen style), after or before a block or at the end. `heading` defaults to the style's ("References",
    /// "Works Cited" or "Bibliography"); an empty string leaves it out.
    InsertBibliography {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<Id>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        heading: Option<String>,
    },
    /// Write every citation and bibliography again from the sources (including ones Word made); the result's
    /// `count` is how many fields changed.
    UpdateBibliography,
    /// Insert a shape at a position (main text only): a preset such as `rect`, `roundRect`, `ellipse`, `triangle`,
    /// `rightArrow`, `star5`, `line` (sizes in points). `fill` is hex or `none`; `line` is `none` or
    /// `{color, width_pt, dash, head_arrow, tail_arrow}`; `text` goes inside the shape. A floating shape
    /// (the default) is anchored to the paragraph with square wrapping, `x` and `y` points from the column and the
    /// paragraph; a non-floating one sits in the line of text. The caret lands after the shape.
    InsertShape {
        at: Pos,
        geometry: String,
        w: f64,
        h: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<crate::shapes::LineSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default = "default_true")]
        floating: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
    },
    /// Insert a text box (a rectangle with a thin black outline and white fill) with its text, one paragraph per
    /// line. The caret lands in its first paragraph, whose id is in `created`.
    InsertTextBox {
        at: Pos,
        w: f64,
        h: f64,
        #[serde(default)]
        text: String,
        #[serde(default = "default_true")]
        floating: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
    },
    /// Move, resize, rotate, recolour or re-wrap the shape at a position (the shape occupies `[o, o+1)`).
    /// Delete a shape with `delete` over that one position.
    SetShape {
        at: Pos,
        #[serde(flatten)]
        patch: crate::shapes::patch::ShapePatch,
    },
    /// Move a floating object (a shape, text box, chart or picture that is not in line with the text) in the
    /// stacking order of its kind: `front`, `back`, `forward` or `backward`. Objects behind the text and objects in
    /// front of it are two stacks.
    SetObjectOrder { at: Pos, order: String },
    /// Put a floating object behind the text (`behind`) or in front of it, where it is now. The text then runs
    /// through it instead of around it.
    SetBehindText { at: Pos, behind: bool },
    /// Group the shapes at these positions into one (the group goes where the first shape is). `offsets` gives the
    /// top-left corner of each shape in points, in the frame the first shape's position counts from; without it
    /// the shapes' own offsets are used, which must count from the same point.
    GroupShapes {
        at: Vec<Pos>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offsets: Option<Vec<[f64; 2]>>,
    },
    /// Take a group apart into the shapes it holds, in the same place on the page. `offset` is the group's
    /// top-left corner in points when it is placed by alignment.
    UngroupShape {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[f64; 2]>,
    },
    /// Start a mail merge or change it (Word's Mailings, Start Mail Merge and Select Recipients): the kind of main document
    /// (`formLetters`, `mailingLabels`, `envelopes`, `catalog`, `email`, `fax`) and the recipients as a header row and records. Word's
    /// `w:mailMerge` is written to the settings, and the recipients are kept in the file. Without `source` and `columns` only the
    /// kind changes. `merge: null` ends the mail merge.
    SetMailMerge {
        #[serde(default)]
        merge: Option<crate::merge::MailMergeDef>,
    },
    /// Edit the recipients of a mail merge: the ticks (one for each record), the filter, the sort and which column stands for each
    /// address field (`{"First Name": "Given"}`). Each is optional.
    SetMergeRecipients {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active: Option<Vec<bool>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<Vec<crate::merge::FilterRule>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sort: Option<Vec<crate::merge::SortKey>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mapping: Option<std::collections::BTreeMap<String, String>>,
    },
    /// Insert a merge field or a rule at a position: a column (`{"kind":"field","name":"First Name"}`), an address block, a greeting
    /// line, `if`, `ask`, `fill_in`, `next`, `next_if`, `skip_if`, `merge_rec`, `merge_seq` or `set`.
    InsertMergeField { at: Pos, field: crate::merge::instr::MergeFieldSpec },
    /// Copy the first label of a sheet of labels into the others, each starting with a Next Record field (Word's Update Labels). `at`
    /// is a paragraph in the sheet; without it the first table is used.
    UpdateLabels {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<Id>,
    },
    /// Replace the document with a sheet of labels made as a table the way Word makes one: `spec` is the sheet's size and the labels'
    /// (twips), `fill` says what is on them (`{"kind":"same","lines":[...]}`, `{"kind":"single","lines":[...],"row":1,"col":1}` or
    /// `{"kind":"merge"}`), `look` the font.
    MakeLabels {
        spec: crate::merge::labels::LabelSpec,
        fill: crate::merge::labels::LabelFill,
        #[serde(default)]
        look: crate::merge::labels::TextLook,
    },
    /// Add an envelope as the first section, with the envelope's page size (Word's Envelopes, Add to Document); an envelope that is
    /// already first is replaced. `size` is one of Word's (`10`, `9`, `6-3-4`, `monarch`, `11`, `12`, `dl`, `c6`, `c65`, `c5`, `c4`,
    /// `b6`, `b5`, `b4`), or give `width` and `height` in twips. `replace` makes the envelope the whole document (for a merge), and
    /// `address_block` puts an address block where the delivery address goes.
    InsertEnvelope {
        #[serde(default = "default_envelope")]
        size: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<i64>,
        layout: crate::merge::envelope::EnvelopeLayout,
        #[serde(default)]
        replace: bool,
        #[serde(default)]
        address_block: bool,
    },
    /// Insert a freehand drawing at a position: `strokes` are lists of `[x, y]` points (in points, from the column
    /// and the paragraph, like a shape's `x` and `y`), drawn with one pen (`color` as hex, `width_pt`). It becomes
    /// one shape in front of the text, with a smooth path for each stroke.
    InsertInk {
        at: Pos,
        strokes: Vec<Vec<[f64; 2]>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width_pt: Option<f64>,
        /// How much of the colour shows, 0 to 1 (a highlighter is about 0.5). Opaque when left out.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alpha: Option<f64>,
        /// How the pen ends the line: `round` (the default), `square` or `flat`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cap: Option<String>,
    },
    /// Insert a curve, freeform or scribble at a position: a shape with its own outline (`a:custGeom`). Give the
    /// outline as `points` (in points; `smooth` draws a curve through them, `closed` joins the last to the first) or
    /// as `cmds` (`{t, pts}` with `M`, `L`, `C`, `Z`, and `S` for a smooth run). The first point sits `x`, `y` points
    /// from the column and the paragraph; the shape's box is the outline's bounds. Open outlines get no fill.
    InsertFreeform {
        at: Pos,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        points: Vec<[f64; 2]>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cmds: Vec<crate::shapes::freeform::CmdIn>,
        #[serde(default)]
        smooth: bool,
        #[serde(default)]
        closed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<crate::shapes::LineSpec>,
        #[serde(default = "default_true")]
        floating: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
        /// The name Word shows before the number (`Freeform: Shape`, `Freeform: Scribble`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// Give the shape at a position a new outline ("Edit points"): `paths` are lists of `{t, pts}` commands, each in
    /// its own `w` by `h` space (points of the shape's box when left out). The shape's box becomes the bounds of
    /// the outline and moves with it (`x`, `y` say where its top-left corner lands, for a floating shape). A preset
    /// shape becomes a custom one. Written to `a:custGeom` and the VML copy.
    SetShapePath {
        at: Pos,
        paths: Vec<crate::shapes::freeform::PathIn>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
    },
    /// Insert a connector (`straight`, `elbow` or `curved`) from point `from` to point `to` (points, from the column
    /// and the paragraph, like a shape's `x` and `y`). An end can be glued to a shape: `start` / `end` are
    /// `{shape: {p, o}, idx}` (the site's number as Word counts them). Glued ends take the site's place, and the
    /// engine keeps them there when the shape moves. Without `from` or `to`, a glued end's site gives the point.
    InsertConnector {
        at: Pos,
        #[serde(default = "default_connector_kind")]
        kind: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<crate::shapes::connector::EndSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end: Option<crate::shapes::connector::EndSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<[f64; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<[f64; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<crate::shapes::LineSpec>,
    },
    /// Change a connector: its `kind`, where its ends are (`from`, `to`) or what they are glued to (`start`, `end`:
    /// `"free"` lets go of a shape). A glued end moves to its site.
    SetConnector {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        start: Option<crate::shapes::connector::EndSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end: Option<crate::shapes::connector::EndSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<[f64; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<[f64; 2]>,
    },
    /// Name a drawing object (a shape, text box, picture, chart or group; `wp:docPr name`) or hide it
    /// (`hidden="1"`, Word's eye in the Selection pane) or show it again. A hidden object is not drawn or
    /// printed but stays in the file.
    SetObjectProps {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hidden: Option<bool>,
    },
    /// Place a floating object on the page the way Word does: `rel_x` / `rel_y` say what the position counts from
    /// (`margin`, `page`, `column`, `paragraph`, ...), `align` / `align_y` an alignment from that (`left`, `center`,
    /// `right`; `top`, `center`, `bottom`), and `x` / `y` an offset in points when there is no alignment.
    SetObjectPosition {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rel_x: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        align: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rel_y: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        align_y: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
    },
    /// Put a floating object directly above `above` or directly below `below` in the stacking order. Both
    /// objects must be in the same stack (behind the text, or in front of it).
    MoveObject {
        at: Pos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        above: Option<Pos>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        below: Option<Pos>,
    },
}
