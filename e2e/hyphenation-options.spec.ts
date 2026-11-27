// Hyphenation: the options (automatic, words in capitals, the zone, the limit of hyphens in a row), hyphenating by hand, and the optional
// and non-breaking hyphens (Insert, with ⌘- and ⌘⇧- as in Word).
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, LINE_START, menuItem, openMenu, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup() as unknown as Record<string, unknown>);
const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());

test.describe("hyphenation options", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["A paragraph with some extraordinarily lengthy vocabulary"] }] });
  });

  test("automatic hyphenation, the zone, the limit and words in capitals are written", async ({ page }) => {
    await chooseMenu(page, "Tools", "Hyphenation options…");
    const d = dialog(page, "Hyphenation options");
    await expect(d.getByLabel("Automatically hyphenate document")).not.toBeChecked();
    await expect(d.getByLabel("Hyphenate words in CAPS")).toBeChecked();
    await d.getByLabel("Automatically hyphenate document").check();
    await d.getByLabel("Hyphenate words in CAPS").uncheck();
    await d.getByRole("spinbutton", { name: /Hyphenation zone/ }).fill("0.5");
    await d.getByLabel("Limit consecutive hyphens to").fill("2");
    await d.getByRole("button", { name: "OK" }).click();
    const s = await setup(page);
    expect(s).toMatchObject({ hyphenation: true, doNotHyphenateCaps: true, hyphenationZone: 720, consecutiveHyphenLimit: 2 });
    await expect(page.locator(".docxtor .stage").first()).toHaveCSS("hyphens", "auto");
    // The dialog reads it back.
    await chooseMenu(page, "Tools", "Hyphenation options…");
    const again = dialog(page, "Hyphenation options");
    await expect(again.getByLabel("Limit consecutive hyphens to")).toHaveValue("2");
    await expect(again.getByRole("spinbutton", { name: /Hyphenation zone/ })).toHaveValue("0.5");
    await again.getByRole("button", { name: "Cancel" }).click();
    // One undo for all of it.
    await page.keyboard.press(`${MOD}+Z`);
    expect((await setup(page)).hyphenationZone).toBeUndefined();
  });

  test("Insert, Optional hyphen and Non-breaking hyphen, and their shortcuts", async ({ page }) => {
    const p = page.locator(".flow .fp").first();
    await p.click();
    await page.keyboard.press(LINE_START);
    for (let i = 0; i < 4; i++) await page.keyboard.press("ArrowRight");
    await openMenu(page, "Insert");
    await menuItem(page, "Optional hyphen").click();
    expect(await text(page)).toContain("A pa­ragraph");
    await page.keyboard.press(`${MOD}+-`);
    expect(await text(page)).toContain("A pa­­ragraph");
    await page.keyboard.press(`${MOD}+Shift+-`);
    expect(await text(page)).toContain("‑");
    await page.keyboard.press(`${MOD}+Z`);
    expect(await text(page)).not.toContain("‑");
  });
});

test.describe("hyphenate manually", () => {
  test("steps through the words that start a line, offers where each breaks, and puts in the hyphens that are agreed to", async ({ page }) => {
    // A narrow column so that long words land at the start of lines.
    await openBuilt(page, {
      sections: [{ margins: [1440, 4400, 1440, 1440], paragraphs: ["Considerable administration notwithstanding extraordinary documentation requirements regarding international collaboration"] }],
    });
    await page.locator(".flow .fp").first().click();
    // From the start of the paragraph.
    await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { range(): [{ p: number }]; select(p: { p: number; o: number }): void };
      e.select({ p: e.range()[0].p, o: 0 });
    });
    await chooseMenu(page, "Tools", "Hyphenate manually…");
    const panel = page.getByRole("dialog", { name: "Hyphenate manually" });
    await expect(panel).toBeVisible();
    await expect(panel.locator(".hy-word")).toContainText("-");
    const first = (await panel.locator(".hy-word").textContent())!.replace("-", "");
    // The word is selected in the text, so the person sees which one it is.
    expect(await inPage(page, (w) => w.fe.editor.selectedText())).toBe(first);
    // Moving the break changes where the hyphen is offered.
    const before = await panel.locator(".hy-word").textContent();
    if (await panel.getByRole("button", { name: "Move the hyphen right" }).isEnabled()) {
      await panel.getByRole("button", { name: "Move the hyphen right" }).click();
      expect(await panel.locator(".hy-word").textContent()).not.toBe(before);
    }
    await panel.getByRole("button", { name: "Hyphenate" }).click();
    expect(await text(page)).toContain("­");
    // Skip moves to the next word without a hyphen; Done closes the panel.
    const hyphens = (await text(page)).split("­").length - 1;
    if (await panel.getByRole("button", { name: "Skip" }).isEnabled()) await panel.getByRole("button", { name: "Skip" }).click();
    expect((await text(page)).split("­").length - 1).toBe(hyphens);
    await panel.getByRole("button", { name: "Done" }).click();
    await expect(panel).toHaveCount(0);
  });
});
