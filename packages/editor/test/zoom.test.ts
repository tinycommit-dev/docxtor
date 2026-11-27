// The zoom numbers and the outline level of a paragraph. These need no browser.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { outlineLevel } from "../src/editor/outline-level.ts";
import { clampZoom, fitPageZoom, fitWidthZoom, pagesZoom, pinchZoom, stepZoom, wheelZoom, zoomLabel, ZOOM_MAX, ZOOM_MIN, ZOOM_PRESETS } from "../src/editor/zoom-math.ts";

test("a zoom factor stays between 10% and 500% and is a whole percent", () => {
  assert.equal(clampZoom(0.01), ZOOM_MIN);
  assert.equal(clampZoom(40), ZOOM_MAX);
  assert.equal(clampZoom(1.2345), 1.23);
  assert.equal(clampZoom(Number.NaN), 1);
  assert.equal(zoomLabel(1.256), "126%");
  assert.deepEqual(ZOOM_PRESETS, [0.5, 0.75, 1, 1.25, 1.5, 2]);
});

test("a step is ten points, snapped to a multiple of five", () => {
  assert.equal(stepZoom(1, 1), 1.1);
  assert.equal(stepZoom(1, -1), 0.9);
  assert.equal(stepZoom(0.28, 1), 0.4);
  assert.equal(stepZoom(0.28, -1), 0.2);
  assert.equal(stepZoom(ZOOM_MAX, 1), ZOOM_MAX);
  assert.equal(stepZoom(ZOOM_MIN, -1), ZOOM_MIN);
});

test("page width fills the window, and automatic stops at 100%", () => {
  assert.equal(fitWidthZoom(1632, 816), 2);
  assert.equal(fitWidthZoom(1000, 816), 1.22);
  assert.equal(fitWidthZoom(1632, 816, 1), 1);
  assert.equal(fitWidthZoom(408, 816, 1), 0.5);
  // Nothing to fit yet: leave it at 100%.
  assert.equal(fitWidthZoom(0, 816), 1);
  assert.equal(fitWidthZoom(800, 0), 1);
});

test("a whole page fits by the tighter of its width and its height", () => {
  // Letter, 816 by 1056: a wide window is limited by height, a narrow one by width.
  assert.equal(fitPageZoom(2000, 528, 816, 1056), 0.5);
  assert.equal(fitPageZoom(408, 2000, 816, 1056), 0.5);
  // The ruler is as tall as it is and scales with the page.
  assert.equal(fitPageZoom(2000, 540, 816, 1056, 24), 0.5);
  assert.equal(fitPageZoom(0, 500, 816, 1056), 1);
});

test("several pages side by side share the window with a gap between them", () => {
  assert.equal(pagesZoom(1648, 816, 2, 16), 1);
  assert.equal(pagesZoom(1648 + 16, 816, 2, 16), 1);
  assert.equal(pagesZoom(1200, 816, 3, 16), 0.47);
  assert.equal(pagesZoom(1000, 816, 1, 16), 1.22);
  assert.equal(pagesZoom(1000, 816, 0, 16), 1);
});

test("a wheel notch steps by ten points and a trackpad pinch zooms smoothly", () => {
  assert.equal(wheelZoom(1, -100), 1.1);
  assert.equal(wheelZoom(1, 100), 0.9);
  assert.equal(wheelZoom(1, -120, 0), 1.1);
  // Lines instead of pixels (a mouse in some browsers) count as a notch.
  assert.equal(wheelZoom(1, -3, 1), 1.1);
  const pinch = wheelZoom(1, -5);
  assert.ok(pinch > 1 && pinch < 1.15, String(pinch));
  assert.ok(wheelZoom(1, 5) < 1);
  assert.equal(wheelZoom(1, 0), 1);
  assert.equal(wheelZoom(ZOOM_MAX, -100), ZOOM_MAX);
});

test("two fingers scale the starting factor by how far they moved apart", () => {
  assert.equal(pinchZoom(1, 100, 200), 2);
  assert.equal(pinchZoom(1, 200, 100), 0.5);
  assert.equal(pinchZoom(2, 100, 400), ZOOM_MAX);
  assert.equal(pinchZoom(1, 0, 100), 1);
  assert.equal(pinchZoom(1, 100, 0), 1);
});

test("the outline level of a paragraph comes from its formatting, then from a Heading style", () => {
  const p = (style: string | undefined, outlineLvl?: number) => ({ style, fmt: { outlineLvl } }) as unknown as Parameters<typeof outlineLevel>[0];
  assert.equal(outlineLevel(p("Heading1", 0)), 1);
  assert.equal(outlineLevel(p("Heading3", 2)), 3);
  assert.equal(outlineLevel(p("MyHeading", 4)), 5);
  assert.equal(outlineLevel(p("Normal", 9)), 0);
  assert.equal(outlineLevel(p("Heading2", 9)), 0);
  assert.equal(outlineLevel(p("Heading2")), 2);
  assert.equal(outlineLevel(p("Heading9")), 9);
  assert.equal(outlineLevel(p("Heading10")), 0);
  assert.equal(outlineLevel(p("Title")), 0);
  assert.equal(outlineLevel(p(undefined)), 0);
});
