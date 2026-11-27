import { contextItem, contextMenu, expect, inPage, openFixture, para, RICH, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** Right-click at a point `x`,`y` inside the first page's sheet (its top-left corner is 0,0). */
async function rightClickSheet(page: Page, x: number, y: number) {
  const box = (await page.locator(".sheet").first().boundingBox())!;
  await page.mouse.click(box.x + x, box.y + y, { button: "right" });
}

test.describe("right-click menu", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("on body text offers the clipboard actions", async ({ page }) => {
    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await expect(contextMenu(page)).toBeVisible();
    for (const name of ["Cut", "Copy", "Paste", "Paste as plain text", "Delete", "Copy as Markdown", "Paste from Markdown", "Link…"]) {
      await expect(contextItem(page, name)).toBeVisible();
    }
    // Nothing is selected, so the ones that need a selection are greyed out; Paste is not.
    await expect(contextItem(page, "Cut")).toBeDisabled();
    await expect(contextItem(page, "Copy")).toBeDisabled();
    await expect(contextItem(page, "Delete")).toBeDisabled();
    await expect(contextItem(page, "Paste")).toBeEnabled();
    // Table and image actions are not offered on plain text.
    await expect(contextItem(page, "Insert row above")).toHaveCount(0);
    await expect(contextItem(page, "Alt text…")).toHaveCount(0);
  });

  test("in a table cell offers row actions, and Insert row above adds a row", async ({ page }) => {
    const rows = page.locator(".flow table.ft tr");
    await expect(rows).toHaveCount(4);

    await para(page, RICH.cell).click({ button: "right" });
    for (const name of ["Insert row above", "Insert row below", "Insert column left", "Insert column right", "Delete row", "Delete column", "Delete table"]) {
      await expect(contextItem(page, name)).toBeVisible();
    }
    await contextItem(page, "Insert row above").click();

    await expect(contextMenu(page)).toHaveCount(0);
    await expect(rows).toHaveCount(5);

    await page.keyboard.press(`${MOD}+Z`);
    await expect(rows).toHaveCount(4);
  });

  test("on the image offers Alt text", async ({ page }) => {
    await page.locator(".flow .atom.img").click({ button: "right" });
    await expect(contextItem(page, "Alt text…")).toBeVisible();
    await expect(contextItem(page, "Replace image…")).toBeVisible();

    await contextItem(page, "Alt text…").click();
    const dialog = page.getByRole("dialog", { name: "Alt text" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("textbox", { name: "Describe the image for screen readers" }).fill("A bar chart of revenue");
    await dialog.getByRole("button", { name: "Save" }).click();
    await expect(dialog).toBeHidden();
    await expect(page.locator(".flow .atom.img img")).toHaveAttribute("alt", "A bar chart of revenue");
  });

  test("on the top margin offers Edit header, which opens header editing", async ({ page }) => {
    await rightClickSheet(page, 300, 55);
    await expect(contextItem(page, "Edit header")).toBeVisible();
    // The body's actions are not offered here.
    await expect(contextItem(page, "Cut")).toHaveCount(0);

    await contextItem(page, "Edit header").click();
    await expect(page.getByRole("textbox", { name: "Header", exact: true })).toBeFocused();
    await expect(page.getByRole("toolbar", { name: "Header and footer" })).toContainText("Header on page 1");
    await expect(page.getByRole("textbox", { name: "Header", exact: true })).toContainText("Docxtor — confidential draft");

    // Escape goes back to the body.
    await page.keyboard.press("Escape");
    await expect(page.getByRole("toolbar", { name: "Header and footer" })).toBeHidden();
    await expect(page.getByRole("textbox", { name: "Header", exact: true })).toHaveCount(0);
  });

  test("on the bottom margin offers Edit footer", async ({ page }) => {
    const footer = page.locator(".sheet .ftr").first();
    // The pages are drawn again while the document settles: take the footer's place once it stays put.
    await expect(async () => {
      await footer.scrollIntoViewIfNeeded({ timeout: 1000 });
      const box = (await footer.boundingBox())!;
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
      await expect(contextItem(page, "Edit footer")).toBeVisible({ timeout: 1000 });
    }).toPass();
  });

  test("Shift+right-click leaves the browser's own menu alone", async ({ page, browserName }) => {
    await page.evaluate(() => {
      const seen: { shift: boolean; prevented: boolean }[] = [];
      (window as unknown as { __seen: typeof seen }).__seen = seen;
      // Bubbling listener on window: it runs after Docxtor's, so it sees whether Docxtor took the event.
      window.addEventListener("contextmenu", (e) => seen.push({ shift: e.shiftKey, prevented: e.defaultPrevented }));
    });
    await para(page, RICH.intro).click({ button: "right", modifiers: ["Shift"], position: { x: 40, y: 8 } });
    // (Firefox does not send the page a contextmenu event at all when Shift is held, `dom.event.contextmenu.shift_suppresses_event`: its own menu opens and the page has no say.)
    if (browserName === "firefox") await page.waitForTimeout(300);
    else await expect.poll(() => page.evaluate(() => (window as unknown as { __seen: unknown[] }).__seen.length)).toBe(1);
    expect(await page.evaluate(() => (window as unknown as { __seen: unknown[] }).__seen)).toEqual(browserName === "firefox" ? [] : [{ shift: true, prevented: false }]);
    await expect(contextMenu(page)).toHaveCount(0);

    // Without Shift the same click is Docxtor's.
    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await expect(contextMenu(page)).toBeVisible();
    expect(await page.evaluate(() => (window as unknown as { __seen: { prevented: boolean }[] }).__seen.at(-1))).toEqual({ shift: false, prevented: true });
  });

  test("Escape closes it, and so does clicking away", async ({ page }) => {
    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await expect(contextMenu(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(contextMenu(page)).toHaveCount(0);

    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await expect(contextMenu(page)).toBeVisible();
    await page.locator(".statusbar").click({ position: { x: 600, y: 4 } });
    await expect(contextMenu(page)).toHaveCount(0);
  });

  test("Shift+F10 opens it at the caret and the arrow keys move through it", async ({ page }) => {
    await para(page, RICH.intro).click({ position: { x: 40, y: 8 } });
    await page.keyboard.press("Shift+F10");
    await expect(contextMenu(page)).toBeVisible();
    // Nothing is selected, so Cut and Copy are greyed out and the focus starts on Paste.
    await expect(contextItem(page, "Paste")).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(contextItem(page, "Paste as plain text")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(contextItem(page, "Paste")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(contextMenu(page)).toHaveCount(0);
  });

  test("Change page to landscape widens the page, and undo restores it", async ({ page }) => {
    const sheet = page.locator(".sheet").first();
    // The page is scaled to fit the window, so compare its shape rather than its pixels.
    const shape = async () => {
      const b = (await sheet.boundingBox())!;
      return Math.round((b.width / b.height) * 100) / 100;
    };
    const portrait = await shape();
    expect(portrait).toBeLessThan(1);

    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await contextItem(page, "Change page to landscape").click();

    await expect.poll(shape).toBeGreaterThan(1);
    expect((await inPage(page, (w) => w.engine!.pageSetup())).width).toBeGreaterThan(15000);

    // The menu now offers the way back.
    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await expect(contextItem(page, "Change page to portrait")).toBeVisible();
    await page.keyboard.press("Escape");

    await para(page, RICH.intro).click({ position: { x: 40, y: 8 } });
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(shape).toBe(portrait);
    expect((await inPage(page, (w) => w.engine!.pageSetup())).width).toBe(12240);
  });
});
