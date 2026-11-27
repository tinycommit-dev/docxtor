// The Page setup dialog (File, Page setup…): margins, paper and layout in three tabs, every paper Word lists, a gutter and where it
// goes, mirrored margins, header and footer distances, vertical alignment, odd and even pages, and where the change applies.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Locator, type Page } from "./helpers";

const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup() as unknown as Record<string, any>);
const open = async (page: Page, tab?: string) => {
  await chooseMenu(page, "File", "Page setup…");
  const d = dialog(page, "Page setup");
  await expect(d).toBeVisible();
  if (tab) await d.getByRole("tab", { name: tab }).click();
  return d;
};
const apply = (d: Locator) => d.getByRole("button", { name: "Apply" }).click();

test.describe("Page setup", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First section text", "Second line"] }] });
  });

  test("three tabs: margins, paper and layout", async ({ page }) => {
    const d = await open(page);
    await expect(d.getByRole("tab")).toHaveText(["Margins", "Paper", "Layout"]);
    await expect(d.getByRole("tab", { name: "Margins" })).toHaveAttribute("aria-selected", "true");
    await expect(d.getByRole("spinbutton", { name: "Top margin (inches)" })).toBeVisible();
    await d.getByRole("tab", { name: "Paper" }).click();
    await expect(d.getByLabel("Paper size", { exact: true })).toBeVisible();
    await d.getByRole("tab", { name: "Layout" }).click();
    await expect(d.getByLabel("Section start")).toBeVisible();
    await page.screenshot({ path: "test-results/page-setup-layout.png" });
  });

  test("every paper Word lists, and a custom size", async ({ page }) => {
    const d = await open(page, "Paper");
    const names = await d.getByLabel("Paper size", { exact: true }).locator("option").allTextContents();
    for (const want of ["Letter", "Legal", "Executive", "A3", "A4", "A5", "A6", "B4 JIS", "B5 JIS", "B5 ISO", "Tabloid", "Statement", "Folio", "Quarto", "10 × 14 in", "Envelope #10", "Envelope DL", "Envelope C5", "Envelope B5", "Envelope Monarch", "Japanese postcard", "Custom size"]) {
      expect(names.some((n) => n.startsWith(want)), want).toBe(true);
    }
    // Choosing a size fills in its width and height; typing one makes the size custom.
    await d.getByLabel("Paper size", { exact: true }).selectOption({ label: "A3 (297 × 420 mm)" });
    await expect(d.getByRole("spinbutton", { name: "Width (inches)" })).toHaveValue("11.69");
    await expect(d.getByRole("spinbutton", { name: "Height (inches)" })).toHaveValue("16.54");
    await d.getByRole("spinbutton", { name: "Width (inches)" }).fill("7");
    await d.getByRole("spinbutton", { name: "Height (inches)" }).fill("9");
    await expect(d.getByLabel("Paper size", { exact: true })).toHaveValue("custom");
    await apply(d);
    const s = (await setup(page)).sections[0];
    expect([s.width, s.height]).toEqual([10080, 12960]);
    // The sheet is that size on screen.
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const b = (await page.locator(".sheet").first().boundingBox())!;
    expect(Math.round(b.width / zoom)).toBe(672);
  });

  test("margins, a gutter at the top, and the header and footer distances", async ({ page }) => {
    let d = await open(page);
    await d.getByRole("spinbutton", { name: "Top margin (inches)" }).fill("1.5");
    await d.getByRole("spinbutton", { name: "Left margin (inches)" }).fill("1.25");
    await d.getByRole("spinbutton", { name: "Gutter (inches)" }).fill("0.5");
    await d.getByLabel("Gutter position").selectOption("top");
    await d.getByRole("tab", { name: "Layout" }).click();
    await d.getByRole("spinbutton", { name: "Header (inches)" }).fill("0.4");
    await d.getByRole("spinbutton", { name: "Footer (inches)" }).fill("0.3");
    await apply(d);
    const ps = await setup(page);
    const s = ps.sections[0];
    expect(s.margin).toEqual([2160, 1440, 1440, 1800]);
    expect([s.gutter, s.header, s.footer]).toEqual([720, 576, 432]);
    expect(ps.gutterAtTop).toBe(true);
    // The text starts below the top margin and the gutter.
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const sheet = (await page.locator(".sheet").first().boundingBox())!;
    const text = (await page.locator(".flow .fp").first().boundingBox())!;
    expect(Math.abs((text.y - sheet.y) / zoom - (2160 + 720) / 15)).toBeLessThan(2);
    expect(Math.abs((text.x - sheet.x) / zoom - 1800 / 15)).toBeLessThan(2);
    // Undo brings the old page back in one step.
    await page.keyboard.press("ControlOrMeta+Z");
    expect((await setup(page)).sections[0].margin).toEqual([1440, 1440, 1440, 1440]);
  });

  test("mirror margins, and the inside and outside names", async ({ page }) => {
    const d = await open(page);
    await d.getByLabel("Multiple pages").selectOption("mirror");
    await expect(d.getByRole("spinbutton", { name: "Inside margin (inches)" })).toBeVisible();
    await expect(d.getByRole("spinbutton", { name: "Outside margin (inches)" })).toBeVisible();
    await d.getByRole("spinbutton", { name: "Inside margin (inches)" }).fill("1.5");
    await apply(d);
    const ps = await setup(page);
    expect(ps.mirrorMargins).toBe(true);
    expect(ps.sections[0].margin[3]).toBe(2160);
    // Back to normal margins writes nothing.
    const again = await open(page);
    await expect(again.getByLabel("Multiple pages")).toHaveValue("mirror");
    await again.getByLabel("Multiple pages").selectOption("normal");
    await apply(again);
    expect((await setup(page)).mirrorMargins).toBeFalsy();
  });

  test("two pages per sheet and book fold turn the page and are saved", async ({ page }) => {
    let d = await open(page);
    await d.getByLabel("Multiple pages").selectOption("twoUp");
    await apply(d);
    let ps = await setup(page);
    expect(ps.printTwoOnOne).toBe(true);
    expect(ps.sections[0].width).toBeGreaterThan(ps.sections[0].height);
    d = await open(page);
    await expect(d.getByLabel("Multiple pages")).toHaveValue("twoUp");
    await d.getByLabel("Multiple pages").selectOption("bookFold");
    await d.getByLabel("Sheets per booklet").selectOption("8");
    await apply(d);
    ps = await setup(page);
    expect(ps).toMatchObject({ bookFold: "bookFold", bookFoldSheets: 8 });
    expect(ps.printTwoOnOne).toBeFalsy();
  });

  test("layout: section start, different first page, odd and even, vertical alignment and text direction", async ({ page }) => {
    const d = await open(page, "Layout");
    await d.getByLabel("Section start").selectOption("oddPage");
    await d.getByLabel("Different first page").check();
    await d.getByLabel("Different odd and even pages").check();
    await d.getByLabel("Vertical alignment").selectOption("center");
    await d.getByLabel("Text direction").selectOption("tbRl");
    await apply(d);
    const ps = await setup(page);
    const s = ps.sections[0];
    expect(s).toMatchObject({ start: "oddPage", titlePage: true, vAlign: "center", textDirection: "tbRl" });
    expect(ps.evenAndOddHeaders).toBe(true);
    // The dialog reads it all back.
    const again = await open(page, "Layout");
    await expect(again.getByLabel("Section start")).toHaveValue("oddPage");
    await expect(again.getByLabel("Different first page")).toBeChecked();
    await expect(again.getByLabel("Different odd and even pages")).toBeChecked();
    await expect(again.getByLabel("Vertical alignment")).toHaveValue("center");
    await expect(again.getByLabel("Text direction")).toHaveValue("tbRl");
  });

  test("this point forward starts a new section where the caret is", async ({ page }) => {
    await page.locator(".flow .fp").nth(1).click();
    const d = await open(page);
    await d.getByRole("radio", { name: "This point forward" }).click();
    await d.getByRole("spinbutton", { name: "Top margin (inches)" }).fill("2");
    await apply(d);
    const ps = await setup(page);
    expect(ps.sections).toHaveLength(2);
    expect(ps.sections.map((s: { margin: number[] }) => s.margin[0])).toEqual([1440, 2880]);
    // One undo takes both the break and the margins back.
    await page.keyboard.press("ControlOrMeta+Z");
    expect((await setup(page)).sections).toHaveLength(1);
  });

  test("a document with several sections can change just this section", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["One"] }, { paragraphs: ["Two"] }] });
    await page.locator(".flow .fp").first().click();
    const d = await open(page);
    await expect(d.getByRole("radio", { name: "This section" })).toBeVisible();
    await d.getByRole("radio", { name: "This section" }).click();
    await d.getByRole("tab", { name: "Layout" }).click();
    await d.getByLabel("Vertical alignment").selectOption("bottom");
    await apply(d);
    const s = (await setup(page)).sections;
    expect(s[0].vAlign).toBe("bottom");
    expect(s[1].vAlign).toBeUndefined();
  });

  test("Apply leaves what was not changed as the file has it", async ({ page }) => {
    // A direction the dialog has no name for, a gutter on the right and a header distance of its own.
    await openBuilt(page, { sections: [{ gutter: 360, rtlGutter: true, header: 500, xml: { end: '<w:textDirection w:val="tbRlV"/>' }, paragraphs: ["One"] }] });
    const d = await open(page);
    await d.getByRole("spinbutton", { name: "Top margin (inches)" }).fill("2");
    await d.getByRole("tab", { name: "Layout" }).click();
    await expect(d.getByLabel("Text direction")).toHaveValue("tbRlV");
    await apply(d);
    const s = (await setup(page)).sections[0];
    expect(s).toMatchObject({ textDirection: "tbRlV", rtlGutter: true, gutter: 360, header: 500 });
    expect(s.margin[0]).toBe(2880);
  });

  test("lengths follow the unit chosen under Preferences, and the file keeps twips", async ({ page }) => {
    await chooseMenu(page, "Tools", "Preferences…");
    const prefs = dialog(page, "Preferences");
    await prefs.getByLabel("Measurement units").selectOption("cm");
    await prefs.getByRole("button", { name: "Apply" }).click();
    const d = await open(page);
    await expect(d.getByRole("spinbutton", { name: "Top margin (cm)" })).toHaveValue("2.54");
    await d.getByRole("spinbutton", { name: "Top margin (cm)" }).fill("3");
    await apply(d);
    expect((await setup(page)).sections[0].margin[0]).toBe(1701);
    // The ruler counts in centimetres too.
    await expect(page.locator(".ruler")).toBeVisible();
  });
});
