// Columns of unequal width: CSS columns are always equal, so the paginator lays these out itself. Each column has the
// width the file gives, blocks fill the columns of a page from left to right and then go on to the next page, a column
// break starts the next column, and the line between columns sits in the middle of the gap.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, test, type Locator, type Page } from "./helpers";

const near = (a: number, b: number, tolerance = 1.5) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);
const TW = 1 / 15;
const COLS = '<w:cols w:num="2" w:space="600" w:equalWidth="0"><w:col w:w="3000" w:space="600"/><w:col w:w="5760"/></w:cols>';
const lines = (n: number, prefix = "Line") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);

const box = async (page: Page, el: Locator, k = 0) => {
  const zoom = await inPage(page, (w) => w.fe.editor.zoom);
  const s = (await page.locator(".sheet").nth(k).boundingBox())!;
  const b = (await el.boundingBox())!;
  return { x: (b.x - s.x) / zoom, y: (b.y - s.y) / zoom, w: b.width / zoom, h: b.height / zoom };
};

test.describe("unequal columns", () => {
  test("each column has its own width and the text is set to it", async ({ page }) => {
    await openBuilt(page, { sections: [{ colsXml: COLS, paragraphs: ["Narrow column text{CBR}", "Wide column text that is set in the wider of the two columns"] }] });
    const fp = page.locator(".flow .fp");
    const narrow = await box(page, fp.nth(0));
    const wide = await box(page, fp.nth(1));
    near(narrow.w, 3000 * TW);
    near(wide.w, 5760 * TW);
    // The first column starts at the margin, the second after the width of the first and the space between.
    near(narrow.x, 96);
    near(wide.x, 96 + (3000 + 600) * TW);
    // Both start at the top of the page.
    near(narrow.y, 96, 2);
    near(wide.y, 96, 2);
    await page.screenshot({ path: "test-results/unequal-columns.png" });
  });

  test("blocks fill the first column and go on in the second", async ({ page }) => {
    await openBuilt(page, { sections: [{ colsXml: COLS, paragraphs: lines(120) }] });
    const fp = page.locator(".flow .fp");
    const first = await box(page, fp.nth(0));
    near(first.x, 96);
    // 120 lines are more than the two columns of a page hold, so there is a second page.
    await expect(page.locator(".sheet")).toHaveCount(2);
    // Some line is at the top of the second column.
    const tops = await fp.evaluateAll((els) => els.map((e) => ({ x: e.getBoundingClientRect().x, y: e.getBoundingClientRect().y })));
    const left = tops[0].x;
    const second = tops.find((t) => t.x > left + 100);
    expect(second).toBeTruthy();
    expect(Math.abs(second!.y - tops[0].y)).toBeLessThanOrEqual(2);
    // No line is lower than its page's text area (an inch margin at the bottom).
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const over = await fp.evaluateAll(
      (els, args) => {
        const sheets = Array.from(document.querySelectorAll(".sheet")).map((s) => s.getBoundingClientRect());
        let worst = -Infinity;
        for (const e of els) {
          const r = e.getBoundingClientRect();
          const s = sheets.find((b) => r.top >= b.top - 1 && r.top < b.bottom);
          if (s) worst = Math.max(worst, r.bottom - (s.bottom - args.margin * args.zoom));
        }
        return worst;
      },
      { margin: 96, zoom },
    );
    expect(over).toBeLessThanOrEqual(1);
  });

  test("a column break starts the next column", async ({ page }) => {
    await openBuilt(page, { sections: [{ colsXml: COLS, paragraphs: ["One", "Two{CBR}", "Three"] }] });
    const fp = page.locator(".flow .fp");
    const two = await box(page, fp.nth(1));
    const three = await box(page, fp.nth(2));
    near(two.x, 96);
    near(three.x, 96 + (3000 + 600) * TW);
    near(three.y, 96, 2);
  });

  test("the line between columns sits in the middle of the gap", async ({ page }) => {
    await openBuilt(page, { sections: [{ colsXml: COLS.replace("<w:cols ", '<w:cols w:sep="1" '), paragraphs: ["Left{CBR}", "Right"] }] });
    const rule = page.locator(".sheet .crule").first();
    await expect(rule).toHaveCount(1);
    const r = await box(page, rule);
    near(r.x, 96 + (3000 + 300) * TW, 1.5);
  });

  test("a section after the columns starts below them and is a single column", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { colsXml: COLS, paragraphs: ["Side note{CBR}", "Main text"] },
        { start: "continuous", paragraphs: ["Back to one column"] },
      ],
    });
    const fp = page.locator(".flow .fp");
    const main = await box(page, fp.nth(1));
    const after = await box(page, fp.nth(2));
    near(after.x, 96);
    expect(after.y).toBeGreaterThanOrEqual(main.y + main.h - 2);
    near(after.w, 12240 * TW - 192, 2);
  });

  test("editing keeps the columns: typing in a column re-flows it", async ({ page }) => {
    await openBuilt(page, { sections: [{ colsXml: COLS, paragraphs: ["Left column{CBR}", "Right column"] }] });
    const right = page.locator(".flow .fp").nth(1);
    await right.click();
    await page.keyboard.press("End");
    await page.keyboard.type(" and more words typed here");
    await expect(right).toContainText("and more words typed here");
    const b = await box(page, right);
    near(b.x, 96 + (3000 + 600) * TW);
    near(b.w, 5760 * TW);
  });
});
