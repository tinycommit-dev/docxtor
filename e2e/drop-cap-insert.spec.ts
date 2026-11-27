// Insert, Drop cap: None, Dropped, In margin, and Options (the letter's font, lines to drop and distance from the text). The first
// letter moves into a frame of its own, as Word does it, and the text wraps beside it.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, LINE_START, test, type Page } from "./helpers";

const LONG = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident.";
const near = (a: number, b: number, tolerance = 2) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);

const texts = (page: Page) => page.locator(".flow .fp").allTextContents();
const frame = (page: Page) => inPage(page, (w) => (w.fe.editor.engine!.renderAll().blocks[0] as unknown as { fmt: { frame?: Record<string, unknown> } }).fmt.frame);

test.describe("drop caps", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { lang: "en-US", sections: [{ paragraphs: [LONG, "A second paragraph."] }] });
    await page.locator(".flow .fp").first().click();
  });

  test("Dropped moves the first letter into a frame and the text wraps beside it", async ({ page }) => {
    await chooseMenu(page, "Insert", "Drop cap", "Dropped");
    expect((await texts(page))[0]).toBe("L");
    expect((await texts(page))[1]).toMatch(/^orem ipsum/);
    const cap = page.locator(".flow .fp.dropcap");
    await expect(cap).toHaveCount(1);
    expect(await frame(page)).toMatchObject({ dropCap: "drop", lines: 3, wrap: "around", vAnchor: "text", hAnchor: "text" });
    // Three lines of text sit to the right of it; the fourth is back at the margin.
    const text = page.locator(".flow .fp").nth(1);
    const lefts = await text.evaluate((p) => {
      const r = document.createRange();
      const tops = new Map<number, number>();
      const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const t = n as Text;
        for (let i = 0; i < t.data.length; i++) {
          r.setStart(t, i);
          r.setEnd(t, i + 1);
          const b = r.getClientRects()[0];
          if (b && b.width) tops.set(Math.round(b.top), Math.min(tops.get(Math.round(b.top)) ?? 1e9, b.left));
        }
      }
      return [...tops.entries()].sort((a, b) => a[0] - b[0]).map(([, left]) => left);
    });
    const capBox = (await cap.boundingBox())!;
    const textBox = (await text.boundingBox())!;
    for (const l of lefts.slice(0, 3)) expect(l).toBeGreaterThanOrEqual(capBox.x + capBox.width - 1);
    near(lefts[3], textBox.x, 1);
    await page.screenshot({ path: "test-results/insert-drop-cap.png" });
  });

  test("In margin hangs the letter out to the left, and None puts the letter back", async ({ page }) => {
    await chooseMenu(page, "Insert", "Drop cap", "In margin");
    expect(await frame(page)).toMatchObject({ dropCap: "margin" });
    const cap = (await page.locator(".flow .fp.dropcap").boundingBox())!;
    const text = (await page.locator(".flow .fp").nth(1).boundingBox())!;
    expect(cap.x + cap.width).toBeLessThanOrEqual(text.x + 1);
    // The caret is in the text paragraph now: None finds the frame before it.
    await chooseMenu(page, "Insert", "Drop cap", "None");
    expect((await texts(page))[0]).toMatch(/^Lorem ipsum/);
    await expect(page.locator(".flow .fp.dropcap")).toHaveCount(0);
    expect(await frame(page)).toBeUndefined();
    // Undo brings the frame back in one step.
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(page.locator(".flow .fp.dropcap")).toHaveCount(1);
  });

  test("Options: more lines, a font and a distance from the text", async ({ page }) => {
    await chooseMenu(page, "Insert", "Drop cap", "Options…");
    const d = dialog(page, "Drop cap");
    await d.getByRole("radio", { name: "Dropped" }).click();
    await d.getByLabel("Font").fill("Georgia");
    await d.getByLabel("Lines to drop").fill("4");
    await d.getByRole("spinbutton", { name: /Distance from text/ }).fill("0.1");
    await d.getByRole("button", { name: "OK" }).click();
    const f = await frame(page);
    expect(f).toMatchObject({ dropCap: "drop", lines: 4, hSpace: 144 });
    const xml = await inPage(page, (w) => JSON.stringify(w.fe.editor.engine!.renderAll().blocks[0]));
    expect(xml).toContain("Georgia");
    // The dialog shows the drop cap the caret is in.
    await chooseMenu(page, "Insert", "Drop cap", "Options…");
    const again = dialog(page, "Drop cap");
    await expect(again.getByRole("radio", { name: "Dropped" })).toHaveAttribute("aria-checked", "true");
    await expect(again.getByLabel("Lines to drop")).toHaveValue("4");
    await again.getByRole("button", { name: "Cancel" }).click();
  });

  test("a paragraph with no letter to drop says so", async ({ page }) => {
    await page.locator(".flow .fp").nth(1).click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.press("Space");
    await page.keyboard.press("ArrowLeft");
    await chooseMenu(page, "Insert", "Drop cap", "Dropped");
    await expect(page.locator(".toast")).toContainText("letter");
    await expect(page.locator(".flow .fp.dropcap")).toHaveCount(0);
  });
});
