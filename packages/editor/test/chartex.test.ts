// The layout maths of the new chart types: treemap rectangles, sunburst arcs, bin labels, bar slots, label fit.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { TreeNode } from "@docxtor/core";
import { arcLabel, arcPath, barSlot, binLabel, countTicks, squarify, sunburstLayout, textWidth, tint, tintHex, treeDepth, treemapLayout, wantsLightText, waterfallRange } from "../src/chart/chartex-math.ts";

const area = (r: { w: number; h: number }) => r.w * r.h;
const near = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);

const TREE: TreeNode = {
  name: "",
  value: 15,
  children: [
    {
      name: "Fruit",
      value: 8,
      children: [
        { name: "Apple", value: 4 },
        { name: "Pear", value: 3 },
        { name: "Plum", value: 1 },
      ],
    },
    {
      name: "Veg",
      value: 7,
      children: [
        { name: "Kale", value: 2 },
        { name: "Leek", value: 5 },
      ],
    },
  ],
};

test("squarified rectangles fill the box and each area follows its value", () => {
  const values = [6, 6, 4, 3, 2, 2, 1];
  const box = { x: 10, y: 20, w: 300, h: 200 };
  const rects = squarify(values, box);
  const total = values.reduce((a, b) => a + b, 0);
  near(
    rects.reduce((a, r) => a + area(r), 0),
    area(box),
    1e-6,
  );
  rects.forEach((r, i) => near(area(r) / area(box), values[i]! / total, 1e-9));
  // Everything stays inside the box and nothing overlaps.
  for (const r of rects) {
    assert.ok(r.x >= box.x - 1e-9 && r.y >= box.y - 1e-9 && r.x + r.w <= box.x + box.w + 1e-9 && r.y + r.h <= box.y + box.h + 1e-9);
  }
  for (let i = 0; i < rects.length; i++)
    for (let j = i + 1; j < rects.length; j++) {
      const a = rects[i]!;
      const b = rects[j]!;
      const overlapW = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
      const overlapH = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
      assert.ok(overlapW <= 1e-6 || overlapH <= 1e-6, `${i} and ${j} overlap`);
    }
});

test("squarified rectangles stay near squares", () => {
  const rects = squarify([5, 5, 5, 5, 5, 5], { x: 0, y: 0, w: 300, h: 200 });
  for (const r of rects) assert.ok(Math.max(r.w / r.h, r.h / r.w) < 2, `${r.w} x ${r.h}`);
});

test("a value of zero or less gets no room", () => {
  const rects = squarify([4, 0, -3, 4], { x: 0, y: 0, w: 100, h: 100 });
  assert.equal(area(rects[1]!), 0);
  assert.equal(area(rects[2]!), 0);
  near(area(rects[0]!) + area(rects[3]!), 10000);
  assert.deepEqual(
    squarify([1], { x: 0, y: 0, w: 0, h: 10 }).map(area),
    [0],
  );
});

test("a treemap puts the leaves of each branch inside the branch, largest first", () => {
  const box = { x: 0, y: 0, w: 400, h: 300 };
  const tiles = treemapLayout(TREE, box, { parentLabels: "none", banner: 0, gap: 0 });
  const leaves = tiles.filter((t) => t.leaf);
  assert.equal(leaves.length, 5);
  // With no gaps and no banners the leaves fill the box exactly, each in proportion to its value.
  near(
    leaves.reduce((a, t) => a + area(t.rect), 0),
    area(box),
    1e-6,
  );
  for (const l of leaves) near(area(l.rect) / area(box), l.node.value / 15, 1e-9);
  // A branch's tile holds exactly its leaves' tiles.
  for (const parent of tiles.filter((t) => !t.leaf)) {
    const kids = leaves.filter((l) => l.path[0] === parent.node.name);
    near(
      kids.reduce((a, t) => a + area(t.rect), 0),
      area(parent.rect),
      1e-6,
    );
    for (const k of kids) assert.ok(k.rect.x >= parent.rect.x - 1e-9 && k.rect.y >= parent.rect.y - 1e-9 && k.rect.x + k.rect.w <= parent.rect.x + parent.rect.w + 1e-9 && k.rect.y + k.rect.h <= parent.rect.y + parent.rect.h + 1e-9);
  }
  // Fruit (8) is bigger than Veg (7).
  const byName = new Map(tiles.map((t) => [t.node.name, t]));
  assert.ok(area(byName.get("Fruit")!.rect) > area(byName.get("Veg")!.rect));
  assert.equal(byName.get("Apple")!.branch, 0);
  assert.equal(byName.get("Leek")!.branch, 1);
  assert.deepEqual(byName.get("Leek")!.path, ["Veg", "Leek"]);
  assert.equal(byName.get("Leek")!.depth, 2);
});

test("a banner takes a strip of its branch for its name", () => {
  const box = { x: 0, y: 0, w: 400, h: 300 };
  const tiles = treemapLayout(TREE, box, { parentLabels: "banner", banner: 18, gap: 2 });
  const fruit = tiles.find((t) => t.node.name === "Fruit")!;
  assert.equal(fruit.label!.h, 18);
  assert.equal(fruit.label!.y, fruit.rect.y + 2);
  const apple = tiles.find((t) => t.node.name === "Apple")!;
  // The leaves sit under the banner.
  assert.ok(apple.rect.y >= fruit.label!.y + 18 - 1e-9);
  const none = treemapLayout(TREE, box, { parentLabels: "none", banner: 18, gap: 2 });
  assert.equal(none.find((t) => t.node.name === "Fruit")!.label, null);
});

test("nodes without a value are left out", () => {
  const tree: TreeNode = { name: "", value: 3, children: [{ name: "a", value: 3 }, { name: "b", value: 0 }] };
  const tiles = treemapLayout(tree, { x: 0, y: 0, w: 100, h: 100 }, { parentLabels: "none", banner: 0, gap: 0 });
  assert.deepEqual(
    tiles.map((t) => t.node.name),
    ["a"],
  );
});

test("a sunburst gives each node an arc in proportion to its value inside its parent's", () => {
  const arcs = sunburstLayout(TREE, 100, 0);
  assert.equal(treeDepth(TREE), 2);
  const by = new Map(arcs.map((a) => [a.node.name, a]));
  const sweep = (name: string) => by.get(name)!.a1 - by.get(name)!.a0;
  near(sweep("Fruit"), (Math.PI * 2 * 8) / 15);
  near(sweep("Veg"), (Math.PI * 2 * 7) / 15);
  near(sweep("Fruit") + sweep("Veg"), Math.PI * 2);
  near(sweep("Apple") + sweep("Pear") + sweep("Plum"), sweep("Fruit"));
  near(sweep("Kale") + sweep("Leek"), sweep("Veg"));
  near(sweep("Apple") / sweep("Fruit"), 4 / 8);
  // Children sit inside their parent's angles, one ring further out.
  for (const leaf of ["Apple", "Pear", "Plum"]) {
    const a = by.get(leaf)!;
    assert.ok(a.a0 >= by.get("Fruit")!.a0 - 1e-9 && a.a1 <= by.get("Fruit")!.a1 + 1e-9);
    assert.equal(a.depth, 2);
    assert.equal(a.r0, by.get("Fruit")!.r1);
  }
  assert.equal(by.get("Fruit")!.r0, 0);
  assert.equal(by.get("Leek")!.r1, 100);
  // The first arc starts at the angle asked for, the arcs run clockwise without gaps.
  assert.equal(by.get("Fruit")!.a0, 0);
  near(by.get("Veg")!.a0, by.get("Fruit")!.a1);
  assert.equal(by.get("Veg")!.branch, 1);
  assert.equal(by.get("Kale")!.branch, 1);
});

test("a parent with a value of its own leaves part of its arc empty", () => {
  const tree: TreeNode = { name: "", value: 10, children: [{ name: "p", value: 10, children: [{ name: "c", value: 5 }] }] };
  const arcs = sunburstLayout(tree, 50, 0);
  const p = arcs.find((a) => a.node.name === "p")!;
  const c = arcs.find((a) => a.node.name === "c")!;
  near(p.a1 - p.a0, Math.PI * 2);
  near(c.a1 - c.a0, Math.PI);
});

test("arc paths close, and a whole circle is two arcs", () => {
  const d = arcPath(100, 100, 20, 50, 0, Math.PI / 2);
  assert.match(d, /^M[\d. -]+A50 50 0 0 1 [\d. -]+L[\d. -]+A20 20 0 0 0 [\d. -]+Z$/);
  const pie = arcPath(100, 100, 0, 50, 0, 1);
  assert.match(pie, /^M100 100L/);
  const full = arcPath(100, 100, 0, 50, 0, Math.PI * 2);
  assert.equal((full.match(/A/g) ?? []).length, 2);
  const big = arcPath(100, 100, 0, 50, 0, Math.PI * 1.5);
  assert.match(big, /A50 50 0 1 1/);
});

test("an arc is named along its ring when long enough, along its radius when thick enough, else not at all", () => {
  const wide = { node: { name: "x", value: 1 }, depth: 1, a0: 0, a1: 3, r0: 0, r1: 40, branch: 0, path: ["x"] };
  const label = arcLabel(wide, 100, 100, 12, "Fruit");
  assert.ok(label);
  assert.equal(label!.text, "Fruit");
  const thin = { ...wide, a0: 0, a1: 0.02, r0: 20, r1: 30 };
  assert.equal(arcLabel(thin, 100, 100, 12, "Fruit"), null);
  // A thick, narrow arc is named along its radius, read from the left side so it is not upside down.
  const spoke = { ...wide, a0: Math.PI - 0.2, a1: Math.PI + 0.2, r0: 10, r1: 90 };
  const l = arcLabel(spoke, 100, 100, 12, "Fruit")!;
  assert.ok(l);
  near(((l.rotate % 360) + 360) % 360, 0, 1e-6);
  // Never upside down.
  for (let a = 0; a < Math.PI * 2; a += 0.3) {
    const r = arcLabel({ ...wide, a0: a, a1: a + 0.9, r0: 10, r1: 80 }, 0, 0, 12, "Ab");
    if (!r) continue;
    const deg = ((r.rotate % 360) + 360) % 360;
    assert.ok(deg <= 90 + 1e-6 || deg >= 270 - 1e-6, `${deg}`);
  }
});

test("histogram bars are labelled the way Excel writes bins", () => {
  const bin = { count: 1, closedLo: true, closedHi: true, kind: "bin" as const, lo: 1, hi: 6 };
  assert.equal(binLabel(bin), "[1, 6]");
  assert.equal(binLabel({ ...bin, closedLo: false, lo: 6, hi: 11 }), "(6, 11]");
  assert.equal(binLabel({ ...bin, closedHi: false, closedLo: true }), "[1, 6)");
  assert.equal(binLabel({ ...bin, kind: "underflow", hi: 3, lo: undefined }), "≤3");
  assert.equal(binLabel({ ...bin, kind: "overflow", lo: 12, hi: undefined }), ">12");
  assert.equal(binLabel({ ...bin, kind: "category", label: "Late" }), "Late");
  assert.equal(binLabel({ ...bin, lo: 0.5, hi: 1.25 }, "0.00"), "[0.50, 1.25]");
});

test("a bar sits in the middle of its slot with the gap shared on both sides", () => {
  const s = barSlot(100, 0.5);
  near(s.width, 100 / 1.5);
  near(s.offset, (100 - 100 / 1.5) / 2);
  const none = barSlot(80, 0);
  assert.deepEqual(none, { offset: 0, width: 80 });
  const two = barSlot(100, 1, 2);
  near(two.width, 25);
  near(two.offset, 25);
});

test("count axes use whole numbers for small counts", () => {
  const small = countTicks(3);
  assert.deepEqual(small.values, [0, 1, 2, 3]);
  const big = countTicks(42);
  assert.equal(big.min, 0);
  assert.ok(big.max >= 42);
  assert.deepEqual(countTicks(0).values, [0, 1]);
  assert.equal(countTicks(9, { max: 20 }).max, 20);
});

test("a waterfall axis covers every bar and zero", () => {
  assert.deepEqual(
    waterfallRange([
      { label: "a", value: 100, start: 0, end: 100, kind: "total" },
      { label: "b", value: -130, start: 100, end: -30, kind: "decrease" },
    ]),
    [-30, 100],
  );
  assert.deepEqual(
    waterfallRange([{ label: "a", value: 5, start: 50, end: 55, kind: "increase" }]),
    [0, 55],
  );
});

test("text colour follows the fill", () => {
  assert.equal(wantsLightText("4472c4"), true);
  assert.equal(wantsLightText("ffc000"), false);
  assert.equal(wantsLightText("a5a5a5"), false);
  assert.equal(wantsLightText("not a colour"), false);
  assert.equal(tint("#4472c4", 0), "#4472c4");
  assert.match(tint("var(--chart-1)", 1), /color-mix\(in srgb, var\(--chart-1\) 78%, white\)/);
  assert.equal(tintHex("000000", 0), "000000");
  // Lighter as it goes deeper.
  assert.ok(parseInt(tintHex("000000", 2), 16) > parseInt(tintHex("000000", 1), 16));
  assert.ok(textWidth("Fruit", 12) > textWidth("i", 12));
});
