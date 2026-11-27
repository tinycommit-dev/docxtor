// How shapes look: gradient, picture and pattern fills, effects, the text frame, outlines. Each test builds a
// document from Word's markup (see shape-looks-markup.ts), checks the SVG and the styles that were drawn, and takes
// a picture (into $DOCXTOR_SHOTS, else the test's own folder) to be looked at.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, test, type Locator, type Page } from "./helpers";
import { bodyPr, effectLst, FX, gradFill, lineXml, looksShape, pattFill, PATTERN_PRESETS, row, solidFill, threeD } from "./support/shape-looks-markup";

declare const process: { env: Record<string, string | undefined> };

const PT_PX = 96 / 72;
const atoms = (page: Page) => page.locator(".flow .atom.shape");
const run = (page: Page, ops: unknown) => inPage(page, (w, o) => w.fe.editor.exec(o), ops);

async function shot(page: Page, info: { outputPath(name: string): string }, name: string, clip?: { x: number; y: number; width: number; height: number }) {
  const dir = process.env.DOCXTOR_SHOTS;
  await page.screenshot({ path: dir ? `${dir}/${name}.png` : info.outputPath(`${name}.png`), clip });
}

/** The box around some shapes on the page, with room for effects that spill out. */
async function around(page: Page, pad = 24) {
  const boxes = await atoms(page).evaluateAll((els) => els.map((e) => e.getBoundingClientRect()).map((r) => ({ x: r.left, y: r.top, r: r.right, b: r.bottom })));
  const x = Math.max(0, Math.min(...boxes.map((b) => b.x)) - pad);
  const y = Math.max(0, Math.min(...boxes.map((b) => b.y)) - pad);
  return { x, y, width: Math.min(1280 - x, Math.max(...boxes.map((b) => b.r)) - x + pad), height: Math.max(...boxes.map((b) => b.b)) - y + pad };
}

const open = (page: Page, ...rows: ReturnType<typeof row>[]) => openBuilt(page, { sections: [{ paragraphs: ["Looks", ...rows.flatMap((r) => [r, "Between"])] }] });

test.describe("gradient fills", () => {
  test("every stop, its position and alpha, the angle, and path gradients are drawn", async ({ page }, info) => {
    await open(
      page,
      row(
        looksShape({ id: 31, w: 120, h: 60, sppr: gradFill([[0, "FF0000"], [35, "FFFF00", 0.8], [100, "0000FF"]], { lin: 45 }) }),
        looksShape({ id: 32, w: 120, h: 60, sppr: gradFill([[0, "FF0000"], [100, "0000FF"]], { lin: 45, scaled: true }) }),
        looksShape({ id: 33, w: 120, h: 60, sppr: gradFill([[0, "FFFFFF"], [100, "1F3864"]], { path: "circle" }), geometry: "ellipse" }),
        looksShape({ id: 34, w: 120, h: 60, sppr: gradFill([[0, "FFFFFF"], [60, "70AD47"], [100, "375623"]], { path: "rect" }) }),
      ),
      row(
        looksShape({ id: 35, w: 120, h: 60, sppr: solidFill("FF4F00", 0.4) }),
        looksShape({ id: 36, w: 120, h: 60, sppr: gradFill([[0, "201515"], [100, "FF4F00"]], { lin: 0 }), geometry: "roundRect" }),
        looksShape({ id: 37, w: 120, h: 60, sppr: gradFill([[0, "FFC000"], [50, "FFFFFF"], [100, "ED7D31"]], { lin: 90 }) }),
      ),
    );
    const all = atoms(page);
    await expect(all).toHaveCount(7);
    // Three stops: their offsets, colours and the alpha of the middle one.
    const g1 = all.nth(0).locator("linearGradient");
    const stops = await g1.locator("stop").evaluateAll((els) => els.map((s) => [s.getAttribute("offset"), s.getAttribute("stop-color"), s.getAttribute("stop-opacity")]));
    expect(stops).toEqual([["0", "#FF0000", "1"], ["0.35", "#FFFF00", "0.8"], ["1", "#0000FF", "1"]]);
    // 45 degrees on a 120 by 60 box, unscaled: the line through the centre reaches the far corners' projections.
    expect(await g1.getAttribute("gradientUnits")).toBe("userSpaceOnUse");
    const [x1, y1, x2, y2] = await Promise.all(["x1", "y1", "x2", "y2"].map(async (a) => Number(await g1.getAttribute(a))));
    const k = Math.SQRT1_2;
    const len = 120 * k + 60 * k;
    expect(x1).toBeCloseTo(60 - (k * len) / 2, 1);
    expect(y1).toBeCloseTo(30 - (k * len) / 2, 1);
    expect(x2).toBeCloseTo(60 + (k * len) / 2, 1);
    expect(y2).toBeCloseTo(30 + (k * len) / 2, 1);
    // Scaled: the angle is the unit square's, so it runs corner to corner of the box.
    const g2 = all.nth(1).locator("linearGradient");
    expect(await g2.getAttribute("gradientUnits")).toBeNull();
    expect([await g2.getAttribute("x1"), await g2.getAttribute("y1"), await g2.getAttribute("x2"), await g2.getAttribute("y2")]).toEqual(["0", "0", "1", "1"]);
    // A circle path gradient is a radial one centred on the focus; a rectangle one is a picture of rings.
    const radial = all.nth(2).locator("radialGradient");
    expect(await radial.getAttribute("cx")).toBe("0.5");
    expect(await radial.locator("stop").count()).toBe(2);
    const rect = all.nth(3).locator("defs pattern image");
    expect(await rect.getAttribute("href")).toMatch(/^data:image\/png/);
    // A translucent solid fill.
    await expect(all.nth(4).locator("svg.shape-svg > path")).toHaveAttribute("fill-opacity", "0.4");
    await shot(page, info, "gradients", await around(page));
  });

  test("a gradient that does not turn with its shape is turned back", async ({ page }) => {
    await open(page, row(looksShape({ id: 31, w: 100, h: 100, sppr: gradFill([[0, "FF0000"], [100, "0000FF"]], { lin: 0, rotWithShape: false }) })));
    await run(page, { op: "set_shape", at: await where(page), rot: 90 });
    const g = atoms(page).first().locator("linearGradient");
    // Turned by 90 degrees clockwise with the shape, the gradient was turned back by 90 degrees: it points up on the page, in the shape's frame.
    expect(Number(await g.getAttribute("y1"))).toBeGreaterThan(Number(await g.getAttribute("y2")));
    expect(Math.abs(Number(await g.getAttribute("x1")) - Number(await g.getAttribute("x2")))).toBeLessThan(0.01);
  });
});

/** The primitives of the filter a group uses: `[{ tag, ...attributes }]`. */
const primitives = (group: Locator) =>
  group.evaluate((g) => {
    const id = g.getAttribute("filter")!.slice(5, -1);
    const f = g.ownerSVGElement!.querySelector(`filter[id="${id}"]`)!;
    return Array.from(f.children).map((c) => ({ tag: c.tagName, ...Object.fromEntries(Array.from(c.attributes).map((a) => [a.name, a.value])) }));
  });

/** Where the first shape is in the model. */
const where = (page: Page) =>
  atoms(page)
    .first()
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));

test.describe("pattern fills", () => {
  test("every DrawingML pattern is drawn from its foreground and background", async ({ page }, info) => {
    const shapes = PATTERN_PRESETS.map((p, i) => looksShape({ id: 40 + i, w: 54, h: 40, sppr: pattFill(p, "1F3864", "FFE699") + lineXml("7F7F7F", 0.5) }));
    const rows: ReturnType<typeof row>[] = [];
    for (let i = 0; i < shapes.length; i += 9) rows.push(row(...shapes.slice(i, i + 9)));
    await open(page, ...rows);
    await expect(atoms(page)).toHaveCount(PATTERN_PRESETS.length);
    const ds = await atoms(page).evaluateAll((els) =>
      els.map((e) => {
        const p = e.querySelector("pattern")!;
        return { name: p.getAttribute("data-pattern"), bg: p.querySelector("rect")!.getAttribute("fill"), fg: p.querySelector("path")!.getAttribute("fill"), d: p.querySelector("path")!.getAttribute("d"), fill: e.querySelector("svg.shape-svg > path")!.getAttribute("fill"), pid: p.id };
      }),
    );
    expect(ds.map((d) => d.name)).toEqual(PATTERN_PRESETS);
    for (const d of ds) {
      expect(d.bg).toBe("#FFE699");
      expect(d.fg).toBe("#1F3864");
      expect(d.d!.length).toBeGreaterThan(4);
      expect(d.fill).toBe(`url(#${d.pid})`);
    }
    // Patterns look different from one another (a few are the same bitmap in Office too).
    expect(new Set(ds.map((d) => d.d)).size).toBeGreaterThanOrEqual(PATTERN_PRESETS.length - 4);
    // 50 percent covers half of the 64 pixels, 5 percent very few (the path is runs of pixels: `h` lengths).
    const cover = (name: string) => {
      const d = ds.find((x) => x.name === name)!.d!;
      return (d.match(/h(\d+)/g) ?? []).reduce((n, r) => n + Number(r.slice(1)), 0);
    };
    expect(cover("pct50")).toBe(32);
    expect(cover("pct5")).toBeLessThan(cover("pct50"));
    expect(cover("pct50")).toBeLessThan(cover("pct90"));
    await shot(page, info, "patterns", await around(page, 12));
  });
});

/** A picture the browser can draw: four coloured quarters, as a PNG added to the document. */
async function addPicture(page: Page): Promise<string> {
  return page.evaluate(async () => {
    const c = document.createElement("canvas");
    c.width = 40;
    c.height = 20;
    const x = c.getContext("2d")!;
    x.fillStyle = "#e4572e";
    x.fillRect(0, 0, 20, 10);
    x.fillStyle = "#17bebb";
    x.fillRect(20, 0, 20, 10);
    x.fillStyle = "#ffc914";
    x.fillRect(0, 10, 20, 10);
    x.fillStyle = "#2e282a";
    x.fillRect(20, 10, 20, 10);
    const blob: Blob = await new Promise((r) => c.toBlob((b) => r(b!), "image/png"));
    const bytes = new Uint8Array(await blob.arrayBuffer());
    const w = (window as unknown as { docxtor: { fe: { editor: { engine: { addMedia(ext: string, b: Uint8Array): string } } } } }).docxtor;
    return w.fe.editor.engine.addMedia("png", bytes);
  });
}

test.describe("picture fills", () => {
  test("a picture fills the shape stretched or cropped, or tiles with its scale, offset and flips", async ({ page }, info) => {
    await open(
      page,
      row(
        looksShape({ id: 31, w: 120, h: 60, sppr: solidFill("FFFFFF") }),
        looksShape({ id: 32, w: 120, h: 60, sppr: solidFill("FFFFFF") }),
        looksShape({ id: 33, w: 120, h: 60, sppr: solidFill("FFFFFF") }),
        looksShape({ id: 34, w: 120, h: 60, sppr: solidFill("FFFFFF") }),
      ),
    );
    const media = await addPicture(page);
    const at = async (i: number) => atoms(page).nth(i).evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
    await run(page, [
      { op: "set_shape", at: await at(0), fill: { kind: "picture", media } },
      { op: "set_shape", at: await at(1), fill: { kind: "picture", media, crop: { l: 50, t: 0, r: 0, b: 0 }, alpha: 0.6 } },
      { op: "set_shape", at: await at(2), fill: { kind: "picture", media, tile: { sx: 100, sy: 100, tx: 0, ty: 0 } } },
      { op: "set_shape", at: await at(3), fill: { kind: "picture", media, tile: { sx: 50, sy: 50, tx: 3, ty: 4, align: "ctr", flip: "xy" } } },
    ]);
    const all = atoms(page);
    // Stretched: one image as big as the box, in a pattern as big as the box.
    const stretched = all.nth(0).locator("defs pattern");
    expect(await stretched.getAttribute("width")).toBe("120");
    const img = stretched.locator("image");
    expect(await img.getAttribute("href")).toMatch(/^blob:/);
    expect([await img.getAttribute("x"), await img.getAttribute("width"), await img.getAttribute("height")]).toEqual(["0", "120", "60"]);
    // Cropped: the left half of the picture is out of sight, so the rest is stretched over the box.
    const cropped = all.nth(1).locator("defs pattern image");
    expect([await cropped.getAttribute("x"), await cropped.getAttribute("width")]).toEqual(["-120", "240"]);
    await expect(all.nth(1).locator("svg.shape-svg > path")).toHaveAttribute("fill-opacity", "0.6");
    // Tiled at the picture's own size (40 by 20 pixels at 96 dpi is 30 by 15 points) once it has loaded.
    const tile = all.nth(2).locator("defs pattern");
    await expect.poll(async () => Number(await tile.getAttribute("width"))).toBeCloseTo(30, 1);
    expect(Number(await tile.getAttribute("height"))).toBeCloseTo(15, 1);
    // Scaled to half, flipped both ways: a tile of two by two copies of 15 by 7.5 points, from the centre.
    const flipped = all.nth(3).locator("defs pattern");
    await expect.poll(async () => Number(await flipped.getAttribute("width"))).toBeCloseTo(30, 1);
    expect(await flipped.locator("image").count()).toBe(4);
    expect(Number(await flipped.getAttribute("x"))).toBeCloseTo((120 - 15) / 2 + 3, 1);
    // (An image in a pattern is not drawn where it is defined, and Firefox gives it no box: its own width says it is there.)
    await expect.poll(() => all.nth(2).locator("defs pattern image").evaluate((i) => Number(i.getAttribute("width")) > 0 && !!(i.getAttribute("href") ?? i.getAttribute("xlink:href")))).toBe(true);
    await page.waitForTimeout(200);
    await shot(page, info, "pictures", await around(page));
    // The fill is read back from the document: a picture, with its part name.
    const fill = (await inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: unknown): { fill: { kind: string; src: string } } }).shapeAt(a), await at(0))).fill;
    expect(fill.kind).toBe("picture");
    expect(fill.src).toBe(media);
  });
});

test.describe("effects", () => {
  test("shadows, glow, soft edges, reflection, bevel and extrusion are drawn in points", async ({ page }, info) => {
    const base = solidFill("4472C4") + lineXml("2F528F", 1);
    await open(
      page,
      row(
        looksShape({ id: 31, w: 90, h: 50, sppr: base + effectLst(FX.shadow) }),
        looksShape({ id: 32, w: 90, h: 50, sppr: base + effectLst(FX.inner) }),
        looksShape({ id: 33, w: 90, h: 50, sppr: base + effectLst(FX.glow) }),
        looksShape({ id: 34, w: 90, h: 50, sppr: solidFill("4472C4") + effectLst(FX.soft) }),
      ),
      row(
        looksShape({ id: 35, w: 90, h: 50, sppr: base + effectLst(FX.reflection) }),
        looksShape({ id: 36, w: 90, h: 50, sppr: base + threeD({ bevel: "circle", bevelPt: 6 }), geometry: "roundRect" }),
        looksShape({ id: 37, w: 90, h: 50, sppr: base + threeD({ camera: "obliqueTopRight", depthPt: 14 }) }),
        looksShape({ id: 38, w: 90, h: 50, sppr: base + effectLst(FX.shadow, FX.glow) + threeD({ bevel: "hardEdge", bevelPt: 4 }) }),
      ),
    );
    const all = atoms(page);
    await expect(all).toHaveCount(8);
    // The effects are in points of the shape's own box: a blur of 5 points is a deviation of 2.5 in the SVG.
    const blur = async (i: number, group: string) => Number((await primitives(all.nth(i).locator(group))).find((p) => p.tag === "feGaussianBlur")!.stdDeviation);
    expect(await blur(0, "g.fx-shadow")).toBeCloseTo(2.5, 2);
    // The shadow moves 4 points down and to the right (45 degrees), seen in the use element's transform.
    const t = await all.nth(0).locator("g.fx-shadow use").getAttribute("transform");
    const m = /translate\(([\d.-]+) ([\d.-]+)\)/.exec(t!)!;
    expect(Number(m[1])).toBeCloseTo(4 * Math.SQRT1_2, 1);
    expect(Number(m[2])).toBeCloseTo(4 * Math.SQRT1_2, 1);
    // The shadow is a copy of the shape's own paths, drawn behind it.
    const body = all.nth(0).locator("g.shape-body");
    const bodyId = await body.getAttribute("id");
    expect(await all.nth(0).locator("g.fx-shadow use").getAttribute("href")).toBe(`#${bodyId}`);
    expect(await all.nth(0).locator("svg.shape-svg").evaluate((s) => Array.from(s.children).map((c) => c.getAttribute("class") ?? c.tagName))).toEqual(["defs", "fx-shadow", "shape-body"]);
    // Inner shadow filters the shape; glow dilates and blurs a copy behind it.
    expect(await all.nth(1).locator("svg.shape-svg > g[filter] > g.shape-body").count()).toBe(1);
    const inner = await primitives(all.nth(1).locator("svg.shape-svg > g[filter]"));
    expect(inner.map((p) => p.tag)).toContain("feOffset");
    expect(Number(inner.find((p) => p.tag === "feOffset")!.dx)).toBeCloseTo(4 * Math.SQRT1_2, 1);
    expect(Number(inner.find((p) => p.tag === "feGaussianBlur")!.stdDeviation)).toBeCloseTo(3, 2);
    const glow = await primitives(all.nth(2).locator("g.fx-glow"));
    expect(glow.find((p) => p.tag === "feMorphology")!.operator).toBe("dilate");
    expect(glow.find((p) => p.tag === "feFlood")!["flood-color"]).toBe("#FFC000");
    expect(glow.find((p) => p.tag === "feFlood")!["flood-opacity"]).toBe("0.45");
    const soft = await primitives(all.nth(3).locator("svg.shape-svg > g[filter]"));
    expect(soft.find((p) => p.tag === "feMorphology")!.operator).toBe("erode");
    // The reflection is a flipped copy under the shape, faded by a mask.
    const refl = all.nth(4).locator("g.fx-reflection");
    expect(await refl.locator("use").count()).toBe(1);
    expect(await refl.locator("g[transform]").getAttribute("transform")).toContain("scale(1 -1)");
    expect(await all.nth(4).locator("defs mask").count()).toBe(1);
    const fade = await all.nth(4).locator("defs linearGradient stop").evaluateAll((els) => els.map((e) => e.getAttribute("stop-opacity")));
    expect(fade).toEqual(["0.5", "0.003"]);
    // A bevel is lit relief; an extrusion is stacked copies; they combine.
    expect((await primitives(all.nth(5).locator("svg.shape-svg > g[filter]"))).map((p) => p.tag)).toContain("feSpecularLighting");
    expect(await all.nth(6).locator("g.fx-extrusion use").count()).toBeGreaterThan(10);
    expect(await all.nth(7).locator("g.fx-shadow").count()).toBe(1);
    expect(await all.nth(7).locator("g.fx-glow").count()).toBe(1);
    // Effects do not change the box the shape takes in the text.
    const size = await all.nth(0).evaluate((e) => ({ w: parseFloat(getComputedStyle(e).width), h: parseFloat(getComputedStyle(e).height) }));
    expect(size.w).toBeCloseTo(90 * PT_PX, 0);
    await shot(page, info, "effects", await around(page, 30));
  });

  test("a shadow that keeps its direction on the page is turned back when the shape turns", async ({ page }) => {
    await open(page, row(looksShape({ id: 31, w: 90, h: 50, sppr: solidFill("4472C4") + effectLst(FX.shadow) })));
    const at = await where(page);
    const move = async () => {
      const t = await atoms(page).first().locator("g.fx-shadow use").getAttribute("transform");
      const m = /translate\(([\d.-]+) ([\d.-]+)\)/.exec(t!)!;
      return [Number(m[1]), Number(m[2])];
    };
    // Unturned it falls down and to the right.
    let [dx, dy] = await move();
    expect(dx).toBeGreaterThan(1);
    expect(dy).toBeGreaterThan(1);
    // Turned a quarter, the shape's own frame is turned back so the shadow still falls down and to the right on the page.
    await run(page, { op: "set_shape", at, rot: 90 });
    [dx, dy] = await move();
    expect(dx).toBeGreaterThan(1);
    expect(dy).toBeLessThan(-1);
    expect(await atoms(page).first().evaluate((e) => (e as HTMLElement).style.transform)).toBe("rotate(90deg)");
  });

  test("the blur stays the same number of points when the shape is resized, and the effects go with set_shape", async ({ page }) => {
    await open(page, row(looksShape({ id: 31, w: 90, h: 50, sppr: solidFill("4472C4") + effectLst(FX.shadow) })));
    const at = await where(page);
    await run(page, { op: "set_shape", at, w: 240, h: 120 });
    const shape = atoms(page).first();
    expect((await primitives(shape.locator("g.fx-shadow"))).find((p) => p.tag === "feGaussianBlur")!.stdDeviation).toBe("2.5");
    expect(await shape.locator("svg.shape-svg").getAttribute("viewBox")).toBe("0 0 240 120");
    // Change one effect and add another; undo takes the glow away and then the whole change.
    await run(page, { op: "set_shape", at, effects: { shadow: { blur_pt: 12 }, glow: { radius_pt: 8, color: "FF4F00", alpha: 0.5 } } });
    await expect(shape.locator("g.fx-glow")).toHaveCount(1);
    expect((await primitives(shape.locator("g.fx-shadow"))).find((p) => p.tag === "feGaussianBlur")!.stdDeviation).toBe("6");
    await run(page, { op: "set_shape", at, effects: "none" });
    await expect(shape.locator("g.fx-shadow, g.fx-glow")).toHaveCount(0);
    expect(await shape.locator("svg.shape-svg > path").count()).toBe(1);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(shape.locator("g.fx-glow")).toHaveCount(1);
  });
});

test.describe("the text frame", () => {
  test("columns, vertical text, anchoring, insets, shrinking and wrapping are laid out", async ({ page }, info) => {
    const long = "Stars are not the only things in the sky that give light, but they are the ones we trust the most.";
    const frame = (b: string, id: number, text = [long], w = 150, h = 90) => looksShape({ id, w, h, sppr: solidFill("FFF2CC") + lineXml("BF9000", 0.75), body: b, text });
    await open(
      page,
      row(
        frame(bodyPr({ cols: 2, gapPt: 12, anchor: "t" }), 31, [long, long]),
        frame(bodyPr({ vert: "vert270", anchor: "ctr", anchorCtr: true }), 32, ["Rotated 270"], 60, 150),
        frame(bodyPr({ vert: "vert" }), 33, ["Rotated 90"], 60, 150),
        frame(bodyPr({ vert: "wordArtVert" }), 34, ["Stacked"], 60, 150),
        frame(bodyPr({ vert: "eaVert" }), 35, ["East Asian"], 60, 150),
      ),
      row(
        frame(bodyPr({ anchor: "b", anchorCtr: true, ins: [20, 10, 20, 10] }), 36, ["Bottom, centred"]),
        frame(bodyPr({ wrap: "none", anchor: "ctr" }), 37, ["No wrapping at all in this line of text"]),
        frame(bodyPr({ fit: '<a:normAutofit fontScale="70000" lnSpcReduction="10000"/>' }), 38, [long, long]),
        frame(bodyPr({ fit: "<a:spAutoFit/>" }), 39, [long, long, long, long, long]),
      ),
    );
    const all = atoms(page);
    const text = (i: number) => all.nth(i).locator(".shape-text");
    const css = (i: number, prop: string) => text(i).evaluate((e, p) => getComputedStyle(e).getPropertyValue(p), prop);
    // Columns.
    expect(await css(0, "column-count")).toBe("2");
    expect(parseFloat(await css(0, "column-gap"))).toBeCloseTo(12 * PT_PX, 1);
    const colRects = await text(0).locator(".fp").evaluateAll((ps) => ps.map((p) => p.getBoundingClientRect().left));
    expect(Math.abs(colRects[1] - colRects[0])).toBeGreaterThan(40);
    // Vertical text.
    expect(["sideways-lr", "vertical-rl"]).toContain(await css(1, "writing-mode"));
    expect(await css(2, "writing-mode")).toBe("vertical-rl");
    expect(await css(2, "text-orientation")).toBe("sideways");
    expect(await css(3, "text-orientation")).toBe("upright");
    expect(await css(4, "text-orientation")).toBe("mixed");
    // The rotated 270 text reads upwards: its line is taller than wide.
    const lineBox = await text(1).locator(".fp").first().evaluate((p) => {
      const r = p.getBoundingClientRect();
      return { w: r.width, h: r.height };
    });
    expect(lineBox.h).toBeGreaterThan(lineBox.w);
    // Anchor at the bottom, text block centred, insets all round.
    expect(await css(5, "justify-content")).toBe("flex-end");
    expect(await css(5, "align-items")).toBe("center");
    const inset = await text(5).evaluate((e) => ({ l: parseFloat((e as HTMLElement).style.left), t: parseFloat((e as HTMLElement).style.top) }));
    expect(inset.l).toBeCloseTo(20 * PT_PX, 1);
    expect(inset.t).toBeCloseTo(10 * PT_PX, 1);
    // No wrapping keeps the text on one line.
    await expect(text(6)).toHaveClass(/nowrap/);
    expect(await text(6).locator(".fp").first().evaluate((p) => getComputedStyle(p).whiteSpace)).toBe("pre");
    // Text that shrinks to fit is scaled to 70 percent into a box that is 1/0.7 as large.
    const tr = await css(7, "transform");
    expect(tr).toMatch(/^matrix\(0\.7, 0, 0, 0\.7/);
    // Lines of that shape are 10 percent closer than those of the one beside it.
    const lh = (i: number) => text(i).locator(".fp").first().evaluate((p) => parseFloat(getComputedStyle(p).lineHeight));
    expect((await lh(7)) / (await lh(8))).toBeCloseTo(0.9, 1);
    // A shape that grows with its text is taller than its 90 points.
    const h = await all.nth(8).evaluate((e) => parseFloat(getComputedStyle(e).height));
    expect(h).toBeGreaterThan(90 * PT_PX + 10);
    await shot(page, info, "text-frame", await around(page, 12));
  });

  test("text keeps its way up under flips and stays upright when asked", async ({ page }) => {
    await open(
      page,
      row(
        looksShape({ id: 31, w: 100, h: 50, sppr: solidFill("FFF2CC"), text: ["Flipped"] }),
        looksShape({ id: 32, w: 100, h: 50, sppr: solidFill("FFF2CC"), text: ["Upright"], body: bodyPr({ upright: true }) }),
      ),
    );
    const at0 = await where(page);
    const second = await atoms(page).nth(1).evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
    await run(page, [
      { op: "set_shape", at: at0, flip_h: true },
      { op: "set_shape", at: second, rot: 30 },
    ]);
    // Flipped sideways: the text is not mirrored (the SVG alone is).
    await expect(atoms(page).first().locator(".shape-text")).toHaveCSS("transform", "none");
    await expect(atoms(page).first().locator("svg.shape-svg")).toHaveCSS("transform", "matrix(-1, 0, 0, 1, 0, 0)");
    // The second shape is turned 30 degrees; its upright text is turned back.
    await expect(atoms(page).nth(1).locator(".shape-text")).toHaveCSS("transform", /^matrix\(0\.866/);
    // Flipped upside down, Word turns the text over with the shape.
    await run(page, { op: "set_shape", at: at0, flip_v: true });
    await expect(atoms(page).first().locator(".shape-text")).toHaveCSS("transform", "matrix(-1, 0, 0, -1, 0, 0)");
  });
});

test.describe("outlines", () => {
  test("compound lines, opacity, miter limits and arrowhead sizes are drawn", async ({ page }, info) => {
    const l = (attrs: string, kids = "") => solidFill("FFFFFF") + lineXml("201515", 6, attrs, kids);
    await open(
      page,
      row(
        looksShape({ id: 31, w: 90, h: 50, sppr: l(' cmpd="dbl"') }),
        looksShape({ id: 32, w: 90, h: 50, sppr: l(' cmpd="tri"') }),
        looksShape({ id: 33, w: 90, h: 50, sppr: solidFill("FFFFFF") + '<a:ln w="76200"><a:solidFill><a:srgbClr val="FF4F00"><a:alpha val="40000"/></a:srgbClr></a:solidFill><a:miter lim="200000"/></a:ln>' }),
        looksShape({ id: 34, w: 90, h: 50, geometry: "line", sppr: lineXml("201515", 3, "", '<a:headEnd type="triangle" w="sm" len="sm"/><a:tailEnd type="triangle" w="lg" len="lg"/>') }),
        looksShape({ id: 35, w: 90, h: 50, geometry: "line", sppr: lineXml("201515", 3, "", '<a:tailEnd type="triangle"/>') }),
      ),
    );
    const all = atoms(page);
    // A compound line is drawn through a mask that cuts the gaps; the mask is stroked in the line's own width.
    for (const i of [0, 1]) {
      const masked = all.nth(i).locator("svg.shape-svg > path[mask]");
      await expect(masked).toHaveCount(1);
      const id = (await masked.getAttribute("mask"))!.slice(5, -1);
      const strokes = all.nth(i).locator(`mask[id="${id}"] path`);
      expect(await strokes.count()).toBe(i === 0 ? 2 : 3);
      expect(Number(await strokes.first().getAttribute("stroke-width"))).toBeCloseTo(6 * PT_PX, 1);
      // The fill is drawn apart from the line, so the mask does not cut it.
      expect(await all.nth(i).locator("svg.shape-svg > path[mask]").getAttribute("fill")).toBe("none");
    }
    // Translucent line with a miter limit of 2.
    const third = all.nth(2).locator("svg.shape-svg > path").first();
    await expect(third).toHaveAttribute("stroke-opacity", "0.4");
    await expect(third).toHaveAttribute("stroke-miterlimit", "2");
    // Arrowheads: large is larger than medium, small smaller.
    const marker = (i: number, n: number) => all.nth(i).locator("marker").nth(n);
    const w = async (i: number, n: number) => Number(await marker(i, n).getAttribute("markerWidth"));
    expect(await w(3, 0)).toBeLessThan(await w(4, 0));
    expect(await w(3, 1)).toBeGreaterThan(await w(4, 0));
    expect(Number(await marker(3, 1).getAttribute("markerHeight"))).toBeGreaterThan(Number(await marker(4, 0).getAttribute("markerHeight")));
    await shot(page, info, "lines", await around(page, 12));
  });
});

test.describe("looks of members of a group", () => {
  test("a flipped group mirrors where its members are, and a picture member flips", async ({ page }) => {
    const member = (id: number, name: string, x: number, fill: string) =>
      `<wps:wsp><wps:cNvPr id="${id}" name="${name}"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="${x * 12700}" y="0"/><a:ext cx="${40 * 12700}" cy="${40 * 12700}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>${fill}</wps:spPr><wps:bodyPr/></wps:wsp>`;
    const group = `<w:r><mc:AlternateContent><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="${100 * 12700}" cy="${40 * 12700}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="50" name="Group 50"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm flipH="1"><a:off x="0" y="0"/><a:ext cx="${100 * 12700}" cy="${40 * 12700}"/><a:chOff x="0" y="0"/><a:chExt cx="${100 * 12700}" cy="${40 * 12700}"/></a:xfrm></wpg:grpSpPr>${member(51, "Left", 0, solidFill("FF0000"))}${member(52, "Right", 60, gradFill([[0, "00FF00"], [100, "004400"]], { lin: 0 }))}</wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict><v:rect style="width:100pt;height:40pt"/></w:pict></mc:Fallback></mc:AlternateContent></w:r>`;
    await open(page, { raw: group });
    const kids = atoms(page).first().locator(".shape-child");
    await expect(kids).toHaveCount(2);
    // The group is flipped sideways: what was on the left is on the right, and its drawing is flipped too.
    expect(parseFloat(await kids.nth(0).evaluate((e) => (e as HTMLElement).style.left))).toBeCloseTo(60 * PT_PX, 0);
    expect(parseFloat(await kids.nth(1).evaluate((e) => (e as HTMLElement).style.left))).toBeCloseTo(0, 0);
    expect(await kids.nth(1).locator("svg.shape-svg").evaluate((s) => (s as SVGElement).style.transform)).toBe("scale(-1, 1)");
  });
});

test.describe("shapes from older files and made by ops", () => {
  test("VML fills, shadows and the text frame are drawn", async ({ page }, info) => {
    const vml =
      '<w:r><w:pict><v:rect id="Old box" o:spid="_x0000_s1026" style="width:120pt;height:80pt" fillcolor="#ff0000" strokecolor="#222222">' +
      '<v:fill type="gradient" color2="#0000ff" colors="0 #ff0000;.5 #ffff00;1 #0000ff" angle="180"/><v:shadow on="t" color="#000000" opacity=".5" offset="4pt,4pt"/>' +
      '<v:textbox style="layout-flow:vertical;mso-layout-flow-alt:bottom-to-top" inset="2pt,2pt,2pt,2pt"><w:txbxContent><w:p><w:r><w:t>Old frame</w:t></w:r></w:p></w:txbxContent></v:textbox></v:rect></w:pict></w:r>';
    await open(page, { raw: vml });
    const shape = atoms(page).first();
    const stops = await shape.locator("defs linearGradient stop").evaluateAll((els) => els.map((e) => [e.getAttribute("offset"), e.getAttribute("stop-color")]));
    expect(stops).toEqual([["0", "#FF0000"], ["0.5", "#FFFF00"], ["1", "#0000FF"]]);
    await expect(shape.locator("g.fx-shadow")).toHaveCount(1);
    expect(["sideways-lr", "vertical-rl"]).toContain(await shape.locator(".shape-text").evaluate((e) => getComputedStyle(e).writingMode));
    await shot(page, info, "vml-shape", await around(page, 16));
  });

  test("set_shape draws a gradient, an effect and a name, and undo takes them back", async ({ page }) => {
    await open(page, row(looksShape({ id: 31, w: 100, h: 60, sppr: solidFill("4472C4") })));
    const at = await where(page);
    await run(page, { op: "set_shape", at, fill: { kind: "gradient", angle: 0, stops: [{ pos: 0, color: "FF0000" }, { pos: 100, color: "0000FF", alpha: 0.5 }] }, effects: { shadow: { blur_pt: 8 } }, name: "Banner" });
    const shape = atoms(page).first();
    await expect(shape.locator("defs linearGradient stop")).toHaveCount(2);
    await expect(shape.locator("g.fx-shadow")).toHaveCount(1);
    expect(await shape.getAttribute("data-name")).toBe("Banner");
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(shape.locator("defs linearGradient")).toHaveCount(0);
    await expect(shape.locator("g.fx-shadow")).toHaveCount(0);
    await expect(shape.locator("svg.shape-svg > path").first()).toHaveAttribute("fill", "#4472C4");
  });
});

test("a shape with effects prints with the pages, and every reference finds an element of its own copy", async ({ page }) => {
  await open(page, row(looksShape({ id: 31, w: 100, h: 50, sppr: gradFill([[0, "FF0000"], [100, "0000FF"]], { lin: 45 }) + effectLst(FX.shadow, FX.glow, FX.soft) })));
  await page.evaluate(() => {
    window.print = () => undefined;
  });
  await page.locator(".flow .fp").first().click();
  await page.keyboard.press("ControlOrMeta+P");
  const copy = page.locator(".docxtor-print .print-page").first();
  await expect(copy.locator(".atom.shape svg.shape-svg")).toHaveCount(1);
  const dupes = await page.evaluate(() => {
    const ids = Array.from(document.querySelectorAll("[id^='fa']"), (e) => e.id);
    return ids.length - new Set(ids).size;
  });
  expect(dupes).toBe(0);
  // Each use, filter, mask and fill points at something that exists in the document.
  const broken = await copy.locator(".atom.shape svg.shape-svg").evaluate((svg) => {
    const refs: string[] = [];
    for (const e of Array.from(svg.querySelectorAll("*"))) {
      for (const a of ["fill", "stroke", "filter", "mask", "marker-start", "marker-end"]) {
        const m = e.getAttribute(a)?.match(/^url\(#([^)]+)\)$/);
        if (m) refs.push(m[1]);
      }
      const h = e.getAttribute("href");
      if (h?.startsWith("#")) refs.push(h.slice(1));
    }
    return refs.filter((id) => !svg.ownerDocument.getElementById(id) || !svg.querySelector(`[id="${id}"]`));
  });
  expect(broken).toEqual([]);
});
