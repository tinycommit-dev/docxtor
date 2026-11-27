// Sections on the page: every page takes the paper and margins of the section that owns it, sections start
// where their break says (a new page, the same page, the next even or odd page), and headers, footers and
// page fields follow the section a page belongs to.
import { buildDocx, openBuilt, type SectionSpec } from "./support/docx-builder";
import { caretAtEnd, chooseMenu, dialog, expect, inPage, openFixture, para, reveal, RICH, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const TW = 1 / 15;

/** Paper, in twips. */
const LETTER: [number, number] = [12240, 15840];
const LANDSCAPE: [number, number] = [15840, 12240];

const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup());
const pageMap = (page: Page) => inPage(page, (w) => w.fe.editor.pageMap());
const zoomOf = (page: Page) => inPage(page, (w) => w.fe.editor.zoom);
const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
const box = async (el: Locator) => (await el.boundingBox())!;
/** The box of a sheet after scrolling it into view (mouse positions are relative to the window). */
const viewSheet = async (page: Page, k: number) => {
  // Its top a little way below the top of the canvas, clear of the sticky ruler.
  await page.evaluate((i) => {
    const c = document.querySelector(".canvas")!;
    c.scrollTop += document.querySelectorAll(".sheet")[i].getBoundingClientRect().top - c.getBoundingClientRect().top - 60;
  }, k);
  return box(sheet(page, k));
};
const near = (a: number, b: number, tolerance = 1) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);
/** The body paragraph that holds this text. */
const withText = (page: Page, text: string) => page.locator(".flow > .fp").filter({ hasText: text });
/** Put the caret in a paragraph (a plain click in its first line). */
const caretIn = async (page: Page, text: string) => {
  const at = withText(page, text).first();
  // With far pages left out (DOCXTOR_FAR) a paragraph far from the view has no box until the view is there.
  await reveal(at);
  await at.click({ position: { x: 24, y: 6 } });
};
const pageOfText = async (page: Page, text: string) => {
  const id = await withText(page, text).first().getAttribute("data-p");
  return (await pageMap(page))[id!];
};
/** The text of a sheet's header or footer, field results included. */
const hf = (page: Page, k: number, zone: "hdr" | "ftr") => sheet(page, k).locator(`.${zone}`).textContent();
const lines = (n: number, prefix = "Line") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);

const portrait = (paragraphs: string[], more: Partial<SectionSpec> = {}): SectionSpec => ({ paragraphs, ...more });
const landscape = (paragraphs: string[], more: Partial<SectionSpec> = {}): SectionSpec => ({ size: LANDSCAPE, paragraphs, ...more });

test.describe("pages take the paper of their section", () => {
  test("a landscape section between portrait ones: sheets, text width and margins follow it", async ({ page }) => {
    // Made through the engine: open a fixture, break it into three sections and turn the middle one around.
    await openFixture(page);
    await caretAtEnd(page, RICH.title);
    await chooseMenu(page, "Insert", "Break", "Section break (next page)");
    await caretAtEnd(page, RICH.intro);
    await chooseMenu(page, "Insert", "Break", "Section break (next page)");
    expect((await setup(page)).sections).toHaveLength(3);
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "set_page_setup", section: id, width: 15840, height: 12240, margins: [1440, 720, 1440, 2880] }), RICH.intro);

    const ps = await setup(page);
    expect(ps.sections.map((s) => s.width)).toEqual([12240, 15840, 12240]);
    // Title, introduction, then the rest (two pages in the document, three sections: 1, 1 and 2 pages).
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBe(4);
    const zoom = await zoomOf(page);
    const s0 = await box(sheet(page, 0));
    const s1 = await box(sheet(page, 1));
    const s2 = await box(sheet(page, 2));
    // Sizes: letter portrait, letter landscape, letter portrait.
    near(s0.width / zoom, 816, 0.5);
    near(s0.height / zoom, 1056, 0.5);
    near(s1.width / zoom, 1056, 0.5);
    near(s1.height / zoom, 816, 0.5);
    near(s2.width / zoom, 816, 0.5);
    // Centred on one another, one gap (28px) apart whatever their height.
    near(s0.x + s0.width / 2, s1.x + s1.width / 2);
    near(s1.x + s1.width / 2, s2.x + s2.width / 2);
    near((s1.y - (s0.y + s0.height)) / zoom, 28, 0.5);
    near((s2.y - (s1.y + s1.height)) / zoom, 28, 0.5);

    // The landscape page's text sits within its own margins: 2 in on the left, half an inch on the right.
    const intro = await box(para(page, RICH.intro));
    near((intro.x - s1.x) / zoom, 192, 0.5);
    near(intro.width / zoom, (15840 - 720 - 2880) * TW, 0.5);
    // And the portrait pages' within theirs (the fixture's side margins are 1.25 in).
    const title = await box(para(page, RICH.title));
    near((title.x - s0.x) / zoom, 120, 0.5);
    near(title.width / zoom, 576, 0.5);
    const highlights = await box(para(page, RICH.highlights));
    near(highlights.x - s2.x - (title.x - s0.x), 0, 1);

    // The stage is as wide as the widest sheet, and the zoom fits that one.
    const stage = await box(page.locator(".stage"));
    near(stage.width / zoom, 1056, 0.5);
    const canvas = await page.locator(".canvas").evaluate((el) => el.clientWidth - 64);
    expect(1056 * zoom).toBeLessThanOrEqual(canvas + 0.5);
  });

  test("margins differ from section to section, and a continuous section can change the side margins part-way down a page", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        portrait(["Alpha paragraph"], { margins: [1440, 1440, 1440, 1440] }),
        portrait(["Beta paragraph"], { start: "continuous", margins: [1440, 2880, 1440, 2880] }),
        portrait(["Gamma paragraph"], { start: "continuous", margins: [1440, 720, 1440, 720] }),
        portrait(["Delta paragraph"], { start: "nextPage", margins: [2160, 1440, 720, 1800] }),
      ],
    });
    // All of the first three share a page; the fourth has a page of its own.
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(2);
    const zoom = await zoomOf(page);
    const s0 = await box(sheet(page, 0));
    const left = async (text: string) => ((await box(withText(page, text))).x - s0.x) / zoom;
    const width = async (text: string) => (await box(withText(page, text))).width / zoom;
    near(await left("Alpha"), 96, 0.5);
    near(await left("Beta"), 192, 0.5);
    near(await left("Gamma"), 48, 0.5);
    near(await width("Alpha"), 624, 0.5);
    near(await width("Beta"), 432, 0.5);
    near(await width("Gamma"), 720, 0.5);
    // They stack down the one page, in order.
    const tops = await Promise.all(["Alpha", "Beta", "Gamma"].map(async (t) => (await box(withText(page, t))).y));
    expect(tops[0]).toBeLessThan(tops[1]);
    expect(tops[1]).toBeLessThan(tops[2]);
    // The fourth section's page has its own margins all round: text 1.25 in from the left, 1.5 in from the top.
    const s1 = await box(sheet(page, 1));
    const delta = await box(withText(page, "Delta"));
    near((delta.x - s1.x) / zoom, 120, 0.5);
    near((delta.y - s1.y) / zoom, 144, 1);
    // One section's own margins don't leak into the next page's blocks: the first page's text still sits at 1in.
    const hdrTop = await page.evaluate(() => getComputedStyle(document.querySelector(".flow")!).paddingTop);
    expect(hdrTop).toBe("96px");
  });

  test("typing in a section with other margins keeps the text where it is", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Alpha paragraph"]), landscape(["Beta paragraph"], { margins: [1440, 720, 1440, 2880] })] });
    const zoom = await zoomOf(page);
    const before = await box(withText(page, "Beta"));
    await caretIn(page, "Beta");
    await page.keyboard.press("End");
    await page.keyboard.type(" and more words typed here");
    await expect(withText(page, "Beta")).toContainText("and more words typed here");
    const after = await box(withText(page, "Beta"));
    near(after.x, before.x);
    near(after.width, before.width);
    near((after.x - (await box(sheet(page, 1))).x) / zoom, 192, 0.5);
    // And after Enter, the new paragraph is in the same margins.
    await page.keyboard.press("Enter");
    await page.keyboard.type("New paragraph");
    const fresh = await box(withText(page, "New paragraph"));
    near(fresh.x, before.x);
  });

  test("undoing the break goes back to the simple layout: the flow's padding is the margin again", async ({ page }) => {
    await openFixture(page);
    await caretAtEnd(page, RICH.title);
    await chooseMenu(page, "Insert", "Break", "Section break (continuous)");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "set_page_setup", section: id, margins: [1440, 720, 1440, 2880] }), RICH.title);
    // Different side margins in the sections: each block carries its own offsets and the flow has no padding.
    await expect(page.locator(".flow")).toHaveCSS("padding-left", "0px");
    expect(await page.locator(".flow > [data-sk]").count()).toBeGreaterThan(5);
    await page.keyboard.press(`${MOD}+Z`);
    await page.keyboard.press(`${MOD}+Z`);
    expect((await setup(page)).sections).toHaveLength(1);
    await expect(page.locator(".flow")).toHaveCSS("padding-left", "120px");
    expect(await page.locator(".flow > [data-sk]").count()).toBe(0);
    // No leftover margins on the blocks.
    expect(await page.locator(".flow > .fp").evaluateAll((els) => els.filter((e) => (e as HTMLElement).style.marginLeft || (e as HTMLElement).style.marginRight).length)).toBe(0);
  });

  test("a one-section document lays out as it always has: the flow's padding is the margin", async ({ page }) => {
    await openFixture(page);
    const css = await page.locator(".flow").evaluate((el) => {
      const c = getComputedStyle(el);
      return { l: c.paddingLeft, r: c.paddingRight, t: c.paddingTop, w: c.width };
    });
    expect(css).toEqual({ l: "120px", r: "120px", t: "96px", w: "816px" });
    // No block carries side offsets of its own.
    expect(await page.locator(".flow > [data-sk]").count()).toBe(0);
    await expect(page.locator(".stage")).toHaveCSS("width", "816px");
    const sheets = await page.locator(".sheet").evaluateAll((els) => els.map((e) => [(e as HTMLElement).style.left, (e as HTMLElement).style.top, (e as HTMLElement).style.height]));
    expect(sheets).toEqual([
      ["0px", "0px", "1056px"],
      ["0px", "1084px", "1056px"],
    ]);
  });
});

test.describe("how a section starts", () => {
  test("a continuous section stays on the page, a next-page section does not, and new paper starts a new page", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        portrait(["First section"]),
        portrait(["Continuous same paper"], { start: "continuous" }),
        portrait(["Next page section"], { start: "nextPage" }),
        portrait(["Continuous same again"], { start: "continuous" }),
        landscape(["Continuous other paper"], { start: "continuous" }),
      ],
    });
    expect(await pageOfText(page, "First section")).toBe(1);
    expect(await pageOfText(page, "Continuous same paper")).toBe(1);
    expect(await pageOfText(page, "Next page section")).toBe(2);
    expect(await pageOfText(page, "Continuous same again")).toBe(2);
    // Word starts a continuous section on a new page when the paper changes.
    expect(await pageOfText(page, "Continuous other paper")).toBe(3);
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(3);
    near((await box(sheet(page, 2))).width / (await zoomOf(page)), 1056, 0.5);
  });

  test("an odd-page section after page 1 leaves page 2 blank, in the new section's paper", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Opening page"]), landscape(["Starts on an odd page"], { start: "oddPage" })] });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(3);
    expect(await pageOfText(page, "Starts on an odd page")).toBe(3);
    // The blank page belongs to the new section: landscape paper, and no text on it.
    const zoom = await zoomOf(page);
    const blank = await box(sheet(page, 1));
    near(blank.width / zoom, 1056, 0.5);
    near(blank.height / zoom, 816, 0.5);
    const owners = await inPage(page, (w) => [0, 1, 2].map((k) => w.fe.editor.pageBox(k).section));
    expect(owners).toEqual([0, 1, 1]);
    // It is a page like any other: it has its number.
    await expect(sheet(page, 1).locator(".page-num")).toHaveText("2");
  });

  test("an even-page section after page 1 needs no blank page; after page 2 it leaves one", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Opening page"]), portrait(["Starts on an even page"], { start: "evenPage" })] });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(2);
    expect(await pageOfText(page, "Starts on an even page")).toBe(2);

    // Two pages in the first section: the next one would be page 3, odd, so page 3 stays blank.
    await openBuilt(page, { sections: [portrait(lines(110)), portrait(["Starts on an even page"], { start: "evenPage" })] });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(4);
    expect(await pageOfText(page, "Starts on an even page")).toBe(4);
    expect(await pageOfText(page, "Line 110")).toBe(2);
  });

  test("an odd-page section after page 1 and 2 pages needs no blank page", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(lines(110)), portrait(["Starts on an odd page"], { start: "oddPage" })] });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(3);
    expect(await pageOfText(page, "Starts on an odd page")).toBe(3);
  });

  test("the break menu's even-page break and its undo", async ({ page }) => {
    await openFixture(page);
    await caretAtEnd(page, RICH.title);
    await chooseMenu(page, "Insert", "Break", "Section break (odd page)");
    // Title on page 1, so the new section skips page 2.
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBeGreaterThanOrEqual(4);
    await page.keyboard.press(`${MOD}+Z`);
    expect((await setup(page)).sections).toHaveLength(1);
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(2);
  });
});

test.describe("headers and footers follow the section", () => {
  test("each section shows its own, and one without its own shows the one before it", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        portrait(["Section one"], { headers: { default: "Header one" }, footers: { default: "Footer one" } }),
        landscape(["Section two"], { headers: { default: "Header two" }, footers: { default: "Footer two" } }),
        portrait(["Section three"]),
      ],
    });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(3);
    expect(await hf(page, 0, "hdr")).toBe("Header one");
    expect(await hf(page, 1, "hdr")).toBe("Header two");
    // Section three has none of its own: it shows section two's, as in Word.
    expect(await hf(page, 2, "hdr")).toBe("Header two");
    expect(await hf(page, 0, "ftr")).toBe("Footer one");
    expect(await hf(page, 1, "ftr")).toBe("Footer two");
    expect(await hf(page, 2, "ftr")).toBe("Footer two");

    // They sit in the page's own margins: the footer's bottom is 0.5 in above the sheet's.
    const zoom = await zoomOf(page);
    for (const k of [0, 1]) {
      const s = await box(sheet(page, k));
      const f = await box(sheet(page, k).locator(".ftr"));
      near((s.y + s.height - (f.y + f.height)) / zoom, 48, 1);
      const h = await box(sheet(page, k).locator(".hdr"));
      near((h.y - s.y) / zoom, 48, 1);
      near((h.x - s.x) / zoom, 96, 0.5);
    }
  });

  test("a section can show a different first page, and only its first page", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        portrait(lines(110), { headers: { default: "Run one", first: "Title one" } }),
        portrait(lines(110, "More"), { start: "nextPage", titlePage: true, headers: { default: "Run two", first: "Title two" } }),
      ],
    });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(4);
    // Section one doesn't ask for a different first page: it shows its running header throughout.
    expect(await hf(page, 0, "hdr")).toBe("Run one");
    expect(await hf(page, 1, "hdr")).toBe("Run one");
    // Section two does.
    expect(await hf(page, 2, "hdr")).toBe("Title two");
    expect(await hf(page, 3, "hdr")).toBe("Run two");
  });

  test("even-page headers show only when the document asks for them", async ({ page }) => {
    const sections = [portrait(lines(180), { headers: { default: "Odd header", even: "Even header" } })];
    await openBuilt(page, { sections, evenAndOdd: true });
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBeGreaterThanOrEqual(3);
    expect(await hf(page, 0, "hdr")).toBe("Odd header");
    expect(await hf(page, 1, "hdr")).toBe("Even header");
    expect(await hf(page, 2, "hdr")).toBe("Odd header");

    await openBuilt(page, { sections });
    expect(await hf(page, 0, "hdr")).toBe("Odd header");
    expect(await hf(page, 1, "hdr")).toBe("Odd header");
  });

  test("a header taller than its margin pushes that section's text down, and only that section's", async ({ page }) => {
    // Section two's header has a line for every word; its top margin is half an inch, which it does not fit in.
    const tall = Array.from({ length: 12 }, () => "tall").join("{BR}");
    await openBuilt(page, {
      sections: [
        portrait(["Short header section"], { headers: { default: "Short" } }),
        portrait(["Tall header section"], { start: "nextPage", headers: { default: tall }, margins: [720, 1440, 1440, 1440], header: 360 }),
      ],
    });
    const zoom = await zoomOf(page);
    const s0 = await box(sheet(page, 0));
    const s1 = await box(sheet(page, 1));
    // The short header fits its margin: the text starts at the top margin (1 in).
    near(((await box(withText(page, "Short header section"))).y - s0.y) / zoom, 96, 1);
    // The tall one doesn't: the text starts below it, well past the 0.5 in margin it asked for.
    const mT = await inPage(page, (w) => w.fe.editor.pageBox(1).mT);
    expect(mT).toBeGreaterThan(48 + 60);
    near(((await box(withText(page, "Tall header section"))).y - s1.y) / zoom, mT, 1);
    expect(await inPage(page, (w) => w.fe.editor.pageBox(0).mT)).toBe(96);
  });

  test("PAGE counts every page; NUMPAGES the document; SECTIONPAGES the section's own pages", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        portrait(lines(110), { footers: { default: "p {PAGE} of {NUMPAGES}, {SECTIONPAGES} here" } }),
        landscape(["Two"], { start: "nextPage" }),
        portrait(lines(110, "Last"), { start: "nextPage" }),
      ],
    });
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBe(5);
    // The footer is inherited all the way down.
    expect(await hf(page, 0, "ftr")).toBe("p 1 of 5, 2 here");
    expect(await hf(page, 1, "ftr")).toBe("p 2 of 5, 2 here");
    expect(await hf(page, 2, "ftr")).toBe("p 3 of 5, 1 here");
    expect(await hf(page, 3, "ftr")).toBe("p 4 of 5, 2 here");
    expect(await hf(page, 4, "ftr")).toBe("p 5 of 5, 2 here");
  });

  test("page numbers restart in a section and can be roman numerals, from Page number options", async ({ page }) => {
    await openBuilt(page, {
      sections: [portrait(lines(110), { footers: { default: "p {PAGE}" } }), portrait(lines(110, "More"), { start: "nextPage" })],
    });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(4);
    expect(await hf(page, 2, "ftr")).toBe("p 3");
    // The first section counts i, ii.
    await caretIn(page, "Line 1");
    await chooseMenu(page, "Insert", "Page elements", "Page number options…");
    let dlg = dialog(page, "Page numbers");
    await expect(dlg.getByRole("radio", { name: "This section" })).toHaveAttribute("aria-checked", "true");
    await dlg.getByRole("radio", { name: "i, ii, iii", exact: true }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    await expect(dlg).toBeHidden();
    expect([await hf(page, 0, "ftr"), await hf(page, 1, "ftr"), await hf(page, 2, "ftr")]).toEqual(["p i", "p ii", "p 3"]);
    // The second starts again at 1.
    await caretIn(page, "More 1");
    await chooseMenu(page, "Insert", "Page elements", "Page number options…");
    dlg = dialog(page, "Page numbers");
    await dlg.getByRole("radio", { name: "Start at", exact: true }).click();
    await dlg.getByRole("spinbutton", { name: "Start at" }).fill("1");
    await dlg.getByRole("button", { name: "Apply" }).click();
    expect([await hf(page, 2, "ftr"), await hf(page, 3, "ftr")]).toEqual(["p 1", "p 2"]);
    const secs = (await setup(page)).sections;
    expect([secs[0].pageFormat, secs[0].pageStart, secs[1].pageFormat, secs[1].pageStart]).toEqual(["lowerRoman", undefined, undefined, 1]);
    // A table of contents uses the numbers the pages print.
    expect((await pageMap(page))[String(await withText(page, "More 1").first().getAttribute("data-p"))]).toBe(1);
    // Undo goes back to counting on.
    await page.keyboard.press(`${MOD}+z`);
    expect(await hf(page, 2, "ftr")).toBe("p 3");
  });

  test("an odd-page section that restarts at 1 needs no blank page", async ({ page }) => {
    // Page 2 would be even, so the odd-page section leaves it blank and starts on page 3...
    await openBuilt(page, { sections: [portrait(["Cover"]), portrait(["Chapter"], { start: "oddPage", footers: { default: "p {PAGE}" } })] });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(3);
    // ...but numbered from 1 again, the page after the cover is page 1, an odd page.
    const anchor = await inPage(page, (w) => w.fe.editor.sectionAnchor(1));
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_page_numbering", section: p, start: 1 }), anchor);
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBe(2);
    expect(await hf(page, 1, "ftr")).toBe("p 1");
  });

  test("double-clicking in the margin of a later section's page edits that section's header", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        portrait(["Section one"], { headers: { default: "Header one" } }),
        landscape(["Section two"], { start: "nextPage", headers: { default: "Header two" } }),
      ],
    });
    const zoom = await zoomOf(page);
    const s1 = await viewSheet(page, 1);
    // Inside the top margin of the second (landscape) sheet.
    await page.mouse.dblclick(s1.x + s1.width / 2, s1.y + 20 * zoom);
    await expect(page.locator(".story-strip")).toBeVisible();
    await expect(page.locator(".story-edit")).toHaveText("Header two");
    await expect(page.locator(".story-strip")).toContainText("Header on page 2");
    // Typing changes that part and not the other.
    await page.keyboard.press("End");
    await page.keyboard.type(" edited");
    await expect(page.locator(".story-edit")).toHaveText("Header two edited");
    await page.keyboard.press("Escape");
    expect(await hf(page, 1, "hdr")).toBe("Header two edited");
    expect(await hf(page, 0, "hdr")).toBe("Header one");
    const parts = (await setup(page)).sections.map((s) => s.headers.default);
    expect(parts[0]).not.toBe(parts[1]);

    // And the first page's header still edits its own.
    const s0 = await viewSheet(page, 0);
    await page.mouse.dblclick(s0.x + s0.width / 2, s0.y + 20 * zoom);
    await expect(page.locator(".story-edit")).toHaveText("Header one");
    await page.keyboard.press("Escape");
  });

  test("a page of a section that has no header gets a header of its own; the section after keeps its own", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Section one"]), landscape(["Section two"], { start: "nextPage", headers: { default: "Header two" } })] });
    await expect(sheet(page, 0).locator(".hdr")).toHaveCount(0);
    const zoom = await zoomOf(page);
    const s0 = await viewSheet(page, 0);
    await page.mouse.dblclick(s0.x + s0.width / 2, s0.y + 20 * zoom);
    await expect(page.locator(".story-strip")).toBeVisible();
    await expect(page.locator(".story-edit")).toHaveText("");
    await page.keyboard.type("Header one");
    await page.keyboard.press("Escape");
    expect(await hf(page, 0, "hdr")).toBe("Header one");
    expect(await hf(page, 1, "hdr")).toBe("Header two");
    const parts = (await setup(page)).sections.map((s) => s.headers.default);
    expect(parts[0]).toBeDefined();
    expect(parts[0]).not.toBe(parts[1]);
  });

  test("a header added to an earlier section carries on into later sections that have none", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Section one"]), portrait(["Section two"], { start: "nextPage" })] });
    const zoom = await zoomOf(page);
    const s0 = await viewSheet(page, 0);
    await page.mouse.dblclick(s0.x + s0.width / 2, s0.y + 20 * zoom);
    await page.keyboard.type("Shared");
    await page.keyboard.press("Escape");
    expect(await hf(page, 0, "hdr")).toBe("Shared");
    expect(await hf(page, 1, "hdr")).toBe("Shared");
  });

  test("Different first page in the header strip turns it on for the page's own section", async ({ page }) => {
    await openBuilt(page, {
      sections: [portrait(lines(110), { headers: { default: "Run one" } }), portrait(lines(110, "More"), { start: "nextPage", headers: { default: "Run two" } })],
    });
    const zoom = await zoomOf(page);
    const s2 = await viewSheet(page, 2);
    await page.mouse.dblclick(s2.x + s2.width / 2, s2.y + 20 * zoom);
    await expect(page.locator(".story-edit")).toHaveText("Run two");
    await page.locator(".story-strip").getByRole("button", { name: "Different first page" }).click();
    // The section's first page now has its own (empty) header, and the strip edits it.
    await expect(page.locator(".story-strip")).toContainText("First page header on page 3");
    await page.keyboard.type("Title two");
    await page.keyboard.press("Escape");
    const secs = (await setup(page)).sections;
    expect([secs[0].titlePage, secs[1].titlePage]).toEqual([false, true]);
    expect(await hf(page, 0, "hdr")).toBe("Run one");
    expect(await hf(page, 2, "hdr")).toBe("Title two");
    expect(await hf(page, 3, "hdr")).toBe("Run two");
  });

  test("double-clicking the desk beside a narrow page's margin does not open the header", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Narrow"], { headers: { default: "Head" } }), landscape(["Wide"])] });
    const zoom = await zoomOf(page);
    const s0 = await viewSheet(page, 0);
    const stage = await box(page.locator(".stage"));
    // Left of the narrow sheet, still inside the stage (the wide sheet is below).
    const x = (stage.x + s0.x) / 2 + 2;
    expect(x).toBeLessThan(s0.x);
    await page.mouse.dblclick(x, s0.y + 20 * zoom);
    await expect(page.locator(".story-strip")).toBeHidden();
  });
});

test.describe("footnotes and line numbers on pages of other sizes", () => {
  test("a footnote in a landscape section sits at the foot of its landscape page", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Opening"]), landscape(["Wide page with a note", "Another line"], { margins: [1440, 1440, 1440, 2160] })] });
    const id = Number(await withText(page, "Wide page with a note").getAttribute("data-p"));
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "insert_footnote", at: { p, o: 4 }, text: "The note's own text" }), id);
    const zoom = await zoomOf(page);
    const s1 = await box(sheet(page, 1));
    near(s1.width / zoom, 1056, 0.5);
    const notes = sheet(page, 1).locator(".notes");
    await expect(notes).toContainText("The note's own text");
    await expect(sheet(page, 0).locator(".notes")).toHaveCount(0);
    const n = await box(notes);
    // Within the page's margins, and its bottom at the bottom margin.
    near((n.x - s1.x) / zoom, 144, 0.5);
    near((s1.y + s1.height - (n.y + n.height)) / zoom, 96, 1.5);
    // The text above it ends before it starts.
    const body = await box(withText(page, "Another line"));
    expect(body.y + body.height).toBeLessThanOrEqual(n.y);

    // Double-clicking the note edits it.
    await notes.scrollIntoViewIfNeeded();
    const m = await box(notes);
    await page.mouse.dblclick(m.x + 40, m.y + m.height - 6);
    await expect(page.locator(".story-edit")).toContainText("The note's own text");
    await page.keyboard.press("Escape");
  });

  test("line numbers sit left of the text of each page, whatever its margin", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["First one", "Second one"]), landscape(["Third one", "Fourth one"], { margins: [1440, 1440, 1440, 2880] })] });
    // Number every section.
    await caretIn(page, "First one");
    await chooseMenu(page, "Tools", "Line numbers…");
    const dlg = dialog(page, "Line numbers");
    await dlg.getByRole("checkbox", { name: "Show line numbers" }).check();
    await dlg.getByRole("radio", { name: "Whole document" }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    const zoom = await zoomOf(page);
    for (const [k, margin] of [[0, 96], [1, 192]] as const) {
      const s = await box(sheet(page, k));
      const num = await box(sheet(page, k).locator(".lnum span").first());
      // Right-aligned a quarter inch (24px) left of the text, which is `margin` from the sheet's edge.
      near((num.x + num.width - s.x) / zoom, margin - 24, 1);
    }
  });
});

test.describe("rulers follow the caret from section to section", () => {
  test("the horizontal ruler shows the caret's section's width and margins", async ({ page }) => {
    await openBuilt(page, {
      sections: [portrait(["First section"]), landscape(["Second section"], { margins: [720, 1440, 2160, 2880] })],
    });
    const zoom = await zoomOf(page);
    const ruler = page.locator(".ruler");
    await caretIn(page, "First section");
    await expect.poll(async () => Math.round((await box(ruler)).width / zoom)).toBe(816);
    const margins = ruler.locator(".ruler-margin");
    near((await box(margins.nth(0))).width / zoom, 96, 0.5);
    near((await box(margins.nth(1))).width / zoom, 96, 0.5);

    await caretIn(page, "Second section");
    await expect.poll(async () => Math.round((await box(ruler)).width / zoom)).toBe(1056);
    near((await box(margins.nth(0))).width / zoom, 192, 0.5);
    near((await box(margins.nth(1))).width / zoom, 96, 0.5);
    // The ruler stands over the sheet it describes: same left edge and width as the landscape sheet.
    const r = await box(ruler);
    const s1 = await box(sheet(page, 1));
    near(r.x, s1.x);
    near(r.width, s1.width);
    // The indent markers are inside the text area: the left one at the left margin.
    const left = await box(ruler.locator(".ruler-marker.left"));
    near((left.x + left.width / 2 - r.x) / zoom, 192, 1);

    await caretIn(page, "First section");
    await expect.poll(async () => Math.round((await box(ruler)).width / zoom)).toBe(816);
  });

  test("the vertical ruler stands beside the caret's page, as tall as it and with its margins", async ({ page }) => {
    await openBuilt(page, {
      sections: [portrait(["First section"]), landscape(["Second section"], { margins: [720, 1440, 2160, 1440] })],
    });
    const zoom = await zoomOf(page);
    const v = page.locator(".vruler");
    await caretIn(page, "First section");
    await expect.poll(async () => Math.abs((await box(v)).y - (await box(sheet(page, 0))).y)).toBeLessThan(1);
    near((await box(v)).height / zoom, 1056, 0.5);
    near((await box(v.locator(".vruler-margin").first())).height / zoom, 96, 0.5);
    near((await box(v.locator(".vruler-margin").nth(1))).height / zoom, 96, 0.5);

    await caretIn(page, "Second section");
    await expect.poll(async () => Math.abs((await box(v)).y - (await box(sheet(page, 1))).y)).toBeLessThan(1);
    near((await box(v)).height / zoom, 816, 0.5);
    // 0.5 in at the top and 1.5 in at the bottom: this section's, not the first one's.
    near((await box(v.locator(".vruler-margin").first())).height / zoom, 48, 0.5);
    near((await box(v.locator(".vruler-margin").nth(1))).height / zoom, 144, 0.5);
    // Its left edge hugs this (wider) sheet, not the narrower one's.
    const s1 = await box(sheet(page, 1));
    const vb = await box(v);
    expect(vb.x + vb.width).toBeLessThanOrEqual(s1.x);
    expect(s1.x - (vb.x + vb.width)).toBeLessThan(20 * zoom);
  });

  test("dragging the vertical ruler changes the margins of the page's own section only", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["First section"]), landscape(["Second section"])] });
    await caretIn(page, "Second section");
    const zoom = await zoomOf(page);
    const v = page.locator(".vruler");
    await expect.poll(async () => Math.abs((await box(v)).y - (await box(sheet(page, 1))).y)).toBeLessThan(1);
    // The handle mid-window, so the drag stays inside it: Playwright's Firefox sends no mouse event past the window's edge (the release is lost),
    // where a real mouse that leaves the window still belongs to the drag.
    await v.locator(".vruler-handle").first().evaluate((el) => el.scrollIntoView({ block: "center" }));
    await expect.poll(async () => Math.abs((await box(v)).y - (await box(sheet(page, 1))).y)).toBeLessThan(1);
    const handle = await box(v.locator(".vruler-handle").first());
    const x = handle.x + handle.width / 2;
    const y = handle.y + handle.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y + 48 * zoom, { steps: 4 });
    await page.mouse.up();
    // (Firefox hands the page the end of a drag a moment after the mouse is up, so the margin is waited for.)
    await expect.poll(async () => (await setup(page)).sections[1].margin[0]).toBeGreaterThan(1440 + 600);
    expect((await setup(page)).sections[0].margin[0]).toBe(1440);
  });
});

test.describe("printing", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      window.print = () => undefined;
    });
  });

  test("each sheet prints on paper of its own size and orientation", async ({ page }) => {
    await openBuilt(page, {
      sections: [portrait(["Portrait one"]), landscape(["Landscape two"], { margins: [1440, 720, 1440, 2880] }), portrait(["Portrait three"], { size: [11906, 16838] })],
      evenAndOdd: false,
    });
    await page.keyboard.press("Escape");
    await caretIn(page, "Portrait one");
    await page.keyboard.press(`${MOD}+P`);
    const printed = page.locator(".docxtor-print .print-page");
    await expect(printed).toHaveCount(3);
    // The sheets are as big as their paper.
    const sizes = await printed.evaluateAll((els) => els.map((e) => [Math.round(parseFloat((e as HTMLElement).style.width)), Math.round(parseFloat((e as HTMLElement).style.height))]));
    expect(sizes).toEqual([[816, 1056], [1056, 816], [794, 1123]]);
    // Each is on a named page of its size, and the named pages carry the sizes.
    const named = await printed.evaluateAll((els) => els.map((e) => (e as HTMLElement).style.getPropertyValue("page")));
    expect(named).toEqual(["s0", "s1", "s2"]);
    const rules = await page.evaluate(() => Array.from(document.head.querySelectorAll("style"), (s) => s.textContent ?? "").filter((t) => t.includes("@page")).join("\n"));
    expect(rules).toContain("@page s0 { size: 8.5in 11in; margin: 0; }");
    expect(rules).toContain("@page s1 { size: 11in 8.5in; margin: 0; }");
    expect(rules).toMatch(/@page s2 \{ size: 8\.268\d*in 11\.693\d*in; margin: 0; \}/);
    // Each print page shows only its own sheet: shifted by the sheet's place in the stage.
    const shifts = await printed.evaluateAll((els) => els.map((e) => [(e.querySelector(".stage") as HTMLElement).style.top, (e.querySelector(".stage") as HTMLElement).style.left]));
    expect(shifts[0]).toEqual(["0px", "-120px"]);
    expect(shifts[1][1]).toBe("0px");
    await expect(printed.nth(1)).toContainText("Landscape two");
    // Page colour stays on the sheets.
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
    await expect(page.locator(".docxtor-print")).toHaveCount(0);
  });

  test("with one paper size there are no named pages", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["One"]), portrait(["Two"], { start: "nextPage", margins: [720, 720, 720, 720] })] });
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .print-page")).toHaveCount(2);
    const named = await page.locator(".docxtor-print .print-page").evaluateAll((els) => els.map((e) => (e as HTMLElement).style.getPropertyValue("page")));
    expect(named).toEqual(["", ""]);
    const rules = await page.evaluate(() => Array.from(document.head.querySelectorAll("style"), (s) => s.textContent ?? "").filter((t) => t.includes("@page")));
    expect(rules).toEqual(["@page { size: 8.5in 11in; margin: 0; }"]);
  });

  test("line numbers and page colour still print on a landscape sheet", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["One"]), landscape(["Two", "Three"])] });
    await caretIn(page, "One");
    await inPage(page, (w) => w.fe.editor.exec([{ op: "set_page_color", color: "FFF9E6" }, { op: "set_line_numbers", on: true }]));
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .sheet").nth(1)).toHaveCSS("background-color", "rgb(255, 249, 230)");
    await expect(page.locator(".docxtor-print .sheet").nth(1).locator(".lnum span").first()).toBeAttached();
  });
});

test.describe("the pages API", () => {
  test("the engine's sections and the editor's pages agree", async ({ page }) => {
    const bytes = buildDocx({ sections: [portrait(["a"]), landscape(["b"]), portrait(["c"])] });
    expect(bytes.length).toBeGreaterThan(500);
    await openBuilt(page, { sections: [portrait(["Alpha"]), landscape(["Beta"]), portrait(["Gamma"])] });
    const boxes = await inPage(page, (w) => Array.from({ length: w.fe.editor.pages }, (_, k) => w.fe.editor.pageBox(k)));
    expect(boxes.map((b) => [b.w, b.h, b.section])).toEqual([[816, 1056, 0], [1056, 816, 1], [816, 1056, 2]]);
    // Pages stack with the gap between them, and narrower sheets are centred in the stage.
    expect(boxes[1].top).toBe(1056 + 28);
    expect(boxes[2].top).toBe(1056 + 28 + 816 + 28);
    expect(boxes.map((b) => b.left)).toEqual([120, 0, 120]);
  });
});
