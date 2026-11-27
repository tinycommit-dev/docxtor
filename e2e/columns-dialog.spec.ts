// The Columns dialog with unequal widths, and the Left and Right presets (Format, Columns).
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

const cols = (page: Page) => inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as unknown as { sections: { cols: { num: number; space: number; sep: boolean; widths: number[]; spaces?: number[] } }[] }).sections[0].cols);
const near = (a: number, b: number, tolerance = 2) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);

test.describe("unequal columns in the dialog", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Left column{CBR}", "Right column"] }] });
  });

  test("the Left and Right presets make a narrow and a wide column", async ({ page }) => {
    await chooseMenu(page, "Format", "Columns", "Left");
    let c = await cols(page);
    expect(c.num).toBe(2);
    // A third and two thirds of what is left after the gap (6.5 inches of text, half an inch between).
    expect(c.widths[0] + c.widths[1] + c.space).toBe(9360);
    near(c.widths[0] * 2, c.widths[1], 2);
    const fp = page.locator(".flow .fp");
    const a = (await fp.nth(0).boundingBox())!;
    const b = (await fp.nth(1).boundingBox())!;
    expect(a.width).toBeLessThan(b.width);
    near(a.width / b.width, 0.5, 0.02);
    await chooseMenu(page, "Format", "Columns", "Right");
    c = await cols(page);
    expect(c.widths[0]).toBeGreaterThan(c.widths[1]);
  });

  test("the dialog: unchecking Equal column width shows a width and a space for each column", async ({ page }) => {
    await chooseMenu(page, "Format", "Columns", "More options…");
    const d = dialog(page, "Columns");
    await d.getByLabel("Number of columns").fill("2");
    await expect(d.getByLabel("Equal column width")).toBeChecked();
    await d.getByLabel("Equal column width").uncheck();
    await expect(d.getByRole("spinbutton", { name: "Width of column 1 (inches)" })).toBeVisible();
    await d.getByRole("spinbutton", { name: "Width of column 1 (inches)" }).fill("2");
    await d.getByRole("spinbutton", { name: "Width of column 2 (inches)" }).fill("4");
    await d.getByRole("spinbutton", { name: "Space after column 1 (inches)" }).fill("0.5");
    await d.getByLabel("Line between columns").check();
    await d.getByRole("button", { name: "Apply" }).click();
    const c = await cols(page);
    expect(c).toMatchObject({ num: 2, sep: true, widths: [2880, 5760], spaces: [720] });
    // The text is set at those widths, in those places.
    const fp = page.locator(".flow .fp");
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const a = (await fp.nth(0).boundingBox())!;
    const b = (await fp.nth(1).boundingBox())!;
    near(a.width / zoom, 192, 2);
    near(b.width / zoom, 384, 2);
    near((b.x - a.x) / zoom, 192 + 48, 2);
    await page.screenshot({ path: "test-results/columns-dialog-unequal.png" });
    // Reopened, it shows the widths; Equal column width puts them back.
    await chooseMenu(page, "Format", "Columns", "More options…");
    const again = dialog(page, "Columns");
    await expect(again.getByLabel("Equal column width")).not.toBeChecked();
    await expect(again.getByRole("spinbutton", { name: "Width of column 2 (inches)" })).toHaveValue("4");
    await again.getByLabel("Equal column width").check();
    await again.getByRole("button", { name: "Apply" }).click();
    expect((await cols(page)).widths).toEqual([]);
  });

  test("the preview draws the widths", async ({ page }) => {
    await chooseMenu(page, "Format", "Columns", "More options…");
    const d = dialog(page, "Columns");
    await d.getByRole("button", { name: "Left", exact: true }).click();
    const widths = await d.locator(".cp-col").evaluateAll((els) => els.map((e) => e.getBoundingClientRect().width));
    expect(widths).toHaveLength(2);
    expect(widths[0]).toBeLessThan(widths[1]);
    await d.getByRole("button", { name: "Three", exact: true }).click();
    // Equal columns are drawn as the browser makes them.
    await expect(d.locator(".cp-text")).toHaveCSS("column-count", "3");
    await d.getByRole("button", { name: "Cancel" }).click();
  });
});
