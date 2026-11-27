// The Navigation pane: headings (click to go, drag to move a section), page thumbnails and search results.
import { chooseMenu, expect, test, type Page } from "./helpers";
import { blocks, bytesOf, caretIn, headings, idOf, LONG_OUTLINE_DOC, onFe, openDoc, order, OUTLINE_DOC, shownText } from "./support/view-helpers";

const pane = (page: Page) => page.locator(".nav-pane");
const tab = (page: Page, name: string) => pane(page).getByRole("tab", { name, exact: true });
const rows = (page: Page) => pane(page).locator(".nav-item:not([hidden])");
const row = (page: Page, text: string) => pane(page).locator(".nav-item", { has: page.locator(".nav-label", { hasText: new RegExp(`^${text}$`) }) });

async function openPane(page: Page, name = "Headings") {
  await page.locator(".sidebar").getByRole("tab", { name: "Navigation", exact: true }).click();
  await expect(pane(page)).toBeVisible();
  await tab(page, name).click();
}

test.describe("the Navigation tab", () => {
  test("is one of the navigator's tabs, and View, Show navigation pane opens it", async ({ page }) => {
    await openDoc(page);
    await expect(page.locator(".sidebar").getByRole("tab")).toHaveText(["Files", "Navigation", "History"]);
    await chooseMenu(page, "View", "Show navigation pane");
    await expect(pane(page)).toBeVisible();
    await expect(pane(page).getByRole("tab")).toHaveText(["Headings", "Pages", "Results"]);
    await expect(tab(page, "Headings")).toHaveAttribute("aria-selected", "true");
  });
});

test.describe("Headings", () => {
  test("lists the headings with their levels, not the body text", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    await expect(rows(page).locator(".nav-label")).toHaveText(["Alpha", "Alpha one", "Alpha two", "Beta", "Gamma"]);
    expect(await rows(page).evaluateAll((els) => els.map((el) => el.getAttribute("aria-level")))).toEqual(["1", "2", "2", "1", "1"]);
    // A deeper heading is indented.
    const left = (text: string) => row(page, text).locator(".nav-label").evaluate((el) => el.getBoundingClientRect().left);
    expect((await left("Alpha one")) - (await left("Alpha"))).toBeGreaterThan(10);
  });

  test("shows the empty state, then follows edits", async ({ page }) => {
    await openDoc(page, { sections: [{ paragraphs: ["Only body text"] }] });
    await openPane(page);
    await expect(pane(page).locator(".nav-empty")).toContainText("Headings appear here");
    // Make the paragraph a heading: it shows up.
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("ControlOrMeta+Alt+1");
    await expect(rows(page)).toHaveCount(1);
    await expect(rows(page).locator(".nav-label")).toHaveText(["Only body text"]);
  });

  test("a click goes to the heading and puts the caret there", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    await openPane(page);
    await row(page, "Gamma").locator(".nav-label").click();
    const gamma = await idOf(page, "Gamma");
    await expect.poll(() => onFe(page, (fe) => fe.editor.sel?.focus.p)).toBe(gamma);
    await expect.poll(() => onFe(page, (fe) => (fe.editor as unknown as { currentPage(): number }).currentPage())).toBeGreaterThan(2);
  });

  test("the heading the caret is under is marked", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    await caretIn(page, "alpha two body");
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { emitSelection(): void } } } }).docxtor.fe.editor.emitSelection());
    await expect(row(page, "Alpha two")).toHaveClass(/current/);
    await expect(rows(page).locator(".nav-label[aria-current]")).toHaveCount(1);
    await caretIn(page, "Gamma");
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { emitSelection(): void } } } }).docxtor.fe.editor.emitSelection());
    await expect(row(page, "Gamma")).toHaveClass(/current/);
    await expect(row(page, "Alpha two")).not.toHaveClass(/current/);
  });

  test("an arrow folds the headings under one in the list, not in the document", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    await row(page, "Alpha").locator(".nav-toggle").click();
    await expect(rows(page).locator(".nav-label")).toHaveText(["Alpha", "Beta", "Gamma"]);
    await expect(row(page, "Alpha")).toHaveAttribute("aria-expanded", "false");
    // The document is untouched.
    await expect(page.locator(".flow .fp", { hasText: /^alpha one body$/ })).toBeVisible();
    await row(page, "Alpha").locator(".nav-toggle").click();
    await expect(rows(page)).toHaveCount(5);
    // A heading with nothing under it has no arrow to use.
    await expect(row(page, "Alpha one").locator(".nav-toggle")).toHaveClass(/none/);
  });

  test("dragging a heading onto another moves its whole section, and Undo puts it back", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    const before = await order(page);
    // Alpha (with Alpha one and Alpha two) goes below Gamma.
    const target = row(page, "Gamma");
    const box = (await target.boundingBox())!;
    await row(page, "Alpha").locator(".nav-label").dragTo(target, { targetPosition: { x: 30, y: box.height - 3 } });
    expect(await order(page)).toEqual(["Intro text", "Beta", "beta body", "Gamma", "gamma body", "Alpha", "alpha body", "Alpha one", "alpha one body", "Alpha two", "alpha two body"]);
    await expect(rows(page).locator(".nav-label")).toHaveText(["Beta", "Gamma", "Alpha", "Alpha one", "Alpha two"]);
    // The levels did not change.
    expect(await headings(page)).toEqual(["1 Beta", "1 Gamma", "1 Alpha", "2 Alpha one", "2 Alpha two"]);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("ControlOrMeta+z");
    expect(await order(page)).toEqual(before);
    await expect(rows(page).locator(".nav-label")).toHaveText(["Alpha", "Alpha one", "Alpha two", "Beta", "Gamma"]);
  });

  test("dropping on the upper half puts the section before the heading", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    await row(page, "Gamma").locator(".nav-label").dragTo(row(page, "Alpha one"), { targetPosition: { x: 30, y: 2 } });
    expect(await headings(page)).toEqual(["1 Alpha", "1 Gamma", "2 Alpha one", "2 Alpha two", "1 Beta"]);
  });

  test("a heading can't be dropped into its own section", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    const before = await order(page);
    await row(page, "Alpha").locator(".nav-label").dragTo(row(page, "Alpha two"), { targetPosition: { x: 30, y: 3 } });
    expect(await order(page)).toEqual(before);
  });

  test("Alt with the arrow keys moves the section from the keyboard", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    await row(page, "Beta").locator(".nav-label").focus();
    await page.keyboard.press("Alt+ArrowUp");
    expect(await headings(page)).toEqual(["1 Beta", "1 Alpha", "2 Alpha one", "2 Alpha two", "1 Gamma"]);
    // The focus stays on the heading that moved.
    await expect(row(page, "Beta").locator(".nav-label")).toBeFocused();
    await page.keyboard.press("Alt+ArrowDown");
    expect(await headings(page)).toEqual(["1 Alpha", "2 Alpha one", "2 Alpha two", "1 Beta", "1 Gamma"]);
    // The arrow keys alone walk the list.
    await page.keyboard.press("ArrowDown");
    await expect(row(page, "Gamma").locator(".nav-label")).toBeFocused();
  });

  test("nothing can be dragged in a read-only document", async ({ page }) => {
    await openDoc(page);
    await openPane(page);
    await onFe(page, (fe) => (fe as unknown as { setReadOnly(on: boolean): void }).setReadOnly(true));
    const before = await order(page);
    // The drag is refused at its start, and the keys don't move anything either.
    const refused = await row(page, "Alpha").evaluate((el) => {
      const ev = new DragEvent("dragstart", { bubbles: true, cancelable: true, dataTransfer: new DataTransfer() });
      el.dispatchEvent(ev);
      return ev.defaultPrevented;
    });
    expect(refused).toBe(true);
    await row(page, "Beta").locator(".nav-label").focus();
    await page.keyboard.press("Alt+ArrowUp");
    expect(await order(page)).toEqual(before);
  });
});

test.describe("Pages", () => {
  test("shows a thumbnail of each page, with its text, and marks the one in view", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    await openPane(page, "Pages");
    const pages = await onFe(page, (fe) => fe.editor.pages);
    const cells = pane(page).locator(".nav-page");
    await expect(cells).toHaveCount(pages);
    await expect(cells.first()).toHaveAttribute("aria-label", "Page 1");
    await expect(cells.first().locator(".pg-snap .fp").first()).toHaveText("Intro text");
    // A thumbnail is the page's shape at a small size.
    const b = (await cells.first().locator(".pg-thumb").boundingBox())!;
    expect(b.width).toBeCloseTo(112, 0);
    expect(b.height / b.width).toBeCloseTo(1056 / 816, 1);
    // And it is what is on the page: a heading, drawn bold, near the top.
    expect(await cells.first().locator(".pg-snap .fp[data-style='Heading1']").count()).toBe(1);
    await expect(cells.first()).toHaveClass(/current/);
    await page.screenshot({ path: "test-results/navigation-pages.png" });
    // Scrolling the document moves the mark.
    await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>(".canvas")!;
      c.scrollTo({ top: c.scrollHeight });
    });
    await expect(cells.last()).toHaveClass(/current/);
    await expect(cells.first()).not.toHaveClass(/current/);
  });

  test("a click goes to the page", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    await openPane(page, "Pages");
    await pane(page).locator(".nav-page").nth(2).click();
    await expect.poll(() => onFe(page, (fe) => (fe.editor as unknown as { currentPage(): number }).currentPage())).toBe(3);
    const caret = await onFe(page, (fe) => fe.editor.sel!.focus.p);
    expect((await onFe(page, (fe) => (fe.editor as unknown as { pageMap(): Record<string, number> }).pageMap()))[String(caret)]).toBe(3);
  });

  test("thumbnails come as they scroll into view, and follow an edit", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    await openPane(page, "Pages");
    const cells = pane(page).locator(".nav-page");
    const count = await cells.count();
    // The document's own text: each thumbnail holds a copy of its page in a `.flow.pg-flow`, which `.flow .fp` would also find.
    await page.locator(".flow:not(.pg-flow) .fp").first().click();
    await page.keyboard.press("Control+End");
    await page.keyboard.press("Enter");
    for (let i = 0; i < 70; i++) await page.keyboard.type("x\n");
    await expect.poll(() => cells.count()).toBeGreaterThan(count);
  });

  test("says so when the layout has no pages", async ({ page }) => {
    await openDoc(page);
    await chooseMenu(page, "View", "Layout", "Pageless");
    await openPane(page, "Pages");
    await expect(pane(page).locator(".nav-empty")).toContainText("no pages");
  });
});

test.describe("Results", () => {
  test("lists every match with the words around it, and a click selects it", async ({ page }) => {
    await openDoc(page, { sections: [{ headers: { default: "Header with body" }, paragraphs: ["The body of the first paragraph.", "Another paragraph with BODY in capitals.", "Nothing here.", "A body at the end"] }] });
    await openPane(page, "Results");
    await expect(pane(page).locator(".nav-empty")).toContainText("Type to search");
    const input = pane(page).getByRole("searchbox", { name: "Search the document" });
    await input.fill("body");
    const results = pane(page).locator(".nav-result");
    // Three in the text (any case) and one in the header.
    await expect(results).toHaveCount(4);
    await expect(pane(page).locator(".nav-status")).toHaveText("4 results");
    await expect(results.first().locator("mark")).toHaveText("body");
    await expect(results.first()).toContainText("The body of the first");
    // A click selects the match in the text.
    await results.nth(1).click();
    expect(await page.evaluate(() => getSelection()?.toString())).toBe("BODY");
    await results.nth(0).click();
    expect(await page.evaluate(() => getSelection()?.toString())).toBe("body");
    // The header's own match is found, and goes into the header.
    await results.nth(3).click();
    await expect(page.locator(".story-strip")).toBeVisible();
  });

  test("Match case narrows the results", async ({ page }) => {
    await openDoc(page, { sections: [{ paragraphs: ["The body of it.", "A BODY of it.", "Body at the start."] }] });
    await openPane(page, "Results");
    const input = pane(page).getByRole("searchbox", { name: "Search the document" });
    await input.fill("body");
    await expect(pane(page).locator(".nav-result")).toHaveCount(3);
    await pane(page).getByLabel("Match case").check();
    await expect(pane(page).locator(".nav-result")).toHaveCount(1);
    await expect(pane(page).locator(".nav-status")).toHaveText("1 result");
    await input.fill("nothing like it");
    await expect(pane(page).locator(".nav-status")).toHaveText("No results");
  });

  test("the search and the tab are kept while you edit, and the list follows the text", async ({ page }) => {
    await openDoc(page, { sections: [{ paragraphs: ["alpha one", "beta"] }] });
    await openPane(page, "Results");
    await pane(page).getByRole("searchbox", { name: "Search the document" }).fill("alpha");
    await expect(pane(page).locator(".nav-result")).toHaveCount(1);
    await page.locator(".flow .fp").nth(1).click();
    await page.keyboard.type(" alpha");
    await expect(pane(page).locator(".nav-result")).toHaveCount(2);
  });

  test("a search typed while the pane is being drawn again for an edit is the one that is run and kept", async ({ page }) => {
    await openDoc(page, { sections: [{ paragraphs: ["alpha one", "beta"] }] });
    await openPane(page, "Results");
    const input = pane(page).getByRole("searchbox", { name: "Search the document" });
    await input.fill("alpha");
    await expect(pane(page).locator(".nav-status")).toHaveText("1 result");
    // An edit draws the tab again 300 ms later; typing 150 ms after it waits 200 ms to search, so the redraw comes first.
    await onFe(page, async (fe) => {
      const ed = (fe as unknown as { editor: { exec(op: unknown): unknown; engine: { renderAll(): { order: number[] } } } }).editor;
      ed.exec({ op: "insert_text", at: { p: ed.engine.renderAll().order[1], o: 0 }, text: "x" });
      await new Promise((r) => setTimeout(r, 150));
      const box = document.querySelector<HTMLInputElement>(".nav-pane .nav-search")!;
      box.focus();
      box.value = "zzzz";
      box.dispatchEvent(new Event("input", { bubbles: true }));
      await new Promise((r) => setTimeout(r, 500));
    });
    await expect(pane(page).locator(".nav-status")).toHaveText("No results");
    await expect(input).toHaveValue("zzzz");
    await expect(input).toBeFocused();
  });

  test("a result inside folded text opens the heading it is under", async ({ page }) => {
    await openDoc(page);
    await page.locator(`.fm-btn[data-id="${await idOf(page, "Alpha")}"]`).click();
    expect(await shownText(page)).not.toContain("alpha two body");
    await openPane(page, "Results");
    await pane(page).getByRole("searchbox", { name: "Search the document" }).fill("alpha two body");
    await pane(page).locator(".nav-result").first().click();
    expect(await shownText(page)).toContain("alpha two body");
    await expect(page.locator(".fm-btn.folded")).toHaveCount(0);
    expect(await page.evaluate(() => getSelection()?.toString())).toBe("alpha two body");
  });

  test("a heading clicked in the list opens what is folded over it", async ({ page }) => {
    await openDoc(page);
    await page.locator(`.fm-btn[data-id="${await idOf(page, "Alpha")}"]`).click();
    await openPane(page);
    await row(page, "Alpha two").locator(".nav-label").click();
    expect(await shownText(page)).toContain("Alpha two");
    await expect(page.locator(".fm-btn.folded")).toHaveCount(0);
  });

  test("the pane reopens on the tab it was left on", async ({ page }) => {
    await openDoc(page);
    await openPane(page, "Pages");
    await page.locator(".sidebar").getByRole("tab", { name: "Files", exact: true }).click();
    await page.locator(".sidebar").getByRole("tab", { name: "Navigation", exact: true }).click();
    await expect(tab(page, "Pages")).toHaveAttribute("aria-selected", "true");
  });
});

test("the headings of a document that opens later show up", async ({ page }) => {
  await openDoc(page, { sections: [{ paragraphs: ["Nothing"] }] });
  await openPane(page);
  await expect(pane(page).locator(".nav-empty")).toBeVisible();
  await page.evaluate(async (b) => (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name: string): Promise<unknown> } } }).docxtor.fe.open(new Uint8Array(b), "next.docx"), bytesOf(OUTLINE_DOC));
  await expect(rows(page)).toHaveCount(5);
  expect((await blocks(page)).filter((b) => b.level).length).toBe(5);
});
