// The drawing tools' geometry: nearness to a stroke, cutting out part of one, and the lasso, without a DOM.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { boundsOf, caughtBy, cutOut, densify, distToSegment, distToStroke, inLoop, type Pt } from "../src/ui/ink-math.ts";

const line = (x1: number, y1: number, x2: number, y2: number, n = 10): Pt[] => Array.from({ length: n + 1 }, (_, i) => ({ x: x1 + ((x2 - x1) * i) / n, y: y1 + ((y2 - y1) * i) / n }));
const near = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);

test("distance to a segment is to its nearest point", () => {
  near(distToSegment({ x: 5, y: 3 }, { x: 0, y: 0 }, { x: 10, y: 0 }), 3);
  near(distToSegment({ x: -4, y: 3 }, { x: 0, y: 0 }, { x: 10, y: 0 }), 5);
  near(distToSegment({ x: 3, y: 4 }, { x: 0, y: 0 }, { x: 0, y: 0 }), 5);
});

test("distance to a stroke is to its nearest segment, and to the point for a dot", () => {
  const s = [{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 10 }];
  near(distToStroke({ x: 12, y: 5 }, s), 2);
  near(distToStroke({ x: 3, y: 4 }, [{ x: 0, y: 0 }]), 5);
});

test("densify adds points so no gap is wider than the step, and keeps the ends", () => {
  const d = densify([{ x: 0, y: 0 }, { x: 100, y: 0 }], 10);
  assert.equal(d.length, 11);
  assert.deepEqual(d[0], { x: 0, y: 0 });
  assert.deepEqual(d[d.length - 1], { x: 100, y: 0 });
  assert.equal(densify([{ x: 1, y: 1 }], 5).length, 1);
});

test("a circle off the stroke cuts nothing", () => {
  assert.equal(cutOut(line(0, 0, 100, 0), { x: 50, y: 30 }, 10), null);
});

test("a circle on the middle splits a stroke in two, leaving a gap as wide as the circle", () => {
  const pieces = cutOut(line(0, 0, 100, 0), { x: 50, y: 0 }, 10)!;
  assert.equal(pieces.length, 2);
  const [a, b] = pieces;
  const end = a[a.length - 1].x;
  const start = b[0].x;
  assert.ok(end <= 40 + 1e-9 && end > 35, `the first piece ends at ${end}`);
  assert.ok(start >= 60 - 1e-9 && start < 65, `the second piece starts at ${start}`);
  assert.equal(a[0].x, 0);
  assert.equal(b[b.length - 1].x, 100);
});

test("a circle on the end takes the end off, and one over the whole stroke takes all of it", () => {
  const end = cutOut(line(0, 0, 100, 0), { x: 100, y: 0 }, 10)!;
  assert.equal(end.length, 1);
  assert.ok(end[0][end[0].length - 1].x <= 90 + 1e-9);
  assert.deepEqual(cutOut(line(0, 0, 8, 0, 4), { x: 4, y: 0 }, 10), []);
});

test("a circle between two far-apart points still cuts the segment between them", () => {
  const pieces = cutOut([{ x: 0, y: 0 }, { x: 200, y: 0 }], { x: 100, y: 0 }, 5)!;
  assert.equal(pieces.length, 2);
});

test("inLoop tells inside from outside", () => {
  const square: Pt[] = [{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 10 }, { x: 0, y: 10 }];
  assert.ok(inLoop({ x: 5, y: 5 }, square));
  assert.ok(!inLoop({ x: 15, y: 5 }, square));
  assert.ok(!inLoop({ x: 5, y: -1 }, square));
});

test("a lasso catches a stroke inside it, and one it only crosses, and leaves the others", () => {
  const loop: Pt[] = [{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 100 }, { x: 0, y: 100 }];
  assert.ok(caughtBy(loop, line(10, 50, 90, 50)));
  // From inside to outside: some points inside.
  assert.ok(caughtBy(loop, line(50, 50, 300, 50)));
  // Right across with both ends outside: only the loop's edges cross it.
  assert.ok(caughtBy(loop, [{ x: -50, y: 50 }, { x: 150, y: 50 }]));
  assert.ok(!caughtBy(loop, line(200, 200, 300, 300)));
  // Two points make no loop.
  assert.ok(!caughtBy([{ x: 0, y: 0 }, { x: 1, y: 1 }], line(0, 0, 5, 5)));
});

test("boundsOf is the box round the strokes' points", () => {
  assert.deepEqual(boundsOf([line(0, 0, 10, 20), line(-5, 3, 4, 40)]), { left: -5, top: 0, right: 10, bottom: 40 });
  assert.equal(boundsOf([]), null);
});
