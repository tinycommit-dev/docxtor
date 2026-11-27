// Change case (Format, Text, Change case, and Shift+F3): the five cases, as one undo step that keeps the formatting,
// the word at the caret, the paragraph's language, and the Shift+F3 cycle.
import { test, expect, chooseMenu, paraText, selectWord, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

const MOD = "ControlOrMeta";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;

async function open(page: Page, paragraphs: string[]) {
  await openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })) }] });
}

/** Select the whole of a paragraph (through the editor: Home and End don't move the caret on every platform). */
async function selectLine(page: Page, id = 1) {
  await page.locator(`.flow [data-p="${id}"]`).click();
  await page.evaluate((p) => {
    const ed = (window as unknown as { docxtor: { fe: { editor: { engine: { textRange(a: unknown, b: unknown): string }; select(s: unknown): void } } } }).docxtor.fe.editor;
    ed.select({ anchor: { p, o: 0 }, focus: { p, o: ed.engine.textRange({ p, o: 0 }, { p, o: 1_000_000 }).length } });
  }, id);
}

test.describe("Change case", () => {
  test("the five cases are in Format, Text, Change case and change the selected text", async ({ page }) => {
    await open(page, [run("", "hello WORLD. this is IT")]);
    const cases: [string, string][] = [
      ["Sentence case", "Hello world. This is it"],
      ["UPPERCASE", "HELLO WORLD. THIS IS IT"],
      ["lowercase", "hello world. this is it"],
      ["Capitalize Each Word", "Hello World. This Is It"],
      ["tOGGLE cASE", "hELLO wORLD. tHIS iS iT"],
    ];
    for (const [name, want] of cases) {
      await selectLine(page);
      await chooseMenu(page, "Format", "Text", "Change case", name);
      await expect.poll(() => paraText(page, 1)).toBe(want);
    }
  });

  test("it keeps the formatting of every run and is one undo step", async ({ page }) => {
    await open(page, [run("<w:b/>", "bold part ") + run("<w:i/>", "italic part")]);
    await selectLine(page);
    await chooseMenu(page, "Format", "Text", "Change case", "UPPERCASE");
    await expect.poll(() => paraText(page, 1)).toBe("BOLD PART ITALIC PART");
    const spans = await page.locator('.flow [data-p="1"] .s').evaluateAll((els) => els.map((e) => [e.textContent, getComputedStyle(e).fontWeight, getComputedStyle(e).fontStyle]));
    expect(spans).toEqual([
      ["BOLD PART ", "700", "normal"],
      ["ITALIC PART", "400", "italic"],
    ]);
    await page.keyboard.press(`${MOD}+z`);
    await expect.poll(() => paraText(page, 1)).toBe("bold part italic part");
    await page.keyboard.press(`${MOD}+Shift+z`);
    await expect.poll(() => paraText(page, 1)).toBe("BOLD PART ITALIC PART");
  });

  test("with only a caret it changes the word the caret is in", async ({ page }) => {
    await open(page, [run("", "one two three")]);
    await selectWord(page, 1, "two");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowRight");
    await chooseMenu(page, "Format", "Text", "Change case", "UPPERCASE");
    await expect.poll(() => paraText(page, 1)).toBe("one TWO three");
  });

  test("it works across paragraphs and leaves the rest alone", async ({ page }) => {
    await open(page, [run("", "first one"), run("", "second one"), run("", "third one")]);
    await page.locator('.flow [data-p="1"]').click();
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { select(s: unknown): void } } } }).docxtor.fe.editor.select({ anchor: { p: 1, o: 0 }, focus: { p: 2, o: 10 } }));
    await chooseMenu(page, "Format", "Text", "Change case", "UPPERCASE");
    await expect.poll(() => paraText(page, 2)).toBe("SECOND ONE");
    expect(await paraText(page, 1)).toBe("FIRST ONE");
    expect(await paraText(page, 3)).toBe("third one");
  });

  test("a Turkish paragraph has a dotted and a dotless i", async ({ page }) => {
    await open(page, [run('<w:lang w:val="tr-TR"/>', "istanbul ılık"), run('<w:lang w:val="en-US"/>', "istanbul ILIK")]);
    await page.locator('.flow [data-p="1"]').click();
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { select(s: unknown): void } } } }).docxtor.fe.editor.select({ anchor: { p: 1, o: 0 }, focus: { p: 2, o: 13 } }));
    await chooseMenu(page, "Format", "Text", "Change case", "UPPERCASE");
    await expect.poll(() => paraText(page, 1)).toBe("İSTANBUL ILIK");
    expect(await paraText(page, 2)).toBe("ISTANBUL ILIK");
    await page.keyboard.press("Shift+F3");
    await expect.poll(() => paraText(page, 1)).toBe("istanbul ılık");
    expect(await paraText(page, 2)).toBe("istanbul ilik");
  });

  test("Shift+F3 goes from lowercase to each word capitalised, to uppercase, and round again", async ({ page }) => {
    await open(page, [run("", "hello big world")]);
    await selectLine(page);
    for (const want of ["Hello Big World", "HELLO BIG WORLD", "hello big world", "Hello Big World"]) {
      await page.keyboard.press("Shift+F3");
      await expect.poll(() => paraText(page, 1)).toBe(want);
    }
  });

  test("the commands are in the palette, and the assistant can run them", async ({ page }) => {
    await open(page, [run("", "hello")]);
    const info = await page.evaluate(() => {
      const w = (window as unknown as { docxtor: { fe: { extensions: { commands: { list(): { id: string; ai?: boolean; keybinding?: string | string[] }[] } } } } }).docxtor;
      const all = w.fe.extensions.commands.list();
      return ["format.case.sentence", "format.case.lower", "format.case.upper", "format.case.title", "format.case.toggle", "format.cycleCase"].map((id) => all.find((c) => c.id === id));
    });
    expect(info.every(Boolean)).toBe(true);
    expect(info[5]?.keybinding).toBe("Shift+F3");
    expect(info.slice(0, 5).every((c) => c?.ai)).toBe(true);
  });
});
