// Design: themes, colour schemes, font schemes, style sets and paragraph spacing; Set as default for new documents;
// and themes and style sets an extension adds. Checked in the document model (theme and styles through the engine)
// and on the page (computed fonts and colours).
import { answer, chooseMenu, dialog, documentReady, expect, inPage, menuItem, openMenu, type Page, test } from "./helpers";

async function newDoc(page: Page, md = "# The heading\n\nBody text of the page.", name = "Design.docx") {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w, a) => w.createIn(w.files.library, "", a.name, a.md), { name, md });
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue(name.replace(/\.docx$/, ""));
  await documentReady(page);
}

interface Theme {
  name: string;
  colors: { name: string; colors: Record<string, string> };
  fonts: { name: string; major: string; minor: string };
}
interface Eng {
  themeInfo(): Theme;
  formatAt(p: { p: number; o: number }): { run: Record<string, unknown>; para: Record<string, unknown> } | null;
  styleDetails(s: string): { run: Record<string, unknown>; para: Record<string, unknown> } | null;
}
const eng = (page: Page) => ({
  theme: () => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).themeInfo()),
  at: (p: number) => inPage(page, (w, id) => (w.fe.editor.engine as unknown as Eng).formatAt({ p: id, o: 0 }), p),
  details: (s: string) => inPage(page, (w, st) => (w.fe.editor.engine as unknown as Eng).styleDetails(st), s),
});
const idOf = async (page: Page, text: string) => Number(await page.locator(".flow .fp").filter({ hasText: text }).first().getAttribute("data-p"));

const themesGallery = (page: Page) => page.locator(".theme-gallery");
const themeTile = (page: Page, id: string) => themesGallery(page).locator(`.theme-tile[data-theme-id="docxtor.theme.${id}"]`);

test.describe("Themes", () => {
  test("the Design menu offers themes, colors, fonts, style sets and spacing", async ({ page }) => {
    await newDoc(page);
    await openMenu(page, "Design");
    for (const label of ["Themes…", "Colors…", "Fonts…", "Style set…", "Paragraph spacing", "Set as default"]) await expect(menuItem(page, label)).toBeVisible();
  });

  test("the gallery lists Word's themes with their colors and heading font", async ({ page }) => {
    await newDoc(page);
    await chooseMenu(page, "Design", "Themes…");
    await expect(themesGallery(page)).toBeVisible();
    const names = await themesGallery(page).locator(".theme-name").allTextContents();
    for (const want of ["Office", "Office 2023", "Facet", "Integral", "Ion", "Ion Boardroom", "Organic", "Retrospect", "Slice", "Wisp", "Basis", "Berlin", "Circuit", "Damask", "Depth", "Droplet", "Frame", "Gallery", "Main Event", "Mesh", "Parallax", "Savon", "View", "Vapor Trail", "Wood Type"]) {
      expect(names, want).toContain(want);
    }
    // A tile shows six accent colors and the heading font; the document's own theme is marked.
    await expect(themeTile(page, "facet").locator(".theme-chips i")).toHaveCount(6);
    await expect(themeTile(page, "facet").locator(".theme-chips i").first()).toHaveCSS("background-color", "rgb(144, 194, 38)");
    await expect(themeTile(page, "facet").locator(".theme-aa")).toHaveCSS("font-family", /Trebuchet MS/);
    await expect(themeTile(page, "office")).toHaveAttribute("aria-selected", "true");
    await page.screenshot({ path: "test-results/theme-gallery.png" });
  });

  test("choosing a theme changes the colors and fonts of everything that uses them, and undo takes it back", async ({ page }) => {
    await newDoc(page);
    const h = await idOf(page, "The heading");
    const b = await idOf(page, "Body text");
    const before = { head: await eng(page).at(h), body: await eng(page).at(b) };
    expect(before.head!.run).toMatchObject({ font: "Calibri Light", color: "2F5496" });
    await chooseMenu(page, "Design", "Themes…");
    await themeTile(page, "ion-boardroom").click();
    const info = await eng(page).theme();
    expect(info.name).toBe("Ion Boardroom");
    expect(info.fonts).toMatchObject({ major: "Century Gothic", minor: "Century Gothic" });
    expect(info.colors.colors.accent1).toBe("B31166");
    const after = { head: await eng(page).at(h), body: await eng(page).at(b) };
    expect(after.head!.run.font).toBe("Century Gothic");
    expect(after.body!.run.font).toBe("Century Gothic");
    expect(after.head!.run.color).not.toBe("2F5496");
    // On the page.
    const css = await page.locator(`.flow [data-p="${h}"]`).evaluate((e) => {
      const s = getComputedStyle((e.querySelector("span") ?? e) as HTMLElement);
      return { family: s.fontFamily, color: s.color };
    });
    expect(css.family).toContain("Century Gothic");
    expect(css.color).not.toBe("rgb(47, 84, 150)");
    await page.screenshot({ path: "test-results/theme-applied.png" });
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).theme()).name).toBe("Office");
    expect((await eng(page).at(h))!.run).toMatchObject({ font: "Calibri Light", color: "2F5496" });
    await page.keyboard.press("ControlOrMeta+Shift+z");
    expect((await eng(page).theme()).name).toBe("Ion Boardroom");
  });

  test("a table design and its borders follow the theme", async ({ page }) => {
    await newDoc(page, "Intro paragraph");
    const intro = await idOf(page, "Intro");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "insert_table", after: id, rows: 3, cols: 2 }), intro);
    const cell = await page.locator(".flow table .fp").first().getAttribute("data-p");
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "table_design", at: Number(id), style: "Grid Table 4 Accent 1" }), cell);
    const header = page.locator(".flow table tr").first().locator("td, th").first();
    await expect(header).toHaveCSS("background-color", "rgb(68, 114, 196)");
    await chooseMenu(page, "Design", "Themes…");
    await themeTile(page, "facet").click();
    await expect(header).toHaveCSS("background-color", "rgb(144, 194, 38)");
  });
});

test.describe("Colors and fonts", () => {
  test("a color scheme on its own, and customized colors", async ({ page }) => {
    await newDoc(page);
    const h = await idOf(page, "The heading");
    await chooseMenu(page, "Design", "Colors…");
    const blueWarm = page.locator('.scheme-row[data-theme-id="docxtor.colors.blue-warm"]');
    await expect(blueWarm.locator(".theme-chips i")).toHaveCount(8);
    await blueWarm.click();
    let info = await eng(page).theme();
    expect(info.colors.name).toBe("Blue Warm");
    expect(info.colors.colors.accent1).toBe("4A66AC");
    // The fonts and the theme's own name stay.
    expect(info.fonts.major).toBe("Calibri Light");
    expect(info.name).toBe("Office");
    // Customize colors: every slot has a picker and a hex box.
    await chooseMenu(page, "Design", "Colors…");
    await page.getByRole("button", { name: "Customize colors…" }).click();
    const d = dialog(page, "Customize colors");
    await expect(d.locator(".cc-row")).toHaveCount(12);
    await page.screenshot({ path: "test-results/customize-colors.png" });
    await expect(d.locator("#cc-accent1")).toHaveValue("#4a66ac");
    await d.getByLabel("Accent 1 (hex)").fill("AA0000");
    await d.locator("#cc-name").fill("Brick");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(d).toBeHidden();
    info = await eng(page).theme();
    expect(info.colors).toMatchObject({ name: "Brick" });
    expect(info.colors.colors.accent1).toBe("AA0000");
    expect(info.colors.colors.accent2).toBe("629DD1");
    // The heading is the accent shaded: it moved with it.
    const color = (await eng(page).at(h))!.run.color as string;
    expect(color).toMatch(/^[0-9A-F]{6}$/);
    expect(parseInt(color.slice(0, 2), 16)).toBeGreaterThan(parseInt(color.slice(2, 4), 16));
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).theme()).colors.colors.accent1).toBe("4A66AC");
  });

  test("a font scheme on its own, and customized fonts", async ({ page }) => {
    await newDoc(page);
    const h = await idOf(page, "The heading");
    const b = await idOf(page, "Body text");
    await chooseMenu(page, "Design", "Fonts…");
    await page.locator('.scheme-row[data-theme-id="docxtor.fonts.garamond"]').click();
    expect((await eng(page).at(h))!.run.font).toBe("Garamond");
    expect((await eng(page).at(b))!.run.font).toBe("Garamond");
    // The theme's colors stay.
    expect((await eng(page).theme()).colors.colors.accent1).toBe("4472C4");
    await chooseMenu(page, "Design", "Fonts…");
    await page.getByRole("button", { name: "Customize fonts…" }).click();
    const d = dialog(page, "Customize fonts");
    await d.locator("#cf-major").selectOption("Georgia");
    await d.locator("#cf-minor").selectOption("Verdana");
    await d.locator("#cf-name").fill("Mine");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(d).toBeHidden();
    expect((await eng(page).at(h))!.run.font).toBe("Georgia");
    expect((await eng(page).at(b))!.run.font).toBe("Verdana");
    expect((await eng(page).theme()).fonts).toEqual({ name: "Mine", major: "Georgia", minor: "Verdana" });
  });
});

test.describe("Style sets and paragraph spacing", () => {
  test("a style set rewrites Normal, the title and the headings, and its tile shows how", async ({ page }) => {
    await newDoc(page, "# The heading\n\nBody text of the page.");
    const h = await idOf(page, "The heading");
    const b = await idOf(page, "Body text");
    await chooseMenu(page, "Design", "Style set…");
    const gallery = page.locator(".set-gallery");
    await expect(gallery.locator(".set-tile")).toHaveCount(16);
    // Word 2003's tile draws Times New Roman text and an Arial heading.
    const w2003 = gallery.locator('.set-tile[data-set-id="word-2003"]');
    await expect(w2003.locator(".set-Heading1")).toHaveCSS("font-family", /Arial/);
    await expect(w2003.locator(".set-Normal").first()).toHaveCSS("font-family", /Times New Roman/);
    // Lines (Simple) puts a rule under the main heading.
    await expect(gallery.locator('.set-tile[data-set-id="lines-simple"] .set-Heading1')).toHaveCSS("border-bottom-style", "solid");
    await page.screenshot({ path: "test-results/style-sets.png" });
    await w2003.click();
    expect((await eng(page).at(h))!.run).toMatchObject({ font: "Arial", size: 16, bold: true });
    expect((await eng(page).at(b))!.run).toMatchObject({ font: "Times New Roman", size: 12 });
    expect((await eng(page).at(b))!.para).toMatchObject({ spaceAfter: 0, line: 240 });
    const css = await page.locator(`.flow [data-p="${b}"]`).evaluate((e) => getComputedStyle((e.querySelector("span") ?? e) as HTMLElement).fontFamily);
    expect(css).toContain("Times New Roman");
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).at(b))!.run).toMatchObject({ font: "Calibri", size: 11 });
  });

  test("the spacing list says one line, not one lines", async ({ page }) => {
    await newDoc(page);
    await inPage(page, (w) => (w.fe.extensions as unknown as { execute(id: string): Promise<unknown> }).execute("design.spacing"));
    const hint = page.locator('.menu-item[data-preset="none"] .hint');
    await expect(hint).toHaveText("0 pt after, 1 line");
    await expect(page.locator('.menu-item[data-preset="double"] .hint')).toHaveText("8 pt after, 2 lines");
  });

  test("a style set follows the theme's colors", async ({ page }) => {
    await newDoc(page);
    const h = await idOf(page, "The heading");
    await chooseMenu(page, "Design", "Style set…");
    await page.locator('.set-tile[data-set-id="word-2010"]').click();
    const blue = (await eng(page).at(h))!.run.color;
    await chooseMenu(page, "Design", "Themes…");
    await themeTile(page, "wisp").click();
    expect((await eng(page).at(h))!.run.color).not.toBe(blue);
  });

  test("paragraph spacing presets are written to the document defaults, and the current one is checked", async ({ page }) => {
    await newDoc(page);
    const b = await idOf(page, "Body text");
    await openMenu(page, "Design");
    await menuItem(page, "Paragraph spacing").click();
    await expect(menuItem(page, "Default")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Double").click();
    expect((await eng(page).at(b))!.para).toMatchObject({ line: 480, spaceAfter: 160 });
    const lh = await page.locator(`.flow [data-p="${b}"]`).evaluate((e) => parseFloat(getComputedStyle(e).lineHeight));
    expect(lh).toBeGreaterThan(32);
    await openMenu(page, "Design");
    await menuItem(page, "Paragraph spacing").click();
    await expect(menuItem(page, "Double")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "No paragraph space").click();
    expect((await eng(page).at(b))!.para).toMatchObject({ line: 240, spaceAfter: 0 });
    await page.keyboard.press("ControlOrMeta+z");
    expect((await eng(page).at(b))!.para).toMatchObject({ line: 480 });
  });
});

test.describe("Set as default", () => {
  test("new documents start with the design that was kept", async ({ page }) => {
    await newDoc(page);
    await chooseMenu(page, "Design", "Themes…");
    await themeTile(page, "facet").click();
    await chooseMenu(page, "Design", "Style set…");
    await page.locator('.set-tile[data-set-id="lines-simple"]').click();
    await chooseMenu(page, "Design", "Set as default");
    await page.getByRole("alertdialog", { name: "Set as default" }).getByRole("button", { name: "Set as default" }).click();
    // The editor keeps nothing itself: the app's extension state holds the design.
    expect(await page.evaluate(() => localStorage.getItem("docxtor.defaultDesign"))).toBeNull();
    expect(await page.evaluate(() => JSON.parse(localStorage.getItem("docxtor.ext.docxtor.design") ?? "{}").defaultDesign?.theme)).toBeTruthy();
    // A new document has the theme and the style set from the start, with nothing to undo.
    await inPage(page, (w) => w.createIn(w.files.library, "", "Second.docx", "# New heading\n\nNew body"));
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Second");
    await documentReady(page);
    expect((await eng(page).theme()).name).toBe("Facet");
    const h = await idOf(page, "New heading");
    expect((await eng(page).at(h))!.run.font).toBe("Trebuchet MS");
    expect(await inPage(page, (w) => (w.fe.editor.engine as unknown as { raw: { canUndo(): boolean } }).raw.canUndo())).toBe(false);
    // Reset: the next document is Word's default again.
    await chooseMenu(page, "Design", "Reset the default design");
    await answer(page, "Reset");
    await inPage(page, (w) => w.createIn(w.files.library, "", "Third.docx", "Third"));
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Third");
    await documentReady(page);
    expect((await eng(page).theme()).name).toBe("Office");
  });
});

test.describe("Themes and style sets from extensions", () => {
  test("an extension's theme, color scheme, font pair and style set sit beside Word's", async ({ page }) => {
    await newDoc(page);
    await inPage(page, async (w) => {
      await w.fe.use({
        id: "acme.design",
        name: "Acme design",
        activate(ctx: any) {
          ctx.documentThemes.register({ id: "acme.brand", label: "Acme brand", colors: { dk2: "1B2A41", lt2: "ECE9E1", accent1: "C8102E", accent2: "0A5C8C", accent3: "6C7A89", accent4: "F2A900", accent5: "2E7D32", accent6: "7B1FA2" }, fonts: { major: "Georgia", minor: "Verdana" }, group: "Acme" });
          ctx.documentThemes.register({ id: "acme.colors", label: "Acme colors", colors: { accent1: "112233" } });
          ctx.documentThemes.register({ id: "acme.fonts", label: "Acme fonts", fonts: { major: "Courier New", minor: "Arial" } });
          ctx.styleSets.register({
            id: "acme.set",
            label: "Acme report",
            description: "Red headings",
            styles: '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/></w:pPr><w:rPr><w:b/><w:color w:val="C8102E"/><w:sz w:val="36"/></w:rPr></w:style>',
            spacing: { after: 100, line: 252 },
          });
        },
      });
    });
    const h = await idOf(page, "The heading");
    await chooseMenu(page, "Design", "Themes…");
    const brand = themesGallery(page).locator('.theme-tile[data-theme-id="acme.brand"]');
    await expect(brand).toBeVisible();
    await expect(page.locator(".theme-gallery .menu-title", { hasText: "Acme" })).toBeVisible();
    await brand.click();
    let info = await eng(page).theme();
    expect(info).toMatchObject({ name: "Acme brand" });
    expect(info.colors.colors.accent1).toBe("C8102E");
    expect(info.fonts).toMatchObject({ major: "Georgia", minor: "Verdana" });
    // Colors and fonts that only carry one of them.
    await chooseMenu(page, "Design", "Colors…");
    await page.locator('.scheme-row[data-theme-id="acme.colors"]').click();
    info = await eng(page).theme();
    expect(info.colors.colors.accent1).toBe("112233");
    expect(info.fonts.major).toBe("Georgia");
    await chooseMenu(page, "Design", "Fonts…");
    await page.locator('.scheme-row[data-theme-id="acme.fonts"]').click();
    expect((await eng(page).theme()).fonts).toMatchObject({ major: "Courier New", minor: "Arial" });
    // A style set, drawn from its XML and applied through the same op.
    await chooseMenu(page, "Design", "Style set…");
    const set = page.locator('.set-tile[data-set-id="acme.set"]');
    await expect(set).toBeVisible();
    await expect(set.locator(".set-Heading1")).toHaveCSS("color", "rgb(200, 16, 46)");
    await set.click();
    expect((await eng(page).at(h))!.run).toMatchObject({ bold: true, color: "C8102E", size: 18 });
    expect((await eng(page).details("Normal"))!.para).toMatchObject({ spaceAfter: 100, line: 252 });
  });
});
