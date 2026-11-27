// Bookmarks in headers, footers and notes: a ribbon beside them on every page that shows them, a line in the
// Bookmarks dialog that says where they are, and Go to, which opens that header, footer or note and selects the
// bookmark. Bookmarks in the text are covered in marks.spec.ts.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

const lines = (n: number) => Array.from({ length: n }, (_, i) => `Line ${i + 1}`);

/** A three-page document with a header, a footer and a footnote, each holding a bookmark, and one bookmark in the text. */
async function openWithBookmarks(page: Page) {
  await openBuilt(page, { sections: [{ headers: { default: "Running head" }, footers: { default: "Closing words" }, paragraphs: ["Alpha beta gamma", ...lines(110)] }] });
  await inPage(page, (w) => {
    const ed = w.fe.editor;
    const setup = ed.engine!.pageSetup() as unknown as { headers: Record<string, { blocks: { id: number }[] }>; footers: Record<string, { blocks: { id: number }[] }> };
    const head = setup.headers.default.blocks[0].id;
    const foot = setup.footers.default.blocks[0].id;
    const first = ed.engine!.renderAll().order[0];
    ed.exec([
      { op: "insert_bookmark", from: { p: head, o: 0 }, to: { p: head, o: 7 }, name: "Head_mark" },
      { op: "insert_bookmark", from: { p: foot, o: 8 }, to: { p: foot, o: 13 }, name: "Foot_mark" },
      { op: "insert_bookmark", from: { p: first, o: 0 }, to: { p: first, o: 5 }, name: "Body_mark" },
    ]);
    const res = ed.exec({ op: "insert_footnote", at: { p: first, o: 16 }, text: "Note text here" }) as { caret?: { p: number; o: number } }[];
    const at = res[0].caret!;
    ed.exec({ op: "insert_bookmark", from: { p: at.p, o: at.o - 4 }, to: { p: at.p, o: at.o }, name: "Note_mark" });
  });
}

const mark = (page: Page, name: string) => page.locator(`.bm-mark[data-name="${name}"]`);
const selected = (page: Page) => page.evaluate(() => window.getSelection()?.toString() ?? "");
const row = (page: Page, name: string) => dialog(page, "Bookmarks").locator(".bm-row").filter({ has: page.locator(".bm-name", { hasText: new RegExp(`^${name}$`) }) });
const openDialog = async (page: Page) => {
  await chooseMenu(page, "Insert", "Bookmark", "Bookmarks…");
  await expect(dialog(page, "Bookmarks")).toBeVisible();
};

test.describe("bookmarks in headers, footers and notes", () => {
  test.beforeEach(async ({ page }) => {
    await openWithBookmarks(page);
  });

  test("a ribbon marks them on every page that shows them, in the header and footer areas and in the note area", async ({ page }) => {
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBeGreaterThanOrEqual(3);
    await expect(mark(page, "Head_mark")).toHaveCount(pages);
    await expect(mark(page, "Foot_mark")).toHaveCount(pages);
    // The footnote is on one page only; the bookmark in the text has one ribbon.
    await expect(mark(page, "Note_mark")).toHaveCount(1);
    await expect(mark(page, "Body_mark")).toHaveCount(1);
    // Each ribbon is beside its area on its own sheet, in the left margin.
    const where = await page.evaluate(() => {
      const sheet = document.querySelector(".sheet")!.getBoundingClientRect();
      const hdr = document.querySelector(".sheet .hdr")!.getBoundingClientRect();
      const m = document.querySelector('.bm-mark[data-name="Head_mark"]')!.getBoundingClientRect();
      return { insideSheet: m.left >= sheet.left && m.right <= hdr.left, level: Math.abs(m.top + m.height / 2 - (hdr.top + 8)) < 14 };
    });
    expect(where).toEqual({ insideSheet: true, level: true });
    // Clicking one opens the bookmark's popover, as for a bookmark in the text.
    await mark(page, "Head_mark").first().click();
    await expect(page.locator(".bm-pop")).toBeVisible();
    await expect(page.locator(".bm-pop .bm-input")).toHaveValue("Head_mark");
    await page.keyboard.press("Escape");
  });

  test("the Bookmarks dialog lists them with where they are", async ({ page }) => {
    await openDialog(page);
    await expect(row(page, "Body_mark").locator(".bm-where")).toHaveCount(0);
    await expect(row(page, "Head_mark").locator(".bm-where")).toHaveText("Header");
    await expect(row(page, "Foot_mark").locator(".bm-where")).toHaveText("Footer");
    await expect(row(page, "Note_mark").locator(".bm-where")).toHaveText("Footnote 1");
    for (const name of ["Head_mark", "Foot_mark", "Note_mark", "Body_mark"]) await expect(row(page, name).getByRole("button", { name: "Go to" })).toBeEnabled();
  });

  test("Go to opens the header and selects the bookmark; then the footer, the note and the text", async ({ page }) => {
    await openDialog(page);
    await row(page, "Head_mark").getByRole("button", { name: "Go to" }).click();
    await expect(dialog(page, "Bookmarks")).toHaveCount(0);
    await expect(page.locator(".story-strip")).toContainText("Header on page 1");
    expect(await selected(page)).toBe("Running");
    await expect(page.locator(".story-edit")).toHaveText("Running head");

    // From inside a header to the footer: that opens instead.
    await openDialog(page);
    await row(page, "Foot_mark").getByRole("button", { name: "Go to" }).click();
    await expect(page.locator(".story-strip")).toContainText("Footer on page 1");
    expect(await selected(page)).toBe("words");

    await openDialog(page);
    await row(page, "Note_mark").getByRole("button", { name: "Go to" }).click();
    await expect(page.locator(".story-strip")).toContainText("Footnotes on page 1");
    expect(await selected(page)).toBe("here");

    // And back to the text: the editing area closes and the bookmark is selected there.
    await openDialog(page);
    await row(page, "Body_mark").getByRole("button", { name: "Go to" }).click();
    await expect(page.locator(".story-strip")).toBeHidden();
    expect(await selected(page)).toBe("Alpha");
  });

  test("a bookmark can be renamed and removed from the dialog like any other", async ({ page }) => {
    await openDialog(page);
    await row(page, "Head_mark").getByRole("button", { name: "Rename" }).click();
    await dialog(page, "Bookmarks").locator(".bm-input").fill("Masthead");
    await dialog(page, "Bookmarks").locator(".bm-row.editing .btn.primary").click();
    await expect(row(page, "Masthead").locator(".bm-where")).toHaveText("Header");
    await row(page, "Masthead").getByRole("button", { name: "Remove" }).click();
    await expect(row(page, "Masthead")).toHaveCount(0);
    await dialog(page, "Bookmarks").getByRole("button", { name: "Done" }).click();
    await expect(mark(page, "Masthead")).toHaveCount(0);
    await expect(mark(page, "Head_mark")).toHaveCount(0);
  });
});
