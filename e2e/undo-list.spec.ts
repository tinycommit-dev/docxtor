// The undo list: a dropdown on the Undo button (and on Redo) that names the steps and undoes to the one you choose.
import { expect, inPage, openFixture, RICH, test, type Page } from "./helpers";

const undoList = (page: Page) => page.getByRole("button", { name: "Undo history" });
const redoList = (page: Page) => page.getByRole("button", { name: "Redo history" });
const items = (page: Page) => page.locator(".history-items [role=menuitem]");

/** Three separate edits: typing, a format and new document properties. */
async function threeSteps(page: Page) {
  await inPage(page, (w) => {
    const e = w.fe.editor;
    const p = e.engine!.renderAll().order[0];
    e.exec({ op: "insert_text", at: { p, o: 0 }, text: "One " });
    e.exec({ op: "format_text", from: { p, o: 0 }, to: { p, o: 3 }, format: { bold: true } });
    e.exec({ op: "set_doc_props", title: "Named" });
  });
}

test.describe("the undo list", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page, "simple.docx");
  });

  test("it is off until there is something to undo, then names each step newest first", { tag: "@core" }, async ({ page }) => {
    await expect(undoList(page)).toBeDisabled();
    await expect(redoList(page)).toBeDisabled();
    await threeSteps(page);
    await expect(undoList(page)).toBeEnabled();
    await undoList(page).click();
    await expect(items(page)).toHaveText(["Document properties", "Text formatting", "Typing"]);
    // The first line is marked, and the line below the list says what a click does.
    await expect(items(page).first()).toHaveClass(/active/);
    await expect(page.locator(".history-foot")).toHaveText("Undo 1 action");
  });

  test("pointing at a step marks it and every step after it, and a click undoes them all in one go", { tag: "@core" }, async ({ page }) => {
    await threeSteps(page);
    await undoList(page).click();
    await items(page).nth(1).hover();
    await expect(items(page).nth(0)).toHaveClass(/active/);
    await expect(items(page).nth(1)).toHaveClass(/active/);
    await expect(items(page).nth(2)).not.toHaveClass(/active/);
    await expect(page.locator(".history-foot")).toHaveText("Undo 2 actions");
    await items(page).nth(1).click();
    await expect(page.locator(".history-items")).toHaveCount(0);
    // The title and the bold are gone; the typing stays.
    const state = await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      const p = e.renderAll().order[0];
      return { title: e.docProps().title, text: e.text().split("\n")[0], bold: e.formatAt({ p, o: 1 })?.run.bold };
    });
    expect(state.title).toBeUndefined();
    expect(state.text.startsWith("One ")).toBe(true);
    expect(state.bold).toBeFalsy();
    // What was undone is on the redo list, in the order it comes back.
    await expect(redoList(page)).toBeEnabled();
    await redoList(page).click();
    await expect(items(page)).toHaveText(["Text formatting", "Document properties"]);
    await expect(page.locator(".history-foot")).toHaveText("Redo 1 action");
    await items(page).nth(1).click();
    expect(await inPage(page, (w) => w.fe.editor.engine!.docProps().title)).toBe("Named");
    await expect(redoList(page)).toBeDisabled();
  });

  test("the keyboard walks the list, Enter chooses and Escape closes", async ({ page }) => {
    await threeSteps(page);
    await undoList(page).click();
    await expect(items(page).first()).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(items(page).nth(1)).toBeFocused();
    await expect(page.locator(".history-foot")).toHaveText("Undo 2 actions");
    await page.keyboard.press("Escape");
    await expect(page.locator(".history-items")).toHaveCount(0);
    await undoList(page).click();
    await page.keyboard.press("End");
    await expect(page.locator(".history-foot")).toHaveText("Undo 3 actions");
    await page.keyboard.press("Enter");
    await expect.poll(() => inPage(page, (w) => (w.fe.editor.engine as unknown as { undoLabels(): string[] }).undoLabels().length)).toBe(0);
    await expect(undoList(page)).toBeDisabled();
  });

  test("other steps are named in words too", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await inPage(page, (w, id) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_table", after: id, rows: 2, cols: 2 });
      e.exec({ op: "set_page_color", color: "FFFFCC" });
    }, RICH.intro);
    await undoList(page).click();
    await expect(items(page).first()).toHaveText("Page color");
    await expect(items(page).nth(1)).toHaveText("Insert table");
  });
});

test.describe("what one undo step holds", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.press("End");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Second line.");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Third line.");
    await expect(page.locator(".flow [data-p]")).toHaveCount(3);
  });

  const para = (page: Page, i: number) => page.locator(".flow [data-p]").nth(i);
  const endOf = async (page: Page, i: number) => {
    await para(page, i).click();
    await page.keyboard.press("End");
  };

  test("typing goes on in one step at one place, and a click elsewhere starts a new step however soon the typing follows", async ({ page }) => {
    await endOf(page, 0);
    await page.keyboard.type(" abc");
    await endOf(page, 2);
    await page.keyboard.type("xyz");
    await expect(para(page, 2)).toHaveText("Third line.xyz");
    await page.keyboard.press("ControlOrMeta+Z");
    // Only the typing at the second place is taken back.
    await expect(para(page, 2)).toHaveText("Third line.");
    await expect(para(page, 0)).toHaveText("Hello world. abc");
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(para(page, 0)).toHaveText("Hello world.");
  });

  test("deleting in two places soon after each other is two steps", async ({ page }) => {
    await endOf(page, 0);
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    await endOf(page, 2);
    await page.keyboard.press("Backspace");
    await expect(para(page, 0)).toHaveText("Hello worl");
    await expect(para(page, 2)).toHaveText("Third line");
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(para(page, 2)).toHaveText("Third line.");
    await expect(para(page, 0)).toHaveText("Hello worl");
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(para(page, 0)).toHaveText("Hello world.");
  });
});
