// Insert, Pages: the cover page gallery (pictures of the real designs), a cover page on page 1 with no header or page
// number whose boxes are the document's properties, replacing and removing it, a saved cover page, Blank page; and the
// header and footer galleries.
import { openBuilt } from "./support/docx-builder";
import { answer, chooseMenu, dialog, expect, inPage, LINE_END, LINE_START, SELECT_TO_LINE_END, test, type Page } from "./helpers";

const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
/** The text of the paragraphs on page k (the text runs over the sheets; a sheet holds its header and footer). */
const pageText = (page: Page, k: number) =>
  page.evaluate((k) => {
    // The rectangles are read on purpose: with far pages left out the pages are drawn first.
    const ed = (window as unknown as { docxtor: { fe: { editor: { withDrawn<T>(fn: () => T): T } } } }).docxtor.fe.editor;
    return ed.withDrawn(() => {
      const s = document.querySelectorAll(".sheet")[k]!.getBoundingClientRect();
      return [...document.querySelectorAll(".flow [data-p]")]
        .filter((p) => {
          const r = p.getBoundingClientRect();
          return r.height > 0 && r.top + r.height / 2 > s.top && r.top + r.height / 2 < s.bottom;
        })
        .map((p) => p.textContent)
        .join("\n");
    });
  }, k);
const lines = (n: number) => Array.from({ length: n }, (_, i) => `Line ${i + 1} of the report`);

async function setTitle(page: Page) {
  await inPage(page, (w) => w.fe.editor.exec({ op: "set_doc_props", title: "Field notes", author: "Ada Lovelace", subject: "A season on the coast" }));
}

async function openGallery(page: Page) {
  await chooseMenu(page, "Insert", "Pages", "Cover page…");
  const box = dialog(page, "Cover page");
  await expect(box.locator(".pg-tile")).toHaveCount(9);
  // The pictures are the real designs, laid out.
  await expect(box.locator(".pg-page.drawn")).toHaveCount(9, { timeout: 15000 });
  return box;
}

test.describe("cover pages", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Introduction", ...lines(4)] }] });
    await setTitle(page);
    await chooseMenu(page, "Insert", "Page elements", "Header…");
    await dialog(page, "Header").getByRole("option", { name: "Title and page number" }).click();
  });

  test("the gallery shows each design drawn in the document's colours, and one goes on page 1 without a header", async ({ page }) => {
    const box = await openGallery(page);
    await expect(box.locator(".pg-page.drawn").first()).toContainText("Field notes");
    await page.waitForTimeout(400);
    await page.screenshot({ path: "test-results/cover-gallery.png" });
    await box.getByRole("option", { name: "Band" }).click();
    await expect(page.locator(".sheet")).toHaveCount(2);
    await expect.poll(() => pageText(page, 0)).toContain("Field notes");
    expect(await pageText(page, 0)).toContain("A season on the coast");
    expect(await pageText(page, 0)).toContain("Ada Lovelace");
    // No header on the cover; the next page has it.
    await expect(sheet(page, 0).locator(".hdr", { hasText: "Field notes" })).toHaveCount(0);
    await expect(sheet(page, 1).locator(".hdr")).toContainText("Field notes");
    expect(await pageText(page, 1)).toContain("Introduction");
    await page.screenshot({ path: "test-results/cover-band.png" });
  });

  test("a new cover replaces the old one, Remove takes it away, and undo brings it back", async ({ page }) => {
    let box = await openGallery(page);
    await box.getByRole("option", { name: "Simple" }).click();
    box = await openGallery(page);
    await box.getByRole("option", { name: "Academic" }).click();
    await expect(page.locator(".sheet")).toHaveCount(2);
    expect(await inPage(page, (w) => w.fe.editor.engine!.docPartControls().filter((c) => c.gallery === "Cover Pages").length)).toBe(1);
    await chooseMenu(page, "Insert", "Pages", "Remove current cover page");
    await answer(page, "Remove cover page");
    await expect(page.locator(".sheet")).toHaveCount(1);
    expect(await pageText(page, 0)).toContain("Introduction");
    await expect(sheet(page, 0).locator(".hdr")).toContainText("Field notes");
    await page.keyboard.press("ControlOrMeta+z");
    await expect(page.locator(".sheet")).toHaveCount(2);
  });

  test("typing in the title box changes the document's title, and the header follows", async ({ page }) => {
    const box = await openGallery(page);
    await box.getByRole("option", { name: "Simple" }).click();
    const title = page.locator(".flow [data-p]", { hasText: "Field notes" }).first();
    // The title is large enough to wrap; type at its start.
    await title.click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Lab ");
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.docProps().title)).toBe("Lab Field notes");
    await expect(sheet(page, 1).locator(".hdr")).toContainText("Lab Field notes");
  });

  test("a cover page saved from a selection is in the gallery and goes in as a cover", async ({ page }) => {
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.press(SELECT_TO_LINE_END);
    await chooseMenu(page, "Insert", "Pages", "Save selection to cover page gallery…");
    const save = dialog(page, "Save selection to Quick parts");
    await save.getByLabel("Name").fill("My plain cover");
    await save.getByRole("button", { name: "Save" }).click();
    const box = await openGallery(page).catch(async () => dialog(page, "Cover page"));
    await box.getByRole("option", { name: "My plain cover" }).click();
    expect(await inPage(page, (w) => w.fe.editor.engine!.hasCoverPage())).toBe(true);
    await expect(page.locator(".sheet")).toHaveCount(2);
  });

  test("Blank page puts an empty page at the caret", async ({ page }) => {
    await page.locator(".flow [data-p]").nth(2).click();
    await page.keyboard.press(LINE_END);
    await chooseMenu(page, "Insert", "Pages", "Blank page");
    await expect(page.locator(".sheet")).toHaveCount(3);
    expect(await pageText(page, 1)).not.toContain("Line");
  });
});

test.describe("header and footer galleries", () => {
  test("each header and footer design is drawn, and one fills the header of every page", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: lines(70) }] });
    await setTitle(page);
    await chooseMenu(page, "Insert", "Page elements", "Header…");
    let box = dialog(page, "Header");
    await expect(box.locator(".pg-page.drawn")).toHaveCount(6, { timeout: 15000 });
    await page.waitForTimeout(400);
    await page.screenshot({ path: "test-results/header-gallery.png" });
    await box.getByRole("option", { name: "Title and date" }).click();
    await expect(sheet(page, 0).locator(".hdr")).toContainText("Field notes");
    await expect(sheet(page, 1).locator(".hdr")).toContainText("Field notes");
    await chooseMenu(page, "Insert", "Page elements", "Footer…");
    box = dialog(page, "Footer");
    await expect(box.locator(".pg-page.drawn")).toHaveCount(6, { timeout: 15000 });
    await box.getByRole("option", { name: "Page number", exact: true }).click();
    await expect(sheet(page, 1).locator(".ftr")).toContainText("Page 2 of");
    await chooseMenu(page, "Insert", "Page elements", "Remove footer");
    await answer(page, "Remove footer");
    await expect(sheet(page, 1).locator(".ftr", { hasText: "Page 2" })).toHaveCount(0);
  });

  test("the gallery is a listbox the keyboard can move through", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Text"] }] });
    await chooseMenu(page, "Insert", "Page elements", "Footer…");
    const box = dialog(page, "Footer");
    await expect(box.getByRole("option").first()).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(box.getByRole("option").nth(1)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(box).toHaveCount(0);
    await expect(sheet(page, 0).locator(".ftr")).toContainText("[Type here]");
  });
});

/** Double-click in the header area at the top of the first sheet (the header itself takes no clicks): starts editing it. */
const editHeader = async (page: Page) => {
  const zoom = await inPage(page, (w) => w.fe.editor.zoom);
  const b = (await sheet(page, 0).boundingBox())!;
  await page.mouse.dblclick(b.x + b.width / 2, b.y + 20 * zoom);
  await expect(page.locator(".story-edit")).toBeVisible();
};

test.describe("typing in the boxes of a header design", () => {
  test("typing in a box takes the placeholder away and leaves only what was typed", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Introduction"] }] });
    await chooseMenu(page, "Insert", "Page elements", "Header…");
    await dialog(page, "Header").getByRole("option", { name: "Blank", exact: true }).click();
    await expect(sheet(page, 0).locator(".hdr")).toContainText("[Type here]");
    await editHeader(page);
    await page.locator(".story-edit [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Quarterly report");
    await expect(page.locator(".story-edit")).toHaveText("Quarterly report");
    await page.keyboard.press("Escape");
    await expect(sheet(page, 0).locator(".hdr")).toHaveText("Quarterly report");
  });

  test("typing in a header's Title box changes the document's title", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Introduction"] }] });
    await setTitle(page);
    await chooseMenu(page, "Insert", "Page elements", "Header…");
    await dialog(page, "Header").getByRole("option", { name: "Title and page number" }).click();
    await expect(sheet(page, 0).locator(".hdr")).toContainText("Field notes");
    await editHeader(page);
    await page.locator(".story-edit [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Lab ");
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.docProps().title)).toBe("Lab Field notes");
    await page.keyboard.press("Escape");
    await expect(sheet(page, 0).locator(".hdr")).toContainText("Lab Field notes");
    // Undo (a word at a time) gives the old title back, in the property and in the box.
    for (let i = 0; i < 4 && (await inPage(page, (w) => w.fe.editor.engine!.docProps().title)) !== "Field notes"; i++) {
      await page.keyboard.press("ControlOrMeta+z");
    }
    expect(await inPage(page, (w) => w.fe.editor.engine!.docProps().title)).toBe("Field notes");
    await expect(sheet(page, 0).locator(".hdr")).toContainText("Field notes");
    await expect(sheet(page, 0).locator(".hdr")).not.toContainText("Lab");
  });
});

test.describe("a centred cover in a document with columns", () => {
  test("the text starts on its own page, in columns, with the section break of the cover seen by the layout", async ({ page, browserName }) => {
    await openBuilt(page, { sections: [{ cols: { num: 2 }, paragraphs: ["Introduction", ...lines(6)] }] });
    await setTitle(page);
    await chooseMenu(page, "Insert", "Pages", "Cover page…");
    const box = dialog(page, "Cover page");
    await expect(box.locator(".pg-page.drawn")).toHaveCount(9, { timeout: 15000 });
    await box.getByRole("option", { name: "Centered" }).click();
    await expect(page.locator(".sheet")).toHaveCount(2);
    expect(await pageText(page, 0)).toContain("Field notes");
    expect(await pageText(page, 0)).not.toContain("Introduction");
    expect(await pageText(page, 1)).toContain("Introduction");
    // The cover is not in a column wrapper; the text after it is, as its own section's.
    const wrappers = await page.evaluate(() => [...document.querySelectorAll(".flow > .cols")].map((w) => ({ sec: (w as HTMLElement).dataset.sec, text: w.textContent ?? "" })));
    // (Where the browser can't wrap rows of columns, only Chromium so far, the section shows as one column and has no wrapper.)
    if (browserName !== "chromium") return void expect(wrappers).toEqual([]);
    expect(wrappers.length).toBe(1);
    expect(wrappers[0].sec).toBe("1");
    expect(wrappers[0].text).toContain("Introduction");
    expect(wrappers[0].text).not.toContain("Field notes");
  });
});
