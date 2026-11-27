// The Table properties dialog (Table, Row, Column, Cell and Alt text tabs), the Insert table dialog and the table and cell
// targets of Borders and shading. Each is checked in the document model and on the page: widths, alignment, row heights,
// margins, text direction, and the lines a cell and a table draw.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, dialog, expect, inPage, test, type Locator, type Page } from "./helpers";
import { table } from "./support/table-markup";

const cell = (page: Page, text: string): Locator => page.locator(".flow td").filter({ hasText: new RegExp(`^${text}$`) });
const tableEl = (page: Page, n = 0) => page.locator(".flow table.ft").nth(n);
const open = (page: Page, ...blocks: ({ body: string } | string)[]) => openBuilt(page, { sections: [{ paragraphs: ["Before the table", ...blocks, "After the table"] }] });
const paraOf = async (c: Locator) => Number(await c.locator("[data-p]").first().getAttribute("data-p"));
const details = (page: Page, p: number) =>
  inPage(page, (w, id) => (w.fe.editor.engine as unknown as { tableDetails(p: number): Record<string, any> | null }).tableDetails(id), p);
const px = (v: string) => parseFloat(v);

async function openProperties(page: Page, tab?: string) {
  await chooseMenu(page, "Format", "Table", "Table properties…");
  const d = dialog(page, "Table properties");
  await expect(d).toBeVisible();
  if (tab) await d.getByRole("tab", { name: tab, exact: true }).click();
  return d;
}

test.describe("Table properties", () => {
  test("opens from the Format menu and the right-click menu and shows what the table has", async ({ page }) => {
    await open(
      page,
      table({
        rows: 3,
        cols: 2,
        tblPr: '<w:tblW w:w="5000" w:type="pct"/><w:jc w:val="center"/><w:tblInd w:w="360" w:type="dxa"/><w:tblLayout w:type="fixed"/><w:tblCaption w:val="Sales"/><w:tblDescription w:val="Sales by region"/>',
        trPr: '<w:cantSplit/><w:trHeight w:val="720" w:hRule="exact"/>',
        tcPr: (r, c) => (r === 0 && c === 0 ? '<w:vAlign w:val="bottom"/>' : ""),
      }),
    );
    await cell(page, "r0c0").click();
    const d = await openProperties(page);
    await expect(d.locator("#tp-width-kind")).toHaveValue("pct");
    await expect(d.locator("#tp-width-value")).toHaveValue("100");
    await expect(d.locator("#tp-align")).toHaveValue("center");
    await expect(d.locator("#tp-indent")).toHaveValue("0.25");
    await expect(d.locator("#tp-autofit")).not.toBeChecked();
    await d.getByRole("tab", { name: "Row", exact: true }).click();
    await expect(d.locator("#tp-height-on")).toBeChecked();
    await expect(d.locator("#tp-height")).toHaveValue("0.5");
    await expect(d.locator("#tp-rule")).toHaveValue("exact");
    await expect(d.locator("#tp-break")).not.toBeChecked();
    await expect(d.locator("#tp-header")).not.toBeChecked();
    await d.getByRole("tab", { name: "Cell", exact: true }).click();
    await expect(d.locator("#tp-valign")).toHaveValue("bottom");
    await d.getByRole("tab", { name: "Alt text", exact: true }).click();
    await expect(d.locator("#tp-title")).toHaveValue("Sales");
    await expect(d.locator("#tp-description")).toHaveValue("Sales by region");
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toBeHidden();

    await cell(page, "r1c1").click({ button: "right" });
    await contextItem(page, "Table properties…").click();
    await expect(dialog(page, "Table properties")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog(page, "Table properties")).toBeHidden();
  });

  test("size, alignment, indent, autofit and default margins are written, shown, and undone in one step", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2000, tblPr: '<w:tblW w:w="0" w:type="auto"/>' }));
    await cell(page, "r0c0").click();
    const d = await openProperties(page);
    await d.locator("#tp-width-kind").selectOption("dxa");
    await d.locator("#tp-width-value").fill("4");
    await d.locator("#tp-align").selectOption("center");
    await d.locator("#tp-indent").fill("0.5");
    await d.locator("#tp-autofit").uncheck();
    await d.locator("#tp-mt").fill("0.05");
    await d.locator("#tp-mb").fill("0.05");
    await d.locator("#tp-ml").fill("0.2");
    await d.locator("#tp-mr").fill("0.2");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toBeHidden();

    const info = await details(page, await paraOf(cell(page, "r0c0")));
    expect(info!.table).toMatchObject({ width: { type: "dxa", value: 5760 }, align: "center", autofit: false, cellMargins: [72, 288, 72, 288] });
    // On the page: four inches wide, fixed columns, centred, with the new margins in every cell.
    const t = tableEl(page);
    expect(px(await t.evaluate((el) => getComputedStyle(el).width))).toBeCloseTo(2000 / 15 * 2, 0); // the columns are 2000 twips each; the grid sets the width
    expect(await t.evaluate((el) => getComputedStyle(el).tableLayout)).toBe("fixed");
    expect(await t.evaluate((el) => getComputedStyle(el).marginLeft)).not.toBe("0px");
    // (Safari writes a length as the float it holds, 19.200001px, where Chrome and Firefox write 19.2px.)
    expect((await cell(page, "r1c1").evaluate((el) => [getComputedStyle(el).paddingTop, getComputedStyle(el).paddingLeft])).map(parseFloat)).toEqual([expect.closeTo(4.8, 3), expect.closeTo(19.2, 3)]);

    // One undo takes all of it back.
    await page.keyboard.press("ControlOrMeta+z");
    const back = await details(page, await paraOf(cell(page, "r0c0")));
    expect(back!.table).toMatchObject({ width: { type: "auto" }, align: "left", autofit: true });
  });

  test("text wrapping Around makes the table float, and None puts it back in line", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 1500 }));
    await cell(page, "r0c0").click();
    let d = await openProperties(page);
    await d.locator("#tp-wrap").selectOption("around");
    await d.locator("#tp-align").selectOption("right");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(page.locator(".flow .ftw.tfloat")).toHaveCount(1);
    expect(await page.locator(".flow .ftw.tfloat").getAttribute("data-float")).toBe("right");
    d = await openProperties(page);
    await expect(d.locator("#tp-wrap")).toHaveValue("around");
    await d.locator("#tp-wrap").selectOption("none");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(page.locator(".flow .ftw.tfloat")).toHaveCount(0);
  });

  test("the Row tab sets the height, keeps rows whole and repeats header rows for the selected rows", async ({ page }) => {
    await open(page, table({ rows: 4, cols: 2, colW: 2000 }));
    await cell(page, "r0c0").click();
    await cell(page, "r1c1").click({ modifiers: ["Shift"] });
    const d = await openProperties(page, "Row");
    await expect(d.locator("#tp-rows")).toHaveText("Rows 1 to 2");
    await d.locator("#tp-height-on").check();
    await d.locator("#tp-height").fill("0.5");
    await d.locator("#tp-rule").selectOption("exact");
    await d.locator("#tp-break").uncheck();
    await d.locator("#tp-header").check();
    await d.getByRole("button", { name: "OK" }).click();
    const rows = tableEl(page).locator("tr");
    for (const i of [0, 1]) {
      await expect(rows.nth(i)).toHaveAttribute("data-exact", "1");
      await expect(rows.nth(i)).toHaveAttribute("data-cs", "1");
      await expect(rows.nth(i)).toHaveAttribute("data-header", "1");
      expect(px(await rows.nth(i).evaluate((el) => (el as HTMLElement).style.height))).toBeCloseTo(48, 0);
    }
    await expect(rows.nth(2)).not.toHaveAttribute("data-exact", "1");
    // The change is one undo step.
    await page.keyboard.press("ControlOrMeta+z");
    await expect(rows.nth(0)).not.toHaveAttribute("data-header", "1");
  });

  test("a header row anywhere but the top is refused with a plain message", async ({ page }) => {
    await open(page, table({ rows: 3, cols: 2 }));
    await cell(page, "r2c0").click();
    const d = await openProperties(page, "Row");
    await d.locator("#tp-header").check();
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d.getByRole("alert")).toContainText("top of the table");
    await expect(d).toBeVisible();
    await d.getByRole("button", { name: "Cancel" }).click();
  });

  test("the Column tab sets a column's width as a length or a percentage", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 3, colW: 2000 }));
    await cell(page, "r0c1").click();
    let d = await openProperties(page, "Column");
    await expect(d.locator("#tp-cols")).toHaveText("Column 2");
    await d.locator("#tp-col-kind").selectOption("dxa");
    await d.locator("#tp-col-value").fill("2.5");
    await d.getByRole("button", { name: "OK" }).click();
    expect((await details(page, await paraOf(cell(page, "r0c1"))))!.grid).toEqual([2000, 3600, 2000]);
    const w = await tableEl(page).locator("col").nth(1).evaluate((el) => (el as HTMLElement).style.width);
    expect(px(w)).toBeCloseTo(240, 0);
    d = await openProperties(page, "Column");
    await expect(d.locator("#tp-col-kind")).toHaveValue("dxa");
    await expect(d.locator("#tp-col-value")).toHaveValue("2.5");
    await d.locator("#tp-col-kind").selectOption("pct");
    await d.locator("#tp-col-value").fill("40");
    await d.getByRole("button", { name: "OK" }).click();
    const info = await details(page, await paraOf(cell(page, "r1c1")));
    expect(info!.columnWidth).toEqual({ type: "pct", value: 2000 });
  });

  test("the Cell tab sets vertical alignment, margins, wrapping and the direction of the text", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 2400, trPr: '<w:trHeight w:val="1200" w:hRule="atLeast"/>' }));
    await cell(page, "r0c0").click();
    const d = await openProperties(page, "Cell");
    await d.locator("#tp-valign").selectOption("bottom");
    await d.locator("#tp-dir").selectOption("btLr");
    await d.locator("#tp-cell-margins").check();
    await d.locator("#tp-cmt").fill("0.1");
    await d.locator("#tp-cml").fill("0.3");
    await d.locator("#tp-nowrap").check();
    await d.getByRole("button", { name: "OK" }).click();
    const c = cell(page, "r0c0");
    const css = await c.evaluate((el) => {
      const s = getComputedStyle(el);
      return { v: s.verticalAlign, wm: s.writingMode, ws: s.whiteSpace, pt: s.paddingTop, pl: s.paddingLeft };
    });
    // (Safari writes a length as the float it holds, 28.799999px, where Chrome and Firefox write 28.8px.)
    expect({ ...css, pt: parseFloat(css.pt), pl: parseFloat(css.pl) }).toEqual({ v: "bottom", wm: "sideways-lr", ws: "nowrap", pt: expect.closeTo(9.6, 3), pl: expect.closeTo(28.8, 3) });
    // Only that cell changed.
    expect(await cell(page, "r0c1").evaluate((el) => getComputedStyle(el).writingMode)).toBe("horizontal-tb");
    const info = await details(page, await paraOf(c));
    expect(info!.cell).toMatchObject({ valign: "bottom", textDirection: "btLr", noWrap: true, margins: [144, 108, 0, 432] });
    await page.locator(".flow table").first().screenshot({ path: "test-results/table-cell-direction.png" });
    // Down is the other way round.
    const d2 = await openProperties(page, "Cell");
    await expect(d2.locator("#tp-dir")).toHaveValue("btLr");
    await d2.locator("#tp-dir").selectOption("tbRl");
    await d2.locator("#tp-cell-margins").uncheck();
    await d2.getByRole("button", { name: "OK" }).click();
    expect(await c.evaluate((el) => getComputedStyle(el).writingMode)).toBe("vertical-rl");
    expect((await details(page, await paraOf(c)))!.cell.margins ?? null).toBeNull();
  });

  test("alt text is a title and a description on the table", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2 }));
    await cell(page, "r1c0").click();
    const d = await openProperties(page, "Alt text");
    await d.locator("#tp-title").fill("Quarterly figures");
    await d.locator("#tp-description").fill("Revenue for each quarter");
    await d.getByRole("button", { name: "OK" }).click();
    const info = await details(page, await paraOf(cell(page, "r0c0")));
    expect(info!.table).toMatchObject({ title: "Quarterly figures", description: "Revenue for each quarter" });
  });
});

test.describe("Insert table", () => {
  test("rows, columns and how the columns are sized", async ({ page }) => {
    await open(page);
    await page.locator('.flow [data-p="1"]').click({ position: { x: 580, y: 8 } });
    await chooseMenu(page, "Format", "Table", "Insert table…");
    const d = dialog(page, "Insert table");
    await d.locator("#it-cols").fill("4");
    await d.locator("#it-rows").fill("2");
    await d.locator("#it-fit").selectOption("window");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(tableEl(page).locator("tr")).toHaveCount(2);
    await expect(tableEl(page).locator("tr").first().locator("td")).toHaveCount(4);
    const info = await details(page, await paraOf(tableEl(page).locator("td").first()));
    expect(info!.table).toMatchObject({ width: { type: "pct", value: 5000 }, autofit: true });
    // It fills the text column.
    const page0 = await page.locator(".flow .fp").first().evaluate((el) => el.getBoundingClientRect().width);
    expect(await tableEl(page).evaluate((el) => el.getBoundingClientRect().width)).toBeGreaterThan(page0 * 0.6);

    await page.locator('.flow [data-p="1"]').click({ position: { x: 580, y: 8 } });
    await chooseMenu(page, "Format", "Table", "Insert table…");
    const d2 = dialog(page, "Insert table");
    await d2.locator("#it-cols").fill("3");
    await d2.locator("#it-rows").fill("1");
    await d2.locator("#it-fit").selectOption("fixed");
    await d2.locator("#it-width").fill("1.5");
    await d2.getByRole("button", { name: "Insert" }).click();
    const fixed = await inPage(page, (w) => {
      const t = Array.from(document.querySelectorAll<HTMLElement>(".flow table.ft")).find((x) => x.dataset.grid === "2160,2160,2160");
      return t ? { grid: t.dataset.grid, layout: getComputedStyle(t).tableLayout } : null;
    });
    expect(fixed).toEqual({ grid: "2160,2160,2160", layout: "fixed" });
  });

  test("inside a table it makes a table in the cell, from the toolbar", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 3000 }));
    await cell(page, "r0c0").click();
    await page.getByRole("button", { name: "Table", exact: true }).first().click();
    await page.getByRole("button", { name: "Insert table in cell…" }).click();
    const d = dialog(page, "Insert table");
    await expect(d).toContainText("The table goes in the cell the caret is in.");
    await d.locator("#it-cols").fill("2");
    await d.locator("#it-rows").fill("2");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(page.locator(".flow td table.ft")).toHaveCount(1);
    await expect(page.locator(".flow td table.ft tr")).toHaveCount(2);
    // The outer table is unchanged and the inner one sits in its first cell.
    await expect(tableEl(page).locator("> tbody > tr")).toHaveCount(2);
    await expect(tableEl(page).locator("> tbody > tr").first().locator("> td").first().locator("table.ft")).toHaveCount(1);
    await page.getByText("After the table").click();
    await page.screenshot({ path: "test-results/table-nested.png" });
  });
});

test.describe("Borders and shading in a table", () => {
  const sides = async (c: Locator) =>
    c.evaluate((el) => {
      const s = getComputedStyle(el);
      return { top: [s.borderTopStyle, s.borderTopWidth, s.borderTopColor], bottom: [s.borderBottomStyle, s.borderBottomWidth, s.borderBottomColor], left: [s.borderLeftStyle, s.borderLeftWidth], right: [s.borderRightStyle, s.borderRightWidth] };
    });

  test("Apply to Cell draws the chosen edges of the selected cell only", async ({ page }) => {
    await open(page, table({ rows: 3, cols: 3, colW: 1800 }));
    await cell(page, "r1c1").click();
    await chooseMenu(page, "Format", "Borders and shading…");
    const d = dialog(page, "Borders and shading");
    await expect(d.getByRole("radio", { name: "Cell" })).toHaveAttribute("aria-checked", "true");
    // A single cell has no lines between cells, but it has diagonals.
    await expect(d.locator('[data-edge="insideH"]')).toBeHidden();
    await expect(d.locator('[data-edge="diagDown"]')).toBeVisible();
    await d.locator('[data-edge="top"]').click();
    await d.locator('[data-edge="bottom"]').click();
    await d.getByRole("radio", { name: "3 pt" }).click();
    await d.getByRole("radio", { name: "Dashed" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    const c = cell(page, "r1c1");
    const s = await sides(c);
    expect(s.top).toEqual(["dashed", "4px", "rgb(0, 0, 0)"]);
    expect(s.bottom[0]).toBe("dashed");
    expect(s.left[0]).toBe("none");
    // The cells above and below share those lines; a cell elsewhere has none.
    expect((await sides(cell(page, "r0c1"))).bottom[0]).toBe("dashed");
    expect((await sides(cell(page, "r2c1"))).top[0]).toBe("dashed");
    expect((await sides(cell(page, "r0c0"))).top[0]).toBe("none");
    await page.locator(".flow table").first().screenshot({ path: "test-results/table-cell-borders.png" });
    // Undo.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await sides(c)).top[0]).toBe("none");
  });

  test("selected cells get an outside box and the line between them; a diagonal is drawn across a cell", async ({ page }) => {
    await open(page, table({ rows: 3, cols: 3, colW: 1800 }));
    await cell(page, "r0c0").click();
    await cell(page, "r1c1").click({ modifiers: ["Shift"] });
    await chooseMenu(page, "Format", "Borders and shading…");
    const d = dialog(page, "Borders and shading");
    await expect(d.locator('[data-edge="insideH"]')).toBeVisible();
    await d.locator(".bs-row").getByRole("button", { name: "All", exact: true }).click();
    await d.getByRole("radio", { name: "2¼ pt" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    for (const t of ["r0c0", "r0c1", "r1c0", "r1c1"]) {
      const s = await sides(cell(page, t));
      expect([s.top[0], s.bottom[0], s.left[0], s.right[0]]).toEqual(["solid", "solid", "solid", "solid"]);
    }
    // Outside the block the cells are as they were, except where they share its edge.
    expect((await sides(cell(page, "r2c2"))).top[0]).toBe("none");
    expect((await sides(cell(page, "r0c2"))).left[0]).toBe("solid");
  });

  test("Apply to Table draws every line of the table and None takes them away", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, colW: 1800 }));
    await cell(page, "r0c0").click();
    await chooseMenu(page, "Format", "Borders and shading…");
    let d = dialog(page, "Borders and shading");
    await d.getByRole("radio", { name: "Table" }).click();
    await d.locator(".bs-row").getByRole("button", { name: "All", exact: true }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    for (const t of ["r0c0", "r0c1", "r1c0", "r1c1"]) {
      const s = await sides(cell(page, t));
      expect([s.top[0], s.bottom[0], s.left[0], s.right[0]]).toEqual(["solid", "solid", "solid", "solid"]);
    }
    await cell(page, "r1c1").click();
    await chooseMenu(page, "Format", "Borders and shading…");
    d = dialog(page, "Borders and shading");
    await d.getByRole("radio", { name: "Table" }).click();
    // It shows what the table has.
    await expect(d.locator('[data-edge="insideV"]')).toHaveAttribute("aria-pressed", "true");
    await d.locator(".bs-row").getByRole("button", { name: "None", exact: true }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    for (const t of ["r0c0", "r0c1", "r1c0", "r1c1"]) {
      const s = await sides(cell(page, t));
      expect([s.top[0], s.bottom[0], s.left[0], s.right[0]]).toEqual(["none", "none", "none", "none"]);
    }
  });

  test("the table's Borders and shading button in Table properties opens it on the table", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2 }));
    await cell(page, "r0c0").click();
    const d = await openProperties(page);
    await d.getByRole("button", { name: "Borders and shading…" }).click();
    const b = dialog(page, "Borders and shading");
    await expect(b.getByRole("radio", { name: "Table" })).toHaveAttribute("aria-checked", "true");
    await b.getByRole("button", { name: "Cancel" }).click();
  });

  test("paragraph borders outside a table still work as before", async ({ page }) => {
    await open(page);
    await page.locator('.flow [data-p="1"]').click({ position: { x: 580, y: 8 } });
    await chooseMenu(page, "Format", "Borders and shading…");
    const d = dialog(page, "Borders and shading");
    await expect(d.getByRole("radio", { name: "Cell" })).toHaveCount(0);
    await expect(d.locator('[data-edge="insideH"]')).toBeHidden();
    await d.locator('[data-edge="bottom"]').click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await page.locator('.flow [data-p="1"]').evaluate((el) => getComputedStyle(el).borderBottomStyle)).toBe("solid");
  });
});
