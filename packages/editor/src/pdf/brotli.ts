// A Brotli decoder (RFC 7932). WOFF2 fonts are compressed with Brotli, which browsers can't decompress from
// script (DecompressionStream has no "brotli"), and Google Fonts serves only WOFF2 to Chrome. This is small
// and not fast, which is fine for one font file at a time. No DOM.
import { CONTEXT_LOOKUP, DICTIONARY, TRANSFORMS } from "./brotli-data.ts";
import { inflateRaw } from "./writer.ts";

/** log2 of the number of words of each length in the static dictionary. */
const DICT_BITS = [0, 0, 0, 0, 10, 10, 11, 11, 10, 10, 10, 10, 10, 9, 9, 8, 7, 7, 8, 7, 7, 6, 6, 5, 5];
const DICT_OFFSETS: number[] = (() => {
  const o = [0, 0, 0, 0, 0];
  for (let len = 5; len <= 24; len++) o[len] = o[len - 1] + (len - 1) * (1 << DICT_BITS[len - 1]);
  return o;
})();

const INSERT_BASE = [0, 1, 2, 3, 4, 5, 6, 8, 10, 14, 18, 26, 34, 50, 66, 98, 130, 194, 322, 578, 1090, 2114, 6210, 22594];
const INSERT_EXTRA = [0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 12, 14, 24];
const COPY_BASE = [2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 18, 22, 30, 38, 54, 70, 102, 134, 198, 326, 582, 1094, 2118];
const COPY_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 7, 8, 9, 10, 24];
const BLOCK_BASE = [1, 5, 9, 13, 17, 25, 33, 41, 49, 65, 81, 97, 113, 145, 177, 209, 241, 305, 369, 497, 753, 1265, 2289, 4337, 8433, 16625];
const BLOCK_EXTRA = [2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 7, 8, 9, 10, 11, 12, 13, 24];
/** Insert and copy length codes of each group of 64 command symbols (the first two groups imply distance 0). */
const INSERT_GROUP = [0, 0, 0, 0, 8, 8, 0, 16, 8, 16, 16];
const COPY_GROUP = [0, 8, 0, 8, 0, 8, 16, 0, 16, 8, 16];
const CODE_LENGTH_ORDER = [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];
/** The fixed code for the lengths of the code length code, indexed by the next four bits: [bits used, value]. */
const CL_LEN = [2, 2, 2, 3, 2, 2, 2, 4, 2, 2, 2, 3, 2, 2, 2, 4];
const CL_VAL = [0, 4, 3, 2, 0, 4, 3, 1, 0, 4, 3, 2, 0, 4, 3, 5];

const LOOKUP = Uint8Array.from(atob(CONTEXT_LOOKUP), (c) => c.charCodeAt(0));
/** Where the two context tables of each mode start in LOOKUP. */
const LOOKUP_OFFSETS = [1024, 1536, 1280, 1536, 0, 256, 768, 512];

/** A prefix code, decoded like Deflate's: by counting codes of each length. */
interface Code {
  counts: Uint16Array;
  symbols: Uint16Array;
  /** The only symbol of a code that uses no bits, or -1. */
  single: number;
}

function buildCode(lengths: ArrayLike<number>, size: number): Code {
  const counts = new Uint16Array(16);
  for (let i = 0; i < size; i++) counts[lengths[i]]++;
  counts[0] = 0;
  const offs = new Uint16Array(16);
  for (let l = 1; l < 15; l++) offs[l + 1] = offs[l] + counts[l];
  const symbols = new Uint16Array(size);
  for (let i = 0; i < size; i++) if (lengths[i]) symbols[offs[lengths[i]]++] = i;
  return { counts, symbols, single: -1 };
}

function singleCode(symbol: number): Code {
  return { counts: new Uint16Array(16), symbols: new Uint16Array(0), single: symbol };
}

class Decoder {
  private input: Uint8Array;
  private pos = 0;
  private acc = 0;
  private nbits = 0;
  private out: Uint8Array;
  private len = 0;
  private dictionary: Uint8Array;

  constructor(input: Uint8Array, dictionary: Uint8Array, sizeHint: number) {
    this.input = input;
    this.dictionary = dictionary;
    this.out = new Uint8Array(Math.max(sizeHint, 1024));
  }

  // -- bits --------------------------------------------------------------------------------------

  private need(n: number) {
    while (this.nbits < n) {
      this.acc |= (this.pos < this.input.length ? this.input[this.pos] : 0) << this.nbits;
      this.pos++;
      this.nbits += 8;
    }
    if (this.pos > this.input.length + 8) throw new Error("Brotli data is cut short");
  }

  private bits(n: number): number {
    if (n === 0) return 0;
    this.need(n);
    const v = this.acc & ((1 << n) - 1);
    this.acc >>>= n;
    this.nbits -= n;
    return v;
  }

  private align() {
    const drop = this.nbits & 7;
    this.acc >>>= drop;
    this.nbits -= drop;
    // Whole bytes still in the buffer go back to the input.
    this.pos -= this.nbits >> 3;
    this.acc = 0;
    this.nbits = 0;
  }

  private decode(code: Code): number {
    if (code.single >= 0) return code.single;
    this.need(15);
    const acc = this.acc;
    let c = 0;
    let first = 0;
    let index = 0;
    for (let len = 1; len <= 15; len++) {
      c |= (acc >>> (len - 1)) & 1;
      const count = code.counts[len];
      if (c - count < first) {
        this.acc >>>= len;
        this.nbits -= len;
        return code.symbols[index + (c - first)];
      }
      index += count;
      first += count;
      first <<= 1;
      c <<= 1;
    }
    throw new Error("Invalid Brotli prefix code");
  }

  private varLen8(): number {
    if (!this.bits(1)) return 0;
    const n = this.bits(3);
    return n === 0 ? 1 : this.bits(n) + (1 << n);
  }

  // -- prefix codes ------------------------------------------------------------------------------

  private readCode(alphabet: number): Code {
    const hskip = this.bits(2);
    if (hskip === 1) {
      const nsym = this.bits(2) + 1;
      let maxBits = 0;
      while (1 << maxBits < alphabet) maxBits++;
      const symbols: number[] = [];
      for (let i = 0; i < nsym; i++) {
        const s = this.bits(maxBits);
        if (s >= alphabet) throw new Error("Invalid Brotli symbol");
        symbols.push(s);
      }
      if (new Set(symbols).size !== nsym) throw new Error("Repeated Brotli symbol");
      if (nsym === 1) return singleCode(symbols[0]);
      const lengths = new Uint8Array(alphabet);
      if (nsym === 2) {
        lengths[symbols[0]] = 1;
        lengths[symbols[1]] = 1;
      } else if (nsym === 3) {
        lengths[symbols[0]] = 1;
        lengths[symbols[1]] = 2;
        lengths[symbols[2]] = 2;
      } else if (this.bits(1)) {
        lengths[symbols[0]] = 1;
        lengths[symbols[1]] = 2;
        lengths[symbols[2]] = 3;
        lengths[symbols[3]] = 3;
      } else for (const s of symbols) lengths[s] = 2;
      return buildCode(lengths, alphabet);
    }
    // A complex code: first the code that spells the code lengths.
    const clLengths = new Uint8Array(18);
    let space = 32;
    let nonzero = 0;
    for (let i = hskip; i < 18 && space > 0; i++) {
      this.need(4);
      const peek = this.acc & 15;
      const l = CL_VAL[peek];
      this.acc >>>= CL_LEN[peek];
      this.nbits -= CL_LEN[peek];
      clLengths[CODE_LENGTH_ORDER[i]] = l;
      if (l) {
        space -= 32 >> l;
        nonzero++;
      }
    }
    if (nonzero !== 1 && space !== 0) throw new Error("Invalid Brotli code length code");
    const clCode = nonzero === 1 ? singleCode(clLengths.findIndex((l) => l !== 0)) : buildCode(clLengths, 18);
    const lengths = new Uint8Array(alphabet);
    let symbol = 0;
    let prev = 8;
    let repeat = 0;
    let repeatLen = 0;
    space = 32768;
    while (symbol < alphabet && space > 0) {
      const cl = this.decode(clCode);
      if (cl < 16) {
        repeat = 0;
        lengths[symbol++] = cl;
        if (cl) {
          prev = cl;
          space -= 32768 >> cl;
        }
      } else {
        const extra = cl === 16 ? 2 : 3;
        const newLen = cl === 16 ? prev : 0;
        if (repeatLen !== newLen) {
          repeat = 0;
          repeatLen = newLen;
        }
        const old = repeat;
        if (repeat > 0) {
          repeat -= 2;
          repeat <<= extra;
        }
        repeat += this.bits(extra) + 3;
        const delta = repeat - old;
        if (symbol + delta > alphabet) throw new Error("Invalid Brotli code lengths");
        for (let k = 0; k < delta; k++) lengths[symbol++] = repeatLen;
        if (repeatLen) space -= delta << (15 - repeatLen);
      }
    }
    if (space !== 0) throw new Error("Invalid Brotli code lengths");
    return buildCode(lengths, alphabet);
  }

  private readContextMap(trees: number, size: number): Uint8Array {
    const map = new Uint8Array(size);
    const rle = this.bits(1) ? this.bits(4) + 1 : 0;
    const code = this.readCode(trees + rle);
    for (let i = 0; i < size; ) {
      const s = this.decode(code);
      if (s === 0) map[i++] = 0;
      else if (s <= rle) {
        const reps = (1 << s) + this.bits(s);
        if (i + reps > size) throw new Error("Invalid Brotli context map");
        i += reps;
      } else map[i++] = s - rle;
    }
    if (this.bits(1)) {
      // Inverse move-to-front.
      const mtf = Array.from({ length: 256 }, (_, i) => i);
      for (let i = 0; i < size; i++) {
        const idx = map[i];
        const v = mtf[idx];
        map[i] = v;
        mtf.splice(idx, 1);
        mtf.unshift(v);
      }
    }
    return map;
  }

  // -- output ------------------------------------------------------------------------------------

  private ensure(n: number) {
    if (this.len + n <= this.out.length) return;
    let size = this.out.length * 2;
    while (size < this.len + n) size *= 2;
    const bigger = new Uint8Array(size);
    bigger.set(this.out.subarray(0, this.len));
    this.out = bigger;
  }

  /** A word of the static dictionary, changed by one of the transforms, appended to the output. */
  private dictionaryWord(length: number, wordId: number) {
    if (length < 4 || length > 24) throw new Error("Invalid Brotli dictionary reference");
    const bits = DICT_BITS[length];
    const index = wordId & ((1 << bits) - 1);
    const t = wordId >> bits;
    if (t >= TRANSFORMS.length) throw new Error("Invalid Brotli transform");
    const [prefix, type, suffix] = TRANSFORMS[t];
    let start = DICT_OFFSETS[length] + index * length;
    let n = length;
    if (type >= 1 && type <= 9) n = Math.max(0, n - type);
    else if (type >= 12 && type <= 20) {
      const skip = Math.min(n, type - 11);
      start += skip;
      n -= skip;
    }
    this.ensure(prefix.length + n + suffix.length);
    for (let i = 0; i < prefix.length; i++) this.out[this.len++] = prefix.charCodeAt(i);
    const at = this.len;
    for (let i = 0; i < n; i++) this.out[this.len++] = this.dictionary[start + i];
    if (type === 10) upper(this.out, at, n);
    else if (type === 11) for (let i = 0; i < n; ) i += upper(this.out, at + i, n - i);
    for (let i = 0; i < suffix.length; i++) this.out[this.len++] = suffix.charCodeAt(i);
  }

  // -- the stream --------------------------------------------------------------------------------

  run(): Uint8Array {
    // The window: how far back a copy may reach.
    let wbits = 16;
    if (this.bits(1)) {
      const n = this.bits(3);
      if (n) wbits = 17 + n;
      else {
        const m = this.bits(3);
        if (m === 1) throw new Error("Brotli large windows are not supported");
        wbits = m ? 8 + m : 17;
      }
    }
    const windowSize = (1 << wbits) - 16;
    const distRing = [16, 15, 11, 4];
    let ringAt = 3;
    for (;;) {
      const last = this.bits(1);
      if (last && this.bits(1)) break;
      const nibbles = this.bits(2);
      if (nibbles === 3) {
        // Metadata: skipped.
        if (this.bits(1)) throw new Error("Invalid Brotli metadata block");
        const skipBytes = this.bits(2);
        const skip = skipBytes ? this.bits(8 * skipBytes) + 1 : 0;
        this.align();
        this.pos += skip;
        if (last) break;
        continue;
      }
      const mlen = this.bits((nibbles + 4) * 4) + 1;
      if (!last && this.bits(1)) {
        this.align();
        this.ensure(mlen);
        if (this.pos + mlen > this.input.length) throw new Error("Brotli data is cut short");
        this.out.set(this.input.subarray(this.pos, this.pos + mlen), this.len);
        this.len += mlen;
        this.pos += mlen;
        continue;
      }
      // Block types and lengths for literals, insert-and-copy commands and distances.
      const blocks = [0, 1, 2].map(() => {
        const n = this.varLen8() + 1;
        let typeCode: Code | null = null;
        let lenCode: Code | null = null;
        let count = 1 << 24;
        if (n >= 2) {
          typeCode = this.readCode(n + 2);
          lenCode = this.readCode(26);
          const s = this.decode(lenCode);
          count = BLOCK_BASE[s] + this.bits(BLOCK_EXTRA[s]);
        }
        return { n, type: 0, prev: 1, count, typeCode, lenCode };
      });
      const [bl, bi, bd] = blocks;
      const switchBlock = (b: (typeof blocks)[number]) => {
        const s = this.decode(b.typeCode!);
        const t = s === 0 ? b.prev : s === 1 ? (b.type + 1) % b.n : s - 2;
        b.prev = b.type;
        b.type = t;
        const l = this.decode(b.lenCode!);
        b.count = BLOCK_BASE[l] + this.bits(BLOCK_EXTRA[l]);
      };
      const postfix = this.bits(2);
      const direct = this.bits(4) << postfix;
      const modes: number[] = [];
      for (let i = 0; i < bl.n; i++) modes.push(this.bits(2));
      const treesL = this.varLen8() + 1;
      const mapL = treesL >= 2 ? this.readContextMap(treesL, 64 * bl.n) : new Uint8Array(64 * bl.n);
      const treesD = this.varLen8() + 1;
      const mapD = treesD >= 2 ? this.readContextMap(treesD, 4 * bd.n) : new Uint8Array(4 * bd.n);
      const litCodes = Array.from({ length: treesL }, () => this.readCode(256));
      const cmdCodes = Array.from({ length: bi.n }, () => this.readCode(704));
      const distCodes = Array.from({ length: treesD }, () => this.readCode(16 + direct + (48 << postfix)));

      const end = this.len + mlen;
      this.ensure(mlen);
      while (this.len < end) {
        if (bi.count === 0) switchBlock(bi);
        bi.count--;
        const cmd = this.decode(cmdCodes[bi.type]);
        const group = cmd >> 6;
        const ic = INSERT_GROUP[group] + ((cmd >> 3) & 7);
        const cc = COPY_GROUP[group] + (cmd & 7);
        const insertLen = INSERT_BASE[ic] + this.bits(INSERT_EXTRA[ic]);
        const copyLen = COPY_BASE[cc] + this.bits(COPY_EXTRA[cc]);
        if (this.len + insertLen > end) throw new Error("Brotli literals run past the block");
        for (let i = 0; i < insertLen; i++) {
          if (bl.count === 0) switchBlock(bl);
          bl.count--;
          const p1 = this.len > 0 ? this.out[this.len - 1] : 0;
          const p2 = this.len > 1 ? this.out[this.len - 2] : 0;
          const m = modes[bl.type];
          const ctx = LOOKUP[LOOKUP_OFFSETS[m * 2] + p1] | LOOKUP[LOOKUP_OFFSETS[m * 2 + 1] + p2];
          this.out[this.len++] = this.decode(litCodes[mapL[bl.type * 64 + ctx]]);
        }
        if (this.len >= end) break;
        let distance: number;
        let push = true;
        if (cmd < 128) {
          distance = distRing[ringAt];
          push = false;
        } else {
          if (bd.count === 0) switchBlock(bd);
          bd.count--;
          const dctx = copyLen > 4 ? 3 : copyLen - 2;
          const sym = this.decode(distCodes[mapD[bd.type * 4 + dctx]]);
          if (sym < 16) {
            const last = distRing[ringAt];
            const second = distRing[(ringAt + 3) & 3];
            const third = distRing[(ringAt + 2) & 3];
            const fourth = distRing[(ringAt + 1) & 3];
            const table = [last, second, third, fourth, last - 1, last + 1, last - 2, last + 2, last - 3, last + 3, second - 1, second + 1, second - 2, second + 2, second - 3, second + 3];
            distance = table[sym];
            push = sym !== 0;
          } else if (sym < 16 + direct) distance = sym - 15;
          else {
            const d = sym - 16 - direct;
            const h = d >> postfix;
            const low = d & ((1 << postfix) - 1);
            const nb = 1 + (h >> 1);
            const offset = ((2 + (h & 1)) << nb) - 4;
            distance = ((offset + this.bits(nb)) << postfix) + low + direct + 1;
          }
          if (distance <= 0) throw new Error("Invalid Brotli distance");
        }
        const maxDistance = Math.min(windowSize, this.len);
        if (distance <= maxDistance) {
          if (this.len + copyLen > end) throw new Error("Brotli copy runs past the block");
          if (push) {
            ringAt = (ringAt + 1) & 3;
            distRing[ringAt] = distance;
          }
          this.ensure(copyLen);
          for (let i = 0; i < copyLen; i++, this.len++) this.out[this.len] = this.out[this.len - distance];
        } else {
          // Beyond the window: a word of the static dictionary.
          this.dictionaryWord(copyLen, distance - maxDistance - 1);
          if (this.len > end) throw new Error("Brotli dictionary word runs past the block");
        }
      }
      if (this.len !== end) throw new Error("Brotli block has the wrong length");
      if (last) break;
    }
    return this.out.subarray(0, this.len);
  }
}

/** Upper-case the first character of UTF-8 text at `at`; returns the bytes it took. */
function upper(p: Uint8Array, at: number, n: number): number {
  if (n <= 0) return 1;
  if (p[at] < 0xc0) {
    if (p[at] >= 0x61 && p[at] <= 0x7a) p[at] ^= 32;
    return 1;
  }
  if (p[at] < 0xe0) {
    if (n > 1) p[at + 1] ^= 32;
    return 2;
  }
  if (n > 2) p[at + 2] ^= 5;
  return 3;
}

let dictionary: Promise<Uint8Array> | undefined;

/** The static dictionary, read when the first stream needs it. */
function loadDictionary(): Promise<Uint8Array> {
  dictionary ??= inflateRaw(Uint8Array.from(atob(DICTIONARY), (c) => c.charCodeAt(0)));
  return dictionary;
}

/** Decompress a Brotli stream. `sizeHint` is the size of the result when it is known. */
export async function brotliDecompress(input: Uint8Array, sizeHint = 0): Promise<Uint8Array> {
  const dict = await loadDictionary();
  return new Decoder(input, dict, sizeHint || input.length * 4).run();
}
