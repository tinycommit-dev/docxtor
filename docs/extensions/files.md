# Extensions for files and review

Opening more file types, PDF files, checking what a file carries, and comparing versions. Part of the [extensions guide](index.md).

## Opening files

An importer is `{ id, label, extensions, detect?(bytes), import(bytes, name) }`. `extensions` (without dots) pick it by file name; `detect` says whether the bytes are its kind of file, which matters for a name that lies (an RTF file called `.docx`) and for an extension several importers share (`.xml` is both Word 2003 XML and a single-file Word package). The host asks `importerFor(name, bytes)`: among the importers for the extension the latest registered one whose `detect` says yes (or that has none) wins, and when none does, any importer that recognises the bytes. `importerForBytes(bytes)` asks without a name.

`import` returns the new document as `{ docx }` or `{ markdown }`. A `{ docx }` made from another kind of file can carry `notice: { title, notes }`: one sentence and the lines behind it, in the person's language. The editor shows it in a strip under the menu bar ("This OpenDocument text file was converted to a Word document") with a Details button, and File, About this file… opens the same lines in a dialog. The person's original file is never changed: opening makes a new .docx.

<!-- example: ext-importer -->
```ts
ctx.importers.register({
  id: "wiki",
  label: t("Wiki page"),
  extensions: ["wiki"],
  detect: (bytes) => new TextDecoder().decode(bytes.slice(0, 5)) === "{{wik",
  import: async (bytes) => ({
    markdown: wikiToMarkdown(new TextDecoder().decode(bytes)),
  }),
});
```

Docxtor's own importers are built in and separate, so any of them can be turned off in the Extensions dialog: `docxtor.open.odt` (OpenDocument text, `.odt`, `.ott` and the single-file `.fodt`), `docxtor.open.rtf`, `docxtor.open.html` (`.html`, `.htm`, `.xhtml`), `docxtor.open.doc` (Word 97 to 2003, `.doc` and `.dot`) and `docxtor.open.xml` (Word 2003 XML and the single-file package Word calls Flat OPC). Each is one call to the engine, `Engine.convert(bytes, name)`, which returns the .docx and an `OpenReport` (`format`, `converted`, `notes` as codes); the readers are in the Rust core (`crates/docxtor-core/src/open/`), which `docxtor convert` runs from the command line. The sentences are in `packages/editor/src/open-report.ts`.

A .docx is not an importer's business: `DocxtorEditor.toDocx` lets a Word file through, and the engine opens it. A damaged one is repaired (a zip whose index is broken or cut off, parts that are cut short, XML that doesn't parse, relationships and content types that are missing), Strict Open XML is read as the standard kind and saved as that, and `engine.openReport` says what was done; the editor turns it into the same strip. A file that isn't a Word document at all (a spreadsheet, a presentation, a PDF, a picture, a zip of other things, nothing) is refused with an `OpenError` whose `code` and `detail` say what it is and whose message is in the person's language.

A host that opens a file in steps (convert it, store it, load it, as the Docxtor app does with its Library) calls `editor.announceOpen(notice)` before `editor.load(engine)`, so the notice from the conversion is the one the person sees; `editor.openNotice` is what the open document was told.

## PDF files

`exporters` takes more than text. Docxtor's own PDF exporter (`packages/editor/src/builtins/pdf.ts`, the `docxtor.pdf` extension) is one: it closes over the editor, so it can write the file from the pages as they are laid out, and ignores the engine it is handed. That is how an extension that needs the screen's layout (an image or print exporter) plugs in:

<!-- example: ext-exporter -->
```ts
ctx.exporters.register({ id: "pdf", label: t("PDF document"), extension: "pdf", mime: "application/pdf", export: async () => exportPdf(editor) });
```

The writer lives in `packages/editor/src/pdf/`: `writer.ts` (objects, streams, cross-reference table), `truetype.ts` (reading and subsetting fonts), `woff.ts`, `woff2.ts` and `brotli.ts` (reading the web font formats), `fontset.ts` (choosing a font for each character and embedding it), `page.ts` (walking the laid-out copy of the pages), `images.ts`, `raster.ts` and `export.ts`. Fonts come from the page's `@font-face` rules, from faces extensions and documents add (a `fonts` contribution with `faces`), and from installed fonts when the browser has let the page list them; so a font an extension offers is embedded in the PDF like any other. Turning the extension off removes PDF from the Download menu.

**Tagged PDF and PDF/A.** By default the file is tagged: `structure.ts` builds the structure tree (Document, headings, paragraphs, lists with their labels and bodies, tables with header cells, figures (with their descriptions when the picture has them), links, footnotes), `tagmap.ts` maps the page's elements to it, and `page.ts` wraps each run of content in a marked-content sequence tied to its element; headers, footers and decoration are artifacts. The catalog gets `/StructTreeRoot`, `/MarkInfo`, `/Lang`, a title display, and links and annotations point into the tree. `exportPdf(editor, { tagged, pdfa })` takes both options; the `docxtor.pdf` extension registers the `pdf` exporter and a `pdfa` exporter (File, Download, PDF/A archive) and two settings, "Tag PDFs for screen readers" and "Make PDF documents PDF/A archive files". PDF/A-2b (`pdfa.ts`) adds an XMP packet, an sRGB output intent with an ICC profile, and needs every font embedded; when the document uses a standard (unembedded) font the file is a normal PDF and the exporter says so (`PdfResult.pdfa`). The `pdfuaid` XMP entry (the PDF/UA-1 claim) is written only when the file backs it (`meetsPdfUa` in `pdfa.ts`): every font held, no heading level skipped, every figure described, every table headed; otherwise the file stays tagged and makes no claim. The structure is checked by tests that read it back (`e2e/pdf-structure.ts`), not by an external validator.

**Charts and shapes as vectors.** An SVG on the page (a chart, a shape) is drawn as paths, gradients (axial and radial shadings) and real text by `svgwalk.ts`, using the geometry in `svgdraw.ts`. When an SVG uses something that cannot be drawn that way (a filter, a clip path, a mask, a blend mode, a marker, a pattern or picture fill, a gradient stop with transparency, text under a stretch), that SVG alone is drawn as a picture. Equations (MathML) remain pictures.

## Inspect document

`ctx.inspectors.register({ id, label, description?, order?, inspect, remove? })` adds a check to Tools, Inspect document…, which lists what the file carries besides what the page shows and offers to remove it. `inspect(ctx)` resolves `{ count, detail? }` (0 means nothing found; `detail` is a few short items, such as names). `remove(ctx)` takes what it found out, by calling `ctx.exec(ops)`: nothing is applied until every inspector the person chose has finished, and then all the edits go in as one undoable step, whatever extensions they come from. Without `remove` the check only reports. Inspectors are listed from the lowest `order` up (Docxtor's own run from 10 to 80, then registration order); one that throws is listed with its error and a count of 0. The host methods are `inspect()` and `removeInspected(ids)`.

Docxtor's own inspectors are the built-in extension `docxtor.inspect`, and each is one category of the engine's `Engine.inspect()`: document properties and personal information (author, last saved by, company, manager, custom properties, the preview picture), the names behind comments and tracked changes, comments, tracked changes, hidden text, headers, footers and watermarks, and custom XML data. Embedded objects and macros are listed but not removed. Removing is one op, `remove_hidden_data` with the categories to take out, so it works from the AI tools (`inspect_document`, `remove_hidden_data`) and the CLI (`docxtor inspect --remove`) too: comments go with their ranges and parts, tracked changes are accepted, names become "Author" and the document gets Word's `w:removePersonalInformation`. The parts that go stay in the package, so undo brings them back; the saved file leaves them out. The list of sources a bibliography keeps in `customXml` is never touched.

## Compare and combine

`ctx.comparers.register({ id, label, compare, redline?, combine? })` is a way of comparing two versions of a document, for Tools, Compare documents… and Combine documents…. The documents are `.docx` bytes. `compare(newer, older)` returns the paragraphs of the newer one against the older one (`CompareBlock[]`: kind `same`, `added`, `removed`, `changed` or `formatted`, with the changed words and the formatting that changed); `redline(newer, older, author)` returns a `.docx` of the newer document with tracked changes; `combine(revised, original, author)` returns the revised document with what differs from the original marked, the tracked changes it already has kept with their own authors. The dialogs use the latest registered comparer that has what they need (`ExtensionHost.comparerFor("compare" | "redline" | "combine")`). Docxtor's own is the built-in extension `docxtor.compare`: it compares word by word, tells paragraph and character formatting changes apart (as `w:pPrChange` and `w:rPrChange` in the redline) and keeps what it doesn't model as it is.
