// Equations Word writes in forms the first version did not keep: linear, skewed and bar-less fractions, and a display
// equation that sits directly under the body with no paragraph around it. Both must survive being edited.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, test, type Page } from "./helpers";

const F = '<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>';
const r = (text: string) => `<m:r>${F}<m:t>${text}</m:t></m:r>`;
const fraction = (type: string, a: string, b: string) => `<m:f><m:fPr><m:type m:val="${type}"/></m:fPr><m:num>${r(a)}</m:num><m:den>${r(b)}</m:den></m:f>`;

const math = (page: Page) => page.locator(".flow .atom.math");
const popup = (page: Page) => page.locator(".eq-pop");
const field = (page: Page) => page.locator(".eq-field");
const ids = (page: Page) => page.locator(".flow [data-p]").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
const equationAt = (page: Page, p: number, o = 0) => inPage(page, (w, a) => (w.fe.editor.engine as unknown as { equationAt(p: unknown): { latex: string; display: boolean } | null }).equationAt({ p: a.p, o: a.o }), { p, o });

/** The saved file's document.xml. */
const savedXml = (page: Page) =>
  page.evaluate(async () => {
    const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } } } } }).docxtor;
    const bytes = w.fe.editor.engine.save();
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let end = bytes.length - 22;
    while (dv.getUint32(end, true) !== 0x06054b50) end--;
    let p = dv.getUint32(end + 16, true);
    const count = dv.getUint16(end + 10, true);
    for (let i = 0; i < count; i++) {
      const method = dv.getUint16(p + 10, true);
      const csize = dv.getUint32(p + 20, true);
      const nlen = dv.getUint16(p + 28, true);
      const xlen = dv.getUint16(p + 30, true);
      const clen = dv.getUint16(p + 32, true);
      const off = dv.getUint32(p + 42, true);
      const name = new TextDecoder().decode(bytes.subarray(p + 46, p + 46 + nlen));
      p += 46 + nlen + xlen + clen;
      if (name !== "word/document.xml") continue;
      const dataStart = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(dataStart, dataStart + csize);
      if (method === 0) return new TextDecoder().decode(data);
      return await new Response(new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"))).text();
    }
    return "";
  });

test.describe("linear, skewed and bar-less fractions", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            { text: "Linear ", xml: `<m:oMath>${fraction("lin", "a", "b")}</m:oMath>` },
            { text: "Skewed ", xml: `<m:oMath>${fraction("skw", "x", "y")}</m:oMath>` },
            { text: "Stacked ", xml: `<m:oMath>${fraction("noBar", "n", "k")}</m:oMath>` },
          ],
        },
      ],
    });
  });

  test("each kind is drawn as itself and shows its own LaTeX", async ({ page }) => {
    await expect(math(page)).toHaveCount(3);
    // A linear and a skewed fraction have a slash and no bar; the stacked one has a bar of no thickness.
    await expect(math(page).nth(0).locator("mfrac")).toHaveCount(0);
    await expect(math(page).nth(0).locator("mo", { hasText: "/" })).toHaveCount(1);
    await expect(math(page).nth(2).locator("mfrac[linethickness='0']")).toHaveCount(1);
    const [p0, p1, p2] = await ids(page);
    expect((await equationAt(page, p0, 7))!.latex).toBe("\\nicefrac{a}{b}");
    expect((await equationAt(page, p1, 7))!.latex).toBe("\\sfrac{x}{y}");
    expect((await equationAt(page, p2, 8))!.latex).toBe("\\genfrac{}{}{0pt}{}{n}{k}");
  });

  test("editing a linear fraction keeps it linear, in the box, on the page and in the saved file", async ({ page }) => {
    const [p0] = await ids(page);
    await math(page).nth(0).dblclick();
    await expect(popup(page)).toBeVisible();
    await expect(field(page)).toHaveValue("\\nicefrac{a}{b}");
    await expect(popup(page).locator(".eq-preview mfrac")).toHaveCount(0);
    await field(page).fill("\\nicefrac{a}{b} + c");
    await popup(page).getByRole("button", { name: "Update" }).click();
    await expect(popup(page)).toHaveCount(0);
    await expect(math(page).nth(0).locator("mfrac")).toHaveCount(0);
    await expect(math(page).nth(0).locator("mo", { hasText: "/" })).toHaveCount(1);
    expect((await equationAt(page, p0, 7))!.latex).toMatch(/^\\nicefrac\{a\}\{b\}\s*\+\s*c$/);
    const xml = await savedXml(page);
    expect(xml).toContain('<m:type m:val="lin"/>');
    // The other two are untouched.
    expect(xml).toContain('<m:type m:val="skw"/>');
    expect(xml).toContain('<m:type m:val="noBar"/>');
  });

  test("the palette's Fraction still makes a fraction with a bar, and the box accepts the other kinds as typed", async ({ page }) => {
    const [p0] = await ids(page);
    await math(page).nth(2).dblclick();
    await field(page).fill("\\genfrac{}{}{0pt}{}{n}{k} + \\frac{1}{2}");
    await expect(popup(page).locator(".eq-preview mfrac")).toHaveCount(2);
    await popup(page).getByRole("button", { name: "Update" }).click();
    await expect(math(page).nth(2).locator("mfrac")).toHaveCount(2);
    expect(p0).toBeGreaterThan(0);
  });
});

test.describe("a display equation directly under the body", () => {
  const QUADRATIC = `<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath>${r("x=1+")}${fraction("bar", "a", "b")}</m:oMath></m:oMathPara>`;

  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before the equation", { body: QUADRATIC }, "After the equation"] }] });
  });

  test("is drawn and edited like other display equations, and is saved where Word put it", async ({ page }) => {
    await expect(math(page)).toHaveCount(1);
    await expect(math(page).first()).toHaveClass(/display/);
    const [, pEq] = await ids(page);
    expect((await equationAt(page, pEq))!.display).toBe(true);
    await math(page).first().dblclick();
    await expect(popup(page)).toBeVisible();
    await expect(field(page)).toHaveValue("x=1+\\frac{a}{b}");
    await expect(popup(page).getByLabel("Display on its own line")).toBeChecked();
    await field(page).fill("y=\\sqrt{2}");
    await popup(page).getByRole("button", { name: "Update" }).click();
    await expect(popup(page)).toHaveCount(0);
    await expect(math(page).first().locator("msqrt")).toHaveCount(1);
    // Saved: still a child of the body, between the two paragraphs, with its properties.
    const xml = await savedXml(page);
    expect(xml).toMatch(/<\/w:p><m:oMathPara><m:oMathParaPr><m:jc m:val="center"\/><\/m:oMathParaPr><m:oMath>[^]*<m:rad>[^]*<\/m:oMathPara><w:p>/);
    expect(xml).not.toContain("<w:p><m:oMathPara");
    // The text around it is as it was, and undo takes the edit back.
    await expect(page.locator(".flow .fp").first()).toContainText("Before the equation");
    await page.keyboard.press("Escape");
    await inPage(page, (w) => w.fe.editor.undo());
    await expect(math(page).first().locator("msqrt")).toHaveCount(0);
    await expect(math(page).first().locator("mfrac")).toHaveCount(1);
  });

  test("text can be typed beside it, and the paragraph is then an ordinary one", async ({ page }) => {
    const [, pEq] = await ids(page);
    await inPage(page, (w, a) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
      e.select({ p: a.p, o: 1 });
      e.focus();
    }, { p: pEq });
    await page.keyboard.type(" (1)");
    await expect(page.locator(`.flow [data-p="${pEq}"]`)).toContainText("(1)");
    const xml = await savedXml(page);
    expect(xml).toMatch(/<w:p><m:oMathPara>[^]*<\/m:oMathPara><w:r>/);
  });
});
