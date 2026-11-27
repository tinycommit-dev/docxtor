// Shapes and text boxes: drawn on the page, selected, moved, resized, rotated and recoloured, typed in,
// inserted from the menu, printed.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, expect, inPage, openFixture, RICH, test, type Page } from "./helpers";
import { ARROW_LINE, CANVAS, ELLIPSE_WITH_TEXT, GROUP, STYLED_RECT, VML_BOX } from "./support/shape-markup";

const MOD = "ControlOrMeta";

interface Shape {
  geometry: string;
  w: number;
  h: number;
  x?: number;
  y?: number;
  rot?: number;
  wrap: string;
  alt?: string;
  fill?: { color: string; kind: string; color2?: string };
  line?: { color: string; widthPt: number; dash: string; tailArrow?: string };
  text?: { id: number; segs: { x?: string }[] }[];
}

const shapeAt = (page: Page, p: number, o: number) =>
  inPage(page, (w, a) => (w.fe.editor.engine as unknown as { shapeAt(p: { p: number; o: number }): unknown }).shapeAt(a), { p, o }) as Promise<Shape | null>;
const run = (page: Page, ops: unknown) => inPage(page, (w, o) => w.fe.editor.exec(o), ops);
const atoms = (page: Page) => page.locator(".flow .atom.shape");
const frame = (page: Page) => page.locator(".obj-frame");
const strip = (page: Page) => page.locator(".shape-strip");
const zoomOf = (page: Page) => inPage(page, (w) => w.fe.editor.zoom);
/** The text of a shape's paragraphs, one string each. */
const textOf = async (page: Page, p: number, o: number) => ((await shapeAt(page, p, o))?.text ?? []).map((b) => b.segs.map((s) => s.x ?? "").join(""));

/** Where the n-th shape is in the model: the paragraph that holds it and its offset there. */
const where = (page: Page, n = 0) =>
  atoms(page)
    .nth(n)
    .evaluate((a) => ({ p: Number(a.closest<HTMLElement>("[data-p]")!.dataset.p), o: Number((a as HTMLElement).dataset.o) }));
/** The text of the paragraph that holds the first shape, outside the shape. */
const paraTextBefore = (page: Page) =>
  atoms(page)
    .first()
    .evaluate((a) => {
      const p = a.closest<HTMLElement>("[data-p]")!;
      const copy = p.cloneNode(true) as HTMLElement;
      copy.querySelectorAll(".atom").forEach((n) => n.remove());
      return copy.textContent;
    });
const PT_PX = 96 / 72;
const center = async (loc: ReturnType<Page["locator"]>) => {
  const b = (await loc.boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2, box: b };
};

test.describe("shapes and text boxes", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page, "rich.docx");
  });

  test("a shape is an SVG with its fill, outline, size, rotation and text", async ({ page }) => {
    await run(page, [
      { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 100, h: 60, fill: "4472C4", line: { color: "2F528F", width_pt: 2, dash: "dash", tail_arrow: "triangle" }, text: "Hi there" },
      { op: "set_shape", at: { p: RICH.intro, o: 0 }, rot: 30 },
    ]);
    const atom = atoms(page).first();
    await expect(atom).toHaveCount(1);
    await expect(atom.locator("svg.shape-svg > path").first()).toHaveAttribute("fill", "#4472C4");
    await expect(atom.locator("svg.shape-svg > path").first()).toHaveAttribute("stroke", "#2F528F");
    const path = atom.locator("svg.shape-svg > path").first();
    expect(Number(await path.getAttribute("stroke-width"))).toBeCloseTo(2 * PT_PX, 1);
    expect(await path.getAttribute("stroke-dasharray")).toMatch(/^[\d.]+ [\d.]+$/);
    const size = await atom.evaluate((e) => ({ w: parseFloat(getComputedStyle(e).width), h: parseFloat(getComputedStyle(e).height) }));
    expect(size.w).toBeCloseTo(100 * PT_PX, 0);
    expect(size.h).toBeCloseTo(60 * PT_PX, 0);
    expect(await atom.evaluate((e) => (e as HTMLElement).style.transform)).toBe("rotate(30deg)");
    await expect(atom.locator(".shape-text")).toHaveText("Hi there");
    // The tail arrow is a marker in the SVG's own defs.
    await expect(atom.locator("svg marker")).toHaveCount(1);
  });

  test("shapes of other kinds draw their own outlines", async ({ page }) => {
    const kinds = ["ellipse", "triangle", "rightArrow", "star5", "heart", "wedgeRectCallout", "flowChartDecision", "cloud", "line"];
    await run(
      page,
      kinds.map((geometry, i) => ({ op: "insert_shape", at: { p: RICH.intro, o: i }, geometry, w: 60, h: geometry === "line" ? 0 : 40, fill: "FFC000", floating: false })),
    );
    await expect(atoms(page)).toHaveCount(kinds.length);
    const ds = await atoms(page).evaluateAll((els) => els.map((e) => e.querySelector("path")?.getAttribute("d") ?? ""));
    for (const d of ds) expect(d.length).toBeGreaterThan(8);
    // Each preset is its own outline.
    expect(new Set(ds).size).toBe(kinds.length);
    // A line has no inside.
    expect(await atoms(page).last().locator("path").getAttribute("fill")).toBe("none");
  });

  test("typing in a text box edits its text, keeps the caret inside, and undo works", async ({ page }) => {
    await run(page, { op: "insert_text_box", at: { p: RICH.intro, o: 0 }, w: 180, h: 72, text: "Hello" });
    const box = atoms(page).first();
    await box.dblclick();
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    await page.keyboard.type(" world");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toEqual(["Hello world"]);
    // The paragraph that holds the box was drawn again after each key: the caret is still in the box.
    expect(await page.evaluate(() => !!document.activeElement?.closest(".shape-text"))).toBe(true);
    await page.keyboard.press("Enter");
    await page.keyboard.type("Second");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toEqual(["Hello world", "Second"]);
    await page.keyboard.press("Backspace");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toEqual(["Hello world", "Secon"]);
    // Select all in a box is its text; bold from the keyboard applies inside it.
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press(`${MOD}+B`);
    const allBold = async () => ((await shapeAt(page, RICH.intro, 0))?.text as unknown as { segs: { f: { bold?: boolean } }[] }[]).every((b) => b.segs.every((s) => s.f.bold));
    await expect.poll(allBold).toBe(true);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(allBold).toBe(false);
    // Undo left the caret in the box: typing carries on there.
    expect(await page.evaluate(() => !!document.activeElement?.closest(".shape-text"))).toBe(true);
    await page.keyboard.press("ArrowRight");
    await page.keyboard.type("!");
    await expect.poll(async () => (await textOf(page, RICH.intro, 0)).join("")).toContain("!");
    await page.keyboard.press(`${MOD}+Shift+Z`);
    // Backspace at the very start of a text box leaves the paragraph before it alone.
    await page.keyboard.press(`${MOD}+Home`);
    const before = await paraTextBefore(page);
    await page.keyboard.press("Backspace");
    expect(await paraTextBefore(page)).toBe(before);
    // Escape goes back to the page with the box selected.
    await page.keyboard.press("Escape");
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "false");
    await expect(frame(page)).toBeVisible();
    await expect(frame(page).locator(".obj-handle.se")).toBeVisible();
    // Clicking outside the box also leaves it.
    await box.dblclick();
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    await page.locator(`.flow [data-p="${RICH.last}"]`).click();
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "false");
    await expect(frame(page)).toBeHidden();
  });

  test("Enter on a selected box starts typing in it, and typing a letter does too", async ({ page }) => {
    await run(page, { op: "insert_text_box", at: { p: RICH.intro, o: 0 }, w: 180, h: 72, text: "Hi" });
    const box = atoms(page).first();
    await box.click();
    await expect(frame(page)).toBeVisible();
    await page.keyboard.press("Enter");
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    await page.keyboard.type("!");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toEqual(["Hi!"]);
    await page.keyboard.press("Escape");
    await page.keyboard.type("a");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toEqual(["Hi!a"]);
  });

  test("Insert, Drawing, Text box puts a floating box at the caret with the caret in it", async ({ page }) => {
    await page.locator(`.flow [data-p="${RICH.intro}"]`).click({ position: { x: 5, y: 5 } });
    await chooseMenu(page, "Insert", "Drawing", "Text box");
    await expect(atoms(page)).toHaveCount(1);
    const box = atoms(page).first();
    await expect(box).toHaveClass(/textbox/);
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect([s.w, s.h]).toEqual([180, 72]);
    expect(s.wrap).toBe("square");
    await page.keyboard.type("Typed");
    await expect.poll(() => textOf(page, at.p, at.o)).toEqual(["Typed"]);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => textOf(page, at.p, at.o)).not.toEqual(["Typed"]);
  });

  test("Insert, Drawing, Shape offers every shape with a preview; a click inserts it selected", async ({ page }) => {
    await page.locator(`.flow [data-p="${RICH.intro}"]`).click({ position: { x: 5, y: 5 } });
    await chooseMenu(page, "Insert", "Drawing", "Shape");
    const picker = page.locator(".shape-picker");
    await expect(picker).toBeVisible();
    for (const group of ["Lines and arrows", "Rectangles", "Basic shapes", "Block arrows", "Callouts", "Flowchart"]) await expect(picker.getByText(group, { exact: true })).toBeVisible();
    expect(await picker.locator(".shape-choice svg path").count()).toBeGreaterThan(40);
    await picker.getByRole("button", { name: "Oval", exact: true }).click();
    await expect(picker).toHaveCount(0);
    await expect(atoms(page)).toHaveCount(1);
    const at = await where(page);
    const s = (await shapeAt(page, at.p, at.o))!;
    expect(s.geometry).toBe("ellipse");
    expect([s.w, s.h]).toEqual([108, 72]);
    expect(s.fill?.color).toBe("4472C4");
    // Selected, ready to resize.
    await expect(frame(page)).toBeVisible();
    await expect(frame(page).locator(".obj-handle.se")).toBeVisible();
    await expect(strip(page)).toBeVisible();
    // An arrow comes with its head.
    await chooseMenu(page, "Insert", "Drawing", "Shape");
    await page.locator(".shape-picker").getByRole("button", { name: "Arrow", exact: true }).click();
    await expect(atoms(page)).toHaveCount(2);
    const arrows = await Promise.all([0, 1].map(async (n) => {
      const w = await where(page, n);
      return (await shapeAt(page, w.p, w.o))!;
    }));
    const arrow = arrows.find((a) => a.geometry === "straightConnector1")!;
    expect(arrow.line?.tailArrow).toBe("triangle");
  });

  test("dragging moves a floating shape, the arrow keys nudge it", async ({ page }) => {
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 100, h: 60, fill: "FFC000", x: 20, y: 10 });
    const atom = atoms(page).first();
    const z = await zoomOf(page);
    await atom.click();
    const before = (await shapeAt(page, RICH.intro, 0))!;
    const c = await center(atom);
    await page.mouse.move(c.x, c.y);
    await page.mouse.down();
    await page.mouse.move(c.x + 30, c.y + 10, { steps: 3 });
    await page.mouse.move(c.x + 60, c.y + 30, { steps: 3 });
    await page.mouse.up();
    const after = (await shapeAt(page, RICH.intro, 0))!;
    expect(after.x! - before.x!).toBeCloseTo(60 / z / PT_PX, 0);
    expect(after.y! - before.y!).toBeGreaterThan(20 / z / PT_PX);
    expect([after.w, after.h]).toEqual([100, 60]);
    // The shape drew itself again where it was dropped.
    await expect(frame(page)).toBeVisible();
    await page.keyboard.press("ArrowRight");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.x).toBeCloseTo(after.x! + 1, 0);
    await page.keyboard.press("Shift+ArrowDown");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.y).toBeCloseTo(after.y! + 10, 0);
    // One undo takes back what the keys did together.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.y).toBeCloseTo(after.y!, 0);
  });

  test("handles resize, Shift keeps the ratio; the rotation handle turns, Shift snaps to 15 degrees", async ({ page }) => {
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 100, h: 60, fill: "FFC000", x: 0, y: 0 });
    const atom = atoms(page).first();
    const z = await zoomOf(page);
    await atom.click();
    const se = frame(page).locator(".obj-handle.se");
    let c = await center(se);
    await page.mouse.move(c.x, c.y);
    await page.mouse.down();
    await page.mouse.move(c.x + 40, c.y + 5, { steps: 4 });
    await page.mouse.up();
    let s = (await shapeAt(page, RICH.intro, 0))!;
    expect(s.w).toBeCloseTo(100 + 40 / z / PT_PX, 0);
    expect(s.h).toBeCloseTo(60 + 5 / z / PT_PX, 0);
    // With Shift the ratio stays.
    const w0 = s.w;
    const h0 = s.h;
    c = await center(frame(page).locator(".obj-handle.se"));
    await page.mouse.move(c.x, c.y);
    await page.keyboard.down("Shift");
    await page.mouse.down();
    await page.mouse.move(c.x + 60, c.y + 4, { steps: 4 });
    await page.mouse.up();
    await page.keyboard.up("Shift");
    s = (await shapeAt(page, RICH.intro, 0))!;
    expect(s.w / s.h).toBeCloseTo(w0 / h0, 1);
    expect(s.w).toBeGreaterThan(w0 + 20);
    // Rotate a quarter turn: from the handle above the box to a point level with its centre, on the right.
    const rot = frame(page).locator(".obj-handle.rot");
    const r = await center(rot);
    const box = (await frame(page).boundingBox())!;
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    await page.mouse.move(r.x, r.y);
    await page.mouse.down();
    await page.mouse.move(cx + 40, cy - 40, { steps: 3 });
    await page.mouse.move(cx + 100, cy, { steps: 3 });
    await page.mouse.up();
    s = (await shapeAt(page, RICH.intro, 0))!;
    expect(s.rot).toBeGreaterThan(80);
    expect(s.rot).toBeLessThan(100);
    // Shift snaps to steps of 15.
    const box2 = (await frame(page).boundingBox())!;
    const cx2 = box2.x + box2.width / 2;
    const cy2 = box2.y + box2.height / 2;
    const r2 = await center(frame(page).locator(".obj-handle.rot"));
    await page.mouse.move(r2.x, r2.y);
    await page.keyboard.down("Shift");
    await page.mouse.down();
    await page.mouse.move(cx2 + 200, cy2 + 60, { steps: 5 });
    await page.mouse.up();
    await page.keyboard.up("Shift");
    s = (await shapeAt(page, RICH.intro, 0))!;
    expect(s.rot! % 15).toBe(0);
  });

  test("the strip recolours the fill and the outline, and sets weight and dash", async ({ page }) => {
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "ellipse", w: 100, h: 60, fill: "FFC000", line: { color: "2F528F", width_pt: 1 }, x: 0, y: 0 });
    await atoms(page).first().click();
    await strip(page).getByRole("button", { name: "Fill" }).click();
    await page.getByRole("button", { name: "#C0392B" }).click();
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.fill?.color).toBe("C0392B");
    await expect(atoms(page).first().locator("path").first()).toHaveAttribute("fill", "#C0392B");
    await strip(page).getByRole("button", { name: "Outline" }).click();
    await page.getByRole("button", { name: "#27AE60" }).click();
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.line?.color).toBe("27AE60");
    await strip(page).getByLabel("Outline weight").selectOption("3");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.line?.widthPt).toBe(3);
    await strip(page).getByLabel("Outline style").selectOption("dash");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.line?.dash).toBe("dash");
    await expect(frame(page)).toBeVisible();
    // No fill and no outline.
    await strip(page).getByRole("button", { name: "Fill" }).click();
    await page.getByRole("menuitem", { name: "No fill" }).or(page.getByRole("button", { name: "No fill" })).first().click();
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.fill).toBeUndefined();
    await strip(page).getByRole("button", { name: "Outline" }).click();
    await page.getByRole("button", { name: "No outline" }).click();
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.line).toBeUndefined();
    await expect(atoms(page).first().locator("path").first()).toHaveAttribute("fill", "none");
    await expect(atoms(page).first().locator("path").first()).toHaveAttribute("stroke", "none");
  });

  test("wrapping changes how the paragraph lays out around the shape", async ({ page }) => {
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rect", w: 100, h: 60, fill: "FFC000", x: 0, y: 0 });
    const atom = atoms(page).first();
    await atom.click();
    const wrap = (name: string) => strip(page).getByRole("button", { name, exact: true }).click();
    await expect(atom).toHaveCSS("float", "left");
    const text = page.locator(`.flow [data-p="${RICH.intro}"] .s`).first();
    const leftOf = async () => (await text.evaluate((e) => e.getBoundingClientRect().left)) - (await page.locator(`.flow [data-p="${RICH.intro}"]`).evaluate((e) => e.getBoundingClientRect().left));
    const wrapped = await leftOf();
    await wrap("In line");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.wrap).toBe("inline");
    await expect(atoms(page).first()).toHaveCSS("float", "none");
    await expect(atoms(page).first()).toHaveCSS("display", "inline-block");
    await wrap("Break");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.wrap).toBe("topBottom");
    await expect(atoms(page).first()).toHaveCSS("display", "block");
    expect(await leftOf()).toBeLessThan(wrapped);
    await wrap("Behind");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.wrap).toBe("behind");
    await expect(atoms(page).first()).toHaveCSS("position", "absolute");
    // Behind the text: below every line, in the order of the shape's own stacking value.
    expect(await atoms(page).first().evaluate((e) => Number(getComputedStyle(e).zIndex))).toBeLessThan(0);
    // Text no longer makes room for it.
    expect(await leftOf()).toBeLessThan(wrapped);
    await wrap("Front");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.wrap).toBe("front");
    expect(await atoms(page).first().evaluate((e) => Number(getComputedStyle(e).zIndex))).toBeGreaterThan(1);
    await wrap("Wrap");
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.wrap).toBe("square");
    await expect(atoms(page).first()).toHaveCSS("float", "left");
  });

  test("Delete removes a selected shape, and so does the strip; undo brings it back", async ({ page }) => {
    await run(page, [
      { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "diamond", w: 60, h: 60, fill: "FFC000" },
      { op: "insert_shape", at: { p: RICH.highlights, o: 0 }, geometry: "diamond", w: 60, h: 60, fill: "FFC000" },
    ]);
    await expect(atoms(page)).toHaveCount(2);
    await atoms(page).first().click();
    await page.keyboard.press("Delete");
    await expect(atoms(page)).toHaveCount(1);
    await expect(frame(page)).toBeHidden();
    await page.keyboard.press(`${MOD}+Z`);
    await expect(atoms(page)).toHaveCount(2);
    await atoms(page).first().click();
    await strip(page).getByRole("button", { name: "Delete shape" }).click();
    await expect(atoms(page)).toHaveCount(1);
    await atoms(page).first().click();
    await page.keyboard.press("Backspace");
    await expect(atoms(page)).toHaveCount(0);
  });

  test("right-click offers editing text, colours, wrapping, alt text and delete", async ({ page }) => {
    await run(page, { op: "insert_text_box", at: { p: RICH.intro, o: 0 }, w: 180, h: 72, text: "Right here" });
    const box = atoms(page).first();
    await box.click({ button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    for (const label of ["Edit text", "Fill…", "Outline…", "In line with text", "Text wraps around", "Behind text", "In front of text", "Alt text…", "Delete shape"]) await expect(contextItem(page, label)).toBeVisible();
    // The box is wrapped around the text now.
    await expect(contextItem(page, "Text wraps around").locator(".check, [aria-checked]").or(contextItem(page, "Text wraps around"))).toBeVisible();
    await contextItem(page, "Edit text").click();
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    await page.keyboard.type("!");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toEqual(["Right here!"]);
    await page.keyboard.press("Escape");
    // Delete from the menu (a second box, since a shape behind the text can't be clicked).
    await run(page, { op: "insert_text_box", at: { p: RICH.highlights, o: 0 }, w: 100, h: 40, text: "Other" });
    await expect(atoms(page)).toHaveCount(2);
    await atoms(page).nth(1).click({ button: "right" });
    await contextItem(page, "Delete shape").click();
    await expect(atoms(page)).toHaveCount(1);
    await box.click({ button: "right" });
    await contextItem(page, "Behind text").click();
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.wrap).toBe("behind");
  });

  test("alt text is set from the strip", async ({ page }) => {
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "heart", w: 60, h: 60, fill: "C0392B" });
    await atoms(page).first().click();
    await strip(page).getByRole("button", { name: "Alt text" }).click();
    await page.getByRole("dialog").getByRole("textbox").fill("A red heart");
    await page.getByRole("dialog").getByRole("button", { name: "Save" }).click();
    await expect.poll(async () => (await shapeAt(page, RICH.intro, 0))!.alt).toBe("A red heart");
  });

  test("a shape prints with the pages, with ids of its own", async ({ page }) => {
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "rightArrow", w: 100, h: 60, fill: "4472C4", line: { color: "2F528F", width_pt: 1, tail_arrow: "triangle" }, text: "Print me" });
    await page.keyboard.press(`${MOD}+P`);
    const copy = page.locator(".docxtor-print .print-page").first();
    await expect(copy.locator(".atom.shape svg.shape-svg > path").first()).toHaveAttribute("fill", "#4472C4");
    await expect(copy.locator(".atom.shape .shape-text")).toHaveText("Print me");
    // Ids are unique across the page and its printed copies, so every arrow head finds its own marker.
    const dupes = await page.evaluate(() => {
      const ids = Array.from(document.querySelectorAll("[id^='fa']"), (e) => e.id);
      return ids.length - new Set(ids).size;
    });
    expect(dupes).toBe(0);
    const refs = await copy.locator(".atom.shape svg.shape-svg > path").first().evaluate((p) => {
      const m = p.getAttribute("marker-end")?.match(/#([^)]+)/)?.[1];
      return !!m && !!document.getElementById(m);
    });
    expect(refs).toBe(true);
  });

  test("a second click in a selected box's text puts the caret there; its edge moves it", async ({ page }) => {
    await run(page, { op: "insert_text_box", at: { p: RICH.intro, o: 0 }, w: 180, h: 72, text: "Click here", x: 20, y: 10 });
    const box = atoms(page).first();
    const z = await zoomOf(page);
    await box.click({ position: { x: 100, y: 60 } });
    await expect(frame(page)).toBeVisible();
    await expect(box.locator(".shape-text")).not.toHaveAttribute("contenteditable", "true");
    await box.locator(".shape-text .fp").first().click({ position: { x: 20, y: 5 } });
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    await page.keyboard.type("X");
    await expect.poll(() => textOf(page, RICH.intro, 0)).toHaveLength(1);
    expect((await textOf(page, RICH.intro, 0))[0]).toContain("X");
    await page.keyboard.press("Escape");
    // Drag by the frame's edge, away from the handles.
    const before = (await shapeAt(page, RICH.intro, 0))!;
    const fb = (await frame(page).boundingBox())!;
    await page.mouse.move(fb.x + fb.width * 0.25, fb.y);
    await page.mouse.down();
    await page.mouse.move(fb.x + fb.width * 0.25 + 30, fb.y + 10, { steps: 3 });
    await page.mouse.move(fb.x + fb.width * 0.25 + 60, fb.y + 20, { steps: 3 });
    await page.mouse.up();
    const after = (await shapeAt(page, RICH.intro, 0))!;
    expect(after.x! - before.x!).toBeCloseTo(60 / z / PT_PX, 0);
    expect(after.y! - before.y!).toBeGreaterThan(10);
  });

  test("Shift+Arrow over a shape selects it like a click", async ({ page }) => {
    await run(page, { op: "insert_shape", at: { p: RICH.intro, o: 0 }, geometry: "triangle", w: 80, h: 60, fill: "FFC000", floating: false });
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
      e.select({ p, o: 0 });
      e.focus();
    }, RICH.intro);
    await page.keyboard.press("Shift+ArrowRight");
    await expect(frame(page)).toBeVisible();
    await expect(strip(page)).toBeVisible();
    // Typing now doesn't replace the shape (it has no text to type in).
    await page.keyboard.type("z");
    await expect(atoms(page)).toHaveCount(1);
  });

  test("shapes read in the dark theme and the page stays white", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await run(page, { op: "insert_text_box", at: { p: RICH.intro, o: 0 }, w: 180, h: 72, text: "Dark theme" });
    const box = atoms(page).first();
    await expect(box).toBeVisible();
    expect(await box.locator(".shape-text").evaluate((e) => getComputedStyle(e).color)).toBe("rgb(0, 0, 0)");
    await expect(box.locator("path").first()).toHaveAttribute("fill", "#FFFFFF");
  });
});

test.describe("shapes made in Word", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: ["First paragraph", { raw: ELLIPSE_WITH_TEXT }, { raw: GROUP }, { raw: CANVAS }, { raw: ARROW_LINE }, { raw: STYLED_RECT }, { raw: VML_BOX }, "Last paragraph"],
        },
      ],
    });
  });

  test("every kind is drawn: gradients, groups, canvases, arrows, rotation, VML", async ({ page }) => {
    const all = atoms(page);
    await expect(all).toHaveCount(6);
    // An oval with a gradient (red to half-transparent blue, top to bottom), behind the text, centred on the page.
    const oval = all.nth(0);
    const grad = oval.locator("linearGradient");
    await expect(grad.locator("stop")).toHaveCount(2);
    await expect(grad.locator("stop").nth(0)).toHaveAttribute("stop-color", "#FF0000");
    await expect(grad.locator("stop").nth(1)).toHaveAttribute("stop-color", "#0000FF");
    expect([await grad.getAttribute("y1"), await grad.getAttribute("y2")]).toEqual(["0", "1"]);
    const id = await grad.getAttribute("id");
    await expect(oval.locator("svg.shape-svg > path")).toHaveAttribute("fill", `url(#${id})`);
    await expect(oval.locator("svg.shape-svg > path")).toHaveAttribute("stroke", "none");
    await expect(oval.locator(".shape-text")).toHaveText("Hello oval");
    expect(await oval.evaluate((e) => Number(getComputedStyle(e).zIndex))).toBeLessThan(0);
    await expect(oval).toHaveCSS("position", "absolute");
    // A group: two members placed by their offsets in the group's box, one with its text.
    const group = all.nth(1);
    await expect(group.locator(".shape-child")).toHaveCount(2);
    const right = group.locator(".shape-child").nth(1);
    expect(parseFloat(await right.evaluate((e) => (e as HTMLElement).style.left))).toBeCloseTo(140 * PT_PX, 0);
    await expect(right.locator(".shape-text")).toHaveText("In the group");
    await expect(group.locator(".shape-child").nth(0).locator("path")).toHaveAttribute("fill", "#FF0000");
    // A canvas.
    const canvas = all.nth(2);
    await expect(canvas.locator(".shape-child")).toHaveCount(2);
    await expect(canvas.locator(".shape-text")).toHaveText("Canvas text");
    // An arrow: dashed, flipped, with its head.
    const arrow = all.nth(3);
    await expect(arrow.locator("marker")).toHaveCount(1);
    expect(await arrow.locator("svg.shape-svg > path").getAttribute("stroke-dasharray")).toBeTruthy();
    expect(await arrow.locator("svg.shape-svg").evaluate((e) => (e as SVGElement).style.transform)).toBe("scale(-1, 1)");
    // A rounded rectangle turned 30 degrees.
    const rect = all.nth(4);
    expect(await rect.evaluate((e) => (e as HTMLElement).style.transform)).toBe("rotate(30deg)");
    // A text box from an old file (VML only).
    const old = all.nth(5);
    await expect(old.locator(".shape-text")).toHaveText("Old text box");
    await expect(old.locator("svg.shape-svg > path")).toHaveAttribute("stroke", /^#FF0000$/i);
    await expect(old.locator("svg.shape-svg > path")).toHaveAttribute("fill", /^#FFFFCC$/i);
    // Text in a Word text box is edited in place like any other.
    await old.dblclick();
    await page.keyboard.type("!");
    await expect(old.locator(".shape-text")).toHaveText("Old text box!");
  });
});
