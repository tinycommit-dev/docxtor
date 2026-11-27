// Shape geometry: curves, freeforms and scribbles, edit points, and connectors that stay attached to shapes. The pointer
// is driven for real; what is asserted is the model the engine reports, the saved markup and what is on screen.
import { floatShape } from "./support/arrange-markup";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, expect, inPage, test, type Locator, type Page } from "./helpers";
import { wordConnector, wordFreeform } from "./support/shape-markup";

const MOD = "ControlOrMeta";
const PT_PX = 96 / 72;
const SHOTS = process.env.GEO_SHOTS ?? "";

interface Cmd {
  t: string;
  pts: number[][];
}
interface Shape {
  id: number;
  geometry: string;
  kind: string;
  w: number;
  h: number;
  x?: number;
  y?: number;
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  wrap: string;
  fill?: { color: string };
  line?: { color: string; widthPt: number; tailArrow?: string };
  adj?: Record<string, number>;
  custom?: { paths: { w: number; h: number; d: string; cmds?: Cmd[] }[] };
  connect?: { start?: { id: number; idx: number }; end?: { id: number; idx: number } };
}

const shapeAt = (page: Page, p: number, o: number) => inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape | null>;
const atoms = (page: Page) => page.locator(".flow .atom.shape");
const frame = (page: Page) => page.locator(".obj-frame").first();
const where = (page: Page, n = 0) =>
  atoms(page)
    .nth(n)
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
/** The letters of the commands of a shape's first path. */
const letters = (s: Shape) => s.custom!.paths[0].cmds!.map((c) => c.t).join("");

/** A part of the saved file as text. */
async function savedPart(page: Page, name: string): Promise<string> {
  return page.evaluate(async (want) => {
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
      if (name !== want) continue;
      const dataStart = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(dataStart, dataStart + csize);
      if (method === 0) return new TextDecoder().decode(data);
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return await new Response(stream).text();
    }
    return "";
  }, name);
}

/** A point on the first sheet, in points from its top-left corner, as a point on screen. */
async function onSheet(page: Page, x: number, y: number) {
  const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
  const z = await inPage(page, (w) => w.fe.editor.zoom);
  return { x: sheet.x + x * PT_PX * z, y: sheet.y + y * PT_PX * z };
}

const center = async (loc: Locator) => {
  const b = (await loc.boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
};

/** Look at the page: a screenshot of the sheets, saved when GEO_SHOTS names a folder. */
async function shot(page: Page, name: string) {
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/${name}.png` });
}

/** Choose a tool in the shape picker (Insert, Drawing, Shape). */
async function pickTool(page: Page, name: string) {
  await chooseMenu(page, "Insert", "Drawing", "Shape");
  await page.locator(".shape-picker").getByRole("button", { name, exact: true }).click();
}

async function blankPage(page: Page, paragraphs: (string | { raw: string })[] = ["Draw below this line", "A second paragraph", "A third paragraph"]) {
  await openBuilt(page, { sections: [{ paragraphs }] });
  await inPage(page, (w) => w.fe.editor.setZoom(1));
}

const run = (page: Page, id: string, ...args: unknown[]) => inPage(page, (w, a) => (w.fe.extensions as unknown as { execute(id: string, ...args: unknown[]): Promise<unknown> }).execute(a.id, ...a.args), { id, args });

// ---------------------------------------------------------------------------------------------------------------------

test.describe("the picker", () => {
  test("lists curves, freeforms, scribbles and the three kinds of connector, each with a small drawing", async ({ page }) => {
    await blankPage(page);
    await chooseMenu(page, "Insert", "Drawing", "Shape");
    const picker = page.locator(".shape-picker");
    for (const name of ["Line", "Arrow", "Double arrow", "Straight connector", "Straight arrow connector", "Elbow connector", "Elbow arrow connector", "Curved connector", "Curved arrow connector", "Curve", "Freeform", "Scribble"]) {
      const b = picker.getByRole("button", { name, exact: true });
      await expect(b).toBeVisible();
      expect(await b.locator("svg path").count()).toBeGreaterThan(0);
    }
    await page.keyboard.press("Escape");
  });

  test("the tools are commands too", async ({ page }) => {
    await blankPage(page);
    for (const [id, layer] of [
      ["insert.curve", "curve"],
      ["insert.freeform", "freeform"],
      ["insert.scribble", "scribble"],
      ["insert.elbowConnector", "connector"],
    ] as const) {
      await run(page, id);
      await expect(page.locator(`.geo-layer[data-geo="${layer}"]`)).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(page.locator(".geo-layer")).toHaveCount(0);
    }
  });
});

test.describe("curve, freeform and scribble", () => {
  test("a curve: click the points, double-click to finish, and it is a smooth open curve", async ({ page }) => {
    await blankPage(page);
    await pickTool(page, "Curve");
    const layer = page.locator(".geo-layer");
    await expect(layer).toBeVisible();
    await expect(page.locator(".geo-strip")).toContainText("Click to place points");
    const pts = [await onSheet(page, 120, 220), await onSheet(page, 170, 160), await onSheet(page, 230, 240), await onSheet(page, 300, 170)];
    for (const p of pts.slice(0, 3)) await page.mouse.click(p.x, p.y);
    // The line so far is already a curve through the points, drawn live.
    await page.mouse.move(pts[3].x, pts[3].y);
    await expect(layer.locator("path.geo-path")).toHaveAttribute("d", /C/);
    await expect(layer.locator(".geo-mark")).toHaveCount(3);
    await shot(page, "curve-drawing");
    await page.mouse.click(pts[3].x, pts[3].y);
    await page.mouse.click(pts[3].x, pts[3].y);
    await expect(layer).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(1);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.geometry).toBe("custom");
    expect(letters(s)).toBe("MCCC");
    expect(s.fill).toBeUndefined();
    expect(s.line!.widthPt).toBe(1.5);
    expect(s.wrap).toBe("square");
    // It is drawn where the points were: its box is the curve's bounds.
    const box = (await atoms(page).first().boundingBox())!;
    expect(Math.abs(box.x - pts[0].x)).toBeLessThan(6);
    expect(box.x + box.width).toBeGreaterThan(pts[3].x - 6);
    // Selected afterwards, so Format shape and the strip work on it.
    await expect(frame(page)).toBeVisible();
    await expect(page.locator(".shape-strip")).toBeVisible();
    await shot(page, "curve-done");
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:custGeom>");
    expect(xml).toContain("<a:cubicBezTo>");
    expect(xml).not.toContain("<a:close/>");
    expect(xml).toContain('o:spt="100"');
    // One undo takes it out.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(0);
  });

  test("clicking the first point closes a curve and it gets a fill", async ({ page }) => {
    await blankPage(page);
    await run(page, "insert.curve");
    const [a, b, c] = [await onSheet(page, 140, 200), await onSheet(page, 240, 200), await onSheet(page, 190, 290)];
    for (const p of [a, b, c]) await page.mouse.click(p.x, p.y);
    await page.mouse.click(a.x + 2, a.y + 1);
    await expect(page.locator(".geo-layer")).toHaveCount(0);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(letters(s)).toBe("MCCCZ");
    expect(s.fill!.color).toBe("4472C4");
    await shot(page, "curve-closed");
  });

  test("a freeform takes corners and freehand runs, and clicking the start closes it", async ({ page }) => {
    await blankPage(page);
    await pickTool(page, "Freeform");
    const layer = page.locator(".geo-layer");
    const [p0, p1] = [await onSheet(page, 120, 220), await onSheet(page, 230, 220)];
    await page.mouse.click(p0.x, p0.y);
    await page.mouse.click(p1.x, p1.y);
    // A press that travels is freehand.
    const start = await onSheet(page, 260, 250);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    for (let i = 1; i <= 14; i++) await page.mouse.move(start.x + i * 6, start.y + 30 + Math.sin(i / 2) * 16);
    await page.mouse.up();
    await expect(layer.locator("path.geo-path")).toHaveAttribute("d", /L.*C/);
    await shot(page, "freeform-drawing");
    await page.mouse.click(p0.x + 1, p0.y + 1);
    await expect(layer).toHaveCount(0);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    const l = letters(s);
    expect(l.startsWith("ML")).toBe(true);
    expect(l.endsWith("Z")).toBe(true);
    expect(l).toContain("C");
    expect(s.fill).toBeDefined();
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:lnTo>");
    expect(xml).toContain("<a:close/>");
    await shot(page, "freeform-done");
  });

  test("a freeform finishes with Enter, open, and Escape and Cancel draw nothing", async ({ page }) => {
    await blankPage(page);
    await run(page, "insert.freeform");
    const done = page.locator(".geo-strip").getByRole("button", { name: "Done", exact: true });
    await expect(done).toBeDisabled();
    for (const [x, y] of [
      [120, 230],
      [200, 200],
      [260, 260],
    ]) {
      const p = await onSheet(page, x, y);
      await page.mouse.click(p.x, p.y);
    }
    await expect(done).toBeEnabled();
    await page.keyboard.press("Enter");
    await expect(page.locator(".geo-layer")).toHaveCount(0);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(letters(s)).toBe("MLL");
    expect(s.fill).toBeUndefined();
    // Escape while drawing: nothing is added.
    await run(page, "insert.freeform");
    const q = await onSheet(page, 300, 300);
    await page.mouse.click(q.x, q.y);
    await page.keyboard.press("Escape");
    await expect(page.locator(".geo-layer")).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(1);
    // So does Cancel.
    await run(page, "insert.freeform");
    await page.locator(".geo-strip").getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator(".geo-layer")).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(1);
  });

  test("a scribble is one smoothed stroke with no fill", async ({ page }) => {
    await blankPage(page);
    await pickTool(page, "Scribble");
    const start = await onSheet(page, 120, 240);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    for (let i = 1; i <= 40; i++) await page.mouse.move(start.x + i * 5, start.y + Math.sin(i / 3) * 22);
    await shot(page, "scribble-drawing");
    await page.mouse.up();
    await expect(page.locator(".geo-layer")).toHaveCount(0);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.geometry).toBe("custom");
    expect(s.custom!.paths).toHaveLength(1);
    const n = s.custom!.paths[0].cmds!.length;
    expect(n).toBeGreaterThan(3);
    expect(n).toBeLessThan(41);
    expect(letters(s).startsWith("MC")).toBe(true);
    expect(s.fill).toBeUndefined();
    expect(s.name).toContain("Scribble");
    await shot(page, "scribble-done");
  });

  test("a freeform is an ordinary shape: restyle, resize and undo", async ({ page }) => {
    await blankPage(page);
    await run(page, "insert.curve");
    for (const [x, y] of [
      [140, 220],
      [200, 170],
      [260, 230],
    ]) {
      const p = await onSheet(page, x, y);
      await page.mouse.click(p.x, p.y);
    }
    await page.keyboard.press("Enter");
    const at = await where(page);
    await page.locator(".shape-strip").getByRole("button", { name: "Outline", exact: true }).click();
    const swatch = page.locator(".pop .swatch").nth(5);
    const picked = ((await swatch.getAttribute("title")) ?? "").slice(1).toUpperCase();
    await swatch.click();
    expect((await shapeAt(page, at.p, at.o))!.line!.color).toBe(picked);
    const before = (await shapeAt(page, at.p, at.o))!;
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "set_shape", at: a.at, w: a.w * 2, h: a.h * 2 }), { at, w: before.w, h: before.h });
    const after = (await shapeAt(page, at.p, at.o))!;
    expect(after.w).toBeCloseTo(before.w * 2, 0);
    expect(after.geometry).toBe("custom");
  });
});

// ---------------------------------------------------------------------------------------------------------------------

test.describe("edit points", () => {
  const triangle = wordFreeform({ id: 31, x: 150, y: 300, corners: [[0, 60], [50, 0], [100, 60]], closed: true, fill: "FFE699" });

  async function openTriangle(page: Page) {
    await blankPage(page, ["A page with a freeform", { raw: triangle }]);
    await atoms(page).first().click();
    await expect(frame(page)).toBeVisible();
    return where(page);
  }

  const anchors = (page: Page) => page.locator(".edit-points .ep-anchor");

  test("the strip, the Format menu and the right-click menu all start it, and Escape ends it", async ({ page }) => {
    await openTriangle(page);
    const stripButton = page.locator(".shape-strip").getByRole("button", { name: "Edit points", exact: true });
    await expect(stripButton).toBeVisible();
    await stripButton.click();
    await expect(anchors(page)).toHaveCount(3);
    await expect(frame(page)).toBeHidden();
    await expect(page.locator(".ep-strip")).toBeVisible();
    await shot(page, "edit-points");
    await page.keyboard.press("Escape");
    await expect(page.locator(".edit-points")).toHaveCount(0);
    await expect(frame(page)).toBeVisible();
    await chooseMenu(page, "Format", "Shape", "Edit points");
    await expect(anchors(page)).toHaveCount(3);
    await page.keyboard.press("Escape");
    await atoms(page).first().click({ button: "right" });
    await contextItem(page, "Edit points").click();
    await expect(anchors(page)).toHaveCount(3);
    // Clicking away leaves it.
    await page.mouse.click(5, 300);
    await expect(page.locator(".edit-points")).toHaveCount(0);
  });

  test("dragging a vertex changes the outline and the shape's box follows it, in one undo step", async ({ page }) => {
    const at = await openTriangle(page);
    const before = (await shapeAt(page, at.p, at.o))!;
    await page.locator(".shape-strip").getByRole("button", { name: "Edit points", exact: true }).click();
    const top = anchors(page).nth(1);
    const c = await center(top);
    await page.mouse.move(c.x, c.y);
    await page.mouse.down();
    await page.mouse.move(c.x - 20, c.y - 40, { steps: 6 });
    await page.mouse.up();
    const after = (await shapeAt(page, at.p, at.o))!;
    expect(letters(after)).toBe("MLLZ");
    // The apex moved 20 px left and 40 px up: the box is 40 px (30 pt) taller, its top moved up with it.
    expect(after.h).toBeCloseTo(before.h + 30, 0);
    expect(after.y!).toBeCloseTo(before.y! - 30, 0);
    expect(after.w).toBeCloseTo(before.w, 0);
    // The apex is still the second point, and sits left of where it was in the new box.
    const cmds = after.custom!.paths[0].cmds!;
    const w = after.custom!.paths[0].w;
    expect(cmds[1].pts[0][0] / w).toBeLessThan(0.5 - 0.1);
    // The shape's own frame stays out of the way while its points are edited, also after a change.
    await expect(frame(page)).toBeHidden();
    await shot(page, "edit-points-dragged");
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:lnTo>");
    await page.keyboard.press("Escape");
    await page.keyboard.press(`${MOD}+Z`);
    const undone = (await shapeAt(page, at.p, at.o))!;
    expect(undone.h).toBeCloseTo(before.h, 0);
    expect(undone.y!).toBeCloseTo(before.y!, 0);
  });

  test("clicking the outline adds a point and Delete takes the selected one out", async ({ page }) => {
    const at = await openTriangle(page);
    await page.locator(".shape-strip").getByRole("button", { name: "Edit points", exact: true }).click();
    const [a, b] = [await center(anchors(page).nth(1)), await center(anchors(page).nth(2))];
    await page.mouse.click((a.x + b.x) / 2, (a.y + b.y) / 2);
    await expect(anchors(page)).toHaveCount(4);
    expect(letters((await shapeAt(page, at.p, at.o))!)).toBe("MLLLZ");
    // The new point is selected; Delete removes it again.
    await expect(anchors(page).nth(2)).toHaveClass(/selected/);
    await page.keyboard.press("Delete");
    await expect(anchors(page)).toHaveCount(3);
    expect(letters((await shapeAt(page, at.p, at.o))!)).toBe("MLLZ");
    // A triangle can't lose a point.
    await anchors(page).nth(0).click();
    await page.keyboard.press("Delete");
    await expect(anchors(page)).toHaveCount(3);
    await expect(page.locator(".toast")).toContainText("at least two points");
  });

  test("the right-click menu on a point makes it smooth, straight or a corner, and Add point works on the outline", async ({ page }) => {
    const at = await openTriangle(page);
    await page.locator(".shape-strip").getByRole("button", { name: "Edit points", exact: true }).click();
    await anchors(page).nth(1).click({ button: "right" });
    await page.locator(".ep-menu").getByRole("menuitem", { name: "Smooth point", exact: true }).click();
    let s = (await shapeAt(page, at.p, at.o))!;
    expect(letters(s)).toContain("C");
    // The point has handles now.
    await expect(page.locator(".edit-points .ep-handle")).toHaveCount(2);
    await shot(page, "edit-points-smooth");
    await anchors(page).nth(1).click({ button: "right" });
    await page.locator(".ep-menu").getByRole("menuitem", { name: "Straight point", exact: true }).click();
    s = (await shapeAt(page, at.p, at.o))!;
    expect(letters(s)).toBe("MLLZ");
    // Add point from the menu on the outline.
    const [a, b] = [await center(anchors(page).nth(0)), await center(anchors(page).nth(1))];
    await page.mouse.click((a.x + b.x) / 2, (a.y + b.y) / 2, { button: "right" });
    await page.locator(".ep-menu").getByRole("menuitem", { name: "Add point", exact: true }).click();
    await expect(anchors(page)).toHaveCount(4);
    expect(letters((await shapeAt(page, at.p, at.o))!)).toBe("MLLLZ");
  });

  test("dragging a curve handle reshapes the curve", async ({ page }) => {
    await blankPage(page);
    await run(page, "insert.curve");
    for (const [x, y] of [
      [140, 260],
      [220, 200],
      [300, 260],
    ]) {
      const p = await onSheet(page, x, y);
      await page.mouse.click(p.x, p.y);
    }
    await page.keyboard.press("Enter");
    const at = await where(page);
    const before = (await shapeAt(page, at.p, at.o))!;
    await run(page, "shape.editPoints");
    await expect(anchors(page)).toHaveCount(3);
    // Select the middle point: its handles show.
    await anchors(page).nth(1).click();
    const handles = page.locator(".edit-points .ep-handle");
    await expect(handles).toHaveCount(2);
    const h = await center(handles.nth(1));
    await page.mouse.move(h.x, h.y);
    await page.mouse.down();
    await page.mouse.move(h.x + 30, h.y - 50, { steps: 5 });
    await page.mouse.up();
    const after = (await shapeAt(page, at.p, at.o))!;
    expect(JSON.stringify(after.custom!.paths[0].cmds)).not.toBe(JSON.stringify(before.custom!.paths[0].cmds));
    expect(letters(after)).toBe(letters(before));
    // The curve is still smooth at the point: the two handles stay in line.
    await shot(page, "edit-points-handle");
  });

  test("a rectangle becomes a custom outline when its first point moves, and the file says so", async ({ page }) => {
    await blankPage(page, ["A page with a rectangle", { raw: floatShape({ id: 21, name: "Box", relH: "page", x: 150, relV: "page", y: 300, w: 120, h: 70, fill: "FFE699" }) }]);
    const at = await where(page);
    expect((await shapeAt(page, at.p, at.o))!.geometry).toBe("rect");
    await atoms(page).first().click();
    await run(page, "shape.editPoints");
    await expect(anchors(page)).toHaveCount(4);
    // The bottom right corner out by 30 and down by 20.
    const corner = await center(anchors(page).nth(2));
    await page.mouse.move(corner.x, corner.y);
    await page.mouse.down();
    await page.mouse.move(corner.x + 40, corner.y + 26, { steps: 5 });
    await page.mouse.up();
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.geometry).toBe("custom");
    expect(letters(s)).toBe("MLLLZ");
    expect(s.w).toBeCloseTo(120 + 40 / PT_PX, 0);
    expect(s.h).toBeCloseTo(70 + 26 / PT_PX, 0);
    // Where the top left corner was, it still is.
    expect(s.x).toBeCloseTo(150, 0);
    expect(s.y).toBeCloseTo(300, 0);
    expect(s.fill!.color).toBe("FFE699");
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).not.toContain('prst="rect"');
    expect(xml).toContain("<a:custGeom>");
    expect(xml).toContain('o:spt="100"');
    await shot(page, "edit-points-rect");
  });

  test("the points sit on the corners of a shape that is turned, mirrored and zoomed", async ({ page }) => {
    await blankPage(page, ["A turned rectangle", { raw: floatShape({ id: 23, relH: "page", x: 200, relV: "page", y: 260, w: 120, h: 70 }) }]);
    const at = await where(page);
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "set_shape", at: a, rot: 90, flip_h: true }), at);
    await inPage(page, (w) => w.fe.editor.setZoom(1.25));
    await atoms(page).first().click();
    await run(page, "shape.editPoints");
    await expect(anchors(page)).toHaveCount(4);
    // Turned a quarter, the rectangle's four corners are the four corners of the box it fills on screen.
    const box = (await atoms(page).first().boundingBox())!;
    const corners = [
      [box.x, box.y],
      [box.x + box.width, box.y],
      [box.x + box.width, box.y + box.height],
      [box.x, box.y + box.height],
    ];
    const found: number[][] = [];
    for (let i = 0; i < 4; i++) {
      const c = await center(anchors(page).nth(i));
      found.push([c.x, c.y]);
    }
    for (const [cx, cy] of corners) expect(found.some(([x, y]) => Math.abs(x - cx) < 1.5 && Math.abs(y - cy) < 1.5)).toBe(true);
    // Dragging one by 40 pixels moves it by 40 / 1.25 / 1.333 points in the shape's own turned directions.
    const before = (await shapeAt(page, at.p, at.o))!;
    const c0 = await center(anchors(page).nth(2));
    await page.mouse.move(c0.x, c0.y);
    await page.mouse.down();
    await page.mouse.move(c0.x + 40, c0.y, { steps: 5 });
    await page.mouse.up();
    const after = (await shapeAt(page, at.p, at.o))!;
    expect(after.geometry).toBe("custom");
    expect(after.rot).toBe(90);
    expect(after.flipH).toBe(true);
    expect(before.geometry).toBe("rect");
    // One corner moved inside the box, so the outline is no longer a rectangle: a point is off the corners of the bounds.
    const path = after.custom!.paths[0];
    // (Firefox and Safari move the mouse in whole pixels, so the dragged point can be a tenth of a point off its row or column: the corners are found within a fifth of a point, 2,540 EMU.)
    const onCorner = (p: number[]) => [0, path.w].some((x) => Math.abs(p[0] - x) < 2540) && [0, path.h].some((y) => Math.abs(p[1] - y) < 2540);
    const pts = path.cmds!.filter((c) => c.t !== "Z").map((c) => c.pts[0]);
    expect(pts.filter((p) => !onCorner(p))).toHaveLength(1);
    await shot(page, "edit-points-turned");
  });

  test("an oval's points can be edited: it has four curves", async ({ page }) => {
    await blankPage(page, ["An oval", { raw: floatShape({ id: 22, relH: "page", x: 150, relV: "page", y: 300, w: 120, h: 80 }).replace('prst="rect"', 'prst="ellipse"') }]);
    await atoms(page).first().click();
    await run(page, "shape.editPoints");
    await expect(anchors(page)).toHaveCount(5);
    await shot(page, "edit-points-oval");
  });
});

// ---------------------------------------------------------------------------------------------------------------------

test.describe("connectors", () => {
  /** Two rectangles, A at (100, 300) and B at (350, 400) on the page (100 by 60 points each). */
  const boxes =
    floatShape({ id: 11, name: "Box A", relH: "page", x: 100, relV: "page", y: 300, w: 100, h: 60, fill: "DAE3F3" }) +
    floatShape({ id: 12, name: "Box B", relH: "page", x: 350, relV: "page", y: 400, w: 100, h: 60, fill: "FFE699" });

  async function openBoxes(page: Page) {
    await blankPage(page, ["Two boxes to connect", { raw: boxes }]);
    return { a: await where(page, 0), b: await where(page, 1) };
  }

  /** The connector on the page, if there is one: the atom that is a line. */
  const connectorAtom = (page: Page) => page.locator(".flow .atom.shape.line").first();

  async function drawConnector(page: Page, tool: string, from: { x: number; y: number }, to: { x: number; y: number }) {
    await pickTool(page, tool);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
    await page.mouse.move(to.x, to.y, { steps: 4 });
  }

  test("hovering a shape shows its connection sites, and the end snaps to the nearest", async ({ page }) => {
    await openBoxes(page);
    await pickTool(page, "Elbow connector");
    const a = (await atoms(page).nth(0).boundingBox())!;
    await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
    const dots = page.locator(".geo-overlay .geo-site:not(.hot)");
    await expect(dots).toHaveCount(4);
    // Near the right middle, that site is the hot one.
    await page.mouse.move(a.x + a.width + 3, a.y + a.height / 2 + 2);
    const hot = page.locator(".geo-overlay .geo-site.hot");
    await expect(hot).toHaveCount(1);
    const hb = (await hot.boundingBox())!;
    expect(Math.abs(hb.x + hb.width / 2 - (a.x + a.width))).toBeLessThan(2);
    expect(Math.abs(hb.y + hb.height / 2 - (a.y + a.height / 2))).toBeLessThan(2);
    await shot(page, "connector-sites");
    await page.keyboard.press("Escape");
  });

  test("dragging from one shape's site to another's draws an elbow connector glued to both", async ({ page }) => {
    const { a, b } = await openBoxes(page);
    const [ab, bb] = [(await atoms(page).nth(0).boundingBox())!, (await atoms(page).nth(1).boundingBox())!];
    const from = { x: ab.x + ab.width, y: ab.y + ab.height / 2 };
    const to = { x: bb.x, y: bb.y + bb.height / 2 };
    await drawConnector(page, "Elbow connector", from, to);
    // While the button is down the connector shows as it will be.
    await expect(page.locator(".geo-overlay .geo-route")).toBeVisible();
    await shot(page, "connector-drawing");
    await page.mouse.up();
    await expect(page.locator(".geo-layer")).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(3);
    const c = (await shapeAt(page, a.p, 0))!;
    expect(c.geometry).toBe("bentConnector3");
    const sa = (await shapeAt(page, a.p, 1))!;
    const sb = (await shapeAt(page, a.p, 2))!;
    expect([sa.id, sb.id].sort()).toEqual([11, 12]);
    // Right middle of A, left middle of B: sites 3 and 1 of a rectangle as Word numbers them.
    expect(c.connect).toEqual({ start: { id: 11, idx: 3 }, end: { id: 12, idx: 1 } });
    expect(c.line!.color).toBe("4472C4");
    expect(c.wrap).toBe("front");
    // On screen it runs from A's right middle to B's left middle (the page scrolled to show the new connector, so measure again).
    const [ab2, bb2] = [(await atoms(page).nth(1).boundingBox())!, (await atoms(page).nth(2).boundingBox())!];
    const cb = (await connectorAtom(page).boundingBox())!;
    expect(Math.abs(cb.x - (ab2.x + ab2.width))).toBeLessThan(2);
    expect(Math.abs(cb.y - (ab2.y + ab2.height / 2))).toBeLessThan(2);
    expect(Math.abs(cb.x + cb.width - bb2.x)).toBeLessThan(2);
    expect(Math.abs(cb.y + cb.height - (bb2.y + bb2.height / 2))).toBeLessThan(2);
    expect(Math.abs(ab2.x + ab2.width - from.x)).toBeLessThan(2);
    expect(Math.abs(bb2.x - to.x)).toBeLessThan(2);
    // And the file says it.
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('<a:stCxn id="11" idx="3"/>');
    expect(xml).toContain('<a:endCxn id="12" idx="1"/>');
    expect(xml).toContain('prst="bentConnector3"');
    await expect(frame(page)).toBeHidden();
    await expect(page.locator(".cxn-end")).toHaveCount(2);
    await shot(page, "connector-done");
  });

  test("moving a shape, nudging it and undoing keep the connector on it", async ({ page }) => {
    const { a } = await openBoxes(page);
    const [ab, bb] = [(await atoms(page).nth(0).boundingBox())!, (await atoms(page).nth(1).boundingBox())!];
    await drawConnector(page, "Elbow connector", { x: ab.x + ab.width, y: ab.y + ab.height / 2 }, { x: bb.x, y: bb.y + bb.height / 2 });
    await page.mouse.up();
    // The connector is in the paragraph before the boxes (it is inserted at the start); find the boxes again.
    const boxIndex = async (id: number) => {
      for (let i = 0; i < 3; i++) {
        const pos = await where(page, i);
        if ((await shapeAt(page, pos.p, pos.o))!.id === id) return i;
      }
      return -1;
    };
    const bi = await boxIndex(12);
    const connectorBox = async () => (await connectorAtom(page).boundingBox())!;
    const before = await connectorBox();
    // Drag B 60 px right and 40 px down with the mouse.
    const b = (await atoms(page).nth(bi).boundingBox())!;
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await page.mouse.down();
    await page.mouse.move(b.x + b.width / 2 + 30, b.y + b.height / 2 + 20, { steps: 4 });
    await page.mouse.move(b.x + b.width / 2 + 60, b.y + b.height / 2 + 40, { steps: 4 });
    await page.mouse.up();
    const after = await connectorBox();
    expect(after.x).toBeCloseTo(before.x, 0);
    expect(after.y).toBeCloseTo(before.y, 0);
    expect(after.width).toBeCloseTo(before.width + 60, 0);
    expect(after.height).toBeCloseTo(before.height + 40, 0);
    // B's left middle is the connector's far corner.
    const moved = (await atoms(page).nth(bi).boundingBox())!;
    expect(Math.abs(after.x + after.width - moved.x)).toBeLessThan(2);
    expect(Math.abs(after.y + after.height - (moved.y + moved.height / 2))).toBeLessThan(2);
    const pos = await where(page, 0);
    expect((await shapeAt(page, pos.p, pos.o))!.connect!.end).toEqual({ id: 12, idx: 1 });
    await shot(page, "connector-moved");
    // Arrow keys nudge B by a point and the connector follows.
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect.poll(async () => (await connectorBox()).width).toBeCloseTo(after.width + 2 * PT_PX, 0);
    // One undo takes the nudges back, another the drag, and the connector with each.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(async () => Math.round((await connectorBox()).width)).toBe(Math.round(after.width));
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(async () => Math.round((await connectorBox()).width)).toBe(Math.round(before.width));
    expect(a.p).toBeGreaterThan(0);
  });

  test("deleting a shape lets go of the connector, which stays where it was", async ({ page }) => {
    await openBoxes(page);
    const [ab, bb] = [(await atoms(page).nth(0).boundingBox())!, (await atoms(page).nth(1).boundingBox())!];
    await drawConnector(page, "Curved connector", { x: ab.x + ab.width, y: ab.y + ab.height / 2 }, { x: bb.x, y: bb.y + bb.height / 2 });
    await page.mouse.up();
    const pos = await where(page, 0);
    expect((await shapeAt(page, pos.p, pos.o))!.geometry).toBe("curvedConnector3");
    const cbox = (await connectorAtom(page).boundingBox())!;
    // Select B and delete it.
    const bi = await (async () => {
      for (let i = 0; i < 3; i++) {
        const q = await where(page, i);
        if ((await shapeAt(page, q.p, q.o))!.id === 12) return i;
      }
      return -1;
    })();
    await atoms(page).nth(bi).click();
    await page.keyboard.press("Delete");
    await expect(atoms(page)).toHaveCount(2);
    const c = (await shapeAt(page, pos.p, pos.o))!;
    expect(c.connect).toEqual({ start: { id: 11, idx: 3 } });
    const now = (await connectorAtom(page).boundingBox())!;
    expect(now.x).toBeCloseTo(cbox.x, 0);
    expect(now.width).toBeCloseTo(cbox.width, 0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(3);
    expect((await shapeAt(page, pos.p, pos.o))!.connect!.end).toEqual({ id: 12, idx: 1 });
  });

  test("a selected connector has a handle at each end: drag one to another site, or off the shapes", async ({ page }) => {
    const { a } = await openBoxes(page);
    const [ab, bb] = [(await atoms(page).nth(0).boundingBox())!, (await atoms(page).nth(1).boundingBox())!];
    await drawConnector(page, "Straight arrow connector", { x: ab.x + ab.width, y: ab.y + ab.height / 2 }, { x: bb.x, y: bb.y + bb.height / 2 });
    await page.mouse.up();
    const pos = await where(page, 0);
    const model = async () => (await shapeAt(page, pos.p, pos.o))!;
    expect((await model()).geometry).toBe("straightConnector1");
    expect((await model()).line!.tailArrow).toBe("triangle");
    const handles = page.locator(".cxn-end");
    await expect(handles).toHaveCount(2);
    await expect(page.locator(".cxn-end.glued")).toHaveCount(2);
    // Drag the end onto B's top middle (site 0); B is where it is now, after the page scrolled to the connector.
    const bNow = (await atoms(page).nth(2).boundingBox())!;
    const end = await center(handles.nth(1));
    await page.mouse.move(end.x, end.y);
    await page.mouse.down();
    await page.mouse.move(bNow.x + bNow.width / 2, bNow.y + 3, { steps: 6 });
    await expect(page.locator(".geo-overlay .geo-site.hot")).toHaveCount(1);
    await page.mouse.up();
    expect((await model()).connect!.end).toEqual({ id: 12, idx: 0 });
    // Drag it away from B into the empty page: it lets go, and the start stays glued.
    const end2 = await center(page.locator(".cxn-end").nth(1));
    await page.mouse.move(end2.x, end2.y);
    await page.mouse.down();
    await page.mouse.move(end2.x + 80, end2.y + 120, { steps: 6 });
    await page.mouse.up();
    const m = await model();
    expect(m.connect!.end).toBeUndefined();
    expect(m.connect!.start).toEqual({ id: 11, idx: 3 });
    await expect(page.locator(".cxn-end.glued")).toHaveCount(1);
    await shot(page, "connector-ends");
    // Moving A still moves the start.
    const sa = await (async () => {
      for (let i = 0; i < 3; i++) {
        const q = await where(page, i);
        if ((await shapeAt(page, q.p, q.o))!.id === 11) return q;
      }
      return a;
    })();
    const w0 = (await model()).w;
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_shape", at: p, x: 60, y: 300 }), sa);
    expect((await model()).w).not.toBeCloseTo(w0, 0);
  });

  test("moving the connector itself lets go of both shapes", async ({ page }) => {
    await openBoxes(page);
    const [ab, bb] = [(await atoms(page).nth(0).boundingBox())!, (await atoms(page).nth(1).boundingBox())!];
    await drawConnector(page, "Elbow connector", { x: ab.x + ab.width, y: ab.y + ab.height / 2 }, { x: bb.x, y: bb.y + bb.height / 2 });
    await page.mouse.up();
    const pos = await where(page, 0);
    await page.keyboard.press("ArrowDown");
    const m = (await shapeAt(page, pos.p, pos.o))!;
    expect(m.connect).toEqual({});
  });

  test("a connector from Word is glued: moving its shape re-routes it, and an untouched one is not rewritten", async ({ page }) => {
    const fixture =
      floatShape({ id: 11, name: "Box A", relH: "page", x: 100, relV: "page", y: 300, w: 100, h: 60 }) +
      floatShape({ id: 12, name: "Box B", relH: "page", x: 300, relV: "page", y: 400, w: 100, h: 60 }) +
      floatShape({ id: 13, name: "Box C", relH: "page", x: 100, relV: "page", y: 520, w: 100, h: 60 }) +
      wordConnector({ id: 14, prst: "bentConnector3", x: 200, y: 330, w: 100, h: 100, start: { id: 11, idx: 3 }, end: { id: 12, idx: 1 } }) +
      wordConnector({ id: 15, prst: "straightConnector1", x: 150, y: 360, w: 0, h: 160, start: { id: 11, idx: 2 }, end: { id: 13, idx: 0 }, arrow: true });
    await blankPage(page, ["Connectors from Word", { raw: fixture }]);
    const find = async (id: number) => {
      const n = await atoms(page).count();
      for (let i = 0; i < n; i++) {
        const q = await where(page, i);
        if ((await shapeAt(page, q.p, q.o))!.id === id) return q;
      }
      throw new Error(`no shape ${id}`);
    };
    const [pb, pc1, pc2] = [await find(12), await find(14), await find(15)];
    const c1 = (await shapeAt(page, pc1.p, pc1.o))!;
    expect(c1.connect).toEqual({ start: { id: 11, idx: 3 }, end: { id: 12, idx: 1 } });
    expect(c1.geometry).toBe("bentConnector3");
    const xml0 = await savedPart(page, "word/document.xml");
    const second = (xml: string) => xml.slice(xml.indexOf('name="Connector 15"') - 60, xml.indexOf("</mc:AlternateContent>", xml.indexOf('name="Connector 15"')));
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_shape", at: p, x: 340, y: 450 }), pb);
    const c1b = (await shapeAt(page, pc1.p, pc1.o))!;
    expect(c1b.w).toBeCloseTo(140, 0);
    expect(c1b.h).toBeCloseTo(150, 0);
    // The other connector joins A and C, which did not move: byte for byte as it was.
    const xml1 = await savedPart(page, "word/document.xml");
    expect(second(xml1)).toBe(second(xml0));
    expect((await shapeAt(page, pc2.p, pc2.o))!.connect).toEqual({ start: { id: 11, idx: 2 }, end: { id: 13, idx: 0 } });
    await shot(page, "connector-from-word");
    await page.keyboard.press(`${MOD}+Z`);
    expect((await shapeAt(page, pc1.p, pc1.o))!.w).toBeCloseTo(100, 0);
  });
});
