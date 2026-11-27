// Small pure helpers for turning the text of a page into PDF text: what counts as a space, what is never
// drawn, how CSS text-transform and small caps change what is shown, and colours as CSS writes them.

/** Characters that separate words: spaces, tabs, line breaks and the wide and narrow spaces. */
export function isSpace(cp: number): boolean {
  return cp === 0x20 || cp === 0x09 || cp === 0x0a || cp === 0x0d || cp === 0x0c || cp === 0xa0 || (cp >= 0x2000 && cp <= 0x200a) || cp === 0x202f || cp === 0x205f || cp === 0x3000;
}

/** Characters that take no room and are never drawn: soft hyphens, zero-width marks, direction marks. */
export function isInvisible(cp: number): boolean {
  return cp === 0xad || (cp >= 0x200b && cp <= 0x200f) || (cp >= 0x202a && cp <= 0x202e) || (cp >= 0x2060 && cp <= 0x2064) || cp === 0xfeff || cp === 0x034f || (cp >= 0xfe00 && cp <= 0xfe0f);
}

export interface Shown {
  /** The code points drawn for the character (text-transform can change one into several). */
  cps: number[];
  /** Drawn as a small capital: a capital letter at a smaller size. */
  small: boolean;
}

/** What CSS draws for a character under `text-transform` and `font-variant-caps`. */
export function shownChars(ch: string, transform: string, caps: string, wordStart: boolean): Shown {
  let s = ch;
  let small = false;
  if (transform === "uppercase") s = ch.toUpperCase();
  else if (transform === "lowercase") s = ch.toLowerCase();
  else if (transform === "capitalize" && wordStart) s = ch.toUpperCase();
  if (caps === "small-caps" || caps === "all-small-caps") {
    const up = s.toUpperCase();
    if (up !== s) {
      s = up;
      small = true;
    } else if (caps === "all-small-caps" && s.toLowerCase() !== s) small = true;
  }
  return { cps: Array.from(s, (c) => c.codePointAt(0)!), small };
}

export type Rgba = [number, number, number, number];

/** `rgb(1, 2, 3)`, `rgba(…)`, `rgb(1 2 3 / 50%)`, `color(srgb 0.1 0.2 0.3 / 0.5)` or `transparent` as 0 to 1 values. Null for anything else. */
export function parseColor(s: string): Rgba | null {
  const v = s.trim().toLowerCase();
  if (v === "transparent") return [0, 0, 0, 0];
  const m = /^(rgba?|color)\(\s*(?:srgb\s+)?([^)]*)\)$/.exec(v);
  if (!m) return null;
  const parts = m[2].split(/[\s,/]+/).filter(Boolean);
  if (parts.length < 3) return null;
  const channel = (p: string, max: number) => {
    const n = p.endsWith("%") ? (parseFloat(p) / 100) * max : parseFloat(p);
    return Number.isFinite(n) ? n / max : NaN;
  };
  const scale = m[1] === "color" ? 1 : 255;
  const rgb = parts.slice(0, 3).map((p) => channel(p, scale));
  const a = parts.length > 3 ? (parts[3].endsWith("%") ? parseFloat(parts[3]) / 100 : parseFloat(parts[3])) : 1;
  if (rgb.some((n) => Number.isNaN(n)) || Number.isNaN(a)) return null;
  const clamp = (n: number) => Math.max(0, Math.min(1, n));
  return [clamp(rgb[0]), clamp(rgb[1]), clamp(rgb[2]), clamp(a)];
}

/** A string for a PDF literal, one byte per character (WinAnsi codes), with the characters that need it escaped. */
export function literalBytes(codes: number[]): string {
  let out = "(";
  for (const c of codes) {
    if (c === 0x28 || c === 0x29 || c === 0x5c) out += `\\${String.fromCharCode(c)}`;
    else if (c < 32 || c > 126) out += `\\${c.toString(8).padStart(3, "0")}`;
    else out += String.fromCharCode(c);
  }
  return `${out})`;
}

/** Glyph numbers as the hexadecimal string a two-byte font shows. */
export function glyphHex(gids: number[]): string {
  let out = "<";
  for (const g of gids) out += g.toString(16).padStart(4, "0");
  return `${out}>`;
}

/** The characters MathML's `text-transform: math-auto` turns italic: Latin and Greek letters, dotless i and j, ∂ and ∇. */
const MATH_ITALIC = /^[A-Za-zıȷΑ-ΡΣ-Ωα-ωϑϕϖϰϱϵ∂∇]$/u;

/** A letter standing alone in an equation's `<mi>`, which the browser draws in italic. */
export function mathItalic(cs: { textTransform: string }, text: string): boolean {
  return cs.textTransform === "math-auto" && MATH_ITALIC.test(text.trim());
}
