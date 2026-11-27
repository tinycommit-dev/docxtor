// Equations: Word's equations drawn as MathML (inline and on their own line), the equation editor (Insert, Equation: a
// LaTeX field with a live preview and a palette), editing by double-click and Enter, undo, the caret crossing an
// equation as one unit, and what copying and printing keep. The engine's side (OMML, LaTeX, ops) is tested in Rust.
import { openBuilt } from "./support/docx-builder";
import { CHROMIUM_ONLY, chooseMenu, expect, inPage, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

// -- Word's markup -----------------------------------------------------------------------------------------

const F = '<w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>';
const CTRL = `<m:ctrlPr><w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/><w:i/></w:rPr></m:ctrlPr>`;
const r = (text: string) => `<m:r>${F}<m:t>${text}</m:t></m:r>`;

/** A sum with limits, in a line of text. */
const SUM = `<m:oMath><m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="undOvr"/><m:grow m:val="1"/>${CTRL}</m:naryPr><m:sub>${r("i=1")}</m:sub><m:sup>${r("n")}</m:sup><m:e>${r("i")}</m:e></m:nary></m:oMath>`;
/** The quadratic formula on a line of its own. */
const QUADRATIC =
  `<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath>${r("x=")}<m:f><m:fPr>${CTRL}</m:fPr><m:num>${r("-b±")}` +
  `<m:rad><m:radPr><m:degHide m:val="1"/>${CTRL}</m:radPr><m:deg/><m:e><m:sSup><m:sSupPr>${CTRL}</m:sSupPr><m:e>${r("b")}</m:e><m:sup>${r("2")}</m:sup></m:sSup>${r("-4ac")}</m:e></m:rad>` +
  `</m:num><m:den>${r("2a")}</m:den></m:f></m:oMath></m:oMathPara>`;
const row = (a: string, b: string) => `<m:mr><m:e>${r(a)}</m:e><m:e>${r(b)}</m:e></m:mr>`;
const MATRIX = `<m:oMathPara><m:oMath><m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/>${CTRL}</m:dPr><m:e><m:m><m:mPr>${CTRL}</m:mPr>${row("a", "b")}${row("c", "d")}</m:m></m:e></m:d></m:oMath></m:oMathPara>`;

// -- helpers -------------------------------------------------------------------------------------------------

/** A document with an inline sum, the quadratic formula, a matrix and a closing paragraph. */
async function openWordEquations(page: Page) {
  await openBuilt(page, {
    sections: [
      {
        paragraphs: [
          { text: "Solve ", xml: `${SUM}<w:r><w:t xml:space="preserve"> now.</w:t></w:r>` },
          { text: "", xml: QUADRATIC },
          { text: "", xml: MATRIX },
          "The end.",
        ],
      },
    ],
  });
}

const ids = (page: Page) => page.locator(".flow [data-p]").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
const math = (page: Page) => page.locator(".flow .atom.math");
const popup = (page: Page) => page.locator(".eq-pop");
const field = (page: Page) => page.locator(".eq-field");
const selection = (page: Page) => inPage(page, (w) => w.fe.editor.sel);
const modelText = (page: Page, p: number) => inPage(page, (w, a) => w.fe.editor.engine!.textRange({ p: a.p, o: 0 }, { p: a.p, o: 100000 }), { p });
const equationAt = (page: Page, p: number, o: number) => inPage(page, (w, a) => (w.fe.editor.engine as unknown as { equationAt(p: unknown): { latex: string; display: boolean } | null }).equationAt({ p: a.p, o: a.o }), { p, o });
const caretAt = (page: Page, p: number, o: number) =>
  inPage(
    page,
    (w, a) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
      e.select({ p: a.p, o: a.o });
      e.focus();
    },
    { p, o },
  );
const undo = (page: Page) => page.keyboard.press(`${MOD}+Z`);

/** Open Insert, Equation from the menu bar. */
async function openEditor(page: Page) {
  await chooseMenu(page, "Insert", "Equation…");
  await expect(popup(page)).toBeVisible();
}

// -- drawing -------------------------------------------------------------------------------------------------

test.describe("Word equations on the page", () => {
  test.beforeEach(async ({ page }) => {
    await openWordEquations(page);
  });

  test("are drawn as MathML, inline in the text and on lines of their own", async ({ page }) => {
    await expect(math(page)).toHaveCount(3);
    // Every equation is a <math> element in the MathML namespace, with a box the browser has laid out.
    const info = await math(page).evaluateAll((els) =>
      els.map((e) => {
        const m = e.querySelector("math")!;
        const b = m.getBoundingClientRect();
        return { ns: m.namespaceURI, w: b.width, h: b.height, display: e.classList.contains("display"), editable: e.getAttribute("contenteditable") };
      }),
    );
    for (const i of info) {
      expect(i.ns).toBe("http://www.w3.org/1998/Math/MathML");
      expect(i.w).toBeGreaterThan(8);
      expect(i.h).toBeGreaterThan(8);
      expect(i.editable).toBe("false");
    }
    expect(info.map((i) => i.display)).toEqual([false, true, true]);
    // The sum sits in the line of text; the others are on lines of their own and centred in the text width.
    const [p0, p1] = await ids(page);
    const text = (await page.locator(`.flow [data-p="${p0}"]`).boundingBox())!;
    const sum = (await math(page).nth(0).boundingBox())!;
    expect(sum.y).toBeGreaterThanOrEqual(text.y - 2);
    expect(sum.y + sum.height).toBeLessThanOrEqual(text.y + text.height + 2);
    const para = (await page.locator(`.flow [data-p="${p1}"]`).boundingBox())!;
    const formula = (await math(page).nth(1).locator("math").boundingBox())!;
    expect(Math.abs(formula.x + formula.width / 2 - (para.x + para.width / 2))).toBeLessThan(6);
    // Fractions, radicals, superscripts, brackets and a table are in the markup.
    await expect(math(page).nth(1).locator("mfrac")).toHaveCount(1);
    await expect(math(page).nth(1).locator("msqrt")).toHaveCount(1);
    await expect(math(page).nth(1).locator("msup")).toHaveCount(1);
    await expect(math(page).nth(2).locator("mtable mtr")).toHaveCount(2);
    await expect(math(page).nth(2).locator("mo").first()).toHaveText("[");
  });

  test("hold only MathML: nothing else gets onto the page from an equation", async ({ page }) => {
    const bad = await page.evaluate(() => {
      const ok = new Set(["math", "mrow", "mi", "mn", "mo", "mtext", "mspace", "mfrac", "msqrt", "mroot", "msub", "msup", "msubsup", "munder", "mover", "munderover", "mmultiscripts", "mprescripts", "none", "mtable", "mtr", "mtd", "mphantom", "mpadded", "merror"]);
      const out: string[] = [];
      document.querySelectorAll(".flow .atom.math *").forEach((e) => {
        if (e.namespaceURI !== "http://www.w3.org/1998/Math/MathML" || !ok.has(e.localName)) out.push(e.localName);
        for (const a of Array.from(e.attributes)) if (/^on|href|style|src/i.test(a.name)) out.push(`${e.localName}@${a.name}`);
      });
      return out;
    });
    expect(bad).toEqual([]);
  });

  test("the model counts an equation as one character, and the closing paragraph is untouched", async ({ page }) => {
    const [p0] = await ids(page);
    expect(await modelText(page, p0)).toBe("Solve $\\sum_{i=1}^{n} i$ now.");
    expect(await equationAt(page, p0, 6)).toEqual({ latex: "\\sum_{i=1}^{n} i", display: false });
    expect(await equationAt(page, p0, 5)).toBeNull();
  });

  test("the caret crosses an equation in one step, like an image", async ({ page }) => {
    const [p0] = await ids(page);
    const focus = () => selection(page).then((s) => s?.focus);
    await caretAt(page, p0, 6);
    await page.keyboard.press("ArrowRight");
    await expect.poll(focus).toEqual({ p: p0, o: 7 });
    await page.keyboard.press("ArrowLeft");
    await expect.poll(focus).toEqual({ p: p0, o: 6 });
    await page.keyboard.press("ArrowLeft");
    await expect.poll(focus).toEqual({ p: p0, o: 5 });
    // Typing next to it lands on the right side of it.
    await caretAt(page, p0, 7);
    await page.keyboard.type("X");
    expect(await modelText(page, p0)).toBe("Solve $\\sum_{i=1}^{n} i$X now.");
  });

  test("a click selects the whole equation; Backspace removes it and undo brings it back", async ({ page }) => {
    const [p0] = await ids(page);
    await math(page).nth(0).click();
    const s = (await selection(page))!;
    expect([s.anchor.o, s.focus.o].sort()).toEqual([6, 7]);
    await page.keyboard.press("Backspace");
    await expect(math(page)).toHaveCount(2);
    expect(await modelText(page, p0)).toBe("Solve  now.");
    await undo(page);
    await expect(math(page)).toHaveCount(3);
    expect(await modelText(page, p0)).toBe("Solve $\\sum_{i=1}^{n} i$ now.");
  });

  test("selecting across text and an equation, then typing, replaces all of it", async ({ page }) => {
    const [p0] = await ids(page);
    await inPage(
      page,
      (w, a) => {
        const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
        e.select({ anchor: { p: a.p, o: 3 }, focus: { p: a.p, o: 9 } });
        e.focus();
      },
      { p: p0 },
    );
    await page.keyboard.type("Z");
    expect(await modelText(page, p0)).toBe("SolZow.");
  });
});

// -- the equation editor -----------------------------------------------------------------------------------------

test.describe("Insert, Equation", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["The area is  and that is all.", "A second paragraph."] }] });
  });

  test("opens at the caret with a field, a preview and a palette; Escape cancels and changes nothing", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await expect(field(page)).toBeFocused();
    await expect(popup(page).getByRole("button", { name: "Fraction" })).toBeVisible();
    await expect(popup(page).getByRole("group", { name: "Greek letters" })).toBeVisible();
    await expect(popup(page).getByRole("group", { name: "Operators" })).toBeVisible();
    await expect(popup(page).getByLabel("Display on its own line")).not.toBeChecked();
    await expect(popup(page).getByRole("button", { name: "Insert" })).toBeDisabled();
    await expect(popup(page).locator(".eq-hint")).toBeVisible();
    await field(page).fill("x^2");
    await page.keyboard.press("Escape");
    await expect(popup(page)).toHaveCount(0);
    await expect(math(page)).toHaveCount(0);
    expect(await modelText(page, p0)).toBe("The area is  and that is all.");
    // The caret is back in the text where it was.
    expect((await selection(page))!.focus).toEqual({ p: p0, o: 12 });
  });

  test("the preview follows what is typed", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await field(page).fill("x^2");
    await expect(popup(page).locator(".eq-preview msup")).toHaveCount(1);
    await field(page).fill("\\frac{a}{b} + \\sqrt{c}");
    await expect(popup(page).locator(".eq-preview mfrac")).toHaveCount(1);
    await expect(popup(page).locator(".eq-preview msqrt")).toHaveCount(1);
    await expect(popup(page).locator(".eq-preview msup")).toHaveCount(0);
    await expect(popup(page).getByRole("button", { name: "Insert" })).toBeEnabled();
  });

  test("invalid LaTeX shows what is wrong under the field and Insert waits", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await field(page).fill("\\frac{a}");
    await expect(popup(page).locator(".eq-error")).toContainText("second group");
    await expect(field(page)).toHaveAttribute("aria-invalid", "true");
    await expect(popup(page).getByRole("button", { name: "Insert" })).toBeDisabled();
    // Enter does not insert it either.
    await field(page).press("Enter");
    await expect(popup(page)).toBeVisible();
    await expect(math(page)).toHaveCount(0);
    await field(page).fill("\\bogus");
    await expect(popup(page).locator(".eq-error")).toContainText("Unknown command \\bogus");
    // Mended, the message goes.
    await field(page).fill("\\frac{a}{b}");
    await expect(popup(page).locator(".eq-error")).toBeHidden();
    await expect(field(page)).not.toHaveAttribute("aria-invalid", "true");
    await expect(popup(page).getByRole("button", { name: "Insert" })).toBeEnabled();
  });

  test("the palette puts a fraction in the field with the cursor in its numerator", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await popup(page).getByRole("button", { name: "Fraction" }).click();
    await expect(field(page)).toHaveValue("\\frac{}{}");
    await expect(field(page)).toBeFocused();
    await page.keyboard.type("a");
    await expect(field(page)).toHaveValue("\\frac{a}{}");
    // The cursor is after the "a": a Greek letter goes there.
    await popup(page).getByRole("button", { name: "\\pi" }).click();
    await expect(field(page)).toHaveValue("\\frac{a\\pi }{}");
    await expect(popup(page).locator(".eq-preview mfrac")).toHaveCount(1);
    // The structures are drawn, not named.
    await expect(popup(page).locator(".eq-struct math")).toHaveCount(8);
  });

  test("a structure wraps the text that is selected in the field", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await field(page).fill("x+1");
    await field(page).evaluate((el: HTMLTextAreaElement) => el.setSelectionRange(0, 3));
    await popup(page).getByRole("button", { name: "Square root" }).click();
    await expect(field(page)).toHaveValue("\\sqrt{x+1}");
  });

  test("Insert puts an inline equation at the caret, in one undo step", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await field(page).fill("A = \\pi r^2");
    await expect(popup(page).locator(".eq-preview msup")).toHaveCount(1);
    await popup(page).getByRole("button", { name: "Insert" }).click();
    await expect(popup(page)).toHaveCount(0);
    await expect(math(page)).toHaveCount(1);
    await expect(math(page).first()).not.toHaveClass(/display/);
    expect(await modelText(page, p0)).toBe("The area is $A=\\pi r^2$ and that is all.");
    // The caret is after it, in the text, and typing carries on there.
    expect((await selection(page))!.focus).toEqual({ p: p0, o: 13 });
    await page.keyboard.type("!");
    expect(await modelText(page, p0)).toBe("The area is $A=\\pi r^2$! and that is all.");
    await undo(page);
    await undo(page);
    await expect(math(page)).toHaveCount(0);
    expect(await modelText(page, p0)).toBe("The area is  and that is all.");
  });

  test("Enter in the field inserts too; a selection is replaced", async ({ page }) => {
    const [p0] = await ids(page);
    await inPage(
      page,
      (w, a) => {
        const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
        e.select({ anchor: { p: a.p, o: 4 }, focus: { p: a.p, o: 8 } });
        e.focus();
      },
      { p: p0 },
    );
    await openEditor(page);
    await field(page).fill("E = mc^2");
    await field(page).press("Enter");
    await expect(popup(page)).toHaveCount(0);
    expect(await modelText(page, p0)).toBe("The $E=mc^2$ is  and that is all.");
  });

  test("Display on its own line splits the paragraph and centres the equation", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 12);
    await openEditor(page);
    await field(page).fill("\\int_0^1 x^2 \\, dx = \\frac{1}{3}");
    await popup(page).getByLabel("Display on its own line").check();
    await expect(popup(page).locator(".eq-preview math")).toHaveAttribute("display", "block");
    await popup(page).getByRole("button", { name: "Insert" }).click();
    await expect(math(page)).toHaveCount(1);
    await expect(math(page).first()).toHaveClass(/display/);
    const all = await ids(page);
    expect(all).toHaveLength(4);
    expect(await modelText(page, all[0])).toBe("The area is ");
    expect(await modelText(page, all[1])).toBe("$$\\int_{0}^{1} x^2\\,dx=\\frac{1}{3}$$");
    expect(await modelText(page, all[2])).toBe(" and that is all.");
    // On a line of its own, centred in the text width.
    const para = (await page.locator(`.flow [data-p="${all[1]}"]`).boundingBox())!;
    const formula = (await math(page).first().locator("math").boundingBox())!;
    expect(Math.abs(formula.x + formula.width / 2 - (para.x + para.width / 2))).toBeLessThan(6);
    await undo(page);
    await expect(math(page)).toHaveCount(0);
    expect(await ids(page)).toHaveLength(2);
    expect(await modelText(page, p0)).toBe("The area is  and that is all.");
  });

  test("the keyboard shortcut Alt+= opens it", async ({ page }) => {
    const [p0] = await ids(page);
    await caretAt(page, p0, 3);
    await page.keyboard.press("Alt+=");
    await expect(popup(page)).toBeVisible();
    await expect(field(page)).toBeFocused();
  });

  test("Insert, Equation is in the menu with its shortcut", async ({ page }) => {
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Insert", exact: true }).click();
    const item = page.getByRole("menuitem", { name: /Equation…/ });
    await expect(item).toBeVisible();
    await expect(item).toContainText(/Alt|⌥/);
  });
});

// -- editing an equation --------------------------------------------------------------------------------------------

test.describe("editing an equation", () => {
  test.beforeEach(async ({ page }) => {
    await openWordEquations(page);
  });

  test("double-click opens the editor with its LaTeX; Update changes the equation, and undo takes it back", async ({ page }) => {
    const [p0] = await ids(page);
    await math(page).nth(0).dblclick();
    await expect(popup(page)).toBeVisible();
    await expect(field(page)).toHaveValue("\\sum_{i=1}^{n} i");
    await expect(popup(page).locator(".eq-preview munderover, .eq-preview msubsup")).toHaveCount(1);
    await expect(popup(page).getByRole("button", { name: "Update" })).toBeEnabled();
    await field(page).fill("\\prod_{k=1}^{m} k^2");
    await popup(page).getByRole("button", { name: "Update" }).click();
    await expect(popup(page)).toHaveCount(0);
    expect(await modelText(page, p0)).toBe("Solve $\\prod_{k=1}^{m} k^2$ now.");
    await expect(math(page)).toHaveCount(3);
    await expect(math(page).nth(0).locator("msup")).toHaveCount(1);
    await undo(page);
    expect(await modelText(page, p0)).toBe("Solve $\\sum_{i=1}^{n} i$ now.");
  });

  test("Enter on a selected equation opens the same editor; Escape leaves it as it was", async ({ page }) => {
    const [, p1] = await ids(page);
    await math(page).nth(1).click();
    await page.keyboard.press("Enter");
    await expect(popup(page)).toBeVisible();
    await expect(field(page)).toHaveValue("x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}");
    await expect(popup(page).getByLabel("Display on its own line")).toBeChecked();
    await field(page).fill("y");
    await page.keyboard.press("Escape");
    await expect(popup(page)).toHaveCount(0);
    expect(await equationAt(page, p1, 0)).toEqual({ latex: "x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}", display: true });
    // Back in the document, with the equation still selected.
    expect(((await selection(page))!.focus.o)).toBeGreaterThanOrEqual(0);
  });

  test("an equation can move onto a line of its own and back", async ({ page }) => {
    const [p0] = await ids(page);
    await math(page).nth(0).dblclick();
    await popup(page).getByLabel("Display on its own line").check();
    await popup(page).getByRole("button", { name: "Update" }).click();
    await expect(math(page).nth(0)).toHaveClass(/display/);
    const all = await ids(page);
    expect(await modelText(page, all[0])).toBe("Solve ");
    expect(await modelText(page, all[1])).toBe("$$\\sum_{i=1}^{n} i$$");
    expect(await modelText(page, all[2])).toBe(" now.");
    await undo(page);
    expect(await modelText(page, p0)).toBe("Solve $\\sum_{i=1}^{n} i$ now.");
    await expect(math(page).nth(0)).not.toHaveClass(/display/);
  });

  test("editing keeps the rest of the document as it was", async ({ page }) => {
    await math(page).nth(2).dblclick();
    await expect(field(page)).toHaveValue("\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}");
    await field(page).fill("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}");
    await popup(page).getByRole("button", { name: "Update" }).click();
    await expect(math(page).nth(2).locator("mtr")).toHaveCount(2);
    await expect(math(page).nth(2).locator("mo").first()).toHaveText("(");
    await expect(math(page).nth(1).locator("mfrac")).toHaveCount(1);
    await expect(math(page).nth(0).locator("msubsup, munderover")).toHaveCount(1);
  });
});

// -- copying and printing -------------------------------------------------------------------------------------------

test.describe("copying and printing", () => {
  test.beforeEach(async ({ page }) => {
    await openWordEquations(page);
  });

  test("Copy as Markdown gives $…$ for an equation in the text and $$…$$ for one on its own line", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    const [p0, p1] = await ids(page);
    await inPage(
      page,
      (w, a) => {
        const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
        e.select({ anchor: { p: a.p0, o: 0 }, focus: { p: a.p1, o: 1 } });
        e.focus();
      },
      { p0, p1 },
    );
    await chooseMenu(page, "Edit", "Copy as Markdown");
    const md = await page.evaluate(() => navigator.clipboard.readText());
    expect(md).toContain("Solve $\\sum_{i=1}^{n} i$ now.");
    expect(md).toContain("$$x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}$$");
  });

  test("pasting Markdown with $…$ makes an equation", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    const all = await ids(page);
    const last = all[all.length - 1];
    await caretAt(page, last, 4);
    await page.evaluate(() => navigator.clipboard.writeText("a $E = mc^2$ b"));
    await chooseMenu(page, "Edit", "Paste from Markdown");
    await expect(math(page)).toHaveCount(4);
    expect(await modelText(page, last)).toContain("$E=mc^2$");
  });

  test("the print copy has the equations, drawn", async ({ page }) => {
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await page.keyboard.press(`${MOD}+P`);
    const printed = page.locator(".docxtor-print .print-page");
    await expect(printed).toHaveCount(1);
    await expect(printed.locator(".atom.math math")).toHaveCount(3);
    await expect(printed.locator(".atom.math mfrac")).toHaveCount(1);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });
});

// -- the interface ----------------------------------------------------------------------------------------------------

test.describe("the equation editor in the interface", () => {
  test("the preview is a bit of page, white with dark ink, in both themes", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Hello there."] }] });
    for (const theme of ["light", "dark"] as const) {
      await page.emulateMedia({ colorScheme: theme });
      const [p0] = await ids(page);
      await caretAt(page, p0, 5);
      await openEditor(page);
      await field(page).fill("\\frac{a}{b}");
      await expect(popup(page).locator(".eq-preview mfrac")).toHaveCount(1);
      // The preview is a bit of page: white with dark ink, whatever the theme.
      const colours = await popup(page)
        .locator(".eq-preview")
        .evaluate((el) => ({ bg: getComputedStyle(el).backgroundColor, fg: getComputedStyle(el).color }));
      expect(colours.bg).toBe("rgb(255, 255, 255)");
      expect(colours.fg).toBe("rgb(32, 21, 21)");
      await page.keyboard.press("Escape");
    }
  });
});
