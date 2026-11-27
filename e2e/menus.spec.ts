import { chooseMenu, expect, menuBar, menuItem, openFixture, openMenu, runAt, selectWord, test, topMenu, RICH } from "./helpers";

const MENUS = ["File", "Edit", "View", "Insert", "Design", "Format", "Mailings", "Tools", "Extensions", "Help"];

test.describe("menu bar", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("lists every menu", async ({ page }) => {
    const tops = menuBar(page).getByRole("menuitem");
    await expect(tops).toHaveText(MENUS);
  });

  for (const name of MENUS) {
    test(`${name} opens, has items, and closes when you click away`, async ({ page }) => {
      await openMenu(page, name);
      const items = page.locator(".menu-drop").first().locator('[role="menuitem"], [role="menuitemcheckbox"]');
      expect(await items.count()).toBeGreaterThan(0);
      await expect(items.first()).toBeVisible();
      await page.locator(".statusbar").click({ position: { x: 600, y: 4 } });
      await expect(page.locator(".menu-drop")).toHaveCount(0);
      await expect(topMenu(page, name)).toHaveAttribute("aria-expanded", "false");
    });
  }

  test("Escape closes a menu that was opened with the mouse and leaves the focus in the document", async ({ page }) => {
    await selectWord(page, RICH.last, "paragraph");
    await openMenu(page, "File");
    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    await expect(topMenu(page, "File")).toHaveAttribute("aria-expanded", "false");
    // The focus never left the document, and the selection is still there.
    await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
    expect(await page.evaluate(() => getSelection()?.toString())).toBe("paragraph");
  });

  test("Escape with a submenu open (mouse) closes only the submenu, then the menu", async ({ page }) => {
    await openMenu(page, "Format");
    await menuItem(page, "Text").click();
    await expect(page.locator(".menu-drop")).toHaveCount(2);

    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(1);
    await expect(topMenu(page, "Format")).toHaveAttribute("aria-expanded", "true");
    await expect(menuItem(page, "Text")).toHaveAttribute("aria-expanded", "false");

    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    await expect(topMenu(page, "Format")).toHaveAttribute("aria-expanded", "false");
  });

  test("Escape in the document does nothing once the menu is closed again", async ({ page }) => {
    await openMenu(page, "File");
    await page.keyboard.press("Escape");
    // A second Escape (now the editor's) must not reopen or break anything.
    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    await openMenu(page, "Edit");
    await expect(menuItem(page, "Undo")).toBeVisible();
  });

  test("pointing at another menu title switches menus", async ({ page }) => {
    await openMenu(page, "File");
    await topMenu(page, "Edit").hover();
    await expect(topMenu(page, "Edit")).toHaveAttribute("aria-expanded", "true");
    await expect(topMenu(page, "File")).toHaveAttribute("aria-expanded", "false");
    await expect(menuItem(page, "Undo")).toBeVisible();
  });

  test("Format, Text, Bold applies bold to the selected word with the mouse", async ({ page }) => {
    await selectWord(page, RICH.last, "paragraph");
    expect((await runAt(page, RICH.last, 8))?.bold).toBeFalsy();

    await chooseMenu(page, "Format", "Text", "Bold");

    expect((await runAt(page, RICH.last, 8))?.bold).toBe(true);
    expect((await runAt(page, RICH.last, 1))?.bold).toBeFalsy();
  });

  test("works from the keyboard: arrows move, Right opens a submenu, Left and Escape close", async ({ page }) => {
    const file = topMenu(page, "File");
    await file.focus();
    await page.keyboard.press("ArrowDown");
    await expect(file).toHaveAttribute("aria-expanded", "true");

    const focused = page.locator(".menu-drop :focus");
    await expect(focused).toContainText("New document");
    await page.keyboard.press("ArrowDown");
    await expect(focused).toContainText("New from a template");
    await page.keyboard.press("ArrowUp");
    await expect(focused).toContainText("New document");
    await page.keyboard.press("End");
    await expect(focused).not.toContainText("New document");
    await page.keyboard.press("Home");
    await expect(focused).toContainText("New document");

    // Down to "Download", which has a submenu.
    for (let i = 0; i < 30; i++) {
      if (/^Download\b/.test((await focused.innerText().catch(() => "")).trim())) break;
      await page.keyboard.press("ArrowDown");
    }
    await expect(focused).toHaveText(/^Download/);
    await expect(focused).toHaveAttribute("aria-haspopup", "menu");
    await page.keyboard.press("ArrowRight");
    await expect(page.locator(".menu-drop")).toHaveCount(2);
    await expect(page.locator(".menu-drop").nth(1).locator(":focus")).toContainText("Word document");

    // Left closes the submenu and gives "Download" the focus back.
    await page.keyboard.press("ArrowLeft");
    await expect(page.locator(".menu-drop")).toHaveCount(1);
    await expect(focused).toHaveText(/^Download/);

    // Open it again, then Escape steps back one level at a time.
    await page.keyboard.press("ArrowRight");
    await expect(page.locator(".menu-drop")).toHaveCount(2);
    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(1);
    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    await expect(file).toBeFocused();
    await expect(file).toHaveAttribute("aria-expanded", "false");
  });

  test("Left and Right in an open menu move along the bar", async ({ page }) => {
    await topMenu(page, "File").focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Home");
    await page.keyboard.press("ArrowLeft");
    await expect(topMenu(page, "File")).toHaveAttribute("aria-expanded", "false");
    await expect(topMenu(page, "Help")).toHaveAttribute("aria-expanded", "true");
  });

  test("View, Show ruler hides the ruler and shows it again", async ({ page }) => {
    const ruler = page.locator(".ruler");
    await expect(ruler).toBeVisible();

    await openMenu(page, "View");
    await expect(menuItem(page, "Show ruler")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Show ruler").click();
    await expect(ruler).toBeHidden();

    // The next time the menu opens, the item is not checked.
    await openMenu(page, "View");
    await expect(menuItem(page, "Show ruler")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Show ruler").click();
    await expect(ruler).toBeVisible();

    await openMenu(page, "View");
    await expect(menuItem(page, "Show ruler")).toHaveAttribute("aria-checked", "true");
  });

  test("items that don't apply are disabled", async ({ page }) => {
    await openMenu(page, "Edit");
    // Nothing is selected: no Cut, Copy or Delete. Paste still works.
    await expect(menuItem(page, "Cut")).toBeDisabled();
    await expect(menuItem(page, "Copy")).toBeDisabled();
    await expect(menuItem(page, "Delete")).toBeDisabled();
    await expect(menuItem(page, "Paste")).toBeEnabled();
    await expect(menuItem(page, "Select all")).toBeEnabled();
    await page.keyboard.press("Escape");

    // With a word selected they come back.
    await selectWord(page, RICH.last, "paragraph");
    await openMenu(page, "Edit");
    await expect(menuItem(page, "Cut")).toBeEnabled();
    await expect(menuItem(page, "Copy")).toBeEnabled();
    await expect(menuItem(page, "Delete")).toBeEnabled();
  });

  test("table commands are disabled outside a table", async ({ page }) => {
    await openMenu(page, "Format");
    await menuItem(page, "Table").click();
    await expect(menuItem(page, "Insert row above")).toBeDisabled();
    await expect(menuItem(page, "Delete table")).toBeDisabled();
  });
});
