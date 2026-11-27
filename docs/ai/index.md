# Using AI with Docxtor

Docxtor gives any model the same interface a coding agent gets for source code: a precise, addressable view of the document and small, verifiable editing tools.

## DocScript: what the model sees

<!-- text -->
```
[p1 Title] Quarterly Product Report
[p2] This report summarises **bold progress**, *italic caveats* and <u>red underlined risks</u>. Visit [the dashboard](https://example.com/report) for live numbers.
[p3 Heading1] Highlights
[p4 ListBullet list:"•"] Revenue grew 18% year over year
[p9 ListNumber list:"1."] Ship the editor
[t15 table 4x3]
  [p16 r1c1] **Metric**
  [p26 r4c2 span2] Merged NPS 41 to 47
[/t15]
[p28] ![image 144x72pt]
[p29] [chart p29@0: Sales by quarter (column, legend bottom)
| | Q1 | Q2 | Q3 | Q4 |
|---|---|---|---|---|
| 2024 | 10 | 12 | 15 | 18 |
| 2025 | 12 | 14 | 19 | 22 |]
# Header (default)
[p40 Header] Docxtor — confidential draft
# Footnotes
[p44 FootnoteText] [^1] Source: Q3 board deck.
```

- Every block has an id (`p` paragraph, `t` table, `c` container, `x` preserved content).
- The style is shown when it isn't Normal; list labels, alignment and table coordinates are explicit.
- Inline formatting is Markdown, relative to the paragraph style (a heading isn't shown as `**bold**`).
- Page and column breaks appear as `<pagebreak>` and `<columnbreak>`. A paragraph that ends a section carries `section_break=<how the next section starts>` (`next_page`, `continuous`, `even_page`, `odd_page`).
- Footnote marks appear as `[^1]`. Headers, footers and footnote texts follow the body with their own ids, and every tool works on them.
- Endnote marks appear as `[^end:i]` (the number as the document formats it: footnotes are 1, 2, 3 and endnotes i, ii, iii unless the document says otherwise), and the endnote texts follow the footnotes under `# Endnotes`. A caption is a paragraph in style `Caption` that reads `Figure 1: Revenue`; the number, cross-references, page references and note numbers are fields whose results read as ordinary text (a cross-reference does not show the hidden bookmark it links to). Index entries (`XE` fields) are hidden and take no space, so they are not in DocScript; the index itself is paragraphs in styles `Index 1`, `Index 2` and `Index Heading`.
- A paragraph that starts bookmarks carries `bookmarks=Intro,Results` (hidden `_Toc…` ones are left out); a link to a bookmark is a Markdown link with `#name`, like `[the intro](#Intro)`.
- A citation appears as `[@Smi20]` (or `[@Smi20; @Lee21]` for several sources): the sources it cites, not the text Word formats for it. The reference list is a container labelled "Bibliography" whose paragraphs are the entries.
- An equation appears as `$x^2$` in the text and `$$\frac{a}{b}$$` when it has a line of its own: LaTeX between dollar signs, for equations Word made too (best effort for markup LaTeX cannot say). It takes one position in the paragraph, like an image. `rewrite_paragraph` and `insert_content` read `$...$` and `$$...$$` back into equations; a paragraph rewritten without its `$...$` loses the equation.
- A chart appears as `[chart p29@0: Title (kind, legend bottom, data labels)]` with its data as a small table (series as rows, categories as columns; a scatter chart lists `(x, y)` points): the paragraph id and offset say where it is, and `set_chart` takes the id. Kinds are column (vertical bars), bar (horizontal), line, pie, doughnut, area, scatter, radar, bubble, stock, surface and combination charts (a stock chart shows as `(ohlc stock, ...)`, a surface chart as `(contour surface, ...)`, a bubble chart lists `(x, y, size n)` points); a chart Docxtor can't read shows as `[chart ...: Title (other) - Docxtor can't read the data of this kind of chart]`. The new chart types (waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst, region map) show as `![Waterfall chart "Cash flow": 6 points]` and are read-only for the assistant: `set_chart` does not apply to them, and Word edits them.
- A horizontal line is an empty paragraph tagged `hr`. A watermark is listed after the headers as `# Watermark: "DRAFT" (diagonal, C0C0C0)`; its shape isn't part of the header's text.

## Tools

The tables are on these pages, by what the tools do:

- [AI tools for text, styles and layout](tools-text.md): Reading and finding text, editing and formatting it, styles, themes, page layout and tables.
- [AI tools for headers, notes, objects and fields](tools-parts.md): Headers and footers, notes, objects and shapes, captions, cross-references, tables of contents, the index, fields, mail merge and bookmarks.
- [AI tools for equations, citations, charts and forms](tools-content.md): Equations, watermarks, citations and bibliographies, charts, content controls, East Asian text, protection and document properties.
- [AI tool for raw ops](tools-raw-ops.md): `apply_ops`: raw engine ops for anything else.

Each tool applies atomically (a failed call changes nothing) and returns the affected paragraphs as DocScript, so the model sees the result of its edit.

## In the app

Open the assistant (the Assistant button, or ⌘/), click the model chip, choose a provider and paste a key. Keys are stored only in your browser and sent only to that provider.

| Provider | Notes |
|---|---|
| Anthropic | Uses the official SDK in browser mode with streaming, adaptive thinking (effort selectable), prompt caching of the tool/guide prefix, and server-side refusal fallback on supported models. |
| OpenAI, OpenRouter, Gemini, Groq, Mistral | OpenAI-compatible chat completions with streaming tool calls. |
| Ollama / LM Studio | Local models. Allow browser origins (`OLLAMA_ORIGINS=*`, or LM Studio's CORS toggle) and pick a model with tool support. |
| Custom | Any OpenAI-compatible base URL. |

After each request the panel shows a word-level diff of every changed paragraph; changed paragraphs get a margin marker. **Keep** accepts, **Revert all** restores the document exactly (and each step is also in the normal undo history).

## MCP server

*Sketch, not run by a test: needs the claude program; the MCP server itself is run by scripts/docs-examples.test.mjs.*
```bash
cargo build --release -p docxtor-mcp
claude mcp add docxtor -- /absolute/path/to/target/release/docxtor-mcp
```

Extra tools on top of the ones above: `open_document` (with `create: true` for new files, and `password` for a password-protected file, which is saved protected again; `DOCXTOR_PASSWORD` is the fallback), `save_document` (`save_as` for copies), `undo`, `list_open_documents`. Every document tool accepts an optional `path`; otherwise it acts on the most recently opened document. Edits are saved after each successful change unless `DOCXTOR_MCP_AUTOSAVE=0`.

## CLI

<!-- example: ai-cli -->
```bash
docxtor read doc.docx                       # DocScript
docxtor tools                               # tool definitions (JSON schema)
docxtor tool doc.docx search '{"query":"revenue"}'
docxtor tool doc.docx insert_content '{"after":"p3","markdown":"- new point"}' -o out.docx
docxtor apply doc.docx ops.json -o out.docx # raw ops
docxtor export doc.docx --format odt -o doc.odt   # html, epub, odt or rtf (html and rtf print to the screen without -o)
docxtor convert old.doc new.docx            # a .docx from an .odt, .rtf, .html, .doc, Word 2003 XML or Flat OPC file, or a repaired copy of a damaged .docx; says what was fixed or left out
docxtor check doc.docx                      # accessibility problems (--json for the list)
docxtor compare old.docx new.docx -o redline.docx --author "Ada"   # new.docx with Word tracked changes against old.docx
docxtor combine original.docx revised.docx -o out.docx --author "Ada"   # revised.docx with what differs marked, its own tracked changes kept
docxtor inspect doc.docx                   # personal information and hidden data (--json for the list)
docxtor inspect doc.docx --remove all -o clean.docx   # take it out: all, or kinds such as properties,comments
docxtor count doc.docx --extras            # paragraphs, words, characters, with notes and text boxes (--json for the list)
docxtor sources doc.docx                    # the document's sources, citation style and how often each is cited (--json, --bibtex)
docxtor cite-import doc.docx refs.bib -o out.docx   # add the sources in a BibTeX or CSL-JSON file; ones already there are skipped
docxtor merge letter.docx people.csv -o letters.docx   # mail merge: a copy of the template for each recipient (csv or xlsx; --type, --sheet, --from, --to)
docxtor encrypt doc.docx locked.docx --password pw  # protect with a password, as Word's "Encrypt with password" does
docxtor decrypt locked.docx doc.docx --password pw  # remove the password
docxtor text locked.docx --password pw      # every command that opens a document takes --password (or DOCXTOR_PASSWORD)
```

A document opened with a password is saved protected again with the same password. `docxtor corpus` skips protected files unless it is given a password.
