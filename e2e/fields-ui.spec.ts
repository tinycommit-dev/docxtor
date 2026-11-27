// The field tools: Insert, Date and time; Insert, Field with its form, code and preview; document properties; field
// types an extension adds; field codes (View, Field codes, Alt+F9, Shift+F9, editing a code); and updating fields
// before printing and downloading, with its setting.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, openMenu, menuItem, test, type Locator, type Page } from "./helpers";

const run = (t: string) => `<w:r><w:t xml:space="preserve">${t}</w:t></w:r>`;
const fld = (instr: string, result: string) =>
  `<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> ${instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>${result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`;
const doc = (raws: string[], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs: [...raws.map((raw) => ({ raw })), "The end"] }], ...extra });
const YEAR = String(new Date().getFullYear());

interface Field {
  p: number;
  o: number;
  instr: string;
  result: string;
  code: string;
}
const fields = (page: Page): Promise<Field[]> => inPage(page, (w) => (w.fe.editor.engine as unknown as { listFields(): Field[] }).listFields());
const line = (page: Page, n: number): Locator => page.locator(".flow .fp").nth(n);
const exportAs = (page: Page, id: string) => inPage(page, async (w, format) => void (await (w.fe as unknown as { export(id: string): Promise<unknown> }).export(format)), id);
const canUndo = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as { raw: { canUndo(): boolean } }).raw.canUndo());
/** The app's extension host, as the page exposes it. */
interface Host {
  add(ext: unknown): Promise<void>;
  execute(id: string): unknown;
}
const bodyText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());

/** Put the caret at the end of a paragraph of the document. */
async function caretAtEndOf(page: Page, n: number) {
  await line(page, n).click();
  await page.keyboard.press("End");
}

test.describe("Insert, Date and time", () => {
  test("lists the formats with today's date, and inserts the chosen one as plain text", async ({ page }) => {
    await openBuilt(page, doc([run("Dated ")], {}));
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Date and time…");
    const box = dialog(page, "Date and time");
    await expect(box).toBeVisible();
    const options = box.getByRole("option");
    expect(await options.count()).toBeGreaterThan(10);
    // One of the formats is the year as four digits somewhere; the first is the short date.
    const first = await options.first().innerText();
    expect(first).toMatch(/^\d{1,2}\/\d{1,2}\/\d{4}$/);
    await options.nth(2).click();
    const picked = await options.nth(2).innerText();
    await box.getByRole("button", { name: "Insert" }).click();
    await expect(box).toBeHidden();
    expect(await fields(page)).toEqual([]);
    expect(await bodyText(page)).toContain(`Dated ${picked}`);
    // One step, and undo takes it out.
    await page.keyboard.press("ControlOrMeta+z");
    expect(await bodyText(page)).not.toContain(picked);
  });

  test("with Update automatically it inserts a DATE field with the format as its picture; a time format makes a TIME field", async ({ page }) => {
    await openBuilt(page, doc([run("")]));
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Date and time…");
    let box = dialog(page, "Date and time");
    await box.getByRole("option").nth(1).click();
    await box.getByLabel("Update automatically").check();
    await box.getByRole("button", { name: "Insert" }).click();
    let f = await fields(page);
    expect(f).toHaveLength(1);
    expect(f[0].code).toBe("DATE");
    expect(f[0].instr).toMatch(/\\@ "dddd, MMMM d, yyyy"/);
    expect(f[0].result).toContain(YEAR);

    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Date and time…");
    box = dialog(page, "Date and time");
    const times = box.getByRole("option").filter({ hasText: /^\d{1,2}:\d\d(:\d\d)?\s?(am|pm)?$/i });
    await times.first().click();
    await box.getByLabel("Update automatically").check();
    await box.getByRole("button", { name: "Insert" }).click();
    f = await fields(page);
    expect(f.map((x) => x.code)).toEqual(["DATE", "TIME"]);
  });

  test("the formats follow the document's language", async ({ page }) => {
    await openBuilt(page, doc([run("x")], { lang: "de-DE" }));
    await line(page, 0).click();
    await chooseMenu(page, "Insert", "Date and time…");
    const first = await dialog(page, "Date and time").getByRole("option").first().innerText();
    // German short date: day.month.year.
    expect(first).toMatch(/^\d{2}\.\d{2}\.\d{4}$/);
  });
});

test.describe("Insert, Field", () => {
  test("lists the field names by category, shows the form, the code and a preview, and inserts it", async ({ page }) => {
    await openBuilt(page, doc([run("By ")], { props: { creator: "Ada Lovelace" } }));
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Field…");
    const box = dialog(page, "Field");
    await expect(box).toBeVisible();
    // The categories are Word's.
    const cats = await box.getByLabel("Category").locator("option").allInnerTexts();
    expect(cats).toEqual(["All", "Date and time", "Document information", "Numbering", "Links and references", "Equations and formulas", "User information", "Document automation"]);
    await box.getByLabel("Category").selectOption({ label: "Document information" });
    const names = await box.locator(".fd-names").getByRole("option").allInnerTexts();
    expect(names.some((n) => n.startsWith("Author"))).toBe(true);
    expect(names.some((n) => n.startsWith("Date"))).toBe(false);
    await box.locator(".fd-names").getByRole("option").filter({ hasText: "AUTHOR" }).click();
    await expect(box.getByLabel("Field code")).toHaveValue("AUTHOR \\* MERGEFORMAT");
    await expect(box.locator(".fd-preview")).toHaveText("Ada Lovelace");
    // A text format is a switch in the code, and the preview follows.
    await box.getByLabel("Text format").selectOption({ label: "Uppercase" });
    await expect(box.getByLabel("Field code")).toHaveValue("AUTHOR \\* Upper \\* MERGEFORMAT");
    await expect(box.locator(".fd-preview")).toHaveText("ADA LOVELACE");
    await box.getByRole("button", { name: "Insert" }).click();
    await expect(box).toBeHidden();
    const f = await fields(page);
    expect(f).toHaveLength(1);
    expect(f[0].instr.trim()).toBe("AUTHOR \\* Upper \\* MERGEFORMAT");
    expect(f[0].result).toBe("ADA LOVELACE");
    await expect(line(page, 0)).toContainText("By ADA LOVELACE");
  });

  test("a date field's form offers the document's formats, and the code can be edited by hand", async ({ page }) => {
    await openBuilt(page, doc([run("")]));
    await line(page, 0).click();
    await chooseMenu(page, "Insert", "Field…");
    const box = dialog(page, "Field");
    await box.locator(".fd-names").getByRole("option").filter({ hasText: /^Date\s*DATE$/ }).click();
    await expect(box.getByLabel("Field code")).toHaveValue(/^DATE \\@ "MMMM d, yyyy"/);
    const select = box.getByLabel("Date and time format");
    expect(await select.locator("option").count()).toBeGreaterThan(10);
    await select.selectOption("yyyy-MM-dd");
    await expect(box.getByLabel("Field code")).toHaveValue(/DATE \\@ "yyyy-MM-dd"/);
    await expect(box.locator(".fd-preview")).toHaveText(/^\d{4}-\d\d-\d\d$/);
    // Typing a code of your own is allowed.
    await box.getByLabel("Field code").fill('DATE \\@ "yyyy"');
    await expect(box.locator(".fd-preview")).toHaveText(YEAR);
    await box.getByRole("button", { name: "Insert" }).click();
    expect((await fields(page))[0].result).toBe(YEAR);
  });

  test("a formula, a cross-reference and a sequence number are worked out when inserted", async ({ page }) => {
    await openBuilt(page, doc([run("Sum: ")]));
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Field…");
    let box = dialog(page, "Field");
    await box.locator(".fd-names").getByRole("option").filter({ hasText: /^Formula\s*=$/ }).click();
    await box.getByLabel("Formula", { exact: true }).fill("6*7");
    await box.getByLabel("Number picture").fill("0.0");
    await expect(box.locator(".fd-preview")).toHaveText("42.0");
    await box.getByRole("button", { name: "Insert" }).click();
    await expect(line(page, 0)).toContainText("Sum: 42.0");

    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Field…");
    box = dialog(page, "Field");
    await box.locator(".fd-names").getByRole("option").filter({ hasText: /^Sequence\s*SEQ$/ }).click();
    await box.getByRole("button", { name: "Insert" }).click();
    const f = await fields(page);
    expect(f.map((x) => x.code)).toEqual(["=", "SEQ"]);
    expect(f[1].result).toBe("1");
  });

  test("an equation field is drawn when inserted, and the list has no table of contents field (it has its own dialog)", async ({ page }) => {
    await openBuilt(page, doc([run("x = ")]));
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Field…");
    const box = dialog(page, "Field");
    const names = await box.locator(".fd-names").getByRole("option").allInnerTexts();
    expect(names.some((n) => /\bTOC\b/.test(n))).toBe(false);
    await box.locator(".fd-names").getByRole("option").filter({ hasText: /^Equation field\s*EQ$/ }).click();
    await box.getByLabel("Equation", { exact: true }).fill("\\f(1,2)");
    await box.getByRole("button", { name: "Insert" }).click();
    await expect(box).toBeHidden();
    await expect(line(page, 0).locator(".ghost.eq math mfrac")).toHaveCount(1);
    expect((await fields(page))[0].code).toBe("EQ");
  });

  test("document properties: Insert, Document property inserts a DOCPROPERTY field with the value", async ({ page }) => {
    await openBuilt(page, doc([run("Company: ")], { app: "<Company>Analytical</Company>", props: { title: "Notes" } }));
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Document property", "Company");
    let f = await fields(page);
    expect(f[0].code).toBe("DOCPROPERTY");
    expect(f[0].instr).toContain("DOCPROPERTY Company");
    expect(f[0].result).toBe("Analytical");
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Document property", "Title");
    f = await fields(page);
    expect(f.map((x) => x.result)).toEqual(["Analytical", "Notes"]);
  });

  test("an extension can add field types: they are listed, and its evaluator works out the result on update", async ({ page }) => {
    await openBuilt(page, doc([run("Weather: ") + fld("WEATHER London \\* Upper", "?")]));
    await inPage(page, async (w) => {
      await (w.fe.extensions as unknown as Host).add({
        id: "test.weather",
        name: "Weather",
        activate(ctx: { fields: { register(f: unknown): void } }) {
          ctx.fields.register({
            name: "WEATHER",
            title: "Weather",
            category: "Document automation",
            description: "The weather in a city.",
            options: [{ id: "city", label: "City", kind: "text", write: "arg" }],
            evaluate: (call: { args: string[] }) => `Sunny in ${call.args[0]}`,
          });
        },
      } as never);
    });
    // It is in the dialog, under its category.
    await caretAtEndOf(page, 0);
    await chooseMenu(page, "Insert", "Field…");
    const box = dialog(page, "Field");
    await box.getByLabel("Category").selectOption({ label: "Document automation" });
    await expect(box.locator(".fd-names").getByRole("option").filter({ hasText: "WEATHER" })).toBeVisible();
    await box.getByRole("button", { name: "Cancel" }).click();
    // Update fields runs its evaluator.
    await page.keyboard.press("F9");
    await expect.poll(async () => (await fields(page))[0].result).toBe("Sunny in London");
    // The update is part of one undo step with the others.
    await page.keyboard.press("ControlOrMeta+z");
    await expect.poll(async () => (await fields(page))[0].result).toBe("?");
  });
});

test.describe("field codes", () => {
  const open = (page: Page) => openBuilt(page, doc([run("Year: ") + fld('DATE \\@ "yyyy"', "1999")]));

  test("View, Field codes shows every code and the result again when turned off", async ({ page }) => {
    await open(page);
    await openMenu(page, "View");
    const item = menuItem(page, "Field codes");
    await expect(item).toBeVisible();
    await expect(item).toHaveAttribute("aria-checked", "false");
    await item.click();
    await expect(page.locator(".flow .fc-first")).toHaveCount(1);
    expect(await page.locator(".flow .fc-first").evaluate((e) => (e as HTMLElement).dataset.code)).toBe('{ DATE \\@ "yyyy" }');
    await openMenu(page, "View");
    await expect(menuItem(page, "Field codes")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Field codes").click();
    await expect(page.locator(".flow .fc-first")).toHaveCount(0);
  });

  test("Shift+F9 shows the code of the field at the caret only", async ({ page }) => {
    await openBuilt(page, doc([run("A ") + fld("AUTHOR", "x") + run(" B ") + fld("TITLE", "y")], { props: { creator: "Ada", title: "Notes" } }));
    const a = await fields(page);
    await inPage(page, (w, o) => {
      const e = w.fe.editor as unknown as { select(s: unknown): void; focus(): void };
      e.select({ p: o.p, o: o.o + 1 });
      e.focus();
    }, a[0]);
    await page.keyboard.press("Shift+F9");
    await expect(page.locator(".flow .fc-first")).toHaveCount(1);
    expect(await page.locator(".flow .fc-first").evaluate((e) => (e as HTMLElement).dataset.code)).toBe("{ AUTHOR }");
    await page.keyboard.press("Shift+F9");
    await expect(page.locator(".flow .fc-first")).toHaveCount(0);
  });

  test("a shown code can be edited: the result is worked out again, and undo brings the old code back", async ({ page }) => {
    await openBuilt(page, doc([run("Year: ") + fld('DATE \\@ "yyyy"', "1999")]));
    await page.keyboard.press("Alt+F9");
    // The code is drawn by the span's ::before at font size 0 on the span itself, so Firefox and Safari give the span no box a mouse can be sent
    // to ("outside of the viewport"); the double-click is sent to the span, which is where a real one lands.
    await page.locator(".flow .fc-first").dispatchEvent("dblclick");
    const input = page.getByLabel("Field code");
    await expect(input).toHaveValue('DATE \\@ "yyyy"');
    await input.fill('DATE \\@ "MMMM yyyy"');
    await input.press("Enter");
    await expect.poll(async () => (await fields(page))[0].instr.trim()).toBe('DATE \\@ "MMMM yyyy"');
    expect((await fields(page))[0].result).toMatch(new RegExp(`^[A-Za-z]+ ${YEAR}$`));
    await page.keyboard.press("Alt+F9");
    await expect(line(page, 0)).toContainText(new RegExp(`Year: [A-Za-z]+ ${YEAR}`));
    await page.keyboard.press("ControlOrMeta+z");
    await expect.poll(async () => (await fields(page))[0].instr.trim()).toBe('DATE \\@ "yyyy"');
  });

  test("Escape leaves a code as it was", async ({ page }) => {
    await openBuilt(page, doc([run("Year: ") + fld('DATE \\@ "yyyy"', "1999")]));
    await page.keyboard.press("Alt+F9");
    // The code is drawn by the span's ::before at font size 0 on the span itself, so Firefox and Safari give the span no box a mouse can be sent
    // to ("outside of the viewport"); the double-click is sent to the span, which is where a real one lands.
    await page.locator(".flow .fc-first").dispatchEvent("dblclick");
    await page.getByLabel("Field code").fill("TITLE");
    await page.getByLabel("Field code").press("Escape");
    expect((await fields(page))[0].instr.trim()).toBe('DATE \\@ "yyyy"');
  });
});

test.describe("updating fields before printing and downloading", () => {
  const stale = (page: Page) => openBuilt(page, doc([run("By ") + fld("AUTHOR", "stale")], { props: { creator: "Ada" } }));

  test("downloading brings the fields up to date as one undoable step", async ({ page }) => {
    await stale(page);
    expect((await fields(page))[0].result).toBe("stale");
    expect(await canUndo(page)).toBe(false);
    await exportAs(page, "docx");
    expect((await fields(page))[0].result).toBe("Ada");
    expect(await canUndo(page)).toBe(true);
    await page.keyboard.press("ControlOrMeta+z");
    expect((await fields(page))[0].result).toBe("stale");
  });

  test("when nothing changes, nothing is recorded in undo", async ({ page }) => {
    await openBuilt(page, doc([run("By ") + fld("AUTHOR", "Ada")], { props: { creator: "Ada" } }));
    await exportAs(page, "docx");
    expect(await canUndo(page)).toBe(false);
  });

  test("a read-only document is brought up to date too, with nothing to undo", async ({ page }) => {
    await stale(page);
    await inPage(page, (w) => (w.fe as unknown as { setReadOnly(on: boolean): void }).setReadOnly(true));
    await exportAs(page, "docx");
    expect((await fields(page))[0].result).toBe("Ada");
    expect(await canUndo(page)).toBe(false);
    // The words a person wrote are still out of reach.
    const refused = await inPage(page, (w) => {
      try {
        (w.fe.editor.engine as unknown as { apply(op: unknown): unknown }).apply({ op: "insert_text", at: { p: w.fe.editor.engine.renderAll().order[0], o: 0 }, text: "X" });
        return false;
      } catch {
        return true;
      }
    });
    expect(refused).toBe(true);
  });

  test("PDF downloads are updated too", async ({ page }) => {
    await stale(page);
    await exportAs(page, "pdf");
    expect((await fields(page))[0].result).toBe("Ada");
  });

  test("printing updates the fields first", async ({ page }) => {
    await stale(page);
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await inPage(page, (w) => (w.fe.extensions as unknown as Host).execute("file.print"));
    expect((await fields(page))[0].result).toBe("Ada");
  });

  test("Tools, Update fields before printing turns it off, and it is on by default", async ({ page }) => {
    await stale(page);
    await openMenu(page, "Tools");
    const item = menuItem(page, "Update fields before printing");
    await expect(item).toHaveAttribute("aria-checked", "true");
    await item.click();
    await exportAs(page, "docx");
    expect((await fields(page))[0].result).toBe("stale");
    await openMenu(page, "Tools");
    await expect(menuItem(page, "Update fields before printing")).toHaveAttribute("aria-checked", "false");
    await page.keyboard.press("Escape");
    // Update fields (F9) still works.
    await page.keyboard.press("F9");
    await expect.poll(async () => (await fields(page))[0].result).toBe("Ada");
  });

  test("Tools, User information sets what USERNAME shows", async ({ page }) => {
    await openBuilt(page, doc([run("Me: ") + fld("USERNAME", "")]));
    await chooseMenu(page, "Tools", "User information…");
    const box = dialog(page, "User information");
    await box.getByLabel("Name").fill("Grace Hopper");
    await box.getByRole("button", { name: "Apply" }).click();
    await page.keyboard.press("F9");
    await expect.poll(async () => (await fields(page))[0].result).toBe("Grace Hopper");
  });
});
