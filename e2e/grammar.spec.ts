// Grammar: Docxtor's own rules draw blue underlines on the spell layer, the right-click menu and the check panel
// offer the fixes, the switch under Tools turns them off, other extensions can add checkers and dictionaries,
// and each paragraph is checked in its own language.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, dialog, expect, inPage, menuItem, openMenu, selectWord, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const doc = (paragraphs: string[], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs }], lang: "en-US", ...extra });
async function openDoc(page: Page, paragraphs: string[], extra: Partial<DocSpec> = {}) {
  await openBuilt(page, doc(paragraphs, extra));
}

const flowParas = (page: Page) => page.locator(".flow .fp");
const spelling = (page: Page) => page.locator(".sp-layer .sp-u");
const marks = (page: Page) => page.locator(".sp-layer .sp-g");
const mark = (page: Page, word: string) => page.locator(`.sp-layer .sp-g[data-word="${word}"]`);
/** The words marked as grammar problems, in order of position (each once). */
const marked = (page: Page) => marks(page).evaluateAll((els) => [...new Set(els.map((e) => (e as HTMLElement).dataset.word))]);
const misspelled = (page: Page) => spelling(page).evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.word));
const engineText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());

async function centreOf(u: Locator) {
  const b = (await u.first().boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}
async function rightClick(page: Page, u: Locator) {
  const { x, y } = await centreOf(u);
  await page.mouse.click(x, y, { button: "right" });
  await expect(contextMenu(page)).toBeVisible();
}

test.describe("grammar underlines", () => {
  test("a repeated word gets a blue mark, and the right-click menu says what is wrong and fixes it in one undo step", async ({ page }) => {
    await openDoc(page, ["This is is a test."]);
    await expect(mark(page, "is is")).toHaveCount(1);
    expect(await marked(page)).toEqual(["is is"]);
    expect(await misspelled(page)).toEqual([]);

    await rightClick(page, mark(page, "is is"));
    // The message first (it is a note, not a choice), then the replacement in bold, then Ignore.
    await expect(contextMenu(page).locator(".menu-note")).toHaveText("Repeated word: “is”");
    await expect(contextMenu(page).locator(".menu-label.strong").first()).toHaveText("is");
    await expect(contextItem(page, "Ignore")).toBeVisible();
    // Spelling's lines are not here.
    await expect(contextItem(page, "Add to dictionary")).toHaveCount(0);
    await contextItem(page, "is").click();
    await expect(contextMenu(page)).toHaveCount(0);

    await expect.poll(() => engineText(page)).toContain("This is a test.");
    await expect(marks(page)).toHaveCount(0);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => engineText(page)).toContain("This is is a test.");
    await expect(mark(page, "is is")).toHaveCount(1);
  });

  test("the mark is drawn in the grammar colour, a style hint is dotted, and the layer takes no clicks", async ({ page }) => {
    await openDoc(page, ["Two  spaces and the the cat."]);
    await expect(marks(page)).toHaveCount(2);
    const grammar = mark(page, "the the");
    // (The marks are drawn again when the page settles, so look until the element read is a live one.)
    await expect.poll(() => grammar.first().evaluate((el) => getComputedStyle(el, "::after").backgroundColor)).toBe("rgb(25, 103, 210)");
    await expect(grammar.first()).toHaveAttribute("data-kind", "grammar");
    const hint = page.locator('.sp-layer .sp-g[data-kind="style"]');
    await expect(hint).toHaveCount(1);
    // Dotted, not wavy: the wave's mask is gone and the dots are a gradient.
    await expect.poll(() => hint.evaluate((el) => getComputedStyle(el, "::after").backgroundImage)).toContain("repeating-linear-gradient");
    expect(await page.locator(".sp-layer").evaluate((el) => getComputedStyle(el).pointerEvents)).toBe("none");
    // The text itself is untouched.
    expect(await flowParas(page).first().evaluate((p) => p.querySelector(".sp-g") === null && p.textContent)).toBe("Two  spaces and the the cat.");
  });

  test("a misspelled word keeps its red mark and the grammar mark on the same words steps aside", async ({ page }) => {
    await openDoc(page, ["I have alot of work."]);
    await expect(spelling(page).first()).toBeVisible();
    expect(await misspelled(page)).toEqual(["alot"]);
    await page.waitForTimeout(400);
    await expect(marks(page)).toHaveCount(0);
    // The spelling menu is the one that opens.
    await rightClick(page, spelling(page).first());
    await expect(contextItem(page, "Add to dictionary")).toBeVisible();
    await page.keyboard.press("Escape");
  });

  test("Ignore leaves the mark off in this document, and the text alone", async ({ page }) => {
    await openDoc(page, ["First the the one.", "Second the the one."]);
    await expect(marks(page)).toHaveCount(2);
    await rightClick(page, marks(page).first());
    await contextItem(page, "Ignore").click();
    // The same rule on the same words is ignored everywhere in the document.
    await expect(marks(page)).toHaveCount(0);
    expect(await engineText(page)).toContain("First the the one.");
    // A different problem is still marked.
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "An apple, a orange. " });
    });
    await expect(mark(page, "a")).toHaveCount(1);
  });

  test("the right-click menu opened from the keyboard offers the problem at the caret", async ({ page }) => {
    await openDoc(page, ["Say it again again please."]);
    await expect(mark(page, "again again")).toHaveCount(1);
    const { x, y } = await centreOf(mark(page, "again again"));
    await page.mouse.click(x, y);
    await page.keyboard.press("Shift+F10");
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextMenu(page).locator(".menu-note")).toHaveText("Repeated word: “again”");
  });

  test("the word being typed is left alone until the caret moves off it", async ({ page }) => {
    await openDoc(page, ["Hello the"]);
    const para = flowParas(page).first();
    await para.scrollIntoViewIfNeeded();
    const box = (await para.boundingBox())!;
    await para.click({ position: { x: box.width - 2, y: Math.min(8, box.height / 2) } });
    await page.keyboard.type(" the");
    await page.waitForTimeout(500);
    // The caret is still in the second "the".
    await expect(marks(page)).toHaveCount(0);
    await page.keyboard.type(" end");
    await expect(mark(page, "the the")).toHaveCount(1);
  });

  test("grammar marks are not printed", async ({ page }) => {
    await openDoc(page, ["Say the the word."]);
    await expect(marks(page)).toHaveCount(1);
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .print-page")).toHaveCount(1);
    expect(await page.locator(".docxtor-print .sp-layer, .docxtor-print .sp-g").count()).toBe(0);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });
});

test.describe("rules by language", () => {
  test("English: articles, capitals, spaces, brackets, quotes and words that are mixed up", async ({ page }) => {
    await openDoc(page, [
      "She bought a apple. it was red",
      "Hello , world.It ended (almost",
      "He said “hello and you could of left alot earlier.",
    ]);
    await expect.poll(() => marked(page)).toEqual(expect.arrayContaining(["a", "it", " ,", "d.I", "("]));
    const all = await marked(page);
    expect(all).toContain("“");
    expect(all).toContain("could of");
  });

  test("French keeps its space before ? and !, German and English do not", async ({ page }) => {
    await openDoc(page, ["Vraiment ? Oui ! Voici : un exemple ; un autre."], { lang: "fr-FR" });
    await page.waitForTimeout(600);
    await expect(marks(page)).toHaveCount(0);
    await openDoc(page, ["Vraiment ? Oui !"], { lang: "de-DE" });
    await expect.poll(() => marked(page)).toEqual([" ?", " !"]);
  });

  test("Spanish asks for the inverted question mark", async ({ page }) => {
    await openDoc(page, ["Cómo estás? Muy bien."], { lang: "es-ES" });
    await expect(mark(page, "?")).toHaveCount(1);
    await rightClick(page, mark(page, "?"));
    await expect(contextMenu(page).locator(".menu-note")).toHaveText("Add the opening question mark ¿");
  });

  test("a language with no rules gets no grammar marks", async ({ page }) => {
    await openDoc(page, ["the the cat  sat"], { lang: "ja-JP" });
    await page.waitForTimeout(600);
    await expect(marks(page)).toHaveCount(0);
    // With no checker for the language the check command says so.
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");
    await expect(page.locator(".toast")).toHaveText("Spelling isn't available for this language");
  });
});

test.describe("Tools, Spelling and grammar", () => {
  test("Show grammar suggestions hides the marks and is remembered", async ({ page }) => {
    await openDoc(page, ["Say the the word."]);
    await expect(marks(page)).toHaveCount(1);
    await openMenu(page, "Tools");
    await menuItem(page, "Spelling and grammar").click();
    await expect(menuItem(page, "Show grammar suggestions")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Show grammar suggestions").click();
    await expect(marks(page)).toHaveCount(0);
    // Spelling is a separate switch: its marks stay.
    await expect(spelling(page)).toHaveCount(0);

    await page.goto("/?open=simple.docx");
    await expect(flowParas(page).first()).toBeVisible();
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "the the " });
    });
    await page.waitForTimeout(700);
    await expect(marks(page)).toHaveCount(0);
    await chooseMenu(page, "Tools", "Spelling and grammar", "Show grammar suggestions");
    await expect(mark(page, "the the")).toHaveCount(1);
  });

  test("the check panel steps through spelling and grammar together, in reading order", async ({ page }) => {
    await openDoc(page, ["First line is fine.", "The the documnet has an erorr here.", "It ended.Then it began."]);
    await expect(marks(page)).toHaveCount(2);
    await expect(spelling(page)).toHaveCount(2);
    await flowParas(page).first().click({ position: { x: 4, y: 8 } });
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");

    const panel = page.getByRole("dialog", { name: "Spelling and grammar check" });
    await expect(panel).toBeVisible();
    await expect(panel.locator(".sp-count")).toHaveText("1 of 4");
    // First the repeated words, with what is wrong, then the two misspellings, then the missing space.
    await expect(panel.locator(".sp-label")).toHaveText("Grammar");
    await expect(panel.locator(".sp-word")).toHaveText("The the");
    await expect(panel.locator(".sp-message")).toHaveText("Repeated word: “The”");
    await expect(panel.getByRole("option").first()).toHaveText("The");
    // Add to dictionary makes no sense for a grammar problem.
    await expect(panel.getByRole("button", { name: "Add to dictionary" })).toBeHidden();
    await expect.poll(() => page.evaluate(() => window.getSelection()!.toString())).toBe("The the");

    await panel.getByRole("button", { name: "Change", exact: true }).click();
    await expect.poll(() => engineText(page)).toContain("The documnet has an erorr here.");
    await expect(panel.locator(".sp-label")).toHaveText("Not in dictionary");
    await expect(panel.locator(".sp-message")).toBeHidden();
    await expect(panel.locator(".sp-word")).toHaveText("documnet");
    await expect(panel.locator(".sp-count")).toHaveText("1 of 3");

    await panel.getByRole("button", { name: "Next" }).click();
    await expect(panel.locator(".sp-word")).toHaveText("erorr");
    await panel.getByRole("button", { name: "Next" }).click();
    await expect(panel.locator(".sp-label")).toHaveText("Grammar");
    await expect(panel.locator(".sp-word")).toHaveText("d.T");
    await expect(panel.locator(".sp-message")).toHaveText("Add a space after “.”");
    await expect(panel.locator(".sp-count")).toHaveText("3 of 3");
    // Ignore moves on and, at the end, the panel closes.
    await panel.getByRole("button", { name: "Ignore", exact: true }).click();
    await expect(panel).toHaveCount(0);
    await expect(marks(page)).toHaveCount(0);
    expect(await engineText(page)).toContain("It ended.Then it began.");
  });

  test("the panel works with grammar alone", async ({ page }) => {
    await openDoc(page, ["Say the the word."]);
    await expect(marks(page)).toHaveCount(1);
    await chooseMenu(page, "Tools", "Spelling and grammar", "Show spelling suggestions");
    await expect(spelling(page)).toHaveCount(0);
    await flowParas(page).first().click({ position: { x: 4, y: 8 } });
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");
    const panel = page.getByRole("dialog", { name: "Spelling and grammar check" });
    await expect(panel.locator(".sp-word")).toHaveText("the the");
    await panel.getByRole("button", { name: "Change", exact: true }).click();
    await expect(panel).toHaveCount(0);
    await expect.poll(() => engineText(page)).toContain("Say the word.");
  });

  test("the commands sit in the Spelling and grammar submenu", async ({ page }) => {
    await openDoc(page, ["Say the word."]);
    await openMenu(page, "Tools");
    await menuItem(page, "Spelling and grammar").click();
    const labels = await page.locator('.menu-drop [role="menuitem"], .menu-drop [role="menuitemcheckbox"]').locator(".menu-label").allTextContents();
    const i = labels.indexOf("Show grammar suggestions");
    expect(i).toBeGreaterThan(labels.indexOf("Show spelling suggestions"));
    expect(labels).toContain("Custom dictionaries…");
    expect(labels).toContain("Readability statistics…");
    // LanguageTool is off by default, so its settings are not offered.
    expect(labels).not.toContain("LanguageTool settings…");
  });
});

test.describe("a language for each paragraph", () => {
  test("a paragraph set to Spanish is checked with the Spanish dictionary, the rest in English", async ({ page }) => {
    await openDoc(page, ["I will recieve the parcel tomorrow.", "Ayer fuimos al mercado y compramos recieve.", "Another recieve here."]);
    // This is about setting a paragraph's language by hand: without Detect language automatically, the Spanish
    // paragraph is in the document's English and its Spanish words are flagged.
    await chooseMenu(page, "Tools", "Spelling and grammar", "Proofing options…");
    await dialog(page, "Proofing options").getByRole("checkbox", { name: "Detect language automatically" }).uncheck();
    await dialog(page, "Proofing options").getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => misspelled(page)).toContain("fuimos");
    const ids = await flowParas(page).evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));

    await selectWord(page, ids[1], "mercado");
    await chooseMenu(page, "File", "Language…");
    const lang = dialog(page, "Language");
    await lang.getByRole("combobox", { name: "Search languages" }).fill("spanish");
    await lang.getByRole("option", { name: "Spanish (Spain)" }).click();
    await lang.getByLabel("Apply to the selected text only").check();
    await lang.getByRole("button", { name: "Apply" }).click();

    // The paragraph knows its language; the others keep the document's.
    await expect(flowParas(page).nth(1)).toHaveAttribute("data-lang", "es-ES");
    await expect(flowParas(page).nth(0)).toHaveAttribute("data-lang", "en-US");
    // Only the English misspellings are left: one in each English paragraph, and the Spanish paragraph's "recieve".
    await expect.poll(() => misspelled(page)).toEqual(["recieve", "recieve", "recieve"]);
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("en-US");
    // The suggestions come from the paragraph's own dictionary.
    const spanishWord = page.locator('.sp-layer .sp-u[data-p="' + ids[1] + '"]');
    await rightClick(page, spanishWord);
    expect(await contextMenu(page).locator(".menu-label").first().textContent()).not.toBe("receive");
    await page.keyboard.press("Escape");
    // And an English word in the Spanish paragraph is a mistake there.
    await inPage(page, (w, p) => {
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "parcel " });
    }, ids[1]);
    await expect.poll(() => misspelled(page)).toContain("parcel");
  });

  test("a paragraph in a language with no dictionary is left alone, and has no grammar rules either", async ({ page }) => {
    await openDoc(page, ["I will recieve the the parcel.", "Hello wrold, recieve this the the."]);
    await expect.poll(() => marks(page).count()).toBe(2);
    const ids = await flowParas(page).evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
    await inPage(page, (w, id) => {
      w.fe.editor.exec({ op: "set_language", lang: "ja-JP", ids: [id] });
      (w.fe.editor as unknown as { reloadPageSetup(): void }).reloadPageSetup();
    }, ids[1]);
    await expect(flowParas(page).nth(1)).toHaveAttribute("data-lang", "ja-JP");
    await expect.poll(() => misspelled(page)).toEqual(["recieve"]);
    await expect.poll(() => marks(page).count()).toBe(1);
    expect(await marks(page).first().getAttribute("data-p")).toBe(String(ids[0]));
  });

  test("the paragraph language follows the file: a run's own language, then the document's", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            "Plain English recieve.",
            { text: "", xml: '<w:r><w:rPr><w:lang w:val="fr-FR"/></w:rPr><w:t>Bonjour le monde et recieve.</w:t></w:r>' },
          ],
        },
      ],
      lang: "en-GB",
    });
    await expect(flowParas(page).nth(0)).toHaveAttribute("data-lang", "en-GB");
    await expect(flowParas(page).nth(1)).toHaveAttribute("data-lang", "fr-FR");
    await expect.poll(() => misspelled(page)).toEqual(["recieve", "recieve"]);
    // Changing the document's language reaches the paragraphs that have none of their own.
    await inPage(page, (w) => {
      w.fe.editor.exec({ op: "set_language", lang: "de-DE" });
      (w.fe.editor as unknown as { reloadPageSetup(): void }).reloadPageSetup();
    });
    await expect(flowParas(page).nth(0)).toHaveAttribute("data-lang", "de-DE");
    await expect(flowParas(page).nth(1)).toHaveAttribute("data-lang", "fr-FR");
    // "Plain English recieve." is all wrong in German now; the French paragraph is as it was.
    await expect.poll(() => spelling(page).count()).toBeGreaterThan(2);
  });
});

test.describe("checkers and dictionaries are contributions", () => {
  test("an extension adds a grammar checker; its issues join Docxtor's, and turning it off takes them away", async ({ page }) => {
    await openDoc(page, ["Say the the word and eat a banana."]);
    await expect(marks(page)).toHaveCount(1);
    await inPage(page, async (w) => {
      const fe = w.fe as unknown as { use(ext: unknown): Promise<void> };
      await fe.use({
        id: "test.banana",
        name: "Banana checker",
        activate(ctx: { grammar: { register(c: unknown): void } }) {
          ctx.grammar.register({
            id: "test.banana",
            label: "Banana",
            languages: ["en"],
            async check({ paragraphs }: { paragraphs: { id: number; text: string }[] }) {
              return paragraphs.flatMap((p) => {
                const at = p.text.indexOf("banana");
                return at < 0 ? [] : [{ paragraph: p.id, from: at, to: at + 6, message: "Fruit is not allowed here", replacements: ["pear"], rule: { id: "BANANA" }, kind: "style" }];
              });
            },
          });
        },
      });
    });
    await expect(mark(page, "banana")).toHaveCount(1);
    expect(await marked(page)).toEqual(["the the", "banana"]);
    await rightClick(page, mark(page, "banana"));
    await expect(contextMenu(page).locator(".menu-note")).toHaveText("Fruit is not allowed here");
    await contextItem(page, "pear").click();
    await expect.poll(() => engineText(page)).toContain("eat a pear.");
    await expect(mark(page, "banana")).toHaveCount(0);

    // Turned off in the Extensions dialog, its checker goes and so would its underlines.
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "insert_text", at: { p: id, o: 0 }, text: "banana " }), Number(await flowParas(page).first().getAttribute("data-p")));
    await expect(mark(page, "banana")).toHaveCount(1);
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("test.banana"));
    await expect(mark(page, "banana")).toHaveCount(0);
    // Docxtor's own marks stay.
    await expect(mark(page, "the the")).toHaveCount(1);
  });

  test("turning Docxtor's grammar rules off in the Extensions dialog clears its underlines, and on brings them back", async ({ page }) => {
    await openDoc(page, ["Say the the word."]);
    await expect(marks(page)).toHaveCount(1);
    await chooseMenu(page, "Extensions", "Manage extensions…");
    const ext = dialog(page, "Extensions");
    const sw = ext.locator('.switch[data-ext="docxtor.grammar.rules"]');
    await expect(sw).toHaveAttribute("aria-checked", "true");
    await sw.click();
    await expect(sw).toHaveAttribute("aria-checked", "false");
    await expect(marks(page)).toHaveCount(0);
    await sw.click();
    await expect(sw).toHaveAttribute("aria-checked", "true");
    await expect(marks(page)).toHaveCount(1);
    // The grammar underlines as a whole are their own extension.
    await ext.locator('.switch[data-ext="docxtor.grammar"]').click();
    await expect(marks(page)).toHaveCount(0);
  });

  test("an extension adds a spelling dictionary for a language Docxtor has none for, and the latest registered wins", async ({ page }) => {
    await openDoc(page, ["ciao mondo foo"], { lang: "it-IT" });
    await page.waitForTimeout(500);
    await expect(spelling(page)).toHaveCount(0);
    await inPage(page, async (w) => {
      const fe = w.fe as unknown as { use(ext: unknown): Promise<void> };
      await fe.use({
        id: "test.italian",
        name: "Italian",
        activate(ctx: { spelling: { dictionaries: { register(d: unknown): void } } }) {
          ctx.spelling.dictionaries.register({ id: "test.italian", label: "Italian", languages: ["it"], load: async () => ({ aff: "SET UTF-8\n", dic: "2\nciao\nmondo\n" }) });
        },
      });
    });
    await expect.poll(() => misspelled(page)).toEqual(["foo"]);
    // A later provider for the same language replaces it.
    await inPage(page, async (w) => {
      const fe = w.fe as unknown as { use(ext: unknown): Promise<void> };
      await fe.use({
        id: "test.italian2",
        name: "Italian 2",
        activate(ctx: { spelling: { dictionaries: { register(d: unknown): void } } }) {
          ctx.spelling.dictionaries.register({ id: "test.italian2", label: "Italian 2", languages: ["it"], load: async () => ({ aff: "SET UTF-8\n", dic: "2\nfoo\nmondo\n" }) });
        },
      });
    });
    await expect.poll(() => misspelled(page)).toEqual(["ciao"]);
    // Turn it off and the first is served again.
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("test.italian2"));
    await expect.poll(() => misspelled(page)).toEqual(["foo"]);
  });

  test("the app's dictionaries are an extension that can be turned off", async ({ page }) => {
    await openDoc(page, ["I will recieve it."]);
    await expect(spelling(page)).toHaveCount(1);
    await chooseMenu(page, "Extensions", "Manage extensions…");
    const ext = dialog(page, "Extensions");
    await expect(ext.getByText("Spelling dictionaries: English, Spanish, French, German")).toBeVisible();
    const sw = ext.locator('.switch[data-ext="docxtor.dictionaries"]');
    await sw.click();
    await expect(spelling(page)).toHaveCount(0);
    // With no dictionaries the browser's own spellcheck is back.
    await expect(page.locator(".flow")).toHaveAttribute("spellcheck", "true");
    await sw.click();
    await expect(spelling(page)).toHaveCount(1);
  });

  test("the editor's spelling option still registers one provider", async ({ page }) => {
    await openDoc(page, ["Say hello."]);
    const result = await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { constructor: new (o: object) => any } } }).docxtor;
      const host = document.createElement("div");
      host.style.cssText = "position:fixed;left:0;top:0;width:600px;height:400px;z-index:5";
      document.body.append(host);
      const ed = new w.fe.constructor({ menuBar: true, spelling: { load: async () => ({ aff: "SET UTF-8\n", dic: "3\nhello\nworld\nthe\n" }) } });
      ed.mount(host);
      await ed.ready;
      await ed.create("plain.docx");
      const p = Number(host.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      ed.exec({ op: "insert_text", at: { p, o: 0 }, text: "hello wrold the the " });
      await new Promise((r) => setTimeout(r, 900));
      const out = {
        spelling: Array.from(host.querySelectorAll<HTMLElement>(".sp-u")).map((e) => e.dataset.word),
        grammar: Array.from(host.querySelectorAll<HTMLElement>(".sp-g")).map((e) => e.dataset.word),
        providers: ed.extensions.spellingDictionaries.list().map((d: { id: string }) => d.id),
        checkers: ed.extensions.grammarCheckers.list().map((c: { id: string }) => c.id),
      };
      ed.destroy();
      host.remove();
      return out;
    });
    expect(result.spelling).toEqual(["wrold"]);
    expect(result.grammar).toEqual(["the the"]);
    expect(result.providers).toEqual(["docxtor.spelling.option"]);
    expect(result.checkers).toEqual(["docxtor.grammar.rules"]);
  });
});
