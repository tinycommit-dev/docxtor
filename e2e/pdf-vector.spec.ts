// Charts and shapes in a PDF are drawn as vector paths and real text, not pictures, so they stay sharp at any zoom and their text can be
// selected and searched. A drawing the vector writer cannot reproduce (a blur filter, say) still comes out, as a picture.
import { readFileSync } from "node:fs";
import { openBuilt } from "./support/docx-builder";
import { CHROMIUM_ONLY, chooseMenu, expect, inPage, openFixture, test, type Page } from "./helpers";
import { pageContent, readPdf, type PdfFile } from "./support/pdf-reader";

// Chromium on Linux finds no maths font among the ones the editor asks for (even with Noto Sans Math installed), so it draws an
// equation's brackets at their plain size and puts the fraction bar on the text font's axis: there is nothing stretched to check.
const NO_MATH_FONT_ON_LINUX = "Chromium on Linux draws equations without a maths font: brackets are not stretched, and the bar follows the text font's axis";

async function download(page: Page): Promise<PdfFile> {
  const [d] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
  return readPdf(readFileSync((await d.path())!));
}

const images = (pdf: PdfFile) => [...pdf.objects.values()].filter((o) => /\/Subtype \/Image/.test(o.dict)).length;
const content = (pdf: PdfFile) => pdf.pages.map((p) => pageContent(pdf, p)).join("\n");

const addShapes = (page: Page) =>
  inPage(page, (w) => {
    const e = w.fe.editor;
    const first = e.engine!.renderAll().order[0]!;
    ["rightArrow", "ellipse", "roundRect", "star5"].forEach((geometry, o) => e.exec({ op: "insert_shape", at: { p: first, o }, geometry, w: 90, h: 60, fill: "4472C4", line: { color: "2F528F", width_pt: 2 }, floating: false }));
  });

test.describe("vector drawings in PDF", () => {
  test("shapes are paths with fill and stroke, and no pictures", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await addShapes(page);
    const pdf = await download(page);
    expect(images(pdf)).toBe(0);
    const ops = content(pdf);
    // Fill (0.267 0.447 0.769 is 4472C4) and stroke (2F528F) colours, curves for the ellipse and rounded corners.
    expect(ops).toContain("0.267 0.447 0.769 rg");
    expect(ops).toContain("0.184 0.322 0.561 RG");
    expect(ops.match(/ c\n|\bc h\b| c /g)!.length).toBeGreaterThan(8);
    expect(ops.match(/ f\* Q/g)!.length).toBeGreaterThanOrEqual(4);
  });

  test("a chart is paths and real text, no picture", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await inPage(page, (w) => {
      const e = w.fe.editor;
      const order = e.engine!.renderAll().order;
      e.exec({ op: "insert_paragraph", after: order[order.length - 1], text: "" });
      const after = e.engine!.renderAll().order;
      e.exec({ op: "insert_chart", at: { p: after[after.length - 1]!, o: 0 }, kind: "column", title: "Quarterly sales", categories: ["North", "South"], series: [{ name: "Units", values: [3, 5] }], width: 360, height: 220 });
    });
    const pdf = await download(page);
    expect(images(pdf)).toBe(0);
    expect(pdf.pageText.join(" ")).toContain("Quarterly sales");
    expect(pdf.pageText.join(" ")).toContain("North");
    expect(content(pdf)).toMatch(/ re\b| l\b/);
  });

  test("a drawing with a filter is kept as a picture", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await addShapes(page);
    await page.locator(".flow .shape-svg").first().evaluate((el) => ((el as unknown as HTMLElement).style.filter = "blur(1px)"));
    const pdf = await download(page);
    expect(images(pdf)).toBeGreaterThanOrEqual(1);
    // The other three are still paths.
    expect(content(pdf).match(/ f\* Q/g)!.length).toBeGreaterThanOrEqual(3);
  });
});

const F = '<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>';
const run = (text: string) => `<m:r>${F}<m:t>${text}</m:t></m:r>`;
const QUADRATIC =
  `<m:oMathPara><m:oMath>${run("x=")}<m:f><m:num>${run("-b±")}` +
  `<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e><m:sSup><m:e>${run("b")}</m:e><m:sup>${run("2")}</m:sup></m:sSup>${run("-4ac")}</m:e></m:rad>` +
  `</m:num><m:den>${run("2a")}</m:den></m:f></m:oMath></m:oMathPara>`;
const MATRIX = `<m:oMathPara><m:oMath><m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e><m:m><m:mr><m:e>${run("1")}</m:e><m:e>${run("0")}</m:e></m:mr><m:mr><m:e>${run("0")}</m:e><m:e>${run("1")}</m:e></m:mr><m:mr><m:e>${run("7")}</m:e><m:e>${run("9")}</m:e></m:mr></m:m></m:e></m:d></m:oMath></m:oMathPara>`;
const THIN = `<m:oMathPara><m:oMath>${run("|x|+n!")}</m:oMath></m:oMathPara>`;
/** A short top over a tall bottom (a fraction in the denominator): the gaps above and below the bar differ. */
const LOPSIDED = `<m:oMathPara><m:oMath>${run("y=")}<m:f><m:num>${run("1")}</m:num><m:den>${run("1+")}<m:f><m:num>${run("1")}</m:num><m:den>${run("x")}</m:den></m:f></m:den></m:f></m:oMath></m:oMathPara>`;
const SUM = `<m:oMathPara><m:oMath><m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="undOvr"/></m:naryPr><m:sub>${run("k=1")}</m:sub><m:sup>${run("n")}</m:sup><m:e>${run("k")}</m:e></m:nary></m:oMath></m:oMathPara>`;

test.describe("equations in PDF", () => {
  test("an equation is text and lines: its letters can be searched, the bar and the root sign are paths, and there is no picture", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: QUADRATIC }] }] });
    const pdf = await download(page);
    expect(images(pdf)).toBe(0);
    expect(pdf.pageText.join("").replace(/\s+/g, "")).toContain("x=-b±b2-4ac2a");
    const ops = content(pdf);
    // The fraction bar is a filled rectangle; the root sign a stroked path of lines.
    expect(ops).toMatch(/ re f/);
    expect(ops).toMatch(/ m [^]*? l [^]*? l [^]*? l [^]*? l S/);
  });

  test("tall brackets are drawn as lines, and the matrix inside them as text", async ({ page }) => {
    test.skip(process.platform === "linux", NO_MATH_FONT_ON_LINUX);
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: MATRIX }] }] });
    const pdf = await download(page);
    expect(images(pdf)).toBe(0);
    expect(pdf.pageText.join("").replace(/\s+/g, "")).toContain("100179");
    // Two brackets of three strokes each.
    expect(content(pdf).match(/ l [^S]*? l [^S]*? l S/g)!.length).toBeGreaterThanOrEqual(2);
  });

  test("a big sum keeps its limits and its letters as text; only a symbol no font has becomes a small picture", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: SUM }] }] });
    const pdf = await download(page);
    const text = pdf.pageText.join("").replace(/\s+/g, "");
    expect(text).toContain("k=1");
    expect(text).toContain("n");
    // At most the ∑ itself is drawn as a picture, when the fonts the PDF can embed don't have it.
    expect((content(pdf).match(/ Do\b/g) ?? []).length).toBeLessThanOrEqual(1);
  });

  test("bars, factorials and other thin operators are plain text, not stretched", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.mathml } }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: THIN }] }] });
    const pdf = await download(page);
    expect(pdf.pageText.join("").replace(/\s+/g, "")).toContain("|x|+n!");
    // A stretched operator is drawn with its own scaling matrix; none here.
    const scales = [...content(pdf).matchAll(/([\d.-]+) 0 0 ([\d.-]+) [\d.-]+ [\d.-]+ Tm/g)].map((m) => [Number(m[1]), Number(m[2])]);
    expect(scales.length).toBeGreaterThan(0);
    expect(scales.filter(([a, d]) => a !== 1 || d !== 1)).toEqual([]);
  });

  test("a fraction's bar sits on the math axis, in line with the equals sign, when its two parts differ in height", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.mathml } }, async ({ page }) => {
    test.skip(process.platform === "linux", NO_MATH_FONT_ON_LINUX);
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: LOPSIDED }] }] });
    // Where the browser draws the outer bar, against the baseline of "y=" beside it, in em.
    const screen = await page.evaluate(() => {
      const frac = document.querySelector(".flow math mfrac")!;
      const num = frac.children[0].getBoundingClientRect();
      const den = frac.children[1].getBoundingClientRect();
      return { mid: (num.bottom + den.top) / 2, gapTop: den.top - num.bottom };
    });
    expect(screen.gapTop).toBeGreaterThan(0);
    const pdf = await download(page);
    const ops = content(pdf);
    // The bars are thin filled rectangles; the outer one is the widest.
    const bars = [...ops.matchAll(/([\d.-]+) ([\d.-]+) ([\d.]+) ([\d.]+) re f/g)].map((m) => ({ y: Number(m[2]) + Number(m[4]) / 2, w: Number(m[3]), h: Number(m[4]) })).filter((b) => b.h < 3);
    expect(bars.length).toBe(2);
    const outer = bars.sort((a, b) => b.w - a.w)[0];
    // The baseline and size of the text before the fraction.
    const head = /\/\w+ ([\d.]+) Tf[^]*?([\d.-]+) ([\d.-]+) Tm|([\d.-]+) ([\d.-]+) Tm[^]*?\/\w+ ([\d.]+) Tf/.exec(ops)!;
    const size = Number(head[1] ?? head[6]);
    const baseline = Number(head[3] ?? head[5]);
    const axis = (outer.y - baseline) / size;
    expect(axis).toBeGreaterThan(0.18);
    expect(axis).toBeLessThan(0.32);
  });
});

