// Version history per document: a timeline of snapshots (opened, saved, checkpoints,
// before/after each AI run, named versions, restores). Records and bytes are
// stored separately so listing the timeline is instant.
import { del, get, getAllByIndex, put } from "./db";

export type VersionKind = "open" | "save" | "auto" | "ai" | "before-ai" | "named" | "restore";

export interface Version {
  id?: number;
  doc: string;
  at: number;
  kind: VersionKind;
  label: string;
  by: string;
  words: number;
  hash: string;
  size: number;
}

const KEEP = 150;

/** Only tells "the same bytes as the last version" apart, so it needs no secrecy. */
async function sha(bytes: Uint8Array): Promise<string> {
  // `crypto.subtle` is missing on a page served without HTTPS (a copy of Docxtor on a home server): there two FNV-1a lanes stand in,
  // marked so they never equal a SHA, rather than version history quietly recording nothing.
  if (!globalThis.crypto?.subtle) return fnv(bytes);
  const d = await crypto.subtle.digest("SHA-256", bytes as unknown as BufferSource);
  return Array.from(new Uint8Array(d).slice(0, 12), (b) => b.toString(16).padStart(2, "0")).join("");
}

function fnv(bytes: Uint8Array): string {
  let a = 0x811c9dc5;
  let b = 0x01000193 ^ bytes.length;
  for (let i = 0; i < bytes.length; i++) {
    a = Math.imul(a ^ bytes[i], 0x01000193);
    b = Math.imul(b ^ bytes[i], 0x5bd1e995) ^ (b >>> 15);
  }
  return `fnv:${(a >>> 0).toString(16).padStart(8, "0")}${(b >>> 0).toString(16).padStart(8, "0")}`;
}

export async function listVersions(doc: string): Promise<Version[]> {
  return (await getAllByIndex<Version>("versions", "doc", doc).catch(() => [])).sort((a, b) => b.at - a.at);
}

export async function versionBytes(id: number): Promise<Uint8Array | null> {
  return (await get<Uint8Array>("blobs", id).catch(() => undefined)) ?? null;
}

/**
 * Record a version. Identical content to the latest version is skipped unless
 * `force` (named versions and AI results are always meaningful as labels).
 */
export async function addVersion(v: Omit<Version, "hash" | "size" | "id">, bytes: Uint8Array, force = false, sameAs?: Uint8Array): Promise<Version | null> {
  try {
    // A protected file is different bytes every time it is written; `sameAs` is its plain content, so unchanged text isn't recorded twice.
    const hash = await sha(sameAs ?? bytes);
    const existing = await listVersions(v.doc);
    if (!force && existing[0]?.hash === hash) return null;
    const rec: Version = { ...v, hash, size: bytes.length };
    const id = (await put("versions", rec)) as number;
    await put("blobs", bytes, id);
    // Prune: keep named/AI versions longer than routine checkpoints.
    const routine = existing.filter((e) => e.kind === "auto" || e.kind === "open" || e.kind === "save");
    if (existing.length >= KEEP) {
      for (const old of routine.slice(Math.floor(KEEP / 2))) {
        await del("versions", old.id!);
        await del("blobs", old.id!);
      }
    }
    return { ...rec, id };
  } catch {
    return null;
  }
}

export async function renameVersion(v: Version, label: string) {
  await put("versions", { ...v, label, kind: "named" });
}
