// The Format shape panel: fill, line, effects, size and position, text box options; flip and turn; following the selection.
// Every change is checked in the engine's view model and on the page, and undone.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, expect, inPage, openFixture, RICH, test, type Locator, type Page } from "./helpers";
import { GROUP } from "./support/shape-markup";

const MOD = "ControlOrMeta";
const PT_PX = 96 / 72;

interface Stop {
  pos: number;
  color: string;
  alpha?: number;
}
interface Shape {
  geometry: string;
  w: number;
  h: number;
  x?: number;
  y?: number;
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  wrap: string;
  textBox: boolean;
  vAnchor: string;
  wrapText: boolean;
  autoFit?: boolean;
  autoFitMode?: string;
  vert?: string;
  columns?: number;
  columnGapPt?: number;
  insets: { l: number; t: number; r: number; b: number };
  fill?: { kind: string; color: string; alpha?: number; color2?: string; angle?: number; stops?: Stop[]; path?: string; rotWithShape?: boolean; src?: string; stretch?: Record<string, number>; tile?: Record<string, unknown>; pattern?: string };
  line?: { color: string; alpha?: number; widthPt: number; dash: string; compound?: string; cap?: string; join?: string; headArrow?: string; tailArrow?: string; headW?: string; tailLen?: string };
  effects?: {
    shadow?: { color: string; alpha?: number; blurPt: number; distPt: number; angle: number };
    innerShadow?: { blurPt: number };
    glow?: { color: string; radiusPt: number; alpha?: number };
    softEdgePt?: number;
    reflection?: { endPos?: number; startAlpha: number };
    bevel?: { preset: string; wPt: number; hPt: number };
    extrusion?: { heightPt: number; color?: string };
  };
  text?: unknown[];
}

const shapeAt = (page: Page, p = RICH.intro, o = 0) =>
  inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape>;
const run = (page: Page, ops: unknown) => inPage(page, (w, o) => w.fe.editor.exec(o), ops);
const atoms = (page: Page) => page.locator(".flow .atom.shape");
const strip = (page: Page) => page.locator(".shape-strip");
const panel = (page: Page) => page.locator('[data-dock="format-shape"]');
const svgOf = (page: Page, n = 0) => atoms(page).nth(n).locator("svg.shape-svg");
/** The path that draws the n-th shape (the ones in the marker and gradient definitions are not it). */
const drawn = (page: Page, n = 0) => atoms(page).nth(n).locator("svg.shape-svg > path").first();
const undo = (page: Page) => inPage(page, (w) => w.fe.editor.undo());

/** A part of the saved file as text (stored or deflated). */
async function savedPart(page: Page, name: string): Promise<string> {
  return page.evaluate(async (want) => {
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
      const entry = new TextDecoder().decode(bytes.subarray(p + 46, p + 46 + nlen));
      p += 46 + nlen + xlen + clen;
      if (entry !== want) continue;
      const dataStart = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(dataStart, dataStart + csize);
      if (method === 0) return new TextDecoder().decode(data);
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return await new Response(stream).text();
    }
    return "";
  }, name);
}

/** The control with this label inside a section of the panel (`fill`, `line`, `effects`, `size`, `textbox`). */
const field = (page: Page, section: string, label: string | RegExp): Locator => panel(page).locator(`[data-section="${section}"]`).getByLabel(label, { exact: typeof label === "string" }).filter({ visible: true });

/** Open a section of the panel by its heading (it stays open for the session). */
async function openSection(page: Page, name: string) {
  const head = panel(page).getByRole("button", { name, exact: true });
  if ((await head.getAttribute("aria-expanded")) !== "true") await head.click();
}

/** Put a rectangle at the start of the intro paragraph, select it and open the panel from its strip. */
async function withShape(page: Page, extra: Record<string, unknown> = {}) {
  await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 120, h: 72, fill: "4472C4", line: { color: "2F528F", width_pt: 1 }, text: "Hello", ...extra });
  await atoms(page).first().click();
  await expect(strip(page)).toBeVisible();
  await strip(page).getByRole("button", { name: "Format shape" }).click();
  await expect(panel(page)).toBeVisible();
}

test.describe("Format shape panel", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page, "rich.docx");
    await inPage(page, (w) => w.fe.editor.setZoom(1));
  });

  test("opens from the strip, the Format menu and the right-click menu, and follows the selection", async ({ page }) => {
    await run(page, [
      { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 100, h: 60, fill: "4472C4", line: { color: "2F528F", width_pt: 1 } },
      { op: "insert_shape", at: { p: RICH.highlights, o: 0 }, geometry: "ellipse", w: 100, h: 60, fill: "FFC000", line: { color: "7F6000", width_pt: 2 } },
    ]);
    const [a, b] = [atoms(page).nth(0), atoms(page).nth(1)];
    // From the Format menu.
    await a.click();
    await chooseMenu(page, "Format", "Shape", "Format shape…");
    await expect(panel(page)).toBeVisible();
    await expect(panel(page).getByRole("heading", { name: "Format shape" })).toBeVisible();
    await openSection(page, "Line");
    await expect(field(page, "line", "Width")).toHaveValue("1");
    await expect(field(page, "line", "Color")).toBeVisible();
    // Selecting the other shape updates the panel.
    await b.click();
    await expect(field(page, "line", "Width")).toHaveValue("2");
    await a.click();
    await expect(field(page, "line", "Width")).toHaveValue("1");
    // Nothing selected: a short hint, and the sections are gone.
    await page.locator(`.flow [data-p="${RICH.last}"]`).click();
    await expect(panel(page).getByText("Select a shape or text box to change how it looks.")).toBeVisible();
    await expect(panel(page).getByRole("button", { name: "Fill", exact: true })).toBeHidden();
    // The right-click menu on a shape opens it again after it was closed.
    await panel(page).getByRole("button", { name: "Close" }).click();
    await expect(panel(page)).toHaveCount(0);
    await b.click({ button: "right" });
    await contextItem(page, "Format shape…").click();
    await expect(panel(page)).toBeVisible();
    await expect(field(page, "line", "Width")).toHaveValue("2");
  });

  test("a solid fill, with a colour typed as hex and a transparency", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Fill");
    await expect(panel(page).getByRole("radio", { name: "Solid fill" })).toBeChecked();
    const hex = panel(page).locator(".fs-hex").first();
    await hex.fill("ff0000");
    await hex.press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.color).toBe("FF0000");
    await expect(drawn(page)).toHaveAttribute("fill", "#FF0000");
    // Transparency, typed in the number beside the slider.
    const pct = panel(page).locator('#fs-fill, .fs-row-slider input[type="number"]').first();
    await pct.fill("40");
    await pct.press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.alpha).toBeCloseTo(0.6, 2);
    await undo(page);
    await expect.poll(async () => (await shapeAt(page)).fill?.alpha).toBeUndefined();
    await undo(page);
    await expect.poll(async () => (await shapeAt(page)).fill?.color).toBe("4472C4");
    // No fill, and back to solid.
    await panel(page).getByRole("radio", { name: "No fill" }).check();
    await expect.poll(async () => (await shapeAt(page)).fill).toBeUndefined();
    await expect(drawn(page)).toHaveAttribute("fill", "none");
  });

  test("flip and turn: the strip, the Arrange menu, the right-click menu, undo", async ({ page }) => {
    await withShape(page);
    // The strip's button.
    await strip(page).getByRole("button", { name: "Flip horizontal" }).click();
    expect((await shapeAt(page)).flipH).toBe(true);
    await expect(strip(page).getByRole("button", { name: "Flip horizontal" })).toHaveAttribute("aria-pressed", "true");
    await undo(page);
    expect((await shapeAt(page)).flipH).toBeFalsy();
    // Format, Arrange, and the toggle shows in the menu.
    await chooseMenu(page, "Format", "Arrange", "Flip vertical");
    expect((await shapeAt(page)).flipV).toBe(true);
    await atoms(page).first().click({ button: "right" });
    await contextItem(page, "Flip vertical").click();
    expect((await shapeAt(page)).flipV).toBeFalsy();
    await chooseMenu(page, "Format", "Arrange", "Rotate right 90°");
    expect((await shapeAt(page)).rot).toBe(90);
    await chooseMenu(page, "Format", "Arrange", "Rotate left 90°");
    await chooseMenu(page, "Format", "Arrange", "Rotate left 90°");
    expect((await shapeAt(page)).rot).toBe(270);
  });

  test("size and position: units, the aspect ratio lock, rotation, and the place of a floating shape", async ({ page }) => {
    await withShape(page, { x: 36, y: 20 });
    await openSection(page, "Size and position");
    expect(await shapeAt(page)).toMatchObject({ w: 120, h: 72, x: 36, y: 20 });
    // Shown in inches, typed with a unit when the person likes: 2 inches, then 5 cm.
    await expect(field(page, "size", "Width")).toHaveValue("1.67");
    await field(page, "size", "Width").fill("2");
    await field(page, "size", "Width").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).w).toBe(144);
    await field(page, "size", "Height").fill("5 cm");
    await field(page, "size", "Height").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).h).toBeCloseTo(141.8, 1);
    await expect(field(page, "size", "Height")).toHaveValue("1.97");
    const box = await atoms(page).first().evaluate((e) => ({ w: parseFloat(getComputedStyle(e).width), h: parseFloat(getComputedStyle(e).height) }));
    expect(box.w).toBeCloseTo(144 * PT_PX, 0);
    expect(box.h).toBeCloseTo(141.8 * PT_PX, 0);
    // With the lock on, the other side follows.
    await panel(page).getByLabel("Lock aspect ratio").check();
    await field(page, "size", "Width").fill("1");
    await field(page, "size", "Width").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).w).toBe(72);
    expect((await shapeAt(page)).h).toBeCloseTo(70.9, 0);
    // One undo step for each change.
    await undo(page);
    expect(await shapeAt(page)).toMatchObject({ w: 144 });
    // Rotation.
    await field(page, "size", "Rotation").fill("45");
    await field(page, "size", "Rotation").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).rot).toBe(45);
    expect(await atoms(page).first().evaluate((e) => (e as HTMLElement).style.transform)).toBe("rotate(45deg)");
    // The place of a floating shape: 1 inch from the column.
    await field(page, "size", "Horizontal position").fill("1");
    await field(page, "size", "Horizontal position").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).x).toBe(72);
    await expect(panel(page).getByText("Counted from the column and the paragraph.")).toBeVisible();
    // The panel's turn and flip buttons.
    await panel(page).getByRole("button", { name: "Flip vertical" }).click();
    expect((await shapeAt(page)).flipV).toBe(true);
    await panel(page).getByRole("button", { name: "Rotate right 90°" }).click();
    expect((await shapeAt(page)).rot).toBe(135);
  });

  test("a shape in line with the text has no place to set", async ({ page }) => {
    await withShape(page, { floating: false });
    await openSection(page, "Size and position");
    await expect(field(page, "size", "Width")).toBeVisible();
    await expect(panel(page).getByLabel("Horizontal position")).toBeHidden();
  });

  test("the line: width, dash, and the arrowheads of an open line", async ({ page }) => {
    await withShape(page, { geometry: "straightConnector1", h: 0, fill: undefined, text: undefined, line: { color: "2F528F", width_pt: 1.5, tail_arrow: "triangle" } });
    await openSection(page, "Line");
    await expect(field(page, "line", "Width")).toHaveValue("1.5");
    await expect(field(page, "line", "End arrow type")).toHaveValue("triangle");
    await expect(field(page, "line", "Begin arrow type")).toHaveValue("none");
    await field(page, "line", "Width").fill("3");
    await field(page, "line", "Width").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).line?.widthPt).toBe(3);
    await field(page, "line", "Dash type").selectOption("lgDash");
    await expect.poll(async () => (await shapeAt(page)).line?.dash).toBe("lgDash");
    await field(page, "line", "Begin arrow type").selectOption("diamond");
    await expect.poll(async () => (await shapeAt(page)).line?.headArrow).toBe("diamond");
    expect((await shapeAt(page)).line?.tailArrow).toBe("triangle");
    const path = drawn(page);
    expect(Number(await path.getAttribute("stroke-width"))).toBeCloseTo(3 * PT_PX, 1);
    expect(await path.getAttribute("stroke-dasharray")).toMatch(/^[\d.]+ [\d.]+$/);
    await undo(page);
    expect((await shapeAt(page)).line?.headArrow).toBeUndefined();
    await undo(page);
    expect((await shapeAt(page)).line?.dash).toBe("solid");
    // A line has no fill section.
    await expect(panel(page).getByRole("button", { name: "Fill", exact: true })).toBeHidden();
  });

  test("the line: colour, transparency, compound, cap and join types, arrow sizes, and no line", async ({ page }) => {
    await withShape(page, { geometry: "straightConnector1", h: 0, fill: undefined, text: undefined, line: { color: "2F528F", width_pt: 4, tail_arrow: "triangle" } });
    await openSection(page, "Line");
    await field(page, "line", "Color").fill("c00000");
    await field(page, "line", "Color").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).line?.color).toBe("C00000");
    await field(page, "line", "Transparency").last().fill("25");
    await field(page, "line", "Transparency").last().press("Enter");
    await expect.poll(async () => (await shapeAt(page)).line?.alpha).toBeCloseTo(0.75, 2);
    await field(page, "line", "Compound type").selectOption("dbl");
    await expect.poll(async () => (await shapeAt(page)).line?.compound).toBe("dbl");
    await field(page, "line", "Cap type").selectOption("round");
    await expect.poll(async () => (await shapeAt(page)).line?.cap).toBe("round");
    await field(page, "line", "Join type").selectOption("bevel");
    await expect.poll(async () => (await shapeAt(page)).line?.join).toBe("bevel");
    await field(page, "line", "End arrow length").selectOption("lg");
    await expect.poll(async () => (await shapeAt(page)).line?.tailLen).toBe("lg");
    await field(page, "line", "Begin arrow type").selectOption("oval");
    await field(page, "line", "Begin arrow width").selectOption("sm");
    await expect.poll(async () => (await shapeAt(page)).line?.headW).toBe("sm");
    // The saved file says it the way Word does.
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('cap="rnd"');
    expect(xml).toContain('cmpd="dbl"');
    expect(xml).toContain("<a:bevel/>");
    expect(xml).toMatch(/<a:tailEnd type="triangle"[^>]*len="lg"/);
    expect(xml).toMatch(/<a:headEnd type="oval" w="sm"/);
    // No line: the outline goes, and so do its controls.
    await panel(page).getByRole("radio", { name: "No line" }).check();
    await expect.poll(async () => (await shapeAt(page)).line).toBeUndefined();
    await expect(field(page, "line", "Dash type")).toBeHidden();
    await panel(page).getByRole("radio", { name: "Solid line" }).check();
    await expect.poll(async () => (await shapeAt(page)).line?.widthPt).toBeGreaterThan(0);
    await expect(field(page, "line", "Dash type")).toBeVisible();
  });

  test("a gradient fill: add, move, recolour and remove stops; linear or path; angle", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Fill");
    await panel(page).getByRole("radio", { name: "Gradient fill" }).check();
    await expect.poll(async () => (await shapeAt(page)).fill?.kind).toBe("gradient");
    let f = (await shapeAt(page)).fill!;
    expect(f.stops).toHaveLength(2);
    expect(f.stops![0]).toMatchObject({ pos: 0, color: "4472C4" });
    expect(f.angle).toBe(90);
    await expect(svgOf(page).locator("linearGradient")).toHaveCount(1);
    await expect(panel(page).locator(".gr-stop")).toHaveCount(2);
    // A third stop in the widest gap, selected.
    await panel(page).getByRole("button", { name: "Add stop" }).click();
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.length).toBe(3);
    f = (await shapeAt(page)).fill!;
    expect(f.stops![1].pos).toBe(50);
    await expect(panel(page).locator(".gr-stop.on")).toHaveAttribute("aria-label", /Stop 2 of 3, at 50 percent/);
    // Its colour, transparency and position.
    await field(page, "fill", "Stop color").fill("00ff00");
    await field(page, "fill", "Stop color").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.[1].color).toBe("00FF00");
    await field(page, "fill", "Stop transparency").last().fill("50");
    await field(page, "fill", "Stop transparency").last().press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.[1].alpha).toBeCloseTo(0.5, 2);
    await field(page, "fill", "Stop position").fill("30");
    await field(page, "fill", "Stop position").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.[1].pos).toBe(30);
    // Dragging a handle along the bar moves its stop; the keys nudge it.
    const bar = (await panel(page).locator(".gr-strip").boundingBox())!;
    const handle = (await panel(page).locator(".gr-stop").nth(1).boundingBox())!;
    await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
    await page.mouse.down();
    await page.mouse.move(bar.x + bar.width * 0.8, handle.y + handle.height / 2, { steps: 6 });
    await page.mouse.up();
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.[1].pos).toBeGreaterThanOrEqual(79);
    expect((await shapeAt(page)).fill?.stops?.[1].pos).toBeLessThanOrEqual(81);
    await panel(page).locator(".gr-stop").nth(1).focus();
    await page.keyboard.press("ArrowLeft");
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.[1].pos).toBeGreaterThanOrEqual(78);
    await page.keyboard.press("Shift+ArrowLeft");
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.[1].pos).toBeLessThanOrEqual(70);
    // The angle, and rotating with the shape.
    await field(page, "fill", "Angle").fill("45");
    await field(page, "fill", "Angle").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.angle).toBe(45);
    await panel(page).getByLabel("Rotate with shape").uncheck();
    await expect.poll(async () => (await shapeAt(page)).fill?.rotWithShape).toBe(false);
    // The saved file has the gradient the way Word writes it.
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:gradFill");
    expect(xml).toMatch(/<a:gs pos="\d+"><a:srgbClr val="00FF00"><a:alpha val="50000"\/>/);
    expect(xml).toMatch(/<a:lin ang="2700000"/);
    // A path gradient: no angle, a radial gradient on the page.
    await field(page, "fill", "Type").selectOption("circle");
    await expect.poll(async () => (await shapeAt(page)).fill?.path).toBe("circle");
    await expect(svgOf(page).locator("radialGradient")).toHaveCount(1);
    await expect(field(page, "fill", "Angle")).toBeDisabled();
    await field(page, "fill", "Type").selectOption("linear");
    await expect.poll(async () => (await shapeAt(page)).fill?.path).toBeUndefined();
    // Removing a stop (not below two) with the button and the key.
    await panel(page).getByRole("button", { name: "Remove stop" }).click();
    await expect.poll(async () => (await shapeAt(page)).fill?.stops?.length).toBe(2);
    await expect(panel(page).getByRole("button", { name: "Remove stop" })).toBeDisabled();
    // Back to the solid colour of the first stop, and one undo step at a time.
    await panel(page).getByRole("radio", { name: "Solid fill" }).check();
    await expect.poll(async () => (await shapeAt(page)).fill?.kind).toBe("solid");
    await undo(page);
    await expect.poll(async () => (await shapeAt(page)).fill?.kind).toBe("gradient");
  });

  test("a picture fill: stretched with offsets, tiled with scale, and transparency", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Fill");
    await panel(page).getByRole("radio", { name: "Picture fill" }).check();
    // Nothing yet: the shape keeps its fill until a picture is chosen.
    expect((await shapeAt(page)).fill?.kind).toBe("solid");
    await panel(page).locator('input[type="file"]').setInputFiles({ name: "pixel.png", mimeType: "image/png", buffer: Buffer.from(PIXEL_PNG, "base64") });
    await expect.poll(async () => (await shapeAt(page)).fill?.kind).toBe("picture");
    const src = (await shapeAt(page)).fill!.src!;
    expect(src).toMatch(/\.png$/);
    await expect(panel(page).locator(".fs-thumb")).toBeVisible();
    await expect(atoms(page).first().locator("svg.shape-svg image, svg.shape-svg pattern")).not.toHaveCount(0);
    await expect(panel(page).getByRole("radio", { name: "Stretch picture to fill" })).toBeChecked();
    await field(page, "fill", "Offset left").fill("10");
    await field(page, "fill", "Offset left").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.stretch?.l).toBe(10);
    // Tiled: scale and offsets in points.
    await panel(page).getByRole("radio", { name: "Tile picture as texture" }).check();
    await expect.poll(async () => (await shapeAt(page)).fill?.tile).toMatchObject({ sx: 100, sy: 100, align: "tl", flip: "none" });
    await field(page, "fill", "Scale x").fill("50");
    await field(page, "fill", "Scale x").press("Enter");
    await expect.poll(async () => ((await shapeAt(page)).fill?.tile as { sx: number } | undefined)?.sx).toBe(50);
    await field(page, "fill", "Offset x").fill("6");
    await field(page, "fill", "Offset x").press("Enter");
    await field(page, "fill", "Alignment").selectOption("ctr");
    await field(page, "fill", "Mirror type").selectOption("xy");
    await expect.poll(async () => (await shapeAt(page)).fill?.tile).toMatchObject({ tx: 6, sx: 50, align: "ctr", flip: "xy" });
    await field(page, "fill", "Transparency").last().fill("30");
    await field(page, "fill", "Transparency").last().press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill?.alpha).toBeCloseTo(0.7, 2);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:blipFill");
    expect(xml).toMatch(/<a:tile [^>]*sx="50000"[^>]*flip="xy"[^>]*algn="ctr"/);
    expect(xml).toContain('<a:alphaModFix amt="70000"/>');
    // Undo steps back through the tile settings to the stretched picture.
    for (let i = 0; i < 6; i++) await undo(page);
    await expect.poll(async () => (await shapeAt(page)).fill?.tile).toBeUndefined();
  });

  test("a pattern fill: a tile from the grid, with its two colours, and the keys", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Fill");
    await panel(page).getByRole("radio", { name: "Pattern fill" }).check();
    await expect.poll(async () => (await shapeAt(page)).fill?.kind).toBe("pattern");
    expect(await panel(page).locator(".pat-tile").count()).toBe(48);
    await panel(page).locator('.pat-tile[data-pattern="dkHorz"]').click();
    await expect.poll(async () => (await shapeAt(page)).fill?.pattern).toBe("dkHorz");
    await expect(panel(page).locator('.pat-tile[data-pattern="dkHorz"]')).toHaveAttribute("aria-checked", "true");
    await field(page, "fill", "Foreground color").fill("c00000");
    await field(page, "fill", "Foreground color").press("Enter");
    await field(page, "fill", "Background color").fill("ffff00");
    await field(page, "fill", "Background color").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).fill).toMatchObject({ pattern: "dkHorz", color: "C00000", color2: "FFFF00" });
    // The arrow keys move through the tiles and pick them.
    await panel(page).locator('.pat-tile[data-pattern="dkHorz"]').focus();
    await page.keyboard.press("ArrowLeft");
    await expect.poll(async () => (await shapeAt(page)).fill?.pattern).toBe("dkVert");
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('<a:pattFill prst="dkVert">');
    expect(xml).toContain('<a:fgClr><a:srgbClr val="C00000"/>');
  });

  test("effects: shadow, glow, soft edges, reflection and 3-D format, each undone with one step", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Effects");
    const fx = async () => (await shapeAt(page)).effects;
    // Shadow: a preset, then its parts.
    await field(page, "effects", "Shadow presets").selectOption("outer:45");
    await expect.poll(async () => (await fx())?.shadow).toMatchObject({ angle: 45, distPt: 3, blurPt: 4 });
    await field(page, "effects", "Blur").first().fill("8");
    await field(page, "effects", "Blur").first().press("Enter");
    await field(page, "effects", "Distance").first().fill("6");
    await field(page, "effects", "Distance").first().press("Enter");
    await field(page, "effects", "Angle").fill("90");
    await field(page, "effects", "Angle").press("Enter");
    await expect.poll(async () => (await fx())?.shadow).toMatchObject({ blurPt: 8, distPt: 6, angle: 90 });
    await field(page, "effects", "Color").first().fill("ff0000");
    await field(page, "effects", "Color").first().press("Enter");
    await expect.poll(async () => (await fx())?.shadow?.color).toBe("FF0000");
    expect(await field(page, "effects", "Shadow presets").inputValue()).toBe("outer:90");
    // The shape draws a shadow on the page.
    await expect(atoms(page).first().locator("svg.shape-svg")).toHaveCSS("filter", /drop-shadow|none/);
    let xml = await savedPart(page, "word/document.xml");
    expect(xml).toMatch(/<a:outerShdw [^>]*blurRad="101600"[^>]*dist="76200"[^>]*dir="5400000"/);
    // An inner shadow replaces the outer one.
    await field(page, "effects", "Shadow presets").selectOption("inner:315");
    await expect.poll(async () => (await fx())?.innerShadow).toBeTruthy();
    expect((await fx())?.shadow).toBeUndefined();
    await field(page, "effects", "Shadow presets").selectOption("none");
    await expect.poll(async () => (await fx())?.innerShadow).toBeUndefined();
    // Glow.
    await field(page, "effects", "Glow presets").selectOption("8");
    await expect.poll(async () => (await fx())?.glow).toMatchObject({ radiusPt: 8 });
    await field(page, "effects", "Size").last().fill("12");
    await field(page, "effects", "Size").last().press("Enter");
    await expect.poll(async () => (await fx())?.glow?.radiusPt).toBe(12);
    expect(await field(page, "effects", "Glow presets").inputValue()).toBe("custom");
    // Soft edges.
    await field(page, "effects", "Soft edge presets").selectOption("5");
    await expect.poll(async () => (await fx())?.softEdgePt).toBe(5);
    // Reflection.
    await field(page, "effects", "Reflection presets").selectOption("half");
    await expect.poll(async () => (await fx())?.reflection?.endPos).toBeCloseTo(55.5, 1);
    expect(await field(page, "effects", "Reflection presets").inputValue()).toBe("half");
    // 3-D format: a bevel and a depth.
    await field(page, "effects", "Bevel").selectOption("circle");
    await expect.poll(async () => (await fx())?.bevel).toMatchObject({ preset: "circle", wPt: 6, hPt: 6 });
    await field(page, "effects", "Bevel width").fill("3");
    await field(page, "effects", "Bevel width").press("Enter");
    await expect.poll(async () => (await fx())?.bevel?.wPt).toBe(3);
    await field(page, "effects", "Depth").fill("10");
    await field(page, "effects", "Depth").press("Enter");
    await expect.poll(async () => (await fx())?.extrusion?.heightPt).toBe(10);
    xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:glow ");
    expect(xml).toContain("<a:softEdge ");
    expect(xml).toContain("<a:reflection ");
    expect(xml).toContain('<a:bevelT w="38100" h="76200" prst="circle"/>');
    expect(xml).toContain("<a:sp3d ");
    // Taking everything off again.
    await field(page, "effects", "Glow presets").selectOption("none");
    await field(page, "effects", "Soft edge presets").selectOption("none");
    await field(page, "effects", "Reflection presets").selectOption("none");
    await field(page, "effects", "Bevel").selectOption("none");
    await field(page, "effects", "Depth").fill("0");
    await field(page, "effects", "Depth").press("Enter");
    await expect.poll(async () => await fx()).toBeUndefined();
    // And undo brings the last one back.
    await undo(page);
    await expect.poll(async () => (await fx())?.extrusion?.heightPt).toBe(10);
  });

  test("text box options: margins, alignment, autofit, direction, wrap and columns", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Text box");
    await field(page, "textbox", "Left margin").fill("0.5");
    await field(page, "textbox", "Left margin").press("Enter");
    await field(page, "textbox", "Top margin").fill("0.25");
    await field(page, "textbox", "Top margin").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).insets).toMatchObject({ l: 36, t: 18 });
    await field(page, "textbox", "Vertical alignment").selectOption("bottom");
    await expect.poll(async () => (await shapeAt(page)).vAnchor).toBe("bottom");
    await panel(page).getByRole("radio", { name: "Resize shape to fit text" }).check();
    await expect.poll(async () => (await shapeAt(page)).autoFitMode).toBe("shape");
    await panel(page).getByRole("radio", { name: "Shrink text on overflow" }).check();
    await expect.poll(async () => (await shapeAt(page)).autoFitMode).toBe("text");
    await panel(page).getByRole("radio", { name: "Do not autofit" }).check();
    await expect.poll(async () => (await shapeAt(page)).autoFitMode).toBeUndefined();
    await field(page, "textbox", "Text direction").selectOption("vert270");
    await expect.poll(async () => (await shapeAt(page)).vert).toBe("vert270");
    await panel(page).getByLabel("Wrap text in shape").uncheck();
    await expect.poll(async () => (await shapeAt(page)).wrapText).toBe(false);
    await expect(field(page, "textbox", "Column spacing")).toBeDisabled();
    await field(page, "textbox", "Columns").fill("2");
    await field(page, "textbox", "Columns").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).columns).toBe(2);
    await expect(field(page, "textbox", "Column spacing")).toBeEnabled();
    await field(page, "textbox", "Column spacing").fill("0.25");
    await field(page, "textbox", "Column spacing").press("Enter");
    await expect.poll(async () => (await shapeAt(page)).columnGapPt).toBe(18);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toMatch(/<wps:bodyPr [^>]*lIns="457200"[^>]*tIns="228600"/);
    expect(xml).toContain('vert="vert270"');
    expect(xml).toContain('numCol="2"');
    expect(xml).toContain('spcCol="228600"');
    await undo(page);
    await expect.poll(async () => (await shapeAt(page)).columnGapPt).not.toBe(18);
    // A shape with no text frame has no Text box section.
    await panel(page).getByRole("button", { name: "Close" }).click();
    await run(page, { op: "insert_shape", at: { p: RICH.highlights, o: 0 }, geometry: "ellipse", w: 60, h: 40, fill: "FFC000" });
    await atoms(page).nth(1).click();
    await strip(page).getByRole("button", { name: "Format shape" }).click();
    await expect(panel(page).getByRole("button", { name: "Text box", exact: true })).toBeHidden();
  });

  test("the panel follows what is selected inside a group: the group has no look of its own, a member has", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before", { raw: GROUP }, "After"] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    const group = atoms(page).first();
    const left = group.locator(".shape-child").nth(0);
    await left.click();
    await strip(page).getByRole("button", { name: "Format shape" }).click();
    await expect(panel(page).getByText(/A group has no fill or line of its own/)).toBeVisible();
    await expect(panel(page).getByRole("button", { name: "Fill", exact: true })).toBeHidden();
    await expect(panel(page).getByRole("button", { name: "Size and position", exact: true })).toBeVisible();
    // The next click selects a member: its fill is its own.
    await left.click();
    await expect(panel(page).getByText("A shape inside a group.")).toBeVisible();
    await openSection(page, "Fill");
    await expect(panel(page).getByRole("radio", { name: "Solid fill" })).toBeChecked();
    await expect(field(page, "fill", "Color")).toHaveValue("FF0000");
    await field(page, "fill", "Color").fill("0000ff");
    await field(page, "fill", "Color").press("Enter");
    const at = await atoms(page).first().evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
    const g = (await shapeAt(page, at.p, at.o)) as Shape & { children: Shape[] };
    expect(g.children[0].fill?.color).toBe("0000FF");
    expect(g.children[1].fill?.color).toBe("00FF00");
    // Its size is in the group's own measures; the other member is untouched by a change to this one.
    await openSection(page, "Line");
    await panel(page).getByRole("radio", { name: "Solid line" }).check();
    const g2 = (await shapeAt(page, at.p, at.o)) as Shape & { children: Shape[] };
    expect(g2.children[0].line).toBeTruthy();
    expect(g2.children[1].line).toBeUndefined();
    // Another member: the panel updates.
    const right = group.locator(".shape-child").nth(1);
    await right.click();
    await right.click();
    await expect(field(page, "fill", "Color")).toHaveValue("00FF00");
    await expect(panel(page).getByRole("radio", { name: "No line" })).toBeChecked();
    await undo(page);
  });

  test("the keyboard: the panel takes the focus, Tab walks its fields, Mod+Z is the document's, Escape closes it", async ({ page }) => {
    await withShape(page);
    // The first heading has the focus after opening.
    await expect(panel(page).getByRole("button", { name: "Fill", exact: true })).toBeFocused();
    await page.keyboard.press("Space");
    await page.keyboard.press("Space");
    await expect(panel(page).getByRole("button", { name: "Fill", exact: true })).toHaveAttribute("aria-expanded", "true");
    await openSection(page, "Line");
    // Tab goes on to the next control in the panel.
    await page.keyboard.press("Tab");
    expect(await page.evaluate(() => !!document.activeElement?.closest('[data-dock="format-shape"]'))).toBe(true);
    // Change the line width with the keyboard, then undo it from the field itself.
    const width = field(page, "line", "Width");
    await width.focus();
    await width.fill("5");
    await width.press("Enter");
    await expect.poll(async () => (await shapeAt(page)).line?.widthPt).toBe(5);
    await expect(width).toBeFocused();
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(async () => (await shapeAt(page)).line?.widthPt).toBe(1);
    // Escape in the panel closes it and the page has the focus.
    await panel(page).getByRole("button", { name: "Line", exact: true }).focus();
    await page.keyboard.press("Escape");
    await expect(panel(page)).toHaveCount(0);
  });

  test("dragging a slider is one undo step, and a change with two shapes selected is for both", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Fill");
    const range = panel(page).locator('[data-section="fill"] input[type="range"]:visible').first();
    // Three input events and the end of the drag.
    await range.evaluate((el: HTMLInputElement) => {
      for (const v of ["10", "20", "35"]) {
        el.value = v;
        el.dispatchEvent(new Event("input", { bubbles: true }));
      }
      el.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect.poll(async () => (await shapeAt(page)).fill?.alpha).toBeCloseTo(0.65, 2);
    await undo(page);
    expect((await shapeAt(page)).fill?.alpha).toBeUndefined();
    // Two shapes selected: the panel's change reaches both.
    await run(page, { op: "insert_shape", at: { p: RICH.highlights, o: 0 }, geometry: "ellipse", w: 60, h: 40, fill: "FFC000" });
    await atoms(page).first().click();
    await atoms(page).nth(1).click({ modifiers: ["Shift"] });
    await expect(strip(page).getByText("2 shapes selected")).toBeVisible();
    await field(page, "fill", "Color").fill("00b050");
    await field(page, "fill", "Color").press("Enter");
    await expect.poll(async () => (await shapeAt(page, RICH.highlights)).fill?.color).toBe("00B050");
    expect((await shapeAt(page)).fill?.color).toBe("00B050");
    await undo(page);
    expect((await shapeAt(page, RICH.highlights)).fill?.color).toBe("FFC000");
    expect((await shapeAt(page)).fill?.color).toBe("4472C4");
  });

  test("sections open and close and stay as they were left while the panel is open", async ({ page }) => {
    await withShape(page);
    const effects = panel(page).getByRole("button", { name: "Effects", exact: true });
    await expect(effects).toHaveAttribute("aria-expanded", "false");
    await effects.click();
    await expect(effects).toHaveAttribute("aria-expanded", "true");
    await expect(field(page, "effects", "Shadow presets")).toBeVisible();
    // Another shape: the section stays as it was left.
    await run(page, { op: "insert_shape", at: { p: RICH.highlights, o: 0 }, geometry: "ellipse", w: 60, h: 40, fill: "FFC000" });
    await atoms(page).nth(1).click();
    await expect(effects).toHaveAttribute("aria-expanded", "true");
    await expect(field(page, "effects", "Shadow presets")).toBeVisible();
    await effects.click();
    await expect(field(page, "effects", "Shadow presets")).toBeHidden();
  });

  test("the strip's Fill and Outline pickers still write a bare colour, and the panel shows it", async ({ page }) => {
    await withShape(page);
    await openSection(page, "Fill");
    await strip(page).getByRole("button", { name: "Fill", exact: true }).click();
    const swatch = page.locator(".pop .swatch").nth(9);
    const picked = ((await swatch.getAttribute("title")) ?? "").slice(1).toUpperCase();
    await swatch.click();
    await expect.poll(async () => (await shapeAt(page)).fill?.color).toBe(picked);
    expect((await shapeAt(page)).fill?.alpha).toBeUndefined();
    await expect(field(page, "fill", "Color")).toHaveValue(picked);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain(`<a:solidFill><a:srgbClr val="${picked}"/></a:solidFill>`);
  });

  test("the panel in light and dark: every label and field reads at 4.5 to 1, and a look at it", async ({ page }, info) => {
    await page.setViewportSize({ width: 1400, height: 1000 });
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 120, h: 72, fill: "4472C4", line: { color: "2F528F", width_pt: 1 }, text: "Hello" });
    await atoms(page).first().click();
    await strip(page).getByRole("button", { name: "Format shape" }).click();
    for (const name of ["Line", "Effects", "Size and position", "Text box"]) await openSection(page, name);
    await field(page, "effects", "Shadow presets").selectOption("outer:45");
    for (const theme of ["light", "dark"]) {
      await page.evaluate((t) => document.querySelectorAll(".docxtor").forEach((e) => e.setAttribute("data-theme", t)), theme);
      const worst = await panel(page).evaluate((root) => {
        const parse = (c: string) => (c.match(/[\d.]+/g) ?? []).map(Number);
        const lum = ([r, g, b]: number[]) => {
          const f = (v: number) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
          return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
        };
        const backdrop = (el: Element): number[] => {
          for (let e: Element | null = el; e; e = e.parentElement) {
            const c = parse(getComputedStyle(e).backgroundColor);
            if (c.length >= 3 && (c.length === 3 || c[3] > 0.5)) return c;
          }
          return [255, 255, 255];
        };
        let min = 21;
        let which = "";
        for (const el of Array.from(root.querySelectorAll<HTMLElement>("label, .fs-label, .fs-hint, .fs-toggle, .fs-sub, .fs-unit, .fs-select, .fs-num, .fs-hex, .fs-radio, .fs-check, .btn"))) {
          if (!el.offsetParent || !(el.textContent || el instanceof HTMLInputElement)) continue;
          const fg = parse(getComputedStyle(el).color);
          const a = lum(fg);
          const b = lum(backdrop(el));
          const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
          if (ratio < min) {
            min = ratio;
            which = `${el.className} ${el.textContent?.slice(0, 20)}`;
          }
        }
        return { min, which };
      });
      expect(worst.min, `${theme}: ${worst.which}`).toBeGreaterThanOrEqual(4.5);
      await info.attach(`format-shape-${theme}`, { body: await page.screenshot(), contentType: "image/png" });
    }
  });
});

/** A 1 by 1 PNG. */
const PIXEL_PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
