// Tagged PDF and PDF/A: Download as a PDF that carries the document's structure (headings, paragraphs, lists, tables, figures with
// descriptions, links, the language), so screen readers read it in order; and an archive PDF (PDF/A-2b) with an sRGB colour
// profile and metadata that says so. The tests read the file back with e2e/pdf-structure.ts and run the checks a validator starts with.
import { readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, openFixture, test, type Page } from "./helpers";
import { readPdf, type PdfFile } from "./support/pdf-reader";
import { find, marked, outline, pdfUaIssues, structTree, textOf, walk } from "./support/pdf-structure";
import { pictureRun, quarters } from "./support/picture-markup";

/** File, Download, then an item of the Download menu: the file read. */
async function download(page: Page, item = "PDF document (.pdf)"): Promise<{ pdf: PdfFile; bytes: Buffer }> {
  const [d] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", item)]);
  const bytes = readFileSync((await d.path())!);
  return { pdf: readPdf(bytes), bytes };
}

/** rich.docx with a description on its picture, which a file needs before it can say it is PDF/UA (the fixture has none). */
async function openDescribedRich(page: Page) {
  await openFixture(page, "rich.docx");
  await inPage(page, (w) => w.fe.editor.exec({ op: "set_image", at: { p: 28, o: 0 }, alt: "A chart of the quarter's numbers" }));
}

const catalogOf =(pdf: PdfFile) => {
  const trailer = pdf.text.slice(pdf.text.lastIndexOf("trailer"));
  const n = Number(/\/Root (\d+) 0 R/.exec(trailer)![1]);
  return { n, dict: pdf.objects.get(n)!.dict };
};
const refOf = (dict: string, key: string) => {
  const m = new RegExp(`/${key} (\\d+) 0 R`).exec(dict);
  return m ? Number(m[1]) : undefined;
};
const streamOf = (pdf: PdfFile, n: number): Buffer => {
  const o = pdf.objects.get(n)!;
  return /\/FlateDecode/.test(o.dict) ? inflateSync(o.stream!) : o.stream!;
};

test.describe("tagged PDF", () => {
  test("rich.docx gives a tree of headings, paragraphs, lists, a table with header cells and a link, and no validator problems", async ({ page }) => {
    await openDescribedRich(page);
    const { pdf } = await download(page);
    expect(pdfUaIssues(pdf)).toEqual([]);
    const tree = structTree(pdf)!;
    expect(tree.type).toBe("Document");
    // The types in reading order at the top level.
    expect(tree.kids.filter((k) => "type" in k).map((k) => (k as { type: string }).type)).toEqual(["P", "P", "H1", "L", "H2", "L", "P", "P", "H1", "Table", "P", "P", "P", "H1", "P"]);
    // Headings carry their text, and the document's title is a paragraph (Word's Title style is not a heading).
    const headings: string[] = [];
    walk(tree, (e) => /^H[1-6]$/.test(e.type) && headings.push(textOf(pdf, e)));
    expect(headings).toEqual(["Highlights", "Plan", "Numbers", "Appendix"]);
    expect(textOf(pdf, tree.kids[0] as never)).toBe("Quarterly Product Report");
    // Lists: a label and a body in each item, and how the list counts.
    const lists = find(tree, "L");
    expect(lists.map((l) => l.listNumbering)).toEqual(["Disc", "Decimal"]);
    for (const l of lists) for (const k of l.kids) expect(outline(k as never).slice(0, 3)).toEqual(["LI", "  Lbl", "  LBody"]);
    expect(textOf(pdf, lists[1]!.kids[0] as never)).toBe("1.Ship the editor");
    // The table: rows of cells, a header row and a header column.
    const [table] = find(tree, "Table");
    const rows = table!.kids as never as { type: string; kids: { type: string; scope?: string }[] }[];
    expect(rows.every((r) => r.type === "TR")).toBe(true);
    expect(rows[0]!.kids.map((c) => `${c.type}:${c.scope}`)).toEqual(["TH:Column", "TH:Column", "TH:Column"]);
    expect(rows[1]!.kids.map((c) => `${c.type}:${c.scope}`)).toEqual(["TH:Row", "TD:undefined", "TD:undefined"]);
    expect(textOf(pdf, rows[1] as never)).toBe("Revenue$1.2M$1.4M");
    // A link is a Link element, in its paragraph, in the middle of the text, holding the annotation.
    const [link] = find(tree, "Link");
    expect(link!.alt).toBe("the dashboard");
    expect(link!.kids.some((k) => "annot" in k)).toBe(true);
    const paragraph = find(tree, "P").find((p) => p.kids.some((k) => "type" in k && k.type === "Link"))!;
    expect(textOf(pdf, paragraph)).toMatch(/^This report summarises.*the dashboard/);
    const text = textOf(pdf, paragraph);
    expect(text.indexOf("the dashboard")).toBeGreaterThan(20);
    // The document says what language it is in, and its title.
    expect(catalogOf(pdf).dict).toMatch(/\/Lang \(en-US\)/);
  });

  test("the header, the footer, page numbers and borders are artifacts, not part of the text", async ({ page }) => {
    await openDescribedRich(page);
    const { pdf } = await download(page);
    const tree = structTree(pdf)!;
    let all = "";
    walk(tree, (e) => {
      if (e.type === "P") all += textOf(pdf, e);
    });
    expect(all).not.toContain("confidential draft");
    // It is on the page, marked as an artifact, so a sighted reader sees it and a screen reader skips it.
    const m = marked(pdf, pdf.pages[0]!);
    expect(m.sequences.some((s) => s.tag === "Artifact" && s.text.includes("confidential draft"))).toBe(true);
    expect(m.bare).toEqual([]);
    expect(m.unbalanced).toBe(false);
    // Every page's marked content is numbered from 0, once each, and is in the tree exactly once (the validator checks it).
    expect(pdfUaIssues(pdf)).toEqual([]);
  });

  test("a picture with a description is a Figure that says it; one without is a Figure with no text, and the file makes no PDF/UA claim", async ({ page }) => {
    await openBuilt(page, {
      media: [{ name: "quarters.png", rid: "rIdPic1", bytes: quarters() }],
      sections: [
        {
          paragraphs: [
            { text: "Chapter", ppr: '<w:pStyle w:val="Heading1"/>' },
            { raw: pictureRun({ rid: "rIdPic1", id: 1, alt: "Four coloured squares", w: 120, h: 60 }) },
            { raw: pictureRun({ rid: "rIdPic1", id: 2, w: 60, h: 30 }) },
            "After the pictures",
          ],
        },
      ],
    });
    const { pdf } = await download(page);
    // The picture with no description is a Figure with no text (a reader knows it is there; a checker flags it), so the file
    // is tagged but does not claim PDF/UA: that is the only thing the validator checks that this file fails.
    expect(pdfUaIssues(pdf)).toEqual(["the metadata does not identify PDF/UA", "a figure has no description"]);
    const figures = find(structTree(pdf)!, "Figure");
    expect(figures.map((f) => f.alt)).toEqual(["Four coloured squares", undefined]);
    // Each picture's content is drawn as a marked-content sequence of its own.
    const m = marked(pdf, pdf.pages[0]!);
    expect(m.sequences.filter((s) => s.tag === "Figure")).toHaveLength(2);
  });

  test("a file claims PDF/UA only when it backs the claim: here, a skipped heading level means no claim", async ({ page }) => {
    const claim = async () => {
      const { pdf } = await download(page);
      const xmp = streamOf(pdf, refOf(catalogOf(pdf).dict, "Metadata")!).toString("utf8");
      return { claims: xmp.includes("<pdfuaid:part>1</pdfuaid:part>"), tagged: !!structTree(pdf), issues: pdfUaIssues(pdf) };
    };
    // Heading 1, then Heading 3: the level above is missing.
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Chapter", ppr: '<w:pStyle w:val="Heading1"/>' }, { text: "Detail", ppr: '<w:pStyle w:val="Heading3"/>' }, "Text"] }] });
    let got = await claim();
    expect(got.claims).toBe(false);
    expect(got.tagged).toBe(true);
    // The same document in order claims it.
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Chapter", ppr: '<w:pStyle w:val="Heading1"/>' }, { text: "Detail", ppr: '<w:pStyle w:val="Heading2"/>' }, "Text"] }] });
    got = await claim();
    expect(got.claims).toBe(true);
    expect(got.issues).toEqual([]);
    // A document that starts at Heading 2 skips Heading 1.
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Section", ppr: '<w:pStyle w:val="Heading2"/>' }, "Text"] }] });
    expect((await claim()).claims).toBe(false);
  });

  test("a passage in another language says so, and footnotes come after the text", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: ["First paragraph in English.", "Un párrafo en español.", { raw: '<w:r><w:t>Last, with a note.</w:t></w:r><w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="1"/></w:r>' }] }],
      footnotes: '<w:footnote w:id="1"><w:p><w:r><w:t>The note says so.</w:t></w:r></w:p></w:footnote>',
    });
    const ids = await inPage(page, (w) => w.fe.editor.engine!.renderAll().order);
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "set_language", lang: "es-ES", ids: [id] }), ids[1]);
    const { pdf } = await download(page);
    const tree = structTree(pdf)!;
    const paras = find(tree, "P");
    expect(paras.find((p) => textOf(pdf, p) === "Un párrafo en español.")?.lang).toBe("es-ES");
    expect(paras.find((p) => textOf(pdf, p) === "First paragraph in English.")?.lang).toBeUndefined();
    // The notes are a section at the end of the document.
    const last = tree.kids[tree.kids.length - 1] as { type: string; kids: unknown[] };
    expect(last.type).toBe("Sect");
    expect(textOf(pdf, last as never)).toContain("The note says so.");
    expect(textOf(pdf, tree.kids[0] as never)).toBe("First paragraph in English.");
    expect(pdfUaIssues(pdf)).toEqual([]);
  });

  test("a document's own headings are headings at their levels, whatever their style is called", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            { text: "Top", ppr: '<w:pStyle w:val="Heading1"/>' },
            { text: "Sub", ppr: '<w:pStyle w:val="Heading2"/>' },
            { text: "Body", ppr: "" },
            { text: "Own heading", ppr: '<w:outlineLvl w:val="2"/>' },
          ],
        },
      ],
    });
    const { pdf } = await download(page);
    const levels = outline(structTree(pdf)!).filter((l) => /^ {2}H\d$/.test(l)).map((l) => l.trim());
    expect(levels).toEqual(["H1", "H2", "H3"]);
  });

  test("with tagging off the file has no structure, and the text is as before", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await chooseMenu(page, "Tools", "Preferences…");
    const prefs = dialog(page, "Preferences");
    await expect(prefs.getByRole("heading", { name: "PDF files" })).toBeVisible();
    const box = prefs.getByLabel("Tag PDFs for screen readers");
    await expect(box).toBeChecked();
    await box.uncheck();
    await prefs.getByRole("button", { name: "Apply" }).click();
    const { pdf } = await download(page);
    const catalog = catalogOf(pdf).dict;
    expect(catalog).not.toContain("StructTreeRoot");
    expect(catalog).not.toContain("MarkInfo");
    expect(structTree(pdf)).toBeNull();
    expect(pdf.pageText.join(" ")).toContain("Quarterly Product Report");
    // The choice is kept with the other settings.
    await page.reload();
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await chooseMenu(page, "Tools", "Preferences…");
    await expect(dialog(page, "Preferences").getByLabel("Tag PDFs for screen readers")).not.toBeChecked();
  });
});

test.describe("PDF/A", () => {
  test("File, Download, PDF/A archive makes a PDF/A-2b file: output intent, sRGB profile, metadata, held fonts", async ({ page }) => {
    await openDescribedRich(page);
    const { pdf, bytes } = await download(page, "PDF/A archive (.pdf)");
    expect(bytes.subarray(0, 8).toString("latin1")).toMatch(/^%PDF-1\.7/);
    const catalog = catalogOf(pdf).dict;
    // The output intent names a profile that is a valid, embedded sRGB ICC profile.
    expect(catalog).toMatch(/\/OutputIntents \[<< \/Type \/OutputIntent \/S \/GTS_PDFA1 \/OutputConditionIdentifier \(sRGB IEC61966-2\.1\)/);
    const profile = streamOf(pdf, refOf(catalog, "DestOutputProfile")!);
    expect(profile.subarray(36, 40).toString("latin1")).toBe("acsp");
    expect(profile.readUInt32BE(0)).toBe(profile.length);
    expect(profile.subarray(16, 20).toString("latin1")).toBe("RGB ");
    expect(pdf.objects.get(refOf(catalog, "DestOutputProfile")!)!.dict).toMatch(/\/N 3 /);
    // The metadata stream is not compressed, says PDF/A-2b, and repeats the file's details.
    const metadataObj = pdf.objects.get(refOf(catalog, "Metadata")!)!;
    expect(metadataObj.dict).toMatch(/\/Type \/Metadata \/Subtype \/XML/);
    expect(metadataObj.dict).not.toContain("FlateDecode");
    const xmp = metadataObj.stream!.toString("utf8");
    expect(xmp).toContain("<pdfaid:part>2</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance>");
    expect(xmp).toContain("<pdfuaid:part>1</pdfuaid:part>");
    expect(xmp).toContain("<dc:title>");
    const title = /\/Title \(([^)]*)\)/.exec(pdf.info)?.[1] ?? /\/Title <([0-9A-F]+)>/.exec(pdf.info)?.[1];
    expect(title).toBeTruthy();
    expect(xmp).toMatch(/<xmp:CreateDate>\d{4}-\d\d-\d\dT[\d:]+Z<\/xmp:CreateDate>/);
    // Pages say what colour space their transparency is in, links print, and every font has its file.
    for (const p of pdf.pages) expect(pdf.objects.get(p)!.dict).toContain("/Group << /S /Transparency /CS /DeviceRGB >>");
    const links = [...pdf.objects.values()].filter((o) => /\/Subtype \/Link/.test(o.dict));
    expect(links.length).toBeGreaterThan(0);
    for (const l of links) expect(l.dict).toContain("/F 4");
    const fonts = [...pdf.objects.values()].filter((o) => /\/Type \/Font\b/.test(o.dict));
    expect(fonts.length).toBeGreaterThan(0);
    for (const f of fonts) expect(f.dict).not.toMatch(/\/Subtype \/Type1\b/);
    for (const d of [...pdf.objects.values()].filter((o) => /\/Type \/FontDescriptor/.test(o.dict))) expect(d.dict).toMatch(/\/FontFile2 \d+ 0 R/);
    // It is tagged too, with no problems.
    expect(pdfUaIssues(pdf)).toEqual([]);
  });

  test("the setting makes every PDF download PDF/A, and an ordinary one has none of it", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const plain = await download(page);
    expect(catalogOf(plain.pdf).dict).not.toContain("OutputIntents");
    expect(plain.bytes.toString("latin1")).not.toContain("pdfaid");
    await chooseMenu(page, "Tools", "Preferences…");
    const prefs = dialog(page, "Preferences");
    await prefs.getByLabel("Make PDF documents PDF/A archive files").check();
    await prefs.getByRole("button", { name: "Apply" }).click();
    const archive = await download(page);
    expect(catalogOf(archive.pdf).dict).toContain("OutputIntents");
    expect(archive.bytes.toString("latin1")).toContain("<pdfaid:part>2</pdfaid:part>");
  });

  test("text in a font the file cannot hold makes it a PDF, not a PDF/A, and says so", async ({ page }) => {
    await openFixture(page, "simple.docx");
    // The font files can't be read, so the standard fonts stand in: they are only named in the file.
    await page.route(/\.woff2?(\?.*)?$/, (route) => route.abort());
    await page.route(/\.ttf(\?.*)?$/, (route) => route.abort());
    const { pdf, bytes } = await download(page, "PDF/A archive (.pdf)");
    await expect(page.locator(".toast")).toContainText("This file is a PDF but not PDF/A");
    expect(catalogOf(pdf).dict).not.toContain("OutputIntents");
    expect(bytes.toString("latin1")).not.toContain("pdfaid");
    // It is still a tagged PDF.
    expect(catalogOf(pdf).dict).toContain("StructTreeRoot");
  });
});
