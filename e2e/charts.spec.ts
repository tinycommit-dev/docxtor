// Charts: a document with a chart draws it as an SVG (bars, points, slices, labels), Insert, Chart puts a
// sample chart in the text, the Edit chart dialog changes type, title and data, the handles resize it, and
// undo, the right-click menu and printing all treat it as one object.
import { zip } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, dialog, expect, inPage, openFixture, para, pasteData, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

interface Series {
  name: string;
  values: (number | null)[];
  x_values?: number[];
}
interface Spec {
  kind: string;
  title?: string;
  categories?: string[];
  series: Series[];
  legend?: string;
  data_labels?: boolean;
  grouping?: string;
}
interface Model {
  kind: string;
  title?: string;
  categories: string[];
  series: { name: string; values: (number | null)[]; xValues?: (number | null)[] }[];
  legend?: { position: string };
  dataLabels: boolean;
  grouping: string;
}

const SALES: Spec = {
  kind: "column",
  title: "Sales by quarter",
  categories: ["Q1", "Q2", "Q3", "Q4"],
  series: [
    { name: "Sales", values: [24, 31, 29, 42] },
    { name: "Costs", values: [15, 18, 22, 24] },
  ],
  legend: "bottom",
};

/** Add a paragraph at the end of the document and put a chart in it; returns the chart's position. */
async function addChart(page: Page, spec: Spec): Promise<{ p: number; o: number }> {
  return inPage(
    page,
    (w, s) => {
      const e = w.fe.editor;
      const order = e.engine!.renderAll().order;
      e.exec({ op: "insert_paragraph", after: order[order.length - 1], text: "" });
      const after = e.engine!.renderAll().order;
      const at = { p: after[after.length - 1]!, o: 0 };
      e.exec({ op: "insert_chart", at, ...s });
      return at;
    },
    spec,
  );
}

const chartAt = (page: Page, at: { p: number; o: number }): Promise<Model | null> => page.evaluate((a) => (window as unknown as { docxtor: { fe: { editor: { engine: { chartAt(p: unknown): Model | null } } } } }).docxtor.fe.editor.engine.chartAt(a), at);

const charts = (page: Page) => page.locator(".flow .atom.chart");
const svgOf = (c: Locator) => c.locator(".chart-svg");

/** The chart's text, as the SVG draws it (labels, title, legend, ticks). */
const texts = (c: Locator) => c.locator(".chart-svg text").allTextContents();

/** Select a chart with a click and wait for its frame. */
async function select(page: Page, c: Locator) {
  await c.scrollIntoViewIfNeeded();
  await svgOf(c).click({ position: { x: 20, y: 20 } });
  await expect(page.locator(".img-frame")).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await openFixture(page, "simple.docx");
});

/** A chart part as Word writes one (a clustered column chart; `group` renames the plot group to try other kinds). */
function wordChart(group = "barChart"): string {
  const cats = ["Category 1", "Category 2", "Category 3", "Category 4"];
  const col = (i: number) => "BCD"[i];
  const ser = (i: number, name: string, vals: number[]) =>
    `<c:ser><c:idx val="${i}"/><c:order val="${i}"/><c:tx><c:strRef><c:f>Sheet1!$${col(i)}$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>${name}</c:v></c:pt></c:strCache></c:strRef></c:tx><c:spPr><a:solidFill><a:schemeClr val="accent${i + 1}"/></a:solidFill></c:spPr><c:invertIfNegative val="0"/><c:cat><c:strRef><c:f>Sheet1!$A$2:$A$5</c:f><c:strCache><c:ptCount val="4"/>${cats.map((c, k) => `<c:pt idx="${k}"><c:v>${c}</c:v></c:pt>`).join("")}</c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$${col(i)}$2:$${col(i)}$5</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="4"/>${vals.map((v, k) => `<c:pt idx="${k}"><c:v>${v}</c:v></c:pt>`).join("")}</c:numCache></c:numRef></c:val></c:ser>`;
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><c:date1904 val="0"/><c:roundedCorners val="0"/><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Quarterly sales</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/><c:plotArea><c:layout/><c:${group}><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>${ser(0, "Series 1", [4.3, 2.5, 3.5, 4.5])}${ser(1, "Series 2", [2.4, 4.4, 1.8, 2.8])}${ser(2, "Series 3", [2, 2, 3, 5])}<c:gapWidth val="219"/><c:overlap val="-27"/><c:axId val="111"/><c:axId val="222"/></c:${group}><c:catAx><c:axId val="111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="222"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx><c:valAx><c:axId val="222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="111"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx></c:plotArea><c:legend><c:legendPos val="b"/><c:overlay val="0"/></c:legend><c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart></c:chartSpace>`;
}

/** A document the way Word saves one with a chart, opened in the editor in place of what is on screen. */
async function openChartFile(page: Page, chartXml: string) {
  const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
  const R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
  const drawing = `<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="4572000" cy="2743200"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="1" name="Chart 1"/><wp:cNvGraphicFramePr/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="${R}" r:id="rId5"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>`;
  const bytes = zip([
    ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/></Types>`],
    ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R}/officeDocument" Target="word/document.xml"/></Relationships>`],
    ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId5" Type="${R}/chart" Target="charts/chart1.xml"/></Relationships>`],
    ["word/document.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${W}" xmlns:r="${R}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><w:body><w:p><w:r><w:t>Before the chart</w:t></w:r></w:p><w:p>${drawing}</w:p><w:p><w:r><w:t>After the chart</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>`],
    ["word/charts/chart1.xml", chartXml],
  ]);
  await page.evaluate(async (b) => {
    const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
    await w.fe.open(new Uint8Array(b), "from-word.docx");
  }, Array.from(bytes));
  await expect(page.locator(".flow .fp").first()).toContainText("Before the chart");
}

test.describe("a document with charts", () => {
  test("a chart written by Word is drawn: its columns, labels, legend and the theme's colours", async ({ page }) => {
    await openChartFile(page, wordChart());
    const c = charts(page).first();
    await expect(c.locator(".chart-bar")).toHaveCount(12);
    expect(await texts(c)).toEqual(expect.arrayContaining(["Quarterly sales", "Category 1", "Category 4", "Series 1", "Series 2", "Series 3", "5"]));
    const box = (await svgOf(c).boundingBox())!;
    expect(Math.round(box.width)).toBe(480); // 360 pt
    expect(Math.round(box.height)).toBe(288); // 216 pt
  });

  test("draws columns, bars, lines, slices, areas, points and spokes from the data", async ({ page }) => {
    await addChart(page, SALES);
    await addChart(page, { kind: "bar", categories: ["North", "South", "East"], series: [{ name: "2024", values: [5, 9, 3] }], data_labels: true });
    await addChart(page, { kind: "line", categories: ["Jan", "Feb", "Mar", "Apr", "May"], series: [{ name: "A", values: [1, 3, 2, 5, 4] }, { name: "B", values: [2, 2, 3, 3, 4] }] });
    await addChart(page, { kind: "pie", categories: ["Rent", "Food", "Other"], series: [{ name: "Share", values: [50, 30, 20] }], data_labels: true });
    await addChart(page, { kind: "doughnut", categories: ["A", "B"], series: [{ name: "Share", values: [3, 1] }] });
    await addChart(page, { kind: "area", categories: ["Q1", "Q2", "Q3"], series: [{ name: "A", values: [1, 3, 2] }, { name: "B", values: [2, 1, 3] }] });
    await addChart(page, { kind: "scatter", series: [{ name: "S", x_values: [1, 2, 3, 4], values: [2, 4, 3, 6] }] });
    await addChart(page, { kind: "radar", categories: ["a", "b", "c", "d", "e"], series: [{ name: "R", values: [3, 4, 2, 5, 1] }] });
    await expect(charts(page)).toHaveCount(8);
    const [col, bar, line, pie, donut, area, scatter, radar] = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => charts(page).nth(i));

    await expect(svgOf(col!)).toHaveAttribute("data-kind", "column");
    await expect(col!.locator(".chart-bar")).toHaveCount(8);
    expect(await texts(col!)).toEqual(expect.arrayContaining(["Sales by quarter", "Q1", "Q2", "Q3", "Q4", "Sales", "Costs"]));
    // The first column rises to 24 of an axis that runs to 50.
    const tall = await col!.locator(".chart-bar").evaluateAll((els) => els.map((e) => Number(e.getAttribute("height"))));
    expect(tall[2]!).toBeGreaterThan(tall[0]!); // Q2 sales (31) are taller than Q1 sales (24)
    expect(tall[1]!).toBeLessThan(tall[0]!); // Q1 costs (15) are shorter

    await expect(bar!.locator(".chart-bar")).toHaveCount(3);
    expect(await texts(bar!)).toEqual(expect.arrayContaining(["North", "South", "East", "5", "9", "3"]));
    await expect(line!.locator(".chart-marker")).toHaveCount(10);
    await expect(line!.locator("path.chart-stroke")).toHaveCount(2);
    await expect(pie!.locator(".chart-slice")).toHaveCount(3);
    expect(await texts(pie!)).toEqual(expect.arrayContaining(["50%", "30%", "20%"]));
    await expect(donut!.locator(".chart-slice")).toHaveCount(2);
    await expect(area!.locator("path.chart-area")).toHaveCount(2);
    await expect(scatter!.locator(".chart-marker")).toHaveCount(4);
    await expect(radar!.locator("path.chart-stroke")).toHaveCount(1);
    await expect(radar!.locator("line.chart-grid")).toHaveCount(5); // one spoke per category

    // The chart is an atom in the text: one character, inline, as wide as the drawing says.
    const box = await svgOf(col!).boundingBox();
    expect(Math.round(box!.width)).toBe(576); // 432 pt
    expect(Math.round(box!.height)).toBe(336); // 252 pt
  });

  test("a saved and reopened document draws the same charts", async ({ page }) => {
    await addChart(page, SALES);
    await addChart(page, { kind: "pie", categories: ["a", "b", "c"], series: [{ name: "S", values: [1, 2, 3] }] });
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } }; open(b: Uint8Array, n?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(w.fe.editor.engine.save(), "with-charts.docx");
    });
    await expect(charts(page)).toHaveCount(2);
    await expect(charts(page).first().locator(".chart-bar")).toHaveCount(8);
    await expect(charts(page).nth(1).locator(".chart-slice")).toHaveCount(3);
    expect(await texts(charts(page).first())).toContain("Sales by quarter");
  });

  test("is one labelled picture for screen readers, with its data as a table", async ({ page }) => {
    await addChart(page, SALES);
    const c = charts(page).first();
    await expect(svgOf(c)).toHaveAttribute("role", "img");
    await expect(svgOf(c)).toHaveAttribute("aria-label", /Column chart: Sales by quarter\. 2 series \(Sales, Costs\), 4 categories\./);
    const sr = c.locator(".chart-sr");
    const table = sr.locator("table.chart-data");
    await expect(table.locator("caption")).toHaveText("Sales by quarter");
    await expect(table.locator("tr")).toHaveCount(5);
    await expect(table.locator("tr").nth(2)).toHaveText("Q23118");
    await expect(table.locator("tr").nth(2).locator("td").nth(1)).toHaveText("18");
    // The table isn't on screen.
    const b = await sr.boundingBox();
    expect(b === null || (b.width <= 1 && b.height <= 1)).toBe(true);
  });

  test("text in the drawing uses the paragraph's font, and the colours are the Office palette", async ({ page }) => {
    await addChart(page, SALES);
    const c = charts(page).first();
    const font = await c.evaluate((el) => ({ chart: getComputedStyle(el.querySelector(".chart-t")!).fontFamily, atom: getComputedStyle(el).fontFamily }));
    expect(font.chart).toBe(font.atom);
    const fills = await c.locator(".chart-bar").evaluateAll((els) => els.map((e) => getComputedStyle(e).fill));
    expect(new Set(fills).size).toBe(2);
    const text = await c.locator(".chart-t").first().evaluate((e) => getComputedStyle(e).fill);
    expect(text).toBe("rgb(89, 89, 89)");
  });
});

test.describe("Insert, Chart", () => {
  const sub = (page: Page, kind: string) => (["Doughnut", "Area", "Scatter", "Radar"].includes(kind) ? chooseMenu(page, "Insert", "Chart", "More types", kind) : chooseMenu(page, "Insert", "Chart", kind));

  for (const kind of ["Column", "Bar", "Line", "Pie", "Doughnut", "Area", "Scatter", "Radar"]) {
    test(`${kind} puts a sample chart in the text and selects it`, async ({ page }) => {
      const p = await inPage(page, (w) => w.fe.editor.engine!.renderAll().order[0]!);
      await para(page, p).click();
      await sub(page, kind);
      await expect(charts(page)).toHaveCount(1);
      await expect(svgOf(charts(page).first())).toHaveAttribute("data-kind", kind.toLowerCase());
      await expect(page.locator(".img-frame")).toBeVisible();
      await expect(page.locator(".img-strip")).toContainText("Edit");
      const m = await inPage(page, (w, a) => (w.fe.editor.engine as unknown as { chartAt(p: unknown): Model | null }).chartAt(a), await selectedAt(page));
      expect(m!.kind).toBe(kind.toLowerCase());
      expect(m!.series.length).toBeGreaterThan(0);
      expect(m!.title).toBe("Chart title");
    });
  }

  test("one undo takes the chart out and redo puts it back", async ({ page }) => {
    await chooseMenu(page, "Insert", "Chart", "Column");
    await expect(charts(page)).toHaveCount(1);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(charts(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect(charts(page)).toHaveCount(1);
    await expect(charts(page).first().locator(".chart-bar")).toHaveCount(8);
  });

  test("is greyed out in a document you can only view", async ({ page }) => {
    await chooseMenu(page, "View", "Mode", "Viewing");
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Insert", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Chart/ }).hover();
    await expect(page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Column$/ })).toBeDisabled();
  });
});

/** Where the selected object is (the editor's selection start). */
const selectedAt = (page: Page) => inPage(page, (w) => w.fe.editor.sel!.anchor);

test.describe("Edit chart", () => {
  test("opens on a double-click and shows the type, title, data and a preview", async ({ page }) => {
    await addChart(page, SALES);
    await charts(page).first().scrollIntoViewIfNeeded();
    await svgOf(charts(page).first()).dblclick({ position: { x: 30, y: 30 } });
    const d = dialog(page, "Edit chart");
    await expect(d).toBeVisible();
    await expect(d.getByRole("radio", { name: "Column" })).toHaveAttribute("aria-checked", "true");
    await expect(d.getByLabel("Title")).toHaveValue("Sales by quarter");
    await expect(d.getByLabel("Category 1", { exact: true })).toHaveValue("Q1");
    await expect(d.getByLabel("Series 2 name")).toHaveValue("Costs");
    await expect(d.locator(".chart-preview .chart-bar")).toHaveCount(8);
    // Legend, labels and the rest of the layout have a tab of their own.
    await d.getByRole("tab", { name: "Layout", exact: true }).click();
    await expect(d.getByLabel("Legend position")).toHaveValue("b");
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toHaveCount(0);
  });

  test("changing a number redraws the preview, and Apply changes the chart and the model; undo takes it back", async ({ page }) => {
    const at = await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    const bars = d.locator(".chart-preview .chart-bar");
    const before = await bars.first().getAttribute("height");
    await d.getByLabel("Sales, Q1").fill("48");
    await expect.poll(() => bars.first().getAttribute("height")).not.toBe(before);
    await d.getByLabel("Title").fill("Revenue");
    await expect(d.locator(".chart-preview .chart-title")).toHaveText("Revenue");
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toHaveCount(0);

    const m = (await chartAt(page, at))!;
    expect(m.title).toBe("Revenue");
    expect(m.series[0]!.values).toEqual([48, 31, 29, 42]);
    expect(m.categories).toEqual(["Q1", "Q2", "Q3", "Q4"]);
    await expect(charts(page).first().locator(".chart-title")).toHaveText("Revenue");
    const h = await charts(page).first().locator(".chart-bar").first().getAttribute("height");
    expect(h).not.toBe(before);

    await page.keyboard.press(`${MOD}+Z`);
    expect((await chartAt(page, at))!.series[0]!.values).toEqual([24, 31, 29, 42]);
    await expect(charts(page).first().locator(".chart-title")).toHaveText("Sales by quarter");
  });

  test("adds and removes rows and series", async ({ page }) => {
    const at = await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    await d.getByRole("button", { name: "Add row" }).click();
    await d.getByLabel("Category 5", { exact: true }).fill("Q5");
    await d.getByLabel("Sales, Q5").fill("50");
    await d.getByRole("button", { name: "Add series" }).click();
    await d.getByLabel("Series 3 name").fill("Profit");
    await d.getByLabel("Profit, Q1").fill("9");
    await d.getByRole("button", { name: "Remove row 2" }).click();
    await expect(d.locator(".chart-preview .chart-bar")).toHaveCount(8); // empty cells draw no bar: 4 + 3 + 1
    await d.getByRole("button", { name: "Apply" }).click();
    const m = (await chartAt(page, at))!;
    expect(m.categories).toEqual(["Q1", "Q3", "Q4", "Q5"]);
    expect(m.series.map((s) => s.name)).toEqual(["Sales", "Costs", "Profit"]);
    expect(m.series[0]!.values).toEqual([24, 29, 42, 50]);
    expect(m.series[2]!.values).toEqual([9, null, null, null]);
    await expect(charts(page).first().locator(".chart-bar")).toHaveCount(8);
  });

  test("checks numbers: a bad one is marked, explained, and holds Apply back", async ({ page }) => {
    await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    const apply = d.getByRole("button", { name: "Apply" });
    await d.getByLabel("Sales, Q2").fill("lots");
    await expect(d.getByLabel("Sales, Q2")).toHaveAttribute("aria-invalid", "");
    await expect(d.locator(".chart-problem")).toContainText("aren't numbers");
    await expect(apply).toBeDisabled();
    await d.getByLabel("Sales, Q2").fill("1,234.5");
    await expect(d.locator(".chart-problem")).toHaveText("");
    await expect(apply).toBeEnabled();
    await d.getByLabel("Sales, Q2").fill("");
    await expect(apply).toBeEnabled();
  });

  test("pasting tab-separated cells from a spreadsheet fills the grid and adds what it needs", async ({ page }) => {
    const at = await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    await d.locator("input.chart-cell").first().focus();
    await pasteData(page, { "text/plain": "Mon\t1\t2\t3\nTue\t4\t5\t6\nWed\t7\t8\t9\nThu\t1\t1\t1\nFri\t2\t2\t2\n" }, ".chart-dialog input.chart-cell[data-r='0'][data-c='0']");
    await expect(d.getByLabel("Series 3 name")).toBeVisible();
    await expect(d.getByLabel("Category 5", { exact: true })).toHaveValue("Fri");
    await d.getByRole("button", { name: "Apply" }).click();
    const m = (await chartAt(page, at))!;
    expect(m.categories).toEqual(["Mon", "Tue", "Wed", "Thu", "Fri"]);
    expect(m.series).toHaveLength(3);
    expect(m.series[1]!.values).toEqual([2, 5, 8, 1, 2]);
    expect(m.series[2]!.values).toEqual([3, 6, 9, 1, 2]);
  });

  test("the type buttons switch the chart and keep the data; stacking is offered for bars", async ({ page }) => {
    const at = await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    await d.getByRole("radio", { name: "Line" }).click();
    await expect(d.getByRole("radio", { name: "Line" })).toHaveAttribute("aria-checked", "true");
    await expect(d.locator(".chart-preview .chart-marker")).toHaveCount(8);
    await expect(d.getByLabel("Style")).toBeVisible();
    await d.getByRole("radio", { name: "Pie" }).click();
    await expect(d.getByLabel("Style")).toBeHidden();
    await expect(d.locator(".chart-preview .chart-slice")).toHaveCount(4);
    await d.getByRole("radio", { name: "Column" }).click();
    await d.getByLabel("Style").selectOption("stacked");
    await d.getByRole("button", { name: "Apply" }).click();
    const m = (await chartAt(page, at))!;
    expect(m.kind).toBe("column");
    expect(m.grouping).toBe("stacked");
    await expect(charts(page).first().locator(".chart-bar")).toHaveCount(8);
    // Switching type for real: line.
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    await d.getByRole("radio", { name: "Line" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(svgOf(charts(page).first())).toHaveAttribute("data-kind", "line");
    expect((await chartAt(page, at))!.series[1]!.values).toEqual([15, 18, 22, 24]);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(svgOf(charts(page).first())).toHaveAttribute("data-kind", "column");
  });

  test("legend and data labels", async ({ page }) => {
    const at = await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    await d.getByRole("tab", { name: "Layout", exact: true }).click();
    await d.getByLabel("Legend position").selectOption("none");
    await d.getByLabel("Show data labels").check();
    await expect(d.locator(".chart-preview .chart-label")).toHaveCount(8);
    await d.getByRole("button", { name: "Apply" }).click();
    const m = (await chartAt(page, at))!;
    expect(m.legend).toBeUndefined();
    expect(m.dataLabels).toBe(true);
    await expect(charts(page).first().locator(".chart-label")).toHaveCount(8);
  });

  test("Format, Chart, Edit chart and the right-click menu open it; Replace image is not offered", async ({ page }) => {
    await addChart(page, SALES);
    await select(page, charts(page).first());
    await chooseMenu(page, "Format", "Chart", "Edit chart…");
    await expect(dialog(page, "Edit chart")).toBeVisible();
    await dialog(page, "Edit chart").getByRole("button", { name: "Cancel" }).click();
    await svgOf(charts(page).first()).click({ button: "right", position: { x: 40, y: 40 } });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Edit chart…")).toBeVisible();
    await expect(contextItem(page, "Alt text…")).toBeVisible();
    await expect(contextItem(page, "Replace image…")).toHaveCount(0);
    await contextItem(page, "Edit chart…").click();
    await expect(dialog(page, "Edit chart")).toBeVisible();
  });

  test("a chart Docxtor can't draw is kept as a placeholder, and picking a type replaces it", async ({ page }) => {
    await openChartFile(page, wordChart("ofPieChart"));
    await expect(charts(page)).toHaveCount(1);
    await expect(charts(page).first().locator(".chart-ph")).toHaveCount(1);
    expect(await texts(charts(page).first())).toContain("Quarterly sales");
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Edit chart" }).click();
    const d = dialog(page, "Edit chart");
    await expect(d).toContainText("can't edit this kind of chart yet");
    await d.getByRole("radio", { name: "Line" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(svgOf(charts(page).first())).toHaveAttribute("data-kind", "line");
  });
});

test.describe("resizing and placing a chart", () => {
  test("the corner handle resizes it in proportion and redraws it; undo gives the old size back", async ({ page }) => {
    const at = await addChart(page, SALES);
    const c = charts(page).first();
    await select(page, c);
    const before = (await svgOf(c).boundingBox())!;
    const handle = page.locator(".img-handle.se");
    const hb = (await handle.boundingBox())!;
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
    await page.mouse.down();
    await page.mouse.move(hb.x - 120, hb.y - 80, { steps: 6 });
    // While dragging the chart is drawn again at the new size.
    const mid = (await svgOf(c).boundingBox())!;
    expect(mid.width).toBeLessThan(before.width);
    await page.mouse.up();
    const after = (await svgOf(c).boundingBox())!;
    expect(after.width).toBeLessThan(before.width - 60);
    expect(Math.abs(after.width / after.height - before.width / before.height)).toBeLessThan(0.05);
    await expect(page.locator(".img-size")).toContainText("pt");
    expect((await chartAt(page, at))!.series).toHaveLength(2);
    // The size was written to the drawing (engine geometry), not only the DOM.
    const w = Number(await c.getAttribute("data-w"));
    expect(Math.round(w * (96 / 72))).toBe(Math.round(after.width));
    await page.keyboard.press(`${MOD}+Z`);
    const undone = (await svgOf(charts(page).first()).boundingBox())!;
    expect(Math.round(undone.width)).toBe(Math.round(before.width));
  });

  test("a side handle changes only the width, and the labels fit the new size", async ({ page }) => {
    await addChart(page, { ...SALES, categories: ["First quarter", "Second quarter", "Third quarter", "Fourth quarter"] });
    const c = charts(page).first();
    await select(page, c);
    const before = (await svgOf(c).boundingBox())!;
    const handle = page.locator(".img-handle.e");
    const hb = (await handle.boundingBox())!;
    await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
    await page.mouse.down();
    await page.mouse.move(hb.x - 250, hb.y, { steps: 5 });
    await page.mouse.up();
    const after = (await svgOf(c).boundingBox())!;
    expect(after.width).toBeLessThan(before.width - 150);
    expect(Math.round(after.height)).toBe(Math.round(before.height));
    // Long labels turn sideways instead of running into each other.
    const rotated = await c.locator(".chart-svg text[transform]").count();
    expect(rotated).toBeGreaterThan(0);
  });

  test("wrapping and alignment from the strip place it like a picture", async ({ page }) => {
    const at = await addChart(page, SALES);
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
    expect((await chartAt(page, at))!.series).toHaveLength(2);
  });

  test("Alt text can be written and the chart can be deleted from the strip", async ({ page }) => {
    await addChart(page, SALES);
    await select(page, charts(page).first());
    await page.locator(".img-strip").getByRole("button", { name: "Alt" }).click();
    const d = dialog(page, "Alt text");
    await expect(d.getByLabel("Describe the chart for screen readers")).toBeVisible();
    await d.getByLabel("Describe the chart for screen readers").fill("Sales rise through the year");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(svgOf(charts(page).first())).toHaveAttribute("aria-label", /Column chart/);
    await expect(charts(page).first()).toHaveAttribute("data-alt", "Sales rise through the year");
    await page.locator(".img-strip").getByRole("button", { name: "Delete chart" }).click();
    await expect(charts(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(charts(page)).toHaveCount(1);
  });
});

test.describe("printing", () => {
  test("the print copy has the chart, drawn", async ({ page }) => {
    await page.addInitScript(() => {
      window.print = () => undefined;
    });
    await page.reload();
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await addChart(page, SALES);
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .chart-svg")).toHaveCount(1);
    await expect(page.locator(".docxtor-print .chart-bar")).toHaveCount(8);
    // Pages print paper white with ink text in any theme.
    const fill = await page.locator(".docxtor-print .chart-t").first().evaluate((e) => getComputedStyle(e).fill);
    expect(fill).toBe("rgb(89, 89, 89)");
  });
});

test.describe("words", () => {
  test("the interface is in Spanish when the language is", async ({ page }) => {
    await openFixture(page, "simple.docx", "&lang=es");
    const bar = page.getByRole("menubar", { name: "Barra de menús" });
    const item = (label: string) => page.locator('.menu-drop [role="menuitem"]').filter({ hasText: new RegExp(`^${label}$`) });
    await bar.getByRole("menuitem", { name: "Insertar", exact: true }).click();
    await item("Gráfico").click();
    await item("Columnas").click();
    await expect(charts(page)).toHaveCount(1);
    await select(page, charts(page).first());
    await page.getByRole("button", { name: "Editar gráfico" }).click();
    const d = dialog(page, "Editar gráfico");
    await expect(d.getByRole("radio", { name: "Circular" })).toBeVisible();
    await expect(d.getByRole("button", { name: "Añadir fila" })).toBeVisible();
  });
});
