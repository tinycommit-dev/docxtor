// Character formatting as CSS (underline styles, effects, automatic colour, the pieces text is cut into), the
// Shift+F3 cycle and the colour maths of the palette. These need no browser: text is not measured here.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { RSeg, RunFmt } from "@docxtor/core";
import { decorationCss, inkOf, isDark, prepareSegs, runExtraCss, UNDERLINE_CHOICES, UNDERLINE_CSS } from "../src/editor/charfmt.ts";
import { nextCase } from "../src/editor/charcmds.ts";
import { fontPatch } from "../src/ui/font-patch.ts";
import { hslToRgb, rgbToHsl, tinted, toHex } from "../src/ui/swatches.ts";

const seg = (x: string, f: RunFmt = {}, o = 0): RSeg => ({ o, k: "t", x, f });
const stack = () => undefined;

test("every underline style Word has maps to a CSS line", () => {
  const names = UNDERLINE_CHOICES.map(([v]) => v).filter((v) => v !== "none");
  assert.equal(names.length, 17);
  for (const n of names) assert.ok(UNDERLINE_CSS[n], n);
  assert.deepEqual(decorationCss({ underline: "double", underlineColor: "FF0000" }), ["text-decoration-line:underline", "text-decoration-style:double", "text-decoration-color:#FF0000"]);
  assert.ok(decorationCss({ underline: "wavyHeavy" }).some((d) => d.startsWith("text-decoration-thickness")));
  assert.deepEqual(decorationCss({ underline: "none" }), []);
  // A double strike-through is double whatever the underline is; the underline colour then stays out of it.
  const both = decorationCss({ underline: "single", dstrike: true, underlineColor: "00FF00" });
  assert.ok(both.includes("text-decoration-style:double") && !both.some((d) => d.includes("color")));
});

test("raised text moves without changing the line, and kerning follows the size", () => {
  assert.ok(runExtraCss({ position: 6 }).includes("position:relative;top:-3pt"));
  assert.ok(runExtraCss({ position: -4 }).includes("position:relative;top:2pt"));
  assert.ok(runExtraCss({ kern: 28, size: 14 }).includes("font-kerning:normal"));
  assert.ok(runExtraCss({ kern: 28, size: 12 }).includes("font-kerning:none"));
  assert.ok(runExtraCss({ kern: 0 }).includes("font-kerning:none"));
  assert.ok(!runExtraCss({}).some((d) => d.startsWith("font-kerning")));
});

test("effects: outline is hollow, shadows stack, a gradient fills the letters", () => {
  const outline = runExtraCss({ outline: true, color: "FF0000" });
  assert.ok(outline.includes("color:transparent") && outline.some((d) => d.startsWith("-webkit-text-stroke:0.7px #FF0000")));
  const shadows = runExtraCss({ shadow: true, emboss: true }).find((d) => d.startsWith("text-shadow:"))!;
  assert.equal(shadows.split("),").length, 3);
  const grad = runExtraCss({ fx: { fill: { stops: [{ pos: 0, color: { hex: "FF0000" } }, { pos: 1, color: { hex: "0000FF" } }], angle: 0 } } });
  assert.ok(grad.some((d) => d.startsWith("background-image:linear-gradient(90deg,#FF0000 0%,#0000FF 100%)")) && grad.includes("-webkit-text-fill-color:transparent"));
  const ot = runExtraCss({ fx: { ligatures: "standardContextual", numForm: "oldStyle", numSpacing: "tabular", styleSets: [3], cntxtAlts: true } });
  assert.ok(ot.includes("font-variant-ligatures:common-ligatures contextual") && ot.includes("font-variant-numeric:oldstyle-nums tabular-nums"));
  assert.ok(ot.includes('font-feature-settings:"ss03" 1,"calt" 1'));
  assert.ok(runExtraCss({ fx: { ligatures: "none" } }).includes("font-variant-ligatures:none"));
  assert.deepEqual(runExtraCss({ em: "underDot" }).slice(-2), ["text-emphasis:filled dot", "text-emphasis-position:under right"]);
});

test("automatic colour is black, or white on a dark background", () => {
  assert.equal(inkOf({}), "#000000");
  assert.equal(inkOf({ highlight: "darkBlue" }), "#ffffff");
  assert.equal(inkOf({ highlight: "yellow" }), "#000000");
  assert.equal(inkOf({ color: "FF0000", highlight: "black" }), "#FF0000");
  assert.equal(inkOf({}, "#1f2937"), "#ffffff");
  assert.ok(isDark("#000080") && isDark("1F2937") && !isDark("#FFFF00") && !isDark("#FF0000") && !isDark(undefined));
});

test("words only underline cuts the text at its spaces, with each piece's offset", () => {
  const pieces = prepareSegs([seg("two  words", { underline: "words" }, 5)], {}, stack);
  assert.deepEqual(
    pieces.map((p) => [p.seg.x, p.seg.o, p.seg.f.underline]),
    [
      ["two", 5, "single"],
      ["  ", 8, "none"],
      ["words", 10, "single"],
    ],
  );
});

test("scaled text is a box per word, and neighbouring runs with one border share a box", () => {
  const scaled = prepareSegs([seg("a b", { scale: 150 })], {}, stack);
  assert.deepEqual(scaled.map((p) => p.seg.x), ["a", " ", "b"]);
  assert.ok(scaled[0].css.includes("display:inline-block") && scaled[0].css.includes("transform:scaleX(1.5)"));
  assert.ok(scaled[1].css.some((c) => c.startsWith("letter-spacing")) && !scaled[1].css.includes("display:inline-block"));
  const border = { val: "single", sz: 8, color: "0000FF", space: 1 };
  const boxed = prepareSegs([seg("one ", { border }), seg("two ", { border, bold: true }), seg("three", { border }), seg(" after")], {}, stack);
  const sides = boxed.map((p) => ["border-left", "border-right"].map((s) => p.css.some((c) => c.startsWith(s))));
  assert.deepEqual(sides, [[true, false], [false, false], [false, true], [false, false]]);
});

test("fit text spaces the letters to fill the width for the whole group", () => {
  const f = { fitText: 2160, fitTextId: 7 };
  const out = prepareSegs([seg("ab", f), seg("cd", f), seg("zz", {})], {}, stack);
  // Four characters share 144px; the others are left alone.
  assert.equal(out[0].seg.f.spacing, out[1].seg.f.spacing);
  assert.notEqual(out[0].seg.f.spacing, undefined);
  assert.equal(out[2].seg.f.spacing, undefined);
});

test("Shift+F3 goes lowercase, each word capitalised, UPPERCASE, lowercase", () => {
  assert.equal(nextCase("hello world"), "title");
  assert.equal(nextCase("Hello World"), "upper");
  assert.equal(nextCase("HELLO WORLD"), "lower");
  assert.equal(nextCase("Hello world"), "upper");
  assert.equal(nextCase("123"), "upper");
});

test("the palette's colour maths agrees with Word's tint and shade", () => {
  // Accent 1 of the Office theme at tint 33 (lighter 80%) and shade BF (darker 25%).
  assert.equal(tinted("5B9BD5", "33"), "DEEBF7");
  assert.equal(tinted("5B9BD5", undefined, "BF"), "4474A0");
  assert.equal(tinted("000000", "80"), "7F7F7F");
  assert.equal(tinted("FFFFFF", undefined, "80"), "808080");
  for (const hex of ["12AB34", "FF4F00", "336699", "808080"]) {
    const [h, s, l] = rgbToHsl(...(hex.match(/../g)!.map((v) => parseInt(v, 16)) as [number, number, number]));
    assert.equal(toHex(...hslToRgb(h, s, l)), hex);
  }
});

test("the Font dialog writes only what changed", () => {
  const base = {
    font: "Georgia", style: "regular", size: 11, color: {}, underline: "none", underlineColor: {}, shading: {}, em: "none", strike: false, dstrike: false, vert: "baseline", smallCaps: false, caps: false, vanish: false,
    outline: false, shadow: false, emboss: false, imprint: false, scale: 100, spacingKind: "normal", spacingBy: 0, positionKind: "normal", positionBy: 0, kern: false, kernFrom: 12, ligatures: "default",
    numSpacing: "default", numForm: "default", styleSet: 0, cntxtAlts: false,
  } as const;
  assert.deepEqual(fontPatch(base, { ...base }), {});
  const patch = fontPatch(base, { ...base, style: "boldItalic", underline: "wave", dstrike: true, spacingKind: "condensed", spacingBy: 0.5, positionKind: "raised", positionBy: 3, kern: true, scale: 90, styleSet: 4, color: { hex: "5B9BD5", theme: { name: "accent1" } } });
  assert.deepEqual(patch, {
    bold: true,
    italic: true,
    color: "5B9BD5",
    themeColor: "accent1",
    underline: "wave",
    dstrike: true,
    scale: 90,
    spacing: -10,
    position: 6,
    kern: 24,
    styleSets: [4],
  });
});
