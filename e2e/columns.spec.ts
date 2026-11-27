// Columns: a section in two, three or more columns lays out as newspaper columns on real pages. Text runs down one
// column, over to the next, and on to the next page; a section that starts part-way down a page starts its columns there;
// columns end level before a continuous section break; and the caret, typing and the model behave as in one column.
import { buildDocx, openBuilt, type DocSpec } from "./support/docx-builder";
import { caretAtEnd, CHROMIUM_ONLY, chooseMenu, dialog, expect, inPage, menuItem, openMenu, para, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. ";
/** `n` paragraphs, P1, P2, … each `len` repetitions of a sentence long. */
const paras = (n: number, len = 3, prefix = "P") => Array.from({ length: n }, (_, i) => `${prefix}${i + 1}. ${lorem.repeat(len)}`);

/** Letter paper with one-inch margins: text 624 wide from x 96, 864 high from y 96; the next page's text starts 220 lower. */
const TEXT_X = 96;
const TEXT_W = 624;
const TEXT_TOP = 96;
const TEXT_H = 864;
const NEXT = 1056 + 28; // distance from a page's top to the next page's top

const near = (a: number, b: number, tolerance = 1) => expect(Math.abs(a - b), `${a} is not within ${tolerance} of ${b}`).toBeLessThanOrEqual(tolerance);
const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup());
const pagesOf = (page: Page) => inPage(page, (w) => w.fe.editor.pages);
const wrappers = (page: Page) => page.locator(".flow > .cols");
const withText = (page: Page, text: string) => page.locator(".flow .fp").filter({ hasText: text }).first();

/** A visual line of text in flow coordinates (the stage's own, where page k starts at `pageBox(k).top`). */
interface Line {
  /** Left edge of the first character on the line, relative to the stage's text area (x 0 is the page's left edge). */
  x: number;
  top: number;
  bottom: number;
  page: number;
}

/** The lines of a paragraph (or of all body paragraphs, when no id is given) in reading order. */
async function lines(page: Page, id?: number | string): Promise<Line[]> {
  return page.evaluate((pid) => {
    const w = (window as unknown as { docxtor: { fe: { editor: { zoom: number; pages: number; pageBox(k: number): { top: number; h: number } } } } }).docxtor;
    const ed = w.fe.editor;
    const stage = document.querySelector(".stage")!.getBoundingClientRect();
    const els = pid === undefined ? Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")) : [document.querySelector<HTMLElement>(`.flow [data-p="${pid}"]`)!];
    const out: { x: number; top: number; bottom: number; page: number }[] = [];
    for (const el of els) {
      const range = document.createRange();
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      const found: { x: number; top: number; bottom: number }[] = [];
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        if ((n.parentElement as HTMLElement).closest(".lbl, .pgsp")) continue;
        range.selectNodeContents(n);
        for (const r of Array.from(range.getClientRects())) {
          if (r.width < 1 || r.height < 1) continue;
          const l = { x: (r.left - stage.left) / ed.zoom, top: (r.top - stage.top) / ed.zoom, bottom: (r.bottom - stage.top) / ed.zoom };
          const same = found.find((f) => Math.abs(f.top - l.top) < 3 && l.x >= f.x - 2 && l.x - f.x < 400 && Math.abs(f.x - l.x) < 400 && !(l.x > f.x + 330));
          if (same) {
            same.x = Math.min(same.x, l.x);
            same.bottom = Math.max(same.bottom, l.bottom);
          } else found.push(l);
        }
      }
      for (const f of found) {
        let pg = 0;
        for (let k = 0; k < ed.pages; k++) if (ed.pageBox(k).top <= f.top + 1) pg = k;
        out.push({ ...f, page: pg });
      }
    }
    return out;
  }, id);
}

/** Group lines into columns: same page, same left edge (within a few px), in reading order. */
const columnsOf = (ls: Line[]) => {
  const out: Line[][] = [];
  for (const l of ls) {
    const last = out[out.length - 1];
    if (last && last[0].page === l.page && Math.abs(last[0].x - l.x) < 6 && l.top >= last[last.length - 1].top - 1) last.push(l);
    else out.push([l]);
  }
  return out;
};

const caretOf = (page: Page) => inPage(page, (w) => w.fe.editor.sel);
const modelText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());

const filler = (n: number) => Array.from({ length: n }, (_, i) => `Filler ${i + 1}`);
const COL2 = TEXT_X + (TEXT_W - 48) / 2 + 48; // left edge of the second of two columns, half an inch apart
const COLW2 = (TEXT_W - 48) / 2;

test.describe("columns on the page", () => {
  test("Format > Columns turns a section into two, then three columns, side by side", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: paras(12, 2) }] });
    await expect(wrappers(page)).toHaveCount(0);
    await page.locator(".flow .fp").first().click();

    await chooseMenu(page, "Format", "Columns", "Two columns");
    await expect(wrappers(page)).toHaveCount(1);
    expect((await setup(page)).sections[0].cols.num).toBe(2);
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const stage = (await page.locator(".stage").boundingBox())!;
    // P1 stands at the left margin and is one column wide; a later paragraph stands in the second column, its top at the content top.
    const p1 = (await para(page, 1).boundingBox())!;
    near((p1.x - stage.x) / zoom, TEXT_X, 0.5);
    near(p1.width / zoom, COLW2, 0.5);
    near((p1.y - stage.y) / zoom, TEXT_TOP, 1);
    const ls = await lines(page);
    const second = ls.filter((l) => l.page === 0 && l.x > 300);
    expect(second.length).toBeGreaterThan(5);
    near(second[0].x, COL2, 0.5);
    near(second[0].top, TEXT_TOP, 1.5);
    // The menu shows what the caret's section has.
    await openMenu(page, "Format");
    await menuItem(page, "Columns").hover();
    await expect(menuItem(page, "Two columns")).toHaveAttribute("aria-checked", "true");
    await expect(menuItem(page, "One column")).not.toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");

    await chooseMenu(page, "Format", "Columns", "Three columns");
    expect((await setup(page)).sections[0].cols.num).toBe(3);
    const w3 = (TEXT_W - 96) / 3;
    const third = (await lines(page)).filter((l) => l.page === 0 && l.x > TEXT_X + 2 * (w3 + 48) - 4);
    near(third[0].x, TEXT_X + 2 * (w3 + 48), 0.5);
    near(third[0].top, TEXT_TOP, 1.5);
    near((await para(page, 1).boundingBox())!.width / zoom, w3, 0.5);

    await chooseMenu(page, "Format", "Columns", "One column");
    await expect(wrappers(page)).toHaveCount(0);
    expect(await page.locator(".flow > .fp").count()).toBe(12);
  });

  test("text flows from the first column to the second, then to the first column of the next page", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    // One paragraph that fills nearly three pages' worth of columns.
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: ["Start. " + lorem.repeat(60)] }] });
    const id = Number(await page.locator(".flow .fp").first().getAttribute("data-p"));
    const cols = columnsOf(await lines(page, id));
    // Column 1 and 2 of page 1, then of page 2, then the rest on page 3.
    expect(cols.length).toBeGreaterThanOrEqual(5);
    const [a, b, c, d, e] = cols;
    expect([a[0].page, b[0].page, c[0].page, d[0].page, e[0].page]).toEqual([0, 0, 1, 1, 2]);
    near(a[0].x, TEXT_X, 0.5);
    near(b[0].x, COL2, 0.5);
    near(c[0].x, TEXT_X, 0.5);
    near(d[0].x, COL2, 0.5);
    near(e[0].x, TEXT_X, 0.5);
    // Both columns start at the content top of their page, and fill it.
    near(a[0].top, TEXT_TOP, 1.5);
    near(b[0].top, TEXT_TOP, 1.5);
    near(c[0].top, NEXT + TEXT_TOP, 1.5);
    near(d[0].top, NEXT + TEXT_TOP, 1.5);
    near(e[0].top, 2 * NEXT + TEXT_TOP, 1.5);
    for (const col of [a, b, c, d]) {
      const last = col[col.length - 1];
      expect(last.bottom).toBeLessThanOrEqual(col[0].page * NEXT + TEXT_TOP + TEXT_H + 1);
      expect(last.bottom).toBeGreaterThan(col[0].page * NEXT + TEXT_TOP + TEXT_H - 40);
    }
    // The pages are the rows of columns: exactly the pages the text needs.
    expect(await pagesOf(page)).toBe(3);
    await expect(page.locator(".sheet")).toHaveCount(3);
    expect(await inPage(page, (w) => Object.values(w.fe.editor.pageMap()))).toEqual([1]);
  });
});

/** Top and bottom of each column of a page, from the lines of every paragraph in it (columns found by their left edge). */
const columnSpans = (ls: Line[], pg: number) => {
  const byX = new Map<number, Line[]>();
  for (const l of ls.filter((l) => l.page === pg)) {
    const key = Math.round(l.x / 10);
    byX.set(key, [...(byX.get(key) ?? []), l]);
  }
  return [...byX.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([, v]) => ({ x: v[0].x, top: Math.min(...v.map((l) => l.top)), bottom: Math.max(...v.map((l) => l.bottom)) }));
};

test.describe("a section that starts part-way down a page", () => {
  test("a full-width title, then two columns that start below it, in both columns, and carry on over the page", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Newsletter title", "A subtitle that runs for a while but fits on one line"] },
        { start: "continuous", cols: { num: 2 }, paragraphs: paras(24) },
      ],
    });
    expect(await pagesOf(page)).toBe(4);
    const title = await lines(page, 1);
    const sub = await lines(page, 2);
    const heading = sub[sub.length - 1].bottom;
    near(title[0].top, TEXT_TOP, 1.5);
    // The title spans the page; the columns below it start where it ends, both of them at the same height.
    const all = await lines(page);
    const first = all.filter((l) => l.page === 0 && l.top >= heading - 1);
    const c1 = first.filter((l) => l.x < 300);
    const c2 = first.filter((l) => l.x > 300);
    near(c1[0].x, TEXT_X, 0.5);
    near(c2[0].x, COL2, 0.5);
    near(c1[0].top, heading, 1.5);
    near(c2[0].top, heading, 1.5);
    // Nothing of the columns overlaps the title.
    expect(all.filter((l) => l.page === 0 && l.top < heading - 1).length).toBe(2);
    // Page 1's columns end at its bottom margin; page 2's start at its content top.
    const spans1 = columnSpans(all, 1);
    near(spans1[0].top, NEXT + TEXT_TOP, 1.5);
    near(spans1[1].top, NEXT + TEXT_TOP, 1.5);
    expect(spans1[0].bottom).toBeLessThanOrEqual(NEXT + TEXT_TOP + TEXT_H + 1);
    const spans0 = columnSpans(all, 0);
    for (const s of spans0.slice(0, 2)) expect(s.bottom).toBeLessThanOrEqual(TEXT_TOP + TEXT_H + 1);
    near(spans0[1].bottom, TEXT_TOP + TEXT_H, 18);
    // The wrapper stands pulled up over the title's line, which stays editable under it.
    await expect(wrappers(page)).toHaveAttribute("data-pull", "1");
  });

  test("the title stays clickable and editable under the pulled-up columns", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Newsletter title"] },
        { start: "continuous", cols: { num: 2 }, paragraphs: paras(24) },
      ],
    });
    await caretAtEnd(page, 1);
    await page.keyboard.type(" extra");
    await expect(withText(page, "Newsletter title")).toContainText("Newsletter title extra");
    expect((await caretOf(page))!.focus.p).toBe(1);
    // The title is still one line, and the columns have not moved.
    expect((await lines(page, 1)).length).toBe(1);
  });

  test("a section that fits below the title in one row has no pull and starts right under it", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Newsletter title"] },
        { start: "continuous", cols: { num: 2 }, paragraphs: paras(3, 1) },
      ],
    });
    expect(await pagesOf(page)).toBe(1);
    await expect(wrappers(page)).not.toHaveAttribute("data-pull", "1");
    const title = (await lines(page, 1))[0];
    const all = await lines(page);
    const body = all.filter((l) => l.top > title.bottom - 1);
    near(body[0].top, title.bottom, 1.5);
  });

  test("a table that runs over into the second column starts there below the title too", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Newsletter title"] }, { start: "continuous", cols: { num: 2 }, paragraphs: filler(50) }] });
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_table", after: 51, rows: 24, cols: 2 }));
    const title = (await lines(page, 1))[0];
    const tops = await page.locator(".flow .ftw td").evaluateAll((tds) => tds.map((td) => td.getBoundingClientRect()).map((r) => [r.left, r.top]));
    const stage = (await page.locator(".stage").boundingBox())!;
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const second = tops.filter(([x]) => (x - stage.x) / zoom > 300);
    expect(second.length).toBeGreaterThan(2);
    // The first cell in the second column is no higher than the text of the first column starts.
    near((Math.min(...second.map(([, y]) => y)) - stage.y) / zoom, title.bottom, 2);
    expect(await pagesOf(page)).toBe(1);
  });

  test("a continuous break after the columns makes them end level, and the next section carries on right below", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Newsletter title"] },
        { start: "continuous", cols: { num: 3 }, paragraphs: paras(5, 2) },
        { start: "continuous", paragraphs: ["After the columns, full width again."] },
      ],
    });
    expect(await pagesOf(page)).toBe(1);
    // The title is paragraph 1, the five in the columns 2 to 6, the closing one 7.
    const body = (await Promise.all([2, 3, 4, 5, 6].map((id) => lines(page, id)))).flat();
    const cols = columnSpans(body, 0).filter((s) => s.bottom - s.top > 60);
    expect(cols.length).toBe(3);
    // Level: the columns' bottoms are within two lines of each other (whole paragraphs and widows keep them from being exact).
    const bottoms = cols.map((s) => s.bottom);
    expect(Math.max(...bottoms) - Math.min(...bottoms)).toBeLessThanOrEqual(32);
    // Much shorter than a page's text height: the section is the height of its text, not of a page.
    expect(Math.max(...bottoms)).toBeLessThan(TEXT_TOP + 500);
    const after = (await lines(page, await withText(page, "After the columns").getAttribute("data-p").then(Number)))[0];
    near(after.x, TEXT_X, 0.5);
    expect(after.top).toBeGreaterThanOrEqual(Math.max(...bottoms) - 1);
    expect(after.top).toBeLessThanOrEqual(Math.max(...bottoms) + 4);
  });

  test("columns that run over a page and are followed by a continuous section end level on the last page", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Newsletter title"] },
        { start: "continuous", cols: { num: 2 }, paragraphs: paras(22) },
        { start: "continuous", paragraphs: ["Closing line after the columns."] },
      ],
    });
    // Paragraphs 2 to 23 are in the columns, 24 follows them.
    const body = (await Promise.all(Array.from({ length: 22 }, (_, i) => lines(page, i + 2)))).flat();
    const closing = (await lines(page, 24))[0];
    expect(closing.page).toBe(2);
    const last = columnSpans(body, 2).filter((s) => s.bottom - s.top > 20);
    expect(last.length).toBe(2);
    expect(Math.abs(last[0].bottom - last[1].bottom)).toBeLessThanOrEqual(32);
    near(closing.top, Math.max(...last.map((s) => s.bottom)), 4);
    // The first pages are full: their columns reach the bottom margin.
    const first = columnSpans(body, 1);
    near(first[0].bottom, NEXT + TEXT_TOP + TEXT_H, 18);
  });

  test("without a continuous break after it, the columns are filled one after another and the next section starts a new page", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { cols: { num: 2 }, paragraphs: paras(7, 2) },
        { start: "nextPage", paragraphs: ["A new section on its own page."] },
      ],
    });
    const all = await lines(page);
    const spans = columnSpans(all, 0);
    // The first column is filled to the bottom margin; the second holds the rest.
    near(spans[0].bottom, TEXT_TOP + TEXT_H, 18);
    expect(spans[1].bottom).toBeLessThan(spans[0].bottom - 60);
    expect(await pagesOf(page)).toBe(2);
    expect(await inPage(page, (w) => w.fe.editor.pageMap())).toMatchObject({ 8: 2 });
  });
});

test.describe("columns within a section's own margins", () => {
  test("a section with wider side margins puts its columns inside them, and the sections around it keep theirs", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Alpha paragraph"] },
        { start: "continuous", margins: [1440, 2880, 1440, 2880], cols: { num: 2 }, paragraphs: paras(8, 1) },
        { start: "continuous", paragraphs: ["Omega paragraph"] },
      ],
    });
    const at = async (id: number) => (await lines(page, id))[0];
    near((await at(1)).x, TEXT_X, 0.5);
    // 2 in margins on both sides leave 4.5 in, 288 px: two columns of (432 - 48) / 2 with half an inch between.
    const cols = (await lines(page)).filter((l) => l.top > 100 && l.top < 400);
    const xs = [...new Set(cols.map((l) => Math.round(l.x)))].filter((x) => x > TEXT_X + 20 || x < TEXT_X + 5);
    expect(xs).toEqual(expect.arrayContaining([192, 432]));
    expect(Math.max(...xs)).toBeLessThan(500);
    near((await at(10)).x, TEXT_X, 0.5);
    // The wrapper itself is as wide as the section's text.
    const w = (await wrappers(page).boundingBox())!;
    near(w.width / (await inPage(page, (x) => x.fe.editor.zoom)), 432, 0.5);
  });
});

test.describe("breaks inside columns", () => {
  test("a column break sends the rest to the next column, not the next page", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: ["Alpha paragraph", "Beta paragraph{CBR}", "Gamma paragraph", "Delta paragraph"] }] });
    expect(await pagesOf(page)).toBe(1);
    const at = async (text: string) => (await lines(page, await withText(page, text).getAttribute("data-p").then(Number)))[0];
    near((await at("Alpha")).x, TEXT_X, 0.5);
    near((await at("Beta")).x, TEXT_X, 0.5);
    const gamma = await at("Gamma");
    near(gamma.x, COL2, 0.5);
    near(gamma.top, TEXT_TOP, 1.5);
    near((await at("Delta")).x, COL2, 0.5);
    expect(gamma.page).toBe(0);
    // It is a block, not a page break: the paragraph with it has no page break marker of its own that the paginator reads.
    await expect(page.locator(".flow .atom.cbr")).toHaveCSS("display", "block");
  });

  test("a column break in the middle of a paragraph splits it over two columns", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: ["Left side{CBR}right side"] }] });
    const ls = await lines(page);
    expect(ls.length).toBe(2);
    near(ls[0].x, TEXT_X, 0.5);
    near(ls[1].x, COL2, 0.5);
    near(ls[0].top, ls[1].top, 1.5);
    expect(await pagesOf(page)).toBe(1);
  });

  test("in a one-column section a column break is still a page break", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Alpha paragraph", "Beta paragraph{CBR}", "Gamma paragraph"] }] });
    expect(await pagesOf(page)).toBe(2);
  });

  test("a page break before sends a paragraph to the next page, in its first column", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ cols: { num: 2 }, paragraphs: ["Alpha paragraph", "Beta paragraph", { text: "Gamma starts a page", ppr: "<w:pageBreakBefore/>" }, "Delta paragraph"] }],
    });
    expect(await pagesOf(page)).toBe(2);
    const gamma = (await lines(page, await withText(page, "Gamma").getAttribute("data-p").then(Number)))[0];
    expect(gamma.page).toBe(1);
    near(gamma.x, TEXT_X, 0.5);
    near(gamma.top, NEXT + TEXT_TOP, 1.5);
  });

  test("a page break (the character) in the middle of a column paragraph continues on the next page", async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: ["Before the break{PBR}after the break", "Next paragraph"] }] });
    expect(await pagesOf(page)).toBe(2);
    const ls = await lines(page);
    expect(ls.filter((l) => l.page === 1).length).toBe(2);
    near(ls.find((l) => l.page === 1)!.x, TEXT_X, 0.5);
  });

  test("keep lines together and keep with next hold at a column's end", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
        const long = lorem.repeat(2);
    const run = async (ppr: string, headPpr: string, fill: number) => {
      await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: [...filler(fill), { text: "Heading line", ppr: headPpr }, { text: "Long paragraph. " + long, ppr }] }] });
      const head = (await lines(page, await withText(page, "Heading line").getAttribute("data-p").then(Number)))[0];
      const longLines = await lines(page, await withText(page, "Long paragraph").getAttribute("data-p").then(Number));
      return { head, longLines };
    };
    // Without them: the long paragraph starts in the first column and goes over to the second.
    const free = await run("", "", 52);
    expect(free.longLines.some((l) => l.x < 300)).toBe(true);
    expect(free.longLines.some((l) => l.x > 300)).toBe(true);
    // Keep lines together: all of it moves to the second column.
    const kept = await run("<w:keepLines/>", "", 52);
    expect(kept.longLines.every((l) => l.x > 300)).toBe(true);
    // Keep with next: the heading that would be the last line of the first column goes with the paragraph.
    const lone = await run("", "", 54);
    expect(lone.head.x).toBeLessThan(300);
    const together = await run("", "<w:keepNext/>", 54);
    expect(together.head.x).toBeGreaterThan(300);
    near(together.head.top, TEXT_TOP, 1.5);
  });
});

/** Where a flow position (stage CSS px) is on the screen. */
const screenAt = async (page: Page, x: number, y: number) => {
  const stage = (await page.locator(".stage").boundingBox())!;
  const zoom = await inPage(page, (w) => w.fe.editor.zoom);
  return { x: stage.x + x * zoom, y: stage.y + y * zoom };
};
/** Paragraph ids in the model order, with the text of each. */
const bodyTexts = (page: Page) => modelText(page).then((t) => t.split("\n"));

test.describe("editing inside columns", () => {
  test("clicking puts the caret in the column that was clicked, and typing goes there", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(10, 2) }] });
    await page.evaluate(() => ((window as unknown as { __w: Element | null }).__w = document.querySelector(".flow > .cols")));
    const all = await lines(page);
    const ids = await page.locator(".flow [data-p]").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
    // The first paragraph that starts in the second column.
    let second = 0;
    for (const id of ids) if ((await lines(page, id))[0].x > 300) {
      second = id;
      break;
    }
    expect(second).toBeGreaterThan(1);
    expect(all.some((l) => l.x > 300)).toBe(true);
    const target = (await lines(page, second))[1];
    const at = await screenAt(page, target.x + 40, (target.top + target.bottom) / 2);
    await page.mouse.click(at.x, at.y);
    await expect.poll(async () => (await caretOf(page))?.focus.p).toBe(second);
    const before = (await modelText(page)).length;
    await page.keyboard.type("ZQX");
    await expect.poll(async () => (await modelText(page)).length).toBe(before + 3);
    expect(await withText(page, "ZQX").getAttribute("data-p")).toBe(String(second));
    // The wrapper stayed the same element: nothing was rebuilt around the typing.
    expect(await page.evaluate(() => (window as unknown as { __w: Element }).__w === document.querySelector(".flow > .cols"))).toBe(true);
    expect(await page.locator(".flow > .fp").count()).toBe(0);
    expect(await pagesOf(page)).toBe(1);
  });

  test("Enter splits a paragraph inside its column, and Backspace at the top of the second column joins it to the first", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: [...filler(56), "Second column start", "Another"] }] });
    const start = await withText(page, "Second column start");
    const startId = Number(await start.getAttribute("data-p"));
    const line = (await lines(page, startId))[0];
    near(line.x, COL2, 0.5);
    near(line.top, TEXT_TOP, 1.5);
    // Click at the start of it, then Backspace: it joins the last line of the first column.
    const at = await screenAt(page, line.x + 1, (line.top + line.bottom) / 2);
    await page.mouse.click(at.x, at.y);
    await page.keyboard.press("Home");
    await page.keyboard.press("Backspace");
    await expect.poll(async () => (await bodyTexts(page)).includes("Filler 56Second column start")).toBe(true);
    expect((await caretOf(page))!.focus.o).toBe("Filler 56".length);
    const joined = (await lines(page, 56))[0];
    near(joined.x, TEXT_X, 0.5);
    // Undo splits them again, and the text is back in the second column.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(async () => (await bodyTexts(page)).includes("Second column start")).toBe(true);
    near((await lines(page, startId))[0].x, COL2, 0.5);

    // Enter in the middle of a line of the second column splits there; the new paragraph is in the same wrapper.
    await page.keyboard.press("End");
    await page.keyboard.press("Enter");
    await page.keyboard.type("After the split");
    await expect.poll(async () => (await bodyTexts(page)).includes("After the split")).toBe(true);
    expect(await page.locator(".flow > .fp").count()).toBe(0);
    expect(await wrappers(page).locator("> [data-p]").count()).toBe(59);
    near((await lines(page, Number(await withText(page, "After the split").getAttribute("data-p"))))[0].x, COL2, 0.5);
  });

  test("arrow keys cross from one column to the next, and Shift selects across the boundary", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: [...filler(56), "Second column start", "Another"] }] });
    const last = Number(await withText(page, "Filler 56").getAttribute("data-p"));
    await caretAtEnd(page, last);
    await page.keyboard.press("ArrowDown");
    await expect.poll(async () => (await caretOf(page))?.focus.p).toBe(last + 1);
    await page.keyboard.press("ArrowUp");
    await expect.poll(async () => (await caretOf(page))?.focus.p).toBe(last);
    // Shift+Down from the end of the first column's last line to the second column: the selection holds both.
    await caretAtEnd(page, last);
    await page.keyboard.press("Shift+ArrowDown");
    await expect.poll(async () => (await caretOf(page))?.focus.p).toBe(last + 1);
    const sel = (await caretOf(page))!;
    expect(sel.anchor.p).toBe(last);
    expect(await page.evaluate(() => getSelection()!.toString())).toContain("\n");
    // Typing replaces it as one range.
    await page.keyboard.type("Q");
    await expect.poll(async () => (await bodyTexts(page)).some((t) => t.startsWith("Filler 56Q"))).toBe(true);
  });

  test("dragging from the first column into the second selects a paragraph that runs across them", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    // Tall enough to see the whole first page.
    await page.setViewportSize({ width: 1280, height: 1300 });
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: ["Start. " + lorem.repeat(40)] }] });
    const ls = (await lines(page, 1)).filter((l) => l.page === 0);
    const inFirst = ls.filter((l) => l.x < 300);
    const inSecond = ls.filter((l) => l.x > 300);
    expect(inFirst.length).toBeGreaterThan(40);
    expect(inSecond.length).toBeGreaterThan(5);
    const from = inFirst[inFirst.length - 6];
    const to = inSecond[4];
    const a = await screenAt(page, from.x + 20, (from.top + from.bottom) / 2);
    const b = await screenAt(page, to.x + 120, (to.top + to.bottom) / 2);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(b.x - 40, b.y, { steps: 6 });
    await page.mouse.move(b.x, b.y, { steps: 4 });
    await page.mouse.up();
    await expect.poll(async () => ((await caretOf(page))?.focus.o ?? 0) - ((await caretOf(page))?.anchor.o ?? 0)).toBeGreaterThan(200);
    const s = (await caretOf(page))!;
    expect(s.anchor.p).toBe(1);
    expect(s.focus.p).toBe(1);
    const before = (await modelText(page)).length;
    await page.keyboard.press("Delete");
    await expect.poll(async () => (await modelText(page)).length).toBe(before - (s.focus.o - s.anchor.o));
  });

  test("Select all spans every column, and typing replaces it all", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(10, 2) }] });
    await page.locator(".flow .fp").first().click({ position: { x: 30, y: 6 } });
    await page.keyboard.press(`${MOD}+A`);
    await expect.poll(async () => (await caretOf(page))?.focus.p).toBe(10);
    const sel = (await caretOf(page))!;
    expect(sel.anchor).toEqual({ p: 1, o: 0 });
    expect(sel.focus.o).toBeGreaterThan(400);
    await page.keyboard.type("Everything replaced");
    await expect.poll(async () => (await modelText(page)).trim()).toBe("Everything replaced");
    await expect(wrappers(page)).toHaveCount(1);
  });

  test("undo and redo of the columns themselves", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: paras(12, 2) }] });
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Format", "Columns", "Two columns");
    await expect(wrappers(page)).toHaveCount(1);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(wrappers(page)).toHaveCount(0);
    expect(await page.locator(".flow > .fp").count()).toBe(12);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect(wrappers(page)).toHaveCount(1);
    expect(await page.locator(".flow > .fp").count()).toBe(0);
    expect((await setup(page)).sections[0].cols.num).toBe(2);
  });

  test("a section break in a column section splits it into two wrappers, the first ending level", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(10, 2) }] });
    const target = await withText(page, "P5.");
    await target.click({ position: { x: 30, y: 6 } });
    await chooseMenu(page, "Insert", "Break", "Section break (continuous)");
    await expect(wrappers(page)).toHaveCount(2);
    const ps = await setup(page);
    expect(ps.sections.map((s) => s.cols.num)).toEqual([2, 2]);
    // The first section is followed by a continuous break: its columns end level, so the second starts part-way down.
    const first = columnSpans((await Promise.all([1, 2, 3, 4, 5].map((id) => lines(page, id)))).flat(), 0);
    expect(first.length).toBe(2);
    expect(Math.abs(first[0].bottom - first[1].bottom)).toBeLessThanOrEqual(32);
    // The break splits P5: what follows it is the first paragraph of the second section.
    const sixth = (await lines(page, Number(await page.locator(".flow [data-p]").nth(5).getAttribute("data-p"))))[0];
    expect(sixth.top).toBeGreaterThanOrEqual(Math.max(first[0].bottom, first[1].bottom) - 1);
    expect(sixth.top).toBeLessThanOrEqual(Math.max(first[0].bottom, first[1].bottom) + 4);
    expect(await pagesOf(page)).toBe(1);
    // Undoing joins them again into one wrapper.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(wrappers(page)).toHaveCount(1);
  });
});

test.describe("the Columns dialog and the line between columns", () => {
  test("More options opens with the section's values; Apply changes the columns, the gap and the line", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2, space: 360, sep: true }, paragraphs: paras(12, 2) }] });
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Format", "Columns", "More options…");
    const d = dialog(page, "Columns");
    await expect(d.getByLabel("Number of columns")).toHaveValue("2");
    await expect(d.getByLabel("Space between columns (inches)")).toHaveValue("0.25");
    await expect(d.getByLabel("Line between columns")).toBeChecked();
    // One section: no choice of where to apply it.
    await expect(d.getByRole("radiogroup", { name: "Apply to" })).toHaveCount(0);
    // The preview draws the columns it will make.
    await expect(d.locator(".cp-text")).toHaveClass(/cp-rule/);
    await d.getByLabel("Number of columns").fill("3");
    await expect(d.locator(".cp-text")).toHaveCSS("column-count", "3");
    await d.getByLabel("Space between columns (inches)").fill("0.3");
    await d.getByLabel("Line between columns").uncheck();
    await expect(d.locator(".cp-text")).not.toHaveClass(/cp-rule/);
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toHaveCount(0);
    const cols = (await setup(page)).sections[0].cols;
    expect(cols).toMatchObject({ num: 3, space: 432, sep: false });
    // Three columns 0.3 in apart: each (624 - 2 * 28.8) / 3 wide.
    const w3 = (TEXT_W - 2 * 28.8) / 3;
    const third = (await lines(page)).filter((l) => l.page === 0 && l.x > TEXT_X + 2 * w3);
    near(third[0].x, TEXT_X + 2 * (w3 + 28.8), 0.6);
    await expect(page.locator(".crule")).toHaveCount(0);

    // Cancel changes nothing.
    await chooseMenu(page, "Format", "Columns", "More options…");
    await dialog(page, "Columns").getByLabel("Number of columns").fill("5");
    await dialog(page, "Columns").getByRole("button", { name: "Cancel" }).click();
    expect((await setup(page)).sections[0].cols.num).toBe(3);
  });

  test("with several sections the dialog applies to the whole document or to this section", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Alpha one", "Alpha two"] }, { start: "nextPage", paragraphs: ["Beta one", "Beta two"] }] });
    await withText(page, "Beta one").click({ position: { x: 20, y: 6 } });
    await chooseMenu(page, "Format", "Columns", "More options…");
    const d = dialog(page, "Columns");
    const apply = d.getByRole("radiogroup", { name: "Apply to" });
    await expect(apply.getByRole("radio", { name: "Whole document" })).toHaveAttribute("aria-checked", "true");
    await d.getByLabel("Number of columns").fill("2");
    await apply.getByRole("radio", { name: "This section" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await setup(page)).sections.map((s) => s.cols.num)).toEqual([1, 2]);
    // And for the whole document (the default).
    await chooseMenu(page, "Format", "Columns", "More options…");
    await dialog(page, "Columns").getByLabel("Number of columns").fill("3");
    await dialog(page, "Columns").getByRole("button", { name: "Apply" }).click();
    expect((await setup(page)).sections.map((s) => s.cols.num)).toEqual([3, 3]);
  });

  test("the menu's One, Two and Three columns change the caret's section when there are several", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Alpha one"] }, { start: "nextPage", paragraphs: ["Beta one"] }] });
    await withText(page, "Alpha one").click({ position: { x: 20, y: 6 } });
    await chooseMenu(page, "Format", "Columns", "Three columns");
    expect((await setup(page)).sections.map((s) => s.cols.num)).toEqual([3, 1]);
    await withText(page, "Beta one").click({ position: { x: 20, y: 6 } });
    await chooseMenu(page, "Format", "Columns", "Two columns");
    expect((await setup(page)).sections.map((s) => s.cols.num)).toEqual([3, 2]);
    // The check mark follows the caret.
    await openMenu(page, "Format");
    await menuItem(page, "Columns").hover();
    await expect(menuItem(page, "Two columns")).toHaveAttribute("aria-checked", "true");
  });

  test("a line between columns is drawn on each page, beside the text it separates", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 3, sep: true }, paragraphs: paras(16, 3) }] });
    const pages = await pagesOf(page);
    expect(pages).toBeGreaterThanOrEqual(2);
    for (let k = 0; k < pages; k++) await expect(page.locator(".sheet").nth(k).locator(".crule")).toHaveCount(2);
    const rules = await page.locator(".sheet").first().locator(".crule").evaluateAll((els) => els.map((e) => ({ left: parseFloat((e as HTMLElement).style.left), top: parseFloat((e as HTMLElement).style.top), height: parseFloat((e as HTMLElement).style.height) })));
    // Between columns of (624 - 96) / 3 with a half inch between.
    const w3 = (TEXT_W - 96) / 3;
    near(rules[0].left, TEXT_X + w3 + 24, 0.5);
    near(rules[1].left, TEXT_X + 2 * w3 + 72, 0.5);
    near(rules[0].top, TEXT_TOP, 0.5);
    near(rules[0].height, TEXT_H, 0.5);
    // None without the setting.
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Format", "Columns", "More options…");
    await dialog(page, "Columns").getByLabel("Line between columns").uncheck();
    await dialog(page, "Columns").getByRole("button", { name: "Apply" }).click();
    await expect(page.locator(".crule")).toHaveCount(0);
  });

  test("below a title the line starts where the text does, and stops at the end of level columns", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Newsletter title"] },
        { start: "continuous", cols: { num: 2, sep: true }, paragraphs: paras(4, 2) },
        { start: "continuous", paragraphs: ["After the columns."] },
      ],
    });
    const rule = (await page.locator(".sheet").first().locator(".crule").evaluateAll((els) => els.map((e) => ({ top: parseFloat((e as HTMLElement).style.top), height: parseFloat((e as HTMLElement).style.height) }))))[0];
    const title = (await lines(page, 1))[0];
    const body = (await Promise.all([2, 3, 4, 5].map((id) => lines(page, id)))).flat();
    near(rule.top, title.bottom, 1.5);
    near(rule.top + rule.height, Math.max(...body.map((l) => l.bottom)), 2);
  });

  test("the horizontal ruler shades the gaps between the columns of the caret's section", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Alpha one"] }, { start: "nextPage", cols: { num: 3, space: 360 }, paragraphs: ["Beta one"] }] });
    await withText(page, "Alpha one").click({ position: { x: 20, y: 6 } });
    await expect(page.locator(".ruler-gap")).toHaveCount(0);
    await withText(page, "Beta one").click({ position: { x: 20, y: 6 } });
    await expect(page.locator(".ruler-gap")).toHaveCount(2);
    const gaps = await page.locator(".ruler-gap").evaluateAll((els) => els.map((e) => ({ left: parseFloat((e as HTMLElement).style.left), width: parseFloat((e as HTMLElement).style.width) })));
    const w3 = (TEXT_W - 2 * 24) / 3;
    near(gaps[0].left, TEXT_X + w3, 0.5);
    near(gaps[1].left, TEXT_X + 2 * w3 + 24, 0.5);
    near(gaps[0].width, 24, 0.5);
  });
});

test.describe("columns and the rest of the document", () => {
  test("line numbers count each column's lines in reading order, beside their own column", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(12, 2) }] });
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_line_numbers", on: true, count_by: 1, restart: "newPage" }));
    await inPage(page, (w) => w.fe.editor.reloadPageSetup());
    const nums = await page.locator(".sheet").first().locator(".lnum span").evaluateAll((els) => els.map((e) => ({ n: Number(e.textContent), right: parseFloat((e as HTMLElement).style.right), top: parseFloat((e as HTMLElement).style.top) })));
    const ls = (await lines(page)).filter((l) => l.page === 0);
    expect(nums.length).toBe(ls.length);
    // Reading order: 1, 2, 3 … down the first column, on down the second.
    expect(nums.map((x) => x.n)).toEqual(nums.map((_, i) => i + 1));
    const w = 816;
    const rightEdges = [...new Set(nums.map((x) => Math.round(w - x.right)))];
    expect(rightEdges.sort((a, b) => a - b)).toEqual([Math.round(TEXT_X - 24), Math.round(COL2 - 24)]);
    // Each number is level with its line.
    const first = ls[0];
    near(nums[0].top, first.top, 1.5);
  });

  test("a page with footnotes ends its columns above them, and the text carries on in the next column", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(30, 2) }] });
    const id = Number(await withText(page, "P2.").getAttribute("data-p"));
    const note = "A footnote long enough to need a few lines at the foot of the page. ".repeat(3);
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "insert_footnote", at: { p: a.id, o: 6 }, text: a.note }), { id, note });
    await expect(page.locator(".sheet").first().locator(".notes")).toBeVisible();
    const notesTop = await page.evaluate(() => {
      const ed = (window as unknown as { docxtor: { fe: { editor: { zoom: number } } } }).docxtor.fe.editor;
      const stage = document.querySelector(".stage")!.getBoundingClientRect();
      return (document.querySelector(".sheet .notes")!.getBoundingClientRect().top - stage.top) / ed.zoom;
    });
    const spans = columnSpans(await lines(page), 0);
    expect(spans).toHaveLength(2);
    for (const c of spans) {
      // Every column of the page stops above the notes, close to them (a line's height at most).
      expect(c.bottom).toBeLessThanOrEqual(notesTop + 0.5);
      expect(c.bottom).toBeGreaterThan(notesTop - 40);
    }
    near(spans[1].top, TEXT_TOP, 1.5);
    // The next page has no notes: its columns run to the bottom of the text area.
    const next = columnSpans(await lines(page), 1);
    expect(next[0].bottom).toBeGreaterThan(NEXT + TEXT_TOP + TEXT_H - 40);
    // Nothing went missing: the text is still all there, in order.
    expect(await inPage(page, (w) => w.fe.editor.engine!.text().includes("P30."))).toBe(true);
  });

  test("a table inside a column fits the column", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(3, 1) }] });
    await withText(page, "P2.").click({ position: { x: 30, y: 6 } });
    await chooseMenu(page, "Insert", "Table");
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    const t = (await page.locator(".flow .ftw").boundingBox())!;
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    expect(t.width / zoom).toBeLessThanOrEqual(COLW2 + 1);
    // And it can be typed in.
    await page.locator(".flow table.ft td").first().click();
    await page.keyboard.type("Cell text");
    await expect(page.locator(".flow table.ft td").first()).toContainText("Cell text");
  });

  test("the file saves its columns and reads them back", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 3, space: 432, sep: true }, paragraphs: paras(6, 1) }] });
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Format", "Columns", "Two columns");
    const xml = await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } } } } }).docxtor;
      const bytes = w.fe.editor.engine.save();
      // Find word/document.xml in the zip and inflate it.
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
    expect(xml).toMatch(/<w:cols [^>]*w:num="2"/);
    expect(xml).toMatch(/<w:cols [^>]*w:space="432"/);
    expect(xml).toMatch(/<w:cols [^>]*w:sep="1"/);
    // Reading the saved file again gives the same columns.
    const back = await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array; pageSetup(): { sections: { cols: unknown }[] } } }; open(b: Uint8Array, n?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(w.fe.editor.engine.save(), "again.docx");
      return w.fe.editor.engine.pageSetup().sections[0].cols;
    });
    expect(back).toMatchObject({ num: 2, space: 432, sep: true });
    await expect(wrappers(page)).toHaveCount(1);
  });

  test("a browser that cannot wrap rows of columns shows the section as one column", async ({ page }) => {
    await page.addInitScript(() => {
      const supports = CSS.supports.bind(CSS);
      CSS.supports = ((...a: string[]) => (a[0] === "column-wrap" ? false : supports(...(a as [string, string])))) as typeof CSS.supports;
    });
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(12, 2) }] });
    // The model has two columns; the page shows one.
    expect((await setup(page)).sections[0].cols.num).toBe(2);
    await expect(wrappers(page)).toHaveCount(0);
    expect(await page.locator(".flow > .fp").count()).toBe(12);
    near((await lines(page, 1))[0].x, TEXT_X, 0.5);
    await expect(page.locator(".ruler-gap")).toHaveCount(0);
    // Everything else still works: typing, and the pages.
    await withText(page, "P3.").click({ position: { x: 30, y: 6 } });
    await page.keyboard.type("hello");
    await expect(withText(page, "hello")).toBeVisible();
  });

  test("a long two-column document lays out quickly", async ({ page }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: paras(130, 3) }] });
    const pages = await pagesOf(page);
    expect(pages).toBeGreaterThanOrEqual(10);
    const ms = await page.evaluate(() => {
      const ed = (window as unknown as { docxtor: { fe: { editor: { layoutNow(): void } } } }).docxtor.fe.editor;
      const t0 = performance.now();
      for (let i = 0; i < 5; i++) ed.layoutNow();
      return (performance.now() - t0) / 5;
    });
    console.log(`layoutNow, ${pages} pages, two columns: ${ms.toFixed(1)} ms`);
    expect(ms).toBeLessThan(400);
    // Typing in the middle of it.
    await withText(page, "P60.").click({ position: { x: 30, y: 6 } });
    const typing = await page.evaluate(async () => {
      const t0 = performance.now();
      document.execCommand("insertText", false, "x");
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      return performance.now() - t0;
    });
    console.log(`one character typed in a ${pages}-page two-column document: ${typing.toFixed(1)} ms`);
    expect(typing).toBeLessThan(600);
  });
});

/** Every line of text sits inside a page's text area, and no two lines of a page cover the same place. */
async function expectConsistent(page: Page, what: string) {
  const ls = await lines(page);
  const boxes = await inPage(page, (w) => Array.from({ length: w.fe.editor.pages }, (_, k) => w.fe.editor.pageBox(k)));
  expect(ls.length, what).toBeGreaterThan(0);
  const problems: string[] = [];
  for (const l of ls) {
    const b = boxes[l.page];
    if (l.top < b.top + b.mT - 1.5) problems.push(`a line at ${l.top} starts above the text of page ${l.page}`);
    if (l.bottom > b.top + b.h - b.mB + 1.5) problems.push(`a line ends at ${l.bottom}, below the text of page ${l.page}`);
  }
  for (let i = 0; i < ls.length; i++) {
    for (let j = i + 1; j < ls.length; j++) {
      const a = ls[i];
      const b = ls[j];
      if (a.page !== b.page || Math.abs(a.x - b.x) >= 150) continue;
      if (Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 4) problems.push(`two lines overlap at x ${a.x}, y ${a.top} on page ${a.page}`);
    }
  }
  expect(problems.slice(0, 5), what).toEqual([]);
}

test.describe("the layout stays consistent", () => {
  const cases: [string, DocSpec][] = [
    ["a title, long columns, then one column that goes on over pages", { sections: [{ paragraphs: ["Title"] }, { start: "continuous", cols: { num: 2 }, paragraphs: paras(14) }, { start: "continuous", paragraphs: paras(18, 2, "Q") }] }],
    ["two, three and one columns, all continuous", { sections: [{ cols: { num: 2 }, paragraphs: paras(5, 2) }, { start: "continuous", cols: { num: 3 }, paragraphs: paras(6, 2, "R") }, { start: "continuous", paragraphs: paras(3, 2, "S") }] }],
    ["columns with a column break, a page break before and a page break character", { sections: [{ cols: { num: 2 }, paragraphs: [...paras(3, 2), "Break here{CBR}", ...paras(2, 2, "B"), { text: "Starts a page", ppr: "<w:pageBreakBefore/>" }, "Before a break{PBR}after a break", ...paras(2, 2, "C")] }] }],
    ["a next-page section after columns, and columns again", { sections: [{ cols: { num: 2 }, paragraphs: paras(9, 2) }, { start: "nextPage", paragraphs: ["Single"] }, { start: "nextPage", cols: { num: 2, sep: true }, paragraphs: paras(9, 2, "D") }] }],
    ["columns after a landscape page", { sections: [{ size: [15840, 12240], paragraphs: ["Landscape page"] }, { start: "nextPage", cols: { num: 3 }, paragraphs: paras(9, 2) }] }],
  ];
  for (const [what, spec] of cases) {
    test(what, async ({ page }) => {
      await page.setViewportSize({ width: 1280, height: 1000 });
      await openBuilt(page, spec);
      await expectConsistent(page, what);
      // Zoomed out the text is measured at another scale, and still sits in the pages' text areas.
      await inPage(page, (w) => w.fe.editor.setZoom(0.5));
      await expectConsistent(page, `${what}, zoomed out`);
    });
  }
});

test.describe("a full layout after a section's columns change, with footnotes", () => {
  // With footnotes in the text every full layout is checked against one made the long way. A
  // wrapper kept the columns of the last layout while the next one measured, so what follows it was measured at another place,
  // and the page map's records disagreed with a layout from scratch (the blocks themselves were in the right place).
  test("Insert index, and columns two, three and one, give the page map a layout from scratch gives", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.columns } }, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => void (m.type() === "error" && errors.push(m.text())));
    await openBuilt(page, { sections: [{ paragraphs: ["Title of the test", ...paras(4, 2), "A paragraph with a footnote", "Last paragraph"] }] });
    await inPage(page, (w) => {
      const ed = w.fe.editor;
      const p = Number(Array.from(ed.root.querySelectorAll<HTMLElement>("[data-p]")).find((e) => e.textContent!.startsWith("A paragraph with"))!.dataset.p);
      ed.exec({ op: "insert_footnote", at: { p, o: 11 }, text: "The note" });
    });
    const status = await page.evaluate(() => (window as any).docxtor.fe.extensions.execute("insert.index").then((r: any) => r.status, (e: any) => `threw: ${e.message}`));
    expect(status).toBe("done");
    await expect(wrappers(page)).toHaveCount(1);
    await page.locator(".flow .fp").first().click();
    for (const n of ["Two columns", "Three columns", "One column"]) await chooseMenu(page, "Format", "Columns", n);
    expect(errors).toEqual([]);
  });
});
