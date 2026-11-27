// How pages look: page borders, vertically aligned text, a gutter (at the side or the top), mirrored margins and the
// page background (a colour, gradient, pattern, texture or picture).
import { buildDocx, openBuilt } from "./support/docx-builder";
import { expect, inPage, test, type Locator, type Page } from "./helpers";

const near = (a: number, b: number, tolerance = 1.5) => expect(Math.abs(a - b)).toBeLessThanOrEqual(tolerance);
const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
const pageLayout = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup() as unknown as Record<string, unknown>);
/** Where an element is on its sheet, in CSS px (zoom taken out). */
const onSheet = async (page: Page, el: Locator, k = 0) => {
  const zoom = await inPage(page, (w) => w.fe.editor.zoom);
  const s = (await sheet(page, k).boundingBox())!;
  const b = (await el.boundingBox())!;
  return { x: (b.x - s.x) / zoom, y: (b.y - s.y) / zoom, w: b.width / zoom, h: b.height / zoom };
};
const lines = (n: number, prefix = "Line") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);
const TW = 1 / 15;

const BOX = (style: string, space: number, extra = "") => `<w:pgBorders w:offsetFrom="page"${extra}><w:top w:val="${style}" w:sz="12" w:space="${space}" w:color="FF0000"/><w:left w:val="${style}" w:sz="12" w:space="${space}" w:color="FF0000"/><w:bottom w:val="${style}" w:sz="12" w:space="${space}" w:color="FF0000"/><w:right w:val="${style}" w:sz="12" w:space="${space}" w:color="FF0000"/></w:pgBorders>`;

test.describe("page borders", () => {
  test("a box round the page, in the colour, width and distance the file gives", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Bordered page"], xml: { afterMar: BOX("single", 24) } }] });
    const border = page.locator(".sheet .pgb").first();
    await expect(border).toHaveCount(1);
    const css = await border.evaluate((el) => {
      const c = getComputedStyle(el);
      return { top: c.borderTopStyle, topW: c.borderTopWidth, color: c.borderTopColor, left: c.borderLeftStyle, bottom: c.borderBottomStyle, right: c.borderRightStyle };
    });
    expect(css).toMatchObject({ top: "solid", left: "solid", bottom: "solid", right: "solid", color: "rgb(255, 0, 0)" });
    // 12 eighths of a point is 1.5 pt.
    near(parseFloat(css.topW), 2, 0.1);
    // From the page edge, 24 pt in on every side.
    const b = await onSheet(page, border);
    near(b.x, 32);
    near(b.y, 32);
    const s = (await pageLayout(page)).sections as { pageBorders: { offsetFrom: string; display: string; top: { val: string; space: number } } }[];
    expect(s[0].pageBorders).toMatchObject({ offsetFrom: "page", display: "allPages", top: { val: "single", space: 24 } });
    await page.screenshot({ path: "test-results/page-borders-box.png" });
  });

  test("measured from the text, the border sits outside the margins", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Bordered page"], xml: { afterMar: BOX("double", 8).replace('offsetFrom="page"', 'offsetFrom="text"') } }] });
    const b = await onSheet(page, page.locator(".sheet .pgb").first());
    // One inch margin, 8 points out, but never past the header at half an inch for the top.
    near(b.x, 96 - 8 * (96 / 72));
    near(b.w, 816 - 2 * (96 - 8 * (96 / 72)));
    expect(await page.locator(".sheet .pgb").first().evaluate((el) => getComputedStyle(el).borderLeftStyle)).toBe("double");
  });

  test("first page only, and every page but the first", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First page{PBR}", "Second page"], xml: { afterMar: BOX("single", 24, ' w:display="firstPage"') } }] });
    await expect(page.locator(".sheet")).toHaveCount(2);
    await expect(sheet(page, 0).locator(".pgb")).toHaveCount(1);
    await expect(sheet(page, 1).locator(".pgb")).toHaveCount(0);
    await openBuilt(page, { sections: [{ paragraphs: ["First page{PBR}", "Second page"], xml: { afterMar: BOX("single", 24, ' w:display="notFirstPage"') } }] });
    await expect(sheet(page, 0).locator(".pgb")).toHaveCount(0);
    await expect(sheet(page, 1).locator(".pgb")).toHaveCount(1);
  });

  test("an art border is drawn as a plain line in its colour, and a shadow as an offset", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Hearts"], xml: { afterMar: BOX("hearts", 20) } }] });
    const css = await page.locator(".sheet .pgb").first().evaluate((el) => {
      const c = getComputedStyle(el);
      return { style: c.borderTopStyle, color: c.borderTopColor };
    });
    expect(css).toEqual({ style: "solid", color: "rgb(255, 0, 0)" });
    await openBuilt(page, { sections: [{ paragraphs: ["Shadow"], xml: { afterMar: BOX("single", 24).replace('w:space="24" w:color="FF0000"/><w:right', 'w:space="24" w:color="FF0000" w:shadow="1"/><w:right') } }] });
    expect(await page.locator(".sheet .pgb").first().evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe("none");
  });

  test("each section has borders of its own", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Boxed"], xml: { afterMar: BOX("single", 24) } },
        { paragraphs: ["Plain"] },
      ],
    });
    await expect(page.locator(".sheet")).toHaveCount(2);
    await expect(sheet(page, 0).locator(".pgb")).toHaveCount(1);
    await expect(sheet(page, 1).locator(".pgb")).toHaveCount(0);
  });
});

test.describe("vertical alignment", () => {
  const body = (page: Page) => page.locator(".flow .fp");

  test("centred text sits half-way between the margins", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Cover title", "By the author"], xml: { end: '<w:vAlign w:val="center"/>' } }] });
    const first = await onSheet(page, body(page).first());
    const last = await onSheet(page, body(page).last());
    const used = last.y + last.h - first.y;
    // The text block is centred between the top and bottom margins (one inch each on Letter).
    near(first.y - 96, 1056 - 96 - (last.y + last.h), 2);
    expect(first.y).toBeGreaterThan(300);
    expect(used).toBeLessThan(120);
    await page.screenshot({ path: "test-results/vertical-center.png" });
  });

  test("bottom-aligned text ends at the bottom margin", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Colophon", "Printed in the year"], xml: { end: '<w:vAlign w:val="bottom"/>' } }] });
    const last = await onSheet(page, body(page).last());
    near(last.y + last.h, 1056 - 96, 6);
  });

  test("justified text spreads its paragraphs between the margins", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["One", "Two", "Three"], xml: { end: '<w:vAlign w:val="both"/>' } }] });
    const first = await onSheet(page, body(page).first());
    const last = await onSheet(page, body(page).last());
    near(first.y, 96, 2);
    near(last.y + last.h, 1056 - 96, 6);
    const mid = await onSheet(page, body(page).nth(1));
    // The gaps are even.
    near(mid.y - (first.y + first.h), last.y - (mid.y + mid.h), 3);
  });

  test("top is the default, and a section that doesn't ask keeps its text at the top", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        { paragraphs: ["Centred cover"], xml: { end: '<w:vAlign w:val="center"/>' } },
        { paragraphs: ["Body text starts at the top"] },
      ],
    });
    const cover = await onSheet(page, body(page).first(), 0);
    const text = await onSheet(page, body(page).last(), 1);
    expect(cover.y).toBeGreaterThan(300);
    near(text.y, 96, 2);
  });

  test("a section of several pages centres only the page that has room", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [...lines(70), "Last page text"], xml: { end: '<w:vAlign w:val="center"/>' } }] });
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    expect(pages).toBe(2);
    const last = await onSheet(page, body(page).last(), 1);
    expect(last.y).toBeGreaterThan(300);
    // The full page keeps its first line near the margin: it only gives up half of what is left under its last line.
    near((await onSheet(page, body(page).first(), 0)).y, 96, 9);
  });
});

test.describe("gutter and mirrored margins", () => {
  test("a gutter at the top of the page moves the text down", async ({ page }) => {
    await openBuilt(page, { sections: [{ gutter: 720, paragraphs: ["Bound at the top"] }], settings: "<w:gutterAtTop/>" });
    const b = await onSheet(page, page.locator(".flow .fp").first());
    near(b.y, 96 + 720 * TW);
    // The side margins are untouched.
    near(b.x, 96);
    await openBuilt(page, { sections: [{ gutter: 720, paragraphs: ["Bound at the side"] }] });
    const side = await onSheet(page, page.locator(".flow .fp").first());
    near(side.x, 96 + 720 * TW);
    near(side.y, 96);
  });

  test("floats measure from the text edge, which the gutter moves", async ({ page }) => {
    await openBuilt(page, { sections: [{ gutter: 720, paragraphs: ["Bound at the side"] }] });
    const m = await page.evaluate(async () => {
      const url = performance.getEntriesByType("resource").map((e) => e.name).find((n) => /\/editor\/place\.ts/.test(n));
      const mod = await import(/* @vite-ignore */ url!);
      return { ...mod.pageMetrics };
    });
    // Margin 72 pt plus the 36 pt gutter on the left; the text is that much narrower.
    expect(m.marginL).toBe(108);
    expect(m.textW).toBe(468 - 36);
  });

  test("mirrored margins put the inside margin on the right of even pages", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ margins: [1440, 720, 1440, 2160], gutter: 360, paragraphs: ["Odd page{PBR}", "Even page"] }],
      settings: "<w:mirrorMargins/>",
    });
    await expect(page.locator(".sheet")).toHaveCount(2);
    const odd = await onSheet(page, page.locator(".flow .fp").nth(0), 0);
    const even = await onSheet(page, page.locator(".flow .fp").nth(1), 1);
    // Inside (left margin) is 1.5 inches plus the 0.25 inch gutter on an odd page; on an even one the same room is on the right.
    near(odd.x, (2160 + 360) * TW);
    near(even.x, 720 * TW);
    // The text is as wide on both.
    near(odd.w, even.w, 1);
    expect(odd.w).toBeCloseTo((12240 - 2160 - 360 - 720) * TW, 0);
    await page.screenshot({ path: "test-results/mirror-margins.png" });
  });

  test("headers follow the mirrored margins too", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ margins: [1440, 720, 1440, 2160], headers: { default: "Header text" }, paragraphs: ["Odd page{PBR}", "Even page"] }],
      settings: "<w:mirrorMargins/>",
    });
    const odd = await onSheet(page, sheet(page, 0).locator(".hdr"), 0);
    const even = await onSheet(page, sheet(page, 1).locator(".hdr"), 1);
    near(odd.x, 2160 * TW);
    near(even.x, 720 * TW);
  });

  test("the page setup reports the settings", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Text"] }], settings: "<w:mirrorMargins/><w:gutterAtTop/>" });
    expect(await pageLayout(page)).toMatchObject({ mirrorMargins: true, gutterAtTop: true });
  });
});

test.describe("page background", () => {
  const bg = (inner: string, color = "FFFFFF") => `<w:background w:color="${color}"><v:background id="_x0000_s1025" o:bwmode="white" fillcolor="#${color}">${inner}</v:background></w:background>`;
  const styleOf = (page: Page) => sheet(page, 0).evaluate((el) => ({ image: getComputedStyle(el).backgroundImage, color: getComputedStyle(el).backgroundColor, size: getComputedStyle(el).backgroundSize, repeat: getComputedStyle(el).backgroundRepeat }));

  test("a gradient is drawn from the VML fill the file has", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Gradient"] }], background: bg('<v:fill color2="#9CCBF0" angle="-90" focus="100%" type="gradient"/>', "FFF2CC"), settings: "<w:displayBackgroundShape/>" });
    const st = await styleOf(page);
    expect(st.image).toContain("linear-gradient");
    expect(st.image).toContain("rgb(255, 242, 204)");
    expect(st.image).toContain("rgb(156, 203, 240)");
    // -90 runs from left to right.
    expect(st.image).toContain("90deg");
    const f = (await pageLayout(page)).pageFill as { kind: string; color2: string };
    expect(f).toMatchObject({ kind: "gradient", color2: "9CCBF0" });
  });

  test("a gradient from the middle is radial", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Radial"] }], background: bg('<v:fill color2="#FF4F00" type="gradientRadial" focusposition=".5,.5" focus="100%"/>', "FFFFFF") });
    expect((await styleOf(page)).image).toContain("radial-gradient");
  });

  test("a plain colour still works", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Colour"] }], background: '<w:background w:color="FFF2CC"/>', settings: "<w:displayBackgroundShape/>" });
    expect((await styleOf(page)).color).toBe("rgb(255, 242, 204)");
  });

  test("a gradient set through the engine shows on every page", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First{PBR}", "Second"] }] });
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_page_background", fill: { kind: "gradient", color: "FFFFFF", color2: "C9B8F0", angle: 0 } }));
    await inPage(page, (w) => w.fe.editor.reloadPageSetup());
    for (const k of [0, 1]) expect(await sheet(page, k).evaluate((el) => getComputedStyle(el).backgroundImage)).toContain("linear-gradient");
    await page.screenshot({ path: "test-results/page-gradient.png" });
  });

  test("a texture tiles and a picture fills the page", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Texture"] }] });
    // A one-pixel PNG as the picture.
    const png = [137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];
    await inPage(
      page,
      (w, bytes) => {
        const media = w.fe.editor.engine!.addMedia("png", new Uint8Array(bytes));
        w.fe.editor.exec({ op: "set_page_background", fill: { kind: "texture", color: "FFFFFF", media, title: "Test" } });
        w.fe.editor.reloadPageSetup();
      },
      png,
    );
    let st = await styleOf(page);
    expect(st.image).toContain("url(");
    expect(st.repeat).toContain("repeat");
    await inPage(
      page,
      (w) => {
        const media = (w.fe.editor.engine!.pageSetup() as unknown as { pageFill: { media: string } }).pageFill.media;
        w.fe.editor.exec({ op: "set_page_background", fill: { kind: "picture", color: "FFFFFF", media } });
        w.fe.editor.reloadPageSetup();
      },
    );
    st = await styleOf(page);
    expect(st.size).toBe("100% 100%");
    expect(st.repeat).toBe("no-repeat");
  });
});
