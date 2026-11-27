// The app's window: the top bar, the status bar, the language, theme and icon pickers, global key and drop handling.
import type { App } from "./main.ts";
import { holdSession } from "./files/recovery";
import { storage, store } from "./storage";
import { EXT_KEY, ICON_PACK_KEY, LANG_KEY, THEME_KEY } from "./app-shared";
import { DocxtorEditor, I, PSEUDO_LOCALE, ZoomControl, confirmDialog, currentLocale, h, pickLanguage, quickPick, setLanguage, svg, t, textDirection, tn, toast } from "@docxtor/editor";
import { baseName, recents, uniqueName } from "./files/fs";
import { AccessibilityPanel } from "./ui/accessibility";
import { AIPanel } from "./ui/ai-panel";
import { FilesPanel } from "./ui/files-panel";
import { HistoryPanel } from "./ui/history-panel";
import type { Extension } from "@docxtor/editor";
import { aiCommands, appCommands, googleFonts, navigator, spellingDictionaries, templates, zotero } from "./builtins";
import { cautions as appCautions } from "./cautions";
import { aiProviders } from "./ai/providers";
import { dictionary } from "./ui/dictionary";
import { stylesPanels } from "./ui/styles-panels";
import { bibtexSource, citations, cslStyles, doiSource, userStyles } from "./ui/citations";
import { languageTool } from "./languagetool";
import { datamuse } from "./thesaurus-datamuse";
import { localSnippets, localSourceLibrary, localTemplates } from "./stores";
import { PREFS_KEY, appSettings } from "./settings";
import { helpLinks } from "./help";
import { languages } from "./languages";
import { extensions as installed } from "./extensions";
import { Sidebar } from "./ui/sidebar";
import { SideBySide } from "./side-by-side";
import { registerOffline } from "./offline";
import { openExtensions } from "./ui/extensions-dialog";
import type { SaveState } from "./app-shared";
import { startElement } from "./start-screen";
import { isStale } from "./files/db";

export function mount(app: App, root: HTMLElement) {
  app.root = root;
  // Pages that are open say so, so a copy kept for a page that closed badly can be told from one in use.
  holdSession();
  const saved = storage(EXT_KEY, { disabled: [] as string[], enabled: [] as string[] });
  const disabled = saved.disabled;
  chooseLanguage(app, disabled);
  // index.html already loads the fonts (preconnected, before first paint).
  app.fe = new DocxtorEditor({
    ask: (p) => app.ai.askAbout(p),
    fonts: false,
    menuBar: true,
    // The top bar's Assistant button opens it; the selection bubble and right-click menu ask about text.
    toolbarAsk: false,
    // An address of this app (a file chip's document) opens here, by the app's own routing; the editor opens the rest.
    openAddress: (url) => {
      const here = `${location.origin}${location.pathname}`;
      const hash = url.startsWith("#/") ? url : url.startsWith(`${here}#/`) ? url.slice(here.length) : null;
      if (hash === null) return false;
      location.hash = hash;
      return true;
    },
    // The result of a mail merge and a sheet of labels open as documents of their own, saved in the Library.
    openNewDocument: async (bytes, name) => {
      const final = await uniqueName(app.files.library, "", name);
      await app.files.library.write(final, bytes);
      await app.openRef({ backend: app.files.library, path: final });
    },
    disabledExtensions: disabled,
    extensionState: {
      get: (id, key) => storage<Record<string, unknown>>(`docxtor.ext.${id}`, {})[key],
      set: (id, key, value) => store(`docxtor.ext.${id}`, { ...storage<Record<string, unknown>>(`docxtor.ext.${id}`, {}), [key]: value }),
    },
  });
  app.fe.useRoot(root);
  app.editor = app.fe.editor;
  const host = app.fe.extensions;
  app.a11y = new AccessibilityPanel(app);
  app.ai = new AIPanel(app.editor, host);
  app.files = new FilesPanel({
    ask: (id, ...args) => host.ask(id, ...args),
    current: () => app.ref,
    open: (b, p) => app.openRef({ backend: b, path: p }),
    create: async (b, dir) => void (await app.createIn(b, dir)),
    importFiles: (b, dir, files) => app.importFiles(b, dir, files),
    renamed: (b, from, to) => app.onRenamed(b, from, to),
    removed: (b, p) => app.onRemoved(b, p),
    download: async (b, p) => app.download(baseName(p), await b.read(p)),
    providers: () => host.storage.list(),
    canOpen: (name) => app.fe.canOpen(name),
    accept: () => app.accept(),
    togglePin: (b, p) => app.togglePinOf(b, p),
    isPinned: async (b, p) => !!(await recents()).find((r) => r.key === `${b.id}:${p}`)?.pinned,
    trash: {
      backendFor: (id) => app.backendFor(id),
      restored: (b, p) => app.onRestored(b, p),
    },
  });
  app.history = new HistoryPanel({
    ask: (id, ...args) => host.ask(id, ...args),
    docKey: () => app.docKey(),
    diffAgainstCurrent: (bytes) => app.diff(bytes, app.engine?.savePlain() ?? null),
    diffBetween: (a, b) => app.diff(a, b),
    restore: (v, bytes) => app.restoreVersion(v, bytes),
    nameCurrent: (label) => app.checkpoint("named", label, true),
    download: (v, bytes) => app.download(`${app.stem()} — ${new Date(v.at).toLocaleString().replace(/[/:]/g, "-")}.docx`, bytes),
  });
  // The app's features are extensions too; the navigator and the app commands can't be turned off.
  const off = new Set(disabled);
  // An extension that starts off (one that sends text away) is on only once someone has turned it on.
  const turnedOn = new Set(saved.enabled ?? []);
  const add = (ext: Extension, required = false) =>
    void app.fe.use(ext, { builtin: ext.id.startsWith("docxtor."), required, enabled: ext.enabledByDefault === false ? turnedOn.has(ext.id) : !off.has(ext.id) }).catch((e) => console.warn(e));
  add(navigator(app), true);
  add(appCommands(app), true);
  add(appCautions(), true);
  add(aiProviders);
  add(aiCommands);
  add(templates);
  add(googleFonts);
  add(dictionary(app));
  add(stylesPanels(app));
  add(citations(app));
  add(spellingDictionaries);
  add(languageTool);
  add(datamuse);
  add(doiSource());
  add(bibtexSource());
  add(cslStyles());
  add(userStyles());
  add(zotero());
  add(localSnippets());
  add(localSourceLibrary());
  add(localTemplates());
  add(appSettings(app));
  add(helpLinks(app));
  for (const ext of languages) add(ext);
  for (const ext of installed) add(ext);
  app.sidebar = new Sidebar(host);
  host.storage.onChange(() => void app.files.refresh());
  app.fe.setTheme(savedTheme(app));
  app.fe.setIconPack(savedIconPack(app));
  app.ai.onToggle = (open) => app.aiBtn.classList.toggle("on", open);
  app.ai.onRunStart = async (prompt) => {
    if (app.dirtySinceVersion) await app.checkpoint("auto", "Before AI edit");
    void prompt;
  };
  app.ai.onRunEnd = async (prompt, n, model) => {
    await app.checkpoint("ai", `AI: ${prompt.length > 60 ? prompt.slice(0, 60) + "…" : prompt}`, true, model);
    void n;
  };

  app.fe.on("change", () => {
    app.markDirty();
    app.a11y.schedule();
    updateWords(app);
  });
  app.fe.on("open", () => app.a11y.schedule());
  app.fe.on("selection", () => app.ai.updateSelection());
  app.fe.on("layout", () => updatePageReadout(app));
  app.fe.on("protection", () => updateProtectStat(app));
  app.fe.on("open", () => updateProtectStat(app));
  app.editor.canvas.addEventListener("scroll", () => updatePageReadout(app), { passive: true });

  app.fileInput = h("input", { type: "file", accept: ".docx,.docm,.dotx", style: "display:none" });
  app.fileInput.addEventListener("click", () => (app.fileInput.accept = app.accept()));
  app.fileInput.addEventListener("change", () => {
    const f = app.fileInput.files?.[0];
    if (f) void app.importFiles(app.files.library, "", [f]);
    app.fileInput.value = "";
  });

  const workspace = h("main", { class: "workspace" }, app.sidebar.el, app.fe.surface, app.ai.el);
  root.append(topbar(app), app.fe.toolbarEl!, workspace, statusbar(app), app.fileInput);
  app.side = new SideBySide({
    main: app.fe,
    workspace,
    keep: async (name, bytes) => {
      const final = await uniqueName(app.files.library, "", name);
      await app.files.library.write(final, bytes);
      return { key: `library:${final}`, name: final, write: (b) => app.files.library.write(final, b) };
    },
    mainKey: () => app.docKey(),
  });
  bindGlobal(app);
  registerOffline(app);
}

export function topbar(app: App): HTMLElement {
  app.titleInput = h("input", { class: "doc-title", "aria-label": t("Document name"), spellcheck: "false" });
  app.titleInput.addEventListener("change", () => void app.renameCurrent(app.titleInput.value));
  app.titleInput.addEventListener("input", () => fitTitle(app));
  app.titleInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") app.titleInput.blur();
    if (e.key === "Escape") {
      setTitle(app, app.stem());
      app.titleInput.blur();
    }
  });
  app.led = h("span", { class: "led", "aria-live": "polite" }, t("Saved"));
  app.lockMark = h("span", { class: "lock-mark", hidden: true, title: t("Protected with a password"), "aria-label": t("Protected with a password"), role: "img" }, svg(I.lock));
  const navBtn = h("button", { class: "btn ghost icon-btn", type: "button", title: t("Navigator"), "aria-label": t("Toggle navigator") }, svg(I.sidebar));
  navBtn.addEventListener("click", () => app.sidebar.toggle());
  app.saveBtn = h("button", { class: "btn primary", type: "button", title: tip(app, t("Save"), "file.save"), hidden: true }, svg(I.save), h("span", { class: "txt" }, t("Save")));
  app.saveBtn.addEventListener("click", () => void app.save(true));
  app.aiBtn = h("button", { class: "btn", type: "button", title: tip(app, t("Assistant"), "view.assistant") }, h("span", { class: "lamp" }), h("span", { class: "txt" }, t("Assistant")));
  app.aiBtn.addEventListener("click", () => app.ai.toggle());
  return h(
    "header",
    { class: "topbar" },
    navBtn,
    h("a", { class: "wordmark", href: "#/", title: t("Your documents"), "aria-label": t("Docxtor, your documents") }, "docxtor"),
    h("div", { class: "doc-meta" }, h("div", { class: "doc-head" }, app.titleInput, h("div", { class: "doc-sub" }, app.lockMark, app.led)), app.fe.menuBarEl),
    h("div", { class: "top-actions" }, darkButton(app, "btn ghost icon-btn"), app.saveBtn, app.aiBtn),
  );
}

/**
 * The interface language, before anything is built: `?lang=` (e.g. `en-XA` to find untranslated
 * text), then the one chosen in Display language, then the browser's. Packs come from installed
 * extensions that are turned on.
 */
export function chooseLanguage(_app: App, disabled: string[]) {
  const packs = [...languages, ...installed].filter((x) => !disabled.includes(x.id)).flatMap((x) => x.contributes?.languages ?? []);
  const requested = new URLSearchParams(location.search).get("lang") || storage(LANG_KEY, { locale: "" }).locale || window.navigator.language;
  setLanguage(pickLanguage(requested, packs));
  document.documentElement.lang = currentLocale() === PSEUDO_LOCALE ? "en" : currentLocale().replace("-XB", "");
  // Arabic, Hebrew, Persian and the rest: the whole app turns round (the pages keep their own direction).
  document.documentElement.dir = textDirection();
}

/** Pick the interface language. Docxtor reloads to rebuild the interface in it. */
export async function pickAppLanguage(app: App) {
  const packs = app.fe.extensions.languages();
  const items = [{ label: "English", locale: "en" }, ...packs.map((p) => ({ label: p.label, locale: p.locale }))].map((x) => ({
    ...x,
    hint: x.locale === currentLocale() ? t("Current") : "",
  }));
  const pick = await quickPick(items, { placeholder: t("Choose a language"), label: t("Display language") });
  if (!pick || pick.locale === currentLocale()) return;
  store(LANG_KEY, { locale: pick.locale });
  if (app.dirty) await app.save(false);
  location.reload();
}

/** "Save (⌘S)": a label with its command's shortcut on this platform. */
export function tip(app: App, label: string, command: string): string {
  const key = app.fe.extensions.shortcut(command);
  return key ? `${label} (${key})` : label;
}

/** Files on disk: save on every change, or only on ⌘S. */
export function toggleAutosave(app: App) {
  app.prefs.autosaveFiles = !app.prefs.autosaveFiles;
  store(PREFS_KEY, app.prefs);
  toast(app.prefs.autosaveFiles ? t("Files on disk now save automatically.") : t("Files on disk save when you press {key}.", { key: app.fe.extensions.shortcut("file.save") }));
}

/** Choose a theme: the system's, light, dark, or one an extension added. Remembered. */
export async function pickTheme(app: App) {
  const items = app.fe.themes().map((x) => ({ label: x.label, id: x.id, hint: x.id === app.fe.currentTheme ? t("Current") : "" }));
  const pick = await quickPick(items, { placeholder: t("Choose a theme"), label: t("Theme") });
  if (!pick) return;
  app.fe.setTheme(pick.id);
  store(THEME_KEY, { theme: pick.id });
}

/** Whether the app shows dark now: Dark, a theme built on dark, or Match the system while the system is dark. */
export function showsDark(app: App): boolean {
  const id = app.fe.currentTheme;
  const base = id === "light" || id === "dark" ? id : app.fe.extensions.themes.list().find((x) => x.id === id)?.base;
  return base ? base === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Light if the app shows dark, dark if it shows light. Remembered, like a theme chosen in View, Theme. */
export function toggleDark(app: App) {
  const id = showsDark(app) ? "light" : "dark";
  app.fe.setTheme(id);
  store(THEME_KEY, { theme: id });
}

/** The light and dark switch: a sun while dark, a moon while light. It follows a theme chosen anywhere else, and the system's. */
export function darkButton(app: App, className: string): HTMLButtonElement {
  const b = h("button", { class: className, type: "button" }) as HTMLButtonElement;
  const system = matchMedia("(prefers-color-scheme: dark)");
  const show = () => {
    // A start screen that closed takes its button with it: stop listening then.
    if (!b.isConnected && b.dataset.shown) return void (off(), system.removeEventListener("change", show));
    const dark = showsDark(app);
    const label = dark ? t("Switch to light") : t("Switch to dark");
    b.replaceChildren(svg(dark ? I.sun : I.moon));
    b.title = label;
    b.setAttribute("aria-label", label);
    b.dataset.shown = "1";
  };
  const off = app.fe.on("theme", show);
  system.addEventListener("change", show);
  b.addEventListener("click", () => toggleDark(app));
  show();
  return b;
}

/** Choose an icon pack: Docxtor's, the light one, or one an extension added. Remembered. */
export async function pickIconPack(app: App) {
  const items = app.fe.iconPacks().map((x) => ({ label: x.label, id: x.id, hint: x.id === app.fe.currentIconPack ? t("Current") : "" }));
  const pick = await quickPick(items, { placeholder: t("Choose an icon pack"), label: t("Icons") });
  if (!pick) return;
  app.fe.setIconPack(pick.id);
  store(ICON_PACK_KEY, { pack: pick.id });
}

/** The pack chosen earlier. It applies as soon as its extension is on, so a pack from an extension is back after a reload. */
export function savedIconPack(_app: App): string {
  return storage(ICON_PACK_KEY, { pack: "docxtor" }).pack || "docxtor";
}

export function savedTheme(_app: App): string {
  const t = storage(THEME_KEY, { theme: "system" }).theme;
  return t === "auto" ? "system" : t;
}

export function manageExtensions(app: App) {
  openExtensions(app.fe.extensions, async (id, on) => {
    const host = app.fe.extensions;
    // An extension that sends the document's text somewhere says so, and asks first.
    const privacy = host.list().find((x) => x.id === id)?.privacy;
    if (on && privacy && !(await confirmDialog({ title: t("Send your text to a server?"), body: t(privacy), ok: t("Turn on") }))) return;
    await (on ? host.enable(id) : host.disable(id));
    const now = storage(EXT_KEY, { disabled: [] as string[], enabled: [] as string[] });
    const off = new Set(now.disabled);
    const turnedOn = new Set(now.enabled ?? []);
    if (on) {
      off.delete(id);
      turnedOn.add(id);
    } else {
      off.add(id);
      turnedOn.delete(id);
    }
    store(EXT_KEY, { disabled: [...off], enabled: [...turnedOn] });
    app.fe.refreshToolbar();
    // Turning a theme's extension back on restores the theme chosen earlier.
    const saved = savedTheme(app);
    if (saved !== app.fe.currentTheme && app.fe.themes().some((x) => x.id === saved)) app.fe.setTheme(saved);
  });
}

export function setTitle(app: App, name: string) {
  app.titleInput.value = name;
  fitTitle(app);
}

let measureCanvas: CanvasRenderingContext2D | undefined;

/** The name field is as wide as the name (capped in CSS). Browsers without `field-sizing` are measured here. */
export function fitTitle(app: App) {
  if (CSS.supports("field-sizing", "content")) return;
  const el = app.titleInput;
  const cs = getComputedStyle(el);
  const ctx = (measureCanvas ??= document.createElement("canvas").getContext("2d")!);
  ctx.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
  const text = el.value || el.placeholder || " ";
  const pad = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
  el.style.width = `${Math.ceil(ctx.measureText(text).width + pad + 2)}px`;
}

export function statusbar(app: App): HTMLElement {
  app.pageReadout = h("span", { class: "readout" }, t("Page {page} of {pages}", { page: 1, pages: 1 }));
  app.wordsReadout = h("span", { class: "readout" }, tn(0, "{n} word", "{n} words"));
  app.fidelityStat = h("div", { class: "stat fidelity hide-sm" }, h("span", { class: "readout" }, ""));
  app.protectStat = h("div", { class: "stat protect", hidden: true, role: "status" }, h("span", { class: "readout" }, ""));
  app.zoom = new ZoomControl(app.fe);
  app.footer = h(
    "footer",
    { class: "statusbar" },
    h("div", { class: "stat" }, app.pageReadout),
    h("div", { class: "stat" }, app.wordsReadout),
    app.protectStat,
    app.fidelityStat,
    h("span", { class: "grow" }),
    app.zoom.el,
  );
  return app.footer;
}

/** Put a line of status in the status bar, before the zoom. */
export function addStatus(app: App, el: HTMLElement) {
  app.footer.insertBefore(el, app.footer.querySelector(".grow"));
}

export function updateProtectStat(app: App) {
  const text = app.fe.protectionStatus;
  app.protectStat.hidden = !text;
  app.protectStat.firstElementChild!.textContent = text ?? "";
}

export function updatePageReadout(app: App) {
  const pages = app.editor.pages;
  app.pageReadout.textContent = t("Page {page} of {pages}", { page: app.editor.currentPage(), pages });
}

export function setState(app: App, s: SaveState) {
  app.saveState = s;
  const text = {
    saved: app.where ? t("Saved in {place}", { place: app.where }) : t("Saved"),
    dirty: t("Unsaved changes"),
    saving: t("Saving…"),
    error: t("Couldn't save"),
    "unsaved-file": t("Unsaved changes"),
  }[s];
  // Every keystroke marks the document unsaved: write only what differs (a write, even of the same text, makes the browser lay the page out again).
  if (app.led.textContent !== text) app.led.textContent = text;
  const cls = `led${s === "saved" || s === "saving" ? "" : s === "error" ? " error" : " dirty"}`;
  if (app.led.className !== cls) app.led.className = cls;
  // Saving is automatic in the Library (and on disk with Autosave on), so the button only shows
  // when something waits for you: unsaved changes to a file, or a save that failed.
  const hide = s !== "unsaved-file" && s !== "error";
  if (app.saveBtn.hidden !== hide) app.saveBtn.hidden = hide;
  const label = app.saveBtn.querySelector(".txt")!;
  const word = s === "error" ? t("Try again") : t("Save");
  if (label.textContent !== word) label.textContent = word;
}

export function bindGlobal(app: App) {
  // Every shortcut is a command (the editor's, the app's, extensions'); the host resolves them.
  document.addEventListener("keydown", (e) => app.fe.handleKey(e));
  let veil: HTMLElement | null = null;
  let depth = 0;
  const isDoc = (e: DragEvent) => Array.from(e.dataTransfer?.items ?? []).some((i) => i.kind === "file" && !i.type.startsWith("image/"));
  window.addEventListener("dragenter", (e) => {
    if (!isDoc(e)) return;
    depth++;
    veil ??= app.root.appendChild(h("div", { class: "drop-veil" }, t("Drop to open")));
    startElement()?.classList.add("drag");
  });
  window.addEventListener("dragleave", () => {
    if (--depth <= 0) {
      depth = 0;
      veil?.remove();
      veil = null;
      startElement()?.classList.remove("drag");
    }
  });
  window.addEventListener("dragover", (e) => e.preventDefault());
  window.addEventListener("drop", (e) => {
    depth = 0;
    veil?.remove();
    veil = null;
    startElement()?.classList.remove("drag");
    const item = Array.from(e.dataTransfer?.items ?? []).find((i) => i.kind === "file");
    const f = e.dataTransfer?.files?.[0];
    if (f && app.fe.canOpen(f.name)) {
      e.preventDefault();
      void app.openDropped(item, f);
    } else if (!f?.type.startsWith("image/")) e.preventDefault();
  });
  window.addEventListener("beforeunload", (e) => {
    if (unsavedAtClose(app)) e.preventDefault();
  });
  window.setInterval(() => {
    if (app.dirtySinceVersion && Date.now() - app.lastVersionAt > app.prefs.checkpointMinutes * 60_000) void app.checkpoint("auto", "Checkpoint");
  }, 30_000);
}

/**
 * Whether closing the page would lose changes, which makes the browser ask first. Changes to the open document and to the
 * side-by-side one count. A Library document is saved on the way out (the write starts now), unless this page can no
 * longer reach the database (a newer Docxtor took it over), a write is running or the last one failed: then the changes
 * exist only here until it ends.
 */
export function unsavedAtClose(app: App): boolean {
  let unsaved = false;
  const q = app.queue;
  if (q?.dirty) {
    // Asked before the write below starts, so a write this page starts for the exit does not count as one in the way.
    unsaved = app.ref?.backend.id !== "library" || isStale() || q.writing || q.failed;
    void app.save(false);
  }
  if (app.side.unsaved) {
    void app.side.flush();
    unsaved = true;
  }
  return unsaved;
}

export function showFidelity(app: App) {
  const out = app.fidelityStat.querySelector(".readout")!;
  if (!app.original || !app.engine) {
    out.textContent = t("New document");
    app.fidelityStat.title = t("Created in Docxtor");
    return;
  }
  // Worked out off the page (a whole save and a comparison: 130 ms in War and Peace, 260 in Proust, which held the first key of a long book),
  // once the document is open; the readout follows when it is ready, for the document it was asked for.
  const engine = app.engine;
  const original = app.original;
  out.textContent = "";
  void engine
    .fidelity(original)
    .then((r) => {
      if (app.engine !== engine || app.original !== original) return;
      out.textContent = t("{score}% round-trip fidelity", { score: Number(r.score).toFixed(1) });
      app.fidelityStat.title = t("Re-saving without edits keeps {same} of {total} parts byte-identical.", { same: r.partsIdentical, total: r.partsTotal });
    })
    .catch(() => {
      if (app.engine === engine) out.textContent = "";
    });
}

export function updateWords(app: App) {
  window.clearTimeout(app.wordsTimer);
  app.wordsTimer = window.setTimeout(() => {
    const n = app.engine?.raw.wordCount() ?? 0;
    app.wordsReadout.textContent = tn(n, "{n} word", "{n} words");
  }, 300);
}
