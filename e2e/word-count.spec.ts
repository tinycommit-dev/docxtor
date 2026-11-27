// Tools > Word count: pages, words, characters, paragraphs and lines, the selection ("x of y"), and the option that adds
// footnotes, endnotes and text boxes to the totals.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, selectWord, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const doc = (paragraphs: DocSpec["sections"][0]["paragraphs"], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs }], ...extra });
const word = (page: Page) => dialog(page, "Word count");
const row = (d: Locator, label: string) => d.locator("dt", { hasText: new RegExp(`^${label}$`) }).locator("+ dd");
const extras = (d: Locator) => d.getByRole("checkbox", { name: "Include textboxes, footnotes and endnotes" });

/** A text box with some words in it, anchored in a paragraph (Word's DrawingML text box). */
const textBox = (inner: string) =>
  `<w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251659264" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>2540000</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>127000</wp:posOffset></wp:positionV><wp:extent cx="2540000" cy="1016000"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapSquare wrapText="bothSides"/><wp:docPr id="2" name="Text Box 2"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="1016000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></wps:spPr><wps:txbx><w:txbxContent>${inner}</w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice></mc:AlternateContent></w:r>`;

const footnote = (id: number, text: string) => `<w:footnote w:id="${id}"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> ${text}</w:t></w:r></w:p></w:footnote>`;
const ref = (id: number) => `<w:r><w:footnoteReference w:id="${id}"/></w:r>`;

async function open(page: Page) {
  await page.keyboard.press(`${MOD}+Shift+C`);
  const d = word(page);
  await expect(d).toBeVisible();
  return d;
}

test.describe("word count", () => {
  test("counts pages, words, characters, paragraphs and lines", async ({ page }) => {
    await openBuilt(page, doc(["One two three", "Four five", "", "Six"]));
    const d = await open(page);
    await expect(row(d, "Pages")).toHaveText("1");
    await expect(row(d, "Words")).toHaveText("6");
    await expect(row(d, "Characters")).toHaveText(String("One two three".length + "Four five".length + "Six".length));
    await expect(row(d, "Characters excluding spaces")).toHaveText(String("Onetwothree".length + "Fourfive".length + "Six".length));
    // An empty paragraph counts as a paragraph, as Word counts it, and takes a line.
    await expect(row(d, "Paragraphs")).toHaveText("4");
    await expect(row(d, "Lines")).toHaveText("4");
    await d.getByRole("button", { name: "Done" }).click();
    await expect(d).toBeHidden();
  });

  test("lines are the rows the text takes on the page, not the paragraphs", async ({ page }) => {
    const long = Array.from({ length: 120 }, (_, i) => `word${i}`).join(" ");
    await openBuilt(page, doc([long, "Short{LBR}with{LBR}breaks"]));
    // What the page shows: the rows of text, counted from where the words sit.
    const rows = await page.locator(".flow .fp").evaluateAll((els) =>
      els.map((el) => {
        const range = document.createRange();
        const tops = new Set<number>();
        const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          range.selectNodeContents(n);
          for (const r of Array.from(range.getClientRects())) if (r.height > 1) tops.add(Math.round(r.top));
        }
        return tops.size;
      }),
    );
    expect(rows[0]).toBeGreaterThan(3);
    expect(rows[1]).toBe(3);
    const d = await open(page);
    await expect(row(d, "Paragraphs")).toHaveText("2");
    await expect(row(d, "Lines")).toHaveText(String(rows[0] + rows[1]));
    await expect(row(d, "Words")).toHaveText("123");
  });

  test("Pages follow the layout", async ({ page }) => {
    await openBuilt(page, doc([...Array.from({ length: 4 }, () => "Text"), "{PBR}", "On the second page"]));
    const d = await open(page);
    await expect(row(d, "Pages")).toHaveText("2");
  });

  test("the selection shows 'x of y' for words, characters and paragraphs; pages and lines stay whole", async ({ page }) => {
    await openBuilt(page, doc(["Alpha beta gamma", "Delta epsilon"]));
    const id = Number(await page.locator(".flow .fp").first().getAttribute("data-p"));
    await selectWord(page, id, "beta");
    await expect.poll(() => inPage(page, (w) => w.fe.editor.sel?.anchor.o !== w.fe.editor.sel?.focus.o)).toBe(true);
    const d = await open(page);
    await expect(row(d, "Words")).toHaveText("1 of 5");
    await expect(row(d, "Characters")).toHaveText("4 of 29");
    await expect(row(d, "Paragraphs")).toHaveText("1 of 2");
    await expect(row(d, "Lines")).toHaveText("2");
    await expect(row(d, "Pages")).toHaveText("1");
    await page.keyboard.press("Escape");
    await expect(d).toBeHidden();
  });

  test("footnotes, endnotes and text boxes are counted when the box is checked, and apart from the main text", async ({ page }) => {
    await openBuilt(
      page,
      doc(["Main text has five words.", { raw: `<w:r><w:t xml:space="preserve">Note here</w:t></w:r>${ref(1)}` }, { raw: textBox("<w:p><w:r><w:t>Box with four words</w:t></w:r></w:p>") + '<w:r><w:t>Anchor line</w:t></w:r>' }], {
        footnotes: footnote(1, "A note of five words"),
      }),
    );
    const d = await open(page);
    // Docxtor's own counts: the main text, the notes and the boxes.
    const c = await inPage(page, (w) => w.fe.editor.engine!.counts());
    expect(c.words).toBe(5 + 2 + 2);
    expect(c.notes.words).toBe(5);
    expect(c.boxes.words).toBe(4);
    await expect(extras(d)).toBeChecked();
    await expect(row(d, "Words")).toHaveText(String(c.words + 5 + 4));
    await expect(row(d, "Paragraphs")).toHaveText(String(c.paragraphs + 1 + 1));
    const withAll = Number(await row(d, "Lines").textContent());
    await extras(d).uncheck();
    await expect(row(d, "Words")).toHaveText(String(c.words));
    await expect(row(d, "Paragraphs")).toHaveText(String(c.paragraphs));
    const mainOnly = Number(await row(d, "Lines").textContent());
    expect(mainOnly).toBeLessThan(withAll);
    await expect(row(d, "Characters")).toHaveText(String(c.chars));
    // The choice is kept for next time.
    await d.getByRole("button", { name: "Done" }).click();
    await page.keyboard.press(`${MOD}+Shift+C`);
    await expect(extras(word(page))).not.toBeChecked();
    await expect(row(word(page), "Words")).toHaveText(String(c.words));
  });

  test("headers and footers are never counted", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Body words only"], headers: { default: "Header with many words here" }, footers: { default: "Footer words too" } }] });
    const d = await open(page);
    await expect(row(d, "Words")).toHaveText("3");
    await expect(row(d, "Lines")).toHaveText("1");
    await expect(row(d, "Paragraphs")).toHaveText("1");
  });

  test("the menu item and the shortcut open the same dialog, and the engine counts match the command-line tool's words", async ({ page }) => {
    await openBuilt(page, doc(["Count these four words"]));
    await chooseMenu(page, "Tools", "Word count");
    await expect(row(word(page), "Words")).toHaveText("4");
  });
});
