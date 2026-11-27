// Table details: cell margins, table shading, cell spacing, rows that start or end short, old-style merges, exact row
// heights, diagonal borders, autofit columns, floating tables with text around them, rows that split across pages (or
// move whole when they can't), and content controls around rows and cells.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, test, type Locator, type Page } from "./helpers";
import { table } from "./support/table-markup";

const cell = (page: Page, text: string): Locator => page.locator(".flow td").filter({ hasText: new RegExp(`^${text}$`) });
const num = (v: string) => parseFloat(v);
const open = (page: Page, ...blocks: ({ body: string } | string)[]) => openBuilt(page, { sections: [{ paragraphs: ["Before the table", ...blocks, "After the table"] }] });
const tableEl = (page: Page) => page.locator(".flow table.ft").first();
const lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. ";

test.describe("table details", () => {
  test("cell margins, table shading and cell spacing", async ({ page }) => {
    await open(
      page,
      table({
        rows: 2,
        cols: 2,
        tblPr: '<w:tblW w:w="4000" w:type="dxa"/><w:tblCellSpacing w:w="30" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="EEEEEE"/><w:tblLayout w:type="fixed"/><w:tblBorders><w:top w:val="single" w:sz="4" w:color="000000"/><w:left w:val="single" w:sz="4" w:color="000000"/><w:bottom w:val="single" w:sz="4" w:color="000000"/><w:right w:val="single" w:sz="4" w:color="000000"/></w:tblBorders>',
        tcPr: (r, c) => (r === 0 && c === 0 ? '<w:tcMar><w:top w:w="150" w:type="dxa"/><w:left w:w="300" w:type="dxa"/><w:bottom w:w="150" w:type="dxa"/><w:right w:w="45" w:type="dxa"/></w:tcMar>' : ""),
      }),
    );
    const t = tableEl(page);
    expect(await t.evaluate((el) => getComputedStyle(el).borderCollapse)).toBe("separate");
    expect(num(await t.evaluate((el) => getComputedStyle(el).borderSpacing.split(" ")[0]))).toBeCloseTo(2, 1);
    expect(await t.evaluate((el) => getComputedStyle(el).tableLayout)).toBe("fixed");
    // The whole table's shading is each cell's fill.
    expect(await cell(page, "r1c1").evaluate((el) => getComputedStyle(el).backgroundColor)).toBe("rgb(238, 238, 238)");
    // The cell with its own margins uses them (twips to px); the others use the table's.
    const pad = (c: Locator) => c.evaluate((el) => ({ t: parseFloat(getComputedStyle(el).paddingTop), r: parseFloat(getComputedStyle(el).paddingRight), b: parseFloat(getComputedStyle(el).paddingBottom), l: parseFloat(getComputedStyle(el).paddingLeft) }));
    expect(await pad(cell(page, "r0c0"))).toEqual({ t: 10, r: 3, b: 10, l: 20 });
    expect(await pad(cell(page, "r0c1"))).toEqual({ t: 0, r: 7.2, b: 0, l: 7.2 });
    await page.locator(".flow table").first().screenshot({ path: "test-results/table-spacing.png" });
  });

  test("a row that starts and ends short leaves its grid columns empty", async ({ page }) => {
    await open(page, {
      body: `<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:color="000000"/><w:insideH w:val="single" w:sz="4" w:color="000000"/><w:bottom w:val="single" w:sz="4" w:color="000000"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>a1</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>a2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>a3</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>a4</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:trPr><w:gridBefore w:val="1"/><w:wBefore w:w="1500" w:type="dxa"/><w:gridAfter w:val="1"/><w:wAfter w:w="1500" w:type="dxa"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>b2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>b3</w:t></w:r></w:p></w:tc></w:tr></w:tbl>`,
    });
    const gaps = page.locator(".flow table.ft td.gap");
    await expect(gaps).toHaveCount(2);
    // The second row's first cell sits under the first row's second one.
    const x = (c: Locator) => c.evaluate((el) => el.getBoundingClientRect().left);
    expect(Math.abs((await x(cell(page, "b2"))) - (await x(cell(page, "a2"))))).toBeLessThan(1);
    expect(Math.abs((await x(cell(page, "b3"))) - (await x(cell(page, "a3"))))).toBeLessThan(1);
    // The gaps draw nothing and can't be clicked into.
    expect(await gaps.first().evaluate((el) => ({ bg: getComputedStyle(el).backgroundColor, edit: (el as HTMLElement).contentEditable }))).toEqual({ bg: "rgba(0, 0, 0, 0)", edit: "false" });
    // Typing still goes into the right cell.
    await cell(page, "b2").click();
    await page.keyboard.type("x");
    await expect(page.locator(".flow td", { hasText: "b2x" }).or(page.locator(".flow td", { hasText: "xb2" }))).toHaveCount(1);
    await page.locator(".flow table").first().screenshot({ path: "test-results/table-short-rows.png" });
  });

  test("an old-style horizontal merge is one cell across", async ({ page }) => {
    await open(page, {
      body: `<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/><w:hMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>wide</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/><w:hMerge w:val="continue"/></w:tcPr><w:p/></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>end</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>c1</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>c2</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>c3</w:t></w:r></w:p></w:tc></w:tr></w:tbl>`,
    });
    const wide = cell(page, "wide");
    expect(await wide.evaluate((el) => (el as HTMLTableCellElement).colSpan)).toBe(2);
    expect(await tableEl(page).locator("tr").first().locator("td").count()).toBe(2);
    const w = await wide.evaluate((el) => el.getBoundingClientRect().width);
    const c1 = await cell(page, "c1").evaluate((el) => el.getBoundingClientRect().width);
    expect(Math.abs(w - 2 * c1)).toBeLessThan(2);
  });

  test("an exact row height clips what does not fit; at least keeps the height as a minimum", async ({ page }) => {
    await open(
      page,
      table({ rows: 2, cols: 1, colW: 3000, trPr: '<w:trHeight w:val="600" w:hRule="exact"/>', text: (r) => (r === 0 ? lorem.repeat(3) : "short") }),
      table({ rows: 1, cols: 1, colW: 3000, trPr: '<w:trHeight w:val="600" w:hRule="atLeast"/>', text: () => lorem.repeat(3) }),
    );
    const exact = page.locator(".flow table.ft").nth(0);
    const row = exact.locator("tr").first();
    expect(Math.round(await row.evaluate((el) => el.getBoundingClientRect().height))).toBe(40);
    const clip = row.locator(".clip");
    expect(await clip.evaluate((el) => getComputedStyle(el).overflow)).toBe("hidden");
    expect(await clip.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    expect(Math.round(await exact.locator("tr").nth(1).evaluate((el) => el.getBoundingClientRect().height))).toBe(40);
    const atLeast = page.locator(".flow table.ft").nth(1);
    expect(await atLeast.locator("tr").first().evaluate((el) => el.getBoundingClientRect().height)).toBeGreaterThan(80);
    expect(await atLeast.locator(".clip").count()).toBe(0);
  });

  test("diagonal borders are drawn in the cell", async ({ page }) => {
    await open(page, table({ rows: 1, cols: 2, tcPr: (_r, c) => (c === 0 ? '<w:tcBorders><w:tl2br w:val="single" w:sz="8" w:color="FF0000"/><w:tr2bl w:val="single" w:sz="4" w:color="0000FF"/></w:tcBorders>' : "") }));
    const image = await cell(page, "r0c0").evaluate((el) => getComputedStyle(el).backgroundImage);
    expect(image).toContain("linear-gradient");
    expect(image).toContain("rgb(255, 0, 0)");
    expect(image).toContain("rgb(0, 0, 255)");
    expect(await cell(page, "r0c1").evaluate((el) => getComputedStyle(el).backgroundImage)).toBe("none");
    await page.locator(".flow table").first().screenshot({ path: "test-results/table-diagonal.png" });
  });

  test("autofit columns grow to a long word; fixed columns keep the grid", async ({ page }) => {
    const longWord = "Supercalifragilisticexpialidocious";
    await open(
      page,
      table({ rows: 1, cols: 2, colW: 1000, tblPr: '<w:tblW w:w="0" w:type="auto"/>', text: (_r, c) => (c === 0 ? longWord : "ok") }),
      table({ rows: 1, cols: 2, colW: 1000, tblPr: '<w:tblW w:w="2000" w:type="dxa"/><w:tblLayout w:type="fixed"/>', text: (_r, c) => (c === 0 ? longWord : "ok") }),
    );
    const auto = page.locator(".flow table.ft").nth(0);
    const fixed = page.locator(".flow table.ft").nth(1);
    expect(await auto.evaluate((el) => getComputedStyle(el).tableLayout)).toBe("auto");
    expect(await fixed.evaluate((el) => getComputedStyle(el).tableLayout)).toBe("fixed");
    // 1000 twips is 67px: the long word does not fit, so the autofit column is wider and the fixed one is not.
    const w = (t: Locator) => t.locator("td").first().evaluate((el) => el.getBoundingClientRect().width);
    expect(await w(auto)).toBeGreaterThan(150);
    expect(Math.round(await w(fixed))).toBe(67);
  });
});

// -- floating tables ---------------------------------------------------------------------------------------------------

test.describe("floating tables", () => {
  const floating = (side: "right" | "left") =>
    table({
      rows: 3,
      cols: 2,
      colW: 1500,
      tblPr: `<w:tblpPr w:leftFromText="180" w:rightFromText="180" w:topFromText="0" w:bottomFromText="0" w:vertAnchor="text" w:horzAnchor="margin" w:tblpXSpec="${side}" w:tblpY="1"/><w:tblOverlap w:val="never"/><w:tblW w:w="3000" w:type="dxa"/><w:tblBorders><w:top w:val="single" w:sz="4" w:color="000000"/><w:left w:val="single" w:sz="4" w:color="000000"/><w:bottom w:val="single" w:sz="4" w:color="000000"/><w:right w:val="single" w:sz="4" w:color="000000"/><w:insideH w:val="single" w:sz="4" w:color="000000"/><w:insideV w:val="single" w:sz="4" w:color="000000"/></w:tblBorders>`,
    });

  test("text wraps around a table placed at the right margin", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before the table", floating("right"), `Wrapped. ${lorem.repeat(4)}`] }] });
    const wrap = page.locator(".flow .ftw.tfloat");
    await expect(wrap).toHaveCount(1);
    expect(await wrap.evaluate((el) => getComputedStyle(el).float)).toBe("right");
    // It keeps the distance from the text the file asks for (180 twips = 12px).
    expect(await wrap.evaluate((el) => getComputedStyle(el).marginLeft)).toBe("12px");
    const t = await wrap.evaluate((el) => el.getBoundingClientRect().toJSON());
    // The first lines of the paragraph after it are shorter than its last lines: they stop before the table.
    const lines = await page.locator(".flow .fp").filter({ hasText: "Wrapped." }).evaluate((el) => {
      const range = document.createRange();
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      const rights = new Map<number, number>();
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        range.selectNodeContents(n);
        for (const r of Array.from(range.getClientRects())) rights.set(Math.round(r.top), Math.max(rights.get(Math.round(r.top)) ?? 0, r.right));
      }
      return [...rights.entries()].sort((a, b) => a[0] - b[0]).map(([top, right]) => ({ top, right }));
    });
    const wrapped = lines.filter((l) => l.top < t.bottom - 4);
    const free = lines.filter((l) => l.top > t.bottom + 4);
    expect(wrapped.length).toBeGreaterThan(1);
    expect(free.length).toBeGreaterThan(0);
    for (const l of wrapped) expect(l.right).toBeLessThanOrEqual(t.left - 11);
    expect(Math.max(...free.map((l) => l.right))).toBeGreaterThan(t.left);
    await page.screenshot({ path: "test-results/table-floating.png" });
  });

  test("a table at the left margin floats left, and typing in it works", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before the table", floating("left"), `Wrapped. ${lorem.repeat(4)}`] }] });
    const wrap = page.locator(".flow .ftw.tfloat");
    expect(await wrap.evaluate((el) => getComputedStyle(el).float)).toBe("left");
    await cell(page, "r1c1").click();
    await page.keyboard.type("!");
    await expect(page.locator(".flow td", { hasText: "r1c1!" }).or(page.locator(".flow td", { hasText: "!r1c1" }))).toHaveCount(1);
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(1);
  });

  test("a floating table that does not fit goes to the next page with the paragraph it belongs to", async ({ page }) => {
    const filler = Array.from({ length: 22 }, (_, i) => `Filler ${i + 1}. ${lorem}`);
    await openBuilt(page, { sections: [{ paragraphs: [...filler, floating("right"), `Anchor paragraph. ${lorem.repeat(3)}`] }] });
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBeGreaterThanOrEqual(2);
    const where = await page.evaluate(() => {
      const ed = (window as unknown as { docxtor: { fe: { editor: { pageBox(k: number): { top: number; h: number; mB: number } } } } }).docxtor.fe.editor;
      const stage = document.querySelector(".stage")!.getBoundingClientRect();
      const t = document.querySelector(".flow .ftw.tfloat")!.getBoundingClientRect();
      const top = (t.top - stage.top) / ((ed as unknown as { zoom: number }).zoom || 1);
      return { top, bottom: top + t.height, page1: ed.pageBox(0).top + ed.pageBox(0).h - ed.pageBox(0).mB };
    });
    // Either it fits on the first page or it starts on the second, but it never hangs over the first page's bottom margin.
    expect(where.top >= where.page1 || where.bottom <= where.page1 + 1).toBe(true);
  });
});

// -- rows across pages --------------------------------------------------------------------------------------------------

/** The lowest bottom of any text line in the table and the bottom margin line of the page it is on. */
async function overhang(page: Page) {
  return page.evaluate(() => {
    const ed = (window as unknown as { docxtor: { fe: { editor: { zoom: number; pages: number; pageBox(k: number): { top: number; h: number; mB: number } } } } }).docxtor.fe.editor;
    const stage = document.querySelector(".stage")!.getBoundingClientRect();
    const range = document.createRange();
    let worst = -Infinity;
    for (const el of Array.from(document.querySelectorAll(".flow table.ft [data-p]"))) {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        if ((n.parentElement as HTMLElement).closest(".pgsp")) continue;
        range.selectNodeContents(n);
        for (const r of Array.from(range.getClientRects())) {
          if (r.width < 1) continue;
          const bottom = (r.bottom - stage.top) / ed.zoom;
          let k = 0;
          while (k < ed.pages - 1 && bottom > ed.pageBox(k + 1).top) k++;
          const b = ed.pageBox(k);
          worst = Math.max(worst, bottom - (b.top + b.h - b.mB));
        }
      }
    }
    return worst;
  });
}

test.describe("rows across pages", () => {
  test("a tall row splits at the page end, line by line", async ({ page }) => {
    await open(page, table({ rows: 1, cols: 1, colW: 9000, text: () => lorem.repeat(70) }));
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBeGreaterThanOrEqual(3);
    // The cell carries a gap at each page end and no line of text runs into a page's bottom margin.
    expect(await page.locator(".flow table.ft td .pgsp").count()).toBeGreaterThanOrEqual(2);
    expect(await overhang(page)).toBeLessThanOrEqual(1);
    // The cell's fill stops at the cut: the table is masked in the gap between the pages.
    expect(await tableEl(page).evaluate((el) => getComputedStyle(el).maskImage)).toContain("linear-gradient");
    // And the text that follows the table is after the whole row.
    const afterTop = await page.locator(".flow .fp").filter({ hasText: "After the table" }).evaluate((el) => el.getBoundingClientRect().top);
    const lastLine = await tableEl(page).evaluate((el) => el.getBoundingClientRect().bottom);
    expect(afterTop).toBeGreaterThanOrEqual(lastLine - 1);
    await page.screenshot({ path: "test-results/table-split-row.png", fullPage: false });
  });

  test("a row marked cantSplit moves to the next page whole", async ({ page }) => {
    const filler = Array.from({ length: 16 }, (_, i) => `Filler ${i + 1}. ${lorem}`);
    const rowText = (r: number) => (r === 1 ? lorem.repeat(5) : "head");
    await openBuilt(page, { sections: [{ paragraphs: [...filler, table({ rows: 2, cols: 1, colW: 9000, trPr: "<w:cantSplit/>", text: (r) => rowText(r) })] }] });
    const row = page.locator(".flow table.ft tr").filter({ hasText: "Lorem ipsum" }).first();
    // None of its lines is cut off by a page end: all of it is on one page.
    const spans = await row.evaluate((el) => {
      const ed = (window as unknown as { docxtor: { fe: { editor: { zoom: number; pages: number; pageBox(k: number): { top: number } } } } }).docxtor.fe.editor;
      const stage = document.querySelector(".stage")!.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      const page = (y: number) => {
        let k = 0;
        while (k < ed.pages - 1 && y > ed.pageBox(k + 1).top) k++;
        return k;
      };
      return { first: page((r.top - stage.top) / ed.zoom + 1), last: page((r.bottom - stage.top) / ed.zoom - 1), hasGap: !!el.querySelector(".pgsp") };
    });
    expect(spans.first).toBe(spans.last);
    expect(spans.hasGap).toBe(false);
    expect(await overhang(page)).toBeLessThanOrEqual(1);
  });

  test("a row that may split starts on a page only when room is left, and carries on at the top of the next", async ({ page }) => {
    const filler = Array.from({ length: 14 }, (_, i) => `Filler ${i + 1}. ${lorem}`);
    await openBuilt(page, { sections: [{ paragraphs: [...filler, table({ rows: 2, cols: 2, colW: 4500, text: (r, c) => (r === 1 ? `${lorem.repeat(c === 0 ? 8 : 3)}` : `h${c}`) })] }] });
    expect(await overhang(page)).toBeLessThanOrEqual(1);
    // Both cells of the row are cut at the same page end: their text carries on in step.
    const gaps = await page.locator(".flow table.ft tr").nth(1).locator(".pgsp").count();
    expect(gaps).toBeGreaterThanOrEqual(1);
  });
});

// -- content controls in tables ----------------------------------------------------------------------------------------

const SDT_TABLE = `<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2500"/><w:gridCol w:w="2500"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Qty</w:t></w:r></w:p></w:tc></w:tr>
<w:sdt><w:sdtPr><w:alias w:val="Items"/><w:id w:val="101"/></w:sdtPr><w:sdtContent>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Apples</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Pears</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc></w:tr>
</w:sdtContent></w:sdt>
<w:tr><w:sdt><w:sdtPr><w:alias w:val="Total label"/><w:id w:val="202"/></w:sdtPr><w:sdtContent><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Sum</w:t></w:r></w:p></w:tc></w:sdtContent></w:sdt><w:tc><w:tcPr><w:tcW w:w="2500" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc></w:tr></w:tbl>`;

test.describe("content controls in tables", () => {
  test("rows and cells inside content controls are shown and named", async ({ page }) => {
    await open(page, { body: SDT_TABLE });
    for (const text of ["Name", "Apples", "3", "Pears", "5", "Sum", "8"]) await expect(cell(page, text)).toBeVisible();
    await expect(tableEl(page).locator("tr")).toHaveCount(4);
    // The rows in the control carry its name; so does the cell, and the rows outside it do not.
    await expect(cell(page, "Apples")).toHaveAttribute("data-row-sdt", "Items");
    await expect(cell(page, "Pears").locator("xpath=..")).toHaveAttribute("data-sdt", "1");
    await expect(cell(page, "Sum")).toHaveAttribute("data-sdt-label", "Total label");
    await expect(cell(page, "Name")).not.toHaveAttribute("data-row-sdt", /.*/);
    await expect(cell(page, "8")).not.toHaveAttribute("data-sdt", /.*/);
    // The name shows above the row when the pointer is on it.
    await cell(page, "Apples").hover();
    expect(await cell(page, "Apples").evaluate((el) => getComputedStyle(el, "::before").content)).toBe('"Items"');
    expect(await cell(page, "Apples").evaluate((el) => getComputedStyle(el, "::before").opacity)).toBe("1");
    await page.locator(".flow table").first().screenshot({ path: "test-results/table-controls.png" });
  });

  test("they stay editable, and the controls are still there after saving and opening again", async ({ page }) => {
    await open(page, { body: SDT_TABLE });
    await cell(page, "Apples").click();
    await page.keyboard.press("End");
    await page.keyboard.type(" and plums");
    await cell(page, "Sum").click();
    await page.keyboard.press("End");
    await page.keyboard.type("!");
    await expect(page.locator(".flow td", { hasText: "Apples and plums" })).toBeVisible();
    await expect(page.locator(".flow td", { hasText: "Sum!" })).toBeVisible();
    const bytes = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));
    await inPage(page, (w, b) => w.fe.open(new Uint8Array(b), "again.docx"), bytes);
    await expect(page.locator(".flow td", { hasText: "Apples and plums" })).toHaveAttribute("data-row-sdt", "Items");
    await expect(page.locator(".flow td", { hasText: "Sum!" })).toHaveAttribute("data-sdt-label", "Total label");
    await expect(tableEl(page).locator("tr")).toHaveCount(4);
    // A new row under one in the control joins the control.
    await cell(page, "Pears").click();
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "table_edit", at: p, action: "row_below" }), await cell(page, "Pears").locator("[data-p]").first().evaluate((el) => Number((el as HTMLElement).dataset.p)));
    await expect(tableEl(page).locator("tr")).toHaveCount(5);
    await expect(tableEl(page).locator("tr").nth(3).locator("td").first()).toHaveAttribute("data-row-sdt", "Items");
  });
});
