// Everything that works out a page or a position from a block's rectangle goes through the paginator's page map, so that blocks far from the view can be taken out of the layout later. Nothing a person sees changes;
// these tests pin what the moved consumers answer on a long document (30 pages), where a read of every block's rectangle would be
// slow, and, with far pages left out of the layout, wrong.
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { dialog, expect, inPage, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const PAGES = 30;
const PER_PAGE = 20;

/** 30 pages of 20 short paragraphs, "P 1" to "P 600", each page after the first starting with a page break before (so a line is a paragraph). */
function longDoc(extra: Partial<SectionSpec> = {}): SectionSpec {
  const paragraphs: SectionSpec["paragraphs"] = [];
  for (let i = 1; i <= PAGES * PER_PAGE; i++) paragraphs.push(i > 1 && i % PER_PAGE === 1 ? { text: `P ${i}`, ppr: "<w:pageBreakBefore/>" } : `P ${i}`);
  return { paragraphs, ...extra };
}

const text = (page: Page, p: number) => page.locator(`.flow [data-p="${p}"]`).textContent();
const caretText = async (page: Page) => {
  const p = await inPage(page, (w) => w.fe.editor.sel?.focus.p ?? -1);
  return p < 0 ? null : text(page, p);
};
const gotoDialog = (page: Page) => page.getByRole("dialog", { name: "Go to" });

async function openGoto(page: Page, kind: string) {
  await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
  await page.keyboard.press("Home");
  await page.keyboard.press(`${MOD}+g`);
  await expect(gotoDialog(page)).toBeVisible();
  await gotoDialog(page).getByRole("option", { name: kind, exact: true }).click();
}

const go = async (page: Page, label: string, value: string) => {
  await gotoDialog(page).getByLabel(label).fill(value);
  await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
};

test.describe("page map consumers on a long document", () => {
  test("the long document is 30 pages of 20 lines", async ({ page }) => {
    await openBuilt(page, { sections: [longDoc()] });
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
  });

  test("Go to page lands on the first paragraph of that page", async ({ page }) => {
    await openBuilt(page, { sections: [longDoc()] });
    await openGoto(page, "Page");
    for (const k of [17, 30, 2, 1]) {
      await go(page, "Page number", String(k));
      await expect(gotoDialog(page).getByRole("status")).toHaveText(`Page ${k}`);
      expect(await caretText(page)).toBe(`P ${(k - 1) * PER_PAGE + 1}`);
    }
    // Relative: from page 1 forward 12.
    await go(page, "Page number", "+12");
    expect(await caretText(page)).toBe(`P ${12 * PER_PAGE + 1}`);
    await go(page, "Page number", "31");
    await expect(gotoDialog(page).getByRole("status")).toHaveText("The document has 30 pages.");
  });

  test("Go to line counts the lines of every paragraph before it, and + and - move from the caret's line", async ({ page }) => {
    await openBuilt(page, { sections: [longDoc()] });
    await openGoto(page, "Line");
    for (const n of [1, 61, 300, 599, 600]) {
      await go(page, "Line number", String(n));
      expect(await caretText(page)).toBe(`P ${n}`);
    }
    // The caret is on line 600, a line is a paragraph: five back is line 595, then ten forward is none.
    await go(page, "Line number", "-5");
    expect(await caretText(page)).toBe("P 595");
    await go(page, "Line number", "+10");
    await expect(gotoDialog(page).getByRole("status")).toHaveText("There is no line 605.");
    await go(page, "Line number", "601");
    await expect(gotoDialog(page).getByRole("status")).toHaveText("There is no line 601.");
  });

  test("Go to line in a paragraph of several lines goes to the line, not the paragraph", async ({ page }) => {
    const long = Array.from({ length: 150 }, (_, i) => `word${i}`).join(" ");
    await openBuilt(page, { sections: [{ paragraphs: ["First", long, "Last"] }] });
    const rows = await page.locator(".flow .fp").nth(1).evaluate((el) => {
      const range = document.createRange();
      range.selectNodeContents(el);
      return new Set(Array.from(range.getClientRects()).map((r) => Math.round(r.top))).size;
    });
    expect(rows).toBeGreaterThan(3);
    await openGoto(page, "Line");
    await go(page, "Line number", "3");
    const at = await inPage(page, (w) => w.fe.editor.sel!.focus);
    expect(at.o).toBeGreaterThan(20);
    // The line after the paragraph's last is the next paragraph's first.
    await go(page, "Line number", String(1 + rows + 1));
    expect(await caretText(page)).toBe("Last");
  });

  test("line numbers continue across 30 pages, and start again on each page when asked", async ({ page }) => {
    const spec = (restart: string) => ({ sections: [longDoc({ xml: { afterMar: `<w:lnNumType w:countBy="1" w:restart="${restart}"/>` } })] });
    await openBuilt(page, spec("continuous"));
    await expect(page.locator(".sheet")).toHaveCount(PAGES);
    for (const k of [0, 1, 14, 29]) {
      const nums = await page.locator(".sheet").nth(k).locator(".lnum span").allTextContents();
      expect(nums).toEqual(Array.from({ length: PER_PAGE }, (_, i) => String(k * PER_PAGE + i + 1)));
    }
    await openBuilt(page, spec("newPage"));
    for (const k of [0, 7, 29]) {
      const nums = await page.locator(".sheet").nth(k).locator(".lnum span").allTextContents();
      expect(nums).toEqual(Array.from({ length: PER_PAGE }, (_, i) => String(i + 1)));
    }
  });

  test("line numbers carry on through a paragraph cut by a page", async ({ page }) => {
    // 40 lines of one paragraph cut by the page, then a short one: the numbers go on without a gap or a repeat.
    const long = Array.from({ length: 900 }, (_, i) => `word${i}`).join(" ");
    await openBuilt(page, { sections: [{ paragraphs: ["Before", long, "After"], xml: { afterMar: '<w:lnNumType w:countBy="1" w:restart="continuous"/>' } }] });
    const sheets = await page.locator(".sheet").count();
    expect(sheets).toBeGreaterThan(1);
    const all: number[] = [];
    for (let k = 0; k < sheets; k++) all.push(...(await page.locator(".sheet").nth(k).locator(".lnum span").allTextContents()).map(Number));
    expect(all).toEqual(Array.from({ length: all.length }, (_, i) => i + 1));
    // The last paragraph is the last line.
    const lines = await inPage(page, (w) => w.fe.editor.lineTotals().main);
    expect(all.length).toBe(lines);
  });

  test("footnote numbers start again on each of 30 pages, and on the page a long paragraph ends on", async ({ page }) => {
    const note = (id: number) => `<w:footnote w:id="${id}"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Note ${id}.</w:t></w:r></w:p></w:footnote>`;
    const ref = (id: number) => `<w:r><w:footnoteReference w:id="${id}"/></w:r>`;
    const paragraphs: SectionSpec["paragraphs"] = [];
    let id = 0;
    for (let k = 0; k < PAGES; k++) {
      const brk = k ? "<w:pageBreakBefore/>" : "";
      paragraphs.push({ text: `Page ${k + 1} first`, ppr: brk, xml: ref(++id) });
      paragraphs.push({ text: `Page ${k + 1} second`, xml: ref(++id) });
    }
    // A paragraph of 900 words after the last page break runs over three pages; its note is on the page it ends on.
    const long = Array.from({ length: 900 }, (_, i) => `word${i}`).join(" ");
    paragraphs.push({ text: long, ppr: "<w:pageBreakBefore/>", xml: ref(++id) });
    await openBuilt(page, {
      settings: '<w:footnotePr><w:numRestart w:val="eachPage"/></w:footnotePr>',
      footnotes: Array.from({ length: id }, (_, i) => note(i + 1)).join(""),
      sections: [{ paragraphs }],
    });
    const sheets = await page.locator(".sheet").count();
    expect(sheets).toBeGreaterThanOrEqual(PAGES + 2);
    const marks = await page.locator(".flow .atom.note:not(.endnote)").allTextContents();
    expect(marks.slice(0, PAGES * 2)).toEqual(Array.from({ length: PAGES }, () => ["1", "2"]).flat());
    expect(marks[PAGES * 2]).toBe("1");
    for (const k of [0, 13, 29]) expect(await page.locator(".sheet").nth(k).locator(".notes .atom.note").allTextContents()).toEqual(["1", "2"]);
    // The note of the long paragraph is on the last page (the paragraph's last page, not its first).
    expect(await page.locator(".sheet").nth(PAGES).locator(".notes .atom.note").count()).toBe(0);
    expect(await page.locator(".sheet").nth(sheets - 1).locator(".notes .atom.note").allTextContents()).toEqual(["1"]);
  });

  test("word count's lines come from the page map: one a paragraph here, the rows on the page elsewhere", async ({ page }) => {
    await openBuilt(page, { sections: [longDoc()] });
    await page.keyboard.press(`${MOD}+Shift+C`);
    const d = dialog(page, "Word count");
    await expect(d).toBeVisible();
    await expect(d.locator("dt", { hasText: /^Lines$/ }).locator("+ dd")).toHaveText("600");
    await d.getByRole("button", { name: "Done" }).click();
    // A paragraph cut by a page, counted by the rows its text takes (not the paragraph).
    const long = Array.from({ length: 900 }, (_, i) => `word${i}`).join(" ");
    await openBuilt(page, { sections: [{ paragraphs: ["Before", long, "After"] }] });
    const rows = await page.locator(".flow .fp").evaluateAll((els) =>
      els.reduce((sum, el) => {
        const range = document.createRange();
        const tops = new Set<number>();
        const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          if (!(n as Text).data.trim() || n.parentElement?.closest(".pgsp")) continue;
          range.selectNodeContents(n);
          for (const r of Array.from(range.getClientRects())) tops.add(Math.round((r.top + r.bottom) / 2 / 4));
        }
        return sum + Math.max(1, tops.size);
      }, 0),
    );
    await page.keyboard.press(`${MOD}+Shift+C`);
    const d2 = dialog(page, "Word count");
    const shown = Number(await d2.locator("dt", { hasText: /^Lines$/ }).locator("+ dd").textContent());
    expect(Math.abs(shown - rows)).toBeLessThanOrEqual(2);
    expect(await inPage(page, (w) => w.fe.editor.lineTotals().main)).toBe(shown);
  });
});

test.describe("one way in", () => {
  test("ensureDrawn gives the element of a paragraph or block, and opens the tab or fold over it", async ({ page }) => {
    await openBuilt(page, { sections: [longDoc()] });
    const ok = await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { ensureDrawn(t: number | Element): HTMLElement | null };
      const a = e.ensureDrawn(300);
      const b = e.ensureDrawn(a!);
      return a?.dataset.p === "300" && b === a && e.ensureDrawn(99999) === null;
    });
    expect(ok).toBe(true);
  });

  test("a far mark is stripped from copies, and the test switch is off unless set", async ({ page }) => {
    await openBuilt(page, { sections: [longDoc()] });
    const r = await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; range(): [unknown, unknown]; copyPayload(r: [unknown, unknown]): { html: string }; farKeep: number | null };
      const ids = Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")).slice(0, 3);
      e.select({ anchor: { p: Number(ids[0].dataset.p), o: 0 }, focus: { p: Number(ids[2].dataset.p), o: 2 } });
      // Nothing sets the mark in C1: set it by hand on the blocks being copied.
      for (const el of ids) el.setAttribute("data-far", "");
      const html = e.copyPayload(e.range()).html;
      for (const el of ids) el.removeAttribute("data-far");
      return { far: html.includes("data-far"), has: html.includes("P 1"), keep: e.farKeep };
    });
    expect(r).toEqual({ far: false, has: true, keep: process.env.DOCXTOR_FAR ? Number(process.env.DOCXTOR_FAR) : null });
  });
});
