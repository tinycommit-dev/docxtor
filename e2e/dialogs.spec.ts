import { buildDocx, openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, documentReady, expect, inPage, openFixture, RICH, selectWord, test } from "./helpers";

const MOD = "ControlOrMeta";

test.describe("dialogs", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("Ctrl/Cmd+Shift+C opens Word count with the document's numbers", async ({ page }) => {
    // The status bar's count settles after a short delay; the dialog reads the engine directly.
    await page.keyboard.press(`${MOD}+Shift+C`);
    const word = dialog(page, "Word count");
    await expect(word).toBeVisible();
    await expect(word.getByText("Words", { exact: true })).toBeVisible();
    await expect(word.locator("dt", { hasText: /^Words$/ }).locator("+ dd")).toHaveText("98");
    await expect(word.locator("dt", { hasText: /^Pages$/ }).locator("+ dd")).toHaveText("2");
    const counts = await inPage(page, (w) => w.engine!.counts());
    await expect(word.locator("dt", { hasText: /^Characters$/ }).locator("+ dd")).toHaveText(counts.chars.toLocaleString("en-US"));

    // Done closes it and gives the document its focus back.
    await word.getByRole("button", { name: "Done" }).click();
    await expect(word).toBeHidden();
    await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
  });

  test("Word count shows 'x of y' for a selection, and Escape closes it", async ({ page }) => {
    await selectWord(page, RICH.last, "paragraph");
    await chooseMenu(page, "Tools", "Word count");
    const word = dialog(page, "Word count");
    await expect(word.locator("dt", { hasText: /^Words$/ }).locator("+ dd")).toHaveText("1 of 98");
    await page.keyboard.press("Escape");
    await expect(word).toBeHidden();
  });

  test("File, Document details shows the name, where it lives and the fidelity", async ({ page }) => {
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    await expect(details).toBeVisible();
    const row = (label: string) => details.locator("dt", { hasText: new RegExp(`^${label}$`) }).locator("+ dd");
    await expect(row("Name")).toHaveText("rich.docx");
    await expect(row("Where")).toHaveText("Your Library");
    await expect(row("Round-trip fidelity")).toHaveText("100.0%");
    await expect(row("Pages")).toHaveText("2");
    await expect(row("Words")).toHaveText("98");
    await expect(row("Fonts")).toContainText("Cambria");
    await expect(row("Size")).toHaveText(/\d+(\.\d)? (kB|KB|byte)/i);
    await details.getByRole("button", { name: "Done" }).click();
    await expect(details).toBeHidden();
  });

  /** A date as the page writes it in a language: what Document details should show. */
  const written = (page: import("./helpers").Page, iso: string, locale: string) =>
    page.evaluate(([i, l]) => new Intl.DateTimeFormat(l, { dateStyle: "long", timeStyle: "short" }).format(new Date(i)), [iso, locale]);

  test("Document details shows when the file was created and last changed, and by whom", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Hello"] }], props: { created: "2024-01-05T10:20:00Z", modified: "2024-02-06T11:21:00Z", lastModifiedBy: "Ben Okafor" } });
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    const row = (label: string) => details.locator("dt", { hasText: new RegExp(`^${label}$`) }).locator("+ dd");
    await expect(row("Created")).toHaveText(await written(page, "2024-01-05T10:20:00Z", "en-US"));
    await expect(row("Last changed")).toHaveText(await written(page, "2024-02-06T11:21:00Z", "en-US"));
    await expect(row("Last changed by")).toHaveText("Ben Okafor");
    // Looking is not changing: the file's dates are as it had them.
    expect(await inPage(page, (w) => w.fe.editor.engine!.docProps().modified)).toBe("2024-02-06T11:21:00Z");
  });

  test("a document without dates shows no date rows", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Hello"] }] });
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    await expect(details.locator("dt", { hasText: /^Name$/ })).toBeVisible();
    await expect(details.locator("dt", { hasText: /^(Created|Last changed|Last changed by)$/ })).toHaveCount(0);
  });

  test("opening Document details on an untouched document leaves its dates as they were", async ({ page }) => {
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    await expect(details.locator("dt", { hasText: /^Last changed$/ }).locator("+ dd")).toHaveText(await written(page, "2013-12-23T23:15:00Z", "en-US"));
    expect(await inPage(page, (w) => w.fe.editor.engine!.docProps().modified)).toBe("2013-12-23T23:15:00Z");
  });

  test("editing sets the last changed date to now and keeps the created date", async ({ page }) => {
    // rich.docx says it was made and changed in 2013.
    expect(await inPage(page, (w) => w.fe.editor.engine!.docProps().modified)).toBe("2013-12-23T23:15:00Z");
    const before = Date.now();
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "More " });
    });
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    const props = await inPage(page, (w) => w.fe.editor.engine!.docProps());
    expect(props.created).toBe("2013-12-23T23:15:00Z");
    expect(Math.abs(new Date(props.modified!).getTime() - before)).toBeLessThan(60_000);
    const row = (label: string) => details.locator("dt", { hasText: new RegExp(`^${label}$`) }).locator("+ dd");
    await expect(row("Last changed")).toHaveText(await written(page, props.modified!, "en-US"));
    await expect(row("Created")).toHaveText(await written(page, "2013-12-23T23:15:00Z", "en-US"));
  });

  test("a new document gets both dates when it is saved", async ({ page }) => {
    await page.goto("/?open=simple.docx");
    await documentReady(page);
    const props = await inPage(page, (w) => {
      const eng = w.fe.editor.engine!;
      const Engine = eng.constructor as unknown as { blank(name: string): typeof eng };
      const fresh = Engine.blank("New.docx");
      const reopened = fresh.save();
      return { before: fresh.docProps(), reopened: reopened.length };
    });
    expect(props.reopened).toBeGreaterThan(0);
    expect(props.before.created).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
    expect(props.before.modified).toBe(props.before.created);
  });

  test("the dates are written in the interface language", async ({ page }) => {
    await page.goto("/?open=simple.docx&lang=es");
    await documentReady(page);
    await page.evaluate(async (b) => {
      const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(new Uint8Array(b), "built.docx");
    }, Array.from(buildDocx({ sections: [{ paragraphs: ["Hola"] }], props: { created: "2024-01-05T10:20:00Z", modified: "2024-02-06T11:21:00Z" } })));
    await expect(page.locator(".flow .fp").first()).toContainText("Hola");
    await inPage(page, (w) => (w.fe.extensions as unknown as { execute(id: string): void }).execute("file.details"));
    const dates = page.locator(".doc-details dd", { hasText: /2024/ });
    await expect(dates).toHaveCount(2);
    await expect(dates.first()).toHaveText(await written(page, "2024-01-05T10:20:00Z", "es"));
    await expect(dates.first()).toContainText("enero");
    await expect(dates.nth(1)).toContainText("febrero");
  });

  test("Format, Page orientation switches to Landscape and back to Portrait", async ({ page }) => {
    const sheet = page.locator(".sheet").first();
    // The page is scaled to fit the window, so compare its shape rather than its pixels.
    const shape = async () => {
      const b = (await sheet.boundingBox())!;
      return Math.round((b.width / b.height) * 100) / 100;
    };
    const portrait = await shape();
    expect(portrait).toBeLessThan(1);

    await chooseMenu(page, "Format", "Page orientation", "Landscape");
    await expect.poll(shape).toBeCloseTo(Math.round((1 / portrait) * 100) / 100, 1);
    expect(await shape()).toBeGreaterThan(1);
    expect((await inPage(page, (w) => w.engine!.pageSetup())).width).toBeGreaterThan(15000);

    // The menu shows which one is current.
    await page.getByRole("menuitem", { name: "Format", exact: true }).click();
    await page.getByRole("menuitem", { name: "Page orientation" }).click();
    await expect(page.getByRole("menuitemcheckbox", { name: /^Landscape/ })).toHaveAttribute("aria-checked", "true");
    await expect(page.getByRole("menuitemcheckbox", { name: /^Portrait/ })).toHaveAttribute("aria-checked", "false");
    await page.getByRole("menuitemcheckbox", { name: /^Portrait/ }).click();

    await expect.poll(shape).toBe(portrait);
    expect((await inPage(page, (w) => w.engine!.pageSetup())).width).toBe(12240);
  });

  test("File, Page setup applies a paper size", async ({ page }) => {
    await chooseMenu(page, "File", "Page setup…");
    const setup = dialog(page, "Page setup");
    await expect(setup).toBeVisible();
    await setup.getByRole("tab", { name: "Paper" }).click();
    await expect(setup.getByLabel("Paper size", { exact: true })).toHaveValue("12240x15840");
    await setup.getByLabel("Paper size", { exact: true }).selectOption({ label: "A4 (210 × 297 mm)" });
    await setup.getByRole("button", { name: "Apply" }).click();
    await expect(setup).toBeHidden();
    await expect.poll(() => inPage(page, (w) => w.engine!.pageSetup().width)).toBe(11906);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => inPage(page, (w) => w.engine!.pageSetup().width)).toBe(12240);
  });
});

// Rule U6 (architecture review): after a dialog closes, the keyboard is back where it was, never on the page body.
test("a dialog that closes with Escape gives the keyboard back to the document", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await page.locator(".flow .fp").first().click();
  for (const id of ["format.bordersShading", "insert.pageNumberOptions", "watermark.remove"]) {
    await inPage(page, (w, i) => void w.fe.extensions.execute(i), id);
    await expect(page.getByRole("dialog").or(page.getByRole("alertdialog"))).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog").or(page.getByRole("alertdialog"))).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => document.activeElement?.closest(".flow") !== null)).toBe(true);
  }
});
