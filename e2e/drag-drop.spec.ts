// Dragging selected text: move it, or copy it with Alt, with a caret showing where it will land. One undo step.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, paraText, test, wordPoint, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const ids = (page: Page) => page.locator(".flow .fp").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));

async function open(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["The quick brown fox jumps over the lazy dog.", "Second paragraph with some words.", "Third one."] }] });
  return ids(page);
}

/** The point just past the end of a short paragraph's text, where a drop lands at its end. */
async function endOf(page: Page, p: number) {
  const b = (await page.locator(`.flow [data-p="${p}"]`).boundingBox())!;
  return { x: b.x + b.width - 6, y: b.y + Math.min(10, b.height / 2) };
}

/** Select a word through the model, so the next press on it is a drag and not the third click of a triple-click. */
async function selectWord(page: Page, p: number, word: string) {
  const o = (await paraText(page, p)).indexOf(word);
  await inPage(
    page,
    (w, a) => {
      const ed = w.fe.editor as unknown as { select(s: unknown): void; focus(): void; emitSelection(): void };
      ed.focus();
      ed.select({ anchor: { p: a.p, o: a.o }, focus: { p: a.p, o: a.o + a.n } });
      ed.emitSelection();
    },
    { p, o, n: word.length },
  );
}

/**
 * Drag the selected word to a point with the events a browser sends for it: dragstart on the text, dragenter and dragover
 * as the pointer moves, then drop and dragend (unless `drop` is false). Playwright's mouse can't start the browser's own
 * text drag, so the events are sent; the editor reacts to events, not to the mouse.
 */
async function drag(page: Page, from: { x: number; y: number }, to: { x: number; y: number }, opts: { alt?: boolean; drop?: boolean } = {}) {
  await page.evaluate(
    ([a, b, alt, drop]) => {
      const dt = new DataTransfer();
      const fire = (el: Element, type: string, at: { x: number; y: number }) => el.dispatchEvent(new DragEvent(type, { dataTransfer: dt, clientX: at.x, clientY: at.y, altKey: !!alt, bubbles: true, cancelable: true }));
      const src = document.elementFromPoint(a.x, a.y)!;
      const dst = document.elementFromPoint(b.x, b.y)!;
      fire(src, "dragstart", a);
      fire(dst, "dragenter", b);
      fire(dst, "dragover", b);
      if (!drop) return;
      fire(dst, "drop", b);
      fire(src, "dragend", b);
    },
    [from, to, !!opts.alt, opts.drop !== false] as const,
  );
}

test.describe("dragging text", () => {
  test("moves the selection to where it is dropped, as one undo step", async ({ page }) => {
    const [p1, p2] = await open(page);
    await selectWord(page, p1, "quick");
    const from = await wordPoint(page, p1, "quick");
    const to = await endOf(page, p2);
    await drag(page, from, to);
    await expect.poll(() => paraText(page, p1)).toBe("The  brown fox jumps over the lazy dog.");
    expect(await paraText(page, p2)).toBe("Second paragraph with some words.quick");
    // The dropped text is selected.
    expect(await inPage(page, (w) => w.fe.editor.sel && w.fe.editor.engine!.textRange(w.fe.editor.sel.anchor, w.fe.editor.sel.focus))).toBe("quick");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => paraText(page, p1)).toBe("The quick brown fox jumps over the lazy dog.");
    expect(await paraText(page, p2)).toBe("Second paragraph with some words.");
  });

  test("holding Alt copies it instead", async ({ page }) => {
    const [p1, p2] = await open(page);
    await selectWord(page, p1, "fox");
    const from = await wordPoint(page, p1, "fox");
    const to = await endOf(page, p2);
    await drag(page, from, to, { alt: true });
    await expect.poll(() => paraText(page, p2)).toBe("Second paragraph with some words.fox");
    expect(await paraText(page, p1)).toBe("The quick brown fox jumps over the lazy dog.");
  });

  test("a caret shows where the text will land and goes when it does", async ({ page }) => {
    const [p1, p2] = await open(page);
    await selectWord(page, p1, "brown");
    const from = await wordPoint(page, p1, "brown");
    const to = await wordPoint(page, p2, "paragraph");
    await drag(page, from, to, { drop: false });
    const caret = page.locator(".drop-caret");
    await expect(caret).toBeVisible();
    const box = (await caret.boundingBox())!;
    const target = (await page.locator(`.flow [data-p="${p2}"]`).boundingBox())!;
    expect(box.y).toBeGreaterThanOrEqual(target.y - 2);
    expect(box.y + box.height).toBeLessThanOrEqual(target.y + target.height + 2);
    expect(box.height).toBeGreaterThan(8);
    await page.evaluate(([x, y]) => {
      const dst = document.elementFromPoint(x, y)!;
      dst.dispatchEvent(new DragEvent("dragleave", { relatedTarget: document.body, bubbles: true }));
    }, [to.x, to.y]);
    await expect(caret).toHaveCount(0);
  });

  test("dropping the text on itself changes nothing", async ({ page }) => {
    const [p1] = await open(page);
    await selectWord(page, p1, "jumps");
    const from = await wordPoint(page, p1, "jumps");
    await drag(page, from, { x: from.x + 4, y: from.y });
    expect(await paraText(page, p1)).toBe("The quick brown fox jumps over the lazy dog.");
  });

  test("a move keeps the formatting of the text", async ({ page }) => {
    const [p1, p2] = await open(page);
    await selectWord(page, p1, "lazy");
    await page.keyboard.press(`${MOD}+B`);
    const from = await wordPoint(page, p1, "lazy");
    const to = await endOf(page, p2);
    await drag(page, from, to);
    await expect.poll(() => paraText(page, p2)).toContain("lazy");
    const at = (await paraText(page, p2)).indexOf("lazy") + 2;
    expect(await inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos)?.run.bold, { p: p2, o: at })).toBe(true);
  });

  test("text dropped from outside is read like a paste, at the drop point", async ({ page }) => {
    const [p1, p2] = await open(page);
    const to = await wordPoint(page, p2, "some");
    await page.evaluate(([x, y]) => {
      const dt = new DataTransfer();
      dt.setData("text/html", '<b style="color:#c00000">dropped</b> ');
      dt.setData("text/plain", "dropped ");
      const target = document.elementFromPoint(x, y)!;
      for (const type of ["dragenter", "dragover", "drop"]) target.dispatchEvent(new DragEvent(type, { dataTransfer: dt, clientX: x, clientY: y, bubbles: true, cancelable: true }));
    }, [to.x, to.y]);
    await expect.poll(() => paraText(page, p2)).toContain("dropped");
    const t = await paraText(page, p2);
    expect(await inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos)?.run.color, { p: p2, o: t.indexOf("dropped") + 2 })).toBe("C00000");
    void p1;
  });

  test("a move into another window takes the text from here when that window says it moved", async ({ page }) => {
    const [p1] = await open(page);
    await selectWord(page, p1, "brown");
    const from = await wordPoint(page, p1, "brown");
    await page.evaluate(([x, y]) => {
      const dt = new DataTransfer();
      const src = document.elementFromPoint(x, y)!;
      src.dispatchEvent(new DragEvent("dragstart", { dataTransfer: dt, clientX: x, clientY: y, bubbles: true, cancelable: true }));
      // The drag carries Docxtor's own format, so the other window keeps the formatting.
      (window as unknown as { __types: string[] }).__types = Array.from(dt.types);
      Object.defineProperty(dt, "dropEffect", { value: "move" });
      src.dispatchEvent(new DragEvent("dragend", { dataTransfer: dt, bubbles: true }));
    }, [from.x, from.y]);
    expect(await page.evaluate(() => (window as unknown as { __types: string[] }).__types)).toEqual(expect.arrayContaining(["application/x-docxtor-fragment", "text/plain", "text/html"]));
    await expect.poll(() => paraText(page, p1)).toBe("The quick  fox jumps over the lazy dog.");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => paraText(page, p1)).toBe("The quick brown fox jumps over the lazy dog.");
  });
});
