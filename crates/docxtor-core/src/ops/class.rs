//! What an op is: its anchor, its wire name and the class that says how it may be used.

use super::*;

/// What kind of op an op is: every flag the engine asks about an op, set in one place (`Op::class`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpClass {
    /// Only makes sense in the main text: it can't run inside a header, footer or note.
    pub main_text_only: bool,
    /// Defines or restyles styles, the theme or the document defaults (not text formatting).
    pub edits_styles: bool,
    /// Changes how the pages are laid out or look as a whole: page setup, columns, headers and footers, watermark,
    /// page colour, note numbering. Refused while editing is restricted (protection).
    pub changes_page_layout: bool,
    /// Changes a part other than the body (properties, styles): the body is not rewritten on save.
    pub leaves_body_alone: bool,
    /// Changes nothing a person wrote: a view-only document still applies it, and it takes no undo step.
    pub view_only: bool,
    /// Adds or changes a chart (the chart parts are rebuilt after the batch).
    pub touches_charts: bool,
    /// Runs on the whole document even when its position is inside a note, header or footer (citations).
    pub whole_document: bool,
    /// Stays inside one paragraph, so it can't join two tabs or remove the start of one.
    pub within_paragraph: bool,
    /// Formats directly, which a document that limits formatting to some styles refuses.
    pub limited_formatting: bool,
}

impl OpClass {
    /// An ordinary edit of the main text: every flag off.
    pub const BODY: OpClass = OpClass { main_text_only: false, edits_styles: false, changes_page_layout: false, leaves_body_alone: false, view_only: false, touches_charts: false, whole_document: false, within_paragraph: false, limited_formatting: false };
    pub const fn main_text(mut self) -> Self {
        self.main_text_only = true;
        self
    }
    pub const fn styles(mut self) -> Self {
        self.edits_styles = true;
        self
    }
    pub const fn layout(mut self) -> Self {
        self.changes_page_layout = true;
        self
    }
    pub const fn leaves_body(mut self) -> Self {
        self.leaves_body_alone = true;
        self
    }
    pub const fn view_only(mut self) -> Self {
        self.view_only = true;
        self
    }
    pub const fn chart(mut self) -> Self {
        self.touches_charts = true;
        self
    }
    pub const fn whole_document(mut self) -> Self {
        self.whole_document = true;
        self
    }
    pub const fn within_paragraph(mut self) -> Self {
        self.within_paragraph = true;
        self
    }
    pub const fn limits_formatting(mut self) -> Self {
        self.limited_formatting = true;
        self
    }
}

impl Op {
    /// The paragraph that locates an op, used to route it to the story holding it.
    pub fn anchor(&self) -> Option<NodeId> {
        use Op::*;
        let first = |ids: &[Id]| ids.first().map(|i| i.0);
        match self {
            InsertText { at, .. } | SplitParagraph { at } | PasteMarkdown { at, .. } | InsertFragment { at, .. } | PasteHtml { at, .. } | InsertPageBreak { at } | InsertBreak { at, .. } | SetImage { at, .. } | InsertImage { at, .. } | InsertChart { at, .. } | SetChart { at, .. } | InsertField { at, .. } | SetFieldCode { at, .. } | SetFieldResult { at, .. } | InsertFootnote { at, .. } | InsertHorizontalLine { at } | InsertEquation { at, .. } | SetEquation { at, .. } => Some(at.p.0),
            Delete { from, .. } | FormatText { from, .. } | ChangeCase { from, .. } | SetLink { from, .. } | InsertBookmark { from, .. } => Some(from.p.0),
            FormatParagraphs { ids, .. } | SetStyle { ids, .. } | ToggleList { ids, .. } | SetListStyle { ids, .. } | SetChecked { ids, .. } | Indent { ids, .. } | DeleteBlocks { ids } => first(ids),
            DefineList { ids, .. } | ApplyList { ids, .. } | SortParagraphs { ids, .. } | SetOutlineLevel { ids, .. } | MoveBlocks { ids, .. } => first(ids),
            SetNumberingValue { id, .. } | ExpandAltChunk { id } => Some(id.0),
            InsertParagraph { after, before, .. } | InsertMarkdown { after, before, .. } | InsertToc { after, before, .. } => after.or(*before).map(|i| i.0),
            InsertEndnote { at, .. } | InsertCrossReference { at, .. } | MarkIndexEntry { at, .. } | InsertTocEntry { at, .. } | InsertBuildingBlock { at, .. } => Some(at.p.0),
            InsertCaption { after, before, .. } | InsertIndex { after, before, .. } | InsertTableOfFigures { after, before, .. } => after.or(*before).map(|i| i.0),
            ConvertNotes { .. } | SetNoteFormat { .. } | UpdateFields { .. } | MarkAllIndexEntries { .. } | SetSection { .. } | SetPageBorders { .. } | SetNoteProps { .. } | SetLayoutSettings { .. } | SetPageBackground { .. } | InsertPageNumber { .. } | RemovePageNumbers | InsertCoverPage { .. } | RemoveCoverPage | InsertPagePart { .. } | SetIndex { .. } | AutoMarkIndex { .. } => None,
            SetFrame { ids, .. } => first(ids),
            SetDropCap { id, .. } => Some(id.0),
            InsertCustomNote { at, .. } => Some(at.p.0),
            SetLanguage { ids: Some(ids), .. } => first(ids),
            SetParagraphText { id, .. } | RestartNumbering { id, .. } | ContinueNumbering { id } => Some(id.0),
            ReplaceText { ids, .. } => ids.as_deref().and_then(first),
            InsertTable { after, .. } => Some(after.0),
            TableEdit { at, .. } | TableProps { at, .. } | TableDesign { at, .. } => Some(at.0),
            TableCells { from, .. } | TableRows { from, .. } | SortTable { from, .. } => Some(from.0),
            TableFormat { at, .. } | TableColumn { at, .. } | SplitTable { at } | TableCell { at, .. } | ConvertToText { at, .. } => Some(at.0),
            ConvertToTable { ids, .. } => first(ids),
            InsertCitation { at, .. } | InsertCitationPlaceholder { at, .. } | EditCitation { at, .. } | RemoveCitation { at } | InsertShape { at, .. } | InsertTextBox { at, .. } | SetShape { at, .. } | SetObjectOrder { at, .. } | SetBehindText { at, .. } | UngroupShape { at, .. } | InsertInk { at, .. } | SetObjectProps { at, .. } | SetObjectPosition { at, .. } | MoveObject { at, .. } | InsertFreeform { at, .. } | SetShapePath { at, .. } | InsertConnector { at, .. } | SetConnector { at, .. } => Some(at.p.0),
            GroupShapes { at, .. } => at.first().map(|p| p.p.0),
            InsertMergeField { at, .. } => Some(at.p.0),
            UpdateLabels { at } => at.map(|i| i.0),
            SetMailMerge { .. } | SetMergeRecipients { .. } | MakeLabels { .. } | InsertEnvelope { .. } => None,
            SetDefaultTabStop { .. } => None,
            Control(c) => c.anchor(),
            FormField(f) => f.anchor(),
            Protection(p) => p.anchor(),
            EastAsian(e) => e.anchor(),
            Tab(_) => None,
            Toa(t) => t.anchor(),
            CreateStyle { from, .. } | UpdateStyle { from, .. } => Some(from.p.0),
            SetStyleDefinition { .. } | DeleteStyle { .. } | SetTheme { .. } | SetStyleSet { .. } | SetParagraphSpacing { .. } => None,
            InsertBibliography { after, before, .. } => after.or(*before).map(|i| i.0),
            AddSource { .. } | UpdateSource { .. } | DeleteSource { .. } | SetCitationStyle { .. } | UpdateBibliography => None,
            SetPageSetup { .. } | SetColumns { .. } | SetLineNumbers { .. } | SetPageColor { .. } | SetHyphenation { .. } | SetPageNumbering { .. } | AddHeaderFooter { .. } | RemoveHeaderFooter { .. } | LinkHeaderFooter { .. } | SetTitlePage { .. } | UpdateToc { .. } | SetToc { .. } | RemoveToc { .. } | ReplaceFont { .. } | SetDefaultFormat { .. } | DeleteBookmark { .. } | RenameBookmark { .. } | SetWatermark { .. } | SetPictureWatermark { .. } | SetDocProps { .. } | SetCustomProps { .. } | SetLanguage { ids: None, .. } | RemoveHiddenData { .. } => None,
        }
    }
}

impl Op {
    /// The op's name as it is on the wire (`insert_text`, `set_format`...), which is what the undo list shows.
    ///
    /// Read from the variant, not from the op's JSON: a batch asks for it on every step, and an op can carry a whole pasted
    /// fragment. The match has no catch-all, so a new op has to be named here (a test checks the names against the wire).
    pub fn name(&self) -> &'static str {
        match self {
            Op::InsertText { .. } => "insert_text",
            Op::Delete { .. } => "delete",
            Op::SplitParagraph { .. } => "split_paragraph",
            Op::FormatText { .. } => "format_text",
            Op::FormatParagraphs { .. } => "format_paragraphs",
            Op::SetStyle { .. } => "set_style",
            Op::SetStyleDefinition { .. } => "set_style_definition",
            Op::CreateStyle { .. } => "create_style",
            Op::UpdateStyle { .. } => "update_style",
            Op::DeleteStyle { .. } => "delete_style",
            Op::SetTheme { .. } => "set_theme",
            Op::SetStyleSet { .. } => "set_style_set",
            Op::SetParagraphSpacing { .. } => "set_paragraph_spacing",
            Op::ToggleList { .. } => "toggle_list",
            Op::SetListStyle { .. } => "set_list_style",
            Op::RestartNumbering { .. } => "restart_numbering",
            Op::ContinueNumbering { .. } => "continue_numbering",
            Op::DefineList { .. } => "define_list",
            Op::ApplyList { .. } => "apply_list",
            Op::SetNumberingValue { .. } => "set_numbering_value",
            Op::SortParagraphs { .. } => "sort_paragraphs",
            Op::SetOutlineLevel { .. } => "set_outline_level",
            Op::MoveBlocks { .. } => "move_blocks",
            Op::SetChecked { .. } => "set_checked",
            Op::Indent { .. } => "indent",
            Op::InsertParagraph { .. } => "insert_paragraph",
            Op::InsertMarkdown { .. } => "insert_markdown",
            Op::PasteMarkdown { .. } => "paste_markdown",
            Op::InsertFragment { .. } => "insert_fragment",
            Op::PasteHtml { .. } => "paste_html",
            Op::SetParagraphText { .. } => "set_paragraph_text",
            Op::ReplaceText { .. } => "replace_text",
            Op::DeleteBlocks { .. } => "delete_blocks",
            Op::InsertTable { .. } => "insert_table",
            Op::TableEdit { .. } => "table_edit",
            Op::TableCells { .. } => "table_cells",
            Op::TableFormat { .. } => "table_format",
            Op::TableRows { .. } => "table_rows",
            Op::TableColumn { .. } => "table_column",
            Op::SplitTable { .. } => "split_table",
            Op::TableCell { .. } => "table_cell",
            Op::SortTable { .. } => "sort_table",
            Op::ConvertToTable { .. } => "convert_to_table",
            Op::ConvertToText { .. } => "convert_to_text",
            Op::TableProps { .. } => "table_props",
            Op::TableDesign { .. } => "table_design",
            Op::SetLink { .. } => "set_link",
            Op::InsertPageBreak { .. } => "insert_page_break",
            Op::InsertBreak { .. } => "insert_break",
            Op::SetImage { .. } => "set_image",
            Op::SetPageSetup { .. } => "set_page_setup",
            Op::SetColumns { .. } => "set_columns",
            Op::SetLineNumbers { .. } => "set_line_numbers",
            Op::SetPageColor { .. } => "set_page_color",
            Op::SetHyphenation { .. } => "set_hyphenation",
            Op::SetDefaultTabStop { .. } => "set_default_tab_stop",
            Op::InsertImage { .. } => "insert_image",
            Op::InsertChart { .. } => "insert_chart",
            Op::SetChart { .. } => "set_chart",
            Op::AddHeaderFooter { .. } => "add_header_footer",
            Op::InsertCoverPage { .. } => "insert_cover_page",
            Op::RemoveCoverPage => "remove_cover_page",
            Op::InsertPagePart { .. } => "insert_page_part",
            Op::RemoveHeaderFooter { .. } => "remove_header_footer",
            Op::LinkHeaderFooter { .. } => "link_header_footer",
            Op::SetTitlePage { .. } => "set_title_page",
            Op::SetPageNumbering { .. } => "set_page_numbering",
            Op::InsertField { .. } => "insert_field",
            Op::InsertEquation { .. } => "insert_equation",
            Op::SetEquation { .. } => "set_equation",
            Op::InsertFootnote { .. } => "insert_footnote",
            Op::InsertToc { .. } => "insert_toc",
            Op::UpdateToc { .. } => "update_toc",
            Op::SetToc { .. } => "set_toc",
            Op::RemoveToc { .. } => "remove_toc",
            Op::ExpandAltChunk { .. } => "expand_alt_chunk",
            Op::InsertTocEntry { .. } => "insert_toc_entry",
            Op::ReplaceFont { .. } => "replace_font",
            Op::ChangeCase { .. } => "change_case",
            Op::SetDefaultFormat { .. } => "set_default_format",
            Op::InsertBookmark { .. } => "insert_bookmark",
            Op::DeleteBookmark { .. } => "delete_bookmark",
            Op::RenameBookmark { .. } => "rename_bookmark",
            Op::InsertHorizontalLine { .. } => "insert_horizontal_line",
            Op::SetDocProps { .. } => "set_doc_props",
            Op::SetCustomProps { .. } => "set_custom_props",
            Op::SetLanguage { .. } => "set_language",
            Op::RemoveHiddenData { .. } => "remove_hidden_data",
            Op::SetWatermark { .. } => "set_watermark",
            Op::SetPictureWatermark { .. } => "set_picture_watermark",
            Op::Control(..) => "control",
            Op::FormField(..) => "form_field",
            Op::Protection(..) => "protection",
            Op::EastAsian(..) => "east_asian",
            Op::Tab(..) => "tab",
            Op::Toa(..) => "toa",
            Op::InsertEndnote { .. } => "insert_endnote",
            Op::SetSection { .. } => "set_section",
            Op::SetPageBorders { .. } => "set_page_borders",
            Op::SetNoteProps { .. } => "set_note_props",
            Op::SetLayoutSettings { .. } => "set_layout_settings",
            Op::SetPageBackground { .. } => "set_page_background",
            Op::SetFrame { .. } => "set_frame",
            Op::SetDropCap { .. } => "set_drop_cap",
            Op::InsertPageNumber { .. } => "insert_page_number",
            Op::RemovePageNumbers => "remove_page_numbers",
            Op::InsertCustomNote { .. } => "insert_custom_note",
            Op::ConvertNotes { .. } => "convert_notes",
            Op::SetNoteFormat { .. } => "set_note_format",
            Op::InsertCaption { .. } => "insert_caption",
            Op::InsertCrossReference { .. } => "insert_cross_reference",
            Op::UpdateFields { .. } => "update_fields",
            Op::SetFieldCode { .. } => "set_field_code",
            Op::SetFieldResult { .. } => "set_field_result",
            Op::MarkIndexEntry { .. } => "mark_index_entry",
            Op::MarkAllIndexEntries { .. } => "mark_all_index_entries",
            Op::InsertIndex { .. } => "insert_index",
            Op::SetIndex { .. } => "set_index",
            Op::AutoMarkIndex { .. } => "auto_mark_index",
            Op::InsertBuildingBlock { .. } => "insert_building_block",
            Op::InsertTableOfFigures { .. } => "insert_table_of_figures",
            Op::AddSource { .. } => "add_source",
            Op::UpdateSource { .. } => "update_source",
            Op::DeleteSource { .. } => "delete_source",
            Op::SetCitationStyle { .. } => "set_citation_style",
            Op::InsertCitation { .. } => "insert_citation",
            Op::InsertCitationPlaceholder { .. } => "insert_citation_placeholder",
            Op::EditCitation { .. } => "edit_citation",
            Op::RemoveCitation { .. } => "remove_citation",
            Op::InsertBibliography { .. } => "insert_bibliography",
            Op::UpdateBibliography => "update_bibliography",
            Op::InsertShape { .. } => "insert_shape",
            Op::InsertTextBox { .. } => "insert_text_box",
            Op::SetShape { .. } => "set_shape",
            Op::SetObjectOrder { .. } => "set_object_order",
            Op::SetBehindText { .. } => "set_behind_text",
            Op::GroupShapes { .. } => "group_shapes",
            Op::UngroupShape { .. } => "ungroup_shape",
            Op::SetMailMerge { .. } => "set_mail_merge",
            Op::SetMergeRecipients { .. } => "set_merge_recipients",
            Op::InsertMergeField { .. } => "insert_merge_field",
            Op::UpdateLabels { .. } => "update_labels",
            Op::MakeLabels { .. } => "make_labels",
            Op::InsertEnvelope { .. } => "insert_envelope",
            Op::InsertInk { .. } => "insert_ink",
            Op::InsertFreeform { .. } => "insert_freeform",
            Op::SetShapePath { .. } => "set_shape_path",
            Op::InsertConnector { .. } => "insert_connector",
            Op::SetConnector { .. } => "set_connector",
            Op::SetObjectProps { .. } => "set_object_props",
            Op::SetObjectPosition { .. } => "set_object_position",
            Op::MoveObject { .. } => "move_object",
        }
    }

    /// Ops that only make sense in the main text (they can't run inside a header, footer or note).
    pub fn main_text_only(&self) -> bool {
        self.class().main_text_only
    }

    /// Ops that define or restyle styles, the theme or the document defaults (not text formatting).
    pub fn edits_styles(&self) -> bool {
        self.class().edits_styles
    }

    /// Ops that change how the pages are laid out or look as a whole: page setup, columns, headers and footers,
    /// watermark and page colour, fonts replaced everywhere.
    pub fn changes_page_layout(&self) -> bool {
        self.class().changes_page_layout
    }

    /// Ops that change a part other than the body (properties, styles): the body is not rewritten on save.
    pub fn leaves_body_alone(&self) -> bool {
        self.class().leaves_body_alone
    }

    /// Ops a view-only document (`Access::ViewOnly`) still applies, because they change nothing a person wrote
    /// (`OpClass::view_only`, set in `class`).
    ///
    /// - The field update that runs as a file opens (`open: true`): it refreshes dates, dirty fields and the like, leaves
    ///   no undo step and does not count as a change by the person. The update before printing (`quiet`) is not here: it
    ///   is an edit, and a read-only document is printed as it is.
    pub fn is_view_only(&self) -> bool {
        self.class().view_only
    }

    /// What kind of op this is, all the flags together: the one place that decides them.
    ///
    /// The match has no catch-all, so a new op does not compile until its flags are set here. Start from
    /// `OpClass::BODY` (an ordinary edit of the main text) and add what is true of the op.
    pub fn class(&self) -> OpClass {
        use OpClass as C;
        match self {
            // Text and paragraphs.
            Op::InsertText { .. } => C::BODY.within_paragraph(),
            Op::FormatText { .. } => C::BODY.within_paragraph().limits_formatting(),
            Op::FormatParagraphs { .. } | Op::ToggleList { .. } | Op::SetListStyle { .. } | Op::Indent { .. } => C::BODY.limits_formatting(),
            Op::Delete { .. }
            | Op::SplitParagraph { .. }
            | Op::SetStyle { .. }
            | Op::RestartNumbering { .. }
            | Op::ContinueNumbering { .. }
            | Op::DefineList { .. }
            | Op::ApplyList { .. }
            | Op::SetNumberingValue { .. }
            | Op::SortParagraphs { .. }
            | Op::SetOutlineLevel { .. }
            | Op::MoveBlocks { .. }
            | Op::SetChecked { .. }
            | Op::InsertParagraph { .. }
            | Op::InsertMarkdown { .. }
            | Op::PasteMarkdown { .. }
            | Op::InsertFragment { .. }
            | Op::PasteHtml { .. }
            | Op::SetParagraphText { .. }
            | Op::ReplaceText { .. }
            | Op::DeleteBlocks { .. }
            | Op::ChangeCase { .. }
            | Op::SetFrame { .. }
            | Op::SetDropCap { .. } => C::BODY,
            // Styles, the theme and the document defaults.
            Op::SetStyleDefinition { .. } | Op::SetTheme { .. } | Op::SetStyleSet { .. } | Op::SetParagraphSpacing { .. } => C::BODY.styles().leaves_body(),
            Op::SetDefaultFormat { .. } => C::BODY.styles().leaves_body(),
            Op::CreateStyle { .. } | Op::UpdateStyle { .. } | Op::DeleteStyle { .. } => C::BODY.styles(),
            // Tables.
            Op::InsertTable { .. }
            | Op::TableEdit { .. }
            | Op::TableCells { .. }
            | Op::TableFormat { .. }
            | Op::TableRows { .. }
            | Op::TableColumn { .. }
            | Op::SplitTable { .. }
            | Op::TableCell { .. }
            | Op::SortTable { .. }
            | Op::ConvertToTable { .. }
            | Op::ConvertToText { .. }
            | Op::TableProps { .. }
            | Op::TableDesign { .. } => C::BODY,
            // Things put in the text.
            Op::SetLink { .. }
            | Op::SetImage { .. }
            | Op::InsertImage { .. }
            | Op::InsertField { .. }
            | Op::SetFieldCode { .. }
            | Op::SetFieldResult { .. }
            | Op::InsertEquation { .. }
            | Op::SetEquation { .. }
            | Op::InsertBookmark { .. }
            | Op::DeleteBookmark { .. }
            | Op::RenameBookmark { .. }
            | Op::InsertHorizontalLine { .. }
            | Op::ExpandAltChunk { .. }
            | Op::InsertBuildingBlock { .. } => C::BODY,
            // Breaks, notes, tables of contents, captions, indexes and shapes belong in the main text.
            Op::InsertPageBreak { .. }
            | Op::InsertBreak { .. }
            | Op::InsertFootnote { .. }
            | Op::InsertToc { .. }
            | Op::InsertTocEntry { .. }
            | Op::InsertEndnote { .. }
            // A custom note has a reference like any note: in a header or footer it is a file LibreOffice cannot load.
            | Op::InsertCustomNote { .. }
            | Op::InsertCaption { .. }
            | Op::InsertCrossReference { .. }
            | Op::MarkIndexEntry { .. }
            | Op::InsertIndex { .. }
            | Op::InsertTableOfFigures { .. }
            | Op::InsertShape { .. }
            | Op::InsertTextBox { .. }
            | Op::InsertInk { .. }
            | Op::InsertFreeform { .. }
            | Op::InsertConnector { .. } => C::BODY.main_text(),
            Op::UpdateToc { .. } | Op::SetToc { .. } | Op::RemoveToc { .. } | Op::ConvertNotes { .. } | Op::MarkAllIndexEntries { .. } | Op::SetIndex { .. } | Op::AutoMarkIndex { .. } => C::BODY,
            // Charts.
            Op::InsertChart { .. } => C::BODY.chart(),
            Op::SetChart { .. } => C::BODY.chart().leaves_body(),
            // Pages, headers and footers, and the numbering of notes.
            Op::SetPageSetup { .. }
            | Op::SetColumns { .. }
            | Op::SetLineNumbers { .. }
            | Op::SetPageColor { .. }
            | Op::SetHyphenation { .. }
            | Op::SetPageNumbering { .. }
            | Op::AddHeaderFooter { .. }
            | Op::RemoveHeaderFooter { .. }
            | Op::LinkHeaderFooter { .. }
            | Op::SetTitlePage { .. }
            | Op::SetWatermark { .. }
            | Op::SetPictureWatermark { .. }
            | Op::ReplaceFont { .. }
            | Op::SetDefaultTabStop { .. }
            | Op::SetSection { .. }
            | Op::SetPageBorders { .. }
            | Op::SetLayoutSettings { .. }
            | Op::SetPageBackground { .. }
            | Op::InsertPageNumber { .. }
            | Op::RemovePageNumbers
            | Op::InsertCoverPage { .. }
            | Op::RemoveCoverPage
            | Op::InsertPagePart { .. }
            | Op::SetNoteProps { .. }
            | Op::SetNoteFormat { .. } => C::BODY.layout(),
            // Properties and language.
            Op::SetDocProps { .. } | Op::SetCustomProps { .. } => C::BODY.leaves_body(),
            Op::SetLanguage { ids: None, .. } => C::BODY.leaves_body(),
            Op::SetLanguage { ids: Some(_), .. } | Op::RemoveHiddenData { .. } => C::BODY,
            // The ops with their own enums.
            Op::Control(c) => c.class(),
            Op::FormField(f) => f.class(),
            Op::Protection(p) => p.class(),
            Op::EastAsian(e) => e.class(),
            Op::Tab(t) => t.class(),
            Op::Toa(t) => t.class(),
            // Fields: the update that runs as a file opens, and the one before printing or downloading, change nothing a
            // person wrote, so a read-only document lets them through.
            Op::UpdateFields { open: true, .. } | Op::UpdateFields { quiet: true, .. } => C::BODY.leaves_body().view_only(),
            Op::UpdateFields { .. } => C::BODY.leaves_body(),
            // Sources live in their own part. Citations in notes are edited on the whole document.
            Op::AddSource { .. } | Op::UpdateSource { .. } | Op::DeleteSource { .. } | Op::SetCitationStyle { .. } | Op::UpdateBibliography => C::BODY.leaves_body(),
            Op::InsertCitation { .. } | Op::InsertCitationPlaceholder { .. } | Op::InsertBibliography { .. } => C::BODY,
            Op::EditCitation { .. } | Op::RemoveCitation { .. } => C::BODY.whole_document(),
            // Drawing objects.
            Op::SetShape { .. } | Op::SetObjectOrder { .. } | Op::SetBehindText { .. } | Op::GroupShapes { .. } | Op::UngroupShape { .. } | Op::SetShapePath { .. } | Op::SetConnector { .. } | Op::SetObjectProps { .. } | Op::SetObjectPosition { .. } | Op::MoveObject { .. } => C::BODY,
            // Mail merge: the recipients and settings are not in the body.
            Op::SetMailMerge { .. } | Op::SetMergeRecipients { .. } => C::BODY.leaves_body(),
            Op::InsertMergeField { .. } | Op::UpdateLabels { .. } | Op::MakeLabels { .. } | Op::InsertEnvelope { .. } => C::BODY,
        }
    }
}
