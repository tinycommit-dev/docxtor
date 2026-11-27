// What a keystroke costs the page, and that the saving never shows. The editor keeps a few things between keys so
// that typing in a long document does not walk the whole of it: the order of the blocks (the engine sends it only
// when it moved), where each paragraph's element is, which content controls an extension has drawn, which cells are
// marked, where the spelling underlines were measured, what the rulers last drew. Every test here types and then
// looks for what a stale copy of one of those would get wrong, and one counts the page's layouts per key.
import { openBuilt } from "./support/docx-builder";
import { endOfLine, run, startOfLine } from "./support/controls-markup";
import { table } from "./support/table-markup";
import { CHROMIUM_ONLY, chooseMenu, dialog, expect, inPage, showPara, test, type Page } from "./helpers";

const settle = (page: Page) => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));

/** Type one key at a time, each given its frame, as the performance harness does. */
async function typeKeys(page: Page, text: string) {
  for (const ch of text) {
    await page.keyboard.type(ch);
    await settle(page);
  }
}

test.describe("what a keystroke costs", () => {
  test("a keystroke in a long document lays the page out about twice, not four times", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.cdp } }, async ({ page }) => {
    const words = "the quick brown fox jumps over the lazy dog and keeps on running through the field".split(" ");
    const paragraphs = Array.from({ length: 1500 }, (_, i) => `Paragraph ${i}: ${words.map((w, k) => words[(k + i) % words.length]).join(" ")}.`);
    await openBuilt(page, { sections: [{ paragraphs }] });
    await showPara(page, 750);
    await page.locator(".flow [data-p]").nth(750).click();
    await endOfLine(page);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Performance.enable");
    const counts = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]));
    await typeKeys(page, "warm");
    const before = await counts();
    await typeKeys(page, "abcdefghijklmnopqrst");
    const after = await counts();
    const layouts = (after.LayoutCount - before.LayoutCount) / 20;
    const styles = (after.RecalcStyleCount - before.RecalcStyleCount) / 20;
    // Before the editor's per-key work was kept short these were 3.9 and 5.9: the forced layout of setting the caret
    // after the text changed, and then the toolbar, the rulers, the selection bubble and the spelling each asked again.
    expect(layouts).toBeLessThan(3);
    expect(styles).toBeLessThan(3);
    await expect(page.locator(".flow [data-p]").nth(750)).toContainText("abcdefghijklmnopqrst");
  });

  test("typing leaves the next paragraph's element alone; a change to the space after the paragraph draws it again", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First paragraph here.", "Second paragraph here.", "Third one."] }] });
    const paras = page.locator(".flow [data-p]");
    const tag = () => paras.nth(1).evaluate((el) => ((el as unknown as { kept: boolean }).kept = true));
    const kept = () => paras.nth(1).evaluate((el) => (el as unknown as { kept?: boolean }).kept === true);
    const gap = () => page.evaluate(() => {
      const [a, b] = Array.from(document.querySelectorAll(".flow [data-p]")).map((e) => e.getBoundingClientRect());
      return b.top - a.bottom;
    });
    await tag();
    // Typing changes the text alone: what the next paragraph reads of this one (its spacing, style, borders) is as it was.
    await paras.first().click();
    await endOfLine(page);
    await typeKeys(page, "abc");
    await expect(paras.first()).toContainText("here.abc");
    expect(await kept()).toBe(true);
    // The space after it is drawn on the next paragraph: that one is drawn again, further down.
    const before = await gap();
    const id = Number(await paras.first().getAttribute("data-p"));
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "format_paragraphs", ids: [p], format: { spaceAfter: 600 } }), id);
    await expect.poll(kept).toBe(false);
    await expect.poll(gap).toBeGreaterThan(before + 30);
  });

  test("the order of the blocks the editor keeps follows splits, joins, undo and redo", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First", "Second", "Third", "Fourth"] }] });
    const order = () =>
      inPage(page, (w) => {
        const dom = Array.from(document.querySelectorAll<HTMLElement>(".editor-host .flow > [data-id]")).map((e) => Number(e.dataset.id));
        return { dom, engine: w.fe.editor.engine!.renderAll().order };
      });
    const same = async () => {
      const o = await order();
      expect(o.dom).toEqual(o.engine);
      return o.dom.length;
    };
    await page.locator(".flow [data-p]").nth(1).click();
    await endOfLine(page);
    // Typing keeps the order (nothing is sent), and the page still has the four.
    await typeKeys(page, "abc");
    expect(await same()).toBe(4);
    // A split sends it again: five blocks, in the right places.
    await page.keyboard.press("Enter");
    await typeKeys(page, "new");
    expect(await same()).toBe(5);
    await expect(page.locator(".flow [data-p]")).toHaveText(["First", "Secondabc", "new", "Third", "Fourth"]);
    // A join takes one away.
    await startOfLine(page);
    await page.keyboard.press("Backspace");
    expect(await same()).toBe(4);
    await expect(page.locator(".flow [data-p]")).toHaveText(["First", "Secondabcnew", "Third", "Fourth"]);
    // Undo and redo put blocks back; typing after them goes where the caret is.
    await inPage(page, (w) => w.fe.editor.undo());
    expect(await same()).toBe(5);
    await inPage(page, (w) => w.fe.editor.undo());
    await same();
    await inPage(page, (w) => w.fe.editor.exec({ op: "delete_blocks", ids: [w.engine!.renderAll().order[2]] }));
    expect(await same()).toBe(await same());
    await page.locator(".flow [data-p]").last().click();
    await endOfLine(page);
    await typeKeys(page, "!");
    await expect(page.locator(".flow [data-p]").last()).toContainText("Fourth!");
    expect(await same()).toBe(await same());
  });

  test("the selection bubble follows the selection, and typing, which leaves a caret, hides it", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Select some words in this line", "Another line"] }] });
    const bubble = page.locator(".bubble");
    await page.locator(".flow [data-p]").first().click();
    await startOfLine(page);
    await expect(bubble).toBeHidden();
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await expect(bubble).toBeVisible();
    // Collapsing the selection hides it; typing a letter over a selection leaves a caret and a hidden bubble.
    await page.keyboard.press("ArrowRight");
    await expect(bubble).toBeHidden();
    await startOfLine(page);
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await expect(bubble).toBeVisible();
    await page.keyboard.type("Q");
    await expect(page.locator(".flow [data-p]").first()).toContainText("Qlect some words");
    await expect(bubble).toBeHidden();
    // And it still comes back for the next selection, after several keys with a caret.
    await typeKeys(page, "xyz");
    await expect(bubble).toBeHidden();
    await page.keyboard.press("Shift+ArrowLeft");
    await page.keyboard.press("Shift+ArrowLeft");
    await expect(bubble).toBeVisible();
  });
});

test.describe("content controls drawn by an extension", () => {
  const rating = `<w:sdt><w:sdtPr><w:alias w:val="Stars"/><w:tag w:val="acme-rating"/><w:id w:val="400"/><w:richText/></w:sdtPr><w:sdtContent><w:r><w:t>3</w:t></w:r></w:sdtContent></w:sdt>`;

  async function register(page: Page) {
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { use(x: unknown): Promise<void> } } }).docxtor;
      await w.fe.use({
        id: "acme.rating",
        name: "Rating",
        activate(ctx: { contentControls: { register(c: unknown): void } }) {
          ctx.contentControls.register({ kind: "acme-rating", label: "Rating", render: (els: HTMLElement[]) => els.forEach((e) => e.classList.add("acme-stars")) });
        },
      });
    });
  }

  test("a control is drawn again, decorated, when typing, a split and undo draw its paragraph again", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Intro line", { raw: run("Rating: ") + rating }, "Closing line"] }] });
    await register(page);
    const stars = page.locator(".flow .acme-stars");
    await expect(stars).toHaveText("3");
    // Typing in the paragraph that holds it draws that paragraph again as a new element: the control is decorated again.
    const holder = page.locator(".flow [data-p]").nth(1);
    await holder.click();
    await startOfLine(page);
    await typeKeys(page, "ab");
    await expect(holder).toContainText("abRating: 3");
    await expect(stars).toHaveCount(1);
    await expect(stars).toHaveText("3");
    // Typing in another paragraph leaves it as it was.
    await page.locator(".flow [data-p]").first().click();
    await endOfLine(page);
    await typeKeys(page, "xy");
    await expect(stars).toHaveCount(1);
    // A paragraph split in front of it draws the paragraphs again.
    await page.locator(".flow [data-p]").nth(1).click();
    await startOfLine(page);
    await page.keyboard.press("Enter");
    await expect(stars).toHaveCount(1);
    await expect(stars).toHaveText("3");
    // Undo brings the old paragraph back, and it is decorated too.
    await inPage(page, (w) => w.fe.editor.undo());
    await expect(stars).toHaveCount(1);
    await expect(stars).toHaveText("3");
    // A kind that arrives after the document is drawn to the paragraphs that are already there.
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { use(x: unknown): Promise<void> } } }).docxtor;
      await w.fe.use({
        id: "acme.second",
        name: "Second",
        activate(ctx: { contentControls: { register(c: unknown): void } }) {
          ctx.contentControls.register({ kind: "acme-other", label: "Other", render: () => undefined });
        },
      });
    });
    await expect(stars).toHaveCount(1);
    await typeKeys(page, "z");
    await expect(stars).toHaveText("3");
  });
});

test.describe("document tabs while typing", () => {
  async function addTab(page: Page, name: string) {
    await chooseMenu(page, "Insert", "Tab…");
    const d = dialog(page, "Add tab");
    await d.getByRole("textbox").fill(name);
    await d.getByRole("button", { name: "Add" }).click();
  }
  const flow = (page: Page) => page.locator(".editor-host .flow").first();
  const pane = (page: Page) => page.locator(".doc-tabs-pane");

  test("typing in a document without tabs, then adding one, then typing in each tab and switching", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text", "More of the overview"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
    // Many keys with no tabs: the editor does not ask the engine about tabs for each of them, and a tab added after still shows.
    await page.locator(".flow [data-p]").first().click();
    await endOfLine(page);
    await typeKeys(page, " one two three");
    await addTab(page, "Details");
    await expect(pane(page).getByRole("treeitem")).toHaveCount(2);
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text one two three" })).toBeHidden();
    await typeKeys(page, "second tab");
    await expect(flow(page)).toContainText(/second tab/i);
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text one two three" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: /second tab/i })).toBeHidden();
    // Typing in the first tab leaves the second hidden, and its blocks are not marked shown by the keys.
    await page.locator(".flow [data-p]").first().click();
    await endOfLine(page);
    await typeKeys(page, " more");
    await expect(flow(page).locator("[data-p]", { hasText: /second tab/i })).toBeHidden();
    await expect(flow(page).locator("[data-p]", { hasText: "three more" })).toBeVisible();
    await pane(page).getByRole("treeitem", { name: /Details/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: /second tab/i })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeHidden();
    // A paragraph of the tab that is not shown, drawn again by something other than the keys (an extension's edit), is still not shown.
    await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { sel: unknown; exec(ops: unknown, opts: unknown): unknown };
      return ed.exec({ op: "insert_text", at: { p: 1, o: 0 }, text: "Edited " }, { select: ed.sel });
    });
    await expect(page.locator('.flow [data-p="1"]')).toHaveAttribute("data-tab-hidden", "");
    await expect(flow(page).locator("[data-p]", { hasText: /second tab/i })).toBeVisible();
    // Undo takes back the last typing (in the first tab) and the tab shown stays the one the person chose.
    await inPage(page, (w) => w.fe.editor.undo());
    await expect(flow(page).locator("[data-p]", { hasText: /second tab/i })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text" })).toBeHidden();
    await inPage(page, (w) => w.fe.editor.undo());
    await pane(page).getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow(page).locator("[data-p]", { hasText: "Overview text one two three" })).toBeVisible();
    await expect(flow(page).locator("[data-p]", { hasText: "three more" })).toHaveCount(0);
  });
});

test.describe("what is marked while typing", () => {
  test("cells marked as selected are unmarked by the next key, wherever the caret goes", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before the table", table({ rows: 2, cols: 3 }), "After the table"] }] });
    const cell = (r: number, c: number) => page.locator(".flow table.ft tr").nth(r).locator("td").nth(c);
    await cell(0, 0).click();
    await cell(1, 1).click({ modifiers: ["Shift"] });
    await expect(page.locator(".flow .cell-sel")).toHaveCount(4);
    await expect(page.locator(".flow .cells-selected")).toHaveCount(1);
    // A click outside the table takes the marks away; typing there leaves none. (The click is right of where the table's strip floats over the paragraph above it.)
    await page.locator(".flow [data-p]").first().click({ position: { x: 580, y: 8 } });
    await endOfLine(page);
    await typeKeys(page, "ok");
    await expect(page.locator(".flow .cell-sel")).toHaveCount(0);
    await expect(page.locator(".flow .cells-selected")).toHaveCount(0);
    // Marked again, and unmarked again by a key typed in a cell.
    await cell(0, 0).click();
    await cell(0, 2).click({ modifiers: ["Shift"] });
    await expect(page.locator(".flow .cell-sel")).toHaveCount(3);
    await cell(1, 0).click();
    await typeKeys(page, "k");
    await expect(page.locator(".flow .cell-sel")).toHaveCount(0);
    await expect(cell(1, 0)).toContainText("k");
  });

  test("the ruler follows the caret's paragraph and is not drawn stale by typing", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Indented paragraph", ppr: '<w:ind w:left="1440"/>' }, "Plain paragraph", { text: "Hanging paragraph", ppr: '<w:ind w:left="720" w:hanging="360"/>' }] }] });
    const marker = (kind: string) => page.locator(`.ruler-marker.${kind}`).evaluate((e) => Math.round(parseFloat((e as HTMLElement).style.left)));
    const at = async (n: number) => {
      await page.locator(".flow [data-p]").nth(n).click();
      await endOfLine(page);
      await settle(page);
    };
    await at(0);
    const indented = await marker("left");
    await typeKeys(page, "abc");
    expect(await marker("left")).toBe(indented);
    await at(1);
    const plain = await marker("left");
    expect(plain).toBeLessThan(indented);
    await typeKeys(page, "abc");
    expect(await marker("left")).toBe(plain);
    await at(2);
    const hanging = await marker("left");
    expect(hanging).toBeGreaterThan(plain);
    expect(hanging).toBeLessThan(indented);
    expect(await marker("first")).toBeLessThan(hanging);
    await typeKeys(page, "abc");
    expect(await marker("left")).toBe(hanging);
    // Back to the first paragraph: the marker is where it was.
    await at(0);
    expect(await marker("left")).toBe(indented);
  });
});

test.describe("spelling underlines kept between keys", () => {
  /** How far each underline is from the word it marks (px): its box against the range of the word in the text. */
  const misplaced = (page: Page) =>
    page.evaluate(() => {
      const out: { word: string; off: number }[] = [];
      for (const u of Array.from(document.querySelectorAll<HTMLElement>(".sp-layer .sp-u"))) {
        const p = document.querySelector<HTMLElement>(`.flow [data-p="${u.dataset.p}"]`);
        if (!p) continue;
        const from = Number(u.dataset.from);
        const to = Number(u.dataset.to);
        let at = 0;
        const range = document.createRange();
        let set = false;
        const walk = (n: Node) => {
          for (const c of Array.from(n.childNodes)) {
            if (c.nodeType === Node.TEXT_NODE) {
              const len = (c as Text).data.length;
              if (!set && from >= at && from < at + len) {
                range.setStart(c, from - at);
                set = true;
              }
              if (set && to > at && to <= at + len) {
                range.setEnd(c, to - at);
                return true;
              }
              at += len;
            } else if (c.nodeType === Node.ELEMENT_NODE && walk(c)) return true;
          }
          return false;
        };
        walk(p);
        const r = range.getClientRects();
        const b = u.getBoundingClientRect();
        const want = Array.from(r).find((x) => Math.abs(x.top - b.top) < 40 && Math.abs(x.left - b.left) < 40) ?? r[0];
        if (!want) continue;
        out.push({ word: u.dataset.word ?? "", off: Math.max(Math.abs(want.left - b.left), Math.abs(want.top - b.top), Math.abs(want.width - b.width), Math.abs(want.height - b.height)) });
      }
      return out;
    });

  const flowText = "Please recieve the parcel and the definately correct invoice. ".repeat(2);

  test("underlines stay on their words after typing earlier in the same paragraph and in the paragraph above", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["A short line above.", flowText, "Below we beleive that too.", flowText] }] });
    const under = page.locator(".sp-layer .sp-u");
    await expect.poll(() => under.count()).toBeGreaterThanOrEqual(8);
    expect((await misplaced(page)).every((m) => m.off < 1.5)).toBe(true);
    const count = await under.count();
    // Earlier in the same paragraph: the words move right and down as the line wraps differently.
    await page.locator(".flow [data-p]").nth(1).click();
    await startOfLine(page);
    await typeKeys(page, "This text goes in front of it all and wraps the line differently now. ");
    await expect.poll(async () => (await under.count()) > 0 && (await misplaced(page)).every((m) => m.off < 1.5)).toBe(true);
    await expect.poll(() => under.count()).toBeGreaterThanOrEqual(count);
    // In the paragraph above: everything below it moves down, and its underlines with it, though their paragraphs did not change.
    await page.locator(".flow [data-p]").first().click();
    await endOfLine(page);
    await typeKeys(page, " Now this paragraph is long enough to wrap onto a second line and a third one as well, which pushes the rest of the page down.");
    await expect.poll(async () => (await under.count()) >= count && (await misplaced(page)).every((m) => m.off < 1.5), { timeout: 10_000 }).toBe(true);
    // And the word under the caret gets its underline once the caret leaves it.
    await page.locator(".flow [data-p]").nth(2).click();
    await endOfLine(page);
    await typeKeys(page, " recieve");
    await page.keyboard.press("Space");
    await expect.poll(async () => (await misplaced(page)).every((m) => m.off < 1.5), { timeout: 10_000 }).toBe(true);
  });

  test("the paragraph being typed in is marked again after a pause, and the others keep their underlines meanwhile", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["We recieve it today.", "Below we beleive that too."] }] });
    const ids = await page.locator(".flow [data-p]").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.p));
    const under = (p: string | undefined) => page.locator(`.sp-layer .sp-u[data-p="${p}"]`);
    await expect(under(ids[0])).toHaveCount(1);
    await expect(under(ids[1])).toHaveCount(1);
    await under(ids[1]).evaluate((el) => ((el as unknown as { kept: boolean }).kept = true));
    await page.locator(".flow [data-p]").first().click();
    await endOfLine(page);
    await typeKeys(page, " and more");
    // While the keys come, the typed paragraph waits; the other one's underline is the same element, untouched.
    await expect(under(ids[0])).toHaveCount(0);
    expect(await under(ids[1]).evaluate((el) => (el as unknown as { kept?: boolean }).kept === true)).toBe(true);
    // After the pause it is checked and marked again, in the right place.
    await expect(under(ids[0])).toHaveCount(1);
    expect((await misplaced(page)).every((m) => m.off < 1.5)).toBe(true);
  });

  test("in a long document, keys that keep the typed paragraph's height start no pass over the underlines; a key that adds a line moves the ones below at once", async ({ page }) => {
    // Over 400 blocks the pages are laid out only after a 120 ms pause, and the typed paragraph is marked again after 500 ms: both are held back,
    // so what is checked is what happens while keys come.
    await page.addInitScript(() => {
      const real = window.setTimeout.bind(window);
      (window as unknown as { setTimeout: unknown }).setTimeout = (fn: TimerHandler, ms?: number, ...a: unknown[]) => real(fn, ms === 120 || ms === 500 ? 600_000 : ms, ...a);
    });
    const filler = Array.from({ length: 450 }, (_, i) => `Filler paragraph number ${i} with ordinary words only.`);
    const target = ["We recieve it today.", "Below we beleive that too.", "And agian here."];
    await openBuilt(page, { sections: [{ paragraphs: [...filler.slice(0, 220), ...target, ...filler.slice(220)] }] });
    await showPara(page, 220);
    const under = page.locator(".sp-layer .sp-u");
    await expect.poll(() => under.count(), { timeout: 15_000 }).toBe(3);
    const passes = () => page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { spell: { passes: number } } } } }).docxtor.fe.editor.spell.passes);
    const frames = () => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await page.locator(".flow [data-p]").nth(220).click();
    await endOfLine(page);
    await typeKeys(page, " a");
    await frames();
    const before = await passes();
    await typeKeys(page, "nd so");
    await frames();
    expect(await passes()).toBe(before);
    // Now the paragraph grows a line: the two below move down, and their underlines with them, before any pause.
    const top = await page.locator(".flow [data-p]").nth(221).evaluate((el) => el.getBoundingClientRect().top);
    await typeKeys(page, " on, and this goes on long enough to wrap onto a second line of the page for sure, with a few more words after it.");
    const after = await page.locator(".flow [data-p]").nth(221).evaluate((el) => el.getBoundingClientRect().top);
    expect(after).toBeGreaterThan(top + 5);
    await expect.poll(async () => (await under.count()) === 2 && (await misplaced(page)).every((m) => m.off < 1.5)).toBe(true);
    expect(await passes()).toBeGreaterThan(before);
  });

  test("in a long document, where the pages are laid out only after a pause, the paragraphs below the typing keep their underlines in step with the text", async ({ page }) => {
    // Over 400 blocks the pagination waits 120 ms after the last key, and then redraws every underline. Here that pause never comes (the editor's
    // 120 ms timer is held back), so what is checked is the underlines' own bookkeeping: a paragraph that moved without being touched is measured again.
    await page.addInitScript(() => {
      const real = window.setTimeout.bind(window);
      (window as unknown as { setTimeout: unknown }).setTimeout = (fn: TimerHandler, ms?: number, ...a: unknown[]) => real(fn, ms === 120 ? 600_000 : ms, ...a);
    });
    const filler = Array.from({ length: 450 }, (_, i) => `Filler paragraph number ${i} with ordinary words only.`);
    const target = ["A short line above.", flowText, "Below we beleive that too.", flowText];
    await openBuilt(page, { sections: [{ paragraphs: [...filler.slice(0, 220), ...target, ...filler.slice(220)] }] });
    await showPara(page, 220);
    await page.locator(".flow [data-p]").nth(220).click();
    await endOfLine(page);
    const under = page.locator(".sp-layer .sp-u");
    await expect.poll(() => under.count(), { timeout: 15_000 }).toBeGreaterThanOrEqual(8);
    await expect.poll(async () => (await misplaced(page)).every((m) => m.off < 1.5)).toBe(true);
    const count = await under.count();
    // Keys that make the paragraph taller: the ones below move down at once.
    await typeKeys(page, " This paragraph is long enough to wrap onto a second line and a third one as well, which pushes the rest of the page down.");
    // The caret moving (what asks for a soft draw while a word is held) draws the layer again without the pagination's say: the paragraphs below moved, so their boxes are measured again.
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { spell: { invalidate(soft: boolean): void } } } } }).docxtor.fe.editor.spell.invalidate(true));
    await expect.poll(async () => (await under.count()) >= count - 1 && (await misplaced(page)).every((m) => m.off < 1.5), { timeout: 5_000 }).toBe(true);
  });
});
