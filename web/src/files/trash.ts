// The trash: deleting a document from the Library or a folder keeps a copy here for 30 days, so it can be put back.
// A copy of the bytes is kept (not a flag on the file), because a folder on disk has no trash of its own that a page can reach.
import { t } from "@docxtor/editor";
import { del, getAll, put } from "./db";
import { baseName, join, parentOf, uniqueName, type Backend, type FsNode } from "./fs";

/** How long a deleted document stays in the trash. */
export const KEEP_DAYS = 30;
const DAY = 86_400_000;

export interface TrashItem {
  id?: number;
  /** The backend it was deleted from (`library`, `folder:Reports`...), to put it back in the same place. */
  backend: string;
  /** Where that is, in words ("Library", the folder's name). */
  place: string;
  path: string;
  name: string;
  bytes: Uint8Array;
  size: number;
  /** When it was deleted (ms). */
  at: number;
}

/** Everything in the trash, newest first. Items older than the keeping time are dropped first. */
export async function listTrash(now = Date.now()): Promise<TrashItem[]> {
  const all = await getAll<TrashItem>("trash").catch(() => []);
  const keep: TrashItem[] = [];
  for (const item of all) {
    if (now - item.at > KEEP_DAYS * DAY) await del("trash", item.id!).catch(() => undefined);
    else keep.push(item);
  }
  return keep.sort((a, b) => b.at - a.at);
}

/** Days left before an item is dropped. */
export function daysLeft(item: TrashItem, now = Date.now()): number {
  return Math.max(0, Math.ceil(KEEP_DAYS - (now - item.at) / DAY));
}

/** Keep a copy of a document that is about to be deleted. */
export async function toTrash(fs: Backend, path: string, now = Date.now()): Promise<TrashItem> {
  const bytes = await fs.read(path);
  const item: TrashItem = { backend: fs.id, place: fs.label, path, name: baseName(path), bytes, size: bytes.length, at: now };
  item.id = (await put("trash", item)) as number;
  return item;
}

/** Every document in a folder and below it, as nodes. */
async function filesUnder(fs: Backend, dir: string): Promise<FsNode[]> {
  const out: FsNode[] = [];
  for (const n of await fs.list(dir)) {
    if (n.kind === "dir") out.push(...(await filesUnder(fs, n.path)));
    else out.push(n);
  }
  return out;
}

/** Delete a document or a folder, keeping a copy of each document in the trash. */
export async function deleteToTrash(fs: Backend, path: string, kind: "file" | "dir"): Promise<number> {
  const files = kind === "dir" ? await filesUnder(fs, path) : [{ kind: "file", name: baseName(path), path } as FsNode];
  for (const f of files) await toTrash(fs, f.path);
  await fs.remove(path);
  return files.length;
}

/**
 * Put an item back where it was deleted from (under another name when that name is taken now). Returns the path, or
 * throws a plain message when the place isn't there any more.
 */
export async function restore(item: TrashItem, backend: Backend | undefined): Promise<string> {
  if (!backend) throw new Error(t("“{place}” isn't connected right now.", { place: item.place }));
  const dir = parentOf(item.path);
  // The folders the document was in may be gone.
  let at = "";
  for (const seg of dir.split("/").filter(Boolean)) {
    at = join(at, seg);
    await backend.mkdir(at);
  }
  const name = await uniqueName(backend, dir, item.name);
  await backend.write(join(dir, name), item.bytes);
  await del("trash", item.id!);
  return join(dir, name);
}

export async function forever(item: TrashItem): Promise<void> {
  await del("trash", item.id!);
}

export async function emptyTrash(): Promise<void> {
  for (const item of await getAll<TrashItem>("trash").catch(() => [])) await del("trash", item.id!);
}
