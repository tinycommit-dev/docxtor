// The Paragraph dialog (Format, Paragraph…, and the right-click menu), the Tabs dialog, tab stops on the ruler,
// and the text direction commands. Everything is checked in the document model and on the page.
import { test, expect, chooseMenu, caretAtEnd, contextItem, dialog, inPage, menuItem, openMenu, savedPart, type Page } from "./helpers";
import { openBuilt, type SectionSpec } from "./support/docx-builder";

type P = SectionSpec["paragraphs"][number];

async function open(page: Page, paragraphs: P[] = ["First paragraph of text", "Second paragraph"], extra: { settings?: string; styles?: string } = {}) {
  await openBuilt(page, { sections: [{ paragraphs }], ...extra });
  await caretAtEnd(page, 1);
}

const fmt = (page: Page, id = 1) =>
  inPage(page, (w, p) => w.fe.editor.engine!.formatAt({ p, o: 0 })!.para as Record<string, unknown> & { tabs?: { val: string; pos: number; leader?: string }[] }, id);

const field = (page: Page, id: string) => page.locator(`#${id}`);

test.describe("Paragraph dialog", () => {
  test("opens from the Format menu and the right-click menu, showing what the paragraph has", async ({ page }) => {
    await open(page, [{ text: "Indented", ppr: '<w:ind w:left="720" w:hanging="360"/><w:spacing w:before="240" w:after="120" w:line="360" w:lineRule="exact"/><w:jc w:val="center"/><w:keepNext/>' }, "Other"]);
    await chooseMenu(page, "Format", "Paragraph…");
    const d = dialog(page, "Paragraph");
    await expect(d).toBeVisible();
    await expect(field(page, "pd-align")).toHaveValue("center");
    await expect(field(page, "pd-left")).toHaveValue("0.5");
    await expect(field(page, "pd-special")).toHaveValue("hanging");
    await expect(field(page, "pd-by")).toHaveValue("0.25");
    await expect(field(page, "pd-before")).toHaveValue("12");
    await expect(field(page, "pd-after")).toHaveValue("6");
    await expect(field(page, "pd-line")).toHaveValue("exactly");
    await expect(field(page, "pd-at")).toHaveValue("18");
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toBeHidden();

    await page.locator('.flow [data-p="1"]').click({ button: "right" });
    await contextItem(page, "Paragraph…").click();
    await expect(dialog(page, "Paragraph")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog(page, "Paragraph")).toBeHidden();
  });

  test("indents, spacing and line spacing are written, and undo takes them back in one step", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Paragraph…");
    await field(page, "pd-left").fill("0.5");
    await field(page, "pd-right").fill("1 cm");
    await field(page, "pd-special").selectOption("first");
    await field(page, "pd-by").fill("0.3");
    await field(page, "pd-before").fill("12");
    await field(page, "pd-after").fill("3.5");
    await field(page, "pd-line").selectOption("atLeast");
    await field(page, "pd-at").fill("20");
    await field(page, "pd-contextual").check();
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    const f = await fmt(page);
    expect(f).toMatchObject({ indLeft: 720, indRight: 567, indFirstLine: 432, spaceBefore: 240, spaceAfter: 70, line: 400, lineRule: "atLeast", contextualSpacing: true });
    // On the page: the paragraph is indented, with space above, and the first line starts further in.
    const css = await page.locator('.flow [data-p="1"]').evaluate((e) => ({ pl: getComputedStyle(e).paddingLeft, pr: getComputedStyle(e).paddingRight, ti: getComputedStyle(e).textIndent }));
    // (Safari writes a length as the float it holds, 37.799999px, where Chrome and Firefox write 37.8px.)
    expect([css.pl, css.pr, css.ti].map(parseFloat)).toEqual([48, 37.8, 28.8].map((n) => expect.closeTo(n, 3)));
    await page.keyboard.press("ControlOrMeta+z");
    const again = await fmt(page);
    expect(again.indLeft).toBeUndefined();
    expect(again.lineRule).not.toBe("atLeast");
  });

  test("alignment, outline level and direction", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Paragraph…");
    await field(page, "pd-align").selectOption("distribute");
    await field(page, "pd-outline").selectOption("1");
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    expect(await fmt(page)).toMatchObject({ align: "distribute", outlineLvl: 1 });
    // Distributed text spreads its last line as well.
    const css = await page.locator('.flow [data-p="1"]').evaluate((e) => ({ a: getComputedStyle(e).textAlign, last: getComputedStyle(e).textAlignLast }));
    expect(css).toEqual({ a: "justify", last: "justify" });

    await chooseMenu(page, "Format", "Paragraph…");
    await expect(field(page, "pd-align")).toHaveValue("distribute");
    await expect(field(page, "pd-outline")).toHaveValue("1");
    await field(page, "pd-direction").selectOption("rtl");
    // With right-to-left text the indents are before and after the text.
    await expect(dialog(page, "Paragraph").getByText("Before text (in)")).toBeVisible();
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    expect(await fmt(page)).toMatchObject({ bidi: true });
    await expect(page.locator('.flow [data-p="1"]')).toHaveCSS("direction", "rtl");
  });

  test("mirror indents calls the sides inside and outside, and auto spacing writes the flag", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Paragraph…");
    const d = dialog(page, "Paragraph");
    await expect(d.getByText("Left (in)")).toBeVisible();
    await field(page, "pd-mirror").check();
    await expect(d.getByText("Inside (in)")).toBeVisible();
    await expect(d.getByText("Outside (in)")).toBeVisible();
    await field(page, "pd-before-auto").check();
    await expect(field(page, "pd-before")).toBeDisabled();
    await d.getByRole("button", { name: "OK" }).click();
    expect(await fmt(page)).toMatchObject({ mirrorIndents: true, beforeAuto: true });
    // Automatic spacing is the 14 points Word uses for HTML.
    const mt = await page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).marginTop);
    // (Safari lays a margin out in 1/64 px: 18.65625px for 18.6667.)
    expect(parseFloat(mt)).toBeCloseTo(280 / 15, 1);
  });

  test("the Line and page breaks tab sets the pagination options and the exceptions", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Paragraph…");
    const d = dialog(page, "Paragraph");
    await d.getByRole("tab", { name: "Line and page breaks" }).click();
    await expect(field(page, "pd-widow")).toBeChecked();
    await field(page, "pd-widow").uncheck();
    await field(page, "pd-keep-next").check();
    await field(page, "pd-keep-lines").check();
    await field(page, "pd-page-break").check();
    await field(page, "pd-no-numbers").check();
    await field(page, "pd-no-hyphen").check();
    await d.getByRole("button", { name: "OK" }).click();
    expect(await fmt(page)).toMatchObject({ widowControl: false, keepNext: true, keepLines: true, pageBreakBefore: true, suppressLineNumbers: true, suppressAutoHyphens: true });
    // And back.
    await chooseMenu(page, "Format", "Paragraph…");
    await d.getByRole("tab", { name: "Line and page breaks" }).click();
    await expect(field(page, "pd-keep-next")).toBeChecked();
    await expect(field(page, "pd-widow")).not.toBeChecked();
    await field(page, "pd-keep-next").uncheck();
    await d.getByRole("button", { name: "OK" }).click();
    expect((await fmt(page)).keepNext).toBeFalsy();
  });

  test("a typed length takes its unit, and a paragraph left alone is not rewritten", async ({ page }) => {
    await open(page, [{ text: "Fine as it is", ppr: '<w:ind w:left="1440"/>' }]);
    await chooseMenu(page, "Format", "Paragraph…");
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    // Nothing changed: no step to undo.
    expect(await inPage(page, (w) => w.fe.editor.engine!.undo())).toBe(false);
    await chooseMenu(page, "Format", "Paragraph…");
    await field(page, "pd-left").fill("2.54 cm");
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    expect((await fmt(page)).indLeft).toBe(1440);
  });

  test("a right-to-left paragraph shows its start indent in Before text and writes an edit to the side it names", async ({ page }) => {
    await open(page, [{ text: "Right to left", ppr: '<w:bidi/><w:ind w:left="720" w:right="0"/>' }]);
    await chooseMenu(page, "Format", "Paragraph…");
    await expect(dialog(page, "Paragraph").getByText("Before text (in)")).toBeVisible();
    await expect(field(page, "pd-left")).toHaveValue("0.5");
    await expect(field(page, "pd-right")).toHaveValue("0");
    await field(page, "pd-right").fill("1");
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    expect(await fmt(page)).toMatchObject({ indLeft: 720, indRight: 1440 });
    await chooseMenu(page, "Format", "Paragraph…");
    await expect(field(page, "pd-left")).toHaveValue("0.5");
    await expect(field(page, "pd-right")).toHaveValue("1");
  });

  test("lengths that the fields round are kept exactly when the paragraph is left alone", async ({ page }) => {
    // 567 twips is 1 cm: shown as 0.39 inch, which would parse back to 562.
    await open(page, [{ text: "Metric", ppr: '<w:ind w:left="567" w:right="283" w:hanging="283"/>' }]);
    await chooseMenu(page, "Format", "Paragraph…");
    await expect(field(page, "pd-left")).toHaveValue("0.39");
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    expect(await inPage(page, (w) => w.fe.editor.engine!.undo())).toBe(false);
    expect(await fmt(page)).toMatchObject({ indLeft: 567, indRight: 283, indHanging: 283 });
    // Changing one field leaves the others as they were.
    await chooseMenu(page, "Format", "Paragraph…");
    await field(page, "pd-right").fill("1");
    await dialog(page, "Paragraph").getByRole("button", { name: "OK" }).click();
    expect(await fmt(page)).toMatchObject({ indLeft: 567, indRight: 1440, indHanging: 283 });
  });

  test("Tabs… in the Paragraph dialog opens the Tabs dialog", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Paragraph…");
    await dialog(page, "Paragraph").getByRole("button", { name: "Tabs…" }).click();
    await expect(dialog(page, "Tabs")).toBeVisible();
  });
});

test.describe("Tabs dialog", () => {
  const tabs = (page: Page) => dialog(page, "Tabs");

  test("sets stops with their alignment and leader, and writes them as w:tabs", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Tabs…");
    const d = tabs(page);
    await expect(d).toBeVisible();
    await field(page, "tb-pos").fill("1.5");
    await d.getByRole("radio", { name: "Decimal" }).click();
    await field(page, "tb-leader").selectOption("dot");
    await d.getByRole("button", { name: "Set", exact: true }).click();
    await field(page, "tb-pos").fill("4");
    await d.getByRole("radio", { name: "Right" }).click();
    await field(page, "tb-leader").selectOption("none");
    await d.getByRole("button", { name: "Set", exact: true }).click();
    await expect(field(page, "tb-list").locator("option")).toHaveText(["1.5 in, Decimal, Dots", "4 in, Right"]);
    await d.getByRole("button", { name: "OK" }).click();
    expect((await fmt(page)).tabs).toEqual([
      { val: "decimal", pos: 2160, leader: "dot" },
      { val: "right", pos: 5760 },
    ]);
    // The ruler shows them.
    await expect(page.locator(".ruler-tab.decimal")).toHaveCount(1);
    await expect(page.locator(".ruler-tab.right")).toHaveCount(1);
  });

  test("clears one stop, then all, and the default width is written to the settings", async ({ page }) => {
    await open(page, [{ text: "a{TAB}b", ppr: '<w:tabs><w:tab w:val="left" w:pos="1440"/><w:tab w:val="center" w:pos="4320"/></w:tabs>' }], { settings: '<w:defaultTabStop w:val="720"/>' });
    await chooseMenu(page, "Format", "Tabs…");
    const d = tabs(page);
    await expect(field(page, "tb-list").locator("option")).toHaveCount(2);
    await field(page, "tb-list").selectOption({ index: 0 });
    await expect(field(page, "tb-pos")).toHaveValue("1");
    await d.getByRole("button", { name: "Clear", exact: true }).click();
    await expect(field(page, "tb-list").locator("option")).toHaveCount(1);
    await field(page, "tb-default").fill("1");
    await d.getByRole("button", { name: "OK" }).click();
    expect((await fmt(page)).tabs).toEqual([{ val: "center", pos: 4320 }]);
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().defaultTab)).toBe(1440);
    // One undo step takes both back.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await fmt(page)).tabs).toHaveLength(2);
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().defaultTab)).toBe(720);

    await chooseMenu(page, "Format", "Tabs…");
    await tabs(page).getByRole("button", { name: "Clear all" }).click();
    await expect(field(page, "tb-list").locator("option")).toHaveCount(0);
    await tabs(page).getByRole("button", { name: "OK" }).click();
    expect((await fmt(page)).tabs ?? []).toEqual([]);
  });

  test("a bad distance is refused with a message", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Format", "Tabs…");
    await field(page, "tb-pos").fill("far");
    await tabs(page).getByRole("button", { name: "Set", exact: true }).click();
    await expect(tabs(page).getByRole("alert")).toContainText("Type a distance");
    expect((await fmt(page)).tabs ?? []).toEqual([]);
  });

  test("the default tab width is kept exactly when only a stop is changed", async ({ page }) => {
    // 708 twips (1.25 cm) is shown as 0.49 inch, which would parse back to 706.
    await open(page, ["One", "Two"], { settings: '<w:defaultTabStop w:val="708"/>' });
    await chooseMenu(page, "Format", "Tabs…");
    await expect(field(page, "tb-default")).toHaveValue("0.49");
    await field(page, "tb-pos").fill("2");
    await tabs(page).getByRole("button", { name: "Set", exact: true }).click();
    await tabs(page).getByRole("button", { name: "OK" }).click();
    expect(await savedPart(page, "word/settings.xml")).toContain('w:defaultTabStop w:val="708"');
  });

  test("a list level's number stop is not shown on the ruler or in the dialog, and is not copied into the paragraph", async ({ page }) => {
    const numbering =
      '<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/><w:pPr><w:tabs><w:tab w:val="num" w:pos="720"/></w:tabs><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>';
    await openBuilt(page, { numbering, sections: [{ paragraphs: [{ text: "Item", ppr: '<w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>' }] }] });
    await caretAtEnd(page, 1);
    await expect(page.locator(".ruler-tab")).toHaveCount(0);
    await chooseMenu(page, "Format", "Tabs…");
    await expect(field(page, "tb-list").locator("option")).toHaveCount(0);
    await field(page, "tb-pos").fill("3");
    await tabs(page).getByRole("button", { name: "Set", exact: true }).click();
    await tabs(page).getByRole("button", { name: "OK" }).click();
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('w:pos="4320"');
    expect(xml).not.toContain('w:val="num"');
  });

  test("a paragraph's own number stop is kept when a stop is added in the dialog", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Item", ppr: '<w:tabs><w:tab w:val="num" w:pos="1080"/></w:tabs><w:ind w:left="720" w:hanging="360"/>' }] }] });
    await caretAtEnd(page, 1);
    await chooseMenu(page, "Format", "Tabs…");
    await expect(field(page, "tb-list").locator("option")).toHaveCount(0);
    await field(page, "tb-pos").fill("3");
    await tabs(page).getByRole("button", { name: "Set", exact: true }).click();
    await tabs(page).getByRole("button", { name: "OK" }).click();
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('w:pos="4320"');
    expect(xml).toContain('<w:tab w:val="num" w:pos="1080"/>');
  });
});

test.describe("tab stops on the ruler", () => {
  /** The ruler's left edge and the page margin, in client px. */
  const rulerX = (page: Page) => page.locator(".ruler").evaluate((e) => e.getBoundingClientRect().left);

  test("the kind button cycles through the stops, and a click on the ruler sets one of that kind", async ({ page }) => {
    await open(page);
    const kind = page.locator(".ruler-kind");
    await expect(kind).toHaveAttribute("data-kind", "left");
    await kind.click();
    await expect(kind).toHaveAttribute("data-kind", "center");
    await kind.click();
    await kind.click();
    await expect(kind).toHaveAttribute("data-kind", "decimal");
    await kind.click();
    await expect(kind).toHaveAttribute("data-kind", "bar");
    await kind.click();
    await expect(kind).toHaveAttribute("data-kind", "left");
    await kind.click();
    // Centre tab, 2 inches in from the margin (the margin is 96 px, so 288 px from the ruler's left edge).
    const x = (await rulerX(page)) + 96 + 192;
    const y = (await page.locator(".ruler-track").boundingBox())!.y + 8;
    await page.mouse.click(x, y);
    await expect.poll(async () => (await fmt(page)).tabs).toEqual([{ val: "center", pos: 2880 }]);
    await expect(page.locator(".ruler-tab.center")).toHaveCount(1);
  });

  test("a stop can be dragged to a new place, and dragged off the ruler to remove it", async ({ page }) => {
    await open(page, [{ text: "a{TAB}b", ppr: '<w:tabs><w:tab w:val="left" w:pos="2880"/></w:tabs>' }, "x"]);
    const stop = page.locator(".ruler-tab.left");
    await expect(stop).toHaveCount(1);
    const box = (await stop.boundingBox())!;
    const cx = box.x + 3;
    const cy = box.y + box.height / 2;
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx + 96, cy, { steps: 6 });
    await page.mouse.up();
    await expect.poll(async () => (await fmt(page)).tabs).toEqual([{ val: "left", pos: 4320 }]);
    // The text moved with it.
    const left = await page.locator('.flow [data-p="1"]').evaluate((e) => {
      const walker = document.createTreeWalker(e, NodeFilter.SHOW_TEXT);
      let last: Text | null = null;
      for (let n = walker.nextNode(); n; n = walker.nextNode()) last = n as Text;
      const r = document.createRange();
      r.selectNodeContents(last!);
      return r.getBoundingClientRect().left - e.getBoundingClientRect().left;
    });
    expect(left).toBeCloseTo(4320 / 15, 0);

    const again = (await page.locator(".ruler-tab.left").boundingBox())!;
    await page.mouse.move(again.x + 3, again.y + 5);
    await page.mouse.down();
    await page.mouse.move(again.x + 3, again.y + 90, { steps: 6 });
    await page.mouse.up();
    await expect.poll(async () => (await fmt(page)).tabs ?? []).toEqual([]);
    await expect(page.locator(".ruler-tab")).toHaveCount(0);
  });

  test("double-clicking a stop opens the Tabs dialog", async ({ page }) => {
    await open(page, [{ text: "a{TAB}b", ppr: '<w:tabs><w:tab w:val="right" w:pos="5000"/></w:tabs>' }]);
    await page.locator(".ruler-tab.right").dblclick();
    await expect(dialog(page, "Tabs")).toBeVisible();
    await expect(field(page, "tb-list").locator("option")).toHaveCount(1);
  });
});

test.describe("text direction", () => {
  test("the toolbar's alignment popover has left-to-right and right-to-left, and the Format menu too", async ({ page }) => {
    await open(page, ["Plain text", "More"]);
    await page.getByRole("button", { name: "Align", exact: true }).click();
    const ltr = page.getByRole("button", { name: "Left-to-right" });
    const rtl = page.getByRole("button", { name: "Right-to-left" });
    await expect(ltr).toHaveAttribute("aria-pressed", "true");
    await expect(rtl).toHaveAttribute("aria-pressed", "false");
    await rtl.click();
    expect((await fmt(page)).bidi).toBe(true);
    await expect(page.locator('.flow [data-p="1"]')).toHaveCSS("direction", "rtl");
    // The Format menu's submenu shows the current direction.
    await openMenu(page, "Format");
    await menuItem(page, "Align and indent").click();
    await expect(menuItem(page, "Right-to-left")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Left-to-right").click();
    expect((await fmt(page)).bidi).toBeFalsy();
    await expect(page.locator('.flow [data-p="1"]')).toHaveCSS("direction", "ltr");
  });

  test("Align left in a right-to-left paragraph puts the text on the left, writing right as Word does", async ({ page }) => {
    await open(page, [{ text: "שלום עולם", ppr: "<w:bidi/>" }, "Second"]);
    const p = page.locator('.flow [data-p="1"]');
    // No alignment in the file: the start of the line, which is the right.
    const box = (await p.boundingBox())!;
    const word = async () => (await p.evaluate((e) => {
      const r = document.createRange();
      r.selectNodeContents(e.querySelector(".s")!);
      const b = r.getBoundingClientRect();
      return [b.left, b.right];
    })) as number[];
    const [l0, r0] = await word();
    expect(r0).toBeGreaterThan(box.x + box.width - 5);
    expect(l0).toBeGreaterThan(box.x + 100);
    await chooseMenu(page, "Format", "Align and indent", "Align left");
    expect((await fmt(page)).align).toBe("right");
    const [l1] = await word();
    expect(l1).toBeLessThan(box.x + 5);
    // The toolbar shows what is on screen.
    await page.getByRole("button", { name: "Align", exact: true }).click();
    await expect(page.getByRole("button", { name: "Align left" })).toHaveAttribute("aria-pressed", "true");
  });
});
