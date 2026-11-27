// The layouts (Print layout, Pageless, Reading), views that extensions add, and compact mode.
import { readFileSync } from "node:fs";
import { chooseMenu, expect, menuItem, openMenu, test, topMenu, type Page } from "./helpers";
import { readPdf } from "./support/pdf-reader";
import { bytesOf, chooseLayout, filler, heading, HEADING_STYLES, idOf, LONG_OUTLINE_DOC, onFe, openDoc, OUTLINE_DOC } from "./support/view-helpers";

const sheets = (page: Page) => page.locator(".editor-host .sheet");
const sheetBox = (page: Page) => page.locator(".sheet").first().boundingBox();
const view = (page: Page) => onFe(page, (fe) => fe.view);

/** Two pages with a header and a footer, and a footnote on the first. */
const PAGED = {
  sections: [
    {
      headers: { default: "Header text" },
      footers: { default: "Footer {PAGE}" },
      paragraphs: ["First page text", "More first page text{PBR}", "Second page text", ...filler(3, "Line")],
    },
  ],
};

test.describe("Layout", () => {
  test("View, Layout lists the layouts, with the one on screen checked", async ({ page }) => {
    await openDoc(page);
    await openMenu(page, "View");
    await menuItem(page, "Layout").click();
    const names = await page.locator(".menu-drop").last().locator('[role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"]').locator(".menu-label").allTextContents();
    expect(names).toEqual(["Print layout", "Pageless", "Reading", "Outline", "Draft"]);
    await expect(menuItem(page, "Print layout")).toHaveAttribute("aria-checked", "true");
    await expect(menuItem(page, "Pageless")).toHaveAttribute("aria-checked", "false");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
  });

  test("Pageless is one long sheet: no page breaks, headers, footers or page numbers", async ({ page }) => {
    await openDoc(page, PAGED);
    await expect(sheets(page)).toHaveCount(2);
    await expect(page.locator(".sheet .hdr")).toHaveCount(2);
    await expect(page.locator(".sheet .ftr")).toHaveCount(2);
    const print = await onFe(page, (fe) => fe.editor.zoom);
    await chooseLayout(page, "Pageless");
    expect(await view(page)).toBe("pageless");
    await expect(sheets(page)).toHaveCount(1);
    await expect(page.locator(".sheet .hf")).toHaveCount(0);
    await expect(page.locator(".sheet .page-num")).toHaveCount(0);
    expect(await onFe(page, (fe) => fe.editor.pages)).toBe(1);
    // The page break is not a break here: the text runs on, and the sheet ends below the last line.
    const text = await page.locator(".flow .fp").evaluateAll((els) => els.map((el) => ({ t: el.textContent, bottom: el.getBoundingClientRect().bottom })));
    expect(text.map((x) => x.t)).toEqual(["First page text", "More first page text", "Second page text", "Line 1", "Line 2", "Line 3"]);
    const sheet = (await sheetBox(page))!;
    const last = text[text.length - 1];
    expect(sheet.y + sheet.height).toBeGreaterThan(last.bottom);
    expect(sheet.y + sheet.height - last.bottom).toBeLessThan(150);
    // The sheet is as wide as the page's text and margins, and as tall as the text.
    const zoom = await onFe(page, (fe) => fe.editor.zoom);
    expect(sheet.width / zoom).toBeCloseTo(816, 0);
    expect(zoom).toBeLessThanOrEqual(1);
    expect(print).toBeLessThanOrEqual(1);
    await page.screenshot({ path: "test-results/view-pageless.png" });
    // Back to print layout: pages, headers and footers are back.
    await chooseLayout(page, "Print layout");
    await expect(sheets(page)).toHaveCount(2);
    await expect(page.locator(".sheet .hdr")).toHaveCount(2);
    expect(await view(page)).toBe("print");
  });

  // A header entered in a view without pages had nothing on screen to type in, so the keyboard
  // ended on the page body after the next command that moved it.
  test("a view without pages leaves the header being edited, and Edit header goes back to Print layout first", async ({ page }) => {
    await openDoc(page, PAGED);
    await onFe(page, (fe) => fe.editor.enterStory("header", 0));
    await expect(page.locator(".story-edit")).toBeFocused();
    for (const layout of ["Pageless", "Draft"]) {
      await chooseLayout(page, layout);
      await expect(page.locator(".story-edit")).toHaveCount(0);
      await expect(page.locator(".flow")).toBeFocused();
      // Asked for the header here: nothing to edit on screen, so nothing happens.
      await onFe(page, (fe) => fe.editor.enterStory("header", 0));
      expect(await onFe(page, (fe) => fe.editor.story)).toBeNull();
      // Insert, Edit header: Print layout, and the header.
      await page.evaluate(() => (window as unknown as { docxtor: { fe: { extensions: { execute(id: string): Promise<unknown> } } } }).docxtor.fe.extensions.execute("insert.header"));
      expect(await view(page)).toBe("print");
      await expect(page.locator(".story-edit")).toBeFocused();
    }
  });

  test("Pageless can be edited, and the sheet grows with the text", async ({ page }) => {
    await openDoc(page, PAGED);
    await chooseLayout(page, "Pageless");
    const before = (await sheetBox(page))!.height;
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("End");
    for (let i = 0; i < 12; i++) {
      await page.keyboard.press("Enter");
      await page.keyboard.type(`Typed line ${i + 1}`);
    }
    await expect(page.locator(".flow .fp", { hasText: "Typed line 12" })).toBeVisible();
    await expect.poll(async () => (await sheetBox(page))!.height).toBeGreaterThan(before + 100);
    await expect(sheets(page)).toHaveCount(1);
  });

  test("a footnote goes at the foot of the long sheet", async ({ page }) => {
    await openDoc(page, {
      footnotes: '<w:footnote w:id="1"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> The note text.</w:t></w:r></w:p></w:footnote>',
      sections: [{ paragraphs: [{ text: "Body text", xml: '<w:r><w:footnoteReference w:id="1"/></w:r>' }, ...filler(60)] }],
    });
    await expect(sheets(page)).toHaveCount(2);
    await chooseLayout(page, "Pageless");
    const notes = page.locator(".sheet .notes");
    await expect(notes).toHaveCount(1);
    await expect(notes).toContainText("The note text.");
    const n = (await notes.boundingBox())!;
    const s = (await sheetBox(page))!;
    const lastLine = (await page.locator(".flow .fp").last().boundingBox())!;
    expect(n.y).toBeGreaterThan(lastLine.y + lastLine.height - 2);
    expect(n.y + n.height).toBeLessThanOrEqual(s.y + s.height + 1);
  });

  test("columns show as one column and a new-page section break does not start a page", async ({ page }) => {
    await openDoc(page, {
      sections: [
        { paragraphs: ["Section one text", "More text"], cols: { num: 2 } },
        { size: [15840, 12240], paragraphs: ["Section two text"] },
      ],
    });
    await expect(sheets(page)).toHaveCount(2);
    await chooseLayout(page, "Pageless");
    await expect(sheets(page)).toHaveCount(1);
    await expect(page.locator(".flow > .cols")).toHaveCount(0);
    const widths = await page.locator(".flow .fp").evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().width)));
    expect(new Set(widths).size).toBe(1);
    const zoom = await onFe(page, (fe) => fe.editor.zoom);
    // The first section's paper decides the width, so the landscape section has the portrait width too.
    expect((await sheetBox(page))!.width / zoom).toBeCloseTo(816, 0);
    const tops = await page.locator(".flow .fp").evaluateAll((els) => els.map((el) => el.getBoundingClientRect().top));
    expect(tops[2] - tops[1]).toBeLessThan(60);
  });

  test("the rulers go while there are no pages, and come back", async ({ page }) => {
    await openDoc(page, PAGED);
    await expect(page.locator(".ruler")).toBeVisible();
    await chooseLayout(page, "Pageless");
    await expect(page.locator(".ruler")).toBeHidden();
    await chooseLayout(page, "Print layout");
    await expect(page.locator(".ruler")).toBeVisible();
  });

  test("a view stays when another document opens", async ({ page }) => {
    await openDoc(page, PAGED);
    await chooseLayout(page, "Pageless");
    const next = bytesOf({ sections: [{ paragraphs: ["Another document", "Page{PBR}", "Last"] }] });
    await page.evaluate(async (b) => (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name: string): Promise<unknown> } } }).docxtor.fe.open(new Uint8Array(b), "next.docx"), next);
    await expect(page.locator(".flow .fp").first()).toHaveText("Another document");
    expect(await view(page)).toBe("pageless");
    await expect(sheets(page)).toHaveCount(1);
  });
});

test.describe("Reading", () => {
  test("is read-only with the text in a column that fits the window", async ({ page }) => {
    await openDoc(page, PAGED);
    await expect(page.locator(".toolbar")).toBeVisible();
    await chooseLayout(page, "Reading");
    expect(await view(page)).toBe("reading");
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(true);
    await expect(page.locator(".toolbar")).toBeHidden();
    await expect(sheets(page)).toHaveCount(1);
    const stage = await onFe(page, (fe) => fe.editor.stageWidth);
    // 720 for the text and 40 each side.
    expect(stage).toBe(800);
    // Typing does nothing.
    await page.locator(".flow .fp").first().click();
    await page.keyboard.type("nope");
    await expect(page.locator(".flow .fp").first()).toHaveText("First page text");
    await page.screenshot({ path: "test-results/view-reading.png" });
  });

  test("the text reflows as the window gets narrower", async ({ page }) => {
    await openDoc(page, { sections: [{ paragraphs: ["A sentence long enough to be broken over several lines when the column is narrow, which it will be soon. ".repeat(6)] }] });
    await chooseLayout(page, "Reading");
    const lines = () => page.locator(".flow .fp").first().evaluate((el) => el.getClientRects().length && Math.round(el.getBoundingClientRect().height / parseFloat(getComputedStyle(el).lineHeight || "20")));
    const wide = await onFe(page, (fe) => fe.editor.stageWidth);
    const wideHeight = (await page.locator(".flow .fp").first().boundingBox())!.height;
    expect(lines).toBeDefined();
    await page.setViewportSize({ width: 700, height: 900 });
    await expect.poll(() => onFe(page, (fe) => fe.editor.stageWidth)).toBeLessThan(wide);
    const narrow = await onFe(page, (fe) => fe.editor.stageWidth);
    const room = await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>(".canvas")!;
      const cs = getComputedStyle(c);
      return c.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
    });
    const zoom = await onFe(page, (fe) => fe.editor.zoom);
    // The sheet fits in the window, and the text is taller because there are more lines. clientWidth is a whole number of pixels
    // and the zoom a fraction, so allow a pixel and a half; a sheet that does not fit is out by a scrollbar or more.
    expect(narrow * zoom).toBeLessThanOrEqual(room + 1.5);
    expect((await page.locator(".flow .fp").first().boundingBox())!.height / zoom).toBeGreaterThan(wideHeight / 1.15);
  });

  test("Escape goes back to the print layout, editable, with the zoom as it was", async ({ page }) => {
    await openDoc(page, PAGED);
    const before = { zoom: await onFe(page, (fe) => fe.editor.zoom), mode: await onFe(page, (fe) => fe.editor.zoomer.mode) };
    await chooseLayout(page, "Reading");
    expect(await onFe(page, (fe) => fe.editor.zoom)).toBe(1.15);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("Escape");
    expect(await view(page)).toBe("print");
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(false);
    await expect(page.locator(".toolbar")).toBeVisible();
    expect(await onFe(page, (fe) => fe.editor.zoom)).toBe(before.zoom);
    expect(await onFe(page, (fe) => fe.editor.zoomer.mode)).toBe(before.mode);
    await expect(sheets(page)).toHaveCount(2);
  });

  test("a document that was being viewed stays read-only after Reading", async ({ page }) => {
    await openDoc(page, PAGED);
    await chooseMenu(page, "View", "Mode", "Viewing");
    await chooseLayout(page, "Reading");
    await chooseLayout(page, "Print layout");
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(true);
  });
});

test.describe("views from extensions", () => {
  const ADD = `({
    id: "acme.wide",
    name: "Wide view",
    activate(ctx) {
      ctx.views.register({
        id: "acme.wide",
        title: "Wide column",
        layout: "continuous",
        width: "window",
        maxWidth: 420,
        margin: 20,
        className: "acme-wide",
        enter(view) {
          window.entered = view.root.className;
          return () => { window.left = true; };
        },
      });
    },
  })`;

  test("a view an extension registers gets a command and a place in View, Layout", async ({ page }) => {
    await openDoc(page);
    await page.evaluate(async (src) => {
      const w = window as unknown as { docxtor: { fe: { use(e: unknown): Promise<void> } } };
      await w.docxtor.fe.use(new Function(`return ${src}`)());
    }, ADD);
    expect(await onFe(page, (fe) => fe.views().map((v) => v.id))).toEqual(["print", "pageless", "reading", "outline", "draft", "acme.wide"]);
    await chooseLayout(page, "Wide column");
    expect(await view(page)).toBe("acme.wide");
    // Its layout and its class are in force, and enter() ran.
    expect(await onFe(page, (fe) => fe.editor.stageWidth)).toBe(460);
    await expect(page.locator(".editor-host.acme-wide.view-acme\\.wide")).toHaveCount(1);
    expect(await page.evaluate(() => (window as unknown as { entered: string }).entered)).toContain("acme-wide");
    await openMenu(page, "View");
    await menuItem(page, "Layout").click();
    await expect(menuItem(page, "Wide column")).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    // Choosing another view undoes it.
    await chooseLayout(page, "Print layout");
    expect(await page.evaluate(() => (window as unknown as { left?: boolean }).left)).toBe(true);
    await expect(page.locator(".editor-host.acme-wide")).toHaveCount(0);
  });

  test("taking the extension away while its view is on goes back to the print layout", async ({ page }) => {
    await openDoc(page);
    await page.evaluate(async (src) => {
      const w = window as unknown as { docxtor: { fe: { use(e: unknown): Promise<void> } } };
      await w.docxtor.fe.use(new Function(`return ${src}`)());
    }, ADD);
    await chooseLayout(page, "Wide column");
    await onFe(page, (fe) => fe.extensions.remove("acme.wide"));
    expect(await view(page)).toBe("print");
    expect(await page.evaluate(() => (window as unknown as { left?: boolean }).left)).toBe(true);
    await openMenu(page, "View");
    await menuItem(page, "Layout").click();
    await expect(menuItem(page, "Wide column")).toHaveCount(0);
  });
});

test.describe("compact mode", () => {
  test("Ctrl+Shift+F folds the menu bar into its Menu button and brings it back", async ({ page }) => {
    await openDoc(page, { styles: HEADING_STYLES, sections: [{ paragraphs: [heading(1, "Title"), "Text"] }] });
    await expect(topMenu(page, "File")).toBeVisible();
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("Control+Shift+F");
    expect(await onFe(page, (fe) => fe.compact)).toBe(true);
    await expect(page.locator(".docxtor.compact")).toHaveCount(1);
    await expect(topMenu(page, "File")).toBeHidden();
    const menu = page.locator(".menubar-fold");
    await expect(menu).toBeVisible();
    // Everything is still in the Menu button.
    await menu.click();
    await expect(page.getByRole("menuitem", { name: "Insert", exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await page.screenshot({ path: "test-results/view-compact.png" });
    await page.keyboard.press("Control+Shift+F");
    expect(await onFe(page, (fe) => fe.compact)).toBe(false);
    await expect(topMenu(page, "File")).toBeVisible();
  });

  test("View, Compact mode is a checked item, and it is remembered", async ({ page }) => {
    await openDoc(page, OUTLINE_DOC);
    await chooseMenu(page, "View", "Compact mode");
    // The menus are in the Menu button now, with the item checked.
    await page.locator(".menubar-fold").click();
    await page.getByRole("menuitem", { name: "View", exact: true }).click();
    await expect(menuItem(page, "Compact mode")).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await page.reload();
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await expect.poll(() => onFe(page, (fe) => fe.compact)).toBe(true);
    await expect(topMenu(page, "File")).toBeHidden();
  });

  test("the toolbar is tighter in compact mode", async ({ page }) => {
    await openDoc(page, OUTLINE_DOC);
    const pad = () => page.locator(".toolbar .tgroup").first().evaluate((el) => parseFloat(getComputedStyle(el).paddingTop));
    const normal = await pad();
    await onFe(page, (fe) => (fe as unknown as { setCompact(on: boolean): void }).setCompact(true));
    expect(await pad()).toBeLessThan(normal);
  });
});

test.describe("printing and the PDF from a view", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      const calls: number[] = [];
      (window as unknown as { __printCalls: number[] }).__printCalls = calls;
      window.print = () => void calls.push(document.querySelectorAll(".docxtor-print .print-page").length);
    });
  });
  const printCalls = (page: Page) => page.evaluate(() => (window as unknown as { __printCalls: number[] }).__printCalls);

  test("Pageless prints the pages, with their headers, and the view is back afterwards", async ({ page }) => {
    await openDoc(page, PAGED);
    await chooseLayout(page, "Pageless");
    await page.keyboard.press("ControlOrMeta+P");
    await expect.poll(() => printCalls(page)).toEqual([2]);
    await expect(page.locator(".docxtor-print .print-page").first()).toContainText("Header text");
    await expect.poll(() => onFe(page, (fe) => fe.editor.continuous)).toBe(true);
    await expect(sheets(page)).toHaveCount(1);
    expect(await view(page)).toBe("pageless");
  });

  test("folded headings print in full, and are folded again afterwards", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    const pages = await onFe(page, (fe) => fe.editor.pages);
    await page.locator(`.fm-btn[data-id="${await idOf(page, "Alpha")}"]`).click();
    expect(await onFe(page, (fe) => fe.editor.pages)).toBeLessThan(pages);
    await page.keyboard.press("ControlOrMeta+P");
    await expect.poll(() => printCalls(page)).toEqual([pages]);
    await expect.poll(() => onFe(page, (fe) => fe.editor.pages)).toBeLessThan(pages);
    await expect(page.locator(".fm-btn.folded")).toHaveCount(1);
  });

  test("the PDF has the pages of the document, not the one long sheet", async ({ page }) => {
    await openDoc(page, PAGED);
    await chooseLayout(page, "Pageless");
    const [download] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
    const pdf = readPdf(readFileSync((await download.path())!));
    expect(pdf.pages).toHaveLength(2);
    expect(await view(page)).toBe("pageless");
    await expect(sheets(page)).toHaveCount(1);
  });
});

test.describe("on a narrow screen", () => {
  test.use({ viewport: { width: 420, height: 800 } });

  test("the outline strip wraps, the slider gives way to the factor, and nothing runs off the screen", async ({ page }) => {
    await openDoc(page, OUTLINE_DOC);
    await expect(page.locator(".zoom-slider")).toBeHidden();
    await expect(page.locator(".zoom-readout")).toBeVisible();
    // The menus are folded into one button here: choose the view through the editor.
    await onFe(page, (fe) => fe.setView("outline"));
    const strip = page.getByRole("toolbar", { name: "Outline" });
    await expect(strip).toBeVisible();
    for (const name of ["Promote", "Demote", "Move up", "Move down", "Close outline view"]) await expect(strip.getByRole("button", { name, exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
    // The column fits the window.
    const stage = await onFe(page, (fe) => fe.editor.stageWidth);
    const zoom = await onFe(page, (fe) => fe.editor.zoom);
    const room = await page.evaluate(() => document.querySelector<HTMLElement>(".canvas")!.clientWidth);
    expect(stage * zoom).toBeLessThanOrEqual(room);
  });
});
