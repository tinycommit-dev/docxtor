import { dialog, expect, inPage, openFixture, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const FONT = "Frutiger Next";

/**
 * Open a copy of the document in which the start of the last paragraph uses a font no machine has.
 * (Setup only: the app is told about the new document the same way an embedder would.)
 */
async function openWithMissingFont(page: Page) {
  await inPage(
    page,
    async (w, font) => {
      const engine = w.engine!;
      engine.apply({ op: "format_text", from: { p: 32, o: 0 }, to: { p: 32, o: 15 }, format: { font } });
      await w.fe.open(engine.save(), "missing.docx");
    },
    FONT,
  );
  await expect.poll(() => inPage(page, (w) => w.fe.editor.pages)).toBeGreaterThan(0);
}

const strip = (page: Page) => page.locator(".fonts-strip");

test.describe("missing fonts", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("a document with only available fonts shows no strip", async ({ page }) => {
    await expect(strip(page)).toBeHidden();
  });

  test("names the font, offers to replace it, and undo brings the strip back", async ({ page }) => {
    await openWithMissingFont(page);
    await expect(strip(page)).toBeVisible();
    await expect(strip(page)).toContainText(FONT);
    await expect(strip(page)).toContainText("isn't available on this device");

    // Replace fonts… opens the dialog with a row for the font.
    await strip(page).getByRole("button", { name: "Replace fonts…" }).click();
    const replace = dialog(page, "Replace missing fonts");
    await expect(replace).toBeVisible();
    await expect(replace).toContainText(FONT);
    // A sans-serif font gets a sans-serif suggestion.
    await expect(replace.getByRole("combobox", { name: new RegExp(FONT) })).toHaveValue("Arial");

    await replace.getByRole("button", { name: "Replace" }).click();
    await expect(replace).toBeHidden();
    await expect(strip(page)).toBeHidden();
    expect((await inPage(page, (w) => w.fe.editor.engine!.fontsUsed())).map((f) => f.family)).not.toContain(FONT);

    // One undo puts the font back, and the strip notices (the check is debounced).
    await page.keyboard.press(`${MOD}+Z`);
    await expect(strip(page)).toBeVisible();
    await expect(strip(page)).toContainText(FONT);
  });

  test("Cancel leaves the document and the strip alone", async ({ page }) => {
    await openWithMissingFont(page);
    await strip(page).getByRole("button", { name: "Replace fonts…" }).click();
    await dialog(page, "Replace missing fonts").getByRole("button", { name: "Cancel" }).click();
    await expect(dialog(page, "Replace missing fonts")).toBeHidden();
    await expect(strip(page)).toBeVisible();
  });

  test("Dismiss hides the strip without changing the document", async ({ page }) => {
    await openWithMissingFont(page);
    await strip(page).getByRole("button", { name: "Dismiss" }).click();
    await expect(strip(page)).toBeHidden();
    expect((await inPage(page, (w) => w.fe.editor.engine!.fontsUsed())).map((f) => f.family)).toContain(FONT);
  });

  test("Format, Replace missing fonts… opens the same dialog", async ({ page }) => {
    await openWithMissingFont(page);
    await page.getByRole("menuitem", { name: "Format", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]', { hasText: "Replace missing fonts…" }).click();
    await expect(dialog(page, "Replace missing fonts")).toBeVisible();
  });
});

/** The stylesheets of fonts an extension offers (Google Fonts) on the page. */
const offeredSheets = (page: Page) => inPage(page, () => [...document.querySelectorAll<HTMLLinkElement>("link[data-docxtor-font]")].map((l) => l.dataset.docxtorFont!).sort());

test.describe("fonts an extension offers", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("load when something uses them, not when the app starts", async ({ page }) => {
    // Twenty Google Fonts are offered; a document that uses none of them loads none of their stylesheets.
    expect(await offeredSheets(page)).toEqual([]);
    // A document that uses one has it before its first layout.
    await inPage(page, async (w) => {
      const engine = w.engine!;
      engine.apply({ op: "format_text", from: { p: 32, o: 0 }, to: { p: 32, o: 15 }, format: { font: "Lato" } });
      await w.fe.open(engine.save(), "lato.docx");
    });
    expect(await offeredSheets(page)).toEqual(["Lato"]);
    // The font box draws every font in its own face, so it asks for all of them.
    await page.locator(".font-pick").click();
    await expect.poll(() => offeredSheets(page).then((s) => s.length)).toBeGreaterThan(10);
    expect(await offeredSheets(page)).toContain("Roboto");
  });
});
