// Select all matching text: Docxtor's own selection of several ranges at once. The real selection stays the first
// range; the others are drawn on a layer over the pages. Formatting, deleting, typing, copying and pasting apply
// to all of them as one undoable step, and any click or caret move ends it.
import { openBuilt } from "./support/docx-builder";
import { CHROMIUM_ONLY, chooseMenu, contextItem, contextMenu, expect, inPage, openMenu, menuItem, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** "apple" occurs twice, three times (once inside "pineapple"), nowhere, and once as "Apple", which is another text. */
const PARAS = ["The apple fell near the apple tree.", "A banana is not an apple, but a pineapple contains apple.", "Nothing to see here.", "Apple with a capital letter is different."];

/** Five places, as {p, from, to}, once the document is open. */
interface Place {
  p: number;
  from: number;
  to: number;
}

async function open(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: PARAS }] });
}

const ids = (page: Page) => page.locator(".flow .fp").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());
/** The run formatting of the character at a position in the document on screen (not the app's first document). */
const runOn = (page: Page, p: number, o: number) => inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos)?.run, { p, o });
const rects = (page: Page) => page.locator(".xs-layer .xs-r");
const carets = (page: Page) => page.locator(".xs-layer .xs-caret");
const places = async (page: Page, needle: string): Promise<Place[]> => {
  const all = await ids(page);
  const lines = (await text(page)).split("\n");
  const out: Place[] = [];
  lines.forEach((line, i) => {
    for (let at = line.indexOf(needle); at >= 0; at = line.indexOf(needle, at + needle.length)) out.push({ p: all[i], from: at, to: at + needle.length });
  });
  return out;
};

interface Sel {
  from: { p: number; o: number };
  to: { p: number; o: number };
  text: string;
}
interface Api {
  selections(): Sel[];
  selection(): Sel | null;
  selectRanges(r: unknown[]): void;
}
/** Give the page an extension of its own, so the tests can see what an extension sees of the document. */
async function probe(page: Page) {
  await page.evaluate(async () => {
    const w = window as unknown as { __doc?: unknown; docxtor: { fe: { use(e: unknown): Promise<void> } } };
    if (w.__doc) return;
    await w.docxtor.fe.use({ id: "test.probe", name: "Selection probe", activate: (ctx: { document: unknown }) => void (w.__doc = ctx.document) });
  });
}
const api = <R, A = undefined>(page: Page, fn: (doc: Api, arg: A) => R, arg?: A): Promise<R> =>
  page.evaluate(([src, a]) => new Function("doc", "a", `return (${src})(window.__doc, a)`)(src, a) as R, [fn.toString(), arg] as const) as Promise<R>;

/** Select a stretch of one paragraph in the model and give the text the keyboard. */
async function select(page: Page, p: number, from: number, to: number) {
  await inPage(
    page,
    (w, a) => {
      const ed = w.fe.editor as unknown as { select(s: unknown): void; focus(): void; emitSelection(): void };
      ed.focus();
      ed.select({ anchor: { p: a.p, o: a.from }, focus: { p: a.p, o: a.to } });
      ed.emitSelection();
    },
    { p, from, to },
  );
}

/** Select the first "apple" and then all the matching text, from the menu. */
async function selectAllApples(page: Page): Promise<Place[]> {
  const found = await places(page, "apple");
  await select(page, found[0].p, found[0].from, found[0].to);
  await chooseMenu(page, "Edit", "Select all matching text");
  await expect(rects(page)).toHaveCount(found.length - 1);
  return found;
}

/** The boxes of the places as the browser lays the text out now, in screen pixels. */
const textBoxes = (page: Page, found: Place[]) =>
  page.evaluate((list) => {
    const out: { left: number; top: number; width: number; height: number }[] = [];
    for (const f of list) {
      const el = document.querySelector(`.flow [data-p="${f.p}"]`)!;
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      let seen = 0;
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const len = (n as Text).data.length;
        if (f.from >= seen && f.to <= seen + len) {
          const r = document.createRange();
          r.setStart(n, f.from - seen);
          r.setEnd(n, f.to - seen);
          const b = r.getBoundingClientRect();
          out.push({ left: b.left, top: b.top, width: b.width, height: b.height });
          break;
        }
        seen += len;
      }
    }
    return out;
  }, found);

const layerBoxes = (page: Page) =>
  rects(page).evaluateAll((els) =>
    els.map((e) => {
      const b = e.getBoundingClientRect();
      return { left: b.left, top: b.top, width: b.width, height: b.height };
    }),
  );

/** Every drawn box sits on a place's text (the real selection is not drawn on the layer, so `skip` leaves its place out). */
async function expectOnText(page: Page, found: Place[], skip: number) {
  const want = (await textBoxes(page, found)).filter((_, i) => i !== skip);
  const got = await layerBoxes(page);
  expect(got).toHaveLength(want.length);
  for (const [i, w] of want.entries()) {
    expect(Math.abs(got[i].left - w.left)).toBeLessThan(1.5);
    expect(Math.abs(got[i].top - w.top)).toBeLessThan(1.5);
    expect(Math.abs(got[i].width - w.width)).toBeLessThan(1.5);
    expect(Math.abs(got[i].height - w.height)).toBeLessThan(1.5);
  }
}

test.describe("select all matching text", () => {
  test.beforeEach(async ({ page }) => {
    await open(page);
  });

  test("selects every place the text occurs, case-sensitively and inside longer words, and says how many", async ({ page }) => {
    const found = await places(page, "apple");
    expect(found).toHaveLength(5);
    await select(page, found[0].p, found[0].from, found[0].to);
    await chooseMenu(page, "Edit", "Select all matching text");
    await expect(page.locator(".toast")).toHaveText("5 matches selected");
    // The real selection stays on the first one; the other four are drawn on the layer, in the selection's own colour.
    await expect(rects(page)).toHaveCount(4);
    expect(await page.evaluate(() => window.getSelection()!.toString())).toBe("apple");
    expect(await rects(page).first().evaluate((e) => getComputedStyle(e).backgroundColor)).toBe(await page.evaluate(() => {
      const probe = document.createElement("div");
      probe.style.background = "var(--selection)";
      document.querySelector(".docxtor")!.append(probe);
      const c = getComputedStyle(probe).backgroundColor;
      probe.remove();
      return c;
    }));
    await expectOnText(page, found, 0);
    // "Apple" is another text, and the selection reports all five ranges in order.
    await probe(page);
    const all = await api(page, (doc) => doc.selections().map((s) => ({ p: s.from.p, from: s.from.o, to: s.to.o, text: s.text })));
    expect(all).toEqual(found.map((f) => ({ ...f, text: "apple" })));
    // The extension API's `selection()` is still just the real one.
    expect(await api(page, (doc) => doc.selection()!.from.o)).toBe(found[0].from);
  });

  test("is in the right-click menu when text is selected, and the shortcut does the same", async ({ page, browserName }) => {
    const found = await places(page, "apple");
    // Nothing selected: not offered. (Safari on a Mac selects the word under a right-click, the way the Mac's own text does, so there is a selection to offer it for.)
    if (browserName !== "webkit") {
      await page.locator(`.flow [data-p="${found[0].p}"]`).click({ button: "right", position: { x: 4, y: 8 } });
      await expect(contextMenu(page)).toBeVisible();
      await expect(contextItem(page, "Select all matching text")).toHaveCount(0);
      await page.keyboard.press("Escape");
    }

    await select(page, found[0].p, found[0].from, found[0].to);
    const { x, y } = await page.evaluate(() => {
      const r = window.getSelection()!.getRangeAt(0).getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    });
    await page.mouse.click(x, y, { button: "right" });
    await expect(contextItem(page, "Select all matching text")).toBeVisible();
    await contextItem(page, "Select all matching text").click();
    await expect(rects(page)).toHaveCount(4);

    // Escape ends it; the shortcut brings it back.
    await page.keyboard.press("Escape");
    await expect(rects(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Alt+A`);
    await expect(rects(page)).toHaveCount(4);
  });

  test("a selection that spans paragraphs or is only a caret has nothing to match", async ({ page }) => {
    const [a, b] = await ids(page);
    await openMenu(page, "Edit");
    await expect(menuItem(page, "Select all matching text")).toBeDisabled();
    await page.keyboard.press("Escape");
    await inPage(
      page,
      (w, s) => {
        const ed = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
        ed.focus();
        ed.select({ anchor: { p: s.a, o: 2 }, focus: { p: s.b, o: 5 } });
      },
      { a, b },
    );
    await openMenu(page, "Edit");
    await expect(menuItem(page, "Select all matching text")).toBeDisabled();
  });

  test("bold applies to every range in one step, and one undo takes it all back", async ({ page }) => {
    const found = await selectAllApples(page);
    await page.keyboard.press(`${MOD}+B`);
    for (const f of found) expect((await runOn(page, f.p, f.from + 1))?.bold).toBe(true);
    // Text outside the matches is untouched, and the ranges are still selected.
    expect((await runOn(page, found[0].p, 1))?.bold).toBeFalsy();
    await expect(rects(page)).toHaveCount(4);

    await page.keyboard.press(`${MOD}+Z`);
    for (const f of found) expect((await runOn(page, f.p, f.from + 1))?.bold).toBeFalsy();
  });

  test("underline, strikethrough, size, colour, highlight and clear formatting reach every range", async ({ page }) => {
    const found = await selectAllApples(page);
    await page.keyboard.press(`${MOD}+I`);
    await page.keyboard.press(`${MOD}+Shift+X`);
    await page.keyboard.press(`${MOD}+Shift+.`);
    for (const f of found) {
      const run = await runOn(page, f.p, f.from + 1);
      expect(run?.italic).toBe(true);
      expect(run?.strike).toBe(true);
    }
    await page.keyboard.press(`${MOD}+\\`);
    for (const f of found) {
      const run = await runOn(page, f.p, f.from + 1);
      expect(run?.italic).toBeFalsy();
      expect(run?.strike).toBeFalsy();
    }
    // A colour from the toolbar goes onto all of them too.
    await page.getByRole("button", { name: "Text color" }).click();
    await page.locator(".palette .swatch").nth(5).click();
    const colours = await Promise.all(found.map(async (f) => (await runOn(page, f.p, f.from + 1)) as { color?: string }));
    expect(new Set(colours.map((c) => c.color)).size).toBe(1);
    expect(colours[0].color).toBeTruthy();
  });

  test("paragraph commands reach every paragraph a range is in", async ({ page }) => {
    const found = await selectAllApples(page);
    const [p0, p1, p2, p3] = await ids(page);
    await page.keyboard.press(`${MOD}+Alt+1`);
    const style = (p: number) => inPage(page, (w, id) => w.fe.editor.engine!.formatAt({ p: id, o: 0 })?.style, p);
    expect(await style(p0)).toMatch(/Heading ?1/);
    expect(await style(p1)).toMatch(/Heading ?1/);
    // The paragraphs with no match are left alone.
    expect((await style(p2)) ?? "").not.toMatch(/Heading/);
    expect((await style(p3)) ?? "").not.toMatch(/Heading/);

    await page.keyboard.press(`${MOD}+Shift+E`);
    const align = (p: number) => inPage(page, (w, id) => w.fe.editor.engine!.formatAt({ p: id, o: 0 })?.para.align, p);
    expect(await align(p0)).toBe("center");
    expect(await align(p1)).toBe("center");
    expect(await align(p2)).toBeFalsy();
    expect(found).toHaveLength(5);

    await page.keyboard.press(`${MOD}+Shift+8`);
    for (const p of [p0, p1]) expect(await inPage(page, (w, id) => w.fe.editor.engine!.formatAt({ p: id, o: 0 })?.list, p)).toBe("bullet");
    expect(await inPage(page, (w, id) => w.fe.editor.engine!.formatAt({ p: id, o: 0 })?.list, p2)).toBeFalsy();
    // One undo for the last command only.
    await page.keyboard.press(`${MOD}+Z`);
    expect(await inPage(page, (w, id) => w.fe.editor.engine!.formatAt({ p: id, o: 0 })?.list, p0)).toBeFalsy();
    expect(await align(p0)).toBe("center");
  });

  test("typing replaces every range, goes on at all of them, and one undo restores the text", async ({ page }) => {
    const before = await text(page);
    const found = await selectAllApples(page);
    await page.keyboard.type("pear");
    const after = await text(page);
    expect(after).toBe(before.replace(/apple/g, "pear"));
    // Each range carries on as a caret.
    await expect(carets(page)).toHaveCount(found.length - 1);
    await expect(rects(page)).toHaveCount(0);
    // The caret that follows the typing is the real one: it is after the first "pear".
    expect(await inPage(page, (w) => w.fe.editor.sel)).toEqual({ anchor: { p: found[0].p, o: found[0].from + 4 }, focus: { p: found[0].p, o: found[0].from + 4 } });

    await page.keyboard.press(`${MOD}+Z`);
    expect(await text(page)).toBe(before);
  });

  test("Backspace deletes every range, then a character before each caret; one undo brings the text back", async ({ page }) => {
    const before = await text(page);
    await selectAllApples(page);
    await page.keyboard.press("Backspace");
    expect(await text(page)).toBe(before.replace(/apple/g, ""));
    await page.keyboard.press(`${MOD}+Z`);
    expect(await text(page)).toBe(before);

    await selectAllApples(page);
    await page.keyboard.press("Delete");
    expect(await text(page)).toBe(before.replace(/apple/g, ""));
    await expect(carets(page)).toHaveCount(4);
    // The carets are still there: typing puts text at all of them.
    await page.keyboard.type("fig");
    expect(await text(page)).toBe(before.replace(/apple/g, "fig"));
  });

  test("a click, an arrow key or Escape ends the extra ranges", async ({ page }) => {
    await selectAllApples(page);
    await page.locator(".flow .fp").nth(2).click({ position: { x: 20, y: 8 } });
    await expect(rects(page)).toHaveCount(0);

    await selectAllApples(page);
    await page.keyboard.press("ArrowRight");
    await expect(rects(page)).toHaveCount(0);

    await selectAllApples(page);
    await page.keyboard.press("Escape");
    await expect(rects(page)).toHaveCount(0);
    // Typing after that only changes the one place.
    const before = await text(page);
    await select(page, (await ids(page))[0], 4, 9);
    await page.keyboard.type("x");
    expect(await text(page)).toBe(before.replace("apple", "x"));
  });

  test("copy puts the ranges' text on separate lines; cut and paste work on all of them", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    const before = await text(page);
    await selectAllApples(page);
    await page.keyboard.press(`${MOD}+C`);
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("apple\napple\napple\napple\napple");
    expect(await text(page)).toBe(before);

    await page.keyboard.press(`${MOD}+X`);
    expect(await text(page)).toBe(before.replace(/apple/g, ""));
    await page.keyboard.press(`${MOD}+Z`);
    expect(await text(page)).toBe(before);

    await selectAllApples(page);
    await page.evaluate(() => navigator.clipboard.writeText("kiwi"));
    await page.keyboard.press(`${MOD}+V`);
    expect(await text(page)).toBe(before.replace(/apple/g, "kiwi"));
    await page.keyboard.press(`${MOD}+Z`);
    expect(await text(page)).toBe(before);
  });

  test("the toolbar shows the real selection's formatting", async ({ page }) => {
    const found = await places(page, "apple");
    // Make the second one bold alone, then select all: the first (real) one is not bold, so the Bold key is off.
    await select(page, found[1].p, found[1].from, found[1].to);
    await page.keyboard.press(`${MOD}+B`);
    await select(page, found[0].p, found[0].from, found[0].to);
    await chooseMenu(page, "Edit", "Select all matching text");
    await expect(page.locator('.toolbar button[aria-label="Bold"]')).not.toHaveClass(/\bon\b/);
    expect(await inPage(page, (w) => w.fe.editor.sel)).toEqual({ anchor: { p: found[0].p, o: found[0].from }, focus: { p: found[0].p, o: found[0].to } });
  });

  test("right-click on another selected range keeps all of them", async ({ page }) => {
    const found = await selectAllApples(page);
    await page.keyboard.press(`${MOD}+B`);
    const last = rects(page).last();
    const box = (await last.boundingBox())!;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Select all matching text")).toBeVisible();
    await contextItem(page, "Clear formatting").click();
    for (const f of found) expect((await runOn(page, f.p, f.from + 1))?.bold).toBeFalsy();
  });

  test("the find bar's Select all selects every match", async ({ page }) => {
    await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
    await page.keyboard.press(`${MOD}+F`);
    const bar = page.getByRole("search");
    await bar.getByRole("searchbox", { name: "Find" }).fill("apple");
    // The find bar ignores case, so "Apple" counts too.
    await expect(bar.locator(".find-count")).toHaveText("1 of 6");
    await bar.getByRole("button", { name: "Select all" }).click();
    await expect(page.locator(".toast")).toHaveText("6 matches selected");
    await expect(bar).toBeHidden();
    await expect(rects(page)).toHaveCount(5);
    const before = await text(page);
    await page.keyboard.type("fig");
    expect(await text(page)).toBe(before.replace(/apple/gi, "fig"));
  });

  test("the find bar's Select all is off when nothing matches", async ({ page }) => {
    await page.keyboard.press(`${MOD}+F`);
    const bar = page.getByRole("search");
    await bar.getByRole("searchbox", { name: "Find" }).fill("zebra");
    await expect(bar.locator(".find-count")).toHaveText("No results");
    await expect(bar.getByRole("button", { name: "Select all" })).toBeDisabled();
  });

  test("the extra ranges stay on their text after the pages are drawn again", async ({ page }) => {
    const found = await selectAllApples(page);
    await expectOnText(page, found, 0);
    // Zoom and a narrower window both lay the pages out again.
    // The number of ranges is the same before and after the pages are laid out again, so wait for their places.
    const settled = () =>
      expect
        .poll(async () => {
          const want = (await textBoxes(page, found)).slice(1);
          const got = await layerBoxes(page);
          return got.length === want.length && want.every((w, i) => Math.abs(got[i].left - w.left) < 1.5 && Math.abs(got[i].top - w.top) < 1.5);
        })
        .toBe(true);
    await inPage(page, (w) => (w.fe.editor as unknown as { setZoom(z: number): void }).setZoom(1.4));
    await settled();
    await expectOnText(page, found, 0);
    await page.setViewportSize({ width: 700, height: 800 });
    await settled();
  });

  test("the ranges follow the text through a long document, onto later pages", async ({ page }) => {
    const filler = Array.from({ length: 60 }, (_, i) => `Line ${i} about the apple harvest and a basket of plums.`);
    await openBuilt(page, { sections: [{ paragraphs: ["Start with an apple.", ...filler, "The last apple."] }] });
    const found = await places(page, "apple");
    expect(found).toHaveLength(62);
    await select(page, found[0].p, found[0].from, found[0].to);
    await chooseMenu(page, "Edit", "Select all matching text");
    await expect(page.locator(".toast")).toHaveText("62 matches selected");
    await expect(rects(page)).toHaveCount(61);
    await page.keyboard.press(`${MOD}+B`);
    for (const f of [found[1], found[40], found[61]]) expect((await runOn(page, f.p, f.from + 1))?.bold).toBe(true);
    await inPage(page, (w) => (w.fe.editor as unknown as { canvas: HTMLElement }).canvas.scrollTo(0, 10000));
    await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));
    const last = (await textBoxes(page, [found[61]]))[0];
    const drawn = await layerBoxes(page);
    expect(drawn.some((b) => Math.abs(b.left - last.left) < 1.5 && Math.abs(b.top - last.top) < 1.5)).toBe(true);
  });

  test("extensions can read and set several selections", async ({ page }) => {
    const found = await places(page, "apple");
    await probe(page);
    const got = await api(
      page,
      (doc, f) => {
        const at = (x: { p: number; from: number; to: number }) => ({ from: { p: x.p, o: x.from }, to: { p: x.p, o: x.to } });
        doc.selectRanges([at(f[2]), at(f[0]), at(f[4])]);
        return { many: doc.selections().map((s) => `${s.from.p}:${s.from.o}`), primary: doc.selection()!.text };
      },
      found,
    );
    // Document order, with the first range given as the real selection.
    expect(got.many).toEqual([found[0], found[2], found[4]].map((f) => `${f.p}:${f.from}`));
    expect(got.primary).toBe("apple");
    await expect(rects(page)).toHaveCount(2);
    expect(await inPage(page, (w) => w.fe.editor.sel)).toEqual({ anchor: { p: found[2].p, o: found[2].from }, focus: { p: found[2].p, o: found[2].to } });
    // Selecting a single range ends the multiple selection.
    await inPage(page, (w, f) => (w.fe.editor as unknown as { select(s: unknown): void }).select({ anchor: { p: f.p, o: 0 }, focus: { p: f.p, o: 3 } }), found[0]);
    await expect(rects(page)).toHaveCount(0);
  });
});
