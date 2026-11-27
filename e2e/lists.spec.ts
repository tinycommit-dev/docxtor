import { CHROMIUM_ONLY, caretAtEnd, chooseMenu, contextItem, contextMenu, expect, inPage, menuItem, openFixture, openMenu, para, RICH, runAt, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const writeClipboard = (page: Page, text: string) => page.evaluate((value: string) => navigator.clipboard.writeText(value), text);

/** The paragraph that has this text (the words are unique in the fixture). */
const item = (page: Page, text: string): Locator => page.locator(".flow .fp").filter({ hasText: text });
/** The marker of a list item: a bullet, a number or a box. */
const marker = (page: Page, text: string): Locator => item(page, text).locator(".lbl");
/** Right-click an item's text, after a plain click puts the caret there (a right-click inside a selection keeps it). */
async function rightClick(page: Page, text: string) {
  await item(page, text).click({ position: { x: 60, y: 6 } });
  await item(page, text).click({ button: "right", position: { x: 60, y: 6 } });
}
const modelId = async (page: Page, text: string) => Number(await item(page, text).getAttribute("data-p"));

/** Three new paragraphs after the document's last one, with the three of them selected. */
async function threeItems(page: Page, words = ["Apple", "Banana", "Cherry"]) {
  await caretAtEnd(page, RICH.last);
  for (const w of words) {
    await page.keyboard.press("Enter");
    await page.keyboard.type(w);
  }
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");
}

const bulletStyles = (page: Page) => page.getByRole("button", { name: "Bullet styles" });
const numberStyles = (page: Page) => page.getByRole("button", { name: "Numbering styles" });
const tile = (page: Page, name: string) => page.getByRole("menuitemradio", { name });

test.describe("list styles and checklists", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("a style from the bullet gallery restyles the list and the gallery marks it", { tag: "@core" }, async ({ page }) => {
    await threeItems(page);
    await page.getByRole("button", { name: "Bulleted list" }).click();
    await expect(marker(page, "Apple")).toHaveText("●");

    await bulletStyles(page).click();
    // The six bullet styles; the Bulleted list key made disc bullets, so that tile is the current one.
    for (const name of ["Disc bullets", "Diamond bullets", "Arrow bullets", "Star bullets", "Arrowhead bullets", "Box bullets"]) await expect(tile(page, name)).toBeVisible();
    await expect(tile(page, "Star bullets")).toHaveAttribute("aria-checked", "false");
    await expect(tile(page, "Disc bullets")).toHaveAttribute("aria-checked", "true");
    await tile(page, "Star bullets").click();

    await expect(page.locator(".pop")).toHaveCount(0);
    for (const w of ["Apple", "Banana", "Cherry"]) await expect(marker(page, w)).toHaveText("★");

    await bulletStyles(page).click();
    await expect(tile(page, "Star bullets")).toHaveAttribute("aria-checked", "true");
    await expect(tile(page, "Disc bullets")).toHaveAttribute("aria-checked", "false");
    await page.keyboard.press("Escape");
    await expect(page.locator(".pop")).toHaveCount(0);

    // One step to undo.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(marker(page, "Banana")).toHaveText("●");
  });

  test("the plain list keys use the presets: Disc and Decimal show as current, and pressing again turns the list off", { tag: "@core" }, async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+8`);
    await expect(marker(page, "Apple")).toHaveText("●");
    await bulletStyles(page).click();
    await expect(tile(page, "Disc bullets")).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await page.keyboard.press(`${MOD}+Shift+8`);
    await expect(marker(page, "Apple")).toHaveCount(0);
    await expect(marker(page, "Cherry")).toHaveCount(0);

    // From the other kind of list it switches over rather than turning off.
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Apple")).toHaveText("1.");
    await numberStyles(page).click();
    await expect(tile(page, "Decimal numbering 1. a. i.")).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await page.keyboard.press(`${MOD}+Shift+8`);
    await expect(marker(page, "Apple")).toHaveText("●");
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Cherry")).toHaveText("3.");
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Cherry")).toHaveCount(0);
  });

  test("numbering a paragraph right after a numbered list carries on from it, in one undo step", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+7`);
    // A new item, then Enter again to leave the list: a plain paragraph straight after item 3.
    await caretAtEnd(page, await modelId(page, "Cherry"));
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Date");
    await expect(marker(page, "Date")).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Date")).toHaveText("4.");

    await page.keyboard.press(`${MOD}+Z`);
    await expect(marker(page, "Date")).toHaveCount(0);
    await expect(marker(page, "Cherry")).toHaveText("3.");
  });

  test("a numbered style from the gallery carries on from a list in the same style, and starts at 1 otherwise", async ({ page }) => {
    await threeItems(page);
    await numberStyles(page).click();
    await tile(page, "Parenthesis numbering 1) a) i)").click();
    await expect(marker(page, "Cherry")).toHaveText("3)");
    await caretAtEnd(page, await modelId(page, "Cherry"));
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Date");
    await numberStyles(page).click();
    await tile(page, "Parenthesis numbering 1) a) i)").click();
    await expect(marker(page, "Date")).toHaveText("4)");
    // Another style doesn't continue it.
    await page.keyboard.press(`${MOD}+Z`);
    await numberStyles(page).click();
    await tile(page, "Capital letter numbering A. a. i.").click();
    await expect(marker(page, "Date")).toHaveText("A.");
  });

  test("with a plain paragraph in between, a new numbered list starts at 1", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+7`);
    await caretAtEnd(page, await modelId(page, "Cherry"));
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Plain");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Date");
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Date")).toHaveText("1.");
    await expect(marker(page, "Cherry")).toHaveText("3.");
  });

  test("the checklist box is drawn, at a size that follows the text", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    await marker(page, "Banana").click();
    const box = marker(page, "Banana").locator(".box");
    const size = await box.evaluate((el) => ({ w: el.getBoundingClientRect().width, h: el.getBoundingClientRect().height, font: parseFloat(getComputedStyle(el).fontSize) }));
    expect(size.w).toBeGreaterThan(size.font * 0.8);
    expect(size.w).toBeCloseTo(size.h, 0);
    await expect(box.locator("svg")).toHaveCount(1);
    await expect(marker(page, "Apple").locator(".box svg")).toHaveCount(0);
  });

  test("the numbering gallery's outline style numbers nested items 1.1.", async ({ page }) => {
    await threeItems(page);
    await numberStyles(page).click();
    await expect(tile(page, "Outline numbering 1. 1.1. 1.1.1.")).toBeVisible();
    await tile(page, "Outline numbering 1. 1.1. 1.1.1.").click();
    await expect(marker(page, "Apple")).toHaveText("1.");
    await expect(marker(page, "Cherry")).toHaveText("3.");

    // Tab nests the second item under the first.
    await item(page, "Banana").click({ position: { x: 60, y: 6 } });
    await page.keyboard.press("Tab");
    await expect(marker(page, "Banana")).toHaveText("1.1.");
    await expect(marker(page, "Cherry")).toHaveText("2.");

    await numberStyles(page).click();
    await expect(tile(page, "Outline numbering 1. 1.1. 1.1.1.")).toHaveAttribute("aria-checked", "true");
  });

  test("the gallery works from the keyboard: arrows move, Enter applies, Escape closes", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+8`);
    await bulletStyles(page).focus();
    await page.keyboard.press("Enter");
    // The focus moves into the gallery, on the tile for the current style.
    await expect(tile(page, "Disc bullets")).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(tile(page, "Diamond bullets")).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(tile(page, "Arrowhead bullets")).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await expect(tile(page, "Star bullets")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(tile(page, "Disc bullets")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.locator(".pop")).toHaveCount(0);
    // Escape gives the focus back to the key that opened it, and changes nothing.
    await expect(bulletStyles(page)).toBeFocused();
    await expect(marker(page, "Apple")).toHaveText("●");

    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Enter");
    await expect(marker(page, "Apple")).toHaveText("➔");
    // Applying puts the focus back in the document.
    await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
  });

  test("Ctrl/Cmd+Shift+9 makes a checklist, and the Checklist key shows it pressed", async ({ page }) => {
    await threeItems(page);
    const key = page.getByRole("button", { name: /^Checklist/ });
    await expect(key).not.toHaveClass(/\bon\b/);
    await page.keyboard.press(`${MOD}+Shift+9`);
    for (const w of ["Apple", "Banana", "Cherry"]) {
      await expect(marker(page, w)).toHaveAttribute("aria-checked", "false");
      await expect(marker(page, w)).toHaveAttribute("role", "checkbox");
      await expect(marker(page, w)).toHaveAttribute("aria-checked", "false");
    }
    await expect(key).toHaveClass(/\bon\b/);

    // Again takes the list off.
    await page.keyboard.press(`${MOD}+Shift+9`);
    await expect(marker(page, "Apple")).toHaveCount(0);
    await expect(key).not.toHaveClass(/\bon\b/);
  });

  test("clicking the box ticks an item and strikes its text; clicking again unticks it", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    const id = await modelId(page, "Banana");
    // The caret is at the end of the first item.
    await caretAtEnd(page, await modelId(page, "Apple"));

    await marker(page, "Banana").click();
    await expect(marker(page, "Banana")).toHaveAttribute("aria-checked", "true");
    await expect(marker(page, "Banana")).toHaveAttribute("aria-checked", "true");
    expect((await runAt(page, id, 2))?.strike).toBe(true);
    // The other items are untouched, and the caret stayed where it was.
    await expect(marker(page, "Apple")).toHaveAttribute("aria-checked", "false");
    await expect(marker(page, "Cherry")).toHaveAttribute("aria-checked", "false");
    await page.keyboard.type("!");
    await expect(item(page, "Apple")).toContainText("Apple!");
    await expect(item(page, "Banana")).toContainText("Banana");

    await marker(page, "Banana").click();
    await expect(marker(page, "Banana")).toHaveAttribute("aria-checked", "false");
    expect((await runAt(page, id, 2))?.strike).toBeFalsy();
  });

  test("a box can't be ticked while viewing", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    await chooseMenu(page, "View", "Mode", "Viewing");
    await marker(page, "Apple").click();
    await expect(marker(page, "Apple")).toHaveAttribute("aria-checked", "false");
    await expect(marker(page, "Apple")).toHaveCSS("cursor", "default");
  });

  test("Ctrl/Cmd+Alt+Enter ticks the item at the caret, and Mark as done is in the right-click menu", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    await page.keyboard.press("Escape");
    await item(page, "Apple").click({ position: { x: 60, y: 6 } });
    await page.keyboard.press(`${MOD}+Alt+Enter`);
    await expect(marker(page, "Apple")).toHaveAttribute("aria-checked", "true");

    await rightClick(page, "Apple");
    await contextItem(page, "Mark as not done").click();
    await expect(marker(page, "Apple")).toHaveAttribute("aria-checked", "false");
    await rightClick(page, "Cherry");
    await contextItem(page, "Mark as done").click();
    await expect(marker(page, "Cherry")).toHaveAttribute("aria-checked", "true");
    // Numbered-list actions are not offered on a checklist.
    await rightClick(page, "Cherry");
    await expect(contextItem(page, "Restart numbering")).toHaveCount(0);
  });

  test("Enter after a ticked item starts an unticked one; Enter on an empty item leaves the list", { tag: "@core" }, async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    await marker(page, "Cherry").click();
    await expect(marker(page, "Cherry")).toHaveAttribute("aria-checked", "true");
    await caretAtEnd(page, await modelId(page, "Cherry"));
    await page.keyboard.press("Enter");
    await page.keyboard.type("Date");
    await expect(marker(page, "Date")).toHaveAttribute("aria-checked", "false");
    await expect(marker(page, "Date")).toHaveAttribute("aria-checked", "false");
    await expect(marker(page, "Cherry")).toHaveAttribute("aria-checked", "true");

    // An empty item: Enter takes it out of the list.
    await page.keyboard.press("Enter");
    const labels = page.locator(".flow .fp .lbl");
    const before = await labels.count();
    await page.keyboard.press("Enter");
    await expect(labels).toHaveCount(before - 1);
    await page.keyboard.type("After");
    await expect(item(page, "After").locator(".lbl")).toHaveCount(0);
    await expect(marker(page, "Date")).toHaveAttribute("aria-checked", "false");
  });

  test("Backspace at the start of a checklist item takes it out of the list", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    await marker(page, "Banana").click();
    // To the start of the word (⌘← on a Mac, Ctrl+← elsewhere; Home would scroll the page on a Mac).
    await caretAtEnd(page, await modelId(page, "Banana"));
    await page.keyboard.press(`${MOD}+ArrowLeft`);
    await page.keyboard.press("Backspace");
    await expect(item(page, "Banana").locator(".lbl")).toHaveCount(0);
    await expect(item(page, "Banana")).toContainText("Banana");
    // It was ticked, so the strike-through went with the list.
    expect((await runAt(page, await modelId(page, "Banana"), 2))?.strike).toBeFalsy();
    await expect(marker(page, "Apple")).toHaveAttribute("aria-checked", "false");
    await expect(marker(page, "Cherry")).toHaveAttribute("aria-checked", "false");
  });

  test("the bullet key turns a checklist into plain bullets", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+9`);
    await page.getByRole("button", { name: "Bulleted list" }).click();
    await expect(marker(page, "Apple")).toHaveText("●");
    await expect(marker(page, "Cherry")).toHaveText("●");
  });

  test("Restart numbering and Continue previous numbering from the right-click menu", async ({ page }) => {
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Apple")).toHaveText("1.");
    await expect(marker(page, "Cherry")).toHaveText("3.");

    await rightClick(page, "Cherry");
    await expect(contextMenu(page)).toBeVisible();
    await contextItem(page, "Restart numbering").click();
    await expect(marker(page, "Cherry")).toHaveText("1.");
    await expect(marker(page, "Banana")).toHaveText("2.");

    await rightClick(page, "Cherry");
    await contextItem(page, "Continue previous numbering").click();
    await expect(marker(page, "Cherry")).toHaveText("3.");

    // Outside a numbered list the lines aren't offered.
    await para(page, RICH.last).click({ button: "right", position: { x: 40, y: 8 } });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Restart numbering")).toHaveCount(0);
  });

  test("Format, Bullets and numbering, Bullet style, Star restyles the list", async ({ page }) => {
    await threeItems(page);
    await page.getByRole("button", { name: "Bulleted list" }).click();
    await chooseMenu(page, "Format", "Bullets and numbering", "Bullet style", "Star");
    await expect(marker(page, "Apple")).toHaveText("★");

    // The menu marks the current style; Numbering style is its own submenu.
    await openMenu(page, "Format");
    await menuItem(page, "Bullets and numbering").click();
    await menuItem(page, "Bullet style").click();
    await expect(menuItem(page, "Star")).toHaveAttribute("aria-checked", "true");
    await expect(menuItem(page, "Disc")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Numbering style").click();
    await menuItem(page, "A. a. i.").click();
    await expect(marker(page, "Apple")).toHaveText("A.");
    await expect(marker(page, "Banana")).toHaveText("B.");
  });

  test("Restart numbering and Continue previous numbering are greyed out outside a numbered list", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openMenu(page, "Format");
    await menuItem(page, "Bullets and numbering").click();
    await expect(menuItem(page, "Restart numbering")).toBeDisabled();
    await expect(menuItem(page, "Continue previous numbering")).toBeDisabled();
    await expect(menuItem(page, "Done")).toBeDisabled();
  });

  test("Continue previous numbering says so when there is no earlier list", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await page.keyboard.press("Enter");
    await page.keyboard.type("Solo");
    await page.keyboard.press(`${MOD}+Shift+7`);
    await chooseMenu(page, "Format", "Bullets and numbering", "Continue previous numbering");
    await expect(page.locator(".toast")).toContainText("no earlier list");
  });

  test("Paste from Markdown makes a checklist from task items", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await writeClipboard(page, "- [ ] a\n- [x] b");
    await caretAtEnd(page, RICH.last);
    await page.keyboard.press("Enter");
    await chooseMenu(page, "Edit", "Paste from Markdown");
    const boxes = page.locator(".flow .fp .lbl[role=checkbox]");
    await expect(boxes).toHaveCount(2);
    await expect(boxes.nth(0)).toHaveAttribute("aria-checked", "false");
    await expect(boxes.nth(1)).toHaveAttribute("aria-checked", "true");
    await expect(boxes.nth(1)).toHaveAttribute("aria-checked", "true");
    expect(await inPage(page, (w) => w.engine!.text())).toContain("b");
  });
});

test.describe("the ruler in a list", () => {
  /** A blank document: Intro, three bullets with a nested one under Two, After. */
  async function bullets(page: Page) {
    await page.goto("/");
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    await inPage(page, (w) => w.createIn(w.files.library, ""));
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.type("Intro");
    for (const t of ["One", "Two", "Sub", "Three", "After"]) {
      await page.keyboard.press("Enter");
      await page.keyboard.type(t);
    }
    await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { exec(op: unknown, o: unknown): unknown; sel: unknown };
      const id = (t: string) => Number([...document.querySelectorAll<HTMLElement>(".flow [data-p]")].find((p) => p.textContent === t)!.dataset.p);
      const [one, two, sub, three] = ["One", "Two", "Sub", "Three"].map(id);
      ed.exec({ op: "toggle_list", ids: [one, two, sub, three], kind: "bullet" }, { select: ed.sel });
      ed.exec({ op: "indent", ids: [sub], delta: 1 }, { select: ed.sel });
    });
    await expect(item(page, "Sub").locator(".lbl")).toHaveText("◦");
  }
  /** Each paragraph's left and first-line indents, by its text. */
  const indents = (page: Page) =>
    page.evaluate(() =>
      Object.fromEntries(
        [...document.querySelectorAll<HTMLElement>(".flow [data-p]")].map((p) => {
          const f = (window as unknown as { docxtor: { fe: { editor: { engine: { formatAt(x: unknown): { para: { indLeft?: number; indFirst?: number; indHanging?: number } } } } } } }).docxtor.fe.editor.engine.formatAt({ p: Number(p.dataset.p), o: 0 }).para;
          return [p.textContent!.replace(/^[•◦]/, ""), [f.indLeft ?? 0, f.indFirst ?? -(f.indHanging ?? 0)]];
        }),
      ),
    );
  async function drag(page: Page, kind: "left" | "first", dx: number) {
    // The ruler follows the caret and the zoom: wait until the marker stays put before taking hold of it.
    const box = () => page.locator(`.ruler-marker.${kind}`).evaluate((e) => Math.round(e.getBoundingClientRect().x));
    let last = -1;
    await expect.poll(async () => {
      const x = await box();
      const same = x === last;
      last = x;
      return same;
    }, { intervals: [150] }).toBe(true);
    const m = (await page.locator(`.ruler-marker.${kind}`).boundingBox())!;
    await page.mouse.move(m.x + m.width / 2, m.y + m.height / 2);
    await page.mouse.down();
    await page.mouse.move(m.x + m.width / 2 + dx / 2, m.y + m.height / 2, { steps: 4 });
    await page.mouse.move(m.x + m.width / 2 + dx, m.y + m.height / 2, { steps: 4 });
    await page.mouse.up();
  }

  test("moving an indent with the caret in one item moves every item of its level, and one undo puts them back", async ({ page }) => {
    await bullets(page);
    const before = await indents(page);
    await item(page, "Two").click();
    await drag(page, "left", 48);
    await expect.poll(async () => (await indents(page)).One[0]).toBe(before.One[0] + 720);
    const after = await indents(page);
    expect(after.Two[0]).toBe(before.Two[0] + 720);
    expect(after.Three[0]).toBe(before.Three[0] + 720);
    // The nested level and the paragraphs around the list stay where they were.
    expect(after.Sub).toEqual(before.Sub);
    expect(after.Intro).toEqual(before.Intro);
    expect(after.After).toEqual(before.After);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => indents(page)).toEqual(before);
  });

  test("the first-line marker moves every item's bullet of the level too; a nested item moves only its own level", async ({ page }) => {
    await bullets(page);
    const before = await indents(page);
    await item(page, "Three").click();
    await drag(page, "first", -24);
    await expect.poll(async () => (await indents(page)).One[1]).toBe(before.One[1] - 360);
    expect((await indents(page)).Two[1]).toBe(before.Two[1] - 360);
    expect((await indents(page)).Sub).toEqual(before.Sub);
    await item(page, "Sub").click();
    await drag(page, "left", 48);
    await expect.poll(async () => (await indents(page)).Sub[0]).toBe(before.Sub[0] + 720);
    expect((await indents(page)).Two[0]).toBe(before.Two[0]);
  });

  test("another list with the same list id, and a bullet in the footer, stay where they are", async ({ page }) => {
    await bullets(page);
    // After, then two more items put in the same list (as Word files do for unrelated lists), and a bulleted footer.
    await page.keyboard.press("Enter");
    await page.keyboard.type("Four");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Five");
    const num = await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { exec(op: unknown, o?: unknown): unknown; sel: unknown };
      const id = (t: string) => Number([...document.querySelectorAll<HTMLElement>(".flow [data-p]")].find((p) => p.textContent?.replace(/^[•◦]/, "") === t)!.dataset.p);
      const numId = Number(document.querySelector<HTMLElement>(`.flow [data-p="${id("One")}"]`)!.dataset.ln!.split(":")[0]);
      ed.exec({ op: "apply_list", ids: [id("Four"), id("Five")], num_id: numId }, { select: ed.sel });
      return numId;
    });
    // The footer gets an item of the same list and level (it is drawn without paragraph ids, so the ruler must skip it).
    await inPage(page, (w) => (w.fe.editor as unknown as { enterStory(z: string, p: number): void }).enterStory("footer", 0));
    await page.keyboard.type("Footer bullet");
    await page.getByRole("button", { name: "Bulleted list", exact: true }).click();
    await inPage(page, (w, n) => {
      const ed = w.fe.editor as unknown as { exec(op: unknown, o?: unknown): unknown; sel: { focus: { p: number } } };
      ed.exec({ op: "apply_list", ids: [ed.sel.focus.p], num_id: n }, { select: ed.sel });
    }, num);
    await page.keyboard.press("Escape");
    await expect(page.locator(`[data-ln^="${num}:"]:not([data-p])`).first()).toBeAttached();
    await expect(item(page, "Four").locator(".lbl")).toHaveText("•");
    const before = await indents(page);
    await item(page, "Two").click();
    await drag(page, "left", 48);
    await expect.poll(async () => (await indents(page)).One[0]).toBe(before.One[0] + 720);
    expect((await indents(page)).Three[0]).toBe(before.Three[0] + 720);
    expect((await indents(page)).Four).toEqual(before.Four);
    expect((await indents(page)).Five).toEqual(before.Five);
  });
});
