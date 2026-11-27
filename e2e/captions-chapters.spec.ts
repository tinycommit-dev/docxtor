// Captions numbered by chapter ("Figure 2-1"), AutoCaption, and cross-references to numbered items.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

type Eng = { text(): string; renderAll(): { order: number[] }; xrefTargets(): { captions: { label: string; number: string; text: string }[] } };
type Ed = { select(s: unknown): void; focus(): void };

const NUMBERED = {
  numbering:
    '<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="432" w:hanging="432"/></w:pPr></w:lvl></w:abstractNum>' +
    '<w:abstractNum w:abstractNumId="1"><w:multiLevelType w:val="multilevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2)"/><w:lvlJc w:val="left"/></w:lvl><w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="lowerRoman"/><w:lvlText w:val="%3."/><w:lvlJc w:val="left"/></w:lvl></w:abstractNum>' +
    '<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>',
};
const HEAD = (n: number) => `<w:pStyle w:val="Heading${n}"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>`;
const ITEM = (lvl: number) => `<w:numPr><w:ilvl w:val="${lvl}"/><w:numId w:val="2"/></w:numPr>`;

const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().split("\n"));
const captions = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).xrefTargets().captions);

async function caretIn(page: Page, needle: string) {
  await inPage(
    page,
    (w, n) => {
      const eng = w.fe.editor.engine!;
      const ids = (eng as unknown as Eng).renderAll().order;
      const lines = eng.text().split("\n");
      const p = ids[lines.findIndex((l) => l.includes(n))];
      const e = w.fe.editor as unknown as Ed;
      e.select({ p, o: 0 });
      e.focus();
    },
    needle,
  );
}

const startChapters = (page: Page) =>
  openBuilt(page, { ...NUMBERED, sections: [{ paragraphs: [{ text: "Intro", ppr: HEAD(1) }, "Body one.", "Body two.", { text: "Results", ppr: HEAD(1) }, "Body three."] }] });

test.describe("captions numbered by chapter", () => {
  test("Include chapter number gives Figure 1-1, restarts under the next heading, and the separator can change", async ({ page }) => {
    await startChapters(page);
    await caretIn(page, "Body one.");
    await chooseMenu(page, "Insert", "References", "Caption…");
    const d = dialog(page, "Caption");
    await d.getByLabel("Caption text").fill("A chart");
    await expect(d.locator(".cap-preview")).toHaveText("Figure 1: A chart");
    await d.getByLabel("Include chapter number").check();
    await expect(d.locator(".cap-preview")).toHaveText("Figure 1-1: A chart");
    await d.getByRole("button", { name: "Insert" }).click();
    expect(await text(page)).toContain("Figure 1-1: A chart");

    // The second one in chapter 1 is 1-2, and the preview says so with another separator.
    await caretIn(page, "Body two.");
    await chooseMenu(page, "Insert", "References", "Caption…");
    const d2 = dialog(page, "Caption");
    // The label already has chapter numbers, so the box starts ticked with the separator in use.
    await expect(d2.getByLabel("Include chapter number")).toBeChecked();
    await d2.getByLabel("Separator").selectOption(".");
    await expect(d2.locator(".cap-preview")).toHaveText("Figure 1.2");
    await d2.getByLabel("Separator").selectOption("-");
    await d2.getByRole("button", { name: "Insert" }).click();

    // Under the second heading the count starts again.
    await caretIn(page, "Body three.");
    await chooseMenu(page, "Insert", "References", "Caption…");
    const d3 = dialog(page, "Caption");
    await expect(d3.locator(".cap-preview")).toHaveText("Figure 2-1");
    await d3.getByRole("button", { name: "Insert" }).click();
    const lines = await text(page);
    expect(lines.filter((l) => l.startsWith("Figure"))).toEqual(["Figure 1-1: A chart", "Figure 1-2", "Figure 2-1"]);
    expect((await captions(page)).map((c) => c.number)).toEqual(["1-1", "1-2", "2-1"]);
    // The page shows the caption as one number.
    await expect(page.locator(".flow").getByText("Figure 2-1", { exact: true })).toBeVisible();
    // The layout leaves the chapter number alone: no field result is hidden behind an empty value.
    await expect(page.locator(".flow .fval")).toHaveCount(0);
    const shown = await page.locator(".flow .fp", { hasText: /^Figure 2-1$/ }).evaluate((root) => {
      const walk = (n: Node): string => {
        if (n.nodeType === Node.TEXT_NODE) return getComputedStyle((n as Text).parentElement!).fontSize === "0px" ? "" : (n as Text).data;
        return Array.from(n.childNodes).map(walk).join("");
      };
      return walk(root).replace(/\s+/g, " ").trim();
    });
    expect(shown).toBe("Figure 2-1");
  });

  test("Without numbered headings the box is off and says how to turn it on", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "Plain", ppr: '<w:pStyle w:val="Heading1"/>' }, "Body one."] }] });
    await caretIn(page, "Body one.");
    await chooseMenu(page, "Insert", "References", "Caption…");
    const d = dialog(page, "Caption");
    await expect(d.getByLabel("Include chapter number")).toBeDisabled();
    await expect(d.getByText("Number the headings first")).toBeVisible();
    await d.getByRole("button", { name: "Cancel" }).click();
  });
});

test.describe("AutoCaption", () => {
  test("Choosing a label for tables adds a caption each time a table goes in, above it", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First paragraph.", "Second paragraph."] }] });
    await caretIn(page, "First paragraph.");
    await chooseMenu(page, "Insert", "References", "AutoCaption…");
    const d = dialog(page, "AutoCaption");
    await d.getByLabel("Label for Tables").selectOption("Table");
    await expect(d.getByLabel("Position for Tables")).toHaveValue("above");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(d).toHaveCount(0);

    await caretIn(page, "First paragraph.");
    await chooseMenu(page, "Insert", "Table");
    const lines = await text(page);
    const at = lines.findIndex((l) => l === "Table 1");
    expect(at).toBeGreaterThan(-1);
    // The caption is above the table.
    const cap = await page.locator(".flow [data-p]").filter({ hasText: /^Table 1$/ }).boundingBox();
    const table = await page.locator(".flow .ftw").first().boundingBox();
    expect(cap!.y).toBeLessThan(table!.y);
    expect(await captions(page)).toEqual([expect.objectContaining({ label: "Table", number: "1" })]);
  });

  test("The Caption dialog opens AutoCaption", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["A paragraph."] }] });
    await caretIn(page, "A paragraph.");
    await chooseMenu(page, "Insert", "References", "Caption…");
    await dialog(page, "Caption").getByRole("button", { name: "AutoCaption…" }).click();
    await expect(dialog(page, "AutoCaption")).toBeVisible();
  });
});

test.describe("cross-references to numbered items", () => {
  test("Numbered item lists the list items; the number with and without context is inserted as a live field", async ({ page }) => {
    await openBuilt(page, { ...NUMBERED, sections: [{ paragraphs: [{ text: "Alpha", ppr: ITEM(0) }, { text: "Beta", ppr: ITEM(1) }, { text: "Gamma", ppr: ITEM(2) }, "See item: "] }] });
    await caretIn(page, "See item");
    await inPage(page, (w) => {
      const e = w.fe.editor as unknown as Ed & { sel: { focus: { p: number } } };
      const p = e.sel.focus.p;
      e.select({ p, o: 10 });
      e.focus();
    });
    await chooseMenu(page, "Insert", "References", "Cross-reference…");
    const d = dialog(page, "Cross-reference");
    await d.getByLabel("Reference type").selectOption("numbered");
    await expect(d.locator(".xr-item")).toHaveText([/Alpha/, /Beta/, /Gamma/]);
    await d.locator(".xr-item").filter({ hasText: "Gamma" }).click();
    await expect(d.getByLabel("Insert reference to")).toHaveValue("text");
    await expect(d.getByLabel("Insert reference to").locator("option")).toHaveText(["Paragraph text", "Paragraph number", "Paragraph number (no context)", "Paragraph number (full context)", "Page number"]);
    await d.getByLabel("Insert reference to").selectOption("number_full_context");
    await d.getByRole("button", { name: "Insert" }).click();
    expect(await text(page)).toContain("See item: 1.a)i");

    // Without context it is only the item's own number.
    await inPage(page, (w) => {
      const e = w.fe.editor as unknown as Ed & { sel: { focus: { p: number } } };
      e.select({ p: e.sel.focus.p, o: 0 });
      e.focus();
    });
    await chooseMenu(page, "Insert", "References", "Cross-reference…");
    const d2 = dialog(page, "Cross-reference");
    await d2.getByLabel("Reference type").selectOption("numbered");
    await d2.locator(".xr-item").filter({ hasText: "Gamma" }).click();
    await d2.getByLabel("Insert reference to").selectOption("number_no_context");
    await d2.getByRole("button", { name: "Insert" }).click();
    expect(await text(page)).toContain("iSee item: 1.a)i");
    // Update fields keeps them.
    await chooseMenu(page, "Insert", "References", "Update fields");
    expect(await text(page)).toContain("iSee item: 1.a)i");
  });
});
