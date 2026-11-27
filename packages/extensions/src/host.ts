// The extension host: activates extensions, keeps what they contribute in registries the editor and
// app render from, and removes all of it again when an extension is disabled.
import { formatChord, parseChord, type Chord, type KeyLike } from "./keys.ts";
import type { LanguagePack } from "./l10n.ts";
import type { AtMenuProvider, AtMenuSection, DocBuildingBlock, DropdownPreset, FileProvider, FileRef, LinkPreview, LinkPreviewProvider, PeopleProvider, Person } from "./smart.ts";
import type { ViewMode } from "./views.ts";
import { t } from "./l10n.ts";
import type { Caution, AIProvider, AITool, CitationStyleContribution, Command, CommandOptions, CommandResult, Disposable, DocumentApi, DocumentComparer, Exporter, Extension, ExtensionContext, FieldCall, FieldType, FontContribution, GrammarChecker, ChartPaletteContribution, Hyphenator, ReadingProvider, GrammarIssue, GrammarParagraph, IconContribution, IconPack, Importer, Inspector, InspectorReport, LabelProduct, EmailSender, MailMergeDataSource, PasteConverter, MenuEntry, MenuItem, Panel, PictureSource, SnippetStore, SourceLibrary, SpellingDictionary, LanguageDetector, DetectedLanguage, SpellingDictionaryProvider, SourceProvider, StorageProvider, DocumentThemeContribution, StyleSetContribution, TableDesignContribution, ContentControlContribution, SettingDefinition, Template, TemplateEntry, TemplateStore, Thesaurus, ThesaurusAnswer, ThemeContribution, ToolbarItem, UiApi } from "./types.ts";
import { context } from "./host-context.ts";
import { defaultShortcutSpecs, handleKey, loadShortcutOverrides, setShortcuts, shortcutSpecs, shortcutUsers } from "./host-shortcuts.ts";
import { checkGrammar, detectLanguage, grammarCheckersFor, hyphenationPoints, readingFor, spellingDictionary, thesaurusLookup, thesaurusesFor } from "./host-language.ts";
import { atMenuSections, comparerFor, evaluateField, evaluatesField, fieldType, fieldTypeList, findFiles, findPeople, importerFor, importerForBytes, inspect, linkPreview, rememberPerson, removeInspected } from "./host-services.ts";

/** The id of Docxtor's own icon pack, which every other pack falls back to. */
export const DEFAULT_ICON_PACK = "docxtor";


export interface Entry<T> {
  item: T;
  /** Id of the extension that contributed it. */
  extension: string;
}

/** One kind of contribution. Hosts read `list()` and re-render on `onChange`. */
export class Registry<T> {
  private entries: Entry<T>[] = [];
  private listeners = new Set<() => void>();

  add(item: T, extension: string): Disposable {
    const entry = { item, extension };
    this.entries.push(entry);
    this.changed();
    return {
      dispose: () => {
        const i = this.entries.indexOf(entry);
        if (i < 0) return;
        this.entries.splice(i, 1);
        this.changed();
      },
    };
  }

  list(): T[] {
    return this.entries.map((e) => e.item);
  }

  all(): readonly Entry<T>[] {
    return this.entries;
  }

  onChange(fn: () => void): Disposable {
    this.listeners.add(fn);
    return { dispose: () => this.listeners.delete(fn) };
  }

  private changed() {
    for (const fn of this.listeners) fn();
  }
}

/** What a host supplies to every extension. */
/** A key one command's own choice took from another command (`chord` is its normalised form, `spec` how it was written). */
export interface TakenKey {
  chord: string;
  spec: string;
  from: string;
  by: string;
}

export interface HostServices {
  document: DocumentApi;
  ui: UiApi;
  /** Where extension state is kept (default: in memory for this page). */
  state?: { get(extension: string, key: string): unknown; set(extension: string, key: string, value: unknown): void };
  /** An extension failed to activate or a command threw. Default: console.error. */
  onError?: (extension: string, error: unknown) => void;
  /** Whether `Mod` means ⌘ (default: detected from the platform). */
  mac?: boolean;
}

export interface AddOptions {
  /** Start disabled (e.g. the user turned it off earlier). */
  enabled?: boolean;
  /** Part of the host itself; listed as built in. */
  builtin?: boolean;
  /** The host doesn't work without it, so it can't be disabled. */
  required?: boolean;
}

export interface ExtensionInfo {
  id: string;
  name: string;
  description?: string;
  version?: string;
  publisher?: string;
  enabled: boolean;
  builtin: boolean;
  required: boolean;
  /** Set when activation failed. */
  error?: string;
  /** False for an extension that waits to be turned on. */
  enabledByDefault: boolean;
  /** What the extension sends away, in a sentence (see `Extension.privacy`). */
  privacy?: string;
}

export interface Slot {
  ext: Extension;
  opts: AddOptions;
  enabled: boolean;
  active: boolean;
  subscriptions: Disposable[];
  error?: string;
}



const isMac = () => typeof navigator !== "undefined" && /mac|iphone|ipad/i.test(navigator.platform);


export class ExtensionHost {
  readonly commands = new Registry<Command>();
  readonly toolbar = new Registry<ToolbarItem>();
  readonly menus = new Registry<MenuItem>();
  readonly panels = new Registry<Panel>();
  readonly exporters = new Registry<Exporter>();
  readonly importers = new Registry<Importer>();
  readonly templates = new Registry<Template>();
  readonly templateStores = new Registry<TemplateStore>();
  readonly settings = new Registry<SettingDefinition>();
  readonly themes = new Registry<ThemeContribution>();
  readonly aiProviders = new Registry<AIProvider>();
  readonly aiTools = new Registry<AITool>();
  readonly storage = new Registry<StorageProvider>();
  readonly fonts = new Registry<FontContribution>();
  readonly icons = new Registry<IconContribution>();
  readonly spellingDictionaries = new Registry<SpellingDictionaryProvider>();
  readonly grammarCheckers = new Registry<GrammarChecker>();
  readonly hyphenators = new Registry<Hyphenator>();
  readonly languageDetectors = new Registry<LanguageDetector>();
  readonly readingProviders = new Registry<ReadingProvider>();
  readonly thesauruses = new Registry<Thesaurus>();
  readonly inspectors = new Registry<Inspector>();
  readonly comparers = new Registry<DocumentComparer>();
  readonly iconPacks = new Registry<IconPack>();
  readonly fieldTypes = new Registry<FieldType>();
  readonly tableDesigns = new Registry<TableDesignContribution>();
  readonly chartPalettes = new Registry<ChartPaletteContribution>();  readonly contentControls = new Registry<ContentControlContribution>();
  readonly atMenuProviders = new Registry<AtMenuProvider>();
  readonly peopleProviders = new Registry<PeopleProvider>();
  readonly fileProviders = new Registry<FileProvider>();
  readonly dropdownPresets = new Registry<DropdownPreset>();
  readonly docBuildingBlocks = new Registry<DocBuildingBlock>();
  readonly linkPreviewProviders = new Registry<LinkPreviewProvider>();
  /** Warnings for risky actions, by command (or dialog button) id; see `ask` and `execute`. */
  readonly cautions = new Registry<Caution>();
  readonly documentThemes = new Registry<DocumentThemeContribution>();
  readonly styleSets = new Registry<StyleSetContribution>();
  readonly citationStyles = new Registry<CitationStyleContribution>();
  readonly citationSources = new Registry<SourceProvider>();
  readonly pictureSources = new Registry<PictureSource>();
  /** The ways to show the document (print layout, pageless, reading, outline, and an extension's own). */
  readonly views = new Registry<ViewMode>();
  readonly sourceLibraries = new Registry<SourceLibrary>();
  readonly snippetStores = new Registry<SnippetStore>();
  /** Where lists of recipients come from (Mailings, Select recipients), and the label sheets the Labels dialog offers. */
  readonly mailMergeSources = new Registry<MailMergeDataSource>();
  readonly mailSenders = new Registry<EmailSender>();
  readonly labelProducts = new Registry<LabelProduct>();
  /** Readers for clipboard formats; the editor tries them from the highest priority down. */
  readonly pasteConverters = new Registry<PasteConverter>();

  readonly mac: boolean;
  private slots = new Map<string, Slot>();
  private listeners = new Set<() => void>();
  chords = new WeakMap<Command, Chord[]>();
  memory = new Map<string, unknown>();
  services: HostServices;
  /** Icon id to SVG, built on first use and dropped whenever an icon is added or removed. */
  private iconIndex: Map<string, string> | null = null;
  private iconPackId = DEFAULT_ICON_PACK;
  private iconListeners = new Set<() => void>();

  constructor(services: HostServices) {
    this.services = services;
    this.mac = services.mac ?? isMac();
    this.icons.onChange(() => this.iconsChanged());
    this.iconPacks.onChange(() => this.iconsChanged());
  }

  private iconsChanged() {
    this.iconIndex = null;
    // Told once for a run of changes (an extension that adds 40 icons as it starts), in a microtask: each listener draws every icon it shows
    // again, and at startup that was 5,600 icons parsed from their markup, most of them thrown away at once.
    if (this.iconsQueued) return;
    this.iconsQueued = true;
    queueMicrotask(() => {
      this.iconsQueued = false;
      for (const fn of this.iconListeners) fn();
    });
  }
  private iconsQueued = false;

  /** Runs when an icon, a pack or the chosen pack changes, so what shows icons can draw them again: once, in a microtask, for a run of changes. */
  onDidChangeIcons(fn: () => void): Disposable {
    this.iconListeners.add(fn);
    return { dispose: () => this.iconListeners.delete(fn) };
  }

  /** The id of the icon pack that was chosen (`docxtor` until another is). It is kept even while that pack's extension is off. */
  get iconPack(): string {
    return this.iconPackId;
  }

  /** Choose an icon pack by id. A pack that isn't there (yet) shows Docxtor's icons until it is. */
  setIconPack(id: string) {
    const next = id || DEFAULT_ICON_PACK;
    if (next === this.iconPackId) return;
    this.iconPackId = next;
    this.iconsChanged();
  }

  /** The packs to offer: Docxtor's first, then the others in the order they were added. */
  listIconPacks(): { id: string; label: string }[] {
    const out: { id: string; label: string }[] = [];
    for (const p of this.iconPacks.list()) if (!out.some((o) => o.id === p.id)) out.push({ id: p.id, label: p.label });
    return out.sort((a, b) => Number(b.id === DEFAULT_ICON_PACK) - Number(a.id === DEFAULT_ICON_PACK));
  }

  /** The pack in use: the chosen one, or Docxtor's when the chosen one isn't available. */
  get activeIconPack(): string {
    return this.iconPacks.list().some((p) => p.id === this.iconPackId) ? this.iconPackId : DEFAULT_ICON_PACK;
  }

  // -- extensions ------------------------------------------------------------

  /** Add an extension and activate it (unless `enabled: false`). Resolves once it's active. */
  async add(ext: Extension, opts: AddOptions = {}): Promise<void> {
    if (!ext?.id || typeof ext.activate !== "function") throw new Error("An extension needs an id and an activate() function");
    if (this.slots.has(ext.id)) throw new Error(`Extension ${ext.id} is already added`);
    const slot: Slot = { ext, opts, enabled: opts.required || opts.enabled !== false, active: false, subscriptions: [] };
    this.slots.set(ext.id, slot);
    if (slot.enabled) await this.activate(slot);
    this.changed();
  }

  list(): ExtensionInfo[] {
    return [...this.slots.values()].map((s) => ({
      id: s.ext.id,
      name: s.ext.name,
      description: s.ext.description,
      version: s.ext.version,
      publisher: s.ext.publisher,
      enabled: s.enabled,
      enabledByDefault: s.ext.enabledByDefault !== false,
      privacy: s.ext.privacy,
      builtin: !!s.opts.builtin,
      required: !!s.opts.required,
      error: s.error,
    }));
  }

  has(id: string): boolean {
    return this.slots.has(id);
  }

  async enable(id: string): Promise<void> {
    const s = this.slot(id);
    if (s.enabled) return;
    s.enabled = true;
    await this.activate(s);
    this.changed();
  }

  async disable(id: string): Promise<void> {
    const s = this.slot(id);
    if (!s.enabled || s.opts.required) return;
    s.enabled = false;
    await this.deactivate(s);
    this.changed();
  }

  /** Disable and forget an extension. */
  async remove(id: string): Promise<void> {
    const s = this.slots.get(id);
    if (!s) return;
    await this.deactivate(s);
    this.slots.delete(id);
    this.changed();
  }

  /** The extension list changed (added, enabled, disabled, removed). */
  onChange(fn: () => void): Disposable {
    this.listeners.add(fn);
    return { dispose: () => this.listeners.delete(fn) };
  }

  /** Deactivate everything. */
  async dispose(): Promise<void> {
    for (const s of [...this.slots.values()].reverse()) await this.deactivate(s);
    this.slots.clear();
    this.listeners.clear();
  }

  private slot(id: string): Slot {
    const s = this.slots.get(id);
    if (!s) throw new Error(`No extension ${id}`);
    return s;
  }

  private changed() {
    for (const fn of this.listeners) fn();
  }

  report(extension: string, err: unknown) {
    if (this.services.onError) this.services.onError(extension, err);
    else console.error(`[${extension}]`, err);
  }

  private async activate(s: Slot) {
    if (s.active) return;
    s.error = undefined;
    s.active = true;
    try {
      await s.ext.activate(this.context(s));
    } catch (err) {
      s.error = err instanceof Error ? err.message : String(err);
      this.report(s.ext.id, err);
      await this.deactivate(s);
    }
  }

  private async deactivate(s: Slot) {
    if (!s.active) return;
    s.active = false;
    for (const d of s.subscriptions.splice(0).reverse()) {
      try {
        d.dispose();
      } catch (err) {
        this.report(s.ext.id, err);
      }
    }
    try {
      await s.ext.deactivate?.();
    } catch (err) {
      this.report(s.ext.id, err);
    }
  }

  private context(s: Slot): ExtensionContext { return context(this, s); }

  // -- mail merge ------------------------------------------------------------

  /** The label products to offer, one per id (the latest registered wins, in the place of the first), grouped by vendor in the order they were added. */
  labelProductGroups(): { vendor: string; products: LabelProduct[] }[] {
    const byId = new Map<string, LabelProduct>();
    for (const p of this.labelProducts.list()) byId.set(p.id, p);
    const groups = new Map<string, LabelProduct[]>();
    for (const p of byId.values()) groups.set(p.vendor, [...(groups.get(p.vendor) ?? []), p]);
    return [...groups].map(([vendor, products]) => ({ vendor, products }));
  }

  // -- citations -------------------------------------------------------------

  /** The citation styles to offer: one per id (the latest registered wins), in the order they were added. */
  citationStyleList(): CitationStyleContribution[] {
    const byId = new Map<string, CitationStyleContribution>();
    for (const c of this.citationStyles.list()) {
      byId.delete(c.id);
      byId.set(c.id, c);
    }
    return [...byId.values()];
  }

  /** The CSL XML of a registered style, or null for a style the engine formats itself (or an unknown id). */
  async citationStyleCsl(id: string): Promise<string | null> {
    const c = this.citationStyleList().find((x) => x.id === id);
    if (!c?.csl) return null;
    return typeof c.csl === "function" ? await c.csl() : c.csl;
  }

  // -- templates ------------------------------------------------------------

  /**
   * Every template New from a template can offer: the ones extensions registered (built in), then the ones each
   * template store keeps. A store that fails is reported and skipped.
   */
  async templateEntries(): Promise<TemplateEntry[]> {
    const out: TemplateEntry[] = this.templates.all().map(({ item, extension }) => ({
      id: item.id,
      name: item.name,
      description: item.description,
      fileName: item.fileName,
      source: extension.startsWith("docxtor.") ? "" : (this.list().find((x) => x.id === extension)?.name ?? extension),
      create: () => item.create(),
    }));
    for (const entry of this.templateStores.all()) {
      const store = entry.item;
      try {
        for (const info of await store.list()) {
          out.push({
            id: `${store.id}:${info.id}`,
            name: info.name,
            description: info.description,
            fileName: `${info.name}.docx`,
            source: store.label,
            remove: async () => void (await store.remove(info.id)),
            create: async () => ({ docx: await store.read(info.id) }),
          });
        }
      } catch (err) {
        this.report(entry.extension, err);
      }
    }
    return out;
  }

  // -- commands --------------------------------------------------------------

  registerCommand(id: string, run: (...args: unknown[]) => unknown, opts: CommandOptions, extension: string): Disposable {
    if (!id || typeof run !== "function") throw new Error("A command needs an id and a function");
    if (!opts?.title) throw new Error(`Command ${id} needs a title`);
    if (this.command(id)) throw new Error(`Command ${id} is already registered`);
    const cmd: Command = { ...opts, id, run, extension };
    const specs = cmd.keybinding === undefined ? [] : Array.isArray(cmd.keybinding) ? cmd.keybinding : [cmd.keybinding];
    this.chords.set(cmd, specs.map((k) => parseChord(k, this.mac)));
    return this.commands.add(cmd, extension);
  }

  command(id: string): Command | undefined {
    return this.commands.list().find((c) => c.id === id);
  }

  /** The warning for a command or a dialog's button, if there is one (the last registered wins). */
  caution(id: string): Caution | undefined {
    return [...this.cautions.list()].reverse().find((c) => c.id === id);
  }

  /**
   * Ask before a risky action: resolves true when there is nothing to ask (no `ask` caution for `id`) or the person
   * confirms. Commands are asked about by `execute`; a dialog's button that isn't a command calls this itself.
   */
  async ask(id: string, ...args: unknown[]): Promise<boolean> {
    const c = this.caution(id);
    if (!c || c.kind !== "ask") return true;
    const say = (v: Caution["title"] | undefined) => (typeof v === "function" ? v(...args) : v ?? "");
    const impact = [say(c.impact), c.permanent ? t("This can't be undone.") : ""].filter(Boolean).join(" ");
    return this.services.ui.confirm(impact, { title: say(c.title), confirm: say(c.confirm) || t("Continue"), danger: c.danger !== false });
  }

  /**
   * Run a command: the only door to one. Rejects if it doesn't exist; errors it throws are reported and rethrown. It
   * answers `unavailable` when the command's `when` says no (nothing is asked or run), `declined` when its caution asks
   * and the person says no, and otherwise `done` with what the command returned. A command with an undo caution offers
   * Undo after it, unless it returns false (nothing was done) or the document's revision did not change; the offer only
   * works while that step is still the last one.
   */
  async execute<T = unknown>(id: string, ...args: unknown[]): Promise<CommandResult<T>> {
    const cmd = this.command(id);
    if (!cmd) throw new Error(`No command ${id}`);
    if (!this.enabled(cmd)) return { status: "unavailable" };
    // Only a command that asks waits for the answer: the rest run straight away, as a shortcut expects.
    if (this.caution(id)?.kind === "ask" && !(await this.ask(id, ...args))) return { status: "declined" };
    try {
      const before = this.revision();
      const value = (await cmd.run(...args)) as T;
      const c = this.caution(id);
      const after = this.revision();
      if (c?.kind === "undo" && (value as unknown) !== false && (before === undefined || after !== before)) {
        const title = typeof c.title === "function" ? c.title(...args) : c.title;
        this.services.ui.notify(title, {
          action: {
            label: t("Undo"),
            run: () => {
              // After a later edit, Undo would take back something else.
              if (after !== undefined && this.revision() !== after) return;
              void this.execute("edit.undo").catch(() => undefined);
            },
          },
        });
      }
      return { status: "done", value };
    } catch (err) {
      this.report(cmd.extension, err);
      throw err;
    }
  }

  /** The open document's revision, or undefined when there is no engine to ask. */
  private revision(): number | undefined {
    try {
      return this.services.document.engine?.revision;
    } catch {
      return undefined;
    }
  }

  /** Whether a command is available right now. */
  enabled(cmd: Command): boolean {
    try {
      return cmd.when ? cmd.when() : true;
    } catch {
      return false;
    }
  }

  /**
   * A menu's entries, in groups (separators go between them). Commands that aren't available right
   * now are included but not `enabled`, so menus keep their shape; items for missing commands and
   * empty submenus are left out.
   */
  /**
   * Whether `menu(id)` would list anything: an item whose command is registered, or a submenu that has one. Without building the entries (their
   * shortcuts, checks and icons): the menu bar asks this of every top menu each time a command or a menu item is registered, which at startup
   * built every menu several times over (220 ms, and the icons' markup each time).
   */
  hasMenu(id: string, seen = new Set<string>()): boolean {
    seen.add(id);
    for (const m of this.menus.list()) {
      if (m.menu !== id) continue;
      if (m.submenu ? !seen.has(m.submenu) && this.hasMenu(m.submenu, new Set(seen)) : !!m.command && !!this.command(m.command)) return true;
    }
    return false;
  }

  menu(id: string, seen = new Set<string>()): MenuEntry[][] {
    seen.add(id);
    const all = this.menus.list().filter((m) => m.menu === id);
    const groups = new Map<string, { m: MenuItem; i: number }[]>();
    all.forEach((m, i) => {
      const g = groups.get(m.group ?? "") ?? [];
      g.push({ m, i });
      groups.set(m.group ?? "", g);
    });
    const names = [...groups.keys()].sort((a, b) => (a === "" ? 1 : b === "" ? -1 : a.localeCompare(b)));
    const out: MenuEntry[][] = [];
    for (const name of names) {
      const entries: MenuEntry[] = [];
      for (const { m } of groups.get(name)!.sort((a, b) => (a.m.order ?? 0) - (b.m.order ?? 0) || a.i - b.i)) {
        if (m.submenu) {
          if (seen.has(m.submenu) || !this.menu(m.submenu, new Set(seen)).length) continue;
          entries.push({ item: m, title: m.title ?? m.submenu, shortcut: "", enabled: true, checked: false, icon: this.icon(m.icon) });
          continue;
        }
        const command = m.command ? this.command(m.command) : undefined;
        if (!command) continue;
        let checked = false;
        try {
          checked = !!command.toggled?.();
        } catch {
          /* a broken check shows no mark */
        }
        entries.push({ item: m, title: m.title ?? command.title, shortcut: this.shortcut(command.id), enabled: this.enabled(command), checked, icon: this.icon(m.icon ?? command.icon), command });
      }
      if (entries.length) out.push(entries);
    }
    return out;
  }

  /**
   * The SVG for an icon id, or the markup itself if it is SVG. Docxtor's pack is the base, the chosen pack
   * draws over it (so what it leaves out stays Docxtor's), and icons registered on their own draw over both.
   * Within each, the most recently added wins.
   */
  icon(ref: string | undefined): string | undefined {
    if (!ref) return undefined;
    if (ref.trim().startsWith("<svg")) return ref;
    if (!this.iconIndex) {
      const index = new Map<string, string>();
      const packs = this.iconPacks.list();
      const chosen = this.activeIconPack;
      for (const id of chosen === DEFAULT_ICON_PACK ? [DEFAULT_ICON_PACK] : [DEFAULT_ICON_PACK, chosen]) {
        for (const p of packs) if (p.id === id) for (const i of p.icons) index.set(i.id, i.svg);
      }
      for (const i of this.icons.list()) index.set(i.id, i.svg);
      this.iconIndex = index;
    }
    return this.iconIndex.get(ref);
  }

  /** Run a menu entry's command (with its arguments). */
  run(entry: MenuEntry): Promise<unknown> {
    if (!entry.command) return Promise.resolve();
    return this.execute(entry.command.id, ...(entry.item.args ?? []));
  }

  /** Commands to list in the command palette, by category then title. */
  paletteCommands(): Command[] {
    return this.commands
      .list()
      .filter((c) => c.palette !== false && this.enabled(c))
      .sort((a, b) => (a.category ?? "").localeCompare(b.category ?? "") || a.title.localeCompare(b.title));
  }

  /** The first shortcut of a command, written for this platform ("⌘⇧P"), or "". */
  shortcut(id: string): string {
    const cmd = this.command(id);
    if (!cmd) return "";
    const own = this.shortcutOverrides[id];
    const spec = own ? own[0] : (Array.isArray(cmd.keybinding) ? cmd.keybinding[0] : cmd.keybinding) ?? cmd.shortcutHint;
    return spec ? formatChord(spec, this.mac) : "";
  }

  // -- the person's own shortcuts ---------------------------------------------

  /** The shortcuts the person chose, by command id. An empty list means the command has none. Only commands that were changed are here. */
  shortcutOverrides: Record<string, string[]> = {};
  overrideChords = new Map<string, Chord[]>();
  shortcutListeners = new Set<() => void>();
  /** Keys a command came with that the person took away and gave to nothing: they do nothing, rather than what the browser does with them. */
  released: Chord[] = [];
  /** Keys a command's own choice took from another command, in the order they were taken, so that giving the key up gives it back. */
  taken: TakenKey[] = [];

  /** Which keys one command took from another, to be saved with `shortcutOverrides` so that resetting still gives them back after a reload. */
  get shortcutTaken(): TakenKey[] {
    return this.taken.map((t) => ({ ...t }));
  }


  /** The shortcuts a command has now, as written in `keybinding` (`Mod+Shift+K`): the person's own, else the command's. */
  shortcutSpecs(id: string): string[] { return shortcutSpecs(this, id); }

  /** The shortcuts the command came with. A command whose key the browser handles itself (Cut, Copy) reports it as a hint here. */
  defaultShortcutSpecs(id: string): string[] { return defaultShortcutSpecs(this, id); }

  /**
   * Give a command shortcuts of the person's own, replacing its defaults (an empty list takes them all away), or
   * `null` to go back to the defaults. A shortcut another command uses is taken from that command, so each key does
   * one thing. Throws for a shortcut that cannot be parsed.
   */
  setShortcuts(id: string, specs: string[] | null) { return setShortcuts(this, id, specs); }


  /**
   * Replace every shortcut the person chose (what was saved earlier), and the record of which command took which key from
   * which (`shortcutTaken`). Unreadable shortcuts are skipped, and so is a record the saved shortcuts no longer match.
   */
  loadShortcutOverrides(saved: Record<string, string[]> | null | undefined, taken?: TakenKey[] | null) { return loadShortcutOverrides(this, saved, taken); }

  /** The commands that use this shortcut now (the person's own or default), so a dialog can warn before taking it. */
  shortcutUsers(spec: string, except?: string): Command[] { return shortcutUsers(this, spec, except); }

  onShortcutsChange(fn: () => void): Disposable {
    this.shortcutListeners.add(fn);
    return { dispose: () => void this.shortcutListeners.delete(fn) };
  }


  /**
   * Run the command bound to this key event, if any. Skips events something else already handled
   * (the editor's own shortcuts). In a text field (`textField: true`), keys without ⌘/Ctrl/Alt and
   * the field's own editing shortcuts (⌘Z, ⌘A, ⌘C…) stay with the field. Returns true when a command ran.
   */
  handleKey(e: KeyLike & { defaultPrevented?: boolean; isComposing?: boolean; preventDefault(): void }, opts: { textField?: boolean } = {}): boolean { return handleKey(this, e, opts); }

  // -- proofing ----------------------------------------------------------------

  /**
   * A Hunspell dictionary for a language. Providers that cover the language are asked latest first; the
   * first to return one wins. A provider that throws is skipped.
   */
  async spellingDictionary(lang: string): Promise<SpellingDictionary | null> { return spellingDictionary(this, lang); }

  /**
   * The language of a piece of text, for Detect language automatically. Detectors are asked latest first; the first
   * answer at least `sure` confident wins. A detector that throws is reported and skipped. Null when none is sure.
   */
  async detectLanguage(text: string, sure = 0.6): Promise<DetectedLanguage | null> { return detectLanguage(this, text, sure); }

  /**
   * The lines of the @ menu for what was typed after the @, as sections. Providers are asked in order, a provider that
   * throws is reported and skipped, and lines with the same heading share a section. Empty sections are left out.
   */
  async atMenuSections(query: string): Promise<AtMenuSection[]> { return atMenuSections(this, query); }

  /** People from every provider whose name or email fits `query`, the first mention of each person kept (people count as the same when their email, or without one their name, is). */
  async findPeople(query: string): Promise<Person[]> { return findPeople(this, query); }

  /** Hand a person typed in by hand to the providers that keep people. */
  async rememberPerson(person: Person): Promise<void> { return rememberPerson(this, person); }

  /** Files from every provider whose name fits `query`, each address once. */
  async findFiles(query: string): Promise<FileRef[]> { return findFiles(this, query); }

  /** What the providers know about a link's page: the latest registered that has an answer wins. Null when none does. */
  async linkPreview(url: string, signal?: AbortSignal): Promise<LinkPreview | null> { return linkPreview(this, url, signal); }

  /**
   * Where a word can be hyphenated in a language: the offsets from the latest registered hyphenator that knows the
   * language and has an opinion. A hyphenator that throws is reported and skipped. No opinion at all is an empty list.
   */
  async hyphenationPoints(word: string, lang: string): Promise<number[]> { return hyphenationPoints(this, word, lang); }

  /**
   * The reading of East Asian text in a language: the answer of the latest registered provider that knows the language
   * and has one, with the id of the provider. A provider that throws is reported and skipped. No answer is null.
   */
  async readingFor(text: string, lang: string): Promise<{ text: string; source: string } | null> { return readingFor(this, text, lang); }

  // -- thesaurus -------------------------------------------------------------

  /** The thesauruses that can serve a language, in the order they were registered. */
  thesaurusesFor(lang: string): Thesaurus[] { return thesaurusesFor(this, lang); }

  /**
   * Ask every thesaurus that serves the language about a word. The answers are listed by thesaurus, in the order they
   * were registered. Synonyms are tidied (blank ones, repeats and the word itself are dropped) and a thesaurus with
   * nothing left for the word is left out. One that fails is reported and left out; the others still count.
   */
  async thesaurusLookup(word: string, lang: string): Promise<ThesaurusAnswer> { return thesaurusLookup(this, word, lang); }

  // -- inspect document ------------------------------------------------------


  /** Run every inspector. One that fails is reported, and listed with its error and a count of 0. */
  async inspect(signal: AbortSignal = new AbortController().signal): Promise<InspectorReport[]> { return inspect(this, signal); }

  /**
   * Take what some inspectors found out of the document. Every edit they ask for (`ctx.exec`) is applied together as
   * one undoable step, after all of them have run. Returns the ids of the inspectors that finished; one that fails is
   * reported and skipped, and its edits are not applied.
   */
  async removeInspected(ids: string[], signal: AbortSignal = new AbortController().signal): Promise<string[]> { return removeInspected(this, ids, signal); }

  // -- compare and combine ---------------------------------------------------

  /** The latest registered comparer that can do the job (`redline` and `combine` are optional parts of a comparer). */
  comparerFor(job: "compare" | "redline" | "combine"): DocumentComparer | undefined { return comparerFor(this, job); }

  // -- fields ----------------------------------------------------------------

  /** The field type with this name (the latest registered wins), or undefined. */
  fieldType(name: string): FieldType | undefined { return fieldType(this, name); }

  /** Every field type, one per name (the latest registered wins), in the order of the dialog's groups and then of registration. */
  fieldTypeList(): FieldType[] { return fieldTypeList(this); }

  /**
   * Work out a field with an extension's evaluator. Resolves null when no extension evaluates that field name, when the
   * evaluator has nothing to say, or when it fails (the failure is reported and the result stays as it is).
   */
  async evaluateField(call: FieldCall): Promise<string | null> { return evaluateField(this, call); }

  /** Whether an extension evaluates fields with this name. */
  evaluatesField(name: string): boolean { return evaluatesField(this, name); }

  /** The grammar checkers that can check a language, in the order they were registered. */
  grammarCheckersFor(lang: string): GrammarChecker[] { return grammarCheckersFor(this, lang); }

  /**
   * Ask every grammar checker about the paragraphs it can check and merge what comes back: exact duplicates
   * are dropped and the rest is in reading order. A checker that fails, or takes longer than half a minute,
   * is reported and left out; the others still count. Issues that don't fit the text they name are dropped.
   * `partial` is called with what is known each time another checker has answered, so a slow checker (one
   * on a server) does not hold back a fast one.
   */
  async checkGrammar(paragraphs: GrammarParagraph[], partial?: (issues: GrammarIssue[]) => void): Promise<GrammarIssue[]> { return checkGrammar(this, paragraphs, partial); }

  /** Language packs from enabled extensions. */
  languages(): LanguagePack[] {
    return [...this.slots.values()].filter((s) => s.enabled).flatMap((s) => s.ext.contributes?.languages ?? []);
  }

  // -- formats ---------------------------------------------------------------

  exporter(id: string): Exporter | undefined {
    return this.exporters.list().find((x) => x.id === id);
  }

  /**
   * The importer for a file, by extension (the latest registered wins). With the file's `bytes`, importers that can
   * `detect` their kind of file are asked: among those that share the extension the first that says yes wins, and when
   * the name matches none (or the bytes say it isn't what the name claims) the first importer that detects the bytes.
   */
  importerFor(name: string, bytes?: Uint8Array): Importer | undefined { return importerFor(this, name, bytes); }

  /** The importer whose `detect` recognises these bytes, whatever the file is called. */
  importerForBytes(bytes: Uint8Array): Importer | undefined { return importerForBytes(this, bytes); }
}
