// File, Document details: the properties a .docx carries (author, subject, keywords, category, status, company, manager,
// comments and custom properties), the name saved with changes, and the statistics Word keeps in docProps/app.xml.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, openFixture, test, type Page } from "./helpers";
import { partOf } from "./support/zip-read";

interface Props {
  title?: string;
  subject?: string;
  author?: string;
  keywords?: string;
  description?: string;
  category?: string;
  status?: string;
  company?: string;
  manager?: string;
  lastModifiedBy?: string;
  custom?: { name: string; type: string; value: string }[];
}

const props = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.docProps() as unknown as Props);

/** The file as the page would save it now. */
const saved = (page: Page) => inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));

async function openDetails(page: Page) {
  await chooseMenu(page, "File", "Document details…");
  const details = dialog(page, "Document details");
  await expect(details).toBeVisible();
  return details;
}

test.describe("document properties", () => {
  test("Document details has a field for each property Word keeps, and each is applied as it is left", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const details = await openDetails(page);
    const fill = async (label: string, value: string) => {
      const field = details.getByLabel(label, { exact: true });
      await field.fill(value);
      await field.blur();
    };
    await fill("Title", "Quarterly plan");
    await fill("Subject", "Budget");
    await fill("Author", "Ana Ortiz");
    await fill("Manager", "Ada Lovelace");
    await fill("Company", "Analytical Engines");
    await fill("Category", "Finance");
    await fill("Keywords", "budget, plan");
    await fill("Status", "Draft");
    await fill("Comments", "First pass");
    await expect.poll(() => props(page)).toMatchObject({
      title: "Quarterly plan",
      subject: "Budget",
      author: "Ana Ortiz",
      manager: "Ada Lovelace",
      company: "Analytical Engines",
      category: "Finance",
      keywords: "budget, plan",
      status: "Draft",
      description: "First pass",
    });
    // They are the file's own properties, where Word and other programs read them.
    const bytes = await saved(page);
    const core = partOf(bytes, "docProps/core.xml")!;
    for (const want of ["<dc:subject>Budget</dc:subject>", "<dc:creator>Ana Ortiz</dc:creator>", "<cp:keywords>budget, plan</cp:keywords>", "<cp:category>Finance</cp:category>", "<cp:contentStatus>Draft</cp:contentStatus>", "<dc:description>First pass</dc:description>"]) expect(core).toContain(want);
    const app = partOf(bytes, "docProps/app.xml")!;
    expect(app).toContain("<Company>Analytical Engines</Company>");
    expect(app).toContain("<Manager>Ada Lovelace</Manager>");
  });

  test("each change is one undo step, and the fields show what the file has", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Hello"] }], props: { title: "Old", subject: "Was", creator: "Ben" }, app: "<Company>Acme</Company>" });
    const details = await openDetails(page);
    await expect(details.getByLabel("Title", { exact: true })).toHaveValue("Old");
    await expect(details.getByLabel("Subject", { exact: true })).toHaveValue("Was");
    await expect(details.getByLabel("Author", { exact: true })).toHaveValue("Ben");
    await expect(details.getByLabel("Company", { exact: true })).toHaveValue("Acme");
    await details.getByLabel("Company", { exact: true }).fill("Globex");
    await details.getByLabel("Company", { exact: true }).blur();
    await details.getByLabel("Subject", { exact: true }).fill("");
    await details.getByLabel("Subject", { exact: true }).blur();
    await expect.poll(async () => (await props(page)).company).toBe("Globex");
    expect((await props(page)).subject).toBeUndefined();
    await inPage(page, (w) => w.fe.editor.engine!.undo());
    await expect.poll(() => props(page)).toMatchObject({ company: "Globex", subject: "Was" });
    await inPage(page, (w) => w.fe.editor.engine!.undo());
    expect((await props(page)).company).toBe("Acme");
  });

  test("custom properties are added with a type, changed, removed, and written as Word writes them", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const details = await openDetails(page);
    const add = async (name: string, type: string, value: string) => {
      await details.getByLabel("Name", { exact: true }).fill(name);
      await details.locator(".dd-add select").first().selectOption(type);
      const input = details.locator(".dd-add [aria-label='Value']");
      if (type === "bool") await input.selectOption(value);
      else await input.fill(value);
      await details.getByRole("button", { name: "Add", exact: true }).click();
    };
    await add("Project", "text", "Babbage");
    await add("Revision", "number", "7");
    await add("Due", "date", "2026-10-14");
    await add("Reviewed", "bool", "true");
    const rows = details.locator(".dd-custom .dd-prop");
    await expect(rows).toHaveCount(4);
    await expect(rows.nth(0)).toContainText("Project");
    await expect(rows.nth(1)).toContainText("Number");
    await expect(rows.nth(2)).toContainText("Date");
    expect((await props(page)).custom?.map((c) => [c.name, c.type, c.value])).toEqual([
      ["Project", "text", "Babbage"],
      ["Revision", "number", "7"],
      ["Due", "date", "2026-10-14"],
      ["Reviewed", "bool", "true"],
    ]);
    // Change one in place, remove another.
    await rows.nth(0).locator("input").fill("Engine");
    await rows.nth(0).locator("input").blur();
    await rows.nth(1).getByRole("button", { name: "Remove Revision" }).click();
    await expect(rows).toHaveCount(3);
    expect((await props(page)).custom?.map((c) => c.name)).toEqual(["Project", "Due", "Reviewed"]);
    expect((await props(page)).custom?.[0]?.value).toBe("Engine");
    const custom = partOf(await saved(page), "docProps/custom.xml")!;
    expect(custom).toContain("<vt:lpwstr>Engine</vt:lpwstr>");
    expect(custom).toContain("<vt:filetime>2026-10-14T00:00:00Z</vt:filetime>");
    expect(custom).toContain("<vt:bool>true</vt:bool>");
    expect(custom).not.toContain("Revision");
    // A property with no name is refused with a message, and nothing is added.
    await details.getByRole("button", { name: "Add", exact: true }).click();
    await expect(details.getByRole("alert")).toHaveText("Give the property a name.");
  });

  test("a DOCPROPERTY field shows a custom property, and the properties Word wrote are listed", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: ["Hello"] }],
      app: "<Company>Acme</Company><Manager>Wile</Manager>",
      custom:
        '<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Project"><vt:lpwstr>Babbage</vt:lpwstr></property>' +
        '<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="3" name="_MarkAsFinal"><vt:bool>true</vt:bool></property>',
    });
    const details = await openDetails(page);
    // The private property Word keeps for Mark as final is not in the list.
    await expect(details.locator(".dd-custom .dd-prop")).toHaveCount(1);
    await expect(details.locator(".dd-custom .dd-prop").first()).toContainText("Project");
    await expect(details.getByLabel("Manager", { exact: true })).toHaveValue("Wile");
    const value = await inPage(page, (w) => (w.fe.editor.engine as unknown as { fieldPreview(instr: string): string }).fieldPreview("DOCPROPERTY Project"));
    expect(value).toBe("Babbage");
  });

  test("saving records the statistics Word keeps and who saved", async ({ page }) => {
    await openFixture(page, "rich.docx");
    // The name from Tools, User information is the name saved with changes.
    await chooseMenu(page, "Tools", "User information…");
    const user = dialog(page, "User information");
    await user.getByLabel("Name", { exact: true }).fill("Grace Hopper");
    await user.getByRole("button", { name: "Apply" }).click();
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "More " });
    });
    const pages = await inPage(page, (w) => w.fe.editor.pages);
    const counts = await inPage(page, (w) => w.fe.editor.engine!.counts());
    const bytes = await saved(page);
    const app = partOf(bytes, "docProps/app.xml")!;
    expect(app).toContain(`<Pages>${pages}</Pages>`);
    expect(app).toContain(`<Words>${counts.words}</Words>`);
    expect(app).toContain(`<CharactersWithSpaces>${counts.chars}</CharactersWithSpaces>`);
    expect(app).toContain(`<Characters>${counts.charsNoSpaces}</Characters>`);
    expect(partOf(bytes, "docProps/core.xml")).toContain("<cp:lastModifiedBy>Grace Hopper</cp:lastModifiedBy>");
  });

  test("a new document is by the person named in User information", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await chooseMenu(page, "Tools", "User information…");
    const user = dialog(page, "User information");
    await user.getByLabel("Name", { exact: true }).fill("Grace Hopper");
    await user.getByRole("button", { name: "Apply" }).click();
    await chooseMenu(page, "File", "New document");
    await expect.poll(() => inPage(page, (w) => w.engine!.fileName)).toMatch(/^Untitled/);
    await expect.poll(() => props(page)).toMatchObject({ author: "Grace Hopper" });
    // The Author field says where the name comes from.
    const details = await openDetails(page);
    await expect(details.getByLabel("Author", { exact: true })).toHaveValue("Grace Hopper");
    await expect(details.getByText("Grace Hopper is the name saved with changes you make.")).toBeVisible();
  });

  test("Document details still lists the document's size, dates and counts", async ({ page }) => {
    await openFixture(page, "rich.docx");
    const details = await openDetails(page);
    const row = (label: string) => details.locator("dt", { hasText: new RegExp(`^${label}$`) }).locator("+ dd");
    await expect(row("Words")).toHaveText("98");
    await expect(row("Characters with spaces")).toHaveText(/\d/);
    await expect(row("Pages")).toHaveText("2");
    await details.getByRole("button", { name: "Done" }).click();
    await expect(details).toBeHidden();
  });
});
