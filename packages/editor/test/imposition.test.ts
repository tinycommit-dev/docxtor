import { strict as assert } from "node:assert";
import { test } from "node:test";
import { halfPage, imposition } from "../src/editor/imposition.ts";

test("two pages per sheet go in order, the last half blank when the count is odd", () => {
  assert.deepEqual(imposition(5, "twoUp"), [[0, 1], [2, 3], [4, null]]);
});

test("a book fold of 8 pages: two sheets, 8|1 and 2|7, then 6|3 and 4|5", () => {
  assert.deepEqual(imposition(8, "bookFold"), [[7, 0], [1, 6], [5, 2], [3, 4]]);
});

test("a book fold pads to a multiple of four with blank pages, and right-to-left swaps the halves", () => {
  assert.deepEqual(imposition(6, "bookFold"), [[null, 0], [1, null], [5, 2], [3, 4]]);
  assert.deepEqual(imposition(4, "bookFoldRev"), [[0, 3], [2, 1]]);
});

test("sheets per booklet make several booklets, each folded on its own", () => {
  // One sheet per booklet: pages 1-4, then 5-8.
  assert.deepEqual(imposition(8, "bookFold", 1), [[3, 0], [1, 2], [7, 4], [5, 6]]);
});

test("a page is half the sheet: across a landscape one, down a portrait one", () => {
  assert.deepEqual(halfPage(1056, 816), { w: 528, h: 816 });
  assert.deepEqual(halfPage(816, 1056), { w: 816, h: 528 });
});
