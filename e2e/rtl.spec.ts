// Right-to-left text: complex-script size, bold and italic, mirrored tables, list labels on the right, the right-to-left
// gutter, and what left, right, start and end mean in a right-to-left paragraph. Arabic and Hebrew text throughout.
import { test, expect, inPage, type Page } from "./helpers";
import { openBuilt, type SectionSpec } from "./support/docx-builder";

type P = SectionSpec["paragraphs"][number];

const open = (page: Page, paragraphs: P[], extra: Partial<SectionSpec> = {}) => openBuilt(page, { sections: [{ paragraphs, ...extra }] });

/** Where the first text of a paragraph is, in client px. */
const textBox = (page: Page, id: number) =>
  page.locator(`.flow [data-p="${id}"]`).evaluate((e) => {
    const r = document.createRange();
    r.selectNodeContents(e.querySelector(".s")!);
    const b = r.getBoundingClientRect();
    const p = e.getBoundingClientRect();
    return { left: b.left, right: b.right, pLeft: p.left, pRight: p.right, pw: p.width };
  });

test.describe("complex-script formatting", () => {
  test("Arabic and Hebrew take the complex-script size, bold and italic; Latin text keeps its own", async ({ page }) => {
    await open(page, [
      {
        raw: '<w:pPr><w:bidi/></w:pPr><w:r><w:rPr><w:b/><w:i/><w:sz w:val="22"/><w:szCs w:val="40"/></w:rPr><w:t xml:space="preserve">Hello </w:t></w:r><w:r><w:rPr><w:b/><w:i/><w:sz w:val="22"/><w:szCs w:val="40"/></w:rPr><w:t xml:space="preserve">مرحبا بالعالم </w:t></w:r><w:r><w:rPr><w:b/><w:bCs/><w:i/><w:iCs/><w:sz w:val="22"/><w:szCs w:val="28"/></w:rPr><w:t>שלום עולם</w:t></w:r>',
      },
      "end",
    ]);
    const segs = await page.locator('.flow [data-p="1"] .s').evaluateAll((els) => els.map((e) => ({ text: e.textContent, size: getComputedStyle(e).fontSize, weight: getComputedStyle(e).fontWeight, italic: getComputedStyle(e).fontStyle })));
    const by = (t: string) => segs.find((s) => s.text?.includes(t))!;
    // (Sizes are compared as numbers: Firefox rounds a font size to 1/64 px and Safari writes the float, 14.666667px.)
    const size = (t: string) => parseFloat(by(t).size);
    expect(by("Hello")).toMatchObject({ weight: "700", italic: "italic" });
    expect(size("Hello")).toBeCloseTo(14.6667, 1);
    // The Arabic has szCs 20 pt but no bCs or iCs: large, plain, upright.
    expect(by("مرحبا")).toMatchObject({ weight: "400", italic: "normal" });
    expect(size("مرحبا")).toBeCloseTo(26.6667, 1);
    // The Hebrew has them all.
    expect(by("שלום")).toMatchObject({ weight: "700", italic: "italic" });
    expect(size("שלום")).toBeCloseTo(18.6667, 1);
  });

  test("a run marked right to left is complex script throughout, and reads right to left", async ({ page }) => {
    await open(page, [{ raw: '<w:r><w:rPr><w:rtl/><w:b/><w:bCs/><w:sz w:val="22"/><w:szCs w:val="32"/></w:rPr><w:t>(1) 2024 abc</w:t></w:r>' }, "x"]);
    const s = await page.locator('.flow [data-p="1"] .s').first().evaluate((e) => ({ size: getComputedStyle(e).fontSize, weight: getComputedStyle(e).fontWeight, dir: getComputedStyle(e).direction }));
    // (Firefox rounds a font size to 1/64 px and Safari writes the float, so the size is compared as a number.)
    expect({ ...s, size: parseFloat(s.size) }).toEqual({ size: expect.closeTo(21.3333, 1), weight: "700", dir: "rtl" });
  });

  test("typing keeps both sizes: bold on Arabic text writes bCs as well", async ({ page }) => {
    await open(page, [{ raw: '<w:r><w:t>مرحبا</w:t></w:r>' }, "x"]);
    await inPage(page, (w) => w.fe.editor.exec({ op: "format_text", from: { p: 1, o: 0 }, to: { p: 1, o: 5 }, format: { bold: true } }));
    const bold = await page.locator('.flow [data-p="1"] .s').first().evaluate((e) => getComputedStyle(e).fontWeight);
    expect(bold).toBe("700");
  });
});

test.describe("right-to-left paragraphs", () => {
  test("left and right mean the start and the end of the line, so they swap; start and end do not", async ({ page }) => {
    const text = "שלום עולם";
    const jcs = ["left", "right", "start", "end", "center"];
    await open(page, [...jcs.map((jc) => ({ text, ppr: `<w:bidi/><w:jc w:val="${jc}"/>` })), { text, ppr: "<w:bidi/>" }, { text: "abc", ppr: '<w:jc w:val="left"/>' }, { text: "abc", ppr: '<w:jc w:val="right"/>' }]);
    const side = async (id: number) => {
      const b = await textBox(page, id);
      const toLeft = b.left - b.pLeft;
      const toRight = b.pRight - b.right;
      return Math.abs(toLeft - toRight) < 6 ? "center" : toLeft < toRight ? "left" : "right";
    };
    // In a right-to-left paragraph: left is the start, which is the right side.
    expect(await side(1)).toBe("right");
    expect(await side(2)).toBe("left");
    expect(await side(3)).toBe("right");
    expect(await side(4)).toBe("left");
    expect(await side(5)).toBe("center");
    expect(await side(6)).toBe("right");
    // In an ordinary paragraph they mean what they say.
    expect(await side(7)).toBe("left");
    expect(await side(8)).toBe("right");
  });

  test("indents are at the start of the line, which is the right", async ({ page }) => {
    await open(page, [{ text: "שלום עולם", ppr: '<w:bidi/><w:ind w:left="1440" w:right="720" w:hanging="360"/>' }]);
    const s = await page.locator('.flow [data-p="1"]').evaluate((e) => ({ l: getComputedStyle(e).paddingLeft, r: getComputedStyle(e).paddingRight, ti: getComputedStyle(e).textIndent }));
    expect(s).toEqual({ l: "48px", r: "96px", ti: "-24px" });
    const b = await textBox(page, 1);
    // The first line hangs 0.25 inch out to the right of the 1-inch indent: its right edge is 72 px in.
    expect(b.pRight - b.right).toBeCloseTo(72, 0);
  });

  test("a numbered list label sits on the right of a right-to-left paragraph", async ({ page }) => {
    await open(page, [{ text: "مرحبا بالعالم", ppr: "<w:bidi/>" }, "x"]);
    await inPage(page, (w) => w.fe.editor.exec({ op: "toggle_list", ids: [1], kind: "decimal" }));
    const lbl = await page.locator('.flow [data-p="1"] .lbl').evaluate((e) => {
      const r = e.getBoundingClientRect();
      const p = e.closest(".fp")!.getBoundingClientRect();
      return { right: r.right, pRight: p.right, left: r.left, pLeft: p.left };
    });
    expect(lbl.pRight - lbl.right).toBeLessThan(40);
    expect(lbl.left - lbl.pLeft).toBeGreaterThan(200);
    // The text comes after it, to its left.
    const b = await textBox(page, 1);
    expect(b.right).toBeLessThanOrEqual(lbl.left + 2);
  });

  test("the toolbar and the ruler follow the direction: the ruler's indent markers are on the right", async ({ page }) => {
    await open(page, [{ text: "שלום", ppr: '<w:bidi/><w:ind w:left="1440"/>' }, "x"]);
    await page.locator('.flow [data-p="1"]').click();
    const marker = () =>
      page.evaluate(() => {
        const ruler = document.querySelector(".ruler")!.getBoundingClientRect();
        const left = document.querySelector(".ruler-marker.left")!.getBoundingClientRect();
        return { x: left.left + left.width / 2 - ruler.left, w: ruler.width };
      });
    // 1 inch in from the right margin (the margin is 1 inch too).
    await expect.poll(async () => {
      const m = await marker();
      return Math.round(m.w - 96 - 96 - m.x);
    }).toBe(0);
  });
});

test.describe("right-to-left tables and gutter", () => {
  const cell = (t: string) => `<w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>${t}</w:t></w:r></w:p></w:tc>`;
  const table = (bidi: boolean, jc = "") => ({
    body: `<w:tbl><w:tblPr>${bidi ? "<w:bidiVisual/>" : ""}${jc}<w:tblW w:w="6000" w:type="dxa"/><w:tblBorders><w:left w:val="single" w:sz="24" w:space="0" w:color="FF0000"/><w:right w:val="single" w:sz="4" w:space="0" w:color="0000FF"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/></w:tblGrid><w:tr>${cell("A")}${cell("B")}</w:tr></w:tbl>`,
  });

  test("a bidiVisual table has its first column on the right, flush with the right margin", async ({ page }) => {
    await open(page, [table(false), table(true), "text"]);
    const tbl = page.locator("table.ft");
    const [plain, rtl] = await Promise.all([0, 1].map((i) => tbl.nth(i).evaluate((t) => ({ dir: getComputedStyle(t).direction, a: t.querySelectorAll("td")[0].getBoundingClientRect().left, b: t.querySelectorAll("td")[1].getBoundingClientRect().left, right: t.getBoundingClientRect().right, left: t.getBoundingClientRect().left }))));
    expect(plain.dir).toBe("ltr");
    expect(plain.a).toBeLessThan(plain.b);
    expect(rtl.dir).toBe("rtl");
    expect(rtl.a).toBeGreaterThan(rtl.b);
    // The page text area is 6.5 inches; a 6000-twip table starts at the right margin.
    const flow = await page.locator(".flow").evaluate((f) => f.getBoundingClientRect().right - parseFloat(getComputedStyle(f).paddingRight));
    expect(Math.abs(rtl.right - flow)).toBeLessThan(2);
    expect(Math.abs(plain.left - (flow - 6.5 * 96))).toBeLessThan(2);
    // Its text goes the way its own paragraphs say: left to right here.
    expect(await tbl.nth(1).locator("td").first().evaluate((td) => getComputedStyle(td).direction)).toBe("ltr");
  });

  test("a mirrored table swaps its left and right borders", async ({ page }) => {
    await open(page, [table(true), "x"]);
    const edge = (i: number) => page.locator("table.ft td").nth(i).evaluate((td) => ({ l: getComputedStyle(td).borderLeftColor, r: getComputedStyle(td).borderRightColor }));
    // `left` in the file is the start, which is the right of a mirrored table: the first cell is on the right and has the red edge.
    expect((await edge(0)).r).toBe("rgb(255, 0, 0)");
    expect((await edge(1)).l).toBe("rgb(0, 0, 255)");
  });

  test("a gutter widens the left margin, or the right one with a right-to-left gutter", async ({ page }) => {
    await open(page, ["left gutter"], { gutter: 720 });
    const pads = () => page.locator(".flow").evaluate((f) => [getComputedStyle(f).paddingLeft, getComputedStyle(f).paddingRight]);
    expect(await pads()).toEqual(["144px", "96px"]);
    await open(page, ["right gutter"], { gutter: 720, rtlGutter: true });
    expect(await pads()).toEqual(["96px", "144px"]);
    // The ruler's margins are the text's.
    const ruler = await page.evaluate(() => {
      const m = Array.from(document.querySelectorAll<HTMLElement>(".ruler-margin")).map((e) => parseFloat(e.style.width));
      return m;
    });
    expect(ruler).toEqual([96, 144]);
  });
});
