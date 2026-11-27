// The format painter: copies text and paragraph formatting; one click applies once, a double-click keeps it on until
// Escape or another click. Whole paragraphs get both kinds of formatting, part of a paragraph only the text's.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, paraText, test, type Page } from "./helpers";

const painter = (page: Page) => page.locator('.toolbar button[aria-label="Format painter"]');
const ids = (page: Page) => page.locator(".flow .fp").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
const fmt = (page: Page, p: number, o: number) => inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos), { p, o });

async function open(page: Page) {
  await openBuilt(page, {
    sections: [
      {
        paragraphs: [
          { text: "Source line", ppr: '<w:jc w:val="center"/><w:ind w:left="720"/><w:spacing w:before="120" w:after="360"/>' },
          "Target one is here",
          "Target two is here",
          "Fourth paragraph",
        ],
      },
    ],
  });
  const all = await ids(page);
  // The source is bold, red and 16 pt.
  await inPage(page, (w, p) => w.fe.editor.exec({ op: "format_text", from: { p, o: 0 }, to: { p, o: 11 }, format: { bold: true, color: "C00000", size: 16 } }), all[0]);
  return all;
}

/** Put the caret in the source paragraph (so the painter picks up its formatting). */
async function inSource(page: Page, p: number) {
  await inPage(page, (w, id) => {
    const ed = w.fe.editor as unknown as { select(s: unknown): void; focus(): void; emitSelection(): void };
    ed.focus();
    ed.select({ anchor: { p: id, o: 3 }, focus: { p: id, o: 3 } });
    ed.emitSelection();
  }, p);
}

/** Select text with the mouse: from the left edge of a paragraph (or a point inside it) to past its end. */
async function sweep(page: Page, p: number, opts: { fromChar?: number; toChar?: number } = {}) {
  const box = (await page.locator(`.flow [data-p="${p}"]`).boundingBox())!;
  const y = box.y + Math.min(10, box.height / 2);
  const x = await page.evaluate(
    ([id, from, to]) => {
      const el = document.querySelector(`.flow [data-p="${id}"]`)!;
      const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      const t = w.nextNode() as Text;
      const at = (o: number) => {
        const r = document.createRange();
        r.setStart(t, Math.min(o, t.length));
        r.setEnd(t, Math.min(o, t.length));
        const b = r.getBoundingClientRect();
        return b.left;
      };
      return { start: from === null ? el.getBoundingClientRect().left + 1 : at(from), end: to === null ? el.getBoundingClientRect().right - 2 : at(to) };
    },
    [p, opts.fromChar ?? null, opts.toChar ?? null] as const,
  );
  await page.mouse.move(x.start, y);
  await page.mouse.down();
  await page.mouse.move((x.start + x.end) / 2, y, { steps: 4 });
  await page.mouse.move(x.end, y, { steps: 4 });
  await page.mouse.up();
  // The painter applies a moment after the mouse goes up, once the browser has settled the selection.
  await page.waitForTimeout(150);
}

test.describe("format painter", () => {
  test("a click picks the formatting up and the next selection takes it, whole paragraphs with their paragraph formatting", async ({ page }) => {
    const [src, one, two] = await open(page);
    await inSource(page, src);
    await painter(page).click();
    await expect(painter(page)).toHaveClass(/\bon\b/);
    await expect(page.locator(".canvas")).toHaveClass(/painting/);
    await sweep(page, one);
    const f = (await fmt(page, one, 4))!;
    expect(f.run.bold).toBe(true);
    expect(f.run.color).toBe("C00000");
    expect(f.run.size).toBe(16);
    expect(f.para.align).toBe("center");
    expect(f.para.indLeft).toBe(720);
    expect(f.para.spaceAfter).toBe(360);
    // One use: the painter is off, and the next paragraph is left alone.
    await expect(painter(page)).not.toHaveClass(/\bon\b/);
    await expect(page.locator(".canvas")).not.toHaveClass(/painting/);
    await sweep(page, two);
    expect((await fmt(page, two, 3))!.run.bold).toBeFalsy();
    expect(await paraText(page, one)).toBe("Target one is here");
  });

  test("part of a paragraph gets the text formatting only", async ({ page }) => {
    const [src, one] = await open(page);
    await inSource(page, src);
    await painter(page).click();
    await sweep(page, one, { fromChar: 7, toChar: 10 });
    const inside = (await fmt(page, one, 8))!;
    expect(inside.run.bold).toBe(true);
    expect(inside.run.color).toBe("C00000");
    expect(inside.para.align).not.toBe("center");
    expect(inside.para.indLeft ?? 0).toBe(0);
    expect((await fmt(page, one, 2))!.run.bold).toBeFalsy();
  });

  test("a double-click keeps it on until Escape", async ({ page }) => {
    const [src, one, two, four] = await open(page);
    await inSource(page, src);
    await painter(page).dblclick();
    await expect(painter(page)).toHaveClass(/\bon\b/);
    await sweep(page, one);
    await expect(painter(page)).toHaveClass(/\bon\b/);
    await sweep(page, two);
    expect((await fmt(page, one, 3))!.run.bold).toBe(true);
    expect((await fmt(page, two, 3))!.run.bold).toBe(true);
    await page.keyboard.press("Escape");
    await expect(painter(page)).not.toHaveClass(/\bon\b/);
    await sweep(page, four);
    expect((await fmt(page, four, 3))!.run.bold).toBeFalsy();
  });

  test("a second click on the button switches it off, and so does a click while it is kept on", async ({ page }) => {
    const [src, one] = await open(page);
    await inSource(page, src);
    await painter(page).click();
    await painter(page).click();
    await expect(painter(page)).not.toHaveClass(/\bon\b/);
    await painter(page).dblclick();
    await expect(painter(page)).toHaveClass(/\bon\b/);
    await painter(page).click();
    await expect(painter(page)).not.toHaveClass(/\bon\b/);
    await sweep(page, one);
    expect((await fmt(page, one, 3))!.run.bold).toBeFalsy();
  });

  test("the pointer is a brush while it is on", async ({ page }) => {
    const [src, one] = await open(page);
    await inSource(page, src);
    await painter(page).click();
    const cursor = await page.locator(`.flow [data-p="${one}"]`).evaluate((el) => getComputedStyle(el).cursor);
    expect(cursor).toContain("url(");
    expect(cursor).toContain("svg");
    await page.keyboard.press("Escape");
    const after = await page.locator(`.flow [data-p="${one}"]`).evaluate((el) => getComputedStyle(el).cursor);
    expect(after).not.toContain("url(");
  });

  test("a paragraph style and a list come with the paragraph formatting", async ({ page }) => {
    const [src, one, two] = await open(page);
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_style", ids: [p], style: "Heading 2" }), src);
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_list_style", ids: [p], style: "disc" }), two);
    await inSource(page, src);
    await painter(page).click();
    await sweep(page, one);
    expect((await fmt(page, one, 2))!.style).toBe("Heading2");
    // Painting a paragraph that is in a list from one that is not takes it out of the list.
    await inSource(page, src);
    await painter(page).click();
    await sweep(page, two);
    expect((await fmt(page, two, 2))!.list).toBeFalsy();
  });

  test("painting is one undo step", async ({ page }) => {
    const [src, one] = await open(page);
    await inSource(page, src);
    await painter(page).click();
    await sweep(page, one);
    expect((await fmt(page, one, 3))!.run.bold).toBe(true);
    await page.keyboard.press("ControlOrMeta+Z");
    const f = (await fmt(page, one, 3))!;
    expect(f.run.bold).toBeFalsy();
    expect(f.para.align).not.toBe("center");
  });

  test("the Format menu has the painter, and Paste formatting uses the same copy", async ({ page }) => {
    const [src, one] = await open(page);
    await inSource(page, src);
    await page.keyboard.press("ControlOrMeta+Alt+C");
    await inPage(page, (w, id) => {
      const ed = w.fe.editor as unknown as { select(s: unknown): void; emitSelection(): void };
      ed.select({ anchor: { p: id, o: 0 }, focus: { p: id, o: 18 } });
      ed.emitSelection();
    }, one);
    await page.keyboard.press("ControlOrMeta+Alt+V");
    const f = (await fmt(page, one, 3))!;
    expect(f.run.bold).toBe(true);
    expect(f.para.align).toBe("center");
  });
});
