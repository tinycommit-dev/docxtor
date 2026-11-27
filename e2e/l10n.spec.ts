import { contextMenu, expect, openFixture, para, RICH, test, type Page } from "./helpers";

// The menu bar's own name is translated, so find it by class here.
const bar = (page: Page) => page.locator("nav.menubar");

// ?lang=en-XA is a pseudo-locale: every string that goes through t() comes out wrapped in ⟦…⟧, so
// any label without the brackets skipped translation.
const MARK = "⟦";

interface Line {
  /** Where it is, e.g. "Format > Text > Size". */
  path: string;
  label: string;
}

/** Open every top-level menu and every submenu under it, and list each line (depth-first). */
async function crawlMenus(page: Page): Promise<{ lines: Line[]; empty: string[] }> {
  const lines: Line[] = [];
  const empty: string[] = [];
  const tops = bar(page).locator(".menubar-top:not(.menubar-fold)");
  const titles = await tops.allInnerTexts();
  expect(titles).toHaveLength(10);

  const walk = async (depth: number, path: string) => {
    const panel = page.locator(".menu-drop").nth(depth);
    const items = panel.locator('[role="menuitem"], [role="menuitemcheckbox"]');
    // One round trip per panel: the crawl opens every submenu, and the menus keep growing.
    const rows = await items.evaluateAll((els) =>
      els.map((el) => ({ label: (el.querySelector(".menu-label")?.textContent ?? "").trim(), popup: el.getAttribute("aria-haspopup") === "menu" })),
    );
    expect(rows.length).toBeGreaterThan(0);
    for (let i = 0; i < rows.length; i++) {
      const { label, popup } = rows[i];
      lines.push({ path: `${path} > ${label}`, label });
      if (popup) {
        // A script click: a submenu already open may sit over the next line, and a pointer click would wait for it.
        await items.nth(i).evaluate((el) => (el as HTMLElement).click());
        const opened = await page.locator(".menu-drop").nth(depth + 1).waitFor({ timeout: 1000 }).then(() => true, () => false);
        if (opened) await walk(depth + 1, `${path} > ${label}`);
        else empty.push(`${path} > ${label}`);
      }
    }
  };

  for (let i = 0; i < titles.length; i++) {
    lines.push({ path: titles[i], label: titles[i] });
    await tops.nth(i).click();
    await expect(tops.nth(i)).toHaveAttribute("aria-expanded", "true");
    await walk(0, titles[i]);
    // A tall menu with a submenu open can cover the status bar, so close with the keyboard.
    for (let k = 0; k < 8 && (await page.locator(".menu-drop").count()) > 0; k++) await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);
  }
  return { lines, empty };
}

test.describe("translations", () => {
  test("pseudo-locale: every menu bar and menu label went through translation", async ({ page }) => {
    // The crawl opens every submenu, and the menus keep growing.
    test.setTimeout(180_000);
    await openFixture(page, "rich.docx", "&lang=en-XA");
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    const { lines, empty } = await crawlMenus(page);

    // Sanity: it really walked the nested menus (Format > Arrange > Align > Align left is four deep).
    expect(lines.length).toBeGreaterThan(100);
    expect(Math.max(...lines.map((l) => l.path.split(" > ").length))).toBeGreaterThanOrEqual(4);

    // Every submenu the menus list opened (the crawl didn't skip any).
    expect(empty).toEqual([]);

    // The line spacing options "1.15" and "1.5" are numbers in the language's own format, so the
    // pseudo-locale (which only wraps text) leaves them bare. Spanish below checks them.
    const untranslated = lines.filter((l) => !l.label.includes(MARK) && !/^[\d.,]+$/.test(l.label)).map((l) => l.path);
    expect(untranslated, "menu lines that skipped t()").toEqual([]);
  });

  test("pseudo-locale: the folded menu on a narrow window", async ({ page }) => {
    await page.setViewportSize({ width: 700, height: 900 });
    await openFixture(page, "rich.docx", "&lang=en-XA");
    const fold = bar(page).locator(".menubar-fold");
    await expect(fold).toContainText(MARK);
    await fold.click();
    const labels = await page.locator(".menu-drop .menu-label").allInnerTexts();
    expect(labels).toHaveLength(10);
    for (const l of labels) expect(l).toContain(MARK);
  });

  test("pseudo-locale: the right-click menu on text, a table cell, an image and the header", async ({ page }) => {
    await openFixture(page, "rich.docx", "&lang=en-XA");
    const check = async (where: string) => {
      const menu = contextMenu(page);
      await expect(menu).toBeVisible();
      const labels = await menu.locator(".menu-label").allInnerTexts();
      expect(labels.length, where).toBeGreaterThan(0);
      const bad = labels.filter((l) => !l.includes(MARK));
      expect(bad, `${where}: labels that skipped t()`).toEqual([]);
      // The hints that aren't shortcuts ("Double-click") and the footer line are text too.
      for (const hint of await menu.locator(".menu-hint").allInnerTexts()) expect(hint, `${where}: footer`).toContain(MARK);
      for (const hint of await menu.locator(".hint").allInnerTexts()) {
        if (/^[⌘⌃⌥⇧A-Za-z0-9+\\./,\[\]\-=\s]{1,12}$/.test(hint)) continue; // a shortcut
        expect(hint, `${where}: hint`).toContain(MARK);
      }
      await page.keyboard.press("Escape");
      await expect(menu).toHaveCount(0);
    };

    await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
    await check("text");

    await para(page, RICH.cell).click({ button: "right" });
    await check("table cell");

    await page.locator(".flow .atom.img").click({ button: "right" });
    await check("image");

    const header = page.locator(".sheet .hdr").first();
    await header.scrollIntoViewIfNeeded();
    const box = (await header.boundingBox())!;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
    await check("header");
  });

  test("pseudo-locale: the status bar, top bar and dialogs", async ({ page }) => {
    await openFixture(page, "rich.docx", "&lang=en-XA");
    await expect(page.locator(".statusbar")).toContainText(MARK);
    await expect(page.locator(".led")).toContainText(MARK);
    await expect(page.locator("input.doc-title")).toHaveAttribute("aria-label", new RegExp(MARK));

    await page.keyboard.press("ControlOrMeta+Shift+C");
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    for (const text of await dialog.locator("h2, dt, button").allInnerTexts()) expect(text).toContain(MARK);
  });

  test("Spanish: the menu bar starts with Archivo", async ({ page }) => {
    await openFixture(page, "rich.docx", "&lang=es");
    await expect(page.locator("html")).toHaveAttribute("lang", "es");
    const tops = bar(page).locator(".menubar-top:not(.menubar-fold)");
    await expect(tops.first()).toHaveText("Archivo");
    await expect(tops).toHaveText(["Archivo", "Edición", "Ver", "Insertar", "Diseño", "Formato", /^(Mailings|Correspondencia)$/, "Herramientas", "Extensiones", "Ayuda"]);
  });

  test("Spanish: line spacing multiples use a decimal comma", async ({ page }) => {
    await openFixture(page, "rich.docx", "&lang=es");
    await bar(page).locator(".menubar-top:not(.menubar-fold)").nth(5).click();
    await page.locator('.menu-drop [role="menuitem"]', { hasText: "Interlineado y espacio entre párrafos" }).click();
    await expect(page.locator(".menu-drop").nth(1).locator(".menu-label")).toHaveText(["Sencillo", "1,15", "1,5", "Doble", "Espacio antes del párrafo", "Espacio después del párrafo", "Mantener con el siguiente", "Mantener líneas juntas", "Evitar líneas sueltas", "Añadir salto de página antes"]);
  });
});
