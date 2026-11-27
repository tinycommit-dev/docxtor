// The Font dialog (Format, Font…, ⌘D, and the right-click menu): what it shows, what it writes (only what changed,
// as one undo step), Set as default, and its preview.
import { test, expect, chooseMenu, contextItem, dialog, inPage, selectWord, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

const MOD = "ControlOrMeta";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;

async function open(page: Page, paragraphs: string[] = [run("", "alpha beta gamma")]) {
  await openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })) }] });
}

/** The formatting at the start of a word (the model's view). */
const runAt = (page: Page, o: number, p = 1) => inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos)!.run as Record<string, any>, { p, o: o + 1 });
const xmlOf = (page: Page) =>
  inPage(page, (w) => {
    const bytes = w.fe.editor.engine!.save();
    return new TextDecoder().decode(bytes);
  });

const field = (page: Page, id: string) => page.locator(`#${id}`);

async function openFont(page: Page) {
  await chooseMenu(page, "Format", "Font…");
  await expect(dialog(page, "Font")).toBeVisible();
}

test.describe("Font dialog", () => {
  test("opens from the Format menu, ⌘D and the right-click menu, showing what the text has", async ({ page }) => {
    await open(page, [run('<w:rFonts w:ascii="Georgia" w:hAnsi="Georgia"/><w:b/><w:sz w:val="28"/><w:u w:val="wave" w:color="C00000"/><w:strike/><w:smallCaps/><w:spacing w:val="20"/><w:position w:val="-4"/><w:w w:val="120"/><w:kern w:val="28"/><w14:ligatures w14:val="all"/>', "alpha beta gamma")]);
    await selectWord(page, 1, "beta");
    await openFont(page);
    await expect(field(page, "fd-font")).toHaveValue("Georgia");
    await expect(field(page, "fd-style")).toHaveValue("bold");
    await expect(field(page, "fd-size")).toHaveValue("14");
    await expect(field(page, "fd-underline")).toHaveValue("wave");
    await expect(field(page, "fd-underline-color")).toContainText("#C00000");
    await expect(field(page, "fd-strike")).toBeChecked();
    await expect(field(page, "fd-small-caps")).toBeChecked();
    await expect(field(page, "fd-dstrike")).not.toBeChecked();
    await dialog(page, "Font").getByRole("tab", { name: "Advanced" }).click();
    await expect(field(page, "fd-scale")).toHaveValue("120");
    await expect(field(page, "fd-spacing")).toHaveValue("expanded");
    await expect(field(page, "fd-spacing-by")).toHaveValue("1");
    await expect(field(page, "fd-position")).toHaveValue("lowered");
    await expect(field(page, "fd-position-by")).toHaveValue("2");
    await expect(field(page, "fd-kern")).toBeChecked();
    await expect(field(page, "fd-kern-from")).toHaveValue("14");
    await expect(field(page, "fd-ligatures")).toHaveValue("all");
    await dialog(page, "Font").getByRole("button", { name: "Cancel" }).click();
    await expect(dialog(page, "Font")).toBeHidden();

    // ⌘D and the right-click menu open the same dialog.
    await page.keyboard.press(`${MOD}+d`);
    await expect(dialog(page, "Font")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog(page, "Font")).toBeHidden();
    await page.locator('.flow [data-p="1"]').click({ button: "right" });
    await contextItem(page, "Font…").click();
    await expect(dialog(page, "Font")).toBeVisible();
  });

  test("writes only what changed, and undo takes it back in one step", async ({ page }) => {
    await open(page, [run('<w:b/><w:color w:val="1F4E79"/>', "alpha beta gamma")]);
    await selectWord(page, 1, "beta");
    await openFont(page);
    await field(page, "fd-underline").selectOption("dotDash");
    await field(page, "fd-dstrike").check();
    await field(page, "fd-hidden").check();
    await field(page, "fd-emboss").check();
    await field(page, "fd-size").fill("16");
    await dialog(page, "Font").getByRole("tab", { name: "Advanced" }).click();
    await field(page, "fd-scale").fill("90");
    await field(page, "fd-spacing").selectOption("condensed");
    await field(page, "fd-spacing-by").fill("0.5");
    await field(page, "fd-position").selectOption("raised");
    await field(page, "fd-position-by").fill("3");
    await field(page, "fd-kern").check();
    await field(page, "fd-kern-from").fill("12");
    await field(page, "fd-ligatures").selectOption("standardContextual");
    await field(page, "fd-num-form").selectOption("oldStyle");
    await field(page, "fd-style-set").selectOption("5");
    await field(page, "fd-cntxt").check();
    await dialog(page, "Font").getByRole("button", { name: "OK" }).click();
    await expect(dialog(page, "Font")).toBeHidden();

    const f = await runAt(page, 6);
    expect(f).toMatchObject({ bold: true, color: "1F4E79", size: 16, underline: "dotDash", dstrike: true, vanish: true, emboss: true, scale: 90, spacing: -10, position: 6, kern: 24 });
    expect(f.fx).toMatchObject({ ligatures: "standardContextual", numForm: "oldStyle", styleSets: [5], cntxtAlts: true });
    // Only the word that was selected changed; the rest keeps what it had (and what the dialog did not touch stays).
    expect((await runAt(page, 0)).underline).toBeUndefined();
    const xml = await xmlOf(page);
    expect(xml).toContain("w14:ligatures");

    await page.keyboard.press(`${MOD}+z`);
    const back = await runAt(page, 6);
    expect(back.underline).toBeUndefined();
    expect(back.scale).toBeUndefined();
    expect(back.bold).toBe(true);
  });

  test("a colour is picked from the palette, with its theme colour written beside it", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await openFont(page);
    await field(page, "fd-color").click();
    await page.getByRole("button", { name: "Accent 2, darker 25%" }).click();
    await field(page, "fd-underline").selectOption("single");
    await field(page, "fd-underline-color").click();
    await page.getByRole("button", { name: /^Red #FF0000$/ }).click();
    await field(page, "fd-shading").click();
    await page.getByRole("button", { name: /^Yellow #FFFF00$/ }).click();
    await dialog(page, "Font").getByRole("button", { name: "OK" }).click();
    const f = await runAt(page, 6);
    // Accent 2 (ED7D31) darker 25% is shade BF.
    expect(f).toMatchObject({ color: "C55A11", colorTheme: { name: "accent2", shade: "BF" }, underlineColor: "FF0000", shading: "FFFF00" });
  });

  test("the preview draws the settings", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await openFont(page);
    const sample = page.locator(".fd-sample");
    await field(page, "fd-small-caps").check();
    await field(page, "fd-underline").selectOption("double");
    await expect(sample.locator("span").first()).toHaveCSS("font-variant-caps", "small-caps");
    await expect(sample.locator("span").first()).toHaveCSS("text-decoration-style", "double");
    await field(page, "fd-style").selectOption("boldItalic");
    await expect(sample.locator("span").first()).toHaveCSS("font-weight", "700");
    await expect(sample.locator("span").first()).toHaveCSS("font-style", "italic");
  });

  test("effects that exclude each other switch each other off", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await openFont(page);
    await field(page, "fd-sup").check();
    await field(page, "fd-sub").check();
    await expect(field(page, "fd-sup")).not.toBeChecked();
    await field(page, "fd-emboss").check();
    await field(page, "fd-imprint").check();
    await expect(field(page, "fd-emboss")).not.toBeChecked();
    await field(page, "fd-strike").check();
    await field(page, "fd-dstrike").check();
    await expect(field(page, "fd-strike")).not.toBeChecked();
  });

  test("Set as default writes the changes into the document defaults, in one undo step", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await openFont(page);
    await field(page, "fd-font").selectOption("Georgia");
    await field(page, "fd-size").fill("14");
    await dialog(page, "Font").getByRole("button", { name: "Set as default" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Set as default" }).click();
    await expect(dialog(page, "Font")).toBeHidden();
    // Text that doesn't say otherwise now has the defaults.
    expect(await runAt(page, 0)).toMatchObject({ font: "Georgia", size: 14 });
    // One undo step takes the change back, and redo brings it again.
    await page.keyboard.press(`${MOD}+z`);
    expect(await runAt(page, 0)).toMatchObject({ size: 10 });
    await page.keyboard.press(`${MOD}+Shift+z`);
    expect(await runAt(page, 0)).toMatchObject({ font: "Georgia", size: 14 });
    // They are in the saved file: a copy opened from its bytes has them too.
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } }; open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(w.fe.editor.engine.save(), "again.docx");
    });
    await expect(page.locator('.flow [data-p="1"]')).toContainText("alpha beta gamma");
    expect(await runAt(page, 0)).toMatchObject({ font: "Georgia", size: 14 });
  });

  test("with only a caret, the settings apply to the next typed text", async ({ page }) => {
    await open(page);
    await page.locator('.flow [data-p="1"]').click();
    await page.keyboard.press("End");
    await openFont(page);
    await field(page, "fd-underline").selectOption("thick");
    await field(page, "fd-style").selectOption("bold");
    await dialog(page, "Font").getByRole("button", { name: "OK" }).click();
    await page.keyboard.type("!!");
    const f = await runAt(page, 16);
    expect(f).toMatchObject({ bold: true, underline: "thick" });
    expect((await runAt(page, 0)).bold).toBeUndefined();
  });
});

test("the dialogs and the palette (looked at by eye)", async ({ page }, testInfo) => {
  await open(page);
  await selectWord(page, 1, "beta");
  await openFont(page);
  await field(page, "fd-underline").selectOption("wave");
  await field(page, "fd-small-caps").check();
  const shot = async (name: string, loc = page.locator(".dialog")) => {
    const path = testInfo.outputPath(`${name}.png`);
    await loc.first().screenshot({ path });
    await testInfo.attach(name, { path, contentType: "image/png" });
  };
  await shot("font-tab");
  await dialog(page, "Font").getByRole("tab", { name: "Advanced" }).click();
  await shot("font-advanced");
  await dialog(page, "Font").getByRole("tab", { name: "Font" }).click();
  await field(page, "fd-color").click();
  await shot("palette", page.locator(".pop"));
  await page.getByRole("button", { name: "More colors…" }).click();
  await shot("more-colors", page.locator(".colors-dialog"));
});
