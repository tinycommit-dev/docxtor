// Editing the points of an outline: SVG presets as commands, moving, adding, deleting and retyping points. Pure functions.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { connectorD } from "../src/editor/connector-paths.ts";
import { anchors, cmdsToD, deleteAnchor, handlesOf, movePoint, nearest, point, pointType, setPointType, simplify, smoothClosed, smoothOpen, splitAt, svgToCmds, twin, type Cmd } from "../src/editor/path-edit.ts";

const near = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);
const letters = (c: Cmd[]) => c.map((x) => x.t).join("");

const SQUARE: Cmd[] = [
  { t: "M", pts: [[0, 0]] },
  { t: "L", pts: [[100, 0]] },
  { t: "L", pts: [[100, 100]] },
  { t: "L", pts: [[0, 100]] },
  { t: "Z", pts: [] },
];

test("a preset's path data becomes commands, arcs as cubic curves", () => {
  const rect = svgToCmds("M0,0 L100,0 L100,60 L0,60 Z");
  assert.equal(letters(rect), "MLLLZ");
  assert.deepEqual(rect[2].pts, [[100, 60]]);
  // A rounded rectangle: four quarter arcs, each one curve.
  const round = svgToCmds("M0,10 A10,10 0 0 1 10,0 L90,0 A10,10 0 0 1 100,10 L100,50 A10,10 0 0 1 90,60 L10,60 A10,10 0 0 1 0,50 Z");
  assert.equal(letters(round), "MCLCLCLCZ");
  // The first arc runs from (0,10) to (10,0) and bulges outwards (its handles are on the corner side).
  const arc = round[1].pts;
  near(arc[2][0], 10);
  near(arc[2][1], 0);
  near(arc[0][0], 0);
  assert.ok(arc[0][1] < 10 && arc[0][1] > 0);
  // A full ellipse is two half arcs: four curves in all.
  const oval = svgToCmds("M0,30 A50,30 0 1 1 100,30 A50,30 0 1 1 0,30 Z");
  assert.equal(letters(oval), "MCCCCZ");
  near(oval[2].pts[2][0], 100);
  near(oval[2].pts[2][1], 30);
  // A quadratic curve.
  const q = svgToCmds("M0,0 Q50,50 100,0");
  assert.equal(letters(q), "MC");
  near(q[1].pts[0][0], (2 / 3) * 50);
});

test("the anchors of an outline are its moves, lines and curve ends", () => {
  assert.equal(anchors(SQUARE).length, 4);
  const curve: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "C", pts: [[10, 0], [20, 10], [30, 10]] }];
  assert.deepEqual(anchors(curve), [{ cmd: 0, pt: 0 }, { cmd: 1, pt: 2 }]);
  assert.deepEqual(handlesOf(curve, { cmd: 0, pt: 0 }), { in: null, out: { cmd: 1, pt: 0 } });
  assert.deepEqual(handlesOf(curve, { cmd: 1, pt: 2 }), { in: { cmd: 1, pt: 1 }, out: null });
});

test("moving an anchor takes its handles along", () => {
  const curve: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "C", pts: [[10, 0], [20, 10], [30, 10]] }, { t: "C", pts: [[40, 10], [50, 0], [60, 0]] }];
  const moved = movePoint(curve, { cmd: 1, pt: 2 }, [32, 14]);
  assert.deepEqual(moved[1].pts[2], [32, 14]);
  assert.deepEqual(moved[1].pts[1], [22, 14]);
  assert.deepEqual(moved[2].pts[0], [42, 14]);
  // The others stayed, and the input was not changed.
  assert.deepEqual(moved[1].pts[0], [10, 0]);
  assert.deepEqual(curve[1].pts[2], [30, 10]);
});

test("the vertex where a closed outline ends and starts again moves as one", () => {
  const loop: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "L", pts: [[50, 0]] }, { t: "L", pts: [[50, 50]] }, { t: "L", pts: [[0, 0]] }, { t: "Z", pts: [] }];
  assert.deepEqual(twin(loop, { cmd: 0, pt: 0 }), { cmd: 3, pt: 0 });
  const moved = movePoint(loop, { cmd: 0, pt: 0 }, [-5, -5]);
  assert.deepEqual(moved[0].pts[0], [-5, -5]);
  assert.deepEqual(moved[3].pts[0], [-5, -5]);
  // A plain closed outline has no twin.
  assert.equal(twin(SQUARE, { cmd: 0, pt: 0 }), null);
});

test("dragging a handle of a smooth point turns the other handle to match", () => {
  const smooth: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "C", pts: [[10, 0], [20, 0], [30, 0]] }, { t: "C", pts: [[40, 0], [50, 10], [60, 10]] }];
  assert.equal(pointType(smooth, { cmd: 1, pt: 2 }), "smooth");
  const out = movePoint(smooth, { cmd: 2, pt: 0 }, [30, 10], { handle: true });
  // The outgoing handle now points straight down from the anchor; the incoming one points straight up, as long as before.
  assert.deepEqual(out[2].pts[0], [30, 10]);
  near(out[1].pts[1][0], 30);
  near(out[1].pts[1][1], -10);
  // A corner point leaves the other handle alone.
  const corner = movePoint(smooth, { cmd: 2, pt: 0 }, [30, 10], { handle: true, keepSmooth: false });
  assert.deepEqual(corner[1].pts[1], [20, 0]);
});

test("a point is added on a line or a curve without changing the shape", () => {
  const { cmds, added } = splitAt(SQUARE, 1, 0.5);
  assert.equal(letters(cmds), "MLLLLZ");
  assert.deepEqual(point(cmds, added), [50, 0]);
  const curve: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "C", pts: [[0, 40], [100, 40], [100, 0]] }];
  const split = splitAt(curve, 1, 0.5);
  assert.equal(letters(split.cmds), "MCC");
  // The middle of this curve is at x = 50, y = 30.
  near(point(split.cmds, split.added)[0], 50);
  near(point(split.cmds, split.added)[1], 30);
  // The closing line of an outline can be split as well.
  const closing = splitAt(SQUARE, 4, 0.5);
  assert.equal(letters(closing.cmds), "MLLLLZ");
  assert.deepEqual(point(closing.cmds, closing.added), [0, 50]);
});

test("the nearest point of an outline", () => {
  const hit = nearest(SQUARE, [50, 3], 6)!;
  assert.equal(hit.cmd, 1);
  near(hit.t, 0.5);
  near(hit.pt[1], 0);
  assert.equal(nearest(SQUARE, [50, 40], 6), null);
  // The close counts as a side.
  assert.equal(nearest(SQUARE, [2, 50], 6)!.cmd, 4);
});

test("deleting an anchor joins its neighbours, and an outline keeps enough points", () => {
  const four = deleteAnchor(SQUARE, { cmd: 2, pt: 0 })!;
  assert.equal(letters(four), "MLLZ");
  assert.deepEqual(four[2].pts[0], [0, 100]);
  // Deleting the first vertex makes the second the start.
  const first = deleteAnchor(SQUARE, { cmd: 0, pt: 0 })!;
  assert.equal(letters(first), "MLLZ");
  assert.deepEqual(first[0].pts[0], [100, 0]);
  // A triangle can't lose a point.
  assert.equal(deleteAnchor(four, { cmd: 1, pt: 0 }), null);
  // Curves on either side join into one curve that keeps the outer handles.
  const curve: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "C", pts: [[10, 0], [20, 10], [30, 10]] }, { t: "C", pts: [[40, 10], [50, 0], [60, 0]] }, { t: "L", pts: [[60, 30]] }];
  const joined = deleteAnchor(curve, { cmd: 1, pt: 2 })!;
  assert.equal(letters(joined), "MCL");
  assert.deepEqual(joined[1].pts, [[10, 0], [50, 0], [60, 0]]);
  // The last point of an open outline just ends it sooner.
  assert.equal(letters(deleteAnchor(curve, { cmd: 3, pt: 0 })!), "MCC");
});

test("a point can be made straight, smooth or corner", () => {
  const curve: Cmd[] = [{ t: "M", pts: [[0, 0]] }, { t: "C", pts: [[10, 20], [20, 20], [30, 0]] }, { t: "C", pts: [[40, -10], [50, -20], [60, 0]] }];
  assert.equal(pointType(curve, { cmd: 1, pt: 2 }), "corner");
  const smooth = setPointType(curve, { cmd: 1, pt: 2 }, "smooth");
  assert.equal(pointType(smooth, { cmd: 1, pt: 2 }), "smooth");
  // Lines before and after become lines for a straight point.
  const straight = setPointType(curve, { cmd: 1, pt: 2 }, "straight");
  assert.equal(letters(straight), "MLL");
  assert.equal(pointType(straight, { cmd: 1, pt: 0 }), "straight");
  // A corner point on a line makes the line a curve with handles.
  const corner = setPointType(SQUARE, { cmd: 1, pt: 0 }, "corner");
  assert.equal(letters(corner), "MCCLZ");
  assert.equal(corner[1].t, "C");
  assert.equal(corner[2].t, "C");
  assert.equal(pointType(corner, { cmd: 1, pt: 2 }), "corner");
});

test("a hand-drawn line is simplified and smoothed the way the engine does", () => {
  const wave: [number, number][] = Array.from({ length: 100 }, (_, i) => [i, Math.sin(i / 8) * 20]);
  const simple = simplify(wave, 0.35);
  assert.ok(simple.length > 5 && simple.length < 40, `${simple.length} points`);
  assert.deepEqual(simple[0], wave[0]);
  assert.deepEqual(simple[simple.length - 1], wave[99]);
  const smooth = smoothOpen(wave);
  assert.equal(smooth.length, simple.length - 1);
  assert.ok(smooth.every((c) => c.t === "C"));
  // Two points are a straight segment, a closed curve wraps round to the first point.
  assert.deepEqual(smoothOpen([[0, 0], [10, 10]]), [{ t: "L", pts: [[10, 10]] }]);
  const loop = smoothClosed([[0, 0], [50, 0], [50, 50], [0, 50]]);
  assert.equal(letters(loop), "MCCCCZ");
  assert.deepEqual(loop[4].pts[2], [0, 0]);
  assert.match(cmdsToD(loop), /^M0,0 C/);
});

test("the connector presets are drawn from their formulas", () => {
  assert.equal(connectorD("bentConnector2", 100, 60, undefined), "M0,0 L100,0 L100,60");
  assert.equal(connectorD("bentConnector3", 100, 60, undefined), "M0,0 L50,0 L50,60 L100,60");
  // The bend can lie outside the box: an adjust value over 100000 goes past the end.
  assert.equal(connectorD("bentConnector3", 100, 60, { adj1: 118000 }), "M0,0 L118,0 L118,60 L100,60");
  assert.equal(connectorD("bentConnector4", 100, 60, { adj1: 50000, adj2: 130000 }), "M0,0 L50,0 L50,78 L100,78 L100,60");
  assert.equal(connectorD("bentConnector5", 100, 60, { adj1: -18000, adj2: 50000, adj3: 118000 }), "M0,0 L-18,0 L-18,30 L118,30 L118,60 L100,60");
  const curved = connectorD("curvedConnector3", 100, 60, undefined)!;
  assert.match(curved, /^M0,0 C25,0 50,15 50,30 C50,45 75,60 100,60$/);
  assert.equal(connectorD("curvedConnector2", 100, 60, undefined), "M0,0 C50,0 100,30 100,60");
  assert.equal(connectorD("straightConnector1", 100, 60, undefined), null);
  assert.ok(connectorD("curvedConnector5", 100, 60, undefined)!.split("C").length === 5);
});
