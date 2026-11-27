// WOFF2 to a plain TrueType file. WOFF2 compresses all the tables together with Brotli and stores glyphs
// (and sometimes hmtx) in a form that packs the numbers more tightly; this undoes both. Written from the
// W3C WOFF2 specification. No DOM.
import { brotliDecompress } from "./brotli.ts";
import { buildSfnt } from "./truetype.ts";

const KNOWN_TAGS = [
  "cmap", "head", "hhea", "hmtx", "maxp", "name", "OS/2", "post", "cvt ", "fpgm", "glyf", "loca", "prep", "CFF ", "VORG", "EBDT", "EBLC", "gasp", "hdmx", "kern", "LTSH", "PCLT", "VDMX", "vhea", "vmtx",
  "BASE", "GDEF", "GPOS", "GSUB", "EBSC", "JSTF", "MATH", "CBDT", "CBLC", "COLR", "CPAL", "SVG ", "sbix", "acnt", "avar", "bdat", "bloc", "bsln", "cvar", "fdsc", "feat", "fmtx", "fvar", "gvar", "hsty",
  "just", "lcar", "mort", "morx", "opbd", "prop", "trak", "Zapf", "Silf", "Glat", "Gloc", "Feat", "Sill",
];

/** A cursor over bytes. */
class Reader {
  pos = 0;
  readonly bytes: Uint8Array;
  readonly view: DataView;
  constructor(bytes: Uint8Array) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  u8(): number {
    if (this.pos >= this.bytes.length) throw new Error("The WOFF2 data is cut short");
    return this.bytes[this.pos++];
  }
  u16(): number {
    const v = this.view.getUint16(this.pos);
    this.pos += 2;
    return v;
  }
  i16(): number {
    const v = this.view.getInt16(this.pos);
    this.pos += 2;
    return v;
  }
  u32(): number {
    const v = this.view.getUint32(this.pos);
    this.pos += 4;
    return v;
  }
  take(n: number): Uint8Array {
    if (this.pos + n > this.bytes.length) throw new Error("The WOFF2 data is cut short");
    const out = this.bytes.subarray(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }
  /** A number of up to five bytes, seven bits each. */
  base128(): number {
    let v = 0;
    for (let i = 0; i < 5; i++) {
      const b = this.u8();
      if (i === 0 && b === 0x80) throw new Error("Invalid WOFF2 number");
      v = v * 128 + (b & 0x7f);
      if (!(b & 0x80)) return v;
    }
    throw new Error("Invalid WOFF2 number");
  }
  /** A number that takes one, two or three bytes. */
  u255(): number {
    const code = this.u8();
    if (code === 253) return this.u16();
    if (code === 255) return this.u8() + 253;
    if (code === 254) return this.u8() + 506;
    return code;
  }
}

interface Entry {
  tag: string;
  transformed: boolean;
  origLength: number;
  length: number;
  data: Uint8Array;
}

export async function decodeWoff2(bytes: Uint8Array): Promise<Uint8Array> {
  const r = new Reader(bytes);
  if (r.u32() !== 0x774f4632) throw new Error("Not a WOFF2 file");
  const flavor = r.u32();
  if (flavor === 0x74746366) throw new Error("WOFF2 collections are not supported");
  r.u32(); // length
  const count = r.u16();
  r.u16();
  const totalSfnt = r.u32();
  const compressedSize = r.u32();
  r.pos = 48;
  const entries: Entry[] = [];
  for (let i = 0; i < count; i++) {
    const flags = r.u8();
    const index = flags & 63;
    const tag = index === 63 ? String.fromCharCode(...r.take(4)) : KNOWN_TAGS[index];
    const version = flags >> 6;
    // For glyf and loca version 0 means transformed; for the others version 0 means as it is.
    const transformed = tag === "glyf" || tag === "loca" ? version === 0 : version !== 0;
    const origLength = r.base128();
    const length = transformed ? r.base128() : origLength;
    entries.push({ tag, transformed, origLength, length, data: new Uint8Array(0) });
  }
  const data = await brotliDecompress(r.take(compressedSize), totalSfnt);
  let at = 0;
  for (const e of entries) {
    if (at + e.length > data.length) throw new Error("The WOFF2 tables are cut short");
    e.data = data.subarray(at, at + e.length);
    at += e.length;
  }

  const byTag = new Map(entries.map((e) => [e.tag, e]));
  const out = new Map<string, Uint8Array>();
  let xMins: Int16Array | null = null;
  const glyf = byTag.get("glyf");
  if (glyf?.transformed) {
    const rebuilt = reconstructGlyf(glyf.data);
    out.set("glyf", rebuilt.glyf);
    out.set("loca", rebuilt.loca);
    xMins = rebuilt.xMins;
  }
  for (const e of entries) {
    if (out.has(e.tag)) continue;
    if (e.tag === "loca" && e.transformed) continue;
    if (e.tag === "hmtx" && e.transformed) {
      const head = byTag.get("hhea");
      const maxp = byTag.get("maxp");
      if (!head || !maxp || !xMins) throw new Error("The WOFF2 hmtx table can't be rebuilt");
      out.set("hmtx", reconstructHmtx(e.data, new DataView(maxp.data.buffer, maxp.data.byteOffset, maxp.data.byteLength).getUint16(4), new DataView(head.data.buffer, head.data.byteOffset, head.data.byteLength).getUint16(34), xMins));
    } else out.set(e.tag, e.data.slice());
  }
  // The rebuilt loca table is in the long format.
  if (glyf?.transformed) {
    const head = out.get("head");
    if (head) {
      const h = head.slice();
      new DataView(h.buffer).setInt16(50, 1);
      out.set("head", h);
    }
  }
  const sfnt = buildSfnt([...out]);
  new DataView(sfnt.buffer).setUint32(0, flavor);
  return sfnt;
}

// -- glyf ------------------------------------------------------------------------------------------

const withSign = (flag: number, v: number) => (flag & 1 ? v : -v);

function reconstructGlyf(data: Uint8Array): { glyf: Uint8Array; loca: Uint8Array; xMins: Int16Array } {
  const h = new Reader(data);
  h.u16();
  const options = h.u16();
  const numGlyphs = h.u16();
  h.u16(); // index format: the loca rebuilt here is always long
  const sizes = [h.u32(), h.u32(), h.u32(), h.u32(), h.u32(), h.u32(), h.u32()];
  const [nContourSize, nPointsSize, flagSize, glyphSize, compositeSize, bboxSize, instrSize] = sizes;
  let pos = 36;
  const stream = (n: number) => {
    const s = new Reader(data.subarray(pos, pos + n));
    pos += n;
    return s;
  };
  const nContours = stream(nContourSize);
  const nPoints = stream(nPointsSize);
  const flags = stream(flagSize);
  const glyphs = stream(glyphSize);
  const composites = stream(compositeSize);
  const bboxes = stream(bboxSize);
  const instructions = stream(instrSize);
  const overlap = options & 1 ? data.subarray(pos, pos + ((numGlyphs + 7) >> 3)) : null;
  const bboxBitmap = bboxes.take(4 * ((numGlyphs + 31) >> 5));

  const chunks: Uint8Array[] = [];
  const loca = new Uint8Array((numGlyphs + 1) * 4);
  const lv = new DataView(loca.buffer);
  const xMins = new Int16Array(numGlyphs);
  let total = 0;
  for (let g = 0; g < numGlyphs; g++) {
    lv.setUint32(g * 4, total);
    const n = nContours.i16();
    const hasBox = !!(bboxBitmap[g >> 3] & (0x80 >> (g & 7)));
    let glyph: Uint8Array | null = null;
    if (n > 0) {
      const ends: number[] = [];
      let points = 0;
      for (let c = 0; c < n; c++) {
        points += nPoints.u255();
        ends.push(points - 1);
      }
      const flagBytes = flags.take(points);
      const xs: number[] = [];
      const ys: number[] = [];
      let x = 0;
      let y = 0;
      for (let p = 0; p < points; p++) {
        const f = flagBytes[p] & 0x7f;
        let dx: number;
        let dy: number;
        if (f < 10) {
          dx = 0;
          dy = withSign(f, ((f & 14) << 7) + glyphs.u8());
        } else if (f < 20) {
          dx = withSign(f, (((f - 10) & 14) << 7) + glyphs.u8());
          dy = 0;
        } else if (f < 84) {
          const b0 = f - 20;
          const b1 = glyphs.u8();
          dx = withSign(f, 1 + (b0 & 0x30) + (b1 >> 4));
          dy = withSign(f >> 1, 1 + ((b0 & 0x0c) << 2) + (b1 & 0x0f));
        } else if (f < 120) {
          const b0 = f - 84;
          dx = withSign(f, 1 + (Math.floor(b0 / 12) << 8) + glyphs.u8());
          dy = withSign(f >> 1, 1 + (((b0 % 12) >> 2) << 8) + glyphs.u8());
        } else if (f < 124) {
          const b1 = glyphs.u8();
          const b2 = glyphs.u8();
          const b3 = glyphs.u8();
          dx = withSign(f, (b1 << 4) + (b2 >> 4));
          dy = withSign(f >> 1, ((b2 & 0x0f) << 8) + b3);
        } else {
          const b1 = glyphs.u8();
          const b2 = glyphs.u8();
          const b3 = glyphs.u8();
          const b4 = glyphs.u8();
          dx = withSign(f, (b1 << 8) + b2);
          dy = withSign(f >> 1, (b3 << 8) + b4);
        }
        x += dx;
        y += dy;
        xs.push(x);
        ys.push(y);
      }
      const instrLen = glyphs.u255();
      const instr = instructions.take(instrLen);
      let box: [number, number, number, number];
      if (hasBox) box = [bboxes.i16(), bboxes.i16(), bboxes.i16(), bboxes.i16()];
      else box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
      glyph = simpleGlyph(n, box, ends, instr, flagBytes, xs, ys, !!overlap && !!(overlap[g >> 3] & (0x80 >> (g & 7))));
      xMins[g] = box[0];
    } else if (n < 0) {
      // A composite: copy its components as they are.
      const start = composites.pos;
      let more = true;
      let hasInstr = false;
      while (more) {
        const f = composites.u16();
        composites.u16();
        composites.take(f & 1 ? 4 : 2);
        if (f & 0x8) composites.take(2);
        else if (f & 0x40) composites.take(4);
        else if (f & 0x80) composites.take(8);
        more = !!(f & 0x20);
        if (f & 0x100) hasInstr = true;
      }
      const comps = composites.bytes.subarray(start, composites.pos);
      const instr = hasInstr ? instructions.take(glyphs.u255()) : new Uint8Array(0);
      if (!hasBox) throw new Error("A WOFF2 composite glyph has no bounding box");
      const box = [bboxes.i16(), bboxes.i16(), bboxes.i16(), bboxes.i16()];
      const size = 10 + comps.length + (hasInstr ? 2 + instr.length : 0);
      glyph = new Uint8Array(size);
      const v = new DataView(glyph.buffer);
      v.setInt16(0, -1);
      box.forEach((b, i) => v.setInt16(2 + i * 2, b));
      glyph.set(comps, 10);
      if (hasInstr) {
        v.setUint16(10 + comps.length, instr.length);
        glyph.set(instr, 12 + comps.length);
      }
      xMins[g] = box[0];
    }
    if (glyph) {
      const padded = new Uint8Array((glyph.length + 3) & ~3);
      padded.set(glyph);
      chunks.push(padded);
      total += padded.length;
    }
  }
  lv.setUint32(numGlyphs * 4, total);
  const glyf = new Uint8Array(total);
  let o = 0;
  for (const c of chunks) {
    glyf.set(c, o);
    o += c.length;
  }
  return { glyf, loca, xMins };
}

/** A simple glyph in TrueType's own format: points as short or long deltas with flags. */
function simpleGlyph(n: number, box: number[], ends: number[], instr: Uint8Array, kinds: Uint8Array, xs: number[], ys: number[], overlap: boolean): Uint8Array {
  const points = xs.length;
  const flags = new Uint8Array(points);
  const xb: number[] = [];
  const yb: number[] = [];
  let px = 0;
  let py = 0;
  for (let i = 0; i < points; i++) {
    let f = kinds[i] & 0x80 ? 0 : 1;
    if (i === 0 && overlap) f |= 0x40;
    const dx = xs[i] - px;
    const dy = ys[i] - py;
    if (dx === 0) f |= 0x10;
    else if (Math.abs(dx) < 256) {
      f |= 0x02 | (dx > 0 ? 0x10 : 0);
      xb.push(Math.abs(dx));
    } else xb.push(dx);
    if (dy === 0) f |= 0x20;
    else if (Math.abs(dy) < 256) {
      f |= 0x04 | (dy > 0 ? 0x20 : 0);
      yb.push(Math.abs(dy));
    } else yb.push(dy);
    flags[i] = f;
    px = xs[i];
    py = ys[i];
  }
  let size = 10 + n * 2 + 2 + instr.length + points;
  for (const f of flags) {
    size += f & 0x02 ? 1 : f & 0x10 ? 0 : 2;
    size += f & 0x04 ? 1 : f & 0x20 ? 0 : 2;
  }
  const out = new Uint8Array(size);
  const v = new DataView(out.buffer);
  v.setInt16(0, n);
  box.forEach((b, i) => v.setInt16(2 + i * 2, b));
  let o = 10;
  for (const e of ends) {
    v.setUint16(o, e);
    o += 2;
  }
  v.setUint16(o, instr.length);
  o += 2;
  out.set(instr, o);
  o += instr.length;
  out.set(flags, o);
  o += points;
  for (let i = 0, k = 0; i < points; i++) {
    if (flags[i] & 0x02) out[o++] = xb[k++];
    else if (!(flags[i] & 0x10)) {
      v.setInt16(o, xb[k++]);
      o += 2;
    }
  }
  for (let i = 0, k = 0; i < points; i++) {
    if (flags[i] & 0x04) out[o++] = yb[k++];
    else if (!(flags[i] & 0x20)) {
      v.setInt16(o, yb[k++]);
      o += 2;
    }
  }
  return out;
}

// -- hmtx ------------------------------------------------------------------------------------------

/** The hmtx table without the left side bearings that equal the glyphs' xMin, with them put back. */
function reconstructHmtx(data: Uint8Array, numGlyphs: number, numHMetrics: number, xMins: Int16Array): Uint8Array {
  const r = new Reader(data);
  const flags = r.u8();
  const advances: number[] = [];
  for (let i = 0; i < numHMetrics; i++) advances.push(r.u16());
  const lsb: number[] = [];
  for (let i = 0; i < numHMetrics; i++) lsb.push(flags & 1 ? xMins[i] : r.i16());
  const rest: number[] = [];
  for (let i = numHMetrics; i < numGlyphs; i++) rest.push(flags & 2 ? xMins[i] : r.i16());
  const out = new Uint8Array(numHMetrics * 4 + rest.length * 2);
  const v = new DataView(out.buffer);
  for (let i = 0; i < numHMetrics; i++) {
    v.setUint16(i * 4, advances[i]);
    v.setInt16(i * 4 + 2, lsb[i]);
  }
  rest.forEach((l, i) => v.setInt16(numHMetrics * 4 + i * 2, l));
  return out;
}
