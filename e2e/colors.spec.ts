// The colour menus: the toolbar's Text color (theme colours with their lighter and darker rows, standard colours, recent
// colours, More colors…), Highlight (all of Word's sixteen), More colors… (hex, RGB and HSL, and an eyedropper where the
// browser has one), and the dialogs that share the swatches (Borders and shading, Page setup, Watermark).
import { test, expect, chooseMenu, dialog, inPage, selectWord, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;

async function open(page: Page, paragraphs: string[] = [run("", "alpha beta gamma")]) {
  await openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })) }] });
}

/** The formatting of a word's text (the model's view). */
const runAt = (page: Page, o: number, p = 1) => inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos)!.run as Record<string, any>, { p, o: o + 1 });

const textColor = (page: Page) => page.getByRole("button", { name: "Text color" });
const swatch = (page: Page, name: string | RegExp) => page.locator(".pop .palette").getByRole("button", { name });

async function pickWord(page: Page, word = "beta") {
  await selectWord(page, 1, word);
  await textColor(page).click();
  await expect(page.locator(".pop .palette")).toBeVisible();
}

test.describe("Text color", () => {
  test("shows the theme colours with their lighter and darker rows, the standard colours and More colors", async ({ page }) => {
    await open(page);
    await pickWord(page);
    const pal = page.locator(".pop .palette");
    await expect(pal.getByText("Theme colors")).toBeVisible();
    await expect(pal.getByText("Standard colors")).toBeVisible();
    // Ten columns of six: the colour and five variants of it.
    await expect(pal.locator(".pal-theme .pal-col")).toHaveCount(10);
    await expect(pal.locator(".pal-theme .swatch")).toHaveCount(60);
    await expect(pal.locator(".pal-row .swatch")).toHaveCount(10);
    await expect(pal.getByRole("button", { name: "More colors…" })).toBeVisible();
    await expect(pal.getByRole("button", { name: "Automatic" })).toHaveAttribute("aria-pressed", "true");
    // Word's names: white and black columns have darker and lighter rows respectively.
    await expect(swatch(page, "Background 1, darker 5%")).toBeVisible();
    await expect(swatch(page, "Text 1, lighter 50%")).toBeVisible();
    await expect(swatch(page, "Text 2, lighter 90%")).toBeVisible();
    await expect(swatch(page, "Accent 6, darker 50%")).toBeVisible();
  });

  test("a theme colour is written with its tint or shade, and a plain colour replaces it", async ({ page }) => {
    await open(page);
    await pickWord(page);
    await swatch(page, "Accent 1, lighter 80%").click();
    // The Office theme's accent1 (5B9BD5) at tint 33.
    expect(await runAt(page, 6)).toMatchObject({ color: "DEEBF6", colorTheme: { name: "accent1", tint: "33" } });
    // The file has the theme colour and its hex beside it, as Word writes it.
    await textColor(page).click();
    await swatch(page, "Accent 2, darker 25%").click();
    expect(await runAt(page, 6)).toMatchObject({ color: "C55A11", colorTheme: { name: "accent2", shade: "BF" } });
    await textColor(page).click();
    await swatch(page, /^Dark red #C00000$/).click();
    const plain = await runAt(page, 6);
    expect(plain.color).toBe("C00000");
    expect(plain.colorTheme).toBeUndefined();
    // And the page shows it.
    await expect(page.locator('.flow [data-p="1"] .s', { hasText: "beta" })).toHaveCSS("color", "rgb(192, 0, 0)");
    await textColor(page).click();
    await page.locator(".pop .palette").getByRole("button", { name: "Automatic" }).click();
    expect((await runAt(page, 6)).color).toBe("auto");
  });

  test("recent colours show the ones picked lately, newest first", async ({ page }) => {
    await open(page);
    await pickWord(page);
    await expect(page.locator(".pop .palette").getByText("Recent colors")).toHaveCount(0);
    await swatch(page, /^Orange #FFC000$/).click();
    await textColor(page).click();
    await swatch(page, /^Blue #0070C0$/).click();
    await textColor(page).click();
    const recent = page.locator(".pop .palette .pal-row").nth(1).locator(".swatch");
    await expect(recent).toHaveCount(2);
    await expect(recent.nth(0)).toHaveAttribute("aria-label", "#0070C0");
    await expect(recent.nth(1)).toHaveAttribute("aria-label", "#FFC000");
    await recent.nth(1).click();
    expect((await runAt(page, 6)).color).toBe("FFC000");
    // They last across a reload: the app keeps them (the editor only holds them, through its built-in's state).
    await page.reload();
    await open(page);
    await pickWord(page);
    const kept = page.locator(".pop .palette .pal-row").nth(1).locator(".swatch");
    await expect(kept).toHaveCount(2);
    await expect(kept.nth(0)).toHaveAttribute("aria-label", "#FFC000");
  });

  test("More colors takes a hex colour, and its fields follow each other", async ({ page }) => {
    await open(page);
    await pickWord(page);
    await page.locator(".pop .palette").getByRole("button", { name: "More colors…" }).click();
    const d = dialog(page, "Colors");
    await expect(d).toBeVisible();
    // The menu is closed; the dialog is the only thing open.
    await expect(page.locator(".pop")).toHaveCount(0);
    await d.getByLabel("Hex").fill("#12ab34");
    await expect(d.getByLabel("Red", { exact: true })).toHaveValue("18");
    await expect(d.getByLabel("Green", { exact: true })).toHaveValue("171");
    await expect(d.getByLabel("Blue", { exact: true })).toHaveValue("52");
    await expect(d.getByLabel("Hue", { exact: true }).last()).toHaveValue("133");
    await expect(d.locator(".cd-now")).toHaveCSS("background-color", "rgb(18, 171, 52)");
    // Red, green and blue change the hex; hue, saturation and lightness change them all.
    await d.getByLabel("Red", { exact: true }).fill("255");
    await expect(d.getByLabel("Hex")).toHaveValue("#FFAB34");
    await d.getByLabel("Lightness", { exact: true }).last().fill("0");
    await expect(d.getByLabel("Hex")).toHaveValue("#000000");
    await d.getByLabel("Hex").fill("336699");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toBeHidden();
    expect((await runAt(page, 6)).color).toBe("336699");
    // It is a recent colour now.
    await textColor(page).click();
    await expect(page.locator(".pop .palette .pal-row").nth(1).locator(".swatch").first()).toHaveAttribute("aria-label", "#336699");
  });

  test("the eyedropper is offered where the browser has one and picks a colour from the screen", async ({ page }) => {
    await open(page);
    // A browser without an eyedropper has no such button.
    await page.evaluate(() => Object.defineProperty(window, "EyeDropper", { value: undefined, configurable: true, writable: true }));
    await pickWord(page);
    await page.locator(".pop .palette").getByRole("button", { name: "More colors…" }).click();
    await expect(dialog(page, "Colors").getByRole("button", { name: "Pick from screen" })).toHaveCount(0);
    await page.keyboard.press("Escape");
    await page.evaluate(() => {
      (window as unknown as { EyeDropper: unknown }).EyeDropper = class {
        open() {
          return Promise.resolve({ sRGBHex: "#8844aa" });
        }
      };
    });
    await textColor(page).click();
    await page.locator(".pop .palette").getByRole("button", { name: "More colors…" }).click();
    const d = dialog(page, "Colors");
    await d.getByRole("button", { name: "Pick from screen" }).click();
    await expect(d.getByLabel("Hex")).toHaveValue("#8844AA");
    await d.getByRole("button", { name: "OK" }).click();
    expect((await runAt(page, 6)).color).toBe("8844AA");
  });
});

test.describe("Highlight", () => {
  test("offers all sixteen of Word's colours and none", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await page.getByRole("button", { name: "Highlight" }).click();
    const names = ["Yellow", "Bright green", "Turquoise", "Pink", "Blue", "Red", "Dark blue", "Teal", "Green", "Violet", "Dark red", "Dark yellow", "Gray 50%", "Gray 25%", "Black", "White"];
    await expect(page.locator(".pop .swatch")).toHaveCount(16);
    for (const n of names) await expect(page.locator(".pop").getByRole("button", { name: n, exact: true })).toBeVisible();
    await page.locator(".pop").getByRole("button", { name: "Violet", exact: true }).click();
    expect(await runAt(page, 6)).toMatchObject({ highlight: "darkMagenta" });
    await expect(page.locator('.flow [data-p="1"] .s', { hasText: "beta" })).toHaveCSS("background-color", "rgb(128, 0, 128)");
    // The current one is marked, and No highlight takes it away.
    await page.getByRole("button", { name: "Highlight" }).click();
    await expect(page.locator(".pop").getByRole("button", { name: "Violet", exact: true })).toHaveAttribute("aria-pressed", "true");
    await page.locator(".pop").getByRole("button", { name: "No highlight" }).click();
    expect((await runAt(page, 6)).highlight).toBeUndefined();
  });
});

test.describe("Borders and shading for text", () => {
  test("a box and a shading colour go round the selected text only", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await chooseMenu(page, "Format", "Borders and shading…");
    const d = dialog(page, "Borders and shading");
    // Text is chosen when a part of a paragraph is selected; its sides are a box or nothing.
    await expect(d.getByRole("radio", { name: "Text" })).toHaveAttribute("aria-checked", "true");
    await expect(d.getByRole("button", { name: "Top", exact: true })).toBeDisabled();
    await d.getByRole("button", { name: "Box" }).click();
    await d.getByRole("radio", { name: "2¼ pt" }).click();
    await d.locator(".field", { hasText: "Border color" }).getByRole("button", { name: /^Red #FF0000$/ }).click();
    await d.locator(".field", { hasText: "Background color" }).getByRole("button", { name: /^Light green #92D050$/ }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await runAt(page, 6)).toMatchObject({ border: { val: "single", sz: 18, color: "FF0000", space: 1 }, shading: "92D050" });
    expect((await runAt(page, 0)).border).toBeUndefined();
    const para = await inPage(page, (w) => w.fe.editor.engine!.formatAt({ p: 1, o: 3 })!.para as Record<string, unknown>);
    expect(para.borders).toBeUndefined();
    // On the page the word is boxed and shaded.
    const word = page.locator('.flow [data-p="1"] .s', { hasText: "beta" });
    await expect(word).toHaveCSS("border-top-style", "solid");
    await expect(word).toHaveCSS("background-color", "rgb(146, 208, 80)");
    // The dialog opens with them again, and None takes the box away.
    await chooseMenu(page, "Format", "Borders and shading…");
    await expect(d.getByRole("radio", { name: "2¼ pt" })).toHaveAttribute("aria-checked", "true");
    await d.getByRole("button", { name: "None" }).first().click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await runAt(page, 6)).border).toBeUndefined();
    expect((await runAt(page, 6)).shading).toBe("92D050");
  });

  test("the same dialog still does paragraphs, from the Apply to switch", async ({ page }) => {
    await open(page);
    await page.locator('.flow [data-p="1"]').click();
    await chooseMenu(page, "Format", "Borders and shading…");
    const d = dialog(page, "Borders and shading");
    await expect(d.getByRole("radio", { name: "Paragraph" })).toHaveAttribute("aria-checked", "true");
    await d.getByRole("button", { name: "Bottom", exact: true }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    const para = await inPage(page, (w) => w.fe.editor.engine!.formatAt({ p: 1, o: 3 })!.para as { borders?: Record<string, unknown> });
    expect(Object.keys(para.borders ?? {})).toEqual(["bottom"]);
  });
});

test.describe("Other dialogs share the swatches", () => {
  test("Page color and Watermark offer More colors for any colour", async ({ page }) => {
    await open(page);
    // The page colour moved out of Page setup into Format, Page color… (with the fill effects).
    await chooseMenu(page, "Format", "Page color…");
    const d = dialog(page, "Page color");
    await d.getByRole("button", { name: "More colors…" }).click();
    await dialog(page, "Colors").getByLabel("Hex").fill("E8F0FE");
    await dialog(page, "Colors").getByRole("button", { name: "OK" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.pageSetup().pageColor ?? "")).toMatch(/E8F0FE/i);
    await chooseMenu(page, "Insert", "Page elements", "Watermark…");
    await expect(dialog(page, "Watermark").getByRole("button", { name: "More colors…" })).toBeVisible();
  });
});
