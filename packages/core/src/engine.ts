// Thin typed wrapper over the WASM document engine.
import { DocxtorCrypt, DocxtorDoc } from "../wasm/docxtor.js";
import { wasmChartMerge, wasmEnvelopeSizes, wasmFragmentText, wasmHtmlImageSources, wasmHtmlToFragment, wasmIsEncrypted, wasmLabelProducts, wasmLatexToMathml, wasmMergeFieldInstr, wasmMergeVocabulary, wasmNewAutoMarkFile, wasmOpsAreViewOnly, wasmPageGallery, wasmParseBibtex, wasmParseCslJson, wasmReadAutoMarkFile, wasmReadRecipients, wasmRtfToFragment, wasmSniffFormat, wasmSpacingPresets, wasmStyleSetInfos, wasmThemeLibrary } from "./engine-wasm.ts";
import { EncryptedError, cryptError } from "./crypt.ts";
import { metered, watched, type PerfMeter } from "./engine-meter.ts";
import { fidelity, save, saveInSteps, savePlain, warmSaves } from "./engine-save.ts";
import { addMedia, dispose, mediaDataUri, mediaUrl, releaseUnusedMedia } from "./engine-media.ts";
export { loadEngine, type WasmSource } from "./engine-wasm.ts";
export * from "./crypt.ts";
export type { PerfMeter } from "./engine-meter.ts";
import { openError, type OpenReport, type SniffedFormat } from "./open-types.ts";
import type { AsianSettings, RRuby } from "./east-asian-types.ts";
import { EngineCrashedError, engineCrash, guardedNew } from "./crash.ts";
import type { FidelityReport, TabsInfo, GalleryDesign, GalleryLabels, ToaOptions, A11yIssue, InspectFinding, TextStats, AutoMarkEntry, BuildingBlock, DocPartControl, IndexOptions, NumberedItem, TocOptions, Fragment, BookmarkInfo, CitationInfo, CitationStyle, CitationStyleInfo, CompareBlock, DocProps, EmbeddedFont, EquationAt, FieldInfo, FieldInput, FontUse, FormatState, MPos, Op, OpResult, PageSetup, ProtectionInfo, RenderOut, RImage, RTableLook, Source, StyleInfo, TableDesign, XrefTargets } from "./types.ts";
import type { TableDetails } from "./table-types.ts";
import type { FindFormat, FindOptions, FindResult, GotoKind, GotoTarget } from "./find-types.ts";
import type { OutlineEntry } from "./outline-types.ts";
import type { ConnectorRoute, RShape, RSite } from "./shapes.ts";
import type { ChartModel, ChartOptions } from "./chart-types.ts";
import type { ListDef, ListDefinition } from "./list-types.ts";
import type { EnvelopeSize, LabelSpec, MergeAnswers, MergeData, MergeFieldSpec, MergeFilterRule, MergeInfo, MergeSortKey, MergePreview, MergePrompt, MergeRange, MergeResult, MergeVocabulary, RecipientList } from "./mail-types.ts";
import type { DesignSnapshot, Reveal, SpacingPreset, StyleDetails, StyleLook, StyleSetDef, StyleSetPreview, StyleUse, ThemeInfo, ThemeLibrary } from "./design-types.ts";
import type { ObjectInfo } from "./objects.ts";

/** JSON the engine hands over in pieces (a render: see `render_json_pieces`), joined. The engine's memory only grows, so it
 * keeps none of a long render's JSON; the pieces are the page's, and go with the next collection. */
function pieces(write: (into: string[]) => void): string {
  const into: string[] = [];
  write(into);
  return into.length === 1 ? into[0] : into.join("");
}

/** A table's options as the `w:tblLook` value. */
function lookBits(look: RTableLook | number): number {
  if (typeof look === "number") return look;
  return (look.firstRow ? 0x20 : 0) | (look.lastRow ? 0x40 : 0) | (look.firstColumn ? 0x80 : 0) | (look.lastColumn ? 0x100 : 0) | (look.bandedRows ? 0 : 0x200) | (look.bandedColumns ? 0 : 0x400);
}

/** What `Engine.saveInfo` hands the next save. */
export interface SaveInfo {
  user?: string;
  pages?: number;
  lines?: number;
}

/** What a closed or crashed engine still answers: closing it, asking whether it is closed, and the idle sweep that may fire after. */
const stillAllowedWhenClosed = new Set<string | symbol>(["dispose", "isDisposed", "releaseUnusedMedia"]);

/**
 * After `dispose()`, every method and property of the Engine that is not plain data throws "This document was closed."
 * A use after close is a bug in the host, and this makes it show up at the call that did it.
 * After the engine crashed (see crash.ts) the same members throw the `EngineCrashedError`, so nothing a crashed engine makes can be saved
 * even by code that never touches `raw`. One flag check per member on the way.
 */
const closedGuard: ProxyHandler<Engine> = {
  get(target, key, receiver) {
    const closed = (target as unknown as { disposed: boolean }).disposed;
    const crashed = engineCrash();
    if ((!closed && !crashed) || stillAllowedWhenClosed.has(key)) return Reflect.get(target, key, receiver);
    const fail = (): never => {
      throw closed ? new Error(CLOSED) : crashed!;
    };
    let proto: object | null = Engine.prototype;
    while (proto) {
      const d = Object.getOwnPropertyDescriptor(proto, key);
      if (d) {
        if (d.get) return fail();
        // An async method of a crashed engine rejects, as it would for any other failure; the rest throw.
        if (typeof d.value === "function") return !closed && d.value.constructor.name === "AsyncFunction" ? () => Promise.reject(crashed) : fail;
        break;
      }
      proto = Object.getPrototypeOf(proto);
    }
    return Reflect.get(target, key, receiver);
  },
};

const CLOSED = "This document was closed.";

export class Engine {
  /** The object URL made for each picture part, and the part's generation (`raw.mediaGen`) it was made for: a part name is reused after undo, so a URL is only good while the generation matches. */
  mediaUrls = new Map<string, { url: string; gen: number }>();
  /** How many object URLs may be alive before the ones for deleted pictures are let go (see `releaseUnusedMedia`). */
  sweepAt = 16;
  sweepQueued = false;
  disposed = false;
  /** Media parts the engine could not convert for display (with the generation that failed), so a failed conversion runs once, not on every render. */
  mediaFailed = new Map<string, number>();
  /** The password and keys of a protected document. Kept in memory only; never part of the document. */
  crypt: DocxtorCrypt | null = null;
  pw: string | null = null;
  opened: Uint8Array | null = null;

  /** The underlying WASM document, for calls this wrapper doesn't cover. */
  raw: DocxtorDoc;
  fileName: string;

  constructor(raw: DocxtorDoc, fileName: string) {
    const g = globalThis as { __DOCXTOR_CHECKS__?: unknown; __DOCXTOR_PERF__?: PerfMeter };
    const checked = g.__DOCXTOR_CHECKS__ === true ? watched(raw) : raw;
    this.raw = g.__DOCXTOR_PERF__ ? metered(checked, g.__DOCXTOR_PERF__) : checked;
    this.fileName = fileName;
    // A closed document says so, on any call, instead of whatever the freed WASM object would throw (see `closedGuard`).
    return new Proxy(this, closedGuard);
  }

  /**
   * The design (theme and style set) that new documents start with: what Design, Set as default keeps. Null starts them
   * the way Word does. The app keeps it between sessions and sets it once the engine is loaded.
   */
  static defaultDesign: DesignSnapshot | null = null;

  static blank(name = "Untitled document.docx"): Engine {
    if (Engine.defaultDesign) {
      try {
        return new Engine(DocxtorDoc.withDesign(JSON.stringify(Engine.defaultDesign)), name);
      } catch {
        /* a design that can't be applied is skipped: the document starts as Word's default */
      }
    }
    return new Engine(guardedNew(() => new DocxtorDoc()), name);
  }

  /**
   * Open a .docx. A password-protected one (Word's "Encrypt with password") needs `password`: without it this throws
   * an `EncryptedError`, with a wrong one a `WrongPasswordError`. A document opened with a password is saved protected
   * again with the same password (see `setPassword`). A password given for a file that has none is ignored.
   */
  static open(bytes: Uint8Array, name: string, password?: string): Engine {
    if (!wasmIsEncrypted(bytes)) return Engine.fromFile(bytes, name);
    if (!password) throw new EncryptedError();
    let crypt: DocxtorCrypt;
    try {
      crypt = DocxtorCrypt.open(bytes, password);
    } catch (e) {
      throw cryptError(e);
    }
    const pkg = crypt.takePackage();
    const eng = Engine.fromFile(pkg, name);
    eng.opened = pkg;
    eng.crypt = crypt;
    eng.pw = password;
    return eng;
  }

  /**
   * What opening the file did: repairs made to a damaged package, Strict Open XML read as the standard kind, or the
   * conversion of another kind of file (`converted`) with what it left out. Null for a new document.
   */
  openReport: OpenReport | null = null;

  /** Open any file the engine reads; failures are `OpenError`s. */
  private static fromFile(bytes: Uint8Array, name: string): Engine {
    try {
      const raw = DocxtorDoc.open(bytes);
      const eng = new Engine(raw, name);
      const report = raw.openReport();
      eng.openReport = report ? (JSON.parse(report) as OpenReport) : null;
      return eng;
    } catch (e) {
      if (e instanceof EngineCrashedError) throw e;
      throw openError(e);
    }
  }

  /** What a file is, from its content (not its name). Needs `loadEngine()` first. */
  static sniff(bytes: Uint8Array): SniffedFormat {
    return wasmSniffFormat(bytes) as SniffedFormat;
  }

  /**
   * Make .docx bytes from any file the engine reads: OpenDocument text, RTF, a web page, an old binary Word file,
   * Word 2003 XML, Flat OPC, or a damaged or Strict .docx. The report says what was repaired or left out.
   */
  static convert(bytes: Uint8Array, name: string): { docx: Uint8Array; report: OpenReport } {
    const eng = Engine.fromFile(bytes, name);
    try {
      return { docx: eng.save(), report: eng.openReport! };
    } finally {
      eng.dispose();
    }
  }

  /** Whether `save()` protects the document with a password. */
  get hasPassword(): boolean {
    return this.crypt !== null;
  }

  /**
   * Protect the document with a password, or take the protection off (`null`). Takes effect on the next `save()`, which
   * writes Word's Agile Encryption (AES-256). The password lives in memory only. The keys are made on the first save
   * after this, which takes a moment (100000 rounds of hashing, as Word does).
   */
  setPassword(password: string | null) {
    this.extraRevision++;
    this.crypt?.free();
    this.crypt = password ? guardedNew(() => new DocxtorCrypt(password)) : null;
    this.pw = password || null;
  }

  /**
   * How many times the document has changed since it was opened: up by one for every edit, undo and redo, and when the
   * password changes (those are written by the next save). Not saved in the file. A document is saved when the revision
   * whose bytes were written is the revision now.
   */
  get revision(): number {
    return Number(this.raw.revision()) + this.extraRevision;
  }

  private extraRevision = 0;

  /** Whether `dispose()` has been called: the document is closed and every other call throws. */
  isDisposed(): boolean {
    return this.disposed;
  }

  /** The document's password, or null. Held in memory only: hosts use it to reopen a copy or an older version without asking again. */
  get password(): string | null {
    return this.pw;
  }

  /** For a document opened from a protected file: the plain .docx package the file held, as it was opened (for fidelity checks). */
  get openedPackage(): Uint8Array | null {
    return this.opened;
  }

  /** Protect .docx bytes with this document's password (as `save()` does); bytes come back as they are when it has none. */
  protectBytes(plain: Uint8Array): Uint8Array {
    return this.crypt ? this.crypt.encrypt(plain, crypto.getRandomValues(new Uint8Array(DocxtorCrypt.randomLen()))) : plain;
  }

  /** Bytes as a plain .docx, using this document's own password for a protected one (an older saved version, say). */
  plainBytes(bytes: Uint8Array): Uint8Array {
    return Engine.plain(bytes, this.pw ?? undefined);
  }

  /**
   * Bytes as a plain .docx: a protected file (a version of this document saved earlier) is decrypted with `password`;
   * anything else comes back as is. Throws an `EncryptedError` without a password and a `WrongPasswordError` with a wrong one.
   */
  static plain(bytes: Uint8Array, password?: string): Uint8Array {
    if (!wasmIsEncrypted(bytes)) return bytes;
    if (!password) throw new EncryptedError();
    try {
      return DocxtorCrypt.open(bytes, password).takePackage();
    } catch (e) {
      throw cryptError(e);
    }
  }

  /** A new document with Word's default styles, holding `markdown` (# headings, lists, tables, > quotes…). */
  static fromMarkdown(markdown: string, name = "Untitled document.docx"): Engine {
    const eng = Engine.blank(name);
    if (markdown.trim()) {
      const first = eng.renderAll().order[0];
      eng.apply([
        { op: "insert_markdown", before: first, markdown },
        { op: "delete_blocks", ids: [first] },
      ] as Op[]);
    }
    return eng;
  }

  /** Every block, with the order (always present here, empty when nothing is visible). */
  renderAll(): RenderOut & { order: number[] } {
    const out = JSON.parse(pieces((p) => this.raw.renderAll(p)));
    out.order ??= [];
    return out;
  }
  /** What changed since the last render. The order is left out when it did not move (`orderKept`): keep the one you have. */
  renderChanged(): RenderOut {
    return JSON.parse(pieces((p) => this.raw.renderChanged(p)));
  }
  pageSetup(): PageSetup {
    return JSON.parse(this.raw.pageSetup());
  }
  /** The bookmarks people can see, in document order (hidden ones such as the table of contents' `_Toc…` are left out). */
  bookmarks(): BookmarkInfo[] {
    return JSON.parse(this.raw.bookmarks());
  }
  /**
   * Every match of `find` for the options, in the text and (unless `main_only`) in headers, footers, notes and text
   * boxes. Throws with a message when the pattern is not valid (an unclosed bracket in a wildcard, say).
   */
  find(find: string, options: FindOptions = {}): FindResult {
    return JSON.parse(this.raw.find(find, JSON.stringify(options)));
  }
  /** The character formatting at a position, as the `find_format` that finds text formatted the same way. */
  findFormatAt(p: number, o: number): FindFormat | null {
    return JSON.parse(this.raw.findFormatAt(p, o));
  }
  /** The places of one kind in the main text, in order, for Go to. */
  gotoTargets(kind: GotoKind): GotoTarget[] {
    return JSON.parse(this.raw.gotoTargets(kind));
  }
  /** Every block of the main text, in order, with its outline level (0 is body text, 1 to 9 are headings). */
  outlineBlocks(): OutlineEntry[] {
    return JSON.parse(this.raw.outlineBlocks());
  }
  /**
   * Everything a cross-reference can point at, in document order: numbered headings, visible bookmarks, captions (a
   * paragraph with a `SEQ` field), footnotes and endnotes. Feeds the "Insert cross-reference" dialog; the ids go into
   * an `insert_cross_reference` op's `target`.
   */
  xrefTargets(): XrefTargets {
    return JSON.parse(this.raw.xrefTargets());
  }
  /** The numbered paragraphs a cross-reference can point at ("Numbered item"), in order, with their numbers. */
  numberedItems(): NumberedItem[] {
    return JSON.parse(this.raw.numberedItems());
  }
  /** The options of the table of contents holding paragraph `at` (the first one without it), or null when there is none. */
  tocOptions(at?: number): TocOptions | null {
    const json = this.raw.tocOptions(at);
    return json ? JSON.parse(json) : null;
  }
  /** The first paragraph of the table of contents holding paragraph `at` (the first table without it), or null. */
  tocAt(at?: number): number | null {
    return this.raw.tocAt(at) ?? null;
  }
  /** The options of the index holding paragraph `at` (the first one without it), or null when there is none. */
  indexOptions(at?: number): IndexOptions | null {
    const json = this.raw.indexOptions(at);
    return json ? JSON.parse(json) : null;
  }
  /** The table of authorities the paragraph `at` is in (else the first one): its options, or null when there is none. */
  toaOptions(at?: number): ToaOptions | null {
    const json = this.raw.toaOptions(at);
    return json ? JSON.parse(json) : null;
  }
  /** The names of the 16 table of authorities categories, the document's own for 8 to 16. */
  toaCategoryNames(): string[] {
    return JSON.parse(this.raw.toaCategoryNames());
  }
  /** The next likely legal citation from a position (a case, a statute, Id.), as the range it covers. */
  nextCitation(from: MPos): [MPos, MPos] | null {
    const json = this.raw.nextCitation(from.p, from.o);
    return json ? JSON.parse(json) : null;
  }
  /**
   * The document's tabs (none for a document without them), where each starts, and `active`: the tab the file was saved
   * on, which a pane shows until it has chosen one. Which tab is open is view state of each pane (the editor), not of the
   * document.
   */
  tabsInfo(): TabsInfo {
    return JSON.parse(this.raw.tabsInfo());
  }
  /** Say which tab the next save writes into the file as the open one. Not an edit: no op, no undo step. */
  setOpenTab(id: string): void {
    this.raw.setOpenTab(id);
  }
  /** The building blocks (quick parts) the document's glossary holds, read without changing the file. */
  buildingBlocks(): BuildingBlock[] {
    return JSON.parse(this.raw.buildingBlocks());
  }
  /** A glossary building block as a fragment, ready for an `insert_fragment` op. */
  buildingBlockFragment(index: number): Fragment | null {
    const json = this.raw.buildingBlockFragment(index);
    return json ? JSON.parse(json) : null;
  }
  /** Docxtor's cover pages, headers or footers, for the galleries. */
  static pageGallery(kind: "cover" | "header" | "footer"): GalleryDesign[] {
    return JSON.parse(wasmPageGallery(kind));
  }
  /** A small .docx of every design of a gallery, one to a page, as it would look in this document (theme, paper, properties). */
  pageGalleryPreview(kind: "cover" | "header" | "footer", labels: GalleryLabels = {}): Uint8Array {
    return this.raw.pageGalleryPreview(kind, JSON.stringify(labels));
  }
  /** Whether the document starts with a cover page. */
  hasCoverPage(): boolean {
    return this.raw.hasCoverPage();
  }
  /** The content controls that came from a gallery (Word's built-in tables of contents, cover pages, bibliographies). */
  docPartControls(): DocPartControl[] {
    return JSON.parse(this.raw.docPartControls());
  }
  /** The rows of an AutoMark file (a .docx with one table of two columns). Throws a plain message when it has none. */
  static readAutoMarkFile(bytes: Uint8Array): AutoMarkEntry[] {
    return JSON.parse(wasmReadAutoMarkFile(bytes));
  }
  /** A new AutoMark file (.docx bytes): a table of two columns with the entries, or two examples. */
  static newAutoMarkFile(entries: AutoMarkEntry[] = []): Uint8Array {
    return wasmNewAutoMarkFile(JSON.stringify(entries));
  }
  /**
   * Accessibility problems (images without alt text, skipped headings, tables without a header row, vague link
   * text, low contrast, typed lists, blank lines, no title or language), in the order they appear in the text,
   * then the ones about the whole document. Fix them with ops (`set_image` alt, `table_props` header,
   * `set_doc_props`, `set_language`) or by editing.
   */
  accessibility(): A11yIssue[] {
    return JSON.parse(this.raw.accessibility());
  }
  /** The document's sources (Word's Manage Sources, the current list), in order. */
  sources(): Source[] {
    return JSON.parse(this.raw.sources());
  }
  /** The citation style the document uses: `apa`, `mla`, `chicago`, `ieee` or `harvard`, or the id of a CSL style. */
  citationStyle(): CitationStyle {
    return this.raw.citationStyle() as CitationStyle;
  }
  /**
   * Add a CSL style (the XML of a CSL 1.0.2 style) under an id, so `set_citation_style` can choose it. The style
   * is not saved in the file: a document remembers the id, and whoever opens it adds the style again. Throws a
   * plain message when the XML is not a usable style or the id belongs to one of Docxtor's own five.
   */
  registerCitationStyle(id: string, csl: string): CitationStyleInfo {
    return JSON.parse(this.raw.registerCitationStyle(id, csl));
  }
  /** The CSL styles added so far. */
  citationStyles(): CitationStyleInfo[] {
    return JSON.parse(this.raw.citationStyles());
  }
  /** Whether the document's citations are footnotes (its style is a CSL note style). */
  citationsAreNotes(): boolean {
    return this.raw.citationsAreNotes();
  }
  /** Where the citations are (body first, then headers, footers and notes). */
  citations(): CitationInfo[] {
    return JSON.parse(this.raw.citations());
  }
  /** The sources as BibTeX text. */
  sourcesBibtex(): string {
    return this.raw.sourcesBibtex();
  }
  /**
   * Whether every op is one a view-only document still applies (the field update that runs as a file opens). The engine
   * decides; false for no ops.
   */
  static opsAreViewOnly(ops: Op | Op[]): boolean {
    return wasmOpsAreViewOnly(JSON.stringify(ops));
  }
  /** Read BibTeX text (Google Scholar, Zotero, ...) into sources without tags; `add_source` makes the tags. Throws a plain message when nothing can be read. */
  static parseBibtex(text: string): Source[] {
    return JSON.parse(wasmParseBibtex(text));
  }
  /** Read CSL-JSON (Zotero, Mendeley, or `doi.org` with `Accept: application/vnd.citationstyles.csl+json`) into sources without tags. */
  static parseCslJson(text: string): Source[] {
    return JSON.parse(wasmParseCslJson(text));
  }
  /**
   * LaTeX as MathML Core, for a live preview while the equation is typed. `display` draws it as a centred block.
   * Throws a plain message that says what is wrong with the LaTeX (and where) when it can't be read.
   */
  static latexToMathml(latex: string, display = false): string {
    return wasmLatexToMathml(latex, display);
  }
  /** Same as {@link Engine.latexToMathml}. */
  latexToMathml(latex: string, display = false): string {
    return wasmLatexToMathml(latex, display);
  }
  /** The equation at a position (the one character `[o, o + 1)`), or null when what is there is not an equation. */
  equationAt(p: MPos): EquationAt | null {
    return JSON.parse(this.raw.equationAt(p.p, p.o));
  }
  /** The phonetic guide at a position (the one character `[o, o + 1)`), or null when what is there is not one. */
  rubyAt(p: MPos): RRuby | null {
    return JSON.parse(this.raw.rubyAt(p.p, p.o));
  }
  /** The document's East Asian line-breaking rules: the strict level and the lists of characters it sets itself. */
  asianSettings(): AsianSettings {
    return JSON.parse(this.raw.asianSettings());
  }
  /** Every field in the document, in order: main text first, then headers, footers and notes. */
  listFields(): FieldInfo[] {
    return JSON.parse(this.raw.listFields());
  }
  /** The result a field instruction would have now, or "" when the engine can't work it out. */
  fieldPreview(instr: string, input: FieldInput = {}): string {
    return this.raw.fieldPreview(instr, JSON.stringify(input));
  }
  /** A field's switches applied to a value the layout worked out (a page number), such as `\* ROMAN` or `\# "000"`. */
  applyFieldSwitches(instr: string, value: string): string {
    return this.raw.applyFieldSwitches(instr, value);
  }
  /** The date and time pictures to offer for the document's language, with the text each gives for `now` (`2026-10-13T09:30:00`). */
  dateFormats(now: string): [picture: string, text: string][] {
    return JSON.parse(this.raw.dateFormats(now));
  }
  /** Evaluate a formula body such as `SUM(ABOVE)`, in the table that holds paragraph `p` when it is given. */
  evaluateFormula(expr: string, p = 0): { value: number } | { error: string } {
    return JSON.parse(this.raw.evaluateFormula(expr, p));
  }
  /** What the table dialogs read about the table holding paragraph `p`: its properties, rows, the caret's cell and the borders drawn. Null when `p` is in no table. */
  tableDetails(p: number): TableDetails | null {
    return JSON.parse(this.raw.tableDetails(p));
  }
  /** The formula Word suggests in the table cell holding paragraph `p`: `=SUM(ABOVE)` or `=SUM(LEFT)`. */
  suggestFormula(p: number): string {
    return this.raw.suggestFormula(p);
  }
  /**
   * Word's built-in table designs (Table Grid, Plain Table, Grid Table and List Table) drawn as five rows of four cells with
   * the given options (a table's `look`, or its `w:tblLook` value; the default is Word's for a new table: header row, first
   * column, banded rows), in the colours of this document's theme. Pick one with a `table_design` op.
   */
  tableDesigns(look: RTableLook | number = 0x04a0): TableDesign[] {
    return JSON.parse(this.raw.tableDesigns(lookBits(look)));
  }
  /**
   * What the document protects (`w:documentProtection`), whether it is marked as final, which styles are locked, where the
   * ranges the user may edit are and, for forms protection, which paragraphs lie in unprotected sections. Edits outside
   * what is allowed are refused by the engine, so this is for showing it.
   */
  protectionInfo(): ProtectionInfo {
    return JSON.parse(this.raw.protectionInfo());
  }
  /**
   * Who may change the document: "edit" (the default) or "view". In "view" the engine refuses every edit (ops, the
   * assistant's tools, undo, redo, revert, added pictures) with "This document is read-only." Only the
   * field update that runs as a file opens still goes through (showing a tab is not an op). The editor sets it from its lock reasons; hosts that embed
   * the engine alone (a server, a script) set it directly.
   */
  setAccess(access: "edit" | "view") {
    this.raw.setAccess(access);
  }
  access(): "edit" | "view" {
    return this.raw.access() as "edit" | "view";
  }
  /** Which of Word's editing groups (`editors`, `owners`...) the user counts as. `everyone` always applies. */
  setUserGroups(groups: string[]) {
    this.raw.setUserGroups(JSON.stringify(groups));
  }
  /**
   * A design an extension offers (one `w:style` of type table as XML) drawn the same way. Throws a plain message when the XML is
   * not a table style. Give it to a `table_design` op as `definition` to use it.
   */
  tableDesignPreview(xml: string, look: RTableLook | number = 0x04a0): TableDesign {
    return JSON.parse(this.raw.tableDesignPreview(xml, lookBits(look)));
  }
  /** The file's properties (title, subject, author, keywords, description); absent ones are left out. */
  docProps(): DocProps {
    return JSON.parse(this.raw.docProps());
  }
  /**
   * Every paragraph of this document against an older version, with word-level changes for the paragraphs that
   * changed. This document is the NEWER one; `older` is the .docx bytes of the older one.
   */
  compareView(older: Uint8Array): CompareBlock[] {
    return JSON.parse(this.raw.compareView(this.plainBytes(older)));
  }
  /**
   * A .docx of this document (the NEWER one) with Word tracked changes showing how it differs from `older` (the
   * .docx bytes of the older version), like Word's Review > Compare. `date` is ISO 8601 and defaults to now.
   */
  redline(older: Uint8Array, author: string, date: string = new Date().toISOString().replace(/\.\d+Z$/, "Z")): Uint8Array {
    return this.raw.redline(this.plainBytes(older), author, date);
  }
  /**
   * A .docx of this document (the REVISED one) with what differs from `original` (the .docx bytes of the original)
   * marked as tracked changes by `author`, like Word's Review > Combine. Tracked changes this document already has
   * keep their authors and dates. `date` is ISO 8601 and defaults to now.
   */
  combine(original: Uint8Array, author: string, date: string = new Date().toISOString().replace(/\.\d+Z$/, "Z")): Uint8Array {
    return this.raw.combine(original, author, date);
  }
  /** What the file carries besides what the page shows (names, comments, tracked changes, hidden text, custom data), by category. Remove it with the `remove_hidden_data` op. */
  inspect(): InspectFinding[] {
    return JSON.parse(this.raw.inspect());
  }
  styles(): StyleInfo[] {
    return JSON.parse(this.raw.styles());
  }
  /**
   * The styles for the gallery and the Styles panel, each with how it looks (fonts and colours resolved): the ones the
   * document defines, then Word's built-in styles it doesn't define yet (`latent`; choosing one adds it).
   */
  styleGallery(): StyleLook[] {
    return JSON.parse(this.raw.styleGallery());
  }
  /** One style's definition, its own formatting and its resolved look, for the Modify style dialog; null for no such style. */
  styleDetails(style: string): StyleDetails | null {
    return JSON.parse(this.raw.styleDetails(style));
  }
  /** Where a style is used in the main text (a paragraph and the part of it), to select every instance. */
  styleUses(style: string): StyleUse[] {
    return JSON.parse(this.raw.styleUses(style));
  }
  /** Which formatting the text at a position has and where each setting comes from (Reveal formatting); null for no such paragraph. */
  revealFormatting(p: MPos): Reveal | null {
    return JSON.parse(this.raw.revealFormatting(p.p, p.o));
  }
  /** The document's theme: its name, colour scheme and font scheme. */
  themeInfo(): ThemeInfo {
    return JSON.parse(this.raw.themeInfo());
  }
  /** Word's built-in style sets drawn in this document's theme: the looks of the title, two headings and the body text. */
  styleSetPreviews(): StyleSetPreview[] {
    return JSON.parse(this.raw.styleSetPreviews());
  }
  /** A style set an extension offers, drawn the same way. Throws a plain message when it can't be read. */
  styleSetPreview(def: StyleSetDef): StyleSetPreview {
    return JSON.parse(this.raw.styleSetPreview(JSON.stringify(def)));
  }
  /** The theme and style set as they are now, to keep as the design of new documents. */
  designSnapshot(): DesignSnapshot {
    return JSON.parse(this.raw.designSnapshot());
  }
  /** Word's built-in themes, colour schemes and font schemes. Needs the engine loaded, not a document. */
  static themeLibrary(): ThemeLibrary {
    return JSON.parse(wasmThemeLibrary());
  }
  /** Docxtor's built-in style sets: id, name and a plain description. Needs the engine loaded, not a document. */
  static styleSetInfos(): { id: string; name: string; description: string }[] {
    return JSON.parse(wasmStyleSetInfos());
  }
  /** Word's paragraph spacing presets (No paragraph space, Compact, Tight, Open, Relaxed, Double). */
  static spacingPresets(): SpacingPreset[] {
    return JSON.parse(wasmSpacingPresets());
  }
  /** The lists the document uses that Docxtor didn't make from a preset: a gallery's "In this document" entries. */
  listDefinitions(): ListDefinition[] {
    return JSON.parse(this.raw.listDefinitions());
  }
  /** The definition of a list (its numId from `FormatState.listNum`), to start the Define new list dialogs from. */
  listDefinition(numId: number): ListDef | null {
    return JSON.parse(this.raw.listDefinition(numId));
  }
  /** Count `n` in a number format (`decimal`, `upperRoman`, `cardinalText`, `japaneseCounting`...), in a language (BCP 47) for formats that spell it out. */
  formatNumber(n: number, format: string, lang?: string): string {
    return this.raw.formatNumber(n, format, lang);
  }
  /**
   * A list of recipients read from a file: a text file with comma, semicolon, tab or bar separated values, or an Excel workbook
   * (.xlsx; the first sheet unless `sheet` names another). The name says which by its extension. Throws a plain message when the
   * file can't be read.
   */
  static readRecipients(bytes: Uint8Array, name: string, sheet = ""): RecipientList {
    return JSON.parse(wasmReadRecipients(bytes, name, sheet));
  }
  /** Docxtor's own label products (a sheet's size and its labels', in twips). Needs the engine loaded, not a document. */
  static labelProducts(): LabelSpec[] {
    return JSON.parse(wasmLabelProducts());
  }
  /** Word's envelope sizes (twips, the long side first). */
  static envelopeSizes(): EnvelopeSize[] {
    return JSON.parse(wasmEnvelopeSizes());
  }
  /** Word's address fields (the names Match Fields maps columns to) and the formats for a name in an address block. */
  static mergeVocabulary(): MergeVocabulary {
    return JSON.parse(wasmMergeVocabulary());
  }
  /** The code a merge field or rule is written with (for a dialog to show and preview). Throws a plain message for a spec that can't be made. */
  static mergeFieldInstr(spec: MergeFieldSpec): string {
    return wasmMergeFieldInstr(JSON.stringify(spec));
  }
  /** The mail merge: the kind of document, the recipients' file, the columns and how many records are in the merge. */
  mergeInfo(): MergeInfo {
    return JSON.parse(this.raw.mergeInfo());
  }
  /** The recipients with their ticks, the filter and the sort; null when there are none. */
  mergeData(): MergeData | null {
    return JSON.parse(this.raw.mergeData());
  }
  /** The records that pass a filter, in the order a sort puts them, whatever the ticks say (indexes into `mergeData().rows`). */
  mergeOrder(filter: MergeFilterRule[], sort: MergeSortKey[]): number[] {
    return JSON.parse(this.raw.mergeOrder(JSON.stringify(filter), JSON.stringify(sort)));
  }
  /** The questions the merge will ask (`ASK` and `FILLIN` fields), in order. */
  mergePrompts(): MergePrompt[] {
    return JSON.parse(this.raw.mergePrompts());
  }
  /**
   * Show the record at a position of the merge's list (from 0) in place of the template, or the template again with `null`. While a record
   * is shown the document can't be edited and saves as the template. Returns what is shown. Re-render the editor afterwards.
   */
  mergePreview(position: number | null, answers: MergeAnswers = {}): MergePreview | null {
    return JSON.parse(this.raw.mergePreview(position ?? -1, JSON.stringify(answers)));
  }
  /** Positions in the merge's list of the records that hold `query`, in the column `field` when given. */
  mergeFind(query: string, field = ""): number[] {
    return JSON.parse(this.raw.mergeFind(query, field));
  }
  /** What an address block, greeting line or merge field code gives for the record at a position in the merge's list. */
  mergeFieldPreview(instr: string, position: number): string {
    return this.raw.mergeFieldPreview(instr, position);
  }
  /**
   * Merge the document with the recipients into a new .docx: a copy for each record, a section for each. The result has no merge fields
   * and no recipients. Throws a plain message when there is nothing to merge.
   */
  mergeToDocx(answers: MergeAnswers = {}, range: MergeRange = {}): { bytes: Uint8Array; result: MergeResult } {
    const out = this.raw.mergeToDocx(JSON.stringify(answers), JSON.stringify(range));
    try {
      return { bytes: out.bytes, result: JSON.parse(out.info) };
    } finally {
      out.free();
    }
  }
  apply(ops: Op | Op[], merge = false): OpResult[] {
    return JSON.parse(this.raw.apply(JSON.stringify(ops), merge));
  }
  /**
   * A chart model with chart options merged in, as the engine would apply them (a look, a palette, axis settings ...).
   * The Edit chart dialog draws its preview from this. Throws a plain message when the options are not valid.
   */
  chartMerge(model: ChartModel, options: ChartOptions): ChartModel {
    return JSON.parse(wasmChartMerge(JSON.stringify(model), JSON.stringify(options)));
  }
  /** The chart at a position (the object occupying `[o, o+1)`); null when there isn't one. */
  chartAt(p: MPos): ChartModel | null {
    return JSON.parse(this.raw.chartAt(p.p, p.o));
  }
  formatAt(p: MPos): FormatState | null {
    return JSON.parse(this.raw.formatAt(p.p, p.o));
  }
  /**
   * The shape or text box whose object sits at a position (the object is one character, `[o, o+1)`), as the
   * renderer draws it, or null when there is none. Its text is a story: use the paragraph ids in `text` with the
   * ordinary text ops.
   */
  /** The picture at a position, with its size, placement and formatting (null when there is none). */
  pictureAt(p: MPos): RImage | null {
    return JSON.parse(this.raw.pictureAt(p.p, p.o));
  }
  /** The theme's colours by Word's names (`dk1`, `lt1`, `dk2`, `lt2`, `accent1` to `accent6`, `hlink`, `folHlink`), as hex without `#`. */
  themeColors(): [string, string][] {
    return JSON.parse(this.raw.themeColors());
  }
  shapeAt(p: MPos): RShape | null {
    return JSON.parse(this.raw.shapeAt(p.p, p.o));
  }
  /**
   * The connection sites of the shape at a position, as Word numbers them: where a connector can be glued, in
   * points from the top-left of the shape's box (not turned or mirrored), and the direction a connector leaves
   * it in (`ang`, degrees: 0 right, 90 down, 180 left, 270 up).
   */
  shapeSites(p: MPos): RSite[] {
    return JSON.parse(this.raw.shapeSites(p.p, p.o));
  }
  /**
   * The connector between two points, as the preset and box Word would write: `x`, `y`, `w`, `h`, `rot` and the
   * mirroring are those of the unturned box, `adj` the adjust values that differ from the default. `ds` and `de` are the
   * directions (unit steps along an axis) the two ends' sites face, `[0, 0]` for an end on no shape.
   */
  connectorRoute(kind: "straight" | "elbow" | "curved", s: [number, number], ds: [number, number], e: [number, number], de: [number, number]): ConnectorRoute {
    return JSON.parse(this.raw.connectorRoute(kind, s[0], s[1], ds[0], ds[1], e[0], e[1], de[0], de[1]));
  }
  /** Every drawing object of the main text, in document order, as the Selection pane lists them. */
  objects(): ObjectInfo[] {
    return JSON.parse(this.raw.objects());
  }
  textRange(a: MPos, b: MPos): string {
    return this.raw.textRange(a.p, a.o, b.p, b.o);
  }
  /**
   * The content between two positions as a fragment: runs, paragraph properties, styles, lists, tables, pictures, links and notes.
   * It is what Docxtor puts on the clipboard (as JSON) and what `insert_fragment` takes. Null for an empty range.
   */
  copyFragment(a: MPos, b: MPos): Fragment | null {
    const json = this.raw.copyFragment(a.p, a.o, b.p, b.o);
    return json ? JSON.parse(json) : null;
  }
  /** HTML from Word, Google Docs or a web page as a fragment. `images` holds pictures already fetched, by the address in the HTML (as `data:` addresses or base64). */
  static htmlToFragment(html: string, images: Record<string, string> = {}): Fragment {
    return JSON.parse(wasmHtmlToFragment(html, JSON.stringify(images)));
  }
  /** The addresses of the pictures in some HTML that are not `data:` addresses. */
  static htmlImageSources(html: string): string[] {
    return JSON.parse(wasmHtmlImageSources(html));
  }
  /** RTF (the clipboard's text/rtf from Word or WordPad) as a fragment. */
  static rtfToFragment(rtf: string): Fragment {
    return JSON.parse(wasmRtfToFragment(rtf));
  }
  /** The text of a fragment: paragraphs on lines, table cells separated by tabs. */
  static fragmentText(fragment: Fragment): string {
    return wasmFragmentText(JSON.stringify(fragment));
  }
  /** The text between two positions as Markdown; null when they're in a header, footer or footnote. */
  rangeMarkdown(a: MPos, b: MPos): string | null {
    return this.raw.rangeMarkdown(a.p, a.o, b.p, b.o) ?? null;
  }
  undo(): boolean {
    return this.raw.undo();
  }
  redo(): boolean {
    return this.raw.redo();
  }
  /** The names of the ops behind each undo step, oldest first (`insert_text`, `set_format`...); the last is what undo takes back. */
  undoLabels(): string[] {
    return JSON.parse(this.raw.undoLabels());
  }
  /** The same for the steps undone, the next one to redo first. */
  redoLabels(): string[] {
    return JSON.parse(this.raw.redoLabels());
  }
  /** Undo `n` steps at once (choosing a point in the undo list). */
  undoSteps(n: number): boolean {
    return this.raw.undoSteps(n);
  }
  redoSteps(n: number): boolean {
    return this.raw.redoSteps(n);
  }
  /** A .docx holding only the text between two positions of the body (styles, headers and page setup kept), for printing a selection. This document is not touched. */
  selectionCopy(a: MPos, b: MPos): Uint8Array {
    return this.raw.selectionCopy(a.p, a.o, b.p, b.o);
  }
  /**
   * What the next `save()` records besides the time: who is saving (`cp:lastModifiedBy`) and how many pages and lines
   * the page laid the document out in (`Pages` and `Lines` in `docProps/app.xml`, with the word and character counts
   * Word keeps there). The editor sets it after laying out; blank or 0 leaves the value out.
   */
  saveInfo: SaveInfo | (() => SaveInfo) = {};
  /**
   * The document as .docx bytes. Parts that were not edited are written back byte for byte. When the document was
   * edited since it was opened or last saved, its "last changed" date (`dcterms:modified`) becomes `now`, and a new
   * document gets its "created" date too, as Word does.
   */
  save(now: Date = new Date()): Uint8Array { return save(this, now); }
  /**
   * `save()` in steps, for a page that is being typed in: the dates and statistics (one call), a pause that lets keys that have
   * arrived run (`pause`), the document's parts (a second call), and the compressing in a worker. No call holds the page
   * longer than a few dozen milliseconds in a book of 1,000 pages, where `save()` holds it for 170. The bytes are the ones
   * `save()` gives; `revision` is the document's at the start, so a key typed during the steps leaves the document unsaved.
   */
  async saveInSteps(pause: () => Promise<void> = () => Promise.resolve(), now: Date = new Date()): Promise<{ revision: number; bytes: Uint8Array }> { return saveInSteps(this, pause, now); }
  /**
   * How much of `original` (the file as it was opened) a save now keeps, part by part: the fidelity report, worked out off the main thread
   * (the page only gathers the parts; in a long book the zip and the comparison take a few hundred milliseconds). Without a password: the
   * comparison is of the plain package.
   */
  async fidelity(original: Uint8Array): Promise<FidelityReport> { return fidelity(this, original); }
  /** How many bytes of the document's parts were to be compressed the last time they were gathered for a save (0: not yet; see `toDeflate`). */
  packedBytes = 0;
  /**
   * Start the worker that compresses saves, so the next save does not wait for it to load: only for a document whose parts are big enough to
   * be zipped there (a small one is zipped on the page, see `ZIP_ON_PAGE_BELOW`), as the last save or fidelity check found them.
   */
  warmSaves(): void { return warmSaves(this); }
  /** The same as `save()` but never protected with a password: the .docx zip itself, for comparing and checking. */
  savePlain(now: Date = new Date()): Uint8Array { return savePlain(this, now); }
  /** The body as GitHub-flavoured Markdown, with footnotes. */
  markdown(): string {
    return this.raw.toMarkdown();
  }
  /** The document as one self-contained web page (HTML5; pictures are inline, styles are in the file). */
  exportHtml(): string {
    return this.raw.exportHtml();
  }
  /** The document as an EPUB 3 book: a chapter for each Heading 1, a contents page, pictures as files. */
  exportEpub(): Uint8Array {
    return this.raw.exportEpub(new Date().toISOString().replace(/\.\d+Z$/, "Z"));
  }
  /** The document as an OpenDocument text file (.odt) that LibreOffice and other editors open. */
  exportOdt(): Uint8Array {
    return this.raw.exportOdt();
  }
  /** The document as Rich Text (RTF, plain ASCII). */
  exportRtf(): string {
    return this.raw.exportRtf();
  }
  /** The font families the document names (its font table, theme and styles), read without a render: to load their faces before the first layout. */
  namedFonts(): string[] {
    return JSON.parse(this.raw.namedFonts());
  }
  /** Fonts embedded in the document (Word's "Embed fonts in the file"), ready for `FontFace`. */
  embeddedFonts(): EmbeddedFont[] {
    const list: { family: string; style: EmbeddedFont["style"] }[] = JSON.parse(this.raw.embeddedFonts());
    return list.flatMap((f, i) => {
      const data = this.raw.embeddedFontData(i);
      return data ? [{ ...f, data }] : [];
    });
  }
  /** Fonts the text uses (body, headers, footers and footnotes) and how many characters use each, most used first. */
  fontsUsed(): FontUse[] {
    return JSON.parse(this.raw.fontsUsed());
  }
  /** Paragraphs, words and characters of the main text (list numbers and bullets aren't counted), with the notes and the text boxes counted apart. */
  counts(): TextStats {
    return JSON.parse(this.raw.textCounts());
  }
  /** The body as plain text. */
  text(): string {
    return this.raw.toText();
  }
  /** Add a picture; throws "This document is read-only." while the access is "view". */
  addMedia(ext: string, bytes: Uint8Array): string { return addMedia(this, ext, bytes); }

  /**
   * Object URL for a media part (cached). A picture linked at a web address is not fetched for you: it comes back
   * empty, since asking the address tells whoever runs it that the document was opened. The editor shows those
   * only after the person says so.
   */
  mediaUrl(path: string): string { return mediaUrl(this, path); }

  /**
   * Let go of the object URLs of pictures the document no longer uses (deleted, or inserted and undone). The engine
   * keeps those pictures so undo can bring them back; the next `mediaUrl` for one makes a new URL. The newest few
   * URLs are kept whatever, since a dialog may be showing a picture that is not in the document yet. Runs by itself
   * when the URLs pile up (see `sweepWhenIdle`), never on every edit, since it looks through the whole document.
   */
  releaseUnusedMedia(): void { return releaseUnusedMedia(this); }


  /** A `data:` address for a picture shown from an object URL made by `mediaUrl` (for HTML that leaves the page); null for any other address. */
  mediaDataUri(url: string): string | null { return mediaDataUri(this, url); }

  dispose() { return dispose(this); }
}
