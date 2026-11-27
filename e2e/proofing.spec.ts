// "Do not check spelling or grammar" (text marked w:noProof, from a style, and File, Language) and Proofing options:
// words with capitals, numbers and addresses, repeated words, hiding the marks in one document, and Recheck document.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, contrasts, dialog, expect, inPage, menuItem, openMenu, savedPart, selectWord, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const doc = (paragraphs: DocSpec["sections"][0]["paragraphs"], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs }], ...extra });
const run = (text: string, rpr = "") => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;
const red = (page: Page) => page.locator(".sp-layer .sp-u");
const blue = (page: Page) => page.locator(".sp-layer .sp-g");
const redWords = (page: Page) => red(page).evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.word));

test.describe("do not check spelling or grammar", () => {
  test("text marked w:noProof is not checked, the rest is", async ({ page }) => {
    await openBuilt(page, doc([{ raw: run("recieve ", "<w:noProof/>") + run("and teh ") + run("seperate", "<w:noProof/>") + run(" thing") }]));
    await expect.poll(() => redWords(page)).toEqual(["teh"]);
  });

  test("a character style that turns checking off leaves its text alone", async ({ page }) => {
    await openBuilt(page, doc([{ raw: run("Plain recieve here. ") + run("code recieve", '<w:rStyle w:val="Code"/>') }], { styles: '<w:style w:type="character" w:customStyle="1" w:styleId="Code"><w:name w:val="Code"/><w:rPr><w:noProof/></w:rPr></w:style>' }));
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
    // The one in plain text is the first.
    const p = page.locator(".flow .fp").first();
    const underline = await red(page).first().boundingBox();
    const box = (await p.boundingBox())!;
    expect(underline!.x - box.x).toBeLessThan(120);
  });

  test("grammar is left alone in marked text too", async ({ page }) => {
    await openBuilt(page, doc([{ raw: run("It was the the end. ") + run("Also the the end.", "<w:noProof/>") }]));
    await expect(blue(page)).toHaveCount(1);
  });

  test("File, Language: the checkbox marks the selection, undoes in one step, and is written as w:noProof", async ({ page }) => {
    await openBuilt(page, doc(["I will recieve the parcel and teh letter."]));
    await expect.poll(() => redWords(page)).toEqual(["recieve", "teh"]);
    const id = Number(await page.locator(".flow .fp").first().getAttribute("data-p"));
    await selectWord(page, id, "recieve");
    await chooseMenu(page, "File", "Language…");
    const lang = dialog(page, "Language");
    const box = lang.getByRole("checkbox", { name: "Do not check spelling or grammar" });
    await expect(box).not.toBeChecked();
    await box.check();
    await lang.getByRole("button", { name: "Apply" }).click();
    await expect(lang).toBeHidden();
    await expect.poll(() => redWords(page)).toEqual(["teh"]);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<w:noProof/>");
    expect(xml.match(/<w:noProof\/>/g)!.length).toBe(1);
    // The language itself did not change.
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang ?? "")).toBe("");

    // The dialog shows the state of the selection, and unchecking turns checking back on.
    await chooseMenu(page, "File", "Language…");
    await expect(lang.getByRole("checkbox", { name: "Do not check spelling or grammar" })).toBeChecked();
    await lang.getByRole("checkbox", { name: "Do not check spelling or grammar" }).uncheck();
    await lang.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => redWords(page)).toEqual(["recieve", "teh"]);

    // One undo step each way.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => redWords(page)).toEqual(["teh"]);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => redWords(page)).toEqual(["recieve", "teh"]);
  });
});

test.describe("proofing options", () => {
  const sample = ["See ZXQW and abc123 at www.exmplz.com and the the cat."];
  const open = async (page: Page) => {
    await chooseMenu(page, "Tools", "Spelling and grammar", "Proofing options…");
    const d = dialog(page, "Proofing options");
    await expect(d).toBeVisible();
    return d;
  };

  test("by default capitals, numbers and addresses are left alone and repeated words are flagged", async ({ page }) => {
    await openBuilt(page, doc(sample));
    await expect.poll(() => blue(page).count()).toBe(1);
    expect(await redWords(page)).toEqual([]);
    const d = await open(page);
    for (const name of ["Ignore words in UPPERCASE", "Ignore words that contain numbers", "Ignore Internet and file addresses", "Flag repeated words"]) {
      await expect(d.getByRole("checkbox", { name })).toBeChecked();
    }
    await expect(d.getByRole("checkbox", { name: "Hide spelling errors in this document only" })).not.toBeChecked();
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toBeHidden();
  });

  test("each option changes what is flagged, and Cancel changes nothing", async ({ page }) => {
    await openBuilt(page, doc(sample));
    await expect.poll(() => blue(page).count()).toBe(1);
    let d = await open(page);
    await d.getByRole("checkbox", { name: "Ignore words in UPPERCASE" }).uncheck();
    await d.getByRole("button", { name: "Cancel" }).click();
    await page.waitForTimeout(300);
    expect(await redWords(page)).toEqual([]);

    d = await open(page);
    await d.getByRole("checkbox", { name: "Ignore words in UPPERCASE" }).uncheck();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => redWords(page)).toEqual(["ZXQW"]);

    d = await open(page);
    await d.getByRole("checkbox", { name: "Ignore words that contain numbers" }).uncheck();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => redWords(page)).toEqual(["ZXQW", "abc123"]);

    d = await open(page);
    await d.getByRole("checkbox", { name: "Ignore Internet and file addresses" }).uncheck();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => redWords(page)).toEqual(["ZXQW", "abc123", "www", "exmplz"]);

    d = await open(page);
    await d.getByRole("checkbox", { name: "Flag repeated words" }).uncheck();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(blue(page)).toHaveCount(0);

    // Turning them back on brings back the defaults.
    d = await open(page);
    for (const name of ["Ignore words in UPPERCASE", "Ignore words that contain numbers", "Ignore Internet and file addresses", "Flag repeated words"]) await d.getByRole("checkbox", { name }).check();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => redWords(page)).toEqual([]);
    await expect(blue(page)).toHaveCount(1);
  });

  test("the choices are remembered after a reload", async ({ page }) => {
    await openBuilt(page, doc(sample));
    const d = await open(page);
    await d.getByRole("checkbox", { name: "Ignore words in UPPERCASE" }).uncheck();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => redWords(page)).toEqual(["ZXQW"]);
    await openBuilt(page, doc(sample));
    await expect.poll(() => redWords(page)).toEqual(["ZXQW"]);
    const again = await open(page);
    await expect(again.getByRole("checkbox", { name: "Ignore words in UPPERCASE" })).not.toBeChecked();
  });

  test("Hide spelling errors in this document: the marks go, the file says so, undo brings them back", async ({ page }) => {
    await openBuilt(page, doc(["I will recieve the the parcel."]));
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
    await expect(blue(page)).toHaveCount(1);
    await chooseMenu(page, "Tools", "Spelling and grammar", "Hide spelling errors in this document");
    await expect(red(page)).toHaveCount(0);
    await expect(blue(page)).toHaveCount(1);
    expect(await savedPart(page, "word/settings.xml")).toContain("<w:hideSpellingErrors/>");
    expect(await inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as unknown as { hideSpellingErrors?: boolean }).hideSpellingErrors)).toBe(true);
    // The menu line is checked.
    await openMenu(page, "Tools");
    await menuItem(page, "Spelling and grammar").click();
    await expect(menuItem(page, "Hide spelling errors in this document")).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");

    // The check panel still finds the word.
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");
    await expect(page.locator(".sp-panel .sp-word")).toHaveText("recieve");
    await page.locator(".sp-panel").getByRole("button", { name: "Close" }).click();

    await chooseMenu(page, "Tools", "Spelling and grammar", "Hide grammar errors in this document");
    await expect(blue(page)).toHaveCount(0);

    // Undo shows them again; the setting is part of the document and goes through a save and an open.
    await page.keyboard.press(`${MOD}+Z`);
    await expect(blue(page)).toHaveCount(1);
    const bytes = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));
    await inPage(page, (w, b) => w.fe.open(new Uint8Array(b), "again.docx"), bytes);
    await expect(page.locator(".flow .fp").first()).toContainText("recieve");
    await page.waitForTimeout(600);
    await expect(red(page)).toHaveCount(0);
    await expect(blue(page)).toHaveCount(1);
  });

  test("the same choice in the dialog is written to the file", async ({ page }) => {
    await openBuilt(page, doc(["I will recieve the the parcel."]));
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
    const d = await open(page);
    await d.getByRole("checkbox", { name: "Hide spelling errors in this document only" }).check();
    await d.getByRole("checkbox", { name: "Hide grammar errors in this document only" }).check();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(red(page)).toHaveCount(0);
    await expect(blue(page)).toHaveCount(0);
    const xml = await savedPart(page, "word/settings.xml");
    expect(xml).toContain("<w:hideSpellingErrors/>");
    expect(xml).toContain("<w:hideGrammaticalErrors/>");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
    expect(await savedPart(page, "word/settings.xml")).not.toContain("hideSpellingErrors");
  });

  test("Recheck document brings back what was ignored", async ({ page }) => {
    await openBuilt(page, doc(["I will recieve the parcel."]));
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
    const box = (await red(page).first().boundingBox())!;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    await contextItem(page, "Ignore").click();
    await expect(red(page)).toHaveCount(0);
    await chooseMenu(page, "Tools", "Spelling and grammar", "Recheck document");
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
    // The dialog has the same button.
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: "right" });
    await contextItem(page, "Ignore").click();
    await expect(red(page)).toHaveCount(0);
    const d = await open(page);
    await d.getByRole("button", { name: "Recheck document" }).click();
    await expect(d).toBeHidden();
    await expect.poll(() => redWords(page)).toEqual(["recieve"]);
  });
});

test.describe("more proofing options", () => {
  const open = async (page: Page) => {
    await chooseMenu(page, "Tools", "Spelling and grammar", "Proofing options…");
    const d = dialog(page, "Proofing options");
    await expect(d).toBeVisible();
    return d;
  };
  const setOption = async (page: Page, name: string, on: boolean) => {
    const d = await open(page);
    const box = d.getByRole("checkbox", { name });
    if (on) await box.check();
    else await box.uncheck();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toBeHidden();
  };

  test("the new choices are offered with Word's defaults", async ({ page }) => {
    await openBuilt(page, doc(["Say the word."]));
    const d = await open(page);
    await expect(d.getByRole("checkbox", { name: "Detect language automatically" })).toBeChecked();
    await expect(d.getByRole("checkbox", { name: "Enforce accented uppercase in French" })).not.toBeChecked();
    await expect(d.getByRole("checkbox", { name: "Suggest from main dictionary only" })).not.toBeChecked();
    await expect(d.getByRole("checkbox", { name: "Frequently confused words" })).toBeChecked();
    await expect(d.getByRole("checkbox", { name: "Show readability statistics" })).not.toBeChecked();
    await expect(d.getByRole("button", { name: "Custom dictionaries…" })).toBeVisible();
  });

  test("a Spanish paragraph in an English document is checked in Spanish; without detection its words are English mistakes", async ({ page }) => {
    await openBuilt(page, doc(["The committee met today and agreed on the plan.", "El comité revisará la propuesta y enviará su respuessta antes del final de la semana."], { lang: "en-US" }));
    await expect.poll(() => redWords(page), { timeout: 15000 }).toEqual(["respuessta"]);
    await setOption(page, "Detect language automatically", false);
    await expect.poll(() => redWords(page)).toContain("propuesta");
    await setOption(page, "Detect language automatically", true);
    await expect.poll(() => redWords(page), { timeout: 15000 }).toEqual(["respuessta"]);
  });

  test("French capitals may go without their accents, unless accented capitals are enforced", async ({ page }) => {
    await openBuilt(page, doc(["Ecole fermée aujourd'hui, mais la rentrée est demain."], { lang: "fr-FR" }));
    await expect.poll(() => redWords(page), { timeout: 15000 }).toEqual([]);
    await setOption(page, "Enforce accented uppercase in French", true);
    await expect.poll(() => redWords(page), { timeout: 15000 }).toEqual(["Ecole"]);
  });

  test("frequently confused words are flagged, and the choice turns them off", async ({ page }) => {
    await openBuilt(page, doc(["Your welcome to stay with us."]));
    await expect.poll(() => blue(page).count()).toBe(1);
    await setOption(page, "Frequently confused words", false);
    await expect(blue(page)).toHaveCount(0);
  });

  test("readability statistics: from the menu, and after the check when asked for", async ({ page }) => {
    await openBuilt(page, doc(["The cat sat on the mat. The report was written by the team."], { lang: "en-US" }));
    await chooseMenu(page, "Tools", "Spelling and grammar", "Readability statistics…");
    const stats = dialog(page, "Readability statistics");
    await expect(stats).toBeVisible();
    const value = (label: string) => stats.locator(".rs-row", { has: page.getByText(label, { exact: true }) }).locator("dd");
    await expect(value("Words")).toHaveText("13");
    await expect(value("Sentences")).toHaveText("2");
    await expect(value("Passive sentences")).toHaveText("50%");
    await expect(value("Flesch Reading Ease")).not.toHaveText("");
    await stats.getByRole("button", { name: "OK" }).click();
    await expect(stats).toBeHidden();

    // Show readability statistics: they come up when the check is done.
    await setOption(page, "Show readability statistics", true);
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+Alt+X`);
    await expect(dialog(page, "Readability statistics")).toBeVisible();
  });

  test("custom dictionaries: a French-only list counts only in French, off counts nowhere, and its words are suggested unless suggestions are from the main dictionary only", async ({ page }) => {
    await openBuilt(page, doc(["I like zorgle and blarf today.", { text: "", xml: '<w:r><w:rPr><w:lang w:val="fr-FR"/></w:rPr><w:t>Le mot glorbe est ici.</w:t></w:r>' }], { lang: "en-US" }));
    await expect.poll(() => redWords(page), { timeout: 15000 }).toEqual(["zorgle", "blarf", "glorbe"]);

    await chooseMenu(page, "Tools", "Spelling and grammar", "Custom dictionaries…");
    const d = dialog(page, "Custom dictionaries");
    await expect(d.locator(".dict-row")).toHaveCount(1);
    await d.getByRole("button", { name: "New dictionary" }).click();
    await d.getByLabel("Name").fill("French words");
    await d.getByLabel("Name").press("Tab");
    await d.getByLabel("Language").selectOption("fr-FR");
    await d.getByRole("textbox", { name: "Add a word" }).fill("glorbe blarf");
    await d.getByRole("button", { name: "Add" }).click();
    await expect(d.locator(".sp-word-text")).toHaveText(["blarf", "glorbe"]);
    await expect(d.getByRole("button", { name: /French words/ })).toBeVisible();
    await d.getByRole("button", { name: "Done" }).click();
    // glorbe is accepted in the French paragraph; blarf is still a mistake in the English one.
    await expect.poll(() => redWords(page)).toEqual(["zorgle", "blarf"]);

    await chooseMenu(page, "Tools", "Spelling and grammar", "Custom dictionaries…");
    await dialog(page, "Custom dictionaries").getByRole("checkbox", { name: "Use French words" }).uncheck();
    await dialog(page, "Custom dictionaries").getByRole("button", { name: "Done" }).click();
    await expect.poll(() => redWords(page)).toEqual(["zorgle", "blarf", "glorbe"]);

    // A word in the default dictionary close to a misspelling is offered as its correction.
    await chooseMenu(page, "Tools", "Spelling and grammar", "Custom dictionaries…");
    const again = dialog(page, "Custom dictionaries");
    await again.getByRole("button", { name: /Personal/ }).click();
    await again.getByRole("textbox", { name: "Add a word" }).fill("Zorglex");
    await again.getByRole("textbox", { name: "Add a word" }).press("Enter");
    await again.getByRole("button", { name: "Done" }).click();
    const word = page.locator('.sp-layer .sp-u[data-word="zorgle"]');
    const b = (await word.boundingBox())!;
    await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2, { button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Zorglex")).toBeVisible();
    await page.keyboard.press("Escape");

    await setOption(page, "Suggest from main dictionary only", true);
    await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2, { button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Zorglex")).toHaveCount(0);
  });
});

for (const scheme of ["light", "dark"] as const) {
  test.describe(`proofing dialogs in the ${scheme} theme`, () => {
    test.use({ colorScheme: scheme });

    test("Custom dictionaries, Readability statistics and Proofing options are readable", async ({ page }) => {
      await openBuilt(page, doc(["The cat sat on the mat. The report was written by the team."], { lang: "en-US" }));
      await chooseMenu(page, "Tools", "Spelling and grammar", "Custom dictionaries…");
      let found = await contrasts(page, [".dict-row .dict-name", ".dict-row .dict-lang", ".dict-row .dict-default", ".dict-fields label", ".sp-empty"]);
      for (const f of found) expect(f.ratio, f.selector).toBeGreaterThanOrEqual(4.5);
      await dialog(page, "Custom dictionaries").getByRole("button", { name: "Done" }).click();

      await chooseMenu(page, "Tools", "Spelling and grammar", "Readability statistics…");
      found = await contrasts(page, [".rs-row dt", ".rs-row dd", ".rs-group .pref-group"]);
      for (const f of found) expect(f.ratio, f.selector).toBeGreaterThanOrEqual(4.5);
      await dialog(page, "Readability statistics").getByRole("button", { name: "OK" }).click();

      await chooseMenu(page, "Tools", "Spelling and grammar", "Proofing options…");
      found = await contrasts(page, [".pref-check .check span", ".pref-help", ".pref-group"]);
      for (const f of found) expect(f.ratio, f.selector).toBeGreaterThanOrEqual(4.5);
      await page.locator(".dialog").screenshot({ path: `test-results/proofing-options-${scheme}.png` });
    });
  });
}

