// Insert: horizontal lines, bookmarks (markers in the margin, the popover, the Bookmarks dialog, links to them)
// and watermarks (the dialog, the drawing behind the text of every page, printing, ones made in Word).
import { buildDocx, openBuilt } from "./support/docx-builder";
import { CHROMIUM_ONLY, answer, caretAtEnd, chooseMenu, dialog, expect, inPage, type Locator, menuItem, openFixture, openMenu, type Page, para, paraText, RICH, test } from "./helpers";

const MOD = "ControlOrMeta";

interface P {
  id: number;
  hr: boolean;
  len: number;
}
/** The paragraphs of the body as the engine has them. */
const paragraphs = (page: Page): Promise<P[]> =>
  inPage(page, (w) =>
    w.fe.editor.engine!
      .renderAll()
      .blocks.filter((b) => b.t === "p")
      .map((b) => ({ id: b.id, hr: !!b.hr, len: b.len ?? 0 })),
  );
const bookmarks = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.bookmarks());
const watermark = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup().watermark);
const selection = (page: Page) => inPage(page, (w) => w.fe.editor.sel);

/** Put the caret in a paragraph at an offset (focus stays in the document). */
const caretAt = (page: Page, p: number, o: number) =>
  inPage(
    page,
    (w, a) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
      e.select({ p: a.p, o: a.o });
      e.focus();
    },
    { p, o },
  );
/** Select a stretch of one paragraph. */
const selectText = (page: Page, p: number, from: number, to: number) =>
  inPage(
    page,
    (w, a) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
      e.select({ anchor: { p: a.p, o: a.from }, focus: { p: a.p, o: a.to } });
      e.focus();
    },
    { p, from, to },
  );
/** Where a phrase starts in a paragraph. */
const offsetOf = async (page: Page, p: number, phrase: string) => (await paraText(page, p)).indexOf(phrase);

const rules = (page: Page) => page.locator(".flow .fp[data-hr]");
const markers = (page: Page) => page.locator(".bm-mark");
const popover = (page: Page) => page.locator(".pop .bm-pop");

test.describe("horizontal line", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("Insert, Horizontal line splits a paragraph, draws a rule across the text and puts the caret after it; one undo takes it back", async ({ page }) => {
    const before = await paragraphs(page);
    const text = await paraText(page, RICH.intro);
    const at = await offsetOf(page, RICH.intro, "bold progress");
    await caretAt(page, RICH.intro, at);
    await chooseMenu(page, "Insert", "Horizontal line");

    const after = await paragraphs(page);
    expect(after).toHaveLength(before.length + 2);
    const i = after.findIndex((p) => p.hr);
    expect(after.filter((p) => p.hr)).toHaveLength(1);
    expect(after[i].len).toBe(0);
    expect(after[i - 1].len).toBe(at);
    expect(after[i + 1].len).toBe(text.length - at);
    await expect(rules(page)).toHaveCount(1);

    // A rule, not an empty box: only the bottom border, as wide as the text, no taller than a line.
    const rule = await rules(page).evaluate((el) => {
      const cs = getComputedStyle(el);
      const r = el.getBoundingClientRect();
      const next = el.nextElementSibling!.getBoundingClientRect();
      return { bottom: parseFloat(cs.borderBottomWidth), top: parseFloat(cs.borderTopWidth), left: parseFloat(cs.borderLeftWidth), right: parseFloat(cs.borderRightWidth), height: r.height, width: r.width, left0: r.left, nextLeft: next.left, nextWidth: next.width };
    });
    expect(rule.bottom).toBeGreaterThan(0);
    expect([rule.top, rule.left, rule.right]).toEqual([0, 0, 0]);
    expect(rule.height).toBeLessThan(30);
    expect(rule.width).toBeCloseTo(rule.nextWidth, 0);
    expect(rule.left0).toBeCloseTo(rule.nextLeft, 0);

    // The caret is at the start of the text after the line.
    expect(await selection(page)).toEqual({ anchor: { p: after[i + 1].id, o: 0 }, focus: { p: after[i + 1].id, o: 0 } });
    await page.keyboard.type("X");
    expect((await paraText(page, after[i + 1].id)).startsWith("Xbold")).toBe(true);
    await page.keyboard.press(`${MOD}+Z`);
    await page.keyboard.press(`${MOD}+Z`);
    expect(await paragraphs(page)).toEqual(before);
    await expect(rules(page)).toHaveCount(0);
    expect(await paraText(page, RICH.intro)).toBe(text);
  });

  test("at the start of a paragraph the line goes before it, at the end after it; the caret goes to the text after the line", async ({ page }) => {
    const before = await paragraphs(page);
    const len = before.find((p) => p.id === RICH.intro)!.len;
    await caretAt(page, RICH.intro, 0);
    await chooseMenu(page, "Insert", "Horizontal line");
    let after = await paragraphs(page);
    expect(after).toHaveLength(before.length + 1);
    const at = after.findIndex((p) => p.hr);
    expect(after[at + 1]).toEqual({ id: RICH.intro, hr: false, len });
    expect((await selection(page))!.focus).toEqual({ p: RICH.intro, o: 0 });
    await page.keyboard.press(`${MOD}+Z`);
    expect(await paragraphs(page)).toEqual(before);

    await caretAt(page, RICH.intro, len);
    await chooseMenu(page, "Insert", "Horizontal line");
    after = await paragraphs(page);
    const j = after.findIndex((p) => p.hr);
    expect(after).toHaveLength(before.length + 1);
    expect(after[j - 1]).toEqual({ id: RICH.intro, hr: false, len });
    expect(after[j + 1]).toEqual(before[before.findIndex((p) => p.id === RICH.intro) + 1]);
    expect((await selection(page))!.focus).toEqual({ p: after[j + 1].id, o: 0 });
    await page.keyboard.press(`${MOD}+Z`);
    expect(await paragraphs(page)).toEqual(before);

    // At the end of the last paragraph there is no text after the line: an empty paragraph is made for the caret.
    const last = before[before.length - 1];
    await caretAt(page, last.id, last.len);
    await chooseMenu(page, "Insert", "Horizontal line");
    const end = await paragraphs(page);
    expect(end).toHaveLength(before.length + 2);
    expect(end.slice(-2).map((p) => [p.hr, p.len])).toEqual([[true, 0], [false, 0]]);
    expect((await selection(page))!.focus).toEqual({ p: end[end.length - 1].id, o: 0 });
  });

  test("a selection is replaced by the line, in one undo step", async ({ page }) => {
    const text = await paraText(page, RICH.intro);
    const from = await offsetOf(page, RICH.intro, "bold progress");
    await selectText(page, RICH.intro, from, from + "bold progress".length);
    await chooseMenu(page, "Insert", "Horizontal line");
    await expect(rules(page)).toHaveCount(1);
    expect(await paraText(page, RICH.intro)).not.toContain("bold progress");
    await page.keyboard.press(`${MOD}+Z`);
    await expect(rules(page)).toHaveCount(0);
    expect(await paraText(page, RICH.intro)).toBe(text);
  });

  test("Backspace right after the line takes the line away and keeps the text; Backspace and Delete on the line do too", async ({ page }) => {
    const at = await offsetOf(page, RICH.intro, "bold progress");
    const insert = async () => {
      await caretAt(page, RICH.intro, at);
      await chooseMenu(page, "Insert", "Horizontal line");
      await expect(rules(page)).toHaveCount(1);
    };
    await insert();
    const withLine = await paragraphs(page);
    const i = withLine.findIndex((p) => p.hr);
    // After the line: Backspace removes the line, not the text.
    await page.keyboard.press("Backspace");
    await expect(rules(page)).toHaveCount(0);
    let now = await paragraphs(page);
    expect(now).toHaveLength(withLine.length - 1);
    expect(now.map((p) => p.len)).toEqual(withLine.filter((p) => !p.hr).map((p) => p.len));
    expect(await paraText(page, withLine[i + 1].id)).toMatch(/^bold progress/);

    // On the line: Backspace joins it to the paragraph before; the text before is untouched.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(rules(page)).toHaveCount(1);
    await caretAt(page, withLine[i].id, 0);
    await page.keyboard.press("Backspace");
    await expect(rules(page)).toHaveCount(0);
    now = await paragraphs(page);
    expect(now.find((p) => p.id === withLine[i - 1].id)!.len).toBe(at);
    expect(await paraText(page, withLine[i + 1].id)).toMatch(/^bold progress/);

    // On the line: Delete takes it away and the text after it stays as it was.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(rules(page)).toHaveCount(1);
    await caretAt(page, withLine[i].id, 0);
    await page.keyboard.press("Delete");
    await expect(rules(page)).toHaveCount(0);
    expect(await paraText(page, withLine[i + 1].id)).toMatch(/^bold progress/);
    expect((await paragraphs(page)).find((p) => p.id === withLine[i + 1].id)!.hr).toBe(false);
  });

  test("the menu item has an icon and the command is in the palette", async ({ page }) => {
    await openMenu(page, "Insert");
    await expect(menuItem(page, "Horizontal line").locator(".menu-icon svg")).toHaveCount(1);
    await page.keyboard.press("Escape");
    expect(await inPage(page, (w) => (w as unknown as { fe: { extensions: { commands: { list(): { id: string; ai?: boolean }[] } } } }).fe.extensions.commands.list().find((c) => c.id === "insert.horizontalLine")?.ai)).toBe(true);
  });
});

test.describe("bookmarks", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  /** Select "live numbers" in the introduction and add a bookmark from the menu. */
  async function addOnPhrase(page: Page) {
    const from = await offsetOf(page, RICH.intro, "live numbers");
    await selectText(page, RICH.intro, from, from + "live numbers".length);
    await chooseMenu(page, "Insert", "Bookmark", "Add bookmark");
    return from;
  }

  test("Insert, Bookmark, Add bookmark names the selection after its text and shows its marker and popover", async ({ page }) => {
    const rect = (await para(page, RICH.intro).boundingBox())!;
    const html = await para(page, RICH.intro).innerHTML();
    const from = await addOnPhrase(page);
    expect(await bookmarks(page)).toEqual([{ name: "live_numbers", p: RICH.intro, o: from, endP: RICH.intro, endO: from + 12, text: "live numbers" }]);

    // A ribbon in the margin of the line, drawn over the pages and not in the text.
    const marker = markers(page);
    await expect(marker).toHaveCount(1);
    await expect(marker).toHaveAttribute("aria-label", "Bookmark live_numbers");
    expect(await page.locator(".flow .bm-mark").count()).toBe(0);
    expect(await marker.evaluate((el) => !!el.closest(".bm-layer") && getComputedStyle(el).position)).toBe("absolute");
    const m = (await marker.boundingBox())!;
    expect(m.x + m.width).toBeLessThanOrEqual(rect.x + 1);
    // Zero width: the paragraph is exactly where it was and its markup has no new node.
    expect(await para(page, RICH.intro).boundingBox()).toEqual(rect);
    expect(await para(page, RICH.intro).innerHTML()).toBe(html);
    expect(await para(page, RICH.intro).getAttribute("data-bm")).toBe("live_numbers");

    // The popover: the name, Copy link, Remove.
    await expect(popover(page)).toBeVisible();
    await expect(popover(page).getByRole("textbox", { name: "Bookmark name" })).toHaveValue("live_numbers");
    await expect(popover(page).getByRole("button", { name: "Copy link" })).toBeVisible();
    await expect(popover(page).getByRole("button", { name: "Remove" })).toBeVisible();
    // The selection stays on the text.
    expect((await selection(page))!.anchor).toEqual({ p: RICH.intro, o: from });
  });

  test("at a caret the names are Bookmark1, Bookmark2, and the marker is not printed", async ({ page }) => {
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await caretAt(page, RICH.intro, 0);
    await chooseMenu(page, "Insert", "Bookmark", "Add bookmark");
    await page.keyboard.press("Escape");
    await caretAt(page, RICH.quote, 3);
    await chooseMenu(page, "Insert", "Bookmark", "Add bookmark");
    await page.keyboard.press("Escape");
    expect((await bookmarks(page)).map((b) => [b.name, b.p, b.o, b.endP, b.endO, b.text])).toEqual([
      ["Bookmark1", RICH.intro, 0, RICH.intro, 0, expect.any(String)],
      ["Bookmark2", RICH.quote, 3, RICH.quote, 3, expect.any(String)],
    ]);
    await expect(markers(page)).toHaveCount(2);

    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .print-page")).toHaveCount(2);
    await expect(page.locator(".docxtor-print .bm-layer, .docxtor-print .bm-mark")).toHaveCount(0);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
    await expect(markers(page)).toHaveCount(2);
  });

  test("the popover renames the bookmark: a bad name shows the engine's message, a good one is kept", async ({ page }) => {
    await addOnPhrase(page);
    const name = popover(page).getByRole("textbox", { name: "Bookmark name" });
    await name.fill("1bad");
    await name.press("Enter");
    await expect(popover(page).getByRole("alert")).toHaveText("A bookmark name has to start with a letter.");
    await expect(name).toHaveAttribute("aria-invalid", "true");
    expect((await bookmarks(page)).map((b) => b.name)).toEqual(["live_numbers"]);

    await name.fill("has space");
    await name.press("Enter");
    await expect(popover(page).getByRole("alert")).toContainText("can't have spaces");

    await name.fill("Dashboard_numbers");
    await expect(popover(page).getByRole("alert")).toBeHidden();
    await name.press("Enter");
    await expect(popover(page)).toBeHidden();
    expect((await bookmarks(page)).map((b) => b.name)).toEqual(["Dashboard_numbers"]);
    await expect(markers(page)).toHaveAttribute("data-name", "Dashboard_numbers");
    await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
  });

  test("clicking the marker opens the popover; Copy link copies #name; Remove deletes the bookmark and undo brings it back", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await addOnPhrase(page);
    await page.keyboard.press("Escape");
    await expect(popover(page)).toBeHidden();
    await markers(page).click();
    await expect(popover(page)).toBeVisible();
    await popover(page).getByRole("button", { name: "Copy link" }).click();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("#live_numbers");
    await expect(page.locator(".toast")).toHaveText("Link copied");

    await markers(page).click();
    await popover(page).getByRole("button", { name: "Remove" }).click();
    expect(await bookmarks(page)).toEqual([]);
    await expect(markers(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    expect((await bookmarks(page)).map((b) => b.name)).toEqual(["live_numbers"]);
    await expect(markers(page)).toHaveCount(1);
  });

  test("the marker follows its line when the text above changes, and hidden bookmarks (_Toc…) have none", async ({ page }) => {
    await addOnPhrase(page);
    await page.keyboard.press("Escape");
    const y0 = (await markers(page).boundingBox())!.y;
    await caretAt(page, RICH.title, 0);
    await page.keyboard.type("A much longer title that is going to wrap onto a second line of the page ");
    await expect.poll(async () => (await markers(page).boundingBox())?.y ?? 0).toBeGreaterThan(y0 + 10);

    // A table of contents makes hidden bookmarks on the headings; they get no ribbon.
    await chooseMenu(page, "Insert", "Page elements", "Table of contents");
    await expect(page.locator(".flow .fp[data-bm]").filter({ hasText: "Highlights" })).toHaveCount(1);
    await expect(markers(page)).toHaveCount(1);
  });

  test("Insert, Bookmark, Bookmarks… lists them with their text; Go to selects the range; Rename and Remove work", async ({ page }) => {
    await addOnPhrase(page);
    await page.keyboard.press("Escape");
    await caretAt(page, RICH.last, 0);
    await chooseMenu(page, "Insert", "Bookmark", "Add bookmark");
    await page.keyboard.press("Escape");

    await chooseMenu(page, "Insert", "Bookmark", "Bookmarks…");
    const box = dialog(page, "Bookmarks");
    await expect(box).toBeVisible();
    const rows = box.locator(".bm-row");
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(0).locator(".bm-name")).toHaveText("live_numbers");
    await expect(rows.nth(0).locator(".bm-text")).toHaveText("live numbers");
    await expect(rows.nth(1).locator(".bm-name")).toHaveText("Bookmark1");

    // Rename: a bad name shows the message in the row.
    await rows.nth(1).getByRole("button", { name: "Rename" }).click();
    const name = box.getByRole("textbox", { name: "Bookmark name" });
    await name.fill("9lives");
    await name.press("Enter");
    await expect(box.getByRole("alert")).toHaveText("A bookmark name has to start with a letter.");
    await name.fill("Ending");
    await name.press("Enter");
    await expect(rows.nth(1).locator(".bm-name")).toHaveText("Ending");

    // Go to: closes the dialog and selects the bookmarked text.
    await rows.nth(0).getByRole("button", { name: "Go to" }).click();
    await expect(box).toBeHidden();
    const b = (await bookmarks(page)).find((x) => x.name === "live_numbers")!;
    expect(await selection(page)).toEqual({ anchor: { p: b.p, o: b.o }, focus: { p: b.endP, o: b.endO } });
    expect(await page.evaluate(() => window.getSelection()!.toString())).toBe("live numbers");

    // Remove, until the empty state.
    await chooseMenu(page, "Insert", "Bookmark", "Bookmarks…");
    await box.locator(".bm-row").first().getByRole("button", { name: "Remove" }).click();
    await box.locator(".bm-row").first().getByRole("button", { name: "Remove" }).click();
    await expect(box.locator(".bm-row")).toHaveCount(0);
    await expect(box.getByText("No bookmarks yet.")).toBeVisible();
    expect(await bookmarks(page)).toEqual([]);
    await box.getByRole("button", { name: "Done" }).click();
    await expect(box).toBeHidden();
  });

  test("the link editor offers the bookmarks; choosing one links to #name, and Ctrl/Cmd-click goes there", async ({ page }) => {
    // A bookmark on the last paragraph, on the second page.
    await caretAt(page, RICH.last, 0);
    await chooseMenu(page, "Insert", "Bookmark", "Add bookmark");
    await page.keyboard.press("Escape");
    const target = (await bookmarks(page))[0];
    expect(target.p).toBe(RICH.last);

    // Link a phrase in the introduction to it.
    const from = await offsetOf(page, RICH.intro, "bold progress");
    await selectText(page, RICH.intro, from, from + "bold progress".length);
    await chooseMenu(page, "Insert", "Link…");
    const choice = page.locator(".link-targets").getByRole("button", { name: "Bookmark1" });
    await expect(choice).toBeVisible();
    await choice.click();
    const link = para(page, RICH.intro).locator(".lnk", { hasText: "bold progress" });
    await expect(link).toHaveAttribute("data-href", "#Bookmark1");

    // Ctrl/Cmd-click scrolls to the bookmark and puts the caret there.
    await link.click({ modifiers: [MOD] });
    await expect.poll(async () => (await selection(page))?.focus.p).toBe(RICH.last);
    await expect(para(page, RICH.last)).toBeInViewport();
  });

  test("with no selection the link editor inserts the bookmark's name as the linked text; headings with a table of contents are offered too", async ({ page }) => {
    await caretAt(page, RICH.last, 0);
    await chooseMenu(page, "Insert", "Bookmark", "Add bookmark");
    await page.keyboard.press("Escape");
    await caretAt(page, RICH.quote, 0);
    await chooseMenu(page, "Insert", "Page elements", "Table of contents");

    await caretAt(page, RICH.last, 0);
    await page.keyboard.press("End");
    await chooseMenu(page, "Insert", "Link…");
    const targets = page.locator(".link-targets");
    await expect(targets.getByRole("button", { name: "Bookmark1" })).toBeVisible();
    await expect(targets.locator(".menu-title", { hasText: "Headings" })).toBeVisible();
    const heading = targets.getByRole("button", { name: "Highlights" });
    await expect(heading).toBeVisible();
    await heading.click();
    const link = page.locator(".flow .lnk[data-href^='#_Toc']").last();
    await expect(link).toHaveText("Highlights");
    // Linking to a heading follows the same way.
    await link.click({ modifiers: [MOD] });
    await expect.poll(async () => (await selection(page))?.focus.p).not.toBe(RICH.last);
  });
});

test.describe("watermark", () => {
  const marks = (page: Page) => page.locator(".sheet .wm");
  const text = (m: Locator) => m.locator(".wm-mark");

  /** Open Insert, Page elements, Watermark… and wait for the dialog. */
  async function openDialog(page: Page) {
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    const box = dialog(page, "Watermark");
    await expect(box).toBeVisible();
    return box;
  }

  test("the dialog applies the text, layout, colour and transparency", async ({ page }) => {
    await openFixture(page);
    const box = await openDialog(page);
    await expect(box.getByRole("button", { name: "Apply" })).toBeDisabled();
    await expect(box.getByRole("button", { name: "Remove watermark" })).toHaveCount(0);
    for (const name of ["Draft", "Confidential", "Sample", "Do not copy"]) await expect(box.getByRole("button", { name, exact: true })).toBeVisible();
    await box.getByRole("button", { name: "Confidential" }).click();
    await expect(box.getByRole("textbox", { name: "Text" })).toHaveValue("CONFIDENTIAL");
    await box.getByRole("radio", { name: "Horizontal" }).click();
    await box.getByRole("radio", { name: "Solid" }).click();
    await box.getByRole("button", { name: "#808080" }).click();
    // A small page shows the mark as it will look.
    await expect(box.locator(".wm-page .wm-mark")).toHaveText("CONFIDENTIAL");
    await box.getByRole("button", { name: "Apply" }).click();
    await expect(box).toBeHidden();

    const wm = await watermark(page);
    expect(wm).toMatchObject({ kind: "text", text: "CONFIDENTIAL", layout: "horizontal", color: "808080", angle: 0 });
    // Solid is the shape's own default: no opacity written.
    expect(wm!.opacity).toBeUndefined();
    await expect(marks(page)).toHaveCount(2);
    const first = text(marks(page).first());
    await expect(first).toHaveText("CONFIDENTIAL");
    const css = await first.evaluate((el) => {
      const cs = getComputedStyle(el);
      return { color: cs.color, opacity: cs.opacity, transform: cs.transform };
    });
    expect(css.color).toBe("rgb(128, 128, 128)");
    expect(css.opacity).toBe("1");
    // Horizontal: a translation only, no turn.
    expect(css.transform).toMatch(/^matrix\(1, 0, 0, 1,/);
  });

  test("diagonal is drawn turned 45 degrees up to the right, semi-transparent, in the document's colour", async ({ page }) => {
    await openFixture(page);
    const box = await openDialog(page);
    await box.getByRole("button", { name: "Draft" }).click();
    await box.getByRole("combobox", { name: "Font" }).selectOption("Georgia");
    await box.getByRole("button", { name: "Apply" }).click();
    expect(await watermark(page)).toMatchObject({ text: "DRAFT", layout: "diagonal", angle: 315, font: "Georgia", color: "C0C0C0", opacity: 0.5 });
    const mark = text(marks(page).first());
    const css = await mark.evaluate((el) => {
      const cs = getComputedStyle(el);
      const [a, b] = cs.transform.replace("matrix(", "").split(",").map(Number);
      return { angle: (Math.atan2(b, a) * 180) / Math.PI, opacity: cs.opacity, color: cs.color, font: cs.fontFamily, size: parseFloat(cs.fontSize) };
    });
    expect(css.angle).toBeCloseTo(-45, 1);
    expect(css.opacity).toBe("0.5");
    expect(css.color).toBe("rgb(192, 192, 192)");
    expect(css.font).toContain("Georgia");
    expect(css.size).toBeGreaterThan(60);
  });

  test("every page shows it, behind the text, centred in the margins, unselectable", async ({ page }) => {
    await openFixture(page);
    const box = await openDialog(page);
    await box.getByRole("button", { name: "Draft" }).click();
    await box.getByRole("button", { name: "Apply" }).click();
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBe(2);
    await expect(marks(page)).toHaveCount(pages);
    for (let k = 0; k < pages; k++) {
      const info = await page.locator(".sheet").nth(k).evaluate((sheet) => {
        const wm = sheet.querySelector<HTMLElement>(".wm")!;
        const mark = wm.querySelector<HTMLElement>(".wm-mark")!;
        const flow = document.querySelector<HTMLElement>(".flow")!;
        const s = sheet.getBoundingClientRect();
        const m = mark.getBoundingClientRect();
        return {
          first: sheet.firstElementChild === wm,
          // The sheets sit below the editable flow in the stack.
          flowZ: getComputedStyle(flow).zIndex,
          sheetsZ: getComputedStyle(sheet.parentElement!).zIndex,
          sheetFirst: !!(sheet.parentElement!.compareDocumentPosition(flow) & Node.DOCUMENT_POSITION_FOLLOWING),
          events: getComputedStyle(wm).pointerEvents,
          select: getComputedStyle(mark).userSelect,
          // The centre of the mark is the centre of the margin box (one inch each side).
          dx: (m.left + m.width / 2 - (s.left + s.width / 2)) / s.width,
          dy: (m.top + m.height / 2 - (s.top + s.height / 2)) / s.height,
        };
      });
      expect(info.first).toBe(true);
      expect(info.flowZ).toBe("1");
      expect(info.sheetsZ).toBe("auto");
      expect(info.sheetFirst).toBe(true);
      expect(info.events).toBe("none");
      expect(info.select).toBe("none");
      expect(Math.abs(info.dx)).toBeLessThan(0.01);
      expect(Math.abs(info.dy)).toBeLessThan(0.01);
    }
    // It is not in the editable text, and a click on the page goes to the text under it.
    expect(await page.locator(".flow .wm").count()).toBe(0);
    await expect(page.locator(".flow")).not.toContainText("DRAFT");
    expect(await page.evaluate(() => window.getSelection()!.toString())).toBe("");
  });

  test("a document of several sections shows it on all their pages, each in its own margin box", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["First section"], headers: { default: "Header one" } },
        { size: [15840, 12240], margins: [720, 720, 720, 2880], start: "nextPage", paragraphs: ["Second section, landscape"], headers: { default: "Header two" } },
        { paragraphs: ["Third section"], headers: { default: "Header three" }, start: "nextPage" },
      ],
    });
    const box = await openDialog(page);
    await box.getByRole("button", { name: "Confidential" }).click();
    await box.getByRole("button", { name: "Apply" }).click();
    const setup = await inPage(page, (w) => w.fe.editor.engine!.pageSetup());
    expect(setup.sections.map((s) => s.watermark?.text)).toEqual(["CONFIDENTIAL", "CONFIDENTIAL", "CONFIDENTIAL"]);
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBe(3);
    await expect(marks(page)).toHaveCount(3);
    // The headers kept their own words.
    await expect(page.locator(".sheet").nth(1).locator(".hdr")).toHaveText("Header two");
    // Each mark sits in the margin box of its page: the landscape page is wider and has a wide left margin.
    const boxes = await marks(page).evaluateAll((els) => els.map((el) => ({ left: (el as HTMLElement).offsetLeft, width: (el as HTMLElement).offsetWidth, sheetWidth: (el.parentElement as HTMLElement).offsetWidth })));
    expect(boxes[0]).toEqual({ left: 96, width: 624, sheetWidth: 816 });
    expect(boxes[1]).toEqual({ left: 192, width: 1056 - 192 - 48, sheetWidth: 1056 });
    expect(boxes[2]).toEqual(boxes[0]);
  });

  test("the dialog shows what the document has, Remove watermark removes it and undo brings it back", async ({ page }) => {
    await openFixture(page);
    let box = await openDialog(page);
    await box.getByRole("button", { name: "Sample" }).click();
    await box.getByRole("radio", { name: "Horizontal" }).click();
    await box.getByRole("textbox", { name: "Text" }).press("Enter");
    await expect(box).toBeHidden();
    await expect(marks(page)).toHaveCount(2);

    box = await openDialog(page);
    await expect(box.getByRole("textbox", { name: "Text" })).toHaveValue("SAMPLE");
    await expect(box.getByRole("radio", { name: "Horizontal" })).toHaveAttribute("aria-checked", "true");
    await box.getByRole("button", { name: "Remove watermark" }).click();
    await answer(page, "Remove watermark");
    await expect(box).toBeHidden();
    expect(await watermark(page)).toBeUndefined();
    await expect(marks(page)).toHaveCount(0);

    await page.keyboard.press(`${MOD}+Z`);
    expect(await watermark(page)).toMatchObject({ text: "SAMPLE" });
    await expect(marks(page)).toHaveCount(2);
    // Undoing the watermark itself.
    await page.keyboard.press(`${MOD}+Z`);
    expect(await watermark(page)).toBeUndefined();
    await expect(marks(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect(marks(page)).toHaveCount(2);
  });

  test("the print copy has it on every page, in colour", async ({ page }) => {
    await page.addInitScript(() => {
      window.print = () => undefined;
    });
    await openFixture(page);
    const box = await openDialog(page);
    await box.getByRole("button", { name: "Draft" }).click();
    await box.getByRole("button", { name: "Apply" }).click();
    await page.keyboard.press(`${MOD}+P`);
    const printed = page.locator(".docxtor-print .print-page");
    await expect(printed).toHaveCount(2);
    // Each printed page is the stage cut to one sheet; it holds the watermark of every sheet, and its own is in the window.
    for (let k = 0; k < 2; k++) {
      await expect(printed.nth(k).locator(".sheet .wm")).toHaveCount(2);
      await expect(printed.nth(k).locator(".sheet").nth(k).locator(".wm .wm-mark")).toHaveText("DRAFT");
    }
    // On paper the backgrounds and colours print as they are.
    await page.emulateMedia({ media: "print" });
    expect(await page.locator(".docxtor-print").evaluate((el) => getComputedStyle(el).getPropertyValue("print-color-adjust"))).toBe("exact");
    await page.emulateMedia({ media: "screen" });
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });

  test("a watermark made in Word shows when the document opens, and editing the header keeps it", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: ["Page one", "{PBR}", "Page two"],
          headers: { default: "The header", watermark: { text: "DO NOT COPY", color: "red", opacity: 0.25, font: "Arial" } },
        },
      ],
    });
    expect(await watermark(page)).toMatchObject({ kind: "text", text: "DO NOT COPY", layout: "diagonal", angle: 315, font: "Arial", color: "FF0000", opacity: 0.25 });
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    await expect(marks(page)).toHaveCount(pages);
    const first = text(marks(page).first());
    await expect(first).toHaveText("DO NOT COPY");
    const css = await first.evaluate((el) => {
      const cs = getComputedStyle(el);
      return { color: cs.color, opacity: cs.opacity, font: cs.fontFamily };
    });
    expect(css).toMatchObject({ color: "rgb(255, 0, 0)", opacity: "0.25" });
    expect(css.font).toContain("Arial");
    // The header shows its own words, not the shape.
    await expect(page.locator(".sheet").first().locator(".hdr")).toHaveText("The header");

    // Edit the header: the shape stays out of the text and is still there afterwards.
    await chooseMenu(page, "Insert", "Page elements", "Edit header");
    await expect(page.locator(".story-edit")).toBeVisible();
    await expect(page.locator(".story-edit")).toHaveText("The header");
    await expect(marks(page)).toHaveCount(pages);
    await page.keyboard.type(" changed");
    await page.keyboard.press("Escape");
    await expect(page.locator(".sheet").first().locator(".hdr")).toHaveText("The header changed");
    expect(await watermark(page)).toMatchObject({ text: "DO NOT COPY" });
    await expect(marks(page)).toHaveCount(pages);

    // The dialog starts from it.
    const box = await openDialog(page);
    await expect(box.getByRole("textbox", { name: "Text" })).toHaveValue("DO NOT COPY");
    await expect(box.getByRole("button", { name: "Remove watermark" })).toBeVisible();
  });
});

test.describe("picture watermark", () => {
  test("a picture from Word is drawn centred in the margin box, washed out and as large as the file says", async ({ page }) => {
    await openFixture(page);
    // The drawing function itself (the engine reads pictures from Word's files; here one is handed to it).
    const info = await page.evaluate(async (src) => {
      const { watermarkElement } = await import(/* @vite-ignore */ src);
      const el: HTMLElement = watermarkElement({ kind: "picture", layout: "horizontal", angle: 0, media: "word/media/image1.png", width: 200, height: 100 }, 600, 800, (m: string) => `data:image/gif;base64,R0lGODlhAQABAAAAACw=#${m}`);
      document.querySelector(".sheet")!.append(el);
      const img = el.querySelector("img")!;
      const cs = getComputedStyle(img);
      const r = img.getBoundingClientRect();
      const b = el.getBoundingClientRect();
      const out = { opacity: cs.opacity, src: img.src.endsWith("#word/media/image1.png"), dx: r.left + r.width / 2 - (b.left + b.width / 2), dy: r.top + r.height / 2 - (b.top + b.height / 2), width: parseFloat(cs.width), height: parseFloat(cs.height) };
      el.remove();
      return out;
    }, `/@fs${process.cwd()}/packages/editor/src/editor/watermark.ts`);
    expect(info.src).toBe(true);
    expect(info.opacity).toBe("0.3");
    expect(Math.abs(info.dx)).toBeLessThan(1);
    expect(Math.abs(info.dy)).toBeLessThan(1);
    expect(info.width).toBeCloseTo((200 * 96) / 72, 0);
    expect(info.height).toBeCloseTo((100 * 96) / 72, 0);
  });

  const marks = (page: Page) => page.locator(".sheet .wm");
  /** A 320 by 160 pixel PNG, drawn in the page. */
  const makePng = (page: Page) =>
    page.evaluate(() => {
      const c = document.createElement("canvas");
      c.width = 320;
      c.height = 160;
      const g = c.getContext("2d")!;
      g.fillStyle = "#cc3300";
      g.fillRect(0, 0, 320, 160);
      return c.toDataURL("image/png").split(",")[1];
    });
  const savedHas = (page: Page, needle: string) =>
    inPage(
      page,
      (w, n) => {
        const bytes = w.fe.editor.engine!.save();
        return new TextDecoder("latin1").decode(bytes).includes(n);
      },
      needle,
    );

  test("the dialog makes a picture watermark from a file, drawn washed out on every page; Remove takes it and its image away", async ({ page }) => {
    await openFixture(page);
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    let box = dialog(page, "Watermark");
    await box.getByRole("radio", { name: "Picture" }).click();
    await expect(box.getByRole("button", { name: "Apply" })).toBeDisabled();
    await expect(box.getByRole("textbox", { name: "Text" })).toBeHidden();
    await expect(box.getByRole("radio", { name: "Auto" })).toHaveAttribute("aria-checked", "true");
    await expect(box.getByRole("checkbox", { name: "Washout" })).toBeChecked();
    await box.locator("input[type=file]").setInputFiles({ name: "logo.png", mimeType: "image/png", buffer: Buffer.from(await makePng(page), "base64") });
    await expect(box.locator(".wm-file")).toHaveText("logo.png");
    // The small page shows the picture, washed out.
    const previewImg = box.locator(".wm-page img.wm-mark");
    await expect(previewImg).toBeVisible();
    expect(await previewImg.evaluate((el) => getComputedStyle(el).opacity)).toBe("0.3");
    await box.getByRole("radio", { name: "150%" }).click();
    await box.getByRole("button", { name: "Apply" }).click();
    await expect(box).toBeHidden();

    // 320 by 160 pixels is 240 by 120 points; at 150% it is 360 by 180.
    expect(await watermark(page)).toMatchObject({ kind: "picture", washout: true, width: 360, height: 180 });
    const pages = await page.locator(".sheet").count();
    expect(pages).toBeGreaterThan(1);
    await expect(marks(page)).toHaveCount(pages);
    const shown = marks(page).first().locator("img.wm-mark");
    await expect(shown).toBeVisible();
    expect(await shown.evaluate((el) => getComputedStyle(el).opacity)).toBe("0.3");
    expect(await shown.evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBe(320);
    expect(await shown.evaluate((el) => parseFloat(getComputedStyle(el).width))).toBeCloseTo((360 * 96) / 72, 0);
    expect(await savedHas(page, "word/media/docxtor")).toBe(true);

    // The dialog knows the picture; a new scale and no washout keep the same image.
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    box = dialog(page, "Watermark");
    await expect(box.getByRole("radio", { name: "Picture" })).toHaveAttribute("aria-checked", "true");
    await expect(box.getByRole("button", { name: "Apply" })).toBeEnabled();
    await box.getByRole("radio", { name: "50%", exact: true }).click();
    await box.getByRole("checkbox", { name: "Washout" }).uncheck();
    await box.getByRole("button", { name: "Apply" }).click();
    expect(await watermark(page)).toMatchObject({ kind: "picture", washout: false, width: 120, height: 60 });
    expect(await marks(page).first().locator("img.wm-mark").evaluate((el) => getComputedStyle(el).opacity)).toBe("1");

    // Remove, and the image leaves the saved file.
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    box = dialog(page, "Watermark");
    await box.getByRole("button", { name: "Remove watermark" }).click();
    await answer(page, "Remove watermark");
    expect(await watermark(page)).toBeUndefined();
    await expect(marks(page)).toHaveCount(0);
    expect(await savedHas(page, "word/media/docxtor")).toBe(false);
    // Undo brings it back with its image.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(marks(page)).toHaveCount(pages);
    expect(await savedHas(page, "word/media/docxtor")).toBe(true);
  });

  test("a text watermark replaces a picture one", async ({ page }) => {
    await openFixture(page);
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    let box = dialog(page, "Watermark");
    await box.getByRole("radio", { name: "Picture" }).click();
    await box.locator("input[type=file]").setInputFiles({ name: "logo.png", mimeType: "image/png", buffer: Buffer.from(await makePng(page), "base64") });
    await box.getByRole("button", { name: "Apply" }).click();
    await expect(box).toBeHidden();
    expect(await watermark(page)).toMatchObject({ kind: "picture" });
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    box = dialog(page, "Watermark");
    await box.getByRole("radio", { name: "Text" }).click();
    await box.getByRole("button", { name: "Draft" }).click();
    await box.getByRole("button", { name: "Apply" }).click();
    expect(await watermark(page)).toMatchObject({ kind: "text", text: "DRAFT" });
    await expect(marks(page).first().locator("img")).toHaveCount(0);
    expect(await savedHas(page, "word/media/docxtor")).toBe(false);
  });
});

// Used by the specs above through helpers; keeps the import list honest for `caretAtEnd`.
void caretAtEnd;
