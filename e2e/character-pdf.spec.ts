// Download as PDF keeps character formatting: underline styles and colours, raised and lowered text, scaled text,
// outline text, a box round a run, and hidden text (left out unless it is shown).
import { readFileSync } from "node:fs";
import { chooseMenu, expect, test, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";
import { pageContent, readPdf, textRuns, type PdfFile } from "./support/pdf-reader";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;
const open = (page: Page, paragraphs: string[]) => openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })) }] });

async function downloadPdf(page: Page): Promise<PdfFile> {
  const [download] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
  return readPdf(readFileSync(await download.path()));
}

const text = (pdf: PdfFile) => textRuns(pdf, pdf.pages[0]).map((r) => r.text).join("");

test.describe("character formatting in the PDF", () => {
  test("underlines are drawn as lines in their own colour", async ({ page }) => {
    await open(page, [run('<w:u w:val="single" w:color="C00000"/>', "red line"), run('<w:u w:val="double" w:color="0000FF"/>', "blue double"), run('<w:u w:val="wave" w:color="00AA00"/>', "green wave")]);
    const pdf = await downloadPdf(page);
    const content = pageContent(pdf, pdf.pages[0]);
    // Strokes in 192/255 red, blue and 170/255 green; the wave is many short strokes.
    expect(content).toMatch(/0\.75\d* 0 0 (RG|rg)/);
    expect(content).toMatch(/0 0 1 (RG|rg)/);
    expect(content).toMatch(/0 0\.667 0 (RG|rg)/);
    const strokes = (colour: string) => (content.match(new RegExp(`${colour.replace(/\./g, "\\.")} (RG|rg)`, "g")) ?? []).length;
    expect(strokes("0 0 1")).toBeGreaterThanOrEqual(2);
    expect(strokes("0 0.667 0")).toBeGreaterThan(4);
  });

  test("raised and lowered text sits above and below the line", async ({ page }) => {
    await open(page, [run("", "aa ") + run('<w:position w:val="12"/>', "up ") + run('<w:position w:val="-12"/>', "down ") + run("", "bb")]);
    const pdf = await downloadPdf(page);
    const ys = [...pageContent(pdf, pdf.pages[0]).matchAll(/BT 1 0 [\d.-]+ 1 ([\d.-]+) ([\d.-]+) Tm/g)].map((m) => Number(m[2]));
    expect(ys).toHaveLength(4);
    // Twelve half-points are six points.
    expect(ys[1] - ys[0]).toBeGreaterThan(5);
    expect(ys[1] - ys[0]).toBeLessThan(7);
    expect(ys[0] - ys[2]).toBeGreaterThan(5);
    expect(Math.abs(ys[3] - ys[0])).toBeLessThan(0.1);
  });

  test("scaled text is stretched and still extractable", async ({ page }) => {
    await open(page, [run("", "plain ") + run('<w:w w:val="150"/>', "stretched words") + run("", " after")]);
    const pdf = await downloadPdf(page);
    expect(text(pdf).replace(/\s+/g, " ")).toContain("plain stretched words after");
    // Each scaled word is drawn under a matrix that stretches it across.
    expect(pageContent(pdf, pdf.pages[0])).toMatch(/q 1\.5 0 0 1 [\d.-]+ [\d.-]+ cm/);
  });

  test("outline text is stroked with no fill", async ({ page }) => {
    await open(page, [run("<w:outline/>", "hollow letters")]);
    const pdf = await downloadPdf(page);
    expect(pageContent(pdf, pdf.pages[0])).toMatch(/ 1 Tr /);
    expect(text(pdf)).toContain("hollow letters");
  });

  test("a box round a run is drawn on the line", async ({ page }) => {
    const bdr = '<w:bdr w:val="single" w:sz="8" w:space="1" w:color="FF0000"/>';
    await open(page, [run("", "before ") + run(bdr, "boxed words") + run("", " after")]);
    const pdf = await downloadPdf(page);
    // Four red lines drawn as rectangles (top, bottom, left, right).
    const reds = pageContent(pdf, pdf.pages[0]).match(/1 0 0 rg\n?[^\n]*re/g) ?? [];
    expect(reds.length).toBeGreaterThanOrEqual(4);
  });

  test("hidden text is left out, and drawn once it is shown", async ({ page }) => {
    await open(page, [run("", "shown ") + run("<w:vanish/>", "secret ") + run("", "end")]);
    expect(text(await downloadPdf(page)).replace(/\s+/g, " ")).toBe("shown end");
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { showHiddenText(on: boolean): void } } } }).docxtor.fe.editor.showHiddenText(true));
    expect(text(await downloadPdf(page)).replace(/\s+/g, " ")).toBe("shown secret end");
  });
});
