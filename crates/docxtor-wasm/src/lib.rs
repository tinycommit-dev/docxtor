//! WebAssembly bindings. Structured values cross the boundary as JSON strings.

use docxtor_core::{ai, Document, Pos};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct DocxtorDoc {
    doc: Document,
    /// What opening the file did (`docxtor_core::open::Report` as JSON); empty for a new document.
    report: String,
}

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Opening errors carry their kind in the message as `docxtor-open:<code>:<detail>: <text>`.
fn open_err(e: docxtor_core::open::OpenError) -> JsError {
    JsError::new(&format!("docxtor-open:{}:{}: {}", e.code, e.detail, e.message))
}

#[wasm_bindgen]
extern "C" {
    /// A JavaScript array of strings that a render pushes its JSON onto, piece by piece.
    #[wasm_bindgen(typescript_type = "string[]")]
    pub type Pieces;
    #[wasm_bindgen(method)]
    fn push(this: &Pieces, piece: &str);

    /// A JavaScript array of byte arrays that a save pushes its parts onto, piece by piece (each piece copied out).
    #[wasm_bindgen(typescript_type = "Uint8Array[]")]
    pub type BytePieces;
    #[wasm_bindgen(method, js_name = push)]
    fn push(this: &BytePieces, piece: Vec<u8>);
}

/// Rust's panic message to the browser console. A panic in WebAssembly ends in a trap (`RuntimeError: unreachable`) that says
/// nothing about why; this is what says. The page treats the trap as the end of the engine (see `packages/core/src/crash.ts`),
/// so the message is only for the person who reads the console.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Runs once when the module is loaded.
#[wasm_bindgen(start)]
fn start() {
    std::panic::set_hook(Box::new(|info| console_error(&format!("Docxtor engine panic: {info}"))));
}

fn to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

#[wasm_bindgen]
impl DocxtorDoc {
    /// A new blank document.
    #[wasm_bindgen(constructor)]
    pub fn new() -> DocxtorDoc {
        DocxtorDoc { doc: Document::new_blank(), report: String::new() }
    }

    /// A new blank document with a design applied: `{theme?, styleSet?}` as `designSnapshot` gives it.
    #[wasm_bindgen(js_name = withDesign)]
    pub fn with_design(design: &str) -> Result<DocxtorDoc, JsError> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Design {
            theme: Option<docxtor_core::themes::ThemeDef>,
            style_set: Option<docxtor_core::style_sets::StyleSetDef>,
        }
        let d: Design = serde_json::from_str(design).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(DocxtorDoc { doc: Document::new_blank_with_design(d.theme.as_ref(), d.style_set.as_ref()).map_err(err)?, report: String::new() })
    }

    /// Open a file. A Word file opens as it is (a damaged one is repaired, a Strict one read as the standard
    /// kind); an OpenDocument, RTF, web page, old binary Word, Word 2003 XML or Flat OPC file becomes a new
    /// document. `openReport` says what was done.
    pub fn open(bytes: &[u8]) -> Result<DocxtorDoc, JsError> {
        let o = docxtor_core::open::open(bytes, "").map_err(open_err)?;
        Ok(DocxtorDoc { doc: o.doc, report: to_json(&o.report) })
    }

    /// What opening this document did, as JSON: `{format, converted, notes: [{code, detail?, count}]}`.
    /// Empty for a new document.
    #[wasm_bindgen(js_name = openReport)]
    pub fn open_report(&self) -> String {
        self.report.clone()
    }

    pub fn save(&mut self) -> Result<Vec<u8>, JsError> {
        self.doc.save().map_err(err)
    }

    /// Save, recording `now` (an ISO 8601 UTC time from the page's clock) as the modified date when the
    /// document changed, and as the created date of a new document.
    #[wasm_bindgen(js_name = saveStamped)]
    pub fn save_stamped(&mut self, now: &str) -> Result<Vec<u8>, JsError> {
        self.doc.save_stamped(now).map_err(err)
    }

    /// The dates and statistics of `saveStamped`, on their own, for a save that goes on in steps: `stamp`, then `savePackedPieces`,
    /// then `zipPacked` (which needs no document, so it can run in a worker).
    pub fn stamp(&mut self, now: &str) {
        self.doc.stamp(now);
    }

    /// The parts of the document, not compressed yet, for `zipPacked`, pushed onto `pieces` (a JavaScript array) in pieces of at most 1 MB to be joined: each piece goes to the
    /// page as it is made, so the engine's memory, which never shrinks, does not hold a copy of the whole package for the
    /// save (10,000 Word-style paragraphs: 6.6 MB).
    #[wasm_bindgen(js_name = savePackedPieces)]
    pub fn save_packed_pieces(&mut self, pieces: &BytePieces) -> Result<(), JsError> {
        self.doc.save_packed_pieces(1 << 20, &mut |p| pieces.push(p.to_vec())).map_err(err)
    }

    /// Tell the next stamped save who is saving (`cp:lastModifiedBy`; blank for nobody) and how many pages and
    /// lines the page laid the document out in (0 when unknown): they go into `docProps` as Word writes them.
    #[wasm_bindgen(js_name = setSaveInfo)]
    pub fn set_save_info(&mut self, user: &str, pages: u32, lines: u32) {
        self.doc.set_save_info(docxtor_core::document::SaveInfo {
            user: Some(user.to_string()).filter(|u| !u.trim().is_empty()),
            pages: Some(pages).filter(|p| *p > 0),
            lines: Some(lines).filter(|l| *l > 0),
        });
    }

    /// A .docx that holds only the text between two positions of the main text, with the same styles, headers and page
    /// setup, for printing the selection. The open document is not touched.
    #[wasm_bindgen(js_name = selectionCopy)]
    pub fn selection_copy(
        &self,
        from_p: u32,
        from_o: usize,
        to_p: u32,
        to_o: usize,
    ) -> Result<Vec<u8>, JsError> {
        self.doc
            .selection_copy(
                docxtor_core::ops::Pos::new(from_p, from_o),
                docxtor_core::ops::Pos::new(to_p, to_o),
            )
            .map_err(err)
    }

    #[wasm_bindgen(js_name = isModified)]
    pub fn is_modified(&self) -> bool {
        self.doc.is_modified()
    }

    /// The render's JSON, pushed onto `pieces` (a JavaScript array) in pieces to be joined: each piece goes to the page as
    /// it is written, so the engine's memory holds one piece, not the whole render (see `render_json_pieces`).
    #[wasm_bindgen(js_name = renderAll)]
    pub fn render_all(&mut self, pieces: &Pieces) {
        docxtor_core::document::render_json_pieces(|each| self.doc.render_all_each(each), |p| pieces.push(p));
    }

    #[wasm_bindgen(js_name = renderChanged)]
    pub fn render_changed(&mut self, pieces: &Pieces) {
        docxtor_core::document::render_json_pieces(|each| self.doc.render_changed_each(each), |p| pieces.push(p));
    }

    #[wasm_bindgen(js_name = pageSetup)]
    pub fn page_setup(&self) -> String {
        to_json(&self.doc.page_setup())
    }

    /// The bookmarks people can see (not the hidden `_Toc…` ones), in document order.
    pub fn bookmarks(&self) -> String {
        to_json(&self.doc.bookmarks())
    }

    /// Every match of `find` for the options (JSON, `FindOptions`): `{hits, truncated}`. An error says what is wrong
    /// with the pattern.
    pub fn find(&self, find: &str, options: &str) -> Result<String, JsError> {
        let opts: docxtor_core::document::FindOptions = if options.trim().is_empty() { Default::default() } else { serde_json::from_str(options).map_err(err)? };
        Ok(to_json(&self.doc.find(find, &opts).map_err(err)?))
    }

    /// The character formatting at a position, as the formatting to find for "Select similar formatting" (JSON or null).
    #[wasm_bindgen(js_name = findFormatAt)]
    pub fn find_format_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.find_format_at(p, o))
    }

    /// The places of one kind to go to (`heading`, `table`, `graphic`, `equation`, `field`, `footnote`, `endnote`,
    /// `section`) in the main text, in order, as JSON.
    #[wasm_bindgen(js_name = gotoTargets)]
    pub fn goto_targets(&self, kind: &str) -> Result<String, JsError> {
        Ok(to_json(&self.doc.goto_targets(kind).map_err(err)?))
    }

    /// Every block of the main text with its outline level, as JSON (the Outline view and the Navigation pane).
    #[wasm_bindgen(js_name = outlineBlocks)]
    pub fn outline_blocks(&self) -> String {
        to_json(&self.doc.outline())
    }

    /// What a cross-reference can point at (headings, bookmarks, captions, footnotes, endnotes), as JSON.
    #[wasm_bindgen(js_name = xrefTargets)]
    pub fn xref_targets(&self) -> String {
        to_json(&self.doc.xref_targets())
    }

    /// The numbered paragraphs a cross-reference can point at, as JSON `NumberedItem[]`.
    #[wasm_bindgen(js_name = numberedItems)]
    pub fn numbered_items(&self) -> String {
        to_json(&self.doc.numbered_items())
    }

    /// The options of the table of contents holding paragraph `at` (the first one without it), as JSON
    /// `TocOptions`, or undefined when the document has none.
    #[wasm_bindgen(js_name = tocOptions)]
    pub fn toc_options(&self, at: Option<u32>) -> Option<String> {
        self.doc.toc_options(at).map(|o| to_json(&o))
    }

    /// The first paragraph of the table of contents holding paragraph `at` (or the first table), if any.
    #[wasm_bindgen(js_name = tocAt)]
    pub fn toc_at(&self, at: Option<u32>) -> Option<u32> {
        self.doc.toc_at(at)
    }

    /// The options of the index holding paragraph `at` (the first one without it), as JSON `IndexOptions`.
    #[wasm_bindgen(js_name = indexOptions)]
    pub fn index_options(&self, at: Option<u32>) -> Option<String> {
        self.doc.index_options(at).map(|o| to_json(&o))
    }

    /// The options of the table of authorities holding paragraph `at` (or the first one), as JSON; None when there is none.
    #[wasm_bindgen(js_name = toaOptions)]
    pub fn toa_options(&self, at: Option<u32>) -> Option<String> {
        self.doc.toa_options(at).map(|o| to_json(&o))
    }

    /// The names of the 16 table of authorities categories, as JSON (the document's own for 8 to 16).
    #[wasm_bindgen(js_name = toaCategoryNames)]
    pub fn toa_category_names(&self) -> String {
        to_json(&self.doc.toa_category_names())
    }

    /// The next likely legal citation from a position (Mark citation's Next citation): `[from, to]` as JSON, or None.
    #[wasm_bindgen(js_name = nextCitation)]
    pub fn next_citation(&self, p: u32, o: u32) -> Option<String> {
        self.doc.next_citation(Pos::new(p, o as usize)).map(|(a, b)| to_json(&(a, b)))
    }

    /// The document's tabs (Google Docs' document tabs), where each starts, and the tab the file was saved on, as JSON.
    /// Which tab is open belongs to each pane of the editor, not to the document.
    #[wasm_bindgen(js_name = tabsInfo)]
    pub fn tabs_info(&self) -> String {
        to_json(&self.doc.tabs_info())
    }

    /// Say which tab to write into the file at the next save (the one the editor shows). Not an edit: no undo step.
    #[wasm_bindgen(js_name = setOpenTab)]
    pub fn set_open_tab(&mut self, id: &str) -> Result<(), JsError> {
        self.doc.set_open_tab(id).map_err(err)
    }

    /// The building blocks of the glossary document (`word/glossary/document.xml`), as JSON.
    #[wasm_bindgen(js_name = buildingBlocks)]
    pub fn building_blocks(&self) -> String {
        to_json(&self.doc.building_blocks())
    }

    /// A building block as a fragment (JSON), ready for `insert_fragment`.
    #[wasm_bindgen(js_name = buildingBlockFragment)]
    pub fn building_block_fragment(&self, index: usize) -> Option<String> {
        self.doc.building_block_fragment(index).map(|f| to_json(&f))
    }

    /// A document of every design of a gallery, one to a page, in this document's theme, paper and properties.
    /// `labels` is JSON (`GalleryLabels`).
    #[wasm_bindgen(js_name = pageGalleryPreview)]
    pub fn page_gallery_preview(&self, kind: &str, labels: &str) -> Result<Vec<u8>, JsError> {
        let labels: docxtor_core::document::GalleryLabels = serde_json::from_str(labels).unwrap_or_default();
        self.doc.page_gallery_preview(kind, &labels).map_err(|e| JsError::new(&e))
    }

    /// Whether the document starts with a cover page.
    #[wasm_bindgen(js_name = hasCoverPage)]
    pub fn has_cover_page(&self) -> bool {
        self.doc.has_cover_page()
    }

    /// The content controls that came from a gallery (`w:docPartObj`), as JSON.
    #[wasm_bindgen(js_name = docPartControls)]
    pub fn doc_part_controls(&self) -> String {
        to_json(&self.doc.doc_part_controls())
    }

    pub fn styles(&self) -> String {
        to_json(&self.doc.style_list())
    }

    /// The styles for the gallery and the Styles panel, each with how it looks, as JSON.
    #[wasm_bindgen(js_name = styleGallery)]
    pub fn style_gallery(&self) -> String {
        to_json(&self.doc.style_gallery())
    }

    /// One style's definition, own formatting and resolved look as JSON (`null` when there is no such style).
    #[wasm_bindgen(js_name = styleDetails)]
    pub fn style_details(&self, style: &str) -> String {
        to_json(&self.doc.style_details(style))
    }

    /// Where a style is used (paragraph, from, to), as JSON.
    #[wasm_bindgen(js_name = styleUses)]
    pub fn style_uses(&self, style: &str) -> String {
        to_json(&self.doc.style_uses(style))
    }

    /// Which formatting the text at a position has and where each setting comes from, as JSON (`null` for no such paragraph).
    #[wasm_bindgen(js_name = revealFormatting)]
    pub fn reveal_formatting(&self, p: u32, o: u32) -> String {
        to_json(&self.doc.reveal_formatting(docxtor_core::Pos::new(p, o as usize)))
    }

    /// The document's theme (name, colours, fonts) as JSON.
    #[wasm_bindgen(js_name = themeInfo)]
    pub fn theme_info(&self) -> String {
        to_json(&self.doc.theme_info())
    }

    /// The built-in style sets drawn in this document's theme, as JSON.
    #[wasm_bindgen(js_name = styleSetPreviews)]
    pub fn style_set_previews(&self) -> String {
        to_json(&self.doc.style_set_previews())
    }

    /// A style set an extension offers (a `StyleSetDef` as JSON) drawn in this document's theme; an error says what is wrong with it.
    #[wasm_bindgen(js_name = styleSetPreview)]
    pub fn style_set_preview(&self, def: &str) -> Result<String, JsError> {
        let def: docxtor_core::style_sets::StyleSetDef = serde_json::from_str(def).map_err(|e| JsError::new(&e.to_string()))?;
        self.doc.style_set_preview(&def).map(|p| to_json(&p)).map_err(|e| JsError::new(&e))
    }

    /// The document's theme and style set as they are now, as JSON `{theme, styleSet}`: what Set as default keeps.
    #[wasm_bindgen(js_name = designSnapshot)]
    pub fn design_snapshot(&self) -> String {
        let (theme, style_set) = self.doc.design_snapshot();
        to_json(&serde_json::json!({ "theme": theme, "styleSet": style_set }))
    }

    /// The lists the document uses that aren't Docxtor's presets (a gallery's "In this document"), as JSON.
    #[wasm_bindgen(js_name = listDefinitions)]
    pub fn list_definitions(&self) -> String {
        to_json(&self.doc.list_definitions())
    }

    /// The definition of a list as JSON (`null` when there is none).
    #[wasm_bindgen(js_name = listDefinition)]
    pub fn list_definition(&self, num_id: i32) -> String {
        to_json(&self.doc.list_definition(i64::from(num_id)))
    }

    /// Count `n` in a number format, in a language for the formats that spell the number out.
    #[wasm_bindgen(js_name = formatNumber)]
    pub fn format_number(&self, n: i32, format: &str, lang: Option<String>) -> String {
        docxtor_core::numfmt::format(i64::from(n), format, lang.as_deref())
    }

    /// Apply an op or array of ops (JSON). Returns the results as JSON.
    pub fn apply(&mut self, ops: &str, merge: bool) -> Result<String, JsError> {
        self.doc.apply_json(ops, merge).map(|r| to_json(&r)).map_err(err)
    }

    pub fn undo(&mut self) -> bool {
        self.doc.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.doc.redo()
    }

    /// How much memory the undo steps may take together, in bytes (the default is 128 MB). Older steps go when it is passed.
    #[wasm_bindgen(js_name = setUndoBudget)]
    pub fn set_undo_budget(&mut self, bytes: f64) {
        self.doc.set_undo_budget(bytes as usize);
    }

    /// What each undo step did, oldest first, as JSON `["insert_text", "set_format"...]`.
    #[wasm_bindgen(js_name = undoLabels)]
    pub fn undo_labels(&self) -> String {
        to_json(&self.doc.undo_labels())
    }

    /// What each redo step did, the next one first.
    #[wasm_bindgen(js_name = redoLabels)]
    pub fn redo_labels(&self) -> String {
        to_json(&self.doc.redo_labels())
    }

    /// Undo `n` steps in one go (for choosing a point in the undo list).
    #[wasm_bindgen(js_name = undoSteps)]
    pub fn undo_steps(&mut self, n: usize) -> bool {
        self.doc.undo_steps(n)
    }

    #[wasm_bindgen(js_name = redoSteps)]
    pub fn redo_steps(&mut self, n: usize) -> bool {
        self.doc.redo_steps(n)
    }

    /// How many times the document has changed since it was opened (see `Document::revision`).
    #[wasm_bindgen(js_name = revision)]
    pub fn revision(&self) -> f64 {
        self.doc.revision() as f64
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.doc.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.doc.can_redo()
    }

    /// A number that changes when the picture part `path` holds other bytes (part names are reused after an undo).
    #[wasm_bindgen(js_name = mediaGen)]
    pub fn media_gen(&self, path: &str) -> u32 {
        self.doc.media_gen(path)
    }

    pub fn media(&self, path: &str) -> Option<Vec<u8>> {
        self.doc.media(path).map(|b| b.to_vec())
    }

    /// A picture's media as a browser can show it: a kind byte (1 PNG, 2 SVG, 0 cannot be shown) and the bytes; empty
    /// when the media is shown as it is.
    #[wasm_bindgen(js_name = mediaDisplay)]
    pub fn media_display(&self, path: &str) -> Vec<u8> {
        self.doc.media_display_bytes(path)
    }

    /// The pictures nothing in the document uses now, as JSON `["word/media/docxtor3.png", ...]` (they stay for undo).
    #[wasm_bindgen(js_name = unusedMedia)]
    pub fn unused_media(&mut self) -> String {
        to_json(&self.doc.unused_media())
    }

    #[wasm_bindgen(js_name = addMedia)]
    pub fn add_media(&mut self, ext: &str, bytes: Vec<u8>) -> Result<String, JsError> {
        self.doc.try_add_media(ext, bytes).map_err(err)
    }

    /// Who may change the document: `"edit"` (the default) or `"view"`. While it is `"view"` the engine refuses every
    /// edit, undo, redo and added picture ("This document is read-only."), whoever asks.
    #[wasm_bindgen(js_name = setAccess)]
    pub fn set_access(&mut self, access: &str) -> Result<(), JsError> {
        self.doc.set_access(match access {
            "edit" => docxtor_core::Access::Edit,
            "view" => docxtor_core::Access::ViewOnly,
            other => return Err(JsError::new(&format!("unknown access \"{other}\" (use \"edit\" or \"view\")"))),
        });
        Ok(())
    }

    /// `"edit"` or `"view"`.
    pub fn access(&self) -> String {
        match self.doc.access() {
            docxtor_core::Access::Edit => "edit",
            docxtor_core::Access::ViewOnly => "view",
        }
        .to_string()
    }

    /// The picture whose object sits at a position, as JSON (`null` when there is none).
    #[wasm_bindgen(js_name = pictureAt)]
    pub fn picture_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.picture_at(Pos::new(p, o)))
    }

    /// The theme's colours as JSON `[[name, hex], ...]` (`accent1`, `dk2`, ...).
    #[wasm_bindgen(js_name = themeColors)]
    pub fn theme_colors(&self) -> String {
        to_json(&self.doc.theme_colors())
    }

    /// The chart at a position (the object occupying `[o, o+1)`), as JSON; `null` when there is none.
    #[wasm_bindgen(js_name = chartAt)]
    pub fn chart_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.chart_at(Pos::new(p, o)))
    }

    #[wasm_bindgen(js_name = formatAt)]
    pub fn format_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.format_at(Pos::new(p, o)))
    }

    /// The shape or text box whose object sits at a position, as JSON (`null` when there is none).
    #[wasm_bindgen(js_name = shapeAt)]
    pub fn shape_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.shape_at(Pos::new(p, o)))
    }

    /// The connection sites of the shape at a position as JSON: `[{x, y, ang}]` in points from the top-left of
    /// its box (not turned or mirrored), numbered as Word numbers them. Empty when the shape has none.
    #[wasm_bindgen(js_name = shapeSites)]
    pub fn shape_sites(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.shape_sites(Pos::new(p, o)))
    }

    /// The connector between two points as JSON `{prst, x, y, w, h, rot, flipH, flipV, adj}`: the preset and box
    /// Word would use. `ds` and `de` are the directions the ends' sites face (`0, 0` for an end on no shape).
    #[wasm_bindgen(js_name = connectorRoute)]
    #[allow(clippy::too_many_arguments)]
    pub fn connector_route(&self, kind: &str, sx: f64, sy: f64, dsx: i32, dsy: i32, ex: f64, ey: f64, dex: i32, dey: i32) -> Result<String, JsError> {
        docxtor_core::shapes::connector::route_json(kind, (sx, sy), (dsx, dsy), (ex, ey), (dex, dey)).map(|v| v.to_string()).map_err(|e| JsError::new(&e))
    }

    /// Every drawing object of the main text (shapes, text boxes, pictures, charts, groups) as JSON: position,
    /// kind, name, hidden flag and stacking. What the Selection pane lists.
    pub fn objects(&self) -> String {
        to_json(&self.doc.objects())
    }

    #[wasm_bindgen(js_name = textRange)]
    pub fn text_range(&self, p1: u32, o1: usize, p2: u32, o2: usize) -> String {
        self.doc.text_range(Pos::new(p1, o1), Pos::new(p2, o2))
    }

    /// The equation at a position as JSON `{latex, display}`, or `null` when the character there is not one.
    #[wasm_bindgen(js_name = equationAt)]
    pub fn equation_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.equation_at(Pos::new(p, o)))
    }

    /// The phonetic guide at a position as JSON, or `null` when the character there is not one.
    #[wasm_bindgen(js_name = rubyAt)]
    pub fn ruby_at(&self, p: u32, o: usize) -> String {
        to_json(&self.doc.ruby_at(Pos::new(p, o)))
    }

    /// The document's East Asian line-breaking settings as JSON.
    #[wasm_bindgen(js_name = asianSettings)]
    pub fn asian_settings(&self) -> String {
        to_json(&self.doc.asian_settings())
    }

    /// Word's built-in table designs drawn small, as JSON. `look` is a table's `w:tblLook` value (`0x04A0`).
    #[wasm_bindgen(js_name = tableDesigns)]
    pub fn table_designs(&self, look: u32) -> String {
        to_json(&self.doc.table_design_previews(&docxtor_core::table_style::TableLook::from_bits(look)))
    }

    /// A table design an extension offers (one `w:style` as XML) drawn small, as JSON; an error says what is wrong with it.
    #[wasm_bindgen(js_name = tableDesignPreview)]
    pub fn table_design_preview(&self, xml: &str, look: u32) -> Result<String, JsError> {
        self.doc.table_design_preview(xml, &docxtor_core::table_style::TableLook::from_bits(look)).map(|p| to_json(&p)).map_err(|e| JsError::new(&e))
    }

    /// What the document protects and where its editable ranges are, as JSON.
    #[wasm_bindgen(js_name = protectionInfo)]
    pub fn protection_info(&self) -> String {
        to_json(&self.doc.protection_info())
    }

    /// The groups the user counts as for editable ranges (JSON array of strings; `everyone` always applies).
    #[wasm_bindgen(js_name = setUserGroups)]
    pub fn set_user_groups(&mut self, groups: &str) {
        self.doc.set_user_groups(serde_json::from_str(groups).unwrap_or_default());
    }

    // ---- AI ----------------------------------------------------------------

    #[wasm_bindgen(js_name = toolDefinitions)]
    pub fn tool_definitions() -> String {
        ai::tool_definitions().to_string()
    }

    #[wasm_bindgen(js_name = systemGuide)]
    pub fn system_guide() -> String {
        ai::SYSTEM_GUIDE.to_string()
    }

    /// Panics on purpose, to test what the page does when the engine traps. It breaks the engine: every call after it is
    /// unsafe, and the page refuses them. Not for use outside tests.
    #[doc(hidden)]
    #[wasm_bindgen(js_name = __panicForTests)]
    pub fn panic_for_tests(&self) {
        panic!("deliberate panic for tests");
    }

    /// Turn the engine's own check of its rules after every edit on or off (see `docxtor_core::invariants`). For tests:
    /// what an edit breaks is collected, not thrown, and read with `takeInvariantViolations`.
    #[wasm_bindgen(js_name = setInvariantChecks)]
    pub fn set_invariant_checks(&mut self, on: bool) {
        self.doc.set_invariant_checks(on);
    }

    /// The rules edits broke since the last call, as JSON `[{rule, message}]`.
    #[wasm_bindgen(js_name = takeInvariantViolations)]
    pub fn take_invariant_violations(&mut self) -> String {
        to_json(&self.doc.take_invariant_violations())
    }

    /// Run an AI tool. Errors are returned as `Err` so the caller can report them to the model.
    #[wasm_bindgen(js_name = callTool)]
    pub fn call_tool(&mut self, name: &str, args: &str) -> Result<String, JsError> {
        let v: serde_json::Value = serde_json::from_str(args).map_err(err)?;
        ai::call_tool(&mut self.doc, name, &v).map_err(|e| JsError::new(&e))
    }

    pub fn summary(&mut self) -> String {
        ai::summary(&mut self.doc)
    }

    pub fn outline(&mut self) -> String {
        ai::outline(&mut self.doc)
    }

    pub fn docscript(&mut self, limit: usize) -> String {
        ai::docscript(&mut self.doc, &ai::ScriptOpts { start: None, limit })
    }

    /// The first `limit` blocks as DocScript, without rendering the rest.
    #[wasm_bindgen(js_name = docscriptHead)]
    pub fn docscript_head(&self, limit: usize) -> String {
        ai::docscript_head(&self.doc, limit)
    }

    #[wasm_bindgen(js_name = selectionContext)]
    pub fn selection_context(&mut self, p1: u32, o1: usize, p2: u32, o2: usize) -> String {
        ai::selection_context(&mut self.doc, Pos::new(p1, o1), Pos::new(p2, o2))
    }

    /// The font families the document names (font table, theme, styles), as a JSON array: cheap, read before the first layout to load
    /// their faces (see `docxtor_core::fonts::named_families`).
    #[wasm_bindgen(js_name = namedFonts)]
    pub fn named_fonts(&self) -> String {
        serde_json::Value::from(docxtor_core::fonts::named_families(&self.doc.pkg, &self.doc.main_path, &self.doc.styles)).to_string()
    }

    /// Fonts embedded in the document, as JSON `[{ family, style }]` (see `embeddedFontData`).
    #[wasm_bindgen(js_name = embeddedFonts)]
    pub fn embedded_fonts(&self) -> String {
        let list: Vec<_> = docxtor_core::fonts::embedded_fonts(&self.doc.pkg, &self.doc.main_path)
            .into_iter()
            .map(|f| serde_json::json!({ "family": f.family, "style": f.style }))
            .collect();
        serde_json::Value::Array(list).to_string()
    }

    /// Fonts the text uses, as JSON `[{ family, chars }]`, most used first.
    #[wasm_bindgen(js_name = fontsUsed)]
    pub fn fonts_used(&self) -> String {
        to_json(&self.doc.fonts_used())
    }

    /// The font file of the `index`th embedded font, deobfuscated.
    #[wasm_bindgen(js_name = embeddedFontData)]
    pub fn embedded_font_data(&self, index: usize) -> Option<Vec<u8>> {
        docxtor_core::fonts::embedded_fonts(&self.doc.pkg, &self.doc.main_path).into_iter().nth(index).map(|f| f.data)
    }

    /// The text between two body positions as Markdown, or undefined outside the body.
    #[wasm_bindgen(js_name = rangeMarkdown)]
    pub fn range_markdown(&self, p1: u32, o1: usize, p2: u32, o2: usize) -> Option<String> {
        docxtor_core::export::range_to_markdown(&self.doc, Pos::new(p1, o1), Pos::new(p2, o2))
    }

    /// The content between two positions as a fragment (JSON): runs, paragraph properties, styles, lists,
    /// tables, pictures, links and notes, for the Docxtor clipboard format. Undefined for an empty range.
    #[wasm_bindgen(js_name = copyFragment)]
    pub fn copy_fragment(&self, p1: u32, o1: usize, p2: u32, o2: usize) -> Option<String> {
        self.doc.copy_fragment(Pos::new(p1, o1), Pos::new(p2, o2)).map(|f| to_json(&f))
    }

    /// The body as GitHub-flavoured Markdown (footnotes included, headers and footers left out).
    #[wasm_bindgen(js_name = toMarkdown)]
    pub fn to_markdown(&self) -> String {
        docxtor_core::export::to_markdown(&self.doc)
    }

    /// The document as one self-contained web page (HTML5, pictures inline).
    #[wasm_bindgen(js_name = exportHtml)]
    pub fn export_html(&self) -> String {
        docxtor_core::export::html::to_html(&self.doc)
    }

    /// The document as an EPUB 3 book. `modified` is the time to record (ISO 8601, `2026-10-13T09:30:00Z`).
    #[wasm_bindgen(js_name = exportEpub)]
    pub fn export_epub(&self, modified: Option<String>) -> Result<Vec<u8>, JsError> {
        docxtor_core::export::epub::to_epub(&self.doc, modified.as_deref()).map_err(err)
    }

    /// The document as an OpenDocument text file (.odt).
    #[wasm_bindgen(js_name = exportOdt)]
    pub fn export_odt(&self) -> Result<Vec<u8>, JsError> {
        docxtor_core::export::odt::to_odt(&self.doc).map_err(err)
    }

    /// The document as Rich Text (RTF, plain ASCII).
    #[wasm_bindgen(js_name = exportRtf)]
    pub fn export_rtf(&self) -> String {
        docxtor_core::export::rtf::to_rtf(&self.doc)
    }

    /// The body as plain text.
    #[wasm_bindgen(js_name = toText)]
    pub fn to_text(&self) -> String {
        docxtor_core::export::to_text(&self.doc)
    }

    /// Paragraphs, words, characters and characters excluding spaces of the main text, and of the notes and the
    /// text boxes, as JSON `{ words, chars, charsNoSpaces, paragraphs, notes: {...}, boxes: {...} }`. The first four
    /// are the main text's.
    #[wasm_bindgen(js_name = textCounts)]
    pub fn text_counts(&self) -> String {
        let s = self.doc.text_stats();
        let mut v = serde_json::to_value(s.main).unwrap_or_default();
        v["notes"] = serde_json::to_value(s.notes).unwrap_or_default();
        v["boxes"] = serde_json::to_value(s.boxes).unwrap_or_default();
        v.to_string()
    }

    /// What the file carries besides what the page shows, as JSON `[{ id, count, detail, removable }]`.
    pub fn inspect(&self) -> String {
        to_json(&self.doc.inspect())
    }

    #[wasm_bindgen(js_name = wordCount)]
    pub fn word_count(&mut self) -> usize {
        self.doc.word_count()
    }

    /// Diff of the current document against an older .docx, as JSON.
    #[wasm_bindgen(js_name = diffAgainst)]
    pub fn diff_against(&self, older: &[u8]) -> Result<String, JsError> {
        let a = Document::open(older).map_err(err)?;
        Ok(to_json(&docxtor_core::diff::diff_documents(&a, &self.doc)))
    }

    /// Accessibility problems as JSON `[{ kind, severity, p?, o?, message, detail? }]` (see `docxtor_core::a11y`).
    pub fn accessibility(&self) -> String {
        to_json(&self.doc.accessibility())
    }

    /// Every field in the document (main text, headers, footers and notes) as JSON `[{ p, o, instr, result, code,
    /// locked, plain, story }]`.
    #[wasm_bindgen(js_name = listFields)]
    pub fn list_fields(&self) -> String {
        to_json(&self.doc.list_fields())
    }

    /// The result a field instruction would have now (`input` is JSON: `{ now, user_name, file_name, ... }`),
    /// or an empty string when it can't be worked out here.
    #[wasm_bindgen(js_name = fieldPreview)]
    pub fn field_preview(&self, instr: &str, input: &str) -> String {
        let input = serde_json::from_str(input).ok();
        self.doc.field_preview(instr, input.as_ref()).unwrap_or_default()
    }

    /// A field's switches applied to a value the layout worked out (a page number): `\* ROMAN`, `\# "000"`.
    #[wasm_bindgen(js_name = applyFieldSwitches)]
    pub fn apply_field_switches(&self, instr: &str, value: &str) -> String {
        docxtor_core::field_values::apply_switches(instr, value)
    }

    /// The date and time pictures to offer in the document's language, with today's text for each, as JSON
    /// `[[picture, text]]`. `now` is `2026-10-13T09:30:00`.
    #[wasm_bindgen(js_name = dateFormats)]
    pub fn date_formats(&self, now: &str) -> String {
        to_json(&self.doc.date_format_samples(now))
    }

    /// Evaluate a formula body (`SUM(ABOVE)`), in the table holding paragraph `p` when one is given (pass 0 for none).
    /// Returns JSON `{ value }` or `{ error }`.
    #[wasm_bindgen(js_name = evaluateFormula)]
    pub fn evaluate_formula(&self, expr: &str, p: u32) -> String {
        match self.doc.evaluate_formula(expr, (p != 0).then_some(p)) {
            Ok(v) => serde_json::json!({ "value": v }).to_string(),
            Err(e) => serde_json::json!({ "error": e }).to_string(),
        }
    }

    /// What the table dialogs read about the table holding paragraph `p`, as JSON (`null` when it is in no table).
    #[wasm_bindgen(js_name = tableDetails)]
    pub fn table_details(&self, p: u32) -> String {
        to_json(&self.doc.table_details(p))
    }

    /// The formula Word suggests for the table cell holding paragraph `p`: `=SUM(ABOVE)` or `=SUM(LEFT)`.
    #[wasm_bindgen(js_name = suggestFormula)]
    pub fn suggest_formula(&self, p: u32) -> String {
        self.doc.suggest_formula(p)
    }

    /// The document's title, subject, author, keywords and description as JSON (absent ones are left out).
    #[wasm_bindgen(js_name = docProps)]
    pub fn doc_props(&self) -> String {
        to_json(&self.doc.doc_props())
    }

    /// Word-level comparison as JSON `[{ kind, parts: [{ t, text }], p?, index, table? }]`. The open document
    /// is the NEWER version; `older` holds the .docx bytes of the older one.
    #[wasm_bindgen(js_name = compareView)]
    pub fn compare_view(&self, older: &[u8]) -> Result<String, JsError> {
        let old = Document::open(older).map_err(err)?;
        Ok(to_json(&self.doc.compare_view(&old)))
    }

    /// A .docx of the open document (the NEWER version) with tracked changes showing how it differs from
    /// `older` (the .docx bytes of the older version). `date` is ISO 8601, e.g. `2026-10-12T09:30:00Z`.
    pub fn redline(&self, older: &[u8], author: &str, date: &str) -> Result<Vec<u8>, JsError> {
        let old = Document::open(older).map_err(err)?;
        docxtor_core::compare::redline(&old, &self.doc, author, date).map_err(err)
    }

    /// A .docx of the open document (the REVISED version) with what differs from `original` (the .docx bytes of
    /// the original) marked as tracked changes by `author`. Tracked changes the revised document has keep their authors.
    pub fn combine(&self, original: &[u8], author: &str, date: &str) -> Result<Vec<u8>, JsError> {
        let old = Document::open(original).map_err(err)?;
        docxtor_core::compare::combine(&old, &self.doc, author, date).map_err(err)
    }

    /// The document's sources (its bibliography list) as JSON `Source[]`.
    pub fn sources(&self) -> String {
        to_json(&self.doc.sources())
    }

    /// The citation style: `apa`, `mla`, `chicago`, `ieee` or `harvard`, or the id of a CSL style.
    #[wasm_bindgen(js_name = citationStyle)]
    pub fn citation_style(&self) -> String {
        self.doc.citation_style()
    }

    /// Add a CSL style under an id, so `set_citation_style` can choose it. Returns JSON
    /// `{ id, title, note, numeric }`; the error says what is wrong with the style.
    #[wasm_bindgen(js_name = registerCitationStyle)]
    pub fn register_citation_style(&mut self, id: &str, csl: &str) -> Result<String, JsError> {
        self.doc.register_citation_style(id, csl).map(|i| to_json(&i)).map_err(|e| JsError::new(&e))
    }

    /// The CSL styles added so far, as JSON `[{ id, title, note, numeric }]`.
    #[wasm_bindgen(js_name = citationStyles)]
    pub fn citation_styles(&self) -> String {
        to_json(&self.doc.citation_styles())
    }

    /// Whether the document's citations are footnotes (a CSL note style).
    #[wasm_bindgen(js_name = citationsAreNotes)]
    pub fn citations_are_notes(&self) -> bool {
        self.doc.citations_are_notes()
    }

    /// Where the citations are, as JSON `[{ p, o, len, tags, page?, ... }]`.
    pub fn citations(&self) -> String {
        to_json(&self.doc.citations())
    }

    /// The sources as BibTeX text.
    #[wasm_bindgen(js_name = sourcesBibtex)]
    pub fn sources_bibtex(&self) -> String {
        docxtor_core::bibtex::to_bibtex(&self.doc.sources())
    }

    /// The mail merge as JSON `{ active, mainType, sourceName, hasRecipients, source, columns, records, included, previewing, mapping, viewMerged }`.
    #[wasm_bindgen(js_name = mergeInfo)]
    pub fn merge_info(&self) -> String {
        to_json(&self.doc.merge_info())
    }

    /// The recipients of the merge for the list editor as JSON (`null` when there are none): columns, rows, a tick for each, the
    /// filter, the sort, the matched address fields and the records in the merge in order.
    #[wasm_bindgen(js_name = mergeData)]
    pub fn merge_data(&self) -> String {
        to_json(&self.doc.merge_data())
    }

    /// The records that pass a filter, sorted, whatever the ticks say, as JSON indexes into the recipients (`filter` and `sort` are JSON).
    #[wasm_bindgen(js_name = mergeOrder)]
    pub fn merge_order(&self, filter: &str, sort: &str) -> Result<String, JsError> {
        let filter: Vec<docxtor_core::merge::FilterRule> = serde_json::from_str(filter).map_err(err)?;
        let sort: Vec<docxtor_core::merge::SortKey> = serde_json::from_str(sort).map_err(err)?;
        Ok(to_json(&self.doc.merge_order(&filter, &sort)))
    }

    /// The questions the merge will ask (`ASK` and `FILLIN` fields) as JSON.
    #[wasm_bindgen(js_name = mergePrompts)]
    pub fn merge_prompts(&self) -> String {
        to_json(&self.doc.merge_prompts())
    }

    /// Show the record at a position of the merge's list in place of the template (`-1` shows the template again). `answers` is
    /// JSON (`{once, records}`). Returns JSON `{position, count, used, skipped}`, or `null` for the template.
    #[wasm_bindgen(js_name = mergePreview)]
    pub fn merge_preview(&mut self, position: i32, answers: &str) -> Result<String, JsError> {
        let answers: docxtor_core::merge::run::Answers = if answers.trim().is_empty() { Default::default() } else { serde_json::from_str(answers).map_err(err)? };
        let r = self.doc.merge_preview((position >= 0).then_some(position as usize), &answers).map_err(|e| JsError::new(&e))?;
        Ok(to_json(&r))
    }

    /// Positions in the merge's list of the records that hold `query` (in the column `field` when given).
    #[wasm_bindgen(js_name = mergeFind)]
    pub fn merge_find(&self, query: &str, field: &str) -> String {
        to_json(&self.doc.merge_find(query, (!field.is_empty()).then_some(field)))
    }

    /// What an address block, greeting line or merge field code gives for the record at a position, for the dialogs.
    #[wasm_bindgen(js_name = mergeFieldPreview)]
    pub fn merge_field_preview(&self, instr: &str, position: usize) -> String {
        self.doc.merge_field_preview(instr, position)
    }

    /// Merge the document with the recipients into a new document (see `Document::merge_to_docx`). `answers` and `range` are JSON.
    #[wasm_bindgen(js_name = mergeToDocx)]
    pub fn merge_to_docx(&mut self, answers: &str, range: &str) -> Result<MergeOutput, JsError> {
        let answers: docxtor_core::merge::run::Answers = if answers.trim().is_empty() { Default::default() } else { serde_json::from_str(answers).map_err(err)? };
        let range: docxtor_core::document::MergeRange = if range.trim().is_empty() { Default::default() } else { serde_json::from_str(range).map_err(err)? };
        let (bytes, info) = self.doc.merge_to_docx(&answers, &range).map_err(|e| JsError::new(&e))?;
        Ok(MergeOutput { bytes, info: to_json(&info) })
    }

    pub fn checkpoint(&self) -> usize {
        self.doc.checkpoint()
    }

    /// Keep the undo steps after `checkpoint` whatever the memory budget says, until `unpinCheckpoint`.
    #[wasm_bindgen(js_name = pinCheckpoint)]
    pub fn pin_checkpoint(&mut self, checkpoint: usize) {
        self.doc.pin(checkpoint)
    }

    #[wasm_bindgen(js_name = unpinCheckpoint)]
    pub fn unpin_checkpoint(&mut self, checkpoint: usize) {
        self.doc.unpin(checkpoint)
    }

    #[wasm_bindgen(js_name = revertTo)]
    pub fn revert_to(&mut self, checkpoint: usize) -> bool {
        self.doc.revert_to(checkpoint)
    }

    #[wasm_bindgen(js_name = diffSince)]
    pub fn diff_since(&self, checkpoint: usize) -> String {
        to_json(&self.doc.diff_since(checkpoint))
    }
}

/// Word's built-in themes, colour schemes and font schemes, as JSON.
/// The .docx zip of what `savePackedPieces` gave, joined: the same bytes `saveStamped` gives.
#[wasm_bindgen(js_name = zipPacked)]
pub fn zip_packed(packed: &[u8]) -> Result<Vec<u8>, JsError> {
    docxtor_core::package::zip_packed(packed).map_err(err)
}

#[wasm_bindgen(js_name = themeLibrary)]
pub fn theme_library() -> String {
    to_json(&docxtor_core::themes::library())
}

/// The built-in style sets (id, name, description), as JSON.
#[wasm_bindgen(js_name = styleSetInfos)]
pub fn style_set_infos() -> String {
    to_json(&docxtor_core::style_sets::style_set_infos())
}

/// Word's paragraph spacing presets, as JSON `[{id, name, before, after, line}]`.
#[wasm_bindgen(js_name = spacingPresets)]
pub fn spacing_presets() -> String {
    to_json(&docxtor_core::style_sets::SPACING_PRESETS.iter().map(|p| serde_json::json!({"id": p.0, "name": p.1, "before": p.2, "after": p.3, "line": p.4})).collect::<Vec<_>>())
}

/// LaTeX to MathML for a live preview of an equation. The error says what is wrong with the LaTeX.
#[wasm_bindgen(js_name = latexToMathml)]
pub fn latex_to_mathml(latex: &str, display: bool) -> Result<String, JsError> {
    docxtor_core::math::latex_to_mathml(latex, display).map_err(|e| JsError::new(&e))
}

/// The rows of an AutoMark file (a .docx with a table of two columns) as JSON `AutoMarkEntry[]`.
#[wasm_bindgen(js_name = readAutoMarkFile)]
pub fn read_auto_mark_file(bytes: &[u8]) -> Result<String, JsError> {
    docxtor_core::automark::read(bytes).map(|e| to_json(&e)).map_err(|e| JsError::new(&e))
}

/// A new AutoMark file as .docx bytes: a table of two columns with the given entries (JSON, or two examples when empty).
#[wasm_bindgen(js_name = newAutoMarkFile)]
pub fn new_auto_mark_file(entries: &str) -> Result<Vec<u8>, JsError> {
    let list: Vec<docxtor_core::ops::AutoMarkEntry> = serde_json::from_str(entries).unwrap_or_default();
    docxtor_core::automark::template(&list).map_err(|e| JsError::new(&e))
}

/// Read BibTeX text into sources, as JSON `Source[]` (without tags). Errors say what is wrong.
#[wasm_bindgen(js_name = parseBibtex)]
pub fn parse_bibtex(text: &str) -> Result<String, JsError> {
    docxtor_core::bibtex::parse(text).map(|s| to_json(&s)).map_err(|e| JsError::new(&e))
}

/// Read CSL-JSON (Zotero, Mendeley, doi.org) into sources, as JSON `Source[]` (without tags).
#[wasm_bindgen(js_name = parseCslJson)]
pub fn parse_csl_json(text: &str) -> Result<String, JsError> {
    docxtor_core::csl_json::parse(text).map(|s| to_json(&s)).map_err(|e| JsError::new(&e))
}

/// A chart (JSON model) with chart options (JSON) merged in, as the engine would apply them: the Edit chart dialog
/// draws its preview from this. The error says what is wrong with the options.
#[wasm_bindgen(js_name = chartMerge)]
pub fn chart_merge(model: &str, options: &str) -> Result<String, JsError> {
    let m: docxtor_core::chart::ChartModel = serde_json::from_str(model).map_err(|e| JsError::new(&e.to_string()))?;
    let o: serde_json::Value = serde_json::from_str(options).map_err(|e| JsError::new(&e.to_string()))?;
    docxtor_core::chart::format::merge(&m, &o).map(|m| to_json(&m)).map_err(|e| JsError::new(&e))
}

/// HTML (from Word, Google Docs, a web page) as a fragment (JSON). `images` is a JSON object of picture
/// addresses to `data:` addresses or base64, for pictures the page fetched.
#[wasm_bindgen(js_name = htmlToFragment)]
pub fn html_to_fragment(html: &str, images: &str) -> String {
    to_json(&docxtor_core::html_import::html_to_fragment(html, &decode_images(images)))
}

/// The addresses of the pictures in some HTML that are not `data:` addresses, as a JSON array.
#[wasm_bindgen(js_name = htmlImageSources)]
pub fn html_image_sources(html: &str) -> String {
    to_json(&docxtor_core::html_import::html_image_sources(html))
}

/// RTF (from Word, WordPad, LibreOffice) as a fragment (JSON).
#[wasm_bindgen(js_name = rtfToFragment)]
pub fn rtf_to_fragment(rtf: &str) -> String {
    to_json(&docxtor_core::rtf::rtf_to_fragment(rtf))
}

/// Give this engine a secret (random, at least 16 characters) that marks fragments it copies, so a paste can tell
/// Docxtor's own clipboard content from anyone else's. The first call wins.
#[wasm_bindgen(js_name = setClipboardNonce)]
pub fn set_clipboard_nonce(nonce: &str) -> bool {
    docxtor_core::fragment::set_session_nonce(nonce)
}

/// The text of a fragment (JSON): paragraphs on lines, table cells separated by tabs.
#[wasm_bindgen(js_name = fragmentText)]
pub fn fragment_text(fragment: &str) -> Result<String, JsError> {
    docxtor_core::fragment::Fragment::from_json(fragment).map(|f| f.plain_text()).map_err(|e| JsError::new(&e))
}

/// A JSON object of address to base64 (or `data:` address) into address to bytes.
fn decode_images(json: &str) -> std::collections::HashMap<String, Vec<u8>> {
    let map: std::collections::HashMap<String, String> = serde_json::from_str(json).unwrap_or_default();
    map.into_iter()
        .filter_map(|(k, v)| {
            let data = v.split_once(";base64,").map(|(_, d)| d).unwrap_or(&v);
            docxtor_core::fragment::b64_decode(data).map(|b| (k, b))
        })
        .collect()
}

/// Paragraph-level diff between two .docx files (older first), as JSON.
#[wasm_bindgen(js_name = diffDocuments)]
pub fn diff_documents(older: &[u8], newer: &[u8]) -> Result<String, JsError> {
    let a = Document::open(older).map_err(err)?;
    let b = Document::open(newer).map_err(err)?;
    Ok(to_json(&docxtor_core::diff::diff_documents(&a, &b)))
}

/// A merged document and what the merge did.
#[wasm_bindgen]
pub struct MergeOutput {
    bytes: Vec<u8>,
    info: String,
}

#[wasm_bindgen]
impl MergeOutput {
    /// The new .docx.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// JSON `{records, copies, skipped}`.
    #[wasm_bindgen(getter)]
    pub fn info(&self) -> String {
        self.info.clone()
    }
}

/// The code of a merge field or rule, written as Word writes it, from JSON (`{"kind":"address_block",...}`), for dialogs to show.
#[wasm_bindgen(js_name = mergeFieldInstr)]
pub fn merge_field_instr(spec: &str) -> Result<String, JsError> {
    let spec: docxtor_core::merge::instr::MergeFieldSpec = serde_json::from_str(spec).map_err(err)?;
    docxtor_core::merge::instr::instruction_of(&spec).map_err(|e| JsError::new(&e))
}

/// Read a list of recipients from a file (CSV, TSV, text or .xlsx) as JSON `{source, columns, rows, sheets, truncated}`.
#[wasm_bindgen(js_name = readRecipients)]
pub fn read_recipients(bytes: &[u8], name: &str, sheet: &str) -> Result<String, JsError> {
    docxtor_core::merge::read_recipients(bytes, name, (!sheet.is_empty()).then_some(sheet)).map(|r| to_json(&r)).map_err(|e| JsError::new(&e))
}

/// Docxtor's cover pages, headers or footers (`kind`: `cover`, `header`, `footer`), as JSON.
#[wasm_bindgen(js_name = pageGallery)]
pub fn page_gallery(kind: &str) -> String {
    to_json(&Document::page_gallery(kind))
}

/// The label products Docxtor knows (a sheet's size and its labels', in twips) as JSON.
#[wasm_bindgen(js_name = labelProducts)]
pub fn label_products() -> String {
    to_json(&docxtor_core::merge::labels::products())
}

/// Word's envelope sizes as JSON `[{id, name, w, h}]` (twips).
#[wasm_bindgen(js_name = envelopeSizes)]
pub fn envelope_sizes() -> String {
    to_json(&docxtor_core::merge::envelope::sizes())
}

/// Word's address fields (the names Match Fields maps columns to) and the formats for a name in an address block, as JSON.
#[wasm_bindgen(js_name = mergeVocabulary)]
pub fn merge_vocabulary() -> String {
    let fields: Vec<&str> = docxtor_core::merge::address::STANDARD_FIELDS.iter().map(|(n, _)| *n).collect();
    let names: Vec<(&str, &str)> = docxtor_core::merge::instr::address_name_formats();
    serde_json::json!({ "fields": fields, "nameFormats": names }).to_string()
}

/// Compare two .docx files and return Docxtor's fidelity report as JSON.
#[wasm_bindgen(js_name = fidelityReport)]
pub fn fidelity_report(original: &[u8], other: &[u8]) -> Result<String, JsError> {
    let a = docxtor_core::package::Package::open(original).map_err(err)?;
    let b = docxtor_core::package::Package::open(other).map_err(err)?;
    Ok(to_json(&docxtor_core::fidelity::compare(&a, &b)))
}

/// Whether a .docx package is a document or a template, and whether it has macros, as JSON `{"template", "macros"}`.
#[wasm_bindgen(js_name = packageKind)]
pub fn package_kind(bytes: &[u8]) -> Result<String, JsError> {
    let k = docxtor_core::template::kind_of(bytes).map_err(err)?;
    Ok(serde_json::json!({ "template": k.template, "macros": k.macros }).to_string())
}

/// The same package as a Word template (.dotx, or .dotm with macros): only the main part's content type changes.
#[wasm_bindgen(js_name = packageAsTemplate)]
pub fn package_as_template(bytes: &[u8]) -> Result<Vec<u8>, JsError> {
    docxtor_core::template::as_template(bytes).map_err(err)
}

/// The same package as a document (.docx, or .docm with macros). With `now` (an ISO 8601 UTC time) it is also a new
/// document, made at that time, as Word makes one from a template.
#[wasm_bindgen(js_name = packageAsDocument)]
pub fn package_as_document(bytes: &[u8], now: Option<String>) -> Result<Vec<u8>, JsError> {
    docxtor_core::template::as_document(bytes, now.as_deref()).map_err(err)
}

/// Whether the bytes are a password-protected Office file (a compound file holding `EncryptedPackage`).
#[wasm_bindgen(js_name = isEncrypted)]
pub fn is_encrypted(bytes: &[u8]) -> bool {
    docxtor_core::office_crypt::is_encrypted(bytes)
}

/// What kind of file the bytes are, by their content: `word`, `odt`, `flat-opc`, `word-2003`, `rtf`, `html`, `doc`,
/// `encrypted`, `foreign:<what>` (a spreadsheet, a presentation, a PDF...) or `unknown`.
#[wasm_bindgen(js_name = sniffFormat)]
pub fn sniff_format(bytes: &[u8]) -> String {
    use docxtor_core::open::Kind;
    match docxtor_core::open::sniff(bytes) {
        Kind::Word => "word".into(),
        Kind::Odt => "odt".into(),
        Kind::FlatOpc => "flat-opc".into(),
        Kind::Word2003 => "word-2003".into(),
        Kind::Rtf => "rtf".into(),
        Kind::Html => "html".into(),
        Kind::Doc => "doc".into(),
        Kind::Encrypted => "encrypted".into(),
        Kind::Foreign(what) => format!("foreign:{what}"),
        Kind::Unknown => "unknown".into(),
    }
}

/// A password and the keys made from it, kept for as long as a document is open. The page keeps the password only
/// in memory. Errors carry their kind in the message as `docxtor-crypt:<code>: <text>`.
#[wasm_bindgen]
pub struct DocxtorCrypt {
    session: docxtor_core::office_crypt::CryptSession,
    package: Option<Vec<u8>>,
}

fn crypt_err(e: docxtor_core::office_crypt::CryptError) -> JsError {
    JsError::new(&format!("docxtor-crypt:{}: {e}", e.code()))
}

#[wasm_bindgen]
impl DocxtorCrypt {
    /// A session for a password that has not been used on a file yet (keys are made on the first `encrypt`).
    #[wasm_bindgen(constructor)]
    pub fn new(password: &str) -> DocxtorCrypt {
        DocxtorCrypt { session: docxtor_core::office_crypt::CryptSession::new(password), package: None }
    }

    /// Decrypt a protected file. The package comes out of `takePackage`; the session saves again with the file's own keys.
    pub fn open(bytes: &[u8], password: &str) -> Result<DocxtorCrypt, JsError> {
        let (package, session) = docxtor_core::office_crypt::open(bytes, password).map_err(crypt_err)?;
        Ok(DocxtorCrypt { session, package: Some(package) })
    }

    /// The decrypted .docx bytes of `open` (once).
    #[wasm_bindgen(js_name = takePackage)]
    pub fn take_package(&mut self) -> Vec<u8> {
        self.package.take().unwrap_or_default()
    }

    /// How many random bytes `encrypt` needs at most; pass at least this many from `crypto.getRandomValues`.
    #[wasm_bindgen(js_name = randomLen)]
    pub fn random_len() -> usize {
        docxtor_core::office_crypt::CryptSession::RANDOM_MAX
    }

    /// A protected file holding the .docx bytes `zip`. `random` is cryptographically random bytes from the page.
    pub fn encrypt(&mut self, zip: &[u8], random: &[u8]) -> Result<Vec<u8>, JsError> {
        if random.len() < docxtor_core::office_crypt::CryptSession::RANDOM_MAX {
            return Err(JsError::new("not enough random bytes"));
        }
        let mut at = 0;
        let mut rng = |buf: &mut [u8]| {
            buf.copy_from_slice(&random[at..at + buf.len()]);
            at += buf.len();
        };
        Ok(self.session.encrypt(zip, &mut rng))
    }
}

impl Default for DocxtorDoc {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether these ops (JSON, one op or an array) are all ones a view-only document still applies (`Op::is_view_only`):
/// the field update that runs as a file opens. False for no ops, and for anything that does not parse.
#[wasm_bindgen(js_name = opsAreViewOnly)]
pub fn ops_are_view_only(ops: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(ops) else { return false };
    let list: Vec<serde_json::Value> = match v {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    !list.is_empty() && list.into_iter().all(|o| serde_json::from_value::<docxtor_core::Op>(o).is_ok_and(|op| op.is_view_only()))
}

/// Counters for the performance harness, as JSON: `{"bodyClones": n, "blockClones": n, "paragraphsWalked": n, "blocksWritten": n}`.
#[wasm_bindgen(js_name = perfCounters)]
pub fn perf_counters() -> String {
    format!("{{\"bodyClones\":{},\"blockClones\":{},\"paragraphsWalked\":{},\"blocksWritten\":{}}}", docxtor_core::perf::body_clones(), docxtor_core::perf::block_clones(), docxtor_core::perf::paragraphs_walked(), docxtor_core::perf::blocks_written_total())
}

/// Turn on a planted regression: code 1 `busy` (loop iterations per edit), 2 `alloc_mb` (on open), 3 `double_clone`.
/// Only in a build made with the `perf-plant` feature. Two numbers, so a page can call the raw export without glue.
#[cfg(feature = "perf-plant")]
#[wasm_bindgen(js_name = setPerfPlant)]
pub fn set_perf_plant(code: u32, value: f64) {
    let name = match code {
        1 => "busy",
        2 => "alloc_mb",
        _ => "double_clone",
    };
    docxtor_core::perf::set_plant(name, value as u64);
}
