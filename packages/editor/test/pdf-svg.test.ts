// The geometry behind drawing SVG as PDF vectors: path data, arcs as curves, shapes as paths, matrices and gradients. No DOM.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { apply, axialShading, ellipseSegments, gradientFunction, IDENTITY, invert, isSimilarity, mul, parsePathData, pathOps, pointList, polySegments, radialShading, rectSegments, scaleOf, transformSegments, type Seg } from "../src/pdf/svgdraw.ts";

const close = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) < eps, `${a} is not ${b}`);
const end = (s: Seg | undefined): [number, number] => (s && s[0] !== "Z" ? [s[s.length - 2] as number, s[s.length - 1] as number] : [NaN, NaN]);

test("path data: moves, lines, relative and absolute, horizontal and vertical, close", () => {
  assert.deepEqual(parsePathData("M10,20 L30,40 H50 V60 Z"), [["M", 10, 20], ["L", 30, 40], ["L", 50, 40], ["L", 50, 60], ["Z"]]);
  assert.deepEqual(parsePathData("m10 20 l5 5 h5 v-5 z"), [["M", 10, 20], ["L", 15, 25], ["L", 20, 25], ["L", 20, 20], ["Z"]]);
  // More pairs after a move are lines; numbers can run together by their signs and decimal points.
  assert.deepEqual(parsePathData("M0 0 10 0 10 10"), [["M", 0, 0], ["L", 10, 0], ["L", 10, 10]]);
  assert.deepEqual(parsePathData("M.5-.5L1e1,2.5E0"), [["M", 0.5, -0.5], ["L", 10, 2.5]]);
  // After a close, the next relative move starts from where the subpath began.
  assert.deepEqual(parsePathData("M10 10 l5 0 z m5 5 l1 1"), [["M", 10, 10], ["L", 15, 10], ["Z"], ["M", 15, 15], ["L", 16, 16]]);
  assert.deepEqual(parsePathData(""), []);
  assert.equal(parsePathData("L10 10"), null, "a path begins with a move");
  assert.equal(parsePathData("M10"), null);
  assert.equal(parsePathData("M10 10 X"), null);
});

test("curves: cubic, smooth, quadratic and smooth quadratic", () => {
  assert.deepEqual(parsePathData("M0 0 C10 0 20 10 30 10"), [["M", 0, 0], ["C", 10, 0, 20, 10, 30, 10]]);
  // S reflects the previous second control point; with no curve before it, it starts at the point.
  assert.deepEqual(parsePathData("M0 0 C10 0 20 10 30 10 S50 20 60 10"), [["M", 0, 0], ["C", 10, 0, 20, 10, 30, 10], ["C", 40, 10, 50, 20, 60, 10]]);
  assert.deepEqual(parsePathData("M0 0 S10 10 20 0"), [["M", 0, 0], ["C", 0, 0, 10, 10, 20, 0]]);
  // A quadratic curve is the cubic with the same shape: control points two thirds of the way to the single one.
  const [, q] = parsePathData("M0 0 Q30 60 60 0")!;
  assert.deepEqual(q, ["C", 20, 40, 40, 40, 60, 0]);
  const [, , t] = parsePathData("M0 0 Q30 60 60 0 T120 0")!;
  assert.deepEqual(t, ["C", 80, -40, 100, -40, 120, 0]);
});

test("arcs become cubic curves that start and end where the arc does, on the circle", () => {
  // A half circle of radius 10 from (0,0) to (20,0).
  const half = parsePathData("M0 0 A10 10 0 0 1 20 0")!;
  assert.equal(half.length, 3, "a half circle is two quarter curves");
  close(end(half[2])[0], 20);
  close(end(half[2])[1], 0);
  // Its midpoint is on the circle of centre (10, 0): 10 away.
  const mid = end(half[1]);
  close(Math.hypot(mid[0] - 10, mid[1]), 10, 1e-6);
  assert.ok(mid[1] < 0, "sweep 1 goes the way of increasing angle, which is up in y-down coordinates");
  // The other sweep goes the other way, the large flag takes the long way round.
  assert.ok(end(parsePathData("M0 0 A10 10 0 0 0 20 0")![1])[1] > 0);
  const large = parsePathData("M0 0 A10 10 0 1 1 10 10")!;
  assert.equal(large.length, 1 + 3, "three quarters of a circle is three curves");
  close(end(large[3])[0], 10);
  close(end(large[3])[1], 10);
  // The flags may be written with no space after them, and radii that are too small are made big enough.
  const packed = parsePathData("M0 0a10 10 0 0120 0")!;
  assert.equal(packed.length, 3);
  const big = parsePathData("M0 0 A1 1 0 0 1 20 0")!;
  close(end(big[big.length - 1])[0], 20);
  assert.deepEqual(parsePathData("M5 5 A0 10 0 0 1 20 20"), [["M", 5, 5], ["L", 20, 20]], "a zero radius is a line");
});

test("rectangles, rounded rectangles, ellipses and lines are paths", () => {
  assert.deepEqual(rectSegments(1, 2, 10, 20), [["M", 1, 2], ["L", 11, 2], ["L", 11, 22], ["L", 1, 22], ["Z"]]);
  assert.deepEqual(rectSegments(0, 0, 0, 5), []);
  const rounded = rectSegments(0, 0, 100, 50, 10, 10);
  assert.equal(rounded.filter((s) => s[0] === "C").length, 4);
  // Corner radii are held to half the sides.
  assert.deepEqual(rectSegments(0, 0, 10, 10, 50, 50).filter((s) => s[0] === "L").length, 4);
  const circle = ellipseSegments(5, 5, 5, 5);
  assert.deepEqual(end(circle[0]), [10, 5]);
  assert.equal(circle.filter((s) => s[0] === "C").length, 4);
  // The curves pass through the four extremes.
  assert.deepEqual([1, 2, 3, 4].map((i) => end(circle[i])), [[5, 10], [0, 5], [5, 0], [10, 5]]);
  assert.deepEqual(polySegments([0, 0, 10, 10, 20, 0], false), [["M", 0, 0], ["L", 10, 10], ["L", 20, 0]]);
  assert.deepEqual(polySegments([0, 0, 10, 10, 20, 0], true).at(-1), ["Z"]);
  assert.deepEqual(polySegments([1, 2], false), []);
  assert.deepEqual(pointList("0,0 10.5,-3 1e1,2"), [0, 0, 10.5, -3, 10, 2]);
});

test("matrices: product, apply, scale and what keeps squares square", () => {
  const move: [number, number, number, number, number, number] = [1, 0, 0, 1, 10, 20];
  const twice: [number, number, number, number, number, number] = [2, 0, 0, 2, 0, 0];
  // "a after b": scale then move.
  assert.deepEqual(apply(mul(move, twice), 1, 1), [12, 22]);
  assert.deepEqual(apply(mul(twice, move), 1, 1), [22, 42]);
  assert.deepEqual(mul(IDENTITY, move), move);
  assert.equal(scaleOf(twice), 2);
  assert.ok(isSimilarity(twice));
  assert.ok(isSimilarity([0, 1, -1, 0, 5, 5]), "a quarter turn");
  assert.ok(!isSimilarity([2, 0, 0, 1, 0, 0]), "a stretch");
  assert.ok(!isSimilarity([1, 0, 0, -1, 0, 0]), "a mirror");
  // The inverse undoes it; a flat matrix has none.
  const turned: [number, number, number, number, number, number] = [0, 1, -1, 0, 7, 3];
  const back = invert(mul(move, turned))!;
  const [bx, by] = apply(back, ...apply(mul(move, turned), 4, 9));
  close(bx, 4);
  close(by, 9);
  assert.equal(invert([1, 2, 2, 4, 0, 0]), null);
  const [m, l] = transformSegments([["M", 1, 1], ["L", 2, 3], ["Z"]], twice);
  assert.deepEqual([m, l], [["M", 2, 2], ["L", 4, 6]]);
  assert.equal(pathOps([["M", 0, 0], ["L", 1.5, 2], ["C", 1, 2, 3, 4, 5, 6], ["Z"]]), "0 0 m 1.5 2 l 1 2 3 4 5 6 c h");
});

test("gradients: a function between colours, stitched for more stops, held beyond the ends", () => {
  const two = gradientFunction([{ offset: 0, rgb: [1, 0, 0] }, { offset: 1, rgb: [0, 0, 1] }]);
  assert.equal(two, "<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>");
  const three = gradientFunction([{ offset: 0, rgb: [1, 0, 0] }, { offset: 0.5, rgb: [0, 1, 0] }, { offset: 1, rgb: [0, 0, 1] }]);
  assert.match(three, /^<< \/FunctionType 3 \/Domain \[0 1\] \/Functions \[<<.*>> <<.*>>\] \/Bounds \[0\.5\] \/Encode \[0 1 0 1\] >>$/);
  // Stops that start late or end early hold their colour out to the ends.
  const held = gradientFunction([{ offset: 0.25, rgb: [1, 0, 0] }, { offset: 0.75, rgb: [0, 0, 1] }]);
  assert.match(held, /\/Bounds \[0\.25 0\.75\]/);
  assert.equal((held.match(/FunctionType 2/g) ?? []).length, 3);
  // Offsets that go backwards are held where they were; one stop is a flat colour.
  assert.match(gradientFunction([{ offset: 0.6, rgb: [1, 0, 0] }, { offset: 0.2, rgb: [0, 1, 0] }, { offset: 1, rgb: [0, 0, 1] }]), /\/Bounds \[0\.6 0\.6\]/);
  assert.match(gradientFunction([{ offset: 0.5, rgb: [0, 0, 0] }]), /\/C0 \[0 0 0\] \/C1 \[0 0 0\]/);
  assert.equal(axialShading(0, 0, 1, 0, [{ offset: 0, rgb: [1, 1, 1] }, { offset: 1, rgb: [0, 0, 0] }]), "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 1 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 1 1] /C1 [0 0 0] /N 1 >> /Extend [true true] >>");
  assert.match(radialShading(0.5, 0.5, 0.5, 0.5, 0.5, [{ offset: 0, rgb: [1, 1, 1] }, { offset: 1, rgb: [0, 0, 0] }]), /^<< \/ShadingType 3 \/ColorSpace \/DeviceRGB \/Coords \[0\.5 0\.5 0 0\.5 0\.5 0\.5\] /);
});
