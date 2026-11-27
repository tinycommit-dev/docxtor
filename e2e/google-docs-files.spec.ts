// Files exported from Google Docs (File, Download, Microsoft Word): a banner picture behind the header that is wider
// than the page, horizontal lines (VML rules), and code blocks, whose lines Google marks with private characters.
import { openBuilt, pngBytes } from "./support/docx-builder";
import { expect, savedPart, test, type Page } from "./helpers";

const PIC = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const A = "http://schemas.openxmlformats.org/drawingml/2006/main";
/** A picture behind the text at the very top of the page, 9 inches wide (the page is 8.5): Google's header banner. */
const BANNER = `<w:r><w:drawing><wp:anchor allowOverlap="1" behindDoc="1" distB="0" distT="0" distL="0" distR="0" hidden="0" layoutInCell="1" locked="0" relativeHeight="0" simplePos="0"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="8229600" cy="365760"/><wp:effectExtent b="0" l="0" r="0" t="0"/><wp:wrapNone/><wp:docPr id="1" name="banner.png"/><a:graphic xmlns:a="${A}"><a:graphicData uri="${PIC}"><pic:pic xmlns:pic="${PIC}"><pic:nvPicPr><pic:cNvPr id="0" name="banner.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="8229600" cy="365760"/></a:xfrm><a:prstGeom prst="rect"/></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>`;
const RULE = `<w:r><w:pict><v:rect style="width:0.0pt;height:1.5pt" o:hr="t" o:hrstd="t" o:hralign="center" fillcolor="#A0A0A0" stroked="f"/></w:pict></w:r>`;
const MONO = `<w:rPr><w:rFonts w:ascii="Roboto Mono" w:hAnsi="Roboto Mono"/><w:color w:val="188038"/></w:rPr>`;

async function open(page: Page) {
  await openBuilt(page, {
    sections: [
      {
        headers: { default: "", raw: BANNER },
        paragraphs: [
          "Cowork prompts",
          { raw: RULE },
          { raw: `<w:r>${MONO}<w:t xml:space="preserve">ticker,shares,cost_basis</w:t></w:r>` },
          { raw: `<w:r><w:t xml:space="preserve">Rules:</w:t></w:r>` },
        ],
      },
    ],
    media: [{ name: "banner.png", rid: "rIdBanner", bytes: pngBytes(40, 4, [200, 0, 0]) }],
    files: [
      {
        name: "_rels/header1.xml.rels",
        contentType: "application/vnd.openxmlformats-package.relationships+xml",
        data: `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/banner.png"/></Relationships>`,
      },
    ],
  });
}

/** The colour on screen at a point, as [r, g, b]. */
async function pixel(page: Page, x: number, y: number): Promise<number[]> {
  const png = await page.screenshot({ clip: { x, y, width: 1, height: 1 } });
  return page.evaluate(async (b64) => {
    const img = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob());
    const c = new OffscreenCanvas(1, 1).getContext("2d")!;
    c.drawImage(img, 0, 0);
    return Array.from(c.getImageData(0, 0, 1, 1).data.slice(0, 3));
  }, png.toString("base64"));
}

test("a picture behind the header shows on the page, cut off at the page's edge", async ({ page }) => {
  await open(page);
  const sheet = (await page.locator(".sheet").first().boundingBox())!;
  await expect(page.locator(".sheet .hf.hdr img").first()).toBeAttached();
  await page.evaluate(() => document.querySelector(".sheet .hf.hdr img")!.decode());
  const red = (c: number[]) => c[0] > 150 && c[1] < 80 && c[2] < 80;
  // On the page, near its top left corner: the banner.
  expect(red(await pixel(page, sheet.x + 40, sheet.y + 10))).toBe(true);
  expect(red(await pixel(page, sheet.x + sheet.width - 4, sheet.y + 10))).toBe(true);
  // Past the page's right edge, where the 9-inch picture would run on: the desk, not the picture.
  expect(red(await pixel(page, sheet.x + sheet.width + 6, sheet.y + 10))).toBe(false);
});

test("a horizontal line is drawn across the column, not shown as a Shape chip, and saved as it was", async ({ page }) => {
  await open(page);
  const rule = page.locator(".flow .hrule");
  await expect(rule).toHaveCount(1);
  await expect(page.locator(".flow .atom.obj")).toHaveCount(0);
  const look = await rule.evaluate((e) => {
    const p = e.closest(".fp")!.getBoundingClientRect();
    const r = e.getBoundingClientRect();
    return { width: r.width / p.width, height: r.height, color: getComputedStyle(e).backgroundColor };
  });
  expect(look.width).toBeGreaterThan(0.98);
  expect(look.height).toBe(2);
  expect(look.color).toBe("rgb(160, 160, 160)");
  expect(await savedPart(page, "word/document.xml")).toContain('o:hr="t"');
});

test("Google's code block marks show as nothing, and stay in the file", async ({ page }) => {
  await open(page);
  await expect(page.locator(".flow")).toContainText("ticker,shares,cost_basis");
  const shown = await page.locator(".flow").evaluate((f) => f.textContent!);
  expect(shown).not.toMatch(/[]/);
  // The mark takes no room: the code starts where the line starts.
  const offset = await page.locator(".flow .fp", { hasText: "ticker" }).evaluate((p) => {
    const w = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
    for (let n = w.nextNode(); n; n = w.nextNode()) {
      const i = (n as Text).data.indexOf("ticker");
      if (i < 0) continue;
      const r = document.createRange();
      r.setStart(n, i);
      r.setEnd(n, i + 1);
      return r.getBoundingClientRect().left - p.getBoundingClientRect().left;
    }
    return -1;
  });
  expect(offset).toBeGreaterThanOrEqual(0);
  expect(offset).toBeLessThan(1);
  const xml = await savedPart(page, "word/document.xml");
  expect(xml).toContain("ticker");
  expect(xml).toContain("Rules:");
});
