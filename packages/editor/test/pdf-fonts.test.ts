// Choosing and writing the PDF's fonts, reading font declarations in CSS, and the small text helpers. No DOM.
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { bestFace, inRanges, parseFontFaces, parseUnicodeRange, parseWeight, splitFamilies } from "../src/pdf/fontcss.ts";
import { FontSet, kindOf, toUnicodeCMap, winAnsi, type Face } from "../src/pdf/fontset.ts";
import { readJpeg } from "../src/pdf/jpeg.ts";
import { glyphHex, isInvisible, isSpace, literalBytes, parseColor, shownChars } from "../src/pdf/textutil.ts";
import { TrueTypeFont } from "../src/pdf/truetype.ts";
import { PdfWriter } from "../src/pdf/writer.ts";

const dec = new TextDecoder("latin1");
const fixture = (name: string) => new Uint8Array(readFileSync(new URL(`./fixtures/${name}`, import.meta.url)));

test("font-family lists are split at commas outside quotes", () => {
  assert.deepEqual(splitFamilies(`Calibri, Carlito, "Segoe UI", 'A, B', sans-serif`), ["Calibri", "Carlito", "Segoe UI", "A, B", "sans-serif"]);
});

test("unicode ranges, weights and @font-face rules (as Google Fonts writes them)", () => {
  assert.deepEqual(parseUnicodeRange("U+0000-00FF, U+0131, U+4??"), [
    [0, 255],
    [0x131, 0x131],
    [0x400, 0x4ff],
  ]);
  assert.ok(inRanges([[0, 255]], 65) && !inRanges([[0, 255]], 0x4e2d) && inRanges(null, 0x4e2d));
  assert.deepEqual(parseWeight("100 900"), [100, 900]);
  assert.deepEqual(parseWeight("bold"), [700, 700]);
  const css = `/* latin */
@font-face {
  font-family: 'Carlito';
  font-style: italic;
  font-weight: 400;
  src: url(https://fonts.gstatic.com/s/carlito/v4/a.woff2) format('woff2'), url("b.ttf") format("truetype");
  unicode-range: U+0000-00FF;
}`;
  const [face] = parseFontFaces(css, "https://example.org/css/x.css");
  assert.equal(face.family, "Carlito");
  assert.equal(face.style, "italic");
  assert.deepEqual(face.weight, [400, 400]);
  assert.deepEqual(face.ranges, [[0, 255]]);
  // Plain TrueType first (it needs no decoding), the address made absolute.
  assert.deepEqual(
    face.sources.map((x) => x.url),
    ["https://example.org/css/b.ttf", "https://fonts.gstatic.com/s/carlito/v4/a.woff2"],
  );
});

test("face matching follows CSS: style first, then the nearest weight in the direction asked", () => {
  const f = (weight: number, style: "normal" | "italic" = "normal", hi = weight) => ({ weight: [weight, hi] as [number, number], style, id: `${weight}${style[0]}` });
  const faces = [f(400), f(700), f(400, "italic"), f(700, "italic")];
  assert.equal(bestFace(faces, 400, false)?.id, "400n");
  assert.equal(bestFace(faces, 700, false)?.id, "700n");
  assert.equal(bestFace(faces, 600, false)?.id, "700n");
  assert.equal(bestFace(faces, 300, false)?.id, "400n");
  assert.equal(bestFace(faces, 500, false)?.id, "400n");
  assert.equal(bestFace(faces, 700, true)?.id, "700i");
  assert.equal(bestFace([f(400), f(700)], 400, true)?.id, "400n");
  assert.equal(bestFace([f(100, "normal", 900)], 650, false)?.id, "100n");
  assert.equal(bestFace([], 400, false), null);
});

const testFace = (family: string, weight: [number, number] = [400, 400], ranges: [number, number][] | null = null): Face => ({
  family,
  weight,
  style: "normal",
  ranges,
  load: async () => new TrueTypeFont(fixture("test-font.ttf")),
});

test("a font set picks the first family that has the character, then standard fonts, then a picture", async () => {
  const set = new FontSet([testFace("Docxtor Test"), testFace("Tinos", [400, 400], [[0x41, 0x41]])]);
  const key = { families: ["Missing One", "Docxtor Test", "serif"], weight: 400, italic: false };
  await set.prepare(key, [65, 66, 0x44]);
  const a = set.pick(key, 65);
  assert.equal(a?.kind, "ttf");
  assert.equal(a?.kind === "ttf" && a.font.family, "Docxtor Test");
  // D is in no font file: a standard font draws it.
  const d = set.pick(key, 0x44);
  assert.equal(d?.kind, "std");
  assert.equal(d?.kind === "std" && d.font.baseFont, "Times-Roman");
  // A character WinAnsi doesn't have and no font covers: a picture.
  assert.equal(set.pick(key, 0x4e2e), null);
  // The family asked for was not the one that drew the text.
  set.noteUse(key, a);
  assert.deepEqual([...set.substituted], ["Missing One"]);
});

test("bold and italic asked for, a face that is neither: drawn thickened and slanted", async () => {
  const set = new FontSet([testFace("Docxtor Test")]);
  const key = { families: ["Docxtor Test"], weight: 700, italic: true };
  await set.prepare(key, [65]);
  const p = set.pick(key, 65);
  assert.ok(p?.kind === "ttf" && p.synthBold && p.synthItalic);
});

test("kinds of fallback font", () => {
  assert.equal(kindOf(["Foo", "monospace"]), "mono");
  assert.equal(kindOf(["Foo", "sans-serif"]), "sans");
  assert.equal(kindOf(["Foo"]), "serif");
});

test("a written font has widths, a subset file and a ToUnicode map", async () => {
  const set = new FontSet([testFace("Docxtor Test")]);
  const key = { families: ["Docxtor Test"], weight: 400, italic: false };
  await set.prepare(key, [65, 66, 0x1f600]);
  for (const cp of [65, 66, 0x1f600]) {
    const p = set.pick(key, cp);
    if (p?.kind === "ttf") p.font.use(p.gid, p.cp);
  }
  const w = new PdfWriter();
  const entries = await set.write(w);
  assert.match(entries, /^\/F1 \d+ 0 R$/);
  const text = dec.decode(w.build(w.add("<< /Type /Catalog >>")));
  assert.match(text, /\/Subtype \/CIDFontType2/);
  assert.match(text, /\/BaseFont \/[A-Z]{6}\+DocxtorTest-Regular/);
  // Widths of A (700) and B (600) as one run of consecutive glyphs, then the emoji's glyph.
  assert.match(text, /\/W \[\d+ \[700 600\] \d+ \[1000\]\]/);
  assert.match(text, /\/FontFile2 \d+ 0 R/);
  assert.match(text, /\/ToUnicode \d+ 0 R/);
});

test("the ToUnicode map writes characters as UTF-16, with surrogates beyond the first plane", () => {
  const cmap = toUnicodeCMap(
    new Map([
      [5, 0x41],
      [9, 0x1f600],
    ]),
  );
  assert.match(cmap, /<0005> <0041>/);
  assert.match(cmap, /<0009> <D83DDE00>/);
  assert.match(cmap, /1 begincodespacerange\n<0000> <FFFF>/);
});

test("WinAnsi codes", () => {
  assert.equal(winAnsi(65), 65);
  assert.equal(winAnsi(0xe9), 0xe9);
  assert.equal(winAnsi(0x2019), 0x92);
  assert.equal(winAnsi(0x20ac), 0x80);
  assert.equal(winAnsi(0x4e2d), -1);
});

test("what counts as a space, and what is never drawn", () => {
  assert.ok(isSpace(0x20) && isSpace(0xa0) && isSpace(0x09) && isSpace(0x3000));
  assert.ok(!isSpace(65));
  assert.ok(isInvisible(0xad) && isInvisible(0x200b) && isInvisible(0xfeff) && !isInvisible(65));
});

test("text-transform and small capitals", () => {
  assert.deepEqual(shownChars("a", "uppercase", "normal", false), { cps: [65], small: false });
  assert.deepEqual(shownChars("ß", "uppercase", "normal", false).cps, [83, 83]);
  assert.deepEqual(shownChars("a", "capitalize", "normal", true), { cps: [65], small: false });
  assert.deepEqual(shownChars("a", "capitalize", "normal", false), { cps: [97], small: false });
  assert.deepEqual(shownChars("a", "none", "small-caps", false), { cps: [65], small: true });
  assert.deepEqual(shownChars("A", "none", "small-caps", false), { cps: [65], small: false });
  assert.deepEqual(shownChars("A", "none", "all-small-caps", false), { cps: [65], small: true });
});

test("colours as the browser writes them", () => {
  assert.deepEqual(parseColor("rgb(255, 0, 0)"), [1, 0, 0, 1]);
  assert.deepEqual(parseColor("rgba(0, 0, 255, 0.5)"), [0, 0, 1, 0.5]);
  assert.deepEqual(parseColor("rgb(255 255 255 / 25%)"), [1, 1, 1, 0.25]);
  assert.deepEqual(parseColor("color(srgb 0.5 0.25 1)"), [0.5, 0.25, 1, 1]);
  assert.deepEqual(parseColor("transparent"), [0, 0, 0, 0]);
  assert.equal(parseColor("oklch(0.5 0.1 20)"), null);
});

test("strings for the PDF's text operators", () => {
  assert.equal(literalBytes([72, 0x28, 0x29, 0x5c, 0x92]), "(H\\(\\)\\\\\\222)");
  assert.equal(glyphHex([1, 0xabc]), "<00010abc>");
});

test("a JPEG's size and colour model are read without decoding it", () => {
  // SOI, an APP0, then a baseline frame header: 8 bits, 30 high, 40 wide, 3 components.
  const bytes = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 0, 30, 0, 40, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
  assert.deepEqual(readJpeg(bytes), { width: 40, height: 30, components: 3, bits: 8, inverted: false, orientation: 1 });
  assert.equal(readJpeg(new Uint8Array([1, 2, 3, 4, 5])), null);
});
