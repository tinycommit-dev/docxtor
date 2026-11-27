// Folding headings in the text and the Outline view: promote and demote with Tab and Shift+Tab or the strip, move a
// section up and down, fold and unfold, show levels, first line only. Document changes are ops, so Ctrl+Z takes them back.
import { chooseMenu, expect, test, type Page } from "./helpers";
import { blocks, caretIn, chooseLayout, heading, headings, idOf, LONG_OUTLINE_DOC, onFe, openDoc, order, OUTLINE_DOC, shownText } from "./support/view-helpers";

const ALPHA = ["1 Alpha", "2 Alpha one", "2 Alpha two", "1 Beta", "1 Gamma"];
const strip = (page: Page) => page.getByRole("toolbar", { name: "Outline" });
const button = (page: Page, name: string) => strip(page).getByRole("button", { name, exact: true });
const flowBlock = (page: Page, text: string) => page.locator(".flow > .fp", { hasText: new RegExp(`^${text}$`) });

test.describe("folding headings in the text", () => {
  test("an arrow beside a heading folds what is under it and the pages shrink", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    const before = await onFe(page, (fe) => fe.editor.pages);
    expect(before).toBeGreaterThan(3);
    const alphaLines = (await shownText(page)).filter((t) => t.startsWith("alpha")).length;
    expect(alphaLines).toBe(90);
    const alpha = await idOf(page, "Alpha");
    // The arrow shows while the pointer is over the heading.
    const arrow = page.locator(`.fm-btn[data-id="${alpha}"]`);
    await flowBlock(page, "Alpha").hover();
    await expect(arrow).toBeVisible();
    await expect(arrow).toHaveAttribute("aria-expanded", "true");
    await expect(arrow).toHaveAttribute("aria-label", "Collapse heading");
    await arrow.click();
    // Everything down to the next level 1 heading is folded away: Alpha one and its text, too.
    await expect(arrow).toHaveAttribute("aria-expanded", "false");
    await expect(arrow).toHaveAttribute("aria-label", "Expand heading");
    const shown = await shownText(page);
    expect(shown).toEqual(expect.arrayContaining(["Intro text", "Alpha", "Beta", "Gamma"]));
    expect(shown.some((t) => t.startsWith("alpha"))).toBe(false);
    expect(await onFe(page, (fe) => fe.editor.pages)).toBeLessThan(before);
    // The folded heading's arrow stays on view without the pointer.
    await page.mouse.move(5, 5);
    await expect(arrow).toBeVisible();
    await arrow.click();
    expect(await onFe(page, (fe) => fe.editor.pages)).toBe(before);
    expect((await shownText(page)).filter((t) => t.startsWith("alpha")).length).toBe(alphaLines);
  });

  test("only a heading with something under it has an arrow, and a nested fold is kept when its parent unfolds", async ({ page }) => {
    await openDoc(page);
    const ids = { alpha: await idOf(page, "Alpha"), one: await idOf(page, "Alpha one"), two: await idOf(page, "Alpha two") };
    await expect(page.locator(".fm-btn")).toHaveCount(5);
    // Alpha one and Alpha two both have a line of text; fold the inner one, then the outer, then open the outer.
    await onFe(page, (fe, id) => {
      fe.editor.folds.folded.add(id);
      (fe.editor as unknown as { foldsChanged(): void }).foldsChanged();
    }, ids.one);
    expect(await shownText(page)).not.toContain("alpha one body");
    expect(await shownText(page)).toContain("alpha two body");
    await page.locator(`.fm-btn[data-id="${ids.alpha}"]`).click();
    expect(await shownText(page)).not.toContain("alpha two body");
    await page.locator(`.fm-btn[data-id="${ids.alpha}"]`).click();
    expect(await shownText(page)).toContain("alpha two body");
    expect(await shownText(page)).not.toContain("alpha one body");
  });

  test("the caret leaves what is folded away and stays on the heading", async ({ page }) => {
    await openDoc(page);
    await caretIn(page, "alpha one body");
    await page.locator(`.fm-btn[data-id="${await idOf(page, "Alpha")}"]`).click();
    const alpha = await idOf(page, "Alpha");
    expect(await onFe(page, (fe) => fe.editor.sel?.focus.p)).toBe(alpha);
  });

  test("View, Headings has commands to fold the heading at the caret, every heading, and to open them all again", async ({ page }) => {
    await openDoc(page);
    await caretIn(page, "alpha body");
    await chooseHeadings(page, "Collapse or expand heading");
    expect(await shownText(page)).not.toContain("alpha body");
    expect(await shownText(page)).not.toContain("alpha two body");
    await chooseHeadings(page, "Expand all headings");
    expect(await shownText(page)).toContain("alpha body");
    await chooseHeadings(page, "Collapse all headings");
    expect(await shownText(page)).toEqual(["Intro text", "Alpha", "Beta", "Gamma"]);
    await chooseHeadings(page, "Expand all headings");
    expect((await shownText(page)).length).toBe(11);
  });

  test("folding is not a change to the document: it is not saved and Undo leaves it", async ({ page }) => {
    await openDoc(page);
    const before = await order(page);
    await page.locator(`.fm-btn[data-id="${await idOf(page, "Alpha")}"]`).click();
    expect(await order(page)).toEqual(before);
    await expect(page.locator(".led")).toHaveText(/Saved/);
  });
});

const chooseHeadings = (page: Page, item: string) => chooseMenu(page, "View", "Headings", item);

test.describe("the Outline view", () => {
  test("View, Layout, Outline shows the strip, indents blocks by level and marks each one", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await expect(strip(page)).toBeVisible();
    expect(await onFe(page, (fe) => fe.view)).toBe("outline");
    // One sheet, no pages, and no rulers.
    await expect(page.locator(".sheet")).toHaveCount(1);
    await expect(page.locator(".ruler")).toBeHidden();
    const left = (text: string) => flowBlock(page, text).evaluate((el) => el.getBoundingClientRect().left);
    const alpha = await left("Alpha");
    expect((await left("Alpha one")) - alpha).toBeCloseTo(28, 0);
    expect((await left("alpha body")) - alpha).toBeCloseTo(28, 0);
    expect((await left("alpha one body")) - alpha).toBeCloseTo(56, 0);
    expect((await left("Intro text")) - alpha).toBeCloseTo(0, 0);
    // Marks: an arrow for headings with text under them, a square for body text.
    await expect(page.locator(".fm-btn")).toHaveCount(5);
    expect(await page.locator(".fm-dot.dot").count()).toBe(6);
    await page.screenshot({ path: "test-results/outline-view.png" });
    await page.getByRole("button", { name: "Close outline view" }).click();
    await expect(strip(page)).toHaveCount(0);
    await expect(page.locator(".flow > [data-ob]")).toHaveCount(0);
    expect(await onFe(page, (fe) => fe.view)).toBe("print");
  });

  test("Tab demotes and Shift+Tab promotes a heading, as ops that Undo takes back", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "Alpha one");
    await page.keyboard.press("Tab");
    expect(await headings(page)).toEqual(["1 Alpha", "3 Alpha one", "2 Alpha two", "1 Beta", "1 Gamma"]);
    await expect(flowBlock(page, "Alpha one")).toHaveAttribute("data-style", "Heading3");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    expect(await headings(page)).toEqual(["1 Alpha", "1 Alpha one", "2 Alpha two", "1 Beta", "1 Gamma"]);
    // Level 1 can't go further up.
    await page.keyboard.press("Shift+Tab");
    expect(await headings(page)).toEqual(["1 Alpha", "1 Alpha one", "2 Alpha two", "1 Beta", "1 Gamma"]);
    // The selection stays on the heading and Tab doesn't type a tab character.
    await expect(flowBlock(page, "Alpha one")).toBeVisible();
    for (let i = 0; i < 3; i++) await page.keyboard.press("ControlOrMeta+z");
    expect(await headings(page)).toEqual(ALPHA);
    expect(await onFe(page, (fe) => fe.editor.sel?.focus.p)).toBe(await idOf(page, "Alpha one"));
  });

  test("body text promotes to a heading below the one above it and a heading demotes to body text", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "alpha body");
    await page.keyboard.press("Shift+Tab");
    expect(await headings(page)).toEqual(["1 Alpha", "2 alpha body", "2 Alpha one", "2 Alpha two", "1 Beta", "1 Gamma"]);
    await button(page, "Demote to body text").click();
    expect(await headings(page)).toEqual(ALPHA);
    await caretIn(page, "Gamma");
    await button(page, "Demote to body text").click();
    expect((await blocks(page)).find((b) => b.text === "Gamma")!.level).toBe(0);
    await expect(flowBlock(page, "Gamma")).not.toHaveAttribute("data-style", /Heading/);
  });

  test("the strip's Promote and Demote work on a selection of several headings", async ({ page }) => {
    await openDoc(page, { styles: OUTLINE_DOC.styles, sections: [{ paragraphs: [heading(1, "Alpha"), heading(2, "Alpha one"), heading(2, "Alpha two"), heading(1, "Beta")] }] });
    await chooseLayout(page, "Outline");
    // Select from Alpha one down into Alpha two with the keyboard.
    await caretIn(page, "Alpha one");
    await page.keyboard.press("Shift+ArrowDown");
    await button(page, "Demote").click();
    expect(await headings(page)).toEqual(["1 Alpha", "3 Alpha one", "3 Alpha two", "1 Beta"]);
    await button(page, "Promote").click();
    expect(await headings(page)).toEqual(["1 Alpha", "2 Alpha one", "2 Alpha two", "1 Beta"]);
    // Level 1 can't be promoted: the heading stays and the others are promoted as far as they go.
    await caretIn(page, "Alpha");
    await page.keyboard.press("Shift+ArrowDown");
    await button(page, "Promote").click();
    expect(await headings(page)).toEqual(["1 Alpha", "1 Alpha one", "2 Alpha two", "1 Beta"]);
  });

  test("a folded heading takes the headings folded under it along", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "Alpha");
    await button(page, "Collapse").click();
    await button(page, "Demote").click();
    expect(await headings(page)).toEqual(["2 Alpha", "3 Alpha one", "3 Alpha two", "1 Beta", "1 Gamma"]);
    await page.keyboard.press("ControlOrMeta+z");
    expect(await headings(page)).toEqual(ALPHA);
  });

  test("Move up and Move down carry a heading with everything under it", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "Beta");
    await button(page, "Move up").click();
    expect(await order(page)).toEqual(["Intro text", "Beta", "beta body", "Alpha", "alpha body", "Alpha one", "alpha one body", "Alpha two", "alpha two body", "Gamma", "gamma body"]);
    await button(page, "Move down").click();
    await button(page, "Move down").click();
    expect(await order(page)).toEqual(["Intro text", "Alpha", "alpha body", "Alpha one", "alpha one body", "Alpha two", "alpha two body", "Gamma", "gamma body", "Beta", "beta body"]);
    // Undo, twice more, puts the document back.
    for (let i = 0; i < 3; i++) await page.keyboard.press("ControlOrMeta+z");
    expect(await order(page)).toEqual(["Intro text", "Alpha", "alpha body", "Alpha one", "alpha one body", "Alpha two", "alpha two body", "Beta", "beta body", "Gamma", "gamma body"]);
  });

  test("a heading moves past its sibling's whole section, and into the next section when it is the last of its own", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "Alpha one");
    await page.keyboard.press("Alt+Shift+ArrowDown");
    expect(await order(page)).toEqual(["Intro text", "Alpha", "alpha body", "Alpha two", "alpha two body", "Alpha one", "alpha one body", "Beta", "beta body", "Gamma", "gamma body"]);
    // Now the last of Alpha: moving down puts it in Beta, below Beta's own text.
    await page.keyboard.press("Alt+Shift+ArrowDown");
    expect(await order(page)).toEqual(["Intro text", "Alpha", "alpha body", "Alpha two", "alpha two body", "Beta", "beta body", "Alpha one", "alpha one body", "Gamma", "gamma body"]);
    await page.keyboard.press("Alt+Shift+ArrowUp");
    await page.keyboard.press("Alt+Shift+ArrowUp");
    expect(await order(page)).toEqual(["Intro text", "Alpha", "alpha body", "Alpha one", "alpha one body", "Alpha two", "alpha two body", "Beta", "beta body", "Gamma", "gamma body"]);
  });

  test("a body paragraph moves one block at a time", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "beta body");
    await page.keyboard.press("Alt+Shift+ArrowDown");
    expect((await order(page)).slice(-4)).toEqual(["Beta", "Gamma", "beta body", "gamma body"]);
    await page.keyboard.press("Alt+Shift+ArrowUp");
    expect((await order(page)).slice(-4)).toEqual(["Beta", "beta body", "Gamma", "gamma body"]);
  });

  test("Collapse and Expand fold the heading at the caret, and the marks follow", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    await caretIn(page, "alpha one body");
    await page.keyboard.press("Alt+Shift+-");
    expect(await shownText(page)).not.toContain("alpha one body");
    expect(await shownText(page)).toContain("alpha two body");
    // The caret went to the heading it is under.
    expect(await onFe(page, (fe) => fe.editor.sel?.focus.p)).toBe(await idOf(page, "Alpha one"));
    await expect(page.locator(".fm-btn.folded")).toHaveCount(1);
    await page.keyboard.press("Alt+Shift+=");
    expect(await shownText(page)).toContain("alpha one body");
    await expect(page.locator(".fm-btn.folded")).toHaveCount(0);
  });

  test("Show level limits the outline to headings down to that level, and All levels brings it back", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    const level = strip(page).getByRole("combobox", { name: "Show level" });
    await level.selectOption("1");
    expect(await shownText(page)).toEqual(["Alpha", "Beta", "Gamma"]);
    await level.selectOption("2");
    expect(await shownText(page)).toEqual(["Alpha", "Alpha one", "Alpha two", "Beta", "Gamma"]);
    await level.selectOption("0");
    expect((await shownText(page)).length).toBe(11);
    // The keys: Alt+Shift with a digit, and A for all.
    await page.keyboard.press("Alt+Shift+1");
    expect(await shownText(page)).toEqual(["Alpha", "Beta", "Gamma"]);
    await expect(level).toHaveValue("1");
    await page.keyboard.press("Alt+Shift+A");
    expect((await shownText(page)).length).toBe(11);
    await expect(level).toHaveValue("0");
  });

  test("Show levels 1 to 9 are all offered", async ({ page }) => {
    await openDoc(page);
    await chooseLayout(page, "Outline");
    const options = await strip(page).getByRole("combobox", { name: "Show level" }).locator("option").allTextContents();
    expect(options).toEqual(["All levels", ...Array.from({ length: 9 }, (_, i) => `Level ${i + 1}`)]);
  });

  test("First line only cuts body text to one line", async ({ page }) => {
    const long = "A long paragraph that goes on and on so that it needs several lines to show, with many more words than a single line of the outline can hold at once, and then some more words to make sure of it. ".repeat(3);
    await openDoc(page, { ...OUTLINE_DOC, sections: [{ paragraphs: [long, ...(OUTLINE_DOC.sections[0].paragraphs as string[]).slice(1)] }] });
    await chooseLayout(page, "Outline");
    const first = page.locator(".flow > .fp").first();
    const tall = (await first.boundingBox())!.height;
    expect(tall).toBeGreaterThan(40);
    await button(page, "First line only").click();
    const one = (await first.boundingBox())!.height;
    expect(one).toBeLessThan(tall / 2);
    expect(one).toBeLessThan(25);
    await expect(button(page, "First line only")).toHaveAttribute("aria-pressed", "true");
    // Headings keep their full text.
    await expect(flowBlock(page, "Alpha")).toBeVisible();
    await button(page, "First line only").click();
    expect((await first.boundingBox())!.height).toBeCloseTo(tall, 0);
  });

  test("closing the outline puts the document back on pages with every fold and level cleared", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    const pages = await onFe(page, (fe) => fe.editor.pages);
    await chooseLayout(page, "Outline");
    await strip(page).getByRole("combobox", { name: "Show level" }).selectOption("1");
    await page.getByRole("button", { name: "Close outline view" }).click();
    expect(await onFe(page, (fe) => fe.editor.pages)).toBe(pages);
    expect(await onFe(page, (fe) => fe.editor.folds.levels)).toBe(0);
    expect(await page.locator(".flow [data-fold]").count()).toBe(0);
  });

  test("the commands are in the palette only while the outline is on", async ({ page }) => {
    await openDoc(page);
    const ids = () =>
      onFe(page, (fe) => {
        const host = (fe as unknown as { extensions: { commands: { list(): { id: string }[] }; enabled(c: { id: string }): boolean } }).extensions;
        return host.commands.list().filter((c) => host.enabled(c)).map((c) => c.id).filter((id) => id.startsWith("outline."));
      });
    expect(await ids()).toEqual([]);
    await chooseLayout(page, "Outline");
    expect(await ids()).toEqual(expect.arrayContaining(["outline.promote", "outline.demote", "outline.moveUp", "outline.moveDown", "outline.expand", "outline.collapse", "outline.showLevel.9", "outline.firstLine"]));
  });
});
