// Pictures and other media parts as object URLs: making them, letting unused ones go, and closing the document.
import type { Engine } from "./engine.ts";
import { engineCrash } from "./crash.ts";

/** Add a picture; throws "This document is read-only." while the access is "view". */
export function addMedia(host: Engine, ext: string, bytes: Uint8Array): string {
  return host.raw.addMedia(ext, bytes);
}

/**
 * Object URL for a media part (cached). A picture linked at a web address is not fetched for you: it comes back
 * empty, since asking the address tells whoever runs it that the document was opened. The editor shows those
 * only after the person says so.
 */
export function mediaUrl(host: Engine, path: string): string {
  if (/^data:/.test(path)) return path;
  if (/^https?:/i.test(path)) return "";
  const gen = host.raw.mediaGen(path);
  const cached = host.mediaUrls.get(path);
  if (cached) {
    if (cached.gen === gen) return cached.url;
    // The part was replaced, or taken out and its name used again for another picture: the URL shows the old one.
    URL.revokeObjectURL(cached.url);
    host.mediaUrls.delete(path);
  }
  if (host.mediaFailed.get(path) === gen) return "";
  host.mediaFailed.delete(path);
  let bytes = host.raw.media(path);
  if (!bytes) return "";
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  let type =
    ({ png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", svg: "image/svg+xml", webp: "image/webp", bmp: "image/bmp" } as Record<string, string>)[ext] ??
    "application/octet-stream";
  // TIFF, EMF and WMF are shown as the PNG or SVG the engine makes of them; the file keeps the original.
  const shown = host.raw.mediaDisplay(path);
  if (shown.length) {
    // A format the engine could not convert has nothing to show.
    if (shown[0] === 0) {
      host.mediaFailed.set(path, gen);
      return "";
    }
    type = shown[0] === 1 ? "image/png" : "image/svg+xml";
    bytes = shown.subarray(1);
  }
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type }));
  host.mediaUrls.set(path, { url, gen });
  if (host.mediaUrls.size > host.sweepAt) sweepWhenIdle(host);
  return url;
}

/**
 * Let go of the object URLs of pictures the document no longer uses (deleted, or inserted and undone). The engine
 * keeps those pictures so undo can bring them back; the next `mediaUrl` for one makes a new URL. The newest few
 * URLs are kept whatever, since a dialog may be showing a picture that is not in the document yet. Runs by itself
 * when the URLs pile up (see `sweepWhenIdle`), never on every edit, since it looks through the whole document.
 */
export function releaseUnusedMedia(host: Engine): void {
  if (host.disposed || engineCrash()) return;
  const unused = new Set<string>(JSON.parse(host.raw.unusedMedia()));
  const keepNewest = 8;
  // A URL made for a part that has since been replaced or taken out shows a picture that is no longer there.
  for (const [path, m] of [...host.mediaUrls]) {
    if (m.gen === host.raw.mediaGen(path)) continue;
    URL.revokeObjectURL(m.url);
    host.mediaUrls.delete(path);
  }
  const paths = [...host.mediaUrls.keys()];
  for (const path of paths.slice(0, Math.max(0, paths.length - keepNewest))) {
    if (!unused.has(path)) continue;
    URL.revokeObjectURL(host.mediaUrls.get(path)!.url);
    host.mediaUrls.delete(path);
  }
  // Try again when the URLs have doubled, so a document that really uses many pictures is not searched every time.
  host.sweepAt = Math.max(16, host.mediaUrls.size * 2);
}

/** Run `releaseUnusedMedia` when the page is idle, so the scan is not part of drawing the page. */
export function sweepWhenIdle(host: Engine) {
  if (host.sweepQueued) return;
  host.sweepQueued = true;
  const run = () => {
    host.sweepQueued = false;
    releaseUnusedMedia(host);
  };
  const idle = (globalThis as { requestIdleCallback?: (f: () => void, o?: { timeout: number }) => number }).requestIdleCallback;
  if (idle) idle.call(globalThis, run, { timeout: 2000 });
  else {
    const timer = setTimeout(run, 200) as unknown as { unref?: () => void };
    timer.unref?.();
  }
}

/** A `data:` address for a picture shown from an object URL made by `mediaUrl` (for HTML that leaves the page); null for any other address. */
export function mediaDataUri(host: Engine, url: string): string | null {
  for (const [path, m] of host.mediaUrls) {
    if (m.url !== url) continue;
    const bytes = host.raw.media(path);
    if (!bytes) return null;
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    const type = ({ png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", svg: "image/svg+xml", webp: "image/webp", bmp: "image/bmp" } as Record<string, string>)[ext] ?? "application/octet-stream";
    let bin = "";
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    return `data:${type};base64,${btoa(bin)}`;
  }
  return null;
}

export function dispose(host: Engine) {
  if (host.disposed) return;
  host.disposed = true;
  for (const m of host.mediaUrls.values()) URL.revokeObjectURL(m.url);
  const crypt = host.crypt;
  host.crypt = null;
  host.pw = null;
  host.opened = null;
  // A crashed engine is not called into again, not even to let go: the memory goes with the page.
  if (engineCrash()) return;
  crypt?.free();
  host.raw.free();
}
