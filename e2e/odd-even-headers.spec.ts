// Different odd and even pages, switched on and off from the header bar and from Insert, Page elements: the setting is the document's,
// and turning it on while editing an even page's header makes the even header.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, openMenu, menuItem, test, type Page } from "./helpers";

const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
/** Double-click in the header area at the top of a sheet (the header itself takes no clicks), which starts editing it. */
const editHeader = async (page: Page, k: number) => {
  await page.evaluate((i) => {
    const c = document.querySelector(".canvas")!;
    c.scrollTop += document.querySelectorAll(".sheet")[i].getBoundingClientRect().top - c.getBoundingClientRect().top - 60;
  }, k);
  const zoom = await inPage(page, (w) => w.fe.editor.zoom);
  const b = (await sheet(page, k).boundingBox())!;
  await page.mouse.dblclick(b.x + b.width / 2, b.y + 20 * zoom);
};
const evenOdd = (page: Page) => inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as unknown as { evenAndOddHeaders: boolean }).evenAndOddHeaders);

test.describe("different odd and even pages", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ headers: { default: "Odd header" }, paragraphs: ["Page one{PBR}", "Page two{PBR}", "Page three"] }] });
    await expect(page.locator(".sheet")).toHaveCount(3);
  });

  test("from the menu: even pages get a header of their own, and it can be switched off again", async ({ page }) => {
    await expect(sheet(page, 1).locator(".hdr")).toHaveText("Odd header");
    await chooseMenu(page, "Insert", "Page elements", "Different odd and even pages");
    expect(await evenOdd(page)).toBe(true);
    // Even pages have no header of their own yet, so they show none; odd pages keep theirs.
    await expect(sheet(page, 0).locator(".hdr")).toHaveText("Odd header");
    await expect(sheet(page, 2).locator(".hdr")).toHaveText("Odd header");
    await expect(sheet(page, 1).locator(".hdr")).toHaveCount(0);
    await openMenu(page, "Insert");
    await menuItem(page, "Page elements").click();
    await expect(menuItem(page, "Different odd and even pages")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Different odd and even pages").click();
    expect(await evenOdd(page)).toBe(false);
    await expect(sheet(page, 1).locator(".hdr")).toHaveText("Odd header");
  });

  test("from the header bar while editing an even page's header", async ({ page }) => {
    await editHeader(page, 1);
    const strip = page.locator(".story-strip");
    await expect(strip).toBeVisible();
    await strip.getByRole("button", { name: "Different odd and even" }).click();
    expect(await evenOdd(page)).toBe(true);
    // The even header was made and is being edited.
    await expect(strip.locator(".story-where")).toHaveText("Even page header on page 2");
    await page.keyboard.type("Even header");
    await page.keyboard.press("Escape");
    await expect(sheet(page, 1).locator(".hdr")).toHaveText("Even header");
    await expect(sheet(page, 0).locator(".hdr")).toHaveText("Odd header");
    await expect(sheet(page, 2).locator(".hdr")).toHaveText("Odd header");
    const part = await inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as unknown as { sections: { headers: Record<string, string> }[] }).sections[0].headers);
    expect(Object.keys(part).sort()).toEqual(["default", "even"]);
    // Off again from the bar.
    await editHeader(page, 1);
    await page.locator(".story-strip").getByRole("button", { name: "Different odd and even" }).click();
    expect(await evenOdd(page)).toBe(false);
  });
});
