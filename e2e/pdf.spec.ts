// Download as PDF: File, Download, PDF document. Docxtor writes the file itself from the laid-out pages.
// The tests download it, read it back with a small reader (e2e/pdf-reader.ts) and check what a viewer would find:
// the page count and sizes, the text of each page (through the fonts' ToUnicode maps), pictures, links, bookmarks,
// document details and the fonts.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { chooseMenu, expect, inPage, openFixture, test, type Page } from "./helpers";
import { mediaBox, pageContent, readPdf, textRuns, type PdfFile } from "./support/pdf-reader";

const MOD = "ControlOrMeta";

/** File, Download, PDF document: the downloaded file, read. */
async function downloadPdf(page: Page): Promise<{ pdf: PdfFile; name: string; bytes: Buffer }> {
  const [download] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
  const path = await download.path();
  const bytes = readFileSync(path);
  return { pdf: readPdf(bytes), name: download.suggestedFilename(), bytes };
}

const normal = (s: string) => s.replace(/\s+/g, " ");

test.describe("download as PDF", () => {
  test("File, Download lists the PDF, and the file is a valid PDF named after the document", async ({ page }) => {
    await openFixture(page);
    const { pdf, name, bytes } = await downloadPdf(page);
    expect(name).toBe("rich.pdf");
    expect(bytes.subarray(0, 5).toString("latin1")).toBe("%PDF-");
    expect(pdf.text.trimEnd().endsWith("%%EOF")).toBe(true);
    // The cross-reference table points at every object.
    const start = Number(/startxref\n(\d+)/.exec(pdf.text)![1]);
    expect(pdf.text.slice(start, start + 4)).toBe("xref");
    const entries = [...pdf.text.slice(start).matchAll(/^(\d{10}) 00000 n $/gm)].map((m) => Number(m[1]));
    expect(entries).toHaveLength(pdf.objects.size);
    entries.forEach((at, i) => expect(pdf.text.slice(at).startsWith(`${i + 1} 0 obj`)).toBe(true));
    expect(pdf.text).toMatch(/\/Size \d+ \/Root \d+ 0 R \/Info \d+ 0 R/);
  });

  test("it has as many pages as the editor, each the size of its sheet, with the text on its own page", async ({ page }) => {
    await openFixture(page);
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    const { pdf } = await downloadPdf(page);
    expect(pdf.pages).toHaveLength(pages);
    expect(pages).toBe(2);
    for (const p of pdf.pages) expect(mediaBox(pdf, p)).toEqual([612, 792]);
    expect(normal(pdf.pageText[0])).toContain("Quarterly Product Report");
    expect(normal(pdf.pageText[0])).toContain("This report summarises bold progress, italic caveats and red underlined risks.");
    expect(normal(pdf.pageText[0])).not.toContain("Final paragraph on page two.");
    expect(normal(pdf.pageText[1])).toContain("Final paragraph on page two.");
    // The header and footer are on the pages (as artifacts, so a screen reader skips them), and the table's cells come out in order.
    expect(normal(textRuns(pdf, pdf.pages[0], { artifacts: true }).map((r) => r.text).join(""))).toContain("confidential draft");
    expect(normal(pdf.pageText[0])).not.toContain("confidential draft");
    expect(normal(pdf.pageText[0])).toMatch(/Metric.*Q2.*Q3.*Revenue.*\$1\.2M.*\$1\.4M/);
  });

  test("bold and italic text, and bold headings and table headers, are drawn in bold and italic fonts, spaced like the screen", async ({ page }) => {
    await openFixture(page);
    const { pdf } = await downloadPdf(page);
    const runs = textRuns(pdf, pdf.pages[0]);
    const font = (word: string) => runs.find((r) => r.text.trim() === word)?.font ?? "";
    // With web fonts blocked (these tests) the standard fonts stand in: Times-Bold, Times-Italic, Helvetica-Bold.
    // With them reachable the faces of Carlito and Caladea do. Either way the style follows the run.
    expect(font("bold")).toMatch(/Bold/);
    expect(font("progress")).toMatch(/Bold/);
    expect(font("italic")).toMatch(/Italic|Oblique/);
    expect(font("Highlights")).toMatch(/Bold/);
    expect(font("Numbers")).toMatch(/Bold/);
    expect(font("Metric")).toMatch(/Bold/);
    expect(font("Q2")).toMatch(/Bold/);
    expect(font("Simplicity")).toMatch(/Italic|Oblique/);
    // Plain text is not bold or italic.
    expect(font("Revenue")).not.toMatch(/Bold|Italic|Oblique/);
    expect(font("report")).not.toMatch(/Bold|Italic|Oblique/);
    // A stand-in font is stretched to the width of the word on screen, so the letters are not spread apart.
    for (const word of ["bold", "progress", "Metric", "Highlights", "Simplicity"]) {
      const run = runs.find((r) => r.text.trim() === word)!;
      expect(Math.abs(run.tc), `${word} spacing`).toBeLessThan(0.6);
      expect(run.tz, `${word} scale`).toBeGreaterThan(60);
      expect(run.tz, `${word} scale`).toBeLessThan(160);
    }
  });

  test("the text is real text: every font has a ToUnicode map or is a standard font, and non-printing marks stay out", async ({ page }) => {
    await openFixture(page);
    await page.keyboard.press(`${MOD}+Alt+P`);
    await expect(page.locator(".np-layer")).toBeAttached();
    const { pdf } = await downloadPdf(page);
    const fonts = [...pdf.objects.values()].filter((o) => /\/Type \/Font\b/.test(o.dict) && /\/Subtype \/(Type0|Type1)/.test(o.dict));
    expect(fonts.length).toBeGreaterThan(0);
    for (const f of fonts) expect(/\/ToUnicode \d+ 0 R/.test(f.dict) || /\/Subtype \/Type1/.test(f.dict)).toBe(true);
    // Fonts that were embedded are subsets of TrueType files.
    for (const f of fonts.filter((x) => /Type0/.test(x.dict))) expect(f.dict).toMatch(/\/BaseFont \/[A-Z]{6}\+/);
    const all = pdf.pageText.join("");
    expect(all).not.toContain("¶");
    expect(all).not.toContain("·");
  });

  test("pictures, the table's borders and a link are in the file; the headings make bookmarks", async ({ page }) => {
    await openFixture(page);
    const { pdf } = await downloadPdf(page);
    // The picture is an image object, drawn on the first page.
    const images = [...pdf.objects.values()].filter((o) => /\/Subtype \/Image/.test(o.dict));
    expect(images.length).toBeGreaterThan(0);
    expect(pdf.text).toMatch(/\/XObject << \/Im1 \d+ 0 R/);
    // The dashboard link is an annotation with its address.
    expect(pdf.text).toMatch(/\/Subtype \/Link[^>]*\/S \/URI \/URI \(https?:\/\//);
    // Headings as bookmarks, with their titles and a page to go to.
    expect(pdf.text).toMatch(/\/Type \/Outlines/);
    expect(pdf.text).toMatch(/\/Title \(Highlights\)[^>]*\/Dest \[\d+ 0 R \/XYZ/);
    expect(pdf.text).toMatch(/\/Title \(Appendix\)/);
  });

  test("a JPEG goes in as it is, a PNG with transparency gets a mask", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await inPage(page, async (w) => {
      const make = async (type: string, draw: (g: CanvasRenderingContext2D) => void) => {
        const c = document.createElement("canvas");
        c.width = 60;
        c.height = 40;
        draw(c.getContext("2d")!);
        const blob = await new Promise<Blob>((r) => c.toBlob((b) => r(b!), type, 0.9));
        return new Uint8Array(await blob.arrayBuffer());
      };
      const eng = w.engine!;
      const last = () => eng.renderAll().order.at(-1)!;
      const jpg = await make("image/jpeg", (g) => {
        g.fillStyle = "#2a7";
        g.fillRect(0, 0, 60, 40);
      });
      const png = await make("image/png", (g) => {
        g.fillStyle = "rgba(200,30,30,0.5)";
        g.fillRect(10, 10, 30, 20);
      });
      w.fe.editor.exec({ op: "insert_paragraph", text: "" });
      w.fe.editor.exec({ op: "insert_image", at: { p: last(), o: 0 }, media: eng.addMedia("jpg", jpg), width: 60, height: 40 });
      w.fe.editor.exec({ op: "insert_paragraph", text: "" });
      w.fe.editor.exec({ op: "insert_image", at: { p: last(), o: 0 }, media: eng.addMedia("png", png), width: 60, height: 40 });
    });
    await expect(page.locator(".flow .atom.img")).toHaveCount(2);
    const { pdf } = await downloadPdf(page);
    expect(pdf.text).toMatch(/\/Filter \/DCTDecode/);
    const masked = [...pdf.objects.values()].filter((o) => /\/Subtype \/Image/.test(o.dict) && /\/SMask \d+ 0 R/.test(o.dict));
    expect(masked).toHaveLength(1);
    // Both pictures are painted on the page.
    expect(pageContent(pdf, pdf.pages[0]).match(/ Do Q/g)!.length).toBe(2);
  });

  test("a landscape document gives landscape pages", async ({ page }) => {
    await openFixture(page);
    await chooseMenu(page, "Format", "Page orientation", "Landscape");
    const { pdf } = await downloadPdf(page);
    for (const p of pdf.pages) expect(mediaBox(pdf, p)).toEqual([792, 612]);
  });

  test("the title and author come from the document's details", async ({ page }) => {
    await openFixture(page);
    const { pdf } = await downloadPdf(page);
    expect(pdf.info).toMatch(/\/Title \(Docxtor fixture\)/);
    expect(pdf.info).toMatch(/\/Author \(python-docx\)/);
    expect(pdf.info).toMatch(/\/Producer \(Docxtor\)/);
  });

  test("a font an extension provides is embedded as a subset, and its characters are searchable", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const font = readFileSync(resolve(process.cwd(), "packages/editor/test/fixtures/test-font.ttf")).toString("base64");
    await inPage(
      page,
      async (w, b64) => {
        const fe = w.fe as unknown as { use(x: unknown): Promise<void> };
        await fe.use({
          id: "test.font",
          name: "Test font",
          activate(ctx: { fonts: { register(f: unknown): void } }) {
            ctx.fonts.register({ family: "Docxtor Test", faces: [{ src: `data:font/ttf;base64,${b64}` }] });
          },
        });
        const eng = w.engine!;
        w.fe.editor.exec({ op: "insert_paragraph", text: "ABC CBA BAC" });
        const id = eng.renderAll().order.at(-1)!;
        w.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 11 }, format: { font: "Docxtor Test" } });
        await document.fonts.load('16px "Docxtor Test"', "ABC");
      },
      font,
    );
    const { pdf } = await downloadPdf(page);
    expect(pdf.text).toMatch(/\/BaseFont \/[A-Z]{6}\+DocxtorTest-Regular/);
    expect(pdf.text).toMatch(/\/FontFile2 \d+ 0 R/);
    expect(normal(pdf.pageText[0])).toContain("ABC CBA BAC");
  });

  test("the command palette has Download as PDF", async ({ page }) => {
    await openFixture(page);
    await page.keyboard.press(`${MOD}+Shift+P`);
    await page.keyboard.type("as pdf");
    const option = page.getByRole("option", { name: /Download as PDF/ });
    await expect(option).toBeVisible();
    const [download] = await Promise.all([page.waitForEvent("download"), page.keyboard.press("Enter")]);
    expect(download.suggestedFilename()).toBe("rich.pdf");
  });

  test("the exporter is registered under the id pdf, for embedders", async ({ page }) => {
    await openFixture(page);
    const out = await inPage(page, async (w) => {
      const f = await (w.fe as unknown as { export(id: string): Promise<{ name: string; mime: string; bytes: Uint8Array }> }).export("pdf");
      return { name: f.name, mime: f.mime, head: String.fromCharCode(...f.bytes.subarray(0, 5)) };
    });
    expect(out).toEqual({ name: "rich.pdf", mime: "application/pdf", head: "%PDF-" });
  });
});
