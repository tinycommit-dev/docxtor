// Reading what is on the clipboard (or in a drag): which formats it holds, the converters that turn a
// format into content, and Docxtor's own converters (its fragment, HTML from Word, Google Docs and web pages,
// RTF, plain text). Extensions add converters through `ctx.clipboard.converters`.
import { Engine, type Fragment } from "@docxtor/core";
import { timeLimit, t, tn, type PasteContent, type PasteConverter, type PasteInput } from "@docxtor/extensions";

/** The format Docxtor puts on the clipboard for its own content. */
export const FRAGMENT_MIME = "application/x-docxtor-fragment";

/** What is on the clipboard or in a drag, read into text. */
export interface Clip {
  types: string[];
  get(type: string): string | undefined;
  files: File[];
}

export function clipFromData(dt: DataTransfer): Clip {
  const types = Array.from(dt.types).filter((x) => x !== "Files");
  const cache = new Map<string, string>();
  for (const ty of types) {
    try {
      cache.set(ty, dt.getData(ty));
    } catch {
      /* a format the page may not read */
    }
  }
  return { types: [...cache.keys()], get: (ty) => cache.get(ty), files: Array.from(dt.files ?? []) };
}

/** Read the clipboard through the async API (needs the browser's permission). */
export async function clipFromClipboard(): Promise<Clip> {
  const cache = new Map<string, string>();
  const files: File[] = [];
  for (const item of await navigator.clipboard.read()) {
    for (const ty of item.types) {
      const blob = await item.getType(ty);
      if (ty.startsWith("image/")) files.push(new File([blob], "pasted-image", { type: ty }));
      else cache.set(ty.replace(/^web /, ""), await blob.text());
    }
  }
  return { types: [...cache.keys()], get: (ty) => cache.get(ty), files };
}

/** A clip holding only some text (for the paste-as-plain-text paths). */
export function clipFromText(text: string): Clip {
  return { types: ["text/plain"], get: (ty) => (ty === "text/plain" ? text : undefined), files: [] };
}

export interface Converted {
  content: PasteContent;
  /** The converter that read it. */
  by: string;
  /** What the clip carried, for pasting again another way. */
  clip: Clip;
}

/** Fetch a picture with the browser's rules; null when the page can't reach it. */
export async function fetchImage(address: string): Promise<Blob | null> {
  if (!/^https?:/i.test(address)) return null;
  try {
    const res = await fetch(address, { signal: timeLimit(5000), credentials: "omit" });
    if (!res.ok) return null;
    const blob = await res.blob();
    return blob.type.startsWith("image/") || blob.size > 0 ? blob : null;
  } catch {
    return null;
  }
}

async function toDataUri(blob: Blob): Promise<string> {
  const buf = new Uint8Array(await blob.arrayBuffer());
  let s = "";
  for (let i = 0; i < buf.length; i += 0x8000) s += String.fromCharCode(...buf.subarray(i, i + 0x8000));
  return `data:${blob.type || "application/octet-stream"};base64,${btoa(s)}`;
}

/**
 * Run the converters whose format is on the clipboard, from the highest priority down; the first to return content wins.
 * `only` limits the converters to some formats (Paste special).
 */
export async function convertClip(clip: Clip, converters: PasteConverter[], engine: Engine, only?: string): Promise<Converted | null> {
  const input = (data: string): PasteInput => ({ data, get: (ty) => clip.get(ty), types: clip.types, engine, fetchImage });
  const list = converters.filter((c) => clip.types.includes(c.mime) && (!only || c.id === only)).sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
  for (const c of list) {
    const data = clip.get(c.mime);
    if (data === undefined || data === "") continue;
    try {
      const content = await c.convert(input(data));
      if (content) return { content, by: c.id, clip };
    } catch (e) {
      console.warn(`Paste converter ${c.id} failed:`, e);
    }
  }
  return null;
}

/** The converters that can read a clip, for Paste special. */
export function convertersFor(clip: Clip, converters: PasteConverter[]): PasteConverter[] {
  return converters.filter((c) => clip.types.includes(c.mime) && (clip.get(c.mime) ?? "") !== "").sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0));
}

/** What the person is told after a paste, from the warnings a fragment carries. Null when there is nothing to say. */
export function warningText(f: Fragment): string | null {
  const parts: string[] = [];
  for (const w of f.warnings ?? []) {
    if (w.kind === "picture-unreadable") parts.push(tn(w.count, "{n} picture couldn't be read, so it was left out", "{n} pictures couldn't be read, so they were left out"));
    else if (w.kind === "picture-unsupported") parts.push(tn(w.count, "{n} picture is in a format Docxtor can't show, so it was left out", "{n} pictures are in a format Docxtor can't show, so they were left out"));
    else if (w.kind === "object-dropped") parts.push(tn(w.count, "{n} object (a chart or embedded item) can't be pasted and was left out", "{n} objects (charts or embedded items) can't be pasted and were left out"));
  }
  return parts.length ? parts.join(". ") : null;
}

// ---------------------------------------------------------------------------
// Docxtor's own converters
// ---------------------------------------------------------------------------

/** A fragment from the clipboard is untrusted (any page can write one): check its shape here, and the engine checks its content when it goes in. */
function parseFragment(json: string): Fragment | null {
  try {
    const f = JSON.parse(json) as Record<string, unknown>;
    if (!f || typeof f !== "object" || f.version !== 1 || typeof f.body !== "string" || !f.body) return null;
    const text = (v: unknown) => v === undefined || typeof v === "string";
    const table = (v: unknown) => v === undefined || (typeof v === "object" && v !== null && !Array.isArray(v));
    if (!text(f.styles) || !text(f.numbering) || !table(f.media) || !table(f.links) || !table(f.notes)) return null;
    if (f.ns !== undefined && !(Array.isArray(f.ns) && f.ns.every((n) => Array.isArray(n) && n.length === 2 && typeof n[0] === "string" && typeof n[1] === "string"))) return null;
    return f as unknown as Fragment;
  } catch {
    return null;
  }
}

/** Fragment JSON hidden in Docxtor's HTML flavour, for clipboards that carry only the standard formats. */
const IN_HTML = /data-docxtor-fragment="([A-Za-z0-9+/=]+)"/;

export function foldedFragment(json: string): string {
  const bytes = new TextEncoder().encode(json);
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function unfold(b64: string): string {
  const bin = atob(b64);
  return new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
}

export const BUILTIN_CONVERTERS: PasteConverter[] = [
  {
    id: "docxtor.fragment",
    label: "Docxtor content",
    mime: FRAGMENT_MIME,
    priority: 100,
    convert: ({ data }) => {
      const fragment = parseFragment(data);
      return fragment ? { fragment } : null;
    },
  },
  {
    id: "docxtor.html",
    label: "Formatted text (HTML)",
    mime: "text/html",
    priority: 50,
    convert: async ({ data, get, fetchImage: fetchPicture }) => {
      // Docxtor's own content rides in the HTML when the clipboard has only the standard formats.
      const own = IN_HTML.exec(data);
      const carried = own ? parseFragment(unfold(own[1])) : null;
      if (carried) return { fragment: carried };
      const sources = Engine.htmlImageSources(data);
      const images: Record<string, string> = {};
      // Word names its pictures by file path, which a page can't read; they are in the RTF next to it, in the same order.
      const local = sources.filter((s) => /^file:/i.test(s));
      const rtf = get("text/rtf");
      if (local.length && rtf) {
        const media = Object.values(Engine.rtfToFragment(rtf).media);
        if (media.length === local.length) local.forEach((s, i) => (images[s] = media[i].data));
      }
      await Promise.all(
        sources
          .filter((s) => /^https?:/i.test(s) && !images[s])
          .slice(0, 24)
          .map(async (s) => {
            const blob = await fetchPicture(s);
            if (blob) images[s] = await toDataUri(blob);
          }),
      );
      const fragment = Engine.htmlToFragment(data, images);
      return fragment.body ? { fragment } : null;
    },
  },
  {
    id: "docxtor.rtf",
    label: "Formatted text (RTF)",
    mime: "text/rtf",
    priority: 40,
    convert: ({ data }) => {
      const fragment = Engine.rtfToFragment(data);
      return fragment.body ? { fragment } : null;
    },
  },
  {
    id: "docxtor.text",
    label: "Plain text",
    mime: "text/plain",
    priority: 10,
    convert: ({ data }) => ({ text: data }),
  },
];

/** Names for the formats Paste special offers, in the reader's language. */
export function converterLabel(c: PasteConverter): string {
  switch (c.id) {
    case "docxtor.fragment":
      return t("Docxtor content");
    case "docxtor.html":
      return t("Formatted text (HTML)");
    case "docxtor.rtf":
      return t("Formatted text (RTF)");
    case "docxtor.text":
      return t("Plain text");
    default:
      return c.label;
  }
}
