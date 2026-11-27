// Quick parts (saved snippets, AutoText, the building blocks a document brings), citation placeholders, and the shared list
// of sources.
import { openBuilt } from "./support/docx-builder";
import { answer, chooseMenu, dialog, documentReady, expect, inPage, openFixture, type Page, test } from "./helpers";

type Eng = { text(): string; renderAll(): { order: number[] }; sources(): { tag: string; title: string; authors: unknown[] }[]; citations(): { tags: string[] }[]; formatAt(p: { p: number; o: number }): { run?: { bold?: boolean } } | null };
type Ed = { select(s: unknown): void; focus(): void; sel: { focus: { p: number; o: number } } };

const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().split("\n"));
const order = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);

const select = (page: Page, p: number, from: number, to: number) =>
  inPage(
    page,
    (w, a) => {
      const e = w.fe.editor as unknown as Ed;
      e.select({ anchor: { p: a.p, o: a.from }, focus: { p: a.p, o: a.to } });
      e.focus();
    },
    { p, from, to },
  );
const caret = (page: Page, p: number, o: number) => select(page, p, o, o);

const BOLD = '<w:r><w:rPr><w:b/></w:rPr><w:t>Kind regards,</w:t></w:r>';

const GLOSSARY = {
  parts:
    '<w:docPart><w:docPartPr><w:name w:val="Sign-off"/><w:category><w:name w:val="General"/><w:gallery w:val="autoText"/></w:category><w:types><w:type w:val="autoText"/></w:types><w:behaviors><w:behavior w:val="content"/></w:behaviors><w:description w:val="Closing line"/><w:guid w:val="{11111111-2222-3333-4444-555555555555}"/></w:docPartPr><w:docPartBody><w:p><w:r><w:rPr><w:i/></w:rPr><w:t>With thanks,</w:t></w:r></w:p></w:docPartBody></w:docPart>' +
    '<w:docPart><w:docPartPr><w:name w:val="Notice"/><w:category><w:name w:val="Legal"/><w:gallery w:val="docParts"/></w:category><w:types><w:type w:val="bbPlcHdr"/></w:types><w:behaviors><w:behavior w:val="content"/></w:behaviors><w:guid w:val="{AAAAAAAA-2222-3333-4444-555555555555}"/></w:docPartPr><w:docPartBody><w:p><w:pPr><w:pStyle w:val="Notice"/></w:pPr><w:r><w:t>This is a notice.</w:t></w:r></w:p><w:p><w:r><w:t>Second line.</w:t></w:r></w:p></w:docPartBody></w:docPart>',
  styles: '<w:style w:type="paragraph" w:customStyle="1" w:styleId="Notice"><w:name w:val="Notice"/><w:basedOn w:val="Normal"/><w:rPr><w:i/></w:rPr></w:style>',
};

const parts = (page: Page) => dialog(page, "Quick parts");

async function start(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: BOLD }, "Dear reader,", "Closing: "] }] });
  return order(page);
}

test.describe("quick parts", () => {
  test("Save selection keeps the text and its formatting; the gallery lists it and inserts it again", async ({ page }) => {
    const ids = await start(page);
    await select(page, ids[0], 0, 13);
    await chooseMenu(page, "Insert", "Quick parts", "Save selection to Quick parts…");
    const d = dialog(page, "Save selection to Quick parts");
    await expect(d.getByLabel("Name")).toHaveValue("Kind regards,");
    await d.getByLabel("Name").fill("Sign-off");
    await d.getByLabel("Gallery").fill("AutoText");
    await d.getByLabel("Category").fill("Letters");
    await d.getByLabel("Description").fill("The usual ending");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(page.locator(".toast")).toContainText("Saved to AutoText");

    await caret(page, ids[2], 9);
    await chooseMenu(page, "Insert", "Quick parts", "Quick parts…");
    const list = parts(page);
    await expect(list.locator(".qp-group")).toHaveText(["AutoText"]);
    await list.getByRole("option", { name: /Sign-off/ }).click();
    await expect(list.locator(".qp-detail")).toContainText("The usual ending");
    await expect(list.locator(".qp-detail")).toContainText("Letters");
    await list.getByRole("button", { name: "Insert" }).click();
    expect(await text(page)).toContain("Closing: Kind regards,");
    // Bold came with it.
    const bold = await inPage(page, (w, p) => (w.fe.editor.engine as unknown as Eng).formatAt({ p, o: 12 })?.run?.bold === true, ids[2]);
    expect(bold).toBe(true);
    // One undo takes it out.
    await page.keyboard.press("ControlOrMeta+Z");
    expect(await text(page)).toContain("Closing: ");
  });

  test("The part is kept for the next document: the app's own store lives in the browser", async ({ page }) => {
    const ids = await start(page);
    await select(page, ids[1], 0, 4);
    await chooseMenu(page, "Insert", "Quick parts", "Save selection to Quick parts…");
    await dialog(page, "Save selection to Quick parts").getByRole("button", { name: "Save" }).click();
    await page.goto("/?open=simple.docx");
    await documentReady(page);
    await chooseMenu(page, "Insert", "Quick parts", "Quick parts…");
    await expect(parts(page).getByRole("option", { name: /Dear/ })).toBeVisible();
    await expect(parts(page).locator(".qp-group")).toHaveText(["Quick parts"]);
  });

  test("Edit properties changes name, gallery, category and description; Delete takes it away", async ({ page }) => {
    const ids = await start(page);
    await select(page, ids[1], 0, 11);
    await chooseMenu(page, "Insert", "Quick parts", "Save selection to Quick parts…");
    await dialog(page, "Save selection to Quick parts").getByRole("button", { name: "Save" }).click();
    await chooseMenu(page, "Insert", "Quick parts", "Quick parts…");
    const list = parts(page);
    await list.getByRole("option", { name: /Dear reader/ }).click();
    await list.getByRole("button", { name: "Edit properties…" }).click();
    await list.getByLabel("Name").fill("Greeting");
    await list.getByLabel("Gallery").fill("Letters");
    await list.getByLabel("Category").fill("Openers");
    await list.getByLabel("Description").fill("Says hello");
    await list.getByRole("button", { name: "Save" }).click();
    await expect(list.locator(".qp-group")).toHaveText(["Letters"]);
    await expect(list.getByRole("option", { name: /Greeting/ })).toBeVisible();
    await expect(list.locator(".qp-detail")).toContainText("Says hello");
    await list.getByLabel("Search quick parts").fill("hello");
    await expect(list.getByRole("option")).toHaveCount(1);
    await list.getByLabel("Search quick parts").fill("zzz");
    await expect(list.getByRole("option")).toHaveCount(0);
    await list.getByLabel("Search quick parts").fill("");
    await list.getByRole("option", { name: /Greeting/ }).click();
    await list.getByRole("button", { name: "Delete" }).click();
    await answer(page, "Delete");
    await expect(list.getByRole("option")).toHaveCount(0);
    await expect(list).toContainText("Nothing saved yet");
  });

  test("Ctrl+Alt+F3 expands the AutoText named by the word before the caret", async ({ page }) => {
    const ids = await start(page);
    await select(page, ids[1], 0, 4);
    await chooseMenu(page, "Insert", "Quick parts", "Save selection to Quick parts…");
    const d = dialog(page, "Save selection to Quick parts");
    await d.getByLabel("Name").fill("dr");
    await d.getByLabel("Gallery").fill("AutoText");
    await d.getByRole("button", { name: "Save" }).click();
    // Type the name in the last paragraph and press F3.
    await caret(page, ids[2], 9);
    await page.keyboard.type("dr");
    await page.keyboard.press("ControlOrMeta+Alt+F3");
    await expect.poll(async () => (await text(page)).includes("Closing: Dear")).toBe(true);
    // With no AutoText of that name, the shortcut says so.
    await page.keyboard.type(" zz");
    await page.keyboard.press("ControlOrMeta+Alt+F3");
    await expect(page.locator(".toast")).toContainText("No AutoText has that name");
  });

  test("Without a place to keep them, saving says so", async ({ page }) => {
    const ids = await start(page);
    await inPage(page, (w) => {
      const host = (w as unknown as { fe: { extensions: { disable(id: string): Promise<void> } } }).fe.extensions;
      return host.disable("docxtor.snippets.local");
    });
    await select(page, ids[1], 0, 4);
    await chooseMenu(page, "Insert", "Quick parts", "Save selection to Quick parts…");
    await expect(page.locator(".toast")).toContainText("nowhere to save quick parts");
  });
});

test.describe("building blocks of a document", () => {
  test("A glossary document's blocks are listed under their galleries, insert with their styles, and can't be changed", async ({ page }) => {
    await openBuilt(page, { glossary: GLOSSARY, sections: [{ paragraphs: ["Dear reader,", "Closing: "] }] });
    const ids = await order(page);
    await caret(page, ids[1], 9);
    await chooseMenu(page, "Insert", "Quick parts", "Quick parts…");
    const list = parts(page);
    await expect(list.locator(".qp-group")).toHaveText(["AutoText", "Quick parts"]);
    await list.getByRole("option", { name: /Notice/ }).click();
    await expect(list.getByRole("button", { name: "Edit properties…" })).toBeDisabled();
    await expect(list.getByRole("button", { name: "Delete" })).toBeDisabled();
    await expect(list.locator(".qp-detail")).toContainText("inserted but not changed");
    await list.getByRole("option", { name: /Sign-off/ }).click();
    await expect(list.locator(".qp-detail")).toContainText("Closing line");
    await list.getByRole("button", { name: "Insert" }).click();
    expect(await text(page)).toContain("Closing: With thanks,");
    // The block with a style of its own comes in with that style.
    await caret(page, ids[0], 0);
    await chooseMenu(page, "Insert", "Quick parts", "Quick parts…");
    await parts(page).getByRole("option", { name: /Notice/ }).click();
    await parts(page).getByRole("button", { name: "Insert" }).click();
    const lines = await text(page);
    expect(lines).toContain("This is a notice.");
    expect(lines).toContain("Second line.Dear reader,");
    const styled = page.locator(".flow [data-p]").filter({ hasText: "This is a notice." }).first();
    await expect(styled).toHaveCSS("font-style", "italic");
    // Ctrl+Alt+F3 also finds the document's own AutoText by name.
    await caret(page, ids[1], 100);
    await page.keyboard.type(" Sign-off");
    await page.keyboard.press("ControlOrMeta+Alt+F3");
    await expect.poll(async () => (await text(page)).some((l) => l.endsWith("With thanks, With thanks,"))).toBe(true);
  });

  test("A glossary with no matching blocks leaves the list empty", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await chooseMenu(page, "Insert", "Quick parts", "Quick parts…");
    await expect(parts(page)).toContainText("Nothing saved yet");
  });
});

