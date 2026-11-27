// Saving: the bytes of the document, saved in steps, and the fidelity check.
import type { Engine } from "./engine.ts";
import { ZIP_ON_PAGE_BELOW, fidelityOffThread, toDeflate, warmZipWorker, zipOffThread } from "./zip-client.ts";
import type { FidelityReport } from "./types.ts";

/**
 * The document as .docx bytes. Parts that were not edited are written back byte for byte. When the document was
 * edited since it was opened or last saved, its "last changed" date (`dcterms:modified`) becomes `now`, and a new
 * document gets its "created" date too, as Word does.
 */
export function save(host: Engine, now: Date = new Date()): Uint8Array {
  return host.protectBytes(savePlain(host, now));
}

/**
 * `save()` in steps, for a page that is being typed in: the dates and statistics (one call), a pause that lets keys that have
 * arrived run (`pause`), the document's parts (a second call), and the compressing in a worker. No call holds the page
 * longer than a few dozen milliseconds in a book of 1,000 pages, where `save()` holds it for 170. The bytes are the ones
 * `save()` gives; `revision` is the document's at the start, so a key typed during the steps leaves the document unsaved.
 */
export async function saveInSteps(host: Engine, pause: () => Promise<void> = () => Promise.resolve(), now: Date = new Date()): Promise<{ revision: number; bytes: Uint8Array }> {
  const revision = host.revision;
  if (host.hasPassword) {
    // Encrypting needs the whole file at once.
    await pause();
    return { revision, bytes: save(host, now) };
  }
  const info = typeof host.saveInfo === "function" ? host.saveInfo() : host.saveInfo;
  host.raw.setSaveInfo(info.user ?? "", info.pages ?? 0, info.lines ?? 0);
  host.raw.stamp(now.toISOString().replace(/\.\d+Z$/, "Z"));
  await pause();
  const packed = gatherParts(host);
  host.packedBytes = toDeflate(packed);
  return { revision, bytes: await zipOffThread(packed, () => gatherParts(host)) };
}

/**
 * How much of `original` (the file as it was opened) a save now keeps, part by part: the fidelity report, worked out off the main thread
 * (the page only gathers the parts; in a long book the zip and the comparison take a few hundred milliseconds). Without a password: the
 * comparison is of the plain package.
 */
export async function fidelity(host: Engine, original: Uint8Array): Promise<FidelityReport> {
  const packed = gatherParts(host);
  host.packedBytes = toDeflate(packed);
  return JSON.parse(await fidelityOffThread(original, packed, () => gatherParts(host)));
}

/** The parts, not compressed yet (`savePacked`), handed over by the engine in pieces and joined here: the engine keeps no copy of them. */
export function gatherParts(host: Engine): Uint8Array {
  const parts: Uint8Array[] = [];
  host.raw.savePackedPieces(parts);
  if (parts.length === 1) return parts[0];
  let n = 0;
  for (const p of parts) n += p.length;
  const out = new Uint8Array(n);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/**
 * Start the worker that compresses saves, so the next save does not wait for it to load: only for a document whose parts are big enough to
 * be zipped there (a small one is zipped on the page, see `ZIP_ON_PAGE_BELOW`), as the last save or fidelity check found them.
 */
export function warmSaves(host: Engine): void {
  if (host.packedBytes >= ZIP_ON_PAGE_BELOW) warmZipWorker();
}

/** The same as `save()` but never protected with a password: the .docx zip itself, for comparing and checking. */
export function savePlain(host: Engine, now: Date = new Date()): Uint8Array {
  const info = typeof host.saveInfo === "function" ? host.saveInfo() : host.saveInfo;
  host.raw.setSaveInfo(info.user ?? "", info.pages ?? 0, info.lines ?? 0);
  return host.raw.saveStamped(now.toISOString().replace(/\.\d+Z$/, "Z"));
}
