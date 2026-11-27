// The Page borders dialog (Format, Page borders…): none, box, shadow, 3-D and custom sides, line styles and art borders, colour and
// width, where the border counts from and how far, which pages show it, and what it writes.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

type Borders = { offsetFrom: string; display: string; zOrder: string } & Partial<Record<"top" | "left" | "bottom" | "right", { val: string; sz: number; space: number; color: string; shadow?: boolean }>>;
const borders = (page: Page, section = 0) => inPage(page, (w, i) => (w.fe.editor.engine!.pageSetup() as unknown as { sections: { pageBorders?: unknown }[] }).sections[i].pageBorders, section) as Promise<Borders | undefined>;
const open = async (page: Page) => {
  await chooseMenu(page, "Format", "Page borders…");
  const d = dialog(page, "Page borders");
  await expect(d).toBeVisible();
  return d;
};

test.describe("Page borders", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Bordered page{PBR}", "Second page"] }] });
  });

  test("a box with the style, width and colour chosen, on every page", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("radio", { name: "Box" }).click();
    await d.getByLabel("Style", { exact: true }).selectOption("double");
    await d.getByLabel("Width", { exact: true }).selectOption("12");
    await d.getByLabel("Custom color").fill("#FF4F00");
    await expect(d.locator(".pb-sample")).toHaveCSS("border-top-style", "double");
    await d.getByRole("button", { name: "Apply" }).click();
    const b = await borders(page);
    expect(b).toMatchObject({ offsetFrom: "page", display: "allPages", zOrder: "front" });
    for (const side of ["top", "left", "bottom", "right"] as const) expect(b![side]).toMatchObject({ val: "double", sz: 12, space: 24, color: "FF4F00" });
    await expect(page.locator(".sheet")).toHaveCount(2);
    for (const k of [0, 1]) {
      await expect(page.locator(".sheet").nth(k).locator(".pgb")).toHaveCount(1);
      await expect(page.locator(".sheet").nth(k).locator(".pgb")).toHaveCSS("border-top-color", "rgb(255, 79, 0)");
    }
    await page.screenshot({ path: "test-results/page-borders-dialog.png" });
    // Reopened, the dialog shows what the page has.
    const again = await open(page);
    await expect(again.getByRole("radio", { name: "Box" })).toHaveAttribute("aria-checked", "true");
    await expect(again.getByLabel("Style", { exact: true })).toHaveValue("double");
    await expect(again.getByLabel("Width", { exact: true })).toHaveValue("12");
  });

  test("shadow and 3-D", async ({ page }) => {
    let d = await open(page);
    await d.getByRole("radio", { name: "Shadow" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await borders(page))!.top).toMatchObject({ val: "single", shadow: true });
    await expect(page.locator(".sheet .pgb").first()).not.toHaveCSS("box-shadow", "none");
    d = await open(page);
    await d.getByRole("radio", { name: "3-D" }).click();
    // 3-D has its own styles.
    expect(await d.getByLabel("Style", { exact: true }).locator("option").allTextContents()).toEqual(["Embossed", "Engraved", "Outset", "Inset"]);
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await borders(page))!.left).toMatchObject({ val: "threeDEmboss" });
  });

  test("custom sides: only the ones that are on are written", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("button", { name: "Top", exact: true }).click();
    await d.getByRole("button", { name: "Bottom", exact: true }).click();
    await expect(d.getByRole("radio", { name: "Custom" })).toHaveAttribute("aria-checked", "true");
    await d.getByRole("button", { name: "Apply" }).click();
    const b = (await borders(page))!;
    expect(Object.keys(b).filter((k) => ["top", "left", "bottom", "right"].includes(k)).sort()).toEqual(["bottom", "top"]);
    await expect(page.locator(".sheet").first().locator(".pgb")).toHaveCSS("border-left-style", "none");
  });

  test("an art border, measured from the text, in the back, on the first page only", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("radio", { name: "Box" }).click();
    await d.getByLabel("Art").selectOption("hearts");
    await d.getByLabel("Measure from").selectOption("text");
    await d.getByLabel("Always display in front").uncheck();
    await d.getByLabel("Apply to").selectOption("first");
    await d.getByRole("button", { name: "Apply" }).click();
    const b = (await borders(page))!;
    expect(b).toMatchObject({ offsetFrom: "text", display: "firstPage", zOrder: "back" });
    expect(b.top!.val).toBe("hearts");
    // Measured from the text the distance is Word's 4 points, and the border shows on the first page only.
    expect(b.top!.space).toBe(4);
    await expect(page.locator(".sheet").nth(0).locator(".pgb")).toHaveCount(1);
    await expect(page.locator(".sheet").nth(1).locator(".pgb")).toHaveCount(0);
  });

  test("None takes the borders away, and undo brings them back", async ({ page }) => {
    let d = await open(page);
    await d.getByRole("radio", { name: "Box" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await borders(page)).toBeTruthy();
    d = await open(page);
    await d.getByRole("radio", { name: "None" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await borders(page)).toBeUndefined();
    await expect(page.locator(".sheet .pgb")).toHaveCount(0);
    await page.keyboard.press("ControlOrMeta+Z");
    expect(await borders(page)).toBeTruthy();
  });
});
