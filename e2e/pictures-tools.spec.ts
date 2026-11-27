// Working on a picture in the editor: crop on the picture (with handles, an aspect ratio, a shape), turn and flip,
// dragging a floating picture, and the picture strip. Every change is an op, so each undoes.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, expect, inPage, test, type Locator, type Page } from "./helpers";
import { pictureRun, quarters, type PictureSpec } from "./support/picture-markup";

const PT_PX = 96 / 72;

interface Pic {
  w: number;
  h: number;
  wrap: string;
  x?: number;
  y?: number;
  relX?: string;
  relY?: string;
  align?: string;
  alignY?: string;
  pic?: { crop?: number[]; rot?: number; flipH?: boolean; flipV?: boolean; geom?: string };
  place?: { dist: number[]; wrapText?: string; through?: boolean; locked?: boolean; overlap: boolean; alignH?: string };
}

/** A document with one picture (a 80 x 40 quarters PNG) between two lines of text. */
async function open(page: Page, spec: PictureSpec = {}) {
  await openBuilt(page, {
    media: [{ name: "quarters.png", rid: "rIdPic1", bytes: quarters(80, 40) }],
    sections: [{ paragraphs: ["A line of text above the picture", { raw: pictureRun({ w: 160, h: 80, ...spec }) }, "A line of text below the picture"] }],
  });
  await inPage(page, (w) => w.fe.editor.setZoom(1));
  await page.evaluate(() => document.fonts.ready);
  await page.waitForFunction(() => {
    const img = document.querySelector<HTMLImageElement>(".flow .atom.img img");
    return !!img && img.complete && img.naturalWidth > 0;
  });
}

const atom = (page: Page) => page.locator(".flow .atom.img").first();
const strip = (page: Page) => page.locator(".img-strip");
const frame = (page: Page) => page.locator(".img-frame");

/** Where the picture is in the model. */
const where = (page: Page) => atom(page).evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
/** The picture as the engine reads it. */
async function model(page: Page): Promise<Pic> {
  const at = await where(page);
  return inPage(page, (w, pos) => w.fe.editor.engine!.pictureAt(pos), at) as unknown as Pic;
}

/** The document.xml of the saved file. */
async function savedXml(page: Page): Promise<string> {
  return page.evaluate(async () => {
    const w = window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } } } } };
    const bytes = w.docxtor.fe.editor.engine.save();
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
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return await new Response(stream).text();
    }
    return "";
  });
}

async function select(page: Page) {
  await atom(page).click();
  await expect(frame(page)).toBeVisible();
  await expect(strip(page)).toBeVisible();
}

const centre = async (loc: Locator) => {
  const b = (await loc.boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
};

/** Drag from the middle of one element by a distance on screen. */
async function drag(page: Page, from: Locator, dx: number, dy: number) {
  const c = await centre(from);
  await page.mouse.move(c.x, c.y);
  await page.mouse.down();
  await page.mouse.move(c.x + dx / 2, c.y + dy / 2, { steps: 4 });
  await page.mouse.move(c.x + dx, c.y + dy, { steps: 4 });
  await page.mouse.up();
}

test.describe("crop", () => {
  test("Crop shows the whole picture dimmed with handles on its frame; dragging a handle crops, and Done keeps it", async ({ page }) => {
    await open(page);
    await select(page);
    await strip(page).getByRole("button", { name: "Crop", exact: true }).click();
    const win = page.locator(".crop-win");
    await expect(win).toBeVisible();
    await expect(page.locator(".crop-dim")).toBeVisible();
    await expect(page.locator(".crop-handle")).toHaveCount(8);
    // The part that is cut off is drawn dimmed: nothing yet, so the frame is the whole picture.
    const box0 = (await win.boundingBox())!;
    expect(box0.width).toBeCloseTo(160 * PT_PX, 0);
    // Pull the left edge in by a quarter of the picture.
    // (Firefox and Safari move the mouse in whole pixels, so the drag is a whole number of them.)
    const pull = Math.round(box0.width / 4);
    await drag(page, page.locator(".crop-handle.w"), pull, 0);
    const box1 = (await win.boundingBox())!;
    expect(box1.width).toBeCloseTo(box0.width - pull, 0);
    expect(box1.x).toBeCloseTo(box0.x + pull, 0);
    await page.locator(".crop-strip").getByRole("button", { name: "Done" }).click();
    await expect(page.locator(".crop-layer")).toHaveCount(0);
    const m = await model(page);
    expect(m.pic!.crop![0]).toBeCloseTo(0.25, 2);
    expect(m.pic!.crop!.slice(1)).toEqual([0, 0, 0]);
    // The picture is 3/4 as wide, at the same scale.
    expect(m.w).toBeCloseTo(120, 0);
    expect(m.h).toBeCloseTo(80, 0);
    const xml = await savedXml(page);
    expect(xml).toMatch(/<a:srcRect l="2[45]\d{3}"\/>/);
    const box = (await atom(page).locator(".pic").boundingBox())!;
    expect(box.width).toBeCloseTo(120 * PT_PX, 0);
    // One undo gives the whole picture back.
    await page.keyboard.press("ControlOrMeta+z");
    await expect.poll(async () => (await model(page)).pic).toBeUndefined();
    expect((await model(page)).w).toBeCloseTo(160, 0);
  });

  test("cropping works when the page is zoomed", async ({ page }) => {
    await open(page);
    await inPage(page, (w) => w.fe.editor.setZoom(1.25));
    await select(page);
    await strip(page).getByRole("button", { name: "Crop", exact: true }).click();
    const win = page.locator(".crop-win");
    const box0 = (await win.boundingBox())!;
    expect(box0.width).toBeCloseTo(160 * PT_PX * 1.25, 0);
    await drag(page, page.locator(".crop-handle.e"), -box0.width / 4, 0);
    await drag(page, page.locator(".crop-handle.n"), 0, box0.height / 2);
    await page.keyboard.press("Enter");
    const m = await model(page);
    expect(m.pic!.crop![2]).toBeCloseTo(0.25, 1);
    expect(m.pic!.crop![1]).toBeCloseTo(0.5, 1);
    expect(m.w).toBeCloseTo(120, 0);
    expect(m.h).toBeCloseTo(40, 0);
  });

  test("Escape leaves the picture as it was; a drag inside the frame moves the picture under it", async ({ page }) => {
    await open(page, { crop: [0.25, 0, 0.25, 0], w: 80, h: 80 });
    await select(page);
    await strip(page).getByRole("button", { name: "Crop", exact: true }).click();
    const win = page.locator(".crop-win");
    // Move the picture under the frame to the right by a tenth of the frame: more of its left is seen.
    const box = (await win.boundingBox())!;
    await drag(page, win, box.width / 4, 0);
    await page.keyboard.press("Enter");
    const m = await model(page);
    // The left crop shrank and the right one grew by the same amount.
    expect(m.pic!.crop![0]).toBeLessThan(0.25);
    expect(m.pic!.crop![2]).toBeGreaterThan(0.25);
    expect(m.pic!.crop![0] + m.pic!.crop![2]).toBeCloseTo(0.5, 2);
    expect(m.w).toBeCloseTo(80, 0);
    // Escape: nothing changes.
    const before = (await model(page)).pic!.crop;
    await select(page);
    await strip(page).getByRole("button", { name: "Crop", exact: true }).click();
    await drag(page, page.locator(".crop-handle.e"), -30, 0);
    await page.keyboard.press("Escape");
    await expect(page.locator(".crop-layer")).toHaveCount(0);
    expect((await model(page)).pic!.crop).toEqual(before);
  });

  test("an aspect ratio crops the middle of the picture to that shape", async ({ page }) => {
    await open(page);
    await select(page);
    await strip(page).getByRole("button", { name: "Crop", exact: true }).click();
    await page.locator(".crop-strip").getByRole("button", { name: "Aspect ratio" }).click();
    await page.getByRole("menuitem", { name: "Square, 1:1" }).click();
    await page.locator(".crop-strip").getByRole("button", { name: "Done" }).click();
    const m = await model(page);
    // 160 x 80 becomes 80 x 80: a quarter off each side.
    expect(m.w).toBeCloseTo(m.h, 0);
    expect(m.pic!.crop![0]).toBeCloseTo(0.25, 2);
    expect(m.pic!.crop![2]).toBeCloseTo(0.25, 2);
    expect(m.pic!.crop![1]).toBeCloseTo(0, 3);
    // The same from the Rotate menu of the strip, now to 16:9, without opening the crop mode.
    await select(page);
    await strip(page).getByRole("button", { name: "Rotate and flip" }).click();
    await page.getByRole("menuitem", { name: "Crop to aspect ratio" }).click();
    await page.getByRole("menuitem", { name: "Widescreen, 16:9" }).click();
    const w = await model(page);
    expect(w.w / w.h).toBeCloseTo(16 / 9, 1);
  });

  test("Crop to shape writes the shape and Rectangle takes it off", async ({ page }) => {
    await open(page);
    await select(page);
    await strip(page).getByRole("button", { name: "Crop to shape" }).click();
    await page.getByRole("button", { name: "Oval" }).click();
    expect((await model(page)).pic!.geom).toBe("ellipse");
    expect(await savedXml(page)).toMatch(/<a:prstGeom prst="ellipse">/);
    await expect(atom(page).locator(".pic-clip")).toHaveCSS("clip-path", /path\(/);
    await strip(page).getByRole("button", { name: "Crop to shape" }).click();
    await expect(page.getByRole("button", { name: "Oval" })).toHaveAttribute("aria-pressed", "true");
    await page.getByRole("button", { name: "Rounded rectangle" }).click();
    expect((await model(page)).pic!.geom).toBe("roundRect");
    await strip(page).getByRole("button", { name: "Crop to shape" }).click();
    await page.getByRole("button", { name: "Rectangle", exact: true }).click();
    expect((await model(page)).pic).toBeUndefined();
  });

  test("the menus crop too: Format, Image has Crop, Crop to shape and Crop to aspect ratio", async ({ page }) => {
    await open(page);
    await select(page);
    await chooseMenu(page, "Format", "Image", "Crop to shape", "Triangle");
    expect((await model(page)).pic!.geom).toBe("triangle");
    await chooseMenu(page, "Format", "Image", "Crop to aspect ratio", "Landscape, 4:3");
    const m = await model(page);
    expect(m.w / m.h).toBeCloseTo(4 / 3, 1);
    await chooseMenu(page, "Format", "Image", "Crop");
    await expect(page.locator(".crop-layer")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator(".crop-layer")).toHaveCount(0);
  });
});

test.describe("turn and flip", () => {
  test("Rotate right and left turn by 90 degrees; Flip mirrors; each is one undo", async ({ page }) => {
    await open(page);
    await select(page);
    const turn = async (label: string) => {
      await strip(page).getByRole("button", { name: "Rotate and flip" }).click();
      await page.getByRole("menuitem", { name: label }).click();
    };
    await turn("Rotate right 90°");
    expect((await model(page)).pic!.rot).toBe(90);
    expect(await savedXml(page)).toMatch(/<a:xfrm rot="5400000">/);
    await turn("Rotate right 90°");
    expect((await model(page)).pic!.rot).toBe(180);
    await turn("Rotate left 90°");
    await turn("Rotate left 90°");
    await turn("Rotate left 90°");
    expect((await model(page)).pic!.rot).toBe(270);
    await turn("Flip horizontal");
    expect((await model(page)).pic!.flipH).toBe(true);
    expect(await savedXml(page)).toMatch(/flipH="1"/);
    await turn("Flip vertical");
    await turn("Flip vertical");
    expect((await model(page)).pic!.flipV).toBeUndefined();
    // The picture on screen is turned and its frame is turned with it.
    await expect(atom(page).locator(".pic")).toHaveCSS("transform", /matrix\(/);
    const f = await frame(page).evaluate((e) => e.style.transform);
    expect(f).toContain("rotate(270deg)");
    // Each turn and flip is one step: the last flip back, then the first one, then the flip across.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).pic!.flipV).toBe(true);
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).pic!.flipV).toBeUndefined();
    expect((await model(page)).pic!.flipH).toBe(true);
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).pic!.flipH).toBeUndefined();
  });

  test("the commands are in Format, Image, and a flip toggles its check mark", async ({ page }) => {
    await open(page);
    await select(page);
    await chooseMenu(page, "Format", "Image", "Rotate right 90°");
    expect((await model(page)).pic!.rot).toBe(90);
    await chooseMenu(page, "Format", "Image", "Flip vertical");
    expect((await model(page)).pic!.flipV).toBe(true);
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Format", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Image$/ }).click();
    await expect(page.locator('.menu-drop [role="menuitemcheckbox"]').filter({ hasText: "Flip vertical" })).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
  });

  test("dragging the handle above the picture turns it", async ({ page }) => {
    await open(page);
    await select(page);
    const handle = frame(page).locator(".obj-handle.rot");
    await expect(handle).toBeVisible();
    const c = await centre(atom(page).locator("img"));
    const h = await centre(handle);
    // A quarter turn clockwise: from straight up to straight right.
    const r = Math.hypot(h.x - c.x, h.y - c.y);
    await page.mouse.move(h.x, h.y);
    await page.mouse.down();
    await page.mouse.move(c.x + r * 0.5, c.y - r * 0.5, { steps: 3 });
    await page.mouse.move(c.x + r, c.y, { steps: 3 });
    await page.mouse.up();
    const m = await model(page);
    expect(m.pic!.rot).toBeGreaterThan(85);
    expect(m.pic!.rot).toBeLessThan(95);
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).pic).toBeUndefined();
  });

});

test.describe("moving a floating picture", () => {
  test("a picture in front of the text follows the pointer and keeps what its position counts from", async ({ page }) => {
    await open(page, { w: 100, h: 50, anchor: { wrap: "none", relH: "page", x: 200, relV: "page", y: 300 } });
    await select(page);
    const before = await model(page);
    expect([before.x, before.y, before.relX, before.relY]).toEqual([200, 300, "page", "page"]);
    // Drag by the frame's edge, and by the picture itself: 60 px right and 30 px down.
    const z = await inPage(page, (w) => w.fe.editor.zoom);
    await drag(page, atom(page).locator("img"), 60, 30);
    const m = await model(page);
    expect(m.x).toBeCloseTo(200 + 60 / PT_PX / z, 0);
    expect(m.y).toBeCloseTo(300 + 30 / PT_PX / z, 0);
    expect([m.relX, m.relY]).toEqual(["page", "page"]);
    const xml = await savedXml(page);
    expect(xml).toMatch(/<wp:positionH relativeFrom="page"><wp:posOffset>\d+<\/wp:posOffset>/);
    // On screen it is where it was dropped.
    const box = (await atom(page).locator("img").boundingBox())!;
    const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
    expect((box.x - sheet.x) / PT_PX / z).toBeCloseTo(m.x!, 0);
    expect((box.y - sheet.y) / PT_PX / z).toBeCloseTo(m.y!, 0);
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).x).toBeCloseTo(200, 0);
  });

  test("a picture that text wraps around is moved the same way, from the paragraph", async ({ page }) => {
    await open(page, { w: 100, h: 50, anchor: { wrap: "square", relH: "column", x: 0, relV: "paragraph", y: 0 } });
    await select(page);
    const z = await inPage(page, (w) => w.fe.editor.zoom);
    await drag(page, atom(page).locator("img"), 120, 40);
    const m = await model(page);
    expect(m.wrap).toBe("square");
    expect(m.x!).toBeGreaterThan(80);
    expect(m.y!).toBeGreaterThan(20);
    expect([m.relX, m.relY]).toEqual(["column", "paragraph"]);
    const xml = await savedXml(page);
    expect(xml).toMatch(/<wp:positionH relativeFrom="column"><wp:posOffset>\d+<\/wp:posOffset>/);
    expect(xml).toMatch(/<wp:positionV relativeFrom="paragraph"><wp:posOffset>\d+<\/wp:posOffset>/);
    void z;
  });

  test("a picture in line with the text is not dragged away, and a click does not move a floating one", async ({ page }) => {
    await open(page);
    await select(page);
    const before = await atom(page).locator("img").boundingBox();
    await drag(page, atom(page).locator("img"), 80, 40);
    expect((await model(page)).wrap).toBe("inline");
    const after = await atom(page).locator("img").boundingBox();
    expect(after!.x).toBeCloseTo(before!.x, 0);
    await open(page, { w: 100, h: 50, anchor: { wrap: "none", relH: "page", x: 200, relV: "page", y: 300 } });
    await select(page);
    await atom(page).locator("img").click();
    expect((await model(page)).x).toBeCloseTo(200, 0);
  });
});

test.describe("the picture strip and menus", () => {
  test("the strip has Crop, Crop to shape, Rotate and flip, Format picture and Position and layout", async ({ page }) => {
    await open(page);
    await select(page);
    for (const name of ["Crop", "Crop to shape", "Rotate and flip", "Format picture", "Position and layout", "Arrange", "Alt", "Delete image"]) {
      await expect(strip(page).getByRole("button", { name, exact: true })).toBeVisible();
    }
  });

  test("the right-click menu on a picture offers Crop, Format picture and Position and layout", async ({ page }) => {
    await open(page);
    await atom(page).click({ button: "right" });
    for (const label of ["Crop", "Format picture…", "Position and layout…"]) await expect(contextItem(page, label)).toBeVisible();
    await contextItem(page, "Crop").click();
    await expect(page.locator(".crop-layer")).toBeVisible();
    await page.keyboard.press("Escape");
  });

  test("a double-click on a picture opens Format picture", async ({ page }) => {
    await open(page);
    await atom(page).dblclick();
    await expect(page.getByRole("dialog", { name: "Format picture" })).toBeVisible();
  });
});
