// Arranging objects: the stacking order, shapes behind the text, members of groups, grouping, shapes in a header,
// shapes placed from the page, and drawing by hand.
import { floatShape } from "./support/arrange-markup";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, expect, inPage, test, type Locator, type Page } from "./helpers";
import { GROUP } from "./support/shape-markup";

const MOD = "ControlOrMeta";
const PT_PX = 96 / 72;

interface Shape {
  geometry: string;
  kind: string;
  w: number;
  h: number;
  x?: number;
  y?: number;
  z?: number;
  wrap: string;
  fill?: { color: string };
  custom?: { paths: unknown[] };
  children?: Shape[];
  childX?: number;
  childY?: number;
}

const shapeAt = (page: Page, p: number, o: number) =>
  inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape | null>;
const atoms = (page: Page) => page.locator(".flow .atom.shape");
const frame = (page: Page) => page.locator(".obj-frame").first();
const strip = (page: Page) => page.locator(".shape-strip");
const zoomOf = (page: Page) => inPage(page, (w) => w.fe.editor.zoom);
const zIndex = (loc: Locator) => loc.evaluate((e) => Number(getComputedStyle(e).zIndex));
const where = (page: Page, n = 0) =>
  atoms(page)
    .nth(n)
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));

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
      const name = new TextDecoder().decode(bytes.subarray(p + 46, p + 46 + nlen));
      p += 46 + nlen + xlen + clen;
      if (name !== want) continue;
      const dataStart = off + 30 + dv.getUint16(off + 26, true) + dv.getUint16(off + 28, true);
      const data = bytes.subarray(dataStart, dataStart + csize);
      if (method === 0) return new TextDecoder().decode(data);
      const stream = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
      return await new Response(stream).text();
    }
    return "";
  }, name);
}

const heights = (xml: string) => [...xml.matchAll(/relativeHeight="(\d+)"/g)].map((m) => Number(m[1]));

/** Which of the shapes is on top at a point of the screen (its index among the shapes), or -1. */
const topShapeAt = (page: Page, x: number, y: number) =>
  page.evaluate(
    ([px, py]) => {
      const all = Array.from(document.querySelectorAll(".flow .atom.shape"));
      for (const el of document.elementsFromPoint(px, py)) {
        const atom = el.closest(".atom.shape");
        if (atom) return all.indexOf(atom);
      }
      return -1;
    },
    [x, y],
  );

/** A point on the first sheet, in points from its top-left corner, as a point on screen. */
async function onSheet(page: Page, x: number, y: number) {
  const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
  const z = await zoomOf(page);
  return { x: sheet.x + x * PT_PX * z, y: sheet.y + y * PT_PX * z };
}

async function arrange(page: Page, label: string) {
  await strip(page).getByRole("button", { name: "Arrange", exact: true }).click();
  await page.locator(".arrange-menu").getByRole("menuitem", { name: label, exact: true }).click();
}

test.describe("stacking order", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "Some text on the page",
            {
              raw:
                floatShape({ id: 11, name: "Red", x: 100, relH: "page", y: 200, relV: "page", z: 251658240, fill: "FF0000" }) +
                floatShape({ id: 12, name: "Blue", x: 160, relH: "page", y: 230, relV: "page", z: 251659264, fill: "0000FF" }),
            },
          ],
        },
      ],
    });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
  });

  test("bring to front and send to back change what is on top and the saved relativeHeight", async ({ page }) => {
    const red = atoms(page).nth(0);
    const blue = atoms(page).nth(1);
    expect(await zIndex(blue)).toBeGreaterThan(await zIndex(red));
    // Where the two overlap (the red one is 100 by 60 points; the blue one starts 60 across and 30 down).
    const redBox = (await red.boundingBox())!;
    const z = await zoomOf(page);
    const overlap = { x: redBox.x + 80 * PT_PX * z, y: redBox.y + 45 * PT_PX * z };
    expect(await topShapeAt(page, overlap.x, overlap.y)).toBe(1);
    // Select the red shape by the corner nothing covers, and bring it to the front from its strip.
    await page.mouse.click(redBox.x + 6, redBox.y + 6);
    await expect(strip(page)).toBeVisible();
    await arrange(page, "Bring to front");
    expect(await zIndex(red)).toBeGreaterThan(await zIndex(blue));
    expect(await topShapeAt(page, overlap.x, overlap.y)).toBe(0);
    const xml = await savedPart(page, "word/document.xml");
    const [h1, h2] = heights(xml);
    expect(h1).toBeGreaterThan(h2);
    // The fallback says the same.
    const zs = [...xml.matchAll(/z-index:(-?\d+)/g)].map((m) => Number(m[1]));
    expect(zs[0]).toBeGreaterThan(zs[1]);
    // Still selected, so the next step is one click away: send it to the back from the right-click menu.
    await expect(frame(page)).toBeVisible();
    await red.click({ button: "right", position: { x: 6, y: 6 } });
    await contextItem(page, "Send to back").click();
    expect(await zIndex(red)).toBeLessThan(await zIndex(blue));
    expect(await topShapeAt(page, overlap.x, overlap.y)).toBe(1);
    const [g1, g2] = heights(await savedPart(page, "word/document.xml"));
    expect(g1).toBeLessThan(g2);
    // The menu bar has them too: Format, Arrange.
    await chooseMenu(page, "Format", "Arrange", "Bring forward");
    expect(await zIndex(red)).toBeGreaterThan(await zIndex(blue));
    // One undo for each step.
    await page.keyboard.press(`${MOD}+Z`);
    expect(await zIndex(red)).toBeLessThan(await zIndex(blue));
    await page.keyboard.press(`${MOD}+Z`);
    expect(await zIndex(red)).toBeGreaterThan(await zIndex(blue));
    expect(await topShapeAt(page, overlap.x, overlap.y)).toBe(0);
    await page.keyboard.press(`${MOD}+Z`);
    expect(await zIndex(red)).toBeLessThan(await zIndex(blue));
    expect(await topShapeAt(page, overlap.x, overlap.y)).toBe(1);
  });

  test("the commands are greyed out for a shape in line with the text", async ({ page }) => {
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_shape", at: { p: 1, o: 0 }, geometry: "ellipse", w: 60, h: 40, floating: false }));
    const inline = atoms(page).first();
    await inline.click();
    await strip(page).getByRole("button", { name: "Arrange", exact: true }).click();
    await expect(page.locator(".arrange-menu").getByRole("menuitem", { name: "Bring to front", exact: true })).toBeDisabled();
    await expect(page.locator(".arrange-menu").getByRole("menuitem", { name: "Behind text", exact: true })).toBeEnabled();
  });
});

test.describe("charts and pictures", () => {
  test("a chart is ordered with the Arrange button of its strip, also when two have the same value", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Charts on the page", "Second paragraph"] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    const chart = (n: number) => page.locator(".flow .atom.img.chart").nth(n);
    for (const [i, kind] of ["column", "line"].entries()) {
      await inPage(
        page,
        (w, a) => {
          const e = w.fe.editor as unknown as { exec(ops: unknown): unknown };
          e.exec({ op: "insert_chart", at: { p: 1, o: a.i }, kind: a.kind, categories: ["A", "B"], series: [{ name: "S", values: [1, 2] }], width: 200, height: 120 });
          e.exec({ op: "set_image", at: { p: 1, o: a.i }, wrap: "front" });
        },
        { i, kind },
      );
    }
    await expect(page.locator(".flow .atom.img.chart")).toHaveCount(2);
    const same = heights(await savedPart(page, "word/document.xml"));
    expect(same[0]).toBe(same[1]);
    // The second one is on top, as the first of two with the same value would be.
    expect(await zIndex(chart(1))).toBeGreaterThanOrEqual(await zIndex(chart(0)));
    await inPage(page, (w) => (w.fe.extensions as unknown as { execute(id: string, ...args: unknown[]): Promise<unknown> }).execute("image.select", 1, 0));
    await expect(page.locator(".img-strip")).toBeVisible();
    await page.locator(".img-strip").getByRole("button", { name: "Arrange", exact: true }).click();
    await page.locator(".arrange-menu").getByRole("menuitem", { name: "Bring to front", exact: true }).click();
    expect(await zIndex(chart(0))).toBeGreaterThan(await zIndex(chart(1)));
    const [a, b] = heights(await savedPart(page, "word/document.xml"));
    expect(a).toBeGreaterThan(b);
    // The menu bar's Arrange acts on the selected chart too.
    await chooseMenu(page, "Format", "Arrange", "Send to back");
    expect(await zIndex(chart(0))).toBeLessThan(await zIndex(chart(1)));
  });

  test("a chart behind the text is selected by a click where there is no text", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["A short line", "Second paragraph"] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    await inPage(page, (w) => {
      const e = w.fe.editor as unknown as { exec(ops: unknown): unknown };
      e.exec({ op: "insert_chart", at: { p: 1, o: 0 }, kind: "column", categories: ["A", "B"], series: [{ name: "S", values: [1, 2] }], width: 300, height: 200 });
      e.exec({ op: "set_image", at: { p: 1, o: 0 }, wrap: "behind" });
    });
    const chart = page.locator(".flow .atom.img.chart");
    await expect(chart).toHaveClass(/wrap-behind/);
    const box = (await chart.boundingBox())!;
    // The line of text is short; the chart is as wide as the text area, so the right of the line is the chart's.
    await page.mouse.click(box.x + box.width - 20, box.y + 20);
    await expect(page.locator(".img-frame")).toBeVisible();
  });
});

test.describe("shapes behind the text", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "A line of text over the shape",
            { raw: floatShape({ id: 21, name: "Backdrop", behind: true, x: 72, relH: "page", y: 60, relV: "page", w: 320, h: 200, z: 251658240, fill: "FFC000" }) },
            "Second line of text",
          ],
        },
      ],
    });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
  });

  test("a click where there is no text selects it; a click on the text puts the caret there", async ({ page }) => {
    const shape = atoms(page).first();
    await expect(shape).toHaveClass(/wrap-behind/);
    expect(await zIndex(shape)).toBeLessThan(0);
    // The line of text is 72 points down and the shape starts 60 down and 72 across: to the right of the words is empty page.
    const blank = await onSheet(page, 330, 82);
    await page.mouse.click(blank.x, blank.y);
    await expect(frame(page)).toBeVisible();
    await expect(strip(page)).toBeVisible();
    const at = await where(page);
    expect((await shapeAt(page, at.p, at.o))!.wrap).toBe("behind");
    // On the words it is the text that is clicked.
    await page.keyboard.press("Escape");
    await expect(frame(page)).toBeHidden();
    const word = page.locator(".flow .fp").first().locator(".s").first();
    await word.click({ position: { x: 20, y: 5 } });
    await expect(frame(page)).toBeHidden();
    // Alt picks the shape even over the words.
    await word.click({ position: { x: 20, y: 5 }, modifiers: ["Alt"] });
    await expect(frame(page)).toBeVisible();
    // Outside the shape the page is the page.
    await page.mouse.click((await onSheet(page, 500, 400)).x, (await onSheet(page, 500, 400)).y);
    await expect(frame(page)).toBeHidden();
  });

  test("the shape command still selects it, and it can be moved and sent in front of the text", async ({ page }) => {
    const at = await where(page);
    await inPage(page, (w, p) => (w.fe.extensions as unknown as { execute(id: string, ...args: unknown[]): Promise<unknown> }).execute("shape.select", p.p, p.o), at);
    await expect(frame(page)).toBeVisible();
    const before = (await shapeAt(page, at.p, at.o))!;
    // Drag it by an empty part.
    const start = await onSheet(page, 330, 120);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x + 20, start.y + 10, { steps: 4 });
    await page.mouse.up();
    const after = (await shapeAt(page, at.p, at.o))!;
    expect(after.x! - before.x!).toBeCloseTo(20 / PT_PX, 0);
    // In front of the text: it now takes the click on the words.
    await arrange(page, "In front of text");
    expect((await shapeAt(page, at.p, at.o))!.wrap).toBe("front");
    expect(await zIndex(atoms(page).first())).toBeGreaterThan(0);
  });
});

test.describe("groups", () => {
  test("the first click selects the group, the next one a member that can be recoloured, moved and typed in", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before", { raw: GROUP }, "After"] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    const group = atoms(page).first();
    const left = group.locator(".shape-child").nth(0);
    const right = group.locator(".shape-child").nth(1);
    const fillChip = () => strip(page).getByRole("button", { name: "Fill", exact: true });
    const at = await where(page);
    await left.click();
    await expect(frame(page)).toBeVisible();
    // The group itself: its members' colours are not its own.
    await expect(fillChip()).toBeDisabled();
    expect(await frame(page).evaluate((e) => e.getBoundingClientRect().width)).toBeCloseTo((await group.boundingBox())!.width, 0);
    // The second click selects the member under the pointer.
    await left.click();
    await expect(fillChip()).toBeEnabled();
    expect(await frame(page).evaluate((e) => e.getBoundingClientRect().width)).toBeCloseTo((await left.boundingBox())!.width, 0);
    await fillChip().click();
    const swatch = page.locator(".pop .swatch").nth(8);
    const picked = ((await swatch.getAttribute("title")) ?? "").slice(1).toUpperCase();
    await swatch.click();
    const g = (await shapeAt(page, at.p, at.o))!;
    expect(g.children![0].fill!.color).toBe(picked);
    expect(g.children![1].fill!.color).toBe("00FF00");
    // The file keeps it in the group's coordinates: in the DrawingML member and in the VML copy.
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain(`<a:srgbClr val="${picked}"/>`);
    expect(xml.toLowerCase()).toContain(`fillcolor="#${picked.toLowerCase()}"`);
    // Moving the member: it lands where it was dropped, the other one stays.
    const z = await zoomOf(page);
    const box = (await left.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 15, box.y + box.height / 2 + 9, { steps: 4 });
    await page.mouse.up();
    const moved = (await shapeAt(page, at.p, at.o))!;
    expect(moved.children![0].childX!).toBeCloseTo(15 / z / PT_PX, 0);
    expect(moved.children![0].childY!).toBeCloseTo(9 / z / PT_PX, 0);
    expect(moved.children![1].childX).toBe(140);
    // Typing goes into the member that has text: select it with a click, then Enter.
    await right.click();
    await right.click();
    await page.keyboard.press("Enter");
    await page.keyboard.type("!");
    await expect(right.locator(".shape-text")).toContainText("In the group!");
    // Escape leaves the text; Escape again goes back to the group.
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await expect(fillChip()).toBeDisabled();
  });

  test("Shift-click adds a shape, Group joins them into one, Ungroup puts them back where they were", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "Some text on the page",
            {
              raw:
                floatShape({ id: 41, name: "One", x: 100, relH: "page", y: 200, relV: "page", z: 251658240, fill: "FF0000", w: 80, h: 50 }) +
                floatShape({ id: 42, name: "Two", x: 260, relH: "page", y: 300, relV: "page", z: 251659264, fill: "0000FF", w: 60, h: 40, text: "Second" }),
            },
          ],
        },
      ],
    });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    const one = atoms(page).nth(0);
    const two = atoms(page).nth(1);
    const first = (await one.boundingBox())!;
    const second = (await two.boundingBox())!;
    const sheetTop = async () => (await page.locator(".sheets .sheet").first().boundingBox())!;
    const s0 = await sheetTop();
    await page.mouse.click(first.x + 6, first.y + 6);
    await page.keyboard.down("Shift");
    await page.mouse.click(second.x + 6, second.y + 6);
    await page.keyboard.up("Shift");
    // Positions on the sheet: selecting scrolls the page.
    const onPage = async (loc: Locator) => {
      const b = (await loc.boundingBox())!;
      const s = await sheetTop();
      return { x: b.x - s.x, y: b.y - s.y };
    };
    const firstAt = { x: first.x - s0.x, y: first.y - s0.y };
    const secondAt = { x: second.x - s0.x, y: second.y - s0.y };
    await expect(strip(page).getByText("2 shapes selected")).toBeVisible();
    await strip(page).getByRole("button", { name: "Group", exact: true }).click();
    await expect(atoms(page)).toHaveCount(1);
    const group = atoms(page).first();
    await expect(group.locator(".shape-child")).toHaveCount(2);
    // The group covers both, and each member is where it was on the page.
    const gb = await onPage(group);
    expect(gb.x).toBeCloseTo(firstAt.x, 0);
    expect(gb.y).toBeCloseTo(firstAt.y, 0);
    const kids = group.locator(".shape-child");
    const k2 = await onPage(kids.nth(1));
    expect(k2.x).toBeCloseTo(secondAt.x, 0);
    expect(k2.y).toBeCloseTo(secondAt.y, 0);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<wpg:wgp>");
    expect(xml).toContain("<v:group ");
    // The text of the member in the group is still the member's, editable.
    await expect(kids.nth(1).locator(".shape-text")).toHaveText("Second");
    await expect(strip(page).getByRole("button", { name: "Ungroup", exact: true })).toBeVisible();
    await strip(page).getByRole("button", { name: "Ungroup", exact: true }).click();
    await expect(atoms(page)).toHaveCount(2);
    const back1 = await onPage(atoms(page).nth(0));
    const back2 = await onPage(atoms(page).nth(1));
    expect(back1.x).toBeCloseTo(firstAt.x, 0);
    expect(back1.y).toBeCloseTo(firstAt.y, 0);
    expect(back2.x).toBeCloseTo(secondAt.x, 0);
    expect(back2.y).toBeCloseTo(secondAt.y, 0);
    await expect(atoms(page).nth(1).locator(".shape-text")).toHaveText("Second");
    expect(await savedPart(page, "word/document.xml")).not.toContain("wpg:wgp");
    // Both are selected after Ungroup.
    await expect(strip(page).getByText("2 shapes selected")).toBeVisible();
    // Undo brings the group back, and again the two shapes.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(1);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(2);
  });
});

test.describe("shapes in a header", () => {
  test("a shape in the header being edited is selected, recoloured and sent to the back", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          headers: { default: "Header words", raw: floatShape({ id: 51, name: "Logo", x: 440, relH: "page", y: 30, relV: "page", w: 80, h: 30, fill: "FF0000", z: 251658240 }) + floatShape({ id: 52, name: "Badge", x: 470, relH: "page", y: 40, relV: "page", w: 50, h: 30, fill: "0000FF", z: 251659264 }) },
          paragraphs: ["Body of the page"],
        },
      ],
    });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    // Until the header is edited its shapes are part of the page's picture.
    const hf = page.locator(".sheet .hf.hdr");
    await expect(hf.locator(".atom.shape")).toHaveCount(2);
    const body = await savedPart(page, "word/document.xml");
    await chooseMenu(page, "Insert", "Page elements", "Edit header");
    const story = page.locator(".story-edit");
    await expect(story).toBeVisible();
    const logo = story.locator(".atom.shape").nth(0);
    const badge = story.locator(".atom.shape").nth(1);
    const lb = (await logo.boundingBox())!;
    // The logo's corner nothing covers.
    await page.mouse.click(lb.x + 4, lb.y + 4);
    await expect(frame(page)).toBeVisible();
    await expect(strip(page)).toBeVisible();
    await strip(page).getByRole("button", { name: "Fill", exact: true }).click();
    const swatch = page.locator(".pop .swatch").nth(5);
    const picked = ((await swatch.getAttribute("title")) ?? "").slice(1).toUpperCase();
    await swatch.click();
    await expect.poll(async () => (await savedPart(page, "word/header1.xml")).includes(`<a:srgbClr val="${picked}"/>`)).toBe(true);
    // The order of the header's shapes is the header's own.
    expect(await zIndex(badge)).toBeGreaterThan(await zIndex(logo));
    await arrange(page, "Bring to front");
    expect(await zIndex(logo)).toBeGreaterThan(await zIndex(badge));
    const header = await savedPart(page, "word/header1.xml");
    const [a, b] = heights(header);
    expect(a).toBeGreaterThan(b);
    // The body of the document is untouched.
    expect(await savedPart(page, "word/document.xml")).toBe(body);
    // Moving it with the arrow keys writes the offset in the header, counted from the page.
    const x0 = (await frame(page).boundingBox())!.x;
    await page.keyboard.press("Shift+ArrowLeft");
    await expect.poll(async () => (await frame(page).boundingBox())!.x).toBeCloseTo(x0 - 10 * PT_PX, 0);
    // Leaving the header lets go of the shape.
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await expect(frame(page)).toBeHidden();
  });
});

test.describe("shapes placed from the page", () => {
  const PAGE_PX = { w: 612 * PT_PX, h: 792 * PT_PX };

  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "First paragraph",
            "Second paragraph",
            {
              raw:
                floatShape({ id: 61, name: "FromPage", x: 100, relH: "page", y: 200, relV: "page", w: 100, h: 60, fill: "FF0000" }) +
                floatShape({ id: 62, name: "FromMargin", x: 36, relH: "margin", y: 36, relV: "margin", w: 100, h: 60, fill: "00AA00" }) +
                floatShape({ id: 63, name: "Centered", x: "center", relH: "page", y: "center", relV: "page", w: 100, h: 60, fill: "0000FF" }) +
                floatShape({ id: 64, name: "FromParagraph", x: 20, relH: "column", y: 50, relV: "paragraph", w: 100, h: 60, fill: "FFC000" }),
            },
          ],
        },
      ],
    });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
  });

  test("each shape is where its offset counts from the page, the margins, the column or the paragraph", async ({ page }) => {
    const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
    const rel = async (n: number) => {
      const b = (await atoms(page).nth(n).boundingBox())!;
      return { x: b.x - sheet.x, y: b.y - sheet.y };
    };
    const near = (a: { x: number; y: number }, x: number, y: number) => {
      expect(a.x).toBeCloseTo(x, 0);
      expect(a.y).toBeCloseTo(y, 0);
    };
    near(await rel(0), 100 * PT_PX, 200 * PT_PX);
    // One inch of margin on every side.
    near(await rel(1), (72 + 36) * PT_PX, (72 + 36) * PT_PX);
    near(await rel(2), (PAGE_PX.w - 100 * PT_PX) / 2, (PAGE_PX.h - 60 * PT_PX) / 2);
    const paraTop = await page
      .locator(".flow .fp")
      .last()
      .evaluate((e, top) => e.getBoundingClientRect().top - top, sheet.y);
    near(await rel(3), (72 + 20) * PT_PX, paraTop + 50 * PT_PX);
  });

  test("dragging writes the new offset from the same place, and the shape is drawn where it was dropped", async ({ page }) => {
    const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
    const at = await where(page, 0);
    const shape = atoms(page).nth(0);
    const box = (await shape.boundingBox())!;
    await page.mouse.click(box.x + 50, box.y + 30);
    await page.mouse.move(box.x + 50, box.y + 30);
    await page.mouse.down();
    await page.mouse.move(box.x + 70, box.y + 45, { steps: 4 });
    await page.mouse.move(box.x + 90, box.y + 60, { steps: 4 });
    await page.mouse.up();
    const s = (await shapeAt(page, at.p, at.o))!;
    // Still counted from the page, now 40 pixels right and 30 down of where it was.
    expect(Math.abs(s.x! - (100 + 40 / PT_PX))).toBeLessThan(1);
    expect(Math.abs(s.y! - (200 + 30 / PT_PX))).toBeLessThan(1);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('<wp:positionH relativeFrom="page">');
    expect(xml).toContain('<wp:positionV relativeFrom="page">');
    const now = (await shape.boundingBox())!;
    expect(Math.abs(now.x - sheet.x - (100 * PT_PX + 40))).toBeLessThan(1.5);
    expect(Math.abs(now.y - sheet.y - (200 * PT_PX + 30))).toBeLessThan(1.5);
    // The arrow keys move by a point from the same place.
    await page.keyboard.press("ArrowRight");
    await expect.poll(async () => Math.abs((await shapeAt(page, at.p, at.o))!.x! - (100 + 40 / PT_PX + 1))).toBeLessThan(1);
    const after = (await shape.boundingBox())!;
    expect(Math.abs(after.x - sheet.x - (100 * PT_PX + 40 + PT_PX))).toBeLessThan(1.5);
  });
});

test.describe("drawing by hand", () => {
  test("two strokes and Done make one shape that survives saving and opening again", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Draw below this line", "A second paragraph", "A third paragraph"] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    const layer = page.locator(".ink-layer");
    await expect(layer).toBeVisible();
    const bar = page.locator(".ink-strip");
    await expect(bar).toBeVisible();
    const start = await onSheet(page, 120, 160);
    const stroke = async (dx: number, dy: number) => {
      await page.mouse.move(start.x + dx, start.y + dy);
      await page.mouse.down();
      for (let i = 1; i <= 12; i++) await page.mouse.move(start.x + dx + i * 8, start.y + dy + Math.sin(i / 2) * 14);
      await page.mouse.up();
    };
    await stroke(0, 0);
    await expect(layer.locator("path.ink-stroke")).toHaveCount(1);
    await stroke(0, 50);
    await expect(layer.locator("path.ink-stroke")).toHaveCount(2);
    await expect(bar.getByText("2 strokes")).toBeVisible();
    // The last stroke can be taken back.
    await bar.getByRole("button", { name: "Undo stroke" }).click();
    await expect(layer.locator("path.ink-stroke")).toHaveCount(1);
    await stroke(0, 50);
    await bar.getByRole("button", { name: "Done", exact: true }).click();
    await expect(layer).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(1);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.geometry).toBe("custom");
    expect(s.custom!.paths).toHaveLength(2);
    expect(s.wrap).toBe("front");
    // It is drawn where the strokes were.
    const box = (await atoms(page).first().boundingBox())!;
    expect(Math.abs(box.x - (start.x - 2))).toBeLessThan(6);
    expect(Math.abs(box.y - (start.y - 14 - 2))).toBeLessThan(10);
    await expect(frame(page)).toBeVisible();
    // Undo takes the whole drawing out in one step, and redo brings it back.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect(atoms(page)).toHaveCount(1);
    await inPage(page, (w, p) => (w.fe.extensions as unknown as { execute(id: string, ...args: unknown[]): Promise<unknown> }).execute("shape.select", p.p, p.o), at);
    await expect(frame(page)).toBeVisible();
    // It is an ordinary shape: recolour it.
    await strip(page).getByRole("button", { name: "Outline", exact: true }).click();
    const swatch = page.locator(".pop .swatch").nth(4);
    const picked = ((await swatch.getAttribute("title")) ?? "").slice(1).toUpperCase();
    await swatch.click();
    expect(((await shapeAt(page, at.p, at.o))!.custom!.paths as unknown[]).length).toBe(2);
    // Save and open the saved file: the drawing is still one shape with two paths.
    await inPage(page, async (w) => {
      await w.fe.open(w.fe.editor.engine!.save(), "again.docx");
    });
    await expect(atoms(page)).toHaveCount(1);
    const back = await where(page);
    const s2 = (await shapeAt(page, back.p, back.o))!;
    expect(s2.custom!.paths).toHaveLength(2);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<a:custGeom>");
    expect(xml).toContain(`<a:srgbClr val="${picked}"/>`);
    expect(xml).toContain('path="m');
  });

  test("starting to draw lets go of a shape that is selected", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Some words", "More words"] }] });
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_shape", at: { p: 1, o: 0 }, geometry: "ellipse", w: 60, h: 40 }));
    await atoms(page).first().click();
    await expect(frame(page)).toBeVisible();
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    await expect(page.locator(".ink-layer")).toBeVisible();
    await expect(frame(page)).toBeHidden();
    await expect(strip(page)).toBeHidden();
    await page.keyboard.press("Escape");
    await expect(page.locator(".ink-layer")).toHaveCount(0);
  });

  test("Escape puts the pen down without drawing anything", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Nothing to see"] }] });
    await chooseMenu(page, "Insert", "Drawing", "Draw");
    const layer = page.locator(".ink-layer");
    await expect(layer).toBeVisible();
    const start = await onSheet(page, 120, 200);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x + 60, start.y + 20, { steps: 5 });
    await page.mouse.up();
    await page.keyboard.press("Escape");
    await expect(layer).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(0);
  });
});
