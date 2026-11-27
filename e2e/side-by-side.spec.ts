// View, Window: View side by side shows another document beside the open one, editable and saved to its own file, and
// Synchronous scrolling moves both by the same amount; New window opens the document again, and a second window on the
// same document is read-only so the two can't overwrite each other.
import { chooseMenu, documentReady, expect, inPage, LINE_START, test, type Page } from "./helpers";

const many = (word: string) => Array.from({ length: 80 }, (_, i) => `${word} paragraph ${i + 1}, long enough to fill a line of the page.`).join("\n\n");

async function twoDocs(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Notes.docx", md), many("Notes"));
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Notes");
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Report.docx", md), many("Report"));
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Report");
  await documentReady(page);
}

test.describe("view side by side", () => {
  test("another document beside this one, edited and saved to its own file", async ({ page }) => {
    await twoDocs(page);
    await chooseMenu(page, "View", "Window", "View side by side…");
    await page.locator(".qp-item", { hasText: "Notes" }).click();
    const pane = page.locator(".sbs-pane");
    await expect(pane).toBeVisible();
    await expect(pane.locator(".flow")).toContainText("Notes paragraph 1,");
    await expect(page.locator(".workspace > .editor-host .flow").first()).toContainText("Report paragraph 1,");
    // Typing in the second document saves it to Notes.docx, not to the open one.
    await pane.locator(".flow [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Edited ");
    await expect(pane.locator(".sbs-status")).toHaveText("Saved", { timeout: 10000 });
    // Notes.docx in the Library has the edit.
    const notes = await inPage(page, async (w) => {
      const Engine = w.fe.editor.engine!.constructor as unknown as { open(b: Uint8Array, n: string): { renderAll(): { blocks: { segs?: { x?: string }[] }[] }; dispose(): void } };
      const e = Engine.open(await w.files.library.read("Notes.docx"), "Notes.docx");
      const text = e.renderAll().blocks.map((b) => (b.segs ?? []).map((s) => s.x ?? "").join("")).join("\n");
      e.dispose();
      return text;
    });
    expect(notes).toContain("Edited Notes paragraph 1");
    await expect(page.locator(".workspace > .editor-host .flow").first()).not.toContainText("Edited");
    await page.screenshot({ path: "test-results/side-by-side.png" });

    // Synchronous scrolling: both move by the same amount.
    const canvases = () => page.evaluate(() => [...document.querySelectorAll(".workspace .canvas")].map((c) => (c as HTMLElement).scrollTop));
    await page.evaluate(() => ((document.querySelector(".workspace > .editor-host .canvas") as HTMLElement).scrollTop += 400));
    await expect.poll(async () => (await canvases())[1]).toBeGreaterThan(300);
    await chooseMenu(page, "View", "Window", "Synchronous scrolling");
    const before = (await canvases())[1];
    await page.evaluate(() => ((document.querySelector(".workspace > .editor-host .canvas") as HTMLElement).scrollTop += 300));
    await page.waitForTimeout(200);
    expect((await canvases())[1]).toBe(before);

    await chooseMenu(page, "View", "Window", "View side by side…");
    await expect(pane).toHaveCount(0);
  });
});

test.describe("new window", () => {
  test("a second window on the same document is read-only", async ({ page, context }) => {
    await twoDocs(page);
    const url = page.url();
    const second = await context.newPage();
    await second.goto(url);
    await documentReady(second);
    await expect(second.locator(".toast")).toContainText("open in another window", { timeout: 5000 });
    expect(await second.evaluate(() => (window as unknown as { docxtor: { fe: { readOnly: boolean } } }).docxtor.fe.readOnly)).toBe(true);
    // The first window still edits it.
    expect(await page.evaluate(() => (window as unknown as { docxtor: { fe: { readOnly: boolean } } }).docxtor.fe.readOnly)).toBe(false);
    await second.close();
  });
});
