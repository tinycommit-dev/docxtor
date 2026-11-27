// Format picture (outline, effects, adjustments, recolour, Reset picture), Position and layout (numbers, moves with
// text, wrapping and distances), and Insert, Image, From a URL with the picture sources extensions can add.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Page } from "./helpers";
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
  alignY?: string;
  pic?: {
    crop?: number[];
    rot?: number;
    line?: { color: string; widthPt: number; dash: string };
    shadow?: { angle: number; distPt: number; blurPt: number };
    glow?: { radiusPt: number; color: string };
    softEdge?: number;
    reflection?: { size: number; distPt: number };
    bright?: number;
    contrast?: number;
    opacity?: number;
    recolor?: { kind: string; color1?: string; color2?: string };
  };
  place?: { dist: number[]; wrapText?: string; through?: boolean; locked?: boolean; overlap: boolean; alignH?: string };
}

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

async function model(page: Page): Promise<Pic> {
  const at = await atom(page).evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
  return inPage(page, (w, pos) => w.fe.editor.engine!.pictureAt(pos), at) as unknown as Pic;
}

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

/** Select the picture and open Format picture from the strip. */
async function openFormat(page: Page) {
  await atom(page).click();
  await strip(page).getByRole("button", { name: "Format picture" }).click();
  const d = page.getByRole("dialog", { name: "Format picture" });
  await expect(d).toBeVisible();
  return d;
}

/** Set a number field of a dialog and leave it, as typing a value and pressing Tab does. */
async function setNumber(page: Page, label: string, value: string) {
  const input = page.getByLabel(label, { exact: true });
  await input.fill(value);
  await input.press("Tab");
}

test.describe("Format picture", () => {
  test("the outline, effects, adjustments and recolour are chosen with a live preview, and Apply writes them as one step", async ({ page }) => {
    await open(page);
    const d = await openFormat(page);
    const preview = d.locator(".pf-preview");
    // The preview is drawn the way the page draws the picture.
    await expect(preview.locator("img")).toBeVisible();
    // Outline: a line, 3 pt, dashed, in the first accent colour of the theme.
    await d.getByRole("radio", { name: "Line" }).click();
    await d.getByLabel("Outline width").selectOption("3");
    await d.getByLabel("Outline style").selectOption("dash");
    await d.locator(".field", { hasText: "Outline color" }).getByRole("button", { name: "#ED7D31" }).click();
    await expect(preview.locator(".pic-ln path")).toHaveAttribute("stroke", "#ED7D31");
    // Effects.
    await d.getByLabel("Shadow", { exact: true }).selectOption("br");
    await expect(preview.locator(".pic")).toHaveCSS("filter", /drop-shadow/);
    await d.getByLabel("Glow", { exact: true }).selectOption("8");
    await d.getByLabel("Reflection", { exact: true }).selectOption("half");
    await d.getByLabel("Soft edges", { exact: true }).selectOption("5");
    // Adjustments.
    await setNumber(page, "Brightness (percent)", "20");
    await setNumber(page, "Contrast (percent)", "-30");
    await setNumber(page, "Transparency (percent)", "25");
    await expect(preview.locator(".pic-clip")).toHaveCSS("opacity", "0.75");
    await d.getByRole("button", { name: "Grayscale" }).click();
    // Nothing has changed in the document yet.
    expect((await model(page)).pic).toBeUndefined();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toHaveCount(0);
    const m = await model(page);
    expect(m.pic!.line).toEqual({ color: "ED7D31", widthPt: 3, dash: "dash" });
    expect(m.pic!.shadow).toMatchObject({ angle: 45 });
    expect(m.pic!.glow).toMatchObject({ radiusPt: 8 });
    expect(m.pic!.reflection).toMatchObject({ size: 0.5, distPt: 4 });
    expect(m.pic!.softEdge).toBe(5);
    expect(m.pic!.bright).toBeCloseTo(0.2, 3);
    expect(m.pic!.contrast).toBeCloseTo(-0.3, 3);
    expect(m.pic!.opacity).toBeCloseTo(0.75, 3);
    expect(m.pic!.recolor!.kind).toBe("grayscale");
    const xml = await savedXml(page);
    for (const needle of ['<a:ln w="38100">', '<a:prstDash val="dash"/>', "<a:outerShdw", "<a:glow", "<a:reflection", '<a:softEdge rad="63500"/>', '<a:lum bright="20000" contrast="-30000"/>', '<a:alphaModFix amt="75000"/>', "<a:grayscl/>"]) {
      expect(xml, needle).toContain(needle);
    }
    // On the page the picture is drawn with all of it.
    await expect(atom(page).locator(".pic-ln path")).toHaveAttribute("stroke", "#ED7D31");
    await expect(atom(page).locator(".pic")).toHaveCSS("filter", /drop-shadow/);
    // It was one step.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).pic).toBeUndefined();
  });

  test("Cancel changes nothing, and what was already there shows in the dialog", async ({ page }) => {
    await open(page, { line: { color: "FF4F00", w: 2.25, dash: "sysDash" }, blip: '<a:lum bright="40000"/><a:alphaModFix amt="60000"/>' });
    const d = await openFormat(page);
    await expect(d.getByRole("radio", { name: "Line" })).toHaveAttribute("aria-checked", "true");
    await expect(d.getByLabel("Outline width")).toHaveValue("2.25");
    await expect(d.getByLabel("Outline style")).toHaveValue("sysDash");
    await expect(page.getByLabel("Brightness (percent)")).toHaveValue("40");
    await expect(page.getByLabel("Transparency (percent)")).toHaveValue("40");
    await d.getByLabel("Shadow", { exact: true }).selectOption("b");
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toHaveCount(0);
    expect((await model(page)).pic!.shadow).toBeUndefined();
  });

  test("recolour: sepia, washout, black and white, a theme colour pair, and No recolor", async ({ page }) => {
    await open(page);
    let d = await openFormat(page);
    await d.getByRole("button", { name: "Sepia" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await model(page)).pic!.recolor!.kind).toBe("duotone");
    expect(await savedXml(page)).toContain("<a:duotone>");
    d = await openFormat(page);
    await d.getByRole("button", { name: "No recolor" }).click();
    await d.getByRole("button", { name: "Washout" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    let m = await model(page);
    expect(m.pic!.recolor).toBeUndefined();
    expect([m.pic!.bright, m.pic!.contrast]).toEqual([0.7, -0.7]);
    d = await openFormat(page);
    await d.getByRole("button", { name: "Black and white" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await model(page)).pic!.recolor!.kind).toBe("blackWhite");
    expect(await savedXml(page)).toContain('<a:biLevel thresh="50000"/>');
    d = await openFormat(page);
    await d.getByRole("button", { name: "Dark theme color 2" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    m = await model(page);
    expect(m.pic!.recolor!.kind).toBe("duotone");
    // The second accent of the theme (Office's orange), darkened, and white.
    expect(m.pic!.recolor!.color2).toBe("FFFFFF");
    expect(m.pic!.recolor!.color1).toMatch(/^[0-9A-F]{6}$/);
    d = await openFormat(page);
    await d.getByRole("button", { name: "No recolor" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await model(page)).pic!.recolor).toBeUndefined();
  });

  test("Reset picture takes off every adjustment, effect, the outline, the shape and the crop", async ({ page }) => {
    await open(page, { crop: [0.25, 0, 0.25, 0], w: 80, rot: 20, geometry: "ellipse", line: { color: "000000", w: 2 }, blip: '<a:lum bright="30000"/>', effects: '<a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:srgbClr val="000000"/></a:outerShdw>' });
    expect((await model(page)).w).toBe(80);
    const d = await openFormat(page);
    await d.getByRole("button", { name: "Reset picture" }).click();
    await expect(d).toHaveCount(0);
    const m = await model(page);
    expect(m.pic).toBeUndefined();
    expect(m.w).toBeCloseTo(160, 0);
    const xml = await savedXml(page);
    expect(xml).not.toMatch(/a:lum|srcRect [lrtb]|effectLst|<a:ln|rot=/);
    await expect(atom(page).locator("img")).toBeVisible();
    // Reset is also in the menu and the undo brings it all back.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await model(page)).pic!.geom).toBe("ellipse");
    await atom(page).click();
    await chooseMenu(page, "Format", "Image", "Reset picture");
    expect((await model(page)).pic).toBeUndefined();
  });

  test("Format, Image, Format picture opens it, and an error is shown inside the dialog", async ({ page }) => {
    await open(page);
    await atom(page).click();
    await chooseMenu(page, "Format", "Image", "Format picture…");
    await expect(page.getByRole("dialog", { name: "Format picture" })).toBeVisible();
    await page.getByRole("button", { name: "Cancel" }).click();
  });
});

test.describe("Position and layout", () => {
  const anchored: PictureSpec = { w: 100, h: 50, anchor: { wrap: "square", relH: "column", x: 20, relV: "paragraph", y: 10, dist: [3, 6, 9, 12] } };

  async function openPosition(page: Page) {
    await atom(page).click();
    await strip(page).getByRole("button", { name: "Position and layout" }).click();
    const d = page.getByRole("dialog", { name: "Position and layout" });
    await expect(d).toBeVisible();
    return d;
  }

  test("a floating picture's position, wrapping and distances are shown as numbers and written as chosen", async ({ page }) => {
    await open(page, anchored);
    const d = await openPosition(page);
    // What the file says.
    await expect(d.getByLabel("Horizontal position (points)")).toHaveValue("20");
    await expect(d.getByLabel("Horizontal position is relative to")).toHaveValue("column");
    await expect(d.getByLabel("Vertical position (points)")).toHaveValue("10");
    await expect(d.getByLabel("Vertical position is relative to")).toHaveValue("paragraph");
    await expect(d.getByLabel("Move object with text")).toBeChecked();
    await expect(d.getByLabel("Lock anchor")).not.toBeChecked();
    await expect(d.getByLabel("Allow overlap")).toBeChecked();
    // Change it: 72 pt from the page, 36 pt from the margin, which does not move with the text.
    await d.getByLabel("Horizontal position is relative to").selectOption("page");
    await setNumber(page, "Horizontal position (points)", "72");
    await d.getByLabel("Vertical position is relative to").selectOption("margin");
    await setNumber(page, "Vertical position (points)", "36");
    await expect(d.getByLabel("Move object with text")).not.toBeChecked();
    await d.getByLabel("Lock anchor").check();
    await d.getByLabel("Allow overlap").uncheck();
    await d.getByRole("radio", { name: "Text wrapping" }).click();
    await d.getByRole("radio", { name: "Tight" }).click();
    await d.getByRole("radio", { name: "Left only" }).click();
    await setNumber(page, "Top distance from text (points)", "4");
    await setNumber(page, "Bottom distance from text (points)", "5");
    await setNumber(page, "Left distance from text (points)", "10");
    await setNumber(page, "Right distance from text (points)", "11");
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toHaveCount(0);
    const m = await model(page);
    expect([m.wrap, m.x, m.y, m.relX, m.relY]).toEqual(["tight", 72, 36, "page", "margin"]);
    expect(m.place).toMatchObject({ dist: [4, 5, 10, 11], wrapText: "left", locked: true, overlap: false });
    const xml = await savedXml(page);
    expect(xml).toContain('<wp:positionH relativeFrom="page"><wp:posOffset>914400</wp:posOffset></wp:positionH>');
    expect(xml).toContain('<wp:positionV relativeFrom="margin"><wp:posOffset>457200</wp:posOffset></wp:positionV>');
    expect(xml).toContain('distT="50800" distB="63500" distL="127000" distR="139700"');
    expect(xml).toContain('locked="1"');
    expect(xml).toContain('allowOverlap="0"');
    expect(xml).toMatch(/<wp:wrapTight wrapText="left"><wp:wrapPolygon edited="0">/);
    // One step.
    await page.keyboard.press("ControlOrMeta+z");
    expect(await model(page)).toMatchObject({ wrap: "square", x: 20, y: 10, relX: "column" });
  });

  test("an alignment can replace a position, and Through is its own wrapping", async ({ page }) => {
    await open(page, anchored);
    let d = await openPosition(page);
    await d.getByRole("radio", { name: "Alignment" }).first().click();
    await d.getByLabel("Horizontal alignment").selectOption("center");
    await d.getByLabel("Horizontal position is relative to").selectOption("margin");
    await d.getByRole("radio", { name: "Alignment" }).last().click();
    await d.getByLabel("Vertical alignment").selectOption("bottom");
    await d.getByLabel("Vertical position is relative to").selectOption("page");
    await d.getByRole("radio", { name: "Text wrapping" }).click();
    await d.getByRole("radio", { name: "Through" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    let m = await model(page);
    expect(m.alignY).toBe("bottom");
    expect(m.relY).toBe("page");
    expect(m.place).toMatchObject({ alignH: "center", through: true });
    expect(m.x).toBeUndefined();
    let xml = await savedXml(page);
    expect(xml).toContain('<wp:positionH relativeFrom="margin"><wp:align>center</wp:align></wp:positionH>');
    expect(xml).toContain('<wp:positionV relativeFrom="page"><wp:align>bottom</wp:align></wp:positionV>');
    expect(xml).toContain("<wp:wrapThrough");
    // The dialog shows what is there now, and Square puts the plain rectangle back.
    d = await openPosition(page);
    await expect(d.getByLabel("Horizontal alignment")).toHaveValue("center");
    await d.getByRole("radio", { name: "Text wrapping" }).click();
    await expect(d.getByRole("radio", { name: "Through" })).toHaveAttribute("aria-checked", "true");
    await d.getByRole("radio", { name: "Square" }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    m = await model(page);
    expect(m.wrap).toBe("square");
    xml = await savedXml(page);
    expect(xml).toContain("<wp:wrapSquare");
    expect(xml).not.toContain("wrapThrough");
  });

  test("turning Move object with text off keeps the picture where it is, with the number counted from the page", async ({ page }) => {
    // In front of the text, so the page draws it exactly where its numbers say.
    await open(page, { ...anchored, anchor: { ...anchored.anchor, wrap: "none" } });
    const d = await openPosition(page);
    const before = Number(await d.getByLabel("Vertical position (points)").inputValue());
    await d.getByLabel("Move object with text").uncheck();
    await expect(d.getByLabel("Vertical position is relative to")).toHaveValue("page");
    const after = Number(await d.getByLabel("Vertical position (points)").inputValue());
    // The picture is lower than the top of its paragraph by the page's top margin and the line above it.
    expect(after).toBeGreaterThan(before + 60);
    // Where it is on screen is where the number says, counted from the top of the page.
    const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
    const z = await inPage(page, (w) => w.fe.editor.zoom);
    const box = (await atom(page).locator("img").boundingBox())!;
    expect(after).toBeCloseTo((box.y - sheet.y) / PT_PX / z, 0);
    await d.getByRole("button", { name: "Apply" }).click();
    const m = await model(page);
    expect(m.relY).toBe("page");
    expect(m.y).toBeCloseTo(after, 1);
    // And on again: counted from the paragraph.
    const d2 = await openPosition(page);
    await d2.getByLabel("Move object with text").check();
    await expect(d2.getByLabel("Vertical position is relative to")).toHaveValue("paragraph");
    expect(Number(await d2.getByLabel("Vertical position (points)").inputValue())).toBeCloseTo(before, 0);
    await d2.getByRole("button", { name: "Cancel" }).click();
  });

  test("a picture in line with the text has no position until it is given a wrapping style", async ({ page }) => {
    await open(page);
    const d = await openPosition(page);
    // It opens on the wrapping tab, with In line chosen; the position tab says why it has nothing.
    await expect(d.getByRole("radio", { name: "In line with text" })).toHaveAttribute("aria-checked", "true");
    await d.getByRole("radio", { name: "Position" }).last().click();
    await expect(d.getByText("in line with the text, so it has no position")).toBeVisible();
    await expect(d.getByLabel("Horizontal position (points)")).toBeHidden();
    await d.getByRole("radio", { name: "Text wrapping" }).click();
    await d.getByRole("radio", { name: "Top and bottom" }).click();
    await expect(d.getByLabel("Left distance from text (points)")).toBeDisabled();
    await expect(d.getByLabel("Top distance from text (points)")).toBeEnabled();
    await d.getByRole("radio", { name: "Square" }).click();
    await d.getByRole("radio", { name: "Position" }).last().click();
    await expect(d.getByLabel("Horizontal position (points)")).toBeVisible();
    await d.getByRole("button", { name: "Apply" }).click();
    expect((await model(page)).wrap).toBe("square");
    expect(await savedXml(page)).toContain("<wp:anchor");
  });

  test("the distance from the text shows on the page", async ({ page }) => {
    await open(page, anchored);
    // 3, 6, 9 and 12 pt: the picture floats left, so the text keeps 12 pt away on its right side.
    const margin = await atom(page).evaluate((a) => ({ right: parseFloat(getComputedStyle(a).marginRight), bottom: parseFloat(getComputedStyle(a).marginBottom) }));
    expect(margin.right).toBeCloseTo(12 * (96 / 72), 0);
    expect(margin.bottom).toBeCloseTo(6 * (96 / 72), 0);
    const d = await openPosition(page);
    await d.getByRole("radio", { name: "Text wrapping" }).click();
    await setNumber(page, "Right distance from text (points)", "30");
    await d.getByRole("button", { name: "Apply" }).click();
    const wider = await atom(page).evaluate((a) => parseFloat(getComputedStyle(a).marginRight));
    expect(wider).toBeCloseTo(30 * (96 / 72), 0);
  });

  test("Format, Image, Position and layout opens it", async ({ page }) => {
    await open(page, anchored);
    await atom(page).click();
    await chooseMenu(page, "Format", "Image", "Position and layout…");
    await expect(page.getByRole("dialog", { name: "Position and layout" })).toBeVisible();
    await page.keyboard.press("Escape");
  });
});

test.describe("pictures from a URL and other sources", () => {
  const PICTURE = quarters(60, 30);

  /** A page with an empty document and the caret in it. */
  async function blank(page: Page) {
    await openBuilt(page, { sections: [{ paragraphs: ["A line of text"] }] });
    await page.locator(".flow .fp").first().click();
  }

  test("Insert, Image lists From your computer and From a URL", async ({ page }) => {
    await blank(page);
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Insert", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Image$/ }).click();
    await expect(page.locator('.menu-drop [role="menuitem"]').filter({ hasText: "From your computer…" })).toBeVisible();
    await expect(page.locator('.menu-drop [role="menuitem"]').filter({ hasText: "From a URL…" })).toBeVisible();
    await page.keyboard.press("Escape");
  });

  test("a picture is fetched from the address by the browser and put in the document, with a plain note saying so", async ({ page }) => {
    await page.route("https://pictures.example/photos/Logo%20one.png", (route) => route.fulfill({ status: 200, contentType: "image/png", headers: { "access-control-allow-origin": "*" }, body: Buffer.from(PICTURE) }));
    await blank(page);
    await chooseMenu(page, "Insert", "Image", "From a URL…");
    const d = page.getByRole("dialog", { name: "Picture from a URL" });
    await expect(d).toBeVisible();
    // It says plainly that the address is fetched from this device.
    await expect(d).toContainText("Your browser fetches this address directly");
    await d.getByLabel("Picture address").fill("https://pictures.example/photos/Logo%20one.png");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(d).toHaveCount(0);
    await expect(page.locator(".flow .atom.img img")).toBeVisible();
    // 60 x 30 pixels at 96 to the inch: 45 x 22.5 points; the name is the alt text.
    const m = (await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      const seg = e.renderAll().blocks.flatMap((b) => ("segs" in b ? (b as unknown as { segs: { k: string; img?: unknown }[] }).segs : [])).find((s) => s.k === "img");
      return seg?.img;
    })) as { w: number; h: number; alt: string };
    expect(m.w).toBeCloseTo(45, 0);
    expect(m.h).toBeGreaterThanOrEqual(22);
    expect(m.h).toBeLessThanOrEqual(23);
    expect(m.alt).toBe("Logo one.png");
    // The picture is in the file, not just linked.
    const media = await inPage(page, (w) => w.fe.editor.engine!.save().length);
    expect(media).toBeGreaterThan(0);
    await page.keyboard.press("ControlOrMeta+z");
    await expect(page.locator(".flow .atom.img")).toHaveCount(0);
  });

  test("the file extension comes from the picture's bytes, not from an address or name without one", async ({ page }) => {
    await page.route("https://pictures.example/abc123", (route) => route.fulfill({ status: 200, contentType: "application/octet-stream", headers: { "access-control-allow-origin": "*" }, body: Buffer.from(PICTURE) }));
    await blank(page);
    await chooseMenu(page, "Insert", "Image", "From a URL…");
    const d = page.getByRole("dialog", { name: "Picture from a URL" });
    await d.getByLabel("Picture address").fill("https://pictures.example/abc123");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(page.locator(".flow .atom.img img")).toBeVisible();
    const src = await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      const seg = e.renderAll().blocks.flatMap((b) => ("segs" in b ? (b as unknown as { segs: { k: string; img?: { src: string } }[] }).segs : [])).find((s) => s.k === "img");
      return seg?.img?.src;
    });
    expect(src).toMatch(/\.png$/);
  });

  test("a picture format Word cannot show is refused with a plain message", async ({ page }) => {
    await page.route("https://pictures.example/photo", (route) => route.fulfill({ status: 200, contentType: "image/avif", headers: { "access-control-allow-origin": "*" }, body: Buffer.from("\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf") }));
    await blank(page);
    await chooseMenu(page, "Insert", "Image", "From a URL…");
    const d = page.getByRole("dialog", { name: "Picture from a URL" });
    await d.getByLabel("Picture address").fill("https://pictures.example/photo");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(page.locator(".toast")).toContainText("isn't a picture Docxtor can use");
    await expect(page.locator(".flow .atom.img")).toHaveCount(0);
  });

  test("a fetch that is blocked says why in plain words and offers the way round it", async ({ page }) => {
    await page.route("https://blocked.example/**", (route) => route.abort("failed"));
    await blank(page);
    await chooseMenu(page, "Insert", "Image", "From a URL…");
    const d = page.getByRole("dialog", { name: "Picture from a URL" });
    await d.getByLabel("Picture address").fill("https://blocked.example/cat.png");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(d.getByRole("status")).toContainText("could not be fetched");
    await expect(d.getByRole("status")).toContainText("may not let other pages read its pictures");
    await expect(d.getByRole("status")).toContainText("From your computer");
    // The dialog stays so the address can be fixed.
    await expect(d.getByLabel("Picture address")).toBeEnabled();
    await expect(page.locator(".flow .atom.img")).toHaveCount(0);
  });

  test("an address that is not a picture, a missing one and a mistyped one each get their own message", async ({ page }) => {
    await page.route("https://pictures.example/page.html", (route) => route.fulfill({ status: 200, contentType: "text/html", headers: { "access-control-allow-origin": "*" }, body: "<p>hi</p>" }));
    await page.route("https://pictures.example/gone.png", (route) => route.fulfill({ status: 404, contentType: "text/plain", headers: { "access-control-allow-origin": "*" }, body: "no" }));
    await blank(page);
    await chooseMenu(page, "Insert", "Image", "From a URL…");
    const d = page.getByRole("dialog", { name: "Picture from a URL" });
    const status = d.getByRole("status");
    const input = d.getByLabel("Picture address");
    await input.fill("https://pictures.example/page.html");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(status).toContainText("not a picture (it sent text/html)");
    await input.fill("https://pictures.example/gone.png");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(status).toContainText("answered 404");
    await input.fill("not an address");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(status).toContainText("isn't a web address");
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toHaveCount(0);
    await expect(page.locator(".flow .atom.img")).toHaveCount(0);
  });

  test("an extension adds a picture source: it shows in Insert, Image and its picture is inserted", async ({ page }) => {
    await blank(page);
    const bytes = Array.from(PICTURE);
    await inPage(
      page,
      async (w, b) => {
        await (w.fe as unknown as { use(x: unknown): Promise<void> }).use({
          id: "acme.stock",
          name: "Stock photos",
          activate(ctx: { pictures: { sources: { register(s: unknown): void } } }) {
            ctx.pictures.sources.register({
              id: "acme.stock",
              label: "From stock photos…",
              note: "Sends nothing.",
              pick: async () => ({ bytes: new Uint8Array(b), name: "stock-tree.png", type: "image/png", alt: "A tree" }),
            });
          },
        });
      },
      bytes,
    );
    await chooseMenu(page, "Insert", "Image", "From stock photos…");
    await expect(page.locator(".flow .atom.img img")).toBeVisible();
    await expect(page.locator(".flow .atom.img img")).toHaveAttribute("alt", "A tree");
    // Turning the extension off takes its line away.
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("acme.stock"));
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Insert", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Image$/ }).click();
    await expect(page.locator('.menu-drop [role="menuitem"]').filter({ hasText: "From stock photos…" })).toHaveCount(0);
    await expect(page.locator('.menu-drop [role="menuitem"]').filter({ hasText: "From a URL…" })).toBeVisible();
    await page.keyboard.press("Escape");
  });
});
