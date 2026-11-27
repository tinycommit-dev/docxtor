// Icon packs: View, Icons chooses the set the menus, the toolbar and the context menu draw their icons from;
// the choice is remembered; an icon a pack leaves out stays Docxtor's. Also the outline icon.
import { chooseMenu, documentReady, expect, inPage, menuItem, openFixture, openMenu, test, type Page } from "./helpers";

const stroke = (page: Page, selector: string) => page.locator(selector).first().evaluate((el) => el.getAttribute("stroke-width"));
const toolbarIcon = (id: string) => `.toolbar button svg[data-icon="${id}"]`;

async function pick(page: Page, pack: string) {
  await chooseMenu(page, "View", "Icons…");
  const picker = page.getByRole("dialog", { name: "Icons" });
  await expect(picker).toBeVisible();
  await picker.locator(".qp-item").filter({ has: page.locator(".qp-label", { hasText: new RegExp(`^${pack}$`) }) }).click();
  await expect(picker).toBeHidden();
}

test.describe("icon packs", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("View, Icons lists the packs and marks the current one", async ({ page }) => {
    await chooseMenu(page, "View", "Icons…");
    const picker = page.getByRole("dialog", { name: "Icons" });
    const rows = picker.locator(".qp-item");
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(0)).toContainText("Docxtor");
    await expect(rows.nth(0)).toContainText("Current");
    await expect(rows.nth(1)).toContainText("Docxtor light");
    await page.keyboard.press("Escape");
    await expect(picker).toBeHidden();
    expect(await inPage(page, (w) => w.fe.editor && (w.fe as unknown as { currentIconPack: string }).currentIconPack)).toBe("docxtor");
  });

  test("choosing the light pack redraws the toolbar and the menus, and Docxtor brings them back", async ({ page }) => {
    expect(await stroke(page, toolbarIcon("undo"))).toBe("1.75");
    await pick(page, "Docxtor light");
    expect(await stroke(page, toolbarIcon("undo"))).toBe("1.25");
    expect(await stroke(page, toolbarIcon("table"))).toBe("1.25");
    // A menu line follows too, and so does the right-click menu.
    await openMenu(page, "Insert");
    expect(await menuItem(page, "Table").locator("svg").first().evaluate((el) => el.getAttribute("stroke-width"))).toBe("1.25");
    await page.keyboard.press("Escape");
    await page.locator(".flow .fp").first().click({ button: "right" });
    expect(await page.locator(".ctx-menu svg").first().evaluate((el) => el.getAttribute("stroke-width"))).toBe("1.25");
    await page.keyboard.press("Escape");
    // The chart icons are in the pack too.
    await openMenu(page, "Insert");
    await menuItem(page, "Chart").hover();
    expect(await menuItem(page, "Column").locator("svg").first().evaluate((el) => el.getAttribute("stroke-width"))).toBe("1.25");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");

    await pick(page, "Docxtor");
    expect(await stroke(page, toolbarIcon("undo"))).toBe("1.75");
    await openMenu(page, "Insert");
    expect(await menuItem(page, "Table").locator("svg").first().evaluate((el) => el.getAttribute("stroke-width"))).toBe("1.75");
  });

  test("the choice is remembered when the page is opened again", async ({ page }) => {
    await pick(page, "Docxtor light");
    await page.reload();
    await documentReady(page);
    expect(await inPage(page, (w) => (w.fe as unknown as { currentIconPack: string }).currentIconPack)).toBe("docxtor-light");
    expect(await stroke(page, toolbarIcon("undo"))).toBe("1.25");
  });

  test("a pack from an extension replaces the icons it draws; the rest stay Docxtor's", async ({ page }) => {
    await page.evaluate(async () => {
      const w = window as unknown as { docxtor: { fe: { use(ext: unknown): Promise<unknown> } } };
      await w.docxtor.fe.use({
        id: "acme.icons",
        name: "Acme icons",
        activate(ctx: { iconPacks: { register(p: unknown): unknown } }) {
          ctx.iconPacks.register({ id: "acme", label: "Acme", icons: [{ id: "undo", svg: '<svg viewBox="0 0 24 24" data-acme="1" fill="none" stroke="currentColor"><circle cx="12" cy="12" r="6"/></svg>' }] });
        },
      });
    });
    // Offered, not applied.
    expect(await page.locator(`${toolbarIcon("undo")}[data-acme]`).count()).toBe(0);
    await pick(page, "Acme");
    await expect(page.locator(`${toolbarIcon("undo")}[data-acme]`)).toHaveCount(1);
    expect(await stroke(page, toolbarIcon("redo"))).toBe("1.75");
    // Turning the extension off puts Docxtor's icon back; the choice waits for it.
    await inPage(page, (w) => (w.fe.extensions as unknown as { disable(id: string): Promise<void> }).disable("acme.icons"));
    await expect(page.locator(`${toolbarIcon("undo")}[data-acme]`)).toHaveCount(0);
    expect(await stroke(page, toolbarIcon("undo"))).toBe("1.75");
    await inPage(page, (w) => (w.fe.extensions as unknown as { enable(id: string): Promise<void> }).enable("acme.icons"));
    await expect(page.locator(`${toolbarIcon("undo")}[data-acme]`)).toHaveCount(1);
  });

  test("an icon registered on its own beats the chosen pack", async ({ page }) => {
    await pick(page, "Docxtor light");
    await page.evaluate(async () => {
      const w = window as unknown as { docxtor: { fe: { use(ext: unknown): Promise<unknown> } } };
      await w.docxtor.fe.use({
        id: "acme.bold",
        name: "Acme bold",
        activate(ctx: { icons: { register(i: unknown): unknown } }) {
          ctx.icons.register({ id: "undo", svg: '<svg viewBox="0 0 24 24" data-solo="1" fill="none" stroke="currentColor"><path d="M6 6h12"/></svg>' });
        },
      });
    });
    await expect(page.locator(`${toolbarIcon("undo")}[data-solo]`)).toHaveCount(1);
    expect(await stroke(page, toolbarIcon("redo"))).toBe("1.25");
  });

  test("the navigation pane icon is an indented list with bullets", async ({ page }) => {
    await openMenu(page, "View");
    const icon = menuItem(page, "Show navigation pane").locator("svg").first();
    await expect(icon.locator("circle")).toHaveCount(4);
    // Lines get shorter as the bullets step in: each line starts further right, all end at the same edge.
    const lines = await icon.locator("path").first().getAttribute("d");
    expect(lines).toContain("M8.5 5.5H20");
    expect(lines).toContain("M15.5 14.5H20");
  });
});
