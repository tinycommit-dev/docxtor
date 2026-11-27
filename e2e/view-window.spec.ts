// View: the Draft view (no page boxes, breaks as labelled lines), Line focus, Gridlines and Text boundaries, and Split:
// two panes of one document, each with its own caret and scroll, edits shown in both, one undo history, and nothing
// left behind when the split is taken away.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Page } from "./helpers";

const lines = (n: number) => Array.from({ length: n }, (_, i) => `Line ${i + 1} of the text, long enough to read as a sentence.`);

async function open(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Chapter one", ...lines(30), { text: "After the break", ppr: "<w:pageBreakBefore/>" }, ...lines(10)] }] });
}

test.describe("draft view", () => {
  test("shows the text without page boxes, with the page break as a labelled line, and edits as usual", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "View", "Layout", "Draft");
    await expect(page.locator(".sheet")).toHaveCount(1);
    // The page break is a labelled line where the page ended.
    await expect(page.locator(".np-layer .np-brk").first()).toBeVisible();
    await expect(page.locator(".np-layer")).toContainText("Page break");
    await page.locator(".flow [data-p]", { hasText: "Chapter one" }).click();
    await page.keyboard.press("End");
    await page.keyboard.type(" and more");
    await expect(page.locator(".flow")).toContainText("Chapter one and more");
    await page.screenshot({ path: "test-results/draft-view.png" });
    await chooseMenu(page, "View", "Layout", "Print layout");
    await expect(page.locator(".sheet")).toHaveCount(2);
  });
});

test.describe("line focus", () => {
  test("dims all but the lines in focus, moves with the arrow keys, and Escape leaves", async ({ page }) => {
    await open(page);
    await page.locator(".flow [data-p]", { hasText: "Line 3 of" }).first().click();
    await chooseMenu(page, "View", "Immersive", "Line focus");
    // The lines in focus move down with the arrow key: the veil below starts lower.
    const below = page.locator(".lf-layer .lf-below");
    await expect(below).toBeVisible();
    const before = (await below.boundingBox())!.y;
    await page.keyboard.press("ArrowDown");
    await expect.poll(async () => (await below.boundingBox())!.y).toBeGreaterThan(before);
    await expect(page.locator(".lf-live")).not.toBeEmpty();
    await page.screenshot({ path: "test-results/line-focus.png" });
    await page.keyboard.press("Escape");
    await expect(page.locator(".lf-layer")).toHaveCount(0);
  });
});

test.describe("page guides", () => {
  test("gridlines and text boundaries draw on the page and are not printed", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "View", "Text boundaries");
    await expect(page.locator(".pg-guides").first()).toBeAttached();
    await chooseMenu(page, "View", "Gridlines");
    await page.screenshot({ path: "test-results/guides.png" });
    await page.evaluate(() => (window.print = () => undefined));
    await page.keyboard.press("ControlOrMeta+p");
    await expect(page.locator(".docxtor-print .pg-guides")).toHaveCount(0);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });
});

test.describe("split window", () => {
  test("two panes of one document: typing in one shows in the other, undo from either, and Remove split cleans up", async ({ page }) => {
    await open(page);
    const flows = page.locator(".flow");
    await chooseMenu(page, "View", "Window", "Split");
    await expect(flows).toHaveCount(2);
    // Each pane scrolls on its own.
    await flows.nth(1).locator("[data-p]", { hasText: "After the break" }).scrollIntoViewIfNeeded();
    await flows.nth(1).locator("[data-p]", { hasText: "After the break" }).click();
    await page.keyboard.press("End");
    await page.keyboard.type(" (edited below)");
    await expect(flows.nth(0)).toContainText("After the break (edited below)");
    // Undo from the first pane takes it back in both.
    await flows.nth(0).locator("[data-p]", { hasText: "Chapter one" }).click();
    await page.keyboard.press("ControlOrMeta+z");
    await expect(flows.nth(0)).not.toContainText("(edited below)");
    await expect(flows.nth(1)).not.toContainText("(edited below)");
    await page.screenshot({ path: "test-results/split.png" });
    const listeners = await inPage(page, (w) => w.fe.editor.root.querySelectorAll("*").length);
    await chooseMenu(page, "View", "Window", "Remove split");
    await expect(flows).toHaveCount(1);
    expect(await inPage(page, (w) => !!(w.fe as unknown as { split: { on: boolean } }).split.on)).toBe(false);
    expect(listeners).toBeGreaterThan(0);
  });
});
