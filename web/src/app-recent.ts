// Recent documents and pins, recovered copies, comparing and side by side, and the document details.
import type { App } from "./main.ts";
import type { Backend, FsNode, Recent } from "./files/fs";
import { DOC_EXT, SingleFileFs, addRecent, baseName, ensurePermission, parentOf, recents, setPinned, uniqueName, writeHandle } from "./files/fs";
import { previewLines } from "./start-screen";
import { currentLocale, quickPick, t, toast } from "@docxtor/editor";
import type { RecoveryCopy } from "./files/recovery";
import { dropCopy, lostCopies } from "./files/recovery";
import { openRecovered } from "./ui/recovered";
import { openCompare } from "./ui/compare";
import type { SideDoc } from "./side-by-side";
import { fidelityReport } from "@docxtor/core";
import { openDocDetails } from "./ui/doc-details";

/** Record the open document in Recents, with a text preview of its first page. */
export function rememberRecent(app: App, pinned = false) {
  const ref = app.ref;
  if (!ref || !app.engine) return;
  const r: Recent = {
    key: `${ref.backend.id}:${ref.path}`,
    name: baseName(ref.path),
    backend: ref.backend.id === "library" ? "library" : ref.backend.id.startsWith("folder:") ? "folder" : app.isProvided(ref.backend) ? "storage" : "file",
    path: ref.path,
    at: Date.now(),
    // A protected document's text must not land in storage in the clear: its tile shows a padlock instead.
    preview: app.engine.hasPassword ? [] : previewLines(app.engine.raw.docscriptHead(16)),
    locked: app.engine.hasPassword || undefined,
    // A document that is pinned keeps its pin (addRecent also keeps the pin of the record already there).
    pinned: pinned || undefined,
  };
  if (ref.backend instanceof SingleFileFs) r.handle = ref.backend.handle;
  if (r.backend === "folder") r.folder = ref.backend.label;
  if (r.backend === "storage") r.provider = ref.backend.id;
  void addRecent(r).then(async () => {
    app.pinned = !!(await recents()).find((x) => x.key === r.key)?.pinned;
    app.recentListeners.forEach((fn) => fn());
  });
}

/** Called after the list of recent documents changes (a document opened, saved or renamed). */
export function onRecentsChange(app: App, fn: () => void) {
  app.recentListeners.add(fn);
  return { dispose: () => void app.recentListeners.delete(fn) };
}

/** Where a recent document lives, in words. */
export function recentPlace(app: App, r: Recent): string {
  return r.backend === "library"
    ? t("Library")
    : r.backend === "folder"
      ? (r.folder ?? t("Folder"))
      : r.backend === "storage"
        ? (app.fe.extensions.storage.list().find((p) => p.id === r.provider)?.label ?? t("Elsewhere"))
        : t("On disk");
}

/** Open one of the recent documents by its key (from the Open recent menu). */
export async function openRecentKey(app: App, key: string) {
  const r = (await recents()).find((x) => x.key === key);
  if (r) await app.openRecent(r);
}

/** The Recent record of a document, for one that was never opened here. */
export function recentFor(app: App, backend: Backend, path: string): Recent {
  const provided = app.isProvided(backend);
  const kind = backend.id === "library" ? "library" : backend.id.startsWith("folder:") ? "folder" : provided ? "storage" : "file";
  const r: Recent = { key: `${backend.id}:${path}`, name: baseName(path), backend: kind, path, at: Date.now() };
  if (kind === "folder") r.folder = backend.label;
  if (kind === "storage") r.provider = backend.id;
  if (backend instanceof SingleFileFs) r.handle = backend.handle;
  return r;
}

/** Pin a document to the start screen or let go of it. Returns whether it is pinned now. */
export async function togglePinOf(app: App, backend: Backend, path: string): Promise<boolean> {
  const key = `${backend.id}:${path}`;
  const now = !(await recents()).find((r) => r.key === key)?.pinned;
  await setPinned(key, now, recentFor(app, backend, path));
  if (app.docKey() === key) app.pinned = now;
  app.recentListeners.forEach((fn) => fn());
  toast(now ? t("“{name}” is pinned to the start screen.", { name: baseName(path).replace(DOC_EXT, "") }) : t("“{name}” is no longer pinned.", { name: baseName(path).replace(DOC_EXT, "") }));
  return now;
}

/** File, Pin to the start screen. */
export async function togglePin(app: App) {
  if (app.ref) await togglePinOf(app, app.ref.backend, app.ref.path);
}

/** Make a recovered copy a document of its own in the Library and open it. */
export async function openRecoveredCopy(app: App, c: RecoveryCopy) {
  const name = await uniqueName(app.files.library, "", t("Recovered {name}", { name: c.name }));
  await app.files.library.write(name, c.bytes);
  await dropCopy(c.key);
  await app.openRef({ backend: app.files.library, path: name });
  await countRecoverable(app);
}

export async function countRecoverable(app: App) {
  app.recoverable = (await lostCopies()).length;
}

/** File, Trash: the navigator's Files view with the trash list open. */
export async function showTrash(app: App) {
  app.showPanel("files");
  await app.files.showTrash();
}

/** File, Recovered documents. */
export function showRecovered(app: App) {
  openRecovered(
    {
      list: async () => {
        const copies = await lostCopies();
        app.recoverable = copies.length;
        return copies;
      },
      open: (c) => openRecoveredCopy(app, c),
      discard: async (c) => {
        await dropCopy(c.key);
        await countRecoverable(app);
      },
    },
    () => app.engine && app.editor.focus(),
  );
}

/** At start: when a page closed before its changes were saved, say so and offer them back. */
export async function offerRecovery(app: App) {
  await countRecoverable(app);
  if (app.recoverable) showRecovered(app);
}

/**
 * Tools, Compare documents: pick another version, then show the differences. Combine documents is the same with the
 * open document as the original and the one picked as the revised version. How they are compared is up to the
 * comparers extensions register (Docxtor's own is the built-in "Compare and combine").
 */
export async function compareDocuments(app: App, mode: "compare" | "combine" = "compare") {
  const engine = app.fe.engine;
  if (!engine) return;
  const host = app.fe.extensions;
  const job = mode === "combine" ? "combine" : "redline";
  const view = host.comparerFor("compare");
  const make = host.comparerFor(job);
  if (!view || !make) {
    toast(t("No way to compare documents is on. Turn one on under Extensions, Manage extensions."), 5000);
    return;
  }
  const current = { name: engine.fileName || app.fileName(), bytes: engine.savePlain() };
  const picked = await pickOtherVersion(app, mode);
  if (!picked) return;
  // A protected document is compared as it reads, then asks for its password.
  const bytes = await app.unlockBytes(picked.bytes, picked.name, engine.password);
  if (!bytes) return;
  const other = { name: picked.name, bytes };
  openCompare({
    mode,
    current,
    other,
    author: "Docxtor",
    compare: (newer, older) => view.compare(newer.bytes, older.bytes),
    makeDocument: (newer, older, author) => (mode === "combine" ? make.combine!(newer.bytes, older.bytes, author) : make.redline!(newer.bytes, older.bytes, author)),
    onClose: () => app.editor.focus(),
    openRedline: async (bytes, name) => {
      const final = await uniqueName(app.files.library, "", name);
      // The comparison of a protected document is protected the same way.
      await app.files.library.write(final, engine.protectBytes(bytes));
      await app.openRef({ backend: app.files.library, path: final }, engine.password);
    },
  });
}

/**
 * Choose another document: one from the Library or the recent ones, or a file from the computer. With it comes how to
 * save it back where it came from (none for a file from the computer), for View side by side.
 */
export async function pickOtherVersion(app: App, mode: "compare" | "combine" | "side" = "compare"): Promise<SideDoc | null> {
  const open = app.docKey();
  const walk = async (dir: string): Promise<FsNode[]> => {
    const out: FsNode[] = [];
    for (const n of await app.files.library.list(dir)) {
      if (n.kind === "dir") out.push(...(await walk(n.path)));
      else if (DOC_EXT.test(n.name)) out.push(n);
    }
    return out;
  };
  type Item = { label: string; detail: string; read: () => Promise<SideDoc | null> };
  const items: Item[] = [];
  const seen = new Set<string>([open ?? ""]);
  const lib = async (path: string): Promise<SideDoc> => ({ key: `library:${path}`, name: baseName(path), bytes: await app.files.library.read(path), write: (b) => app.files.library.write(path, b) });
  for (const r of await recents()) {
    if (seen.has(r.key)) continue;
    seen.add(r.key);
    items.push({
      label: r.name.replace(DOC_EXT, ""),
      detail: recentPlace(app, r),
      read: async () => {
        const bytes = await app.readRecent(r);
        return bytes ? { key: r.key, name: r.name, bytes, write: writerFor(app, r) } : null;
      },
    });
  }
  for (const n of (await walk("")).sort((a, b) => a.path.localeCompare(b.path))) {
    const key = `library:${n.path}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const dir = parentOf(n.path);
    items.push({ label: n.name.replace(DOC_EXT, ""), detail: dir ? `${t("Library")}, ${dir}` : t("Library"), read: () => lib(n.path) });
  }
  items.push({ label: t("From your computer…"), detail: t("Choose a file"), read: () => app.pickFileFromComputer() });
  const pick =
    mode === "combine"
      ? await quickPick(items, { placeholder: t("Choose the revised version to combine with this document"), label: t("Combine documents") })
      : mode === "side"
        ? await quickPick(items, { placeholder: t("Choose the document to show beside this one"), label: t("View side by side") })
        : await quickPick(items, { placeholder: t("Choose the version to compare with"), label: t("Compare documents") });
  if (!pick) return null;
  try {
    return await pick.read();
  } catch (e) {
    toast(t("Couldn't open that document: {reason}", { reason: (e as Error).message }), 5000);
    return null;
  }
}

/** How to save a recent document back where it is; none where Docxtor can't write to it. */
export function writerFor(app: App, r: Recent): ((bytes: Uint8Array) => Promise<unknown>) | undefined {
  if (r.backend === "library") return (b) => app.files.library.write(r.path, b);
  if (r.backend === "file" && r.handle) {
    const handle = r.handle;
    return async (b) => {
      if (!(await ensurePermission(handle))) throw new Error(t("permission to write was not granted"));
      await writeHandle(handle, b);
    };
  }
  if (r.backend === "storage") {
    const p = r.provider ? app.provider(r.provider) : undefined;
    return p ? (b) => p.write(r.path, b) : undefined;
  }
  if (r.backend === "folder" && app.files.folder && app.files.folder.label === r.folder) {
    const folder = app.files.folder;
    return (b) => folder.write(r.path, b);
  }
  return undefined;
}

/** View side by side: another document beside this one; again, it closes. */
export async function viewSideBySide(app: App) {
  if (app.side.on) return void (await app.side.close());
  const doc = await pickOtherVersion(app, "side");
  if (doc) await app.side.open(doc);
}

/** The Document details dialog. */
export function showDetails(app: App) {
  if (!app.engine || !app.ref) return;
  const saved = app.engine.save();
  const plain = app.engine.hasPassword ? app.engine.savePlain() : saved;
  let fidelity = t("Created in Docxtor");
  if (app.original) {
    try {
      const score = Number(fidelityReport(app.original, plain).score) / 100;
      const opts = { style: "percent", minimumFractionDigits: 1, maximumFractionDigits: 1 } as const;
      try {
        fidelity = new Intl.NumberFormat(currentLocale(), opts).format(score);
      } catch {
        fidelity = new Intl.NumberFormat(undefined, opts).format(score);
      }
    } catch {
      fidelity = t("Couldn't measure");
    }
  }
  openDocDetails(app.editor, {
    name: app.fileName(),
    where: app.where,
    bytes: saved.length,
    fonts: app.engine.fontsUsed().map((f) => f.family),
    offered: app.fe.extensions.fonts.list().map((f) => f.family),
    fidelity,
    user: app.fe.fieldOptions.user.name?.trim() || undefined,
  });
}
