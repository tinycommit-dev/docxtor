// How character formatting shows on the page: every underline style and colour, raised and lowered text, character
// scale, kerning, outline and the other effects, text borders, patterned and theme shading, theme colours and
// Word's automatic colour, hidden text (and showing it), emphasis marks, fit text and Word 2010's text effects.
// Checked in computed styles; the pictures were looked at by eye while writing these.
import { expect, LINE_START, paraText, test, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

const run = (rpr: string, text: string) => `<w:r>${rpr ? `<w:rPr>${rpr}</w:rPr>` : ""}<w:t xml:space="preserve">${text}</w:t></w:r>`;
const open = (page: Page, paragraphs: string[], extra: { styles?: string } = {}) => openBuilt(page, { sections: [{ paragraphs: paragraphs.map((raw) => ({ raw })) }], ...extra });

/** The computed style of the first piece of text in a paragraph. */
const spanStyle = (page: Page, id: number, props: string[], nth = 0) =>
  page.locator(`.flow [data-p="${id}"] .s`).nth(nth).evaluate((e, ps) => Object.fromEntries(ps.map((p) => [p, getComputedStyle(e).getPropertyValue(p)])), props);

test.describe("underline", () => {
  test("every style is drawn with the nearest CSS line, in the colour the file gives", async ({ page }) => {
    const styles: [string, string, boolean][] = [
      ["single", "solid", false],
      ["double", "double", false],
      ["thick", "solid", true],
      ["dotted", "dotted", false],
      ["dottedHeavy", "dotted", true],
      ["dash", "dashed", false],
      ["dashedHeavy", "dashed", true],
      ["dashLong", "dashed", false],
      ["dashLongHeavy", "dashed", true],
      ["dotDash", "dashed", false],
      ["dashDotHeavy", "dashed", true],
      ["dotDotDash", "dashed", false],
      ["dashDotDotHeavy", "dashed", true],
      ["wave", "wavy", false],
      ["wavyHeavy", "wavy", true],
      ["wavyDouble", "wavy", true],
    ];
    await open(page, styles.map(([u]) => run(`<w:u w:val="${u}" w:color="C00000"/>`, `text ${u}`)));
    const plain = await spanStyle(page, 1, ["text-decoration-thickness"]);
    for (const [i, [u, want, heavy]] of styles.entries()) {
      const s = await spanStyle(page, i + 1, ["text-decoration-line", "text-decoration-style", "text-decoration-color", "text-decoration-thickness"]);
      expect(s["text-decoration-line"], u).toContain("underline");
      expect(s["text-decoration-style"], u).toBe(want);
      expect(s["text-decoration-color"], u).toBe("rgb(192, 0, 0)");
      if (heavy) expect(parseFloat(s["text-decoration-thickness"]), u).toBeGreaterThanOrEqual(2);
      else expect(s["text-decoration-thickness"], u).toBe(plain["text-decoration-thickness"]);
    }
  });

  test("an underline with a theme colour takes it from the theme, and automatic follows the text", async ({ page }) => {
    await open(page, [
      run('<w:u w:val="single" w:color="000000" w:themeColor="accent2" w:themeShade="80"/>', "theme"),
      run('<w:color w:val="1F4E79"/><w:u w:val="single" w:color="auto"/>', "auto"),
    ]);
    // accent2 (ED7D31) at shade 80 (50% darker) is 843C0C, as in Word.
    expect((await spanStyle(page, 1, ["text-decoration-color"]))["text-decoration-color"]).toBe("rgb(132, 60, 12)");
    expect((await spanStyle(page, 2, ["text-decoration-color"]))["text-decoration-color"]).toBe("rgb(31, 78, 121)");
  });

  test("words only leaves the spaces out", async ({ page }) => {
    await open(page, [run('<w:u w:val="words"/>', "two  words here")]);
    const pieces = await page.locator('.flow [data-p="1"] .s').evaluateAll((els) =>
      els.map((e) => ({ text: e.textContent, line: getComputedStyle(e).textDecorationLine, o: (e as HTMLElement).dataset.o })),
    );
    expect(pieces.map((p) => p.text)).toEqual(["two", "  ", "words", " ", "here"]);
    expect(pieces.map((p) => p.line.includes("underline"))).toEqual([true, false, true, false, true]);
    expect(pieces.map((p) => p.o)).toEqual(["0", "3", "5", "10", "11"]);
    // The text is still one run to the person typing: the caret and selection map as usual.
    await page.locator('.flow [data-p="1"]').click();
    await page.keyboard.press(LINE_START);
    for (let i = 0; i < 6; i++) await page.keyboard.press("ArrowRight");
    await expect
      .poll(() => page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { sel: { focus: { o: number } } } } } }).docxtor.fe.editor.sel.focus.o))
      .toBe(6);
  });

  test("double strikethrough and an underline together keep both lines", async ({ page }) => {
    await open(page, [run('<w:dstrike/><w:u w:val="single"/>', "both"), run("<w:dstrike/>", "double")]);
    const both = await spanStyle(page, 1, ["text-decoration-line"]);
    expect(both["text-decoration-line"]).toContain("underline");
    expect(both["text-decoration-line"]).toContain("line-through");
    const dbl = await spanStyle(page, 2, ["text-decoration-line", "text-decoration-style"]);
    expect([dbl["text-decoration-line"], dbl["text-decoration-style"]]).toEqual(["line-through", "double"]);
  });
});

test.describe("position, scale and kerning", () => {
  test("raised and lowered text moves without changing the line", async ({ page }) => {
    await open(page, [run("", "plain line"), run('<w:position w:val="6"/>', "raised"), run('<w:position w:val="-6"/>', "lowered")]);
    const heights = await Promise.all([1, 2, 3].map((id) => page.locator(`.flow [data-p="${id}"]`).evaluate((e) => e.getBoundingClientRect().height)));
    // (Firefox lays a line out in 1/60 px, so equal lines can differ by a hundred-thousandth of a pixel in the value it reports.)
    expect(heights[1]).toBeCloseTo(heights[0], 2);
    expect(heights[2]).toBeCloseTo(heights[0], 2);
    const up = await spanStyle(page, 2, ["position", "top", "vertical-align"]);
    // Six half-points are three points (4px) up.
    expect([up.position, up.top, up["vertical-align"]]).toEqual(["relative", "-4px", "baseline"]);
    expect((await spanStyle(page, 3, ["top"])).top).toBe("4px");
  });

  test("scale stretches each word and keeps the line breaks where the browser puts them", async ({ page }) => {
    await open(page, [run("", "stretch me please"), run('<w:w w:val="150"/>', "stretch me please"), run('<w:w w:val="50"/>', "stretch me please")]);
    const widths = async (id: number) =>
      page.locator(`.flow [data-p="${id}"]`).evaluate((e) => {
        const r = document.createRange();
        r.selectNodeContents(e);
        const rect = r.getBoundingClientRect();
        return { w: rect.width, boxes: Array.from(e.querySelectorAll<HTMLElement>(".s")).map((s) => ({ text: s.textContent, tf: getComputedStyle(s).transform, display: getComputedStyle(s).display })) };
      });
    const [normal, wide, narrow] = [await widths(1), await widths(2), await widths(3)];
    expect(wide.w / normal.w).toBeGreaterThan(1.4);
    expect(wide.w / normal.w).toBeLessThan(1.6);
    expect(narrow.w / normal.w).toBeGreaterThan(0.4);
    expect(narrow.w / normal.w).toBeLessThan(0.6);
    // Words are boxes scaled across; the spaces between them are text.
    expect(wide.boxes.map((b) => b.text)).toEqual(["stretch", " ", "me", " ", "please"]);
    expect(wide.boxes[0].display).toBe("inline-block");
    expect(wide.boxes[0].tf).toBe("matrix(1.5, 0, 0, 1, 0, 0)");
    expect(wide.boxes[1].display).toBe("inline");
  });

  test("kerning turns on from the size the file gives", async ({ page }) => {
    await open(page, [run('<w:sz w:val="24"/><w:kern w:val="24"/>', "kerned"), run('<w:sz w:val="20"/><w:kern w:val="24"/>', "too small"), run('<w:kern w:val="0"/>', "off"), run("", "unset")]);
    expect((await spanStyle(page, 1, ["font-kerning"]))["font-kerning"]).toBe("normal");
    expect((await spanStyle(page, 2, ["font-kerning"]))["font-kerning"]).toBe("none");
    expect((await spanStyle(page, 3, ["font-kerning"]))["font-kerning"]).toBe("none");
    expect((await spanStyle(page, 4, ["font-kerning"]))["font-kerning"]).toBe("auto");
  });

  test("fit text spaces the letters to the width the file asks for", async ({ page }) => {
    // 1.5 inches wide (2160 twips = 144px), whatever the text is.
    await open(page, [run('<w:fitText w:val="2160" w:id="1"/>', "fit"), run('<w:fitText w:val="2160" w:id="2"/>', "a much longer piece")]);
    for (const id of [1, 2]) {
      const w = await page.locator(`.flow [data-p="${id}"] .s`).first().evaluate((e) => {
        const r = document.createRange();
        r.selectNodeContents(e);
        return r.getBoundingClientRect().width;
      });
      expect(Math.abs(w - 144), `paragraph ${id}`).toBeLessThan(6);
    }
  });
});

test.describe("effects", () => {
  test("outline is a hollow letter with a line, and the others are shadows", async ({ page }) => {
    await open(page, [run("<w:outline/>", "outline"), run("<w:shadow/>", "shadow"), run("<w:emboss/>", "emboss"), run("<w:imprint/>", "engrave"), run("", "plain")]);
    const props = ["-webkit-text-stroke-width", "color", "text-shadow"];
    const [outline, shadow, emboss, imprint, plain] = await Promise.all([1, 2, 3, 4, 5].map((id) => spanStyle(page, id, props)));
    expect(parseFloat(outline["-webkit-text-stroke-width"])).toBeGreaterThan(0);
    expect(outline.color).toBe("rgba(0, 0, 0, 0)");
    expect(shadow["text-shadow"]).not.toBe("none");
    expect(emboss["text-shadow"]).not.toBe("none");
    expect(imprint["text-shadow"]).not.toBe("none");
    expect(emboss["text-shadow"]).not.toBe(imprint["text-shadow"]);
    expect(plain["text-shadow"]).toBe("none");
    expect(plain["-webkit-text-stroke-width"]).toBe("0px");
  });

  test("emphasis marks sit above the letters, or below for the dot below", async ({ page }) => {
    await open(page, [run('<w:em w:val="dot"/>', "dot"), run('<w:em w:val="comma"/>', "comma"), run('<w:em w:val="circle"/>', "circle"), run('<w:em w:val="underDot"/>', "under")]);
    const props = ["text-emphasis-style", "text-emphasis-position"];
    const got = await Promise.all([1, 2, 3, 4].map((id) => spanStyle(page, id, props)));
    expect(got.map((g) => g["text-emphasis-style"])).toEqual(["dot", "sesame", "open circle", "dot"]);
    expect(got.map((g) => g["text-emphasis-position"])).toEqual(["over", "over", "over", "under"]);
  });

  test("Word 2010 effects: glow, shadow, outline, gradient fill and OpenType settings", async ({ page }) => {
    await open(page, [
      run('<w14:glow w14:rad="63500"><w14:srgbClr w14:val="FF0000"><w14:alpha w14:val="60000"/></w14:srgbClr></w14:glow>', "glow"),
      run('<w14:shadow w14:blurRad="50800" w14:dist="38100" w14:dir="2700000"><w14:srgbClr w14:val="000000"/></w14:shadow>', "shadow"),
      run('<w14:textOutline w14:w="19050"><w14:solidFill><w14:srgbClr w14:val="0000FF"/></w14:solidFill></w14:textOutline>', "outline"),
      run('<w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="FF0000"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="0000FF"/></w14:gs></w14:gsLst><w14:lin w14:ang="0"/></w14:gradFill></w14:textFill>', "gradient"),
      run('<w14:textFill><w14:solidFill><w14:srgbClr w14:val="00AA00"/></w14:solidFill></w14:textFill>', "solid"),
      run('<w14:ligatures w14:val="standardContextual"/><w14:numForm w14:val="oldStyle"/><w14:numSpacing w14:val="tabular"/><w14:stylisticSets><w14:styleSet w14:id="3"/></w14:stylisticSets><w14:cntxtAlts/>', "1234 fi"),
    ]);
    const glow = await spanStyle(page, 1, ["text-shadow"]);
    expect(glow["text-shadow"]).toContain("rgba(255, 0, 0, 0.6)");
    const shadow = await spanStyle(page, 2, ["text-shadow"]);
    // Three points at 45 degrees is 2.83px each way.
    expect(shadow["text-shadow"]).toMatch(/2\.83px 2\.83px 5\.33px/);
    const outline = await spanStyle(page, 3, ["-webkit-text-stroke-width", "-webkit-text-stroke-color"]);
    expect([outline["-webkit-text-stroke-width"], outline["-webkit-text-stroke-color"]]).toEqual(["2px", "rgb(0, 0, 255)"]);
    const grad = await spanStyle(page, 4, ["background-image", "-webkit-background-clip", "-webkit-text-fill-color"]);
    expect(grad["background-image"]).toContain("linear-gradient(90deg, rgb(255, 0, 0) 0%, rgb(0, 0, 255) 100%)");
    expect(grad["-webkit-background-clip"]).toBe("text");
    expect(grad["-webkit-text-fill-color"]).toBe("rgba(0, 0, 0, 0)");
    expect((await spanStyle(page, 5, ["color"])).color).toBe("rgb(0, 170, 0)");
    const ot = await spanStyle(page, 6, ["font-variant-ligatures", "font-variant-numeric", "font-feature-settings"]);
    expect(ot["font-variant-ligatures"]).toBe("common-ligatures contextual");
    expect(ot["font-variant-numeric"]).toBe("oldstyle-nums tabular-nums");
    expect(ot["font-feature-settings"]).toContain('"ss03"');
    expect(ot["font-feature-settings"]).toContain('"calt"');
  });
});

test.describe("borders, shading and colour", () => {
  test("neighbouring runs with the same border share one box", async ({ page }) => {
    const bdr = '<w:bdr w:val="single" w:sz="8" w:space="1" w:color="0000FF"/>';
    await open(page, [run(bdr, "one ") + run(`<w:b/>${bdr}`, "two ") + run(bdr, "three") + run("", " after")]);
    const sides = await page.locator('.flow [data-p="1"] .s').evaluateAll((els) =>
      els.map((e) => {
        const cs = getComputedStyle(e);
        return [cs.borderLeftStyle, cs.borderTopStyle, cs.borderBottomStyle, cs.borderRightStyle].map((s) => s === "solid");
      }),
    );
    // Top and bottom on every run; the left only on the first and the right only on the last.
    expect(sides[0]).toEqual([true, true, true, false]);
    expect(sides[1]).toEqual([false, true, true, false]);
    expect(sides[2]).toEqual([false, true, true, true]);
    expect(sides[3]).toEqual([false, false, false, false]);
  });

  test("shading with a pattern, or a theme colour, is drawn", async ({ page }) => {
    await open(page, [
      run('<w:shd w:val="pct25" w:color="FF0000" w:fill="FFFF00"/>', "pattern"),
      run('<w:shd w:val="clear" w:color="auto" w:fill="FFFFFF" w:themeFill="accent1" w:themeFillTint="33"/>', "theme"),
      run('<w:shd w:val="solid" w:color="00FF00" w:fill="auto"/>', "solid"),
    ]);
    const a = await spanStyle(page, 1, ["background-color", "background-image"]);
    expect(a["background-color"]).toBe("rgb(255, 255, 0)");
    expect(a["background-image"]).toContain("data:image/svg+xml");
    // accent1 (5B9BD5) at tint 33 is DEEBF6 here (Word rounds one step up: DEEBF7).
    expect((await spanStyle(page, 2, ["background-color"]))["background-color"]).toBe("rgb(222, 235, 246)");
    expect((await spanStyle(page, 3, ["background-color"]))["background-color"]).toBe("rgb(0, 255, 0)");
  });

  test("theme colours resolve from the theme, with tint and shade", async ({ page }) => {
    await open(page, [
      run('<w:color w:val="000000" w:themeColor="accent1" w:themeTint="99"/>', "lighter"),
      run('<w:color w:val="000000" w:themeColor="accent1" w:themeShade="BF"/>', "darker"),
      run('<w:color w:val="000000" w:themeColor="accent6"/>', "plain"),
    ]);
    // 5B9BD5 at tint 99 (60%): lightness mixed as Word does, 9CC3E5 (Word shows 9DC3E6).
    expect((await spanStyle(page, 1, ["color"])).color).toBe("rgb(156, 195, 229)");
    expect((await spanStyle(page, 2, ["color"])).color).toBe("rgb(46, 117, 181)");
    expect((await spanStyle(page, 3, ["color"])).color).toBe("rgb(112, 173, 71)");
  });

  test("automatic colour is white on a dark background and black otherwise", async ({ page }) => {
    await open(page, [
      run('<w:highlight w:val="black"/>', "on black"),
      run('<w:highlight w:val="yellow"/>', "on yellow"),
      run('<w:shd w:val="clear" w:color="auto" w:fill="1F2937"/>', "dark shading"),
      run('<w:color w:val="FF0000"/><w:highlight w:val="black"/>', "red on black"),
    ]);
    expect((await spanStyle(page, 1, ["color"])).color).toBe("rgb(255, 255, 255)");
    expect((await spanStyle(page, 2, ["color"])).color).toBe("rgb(0, 0, 0)");
    expect((await spanStyle(page, 3, ["color"])).color).toBe("rgb(255, 255, 255)");
    expect((await spanStyle(page, 4, ["color"])).color).toBe("rgb(255, 0, 0)");
  });

  test("automatic colour is white in a paragraph with a dark fill, but a colour the paragraph has stays", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          paragraphs: [
            { text: "dark paragraph", ppr: '<w:shd w:val="clear" w:color="auto" w:fill="000000"/>' },
            { text: "coloured on dark", ppr: '<w:shd w:val="clear" w:color="auto" w:fill="000000"/><w:rPr><w:color w:val="FFC000"/></w:rPr>' },
          ],
        },
      ],
    });
    const color = (id: number) => page.locator(`.flow [data-p="${id}"]`).evaluate((e) => getComputedStyle(e).color);
    expect(await color(1)).toBe("rgb(255, 255, 255)");
    expect(await color(2)).toBe("rgb(255, 192, 0)");
  });
});

test.describe("hidden text", () => {
  const hidden = [run("", "shown "), run("<w:vanish/>", "secret "), run("", "end")];

  test("is not drawn, and is drawn with a dotted underline when shown", async ({ page }) => {
    await open(page, [hidden.join(""), run("<w:vanish/>", "only hidden")]);
    const visible = (id: number) => page.locator(`.flow [data-p="${id}"] .s.hid`).evaluate((e) => ({ display: getComputedStyle(e).display, border: getComputedStyle(e).borderBottomStyle }));
    expect((await visible(1)).display).toBe("none");
    // A paragraph of hidden text only keeps its line.
    const h2 = await page.locator('.flow [data-p="2"]').evaluate((e) => e.getBoundingClientRect().height);
    expect(h2).toBeGreaterThan(8);
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { showHiddenText(on: boolean): void } } } }).docxtor.fe.editor.showHiddenText(true));
    expect(await visible(1)).toEqual({ display: "inline", border: "dotted" });
    await expect(page.locator('.flow [data-p="1"]')).toContainText("shown secret end");
  });

  test("View, Show hidden text turns it on, and so do non-printing characters", async ({ page }) => {
    await open(page, [hidden.join("")]);
    const shown = () => page.locator('.flow [data-p="1"] .s.hid').evaluate((e) => getComputedStyle(e).display !== "none");
    expect(await shown()).toBe(false);
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "View", exact: true }).click();
    await page.getByRole("menuitemcheckbox", { name: "Show hidden text" }).click();
    expect(await shown()).toBe(true);
    await page.getByRole("menubar", { name: "Menu bar" }).getByRole("menuitem", { name: "View", exact: true }).click();
    await page.getByRole("menuitemcheckbox", { name: "Show hidden text" }).click();
    expect(await shown()).toBe(false);
    // The non-printing characters bring hidden text back as well.
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { showMarks(on: boolean): void } } } }).docxtor.fe.editor.showMarks(true));
    expect(await shown()).toBe(true);
  });

  test("hidden text is left out of the printed pages unless it is shown", async ({ page }) => {
    await open(page, [hidden.join("")]);
    const printed = () =>
      page.evaluate(async () => {
        const ed = (window as unknown as { docxtor: { fe: { editor: unknown } } }).docxtor.fe.editor as { stageElement: HTMLElement };
        // A copy of the stage on a Docxtor root, as the printed pages and the PDF make it.
        const box = document.createElement("div");
        box.className = "docxtor docxtor-print";
        box.dataset.theme = "light";
        box.append(ed.stageElement.cloneNode(true));
        document.body.append(box);
        const text = Array.from(box.querySelectorAll<HTMLElement>(".fp .s")).filter((e) => getComputedStyle(e).display !== "none").map((e) => e.textContent).join("");
        box.remove();
        return text;
      });
    expect(await printed()).toBe("shown end");
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { showHiddenText(on: boolean): void } } } }).docxtor.fe.editor.showHiddenText(true));
    expect(await printed()).toBe("shown secret end");
  });
});

test("a sheet with every kind of character formatting is drawn (looked at by eye)", async ({ page }, testInfo) => {
  const bdr = '<w:bdr w:val="single" w:sz="8" w:space="1" w:color="0000FF"/>';
  const u = (v: string) => run(`<w:u w:val="${v}" w:color="C00000"/>`, `${v} `);
  await open(page, [
    ["single", "double", "thick", "dotted", "dash", "dotDash", "wave", "wavyHeavy"].map(u).join(""),
    run('<w:u w:val="words"/>', "words only underline ") + run('<w:position w:val="8"/>', "raised ") + run('<w:position w:val="-8"/>', "lowered ") + run('<w:w w:val="150"/>', "wide ") + run('<w:w w:val="60"/>', "narrow"),
    run("<w:outline/>", "outline ") + run("<w:shadow/>", "shadow ") + run("<w:emboss/>", "emboss ") + run("<w:imprint/>", "engrave ") + run(bdr, "boxed ") + run(bdr, "together"),
    run('<w:shd w:val="pct25" w:color="FF0000" w:fill="FFFF00"/>', "pattern ") + run('<w:color w:val="000000" w:themeColor="accent1" w:themeShade="BF"/>', "theme ") + run('<w:highlight w:val="black"/>', "auto on black ") + run('<w:em w:val="dot"/>', "emphasis ") + run('<w:fitText w:val="2160" w:id="9"/>', "fitted"),
    run('<w14:glow w14:rad="63500"><w14:srgbClr w14:val="FF0000"><w14:alpha w14:val="60000"/></w14:srgbClr></w14:glow>', "glow ") + run('<w14:textOutline w14:w="9525"><w14:solidFill><w14:srgbClr w14:val="0000FF"/></w14:solidFill></w14:textOutline>', "outline ") + run('<w:sz w:val="36"/><w14:textFill><w14:gradFill><w14:gsLst><w14:gs w14:pos="0"><w14:srgbClr w14:val="FF0000"/></w14:gs><w14:gs w14:pos="100000"><w14:srgbClr w14:val="0000FF"/></w14:gs></w14:gsLst><w14:lin w14:ang="0"/></w14:gradFill></w14:textFill>', "gradient"),
  ]);
  const shot = testInfo.outputPath("character-sheet.png");
  await page.locator(".sheets .sheet").first().screenshot({ path: shot });
  await testInfo.attach("character-sheet", { path: shot, contentType: "image/png" });
});

test.describe("typing around hidden text", () => {
  test("the caret and typing work next to text that isn't drawn, and the hidden text is kept", async ({ page }) => {
    await open(page, [run("", "shown ") + run("<w:vanish/>", "secret ") + run("", "end")]);
    const ed = (fn: string) => page.evaluate(`(${fn})(window.docxtor.fe.editor)`);
    await page.locator('.flow [data-p="1"]').click();
    // At the end, then at the start.
    await ed("(e) => e.select({ p: 1, o: 16 })");
    await page.keyboard.type("!");
    await ed("(e) => e.select({ p: 1, o: 0 })");
    await page.keyboard.type(">");
    await expect.poll(() => paraText(page, 1)).toBe(">shown secret end!");
    // The arrow keys skip the hidden word (it isn't there to step over), and typing lands before it.
    await ed("(e) => e.select({ p: 1, o: 1 })");
    for (let i = 0; i < 6; i++) await page.keyboard.press("ArrowRight");
    await page.keyboard.type("X");
    await expect.poll(() => paraText(page, 1)).toBe(">shown Xsecret end!");
    // The text typed is not hidden, and the hidden run is untouched.
    const hiddenRuns = await page.locator('.flow [data-p="1"] .s.hid').evaluateAll((els) => els.map((e) => e.textContent));
    expect(hiddenRuns).toEqual(["secret "]);
  });
});
