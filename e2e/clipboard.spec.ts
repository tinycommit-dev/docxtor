import { CHROMIUM_ONLY, caretAtEnd, chooseMenu, expect, inPage, openFixture, paraText, RICH, runAt, selectWord, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const LAST = "Final paragraph on page two.";

const writeClipboard = (page: Page, text: string) => page.evaluate((value: string) => navigator.clipboard.writeText(value), text);
const readClipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());

test.describe("clipboard", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("Edit, Paste from Markdown inserts bold and italic text", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await writeClipboard(page, "**bold** and *italic*");
    await caretAtEnd(page, RICH.last);
    await chooseMenu(page, "Edit", "Paste from Markdown");

    // The markers are gone; the words stay.
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST}bold and italic`);
    const at = LAST.length;
    expect((await runAt(page, RICH.last, at + 2))?.bold).toBe(true);
    expect((await runAt(page, RICH.last, at + 2))?.italic).toBeFalsy();
    expect((await runAt(page, RICH.last, at + 6))?.bold).toBeFalsy();
    expect((await runAt(page, RICH.last, at + 12))?.italic).toBe(true);
    expect((await runAt(page, RICH.last, at + 12))?.bold).toBeFalsy();
    // The text before the paste is untouched.
    expect((await runAt(page, RICH.last, 3))?.bold).toBeFalsy();
  });

  test("Ctrl/Cmd+V pastes plain text at the caret", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await writeClipboard(page, " pasted words");
    await caretAtEnd(page, RICH.last);
    await page.keyboard.press(`${MOD}+V`);
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST} pasted words`);
    expect((await runAt(page, RICH.last, LAST.length + 3))?.bold).toBeFalsy();
  });

  test("pasting replaces the selection", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await writeClipboard(page, "closing");
    await selectWord(page, RICH.last, "paragraph");
    await page.keyboard.press(`${MOD}+V`);
    await expect.poll(() => paraText(page, RICH.last)).toBe("Final closing on page two.");
  });

  test("Edit, Copy as Markdown puts the selection on the clipboard as Markdown", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await selectWord(page, RICH.last, "paragraph");
    await page.keyboard.press(`${MOD}+B`);
    await chooseMenu(page, "Edit", "Copy as Markdown");
    await expect.poll(async () => (await readClipboard(page)).trim()).toBe("**paragraph**");
  });

  test("Ctrl/Cmd+C then Ctrl/Cmd+V duplicates a word", { tag: "@core" }, async ({ page }) => {
    await selectWord(page, RICH.last, "Final");
    await page.keyboard.press(`${MOD}+C`);
    await caretAtEnd(page, RICH.last);
    await page.keyboard.press(`${MOD}+V`);
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST}Final`);
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain(`${LAST}Final`);
  });
});
