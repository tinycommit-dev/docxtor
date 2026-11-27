// Reading font declarations as CSS writes them (@font-face rules, font-family lists) and choosing the
// face CSS would choose. Pure functions, no DOM: the PDF exporter uses them to find the font files a
// page loaded, and they are tested in Node.

export type FaceStyle = "normal" | "italic" | "oblique";

/** One @font-face declaration. */
export interface RawFace {
  family: string;
  /** The weights it covers: 400 for a plain face, 100 to 900 for a variable one. */
  weight: [number, number];
  style: FaceStyle;
  /** The characters it covers (a Google Fonts subset), or null for all of them. */
  ranges: [number, number][] | null;
  /** Font files in the order the declaration lists them. */
  sources: { url: string; format: string }[];
}

/** The families of a `font-family` value, unquoted, in order. */
export function splitFamilies(list: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quote = "";
  for (const ch of list) {
    if (quote) {
      if (ch === quote) quote = "";
      else cur += ch;
    } else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === ",") {
      if (cur.trim()) out.push(cur.trim());
      cur = "";
    } else cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

/** `U+0000-00FF, U+0131, U+4??` as ranges of code points. */
export function parseUnicodeRange(value: string): [number, number][] | null {
  const out: [number, number][] = [];
  for (const part of value.split(",")) {
    const m = /^\s*u\+([0-9a-f?]+)(?:-([0-9a-f]+))?\s*$/i.exec(part);
    if (!m) continue;
    if (m[1].includes("?")) out.push([parseInt(m[1].replace(/\?/g, "0"), 16), parseInt(m[1].replace(/\?/g, "f"), 16)]);
    else {
      const lo = parseInt(m[1], 16);
      out.push([lo, m[2] ? parseInt(m[2], 16) : lo]);
    }
  }
  return out.length ? out : null;
}

export function inRanges(ranges: [number, number][] | null, cp: number): boolean {
  if (!ranges) return true;
  for (const [lo, hi] of ranges) if (cp >= lo && cp <= hi) return true;
  return false;
}

/** `400`, `bold` or `100 900`. */
export function parseWeight(value: string | undefined): [number, number] {
  const v = (value ?? "").trim().toLowerCase();
  if (!v || v === "normal") return [400, 400];
  if (v === "bold") return [700, 700];
  const nums = v.split(/\s+/).map(Number);
  if (nums.length && nums.every((n) => Number.isFinite(n))) return [Math.min(...nums), Math.max(...nums)];
  return [400, 400];
}

export function parseStyle(value: string | undefined): FaceStyle {
  const v = (value ?? "").trim().toLowerCase();
  return v.startsWith("italic") ? "italic" : v.startsWith("oblique") ? "oblique" : "normal";
}

/** The `url(...)` parts of a `src` value with their formats, made absolute against `base`. */
export function parseSrc(value: string, base: string): { url: string; format: string }[] {
  const out: { url: string; format: string }[] = [];
  const re = /url\(\s*(?:"([^"]*)"|'([^']*)'|([^)\s]*))\s*\)(?:\s*format\(\s*(?:"([^"]*)"|'([^']*)'|([^)\s]*))\s*\))?/gi;
  for (const m of value.matchAll(re)) {
    const raw = m[1] ?? m[2] ?? m[3] ?? "";
    if (!raw) continue;
    let url = raw;
    try {
      url = new URL(raw, base).href;
    } catch {
      // keep the text as written
    }
    const format = (m[4] ?? m[5] ?? m[6] ?? "").toLowerCase() || guessFormat(url);
    out.push({ url, format });
  }
  return out;
}

function guessFormat(url: string): string {
  const path = url.split(/[?#]/)[0].toLowerCase();
  if (path.endsWith(".woff2")) return "woff2";
  if (path.endsWith(".woff")) return "woff";
  if (path.endsWith(".otf")) return "opentype";
  if (path.endsWith(".ttf")) return "truetype";
  return "";
}

/** Order font files by how easily Docxtor reads them: TrueType, then WOFF, then WOFF2. */
export function sourceRank(format: string): number {
  if (format === "truetype" || format === "opentype" || format === "ttf" || format === "otf") return 0;
  if (format === "woff") return 1;
  if (format === "woff2") return 2;
  return 3;
}

/** Every @font-face rule in a piece of CSS text. */
export function parseFontFaces(css: string, base: string): RawFace[] {
  const out: RawFace[] = [];
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const m of text.matchAll(/@font-face\s*\{([^}]*)\}/gi)) {
    const props = new Map<string, string>();
    for (const decl of m[1].split(";")) {
      const at = decl.indexOf(":");
      if (at > 0) props.set(decl.slice(0, at).trim().toLowerCase(), decl.slice(at + 1).trim());
    }
    const face = faceFromProps((name) => props.get(name), base);
    if (face) out.push(face);
  }
  return out;
}

/** A face from a property lookup (a parsed rule's `getPropertyValue`, or a map). */
export function faceFromProps(get: (name: string) => string | undefined, base: string): RawFace | null {
  const family = splitFamilies(get("font-family") ?? "")[0];
  const sources = parseSrc(get("src") ?? "", base);
  if (!family || !sources.length) return null;
  return {
    family,
    weight: parseWeight(get("font-weight")),
    style: parseStyle(get("font-style")),
    ranges: parseUnicodeRange(get("unicode-range") ?? ""),
    sources: sources.sort((a, b) => sourceRank(a.format) - sourceRank(b.format)),
  };
}

/**
 * The face CSS font matching picks for a weight and style: the right style first (italic falls back to
 * oblique, then upright), then the closest weight, looking in the direction the weight asks for.
 */
export function bestFace<T extends { weight: [number, number]; style: FaceStyle }>(faces: T[], weight: number, italic: boolean): T | null {
  if (!faces.length) return null;
  const order: FaceStyle[] = italic ? ["italic", "oblique", "normal"] : ["normal", "oblique", "italic"];
  let pool: T[] = [];
  for (const s of order) {
    pool = faces.filter((f) => f.style === s);
    if (pool.length) break;
  }
  const exact = pool.filter((f) => f.weight[0] <= weight && weight <= f.weight[1]);
  if (exact.length) return exact[0];
  // The weight a face stands for when it doesn't contain the one asked for: its nearest end.
  const near = (f: T) => (weight < f.weight[0] ? f.weight[0] : f.weight[1]);
  const sorted = (xs: T[], dir: 1 | -1) => [...xs].sort((a, b) => (near(a) - near(b)) * dir);
  const below = pool.filter((f) => near(f) < weight);
  const above = pool.filter((f) => near(f) > weight);
  if (weight >= 400 && weight <= 500) {
    const mid = sorted(above.filter((f) => near(f) <= 500), 1);
    return mid[0] ?? sorted(below, -1)[0] ?? sorted(above, 1)[0] ?? null;
  }
  if (weight < 400) return sorted(below, -1)[0] ?? sorted(above, 1)[0] ?? null;
  return sorted(above, 1)[0] ?? sorted(below, -1)[0] ?? null;
}
