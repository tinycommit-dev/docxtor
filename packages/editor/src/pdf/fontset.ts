// The fonts of a PDF: which font file draws each character, the glyphs used from each, and the PDF
// objects that embed them (TrueType as CIDFontType2 with Identity-H, a ToUnicode map so text can be
// selected and searched, widths from hmtx). No DOM: the exporter hands it faces that know how to load.
import { bestFace, inRanges, type FaceStyle } from "./fontcss.ts";
import { TrueTypeFont } from "./truetype.ts";
import { num, pdfName, ref, type PdfWriter } from "./writer.ts";

const enc = new TextEncoder();

/** A font face the exporter can fetch. */
export interface Face {
  family: string;
  weight: [number, number];
  style: FaceStyle;
  ranges: [number, number][] | null;
  /** Read the font file. Resolves to null when it can't be fetched or read. Called at most once. */
  load(): Promise<TrueTypeFont | null>;
}

/** What a run of text asks of a font. */
export interface FontKey {
  /** The families of the `font-family` value, in order (generic names such as serif included). */
  families: string[];
  weight: number;
  italic: boolean;
}

export const keyString = (k: FontKey): string => `${k.families.join(",")}|${k.weight >= 600 ? 700 : k.weight}|${k.italic ? "i" : "n"}`;

const GENERIC = new Set(["serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded", "math", "emoji", "fangsong", "-apple-system", "blinkmacsystemfont"]);
const isGeneric = (f: string) => GENERIC.has(f.toLowerCase());

type Kind = "serif" | "sans" | "mono";

/** Which kind of font a stack falls back to. */
export function kindOf(families: string[]): Kind {
  for (let i = families.length - 1; i >= 0; i--) {
    const f = families[i].toLowerCase();
    if (f === "monospace" || f === "ui-monospace") return "mono";
    if (f === "serif" || f === "ui-serif" || f === "math" || f === "fangsong") return "serif";
    if (isGeneric(f)) return "sans";
  }
  // Nothing generic named: browsers fall back to their standard font, a serif.
  return "serif";
}

/** Open fonts with the metrics of the standard ones, which Docxtor offers on the page. */
const STAND_INS: Record<Kind, string[]> = {
  serif: ["Tinos", "Caladea", "Gelasio", "Liberation Serif"],
  sans: ["Arimo", "Carlito", "Liberation Sans"],
  mono: ["Cousine", "Liberation Mono"],
};

/** A font file embedded in the PDF, and the glyphs used from it. */
export class EmbeddedFont {
  readonly ttf: TrueTypeFont;
  readonly resource: string;
  /** The family of the face the file came from. */
  readonly family: string;
  /** The glyphs used, each with a character that maps to it. */
  readonly glyphs = new Map<number, number>();

  constructor(ttf: TrueTypeFont, index: number, family: string) {
    this.ttf = ttf;
    this.resource = `F${index}`;
    this.family = family;
  }

  use(gid: number, cp: number) {
    if (!this.glyphs.has(gid)) this.glyphs.set(gid, cp);
  }

  /** Write the font's objects and return the number of the Type0 font. */
  async write(w: PdfWriter): Promise<number> {
    const f = this.ttf;
    const scale = 1000 / f.unitsPerEm;
    const tag = subsetTag([...this.glyphs.keys()]);
    const psName = (f.names[6] || f.names[4] || f.names[1] || "Font").replace(/[^A-Za-z0-9_-]/g, "");
    const base = `${tag}+${psName}`;
    const file = f.subset(this.glyphs.keys());
    const fileObj = await w.addStream(`/Length1 ${file.length}`, file);
    const flags = (f.isFixedPitch ? 1 : 0) | 4 | (f.italic ? 64 : 0) | (f.weightClass >= 700 ? 0x40000 : 0);
    const ascent = f.useTypoMetrics && f.typoAscender ? f.typoAscender : f.ascender || f.typoAscender;
    const descent = f.useTypoMetrics && f.typoDescender ? f.typoDescender : f.descender || f.typoDescender;
    const descriptor = w.add(
      `<< /Type /FontDescriptor /FontName ${pdfName(base)} /Flags ${flags} /FontBBox [${f.bbox.map((v) => num(v * scale)).join(" ")}] ` +
        `/ItalicAngle ${num(f.italicAngle)} /Ascent ${num(ascent * scale)} /Descent ${num(descent * scale)} /CapHeight ${num((f.capHeight || ascent * 0.7) * scale)} ` +
        `/StemV ${num(50 + (f.weightClass / 100) ** 2 * 1.2)} /FontFile2 ${ref(fileObj)} >>`,
    );
    const gids = [...this.glyphs.keys()].sort((a, b) => a - b);
    // Widths as runs of consecutive glyphs: c [w1 w2 …].
    let widths = "";
    for (let i = 0; i < gids.length; ) {
      let j = i;
      while (j + 1 < gids.length && gids[j + 1] === gids[j] + 1) j++;
      widths += `${gids[i]} [${gids.slice(i, j + 1).map((g) => num(f.advance(g) * scale)).join(" ")}] `;
      i = j + 1;
    }
    const cid = w.add(
      `<< /Type /Font /Subtype /CIDFontType2 /BaseFont ${pdfName(base)} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> ` +
        `/FontDescriptor ${ref(descriptor)} /DW 1000 /W [${widths.trim()}] /CIDToGIDMap /Identity >>`,
    );
    const toUnicode = await w.addStream("", enc.encode(toUnicodeCMap(this.glyphs)));
    return w.add(`<< /Type /Font /Subtype /Type0 /BaseFont ${pdfName(base)} /Encoding /Identity-H /DescendantFonts [${ref(cid)}] /ToUnicode ${ref(toUnicode)} >>`);
  }
}

/** Six capital letters that make the names of two subsets of a font differ. */
function subsetTag(gids: number[]): string {
  let h = 2166136261;
  for (const g of gids) h = Math.imul(h ^ g, 16777619) >>> 0;
  let out = "";
  for (let i = 0; i < 6; i++) {
    out += String.fromCharCode(65 + (h % 26));
    h = Math.imul(h, 1103515245) + 12345 >>> 0;
  }
  return out;
}

const hex4 = (n: number) => n.toString(16).padStart(4, "0").toUpperCase();

/** The UTF-16 code units of a code point, in hexadecimal. */
function utf16Hex(cp: number): string {
  if (cp < 0x10000) return hex4(cp);
  const v = cp - 0x10000;
  return hex4(0xd800 + (v >> 10)) + hex4(0xdc00 + (v & 0x3ff));
}

/** A ToUnicode CMap: what each glyph of a font stands for, so that text copies and searches as text. */
export function toUnicodeCMap(glyphs: Map<number, number>): string {
  const entries = [...glyphs].sort((a, b) => a[0] - b[0]);
  let out =
    "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n";
  for (let i = 0; i < entries.length; i += 100) {
    const part = entries.slice(i, i + 100);
    out += `${part.length} beginbfchar\n`;
    for (const [gid, cp] of part) out += `<${hex4(gid)}> <${utf16Hex(cp)}>\n`;
    out += "endbfchar\n";
  }
  return `${out}endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend`;
}

/** The standard fonts every viewer has, for text no embeddable font covers. */
export class StdFont {
  readonly resource: string;
  readonly baseFont: string;

  constructor(resource: string, baseFont: string) {
    this.resource = resource;
    this.baseFont = baseFont;
  }

  write(w: PdfWriter): number {
    return w.add(`<< /Type /Font /Subtype /Type1 /BaseFont ${pdfName(this.baseFont)} /Encoding /WinAnsiEncoding >>`);
  }
}

const WIN_ANSI_HIGH: Record<number, number> = {
  0x20ac: 0x80, 0x201a: 0x82, 0x0192: 0x83, 0x201e: 0x84, 0x2026: 0x85, 0x2020: 0x86, 0x2021: 0x87, 0x02c6: 0x88, 0x2030: 0x89, 0x0160: 0x8a, 0x2039: 0x8b, 0x0152: 0x8c, 0x017d: 0x8e,
  0x2018: 0x91, 0x2019: 0x92, 0x201c: 0x93, 0x201d: 0x94, 0x2022: 0x95, 0x2013: 0x96, 0x2014: 0x97, 0x02dc: 0x98, 0x2122: 0x99, 0x0161: 0x9a, 0x203a: 0x9b, 0x0153: 0x9c, 0x017e: 0x9e, 0x0178: 0x9f,
};

/** The WinAnsi code of a character, or -1. */
export function winAnsi(cp: number): number {
  if ((cp >= 0x20 && cp < 0x7f) || (cp >= 0xa0 && cp <= 0xff)) return cp;
  return WIN_ANSI_HIGH[cp] ?? -1;
}

/** How a character is drawn. */
export type Pick =
  | { kind: "ttf"; font: EmbeddedFont; gid: number; cp: number; unitsPerEm: number; synthBold: boolean; synthItalic: boolean }
  | { kind: "std"; font: StdFont; code: number; cp: number; kindOf: Kind; bold: boolean; italic: boolean; synthBold: false; synthItalic: false };

export interface FontSetOptions {
  /** Fonts to try for text no named family covers, by kind, before the standard fonts. */
  standIns?: Record<Kind, string[]>;
}

/** Chooses fonts for text and keeps what is used. */
export class FontSet {
  private byFamily = new Map<string, Face[]>();
  /** Font files read so far (null: could not be read). */
  private files = new Map<Face, TrueTypeFont | null>();
  private pending = new Map<Face, Promise<void>>();
  private fonts = new Map<TrueTypeFont, EmbeddedFont>();
  private std = new Map<string, StdFont>();
  private cache = new Map<string, Map<number, Pick | null>>();
  private next = 1;
  /** Families asked for that another font stood in for. */
  readonly substituted = new Set<string>();
  /** Whether any text is drawn in one of the standard fonts, which a file does not hold (an archive PDF can have none). */
  get usesStandardFonts(): boolean {
    return this.std.size > 0;
  }
  private standIns: Record<Kind, string[]>;

  constructor(faces: Face[], opts: FontSetOptions = {}) {
    for (const f of faces) {
      const k = f.family.toLowerCase();
      const list = this.byFamily.get(k);
      if (list) list.push(f);
      else this.byFamily.set(k, [f]);
    }
    this.standIns = opts.standIns ?? STAND_INS;
  }

  /** The families to try for a key, in order: the named ones, then the stand-ins for the kind of font. */
  private chain(key: FontKey): string[] {
    return [...key.families.filter((f) => !isGeneric(f)), ...this.standIns[kindOf(key.families)]];
  }

  /** The face of `family` that covers a character, for a weight and style. */
  private face(family: string, key: FontKey, cp: number): Face | null {
    const list = this.byFamily.get(family.toLowerCase());
    if (!list) return null;
    return bestFace(
      list.filter((f) => inRanges(f.ranges, cp)),
      key.weight,
      key.italic,
    );
  }

  private ensure(face: Face): Promise<void> {
    let p = this.pending.get(face);
    if (!p) {
      p = face.load().then(
        (t) => void this.files.set(face, t),
        () => void this.files.set(face, null),
      );
      this.pending.set(face, p);
    }
    return p;
  }

  /** Read the font files the characters of a run of text will need, so that `pick` can answer at once. */
  async prepare(key: FontKey, chars: Iterable<number>): Promise<void> {
    let rest = new Set(chars);
    for (const family of this.chain(key)) {
      if (!rest.size) break;
      const wanted = new Map<Face, number[]>();
      for (const cp of rest) {
        const f = this.face(family, key, cp);
        if (f) wanted.set(f, [...(wanted.get(f) ?? []), cp]);
      }
      await Promise.all([...wanted.keys()].map((f) => this.ensure(f)));
      rest = new Set(rest);
      for (const [f, cps] of wanted) {
        const ttf = this.files.get(f);
        if (ttf) for (const cp of cps) if (ttf.glyphId(cp)) rest.delete(cp);
      }
    }
  }

  /** How to draw a character in a font key: an embedded font, a standard one, or null (draw it as a picture). */
  pick(key: FontKey, cp: number): Pick | null {
    const ks = keyString(key);
    let memo = this.cache.get(ks);
    if (!memo) this.cache.set(ks, (memo = new Map()));
    if (memo.has(cp)) return memo.get(cp)!;
    const found = this.choose(key, cp);
    memo.set(cp, found);
    return found;
  }

  private choose(key: FontKey, cp: number): Pick | null {
    for (const family of this.chain(key)) {
      const face = this.face(family, key, cp);
      const ttf = face && this.files.get(face);
      const gid = ttf?.glyphId(cp);
      if (!face || !ttf || !gid) continue;
      let embedded = this.fonts.get(ttf);
      if (!embedded) this.fonts.set(ttf, (embedded = new EmbeddedFont(ttf, this.next++, face.family)));
      // Bold or italic asked for, and the face found is neither: draw it thickened or slanted, as browsers do.
      // (A variable font's outlines are its default weight, whatever range the face declares.)
      const synthBold = key.weight >= 600 && ttf.weightClass < 600;
      const synthItalic = key.italic && !ttf.italic && face.style === "normal";
      return { kind: "ttf", font: embedded, gid, cp, unitsPerEm: ttf.unitsPerEm, synthBold, synthItalic };
    }
    // Standard fonts: only characters WinAnsi has.
    const code = winAnsi(cp);
    if (code < 0) return null;
    const kind = kindOf(key.families);
    const bold = key.weight >= 600;
    const name = stdName(kind, bold, key.italic);
    let font = this.std.get(name);
    if (!font) this.std.set(name, (font = new StdFont(`F${this.next++}`, name)));
    return { kind: "std", font, code, cp, kindOf: kind, bold, italic: key.italic, synthBold: false, synthItalic: false };
  }

  /** Note that a character was drawn: a family that was asked for and not used goes on the list of stand-ins. */
  noteUse(key: FontKey, pick: Pick | null) {
    if (!pick) return;
    const primary = key.families.find((f) => !isGeneric(f));
    if (!primary) return;
    if (pick.kind === "std" || pick.font.family.toLowerCase() !== primary.toLowerCase()) this.substituted.add(primary);
  }

  /** Write every font used; returns the resource dictionary entries, `/F1 12 0 R …`. */
  async write(w: PdfWriter): Promise<string> {
    let out = "";
    for (const f of this.fonts.values()) {
      if (f.glyphs.size === 0) continue;
      out += `/${f.resource} ${ref(await f.write(w))} `;
    }
    for (const s of this.std.values()) out += `/${s.resource} ${ref(s.write(w))} `;
    return out.trim();
  }
}

function stdName(kind: Kind, bold: boolean, italic: boolean): string {
  if (kind === "mono") return `Courier${bold || italic ? "-" : ""}${bold ? "Bold" : ""}${italic ? "Oblique" : ""}`;
  if (kind === "sans") return `Helvetica${bold || italic ? "-" : ""}${bold ? "Bold" : ""}${italic ? "Oblique" : ""}`;
  return bold || italic ? `Times-${bold ? "Bold" : ""}${italic ? "Italic" : ""}` : "Times-Roman";
}
