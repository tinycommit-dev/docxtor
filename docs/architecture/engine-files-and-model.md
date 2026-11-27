# Engine modules: files and the model

The modules of `docxtor-core` that read and write files, hold the editable tree and apply edits: packages, encryption, the model, ops, opening and importing other formats, and the checks that guard them. Part of the [architecture](index.md) pages; [how the engine works](engine-concepts.md) comes first. The other module pages are [rendering and layout](engine-rendering-and-layout.md), [content features](engine-content-features.md) and [AI tools and fidelity](engine-ai-and-fidelity.md). `lib` is the crate root that exports them.

## Files and packages

- `xml`: a small lossless DOM over quick-xml. It keeps element and attribute names verbatim, comments, whitespace and processing instructions.
- `package`: the OPC zip container. Parts are kept as raw bytes in their original order; relationships and content types are edited surgically. The package journal (`Package::begin_journal`) records the parts an op changes so undo can restore them.
- `cfb`: the Compound File Binary container (MS-CFB, the OLE format). The reader follows FAT and DIFAT chains, the mini stream and streams by path. The writer produces version 3 files with a balanced directory tree and small streams in the mini stream.
- `template`: Word templates (`.dotx`, and `.dotm` with macros). A template is a `.docx` package whose main part has another content type, so these functions switch between the two kinds without touching any other part. Styles, headers, page setup, building blocks and macros come across as they are.
- `safe_write`: writes a file without leaving a broken one behind: a temporary file beside it, read back and opened again, then renamed over the target. The CLI and the MCP server write through it.
- `fonts`: fonts embedded in a .docx (`word/fonts/*.odttf`, listed in the font table, with the first 32 bytes XOR-ed with a key taken from a GUID). Reading them lets a document show in its own fonts; the package is never changed.

## Password protection

`office_crypt` handles password-protected Office files (MS-OFFCRYPTO).

- `is_encrypted`, `open` and `decrypt` read Agile Encryption (AES-128, 192 or 256 in CBC; SHA-1, SHA-256, SHA-384 or SHA-512; the spun password hash, the verifier, the HMAC integrity check, 4096-byte segments) and Standard Encryption (Word 2007: AES-ECB with SHA-1).
- `CryptSession` writes Agile Encryption as current Word does (AES-256, SHA-512, 100000 rounds, fresh salts). It keeps the derived keys while a document is open, so each save encrypts only the new content and writes a new integrity check.
- The random source is passed in: the page's `crypto.getRandomValues`, or `os_random` natively.
- It sits beside the document, not inside it. A host decrypts the package, opens the `Document` from it, and encrypts what `save` returns.
- AES and the SHA family come from the RustCrypto crates `aes`, `sha1`, `sha2` and `hmac`.

## The model

- `model`: the editable tree, with `Block` (Para, Table, Container, Opaque) and `Inline` (Run, Container, Opaque). Property blocks stay as XML elements. Unknown constructs are `Opaque` and keep their position. Each block has a session id (`NodeId`).
- `body`: the document's top-level blocks, each one shared copy-on-write (`Arc<Block>`) with the undo steps that hold it. Reading works like a `Vec<Block>`; writing copies only the block it changes, and only when an undo step still shares it.
- A content control around table rows or cells (`w:sdt` holding `w:tr` or `w:tc`) is not a node of its own. The rows and cells stay as usual and each carries the controls it sits in (`Row::sdts`, `Cell::sdts`), so the table is edited like any other and neighbours with the same control are written back inside one control.
- `props`: typed views (`RunFmt`, `ParaFmt`) over `w:rPr` and `w:pPr` for reading, and patches (`RunPatch`, `ParaPatch`) that edit the XML in schema order.
- `edit`: text primitives over inline content at UTF-16 offsets: split, insert, delete, format, partition.
- `stories`: headers, footers, footnotes and endnotes as separately stored block lists in their own parts, addressed by the same paragraph ids as the body. `document/header_link.rs` holds "Link to previous": a section without its own header or footer shows the one before, unlinking copies the part, and header parts nothing refers to are dropped on save.

## Ops and the document

- `ops`: the serializable op language (`insert_text`, `delete`, `split_paragraph`, `format_text`, `toggle_list`, `table_edit`, `insert_markdown`, `replace_text` and the rest).
- `document`: `Document` itself, with open and save, op application in atomic batches, undo and redo snapshots, change tracking and page setup. Its child modules (`document/references`, `document/updating`, `document/field_ops.rs` and others) reach its private state.
- `invariants`: rules that must hold after any edit, written so code can check them. `Document::check_invariants` walks the model (ids, tabs, references, content controls) and `check_package` walks saved bytes (XML parts, content types, relationships). Real Word files break some rules already, so only violations an edit adds count. `invariants_known` lists the known violations so `cargo test` stays green while the list says what is broken.
- `perf`: counters and test switches for the performance harness (see [Performance](../performance/index.md)).
- `budget`: one input limit that every reader spends from, so no file from outside can crash, freeze or use up the engine. A `Budget` is made where a file comes in (`open::open`, `Document::open`, a paste, a picture) and handed to every reader, nested ones included. See [the input budget](engine-concepts.md#the-input-budget).

## Document properties

- `docprops`: properties in `docProps/core.xml` (`Op::SetDocProps`). `Document::save_stamped` sets `dcterms:modified` when the document changed and `dcterms:created` on a new one, with the time the caller passes in. It also holds the document language (`Op::SetLanguage`: `w:lang` in the style defaults, or on the runs of some paragraphs).
- `docmeta`: the properties beyond the core ones: the extended ones in `docProps/app.xml` (company, manager, the statistics Word keeps there) and the custom ones in `docProps/custom.xml`.
- `document/inspect`: `Document::inspect` lists what a file carries besides the page (personal properties, names behind comments and tracked changes, comments, tracked changes, hidden text, header and footer text, custom XML, embedded objects). `Op::RemoveHiddenData` takes categories out. Removed parts stay in the package (`Scrub`, part of the undo snapshot) and are left out when saving.
- `document/media_prune`: drops unused pictures, charts and workbooks on save. A deleted picture or chart stays in the package so undo can bring it back; `save` leaves out the parts in `word/media`, `word/charts`, `word/embeddings` (and a glossary's `media`) that nothing uses, with their relationships and content-type overrides. `Document::unused_media` lists the pictures so the page can revoke their object URLs.
  - "In use" means a relationship from another part whose id appears in that part's text, a relationship of an unknown type, a part whose text cannot be read, the parts of a chart in use, or a file name that appears in another part's text.
  - Only parts that were in use when the file opened, or that Docxtor added, can go. Parts the file already did not use (`baseline_orphans`) are kept, and an unedited file is saved byte for byte. `add_media` stores identical bytes once.

## Moving content: fragments and paste

- `fragment`: content that moves between places. A `Fragment` is Word's `w:p` and `w:tbl` elements as XML text plus the style definitions, list definitions, pictures, link targets and notes they need (named by keys, not relationship ids). `Document::copy_fragment` makes one from a range (the Docxtor clipboard format) and `Op::InsertFragment` brings one in: styles map by id and missing ones are added, lists are imported or continued, relationships and drawing ids are made new, notes are created. `PasteMode::Merge` keeps only bold, italic, underline, lists and links. `Builder` is what readers use to make fragments, and `document/paste.rs` holds the copy and the insert.
- `html_import`: HTML to a fragment. It understands Word's `mso-` styles and `mso-list` lists, Google Docs, web pages and Docxtor's own HTML, with its own tolerant parser, inline styles, `<style>` rules and `@list`. It also backs `Op::PasteHtml` and the `insert_html` AI tool.
- `rtf`: a full RTF reader (fonts, colours, stylesheet, lists, tables, pictures, fields, notes) that produces a fragment, used for pasting RTF and opening .rtf files.
- `markdown`: the Markdown subset used for AI edits and paste.
- `alt_chunk`: imported content chunks (`w:altChunk`): the HTML, text, RTF, web archive or .docx that report generators and mail systems put in a file. They are shown as content and kept as they are until an edit inside one turns it into ordinary blocks (`expand_alt_chunk`).

## Opening files

`open/` opens any file. `open::open(bytes, name)` returns a `Document` and a `Report`, or an `OpenError` with a code (`empty`, `not-word` with what it is, `damaged`, `no-text`, `doc-encrypted`, `unknown`). The report has the `format`, whether it was `converted`, and `notes` made of codes the page words in the person's language. `sniff` tells the kind from the bytes, never the name. `docxtor convert` runs it from the command line.

A Word file goes through these steps:

- `zipscan`: a zip whose central directory is broken or missing is read from its local headers. An entry that is cut short or fails its checksum keeps what could be read.
- `repair`: `xmlfix` recovers what parses from malformed XML and drops what does not; missing relationships and content types are put back; relationships to missing parts are dropped. A sound package comes out byte for byte as it was read. `Document::open` goes through the repair, so every host repairs.
- `strict`: maps Strict Open XML (`purl.oclc.org` namespaces, `start` and `end` for left and right) to the standard kind.

Other formats become a `Fragment` and are pasted into a blank document (`text`):

- `odt`: OpenDocument text and flat .fodt, with styles resolved into formatting, lists, tables with spans, notes, pictures, page setup and properties.
- RTF and HTML, through the readers the clipboard uses, plus page size and properties.
- `word2003`: Word 2003 XML. `flat_opc`: a whole package in one XML file.
- `doc_binary`: the Word 97 to 2003 file, with the piece table, formatting pages, style sheet, lists, tables, fields, notes and JPEG and PNG pictures. `doc_story.rs` is its text.
