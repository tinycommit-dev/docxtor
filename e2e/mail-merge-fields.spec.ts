// Mail merge, writing the letter: Insert merge field, Address block, Greeting line, Match fields and the rules; highlighting the
// fields; previewing records and Find recipient; and Finish and merge into a new document and into a PDF.
import { readFileSync } from "node:fs";
import type { DocSpec } from "./support/docx-builder";
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, menuItem, openMenu, test } from "./helpers";
import { bodyText, caretAtEndOf, chooseCsv, CSV2, eng, fields, fileName, info, LETTER, line, mergeField, mergeLetter, pagesOf, parts, run, strip, toast } from "./support/mail-merge-helpers";
import { readPdf } from "./support/pdf-reader";

test.describe("merge fields", () => {
  test("Insert merge field puts the column in as a Word field and stays open for the next one", async ({ page }) => {
    await mergeLetter(page);
    await caretAtEndOf(page, 2);
    await chooseMenu(page, "Mailings", "Insert merge field…");
    const box = dialog(page, "Insert merge field");
    await expect(box.getByRole("listbox").getByRole("option")).toHaveText(["First Name", "Last Name", "State", "Balance"]);
    await box.getByRole("option", { name: "State" }).click();
    await box.getByRole("button", { name: "Insert", exact: true }).click();
    await expect(box).toBeVisible();
    // Options: text before and after, capitals.
    await box.getByText("Options").click();
    await box.getByLabel("Text before").fill("(");
    await box.getByLabel("Text after").fill(")");
    await box.getByLabel("Capitals").selectOption("upper");
    await box.getByRole("option", { name: "Last Name" }).click();
    await box.getByRole("button", { name: "Insert", exact: true }).click();
    await box.getByRole("button", { name: "Close" }).click();
    const all = await fields(page);
    const added = all.filter((f) => f.p === all.at(-1)!.p);
    expect(added.map((f) => f.instr.trim())).toEqual(["MERGEFIELD State", String.raw`MERGEFIELD Last_Name \* Upper \b "(" \f ")"`]);
    expect(added[0].result).toBe("«State»");
    await expect(line(page, 2)).toContainText("Yours sincerely«State»«Last_Name»");
    // They are complex fields, not simple ones.
    expect((await parts(page)).get("word/document.xml")).not.toContain("fldSimple");
    // One undo step for each.
    await page.keyboard.press("ControlOrMeta+z");
    await page.keyboard.press("ControlOrMeta+z");
    await expect(line(page, 2)).toHaveText("Yours sincerely");
  });

  test("the address fields list is Word's, and a field can be inserted with a double click", async ({ page }) => {
    await mergeLetter(page);
    await caretAtEndOf(page, 2);
    await chooseMenu(page, "Mailings", "Insert merge field…");
    const box = dialog(page, "Insert merge field");
    await box.getByRole("radio", { name: "Address fields" }).click();
    await expect(box.getByRole("option", { name: "Courtesy Title" })).toBeVisible();
    await expect(box.getByRole("option", { name: "Postal Code" })).toBeVisible();
    await box.getByRole("option", { name: "First Name" }).dblclick();
    expect((await fields(page)).map((f) => f.instr.trim())).toContain("MERGEFIELD First_Name");
    await box.getByRole("button", { name: "Close" }).click();
  });

  test("Address block: the preview follows the options and the record, and it inserts one ADDRESSBLOCK", async ({ page }) => {
    await openBuilt(page, LETTER);
    await chooseMenu(page, "Mailings", "Start mail merge", "Letters");
    await chooseCsv(page, CSV2);
    await caretAtEndOf(page, 2);
    await chooseMenu(page, "Mailings", "Address block…");
    const box = dialog(page, "Insert address block");
    const preview = box.locator(".mm-preview");
    await expect(preview).toHaveText("Ada Lovelace\n12 Crumb Rd\nLondon, N1 9GU");
    await box.getByLabel("How the name is written").selectOption("title_first_last");
    await expect(preview).toHaveText("Ms. Ada Lovelace\n12 Crumb Rd\nLondon, N1 9GU");
    // The country: only when it isn't your own.
    await box.getByLabel("The country or region").selectOption("different");
    await expect(preview).toContainText("UNITED KINGDOM");
    await box.getByRole("button", { name: "Next record" }).click();
    await expect(preview).toHaveText("Rear Adm. Grace Hopper\n1 Navy Way\nArlington, VA 22202");
    await box.getByLabel("The country or region").selectOption("always");
    await expect(preview).toContainText("UNITED STATES");
    // A recipient with no name loses the line; the address stays.
    await box.getByRole("button", { name: "Last record" }).click();
    await expect(preview).toHaveText("5 Main St\nSpringfield, IL 62701");
    await box.getByLabel("The country or region").selectOption("never");
    await box.getByRole("button", { name: "Insert", exact: true }).click();
    await expect(box).toBeHidden();
    const f = (await fields(page)).find((x) => x.code === "ADDRESSBLOCK")!;
    expect(f.result).toBe("«AddressBlock»");
    expect(f.instr).toContain("\\c 0");
    expect(f.instr).toContain("<Courtesy Title> <First Name> <Last Name> <Suffix>¶");
  });

  test("Greeting line: Dear Ms. Lovelace, and the line for a recipient with no name", async ({ page }) => {
    await openBuilt(page, LETTER);
    await chooseMenu(page, "Mailings", "Start mail merge", "Letters");
    await chooseCsv(page, CSV2);
    await caretAtEndOf(page, 2);
    await chooseMenu(page, "Mailings", "Greeting line…");
    const box = dialog(page, "Insert greeting line");
    const preview = box.locator(".mm-preview");
    await expect(preview).toHaveText("Dear Ms. Lovelace,");
    await box.getByLabel("What ends the greeting").selectOption(":");
    await expect(preview).toHaveText("Dear Ms. Lovelace:");
    await box.getByLabel("The words before the name").selectOption("Hello ");
    await box.getByLabel("How the name is written").selectOption("first");
    await expect(preview).toHaveText("Hello Ada:");
    await box.getByRole("button", { name: "Last record" }).click();
    await expect(preview).toHaveText("Dear Sir or Madam,");
    await box.getByRole("button", { name: "Insert", exact: true }).click();
    const f = (await fields(page)).find((x) => x.code === "GREETINGLINE")!;
    expect(f.instr).toContain("<< _BEFORE_ Hello >><< _FIRST0_>><< _AFTER_ :>>");
  });

  test("Match fields: a column for each address field, written into Word's settings", async ({ page }) => {
    await openBuilt(page, LETTER);
    await chooseMenu(page, "Mailings", "Start mail merge", "Letters");
    await chooseCsv(page, "Given,Family,Addr\nAda,Lovelace,12 Crumb Rd\n");
    // Docxtor guesses what it knows: Addr isn't a name it recognises.
    const guessed = await eng(page, (e) => e.mergeData()!.matched);
    expect(guessed.find(([f]) => f === "Address 1")![1]).toBeNull();
    await chooseMenu(page, "Mailings", "Match fields…");
    const box = dialog(page, "Match fields");
    await box.getByLabel("First Name").selectOption("Given");
    await box.getByLabel("Last Name").selectOption("Family");
    await box.getByLabel("Address 1").selectOption("Addr");
    await box.getByRole("button", { name: "OK" }).click();
    expect(await eng(page, (e) => e.mergeInfo().mapping)).toMatchObject({ "First Name": "Given", "Last Name": "Family", "Address 1": "Addr" });
    expect((await parts(page)).get("word/settings.xml")).toContain('<w:mappedName w:val="Addr"/>');
    // The preview of an address block now finds the columns.
    await caretAtEndOf(page, 2);
    await chooseMenu(page, "Mailings", "Address block…");
    await expect(dialog(page, "Insert address block").locator(".mm-preview")).toHaveText("Ada Lovelace\n12 Crumb Rd");
  });
});

test.describe("rules", () => {
  test("If…Then…Else… writes an IF with the column inside it", async ({ page }) => {
    await mergeLetter(page);
    await caretAtEndOf(page, 2);
    await chooseMenu(page, "Mailings", "Rules", "If…Then…Else…");
    const box = dialog(page, "Insert If…Then…Else");
    await box.getByRole("button", { name: "Insert", exact: true }).click();
    await expect(box).toContainText("Fill in the boxes that are not marked optional.");
    await box.getByLabel("Column").selectOption("State");
    await box.getByLabel("Compare to").fill("CA");
    await box.getByLabel("Insert this text", { exact: true }).fill("Sunny");
    await box.getByLabel("Otherwise insert this text (optional)").fill("Elsewhere");
    await box.getByRole("button", { name: "Insert", exact: true }).click();
    await expect(box).toBeHidden();
    const f = (await fields(page)).find((x) => x.code === "IF")!;
    // The field inside it shows as its placeholder in the code, and shows the text for the other case.
    expect(f.instr.trim()).toBe('IF "«State»" = "CA" "Sunny" "Elsewhere"');
    expect(f.result).toBe("Elsewhere");
    expect((await fields(page)).some((x) => x.instr.trim() === "MERGEFIELD State")).toBe(true);
  });

  test("Ask, Fill-in, Set bookmark, Next record if and Skip record if each have their dialog", async ({ page }) => {
    await mergeLetter(page);
    await caretAtEndOf(page, 2);
    const rule = async (name: string, dialogName: string, fill: (box: ReturnType<typeof dialog>) => Promise<void>) => {
      await chooseMenu(page, "Mailings", "Rules", name);
      const box = dialog(page, dialogName);
      await fill(box);
      await box.getByRole("button", { name: "Insert", exact: true }).click();
      await expect(box).toBeHidden();
    };
    await rule("Ask…", "Insert Ask", async (b) => {
      await b.getByLabel("Bookmark").fill("Due date");
      await b.getByLabel("Prompt").fill("When is it due?");
      await b.getByLabel("Default answer (optional)").fill("Friday");
      await b.getByLabel("Ask once for the whole merge").check();
    });
    await rule("Fill-in…", "Insert Fill-in", async (b) => b.getByLabel("Prompt").fill("Which room?"));
    await rule("Set bookmark…", "Insert Set bookmark", async (b) => {
      await b.getByLabel("Bookmark").fill("Signer");
      await b.getByLabel("Value").fill("Ada");
    });
    await rule("Next record if…", "Insert Next record if", async (b) => {
      await b.getByLabel("Column").selectOption("State");
      await b.getByLabel("Comparison").selectOption("empty");
      await expect(b.getByLabel("Compare to")).toBeHidden();
    });
    await rule("Skip record if…", "Insert Skip record if", async (b) => {
      await b.getByLabel("Column").selectOption("Last Name");
      await b.getByLabel("Compare to").fill("Hopper");
    });
    await chooseMenu(page, "Mailings", "Rules", "Merge record #");
    await chooseMenu(page, "Mailings", "Rules", "Merge sequence #");
    // "Next record" is in the Rules menu and in the preview group; the rule is the one in the submenu.
    await openMenu(page, "Mailings");
    await menuItem(page, "Rules").click();
    await menuItem(page, "Next record").last().click();
    const all = await fields(page);
    const last = all.at(-1)!.p;
    // The fields inside the two rules that compare a column are listed apart; what is left is the rules themselves.
    const rules = all.filter((f) => f.p === last && f.code !== "MERGEFIELD").map((f) => f.instr.replace(/\s+/g, " ").trim());
    expect(rules).toEqual([
      String.raw`ASK Due_date "When is it due?" \d "Friday" \o`,
      'FILLIN "Which room?"',
      'SET Signer "Ada"',
      'NEXTIF "«State»" = ""',
      'SKIPIF "«Last_Name»" = "Hopper"',
      "MERGEREC",
      "MERGESEQ",
      "NEXT",
    ]);
    // Ask and Fill-in are what the merge asks about.
    const prompts = await eng(page, (e) => JSON.parse((e as unknown as { raw: { mergePrompts(): string } }).raw.mergePrompts()) as { key: string; once: boolean; default: string }[]);
    expect(prompts.map((p) => [p.key, p.once, p.default])).toEqual([["ask:due_date", true, "Friday"], ["fill:Which room?", false, ""]]);
  });
});

test.describe("highlight and preview", () => {
  test("Highlight merge fields tints them and takes the tint off again", async ({ page }) => {
    await mergeLetter(page);
    const field = page.locator('.flow [data-fld="MERGEFIELD"]').first();
    const bg = () => field.evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(await bg()).toBe("rgba(0, 0, 0, 0)");
    await chooseMenu(page, "Mailings", "Highlight merge fields");
    expect(await bg()).not.toBe("rgba(0, 0, 0, 0)");
    await chooseMenu(page, "Mailings", "Highlight merge fields");
    expect(await bg()).toBe("rgba(0, 0, 0, 0)");
  });

  test("Preview results shows each record in place of the template, and the template saves", async ({ page }) => {
    await mergeLetter(page);
    const template = (await parts(page)).get("word/document.xml")!;
    await chooseMenu(page, "Mailings", "Preview results");
    await expect(strip(page)).toContainText("Record 1 of 4");
    await expect(line(page, 0)).toHaveText("Dear Ada Lovelace,");
    await expect(line(page, 1)).toHaveText("Your balance is $1,234.50.");
    await strip(page).getByRole("button", { name: "Next record" }).click();
    await expect(strip(page)).toContainText("Record 2 of 4");
    await expect(line(page, 0)).toHaveText("Dear Grace Hopper,");
    // A blank balance writes nothing for the field.
    await strip(page).getByRole("button", { name: "Next record" }).click();
    await expect(line(page, 1)).toHaveText("Your balance is .");
    await strip(page).getByRole("button", { name: "Last record" }).click();
    await expect(strip(page)).toContainText("Record 4 of 4");
    await expect(line(page, 0)).toHaveText("Dear Edsger Dijkstra,");
    await expect(strip(page).getByRole("button", { name: "Next record" })).toBeDisabled();
    await strip(page).getByRole("button", { name: "First record" }).click();
    await expect(strip(page).getByRole("button", { name: "Previous record" })).toBeDisabled();
    // Nothing can be typed while a record is shown, and the file keeps the template.
    await line(page, 0).click();
    await page.keyboard.type("zzz");
    await expect(line(page, 0)).toHaveText("Dear Ada Lovelace,");
    expect((await parts(page)).get("word/document.xml")).toBe(template);
    await openMenu(page, "Mailings");
    await expect(menuItem(page, "Address block…")).toBeDisabled();
    await page.keyboard.press("Escape");
    // Done brings the template back, with its fields, and editing again.
    await strip(page).getByRole("button", { name: "Done" }).click();
    await expect(strip(page)).toBeHidden();
    await expect(line(page, 0)).toHaveText("Dear «First_Name» «Last_Name»,");
    expect((await fields(page)).length).toBe(3);
    await caretAtEndOf(page, 2);
    await page.keyboard.type("!");
    await expect(line(page, 2)).toHaveText("Yours sincerely!");
  });

  test("the first, previous, next and last record commands start the preview", async ({ page }) => {
    await mergeLetter(page);
    await chooseMenu(page, "Mailings", "Last record");
    await expect(strip(page)).toContainText("Record 4 of 4");
    await chooseMenu(page, "Mailings", "Previous record");
    await expect(strip(page)).toContainText("Record 3 of 4");
    await chooseMenu(page, "Mailings", "First record");
    await expect(strip(page)).toContainText("Record 1 of 4");
    await chooseMenu(page, "Mailings", "Preview results");
    await expect(strip(page)).toBeHidden();
    expect((await info(page)).previewing).toBeNull();
  });

  test("Find recipient shows the record that has the text, and Find next goes on", async ({ page }) => {
    await mergeLetter(page);
    await chooseMenu(page, "Mailings", "Find recipient…");
    const box = dialog(page, "Find recipient");
    await box.getByLabel("Find", { exact: true }).fill("Dijk");
    await box.getByRole("button", { name: "Find next" }).click();
    await expect(strip(page)).toContainText("Record 4 of 4");
    await expect(box).toContainText("Showing the only recipient");
    await box.getByLabel("Find", { exact: true }).fill("CA");
    await box.getByLabel("In", { exact: true }).selectOption("State");
    await box.getByRole("button", { name: "Find next" }).click();
    await expect(strip(page)).toContainText("Record 1 of 4");
    await box.getByRole("button", { name: "Find next" }).click();
    await expect(strip(page)).toContainText("Record 3 of 4");
    await expect(box).toContainText("Showing 2 of 2");
    await box.getByLabel("Find", { exact: true }).fill("zzz");
    await box.getByRole("button", { name: "Find next" }).click();
    await expect(box).toContainText("No recipient has that.");
  });
});

test.describe("finish and merge", () => {
  test("a new document with a section for each record, and the template is untouched", async ({ page }) => {
    await mergeLetter(page);
    const template = (await parts(page)).get("word/document.xml")!;
    await chooseMenu(page, "Mailings", "Finish and merge…");
    const box = dialog(page, "Finish and merge");
    await expect(box).toContainText("4 recipients are in the merge.");
    await box.getByRole("button", { name: "Merge" }).click();
    // The result opens as a document of its own.
    await expect.poll(() => fileName(page)).toMatch(/merged/);
    const merged = await bodyText(page);
    expect(merged.split("\n").filter((l) => l.startsWith("Dear"))).toEqual(["Dear Ada Lovelace,", "Dear Grace Hopper,", "Dear Alan Turing,", "Dear Edsger Dijkstra,"]);
    expect(merged).toContain("Your balance is $1,234.50.");
    expect((await eng(page, (e) => e.pageSetup())).sections).toHaveLength(4);
    expect(await fields(page)).toEqual([]);
    expect((await info(page)).active).toBe(false);
    const out = await parts(page);
    expect(out.get("word/settings.xml")).not.toContain("mailMerge");
    expect([...out.keys()].some((n) => /^customXml\/item\d+\.xml$/.test(n))).toBe(false);
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    expect(template).toContain("MERGEFIELD");
    // One page for each record.
    await expect.poll(() => pagesOf(page)).toBe(4);
  });

  test("only the record on show, or a range, and a bad range is refused", async ({ page }) => {
    await mergeLetter(page);
    await chooseMenu(page, "Mailings", "Next record");
    await expect(strip(page)).toContainText("Record 1 of 4");
    await chooseMenu(page, "Mailings", "Next record");
    await chooseMenu(page, "Mailings", "Finish and merge…");
    const box = dialog(page, "Finish and merge");
    await box.getByLabel("Records").selectOption("range");
    await box.getByLabel("From record").fill("3");
    await box.getByLabel("To record").fill("2");
    await box.getByRole("button", { name: "Merge" }).click();
    await expect(box).toContainText("Give two record numbers between 1 and 4");
    await box.getByLabel("Records").selectOption("current");
    await box.getByRole("button", { name: "Merge" }).click();
    await expect.poll(() => fileName(page)).toMatch(/merged/);
    expect((await bodyText(page)).split("\n").filter((l) => l.startsWith("Dear"))).toEqual(["Dear Grace Hopper,"]);
  });

  test("Fill-in questions are asked first, for each record or for the rest", async ({ page }) => {
    const fill = `<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> FILLIN "Which room?" \\d "A1" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>A1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`;
    const spec: DocSpec = { sections: [{ paragraphs: [{ raw: run("Room: ") + fill + run(" for ") + mergeField("First_Name") }] }] };
    await mergeLetter(page, spec);
    await chooseMenu(page, "Mailings", "Finish and merge…");
    await dialog(page, "Finish and merge").getByRole("button", { name: "Merge" }).click();
    const q = dialog(page, /^Record 1 of 4/);
    await expect(q).toBeVisible();
    await q.getByLabel("Which room?").fill("B2");
    await q.getByRole("button", { name: "Continue" }).click();
    const q2 = dialog(page, /^Record 2 of 4/);
    await q2.getByLabel("Which room?").fill("C3");
    await q2.getByLabel("Use these answers for the records that are left").check();
    await q2.getByRole("button", { name: "Continue" }).click();
    await expect.poll(() => fileName(page)).toMatch(/merged/);
    expect((await bodyText(page)).split("\n").filter(Boolean)).toEqual(["Room: B2 for Ada", "Room: C3 for Grace", "Room: C3 for Alan", "Room: C3 for Edsger"]);
  });

  test("a PDF with a page for each record", async ({ page }) => {
    await mergeLetter(page);
    await chooseMenu(page, "Mailings", "Finish and merge…");
    const box = dialog(page, "Finish and merge");
    await box.getByLabel("Make").selectOption("pdf");
    const [download] = await Promise.all([page.waitForEvent("download"), box.getByRole("button", { name: "Merge" }).click()]);
    expect(download.suggestedFilename()).toMatch(/merged\.pdf$/);
    const pdf = readPdf(readFileSync((await download.path())!));
    expect(pdf.pages).toHaveLength(4);
    expect(pdf.pageText[0]).toContain("Dear Ada Lovelace,");
    expect(pdf.pageText[3]).toContain("Dear Edsger Dijkstra,");
    // The document on screen is still the template.
    await expect(line(page, 0)).toHaveText("Dear «First_Name» «Last_Name»,");
    await expect(toast(page)).toBeVisible();
  });
});
