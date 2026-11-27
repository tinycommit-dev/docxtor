// The text of the views' interface (the Outline strip, the Navigation pane, the zoom control) is at least 4.5:1
// against what is behind it, in the light and the dark theme; the marks drawn on the page are at least 3:1.
import { contrasts, expect, test } from "./helpers";
import { chooseLayout, LONG_OUTLINE_DOC, openDoc } from "./support/view-helpers";

for (const scheme of ["light", "dark"] as const) {
  test.describe(`${scheme} theme`, () => {
    test.use({ colorScheme: scheme });

    test("the Outline strip and the zoom control are readable", async ({ page }) => {
      await openDoc(page, LONG_OUTLINE_DOC);
      await chooseLayout(page, "Outline");
      const found = await contrasts(page, [".outline-bar .ob-label", ".outline-bar .ob-level", ".outline-bar .ob-first span", ".outline-bar .ob-close", ".outline-bar .tb", ".zoom-readout", ".zoom-ctl .zoom-step"]);
      for (const f of found) expect(f.ratio, f.selector).toBeGreaterThanOrEqual(4.5);
    });

    test("the Navigation pane is readable on every tab", async ({ page }) => {
      await openDoc(page, LONG_OUTLINE_DOC);
      await page.locator(".sidebar").getByRole("tab", { name: "Navigation", exact: true }).click();
      const tab = (name: string) => page.locator(".nav-pane").getByRole("tab", { name, exact: true });
      let found = await contrasts(page, [".nav-tab:not(.on)", ".nav-tab.on", ".nav-label", ".nav-item[data-level='1'] .nav-label", ".nav-toggle:not(.none)"]);
      for (const f of found) expect(f.ratio, `headings ${f.selector}`).toBeGreaterThanOrEqual(4.5);
      await tab("Pages").click();
      found = await contrasts(page, [".nav-page .nav-page-num", ".nav-page.current .nav-page-num"]);
      for (const f of found) expect(f.ratio, `pages ${f.selector}`).toBeGreaterThanOrEqual(4.5);
      await tab("Results").click();
      await page.getByRole("searchbox", { name: "Search the document" }).fill("alpha");
      await expect(page.locator(".nav-result").first()).toBeVisible();
      found = await contrasts(page, [".nav-search", ".nav-case", ".nav-status", ".nav-result", ".nav-result mark", ".nav-result .r-before", ".nav-link"]);
      for (const f of found) expect(f.ratio, `results ${f.selector}`).toBeGreaterThanOrEqual(4.5);
      await page.getByRole("searchbox", { name: "Search the document" }).fill("zzzz");
      await expect(page.locator(".nav-status")).toHaveText("No results");
      found = await contrasts(page, [".nav-status"]);
      for (const f of found) expect(f.ratio, `no results ${f.selector}`).toBeGreaterThanOrEqual(4.5);
      await page.getByRole("searchbox", { name: "Search the document" }).fill("");
      await expect(page.locator(".nav-empty")).toBeVisible();
      found = await contrasts(page, [".nav-empty"]);
      for (const f of found) expect(f.ratio, `empty ${f.selector}`).toBeGreaterThanOrEqual(4.5);
    });

    test("the folding marks on the page are at least 3:1 against the page", async ({ page }) => {
      await openDoc(page, LONG_OUTLINE_DOC);
      await chooseLayout(page, "Outline");
      await expect(page.locator(".fm-btn").first()).toBeVisible();
      await expect(page.locator(".fm-dot").first()).toBeAttached();
      const [mark] = await contrasts(page, [".fm-btn"]);
      // The arrow is a line drawing, not text: its colour against the white page.
      expect(mark.ratio).toBeGreaterThanOrEqual(3);
      const dot = await page.evaluate(() => {
        const el = document.querySelector(".fm-dot");
        return el ? getComputedStyle(el, "::before").backgroundColor : "";
      });
      expect(dot).toMatch(/^rgb/);
      await page.screenshot({ path: `test-results/views-${scheme}.png` });
    });
  });
}
