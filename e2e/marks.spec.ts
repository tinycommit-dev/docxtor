// Show non-printing characters (¶, dots for spaces, tabs, line breaks, page, column and section breaks) and
// Insert, Special characters and Emoji.
import { openBuilt } from "./support/docx-builder";
import { CHROMIUM_ONLY, caretAtEnd, chooseMenu, dialog, documentReady, expect, inPage, menuItem, openFixture, openMenu, paraText, reveal, RICH, selectWord, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** Turn the marks on from the View menu and wait until the first ones are drawn. */
async function marksOn(page: Page) {
  await chooseMenu(page, "View", "Show non-printing characters");
  await expect(page.locator(".np-layer")).toBeVisible();
  await expect(page.locator(".np-para").first()).toBeVisible();
}

const layer = (page: Page) => page.locator(".np-layer");
/** Two animation frames: the marks are drawn in the frame after a change. */
const settle = (page: Page) => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));

/** Where each paragraph on screen is, and how many pages there are: the layout, to compare with the marks on and off. */
const layoutOf = (page: Page) =>
  page.evaluate(() => {
    const ed = (window as unknown as { docxtor: { fe: { editor: { pages: number; pageMap(): Record<string, number>; withDrawn<T>(fn: () => T): T } } } }).docxtor.fe.editor;
    // Every rectangle is read on purpose: with far pages left out the pages are drawn first.
    return ed.withDrawn(() => {
      const flow = document.querySelector(".flow")!.getBoundingClientRect();
      const boxes = Array.from(document.querySelectorAll<HTMLElement>(".flow .fp, .flow table"), (el) => {
        const r = el.getBoundingClientRect();
        return [Math.round((r.top - flow.top) * 100) / 100, Math.round(r.height * 100) / 100, Math.round((r.left - flow.left) * 100) / 100, Math.round(r.width * 100) / 100];
      });
      return { pages: ed.pages, sheets: document.querySelectorAll(".sheet").length, map: ed.pageMap(), boxes };
    });
  });

test.describe("Show non-printing characters", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("View, Show non-printing characters turns the marks on and off, and is remembered", async ({ page }) => {
    await expect(layer(page)).toBeHidden();
    await openMenu(page, "View");
    await expect(menuItem(page, "Show non-printing characters")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Show non-printing characters").click();
    await expect(layer(page)).toBeVisible();
    await expect(page.locator(".canvas")).toHaveClass(/show-marks/);
    await openMenu(page, "View");
    await expect(menuItem(page, "Show non-printing characters")).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");

    // Still on in the next visit to the app.
    await page.reload();
    await documentReady(page);
    await expect(layer(page)).toBeVisible();
    await expect(page.locator(".np-para").first()).toBeVisible();

    await chooseMenu(page, "View", "Show non-printing characters");
    await expect(layer(page)).toBeHidden();
    await expect(page.locator(".np-m")).toHaveCount(0);
    await page.reload();
    await documentReady(page);
    await expect(layer(page)).toBeHidden();
    await expect(page.locator(".canvas")).not.toHaveClass(/show-marks/);
  });

  test("Ctrl/Cmd+Alt+P toggles the marks", async ({ page }) => {
    await page.keyboard.press(`${MOD}+Alt+P`);
    await expect(layer(page)).toBeVisible();
    await page.keyboard.press(`${MOD}+Alt+P`);
    await expect(layer(page)).toBeHidden();
  });

  test("a ¶ sits right after the last character of paragraphs in the body, in a table and in the header", async ({ page }) => {
    await marksOn(page);
    // For a paragraph, the marks (¶) that sit on its first line and where its last character ends.
    const check = (selector: string) =>
      page.evaluate((sel) => {
        const fp = document.querySelector<HTMLElement>(sel)!;
        const walker = document.createTreeWalker(fp, NodeFilter.SHOW_TEXT);
        let last: Text | null = null;
        for (let n = walker.nextNode(); n; n = walker.nextNode()) if ((n as Text).data && !(n.parentElement as HTMLElement).closest(".lbl")) last = n as Text;
        const range = document.createRange();
        range.setStart(last!, last!.data.length - 1);
        range.setEnd(last!, last!.data.length);
        const end = range.getBoundingClientRect();
        const marks = Array.from(document.querySelectorAll<HTMLElement>(".np-para")).filter((m) => {
          const r = m.getBoundingClientRect();
          return r.left >= end.right - 1 && r.left <= end.right + 8 && r.top < end.bottom && r.bottom > end.top;
        });
        return { count: marks.length, text: marks[0]?.textContent };
      }, selector);
    expect(await check(`.flow [data-p="${RICH.title}"]`)).toEqual({ count: 1, text: "¶" });
    expect(await check(`.flow [data-p="${RICH.intro}"]`)).toEqual({ count: 1, text: "¶" });
    expect(await check(`.flow [data-p="${RICH.cell}"]`)).toEqual({ count: 1, text: "¶" });
    expect(await check(".sheet .hdr .fp")).toEqual({ count: 1, text: "¶" });
  });

  test("an empty paragraph shows a ¶ too", async ({ page }) => {
    await caretAtEnd(page, RICH.title);
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await marksOn(page);
    const id = await inPage(page, (w) => w.fe.editor.sel!.focus.p);
    const hit = await page.evaluate((p) => {
      const fp = document.querySelector(`.flow [data-p="${p}"]`)!.getBoundingClientRect();
      return Array.from(document.querySelectorAll<HTMLElement>(".np-para")).some((m) => {
        const r = m.getBoundingClientRect();
        return r.top >= fp.top - 1 && r.bottom <= fp.bottom + 1 && r.left >= fp.left;
      });
    }, id);
    expect(hit).toBe(true);
  });

  test("a dot sits in the middle of each space on the visible page, and follows the zoom", async ({ page }) => {
    await marksOn(page);
    const align = () =>
      page.evaluate((id) => {
        const stage = document.querySelector<HTMLElement>(".stage")!;
        const sr = stage.getBoundingClientRect();
        const sc = sr.width / stage.offsetWidth;
        const svg = document.querySelector<SVGElement>(".np-dots")!;
        const top = parseFloat(svg.style.top) || 0;
        const dots = Array.from(svg.querySelectorAll("path")).flatMap((p) => Array.from(p.getAttribute("d")!.matchAll(/M([\d.]+) ([\d.]+)h0/g), (m) => [Number(m[1]), Number(m[2]) + top]));
        const fp = document.querySelector(`.flow [data-p="${id}"]`)!;
        const walker = document.createTreeWalker(fp, NodeFilter.SHOW_TEXT);
        const found: boolean[] = [];
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          const text = n as Text;
          if ((text.parentElement as HTMLElement).closest(".lbl")) continue;
          for (let i = text.data.indexOf(" "); i >= 0; i = text.data.indexOf(" ", i + 1)) {
            const range = document.createRange();
            range.setStart(text, i);
            range.setEnd(text, i + 1);
            const r = range.getBoundingClientRect();
            const x = (r.left + r.width / 2 - sr.left) / sc;
            const y0 = (r.top - sr.top) / sc;
            const y1 = (r.bottom - sr.top) / sc;
            found.push(dots.some(([dx, dy]) => Math.abs(dx - x) < 1.5 && dy > y0 && dy < y1));
          }
        }
        return { total: dots.length, found, n: svg.dataset.n };
      }, RICH.intro);
    let r = await align();
    expect(r.total).toBeGreaterThan(30);
    expect(r.n).toBe(String(r.total));
    expect(r.found.length).toBeGreaterThan(10);
    expect(r.found.every(Boolean)).toBe(true);

    await inPage(page, (w) => w.fe.editor.setZoom(1.5));
    await expect.poll(async () => (await align()).found.every(Boolean)).toBe(true);
    r = await align();
    expect(r.found.length).toBeGreaterThan(10);
  });

  test("typing moves the ¶ and draws a dot for a new space", async ({ page }) => {
    await marksOn(page);
    await caretAtEnd(page, RICH.last);
    const mark = async () =>
      page.evaluate((id) => {
        const fp = document.querySelector(`.flow [data-p="${id}"]`)!.getBoundingClientRect();
        const m = Array.from(document.querySelectorAll<HTMLElement>(".np-para")).find((el) => {
          const r = el.getBoundingClientRect();
          return r.top >= fp.top - 2 && r.bottom <= fp.bottom + 2;
        })!;
        return { left: m.getBoundingClientRect().left, dots: Number(document.querySelector<SVGElement>(".np-dots")!.dataset.n) };
      }, RICH.last);
    const before = await mark();
    await page.keyboard.type(" more");
    await expect.poll(async () => (await mark()).left).toBeGreaterThan(before.left + 20);
    await expect.poll(async () => (await mark()).dots).toBe(before.dots + 1);
  });

  test("the marks are not selectable, not clickable and not text", async ({ page }) => {
    await marksOn(page);
    const css = await layer(page).evaluate((el) => ({ events: getComputedStyle(el).pointerEvents, select: getComputedStyle(el).userSelect, hidden: el.getAttribute("aria-hidden"), inFlow: !!el.closest(".flow") }));
    expect(css).toEqual({ events: "none", select: "none", hidden: "true", inFlow: false });
    await page.keyboard.press(`${MOD}+A`);
    const selected = await page.evaluate(() => window.getSelection()!.toString());
    expect(selected).not.toMatch(/[¶·↵]/);
    // The fixture has one → of its own; the tab marks add none.
    expect(selected.match(/→/g)).toHaveLength(1);
    expect(await page.evaluate(() => document.querySelector(".flow")!.querySelectorAll(".np-m, .np-dots, .np-brk").length)).toBe(0);
  });

  test("Copy leaves the marks out", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await marksOn(page);
    await selectWord(page, RICH.intro, "report");
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press(`${MOD}+C`);
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    expect(copied).toContain("Quarterly Product Report");
    expect(copied).not.toMatch(/[¶·↵]/);
    expect(copied.match(/→/g)).toHaveLength(1);
  });

  test("turning the marks on or off does not move anything", async ({ page }) => {
    const off = await layoutOf(page);
    await marksOn(page);
    await settle(page);
    const on = await layoutOf(page);
    expect(on).toEqual(off);
    await chooseMenu(page, "View", "Show non-printing characters");
    await settle(page);
    expect(await layoutOf(page)).toEqual(off);
  });

  test("no pagination change on a long document either", async ({ page }) => {
    await openFixture(page, "long.docx");
    const off = await layoutOf(page);
    expect(off.pages).toBeGreaterThan(5);
    await marksOn(page);
    await settle(page);
    expect(await layoutOf(page)).toEqual(off);
    // Scrolling through the pages draws marks on the way and still moves nothing.
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight / 2));
    await settle(page);
    await expect(page.locator(".np-para").first()).toBeVisible();
    expect(await layoutOf(page)).toEqual(off);
  });

  test("only the part in view is drawn, and scrolling draws the rest", async ({ page }) => {
    await openFixture(page, "long.docx");
    await marksOn(page);
    const marks = () => page.locator(".np-para").count();
    const first = await marks();
    const total = await page.locator(".flow > .fp").count();
    expect(first).toBeGreaterThan(0);
    expect(first).toBeLessThan(total);
    const topOf = () => page.evaluate(() => Number(document.querySelector<SVGElement>(".np-dots")?.style.top.replace("px", "")));
    const before = await topOf();
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight - c.clientHeight));
    await expect.poll(topOf).toBeGreaterThan(before + 1000);
  });

  test("nothing of the marks is in the print copy", async ({ page }) => {
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await marksOn(page);
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .print-page")).toHaveCount(2);
    expect(await page.locator(".docxtor-print .np-layer, .docxtor-print .np-m, .docxtor-print .np-dots, .docxtor-print .np-brk").count()).toBe(0);
    const printed = (await page.locator(".docxtor-print").textContent())!;
    expect(printed).not.toMatch(/[¶·↵]/);
    expect(printed.match(/→/g)).toHaveLength(2);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });
});

test.describe("marks for tabs, line breaks and breaks", () => {
  test("a tab shows an arrow and a line break shows a bent arrow, where they are", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Intro", "Alpha{TAB}beta{LBR}gamma delta"] }] });
    await marksOn(page);
    await expect(page.locator(".np-tab")).toHaveCount(1);
    await expect(page.locator(".np-br")).toHaveCount(1);
    const where = await page.evaluate(() => {
      const fp = document.querySelectorAll(".flow .fp")[1]!;
      const text = Array.from(fp.querySelectorAll(".s"));
      const tab = text.find((s) => s.textContent === "\t")!.getBoundingClientRect();
      const br = text.find((s) => s.textContent === "\n")!.getBoundingClientRect();
      const t = document.querySelector(".np-tab")!.getBoundingClientRect();
      const b = document.querySelector(".np-br")!.getBoundingClientRect();
      return { tab: [t.left - tab.left, t.top - tab.top], br: [b.left - br.left, b.top - br.top], tabText: document.querySelector(".np-tab")!.textContent, brText: document.querySelector(".np-br")!.textContent };
    });
    expect(where.tabText).toBe("→");
    expect(where.brText).toBe("↵");
    expect(Math.abs(where.tab[0])).toBeLessThan(4);
    expect(Math.abs(where.tab[1])).toBeLessThan(4);
    expect(Math.abs(where.br[0])).toBeLessThan(4);
    expect(Math.abs(where.br[1])).toBeLessThan(4);
  });

  test("page and column breaks in the text are drawn as dotted rules with their name", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before{PBR}after the page break", "Next{CBR}after the column break"] }] });
    await marksOn(page);
    const rules = page.locator(".np-brk");
    await expect(page.locator('.np-brk[data-kind="page"]')).toHaveText("Page break");
    await expect.poll(() => page.locator(".np-brk").count()).toBeGreaterThanOrEqual(1);
    // The second page holds the column break (one column: it starts a new page).
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight));
    await expect(page.locator('.np-brk[data-kind="column"]')).toHaveText("Column break");
    await expect(rules.filter({ hasText: "Column break" }).locator("i").first()).toBeVisible();
    const kinds = await page.locator(".np-brk").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.kind));
    expect(kinds).toContain("column");
  });

  test("Insert, Break, Page break draws a page break after the paragraph above", async ({ page }) => {
    await openFixture(page);
    await caretAtEnd(page, RICH.intro);
    await chooseMenu(page, "Insert", "Break", "Page break");
    await marksOn(page);
    await expect(page.locator('.np-brk[data-kind="page"]').first()).toHaveText("Page break");
  });

  test("section breaks are drawn after the paragraph mark with the kind of the next section", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Section one"] },
        { start: "nextPage", paragraphs: ["Section two"] },
        { start: "continuous", paragraphs: ["Section three"] },
        { start: "evenPage", paragraphs: ["Section four"] },
        { start: "oddPage", paragraphs: ["Section five"] },
      ],
    });
    await marksOn(page);
    const labels: Record<string, string> = {
      nextPage: "Section break (next page)",
      continuous: "Section break (continuous)",
      evenPage: "Section break (even page)",
      oddPage: "Section break (odd page)",
    };
    const kinds = await page.locator(".flow .fp[data-sb]").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.sb));
    // Every kind marks its paragraph, the continuous one too.
    expect(kinds).toEqual(["nextPage", "continuous", "evenPage", "oddPage"]);
    for (const [i, kind] of kinds.entries()) {
      const fp = page.locator(".flow .fp[data-sb]").nth(i);
      await reveal(fp);
      await fp.scrollIntoViewIfNeeded();
      const rule = page.locator(`.np-sect[data-kind="${kind}"]`);
      await expect(rule).toHaveCount(1);
      await expect(rule).toHaveText(labels[kind!]);
      // On the paragraph's line, after its mark.
      const fr = (await fp.boundingBox())!;
      const rr = (await rule.boundingBox())!;
      expect(rr.y).toBeGreaterThanOrEqual(fr.y - 2);
      expect(rr.y).toBeLessThan(fr.y + fr.height);
      const mark = (await fp.evaluate((el) => {
        const m = Array.from(document.querySelectorAll<HTMLElement>(".np-para[data-sb]"));
        const r = el.getBoundingClientRect();
        const hit = m.find((x) => x.getBoundingClientRect().top >= r.top - 2 && x.getBoundingClientRect().bottom <= r.bottom + 2);
        return hit ? hit.getBoundingClientRect().right : null;
      }))!;
      expect(rr.x).toBeGreaterThanOrEqual(mark);
    }
  });

  test("section breaks are drawn on a fixture with a section break inserted from the menu", async ({ page }) => {
    await openFixture(page);
    await caretAtEnd(page, RICH.title);
    await chooseMenu(page, "Insert", "Break", "Section break (continuous)");
    await marksOn(page);
    await expect(page.locator('.np-sect[data-kind="continuous"]')).toHaveText("Section break (continuous)");
  });

  test("the marks are one colour in both themes, dark enough on the white page", async ({ page }) => {
    await openFixture(page);
    await marksOn(page);
    const color = (theme: "light" | "dark") =>
      page.evaluate((t) => {
        document.querySelectorAll<HTMLElement>(".docxtor").forEach((el) => (el.dataset.theme = t));
        return getComputedStyle(document.querySelector(".np-para")!).color;
      }, theme);
    const light = await color("light");
    const dark = await color("dark");
    expect(dark).toBe(light);
    // #6b675d on white is above 4.5:1.
    expect(light).toBe("rgb(107, 103, 93)");
  });
});

/** The text of the document as the engine has it. */
const engineText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());
const picker = (page: Page) => dialog(page, "Special characters");
/** A character button in the grid (the recently used row has its own). */
const key = (page: Page, name: string) => picker(page).locator(".sc-results").getByRole("button", { name, exact: true });
/** Choose Insert, Special characters… (or Emoji…) and wait for the dialog: its characters load on demand. */
async function openPicker(page: Page, item = "Special characters…") {
  await chooseMenu(page, "Insert", item);
  await expect(picker(page)).toBeVisible();
  await expect(picker(page).locator(".sc-results .sc-key").first()).toBeVisible();
}
const insertedAt = async (page: Page, id: number) => paraText(page, id);

test.describe("Insert, Special characters", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("Insert, Special characters… opens the dialog with the search box focused", async ({ page }) => {
    await openPicker(page);
    await expect(picker(page)).toBeVisible();
    await expect(picker(page).getByPlaceholder("Search by name, like arrow or euro")).toBeFocused();
    // Common is the first tab, with the usual signs.
    await expect(picker(page).getByRole("tab", { name: "Common" })).toHaveAttribute("aria-selected", "true");
    for (const name of ["copyright sign", "euro sign", "em dash", "degree sign", "multiplication sign"]) await expect(key(page, name)).toBeVisible();
    for (const tab of ["Common", "Arrows", "Math", "Greek", "Currency", "Latin", "Punctuation", "Symbols", "Emoji"]) await expect(picker(page).getByRole("tab", { name: tab, exact: true })).toBeVisible();
  });

  test("searching 'euro' finds the euro sign, and a click inserts it at the caret", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    await picker(page).getByPlaceholder("Search by name, like arrow or euro").fill("euro");
    await expect(key(page, "euro sign")).toBeVisible();
    await expect(key(page, "euro sign")).toHaveText("€");
    await key(page, "euro sign").click();
    // It went into the document's text, and the dialog is still open.
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.€");
    expect(await engineText(page)).toContain("page two.€");
    await expect(picker(page)).toBeVisible();
    // The preview names the character in focus.
    await expect(picker(page).locator(".sc-name")).toHaveText("euro sign");
    await expect(picker(page).locator(".sc-code")).toHaveText("U+20AC");
  });

  test("several characters can be inserted without closing, each as its own undo step", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    await key(page, "copyright sign").click();
    await key(page, "euro sign").click();
    await key(page, "em dash").click();
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.©€—");
    await expect(picker(page)).toBeVisible();
    await picker(page).getByRole("button", { name: "Done" }).click();
    await expect(picker(page)).toBeHidden();
    // The caret is after what went in: what is typed next follows it.
    await page.keyboard.type("!");
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.©€—!");
    // One undo step per character (the typed "!" is one more).
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.©€—");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.©€");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.©");
  });

  test("a character replaces the selection", async ({ page }) => {
    await selectWord(page, RICH.last, "paragraph");
    await openPicker(page);
    await key(page, "section sign").click();
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final § on page two.");
  });

  test("Esc closes the dialog and puts the caret after what was inserted", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    await key(page, "degree sign").click();
    await page.keyboard.press("Escape");
    await expect(picker(page)).toBeHidden();
    await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
    await page.keyboard.type("C");
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.°C");
  });

  test("the recently used row lists the last characters, newest first, and is there after reopening", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    await expect(picker(page).locator(".sc-recent")).toContainText("Recently used");
    await key(page, "copyright sign").click();
    await key(page, "euro sign").click();
    await key(page, "copyright sign").click();
    const recent = () => picker(page).locator(".sc-recent .sc-key").evaluateAll((els) => els.map((e) => e.getAttribute("aria-label")));
    expect(await recent()).toEqual(["copyright sign", "euro sign"]);
    await picker(page).getByRole("button", { name: "Done" }).click();

    await openPicker(page);
    expect(await recent()).toEqual(["copyright sign", "euro sign"]);
    // A click on a recent one inserts it.
    await picker(page).locator(".sc-recent").getByRole("button", { name: "euro sign" }).click();
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.©€©€");
    expect(await recent()).toEqual(["euro sign", "copyright sign"]);
    await page.keyboard.press("Escape");

    // And after a reload: the last 16 are kept.
    await page.reload();
    await documentReady(page);
    await openPicker(page);
    expect(await recent()).toEqual(["euro sign", "copyright sign"]);
  });

  test("at most 16 recent characters are kept", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    await picker(page).getByRole("tab", { name: "Greek" }).click();
    const keys = picker(page).locator(".sc-results .sc-key");
    for (let i = 0; i < 20; i++) await keys.nth(i).click();
    await expect(picker(page).locator(".sc-recent .sc-key")).toHaveCount(16);
  });

  test("the keyboard moves through the characters and Enter inserts the one in focus", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    const search = picker(page).getByPlaceholder("Search by name, like arrow or euro");
    await expect(search).toBeFocused();
    // Down from the search box goes to the first character; the arrows move along and down the grid.
    await page.keyboard.press("ArrowDown");
    await expect(key(page, "copyright sign")).toBeFocused();
    await expect(picker(page).locator(".sc-name")).toHaveText("copyright sign");
    await page.keyboard.press("ArrowRight");
    await expect(key(page, "registered sign")).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(key(page, "trade mark sign")).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.™");
    // The focus stays in the dialog, on the same key.
    await expect(key(page, "trade mark sign").first()).toBeFocused();
    const x = (await key(page, "trade mark sign").first().boundingBox())!.x;
    await page.keyboard.press("ArrowDown");
    const next = await picker(page).locator(".sc-key:focus").boundingBox();
    expect(next!.y).toBeGreaterThan((await key(page, "trade mark sign").first().boundingBox())!.y);
    expect(Math.abs(next!.x - x)).toBeLessThan(40);
    await page.keyboard.press("ArrowUp");
    await expect(key(page, "trade mark sign").first()).toBeFocused();
    // Home and End go to the first and last character.
    await page.keyboard.press("End");
    await expect(picker(page).locator(".sc-name")).toHaveText("middle dot");
    await page.keyboard.press("Home");
    await expect(picker(page).locator(".sc-name")).toHaveText("trade mark sign");
    // Enter in the search box inserts the first match.
    await search.focus();
    await search.fill("pound");
    await page.keyboard.press("Enter");
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.™£");
  });

  test("search finds by name, by the character and by code point, and says when nothing matches", async ({ page }) => {
    await openPicker(page);
    const search = picker(page).getByPlaceholder("Search by name, like arrow or euro");
    await search.fill("arrow");
    await expect(key(page, "rightwards arrow").first()).toBeVisible();
    await search.fill("é");
    await expect(key(page, "Latin small letter e with acute")).toBeVisible();
    await search.fill("U+00A9");
    await expect(key(page, "copyright sign")).toBeVisible();
    await search.fill("zzzzqq");
    await expect(picker(page).getByRole("status")).toContainText("No characters match");
    await search.fill("");
    await expect(picker(page).getByRole("tab", { name: "Common" })).toHaveAttribute("aria-selected", "true");
  });

  test("the categories show their characters", async ({ page }) => {
    await openPicker(page);
    const expectIn = async (tab: string, names: string[]) => {
      await picker(page).getByRole("tab", { name: tab, exact: true }).click();
      await expect(picker(page).getByRole("tab", { name: tab, exact: true })).toHaveAttribute("aria-selected", "true");
      for (const n of names) await expect(key(page, n)).toHaveCount(1);
    };
    await expectIn("Arrows", ["rightwards arrow", "leftwards double arrow"]);
    await expectIn("Math", ["infinity", "square root", "not equal to"]);
    await expectIn("Greek", ["Greek small letter pi", "Greek capital letter omega"]);
    await expectIn("Currency", ["euro sign", "indian rupee sign", "yen sign"]);
    await expectIn("Latin", ["Latin small letter n with tilde", "Latin small letter sharp s"]);
    await expectIn("Punctuation", ["en dash", "left double quotation mark", "no-break space"]);
    await expectIn("Symbols", ["check mark", "black star", "ballot box with check"]);
  });

  test("Insert, Emoji… opens the same dialog on the Emoji tab and inserts an emoji", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page, "Emoji…");
    await expect(picker(page)).toBeVisible();
    await expect(picker(page).getByRole("tab", { name: "Emoji" })).toHaveAttribute("aria-selected", "true");
    for (const group of ["Smileys", "People", "Animals and nature", "Food", "Activities", "Travel", "Objects", "Symbols", "Flags"]) await expect(picker(page).getByRole("heading", { name: group, exact: true })).toHaveCount(1);
    expect(await picker(page).locator(".sc-results .sc-key").count()).toBeGreaterThan(300);
    await key(page, "grinning face").click();
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.😀");
    expect(await engineText(page)).toContain("page two.😀");
    await expect(picker(page).locator(".sc-code")).toHaveText("U+1F600");
    // Searching by name finds emoji too.
    await picker(page).getByPlaceholder("Search by name, like arrow or euro").fill("rocket");
    await key(page, "rocket").click();
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.😀🚀");
  });

  test("undo removes the last character inserted", async ({ page }) => {
    await caretAtEnd(page, RICH.last);
    await openPicker(page);
    await key(page, "euro sign").click();
    await key(page, "pound sign").click();
    await page.keyboard.press("Escape");
    await expect(picker(page)).toBeHidden();
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.€");
    expect(await engineText(page)).not.toContain("£");
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => insertedAt(page, RICH.last)).toBe("Final paragraph on page two.€£");
  });

  test("the commands are in the command palette", async ({ page }) => {
    await page.keyboard.press(`${MOD}+Shift+P`);
    await page.keyboard.type("special char");
    await expect(page.getByRole("option", { name: /Special characters/ })).toBeVisible();
    await page.keyboard.press("Escape");
    await page.keyboard.press(`${MOD}+Shift+P`);
    await page.keyboard.type("non-printing");
    await expect(page.getByRole("option", { name: /Show non-printing characters/ })).toBeVisible();
  });

  test("a document that is read-only cannot be inserted into", async ({ page }) => {
    await chooseMenu(page, "View", "Mode", "Viewing");
    await openMenu(page, "Insert");
    await expect(menuItem(page, "Special characters…")).toBeDisabled();
    await expect(menuItem(page, "Emoji…")).toBeDisabled();
  });

  test("the dialog is written in Spanish when the page is", async ({ page }) => {
    await openFixture(page, "rich.docx", "&lang=es");
    // The menu bar's own name is translated: find the top menus by class.
    await page.locator("nav.menubar .menubar-top:not(.menubar-fold)").nth(3).click();
    await page.locator('.menu-drop [role="menuitem"]', { hasText: "Caracteres especiales…" }).click();
    const d = dialog(page, "Caracteres especiales");
    await expect(d).toBeVisible();
    await expect(d.getByPlaceholder("Busca por nombre, como flecha o euro")).toBeFocused();
    await expect(d.getByRole("tab", { name: "Flechas" })).toBeVisible();
    await expect(d.getByRole("tab", { name: "Monedas" })).toBeVisible();
    // The names of the characters stay in English (they are data), the interface does not.
    await expect(d.getByRole("button", { name: "copyright sign" })).toBeVisible();
    await expect(d.getByText("Usados recientemente")).toBeVisible();
    await expect(d.getByRole("button", { name: "Listo" })).toBeVisible();
  });
});
