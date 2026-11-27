// Breaks and layout: the Break menu, pagination options, borders and shading, page color, hyphenation,
// line numbers and the vertical ruler.
import { caretAtEnd, chooseMenu, dialog, expect, inPage, menuItem, openFixture, openMenu, para, paraText, RICH, selectWord, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** The document on screen (not the one the app opened: `fe.open` can swap it). */
const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup());
const pageOf = (page: Page, id: number) => inPage(page, (w, p) => w.fe.editor.pageMap()[String(p)], id);
const caretPara = (page: Page) => inPage(page, (w) => w.fe.editor.sel!.focus.p);
const caretOffset = (page: Page) => inPage(page, (w) => w.fe.editor.sel!.focus.o);
const paraFmt = (page: Page, id: number) => inPage(page, (w, p) => w.engine!.formatAt({ p, o: 0 })!.para, id);
/** Two animation frames: the browser has reported the caret's new place by then. */
const settle = (page: Page) => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));

/** Put the caret in a paragraph (a plain click in its first line). */
async function caretIn(page: Page, id: number) {
  await para(page, id).click({ position: { x: 24, y: 6 } });
}

/** Add paragraphs after the document's last one, each on its own line. */
async function typeLines(page: Page, last: number, lines: string[]) {
  await caretAtEnd(page, last);
  for (const line of lines) {
    await page.keyboard.press("Enter");
    await page.keyboard.type(line);
  }
}

test.describe("Insert, Break", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  // How many pages later the new section starts: the title is on page 1, so an odd-page section skips page 2.
  for (const [label, start, later] of [
    ["Section break (next page)", "nextPage", 1],
    ["Section break (continuous)", "continuous", 0],
    ["Section break (even page)", "evenPage", 1],
    ["Section break (odd page)", "oddPage", 2],
  ] as const) {
    test(`${label} starts a new section${later ? " on a new page" : " on the same page"}`, async ({ page }) => {
      await caretAtEnd(page, RICH.title);
      expect((await setup(page)).sections).toHaveLength(1);
      await chooseMenu(page, "Insert", "Break", label);

      const sections = (await setup(page)).sections;
      expect(sections).toHaveLength(2);
      expect(sections[0].end).toBe(RICH.title);
      expect(sections[1].start).toBe(start);
      // The caret is at the start of the new section.
      const caret = await caretPara(page);
      expect(caret).not.toBe(RICH.title);
      expect(await caretOffset(page)).toBe(0);
      const here = await pageOf(page, caret);
      const before = await pageOf(page, RICH.title);
      // An even-page or odd-page break lands on a page of that number, leaving a blank page when it has to.
      expect(here).toBe(before + later);

      // One undo step.
      await page.keyboard.press(`${MOD}+Z`);
      expect((await setup(page)).sections).toHaveLength(1);
    });
  }

  test("a selection is replaced by the break", async ({ page }) => {
    await selectWord(page, RICH.intro, "report");
    const text = (await para(page, RICH.intro).textContent()) ?? "";
    expect(text).toContain("report");
    await chooseMenu(page, "Insert", "Break", "Section break (continuous)");
    expect((await setup(page)).sections).toHaveLength(2);
    // The word is gone: the paragraph was split where it stood.
    const all = await inPage(page, (w) => w.engine!.text());
    expect(all).not.toContain("This report");
    expect(all).toContain("This ");
    await expect(para(page, RICH.intro)).not.toContainText("report");
  });

  test("a column break is one character; the next paragraph goes to the next page (one column)", async ({ page }) => {
    await caretAtEnd(page, RICH.title);
    const len = (await para(page, RICH.title).textContent())!.length;
    expect(await pageOf(page, RICH.intro)).toBe(1);
    await chooseMenu(page, "Insert", "Break", "Column break");

    const atom = para(page, RICH.title).locator("span.atom.cbr");
    await expect(atom).toHaveCount(1);
    await expect(atom).toHaveAttribute("title", "Column break");
    await expect(para(page, RICH.title)).toHaveAttribute("data-pbr", "1");
    await expect(para(page, RICH.title)).toHaveAttribute("data-cbr", "1");
    // The caret is just after it, and the next paragraph starts the next page.
    expect(await caretOffset(page)).toBe(len + 1);
    expect(await pageOf(page, RICH.intro)).toBe(2);
    expect((await setup(page)).sections).toHaveLength(1);

    // Text after the break continues on the next page.
    await page.keyboard.type("X");
    await expect(para(page, RICH.title).locator(".pgsp")).toHaveCount(1);
    expect(await caretOffset(page)).toBe(len + 2);

    // The caret crosses the break in single steps, as it crosses a page break.
    // (The browser stops once more on the page-split spacer, so a few more key presses are needed.)
    const left: number[] = [];
    for (let i = 0; i < 8; i++) {
      await page.keyboard.press("ArrowLeft");
      await settle(page);
      const o = await caretOffset(page);
      if (left[left.length - 1] !== o) left.push(o);
    }
    // One character at a time, the break included, and never past it in one step.
    expect(left.slice(0, 3)).toEqual([len + 1, len, len - 1]);
    expect(left.every((o, i) => i === 0 || o === left[i - 1] - 1)).toBe(true);
    const right: number[] = [];
    for (let i = 0; i < 8; i++) {
      await page.keyboard.press("ArrowRight");
      await settle(page);
      const o = await caretOffset(page);
      if (right[right.length - 1] !== o) right.push(o);
    }
    expect(right.every((o, i) => i === 0 || o === right[i - 1] + 1)).toBe(true);
    expect(right).toEqual(expect.arrayContaining([len, len + 1, len + 2]));

    // Backspace takes it away again, and the document is back on two pages.
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("Backspace");
    await settle(page);
    await page.keyboard.press("Backspace");
    await expect(para(page, RICH.title).locator("span.atom.cbr")).toHaveCount(0);
    expect(await pageOf(page, RICH.intro)).toBe(1);
  });

  test("section breaks are greyed out in a table and while editing a header; page and column breaks are not", async ({ page }) => {
    await para(page, RICH.cell).click();
    await openMenu(page, "Insert");
    await menuItem(page, "Break").click();
    for (const label of ["Section break (next page)", "Section break (continuous)", "Section break (even page)", "Section break (odd page)"]) await expect(menuItem(page, label)).toBeDisabled();
    await expect(menuItem(page, "Page break")).toBeEnabled();
    await expect(menuItem(page, "Column break")).toBeEnabled();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);

    await chooseMenu(page, "Insert", "Page elements", "Edit header");
    await expect(page.locator(".story-strip")).toBeVisible();
    await openMenu(page, "Insert");
    await menuItem(page, "Break").click();
    await expect(menuItem(page, "Section break (next page)")).toBeDisabled();
  });
});

test.describe("Format, pagination options", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
    await caretIn(page, RICH.intro);
  });

  const SPACING = ["Format", "Line and paragraph spacing"];
  const toggles = [
    ["Keep with next", "keepNext", "data-kn"],
    ["Keep lines together", "keepLines", "data-kl"],
    ["Add page break before", "pageBreakBefore", "data-pbb"],
  ] as const;

  for (const [label, key, attr] of toggles) {
    test(`${label} shows its state and writes ${key}`, async ({ page }) => {
      await openMenu(page, SPACING[0]);
      await menuItem(page, SPACING[1]).click();
      await expect(menuItem(page, label)).toHaveAttribute("aria-checked", "false");
      await menuItem(page, label).click();
      expect((await paraFmt(page, RICH.intro))[key]).toBe(true);
      await expect(para(page, RICH.intro)).toHaveAttribute(attr, "1");

      await openMenu(page, SPACING[0]);
      await menuItem(page, SPACING[1]).click();
      await expect(menuItem(page, label)).toHaveAttribute("aria-checked", "true");
      await menuItem(page, label).click();
      expect((await paraFmt(page, RICH.intro))[key] ?? false).toBe(false);
      await expect(para(page, RICH.intro)).not.toHaveAttribute(attr, /.*/);
    });
  }

  test("Prevent single lines is on until the paragraph turns it off", async ({ page }) => {
    await openMenu(page, "Format");
    await menuItem(page, "Line and paragraph spacing").click();
    await expect(menuItem(page, "Prevent single lines")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Prevent single lines").click();
    expect((await paraFmt(page, RICH.intro)).widowControl).toBe(false);
    await expect(para(page, RICH.intro)).toHaveAttribute("data-wc", "0");

    await openMenu(page, "Format");
    await menuItem(page, "Line and paragraph spacing").click();
    await expect(menuItem(page, "Prevent single lines")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Prevent single lines").click();
    await expect(para(page, RICH.intro)).not.toHaveAttribute("data-wc", /.*/);
    await expect(para(page, RICH.intro)).toBeVisible();
  });
});

test.describe("widow control", () => {
  test("with Prevent single lines off, a single line may sit at the bottom of a page", async ({ page }) => {
    await openFixture(page, "long.docx");
    const id = 3;
    // Move the paragraph down until exactly one of its lines fits above the bottom margin.
    const extra = await page.evaluate((p) => {
      const w = (window as unknown as { docxtor: { engine: { pageSetup(): { height: number; margin: number[] } }; fe: { editor: { flow: HTMLElement } } } }).docxtor;
      const flow = w.fe.editor.flow;
      const el = flow.querySelector<HTMLElement>(`[data-p="${p}"]`)!;
      const fr = flow.getBoundingClientRect();
      const sc = flow.offsetHeight ? fr.height / flow.offsetHeight : 1;
      const range = document.createRange();
      range.selectNodeContents(el);
      const bottoms = Array.from(new Set(Array.from(range.getClientRects()).filter((r) => r.height > 0).map((r) => Math.round((r.bottom - fr.top) / sc)))).sort((a, b) => a - b);
      const ps = w.engine.pageSetup();
      const limit = (ps.height - ps.margin[2]) / 15;
      return { lines: bottoms.length, shift: limit - (bottoms[0] + bottoms[1]) / 2 };
    }, id);
    expect(extra.lines).toBeGreaterThanOrEqual(3);
    const before = ((await paraFmt(page, id)).spaceBefore as number | undefined) ?? 0;
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "format_paragraphs", ids: [a.id], format: { spaceBefore: a.value } }), { id, value: Math.round(before + extra.shift * 15) });

    // On: the whole paragraph moves to the next page.
    await expect(para(page, id).locator(".pgsp")).toHaveCount(0);
    expect(await pageOf(page, id)).toBe(2);

    await caretIn(page, id);
    await chooseMenu(page, "Format", "Line and paragraph spacing", "Prevent single lines");
    await expect(para(page, id)).toHaveAttribute("data-wc", "0");
    // Off: one line stays on page 1 and the rest carries on over the page.
    expect(await pageOf(page, id)).toBe(1);
    await expect(para(page, id).locator(".pgsp")).toHaveCount(1);
  });
});

test.describe("a paragraph over several pages", () => {
  // The paginator works out every page end of a paragraph from one measurement and inserts the spacers afterwards
  // (Paginator.flushSpacers): several cuts in one text node, and the page each part lands on, must come out as if
  // measured one by one.
  test("is cut at every page end, each page carrying on at its top margin, and the same again after undo and redo", async ({ page }) => {
    await openFixture(page);
    const id = RICH.title;
    const inserted = "The quick brown fox jumps over the lazy dog. ".repeat(260);
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "insert_text", at: { p: a.id, o: 0 }, text: a.text }), { id, text: inserted });

    const cuts = () =>
      para(page, id).evaluate((el) => {
        const flow = el.closest(".flow") as HTMLElement;
        const sc = flow.getBoundingClientRect().height / flow.offsetHeight;
        const sheets = Array.from(document.querySelectorAll<HTMLElement>(".sheet")).map((s) => s.getBoundingClientRect());
        const sheetOf = (y: number) => sheets.find((s) => y >= s.top - 1 && y < s.bottom)!;
        const range = document.createRange();
        const spacers = Array.from(el.querySelectorAll<HTMLElement>(".pgsp"));
        // Where the text carries on after each spacer, from the top of its page.
        const starts = spacers.map((sp) => {
          const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
          walker.currentNode = sp;
          const t = walker.nextNode() as Text;
          range.setStart(t, 0);
          range.setEnd(t, 1);
          const r = range.getClientRects()[0];
          return (r.top - sheetOf(r.top).top) / sc;
        });
        // The text before each spacer: where the cuts fall.
        const at = spacers.map((sp) => {
          range.setStart(el, 0);
          range.setEndBefore(sp);
          return range.toString().length;
        });
        // The text's lines (not the spacers' boxes): the lowest one on any page, from the top of its page, and the pages.
        const rects: DOMRect[] = [];
        const texts = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        for (let t = texts.nextNode(); t; t = texts.nextNode()) {
          range.selectNodeContents(t);
          rects.push(...Array.from(range.getClientRects()).filter((r) => r.height > 0 && r.width > 0));
        }
        const lowest = Math.max(...rects.map((r) => (r.bottom - sheetOf(r.top).top) / sc));
        return { at, starts, lowest, pages: new Set(rects.map((r) => sheets.indexOf(sheetOf(r.top)))).size };
      });
    const check = async () => {
      const c = await cuts();
      expect(c.at.length).toBeGreaterThanOrEqual(3);
      expect(c.pages).toBe(c.at.length + 1);
      // Every page carries on at the same height: no drift from page to page (it used to grow by a little each page).
      expect(Math.max(...c.starts) - Math.min(...c.starts)).toBeLessThan(0.05);
      // No line runs into the bottom margin.
      const ps = await setup(page);
      expect(c.lowest).toBeLessThanOrEqual((ps.height - ps.margin[2]) / 15 + 1);
      // The spacers take no text: the paragraph holds what was typed, once.
      expect((await inPage(page, (w) => w.engine!.text())).split(inserted)).toHaveLength(2);
      expect((await paraText(page, id)).startsWith(inserted)).toBe(true);
      return c.at;
    };
    const first = await check();

    await page.keyboard.press(`${MOD}+z`);
    await expect(para(page, id).locator(".pgsp")).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+z`);
    await expect(para(page, id).locator(".pgsp")).toHaveCount(first.length);
    expect(await check()).toEqual(first);
  });
});

test.describe("Format, Borders and shading", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  const field = (dlg: Locator, label: string) => dlg.locator(".field").filter({ has: dlg.page().locator("label", { hasText: new RegExp(`^${label}$`) }) });
  const css = (page: Page, id: number) =>
    para(page, id).evaluate((el) => {
      const c = getComputedStyle(el);
      return {
        top: [c.borderTopStyle, c.borderTopWidth, c.borderTopColor],
        bottom: [c.borderBottomStyle, c.borderBottomWidth],
        left: [c.borderLeftStyle, c.borderLeftWidth],
        right: [c.borderRightStyle, c.borderRightWidth],
        background: c.backgroundColor,
      };
    });

  test("Box, width, style and colours apply, and the dialog opens with them again", async ({ page }) => {
    await caretIn(page, RICH.intro);
    await chooseMenu(page, "Format", "Borders and shading…");
    const dlg = dialog(page, "Borders and shading");
    await expect(dlg).toBeVisible();
    // Nothing yet.
    for (const side of ["Top", "Bottom", "Left", "Right"]) await expect(dlg.getByRole("button", { name: side, exact: true })).toHaveAttribute("aria-pressed", "false");

    await dlg.locator(".bs-row").getByRole("button", { name: "Box" }).click();
    for (const side of ["Top", "Bottom", "Left", "Right"]) await expect(dlg.getByRole("button", { name: side, exact: true })).toHaveAttribute("aria-pressed", "true");
    await dlg.getByRole("radio", { name: "2¼ pt" }).click();
    await dlg.getByRole("radio", { name: "Dashed" }).click();
    await field(dlg, "Border color").getByRole("button", { name: /^Red #FF0000$/ }).click();
    await field(dlg, "Background color").getByRole("button", { name: /^Yellow #FFFF00$/ }).click();
    // The preview draws what will be applied.
    await expect(dlg.locator(".bs-sample")).toHaveCSS("border-top-style", "dashed");
    await expect(dlg.locator(".bs-sample")).toHaveCSS("background-color", "rgb(255, 255, 0)");
    await dlg.getByRole("button", { name: "Apply" }).click();
    await expect(dlg).toBeHidden();

    const fmt = await paraFmt(page, RICH.intro);
    const borders = fmt.borders as Record<string, { val: string; sz: number; color: string; space: number }>;
    expect(Object.keys(borders).sort()).toEqual(["bottom", "left", "right", "top"]);
    expect(borders.top).toEqual({ val: "dashed", sz: 18, color: "FF0000", space: 1 });
    expect(borders.left).toEqual({ val: "dashed", sz: 18, color: "FF0000", space: 4 });
    expect((fmt.shading as string).toUpperCase()).toBe("FFFF00");
    const drawn = await css(page, RICH.intro);
    expect(drawn.top).toEqual(["dashed", "3px", "rgb(255, 0, 0)"]);
    expect(drawn.left[0]).toBe("dashed");
    expect(drawn.background).toBe("rgb(255, 255, 0)");

    // It opens with the paragraph's values.
    await chooseMenu(page, "Format", "Borders and shading…");
    for (const side of ["Top", "Bottom", "Left", "Right"]) await expect(dlg.getByRole("button", { name: side, exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(dlg.getByRole("radio", { name: "2¼ pt" })).toHaveAttribute("aria-checked", "true");
    await expect(dlg.getByRole("radio", { name: "Dashed" })).toHaveAttribute("aria-checked", "true");
    await expect(field(dlg, "Border color").getByRole("button", { name: /^Red #FF0000$/ })).toHaveAttribute("aria-pressed", "true");
    await expect(field(dlg, "Background color").getByRole("button", { name: /^Yellow #FFFF00$/ })).toHaveAttribute("aria-pressed", "true");

    // None and no background take it all away.
    await dlg.locator(".bs-row").getByRole("button", { name: "None" }).click();
    await field(dlg, "Background color").getByRole("button", { name: "None" }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    const gone = await paraFmt(page, RICH.intro);
    expect(Object.keys((gone.borders as object | undefined) ?? {})).toHaveLength(0);
    expect(gone.shading ?? "").toBe("");
    const plain = await css(page, RICH.intro);
    expect(plain.top[0]).toBe("none");
    expect(plain.background).toBe("rgba(0, 0, 0, 0)");
  });

  test("a border sits outside the text: adding sides does not move the lines", async ({ page }) => {
    const lines = () =>
      para(page, RICH.intro).evaluate((el) => {
        const w = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        const r = document.createRange();
        r.selectNodeContents(w.nextNode()!);
        const box = r.getBoundingClientRect();
        return { left: box.left, top: box.top, right: el.getBoundingClientRect().right };
      });
    const plain = await lines();
    await inPage(page, (w, id) => {
      const side = { val: "single", sz: 8, color: "auto", space: 4 };
      return w.fe.editor.exec({ op: "format_paragraphs", ids: [id], format: { borders: { left: side, right: side, top: { ...side, space: 1 } } } });
    }, RICH.intro);
    const boxed = await lines();
    expect(Math.abs(boxed.left - plain.left)).toBeLessThan(0.5);
    // The box reaches out past the text on the right (4 pt and the line itself).
    expect(boxed.right - plain.right).toBeGreaterThan(5);
    // Only top and bottom add height.
    expect(boxed.top - plain.top).toBeGreaterThan(1);
    const css = await para(page, RICH.intro).evaluate((el) => ({ ml: getComputedStyle(el).marginLeft, pl: getComputedStyle(el).paddingLeft }));
    expect(parseFloat(css.ml)).toBeLessThan(-5);
    expect(parseFloat(css.pl)).toBeGreaterThan(5);
  });

  test("a single side, and Escape or Cancel leave the paragraph alone", async ({ page }) => {
    await caretIn(page, RICH.intro);
    await chooseMenu(page, "Format", "Borders and shading…");
    const dlg = dialog(page, "Borders and shading");
    await dlg.getByRole("button", { name: "Bottom", exact: true }).click();
    await dlg.getByRole("button", { name: "Cancel" }).click();
    expect(Object.keys(((await paraFmt(page, RICH.intro)).borders as object | undefined) ?? {})).toHaveLength(0);

    await chooseMenu(page, "Format", "Borders and shading…");
    await dlg.getByRole("button", { name: "Bottom", exact: true }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    const borders = (await paraFmt(page, RICH.intro)).borders as Record<string, { val: string; sz: number; color: string }>;
    expect(Object.keys(borders)).toEqual(["bottom"]);
    // Word's own defaults: a solid hairline of 1 pt in the automatic colour.
    expect(borders.bottom).toMatchObject({ val: "single", sz: 8, color: "auto" });
    const drawn = await css(page, RICH.intro);
    expect(drawn.bottom[0]).toBe("solid");
    expect(drawn.top[0]).toBe("none");
  });

  test("paragraphs with identical borders form one box", async ({ page }) => {
    await typeLines(page, RICH.last, ["One", "Two", "Three"]);
    await page.keyboard.press("Shift+ArrowUp");
    await page.keyboard.press("Shift+ArrowUp");
    await chooseMenu(page, "Format", "Borders and shading…");
    const dlg = dialog(page, "Borders and shading");
    await dlg.locator(".bs-row").getByRole("button", { name: "Box" }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();

    const ids = [] as number[];
    for (const w of ["One", "Two", "Three"]) ids.push(Number(await page.locator(".flow .fp").filter({ hasText: new RegExp(`^${w}$`) }).getAttribute("data-p")));
    const [one, two, three] = await Promise.all(ids.map((id) => css(page, id)));
    // One box: the top line on the first, the bottom line on the last, the sides on all.
    expect(one.top[0]).toBe("solid");
    expect(one.bottom[0]).toBe("none");
    expect(two.top[0]).toBe("none");
    expect(two.bottom[0]).toBe("none");
    expect(three.top[0]).toBe("none");
    expect(three.bottom[0]).toBe("solid");
    for (const c of [one, two, three]) {
      expect(c.left[0]).toBe("solid");
      expect(c.right[0]).toBe("solid");
    }

    // Take the border off the last one: the one before it now ends the box, though it was not drawn again by the edit.
    await caretIn(page, ids[2]);
    await chooseMenu(page, "Format", "Borders and shading…");
    await dlg.locator(".bs-row").getByRole("button", { name: "None" }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    expect((await css(page, ids[1])).bottom[0]).toBe("solid");
    expect((await css(page, ids[2])).top[0]).toBe("none");

    // And back: one box of three again.
    await page.keyboard.press(`${MOD}+Z`);
    expect((await css(page, ids[1])).bottom[0]).toBe("none");
    expect((await css(page, ids[2])).bottom[0]).toBe("solid");
  });
});

test.describe("Page color and Page setup: Apply to", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("a page color paints the sheets, is saved, and None takes it away", async ({ page }) => {
    // The page color is in Format, Page color… (with the fill effects), not in Page setup.
    await chooseMenu(page, "Format", "Page color…");
    const dlg = dialog(page, "Page color");
    await dlg.getByRole("button", { name: "#FFF9E6" }).click();
    await expect(dlg.getByRole("button", { name: "#FFF9E6" })).toHaveAttribute("aria-pressed", "true");
    await dlg.getByRole("button", { name: "Apply" }).click();

    await expect(page.locator(".sheet").first()).toHaveCSS("background-color", "rgb(255, 249, 230)");
    await expect(page.locator(".sheet").nth(1)).toHaveCSS("background-color", "rgb(255, 249, 230)");
    expect((await setup(page)).pageColor?.toUpperCase()).toBe("FFF9E6");
    // It reaches the saved file: opening the saved bytes again shows the same color.
    const reopened = await inPage(page, async (w) => {
      await w.fe.open(w.engine!.save(), "colored.docx");
      return w.fe.editor.engine!.pageSetup().pageColor;
    });
    expect(reopened?.toUpperCase()).toBe("FFF9E6");
    await expect(page.locator(".sheet").first()).toHaveCSS("background-color", "rgb(255, 249, 230)");

    // And the printed copy.
    await page.evaluate(() => (window.print = () => undefined));
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .sheet").first()).toHaveCSS("background-color", "rgb(255, 249, 230)");
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));

    await chooseMenu(page, "Format", "Page color…");
    await expect(dlg.getByRole("button", { name: "#FFF9E6" })).toHaveAttribute("aria-pressed", "true");
    await dlg.getByRole("button", { name: "None" }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    await expect(page.locator(".sheet").first()).toHaveCSS("background-color", "rgb(255, 255, 255)");
    expect((await setup(page)).pageColor).toBeFalsy();
  });

  test("with several sections the dialog shows the caret's section and can change just that one", async ({ page }) => {
    await caretAtEnd(page, RICH.title);
    await chooseMenu(page, "Insert", "Break", "Section break (continuous)");
    expect((await setup(page)).sections).toHaveLength(2);

    // Caret in the first section.
    await caretIn(page, RICH.title);
    await chooseMenu(page, "File", "Page setup…");
    const dlg = dialog(page, "Page setup");
    await expect(dlg.getByText("Apply to", { exact: true })).toBeVisible();
    await expect(dlg.getByRole("radio", { name: "Whole document" })).toHaveAttribute("aria-checked", "true");
    await dlg.getByRole("radio", { name: "This section" }).click();
    await expect(dlg.getByRole("radio", { name: "This section" })).toHaveAttribute("aria-checked", "true");
    await dlg.getByRole("spinbutton", { name: "Top margin (inches)" }).fill("2");
    await dlg.getByRole("button", { name: "Apply" }).click();
    let sections = (await setup(page)).sections;
    expect(sections[0].margin[0]).toBe(2880);
    expect(sections[1].margin[0]).toBe(1440);

    // It opens with the section the caret is in.
    await chooseMenu(page, "File", "Page setup…");
    await expect(dlg.getByRole("spinbutton", { name: "Top margin (inches)" })).toHaveValue("2");
    await page.keyboard.press("Escape");
    await caretIn(page, RICH.last);
    await chooseMenu(page, "File", "Page setup…");
    await expect(dlg.getByRole("spinbutton", { name: "Top margin (inches)" })).toHaveValue("1");
    // The whole document is the default.
    await dlg.getByRole("spinbutton", { name: "Top margin (inches)" }).fill("0.5");
    await dlg.getByRole("button", { name: "Apply" }).click();
    sections = (await setup(page)).sections;
    expect(sections.map((s) => s.margin[0])).toEqual([720, 720]);
  });
});

test.describe("Tools, Hyphenation", () => {
  test("turns hyphens:auto on for the text and the printed copy, and off again", async ({ page }) => {
    await openFixture(page);
    const stage = page.locator(".docxtor .stage").first();
    await expect(stage).not.toHaveCSS("hyphens", "auto");
    await openMenu(page, "Tools");
    await expect(menuItem(page, "Hyphenation")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Hyphenation").click();

    expect((await setup(page)).hyphenation).toBe(true);
    await expect(stage).toHaveCSS("hyphens", "auto");
    await expect(stage).toHaveAttribute("lang", /^en/);
    await expect(para(page, RICH.intro)).toHaveCSS("hyphens", "auto");

    // A paragraph can opt out.
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_paragraphs", ids: [id], format: { suppressAutoHyphens: true } }), RICH.intro);
    await expect(para(page, RICH.intro)).toHaveAttribute("data-nh", "1");
    await expect(para(page, RICH.intro)).toHaveCSS("hyphens", "manual");

    await page.evaluate(() => (window.print = () => undefined));
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .stage").first()).toHaveCSS("hyphens", "auto");
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));

    await openMenu(page, "Tools");
    await expect(menuItem(page, "Hyphenation")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Hyphenation").click();
    expect((await setup(page)).hyphenation).toBe(false);
    await expect(stage).not.toHaveCSS("hyphens", "auto");
  });
});

test.describe("Tools, Line numbers", () => {
  const numbers = (page: Page, sheet = 0) => page.locator(".sheet").nth(sheet).locator(".lnum span");
  const texts = async (page: Page, sheet = 0) => numbers(page, sheet).allTextContents();
  const dlgName = "Line numbers";

  async function show(page: Page, opts: { restart?: string; countBy?: string; scope?: string } = {}) {
    await chooseMenu(page, "Tools", "Line numbers…");
    const dlg = dialog(page, dlgName);
    const box = dlg.getByRole("checkbox", { name: "Show line numbers" });
    if (!(await box.isChecked())) await box.check();
    if (opts.restart) await dlg.getByRole("radio", { name: opts.restart }).click();
    if (opts.countBy) await dlg.getByRole("spinbutton", { name: "Count by" }).fill(opts.countBy);
    if (opts.scope) await dlg.getByRole("radio", { name: opts.scope }).click();
    await dlg.getByRole("button", { name: "Apply" }).click();
    await expect(dlg).toBeHidden();
  }

  test("numbers every line beside the text, in step with the lines", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await typeLines(page, 1, ["Second line", "Third line", "Fourth line", "Fifth line"]);
    await expect(numbers(page)).toHaveCount(0);

    await show(page);
    expect(await texts(page)).toEqual(["1", "2", "3", "4", "5"]);
    expect((await setup(page)).sections[0].lineNumbers).toMatchObject({ countBy: 1, restart: "newPage" });
    // Each number sits on its line, to the left of the text, and is not part of the text.
    const nums = numbers(page);
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    for (let i = 0; i < 5; i++) {
      const p = (await page.locator(".flow .fp").nth(i).boundingBox())!;
      const n = (await nums.nth(i).boundingBox())!;
      expect(Math.abs(n.y + n.height / 2 - (p.y + p.height / 2))).toBeLessThan(3);
      const gap = p.x - (n.x + n.width);
      expect(gap).toBeGreaterThan(20 * zoom);
      expect(gap).toBeLessThan(28 * zoom);
    }
    await expect(page.locator(".sheet").first().locator(".lnum")).toHaveAttribute("aria-hidden", "true");
    expect(await inPage(page, (w) => w.engine!.text())).not.toMatch(/\b1\b.*Second/s);

    // Every number has the same size and the document's text color, whatever its line is.
    const looks = await numbers(page).evaluateAll((els) => els.map((e) => `${getComputedStyle(e).fontSize} ${getComputedStyle(e).color}`));
    expect(new Set(looks).size).toBe(1);

    // Count by two shows every second number, while all lines still count.
    await show(page, { countBy: "2" });
    expect(await texts(page)).toEqual(["2", "4"]);

    // Printing shows them.
    await page.evaluate(() => (window.print = () => undefined));
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .lnum span").first()).toBeAttached();
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));

    // Off again.
    await chooseMenu(page, "Tools", "Line numbers…");
    const dlg = dialog(page, dlgName);
    await expect(dlg.getByRole("checkbox", { name: "Show line numbers" })).toBeChecked();
    await dlg.getByRole("checkbox", { name: "Show line numbers" }).uncheck();
    await dlg.getByRole("button", { name: "Apply" }).click();
    await expect(numbers(page)).toHaveCount(0);
    expect((await setup(page)).sections[0].lineNumbers).toBeUndefined();
  });

  test("numbers are one size even beside a heading", async ({ page }) => {
    await openFixture(page);
    await show(page);
    const sizes = await numbers(page).evaluateAll((els) => els.map((e) => getComputedStyle(e).fontSize));
    expect(sizes.length).toBeGreaterThan(10);
    expect(new Set(sizes).size).toBe(1);
    const colors = await numbers(page).evaluateAll((els) => els.map((e) => getComputedStyle(e).color));
    expect(new Set(colors)).toEqual(new Set(["rgb(0, 0, 0)"]));
  });

  test("Suppress line numbers takes a paragraph out of the count", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await typeLines(page, 1, ["Second line", "Third line"]);
    await show(page);
    expect(await texts(page)).toEqual(["1", "2", "3"]);

    await caretIn(page, Number(await page.locator(".flow .fp").nth(1).getAttribute("data-p")));
    await openMenu(page, "Tools");
    await expect(menuItem(page, "Suppress line numbers")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Suppress line numbers").click();
    expect(await texts(page)).toEqual(["1", "2"]);
    // The numbers sit on the first and third lines.
    const ys = await Promise.all([0, 1].map(async (i) => (await numbers(page).nth(i).boundingBox())!.y));
    const lines = await Promise.all([0, 2].map(async (i) => (await page.locator(".flow .fp").nth(i).boundingBox())!.y));
    expect(Math.abs(ys[0] - lines[0])).toBeLessThan(4);
    expect(Math.abs(ys[1] - lines[1])).toBeLessThan(4);

    await openMenu(page, "Tools");
    await expect(menuItem(page, "Suppress line numbers")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Suppress line numbers").click();
    expect(await texts(page)).toEqual(["1", "2", "3"]);
  });

  test("Each page starts again on every page; Continuous does not", async ({ page }) => {
    await openFixture(page, "long.docx");
    await show(page, { restart: "Each page" });
    expect((await texts(page, 0))[0]).toBe("1");
    expect((await texts(page, 1))[0]).toBe("1");
    expect((await texts(page, 2))[0]).toBe("1");

    await show(page, { restart: "Continuous" });
    const first = await texts(page, 0);
    const next = await texts(page, 1);
    expect(first[0]).toBe("1");
    expect(Number(next[0])).toBe(Number(first[first.length - 1]) + 1);
  });

  test("Each section starts again in a new section, and This section numbers only that one", async ({ page }) => {
    await openFixture(page, "long.docx");
    await caretAtEnd(page, 2);
    await chooseMenu(page, "Insert", "Break", "Section break (next page)");
    expect((await setup(page)).sections).toHaveLength(2);
    // The caret is at the start of section 2: number just that section, restarting with it.
    await show(page, { restart: "Each section", scope: "This section" });
    const sections = (await setup(page)).sections;
    expect(sections[0].lineNumbers).toBeUndefined();
    expect(sections[1].lineNumbers).toMatchObject({ restart: "newSection" });
    await expect(numbers(page, 0)).toHaveCount(0);
    expect((await texts(page, 1))[0]).toBe("1");
    // It carries on over the next page instead of starting again.
    const last = await texts(page, 1);
    expect(Number((await texts(page, 2))[0])).toBe(Number(last[last.length - 1]) + 1);
  });

  test("the dialog opens with the section's values", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await show(page, { restart: "Each section", countBy: "3" });
    await chooseMenu(page, "Tools", "Line numbers…");
    const dlg = dialog(page, dlgName);
    await expect(dlg.getByRole("checkbox", { name: "Show line numbers" })).toBeChecked();
    await expect(dlg.getByRole("radio", { name: "Each section" })).toHaveAttribute("aria-checked", "true");
    await expect(dlg.getByRole("spinbutton", { name: "Count by" })).toHaveValue("3");
    await page.keyboard.press("Escape");
    await expect(dlg).toBeHidden();
  });
});

test.describe("vertical ruler", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  const vruler = (page: Page) => page.locator(".vruler");

  test("shows with the horizontal ruler, stands beside the caret's page and follows the caret", async ({ page }) => {
    await expect(page.locator(".ruler")).toBeVisible();
    await expect(vruler(page)).toBeVisible();
    await caretIn(page, RICH.title);
    const sheet = (n: number) => page.locator(".sheet").nth(n);
    await expect.poll(async () => Math.abs((await vruler(page).boundingBox())!.y - (await sheet(0).boundingBox())!.y)).toBeLessThan(1);
    // Its left edge is left of the page, and it is as tall as the page.
    const v = (await vruler(page).boundingBox())!;
    const s0 = (await sheet(0).boundingBox())!;
    expect(v.x + v.width).toBeLessThanOrEqual(s0.x);
    expect(Math.abs(v.height - s0.height)).toBeLessThan(1);

    await caretIn(page, RICH.last);
    await expect.poll(async () => Math.abs((await vruler(page).boundingBox())!.y - (await sheet(1).boundingBox())!.y)).toBeLessThan(1);

    // Hidden together with the horizontal one.
    await chooseMenu(page, "View", "Show ruler");
    await expect(page.locator(".ruler")).toBeHidden();
    await expect(vruler(page)).toBeHidden();
    await chooseMenu(page, "View", "Show ruler");
    await expect(vruler(page)).toBeVisible();
  });

  test("moves to the first page in view when the caret's page is scrolled away, and follows the zoom", async ({ page }) => {
    await caretIn(page, RICH.title);
    const sheet = (n: number) => page.locator(".sheet").nth(n);
    await page.locator(".canvas").evaluate((el) => (el.scrollTop = el.scrollHeight));
    await expect.poll(async () => Math.abs((await vruler(page).boundingBox())!.y - (await sheet(1).boundingBox())!.y)).toBeLessThan(1);

    await caretIn(page, RICH.title);
    await expect.poll(async () => Math.abs((await vruler(page).boundingBox())!.y - (await sheet(0).boundingBox())!.y)).toBeLessThan(1);
    const before = (await vruler(page).boundingBox())!.height;
    await chooseMenu(page, "View", "Zoom", "Zoom in");
    await expect.poll(async () => (await vruler(page).boundingBox())!.height).toBeGreaterThan(before * 1.05);
    const v = (await vruler(page).boundingBox())!;
    const s0 = (await sheet(0).boundingBox())!;
    expect(Math.abs(v.height - s0.height)).toBeLessThan(1);
    expect(v.x + v.width).toBeLessThanOrEqual(s0.x);
  });

  test("shows the margins, and dragging the top boundary changes the top margin with a guide line", async ({ page }) => {
    await caretIn(page, RICH.title);
    const ps = await setup(page);
    expect(ps.margin[0]).toBe(1440);
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    // The top margin strip is as tall as the margin.
    const top = (await vruler(page).locator(".vruler-margin").first().boundingBox())!;
    expect(Math.abs(top.height - (1440 / 15) * zoom)).toBeLessThan(1.5);

    const handle = (await vruler(page).locator(".vruler-handle").first().boundingBox())!;
    const x = handle.x + handle.width / 2;
    const y = handle.y + handle.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y + 24 * zoom, { steps: 4 });
    await expect(page.locator(".vruler-guide")).toHaveCount(1);
    await page.mouse.move(x, y + 48 * zoom, { steps: 4 });
    await page.mouse.up();
    await expect(page.locator(".vruler-guide")).toHaveCount(0);

    const after = await setup(page);
    // 48 screen pixels at this zoom, snapped to eighths of an inch.
    expect(after.margin[0]).toBeGreaterThan(1440 + 600);
    expect(after.margin[0]).toBeLessThan(1440 + 800);
    expect(after.margin[2]).toBe(1440);
    // The pages follow: the text now starts lower.
    await expect(page.locator(".flow")).toHaveCSS("padding-top", `${after.margin[0] / 15}px`);
    // And it can be undone.
    await page.keyboard.press(`${MOD}+Z`);
    expect((await setup(page)).margin[0]).toBe(1440);
  });

  test("dragging the bottom boundary changes the bottom margin", async ({ page }) => {
    await caretIn(page, RICH.title);
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    await vruler(page).locator(".vruler-handle").nth(1).scrollIntoViewIfNeeded();
    const handle = (await vruler(page).locator(".vruler-handle").nth(1).boundingBox())!;
    const x = handle.x + handle.width / 2;
    const y = handle.y + handle.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y - 48 * zoom, { steps: 6 });
    await page.mouse.up();
    const after = await setup(page);
    expect(after.margin[2]).toBeGreaterThan(1440 + 600);
    expect(after.margin[0]).toBe(1440);
  });

  test("it is not drawn on a narrow screen", async ({ page }) => {
    await page.setViewportSize({ width: 700, height: 900 });
    await expect(vruler(page)).toBeHidden();
  });
});
