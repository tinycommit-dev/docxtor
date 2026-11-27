// Pages far from the view are out of Chrome's layout (far pages): `data-far` blocks are `display: none`, kept in one hidden box a run, and the
// block after a stretch of them keeps its height. These tests run on a 60-page document with the test switch on (one page either side of the
// view), where nearly every block is far, and check that everything a person does still reaches them: typing, scrolling, select all, jumps,
// the keys, drag-select, output, notes and fields, undo, and a split view. The paginator's own check (every incremental layout against a full
// one) and the geometry check (the drawn stretch against everything drawn) are on, as in every spec.
import { readFileSync } from "node:fs";
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { readPdf, textRuns } from "./support/pdf-reader";
import { chooseMenu, CHROMIUM_ONLY, expect, inPage, pasteData, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const PAGES = 60;
const PER_PAGE = 20;
const TOTAL = PAGES * PER_PAGE;

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as unknown as { __DOCXTOR_FAR_PAGES__: number }).__DOCXTOR_FAR_PAGES__ = 1;
  });
});

/** 60 pages of 20 short paragraphs, "P 1" to "P 1200", each page after the first starting with a page break before. */
function longDoc(each?: (i: number) => { ppr?: string; xml?: string }): SectionSpec {
  const paragraphs: SectionSpec["paragraphs"] = [];
  for (let i = 1; i <= TOTAL; i++) {
    const extra = each?.(i) ?? {};
    const ppr = `${i > 1 && i % PER_PAGE === 1 ? "<w:pageBreakBefore/>" : ""}${extra.ppr ?? ""}`;
    paragraphs.push(ppr || extra.xml ? { text: `P ${i}`, ppr, xml: extra.xml } : `P ${i}`);
  }
  return { paragraphs };
}

/** The top-level blocks with a layout box, and the ones left out. */
const counts = (page: Page) =>
  inPage(page, (w) => {
    const e = w.fe.editor as unknown as { flow: HTMLElement; paginator: { farBlocks: number } };
    // A run of far blocks is kept in one box: the blocks are counted, not the flow's children.
    const kids = (Array.from(e.flow.children) as HTMLElement[]).flatMap((k) => (k.classList.contains("far-box") ? (Array.from(k.children) as HTMLElement[]) : [k]));
    return { laidOut: kids.filter((k) => k.getClientRects().length > 0).length, far: e.paginator.farBlocks, total: kids.length };
  });

/** The paragraph's id, by its place in the text. */
const idAt = (page: Page, n: number) => inPage(page, (w, i) => Number((document.querySelectorAll<HTMLElement>(".flow [data-p]")[i] as HTMLElement).dataset.p), n);
const hasBox = (page: Page, p: number) => inPage(page, (w, id) => (document.querySelector<HTMLElement>(`.flow [data-p="${id}"]`)?.getClientRects().length ?? 0) > 0, p);
const caretP = (page: Page) => inPage(page, (w) => w.fe.editor.sel?.focus.p ?? -1);
const caretText = async (page: Page) => (await page.locator(`.flow [data-p="${await caretP(page)}"]`).textContent()) ?? "";

/** Wait for the drawn stretch to settle (the frame after a scroll or a jump). */
const settled = (page: Page) => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));

async function open(page: Page, spec: ReturnType<typeof longDoc> = longDoc(), extra: Record<string, unknown> = {}) {
  await openBuilt(page, { sections: [spec], ...extra });
  await expect(page.locator(".sheet")).toHaveCount(PAGES);
}

test.describe("far pages", () => {
  test("a long document opens with most of it out of the layout, and typing in the middle keeps it so", async ({ page }) => {
    await open(page);
    const c0 = await counts(page);
    expect(c0.total).toBe(TOTAL);
    expect(c0.far).toBeGreaterThan(TOTAL - 5 * PER_PAGE);
    expect(c0.laidOut).toBeLessThan(5 * PER_PAGE);
    // Go to the middle and type.
    const mid = await idAt(page, 610);
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; scrollParaIntoView(p: number): void };
      e.select({ p, o: 0 });
      e.scrollParaIntoView(p);
    }, mid);
    await settled(page);
    expect(await hasBox(page, mid)).toBe(true);
    await page.locator(".flow").focus();
    await page.keyboard.type("42 ", { delay: 5 });
    await expect(page.locator(`.flow [data-p="${mid}"]`)).toContainText("42 P 611");
    await settled(page);
    const c1 = await counts(page);
    expect(c1.laidOut).toBeLessThan(6 * PER_PAGE);
    expect(c1.far).toBeGreaterThan(TOTAL - 6 * PER_PAGE);
    // The first page is far now; the middle is drawn.
    expect(await hasBox(page, await idAt(page, 1))).toBe(false);
    expect(await hasBox(page, mid)).toBe(true);
    // The stretch is where the layout puts it with everything drawn (the geometry check ran after each change); the sheet count is unchanged.
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
  });

  test("far blocks are kept in a few hidden boxes, which follow a jump into the middle of one, scrolling and typing", async ({ page }) => {
    // Chrome tells the input method where the caret is by walking the flow's children on every key, hidden ones too: the flow keeps
    // its far blocks in one box a run, so it has a few children, not a child a block.
    await open(page);
    const shape = () =>
      inPage(page, (w) => {
        const flow = (w.fe.editor as unknown as { flow: HTMLElement }).flow;
        const kids = Array.from(flow.children) as HTMLElement[];
        const boxes = kids.filter((k) => k.classList.contains("far-box"));
        return {
          children: kids.length,
          boxes: boxes.length,
          boxed: boxes.reduce((n, b) => n + b.children.length, 0),
          // Every boxed block carries the far mark, every far block is in a box, and no box is empty.
          right: boxes.every((b) => b.children.length > 0 && Array.from(b.children).every((c) => c.hasAttribute("data-far"))) && kids.every((k) => !k.hasAttribute("data-far") || k.classList.contains("far-box")),
        };
      });
    const s0 = await shape();
    expect(s0.right).toBe(true);
    expect(s0.boxed).toBe((await counts(page)).far);
    expect(s0.children).toBeLessThan(6 * PER_PAGE);
    // A jump to a paragraph deep in a box: it comes out (its box is cut in two), and the rest stays boxed.
    const mid = await idAt(page, 610);
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; scrollParaIntoView(p: number): void };
      e.select({ p, o: 0 });
      e.scrollParaIntoView(p);
    }, mid);
    await settled(page);
    expect(await hasBox(page, mid)).toBe(true);
    const s1 = await shape();
    expect(s1.right).toBe(true);
    expect(s1.boxes).toBeGreaterThanOrEqual(2);
    expect(s1.children).toBeLessThan(8 * PER_PAGE);
    // Typing there, a new paragraph and scrolling to the end keep the boxes right.
    await page.locator(".flow").focus();
    await page.keyboard.type("Boxed ", { delay: 5 });
    await page.keyboard.press("Enter");
    await expect(page.locator(`.flow [data-p="${mid}"]`)).toContainText("Boxed");
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight));
    await settled(page);
    await expect.poll(async () => (await shape()).right).toBe(true);
    const s2 = await shape();
    expect(s2.boxed).toBe((await counts(page)).far);
    expect(s2.children).toBeLessThan(8 * PER_PAGE);
    // Drawing everything (print, PDF) takes every block out of its box.
    const boxedWhileDrawn = await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { withDrawn<T>(fn: () => T): T; flow: HTMLElement };
      return e.withDrawn(() => (Array.from(e.flow.children) as HTMLElement[]).some((k) => k.classList.contains("far-box")));
    });
    expect(boxedWhileDrawn).toBe(false);
  });

  test("scrolling to the end draws the end and leaves the start out", async ({ page }) => {
    await open(page);
    expect(await hasBox(page, await idAt(page, TOTAL - 1))).toBe(true); // the last block is always drawn (the page count needs it)
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight));
    await settled(page);
    await expect.poll(async () => hasBox(page, await idAt(page, 6 * PER_PAGE))).toBe(false);
    const last = await idAt(page, TOTAL - 3);
    expect(await hasBox(page, last)).toBe(true);
    expect(await hasBox(page, await idAt(page, 6 * PER_PAGE))).toBe(false);
    // And back: the first page is drawn again, the end is not.
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = 0));
    await settled(page);
    await expect.poll(() => hasBox(page, last)).toBe(false);
    expect(await hasBox(page, await idAt(page, 1))).toBe(true);
    // Scrolling to the middle by the scrollbar lands on the page the view is on.
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight / 2));
    await settled(page);
    const view = await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { canvas: HTMLElement };
      const c = e.canvas.getBoundingClientRect();
      const hit = Array.from(document.querySelectorAll<HTMLElement>(".flow > .fp, .flow > .far-box > .fp")).find((el) => {
        const r = el.getBoundingClientRect();
        return r.height > 0 && r.top >= c.top && r.bottom <= c.bottom;
      });
      return hit?.textContent ?? "";
    });
    expect(Number(view.replace("P ", ""))).toBeGreaterThan(500);
    expect(Number(view.replace("P ", ""))).toBeLessThan(700);
  });

  test("select all, copy and delete take every paragraph, far ones too", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await open(page);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press(`${MOD}+C`);
    const clip = await page.evaluate(() => navigator.clipboard.readText());
    expect(clip).toContain("P 1\n");
    expect(clip).toContain("P 600");
    expect(clip).toContain(`P ${TOTAL}`);
    expect(clip.split("\n").filter((l) => /^P \d+$/.test(l.trim())).length).toBe(TOTAL);
    await page.keyboard.press("Delete");
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.text().trim())).toBe("");
    await expect(page.locator(".sheet")).toHaveCount(1);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
    expect(await inPage(page, (w) => w.fe.editor.engine!.text().includes("P 1200"))).toBe(true);
  });

  test("Find to a far match, Go to a page and a bookmark go there and draw it", async ({ page }) => {
    await open(page, longDoc((i) => (i === 905 ? { xml: '<w:bookmarkStart w:id="1" w:name="Far"/><w:bookmarkEnd w:id="1"/>' } : {})));
    // Find.
    await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
    await page.keyboard.press(`${MOD}+F`);
    const bar = page.getByRole("search");
    await expect(bar).toBeVisible();
    await bar.getByRole("searchbox", { name: "Find" }).fill("P 1137");
    await page.keyboard.press("Enter");
    // The match is the current one, painted where it is: its block is drawn and in view.
    await expect
      .poll(() =>
        page.evaluate(() => {
          // A fixed range (it keeps its text when a far block is drawn), read through a live one.
          const hl = (CSS as unknown as { highlights: Map<string, Set<AbstractRange>> }).highlights.get("find-current");
          const fixed = hl ? Array.from(hl)[0] : null;
          const el = fixed?.startContainer.parentElement?.closest<HTMLElement>(".fp");
          if (!fixed || !el) return "none";
          const r = document.createRange();
          r.setStart(fixed.startContainer, fixed.startOffset);
          r.setEnd(fixed.endContainer, fixed.endOffset);
          const box = r.getBoundingClientRect();
          const c = document.querySelector(".canvas")!.getBoundingClientRect();
          return `${r.toString()}|${el.getClientRects().length > 0}|${box.height > 0 && box.bottom > c.top && box.top < c.bottom}`;
        }),
      )
      .toBe("P 1137|true|true");
    await page.keyboard.press("Escape");
    // Go to a page.
    await page.locator(".flow").focus();
    await page.keyboard.press(`${MOD}+g`);
    const go = page.getByRole("dialog", { name: "Go to" });
    await expect(go).toBeVisible();
    await go.getByRole("option", { name: "Page", exact: true }).click();
    await go.getByLabel("Page number").fill("31");
    await go.getByRole("button", { name: "Go to", exact: true }).click();
    await expect.poll(() => caretText(page)).toBe("P 601");
    expect(await hasBox(page, await caretP(page))).toBe(true);
    await go.getByRole("button", { name: "Close" }).click().catch(() => page.keyboard.press("Escape"));
    // A bookmark.
    const picked = await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { engine: { bookmarks(): { name: string }[] }; selectBookmark(b: unknown): boolean };
      const b = e.engine.bookmarks().find((x) => x.name === "Far");
      return b ? e.selectBookmark(b) : false;
    });
    expect(picked).toBe(true);
    await settled(page);
    expect(await caretText(page)).toBe("P 905");
    expect(await hasBox(page, await caretP(page))).toBe(true);
    const inView = await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { canvas: HTMLElement };
      const el = document.querySelector<HTMLElement>(`.flow [data-p="${w.fe.editor.sel!.focus.p}"]`)!;
      const r = el.getBoundingClientRect();
      const c = e.canvas.getBoundingClientRect();
      return r.bottom > c.top && r.top < c.bottom;
    });
    expect(inView).toBe(true);
  });

  test("a link to a bookmark and the navigation pane go to a far heading and draw it", async ({ page }) => {
    const heading = '<w:pStyle w:val="Heading1"/>';
    await openBuilt(page, {
      styles: `<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style>`,
      sections: [
        longDoc((i) => {
          if (i === 3) return { xml: '<w:hyperlink w:anchor="Chapter"><w:r><w:t xml:space="preserve"> to the chapter</w:t></w:r></w:hyperlink>' };
          if (i === 1015) return { ppr: heading, xml: '<w:bookmarkStart w:id="2" w:name="Chapter"/><w:bookmarkEnd w:id="2"/>' };
          return {};
        }),
      ],
    });
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
    expect(await hasBox(page, await idAt(page, 1014))).toBe(false);
    // The link (Ctrl/Cmd+click).
    await page.locator(".flow .lnk").first().click({ modifiers: [MOD === "ControlOrMeta" ? "ControlOrMeta" : "Control"] });
    await expect.poll(() => caretText(page)).toContain("P 1015");
    expect(await hasBox(page, await idAt(page, 1014))).toBe(true);
    // The navigation pane: back to the top, then the heading.
    await page.locator(".canvas").evaluate((c) => (c.scrollTop = 0));
    await settled(page);
    await page.locator(".flow .fp").first().click();
    await expect.poll(() => hasBox(page, 1)).toBe(true);
    await page.locator(".sidebar").getByRole("tab", { name: "Navigation", exact: true }).click();
    const pane = page.locator(".nav-pane");
    await pane.getByRole("tab", { name: "Headings", exact: true }).click();
    await expect.poll(async () => hasBox(page, await idAt(page, 1014))).toBe(false);
    await pane.locator(".nav-item", { hasText: "P 1015" }).click();
    await expect.poll(() => caretText(page)).toContain("P 1015");
    expect(await hasBox(page, await idAt(page, 1014))).toBe(true);
  });

  test("the arrow keys cross the edge of the drawn stretch one line at a time", async ({ page }) => {
    await open(page);
    // Park the view on page 30 and walk down through the first block of the page after it.
    const first = await idAt(page, 29 * PER_PAGE);
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; scrollParaIntoView(p: number): void };
      e.select({ p, o: 0 });
      e.scrollParaIntoView(p);
    }, first);
    await settled(page);
    await page.locator(".flow").focus();
    for (let i = 0; i < 2 * PER_PAGE + 3; i++) {
      const before = await caretP(page);
      await page.keyboard.press("ArrowDown");
      await settled(page);
      const after = await caretP(page);
      expect(after - before, `down ${i}`).toBeLessThanOrEqual(1);
      expect(after - before, `down ${i}`).toBeGreaterThanOrEqual(0);
    }
    expect(await caretText(page)).toBe(`P ${29 * PER_PAGE + 1 + 2 * PER_PAGE + 3}`);
    for (let i = 0; i < 4 * PER_PAGE + 5; i++) {
      const before = await caretP(page);
      await page.keyboard.press("ArrowUp");
      await settled(page);
      const after = await caretP(page);
      expect(before - after, `up ${i}`).toBeLessThanOrEqual(1);
      expect(before - after, `up ${i}`).toBeGreaterThanOrEqual(0);
    }
    expect(await caretText(page)).toBe(`P ${29 * PER_PAGE + 1 + 2 * PER_PAGE + 3 - (4 * PER_PAGE + 5)}`);
    // Page Down scrolls by about a screen, and what comes into view is drawn (never a blank gap): the first paragraph of every page in view has a box.
    for (let i = 0; i < 4; i++) {
      await page.keyboard.press("PageDown");
      await settled(page);
      const missing = await inPage(page, (w) => {
        const e = w.fe.editor as unknown as { canvas: HTMLElement };
        const c = e.canvas.getBoundingClientRect();
        const kids = Array.from(document.querySelectorAll<HTMLElement>(".flow > .fp, .flow > .far-box > .fp"));
        const out: number[] = [];
        Array.from(document.querySelectorAll<HTMLElement>(".sheet")).forEach((sheet, k) => {
          const r = sheet.getBoundingClientRect();
          if (r.bottom > c.top && r.top < c.bottom && kids[k * 20].getClientRects().length === 0) out.push(k);
        });
        return out;
      });
      expect(missing, `page down ${i}`).toEqual([]);
    }
  });

  test("dragging a selection down with the view following it ends where the pointer is", async ({ page }) => {
    await open(page);
    const first = await idAt(page, 20);
    await page.locator(`.flow [data-p="${first}"]`).click();
    const canvas = await page.locator(".canvas").boundingBox();
    const start = await page.locator(`.flow [data-p="${first}"]`).boundingBox();
    expect(canvas && start).toBeTruthy();
    await page.mouse.move(start!.x + 20, start!.y + 6);
    await page.mouse.down();
    // Pull to the bottom edge, past it: the view scrolls by itself, and new pages are drawn ahead of it.
    await page.mouse.move(start!.x + 20, canvas!.y + canvas!.height - 4, { steps: 8 });
    await page.mouse.move(start!.x + 20, canvas!.y + canvas!.height + 20, { steps: 4 });
    const y0 = await page.locator(".canvas").evaluate((c) => c.scrollTop);
    await expect.poll(() => page.locator(".canvas").evaluate((c) => c.scrollTop), { timeout: 5000 }).toBeGreaterThan(y0 + 600);
    await page.mouse.move(start!.x + 20, canvas!.y + canvas!.height - 30, { steps: 4 });
    await settled(page);
    await page.mouse.up();
    const sel = await inPage(page, (w) => ({ a: w.fe.editor.sel!.anchor.p, f: w.fe.editor.sel!.focus.p }));
    expect(sel.a).toBe(first);
    expect(sel.f).toBeGreaterThan(first + PER_PAGE);
    // Every block between the ends is part of the selection: deleting takes them all.
    await page.keyboard.press("Delete");
    const left = await inPage(page, (w) => w.fe.editor.engine!.text().split("\n").filter((l) => l.trim()).length);
    expect(left).toBeLessThan(TOTAL - PER_PAGE);
    expect(left).toBeGreaterThan(TOTAL - 4 * PAGES * PER_PAGE);
  });

  test("print carries every page's text, and the far blocks go back after", async ({ page }) => {
    await open(page);
    const before = await counts(page);
    expect(before.far).toBeGreaterThan(0);
    // The print path: the pages are copied into the page when the browser prints.
    const pages = await page.evaluate(async () => {
      const w = window as unknown as { docxtor: { fe: { editor: { withDrawn<T>(fn: () => T): T; flow: HTMLElement; sheetsElement: HTMLElement } } } };
      const ed = w.docxtor.fe.editor;
      return ed.withDrawn(() => {
        const kids = (Array.from(ed.flow.children) as HTMLElement[]).flatMap((k) => (k.classList.contains("far-box") ? (Array.from(k.children) as HTMLElement[]) : [k]));
        return { boxes: kids.filter((k) => k.getClientRects().length > 0).length, far: kids.filter((k) => k.hasAttribute("data-far")).length, last: kids[kids.length - 1].textContent };
      });
    });
    expect(pages.boxes).toBe(TOTAL);
    expect(pages.far).toBe(0);
    expect(pages.last).toBe(`P ${TOTAL}`);
    await settled(page);
    const after = await counts(page);
    expect(after.far).toBeGreaterThan(TOTAL - 6 * PER_PAGE);
    // Print through the menu: every page's paragraphs are in the printed copy.
    await page.evaluate(() => {
      const calls: string[] = [];
      (window as unknown as { __printText: string[] }).__printText = calls;
      window.print = () => void calls.push(document.querySelector(".docxtor-print")?.textContent ?? "");
    });
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+P`);
    await expect.poll(() => page.evaluate(() => (window as unknown as { __printText: string[] }).__printText.length)).toBe(1);
    const text = await page.evaluate(() => (window as unknown as { __printText: string[] }).__printText[0]);
    for (const n of [1, 2, 300, 601, 999, TOTAL]) expect(text).toContain(`P ${n}`);
    expect(await page.locator(".docxtor-print .print-page").count()).toBe(PAGES);
    expect(await page.locator(".docxtor-print [data-far]").count()).toBe(0);
  });

  test("the PDF has every page, with the text of far pages", async ({ page }) => {
    await open(page);
    const before = await counts(page);
    expect(before.far).toBeGreaterThan(0);
    const [download] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
    const pdf = readPdf(readFileSync((await download.path())!));
    expect(pdf.pages).toHaveLength(PAGES);
    for (const [k, n] of [[0, 1], [29, 581], [59, 1181]] as const) expect(textRuns(pdf, pdf.pages[k]).map((r) => r.text).join(" ").replace(/\s+/g, " ")).toContain(`P ${n}`);
    // The pages that were left out are left out again.
    await expect.poll(async () => (await counts(page)).far).toBeGreaterThan(TOTAL - 6 * PER_PAGE);
  });

  test("footnote numbers and page fields are right on far pages", async ({ page }) => {
    const note = (id: number) => `<w:footnote w:id="${id}"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Note ${id}.</w:t></w:r></w:p></w:footnote>`;
    const ref = (id: number) => `<w:r><w:footnoteReference w:id="${id}"/></w:r>`;
    let id = 0;
    const spec = longDoc((i) => {
      const first = i % PER_PAGE === 1;
      const xml = `${first ? ref(++id) : ""}${i % PER_PAGE === 2 ? ref(++id) : ""}${i % PER_PAGE === 3 ? '<w:fldSimple w:instr="PAGE"><w:r><w:t>0</w:t></w:r></w:fldSimple>' : ""}`;
      return xml ? { xml } : {};
    });
    await openBuilt(page, { settings: '<w:footnotePr><w:numRestart w:val="eachPage"/></w:footnotePr>', footnotes: Array.from({ length: id }, (_, i) => note(i + 1)).join(""), sections: [spec] });
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
    expect((await counts(page)).far).toBeGreaterThan(TOTAL - 6 * PER_PAGE);
    const marks = await page.locator(".flow .atom.note:not(.endnote)").allTextContents();
    expect(marks).toEqual(Array.from({ length: PAGES }, () => ["1", "2"]).flat());
    for (const k of [0, 22, 59]) expect(await page.locator(".sheet").nth(k).locator(".notes .atom.note").allTextContents()).toEqual(["1", "2"]);
    // Every PAGE field shows its own page.
    const fields = await page.locator(".flow [data-fld='PAGE']").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.val ?? e.textContent));
    expect(fields).toEqual(Array.from({ length: PAGES }, (_, k) => String(k + 1)));
    // Edit on a far page and the numbers stay right.
    await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; scrollParaIntoView(p: number): void };
      const p = Number(document.querySelectorAll<HTMLElement>(".flow [data-p]")[800].dataset.p);
      e.select({ p, o: 0 });
      e.scrollParaIntoView(p);
    });
    await settled(page);
    await page.locator(".flow").focus();
    await page.keyboard.type("x ");
    await page.waitForTimeout(500);
    const marks2 = await page.locator(".flow .atom.note:not(.endnote)").allTextContents();
    expect(marks2).toEqual(marks);
  });

  test("an edit made far from the view, undone and redone, keeps the pages where they are", async ({ page }) => {
    await open(page);
    const far = await idAt(page, 1000);
    expect(await hasBox(page, far)).toBe(false);
    // An edit made to a paragraph far from the view (as the assistant or Find and Replace make them).
    await inPage(page, (w, p) => (w.fe.editor as unknown as { exec(op: unknown): void }).exec({ op: "insert_text", at: { p, o: 0 }, text: "EDIT " }), far);
    await expect(page.locator(`.flow [data-p="${far}"]`)).toHaveText("EDIT P 1001");
    await page.waitForTimeout(400);
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
    await page.locator(".flow .fp").first().click();
    await expect.poll(() => hasBox(page, far)).toBe(false);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => page.locator(`.flow [data-p="${far}"]`).textContent()).toBe("P 1001");
    await page.waitForTimeout(300);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => page.locator(`.flow [data-p="${far}"]`).textContent()).toBe("EDIT P 1001");
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
  });

  test("a structure change far from the view (a new paragraph, a deleted one) keeps the pages where they are", async ({ page }) => {
    await open(page);
    const p0 = await idAt(page, 500);
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; scrollParaIntoView(p: number): void };
      e.select({ p, o: 2 });
      e.scrollParaIntoView(p);
    }, p0);
    await settled(page);
    await page.locator(".flow").focus();
    await page.keyboard.press("Enter");
    await page.keyboard.type("new");
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    await expect(page.locator(`.flow [data-p="${p0}"]`)).toHaveText("P 501");
    expect((await counts(page)).total).toBe(TOTAL);
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
  });
});

/** What the far-pages bookkeeping has done so far: blocks looked at, and times it went over every block (exact). */
const work = (page: Page) =>
  inPage(page, (w) => {
    const pg = (w.fe.editor as unknown as { paginator: { farVisits: number; farLong: number } }).paginator;
    return { visits: pg.farVisits, long: pg.farLong };
  });

/** Move the caret to the paragraph and bring it into view (drawn), as a person does by scrolling. */
const goTo = async (page: Page, n: number) => {
  const p = await idAt(page, n);
  await inPage(page, (w, id) => {
    const e = w.fe.editor as unknown as { select(s: unknown): void; scrollParaIntoView(p: number): void };
    e.select({ p: id, o: 0 });
    e.scrollParaIntoView(id);
  }, p);
  await settled(page);
  await page.locator(".flow").focus();
  return p;
};

test.describe("far pages: changing what is drawn costs what changes", () => {
  // The paginator's checks (on in every spec) draw everything and leave it out again after each change, which the counts would show: they are off for the
  // counted run, and on for the one that checks the pages and the stretch after every change.
  for (const checks of [true, false]) test(`an undo and a redo of a paste of 200 paragraphs look at blocks in proportion to the blocks that change (checks ${checks ? "on" : "off"})`, async ({ page }) => {
    await open(page);
    await inPage(page, (w, on) => ((w.fe.editor as unknown as { paginator: { verify: boolean } }).paginator.verify = on), checks);
    await goTo(page, 610);
    const lines = Array.from({ length: 200 }, (_, i) => `Pasted ${i + 1}`).join("\n");
    const w0 = await work(page);
    await pasteData(page, { "text/plain": lines });
    await expect(page.locator(".flow")).toContainText("Pasted 200");
    await settled(page);
    await page.waitForTimeout(400);
    const pasted = await page.locator(".sheet").count();
    expect(pasted).toBeGreaterThan(PAGES + 2);
    const w1 = await work(page);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => page.locator(".sheet").count()).toBe(PAGES);
    await settled(page);
    await page.waitForTimeout(400);
    const total = (await counts(page)).total;
    expect(total).toBe(TOTAL);
    // The caret was in a pasted paragraph: it goes to the start of the first block after the paste.
    expect(await caretText(page)).toBe("P 612");
    const w2 = await work(page);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => page.locator(".sheet").count()).toBe(pasted);
    await settled(page);
    await page.waitForTimeout(400);
    const w3 = await work(page);
    // The text after the change is laid out and drawn as the pass reaches it, and left out again after: that is a visit or three for each block, once. What it must not be is
    // the length of the text for each block that changed (200 x 1,400).
    const bound = 4 * (TOTAL + 200);
    for (const [name, a, b] of [["paste", w0, w1], ["undo", w1, w2], ["redo", w2, w3]] as const) {
      if (!checks) expect(b.visits - a.visits, `${name}: blocks looked at`).toBeLessThan(bound);
      if (!checks) expect(b.long - a.long, `${name}: times over every block`).toBeLessThanOrEqual(2);
    }
    // The pages are where they were: the paginator's check and the geometry check ran after every change; the count of drawn blocks is the stretch's again.
    const c = await counts(page);
    expect(c.laidOut).toBeLessThan(8 * PER_PAGE);
  });

  test("an edit far from the view, undone and redone, changes the records by a few blocks", async ({ page }) => {
    await open(page);
    await inPage(page, (w) => ((w.fe.editor as unknown as { paginator: { verify: boolean } }).paginator.verify = false));
    const far = await idAt(page, 1000);
    const w0 = await work(page);
    await inPage(page, (w, p) => (w.fe.editor as unknown as { exec(op: unknown): void }).exec({ op: "insert_text", at: { p, o: 0 }, text: "EDIT " }), far);
    await expect(page.locator(`.flow [data-p="${far}"]`)).toHaveText("EDIT P 1001");
    await page.waitForTimeout(400);
    const w1 = await work(page);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => page.locator(`.flow [data-p="${far}"]`).textContent()).toBe("P 1001");
    await page.waitForTimeout(400);
    const w2 = await work(page);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => page.locator(`.flow [data-p="${far}"]`).textContent()).toBe("EDIT P 1001");
    await page.waitForTimeout(400);
    const w3 = await work(page);
    for (const [name, a, b] of [["edit", w0, w1], ["undo", w1, w2], ["redo", w2, w3]] as const) {
      expect(b.visits - a.visits, `${name}: blocks looked at`).toBeLessThan(2000);
      expect(b.long - a.long, `${name}: times over every block`).toBe(0);
    }
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
  });
});

test.describe("far pages in a split view", () => {
  test("each pane draws its own stretch, and an edit in one arrives in the other", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "View", "Window", "Split");
    const flows = page.locator(".flow");
    await expect(flows).toHaveCount(2);
    const state = await inPage(page, () => Array.from(document.querySelectorAll<HTMLElement>(".flow")).map((f) => {
      const kids = (Array.from(f.children) as HTMLElement[]).flatMap((k) => (k.classList.contains("far-box") ? (Array.from(k.children) as HTMLElement[]) : [k]));
      return { far: kids.filter((k) => k.hasAttribute("data-far")).length, total: kids.length };
    }));
    for (const s of state) expect(s.far).toBeGreaterThan(s.total - 6 * PER_PAGE);
    // The second pane goes to page 40 on its own: its stretch moves, the first pane's stays at the top.
    await flows.nth(1).evaluate((f) => {
      const canvas = f.closest(".canvas") as HTMLElement;
      canvas.scrollTop = canvas.scrollHeight * 0.65;
    });
    // The furthest paragraph with a layout box (the caret keeps the top of the text drawn in each pane; the last block always is).
    const furthest = (n: number) =>
      flows.nth(n).evaluate((f) => {
        const kids = Array.from(f.children).slice(1, -1) as HTMLElement[];
        return Math.max(...kids.filter((k) => k.getClientRects().length > 0).map((k) => Number((k.textContent ?? "").replace("P ", ""))));
      });
    await expect.poll(() => furthest(1)).toBeGreaterThan(600);
    expect(await furthest(0)).toBeLessThan(100);
    // Type in the second pane, on a far page of the first: the first pane takes the edit without drawing it.
    const p = await flows.nth(1).evaluate((f) => {
      const kids = (Array.from(f.children).slice(1, -1) as HTMLElement[]).filter((k) => k.getClientRects().length > 0 && Number((k.textContent ?? "").replace("P ", "")) > 600);
      return Number(kids[0].dataset.p);
    });
    await flows.nth(1).locator(`[data-p="${p}"]`).click();
    await page.keyboard.press("Home");
    await page.keyboard.type("Edited ");
    await expect(flows.nth(0).locator(`[data-p="${p}"]`)).toContainText("Edited");
    // The block arrived drawn; once the first pane has laid its pages out it is left out again.
    await expect.poll(() => inPage(page, (w, id) => (document.querySelectorAll(`.flow [data-p="${id}"]`)[0] as HTMLElement).getClientRects().length, p)).toBe(0);
    await chooseMenu(page, "View", "Window", "Remove split");
    await expect(flows).toHaveCount(1);
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
  });
});
