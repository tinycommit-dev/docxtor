// Endnotes, captions, cross-references, the index, a table of figures and Update fields: the dialogs and what they
// leave in the document. The engine's side of these (the markup, the numbering) is tested in Rust.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, dialog, expect, inPage, openFixture, para, paraText, RICH, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

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
const offsetOf = async (page: Page, p: number, phrase: string) => (await paraText(page, p)).indexOf(phrase);
const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup() as unknown as { notes: { id: string; label: string }[]; endnotes: { id: string; label: string }[] });
const docText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());
const selection = (page: Page) => inPage(page, (w) => w.fe.editor.sel);
const sheets = (page: Page) => page.locator(".sheet");
const undo = (page: Page) => page.keyboard.press(`${MOD}+Z`);

/** Add a numbered endnote after some text of a paragraph, as the engine would. */
const addEndnote = (page: Page, p: number, o: number, text: string) => inPage(page, (w, a) => w.fe.editor.exec({ op: "insert_endnote", at: { p: a.p, o: a.o }, text: a.text }), { p, o, text });

test.describe("endnotes", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("Insert, Page elements, Endnote adds a note at the end of the document and puts the caret in it; the mark is a roman numeral", async ({ page }) => {
    const at = await offsetOf(page, RICH.intro, "bold progress");
    await caretAt(page, RICH.intro, at);
    await chooseMenu(page, "Insert", "Page elements", "Endnote");
    const editing = page.locator(".endnotes.story-edit");
    await expect(editing).toBeVisible();
    await page.keyboard.type("Source: the annual report.");
    expect((await setup(page)).endnotes.map((n) => n.label)).toEqual(["i"]);
    await expect(para(page, RICH.intro).locator(".atom.note.endnote")).toHaveText("i");
    await page.keyboard.press("Escape");
    await expect(editing).toHaveCount(0);
    // Listed on the last page, after the text, behind a short rule.
    const last = sheets(page).last();
    await expect(last.locator(".endnotes")).toContainText("Source: the annual report.");
    await expect(last.locator(".endnotes .atom.note")).toHaveText("i");
    const lastPara = await page.locator(".flow [data-p]").last().boundingBox();
    const area = await last.locator(".endnotes").boundingBox();
    expect(area!.y).toBeGreaterThan(lastPara!.y);
    // The caret is back where it was, after the new mark.
    expect((await selection(page))?.focus).toEqual({ p: RICH.intro, o: at + 1 });
  });

  test("a second endnote is ii; double-click on a mark opens its note for editing", async ({ page }) => {
    await addEndnote(page, RICH.intro, 5, "First.");
    await addEndnote(page, RICH.highlights, 3, "Second.");
    await expect(page.locator(".flow .atom.note.endnote")).toHaveText(["i", "ii"]);
    await page.locator(".flow .atom.note.endnote").nth(1).dblclick();
    const editing = page.locator(".endnotes.story-edit");
    await expect(editing).toBeVisible();
    await page.keyboard.type(" More.");
    await expect(editing).toContainText("Second. More.");
    await page.keyboard.press("Escape");
    await expect(sheets(page).last().locator(".endnotes")).toContainText("Second. More.");
  });

  test("the shortcut (Ctrl/Cmd+Alt+D) adds an endnote too", async ({ page }) => {
    await caretAt(page, RICH.intro, 5);
    await page.keyboard.press(`${MOD}+Alt+D`);
    await expect(page.locator(".endnotes.story-edit")).toBeVisible();
    await page.keyboard.type("Typed straight away.");
    await page.keyboard.press("Escape");
    await expect(sheets(page).last().locator(".endnotes")).toContainText("Typed straight away.");
  });

  test("double-click on the list of endnotes, or right-click, opens it for editing with the caret in the note under the pointer", async ({ page }) => {
    await addEndnote(page, RICH.intro, 5, "First note.");
    await addEndnote(page, RICH.intro, 9, "Second note.");
    const area = sheets(page).last().locator(".endnotes");
    await area.scrollIntoViewIfNeeded();
    const second = (await area.locator(".fp").nth(1).boundingBox())!;
    await page.mouse.dblclick(second.x + 40, second.y + second.height / 2);
    const editing = page.locator(".endnotes.story-edit");
    await expect(editing).toBeVisible();
    expect((await selection(page))?.focus.p).toBe(Number(await editing.locator(".fp").nth(1).getAttribute("data-p")));
    await expect(page.locator(".story-strip")).toContainText("Endnotes on page 2");
    await page.keyboard.press("Escape");
    await expect(editing).toHaveCount(0);
    const again = (await area.locator(".fp").first().boundingBox())!;
    await page.mouse.click(again.x + 40, again.y + again.height / 2, { button: "right" });
    await contextItem(page, "Edit endnotes").click();
    await expect(editing).toBeVisible();
  });

  test("one undo takes the endnote back out", async ({ page }) => {
    await caretAt(page, RICH.intro, 5);
    await chooseMenu(page, "Insert", "Page elements", "Endnote");
    await page.keyboard.press("Escape");
    await expect(page.locator(".flow .atom.note.endnote")).toHaveCount(1);
    await undo(page);
    await expect(page.locator(".flow .atom.note.endnote")).toHaveCount(0);
    await expect(page.locator(".endnotes")).toHaveCount(0);
    expect((await setup(page)).endnotes).toHaveLength(0);
  });

  test("long endnotes flow onto further pages after the text", async ({ page }) => {
    const long = "A long note that goes on for a while and needs several lines of the page. ".repeat(10);
    for (let i = 0; i < 12; i++) await addEndnote(page, RICH.intro, 3 + i, `${i + 1}. ${long}`);
    const n = await sheets(page).count();
    expect(n).toBeGreaterThanOrEqual(3);
    // Notes sit on the last pages only, in order, and every one is listed once.
    const texts = await page.locator(".sheet .endnotes").evaluateAll((els) => els.map((el) => el.textContent ?? ""));
    expect(texts.length).toBeGreaterThanOrEqual(2);
    const all = texts.join(" ");
    for (let i = 1; i <= 12; i++) expect(all).toContain(`${i}. A long note`);
    expect(await sheets(page).nth(0).locator(".endnotes").count()).toBe(0);
    // The last page is within the document: nothing runs past the bottom of a page.
    await page.locator(".sheet .endnotes").last().scrollIntoViewIfNeeded();
    for (const k of await sheets(page).evaluateAll((els) => els.map((_, i) => i))) {
      const ok = await sheets(page).nth(k).evaluate((sh) => {
        const notes = sh.querySelector(".endnotes");
        return !notes || notes.getBoundingClientRect().bottom <= sh.getBoundingClientRect().bottom + 1;
      });
      expect(ok).toBe(true);
    }
  });

  test("Cmd/Ctrl-click on the mark goes to the note, and on the note's own mark back to the text", async ({ page }) => {
    await addEndnote(page, RICH.last, 4, "The note.");
    await page.evaluate(() => document.querySelector(".canvas")!.scrollTo(0, 0));
    await page.locator(".flow .atom.note.endnote").click({ modifiers: [MOD] });
    const editing = page.locator(".endnotes.story-edit");
    await expect(editing).toBeVisible();
    await expect(editing).toBeInViewport();
    await editing.locator(".atom.note").click({ modifiers: [MOD] });
    await expect(editing).toHaveCount(0);
    expect((await selection(page))?.focus.p).toBe(RICH.last);
  });
});

test.describe("footnote and endnote options", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  const open = async (page: Page) => {
    await chooseMenu(page, "Insert", "Page elements", "Footnote and endnote options…");
    const box = dialog(page, "Footnote and endnote options");
    await expect(box).toBeVisible();
    return box;
  };

  test("the number format of endnotes and footnotes changes the marks and the list", async ({ page }) => {
    await addEndnote(page, RICH.intro, 3, "One.");
    await addEndnote(page, RICH.intro, 9, "Two.");
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_footnote", at: { p: 3, o: 3 }, text: "Aside." }));
    const box = await open(page);
    // It starts from what the document does now: i, ii for endnotes and 1 for footnotes.
    await expect(box.getByLabel("Number format").nth(0)).toHaveValue("decimal");
    await expect(box.getByLabel("Number format").nth(1)).toHaveValue("lowerRoman");
    await box.getByLabel("Number format").nth(1).selectOption("upperLetter");
    await box.getByLabel("Endnotes start at").fill("3");
    await box.getByLabel("Number format").nth(0).selectOption("symbol");
    await box.getByRole("button", { name: "Apply" }).click();
    await expect(box).toBeHidden();
    expect((await setup(page)).endnotes.map((n) => n.label)).toEqual(["C", "D"]);
    expect((await setup(page)).notes.map((n) => n.label)).toEqual(["*"]);
    await expect(page.locator(".flow .atom.note.endnote")).toHaveText(["C", "D"]);
    await expect(sheets(page).last().locator(".endnotes")).toContainText("C");
    // Open again: the dialog reads the document's numbering back.
    const again = await open(page);
    await expect(again.getByLabel("Number format").nth(1)).toHaveValue("upperLetter");
    await expect(again.getByLabel("Endnotes start at")).toHaveValue("3");
    await again.getByRole("button", { name: "Cancel" }).click();
    await undo(page);
    expect((await setup(page)).endnotes.map((n) => n.label)).toEqual(["i", "ii"]);
  });

  test("Convert all footnotes to endnotes and back, each in one undo step", async ({ page }) => {
    await inPage(page, (w) => w.fe.editor.exec([{ op: "insert_footnote", at: { p: 2, o: 3 }, text: "First." }, { op: "insert_footnote", at: { p: 2, o: 9 }, text: "Second." }]));
    expect((await setup(page)).notes).toHaveLength(2);
    const box = await open(page);
    await expect(box.getByRole("button", { name: "Convert all endnotes to footnotes" })).toBeDisabled();
    await box.getByRole("button", { name: "Convert all footnotes to endnotes" }).click();
    await expect(page.locator(".toast")).toHaveText("2 footnotes are now endnotes");
    expect(await setup(page)).toMatchObject({ notes: [], endnotes: [{ label: "i" }, { label: "ii" }] });
    await expect(page.locator(".flow .atom.note.endnote")).toHaveCount(2);
    await expect(box.getByRole("button", { name: "Convert all footnotes to endnotes" })).toBeDisabled();
    await box.getByRole("button", { name: "Convert all endnotes to footnotes" }).click();
    expect(await setup(page)).toMatchObject({ endnotes: [], notes: [{ label: "1" }, { label: "2" }] });
    await expect(page.locator(".flow .atom.note.endnote")).toHaveCount(0);
    await box.getByRole("button", { name: "Cancel" }).click();
    await undo(page);
    expect((await setup(page)).endnotes).toHaveLength(2);
    await undo(page);
    expect((await setup(page)).notes).toHaveLength(2);
  });
});

// -- captions ------------------------------------------------------------------------------------------------------

interface Caption {
  p: number;
  label: string;
  number: string;
  text: string;
}
const captions = (page: Page): Promise<Caption[]> => inPage(page, (w) => w.fe.editor.engine!.xrefTargets().captions);
const pageMap = (page: Page) => inPage(page, (w) => w.fe.editor.pageMap());
const openCaption = async (page: Page) => {
  await chooseMenu(page, "Insert", "References", "Caption…");
  const box = dialog(page, "Caption");
  await expect(box).toBeVisible();
  return box;
};
const selectImage = async (page: Page) => {
  await page.locator(".flow .atom.img").scrollIntoViewIfNeeded();
  await page.locator(".flow .atom.img").click();
  await expect(page.locator(".img-frame")).toBeVisible();
};
const cellText = (page: Page) => page.locator(".flow table td [data-p]").first();

test.describe("captions", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("an image gets a Figure caption below it, numbered; the dialog previews it", async ({ page }) => {
    await selectImage(page);
    const box = await openCaption(page);
    await expect(box.locator("#cap-label")).toHaveValue("Figure");
    await expect(box.getByRole("radio", { name: "Below" })).toHaveAttribute("aria-checked", "true");
    await expect(box.locator(".cap-preview")).toHaveText("Figure 1");
    await box.getByLabel("Caption text").fill("Revenue by quarter");
    await expect(box.locator(".cap-preview")).toHaveText("Figure 1: Revenue by quarter");
    await box.getByRole("button", { name: "Insert" }).click();
    await expect(box).toBeHidden();
    const caps = await captions(page);
    expect(caps.map((c) => [c.label, c.number, c.text])).toEqual([["Figure", "1", "Figure 1: Revenue by quarter"]]);
    const caption = para(page, caps[0].p);
    await expect(caption).toHaveText("Figure 1: Revenue by quarter");
    const image = (await para(page, RICH.image).boundingBox())!;
    expect((await caption.boundingBox())!.y).toBeGreaterThan(image.y + image.height - 2);
    // The caret is at the end of the caption, ready to type more.
    expect((await selection(page))?.focus).toEqual({ p: caps[0].p, o: "Figure 1: Revenue by quarter".length });
    // One undo step takes the caption away.
    await undo(page);
    expect(await captions(page)).toEqual([]);
  });

  test("a table gets a Table caption above it by default", async ({ page }) => {
    await para(page, 16).click();
    const box = await openCaption(page);
    await expect(box.locator("#cap-label")).toHaveValue("Table");
    await expect(box.getByRole("radio", { name: "Above" })).toHaveAttribute("aria-checked", "true");
    await box.getByLabel("Caption text").fill("Quarterly figures");
    await box.getByLabel("Caption text").press("Enter");
    await expect(box).toBeHidden();
    const [cap] = await captions(page);
    expect([cap.label, cap.number, cap.text]).toEqual(["Table", "1", "Table 1: Quarterly figures"]);
    const table = (await page.locator(".flow .ftw").first().boundingBox())!;
    expect((await para(page, cap.p).boundingBox())!.y).toBeLessThan(table.y);
  });

  test("a caption put above an earlier one takes number 1 and the other renumbers in the same step", async ({ page }) => {
    await selectImage(page);
    let box = await openCaption(page);
    await box.getByLabel("Caption text").fill("Second");
    await box.getByRole("button", { name: "Insert" }).click();
    await selectImage(page);
    box = await openCaption(page);
    await box.getByLabel("Caption text").fill("Cover");
    // Above the image: that is before the other one only when the other is below it.
    await box.getByRole("radio", { name: "Above" }).click();
    await expect(box.locator(".cap-preview")).toHaveText("Figure 1: Cover");
    await box.getByRole("button", { name: "Insert" }).click();
    expect((await captions(page)).map((c) => c.text)).toEqual(["Figure 1: Cover", "Figure 2: Second"]);
    await undo(page);
    expect((await captions(page)).map((c) => c.text)).toEqual(["Figure 1: Second"]);
  });

  test("New label… makes a caption with a label of your own", async ({ page }) => {
    await selectImage(page);
    const box = await openCaption(page);
    await expect(box.getByRole("button", { name: "Insert" })).toBeEnabled();
    await box.locator("#cap-label").selectOption({ label: "New label…" });
    await expect(box.getByRole("button", { name: "Insert" })).toBeDisabled();
    await box.getByLabel("New label").fill("Chart");
    await box.getByLabel("Caption text").fill("Growth");
    await expect(box.locator(".cap-preview")).toHaveText("Chart 1: Growth");
    await box.getByRole("button", { name: "Insert" }).click();
    expect((await captions(page)).map((c) => [c.label, c.text])).toEqual([["Chart", "Chart 1: Growth"]]);
    // The next caption dialog offers it.
    await selectImage(page);
    const again = await openCaption(page);
    await expect(again.locator("#cap-label option")).toHaveText(["Figure", "Table", "Equation", "Chart", "New label…"]);
  });

  test("right-click on an image or inside a table offers Insert caption…", async ({ page }) => {
    await page.locator(".flow .atom.img").scrollIntoViewIfNeeded();
    await page.locator(".flow .atom.img").click({ button: "right" });
    await contextItem(page, "Insert caption…").click();
    const box = dialog(page, "Caption");
    await expect(box.locator("#cap-label")).toHaveValue("Figure");
    await box.getByRole("button", { name: "Cancel" }).click();
    await cellText(page).click({ button: "right" });
    await contextItem(page, "Insert caption…").click();
    await expect(dialog(page, "Caption").locator("#cap-label")).toHaveValue("Table");
    await dialog(page, "Caption").getByRole("button", { name: "Cancel" }).click();
    // Plain text has no such line.
    await para(page, RICH.intro).click({ button: "right" });
    await expect(contextItem(page, "Insert caption…")).toHaveCount(0);
  });
});

// -- cross-references and updating ----------------------------------------------------------------------------------

const openXref = async (page: Page) => {
  await chooseMenu(page, "Insert", "References", "Cross-reference…");
  const box = dialog(page, "Cross-reference");
  await expect(box).toBeVisible();
  return box;
};
/** Make a cross-reference through the dialog, at the end of paragraph `p`. */
async function crossRef(page: Page, p: number, type: string, target: string, show: string, opts: { link?: boolean; above?: boolean } = {}) {
  await caretAt(page, p, (await paraText(page, p)).length);
  const box = await openXref(page);
  await box.getByLabel("Reference type").selectOption({ label: type });
  await box.locator(".xr-list").getByRole("option", { name: target }).first().click();
  await box.getByLabel("Insert reference to").selectOption({ label: show });
  if (opts.link === false) await box.getByLabel("Insert as hyperlink").uncheck();
  if (opts.above) await box.getByLabel("Include above or below").check();
  await box.getByRole("button", { name: "Insert" }).click();
  await expect(box).toBeHidden();
}

test.describe("cross-references", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("to a heading: its text, a link; the dialog lists the headings and filters them", async ({ page }) => {
    await caretAt(page, RICH.last, 0);
    const box = await openXref(page);
    await expect(box.getByLabel("Reference type")).toHaveValue("heading");
    const items = box.locator(".xr-list").getByRole("option");
    await expect(items).toHaveText(["Highlights", "Plan", "Numbers", "Appendix"]);
    await box.getByLabel("Search the list").fill("app");
    await expect(items).toHaveText(["Appendix"]);
    await box.getByLabel("Search the list").fill("");
    await box.getByRole("button", { name: "Cancel" }).click();
    await crossRef(page, RICH.last, "Heading", "Numbers", "Heading text");
    const link = para(page, RICH.last).locator(".lnk[data-href^='#_Ref']");
    await expect(link).toHaveText("Numbers");
    await expect(para(page, RICH.last)).toContainText("Numbers");
    await undo(page);
    await expect(para(page, RICH.last).locator(".lnk")).toHaveCount(0);
  });

  test("to a heading's page number; page numbers come from the layout", async ({ page }) => {
    await crossRef(page, RICH.last, "Heading", "Appendix", "Page number");
    const pages = await pageMap(page);
    expect(pages[31]).toBe(2);
    await expect(para(page, RICH.last).locator(".lnk[data-href^='#_Ref']")).toHaveText("2");
    // Not as a link: plain text.
    await crossRef(page, RICH.intro, "Heading", "Appendix", "Page number", { link: false });
    await expect(para(page, RICH.intro).locator(".lnk[data-href^='#_Ref']")).toHaveCount(0);
    expect(await paraText(page, RICH.intro)).toMatch(/2$/);
  });

  test("to a caption (label and number, or the whole caption), with above or below", async ({ page }) => {
    await selectImage(page);
    const cap = await openCaption(page);
    await cap.getByLabel("Caption text").fill("Revenue");
    await cap.getByRole("button", { name: "Insert" }).click();
    await crossRef(page, RICH.intro, "Figure", "Figure 1: Revenue", "Only label and number", { above: true });
    // "Figure 1", then a space and "below" (the figure is further down).
    expect(await paraText(page, RICH.intro)).toMatch(/Figure 1 below$/);
    await crossRef(page, RICH.quote, "Figure", "Figure 1: Revenue", "Entire caption");
    expect(await paraText(page, RICH.quote)).toMatch(/Figure 1: Revenue$/);
  });

  test("to a bookmark's text and to a footnote's number", async ({ page }) => {
    const at = await offsetOf(page, RICH.intro, "bold progress");
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "insert_bookmark", from: { p: 2, o: a }, to: { p: 2, o: a + 13 }, name: "progress" }), at);
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_footnote", at: { p: 12, o: 3 }, text: "A footnote." }));
    await crossRef(page, RICH.last, "Bookmark", "progress", "Bookmark text");
    await expect(para(page, RICH.last).locator(".lnk[data-href='#progress']")).toHaveText("bold progress");
    await crossRef(page, RICH.last, "Footnote", "A footnote.", "Footnote number");
    expect(await paraText(page, RICH.last)).toMatch(/bold progress1$/);
    // Endnote type: empty at first, says so.
    await caretAt(page, RICH.last, 0);
    const box = await openXref(page);
    await box.getByLabel("Reference type").selectOption({ label: "Endnote" });
    await expect(box.getByText("No endnotes yet.")).toBeVisible();
    await expect(box.getByRole("button", { name: "Insert" })).toBeDisabled();
  });

  test("to an endnote's number, as the document formats it", async ({ page }) => {
    await addEndnote(page, RICH.intro, 5, "The source.");
    await crossRef(page, RICH.last, "Endnote", "The source.", "Endnote number (formatted)");
    expect(await paraText(page, RICH.last)).toMatch(/i$/);
  });

  test("Cmd/Ctrl-click follows the reference to its target", async ({ page }) => {
    await crossRef(page, RICH.last, "Heading", "Numbers", "Heading text");
    await page.evaluate(() => document.querySelector(".canvas")!.scrollTo(0, 0));
    const link = para(page, RICH.last).locator(".lnk[data-href^='#_Ref']");
    await link.scrollIntoViewIfNeeded();
    await link.click({ modifiers: [MOD] });
    await expect.poll(async () => (await selection(page))?.focus.p).toBe(14);
    await expect(para(page, 14)).toBeInViewport();
  });

  test("after a heading changes, Update fields (F9) brings the reference text up to date; one undo step", async ({ page }) => {
    await crossRef(page, RICH.last, "Heading", "Numbers", "Heading text");
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_text", at: { p: 14, o: 0 }, text: "Key " }));
    await expect(para(page, RICH.last).locator(".lnk")).toHaveText("Numbers");
    await page.keyboard.press("F9");
    await expect(para(page, RICH.last).locator(".lnk")).toHaveText("Key Numbers");
    await expect(page.locator(".toast")).toHaveText("Updated 1 field");
    await undo(page);
    await expect(para(page, RICH.last).locator(".lnk")).toHaveText("Numbers");
    // Nothing more to do: says so, and the menu does the same.
    await chooseMenu(page, "Tools", "Update fields");
    await expect(para(page, RICH.last).locator(".lnk")).toHaveText("Key Numbers");
    await chooseMenu(page, "Insert", "References", "Update fields");
    await expect(page.locator(".toast")).toHaveText("All fields are up to date");
  });

  test("Update fields refreshes page numbers after the text moves", async ({ page }) => {
    await crossRef(page, RICH.intro, "Heading", "Appendix", "Page number");
    const ref = para(page, RICH.intro).locator(".lnk[data-href^='#_Ref']");
    await expect(ref).toHaveText("2");
    // A page break at the very top pushes the appendix to page 3.
    await caretAt(page, RICH.title, 0);
    await page.keyboard.press(`${MOD}+Enter`);
    await page.keyboard.press("F9");
    await expect(ref).toHaveText("3");
  });
});

// -- the index and a table of figures ---------------------------------------------------------------------------------

const withText = (page: Page, text: string) => page.locator(".flow [data-p]").filter({ hasText: text });
const idOf = async (page: Page, text: string) => Number(await withText(page, text).first().getAttribute("data-p"));
/** Select a phrase of the document by its text and open Mark index entry… */
async function markDialog(page: Page, phrase: string) {
  const p = await idOf(page, phrase);
  const at = await offsetOf(page, p, phrase);
  await selectText(page, p, at, at + phrase.length);
  await chooseMenu(page, "Insert", "References", "Mark index entry…");
  const box = dialog(page, "Mark index entry");
  await expect(box).toBeVisible();
  return box;
}

test.describe("index", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: ["Page one: apples and cherries grow here.", { text: "Page two: bananas and apples again.", ppr: "<w:pageBreakBefore/>" }, "The end."] }],
    });
  });

  test("Mark index entry… is filled with the selection; Mark hides the entry in the text", async ({ page }) => {
    const before = await docText(page);
    const box = await markDialog(page, "cherries");
    await expect(box.getByLabel("Main entry")).toHaveValue("cherries");
    await box.getByLabel("Main entry").fill("Cherries");
    await box.getByLabel("Subentry").fill("sweet");
    await box.getByLabel("Bold").check();
    await box.getByRole("button", { name: "Mark", exact: true }).click();
    await expect(box).toBeHidden();
    await expect(page.locator(".toast")).toHaveText("Marked for the index");
    // The entry takes no room and shows no text.
    expect(await docText(page)).toBe(before);
    await undo(page);
    // The shortcut opens the same dialog.
    await selectText(page, await idOf(page, "bananas"), 10, 17);
    await page.keyboard.press(`${MOD}+Alt+Shift+X`);
    await expect(dialog(page, "Mark index entry")).toBeVisible();
    await dialog(page, "Mark index entry").getByRole("button", { name: "Cancel" }).click();
  });

  test("Mark all marks every paragraph with the text; the index lists the entries sorted with their pages, in two columns", async ({ page, browserName }) => {
    let box = await markDialog(page, "apples");
    await box.getByLabel("Main entry").fill("Apples");
    await box.getByRole("button", { name: "Mark all" }).click();
    await expect(page.locator(".toast")).toHaveText("Marked 2 places for the index");
    box = await markDialog(page, "cherries");
    await box.getByLabel("Main entry").fill("Cherries");
    await box.getByRole("button", { name: "Mark", exact: true }).click();
    box = await markDialog(page, "bananas");
    await box.getByLabel("Main entry").fill("Bananas");
    await box.getByLabel("Cross-reference").fill("Fruit");
    await box.getByRole("button", { name: "Mark", exact: true }).click();

    await caretAt(page, await idOf(page, "The end."), 4);
    await chooseMenu(page, "Insert", "References", "Index");
    const text = await docText(page);
    const index = text.slice(text.indexOf("The end.") + "The end.".length);
    const lines = index.split("\n").filter(Boolean);
    expect(lines).toEqual(["Apples\t1–2", "Bananas\tSee Fruit", "Cherries\t1"]);
    // In a continuous section of its own with two columns.
    const sections = await inPage(page, (w) => w.fe.editor.engine!.pageSetup().sections.map((s) => s.cols.num));
    expect(sections).toEqual([1, 2, 1]);
    // (Where the browser can't wrap rows of columns, only Chromium so far, the two columns show as one and have no wrapper.)
    await expect(page.locator(".flow .cols")).toHaveCount(browserName === "chromium" ? 1 : 0);
    // One undo takes the whole index (and its section) out again.
    await undo(page);
    expect((await docText(page)).includes("Apples\t")).toBe(false);
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().sections.length)).toBe(1);
  });

  test("Update fields rebuilds the index after the text moves", async ({ page }) => {
    const box = await markDialog(page, "apples");
    await box.getByLabel("Main entry").fill("Apples");
    await box.getByRole("button", { name: "Mark all" }).click();
    await caretAt(page, await idOf(page, "The end."), 4);
    await chooseMenu(page, "Insert", "References", "Index");
    expect(await docText(page)).toContain("Apples\t1–2");
    // A page break at the top: both mentions move one page on.
    await caretAt(page, await idOf(page, "Page one"), 0);
    await page.keyboard.press(`${MOD}+Enter`);
    await page.keyboard.press("F9");
    await expect.poll(() => docText(page)).toContain("Apples\t2–3");
  });
});

test.describe("table of figures", () => {
  test("lists the captions with one label and their pages; Update fields keeps it current", async ({ page }) => {
    await openFixture(page);
    await selectImage(page);
    let box = await openCaption(page);
    await box.getByLabel("Caption text").fill("Revenue");
    await box.getByRole("button", { name: "Insert" }).click();
    await para(page, 16).click();
    box = await openCaption(page);
    await box.getByLabel("Caption text").fill("Quarters");
    await box.getByRole("button", { name: "Insert" }).click();

    await caretAt(page, RICH.highlights, 0);
    await chooseMenu(page, "Insert", "References", "Table of figures…");
    const tof = dialog(page, "Table of figures");
    await expect(tof.getByLabel("Caption label")).toHaveValue("Figure");
    await expect(tof).toContainText("1 caption will be listed.");
    await tof.getByLabel("Caption label").selectOption("Table");
    await expect(tof).toContainText("1 caption will be listed.");
    await tof.getByLabel("Caption label").selectOption("Figure");
    await tof.getByRole("button", { name: "Insert" }).click();
    await expect(tof).toBeHidden();
    const text = await docText(page);
    expect(text).toMatch(/Figure 1: Revenue\t\d+/);
    expect(text).not.toMatch(/Table 1: Quarters\t\d+/);
    // Another figure before it: Update fields lists both.
    await selectImage(page);
    box = await openCaption(page);
    await box.getByLabel("Caption text").fill("Cover");
    await box.getByRole("radio", { name: "Above" }).click();
    await box.getByRole("button", { name: "Insert" }).click();
    await page.keyboard.press("F9");
    await expect.poll(() => docText(page)).toMatch(/Figure 1: Cover\t\d+\s+Figure 2: Revenue\t\d+/);
    await undo(page);
    await undo(page);
    await undo(page);
    expect((await docText(page)).match(/Figure 1: Revenue\t/)).toBeNull();
  });
});
