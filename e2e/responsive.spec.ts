import { documentReady, expect, menuBar, menuItem, test, topMenu } from "./helpers";

test.describe("narrow window", () => {
  test.use({ viewport: { width: 700, height: 900 } });

  test.beforeEach(async ({ page }) => {
    await page.goto("/?open=rich.docx");
    await documentReady(page);
  });

  test("the menu bar folds into one Menu button", async ({ page }) => {
    const fold = menuBar(page).getByRole("button", { name: "Menu" });
    await expect(fold).toBeVisible();
    // The eight titles are not shown beside it.
    for (const name of ["File", "Edit", "View", "Insert", "Format", "Tools", "Extensions", "Help"]) await expect(topMenu(page, name)).toBeHidden();
  });

  test("Menu lists the menus and File opens as a submenu", async ({ page }) => {
    await menuBar(page).getByRole("button", { name: "Menu" }).click();
    for (const name of ["File", "Edit", "View", "Insert", "Format", "Tools", "Extensions", "Help"]) await expect(menuItem(page, name)).toBeVisible();

    await menuItem(page, "File").click();
    await expect(page.locator(".menu-drop")).toHaveCount(2);
    await expect(menuItem(page, "Make a copy")).toBeVisible();

    // A command from there works and closes everything.
    await menuItem(page, "Document details…").click();
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    await expect(page.getByRole("dialog", { name: "Document details" })).toBeVisible();
  });

  test("the keyboard opens it too", async ({ page }) => {
    const fold = menuBar(page).getByRole("button", { name: "Menu" });
    await fold.focus();
    await page.keyboard.press("Enter");
    await expect(page.locator(".menu-drop :focus")).toContainText("File");
    await page.keyboard.press("ArrowRight");
    await expect(page.locator(".menu-drop")).toHaveCount(2);
    await expect(page.locator(".menu-drop").nth(1).locator(":focus")).toBeVisible();
  });

  test("the document still opens and edits at this width", async ({ page }) => {
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await expect(page.locator(".statusbar")).toContainText("98 words");
  });
});

test.describe("wide window", () => {
  test("the full menu bar is shown and the Menu button is not", async ({ page }) => {
    await page.goto("/?open=rich.docx");
    await documentReady(page);
    await expect(topMenu(page, "File")).toBeVisible();
    await expect(menuBar(page).getByRole("button", { name: "Menu" })).toBeHidden();
  });
});
