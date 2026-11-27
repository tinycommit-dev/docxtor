// File backends behind one interface:
//  - LibraryFs: an in-browser file system (IndexedDB), works in every browser
//  - FolderFs:  a folder on disk the user connected (File System Access API)
//  - SingleFileFs: one file opened through the picker or dropped (save in place)
import type { StorageWriteOptions, StorageWriteResult } from "@docxtor/extensions";
import { del, get, getAll, put } from "./db";

export interface FsNode {
  kind: "file" | "dir";
  name: string;
  path: string;
  modified?: number;
  size?: number;
}

export interface Backend {
  /** Stable identity used for version history and recents. */
  id: string;
  label: string;
  list(path: string): Promise<FsNode[]>;
  read(path: string): Promise<Uint8Array>;
  write(path: string, bytes: Uint8Array, options?: StorageWriteOptions): Promise<void | StorageWriteResult>;
  mkdir(path: string): Promise<void>;
  rename(path: string, name: string): Promise<string>;
  remove(path: string): Promise<void>;
  exists(path: string): Promise<boolean>;
}

export const DOC_EXT = /\.(docx|docm|dotx)$/i;

export const parentOf = (p: string) => p.slice(0, Math.max(0, p.lastIndexOf("/")));
export const baseName = (p: string) => p.slice(p.lastIndexOf("/") + 1);
export const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);

function sortNodes(n: FsNode[]): FsNode[] {
  return n.sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name, undefined, { numeric: true }) : a.kind === "dir" ? -1 : 1));
}

/** A name that doesn't exist yet in `dir` ("Untitled.docx", "Untitled 2.docx", …). */
export async function uniqueName(fs: Backend, dir: string, name: string): Promise<string> {
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : "";
  let candidate = name;
  for (let n = 2; await fs.exists(join(dir, candidate)); n++) candidate = `${stem} ${n}${ext}`;
  return candidate;
}

// ---------------------------------------------------------------------------
// Library (IndexedDB)
// ---------------------------------------------------------------------------

interface LibEntry {
  path: string;
  kind: "file" | "dir";
  bytes?: Uint8Array;
  modified: number;
  size?: number;
}

export class LibraryFs implements Backend {
  id = "library";
  label = "Library";

  async list(path: string): Promise<FsNode[]> {
    const all = await getAll<LibEntry>("library");
    return sortNodes(
      all
        .filter((e) => parentOf(e.path) === path)
        .map((e) => ({ kind: e.kind, name: baseName(e.path), path: e.path, modified: e.modified, size: e.size })),
    );
  }
  async read(path: string): Promise<Uint8Array> {
    const e = await get<LibEntry>("library", path);
    if (!e?.bytes) throw new Error(`${path} not found`);
    return e.bytes;
  }
  async write(path: string, bytes: Uint8Array): Promise<void> {
    await put("library", { path, kind: "file", bytes, modified: Date.now(), size: bytes.length } satisfies LibEntry);
  }
  async mkdir(path: string): Promise<void> {
    if (!(await this.exists(path))) await put("library", { path, kind: "dir", modified: Date.now() } satisfies LibEntry);
  }
  async exists(path: string): Promise<boolean> {
    return !!(await get("library", path));
  }
  async rename(path: string, name: string): Promise<string> {
    const target = join(parentOf(path), name);
    if (target === path) return path;
    if (await this.exists(target)) throw new Error(`“${name}” already exists`);
    const all = await getAll<LibEntry>("library");
    for (const e of all) {
      if (e.path === path || e.path.startsWith(`${path}/`)) {
        await del("library", e.path);
        await put("library", { ...e, path: target + e.path.slice(path.length) });
      }
    }
    return target;
  }
  async remove(path: string): Promise<void> {
    const all = await getAll<LibEntry>("library");
    for (const e of all) if (e.path === path || e.path.startsWith(`${path}/`)) await del("library", e.path);
  }
}

// ---------------------------------------------------------------------------
// Local folder (File System Access API)
// ---------------------------------------------------------------------------

type Perm = "granted" | "denied" | "prompt";
interface PermHandle {
  queryPermission?(o: { mode: "read" | "readwrite" }): Promise<Perm>;
  requestPermission?(o: { mode: "read" | "readwrite" }): Promise<Perm>;
}

export async function hasPermission(h: FileSystemHandle, mode: "read" | "readwrite" = "readwrite"): Promise<boolean> {
  const p = h as unknown as PermHandle;
  return !p.queryPermission || (await p.queryPermission({ mode })) === "granted";
}

/** Must be called from a user gesture when permission is "prompt". */
export async function ensurePermission(h: FileSystemHandle, mode: "read" | "readwrite" = "readwrite"): Promise<boolean> {
  if (await hasPermission(h, mode)) return true;
  const p = h as unknown as PermHandle;
  return !!p.requestPermission && (await p.requestPermission({ mode })) === "granted";
}

export const canUseFolders = typeof (window as unknown as { showDirectoryPicker?: unknown }).showDirectoryPicker === "function";
export const canPickFiles = typeof (window as unknown as { showOpenFilePicker?: unknown }).showOpenFilePicker === "function";

export async function writeHandle(h: FileSystemFileHandle, bytes: Uint8Array) {
  const w = await (h as unknown as { createWritable(): Promise<FileSystemWritableFileStream> }).createWritable();
  await w.write(bytes as unknown as BufferSource);
  await w.close();
}

export class FolderFs implements Backend {
  id: string;
  label: string;
  constructor(public root: FileSystemDirectoryHandle) {
    this.id = `folder:${root.name}`;
    this.label = root.name;
  }

  private async dir(path: string, create = false): Promise<FileSystemDirectoryHandle> {
    let d = this.root;
    for (const seg of path.split("/").filter(Boolean)) d = await d.getDirectoryHandle(seg, { create });
    return d;
  }
  private async file(path: string, create = false): Promise<FileSystemFileHandle> {
    return (await this.dir(parentOf(path))).getFileHandle(baseName(path), { create });
  }

  async list(path: string): Promise<FsNode[]> {
    const d = await this.dir(path);
    const out: FsNode[] = [];
    for await (const [name, h] of (d as unknown as { entries(): AsyncIterable<[string, FileSystemHandle]> }).entries()) {
      if (name.startsWith(".") || name.startsWith("~$")) continue;
      if (h.kind === "directory") out.push({ kind: "dir", name, path: join(path, name) });
      else if (DOC_EXT.test(name)) {
        const f = await (h as FileSystemFileHandle).getFile().catch(() => null);
        out.push({ kind: "file", name, path: join(path, name), modified: f?.lastModified, size: f?.size });
      }
    }
    return sortNodes(out);
  }
  async read(path: string): Promise<Uint8Array> {
    return new Uint8Array(await (await (await this.file(path)).getFile()).arrayBuffer());
  }
  async write(path: string, bytes: Uint8Array): Promise<void> {
    await writeHandle(await this.file(path, true), bytes);
  }
  async mkdir(path: string): Promise<void> {
    await this.dir(path, true);
  }
  async exists(path: string): Promise<boolean> {
    try {
      await this.file(path);
      return true;
    } catch {
      try {
        await this.dir(path);
        return true;
      } catch {
        return false;
      }
    }
  }
  async rename(path: string, name: string): Promise<string> {
    const target = join(parentOf(path), name);
    if (target === path) return path;
    if (await this.exists(target)) throw new Error(`“${name}” already exists`);
    const h = await this.file(path).catch(() => null);
    if (!h) throw new Error("Only documents can be renamed here");
    const movable = h as unknown as { move?(name: string): Promise<void> };
    if (movable.move) await movable.move(name);
    else {
      await this.write(target, await this.read(path));
      await this.remove(path);
    }
    return target;
  }
  async remove(path: string): Promise<void> {
    await (await this.dir(parentOf(path))).removeEntry(baseName(path), { recursive: true });
  }
}

// ---------------------------------------------------------------------------
// A single file handle (picker / drag-and-drop)
// ---------------------------------------------------------------------------

export class SingleFileFs implements Backend {
  id: string;
  label: string;
  constructor(public handle: FileSystemFileHandle) {
    this.id = `file:${handle.name}`;
    this.label = handle.name;
  }
  async list(): Promise<FsNode[]> {
    return [{ kind: "file", name: this.handle.name, path: this.handle.name }];
  }
  async read(): Promise<Uint8Array> {
    return new Uint8Array(await (await this.handle.getFile()).arrayBuffer());
  }
  async write(_path: string, bytes: Uint8Array): Promise<void> {
    await writeHandle(this.handle, bytes);
  }
  async mkdir(): Promise<void> {
    throw new Error("Not supported for a single file");
  }
  async exists(path: string): Promise<boolean> {
    return path === this.handle.name;
  }
  async rename(): Promise<string> {
    throw new Error("Rename this file from your system's file manager");
  }
  async remove(): Promise<void> {
    throw new Error("Not supported for a single file");
  }
}

// ---------------------------------------------------------------------------
// Remembered handles & recents
// ---------------------------------------------------------------------------

export async function rememberFolder(h: FileSystemDirectoryHandle | null) {
  if (h) await put("handles", h, "folder");
  else await del("handles", "folder");
}

export async function rememberedFolder(): Promise<FileSystemDirectoryHandle | null> {
  return (await get<FileSystemDirectoryHandle>("handles", "folder").catch(() => undefined)) ?? null;
}

export interface Recent {
  key: string;
  name: string;
  backend: "library" | "folder" | "file" | "storage";
  path: string;
  handle?: FileSystemFileHandle;
  folder?: string;
  /** For `storage`: the id of the storage provider (an extension). */
  provider?: string;
  at: number;
  /** The first lines of the document, for the start screen's page thumbnails. */
  preview?: PreviewLine[];
  /** The document is protected with a password: no text is kept, and its thumbnail shows a padlock. */
  locked?: boolean;
  /** Pinned: listed first on the start screen and in Open recent, and never dropped to make room. */
  pinned?: boolean;
}

export interface PreviewLine {
  /** `title`, `h` (heading), `li` (list item) or `p`. */
  k: "title" | "h" | "li" | "p";
  t: string;
}

export async function addRecent(r: Recent) {
  // Opening a pinned document again keeps it pinned.
  const before = await get<Recent>("recent", r.key).catch(() => undefined);
  await put("recent", before?.pinned ? { ...r, pinned: true } : r).catch(() => undefined);
  const all = (await getAll<Recent>("recent").catch(() => [])).filter((x) => !x.pinned).sort((a, b) => b.at - a.at);
  for (const old of all.slice(12)) await del("recent", old.key).catch(() => undefined);
}

/** The pinned documents first (newest pinned first), then the rest, newest first. */
export async function recents(): Promise<Recent[]> {
  return (await getAll<Recent>("recent").catch(() => [])).sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned) || b.at - a.at);
}

/** Pin a document or let go of it. `fallback` makes the record when the document has never been opened here. */
export async function setPinned(key: string, pinned: boolean, fallback?: Recent): Promise<void> {
  const r = (await get<Recent>("recent", key).catch(() => undefined)) ?? fallback;
  if (!r) return;
  await put("recent", { ...r, pinned: pinned || undefined }).catch(() => undefined);
}

export async function forgetRecent(key: string) {
  await del("recent", key).catch(() => undefined);
}
