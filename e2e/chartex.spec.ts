// The new chart types Word 2016 added (waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst,
// region map): a document with one draws it from its data, in the look of the classic charts; what Docxtor can't
// draw shows the picture Word keeps beside it; the chart is selected, resized, wrapped and deleted like a chart
// or a picture; and saving keeps the chart part, its workbook and the picture byte for byte.
import { buildChartExDocx, CHARTEX_FALLBACK_RGB, chartexXml, type ChartExDoc, type ChartExName, type ChartExOptions } from "./support/docx-builder";
import { contextItem, contextMenu, dialog, expect, inPage, openFixture, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";
/** Set DOCXTOR_SHOTS to a folder to keep a picture of each chart. */
const SHOTS = process.env.DOCXTOR_SHOTS;

interface Opened {
  parts: Record<string, string | Uint8Array>;
  bytes: Uint8Array;
}

/** Open a document with a chartex chart in place of what is on screen. */
async function openChartEx(page: Page, xml: string, doc: ChartExDoc = {}): Promise<Opened> {
  const built = buildChartExDocx(xml, doc);
  await page.evaluate(async (b) => {
    const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
    await w.fe.open(new Uint8Array(b), "chartex.docx");
  }, Array.from(built.bytes));
  await expect(page.locator(".flow .fp").first()).toContainText(doc.before ?? "Before the chart");
  await page.evaluate(() => document.fonts.ready);
  return built;
}

const kindOf = (kind: ChartExName, o: ChartExOptions = {}, d: ChartExDoc = {}) => (page: Page) => openChartEx(page, chartexXml(kind, o), d);

const charts = (page: Page) => page.locator(".flow .atom.chart");
const svgOf = (c: Locator) => c.locator(".chart-svg");
const texts = (c: Locator) => c.locator(".chart-svg text").allTextContents();

interface Bar {
  kind: string | null;
  x: number;
  y: number;
  w: number;
  h: number;
  fill: string;
}
const bars = (c: Locator, sel = ".chartex-bar"): Promise<Bar[]> =>
  c.locator(sel).evaluateAll((els) =>
    els.map((e) => ({ kind: e.getAttribute("data-kind"), x: Number(e.getAttribute("x")), y: Number(e.getAttribute("y")), w: Number(e.getAttribute("width")), h: Number(e.getAttribute("height")), fill: getComputedStyle(e).fill })),
  );

/** Keep a picture of the chart for looking at, when asked. */
async function shot(c: Locator, name: string) {
  if (SHOTS) await svgOf(c).screenshot({ path: `${SHOTS}/chartex-${name}.png` });
}

/** The parts of the saved file, decompressed, as base64. */
async function savedParts(page: Page): Promise<Record<string, string>> {
  return page.evaluate(async () => {
    const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } } } } }).docxtor;
    const bytes = w.fe.editor.engine.save();
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let end = bytes.length - 22;
    while (dv.getUint32(end, true) !== 0x06054b50) end--;
    let p = dv.getUint32(end + 16, true);
    const count = dv.getUint16(end + 10, true);
    const out: Record<string, string> = {};
    for (let i = 0; i < count; i++) {
      const method = dv.getUint16(p + 10, true);
      const csize = dv.getUint32(p + 20, true);
      const nlen = dv.getUint16(p + 28, true);
      const xlen = dv.getUint16(p + 30, true);
      const clen = dv.getUint16(p + 32, true);
      const off = dv.getUint32(p + 42, true);
      const name = new TextDecoder().decode(bytes.subarray(p + 46, p + 46 + nlen));
      p += 46 + nlen + xlen + clen;
      const dataStart = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(dataStart, dataStart + csize);
      let raw: Uint8Array = data;
      if (method !== 0) raw = new Uint8Array(await new Response(new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"))).arrayBuffer());
      let bin = "";
      for (const b of raw) bin += String.fromCharCode(b);
      out[name] = btoa(bin);
    }
    return out;
  });
}

const b64 = (v: string | Uint8Array) => Buffer.from(typeof v === "string" ? new TextEncoder().encode(v) : v).toString("base64");
const decode = (v: string) => Buffer.from(v, "base64").toString("utf8");

/** Select a chart with a click and wait for its frame. */
async function select(page: Page, c: Locator) {
  await c.scrollIntoViewIfNeeded();
  await c.locator(".chart-svg, img").first().click({ position: { x: 20, y: 20 } });
  await expect(page.locator(".img-frame")).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await openFixture(page, "simple.docx");
});

test.describe("waterfall", () => {
  test("bars rise and fall from the running total, and totals stand on the axis", async ({ page }) => {
    await kindOf("waterfall", { labels: true })(page);
    const c = charts(page).first();
    await expect(svgOf(c)).toHaveAttribute("data-chartex", "waterfall");
    const b = await bars(c);
    expect(b.map((x) => x.kind)).toEqual(["total", "increase", "decrease", "decrease", "decrease", "total"]);
    // Heights follow the size of each step: 100, 80, 20, 35, 10 and 115.
    const unit = b[0]!.h / 100;
    [100, 80, 20, 35, 10, 115].forEach((v, i) => expect(b[i]!.h, `bar ${i}`).toBeCloseTo(v * unit, 0));
    // The first total stands on the axis; each step starts where the one before ended.
    const zero = b[0]!.y + b[0]!.h;
    expect(b[5]!.y + b[5]!.h).toBeCloseTo(zero, 0);
    expect(b[1]!.y + b[1]!.h).toBeCloseTo(b[0]!.y, 0); // 100 up to 180: starts at 100
    expect(b[2]!.y).toBeCloseTo(b[1]!.y, 0); // 180 down to 160: starts at the top
    expect(b[3]!.y).toBeCloseTo(b[2]!.y + b[2]!.h, 0);
    expect(b[4]!.y).toBeCloseTo(b[3]!.y + b[3]!.h, 0);
    expect(b[5]!.y).toBeCloseTo(zero - 115 * unit, 0);
    // Bars are as wide as a half-width gap leaves them, in even steps.
    expect(b[1]!.x - b[0]!.x).toBeCloseTo(b[2]!.x - b[1]!.x, 0);
    expect(b[0]!.w / (b[1]!.x - b[0]!.x)).toBeCloseTo(1 / 1.5, 1);
    // The colours: Word's palette for increase, decrease and total; the file's own colour for point 2.
    expect(b[1]!.fill).toBe("rgb(68, 114, 196)");
    expect(b[3]!.fill).toBe("rgb(237, 125, 49)");
    expect(b[0]!.fill).toBe("rgb(165, 165, 165)");
    expect(b[2]!.fill).toBe("rgb(192, 0, 0)");
    // A line joins each bar to the next at the level it reached.
    const lines = await c.locator(".chartex-connector").evaluateAll((els) => els.map((e) => ({ x1: Number(e.getAttribute("x1")), x2: Number(e.getAttribute("x2")), y: Number(e.getAttribute("y1")) })));
    expect(lines).toHaveLength(5);
    expect(lines[0]!.y).toBeCloseTo(b[0]!.y, 0);
    expect(lines[1]!.y).toBeCloseTo(b[1]!.y, 0);
    expect(lines[0]!.x1).toBeCloseTo(b[0]!.x + b[0]!.w, 0);
    expect(lines[0]!.x2).toBeCloseTo(b[1]!.x, 0);
    const t = await texts(c);
    expect(t).toEqual(expect.arrayContaining(["Cash flow", "Start", "Sales", "Returns", "Costs", "Tax", "Profit", "Increase", "Decrease", "Total", "100", "80", "-20", "-35", "-10", "115"]));
    await shot(c, "waterfall");
  });

  test("connector lines can be turned off, and the chart is labelled for screen readers", async ({ page }) => {
    await kindOf("waterfall", { connectors: false, labels: false })(page);
    const c = charts(page).first();
    await expect(c.locator(".chartex-connector")).toHaveCount(0);
    await expect(c.locator(".chartex-bar")).toHaveCount(6);
    await expect(svgOf(c)).toHaveAttribute("role", "img");
    await expect(svgOf(c)).toHaveAttribute("aria-label", "Waterfall chart: Cash flow. 6 points.");
    const table = c.locator(".chart-sr table.chart-data");
    await expect(table.locator("caption")).toHaveText("Cash flow");
    await expect(table.locator("tr")).toHaveCount(7);
    await expect(table.locator("tr").nth(3)).toHaveText("Returns-20160");
    // No data labels asked for: none drawn.
    await expect(c.locator(".chart-label")).toHaveCount(0);
  });
});

test.describe("histogram", () => {
  test("bars count the values in each bin, with the bin's range under it", async ({ page }) => {
    await kindOf("histogram")(page);
    const c = charts(page).first();
    const b = await bars(c);
    expect(b).toHaveLength(4);
    const unit = b[0]!.h / 11;
    [11, 6, 2, 1].forEach((v, i) => expect(b[i]!.h, `bin ${i}`).toBeCloseTo(v * unit, 0));
    // Side by side, with no space between (the file says gapWidth 0), standing on one line.
    for (let i = 0; i < 3; i++) expect(b[i]!.x + b[i]!.w).toBeCloseTo(b[i + 1]!.x, 0);
    const bottoms = b.map((x) => x.y + x.h);
    bottoms.forEach((v) => expect(v).toBeCloseTo(bottoms[0]!, 0));
    // Excel's range labels: the first bin includes both ends, the others only the upper one.
    const labels = await c.locator(".chartex-cats text").allTextContents();
    expect(labels).toEqual(["[1, 6]", "(6, 11]", "(11, 16]", "(16, 21]"]);
    // Counts are whole numbers on the axis.
    const ticks = (await texts(c)).filter((x) => /^\d+$/.test(x));
    expect(ticks.length).toBeGreaterThan(2);
    // A bar's tip says what it counts.
    await expect(c.locator(".chartex-bar").first().locator("title")).toHaveText("[1, 6]: 11");
    await shot(c, "histogram");
  });

  test("a bin count, intervals closed on the left, and overflow and underflow bins", async ({ page }) => {
    await kindOf("histogram", { binning: `<cx:binCount val="4"/>`, labels: true })(page);
    let c = charts(page).first();
    let b = await bars(c);
    expect(b).toHaveLength(4);
    // 1 to 20 in four: 4.75 wide.
    expect(await c.locator(".chartex-cats text").allTextContents()).toEqual(["[1, 5.75]", "(5.75, 10.5]", "(10.5, 15.25]", "(15.25, 20]"]);
    expect(await texts(c)).toEqual(expect.arrayContaining(["10", "6", "3", "1"]));
    expect(b.reduce((a, x) => a + x.h, 0)).toBeGreaterThan(0);

    await kindOf("histogram", { binAttrs: 'underflow="3" overflow="12"', binning: `<cx:binSize val="3"/>` })(page);
    c = charts(page).first();
    b = await bars(c);
    expect(b.map((x) => x.kind)).toEqual(["underflow", "bin", "bin", "bin", "overflow"]);
    const labels = await c.locator(".chartex-cats text").allTextContents();
    expect(labels[0]).toBe("≤3");
    expect(labels[1]).toBe("(3, 6]");
    expect(labels.at(-1)).toBe(">12");
    // Six values at most 3 (1, 2, 2, 3, 3, 3), five in (3, 6] (4, 4, 5, 5, 6), two above 12 (14, 20).
    expect(b[0]!.h / b[1]!.h).toBeCloseTo(6 / 5, 1);
    expect(b[0]!.h / b[4]!.h).toBeCloseTo(3, 1);
  });

  test("automatic bins follow Scott's rule", async ({ page }) => {
    await kindOf("histogram", { binning: "" })(page);
    const c = charts(page).first();
    const b = await bars(c);
    // Width 3.49 s / n^(1/3) = 6.17 over a span of 19: four bins; every value is counted once.
    expect(b).toHaveLength(4);
    const unit = b[3]!.h / 1;
    const total = b.reduce((a, x) => a + x.h / unit, 0);
    expect(Math.round(total)).toBe(20);
  });
});

test.describe("Pareto", () => {
  test("bars are sorted largest first, with the cumulative percent on a second axis", async ({ page }) => {
    await kindOf("pareto")(page);
    const c = charts(page).first();
    await expect(svgOf(c)).toHaveAttribute("data-chartex", "pareto");
    const b = await bars(c);
    const unit = b[0]!.h / 40;
    [40, 25, 15, 12, 8].forEach((v, i) => expect(b[i]!.h, `bar ${i}`).toBeCloseTo(v * unit, 0));
    expect(await c.locator(".chartex-cats text").allTextContents()).toEqual(["Dent", "Stain", "Other", "Scratch", "Crack"]);
    // The percent axis on the right runs 0 to 100%.
    const t = await texts(c);
    expect(t).toEqual(expect.arrayContaining(["0%", "20%", "40%", "60%", "80%", "100%", "Defects", "Cumulative %"]));
    const axisY = async (label: string) => c.locator(".chart-svg text", { hasText: new RegExp(`^${label}$`) }).evaluateAll((els) => els.map((e) => Number(e.getAttribute("y")) - 4.2));
    const y0 = (await axisY("0%"))[0]!;
    const y100 = (await axisY("100%"))[0]!;
    const marks = await c.locator(".chartex-pareto .chart-marker").evaluateAll((els) => els.map((e) => Number(e.getAttribute("cy"))));
    expect(marks).toHaveLength(5);
    [0.4, 0.65, 0.8, 0.92, 1].forEach((v, i) => expect((y0 - marks[i]!) / (y0 - y100), `point ${i}`).toBeCloseTo(v, 1));
    // The line's colour is the file's.
    const stroke = await c.locator(".chartex-pareto path.chart-stroke").evaluate((e) => getComputedStyle(e).stroke);
    expect(stroke).toBe("rgb(237, 125, 49)");
    await shot(c, "pareto");
  });
});

test.describe("box and whisker", () => {
  test("a box per group: quartiles, whiskers, the mean marker and the outlier", async ({ page }) => {
    await kindOf("box", { quartile: "exclusive", meanLine: true })(page);
    const c = charts(page).first();
    const boxes = await bars(c, ".chartex-box-body");
    expect(boxes).toHaveLength(2);
    // Ward A: 4 to 11 and an outlier at 30, quartiles 5.5 and 10.5 (exclusive). Ward B: 1 to 8, quartiles 2.25 and 6.75.
    const unit = boxes[0]!.h / 5;
    expect(boxes[1]!.h).toBeCloseTo(4.5 * unit, 0);
    // Ward B's box starts 3.75 below Ward A's top... in value terms: tops at 10.5 and 6.75.
    expect(boxes[0]!.y - boxes[1]!.y).toBeCloseTo(-(10.5 - 6.75) * unit, 0);
    // The median line of Ward A, at 8, sits 2.5 under the top of the box.
    const medians = await c.locator(".chartex-median").evaluateAll((els) => els.map((e) => Number(e.getAttribute("y1"))));
    expect(medians[0]! - boxes[0]!.y).toBeCloseTo(2.5 * unit, 0);
    expect(medians[1]! - boxes[1]!.y).toBeCloseTo(2.25 * unit, 0);
    // Whiskers: the upper one of Ward A ends at 11 (0.5 above the box), the lower at 4 (1.5 under the 5.5).
    const whiskers = await c.locator(".chartex-box").first().locator(".chartex-whisker").evaluateAll((els) => els.map((e) => ({ y1: Number(e.getAttribute("y1")), y2: Number(e.getAttribute("y2")), x1: Number(e.getAttribute("x1")), x2: Number(e.getAttribute("x2")) })));
    expect(whiskers).toHaveLength(4);
    expect(boxes[0]!.y - whiskers[0]!.y2).toBeCloseTo(0.5 * unit, 0);
    expect(whiskers[2]!.y2 - (boxes[0]!.y + boxes[0]!.h)).toBeCloseTo(1.5 * unit, 0);
    // The outlier at 30 is 19.5 above the box.
    const outlier = await c.locator(".chartex-outlier").evaluateAll((els) => els.map((e) => Number(e.getAttribute("cy"))));
    expect(outlier).toHaveLength(1);
    expect((boxes[0]!.y - outlier[0]!) / unit).toBeCloseTo(19.5, 0);
    // The mean marker (an X) of each box, and a line between the means.
    await expect(c.locator(".chartex-mean")).toHaveCount(2);
    await expect(c.locator(".chartex-mean-line")).toHaveCount(1);
    expect(await c.locator(".chartex-cats text").allTextContents()).toEqual(["Ward A", "Ward B"]);
    await shot(c, "box-exclusive");
  });

  test("the inclusive quartile method gives other quartiles, and the options hide the outlier and show the points", async ({ page }) => {
    await kindOf("box", { quartile: "inclusive", outliers: false, points: true })(page);
    const c = charts(page).first();
    const boxes = await bars(c, ".chartex-box-body");
    // Ward A inclusive: 6 to 10 (4 wide); Ward B: 2.75 to 6.25 (3.5 wide).
    expect(boxes[1]!.h / boxes[0]!.h).toBeCloseTo(3.5 / 4, 1);
    await expect(c.locator(".chartex-outlier")).toHaveCount(0);
    await expect(c.locator(".chartex-point")).toHaveCount(8 + 8);
    await expect(c.locator(".chartex-mean-line")).toHaveCount(0);
    await shot(c, "box-inclusive");
  });
});

test.describe("funnel", () => {
  test("centred bars, as wide as their values, under their names", async ({ page }) => {
    await kindOf("funnel")(page);
    const c = charts(page).first();
    const b = await bars(c);
    expect(b).toHaveLength(4);
    const unit = b[0]!.w / 1000;
    [1000, 400, 150, 60].forEach((v, i) => expect(b[i]!.w, `stage ${i}`).toBeCloseTo(v * unit, 0));
    const centres = b.map((x) => x.x + x.w / 2);
    centres.forEach((v) => expect(v).toBeCloseTo(centres[0]!, 0));
    // One under the other, top to bottom, the same height.
    expect(b[1]!.y).toBeGreaterThan(b[0]!.y + b[0]!.h - 0.5);
    expect(b[0]!.h).toBeCloseTo(b[3]!.h, 1);
    const t = await texts(c);
    expect(t).toEqual(expect.arrayContaining(["Sales funnel", "Visits", "Sign-ups", "Trials", "Paid", "1000", "400", "150", "60"]));
    await shot(c, "funnel");
  });
});

test.describe("treemap", () => {
  test("tiles fill the chart: each leaf's area follows its value inside its branch", async ({ page }) => {
    await kindOf("treemap")(page);
    const c = charts(page).first();
    const tiles = await c.locator(".chartex-tile").evaluateAll((els) =>
      els.map((e) => ({ name: e.getAttribute("data-name")!, depth: Number(e.getAttribute("data-depth")), w: Number(e.getAttribute("width")), h: Number(e.getAttribute("height")), x: Number(e.getAttribute("x")), y: Number(e.getAttribute("y")), fill: getComputedStyle(e).fill })),
    );
    const by = new Map(tiles.map((t) => [t.name, t]));
    expect(tiles).toHaveLength(7);
    const area = (n: string) => by.get(n)!.w * by.get(n)!.h;
    // Branches share the chart 8 to 7; leaves share their branch's body in proportion.
    expect(area("Fruit") / area("Veg")).toBeCloseTo(8 / 7, 2);
    expect(area("Apple") / area("Pear")).toBeCloseTo(4 / 3, 2);
    expect(area("Pear") / area("Plum")).toBeCloseTo(3, 2);
    expect(area("Leek") / area("Kale")).toBeCloseTo(5 / 2, 2);
    // Each leaf lies inside its branch.
    for (const [leaf, parent] of [["Apple", "Fruit"], ["Plum", "Fruit"], ["Kale", "Veg"], ["Leek", "Veg"]] as const) {
      const l = by.get(leaf)!;
      const p = by.get(parent)!;
      expect(l.x).toBeGreaterThanOrEqual(p.x - 0.01);
      expect(l.y).toBeGreaterThanOrEqual(p.y - 0.01);
      expect(l.x + l.w).toBeLessThanOrEqual(p.x + p.w + 0.01);
      expect(l.y + l.h).toBeLessThanOrEqual(p.y + p.h + 0.01);
    }
    // The branch has a banner with its name; the leaves carry theirs.
    const t = await texts(c);
    expect(t).toEqual(expect.arrayContaining(["Produce", "Fruit", "Veg", "Apple", "Pear", "Kale", "Leek"]));
    // The two branches have different colours.
    expect(by.get("Fruit")!.fill).not.toBe(by.get("Veg")!.fill);
    // Names are white on the dark blue banner and page ink on the orange one, whichever reads better.
    const textFill = (name: string) => c.locator(".chartex-parent-name", { hasText: name }).evaluate((e) => getComputedStyle(e).fill);
    expect(await textFill("Fruit")).toBe("rgb(255, 255, 255)");
    expect(await textFill("Veg")).toBe("rgb(32, 21, 21)");
    await shot(c, "treemap");
  });

  test("the file may list the leaf level first, and the names of branches can overlap or be left out", async ({ page }) => {
    await kindOf("treemap", { leafFirst: true, parentLabels: "none" })(page);
    let c = charts(page).first();
    expect(await c.locator(".chartex-tile.parent").evaluateAll((els) => els.map((e) => e.getAttribute("data-name")))).toEqual(["Fruit", "Veg"]);
    expect(await texts(c)).not.toContain("Fruit");
    expect(await texts(c)).toEqual(expect.arrayContaining(["Apple", "Leek"]));
    await kindOf("treemap", { parentLabels: "overlapping" })(page);
    c = charts(page).first();
    expect(await texts(c)).toEqual(expect.arrayContaining(["Fruit", "Veg"]));
    await kindOf("treemap", { labels: true })(page);
    c = charts(page).first();
    expect(await texts(c)).toEqual(expect.arrayContaining(["4", "5"]));
  });
});

test.describe("sunburst", () => {
  test("rings per level, with arcs in proportion to their values and names where they fit", async ({ page }) => {
    await kindOf("sunburst")(page);
    const c = charts(page).first();
    const arcs = await c.locator(".chartex-arc").evaluateAll((els) => els.map((e) => ({ name: e.getAttribute("data-name")!, depth: Number(e.getAttribute("data-depth")), a0: Number(e.getAttribute("data-a0")), a1: Number(e.getAttribute("data-a1")), value: Number(e.getAttribute("data-value")) })));
    expect(arcs).toHaveLength(7);
    const by = new Map(arcs.map((a) => [a.name, a]));
    const sweep = (n: string) => by.get(n)!.a1 - by.get(n)!.a0;
    expect(sweep("Fruit")).toBeCloseTo((Math.PI * 2 * 8) / 15, 3);
    expect(sweep("Veg")).toBeCloseTo((Math.PI * 2 * 7) / 15, 3);
    expect(sweep("Apple") + sweep("Pear") + sweep("Plum")).toBeCloseTo(sweep("Fruit"), 3);
    expect(sweep("Leek") / sweep("Kale")).toBeCloseTo(5 / 2, 3);
    expect(by.get("Fruit")!.depth).toBe(1);
    expect(by.get("Leek")!.depth).toBe(2);
    // The first arc starts at twelve o'clock and they run clockwise.
    expect(by.get("Fruit")!.a0).toBeCloseTo(-Math.PI / 2, 3);
    expect(by.get("Veg")!.a0).toBeCloseTo(by.get("Fruit")!.a1, 3);
    const t = await texts(c);
    expect(t).toEqual(expect.arrayContaining(["Produce by kind", "Fruit", "Veg", "Apple", "Leek"]));
    // Names along the radius are turned to read left to right.
    const turned = await c.locator(".chart-labels text[transform]").evaluateAll((els) => els.map((e) => /rotate\(([-\d.]+)/.exec(e.getAttribute("transform")!)![1]));
    expect(turned.length).toBeGreaterThan(0);
    await shot(c, "sunburst");
  });

  test("a legend names the branches in their colours", async ({ page }) => {
    await kindOf("sunburst", { legend: "r" })(page);
    const c = charts(page).first();
    expect(await texts(c)).toEqual(expect.arrayContaining(["Fruit", "Veg"]));
    await expect(svgOf(c).locator("rect[fill^='var(--chart-']")).toHaveCount(2);
  });
});

test.describe("what Docxtor does not draw", () => {
  test("a region map shows the picture Word saved with it", async ({ page }) => {
    for (const fallback of ["picture", "vml"] as const) {
      await kindOf("region", {}, { fallback })(page);
      const pic = page.locator(".flow .atom.img img");
      await expect(pic).toHaveCount(1);
      await expect(page.locator(".flow .chart-svg")).toHaveCount(0);
      // It is the picture in the file: its centre pixel has the colour the test painted it.
      const px = await pic.evaluate(async (img: HTMLImageElement) => {
        await img.decode();
        const cv = document.createElement("canvas");
        cv.width = img.naturalWidth;
        cv.height = img.naturalHeight;
        const g = cv.getContext("2d")!;
        g.drawImage(img, 0, 0);
        return Array.from(g.getImageData(cv.width / 2, cv.height / 2, 1, 1).data.slice(0, 3));
      });
      expect(px).toEqual(CHARTEX_FALLBACK_RGB);
      const box = (await pic.boundingBox())!;
      expect(Math.round(box.width)).toBe(576); // 432 pt
      expect(Math.round(box.height)).toBe(336); // 252 pt
    }
  });

  test("a region map with no picture is a placeholder with its title", async ({ page }) => {
    await kindOf("region", {}, { fallback: "none" })(page);
    const c = charts(page).first();
    await expect(c.locator(".chart-ph")).toHaveCount(1);
    expect(await texts(c)).toEqual(expect.arrayContaining(["Sales by country", "Docxtor keeps this chart as it is."]));
    await shot(c, "region-placeholder");
  });

  test("a file whose chart part is missing still shows its picture", async ({ page }) => {
    await kindOf("waterfall", {}, { noPart: true })(page);
    await expect(page.locator(".flow .atom.img img")).toHaveCount(1);
    await expect(page.locator(".flow .chart-svg")).toHaveCount(0);
  });

  test("a damaged chart part shows the picture, or a placeholder when there is none", async ({ page }) => {
    await openChartEx(page, "<cx:chartSpace");
    await expect(page.locator(".flow .atom.img img")).toHaveCount(1);
    await openChartEx(page, "<cx:chartSpace", { fallback: "none" });
    await expect(charts(page).first().locator(".chart-ph")).toHaveCount(1);
  });

  test("the native drawing is used when both the data and the picture are there", async ({ page }) => {
    await kindOf("treemap", {}, { fallback: "picture" })(page);
    await expect(charts(page).first().locator(".chartex-tile")).toHaveCount(7);
    await expect(page.locator(".flow .atom.img img")).toHaveCount(0);
  });
});

test.describe("an object in the text", () => {
  test("it is as big as the drawing says, and a click selects it with the handles of a picture", async ({ page }) => {
    await kindOf("waterfall", {}, { extent: [4572000, 2743200] })(page);
    const c = charts(page).first();
    const box = (await svgOf(c).boundingBox())!;
    expect(Math.round(box.width)).toBe(480); // 360 pt
    expect(Math.round(box.height)).toBe(288); // 216 pt
    await select(page, c);
    // Eight resize handles and the turn handle.
    await expect(page.locator(".img-handle")).toHaveCount(9);
    await expect(page.locator(".img-strip").getByRole("button", { name: "Edit chart" })).toBeVisible();
    // It is one character: the strip deletes it, and undo brings it back.
    await page.locator(".img-strip").getByRole("button", { name: "Delete chart" }).click();
    await expect(charts(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(charts(page)).toHaveCount(1);
  });

  test("dragging a handle draws it again at the new size, and the file keeps the chart as it was", async ({ page }) => {
    const { parts } = await kindOf("histogram")(page);
    const c = charts(page).first();
    await select(page, c);
    const before = (await svgOf(c).boundingBox())!;
    const handle = page.locator(".img-handle.e");
    const hb = (await handle.boundingBox())!;
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
    await page.mouse.down();
    // 108 pt narrower.
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const drag = 144 * zoom;
    await page.mouse.move(hb.x + hb.width / 2 - drag, hb.y + hb.height / 2, { steps: 5 });
    // Still dragging: the chart is drawn again for the new width, not stretched.
    const viewBox = await svgOf(c).getAttribute("viewBox");
    expect(Number(viewBox!.split(" ")[2])).toBeCloseTo(576 - 144, 0);
    const during = await bars(c);
    expect(during).toHaveLength(4);
    expect(during[0]!.w).toBeLessThan(before.width / 4);
    await page.mouse.up();
    const after = (await svgOf(c).boundingBox())!;
    expect(after.width).toBeCloseTo(before.width - drag, 0);
    expect(Math.round(after.height)).toBe(Math.round(before.height));
    // Still drawn natively, now 324 pt wide.
    await expect(charts(page).first()).toHaveAttribute("data-w", "324");
    const saved = await savedParts(page);
    // The chart, its relationships, its workbook and the picture are the bytes that were opened.
    for (const name of ["word/charts/chartEx1.xml", "word/embeddings/Microsoft_Excel_Sheet1.xlsx", "word/media/image1.png"]) expect(saved[name], name).toBe(b64(parts[name]!));
    const doc = decode(saved["word/document.xml"]!);
    expect(doc).toContain("<mc:AlternateContent");
    expect(doc).toContain('Requires="cx1"');
    expect(doc).toContain("<mc:Fallback>");
    expect(doc.match(/<wp:extent cx="4114800" cy="3200400"\/>/g)).toHaveLength(2);
    expect(doc).not.toContain('cx="5486400"');
    // Reopened, it is a chart of the same size.
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } }; open(b: Uint8Array, n?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(w.fe.editor.engine.save(), "again.docx");
    });
    await expect(charts(page).first().locator(".chartex-bar")).toHaveCount(4);
    await expect(charts(page).first()).toHaveAttribute("data-w", "324");
  });

  test("saving without touching it keeps every part of the chart byte for byte", async ({ page }) => {
    const { parts } = await kindOf("box")(page);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.type("Edited. ");
    const saved = await savedParts(page);
    for (const name of Object.keys(parts)) expect(saved[name], name).toBe(b64(parts[name]!));
    expect(decode(saved["word/charts/_rels/chartEx1.xml.rels"]!)).toContain("Microsoft_Excel_Sheet1.xlsx");
    expect(decode(saved["[Content_Types].xml"]!)).toContain("application/vnd.ms-office.chartex+xml");
    expect(decode(saved["word/document.xml"]!)).toContain("Edited. ");
  });

  test("wrapping and alignment from the strip place it like a picture", async ({ page }) => {
    await kindOf("funnel")(page);
    const c = charts(page).first();
    await select(page, c);
    const strip = page.locator(".img-strip");
    await strip.getByRole("button", { name: "Wrap" }).click();
    await expect(charts(page).first()).toHaveClass(/wrap-square/);
    await strip.getByRole("button", { name: "Align right" }).click();
    const box = (await charts(page).first().boundingBox())!;
    const sheet = (await page.locator(".flow .fp").first().boundingBox())!;
    expect(box.x + box.width).toBeGreaterThan(sheet.x + sheet.width / 2);
    await strip.getByRole("button", { name: "In line" }).click();
    await expect(charts(page).first()).toHaveClass(/wrap-inline/);
    await expect(charts(page).first().locator(".chartex-bar")).toHaveCount(4);
    // Still alternate content in the saved file.
    expect(decode((await savedParts(page))["word/document.xml"]!)).toContain("<mc:AlternateContent");
  });

  test("Alt text works from the strip and the right-click menu", async ({ page }) => {
    await kindOf("treemap", {}, { alt: "Produce by weight" })(page);
    const c = charts(page).first();
    await expect(c).toHaveAttribute("data-alt", "Produce by weight");
    await select(page, c);
    await svgOf(c).click({ button: "right", position: { x: 40, y: 40 } });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Alt text…")).toBeVisible();
    await expect(contextItem(page, "Replace image…")).toHaveCount(0);
    await contextItem(page, "Alt text…").click();
    const d = dialog(page, "Alt text");
    await expect(d.getByLabel("Describe the chart for screen readers")).toHaveValue("Produce by weight");
    await d.getByLabel("Describe the chart for screen readers").fill("Fruit outweighs veg");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(charts(page).first()).toHaveAttribute("data-alt", "Fruit outweighs veg");
    expect(decode((await savedParts(page))["word/document.xml"]!)).toContain('descr="Fruit outweighs veg"');
  });

  test("editing it says where it can be done: Word", async ({ page }) => {
    await kindOf("sunburst")(page);
    const c = charts(page).first();
    // A double-click.
    await svgOf(c).dblclick({ position: { x: 30, y: 30 } });
    await expect(page.locator(".toast")).toHaveText("Edit this chart type in Word");
    await expect(dialog(page, "Edit chart")).toHaveCount(0);
    await page.locator(".toast").evaluate((e) => e.remove());
    // The Edit button of the strip.
    await select(page, c);
    await page.locator(".img-strip").getByRole("button", { name: "Edit chart" }).click();
    await expect(page.locator(".toast")).toHaveText("Edit this chart type in Word");
    await expect(dialog(page, "Edit chart")).toHaveCount(0);
    // The classic chart's data and type are not offered for it: nothing in the model.
    const none = await page.evaluate(() => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { chartAt(p: unknown): unknown } } } } }).docxtor;
      const p = document.querySelector<HTMLElement>(".flow .atom.chart")!.closest<HTMLElement>("[data-p]")!;
      return w.fe.editor.engine.chartAt({ p: Number(p.dataset.p), o: Number(document.querySelector<HTMLElement>(".flow .atom.chart")!.dataset.o) });
    });
    expect(none).toBeNull();
  });
});

test.describe("looks", () => {
  test("text in the drawing uses the paragraph's font and Word's greys; colours are the chart palette", async ({ page }) => {
    await kindOf("waterfall")(page);
    const c = charts(page).first();
    const font = await c.evaluate((el) => ({ chart: getComputedStyle(el.querySelector(".chart-t")!).fontFamily, atom: getComputedStyle(el).fontFamily }));
    expect(font.chart).toBe(font.atom);
    const text = await c.locator(".chart-t").first().evaluate((e) => getComputedStyle(e).fill);
    expect(text).toBe("rgb(89, 89, 89)");
    const grid = await c.locator(".chart-grid").first().evaluate((e) => getComputedStyle(e).stroke);
    expect(grid).toBe("rgb(217, 217, 217)");
    // A small chart still draws without text running out of it.
    await openChartEx(page, chartexXml("sunburst"), { extent: [1828800, 1371600] });
    const small = (await svgOf(charts(page).first()).boundingBox())!;
    expect(Math.round(small.width)).toBe(192);
  });

  test("the print copy has the chart, drawn", async ({ page }) => {
    await page.addInitScript(() => {
      window.print = () => undefined;
    });
    await page.reload();
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await kindOf("histogram")(page);
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .chart-svg")).toHaveCount(1);
    await expect(page.locator(".docxtor-print .chartex-bar")).toHaveCount(4);
    const fill = await page.locator(".docxtor-print .chart-t").first().evaluate((e) => getComputedStyle(e).fill);
    expect(fill).toBe("rgb(89, 89, 89)");
  });

  test("the chart is as readable in the dark theme: the page stays paper", async ({ page }) => {
    await kindOf("pareto")(page);
    await page.emulateMedia({ colorScheme: "dark" });
    const c = charts(page).first();
    const text = await c.locator(".chart-t").first().evaluate((e) => getComputedStyle(e).fill);
    expect(text).toBe("rgb(89, 89, 89)");
    const bg = await c.locator(".chart-bg").evaluate((e) => getComputedStyle(e).fill);
    expect(bg.match(/\d+/g)!.map(Number).every((v) => v >= 240)).toBe(true);
    await shot(c, "pareto-dark");
  });
});
