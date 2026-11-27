// A table of contents (and cross-references to a page) shows the page numbers the pages print: i, ii, iii in a
// roman-numbered section, and the restarted 1, 2, 3 after it. The engine's side is tested in Rust.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Page } from "./helpers";

const H1 = '<w:pStyle w:val="Heading1"/>';
const lines = (n: number, prefix = "Line") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);

/** A preface in a lower-roman section (two pages), then chapters that restart at 1. */
async function openBook(page: Page) {
  await openBuilt(page, {
    sections: [
      {
        pageNumbers: { fmt: "lowerRoman" },
        footers: { default: "Page {PAGE}" },
        paragraphs: ["Front matter", { text: "Preface", ppr: H1 }, ...lines(5), { text: "Acknowledgements", ppr: `${H1}<w:pageBreakBefore/>` }, ...lines(5, "Thanks")],
      },
      {
        start: "nextPage",
        pageNumbers: { start: 1 },
        footers: { default: "Page {PAGE}" },
        paragraphs: [{ text: "Chapter one", ppr: H1 }, ...lines(5, "Text"), { text: "Chapter two", ppr: `${H1}<w:pageBreakBefore/>` }, ...lines(5, "More")],
      },
    ],
  });
}

/** The table of contents' entries as text: a heading, a tab, its page number. */
const entries = (page: Page) =>
  inPage(page, (w) =>
    w.fe.editor.engine!
      .text()
      .split("\n")
      .filter((l) => l.includes("\t")),
  );

test.describe("table of contents page numbers", () => {
  test.beforeEach(async ({ page }) => {
    await openBook(page);
  });

  test("a roman-numbered section lists i, ii, and the restarted section lists 1, 2", async ({ page }) => {
    // The pages print what the TOC must say.
    await expect(page.locator(".sheet").nth(0).locator(".ftr")).toHaveText("Page i");
    await expect(page.locator(".sheet").nth(1).locator(".ftr")).toHaveText("Page ii");
    await expect(page.locator(".sheet").nth(2).locator(".ftr")).toHaveText("Page 1");
    await expect(page.locator(".sheet").nth(3).locator(".ftr")).toHaveText("Page 2");
    // Put the table of contents on the front matter page (the caret is in its first paragraph).
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Insert", "Page elements", "Table of contents");
    expect(await entries(page)).toEqual(["Preface\ti", "Acknowledgements\tii", "Chapter one\t1", "Chapter two\t2"]);
  });

  test("Update fields keeps them as the pages print them, and follows the layout when it changes", async ({ page }) => {
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Insert", "Page elements", "Table of contents");
    // Change the format: capitals. The contents still match the pages after Update table of contents.
    const anchor = await inPage(page, (w) => w.fe.editor.sectionAnchor(0));
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_page_numbering", section: p, format: "upperRoman" }), anchor);
    await chooseMenu(page, "Insert", "Page elements", "Update table of contents");
    expect(await entries(page)).toEqual(["Preface\tI", "Acknowledgements\tII", "Chapter one\t1", "Chapter two\t2"]);
    await expect(page.locator(".sheet").nth(1).locator(".ftr")).toHaveText("Page II");
  });
});
