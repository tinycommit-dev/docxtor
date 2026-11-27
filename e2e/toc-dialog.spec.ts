// The table of contents dialog: levels, page numbers, alignment, leaders, links, styles, looks; changing a table that
// is there; update page numbers only; remove; and lines added from the text (Add to table of contents).
import { openBuilt } from "./support/docx-builder";
import { answer, chooseMenu, dialog, expect, inPage, type Page, test } from "./helpers";

type Eng = {
  text(): string;
  tocAt(at?: number): number | null;
  tocOptions(at?: number): { levels: number; pageNumbers: boolean; rightAlign: boolean; leader: string; links: boolean; entries: boolean; styles: { style: string; level: number }[]; look?: string; outline: boolean } | null;
  renderAll(): { order: number[] };
};
type Ed = { select(s: unknown): void; focus(): void; undo(): void };

const H1 = '<w:pStyle w:val="Heading1"/>';
const H2 = '<w:pStyle w:val="Heading2"/>';

const options = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).tocOptions());
const tocLines = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().split("\n"));
const flowText = (page: Page) => page.locator(".flow").innerText();

async function start(page: Page) {
  await openBuilt(page, {
    sections: [
      {
        paragraphs: [
          { text: "Alpha", ppr: H1 },
          "Body one.",
          { text: "Beta", ppr: H2 },
          "Body two.",
          { text: "Gamma", ppr: H1 },
          "Body three.",
        ],
      },
    ],
  });
  const order = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
  await inPage(page, (w, p) => {
    const e = w.fe.editor as unknown as Ed;
    e.select({ p, o: 0 });
    e.focus();
  }, order[0]);
  return order;
}

const open = (page: Page) => chooseMenu(page, "Insert", "Page elements", "Table of contents options…");
const toc = (page: Page) => dialog(page, "Table of contents");

/** Put the caret in the table of contents. */
async function caretInToc(page: Page) {
  await inPage(page, (w) => {
    const e = w.fe.editor as unknown as Ed;
    const eng = w.fe.editor.engine as unknown as Eng;
    e.select({ p: eng.tocAt()!, o: 0 });
    e.focus();
  });
}

test.describe("table of contents dialog", () => {
  test("Insert with levels, no page numbers and a look puts a table in with the TOC styles formatted", async ({ page }) => {
    await start(page);
    await open(page);
    const d = toc(page);
    await expect(d.getByRole("button", { name: "Insert" })).toBeVisible();
    await expect(d.getByRole("button", { name: "Remove table of contents" })).toHaveCount(0);
    await d.getByLabel("Heading levels").selectOption("1");
    await d.getByLabel("Show page numbers").uncheck();
    await expect(d.getByLabel("Right align page numbers")).toBeDisabled();
    await expect(d.getByLabel("Line before the number")).toBeDisabled();
    await d.getByLabel("Look").selectOption("formal");
    await d.getByRole("button", { name: "Insert" }).click();
    await expect(d).toHaveCount(0);

    const o = await options(page);
    expect(o).toMatchObject({ levels: 1, pageNumbers: false, links: true });
    // Only the level 1 headings are listed, with no tab or number after them, and level 1 is in capitals (the formal look).
    const lines = await tocLines(page);
    expect(lines.slice(0, 4)).toEqual(["Contents", "Alpha", "Gamma", "Alpha"]);
    const first = page.locator('.flow [data-fld="TOC"]').first();
    await expect(first).toBeVisible();
    const caps = await page.locator(".flow [data-p]").filter({ hasText: /^Alpha$/ }).first().evaluate((el) => Array.from(el.querySelectorAll("*")).concat(el).map((x) => getComputedStyle(x).textTransform));
    expect(caps).toContain("uppercase");
    await page.screenshot({ path: "test-results/toc-formal.png", clip: { x: 0, y: 0, width: 1280, height: 520 } });
  });

  test("Open on a table, the dialog shows its options; changing the leader and aligning them rebuilds it in place", async ({ page }) => {
    await start(page);
    await chooseMenu(page, "Insert", "Page elements", "Table of contents");
    await caretInToc(page);
    await open(page);
    const d = toc(page);
    await expect(d.getByRole("button", { name: "Apply" })).toBeVisible();
    await expect(d.getByLabel("Heading levels")).toHaveValue("3");
    await expect(d.getByLabel("Line before the number")).toHaveValue("dot");
    await d.getByLabel("Line before the number").selectOption("hyphen");
    await d.getByRole("button", { name: "Apply" }).click();
    expect(await options(page)).toMatchObject({ leader: "hyphen", pageNumbers: true, rightAlign: true });
    // Page numbers are in the table, still right aligned: the entries end with a tab and the number.
    const lines = await tocLines(page);
    expect(lines.filter((l) => /^(Alpha|Beta|Gamma)\t\d+$/.test(l)).length).toBe(3);
    // Not right aligned: the number follows the entry after a space.
    await caretInToc(page);
    await open(page);
    await toc(page).getByLabel("Right align page numbers").uncheck();
    await toc(page).getByRole("button", { name: "Apply" }).click();
    expect((await tocLines(page)).filter((l) => /^(Alpha|Beta|Gamma) \d+$/.test(l)).length).toBe(3);
    // One undo goes back a step: the numbers are aligned again.
    await inPage(page, (w) => (w.fe.editor as unknown as Ed).undo());
    expect((await tocLines(page)).filter((l) => /^(Alpha|Beta|Gamma)\t\d+$/.test(l)).length).toBe(3);
  });

  test("Other styles make entries at the level given, and outline levels and table entry fields can be switched", async ({ page }) => {
    await openBuilt(page, {
      styles: '<w:style w:type="paragraph" w:customStyle="1" w:styleId="Notice"><w:name w:val="Notice"/><w:basedOn w:val="Normal"/><w:qFormat/></w:style>',
      sections: [{ paragraphs: [{ text: "Alpha", ppr: H1 }, { text: "Important notice", ppr: '<w:pStyle w:val="Notice"/>' }, "Plain."] }],
    });
    const order = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as Ed;
      e.select({ p, o: 0 });
      e.focus();
    }, order[0]);
    await open(page);
    const d = toc(page);
    await d.getByText("Other styles").click();
    await d.getByLabel("Level for Notice").fill("2");
    await d.getByRole("button", { name: "Insert" }).click();
    expect(await options(page)).toMatchObject({ styles: [{ style: "Notice", level: 2 }] });
    const lines = await tocLines(page);
    expect(lines).toContain("Important notice\t1");
    expect(await options(page)).toMatchObject({ levels: 3, outline: true });
  });

  test("Update page numbers only keeps the entries; Remove table of contents takes the table and its title away", async ({ page }) => {
    const order = await start(page);
    await chooseMenu(page, "Insert", "Page elements", "Table of contents");
    // Rename a heading: the table still says Gamma until it is updated in full.
    await inPage(page, (w, p) => w.fe.editor.exec({ op: "set_paragraph_text", id: p, markdown: "Gamma renamed" }), order[4]);
    await chooseMenu(page, "Insert", "Page elements", "Update page numbers only");
    let lines = await tocLines(page);
    expect(lines.some((l) => /^Gamma\t\d+$/.test(l))).toBe(true);
    expect(lines.some((l) => l.startsWith("Gamma renamed\t"))).toBe(false);
    await chooseMenu(page, "Insert", "Page elements", "Update table of contents");
    lines = await tocLines(page);
    expect(lines.some((l) => /^Gamma renamed\t\d+$/.test(l))).toBe(true);
    // Remove.
    await caretInToc(page);
    await chooseMenu(page, "Insert", "Page elements", "Remove table of contents");
    await answer(page, "Remove table of contents");
    lines = await tocLines(page);
    expect(lines.some((l) => l === "Contents" || /\t\d+$/.test(l))).toBe(false);
    expect(lines).toContain("Gamma renamed");
    expect(await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).tocAt())).toBeNull();
    // The command is not offered without a table.
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Insert", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Page elements/ }).click();
    await expect(page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Remove table of contents$/ })).toBeDisabled();
    await page.keyboard.press("Escape");
  });

  test("Add to table of contents hides a line in the text, and a table that lists table entry fields shows it", async ({ page }) => {
    const order = await start(page);
    // Select "Body two." and add it as a level 2 line.
    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as Ed;
      e.select({ anchor: { p, o: 0 }, focus: { p, o: 9 } });
      e.focus();
    }, order[3]);
    await chooseMenu(page, "Insert", "References", "Add to table of contents…");
    const d = dialog(page, "Add to table of contents");
    await expect(d.getByLabel("Line in the table")).toHaveValue("Body two.");
    await d.getByLabel("Level").selectOption("2");
    await d.getByRole("button", { name: "Add" }).click();
    // Nothing shows in the text, but the field is there.
    await expect(page.locator(".flow").getByText("Body two.", { exact: true })).toHaveCount(1);
    expect(await inPage(page, (w) => w.fe.editor.engine!.listFields().filter((f) => f.code === "TC").length)).toBe(1);

    await inPage(page, (w, p) => {
      const e = w.fe.editor as unknown as Ed;
      e.select({ p, o: 0 });
      e.focus();
    }, order[0]);
    await open(page);
    await toc(page).getByLabel("Table entry fields (Insert, Add to contents)").check();
    await toc(page).getByRole("button", { name: "Insert" }).click();
    const lines = await tocLines(page);
    expect(lines.some((l) => /^Body two\.\t\d+$/.test(l))).toBe(true);
    expect((await flowText(page)).includes("Body two.")).toBe(true);
  });
});

test.describe("a table of contents the assistant makes", () => {
  // The engine's tools have no page layout, so a table they insert had page 1 on every line until "Update page numbers only".
  // The editor runs engine tools (`Editor.runTool`, used by the assistant and `ctx.ai.tool`) and numbers the pages from its layout,
  // in the same undo step.
  test("has the real page numbers at once, and one undo takes it away", async ({ page }) => {
    // Text fills the pages, so the headings land on later pages by layout alone (no page breaks the engine could count).
    const body = (n: string) => Array.from({ length: 45 }, (_, i) => `${n} paragraph ${i + 1}, long enough to take a line of its own on the page.`);
    await openBuilt(page, {
      sections: [{ paragraphs: [{ text: "Alpha", ppr: H1 }, ...body("First"), { text: "Beta", ppr: H1 }, ...body("Second"), { text: "Gamma", ppr: H1 }, "The end."] }],
    });
    // Where the headings are in the layout as it is now (with the table in it, after the tool ran).
    const where = () => inPage(page, (w) => {
      const pages = w.fe.editor.pageMap() as Record<string, number>;
      const eng = w.fe.editor.engine as unknown as Eng;
      const order = eng.renderAll().order;
      const text = eng.text().split("\n");
      return ["Alpha", "Beta", "Gamma"].map((h) => `${h}\t${pages[String(order[text.indexOf(h)])]}`);
    });
    const out = await inPage(page, (w) => (w.fe.editor as unknown as { runTool(n: string, i: unknown): string }).runTool("insert_table_of_contents", { title: "Contents" }));
    expect(out).toContain("Table of contents inserted");
    const want = await where();
    expect(want[2]).not.toBe("Gamma\t1");
    const lines = await tocLines(page);
    expect(lines).toEqual(expect.arrayContaining(want));
    await inPage(page, (w) => (w.fe.editor as unknown as Ed).undo());
    expect((await tocLines(page)).some((l) => l === "Contents" || /\t\d+$/.test(l))).toBe(false);
  });
});
