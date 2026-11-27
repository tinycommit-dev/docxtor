// Templates: File, Save as template (kept as a Word template, .dotx), New from a template (yours, the ones that come with
// Docxtor, or a template file from your computer), Download as a Word template, opening and saving a .dotx, and Modify
// style's "New documents based on this template".
import { readFileSync } from "node:fs";
import { chooseMenu, dialog, documentReady, expect, inPage, menuItem, openFixture, openMenu, test, type Page } from "./helpers";
import { zip } from "./support/docx-builder";
import { partOf } from "./support/zip-read";

const TEMPLATE = "wordprocessingml.template.main+xml";
const DOCUMENT = "wordprocessingml.document.main+xml";

const contentTypes = (bytes: ArrayLike<number>) => partOf(bytes, "[Content_Types].xml") ?? "";

/** The templates this browser keeps, as the app's store has them. */
const stored = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<{ name: string; description?: string; bytes: number[] }[]>((resolve, reject) => {
        const open = indexedDB.open("docxtor");
        open.onerror = () => reject(open.error);
        open.onsuccess = () => {
          const all = open.result.transaction("templates", "readonly").objectStore("templates").getAll();
          all.onsuccess = () => resolve((all.result as { name: string; description?: string; bytes: Uint8Array }[]).map((r) => ({ ...r, bytes: Array.from(r.bytes) })));
          all.onerror = () => reject(all.error);
        };
      }),
  );

/** A file from the Library, as the page keeps it. */
const libraryFile = (page: Page, path: string) => inPage(page, async (w, p) => Array.from(await w.files.library.read(p)), path);

async function saveAsTemplate(page: Page, name: string, description = "") {
  await chooseMenu(page, "File", "Save as template…");
  const d = dialog(page, "Save as template");
  await expect(d).toBeVisible();
  await d.getByLabel("Name", { exact: true }).fill(name);
  if (description) await d.getByLabel("Description", { exact: true }).fill(description);
  await d.getByRole("button", { name: "Save", exact: true }).click();
  await expect(d).toBeHidden();
}

test.describe("templates", () => {
  test("Save as template keeps the document as a Word template, and New from a template lists it beside the built-in ones", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await saveAsTemplate(page, "Product report", "Quarterly format");
    await expect(page.locator(".toast")).toContainText("Saved the template “Product report”.");
    const all = await stored(page);
    expect(all).toHaveLength(1);
    expect(all[0]).toMatchObject({ name: "Product report", description: "Quarterly format" });
    // A template file: its main part has the template content type, and the rest of the package is the document's.
    expect(contentTypes(all[0].bytes)).toContain(TEMPLATE);
    expect(contentTypes(all[0].bytes)).not.toContain(DOCUMENT);
    expect(partOf(all[0].bytes, "word/document.xml")).toContain("Quarterly Product Report");

    await chooseMenu(page, "File", "New from a template…");
    const d = dialog(page, "New from a template");
    await expect(d.getByRole("heading", { name: "Your templates" })).toBeVisible();
    await expect(d.getByRole("heading", { name: "Built in" })).toBeVisible();
    await expect(d.getByRole("listitem").filter({ hasText: "Product report" })).toContainText("Quarterly format");
    for (const name of ["Letter", "Meeting notes", "Report", "Paper"]) await expect(d.getByRole("listitem").filter({ hasText: new RegExp(`^${name}`) })).toBeVisible();
    await d.getByRole("button", { name: "Use Product report" }).click();
    // A new document, made now, with the template's text and styles: a document, not a template.
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Product report");
    await documentReady(page);
    await expect(page.locator(".flow .fp").first()).toContainText("Quarterly Product Report");
    const made = await libraryFile(page, "Product report.docx");
    expect(contentTypes(made)).toContain(DOCUMENT);
    expect(contentTypes(made)).not.toContain(TEMPLATE);
    const props = await inPage(page, (w) => w.fe.editor.engine!.docProps());
    expect(props.modified).toBeUndefined();
    expect(Date.now() - new Date(props.created ?? 0).getTime()).toBeLessThan(120_000);
  });

  test("a built-in template still makes a document", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await chooseMenu(page, "File", "New from a template…");
    await dialog(page, "New from a template").getByRole("button", { name: "Use Meeting notes" }).click();
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Meeting notes");
    await expect(page.locator(".flow .fp").first()).toContainText("Meeting notes");
  });

  test("a template you saved can be deleted from the dialog, and the built-in ones can't", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await saveAsTemplate(page, "Temporary");
    await chooseMenu(page, "File", "New from a template…");
    const d = dialog(page, "New from a template");
    await expect(d.getByRole("button", { name: "Delete Letter" })).toHaveCount(0);
    await d.getByRole("button", { name: "Delete Temporary" }).click();
    await page.getByRole("alertdialog", { name: /Delete “Temporary”\?/ }).getByRole("button", { name: "Delete" }).click();
    await expect(d.getByRole("listitem").filter({ hasText: "Temporary" })).toHaveCount(0);
    expect(await stored(page)).toHaveLength(0);
    await expect(d.getByRole("heading", { name: "Your templates" })).toHaveCount(0);
  });

  test("Save as template asks for a name", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await chooseMenu(page, "File", "Save as template…");
    const d = dialog(page, "Save as template");
    await expect(d.getByLabel("Name", { exact: true })).toHaveValue("simple");
    await d.getByLabel("Name", { exact: true }).fill("");
    await d.getByRole("button", { name: "Save", exact: true }).click();
    await expect(d.getByRole("alert")).toHaveText("Give the template a name.");
    expect(await stored(page)).toHaveLength(0);
  });

  test("a template file from your computer starts a document", async ({ page }) => {
    await openFixture(page, "rich.docx");
    // A template file as the download makes it.
    const asTemplate = await inPage(page, async (w) => {
      const fe = w.fe as unknown as { extensions: { exporter(id: string): { export(e: unknown): Promise<Uint8Array> } }; editor: { engine: unknown } };
      return Array.from(await fe.extensions.exporter("dotx").export(fe.editor.engine));
    });
    await chooseMenu(page, "File", "New from a template…");
    const d = dialog(page, "New from a template");
    const chooser = page.waitForEvent("filechooser");
    await d.getByRole("button", { name: "From a file…" }).click();
    await (await chooser).setFiles({ name: "Memo.dotx", mimeType: "application/octet-stream", buffer: Buffer.from(asTemplate) });
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Memo");
    await documentReady(page);
    await expect(page.locator(".flow .fp").first()).toContainText("Quarterly Product Report");
    const made = await libraryFile(page, "Memo.docx");
    expect(contentTypes(made)).toContain(DOCUMENT);
  });

  test("a macro-enabled template makes a .docm, so the name and the main part agree", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const types = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.ms-word.template.macroEnabledTemplate.main+xml"/></Types>';
    const rels = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>';
    const body = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Macro memo</w:t></w:r></w:p><w:sectPr/></w:body></w:document>';
    const dotm = Array.from(
      zip([
        ["[Content_Types].xml", types],
        ["_rels/.rels", rels],
        ["word/document.xml", body],
      ]),
    );
    // What "New from a template" does with a stored template of that kind: the entry is named .docx, the bytes are a macro template.
    await inPage(page, (w, bytes) => (w as unknown as { createFromSource(s: { docx: Uint8Array }, name: string): Promise<void> }).createFromSource({ docx: new Uint8Array(bytes) }, "Report.docx"), dotm);
    await documentReady(page);
    await expect(page.locator(".flow .fp").first()).toContainText("Macro memo");
    const names = await inPage(page, async (w) => (await w.files.library.list("")).map((n) => n.name));
    expect(names).toContain("Report.docm");
    expect(names).not.toContain("Report.docx");
    const made = await libraryFile(page, "Report.docm");
    expect(contentTypes(made)).toContain("application/vnd.ms-word.document.macroEnabled.main+xml");
  });

  test("File, Download offers a Word template (.dotx) that is a template file", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openMenu(page, "File");
    await menuItem(page, "Download").click();
    const [download] = await Promise.all([page.waitForEvent("download"), menuItem(page, "Word template (.dotx)").click()]);
    expect(download.suggestedFilename()).toBe("rich.dotx");
    const bytes = readFileSync((await download.path())!);
    expect(contentTypes(bytes)).toContain(TEMPLATE);
    expect(partOf(bytes, "word/document.xml")).toContain("Quarterly Product Report");
    // The other parts are the document's own, untouched.
    expect(partOf(bytes, "word/styles.xml")).toBeTruthy();
  });

  test("a .dotx opens for editing and is saved as a template again", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const path = "Letterhead.dotx";
    await inPage(page, async (w) => {
      const docxtor = w as unknown as { fe: { extensions: { exporter(id: string): { export(e: unknown): Promise<Uint8Array> } }; editor: { engine: unknown } }; files: { library: { write(p: string, b: Uint8Array): Promise<void> } }; openRef(r: unknown): Promise<void> };
      const bytes = await docxtor.fe.extensions.exporter("dotx").export(docxtor.fe.editor.engine);
      await docxtor.files.library.write("Letterhead.dotx", bytes);
      await docxtor.openRef({ backend: docxtor.files.library, path: "Letterhead.dotx" });
    });
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Letterhead");
    await documentReady(page);
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "Edited " });
    });
    await page.keyboard.press("ControlOrMeta+s");
    await expect.poll(async () => partOf(await libraryFile(page, path), "word/document.xml") ?? "").toContain("Edited ");
    expect(contentTypes(await libraryFile(page, path))).toContain(TEMPLATE);
  });

  test("Modify style can keep a style for new documents, for the styles the default design holds", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_markdown", before: e.engine!.renderAll().order[0], markdown: "# Heading\n\nBody" });
    });
    const heading = page.locator(".flow .fp").filter({ hasText: "Heading" }).first();
    await heading.click();
    await chooseMenu(page, "Format", "Paragraph styles", "Modify style…");
    const d = dialog(page, "Modify style");
    const keep = d.getByRole("radio", { name: "New documents based on this template" });
    await expect(keep).toBeEnabled();
    await d.locator("#ms-size").fill("33");
    await d.locator("#ms-size").dispatchEvent("change");
    await keep.check();
    await d.getByRole("button", { name: "OK" }).click();
    await expect(page.locator(".toast")).toContainText("New documents will start with this style too.");
    // A new document has the style as it was changed; the one before it is as it was.
    await chooseMenu(page, "File", "New document");
    await expect.poll(() => inPage(page, (w) => w.engine!.fileName)).toMatch(/^Untitled/);
    const size = await inPage(page, (w) => (w.fe.editor.engine as unknown as { styleDetails(s: string): { run: { size?: number } } }).styleDetails("Heading1").run.size);
    expect(size).toBe(33);
  });

  test("a style outside the default design is not offered for new documents, and the dialog says how to keep it", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await inPage(page, (w) => w.fe.editor.exec({ op: "set_style_definition", def: { name: "Aside", kind: "paragraph" } }));
    await inPage(page, (w) => (w.fe as unknown as { extensions: { execute(id: string, ...a: unknown[]): Promise<unknown> } }).extensions.execute("format.modifyStyle", "Aside"));
    const d = dialog(page, "Modify style");
    await expect(d.getByRole("radio", { name: "New documents based on this template" })).toBeDisabled();
    await expect(d).toContainText("save the document as a template");
  });
});
