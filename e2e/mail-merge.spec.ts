// Mail merge, from the Mailings menu: starting a merge, choosing recipients from a CSV or an Excel file (and from a source an
// extension adds), and editing the list. What is written into the file is read back from the saved .docx. The merge fields, the
// rules, the preview and Finish and merge are in mail-merge-fields.spec.ts.
import { openBuilt, zip } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, menuItem, openMenu, test } from "./helpers";
import { chooseCsv, eng, info, LETTER, mergeLetter, parts, toast } from "./support/mail-merge-helpers";

test.describe("starting a mail merge", () => {
  test("the Mailings menu is there, and what needs a merge is greyed out until one starts", async ({ page }) => {
    await openBuilt(page, LETTER);
    await openMenu(page, "Mailings");
    for (const label of ["Envelopes…", "Labels…", "Start mail merge", "Select recipients"]) await expect(menuItem(page, label)).toBeVisible();
    for (const label of ["Insert merge field…", "Address block…", "Greeting line…", "Preview results", "Finish and merge…", "Edit recipient list…"]) await expect(menuItem(page, label)).toBeDisabled();
    await page.keyboard.press("Escape");
    expect((await info(page)).active).toBe(false);
  });

  test("Letters writes Word's mail merge settings, and it undoes", async ({ page }) => {
    await openBuilt(page, LETTER);
    await chooseMenu(page, "Mailings", "Start mail merge", "Letters");
    expect(await info(page)).toMatchObject({ active: true, mainType: "formLetters", hasRecipients: false });
    const saved = await parts(page);
    expect(saved.get("word/settings.xml")).toContain('<w:mainDocumentType w:val="formLetters"/>');
    await page.keyboard.press("ControlOrMeta+z");
    expect((await info(page)).active).toBe(false);
    expect((await parts(page)).get("word/settings.xml")).not.toContain("mailMerge");
  });

  test("Directory is a kind of merge too, and Normal document ends it", async ({ page }) => {
    await openBuilt(page, LETTER);
    await chooseMenu(page, "Mailings", "Start mail merge", "Directory");
    expect((await info(page)).mainType).toBe("catalog");
    await chooseMenu(page, "Mailings", "Start mail merge", "Normal document (stop the mail merge)");
    expect((await info(page)).active).toBe(false);
  });
});

test.describe("recipients", () => {
  test("from a CSV file: the list is read, written as Word links it and kept in the file", async ({ page }) => {
    await mergeLetter(page);
    const i = await info(page);
    expect(i).toMatchObject({ active: true, mainType: "formLetters", sourceName: "people.csv", records: 4, included: 4 });
    expect(i.columns).toEqual(["First Name", "Last Name", "State", "Balance"]);
    await expect(toast(page)).toContainText("4 recipients from people.csv");
    const saved = await parts(page);
    const s = saved.get("word/settings.xml")!;
    expect(s).toContain('<w:dataType w:val="textFile"/>');
    expect(s).toContain("SELECT * FROM people.csv");
    expect(s).toContain('<w:colDelim w:val="44"/>');
    expect(saved.get("word/_rels/settings.xml.rels")).toMatch(/mailMergeSource.*file:\/\/\/people\.csv|file:\/\/\/people\.csv.*mailMergeSource/s);
    // Docxtor's own part has the list, so reopening the file keeps it.
    const part = [...saved].find(([n, v]) => /^customXml\/item\d+\.xml$/.test(n) && v.includes("mail-merge"));
    expect(part?.[1]).toContain("Lovelace");
    // A file that was saved and opened again is still a merge with its recipients.
    const bytes = await eng(page, (e) => Array.from(e.save()));
    await page.evaluate(async (b) => {
      const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(new Uint8Array(b), "again.docx");
    }, bytes);
    await expect.poll(async () => (await info(page)).records).toBe(4);
  });

  test("from an Excel file: the first sheet, its first row as the column names", async ({ page }) => {
    await openBuilt(page, LETTER);
    const xlsx = zip([
      ["[Content_Types].xml", '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>'],
      ["_rels/.rels", '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>'],
      ["xl/workbook.xml", '<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Members" sheetId="1" r:id="rId1"/></sheets></workbook>'],
      ["xl/_rels/workbook.xml.rels", '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>'],
      ["xl/worksheets/sheet1.xml", '<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>First Name</t></is></c><c r="B1" t="inlineStr"><is><t>Last Name</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Ada</t></is></c><c r="B2" t="inlineStr"><is><t>Lovelace</t></is></c></row></sheetData></worksheet>'],
    ]);
    const [chooser] = await Promise.all([page.waitForEvent("filechooser"), chooseMenu(page, "Mailings", "Select recipients", "From an Excel file…")]);
    await chooser.setFiles({ name: "members.xlsx", mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", buffer: Buffer.from(xlsx) });
    await expect.poll(async () => (await info(page)).hasRecipients).toBe(true);
    expect(await info(page)).toMatchObject({ records: 1, sourceName: "members.xlsx", columns: ["First Name", "Last Name"], mainType: "formLetters" });
    const s = (await parts(page)).get("word/settings.xml")!;
    expect(s).toContain('<w:dataType w:val="native"/>');
    expect(s).toContain("Members$");
  });

  test("a file that can't be read says why, and changes nothing", async ({ page }) => {
    await openBuilt(page, LETTER);
    const [chooser] = await Promise.all([page.waitForEvent("filechooser"), chooseMenu(page, "Mailings", "Select recipients", "From an Excel file…")]);
    await chooser.setFiles({ name: "broken.xlsx", mimeType: "application/octet-stream", buffer: Buffer.from("not a workbook") });
    await expect(toast(page)).toContainText("isn't an Excel");
    expect((await info(page)).hasRecipients).toBe(false);
  });

  test("an extension adds a source of recipients, and it is listed with the others", async ({ page }) => {
    await openBuilt(page, LETTER);
    await inPage(page, async (w) => {
      await (w.fe as unknown as { use(ext: unknown): Promise<void> }).use({
        id: "acme.sheets",
        name: "Acme sheets",
        activate(ctx: { mailMerge: { dataSources: { register(s: unknown): void } } }) {
          ctx.mailMerge.dataSources.register({ id: "acme.sheet", label: "From an Acme sheet…", pick: () => ({ columns: ["First Name", "Last Name"], rows: [["Ada", "Lovelace"], ["Alan", "Turing"]], name: "Acme sheet" }) });
        },
      });
    });
    await chooseMenu(page, "Mailings", "Select recipients", "From an Acme sheet…");
    await expect.poll(async () => (await info(page)).records).toBe(2);
    expect(await info(page)).toMatchObject({ sourceName: "Acme sheet", mainType: "formLetters" });
    // Not a file Word can link to, so the settings only name the kind of document.
    expect((await parts(page)).get("word/settings.xml")).not.toContain("SELECT *");
    await openMenu(page, "Mailings");
    await menuItem(page, "Select recipients").hover();
    await expect(menuItem(page, "From a CSV file…")).toBeVisible();
    await expect(menuItem(page, "From an Excel file…")).toBeVisible();
    await expect(menuItem(page, "From an Acme sheet…")).toBeVisible();
  });
});

test.describe("the recipient list", () => {
  test("ticks, a filter and a sort, applied once as one undo step", async ({ page }) => {
    await mergeLetter(page);
    await chooseMenu(page, "Mailings", "Edit recipient list…");
    const box = dialog(page, "Recipients");
    await expect(box).toBeVisible();
    await expect(box.locator("tbody tr")).toHaveCount(4);
    await expect(box).toContainText("4 of 4 recipients are in the merge.");
    // Untick Grace.
    await box.locator("tbody tr").nth(1).getByRole("checkbox").uncheck();
    await expect(box).toContainText("3 of 4 recipients are in the merge.");
    // Sort by last name, descending (click the heading twice).
    await box.getByRole("button", { name: "Sort by Last Name" }).click();
    await box.getByRole("button", { name: "Sort by Last Name" }).click();
    await expect(box.locator("tbody tr").first()).toContainText("Turing");
    // Filter to California.
    await box.getByRole("button", { name: "Filter…" }).click();
    await box.getByLabel("Column").selectOption("State");
    await box.getByLabel("Compare to").fill("CA");
    await expect(box.locator("tbody tr")).toHaveCount(2);
    await expect(box).toContainText("The filter shows 2.");
    await box.getByRole("button", { name: "OK" }).click();
    await expect(box).toBeHidden();
    const data = await eng(page, (e) => e.mergeData()!);
    // California, last name descending: Turing, then Lovelace; Grace stays unticked.
    expect(data.order).toEqual([2, 0]);
    expect(data.active).toEqual([true, false, true, true]);
    expect(await info(page)).toMatchObject({ records: 4, included: 2 });
    // One undo step puts the list back.
    await page.keyboard.press("ControlOrMeta+z");
    expect((await info(page)).included).toBe(4);
  });

  test("Select all and Clear all work on what the search shows", async ({ page }) => {
    await mergeLetter(page);
    await chooseMenu(page, "Mailings", "Edit recipient list…");
    const box = dialog(page, "Recipients");
    await box.getByRole("button", { name: "Clear all" }).click();
    await expect(box).toContainText("0 of 4 recipients are in the merge.");
    await box.getByLabel("Find in the list").first().fill("ing");
    await expect(box.locator("tbody tr")).toHaveCount(1);
    await box.getByRole("button", { name: "Select all" }).click();
    await expect(box).toContainText("1 of 4 recipients are in the merge.");
    await box.getByRole("button", { name: "Cancel" }).click();
    expect((await info(page)).included).toBe(4);
  });
});
