// Citation placeholders (Add new placeholder) and the shared list of sources (Word's master list).
import { openBuilt } from "./support/docx-builder";
import { answer, chooseMenu, dialog, documentReady, expect, inPage, type Page, test } from "./helpers";

type Src = { tag: string; title: string; authors: unknown[] };
type Eng = { text(): string; sources(): Src[]; citations(): { tags: string[] }[]; renderAll(): { order: number[] } };
type Ed = { select(s: unknown): void; focus(): void };

const sources = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).sources());
const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().split("\n"));
const panel = (page: Page) => page.locator(".cite-panel");

async function start(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Citing is a habit.", "Another paragraph follows."] }] });
  const ids = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
  await inPage(page, (w, p) => {
    const e = w.fe.editor as unknown as Ed;
    e.select({ p, o: 18 });
    e.focus();
  }, ids[0]);
}

async function openPanel(page: Page) {
  await chooseMenu(page, "Tools", "Citations");
  await expect(panel(page)).toBeVisible();
}

async function addBook(page: Page, title: string) {
  await panel(page).getByRole("button", { name: "Fill in a form…" }).click();
  const f = dialog(page, "Add a source");
  await f.getByLabel("Title", { exact: true }).fill(title);
  await f.getByLabel("Family name").first().fill("Roe");
  await f.getByLabel("Given names").first().fill("Jane");
  await f.getByLabel("Year").fill("2019");
  await f.getByLabel("Publisher").fill("Penguin");
  await f.getByRole("button", { name: "Add source" }).click();
  await expect(f).toHaveCount(0);
}

test.describe("citation placeholders", () => {
  test("Add new placeholder cites a tag now, keeps it out of the bibliography, and filling it in changes the citation", async ({ page }) => {
    await start(page);
    await chooseMenu(page, "Insert", "Add new placeholder…");
    const d = dialog(page, "Placeholder name");
    await expect(d.getByLabel("Name")).toHaveValue("Placeholder1");
    await d.getByLabel("Name").fill("Roe2019");
    await d.getByRole("button", { name: "Add" }).click();
    await expect(page.locator(".toast")).toContainText("Cited Roe2019");
    expect(await text(page)).toContain("Citing is a habit.(Roe2019)");
    expect((await sources(page)).map((s) => s.tag)).toEqual(["Roe2019"]);

    // The bibliography leaves it out until it has details.
    await chooseMenu(page, "Insert", "Bibliography");
    expect((await text(page)).filter((l) => l.includes("Roe2019"))).toHaveLength(1);

    // Fill it in from the panel: Edit, add the author and the title.
    await openPanel(page);
    await panel(page).locator(".src-item", { hasText: "Roe2019" }).getByRole("button", { name: /^Edit/ }).click();
    const f = dialog(page, /./).last();
    await f.getByLabel("Family name").first().fill("Roe");
    await f.getByLabel("Given names").first().fill("Jane");
    await f.getByLabel("Year").fill("2019");
    await f.getByRole("button", { name: /Save|Update/ }).click();
    await expect.poll(async () => (await text(page)).join("\n")).toContain("(Roe, 2019)");
    expect((await text(page)).join("\n")).not.toContain("(Roe2019)");
  });

  test("The panel offers Add a placeholder, and a second one gets the next name", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "Add a placeholder…" }).click();
    await dialog(page, "Placeholder name").getByRole("button", { name: "Add" }).click();
    await panel(page).getByRole("button", { name: "Add a placeholder…" }).click();
    await expect(dialog(page, "Placeholder name").getByLabel("Name")).toHaveValue("Placeholder2");
    await dialog(page, "Placeholder name").getByRole("button", { name: "Cancel" }).click();
    expect((await sources(page)).map((s) => s.tag)).toEqual(["Placeholder1"]);
  });
});

test.describe("the shared list of sources", () => {
  test("Sources copy into the shared list and out of it into another document", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page, "The Art of Citing");
    await panel(page).getByRole("button", { name: "Shared list…" }).click();
    const d = dialog(page, "Shared list of sources");
    await expect(d.getByText("The shared list is empty")).toBeVisible();
    await expect(d.getByRole("button", { name: "Copy to the shared list" })).toBeDisabled();
    await d.getByRole("list", { name: "Sources in this document" }).getByRole("checkbox").first().check();
    await d.getByRole("button", { name: "Copy to the shared list" }).click();
    await expect(d.getByRole("list", { name: "Sources in the shared list" })).toContainText("The Art of Citing");
    // Copying it again says it is there already.
    await d.getByRole("list", { name: "Sources in this document" }).getByRole("checkbox").first().check();
    await d.getByRole("button", { name: "Copy to the shared list" }).click();
    await expect(d.getByRole("alert")).toContainText("already has those sources");
    await d.getByRole("button", { name: "Close" }).click();

    // Another document sees the list (it is kept in the browser) and can take the source.
    await page.goto("/?open=simple.docx");
    await documentReady(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "Shared list…" }).click();
    const d2 = dialog(page, "Shared list of sources");
    await expect(d2.getByRole("list", { name: "Sources in this document" })).toContainText("This document has no sources");
    await d2.getByRole("list", { name: "Sources in the shared list" }).getByRole("checkbox").first().check();
    await d2.getByRole("button", { name: "Copy to this document" }).click();
    await expect(page.locator(".toast")).toContainText("Copied 1 source to this document");
    await expect(d2.getByRole("list", { name: "Sources in this document" })).toContainText("The Art of Citing");
    expect((await sources(page)).map((s) => s.title)).toEqual(["The Art of Citing"]);
    // Taking it twice is refused.
    await d2.getByRole("list", { name: "Sources in the shared list" }).getByRole("checkbox").first().check();
    await d2.getByRole("button", { name: "Copy to this document" }).click();
    await expect(d2.getByRole("alert")).toContainText("already has those sources");
    // Removing it from the shared list leaves the document's copy.
    await d2.getByRole("button", { name: "Remove from the shared list" }).click();
    await answer(page, "Remove from the shared list");
    await expect(d2.getByText("The shared list is empty")).toBeVisible();
    expect((await sources(page)).length).toBe(1);
  });

  test("Without a place to keep the list, the button says so", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await inPage(page, (w) => (w as unknown as { fe: { extensions: { disable(id: string): Promise<void> } } }).fe.extensions.disable("docxtor.library.local"));
    await panel(page).getByRole("button", { name: "Shared list…" }).click();
    await expect(page.locator(".toast")).toContainText("no shared list of sources");
  });
});
