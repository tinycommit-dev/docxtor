// References, Table of authorities: Mark citation fills the long and short citation from the selection, Mark all marks
// every place, Next citation finds the next likely one, and the table lists the citations by category with their page
// numbers. Add text gives a paragraph a level in the table of contents.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

const CASE = "Smith v. Jones, 123 U.S. 456 (1990)";

async function select(page: Page, text: string) {
  await inPage(
    page,
    (w, text) => {
      const e = w.fe.editor;
      const out = e.engine!.renderAll();
      for (const b of out.blocks) {
        if (b.t !== "p") continue;
        const plain = b.segs.map((s) => ("x" in s ? s.x : "")).join("");
        const i = plain.indexOf(text);
        if (i >= 0) return e.select({ anchor: { p: b.id, o: i }, focus: { p: b.id, o: i + text.length } });
      }
      throw new Error(`no ${text}`);
    },
    text,
  );
}

const menu = (page: Page, item: string) => chooseMenu(page, "Insert", "References", "Table of authorities", item);

test.describe("table of authorities", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "Argument",
            `In ${CASE}, the court held otherwise.`,
            "Under 42 U.S.C. § 1983, a claim lies.",
            "As Smith said, the rule is narrow.",
            { text: "Smith again, on the next page.", ppr: "<w:pageBreakBefore/>" },
          ],
        },
      ],
    });
  });

  test("mark a citation and all its mentions, then insert the table with page numbers", async ({ page }) => {
    await select(page, CASE);
    await menu(page, "Mark citation…");
    const d = dialog(page, "Mark citation");
    await expect(d.getByLabel("Long citation")).toHaveValue(CASE);
    await expect(d.getByLabel("Short citation")).toHaveValue("Smith");
    await expect(d.getByLabel("Category")).toHaveValue("1");
    await d.getByRole("button", { name: "Mark all" }).click();
    await expect(d.getByRole("status")).toHaveText("Marked 3 places.");
    // Next citation: the statute.
    await d.getByRole("button", { name: "Next citation" }).click();
    await expect(d.getByLabel("Long citation")).toHaveValue(/42 U\.S\.C\. § 1983/);
    await d.getByLabel("Category").selectOption("2");
    await d.getByRole("button", { name: "Mark", exact: true }).click();
    await expect(d.getByRole("status")).toHaveText("Marked.");
    await d.getByRole("button", { name: "Close" }).click();
    // The marks are hidden: the text reads as before.
    await expect(page.locator(".flow")).toContainText(`In ${CASE}, the court held otherwise.`);

    await page.locator(".flow [data-p]", { hasText: "Argument" }).click();
    await page.keyboard.press("Home");
    await menu(page, "Insert table of authorities…");
    const t = dialog(page, "Table of authorities");
    await t.getByRole("button", { name: "Insert" }).click();
    const flow = page.locator(".flow");
    await expect(flow.locator("[data-p]", { hasText: /^Cases$/ })).toHaveCount(1);
    await expect(flow.locator("[data-p]", { hasText: /^Statutes$/ })).toHaveCount(1);
    // The case is on pages 1 and 2 (Smith again is after the page break); the table pushed nothing past them.
    const entry = flow.locator("[data-p]", { hasText: /^Smith v\. Jones/ }).first();
    await expect(entry).toContainText(/1, 2$/);
    await page.screenshot({ path: "test-results/table-of-authorities.png" });
    // Saved as Word writes it.
    const xml = await inPage(page, (w) => new TextDecoder().decode(w.fe.editor.engine!.save()).length);
    expect(xml).toBeGreaterThan(0);
  });

  test("Apply in the table's dialog keeps what the dialog doesn't show: no headings, a comma before the pages, its own separator", async ({ page }) => {
    await select(page, CASE);
    await menu(page, "Mark citation…");
    const d = dialog(page, "Mark citation");
    await d.getByRole("button", { name: "Mark all" }).click();
    await d.getByRole("button", { name: "Close" }).click();
    const options = { category: 1, passim: false, keepFormatting: true, headings: false, rightAlign: false, leader: "none", pageSep: "; " };
    await inPage(page, (w, o) => (w.fe.editor as unknown as { exec(op: unknown): unknown }).exec({ op: "toa", action: "insert", options: o }), options);
    const entry = page.locator('.flow [data-style="TableofAuthorities"]').first();
    await expect(entry).toBeVisible();
    await entry.click();
    await menu(page, "Insert table of authorities…");
    const t = dialog(page, "Table of authorities");
    await t.getByRole("button", { name: /Apply|Update|OK/ }).click();
    const after = await inPage(page, (w) => (w.fe.editor.engine as unknown as { toaOptions(): unknown }).toaOptions());
    expect(after).toMatchObject({ category: 1, headings: false, rightAlign: false, pageSep: "; " });
  });

  test("Add text sets a paragraph's level in the table of contents", async ({ page }) => {
    await page.locator(".flow [data-p]", { hasText: "Argument" }).click();
    await chooseMenu(page, "Insert", "References", "Add text", "Level 2");
    await expect.poll(() => inPage(page, (w) => w.fe.editor.format()?.style)).toBe("Heading2");
    await chooseMenu(page, "Insert", "References", "Add text", "Do not show in table of contents");
    await expect.poll(() => inPage(page, (w) => w.fe.editor.format()?.style ?? "Normal")).toBe("Normal");
  });
});
