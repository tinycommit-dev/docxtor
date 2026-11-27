// avoid-ai-design-ignore-file: T1 (Inter is a fixed part of the design)
// The faces Docxtor needs: Inter for the interface, and metric-compatible stand-ins for Word's fonts
// (Carlito ≈ Calibri, Caladea ≈ Cambria, Arimo ≈ Arial, Tinos ≈ Times New Roman, Cousine ≈ Courier New,
// Gelasio ≈ Georgia) so lines break where Word breaks them.
export const DEFAULT_FONTS_URL =
  "https://fonts.googleapis.com/css2?family=Inter:ital,opsz,wght@0,14..32,100..900;1,14..32,100..900&family=Carlito:ital,wght@0,400;0,700;1,400;1,700&family=Caladea:ital,wght@0,400;0,700;1,400;1,700&family=Arimo:ital,wght@0,400;0,700;1,400;1,700&family=Tinos:ital,wght@0,400;0,700;1,400;1,700&family=Cousine:ital,wght@0,400;0,700;1,400;1,700&family=Gelasio:ital,wght@0,400;0,700;1,400;1,700&display=swap";

/**
 * Add the font stylesheet once per page. Pass your own URL to self-host the same families
 * (no third-party requests), or skip this entirely and load them however you like.
 */
export function ensureFonts(href = DEFAULT_FONTS_URL) {
  if (document.querySelector(`link[rel="stylesheet"][href="${CSS.escape(href)}"]`)) return;
  if (document.querySelector("link[data-docxtor-fonts]")) return;
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = href;
  link.dataset.docxtorFonts = "";
  document.head.append(link);
}

// ---------------------------------------------------------------------------
// Fonts extensions add, and fonts embedded in documents. Fonts are page-wide (one `document.fonts`),
// so loads and stand-ins are counted: the last editor to let go of a font removes it.
// ---------------------------------------------------------------------------

import type { EmbeddedFont, FontContribution } from "@docxtor/extensions";

const loaded = new Map<string, { count: number; remove: () => void }>();
const standIns = new Map<string, string[]>();
const categories = new Map<string, NonNullable<FontContribution["category"]>>();

function hold(key: string, add: () => () => void): () => void {
  const cur = loaded.get(key);
  if (cur) cur.count++;
  else loaded.set(key, { count: 1, remove: add() });
  let done = false;
  return () => {
    if (done) return;
    done = true;
    const c = loaded.get(key);
    if (c && --c.count === 0) {
      c.remove();
      loaded.delete(key);
    }
  };
}

/** A face added to the page by an extension or a document: where its file is (a URL) or the file itself. */
export interface AddedFace {
  family: string;
  weight: string;
  style: string;
  source: string | BufferSource;
}
const added = new Set<AddedFace>();

/** The faces extensions and documents added to the page; the PDF exporter reads their files from here. */
export function addedFaces(): AddedFace[] {
  return [...added];
}

function addFaces(family: string, faces: { source: string | BufferSource; weight?: string; style?: string }[]): () => void {
  const made = faces.map(({ source, weight, style }) => {
    const face = new FontFace(family, typeof source === "string" ? `url(${JSON.stringify(source)})` : source, { weight: weight ?? "400", style: style ?? "normal", display: "swap" });
    document.fonts.add(face);
    // Files download only when text uses the font; data from a document is decoded now.
    if (typeof source !== "string") void face.load().catch(() => undefined);
    const entry = { family, weight: weight ?? "400", style: style ?? "normal", source };
    added.add(entry);
    return { face, entry };
  });
  return () =>
    made.forEach(({ face, entry }) => {
      document.fonts.delete(face);
      added.delete(entry);
    });
}

/**
 * Stylesheets of fonts extensions offer, by family. A stylesheet goes on the page the first time something asks for its family
 * (`wantFont`, from `fontStack`: text in the font, the font menu's preview), not when the font is offered: twenty Google Fonts
 * stylesheets on every start held 13 MB of the page's memory for fonts most documents never use.
 */
const sheets = new Map<string, { href: string; users: number; ready?: Promise<void>; remove?: () => void }>();

function offerSheet(family: string, href: string): () => void {
  const k = family.toLowerCase();
  const cur = sheets.get(k);
  if (cur) cur.users++;
  else sheets.set(k, { href, users: 1 });
  let done = false;
  return () => {
    if (done) return;
    done = true;
    const s = sheets.get(k);
    if (s && --s.users === 0) {
      s.remove?.();
      sheets.delete(k);
    }
  };
}

/**
 * Put the stylesheet of an offered font on the page the first time its family is wanted. Resolves when the stylesheet has loaded
 * (or failed); at once for a family that has none, or whose stylesheet is already there.
 */
export function wantFont(family: string): Promise<void> {
  const s = typeof document === "undefined" ? undefined : sheets.get(family.toLowerCase());
  if (!s) return Promise.resolve();
  s.ready ??= new Promise<void>((done) => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = s.href;
    link.dataset.docxtorFont = family;
    link.addEventListener("load", () => done(), { once: true });
    link.addEventListener("error", () => done(), { once: true });
    document.head.append(link);
    s.remove = () => link.remove();
  });
  return s.ready;
}

/** Load a contributed font and register its stand-ins. Returns the function that releases it. */
export function useFont(font: FontContribution): () => void {
  const releases: (() => void)[] = [];
  if (font.stylesheet) releases.push(offerSheet(font.family, font.stylesheet));
  if (font.faces?.length) {
    const key = `faces:${font.family}:${font.faces.map((f) => f.src).join("|")}`;
    releases.push(hold(key, () => addFaces(font.family, font.faces!.map((f) => ({ source: f.src, weight: f.weight, style: f.style })))));
  }
  if (font.category) {
    const fam = font.family.toLowerCase();
    categories.set(fam, font.category);
    releases.push(() => categories.delete(fam));
  }
  for (const word of font.substitutes ?? []) {
    const k = word.toLowerCase();
    standIns.set(k, [...(standIns.get(k) ?? []), font.family]);
    releases.push(() => {
      const list = standIns.get(k) ?? [];
      list.splice(list.indexOf(font.family), 1);
      if (!list.length) standIns.delete(k);
    });
  }
  return () => releases.splice(0).forEach((r) => r());
}

/** Load a document's embedded fonts. Returns the function that releases them. */
export function useEmbeddedFonts(fonts: EmbeddedFont[]): () => void {
  const variant = { regular: ["400", "normal"], bold: ["700", "normal"], italic: ["400", "italic"], boldItalic: ["700", "italic"] } as const;
  const releases = fonts.map((f) => {
    const [weight, style] = variant[f.style];
    // Same family and style from two documents: whichever loaded first serves both.
    return hold(`embedded:${f.family}:${f.style}:${f.data.length}`, () => addFaces(f.family, [{ source: f.data as BufferSource, weight, style }]));
  });
  return () => releases.forEach((r) => r());
}

/** Families contributed fonts offer in place of a Word font (most recent first). */
export function standInsFor(font: string): string[] {
  return [...(standIns.get(font.toLowerCase()) ?? [])].reverse();
}

/** The category a contributed font declared, if any. */
export function fontCategory(font: string): FontContribution["category"] | undefined {
  return categories.get(font.toLowerCase());
}

// ---------------------------------------------------------------------------
// Which of a document's fonts the browser can show.
// ---------------------------------------------------------------------------

/** The fonts the font menu always lists. */
export const MENU_FONTS = ["Calibri", "Aptos", "Arial", "Helvetica", "Georgia", "Times New Roman", "Cambria", "Garamond", "Verdana", "Courier New", "Consolas"];

// Open fonts with the same letter widths as Word's (loaded with DEFAULT_FONTS_URL). Text in the Word
// font breaks into the same lines, so Docxtor doesn't count those fonts as missing.
const METRIC_TWINS: Record<string, string> = {
  calibri: "Carlito",
  "calibri light": "Carlito",
  cambria: "Caladea",
  arial: "Arimo",
  "times new roman": "Tinos",
  "courier new": "Cousine",
  georgia: "Gelasio",
};

const unquote = (family: string) => family.replace(/^["']|["']$/g, "").toLowerCase();

/** A web font with this name: embedded in a document, added by an extension, or from a stylesheet. */
function isWebFont(family: string): boolean {
  const k = family.toLowerCase();
  for (const face of document.fonts) if (unquote(face.family) === k && face.status !== "error") return true;
  return false;
}

let probe: CanvasRenderingContext2D | null | undefined;
const local = new Map<string, boolean>();

/** Installed on this computer: text in it measures differently from every generic fallback. */
function isInstalled(family: string): boolean {
  const k = family.toLowerCase();
  const known = local.get(k);
  if (known !== undefined) return known;
  probe ??= document.createElement("canvas").getContext("2d");
  if (!probe) return true; // can't tell, so don't warn
  const ctx = probe;
  const sample = "mmmmmmmmmmlli1WQ@#";
  const name = `"${family.replace(/["\\]/g, "")}"`;
  const found = ["monospace", "serif", "sans-serif"].some((base) => {
    ctx.font = `72px ${base}`;
    const fallback = ctx.measureText(sample).width;
    ctx.font = `72px ${name}, ${base}`;
    return ctx.measureText(sample).width !== fallback;
  });
  local.set(k, found);
  return found;
}

/**
 * How text in `family` shows here: in that font (installed, embedded in the document, or offered by
 * an extension in `offered`), in a font with the same letter widths, or in a guess.
 */
export function fontStatus(family: string, offered: string[] = []): "available" | "standIn" | "missing" {
  const k = family.toLowerCase();
  if (offered.some((f) => f.toLowerCase() === k) || isWebFont(family) || isInstalled(family)) return "available";
  const twin = METRIC_TWINS[k];
  if (standInsFor(family).length) return "standIn";
  if (twin && (document.querySelector("link[data-docxtor-fonts]") || isWebFont(twin) || isInstalled(twin))) return "standIn";
  return "missing";
}
