// Find and replace as Word's: options, special characters, wildcards, regular expressions, keeping the case,
// paragraph marks, headers, footers and notes, formatting to find and to put on, and the list of results.
import { openBuilt } from "./support/docx-builder";
import { expect, inPage, openFixture, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const bar = (page: Page) => page.getByRole("search");
const findBox = (page: Page) => bar(page).getByRole("searchbox", { name: "Find" });
const replaceBox = (page: Page) => bar(page).getByRole("textbox", { name: "Replace with" });
const count = (page: Page) => bar(page).locator(".find-count");
const option = (page: Page, label: string) => bar(page).getByLabel(label, { exact: true });
const flowText = async (page: Page) => (await inPage(page, (w) => w.fe.editor.engine!.text())).trimEnd();

async function openBar(page: Page) {
  await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
  // To the start of the document with Docxtor's own key: Home alone scrolls on a Mac in Safari and puts no caret anywhere.
  await page.keyboard.press(`${MOD}+Home`);
  await page.keyboard.press(`${MOD}+F`);
  await expect(bar(page)).toBeVisible();
}

async function moreOptions(page: Page) {
  if ((await bar(page).locator(".find-more").isHidden())) await bar(page).getByRole("button", { name: "More" }).click();
}

/** The highlighted ranges the find bar painted, as the text they cover and the area they are in. */
const painted = (page: Page, name = "find") =>
  page.evaluate((n) => {
    // The highlights hold fixed ranges (they keep their text when a far block is drawn): read each through a live one.
    const hl = (CSS as unknown as { highlights: Map<string, Set<AbstractRange>> }).highlights.get(n);
    const text = (r: AbstractRange) => {
      const live = document.createRange();
      live.setStart(r.startContainer, r.startOffset);
      live.setEnd(r.endContainer, r.endOffset);
      return live.toString();
    };
    return hl ? Array.from(hl).map((r) => ({ text: text(r), where: (r.startContainer.parentElement?.closest(".hf, .notes") ? "area" : "text") as string })) : [];
  }, name);

const WORDS = ["The cat sat on the mat.", "Concatenate the cats and the cat.", "Dates: 2024-05-17 and 1999-12-03; call 555-1234.", "Smith met Smythe; Jones left."];

async function openWords(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: WORDS }] });
  await openBar(page);
}

test.describe("find limits", () => {
  test("a long text to find is capped at 255 characters and long patterns do not take the engine down", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["word ".repeat(4000) + "ZZ end", "Short one."] }] });
    await openBar(page);
    // 255 characters is searched (a call per character would not fit the engine's stack).
    await findBox(page).fill("word ".repeat(51));
    await expect(count(page)).toHaveText(/^1 of \d+\+?$/);
    // 256 is not: the bar says so, and nothing can be replaced.
    await findBox(page).fill("word ".repeat(51) + "w");
    await expect(count(page)).toHaveText("Too long (255 characters at most)");
    await expect(bar(page).getByRole("button", { name: "All", exact: true })).toBeDisabled();
    // A pattern that backtracks deeply over a 20,000-character paragraph stops, and the engine still works afterwards.
    await moreOptions(page);
    await option(page, "Regular expression").check();
    await findBox(page).fill(String.raw`(\w+ )+x`);
    await expect(count(page)).not.toHaveText("Too long (255 characters at most)");
    await option(page, "Regular expression").uncheck();
    await findBox(page).fill("ZZ end");
    await expect(count(page)).toHaveText("1 of 1");
    await findBox(page).fill("Short one");
    await expect(count(page)).toHaveText("1 of 1");
  });
});

test.describe("find options", () => {
  test("match case and whole words change what is found", { tag: "@core" }, async ({ page }) => {
    await openWords(page);
    await findBox(page).fill("the");
    // Case is ignored by default: "The", "the", "the", "the" and the end of "Smythe".
    await expect(count(page)).toHaveText("1 of 5");
    await moreOptions(page);
    await option(page, "Match case").check();
    await expect(count(page)).toHaveText("1 of 4");
    await option(page, "Match case").uncheck();
    await findBox(page).fill("cat");
    await expect(count(page)).toHaveText("1 of 4");
    await option(page, "Whole words only").check();
    await expect(count(page)).toHaveText("1 of 2");
    await option(page, "Whole words only").uncheck();
    await option(page, "Match prefix").check();
    await expect(count(page)).toHaveText("1 of 3");
    await option(page, "Match prefix").uncheck();
    await option(page, "Match suffix").check();
    await expect(count(page)).toHaveText("1 of 2");
  });

  test("the matches are painted on the page and the current one is stronger", async ({ page }) => {
    await openWords(page);
    await findBox(page).fill("cat");
    await expect(count(page)).toHaveText("1 of 4");
    expect((await painted(page)).map((r) => r.text)).toEqual(["cat", "cat", "cat", "cat"]);
    expect((await painted(page, "find-current")).map((r) => r.text)).toEqual(["cat"]);
    await findBox(page).press("Enter");
    await expect(count(page)).toHaveText("2 of 4");
  });

  test("wildcards: sets, counts, word edges and groups in the replacement", async ({ page }) => {
    await openWords(page);
    await moreOptions(page);
    await option(page, "Use wildcards").check();
    await expect(option(page, "Match case")).toBeDisabled();
    await findBox(page).fill("<s?t>");
    await expect(count(page)).toHaveText("1 of 1");
    await findBox(page).fill("[0-9]{4}-[0-9]{2}-[0-9]{2}");
    await expect(count(page)).toHaveText("1 of 2");
    await findBox(page).fill("([0-9]{4})-([0-9]{2})-([0-9]{2})");
    await replaceBox(page).fill("\\3/\\2/\\1");
    await bar(page).getByRole("button", { name: "All", exact: true }).click();
    await expect(count(page)).toHaveText("Replaced 2");
    expect(await flowText(page)).toContain("Dates: 17/05/2024 and 03/12/1999; call 555-1234.");
    await findBox(page).fill("[abc");
    await expect(count(page)).toHaveText("Not a valid pattern");
  });

  test("regular expressions and their groups", async ({ page }) => {
    await openWords(page);
    await moreOptions(page);
    await option(page, "Regular expression").check();
    // Wildcards and regular expressions exclude each other.
    await option(page, "Use wildcards").check();
    await expect(option(page, "Regular expression")).not.toBeChecked();
    await option(page, "Regular expression").check();
    await expect(option(page, "Use wildcards")).not.toBeChecked();
    await findBox(page).fill(String.raw`\b\w+at\b`);
    await expect(count(page)).toHaveText("1 of 4");
    await findBox(page).fill(String.raw`(\d{3})-(\d{4})`);
    await replaceBox(page).fill("$2-$1");
    await bar(page).getByRole("button", { name: "Replace", exact: true }).click();
    expect(await flowText(page)).toContain("call 1234-555.");
    await findBox(page).fill("(unclosed");
    await expect(count(page)).toHaveText("Not a valid pattern");
    await expect(bar(page).getByRole("button", { name: "All", exact: true })).toBeDisabled();
  });

  test("ignoring punctuation and accents, and words that sound alike", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["A well-known café, and a cafe; Smith met Smythe."] }] });
    await openBar(page);
    await moreOptions(page);
    await findBox(page).fill("wellknown");
    await expect(count(page)).toHaveText("No results");
    await option(page, "Ignore punctuation").check();
    await expect(count(page)).toHaveText("1 of 1");
    await option(page, "Ignore punctuation").uncheck();
    await findBox(page).fill("cafe");
    await expect(count(page)).toHaveText("1 of 1");
    await option(page, "Ignore accents").check();
    await expect(count(page)).toHaveText("1 of 2");
    await option(page, "Ignore accents").uncheck();
    await findBox(page).fill("Smith");
    await option(page, "Sounds like (English)").check();
    await expect(count(page)).toHaveText("1 of 2");
  });

  test("the Special menu puts codes into the field, and they find what they stand for", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["one{TAB}two", "three four"] }] });
    await openBar(page);
    await moreOptions(page);
    await bar(page).getByRole("button", { name: "Special" }).first().click();
    await page.getByRole("menuitem", { name: /Tab/ }).click();
    await expect(findBox(page)).toHaveValue("^t");
    await expect(count(page)).toHaveText("1 of 1");
    await findBox(page).fill("e");
    await bar(page).getByRole("button", { name: "Special" }).first().click();
    await page.getByRole("menuitem", { name: /Paragraph mark/ }).click();
    await expect(findBox(page)).toHaveValue("e^p");
    await expect(count(page)).toHaveText("No results");
    await findBox(page).fill("o^p");
    await expect(count(page)).toHaveText("1 of 1");
    // The replacement's own list has "Find what text".
    await bar(page).getByRole("button", { name: "Special" }).nth(1).click();
    await expect(page.getByRole("menuitem", { name: /Find what text/ })).toBeVisible();
    await page.keyboard.press("Escape");
  });
});

test.describe("replacing", () => {
  test("replacing keeps the case of what is replaced when Match case is off", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Apple and APPLE and apple and Big Apple."] }] });
    await openBar(page);
    await findBox(page).fill("apple");
    await replaceBox(page).fill("pear");
    await bar(page).getByRole("button", { name: "All", exact: true }).click();
    await expect(count(page)).toHaveText("Replaced 4");
    expect(await flowText(page)).toBe("Pear and PEAR and pear and Big Pear.");
  });

  test("with Match case on the replacement is as typed", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Apple and apple."] }] });
    await openBar(page);
    await moreOptions(page);
    await option(page, "Match case").check();
    await findBox(page).fill("Apple");
    await replaceBox(page).fill("pear");
    await bar(page).getByRole("button", { name: "All", exact: true }).click();
    expect(await flowText(page)).toBe("pear and apple.");
  });

  test("^p in the text to find joins paragraphs and ^p in the replacement splits them", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["alpha", "beta", "gamma"] }] });
    await openBar(page);
    await findBox(page).fill("alpha^pbeta");
    await expect(count(page)).toHaveText("1 of 1");
    await replaceBox(page).fill("alpha beta");
    await bar(page).getByRole("button", { name: "Replace", exact: true }).click();
    await expect(page.locator(".flow .fp")).toHaveCount(2);
    expect(await flowText(page)).toBe("alpha beta\ngamma");
    await findBox(page).fill(" ");
    await replaceBox(page).fill("^p");
    await bar(page).getByRole("button", { name: "All", exact: true }).click();
    await expect(page.locator(".flow .fp")).toHaveCount(3);
    expect(await flowText(page)).toBe("alpha\nbeta\ngamma");
    // One undo takes the split back.
    await inPage(page, (w) => w.fe.editor.undo());
    await expect(page.locator(".flow .fp")).toHaveCount(2);
    expect(await flowText(page)).toBe("alpha beta\ngamma");
  });

  test("Replace goes on to the next match and leaves the rest alone", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["one two one two one"] }] });
    await openBar(page);
    await findBox(page).fill("one");
    await replaceBox(page).fill("1");
    const replace = bar(page).getByRole("button", { name: "Replace", exact: true });
    await replace.click();
    expect(await flowText(page)).toBe("1 two one two one");
    await expect(count(page)).toHaveText("1 of 2");
    await replace.click();
    expect(await flowText(page)).toBe("1 two 1 two one");
  });

  test("Down and Up search from the caret", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["x one", "x two", "x three"] }] });
    await page.locator(".flow .fp").nth(1).click({ position: { x: 30, y: 8 } });
    await page.keyboard.press("End");
    await page.keyboard.press(`${MOD}+F`);
    await findBox(page).fill("x");
    await expect(count(page)).toHaveText("3 of 3");
    await moreOptions(page);
    await bar(page).getByLabel("Search", { exact: true }).selectOption("down");
    await expect(count(page)).toHaveText("1 of 1");
    await bar(page).getByLabel("Search", { exact: true }).selectOption("up");
    await expect(count(page)).toHaveText("1 of 2");
  });
});

test.describe("headers, footers, notes and text boxes", () => {
  test("a header and a footer are searched, painted and replaced", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openBar(page);
    await findBox(page).fill("confidential");
    await expect(count(page)).toHaveText("1 of 1");
    // The header is drawn on both pages, so it is painted twice.
    const marks = await painted(page);
    expect(new Set(marks.map((m) => m.text))).toEqual(new Set(["confidential"]));
    expect(marks.every((m) => m.where === "area")).toBe(true);
    await moreOptions(page);
    await bar(page).getByLabel("In", { exact: true }).selectOption("main");
    await expect(count(page)).toHaveText("No results");
    await bar(page).getByLabel("In", { exact: true }).selectOption("all");
    await replaceBox(page).fill("internal");
    await bar(page).getByRole("button", { name: "All", exact: true }).click();
    await expect(page.locator(".hf.hdr").first()).toContainText("Docxtor — internal draft");
  });

  test("clicking a match in the list opens the header for editing with the match selected", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openBar(page);
    await findBox(page).fill("confidential");
    await bar(page).getByRole("button", { name: "List results" }).click();
    await expect(page.locator(".find-hit .find-hit-place")).toHaveText("Header");
    // Stepping to it only highlights it: the find bar keeps the keyboard.
    await expect(page.locator(".story-edit")).toHaveCount(0);
    await page.locator(".find-hit").click();
    await expect(page.locator(".story-edit")).toHaveCount(1);
    const sel = await inPage(page, (w) => {
      const e = w.fe.editor;
      return e.engine!.textRange(e.sel!.anchor, e.sel!.focus);
    });
    expect(sel).toBe("confidential");
  });

  test("Replace changes a match in the header, leaves the text alone and undoes in one step", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openBar(page);
    await findBox(page).fill("confidential");
    await replaceBox(page).fill("private");
    await bar(page).getByRole("button", { name: "Replace", exact: true }).click();
    await expect(page.locator(".hf.hdr").first()).toContainText("Docxtor — private draft");
    expect(await flowText(page)).not.toContain("private");
    await inPage(page, (w) => w.fe.editor.undo());
    await expect(page.locator(".hf.hdr").first()).toContainText("Docxtor — confidential draft");
  });

  test("Select all says why when the matches are not in the text", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openBar(page);
    await findBox(page).fill("confidential");
    await bar(page).getByRole("button", { name: "Select all" }).click();
    await expect(page.locator(".toast")).toContainText("headers, footers, notes or across paragraphs");
    await expect(bar(page)).toBeVisible();
  });

  test("Escape closes the bar with the current match selected", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["alpha beta gamma beta"] }] });
    await openBar(page);
    await findBox(page).fill("beta");
    await findBox(page).press("Enter");
    await expect(count(page)).toHaveText("2 of 2");
    await findBox(page).press("Escape");
    await expect(bar(page)).toBeHidden();
    const sel = await inPage(page, (w) => {
      const e = w.fe.editor;
      return [e.sel!.anchor.o, e.sel!.focus.o];
    });
    expect(sel).toEqual([17, 21]);
  });

  test("a footnote is searched and painted where it is drawn", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Body text with a note here.", "More text."] }] });
    await inPage(page, (w) => {
      const first = w.fe.editor.engine!.renderAll().order[0];
      w.fe.editor.exec({ op: "insert_footnote", at: { p: first, o: 14 }, text: "The note mentions apples." });
    });
    await openBar(page);
    await findBox(page).fill("apples");
    await expect(count(page)).toHaveText("1 of 1");
    const marks = await painted(page);
    expect(marks).toEqual([{ text: "apples", where: "area" }]);
    await bar(page).getByRole("button", { name: "List results" }).click();
    await expect(page.locator(".find-hit")).toHaveCount(1);
    await expect(page.locator(".find-hit .find-hit-place")).toHaveText("Footnote");
  });

  test("text in a text box is found", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before the box."] }] });
    await inPage(page, (w) => {
      const first = w.fe.editor.engine!.renderAll().order[0];
      w.fe.editor.exec({ op: "insert_text_box", at: { p: first, o: 6 }, w: 144, h: 60, text: "Inside the box", floating: false });
    });
    await openBar(page);
    await findBox(page).fill("Inside");
    await expect(count(page)).toHaveText("1 of 1");
    await bar(page).getByRole("button", { name: "List results" }).click();
    await expect(page.locator(".find-hit .find-hit-place")).toHaveText("Text box");
  });
});

test.describe("the list of results", () => {
  test("lists every match with its context, and a click goes there", async ({ page }) => {
    await openWords(page);
    await findBox(page).fill("cat");
    await bar(page).getByRole("button", { name: "List results" }).click();
    const hits = page.locator(".find-hit");
    await expect(hits).toHaveCount(4);
    await expect(page.locator(".find-results-head")).toHaveText("4 results");
    await expect(hits.nth(1)).toContainText("Concatenate");
    await expect(hits.nth(1).locator("mark")).toHaveText("cat");
    await hits.nth(3).click();
    await expect(count(page)).toHaveText("4 of 4");
    // The click selects the match in the text.
    const sel = await inPage(page, (w) => {
      const e = w.fe.editor;
      return e.engine!.textRange(e.sel!.anchor, e.sel!.focus);
    });
    expect(sel).toBe("cat");
  });
});

/** Which runs are bold in the first paragraph of the open document (the model's own idea). */
const boldWords = (page: Page, id: number) =>
  inPage(page, (w, p) => {
    const e = w.fe.editor.engine!;
    const text = e.textRange({ p, o: 0 }, { p, o: 1000 });
    const out: string[] = [];
    let cur = "";
    for (let i = 0; i < text.length; i++) {
      const bold = !!e.formatAt({ p, o: i + 1 })?.run.bold;
      if (bold) cur += text[i];
      else if (cur) {
        out.push(cur);
        cur = "";
      }
    }
    if (cur) out.push(cur);
    return out;
  }, id);

test.describe("formatting", () => {
  test("find formatting finds bold text and a replacement style changes it", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: '<w:r><w:t xml:space="preserve">Plain words, </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold words</w:t></w:r><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>more bold</w:t></w:r><w:r><w:t>.</w:t></w:r>' }] }] });
    await openBar(page);
    await moreOptions(page);
    await bar(page).getByRole("button", { name: "Format", exact: true }).first().click();
    await page.getByRole("menuitem", { name: "Font…" }).click();
    const dlg = page.getByRole("dialog", { name: "Find font" });
    await dlg.getByLabel("Bold", { exact: true }).selectOption("on");
    await dlg.getByRole("button", { name: "OK" }).click();
    await expect(bar(page).locator(".find-note").first()).toContainText("Bold");
    // No text and a format: every stretch of bold text.
    await expect(count(page)).toHaveText("1 of 2");
    expect((await painted(page)).map((r) => r.text)).toEqual(["bold words", "more bold"]);
    // Replace with the Emphasis character style and no replacement text: the words stay.
    await bar(page).getByRole("button", { name: "Format", exact: true }).nth(1).click();
    await page.getByRole("menuitem", { name: "Style…" }).click();
    const style = page.getByRole("dialog", { name: "Replace with style" });
    await style.getByLabel("Style", { exact: true }).selectOption({ label: "Emphasis" });
    await style.getByRole("button", { name: "OK" }).click();
    await bar(page).getByRole("button", { name: "All", exact: true }).click();
    expect(await flowText(page)).toBe("Plain words, bold words and more bold.");
    // The Emphasis style is italic, and the paragraph shows it.
    const italics = await page.locator(".flow .fp").first().evaluate((el) => {
      const out: string[] = [];
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) if (getComputedStyle(n.parentElement!).fontStyle === "italic") out.push((n as Text).data);
      return out;
    });
    expect(italics).toEqual(["bold words", "more bold"]);
    const styled = await inPage(page, (w) => w.fe.editor.engine!.find("", { find_format: { style: "Emphasis" } }).hits.map((h) => h.text));
    expect(styled).toEqual(["bold words", "more bold"]);
    expect(await boldWords(page, await inPage(page, (w) => w.fe.editor.engine!.renderAll().order[0]))).toEqual(["bold words", "more bold"]);
  });

  test("replacing text with a formatted replacement", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Intro", "Key point here", "Outro"] }] });
    await openBar(page);
    await moreOptions(page);
    await findBox(page).fill("Key point");
    await replaceBox(page).fill("Highlight");
    await bar(page).getByRole("button", { name: "Format", exact: true }).nth(1).click();
    await page.getByRole("menuitem", { name: "Font…" }).click();
    const dlg = page.getByRole("dialog", { name: "Replace with font" });
    await dlg.getByLabel("Bold", { exact: true }).selectOption("on");
    await dlg.getByLabel("Colour").fill("#ff0000");
    await dlg.getByRole("button", { name: "OK" }).click();
    await expect(bar(page).locator(".find-note").nth(1)).toContainText("Bold, #FF0000");
    await bar(page).getByRole("button", { name: "Replace", exact: true }).click();
    expect(await flowText(page)).toBe("Intro\nHighlight here\nOutro");
    const hit = await inPage(page, (w) => w.fe.editor.engine!.find("", { find_format: { bold: true, color: "FF0000" } }).hits.map((h) => h.text));
    expect(hit).toEqual(["Highlight"]);
    // No formatting clears the note.
    await bar(page).getByRole("button", { name: "No formatting" }).last().click();
    await expect(bar(page).locator(".find-note").nth(1)).toBeHidden();
  });
});

test.describe("select similar formatting", () => {
  test("selects every stretch of text formatted like the caret", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: [{ text: "", xml: '<w:r><w:t xml:space="preserve">Plain </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>one</w:t></w:r><w:r><w:t xml:space="preserve"> mid </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>two</w:t></w:r>' }, { text: "", xml: '<w:r><w:rPr><w:b/></w:rPr><w:t>three</w:t></w:r><w:r><w:t> end</w:t></w:r>' }] }] });
    const first = page.locator(".flow .fp").first();
    await first.click({ position: { x: 4, y: 8 } });
    // Put the caret in "one".
    await inPage(page, (w) => {
      const e = w.fe.editor;
      const p = e.engine!.renderAll().order[0];
      e.select({ p, o: 8 });
    });
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Edit", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: "Select text with similar formatting" }).click();
    await expect(page.locator(".toast")).toHaveText("3 places selected");
    await expect(page.locator(".xs-layer .xs-r")).toHaveCount(2);
  });
});

const caret = (page: Page) => inPage(page, (w) => w.fe.editor.sel?.focus ?? null);
const gotoDialog = (page: Page) => page.getByRole("dialog", { name: "Go to" });

/** Open Go to and choose the kind of place. */
async function openGoto(page: Page, kind: string) {
  await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
  await page.keyboard.press("Home");
  await page.keyboard.press(`${MOD}+g`);
  await expect(gotoDialog(page)).toBeVisible();
  await gotoDialog(page).getByRole("option", { name: kind, exact: true }).click();
}

test.describe("Go to", () => {
  test("a number goes to that page, and + or - moves from where you are", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openGoto(page, "Page");
    await gotoDialog(page).getByLabel("Page number").fill("2");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Page 2");
    const p = (await caret(page))!.p;
    expect((await inPage(page, (w) => w.fe.editor.pageMap()))[String(p)]).toBe(2);
    await gotoDialog(page).getByLabel("Page number").fill("-1");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Page 1");
    await gotoDialog(page).getByLabel("Page number").fill("9");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("The document has 2 pages.");
  });

  test("Next and Previous step through the headings", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openGoto(page, "Heading");
    const next = gotoDialog(page).getByRole("button", { name: "Next", exact: true });
    const sel = () => inPage(page, (w) => { const e = w.fe.editor; return e.engine!.textRange(e.sel!.anchor, e.sel!.focus); });
    await next.click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Heading 1: Highlights");
    await next.click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Heading 2: Plan");
    await next.click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Heading 1: Numbers");
    await gotoDialog(page).getByRole("button", { name: "Previous", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Heading 2: Plan");
    expect((await caret(page))!.p).toBe(8);
    // The list picks one directly.
    await gotoDialog(page).getByLabel("Choose one").selectOption({ label: "Heading 1: Appendix" });
    expect((await caret(page))!.p).toBe(31);
    expect(await sel()).toBe("");
  });

  test("tables, pictures and bookmarks", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openGoto(page, "Table");
    await gotoDialog(page).getByLabel("Table number").fill("1");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    expect((await caret(page))!.p).toBe(16);
    await gotoDialog(page).getByRole("option", { name: "Graphic", exact: true }).click();
    await gotoDialog(page).getByRole("button", { name: "Next", exact: true }).click();
    expect((await caret(page))!.p).toBe(28);
    // Nothing of that kind: the dialog says so.
    await gotoDialog(page).getByRole("option", { name: "Equation", exact: true }).click();
    await gotoDialog(page).getByRole("button", { name: "Next", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("There are none in the document.");
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_bookmark", name: "Spot", from: { p: 12, o: 0 }, to: { p: 12, o: 4 } }));
    await gotoDialog(page).getByRole("option", { name: "Bookmark", exact: true }).click();
    await gotoDialog(page).getByLabel("Choose one").selectOption({ label: "Spot" });
    expect((await caret(page))!.p).toBe(12);
  });

  test("a line number goes to that line", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await openGoto(page, "Line");
    // Line 1 is the title; the second paragraph runs over two lines.
    await gotoDialog(page).getByLabel("Line number").fill("3");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    const at = (await caret(page))!;
    expect(at.p).toBe(2);
    expect(at.o).toBeGreaterThan(20);
    await gotoDialog(page).getByLabel("Line number").fill("+1");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Line 4");
    expect((await caret(page))!.p).toBe(3);
  });

  test("F5 and Edit, Go to open it, and Escape closes it", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
    await page.keyboard.press("F5");
    await expect(gotoDialog(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(gotoDialog(page)).toHaveCount(0);
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Edit", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Go to…/ }).click();
    await expect(gotoDialog(page)).toBeVisible();
  });
});

test.describe("Go to, more kinds", () => {
  test("sections and fields, and a kind with none says so", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First section.", "More {PAGE} here."] }, { start: "nextPage", paragraphs: ["Second section."] }] });
    await openGoto(page, "Section");
    await gotoDialog(page).getByLabel("Section number").fill("2");
    await gotoDialog(page).getByRole("button", { name: "Go to", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText("Section 2");
    const second = await inPage(page, (w) => w.fe.editor.engine!.renderAll().order[2]);
    expect((await caret(page))!.p).toBe(second);
    await gotoDialog(page).getByRole("option", { name: "Field", exact: true }).click();
    await gotoDialog(page).getByRole("button", { name: "Next", exact: true }).click();
    await expect(gotoDialog(page).getByRole("status")).toHaveText(/^Field 1: PAGE/);
    // The field's result is selected.
    const marked = await inPage(page, (w) => {
      const e = w.fe.editor;
      return e.engine!.textRange(e.sel!.anchor, e.sel!.focus);
    });
    expect(marked).toBe("1");
  });
});

test.describe("next and previous footnote", () => {
  test("the commands step through the notes and say when there are none", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First paragraph here.", "Second paragraph here.", "Third paragraph here."] }] });
    await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
    await page.keyboard.press("Home");
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "Insert", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^References/ }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Go to a note/ }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: /^Next footnote/ }).click();
    await expect(page.locator(".toast")).toHaveText("There are no footnotes.");
    const ids = await inPage(page, (w) => {
      const order = w.fe.editor.engine!.renderAll().order;
      order.forEach((p, i) => w.fe.editor.exec({ op: "insert_footnote", at: { p, o: 5 }, text: `Note ${i + 1}.` }));
      return order;
    });
    await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
    await page.keyboard.press("Home");
    const press = async (key: string) => {
      await page.keyboard.press(`${MOD}+Alt+${key}`);
    };
    await press("]");
    await expect.poll(async () => (await caret(page))?.p).toBe(ids[0]);
    await press("]");
    await expect.poll(async () => (await caret(page))?.p).toBe(ids[1]);
    await press("[");
    await expect.poll(async () => (await caret(page))?.p).toBe(ids[0]);
    // The reference mark itself is selected.
    const marked = await inPage(page, (w) => { const e = w.fe.editor; return [e.sel!.anchor.o, e.sel!.focus.o]; });
    expect(marked).toEqual([5, 6]);
  });
});

/** Long documents: the find bar stays quick. */
test("finding in a long document is quick and the page keeps up while typing", async ({ page }) => {
  const paragraphs = Array.from({ length: 2400 }, (_, i) => `Paragraph ${i} talks about the quarterly numbers, the roadmap and one more thing: foo ${i % 7}.`);
  await openBuilt(page, { sections: [{ paragraphs }] });
  await openBar(page);
  const ms = await inPage(page, (w) => {
    const e = w.fe.editor.engine as unknown as { find(q: string, o?: unknown): { hits: unknown[] } };
    const t = performance.now();
    const n = e.find("foo", { ignore_case: true }).hits.length;
    const x = e.find("the", { ignore_case: true, whole_word: true }).hits.length;
    return { ms: performance.now() - t, n, x };
  });
  expect(ms.n).toBe(2400);
  expect(ms.ms).toBeLessThan(1500);
  await findBox(page).pressSequentially("quarterly", { delay: 5 });
  await expect(count(page)).toHaveText("1 of 2,400");
  // The bar answers without freezing the page: a frame still comes within a fraction of a second.
  const frame = await page.evaluate(() => new Promise<number>((res) => { const t = performance.now(); requestAnimationFrame(() => res(performance.now() - t)); }));
  expect(frame).toBeLessThan(500);
});

test.describe("find previous from the keyboard", () => {
  test("Mod+Shift+G from the document steps back through the matches, and F3 steps forward", async ({ page }) => {
    await openWords(page);
    await findBox(page).fill("cat");
    await expect(count(page)).toHaveText("1 of 4");
    // The shortcut is the command's own, so a clash with another command shows up here.
    const keys = await page.evaluate(() => {
      const host = (window as unknown as { docxtor: { fe: { extensions: { shortcutSpecs(id: string): string[]; shortcutUsers(spec: string): { id: string }[] } } } }).docxtor.fe.extensions;
      return { specs: host.shortcutSpecs("find.previous"), users: host.shortcutUsers("Mod+Shift+G").map((c) => c.id) };
    });
    expect(keys).toEqual({ specs: ["Mod+Shift+G"], users: ["find.previous"] });
    // Back to the document, with the bar still showing its matches.
    await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
    await page.keyboard.press(`${MOD}+Shift+G`);
    await expect(count(page)).toHaveText("4 of 4");
    await page.keyboard.press(`${MOD}+Shift+G`);
    await expect(count(page)).toHaveText("3 of 4");
    await page.keyboard.press("F3");
    await expect(count(page)).toHaveText("4 of 4");
  });
});

export type { Locator };
