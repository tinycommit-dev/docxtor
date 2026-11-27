// How paragraph formatting shows on the page: every border style, the between and bar borders, patterned and theme
// shading, distributed text, spacing in lines, indents in characters, mirrored indents and automatic spacing.
// Checked in computed styles, with the pictures of the painted styles looked at by eye while writing these.
import { test, expect, type Page } from "./helpers";
import { openBuilt, type SectionSpec } from "./support/docx-builder";

type P = SectionSpec["paragraphs"][number];

const open = (page: Page, paragraphs: P[], extra: { settings?: string } = {}) => openBuilt(page, { sections: [{ paragraphs }], ...extra });

const box = (val: string, sz = 8) => `<w:pBdr>${["top", "left", "bottom", "right"].map((s) => `<w:${s} w:val="${val}" w:sz="${sz}" w:space="4" w:color="1F4E79"/>`).join("")}</w:pBdr>`;

const style = (page: Page, id: number, ...props: string[]) =>
  page.locator(`.flow [data-p="${id}"]`).evaluate((e, ps) => Object.fromEntries(ps.map((p) => [p, getComputedStyle(e).getPropertyValue(p)])), props);

test.describe("border styles", () => {
  test("the styles CSS has are drawn by CSS", async ({ page }) => {
    const css: [string, string][] = [
      ["single", "solid"],
      ["double", "double"],
      ["dotted", "dotted"],
      ["dashed", "dashed"],
      ["threeDEmboss", "ridge"],
      ["threeDEngrave", "groove"],
      ["outset", "outset"],
      ["inset", "inset"],
      ["thick", "solid"],
    ];
    await open(page, css.map(([v]) => ({ text: v, ppr: box(v) })));
    for (const [i, [val, want]] of css.entries()) {
      const s = await style(page, i + 1, "border-top-style", "border-top-width", "background-image");
      expect(s["border-top-style"], val).toBe(want);
      expect(s["background-image"], val).toBe("none");
    }
    // A double line is three times a single one's width (two lines and the gap), and thick is heavier than single.
    const widths = await Promise.all([1, 2, 9].map((id) => style(page, id, "border-top-width")));
    const [single, dbl, thick] = widths.map((w) => parseFloat(w["border-top-width"]));
    expect(dbl).toBeGreaterThanOrEqual(single * 2.9);
    expect(thick).toBeGreaterThan(single * 1.5);
  });

  test("triple, the thin-thick pairs, dot-dash and wave reserve their width and paint it as an image", async ({ page }) => {
    const painted = ["triple", "thinThickSmallGap", "thickThinMediumGap", "thinThickThinLargeGap", "dotDash", "dotDotDash", "dashSmallGap", "wave", "doubleWave"];
    await open(page, painted.map((v) => ({ text: v, ppr: box(v) })));
    for (const [i, val] of painted.entries()) {
      const s = await style(page, i + 1, "border-top-style", "border-top-color", "border-top-width", "border-left-width", "background-image", "background-origin");
      expect(s["border-top-style"], val).toBe("solid");
      expect(s["border-top-color"], val).toBe("rgba(0, 0, 0, 0)");
      expect(parseFloat(s["border-top-width"]), val).toBeGreaterThan(0.7);
      expect(parseFloat(s["border-left-width"]), val).toBeGreaterThan(0.7);
      expect(s["background-image"], val).toMatch(/gradient|svg/);
      // Four layers, one for each edge.
      expect(s["background-image"].match(/url\(|linear-gradient\(/g)?.length, val).toBe(4);
    }
    // Triple is five line widths across, the line and gaps of the thin-thick pair add up the same way.
    const t = await style(page, 1, "border-top-width");
    const base = (8 / 8) * (96 / 72);
    expect(Math.abs(parseFloat(t["border-top-width"]) - 5 * base)).toBeLessThan(1);
    const tt = await style(page, 2, "border-top-width");
    expect(Math.abs(parseFloat(tt["border-top-width"]) - 4 * base)).toBeLessThan(1);
    // A wave is an SVG; a double wave is taller than a single one.
    const w = await style(page, 8, "background-image", "border-top-width");
    const dw = await style(page, 9, "border-top-width");
    expect(w["background-image"]).toContain("data:image/svg+xml");
    expect(parseFloat(dw["border-top-width"])).toBeGreaterThan(parseFloat(w["border-top-width"]));
  });

  test("a border keeps its colour and width in the painted layers, and the text stays where it was", async ({ page }) => {
    await open(page, ["Plain", { text: "Triple", ppr: `${box("triple")}<w:ind w:left="720"/>` }]);
    const s = await style(page, 2, "background-image");
    expect(s["background-image"]).toContain("rgb(31, 78, 121)");
    const plain = await page.locator('.flow [data-p="1"]').evaluate((e) => e.getBoundingClientRect().left);
    // The text of the bordered paragraph is 0.5 inch (48 px) in, the border outside it.
    const text = await page.locator('.flow [data-p="2"]').evaluate((e) => {
      const r = document.createRange();
      r.selectNodeContents(e.querySelector(".s")!);
      return r.getBoundingClientRect().left;
    });
    expect(Math.abs(text - plain - 48)).toBeLessThan(1);
  });
});

test.describe("between and bar borders", () => {
  const same = `<w:pBdr><w:top w:val="single" w:sz="8" w:space="2" w:color="000000"/><w:left w:val="single" w:sz="8" w:space="4" w:color="000000"/><w:bottom w:val="single" w:sz="8" w:space="2" w:color="000000"/><w:right w:val="single" w:sz="8" w:space="4" w:color="000000"/>`;

  test("a between border is drawn between paragraphs that share their borders, and not above the first or below the last", async ({ page }) => {
    const bdr = `${same}<w:between w:val="dashed" w:sz="8" w:space="2" w:color="C00000"/></w:pBdr>`;
    await open(page, [{ text: "One", ppr: bdr }, { text: "Two", ppr: bdr }, { text: "Three", ppr: bdr }, "After"]);
    const [a, b, c] = await Promise.all([1, 2, 3].map((id) => style(page, id, "border-top-style", "border-top-color", "border-bottom-style")));
    expect(a["border-top-style"]).toBe("solid");
    expect(a["border-bottom-style"]).toBe("none");
    // The second and third paragraphs start with the between line.
    expect([b["border-top-style"], b["border-top-color"]]).toEqual(["dashed", "rgb(192, 0, 0)"]);
    expect([c["border-top-style"], c["border-top-color"]]).toEqual(["dashed", "rgb(192, 0, 0)"]);
    expect(b["border-bottom-style"]).toBe("none");
    expect(c["border-bottom-style"]).toBe("solid");
  });

  test("without a between border the paragraphs run together, and different borders are separate boxes", async ({ page }) => {
    await open(page, [{ text: "One", ppr: `${same}</w:pBdr>` }, { text: "Two", ppr: `${same}</w:pBdr>` }, { text: "Other", ppr: `${same}<w:between w:val="dashed" w:sz="8" w:space="2" w:color="C00000"/></w:pBdr>` }]);
    const b = await style(page, 2, "border-top-style");
    expect(b["border-top-style"]).toBe("none");
    const c = await style(page, 3, "border-top-style");
    expect(c["border-top-style"]).toBe("solid");
  });

  test("the bar border is a line outside the text, on the left", async ({ page }) => {
    await open(page, [{ text: "Barred", ppr: '<w:pBdr><w:bar w:val="single" w:sz="12" w:space="6" w:color="2E7D32"/></w:pBdr><w:ind w:left="720"/>' }]);
    const p = page.locator('.flow [data-p="1"]');
    await expect(p).toHaveAttribute("data-bar", "1");
    const bar = await p.evaluate((e) => {
      const b = getComputedStyle(e, "::before");
      return { w: b.borderLeftWidth, c: b.borderLeftColor, left: parseFloat(b.left), content: b.content };
    });
    expect(bar.c).toBe("rgb(46, 125, 50)");
    expect(parseFloat(bar.w)).toBeCloseTo(2, 1);
    // 0.5 inch of indent, less the 6 points of space and the line's width: left of the text.
    expect(bar.left).toBeCloseTo(48 - 6 * (96 / 72) - 2, 0);
  });
});

test.describe("shading", () => {
  test("a pattern is drawn over its fill, in its colour", async ({ page }) => {
    const pats = ["pct5", "pct25", "pct50", "pct90", "horzStripe", "vertStripe", "diagStripe", "reverseDiagStripe", "horzCross", "diagCross", "thinHorzStripe", "thinDiagCross"];
    await open(page, pats.map((v) => ({ text: v, ppr: `<w:shd w:val="${v}" w:color="1F4E79" w:fill="FFE699"/>` })));
    for (const [i, v] of pats.entries()) {
      const s = await style(page, i + 1, "background-color", "background-image");
      expect(s["background-color"], v).toBe("rgb(255, 230, 153)");
      expect(s["background-image"], v).toContain("data:image/svg+xml");
      // The pattern is in the pattern's colour (the SVG says it in hex).
      expect(decodeURIComponent(s["background-image"]).toLowerCase(), v).toContain("#1f4e79");
    }
    // More dots for a higher percentage.
    const dots = async (id: number) => decodeURIComponent((await style(page, id, "background-image"))["background-image"]).match(/<rect /g)?.length ?? 0;
    expect(await dots(1)).toBe(1);
    expect(await dots(2)).toBe(4);
    expect(await dots(3)).toBe(8);
    expect(await dots(4)).toBe(14);
  });

  test("solid shading is its colour, and a plain fill is as before", async ({ page }) => {
    await open(page, [{ text: "Solid", ppr: '<w:shd w:val="solid" w:color="C00000" w:fill="auto"/>' }, { text: "Fill", ppr: '<w:shd w:val="clear" w:color="auto" w:fill="DDEEFF"/>' }]);
    expect((await style(page, 1, "background-color", "background-image"))["background-color"]).toBe("rgb(192, 0, 0)");
    const f = await style(page, 2, "background-color", "background-image");
    expect(f["background-color"]).toBe("rgb(221, 238, 255)");
    expect(f["background-image"]).toBe("none");
  });

  test("theme colours are resolved, with their tint and shade", async ({ page }) => {
    await open(page, [
      { text: "Tint", ppr: '<w:shd w:val="clear" w:color="auto" w:fill="FFFFFF" w:themeFill="accent1" w:themeFillTint="33"/>' },
      { text: "Shade", ppr: '<w:shd w:val="clear" w:color="auto" w:fill="FFFFFF" w:themeFill="accent2" w:themeFillShade="80"/>' },
      { text: "Pattern", ppr: '<w:shd w:val="pct50" w:color="000000" w:themeColor="accent3" w:fill="FFFFFF" w:themeFill="accent4"/>' },
    ]);
    // The Office theme's accent 1 is 5B9BD5: a fifth of it over white.
    expect((await style(page, 1, "background-color"))["background-color"]).toBe("rgb(222, 235, 246)");
    expect((await style(page, 2, "background-color"))["background-color"]).toBe("rgb(132, 60, 12)");
    const p = await style(page, 3, "background-color", "background-image");
    expect(p["background-color"]).toBe("rgb(255, 192, 0)");
    expect(decodeURIComponent(p["background-image"]).toLowerCase()).toContain("#a5a5a5");
  });

  test("the Borders and shading dialog leaves a pattern and an unusual border alone when only something else changes", async ({ page }) => {
    await open(page, [{ text: "Keep", ppr: `${box("triple")}<w:shd w:val="diagStripe" w:color="1F4E79" w:fill="FFE699"/>` }]);
    await page.locator('.flow [data-p="1"]').click();
    const { chooseMenu } = await import("./helpers");
    await chooseMenu(page, "Format", "Borders and shading…");
    await page.getByRole("dialog", { name: "Borders and shading" }).getByRole("button", { name: "Apply" }).click();
    const s = await style(page, 1, "background-image");
    expect(decodeURIComponent(s["background-image"])).toContain("<path");
    expect((await style(page, 1, "border-top-width"))["border-top-width"]).not.toBe("0px");
    const para = await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { engine: { formatAt(p: { p: number; o: number }): { para: { borders: { top: { val: string } }; shd: { val: string } } } } } } } }).docxtor.fe.editor.engine.formatAt({ p: 1, o: 0 }).para);
    expect(para.borders.top.val).toBe("triple");
    expect(para.shd.val).toBe("diagStripe");
  });
});

test.describe("spacing and indents", () => {
  test("distributed text spreads its last line", async ({ page }) => {
    await open(page, [{ text: "Short line", ppr: '<w:jc w:val="distribute"/>' }, { text: "Justified", ppr: '<w:jc w:val="both"/>' }]);
    const a = await style(page, 1, "text-align", "text-align-last");
    expect([a["text-align"], a["text-align-last"]]).toEqual(["justify", "justify"]);
    const b = await style(page, 2, "text-align", "text-align-last");
    expect([b["text-align"], b["text-align-last"]]).toEqual(["justify", "auto"]);
    // The line really fills the width: its text reaches the right margin.
    const gap = await page.locator('.flow [data-p="1"]').evaluate((e) => {
      const r = document.createRange();
      r.selectNodeContents(e.querySelector(".s")!);
      return e.getBoundingClientRect().right - r.getBoundingClientRect().right;
    });
    expect(gap).toBeLessThan(3);
  });

  test("spacing in lines is as many lines of the paragraph's own font", async ({ page }) => {
    await open(page, [
      { text: "Plain", ppr: '<w:spacing w:before="0" w:after="0"/>' },
      { text: "One line", ppr: '<w:spacing w:beforeLines="100" w:before="0" w:after="0"/>' },
      { text: "Half", ppr: '<w:spacing w:beforeLines="50" w:after="0"/>' },
      { text: "Wins over twips", ppr: '<w:spacing w:beforeLines="200" w:before="20" w:after="0"/>' },
    ]);
    const m = async (id: number) => parseFloat((await style(page, id, "margin-top"))["margin-top"]);
    const one = await m(2);
    // 11 pt text: a line is about 17 px.
    expect(one).toBeGreaterThan(14);
    expect(one).toBeLessThan(21);
    expect(await m(3)).toBeCloseTo(one / 2, 0);
    expect(await m(4)).toBeCloseTo(one * 2, 0);
  });

  test("indents in characters are that many of the paragraph's font size, and win over twips", async ({ page }) => {
    await open(page, [
      { text: "Chars", ppr: '<w:ind w:leftChars="200" w:left="720" w:rightChars="100" w:firstLineChars="150"/>' },
      { text: "Hanging", ppr: '<w:ind w:leftChars="300" w:hangingChars="100"/>' },
    ]);
    const a = await style(page, 1, "padding-left", "padding-right", "text-indent", "font-size");
    // A character is the paragraph's font size.
    const char = parseFloat(a["font-size"]);
    expect(parseFloat(a["padding-left"])).toBeCloseTo(2 * char, 1);
    expect(parseFloat(a["padding-right"])).toBeCloseTo(char, 1);
    expect(parseFloat(a["text-indent"])).toBeCloseTo(1.5 * char, 1);
    const b = await style(page, 2, "padding-left", "text-indent");
    expect(parseFloat(b["padding-left"])).toBeCloseTo(3 * char, 1);
    expect(parseFloat(b["text-indent"])).toBeCloseTo(-char, 1);
  });

  test("mirrored indents swap on even pages", async ({ page }) => {
    await open(page, [
      { text: "Odd page", ppr: '<w:ind w:left="1440" w:right="360"/><w:mirrorIndents/>' },
      { text: "Even page", ppr: '<w:pageBreakBefore/><w:ind w:left="1440" w:right="360"/><w:mirrorIndents/>' },
      { text: "Third page", ppr: '<w:pageBreakBefore/><w:ind w:left="1440" w:right="360"/><w:mirrorIndents/>' },
      { text: "Not mirrored", ppr: '<w:pageBreakBefore/><w:ind w:left="1440" w:right="360"/>' },
    ]);
    const pads = async (id: number) => {
      const s = await style(page, id, "padding-left", "padding-right");
      return [s["padding-left"], s["padding-right"]];
    };
    expect(await pads(1)).toEqual(["96px", "24px"]);
    expect(await pads(2)).toEqual(["24px", "96px"]);
    expect(await pads(3)).toEqual(["96px", "24px"]);
    expect(await pads(4)).toEqual(["96px", "24px"]);
  });

  test("automatic spacing is 14 points, and two automatic spaces meet as one, as in HTML", async ({ page }) => {
    await open(page, [
      { text: "One", ppr: '<w:spacing w:before="0" w:after="0" w:afterAutospacing="1"/>' },
      { text: "Two", ppr: '<w:spacing w:beforeAutospacing="1" w:before="0" w:after="0"/>' },
      { text: "Three", ppr: '<w:spacing w:before="200" w:after="200"/>' },
      { text: "Four", ppr: '<w:spacing w:before="200" w:after="0"/>' },
    ]);
    const m = async (id: number) => parseFloat((await style(page, id, "margin-top"))["margin-top"]);
    // 14 pt is 18.67 px; the after of the first and the before of the second collapse into one.
    expect(await m(2)).toBeCloseTo(280 / 15, 1);
    // Without automatic spacing the two add up.
    expect(await m(4)).toBeCloseTo(400 / 15, 1);
  });
});
