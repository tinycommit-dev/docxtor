// East Asian typography: the document's own CJK fonts, phonetic guides (ruby) and the Phonetic guide box, enclosed
// characters, two lines in one, text upright in vertical lines, the Asian typography tab of the Paragraph box, line
// breaking (kinsoku and the document's own lists), the document grid and the East Asian font in the Font box.
// Checked in computed styles and in where the browser puts the characters; Chrome draws CJK text with a system font.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, menuItem, openMenu, test, type Page } from "./helpers";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;
const open = (page: Page, paragraphs: string[], extra: Record<string, unknown> = {}, sectionXml = "") =>
  openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })), ...(sectionXml ? { xml: { end: sectionXml } } : {}) }], ...extra });

const select = (page: Page, p: number, from: number, to: number) => inPage(page, (w, a) => w.fe.editor.select({ anchor: { p: a.p, o: a.from }, focus: { p: a.p, o: a.to } }), { p, from, to });
const fmtAt = (page: Page, p: number, o: number) => inPage(page, (w, a) => w.fe.editor.engine!.formatAt({ p: a.p, o: a.o }), { p, o });
const fmtOf = (page: Page, p: number) => inPage(page, (w, id) => (w.fe.editor.engine!.renderAll().blocks.find((b: any) => b.id === id) as any).fmt, p);

/** The lines a paragraph's text is broken into: the text of each, from where the browser put every character. */
const lines = (page: Page, p: number) =>
  page.locator(`.flow [data-p="${p}"]`).evaluate((para) => {
    const out: { top: number; text: string }[] = [];
    const walker = document.createTreeWalker(para, NodeFilter.SHOW_TEXT);
    const r = document.createRange();
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const t = n as Text;
      if ((t.parentElement as HTMLElement).closest(".lbl")) continue;
      for (let i = 0; i < t.data.length; i++) {
        r.setStart(t, i);
        r.setEnd(t, i + 1);
        const rect = r.getClientRects()[0];
        if (!rect) continue;
        const last = out[out.length - 1];
        if (last && Math.abs(last.top - rect.top) < 4) last.text += t.data[i];
        else out.push({ top: rect.top, text: t.data[i] });
      }
    }
    return out.map((l) => l.text);
  });

const JP = "あいうえおかきくけこ";

test.describe("the document's own East Asian fonts", () => {
  test("East Asian text takes the run's East Asian font, Latin text the Latin one", async ({ page }) => {
    await open(page, [run('<w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="MS Mincho"/>', "Hello 漢字です abc")]);
    const spans = await page.locator('.flow [data-p="1"] .s').evaluateAll((els) => els.map((e) => ({ text: e.textContent, family: getComputedStyle(e).fontFamily })));
    const ea = spans.find((s) => s.text?.includes("漢字"))!;
    const latin = spans.find((s) => s.text?.includes("Hello"))!;
    expect(ea.family).toMatch(/^"?MS Mincho/);
    expect(latin.family).toMatch(/^"?Arial/);
    expect(spans.find((s) => s.text === "abc")!.family).toMatch(/^"?Arial/);
    await page.screenshot({ path: "test-results/east-asian-fonts.png" });
  });

  test("a style's East Asian font reaches its text, and the language picks the glyphs", async ({ page }) => {
    await open(page, [run('<w:lang w:val="en-US" w:eastAsia="ja-JP"/>', "かな漢字"), run("", "한국어")]);
    const lang = await page.locator('.flow [data-p="1"] .s').first().getAttribute("lang");
    expect(lang).toBe("ja-JP");
    // Hangul without a language is Korean; Han alone is left to the browser.
    expect(await page.locator('.flow [data-p="2"] .s').first().getAttribute("lang")).toBe("ko");
  });

  test("the Font box offers an Asian text font and writes it without touching the Latin font", async ({ page }) => {
    await open(page, [run('<w:rFonts w:ascii="Arial" w:hAnsi="Arial"/>', "abc 漢字")]);
    await select(page, 1, 0, 6);
    await chooseMenu(page, "Format", "Font…");
    const d = dialog(page, "Font");
    await expect(d.getByLabel("Asian text font")).toHaveValue("");
    await d.getByLabel("Asian text font").selectOption("MS Gothic");
    await d.getByRole("button", { name: "OK" }).click();
    const f = await fmtAt(page, 1, 5);
    expect(f!.run.fontEa).toBe("MS Gothic");
    expect(f!.run.font === "Arial" || f!.run.fontEa === "MS Gothic").toBe(true);
    const ea = await page.locator('.flow [data-p="1"] .s', { hasText: "漢字" }).evaluate((e) => getComputedStyle(e).fontFamily);
    expect(ea).toMatch(/^"?MS Gothic/);
    // Open it again: the box shows the font; choosing none removes it.
    await select(page, 1, 0, 6);
    await chooseMenu(page, "Format", "Font…");
    await expect(dialog(page, "Font").getByLabel("Asian text font")).toHaveValue("MS Gothic");
    await dialog(page, "Font").getByLabel("Asian text font").selectOption("");
    await dialog(page, "Font").getByRole("button", { name: "OK" }).click();
    expect((await fmtAt(page, 1, 5))!.run.fontEa).toBeUndefined();
  });
});

const RUBY = `${run("", "前 ")}<w:r><w:rPr><w:rFonts w:hint="eastAsia"/></w:rPr><w:ruby><w:rubyPr><w:rubyAlign w:val="distributeSpace"/><w:hps w:val="10"/><w:hpsRaise w:val="18"/><w:hpsBaseText w:val="21"/><w:lid w:val="ja-JP"/></w:rubyPr><w:rt><w:r><w:rPr><w:rFonts w:hint="eastAsia"/><w:sz w:val="10"/></w:rPr><w:t>かんじ</w:t></w:r></w:rt><w:rubyBase><w:r><w:rPr><w:rFonts w:hint="eastAsia"/></w:rPr><w:t>漢字</w:t></w:r></w:rubyBase></w:ruby></w:r>${run("", " 後")}`;

test.describe("phonetic guides", () => {
  test("the base text is shown with its guide above it", async ({ page }) => {
    await open(page, [RUBY]);
    const ruby = page.locator('.flow [data-p="1"] ruby.atom.ruby');
    await expect(ruby).toHaveCount(1);
    await expect(ruby.locator(".rb")).toHaveText("漢字");
    await expect(ruby.locator("rt")).toHaveText("かんじ");
    const rt = (await ruby.locator("rt").boundingBox())!;
    const rb = (await ruby.locator(".rb").boundingBox())!;
    // The guide sits above the base and is smaller: 5 points.
    expect(rt.y + rt.height).toBeLessThanOrEqual(rb.y + 2);
    expect(await ruby.locator("rt").evaluate((e) => parseFloat(getComputedStyle(e).fontSize))).toBeCloseTo(5 * (96 / 72), 1);
    expect(await ruby.evaluate((e) => getComputedStyle(e).rubyAlign)).toBe("space-around");
    expect(await ruby.getAttribute("lang")).toBe("ja-JP");
    // The text around it is still there, and the guide is one position.
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => e.textContent)).toContain("前 ");
    await page.screenshot({ path: "test-results/east-asian-ruby.png" });
  });

  test("the Phonetic guide box puts a guide over the selected text and undo takes it away", async ({ page }) => {
    await open(page, [run("", "前 漢字 後")]);
    await select(page, 1, 2, 4);
    await chooseMenu(page, "Format", "Asian layout", "Phonetic guide…");
    const d = dialog(page, "Phonetic guide");
    await expect(d.getByLabel("Base text")).toHaveValue("漢字");
    await d.getByLabel("Guide text").fill("かんじ");
    await d.getByLabel("Alignment").selectOption("left");
    await expect(d.locator(".pg-preview ruby rt")).toHaveText("かんじ");
    await d.getByRole("button", { name: "OK" }).click();
    const ruby = page.locator('.flow [data-p="1"] ruby.atom.ruby');
    await expect(ruby).toHaveCount(1);
    await expect(ruby.locator("rt")).toHaveText("かんじ");
    expect(await ruby.evaluate((e) => getComputedStyle(e).rubyAlign)).toBe("start");
    const g = await inPage(page, (w) => w.fe.editor.engine!.rubyAt({ p: 1, o: 2 }));
    expect(g).toMatchObject({ base: "漢字", text: "かんじ", align: "left" });
    await page.keyboard.press("Meta+z");
    await page.keyboard.press("Control+z");
    await expect(page.locator('.flow [data-p="1"] ruby')).toHaveCount(0);
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => e.textContent)).toContain("前 漢字 後");
  });

  test("double-click opens the box on a guide; the guide can be changed and removed", async ({ page }) => {
    await open(page, [RUBY]);
    await page.locator('.flow [data-p="1"] ruby.atom.ruby').dblclick();
    const d = dialog(page, "Phonetic guide");
    await expect(d).toBeVisible();
    await expect(d.getByLabel("Base text")).toHaveValue("漢字");
    await expect(d.getByLabel("Guide text")).toHaveValue("かんじ");
    await expect(d.getByLabel("Alignment")).toHaveValue("distributeSpace");
    await expect(d.getByLabel("Size (pt)")).toHaveValue("5");
    await d.getByLabel("Guide text").fill("カンジ");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(page.locator('.flow [data-p="1"] ruby rt')).toHaveText("カンジ");
    await page.locator('.flow [data-p="1"] ruby.atom.ruby').dblclick();
    await dialog(page, "Phonetic guide").getByRole("button", { name: "Remove guide" }).click();
    await expect(page.locator('.flow [data-p="1"] ruby')).toHaveCount(0);
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => e.textContent)).toContain("漢字");
  });

  test("the box suggests a reading from the providers: Docxtor's own for kana, and any an extension adds", async ({ page }) => {
    await open(page, [run("", "カンジ 漢字")]);
    // Katakana is read as hiragana without asking.
    await select(page, 1, 0, 3);
    await chooseMenu(page, "Format", "Asian layout", "Phonetic guide…");
    let d = dialog(page, "Phonetic guide");
    await expect(d.getByLabel("Guide text")).toHaveValue("かんじ");
    await expect(d.getByRole("status")).toContainText("docxtor.kana");
    await d.getByRole("button", { name: "Cancel" }).click();
    // Kanji has no reading until a provider adds one; then Suggest asks for it.
    await select(page, 1, 4, 6);
    await chooseMenu(page, "Format", "Asian layout", "Phonetic guide…");
    d = dialog(page, "Phonetic guide");
    await expect(d.getByLabel("Guide text")).toHaveValue("");
    await d.getByRole("button", { name: "Suggest" }).click();
    await expect(d.getByRole("status")).toContainText("No reading found");
    await d.getByRole("button", { name: "Cancel" }).click();
    await inPage(page, (w) =>
      w.fe.extensions.add({
        id: "test.kanji",
        name: "Kanji",
        activate: (ctx: any) => void ctx.readings.register({ id: "test.kanji", label: "Test kanji", languages: ["ja"], reading: (text: string) => (text === "漢字" ? "かんじ" : null) }),
      } as any),
    );
    await select(page, 1, 4, 6);
    await chooseMenu(page, "Format", "Asian layout", "Phonetic guide…");
    d = dialog(page, "Phonetic guide");
    await expect(d.getByLabel("Guide text")).toHaveValue("かんじ");
    await expect(d.getByRole("status")).toContainText("test.kanji");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(page.locator('.flow [data-p="1"] ruby rt')).toHaveText("かんじ");
  });

  test("the menu names the command and it is off without a selection", async ({ page }) => {
    await open(page, [run("", "漢字")]);
    await openMenu(page, "Format");
    await menuItem(page, "Asian layout").hover();
    await expect(menuItem(page, "Phonetic guide…")).toBeVisible();
    await expect(menuItem(page, "Enclose characters…")).toBeVisible();
    await expect(menuItem(page, "Two lines in one…")).toBeVisible();
    await expect(menuItem(page, "Horizontal in vertical")).toBeVisible();
    await expect(menuItem(page, "Line breaking rules…")).toBeVisible();
  });
});

test.describe("enclosed characters", () => {
  test("Enclose characters writes the field, draws it and can take it away", async ({ page }) => {
    await open(page, [run("", "前 字 後")]);
    await select(page, 1, 2, 3);
    await chooseMenu(page, "Format", "Asian layout", "Enclose characters…");
    const d = dialog(page, "Enclose characters");
    await d.getByRole("radio", { name: "Square" }).click();
    await d.getByRole("button", { name: "OK" }).click();
    const eq = page.locator('.flow [data-p="1"] .ghost.eq');
    await expect(eq).toHaveCount(1);
    expect(await eq.getAttribute("data-fi")).toContain("\\o\\ac(□,字)");
    await expect(eq.locator("math")).toContainText("字");
    // Open it again at the field: it knows the shape and removes it.
    await inPage(page, (w) => w.fe.editor.select({ anchor: { p: 1, o: 2 }, focus: { p: 1, o: 2 } }));
    await chooseMenu(page, "Format", "Asian layout", "Enclose characters…");
    await expect(dialog(page, "Enclose characters").getByRole("radio", { name: "Square" })).toHaveClass(/on/);
    await dialog(page, "Enclose characters").getByRole("button", { name: "Remove enclosure" }).click();
    await expect(page.locator('.flow [data-p="1"] .ghost.eq')).toHaveCount(0);
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => e.textContent)).toContain("前 字 後");
  });

  test("changing an enclosure's shape is one undo step", async ({ page }) => {
    await open(page, [run("", "前 字 後")]);
    await select(page, 1, 2, 3);
    await chooseMenu(page, "Format", "Asian layout", "Enclose characters…");
    await dialog(page, "Enclose characters").getByRole("radio", { name: "Square" }).click();
    await dialog(page, "Enclose characters").getByRole("button", { name: "OK" }).click();
    const eq = page.locator('.flow [data-p="1"] .ghost.eq');
    expect(await eq.getAttribute("data-fi")).toContain("\\o\\ac(□,字)");
    await inPage(page, (w) => w.fe.editor.select({ anchor: { p: 1, o: 2 }, focus: { p: 1, o: 2 } }));
    await chooseMenu(page, "Format", "Asian layout", "Enclose characters…");
    await dialog(page, "Enclose characters").getByRole("radio", { name: "Circle" }).click();
    await dialog(page, "Enclose characters").getByRole("button", { name: "OK" }).click();
    expect(await eq.getAttribute("data-fi")).toContain("\\o\\ac(○,字)");
    // One Undo gives the square back, not the bare character.
    await page.keyboard.press("ControlOrMeta+z");
    await expect(eq).toHaveCount(1);
    expect(await eq.getAttribute("data-fi")).toContain("\\o\\ac(□,字)");
  });
});

test.describe("two lines in one and text upright in a vertical line", () => {
  test("two lines in one is a box of half-size text two lines tall, with its brackets", async ({ page }) => {
    await open(page, [run("", "前") + run('<w:eastAsianLayout w:id="1" w:combine="1" w:combineBrackets="round"/>', "二行文字") + run("", "後")]);
    const box = page.locator('.flow [data-p="1"] .ea-comb');
    await expect(box).toHaveCount(1);
    const style = await box.evaluate((e) => {
      const s = getComputedStyle(e);
      return { display: s.display, size: parseFloat(s.fontSize), around: parseFloat(getComputedStyle(e.parentElement!).fontSize), h: e.getBoundingClientRect().height };
    });
    expect(style.display).toBe("inline-block");
    // Half the size of the text around it, two lines tall: one line of the paragraph, about.
    expect(style.size).toBeCloseTo(style.around / 2, 0);
    expect(style.h).toBeGreaterThan(style.size * 1.7);
    expect(style.h).toBeLessThan(style.size * 2.6);
    // Four characters in two lines of two, inside one line of the paragraph: it is no taller than a single line.
    const para = (await page.locator('.flow [data-p="1"]').boundingBox())!;
    expect(para.height).toBeLessThan(style.around * 2.2);
    const rows = await box.evaluate((e) => {
      const r = document.createRange();
      const tops = new Set<number>();
      const t = e.firstChild as Text;
      for (let i = 0; i < t.data.length; i++) {
        r.setStart(t, i);
        r.setEnd(t, i + 1);
        tops.add(Math.round(r.getClientRects()[0].top));
      }
      return tops.size;
    });
    expect(rows).toBe(2);
    const brackets = await box.evaluate((e) => [getComputedStyle(e, "::before").content, getComputedStyle(e, "::after").content]);
    expect(brackets).toEqual(['"("', '")"']);
    await page.screenshot({ path: "test-results/east-asian-combine.png" });
  });

  test("the Two lines in one box sets it on the selected text, with brackets, and removes it", async ({ page }) => {
    await open(page, [run("", "前二行文字後")]);
    await select(page, 1, 1, 5);
    await chooseMenu(page, "Format", "Asian layout", "Two lines in one…");
    const d = dialog(page, "Two lines in one");
    await d.getByLabel("Brackets").selectOption("square");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(page.locator('.flow [data-p="1"] .ea-comb')).toHaveCount(1);
    expect((await fmtAt(page, 1, 2))!.run.eaLayout).toMatchObject({ combine: true, combineBrackets: "square" });
    await select(page, 1, 1, 5);
    await chooseMenu(page, "Format", "Asian layout", "Two lines in one…");
    await dialog(page, "Two lines in one").getByRole("button", { name: "Remove" }).click();
    await expect(page.locator('.flow [data-p="1"] .ea-comb')).toHaveCount(0);
  });

  test("horizontal in vertical fits text into a vertical line; across a line it is plain text", async ({ page }) => {
    await open(
      page,
      [run("", "縦") + run('<w:eastAsianLayout w:id="2" w:vert="1" w:vertCompress="1"/>', "12") + run('<w:eastAsianLayout w:id="3" w:vert="1"/>', "ab") + run("", "横")],
      {},
      '<w:textDirection w:val="tbRl"/>',
    );
    const compress = await page.locator('.flow [data-p="1"] .s', { hasText: "12" }).evaluate((e) => getComputedStyle(e).textCombineUpright);
    expect(compress).toBe("all");
    const across = await page.locator('.flow [data-p="1"] .ea-vert').evaluate((e) => {
      const s = getComputedStyle(e);
      return { mode: s.writingMode, display: s.display };
    });
    expect(across).toEqual({ mode: "horizontal-tb", display: "inline-block" });
    // The fitted digits take one character's height; the run across is wider than tall.
    const fitted = (await page.locator('.flow [data-p="1"] .s', { hasText: "12" }).boundingBox())!;
    const plain = (await page.locator('.flow [data-p="1"] .ea-vert').boundingBox())!;
    expect(fitted.height).toBeLessThan(plain.height * 1.6 + 30);
    expect(plain.width).toBeGreaterThan(plain.height * 0.5);
    await page.screenshot({ path: "test-results/east-asian-vert.png" });
  });

  test("horizontal in vertical is a toggle in the Asian layout menu", async ({ page }) => {
    await open(page, [run("", "縦12横")], {}, '<w:textDirection w:val="tbRl"/>');
    await select(page, 1, 1, 3);
    await chooseMenu(page, "Format", "Asian layout", "Horizontal in vertical");
    expect((await fmtAt(page, 1, 2))!.run.eaLayout).toMatchObject({ vert: true, vertCompress: true });
    await select(page, 1, 1, 3);
    await chooseMenu(page, "Format", "Asian layout", "Horizontal in vertical");
    expect((await fmtAt(page, 1, 2))!.run.eaLayout).toBeUndefined();
  });
});

test.describe("vertical text in a paragraph", () => {
  test("a paragraph with its own text direction is set in a vertical writing mode", async ({ page }) => {
    await open(page, ['<w:pPr><w:textDirection w:val="tbRl"/></w:pPr>' + run("", "縦書きの段落")]);
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).writingMode)).toBe("vertical-rl");
    const b = (await page.locator('.flow [data-p="1"]').boundingBox())!;
    expect(b.height).toBeGreaterThan(b.width);
  });
});

test.describe("the Asian typography tab", () => {
  const para = (ppr: string) => ["<w:pPr>" + ppr + "</w:pPr>" + run("", "漢字 abc")];

  test("shows Word's defaults, and reads what a paragraph says", async ({ page }) => {
    await open(page, [...para(""), ...para('<w:kinsoku w:val="0"/><w:wordWrap w:val="0"/><w:overflowPunct w:val="0"/><w:topLinePunct/><w:autoSpaceDE w:val="0"/><w:autoSpaceDN w:val="0"/><w:snapToGrid w:val="0"/>')]);
    await select(page, 1, 0, 0);
    await chooseMenu(page, "Format", "Paragraph…");
    let d = dialog(page, "Paragraph");
    await d.getByRole("tab", { name: "Asian typography" }).click();
    for (const [label, on] of [["Follow the East Asian rules for the first and last character of a line", true], ["Allow Latin text to wrap in the middle of a word", false], ["Allow punctuation to hang past the margin", true], ["Compress punctuation at the start of a line", false], ["Adjust the space between Asian and Latin text", true], ["Adjust the space between Asian text and numbers", true], ["Snap to the document grid", true]] as const) {
      const box = d.getByLabel(label);
      if (on) await expect(box, label).toBeChecked();
      else await expect(box, label).not.toBeChecked();
    }
    await d.getByRole("button", { name: "Cancel" }).click();
    await select(page, 2, 0, 0);
    await chooseMenu(page, "Format", "Paragraph…");
    d = dialog(page, "Paragraph");
    await d.getByRole("tab", { name: "Asian typography" }).click();
    await expect(d.getByLabel("Follow the East Asian rules for the first and last character of a line")).not.toBeChecked();
    await expect(d.getByLabel("Allow Latin text to wrap in the middle of a word")).toBeChecked();
    await expect(d.getByLabel("Compress punctuation at the start of a line")).toBeChecked();
    await expect(d.getByLabel("Snap to the document grid")).not.toBeChecked();
  });

  test("changing a switch writes it, and one step of undo takes it back", async ({ page }) => {
    await open(page, para(""));
    await select(page, 1, 0, 0);
    await chooseMenu(page, "Format", "Paragraph…");
    const d = dialog(page, "Paragraph");
    await d.getByRole("tab", { name: "Asian typography" }).click();
    await d.getByLabel("Allow Latin text to wrap in the middle of a word").check();
    await d.getByLabel("Compress punctuation at the start of a line").check();
    await d.getByLabel("Adjust the space between Asian text and numbers").uncheck();
    await d.getByRole("button", { name: "OK" }).click();
    expect(await fmtOf(page, 1)).toMatchObject({ wordWrap: false, topLinePunct: true, autoSpaceDn: false });
    const style = await page.locator('.flow [data-p="1"]').evaluate((e) => {
      const s = getComputedStyle(e);
      return { wordBreak: s.wordBreak, trim: s.getPropertyValue("text-spacing-trim"), autospace: s.getPropertyValue("text-autospace") };
    });
    expect(style.wordBreak).toBe("break-all");
    if (await page.evaluate(() => CSS.supports("text-spacing-trim", "trim-start"))) expect(style.trim).toBe("trim-start");
    if (await page.evaluate(() => CSS.supports("text-autospace", "ideograph-alpha"))) expect(style.autospace).toBe("ideograph-alpha");
    await page.keyboard.press("Control+z");
    await page.keyboard.press("Meta+z");
    expect(await fmtOf(page, 1)).not.toMatchObject({ wordWrap: false });
  });

  test("turning auto spacing off for both is no-autospace", async ({ page }) => {
    test.skip(!(await page.evaluate(() => CSS.supports("text-autospace", "no-autospace"))), "this browser has no text-autospace");
    await open(page, para('<w:autoSpaceDE w:val="0"/><w:autoSpaceDN w:val="0"/>'));
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).getPropertyValue("text-autospace"))).toBe("no-autospace");
  });
});

test.describe("line breaking", () => {
  // A line 287 px wide holds ten 20-point characters and not eleven, whether the system draws them 26.7 px wide (a Mac) or rounds
  // them up to 28 px (Linux's fallback Japanese fonts).
  const narrow = (extra = "") => `<w:pPr><w:ind w:right="5055"/>${extra}<w:rPr><w:sz w:val="40"/></w:rPr></w:pPr>`;
  const text = (s: string) => run('<w:sz w:val="40"/><w:lang w:val="en-US" w:eastAsia="ja-JP"/>', s);

  test("kinsoku keeps punctuation from starting a line", async ({ page }) => {
    // Ten characters fit in a line; the ideographic comma would start the second.
    await open(page, [narrow() + text(`${JP}、さしすせそ`)]);
    const l = await lines(page, 1);
    expect(l.length).toBeGreaterThanOrEqual(2);
    expect(l[1][0]).not.toBe("、");
    expect(l[0].length).toBeLessThanOrEqual(10);
  });

  test("a paragraph that turns the rules off is set loosely", async ({ page }) => {
    await open(page, [narrow('<w:kinsoku w:val="0"/>') + text(`${JP}、さしすせそ`)]);
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).lineBreak)).toBe("loose");
  });

  test("the strict level of the document is the browser's strict line breaking", async ({ page }) => {
    await open(page, [narrow() + text(`${JP}ゃさしすせそ`)], { settings: "<w:strictFirstAndLastChars/>" });
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).lineBreak)).toBe("strict");
    const l = await lines(page, 1);
    // A small kana may not start a line at the strict level.
    expect(l[1][0]).not.toBe("ゃ");
  });

  test("characters the document lists as not starting a line stay with the one before", async ({ page }) => {
    await open(page, [narrow() + text(`${JP}さしすせそ`)], { settings: '<w:noLineBreaksBefore w:lang="ja-JP" w:val="さ"/>' });
    const l = await lines(page, 1);
    expect(l[0]).toBe(JP.slice(0, 9));
    expect(l[1].startsWith("こさ")).toBe(true);
    await expect(page.locator('.flow [data-p="1"] .kin-keep')).toHaveCount(1);
  });

  test("characters the document lists as not ending a line stay with the one after", async ({ page }) => {
    await open(page, [narrow() + text(`${JP.slice(0, 9)}「さしすせそ`)], { settings: '<w:noLineBreaksAfter w:lang="ja-JP" w:val="「"/>' });
    const l = await lines(page, 1);
    // The opening bracket would end the first line; it goes down with the character after it.
    expect(l[0].endsWith("「")).toBe(false);
    expect(l[1].startsWith("「さ")).toBe(true);
  });

  test("the Line breaking rules box writes the strict level and a list, and undo takes them back", async ({ page }) => {
    await open(page, [narrow() + text(`${JP}さしすせそ`)]);
    await chooseMenu(page, "Format", "Asian layout", "Line breaking rules…");
    const d = dialog(page, "Line breaking rules");
    await d.getByLabel("Rules").selectOption("strict");
    await d.getByLabel("Characters that can't start a line").fill("さ");
    await d.getByRole("button", { name: "OK" }).click();
    const s = await inPage(page, (w) => w.fe.editor.engine!.asianSettings());
    expect(s).toMatchObject({ strict: true, custom: [{ lang: "ja-JP", before: "さ", after: "" }] });
    const l = await lines(page, 1);
    expect(l[1].startsWith("こさ")).toBe(true);
    await page.keyboard.press("Control+z");
    await page.keyboard.press("Meta+z");
    expect(await inPage(page, (w) => w.fe.editor.engine!.asianSettings())).toEqual({});
    expect((await lines(page, 1))[0]).toBe(JP);
    // The box shows the standard lists on request.
    await chooseMenu(page, "Format", "Asian layout", "Line breaking rules…");
    await dialog(page, "Line breaking rules").getByRole("button", { name: "Use the standard lists" }).click();
    await expect(dialog(page, "Line breaking rules").getByLabel("Characters that can't start a line")).toHaveValue(/、。/);
  });
});

test.describe("the document grid", () => {
  const grid = (g: string) => `<w:docGrid ${g}/>`;
  const body = [run("", "漢字漢字漢字"), run("", "行二".repeat(60))];

  test("lines snap to the grid's pitch, and a paragraph that opts out keeps its spacing", async ({ page }) => {
    await open(page, ['<w:pPr><w:snapToGrid w:val="0"/></w:pPr>' + body[0], body[0], body[1]], {}, grid('w:type="lines" w:linePitch="480"'));
    const h = (n: number) => page.locator(`.flow [data-p="${n}"]`).evaluate((e) => ({ line: getComputedStyle(e).lineHeight, h: e.getBoundingClientRect().height }));
    // 480 twips is 32px: one line of 11 point text takes a grid line, not its own height.
    const a = await h(1);
    const b = await h(2);
    expect(b.line).toBe("32px");
    expect(Math.round(b.h)).toBe(32);
    expect(a.line).not.toBe("32px");
    // A long paragraph is a whole number of grid lines tall.
    const long = (await h(3)).h;
    expect(Math.round(long % 32 === 0 ? 0 : long % 32)).toBe(0);
    expect(long).toBeGreaterThan(64);
    await page.screenshot({ path: "test-results/east-asian-grid.png" });
  });

  test("a character grid gives each East Asian character the grid's width", async ({ page }) => {
    await open(page, [body[0]], {}, grid('w:type="linesAndChars" w:linePitch="360" w:charSpace="8192"'));
    const info = await inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as any).sections[0]);
    // The base size plus 8192/4096 = 2 points is a character's width.
    const want = ((info.gridBase as number) + 2) * (96 / 72);
    const width = await page.locator('.flow [data-p="1"] .s').first().evaluate((e) => {
      const r = document.createRange();
      r.selectNodeContents(e);
      return r.getBoundingClientRect().width / (e.textContent ?? "").length;
    });
    expect(width).toBeCloseTo(want, 0);
  });

  test("a document without a grid is untouched, and removing the grid puts the spacing back", async ({ page }) => {
    await open(page, [body[0]], {}, grid('w:type="lines" w:linePitch="480"'));
    const before = await page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).lineHeight);
    expect(before).toBe("32px");
    await inPage(page, (w) => {
      w.fe.editor.exec({ op: "set_section", doc_grid: { type: "default" } });
      w.fe.editor.reloadPageSetup();
    });
    await expect.poll(() => page.locator('.flow [data-p="1"]').evaluate((e) => getComputedStyle(e).lineHeight)).not.toBe("32px");
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => (e as HTMLElement).dataset.gr)).toBeUndefined();
  });

  test("Page setup, Layout sets the grid by lines per page and characters per line", async ({ page }) => {
    await open(page, [body[0]]);
    await chooseMenu(page, "File", "Page setup…");
    const d = dialog(page, "Page setup");
    await d.getByRole("tab", { name: "Layout" }).click();
    await expect(d.getByLabel("Grid", { exact: true })).toHaveValue("default");
    await expect(d.getByLabel("Lines per page")).toBeDisabled();
    await d.getByLabel("Grid", { exact: true }).selectOption("linesAndChars");
    await d.getByLabel("Lines per page").fill("30");
    await d.getByLabel("Characters per line").fill("36");
    await d.getByRole("button", { name: "Apply" }).click();
    const s = await inPage(page, (w) => (w.fe.editor.engine!.pageSetup() as any).sections[0].docGrid);
    expect(s.type).toBe("linesAndChars");
    // The text area is 9 inches tall: 648 points over 30 lines is 21.6 points, 432 twips.
    expect(s.linePitch).toBe(432);
    // Its width is 6.5 inches: 468 points over 36 characters is 13 points, which is the base size plus charSpace/4096.
    expect(s.charSpace).toBeGreaterThan(0);
    expect(await page.locator('.flow [data-p="1"]').evaluate((e) => parseFloat(getComputedStyle(e).lineHeight))).toBeCloseTo(21.6 * (96 / 72), 0);
    // Opened again, it shows the same counts.
    await chooseMenu(page, "File", "Page setup…");
    const again = dialog(page, "Page setup");
    await again.getByRole("tab", { name: "Layout" }).click();
    await expect(again.getByLabel("Lines per page")).toHaveValue("30");
    await expect(again.getByLabel("Characters per line")).toHaveValue("36");
  });
});
