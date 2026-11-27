// East Asian typography as CSS and pieces: the paragraph switches, two lines in one, text upright in a vertical line, the
// kinsoku lists a document sets itself, the document grid's character pitch and the kana reading. No browser needed.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { RSeg, RunFmt } from "@docxtor/core";
import { charPitch, gridCounts, gridFor } from "../src/editor/grid.ts";
import { eaParaCss, eaSegPiece, guessLang, hiraganaOf, kinsokuPieces, lineBreakFor } from "../src/editor/east-asian.ts";
import { fontPatch, type FontState } from "../src/ui/font-patch.ts";

const seg = (x: string, f: RunFmt = {}, o = 0): RSeg => ({ o, k: "t", x, f: { eaText: true, ...f } });
const piece = (s: RSeg) => ({ seg: s, css: [] as string[], cls: [] as string[] });

test("a paragraph that follows Word's defaults needs no CSS of its own", () => {
  assert.deepEqual(eaParaCss({}), []);
});

test("each Asian typography switch maps to the CSS that can express it", () => {
  const css = eaParaCss({ kinsoku: false, wordWrap: false, topLinePunct: true, autoSpaceDe: false, autoSpaceDn: false });
  assert.ok(css.includes("line-break:loose") && css.includes("word-break:break-all") && css.includes("text-spacing-trim:trim-start") && css.includes("text-autospace:no-autospace"));
  assert.ok(eaParaCss({ autoSpaceDe: false }).includes("text-autospace:ideograph-numeric"));
  assert.ok(eaParaCss({ autoSpaceDn: false }).includes("text-autospace:ideograph-alpha"));
  assert.ok(eaParaCss({ textDirection: "tbRl" }).includes("writing-mode:vertical-rl"));
});

test("the strict level is a variable of the flow, and the standard one is nothing", () => {
  assert.equal(lineBreakFor({ strict: true }), "strict");
  assert.equal(lineBreakFor({}), "");
  assert.equal(lineBreakFor(undefined), "");
});

test("two lines in one is a half-size box as wide as half the characters", () => {
  const p = eaSegPiece(seg("二行文字", { eaLayout: { combine: true, combineBrackets: "square" } }), 12)!;
  assert.ok(p.cls.includes("ea-comb") && p.cls.includes("ea-comb-br"));
  assert.ok(p.css.includes("font-size:6.00pt") && p.css.includes("width:calc(2em + 2px)"));
  assert.deepEqual([p.attrs["data-bl"], p.attrs["data-brt"]], ["[", "]"]);
  // Five characters make two lines of three.
  assert.ok(eaSegPiece(seg("あいうえお", { eaLayout: { combine: true } }), 12)!.css.some((c) => c === "width:calc(3em + 3px)"));
});

test("horizontal in vertical: fitted into the line, or across it", () => {
  assert.ok(eaSegPiece(seg("12", { eaLayout: { vert: true, vertCompress: true } }), 11)!.css.includes("text-combine-upright:all"));
  assert.ok(eaSegPiece(seg("ab", { eaLayout: { vert: true } }), 11)!.cls.includes("ea-vert"));
});

test("East Asian text is marked for the grid and gets a language; Latin text is left alone", () => {
  const ea = eaSegPiece(seg("かな", { langEa: "ja-JP" }), 11)!;
  assert.ok(ea.cls.includes("ea-t"));
  assert.equal(ea.attrs.lang, "ja-JP");
  assert.equal(eaSegPiece(seg("abc", { eaText: false }), 11), null);
  // A run that opts out of the grid is not marked.
  assert.ok(!eaSegPiece(seg("かな", { snapToGrid: false }), 11)?.cls.includes("ea-t"));
  assert.equal(guessLang("ひらがな"), "ja");
  assert.equal(guessLang("한국어"), "ko");
  assert.equal(guessLang("漢字"), undefined);
});

test("characters that may not start a line are held to the one before them, in a piece that does not break", () => {
  const settings = { custom: [{ lang: "ja-JP", before: "、。", after: "「" }] };
  const out = kinsokuPieces([piece(seg("あい、うえ「おか", {}, 10))], {}, settings);
  assert.deepEqual(
    out.map((p) => [p.seg.x, p.seg.o, p.css.includes("white-space:nowrap")]),
    [
      ["あ", 10, false],
      ["い、", 11, true],
      ["うえ", 13, false],
      ["「お", 15, true],
      ["か", 17, false],
    ],
  );
  // Offsets stay a partition of the text.
  assert.equal(out.map((p) => p.seg.x).join(""), "あい、うえ「おか");
});

test("a paragraph that turned the rules off, no lists, or Latin text change nothing", () => {
  const p = piece(seg("あい、う"));
  assert.deepEqual(kinsokuPieces([p], { kinsoku: false }, { custom: [{ lang: "ja-JP", before: "、", after: "" }] }), [p]);
  assert.deepEqual(kinsokuPieces([p], {}, undefined), [p]);
  assert.deepEqual(kinsokuPieces([p], {}, { custom: [] }), [p]);
  const latin = piece(seg("a,b", { eaText: false }));
  assert.deepEqual(kinsokuPieces([latin], {}, { custom: [{ lang: "ja-JP", before: ",", after: "" }] }), [latin]);
});

test("the grid's character width is the base size plus what the grid adds", () => {
  assert.equal(charPitch({ type: "lines", linePitch: 360 }, 10.5), undefined);
  assert.equal(charPitch({ type: "linesAndChars", charSpace: 4096 }, 10), (10 + 1) * (96 / 72));
  assert.equal(charPitch({ type: "snapToChars", charSpace: -2048 }, 12), (12 - 0.5) * (96 / 72));
});

test("the Page setup counts and the grid the file stores go both ways", () => {
  // A text area of 468 by 648 points with 36 characters per line and 30 lines at a 10.5 point base.
  const grid = gridFor("linesAndChars", 10.5, 468, 648, 36, 30);
  assert.equal(grid.linePitch, 432);
  assert.equal(grid.charSpace, Math.round((13 - 10.5) * 4096));
  assert.deepEqual(gridCounts(grid, 10.5, 468, 648), { chars: 36, lines: 30 });
  assert.deepEqual(gridFor("default", 10.5, 468, 648, 36, 30), { type: "default" });
  assert.equal(gridFor("lines", 10.5, 468, 648, 36, 30).charSpace, undefined);
});

test("kana read as hiragana, and anything else has no reading", () => {
  assert.equal(hiraganaOf("カンジ"), "かんじ");
  assert.equal(hiraganaOf("ひらがな ー"), "ひらがな ー");
  assert.equal(hiraganaOf("漢字"), null);
  assert.equal(hiraganaOf("abc"), null);
  assert.equal(hiraganaOf(""), null);
});

test("the Font box writes an Asian text font only when it changed", () => {
  const base: FontState = {
    font: "Arial", fontEa: "", style: "regular", size: 11, color: {}, underline: "none", underlineColor: {}, shading: {}, em: "none", strike: false, dstrike: false,
    vert: "baseline", smallCaps: false, caps: false, vanish: false, outline: false, shadow: false, emboss: false, imprint: false, scale: 100,
    spacingKind: "normal", spacingBy: 0, positionKind: "normal", positionBy: 0, kern: false, kernFrom: 12, ligatures: "default", numSpacing: "default",
    numForm: "default", styleSet: 0, cntxtAlts: false,
  };
  assert.equal(fontPatch(base, { ...base }).fontEa, undefined);
  assert.equal(fontPatch(base, { ...base, fontEa: "MS Mincho" }).fontEa, "MS Mincho");
  assert.equal(fontPatch({ ...base, fontEa: "MS Mincho" }, { ...base, fontEa: "" }).fontEa, "");
});
