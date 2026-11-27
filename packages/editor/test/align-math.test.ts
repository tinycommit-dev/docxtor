// Aligning and distributing: how far each box moves, without a DOM.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { alignMoves, axisOf, bounds, distributeMoves, type Rect } from "../src/ui/align-math.ts";

const box = (left: number, top: number, w: number, h: number): Rect => ({ left, top, right: left + w, bottom: top + h });
const near = (a: number, b: number, eps = 1e-9) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);

// A at 100 to 180, B at 200 to 300, C at 330 to 390; tops 200, 260, 230; bottoms 250, 300, 300.
const A = box(100, 200, 80, 50);
const B = box(200, 260, 100, 40);
const C = box(330, 230, 60, 70);

test("the edges belong to an axis", () => {
  assert.deepEqual((["left", "center", "right", "top", "middle", "bottom"] as const).map(axisOf), ["h", "h", "h", "v", "v", "v"]);
});

test("bounds is the box round all of them", () => {
  assert.deepEqual(bounds([A, B, C]), { left: 100, top: 200, right: 390, bottom: 300 });
});

test("left, center and right meet the group's edge", () => {
  const group = bounds([A, B, C]);
  assert.deepEqual(alignMoves([A, B, C], "left", group), [0, -100, -230]);
  assert.deepEqual(alignMoves([A, B, C], "right", group), [210, 90, 0]);
  const centred = alignMoves([A, B, C], "center", group);
  // The group's middle is 245: A's is 140, B's 250, C's 360.
  assert.deepEqual(centred, [105, -5, -115]);
});

test("top, middle and bottom do the same down the page", () => {
  const group = bounds([A, B, C]);
  assert.deepEqual(alignMoves([A, B, C], "top", group), [0, -60, -30]);
  assert.deepEqual(alignMoves([A, B, C], "bottom", group), [50, 0, 0]);
  assert.deepEqual(alignMoves([A, B, C], "middle", group), [25, -30, -15]);
});

test("aligning to a page or margins box works the same way", () => {
  const page = box(0, 0, 612, 792);
  assert.deepEqual(alignMoves([A, C], "right", page), [612 - 180, 612 - 390]);
  assert.deepEqual(alignMoves([A], "bottom", page), [792 - 250]);
});

test("distributing keeps the outer two and makes the gaps equal", () => {
  const moves = distributeMoves([A, B, C], "h");
  const placed = [A, B, C].map((r, i) => ({ left: r.left + moves[i], right: r.right + moves[i] }));
  near(placed[0].left, 100);
  near(placed[2].right, 390);
  // Widths 80 + 100 + 60 = 240 in 290: gaps of 25.
  near(placed[1].left - placed[0].right, 25);
  near(placed[2].left - placed[1].right, 25);
});

test("distributing does not depend on the order the boxes are given in", () => {
  const moves = distributeMoves([C, A, B], "h");
  const placed = [C, A, B].map((r, i) => r.left + moves[i]);
  // A, B, C by their lefts: 100, 205, 330.
  near(placed[1], 100);
  near(placed[2], 205);
  near(placed[0], 330);
});

test("distributing down the page", () => {
  // Tops 200, 230, 260 once in order (A, C, B); heights 50, 70, 40; the group runs 200 to 300.
  const moves = distributeMoves([A, B, C], "v");
  const placed = [A, B, C].map((r, i) => ({ top: r.top + moves[i], bottom: r.bottom + moves[i] }));
  near(placed[0].top, 200);
  near(placed[1].bottom, 300);
  near(placed[2].top - placed[0].bottom, placed[1].top - placed[2].bottom);
});

test("across the margins the first lands on the start and the last on the end", () => {
  const margins = box(72, 72, 468, 648);
  const moves = distributeMoves([A, B], "h", margins);
  near(A.left + moves[0], 72);
  near(B.right + moves[1], 540);
  const three = distributeMoves([A, B, C], "h", margins);
  near(A.left + three[0], 72);
  near(C.right + three[2], 540);
  const g1 = B.left + three[1] - (A.right + three[0]);
  const g2 = C.left + three[2] - (B.right + three[1]);
  near(g1, g2);
});

test("one box has nothing to be distributed against", () => {
  assert.deepEqual(distributeMoves([A], "h"), [0]);
});
