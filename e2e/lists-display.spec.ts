// Lists as Word shows them: numbering from list styles, restarts, every number format, label alignment and
// formatting, picture bullets, symbol bullets and the indent a list gets when a style and a level disagree.
import { expect, inPage, test, type Page } from "./helpers";
import { openBuilt, type DocSpec } from "./support/docx-builder";

const NUMPR = (num: number, lvl = 0) => `<w:numPr><w:ilvl w:val="${lvl}"/><w:numId w:val="${num}"/></w:numPr>`;
const lvl = (ilvl: number, fmt: string, text: string, extra = "", rpr = "", jc = "left", ind = `w:left="${720 * (ilvl + 1)}" w:hanging="360"`) =>
  `<w:lvl w:ilvl="${ilvl}"><w:start w:val="1"/><w:numFmt w:val="${fmt}"/>${extra}<w:lvlText w:val="${text}"/><w:lvlJc w:val="${jc}"/><w:pPr><w:ind ${ind}/></w:pPr>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}</w:lvl>`;
const item = (text: string, num: number, level = 0) => ({ text, ppr: NUMPR(num, level) });

/** The text of every label, in document order. */
const labels = (page: Page) => page.locator(".flow .fp .lbl").evaluateAll((els) => els.map((e) => (e as HTMLElement).innerText.replace(/ /g, " ")));

/** Open a document whose body is the given paragraphs. */
async function open(page: Page, spec: Partial<DocSpec> & { paragraphs: DocSpec["sections"][0]["paragraphs"] }) {
  const { paragraphs, ...rest } = spec;
  await openBuilt(page, { sections: [{ paragraphs }], ...rest });
}

test.describe("lists as Word shows them", () => {
  test("a list that takes its numbering from a list style shows numbers", async ({ page }) => {
    const numbering =
      `<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/><w:styleLink w:val="MyList"/>${lvl(0, "upperRoman", "%1.")}${lvl(1, "lowerLetter", "%2)")}</w:abstractNum>` +
      `<w:abstractNum w:abstractNumId="1"><w:multiLevelType w:val="multilevel"/><w:numStyleLink w:val="MyList"/></w:abstractNum>` +
      `<w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="0"/></w:num>`;
    const styles = `<w:style w:type="numbering" w:styleId="MyList"><w:name w:val="MyList"/><w:pPr><w:numPr><w:numId w:val="2"/></w:numPr></w:pPr></w:style>`;
    await open(page, { numbering, styles, paragraphs: [item("First topic", 1), item("A point", 1, 1), item("Second topic", 1), item("Third topic", 2)] });
    expect(await labels(page)).toEqual(["I.", "a)", "II.", "III."]);
    await expect(page.locator(".flow .fp").first()).toContainText("First topic");
  });

  test("a level that never restarts keeps counting, and one can restart only after a chosen level", async ({ page }) => {
    const numbering =
      `<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/>${lvl(0, "decimal", "%1.")}${lvl(1, "decimal", "%2.", '<w:lvlRestart w:val="0"/>')}${lvl(2, "decimal", "%3.", '<w:lvlRestart w:val="1"/>')}</w:abstractNum>` +
      `<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>`;
    await open(page, {
      numbering,
      paragraphs: [item("a", 1), item("b", 1, 1), item("c", 1, 2), item("d", 1, 1), item("e", 1, 2), item("f", 1), item("g", 1, 1), item("h", 1, 2)],
    });
    // Level 2 counts 1, 2, 3 across both top-level items; level 3 restarts only after a top-level item.
    expect(await labels(page)).toEqual(["1.", "1.", "1.", "2.", "2.", "2.", "3.", "1."]);
  });

  // The label of the first item in every number format Word writes.
  const FORMATS: [string, string][] = [
    ["decimal", "1"], ["decimalZero", "01"], ["upperRoman", "I"], ["lowerRoman", "i"], ["upperLetter", "A"], ["lowerLetter", "a"], ["ordinal", "1st"],
    ["cardinalText", "One"], ["ordinalText", "First"], ["hex", "1"], ["chicago", "*"], ["decimalEnclosedCircle", "①"], ["decimalEnclosedFullstop", "⒈"],
    ["decimalEnclosedParen", "⑴"], ["decimalFullWidth", "１"], ["decimalHalfWidth", "1"], ["japaneseCounting", "一"], ["japaneseDigitalTenThousand", "一"],
    ["aiueo", "ｱ"], ["iroha", "ｲ"], ["chineseCounting", "一"], ["chineseCountingThousand", "一"], ["chineseLegalSimplified", "壹"],
    ["ideographDigital", "一"], ["ideographTraditional", "甲"], ["ideographZodiac", "子"], ["koreanDigital", "일"], ["koreanCounting", "일"],
    ["ganada", "가"], ["chosung", "ㄱ"], ["hebrew1", "א"], ["hebrew2", "א"], ["arabicAlpha", "أ"], ["arabicAbjad", "أ"],
    ["hindiVowels", "अ"], ["hindiConsonants", "क"], ["hindiNumbers", "१"], ["thaiLetters", "ก"], ["thaiNumbers", "๑"],
    ["thaiCounting", "หนึ่ง"], ["vietnameseCounting", "Một"], ["russianLower", "а"], ["russianUpper", "А"], ["numberInDash", "- 1 -"],
  ];

  test("every number format Word writes shows its own numerals", async ({ page }) => {
    const numbering = FORMATS.map(([f], i) => `<w:abstractNum w:abstractNumId="${i}"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, f, "%1")}</w:abstractNum><w:num w:numId="${i + 1}"><w:abstractNumId w:val="${i}"/></w:num>`).join("");
    await open(page, { numbering, paragraphs: FORMATS.map(([f], i) => item(f, i + 1)) });
    const got = await labels(page);
    expect(got).toHaveLength(FORMATS.length);
    FORMATS.forEach(([format, want], i) => expect(got[i], format).toBe(want));
    // None of them is the plain 1, 2, 3 it used to print, except the ones that are.
    const plain = got.filter((g) => g === "1");
    expect(plain).toHaveLength(3);
  });

  test("the formats that spell the number out follow the document's language", async ({ page }) => {
    const numbering = `<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "cardinalText", "%1")}</w:abstractNum><w:abstractNum w:abstractNumId="1"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "ordinalText", "%1")}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>`;
    const paragraphs = [item("one", 1), item("two", 1), item("first", 2), item("second", 2)];
    await open(page, { numbering, paragraphs, lang: "es-ES" });
    expect(await labels(page)).toEqual(["Uno", "Dos", "Primero", "Segundo"]);
    await open(page, { numbering, paragraphs, lang: "de-DE" });
    expect(await labels(page)).toEqual(["Eins", "Zwei", "Erste", "Zweite"]);
    await open(page, { numbering, paragraphs, lang: "en-GB" });
    expect(await labels(page)).toEqual(["One", "Two", "First", "Second"]);
  });

  test("labels are aligned left, centre or right on the number position and carry the level's formatting", async ({ page }) => {
    // Left 1440, hanging 720: the number position is 720 twips (48px) into the text area.
    const ind = 'w:left="1440" w:hanging="720"';
    const numbering =
      `<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "lowerRoman", "%1", "", "", "left", ind)}</w:abstractNum>` +
      `<w:abstractNum w:abstractNumId="1"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "lowerRoman", "%1", "", "", "right", ind)}</w:abstractNum>` +
      `<w:abstractNum w:abstractNumId="2"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "lowerRoman", "%1", "", "", "center", ind)}</w:abstractNum>` +
      `<w:abstractNum w:abstractNumId="3"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "decimal", "%1.", "", '<w:i/><w:u w:val="double"/><w:strike/><w:highlight w:val="yellow"/><w:caps/><w:b/><w:color w:val="C00000"/>', "left", ind)}</w:abstractNum>` +
      [0, 1, 2, 3].map((i) => `<w:num w:numId="${i + 1}"><w:abstractNumId w:val="${i}"/></w:num>`).join("");
    const items = [1, 2, 3, 4].flatMap((n) => [item(`row ${n} a`, n), item(`row ${n} b`, n), item(`row ${n} c`, n)]);
    await open(page, { numbering, paragraphs: items });
    const geometry = async (text: string) =>
      page.locator(".flow .fp", { hasText: text }).evaluate((p) => {
        const r = p.getBoundingClientRect();
        const lbl = p.querySelector(".lbl")!;
        const target = (lbl.querySelector(".lbl-in") ?? lbl).getBoundingClientRect();
        const text = document.createRange();
        const node = Array.from(p.childNodes).find((n) => n.nodeType === Node.TEXT_NODE || (n as HTMLElement).dataset?.o !== undefined)!;
        text.selectNodeContents(node);
        return { paraLeft: r.left, labelLeft: target.left, labelRight: target.right, textLeft: text.getBoundingClientRect().left };
      });
    const left = await geometry("row 1 c");
    const right = await geometry("row 2 c");
    const center = await geometry("row 3 c");
    // Left: starts on the number position (720 twips = 48px), text starts at the indent (96px).
    expect(left.labelLeft - left.paraLeft).toBeCloseTo(48, 0);
    expect(left.textLeft - left.paraLeft).toBeCloseTo(96, 0);
    // Right: the label ends on the number position, and the text still starts at the indent.
    expect(right.labelRight - right.paraLeft).toBeCloseTo(48, 0);
    expect(right.textLeft - right.paraLeft).toBeCloseTo(96, 0);
    // Centre: the middle of the label is on the number position.
    expect((center.labelLeft + center.labelRight) / 2 - center.paraLeft).toBeCloseTo(48, 0);
    expect(center.textLeft - center.paraLeft).toBeCloseTo(96, 0);
    // The right-aligned numerals line up on their right edge: iii is wider than i, but both end together.
    const rightA = await geometry("row 2 a");
    expect(rightA.labelRight).toBeCloseTo(right.labelRight, 0);
    expect(right.labelLeft).toBeLessThan(rightA.labelLeft);
    // Formatting from the level's run properties.
    const styled = await page.locator(".flow .fp", { hasText: "row 4 a" }).locator(".lbl-in").evaluate((e) => {
      const c = getComputedStyle(e);
      return { fontStyle: c.fontStyle, deco: c.textDecorationLine, decoStyle: c.textDecorationStyle, bg: c.backgroundColor, caps: c.textTransform, weight: c.fontWeight, color: c.color };
    });
    expect(styled).toEqual({ fontStyle: "italic", deco: "underline line-through", decoStyle: "double", bg: "rgb(255, 255, 0)", caps: "uppercase", weight: "700", color: "rgb(192, 0, 0)" });
    await page.screenshot({ path: "test-results/lists-display-alignment.png", clip: { x: 0, y: 100, width: 900, height: 600 } });
  });

  test("symbol-font bullets look like the bullets they stand for", async ({ page }) => {
    const sym = (code: string, font: string) => `<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="${code}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr><w:rPr><w:rFonts w:ascii="${font}" w:hAnsi="${font}" w:hint="default"/></w:rPr></w:lvl>`;
    const defs: [string, string][] = [["&#xF0B7;", "Symbol"], ["&#xF0A7;", "Wingdings"], ["&#xF0D8;", "Wingdings"], ["&#xF0FC;", "Wingdings"], ["&#xF076;", "Wingdings"], ["l", "Wingdings"], ["&#xF0A8;", "Symbol"], ["o", "Courier New"]];
    const numbering = defs.map(([c, f], i) => `<w:abstractNum w:abstractNumId="${i}"><w:multiLevelType w:val="hybridMultilevel"/>${sym(c, f)}</w:abstractNum><w:num w:numId="${i + 1}"><w:abstractNumId w:val="${i}"/></w:num>`).join("");
    await open(page, { numbering, paragraphs: defs.map((_, i) => item(`bullet ${i}`, i + 1)) });
    expect(await labels(page)).toEqual(["•", "▪", "➢", "✔", "❖", "●", "♦", "◦"]);
  });

  test("a picture bullet is drawn as the picture at its size", async ({ page }) => {
    await open(page, { paragraphs: ["Alpha", "Beta"] });
    // A 2 by 2 red PNG, added to the document and used as a bullet.
    const png = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEklEQVR4nGP8z8Dwn4EIwDiqEAA/rQP/gdCAnQAAAABJRU5ErkJggg==";
    await inPage(
      page,
      (w, b64) => {
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
        const ed = w.fe.editor;
        const media = ed.engine!.addMedia("png", bytes);
        const ids = (ed.engine!.renderAll().blocks as { id: number }[]).map((b) => b.id);
        ed.exec({ op: "define_list", ids, def: { levels: [{ format: "bullet", text: "", picture: media, picture_size: [12, 12], left: 720, hanging: 360 }] } });
      },
      png,
    );
    const pics = page.locator(".flow .fp .lbl img.lbl-pic");
    await expect(pics).toHaveCount(2);
    const box = await pics.first().boundingBox();
    expect(box!.width).toBeCloseTo(16, 0);
    expect(box!.height).toBeCloseTo(16, 0);
    await expect(pics.first()).toHaveJSProperty("complete", true);
    expect(await pics.first().evaluate((i) => (i as HTMLImageElement).naturalWidth)).toBe(2);
    // The label has no text of its own, and the text of the paragraph starts after the picture.
    expect(await labels(page)).toEqual(["", ""]);
    await page.screenshot({ path: "test-results/lists-display-picture.png", clip: { x: 0, y: 100, width: 700, height: 300 } });
  });

  test("when a style and the numbering level both set the indent, the one Word uses wins", async ({ page }) => {
    const numbering = `<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/>${lvl(0, "decimal", "%1.")}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>`;
    const styles =
      `<w:style w:type="paragraph" w:styleId="Wide"><w:name w:val="Wide"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="2000"/></w:pPr></w:style>` +
      `<w:style w:type="paragraph" w:styleId="WideList"><w:name w:val="WideList"/><w:basedOn w:val="Normal"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr><w:ind w:left="2000" w:hanging="500"/></w:pPr></w:style>` +
      `<w:style w:type="paragraph" w:styleId="PlainList"><w:name w:val="PlainList"/><w:basedOn w:val="Wide"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr></w:style>`;
    await open(page, {
      numbering,
      styles,
      paragraphs: [
        { text: "direct", ppr: `<w:pStyle w:val="Wide"/>${NUMPR(1)}` },
        { text: "styled", ppr: '<w:pStyle w:val="WideList"/>' },
        { text: "inherited", ppr: '<w:pStyle w:val="PlainList"/>' },
        { text: "own indent", ppr: '<w:pStyle w:val="WideList"/><w:ind w:left="300"/>' },
      ],
    });
    const indent = (text: string) => page.locator(".flow .fp", { hasText: text }).evaluate((p) => ({ left: parseFloat(getComputedStyle(p).paddingLeft), first: parseFloat(getComputedStyle(p).textIndent) }));
    // Twips to px is 1/15.
    expect(await indent("direct")).toEqual({ left: 48, first: -24 });
    const styled = await indent("styled");
    expect(styled.left).toBeCloseTo(2000 / 15, 1);
    expect(styled.first).toBeCloseTo(-500 / 15, 1);
    expect(await indent("inherited")).toEqual({ left: 48, first: -24 });
    expect((await indent("own indent")).left).toBe(20);
  });
});
