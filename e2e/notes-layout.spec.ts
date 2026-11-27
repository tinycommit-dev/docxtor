// Notes and page numbers by section: numbering that starts again in each section or on each page, footnotes beneath the
// text, endnotes at the end of a section, the separators and continuation notices the notes part gives, notes that don't
// fit and go on to the next page, and chapter page numbers (2-1, 2-2).
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { expect, inPage, test, type Page } from "./helpers";

const near = (a: number, b: number, tolerance = 1.5) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);
const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
const lines = (n: number, prefix = "Line") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);

const note = (id: number, text: string) => `<w:footnote w:id="${id}"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> ${text}</w:t></w:r></w:p></w:footnote>`;
const endnote = (id: number, text: string) => `<w:endnote w:id="${id}"><w:p><w:pPr><w:pStyle w:val="EndnoteText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="EndnoteReference"/></w:rPr><w:endnoteRef/></w:r><w:r><w:t xml:space="preserve"> ${text}</w:t></w:r></w:p></w:endnote>`;
const ref = (id: number) => `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="${id}"/></w:r>`;
const eref = (id: number) => `<w:r><w:rPr><w:rStyle w:val="EndnoteReference"/></w:rPr><w:endnoteReference w:id="${id}"/></w:r>`;
/** A paragraph with a note reference after its text. */
const withNote = (text: string, id: number) => ({ text, xml: ref(id) });
const STYLES = '<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:pPr><w:spacing w:after="0"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:qFormat/><w:pPr><w:keepNext/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>';

/** The marks in the text (what the reader sees as note numbers), in order. */
const marks = (page: Page, kind: "note" | "endnote" = "note") => page.locator(kind === "note" ? ".flow .atom.note:not(.endnote)" : ".flow .atom.note.endnote").allTextContents();
const sheetNotes = (page: Page, k: number) => sheet(page, k).locator(".notes");
const noteLabels = async (page: Page, k: number) => sheetNotes(page, k).locator(".atom.note").allTextContents();

test.describe("note numbering", () => {
  test("numbering that restarts in each section", async ({ page }) => {
    await openBuilt(page, {
      styles: STYLES,
      footnotes: [1, 2, 3].map((i) => note(i, `Note ${i}.`)).join(""),
      sections: [
        { paragraphs: [withNote("Alpha one", 1), withNote("Alpha two", 2)], xml: { notes: '<w:footnotePr><w:numRestart w:val="eachSect"/></w:footnotePr>' } },
        { paragraphs: [withNote("Bravo one", 3)], xml: { notes: '<w:footnotePr><w:numRestart w:val="eachSect"/></w:footnotePr>' } },
      ],
    });
    expect(await marks(page)).toEqual(["1", "2", "1"]);
    expect(await noteLabels(page, 0)).toEqual(["1", "2"]);
    expect(await noteLabels(page, 1)).toEqual(["1"]);
  });

  test("a section can number its notes another way", async ({ page }) => {
    await openBuilt(page, {
      styles: STYLES,
      footnotes: [1, 2, 3].map((i) => note(i, `Note ${i}.`)).join(""),
      sections: [
        { paragraphs: [withNote("Alpha one", 1), withNote("Alpha two", 2)] },
        { paragraphs: [withNote("Bravo one", 3)], xml: { notes: '<w:footnotePr><w:numFmt w:val="lowerRoman"/><w:numStart w:val="4"/></w:footnotePr>' } },
      ],
    });
    // Numbering carries on across the sections (it was not asked to restart), in the format each section gives.
    expect(await marks(page)).toEqual(["1", "2", "iii"]);
  });

  test("numbering that restarts on each page", async ({ page }) => {
    await openBuilt(page, {
      styles: STYLES,
      settings: '<w:footnotePr><w:numRestart w:val="eachPage"/></w:footnotePr>',
      footnotes: [1, 2, 3].map((i) => note(i, `Note ${i}.`)).join(""),
      sections: [{ paragraphs: [withNote("First page one", 1), withNote("First page two", 2), "{PBR}", withNote("Second page", 3)] }],
    });
    await expect(page.locator(".sheet")).toHaveCount(2);
    expect(await marks(page)).toEqual(["1", "2", "1"]);
    expect(await noteLabels(page, 0)).toEqual(["1", "2"]);
    expect(await noteLabels(page, 1)).toEqual(["1"]);
  });
});

test.describe("note position", () => {
  test("footnotes at the foot of the page, or right under the text", async ({ page }) => {
    const spec = (pos: string): Parameters<typeof openBuilt>[1] => ({
      styles: STYLES,
      footnotes: note(1, "A short note."),
      sections: [{ paragraphs: [withNote("The text is short", 1)], xml: { notes: pos ? `<w:footnotePr><w:pos w:val="${pos}"/></w:footnotePr>` : "" } }],
    });
    await openBuilt(page, spec(""));
    const bottom = await sheetNotes(page, 0).evaluate((el) => el.getBoundingClientRect().bottom - el.parentElement!.getBoundingClientRect().top);
    // At the foot: the bottom margin is an inch.
    near(bottom, 1056 - 96, 1.5);
    await openBuilt(page, spec("beneathText"));
    const top = await sheetNotes(page, 0).evaluate((el) => el.getBoundingClientRect().top - el.parentElement!.getBoundingClientRect().top);
    const text = await inPage(page, () => 0);
    void text;
    // Beneath the text: just below the one line of text at the top of the page.
    expect(top).toBeLessThan(200);
    expect(top).toBeGreaterThan(96);
    await page.screenshot({ path: "test-results/notes-beneath-text.png" });
  });

  test("endnotes at the end of the section, or of the document", async ({ page }) => {
    const sections = (pos: string): SectionSpec[] => [
      { paragraphs: [{ text: "First section", xml: eref(1) }], xml: { notes: pos ? `<w:endnotePr><w:pos w:val="${pos}"/></w:endnotePr>` : "" } },
      { paragraphs: [{ text: "Second section", xml: eref(2) }], xml: { notes: pos ? `<w:endnotePr><w:pos w:val="${pos}"/></w:endnotePr>` : "" } },
    ];
    const notes = endnote(1, "End note one.") + endnote(2, "End note two.");
    await openBuilt(page, { styles: STYLES, endnotes: notes, sections: sections("") });
    // Both at the end of the document: on the last page.
    await expect(page.locator(".sheet")).toHaveCount(2);
    await expect(sheet(page, 0).locator(".endnotes")).toHaveCount(0);
    await expect(sheet(page, 1).locator(".endnotes .atom.note")).toHaveCount(2);
    await openBuilt(page, { styles: STYLES, endnotes: notes, sections: sections("sectEnd") });
    await expect(page.locator(".sheet")).toHaveCount(2);
    await expect(sheet(page, 0).locator(".endnotes .atom.note")).toHaveCount(1);
    await expect(sheet(page, 1).locator(".endnotes .atom.note")).toHaveCount(1);
    expect(await sheet(page, 0).locator(".endnotes").textContent()).toContain("End note one.");
    expect(await sheet(page, 1).locator(".endnotes").textContent()).toContain("End note two.");
  });
});

test.describe("separators and notes that go on", () => {
  test("a custom separator and a continuation notice replace Word's", async ({ page }) => {
    const own =
      '<w:footnote w:type="separator" w:id="-1"><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>* * *</w:t></w:r></w:p></w:footnote>' +
      '<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>' +
      '<w:footnote w:type="continuationNotice" w:id="1"><w:p><w:r><w:t>Notes go on</w:t></w:r></w:p></w:footnote>';
    await openBuilt(page, {
      styles: STYLES,
      separators: false,
      footnotes: own + note(2, "A note."),
      sections: [{ paragraphs: [withNote("Text", 2)] }],
    });
    const n = sheetNotes(page, 0);
    await expect(n).toHaveAttribute("data-sep", "custom");
    await expect(n.locator(".notes-sep")).toContainText("* * *");
    // Word's short rule is gone.
    expect(await n.evaluate((el) => getComputedStyle(el, "::before").display)).toBe("none");
    // The note itself is still the only note.
    expect(await noteLabels(page, 0)).toEqual(["1"]);
  });

  test("an emptied separator draws no line", async ({ page }) => {
    const own = '<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:t xml:space="preserve"></w:t></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>';
    await openBuilt(page, { styles: STYLES, separators: false, footnotes: own + note(1, "A note."), sections: [{ paragraphs: [withNote("Text", 1)] }] });
    expect(await sheetNotes(page, 0).evaluate((el) => getComputedStyle(el, "::before").display)).toBe("none");
  });

  test("notes that don't fit go on to the next page under a continuation separator", async ({ page }) => {
    // Each note is a few hundred pixels tall in any font (about 13 lines of Calibri's metric twin), so three fit under the
    // text and the rest have to go on. (With fewer words every note fits in the metric-compatible font and nothing carries.)
    const long = (i: number) => note(i, `Note ${i}. ${"Lots of words in this note to fill lines of the page. ".repeat(30)}`);
    const own =
      '<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>' +
      '<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>' +
      '<w:footnote w:type="continuationNotice" w:id="9"><w:p><w:r><w:t>Notes continue on the next page</w:t></w:r></w:p></w:footnote>';
    await openBuilt(page, {
      styles: STYLES,
      separators: false,
      footnotes: own + [1, 2, 3, 4, 5, 6].map(long).join(""),
      sections: [{ paragraphs: [withNote("One", 1), withNote("Two", 2), withNote("Three", 3), withNote("Four", 4), withNote("Five", 5), withNote("Six", 6), "{PBR}", "The second page has text of its own"] }],
    });
    await expect(page.locator(".sheet")).toHaveCount(2);
    const first = await noteLabels(page, 0);
    const second = await noteLabels(page, 1);
    // Not every note fits under the text of the first page: the rest carry over, in order, and none is lost.
    expect(first.length).toBeGreaterThan(0);
    expect(first.length).toBeLessThan(6);
    expect([...first, ...second]).toEqual(["1", "2", "3", "4", "5", "6"]);
    await expect(sheetNotes(page, 0)).toContainText("Notes continue on the next page");
    await expect(sheetNotes(page, 1)).toHaveAttribute("data-cont", "1");
    // The notes keep clear of the text: at least a few lines of body text are left on the page.
    const room = await sheet(page, 0).evaluate((el) => {
      const notes = el.querySelector(".notes")!.getBoundingClientRect();
      return notes.top - el.getBoundingClientRect().top;
    });
    expect(room).toBeGreaterThan(96 + 60);
    await page.screenshot({ path: "test-results/notes-continue.png" });
  });
});

test.describe("chapter page numbers", () => {
  const spec = (sep: string): Parameters<typeof openBuilt>[1] => ({
    styles: STYLES,
    sections: [
      {
        footers: { default: "Page {PAGE}" },
        xml: { afterMar: `<w:pgNumType w:chapStyle="1"${sep}/>` },
        paragraphs: [{ text: "First chapter", ppr: '<w:pStyle w:val="Heading1"/>' }, ...lines(70), { text: "Second chapter", ppr: '<w:pStyle w:val="Heading1"/><w:pageBreakBefore/>' }, "Text of the second chapter"],
      },
    ],
  });

  test("the page number follows the chapter, and starts again in each chapter", async ({ page }) => {
    await openBuilt(page, spec(' w:chapSep="enDash"'));
    await expect(page.locator(".sheet")).toHaveCount(3);
    const footers = await Promise.all([0, 1, 2].map((k) => sheet(page, k).locator(".ftr").textContent()));
    expect(footers).toEqual(["Page 1–1", "Page 1–2", "Page 2–1"]);
  });

  test("the separator is a hyphen unless the file says otherwise", async ({ page }) => {
    await openBuilt(page, spec(""));
    expect(await sheet(page, 0).locator(".ftr").textContent()).toBe("Page 1-1");
    await openBuilt(page, spec(' w:chapSep="colon"'));
    expect(await sheet(page, 2).locator(".ftr").textContent()).toBe("Page 2:1");
  });
});
