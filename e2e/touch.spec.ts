// Editing on a tablet: a tap puts the caret in the text and the keyboard types there, controls are big enough for a
// finger, a long press opens Docxtor's menu, and two fingers pinch-zoom the pages, not the browser's page.
import { expect, inPage, openFixture, test, type Page, CHROMIUM_ONLY, chromiumOnlyFile } from "./helpers";

chromiumOnlyFile(CHROMIUM_ONLY.cdp);

test.use({ hasTouch: true, viewport: { width: 1180, height: 820 } });

/** A gesture: touch events sent through one browser session, so they are one touch from start to end. */
async function gesture(page: Page) {
  const cdp = await page.context().newCDPSession(page);
  return (type: "touchStart" | "touchMove" | "touchEnd", points: { x: number; y: number }[]) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points.map((p, id) => ({ x: p.x, y: p.y, id })) });
}

test.describe("touch screens", () => {
  test("a tap puts the caret in the text, and controls are big enough for a finger", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const p = page.locator(".flow [data-p]").first();
    const box = (await p.boundingBox())!;
    await page.touchscreen.tap(box.x + box.width - 2, box.y + box.height / 2);
    await page.keyboard.type(" Tapped");
    await expect(p).toContainText("Tapped");
    const tb = (await page.locator(".toolbar .tb").first().boundingBox())!;
    expect(tb.height).toBeGreaterThanOrEqual(40);
    expect(await page.locator(".canvas").first().evaluate((el) => getComputedStyle(el).touchAction)).toBe("pan-x pan-y");
  });

  test("a long press opens Docxtor's menu", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const box = (await page.locator(".flow [data-p]").first().boundingBox())!;
    const at = { x: box.x + 20, y: box.y + box.height / 2 };
    const touch = await gesture(page);
    await touch("touchStart", [at]);
    await page.waitForTimeout(900);
    await touch("touchEnd", []);
    await expect(page.locator(".ctx-menu, .context-menu, [role=menu]").first()).toBeVisible();
  });

  test("two fingers pinch-zoom the pages", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const before = await inPage(page, (w) => w.fe.editor.zoom);
    const c = (await page.locator(".canvas").first().boundingBox())!;
    const mid = { x: c.x + c.width / 2, y: c.y + c.height / 2 };
    const touch = await gesture(page);
    await touch("touchStart", [{ x: mid.x - 40, y: mid.y }, { x: mid.x + 40, y: mid.y }]);
    for (let k = 1; k <= 5; k++) await touch("touchMove", [{ x: mid.x - 40 - k * 20, y: mid.y }, { x: mid.x + 40 + k * 20, y: mid.y }]);
    await touch("touchEnd", []);
    await expect.poll(() => inPage(page, (w) => w.fe.editor.zoom)).toBeGreaterThan(before + 0.2);
  });
});
