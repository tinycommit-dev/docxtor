// Docxtor's own spelling checker: wavy underlines on a layer over the pages, corrections in the right-click menu,
// Tools > Spelling and grammar (the check panel, Show spelling suggestions, Custom dictionaries), the languages,
// and a host without dictionaries. The app supplies real en-US, en-GB, es, fr and de Hunspell dictionaries.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, dialog, expect, inPage, menuItem, openFixture, openMenu, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const doc = (paragraphs: string[], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs }], ...extra });
/** Open a built document and wait until its first paragraph has been checked. */
async function openDoc(page: Page, paragraphs: string[], extra: Partial<DocSpec> = {}) {
  await openBuilt(page, doc(paragraphs, extra));
}

const layer = (page: Page) => page.locator(".sp-layer");
const underlines = (page: Page) => page.locator(".sp-layer .sp-u");
const flowParas = (page: Page) => page.locator(".flow .fp");
/** The words underlined now, in order of position. */
const underlined = (page: Page) => underlines(page).evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.word));
const underline = (page: Page, word: string) => page.locator(`.sp-layer .sp-u[data-word="${word}"]`);
/** The text of the document as the engine has it, paragraphs on lines. */
const engineText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());
const settle = (page: Page) => page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done(null)))));

/** Put the caret at the end of a paragraph. */
async function endOf(page: Page, para: Locator) {
  await para.scrollIntoViewIfNeeded();
  const box = (await para.boundingBox())!;
  await para.click({ position: { x: box.width - 2, y: Math.min(8, box.height / 2) } });
}

/** The middle of the underline's word, on screen. */
async function centreOf(u: Locator) {
  const b = (await u.first().boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}

async function rightClickWord(page: Page, word: string) {
  const { x, y } = await centreOf(underline(page, word));
  await page.mouse.click(x, y, { button: "right" });
  await expect(contextMenu(page)).toBeVisible();
}

test.describe("spelling underlines", () => {
  test("newer everyday words are known (chatbot, pushback, roadmaps, coworking), real mistakes still are not", async ({ page }) => {
    await openDoc(page, ["Ask the chatbot, expect pushback, share the roadmaps of our coworking space. Load it everytime."]);
    await expect(underline(page, "everytime")).toHaveCount(1);
    expect(await underlined(page)).toEqual(["everytime"]);
  });

  test("a misspelled word gets an underline at its position, correct words and skipped kinds do not", async ({ page }) => {
    await openDoc(page, [
      "I will recieve the parcel tomorrow.",
      "NASA and ICAO are fine, as are abc123 and x9y and e.g. this.",
      "Write to john@exmplz.com or visit https://exmplz.com/recieve now.",
      "The don't and the don’t are spelled right.",
    ]);
    await expect(underline(page, "recieve")).toHaveCount(1);
    expect(await underlined(page)).toEqual(["recieve"]);

    // The underline sits on the word: its box is where the word's text is.
    const word = await flowParas(page)
      .nth(0)
      .evaluate((p) => {
        const w = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
        for (let n = w.nextNode(); n; n = w.nextNode()) {
          const i = (n as Text).data.indexOf("recieve");
          if (i < 0) continue;
          const r = document.createRange();
          r.setStart(n, i);
          r.setEnd(n, i + 7);
          const b = r.getBoundingClientRect();
          return { left: b.left, right: b.right, top: b.top, bottom: b.bottom };
        }
        return null;
      });
    const u = (await underline(page, "recieve").boundingBox())!;
    expect(Math.abs(u.x - word!.left)).toBeLessThan(2);
    expect(Math.abs(u.x + u.width - word!.right)).toBeLessThan(2);
    expect(Math.abs(u.y - word!.top)).toBeLessThan(2);
    expect(Math.abs(u.y + u.height - word!.bottom)).toBeLessThan(2);

    // It is a layer of its own: the text is the same, and it never takes a click or a selection.
    expect(await layer(page).evaluate((el) => getComputedStyle(el).pointerEvents)).toBe("none");
    expect(await flowParas(page).nth(0).evaluate((p) => p.querySelector(".sp-u") === null && p.textContent)).toBe("I will recieve the parcel tomorrow.");
  });

  test("words in a code font and link text are skipped", async ({ page }) => {
    await openDoc(page, ["Plain recieve here.", "Code recieve here.", "Link recieve here."]);
    await expect(underlines(page)).toHaveCount(3);
    await inPage(page, (w) => {
      const e = w.fe.editor;
      const ids = Array.from(document.querySelectorAll<HTMLElement>(".flow .fp")).map((p) => Number(p.dataset.p));
      e.exec({ op: "format_text", from: { p: ids[1], o: 5 }, to: { p: ids[1], o: 12 }, format: { font: "Courier New" } });
      e.exec({ op: "set_link", from: { p: ids[2], o: 5 }, to: { p: ids[2], o: 12 }, url: "https://example.org" });
    });
    await expect(underlines(page)).toHaveCount(1);
    await expect(flowParas(page).nth(0).locator("..")).toBeVisible();
    expect(await underline(page, "recieve").evaluate((el) => (el as HTMLElement).dataset.p)).toBe(await flowParas(page).nth(0).getAttribute("data-p"));
  });

  test("typing a misspelling underlines it once the caret leaves the word", async ({ page }) => {
    await openDoc(page, ["Hello there"]);
    await endOf(page, flowParas(page).first());
    await page.keyboard.type(" recieve");
    await expect(flowParas(page).first()).toContainText("Hello there recieve");
    // The caret is still in the word: Docs and Word wait too.
    await settle(page);
    await page.waitForTimeout(500);
    await expect(underlines(page)).toHaveCount(0);

    await page.keyboard.type(" ");
    await expect(underline(page, "recieve")).toHaveCount(1);

    // Back into the word with the arrow keys, and typing on, hides it again until the caret leaves.
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.type("x");
    await settle(page);
    await page.waitForTimeout(400);
    await expect(underlines(page)).toHaveCount(0);
    // Two steps right leave the word (End would scroll the page on a Mac).
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(underline(page, "recievxe")).toHaveCount(1);
  });

  test("a checker that starts while a word is being typed leaves that word alone until the caret leaves it", async ({ page }) => {
    // The checker is set once its dictionary has loaded, which on a slow machine comes after the person has started typing.
    await openDoc(page, ["Hello there"]);
    await endOf(page, flowParas(page).first());
    await page.keyboard.type(" recieve");
    await expect(flowParas(page).first()).toContainText("Hello there recieve");
    await inPage(page, (w) => {
      const s = w.fe.editor.spell as unknown as { checker: unknown; setChecker(c: unknown): void };
      s.setChecker(s.checker);
    });
    await settle(page);
    await page.waitForTimeout(500);
    await expect(underlines(page)).toHaveCount(0);
    await page.keyboard.type(" ");
    await expect(underline(page, "recieve")).toHaveCount(1);

    // Typed with no checker at all yet: the word is still the one being typed when the checker arrives.
    const checker = await page.evaluateHandle(() => {
      const s = (window as unknown as { docxtor: { fe: { editor: { spell: { checker: unknown; setChecker(c: unknown): void } } } } }).docxtor.fe.editor.spell;
      const c = s.checker;
      s.setChecker(null);
      return c;
    });
    await page.keyboard.type("anothr");
    await page.evaluate((c) => (window as unknown as { docxtor: { fe: { editor: { spell: { setChecker(c: unknown): void } } } } }).docxtor.fe.editor.spell.setChecker(c), checker);
    await settle(page);
    await page.waitForTimeout(500);
    await expect(underline(page, "anothr")).toHaveCount(0);
    await expect(underline(page, "recieve")).toHaveCount(1);
    await page.keyboard.type(" ");
    await expect(underline(page, "anothr")).toHaveCount(1);
  });

  test("an underline goes away when the word is fixed by hand", async ({ page }) => {
    await openDoc(page, ["Please recieve it."]);
    await expect(underline(page, "recieve")).toHaveCount(1);
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.type("Please receive it. And again teh.");
    await expect(underline(page, "teh")).toHaveCount(1);
    await expect(underline(page, "recieve")).toHaveCount(0);
  });

  test("underlines are not printed", async ({ page }) => {
    await openDoc(page, ["Please recieve it."]);
    await expect(underlines(page)).toHaveCount(1);
    await page.evaluate(() => {
      window.print = () => undefined;
    });
    await page.keyboard.press(`${MOD}+P`);
    await expect(page.locator(".docxtor-print .print-page")).toHaveCount(1);
    expect(await page.locator(".docxtor-print .sp-layer, .docxtor-print .sp-u").count()).toBe(0);
    await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  });

  test("a word in a table cell is underlined", async ({ page }) => {
    await openFixture(page);
    await inPage(page, (w) => {
      const e = w.fe.editor;
      const cell = Array.from(document.querySelectorAll<HTMLElement>(".flow table .fp[data-p]"))[0];
      e.exec({ op: "insert_text", at: { p: Number(cell.dataset.p), o: 0 }, text: "teh " });
    });
    await expect(underline(page, "teh")).toHaveCount(1);
    expect(await underline(page, "teh").evaluate((el) => !!document.querySelector(".flow table")!.contains(document.elementFromPoint(el.getBoundingClientRect().left + 3, el.getBoundingClientRect().top + 3)))).toBe(true);
  });

  test("a header and a footer that are on screen are underlined too, but their words can only be fixed by editing them", async ({ page }) => {
    await openBuilt(page, { sections: [{ headers: { default: "A recieve header" }, footers: { default: "The footer is fine" }, paragraphs: ["The body is fine."] }] });
    const inHeader = underline(page, "recieve");
    await expect(inHeader).toHaveCount(1);
    expect(await inHeader.evaluate((el) => (el as HTMLElement).dataset.p)).toBeUndefined();
    expect(
      await inHeader.evaluate((el) => {
        const u = el.getBoundingClientRect();
        const h = document.querySelector(".sheet .hf")!.getBoundingClientRect();
        return u.left >= h.left - 1 && u.right <= h.right + 1 && u.top >= h.top - 1 && u.bottom <= h.bottom + 1;
      }),
    ).toBe(true);
    // Right-clicking it offers to edit the header, not suggestions.
    const { x, y } = await centreOf(inHeader);
    await page.mouse.click(x, y, { button: "right" });
    await expect(contextItem(page, "Edit header")).toBeVisible();
    await expect(contextItem(page, "Ignore")).toHaveCount(0);
    await page.keyboard.press("Escape");

    // In the header being edited the word is an ordinary one: suggestions and Change work.
    await page.mouse.dblclick(x, y);
    await expect(page.locator(".story-edit")).toBeVisible();
    await expect(page.locator('.sp-layer .sp-u[data-word="recieve"][data-p]')).toHaveCount(1);
    const at = await centreOf(underline(page, "recieve"));
    await page.mouse.click(at.x, at.y, { button: "right" });
    await contextItem(page, "receive").click();
    await expect(page.locator(".story-edit")).toContainText("A receive header");
    await expect(underlines(page)).toHaveCount(0);
  });
});

test.describe("spelling in the right-click menu", () => {
  test("suggestions come first, in bold, then Ignore and Add to dictionary, then the usual items", async ({ page }) => {
    await openDoc(page, ["I will recieve the parcel."]);
    await expect(underline(page, "recieve")).toHaveCount(1);
    await rightClickWord(page, "recieve");
    const labels = await contextMenu(page).locator(".menu-label").allTextContents();
    expect(labels[0]).toBe("receive");
    expect(labels.indexOf("Ignore")).toBeGreaterThan(0);
    expect(labels.indexOf("Add to dictionary")).toBe(labels.indexOf("Ignore") + 1);
    expect(labels.indexOf("Cut")).toBeGreaterThan(labels.indexOf("Add to dictionary"));
    expect(labels.filter((l, i) => i < labels.indexOf("Ignore")).length).toBeLessThanOrEqual(5);
    await expect(contextMenu(page).locator(".menu-label.strong").first()).toHaveText("receive");
    // Only the suggestions are bold.
    await expect(contextItem(page, "Ignore").locator(".menu-label")).not.toHaveClass(/strong/);
    await expect(contextMenu(page).locator(".menu-hint")).toHaveText("Shift+right-click for your browser's menu");
  });

  test("a right-click away from a misspelled word has no spelling lines", async ({ page }) => {
    await openDoc(page, ["I will recieve the parcel."]);
    await expect(underline(page, "recieve")).toHaveCount(1);
    await flowParas(page).first().click({ button: "right", position: { x: 4, y: 8 } });
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextItem(page, "Ignore")).toHaveCount(0);
    await expect(contextItem(page, "Cut")).toBeVisible();
  });

  test("choosing a suggestion replaces the word in one undo step and keeps its formatting", async ({ page }) => {
    await openDoc(page, ["I will recieve the parcel."]);
    await expect(underline(page, "recieve")).toHaveCount(1);
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "format_text", from: { p, o: 7 }, to: { p, o: 14 }, format: { bold: true } });
    });
    await expect(underline(page, "recieve")).toHaveCount(1);
    await rightClickWord(page, "recieve");
    await contextItem(page, "receive").click();
    await expect(contextMenu(page)).toHaveCount(0);

    await expect.poll(() => engineText(page)).toContain("I will receive the parcel.");
    await expect(underlines(page)).toHaveCount(0);
    // The new word is bold like the old one.
    const bold = await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      const at = (o: number) => w.fe.editor.engine!.formatAt({ p, o })?.run.bold ?? false;
      return [at(8), at(12), at(3), at(18)];
    });
    expect(bold).toEqual([true, true, false, false]);

    // One Undo brings the misspelling back.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => engineText(page)).toContain("I will recieve the parcel.");
    await expect(underline(page, "recieve")).toHaveCount(1);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => engineText(page)).toContain("I will receive the parcel.");
  });

  test("Ignore removes the underline for the rest of the session in this document", async ({ page }) => {
    await openDoc(page, ["First recieve here.", "Second recieve there."]);
    await expect(underline(page, "recieve")).toHaveCount(2);
    await rightClickWord(page, "recieve");
    await contextItem(page, "Ignore").click();
    await expect(underlines(page)).toHaveCount(0);
    // The text is unchanged and nothing was added to the dictionary.
    expect(await engineText(page)).toContain("First recieve here.");
    await openMenu(page, "Tools");
    await menuItem(page, "Spelling and grammar").click();
    await menuItem(page, "Custom dictionaries…").click();
    await expect(dialog(page, "Custom dictionaries")).toBeVisible();
    await expect(dialog(page, "Custom dictionaries").getByText("No words yet", { exact: false })).toBeVisible();
  });

  test("Add to dictionary removes the underline, is remembered after a reload, and the Custom dictionaries dialog lists and removes it", async ({ page }) => {
    await openDoc(page, ["First recieve here."]);
    await rightClickWord(page, "recieve");
    await contextItem(page, "Add to dictionary").click();
    await expect(underlines(page)).toHaveCount(0);

    // Shared across documents and sessions.
    await page.goto("/?open=simple.docx");
    await expect(flowParas(page).first()).toBeVisible();
    await openFixture(page, "simple.docx");
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "recieve teh " });
    });
    await expect(underline(page, "teh")).toHaveCount(1);
    await expect(underline(page, "recieve")).toHaveCount(0);

    await chooseMenu(page, "Tools", "Spelling and grammar", "Custom dictionaries…");
    const d = dialog(page, "Custom dictionaries");
    await expect(d.locator(".sp-word-text")).toHaveText(["recieve"]);
    await d.getByRole("button", { name: "Remove recieve" }).click();
    await expect(d.getByText("No words yet", { exact: false })).toBeVisible();

    // Words can be added by hand too.
    await d.getByRole("textbox", { name: "Add a word" }).fill("teh");
    await d.getByRole("textbox", { name: "Add a word" }).press("Enter");
    await expect(d.locator(".sp-word-text")).toHaveText(["teh"]);
    await d.getByRole("button", { name: "Done" }).click();
    await expect(d).toHaveCount(0);
    await expect(underline(page, "teh")).toHaveCount(0);
    await expect(underline(page, "recieve")).toHaveCount(1);
  });

  test("the right-click menu opened from the keyboard offers the word at the caret", async ({ page }) => {
    await openDoc(page, ["I will recieve the parcel."]);
    await expect(underline(page, "recieve")).toHaveCount(1);
    const { x, y } = await centreOf(underline(page, "recieve"));
    await page.mouse.click(x, y);
    await page.keyboard.press("Shift+F10");
    await expect(contextMenu(page)).toBeVisible();
    await expect(contextMenu(page).locator(".menu-label").first()).toHaveText("receive");
  });
});

test.describe("Tools, Spelling and grammar", () => {
  test("Show spelling suggestions hides the underlines and turns the browser's own spellcheck back on, and is remembered", async ({ page }) => {
    await openDoc(page, ["I will recieve the parcel."]);
    await expect(underlines(page)).toHaveCount(1);
    // One set of underlines: the browser's is off while Docxtor's is on.
    await expect(page.locator(".flow")).toHaveAttribute("spellcheck", "false");

    await openMenu(page, "Tools");
    await menuItem(page, "Spelling and grammar").click();
    await expect(menuItem(page, "Show spelling suggestions")).toHaveAttribute("aria-checked", "true");
    await menuItem(page, "Show spelling suggestions").click();
    await expect(underlines(page)).toHaveCount(0);
    await expect(page.locator(".flow")).toHaveAttribute("spellcheck", "true");
    // The right-click menu has its old hint again, and no spelling lines.
    await rightClickAtParagraph(page);
    await expect(contextItem(page, "Ignore")).toHaveCount(0);
    await expect(contextMenu(page).locator(".menu-hint")).toHaveText("Shift+right-click for your browser's menu and spelling suggestions");
    await page.keyboard.press("Escape");

    await openFixture(page, "simple.docx");
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "recieve " });
    });
    await page.waitForTimeout(500);
    await expect(underlines(page)).toHaveCount(0);
    await expect(page.locator(".flow")).toHaveAttribute("spellcheck", "true");

    await chooseMenu(page, "Tools", "Spelling and grammar", "Show spelling suggestions");
    await expect(underline(page, "recieve")).toHaveCount(1);
    await expect(page.locator(".flow")).toHaveAttribute("spellcheck", "false");
  });

  test("the check panel steps through the errors from the caret, and Change fixes one", async ({ page }) => {
    await openDoc(page, ["First line is fine.", "The documnet has an erorr here.", "And a third recieve line."]);
    await expect(underlines(page)).toHaveCount(3);
    await flowParas(page).first().click({ position: { x: 4, y: 8 } });
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");

    const panel = page.getByRole("dialog", { name: "Spelling and grammar check" });
    await expect(panel).toBeVisible();
    // The selection bubble would sit over the text the panel is about.
    await expect(page.locator(".bubble")).toBeHidden();
    await expect(panel.locator(".sp-word")).toHaveText("documnet");
    await expect(panel.locator(".sp-count")).toHaveText("1 of 3");
    // The word is selected in the text and its corrections are buttons.
    await expect.poll(() => page.evaluate(() => window.getSelection()!.toString())).toBe("documnet");
    await expect(panel.getByRole("option").first()).toHaveText("document");
    expect(await panel.getByRole("option").count()).toBeLessThanOrEqual(5);

    await panel.getByRole("button", { name: "Change", exact: true }).click();
    await expect.poll(() => engineText(page)).toContain("The document has an erorr here.");
    await expect(panel.locator(".sp-word")).toHaveText("erorr");
    await expect(panel.locator(".sp-count")).toHaveText("1 of 2");

    // Next and Previous move between the ones left.
    await panel.getByRole("button", { name: "Next" }).click();
    await expect(panel.locator(".sp-word")).toHaveText("recieve");
    await expect(panel.locator(".sp-count")).toHaveText("2 of 2");
    await panel.getByRole("button", { name: "Previous" }).click();
    await expect(panel.locator(".sp-word")).toHaveText("erorr");

    // Ignore and Add to dictionary go on to the next; at the end the panel closes.
    await panel.getByRole("button", { name: "Ignore", exact: true }).click();
    await expect(panel.locator(".sp-word")).toHaveText("recieve");
    await panel.getByRole("button", { name: "Add to dictionary" }).click();
    await expect(panel).toHaveCount(0);
    await expect(page.locator(".toast")).toHaveText("No more spelling errors");
    await expect(underlines(page)).toHaveCount(0);
    expect(await engineText(page)).toContain("erorr");
  });

  test("the check panel starts from the caret, opens with Ctrl/Cmd+Alt+X and closes with Escape", async ({ page }) => {
    await openDoc(page, ["One recieve here.", "Two teh here."]);
    await expect(underlines(page)).toHaveCount(2);
    await endOf(page, flowParas(page).first());
    await page.keyboard.press(`${MOD}+Alt+X`);
    const panel = page.getByRole("dialog", { name: "Spelling and grammar check" });
    await expect(panel).toBeVisible();
    await expect(panel.locator(".sp-word")).toHaveText("teh");
    await page.keyboard.press("Escape");
    await expect(panel).toHaveCount(0);
  });

  test("a document with nothing misspelled says so", async ({ page }) => {
    await openDoc(page, ["All of this is spelled right."]);
    await page.waitForTimeout(300);
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");
    await expect(page.locator(".toast")).toHaveText("No spelling errors");
    await expect(page.locator(".sp-panel")).toHaveCount(0);
  });
});

test.describe("spelling languages", () => {
  test("a Spanish document uses the Spanish dictionary", async ({ page }) => {
    await openDoc(page, ["Ayer fuimos al mercado y compramos niños recieve."], { lang: "es-ES" });
    await expect(underline(page, "recieve")).toHaveCount(1);
    expect(await underlined(page)).toEqual(["recieve"]);
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("es-ES");
    await rightClickWord(page, "recieve");
    // The Spanish suggestions, not English ones.
    expect(await contextMenu(page).locator(".menu-label").first().textContent()).not.toBe("receive");
  });

  test("English words are flagged in a Spanish document and the other way round", async ({ page }) => {
    await openDoc(page, ["The parcel arrived yesterday."], { lang: "es-ES" });
    await expect(underline(page, "parcel")).toHaveCount(1);
    await openDoc(page, ["Ayer llegó el paquete."], { lang: "en-US" });
    await expect(underline(page, "paquete")).toHaveCount(1);
  });

  test("British English accepts colour and flags color", async ({ page }) => {
    await openDoc(page, ["The colour and the color."], { lang: "en-GB" });
    await expect(underline(page, "color")).toHaveCount(1);
    await expect(underline(page, "colour")).toHaveCount(0);
  });

  test("French and German documents have dictionaries too", async ({ page }) => {
    await openDoc(page, ["Bonjour le monde et recieve."], { lang: "fr-FR" });
    await expect(underline(page, "recieve")).toHaveCount(1);
    await openDoc(page, ["Guten Morgen und recieve."], { lang: "de-DE" });
    await expect(underline(page, "recieve")).toHaveCount(1);
  });

  test("a language without a dictionary is not checked, says so under Tools, and leaves the browser's spellcheck on", async ({ page }) => {
    await openDoc(page, ["Hello wrold, recieve this."], { lang: "ja-JP" });
    await page.waitForTimeout(600);
    await expect(underlines(page)).toHaveCount(0);
    await expect(page.locator(".flow")).toHaveAttribute("spellcheck", "true");
    await chooseMenu(page, "Tools", "Spelling and grammar", "Spelling and grammar check");
    await expect(page.locator(".toast")).toHaveText("Spelling isn't available for this language");
  });
});

test.describe("spelling in large documents", () => {
  test("typing stays quick with the checker on, on long.docx", async ({ page }) => {
    await openFixture(page, "long.docx");
    // Let the first pass and the idle sweep finish, then type like the keyboard does (one typing edit per frame)
    // and time each keystroke up to the frame that shows it, with the checker on and then off.
    await page.waitForTimeout(2500);
    const perKey = () =>
      inPage(page, async (w) => {
        const e = w.fe.editor as unknown as { select(p: object): void; exec(o: object, opts: object): unknown };
        const frame = () => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));
        const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
        const text = "The quick brown fox jumps over the lazy dog and recieve ";
        let o = 0;
        const t0 = performance.now();
        for (const ch of text) {
          e.select({ p, o });
          e.exec({ op: "insert_text", at: { p, o }, text: ch }, { typing: true, merge: o > 0 });
          o++;
          await frame();
        }
        return (performance.now() - t0) / text.length;
      });
    const on = await perKey();
    await chooseMenu(page, "Tools", "Spelling and grammar", "Show spelling suggestions");
    await expect(underlines(page)).toHaveCount(0);
    const off = await perKey();
    console.log(`long.docx (${await flowParas(page).count()} paragraphs): ${on.toFixed(1)} ms per keystroke with the checker on, ${off.toFixed(1)} ms off (two frames of ${(1000 / 60).toFixed(1)} ms are included in both)`);
    // The checker adds well under a frame to a keystroke.
    expect(on).toBeLessThan(off + 12);
  });
});

test.describe("a host without dictionaries", () => {
  test("an editor created with spelling: false has no checker and keeps the browser's spellcheck", async ({ page }) => {
    await openFixture(page, "simple.docx");
    const result = await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { constructor: new (o: object) => any } } }).docxtor;
      const host = document.createElement("div");
      host.style.cssText = "position:fixed;left:0;top:0;width:600px;height:400px;z-index:5";
      document.body.append(host);
      const ed = new w.fe.constructor({ spelling: false, menuBar: true });
      ed.mount(host);
      await ed.ready;
      await ed.create("plain.docx");
      const p = Number(host.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      ed.exec({ op: "insert_text", at: { p, o: 0 }, text: "recieve this" });
      await new Promise((r) => setTimeout(r, 700));
      const out = {
        underlines: host.querySelectorAll(".sp-u").length,
        spellcheck: host.querySelector(".flow")!.getAttribute("spellcheck"),
        commands: ed.extensions.commands.list().filter((c: { id: string }) => c.id.startsWith("tools.spell") || c.id === "tools.personalDictionary").length,
        active: ed.editor.spell.active,
      };
      ed.destroy();
      host.remove();
      return out;
    });
    expect(result).toEqual({ underlines: 0, spellcheck: "true", commands: 0, active: false });
  });
});

async function rightClickAtParagraph(page: Page) {
  await flowParas(page).first().click({ button: "right", position: { x: 4, y: 8 } });
  await expect(contextMenu(page)).toBeVisible();
}
