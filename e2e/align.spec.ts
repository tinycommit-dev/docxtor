// Aligning and distributing objects: Format, Arrange, Align. Each command with two and three shapes (where the shapes
// land on the pages), aligning to the margin and the page (the alignment is written, as Word writes it), spreading
// evenly, shapes in line with the text, a picture and a shape together, and one undo for the whole command.
import { floatShape } from "./support/arrange-markup";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, expect, inPage, menuItem, openMenu, test, type Locator, type Page } from "./helpers";

const PT_PX = 96 / 72;

interface Shape {
  x?: number;
  y?: number;
  w: number;
  h: number;
  align?: string;
  alignY?: string;
  relX?: string;
  relY?: string;
  wrap: string;
}

const shapeAt = (page: Page, p: number, o: number) =>
  inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape | null>;
const atoms = (page: Page) => page.locator(".flow .atom.shape");
const zoomOf = (page: Page) => inPage(page, (w) => w.fe.editor.zoom);
const where = (page: Page, n: number) =>
  atoms(page)
    .nth(n)
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));

/** A shape's box on the first page, in points from the top-left corner of the sheet. */
async function onPage(page: Page, loc: Locator) {
  const b = (await loc.boundingBox())!;
  const s = (await page.locator(".sheets .sheet").first().boundingBox())!;
  const k = PT_PX * (await zoomOf(page));
  return { left: (b.x - s.x) / k, top: (b.y - s.y) / k, right: (b.x + b.width - s.x) / k, bottom: (b.y + b.height - s.y) / k };
}

/** Three shapes in front of the text, none touching: A at (100, 200) 80 by 50, B at (200, 260) 100 by 40, C at (330, 230) 60 by 70. */
const THREE =
  floatShape({ id: 51, name: "A", x: 100, relH: "page", y: 200, relV: "page", w: 80, h: 50, fill: "FF0000" }) +
  floatShape({ id: 52, name: "B", x: 200, relH: "page", y: 260, relV: "page", w: 100, h: 40, fill: "00AA00", z: 251659264 }) +
  floatShape({ id: 53, name: "C", x: 330, relH: "page", y: 230, relV: "page", w: 60, h: 70, fill: "0000FF", z: 251660288 });

async function openThree(page: Page, raw = THREE) {
  await openBuilt(page, { sections: [{ paragraphs: ["Some text on the page", { raw }] }] });
  await inPage(page, (w) => w.fe.editor.setZoom(1));
}

/** Select shapes by clicking the top-left corner of each (the first with a click, the rest with Shift). */
async function select(page: Page, ...which: number[]) {
  // Let go of a selection first: the strip over a selected shape can sit on top of the next one.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  for (const [i, n] of which.entries()) {
    const b = (await atoms(page).nth(n).boundingBox())!;
    if (i) await page.keyboard.down("Shift");
    await page.mouse.click(b.x + 6, b.y + 6);
    if (i) await page.keyboard.up("Shift");
  }
}

const align = (page: Page, label: string) => chooseMenu(page, "Format", "Arrange", "Align", label);

/** Open Format, Arrange, Align and leave the menu open. */
async function openAlign(page: Page) {
  await openMenu(page, "Format");
  await menuItem(page, "Arrange").click();
  await menuItem(page, "Align").click();
}

const near = (a: number, b: number, tol = 0.8) => expect(Math.abs(a - b), `${a} is not near ${b}`).toBeLessThan(tol);

test.describe("align to the selected objects", () => {
  // Left edges 100, 200, 330; right edges 180, 300, 390; so the group is 100 to 390 wide and its middle is 245.
  const across: [string, (b: { left: number; right: number }) => number, number][] = [
    ["Align left", (b) => b.left, 100],
    ["Align center", (b) => (b.left + b.right) / 2, 245],
    ["Align right", (b) => b.right, 390],
  ];
  for (const [label, edge, want] of across) {
    test(`${label} lines the shapes up across`, async ({ page }) => {
      await openThree(page);
      await select(page, 0, 1, 2);
      await align(page, label);
      for (let i = 0; i < 3; i++) near(edge(await onPage(page, atoms(page).nth(i))), want);
      // The offsets are written from the page, where the shapes were counted from.
      const at = await where(page, 0);
      const s = (await shapeAt(page, at.p, at.o))!;
      expect(s.relX).toBe("page");
      expect(s.align).toBeUndefined();
    });
  }

  // Tops 200, 260, 230; bottoms 250, 300, 300; so the group is 200 to 300 high and its middle is 250.
  const down: [string, (b: { top: number; bottom: number }) => number, number][] = [
    ["Align top", (b) => b.top, 200],
    ["Align middle", (b) => (b.top + b.bottom) / 2, 250],
    ["Align bottom", (b) => b.bottom, 300],
  ];
  for (const [label, edge, want] of down) {
    test(`${label} lines the shapes up down the page`, async ({ page }) => {
      await openThree(page);
      await select(page, 0, 1, 2);
      await align(page, label);
      for (let i = 0; i < 3; i++) near(edge(await onPage(page, atoms(page).nth(i))), want);
    });
  }

  test("with two shapes", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 2);
    await align(page, "Align bottom");
    // A (200 to 250) and C (230 to 300): both end at 300; B did not move.
    near((await onPage(page, atoms(page).nth(0))).bottom, 300);
    near((await onPage(page, atoms(page).nth(2))).bottom, 300);
    near((await onPage(page, atoms(page).nth(1))).top, 260);
  });

  test("one undo puts every shape back, and redo does it again", async ({ page }) => {
    await openThree(page);
    const before = await Promise.all([0, 1, 2].map((i) => onPage(page, atoms(page).nth(i))));
    await select(page, 0, 1, 2);
    await align(page, "Align left");
    near((await onPage(page, atoms(page).nth(2))).left, 100);
    await page.keyboard.press("ControlOrMeta+Z");
    for (let i = 0; i < 3; i++) {
      const b = await onPage(page, atoms(page).nth(i));
      near(b.left, before[i].left);
      near(b.top, before[i].top);
    }
    await page.keyboard.press("ControlOrMeta+Shift+Z");
    near((await onPage(page, atoms(page).nth(2))).left, 100);
  });

  test("the lines are in the right-click menu on a selection of several, and not on one shape", async ({ page }) => {
    await openThree(page);
    await select(page, 0);
    const b = (await atoms(page).nth(0).boundingBox())!;
    await page.mouse.click(b.x + 6, b.y + 6, { button: "right" });
    await expect(contextItem(page, "Align left")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await select(page, 0, 1, 2);
    await page.mouse.click(b.x + 6, b.y + 6, { button: "right" });
    await expect(contextItem(page, "Align left")).toBeVisible();
    await expect(contextItem(page, "Distribute horizontally")).toBeVisible();
    await contextItem(page, "Align top").click();
    for (let i = 0; i < 3; i++) near((await onPage(page, atoms(page).nth(i))).top, 200);
  });
});

test.describe("distribute", () => {
  test("horizontally keeps the outer two and makes the gaps equal", async ({ page }) => {
    await openThree(page);
    // Make the gaps unequal first: B near C.
    await select(page, 1);
    await page.keyboard.press("ArrowRight");
    await select(page, 0, 1, 2);
    await align(page, "Distribute horizontally");
    const [a, b, c] = await Promise.all([0, 1, 2].map((i) => onPage(page, atoms(page).nth(i))));
    near(a.left, 100);
    near(c.right, 390);
    // Widths 80 + 100 + 60 = 240 of 290: two gaps of 25.
    near(b.left - a.right, 25);
    near(c.left - b.right, 25);
  });

  test("vertically", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 1, 2);
    await align(page, "Distribute vertically");
    // Sorted by top: A (200 to 250), C (230 to 300), B (260 to 300): the group is 200 to 300 and the heights add to 160.
    const [a, b, c] = await Promise.all([0, 1, 2].map((i) => onPage(page, atoms(page).nth(i))));
    near(a.top, 200);
    near(Math.max(b.bottom, c.bottom), 300);
    const sorted = [a, c, b];
    const gap1 = sorted[1].top - sorted[0].bottom;
    const gap2 = sorted[2].top - sorted[1].bottom;
    near(gap1, gap2);
  });

  test("needs three shapes among themselves", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 1);
    await openAlign(page);
    await expect(menuItem(page, "Distribute horizontally")).toBeDisabled();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await select(page, 0, 1, 2);
    await openAlign(page);
    await expect(menuItem(page, "Distribute horizontally")).toBeEnabled();
  });
});

test.describe("align to the margin or the page", () => {
  test("is written as Word writes it, relative from the margin with an alignment", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 1, 2);
    await chooseMenu(page, "Format", "Arrange", "Align", "Align to margin");
    await align(page, "Align left");
    // The margins are 72 points on a Letter page.
    for (let i = 0; i < 3; i++) near((await onPage(page, atoms(page).nth(i))).left, 72);
    const at = await where(page, 1);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.relX).toBe("margin");
    expect(s.align).toBe("left");
    const xml = await savedDocument(page);
    expect(xml.match(/<wp:positionH relativeFrom="margin"><wp:align>left<\/wp:align><\/wp:positionH>/g)).toHaveLength(3);
    // The vertical alignment is separate: the shapes keep where they are down the page.
    expect(s.y).toBe(260);
  });

  test("middle between the top and bottom margins", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 1, 2);
    await chooseMenu(page, "Format", "Arrange", "Align", "Align to margin");
    await align(page, "Align middle");
    const at = await where(page, 1);
    const m = await shapeAt(page, at.p, at.o);
    expect(m!.relY).toBe("margin");
    expect(m!.alignY).toBe("center");
    // Centred between the top and bottom margins: 72 to 720 on a Letter page.
    for (let i = 0; i < 3; i++) {
      const b = await onPage(page, atoms(page).nth(i));
      near((b.top + b.bottom) / 2, 396, 1.2);
    }
  });

  test("the page edge and the bottom, and the choice sticks", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 1);
    await chooseMenu(page, "Format", "Arrange", "Align", "Align to page");
    await align(page, "Align right");
    near((await onPage(page, atoms(page).nth(0))).right, 612, 1.2);
    near((await onPage(page, atoms(page).nth(1))).right, 612, 1.2);
    // The choice stays: the next command goes to the page too, and the menu shows it checked.
    await select(page, 0);
    await align(page, "Align bottom");
    near((await onPage(page, atoms(page).nth(0))).bottom, 792, 1.5);
    const at = await where(page, 0);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.relY).toBe("page");
    expect(s.alignY).toBe("bottom");
    await openAlign(page);
    await expect(menuItem(page, "Align to page")).toHaveAttribute("aria-checked", "true");
    await expect(menuItem(page, "Align to margin")).toHaveAttribute("aria-checked", "false");
    await page.keyboard.press("Escape");
  });

  test("a single shape goes to the page even with 'selected objects' chosen", async ({ page }) => {
    await openThree(page);
    await select(page, 2);
    await align(page, "Align center");
    const b = await onPage(page, atoms(page).nth(2));
    near((b.left + b.right) / 2, 306, 1.2);
    const at = await where(page, 2);
    expect((await shapeAt(page, at.p, at.o))!.relX).toBe("page");
  });

  test("distributing across the margins spreads from margin to margin", async ({ page }) => {
    await openThree(page);
    await select(page, 0, 1);
    await chooseMenu(page, "Format", "Arrange", "Align", "Align to margin");
    await align(page, "Distribute horizontally");
    // Two shapes: the first on the left margin, the last on the right (540 on a Letter page with one inch margins).
    near((await onPage(page, atoms(page).nth(0))).left, 72);
    near((await onPage(page, atoms(page).nth(1))).right, 540);
  });
});

test.describe("what can't be aligned", () => {
  test("a shape in line with the text is left out, with a message", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Some text on the page", { raw: THREE }] }] });
    await inPage(page, (w) => w.fe.editor.setZoom(1));
    // Put the third shape in line with the text.
    const at = await where(page, 2);
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "set_shape", at: a, wrap: "inline" }), at);
    await select(page, 0, 1, 2);
    await align(page, "Align left");
    await expect(page.locator(".toast")).toContainText("in line with the text");
    for (let i = 0; i < 2; i++) near((await onPage(page, atoms(page).nth(i))).left, 100);
  });

  test("only the inline shape selected: nothing moves, and it says why", async ({ page }) => {
    await openThree(page);
    const at = await where(page, 2);
    await inPage(page, (w, a) => w.fe.editor.exec({ op: "set_shape", at: a, wrap: "inline" }), at);
    await atoms(page).nth(2).click();
    await align(page, "Align left");
    await expect(page.locator(".toast")).toContainText("in line with the text");
  });
});

test.describe("pictures and charts", () => {
  test("a picture aligns with a shape (picked from the Selection pane)", async ({ page }) => {
    await openThree(page);
    // A floating picture at (250, 340).
    const pos = await inPage(page, (w) => {
      const eng = w.fe.editor.engine!;
      const ed = w.fe.editor;
      const png = Uint8Array.from(atob("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="), (c) => c.charCodeAt(0));
      const media = eng.addMedia("png", png);
      const p = eng.renderAll().order.at(-1)!;
      ed.exec({ op: "insert_image", at: { p, o: 3 }, media, width: 50, height: 40 });
      ed.exec({ op: "set_image", at: { p, o: 3 }, wrap: "front", align: "left" });
      return { p, o: 3 };
    });
    await expect(page.locator(".flow .atom.img")).toHaveCount(1);
    await chooseMenu(page, "Format", "Arrange", "Selection pane");
    const rows = page.locator(".sp-row");
    await expect(rows).toHaveCount(4);
    await rows.filter({ hasText: /^A$/ }).click();
    await rows.filter({ hasText: /Picture/ }).click({ modifiers: ["Shift"] });
    await align(page, "Align right");
    const shapeBox = await onPage(page, atoms(page).nth(0));
    const picBox = await onPage(page, page.locator(".flow .atom.img"));
    near(picBox.right, shapeBox.right, 1.2);
    // Written as an offset from where the picture counts from, not as an alignment any more.
    const info = await inPage(page, (w, a) => w.fe.editor.engine!.objects().find((o) => o.p === a.p && o.o === a.o), pos);
    expect(info?.align).toBeUndefined();
    expect(typeof info?.x).toBe("number");
  });
});

/** The saved word/document.xml. */
async function savedDocument(page: Page): Promise<string> {
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
