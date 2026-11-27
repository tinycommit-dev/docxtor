// Download as OpenDocument text, Rich Text, a web page and an EPUB book: each is its own extension.
import { readFile } from "node:fs/promises";
import { chooseMenu, dialog, expect, menuItem, openFixture, openMenu, test, topMenu, type Page } from "./helpers";

const FORMATS = [
  { label: "OpenDocument text (.odt)", ext: "odt", id: "docxtor.format.odt", name: "OpenDocument text" },
  { label: "Rich Text (.rtf)", ext: "rtf", id: "docxtor.format.rtf", name: "Rich Text" },
  { label: "Web page (.html)", ext: "html", id: "docxtor.format.html", name: "Web page" },
  { label: "EPUB book (.epub)", ext: "epub", id: "docxtor.format.epub", name: "EPUB book" },
] as const;

/** Choose File, Download, then a format, and read the file the browser saves. */
async function download(page: Page, label: string) {
  const [file] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", label)]);
  return { name: file.suggestedFilename(), bytes: await readFile((await file.path())!) };
}

async function toggleExtension(page: Page, id: string) {
  await openMenu(page, "Extensions");
  await menuItem(page, "Manage extensions…").click();
  const ext = dialog(page, "Extensions");
  await ext.locator(`.switch[data-ext="${id}"]`).click();
  await ext.getByRole("button", { name: "Done" }).click();
  await expect(ext).toHaveCount(0);
}

test.describe("more download formats", () => {
  test("File, Download lists the four formats after Word, Markdown, plain text, PDF and PDF/A, then the template and the whole Library", async ({ page }) => {
    await openFixture(page);
    await openMenu(page, "File");
    await menuItem(page, "Download").click();
    const lines = page.locator(".menu-drop").nth(1).locator('[role="menuitem"] .menu-label');
    await expect(lines).toHaveText(["Word document (.docx)", "Markdown (.md)", "Plain text (.txt)", "PDF document (.pdf)", "PDF/A archive (.pdf)", ...FORMATS.map((f) => f.label), "Word template (.dotx)", "Download all documents"]);
  });

  test("a web page download is one HTML file with the text, the table and the picture", async ({ page }) => {
    await openFixture(page);
    const { name, bytes } = await download(page, "Web page (.html)");
    expect(name).toMatch(/\.html$/);
    const html = bytes.toString("utf8");
    expect(html.startsWith("<!doctype html")).toBe(true);
    expect(html).toContain("<title>Docxtor fixture</title>");
    expect(html).toContain("Quarterly Product Report");
    expect(html).toContain("<table");
    expect(html).toContain("src=\"data:image/png;base64,");
  });

  test("a Rich Text download starts with the RTF header and holds the text", async ({ page }) => {
    await openFixture(page);
    const { name, bytes } = await download(page, "Rich Text (.rtf)");
    expect(name).toMatch(/\.rtf$/);
    const rtf = bytes.toString("latin1");
    expect(rtf.startsWith("{\\rtf1")).toBe(true);
    expect(rtf).toContain("Quarterly Product Report");
    expect(rtf).toContain("\\trowd");
  });

  for (const [label, ext, mime] of [
    ["OpenDocument text (.odt)", "odt", "application/vnd.oasis.opendocument.text"],
    ["EPUB book (.epub)", "epub", "application/epub+zip"],
  ] as const) {
    test(`a .${ext} download is a zip that starts with its mimetype file`, async ({ page }) => {
      await openFixture(page);
      const { name, bytes } = await download(page, label);
      expect(name).toMatch(new RegExp(`\\.${ext}$`));
      // A zip, whose first entry is "mimetype", stored, holding the media type.
      expect([...bytes.subarray(0, 4)]).toEqual([0x50, 0x4b, 0x03, 0x04]);
      expect(bytes.subarray(30, 38).toString("latin1")).toBe("mimetype");
      expect(bytes.subarray(38, 38 + mime.length).toString("latin1")).toBe(mime);
    });
  }

  test("each format is an extension in the Extensions dialog", async ({ page }) => {
    await openFixture(page);
    await openMenu(page, "Extensions");
    await menuItem(page, "Manage extensions…").click();
    const ext = dialog(page, "Extensions");
    for (const f of FORMATS) {
      const row = ext.locator(".ext-row").filter({ has: page.locator(`.switch[data-ext="${f.id}"]`) });
      await expect(row.locator(".ext-name")).toContainText(f.name);
      await expect(row.locator(".ext-adds")).toHaveText("Adds 1 export format");
    }
  });

  test("turning a format's extension off removes it from the menu, and on brings it back", async ({ page }) => {
    await openFixture(page);
    await toggleExtension(page, "docxtor.format.epub");
    await openMenu(page, "File");
    await menuItem(page, "Download").click();
    const lines = page.locator(".menu-drop").nth(1).locator('[role="menuitem"] .menu-label');
    await expect(lines).toHaveText(["Word document (.docx)", "Markdown (.md)", "Plain text (.txt)", "PDF document (.pdf)", "PDF/A archive (.pdf)", "OpenDocument text (.odt)", "Rich Text (.rtf)", "Web page (.html)", "Word template (.dotx)", "Download all documents"]);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await expect(topMenu(page, "File")).toHaveAttribute("aria-expanded", "false");

    await toggleExtension(page, "docxtor.format.epub");
    await openMenu(page, "File");
    await menuItem(page, "Download").click();
    await expect(menuItem(page, "EPUB book (.epub)")).toBeVisible();
  });

  test("Download a copy in the command palette offers the new formats", async ({ page }) => {
    await openFixture(page);
    await page.keyboard.press("ControlOrMeta+Shift+P");
    await page.keyboard.type("Download a copy");
    await page.keyboard.press("Enter");
    for (const f of FORMATS) await expect(page.getByRole("option", { name: new RegExp(f.name) })).toBeVisible();
    const [file] = await Promise.all([page.waitForEvent("download"), page.getByRole("option", { name: /Web page/ }).click()]);
    expect(file.suggestedFilename()).toMatch(/\.html$/);
  });
});
