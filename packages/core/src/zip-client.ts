// The page's side of `zip-worker.ts`: hand it a document's packed parts, get the .docx zip back. Without Worker (Node,
// a host that blocks workers), or when the worker fails, the zip is made right here, which is the same bytes, only slower
// for the page.
import { fidelityReport, zipPacked } from "../wasm/docxtor.js";
import { engineModule } from "./engine-module.ts";
import type { ZipInit, ZipRequest, ZipResponse } from "./zip-worker.ts";

/** A worker nobody has asked for this long is let go: its copy of the engine holds memory the size of the book. */
const IDLE_MS = 10_000;
/**
 * Parts to compress smaller than this are zipped on the page: a few milliseconds there, where the worker's own copy of the engine costs about
 * 110 MB for as long as it runs, when it compiled its own (it now shares the page's compiled engine and still costs about 40 MB while a document
 * is being edited, since every autosave keeps it alive; a blank document's page held 232 MB with it, 114 without). Only the parts that are
 * compressed count (`toDeflate`): a picture is stored as it is, a copy. Measured in Node: a 100-page thesis has 0.55 MB to
 * compress and 4.4 MB of pictures, 9 ms; War and Peace 4.4 MB to compress, 91 ms; a 979-page thesis 5.4 MB, 66 ms.
 */
export const ZIP_ON_PAGE_BELOW = 1 << 20;

/**
 * How many bytes of `packed` will be compressed: the parts not marked stored. The packed form (`Package::pack_pieces`) is, per part, a stored
 * flag (1 byte), the name's length (u32, little-endian), the name, the data's length (u32) and the data. Only the headers are read.
 */
export function toDeflate(packed: Uint8Array): number {
  const dv = new DataView(packed.buffer, packed.byteOffset, packed.byteLength);
  let n = 0;
  for (let i = 0; i < packed.length; ) {
    if (i + 9 > packed.length) return packed.length;
    const stored = packed[i] === 1;
    const nameLen = dv.getUint32(i + 1, true);
    if (i + 9 + nameLen > packed.length) return packed.length;
    const dataLen = dv.getUint32(i + 5 + nameLen, true);
    // Cut short: all of it counts, so a broken buffer goes where a big one would and fails there the same way.
    if (i + 9 + nameLen + dataLen > packed.length) return packed.length;
    if (!stored) n += dataLen;
    i += 9 + nameLen + dataLen;
  }
  return n;
}

let worker: Worker | null = null;
let broken = false;
let seq = 0;
let idleTimer: ReturnType<typeof setTimeout> | undefined;
const waiting = new Map<number, { resolve(b: Uint8Array | string): void; reject(e: Error): void }>();

function start(): Worker | null {
  if (worker || broken || typeof Worker === "undefined") return worker;
  try {
    const w = new Worker(new URL("./zip-worker.ts", import.meta.url), { type: "module" });
    w.onmessage = (e: MessageEvent<ZipResponse>) => {
      const r = e.data;
      const p = waiting.get(r.id);
      if (!p) return;
      waiting.delete(r.id);
      if ("error" in r) p.reject(new Error(r.error));
      else if ("report" in r) p.resolve(r.report);
      else p.resolve(new Uint8Array(r.bytes));
    };
    w.onerror = () => stop(true);
    worker = w;
    // The page's compiled engine goes first: the worker shares its code and does not compile its own.
    const module = engineModule();
    if (module) w.postMessage({ module } satisfies ZipInit);
  } catch {
    broken = true;
  }
  return worker;
}

function stop(fail: boolean) {
  worker?.terminate();
  worker = null;
  if (fail) broken = true;
  for (const p of waiting.values()) p.reject(new Error("The save worker stopped"));
  waiting.clear();
}

/** Start the worker (and let it load the engine) so the first save does not wait for that. */
export function warmZipWorker(): void {
  start();
}

/** The packed parts' own buffer, to move to the worker without a copy. */
function own(b: Uint8Array): ArrayBuffer {
  return b.byteOffset === 0 && b.byteLength === b.buffer.byteLength ? (b.buffer as ArrayBuffer) : (b.slice().buffer as ArrayBuffer);
}

/** Ask the worker; `here` does the same on the page when there is no worker or it fails. */
async function ask<T extends Uint8Array | string>(packed: Uint8Array, original: Uint8Array | undefined, here: () => T, moved: () => void): Promise<T> {
  if (toDeflate(packed) < ZIP_ON_PAGE_BELOW) return here();
  const w = start();
  if (!w) return here();
  clearTimeout(idleTimer);
  try {
    const id = ++seq;
    const buf = own(packed);
    if (buf === packed.buffer) moved();
    // The original stays the caller's: the worker gets a copy.
    const orig = original ? (original.slice().buffer as ArrayBuffer) : undefined;
    return (await new Promise<Uint8Array | string>((resolve, reject) => {
      waiting.set(id, { resolve, reject });
      w.postMessage({ id, packed: buf, original: orig } satisfies ZipRequest, orig ? [buf, orig] : [buf]);
    })) as T;
  } catch {
    // The worker could not do it: the page does.
    return here();
  } finally {
    idleTimer = setTimeout(() => stop(false), IDLE_MS);
    (idleTimer as { unref?: () => void }).unref?.();
  }
}

/**
 * The .docx zip of `packed` (from `DocxtorDoc.savePackedPieces`, joined), made off the main thread where it can be. `packed` moves to the worker; when the worker
 * fails after that, `again` gives the parts once more for the page to zip.
 */
export function zipOffThread(packed: Uint8Array, again: () => Uint8Array = () => packed): Promise<Uint8Array> {
  let sent = false;
  return ask(packed, undefined, () => zipPacked(sent ? again() : packed), () => (sent = true));
}

/**
 * The fidelity report (JSON) of `original` against the .docx `packed` makes, worked out off the main thread where it can be: in a long book the
 * zip and the comparison take a few hundred milliseconds. `again` as for `zipOffThread`.
 */
export function fidelityOffThread(original: Uint8Array, packed: Uint8Array, again: () => Uint8Array = () => packed): Promise<string> {
  let sent = false;
  return ask(packed, original, () => fidelityReport(original, zipPacked(sent ? again() : packed)), () => (sent = true));
}
