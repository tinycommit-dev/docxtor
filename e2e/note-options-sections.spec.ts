// The Footnote and endnote options dialog, past the number format: restarting numbers in each section or on each page, where the notes go,
// changing one section's notes only, and a note with a mark of its own.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup() as unknown as { notes: { label: string; id: string }[]; endnotes: { label: string }[]; footnotePr: Record<string, unknown>; endnotePr: Record<string, unknown>; sections: { footnotePr: Record<string, unknown> }[] });
const open = async (page: Page) => {
  await chooseMenu(page, "Insert", "Page elements", "Footnote and endnote options…");
  const d = dialog(page, "Footnote and endnote options");
  await expect(d).toBeVisible();
  return d;
};
const note = (id: number, text: string) => `<w:footnote w:id="${id}"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> ${text}</w:t></w:r></w:p></w:footnote>`;
const ref = (id: number) => `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="${id}"/></w:r>`;
const STYLES = '<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>';
const marks = (page: Page) => page.locator(".flow .atom.note:not(.endnote)").allTextContents();

test.describe("note options", () => {
  const twoSections = (page: Page) =>
    openBuilt(page, {
      styles: STYLES,
      footnotes: [1, 2, 3].map((i) => note(i, `Note ${i}.`)).join(""),
      sections: [
        { paragraphs: [{ text: "Alpha one", xml: ref(1) }, { text: "Alpha two", xml: ref(2) }] },
        { paragraphs: [{ text: "Bravo one", xml: ref(3) }] },
      ],
    });

  test("numbering restarts in each section", async ({ page }) => {
    await twoSections(page);
    expect(await marks(page)).toEqual(["1", "2", "3"]);
    const d = await open(page);
    await d.getByLabel("Numbering").nth(0).selectOption("eachSect");
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await marks(page)).toEqual(["1", "2", "1"]);
    expect((await setup(page)).footnotePr).toMatchObject({ restart: "eachSect" });
    // The dialog reads it back, and undo takes it away.
    const again = await open(page);
    await expect(again.getByLabel("Numbering").nth(0)).toHaveValue("eachSect");
    await again.getByRole("button", { name: "Cancel" }).click();
    await page.keyboard.press("ControlOrMeta+Z");
    expect(await marks(page)).toEqual(["1", "2", "3"]);
  });

  test("numbering restarts on each page", async ({ page }) => {
    await openBuilt(page, {
      styles: STYLES,
      footnotes: [1, 2, 3].map((i) => note(i, `Note ${i}.`)).join(""),
      sections: [{ paragraphs: [{ text: "One", xml: ref(1) }, "{PBR}", { text: "Two", xml: ref(2) }, { text: "Three", xml: ref(3) }] }],
    });
    const d = await open(page);
    await d.getByLabel("Numbering").nth(0).selectOption("eachPage");
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await marks(page)).toEqual(["1", "1", "2"]);
    expect(await page.locator(".sheet").nth(1).locator(".notes .atom.note").allTextContents()).toEqual(["1", "2"]);
  });

  test("where the notes go: below the text, and endnotes at the end of the section", async ({ page }) => {
    await twoSections(page);
    const d = await open(page);
    await d.getByLabel("Position").nth(0).selectOption("beneathText");
    await d.getByLabel("Position").nth(1).selectOption("sectEnd");
    await d.getByRole("button", { name: "Apply" }).click();
    const s = await setup(page);
    expect(s.footnotePr).toMatchObject({ pos: "beneathText" });
    expect(s.endnotePr).toMatchObject({ pos: "sectEnd" });
    // Below the text: the notes sit right under the one line of text, not at the foot of the page.
    const top = await page.locator(".sheet").first().locator(".notes").evaluate((el) => el.getBoundingClientRect().top - el.parentElement!.getBoundingClientRect().top);
    expect(top).toBeLessThan(250);
  });

  test("one section's notes can be numbered another way", async ({ page }) => {
    await twoSections(page);
    await page.locator(".flow .fp").nth(2).click();
    const d = await open(page);
    await d.getByLabel("Apply changes to").selectOption("section");
    await d.getByLabel("Number format").nth(0).selectOption("lowerLetter");
    await d.getByRole("button", { name: "Apply" }).click();
    const s = await setup(page);
    expect(s.sections[1].footnotePr).toMatchObject({ fmt: "lowerLetter" });
    expect(s.sections[0].footnotePr).toMatchObject({ fmt: "decimal" });
    expect(await marks(page)).toEqual(["1", "2", "c"]);
  });

  test("a note with a mark of its own", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Some text to note"] }] });
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("End");
    const d = await open(page);
    const insert = d.getByRole("button", { name: "Insert footnote with this mark" });
    await expect(insert).toBeDisabled();
    await d.getByRole("button", { name: "†" }).first().click();
    await expect(d.getByLabel("Custom mark for a footnote")).toHaveValue("†");
    await insert.click();
    await expect(d).toBeHidden();
    // The reference carries no number; the mark is text after it, and the note starts with the mark too.
    expect(await marks(page)).toEqual([""]);
    await expect(page.locator(".flow .fp").first()).toContainText("Some text to note†");
    const s = await setup(page);
    expect(s.notes).toHaveLength(1);
    expect(s.notes[0].label).toBe("");
    await expect(page.locator(".sheet").first().locator(".notes")).toContainText("†");
  });
});
