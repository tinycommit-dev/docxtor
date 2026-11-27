// Table tools: Split cells, Split table, Insert and Delete cells, Distribute rows, Sort table, Convert text to table and
// back, Formula, the direction of text in cells, and where each sits in the Format menu and the right-click menu.
// Results are checked on the page (cells, rows, texts, sizes) and in the model, with undo.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, dialog, expect, inPage, menuItem, openMenu, test, type Locator, type Page } from "./helpers";
import { table } from "./support/table-markup";

const cell = (page: Page, text: string): Locator => page.locator(".flow td").filter({ hasText: new RegExp(`^${text}$`) });
const tableEl = (page: Page, n = 0) => page.locator(".flow table.ft").nth(n);
const open = (page: Page, ...blocks: ({ body: string } | string)[]) => openBuilt(page, { sections: [{ paragraphs: ["Before the table", ...blocks, "After the table"] }] });
const rows = (page: Page, n = 0) => tableEl(page, n).locator("> tbody > tr");
/** The text of every cell, row by row. */
const texts = (page: Page, n = 0): Promise<string[][]> => tableEl(page, n).evaluate((t) => Array.from((t as HTMLTableElement).rows).map((r) => Array.from(r.cells).filter((c) => !c.classList.contains("gap")).map((c) => (c.textContent ?? "").replace(/​/g, "").trim())));
const grid = (page: Page, n = 0) => tableEl(page, n).getAttribute("data-grid");
const paraOf = async (c: Locator) => Number(await c.locator("[data-p]").first().getAttribute("data-p"));
const undo = (page: Page) => page.keyboard.press("ControlOrMeta+z");

/** A table from rows of text. */
function cells(rowsOfText: string[][], tblPr = "", trPr = ""): { body: string } {
  const cols = Math.max(...rowsOfText.map((r) => r.length));
  let x = `<w:tbl><w:tblPr>${tblPr}</w:tblPr><w:tblGrid>${'<w:gridCol w:w="2000"/>'.repeat(cols)}</w:tblGrid>`;
  for (const r of rowsOfText) {
    x += `<w:tr>${trPr ? `<w:trPr>${trPr}</w:trPr>` : ""}`;
    for (const c of r) x += `<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>${c}</w:t></w:r></w:p></w:tc>`;
    x += "</w:tr>";
  }
  return { body: `${x}</w:tbl>` };
}

test.describe("Split cells", () => {
  test("a cell becomes any number of columns and rows, and the table grows to fit", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2000 }));
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Table", "Split cells…");
    const d = dialog(page, "Split cells");
    await expect(d.locator("#sc-cols")).toHaveValue("2");
    await d.locator("#sc-cols").fill("3");
    await d.locator("#sc-rows").fill("2");
    await d.getByRole("button", { name: "Split" }).click();
    await expect(d).toBeHidden();
    // A grid column was added for each new piece, and a row under the cell; the cells beside it carry on through it.
    expect((await grid(page))!.split(",")).toHaveLength(4);
    await expect(rows(page)).toHaveCount(3);
    expect(await texts(page)).toEqual([["r0c0", "", "", "r0c1"], ["", "", ""], ["r1c0", "r1c1"]]);
    // The cell below stretches across the three new columns.
    expect(await cell(page, "r1c0").evaluate((el) => (el as HTMLTableCellElement).colSpan)).toBe(3);
    // r0c1 runs down two rows.
    expect(await cell(page, "r0c1").evaluate((el) => (el as HTMLTableCellElement).rowSpan)).toBe(2);
    await tableEl(page).screenshot({ path: "test-results/table-split-cells.png" });
    await undo(page);
    await expect(rows(page)).toHaveCount(2);
    expect((await grid(page))!.split(",")).toHaveLength(2);
  });

  test("a merged cell is divided where it was joined, and several selected cells can be merged first", async ({ page }) => {
    await open(page, table({ rows: 3, cols: 3, colW: 1500 }));
    await cell(page, "r0c0").click();
    await cell(page, "r1c1").click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Table", "Split cells…");
    const d = dialog(page, "Split cells");
    // The four cells are joined and cut again into the same two by two.
    await expect(d.locator("#sc-merge")).toBeChecked();
    await expect(d.locator("#sc-cols")).toHaveValue("2");
    await expect(d.locator("#sc-rows")).toHaveValue("2");
    await d.locator("#sc-cols").fill("1");
    await d.locator("#sc-rows").fill("3");
    await d.getByRole("button", { name: "Split" }).click();
    // The merged block (two columns wide) is now one column wide by three rows tall... the table gained rows.
    await expect(rows(page)).toHaveCount(4);
    await undo(page);
    await expect(rows(page)).toHaveCount(3);
  });

  test("when the split is refused, the cells that were to be joined first stay as they were", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 3, colW: 1500 }));
    await cell(page, "r1c0").click();
    await cell(page, "r1c1").click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Table", "Split cells…");
    const d = dialog(page, "Split cells");
    // Three columns inside a cell two wide, with the row above cut at the middle: the table cannot do that.
    await d.locator("#sc-cols").fill("3");
    await d.locator("#sc-rows").fill("1");
    await d.getByRole("button", { name: "Split" }).click();
    await expect(d.getByRole("alert")).toContainText("split into fewer columns");
    await d.getByRole("button", { name: "Cancel" }).click();
    expect(await texts(page)).toEqual([["r0c0", "r0c1", "r0c2"], ["r1c0", "r1c1", "r1c2"]]);
  });

  test("one column and one row is refused in plain words", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2 }));
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Table", "Split cells…");
    const d = dialog(page, "Split cells");
    await d.locator("#sc-cols").fill("1");
    await d.getByRole("button", { name: "Split" }).click();
    await expect(d.getByRole("alert")).toContainText("Choose more than one column or row");
    await d.getByRole("button", { name: "Cancel" }).click();
  });
});

test.describe("Split table", () => {
  test("cuts the table before the row with the caret and leaves a paragraph between", async ({ page }) => {
    await open(page, table({ rows: 4, cols: 2 }));
    await cell(page, "r2c1").click();
    await chooseMenu(page, "Format", "Table", "Split table");
    await expect(page.locator(".flow table.ft")).toHaveCount(2);
    expect(await texts(page, 0)).toEqual([["r0c0", "r0c1"], ["r1c0", "r1c1"]]);
    expect(await texts(page, 1)).toEqual([["r2c0", "r2c1"], ["r3c0", "r3c1"]]);
    // Both tables keep their columns, and the caret sits in the paragraph between them.
    expect(await grid(page, 1)).toBe(await grid(page, 0));
    const between = await inPage(page, (w) => {
      const sel = w.fe.editor.sel!;
      const el = document.querySelector(`.flow [data-p="${sel.anchor.p}"]`)!;
      return { inTable: !!el.closest("table"), prev: el.closest(".ftw") ? "" : (el.previousElementSibling?.className ?? ""), next: el.nextElementSibling?.className ?? "" };
    });
    expect(between.inTable).toBe(false);
    await undo(page);
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
  });

  test("the right-click menu of a table has the table commands, and other places do not", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2 }));
    await cell(page, "r1c0").click({ button: "right" });
    await expect(contextItem(page, "Split cells…")).toBeVisible();
    await expect(contextItem(page, "Distribute rows")).toBeVisible();
    await expect(contextItem(page, "Sort table…")).toBeVisible();
    await expect(contextItem(page, "Table properties…")).toBeVisible();
    await expect(contextItem(page, "Borders and shading…")).toBeVisible();
    await page.keyboard.press("Escape");
    // Outside a table the table lines are not in the menu.
    // (The table's strip floats over the left of the paragraph above the table, as far across as its buttons are wide, which is not the same in every browser: the click is made at the right.)
    await page.locator('.flow [data-p="1"]').click({ button: "right", position: { x: 580, y: 8 } });
    await expect(contextItem(page, "Table properties…")).toHaveCount(0);
    await expect(contextItem(page, "Borders and shading…")).toHaveCount(0);
    await page.keyboard.press("Escape");
  });
});

test.describe("Insert and delete cells", () => {
  test("shifting cells right adds a cell and a column; deleting with a shift left takes it away again", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2000 }));
    await cell(page, "r0c1").click();
    await chooseMenu(page, "Format", "Table", "Insert cells…");
    const d = dialog(page, "Insert cells");
    await expect(d.getByRole("radio", { name: "Shift cells right" })).toBeChecked();
    await d.getByRole("button", { name: "Insert" }).click();
    expect(await texts(page)).toEqual([["r0c0", "", "r0c1"], ["r1c0", "r1c1"]]);
    expect((await grid(page))!.split(",")).toHaveLength(3);
    // The shorter row leaves its last column empty, like Word.
    await expect(rows(page).nth(1).locator("td.gap")).toHaveCount(1);
    await undo(page);
    expect(await texts(page)).toEqual([["r0c0", "r0c1"], ["r1c0", "r1c1"]]);
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Table", "Delete cells…");
    const x = dialog(page, "Delete cells");
    await expect(x.getByRole("radio", { name: "Shift cells left" })).toBeChecked();
    await x.getByRole("button", { name: "Delete" }).click();
    expect((await texts(page))[0].filter((t) => t)).toEqual(["r0c1"]);
  });

  test("shifting cells down moves the column's text down and adds a row; up moves it back", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2000 }));
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Table", "Insert cells…");
    const d = dialog(page, "Insert cells");
    await d.getByRole("radio", { name: "Shift cells down" }).check();
    await d.getByRole("button", { name: "Insert" }).click();
    expect(await texts(page)).toEqual([["", "r0c1"], ["r0c0", "r1c1"], ["r1c0", ""]]);
    await tableEl(page).locator("td").first().click();
    await chooseMenu(page, "Format", "Table", "Delete cells…");
    const x = dialog(page, "Delete cells");
    await x.getByRole("radio", { name: "Shift cells up" }).check();
    await x.getByRole("button", { name: "Delete" }).click();
    expect(await texts(page)).toEqual([["r0c0", "r0c1"], ["r1c0", "r1c1"], ["", ""]]);
  });

  test("an entire row or column goes in or out", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2000 }));
    await cell(page, "r1c1").click();
    await chooseMenu(page, "Format", "Table", "Insert cells…");
    const d = dialog(page, "Insert cells");
    await d.getByRole("radio", { name: "Insert an entire row" }).check();
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(rows(page)).toHaveCount(3);
    await cell(page, "r1c1").click();
    await chooseMenu(page, "Format", "Table", "Delete cells…");
    const x = dialog(page, "Delete cells");
    await x.getByRole("radio", { name: "Delete the entire column" }).check();
    await x.getByRole("button", { name: "Delete" }).click();
    expect((await grid(page))!.split(",")).toHaveLength(1);
  });

  test("cells that are merged can't shift, and the dialog says so", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2000 }));
    await cell(page, "r0c0").click();
    await cell(page, "r0c1").click({ modifiers: ["Shift"] });
    await page.getByRole("toolbar", { name: "Table" }).getByRole("button", { name: "Merge" }).click();
    // The merged cell holds the caret.
    await chooseMenu(page, "Format", "Table", "Insert cells…");
    const d = dialog(page, "Insert cells");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(d.getByRole("alert")).toContainText("merged");
    await d.getByRole("button", { name: "Cancel" }).click();
  });
});

test.describe("Distribute rows", () => {
  const heights = (page: Page) => rows(page).evaluateAll((trs) => trs.map((tr) => Math.round(tr.getBoundingClientRect().height)));

  test("makes the rows as tall as each other", async ({ page }) => {
    await open(page, {
      body: `<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid>${[600, 1200, 1800].map((h, i) => `<w:tr><w:trPr><w:trHeight w:val="${h}" w:hRule="atLeast"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>row${i}</w:t></w:r></w:p></w:tc></w:tr>`).join("")}</w:tbl>`,
    });
    const before = await heights(page);
    expect(before[0]).toBeLessThan(before[2]);
    await cell(page, "row1").click();
    await chooseMenu(page, "Format", "Table", "Distribute rows");
    const after = await heights(page);
    expect(Math.max(...after) - Math.min(...after)).toBeLessThanOrEqual(1);
    // The average of 40, 80 and 120 pixels.
    expect(after[0]).toBeCloseTo(80, 0);
    await undo(page);
    expect(await heights(page)).toEqual(before);
  });

  test("with rows selected, only those rows share their height", async ({ page }) => {
    await open(page, {
      body: `<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="3000"/></w:tblGrid>${[600, 1200, 1800, 1200].map((h, i) => `<w:tr><w:trPr><w:trHeight w:val="${h}" w:hRule="atLeast"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>row${i}</w:t></w:r></w:p></w:tc></w:tr>`).join("")}</w:tbl>`,
    });
    await cell(page, "row0").click();
    await cell(page, "row1").click({ modifiers: ["Shift"] });
    await cell(page, "row0").click({ button: "right" });
    await contextItem(page, "Distribute rows").click();
    const after = await heights(page);
    expect(after[0]).toBeCloseTo(after[1], 0);
    expect(after[2]).toBeGreaterThan(after[1]);
  });
});

test.describe("Sort table", () => {
  const body = (page: Page) => texts(page).then((r) => r.map((x) => x[0]));
  const sample = cells([["Name", "Qty"], ["pear", "10"], ["apple", "9"], ["fig", "100"]], '<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="0" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>', "");

  test("sorts the rows by a column as text or as numbers and keeps the header", async ({ page }) => {
    await open(page, sample);
    await cell(page, "pear").click();
    await chooseMenu(page, "Format", "Table", "Sort table…");
    const d = dialog(page, "Sort table");
    await expect(d.locator("#so-field-0 option")).toHaveText(["Column 1", "Column 2"]);
    // The table's own header row (its design shows one) starts as the header.
    await expect(d.getByRole("radio", { name: "Header row", exact: true })).toHaveAttribute("aria-checked", "true");
    await d.locator("#so-field-0").selectOption("2");
    await d.locator("#so-type-0").selectOption("number");
    await d.getByRole("radio", { name: "Descending" }).first().click();
    await d.getByRole("button", { name: "Sort" }).click();
    await expect(d).toBeHidden();
    expect(await body(page)).toEqual(["Name", "fig", "pear", "apple"]);
    // Whole rows move: the quantity stays with its name.
    expect((await texts(page))[1]).toEqual(["fig", "100"]);
    await undo(page);
    expect(await body(page)).toEqual(["Name", "pear", "apple", "fig"]);
  });

  test("with rows selected, only they are sorted; without a header the first row moves too", async ({ page }) => {
    await open(page, sample);
    await cell(page, "pear").click();
    await cell(page, "fig").click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Table", "Sort table…");
    const d = dialog(page, "Sort table");
    await d.getByRole("radio", { name: "No header row" }).click();
    await d.getByRole("button", { name: "Sort" }).click();
    expect(await body(page)).toEqual(["Name", "apple", "fig", "pear"]);
  });

  test("a table with rows merged down can't be sorted, and says why", async ({ page }) => {
    await open(page, cells([["a", "1"], ["b", "2"], ["c", "3"]]));
    await cell(page, "1").click();
    await cell(page, "2").click({ modifiers: ["Shift"] });
    await page.getByRole("toolbar", { name: "Table" }).getByRole("button", { name: "Merge" }).click();
    await tableEl(page).locator("td").first().click();
    await chooseMenu(page, "Format", "Table", "Sort table…");
    const d = dialog(page, "Sort table");
    await d.getByRole("button", { name: "Sort" }).click();
    await expect(d.getByRole("alert")).toContainText("merged");
    await d.getByRole("button", { name: "Cancel" }).click();
  });
});

test.describe("Convert text to table and back", () => {
  test("paragraphs with tabs become a table, and the table becomes text with commas", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Intro", "Name{TAB}Qty", "pear{TAB}10", "fig{TAB}100", "Outro"] }] });
    await page.locator('.flow [data-p="2"]').click();
    await page.locator('.flow [data-p="4"]').click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Table", "Convert text to table…");
    const d = dialog(page, "Convert text to table");
    // Tabs are found in the text, so they are chosen.
    await expect(d.getByRole("radio", { name: "Tabs" })).toBeChecked();
    await expect(d.locator("#ct-note")).toHaveText("3 rows by 2 columns");
    await d.getByRole("button", { name: "Convert" }).click();
    await expect(d).toBeHidden();
    expect(await texts(page)).toEqual([["Name", "Qty"], ["pear", "10"], ["fig", "100"]]);
    await expect(page.locator(".flow .fp").first()).toContainText("Intro");
    await expect(page.locator(".flow")).toContainText("Outro");
    // Back to text.
    await cell(page, "pear").click();
    await chooseMenu(page, "Format", "Table", "Convert table to text…");
    const x = dialog(page, "Convert table to text");
    await x.getByRole("radio", { name: "Commas" }).check();
    await x.getByRole("button", { name: "Convert" }).click();
    await expect(page.locator(".flow table.ft")).toHaveCount(0);
    const flow = await page.locator(".flow").innerText();
    expect(flow).toContain("Name,Qty");
    expect(flow).toContain("pear,10");
    expect(flow).toContain("fig,100");
    await undo(page);
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    await undo(page);
    await expect(page.locator(".flow table.ft")).toHaveCount(0);
    expect(await page.locator(".flow").innerText()).toContain("Name\tQty");
  });

  test("each paragraph can be a cell, in rows of the number of columns asked for", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["a", "b", "c", "d", "e"] }] });
    await page.locator('.flow [data-p="1"]').click();
    await page.locator('.flow [data-p="5"]').click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Table", "Convert text to table…");
    const d = dialog(page, "Convert text to table");
    await expect(d.getByRole("radio", { name: "Paragraphs" })).toBeChecked();
    await d.locator("#ct-cols").fill("2");
    await expect(d.locator("#ct-note")).toHaveText("3 rows by 2 columns");
    await d.locator("#ct-fit").selectOption("window");
    await d.getByRole("button", { name: "Convert" }).click();
    expect(await texts(page)).toEqual([["a", "b"], ["c", "d"], ["e", ""]]);
    expect(await tableEl(page).evaluate((t) => (t as HTMLElement).style.width)).toBe("100%");
  });

  test("another character separates the text", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["x;1", "y;2"] }] });
    await page.locator('.flow [data-p="1"]').click();
    await page.locator('.flow [data-p="2"]').click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Table", "Convert text to table…");
    const d = dialog(page, "Convert text to table");
    await d.getByRole("radio", { name: "Another character" }).check();
    await d.locator("#ct-other").fill(";");
    await expect(d.locator("#ct-note")).toHaveText("2 rows by 2 columns");
    await d.getByRole("button", { name: "Convert" }).click();
    expect(await texts(page)).toEqual([["x", "1"], ["y", "2"]]);
  });

  test("Convert text to table is off inside a table, Convert table to text is on", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2 }));
    await cell(page, "r0c0").click();
    await openMenu(page, "Format");
    await menuItem(page, "Table").click();
    await expect(menuItem(page, "Convert table to text…")).toBeVisible();
    await expect(menuItem(page, "Convert text to table…")).toBeDisabled();
    await page.keyboard.press("Escape");
  });
});

test.describe("Formula", () => {
  const sheet = cells([["Item", "Jan", "Feb"], ["Rent", "100", "200"], ["Food", "5.5", "6"], ["Total", "", ""]]);

  test("inserts a formula field that works out the sum of the cells above", async ({ page }) => {
    await open(page, sheet);
    await cell(page, "Total").locator("xpath=following-sibling::td[1]").click();
    await chooseMenu(page, "Format", "Table", "Formula…");
    const d = dialog(page, "Formula");
    // Word's suggestion for a cell under numbers.
    await expect(d.locator("#fm-formula")).toHaveValue("=SUM(ABOVE)");
    await expect(d.locator("#fm-preview")).toHaveText("Result: 105.5");
    await d.locator("#fm-format").selectOption("0.00");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(d).toBeHidden();
    const c = (await texts(page))[3][1];
    expect(c).toBe("105.50");
    // The field keeps its code, so it can be updated, and the table shows the result.
    const fields = await inPage(page, (w) => (w.fe.editor.engine as unknown as { listFields(): { code?: string; instr?: string }[] }).listFields?.() ?? []);
    expect(JSON.stringify(fields)).toContain("SUM(ABOVE)");
    await undo(page);
    expect((await texts(page))[3][1]).toBe("");
  });

  test("suggests the sum to the left when the numbers are beside the cell", async ({ page }) => {
    await open(page, cells([["3", "4", ""]]));
    await tableEl(page).locator("td").nth(2).click();
    await chooseMenu(page, "Format", "Table", "Formula…");
    const d = dialog(page, "Formula");
    await expect(d.locator("#fm-formula")).toHaveValue("=SUM(LEFT)");
    await expect(d.locator("#fm-preview")).toHaveText("Result: 7");
    // A function and a bookmark can be pasted in; a bad formula says what is wrong.
    await d.locator("#fm-formula").fill("=");
    await d.locator("#fm-function").selectOption("MAX");
    await expect(d.locator("#fm-formula")).toHaveValue("=MAX()");
    await d.locator("#fm-formula").fill("=3 +");
    await expect(d.locator("#fm-preview")).toContainText("Not a formula yet");
    await d.getByRole("button", { name: "Cancel" }).click();
  });
});

test.describe("Text direction", () => {
  test("Text down and Text up turn the text of the selected cells", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 1800, trPr: '<w:trHeight w:val="1400" w:hRule="atLeast"/>' }));
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Table", "Text direction", "Text down");
    const wm = (t: string) => cell(page, t).evaluate((el) => getComputedStyle(el).writingMode);
    expect(await wm("r0c0")).toBe("vertical-rl");
    expect(await wm("r0c1")).toBe("horizontal-tb");
    // The cell is taller than wide as the text runs down it.
    const box = await cell(page, "r0c0").boundingBox();
    expect(box!.height).toBeGreaterThan(box!.width / 2);
    await cell(page, "r1c1").click();
    await chooseMenu(page, "Format", "Table", "Text direction", "Text up");
    expect(await wm("r1c1")).toBe("sideways-lr");
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Table", "Text direction", "Text across");
    expect(await wm("r0c0")).toBe("horizontal-tb");
    await page.screenshot({ path: "test-results/table-text-direction.png" });
  });
});

test.describe("menus", () => {
  test("Format, Table lists the table commands, each with an icon, and they are greyed or hidden outside a table", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2 }));
    await cell(page, "r0c0").click();
    await openMenu(page, "Format");
    await menuItem(page, "Table").click();
    for (const name of ["Insert cells…", "Split cells…", "Split table", "Distribute rows", "Delete cells…", "Sort table…", "Formula…", "Text direction", "Convert table to text…", "Table properties…"]) {
      const item = menuItem(page, name);
      await expect(item, name).toBeVisible();
      await expect(item.locator("svg").first(), `${name} has an icon`).toBeVisible();
    }
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    // Outside the table the table commands are not offered.
    await page.locator('.flow [data-p="1"]').click({ position: { x: 580, y: 8 } });
    await openMenu(page, "Insert");
    await page.keyboard.press("Escape");
    // The toolbar's table button offers the dialog under its grid.
    await page.getByRole("button", { name: "Table", exact: true }).first().click();
    await page.getByRole("button", { name: "Insert table…" }).click();
    await expect(dialog(page, "Insert table")).toBeVisible();
    await page.keyboard.press("Escape");
  });
});
