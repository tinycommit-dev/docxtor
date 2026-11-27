// Opening documents: from the library, a file, a folder, an address, a drop or a template, and making new ones.
import { parentOf } from "./files/fs";
import type { App } from "./main.ts";
import { Engine, OpenError, WrongPasswordError, isEncrypted, loadEngine } from "@docxtor/core";
import type { DocRef } from "./app-shared";
import { DOC_EXT, SingleFileFs, baseName, canPickFiles, ensurePermission, hasPermission, join, recents, uniqueName } from "./files/fs";
import { describeOpenError, docxFrom, h, openFailedDialog, packageAsDocument, packageKind, t, toast } from "@docxtor/editor";
import { closeStart, showStart, startOpen } from "./start-screen";
import { claim } from "./doc-lock";
import { askPassword, protectDialog } from "./ui/password";
import type { Route } from "./route";
import { parseRoute, routeHash, sameRoute } from "./route";
import type { DocumentSource, OpenNotice } from "@docxtor/editor";
import type { Backend, Recent } from "./files/fs";
import { chooseTemplate } from "./ui/templates";
import { SAMPLE } from "./sample-doc";

/**
 * Swap the open document, always in this order: stop what looks at the old engine (the split window, with the view it
 * left; the old save queue), hand the new engine to the editor, and only then dispose the old one.
 */
export function load(app: App, engine: Engine, ref: DocRef, original: Uint8Array | null) {
  const old = app.engine;
  app.fe.closePanes();
  app.queue?.close();
  app.queue = null;
  app.engine = engine;
  app.ref = ref;
  app.original = original;
  engine.fileName = baseName(ref.path);
  app.setTitle(app.stem());
  document.title = `${app.stem()} — Docxtor`;
  app.where =
    ref.backend.id === "library"
      ? t("your Library")
      : ref.backend.id.startsWith("folder:")
        ? `${ref.backend.label}/${parentOf(ref.path)}`.replace(/\/$/, "")
        : app.isProvided(ref.backend)
          ? ref.backend.label
          : t("a file on disk");
  syncUrl(app);
  app.setState(app.saveState);
  app.ai.reset();
  // The app owns its engines (and disposes them), so it hands them over with load(), not open().
  app.fe.load(engine);
  if (old && old !== engine) old.dispose();
  app.lockMark.hidden = !engine.hasPassword;
  app.queue = app.newQueue(engine, engine.revision);
  app.dirtySinceVersion = false;
  app.savedElsewhere = false;
  app.setState("saved");
  app.updateWords();
  // The start screen goes first: while it is up the editor is inert, and a focus given to it then is lost (the text had no keyboard after opening).
  closeStart();
  app.editor.focus();
  app.editor.emitSelection();
  app.showFidelity();
  void app.files.refresh();
  if (app.sidebar.tab === "history") void app.history.render();
  // Open in another window already: this one reads it, so the two can't overwrite each other.
  const key = app.docKey();
  if (key)
    void claim(key).then((free) => {
      if (free || app.engine !== engine) return;
      // A reason that belongs to this document: it ends when another one is opened in this window.
      app.fe.lock("window-lock", { scope: "document" });
      // Read-only still lets fields update before printing; whatever that changes stays in this window.
      app.savedElsewhere = true;
      toast(t("This document is open in another window, so this window is read-only. Edit it there, or close that window and open it again here."), 9000);
    });
}

/**
 * Open .docx bytes as an engine. A password-protected file asks for its password in a dialog (`known` is tried first,
 * for a copy or version of a document that is already open); null when the person cancels.
 */
export async function openBytes(_app: App, bytes: Uint8Array, name: string, known?: string | null): Promise<Engine | null> {
  if (!isEncrypted(bytes)) return Engine.open(bytes, name);
  if (known) {
    try {
      return Engine.open(bytes, name, known);
    } catch (e) {
      if (!(e instanceof WrongPasswordError)) throw e;
    }
  }
  const got = await askPassword(name, (password) => Engine.open(bytes, name, password));
  return got?.value ?? null;
}

/** The bytes of a protected file as a plain .docx (asking for the password); null when the person cancels. */
export async function unlockBytes(_app: App, bytes: Uint8Array, name: string, known?: string | null): Promise<Uint8Array | null> {
  if (!isEncrypted(bytes)) return bytes;
  if (known) {
    try {
      return Engine.plain(bytes, known);
    } catch (e) {
      if (!(e instanceof WrongPasswordError)) throw e;
    }
  }
  const got = await askPassword(name, (password) => Engine.plain(bytes, password));
  return got?.value ?? null;
}

/** The person backed out of the password dialog: stay where they were (the address goes back to the open document). */
export function backToWhereWeWere(app: App) {
  if (app.ref) {
    const url = `${location.pathname}${location.search}${routeHash(routeOf(app, app.ref))}`;
    history.replaceState(null, "", url);
  } else {
    history.replaceState(null, "", `${location.pathname}${location.search}#/`);
    if (!startOpen()) void showStart(app);
  }
}

/** The document `key` is about to open in the main editor: close the side-by-side pane if it shows that document. False when the pane could not be saved and closed. */
export async function closeSideFor(app: App, key: string): Promise<boolean> {
  if (!app.side.on || app.side.key !== key) return true;
  const name = (app.side.name ?? "").replace(DOC_EXT, "");
  if (!(await app.side.close())) return false;
  toast(t("{name} was open beside this document. It is open here now, so the pane beside it is closed.", { name }), 6000);
  return true;
}

export async function openRef(app: App, ref: DocRef, password?: string | null) {
  try {
    await loadEngine();
    if (app.dirty) await app.save(false);
    // A document is edited in one place: one that is beside this one is closed (saved first) before it opens here.
    if (!(await closeSideFor(app, `${ref.backend.id}:${ref.path}`))) return;
    const bytes = await ref.backend.read(ref.path);
    if (await importMisnamed(app, bytes, baseName(ref.path))) return;
    const engine = await openBytes(app, bytes, baseName(ref.path), password);
    if (!engine) return backToWhereWeWere(app);
    // The faces of the document's fonts first, so its first layout is its last (see `DocxtorEditor.prepare`). Another document shown
    // while they load (an open the person started, or a host's) wins: this one gives way.
    const shown = app.fe.engine;
    await app.fe.prepare(engine);
    if (app.fe.engine !== shown) {
      engine.dispose();
      return;
    }
    load(app, engine, ref, engine.openedPackage ?? bytes);
    await app.checkpoint("open", "Opened", false, undefined, bytes);
    app.rememberRecent();
  } catch (e) {
    app.fe.announceOpen(undefined);
    backToWhereWeWere(app);
    if (e instanceof OpenError) await openFailedDialog(baseName(ref.path), describeOpenError(e));
    else toast(t("Couldn't open {name}: {reason}", { name: baseName(ref.path), reason: (e as Error).message }), 5000);
  }
}

export function routeOf(app: App, ref: DocRef): Route {
  if (ref.backend.id === "library") return { kind: "library", path: ref.path };
  if (ref.backend.id.startsWith("folder:")) return { kind: "folder", folder: ref.backend.label, path: ref.path };
  if (app.isProvided(ref.backend)) return { kind: "storage", provider: ref.backend.id, path: ref.path };
  return { kind: "file", name: ref.path };
}

/** Point the address bar at the open document (a new history entry, unless replacing). */
export function syncUrl(app: App, replace = false) {
  if (!app.ref) return;
  const hash = routeHash(routeOf(app, app.ref));
  if (location.hash === hash) return;
  const url = `${location.pathname}${location.search}${hash}`;
  if (replace || app.navigating) history.replaceState(null, "", url);
  else history.pushState(null, "", url);
}

/** Open whatever the address names: a document, or the start screen. */
export async function openFromUrl(app: App) {
  const route = parseRoute(location.hash);
  if (route.kind === "home") {
    if (app.dirty) await app.save(false);
    document.title = "Docxtor";
    void showStart(app);
    return;
  }
  if (app.ref && sameRoute(route, routeOf(app, app.ref))) {
    // Back from the start screen to the document that is still open, with the keyboard in its text.
    closeStart();
    app.editor.focus();
    document.title = `${app.stem()} — Docxtor`;
    return;
  }
  app.navigating = true;
  try {
    await openRoute(app, route);
  } finally {
    app.navigating = false;
  }
}

export async function openRoute(app: App, route: Exclude<Route, { kind: "home" }>) {
  const missing = (what: string) => {
    toast(what, 5000);
    history.replaceState(null, "", `${location.pathname}${location.search}#/`);
    void showStart(app);
  };
  if (route.kind === "library") {
    if (!(await app.files.library.exists(route.path))) return missing(t("“{name}” isn't in your Library any more.", { name: baseName(route.path) }));
    return openRef(app, { backend: app.files.library, path: route.path });
  }
  if (route.kind === "storage") {
    await app.fe.ready;
    const p = app.provider(route.provider);
    if (!p) return missing(t("Turn on the extension that provides “{storage}” to open “{name}”.", { storage: route.provider, name: baseName(route.path) }));
    if (!(await p.exists(route.path).catch(() => false))) return missing(t("“{name}” isn't in {place} any more.", { name: baseName(route.path), place: p.label }));
    return openRef(app, { backend: p, path: route.path });
  }
  if (route.kind === "folder") {
    await app.files.ready;
    const folder = app.files.folder;
    if (!folder || folder.label !== route.folder) return missing(t("Connect the “{folder}” folder from the Files tab to open “{name}”.", { folder: route.folder, name: baseName(route.path) }));
    if (await hasPermission(folder.root)) return openRef(app, { backend: folder, path: route.path });
    // Browsers only grant file access from a click: ask on the start screen.
    return showStart(app, {
      name: baseName(route.path),
      open: async () => {
        if (!(await ensurePermission(folder.root))) return toast(t("Permission to open that folder was denied."));
        app.navigating = true;
        try {
          await openRef(app, { backend: folder, path: route.path });
        } finally {
          app.navigating = false;
        }
      },
    });
  }
  const r = (await recents()).find((x) => x.backend === "file" && x.handle && x.name === route.name);
  if (!r?.handle) return missing(t("Open “{name}” again from your computer; Docxtor no longer has access to it.", { name: route.name }));
  const handle = r.handle;
  if (await hasPermission(handle)) return openRef(app, { backend: new SingleFileFs(handle), path: handle.name });
  return showStart(app, {
    name: route.name,
    open: async () => {
      if (!(await ensurePermission(handle))) return toast(t("Permission to open that file was denied."));
      app.navigating = true;
      try {
        await openRef(app, { backend: new SingleFileFs(handle), path: handle.name });
      } finally {
        app.navigating = false;
      }
    },
  });
}

/**
 * Make a .docx from an importer or template result and open it from the Library. A Word template (.dotx) becomes a
 * document, made now, as Word makes one from a template.
 */
export async function createFromSource(app: App, source: DocumentSource, name: string, backend: Backend = app.files.library, dir = "") {
  const fileName = DOC_EXT.test(name) ? name : `${name}.docx`;
  let bytes = await docxFrom(source, fileName);
  await loadEngine();
  if (packageKind(bytes).template) bytes = packageAsDocument(bytes, new Date());
  // The extension says what the main part says (Word refuses a .docx that holds macros): .docm when it has them.
  const ext = packageKind(bytes).macros ? ".docm" : ".docx";
  const final = await uniqueName(backend, dir, fileName.replace(/\.(docx|docm|dotx|dotm)$/i, ext));
  await backend.write(join(dir, final), bytes);
  await openRef(app, { backend, path: join(dir, final) });
  app.stampAuthor();
}

/** Pick a template (a built-in one, one you saved, or a file on your computer) and start a document from it in the Library. */
export async function newFromTemplate(app: App) {
  const choice = await chooseTemplate({ list: () => app.fe.extensions.templateEntries(), ask: (id, ...args) => app.fe.extensions.ask(id, ...args), onClose: () => app.engine && app.editor.focus() });
  if (!choice) return;
  try {
    if ("file" in choice) {
      const picked = await pickFileFromComputer(app);
      if (picked) await createFromSource(app, { docx: picked.bytes }, picked.name.replace(/\.(dotx|dotm)$/i, ".docx"));
      return;
    }
    const { entry } = choice;
    await createFromSource(app, await entry.create(), entry.fileName ?? `${entry.name}.docx`);
  } catch (e) {
    toast(t("Couldn't use that template: {reason}", { reason: (e as Error).message }), 5000);
  }
}

/** Make a new document in `dir` (from Markdown, if given) and open it, unless `open` is false; resolves to its path. */
export async function createIn(app: App, backend: Backend, dir: string, name = "Untitled.docx", markdown?: string, opts: { open?: boolean } = {}): Promise<string> {
  const eng = await newEngine(app, name, markdown);
  const bytes = eng.save();
  eng.dispose();
  const final = await uniqueName(backend, dir, name);
  await backend.write(join(dir, final), bytes);
  if (opts.open !== false) {
    await openRef(app, { backend, path: join(dir, final) });
    app.stampAuthor();
  }
  return join(dir, final);
}

/** A new document as `createIn` makes it: blank, or from Markdown, in the person's default font. */
export async function newEngine(app: App, name: string, markdown?: string): Promise<Engine> {
  await loadEngine();
  const eng = Engine.blank(name);
  // The font the person chose under Tools, Preferences, for documents that start empty.
  const { defaultFont, defaultSize } = app.prefs;
  if (defaultFont || defaultSize) eng.apply({ op: "set_default_format", format: { ...(defaultFont ? { font: defaultFont } : {}), ...(defaultSize ? { size: defaultSize } : {}) } });
  if (markdown) {
    const first = eng.renderAll().order[0];
    eng.apply([
      { op: "insert_markdown", before: first, markdown },
      { op: "delete_blocks", ids: [first] },
    ]);
  }
  return eng;
}

/**
 * Help, Open the tour in a new tab: the Welcome document opens in a tab of its own, so the one being worked on stays
 * where it is (as Google Docs opens Help and new documents). From the start screen (`newTab` false) it opens here;
 * when the browser won't open a tab it opens here too, after the document is saved, and says so. A tour no one has
 * changed is opened again, not copied.
 */
export async function sample(app: App, newTab = true) {
  const lib = app.files.library;
  const name = `${t("Welcome to Docxtor")}.docx`;
  const path = (await unchangedTour(app, lib, name)) ?? (await createIn(app, lib, "", name, t(SAMPLE), { open: false }));
  if (newTab && app.engine) {
    // Not "noopener" in the features: with it the browser gives nothing back, and the tab would look blocked.
    const tab = window.open(`${location.pathname}${location.search}${routeHash({ kind: "library", path })}`, "_blank");
    if (tab) {
      tab.opener = null;
      return;
    }
    const current = app.stem();
    // Only say the document is saved when the write went through: otherwise it stays open, with the save's error.
    if (app.dirty) await app.save(false).catch(() => undefined);
    if (app.dirty && !app.savedElsewhere) {
      toast(t("The browser didn't open a new tab, and “{name}” couldn't be saved, so it stays open. Save it, then open the tour again.", { name: current }), 8000);
      return;
    }
    await openRef(app, { backend: lib, path });
    toast(t("The browser didn't open a new tab, so the tour opened here. “{name}” is saved.", { name: current }), 6000);
    return;
  }
  await openRef(app, { backend: lib, path });
}

/** A tour in the Library (the name, or the name with a number) whose text is still the tour's own. */
export async function unchangedTour(app: App, lib: Backend, name: string): Promise<string | null> {
  const stem = name.replace(/\.docx$/, "");
  const same = new RegExp(`^${stem.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}( \\d+)?\\.docx$`);
  const found = (await lib.list("")).filter((n) => n.kind === "file" && same.test(n.name));
  if (!found.length) return null;
  const fresh = await newEngine(app, name, t(SAMPLE));
  const text = fresh.text();
  fresh.dispose();
  for (const n of found) {
    try {
      const eng = Engine.open(await lib.read(n.path), n.name);
      const unchanged = eng.text() === text;
      eng.dispose();
      if (unchanged) return n.path;
    } catch {
      // Not readable as a tour: leave it.
    }
  }
  return null;
}

/** Copy files into `dir`; formats other than .docx are converted by their importer first. */
export async function importFiles(app: App, backend: Backend, dir: string, files: File[]) {
  let last = "";
  let notice: OpenNotice | undefined;
  let failed: { name: string; reason: string } | null = null;
  for (const f of files) {
    try {
      const doc = await app.fe.toDocx(new Uint8Array(await f.arrayBuffer()), f.name);
      const name = await uniqueName(backend, dir, doc.name);
      await backend.write(join(dir, name), doc.bytes);
      last = join(dir, name);
      notice = doc.notice;
    } catch (e) {
      failed ??= { name: f.name, reason: describeOpenError(e) };
    }
  }
  if (failed) await openFailedDialog(failed.name, failed.reason);
  if (!last) return;
  // The opened document is a new .docx made from the file: the person hears what the conversion left out.
  app.fe.announceOpen(notice);
  await openRef(app, { backend, path: last });
}

/** A file that is called .docx but is really another kind (RTF, a web page, an old .doc...) becomes a new document. */
export async function importMisnamed(app: App, bytes: Uint8Array, name: string): Promise<boolean> {
  if ((bytes[0] === 0x50 && bytes[1] === 0x4b) || (bytes[0] === 0xd0 && bytes[1] === 0xcf)) return false;
  if (!["odt", "rtf", "html", "doc", "flat-opc", "word-2003"].includes(Engine.sniff(bytes))) return false;
  await importFiles(app, app.files.library, "", [new File([bytes as BlobPart], name)]);
  return true;
}

/** Open a file with save-in-place when the browser allows it, else import into the Library. */
export async function openPicker(app: App) {
  if (!canPickFiles) {
    app.fileInput.click();
    return;
  }
  try {
    const pick = (window as unknown as { showOpenFilePicker(o: object): Promise<FileSystemFileHandle[]> }).showOpenFilePicker;
    const others = app.accept()
      .split(",")
      .filter((e) => !DOC_EXT.test(`x${e}`)) as `.${string}`[];
    const [handle] = await pick({
      id: "docxtor-open",
      types: [
        { description: t("Word documents"), accept: { "application/vnd.openxmlformats-officedocument.wordprocessingml.document": [".docx", ".docm", ".dotx"] } },
        ...(others.length ? [{ description: t("Other formats Docxtor can import"), accept: { "text/plain": others } }] : []),
      ],
    });
    if (!handle) return;
    // Word files save back in place; other formats become a new document in the Library.
    if (DOC_EXT.test(handle.name)) await openRef(app, { backend: new SingleFileFs(handle), path: handle.name });
    else await importFiles(app, app.files.library, "", [await handle.getFile()]);
  } catch (e) {
    if ((e as Error).name !== "AbortError") toast((e as Error).message);
  }
}

export async function openDropped(app: App, item: DataTransferItem | undefined, f: File) {
  const getHandle = (item as unknown as { getAsFileSystemHandle?: () => Promise<FileSystemHandle | null> } | undefined)?.getAsFileSystemHandle;
  const handle = getHandle ? await getHandle.call(item).catch(() => null) : null;
  if (handle && handle.kind === "file" && DOC_EXT.test(handle.name)) await openRef(app, { backend: new SingleFileFs(handle as FileSystemFileHandle), path: handle.name });
  else await importFiles(app, app.files.library, "", [f]);
}

export async function openRecent(app: App, r: Recent) {
  if (r.backend === "library") return openRef(app, { backend: app.files.library, path: r.path });
  if (r.backend === "file" && r.handle) {
    if (!(await ensurePermission(r.handle))) return toast(t("Permission to open that file was denied."));
    return openRef(app, { backend: new SingleFileFs(r.handle), path: r.handle.name });
  }
  if (r.backend === "storage") {
    const p = r.provider ? app.provider(r.provider) : undefined;
    if (p) return openRef(app, { backend: p, path: r.path });
    return toast(t("Turn on the extension that provides this document's storage, from Extensions, Manage extensions."));
  }
  if (r.backend === "folder" && app.files.folder && app.files.folder.label === r.folder) {
    if (!(await ensurePermission(app.files.folder.root))) return toast(t("Permission to open that folder was denied."));
    return openRef(app, { backend: app.files.folder, path: r.path });
  }
  toast(t("Reconnect the folder this document lives in from the Files tab."));
}

/** A file input for one document (converted to .docx when it is another format); null if the person backs out. */
export function pickFileFromComputer(app: App): Promise<{ name: string; bytes: Uint8Array } | null> {
  return new Promise((resolve) => {
    const input = h("input", { type: "file", accept: app.accept(), style: "display:none" });
    input.addEventListener("change", async () => {
      const f = input.files?.[0];
      input.remove();
      if (!f) return resolve(null);
      try {
        const doc = await app.fe.toDocx(new Uint8Array(await f.arrayBuffer()), f.name);
        resolve({ name: doc.name, bytes: doc.bytes });
      } catch (e) {
        toast(t("Couldn't open {name}: {reason}", { name: f.name, reason: (e as Error).message }), 5000);
        resolve(null);
      }
    });
    input.addEventListener("cancel", () => {
      input.remove();
      resolve(null);
    });
    app.root.append(input);
    input.click();
  });
}

/** The bytes of a recent document; null when the browser's permission is refused. */
export async function readRecent(app: App, r: Recent): Promise<Uint8Array | null> {
  if (r.backend === "library") return app.files.library.read(r.path);
  if (r.backend === "file" && r.handle) {
    if (!(await ensurePermission(r.handle, "read"))) return null;
    return new Uint8Array(await (await r.handle.getFile()).arrayBuffer());
  }
  if (r.backend === "storage") {
    const p = r.provider ? app.provider(r.provider) : undefined;
    return p ? p.read(r.path) : null;
  }
  if (r.backend === "folder" && app.files.folder && app.files.folder.label === r.folder) {
    if (!(await ensurePermission(app.files.folder.root, "read"))) return null;
    return app.files.folder.read(r.path);
  }
  return null;
}

/** File, Protect with password: set, change or remove the document's password. The next save writes it (or leaves it out). */
export async function protect(app: App) {
  const engine = app.engine;
  if (!engine || !app.ref) return;
  const got = await protectDialog({ name: app.fileName(), hasPassword: engine.hasPassword, ask: (id, ...args) => app.fe.extensions.ask(id, ...args) });
  app.editor.focus();
  if (!got) return;
  engine.setPassword(got.password);
  app.lockMark.hidden = !engine.hasPassword;
  // The password is part of how the file is written, so the document counts as changed and saves again.
  app.markDirty();
  toast(got.password ? t("This document will be saved with a password.") : t("The password is removed. This document will be saved without one."));
}
