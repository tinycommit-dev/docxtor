// Chart options (item 54): the Edit chart dialog's tabs set axis titles and scales, gridlines, series colours, data
// labels, legend, trendlines, error bars, looks and palettes, a series' own type and axis (combination charts);
// the new types (bubble, stock, surface) are drawn; and the data of a chart whose workbook came from Excel is
// edited in place. Each test checks the drawing (the dialog's preview and the page) and the saved XML.
import { inflateRawSync } from "node:zlib";
import { chartDocx, wordChartPart, wordColumnChart, wordSeries, xlsxBytes } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, openFixture, test, type Locator, type Page } from "./helpers";

const SHOTS = process.env.S5_SHOTS;

async function shot(page: Page, name: string) {
  if (!SHOTS) return;
  await page.waitForTimeout(300); // the dialog finishes opening
  await page.screenshot({ path: `${SHOTS}/${name}.png` });
}

/** Open a built document in the editor in place of what is on screen. */
async function openDoc(page: Page, bytes: Uint8Array) {
  await page.evaluate(async (b) => {
    const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
    await w.fe.open(new Uint8Array(b), "from-word.docx");
  }, Array.from(bytes));
  await expect(page.locator(".flow .fp").first()).toContainText("Before the chart");
}

const charts = (page: Page) => page.locator(".flow .atom.chart");

/** The bytes of the document as it would be saved. */
const saved = (page: Page): Promise<number[]> => inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));

/** One part of a zip (a document, or a workbook inside it), as text; null when it isn't there. */
function partOf(bytes: ArrayLike<number>, name: string): string | null {
  const buf = Buffer.from(Array.from(bytes));
  let end = buf.length - 22;
  while (end > 0 && buf.readUInt32LE(end) !== 0x06054b50) end--;
  let p = buf.readUInt32LE(end + 16);
  const count = buf.readUInt16LE(end + 10);
  for (let i = 0; i < count; i++) {
    const method = buf.readUInt16LE(p + 10);
    const csize = buf.readUInt32LE(p + 20);
    const nlen = buf.readUInt16LE(p + 28);
    const xlen = buf.readUInt16LE(p + 30);
    const clen = buf.readUInt16LE(p + 32);
    const off = buf.readUInt32LE(p + 42);
    const n = buf.toString("utf8", p + 46, p + 46 + nlen);
    p += 46 + nlen + xlen + clen;
    if (n !== name) continue;
    const start = off + 30 + buf.readUInt16LE(off + 26) + buf.readUInt16LE(off + 28);
    const data = buf.subarray(start, start + csize);
    return (method === 0 ? data : inflateRawSync(data)).toString("utf8");
  }
  return null;
}

/** The bytes of a part of a zip (for a workbook inside the document). */
function bytesOf(bytes: ArrayLike<number>, name: string): Buffer | null {
  const buf = Buffer.from(Array.from(bytes));
  let end = buf.length - 22;
  while (end > 0 && buf.readUInt32LE(end) !== 0x06054b50) end--;
  let p = buf.readUInt32LE(end + 16);
  const count = buf.readUInt16LE(end + 10);
  for (let i = 0; i < count; i++) {
    const method = buf.readUInt16LE(p + 10);
    const csize = buf.readUInt32LE(p + 20);
    const nlen = buf.readUInt16LE(p + 28);
    const xlen = buf.readUInt16LE(p + 30);
    const clen = buf.readUInt16LE(p + 32);
    const off = buf.readUInt32LE(p + 42);
    const n = buf.toString("utf8", p + 46, p + 46 + nlen);
    p += 46 + nlen + xlen + clen;
    if (n !== name) continue;
    const start = off + 30 + buf.readUInt16LE(off + 26) + buf.readUInt16LE(off + 28);
    const data = buf.subarray(start, start + csize);
    return method === 0 ? Buffer.from(data) : inflateRawSync(data);
  }
  return null;
}

const chartXml = async (page: Page) => partOf(await saved(page), "word/charts/chart1.xml") ?? "";

/** Select the chart with a click and open the Edit chart dialog. */
async function editChart(page: Page): Promise<Locator> {
  const c = charts(page).first();
  await c.scrollIntoViewIfNeeded();
  await c.locator(".chart-svg").click({ position: { x: 20, y: 20 } });
  await expect(page.locator(".img-frame")).toBeVisible();
  await page.getByRole("button", { name: "Edit chart" }).click();
  const d = dialog(page, "Edit chart");
  await expect(d).toBeVisible();
  return d;
}

const tab = (d: Locator, name: string) => d.getByRole("tab", { name, exact: true }).click();
const seriesSelect = (d: Locator) => d.locator("select[aria-label='Series']");
const preview = (d: Locator) => d.locator(".chart-preview .chart-svg");
const previewTexts = (d: Locator) => d.locator(".chart-preview .chart-svg text").allTextContents();

/** Insert a chart into the open document with the engine and open its dialog. */
async function addChart(page: Page, op: Record<string, unknown>) {
  await inPage(
    page,
    (w, s) => {
      const e = w.fe.editor;
      const order = e.engine!.renderAll().order;
      e.exec({ op: "insert_paragraph", after: order[order.length - 1], text: "" });
      const after = e.engine!.renderAll().order;
      e.exec({ op: "insert_chart", at: { p: after[after.length - 1]!, o: 0 }, ...s });
    },
    op,
  );
}

const SALES = { kind: "column", title: "Sales", categories: ["Q1", "Q2", "Q3", "Q4"], series: [{ name: "Sales", values: [24, 31, 29, 42] }, { name: "Costs", values: [15, 18, 22, 24] }], legend: "bottom" };

test.beforeEach(async ({ page }) => {
  await openFixture(page, "simple.docx");
});

test.describe("the dialog's tabs", () => {
  test("the Layout tab sets axis titles, gridlines, the legend and the title position", async ({ page }) => {
    await openDoc(page, chartDocx(wordColumnChart(), { workbook: xlsxBytes([["", "Revenue", "Costs"], ["North", 4.3, 2.4], ["South", 2.5, 4.4], ["East", 3.5, 1.8], ["West", 4.5, 2.8]]) }));
    const d = await editChart(page);
    await shot(page, "dialog-chart-light");
    await tab(d, "Layout");
    await d.getByLabel("Category axis", { exact: true }).fill("Region");
    await d.getByLabel("Value axis", { exact: true }).fill("Money");
    await expect.poll(() => previewTexts(d)).toEqual(expect.arrayContaining(["Region", "Money"]));
    const horizontalGrid = preview(d).locator(".chart-gridlines .chart-grid:not(.minor)");
    const before = await horizontalGrid.count();
    await d.getByLabel("Category axis, major").check();
    await expect(preview(d).locator(".chart-gridlines .chart-grid:not(.minor)")).toHaveCount(before + 5); // a line between each of the four regions, and both ends
    await d.getByLabel("Value axis, minor").check();
    expect(await preview(d).locator(".chart-grid.minor").count()).toBeGreaterThan(5);
    await d.getByLabel("Title position").selectOption("overlay");
    await d.getByLabel("Legend position").selectOption("tr");
    await d.getByLabel("Show over the chart").check();
    await shot(page, "dialog-layout-light");
    await d.getByRole("button", { name: "Apply" }).click();

    const c = charts(page).first();
    expect(await c.locator(".chart-axis-title").allTextContents()).toEqual(["Region", "Money"]);
    await expect(c.locator(".chart-grid.minor").first()).toBeAttached();
    const xml = await chartXml(page);
    expect(xml).toContain("<a:t>Region</a:t>");
    expect(xml).toContain("<a:t>Money</a:t>");
    expect(xml).toContain('<c:legendPos val="tr"/>');
    expect(xml.match(/<c:overlay val="1"\/>/g)?.length).toBe(2); // the title and the legend
    expect(xml).toContain("<c:minorGridlines>");
    expect(xml.indexOf("<c:majorGridlines")).toBeLessThan(xml.indexOf("<c:minorGridlines"));
    // Word's own parts around it are untouched.
    expect(xml).toContain('<c14:style val="102"/>');
  });

  test("the Axes tab: minimum, maximum and units decide the tick labels; the number format, display units and direction", async ({ page }) => {
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Axes");
    await d.getByRole("radio", { name: "Value", exact: true }).click();
    await d.getByLabel("Minimum").fill("0");
    await d.getByLabel("Maximum").fill("100");
    await d.getByLabel("Major unit").fill("25");
    await expect.poll(() => previewTexts(d)).toEqual(expect.arrayContaining(["0", "25", "50", "75", "100"]));
    // Currency, with the engine's own format code.
    await d.getByLabel("Linked to source").uncheck();
    await d.getByLabel("Category", { exact: true }).selectOption("currency");
    await expect.poll(() => previewTexts(d)).toEqual(expect.arrayContaining(["$25.00", "$100.00"]));
    await d.getByLabel("Display units").selectOption("hundreds");
    await expect.poll(() => previewTexts(d)).toEqual(expect.arrayContaining(["$0.25", "$1.00"]));
    await d.getByLabel("Display units").selectOption("");
    await d.getByLabel("Major tick marks").selectOption("out");
    expect(await preview(d).locator(".chart-tick").count()).toBeGreaterThan(3);
    await d.getByLabel("Logarithmic scale").check();
    await d.getByLabel("Logarithmic scale").uncheck();
    await shot(page, "dialog-axes-light");
    await d.getByRole("button", { name: "Apply" }).click();
    const xml = await chartXml(page);
    expect(xml).toContain("<c:max val=\"100\"/><c:min val=\"0\"/>");
    expect(xml).toContain('<c:majorUnit val="25"/>');
    expect(xml).toContain('formatCode="$#,##0.00" sourceLinked="0"');
    expect(xml).toContain('<c:majorTickMark val="out"/>');
    const texts = await charts(page).first().locator(".chart-svg text").allTextContents();
    expect(texts).toEqual(expect.arrayContaining(["$0.00", "$50.00", "$100.00"]));
  });
});

test.describe("series", () => {
  test("colours: a theme colour stays a theme colour, a hex is exact, an outline-only fill shows", async ({ page }) => {
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Series");
    // The first series takes Accent 2 (a theme colour); the second a hex.
    await d.getByRole("button", { name: "Accent 2", exact: true }).click();
    const bars = preview(d).locator(".chart-bar");
    const accent2 = await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      const p = e.renderAll().order.at(-1)!;
      return e.chartAt({ p, o: 0 })!.theme!.accent2!;
    });
    await expect(bars.first()).toHaveAttribute("fill", `#${accent2}`);
    await seriesSelect(d).selectOption("1");
    await d.getByLabel("Hex color").fill("336699");
    await expect(bars.nth(1)).toHaveAttribute("fill", "#336699");
    await d.getByLabel("Fill", { exact: true }).selectOption("none");
    await expect(bars.nth(1)).toHaveAttribute("fill", "none");
    await expect(bars.nth(1)).toHaveAttribute("stroke", "#336699");
    await shot(page, "dialog-series-light");
    await d.getByRole("button", { name: "Apply" }).click();
    const c = charts(page).first();
    await expect(c.locator(".chart-bar").first()).toHaveAttribute("fill", `#${accent2}`);
    await expect(c.locator(".chart-bar").nth(1)).toHaveAttribute("fill", "none");
    const xml = await chartXml(page);
    expect(xml).toContain('<a:schemeClr val="accent2"/>');
    expect(xml).toContain('<a:srgbClr val="336699"/>');
    expect(xml).toMatch(/<c:spPr><a:noFill\/><a:ln w="28575"><a:solidFill><a:srgbClr val="336699"\/>/);
  });

  test("a pie chart's slices take their own colours", async ({ page }) => {
    await addChart(page, { kind: "pie", title: "Share", categories: ["Rent", "Food", "Other"], series: [{ name: "Share", values: [50, 30, 20] }] });
    const d = await editChart(page);
    await tab(d, "Series");
    await d.getByLabel("Point", { exact: true }).selectOption("1");
    await d.getByLabel("Hex color").fill("AA0000");
    await expect(preview(d).locator(".chart-slice").nth(1)).toHaveAttribute("fill", "#AA0000");
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(charts(page).first().locator(".chart-slice").nth(1)).toHaveAttribute("fill", "#AA0000");
    const xml = await chartXml(page);
    expect(xml).toContain('<c:dPt><c:idx val="1"/>');
    expect(xml).toContain('<a:srgbClr val="AA0000"/>');
  });

  test("data labels: what they show, where, and with which separator", async ({ page }) => {
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Series");
    await d.getByLabel("Show data labels").check();
    await d.getByLabel("Category name").check();
    await d.getByLabel("Label position").selectOption("inEnd");
    await d.getByLabel("Separator").selectOption("; ");
    await expect.poll(() => previewTexts(d)).toEqual(expect.arrayContaining(["Q1; 24", "Q4; 42"]));
    expect(await previewTexts(d)).not.toContain("Q1; 15"); // only the first series has labels
    await d.getByRole("button", { name: "Apply" }).click();
    const texts = await charts(page).first().locator(".chart-label").allTextContents();
    expect(texts).toEqual(["Q1; 24", "Q2; 31", "Q3; 29", "Q4; 42"]);
    const xml = await chartXml(page);
    expect(xml).toContain('<c:dLblPos val="inEnd"/>');
    expect(xml).toContain("<c:separator>; </c:separator>");
    expect(xml).toContain('<c:showCatName val="1"/>');
  });

  test("a trendline is drawn with its equation, and listed in the legend", async ({ page }) => {
    await addChart(page, { ...SALES, kind: "line" });
    const d = await editChart(page);
    await tab(d, "Series");
    await d.getByRole("button", { name: "Add trendline" }).click();
    await d.getByLabel("Show equation on chart").check();
    await d.getByLabel("Show R-squared value").check();
    await expect(preview(d).locator(".chart-trend")).toHaveCount(1);
    const texts = await previewTexts(d);
    expect(texts.some((t) => /^y = [\d.]+x [+−] [\d.]+$/.test(t))).toBe(true);
    expect(texts.some((t) => /^R² = /.test(t))).toBe(true);
    expect(texts).toContain("Linear (Sales)");
    await d.getByLabel("Type", { exact: true }).selectOption("poly");
    await d.getByLabel("Order").fill("3");
    await d.getByLabel("Forecast forward").fill("1");
    await expect(preview(d).locator(".chart-trend")).toHaveCount(1);
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(charts(page).first().locator(".chart-trend")).toHaveCount(1);
    const xml = await chartXml(page);
    expect(xml).toContain('<c:trendlineType val="poly"/><c:order val="3"/>');
    expect(xml).toContain('<c:forward val="1"/>');
    expect(xml).toContain('<c:dispRSqr val="1"/><c:dispEq val="1"/>');
  });

  test("error bars: fixed, percentage and custom amounts draw their lines", async ({ page }) => {
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Series");
    await d.getByRole("button", { name: "Add error bars", exact: true }).click();
    // One bar per column of the first series, with a cap at each end.
    await expect(preview(d).locator(".chart-err")).toHaveCount(4 * 3);
    await d.getByLabel("No end cap").check();
    await expect(preview(d).locator(".chart-err")).toHaveCount(4);
    await d.getByLabel("Error amount").selectOption("custom");
    await d.getByLabel("Plus amounts, separated by commas").fill("5, 1, 2, 3");
    await d.getByLabel("Minus amounts, separated by commas").fill("0, 0, 0, 0");
    await d.getByLabel("Direction of the bars").selectOption("plus");
    await expect(preview(d).locator(".chart-err")).toHaveCount(4);
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(charts(page).first().locator(".chart-err")).toHaveCount(4);
    const xml = await chartXml(page);
    expect(xml).toContain('<c:errValType val="cust"/><c:noEndCap val="1"/>');
    expect(xml).toContain("<c:plus><c:numLit>");
    expect(xml).toContain('<c:errBarType val="plus"/>');
  });
});

test.describe("Style and Change colors", () => {
  test("a style and a palette change the series and are written as Word writes them", async ({ page }) => {
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Style");
    await d.getByRole("button", { name: "Style 3" }).click();
    await expect(preview(d).locator(".chart-bar").first()).toHaveAttribute("fill", "none");
    await d.getByRole("button", { name: "Monochromatic palette 2" }).click();
    const fills = await preview(d).locator(".chart-bar").evaluateAll((els) => els.map((e) => e.getAttribute("stroke")));
    expect(new Set(fills).size).toBe(2);
    await shot(page, "dialog-style-light");
    await d.getByRole("button", { name: "Apply" }).click();
    const bytes = await saved(page);
    const xml = partOf(bytes, "word/charts/chart1.xml")!;
    expect(xml).toContain('<c:style val="18"/>');
    expect(xml).toContain('<c14:style val="118"/>');
    expect(xml).toContain('<a:schemeClr val="accent2">');
    const colors = partOf(bytes, "word/charts/colors1.xml")!;
    expect(colors).toContain('meth="withinLinear"');
    expect(colors).toContain('id="15"');
    expect(partOf(bytes, "word/charts/_rels/chart1.xml.rels")).toContain("chartColorStyle");
    expect(partOf(bytes, "[Content_Types].xml")).toContain("chartcolorstyle");
    // The dialog knows the palette when it is opened again.
    const again = await editChart(page);
    await tab(again, "Style");
    await expect(again.getByRole("button", { name: "Monochromatic palette 2" })).toHaveAttribute("aria-pressed", "true");
  });
});

test.describe("combination charts", () => {
  test("a series gets its own type and the second axis, and the chart comes back as a combination", async ({ page }) => {
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Series");
    await seriesSelect(d).selectOption("1");
    await d.getByLabel("Chart type", { exact: true }).selectOption("line");
    await d.getByLabel("Secondary axis").check();
    await expect(preview(d).locator(".chart-bar")).toHaveCount(4);
    await expect(preview(d).locator(".chart-line .chart-stroke")).toHaveCount(1);
    await shot(page, "dialog-combo-light");
    await d.getByRole("button", { name: "Apply" }).click();
    const c = charts(page).first();
    await expect(c.locator(".chart-bar")).toHaveCount(4);
    await expect(c.locator(".chart-line .chart-stroke")).toHaveCount(1);
    // The second value axis labels are on the right of the plot.
    const plotRight = await c.locator(".chart-axis").first().evaluate((e) => Number(e.getAttribute("x2")));
    const right = await c.locator(".chart-axes text[text-anchor=start]").evaluateAll((els) => els.map((e) => Number(e.getAttribute("x"))));
    expect(right.length).toBeGreaterThan(2);
    expect(Math.min(...right)).toBeGreaterThan(plotRight);
    const xml = await chartXml(page);
    expect(xml).toContain("<c:barChart>");
    expect(xml).toContain("<c:lineChart>");
    expect(xml.match(/<c:valAx>/g)?.length).toBe(2);
    // The dialog shows what the chart is.
    const again = await editChart(page);
    await expect(again.getByRole("radio", { name: "Column" })).toHaveAttribute("aria-checked", "false");
    await tab(again, "Series");
    await seriesSelect(again).selectOption("1");
    await expect(again.getByLabel("Chart type", { exact: true })).toHaveValue("line");
    await expect(again.getByLabel("Secondary axis")).toBeChecked();
  });

  test("a combination chart written by Word is drawn, edited and written back as one", async ({ page }) => {
    const cats = ["North", "South", "East", "West"];
    const plot =
      `<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>${wordSeries(0, "Revenue", [4.3, 2.5, 3.5, 4.5], cats)}<c:gapWidth val="219"/><c:overlap val="-27"/><c:axId val="111"/><c:axId val="222"/></c:barChart>` +
      `<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>${wordSeries(1, "Margin", [0.2, 0.4, 0.3, 0.5], cats, { spPr: '<c:spPr><a:ln w="28575" cap="rnd"><a:solidFill><a:schemeClr val="accent2"/></a:solidFill><a:round/></a:ln></c:spPr>', extra: '<c:marker><c:symbol val="circle"/><c:size val="5"/></c:marker>' }).replace("</c:ser>", '<c:smooth val="0"/></c:ser>')}<c:marker val="1"/><c:axId val="333"/><c:axId val="444"/></c:lineChart>` +
      `<c:catAx><c:axId val="111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="222"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx>` +
      `<c:valAx><c:axId val="222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="111"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>` +
      `<c:valAx><c:axId val="444"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="r"/><c:numFmt formatCode="0%" sourceLinked="0"/><c:majorTickMark val="out"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="333"/><c:crosses val="max"/><c:crossBetween val="between"/></c:valAx>` +
      `<c:catAx><c:axId val="333"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="1"/><c:axPos val="b"/><c:majorTickMark val="out"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="444"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx>`;
    await openDoc(page, chartDocx(wordChartPart(plot, { title: "Revenue and margin", legend: '<c:legend><c:legendPos val="b"/><c:overlay val="0"/></c:legend>' })));
    const c = charts(page).first();
    await expect(c.locator(".chart-bar")).toHaveCount(4);
    await expect(c.locator(".chart-line .chart-stroke")).toHaveCount(1);
    expect(await c.locator(".chart-svg text").allTextContents()).toEqual(expect.arrayContaining(["20%", "50%", "Revenue", "Margin"]));
    const d = await editChart(page);
    await d.getByLabel("Title").fill("Revenue and margin, 2025");
    await tab(d, "Series");
    await seriesSelect(d).selectOption("1");
    await expect(d.getByLabel("Secondary axis")).toBeChecked();
    await d.getByRole("button", { name: "Apply" }).click();
    const xml = await chartXml(page);
    expect(xml).toContain("Revenue and margin, 2025");
    expect(xml).toContain("<c:barChart>");
    expect(xml).toContain("<c:lineChart>");
    expect(xml).toContain('<c:numFmt formatCode="0%" sourceLinked="0"/>');
    expect(xml.match(/<c:valAx>/g)?.length).toBe(2);
    expect(xml).toContain('<c:axId val="333"/><c:axId val="444"/>');
    await expect(charts(page).first().locator(".chart-line .chart-stroke")).toHaveCount(1);
  });
});

// The new types, inserted from the menu and drawn.
const NEW_TYPES: { name: string; path: string[]; count: Record<string, number | "some"> }[] = [
  { name: "bubble", path: ["Bubble"], count: { ".chart-bubble": 8 } },
  { name: "stock hlc", path: ["Stock", "High-low-close"], count: { ".chart-hilow": 6, ".chart-close": 6, ".chart-up": 0, ".chart-down": 0 } },
  { name: "stock ohlc", path: ["Stock", "Open-high-low-close"], count: { ".chart-hilow": 6, ".chart-close": 0 } },
  { name: "stock vhlc", path: ["Stock", "Volume-high-low-close"], count: { ".chart-hilow": 6, ".chart-bar": 6, ".chart-close": 6 } },
  { name: "stock vohlc", path: ["Stock", "Volume-open-high-low-close"], count: { ".chart-hilow": 6, ".chart-bar": 6 } },
  { name: "surface", path: ["Surface", "3-D surface"], count: { ".chart-facet": 12 } },
  { name: "wireframe", path: ["Surface", "Wireframe 3-D surface"], count: { ".chart-wire": 12, ".chart-facet": 0 } },
  { name: "contour", path: ["Surface", "Contour"], count: { ".chart-contour .chart-patch": "some" } },
  { name: "wireframe contour", path: ["Surface", "Wireframe contour"], count: { ".chart-contour-line": "some", ".chart-patch": 0 } },
  { name: "combination", path: ["Combination", "Clustered column and line on second axis"], count: { ".chart-bar": 4, ".chart-line .chart-stroke": 1 } },
];

test.describe("Insert, Chart, More types", () => {
  for (const t of NEW_TYPES) {
    test(`${t.name}: inserted, drawn and read back`, async ({ page }) => {
      await page.locator(".flow .fp").first().click();
      await chooseMenu(page, "Insert", "Chart", "More types", ...t.path);
      const c = charts(page).first();
      await expect(c.locator(".chart-svg")).toBeVisible();
      for (const [sel, n] of Object.entries(t.count)) {
        if (n === "some") expect(await c.locator(sel).count(), sel).toBeGreaterThan(10);
        else await expect(c.locator(sel), sel).toHaveCount(n);
      }
      await expect(c.locator(".chart-svg")).toHaveAttribute("data-kind", /bubble|stock|surface|column/);
      if (SHOTS) {
        await c.scrollIntoViewIfNeeded();
        await c.locator(".chart-svg").screenshot({ path: `${SHOTS}/new-${t.name.replace(/ /g, "-")}.png` });
      }
      // The saved file reads back as the same chart.
      await inPage(page, async (w) => {
        await w.fe.open(w.fe.editor.engine!.save(), "again.docx");
      });
      const again = charts(page).first();
      await expect(again.locator(".chart-svg")).toBeVisible();
      const kind = await inPage(page, (w) => {
        const atom = document.querySelector<HTMLElement>(".flow .atom.chart")!;
        const p = Number(atom.closest<HTMLElement>("[data-p]")!.dataset.p);
        const m = w.fe.editor.engine!.chartAt({ p, o: Number(atom.dataset.o) })!;
        return `${m.kind}|${m.stock ?? ""}|${m.surface ?? ""}|${m.combo ? "combo" : ""}`;
      });
      expect(kind).toMatch(/^(bubble|stock|surface|column)\|/);
      for (const [sel, n] of Object.entries(t.count)) {
        if (n !== "some") await expect(again.locator(sel), `again ${sel}`).toHaveCount(n);
      }
    });
  }

  test("the dialog edits a bubble chart: sizes in the grid, scale and what size means", async ({ page }) => {
    await addChart(page, { kind: "bubble", title: "Bubbles", series: [{ name: "North", x_values: [1, 2, 3], values: [3, 5, 4], sizes: [10, 20, 40] }] });
    const d = await editChart(page);
    await expect(d.getByLabel("North, point 3, size")).toHaveValue("40");
    const circles = preview(d).locator(".chart-bubble");
    await expect(circles).toHaveCount(3);
    // A bubble is as big as its size is of the biggest one.
    const r = async () => Number(await circles.nth(0).getAttribute("r"));
    const before = await r();
    await d.getByLabel("North, point 1, size").fill("30");
    await expect.poll(r).toBeGreaterThan(before);
    await tab(d, "Series");
    const grown = await r();
    await d.getByLabel("Scale, percent").fill("50");
    await expect.poll(r).toBeLessThan(grown);
    await d.getByLabel("Size represents").selectOption("width");
    await d.getByRole("button", { name: "Apply" }).click();
    const xml = await chartXml(page);
    expect(xml).toContain('<c:bubbleScale val="50"/>');
    expect(xml).toContain('<c:sizeRepresents val="w"/>');
    expect(xml).toContain("<c:v>30</c:v>");
  });

  test("the dialog changes a stock chart's type and its series follow", async ({ page }) => {
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Insert", "Chart", "More types", "Stock", "High-low-close");
    const d = await editChart(page);
    await d.getByLabel("Stock chart type").selectOption("ohlc");
    await expect(d.getByLabel("Series 1 name")).toHaveValue("Open");
    await expect(d.getByLabel("Series 4 name")).toHaveValue("Close");
    await expect(d.getByRole("button", { name: "Add series" })).toBeDisabled();
    expect(await preview(d).locator(".chart-up, .chart-down").count()).toBe(6);
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await chartXml(page)).toContain("<c:upDownBars>");
    await expect(charts(page).first().locator(".chart-up, .chart-down")).toHaveCount(6);
  });

  test("the dialog changes a surface chart to a contour map", async ({ page }) => {
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Insert", "Chart", "More types", "Surface", "3-D surface");
    const d = await editChart(page);
    await d.getByLabel("Surface chart type").selectOption("contour");
    expect(await preview(d).locator(".chart-patch").count()).toBeGreaterThan(10);
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await chartXml(page)).toContain("<c:surfaceChart>");
  });
});

test.describe("the workbook behind a chart", () => {
  test("editing the data changes the cells the chart uses and keeps the rest of the workbook", async ({ page }) => {
    const book = xlsxBytes([
      ["", "Revenue", "Costs", "Note", "Doubled"],
      ["North", 4.3, 2.4, "keep me", "=B2*2|8.6"],
      ["South", 2.5, 4.4, "and me", "=B3*2|5"],
      ["East", 3.5, 1.8, null, "=B4*2|7"],
      ["West", 4.5, 2.8, "last", "=B5*2|9"],
    ]);
    await openDoc(page, chartDocx(wordColumnChart(), { workbook: book }));
    const d = await editChart(page);
    await d.getByLabel("Revenue, North").fill("9.5");
    await d.getByRole("button", { name: "Add row" }).click();
    await d.getByLabel("Category 5", { exact: true }).fill("Centre");
    await d.getByLabel("Revenue, Centre").fill("6");
    await d.getByLabel("Costs, Centre").fill("3");
    await d.getByRole("button", { name: "Apply" }).click();
    const bytes = await saved(page);
    const wb = bytesOf(bytes, "word/embeddings/Microsoft_Excel_Worksheet.xlsx")!;
    const s1 = partOf(wb, "xl/worksheets/sheet1.xml")!;
    // The columns that are not the chart's, the formulas and the styles are still there.
    expect(s1).toContain("<f>B2*2</f>");
    expect(s1).toContain("<f>B5*2</f>");
    expect(partOf(wb, "xl/sharedStrings.xml")).toContain("Doubled");
    expect(partOf(wb, "xl/sharedStrings.xml")).toContain("keep me");
    expect(partOf(wb, "xl/worksheets/sheet2.xml")).toContain("Do not touch");
    expect(partOf(wb, "xl/workbook.xml")).toContain("definedName");
    expect(partOf(wb, "xl/styles.xml")).toContain("formatCode=\"0.0\"");
    // The changed values and the new row are in; the chart's ranges grew.
    expect(s1).toMatch(/<c r="B2"[^>]*><v>9.5<\/v>/);
    expect(s1).toContain("Centre");
    const xml = await chartXml(page);
    expect(xml).toContain("<c:f>Sheet1!$B$2:$B$6</c:f>");
    expect(xml).toContain("<c:f>Sheet1!$A$2:$A$6</c:f>");
    await expect(charts(page).first().locator(".chart-bar")).toHaveCount(10);
  });
});

test.describe("both themes", () => {
  for (const scheme of ["light", "dark"] as const) {
    test(`the dialog's text is 4.5:1 or better in the ${scheme} theme, on every tab`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await addChart(page, { ...SALES, kind: "line" });
      const d = await editChart(page);
      for (const name of ["Chart", "Layout", "Axes", "Series", "Style"]) {
        await tab(d, name);
        if (name === "Series") {
          await d.getByRole("button", { name: "Add trendline" }).click();
          await d.getByRole("button", { name: "Add error bars", exact: true }).click();
        }
        await shot(page, `dialog-${name.toLowerCase()}-${scheme}`);
        const worst = await page.evaluate(() => {
          const parse = (c: string) => {
            const m = /rgba?\(([^)]+)\)/.exec(c)!;
            const p = m[1]!.split(/[,\s/]+/).filter(Boolean).map(Number);
            return { r: p[0]!, g: p[1]!, b: p[2]!, a: p[3] ?? 1 };
          };
          const lin = (v: number) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
          const lum = (c: { r: number; g: number; b: number }) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
          /** The colour behind an element: translucent layers are put over the ones below them. */
          const backdrop = (el: Element): { r: number; g: number; b: number } => {
            const layers: ReturnType<typeof parse>[] = [];
            for (let e: Element | null = el; e; e = e.parentElement) {
              const c = parse(getComputedStyle(e).backgroundColor);
              if (c.a > 0) layers.push(c);
              if (c.a >= 1) break;
            }
            let out = { r: 255, g: 255, b: 255 };
            for (const l of layers.reverse()) out = { r: out.r * (1 - l.a) + l.r * l.a, g: out.g * (1 - l.a) + l.g * l.a, b: out.b * (1 - l.a) + l.b * l.a };
            return out;
          };
          let worst = { ratio: 99, text: "" };
          const scope = document.querySelector(".chart-dialog")!;
          for (const el of scope.querySelectorAll<HTMLElement>("*")) {
            if (el.closest("svg") || el.closest(".chart-preview") || !el.offsetParent) continue;
            const own = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent!.trim());
            if (!own || (el as HTMLButtonElement).disabled) continue;
            const fg = parse(getComputedStyle(el).color);
            const bg = backdrop(el);
            const a = lum(fg) + 0.05;
            const b = lum(bg) + 0.05;
            const ratio = Math.max(a, b) / Math.min(a, b);
            if (ratio < worst.ratio) worst = { ratio, text: el.textContent!.trim().slice(0, 30) };
          }
          return worst;
        });
        expect(worst.ratio, `${scheme}, ${name} tab: "${worst.text}"`).toBeGreaterThanOrEqual(4.5);
      }
    });
  }
});

test.describe("one step, and extensions", () => {
  test("everything the dialog sets is one undo step", async ({ page }) => {
    await addChart(page, SALES);
    const before = await chartXml(page);
    const d = await editChart(page);
    await tab(d, "Layout");
    await d.getByLabel("Value axis", { exact: true }).fill("Money");
    await tab(d, "Series");
    await d.getByRole("button", { name: "Add trendline" }).click();
    await tab(d, "Style");
    await d.getByRole("button", { name: "Style 2" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    const after = await chartXml(page);
    expect(after).not.toBe(before);
    expect(after).toContain("<c:trendline>");
    await page.keyboard.press("ControlOrMeta+Z");
    expect(await chartXml(page)).toBe(before);
    await page.keyboard.press("ControlOrMeta+Shift+Z");
    expect(await chartXml(page)).toBe(after);
  });

  test("a palette an extension adds is in the Change colors gallery and colours the series", async ({ page }) => {
    await inPage(page, async (w) => {
      await w.fe.extensions.add({ id: "brand", name: "Brand", activate: (ctx) => void ctx.chartPalettes.register({ id: "brand", label: "Brand colors", colors: ["FF4F00", "201515", "scheme:accent4"] }) });
    });
    await addChart(page, SALES);
    const d = await editChart(page);
    await tab(d, "Style");
    await d.getByRole("button", { name: "Brand colors" }).click();
    const bars = preview(d).locator(".chart-bar");
    await expect(bars.nth(0)).toHaveAttribute("fill", "#FF4F00");
    await expect(bars.nth(1)).toHaveAttribute("fill", "#201515");
    await d.getByRole("button", { name: "Apply" }).click();
    const xml = await chartXml(page);
    expect(xml).toContain('<a:srgbClr val="FF4F00"/>');
    expect(xml).toContain('<a:srgbClr val="201515"/>');
    await expect(charts(page).first().locator(".chart-bar").first()).toHaveAttribute("fill", "#FF4F00");
  });
});
