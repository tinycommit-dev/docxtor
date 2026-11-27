// Drawing tools: the four pens (pen, pencil, marker, highlighter) and what each writes, the two erasers, the lasso, and
// putting the result in the text as one undo step.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const PT_PX = 96 / 72;

interface Line {
  color: string;
  widthPt: number;
  alpha?: number;
  cap?: string;
}
interface Shape {
  geometry: string;
  wrap: string;
  line?: Line;
  custom?: { paths: unknown[] };
}

const atoms = (page: Page) => page.locator(".flow .atom.shape");
const layer = (page: Page) => page.locator(".ink-layer");
const bar = (page: Page) => page.locator(".ink-strip");
const strokes = (page: Page) => layer(page).locator("path.ink-stroke");
const shapeAt = (page: Page, p: number, o: number) =>
  inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape | null>;
const whereAtom = (page: Page, n: number) =>
  atoms(page)
    .nth(n)
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));

async function onSheet(page: Page, x: number, y: number) {
  const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
  const z = await inPage(page, (w) => w.fe.editor.zoom);
  return { x: sheet.x + x * PT_PX * z, y: sheet.y + y * PT_PX * z };
}

async function openDraw(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Draw below this line", "A second paragraph", "A third paragraph"] }] });
  await inPage(page, (w) => w.fe.editor.setZoom(1));
  await chooseMenu(page, "Insert", "Drawing", "Draw");
  await expect(layer(page)).toBeVisible();
}

/** A stroke from one point on the sheet to another (in points), with a little wave so it is more than a line. */
async function stroke(page: Page, x1: number, y1: number, x2: number, y2: number) {
  const a = await onSheet(page, x1, y1);
  const b = await onSheet(page, x2, y2);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  for (let i = 1; i <= 10; i++) await page.mouse.move(a.x + ((b.x - a.x) * i) / 10, a.y + ((b.y - a.y) * i) / 10 + Math.sin(i) * 3);
  await page.mouse.up();
}

const tool = (page: Page, name: string) => bar(page).getByRole("button", { name, exact: true });
const done = (page: Page) => bar(page).getByRole("button", { name: "Done", exact: true }).click();

/** A part of the saved file as text. */
async function savedDocument(page: Page): Promise<string> {
  return page.evaluate(async () => {
    const w = window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } } } } };
    const bytes = w.docxtor.fe.editor.engine.save();
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let end = bytes.length - 22;
    while (dv.getUint32(end, true) !== 0x06054b50) end--;
    let p = dv.getUint32(end + 16, true);
    const count = dv.getUint16(end + 10, true);
    for (let i = 0; i < count; i++) {
      const method = dv.getUint16(p + 10, true);
      const csize = dv.getUint32(p + 20, true);
      const nlen = dv.getUint16(p + 28, true);
      const xlen = dv.getUint16(p + 30, true);
      const clen = dv.getUint16(p + 32, true);
      const off = dv.getUint32(p + 42, true);
      const name = new TextDecoder().decode(bytes.subarray(p + 46, p + 46 + nlen));
      p += 46 + nlen + xlen + clen;
      if (name !== "word/document.xml") continue;
      const dataStart = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(dataStart, dataStart + csize);
      if (method === 0) return new TextDecoder().decode(data);
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return await new Response(stream).text();
    }
    return "";
  });
}

const attr = (el: ReturnType<Page["locator"]>, name: string) => el.getAttribute(name);

test.describe("pens", () => {
  const cases: { name: string; color: string; width: number; alpha?: number; cap?: string; xml: string[] }[] = [
    { name: "Pen", color: "000000", width: 2.25, xml: ['cap="rnd"', '<a:srgbClr val="000000"/>'] },
    { name: "Pencil", color: "404040", width: 1, alpha: 0.8, xml: ['cap="rnd"', '<a:srgbClr val="404040"><a:alpha val="80000"/></a:srgbClr>'] },
    { name: "Marker", color: "1A73E8", width: 4.5, xml: ['cap="rnd"', '<a:srgbClr val="1A73E8"/>'] },
    { name: "Highlighter", color: "FFEB3B", width: 15, alpha: 0.5, cap: "square", xml: ['cap="sq"', '<a:srgbClr val="FFEB3B"><a:alpha val="50000"/></a:srgbClr>', 'endcap="square"', 'opacity="0.5"'] },
  ];
  for (const c of cases) {
    test(`${c.name}: its width, colour, opacity and ends are in the model and the saved file`, async ({ page }) => {
      await openDraw(page);
      await tool(page, c.name).click();
      await expect(tool(page, c.name)).toHaveAttribute("aria-pressed", "true");
      await stroke(page, 120, 160, 300, 170);
      // What is drawn on the layer is already the pen's.
      const path = strokes(page).first();
      await expect(path).toHaveAttribute("stroke", `#${c.color}`);
      expect(Number(await attr(path, "stroke-width"))).toBeCloseTo(c.width * PT_PX, 1);
      if (c.alpha) expect(Number(await attr(path, "stroke-opacity"))).toBeCloseTo(c.alpha, 2);
      else expect(await attr(path, "stroke-opacity")).toBeNull();
      await done(page);
      await expect(atoms(page)).toHaveCount(1);
      const s = (await shapeAt(page, (await whereAtom(page, 0)).p, (await whereAtom(page, 0)).o))!;
      expect(s.geometry).toBe("custom");
      expect(s.wrap).toBe("front");
      expect(s.line!.color).toBe(c.color);
      expect(s.line!.widthPt).toBeCloseTo(c.width, 2);
      expect(s.line!.alpha).toBe(c.alpha);
      expect(s.line!.cap).toBe(c.cap ?? "round");
      const xml = await savedDocument(page);
      for (const needle of c.xml) expect(xml, needle).toContain(needle);
    });
  }

  test("the pen that was last used is the one in hand next time, with its colour and width", async ({ page }) => {
    await openDraw(page);
    await tool(page, "Highlighter").click();
    await bar(page).getByRole("button", { name: "#D93025" }).click();
    await bar(page).getByLabel("Pen width").selectOption("18");
    await stroke(page, 120, 160, 300, 160);
    await done(page);
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    await expect(tool(page, "Highlighter")).toHaveAttribute("aria-pressed", "true");
    await stroke(page, 120, 220, 300, 220);
    const path = strokes(page).first();
    await expect(path).toHaveAttribute("stroke", "#D93025");
    expect(Number(await attr(path, "stroke-width"))).toBeCloseTo(18 * PT_PX, 1);
    await page.keyboard.press("Escape");
  });

  test("a colour from the chip's picker is used, and shows among the colours to pick from next", async ({ page }) => {
    await openDraw(page);
    await tool(page, "Pen").click();
    await bar(page).getByRole("button", { name: "Pen color" }).click();
    const swatch = page.locator(".pop .swatch").nth(7);
    const hex = ((await swatch.getAttribute("title")) ?? "").slice(1).toUpperCase();
    await swatch.click();
    await stroke(page, 120, 160, 300, 160);
    await expect(strokes(page).first()).toHaveAttribute("stroke", `#${hex}`);
    // The colour just used is the first of the small row of colours.
    await expect(bar(page).locator(".ink-swatches .swatch").first()).toHaveAttribute("title", `#${hex}`);
    await expect(bar(page).locator(".ink-swatches .swatch").first()).toHaveClass(/\bon\b/);
  });

  test("strokes with different pens go in as separate drawings, and one undo takes them all out", async ({ page }) => {
    await openDraw(page);
    await tool(page, "Pen").click();
    await stroke(page, 120, 160, 300, 160);
    await tool(page, "Highlighter").click();
    await stroke(page, 120, 200, 300, 200);
    await tool(page, "Pen").click();
    await stroke(page, 120, 240, 300, 240);
    await expect(strokes(page)).toHaveCount(3);
    await done(page);
    // Two pens, two drawings: the pen's two strokes in one, the highlighter's in another that sits above it.
    await expect(atoms(page)).toHaveCount(2);
    const first = await shapeAt(page, (await whereAtom(page, 0)).p, (await whereAtom(page, 0)).o);
    const second = await shapeAt(page, (await whereAtom(page, 1)).p, (await whereAtom(page, 1)).o);
    const byAlpha = [first!, second!].sort((a, b) => (a.line!.alpha ?? 1) - (b.line!.alpha ?? 1));
    expect(byAlpha[0].line!.alpha).toBe(0.5);
    expect(byAlpha[0].custom!.paths).toHaveLength(1);
    expect(byAlpha[1].custom!.paths).toHaveLength(2);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect(atoms(page)).toHaveCount(2);
  });
});

test.describe("erasers", () => {
  test("the stroke eraser takes the whole stroke it touches, and Undo stroke brings it back", async ({ page }) => {
    await openDraw(page);
    await stroke(page, 120, 160, 300, 160);
    await stroke(page, 120, 220, 300, 220);
    await expect(strokes(page)).toHaveCount(2);
    await tool(page, "Stroke eraser").click();
    await expect(tool(page, "Stroke eraser")).toHaveAttribute("aria-pressed", "true");
    // A circle follows the pointer.
    const at = await onSheet(page, 200, 160);
    await page.mouse.move(at.x, at.y);
    await expect(page.locator(".ink-eraser")).toBeVisible();
    await page.mouse.down();
    await page.mouse.move(at.x + 4, at.y + 2);
    await page.mouse.up();
    await expect(strokes(page)).toHaveCount(1);
    await bar(page).getByRole("button", { name: "Undo stroke" }).click();
    await expect(strokes(page)).toHaveCount(2);
    // Rubbing across both takes both in one go; one undo brings both back.
    await page.mouse.move(at.x, at.y - 10);
    await page.mouse.down();
    await page.mouse.move(at.x, at.y + 100, { steps: 12 });
    await page.mouse.up();
    await expect(strokes(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(strokes(page)).toHaveCount(2);
  });

  test("the point eraser takes the part under its circle and splits the stroke in two", async ({ page }) => {
    await openDraw(page);
    await stroke(page, 120, 200, 360, 200);
    await expect(strokes(page)).toHaveCount(1);
    await tool(page, "Point eraser").click();
    await bar(page).getByLabel("Eraser size").selectOption({ label: "Large eraser" });
    const mid = await onSheet(page, 240, 201);
    await page.mouse.move(mid.x, mid.y);
    await expect(page.locator(".ink-eraser")).toBeVisible();
    // The circle is the size asked for: 18 points across each way, 36 in all.
    const box = (await page.locator(".ink-eraser").boundingBox())!;
    expect(box.width).toBeCloseTo(36 * PT_PX, 0);
    await page.mouse.down();
    await page.mouse.move(mid.x + 2, mid.y);
    await page.mouse.up();
    await expect(strokes(page)).toHaveCount(2);
    // Both pieces keep the pen, and there is a gap in the middle.
    const parts = await strokes(page).evaluateAll((els) =>
      els.map((e) => {
        const b = (e as SVGGraphicsElement).getBBox();
        return { stroke: e.getAttribute("stroke"), width: e.getAttribute("stroke-width"), left: b.x, right: b.x + b.width };
      }),
    );
    expect(parts[0].stroke).toBe(parts[1].stroke);
    expect(parts[0].width).toBe(parts[1].width);
    const [a, b] = parts[0].left < parts[1].left ? parts : [parts[1], parts[0]];
    // 18 points each way is 24 pixels, less the pen's own half width.
    expect(b.left - a.right).toBeGreaterThan(30);
    // The two pieces go in the document as two paths of one drawing.
    await done(page);
    const s = (await shapeAt(page, (await whereAtom(page, 0)).p, (await whereAtom(page, 0)).o))!;
    expect(s.custom!.paths).toHaveLength(2);
  });

  test("rubbing out a whole short stroke takes it away, and an eraser that touches nothing leaves nothing to undo", async ({ page }) => {
    await openDraw(page);
    await stroke(page, 120, 160, 150, 160);
    await tool(page, "Point eraser").click();
    await bar(page).getByLabel("Eraser size").selectOption({ label: "Large eraser" });
    const away = await onSheet(page, 300, 300);
    await page.mouse.move(away.x, away.y);
    await page.mouse.down();
    await page.mouse.up();
    await expect(strokes(page)).toHaveCount(1);
    const on = await onSheet(page, 135, 160);
    await page.mouse.move(on.x, on.y);
    await page.mouse.down();
    await page.mouse.move(on.x + 3, on.y, { steps: 3 });
    await page.mouse.up();
    await expect(strokes(page)).toHaveCount(0);
    // Back to the pen: drawing goes on.
    await tool(page, "Pen").click();
    await stroke(page, 120, 160, 200, 160);
    await expect(strokes(page)).toHaveCount(1);
  });
});

test.describe("lasso", () => {
  /** Three strokes in a column, each its own colour (the first is the pen's black, then red, then blue). */
  async function three(page: Page) {
    await openDraw(page);
    await stroke(page, 120, 160, 300, 160);
    await bar(page).getByRole("button", { name: "#D93025" }).click();
    await stroke(page, 120, 200, 300, 200);
    await bar(page).getByRole("button", { name: "#1A73E8" }).click();
    await stroke(page, 120, 240, 300, 240);
    await expect(strokes(page)).toHaveCount(3);
  }

  /** A loop round the middle and the last stroke. */
  async function loopRound(page: Page) {
    await tool(page, "Lasso select").click();
    const pts = [
      [100, 185],
      [320, 185],
      [320, 270],
      [100, 270],
      [100, 186],
    ];
    const first = await onSheet(page, pts[0][0], pts[0][1]);
    await page.mouse.move(first.x, first.y);
    await page.mouse.down();
    for (const [x, y] of pts.slice(1)) {
      const p = await onSheet(page, x, y);
      await page.mouse.move(p.x, p.y, { steps: 4 });
    }
    await page.mouse.up();
  }

  test("a loop selects the strokes it encloses and shows a box round them", async ({ page }) => {
    await three(page);
    await loopRound(page);
    await expect(layer(page).locator(".ink-sel")).toHaveCount(2);
    await expect(layer(page).locator(".ink-sel-box")).toHaveCount(1);
    await expect(bar(page).getByText("2 strokes selected")).toBeVisible();
    // A loop that only crosses a stroke catches it too.
    await page.mouse.click((await onSheet(page, 400, 400)).x, (await onSheet(page, 400, 400)).y);
    await expect(layer(page).locator(".ink-sel")).toHaveCount(0);
    const a = await onSheet(page, 200, 150);
    const b = await onSheet(page, 210, 170);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(b.x, a.y, { steps: 3 });
    await page.mouse.move(b.x, b.y, { steps: 3 });
    await page.mouse.move(a.x, b.y, { steps: 3 });
    await page.mouse.move(a.x, a.y - 1, { steps: 3 });
    await page.mouse.up();
    await expect(layer(page).locator(".ink-sel")).toHaveCount(1);
  });

  test("dragging inside the box moves the held strokes and no others", async ({ page }) => {
    await three(page);
    const before = await strokes(page).evaluateAll((els) => els.map((e) => (e as SVGGraphicsElement).getBBox().x));
    await loopRound(page);
    const from = await onSheet(page, 200, 220);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(from.x + 60, from.y + 30, { steps: 6 });
    await page.mouse.up();
    const after = await strokes(page).evaluateAll((els) => els.map((e) => (e as SVGGraphicsElement).getBBox().x));
    expect(after[0]).toBeCloseTo(before[0], 0);
    expect(after[1] - before[1]).toBeCloseTo(60, 0);
    expect(after[2] - before[2]).toBeCloseTo(60, 0);
    // Still held after the move; one Undo stroke takes the move back.
    await expect(layer(page).locator(".ink-sel")).toHaveCount(2);
    await bar(page).getByRole("button", { name: "Undo stroke" }).click();
    const back = await strokes(page).evaluateAll((els) => els.map((e) => (e as SVGGraphicsElement).getBBox().x));
    expect(back[1]).toBeCloseTo(before[1], 0);
  });

  test("a colour from the strip recolours the held strokes", async ({ page }) => {
    await three(page);
    await loopRound(page);
    await bar(page).getByRole("button", { name: "#188038" }).click();
    const colors = await strokes(page).evaluateAll((els) => els.map((e) => e.getAttribute("stroke")));
    expect(colors).toEqual(["#000000", "#188038", "#188038"]);
    // Both strokes now share the colour, and the swatch for it is the one that is on.
    await expect(bar(page).locator('.ink-swatches .swatch[title="#188038"]')).toHaveClass(/\bon\b/);
    await done(page);
    // The black stroke and the two green ones are two pens, so two drawings.
    await expect(atoms(page)).toHaveCount(2);
  });

  test("Delete takes the held strokes away, Escape lets go of them first, and one undo brings them back", async ({ page }) => {
    await three(page);
    await loopRound(page);
    await page.keyboard.press("Escape");
    await expect(layer(page).locator(".ink-sel")).toHaveCount(0);
    await expect(layer(page)).toBeVisible();
    await loopRound(page);
    await page.keyboard.press("Delete");
    await expect(strokes(page)).toHaveCount(1);
    await expect(layer(page).locator(".ink-sel-box")).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(strokes(page)).toHaveCount(3);
  });

  test("clicking elsewhere lets go of the selection", async ({ page }) => {
    await three(page);
    await loopRound(page);
    await expect(layer(page).locator(".ink-sel")).toHaveCount(2);
    const away = await onSheet(page, 450, 120);
    await page.mouse.click(away.x, away.y);
    await expect(layer(page).locator(".ink-sel")).toHaveCount(0);
    await expect(strokes(page)).toHaveCount(3);
  });
});

test.describe("going back to a drawing", () => {
  /** A highlighter stroke put in the text and left selected. */
  async function drawn(page: Page) {
    await openDraw(page);
    await tool(page, "Highlighter").click();
    await stroke(page, 120, 200, 300, 210);
    await done(page);
    await expect(atoms(page)).toHaveCount(1);
    await expect(page.locator(".obj-frame").first()).toBeVisible();
  }

  test("Draw on a selected drawing brings its strokes back, with their pen", async ({ page }) => {
    await drawn(page);
    const before = (await atoms(page).first().boundingBox())!;
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    await expect(strokes(page)).toHaveCount(1);
    const path = strokes(page).first();
    await expect(path).toHaveAttribute("stroke", "#FFEB3B");
    expect(Number(await attr(path, "stroke-width"))).toBeCloseTo(15 * PT_PX, 1);
    expect(Number(await attr(path, "stroke-opacity"))).toBeCloseTo(0.5, 2);
    // The stroke is drawn where the shape was, and the shape itself is out of the way while it is on the layer.
    // (The path's own box, from its geometry: Firefox's `getBoundingClientRect` of a stroked path takes in the pen's width, Chrome's does not.)
    const box = await path.evaluate((e) => {
      const b = (e as unknown as SVGGraphicsElement).getBBox();
      const m = (e as unknown as SVGGraphicsElement).getScreenCTM()!;
      return { x: m.e + b.x * m.a, y: m.f + b.y * m.d, w: b.width * m.a };
    });
    // The shape's box runs half the pen's width (7.5 points, 10 pixels) beyond the line.
    expect(Math.abs(box.x - (before.x + 7.5 * PT_PX))).toBeLessThan(2);
    await expect(atoms(page).first()).toHaveCSS("visibility", "hidden");
    await page.keyboard.press("Escape");
    await expect(layer(page)).toHaveCount(0);
    // Cancelling leaves the drawing as it was.
    await expect(atoms(page)).toHaveCount(1);
    await expect(atoms(page).first()).toHaveCSS("visibility", "visible");
  });

  test("adding a stroke with another pen makes two drawings in place of one, and one undo gives the first back", async ({ page }) => {
    await drawn(page);
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    await tool(page, "Pen").click();
    await stroke(page, 120, 260, 300, 260);
    await expect(strokes(page)).toHaveCount(2);
    await done(page);
    await expect(atoms(page)).toHaveCount(2);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(1);
    const at = await whereAtom(page, 0);
    expect((await shapeAt(page, at.p, at.o))!.line!.alpha).toBe(0.5);
  });

  test("rubbing out the whole drawing and finishing removes it", async ({ page }) => {
    await drawn(page);
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    await tool(page, "Stroke eraser").click();
    const on = await onSheet(page, 200, 205);
    await page.mouse.move(on.x, on.y);
    await page.mouse.down();
    await page.mouse.move(on.x + 3, on.y, { steps: 3 });
    await page.mouse.up();
    await expect(strokes(page)).toHaveCount(0);
    await done(page);
    await expect(atoms(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(1);
  });

  test("a shape that is not a drawing starts a fresh one", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Some words", "More words"] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_shape", at: { p: 1, o: 0 }, geometry: "ellipse", w: 60, h: 40 }));
    await atoms(page).first().click();
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    await expect(layer(page)).toBeVisible();
    await expect(strokes(page)).toHaveCount(0);
    await page.keyboard.press("Escape");
  });
});

test.describe("looks", () => {
  for (const scheme of ["light", "dark"] as const) {
    test(`the strip in ${scheme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await openDraw(page);
      await tool(page, "Highlighter").click();
      await stroke(page, 120, 160, 360, 160);
      await tool(page, "Marker").click();
      await stroke(page, 120, 220, 360, 230);
      await tool(page, "Lasso select").click();
      const pts: [number, number][] = [
        [100, 140],
        [380, 140],
        [380, 180],
        [100, 180],
        [100, 141],
      ];
      const first = await onSheet(page, ...pts[0]);
      await page.mouse.move(first.x, first.y);
      await page.mouse.down();
      for (const [x, y] of pts.slice(1)) {
        const p = await onSheet(page, x, y);
        await page.mouse.move(p.x, p.y, { steps: 4 });
      }
      await page.mouse.up();
      await page.screenshot({ path: `test-results/ink-strip-${scheme}.png` });
      await expect(bar(page)).toBeVisible();
    });
  }
});
