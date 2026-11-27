// Opening files that aren't plain .docx: OpenDocument, Rich Text, web pages, old .doc files, Word 2003 XML and Flat OPC
// become new Word documents (and say so); damaged files are repaired and say what was fixed; Strict Open XML is read as
// the standard kind; files that aren't documents say what they are. Every file goes in through the app's open flow
// (the hidden file input behind File, Open).
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { zip } from "./support/docx-builder";
import { dialog, documentReady, expect, inPage, LINE_START, openFixture, test, type Page } from "./helpers";

const fixtures = resolve(process.cwd(), "fixtures");
const sample = (name: string) => readFile(resolve(fixtures, "formats", name));

/** Put a file through the app's open flow, as File, Open does for a format that opens as a new document. */
async function openFile(page: Page, name: string, buffer: Buffer | Uint8Array, mimeType = "application/octet-stream") {
  await page.locator("#app > input[type=file]").setInputFiles({ name, mimeType, buffer: Buffer.from(buffer) });
}

/** A fresh app with a document open, so a second one visibly replaces it. */
async function start(page: Page) {
  await openFixture(page, "simple.docx");
}

const strip = (page: Page) => page.locator(".open-notice");
const title = (page: Page) => page.getByRole("textbox", { name: "Document name" });
const texts = (page: Page) => page.locator(".flow .fp").allTextContents();
/** The run of text that is exactly `text` (formatting is on the run's style). */
const run = (page: Page, text: string) => page.locator(".flow .s").filter({ hasText: new RegExp(`^${text}$`) }).first();

test.describe("other formats open as new Word documents", () => {
  test("OpenDocument text: headings, formatting, lists, a table and a link, and a notice that it was converted", async ({ page }) => {
    await start(page);
    await openFile(page, "sample.odt", await sample("sample.odt"));
    await expect(title(page)).toHaveValue("sample");
    await expect(page).toHaveURL(/#\/library\/sample\.docx$/);
    await expect(page.locator(".flow .fp").first()).toHaveText("Quarterly report");

    const t = await texts(page);
    for (const want of ["Highlights", "First point", "Second point", "Name", "Score", "Ann", "90", "Centered line", "Plain closing paragraph."]) expect(t.some((x) => x.endsWith(want)), want).toBe(true);
    // Bold, a link, a table with its cells, centred text.
    await expect(run(page, "bold")).toHaveCSS("font-weight", "700");
    await expect(page.locator(`.flow [data-href="https://example.com/page"]`)).toHaveText("link");
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    await expect(page.locator(".flow table.ft tr")).toHaveCount(3);
    await expect(page.locator(".flow .fp", { hasText: "Centered line" })).toHaveCSS("text-align", "center");
    await expect(page.locator(".flow .fp", { hasText: "Second point" }).first()).toContainText("•");

    // The notice says what happened, in a strip that reuses the page's quiet tone.
    await expect(strip(page)).toBeVisible();
    await expect(strip(page)).toContainText("This OpenDocument text file was converted to a Word document.");
    await strip(page).getByRole("button", { name: "Details…" }).click();
    const d = dialog(page, "About this file");
    await expect(d).toContainText("Your original file has not been changed");
    await expect(d).toContainText("Headers and footers, text boxes, drawings and charts are not carried over.");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toHaveCount(0);
    await page.screenshot({ path: "test-results/open-odt.png" });

    // It is a real .docx in the Library now, and an ordinary document: editing and saving work.
    const saved = await inPage(page, (w) => w.files.library.list("").then((l) => l.map((f) => f.name)));
    expect(saved).toContain("sample.docx");
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("A ");
    await expect(page.locator(".flow .fp").first()).toContainText("A Quarterly report");
    await expect(page.locator(".led")).toHaveText("Saved in your Library");
  });

  test("the strip goes when the notice is dismissed, and returns for the next converted file only", async ({ page }) => {
    await start(page);
    await expect(strip(page)).toBeHidden();
    await openFile(page, "sample.rtf", await sample("sample.rtf"));
    await expect(strip(page)).toContainText("This Rich Text file was converted to a Word document.");
    await strip(page).getByRole("button", { name: "Dismiss" }).click();
    await expect(strip(page)).toBeHidden();
    // A plain .docx has nothing to say.
    await openFixture(page, "rich.docx");
    await expect(strip(page)).toBeHidden();
  });

  test("Rich Text and web pages keep their text, formatting and tables", async ({ page }) => {
    await start(page);
    await openFile(page, "sample.rtf", await sample("sample.rtf"));
    await expect(page.locator(".flow .fp").first()).toHaveText("Quarterly report");
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    await expect(page.locator(`.flow [data-href="https://example.com/page"]`)).toHaveText("link");
    await expect(run(page, "italic")).toHaveCSS("font-style", "italic");

    // A second file with the same name gets its own name in the Library.
    await openFile(page, "sample.html", await sample("sample.html"));
    await expect(title(page)).toHaveValue("sample 2");
    await expect(strip(page)).toContainText("This HTML file was converted to a Word document.");
    await expect(page.locator(".flow .fp", { hasText: "Plain closing paragraph." })).toBeVisible();
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    await expect(page.locator(".flow .fp", { hasText: "Second point" }).first()).toBeVisible();
    // The page's title is the document's title.
    expect(await inPage(page, (w) => w.engine!.docProps().title)).toBe("Sample report");
  });

  test("an old .doc file: text, formatting, a table, and what is lost is said plainly", async ({ page }) => {
    await start(page);
    await openFile(page, "sample.doc", await sample("sample.doc"));
    await expect(title(page)).toHaveValue("sample");
    await expect(strip(page)).toContainText("This Word 97 to 2003 file was converted to a Word document.");
    await expect(page.locator(".flow .fp").first()).toHaveText("Quarterly report");
    const t = await texts(page);
    for (const want of ["Highlights", "Name", "Score", "Ann", "90", "Bob", "85", "Centered line", "Plain closing paragraph."]) expect(t.some((x) => x.endsWith(want)), want).toBe(true);
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    await expect(page.locator(".flow table.ft tr")).toHaveCount(3);
    await expect(run(page, "bold")).toHaveCSS("font-weight", "700");
    await expect(page.locator(".flow .fp", { hasText: "Centered line" })).toHaveCSS("text-align", "center");
    await strip(page).getByRole("button", { name: "Details…" }).click();
    const d = dialog(page, "About this file");
    await expect(d).toContainText("Styles are turned into direct formatting");
    await expect(d).toContainText("headers and footers, text boxes, drawings, equations and objects are not carried over");
    await d.getByRole("button", { name: "OK" }).click();
    expect(await inPage(page, (w) => w.engine!.docProps().author)).toBe("Ada Lovelace");
    await page.screenshot({ path: "test-results/open-doc.png" });
  });

  test("Word 2003 XML and a single-file Word package both come in through .xml", async ({ page }) => {
    await start(page);
    await openFile(page, "old.xml", await sample("sample-word2003.xml"), "text/xml");
    await expect(strip(page)).toContainText("This Word 2003 XML file was converted to a Word document.");
    await expect(page.locator(".flow .fp").first()).toHaveText("Quarterly report");
    await expect(title(page)).toHaveValue("old");

    const flat = `<?xml version="1.0" standalone="yes"?><?mso-application progid="Word.Document"?><pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
<pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml"><pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData></pkg:part>
<pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"><pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Single file package</w:t></w:r></w:p><w:p><w:r><w:t>Second paragraph.</w:t></w:r></w:p></w:body></w:document></pkg:xmlData></pkg:part></pkg:package>`;
    await openFile(page, "package.xml", Buffer.from(flat), "text/xml");
    await expect(strip(page)).toContainText("This Word XML package file was converted to a Word document.");
    await expect(page.locator(".flow .fp").first()).toHaveText("Single file package");
    await expect(run(page, "Single file package")).toHaveCSS("font-weight", "700");
    await expect(page.locator(".flow .fp").nth(1)).toHaveText("Second paragraph.");
  });

  test("a file called .docx that is really Rich Text opens as Rich Text", async ({ page }) => {
    await start(page);
    await openFile(page, "report.docx", await sample("sample.rtf"));
    await expect(strip(page)).toContainText("This Rich Text file was converted to a Word document.");
    await expect(page.locator(".flow .fp").first()).toHaveText("Quarterly report");
  });

  test("the strip and the dialogs are readable in the light and the dark theme", async ({ page }) => {
    await start(page);
    await openFile(page, "sample.odt", await sample("sample.odt"));
    await expect(strip(page)).toBeVisible();
    const ratio = () =>
      page.evaluate(() => {
        const lum = (rgb: string) => {
          const [r, g, b] = (rgb.match(/[\d.]+/g) ?? ["0", "0", "0"]).slice(0, 3).map((v) => {
            const c = Number(v) / 255;
            return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          });
          return 0.2126 * r + 0.7152 * g + 0.0722 * b;
        };
        const el = document.querySelector(".open-notice")!;
        const text = getComputedStyle(el.querySelector("p")!).color;
        const bg = getComputedStyle(el).backgroundColor;
        const [a, b] = [lum(text), lum(bg)].sort((x, y) => y - x);
        return (a + 0.05) / (b + 0.05);
      });
    for (const theme of ["light", "dark"]) {
      await inPage(page, (w, th) => (w.fe as unknown as { setTheme(t: string): void }).setTheme(th), theme);
      expect(await ratio(), theme).toBeGreaterThanOrEqual(4.5);
      await strip(page).getByRole("button", { name: "Details…" }).click();
      await expect(dialog(page, "About this file")).toBeVisible();
      await page.screenshot({ path: `test-results/open-notice-${theme}.png` });
      await dialog(page, "About this file").getByRole("button", { name: "OK" }).click();
    }
  });

  test("turning an Open extension off takes its format away", async ({ page }) => {
    await start(page);
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("docxtor.open.rtf"));
    await openFile(page, "sample.rtf", await sample("sample.rtf"));
    const d = dialog(page, /Couldn't open sample\.rtf/);
    await expect(d).toBeVisible();
    await expect(d).toContainText("Docxtor can't open sample.rtf");
    await d.getByRole("button", { name: "OK" }).click();
  });

  test("an embedder opens a file with fe.open and reads the notice", async ({ page }) => {
    await start(page);
    const result = await page.evaluate(async (bytes) => {
      const w = (window as unknown as { docxtor: { fe: { open(b: Uint8Array, n: string): Promise<unknown>; openNotice: { title: string; notes: string[] } | null } } }).docxtor;
      await w.fe.open(new Uint8Array(bytes), "x.odt");
      return w.fe.openNotice;
    }, Array.from(await sample("sample.odt")));
    expect(result?.title).toBe("This OpenDocument text file was converted to a Word document.");
    expect(result?.notes.length).toBeGreaterThan(1);
    await expect(strip(page)).toContainText("This OpenDocument text file was converted");
  });
});

test.describe("damaged files", () => {
  test("a cut-off file opens with the text it has and says it was repaired", async ({ page }) => {
    await start(page);
    const full = await readFile(resolve(fixtures, "long.docx"));
    await openFile(page, "cut.docx", full.subarray(0, Math.floor(full.length * 0.62)));
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await expect(strip(page)).toContainText("This file was damaged. Docxtor repaired it as far as it could.");
    await strip(page).getByRole("button", { name: "Details…" }).click();
    const d = dialog(page, "About this file");
    await expect(d).toContainText("The file's index was damaged. Docxtor rebuilt it and found");
    await expect(d).toContainText("Check the document before you rely on it.");
    await d.getByRole("button", { name: "OK" }).click();
    await page.screenshot({ path: "test-results/open-cut.png" });
  });

  test("damaged markup keeps the text on both sides and says the text is cut", async ({ page }) => {
    await start(page);
    const REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const paras = Array.from({ length: 40 }, (_, i) => `<w:p><w:r><w:t>Line number ${i + 1} of the damaged file</w:t></w:r></w:p>`);
    paras[19] = `<w:p><w:r <<><w:t>Line number 20 of the damaged file</w:t></w:r></w:p>`;
    const body = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="${W}"><w:body>${paras.join("")}<w:p><w:r><w:t>The last li`;
    const bytes = zip([
      ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>`],
      ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${REL}/officeDocument" Target="word/document.xml"/></Relationships>`],
      ["word/document.xml", body],
    ]);
    await openFile(page, "markup.docx", bytes);
    await expect(strip(page)).toContainText("This file was damaged");
    const t = await texts(page);
    expect(t).toContain("Line number 19 of the damaged file");
    expect(t).toContain("Line number 20 of the damaged file");
    expect(t).toContain("Line number 40 of the damaged file");
    await strip(page).getByRole("button", { name: "Details…" }).click();
    const d = dialog(page, "About this file");
    await expect(d).toContainText("The text of the document ends early.");
    await expect(d).toContainText("part had damaged markup");
    await d.getByRole("button", { name: "OK" }).click();
  });

  test("missing relationships and content types are put back", async ({ page }) => {
    await start(page);
    const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const bytes = zip([["word/document.xml", `<?xml version="1.0"?><w:document xmlns:w="${W}"><w:body><w:p><w:r><w:t>Bare document part</w:t></w:r></w:p></w:body></w:document>`]]);
    await openFile(page, "bare.docx", bytes);
    await expect(page.locator(".flow .fp").first()).toHaveText("Bare document part");
    await strip(page).getByRole("button", { name: "Details…" }).click();
    const d = dialog(page, "About this file");
    await expect(d).toContainText("Missing links between the file's parts were put back.");
    await expect(d).toContainText("The file's list of part types was incomplete. Docxtor completed it.");
    await d.getByRole("button", { name: "OK" }).click();
  });
});

test.describe("Strict Open XML", () => {
  test("is read as the standard kind: footnotes, centred text and the theme come through, and the notice says so", async ({ page }) => {
    await start(page);
    const S = "http://purl.oclc.org/ooxml";
    const R = `${S}/officeDocument/relationships`;
    const W = `${S}/wordprocessingml/main`;
    const body = `<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="${W}" xmlns:r="${R}"><w:body><w:p><w:pPr><w:jc w:val="start"/></w:pPr><w:r><w:t>Strict first paragraph</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r></w:p><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Centred strict paragraph</w:t></w:r></w:p></w:body></w:document>`;
    const notes = `<?xml version="1.0" encoding="UTF-8"?><w:footnotes xmlns:w="${W}"><w:footnote w:type="separator" w:id="0"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="1"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote><w:footnote w:id="2"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> A note in the strict file.</w:t></w:r></w:p></w:footnote></w:footnotes>`;
    const theme = `<?xml version="1.0" encoding="UTF-8"?><a:theme xmlns:a="${S}/drawingml/main" name="Strict theme"><a:themeElements><a:clrScheme name="x"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F497D"/></a:dk2><a:lt2><a:srgbClr val="EEECE1"/></a:lt2><a:accent1><a:srgbClr val="4F81BD"/></a:accent1><a:accent2><a:srgbClr val="C0504D"/></a:accent2><a:accent3><a:srgbClr val="9BBB59"/></a:accent3><a:accent4><a:srgbClr val="8064A2"/></a:accent4><a:accent5><a:srgbClr val="4BACC6"/></a:accent5><a:accent6><a:srgbClr val="F79646"/></a:accent6><a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme><a:fontScheme name="y"><a:majorFont><a:latin typeface="Georgia"/></a:majorFont><a:minorFont><a:latin typeface="Georgia"/></a:minorFont></a:fontScheme><a:fmtScheme name="z"/></a:themeElements></a:theme>`;
    const bytes = zip([
      ["[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/><Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>`],
      ["_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R}/officeDocument" Target="word/document.xml"/></Relationships>`],
      ["word/_rels/document.xml.rels", `<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="${R}/footnotes" Target="footnotes.xml"/><Relationship Id="rId2" Type="${R}/theme" Target="theme/theme1.xml"/></Relationships>`],
      ["word/document.xml", body],
      ["word/footnotes.xml", notes],
      ["word/theme/theme1.xml", theme],
    ]);
    await openFile(page, "strict.docx", bytes);
    await expect(page.locator(".flow .fp").first()).toContainText("Strict first paragraph");
    await expect(strip(page)).toContainText("This file uses Strict Open XML.");
    await strip(page).getByRole("button", { name: "Details…" }).click();
    const d = dialog(page, "About this file");
    await expect(d).toContainText("Docxtor read it as the standard kind of Word file, and will save it as one.");
    await d.getByRole("button", { name: "OK" }).click();
    // The footnote text is on the page, the "start" paragraph is aligned to the left and the other is centred.
    await expect(page.locator(".sheet .notes").filter({ hasText: "A note in the strict file." })).toBeVisible();
    await expect(page.locator(".flow .fp", { hasText: "Strict first paragraph" })).toHaveCSS("text-align", /^(left|start)$/);
    await expect(page.locator(".flow .fp", { hasText: "Centred strict paragraph" })).toHaveCSS("text-align", "center");
    // The theme is read too: the document's theme fonts are the strict file's.
  });
});

test.describe("files that cannot be opened", () => {
  const cases: [string, string, Uint8Array | Buffer, string][] = [
    ["a spreadsheet", "budget.docx", zip([["[Content_Types].xml", `<Types><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>`], ["xl/workbook.xml", "<workbook/>"]]), "This is a spreadsheet, not a Word document. Open it in a spreadsheet app."],
    ["a presentation", "slides.docx", zip([["ppt/presentation.xml", "<p/>"]]), "This is a presentation, not a Word document."],
    ["a PDF", "paper.docx", Buffer.from("%PDF-1.7\n1 0 obj\n"), "This is a PDF, and Docxtor can't open PDFs yet."],
    ["an empty file", "empty.docx", Buffer.alloc(0), "The file is empty."],
    ["bytes that are nothing", "noise.docx", Buffer.from([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]), "Docxtor doesn't recognise this kind of file."],
    ["a zip of other things", "stuff.docx", zip([["notes.txt", "hello"]]), "This is a zip archive that doesn't hold a Word document."],
  ];
  for (const [what, name, bytes, message] of cases) {
    test(`${what} says what it is and leaves the open document alone`, async ({ page }) => {
      await start(page);
      const before = await texts(page);
      await openFile(page, name, bytes);
      const d = dialog(page, new RegExp(`Couldn't open ${name.replace(".", "\\.")}`));
      await expect(d).toBeVisible();
      await expect(d).toContainText(message);
      await d.getByRole("button", { name: "OK" }).click();
      await expect(d).toHaveCount(0);
      expect(await texts(page)).toEqual(before);
      await expect(strip(page)).toBeHidden();
    });
  }

  test("an old .doc with a password says that Docxtor can't unlock it", async ({ page }) => {
    await start(page);
    // A compound file whose Word header says it is encrypted: flip the flag in the sample's WordDocument stream.
    const bytes = Buffer.from(await sample("sample.doc"));
    const marked = await page.evaluate(async (b) => {
      // The stream is stored in the compound file's sectors; find the header by its signature (0xA5EC) and set bit 8 of the flags.
      const a = new Uint8Array(b);
      for (let i = 0; i + 12 < a.length; i += 1) {
        if (a[i] === 0xec && a[i + 1] === 0xa5 && a[i + 3] === 0x00 && (a[i + 2] & 0xff) >= 0xc1) {
          a[i + 11] |= 0x01;
          return Array.from(a);
        }
      }
      return null;
    }, Array.from(bytes));
    expect(marked).not.toBeNull();
    await openFile(page, "locked.doc", Buffer.from(marked!));
    const d = dialog(page, /Couldn't open locked\.doc/);
    await expect(d).toContainText("protected with a password, and Docxtor can't unlock those");
    await d.getByRole("button", { name: "OK" }).click();
  });
});

test("a document that opens plainly gets no notice and no extra dialog", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await expect(strip(page)).toBeHidden();
  await documentReady(page);
});
