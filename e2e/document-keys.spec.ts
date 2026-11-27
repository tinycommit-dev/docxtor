// Select all and the start and end of the document (with and without Shift) are commands that set the selection from the
// model, not from what the browser has laid out (it will not lay out far pages).
// The tests read the engine's text and the model selection, never layout, so they hold when only a few pages are drawn.
import { openBuilt } from "./support/docx-builder";
import { dialog, chooseMenu, expect, inPage, openFixture, RICH, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const N = 240;
const lines = Array.from({ length: N }, (_, i) => `Line ${i + 1}`);

const modelSel = (page: Page) => inPage(page, (w) => w.fe.editor.sel);
const engineText = async (page: Page) => (await inPage(page, (w) => w.fe.editor.engine!.text())).trimEnd();
/** The model ids of the first and last paragraph of the body, and the length of the last. */
const ends = (page: Page) =>
  inPage(page, (w) => {
    const all = w.fe.editor.engine!.renderAll();
    const ps = all.blocks.filter((b) => b.t === "p");
    const last = ps[ps.length - 1];
    return { first: ps[0].id, last: last.id, lastLen: last.len ?? 0 };
  });
/** The text of the selection from the engine (the model's answer, not the browser's). */
const selectedText = (page: Page) =>
  inPage(page, (w) => {
    const s = w.fe.editor.sel!;
    return w.fe.editor.engine!.textRange(s.anchor, s.focus);
  });

/** Put the caret in a paragraph in the middle of the long document, with the model selection set (no layout needed). */
async function caretInMiddle(page: Page) {
  const mid = await inPage(page, (w) => {
    const ps = w.fe.editor.engine!.renderAll().blocks.filter((b) => b.t === "p");
    const id = ps[Math.floor(ps.length / 2)].id;
    const at = { p: id, o: 2 };
    (w.fe.editor as unknown as { select(s: unknown): void }).select(at);
    return id;
  });
  await page.locator(".flow").focus();
  return mid;
}

test.describe("select all and the ends of the document", () => {
  test("select all, copy and delete take the whole of a long document", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: lines }] });
    expect(await inPage(page, (w) => w.fe.editor.pages)).toBeGreaterThan(3);
    await caretInMiddle(page);
    await page.evaluate(() => {
      const w = window as unknown as { __copied?: string };
      window.addEventListener("copy", (e) => void (w.__copied = e.clipboardData?.getData("text/plain") ?? ""));
    });
    await page.keyboard.press(`${MOD}+a`);
    const { first, last, lastLen } = await ends(page);
    expect(await modelSel(page)).toEqual({ anchor: { p: first, o: 0 }, focus: { p: last, o: lastLen } });
    await page.keyboard.press(`${MOD}+c`);
    const copied = await page.evaluate(() => (window as unknown as { __copied?: string }).__copied ?? "");
    expect(copied.replace(/\r?\n+/g, "\n").trim()).toBe((await engineText(page)).replace(/\r?\n+/g, "\n").trim());
    expect(copied).toContain("Line 1");
    expect(copied).toContain(`Line ${N}`);
    await page.keyboard.press("Delete");
    expect(await engineText(page)).toBe("");
  });

  test("Mod+Home and Mod+End go to the very start and end, and with Shift extend the selection there", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: lines }] });
    const { first, last, lastLen } = await ends(page);
    const mid = await caretInMiddle(page);
    await page.keyboard.press(`${MOD}+End`);
    expect(await modelSel(page)).toEqual({ anchor: { p: last, o: lastLen }, focus: { p: last, o: lastLen } });
    await page.keyboard.press(`${MOD}+Home`);
    expect(await modelSel(page)).toEqual({ anchor: { p: first, o: 0 }, focus: { p: first, o: 0 } });
    // Shift+Mod+End from the middle: anchored where the caret was, the focus at the very end.
    await caretInMiddle(page);
    await page.keyboard.press(`${MOD}+Shift+End`);
    const s = (await modelSel(page))!;
    expect(s.anchor).toEqual({ p: mid, o: 2 });
    expect(s.focus).toEqual({ p: last, o: lastLen });
    const text = await selectedText(page);
    expect(text.endsWith(`Line ${N}`)).toBe(true);
    expect(text.startsWith("ne ")).toBe(true);
    // And Shift+Mod+Home.
    await caretInMiddle(page);
    await page.keyboard.press(`${MOD}+Shift+Home`);
    const h = (await modelSel(page))!;
    expect(h.anchor).toEqual({ p: mid, o: 2 });
    expect(h.focus).toEqual({ p: first, o: 0 });
    expect((await selectedText(page)).startsWith("Line 1")).toBe(true);
  });

  test("Cmd+Up and Cmd+Down do the same on a Mac", async ({ page }) => {
    const mac = await page.evaluate(() => navigator.platform.toLowerCase().includes("mac"));
    test.skip(!mac, "Mac only");
    await openBuilt(page, { sections: [{ paragraphs: lines }] });
    const { first, last, lastLen } = await ends(page);
    await caretInMiddle(page);
    await page.keyboard.press("Meta+ArrowDown");
    expect((await modelSel(page))!.focus).toEqual({ p: last, o: lastLen });
    await page.keyboard.press("Meta+Shift+ArrowUp");
    expect((await modelSel(page))!.focus).toEqual({ p: first, o: 0 });
  });

  test("in a header the keys stay in the header", async ({ page }) => {
    await openBuilt(page, { sections: [{ headers: { default: "Running head" }, paragraphs: lines }] });
    const head = await inPage(page, (w) => {
      const setup = w.fe.editor.engine!.pageSetup() as unknown as { headers: Record<string, { blocks: { id: number }[] }> };
      const id = setup.headers.default.blocks[0].id;
      (w.fe.editor as unknown as { enterStory(z: string, p: number): void }).enterStory("header", 0);
      return id;
    });
    await expect(page.locator(".story-edit")).toBeVisible();
    await page.keyboard.press(`${MOD}+End`);
    expect((await modelSel(page))!.focus).toEqual({ p: head, o: 12 });
    await page.keyboard.press(`${MOD}+Home`);
    expect((await modelSel(page))!.focus).toEqual({ p: head, o: 0 });
    await page.keyboard.press(`${MOD}+Shift+End`);
    expect(await modelSel(page)).toEqual({ anchor: { p: head, o: 0 }, focus: { p: head, o: 12 } });
    await page.keyboard.press(`${MOD}+End`);
    await page.keyboard.press(`${MOD}+a`);
    expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("Running head");
    expect(await modelSel(page)).toEqual({ anchor: { p: head, o: 0 }, focus: { p: head, o: 12 } });
    await expect(page.locator(".story-edit")).toBeVisible();
  });

  test("in a footnote the keys stay in the footnote", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Alpha beta gamma", ...lines.slice(0, 40)] }] });
    const note = await inPage(page, (w) => {
      const first = w.fe.editor.engine!.renderAll().order[0];
      const res = w.fe.editor.exec({ op: "insert_footnote", at: { p: first, o: 16 }, text: "Note text here" }) as { caret?: { p: number; o: number } }[];
      const at = res[0].caret!;
      (w.fe.editor as unknown as { enterStory(z: string, p: number, c: unknown): void }).enterStory("notes", 0, at);
      return at.p;
    });
    await expect(page.locator(".story-edit")).toBeVisible();
    await page.keyboard.press(`${MOD}+Home`);
    expect((await modelSel(page))!.focus).toEqual({ p: note, o: 0 });
    await page.keyboard.press(`${MOD}+End`);
    const end = (await modelSel(page))!.focus;
    expect(end.p).toBe(note);
    expect(end.o).toBeGreaterThanOrEqual(14);
    await page.keyboard.press(`${MOD}+a`);
    expect(await page.evaluate(() => window.getSelection()?.toString())).toContain("Note text here");
    expect((await modelSel(page))!.focus).toEqual(end);
  });

  test("in a table cell select all still takes the whole document", async ({ page }) => {
    await openFixture(page);
    await page.locator(`.flow [data-p="${RICH.cell}"]`).click();
    await expect(page.locator(`.flow td [data-p="${RICH.cell}"]`)).toHaveCount(1);
    await page.keyboard.press(`${MOD}+a`);
    const { first, last, lastLen } = await ends(page);
    expect(await modelSel(page)).toEqual({ anchor: { p: first, o: 0 }, focus: { p: last, o: lastLen } });
    // The model's text of the selection runs from the title to the last words.
    const text = await selectedText(page);
    expect(text.startsWith("Quarterly Product Report")).toBe(true);
    expect(text.includes("Simplicity is the ultimate sophistication.")).toBe(true);
  });

  test("in the find box, select all and the ends keep to the box's own text", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: lines.slice(0, 30) }] });
    await caretInMiddle(page);
    await page.keyboard.press(`${MOD}+f`);
    const box = page.getByRole("search").getByRole("searchbox", { name: "Find" });
    await expect(box).toBeVisible();
    await box.fill("Line 12");
    const before = await modelSel(page);
    await page.keyboard.press(`${MOD}+a`);
    expect(await box.evaluate((el: HTMLInputElement) => [el.selectionStart, el.selectionEnd])).toEqual([0, 7]);
    // The ends of the document are not taken from a text field: the editor's selection does not move.
    await page.keyboard.press(`${MOD}+Shift+End`);
    await page.keyboard.press(`${MOD}+Home`);
    await expect(box).toBeFocused();
    expect(await modelSel(page)).toEqual(before);
  });

  test("in a dialog's text field, select all keeps to the field", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    await expect(details).toBeVisible();
    const field = details.getByLabel("Title", { exact: true });
    await field.fill("A title for the file");
    const before = await modelSel(page);
    await page.keyboard.press(`${MOD}+a`);
    expect(await field.evaluate((el: HTMLInputElement) => [el.selectionStart, el.selectionEnd])).toEqual([0, 20]);
    await page.keyboard.press(`${MOD}+End`);
    await expect(field).toBeFocused();
    expect(await modelSel(page)).toEqual(before);
  });

  test("in a text box, the start and end are the box's own text, and the caret stays in the box", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await inPage(page, (w, o) => w.fe.editor.exec(o), { op: "insert_text_box", at: { p: RICH.intro, o: 0 }, w: 180, h: 72, text: "Hello" });
    const box = page.locator(".flow .atom.shape").first();
    await box.dblclick();
    await expect(box.locator(".shape-text")).toHaveAttribute("contenteditable", "true");
    const where = () => page.evaluate(() => { const s = getSelection()!; return { inBox: !!s.anchorNode?.parentElement?.closest(".shape-text"), text: s.toString(), offset: s.focusOffset }; });
    await page.keyboard.press(`${MOD}+Home`);
    expect(await where()).toEqual({ inBox: true, text: "", offset: 0 });
    await page.keyboard.press(`${MOD}+End`);
    expect(await where()).toEqual({ inBox: true, text: "", offset: 5 });
    await page.keyboard.press(`${MOD}+Shift+Home`);
    expect(await where()).toMatchObject({ inBox: true, text: "Hello" });
    await page.keyboard.press("Escape");
    await expect(page.locator(".obj-frame")).toBeVisible();
  });
});
