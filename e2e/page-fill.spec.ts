// Page color (Format, Page color…): a plain colour (a swatch or any hex), and the fill effects: a gradient, a texture, a pattern and a
// picture from a file. They are written as Word writes them and come back when the file is opened again.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

type Fill = { kind: string; color?: string; color2?: string; angle?: number; media?: string; title?: string; from?: string };
const fill = (page: Page) => inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as unknown as { pageFill?: unknown }).pageFill) as Promise<Fill | undefined>;
const sheetCss = (page: Page) => page.locator(".sheet").first().evaluate((el) => ({ image: getComputedStyle(el).backgroundImage, color: getComputedStyle(el).backgroundColor, repeat: getComputedStyle(el).backgroundRepeat, size: getComputedStyle(el).backgroundSize }));
const open = async (page: Page) => {
  await chooseMenu(page, "Format", "Page color…");
  const d = dialog(page, "Page color");
  await expect(d).toBeVisible();
  return d;
};
/** The saved file, reopened. */
const reopen = (page: Page) =>
  inPage(page, async (w) => {
    await w.fe.open(w.fe.editor.engine!.save(), "again.docx");
    return (w.fe.editor.engine!.pageSetup() as unknown as { pageFill?: unknown }).pageFill;
  }) as Promise<Fill | undefined>;
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEgAF/QualzQAAAABJRU5ErkJggg==", "base64");

test.describe("Page color", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First page{PBR}", "Second page"] }] });
  });

  test("a swatch or any hex colour, and None", async ({ page }) => {
    let d = await open(page);
    await d.getByRole("button", { name: "#FFF9E6" }).click();
    await expect(d.getByRole("button", { name: "#FFF9E6" })).toHaveAttribute("aria-pressed", "true");
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await sheetCss(page)).color).toBe("rgb(255, 249, 230)");
    d = await open(page);
    await d.getByLabel("Custom color").fill("#123456");
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await sheetCss(page)).color).toBe("rgb(18, 52, 86)");
    expect(await fill(page)).toMatchObject({ kind: "solid", color: "123456" });
    d = await open(page);
    await d.getByRole("button", { name: "None" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await fill(page)).toBeUndefined();
    expect((await sheetCss(page)).color).toBe("rgb(255, 255, 255)");
  });

  test("a gradient: two colours and a direction", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("tab", { name: "Gradient" }).click();
    await d.getByLabel("Color 1").fill("#FFF2CC");
    await d.getByLabel("Color 2").fill("#9CCBF0");
    await d.getByLabel("Shading style").selectOption("vertical");
    await expect(d.locator(".ft-preview")).toHaveCSS("background-image", /linear-gradient/);
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await fill(page)).toMatchObject({ kind: "gradient", color: "FFF2CC", color2: "9CCBF0", angle: -90 });
    const css = await sheetCss(page);
    expect(css.image).toContain("linear-gradient");
    expect(css.image).toContain("rgb(255, 242, 204)");
    await page.screenshot({ path: "test-results/page-fill-gradient.png" });
    // From the middle is a radial one.
    const again = await open(page);
    await again.getByRole("tab", { name: "Gradient" }).click();
    await again.getByLabel("Shading style").selectOption("center");
    await again.getByRole("button", { name: "Apply" }).click();
    expect((await fill(page))!.kind).toBe("radial");
    expect((await sheetCss(page)).image).toContain("radial-gradient");
    // And it comes back from the saved file.
    expect(await reopen(page)).toMatchObject({ kind: "radial", color2: "9CCBF0" });
  });

  test("a texture tiles on every page", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("tab", { name: "Texture" }).click();
    await d.getByRole("button", { name: "Denim" }).click();
    await expect(d.getByRole("button", { name: "Denim" })).toHaveAttribute("aria-pressed", "true");
    await d.getByRole("button", { name: "Apply" }).click();
    const f = (await fill(page))!;
    expect(f).toMatchObject({ kind: "texture", title: "Denim" });
    expect(f.media).toMatch(/media\/.+\.png$/);
    for (const k of [0, 1]) expect(await page.locator(".sheet").nth(k).evaluate((el) => getComputedStyle(el).backgroundImage)).toContain("url(");
    expect((await sheetCss(page)).repeat).toBe("repeat");
    // The picture is in the saved file.
    expect(await reopen(page)).toMatchObject({ kind: "texture", title: "Denim" });
    await page.screenshot({ path: "test-results/page-fill-texture.png" });
  });

  test("a pattern in two colours", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("tab", { name: "Pattern" }).click();
    await d.getByRole("button", { name: "25%" }).click();
    await d.getByLabel("Foreground").fill("#FF4F00");
    await d.getByLabel("Background").fill("#FFFFFF");
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await fill(page)).toMatchObject({ kind: "pattern", color: "FF4F00", color2: "FFFFFF", title: "25%" });
    expect((await sheetCss(page)).image).toContain("url(");
  });

  test("a picture from a file fills the page", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("tab", { name: "Picture" }).click();
    await d.getByLabel("Choose a picture").setInputFiles({ name: "paper.png", mimeType: "image/png", buffer: PNG });
    await expect(d.getByText("paper.png")).toBeVisible();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await fill(page)).toMatchObject({ kind: "picture" });
    const css = await sheetCss(page);
    expect(css.size).toBe("100% 100%");
    expect(css.repeat).toBe("no-repeat");
    expect(await reopen(page)).toMatchObject({ kind: "picture" });
  });

  test("applying with nothing chosen on a fill tab changes nothing", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("tab", { name: "Texture" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await fill(page)).toBeUndefined();
  });

  test("undo takes the fill away in one step", async ({ page }) => {
    const d = await open(page);
    await d.getByRole("button", { name: "#E8EBFA" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await fill(page)).toBeTruthy();
    await page.keyboard.press("ControlOrMeta+Z");
    expect(await fill(page)).toBeUndefined();
  });
});
