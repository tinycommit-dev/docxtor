import { caretAtEnd, expect, inPage, openFixture, para, paraText, RICH, runAt, selectWord, test } from "./helpers";

const MOD = "ControlOrMeta";
const LAST = "Final paragraph on page two.";

test.describe("editing", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("typing puts text in the paragraph", { tag: "@core" }, async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await page.keyboard.type(" And more.");
    await expect(para(page, RICH.last)).toContainText(`${LAST} And more.`);
    // It is in the document, not only on screen.
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain(`${LAST} And more.`);
  });

  test("undo removes typed text and redo brings it back", { tag: "@core" }, async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await page.keyboard.type("Appended");
    await expect(para(page, RICH.last)).toContainText(`${LAST}Appended`);

    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => paraText(page, RICH.last)).toBe(LAST);

    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST}Appended`);

    // Ctrl+Y is the other redo.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => paraText(page, RICH.last)).toBe(LAST);
    await page.keyboard.press(`${MOD}+Y`);
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST}Appended`);
  });

  test("Ctrl/Cmd+B makes the selected word bold", { tag: "@core" }, async ({ page }) => {
    expect((await runAt(page, RICH.last, 8))?.bold).toBeFalsy();
    await selectWord(page, RICH.last, "paragraph");
    await page.keyboard.press(`${MOD}+B`);

    // "paragraph" is offsets 6 to 15: bold inside it, not outside.
    // The key press reaches the engine on its own time under load: wait for the bold, then check its edges.
    await expect.poll(async () => (await runAt(page, RICH.last, 8))?.bold).toBe(true);
    expect((await runAt(page, RICH.last, 1))?.bold).toBeFalsy();
    expect((await runAt(page, RICH.last, 20))?.bold).toBeFalsy();
    await expect(para(page, RICH.last).locator("span", { hasText: "paragraph" }).first()).toHaveCSS("font-weight", /^(700|bold)$/);

    // Pressing it again takes bold off.
    await page.keyboard.press(`${MOD}+B`);
    expect((await runAt(page, RICH.last, 8))?.bold).toBeFalsy();
  });

  test("Ctrl/Cmd+I italicises the selection", { tag: "@core" }, async ({ page }) => {
    await selectWord(page, RICH.last, "Final");
    await page.keyboard.press(`${MOD}+I`);
    expect((await runAt(page, RICH.last, 2))?.italic).toBe(true);
    expect((await runAt(page, RICH.last, 10))?.italic).toBeFalsy();
  });

  test("Enter splits a paragraph at the caret", { tag: "@core" }, async ({ page }) => {
    const before = await page.locator(".flow .fp").count();
    // Put the caret after "Final paragraph" by selecting the word and collapsing to its end.
    await selectWord(page, RICH.last, "paragraph");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Enter");

    await expect(page.locator(".flow .fp")).toHaveCount(before + 1);
    await expect.poll(() => paraText(page, RICH.last)).toBe("Final paragraph");
    // The rest moved to a new paragraph straight after it.
    await expect(page.locator(".flow .fp", { hasText: /^ on page two\.$/ })).toHaveCount(1);

    // One undo joins them again.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(page.locator(".flow .fp")).toHaveCount(before);
    await expect.poll(() => paraText(page, RICH.last)).toBe(LAST);
  });

  test("Backspace at the start of a paragraph joins it to the one before", { tag: "@core" }, async ({ page }) => {
    const before = await page.locator(".flow .fp").count();
    await para(page, RICH.last).click({ position: { x: 1, y: 8 } });
    await page.keyboard.press("Backspace");
    await expect(page.locator(".flow .fp")).toHaveCount(before - 1);
    await expect(page.locator(".flow .fp", { hasText: `Appendix${LAST}` })).toHaveCount(1);
  });
});
