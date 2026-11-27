// Tools > Thesaurus: the panel, replacing a word with a synonym, endings and capitals, the right-click line, the
// languages, and the extension point (a source an extension adds shows up beside Docxtor's own list).
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, expect, inPage, selectWord, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const doc = (paragraphs: string[], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs }], ...extra });
const panel = (page: Page) => page.locator(".th-panel");
const words = (page: Page) => panel(page).locator(".th-word");
const wordNamed = (page: Page, text: string) => panel(page).locator(".th-word", { hasText: new RegExp(`^${text}$`) });
const paras = (page: Page) => page.locator(".flow .fp");
const firstId = async (page: Page) => Number(await paras(page).first().getAttribute("data-p"));
const text = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().trim());

async function select(page: Page, word: string, index = 0) {
  const id = Number(await paras(page).nth(index).getAttribute("data-p"));
  await selectWord(page, id, word);
}

test.describe("thesaurus", () => {
  test("Tools, Thesaurus looks up the selected word and lists its synonyms by meaning", async ({ page }) => {
    await openBuilt(page, doc(["The big dog barked."]));
    await select(page, "big");
    await chooseMenu(page, "Tools", "Thesaurus");
    await expect(panel(page)).toBeVisible();
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “big”");
    await expect(panel(page).getByRole("searchbox").or(panel(page).getByRole("textbox", { name: "Word to look up" }))).toHaveValue("big");
    await expect(wordNamed(page, "large")).toBeVisible();
    await expect(wordNamed(page, "huge").first()).toBeVisible();
    // The meaning names its part of speech and the closest word.
    await expect(panel(page).locator(".th-caption").first()).toHaveText(/^Adjective, like “/);
    // Opposites come with the meaning.
    await expect(panel(page).locator(".th-opposites").first()).toHaveText("Opposites");
    await expect(wordNamed(page, "small").first()).toBeVisible();
    // The synonyms are options in a list, and nothing is chosen yet.
    await expect(panel(page).getByRole("listbox", { name: "Synonyms" })).toBeVisible();
    await expect(panel(page).getByRole("button", { name: "Replace" })).toBeDisabled();
  });

  test("Shift+F7 opens it on the word at the caret, and Escape closes it", async ({ page }) => {
    await openBuilt(page, doc(["Pick a happy ending."]));
    await select(page, "happy");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page)).toBeVisible();
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “happy”");
    await page.keyboard.press("Escape");
    await expect(panel(page)).toBeHidden();
  });

  test("a synonym replaces the word as one undo step, with the capitals of the word", async ({ page }) => {
    await openBuilt(page, doc(["Big trouble comes in big packages."]));
    await select(page, "Big");
    await page.keyboard.press("Shift+F7");
    await wordNamed(page, "large").first().click();
    await expect(wordNamed(page, "large").first()).toHaveAttribute("aria-selected", "true");
    await panel(page).getByRole("button", { name: "Replace" }).click();
    await expect.poll(() => text(page)).toBe("Large trouble comes in big packages.");
    await expect(panel(page).getByRole("status")).toHaveText("Replaced with “Large”.");
    // The new word is selected, so a second pick replaces that one.
    await wordNamed(page, "huge").first().dblclick();
    await expect.poll(() => text(page)).toBe("Huge trouble comes in big packages.");
    // Each replacement is one undo step.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => text(page)).toBe("Large trouble comes in big packages.");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => text(page)).toBe("Big trouble comes in big packages.");
  });

  test("an inflected word is looked up through its base word and the synonyms take the ending", async ({ page }) => {
    await openBuilt(page, doc(["She was stopping the car and running home."]));
    await select(page, "stopping");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “stopping”");
    await expect(panel(page).locator(".th-caption").first()).toContainText("“stop”");
    await expect(wordNamed(page, "halting").first()).toBeVisible();
    await expect(wordNamed(page, "ceasing").first()).toBeVisible();
    await wordNamed(page, "halting").first().click();
    await panel(page).getByRole("button", { name: "Replace" }).click();
    await expect.poll(() => text(page)).toBe("She was halting the car and running home.");
  });

  test("typing another word looks it up, Back goes to the one before, and a word with no entry says so", async ({ page }) => {
    await openBuilt(page, doc(["A quick reply."]));
    await select(page, "quick");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “quick”");
    const input = panel(page).getByRole("textbox", { name: "Word to look up" });
    await input.fill("slow");
    await input.press("Enter");
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “slow”");
    await expect(wordNamed(page, "sluggish").first()).toBeVisible();
    await panel(page).getByRole("button", { name: "Back" }).click();
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “quick”");
    await input.fill("qwzxv");
    await input.press("Enter");
    await expect(panel(page).getByRole("status")).toHaveText("No synonyms found for “qwzxv”.");
    await expect(words(page)).toHaveCount(0);
  });

  test("a synonym can be looked up in turn with Look up", async ({ page }) => {
    await openBuilt(page, doc(["A small cat."]));
    await select(page, "small");
    await page.keyboard.press("Shift+F7");
    await wordNamed(page, "tiny").first().click();
    await panel(page).getByRole("button", { name: "Look up" }).last().click();
    await expect(panel(page).getByRole("status")).toHaveText("Meanings of “tiny”");
    await expect(wordNamed(page, "minuscule").first()).toBeVisible();
  });

  test("right-click on a word offers Synonyms…", async ({ page }) => {
    await openBuilt(page, doc(["A beautiful morning."]));
    await select(page, "beautiful");
    const id = await firstId(page);
    const box = (await page.locator(`.flow [data-p="${id}"]`).boundingBox())!;
    await page.mouse.click(box.x + 60, box.y + 8, { button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    await contextItem(page, "Synonyms…").click();
    await expect(panel(page)).toBeVisible();
    await expect(panel(page).getByRole("status")).toContainText("Meanings of");
  });

  test("a language with no thesaurus says so", async ({ page }) => {
    await openBuilt(page, doc(["La casa es grande."], { lang: "es-ES" }));
    await select(page, "casa");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page).getByRole("status")).toContainText("No thesaurus is on for Spanish");
    await expect(words(page)).toHaveCount(0);
    // The right-click menu doesn't offer it either.
    const id = await firstId(page);
    const box = (await page.locator(`.flow [data-p="${id}"]`).boundingBox())!;
    await page.keyboard.press("Escape");
    await page.mouse.click(box.x + 40, box.y + 8, { button: "right" });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Synonyms…")).toHaveCount(0);
  });

  test("the English thesaurus is an extension: turned off, nothing is found; the list loads only when asked", async ({ page }) => {
    const chunks: string[] = [];
    page.on("request", (r) => {
      if (/english-data/.test(r.url())) chunks.push(r.url());
    });
    await openBuilt(page, doc(["A big dog."]));
    // Opening a document doesn't load the list.
    expect(chunks).toEqual([]);
    await select(page, "big");
    await page.keyboard.press("Shift+F7");
    await expect(wordNamed(page, "large").first()).toBeVisible();
    // The first look-up fetched it.
    expect(chunks.length).toBeGreaterThan(0);
    await page.keyboard.press("Escape");
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("docxtor.thesaurus.english"));
    await select(page, "big");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page).getByRole("status")).toContainText("No thesaurus is on for English");
  });

  test("another extension's thesaurus is listed next to Docxtor's, by source", async ({ page }) => {
    await openBuilt(page, doc(["A big dog."]));
    await inPage(page, async (w) => {
      const fe = w.fe as unknown as { use(ext: unknown): Promise<void> };
      await fe.use({
        id: "test.thesaurus",
        name: "Test thesaurus",
        activate(ctx: { thesaurus: { register(t: unknown): void } }) {
          ctx.thesaurus.register({
            id: "test.thesaurus",
            label: "Test thesaurus",
            languages: ["en"],
            lookup: (word: string) => (word === "big" ? { senses: [{ pos: "adjective", meaning: "important", synonyms: ["momentous", "large"] }] } : null),
          });
        },
      });
    });
    await select(page, "big");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page).locator(".th-source")).toHaveText(["Docxtor English thesaurus", "Test thesaurus"]);
    await expect(wordNamed(page, "momentous")).toBeVisible();
    await wordNamed(page, "momentous").click();
    await panel(page).getByRole("button", { name: "Replace" }).click();
    await expect.poll(() => text(page)).toBe("A momentous dog.");
  });

  test("the Datamuse extension is off until it is turned on, and then adds its words", async ({ page }) => {
    const asked: string[] = [];
    await page.route("https://api.datamuse.com/words**", async (route) => {
      const url = new URL(route.request().url());
      asked.push(url.search);
      const syn = url.searchParams.get("rel_syn");
      const body = syn ? [{ word: "mammoth", tags: ["syn", "adj"] }, { word: "bulk", tags: ["syn", "n"] }, { word: "colossal", tags: ["syn", "adj"] }] : [{ word: "tiny", tags: ["ant", "adj"] }];
      await route.fulfill({ contentType: "application/json", body: JSON.stringify(body) });
    });
    await openBuilt(page, doc(["A big dog."]));
    await select(page, "big");
    await page.keyboard.press("Shift+F7");
    await expect(wordNamed(page, "large").first()).toBeVisible();
    // Off by default: no word left the page.
    expect(asked).toEqual([]);
    await expect(panel(page).locator(".th-source")).toHaveCount(0);
    await page.keyboard.press("Escape");

    await inPage(page, (w) => (w.fe as unknown as { extensions: { enable(id: string): Promise<void> } }).extensions.enable("docxtor.thesaurus.datamuse"));
    await select(page, "big");
    await page.keyboard.press("Shift+F7");
    await expect(panel(page).locator(".th-source")).toHaveText(["Docxtor English thesaurus", "Datamuse"]);
    await expect(wordNamed(page, "mammoth")).toBeVisible();
    await expect(wordNamed(page, "bulk")).toBeVisible();
    expect(asked.length).toBe(2);
    expect(asked.every((a) => a.includes("big"))).toBe(true);
    // Datamuse gives one list for each part of speech.
    await expect(panel(page).locator(".th-sense").filter({ has: page.locator(".th-word", { hasText: /^bulk$/ }) }).locator(".th-caption").first()).toHaveText("Noun");
  });

  test("the panel fits the page in the dark theme and the words can be reached with the keyboard", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await openBuilt(page, doc(["A big dog."]));
    await select(page, "big");
    await page.keyboard.press("Shift+F7");
    const first = wordNamed(page, "large").first();
    await first.focus();
    await page.keyboard.press("ArrowDown");
    await expect(words(page).nth(1)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect.poll(() => text(page)).not.toBe("A big dog.");
    const bg = await panel(page).evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(bg).not.toBe("rgba(0, 0, 0, 0)");
  });
});
