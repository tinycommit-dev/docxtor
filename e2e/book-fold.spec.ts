// Book fold and 2 pages per sheet: each page is half the paper on screen, as in Word, and printing puts two pages on a
// sheet: in order for 2 pages per sheet, in booklet order for a book fold (8 pages: 8|1, 2|7, 6|3, 4|5).
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

const chapter = (k: number) => [`Page ${k} heading`, ...Array.from({ length: 4 }, (_, i) => `Text of page ${k}, line ${i + 1}.`)];

async function multiple(page: Page, value: string) {
  await chooseMenu(page, "File", "Page setup…");
  const d = dialog(page, "Page setup");
  await d.getByLabel("Multiple pages").selectOption(value);
  await d.getByRole("button", { name: "Apply" }).click();
  await expect(d).toHaveCount(0);
}

/** Print with the dialog stubbed, and say which page heading each half of each sheet shows. */
async function printed(page: Page): Promise<string[][]> {
  await page.evaluate(() => (window.print = () => undefined));
  await page.keyboard.press("ControlOrMeta+p");
  await expect(page.locator(".docxtor-print .print-sheet").first()).toBeAttached();
  await page.emulateMedia({ media: "print" });
  const sheets = await page.evaluate(() => {
    document.documentElement.classList.add("docxtor-printing");
    const out = [...document.querySelectorAll(".docxtor-print .print-sheet")].map((s) =>
      [...s.querySelectorAll(":scope > .print-half")].map((half) => {
        const r = half.getBoundingClientRect();
        const hit = [...half.querySelectorAll("[data-p]")].find((p) => {
          const q = p.getBoundingClientRect();
          return q.height > 0 && q.top >= r.top - 1 && q.bottom <= r.bottom + 1 && /heading/.test(p.textContent ?? "");
        });
        return hit ? /Page (\d+)/.exec(hit.textContent!)![1] : "-";
      }),
    );
    document.documentElement.classList.remove("docxtor-printing");
    return out;
  });
  await page.emulateMedia({ media: "screen" });
  await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  return sheets;
}

test.describe("two pages to a sheet", () => {
  test.beforeEach(async ({ page }) => {
    // Eight short pages, each starting with its heading.
    const paragraphs = Array.from({ length: 8 }, (_, i) => chapter(i + 1)).flatMap((c, i) => c.map((text, j) => (j === 0 && i > 0 ? { text, ppr: "<w:pageBreakBefore/>" } : text)));
    await openBuilt(page, { sections: [{ paragraphs }] });
  });

  test("a book fold lays each page out as half the sheet and prints booklet order", async ({ page }) => {
    await multiple(page, "bookFold");
    const s = await inPage(page, (w) => w.fe.editor.engine!.pageSetup());
    const box = await inPage(page, (w) => w.fe.editor.pageBox(0));
    expect(Math.round(box.w)).toBe(Math.round(s.width / 15 / 2));
    expect(Math.round(box.h)).toBe(Math.round(s.height / 15));
    await expect(page.locator(".sheet")).toHaveCount(8);
    expect(await printed(page)).toEqual([["8", "1"], ["2", "7"], ["6", "3"], ["4", "5"]]);
  });

  test("2 pages per sheet prints pages in order", async ({ page }) => {
    await multiple(page, "twoUp");
    await expect(page.locator(".sheet")).toHaveCount(8);
    expect(await printed(page)).toEqual([["1", "2"], ["3", "4"], ["5", "6"], ["7", "8"]]);
  });
});
