// Citation styles people add themselves: from a .csl file or an address, kept across reloads, removable.
import { openBuilt } from "./support/docx-builder";
import { answer, caretAtEnd, chooseMenu, dialog, expect, inPage, type Page, test } from "./helpers";

type Eng = { citationStyle(): string; renderAll(): { order: number[] } };

const panel = (page: Page) => page.locator(".cite-panel");
const item = (page: Page, title: string) => panel(page).locator(".src-item").filter({ hasText: title });
const cite = (page: Page) => page.locator(".flow .cite");
const styleSelect = (page: Page) => panel(page).getByLabel("Citation style");
const style = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).citationStyle());

const MINE = `<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><title>My Brackets</title><id>http://example.com/my-brackets</id><category citation-format="numeric"/></info>
  <citation><layout prefix="(" suffix=")"><text variable="citation-number"/></layout></citation>
  <bibliography><layout><text variable="citation-number" suffix=". "/><text variable="title"/></layout></bibliography>
</style>`;

async function start(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Citing is a habit.", "Another paragraph follows."] }] });
  const [first] = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
  await caretAtEnd(page, first);
}

async function openPanel(page: Page) {
  await chooseMenu(page, "Tools", "Citations");
  await expect(panel(page)).toBeVisible();
}

async function addBook(page: Page) {
  await panel(page).getByRole("button", { name: "Fill in a form…" }).click();
  const f = dialog(page, "Add a source");
  await f.getByLabel("Title", { exact: true }).fill("The Art of Citing");
  await f.getByLabel("Family name").first().fill("Roe");
  await f.getByLabel("Given names").first().fill("Jane");
  await f.getByLabel("Year").fill("2019");
  await f.getByLabel("Publisher").fill("Penguin");
  await f.getByRole("button", { name: "Add source" }).click();
  await expect(f).toHaveCount(0);
}

async function addStyleFromFile(page: Page, csl = MINE) {
  await styleSelect(page).selectOption({ label: "Add a style from a CSL file…" });
  const d = dialog(page, "Add a citation style");
  await d.getByLabel("Style file").setInputFiles({ name: "mine.csl", mimeType: "text/xml", buffer: Buffer.from(csl) });
  return d;
}

/** Open the same document again after a reload. */
async function reopen(page: Page, bytes: number[]) {
  await page.reload();
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: { fe?: unknown } }).docxtor?.fe);
  await inPage(page, (w, b) => w.fe.open(new Uint8Array(b), "again.docx"), bytes);
}

test.describe("your own styles", () => {
  test("adds a style from a file, lists it under Your styles and uses it", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    const d = await addStyleFromFile(page);
    await expect(d).toHaveCount(0);
    await expect(styleSelect(page).locator('optgroup[label="Your styles"] option')).toHaveText(["My Brackets"]);
    await styleSelect(page).selectOption({ label: "My Brackets" });
    await expect(cite(page)).toHaveText("(1)");
    expect(await style(page)).toMatch(/^user-my-brackets/);
    await chooseMenu(page, "Insert", "Bibliography");
    await expect(page.locator(".flow .fc")).toContainText("1. The Art of Citing");
  });

  test("says plainly when the file or address is not a usable style", async ({ page }) => {
    await page.route("https://styles.example.org/**", (route) =>
      route.request().url().endsWith("missing.csl") ? route.fulfill({ status: 404, headers: { "access-control-allow-origin": "*" }, body: "" }) : route.fulfill({ status: 200, headers: { "access-control-allow-origin": "*" }, contentType: "text/xml", body: MINE }),
    );
    await start(page);
    await openPanel(page);
    const d = await addStyleFromFile(page, "<html><body>no</body></html>");
    await expect(d.getByRole("alert")).toContainText("That is not a CSL style");
    await expect(styleSelect(page).locator('optgroup[label="Your styles"]')).toHaveCount(0);

    await d.getByLabel("Address of a .csl file").fill("not an address");
    await d.getByRole("button", { name: "Add from address" }).click();
    await expect(d.getByRole("alert")).toContainText("doesn't look like a web address");
    await d.getByLabel("Address of a .csl file").fill("https://styles.example.org/missing.csl");
    await d.getByRole("button", { name: "Add from address" }).click();
    await expect(d.getByRole("alert")).toContainText("didn't answer (error 404)");
    await expect(d).toContainText("Docxtor fetches this address from your browser");
    await d.getByLabel("Address of a .csl file").fill("https://styles.example.org/mine.csl");
    await d.getByRole("button", { name: "Add from address" }).click();
    await expect(d).toHaveCount(0);
    await expect(styleSelect(page).locator('optgroup[label="Your styles"] option')).toHaveText(["My Brackets"]);
  });

  test("a reload keeps the style; Manage styles removes it, and a document that uses it says it isn't available", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await addStyleFromFile(page);
    await styleSelect(page).selectOption({ label: "My Brackets" });
    await expect(cite(page)).toHaveText("(1)");
    const bytes = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));

    await reopen(page, bytes);
    await openPanel(page);
    await expect(styleSelect(page).locator('optgroup[label="Your styles"] option')).toHaveText(["My Brackets"]);
    await expect(styleSelect(page)).toHaveValue(/^user-my-brackets/);
    await expect(cite(page)).toHaveText("(1)");

    await styleSelect(page).selectOption({ label: "Manage styles…" });
    const m = dialog(page, "Your styles");
    await expect(m.locator(".src-item")).toHaveCount(1);
    await m.getByRole("button", { name: "Remove My Brackets" }).click();
    await answer(page, "Remove");
    await expect(m).toContainText("You haven't added any styles.");
    await m.getByRole("button", { name: "Done" }).click();
    await expect(styleSelect(page).locator('optgroup[label="Your styles"]')).toHaveCount(0);
    await expect(styleSelect(page).locator("option", { hasText: "Manage styles…" })).toHaveCount(0);
    // The text stays as it was; the picker says the style is gone.
    await expect(cite(page)).toHaveText("(1)");
    await expect(styleSelect(page).locator("option:checked")).toContainText("(not available)");

    await reopen(page, bytes);
    await openPanel(page);
    await expect(styleSelect(page).locator('optgroup[label="Your styles"]')).toHaveCount(0);
    await expect(cite(page)).toHaveText("(1)");
    await expect(styleSelect(page).locator("option:checked")).toContainText("(not available)");
  });
});
