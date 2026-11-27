// The Selection pane: every object of the document in a list, front to back, with an eye to hide it, a name to change,
// rows to click and to drag into a new stacking order.
import { floatShape } from "./support/arrange-markup";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const PT_PX = 96 / 72;

const atoms = (page: Page) => page.locator(".flow .atom.shape");
const pane = (page: Page) => page.locator('.dock[data-dock="selection-pane"]');
const rows = (page: Page) => pane(page).locator(".sp-row");
const row = (page: Page, name: string) => rows(page).filter({ has: page.locator(".sp-name", { hasText: new RegExp(`^${name}$`) }) });
const names = (page: Page) => rows(page).locator(".sp-name").allTextContents();
const objects = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.objects());
const zoomOf = (page: Page) => inPage(page, (w) => w.fe.editor.zoom);

/** Red (behind, low), Blue (above it), and Green apart from both; Backdrop behind the text. */
const FOUR =
  floatShape({ id: 61, name: "Red", x: 100, relH: "page", y: 200, relV: "page", w: 100, h: 60, fill: "FF0000", z: 251658240 }) +
  floatShape({ id: 62, name: "Blue", x: 160, relH: "page", y: 230, relV: "page", w: 100, h: 60, fill: "0000FF", z: 251659264 }) +
  floatShape({ id: 63, name: "Green", x: 340, relH: "page", y: 400, relV: "page", w: 80, h: 50, fill: "00AA00", z: 251660288 }) +
  floatShape({ id: 64, name: "Backdrop", x: 72, relH: "page", y: 120, relV: "page", w: 200, h: 40, fill: "FFC000", z: 251658240, behind: true });

async function openFour(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Some text on the page", { raw: FOUR }] }] });
  await inPage(page, (w) => w.fe.editor.setZoom(1));
}

async function openPane(page: Page) {
  await chooseMenu(page, "Format", "Arrange", "Selection pane");
  await expect(pane(page)).toBeVisible();
}

const sheetPoint = async (page: Page, x: number, y: number) => {
  const sheet = (await page.locator(".sheets .sheet").first().boundingBox())!;
  const k = PT_PX * (await zoomOf(page));
  return { x: sheet.x + x * k, y: sheet.y + y * k };
};

/** The name of the shape drawn on top at a point of the page (in points from the sheet's corner). */
const topAt = (page: Page, x: number, y: number) =>
  sheetPoint(page, x, y).then((p) =>
    page.evaluate(([px, py]) => {
      for (const el of document.elementsFromPoint(px, py)) {
        const atom = el.closest(".atom.shape");
        if (atom) return (atom.closest("[data-p]") as HTMLElement | null) ? Array.from(document.querySelectorAll(".flow .atom.shape")).indexOf(atom) : -1;
      }
      return -1;
    }, [p.x, p.y]),
  );

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

test.describe("the list", () => {
  test("lists every object front to back, with its name, in sections for the stacks", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    // Green is nearest, then Blue, then Red; Backdrop is behind the text.
    expect(await names(page)).toEqual(["Green", "Blue", "Red", "Backdrop"]);
    await expect(pane(page).locator(".sp-section")).toHaveText(["In front of the text", "Behind the text"]);
    await expect(pane(page).locator(".sp-count")).toHaveText("4 objects");
    await expect(rows(page).first().locator(".sp-kind svg")).toBeVisible();
    // The pane is in the menu as a checked line, and the command closes it.
    await chooseMenu(page, "Format", "Arrange", "Selection pane");
    await expect(pane(page)).toHaveCount(0);
  });

  test("a picture and a chart are in it, and an object in line with the text under its own heading", async ({ page }) => {
    await openFour(page);
    await inPage(page, (w) => {
      const eng = w.fe.editor.engine!;
      const png = Uint8Array.from(atob("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="), (c) => c.charCodeAt(0));
      const p = eng.renderAll().order.at(-1)!;
      w.fe.editor.exec({ op: "insert_image", at: { p, o: 0 }, media: eng.addMedia("png", png), width: 40, height: 30 });
      w.fe.editor.exec({ op: "insert_chart", at: { p, o: 0 }, kind: "column", title: "Sales", categories: ["A", "B"], series: [{ name: "One", values: [1, 2] }] });
    });
    await openPane(page);
    const list = await names(page);
    expect(list).toHaveLength(6);
    expect(list.filter((n) => /^Picture/.test(n))).toHaveLength(1);
    expect(list.filter((n) => /^Chart/.test(n))).toHaveLength(1);
    await expect(pane(page).locator(".sp-section")).toHaveText(["In front of the text", "Behind the text", "In line with the text"]);
    // An object in line has no place in the stack: no grip to drag it by.
    await expect(rows(page).filter({ hasText: /^Picture/ }).locator(".sp-grip svg")).toHaveCount(0);
    await expect(row(page, "Green").locator(".sp-grip svg")).toHaveCount(1);
  });

  test("an empty document says so", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Nothing drawn here"] }] });
    await openPane(page);
    await expect(pane(page).locator(".sp-empty")).toBeVisible();
    await expect(pane(page).locator(".sp-count")).toHaveText("0 objects");
  });
});

test.describe("beside another panel", () => {
  test("opens to the left of the Format shape panel instead of over it", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Green").click();
    await inPage(page, (w) => (w.fe.extensions as unknown as { execute(id: string): Promise<unknown> }).execute("shape.format"));
    const format = page.locator('.dock[data-dock="format-shape"]');
    await expect(format).toBeVisible();
    await expect.poll(async () => {
      const a = (await pane(page).boundingBox())!;
      const b = (await format.boundingBox())!;
      return a.x + a.width <= b.x + 0.5;
    }).toBe(true);
    // Closing the other one leaves the pane where it is, with room to spare, still on screen.
    await format.getByRole("button", { name: "Close" }).click();
    await expect(pane(page)).toBeVisible();
  });
});

test.describe("selecting", () => {
  test("a click on a row selects the object on the page, and Shift adds another", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Green").click();
    await expect(page.locator(".obj-frame").first()).toBeVisible();
    await expect(row(page, "Green")).toHaveClass(/\bon\b/);
    await expect(row(page, "Blue")).not.toHaveClass(/\bon\b/);
    await row(page, "Red").click({ modifiers: ["Shift"] });
    await expect(page.locator(".shape-strip").getByText("2 shapes selected")).toBeVisible();
    await expect(row(page, "Green")).toHaveClass(/\bon\b/);
    await expect(row(page, "Red")).toHaveClass(/\bon\b/);
    // Shift again takes it out.
    await row(page, "Green").click({ modifiers: ["Shift"] });
    await expect(row(page, "Green")).not.toHaveClass(/\bon\b/);
    await expect(row(page, "Red")).toHaveClass(/\bon\b/);
  });

  test("the list follows what is selected on the page", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    const b = (await atoms(page).nth(2).boundingBox())!;
    await page.mouse.click(b.x + 6, b.y + 6);
    await expect(row(page, "Green")).toHaveClass(/\bon\b/);
    const r = (await atoms(page).nth(0).boundingBox())!;
    await page.mouse.click(r.x + 6, r.y + 6);
    await expect(row(page, "Red")).toHaveClass(/\bon\b/);
    await expect(row(page, "Green")).not.toHaveClass(/\bon\b/);
    // Clicking in the text lets go of every row.
    await page.locator(".flow [data-p]").first().click({ position: { x: 8, y: 6 } });
    await expect(pane(page).locator(".sp-row.on")).toHaveCount(0);
  });

  test("the arrow keys move between rows and Enter selects", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await rows(page).first().focus();
    await page.keyboard.press("ArrowDown");
    await expect(rows(page).nth(1)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(rows(page).nth(1)).toHaveClass(/\bon\b/);
    await expect(page.locator(".obj-frame").first()).toBeVisible();
  });
});

test.describe("hiding", () => {
  test("the eye takes the object off the page and out of reach of clicks, and keeps it in the file", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    // Blue overlaps Red at (180, 245).
    expect(await topAt(page, 180, 245)).toBe(1);
    await row(page, "Blue").getByRole("button", { name: "Hide Blue" }).click();
    await expect(atoms(page).nth(1)).toHaveClass(/obj-hidden/);
    await expect(atoms(page).nth(1)).toBeHidden();
    await expect(row(page, "Blue")).toHaveClass(/hidden/);
    // What was under it is on top now, and a click there selects Red, not Blue.
    expect(await topAt(page, 180, 245)).toBe(0);
    const p = await sheetPoint(page, 180, 245);
    await page.mouse.click(p.x, p.y);
    await expect(row(page, "Red")).toHaveClass(/\bon\b/);
    // The model keeps it, with the flag, and so does the saved file.
    const list = await objects(page);
    expect(list.find((o) => o.name === "Blue")?.hidden).toBe(true);
    const xml = await savedDocument(page);
    expect(xml).toContain('name="Blue" hidden="1"');
    expect(xml.match(/hidden="1"/g)).toHaveLength(1);
    expect(xml).toContain("visibility:hidden");
    // Show it again with the same button.
    await row(page, "Blue").getByRole("button", { name: "Show Blue" }).click();
    await expect(atoms(page).nth(1)).toBeVisible();
    expect(await topAt(page, 180, 245)).toBe(1);
    expect(await savedDocument(page)).not.toContain('hidden="1"');
  });

  test("a hidden object is not printed", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Green").getByRole("button", { name: "Hide Green" }).click();
    await page.emulateMedia({ media: "print" });
    await expect(atoms(page).nth(2)).toBeHidden();
    await page.emulateMedia({ media: "screen" });
  });

  test("Hide all and Show all, each one undo step", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    const hide = pane(page).getByRole("button", { name: "Hide all" });
    const show = pane(page).getByRole("button", { name: "Show all" });
    await expect(show).toBeDisabled();
    await hide.click();
    await expect(pane(page).locator(".sp-row.hidden")).toHaveCount(4);
    await expect(atoms(page).first()).toBeHidden();
    await expect(hide).toBeDisabled();
    await show.click();
    await expect(pane(page).locator(".sp-row.hidden")).toHaveCount(0);
    await expect(atoms(page).first()).toBeVisible();
    await hide.click();
    await page.keyboard.press(`${MOD}+Z`);
    await expect(pane(page).locator(".sp-row.hidden")).toHaveCount(0);
    await expect(atoms(page).first()).toBeVisible();
  });

  test("hidden objects stay hidden after saving and opening the file", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Red").getByRole("button", { name: "Hide Red" }).click();
    await inPage(page, async (w) => {
      await w.fe.open(w.fe.editor.engine!.save(), "again.docx");
    });
    await expect(atoms(page)).toHaveCount(4);
    await expect(atoms(page).nth(0)).toBeHidden();
    const list = await objects(page);
    expect(list.filter((o) => o.hidden).map((o) => o.name)).toEqual(["Red"]);
    // The pane that was open lists the opened document.
    await expect(row(page, "Red")).toHaveClass(/hidden/);
    await expect(pane(page).locator(".sp-row.hidden")).toHaveCount(1);
  });
});

test.describe("renaming", () => {
  test("a double-click edits the name in place; Enter keeps it and the file says it", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Green").dblclick();
    const input = pane(page).locator(".sp-name-input");
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Green");
    await input.fill("Logo");
    await page.keyboard.press("Enter");
    await expect(pane(page).locator(".sp-name-input")).toHaveCount(0);
    expect(await names(page)).toEqual(["Logo", "Blue", "Red", "Backdrop"]);
    expect((await objects(page)).map((o) => o.name)).toContain("Logo");
    const xml = await savedDocument(page);
    expect(xml).toContain('<wp:docPr id="63" name="Logo"/>');
    // One undo takes the name back.
    await page.keyboard.press(`${MOD}+Z`);
    expect(await names(page)).toEqual(["Green", "Blue", "Red", "Backdrop"]);
  });

  test("F2 renames and Escape leaves the name as it was", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Blue").focus();
    await page.keyboard.press("F2");
    const input = pane(page).locator(".sp-name-input");
    await expect(input).toBeFocused();
    await input.fill("Scratch");
    await page.keyboard.press("Escape");
    await expect(input).toHaveCount(0);
    expect(await names(page)).toEqual(["Green", "Blue", "Red", "Backdrop"]);
    // The pane is still open: Escape in the input did not close it.
    await expect(pane(page)).toBeVisible();
    await row(page, "Blue").focus();
    await page.keyboard.press("F2");
    await pane(page).locator(".sp-name-input").fill("Sky");
    await pane(page).locator(".sp-name-input").blur();
    expect(await names(page)).toContain("Sky");
  });
});

test.describe("order", () => {
  /** Drag a row to the gap above another row (or below the last one). */
  async function drag(page: Page, from: Locator, to: Locator, where: "above" | "below" = "above") {
    const a = (await from.boundingBox())!;
    const b = (await to.boundingBox())!;
    await page.mouse.move(a.x + 24, a.y + a.height / 2);
    await page.mouse.down();
    await page.mouse.move(a.x + 24, a.y + a.height / 2 + (b.y > a.y ? 6 : -6), { steps: 3 });
    await page.mouse.move(b.x + 24, where === "above" ? b.y + 3 : b.y + b.height - 3, { steps: 6 });
    await expect(pane(page).locator(".sp-drop")).toBeVisible();
    await page.mouse.up();
  }

  test("dragging a row changes the stacking, in the list, on the page and in the file", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    expect(await topAt(page, 180, 245)).toBe(1);
    // Red goes above Blue.
    await drag(page, row(page, "Red"), row(page, "Blue"));
    expect(await names(page)).toEqual(["Green", "Red", "Blue", "Backdrop"]);
    expect(await topAt(page, 180, 245)).toBe(0);
    const list = await objects(page);
    const z = (n: string) => list.find((o) => o.name === n)!.z!;
    expect(z("Red")).toBeGreaterThan(z("Blue"));
    expect(z("Red")).toBeLessThan(z("Green"));
    const xml = await savedDocument(page);
    const heights = [...xml.matchAll(/relativeHeight="(\d+)"/g)].map((m) => Number(m[1]));
    expect(heights[0]).toBeGreaterThan(heights[1]);
    // One undo puts it back.
    await page.keyboard.press(`${MOD}+Z`);
    expect(await names(page)).toEqual(["Green", "Blue", "Red", "Backdrop"]);
    expect(await topAt(page, 180, 245)).toBe(1);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    expect(await names(page)).toEqual(["Green", "Red", "Blue", "Backdrop"]);
  });

  test("Alt and an arrow key move the focused row one place in its stack", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await row(page, "Red").focus();
    await page.keyboard.press("Alt+ArrowUp");
    expect(await names(page)).toEqual(["Green", "Red", "Blue", "Backdrop"]);
    await expect(row(page, "Red")).toBeFocused();
    expect(await topAt(page, 180, 245)).toBe(0);
    await page.keyboard.press("Alt+ArrowDown");
    expect(await names(page)).toEqual(["Green", "Blue", "Red", "Backdrop"]);
    // The last of a stack has nowhere further to go.
    await page.keyboard.press("Alt+ArrowDown");
    expect(await names(page)).toEqual(["Green", "Blue", "Red", "Backdrop"]);
  });

  test("to the bottom of its stack", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    await drag(page, row(page, "Green"), row(page, "Red"), "below");
    expect(await names(page)).toEqual(["Blue", "Red", "Green", "Backdrop"]);
    const list = await objects(page);
    expect(list.find((o) => o.name === "Green")!.z!).toBeLessThan(list.find((o) => o.name === "Red")!.z!);
  });

  test("a row doesn't leave its stack", async ({ page }) => {
    await openFour(page);
    await openPane(page);
    // Dragging Green below Backdrop (behind the text) puts it at the bottom of the front stack, not behind the text.
    await drag(page, row(page, "Green"), row(page, "Backdrop"), "below");
    const list = await objects(page);
    expect(list.find((o) => o.name === "Green")!.behind).toBeFalsy();
    expect(list.find((o) => o.name === "Backdrop")!.behind).toBe(true);
  });
});

test.describe("looks", () => {
  for (const scheme of ["light", "dark"] as const) {
    test(`the pane in ${scheme}`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await openFour(page);
      await openPane(page);
      await row(page, "Blue").getByRole("button", { name: "Hide Blue" }).click();
      await row(page, "Green").click();
      await row(page, "Red").click({ modifiers: ["Shift"] });
      await page.screenshot({ path: `test-results/selection-pane-${scheme}.png` });
      await expect(pane(page)).toBeVisible();
    });
  }
});
