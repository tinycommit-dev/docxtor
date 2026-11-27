// SmartArt drawn from the drawing Word saves with it (or laid out from its data model), and WordArt: text bent to
// a preset, both the DrawingML and the VML form. What is on the page is checked, not only that nothing failed.
import { smartArtDocx, openBytes, wordArtDocx } from "./support/docx-builder";
import { blockListShapes, cycleShapes, dspDrawing, processShapes, vmlWordArt, wordArtRun } from "./support/smartart-markup";
import { expect, inPage, test, type Page } from "./helpers";

declare global {
  interface Window {
    __fillBox(el: SVGGraphicsElement): { x: number; y: number; left: number; top: number; right: number; bottom: number; width: number; height: number };
  }
}

// Where a shape's own outline is on screen, without what is drawn round it: in Firefox `getBoundingClientRect` of an SVG path reaches out to the
// shape's shadow (the drawing's style gives each box one), where Chrome's is the path alone. The path's box is taken in its own units and put on screen.
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__fillBox = (el) => {
      const b = el.getBBox();
      const m = el.getScreenCTM()!;
      const left = m.e + b.x * m.a;
      const top = m.f + b.y * m.d;
      return { x: left, y: top, left, top, width: b.width * m.a, height: b.height * m.d, right: left + b.width * m.a, bottom: top + b.height * m.d };
    };
  });
});

const PT_PX = 96 / 72;
const MOD = "ControlOrMeta";

interface Shape {
  kind: string;
  w: number;
  h: number;
  x?: number;
  y?: number;
  wrap: string;
  alt?: string;
  diagram?: { source: string; layout?: string };
  children?: { childX: number; childY: number; w: number; h: number; geometry: string; fill?: { color: string }; dtext?: { paras: { runs: { text: string }[] }[] } }[];
  warp?: { preset: string };
  text?: { id: number; segs: { x?: string }[] }[];
}

const shapeAt = (page: Page, p: number, o: number) =>
  inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape | null>;
const run = (page: Page, ops: unknown) => inPage(page, (w, o) => w.fe.editor.exec(o), ops);
const zoomOf = (page: Page) => inPage(page, (w) => w.fe.editor.zoom);
const diagrams = (page: Page) => page.locator(".flow .atom.shape.diagram");
const wordArts = (page: Page) => page.locator(".flow .atom.shape.wordart");
const frame = (page: Page) => page.locator(".obj-frame");

/** Where the n-th shape is in the model. */
const where = (page: Page, sel = ".flow .atom.shape", n = 0) =>
  page
    .locator(sel)
    .nth(n)
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));

const center = async (loc: ReturnType<Page["locator"]>) => {
  const b = (await loc.boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2, box: b };
};

/** A part of the saved file as text (stored or deflated). */
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

/** Take a picture of the first page when DOCXTOR_SHOTS names a folder (to look at, not to compare). */
async function shot(page: Page, name: string) {
  const dir = process.env.DOCXTOR_SHOTS;
  if (!dir) return;
  await page.locator(".sheet").first().scrollIntoViewIfNeeded();
  await page.screenshot({ path: `${dir}/${name}.png`, clip: await clipOf(page) });
}

/** The area from the top of the first page to the bottom of its last object. */
async function clipOf(page: Page) {
  const box = (await page.locator(".sheet").first().boundingBox())!;
  return { x: Math.max(0, box.x), y: Math.max(0, box.y), width: Math.min(box.width, 1280), height: Math.min(box.height, 880) };
}

/** Where each shape of a diagram is on screen, in points from the top-left of the object, and its text. */
async function drawn(page: Page, zoom: number) {
  return diagrams(page)
    .first()
    .evaluate((a, k) => {
      const origin = a.getBoundingClientRect();
      return Array.from(a.querySelectorAll(".dg-shape")).map((g) => {
        const outline = window.__fillBox(g.querySelector("path") as SVGGraphicsElement);
        const text = g.querySelector(".dg-text-box");
        const span = text?.querySelector("span");
        return {
          x: (outline.left - origin.left) / k,
          y: (outline.top - origin.top) / k,
          w: outline.width / k,
          h: outline.height / k,
          fill: g.querySelector("path")!.getAttribute("fill"),
          text: text?.textContent ?? "",
          color: span ? getComputedStyle(span).color : "",
          size: span ? getComputedStyle(span).fontSize : "",
          textBox: text ? text.getBoundingClientRect() : null,
        };
      });
    }, PT_PX * zoom);
}

const NODES = ["Alpha", "Beta", "Gamma"];

test.describe("SmartArt from Word's drawing", () => {
  test("a basic block list is its drawing: boxes where Word put them, filled, with their text", async ({ page }) => {
    const { bytes } = smartArtDocx([{ layout: "default", nodes: NODES, drawing: dspDrawing(blockListShapes(NODES)), alt: "Three words in boxes" }]);
    await openBytes(page, bytes, "Before the graphic");
    await expect(diagrams(page)).toHaveCount(1);
    const z = await zoomOf(page);
    const box = (await diagrams(page).first().boundingBox())!;
    expect(box.width).toBeCloseTo(432 * PT_PX * z, 0);
    expect(box.height).toBeCloseTo(252 * PT_PX * z, 0);
    await expect(diagrams(page).first()).toHaveAttribute("aria-label", "Three words in boxes");
    const want = blockListShapes(NODES);
    const got = await drawn(page, z);
    expect(got).toHaveLength(3);
    got.forEach((g, i) => {
      expect(g.x).toBeCloseTo(want[i].x, 0);
      expect(g.y).toBeCloseTo(want[i].y, 0);
      expect(g.w).toBeCloseTo(want[i].w, 0);
      expect(g.h).toBeCloseTo(want[i].h, 0);
      expect(g.fill).toBe("#4472C4");
      expect(g.text).toBe(NODES[i]);
      // The style's white, at the size the drawing says; the text sits inside its box.
      expect(g.color).toBe("rgb(255, 255, 255)");
      expect(g.size).toBe("20px");
      const t = g.textBox!;
      const o = diagramsOrigin(box);
      expect(t.left).toBeGreaterThanOrEqual(o.x + g.x * PT_PX * z - 1);
      expect(t.right).toBeLessThanOrEqual(o.x + (g.x + g.w) * PT_PX * z + 1);
    });
    await shot(page, "smartart-block-list");
  });

  test("a basic process and a basic cycle show their arrows and ellipses", async ({ page }) => {
    const steps = ["Plan", "Do", "Check"];
    const cycle = ["North", "East", "South", "West"];
    const { bytes } = smartArtDocx([
      { layout: "process1", nodes: steps, drawing: dspDrawing(processShapes(steps, 432, 120)), w: 432, h: 120 },
      { layout: "cycle2", nodes: cycle, drawing: dspDrawing(cycleShapes(cycle, 300, 300)), w: 300, h: 300 },
    ]);
    await openBytes(page, bytes, "Before the graphic");
    await expect(diagrams(page)).toHaveCount(2);
    const z = await zoomOf(page);
    // The process: three rounded boxes and two arrows, in a row.
    const procPaths = await diagrams(page).nth(0).evaluate((a) => Array.from(a.querySelectorAll(".dg-shape")).map((g) => ({ box: window.__fillBox(g.querySelector("path") as SVGGraphicsElement), d: g.querySelector("path")!.getAttribute("d") })));
    expect(procPaths).toHaveLength(5);
    const boxes = [0, 2, 4].map((i) => procPaths[i].box);
    const arrows = [1, 3].map((i) => procPaths[i].box);
    expect(boxes[0].y).toBeCloseTo(boxes[2].y, 0);
    expect(arrows[0].left).toBeGreaterThan(boxes[0].right);
    expect(arrows[0].right).toBeLessThan(boxes[1].left);
    expect(arrows[0].height).toBeLessThan(boxes[0].height / 2);
    // A rounded box has curves in its outline; an arrow has points.
    expect(procPaths[0].d).toMatch(/[AQC]/);
    // The cycle: four ellipses round a circle and an arrow between each pair, turned along it.
    const cyc = await diagrams(page).nth(1).evaluate((a) => {
      const o = a.getBoundingClientRect();
      return Array.from(a.querySelectorAll(".dg-shape")).map((g) => {
        const r = window.__fillBox(g.querySelector("path") as SVGGraphicsElement);
        return { cx: r.left + r.width / 2 - o.left, cy: r.top + r.height / 2 - o.top, transform: g.getAttribute("transform") ?? "", text: g.querySelector(".dg-text-box")?.textContent ?? "" };
      });
    });
    expect(cyc).toHaveLength(8);
    const k = PT_PX * z;
    const nodes = cyc.slice(0, 4);
    const radii = nodes.map((n) => Math.hypot(n.cx - 150 * k, n.cy - 150 * k) / k);
    for (const r of radii) expect(r).toBeCloseTo(radii[0], 0);
    expect(nodes[0].cx / k).toBeCloseTo(150, 0);
    expect(nodes[0].cy).toBeLessThan(150 * k);
    expect(nodes.map((n) => n.text)).toEqual(cycle);
    expect(cyc.slice(4).every((a) => /rotate\(/.test(a.transform))).toBe(true);
    await shot(page, "smartart-process-cycle");
  });

  test("the graphic is selected, resized and deleted like a picture; its drawing scales; double-click only says where to edit", async ({ page }) => {
    const { bytes, parts } = smartArtDocx([{ layout: "default", nodes: NODES, drawing: dspDrawing(blockListShapes(NODES)) }]);
    await openBytes(page, bytes, "Before the graphic");
    const atom = diagrams(page).first();
    const z = await zoomOf(page);
    const at = await where(page, ".flow .atom.shape.diagram");
    await atom.click();
    await expect(frame(page)).toBeVisible();
    await expect(frame(page).locator(".obj-handle.se")).toBeVisible();
    // Double-click does not open a text box; it says what to do.
    await atom.dblclick();
    await expect(page.locator(".toast")).toContainText("Edit SmartArt in Word.");
    await expect(page.locator("[contenteditable='true']:not(.flow)").first()).toHaveCount(0);
    // Resize by the corner: the box and everything in it scale together.
    const before = await drawn(page, z);
    const se = await center(frame(page).locator(".obj-handle.se"));
    await page.mouse.move(se.x, se.y);
    await page.mouse.down();
    await page.mouse.move(se.x - 100, se.y - 40, { steps: 4 });
    await page.mouse.up();
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.w).toBeCloseTo(432 - 100 / z / PT_PX, 0);
    expect(s.h).toBeCloseTo(252 - 40 / z / PT_PX, 0);
    const after = await drawn(page, z);
    const fx = s.w / 432;
    const fy = s.h / 252;
    after.forEach((g, i) => {
      expect(g.x).toBeCloseTo(before[i].x * fx, 0);
      expect(g.y).toBeCloseTo(before[i].y * fy, 0);
      expect(g.w).toBeCloseTo(before[i].w * fx, 0);
      expect(g.h).toBeCloseTo(before[i].h * fy, 0);
    });
    // The parts of the graphic are in the saved file exactly as they were; only the extent changed.
    for (const name of Object.keys(parts)) expect(await savedPart(page, name), name).toBe(parts[name]);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<wp:extent");
    expect(xml).not.toContain(`<wp:extent cx="${Math.round(432 * 12700)}"`);
    // The size the drawing was made for is noted, so a later open scales the same way.
    expect(xml).toContain("https://docxtor.tinycommit.dev/ns/smartart");
    // Undo takes the size back.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(async () => (await shapeAt(page, at.p, at.o))!.w).toBeCloseTo(432, 0);
    // Delete removes the graphic.
    await atom.click();
    await page.keyboard.press("Delete");
    await expect(diagrams(page)).toHaveCount(0);
  });

  test("a floating graphic is moved by dragging", async ({ page }) => {
    const { bytes } = smartArtDocx([{ layout: "default", nodes: NODES, drawing: dspDrawing(blockListShapes(NODES, 216, 126)), w: 216, h: 126, anchor: { x: 20, y: 10 } }]);
    await openBytes(page, bytes, "Before the graphic");
    const atom = diagrams(page).first();
    const at = await where(page, ".flow .atom.shape.diagram");
    const z = await zoomOf(page);
    await atom.click();
    const before = (await shapeAt(page, at.p, at.o))!;
    expect(before.wrap).toBe("square");
    const c = await center(atom);
    await page.mouse.move(c.x, c.y);
    await page.mouse.down();
    await page.mouse.move(c.x + 30, c.y + 10, { steps: 3 });
    await page.mouse.move(c.x + 60, c.y + 30, { steps: 3 });
    await page.mouse.up();
    const after = (await shapeAt(page, at.p, at.o))!;
    expect(Math.abs(after.x! - before.x! - 60 / z / PT_PX)).toBeLessThan(1.5);
    expect(after.y! - before.y!).toBeGreaterThan(20 / z / PT_PX);
    expect([after.w, after.h]).toEqual([216, 126]);
    await expect(diagrams(page)).toHaveCount(1);
  });
});

test.describe("SmartArt laid out from the data model", () => {
  test("without a drawing the basic layouts are laid out here, with the colours of the theme", async ({ page }) => {
    const five = ["One", "Two", "Three", "Four", "Five"];
    const { bytes } = smartArtDocx([
      { layout: "default", nodes: five },
      { layout: "process1", nodes: NODES, h: 150 },
      { layout: "cycle2", nodes: ["a", "b", "c", "d"], w: 300, h: 300 },
      { layout: "vList2", nodes: NODES, h: 200 },
      { layout: "pyramid1", nodes: NODES },
    ]);
    await openBytes(page, bytes, "Before the graphic");
    await expect(diagrams(page)).toHaveCount(5);
    const z = await zoomOf(page);
    const k = PT_PX * z;
    const geom = (n: number) =>
      diagrams(page)
        .nth(n)
        .evaluate((a) => {
          const o = a.getBoundingClientRect();
          return Array.from(a.querySelectorAll(".dg-shape")).map((g) => {
            const r = window.__fillBox(g.querySelector("path") as SVGGraphicsElement);
            return { x: r.left - o.left, y: r.top - o.top, w: r.width, h: r.height, fill: g.querySelector("path")!.getAttribute("fill"), text: g.querySelector(".dg-text-box")?.textContent ?? "" };
          });
        });
    // Basic block list: three on the first row, two centred on the second, all the same size, in the theme's accent.
    const block = await geom(0);
    expect(block.map((b) => b.text)).toEqual(five);
    expect(block[0].y).toBeCloseTo(block[2].y, 0);
    expect(block[3].y).toBeGreaterThan(block[0].y + block[0].h);
    expect(block[3].y).toBeCloseTo(block[4].y, 0);
    expect(block.every((b) => Math.abs(b.w - block[0].w) < 0.5 && b.fill === "#4472C4")).toBe(true);
    const mid = (b: { x: number; w: number }) => b.x + b.w / 2;
    expect((mid(block[3]) + mid(block[4])) / 2).toBeCloseTo((432 * k) / 2, 0);
    // Basic process: arrows between the boxes.
    const proc = await geom(1);
    expect(proc).toHaveLength(5);
    expect(proc[1].x).toBeGreaterThan(proc[0].x + proc[0].w);
    // Basic cycle: round a circle.
    const cyc = await geom(2);
    expect(cyc).toHaveLength(8);
    const r = cyc.slice(0, 4).map((n) => Math.hypot(n.x + n.w / 2 - 150 * k, n.y + n.h / 2 - 150 * k));
    for (const v of r) expect(v).toBeCloseTo(r[0], 0);
    // Vertical list: three bars the width of the object, one above the other.
    const list = await geom(3);
    expect(list).toHaveLength(3);
    expect(list.every((b) => Math.abs(b.w - 432 * k) < 1)).toBe(true);
    expect(list[1].y).toBeGreaterThan(list[0].y + list[0].h - 1);
    // A layout Docxtor does not draw is the basic block list.
    const other = await geom(4);
    expect(other).toHaveLength(3);
    expect(other.map((b) => b.text)).toEqual(NODES);
    await shot(page, "smartart-from-data");
  });

  test("sub-nodes: a vertical list puts them in a box under the bar, a hierarchy hangs them from their parent", async ({ page }) => {
    const { bytes } = smartArtDocx([
      { layout: "vList2", nodes: [{ text: "Fruit", children: ["Apple", "Pear"] }, "Vegetables"], h: 220 },
      { layout: "orgChart1", nodes: [{ text: "Boss", children: [{ text: "Lead A", children: ["Dev 1", "Dev 2"] }, "Lead B"] }], h: 260 },
    ]);
    await openBytes(page, bytes, "Before the graphic");
    await expect(diagrams(page)).toHaveCount(2);
    const read = (n: number) =>
      diagrams(page)
        .nth(n)
        .evaluate((a) => {
          const o = a.getBoundingClientRect();
          return Array.from(a.querySelectorAll(".dg-shape")).map((g) => {
            const r = window.__fillBox(g.querySelector("path") as SVGGraphicsElement);
            return { x: r.left - o.left, y: r.top - o.top, w: r.width, h: r.height, lines: Array.from(g.querySelectorAll(".dg-text-box > div")).map((d) => d.textContent ?? ""), fill: g.querySelector("path")!.getAttribute("fill") };
          });
        });
    const list = await read(0);
    expect(list).toHaveLength(3);
    // The bar, the box of its sub-nodes under it (a lighter colour, bulleted), then the next bar.
    expect(list[0].lines).toEqual(["Fruit"]);
    expect(list[1].lines.map((l) => l.replace(/^\u2022/, ""))).toEqual(["Apple", "Pear"]);
    expect(list[1].y).toBeGreaterThanOrEqual(list[0].y + list[0].h - 1);
    expect(list[1].fill).not.toBe(list[0].fill);
    expect(list[2].lines).toEqual(["Vegetables"]);
    expect(list[2].y).toBeGreaterThan(list[1].y + list[1].h - 1);
    const tree = await read(1);
    const box = (t: string) => tree.find((b) => b.lines[0] === t)!;
    const mid = (b: { x: number; w: number }) => b.x + b.w / 2;
    // A parent is centred over its children; each level is below the one above.
    expect(mid(box("Boss"))).toBeCloseTo((mid(box("Lead A")) + mid(box("Lead B"))) / 2, 0);
    expect(mid(box("Lead A"))).toBeCloseTo((mid(box("Dev 1")) + mid(box("Dev 2"))) / 2, 0);
    expect(box("Lead A").y).toBeGreaterThan(box("Boss").y + box("Boss").h);
    expect(box("Dev 1").y).toBeGreaterThan(box("Lead A").y + box("Lead A").h);
    expect(box("Dev 1").y).toBeCloseTo(box("Lead B").y + (box("Dev 1").y - box("Lead B").y), 0);
    // Connectors: a stem, a bar and a drop for each child, for the two parents.
    expect(tree.length - 5).toBeGreaterThanOrEqual(8);
    await shot(page, "smartart-hierarchy");
  });

  test("a graphic with nothing to draw says so", async ({ page }) => {
    const { bytes } = smartArtDocx([{ layout: "default", nodes: [] }]);
    await openBytes(page, bytes, "Before the graphic");
    await expect(diagrams(page)).toHaveCount(1);
    await expect(diagrams(page).first().locator(".dg-text-box")).toContainText("SmartArt graphic");
    const at = await where(page, ".flow .atom.shape.diagram");
    expect((await shapeAt(page, at.p, at.o))!.diagram?.source).toBe("placeholder");
  });
});

// ---- WordArt ---------------------------------------------------------------------------------------------

/** The warped text's SVG of the n-th WordArt. */
const warp = (page: Page, n = 0) => wordArts(page).nth(n).locator(".warp-svg");

/** For a sliced warp: where the middle of each slice's text lands in the box (points), by walking the slices' maps. */
function sliceCentres(page: Page, n = 0) {
  return wordArts(page)
    .nth(n)
    .evaluate((a) => {
      const svg = a.querySelector(".warp-svg")!;
      const text = svg.querySelector("defs text")!;
      const th = text.querySelectorAll("tspan").length * 120;
      return Array.from(svg.querySelectorAll("g[clip-path]")).map((g) => {
        const m = /matrix\(([^)]+)\)/.exec(g.getAttribute("transform") ?? "")![1].split(/\s+/).map(Number);
        const clip = svg.querySelector(g.getAttribute("clip-path")!.replace(/^url\(#(.+)\)$/, "#$1") + " rect")!;
        const x = Number(clip.getAttribute("x")) + Number(clip.getAttribute("width")) / 2;
        const y = th / 2;
        return { x: m[0] * x + m[2] * y + m[4], y: m[1] * x + m[3] * y + m[5], a: m[0], b: m[1], c: m[2], d: m[3] };
      });
    });
}

test.describe("WordArt", () => {
  test("arch up puts the text on an arc: a text path, the ends lower than the middle", async ({ page }) => {
    const bytes = wordArtDocx([wordArtRun({ prst: "textArchUp", av: '<a:avLst><a:gd name="adj" fmla="val 10800000"/></a:avLst>', w: 300, h: 150, alt: "Arched heading" })]);
    await openBytes(page, bytes, "Before the WordArt");
    await expect(wordArts(page)).toHaveCount(1);
    await expect(wordArts(page).first()).toHaveAttribute("aria-label", "Arched heading");
    const svg = warp(page);
    await expect(svg).toHaveCount(1);
    await expect(svg.locator("textPath")).toHaveCount(1);
    expect(await svg.locator("defs path").first().getAttribute("d")).toMatch(/^M[\d.]+ [\d.]+A[\d.]+ [\d.]+ 0 0 1 [\d.]+ [\d.]+$/);
    // The first, middle and last letters: the middle one is the highest.
    const ys = await svg.evaluate((s) => {
      const t = s.querySelector("text")!;
      const n = t.getNumberOfChars();
      return [0, Math.floor(n / 2), n - 1].map((i) => t.getStartPositionOfChar(i).y);
    });
    expect(ys[1]).toBeLessThan(ys[0] - 10);
    expect(ys[1]).toBeLessThan(ys[2] - 10);
    // The text of the shape is still its story, hidden under the bent text.
    await expect(wordArts(page).first().locator(".shape-text.warped")).toHaveText("Hello WordArt");
    expect(await wordArts(page).first().locator(".shape-text").evaluate((e) => getComputedStyle(e).visibility)).toBe("hidden");
    await shot(page, "wordart-arch-up");
  });

  test("arch down is the other way round, a circle goes all the way round", async ({ page }) => {
    const bytes = wordArtDocx([wordArtRun({ prst: "textArchDown", w: 300, h: 150, id: 2 }), wordArtRun({ prst: "textCircle", w: 200, h: 200, id: 3, text: "Round and round" })]);
    await openBytes(page, bytes, "Before the WordArt");
    await expect(wordArts(page)).toHaveCount(2);
    const ys = await warp(page, 0).evaluate((s) => {
      const t = s.querySelector("text")!;
      const n = t.getNumberOfChars();
      return [0, Math.floor(n / 2), n - 1].map((i) => t.getStartPositionOfChar(i).y);
    });
    expect(ys[1]).toBeGreaterThan(ys[0] + 10);
    expect(ys[1]).toBeGreaterThan(ys[2] + 10);
    // The circle: letters spread over every side of it.
    const pts = await warp(page, 1).evaluate((s) => {
      const t = s.querySelector("text")!;
      return Array.from({ length: t.getNumberOfChars() }, (_, i) => ({ x: t.getStartPositionOfChar(i).x, y: t.getStartPositionOfChar(i).y }));
    });
    expect(Math.min(...pts.map((p) => p.x))).toBeLessThan(40);
    expect(Math.max(...pts.map((p) => p.x))).toBeGreaterThan(160);
    expect(Math.min(...pts.map((p) => p.y))).toBeLessThan(40);
    expect(Math.max(...pts.map((p) => p.y))).toBeGreaterThan(160);
    await shot(page, "wordart-arch-down-circle");
  });

  test("a wave swings the text up and down, and the slices follow the mesh", async ({ page }) => {
    const bytes = wordArtDocx([wordArtRun({ prst: "textWave1", w: 300, h: 100 }), wordArtRun({ prst: "textWave2", w: 300, h: 100, id: 3 })]);
    await openBytes(page, bytes, "Before the WordArt");
    const one = await sliceCentres(page, 0);
    expect(one.length).toBeGreaterThanOrEqual(24);
    // Wave 1: the centre line rises over the first quarter and falls over the third; the slices keep their height.
    const at = (list: { x: number; y: number }[], f: number) => list.reduce((best, s) => (Math.abs(s.x - 300 * f) < Math.abs(best.x - 300 * f) ? s : best));
    expect(at(one, 0.25).y).toBeLessThan(at(one, 0).y - 5);
    expect(at(one, 0.75).y).toBeGreaterThan(at(one, 0).y + 5);
    expect(Math.abs(at(one, 1).y - at(one, 0).y)).toBeLessThan(6);
    for (const s of one) expect(s.d * 120).toBeCloseTo(one[0].d * 120, 0);
    // Wave 2 is the same wave the other way round.
    const two = await sliceCentres(page, 1);
    expect(at(two, 0.25).y).toBeGreaterThan(at(two, 0).y + 5);
    expect(at(two, 0.75).y).toBeLessThan(at(two, 0).y - 5);
    await shot(page, "wordart-waves");
  });

  test("inflate and deflate change the height of the text across it; slant and fade tilt and narrow it", async ({ page }) => {
    const runs = ["textInflate", "textDeflate", "textSlantUp", "textFadeRight", "textPlain"].map((prst, i) => wordArtRun({ prst, w: 300, h: 90, id: 2 + i, text: "Warped text" }));
    await openBytes(page, wordArtDocx(runs), "Before the WordArt");
    await expect(wordArts(page)).toHaveCount(5);
    // The height of a slice is what its map does to a vertical line of the text.
    const heights = async (n: number) => (await sliceCentres(page, n)).map((s) => Math.hypot(s.c, s.d) * 120);
    const infl = await heights(0);
    expect(infl[Math.floor(infl.length / 2)]).toBeGreaterThan(infl[0] * 1.2);
    const defl = await heights(1);
    expect(defl[Math.floor(defl.length / 2)]).toBeLessThan(defl[0] * 0.85);
    const slant = await sliceCentres(page, 2);
    expect(slant[slant.length - 1].y).toBeLessThan(slant[0].y - 5);
    const fade = await heights(3);
    expect(fade[fade.length - 1]).toBeLessThan(fade[0] * 0.6);
    // Plain text is not cut into slices.
    expect(await warp(page, 4).locator("g[clip-path]").count()).toBe(0);
    expect(await warp(page, 4).locator("text").count()).toBe(1);
    await shot(page, "wordart-shapes");
  });

  test("the look of the text: gradient fill, outline, shadow; the box has none of its own", async ({ page }) => {
    const rpr =
      '<w14:textOutline w14:w="19050" w14:cap="flat" w14:cmpd="sng" w14:algn="ctr"><w14:solidFill><w14:srgbClr w14:val="1F3864"/></w14:solidFill><w14:prstDash w14:val="solid"/><w14:round/></w14:textOutline>' +
      '<w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="FF0000"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="FFC000"/></w14:gs></w14:gsLst><w14:lin w14:ang="5400000" w14:scaled="0"/></w14:gradFill></w14:textFill>' +
      '<w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000" w14:sx="100000" w14:sy="100000" w14:kx="0" w14:ky="0" w14:algn="tl" w14:rotWithShape="0"><w14:srgbClr w14:val="000000"><w14:alpha w14:val="40000"/></w14:srgbClr></w14:shadow>';
    await openBytes(page, wordArtDocx([wordArtRun({ prst: "textInflate", rpr, w: 300, h: 100, size: 54 })]), "Before the WordArt");
    const svg = warp(page);
    const text = svg.locator("defs text");
    const fill = (await text.getAttribute("fill"))!;
    expect(fill).toMatch(/^url\(#fa\d+\)$/);
    const grad = svg.locator(`defs ${fill.replace(/^url\((.*)\)$/, "$1")}`);
    await expect(grad.locator("stop")).toHaveCount(2);
    expect(await grad.locator("stop").first().getAttribute("stop-color")).toBe("#FF0000");
    expect(await grad.locator("stop").last().getAttribute("stop-color")).toBe("#FFC000");
    expect(await text.getAttribute("stroke")).toBe("#1F3864");
    expect(Number(await text.getAttribute("stroke-width"))).toBeGreaterThan(0.5);
    await expect(svg.locator("filter feOffset")).toHaveCount(1);
    expect(await svg.locator("filter feFlood").getAttribute("flood-opacity")).toBe("0.4");
    // The shape itself draws no rectangle.
    expect(await wordArts(page).first().locator("> svg.shape-svg:not(.warp-svg) path[fill]:not([fill='none'])").count()).toBe(0);
    await shot(page, "wordart-look");
  });

  test("the text of DrawingML WordArt is edited as plain text; the bend comes back after", async ({ page }) => {
    await openBytes(page, wordArtDocx([wordArtRun({ prst: "textArchUpPour", w: 300, h: 120, text: "Heading" })]), "Before the WordArt");
    const art = wordArts(page).first();
    const at = await where(page, ".flow .atom.shape.wordart");
    await expect(warp(page)).toBeVisible();
    await art.click();
    await art.dblclick();
    // While editing, the bend is dropped and the plain text shows and takes the keys.
    await expect(art).toHaveClass(/warp-off/);
    await expect(warp(page)).toBeHidden();
    await expect(art.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    expect(await art.locator(".shape-text").evaluate((e) => getComputedStyle(e).visibility)).toBe("visible");
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.type("New title");
    await expect.poll(async () => ((await shapeAt(page, at.p, at.o))!.text ?? []).map((b) => b.segs.map((s) => s.x ?? "").join("")).join("|")).toBe("New title");
    // Leaving the text draws the bend again, with the new words.
    await page.keyboard.press("Escape");
    await expect(art).not.toHaveClass(/warp-off/);
    await expect(warp(page)).toBeVisible();
    expect(await warp(page).locator("defs text").textContent()).toBe("New title");
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.warp?.preset).toBe("textArchUpPour");
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('prst="textArchUpPour"');
    expect(xml).toContain("New title");
    await shot(page, "wordart-arch-pour");
  });

  test("VML WordArt shows the text of its textpath, in the shape's fill, bent by its shape type", async ({ page }) => {
    const bytes = wordArtDocx([
      `<w:r><w:t xml:space="preserve">Old WordArt: </w:t></w:r>${vmlWordArt({ spt: 144, text: "Sale today", w: 300, h: 130, fill: "#c00000", stroke: "#600000" })}`,
      vmlWordArt({ spt: 156, text: "Wave", id: 2, w: 260, h: 80, fill: "#2f5496", extra: '<v:shadow on="t" color="silver" offset="3pt,3pt"/>' }),
    ]);
    await openBytes(page, bytes, "Before the WordArt");
    await expect(wordArts(page)).toHaveCount(2);
    const a = (await shapeAt(page, ...(Object.values(await where(page, ".flow .atom.shape.wordart", 0)) as [number, number])))!;
    expect(a.warp?.preset).toBe("textArchUp");
    expect(await warp(page, 0).locator("textPath").textContent()).toBe("Sale today");
    const text = warp(page, 0).locator("defs ~ g text, text").first();
    expect(await text.getAttribute("fill")).toBe("#C00000");
    expect(await text.getAttribute("stroke")).toBe("#600000");
    // The second is a wave: sliced, with a shadow.
    expect(await warp(page, 1).locator("g[clip-path]").count()).toBeGreaterThanOrEqual(24);
    await expect(warp(page, 1).locator("filter feOffset")).toHaveCount(1);
    // No grey chip any more.
    await expect(page.locator(".flow .atom.obj")).toHaveCount(0);
    await shot(page, "wordart-vml");
  });

  test("a text watermark is still not an object in the text", async ({ page }) => {
    const mark =
      '<w:r><w:pict><v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/></v:shapetype><v:shape id="PowerPlusWaterMarkObject1" o:spid="_x0000_s2049" type="#_x0000_t136" style="position:absolute;margin-left:0;margin-top:0;width:412pt;height:247pt;rotation:315;z-index:-251658752;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" fillcolor="silver" stroked="f"><v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="DRAFT"/></v:shape></w:pict></w:r><w:r><w:t>Body text</w:t></w:r>';
    await openBytes(page, wordArtDocx([mark]), "Before the WordArt");
    await expect(page.locator(".flow .atom.shape")).toHaveCount(0);
    await expect(page.locator(".flow .atom.obj")).toHaveCount(0);
  });

  test("resizing WordArt stretches the bent text with the box", async ({ page }) => {
    await openBytes(page, wordArtDocx([wordArtRun({ prst: "textWave1", w: 300, h: 100 })]), "Before the WordArt");
    const at = await where(page, ".flow .atom.shape.wordart");
    await run(page, { op: "set_shape", at, w: 450, h: 100 });
    const box = (await wordArts(page).first().boundingBox())!;
    const z = await zoomOf(page);
    expect(box.width).toBeCloseTo(450 * PT_PX * z, 0);
    const viewBox = await warp(page).getAttribute("viewBox");
    expect(viewBox).toBe("0 0 450 100");
    const slicesNow = await warp(page).locator("g[clip-path]").count();
    expect(slicesNow).toBeGreaterThan(40);
  });
});

/** The origin of a bounding box, for comparing rectangles on the page. */
function diagramsOrigin(box: { x: number; y: number }) {
  return { x: box.x, y: box.y };
}
