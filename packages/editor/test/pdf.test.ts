// The PDF writer's building blocks: objects and streams, TrueType reading and subsetting. No DOM.
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { brotliCompressSync, constants } from "node:zlib";
import { brotliDecompress } from "../src/pdf/brotli.ts";
import { TrueTypeFont } from "../src/pdf/truetype.ts";
import { unwrapWoff } from "../src/pdf/woff.ts";
import { decodeWoff2 } from "../src/pdf/woff2.ts";
import { concat, deflate, hexString, inflate, num, PdfWriter, pdfName, pdfString, ref } from "../src/pdf/writer.ts";

const dec = new TextDecoder("latin1");
const font = () => new TrueTypeFont(new Uint8Array(readFileSync(new URL("./fixtures/test-font.ttf", import.meta.url))));

test("numbers have no exponent and no trailing zeros", () => {
  assert.equal(num(1), "1");
  assert.equal(num(0.5), "0.5");
  assert.equal(num(-12.34567), "-12.346");
  assert.equal(num(0.0004), "0");
  assert.equal(num(123456789012), "123456789012");
  assert.equal(num(Number.NaN), "0");
  assert.equal(num(-0.0001), "0");
});

test("names and strings are escaped", () => {
  assert.equal(pdfName("Font1"), "/Font1");
  assert.equal(pdfName("A B"), "/A#20B");
  assert.equal(pdfName("a/b"), "/a#2fb");
  assert.equal(pdfString("a (b) \\"), "(a \\(b\\) \\\\)");
  assert.equal(pdfString("é"), "<FEFF00E9>");
  assert.equal(hexString(new Uint8Array([0, 255, 16])), "<00ff10>");
  assert.equal(ref(7), "7 0 R");
});

test("flate round trip", async () => {
  const data = new TextEncoder().encode("hello hello hello hello hello hello hello hello".repeat(20));
  const z = await deflate(data);
  assert.ok(z.length < data.length);
  assert.deepEqual(await inflate(z), data);
});

test("a file has a header, a cross-reference table with the right offsets and a trailer", async () => {
  const w = new PdfWriter();
  const pages = w.alloc();
  const page = w.alloc();
  const catalog = w.add(`<< /Type /Catalog /Pages ${ref(pages)} >>`);
  w.put(pages, `<< /Type /Pages /Kids [${ref(page)}] /Count 1 >>`);
  const content = await w.addStream("", new TextEncoder().encode("0 0 100 100 re f\n".repeat(10)));
  w.put(page, `<< /Type /Page /Parent ${ref(pages)} /MediaBox [0 0 100 100] /Contents ${ref(content)} >>`);
  const bytes = w.build(catalog);
  const text = dec.decode(bytes);
  assert.ok(text.startsWith("%PDF-1.7\n"));
  assert.ok(text.trimEnd().endsWith("%%EOF"));
  assert.match(text, /\/Size 5 \/Root 3 0 R/);
  // Every entry of the table points at "<n> 0 obj".
  const start = Number(/startxref\n(\d+)/.exec(text)![1]);
  assert.ok(text.slice(start).startsWith("xref\n0 5\n"));
  const entries = [...text.slice(start).matchAll(/^(\d{10}) 00000 n $/gm)].map((m) => Number(m[1]));
  assert.equal(entries.length, 4);
  entries.forEach((off, i) => assert.ok(text.slice(off).startsWith(`${i + 1} 0 obj`), `object ${i + 1} at ${off}`));
  // The stream is compressed and says so.
  assert.match(text, /\/Filter \/FlateDecode \/Length \d+ >>\nstream/);
});

test("an object that was reserved and never written is an error", () => {
  const w = new PdfWriter();
  w.alloc();
  assert.throws(() => w.build(1), /never written/);
});

test("the font's metrics and names are read", () => {
  const f = font();
  assert.equal(f.unitsPerEm, 1000);
  assert.equal(f.numGlyphs, 10);
  assert.equal(f.ascender, 800);
  assert.equal(f.descender, -200);
  assert.equal(f.capHeight, 700);
  assert.equal(f.underlinePosition, -100);
  assert.equal(f.names[1], "Docxtor Test");
  assert.equal(f.names[6], "DocxtorTest-Regular");
  assert.equal(f.fsType, 0);
  assert.equal(f.isCff, false);
});

test("characters map to glyphs through the cmap, and widths come from hmtx", () => {
  const f = font();
  const a = f.glyphId(65);
  const b = f.glyphId(66);
  assert.ok(a > 0 && b > 0 && a !== b);
  assert.equal(f.advance(a), 700);
  assert.equal(f.advance(b), 600);
  assert.equal(f.advance(f.glyphId(32)), 300);
  // A ligature, a CJK character and one outside the Basic Multilingual Plane (cmap format 12).
  assert.equal(f.advance(f.glyphId(0xfb01)), 850);
  assert.equal(f.advance(f.glyphId(0x4e2d)), 1000);
  assert.equal(f.advance(f.glyphId(0x1f600)), 1000);
  // Not in the font.
  assert.equal(f.glyphId(0x41f), 0);
  assert.equal(f.glyphId(0x10ffff), 0);
});

test("a subset keeps the glyphs asked for, and the parts of composites", () => {
  const f = font();
  const aacute = f.glyphId(0xc1);
  const small = f.subset([aacute]);
  const g = new TrueTypeFont(small);
  assert.equal(g.numGlyphs, f.numGlyphs);
  assert.equal(g.unitsPerEm, 1000);
  assert.equal(g.advance(aacute), 700);
  // Glyph data: Aacute and its two parts (A, acutecomb) are kept; B and C are empty.
  const glyfSize = (font: TrueTypeFont, gid: number) => {
    const loca = font.tables.get("loca")!.offset;
    const v = font.view;
    return v.getUint32(loca + (gid + 1) * 4) - v.getUint32(loca + gid * 4);
  };
  const sub = (cp: number) => glyfSize(g, f.glyphId(cp));
  assert.ok(sub(0xc1) > 0 && sub(65) > 0 && sub(0x301) > 0);
  assert.equal(sub(66), 0);
  assert.equal(sub(67), 0);
  assert.ok(small.length < f.bytes.length * 2);
  // Only the tables a viewer needs are kept.
  assert.deepEqual([...g.tables.keys()].sort(), ["glyf", "head", "hhea", "hmtx", "loca", "maxp"]);
});

test("a font file adds up to the checksum its head table says", () => {
  const small = font().subset([1, 2, 3]);
  const v = new DataView(small.buffer, small.byteOffset, small.byteLength);
  let sum = 0;
  for (let i = 0; i + 3 < small.length; i += 4) sum = (sum + v.getUint32(i)) >>> 0;
  assert.equal(sum, 0xb1b0afba);
});

test("data that isn't a font is refused", () => {
  assert.throws(() => new TrueTypeFont(new Uint8Array(concat([new TextEncoder().encode("not a font at all, sorry")]))), /Not a TrueType font/);
});

// -- Brotli ------------------------------------------------------------------------------------------

const text = new Uint8Array(readFileSync(new URL("../../../LICENSE-APACHE", import.meta.url)));
const noise = (() => {
  // Deterministic pseudo-random bytes.
  const b = new Uint8Array(30000);
  let x = 12345;
  for (let i = 0; i < b.length; i++) {
    x = (Math.imul(x, 1103515245) + 12345) >>> 0;
    b[i] = x >>> 24;
  }
  return b;
})();
const compress = (data: Uint8Array, quality: number, lgwin = 22, mode: number = constants.BROTLI_MODE_GENERIC) =>
  new Uint8Array(brotliCompressSync(data, { params: { [constants.BROTLI_PARAM_QUALITY]: quality, [constants.BROTLI_PARAM_LGWIN]: lgwin, [constants.BROTLI_PARAM_MODE]: mode } }));

test("brotli: text, at every quality, comes back as it went in", async () => {
  for (let q = 0; q <= 11; q++) assert.deepEqual(await brotliDecompress(compress(text, q)), text, `quality ${q}`);
});

test("brotli: noise, repeats, an empty input, and windows of other sizes", async () => {
  assert.deepEqual(await brotliDecompress(compress(noise, 5)), noise);
  const repeats = new TextEncoder().encode("abcabcabcabc".repeat(2000));
  assert.deepEqual(await brotliDecompress(compress(repeats, 9)), repeats);
  assert.deepEqual(await brotliDecompress(compress(new Uint8Array(0), 5)), new Uint8Array(0));
  for (const lgwin of [10, 12, 16, 18, 24]) assert.deepEqual(await brotliDecompress(compress(text, 6, lgwin)), text, `window ${lgwin}`);
});

test("brotli: words from the static dictionary and font data", async () => {
  // Text full of dictionary words, with capitals and the endings the transforms add.
  const words = new TextEncoder().encode("The information about government and development of international business, Education! Environment, Management. ".repeat(40));
  assert.deepEqual(await brotliDecompress(compress(words, 11, 22, constants.BROTLI_MODE_TEXT)), words);
  const ttf = new Uint8Array(readFileSync(new URL("./fixtures/test-font.ttf", import.meta.url)));
  assert.deepEqual(await brotliDecompress(compress(ttf, 11, 22, constants.BROTLI_MODE_FONT)), ttf);
});

test("brotli: broken data is an error, not a hang", async () => {
  const z = compress(text, 6);
  await assert.rejects(brotliDecompress(z.subarray(0, z.length >> 1)));
  // A block that says it holds 65536 bytes and has none.
  await assert.rejects(brotliDecompress(new Uint8Array([0xf0, 0xff, 0x1f])));
});

// -- WOFF and WOFF2 -----------------------------------------------------------------------------------

const fixture = (name: string) => new Uint8Array(readFileSync(new URL(`./fixtures/${name}`, import.meta.url)));

test("woff: unwrapping gives back the tables of the original font", async () => {
  const original = new TrueTypeFont(fixture("test-font.ttf"));
  const unwrapped = new TrueTypeFont(await unwrapWoff(fixture("test-font.woff")));
  for (const tag of ["glyf", "loca", "hmtx", "cmap", "name"]) {
    const a = original.tables.get(tag)!;
    const b = unwrapped.tables.get(tag)!;
    assert.deepEqual(unwrapped.bytes.subarray(b.offset, b.offset + b.length), original.bytes.subarray(a.offset, a.offset + a.length), tag);
  }
});

test("woff2: glyphs (simple and composite), widths and characters survive", async () => {
  const original = new TrueTypeFont(fixture("test-font.ttf"));
  const font = new TrueTypeFont(await decodeWoff2(fixture("test-font.woff2")));
  assert.equal(font.numGlyphs, original.numGlyphs);
  for (const cp of [65, 66, 67, 0x301, 0xc1, 0xfb01, 0x4e2d, 0x1f600]) {
    assert.equal(font.glyphId(cp), original.glyphId(cp), `U+${cp.toString(16)}`);
    assert.equal(font.advance(font.glyphId(cp)), original.advance(original.glyphId(cp)));
  }
  assert.equal(font.names[6], "DocxtorTest-Regular");
  // The composite (Aacute) pulls in its parts when subset, so the glyph data is whole.
  const sub = new TrueTypeFont(font.subset([font.glyphId(0xc1)]));
  assert.equal(sub.numGlyphs, original.numGlyphs);
  const glyphLength = (f: TrueTypeFont, gid: number) => f.view.getUint32(f.tables.get("loca")!.offset + (gid + 1) * 4) - f.view.getUint32(f.tables.get("loca")!.offset + gid * 4);
  assert.ok(glyphLength(sub, font.glyphId(65)) > 0 && glyphLength(sub, font.glyphId(0x301)) > 0);
});

test("woff2: data that isn't WOFF2 is an error", async () => {
  await assert.rejects(decodeWoff2(fixture("test-font.ttf")));
});
