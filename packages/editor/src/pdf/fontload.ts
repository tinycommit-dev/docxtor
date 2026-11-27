// Finding the font files a page uses: the @font-face rules of its style sheets, the faces extensions and
// documents added, and (when the user has allowed it) the fonts installed on the computer. Each one
// becomes a `Face` that can read its file when the PDF needs it.
import { timeLimit } from "@docxtor/extensions";
import { addedFaces } from "../fonts";
import { faceFromProps, parseFontFaces, parseStyle, parseWeight, type RawFace } from "./fontcss.ts";
import type { Face } from "./fontset.ts";
import { TrueTypeFont } from "./truetype.ts";
import { isWoff, unwrapWoff } from "./woff.ts";

/** The font files already read, by address. */
const byUrl = new Map<string, Promise<TrueTypeFont | null>>();
const sheetText = new Map<string, Promise<string>>();

/** Read a font file in any format Docxtor understands. Null when it is not a font it can embed. */
export async function readFont(bytes: Uint8Array): Promise<TrueTypeFont | null> {
  try {
    let sfnt = bytes;
    if (isWoff(bytes)) sfnt = await unwrapWoff(bytes);
    else if (bytes[0] === 0x77 && bytes[1] === 0x4f && bytes[2] === 0x46 && bytes[3] === 0x32) {
      const { decodeWoff2 } = await import("./woff2.ts");
      sfnt = await decodeWoff2(bytes);
    }
    const font = new TrueTypeFont(sfnt);
    // Outlines in CFF and fonts that forbid embedding can't go in the PDF.
    if (font.isCff || font.fsType & 0x0002) return null;
    return font;
  } catch {
    return null;
  }
}

async function fetchBytes(url: string): Promise<Uint8Array> {
  const res = await fetch(url, { signal: timeLimit(20000) });
  if (!res.ok) throw new Error(`${res.status} ${url}`);
  return new Uint8Array(await res.arrayBuffer());
}

function loadUrls(urls: string[]): Promise<TrueTypeFont | null> {
  const key = urls.join("\n");
  let p = byUrl.get(key);
  if (!p) {
    p = (async () => {
      for (const url of urls) {
        try {
          const font = await readFont(await fetchBytes(url));
          if (font) return font;
        } catch {
          // try the next file
        }
      }
      return null;
    })();
    byUrl.set(key, p);
  }
  return p;
}

/** Every @font-face rule the page can see, including those in style sheets from other sites. */
async function sheetFaces(): Promise<RawFace[]> {
  const out: RawFace[] = [];
  const blocked: string[] = [];
  const visit = (rules: CSSRuleList, base: string) => {
    for (const rule of Array.from(rules)) {
      if (rule instanceof CSSFontFaceRule) {
        const face = faceFromProps((name) => rule.style.getPropertyValue(name), base);
        if (face) out.push(face);
      } else if (rule instanceof CSSImportRule) {
        if (rule.styleSheet) visitSheet(rule.styleSheet);
      } else if ("cssRules" in rule) visit((rule as CSSGroupingRule).cssRules, base);
    }
  };
  const visitSheet = (sheet: CSSStyleSheet) => {
    try {
      visit(sheet.cssRules, sheet.href ?? document.baseURI);
    } catch {
      // A style sheet from another site hides its rules; read its text instead.
      if (sheet.href) blocked.push(sheet.href);
    }
  };
  for (const sheet of Array.from(document.styleSheets)) visitSheet(sheet);
  await Promise.all(
    blocked.map(async (href) => {
      let text = sheetText.get(href);
      if (!text) {
        text = fetch(href, { signal: timeLimit(20000) }).then((r) => (r.ok ? r.text() : ""));
        sheetText.set(href, text);
      }
      try {
        out.push(...parseFontFaces(await text, href));
      } catch {
        // the style sheet can't be read: its fonts are not available
      }
    }),
  );
  return out;
}

const WEIGHTS: [RegExp, number][] = [
  [/thin|hairline/i, 100],
  [/extra ?light|ultra ?light/i, 200],
  [/light/i, 300],
  [/medium/i, 500],
  [/semi ?bold|demi ?bold/i, 600],
  [/extra ?bold|ultra ?bold|heavy/i, 800],
  [/black/i, 900],
  [/bold/i, 700],
];

interface LocalFontData {
  family: string;
  style: string;
  blob(): Promise<Blob>;
}

/** The fonts installed on this computer, when the user has already let the page list them. */
async function localFaces(): Promise<Face[]> {
  const query = (window as unknown as { queryLocalFonts?: () => Promise<LocalFontData[]> }).queryLocalFonts;
  if (typeof query !== "function") return [];
  try {
    const perm = await navigator.permissions.query({ name: "local-fonts" as PermissionName });
    if (perm.state !== "granted") return [];
    const all = await query.call(window);
    return all.map((f) => {
      const weight = WEIGHTS.find(([re]) => re.test(f.style))?.[1] ?? 400;
      let font: Promise<TrueTypeFont | null> | undefined;
      return {
        family: f.family,
        weight: [weight, weight] as [number, number],
        style: /italic|oblique/i.test(f.style) ? ("italic" as const) : ("normal" as const),
        ranges: null,
        load: () => (font ??= f.blob().then(async (b) => readFont(new Uint8Array(await b.arrayBuffer())))),
      };
    });
  } catch {
    return [];
  }
}

/** All the faces the PDF can embed. */
export async function discoverFaces(): Promise<Face[]> {
  const faces: Face[] = [];
  for (const raw of await sheetFaces()) {
    let font: Promise<TrueTypeFont | null> | undefined;
    faces.push({ family: raw.family, weight: raw.weight, style: raw.style, ranges: raw.ranges, load: () => (font ??= loadUrls(raw.sources.map((s) => s.url))) });
  }
  for (const a of addedFaces()) {
    const source = a.source;
    const url = typeof source === "string" ? new URL(source, document.baseURI).href : "";
    const bytes = typeof source === "string" ? null : source instanceof ArrayBuffer ? new Uint8Array(source) : new Uint8Array(source.buffer, source.byteOffset, source.byteLength);
    let font: Promise<TrueTypeFont | null> | undefined;
    faces.push({ family: a.family, weight: parseWeight(a.weight), style: parseStyle(a.style), ranges: null, load: () => (font ??= bytes ? readFont(bytes) : loadUrls([url])) });
  }
  // Fonts the faces above cover come first, so installed copies are only a last resort for the same family.
  faces.push(...(await localFaces()));
  return faces;
}
