import { caretAtEnd, chooseMenu, documentReady, expect, inPage, menuItem, openFixture, openMenu, paraText, RICH, test, type Page } from "./helpers";

const LAST = "Final paragraph on page two.";
const saved = (page: Page) => page.locator(".led");

test.describe("files", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("edits save to the Library by themselves and survive a reload", { tag: "@core" }, async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await page.keyboard.type(" Remembered.");
    // The edit is in the model, so the document is already marked as changed...
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain(`${LAST} Remembered.`);
    // ...and Docxtor saves a moment later without being asked.
    await expect(saved(page)).toHaveText("Saved in your Library");

    await page.reload();
    await documentReady(page);
    await expect(page).toHaveURL(/#\/library\/Fixtures\/rich\.docx$/);
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST} Remembered.`);
  });

  test("Ctrl/Cmd+S saves straight away", { tag: "@core" }, async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await page.keyboard.type(" Now");
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain(`${LAST} Now`);
    await page.keyboard.press("ControlOrMeta+S");
    await expect(saved(page)).toHaveText("Saved in your Library");
    await page.reload();
    await documentReady(page);
    await expect.poll(() => paraText(page, RICH.last)).toBe(`${LAST} Now`);
  });

  test("the top bar's Save button stays hidden in the Library", async ({ page }) => {
    const save = page.locator(".top-actions").getByRole("button", { name: /^Save/ });
    await expect(save).toBeHidden();
    await caretAtEnd(page, RICH.last);
    await page.keyboard.type(" x");
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain(" x");
    await expect(save).toBeHidden();
    await expect(saved(page)).toHaveText("Saved in your Library");
    await expect(save).toBeHidden();
  });

  test("File, Make a copy opens 'Copy of rich'", { tag: "@core" }, async ({ page }) => {
    await chooseMenu(page, "File", "Make a copy");
    await expect(page).toHaveURL(/Copy%20of%20rich/);
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Copy of rich");
    await expect(page).toHaveTitle("Copy of rich — Docxtor");
    await expect(page.locator(".statusbar")).toContainText("98 words");

    // A second copy doesn't overwrite the first.
    await chooseMenu(page, "File", "Make a copy");
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Copy of Copy of rich");
  });

  test("File, Open recent lists the other documents and reopens one", { tag: "@core" }, async ({ page }) => {
    await chooseMenu(page, "File", "Make a copy");
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Copy of rich");
    // The list follows the Recents the app keeps; give it a moment to catch up with the copy opening.
    await expect.poll(() => inPage(page, (w) => w.fe.extensions.menu("file/recent").flat().map((e) => e.title))).toEqual(["rich"]);

    await openMenu(page, "File");
    await menuItem(page, "Open recent").click();
    // The open document isn't listed, only the others.
    await expect(menuItem(page, "Copy of rich")).toHaveCount(0);
    await expect(menuItem(page, "rich")).toHaveCount(1);
    await menuItem(page, "rich").click();

    await expect(page).toHaveURL(/#\/library\/Fixtures\/rich\.docx$/);
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("rich");
  });

  test("renaming in the top bar renames the file and its address", { tag: "@core" }, async ({ page }) => {
    const name = page.getByRole("textbox", { name: "Document name" });
    await name.fill("Board report");
    await name.press("Enter");
    await expect(page).toHaveURL(/#\/library\/Fixtures\/Board%20report\.docx$/);
    await expect(page).toHaveTitle("Board report — Docxtor");
    await page.reload();
    await documentReady(page);
    await expect(name).toHaveValue("Board report");
  });

  test("the Docxtor wordmark shows the start screen, where a recent document reopens", { tag: "@core" }, async ({ page }) => {
    await page.getByRole("link", { name: "Docxtor, your documents" }).click();
    await expect(page.getByRole("heading", { name: "Your documents" })).toBeVisible();
    // The document appears among the recent ones; opening it brings it back.
    await page.locator(".start .doc-tile", { hasText: "rich" }).click();
    await expect(page.getByRole("heading", { name: "Your documents" })).toBeHidden();
    await expect(page.locator(".statusbar")).toContainText("98 words");
  });
});
