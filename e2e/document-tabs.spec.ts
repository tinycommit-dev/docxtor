// Document tabs: Insert, Tab makes the text the first tab and opens a new one; only the open tab is shown and paged;
// the navigation pane lists the tabs (rename, emoji, delete, undo); the tabs come back when the file opens again; Print
// all tabs prints every tab.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

async function addTab(page: Page, name: string) {
  await chooseMenu(page, "Insert", "Tab…");
  const d = dialog(page, "Add tab");
  await d.getByRole("textbox").fill(name);
  await d.getByRole("button", { name: "Add" }).click();
}

const flow = (page: Page) => page.locator(".editor-host .flow").first();
const pane = (page: Page) => page.locator(".doc-tabs-pane");

test.describe("document tabs", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text", "More of the overview"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
  });

  test("a plain document shows only Add tab, and a new tab shows on its own", async ({ page }) => {
    await expect(pane(page)).toContainText("Add a tab to split the document");
    await addTab(page, "Details");
    await expect(pane(page).getByRole("treeitem")).toHaveCount(2);
    await expect(pane(page).getByRole("treeitem", { name: /Details/ })).toHaveAttribute("aria-selected", "true");
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeHidden();
    await page.keyboard.type("Details text");
    await expect(flow(page)).toContainText("Details text");
    // The first tab again.
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Details text" })).toBeHidden();
    await expect(page.locator(".sheet")).toHaveCount(1);
    await page.screenshot({ path: "test-results/document-tabs.png" });
    // Kept in the file.
    const names = await inPage(page, (w) => {
      const eng = w.fe.editor.engine!;
      const Engine = eng.constructor as unknown as { open(b: Uint8Array, n: string): { tabsInfo(): { tabs: { name: string }[] }; dispose(): void } };
      const again = Engine.open(eng.save(), "again.docx");
      const n = again.tabsInfo().tabs.map((x) => x.name);
      again.dispose();
      return n;
    });
    expect(names).toEqual(["Tab 1", "Details"]);
  });

  test("rename, an emoji and delete from the tab's menu, and undo brings it back", async ({ page }) => {
    await addTab(page, "Details");
    await pane(page).getByRole("button", { name: "Options for Details" }).click();
    await page.getByRole("menuitem", { name: "Choose emoji…" }).click();
    await page.getByRole("option", { name: "📌" }).click();
    await expect(pane(page).getByRole("treeitem", { name: /Details/ })).toContainText("📌");
    await pane(page).getByRole("button", { name: "Options for Details" }).click();
    await page.getByRole("menuitem", { name: "Rename…" }).click();
    const r = dialog(page, "Rename tab");
    await r.getByRole("textbox").fill("Appendix");
    await r.getByRole("button", { name: "Rename" }).click();
    await expect(pane(page).getByRole("treeitem", { name: /Appendix/ })).toHaveCount(1);
    await pane(page).getByRole("button", { name: "Options for Appendix" }).click();
    await page.getByRole("menuitem", { name: "Delete" }).click();
    await page.getByRole("alertdialog", { name: /Delete/ }).getByRole("button", { name: "Delete" }).click();
    await expect(pane(page).getByRole("treeitem")).toHaveCount(0);
    await flow(page).locator("[data-p]").first().click();
    await page.keyboard.press("ControlOrMeta+z");
    await expect(pane(page).getByRole("treeitem", { name: /Appendix/ })).toHaveCount(1);
  });

  test("Print all tabs prints every tab; Print prints the open one", async ({ page }) => {
    await addTab(page, "Details");
    await page.keyboard.type("Details text");
    await page.evaluate(() => {
      const w = window as unknown as { __printed: number[] };
      w.__printed = [];
      window.print = () => w.__printed.push(document.querySelectorAll(".docxtor-print .print-page").length);
    });
    await chooseMenu(page, "File", "Print all tabs…");
    await expect.poll(() => page.evaluate(() => (window as unknown as { __printed: number[] }).__printed.length)).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
    await page.keyboard.press("ControlOrMeta+p");
    await expect.poll(() => page.evaluate(() => (window as unknown as { __printed: number[] }).__printed.length)).toBe(2);
    const counts = await page.evaluate(() => (window as unknown as { __printed: number[] }).__printed);
    expect(counts[0]).toBe(2);
    expect(counts[1]).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });

  // The open tab is view state: each pane has its own, no op changes it (architecture review 1.2).
  test("in a split window each pane shows its own tab, and switching never freezes the page", async ({ page }) => {
    await addTab(page, "Details");
    await page.keyboard.type("Details text");
    await chooseMenu(page, "View", "Window", "Split");
    const flows = page.locator(".editor-host .flow");
    await expect(flows).toHaveCount(2);
    // The second pane starts on the tab the first one shows.
    await expect(flows.nth(1).locator("[data-p]", { hasText: "Details text" })).toBeVisible();
    // Choose the first tab with the keyboard in the second pane: only that pane changes.
    await flows.nth(1).locator("[data-p]", { hasText: "Details text" }).click();
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flows.nth(1).locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
    await expect(flows.nth(0).locator("[data-p]", { hasText: "Details text" })).toBeVisible();
    await expect(flows.nth(0).locator("[data-p]", { hasText: "Overview text" })).toBeHidden();
    // And back, in the first pane, while the second stays.
    await flows.nth(0).locator("[data-p]", { hasText: "Details text" }).click();
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flows.nth(0).locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
    // Typing in one pane (the caret was put in a hidden tab of the other) is no loop either.
    await page.keyboard.type("x");
    await expect(flows.nth(1)).toContainText("xOverview text");
    expect(await inPage(page, () => 1)).toBe(1);
  });

  test("a read-only document can still change tabs", async ({ page }) => {
    await addTab(page, "Details");
    await page.keyboard.type("Details text");
    await inPage(page, (w) => void w.fe.lock("held for the test"));
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Details text" })).toBeHidden();
    // The Next tab command works too, and nothing about the document changed.
    const rev = await inPage(page, (w) => w.fe.editor.engine!.revision);
    expect(await inPage(page, (w) => w.fe.extensions.execute("view.nextTab").then((r) => r.status))).toBe("done");
    await expect(flow(page).locator("[data-p]", { hasText: "Details text" })).toBeVisible();
    expect(await inPage(page, (w) => w.fe.editor.engine!.revision)).toBe(rev);
  });

  test("the file keeps the tab the first pane showed when it was saved", async ({ page }) => {
    await addTab(page, "Details");
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    const active = await inPage(page, (w) => {
      const eng = w.fe.editor.engine!;
      const Engine = eng.constructor as unknown as { open(b: Uint8Array, n: string): { tabsInfo(): { tabs: { id: string; name: string }[]; active?: string }; dispose(): void } };
      const again = Engine.open(eng.save(), "again.docx");
      const info = again.tabsInfo();
      again.dispose();
      return info.tabs.find((x) => x.id === info.active)?.name;
    });
    expect(active).toBe("Tab 1");
  });

  test("Find opens the tab its match is in, and so does a link to a bookmark there", async ({ page }) => {
    await addTab(page, "Details");
    await page.keyboard.type("The budget is in here");
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "budget" })).toBeHidden();
    // Find a word that is only in the hidden tab: the tab opens and the match shows.
    await inPage(page, (w) => w.fe.editor.focus());
    await page.keyboard.press(`${process.platform === "darwin" ? "Meta" : "Control"}+F`);
    await page.getByRole("search").getByRole("searchbox", { name: "Find" }).fill("budget");
    await page.keyboard.press("Enter");
    await expect(pane(page).getByRole("treeitem", { name: /Details/ })).toHaveAttribute("aria-selected", "true");
    await expect(flow(page).locator("[data-p]", { hasText: "budget" })).toBeVisible();
    await page.keyboard.press("Escape");
    // A jump to a paragraph of another tab (the navigation pane, a link to a bookmark) goes through the same place.
    await inPage(page, (w) => {
      const p = Number([...document.querySelectorAll<HTMLElement>(".flow [data-p]")].find((e) => e.textContent?.includes("Overview text"))!.dataset.p);
      (w.fe.editor as unknown as { scrollToParagraph(p: number): void }).scrollToParagraph(p);
    });
    await expect(pane(page).getByRole("treeitem", { name: /Tab 1/ })).toHaveAttribute("aria-selected", "true");
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
  });

  test("Select all takes the open tab only, so typing replaces that tab's text and leaves the others", async ({ page }) => {
    await addTab(page, "Details");
    await page.keyboard.type("Details text");
    await page.keyboard.press("Enter");
    await page.keyboard.type("More details");
    await addTab(page, "Last");
    await page.keyboard.type("Last text");
    await pane(page).getByRole("treeitem", { name: /Details/ }).click();
    await flow(page).locator("[data-p]", { hasText: "More details" }).click();
    await page.keyboard.press(`${process.platform === "darwin" ? "Meta" : "Control"}+A`);
    await page.keyboard.type("Replaced");
    const texts = await inPage(page, (w) => (w.fe.editor.engine as unknown as { text(): string }).text());
    expect(texts).toContain("Overview text");
    expect(texts).toContain("Last text");
    expect(texts).toContain("Replaced");
    expect(texts).not.toContain("More details");
    // The menu's Select all too.
    await chooseMenu(page, "Edit", "Select all");
    await page.keyboard.type("Again");
    const after = await inPage(page, (w) => (w.fe.editor.engine as unknown as { text(): string }).text());
    expect(after).toContain("Overview text");
    expect(after).toContain("Last text");
    expect(after).toContain("Again");
  });
});

// Tabs 3 (a body-level bookmark end counted as a block), 7 (notes of hidden tabs) and 8 (page numbers across tabs).
test.describe("document tabs: what each tab shows", () => {
  test("a bookmark end between paragraphs does not pull the next tab's first paragraph into the first tab", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ raw: '<w:bookmarkStart w:id="7" w:name="Closing"/><w:r><w:t>Alpha</w:t></w:r>' }, { body: '<w:bookmarkEnd w:id="7"/>' }, "Beta"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
    await addTab(page, "Second");
    await page.keyboard.type("Second tab text");
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Alpha" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Beta" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Second tab text" })).toBeHidden();
    await pane(page).getByRole("treeitem", { name: /Second/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Second tab text" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Beta" })).toBeHidden();
  });

  test("footnotes and endnotes of a tab that is not shown are not drawn on the open tab's page", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
    await addTab(page, "Details");
    await page.keyboard.type("Details text here");
    await inPage(page, (w) => {
      const ed = w.fe.editor;
      const p = Number([...document.querySelectorAll<HTMLElement>(".flow [data-p]")].find((e) => e.textContent?.includes("Details text here"))!.dataset.p);
      ed.exec({ op: "insert_footnote", at: { p, o: 7 }, text: "Footnote of the details" });
      ed.exec({ op: "insert_endnote", at: { p, o: 12 }, text: "Endnote of the details" });
    });
    await expect(page.locator(".sheet .notes")).toHaveCount(2);
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
    await expect(page.locator(".sheet .notes")).toHaveCount(0);
    // Back on the second tab they are drawn again.
    await pane(page).getByRole("treeitem", { name: /Details/ }).click();
    await expect(page.locator(".sheet .notes")).toHaveCount(2);
  });

  test("a table of contents numbers the headings of every tab by the pages all the tabs take together", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
    await addTab(page, "Details");
    // The second tab starts on its own page, so its heading is on page 2 although the open tab is the first.
    await inPage(page, (w) => {
      const ed = w.fe.editor;
      const p = Number([...document.querySelectorAll<HTMLElement>(".flow [data-p]")].at(-1)!.dataset.p);
      ed.exec([{ op: "insert_text", at: { p, o: 0 }, text: "Second heading" }, { op: "set_style", ids: [p], style: "Heading1" }]);
    });
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    const pages = await inPage(page, (w) => w.fe.editor.pageMap());
    const heading = await inPage(page, (w) => String([...document.querySelectorAll<HTMLElement>(".flow [data-p]")].find((e) => e.textContent?.includes("Second heading"))!.dataset.p));
    expect(pages[heading]).toBe(2);
    // The open tab is still the first, and the page count the person sees is the open tab's own.
    await expect(pane(page).getByRole("treeitem", { name: /Tab 1/ })).toHaveAttribute("aria-selected", "true");
    await expect(flow(page).locator("[data-p]", { hasText: "Second heading" })).toBeHidden();
  });

  // Select similar, Find's Select all and Select all text with this style chose text in tabs
  // that are not shown, and the selection stayed there, where it could be neither seen nor typed in.
  test("a selection of several places keeps to the tab shown", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text", "More of the overview"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
    await addTab(page, "Details");
    await page.keyboard.type("Details text");
    await page.keyboard.press("Enter");
    await page.keyboard.type("More details");
    // Back to the first tab: every paragraph is Normal, in both tabs.
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await flow(page).locator("[data-p]", { hasText: "Overview text" }).click();
    const hidden = () =>
      inPage(page, (w) => {
        const ed = w.fe.editor as unknown as { ranges(): { from: { p: number }; to: { p: number } }[] };
        return ed.ranges().filter((r) => [r.from.p, r.to.p].some((p) => !!document.querySelector(`.flow [data-p="${p}"]`)?.closest("[data-tab-hidden]"))).length;
      });
    const count = () => inPage(page, (w) => (w.fe.editor as unknown as { ranges(): unknown[] }).ranges().length);
    await chooseMenu(page, "Edit", "Select text with similar formatting");
    await expect(page.locator(".toast")).toContainText("2 places selected");
    expect(await hidden()).toBe(0);
    expect(await count()).toBe(2);
    // The same through Find, Select all: "text" is in both tabs.
    await page.keyboard.press("ControlOrMeta+f");
    await page.keyboard.type("text");
    await page.locator(".find-select").click();
    await expect(page.locator(".toast").last()).toContainText("1 match selected");
    expect(await hidden()).toBe(0);
  });

  // A note being edited stayed open when another tab was shown, with the text's caret in the
  // tab that went.
  test("showing another tab while editing a footnote goes back to the text, in the tab shown", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text", "More of the overview"] }] });
    const ids = await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { exec(op: unknown): unknown; tabs: { tabs: { id: string }[] }; openTab(id: string): boolean; sel: { focus: { p: number } } };
      const p = ed.sel.focus.p;
      ed.exec({ op: "insert_footnote", at: { p, o: 8 }, text: "A note" });
      ed.exec({ op: "tab", action: "add", name: "Second", first_name: "First" });
      return ed.tabs.tabs.map((t) => t.id);
    });
    await inPage(page, (w, id) => void (w.fe.editor as unknown as { openTab(id: string): boolean }).openTab(id), ids[0]);
    await inPage(page, (w) => w.fe.editor.enterStory("notes", 0));
    await expect(page.locator(".story-edit")).toBeFocused();
    await inPage(page, (w, id) => void (w.fe.editor as unknown as { openTab(id: string): boolean }).openTab(id), ids[1]);
    await expect(page.locator(".story-edit")).toHaveCount(0);
    await expect(page.locator(".flow")).toBeFocused();
    const hidden = await inPage(page, (w) => !!document.querySelector(`.flow [data-p="${w.fe.editor.sel!.focus.p}"]`)?.closest("[data-tab-hidden]"));
    expect(hidden).toBe(false);
  });
});
