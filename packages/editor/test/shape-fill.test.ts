// How a fill is drawn: the line of a linear gradient and the bitmaps of the preset patterns. Pure functions, no DOM.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { linearGradientLine, PATTERN_NAMES, patternPixels } from "../src/editor/shape-fill.ts";

const near = (a: number, b: number) => assert.ok(Math.abs(a - b) < 1e-6, `${a} is not ${b}`);

test("a gradient along an axis runs edge to edge of the box", () => {
  const down = linearGradientLine(90, 200, 50, false);
  assert.equal(down.units, "box");
  near(down.y1, 0);
  near(down.y2, 1);
  near(down.x1, 0.5);
  const right = linearGradientLine(0, 200, 50, false);
  near(right.x1, 0);
  near(right.x2, 1);
});

test("a scaled 45 degree gradient runs corner to corner, an unscaled one at a true angle", () => {
  const scaled = linearGradientLine(45, 200, 50, true);
  assert.deepEqual([scaled.x1, scaled.y1, scaled.x2, scaled.y2].map((v) => Math.round(v * 1000) / 1000), [0, 0, 1, 1]);
  const true45 = linearGradientLine(45, 200, 50, false);
  assert.equal(true45.units, "user");
  // The line is at 45 degrees on the page, through the centre, and its ends project on the far corners.
  near(true45.x2 - true45.x1, true45.y2 - true45.y1);
  near((true45.x1 + true45.x2) / 2, 100);
  near((true45.y1 + true45.y2) / 2, 25);
  near(Math.hypot(true45.x2 - true45.x1, true45.y2 - true45.y1), (200 + 50) * Math.SQRT1_2);
});

test("every DrawingML pattern preset has a bitmap, and the coverage grows with the percentage", () => {
  assert.equal(PATTERN_NAMES.length, 54);
  const cover = (name: string) => patternPixels(name).flat().filter(Boolean).length;
  const pcts = [5, 10, 20, 25, 30, 40, 50, 60, 70, 75, 80, 90].map((p) => cover(`pct${p}`));
  assert.deepEqual([...pcts].sort((a, b) => a - b), pcts);
  assert.equal(cover("pct50"), 32);
  for (const name of PATTERN_NAMES) assert.ok(cover(name) > 0 && cover(name) < 64, name);
});

test("horizontal patterns are rows, vertical ones columns, diagonals run the right way", () => {
  const rows = patternPixels("ltHorz");
  assert.ok(rows[0].every(Boolean) && rows[4].every(Boolean) && rows[1].every((b) => !b));
  const cols = patternPixels("ltVert");
  assert.ok(cols.every((r) => r[0] && r[4] && !r[1]));
  // Down to the right: the pixel one step right and one down is on again.
  const dn = patternPixels("ltDnDiag");
  assert.ok(dn[0][0] && dn[1][1] && dn[2][2] && !dn[0][1]);
  // Up to the right: the pixel one step right and one up is on again.
  const up = patternPixels("ltUpDiag");
  assert.ok(up[4][0] && up[3][1] && up[2][2] && !up[0][1]);
});
