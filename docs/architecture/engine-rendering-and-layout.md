# Engine modules: rendering and layout data

The modules of `docxtor-core` that turn the model into what a renderer draws, and the ones that read and write formatting: styles, themes, numbering, character and page formatting, tables, notes, East Asian text, exports and the accessibility check. Part of the [architecture](index.md) pages; [how the engine works](engine-concepts.md) comes first. The other module pages are [files and the model](engine-files-and-model.md), [content features](engine-content-features.md) and [AI tools and fidelity](engine-ai-and-fidelity.md).

## The rendering view model

`render` resolves formatting into a view model for renderers: fonts, sizes, spacing, list labels, borders, images and field results. A table's rows and cells arrive with everything settled:

- the fill, the four borders and two diagonals, margins and text formatting a cell gets from its table style (see `table_style`);
- horizontal merges folded into spans, and rows that start or end short;
- row height rules and floating table placement;
- the name of a content control around a row or cell.

Text and the colours in it pass through two small modules:

- `colour`: the one check a colour from a file passes before the engine hands it out. A colour in a .docx is a string the file chooses (`FF0000;background:url(...)` is as good a `w:color` as `FF0000`). `hex` (six hex digits), `auto_or_hex`, `highlight` (Word's sixteen names and `none`), `theme_name` and `two_hex` (a theme colour and its tint or shade) are called wherever the XML is read, so the render model, the shape, chart, picture and page views, the styles and the exports hold only colours. Nothing here writes: what the file has is saved as it is. `tests/all/hostile_colours.rs` puts a hostile value in every place and holds the engine to it. `editor/color.ts` is the second check on the page's side, and `packages/editor/test/colour-sinks.test.ts` fails a `#${...}` written into CSS or SVG any other way.
- `shading`: `w:shd` with its pattern, colours and theme colours (`themeFill`, tint and shade). `resolve` turns theme references into hex against `theme1.xml`, so renderers see only `val`, `color` and `fill`.

## Character and paragraph formatting

- `charfmt`: character formatting beyond bold and italic, flattened into `RunFmt` (`RunExt`) and `RunPatch` (`RunExtPatch`). It covers underline colour, raised and lowered text, scale, kerning, outline, shadow, emboss, imprint, text borders, patterned and theme shading, theme text colours, emphasis marks, fit text, Word 2010's `w14:` effects (glow, shadow, reflection, outline, fill) and OpenType settings. `resolve_run` turns theme references into hex. Patches write only the elements they name, so effects Docxtor does not model survive. It also holds the language-aware change-case rules.
- `document/charfmt.rs` has the ops (`change_case`, `set_default_format`), character style lookup (a linked paragraph style stands for its character twin, and Word's built-in character styles are added on demand) and the style at a position.
- `document/reveal.rs` says, for a position, which layer each setting comes from: document defaults, paragraph style chain, character style or formatting by hand.

## Styles and themes

- `styles`: `styles.xml` plus theme fonts, inheritance (`basedOn`) resolution, and Word's built-in styles inserted on demand.
- `builtin_styles`: Word's default definition of each built-in style, so a style the file only has as a latent style becomes real as Word writes it.
- `style_edit`: a style's definition as the Modify style dialog edits it, and what the gallery and panels read (`StyleLook`, `StyleDetails`).
- `style_sets`: the style sets (Normal, Title, Subtitle, Heading 1 to 9 and Quote, with the spacing of the document defaults) and the paragraph spacing presets.
- `themes`: Word's colour schemes, font schemes and themes. It writes a theme part as Word writes one and changes the colours or fonts of the part a document has without touching the rest (`Op::SetTheme`). The part is kept for undo as a shared copy, and a file with no theme gets one.
- `document/design.rs` has the ops (`set_style_definition`, `create_style`, `update_style`, `delete_style`, `set_theme`, `set_style_set`, `set_paragraph_spacing`) and the queries (`style_gallery`, `style_details`, `style_uses`, `style_set_previews`).
- Text colours, borders and fills that name a theme colour (`w:themeColor`, `w:themeFill`, with tint and shade) are looked up in the theme when styles are read and when a paragraph, table or cell is drawn. A new theme recolours them while the file keeps the hex Word writes beside them.

## Lists and numbering

- `numbering`: list definitions and label computation. It handles lists that take their levels from a list style, restarts and `lvlRestart`, overrides, label formatting, alignment and picture bullets, and list creation. `numbering/list_def.rs` defines new lists (`define_list`: one `abstractNum` and `num` per list, written as Word writes them, headings linked to levels) and reads a definition back for the dialogs and galleries.
- `numfmt`: every `w:numFmt` Word writes: Latin, enclosed and wide digits, words in English, Spanish, French and German, and Japanese, Chinese, Korean, Hebrew, Arabic, Hindi, Thai, Vietnamese and Russian.
- `list_glyphs` and `symbol_fonts`: Symbol and Wingdings bullet characters, and the Symbol, Wingdings, Wingdings 2, Wingdings 3 and Webdings fonts, mapped to the Unicode characters that look the same. The renderer draws `w:sym` and text in those fonts as `sym` segments (one per character), and list bullets use the same tables.
- `sort`: sorting lines of text by a key (text, number or date; whole line or field; up to three keys). `document/lists.rs` has `sort_paragraphs`, `set_numbering_value` and the list ops. Table sorting uses `sort::order` too.

## Page layout

`layout_props` holds page layout beyond size and margins, as typed views over Word's XML with their patches:

- page borders (`w:pgBorders`), a section's vertical alignment, text direction and chapter page numbers;
- the note options of a section (`NotePr`);
- the document's layout settings (`w:mirrorMargins`, `w:gutterAtTop`, book fold, the hyphenation limits);
- the page background (`PageFill`: a colour, or the VML `v:background` of a gradient, pattern, texture or picture);
- frames (`w:framePr`, `FrameFmt`) and drop caps.

`document/layout_ops.rs` has the ops: `set_section`, `set_page_borders`, `set_note_props`, `set_layout_settings`, `set_page_background`, `set_frame`, `set_drop_cap`, `insert_custom_note`, `insert_page_number` and `remove_page_numbers`. A drop cap is written as Word does it: the first letter in a paragraph of its own with `w:framePr w:dropCap`, an exact line height and a large letter. `SectionInfo` and `PageSetup` carry all of it to the editor.

## Notes

- `notes`: footnote and endnote numbering. It reads the number format and start from `w:footnotePr` and `w:endnotePr` (settings, then the last section) and makes the labels (`1`, `iv`, `C`, `*`).
- `document/layout_ops.rs` numbers notes per section (`note_labels`: each section's own format and start, restarting each section) and reads the notes part's separators and continuation notice (`note_seps`). Restart on each page belongs to the editor, since only the layout knows the pages.

## Tables

- `table`: table structure editing on a grid model: merge and split cells, column widths, cell shading and alignment, border presets and header rows.
- `table_style`: table styles as Word applies them. `TableLook` reads `w:tblLook` (the attribute form and the old hex `w:val`). `TableStyler` is built once per table and answers for each cell in Word's order: the style's whole-table formatting, vertical bands, horizontal bands, first and last column, first and last row, corner cells, then the table's and the cell's own formatting. A table with no `w:tblLook` follows its rows' and cells' `w:cnfStyle`.
- `table_designs`: Word's built-in table designs (Table Grid, Plain Table 1 to 5, Grid Table 1 to 7 and List Table 1 to 7, grey and six accents). It provides the list for the gallery, each design's `w:style` as Word writes it (theme colours with tints and shades, resolved against the document's theme) and the small drawings the gallery shows, made with `TableStyler` so a tile shows what the design does. `Op::TableDesign` (`document/table_design.rs`) adds the style the first time and writes `w:tblStyle` and `w:tblLook`.
- `table_format`: table, row and cell properties as Word's Table Properties dialog sets them, as patches. A table has width, alignment, indent, text wrapping (`w:tblpPr`), autofit, default cell margins, cell spacing and alt text. A row has a height rule, `cantSplit` and a header flag. A cell has width, vertical alignment, margins, wrapping and text direction. Border edges exist for the table (`w:tblBorders`) and for cells (`w:tcBorders`, with the diagonals). It also provides what the dialogs read back (`TableInfo`, `RowInfo`, `CellInfo`).
- `table_edit`: structure beyond rows and columns. It splits a cell into any number of columns and rows (adding grid columns and rows where the cell had too few), splits a table in two, inserts and deletes single cells with the others shifting, sorts rows by columns, and converts text to a table and back (`split_inlines` keeps the runs).
- `document/table_ops.rs` has the ops (`table_format`, `table_rows`, `table_column`, the cell actions `format`, `borders` and `split`, `split_table`, `table_cell`, `sort_table`, `convert_to_table`, `convert_to_text`), `Document::table_details` for the dialogs and `suggest_formula` for the Formula dialog.

## East Asian text

`east_asian` and `ruby` hold East Asian typography as typed views over Word's XML with their patches:

- the paragraph switches of the Asian typography tab (`ParaEa`: `w:kinsoku`, `w:wordWrap`, `w:overflowPunct`, `w:topLinePunct`, `w:autoSpaceDE`, `w:autoSpaceDN`, `w:snapToGrid`, a paragraph's `w:textDirection` and `w:textAlignment`), flattened into `ParaFmt` and `ParaPatch`;
- a run's East Asian layout (`RunEa`: `w:eastAsianLayout` for two lines in one and horizontal in vertical, `w:snapToGrid`, `w:lang w:eastAsia`), flattened into `RunExt`, with the East Asian font `w:eastAsia` in `RunFmt.font_ea`;
- a section's document grid (`DocGrid`, `w:docGrid`, in `SectionExtras` and `SectionPatch`);
- the settings part's line-breaking rules (`AsianSettings`: `w:strictFirstAndLastChars`, `w:noLineBreaksBefore`, `w:noLineBreaksAfter`).

The renderer cuts a run's text by script (`script_pieces` in `render.rs`) and marks East Asian pieces (`eaText`), which the editor draws in the run's East Asian font. A phonetic guide (`w:ruby`, read by `ruby.rs`) is an object in its run, so it is one position in the paragraph; the renderer reports it as a `ruby` segment. `document/east_asian_ops.rs` has the `east_asian` ops (`set_ruby`, `clear_ruby`, `enclose`, `clear_enclose`, `set_settings`). Enclosed characters are `EQ \o\ac(shape,text)` fields. Character and paragraph switches go through `format_text` and `format_paragraphs`, and the grid through `set_section`.

## Exports

`export/` writes downloads in other formats. They all read the rendering view model, so they match what the editor shows, and none changes the document.

- `html`: one self-contained HTML5 page. The same writer makes an EPUB's XHTML.
- `epub`: EPUB 3, with a chapter for each Heading 1, contents and pictures as files.
- `odt`: OpenDocument 1.3, with named styles from Word's styles, lists, tables, notes, the first section's page layout, header and footer.
- `rtf`: font and colour tables, a stylesheet, `\listtable` lists, tables, `\pict`, fields and notes.
- `common`: formatting differences against a style, lists as a tree, pictures, escaping and zip.

`export.rs` itself has Markdown and plain text.

## Counting and accessibility

- `a11y`: the accessibility check. `Document::accessibility()` returns issues with a stable `kind`, a severity, the paragraph to go to and a detail. Contrast uses the WCAG ratio against the paragraph, cell or page colour. The wording of a message depends only on the kind.
- `wordcount`: `Document::text_stats` gives paragraphs, words and characters of the main text, the footnotes and endnotes, and the text boxes, counted apart. Lines depend on the page layout, so the editor counts them from the pages on screen.
