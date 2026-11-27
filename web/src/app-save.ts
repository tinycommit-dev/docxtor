// Saving: the save queue, autosave, recovery copies, versions, renaming and downloads.
import type { App } from "./main.ts";
import { Engine, assertEngineSound, diffDocuments } from "@docxtor/core";
import { SaveQueue } from "./save-queue";
import type { Taken } from "./app-shared";
import { inputPending, moveHistory, yieldToInput } from "./app-shared";
import { DOC_EXT, SingleFileFs, baseName, ensurePermission, forgetRecent, join, parentOf, recents } from "./files/fs";
import { isStorageFull, keepStorage } from "./files/db";
import { dropCopy, keepCopy } from "./files/recovery";
import { downloadBytes, h, makeZip, quickPick, t, tn, toast } from "@docxtor/editor";
import { showCrashNotice } from "./failsafe";
import type { Version, VersionKind } from "./files/versions";
import { addVersion } from "./files/versions";
import type { DiffSummary } from "./ui/history-panel";
import type { Backend } from "./files/fs";
import type { ZipEntry } from "@docxtor/editor";

/** Save to where the document lives. `explicit` = the user asked (⌘S / button). */
export async function save(app: App, explicit: boolean) {
  const queue = app.queue;
  if (!app.engine || !app.ref || !queue || app.savedElsewhere) return;
  window.clearTimeout(app.saveTimer);
  const done = await queue.save(explicit);
  // The queue says what happened (the state, and a note when the write failed); an explicit save also keeps a version.
  if (explicit && done.ok && done.snapshot && app.queue === queue) await checkpoint(app, "save", "Saved", false, undefined, done.snapshot.bytes);
}

/** The queue that saves `engine`'s document to wherever the open document lives, from the revision it was saved at. */
export function newQueue(app: App, engine: Engine, savedRevision: number): SaveQueue<Taken> {
  return new SaveQueue<Taken>(
    {
      revision: () => engine.revision,
      // The target is named when the take begins; the bytes are made in steps that let keys through (and the compressing runs in a worker).
      take: async () => {
        const ref = app.ref!;
        return { ...(await engine.saveInSteps(yieldToInput)), ref };
      },
      write: async ({ bytes, ref }) => {
        // The bytes were made before the engine stopped, if it stopped while they were being compressed: not worth the risk, so not written.
        assertEngineSound();
        const b = ref.backend;
        const handle = b instanceof SingleFileFs ? b.handle : "root" in b ? (b as unknown as { root: FileSystemDirectoryHandle }).root : null;
        if (handle && !(await ensurePermission(handle))) throw new Error("permission to write was not granted");
        await b.write(ref.path, bytes);
        // A document kept in the browser: ask the browser not to clear its storage when the disk runs low.
        if (b.id === "library") void keepStorage();
        // What follows every write, wherever it came from.
        void dropCopy(`${b.id}:${ref.path}`);
        if (app.ref === ref) app.rememberRecent();
        // The list of files is drawn again when its tab is shown (it reads every file of the Library), so only a list that is on screen is redrawn here.
        if (app.sidebar.open && app.sidebar.tab === "files") void app.files.refresh();
      },
      onState: (status, error) => {
        if (app.engine !== engine) return;
        app.setState(status === "saved" ? "saved" : status === "saving" ? "saving" : status === "failed" ? "error" : autoSaves(app) ? "dirty" : "unsaved-file");
        if (status === "failed" && isStorageFull(error))
          toast(t("There is no room left in this browser to save the document. Download a copy to keep your work, then make room: delete old documents or empty the trash."), 15000, { label: t("Download a copy"), run: () => void exportAs(app, "docx") });
        else if (status === "failed" && (error as { code?: string } | undefined)?.code === "engine_crashed") showCrashNotice();
        else if (status === "failed") toast(t("Couldn't save: {reason}", { reason: error?.message ?? "" }), 5000);
      },
    },
    savedRevision,
  );
}

/** Whether this document saves by itself after a pause (the Library, storage that autosaves, a folder with Autosave on). */
export function autoSaves(app: App): boolean {
  const b = app.ref?.backend;
  return b?.id === "library" || (!!b && app.isProvided(b) && (b as { autosave?: boolean }).autosave !== false) || app.prefs.autosaveFiles;
}

export async function saveAs(app: App) {
  if (!app.engine) return;
  try {
    const pick = (window as unknown as { showSaveFilePicker(o: object): Promise<FileSystemFileHandle> }).showSaveFilePicker;
    const handle = await pick({
      suggestedName: app.fileName(),
      types: [{ description: t("Word document"), accept: { "application/vnd.openxmlformats-officedocument.wordprocessingml.document": [".docx"] } }],
    });
    const backend = new SingleFileFs(handle);
    const revision = app.engine.revision;
    await backend.write(handle.name, app.engine.save());
    void dropCopy(app.docKey()!);
    app.ref = { backend, path: handle.name };
    app.engine.fileName = handle.name;
    app.setTitle(app.stem());
    app.where = "a file on disk";
    app.queue?.markSaved(revision);
    app.setState(app.dirty ? "unsaved-file" : "saved");
    void dropCopy(app.docKey()!);
    await checkpoint(app, "save", "Saved as a new file");
  } catch (e) {
    if ((e as Error).name !== "AbortError") toast((e as Error).message);
  }
}

export function markDirty(app: App) {
  const q = app.queue;
  if (app.savedElsewhere) return app.fe.refreshToolbar();
  // Only a change to the document counts: the engine's revision tells (an editor event that changed nothing is not unsaved work).
  if (app.engine && q?.dirty) {
    app.dirtySinceVersion = true;
    const auto = autoSaves(app);
    q.touch();
    window.clearTimeout(app.saveTimer);
    if (auto) {
      app.engine.warmSaves();
      app.saveTimer = window.setTimeout(() => autosaveWhenQuiet(app), app.prefs.autosaveSeconds * 1000);
    }
    scheduleRecovery(app, auto);
  }
  app.fe.refreshToolbar();
}

/** The autosave's turn has come: it starts when no key or click is waiting, so it never goes in front of one. */
export function autosaveWhenQuiet(app: App) {
  if (inputPending()) {
    app.saveTimer = window.setTimeout(() => autosaveWhenQuiet(app), 100);
    return;
  }
  void save(app, false);
}

/**
 * While there are changes that are not saved, a copy is kept (see files/recovery.ts): once typing pauses, and at the latest
 * after `max` of steady typing. Where an autosave comes as soon as that, it is the copy.
 */
export function scheduleRecovery(app: App, autosaves: boolean) {
  const { idle, max } = app.recoveryDelays;
  if (autosaves && app.prefs.autosaveSeconds * 1000 <= idle) return;
  const now = Date.now();
  app.recoverySince ||= now;
  window.clearTimeout(app.recoveryTimer);
  app.recoveryTimer = window.setTimeout(() => {
    app.recoveryTimer = 0;
    app.recoverySince = 0;
    void keepRecoveryCopy(app);
  }, Math.min(idle, Math.max(0, app.recoverySince + max - now)));
}

export async function keepRecoveryCopy(app: App) {
  const key = app.docKey();
  if (!key || !app.engine || !app.dirty) return;
  try {
    const engine = app.engine;
    const { bytes } = await engine.saveInSteps(yieldToInput);
    await keepCopy({ key, name: app.fileName(), place: app.where, bytes, words: engine.raw.wordCount(), at: Date.now() });
  } catch {
    /* a copy that can't be kept is no reason to interrupt */
  }
}

/** Record a version of the current document. */
export async function checkpoint(app: App, kind: VersionKind, label: string, force = false, model?: string, bytes?: Uint8Array) {
  const key = app.docKey();
  if (!key || !app.engine) return;
  const engine = app.engine;
  const data = bytes ?? (await engine.saveInSteps(yieldToInput)).bytes;
  const v = await addVersion({ doc: key, at: Date.now(), kind, label, by: kind === "ai" ? (model ?? "AI") : "You", words: engine.raw.wordCount() }, data, force, engine.hasPassword ? engine.savePlain() : undefined);
  if (v || kind !== "open") {
    app.lastVersionAt = Date.now();
    app.dirtySinceVersion = false;
  }
  if (app.sidebar.tab === "history") void app.history.render();
}

export function diff(app: App, older: Uint8Array, newer: Uint8Array | null): DiffSummary | null {
  if (!newer) return null;
  try {
    // Saved versions of a protected document are protected too: compare them as they read.
    const plain = (b: Uint8Array) => (app.engine ? app.engine.plainBytes(b) : b);
    return diffDocuments(plain(older), plain(newer));
  } catch {
    return null;
  }
}

export async function restoreVersion(app: App, v: Version, bytes: Uint8Array) {
  if (!app.ref) return;
  await checkpoint(app, "auto", "Before restoring", true);
  const ref = app.ref;
  // The restored text keeps the protection the document has now, not the one the old version had.
  const keep = app.engine?.password ?? null;
  const plain = await app.unlockBytes(bytes, baseName(ref.path), keep);
  if (!plain) return;
  const engine = Engine.open(plain, baseName(ref.path));
  if (keep) engine.setPassword(keep);
  const shown = app.fe.engine;
  await app.fe.prepare(engine);
  if (app.fe.engine !== shown) {
    engine.dispose();
    return;
  }
  app.load(engine, ref, app.original);
  app.queue?.markUnsaved();
  await save(app, false);
  await checkpoint(app, "restore", `Restored “${v.label}”`, true);
  toast(t("Version restored."));
}

export async function renameCurrent(app: App, value: string) {
  if (!app.ref || !app.engine) return;
  const stem = value.trim();
  if (!stem || stem === app.stem()) {
    app.setTitle(app.stem());
    return;
  }
  const ext = app.fileName().match(DOC_EXT)?.[0] ?? ".docx";
  try {
    const to = await app.ref.backend.rename(app.ref.path, stem + ext);
    onRenamed(app, app.ref.backend, app.ref.path, to);
  } catch (e) {
    toast((e as Error).message);
    app.setTitle(app.stem());
  }
}

export function onRenamed(app: App, backend: Backend, from: string, to: string) {
  if (!app.ref || app.ref.backend.id !== backend.id || app.ref.path !== from) return;
  const oldKey = app.docKey()!;
  app.ref = { backend, path: to };
  if (app.engine) app.engine.fileName = baseName(to);
  app.setTitle(app.stem());
  document.title = `${app.stem()} — Docxtor`;
  app.syncUrl(true);
  void moveHistory(oldKey, app.docKey()!);
  // The new name keeps the pin the old one had.
  void recents()
    .then((all) => all.find((r) => r.key === oldKey)?.pinned)
    .then(async (pinned) => {
      await forgetRecent(oldKey);
      app.rememberRecent(!!pinned);
    });
  void app.files.refresh();
}

export function onRemoved(app: App, backend: Backend, path: string) {
  if (app.ref && app.ref.backend.id === backend.id && (app.ref.path === path || app.ref.path.startsWith(`${path}/`))) {
    // Keep editing the in-memory copy; saving recreates the file.
    app.queue?.markUnsaved();
    toast(t("The open document was moved to the Trash. Press ⌘S to save it again."));
  }
}

/** A document came back from the Trash: the lists show it again. */
export function onRestored(app: App, _backend: Backend, _path: string) {
  void app.files.refresh();
}

/** Download the open document in another format. */
export async function exportAs(app: App, id: string) {
  try {
    const f = await app.fe.export(id);
    download(app, `${app.stem()}.${f.name.split(".").pop()}`, f.bytes, f.mime);
  } catch (e) {
    toast(t("Couldn't export: {reason}", { reason: (e as Error).message }), 5000);
  }
}

/** Pick an export format, then download. */
export async function downloadAs(app: App) {
  const items = app.fe.extensions.exporters.list().map((x) => ({ label: x.label, hint: `.${x.extension}`, id: x.id }));
  const pick = await quickPick(items, { placeholder: t("Download as"), label: t("Download a copy") });
  if (pick) await exportAs(app, pick.id);
}

/** Every document of the Library in one .zip file, in its folders: a backup a person can keep outside the browser. */
export async function downloadAll(app: App) {
  const entries: ZipEntry[] = [];
  const walk = async (dir: string) => {
    for (const n of await app.files.library.list(dir)) {
      if (n.kind === "dir") await walk(n.path);
      else entries.push({ name: n.path, bytes: await app.files.library.read(n.path) });
    }
  };
  await walk("");
  if (!entries.length) return void toast(t("There are no documents in your Library yet."));
  const day = new Date().toISOString().slice(0, 10);
  downloadBytes(makeZip(entries), `${t("Docxtor documents")} ${day}.zip`, "application/zip");
  toast(tn(entries.length, "Downloaded {n} document", "Downloaded {n} documents"));
}

export function download(_app: App, name: string, bytes: Uint8Array, mime = "application/vnd.openxmlformats-officedocument.wordprocessingml.document") {
  const blob = new Blob([bytes as BlobPart], { type: mime });
  const a = h("a", { href: URL.createObjectURL(blob), download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 4000);
}

/** Write the open document to "Copy of {name}" in the same place, then open the copy. */
export async function makeCopy(app: App) {
  if (!app.engine || !app.ref || !app.canManageFile) return;
  const { backend, path } = app.ref;
  try {
    const handle = "root" in backend ? (backend as unknown as { root: FileSystemDirectoryHandle }).root : null;
    if (handle && !(await ensurePermission(handle))) throw new Error(t("permission to write was not granted"));
    const dir = parentOf(path);
    const ext = app.fileName().match(DOC_EXT)?.[0] ?? ".docx";
    const base = t("Copy of {name}", { name: app.stem() });
    let name = base + ext;
    for (let n = 2; await backend.exists(join(dir, name)); n++) name = `${base} (${n})${ext}`;
    await backend.write(join(dir, name), app.engine.save());
    await app.openRef({ backend, path: join(dir, name) }, app.engine.password);
  } catch (e) {
    toast(t("Couldn't make a copy: {reason}", { reason: (e as Error).message }), 5000);
  }
}

/** Put the cursor in the document name and select it; leaving the field does the renaming. */
export function renameDocument(app: App) {
  if (!app.canManageFile) return;
  app.titleInput.focus();
  app.titleInput.select();
}

/** A new document is by the person whose name is in Tools, User information (instead of "Docxtor"). */
export function stampAuthor(app: App) {
  const name = app.fe.fieldOptions.user.name?.trim();
  const engine = app.engine;
  if (!name || !engine) return;
  const author = engine.docProps().author;
  if (!author || author === "Docxtor") app.editor.exec({ op: "set_doc_props", author: name });
}

/** Every format Docxtor can open, for file pickers. */
export function accept(app: App): string {
  const exts = new Set([".docx", ".docm", ".dotx"]);
  for (const i of app.fe.extensions.importers.list()) for (const e of i.extensions) exts.add(`.${e.replace(/^\./, "")}`);
  return [...exts].join(",");
}
