// Table styles and the design gallery. A table in one of Word's built-in styles shows its header row, banded rows, first
// column and total row as Word draws them, the options of `w:tblLook` switch them, and Format, Table, Table design
// gives a table any of the built-in designs (written into the document as Word writes them) with the options Word has.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, menuItem, openMenu, para, test, type Locator, type Page } from "./helpers";
import { GRID_TABLE_4_ACCENT_1, GRID_TABLE_5_DARK, LIST_TABLE_3, LOOK, LOOK_DEFAULT, PLAIN_TABLE_1, TABLE_GRID, TABLE_NORMAL, table } from "./support/table-markup";

const STYLES = TABLE_NORMAL + TABLE_GRID + GRID_TABLE_4_ACCENT_1 + PLAIN_TABLE_1 + LIST_TABLE_3 + GRID_TABLE_5_DARK;

const cell = (page: Page, text: string): Locator => page.locator(".flow td").filter({ hasText: new RegExp(`^${text}$`) });
const bg = (td: Locator) => td.evaluate((el) => getComputedStyle(el).backgroundColor);
const weight = (td: Locator) => td.locator(".fp").first().evaluate((el) => getComputedStyle(el.querySelector("span, b, i") ?? el).fontWeight);
const color = (td: Locator) => td.locator(".fp").first().evaluate((el) => getComputedStyle(el.querySelector("span") ?? el).color);
const border = (td: Locator, side: "Top" | "Bottom" | "Left" | "Right") => td.evaluate((el, s) => getComputedStyle(el)[`border${s}Color` as "borderTopColor"], side);

const open = (page: Page, tbl: ReturnType<typeof table>) => openBuilt(page, { styles: STYLES, sections: [{ paragraphs: ["Before the table", tbl, "After the table"] }] });

const rgb = (hex: string) => {
  const n = parseInt(hex, 16);
  return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`;
};

test.describe("table styles", () => {
  test("Grid Table 4 Accent 1 shows its header row, banded rows and first column", async ({ page }) => {
    await open(page, table({ rows: 5, cols: 3, tblPr: `<w:tblStyle w:val="GridTable4-Accent1"/><w:tblW w:w="0" w:type="auto"/>${LOOK_DEFAULT}` }));
    // The header row is filled with the accent colour and its text is bold and white.
    for (const c of ["r0c0", "r0c1", "r0c2"]) {
      expect(await bg(cell(page, c))).toBe(rgb("4472C4"));
      expect(Number(await weight(cell(page, c)))).toBeGreaterThanOrEqual(600);
      expect(await color(cell(page, c))).toBe(rgb("FFFFFF"));
    }
    // Banding starts under the header: the first body row is tinted, the next is not.
    expect(await bg(cell(page, "r1c1"))).toBe(rgb("D9E2F3"));
    expect(await bg(cell(page, "r2c1"))).toBe("rgba(0, 0, 0, 0)");
    expect(await bg(cell(page, "r3c1"))).toBe(rgb("D9E2F3"));
    // The first column is bold, the other columns are not.
    expect(Number(await weight(cell(page, "r2c0")))).toBeGreaterThanOrEqual(600);
    expect(Number(await weight(cell(page, "r2c1")))).toBeLessThan(600);
    // Light blue lines between the body cells.
    expect(await border(cell(page, "r2c1"), "Top")).toBe(rgb("8EAADB"));
    await page.locator(".flow table").screenshot({ path: "test-results/table-grid-4-accent-1.png" });
  });

  test("the options of the table's look switch the parts of the style off and on", async ({ page }) => {
    // Header row off, total row on, banded columns on, first column off.
    await open(page, table({ rows: 4, cols: 3, tblPr: `<w:tblStyle w:val="GridTable4-Accent1"/>${LOOK("0060", { lastRow: true })}` }));
    expect(await bg(cell(page, "r0c1"))).not.toBe(rgb("4472C4"));
    // Banded rows follow from the old hex value alone (no noHBand bit), starting with the first row.
    expect(await bg(cell(page, "r0c1"))).toBe(rgb("D9E2F3"));
    expect(Number(await weight(cell(page, "r0c1")))).toBeLessThan(600);
    // The total row is bold with a double line above it.
    expect(Number(await weight(cell(page, "r3c1")))).toBeGreaterThanOrEqual(600);
    expect(await cell(page, "r3c1").evaluate((el) => getComputedStyle(el).borderTopStyle)).toBe("double");
    expect(Number(await weight(cell(page, "r1c0")))).toBeLessThan(600);
  });

  test("banded rows can be two rows high and a table without a look shows only the whole-table formatting", async ({ page }) => {
    const style = PLAIN_TABLE_1.replace('<w:tblStyleRowBandSize w:val="1"/>', '<w:tblStyleRowBandSize w:val="2"/>');
    await openBuilt(page, { styles: TABLE_NORMAL + style, sections: [{ paragraphs: ["Before", table({ rows: 6, cols: 2, tblPr: `<w:tblStyle w:val="PlainTable1"/>${LOOK("0420", { firstRow: true })}` })] }] });
    // Two rows tinted, two plain, two tinted.
    const fills = [];
    for (let r = 1; r < 6; r++) fills.push(await bg(cell(page, `r${r}c1`)));
    expect(fills).toEqual([rgb("F2F2F2"), rgb("F2F2F2"), "rgba(0, 0, 0, 0)", "rgba(0, 0, 0, 0)", rgb("F2F2F2")]);
    await openBuilt(page, { styles: TABLE_NORMAL + PLAIN_TABLE_1, sections: [{ paragraphs: ["Before", table({ rows: 3, cols: 2, tblPr: '<w:tblStyle w:val="PlainTable1"/>' })] }] });
    expect(await bg(cell(page, "r1c1"))).toBe("rgba(0, 0, 0, 0)");
    expect(Number(await weight(cell(page, "r0c0")))).toBeLessThan(600);
  });

  test("cnfStyle says what a row is when the table has no look", async ({ page }) => {
    await open(page, {
      body: `<w:tbl><w:tblPr><w:tblStyle w:val="GridTable4-Accent1"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:trPr><w:cnfStyle w:val="100000000000" w:firstRow="1"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>r0c0</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>r1c0</w:t></w:r></w:p></w:tc></w:tr></w:tbl>`,
    });
    expect(await bg(cell(page, "r0c0"))).toBe(rgb("4472C4"));
    expect(await bg(cell(page, "r1c0"))).toBe("rgba(0, 0, 0, 0)");
  });

  test("direct formatting of a cell wins over the style", async ({ page }) => {
    await open(page, table({ rows: 2, cols: 2, tblPr: `<w:tblStyle w:val="GridTable4-Accent1"/>${LOOK_DEFAULT}`, tcPr: (r, c) => (r === 0 && c === 0 ? '<w:tcBorders><w:bottom w:val="nil"/></w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="FFFF00"/>' : "") }));
    expect(await bg(cell(page, "r0c0"))).toBe(rgb("FFFF00"));
    expect(await bg(cell(page, "r0c1"))).toBe(rgb("4472C4"));
    expect(await cell(page, "r0c0").evaluate((el) => getComputedStyle(el).borderBottomStyle)).toBe("none");
  });

  test("List Table 3, Plain Table 1 and Grid Table 5 Dark draw their designs", async ({ page }) => {
    await open(page, table({ rows: 4, cols: 3, tblPr: `<w:tblStyle w:val="ListTable3"/>${LOOK_DEFAULT}` }));
    expect(await bg(cell(page, "r0c1"))).toBe(rgb("000000"));
    expect(await color(cell(page, "r0c1"))).toBe(rgb("FFFFFF"));
    expect(await border(cell(page, "r2c0"), "Left")).toBe(rgb("000000"));
    await page.locator(".flow table").screenshot({ path: "test-results/table-list-3.png" });
    await open(page, table({ rows: 4, cols: 3, tblPr: `<w:tblStyle w:val="PlainTable1"/>${LOOK_DEFAULT}` }));
    expect(await bg(cell(page, "r1c1"))).toBe(rgb("F2F2F2"));
    expect(await border(cell(page, "r0c1"), "Bottom")).toBe(rgb("BFBFBF"));
    await page.locator(".flow table").screenshot({ path: "test-results/table-plain-1.png" });
    await open(page, table({ rows: 4, cols: 3, tblPr: `<w:tblStyle w:val="GridTable5Dark"/>${LOOK_DEFAULT}` }));
    expect(await bg(cell(page, "r0c1"))).toBe(rgb("000000"));
    expect(await bg(cell(page, "r2c0"))).toBe(rgb("000000"));
    expect(await bg(cell(page, "r1c1"))).toBe(rgb("999999"));
    expect(await bg(cell(page, "r2c1"))).toBe(rgb("CBCBCB"));
    await page.locator(".flow table").screenshot({ path: "test-results/table-grid-5-dark.png" });
  });
});

// -- the design gallery -------------------------------------------------------------------------------------------------

const strip = (page: Page) => page.getByRole("toolbar", { name: "Table" });
const gallery = (page: Page) => page.getByRole("group", { name: "Table design", exact: true });
const tiles = (page: Page) => gallery(page).locator(".design-tile");
const tableLook = (page: Page) => page.locator(".flow table").first().evaluate((t) => ({ style: (t as HTMLElement).dataset.style, look: JSON.parse((t as HTMLElement).dataset.look ?? "null") }));

async function openGallery(page: Page) {
  // The caret goes to the paragraph above the table by the model: the table's strip, once it has been up, floats over that paragraph, as far across as its buttons
  // are wide (further in Firefox and Safari than in Chrome), where a click can't reach it.
  await inPage(page, (w) => {
    const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
    e.select({ p: 1, o: 0 });
    e.focus();
  });
  await cell(page, "r1c1").click();
  await strip(page).getByRole("button", { name: "Design" }).click();
  await expect(gallery(page)).toBeVisible();
}

test.describe("table design gallery", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { styles: TABLE_NORMAL + TABLE_GRID, sections: [{ paragraphs: ["Before the table", table({ rows: 5, cols: 3, tblPr: '<w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/>' }), "After the table"] }] });
  });

  test("offers Table Grid, five plain tables, and the grid and list tables in grey and six accents", async ({ page }) => {
    await openGallery(page);
    await expect(tiles(page)).toHaveCount(1 + 5 + 7 * 7 + 7 * 7);
    for (const name of ["Table Grid", "Plain Table 1", "Plain Table 5", "Grid Table 1 Light", "Grid Table 2", "Grid Table 3 Accent 4", "Grid Table 4 Accent 1", "Grid Table 5 Dark", "Grid Table 6 Colorful Accent 6", "Grid Table 7 Colorful", "List Table 1 Light Accent 2", "List Table 3", "List Table 7 Colorful Accent 6"]) {
      await expect(gallery(page).getByRole("menuitemradio", { name, exact: true })).toHaveCount(1);
    }
    // Each tile is a small drawing of the design; the one the table has is marked.
    await expect(tiles(page).first().locator("svg")).toBeVisible();
    await expect(gallery(page).getByRole("menuitemradio", { name: "Table Grid", exact: true })).toHaveAttribute("aria-checked", "true");
    await page.locator(".pop:has(.design-gallery)").screenshot({ path: "test-results/design-gallery.png" });
  });

  test("picking a design gives the table its look and writes the style into the document", async ({ page }) => {
    await openGallery(page);
    await gallery(page).getByRole("menuitemradio", { name: "Grid Table 4 Accent 1", exact: true }).click();
    await expect(gallery(page)).toHaveCount(0);
    const t = await tableLook(page);
    expect(t.style).toBe("GridTable4-Accent1");
    expect(t.look).toMatchObject({ firstRow: true, firstColumn: true, bandedRows: true, lastRow: false, lastColumn: false, bandedColumns: false });
    // The header row is drawn in the design's accent colour (the document's theme: the blank theme's first accent).
    const fill = await bg(cell(page, "r0c1"));
    expect(fill).not.toBe("rgba(0, 0, 0, 0)");
    expect(Number(await weight(cell(page, "r0c1")))).toBeGreaterThanOrEqual(600);
    expect(Number(await weight(cell(page, "r2c0")))).toBeGreaterThanOrEqual(600);
    // The document now has the style, once, based on the Normal Table style.
    const styles = await inPage(page, (w) => (w.fe.editor.engine as unknown as { styles(): { id: string; name: string; kind: string; basedOn?: string }[] }).styles());
    expect(styles.filter((s) => s.id === "GridTable4-Accent1")).toEqual([expect.objectContaining({ name: "Grid Table 4 Accent 1", kind: "table", basedOn: "TableNormal" })]);
    // One undo takes the design away.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await tableLook(page)).style).toBe("TableGrid");
    await page.keyboard.press("ControlOrMeta+Shift+z");
    expect((await tableLook(page)).style).toBe("GridTable4-Accent1");
  });

  test("the style options switch parts of the design and redraw the tiles", async ({ page }) => {
    await openGallery(page);
    await gallery(page).getByRole("menuitemradio", { name: "Grid Table 4 Accent 1", exact: true }).click();
    await openGallery(page);
    const header = gallery(page).getByRole("checkbox", { name: "Header row" });
    const total = gallery(page).getByRole("checkbox", { name: "Total row" });
    await expect(header).toBeChecked();
    await expect(total).not.toBeChecked();
    for (const name of ["Banded rows", "First column"]) await expect(gallery(page).getByRole("checkbox", { name })).toBeChecked();
    for (const name of ["Last column", "Banded columns"]) await expect(gallery(page).getByRole("checkbox", { name })).not.toBeChecked();
    const tileSvg = () => gallery(page).getByRole("menuitemradio", { name: "Grid Table 4 Accent 1", exact: true }).innerHTML();
    const before = await tileSvg();
    await total.check();
    expect((await tableLook(page)).look.lastRow).toBe(true);
    await expect(total).toBeChecked();
    // The gallery stays open and its tiles show the total row now.
    expect(await tileSvg()).not.toBe(before);
    await expect(cell(page, "r4c1")).toBeVisible();
    expect(await cell(page, "r4c1").evaluate((el) => getComputedStyle(el).borderTopStyle)).toBe("double");
    await header.uncheck();
    expect((await tableLook(page)).look.firstRow).toBe(false);
    expect(await bg(cell(page, "r0c1"))).not.toBe(rgb("4472C4"));
    await gallery(page).getByRole("checkbox", { name: "Banded columns" }).check();
    await gallery(page).getByRole("checkbox", { name: "Last column" }).check();
    expect((await tableLook(page)).look).toMatchObject({ lastColumn: true, bandedColumns: true, firstRow: false, lastRow: true });
    // Escape closes it.
    await page.keyboard.press("Escape");
    await expect(gallery(page)).toHaveCount(0);
  });

  test("the keyboard moves between tiles and Enter picks one", async ({ page }) => {
    await openGallery(page);
    const active = () => page.evaluate(() => (document.activeElement as HTMLElement).dataset.design);
    expect(await active()).toBe("TableGrid");
    await page.keyboard.press("ArrowRight");
    expect(await active()).toBe("PlainTable1");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Enter");
    await expect(gallery(page)).toHaveCount(0);
    expect((await tableLook(page)).style).toBe("PlainTable2");
  });

  test("Format, Table has Table design and the style options, and they follow the table", async ({ page }) => {
    await cell(page, "r1c1").click();
    await openMenu(page, "Format");
    await menuItem(page, "Table").hover();
    await expect(menuItem(page, "Table design…")).toBeVisible();
    for (const name of ["Header row", "Total row", "Banded rows", "First column", "Last column", "Banded columns"]) await expect(menuItem(page, name)).toBeVisible();
    // A table with no look of its own starts from Word's defaults for a new table.
    await expect(menuItem(page, "Header row")).toHaveAttribute("aria-checked", "true");
    await expect(menuItem(page, "Total row")).not.toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Table design…").click();
    await expect(gallery(page)).toBeVisible();
    await gallery(page).getByRole("menuitemradio", { name: "List Table 3", exact: true }).click();
    await cell(page, "r1c1").click();
    await openMenu(page, "Format");
    await menuItem(page, "Table").hover();
    await expect(menuItem(page, "Header row")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Total row").click();
    expect((await tableLook(page)).look.lastRow).toBe(true);
    // Not offered outside a table.
    // (The table's strip floats over the paragraph above the table, so a click on that paragraph can land on the strip: the keyboard goes to the start.)
    await page.keyboard.press("ControlOrMeta+Home");
    await openMenu(page, "Format");
    await menuItem(page, "Table").hover();
    await expect(menuItem(page, "Table design…")).toBeDisabled();
  });

  test("an extension can offer designs: they get a section, a drawing, and are written into the document when picked", async ({ page }) => {
    const xml = '<w:style w:type="table" w:customStyle="1" w:styleId="AcmeTable"><w:name w:val="Acme table"/><w:basedOn w:val="TableNormal"/><w:tblPr><w:tblBorders><w:bottom w:val="single" w:sz="8" w:space="0" w:color="C00000"/></w:tblBorders></w:tblPr><w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:color w:val="C00000"/></w:rPr><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="FCE4D6"/></w:tcPr></w:tblStylePr></w:style>';
    await inPage(
      page,
      (w, x) => {
        const host = w.fe.extensions as unknown as { add(e: unknown): Promise<void> };
        return host.add({
          id: "acme.tables",
          name: "Acme tables",
          activate: (ctx: { tableDesigns: { register(d: unknown): void } }) => ctx.tableDesigns.register({ id: "AcmeTable", label: "Acme table", xml: x, group: "Acme" }),
        });
      },
      xml,
    );
    await openGallery(page);
    await expect(gallery(page).locator(".menu-title", { hasText: "Acme" })).toBeVisible();
    const tile = gallery(page).getByRole("menuitemradio", { name: "Acme table", exact: true });
    await expect(tile).toHaveCount(1);
    await expect(tile.locator("svg rect[fill='#FCE4D6']")).toHaveCount(4);
    await tile.click();
    expect((await tableLook(page)).style).toBe("AcmeTable");
    expect(await bg(cell(page, "r0c1"))).toBe(rgb("FCE4D6"));
    const styles = await inPage(page, (w) => (w.fe.editor.engine as unknown as { styles(): { id: string }[] }).styles().map((s) => s.id));
    expect(styles).toContain("AcmeTable");
  });

  test("a design survives saving and opening again", async ({ page }) => {
    await openGallery(page);
    await gallery(page).getByRole("menuitemradio", { name: "List Table 3 Accent 5", exact: true }).click();
    const bytes = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));
    await inPage(page, (w, b) => w.fe.open(new Uint8Array(b), "again.docx"), bytes);
    await expect(cell(page, "r1c1")).toBeVisible();
    expect((await tableLook(page)).style).toBe("ListTable3-Accent5");
    expect(await bg(cell(page, "r0c1"))).not.toBe("rgba(0, 0, 0, 0)");
  });
});
