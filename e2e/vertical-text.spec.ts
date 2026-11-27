// Section text direction (`w:textDirection`): a section of vertical text is one page's text area set with a vertical writing mode, its lines running
// down the page with each paragraph a column to the left of the one before it (`tbRl`) or up it (`btLr`).
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, test, type Page } from "./helpers";

const near = (a: number, b: number, tolerance = 2) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);
const wrapper = (page: Page) => page.locator(".flow > .cols[data-vt]");

test.describe("vertical text in a section", () => {
  test("tbRl: the wrapper is the text area, its text runs down and paragraphs go leftwards", async ({ page }) => {
    await openBuilt(page, { sections: [{ xml: { end: '<w:textDirection w:val="tbRl"/>' }, paragraphs: ["First vertical column", "Second vertical column"] }] });
    const w = wrapper(page);
    await expect(w).toHaveCount(1);
    expect(await w.evaluate((el) => getComputedStyle(el).writingMode)).toBe("vertical-rl");
    const zoom = await inPage(page, (x) => x.fe.editor.zoom);
    const sheet = (await page.locator(".sheet").first().boundingBox())!;
    const box = (await w.boundingBox())!;
    // The text area of Letter with one inch margins.
    near(box.width / zoom, 816 - 192);
    near(box.height / zoom, 1056 - 192);
    near((box.x - sheet.x) / zoom, 96);
    // The first paragraph is at the right edge; the second is to its left; both are as tall as the page's text.
    const a = (await page.locator(".flow .fp").nth(0).boundingBox())!;
    const b = (await page.locator(".flow .fp").nth(1).boundingBox())!;
    near(a.x + a.width, box.x + box.width, 2);
    expect(b.x + b.width).toBeLessThanOrEqual(a.x + 2);
    expect(a.height).toBeGreaterThan(a.width * 3);
    await page.screenshot({ path: "test-results/vertical-text.png" });
  });

  test("it starts a page of its own, and the section after it is horizontal again", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Horizontal before"] },
        { start: "continuous", xml: { end: '<w:textDirection w:val="tbRl"/>' }, paragraphs: ["Vertical"] },
        { start: "nextPage", paragraphs: ["Horizontal after"] },
      ],
    });
    await expect(page.locator(".sheet")).toHaveCount(3);
    const first = (await page.locator(".flow .fp").nth(0).boundingBox())!;
    const after = (await page.locator(".flow .fp").nth(2).boundingBox())!;
    expect(after.width).toBeGreaterThan(after.height * 3);
    expect(after.y).toBeGreaterThan(first.y);
    const mode = await page.locator(".flow .fp").nth(2).evaluate((el) => getComputedStyle(el).writingMode);
    expect(mode).toBe("horizontal-tb");
  });

  test("btLr reads from the bottom up", async ({ page }) => {
    await openBuilt(page, { sections: [{ xml: { end: '<w:textDirection w:val="btLr"/>' }, paragraphs: ["Reads upwards"] }] });
    const mode = await wrapper(page).evaluate((el) => getComputedStyle(el).writingMode);
    expect(["sideways-lr", "vertical-lr"]).toContain(mode);
  });

  test("changing it back through Page setup lays the section out as before", async ({ page }) => {
    await openBuilt(page, { sections: [{ xml: { end: '<w:textDirection w:val="tbRl"/>' }, paragraphs: ["Back and forth"] }] });
    await expect(wrapper(page)).toHaveCount(1);
    await inPage(page, (w) => {
      w.fe.editor.exec({ op: "set_section", text_direction: "lrTb" });
      w.fe.editor.reloadPageSetup();
    });
    await expect(wrapper(page)).toHaveCount(0);
    const fp = (await page.locator(".flow .fp").first().boundingBox())!;
    expect(fp.width).toBeGreaterThan(fp.height * 3);
  });
});
