// Zoom: presets, a slider, the page width, a whole page, several pages side by side, Ctrl and the wheel, and pinches.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, expect, test, type Page } from "./helpers";
import { LONG_OUTLINE_DOC, onFe, openDoc } from "./support/view-helpers";

const zoom = (page: Page) => onFe(page, (fe) => fe.editor.zoom);
const mode = (page: Page) => onFe(page, (fe) => fe.editor.zoomer.mode);
const chooseZoom = (page: Page, item: string, sub?: string) => (sub ? chooseMenu(page, "View", "Zoom", sub, item) : chooseMenu(page, "View", "Zoom", item));
const slider = (page: Page) => page.locator(".zoom-slider");
const readout = (page: Page) => page.locator(".zoom-readout");
/** The room the pages have, as the editor measures it: the canvas without its padding. */
const room = (page: Page) =>
  page.evaluate(() => {
    const c = document.querySelector<HTMLElement>(".canvas")!;
    const cs = getComputedStyle(c);
    return { w: c.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight), h: c.clientHeight };
  });
const sheetSize = (page: Page) => page.evaluate(() => ({ w: parseFloat(document.querySelector<HTMLElement>(".sheet")!.style.width), h: parseFloat(document.querySelector<HTMLElement>(".sheet")!.style.height) }));

test.describe("zoom presets and the status bar", () => {
  test.beforeEach(async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
  });

  test("the status bar has zoom out, a slider, zoom in and the factor; the automatic fit is the start", async ({ page }) => {
    await expect(slider(page)).toBeVisible();
    await expect(page.getByRole("button", { name: "Zoom out" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Zoom in" })).toBeVisible();
    expect(await mode(page)).toBe("auto");
    const z = await zoom(page);
    expect(z).toBeLessThanOrEqual(1);
    await expect(readout(page)).toHaveText(`${Math.round(z * 100)}%`);
    await expect(slider(page)).toHaveValue(String(Math.round(z * 100)));
  });

  test("View, Zoom has presets that set a fixed factor, and the slider and the readout follow", async ({ page }) => {
    for (const [label, z] of [["150%", 1.5], ["50%", 0.5], ["75%", 0.75], ["125%", 1.25], ["200%", 2]] as const) {
      await chooseZoom(page, label);
      expect(await zoom(page)).toBe(z);
      expect(await mode(page)).toBe("custom");
      expect(await page.locator(".zoom-view").evaluate((el) => (el as HTMLElement).style.transform)).toBe(z === 1 ? "" : `scale(${z})`);
      await expect(readout(page)).toHaveText(label);
      await expect(slider(page)).toHaveValue(String(z * 100));
    }
    await chooseZoom(page, "Actual size (100%)");
    expect(await zoom(page)).toBe(1);
    // The pages really are that size on screen.
    const sheet = (await page.locator(".sheet").first().boundingBox())!;
    expect(sheet.width).toBeCloseTo(816, 0);
  });

  test("the slider zooms from 10% to 500% in steps of five", async ({ page }) => {
    await slider(page).fill("150");
    expect(await zoom(page)).toBe(1.5);
    await slider(page).fill("10");
    expect(await zoom(page)).toBe(0.1);
    await slider(page).fill("500");
    expect(await zoom(page)).toBe(5);
    await slider(page).fill("100");
    await slider(page).focus();
    await page.keyboard.press("ArrowRight");
    expect(await zoom(page)).toBe(1.05);
  });

  test("Zoom in and Zoom out step by ten points, with the keys too", async ({ page }) => {
    await chooseZoom(page, "Actual size (100%)");
    await page.getByRole("button", { name: "Zoom in" }).click();
    expect(await zoom(page)).toBe(1.1);
    await page.getByRole("button", { name: "Zoom out" }).click();
    await page.getByRole("button", { name: "Zoom out" }).click();
    expect(await zoom(page)).toBe(0.9);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("ControlOrMeta+Alt+=");
    expect(await zoom(page)).toBe(1);
    await page.keyboard.press("ControlOrMeta+Alt+-");
    expect(await zoom(page)).toBe(0.9);
  });

  test("the factor opens the presets, with the one in use checked", async ({ page }) => {
    await chooseZoom(page, "125%");
    await readout(page).click();
    const menu = page.locator(".zoom-menu");
    await expect(menu).toBeVisible();
    await expect(menu.getByRole("menuitemradio", { name: "125%", exact: true })).toHaveAttribute("aria-checked", "true");
    await expect(menu.getByRole("menuitemradio", { name: "150%", exact: true })).toHaveAttribute("aria-checked", "false");
    for (const name of ["50%", "Actual size (100%)", "Page width", "Whole page", "2 pages across", "3 pages across", "4 pages across", "Custom zoom…"]) await expect(menu.getByRole("menuitemradio", { name, exact: true })).toBeVisible();
    await menu.getByRole("menuitemradio", { name: "75%", exact: true }).click();
    await expect(menu).toHaveCount(0);
    expect(await zoom(page)).toBe(0.75);
  });

  test("Custom zoom asks for a percentage", async ({ page }) => {
    await chooseZoom(page, "Custom zoom…");
    const field = page.getByRole("textbox", { name: /Zoom/ });
    await expect(field).toBeVisible();
    await field.fill("80%");
    await page.keyboard.press("Enter");
    expect(await zoom(page)).toBe(0.8);
  });

  test("keeping the point under the middle of the window still when the zoom changes", async ({ page }) => {
    await chooseZoom(page, "Actual size (100%)");
    await page.evaluate(() => (document.querySelector(".canvas") as HTMLElement).scrollTo({ top: 1500 }));
    const at = () =>
      page.evaluate(() => {
        const c = document.querySelector<HTMLElement>(".canvas")!.getBoundingClientRect();
        const el = document.elementFromPoint(c.left + c.width / 2, c.top + c.height / 2)?.closest<HTMLElement>(".fp");
        return el?.textContent ?? "";
      });
    const before = await at();
    expect(before).toMatch(/\w/);
    await chooseZoom(page, "150%");
    expect(await at()).toBe(before);
  });
});

test.describe("page width, a whole page and several pages", () => {
  test.beforeEach(async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
  });

  test("Page width fills the window, even above 100%, and follows the window as it changes", async ({ page }) => {
    await page.setViewportSize({ width: 1700, height: 900 });
    await chooseZoom(page, "Page width");
    expect(await mode(page)).toBe("width");
    const { w } = await room(page);
    const sheet = await sheetSize(page);
    const z = await zoom(page);
    expect(z).toBeGreaterThan(1);
    // As wide as the room allows, to a whole percent.
    expect(sheet.w * z).toBeLessThanOrEqual(w + 0.5);
    expect(sheet.w * (z + 0.02)).toBeGreaterThan(w);
    await page.setViewportSize({ width: 1100, height: 900 });
    await expect.poll(() => zoom(page)).toBeLessThan(z);
    expect(await mode(page)).toBe("width");
  });

  test("Whole page fits the page in the window, by width or by height", async ({ page }) => {
    await chooseZoom(page, "Whole page");
    expect(await mode(page)).toBe("page");
    const { w, h } = await room(page);
    const sheet = await sheetSize(page);
    const z = await zoom(page);
    const ruler = await page.locator(".ruler").evaluate((el) => (el as HTMLElement).offsetHeight);
    expect((sheet.h + ruler) * z).toBeLessThanOrEqual(h + 1);
    expect(sheet.w * z).toBeLessThanOrEqual(w + 1);
    // Any bigger and one of them would not fit.
    const bigger = Math.min(w / sheet.w, h / (sheet.h + ruler));
    expect(z).toBeGreaterThan(bigger - 0.03);
    // The window gets shorter: the page shrinks with it.
    await page.setViewportSize({ width: 1280, height: 600 });
    await expect.poll(() => zoom(page)).toBeLessThan(z);
  });

  test("Several pages shows whole pages side by side, two to a row, drawn from the layout", async ({ page }) => {
    const pages = await onFe(page, (fe) => fe.editor.pages);
    expect(pages).toBeGreaterThanOrEqual(4);
    await chooseMenu(page, "View", "Zoom", "Several pages", "2 pages across");
    expect(await mode(page)).toBe("pages");
    const cells = page.locator(".pg-cell");
    await expect(cells).toHaveCount(pages);
    // The grid gives its cells their size in the frame after it makes them (on a busy machine the first look can find them 0 wide).
    await expect.poll(() => cells.first().evaluate((el) => el.getBoundingClientRect().width)).toBeGreaterThan(0);
    const boxes = await cells.evaluateAll((els) => els.map((el) => el.getBoundingClientRect()).map((r) => ({ x: r.x, y: r.y, w: r.width, h: r.height })));
    // Two in the first row, the next two in the row below.
    expect(Math.abs(boxes[0].y - boxes[1].y)).toBeLessThan(1);
    expect(boxes[2].y).toBeGreaterThan(boxes[0].y + boxes[0].h - 1);
    expect(boxes[1].x).toBeGreaterThan(boxes[0].x + boxes[0].w - 1);
    expect(Math.abs(boxes[2].y - boxes[3].y)).toBeLessThan(1);
    const z = await zoom(page);
    expect(boxes[0].w).toBeCloseTo(816 * z, 0);
    expect(boxes[0].h).toBeCloseTo(1056 * z, 0);
    // The pages in view carry their text, at the page's own place.
    await expect(cells.first().locator(".pg-snap")).toHaveCount(1);
    await expect(cells.first().locator(".pg-snap .fp").first()).toHaveText("Intro text");
    const inner = await cells.first().locator(".pg-snap .fp").first().evaluate((el) => {
      const s = el.closest<HTMLElement>(".pg-snap")!.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      return { x: (r.left - s.left) / s.width, y: (r.top - s.top) / s.height };
    });
    // One inch in from the left of an 8.5 inch page, and one inch down from the top.
    expect(inner.x).toBeCloseTo(96 / 816, 1);
    expect(inner.y).toBeCloseTo(96 / 1056, 1);
    // The copies are inert: no ids, no editing.
    expect(await cells.first().locator("[data-p], [contenteditable]").count()).toBe(0);
    // A read-only look: typing changes nothing.
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(true);
    await page.screenshot({ path: "test-results/zoom-two-pages.png" });
  });

  // The text is hidden while the grid shows, so the keyboard was on the page body, where no key did anything.
  test("the keyboard is on a page of the grid: the arrows go from page to page, Enter goes back to the text at that page", async ({ page }) => {
    const pages = await onFe(page, (fe) => fe.editor.pages);
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "View", "Zoom", "Several pages", "2 pages across");
    const cells = page.locator(".pg-cell");
    await expect(cells.nth(0)).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(cells.nth(1)).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(cells.nth(3)).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await expect(cells.nth(2)).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(cells.nth(0)).toBeFocused();
    await page.keyboard.press("End");
    await expect(cells.nth(pages - 1)).toBeFocused();
    await page.keyboard.press("Home");
    await expect(cells.nth(0)).toBeFocused();
    // Asked for the text's keyboard while the grid shows (a menu that closes gives it back this way), a page of the grid has it.
    await page.keyboard.press("ArrowRight");
    await onFe(page, (fe) => fe.editor.focus());
    await expect(cells.nth(1)).toBeFocused();
    // Enter: the text again, at the start of page 2, with the keyboard.
    await page.keyboard.press("Enter");
    expect(await mode(page)).toBe("width");
    await expect(page.locator(".flow")).toBeFocused();
    const at = await onFe(page, (fe) => fe.editor.sel!.focus.p);
    const first = await onFe(page, (fe) => fe.editor.firstParagraphOnPage(1));
    expect(at).toBe(first);
    // Leaving the grid another way (a view of its own) gives the keyboard back to the text too.
    await chooseMenu(page, "View", "Zoom", "Several pages", "2 pages across");
    // The caret's page has it (page 2, where Enter went), whatever page the scroll has settled at.
    await expect(cells.nth(1)).toBeFocused();
    await chooseMenu(page, "View", "Layout", "Pageless");
    await expect(page.locator(".flow")).toBeFocused();
  });

  test("three or four pages across make the pages smaller, and the grid follows the document", async ({ page }) => {
    await chooseMenu(page, "View", "Zoom", "Several pages", "2 pages across");
    const two = await zoom(page);
    await chooseMenu(page, "View", "Zoom", "Several pages", "4 pages across");
    const four = await zoom(page);
    expect(four).toBeLessThan(two);
    const tops = await page.locator(".pg-cell").evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().top)));
    expect(new Set(tops).size).toBe(1);
  });

  test("clicking a page goes back to the text at that page", async ({ page }) => {
    await chooseMenu(page, "View", "Zoom", "Several pages", "2 pages across");
    await page.locator(".pg-cell").nth(2).click();
    await expect(page.locator(".pg-cell")).toHaveCount(0);
    expect(await mode(page)).toBe("width");
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(false);
    // The view scrolls to the third page (smoothly), and the caret is in its first line.
    await expect.poll(() => onFe(page, (fe) => (fe.editor as unknown as { currentPage(): number }).currentPage())).toBe(3);
    const caret = await onFe(page, (fe) => fe.editor.sel!.focus.p);
    expect((await onFe(page, (fe) => (fe.editor as unknown as { pageMap(): Record<string, number> }).pageMap()))[String(caret)]).toBe(3);
  });

  test("leaving the grid by any other zoom brings the text back and unlocks it", async ({ page }) => {
    await chooseMenu(page, "View", "Zoom", "Several pages", "3 pages across");
    await expect(page.locator(".page-grid")).toBeVisible();
    await chooseZoom(page, "100%".replace("100%", "Actual size (100%)"));
    await expect(page.locator(".page-grid")).toBeHidden();
    await expect(page.locator(".flow").first()).toBeVisible();
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(false);
    expect(await zoom(page)).toBe(1);
  });

  test("a document that was read-only stays read-only after the grid", async ({ page }) => {
    await onFe(page, (fe) => (fe as unknown as { setReadOnly(on: boolean): void }).setReadOnly(true));
    await chooseMenu(page, "View", "Zoom", "Several pages", "2 pages across");
    await chooseZoom(page, "Page width");
    expect(await onFe(page, (fe) => fe.readOnly)).toBe(true);
  });
});

test.describe("Ctrl and the wheel, and pinches", () => {
  test.beforeEach(async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    await chooseZoom(page, "Actual size (100%)");
  });

  test("Ctrl with the wheel zooms ten points a notch, and the page stays put under the pointer", async ({ page }) => {
    await page.evaluate(() => (document.querySelector(".canvas") as HTMLElement).scrollTo({ top: 800 }));
    // The middle of a paragraph that is on screen.
    const { x, y } = await page.evaluate(() => {
      const c = document.querySelector<HTMLElement>(".canvas")!.getBoundingClientRect();
      const fp = Array.from(document.querySelectorAll<HTMLElement>(".flow .fp")).find((el) => el.getBoundingClientRect().top > c.top + c.height / 2)!;
      const r = fp.getBoundingClientRect();
      return { x: r.left + 20, y: r.top + r.height / 2 };
    });
    await page.mouse.move(x, y);
    const under = () => page.evaluate(([px, py]) => document.elementFromPoint(px, py)?.closest<HTMLElement>(".fp")?.textContent ?? "", [x, y]);
    const before = await under();
    expect(before).toMatch(/\w/);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, -100);
    await page.keyboard.up("Control");
    expect(await zoom(page)).toBe(1.1);
    expect(await mode(page)).toBe("custom");
    expect(await under()).toBe(before);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, 100);
    await page.mouse.wheel(0, 100);
    await page.keyboard.up("Control");
    expect(await zoom(page)).toBe(0.9);
  });

  test("the wheel alone scrolls and does not zoom", async ({ page }) => {
    const top = await page.evaluate(() => document.querySelector(".canvas")!.scrollTop);
    await page.mouse.move(640, 400);
    await page.mouse.wheel(0, 300);
    await expect.poll(() => page.evaluate(() => document.querySelector(".canvas")!.scrollTop)).toBeGreaterThan(top);
    expect(await zoom(page)).toBe(1);
  });

  test("a trackpad pinch (small deltas with Ctrl) zooms smoothly", async ({ page }) => {
    const box = (await page.locator(".canvas").boundingBox())!;
    await page.evaluate(
      ([x, y]) => {
        const canvas = document.querySelector(".canvas")!;
        for (let i = 0; i < 12; i++) canvas.dispatchEvent(new WheelEvent("wheel", { deltaY: -4, ctrlKey: true, clientX: x, clientY: y, bubbles: true, cancelable: true }));
      },
      [box.x + 300, box.y + 200],
    );
    const z = await zoom(page);
    expect(z).toBeGreaterThan(1.3);
    expect(Math.round(z * 100) % 10).not.toBe(0);
    // Pinching in zooms out again, to the floor at most.
    await page.evaluate(() => {
      const canvas = document.querySelector(".canvas")!;
      for (let i = 0; i < 40; i++) canvas.dispatchEvent(new WheelEvent("wheel", { deltaY: 10, ctrlKey: true, bubbles: true, cancelable: true }));
    });
    expect(await zoom(page)).toBeGreaterThanOrEqual(0.1);
    expect(await zoom(page)).toBeLessThan(0.5);
  });

  test("two fingers on a touch screen pinch the page", async ({ page }) => {
    const box = (await page.locator(".canvas").boundingBox())!;
    const cx = box.x + box.width / 2;
    const cy = box.y + 300;
    await page.evaluate(
      ([x, y]) => {
        const canvas = document.querySelector(".canvas")!;
        const fire = (type: string, id: number, px: number) => canvas.dispatchEvent(new PointerEvent(type, { pointerId: id, pointerType: "touch", clientX: px, clientY: y, bubbles: true, cancelable: true }));
        fire("pointerdown", 1, x - 50);
        fire("pointerdown", 2, x + 50);
        // The fingers move twice as far apart.
        for (let i = 1; i <= 5; i++) {
          fire("pointermove", 1, x - 50 - i * 10);
          fire("pointermove", 2, x + 50 + i * 10);
        }
        fire("pointerup", 1, x - 100);
        fire("pointerup", 2, x + 100);
      },
      [cx, cy],
    );
    expect(await zoom(page)).toBe(2);
    expect(await mode(page)).toBe("custom");
    // One finger does nothing to the zoom.
    await page.evaluate(([x, y]) => {
      const canvas = document.querySelector(".canvas")!;
      canvas.dispatchEvent(new PointerEvent("pointerdown", { pointerId: 9, pointerType: "touch", clientX: x, clientY: y, bubbles: true }));
      canvas.dispatchEvent(new PointerEvent("pointermove", { pointerId: 9, pointerType: "touch", clientX: x + 80, clientY: y, bubbles: true }));
      canvas.dispatchEvent(new PointerEvent("pointerup", { pointerId: 9, pointerType: "touch", clientX: x + 80, clientY: y, bubbles: true }));
    }, [cx, cy]);
    expect(await zoom(page)).toBe(2);
  });

  test("resizing the window with a mouse and keyboard leaves the view where it was scrolled", async ({ page }) => {
    await openDoc(page, LONG_OUTLINE_DOC);
    // The caret far down the document, then the view scrolled back to the top (the editor keeps the keyboard).
    await page.locator(".flow [data-p]").last().click();
    const canvas = page.locator(".canvas").first();
    await canvas.evaluate((c) => (c.scrollTop = 0));
    await page.setViewportSize({ width: 1100, height: 600 });
    await page.waitForTimeout(300);
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.waitForTimeout(300);
    expect(await canvas.evaluate((c) => c.scrollTop)).toBeLessThan(50);
  });
});

test.describe("the pages after a zoom", () => {
  const words = "the quick brown fox jumps over the lazy dog and keeps on running through the field".split(" ");
  const paragraphs = Array.from({ length: 1200 }, (_, i) => `Paragraph ${i}: ${[...words, ...words, ...words].map((_, k) => words[(k + i) % words.length]).join(" ")}.`);

  /** The page count, where each page starts (the first block on it) and the paginator's pass counts. */
  const map = (page: Page) =>
    page.evaluate(() => {
      const ed = (window as unknown as { docxtor: { fe: { editor: { pages: number; paginator: { firstOnPage: HTMLElement[]; stats: { fullPasses: number; incPasses: number } } } } } }).docxtor.fe.editor;
      return { pages: ed.pages, firsts: ed.paginator.firstOnPage.map((el) => el.dataset.id).join(","), passes: ed.paginator.stats.fullPasses + ed.paginator.stats.incPasses };
    });

  test("a zoom changes the picture only: the pages break where they do at 100% and no layout runs", async ({ page }) => {
    // Chrome lays text out again at another CSS zoom and rounds its lines differently (a novel had 1,164 pages at 100% and 1,160 at others);
    // Word's pages do not depend on the zoom. The pages are laid out at 100% and scaled as a picture.
    await openBuilt(page, { sections: [{ paragraphs }] });
    await onFe(page, (fe) => fe.editor.zoomer.set(1));
    await page.waitForTimeout(600);
    const at100 = await map(page);
    expect(at100.pages).toBeGreaterThan(50);
    for (const z of [0.5, 0.67, 0.87, 1.1, 1.33, 1.75, 2]) {
      await onFe(page, (fe, z) => fe.editor.zoomer.set(z!), z);
      // Longer than the pause before a layout, so a layout that a zoom asked for would have run.
      await page.waitForTimeout(400);
      const now = await map(page);
      expect(now.pages, `pages at ${z}`).toBe(at100.pages);
      expect(now.firsts, `page map at ${z}`).toBe(at100.firsts);
      expect(now.passes, `layout passes after zooming to ${z}`).toBe(at100.passes);
    }
  });

  test("the scroll extent follows the zoom: the pages' height times the factor", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: paragraphs.slice(0, 200) }] });
    for (const z of [0.5, 1, 1.5, 2]) {
      await onFe(page, (fe, z) => fe.editor.zoomer.set(z!), z);
      const r = await page.evaluate(() => {
        const stage = document.querySelector<HTMLElement>(".stage")!;
        const box = stage.getBoundingClientRect();
        return { h: box.height, laid: stage.offsetHeight, fit: stage.parentElement!.parentElement!.getBoundingClientRect().height };
      });
      expect(r.h).toBeCloseTo(r.laid * z, 0);
      expect(Math.abs(r.fit - r.h)).toBeLessThan(1);
    }
  });

  test("at 150% a click puts the caret where it was clicked", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: paragraphs.slice(0, 40) }] });
    await onFe(page, (fe) => fe.editor.zoomer.set(1.5));
    await page.waitForTimeout(200);
    const target = await page.evaluate(() => {
      const para = document.querySelectorAll<HTMLElement>(".flow .fp")[3];
      const node = document.createTreeWalker(para, NodeFilter.SHOW_TEXT).nextNode() as Text;
      const range = document.createRange();
      range.setStart(node, 20);
      range.setEnd(node, 21);
      const r = range.getBoundingClientRect();
      return { x: r.left + 1, y: r.top + r.height / 2, p: Number(para.dataset.p) };
    });
    await page.mouse.click(target.x, target.y);
    const at = await onFe(page, (fe) => fe.editor.sel!.focus);
    expect(at.p).toBe(target.p);
    expect(at.o).toBe(20);
  });

  test("at 150% a menu opens beside the pointer", async ({ page }) => {
    // Tall enough for the whole menu, which is moved up when it does not fit below the pointer.
    await page.setViewportSize({ width: 1280, height: 1400 });
    await openBuilt(page, { sections: [{ paragraphs: paragraphs.slice(0, 40) }] });
    await onFe(page, (fe) => fe.editor.zoomer.set(1.5));
    await page.waitForTimeout(200);
    const box = (await page.locator(".flow .fp").first().boundingBox())!;
    const at = { x: box.x + 120, y: box.y + 8 };
    await page.mouse.click(at.x, at.y, { button: "right" });
    const menu = page.locator(".pop:not(.menu-drop)").last();
    await expect(menu).toBeVisible();
    const m = (await menu.boundingBox())!;
    expect(Math.abs(m.x - at.x)).toBeLessThan(40);
    expect(m.y).toBeGreaterThan(at.y - 40);
    expect(m.y).toBeLessThan(at.y + 40);
  });

  test("the ruler stays at the top of the window as the pages scroll, at any zoom", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs }] });
    for (const z of [0.5, 1.5]) {
      await onFe(page, (fe, z) => fe.editor.zoomer.set(z!), z);
      await page.evaluate(() => (document.querySelector(".canvas") as HTMLElement).scrollTo({ top: 3000 }));
      await page.waitForTimeout(100);
      const gap = await page.evaluate(() => {
        const ruler = document.querySelector<HTMLElement>(".ruler");
        if (!ruler || ruler.hidden || !ruler.offsetParent) return -1;
        return ruler.getBoundingClientRect().top - document.querySelector<HTMLElement>(".canvas")!.getBoundingClientRect().top;
      });
      expect(gap, `ruler at ${z}`).toBeGreaterThanOrEqual(0);
      expect(gap, `ruler at ${z}`).toBeLessThan(2);
    }
  });
});
