import { chooseMenu, documentReady, expect, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** Replace window.print with a recorder of how many print pages exist at the moment it is called. */
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const calls: number[] = [];
    (window as unknown as { __printCalls: number[] }).__printCalls = calls;
    window.print = () => void calls.push(document.querySelectorAll(".docxtor-print .print-page").length);
  });
  await page.goto("/?open=rich.docx");
  await documentReady(page);
});

/** The @page rules in the document head (a <style> element's text isn't visible to text locators). */
const pageRules = (page: Page) => page.evaluate(() => Array.from(document.head.querySelectorAll("style"), (s) => s.textContent ?? "").filter((t) => t.includes("@page")));

const printCalls = (page: Page) => page.evaluate(() => (window as unknown as { __printCalls: number[] }).__printCalls);

test.describe("print", () => {
  test("Ctrl/Cmd+P prints both pages of rich.docx", { tag: "@core" }, async ({ page }) => {
    await page.keyboard.press(`${MOD}+P`);
    await expect.poll(() => printCalls(page)).toEqual([2]);
    // The pages sit in the page until the browser says it's done, sized as the paper is.
    await expect(page.locator(".docxtor-print .print-page")).toHaveCount(2);
    await expect(page.locator("html")).toHaveClass(/docxtor-printing/);
    expect(await pageRules(page)).toEqual([expect.stringContaining("size: 8.5in 11in")]);
    await expect(page.locator(".docxtor-print .print-page").first()).toContainText("Quarterly Product Report");
    await expect(page.locator(".docxtor-print .print-page").nth(1)).toContainText("Final paragraph on page two.");
  });

  test("File, Print… does the same", { tag: "@core" }, async ({ page }) => {
    await chooseMenu(page, "File", "Print…");
    await expect.poll(() => printCalls(page)).toEqual([2]);
  });

  test("the print copy is removed after afterprint", { tag: "@core" }, async ({ page }) => {
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print")).toHaveCount(1);

    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));

    await expect(page.locator(".docxtor-print")).toHaveCount(0);
    await expect(page.locator("html")).not.toHaveClass(/docxtor-printing/);
    expect(await pageRules(page)).toEqual([]);

    // And printing again works, with a fresh copy.
    await page.keyboard.press(`${MOD}+P`);
    await expect.poll(() => printCalls(page)).toEqual([2, 2]);
    await expect(page.locator(".docxtor-print")).toHaveCount(1);
  });

  test("a landscape page prints on landscape paper", async ({ page }) => {
    await chooseMenu(page, "Format", "Page orientation", "Landscape");
    await page.keyboard.press(`${MOD}+P`);
    await expect.poll(() => printCalls(page)).toHaveLength(1);
    expect(await pageRules(page)).toEqual([expect.stringContaining("size: 11in 8.5in")]);
  });
});
