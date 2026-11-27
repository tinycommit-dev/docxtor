// The Index dialog (type, columns, language, leader, separators, look; changing an index that is there) and AutoMark.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

type Eng = {
  text(): string;
  renderAll(): { order: number[] };
  indexOptions(at?: number): { columns: number; runIn: boolean; headings: boolean; rightAlign: boolean; leader: string; language?: number; pageSep?: string } | null;
  listFields(): { code: string; instr: string }[];
};
type Ed = { select(s: unknown): void; focus(): void; exec(ops: unknown): unknown };

const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().split("\n"));
const opts = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).indexOptions());

async function start(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Apple pie is sweet.", "A pear tart.", "An Ñandú nest.", "The end."] }] });
  // Mark three entries.
  await inPage(page, (w) => {
    const e = w.fe.editor as unknown as Ed;
    const ids = (w.fe.editor.engine as unknown as Eng).renderAll().order;
    e.exec([
      { op: "mark_index_entry", at: { p: ids[0], o: 5 }, entry: "Apple" },
      { op: "mark_index_entry", at: { p: ids[1], o: 6 }, entry: "Pear" },
      { op: "mark_index_entry", at: { p: ids[2], o: 7 }, entry: "Ñandú" },
    ]);
    e.select({ p: ids[3], o: 0 });
    e.focus();
  });
}

const caretInIndex = (page: Page) =>
  inPage(page, (w) => {
    const e = w.fe.editor as unknown as Ed;
    const lines = w.fe.editor.engine!.text().split("\n");
    const ids = (w.fe.editor.engine as unknown as Eng).renderAll().order;
    e.select({ p: ids[lines.findIndex((l) => /^Apple\t/.test(l) || l === "Apple")], o: 0 });
    e.focus();
  });

test.describe("index dialog", () => {
  test("Insert with Spanish, letter headings, two columns and dashes lists the entries in that order", async ({ page }) => {
    await start(page);
    await chooseMenu(page, "Insert", "References", "Index options…");
    const d = dialog(page, "Index");
    await expect(d.getByRole("button", { name: "Insert" })).toBeVisible();
    await expect(d.getByLabel("Columns")).toHaveValue("2");
    await d.getByLabel("Language").selectOption("3082");
    await d.getByLabel("A heading before each letter").check();
    await d.getByLabel("Line before the number").selectOption("hyphen");
    await d.getByLabel("Between page numbers").fill("; ");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(d).toHaveCount(0);

    expect(await opts(page)).toMatchObject({ columns: 2, runIn: false, headings: true, rightAlign: true, leader: "hyphen", language: 3082, pageSep: "; " });
    const lines = await text(page);
    const at = (s: string) => lines.findIndex((l) => l === s || l.startsWith(`${s}\t`));
    // Spanish puts the n with a tilde after n, and gives it a heading of its own.
    expect(at("A")).toBeGreaterThan(-1);
    expect(at("Ñ")).toBeGreaterThan(at("A"));
    expect(at("Ñandú")).toBeGreaterThan(at("Ñ"));
    expect(at("Pear")).toBeGreaterThan(at("Ñandú"));
    await expect(page.locator(".flow").getByText("Ñ", { exact: true })).toBeVisible();
    await page.screenshot({ path: "test-results/index-spanish.png", clip: { x: 0, y: 150, width: 1280, height: 700 } });
  });

  test("Open on an index, the dialog changes that index: run-in entries, one column, no aligned numbers", async ({ page }) => {
    await start(page);
    await chooseMenu(page, "Insert", "References", "Index");
    expect(await opts(page)).toMatchObject({ columns: 2, runIn: false });
    await caretInIndex(page);
    await chooseMenu(page, "Insert", "References", "Index options…");
    const d = dialog(page, "Index");
    await expect(d.getByRole("button", { name: "Apply" })).toBeVisible();
    await d.getByRole("radio", { name: "Run-in" }).click();
    await expect(d.getByLabel("Right align page numbers")).toBeDisabled();
    await d.getByLabel("Columns").selectOption("1");
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await opts(page)).toMatchObject({ columns: 1, runIn: true });
    // One index, not two.
    expect((await text(page)).filter((l) => l.startsWith("Apple, ")).length).toBe(1);
    expect((await text(page)).some((l) => /^Apple, \d+$/.test(l))).toBe(true);
  });
});

test.describe("AutoMark", () => {
  test("A file with a table of two columns marks every listed text; the example file is one", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["An apple a day.", "A pear is ripe.", "Nothing here."] }] });
    await inPage(page, (w) => {
      const e = w.fe.editor as unknown as Ed;
      e.select({ p: (w.fe.editor.engine as unknown as Eng).renderAll().order[0], o: 0 });
      e.focus();
    });
    await chooseMenu(page, "Insert", "References", "AutoMark…");
    const d = dialog(page, "AutoMark");
    await expect(d.getByRole("button", { name: "Mark", exact: true })).toBeDisabled();
    const download = page.waitForEvent("download");
    await d.getByRole("button", { name: "Download an example file" }).click();
    const file = await download;
    expect(file.suggestedFilename()).toBe("automark.docx");
    const path = await file.path();
    await d.getByLabel("AutoMark file").setInputFiles(path!);
    await expect(d.locator(".am-row")).toHaveCount(2);
    await expect(d.locator(".am-row").first()).toContainText("apple");
    await expect(d.locator(".am-row").first()).toContainText("Fruit:Apple");
    await d.getByRole("button", { name: "Mark", exact: true }).click();
    const marks = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).listFields().filter((f) => f.code === "XE").map((f) => f.instr.trim()));
    expect(marks).toEqual(expect.arrayContaining(['XE "Fruit:Apple"', 'XE "Fruit:Pear"']));
    expect(marks.length).toBe(2);
    // The marks feed an index with the sub-entries under Fruit.
    await chooseMenu(page, "Insert", "References", "Index");
    const lines = await text(page);
    expect(lines.some((l) => l === "Fruit")).toBe(true);
    expect(lines.some((l) => /^Apple\t\d+$/.test(l))).toBe(true);
  });

  test("A document without a table is refused with a plain sentence", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Text."] }] });
    await chooseMenu(page, "Insert", "References", "AutoMark…");
    const d = dialog(page, "AutoMark");
    // A real .docx that is not an AutoMark file: this very document, saved.
    const bytes = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));
    await d.getByLabel("AutoMark file").setInputFiles({ name: "plain.docx", mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", buffer: Buffer.from(bytes) });
    await expect(d.getByRole("status")).toContainText("no table");
    await expect(d.getByRole("button", { name: "Mark", exact: true })).toBeDisabled();
  });
});
