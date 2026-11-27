// The start screen's recent documents: every tile is the same size, whatever the document holds (nothing, a short line,
// or a first line with no spaces that can't wrap), and no tile runs over its neighbour.
import { expect, inPage, test } from "./helpers";

test("recent documents are tiles of one size that don't overlap", { tag: "@core" }, async ({ page }) => {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w) => w.createIn(w.files.library, "", "Long line.docx", "■".repeat(400)));
  await inPage(page, (w) => w.createIn(w.files.library, "", "Short.docx", "Hi"));
  await inPage(page, (w) => w.createIn(w.files.library, "", "Empty.docx", ""));
  await page.goto("/");
  const pages = page.locator(".start .doc-tile .page");
  await expect(pages).toHaveCount(4);
  const boxes = await pages.evaluateAll((els) => els.map((e) => e.getBoundingClientRect().toJSON() as DOMRect));
  for (const b of boxes) {
    expect(Math.round(b.width)).toBe(Math.round(boxes[0].width));
    expect(Math.round(b.height)).toBe(Math.round(boxes[0].height));
  }
  // Side by side without overlapping.
  const row = boxes.filter((b) => Math.abs(b.top - boxes[0].top) < 2).sort((a, b) => a.left - b.left);
  for (let i = 1; i < row.length; i++) expect(row[i].left).toBeGreaterThanOrEqual(row[i - 1].right);
  await page.screenshot({ path: "test-results/start-tiles.png" });
});
