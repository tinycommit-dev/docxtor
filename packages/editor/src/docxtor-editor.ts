// DocxtorEditor: the whole editing experience behind one object. Bytes in, bytes out; the host decides
// where documents live. The toolbar and the page area are separate elements, so a host can lay them
// out as it likes (the Docxtor app puts a navigator and an assistant between them), or call mount().
import { Engine, loadEngine, OpenError, type FormatState, type Op, type OpResult, type ProtectionInfo, type WasmSource } from "@docxtor/core";
import { currentLocale, ExtensionHost, textDirection, msg, pickLanguage, PSEUDO_LOCALE, setLanguage, sortViews, t, type AddOptions, type CommandResult, type Disposable, type DocumentApi, type DocumentSource, type Extension, type HostServices, type Importer, type OpenNotice, type UiApi, type ViewMode } from "@docxtor/extensions";
import { builtinExtensions } from "./builtins";
import { describeOpenError, describeOpenReport } from "./open-report";
import type { SpellingProvider } from "./spelling/types";
import { Editor, type StoryEdit } from "./editor/editor";
import { SplitWindow } from "./editor/split";
import { LockSet, type LockScope } from "./editor/locks";
import { openBookmarkPopover } from "./ui/bookmarks";
import { openCitationPopover, type CitationPopoverOptions } from "./ui/citations";
import { openEquationEditor } from "./ui/equation";
import { openPhoneticGuide } from "./ui/phonetic-guide";
import { Bubble } from "./ui/bubble";
import { applyTheme, applyTokens, confirmDialog, h, keepFocusOnFrame, promptDialog, setOverlayTheme, setOverlayTokens, toast, type Theme } from "./ui/dom";
import { ensureFonts, useFont } from "./fonts";
import { FindBar } from "./ui/find";
import { ImageTool } from "./ui/image-tool";
import { InkTool } from "./ui/ink-tool";
import { ShapeTool } from "./ui/shape-tool";
import { MenuBar } from "./ui/menu-bar";
import { downloadBytes } from "./mailmerge/download";
import { openPageSetup } from "./ui/page-setup";
import { quickPick } from "./ui/quick-pick";
import { StoryBar } from "./ui/story-bar";
import { TableTool } from "./ui/table-tool";
import { ControlsTool } from "./ui/controls";
import { ProtectionTool } from "./ui/protection";
import { Toolbar } from "./ui/toolbar";
import { loadFaces } from "./editor/render";

export interface DocxtorEditorOptions {
  /** Show the formatting toolbar (default true). */
  toolbar?: boolean;
  /** Show an "Ask" button for `ask` in the toolbar (default true; turn it off when the host has its own). */
  toolbarAsk?: boolean;
  /** Show the menu bar (File, Edit, View, Insert, Format…) above the toolbar (default false). */
  menuBar?: boolean;
  /** Start read-only: readable, selectable and copyable, not editable. */
  readOnly?: boolean;
  /**
   * Typing substitutions (smart quotes, capital letters, links, Markdown shortcuts, symbols such as (c) to ©),
   * which people switch on and off under Tools, Preferences (default true). Pass false to type exactly what's typed:
   * nothing changes the text, and the Preferences command is left out.
   */
  autoformat?: boolean;
  /** Ask the AI about the selection. When given, "Ask" appears in the toolbar and the selection bubble. */
  ask?: (prompt?: string) => void;
  /** ⌘S / Ctrl-S inside the editor. Without it the shortcut is left to the host page. */
  onSave?: (bytes: Uint8Array) => void;
  /** Errors meant for the user (default: a toast). */
  onError?: (message: string) => void;
  /**
   * Open a document the editor made (the result of a mail merge, a sheet of labels) as a new document of its own: the host puts it
   * in a window, a tab or its file list. Without it the .docx is downloaded.
   */
  openNewDocument?: (bytes: Uint8Array, name: string) => void | Promise<void>;
  /**
   * Offered every address a document opens (a link, a chip's Open button) before the editor opens it: return true when
   * the host opened it itself, for example an address of the host's own documents. Without it, or when it returns false,
   * web and email addresses open in a new tab and other kinds are refused.
   */
  openAddress?: (url: string) => boolean;
  /**
   * Font stylesheet to load (Inter + Word-compatible faces). Default: Google Fonts.
   * Pass your own URL to self-host, or false if the page already loads them.
   */
  fonts?: string | false;
  /** Extensions to activate after the built-in ones. More can be added later with `use()`. */
  extensions?: Extension[];
  /** Ids of extensions (built-in or not) to start disabled. */
  disabledExtensions?: string[];
  /**
   * Docxtor's own spelling and grammar checkers: wavy underlines, suggestions in the right-click menu, Tools,
   * Spelling and grammar. Spelling needs dictionaries, which the editor does not bundle: pass a provider that
   * returns the Hunspell `.aff` and `.dic` files for a language (see docs/embedding.md), or register
   * one from an extension with `ctx.spelling.dictionaries` (this option is a shortcut for that). Without a
   * dictionary for the document's language the browser's own spellcheck keeps working; grammar needs none.
   * `false` leaves out both checkers.
   */
  spelling?: SpellingProvider | false;
  /** Where extensions keep their settings (default: in memory). */
  extensionState?: HostServices["state"];
  /**
   * Interface language, e.g. `es` or `pt-BR`, from the language packs in `extensions`. Without it the
   * page's current language stays (English unless the host called `setLanguage`). One language per page.
   */
  locale?: string;
  /**
   * Which of Word's editing groups the user counts as (`editors`, `contributors`, `owners`...), for the parts of a
   * protected document that only some people may edit. Parts for `everyone` always count. Default: none.
   */
  userGroups?: string[];
  /** The id of the icon pack to start with (`docxtor` by default; `docxtor-light` ships with Docxtor; extensions add more). Change it later with `setIconPack()`. */
  iconPack?: string;
}

export interface DocxtorEditorEvents {
  /** The document changed (typing, formatting, undo, AI edits…). */
  change: void;
  /** The selection or the formatting under the caret changed. */
  selection: FormatState | null;
  /** Pagination finished. */
  layout: { pages: number };
  /** A header, footer or the footnotes started (or stopped) being edited. */
  story: StoryEdit | null;
  /** A document was opened or created. */
  open: void;
  /** The theme changed (built-in or contributed). */
  theme: string;
  /** The view changed (`print`, `pageless`, `reading`, `outline`, or one an extension added): the id of the new one. */
  view: string;
  /** Compact mode was turned on or off. */
  compact: boolean;
  /** The window was split in two panes (true), or the split was taken away (false). */
  split: boolean;
  /** @internal The fonts the document uses may have changed (undo, redo, paste, replacing a font). For the editor's own extensions. */
  "fonts-may-change": void;
  /** What the document protects changed (Restrict editing, Mark as final, an opened file that is protected). `null` when there is no document. */
  protection: ProtectionInfo | null;
}

export interface ThemeOption {
  id: string;
  label: string;
}

/** An exported file: name, media type and contents. */
export interface ExportedFile {
  name: string;
  mime: string;
  bytes: Uint8Array;
}

type Listener<T> = (value: T) => void;

const DOCX = /\.(docx|docm|dotx)$/i;

/** Whether the bytes start like a zip or a compound file (what Word files, protected or not, are). */
function looksLikeZipOrCompound(b: Uint8Array): boolean {
  return (b[0] === 0x50 && b[1] === 0x4b) || (b[0] === 0xd0 && b[1] === 0xcf && b[2] === 0x11 && b[3] === 0xe0);
}
const BUILTIN_THEMES: ThemeOption[] = [
  { id: "system", label: msg("Match the system") },
  { id: "light", label: msg("Light") },
  { id: "dark", label: msg("Dark") },
];

/** Turn what an importer or template produced into .docx bytes. */
export async function docxFrom(source: DocumentSource, name = "Untitled.docx"): Promise<Uint8Array> {
  if ("docx" in source) return source.docx;
  await loadEngine();
  const eng = Engine.fromMarkdown(source.markdown, name);
  try {
    return eng.save();
  } finally {
    eng.dispose();
  }
}

/** What a pane shows: the view it is in and what that view set up. */
interface ViewState {
  id: string;
  exit: (() => void) | null;
  classes: string[];
  /** Lets go of the reason the view holds to keep the text from changing (null while the view holds none). */
  release: (() => void) | null;
}

/** Calls that are for every pane of a split window, not only the one with the keyboard (display settings the person set for the document). */
const EVERY_PANE = new Set(["showMarks", "showHiddenText", "showFieldCodes", "showRuler", "showGridlines", "showBoundaries", "setNativeSpellcheck", "setAutoformat"]);
/** Settings the host assigns once; a split window gives them to every pane. */
const SHARED_FIELDS = new Set(["fieldServices", "autoCaptions", "autoCaptionsSave", "aiMarks"]);

export class DocxtorEditor {
  /**
   * The editing surface itself, for advanced use (commands, selection, zoom, stories). While the window is split this is
   * the pane with the keyboard, so commands, the toolbar and the menus act on the pane being used.
   */
  readonly editor: Editor;
  /** The formatting toolbar element (null when created with `toolbar: false`). */
  readonly toolbarEl: HTMLElement | null;
  /** The menu bar, when `menuBar` is on (`mount()` places it; hosts that lay out themselves place it). */
  readonly menuBarEl: HTMLElement | null;
  private menuBar: MenuBar | null;
  /** The page area: canvas, pages and the find bar. */
  readonly surface: HTMLElement;
  /** Extensions and everything they contribute: commands, toolbar items, exporters, themes… */
  readonly extensions: ExtensionHost;
  /** Settings for fields. The built-in fields extension keeps them between sessions; hosts can set them directly. */
  readonly fieldOptions: { updateBeforeOutput: boolean; user: { name?: string; initials?: string; address?: string } } = { updateBeforeOutput: true, user: {} };
  /** Resolves once the built-in extensions and `options.extensions` are active. */
  readonly ready: Promise<void>;
  /** @internal Whether typing substitutions are on offer (the `autoformat` option). For the editor's own extensions. */
  readonly autoformatEnabled: boolean;

  private readonly toolbar: Toolbar | null;
  private readonly bubble: Bubble;
  private readonly findBar: FindBar;
  private readonly images: ImageTool;
  private readonly shapes: ShapeTool;
  private readonly ink: InkTool;
  private readonly tables: TableTool;
  private readonly controls: ControlsTool;
  private readonly protectionTool: ProtectionTool;
  private readonly storyBar: StoryBar;
  private readonly opts: DocxtorEditorOptions;
  private listeners = new Map<keyof DocxtorEditorEvents, Set<Listener<never>>>();
  private root: HTMLElement | null = null;
  private ownedEngine: Engine | null = null;
  /** What opening the document did that the person should hear about, and a notice a host set for the next one. */
  private notice: OpenNotice | null = null;
  private pendingNotice: OpenNotice | null | undefined;
  private theme = "system";
  private fontReleases = new Map<object, () => void>();
  private themeTokens: Record<string, string> = {};
  /** The editor the page was built with, and the pane with the keyboard (the same until a split window gives the second pane focus). */
  private readonly primary: Editor;
  private active: Editor;
  /**
   * Why the whole window can't be changed: the host (its option, the `readonly` attribute), the document (the window
   * lock, marked as final, a mail merge preview). A pane's own reasons (Reading view, Line focus) are on the pane.
   */
  private readonly windowLocks = new LockSet(() => this.windowLocksChanged());
  private hostRelease: (() => void) | null = null;
  /** The view each pane is in. */
  private viewStates = new Map<Editor, ViewState>();
  /** Two panes of the same document, one above the other (View, Split). */
  readonly split: SplitWindow;
  private compactOn = false;

  /** Load the engine's WebAssembly once (optional: open() and create() do it on demand). */
  static ready(wasm?: WasmSource): Promise<unknown> {
    return loadEngine(wasm);
  }

  constructor(opts: DocxtorEditorOptions = {}) {
    this.opts = opts;
    this.autoformatEnabled = opts.autoformat !== false;
    const off = new Set(opts.disabledExtensions ?? []);
    // Language packs are plain data, so the interface can be built in the right language from the start.
    if (opts.locale !== undefined) {
      const packs = (opts.extensions ?? []).filter((x) => !off.has(x.id)).flatMap((x) => x.contributes?.languages ?? []);
      setLanguage(pickLanguage(opts.locale, packs));
    }
    if (opts.fonts !== false) ensureFonts(opts.fonts);
    this.surface = h("section", { class: "editor-host" });
    this.primary = new Editor(this.surface);
    this.primary.windowLocks = this.windowLocks;
    this.active = this.primary;
    this.editor = this.paneProxy();
    this.split = new SplitWindow(this.splitHost());
    this.extensions = new ExtensionHost({
      document: this.documentApi(),
      ui: this.uiApi(),
      state: opts.extensionState,
      onError: (id, err) => console.warn(`Docxtor extension ${id}:`, err),
    });
    if (opts.iconPack) this.extensions.setIconPack(opts.iconPack);
    this.toolbar = opts.toolbar === false ? null : new Toolbar(this.editor, opts.ask && opts.toolbarAsk !== false ? () => opts.ask?.() : null, this.extensions);
    this.toolbarEl = this.toolbar?.el ?? null;
    this.menuBar = opts.menuBar ? new MenuBar(this.extensions, { focusDocument: () => this.editor.engine && this.editor.focus() }) : null;
    this.menuBarEl = this.menuBar?.el ?? null;
    this.bubble = new Bubble(this.editor, opts.ask ?? null, () => this.toolbar?.openLink());
    this.findBar = new FindBar(this.editor);
    this.images = new ImageTool(this.editor);
    this.images.chartPalettes = () => this.extensions.chartPalettes.list();
    this.shapes = new ShapeTool(this.editor);
    this.ink = new InkTool(this.editor, this.shapes);
    this.tables = new TableTool(this.editor, () => this.extensions.tableDesigns.list());
    this.controls = new ControlsTool(this.editor, () => this.extensions.contentControls.list());
    this.extensions.contentControls.onChange(() => this.controls.reapply());
    this.protectionTool = new ProtectionTool(this.editor, (reason) => this.lock(reason, { scope: "document" }));
    this.protectionTool.onChange((info) => this.emit("protection", info));
    this.storyBar = new StoryBar(this.editor);
    this.surface.append(this.findBar.el);

    const ed = this.editor;
    ed.fieldServices = {
      user: () => this.fieldOptions.user,
      evaluates: (code) => this.extensions.evaluatesField(code),
      evaluate: (call) => this.extensions.evaluateField(call),
      updateBeforeOutput: () => this.fieldOptions.updateBeforeOutput,
    };
    ed.hooks = {
      ask: (id, ...args) => this.extensions.ask(id, ...args),
      command: (id, ...args) => this.extensions.execute(id, ...args),
      openAddress: (url) => this.opts.openAddress?.(url) ?? false,
      onChange: () => {
        this.images.refresh();
        this.shapes.refresh();
        this.tables.refresh();
        this.controls.refresh();
        this.emit("change", undefined);
      },
      onSelection: (f) => {
        this.toolbar?.update(ed.pending ? ed.formatWithPending() : f);
        this.controls.refresh();
        this.refreshBubble();
        this.emit("selection", f);
      },
      onLayout: (pages) => {
        this.storyBar.place();
        this.emit("layout", { pages });
      },
      onStory: (st) => {
        this.storyBar.show(st);
        this.images.hide();
        this.shapes.hide();
        if (st) this.bubble.update();
        this.emit("story", st);
      },
      onFontsMayChange: () => this.emit("fonts-may-change", undefined),
      onReadOnly: () => this.locksChanged(),
      onBookmark: (name, marker) => openBookmarkPopover(ed, name, marker),
      onCitation: (citation, el) => openCitationPopover(ed, citation, el, this.citationPopoverOptions()),
      onEquation: (equation, el) => openEquationEditor(ed, { edit: equation }, el),
      onRuby: (ruby, el) => openPhoneticGuide(ed, ruby, el, (text, lang) => this.extensions.readingFor(text, lang)),
      onError: (m) => (opts.onError ?? toast)(ControlsTool.explain(m)),
      beforeEdit: (e, from, to) => this.controls.beforeEdit(e, from, to),
      beforeKey: (e) => this.controls.beforeKey(e),
    };
    ed.life.on(document, "selectionchange", () => this.refreshBubble());
    const keys = (e: Event) => {
      const k = e as KeyboardEvent;
      const mod = this.extensions.mac ? k.metaKey : k.ctrlKey;
      if (mod && !k.shiftKey && !k.altKey && k.key.toLowerCase() === "s" && this.opts.onSave && ed.engine) {
        k.preventDefault();
        // In steps: the compressing runs in a worker, so a long document neither holds the page nor has the engine build the whole file at once.
        const onSave = this.opts.onSave;
        void ed.engine.saveInSteps().then(
          (r) => onSave(r.bytes),
          (err) => console.error(err),
        );
        return;
      }
      this.handleKey(k);
    };
    ed.life.on(this.surface, "keydown", keys);
    if (this.toolbarEl) ed.life.on(this.toolbarEl, "keydown", keys);
    this.extensions.fonts.onChange(() => this.syncFonts());
    // A citation style that arrives after the document (an extension turned on) is added to it.
    this.extensions.citationStyles.onChange(() => void this.useDocumentCitationStyle());
    // A view whose extension went away falls back to the print layout.
    this.extensions.views.onChange(() => {
      for (const pane of this.split.panes()) {
        if (this.viewOf(pane).id !== "print" && !this.extensions.views.list().some((v) => v.id === this.viewOf(pane).id)) this.withActive(pane, () => this.setView("print"));
      }
    });
    this.extensions.themes.onChange(() => {
      // A contributed theme that goes away falls back to the system theme.
      if (!BUILTIN_THEMES.some((t) => t.id === this.theme) && !this.extensions.themes.list().some((t) => t.id === this.theme)) this.setTheme("system");
    });
    if (opts.readOnly) this.setReadOnly(true);

    const add = (ext: Extension, o: AddOptions = {}) => this.extensions.add(ext, { ...o, enabled: !off.has(ext.id) });
    const builtins = builtinExtensions({ fe: this, editor: ed, findBar: this.findBar, tables: this.tables, images: this.images, shapes: this.shapes, ink: this.ink, controls: this.controls, protection: this.protectionTool, openLink: this.toolbar ? () => this.toolbar?.openLink() : null, ask: opts.ask ?? null, hideBubble: () => this.bubble.hide(), spelling: opts.spelling || null, proofing: opts.spelling !== false, contextSources: [] });
    this.ready = (async () => {
      for (const ext of builtins) await add(ext, { builtin: true, required: ext.id === "docxtor.icons" || ext.id === "docxtor.cautions" });
      for (const ext of opts.extensions ?? []) await add(ext).catch((err) => console.warn(err));
    })();
  }

  /** Put the toolbar and the page area into `el`, making it a Docxtor root (tokens, theme, layout). */
  mount(el: HTMLElement, opts: { theme?: string } = {}): this {
    this.useRoot(el);
    el.classList.add("docxtor-editor");
    if (opts.theme) this.setTheme(opts.theme);
    if (this.menuBarEl) el.append(this.menuBarEl);
    if (this.toolbarEl) el.append(this.toolbarEl);
    el.append(this.surface);
    return this;
  }

  /** Takes the root's rule for clicks on the frame away (`keepFocusOnFrame`). */
  private unkeepFocus: (() => void) | null = null;

  /**
   * Treat `el` as this editor's Docxtor root without moving anything into it: for hosts that place
   * `toolbarEl` and `surface` themselves. Themes apply to it.
   */
  useRoot(el: HTMLElement): this {
    this.root = el;
    el.classList.add("docxtor");
    // A click on the frame round the text leaves the keyboard where it was.
    this.unkeepFocus?.();
    this.unkeepFocus = keepFocusOnFrame(el);
    // The interface reads the way its language does; the pages keep their own direction (editor.css).
    el.dir = textDirection();
    el.classList.toggle("compact", this.compactOn);
    this.applyThemeTo(el);
    return this;
  }

  /** Add an extension (and activate it unless `enabled: false`). */
  use(ext: Extension, opts: AddOptions = {}): Promise<void> {
    return this.extensions.add(ext, opts);
  }

  // -- themes ------------------------------------------------------------------

  /** Load fonts extensions added and release removed ones; the open document picks up new stand-ins. */
  private syncFonts() {
    const current = new Set<object>(this.extensions.fonts.list());
    let changed = false;
    for (const [font, release] of this.fontReleases) {
      if (current.has(font)) continue;
      release();
      this.fontReleases.delete(font);
      changed = true;
    }
    for (const font of this.extensions.fonts.list()) {
      if (this.fontReleases.has(font)) continue;
      this.fontReleases.set(font, useFont(font));
      changed ||= !!font.substitutes?.length || !!font.category;
    }
    if (changed) this.editor.rerender();
  }

  /** The id of the icon pack in use (`docxtor`, `docxtor-light`, or one an extension contributed). */
  get currentIconPack(): string {
    return this.extensions.iconPack;
  }

  /** Every icon pack to offer, Docxtor's first. */
  iconPacks(): { id: string; label: string }[] {
    return this.extensions.listIconPacks().map((p) => ({ id: p.id, label: t(p.label) }));
  }

  /** Switch the icons of the menus, the toolbar and the context menu to a pack. A pack that isn't there shows Docxtor's icons. */
  setIconPack(id: string) {
    this.extensions.setIconPack(id);
  }

  /** "system", "light", "dark", or the id of a theme an extension contributed. */
  setTheme(theme: Theme | string) {
    const contributed = this.extensions.themes.list().find((t) => t.id === theme);
    this.theme = contributed || BUILTIN_THEMES.some((t) => t.id === theme) ? theme : "system";
    const tokens = contributed?.tokens ?? {};
    if (this.root) this.applyThemeTo(this.root, tokens);
    setOverlayTheme(contributed?.base ?? (this.theme as Theme));
    setOverlayTokens(tokens);
    this.themeTokens = { ...tokens };
    this.emit("theme", this.theme);
  }

  /** The current theme's id. */
  get currentTheme(): string {
    return this.theme;
  }

  /** Every theme to offer: the built-in ones, then contributed ones. */
  themes(): ThemeOption[] {
    return [...BUILTIN_THEMES.map((x) => ({ ...x, label: t(x.label) })), ...this.extensions.themes.list().map((t) => ({ id: t.id, label: t.label }))];
  }

  private applyThemeTo(el: HTMLElement, tokens = this.themeTokens) {
    const contributed = this.extensions?.themes.list().find((t) => t.id === this.theme);
    applyTheme(el, contributed?.base ?? (this.theme as Theme));
    applyTokens(el, tokens, this.themeTokens);
  }

  // -- documents ---------------------------------------------------------------

  /**
   * Open a document. A .docx opens as is; other formats (Markdown, plain text, anything an
   * extension imports) are converted into a new .docx. Returns the engine for direct access.
   *
   * A password-protected .docx needs `password`. Without it this rejects with an `EncryptedError` (from `@docxtor/core`),
   * with a wrong one a `WrongPasswordError`; ask the person and open it again. The document is then saved protected
   * with the same password (`save()` and `onSave` give protected bytes) until `setPassword(null)`.
   */
  async open(bytes: Uint8Array | ArrayBuffer, name = "document.docx", password?: string): Promise<Engine> {
    try {
      const converted = await this.toDocx(bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes), name);
      await loadEngine();
      const engine = Engine.open(converted.bytes, converted.name, password);
      this.announceOpen(converted.notice ?? undefined);
      await this.prepare(engine);
      // (A document the host showed while the fonts loaded is replaced: this open is the one asked for last.)
      return this.adopt(engine);
    } catch (e) {
      // An `OpenError` says why in the person's language; its `code` and `detail` are the engine's.
      throw e instanceof OpenError ? new OpenError(e.code, e.detail, describeOpenError(e)) : e;
    }
  }

  /**
   * Get ready to show `engine`: the faces of the fonts it names are loaded (at most 400 ms), so that its first layout is laid out with them.
   * Hosts that show an engine of their own with `load` call this first; `open` does. Without it a long book is laid out twice: the bold and
   * italic faces of Docxtor's stand-in fonts arrive after the first layout (600 ms more in War and Peace).
   */
  async prepare(engine: Engine): Promise<void> {
    try {
      await loadFaces(engine.namedFonts());
    } catch {
      /* the layout just waits for the fonts as before */
    }
  }

  /** What opening the current document did that the person should hear about (a conversion, a repair), or null. */
  get openNotice(): OpenNotice | null {
    return this.notice;
  }

  /**
   * For hosts that open a file in steps (convert it, store it, load it): the notice for the next document shown with
   * `load`. `null` says there is nothing to tell. Without it `load` words the engine's own report.
   */
  announceOpen(notice: OpenNotice | null | undefined) {
    this.pendingNotice = notice;
  }

  /** Whether the open document is saved with a password. */
  get hasPassword(): boolean {
    return this.engine?.hasPassword ?? false;
  }

  /**
   * Protect the open document with a password, or remove its protection (`null`). The next `save()` writes the
   * change; the host decides whether that counts as an edit to save. The password stays in memory.
   */
  setPassword(password: string | null) {
    if (!this.engine) throw new Error(t("No document is open"));
    this.engine.setPassword(password);
  }

  /** Start a blank document with Word's default styles. */
  async create(name = "Untitled.docx"): Promise<Engine> {
    await loadEngine();
    return this.adopt(Engine.blank(name));
  }

  /**
   * Take the window's extra panes away (the split window), leaving the one editor. They lay out against the open
   * document's engine, so a host that is about to replace the document calls this first, while that engine is still alive
   * (`load` does too). Order for a swap: `closePanes()`, `load(new)`, then dispose the old engine.
   */
  closePanes() {
    this.split.close();
  }

  /**
   * Show an engine the host created and keeps ownership of. The host disposes the one it replaces, after this returns:
   * nothing here touches the old engine once the new one is handed to the editor. An engine the editor made itself
   * (`open`, `create`) is disposed here, at the same point.
   */
  load(engine: Engine) {
    this.closePanes();
    const replaced = this.ownedEngine;
    this.ownedEngine = null;
    // What kept the last document read-only (the window lock, marked as final...) was about that document.
    this.windowLocks.dropDocument();
    engine.setUserGroups(this.opts.userGroups ?? []);
    // What Word records on a save: who saved, and how many pages the document runs to.
    engine.saveInfo = () => ({ user: this.fieldOptions.user.name, pages: this.editor.pages });
    this.editor.load(engine);
    if (replaced && replaced !== engine) replaced.dispose();
    // A new engine is editable until it is told otherwise: what still holds (the host's option, the Reading view) tells it.
    this.syncAccess();
    // Dates and fields the file marks for updating are brought up to date as it opens.
    this.editor.updateFieldsOnOpen();
    this.notice = this.pendingNotice !== undefined ? this.pendingNotice : describeOpenReport(engine.openReport);
    this.pendingNotice = undefined;
    this.emit("open", undefined);
    void this.useDocumentCitationStyle();
  }

  /** Add the CSL style the open document names to its engine, so new citations are formatted in it. */
  private useDocumentCitationStyle(): Promise<boolean> {
    const engine = this.engine;
    return engine ? this.ensureCitationStyle(engine.citationStyle()) : Promise.resolve(false);
  }

  private adopt(engine: Engine): Engine {
    this.load(engine);
    this.ownedEngine = engine;
    return engine;
  }

  /**
   * @internal Show the find bar's list of results in another place, such as the app's navigator: `el` holds the list and
   * `show` brings that place into view. `null` puts the list back in the bar.
   */
  dockFindResults(dock: { el: HTMLElement; show(): void } | null) {
    this.findBar.setDock(dock);
  }

  /**
   * @internal What the popover on a citation offers: "Edit source" when an extension (the app's Citations panel)
   * provides the command `citations.editSource`. For the editor's own extensions.
   */
  citationPopoverOptions(): CitationPopoverOptions {
    const edit = this.extensions.command("citations.editSource");
    return edit && this.extensions.enabled(edit) ? { editSource: (tag) => void this.extensions.execute("citations.editSource", tag) } : {};
  }

  /**
   * Make the open document ready to format its citations in a style. A CSL style that an extension contributes
   * (`ctx.citations.styles`) is loaded and added to the engine; Docxtor's own five need nothing. Resolves false
   * when no extension offers the style or its CSL can't be read.
   */
  async ensureCitationStyle(id: string): Promise<boolean> {
    const engine = this.engine;
    if (!engine) return false;
    if (["apa", "mla", "chicago", "ieee", "harvard"].includes(id) || engine.citationStyles().some((s) => s.id === id)) return true;
    if (!this.extensions.citationStyleList().some((s) => s.id === id)) return false;
    try {
      const csl = await this.extensions.citationStyleCsl(id);
      // The person may have opened another document while the style loaded.
      if (csl === null || this.engine !== engine) return false;
      engine.registerCitationStyle(id, csl);
      return true;
    } catch (err) {
      console.warn(`Docxtor citation style ${id}:`, err);
      return false;
    }
  }

  /** Switch the document's citation style, as one undo step. Resolves false when the style isn't available. */
  async setCitationStyle(id: string): Promise<boolean> {
    if (!(await this.ensureCitationStyle(id))) return false;
    return !!this.editor.exec({ op: "set_citation_style", style: id });
  }

  /** Open .docx bytes the editor made as a new document, the way the host wants (see the `openNewDocument` option), or download them. */
  async openNewDocument(bytes: Uint8Array, name: string): Promise<void> {
    if (this.opts.openNewDocument) await this.opts.openNewDocument(bytes, name);
    else downloadBytes(bytes, name, "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
  }

  /** The open document's engine, if any. */
  get engine(): Engine | null {
    return this.editor.engine ?? null;
  }

  /** The document as .docx bytes. */
  save(): Uint8Array {
    if (!this.editor.engine) throw new Error(t("No document is open"));
    return this.editor.engine.save();
  }

  /**
   * The same bytes as `save()`, made in steps: the compressing runs in a worker where one can, so a long document does not hold the
   * page, and the engine never builds the whole file at once (`save()` does, for the length of the call). What ⌘S / Ctrl-S uses.
   */
  async saveInSteps(): Promise<Uint8Array> {
    if (!this.editor.engine) throw new Error(t("No document is open"));
    return (await this.editor.engine.saveInSteps()).bytes;
  }

  /** Export with a registered exporter: `docx`, `markdown`, `text`, or one an extension added. */
  async export(id = "docx"): Promise<ExportedFile> {
    const engine = this.editor.engine;
    if (!engine) throw new Error(t("No document is open"));
    const ex = this.extensions.exporter(id);
    if (!ex) throw new Error(t("Nothing exports as {format}", { format: id }));
    // Fields are brought up to date first (a setting, on by default), PDF included.
    await this.editor.updateFieldsForOutput();
    const out = await ex.export(engine);
    const bytes = typeof out === "string" ? new TextEncoder().encode(out) : out instanceof Uint8Array ? out : new Uint8Array(await out.arrayBuffer());
    const stem = engine.fileName.replace(/\.[^.]+$/, "") || "Untitled";
    return { name: `${stem}.${ex.extension}`, mime: ex.mime, bytes };
  }

  /** Whether `name` can be opened: a Word file, or a format an importer reads. */
  canOpen(name: string): boolean {
    return DOCX.test(name) || !!this.extensions.importerFor(name);
  }

  /**
   * Convert a file to .docx with the importer that reads it, found by the file's extension and, where importers can
   * tell, by what the bytes are (an RTF file called .docx opens as RTF). A real Word file passes through: the engine
   * repairs a damaged one when it opens it. `notice` says what the conversion left out, for the person.
   */
  async toDocx(bytes: Uint8Array, name: string): Promise<{ bytes: Uint8Array; name: string; notice?: OpenNotice }> {
    await loadEngine();
    if (DOCX.test(name)) {
      // A zip or a protected compound file is Word's: the engine sorts out damage, Strict markup and misnamed zips.
      if (looksLikeZipOrCompound(bytes)) return { bytes, name };
      const found = this.extensions.importerForBytes(bytes);
      if (!found) return { bytes, name };
      return this.convertWith(found, bytes, name);
    }
    const importer = this.extensions.importerFor(name, bytes);
    if (!importer) {
      // Say what it is when we can tell (a spreadsheet, a PDF), else what Docxtor opens.
      const kind = Engine.sniff(bytes);
      if (!bytes.length) throw new OpenError("empty", "", "");
      if (kind.startsWith("foreign:")) throw new OpenError("not-word", kind.slice(8), "");
      throw new Error(t("Docxtor can't open {name}. It opens {formats}.", { name, formats: this.importLabels() }));
    }
    return this.convertWith(importer, bytes, name);
  }

  private async convertWith(importer: Importer, bytes: Uint8Array, name: string): Promise<{ bytes: Uint8Array; name: string; notice?: OpenNotice }> {
    const docxName = `${name.replace(/\.[^.]+$/, "") || "Untitled"}.docx`;
    const source = await importer.import(bytes, name);
    return { bytes: await docxFrom(source, docxName), name: docxName, notice: "docx" in source ? source.notice : undefined };
  }

  private importLabels(): string {
    const labels = [t("Word documents (.docx)"), ...new Set(this.extensions.importers.list().map((i) => i.label))];
    return new Intl.ListFormat(currentLocale() === PSEUDO_LOCALE ? "en" : currentLocale(), { type: "conjunction" }).format(labels);
  }

  // -- commands ----------------------------------------------------------------

  /** Apply edit ops (the same language the AI tools and the CLI use) as one undoable step. */
  exec(ops: Op | Op[]): OpResult[] | null {
    return this.editor.exec(ops);
  }

  /**
   * Run a command by id (`format.bold`, `find.open`, `palette.show`, or one an extension added). It answers
   * `done` with the command's value, `declined` when the person said no to its question, or `unavailable`.
   */
  command<T = unknown>(id: string, ...args: unknown[]): Promise<CommandResult<T>> {
    return this.extensions.execute<T>(id, ...args);
  }

  /**
   * Run the command bound to a key event. The editor listens on its own toolbar and page area;
   * call this from a page-wide listener to make shortcuts work everywhere.
   */
  handleKey(e: KeyboardEvent): boolean {
    const t = e.target as HTMLElement | null;
    const textField = !!t && (t.matches?.("input, textarea, select") || (t.isContentEditable && !this.editor.root.contains(t)));
    return this.extensions.handleKey(e, { textField });
  }

  // -- views and compact mode --------------------------------------------------

  /** The id of the view on screen in the pane with the keyboard: `print` until another is chosen. */
  get view(): string {
    return this.viewOf(this.active).id;
  }

  /** The views to offer, in order: Docxtor's and the ones extensions registered with `ctx.views`. */
  views(): ViewMode[] {
    return sortViews(this.extensions.views.list());
  }

  /** The pane with the keyboard (the editor itself until the window is split). */
  get activePane(): Editor {
    return this.active;
  }

  private viewOf(e: Editor): ViewState {
    let st = this.viewStates.get(e);
    if (!st) this.viewStates.set(e, (st = { id: "print", exit: null, classes: [], release: null }));
    return st;
  }

  /** Run `fn` with a pane as the one with the keyboard (to undo what its view did, say), and put the other back. */
  withActive<T>(pane: Editor, fn: () => T): T {
    const before = this.active;
    this.active = pane;
    try {
      return fn();
    } finally {
      this.active = before;
    }
  }

  /**
   * Show the document in a view, in the pane with the keyboard. A view with `layout: "continuous"` lays the text out as one unbroken
   * sheet, `readOnly` locks the text while it is on, and its `enter()` runs last. The print layout (pages) is what every other
   * view returns to.
   */
  setView(id: string): boolean {
    const mode = this.extensions.views.list().find((v) => v.id === id);
    if (!mode && id !== "print") return false;
    const pane = this.active;
    const st = this.viewOf(pane);
    if (id === st.id) return true;
    this.leaveView(pane);
    st.id = id;
    // Print layout is where every view returns to and where the window starts: it adds no class, so coming back is the start again.
    st.classes = id === "print" ? [] : [`view-${id}`, ...(mode?.className ? [mode.className] : [])];
    const host = this.split.hostOf(pane);
    if (st.classes.length) host.classList.add(...st.classes);
    pane.setContinuous(mode?.layout === "continuous" ? { width: mode.width ?? "page", maxWidth: mode.maxWidth, margin: mode.margin, marginLeft: mode.marginLeft, marginTop: mode.marginTop } : null);
    // The view's own reason, on its own pane: leaving the view lets go of it and nothing else.
    if (mode?.readOnly) st.release = pane.lock(`view:${id}`);
    try {
      const exit = mode?.enter?.({ root: host });
      st.exit = typeof exit === "function" ? exit : null;
    } catch (err) {
      console.warn(`Docxtor view ${id}:`, err);
    }
    this.emit("view", id);
    return true;
  }

  private leaveView(pane: Editor) {
    const st = this.viewOf(pane);
    try {
      this.withActive(pane, () => st.exit?.());
    } catch (err) {
      console.warn(`Docxtor view ${st.id}:`, err);
    }
    st.exit = null;
    this.split.hostOf(pane).classList.remove(...st.classes);
    st.classes = [];
    st.release?.();
    st.release = null;
  }

  // -- split window -------------------------------------------------------------

  /** One editor that forwards to the pane with the keyboard, so every tool built on `editor` acts on the pane being used. */
  private paneProxy(): Editor {
    const every = () => this.split.panes();
    return new Proxy(this.primary, {
      get: (_, prop) => {
        const target = this.active;
        const value = Reflect.get(target, prop, target);
        if (typeof value !== "function") return value;
        if (typeof prop === "string" && EVERY_PANE.has(prop)) return (...args: unknown[]) => every().map((e) => (e as never as Record<string, (...a: unknown[]) => unknown>)[prop](...args))[0];
        return (value as (...a: unknown[]) => unknown).bind(target);
      },
      set: (_, prop, value) => {
        if (typeof prop === "string" && SHARED_FIELDS.has(prop)) for (const e of every()) Reflect.set(e, prop, value, e);
        else Reflect.set(this.active, prop, value, this.active);
        return true;
      },
      has: (_, prop) => prop in this.active,
    });
  }

  private splitHost() {
    return {
      surface: this.surface,
      primary: this.primary,
      engine: () => this.primary.engine ?? null,
      leaveView: (e: Editor) => this.leaveView(e),
      enterView: (e: Editor, id: string) => {
        const st = this.viewOf(e);
        const was = st.id;
        st.id = "print";
        if (id !== "print") this.withActive(e, () => this.setView(id));
        else if (was !== "print") this.emit("view", "print");
      },
      viewOf: (e: Editor) => this.viewOf(e).id,
      hooksFor: (second: Editor): Editor["hooks"] => {
        const base = this.primary.hooks;
        // The window's reasons hold in this pane too; its own view's are its own.
        second.windowLocks = this.windowLocks;
        second.applyReadOnly();
        return {
          ...base,
          onLayout: undefined,
          onStory: undefined,
          onBookmark: (name, marker) => openBookmarkPopover(second, name, marker),
          onEquation: (equation, el) => openEquationEditor(second, { edit: equation }, el),
          onRuby: (ruby, el) => openPhoneticGuide(second, ruby, el, (text, lang) => this.extensions.readingFor(text, lang)),
          onCitation: (citation, el) => openCitationPopover(second, citation, el, this.citationPopoverOptions()),
        };
      },
      activate: (e: Editor) => this.activate(e),
      closing: (second: Editor) => {
        this.leaveView(second);
        this.viewStates.delete(second);
      },
      changed: (on: boolean) => {
        this.surface.classList.toggle("split", on);
        this.locksChanged();
        this.emit("split", on);
      },
    };
  }

  /** The pane with the keyboard changed: the toolbar and the menus show that pane's state. */
  private activate(e: Editor) {
    if (this.active === e) return;
    this.active = e;
    this.locksChanged();
    let format: FormatState | null = null;
    try {
      format = e.sel ? e.format() : null;
    } catch {
      format = null;
    }
    this.toolbar?.update(e.pending ? e.formatWithPending() : format);
    this.emit("selection", format);
    this.emit("view", this.viewOf(e).id);
    // The tab list in the navigation pane shows the tabs of the pane that has the keyboard.
    this.emit("layout", { pages: e.pages });
  }

  /** Compact mode: the menu bar folds into one Menu button and the toolbar tightens, leaving more room for the page. */
  get compact(): boolean {
    return this.compactOn;
  }

  setCompact(on: boolean) {
    if (on === this.compactOn) return;
    this.compactOn = on;
    this.root?.classList.toggle("compact", on);
    this.surface.classList.toggle("compact", on);
    this.emit("compact", on);
  }

  undo() {
    this.editor.undo();
  }

  redo() {
    this.editor.redo();
  }

  focus() {
    this.editor.focus();
  }

  /** Open the find and replace bar. */
  find() {
    void this.command("find.open").catch(() => this.findBar.show());
  }

  /** Open the page setup dialog. */
  pageSetup() {
    openPageSetup(this.editor);
  }

  /** Re-read the formatting under the caret into the toolbar (after external changes). */
  private bubbleFrame = 0;

  /** Show, move or hide the selection bubble in the next frame, once however many selection events come before it. */
  private refreshBubble() {
    if (this.bubbleFrame) return;
    this.bubbleFrame = requestAnimationFrame(() => {
      this.bubbleFrame = 0;
      if (this.controls.placeholderSelected()) this.bubble.hide();
      else this.bubble.update();
    });
  }

  refreshToolbar() {
    this.toolbar?.update(this.editor.format());
  }

  get readOnly() {
    return this.editor.readOnly;
  }

  /** What the open document protects (Restrict editing, Mark as final, the parts anyone may edit), or null. */
  get protection(): ProtectionInfo | null {
    return this.protectionTool.info;
  }

  /** The protection in words for a status bar ("Protected: read only", "Marked as final"), or null when there is none. */
  get protectionStatus(): string | null {
    return this.protectionTool.status;
  }

  /**
   * Stop the whole window (every pane) from changing the document for `reason`, until the returned function is called.
   * The document is read-only while any reason is held, and the engine refuses edits from every caller (the assistant,
   * extensions, scripts), not only the typing. `scope: "document"` is for reasons that belong to the open document (the
   * window lock, a mail merge preview): they end when another document is loaded. The default, "host", stays. A view's
   * or a tool's own reason (Reading, Line focus) is taken on a pane with `Editor.lock` and stops that pane only.
   */
  lock(reason: string, opts: { scope?: LockScope } = {}): () => void {
    return this.windowLocks.hold(reason, opts.scope ?? "host");
  }

  /** Why the pane with the keyboard can't be changed, in the words each feature gave. Empty when it can. */
  lockReasons(): string[] {
    return this.editor.lockReasons();
  }

  /** The host's own switch: read-only for the reason "host", on or off. Other reasons are not touched. */
  setReadOnly(on: boolean) {
    if (on === !!this.hostRelease) return;
    if (on) this.hostRelease = this.lock("host");
    else {
      this.hostRelease?.();
      this.hostRelease = null;
    }
  }

  private windowLocksChanged() {
    if (!this.split) return;
    for (const pane of this.split.panes()) pane.applyReadOnly();
    this.locksChanged();
  }

  /** A reason was taken or let go of, or the panes changed: the toolbar and the engine follow. */
  private locksChanged() {
    if (!this.split) return;
    if (this.toolbarEl) this.toolbarEl.hidden = this.active.readOnly || !!this.surface.closest(".focus-mode");
    this.syncAccess();
  }

  /**
   * The engine is shared by the panes, so its access is not a pane's: it is view only when a reason holds for the whole
   * window, or when every pane is locked by its own. While one pane of a split window is in Reading and the other is not,
   * the document stays editable, the Reading pane just can't be typed in.
   */
  private syncAccess() {
    const engine = this.engine;
    if (!engine) return;
    const view = this.windowLocks.size > 0 || this.split.panes().every((p) => p.locks.size > 0);
    try {
      if (engine.access() !== (view ? "view" : "edit")) engine.setAccess(view ? "view" : "edit");
    } catch {
      // An engine that was disposed has nothing to guard.
    }
  }

  // -- events ------------------------------------------------------------------

  /** Subscribe to an event; returns the unsubscribe function. */
  on<K extends keyof DocxtorEditorEvents>(event: K, fn: Listener<DocxtorEditorEvents[K]>): () => void {
    let set = this.listeners.get(event);
    if (!set) this.listeners.set(event, (set = new Set()));
    set.add(fn as Listener<never>);
    return () => set.delete(fn as Listener<never>);
  }

  private emit<K extends keyof DocxtorEditorEvents>(event: K, value: DocxtorEditorEvents[K]) {
    for (const fn of this.listeners.get(event) ?? []) (fn as Listener<DocxtorEditorEvents[K]>)(value);
  }

  /** Remove the editor and release everything it registered. Disposes engines it created. */
  destroy() {
    void this.extensions.dispose();
    for (const release of this.fontReleases.values()) release();
    this.fontReleases.clear();
    this.split.close();
    this.protectionTool.destroy();
    this.editor.destroy();
    this.menuBar?.destroy();
    this.toolbarEl?.remove();
    this.surface.remove();
    this.ownedEngine?.dispose();
    this.ownedEngine = null;
    this.listeners.clear();
    this.root?.classList.remove("docxtor-editor", "compact");
    this.unkeepFocus?.();
    this.unkeepFocus = null;
  }

  // -- what extensions see ---------------------------------------------------------

  private documentApi(): DocumentApi {
    const ed = () => this.editor;
    const engine = () => {
      if (!this.editor.engine) throw new Error(t("No document is open"));
      return this.editor.engine;
    };
    const writable = () => {
      if (this.readOnly) throw new Error(t("The document is read-only"));
      return engine();
    };
    const sub = (off: () => void): Disposable => ({ dispose: off });
    const fe = this;
    return {
      get engine() {
        return fe.engine;
      },
      get name() {
        return fe.engine?.fileName ?? null;
      },
      exec: (ops) => (writable(), this.exec(ops)),
      insertMarkdown: (md) => {
        writable();
        ed().insertMarkdownAtCaret(md);
      },
      tool: (name, input) => {
        writable();
        const out = ed().runTool(name, input);
        ed().refresh({ keepScroll: true });
        return out;
      },
      selection: () => {
        if (!this.editor.engine) return null;
        const r = ed().range();
        return r ? { from: r[0], to: r[1], text: ed().selectedText() } : null;
      },
      selections: () => {
        if (!this.editor.engine) return [];
        return ed()
          .ranges()
          .map((r) => ({ from: r.from, to: r.to, text: ed().engine.textRange(r.from, r.to) }));
      },
      selectRanges: (ranges) => {
        engine();
        const sel = ed().selectRanges(ranges.map((r) => ({ from: r.from, to: r.to ?? r.from }))) ? ed().sel : null;
        if (sel) ed().scrollParaIntoView(sel.focus.p);
      },
      select: (from, to) => {
        engine();
        ed().select({ anchor: from, focus: to ?? from });
        ed().focus();
        ed().scrollParaIntoView(from.p);
      },
      format: () => (this.editor.engine ? ed().format() : null),
      markdown: () => engine().markdown(),
      text: () => engine().text(),
      reveal: (p) => ed().scrollToParagraph(p),
      focus: () => this.focus(),
      onDidChange: (fn) => sub(this.on("change", fn)),
      onDidChangeSelection: (fn) => sub(this.on("selection", fn)),
      onDidOpen: (fn) => sub(this.on("open", fn)),
    };
  }

  private uiApi(): UiApi {
    return {
      notify: (m, o) => toast(m, undefined, o?.action),
      confirm: (message, o = {}) => confirmDialog({ title: o.title ?? message, body: o.title ? message : "", ok: o.confirm, danger: o.danger }),
      prompt: (title, o = {}) => promptDialog({ title, label: o.placeholder ?? title, value: o.value, ok: o.confirm }),
      pick: async (items, o = {}) => {
        const inDoc = this.editor.root.contains(document.activeElement);
        const picked = await quickPick(items, o);
        if (inDoc) this.editor.focus();
        return picked;
      },
    };
  }
}
