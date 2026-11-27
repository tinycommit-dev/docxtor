# Engine modules: content features

The modules of `docxtor-core` for what a document contains beyond text and formatting: fields, indexes and tables of contents, find and replace, mail merge, citations, content controls, protection, bookmarks, comparison, pictures, shapes, equations, charts and document tabs. Part of the [architecture](index.md) pages; [how the engine works](engine-concepts.md) comes first. The other module pages are [files and the model](engine-files-and-model.md), [rendering and layout data](engine-rendering-and-layout.md) and [AI tools and fidelity](engine-ai-and-fidelity.md).

## Fields

- `fields`: Word's fields. It parses instructions, handles hyperlink fields (`hyperlink_target`; editing one with the link editor turns it into a `w:hyperlink`), complex fields (`w:fldChar`) and `w:fldSimple` found inside paragraphs, has builders, and replaces results (only plain-text results are replaced, and the first result run's formatting is kept). It counts `SEQ`.
- `field_values`: works out a field's value from the document (core, app and custom properties, document variables, `SET` values, bookmarks, tables, the page and section) and applies its switches. The host passes in a `FieldInput` with the time, user, file name and size and page count.
- `field_fmt`: the formatting of values: `\*` case and number words, `\#` numeric pictures, `\@` date-time pictures, date names for several languages, and the formats the Date and time dialog offers.
- `formula`: evaluates `= expression` with SUM, AVERAGE, IF, ROUND and the rest, table references (ABOVE, LEFT, A1, A1:B3) and bookmarks, against a `Context`. `Document::evaluate_formula` is the entry point for table formulas.
- `field_eq`: turns `EQ` fields into MathML.
- `document/field_ops.rs` is the document side: updating value fields inside out (so a field nested in another's instruction is worked out first), `InsertField`, `SetFieldCode`, `SetFieldResult`, `list_fields`, `field_preview`, and the skipped undo step for updates on open and before output. Locked (`w:fldLock`) fields stay; dirty ones (`w:dirty`) are updated and cleared.

## Indexes, contents and references

- `index`: `XE` entry marks and the `INDEX` field. It sorts case- and accent-insensitively with the language's own letters, handles sub-entries, page ranges and the `\e \h \r \l \g \f \c \z` switches (`IndexOptions` is what the dialog sets), and builds the paragraphs of the result.
- `automark`: reads and writes Word's AutoMark file, a document with one table of two columns. The first column holds text to find and the second the index entry for it (`Fruit:Apple` is the sub-entry Apple under Fruit; empty means the text itself).
- `toc` and `toc_spec`: the table of contents. `toc` builds the result paragraphs (links, tab leaders, page numbers on or off); `toc_spec` has the `TOC` switches (`Spec`, `TocOptions`) and `TC` fields.
- `toa`: the table of authorities. `TA` fields mark citations in the text and a `TOA` field lists them by category with page numbers, as Word writes it.
- `document/toc_ops` has the contents ops: options, entries from styles, outline levels and `TC` fields, leader and look written into the `TOC` and `Index` styles, update page numbers only, and remove. It also has the index ops (`set_index`, AutoMark).
- `document/chapters` handles chapter numbers in captions (`STYLEREF \s`, `SEQ \s`) and numbered-item numbers for cross-references.
- `document/glossary` handles the glossary document's building blocks, read without touching the part, with `docPartObj` content controls.
- `document/references` and `document/updating` hold the ops for endnotes, captions, cross-references, updating fields, the index and tables of figures, plus `Document::xref_targets()`. They are child modules of `document`, so they reach its private state.
- `marks`: bookmarks (names, ids, and renaming along with the links and fields that point at them), the horizontal-line paragraph, and watermarks. A text watermark is Word's WordArt shape and an image watermark its picture shape, both in the headers and each a zero-width marker in header text. `Op::SetPictureWatermark` relates the image to every header, and `save` leaves out image relationships and parts that no header uses any more, so undo still finds them.

## Find and go to

- `matcher`: the pattern matcher behind find and replace, with no outside crate. It is a backtracking engine over the document's characters (each knows if it is a paragraph mark, a footnote reference, a picture, or in a field). It reads plain text with Word's `^` codes, Word's wildcards and JavaScript regular expressions, with a step limit so a bad pattern ends instead of hanging.
- `document/find.rs` turns paragraphs into that text (a run of sibling paragraphs is one text, so a match can cross a paragraph mark), searches the body and every story (`Document::find`), and turns a match into the same ops typing makes (`replace_text`: insert, delete, split, format), so it undoes in one step.
- `document/goto.rs` lists the places of a kind (headings, tables, pictures, equations, fields, notes, sections) for Go to.

## Mail merge

`merge` covers mail merge, envelopes and labels.

- `csv` and `xlsx` read lists of recipients. The zip and XML readers are the package's own.
- `instr` writes the merge fields as Word's complex fields.
- `address` lays out an address block and a greeting line.
- `run` merges a document with records. It is a pure function of the template's blocks and the list (`Merger::unit`), and uses `fields::update_inner_first` so a field nested in an `IF` is worked out first.
- `state` writes Word's `w:mailMerge`, kept as Word wrote it where Docxtor does not manage it, and keeps the recipients in a custom XML part.
- `labels` and `envelope` build a sheet of labels (a table with Word's geometry) and an envelope section.
- `document/mail_merge.rs` has the ops, the record shown in place of the template (the template is parked and saved instead, and the document cannot be edited meanwhile) and `merge_to_docx`.

## Citations and bibliographies

- `citations`: sources (Word's `b:Source` in `customXml/item1.xml`, with its properties part, relationship and content type), the `CITATION` field instruction, and `Document::sources`, `citation_style` and `citations`.
- `cite_fields`: finds and rewrites `CITATION` and `BIBLIOGRAPHY` fields in the model and builds the content controls Word wraps them in. `document/cite.rs` holds the ops.
- `cite_styles`: APA 7, MLA 9, Chicago 17 (author-date), IEEE and Harvard, giving in-text citations and reference-list entries as plain and italic runs, with sorting, 2020a letters and IEEE numbers.
- `csl`: a CSL 1.0.2 processor. `model` reads a style, `eval` formats one item (text, names, dates, numbers, labels, groups, conditions), and `process` works out numbers, order, positions (first, subsequent, ibid), year suffixes and collapsed numbers for a whole document. `names`, `text`, `locale` and `vars` are its parts. `document/cite.rs` registers styles per document and turns citations into footnotes for note styles.
- `bibtex` and `csl_json`: read BibTeX and CSL-JSON into sources, and write BibTeX.

## Content controls, forms and protection

- `controls`: content controls (`w:sdt`). It reads what one is from its `w:sdtPr` (`RControl`: kind, title, tag, colour, lock, placeholder, entries, date format, check box state), builds the markup Word writes for each kind (including the placeholder building blocks in `word/glossary/document.xml`), formats dates, and defines the `control` ops: insert, props, set, remove and repeat.
- `binding`: reads and writes the custom XML node a control is bound to.
- `forms`: Word's older form fields (`FORMTEXT`, `FORMCHECKBOX`, `FORMDROPDOWN` with their `w:ffData`) and the `form_field` ops.
- `protection`: `w:documentProtection` (the SHA-512 password hash Word writes, and the old 16-bit one it still reads), `_MarkAsFinal` in `docProps/custom.xml`, and the `protection` ops.
- `document/controls.rs` applies controls. Placeholder text and bound values are put right after every edit: a control that was showing its placeholder and now holds typed text stops showing it, and an emptied one shows it again. Locks are checked by comparing the locked controls before and after a batch. Repeating sections copy an item with fresh ids and clean controls, also for table rows.
- `document/protect.rs` enforces protection the same way. The text and markup outside the editable ranges (or, for forms, outside controls and form fields, and sections with `formProt` off) are written out before and after a batch, and the batch is undone when they differ. The editor, the AI tools, the CLI and the MCP server are all held to it.

## Comparing documents

- `diff`: paragraph-level comparison of two documents, used for version history, reviews and `docxtor diff`. Paragraphs are lined up with an LCS.
- `compare` builds on it. `compare_view` gives word-level changes per paragraph and formatting changes by name, for display. `redline` gives the newer document with Word tracked changes against the older one, with formatting changes as `w:rPrChange` and `w:pPrChange`. `combine` gives the revised document with what differs from the original marked, keeping the tracked changes it already has with their authors. Removed and added paragraphs are paired by shared words, and changed ones are diffed per word.

## Pictures

- `image`: a picture's size, wrapping, alignment and placement numbers (`ImagePatch`: `set_image`).
- `picture`: reads and writes the rest: crop (`a:srcRect`), turn and flips, the outline, brightness and contrast (`a:lum`), transparency, recolouring (`a:grayscl`, `a:biLevel`, `a:duotone`), a colour made transparent (`a:clrChange`), the picture's shape (`a:prstGeom`), shadow, glow, reflection and soft edges (`a:effectLst`), `wp:effectExtent` (kept in step with the turn and effects), and Reset picture. A cropped picture keeps its scale: the size and an offset position follow the crop.
- The view model gets `RImage.pic` (`PicFmt`) and `RImage.place` (distances, text sides, lock, overlap). `Document::picture_at` returns one picture and `theme_colors` the theme.
- Office 2010 picture corrections (`a14:imgProps`) are drawn only when the blip is the original, since Word puts the corrected picture in the blip itself. A linked picture (`r:link`) is `pic.link`, and one with an SVG version (`asvg:svgBlip`) is `pic.svg`.
- `imgconv`: converts pictures a browser cannot show, for display only (the file keeps the original). TIFF (baseline, LZW, PackBits, Deflate, CCITT) becomes PNG, and EMF and WMF become SVG through Docxtor's own renderer of the GDI records (shapes, paths, text, bitmaps, clipping, transforms). `Document::media_display` is what `Engine.mediaUrl` shows.

## Shapes and text boxes

`shapes/` covers Word shapes and text boxes (`wps`, `wpg`, `wpc` and VML). Its parts:

- `read`: the view model (`RShape`). `story`: each `w:txbxContent` as a `TextBox` story.
- `build`: new shapes as Word writes them, with the VML fallback.
- `patch`: `SetShape` on existing markup. `member`: `SetShape` with a `member` path, for one member of a group or canvas in the group's own coordinates, keeping the group's box and VML copy in step.
- `order`: the stacking order (`relativeHeight`, `behindDoc` and the VML `z-index`). `group`: grouping, ungrouping and freehand drawings.
- `theme`: theme colours, DrawingML colour transforms, and the theme's fill and effect styles that `wps:style` refers to.
- `appearance`: the view model of how a shape looks (`RFill`'s stops, picture and pattern parts, `REffects`) and the op inputs `FillSpec`, `EffectsSpec` and `BodyPatch`.
- `fill`: reads `a:solidFill`, `a:gradFill`, `a:blipFill`, `a:pattFill` and VML `v:fill`, and writes a fill from an op as DrawingML plus the VML that says the same.
- `effects`: reads `a:effectLst`, `a:scene3d` and `a:sp3d`, the shape's own or the theme style's, and writes them in Word's order. It edits an existing effect in place and writes the theme's effects out before an own list replaces them. The VML copy gets `v:shadow`.
- `body`: the text frame: `wps:bodyPr` insets, anchor, wrap, autofit, vertical text, columns, and the VML text box's `inset`, `mso-fit-shape-to-text` and `layout-flow`.
- `freeform`: a shape with its own outline. Path commands `M`, `L`, `C`, `Z` (and `S`, as input only) become `a:custGeom` with a connection site at each corner and the VML `path`. `RPath::cmds` reads the outline back, with arcs and quadratic curves as cubic ones. `plan` works out a new outline and how the shape's box follows its bounds for "Edit points".
- `connector`: connection sites of the presets in the order of `cxnLst`, `a:stCxn` and `a:endCxn`, and the choice of `straightConnector1`, `bentConnector2` to `5` or `curvedConnector2` to `5` for two sites, with the turn and mirroring Word writes, plus the VML copy.
- `ai`: the shape tools.

The document side:

- `document/geometry.rs` has `insert_freeform` and `set_shape_path`.
- `document/connectors.rs` has `insert_connector` and `set_connector`, and keeps connectors attached. `apply_batch` notes where the sites that connectors hold are before the ops and compares them after. A connector whose shape moved, was resized, turned, flipped or regrouped is routed again in the same undo step; one whose shape is gone lets go of it; one that was itself moved lets go of both.
- `document/shapes.rs` has the ops and keeps stories and shapes in step.
- `document/arrange.rs` has `set_object_order`, `set_behind_text`, `group_shapes`, `ungroup_shape` and `insert_ink`. A pen can have an opacity and square or flat ends, for a highlighter or a pencil.
- `object_props` covers what every drawing object has in common: the name in `wp:docPr`, `hidden="1"`, the VML `id` and `visibility`, `wp:positionH` and `wp:positionV` with `relativeFrom` and `wp:align`, and `ObjectInfo`, one row of the Selection pane. `stack_move` puts an object directly above or below another in its stack. `document/object_props.rs` uses both and has `set_object_props`, `set_object_position`, `move_object` and `Document::objects()`, every drawing object of the text in document order (the `objects` export of the browser engine).
- A hidden object is marked on its render segment (`RSeg::hidden`) and the editor does not draw it.

### SmartArt and WordArt

`shapes/smartart`, `shapes/smartart_layout` and `shapes/warp` draw SmartArt and WordArt from the file; they are not edited.

- A SmartArt graphic (`dgm:relIds` in a drawing) is an `RShape` of kind `diagram`. Its children are the shapes of the drawing Word saves with it (the part the data model names in `dsp:dataModelExt`), read with the shape readers plus their text as read-only `RDText` (runs with size, weight, colour and font; the `dsp:txXfrm` rectangle, insets and anchor).
- The parts are never written. A resize changes `wp:extent` only, and the drawing scales by the extent over the size it was made for. The first resize notes that size in a Docxtor extension of `wp:docPr`, which Word ignores.
- Without the drawing part, `smartart_layout` lays out the nodes of the data model: basic block list, basic process, basic cycle, vertical bullet list, hierarchy and organization chart. Any other layout is the block list. Colours come from the colours part or the theme accent. With nothing to lay out, a placeholder box names the graphic and lists the texts.
- WordArt is `RShape.warp` (`RWarp`). For DrawingML it is the preset of `a:prstTxWarp` with its adjust values and the run's `w14:textFill`, `w14:textOutline`, `w14:shadow`, `w14:glow` and `w14:reflection`; the text stays the shape's story. For VML it is the `v:textpath` string and font, and the shape's fill and stroke as the text's (`warp::PRESETS`, `o:spt` 136 to 175). Text watermarks stay markers in the header (`marks`).

## Equations

`math` handles equations:

- OMML to MathML Core (`omml_to_mathml`, for the browser to draw);
- LaTeX to OMML (`latex_to_omml`, with plain error messages);
- OMML back to LaTeX (`omml_to_latex`, so Word's equations can be edited);
- the AI tools `insert_equation` and `set_equation`.

`document/equations.rs` holds the ops. An equation stays the `m:oMath` or `m:oMathPara` element Word wrote, so an untouched one is written back byte for byte.

## Charts

`chart/` covers charts:

- `read` turns a chart part (`word/charts/chartN.xml`) into a `ChartModel` from its cached data: kinds, grouping, series with colours resolved against the theme, legend, axes, data labels, combination charts and a kind per series.
- `write` changes an existing part in place and keeps everything else. It works in two steps: the classic parameters (title, type, data caches and formulas, legend, labels, stacking), then `options`.
- `format` holds the `options` machinery. `merge` turns an options object (a merge patch over the model, with `look` and `palette` applied first) into the model the chart should have. `write` compares that model with what the part says now and edits only the elements that differ, in schema order: axes with titles, scale, units, gridlines, ticks and number formats; legend; data labels; trendlines; error bars; colours and fills; `c:style`.
- `palette` has the eight looks and Word's 17 colour palettes, and writes `colorsN.xml`.
- `build` writes a clean chart the way Word writes one for each kind: bubble, stock (high-low-close to volume-open-high-low-close), surface (3-D, wireframe, contour) and combination charts (a group per type, a second pair of axes for series on the second value axis).
- `xlsx` writes a minimal workbook of the data (sheet1: categories in column A, a series per column, names in row 1) and reads one back. `xlsx_patch` edits the embedded workbook of a chart in place as text: only the cells the chart uses change, other cells, sheets, formulas, tables, styles and defined names stay byte-identical, ranges and tables grow with the data, and new series go to the first free column. It falls back to `xlsx` when the ranges are unusual.
- Undo gives the chart parts back through the package journal (`Package::begin_journal`), which records only the chart and workbook parts an op changes, their relationships and the content types.
- `document/charts.rs` holds the ops (`insert_chart`, `set_chart`) and `Document::chart_at`. `ai_charts` has the AI tools and DocScript's chart table (see [AI tools and fidelity](engine-ai-and-fidelity.md)).

### The newer chart types

`chartex` reads the chart types Word 2016 added: waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst and region map. Word writes them as "chartex": an `mc:AlternateContent` drawing whose `mc:Choice Requires="cx1"` points at `word/charts/chartExN.xml` (`cx:chartSpace`, content type `application/vnd.ms-office.chartex+xml`) and whose `mc:Fallback` holds a picture.

- The part becomes a `ChartExModel` that already holds what is drawn: histogram bins (by Scott's rule or the file's bin width, with count and overflow and underflow values), the cumulative line of a Pareto, quartiles by either quartile method with whiskers and outliers, the running totals of a waterfall, and the tree of a treemap or sunburst. It is sent as `chartex` on a `chart` segment next to the usual `img` size and placement.
- It is never written. The part, its workbook, style and colour parts and the fallback picture stay as they are, and a resize changes the extent of the choice and of the fallback together (`chartex::apply_patch`, called from `image::apply`).
- A region map, a part Docxtor cannot read, or one without data shows the fallback picture, or a placeholder with the title when there is none.

## Ready-made page parts

`gallery/` provides ready-made page parts, written as Word writes the ones in its galleries: cover pages (`gallery/cover.rs`) and headers and footers (`gallery/headers.rs`).

## Document tabs

`tabs` holds document tabs, which Google Docs has and Word does not. The body stays one linear body, so Word shows every tab one after the other. Each tab after the first starts a new section (next page), and its first block holds a hidden point bookmark named `_DocxtorTab_<id>`. The list of tabs (id, name, emoji icon, parent, order and the open tab) lives in a custom XML part of Docxtor's own, which Word ignores and keeps; a document without it has one implicit tab. The ops are applied in `document/tabs.rs`. See [Document tabs](index.md#document-tabs).
