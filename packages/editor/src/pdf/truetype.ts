// A reader for TrueType fonts (and OpenType fonts with TrueType outlines), enough to embed them in a PDF:
// the tables that describe the font (head, hhea, hmtx, maxp, OS/2, post, name), character lookup (cmap
// formats 0, 4, 6 and 12), and a subsetter that keeps only the glyphs a document uses. No DOM.

interface Table {
  offset: number;
  length: number;
}

/** A name id from the `name` table. */
const NAME_FAMILY = 1;
const NAME_SUBFAMILY = 2;
const NAME_FULL = 4;
const NAME_POSTSCRIPT = 6;

export class TrueTypeFont {
  readonly view: DataView;
  readonly tables = new Map<string, Table>();
  unitsPerEm = 1000;
  numGlyphs = 0;
  /** Horizontal metrics in the file's units. */
  ascender = 0;
  descender = 0;
  lineGap = 0;
  typoAscender = 0;
  typoDescender = 0;
  typoLineGap = 0;
  winAscent = 0;
  winDescent = 0;
  /** OS/2 says to use the typographic metrics. */
  useTypoMetrics = false;
  bbox: [number, number, number, number] = [0, 0, 0, 0];
  italicAngle = 0;
  isFixedPitch = false;
  underlinePosition = 0;
  underlineThickness = 0;
  strikeoutPosition = 0;
  strikeoutSize = 0;
  capHeight = 0;
  xHeight = 0;
  weightClass = 400;
  /** The embedding permissions (OS/2 fsType). */
  fsType = 0;
  /** Whether the font is italic or oblique (from OS/2 fsSelection, else the head macStyle). */
  italic = false;
  bold = false;
  names: Record<number, string> = {};
  /** The outlines are CFF (an .otf), which this reader can't subset. */
  readonly isCff: boolean;
  /** The font has variation tables: the outlines are the default instance. */
  readonly isVariable: boolean;
  private numHMetrics = 0;
  private locaFormat = 0;
  private cmaps: { format: number; offset: number; symbol: boolean }[] = [];
  private cache = new Map<number, number>();

  readonly bytes: Uint8Array;

  constructor(bytes: Uint8Array) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let base = 0;
    let tag = this.tag(0);
    if (tag === "ttcf") {
      base = this.u32(12);
      tag = this.tag(base);
    }
    if (tag !== "\0\x01\0\0" && tag !== "true" && tag !== "OTTO") throw new Error("Not a TrueType font");
    const count = this.u16(base + 4);
    for (let i = 0; i < count; i++) {
      const at = base + 12 + i * 16;
      this.tables.set(this.tag(at), { offset: this.u32(at + 8), length: this.u32(at + 12) });
    }
    this.isCff = tag === "OTTO" || this.tables.has("CFF ") || this.tables.has("CFF2");
    this.isVariable = this.tables.has("fvar");
    for (const need of ["head", "hhea", "maxp", "hmtx"]) if (!this.tables.has(need)) throw new Error(`The font has no ${need} table`);
    if (!this.isCff && (!this.tables.has("glyf") || !this.tables.has("loca"))) throw new Error("The font has no outlines");
    this.readHead();
    this.readHhea();
    this.numGlyphs = this.u16(this.at("maxp") + 4);
    this.readOs2();
    this.readPost();
    this.readNames();
    if (this.tables.has("cmap")) this.readCmap();
  }

  private tag(o: number): string {
    return String.fromCharCode(this.view.getUint8(o), this.view.getUint8(o + 1), this.view.getUint8(o + 2), this.view.getUint8(o + 3));
  }
  private u16(o: number): number {
    return this.view.getUint16(o);
  }
  private i16(o: number): number {
    return this.view.getInt16(o);
  }
  private u32(o: number): number {
    return this.view.getUint32(o);
  }
  private at(tag: string): number {
    return this.tables.get(tag)!.offset;
  }

  private readHead() {
    const o = this.at("head");
    this.unitsPerEm = this.u16(o + 18) || 1000;
    this.bbox = [this.i16(o + 36), this.i16(o + 38), this.i16(o + 40), this.i16(o + 42)];
    const mac = this.u16(o + 44);
    this.bold = !!(mac & 1);
    this.italic = !!(mac & 2);
    this.locaFormat = this.i16(o + 50);
  }

  private readHhea() {
    const o = this.at("hhea");
    this.ascender = this.i16(o + 4);
    this.descender = this.i16(o + 6);
    this.lineGap = this.i16(o + 8);
    this.numHMetrics = this.u16(o + 34);
  }

  private readOs2() {
    const t = this.tables.get("OS/2");
    if (!t) return;
    const o = t.offset;
    const version = this.u16(o);
    this.weightClass = this.u16(o + 4) || 400;
    this.fsType = this.u16(o + 8);
    this.strikeoutSize = this.i16(o + 26);
    this.strikeoutPosition = this.i16(o + 28);
    const sel = this.u16(o + 62);
    if (sel & 1) this.italic = true;
    if (sel & 32) this.bold = true;
    this.useTypoMetrics = !!(sel & 128);
    this.typoAscender = this.i16(o + 68);
    this.typoDescender = this.i16(o + 70);
    this.typoLineGap = this.i16(o + 72);
    this.winAscent = this.u16(o + 74);
    this.winDescent = this.u16(o + 76);
    if (version >= 2 && t.length >= 90) {
      this.xHeight = this.i16(o + 86);
      this.capHeight = this.i16(o + 88);
    }
  }

  private readPost() {
    const t = this.tables.get("post");
    if (!t) return;
    const o = t.offset;
    this.italicAngle = this.i16(o + 4) + this.u16(o + 6) / 65536;
    this.underlinePosition = this.i16(o + 8);
    this.underlineThickness = this.i16(o + 10);
    this.isFixedPitch = this.u32(o + 12) !== 0;
  }

  private readNames() {
    const t = this.tables.get("name");
    if (!t) return;
    const o = t.offset;
    const count = this.u16(o + 2);
    const strings = o + this.u16(o + 4);
    const found = new Map<number, { rank: number; text: string }>();
    for (let i = 0; i < count; i++) {
      const r = o + 6 + i * 12;
      const platform = this.u16(r);
      const id = this.u16(r + 6);
      if (id !== NAME_FAMILY && id !== NAME_SUBFAMILY && id !== NAME_FULL && id !== NAME_POSTSCRIPT) continue;
      const len = this.u16(r + 8);
      const off = strings + this.u16(r + 10);
      if (off + len > this.bytes.length) continue;
      // Windows names are UTF-16, Mac names single bytes (ASCII is all a PostScript name uses). Prefer English.
      const rank = platform === 3 && this.u16(r + 4) === 0x409 ? 3 : platform === 3 ? 2 : platform === 1 ? 1 : 0;
      if (rank === 0 || (found.get(id)?.rank ?? -1) >= rank) continue;
      let text = "";
      if (platform === 3 || platform === 0) for (let k = 0; k + 1 < len; k += 2) text += String.fromCharCode(this.u16(off + k));
      else for (let k = 0; k < len; k++) text += String.fromCharCode(this.view.getUint8(off + k));
      found.set(id, { rank, text });
    }
    for (const [id, v] of found) this.names[id] = v.text;
  }

  private readCmap() {
    const o = this.at("cmap");
    const n = this.u16(o + 2);
    const subs: { platform: number; encoding: number; offset: number; format: number }[] = [];
    for (let i = 0; i < n; i++) {
      const platform = this.u16(o + 4 + i * 8);
      const encoding = this.u16(o + 6 + i * 8);
      const offset = o + this.u32(o + 8 + i * 8);
      if (offset + 4 > this.bytes.length) continue;
      subs.push({ platform, encoding, offset, format: this.u16(offset) });
    }
    const usable = (s: { format: number }) => s.format === 0 || s.format === 4 || s.format === 6 || s.format === 12;
    const unicode = (s: { platform: number; encoding: number }) => (s.platform === 3 && (s.encoding === 1 || s.encoding === 10)) || s.platform === 0;
    // Full Unicode first, then the Basic Multilingual Plane, then symbol fonts.
    const ranked = subs.filter(usable).sort((a, b) => Number(unicode(b)) - Number(unicode(a)) || Number(b.format === 12) - Number(a.format === 12));
    for (const s of ranked) {
      const symbol = s.platform === 3 && s.encoding === 0;
      if (unicode(s) || symbol || (s.platform === 1 && s.encoding === 0)) this.cmaps.push({ format: s.format, offset: s.offset, symbol });
    }
  }

  /** The width of a glyph in font units. */
  advance(gid: number): number {
    if (this.numHMetrics === 0) return 0;
    const i = Math.min(gid, this.numHMetrics - 1);
    return this.u16(this.at("hmtx") + i * 4);
  }

  /** The glyph for a character, or 0 when the font has none. */
  glyphId(cp: number): number {
    const known = this.cache.get(cp);
    if (known !== undefined) return known;
    let gid = 0;
    for (const c of this.cmaps) {
      gid = this.lookup(c.format, c.offset, cp);
      // Symbol fonts keep their characters in the private use area.
      if (!gid && c.symbol && cp < 0x100) gid = this.lookup(c.format, c.offset, 0xf000 + cp);
      if (gid && gid < this.numGlyphs) break;
      gid = 0;
    }
    this.cache.set(cp, gid);
    return gid;
  }

  private lookup(format: number, o: number, cp: number): number {
    if (format === 0) return cp < 256 ? this.view.getUint8(o + 6 + cp) : 0;
    if (format === 6) {
      const first = this.u16(o + 6);
      const count = this.u16(o + 8);
      return cp >= first && cp < first + count ? this.u16(o + 10 + (cp - first) * 2) : 0;
    }
    if (format === 4) {
      if (cp > 0xffff) return 0;
      const segX2 = this.u16(o + 6);
      const segs = segX2 >> 1;
      const ends = o + 14;
      const starts = ends + segX2 + 2;
      const deltas = starts + segX2;
      const ranges = deltas + segX2;
      let lo = 0;
      let hi = segs - 1;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (this.u16(ends + mid * 2) < cp) lo = mid + 1;
        else hi = mid;
      }
      if (this.u16(ends + lo * 2) < cp || this.u16(starts + lo * 2) > cp) return 0;
      const delta = this.u16(deltas + lo * 2);
      const range = this.u16(ranges + lo * 2);
      if (range === 0) return (cp + delta) & 0xffff;
      const addr = ranges + lo * 2 + range + (cp - this.u16(starts + lo * 2)) * 2;
      if (addr + 2 > this.bytes.length) return 0;
      const g = this.u16(addr);
      return g === 0 ? 0 : (g + delta) & 0xffff;
    }
    if (format === 12) {
      const groups = this.u32(o + 12);
      let lo = 0;
      let hi = groups - 1;
      while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        const g = o + 16 + mid * 12;
        const start = this.u32(g);
        if (cp < start) hi = mid - 1;
        else if (cp > this.u32(g + 4)) lo = mid + 1;
        else return this.u32(g + 8) + (cp - start);
      }
    }
    return 0;
  }

  /** The glyph's data in the `glyf` table as [start, end) offsets from the table's start. */
  private glyphRange(gid: number): [number, number] {
    const loca = this.at("loca");
    if (this.locaFormat === 0) return [this.u16(loca + gid * 2) * 2, this.u16(loca + gid * 2 + 2) * 2];
    return [this.u32(loca + gid * 4), this.u32(loca + gid * 4 + 4)];
  }

  /** The box a glyph's outline fills, [xMin, yMin, xMax, yMax] in font units (y up); null for an empty glyph. */
  glyphBounds(gid: number): [number, number, number, number] | null {
    if (gid <= 0 || gid >= this.numGlyphs || !this.tables.has("glyf") || !this.tables.has("loca")) return null;
    const [s, e] = this.glyphRange(gid);
    if (e - s < 10) return null;
    const o = this.at("glyf") + s;
    if (o + 10 > this.bytes.length) return null;
    const box: [number, number, number, number] = [this.i16(o + 2), this.i16(o + 4), this.i16(o + 6), this.i16(o + 8)];
    return box[2] > box[0] && box[3] > box[1] ? box : null;
  }

  /** The glyphs a composite glyph is built from. */
  private components(gid: number): number[] {
    const [s, e] = this.glyphRange(gid);
    if (e - s < 10) return [];
    const o = this.at("glyf") + s;
    if (this.i16(o) >= 0) return [];
    const out: number[] = [];
    let p = o + 10;
    for (;;) {
      const flags = this.u16(p);
      out.push(this.u16(p + 2));
      p += 4 + (flags & 1 ? 4 : 2);
      if (flags & 0x8) p += 2;
      else if (flags & 0x40) p += 4;
      else if (flags & 0x80) p += 8;
      if (!(flags & 0x20)) break;
    }
    return out;
  }

  /**
   * A copy of the font that keeps only these glyphs (and the parts composite glyphs are built from).
   * Glyph numbers stay the same, so text already encoded with them still works; the glyphs left out
   * are empty. Only the tables a PDF viewer needs are kept.
   */
  subset(used: Iterable<number>): Uint8Array {
    const keep = new Set<number>([0]);
    const todo = [...used];
    while (todo.length) {
      const g = todo.pop()!;
      if (g < 0 || g >= this.numGlyphs || keep.has(g)) continue;
      keep.add(g);
      todo.push(...this.components(g));
    }
    const glyf = this.at("glyf");
    const chunks: Uint8Array[] = [];
    const loca = new Uint8Array((this.numGlyphs + 1) * 4);
    const lv = new DataView(loca.buffer);
    let size = 0;
    for (let g = 0; g < this.numGlyphs; g++) {
      lv.setUint32(g * 4, size);
      if (!keep.has(g)) continue;
      const [s, e] = this.glyphRange(g);
      if (e <= s) continue;
      const data = this.bytes.subarray(glyf + s, glyf + e);
      const padded = (data.length + 3) & ~3;
      const out = new Uint8Array(padded);
      out.set(data);
      chunks.push(out);
      size += padded;
    }
    lv.setUint32(this.numGlyphs * 4, size);
    const glyfData = new Uint8Array(size);
    let at = 0;
    for (const c of chunks) {
      glyfData.set(c, at);
      at += c.length;
    }
    const tables: [string, Uint8Array][] = [];
    const copy = (tag: string) => {
      const t = this.tables.get(tag);
      if (t) tables.push([tag, this.bytes.slice(t.offset, t.offset + t.length)]);
    };
    for (const tag of ["head", "hhea", "maxp", "hmtx", "cvt ", "fpgm", "prep"]) copy(tag);
    tables.push(["glyf", glyfData], ["loca", loca]);
    const head = tables.find(([t]) => t === "head")![1];
    const hv = new DataView(head.buffer);
    hv.setUint32(8, 0);
    hv.setInt16(50, 1);
    return buildSfnt(tables);
  }
}

/** The checksum of a table: the sum of its 32-bit words. */
function checksum(data: Uint8Array): number {
  let sum = 0;
  const n = data.length & ~3;
  const v = new DataView(data.buffer, data.byteOffset, data.byteLength);
  for (let i = 0; i < n; i += 4) sum = (sum + v.getUint32(i)) >>> 0;
  if (data.length > n) {
    let last = 0;
    for (let i = n; i < data.length; i++) last |= data[i] << (24 - (i - n) * 8);
    sum = (sum + (last >>> 0)) >>> 0;
  }
  return sum;
}

/** Tables to a font file, with the checksums a font needs. */
export function buildSfnt(tables: [string, Uint8Array][]): Uint8Array {
  const sorted = [...tables].sort((a, b) => (a[0] < b[0] ? -1 : 1));
  const n = sorted.length;
  let pow = 1;
  let log = 0;
  while (pow * 2 <= n) {
    pow *= 2;
    log++;
  }
  let offset = 12 + n * 16;
  const placed = sorted.map(([tag, data]) => {
    const at = offset;
    offset += (data.length + 3) & ~3;
    return { tag, data, at };
  });
  const out = new Uint8Array(offset);
  const v = new DataView(out.buffer);
  v.setUint32(0, 0x00010000);
  v.setUint16(4, n);
  v.setUint16(6, pow * 16);
  v.setUint16(8, log);
  v.setUint16(10, n * 16 - pow * 16);
  placed.forEach(({ tag, data, at }, i) => {
    const r = 12 + i * 16;
    for (let k = 0; k < 4; k++) out[r + k] = tag.charCodeAt(k);
    v.setUint32(r + 4, checksum(data));
    v.setUint32(r + 8, at);
    v.setUint32(r + 12, data.length);
    out.set(data, at);
  });
  // The whole file adds up to a fixed number once head.checkSumAdjustment is right.
  const head = placed.find((p) => p.tag === "head");
  if (head) v.setUint32(head.at + 8, (0xb1b0afba - checksum(out)) >>> 0);
  return out;
}
