// File, Print selection: only the selected text is printed, laid out on pages of its own, on the paper it was written for.
import { chooseMenu, documentReady, expect, inPage, menuItem, openMenu, RICH, selectWord, test, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

/** Replace window.print with a recorder of the pages that exist at the moment it is called. */
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { pages: number; text: string[] }[] = [];
    (window as unknown as { __printCalls: typeof calls }).__printCalls = calls;
    window.print = () =>
      void calls.push({
        pages: document.querySelectorAll(".docxtor-print .print-page").length,
        text: Array.from(document.querySelectorAll(".docxtor-print .print-page"), (p) => (p.textContent ?? "").replace(/\s+/g, " ").trim()),
      });
  });
  await page.goto("/?open=rich.docx");
  await documentReady(page);
});

const printCalls = (page: Page) => page.evaluate(() => (window as unknown as { __printCalls: { pages: number; text: string[] }[] }).__printCalls);

type At = { p: number; o: number };
const select = (page: Page, anchor: At, focus: At) =>
  page.evaluate(([a, f]) => (window as unknown as { docxtor: { fe: { editor: { select(s: unknown): void } } } }).docxtor.fe.editor.select({ anchor: a, focus: f }), [anchor, focus] as const);

test.describe("print selection", () => {
  test("it is off with a caret and on with a selection", async ({ page }) => {
    await openMenu(page, "File");
    await expect(menuItem(page, "Print selection")).toBeVisible();
    await expect(menuItem(page, "Print selection")).toBeDisabled();
    await page.keyboard.press("Escape");
    await selectWord(page, RICH.intro, "report");
    await openMenu(page, "File");
    await expect(menuItem(page, "Print selection")).toBeEnabled();
  });

  test("only the selected paragraphs are printed, as one page, and the rest of the document is not", async ({ page }) => {
    await select(page, { p: RICH.intro, o: 0 }, { p: RICH.highlights, o: 12 });
    await chooseMenu(page, "File", "Print selection");
    await expect.poll(async () => (await printCalls(page)).length).toBe(1);
    const [call] = await printCalls(page);
    expect(call.pages).toBe(1);
    // The introduction, and the start of the next paragraph, and nothing else (not the title, the table or page two).
    const intro = await inPage(page, (w, id) => w.engine!.textRange({ p: id, o: 0 }, { p: id, o: 999 }), RICH.intro);
    expect(call.text[0]).toContain(intro.slice(0, 40));
    expect(call.text[0]).not.toContain("Quarterly Product Report");
    expect(call.text[0]).not.toContain("Final paragraph on page two.");
    expect(call.text[0]).not.toMatch(/Metric/);
    // The open document is as it was.
    expect(await inPage(page, (w) => w.fe.editor.engine!.text().includes("Final paragraph on page two."))).toBe(true);
    await expect(page.locator(".flow .fp").first()).toBeVisible();
  });

  test("a selection inside one paragraph prints just those words", async ({ page }) => {
    await selectWord(page, RICH.intro, "summarises");
    await chooseMenu(page, "File", "Print selection");
    await expect.poll(async () => (await printCalls(page)).length).toBe(1);
    const [call] = await printCalls(page);
    // The header and footer print with it, as they do in Word.
    expect(call.text[0]).toContain("summarises");
    expect(call.text[0]).toContain("confidential draft");
    expect(call.text[0]).not.toContain("Quarterly");
    expect(call.text[0]).not.toContain("This report");
  });

  test("a long selection runs to several pages, and the copy is cleaned up after printing", async ({ page }) => {
    const order = await inPage(page, (w) => w.fe.editor.engine!.renderAll().order);
    await select(page, { p: order[0], o: 0 }, { p: order[order.length - 1], o: 5 });
    await chooseMenu(page, "File", "Print selection");
    await expect.poll(async () => (await printCalls(page)).length).toBe(1);
    const [call] = await printCalls(page);
    expect(call.pages).toBe(2);
    expect(call.text[0]).toContain("Quarterly Product Report");
    // While the browser's print dialog is open the sheets and the editor that laid them out are on the page; afterwards, gone.
    await expect(page.locator(".docxtor-print")).toHaveCount(1);
    await expect(page.locator(".docxtor-offscreen")).toHaveCount(1);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
    await expect(page.locator(".docxtor-print")).toHaveCount(0);
    await expect(page.locator(".docxtor-offscreen")).toHaveCount(0);
    await expect(page.locator("html")).not.toHaveClass(/docxtor-printing/);
  });

  test("the selection prints on the paper of its own section", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Portrait page text"] },
        { paragraphs: ["Landscape page text"], size: [15840, 12240], start: "nextPage" },
      ],
    });
    const order = await inPage(page, (w) => w.fe.editor.engine!.renderAll().order);
    // Select the words of the second section only.
    const last = order[order.length - 1];
    await select(page, { p: last, o: 0 }, { p: last, o: 9 });
    await chooseMenu(page, "File", "Print selection");
    await expect.poll(async () => (await printCalls(page)).length).toBe(1);
    const rules = await page.evaluate(() => Array.from(document.head.querySelectorAll("style"), (s) => s.textContent ?? "").filter((t) => t.includes("@page")));
    expect(rules[0]).toContain("size: 11in 8.5in");
    const [call] = await printCalls(page);
    expect(call.text[0]).toContain("Landscape");
    expect(call.text[0]).not.toContain("Portrait");
  });
});
