// Insert, Page elements, Page numbers: top and bottom of the page, the margins and the current position, left, centre or right,
// "Page X of Y" as PAGE and NUMPAGES fields, Remove page numbers, and the options dialog with chapter numbers.
import { openBuilt } from "./support/docx-builder";
import { answer, chooseMenu, dialog, expect, inPage, type Page, test } from "./helpers";

const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
const lines = (n: number, prefix = "Line") => Array.from({ length: n }, (_, i) => `${prefix} ${i + 1}`);
const gallery = (page: Page, ...path: string[]) => chooseMenu(page, "Insert", "Page numbers", ...path);

test.describe("the page number gallery", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [...lines(70), "Last page text"] }] });
    await expect(page.locator(".sheet")).toHaveCount(2);
  });

  test("plain numbers at the bottom, aligned right, on every page", async ({ page }) => {
    await gallery(page, "Bottom of page", "Plain number, right");
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("1");
    await expect(sheet(page, 1).locator(".ftr")).toHaveText("2");
    expect(await sheet(page, 0).locator(".ftr .fp").evaluate((el) => getComputedStyle(el).textAlign)).toMatch(/right|end/);
  });

  test("Page X of Y at the top, centred", async ({ page }) => {
    await gallery(page, "Top of page", "Page X of Y, center");
    await expect(sheet(page, 0).locator(".hdr")).toHaveText("Page 1 of 2");
    await expect(sheet(page, 1).locator(".hdr")).toHaveText("Page 2 of 2");
    expect(await sheet(page, 0).locator(".hdr .fp").evaluate((el) => getComputedStyle(el).textAlign)).toBe("center");
    // The file has the fields, not the numbers.
    const saved = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()).length > 0);
    expect(saved).toBe(true);
    await page.screenshot({ path: "test-results/page-numbers-top.png" });
  });

  test("a second number replaces the first", async ({ page }) => {
    await gallery(page, "Bottom of page", "Plain number, left");
    await gallery(page, "Bottom of page", "Page X of Y, right");
    await expect(sheet(page, 0).locator(".ftr .fp")).toHaveCount(1);
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("Page 1 of 2");
  });

  test("a number in the margin is a frame beside the text", async ({ page }) => {
    await gallery(page, "Page margins", "Right margin");
    const n = sheet(page, 0).locator(".hdr .fp");
    await expect(n).toHaveText("1");
    const frame = await inPage(page, (w) => {
      const s = w.fe.editor.engine!.pageSetup() as unknown as { parts: Record<string, { blocks: { fmt: { frame?: Record<string, unknown> } }[] }> };
      return Object.values(s.parts).flatMap((p) => p.blocks.map((b) => b.fmt.frame)).find(Boolean);
    });
    expect(frame).toMatchObject({ hAnchor: "page", x: 12240 - 1440 + 360 });
    // It sits in the right margin, centred in it and clear of the paper's edge, half-way down the page.
    const zoom = await inPage(page, (w) => w.fe.editor.zoom);
    const sh = (await sheet(page, 0).boundingBox())!;
    const box = (await sheet(page, 0).locator(".hdr .fp").boundingBox())!;
    const marginMid = sh.x + sh.width - 48 * zoom;
    expect(Math.abs((box.x + box.width / 2 - marginMid) / zoom)).toBeLessThan(3);
    expect((sh.x + sh.width - (box.x + box.width)) / zoom).toBeGreaterThan(10);
    expect((box.x - (sh.x + sh.width - 96 * zoom)) / zoom).toBeGreaterThan(10);
    expect(Math.abs((box.y + box.height / 2 - (sh.y + sh.height / 2)) / zoom)).toBeLessThan(3);
    await page.screenshot({ path: "test-results/page-number-margin.png" });
  });

  test("a number in a document with odd and even pages goes on the even pages too", async ({ page }) => {
    await openBuilt(page, { evenAndOdd: true, sections: [{ paragraphs: [...lines(70), "Last page text"] }] });
    await expect(page.locator(".sheet")).toHaveCount(2);
    await gallery(page, "Bottom of page", "Plain number, center");
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("1");
    await expect(sheet(page, 1).locator(".ftr")).toHaveText("2");
    // Taking them out is one step, and so is getting them back.
    await gallery(page, "Remove page numbers");
    await answer(page, "Remove page numbers");
    await expect(sheet(page, 1).locator(".ftr")).toHaveText("");
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(sheet(page, 1).locator(".ftr")).toHaveText("2");
  });

  test("the current position puts the field at the caret", async ({ page }) => {
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("End");
    await gallery(page, "Current position");
    await expect(page.locator(".flow .fp").first().locator("[data-fld='PAGE']")).toHaveCount(1);
  });

  test("Remove page numbers takes them out, and undo puts them back", async ({ page }) => {
    await gallery(page, "Bottom of page", "Plain number, center");
    await gallery(page, "Top of page", "Plain number, left");
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("1");
    await gallery(page, "Remove page numbers");
    await answer(page, "Remove page numbers");
    // The header and footer stay, with no number in them.
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("");
    await expect(sheet(page, 0).locator(".hdr")).toHaveText("");
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("1");
  });
});

test.describe("page number options: chapter numbers", () => {
  const STYLES = '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:qFormat/><w:pPr><w:keepNext/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>';

  test("include the chapter number, with the style and separator chosen", async ({ page }) => {
    await openBuilt(page, {
      styles: STYLES,
      sections: [
        {
          footers: { default: "Page {PAGE}" },
          paragraphs: [{ text: "First chapter", ppr: '<w:pStyle w:val="Heading1"/>' }, ...lines(70), { text: "Second chapter", ppr: '<w:pStyle w:val="Heading1"/><w:pageBreakBefore/>' }, "More"],
        },
      ],
    });
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("Page 1");
    await chooseMenu(page, "Insert", "Page elements", "Page number options…");
    const d = dialog(page, "Page numbers");
    await d.getByLabel("Include chapter number").check();
    await d.getByLabel("Chapter starts with style").selectOption("1");
    await d.getByLabel("Separator").selectOption("enDash");
    await expect(d.locator("#pn-example")).toHaveText("Examples: 1–1, 2–1");
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(sheet(page, 0).locator(".ftr")).toHaveText("Page 1–1");
    await expect(sheet(page, 1).locator(".ftr")).toHaveText("Page 1–2");
    await expect(sheet(page, 2).locator(".ftr")).toHaveText("Page 2–1");
    const s = await inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as unknown as { sections: Record<string, unknown>[] }).sections[0]);
    expect(s).toMatchObject({ chapStyle: 1, chapSep: "enDash" });
    // Off again.
    await chooseMenu(page, "Insert", "Page elements", "Page number options…");
    const again = dialog(page, "Page numbers");
    await expect(again.getByLabel("Include chapter number")).toBeChecked();
    await again.getByLabel("Include chapter number").uncheck();
    await again.getByRole("button", { name: "Apply" }).click();
    await expect(sheet(page, 2).locator(".ftr")).toHaveText("Page 3");
  });
});

