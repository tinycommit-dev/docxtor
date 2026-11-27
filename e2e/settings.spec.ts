// Tools, Preferences and Customize keyboard shortcuts: how soon documents save, the font new documents start in, the
// version interval, exceptions for automatic capitals, and keyboard shortcuts of your own.
import { answer, chooseMenu, dialog, documentReady, expect, inPage, openFixture, type Page, test } from "./helpers";

const MOD = "ControlOrMeta";

async function openPreferences(page: Page) {
  await chooseMenu(page, "Tools", "Preferences…");
  const d = dialog(page, "Preferences");
  await expect(d).toBeVisible();
  return d;
}

/** A new empty document in the Library, open on screen. */
async function newDoc(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w) => w.createIn(w.files.library, "", "Blank.docx", ""));
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Blank");
  await documentReady(page);
}

const prefs = (page: Page) => page.evaluate(() => JSON.parse(localStorage.getItem("docxtor.prefs") ?? "{}") as Record<string, unknown>);

test.describe("preferences", () => {
  test("Preferences lists the app's settings under their headings", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openPreferences(page);
    await expect(d.getByRole("heading", { name: "Saving" })).toBeVisible();
    await expect(d.getByRole("heading", { name: "New documents" })).toBeVisible();
    await expect(d.getByLabel("Save documents in your Library after")).toHaveValue("1");
    await expect(d.getByLabel("Keep a version in the history every")).toHaveValue("5");
    await expect(d.getByLabel("Font", { exact: true })).toHaveValue("");
    await expect(d.getByLabel("Font size")).toHaveValue("0");
    await expect(d.getByRole("button", { name: "Change shortcuts…" })).toBeVisible();
  });

  test("a longer saving interval holds back the autosave until it has passed or the document is saved", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openPreferences(page);
    await d.getByLabel("Save documents in your Library after").fill("90");
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toBeHidden();
    expect((await prefs(page)).autosaveSeconds).toBe(90);
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "SLOWSAVE " });
    });
    await expect(page.locator(".led")).toHaveText("Unsaved changes");
    // Well past the old one second: still not written.
    await page.waitForTimeout(2500);
    await expect(page.locator(".led")).toHaveText("Unsaved changes");
    expect(await inPage(page, async (w) => {
      const bytes = await w.files.library.read("Fixtures/simple.docx");
      const E = w.fe.editor.engine!.constructor as unknown as { open(b: Uint8Array, n: string): { text(): string } };
      return E.open(bytes, "x.docx").text().includes("SLOWSAVE");
    })).toBe(false);
    // Saving by hand writes it.
    await page.keyboard.press(`${MOD}+s`);
    await expect(page.locator(".led")).toHaveText(/Saved/);
    // The bytes are compressed, so look for the text through a fresh engine.
    const text = () => inPage(page, async (w) => {
      const bytes = await w.files.library.read("Fixtures/simple.docx");
      const E = w.fe.editor.engine!.constructor as unknown as { open(b: Uint8Array, n: string): { text(): string } };
      return E.open(bytes, "x.docx").text();
    });
    expect(await text()).toContain("SLOWSAVE");
  });

  test("the default font and size are what a new empty document starts in", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openPreferences(page);
    await d.getByLabel("Font", { exact: true }).fill("Georgia");
    await d.getByLabel("Font size").fill("14");
    await d.getByRole("button", { name: "Apply" }).click();
    await chooseMenu(page, "File", "New document");
    await expect.poll(() => inPage(page, (w) => w.engine!.fileName)).toMatch(/^Untitled/);
    await documentReady(page);
    const run = await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      return e.formatAt({ p: e.renderAll().order[0], o: 0 })?.run;
    });
    expect(run).toMatchObject({ font: "Georgia", size: 14 });
    // The setting is the app's and survives a reload.
    await page.reload();
    await documentReady(page);
    expect(await prefs(page)).toMatchObject({ defaultFont: "Georgia", defaultSize: 14 });
  });

  test("the font setting offers the fonts the document menu offers", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openPreferences(page);
    await d.getByLabel("Font", { exact: true }).focus();
    await expect(d.locator("datalist option[value='Cambria']")).toHaveCount(1);
    await expect(d.locator("datalist option[value='Times New Roman']")).toHaveCount(1);
  });

  test("an abbreviation you add keeps the word after it from being capitalised", async ({ page }) => {
    await newDoc(page);
    const type = async (text: string) => {
      await page.locator(".flow .fp").first().click();
      await page.keyboard.press(`${MOD}+a`);
      await page.keyboard.press("Backspace");
      await page.keyboard.type(text);
    };
    await type("call the dept. he said ");
    await expect(page.locator(".flow .fp").first()).toContainText("Call the dept. He said");
    const d = await openPreferences(page);
    await d.getByLabel("Don't capitalize after these abbreviations").fill("dept.\nreq");
    await d.getByRole("button", { name: "Apply" }).click();
    await type("call the dept. he said ");
    await expect(page.locator(".flow .fp").first()).toContainText("Call the dept. he said");
    // A word that is not on the list still ends a sentence.
    await type("call the office. he said ");
    await expect(page.locator(".flow .fp").first()).toContainText("Call the office. He said");
    // The list is kept with the other typing settings.
    const again = await openPreferences(page);
    await expect(again.getByLabel("Don't capitalize after these abbreviations")).toHaveValue("dept.\nreq.");
  });
});

test.describe("keyboard shortcuts", () => {
  async function openShortcuts(page: Page) {
    await chooseMenu(page, "Tools", "Customize keyboard shortcuts…");
    const d = dialog(page, "Keyboard shortcuts");
    await expect(d).toBeVisible();
    return d;
  }
  const row = (d: ReturnType<Page["locator"]>, title: string) => d.locator(".kb-row").filter({ has: d.page().locator(".kb-name span:first-child", { hasText: new RegExp(`^${title}$`) }) });

  test("the list shows commands that have shortcuts, and the search finds any command", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openShortcuts(page);
    await expect(row(d, "Bold")).toBeVisible();
    await expect(row(d, "Bold").locator(".kbd")).toHaveText(/B$/);
    await expect(row(d, "Bold").getByRole("button", { name: "Reset the shortcut of Bold" })).toHaveCount(0);
    // A command with no shortcut turns up in a search.
    await d.getByRole("searchbox", { name: "Search commands" }).fill("Word count");
    await expect(row(d, "Word count")).toBeVisible();
    await d.getByRole("searchbox", { name: "Search commands" }).fill("zzzzz");
    await expect(d.locator(".kb-note")).toHaveText("No command matches.");
  });

  test("a shortcut of your own replaces the old one, works at once and after a reload, and can be reset", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openShortcuts(page);
    await d.getByRole("searchbox", { name: "Search commands" }).fill("Bold");
    await row(d, "Bold").getByRole("button", { name: "Change the shortcut of Bold" }).click();
    await expect(d.getByRole("status")).toContainText("Press the keys");
    await page.keyboard.press(`${MOD}+Alt+j`);
    await expect(row(d, "Bold").locator(".kbd")).toHaveText(/J$/);
    await d.getByRole("button", { name: "Done" }).click();
    // The new keys make the selection bold; the old ones no longer do.
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+a`);
    await page.keyboard.press(`${MOD}+b`);
    expect(await inPage(page, (w) => { const e = w.fe.editor.engine!; return e.formatAt({ p: e.renderAll().order[0], o: 1 })?.run.bold; })).toBeFalsy();
    await page.keyboard.press(`${MOD}+Alt+j`);
    await expect.poll(() => inPage(page, (w) => { const e = w.fe.editor.engine!; return !!e.formatAt({ p: e.renderAll().order[0], o: 1 })?.run.bold; })).toBe(true);
    // After a reload the choice is still there: the keys toggle bold whether or not the bold they made was saved with the document.
    await page.reload();
    await documentReady(page);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+a`);
    const bold = () => inPage(page, (w) => { const e = w.fe.editor.engine!; return !!e.formatAt({ p: e.renderAll().order[0], o: 1 })?.run.bold; });
    const was = await bold();
    await page.keyboard.press(`${MOD}+Alt+j`);
    await expect.poll(bold).toBe(!was);
    // Reset gives the command its own shortcut back.
    const again = await openShortcuts(page);
    await again.getByRole("searchbox", { name: "Search commands" }).fill("Bold");
    await row(again, "Bold").getByRole("button", { name: "Reset the shortcut of Bold" }).click();
    await expect(row(again, "Bold").locator(".kbd")).toHaveText(/B$/);
    await again.getByRole("button", { name: "Done" }).click();
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+a`);
    const before = await bold();
    await page.keyboard.press(`${MOD}+b`);
    await expect.poll(bold).toBe(!before);
  });

  test("a key another command uses is taken only after asking, and from that command", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openShortcuts(page);
    await d.getByRole("searchbox", { name: "Search commands" }).fill("Bold");
    await row(d, "Bold").getByRole("button", { name: "Change the shortcut of Bold" }).click();
    await page.keyboard.press(`${MOD}+i`);
    await expect(d.getByRole("status")).toContainText("is used by Italic. Use it here instead?");
    // Nothing changed yet.
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(row(d, "Bold").locator(".kbd")).toHaveText(/B$/);
    await row(d, "Bold").getByRole("button", { name: "Change the shortcut of Bold" }).click();
    await page.keyboard.press(`${MOD}+i`);
    await d.getByRole("button", { name: "Use it here" }).click();
    await expect(row(d, "Bold").locator(".kbd")).toHaveText(/I$/);
    await d.getByRole("searchbox", { name: "Search commands" }).fill("Italic");
    await expect(row(d, "Italic").locator(".kb-none")).toHaveText("None");
    await d.getByRole("button", { name: "Reset all" }).click();
    await answer(page, "Reset all");
    await d.getByRole("searchbox", { name: "Search commands" }).fill("Italic");
    await expect(row(d, "Italic").locator(".kbd")).toHaveText(/I$/);
  });

  test("Escape leaves a change alone, and Remove takes the shortcut away", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const d = await openShortcuts(page);
    await row(d, "Bold").getByRole("button", { name: "Change the shortcut of Bold" }).click();
    await page.keyboard.press("Escape");
    // Escape ends the change, not the dialog.
    await expect(d).toBeVisible();
    await expect(row(d, "Bold").locator(".kbd")).toHaveText(/B$/);
    await row(d, "Bold").getByRole("button", { name: "Remove the shortcut of Bold" }).click();
    await expect(row(d, "Bold").locator(".kb-none")).toHaveText("None");
  });
});
