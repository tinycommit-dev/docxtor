// WordArt's text warps: the geometry of every preset, without a DOM.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { EXACT, FLAT, PATHS, followPath, meshBounds, sliceCount, slices, warpDef } from "../src/editor/warp.ts";

const PRESETS = [
  "textPlain", "textStop", "textTriangle", "textTriangleInverted", "textChevron", "textChevronInverted", "textRingInside", "textRingOutside",
  "textArchUp", "textArchDown", "textCircle", "textButton", "textArchUpPour", "textArchDownPour", "textCirclePour", "textButtonPour",
  "textCurveUp", "textCurveDown", "textCascadeUp", "textCascadeDown", "textWave1", "textWave2", "textDoubleWave1", "textWave4",
  "textInflate", "textDeflate", "textInflateBottom", "textDeflateBottom", "textInflateTop", "textDeflateTop", "textDeflateInflate",
  "textDeflateInflateDeflate", "textFadeRight", "textFadeLeft", "textFadeUp", "textFadeDown", "textSlantUp", "textSlantDown", "textCanUp", "textCanDown",
];

const W = 300;
const H = 100;
const near = (a: number, b: number, eps = 1e-3) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);

test("all forty presets are known and keep the text inside the box", () => {
  assert.equal(PRESETS.length, 40);
  for (const preset of PRESETS) {
    const def = warpDef(preset, undefined, W, H);
    if (def.kind === "path") {
      assert.ok(PATHS.has(preset), preset);
      continue;
    }
    assert.ok(def.mesh, preset);
    for (let i = 0; i <= 20; i++) {
      for (let j = 0; j <= 20; j++) {
        const [x, y] = def.mesh!(i / 20, j / 20);
        assert.ok(Number.isFinite(x) && Number.isFinite(y), `${preset} gives a number`);
        assert.ok(x >= -0.01 && x <= W + 0.01 && y >= -0.01 && y <= H + 0.01, `${preset} at ${i / 20},${j / 20} is ${x},${y}, outside the box`);
      }
    }
  }
});

test("plain text fills the box and nothing bends", () => {
  const def = warpDef("textPlain", undefined, W, H);
  assert.equal(def.kind, "plain");
  assert.equal(def.approx, false);
  assert.deepEqual(def.mesh!(0, 0), [0, 0]);
  assert.deepEqual(def.mesh!(1, 1), [W, H]);
  assert.deepEqual(def.mesh!(0.5, 0.5), [W / 2, H / 2]);
  // Two warps that bend one line at a time are drawn as plain text.
  for (const p of FLAT) assert.equal(warpDef(p, undefined, W, H).approx, true);
  assert.ok(EXACT.has("textPlain") && EXACT.has("textArchUp"));
});

test("arch up pours the text over the top half of a ring", () => {
  const m = warpDef("textArchUpPour", undefined, W, H).mesh!;
  // The text starts at the lower left and ends at the lower right, its top on the outer edge.
  const [x0, y0] = m(0, 0);
  const [x1, y1] = m(1, 0);
  near(x0, 0);
  near(x1, W);
  near(y0, H);
  near(y1, H);
  // The middle of the top edge is the top of the box; the bottom edge is the inner ring (half the radius).
  const [mx, my] = m(0.5, 0);
  near(mx, W / 2);
  near(my, 0);
  const [bx, by] = m(0.5, 1);
  near(bx, W / 2);
  near(by, H / 2);
  // The inner ring's ends are inside the outer ones.
  near(m(0, 1)[0], W / 4);
  near(m(1, 1)[0], (3 * W) / 4);
});

test("arch down pours the text under, its tops toward the middle", () => {
  const m = warpDef("textArchDownPour", undefined, W, H).mesh!;
  near(m(0, 0)[0], W / 4);
  near(m(0.5, 0)[1], H / 2);
  near(m(0.5, 1)[1], H);
  near(m(0, 1)[0], 0);
  near(m(1, 1)[0], W);
});

test("a circle pour goes all the way round", () => {
  const m = warpDef("textCirclePour", undefined, 200, 200).mesh!;
  // Starts at the left, a quarter of the way is the top.
  near(m(0, 0)[0], 0);
  near(m(0, 0)[1], 100);
  near(m(0.25, 0)[0], 100);
  near(m(0.25, 0)[1], 0);
  near(m(0.5, 0)[0], 200);
  near(m(0.75, 0)[1], 200);
  near(m(1, 0)[0], 0);
});

test("waves swing the band up and down without changing its height", () => {
  const m = warpDef("textWave1", undefined, W, H).mesh!;
  const height = (u: number) => m(u, 1)[1] - m(u, 0)[1];
  near(height(0), height(0.25));
  near(height(0.25), height(0.8));
  // One period: the top of the band is at its highest a quarter of the way, lowest at three quarters.
  assert.ok(m(0.25, 0)[1] < m(0, 0)[1] && m(0.75, 0)[1] > m(0, 0)[1]);
  near(m(0, 0)[1], m(1, 0)[1]);
  // Wave 2 is the other way round, and the double waves have two periods.
  const w2 = warpDef("textWave2", undefined, W, H).mesh!;
  assert.ok(w2(0.25, 0)[1] > w2(0, 0)[1]);
  const d = warpDef("textDoubleWave1", undefined, W, H).mesh!;
  near(d(0.125, 0)[1], d(0.625, 0)[1]);
  assert.ok(d(0.125, 0)[1] < d(0, 0)[1]);
  // A larger adjust is a bigger swing.
  const big = warpDef("textWave1", { adj1: 20000 }, W, H).mesh!;
  assert.ok(big(0.75, 0)[1] > m(0.75, 0)[1]);
});

test("inflate swells the middle and deflate pinches it", () => {
  const inflate = warpDef("textInflate", undefined, W, H).mesh!;
  const h = (m: (u: number, v: number) => [number, number], u: number) => m(u, 1)[1] - m(u, 0)[1];
  assert.ok(h(inflate, 0.5) > h(inflate, 0) + 10);
  near(h(inflate, 0.5), H);
  const deflate = warpDef("textDeflate", undefined, W, H).mesh!;
  assert.ok(h(deflate, 0.5) < h(deflate, 0) - 10);
  near(h(deflate, 0), H);
  // Inflate bottom only moves the bottom edge; deflate top only the top.
  const ib = warpDef("textInflateBottom", undefined, W, H).mesh!;
  near(ib(0, 0)[1], 0);
  near(ib(0.5, 0)[1], 0);
  near(ib(0.5, 1)[1], H);
  near(ib(0, 1)[1], H * 0.6);
  const dt = warpDef("textDeflateTop", undefined, W, H).mesh!;
  near(dt(0.5, 1)[1], H);
  near(dt(0.5, 0)[1], H * 0.5);
  near(dt(0, 0)[1], 0);
});

test("slant, fade, triangle and chevron keep their straight edges", () => {
  const up = warpDef("textSlantUp", undefined, W, H).mesh!;
  // Higher at the right, and taller there with the default adjust.
  assert.ok(up(1, 0)[1] < up(0, 0)[1]);
  near(up(0, 0)[1], H * 0.55556);
  near(up(0, 1)[1], H);
  near(up(1, 0)[1], 0);
  near(up(1, 1)[1], H * 0.55556);
  const down = warpDef("textSlantDown", undefined, W, H).mesh!;
  near(down(0, 0)[1], 0);
  assert.ok(down(1, 0)[1] > down(0, 0)[1]);
  near(down(1, 1)[1], H);
  const fr = warpDef("textFadeRight", undefined, W, H).mesh!;
  near(fr(0, 0)[1], 0);
  near(fr(0, 1)[1], H);
  near(fr(1, 0)[1], H * 0.33333);
  near(fr(1, 1)[1], H * (1 - 0.33333));
  const fu = warpDef("textFadeUp", undefined, W, H).mesh!;
  // The top row is narrower than the bottom row.
  near(fu(0, 0)[0], W * 0.33333);
  near(fu(1, 0)[0], W * (1 - 0.33333));
  near(fu(0, 1)[0], 0);
  near(fu(1, 1)[0], W);
  const tri = warpDef("textTriangle", undefined, W, H).mesh!;
  near(tri(0.5, 0)[1], 0);
  near(tri(0, 0)[1], H / 2);
  near(tri(1, 1)[1], H);
  const apex = warpDef("textTriangle", { adj: 25000 }, W, H).mesh!;
  near(apex(0.25, 0)[1], 0);
  const chev = warpDef("textChevron", undefined, W, H).mesh!;
  near(chev(0.5, 0)[1], 0);
  near(chev(0, 0)[1], H * 0.25);
  near(chev(0, 1)[1] - chev(0, 0)[1], H * 0.75);
  near(chev(0.5, 1)[1], H * 0.75);
  const stop = warpDef("textStop", undefined, W, H).mesh!;
  near(stop(0, 0)[1], H * 0.25);
  near(stop(0.5, 0)[1], 0);
  near(stop(0, 1)[1], H * 0.75);
});

test("a slice's map fits the mesh in the middle of the slice", () => {
  const mesh = warpDef("textSlantUp", undefined, W, H).mesh!;
  const tw = 200;
  const th = 60;
  const count = 40;
  const list = slices(mesh, tw, th, count);
  assert.equal(list.length, count);
  near(list[0].x0, 0);
  near(list[count - 1].x1, tw);
  for (const s of list) {
    const [a, b, c, d, e, f] = s.matrix;
    // The middle of the slice's top and bottom edges go where the mesh puts them.
    const u = (s.x0 + s.x1) / 2 / tw;
    for (const v of [0, 0.5, 1]) {
      const x = (s.x0 + s.x1) / 2;
      const y = v * th;
      const [mx, my] = mesh(u, v);
      near(a * x + c * y + e, mx, 0.05);
      near(b * x + d * y + f, my, 0.05);
    }
  }
  // Plain text maps by a plain scale.
  const flat = slices(warpDef("textPlain", undefined, W, H).mesh!, tw, th, 8);
  for (const s of flat) {
    near(s.matrix[0], W / tw, 1e-4);
    near(s.matrix[3], H / th, 1e-4);
    near(s.matrix[1], 0, 1e-4);
    near(s.matrix[2], 0, 1e-4);
  }
});

test("slices are about four pixels wide", () => {
  assert.equal(sliceCount(10), 24);
  assert.equal(sliceCount(400), 100);
  assert.equal(sliceCount(5000), 160);
});

test("meshBounds is the box of the warped block", () => {
  const b = meshBounds(warpDef("textArchUpPour", undefined, W, H).mesh!);
  near(b.x0, 0);
  near(b.x1, W);
  near(b.y0, 0);
  near(b.y1, H);
});

test("text on an arc follows an ellipse inscribed in the box", () => {
  const up = followPath("textArchUp", undefined, W, 150, 8);
  // From the left, over the top, to the right; the letters face outward.
  assert.match(up.d, /^M[\d.]+ [\d.]+A[\d.]+ [\d.]+ 0 0 1 [\d.]+ [\d.]+$/);
  assert.equal(up.outward, true);
  assert.ok(up.fontSize > 1 && up.length > 100);
  const down = followPath("textArchDown", undefined, W, 150, 8);
  assert.equal(down.outward, false);
  assert.match(down.d, / 0 0 0 /, "arch down runs the other way round");
  const circle = followPath("textCircle", undefined, 200, 200, 8);
  assert.match(circle.d, /A[\d.]+ [\d.]+ 0 1 1 [\d.]+ [\d.]+A/, "a circle is two arcs");
  // Longer text gets a smaller size on the same path.
  const long = followPath("textArchUp", undefined, W, 150, 16);
  assert.ok(long.fontSize <= up.fontSize);
  // The text never takes more than a share of the room.
  const short = followPath("textCircle", undefined, 200, 200, 0.5);
  assert.ok(short.fontSize <= 100);
});
