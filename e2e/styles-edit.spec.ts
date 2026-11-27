// Editing styles: the style gallery (each style drawn as it looks), Modify style and New style, Create a style from the
// selection, Update to match, Manage styles, the Styles panel and Reveal formatting. Checked in the document model
// (styles.xml through the engine) and on the page.
import { answer, chooseMenu, dialog, documentReady, expect, inPage, menuItem, type Page, selectWord, test } from "./helpers";

/** A new document in the Library holding Markdown (headings, paragraphs), open on screen. */
async function newDoc(page: Page, md: string) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w, m) => w.createIn(w.files.library, "", "Styles.docx", m), md);
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Styles");
  await documentReady(page);
}

/** The id of the first paragraph whose text contains `text`. */
const idOf = async (page: Page, text: string) => Number(await page.locator(".flow .fp").filter({ hasText: text }).first().getAttribute("data-p"));

/** Engine bits the tests read. */
interface Eng {
  styles(): { id: string; name: string; kind: string; custom?: boolean; quick: boolean }[];
  styleDetails(s: string): { id: string; name: string; basedOn?: string; next?: string; quick: boolean; autoUpdate: boolean; ownRun: Record<string, unknown>; ownPara: Record<string, unknown>; run: Record<string, unknown>; para: Record<string, unknown> } | null;
  formatAt(p: { p: number; o: number }): { run: Record<string, unknown>; para: Record<string, unknown>; style?: string } | null;
  styleUses(s: string): { p: number; from: number; to: number }[];
}
const eng = (page: Page) => ({
  details: (style: string) => inPage(page, (w, s) => (w.fe.editor.engine as unknown as Eng).styleDetails(s), style),
  styles: () => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).styles()),
  at: (p: number, o = 0) => inPage(page, (w, pos) => (w.fe.editor.engine as unknown as Eng).formatAt(pos), { p, o }),
  uses: (s: string) => inPage(page, (w, st) => (w.fe.editor.engine as unknown as Eng).styleUses(st), s),
});

const stylePicker = (page: Page) => page.locator(".toolbar .style-pick");
const gallery = (page: Page) => page.locator(".style-gallery");
const tile = (page: Page, id: string) => gallery(page).locator(`.style-tile[data-style="${id}"]`);

async function openGallery(page: Page) {
  await stylePicker(page).click();
  await expect(gallery(page)).toBeVisible();
}

const modifyDialog = (page: Page) => dialog(page, "Modify style");

async function click(page: Page, id: number, text: string) {
  const p = page.locator(`.flow [data-p="${id}"]`);
  await p.scrollIntoViewIfNeeded();
  await p.getByText(text).first().click();
}

test.describe("Style gallery", () => {
  test("each tile is the style's name drawn in its own look, and Word's styles the file lacks are offered", async ({ page }) => {
    await newDoc(page, "# Big title\n\nSome body text here.");
    await openGallery(page);
    // Heading 1 in the file: Calibri Light, 20 pt, the theme's dark blue accent. The tile caps the size so it fits one line.
    const h1 = tile(page, "Heading1").locator(".style-sample");
    await expect(h1).toHaveText("Heading 1");
    const css = await h1.evaluate((e) => ({ color: getComputedStyle(e).color, size: parseFloat(getComputedStyle(e).fontSize), family: getComputedStyle(e).fontFamily }));
    expect(css.color).toBe("rgb(47, 84, 150)");
    expect(css.size).toBeGreaterThanOrEqual(18);
    expect(css.family).toContain("Calibri Light");
    const normal = await tile(page, "Normal").locator(".style-sample").evaluate((e) => ({ size: parseFloat(getComputedStyle(e).fontSize), family: getComputedStyle(e).fontFamily }));
    expect(normal.size).toBeLessThan(css.size);
    expect(normal.family).toContain("Calibri");
    // The built-in styles the file does not have yet, in their own look: Intense Quote is italic and centered.
    await expect(tile(page, "IntenseQuote")).toHaveClass(/latent/);
    const iq = await tile(page, "IntenseQuote").locator(".style-sample").evaluate((e) => ({ style: getComputedStyle(e).fontStyle, align: getComputedStyle(e).textAlign }));
    expect(iq).toEqual({ style: "italic", align: "center" });
    for (const id of ["Title", "Subtitle", "Heading3", "Heading9", "Quote", "Emphasis", "Strong", "NoSpacing", "BookTitle"]) await expect(tile(page, id)).toBeVisible();
    // The paragraph the caret is in has its tile marked.
    await expect(tile(page, "Normal")).toHaveAttribute("aria-selected", "false");
    await page.screenshot({ path: "test-results/style-gallery.png" });
  });

  test("choosing a style the file lacks adds Word's definition and applies it, as one undo step", async ({ page }) => {
    await newDoc(page, "# Heading\n\nAn important quote.");
    const id = await idOf(page, "important quote");
    await click(page, id, "important");
    expect((await eng(page).styles()).some((s) => s.id === "IntenseQuote")).toBe(false);
    await openGallery(page);
    await tile(page, "IntenseQuote").click();
    await expect(gallery(page)).toBeHidden();
    expect((await eng(page).at(id)).style).toBe("IntenseQuote");
    expect((await eng(page).styles()).some((s) => s.id === "IntenseQuote")).toBe(true);
    await expect(stylePicker(page)).toContainText("Intense Quote");
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).styles()).some((s) => s.id === "IntenseQuote")).toBe(false);
    expect((await eng(page).at(id)).style).toBeUndefined();
  });

  test("a character style from the gallery is applied to the selected words", async ({ page }) => {
    await newDoc(page, "Make this word stand out.");
    const id = await idOf(page, "Make this");
    await selectWord(page, id, "word");
    await openGallery(page);
    await tile(page, "Strong").click();
    expect((await eng(page).at(id, 12)).run.bold).toBe(true);
    expect(await eng(page).uses("Strong")).toEqual([{ p: id, from: 10, to: 14 }]);
    expect((await eng(page).at(id, 3)).run.bold).toBeFalsy();
  });

  test("a right-click on a tile offers Modify, Update, Select all and the gallery switch", async ({ page }) => {
    await newDoc(page, "# One\n\nBody\n\n# Two");
    await openGallery(page);
    await tile(page, "Heading1").click({ button: "right" });
    await expect(page.getByRole("button", { name: /^Modify “Heading 1”…/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /^Update “Heading 1” to match the selection/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /^Select all 2 instances/ })).toBeVisible();
    await expect(page.getByRole("button", { name: "Remove from the style gallery" })).toBeVisible();
    // A style of Word's cannot be deleted from here.
    await expect(page.getByRole("button", { name: /^Delete/ })).toHaveCount(0);
    await page.getByRole("button", { name: /^Select all 2 instances/ }).click();
    // The first heading is the real selection, the second is drawn as another range.
    await expect(page.locator(".xs-layer .xs-r")).toHaveCount(1);
  });
});

test.describe("Modify style", () => {
  test("changes every paragraph that uses the style at once, and undo takes it back in one step", async ({ page }) => {
    await newDoc(page, "# First heading\n\nBody\n\n# Second heading");
    const a = await idOf(page, "First heading");
    const b = await idOf(page, "Second heading");
    await click(page, a, "First");
    await chooseMenu(page, "Format", "Paragraph styles", "Modify style…");
    const d = modifyDialog(page);
    await expect(d).toBeVisible();
    await expect(d.locator("#ms-name")).toHaveValue("Heading 1");
    await expect(d.locator("#ms-name")).toBeDisabled();
    await expect(d.locator("#ms-size")).toHaveValue("20");
    await expect(d.locator(".ms-description")).toContainText("Font: Calibri Light, 20 pt");
    await d.locator("#ms-size").fill("30");
    await d.locator("#ms-size").dispatchEvent("change");
    await d.getByRole("button", { name: "Bold" }).click();
    await d.locator("#ms-after").fill("20");
    await d.locator("#ms-after").dispatchEvent("change");
    await d.getByRole("button", { name: "Align right" }).click();
    // The preview shows the new look before anything is written.
    await expect(d.locator(".ms-sample")).toHaveCSS("text-align", "right");
    await page.screenshot({ path: "test-results/modify-style.png" });
    expect(await eng(page).at(a)).toMatchObject({ run: { size: 20 } });
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toBeHidden();
    for (const id of [a, b]) {
      const f = await eng(page).at(id);
      expect(f.run).toMatchObject({ size: 30, bold: true });
      expect(f.para).toMatchObject({ spaceAfter: 400, align: "right" });
    }
    // On the page too: 30 pt is 40 px.
    const px = await page.locator(`.flow [data-p="${b}"]`).evaluate((e) => parseFloat(getComputedStyle((e.querySelector("span") ?? e) as HTMLElement).fontSize));
    expect(px).toBe(40);
    // The text itself carries nothing by hand.
    expect(((await eng(page).details("Heading1"))!.ownRun)).toMatchObject({ size: 30, bold: true });
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).at(a)).run).toMatchObject({ size: 20 });
    expect((await eng(page).at(b)).run).toMatchObject({ size: 20 });
    await page.keyboard.press("ControlOrMeta+Shift+z");
    expect((await eng(page).at(b)).run).toMatchObject({ size: 30 });
  });

  test("the Format button opens the paragraph dialogs for the style, and nothing is written until OK", async ({ page }) => {
    await newDoc(page, "# Heading\n\nBody");
    const a = await idOf(page, "Heading");
    await click(page, a, "Heading");
    await chooseMenu(page, "Format", "Paragraph styles", "Modify style…");
    const d = modifyDialog(page);
    await d.getByRole("button", { name: "Format" }).click();
    await menuItem(page, "Paragraph…").or(page.getByRole("button", { name: "Paragraph…" })).first().click();
    const para = dialog(page, "Paragraph");
    await expect(para).toBeVisible();
    await expect(para.locator("#pd-before")).toHaveValue("18");
    await para.locator("#pd-before").fill("30");
    await para.locator("#pd-keep-lines").check();
    await para.getByRole("button", { name: "OK" }).click();
    // The Modify dialog's own description now says so; the document has not changed.
    await expect(d.locator("#ms-before")).toHaveValue("30");
    await expect(d.locator(".ms-description")).toContainText("Keep lines together");
    expect((await eng(page).details("Heading1"))!.ownPara.spaceBefore).toBe(360);
    // Tabs and Borders open for the style as well.
    await d.getByRole("button", { name: "Format" }).click();
    await page.getByRole("button", { name: "Tabs…" }).click();
    await expect(dialog(page, "Tabs")).toBeVisible();
    await expect(dialog(page, "Tabs").locator("#tb-default")).toHaveCount(0);
    await dialog(page, "Tabs").getByRole("button", { name: "Cancel" }).click();
    await d.getByRole("button", { name: "OK" }).click();
    const details = await eng(page).details("Heading1");
    expect(details!.ownPara).toMatchObject({ spaceBefore: 600, keepLines: true });
  });

  test("a new style from the dialog: type, base, following style, gallery and automatic update", async ({ page }) => {
    await newDoc(page, "Body text");
    await chooseMenu(page, "Format", "Paragraph styles", "New style…");
    const d = dialog(page, "New style");
    await expect(d.locator("#ms-name")).toHaveValue("Style1");
    await d.locator("#ms-name").fill("Pull quote");
    await d.locator("#ms-based").selectOption("Normal");
    await d.locator("#ms-next").selectOption("Normal");
    await d.getByRole("button", { name: "Italic" }).click();
    await d.locator("#ms-font").selectOption("Georgia");
    await d.locator("#ms-auto").check();
    await d.getByRole("button", { name: "Create" }).click();
    await expect(d).toBeHidden();
    const s = await eng(page).details("Pull quote");
    expect(s).toMatchObject({ name: "Pull quote", basedOn: "Normal", next: "Normal", quick: true, autoUpdate: true });
    expect(s!.ownRun).toMatchObject({ italic: true, font: "Georgia" });
    await openGallery(page);
    await expect(tile(page, "Pullquote").locator(".style-sample")).toHaveText("Pull quote");
  });

  test("a name that is taken is refused where it is typed, and the dialog stays open", async ({ page }) => {
    await newDoc(page, "Body");
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "Callout" } }));
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "Aside" } }));
    await chooseMenu(page, "Format", "Paragraph styles", "Modify style…");
    // The caret's style is Normal: take Aside from the Manage styles dialog instead.
    await page.keyboard.press("Escape");
    await chooseMenu(page, "Format", "Paragraph styles", "Manage styles…");
    const m = dialog(page, "Manage styles");
    await m.locator('.styles-main[data-style="Aside"]').click();
    await m.getByRole("button", { name: "Modify…" }).click();
    const d = modifyDialog(page);
    await d.locator("#ms-name").fill("callout");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d.locator(".ms-error")).toContainText("already a style called");
    await expect(d).toBeVisible();
    await d.locator("#ms-name").fill("Aside note");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toBeHidden();
    expect((await eng(page).styles()).find((x) => x.id === "Aside")?.name).toBe("Aside note");
  });
});

test.describe("Create and update a style from the selection", () => {
  test("Create a style takes the selection's formatting, and the text takes the style", async ({ page }) => {
    await newDoc(page, "Make this text a style.\n\nAnother paragraph.");
    const a = await idOf(page, "Make this");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 9 }, format: { bold: true, color: "#336699", size: 16 } }), a);
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_paragraphs", ids: [id], format: { align: "center", spaceAfter: 400 } }), a);
    await selectWord(page, a, "Make");
    await chooseMenu(page, "Format", "Paragraph styles", "Create a style from the selection…");
    const d = dialog(page, "Create a style from the selection");
    await d.locator("#cs-name").fill("Feature");
    await d.getByRole("button", { name: "Create" }).click();
    await expect(d).toBeHidden();
    const s = await eng(page).details("Feature");
    expect(s).toMatchObject({ basedOn: "Normal", quick: true });
    expect(s!.ownRun).toMatchObject({ bold: true, color: "336699", size: 16 });
    expect(s!.ownPara).toMatchObject({ align: "center", spaceAfter: 400 });
    expect((await eng(page).at(a)).style).toBe("Feature");
    // It looks the same as it did, now through the style.
    expect((await eng(page).at(a)).run).toMatchObject({ bold: true, color: "336699", size: 16 });
    await expect(stylePicker(page)).toContainText("Feature");
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).styles()).some((x) => x.id === "Feature")).toBe(false);
    expect((await eng(page).at(a)).style).toBeUndefined();
  });

  test("Update style to match the selection moves what is by hand into the style", async ({ page }) => {
    await newDoc(page, "# One\n\n# Two");
    const a = await idOf(page, "One");
    const b = await idOf(page, "Two");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 3 }, format: { color: "#00AA00", size: 24 } }), a);
    await selectWord(page, a, "One");
    await chooseMenu(page, "Format", "Paragraph styles", "Update style to match the selection");
    for (const id of [a, b]) expect((await eng(page).at(id)).run).toMatchObject({ color: "00AA00", size: 24 });
    expect((await eng(page).details("Heading1"))!.ownRun).toMatchObject({ color: "00AA00", size: 24 });
  });
});

test.describe("Manage styles and the Styles panel", () => {
  async function withStyles(page: Page) {
    await newDoc(page, "# Heading\n\nOne\n\nTwo\n\nThree");
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "Aside", rpr: { italic: true, color: "#CC0000" } } }));
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "Keeper", kind: "character", rpr: { bold: true } } }));
    const two = await idOf(page, "Two");
    const three = await idOf(page, "Three");
    await inPage(page, (w, ids) => w.fe.editor.exec({ op: "set_style", ids, style: "Aside" }), [two, three]);
  }

  test("lists the styles as they look, filters them, and finds every instance", async ({ page }) => {
    await withStyles(page);
    await chooseMenu(page, "Format", "Paragraph styles", "Manage styles…");
    const m = dialog(page, "Manage styles");
    await expect(m).toBeVisible();
    // Gallery styles first (Normal, Heading 1 and the two made here).
    const rows = m.locator(".styles-main");
    await expect(rows.first()).toHaveAttribute("data-style", "Normal");
    await page.screenshot({ path: "test-results/manage-styles.png" });
    const aside = m.locator('.styles-main[data-style="Aside"]');
    await expect(aside.locator(".style-sample")).toHaveCSS("font-style", "italic");
    await expect(aside.locator(".style-sample")).toHaveCSS("color", "rgb(204, 0, 0)");
    await expect(m.locator('[data-row="Aside"] .styles-count')).toHaveText("2");
    // In use only: Normal (the first paragraph is a heading, so Normal has one), Heading 1 and Aside; Keeper is not.
    await m.getByLabel("Show").selectOption("use");
    await expect(m.locator('.styles-main[data-style="Keeper"]')).toHaveCount(0);
    await expect(m.locator('.styles-main[data-style="Aside"]')).toBeVisible();
    // All styles, including Word's that are not in the file yet, A to Z.
    await m.getByLabel("Show").selectOption("all");
    await m.getByLabel("Sort").selectOption("alphabetical");
    const names = await m.locator(".styles-main .style-sample").allTextContents();
    expect(names).toEqual([...names].sort((x, y) => x.localeCompare(y, undefined, { sensitivity: "base" })));
    expect(names).toContain("Intense Quote");
    await m.getByPlaceholder("Search styles").fill("keep");
    await expect(m.locator(".styles-main")).toHaveCount(1);
    await m.getByPlaceholder("Search styles").fill("");
    // Select all instances.
    await m.locator('.styles-main[data-style="Aside"]').click();
    await expect(m.getByRole("button", { name: "Delete…" })).toBeEnabled();
    await m.getByRole("button", { name: "Select all instances" }).click();
    await expect(page.locator(".xs-layer .xs-r")).toHaveCount(1);
  });

  test("clears the formatting of every instance, and deletes a style you made", async ({ page }) => {
    await withStyles(page);
    await chooseMenu(page, "Format", "Paragraph styles", "Manage styles…");
    const m = dialog(page, "Manage styles");
    await m.getByLabel("Show").selectOption("all");
    await m.locator('.styles-main[data-style="Aside"]').click();
    await m.getByRole("button", { name: "Clear formatting of instances" }).click();
    await answer(page, "Clear formatting");
    expect(await eng(page).uses("Aside")).toHaveLength(0);
    await inPage(page, (w, ids) => w.fe.editor.exec({ op: "set_style", ids, style: "Aside" }), [await idOf(page, "Two")]);
    // Word's own styles cannot be deleted; one you made can, after asking.
    await m.locator('.styles-main[data-style="Heading1"]').click();
    await expect(m.getByRole("button", { name: "Delete…" })).toBeDisabled();
    await m.locator('.styles-main[data-style="Aside"]').click();
    await m.getByRole("button", { name: "Delete…" }).click();
    const confirm = page.getByRole("alertdialog", { name: /Delete the style “Aside”\?/ });
    await expect(confirm).toContainText("Normal");
    await confirm.getByRole("button", { name: "Delete" }).click();
    await expect(m.locator('.styles-main[data-style="Aside"]')).toHaveCount(0);
    expect((await eng(page).styles()).some((s) => s.id === "Aside")).toBe(false);
    // The paragraph that used it is a plain paragraph again.
    expect((await eng(page).at(await idOf(page, "Two"))).style ?? "Normal").toBe("Normal");
  });

  test("the Styles panel applies a style with a click and follows the caret", async ({ page }) => {
    await withStyles(page);
    await chooseMenu(page, "Format", "Paragraph styles", "Styles panel");
    const panel = page.locator(".styles-panel");
    await expect(panel).toBeVisible();
    const one = await idOf(page, "One");
    await click(page, one, "One");
    await panel.locator('.styles-main[data-style="Aside"]').click();
    expect((await eng(page).at(one)).style).toBe("Aside");
    await expect(panel.locator('.styles-main[data-style="Aside"]')).toHaveAttribute("aria-selected", "true");
    await click(page, await idOf(page, "Heading"), "Heading");
    await expect(panel.locator('.styles-main[data-style="Heading1"]')).toHaveAttribute("aria-selected", "true");
  });
});

test.describe("Reveal formatting", () => {
  test("says where each setting comes from and clears each layer", async ({ page }) => {
    await newDoc(page, "# A heading\n\nBody");
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "Marker", kind: "character", rpr: { italic: true, color: "#CC0000" } } }));
    const h = await idOf(page, "A heading");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 5 }, format: { style: "Marker", size: 30 } }), h);
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_paragraphs", ids: [id], format: { align: "center" } }), h);
    await page.locator(`.flow [data-p="${h}"]`).getByText("A").first().click();
    await chooseMenu(page, "Format", "Paragraph styles", "Reveal formatting");
    const panel = page.locator(".reveal-pane");
    await expect(panel).toBeVisible();
    const row = (key: string) => panel.locator(`.reveal-row[data-key="${key}"]`);
    await expect(panel.locator(".reveal-layer").first()).toContainText("Heading 1");
    await page.screenshot({ path: "test-results/reveal.png" });
    await expect(row("size")).toContainText("30 pt");
    await expect(row("size").locator(".reveal-source")).toHaveText("Set by hand");
    await expect(row("color").locator(".reveal-source")).toHaveText("Character style: Marker");
    await expect(row("font").locator(".reveal-source")).toHaveText("Style: Heading 1");
    await expect(row("font")).toContainText("Calibri Light");
    await expect(row("lineSpacing").locator(".reveal-source")).toHaveText("Document default");
    await expect(row("alignment").locator(".reveal-source")).toHaveText("Set by hand");
    // Clear the text layer: the size by hand goes, the character style stays.
    await panel.getByRole("button", { name: "Clear text" }).click();
    await expect(row("size").locator(".reveal-source")).toHaveText("Style: Heading 1");
    await expect(row("color").locator(".reveal-source")).toHaveText("Character style: Marker");
    // The character style layer and the paragraph layer have their own links.
    await panel.getByRole("button", { name: "Remove" }).click();
    await expect(row("color").locator(".reveal-source")).toHaveText("Style: Heading 1");
    await panel.getByRole("button", { name: "Clear paragraph" }).click();
    await expect(row("alignment")).toHaveCount(0);
    await panel.getByRole("button", { name: "Use Normal" }).click();
    await expect(panel.locator(".reveal-layer").first()).toContainText("Normal");
  });

  test("shows a style of the person's own as they named it", async ({ page }) => {
    await newDoc(page, "# A heading\n\nBody");
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "my notes", kind: "character", rpr: { italic: true, color: "#CC0000" } } }));
    const h = await idOf(page, "A heading");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 5 }, format: { style: "my notes" } }), h);
    await page.locator(`.flow [data-p="${h}"]`).getByText("A").first().click();
    await chooseMenu(page, "Format", "Paragraph styles", "Reveal formatting");
    const panel = page.locator(".reveal-pane");
    await expect(panel.locator(".reveal-row").filter({ hasText: "Character style: my notes" }).first()).toBeVisible();
    await expect(panel).not.toContainText("My Notes");
  });
});
