// A right-to-left interface (Arabic, Hebrew, Persian, Urdu): the app turns round, the menu bar starts at the right and the
// navigator sits on the right, while the pages keep their own direction. `?lang=ar-XB` is a pseudo-language for checking
// it: every string shows backwards.
import { expect, openFixture, test } from "./helpers";

test.describe("right-to-left interface", () => {
  test("the app turns round and the pages don't", async ({ page }) => {
    await openFixture(page, "simple.docx", "&lang=ar-XB");
    expect(await page.evaluate(() => document.documentElement.dir)).toBe("rtl");
    // The menu bar's first menu is at the right, the last at the left.
    const items = page.getByRole("menubar").getByRole("menuitem");
    const first = (await items.first().boundingBox())!;
    const last = (await items.last().boundingBox())!;
    expect(first.x).toBeGreaterThan(last.x);
    // The navigator is to the right of the page area.
    const nav = (await page.locator(".workspace > .sidebar").first().boundingBox())!;
    const host = (await page.locator(".editor-host").first().boundingBox())!;
    expect(nav.x).toBeGreaterThan(host.x);
    // The page reads left to right, as the document says.
    expect(await page.locator(".canvas").first().evaluate((el) => getComputedStyle(el).direction)).toBe("ltr");
    expect(await page.locator(".flow .fp").first().evaluate((el) => getComputedStyle(el).textAlign)).not.toBe("right");
    // Dialogs and menus turn round too.
    expect(await page.evaluate(() => getComputedStyle(document.querySelector(".docxtor-layer") ?? document.body).direction)).toBe("rtl");
    await page.screenshot({ path: "test-results/rtl.png" });
  });

  test("copies of the pages read left to right too: the PDF copy, the thumbnails and the gallery previews", async ({ page }) => {
    await openFixture(page, "simple.docx", "&lang=ar-XB");
    // A copy of the stage put outside the canvas, as the PDF export and the galleries do, and a page thumbnail.
    const dirs = await page.evaluate(() => {
      const stage = document.querySelector<HTMLElement>(".canvas .stage")!;
      const root = document.createElement("div");
      root.className = "docxtor docxtor-pdf";
      root.append(stage.cloneNode(true));
      document.body.append(root);
      const snap = document.createElement("div");
      snap.className = "pg-snap";
      snap.append(document.querySelector(".canvas .sheet")!.cloneNode(true));
      document.querySelector(".docxtor-layer")!.append(snap);
      const out = { copy: getComputedStyle(root.querySelector(".flow .fp")!).direction, thumb: getComputedStyle(snap.querySelector(".fp") ?? snap).direction };
      root.remove();
      snap.remove();
      return out;
    });
    expect(dirs).toEqual({ copy: "ltr", thumb: "ltr" });
  });
});
