// Character styles in the style picker: listed with an "a" marker beside the paragraph styles, applied to the selected
// text as w:rStyle, linked styles (a heading and its character twin) that apply as paragraph or character styles by the
// selection, and Clear character style.
import { test, expect, inPage, selectWord, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;

const FANCY = '<w:style w:type="character" w:customStyle="1" w:styleId="Fancy"><w:name w:val="Fancy"/><w:basedOn w:val="DefaultParagraphFont"/><w:qFormat/><w:rPr><w:i/><w:color w:val="C00000"/><w:u w:val="dotted"/></w:rPr></w:style>';

async function open(page: Page, paragraphs: string[] = [run("", "alpha beta gamma")], styles = FANCY) {
  await openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })) }], styles });
}

const state = (page: Page, o = 7, p = 1) => inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos)!, { p, o });
const picker = (page: Page) => page.getByRole("button", { name: "Paragraph style" });
const item = (page: Page, name: string) => page.locator(".pop .style-tile").filter({ has: page.locator(".style-sample", { hasText: new RegExp(`^${name}$`) }) });

test.describe("character styles in the style picker", () => {
  test("they are in the gallery with a marker for each kind", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await picker(page).click();
    const mark = (name: string) => item(page, name).locator(".style-kind");
    await expect(mark("Normal")).toHaveText("¶");
    // A heading is linked to a character style of its own, as in Word.
    await expect(mark("Heading 1")).toHaveText("¶a");
    for (const c of ["Strong", "Emphasis", "Subtle Emphasis", "Intense Emphasis", "Subtle Reference", "Intense Reference", "Book Title", "Fancy"]) await expect(mark(c)).toHaveText("a");
    // The document's own character style is there.
    const names = await page.locator(".pop .style-sample").allTextContents();
    expect(names).toContain("Fancy");
    // A heading's character twin is not listed on its own once it exists: the heading stands for it.
    await item(page, "Heading 1").click();
    await picker(page).click();
    expect((await page.locator(".pop .style-sample").allTextContents()).filter((n) => /heading 1/i.test(n))).toHaveLength(1);
  });

  test("choosing one writes w:rStyle on the selected text only, and the style box names it", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    await picker(page).click();
    await item(page, "Fancy").click();
    expect(await state(page)).toMatchObject({ charStyle: "Fancy", run: { italic: true, color: "C00000", underline: "dotted" } });
    expect((await state(page, 1)).charStyle).toBeUndefined();
    await expect(page.locator('.flow [data-p="1"] .s', { hasText: "beta" })).toHaveCSS("font-style", "italic");
    await expect(picker(page).locator(".label")).toHaveText("Fancy");
    // It is the one marked in the list.
    await picker(page).click();
    await expect(item(page, "Fancy")).toHaveAttribute("aria-selected", "true");
  });

  test("Word's own character styles are added to the file when chosen", async ({ page }) => {
    await open(page);
    await selectWord(page, 1, "beta");
    expect(await inPage(page, (w) => w.fe.editor.engine!.styles().some((s) => s.id === "Strong"))).toBe(false);
    await picker(page).click();
    await item(page, "Strong").click();
    expect(await state(page)).toMatchObject({ charStyle: "Strong", run: { bold: true } });
    expect(await inPage(page, (w) => w.fe.editor.engine!.styles().find((s) => s.id === "Strong")?.kind)).toBe("character");
    await expect(page.locator('.flow [data-p="1"] .s', { hasText: "beta" })).toHaveCSS("font-weight", "700");
  });

  test("Clear character style takes it off the selected text", async ({ page }) => {
    await open(page, [run('<w:rStyle w:val="Fancy"/>', "styled words here")]);
    await selectWord(page, 1, "words");
    await picker(page).click();
    await page.locator(".pop").getByRole("button", { name: "Clear character style" }).click();
    expect((await state(page, 8)).charStyle).toBeUndefined();
    expect((await state(page, 8)).run.italic).toBeUndefined();
    // The rest still has it; and with no character style there is nothing to clear.
    expect((await state(page, 1)).charStyle).toBe("Fancy");
    await selectWord(page, 1, "words");
    await picker(page).click();
    await expect(page.locator(".pop").getByRole("button", { name: "Clear character style" })).toHaveCount(0);
  });

  test("a linked style is a character style on part of a paragraph and a paragraph style otherwise", async ({ page }) => {
    await open(page);
    // Part of the paragraph selected: the heading's character twin goes on that text.
    await selectWord(page, 1, "beta");
    await picker(page).click();
    await item(page, "Heading 1").click();
    const part = await state(page);
    expect(part.charStyle).toBe("Heading1Char");
    expect(part.style ?? "Normal").not.toMatch(/Heading/);
    expect((await state(page, 1)).charStyle).toBeUndefined();
    // Only a caret: the whole paragraph takes the style.
    await page.locator('.flow [data-p="1"]').click();
    await picker(page).click();
    await item(page, "Heading 1").click();
    expect((await state(page, 1)).style).toMatch(/Heading ?1/);
  });

  test("the whole paragraph selected makes it a paragraph style too", async ({ page }) => {
    await open(page);
    await page.locator('.flow [data-p="1"]').click();
    await page.keyboard.press("Home");
    await page.keyboard.press("Shift+End");
    await picker(page).click();
    await item(page, "Heading 2").click();
    const s = await state(page, 3);
    expect(s.style).toMatch(/Heading ?2/);
    expect(s.charStyle).toBeUndefined();
  });

  test("typing after choosing one at a caret has the style", async ({ page }) => {
    await open(page);
    await page.locator('.flow [data-p="1"]').click();
    await page.keyboard.press("End");
    await picker(page).click();
    await item(page, "Fancy").click();
    await page.keyboard.type("new");
    expect(await state(page, 18)).toMatchObject({ charStyle: "Fancy", run: { italic: true } });
    expect((await state(page, 2)).charStyle).toBeUndefined();
  });

  test("the assistant's format_text can name a character style", async ({ page }) => {
    await open(page);
    const done = await inPage(page, (w) => {
      w.fe.editor.exec({ op: "format_text", from: { p: 1, o: 6 }, to: { p: 1, o: 10 }, format: { style: "Emphasis" } });
      return w.fe.editor.engine!.formatAt({ p: 1, o: 8 })!.charStyle;
    });
    expect(done).toBe("Emphasis");
  });
});

test("the style picker (looked at by eye)", async ({ page }, testInfo) => {
  await open(page, [run('<w:rStyle w:val="Fancy"/>', "alpha beta gamma")]);
  await selectWord(page, 1, "beta");
  await picker(page).click();
  const path = testInfo.outputPath("style-picker.png");
  await page.locator(".pop").screenshot({ path });
  await testInfo.attach("style-picker", { path, contentType: "image/png" });
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Highlight" }).click();
  const hl = testInfo.outputPath("highlight.png");
  await page.locator(".pop").screenshot({ path: hl });
  await testInfo.attach("highlight", { path: hl, contentType: "image/png" });
});
