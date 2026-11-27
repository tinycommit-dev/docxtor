// How pictures from Word files look: crop, turn and flips, outline, transparency, brightness and contrast,
// recolouring, the picture's shape, shadow, glow, reflection and soft edges, linked and SVG pictures, and the
// distances text keeps from a floating picture. Pixels are read back from a screenshot of the picture.
import { readFileSync } from "node:fs";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Locator, type Page } from "./helpers";
import { pageContent, readPdf } from "./support/pdf-reader";
import { emf, tiff, wmf } from "./support/picture-formats";
import { BLUE, GREEN, pictureRun, quarters, RED, YELLOW, type PictureSpec } from "./support/picture-markup";

const PT_PX = 96 / 72;

/** A document with one picture (a 40 x 20 quarters PNG) in a paragraph of its own, below a line of text. */
async function open(page: Page, spec: PictureSpec, extra: { text?: string } = {}) {
  await openBuilt(page, {
    media: [{ name: "quarters.png", rid: "rIdPic1", bytes: quarters() }],
    sections: [{ paragraphs: [extra.text ?? "A line of text above the picture", { raw: pictureRun(spec) }, "A line of text below"] }],
  });
  await inPage(page, (w) => w.fe.editor.setZoom(1));
  await page.evaluate(() => document.fonts.ready);
  await expect(atom(page)).toBeVisible();
  // The picture must have loaded before its pixels are read.
  await page.waitForFunction(() => {
    const img = document.querySelector<HTMLImageElement>(".flow .atom.img img");
    return !img || (img.complete && img.naturalWidth > 0);
  });
}

const atom = (page: Page) => page.locator(".flow .atom.img").first();
const pic = (page: Page) => page.locator(".flow .atom.img .pic").first();

/** The colours at points of an element (as fractions of its box), read from a screenshot of it, as `r,g,b` rounded to tens. */
async function pixels(page: Page, loc: Locator, at: [number, number][]): Promise<[number, number, number][]> {
  const shot = await loc.screenshot();
  return page.evaluate(
    async ([b64, pts]) => {
      const blob = new Blob([Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))], { type: "image/png" });
      const bmp = await createImageBitmap(blob);
      const c = document.createElement("canvas");
      c.width = bmp.width;
      c.height = bmp.height;
      const g = c.getContext("2d")!;
      g.drawImage(bmp, 0, 0);
      return (pts as [number, number][]).map(([fx, fy]) => {
        const d = g.getImageData(Math.min(bmp.width - 1, Math.floor(fx * bmp.width)), Math.min(bmp.height - 1, Math.floor(fy * bmp.height)), 1, 1).data;
        return [d[0], d[1], d[2]] as [number, number, number];
      });
    },
    [shot.toString("base64"), at] as const,
  );
}

/** Whether a colour is near another (each channel within `tol`). */
const near = (got: [number, number, number], want: readonly number[], tol = 28) => got.every((v, i) => Math.abs(v - want[i]) <= tol);
const rgb = (c: readonly number[]) => [c[0], c[1], c[2]] as const;

test.describe("crop", () => {
  test("a cropped picture shows the part that is left, not the whole picture squeezed", async ({ page }) => {
    // The left half is cropped away: 50 x 50 pt of the right half, which is green over yellow.
    await open(page, { w: 50, h: 50, crop: [0.5, 0, 0, 0] });
    const box = await pic(page).boundingBox();
    expect(box!.width).toBeCloseTo(50 * PT_PX, 0);
    expect(box!.height).toBeCloseTo(50 * PT_PX, 0);
    const img = page.locator(".flow .atom.img .pic-img").first();
    // The whole picture is drawn twice as wide as the box and shifted left by one box.
    const geo = await img.evaluate((e) => ({ w: e.clientWidth, h: e.clientHeight, left: parseFloat(getComputedStyle(e).left), top: parseFloat(getComputedStyle(e).top) }));
    expect(geo.w).toBeCloseTo(100 * PT_PX, 0);
    expect(geo.h).toBeCloseTo(50 * PT_PX, 0);
    expect(geo.left).toBeCloseTo(-50 * PT_PX, 0);
    expect(geo.top).toBeCloseTo(0, 0);
    const [tl, br, tr, bl] = await pixels(page, pic(page), [[0.1, 0.1], [0.9, 0.9], [0.9, 0.1], [0.1, 0.9]]);
    expect(near(tl, rgb(GREEN))).toBe(true);
    expect(near(tr, rgb(GREEN))).toBe(true);
    expect(near(bl, rgb(YELLOW))).toBe(true);
    expect(near(br, rgb(YELLOW))).toBe(true);
  });

  test("a crop from every side keeps the middle of the picture; a negative crop pads", async ({ page }) => {
    await open(page, { w: 100, h: 50, crop: [0.25, 0.25, 0.25, 0.25] });
    // Only the centre is left: the four corners of the picture's quarters meet in it.
    const colours = await pixels(page, pic(page), [[0.05, 0.05], [0.95, 0.05], [0.05, 0.95], [0.95, 0.95]]);
    expect(colours.map((c, i) => near(c, rgb([RED, GREEN, BLUE, YELLOW][i]))).every(Boolean)).toBe(true);
    await open(page, { w: 120, h: 50, crop: [-0.2, 0, -0.2, 0] });
    const img = page.locator(".flow .atom.img .pic-img").first();
    const geo = await img.evaluate((e) => ({ w: e.clientWidth, left: parseFloat(getComputedStyle(e).left) }));
    // 120 pt is 1.4 times the picture (20% padding on each side): the picture is 85.7 pt wide, 17.1 pt in.
    expect(geo.w).toBeCloseTo((120 / 1.4) * PT_PX, 0);
    expect(geo.left).toBeCloseTo(((120 / 1.4) * 0.2) * PT_PX, 0);
    const [pad] = await pixels(page, pic(page), [[0.03, 0.5]]);
    expect(pad.every((v) => v > 240)).toBe(true);
  });
});

test.describe("turn and flip", () => {
  test("a quarter turn turns the picture; a flip mirrors it", async ({ page }) => {
    await open(page, { w: 80, h: 40, rot: 90, effectExtent: [0, 20, 0, 20] });
    const el = pic(page);
    await expect(el).toHaveCSS("transform", /matrix\(/);
    const turned = await el.boundingBox();
    // 80 x 40 turned a quarter is 40 wide and 80 tall (the bounding box on screen).
    expect(turned!.width).toBeCloseTo(40 * PT_PX, 0);
    expect(turned!.height).toBeCloseTo(80 * PT_PX, 0);
    // Clockwise: the bottom left (blue) comes to the top left, the top left (red) to the top right.
    const [tl, tr, bl, br] = await pixels(page, el, [[0.1, 0.1], [0.9, 0.1], [0.1, 0.9], [0.9, 0.9]]);
    expect(near(tl, rgb(BLUE))).toBe(true);
    expect(near(tr, rgb(RED))).toBe(true);
    expect(near(bl, rgb(YELLOW))).toBe(true);
    expect(near(br, rgb(GREEN))).toBe(true);
    // The room Word keeps for the turn: the picture sits 20 pt lower than its own box would, clear of the line above.
    const margin = await pic(page).evaluate((a) => ({ top: parseFloat(getComputedStyle(a).marginTop), bottom: parseFloat(getComputedStyle(a).marginBottom) }));
    expect(margin.top).toBeCloseTo(20 * PT_PX, 0);
    expect(margin.bottom).toBeCloseTo(20 * PT_PX, 0);
    const above = (await page.locator(".flow .fp").first().boundingBox())!;
    expect(turned!.y).toBeGreaterThanOrEqual(above.y + above.height - 1);
  });

  test("flips mirror the picture on one axis", async ({ page }) => {
    await open(page, { w: 80, h: 40, flipH: true });
    let [tl, tr, bl, br] = await pixels(page, pic(page), [[0.1, 0.1], [0.9, 0.1], [0.1, 0.9], [0.9, 0.9]]);
    expect([near(tl, rgb(GREEN)), near(tr, rgb(RED)), near(bl, rgb(YELLOW)), near(br, rgb(BLUE))]).toEqual([true, true, true, true]);
    await open(page, { w: 80, h: 40, flipV: true });
    [tl, tr, bl, br] = await pixels(page, pic(page), [[0.1, 0.1], [0.9, 0.1], [0.1, 0.9], [0.9, 0.9]]);
    expect([near(tl, rgb(BLUE)), near(tr, rgb(YELLOW)), near(bl, rgb(RED)), near(br, rgb(GREEN))]).toEqual([true, true, true, true]);
  });
});

test.describe("outline, shape and effects", () => {
  test("the outline has its colour, width and dash", async ({ page }) => {
    await open(page, { w: 80, h: 40, line: { color: "FF4F00", w: 3, dash: "dash" } });
    const path = page.locator(".flow .atom.img .pic-ln path").first();
    await expect(path).toHaveAttribute("stroke", "#FF4F00");
    expect(Number(await path.getAttribute("stroke-width"))).toBeCloseTo(3 * PT_PX, 1);
    // A dash is longer than it is wide.
    const dashes = (await path.getAttribute("stroke-dasharray"))!.split(" ").map(Number);
    expect(dashes[0]).toBeCloseTo(4 * 3 * PT_PX, 0);
    // On the edge of the picture the colour is the outline's.
    const [edge] = await pixels(page, pic(page), [[0.01, 0.01]]);
    expect(edge[0]).toBeGreaterThan(200);
    expect(edge[1]).toBeLessThan(150);
  });

  test("a picture cropped to a shape is clipped to it", async ({ page }) => {
    await open(page, { w: 80, h: 80, geometry: "ellipse" });
    const clip = page.locator(".flow .atom.img .pic-clip").first();
    await expect(clip).toHaveCSS("clip-path", /path\(/);
    const [corner, centre] = await pixels(page, pic(page), [[0.03, 0.03], [0.5, 0.45]]);
    expect(corner.every((v) => v > 240)).toBe(true);
    expect(corner.every((v) => v > 240) && !near(centre, [255, 255, 255])).toBe(true);
  });

  test("transparency lets the page show through", async ({ page }) => {
    await open(page, { w: 80, h: 40, blip: '<a:alphaModFix amt="50000"/>' });
    await expect(page.locator(".flow .atom.img .pic-clip").first()).toHaveCSS("opacity", "0.5");
    const [red] = await pixels(page, pic(page), [[0.1, 0.1]]);
    // Half red over white: red stays high, green and blue come up to the middle.
    expect(red[0]).toBeGreaterThan(220);
    expect(red[1]).toBeGreaterThan(120);
  });

  test("brightness and contrast change the colours", async ({ page }) => {
    await open(page, { w: 80, h: 40, blip: '<a:lum bright="60000"/>' });
    // Brightness 60% moves the colours up by 30%: the green of the red quarter goes from 30 to about 107.
    const [red] = await pixels(page, pic(page), [[0.1, 0.1]]);
    expect(red[0]).toBeGreaterThan(245);
    expect(red[1]).toBeGreaterThan(90);
    expect(red[1]).toBeLessThan(125);
    await open(page, { w: 80, h: 40, blip: '<a:lum contrast="-70000"/>' });
    // Less contrast pulls the colours to the middle grey.
    const [low] = await pixels(page, pic(page), [[0.1, 0.1]]);
    expect(Math.abs(low[0] - low[1])).toBeLessThan(90);
  });

  test("recolouring: greyscale, black and white, and two colours", async ({ page }) => {
    await open(page, { w: 80, h: 40, blip: "<a:grayscl/>" });
    const [grey] = await pixels(page, pic(page), [[0.1, 0.1]]);
    expect(Math.max(...grey) - Math.min(...grey)).toBeLessThan(8);
    await open(page, { w: 80, h: 40, blip: '<a:biLevel thresh="50000"/>' });
    const [yellow, blue] = await pixels(page, pic(page), [[0.9, 0.9], [0.1, 0.9]]);
    expect(near(yellow, [255, 255, 255], 10)).toBe(true);
    expect(near(blue, [0, 0, 0], 10)).toBe(true);
    await open(page, { w: 80, h: 40, blip: '<a:duotone><a:srgbClr val="000000"/><a:srgbClr val="FF0000"/></a:duotone>' });
    const [duo] = await pixels(page, pic(page), [[0.9, 0.9]]);
    // The light yellow quarter becomes a strong red: no green, no blue.
    expect(duo[0]).toBeGreaterThan(150);
    expect(duo[1]).toBeLessThan(10);
    expect(duo[2]).toBeLessThan(10);
  });

  test("a colour made transparent shows the page", async ({ page }) => {
    await open(page, { w: 80, h: 40, blip: '<a:clrChange><a:clrFrom><a:srgbClr val="DC1E1E"/></a:clrFrom><a:clrTo><a:srgbClr val="DC1E1E"><a:alpha val="0"/></a:srgbClr></a:clrTo></a:clrChange>' });
    await expect
      .poll(async () => {
        const [red, green] = await pixels(page, pic(page), [[0.1, 0.1], [0.9, 0.1]]);
        return [red.every((v) => v > 240), near(green, rgb(GREEN))];
      })
      .toEqual([true, true]);
  });

  test("drawing the picture again does not make a new picture each time", async ({ page }) => {
    await page.addInitScript(() => {
      const w = window as unknown as { madeUrls: number };
      w.madeUrls = 0;
      const make = URL.createObjectURL.bind(URL);
      URL.createObjectURL = (o: Blob | MediaSource) => {
        w.madeUrls++;
        return make(o);
      };
    });
    await open(page, { w: 80, h: 40, blip: '<a:clrChange><a:clrFrom><a:srgbClr val="DC1E1E"/></a:clrFrom><a:clrTo><a:srgbClr val="DC1E1E"><a:alpha val="0"/></a:srgbClr></a:clrTo></a:clrChange>' });
    await expect
      .poll(async () => {
        const [red] = await pixels(page, pic(page), [[0.1, 0.1]]);
        return red.every((v) => v > 240);
      })
      .toBe(true);
    const before = await page.evaluate(() => (window as unknown as { madeUrls: number }).madeUrls);
    for (let i = 0; i < 4; i++) {
      await inPage(page, (w) => w.fe.editor.rerender());
      await expect(atom(page)).toBeVisible();
      await page.waitForTimeout(150);
    }
    const after = await page.evaluate(() => (window as unknown as { madeUrls: number }).madeUrls);
    expect(after).toBe(before);
  });

  test("shadow, glow, reflection and soft edges are drawn", async ({ page, browserName }) => {
    await open(page, {
      w: 80,
      h: 40,
      effects:
        '<a:glow rad="101600"><a:srgbClr val="5B9BD5"><a:alpha val="40000"/></a:srgbClr></a:glow><a:outerShdw blurRad="63500" dist="50800" dir="2700000" algn="tl" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="50000"/></a:srgbClr></a:outerShdw><a:reflection blurRad="0" stA="50000" endPos="50000" dist="25400" dir="5400000" sy="-100000" algn="bl" rotWithShape="0"/><a:softEdge rad="38100"/>',
    });
    const el = pic(page);
    const filter = await el.evaluate((e) => getComputedStyle(e).filter);
    expect(filter).toMatch(/drop-shadow/);
    // The shadow falls down and to the right (45 degrees), the glow all round.
    expect(filter).toMatch(/\d+(\.\d+)?px \d+(\.\d+)?px/);
    // The reflection is drawn with -webkit-box-reflect, which Chrome and Safari have and Firefox does not (the picture shows there without it).
    if (browserName !== "firefox") {
      const reflect = await el.evaluate((e) => getComputedStyle(e).getPropertyValue("-webkit-box-reflect"));
      expect(reflect).toMatch(/below/);
    }
    const mask = await page.locator(".flow .atom.img .pic-clip").first().evaluate((e) => getComputedStyle(e).maskImage || getComputedStyle(e).getPropertyValue("-webkit-mask-image"));
    expect(mask).toMatch(/linear-gradient/);
  });

  test("a shadow keeps its direction on the page when the picture is turned", async ({ page }) => {
    await open(page, { w: 80, h: 40, rot: 90, effects: '<a:outerShdw blurRad="0" dist="50800" dir="0" algn="tl" rotWithShape="0"><a:srgbClr val="000000"/></a:outerShdw>' });
    // Straight to the right on the page; in the picture's own turned space that is straight up.
    const filter = await pic(page).evaluate((e) => getComputedStyle(e).filter);
    const m = filter.match(/drop-shadow\((?:rgba?\([^)]*\)\s*)?(-?[\d.]+)px (-?[\d.]+)px/);
    expect(m).not.toBeNull();
    expect(Math.abs(Number(m![1]))).toBeLessThan(1);
    expect(Number(m![2])).toBeCloseTo(-4 * PT_PX, 0);
  });
});

test.describe("linked and vector pictures", () => {
  test("a picture only linked to the file shows a placeholder with the file name", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "Above",
            {
              raw: pictureRun({ w: 120, h: 60, rid: "rIdLink1" }).replace('r:embed="rIdLink1"', 'r:link="rIdLink1"'),
            },
            "Below",
          ],
        },
      ],
      extraRels: [{ id: "rIdLink1", type: "image", target: "file:///C:/Users/ana/Pictures/Company%20logo.png", external: true }],
    });
    const missing = page.locator(".flow .atom.img .pic-missing");
    await expect(missing).toBeVisible();
    await expect(missing).toContainText("Company logo.png");
    await expect(missing).toContainText("not in this file");
    await expect(page.locator(".flow .atom.img img")).toHaveCount(0);
    const box = await missing.boundingBox();
    expect(box!.width).toBeCloseTo(120 * PT_PX, 0);
    expect(box!.height).toBeCloseTo(60 * PT_PX, 0);
    await expect(missing).toHaveAttribute("aria-label", "Linked picture: Company logo.png");
  });

  test("a picture linked at a web address is not fetched until the person asks, and then shows with no referrer even from a site that sends no cross-origin permission", async ({ page }) => {
    const requests: Record<string, string>[] = [];
    await page.route("https://tracker.example/**", (route) => {
      requests.push(route.request().headers());
      return route.fulfill({ status: 200, contentType: "image/png", body: Buffer.from(quarters()) });
    });
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "Above",
            { raw: pictureRun({ w: 120, h: 60, rid: "rIdLink1" }).replace('r:embed="rIdLink1"', 'r:link="rIdLink1"') },
            "Below",
          ],
        },
      ],
      extraRels: [{ id: "rIdLink1", type: "image", target: "https://tracker.example/pixel.png", external: true }],
    });
    const missing = page.locator(".flow .atom.img .pic-missing");
    await expect(missing).toBeVisible();
    await expect(missing).toContainText("pixel.png");
    await page.waitForTimeout(300);
    expect(requests).toHaveLength(0);
    await missing.getByRole("button", { name: "Show linked pictures" }).click();
    const img = page.locator(".flow .atom.img img");
    await expect(img).toBeVisible();
    await expect.poll(() => img.evaluate((e) => (e as HTMLImageElement).complete && (e as HTMLImageElement).naturalWidth > 0)).toBe(true);
    expect(requests.length).toBeGreaterThan(0);
    expect(requests.every((h) => !h.referer && !h.cookie)).toBe(true);
    await expect(page.locator(".flow .atom.img .pic-missing")).toHaveCount(0);
  });

  test("a linked picture whose site gives no cross-origin permission shows on the page, and the PDF gets a placeholder box and a plain note", async ({ page }) => {
    // The site answers an ordinary picture request, and a script's request (the PDF reading the file) fails for want of permission
    // (the test browser adds that permission by itself to what it serves, so the refusal is made here).
    await page.route("https://nopermission.example/**", (route) =>
      route.request().resourceType() === "fetch" ? route.abort("failed") : route.fulfill({ status: 200, contentType: "image/png", body: Buffer.from(quarters()) }),
    );
    await openBuilt(page, {
      sections: [{ paragraphs: ["Above", { raw: pictureRun({ w: 120, h: 60, rid: "rIdLink1" }).replace('r:embed="rIdLink1"', 'r:link="rIdLink1"') }, "Below"] }],
      extraRels: [{ id: "rIdLink1", type: "image", target: "https://nopermission.example/logo.png", external: true }],
    });
    await page.locator(".flow .atom.img .pic-missing").getByRole("button", { name: "Show linked pictures" }).click();
    const img = page.locator(".flow .atom.img img");
    await expect.poll(() => img.evaluate((e) => (e as HTMLImageElement).complete && (e as HTMLImageElement).naturalWidth > 0)).toBe(true);
    const [d] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
    const file = readFileSync((await d.path())!);
    expect(file.subarray(0, 5).toString()).toBe("%PDF-");
    expect(readPdf(file).text).toContain("/Subtype /Image");
    await expect(page.locator(".toast")).toHaveText(/website wouldn't let Docxtor copy it, so the PDF has an empty box/);
  });

  test("a picture with an SVG version draws the SVG, not its PNG stand-in", async ({ page }) => {
    // The PNG is the four quarters; the SVG is one big green square.
    const svg = new TextEncoder().encode('<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 0 40 20"><rect width="40" height="20" fill="#1eaa3c"/></svg>');
    await openBuilt(page, {
      media: [
        { name: "quarters.png", rid: "rIdPic1", bytes: quarters() },
        { name: "vector.svg", rid: "rIdSvg1", bytes: svg },
      ],
      sections: [
        {
          paragraphs: [
            "Above",
            { raw: pictureRun({ w: 80, h: 40, blip: '<a:extLst><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="rIdSvg1"/></a:ext></a:extLst>' }) },
            "Below",
          ],
        },
      ],
    });
    const img = page.locator(".flow .atom.img img").first();
    await expect(img).toBeVisible();
    const type = await img.evaluate(async (e) => (await fetch((e as HTMLImageElement).src)).headers.get("content-type"));
    expect(type).toBe("image/svg+xml");
    await expect.poll(() => img.evaluate((e) => (e as HTMLImageElement).complete && (e as HTMLImageElement).naturalWidth > 0)).toBe(true);
    const [left, right] = await pixels(page, atom(page), [[0.1, 0.1], [0.9, 0.9]]);
    expect(near(left, [30, 170, 60])).toBe(true);
    expect(near(right, [30, 170, 60])).toBe(true);
  });

  test("Download as PDF keeps the crop, the turn, the flip and the transparency", async ({ page }) => {
    await openBuilt(page, {
      media: [{ name: "quarters.png", rid: "rIdPic1", bytes: quarters() }],
      sections: [
        {
          paragraphs: [
            "Above",
            { raw: pictureRun({ id: 1, w: 50, h: 40, crop: [0.375, 0, 0, 0], rot: 30 }) + pictureRun({ id: 2, w: 80, h: 40, flipH: true, blip: '<a:alphaModFix amt="50000"/>' }) },
            "Below",
          ],
        },
      ],
    });
    const [download] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
    const pdf = readPdf(readFileSync((await download.path())!));
    const content = pageContent(pdf, pdf.pages[0]);
    // The cropped picture: clipped to its 50 x 40 pt box, with the whole picture (80 pt wide) drawn 30 pt to the left of it.
    expect(content).toMatch(/q 72 [\d.]+ (?:49\.99\d?|50) (?:39\.99\d?|40) re W n/);
    expect(content).toMatch(/q (?:79\.99\d?|80) 0 0 (?:39\.99\d?|40) 42 [\d.]+ cm \/Im1 Do Q/);
    // Turned by 30 degrees (cos 0.866, sin 0.5) and mirrored (-1 0 0 1), and half transparent.
    expect(content).toMatch(/q 0\.866 -0\.5 0\.5 0\.866 [-\d.]+ [-\d.]+ cm/);
    expect(content).toMatch(/q -1 0 0 1 [-\d.]+ 0 cm/);
    expect(content).toMatch(/\/GS\d gs/);
  });
});

test.describe("TIFF, EMF and WMF pictures", () => {
  const cases: [string, () => Uint8Array][] = [
    ["tif", tiff],
    ["emf", emf],
    ["wmf", wmf],
  ];
  for (const [ext, make] of cases) {
    test(`a .${ext} picture is drawn, not a broken image, and the file keeps the original`, async ({ page }) => {
      const bytes = make();
      await openBuilt(page, {
        media: [{ name: `picture.${ext}`, rid: "rIdPic1", bytes }],
        sections: [{ paragraphs: ["Above", { raw: pictureRun({ w: 100, h: 50 }) }, "Below"] }],
      });
      const img = page.locator(".flow .atom.img img").first();
      await expect(img).toBeVisible();
      await expect.poll(() => img.evaluate((e) => (e as HTMLImageElement).complete && (e as HTMLImageElement).naturalWidth > 0)).toBe(true);
      // Red on the left, green on the right, as in the file.
      const [left, right] = await pixels(page, atom(page), [[0.2, 0.5], [0.8, 0.5]]);
      expect(near(left, rgb(RED), 40), `${left}`).toBe(true);
      expect(near(right, rgb(GREEN), 40), `${right}`).toBe(true);
      // Saved: the picture part is the original bytes.
      const same = await page.evaluate(
        ([ext2, want]) => {
          const w = window as unknown as { docxtor: { fe: { editor: { engine: { media(p: string): Uint8Array | undefined; renderAll(): { blocks: { segs?: { img?: { src: string } }[] }[] } } } } } };
          const e = w.docxtor.fe.editor.engine;
          const path = `word/media/picture.${ext2}`;
          const got = (e as unknown as { raw: { media(p: string): Uint8Array | undefined } }).raw.media(path);
          return !!got && got.length === (want as number[]).length && (want as number[]).every((v, i) => got[i] === v);
        },
        [ext, Array.from(bytes)] as const,
      );
      expect(same).toBe(true);
    });
  }

  test("a picture the engine cannot convert shows a plain placeholder", async ({ page }) => {
    await openBuilt(page, {
      media: [{ name: "scan.tif", rid: "rIdPic1", bytes: new Uint8Array([0x49, 0x49, 42, 0, 8, 0, 0, 0, 0, 0]) }],
      sections: [{ paragraphs: ["Above", { raw: pictureRun({ w: 100, h: 50 }) }, "Below"] }],
    });
    const missing = page.locator(".flow .atom.img .pic-missing");
    await expect(missing).toBeVisible();
    await expect(missing).toContainText("scan.tif");
    await expect(missing).toContainText("can't be shown here");
  });

  test("Insert, Image, From your computer takes a TIFF and sizes it from what is shown", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["A line of text"] }] });
    await page.locator(".flow .fp").first().click();
    const [chooser] = await Promise.all([page.waitForEvent("filechooser"), chooseMenu(page, "Insert", "Image", "From your computer…")]);
    await chooser.setFiles({ name: "scan.tiff", mimeType: "image/tiff", buffer: Buffer.from(tiff()) });
    const img = page.locator(".flow .atom.img img").first();
    await expect(img).toBeVisible();
    // 4 x 2 pixels at 96 to the inch: 3 x 1.5 points.
    const size = await atom(page).evaluate((a) => ({ w: Number((a as HTMLElement).dataset.w), h: Number((a as HTMLElement).dataset.h) }));
    expect(size.w).toBeGreaterThan(0);
    expect(size.w / size.h).toBeGreaterThan(1.3);
  });
});
