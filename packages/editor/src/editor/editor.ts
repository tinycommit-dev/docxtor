// The editing surface: renders the engine's view model, maps browser input to
// engine ops, and keeps DOM selection and model selection in sync.
import { touchEditing } from "./touch";
import { t, type CommandResult } from "@docxtor/extensions";
import { Engine } from "@docxtor/core";
import type { RRuby, CitationInfo, FormatState, MPos, Op, PageSetup, RBlock, RunPatch } from "@docxtor/core";
import { type AutoformatSettings } from "./autoformat";
import { Paginator, type StoryZone } from "./layout";
import { FoldMarks } from "./fold-marks";
import { farKeepFor } from "./far";
import { Folds } from "./folds";
import { PageGrid } from "./page-grid";
import { PageGuides } from "./guides";
import { Zoomer } from "./zoom";
import { BookmarkMarks } from "./bookmarks";
import { FieldCodes } from "./field-codes";
import { type FieldServices } from "./fields";
import { type AutoCaptions } from "./lists";
import { NonPrinting } from "./marks";
import { ExtraSelection } from "./multi-select";
import { SpellLayer } from "./spell-layer";
import { Ruler } from "./ruler";
import { VRuler } from "./vruler";
import { DragDrop } from "./drag-drop";
import { Painter } from "./format-painter";
import { PasteFlow } from "./paste-flow";
import { type FloatHost } from "./float-place";
import { type RenderEnv } from "./render";
import { openTabs } from "../ui/tabs-dialog";
import { paraIn, paraLength, paraOf, readSelection, writeSelection, type ModelSel } from "./selection";
import { Lifetime } from "../life";
import { LockSet, type LockScope } from "./locks";
import { useEmbeddedFonts } from "../fonts";
import { renderMethods } from "./editor-render";
import { viewMethods } from "./editor-view";
import { selectionMethods } from "./editor-selection";
import { formatMethods } from "./editor-format";
import { referenceMethods } from "./editor-references";
import { insertMethods } from "./editor-insert";
import { fieldMethods } from "./editor-fields";
import { storyMethods } from "./editor-stories";
import { inputMethods } from "./editor-input";

/** How a continuous layout (Pageless, Reading, Outline) is sized: see `Editor.setContinuous`. */
export interface ContinuousSpec {
  /** `page` keeps the first section's text width; `window` fits the window, up to `maxWidth`. */
  width?: "page" | "window";
  /** The widest text column of a `window` layout, in px at 100%. */
  maxWidth?: number;
  /** Side margins in px (the first section's when left out) and top and bottom margins. */
  margin?: number;
  marginLeft?: number;
  marginTop?: number;
}

export type ListKind = "bullet" | "decimal";

export interface StoryEdit {
  zone: StoryZone;
  page: number;
  /** Header/footer variant (`default`, `first`, `even`). */
  variant?: string;
  /** Footnotes or endnotes shown in the notes area being edited. */
  noteIds?: string[];
}

export interface EditorHooks {
  onStory?: (story: StoryEdit | null) => void;
  onChange?: () => void;
  onSelection?: (fmt: FormatState | null) => void;
  onLayout?: (pages: number) => void;
  onError?: (msg: string) => void;
  /** The fonts the document uses may have changed (undo, redo, paste, replacing a font, or edits made outside the editor: the assistant, extensions). */
  onFontsMayChange?: () => void;
  /** This pane took or let go of a reason to be read-only (see `Editor.lock`). */
  onReadOnly?: () => void;
  /** A bookmark's marker was clicked (`marker` is the element to anchor a popover to). */
  onBookmark?: (name: string, marker: HTMLElement) => void;
  /** A citation in the text was clicked (it is selected as one unit; `el` is the element to anchor a popover to). */
  onCitation?: (citation: CitationInfo, el: HTMLElement) => void;
  /** An equation was double-clicked, or Enter was pressed with one selected (`el` is the element to anchor the editor to). */
  onEquation?: (equation: EquationAtCaret, el: HTMLElement) => void;
  /** A phonetic guide was double-clicked, or Enter was pressed with one selected (`el` is the element to anchor the box to). */
  onRuby?: (ruby: RubyAtCaret, el: HTMLElement) => void;
  /**
   * A tool takes over typing or deleting (a blank in a form, say): `from` and `to` are the selection. Return true when it
   * did, and the editor does nothing more with the input.
   */
  beforeEdit?: (e: InputEvent, from: MPos, to: MPos) => boolean;
  /** A tool takes over a key (Space on a check box). Return true when it did. */
  beforeKey?: (e: KeyboardEvent) => boolean;
  /**
   * Ask before a risky action by its caution id (see `ExtensionHost.ask` and builtins/cautions.ts): true to go ahead.
   * Use `editor.ask`, which goes ahead when no host is attached.
   */
  ask?: (id: string, ...args: unknown[]) => Promise<boolean>;
  /** Run a command by id through the host (the only door to a command: it checks, asks, runs). */
  command?: (id: string, ...args: unknown[]) => Promise<CommandResult>;
  /** Offered every address the document opens, before the editor does: true when the host opened it (see `openAddress`). */
  openAddress?: (url: string) => boolean;
}

/** A phonetic guide found in the text: where it is and what it says. */
export interface RubyAtCaret {
  at: MPos;
  ruby: RRuby;
}

/** An equation found in the text: where it is, its LaTeX and whether it has a line of its own. */
export interface EquationAtCaret {
  at: MPos;
  latex: string;
  display: boolean;
}

export class Editor {
  readonly canvas: HTMLElement;
  readonly flow: HTMLElement;
  /** The element taking input: the flow, or the header/footer/notes area being edited. */
  root: HTMLElement;
  /** The header, footer or footnotes being edited, if any. */
  story: StoryEdit | null = null;
  /** Headers, footers and notes can't be opened for editing here (the second pane of a split window leaves them to the first). */
  noStories = false;
  storyEl: HTMLElement | null = null;
  bodySel: ModelSel | null = null;
  zoomEl: HTMLElement;
  /** What the zoom scales: the pages are laid out at 100% inside it and drawn at the zoom with a transform. `fitEl` takes the room the scaled picture takes. */
  viewEl: HTMLElement;
  fitEl: HTMLElement;
  /** Holds the ruler, which stays at the top of the window and so cannot be inside the scaled picture (a sticky box does not follow the scroll through a transform). */
  rulerHost: HTMLElement;
  /** True once a zoom has been set (the first one comes before the first layout). */
  zoomSet = false;
  stage: HTMLElement;
  sheets: HTMLElement;
  paginator!: Paginator;
  readonly ruler: Ruler;
  /** Non-printing characters (¶, spaces, tabs, breaks), drawn over the pages (see marks.ts). */
  readonly marks: NonPrinting;
  /** Wavy underlines under misspelled words, drawn over the pages (see spell-layer.ts); idle until a checker is set. */
  readonly spell: SpellLayer;
  /** The other selected ranges, next to the real selection (see multi-select.ts). Empty most of the time. */
  readonly extra: ExtraSelection;
  /** The vertical ruler beside the page that holds the caret (see vruler.ts). */
  readonly vruler: VRuler;
  /** The ribbons in the margin where visible bookmarks start (see bookmarks.ts). */
  readonly bookmarkMarks: BookmarkMarks;
  /** Gridlines and text boundaries, drawn between the sheets and the text (see guides.ts). */
  readonly guides: PageGuides;
  /** The other panes of a split window: editors of the same document, which draw every change this one makes (see split.ts). */
  readonly peers = new Set<Editor>();
  /** Field codes shown in place of results (View, Field codes). */
  readonly fieldCodes: FieldCodes = new FieldCodes({ setCode: (p, o, instr) => this.setFieldCode({ p, o }, instr) });
  /** What the host provides for fields: the user, extensions' evaluators, the update-before-printing setting. */
  fieldServices: FieldServices | null = null;
  /** The page setup as of the last load or reload (the engine builds it again on every call). */
  setup: PageSetup | null = null;
  /** A section break was added, moved or removed since the page setup was read. */
  sectionsDirty = false;
  engine!: Engine;
  env!: RenderEnv;
  blocks = new Map<number, HTMLElement>();
  json = new Map<number, RBlock>();
  order: number[] = [];
  /** The block elements drawn since a tool last asked (`takeDrawn`), or null: everything is new to it (nothing asked yet, a full draw, or too many to list). */
  drawn: HTMLElement[] | null = null;
  /** Where each block is in `order`. */
  orderIdx = new Map<number, number>();
  sel: ModelSel | null = null;
  pending: RunPatch | null = null;
  composing: ModelSel | null = null;
  lastTyping = 0;
  lastDelete = 0;
  /** Where the caret was left by the last key that typed or deleted (null after any other edit): only a key at that place joins its undo step. */
  typedEnd: MPos | null = null;
  /** The typing substitutions in force (smart quotes, Markdown shortcuts…); null leaves typing as typed. */
  autoformat: AutoformatSettings | null = null;
  /** The last few characters typed in a row, and where: the letter after "--" is what turns it into a dash. */
  recent = { p: -1, o: -1, s: "" };
  /** Set right after a substitution, until anything else happens: Backspace and undo take it back and restore the caret. */
  lastAuto: { caret: MPos; restore: MPos } | null = null;
  plainPaste = false;
  layoutTimer = 0;
  zoom = 1;
  /** Which zoom is in force (a fixed factor, the page width, a whole page, several pages) and how the wheel and pinches change it (see zoom.ts). */
  readonly zoomer = new Zoomer(this);
  /** Several pages side by side, a read-only picture of the pages (see page-grid.ts). */
  readonly grid: PageGrid;
  /** Headings folded away, and the Outline view's levels (see folds.ts). */
  readonly folds: Folds;
  /** The arrows that fold headings, in the margin (see fold-marks.ts). */
  readonly foldMarks: FoldMarks;
  /** The unbroken layout in force (null for pages), as asked for; its sizes are worked out from the window. */
  contSpec: ContinuousSpec | null = null;
  /** A key handler that goes before the editor's own (the Outline view takes Tab and Shift+Tab). It returns true after handling the key. */
  keyHook: ((e: KeyboardEvent) => boolean) | null = null;
  hooks: EditorHooks = {};

  /** Ask before a risky action (a caution in builtins/cautions.ts); goes ahead when nothing is attached to ask with. */
  ask(id: string, ...args: unknown[]): Promise<boolean> {
    return this.hooks.ask ? this.hooks.ask(id, ...args) : Promise.resolve(true);
  }
  /**
   * Run a command by id the way a menu does: a button that does what a command does calls this, not `exec`, so the
   * command's check, its question and its Undo notice apply. Answers `unavailable` when nothing is attached to run it.
   */
  command(id: string, ...args: unknown[]): Promise<CommandResult> {
    return this.hooks.command ? this.hooks.command(id, ...args) : Promise.resolve({ status: "unavailable" });
  }
  /** Captions added when a table or a picture goes in (Insert, Caption, AutoCaption); the host sets them. */
  autoCaptions: AutoCaptions = {};
  /** Called when the person changes the AutoCaption settings, so the host can keep them. */
  autoCaptionsSave: ((a: AutoCaptions) => void) | null = null;
  /** Pasting: the converters, the default way to paste and the Paste options button (see paste-flow.ts). */
  readonly pasteFlow = new PasteFlow(this);
  /** Dragging selected text to move or copy it (see drag-drop.ts). */
  readonly dnd = new DragDrop(this);
  /** Copy formatting and the format painter (see format-painter.ts). */
  readonly painter = new Painter(this);
  /** Everything this editor registers globally; released by destroy(). */
  readonly life = new Lifetime();
  /** The browser's own spellcheck (red underlines and its right-click suggestions); off while Docxtor's checker is on. */
  nativeSpell = true;
  /** Paragraph ids to highlight as recently changed by AI. */
  aiMarks = new Set<number>();

  constructor(host: HTMLElement) {
    this.canvas = document.createElement("div");
    this.canvas.className = "canvas";
    this.zoomEl = document.createElement("div");
    this.zoomEl.className = "zoom";
    this.fitEl = document.createElement("div");
    this.fitEl.className = "zoom-fit";
    this.viewEl = document.createElement("div");
    this.viewEl.className = "zoom-view";
    this.rulerHost = document.createElement("div");
    this.rulerHost.className = "ruler-host";
    this.stage = document.createElement("div");
    this.stage.className = "stage";
    this.sheets = document.createElement("div");
    this.sheets.className = "sheets";
    this.flow = document.createElement("div");
    this.flow.className = "flow";
    this.flow.contentEditable = "true";
    this.flow.spellcheck = this.nativeSpell;
    this.flow.setAttribute("role", "textbox");
    this.flow.setAttribute("aria-multiline", "true");
    this.flow.setAttribute("aria-label", t("Document"));
    this.root = this.flow;
    this.ruler = new Ruler(
      (patch) => this.setIndents(patch),
      () => this.zoom,
      { tabs: (stops) => this.setPara({ tabs: stops }), openTabs: () => openTabs(this) },
    );
    this.vruler = new VRuler({
      zoom: () => this.zoom,
      page: (k) => this.paginator?.page(k) ?? { top: 0, left: 0, w: 0, h: 0 },
      onMargins: (page, top, bottom) => this.setMargins(page, top, bottom),
    });
    this.stage.append(this.sheets, this.flow, this.vruler.el);
    this.guides = new PageGuides({ canvas: this.canvas, stage: this.stage, before: this.flow, pages: () => this.pages, box: (k) => this.paginator.page(k), setup: () => this.setup });
    this.folds = new Folds({ flow: this.flow });
    this.grid = new PageGrid(this);
    this.foldMarks = new FoldMarks({ stage: this.stage, flow: this.flow, folds: this.folds, active: () => !this.story && !this.grid.on, toggle: (id) => this.toggleFold(id) });
    this.marks = new NonPrinting({
      canvas: this.canvas,
      stage: this.stage,
      flow: this.flow,
      sheets: this.sheets,
      blocksBetween: (y0, y1) => this.paginator?.blocksBetween(y0, y1),
      pageRight: (y) => {
        const b = this.paginator?.page(this.paginator.pageAt(y));
        return b ? b.left + b.w : this.stage.offsetWidth;
      },
    });
    this.spell = new SpellLayer({
      canvas: this.canvas,
      stage: this.stage,
      flow: this.flow,
      sheets: this.sheets,
      blocksBetween: (y0, y1) => this.paginator?.blocksBetween(y0, y1),
      // The live selection when the text has focus (the model's copy can lag a moment behind it), without touching the model's.
      caret: () => {
        const s = (this.hasFocus() ? readSelection(this.root) : null) ?? this.sel;
        return s && s.anchor.p === s.focus.p && s.anchor.o === s.focus.o ? s.focus : null;
      },
      busy: () => !!this.composing,
    });
    this.extra = new ExtraSelection({ stage: this.stage, root: () => this.root });
    this.bookmarkMarks = new BookmarkMarks({
      stage: this.stage,
      flow: this.flow,
      sheets: this.sheets,
      bookmarks: () => (this.engine ? this.engine.bookmarks() : []),
      active: () => !this.story,
      open: (name, marker) => this.hooks.onBookmark?.(name, marker),
    });
    this.rulerHost.append(this.ruler.el);
    this.ruler.onSize = () => this.sizeRuler();
    this.viewEl.append(this.stage);
    this.fitEl.append(this.viewEl);
    this.zoomEl.append(this.rulerHost, this.fitEl);
    // The room the picture takes follows the pages' size at 100% (a transform does not change it, so this sees only the layout's changes).
    if (typeof ResizeObserver !== "undefined") {
      const watch = new ResizeObserver(() => {
        this.sizeZoom();
        this.sizeRuler();
      });
      watch.observe(this.viewEl);
      watch.observe(this.ruler.el);
      this.life.defer(() => watch.disconnect());
    }
    this.canvas.append(this.zoomEl, this.grid.el);
    host.append(this.canvas);
    this.bind();
    // The vertical ruler follows the caret's page, or the first page in view; scrolling only needs a look once a frame.
    let frame = 0;
    this.life.on(
      this.canvas,
      "scroll",
      () => {
        this.marks.scrolled();
        this.spell.scrolled();
        this.followDrawn();
        this.farSoon();
        if (frame || !this.rulerShown) return;
        frame = requestAnimationFrame(() => {
          frame = 0;
          this.updateVRuler();
        });
      },
      { passive: true },
    );
    // Web fonts change line breaks when they arrive: lay the pages out again, unless the last whole layout already had every face that came
    // (the faces the text uses are asked for as its runs are drawn, see `wantFace` in render.ts, and usually arrive before that layout).
    this.life.on(document.fonts, "loadingdone", (e: Event) => {
      if (!this.engine) return;
      const faces = (e as Event & { fontfaces?: readonly FontFace[] }).fontfaces;
      if (faces && faces.length && faces.every((f) => this.facesAtFull.has(f))) return;
      this.scheduleLayout();
    });
    // Tablets: the caret above the on-screen keyboard, and a long press that opens the menu.
    touchEditing(this, this.life.signal);
  }

  // ---------------------------------------------------------------------------
  // Loading & rendering
  // ---------------------------------------------------------------------------

  private releaseFonts: (() => void) | null = null;

  load(engine: Engine) {
    if (this.story) this.exitStory(false);
    this.engine = engine;
    this.lastAuto = null;
    this.extra.clear();
    // A document can carry its own fonts (Word's "Embed fonts in the file").
    this.releaseFonts?.();
    try {
      this.releaseFonts = useEmbeddedFonts(engine.embeddedFonts());
    } catch {
      this.releaseFonts = null;
    }
    // Pictures linked at web addresses stay placeholders until the person asks for them, once per document.
    this.env = {
      engine,
      showLinkedPictures: () => {
        this.env.linkedPictures = true;
        this.rerender();
      },
    };
    this.paginator = new Paginator(this.flow, this.sheets, this.stage, this.env);
    this.paginator.zoomOf = () => this.zoom;
    this.paginator.fieldHook = (root) => this.fieldCodes.apply(root);
    this.folds.reset();
    const page = engine.pageSetup();
    this.setup = page;
    this.setupKey = JSON.stringify(page);
    if (this.contSpec) this.paginator.setContinuous(this.continuousLayout(this.contSpec, page));
    this.floatMetrics(page);
    this.paginator.configure(page);
    this.rulerSection = 0;
    this.aiMarks.clear();
    this.spell.clear();
    // A new document starts on the tab its file was saved on.
    this.openTabId = null;
    this.knownTabs = [];
    this.apply(engine.renderAll());
    this.sectionsDirty = false;
    this.applyAutoFit();
    const first = this.flow.querySelector<HTMLElement>("[data-p]:not([data-tab-hidden] [data-p]):not([data-tab-hidden][data-p])") ?? this.flow.querySelector<HTMLElement>("[data-p]");
    this.sel = first ? { anchor: { p: Number(first.dataset.p), o: 0 }, focus: { p: Number(first.dataset.p), o: 0 } } : null;
    this.syncRuler(this.format());
    this.canvas.scrollTop = 0;
    this.layoutNow();
    this.hooks.onChange?.();
  }

  /** Remove the editor from the page and release everything it registered. */
  destroy() {
    if (this.story) this.exitStory(false);
    this.releaseFonts?.();
    this.marks.show(false);
    this.guides.destroy();
    for (const p of this.peers) p.peers.delete(this);
    this.peers.clear();
    this.spell.destroy();
    this.extra.destroy();
    this.life.dispose();
    this.canvas.remove();
  }

  /** Show every tab (to print them all) instead of the open one. */
  allTabs = false;
  /** The document's tabs as last drawn; `active` is the tab this pane shows. */
  tabs: import("@docxtor/core").TabsInfo = { tabs: [] };
  /**
   * The tab this pane shows. What a person looks at is view state: each pane has its own, and no op changes it, so two
   * panes of a split never fight over it and a read-only document can still change tabs. Null until chosen: the tab the
   * file was saved on then.
   */
  openTabId: string | null = null;
  /** The tabs as last drawn, in order: where the open tab was when it goes. */
  knownTabs: string[] = [];
  /**
   * Whether this pane tells the engine which tab to write into the file when it is saved. The first pane does (a split's
   * second pane does not): it is the one that owns the file, and the answer does not depend on where the keyboard was
   * when the autosave ran.
   */
  savesOpenTab = true;

  /** What the blocks' tab marks were last painted from: the open tab and where each tab starts and ends. */
  paintedTabs = "";

  /** The page setup as last read (headers, footers, notes and sections as the engine described them), to tell whether an undo or a redo changed it. */
  setupKey = "";

  /** Something that is not a drawn block changed since the last layout (fonts, a peer's setup): the next one is a full one. */
  pendingFull = false;
  /** The font faces that were loaded when the pages were last laid out whole. */
  facesAtFull = new Set<FontFace>();

  get pages() {
    return this.paginator?.pages ?? 1;
  }

  /** Every rendered block at any depth by id (what `paragraphView` and `blockView` answer from, kept up to date by `apply`). */
  views = new Map<number, RBlock>();

  /** The element holding the sheets and the text (what printing copies). */
  get stageElement(): HTMLElement {
    return this.stage;
  }

  /** The first page's size in CSS pixels, as laid out now (pages can differ: see `pageBox`). */
  get pageGeometry(): Readonly<{ pageW: number; pageH: number }> {
    const b = this.paginator.page(0);
    return { pageW: b.w, pageH: b.h };
  }

  /** What objects placed from the page need to know about the pages (see float-place.ts). */
  get floatHost(): FloatHost {
    return {
      stage: this.stage,
      zoom: this.zoom,
      pages: this.pages,
      pageBox: (k) => this.paginator.page(k),
      margins: (k) => {
        const s = this.pageInfo.sections?.[this.paginator.page(k).section] ?? this.pageInfo;
        // Twips in the file, points here.
        return s.margin.map((v) => v / 20) as [number, number, number, number];
      },
    };
  }

  /** True while the zoom follows the available width (never above 100%): the automatic mode. */
  get autoFit(): boolean {
    return this.zoomer.mode === "auto";
  }

  /** What the zoomer needs to know. */
  get canvasReady(): boolean {
    return !!this.paginator;
  }
  get zoomApplied(): boolean {
    return this.zoomSet;
  }
  /** The widest page's width in px at 100%. */
  get stageWidth(): number {
    return this.paginator.stageWidth;
  }
  get widestPage(): number {
    return this.paginator.stageWidth;
  }
  /** The ruler's height in px at 100% (0 when it is off or the layout has none). */
  get rulerHeight(): number {
    return this.rulerShown && !this.continuous ? this.ruler.el.offsetHeight : 0;
  }
  /** The text is laid out as one unbroken sheet (Pageless, Reading, Outline). */
  get continuous(): boolean {
    return !!this.paginator?.continuous;
  }
  /** The page elements, for pictures of a page (see snapshot.ts). */
  get sheetsElement(): HTMLElement {
    return this.sheets;
  }

  // ---------------------------------------------------------------------------
  // Views: continuous layouts and folding
  // ---------------------------------------------------------------------------

  /** Bumped whenever pages are drawn or left out (by C2); the overlays that draw only for drawn pages look at it on a scroll. */
  farStamp = 0;
  farSeen = 0;

  /** How many pages either side of the view stay drawn when far pages are left out (the test switch `__DOCXTOR_FAR_PAGES__`, or the rule for long documents); null when they are not. Worked out in one place: `far.ts`. */
  get farKeep(): number | null {
    return this.story || this.continuous || this.allTabs || this.everyPage ? null : farKeepFor(this.pages);
  }

  private everyPage = false;

  /**
   * Every page is drawn, however long the document (screen reader support): a screen reader reads the whole text, and the browser's own find
   * sees all of it. Off, a long document draws only the pages near the view (`farKeep`).
   */
  get drawEveryPage(): boolean {
    return this.everyPage;
  }

  set drawEveryPage(on: boolean) {
    if (this.everyPage === on) return;
    this.everyPage = on;
    if (on) this.updateFar();
    else this.farSoon();
  }

  /** Blocks `ensureDrawn` brought back, kept drawn until the next frame. */
  farPins = new Set<HTMLElement>();
  pinFrame = 0;
  /** `withDrawn` is running: nothing is left out until it is done. */
  farHold = 0;
  farFrame = 0;
  /** What the drawn stretch was last worked out from. */
  farSig = "";

  // ---------------------------------------------------------------------------
  // Executing ops
  // ---------------------------------------------------------------------------

  /** Apply ops and update the view. Returns the engine results (or null on error). */
  exec(ops: Op | Op[], opts: { merge?: boolean; select?: ModelSel | MPos | null; typing?: boolean; keepExtra?: boolean; onError?: (message: string) => void } = {}) {
    // A read-only document still updates fields as a file opens: ops the engine says change nothing a person wrote.
    if (this.readOnly && !Engine.opsAreViewOnly(ops)) return null;
    // A tab this pane adds or copies is the one it shows (the engine has no open tab to change).
    const list = Array.isArray(ops) ? ops : [ops];
    const showsNew = list.some((o) => o.op === "tab" && ((o as { action?: string }).action === "duplicate" || ((o as { action?: string; open?: boolean }).action === "add" && (o as { open?: boolean }).open !== false)));
    const tabsBefore = showsNew ? this.tabs.tabs.map((x) => x.id) : [];
    this.lastAuto = null;
    this.pasteFlow.edited();
    let res;
    try {
      res = this.engine.apply(ops, !!opts.merge);
    } catch (e) {
      // A caller that shows the message itself (inline in a dialog) asks for it.
      (opts.onError ?? this.hooks.onError)?.(String((e as Error).message ?? e));
      return null;
    }
    if (showsNew) {
      const fresh = this.engine.tabsInfo().tabs.map((x) => x.id).filter((id) => !tabsBefore.includes(id));
      if (fresh.length) this.openTabId = fresh[fresh.length - 1];
    }
    // The other selected ranges stay only for an edit that says it moved no text or put them right itself.
    if (!opts.keepExtra) this.extra.clear();
    const caret = [...res].reverse().find((r) => r.caret)?.caret;
    if (opts.select !== undefined) {
      const s = opts.select;
      this.sel = s && "anchor" in s ? s : s ? { anchor: s, focus: s } : this.sel;
    } else if (caret) this.sel = { anchor: caret, focus: caret };
    this.typedEnd = opts.typing && this.sel ? { ...this.sel.focus } : null;
    this.applyOut(this.engine.renderChanged(), !!opts.typing);
    if (opts.typing) this.spell.typed();
    if (opts.typing && this.order.length > 400) this.scheduleLayout(true);
    else this.layoutNow({ incremental: true });
    this.marks.invalidate();
    if (this.sel) {
      writeSelection(this.root, this.sel);
      this.scrollCaretIntoView();
    }
    this.hooks.onChange?.();
    this.emitSelection();
    if ((Array.isArray(ops) ? ops : [ops]).some((o) => o.op === "replace_font")) this.hooks.onFontsMayChange?.();
    return res;
  }

  /** Take back the last step, or the last `steps` of them (choosing a point in the undo list). */
  undo(steps = 1) {
    if (this.readOnly) return;
    const auto = this.lastAuto;
    this.lastAuto = null;
    if (!(steps > 1 ? this.engine.undoSteps(steps) : this.engine.undo())) return;
    this.afterHistory();
    // Taking back a substitution leaves the caret where it was when the text was typed.
    if (auto) {
      this.sel = { anchor: auto.restore, focus: auto.restore };
      this.pending = null;
      writeSelection(this.root, this.sel);
      this.emitSelection();
    }
  }

  /** Do the next step again, or the next `steps` of them. */
  redo(steps = 1) {
    if (!this.readOnly && (steps > 1 ? this.engine.redoSteps(steps) : this.engine.redo())) this.afterHistory();
  }

  /** Why this pane can't be changed (its own view or tool); see `locks.ts`. Reasons for the whole window are `windowLocks`. */
  readonly locks = new LockSet(() => {
    this.applyReadOnly();
    this.hooks.onReadOnly?.();
  });
  /** @internal The reasons that hold for every pane of the window (the editor component's). */
  windowLocks: LockSet | null = null;

  /** When read-only, the document can be read, selected and copied, but not changed. */
  get readOnly() {
    return this.locks.size > 0 || (this.windowLocks?.size ?? 0) > 0;
  }

  /** Why this pane is read-only, in the words the features gave. */
  lockReasons(): string[] {
    return [...new Set([...this.locks.reasons(), ...(this.windowLocks?.reasons() ?? [])])];
  }

  /** Make this pane read-only for `reason` until the returned function is called. Only this pane's typing is stopped; see `DocxtorEditor.lock` for the whole window. */
  lock(reason: string, scope: LockScope = "host"): () => void {
    return this.locks.hold(reason, scope);
  }

  /** @internal Make the surface match the reasons held. */
  applyReadOnly() {
    const on = this.readOnly;
    // A box that can't be edited takes the keyboard (to scroll, to copy) only if it has a tab index of its own; Chrome lets a scrolling
    // box have it, Firefox and Safari do not, and the keyboard would be left on the page body when the document is locked.
    if (on) this.flow.tabIndex = -1;
    else this.flow.removeAttribute("tabindex");
    if (on && this.story) this.exitStory(false);
    this.flow.contentEditable = on ? "false" : "true";
    this.flow.setAttribute("aria-readonly", String(on));
    this.canvas.classList.toggle("read-only", on);
  }

  private afterHistory() {
    this.pasteFlow.edited();
    // Where the caret's block stood, to find the caret a place when the undo takes its paragraph away (an undone paste).
    const caret = this.sel;
    const was = caret && !this.story ? this.topBlockId(caret.focus.p) : null;
    const oldOrder = this.order;
    const oldAt = was === null ? undefined : this.orderIdx.get(was);
    // Undo can bring back another page size or margins, which aren't part of the rendered text.
    this.reloadStories(false, true);
    this.refresh({ keepScroll: true });
    this.applyAutoFit();
    // (While it was drawn again the editor may already have read some other place from the browser's leftover selection.)
    if (caret && oldAt !== undefined && !paraIn(this.root, caret.focus.p)) {
      // The start of the first block after it that is still there (where the browser's own selection would fall is no guide: it sits between
      // the flow's children, and far blocks are kept in boxes).
      for (let i = oldAt; i < oldOrder.length; i++) {
        if (!this.orderIdx.has(oldOrder[i])) continue;
        const el = this.blocks.get(oldOrder[i]);
        const p = el?.dataset.p ?? el?.querySelector<HTMLElement>("[data-p]")?.dataset.p;
        if (p === undefined) continue;
        this.sel = { anchor: { p: Number(p), o: 0 }, focus: { p: Number(p), o: 0 } };
        this.ensureDrawn(Number(p));
        break;
      }
    }
    if (this.sel) {
      const len = paraLength(this.root, this.sel.focus.p);
      const clamp = (p: MPos) => ({ p: p.p, o: Math.min(p.o, len) });
      this.sel = { anchor: clamp(this.sel.anchor), focus: clamp(this.sel.focus) };
      this.root.focus();
      writeSelection(this.root, this.sel);
    }
    this.emitSelection();
  }

  // ---------------------------------------------------------------------------
  // Selection helpers
  // ---------------------------------------------------------------------------

  // ---------------------------------------------------------------------------
  // More than one selection (see multi-select.ts)
  // ---------------------------------------------------------------------------

  /** Whether other ranges are selected besides the real selection. */
  get multi(): boolean {
    return this.extra.ranges.length > 0;
  }

  formatKept: { engine: Engine; applied: number; p: number; o: number; f: FormatState | null } | null = null;
  /** How many times changes from the engine were drawn (see `format`). */
  applied = 0;

  /** The section the horizontal ruler shows. */
  rulerSection = 0;

  // ---------------------------------------------------------------------------
  // Commands (toolbar / shortcuts)
  // ---------------------------------------------------------------------------

  get hasCopiedFormat() {
    return !!this.painter.copied;
  }

  /** Whether non-printing characters are drawn. */
  get marksShown() {
    return this.marks.shown;
  }

  /** Give another editor (a pane of a split window) the display settings and hooks this one has. */
  copySettingsTo(o: Editor) {
    o.fieldServices = this.fieldServices;
    o.autoCaptions = this.autoCaptions;
    o.autoCaptionsSave = this.autoCaptionsSave;
    o.setAutoformat(this.autoformat);
    o.showMarks(this.marksShown);
    o.showHiddenText(this.hiddenOwn);
    o.showFieldCodes(this.fieldCodesShown);
    o.showRuler(this.rulerShown);
    o.showGridlines(this.guides.gridlines);
    o.showBoundaries(this.guides.boundaries);
    o.setNativeSpellcheck(this.nativeSpell);
  }

  /** Hidden text is drawn when it is switched on here, and whenever non-printing characters are shown (as in Word). */
  hiddenOn = false;

  get hiddenShown() {
    return this.hiddenOn || this.marks.shown;
  }

  /** Whether View, Show hidden text is on (apart from non-printing characters, which show it too). */
  get hiddenOwn() {
    return this.hiddenOn;
  }

  get rulerShown() {
    return !this.ruler.el.hidden;
  }

  // ---------------------------------------------------------------------------
  // Equations
  // ---------------------------------------------------------------------------

  // ---------------------------------------------------------------------------
  // Bookmarks
  // ---------------------------------------------------------------------------

  // ---------------------------------------------------------------------------
  // Citations and the bibliography
  // ---------------------------------------------------------------------------

  /** The page setup as read at the last load or change (cheap; `engine.pageSetup()` builds it again). */
  get pageInfo(): PageSetup {
    return (this.setup ??= this.engine.pageSetup());
  }

  get nativeSpellcheck() {
    return this.nativeSpell;
  }

  // ---------------------------------------------------------------------------
  // Headers, footers and footnotes
  // ---------------------------------------------------------------------------

  // ---------------------------------------------------------------------------
  // Captions, cross-references, the index and updating fields
  // ---------------------------------------------------------------------------

  /** Whether field codes are shown in place of results (View, Field codes). */
  get fieldCodesShown(): boolean {
    return this.fieldCodes.all;
  }

  /** The element of the story being edited (for overlays). */
  get storyElement(): HTMLElement | null {
    return this.storyEl;
  }

  // ---------------------------------------------------------------------------
  // Input
  // ---------------------------------------------------------------------------

  /** Paragraph element for a model position (used by overlays). */
  paraElement(id: number): HTMLElement | null {
    return paraIn(this.root, id);
  }

  /** Paragraph containing the DOM selection focus, if any. */
  focusPara(): HTMLElement | null {
    const s = window.getSelection();
    return s ? paraOf(s.focusNode) : null;
  }
}

// The methods that live in the modules beside this file (see the imports) are the class's own: installed on its prototype, as class methods are
// (not enumerable).
for (const methods of [renderMethods, viewMethods, selectionMethods, formatMethods, referenceMethods, insertMethods, fieldMethods, storyMethods, inputMethods]) {
  for (const [name, value] of Object.entries(methods)) Object.defineProperty(Editor.prototype, name, { value, writable: true, configurable: true });
}

// Each module's methods must match what its `declare module` block says the class has, name by name and signature by signature:
// a method missing from the block, or one whose parameters or result differ from it, fails to compile here.
type Declared<T> = Pick<Editor, keyof T & keyof Editor> & Record<Exclude<keyof T, keyof Editor>, never>;
void ([renderMethods, viewMethods, selectionMethods, formatMethods, referenceMethods, insertMethods, fieldMethods, storyMethods, inputMethods] satisfies [Declared<typeof renderMethods>, Declared<typeof viewMethods>, Declared<typeof selectionMethods>, Declared<typeof formatMethods>, Declared<typeof referenceMethods>, Declared<typeof insertMethods>, Declared<typeof fieldMethods>, Declared<typeof storyMethods>, Declared<typeof inputMethods>]);

