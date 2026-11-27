// Fields as in Word, on screen: links written as fields, dates and document properties brought up to date, locked and
// dirty fields, update on open, page numbers and STYLEREF headings that follow the layout, field codes shown instead
// of results, and the odd elements (symbol fonts, date elements, soft hyphens, custom footnote marks, check boxes, EQ).
// The engine's side is tested in Rust (crates/docxtor-core/tests/all/fields.rs).
import { buildDocx, openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, expect, inPage, test, type Locator, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const run = (t: string) => `<w:r><w:t xml:space="preserve">${t}</w:t></w:r>`;
/** A field as Word writes it: begin, instruction, separate, result, end. */
const fld = (instr: string, result: string, begin = "") =>
  `<w:r><w:fldChar w:fldCharType="begin"${begin}/></w:r><w:r><w:instrText xml:space="preserve"> ${instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>${result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`;
const NOW = new Date();
const YEAR = String(NOW.getFullYear());

/** A one-section document of raw paragraphs (the last one is plain text, so the section break has somewhere to go). */
const doc = (raws: string[], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs: [...raws.map((raw) => ({ raw })), "The end"] }], ...extra });

const line = (page: Page, n: number): Locator => page.locator(".flow .fp").nth(n);
/** What a person reads in an element: the text shown, with a layout value or a field code in place of the stored text it hides. */
const text = async (el: Locator) =>
  (
    await el.evaluate((root) => {
      const shown = (n: Node): string => {
        if (n.nodeType === Node.TEXT_NODE) return (n as Text).data;
        if (n.nodeType !== Node.ELEMENT_NODE) return "";
        const e = n as HTMLElement;
        const cs = getComputedStyle(e);
        if (cs.display === "none" || e.classList.contains("lbl") || e.classList.contains("pgsp")) return "";
        if (e.classList.contains("fval") || e.classList.contains("fc-first")) {
          const pseudo = (w: string) => getComputedStyle(e, w).content.replace(/^"|"$/g, "").replace(/\\(.)/g, "$1");
          const c = pseudo("::before") !== "none" && e.classList.contains("fc-first") ? pseudo("::before") : pseudo("::after");
          return c === "none" ? "" : c;
        }
        return Array.from(e.childNodes).map(shown).join("");
      };
      return shown(root);
    })
  )
    .replace(/\s+/g, " ")
    .trim();
const shownText = (el: Locator) => expect.poll(() => text(el));

test.describe("fields", () => {
  test("a hyperlink written as a field is a link: clickable, with the link editor, and Ctrl/Cmd-click opens it", async ({ page }) => {
    await openBuilt(page, doc([run("See ") + fld('HYPERLINK "https://example.com/a"', "the site") + run(" now")]));
    const link = page.locator(".flow .lnk");
    await expect(link).toHaveText("the site");
    await expect(link).toHaveAttribute("data-href", "https://example.com/a");
    // Ctrl/Cmd-click opens it.
    await page.evaluate(() => {
      (window as unknown as { opened: string[] }).opened = [];
      window.open = ((u: string) => ((window as unknown as { opened: string[] }).opened.push(u), null)) as typeof window.open;
    });
    await link.click({ modifiers: [MOD] });
    expect(await page.evaluate(() => (window as unknown as { opened: string[] }).opened)).toEqual(["https://example.com/a"]);
    // The link editor shows the address, and a new one makes an ordinary link.
    await link.click();
    await chooseMenu(page, "Insert", "Link…");
    const input = page.locator(".popover input[type=url], .pop input[type=url]").first();
    await expect(input).toHaveValue("https://example.com/a");
    await input.fill("https://example.org/b");
    await input.press("Enter");
    await expect(page.locator(".flow .lnk")).toHaveAttribute("data-href", "https://example.org/b");
    const fields = await inPage(page, (w) => (w.fe.editor.engine as unknown as { listFields(): { code: string }[] }).listFields().map((f) => f.code));
    expect(fields).not.toContain("HYPERLINK");
  });

  test("a link field with a place in the document goes there", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: [{ raw: fld('HYPERLINK \\l "Target"', "Jump") }, ...Array.from({ length: 70 }, (_, i) => `Filler ${i}`), { raw: `<w:bookmarkStart w:id="1" w:name="Target"/>${run("Here it is")}<w:bookmarkEnd w:id="1"/>` }, "The end"] }],
    });
    const link = page.locator(".flow .lnk");
    await expect(link).toHaveAttribute("data-href", "#Target");
    await link.click({ modifiers: [MOD] });
    await expect(page.locator(".flow .fp", { hasText: "Here it is" })).toBeInViewport();
  });

  test("dates and times are brought up to date when the document opens; a locked one is not", async ({ page }) => {
    await openBuilt(
      page,
      doc([
        run("Today: ") + fld('DATE \\@ "yyyy"', "1999"),
        run("Locked: ") + fld('DATE \\@ "yyyy"', "1999", ' w:fldLock="1"'),
        run("Time: ") + fld('TIME \\@ "HH:mm"', "00:00"),
      ]),
    );
    await shownText(line(page, 0)).toBe(`Today: ${YEAR}`);
    await shownText(line(page, 1)).toBe("Locked: 1999");
    expect(await text(line(page, 2))).toMatch(/^Time: \d\d:\d\d$/);
    // Opening is not an edit: nothing to undo.
    expect(await inPage(page, (w) => (w.fe.editor.engine as unknown as { raw: { canUndo(): boolean } }).raw.canUndo())).toBe(false);
  });

  test("dirty fields and a file that asks to update fields are brought up to date as it opens", async ({ page }) => {
    const dirty = fld("TITLE", "stale", ' w:dirty="true"');
    await openBuilt(page, doc([run("A: ") + dirty, run("B: ") + fld("AUTHOR", "stale")], { props: { title: "Fresh title", creator: "Ada" } }));
    // Only the dirty one.
    await shownText(line(page, 0)).toBe("A: Fresh title");
    await shownText(line(page, 1)).toBe("B: stale");
    await openBuilt(page, doc([run("B: ") + fld("AUTHOR", "stale")], { props: { creator: "Ada" }, settings: '<w:updateFields w:val="true"/>' }));
    await shownText(line(page, 0)).toBe("B: Ada");
  });

  test("document properties, switches and formulas update with Update fields (F9)", async ({ page }) => {
    await openBuilt(
      page,
      doc(
        [
          run("1 ") + fld("AUTHOR \\* Upper", "x"),
          run("2 ") + fld("DOCPROPERTY Company", "x"),
          run("3 ") + fld('DOCPROPERTY "Project code"', "x"),
          run("4 ") + fld('= 6 * 7 \\# "0.00"', "0"),
          run("5 ") + fld('IF 3 > 2 "yes" "no"', "no"),
          run("6 ") + fld("SUBJECT", "x"),
          run("7 ") + fld("NUMWORDS", "0"),
          run("8 ") + fld('QUOTE "quoted"', "x"),
        ],
        {
          props: { creator: "Ada Lovelace", subject: "Engines" },
          app: "<Company>Analytical</Company>",
          custom: '<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Project code"><vt:lpwstr>Babbage</vt:lpwstr></property>',
        },
      ),
    );
    await page.keyboard.press("F9");
    await shownText(line(page, 0)).toBe("1 ADA LOVELACE");
    await shownText(line(page, 1)).toBe("2 Analytical");
    await shownText(line(page, 2)).toBe("3 Babbage");
    await shownText(line(page, 3)).toBe("4 42.00");
    await shownText(line(page, 4)).toBe("5 yes");
    await shownText(line(page, 5)).toBe("6 Engines");
    await shownText(line(page, 7)).toBe("8 quoted");
    expect(await text(line(page, 6))).toMatch(/^7 \d+$/);
  });

  test("page fields in the text follow the layout, and the page-dependent ones in a header follow their page", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          headers: { default: "", raw: fld('STYLEREF "Heading 1"', "x") } as never,
          paragraphs: [
            { text: "Alpha", ppr: '<w:pStyle w:val="Heading1"/>' },
            { raw: run("On page ") + fld("PAGE", "9") + run(" of ") + fld("NUMPAGES", "9") + run(" in section ") + fld("SECTION", "9") },
            { text: "Beta", ppr: '<w:pStyle w:val="Heading1"/><w:pageBreakBefore/>' },
            { raw: run("On page ") + fld("PAGE", "9") + run(" of ") + fld("NUMPAGES", "9") },
            "The end",
          ],
        },
      ],
    });
    await shownText(page.locator(".flow .fp", { hasText: "On page" }).first()).toBe("On page 1 of 2 in section 1");
    await shownText(page.locator(".flow .fp", { hasText: "On page" }).nth(1)).toBe("On page 2 of 2");
    // The header shows each page's own heading.
    await expect(page.locator(".sheet .hdr").nth(0)).toHaveText("Alpha");
    await expect(page.locator(".sheet .hdr").nth(1)).toHaveText("Beta");
    // The text keeps the stored result; only what is shown follows the layout.
    const stored = await page.locator(".flow .fp", { hasText: "On page" }).first().evaluate((e) => (e as HTMLElement).textContent);
    expect(stored).toContain("On page 9 of 9");
  });

  test("STYLEREF with a heading level shows that heading in a running head, and the chapter form in a caption keeps its number", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          // `STYLEREF 1` is Word's numeric form: the heading of level 1.
          headers: { default: "", raw: fld("STYLEREF 1", "x") } as never,
          paragraphs: [
            { text: "Alpha", ppr: '<w:pStyle w:val="Heading1"/>' },
            // A caption numbered by chapter: the chapter number comes from `STYLEREF 1 \s`.
            { raw: run("Figure ") + fld("STYLEREF 1 \\s", "7") + run("-") + fld("SEQ Figure \\* ARABIC \\s 1", "1") },
            { text: "Beta", ppr: '<w:pStyle w:val="Heading1"/><w:pageBreakBefore/>' },
            "The end",
          ],
        },
      ],
    });
    await expect(page.locator(".sheet .hdr").nth(0)).toHaveText("Alpha");
    await expect(page.locator(".sheet .hdr").nth(1)).toHaveText("Beta");
    // On screen the caption reads as the file has it, not as "Figure -1".
    await shownText(page.locator(".flow .fp", { hasText: "Figure" }).first()).toBe("Figure 7-1");
    await expect(page.locator(".flow .fval")).toHaveCount(0);
  });

  test("a heading in the last STYLEREF mode: \\l takes the last on the page, and a page without one takes the nearest before", async ({ page }) => {
    await openBuilt(page, {
      sections: [
        {
          headers: { default: "", raw: fld('STYLEREF "Heading 1" \\l', "x") } as never,
          paragraphs: [
            { text: "One", ppr: '<w:pStyle w:val="Heading1"/>' },
            { text: "Two", ppr: '<w:pStyle w:val="Heading1"/>' },
            { text: "Body", ppr: "<w:pageBreakBefore/>" },
            "The end",
          ],
        },
      ],
    });
    await expect(page.locator(".sheet .hdr").nth(0)).toHaveText("Two");
    await expect(page.locator(".sheet .hdr").nth(1)).toHaveText("Two");
  });

  test("field switches: number formats, case and pictures apply when fields are updated", async ({ page }) => {
    await openBuilt(page, doc([run("a ") + fld("NUMPAGES \\* ROMAN", "1"), run("b ") + fld("NUMPAGES \\* CardText \\* Caps", "1"), run("c ") + fld('NUMPAGES \\# "000"', "1"), run("d ") + fld('DATE \\@ "MMMM yyyy" \\* Upper', "x")]));
    await page.keyboard.press("F9");
    await shownText(line(page, 0)).toBe("a I");
    await shownText(line(page, 1)).toBe("b One");
    await shownText(line(page, 2)).toBe("c 001");
    expect(await text(line(page, 3))).toMatch(new RegExp(`^d [A-Z]+ ${YEAR}$`));
  });

  test("field codes are shown instead of results, can be edited, and the result follows", async ({ page }) => {
    await openBuilt(page, doc([run("Year: ") + fld('DATE \\@ "yyyy"', "1999", ' w:fldLock="1"')]));
    await shownText(line(page, 0)).toBe("Year: 1999");
    await page.keyboard.press("Alt+F9");
    const code = page.locator(".flow .fc-first");
    await expect(code).toHaveCount(1);
    expect(await code.evaluate((e) => (e as HTMLElement).dataset.code)).toBe('{ DATE \\@ "yyyy" }');
    await shownText(line(page, 0)).toBe('Year: { DATE \\@ "yyyy" }');
    // Back to results.
    await page.keyboard.press("Alt+F9");
    await expect(page.locator(".flow .fc-first")).toHaveCount(0);
    await shownText(line(page, 0)).toBe("Year: 1999");
  });
});

test.describe("odd elements", () => {
  test("Wingdings, Webdings and Symbol are drawn as look-alike characters", async ({ page }) => {
    const rpr = (f: string) => `<w:rPr><w:rFonts w:ascii="${f}" w:hAnsi="${f}"/></w:rPr>`;
    await openBuilt(
      page,
      doc([`<w:r>${rpr("Wingdings")}<w:t>JKL</w:t></w:r>` + `<w:r><w:sym w:font="Wingdings" w:char="F0FC"/></w:r>` + `<w:r>${rpr("Webdings")}<w:t>H</w:t></w:r>` + `<w:r>${rpr("Symbol")}<w:t>ab</w:t></w:r>`]),
    );
    expect(await text(line(page, 0))).toBe("\u263A\u{1F610}\u2639\u2714\u{1F3E0}\u03B1\u03B2");
    // Each one takes the place of the character it replaces: the caret can sit between them.
    expect(await page.locator(".flow .fp").first().locator(".atom.sym").count()).toBe(7);
    // The font of a symbol font is not asked of the page.
    const fonts = await page.locator(".flow .fp").first().locator(".atom.sym").evaluateAll((els) => els.map((e) => getComputedStyle(e).fontFamily));
    expect(fonts.every((f) => !/wingdings|webdings|symbol/i.test(f))).toBe(true);
  });

  test("page number and date elements are drawn as their values, not as a grey chip", async ({ page }) => {
    await openBuilt(page, doc([`<w:r><w:rPr/><w:pgNum/></w:r>` + run(" | ") + `<w:r><w:yearLong/></w:r>` + run(" | ") + `<w:r><w:monthLong/></w:r>`]));
    const l = line(page, 0);
    await expect(l.locator(".atom.obj")).toHaveCount(0);
    const month = new Intl.DateTimeFormat(undefined, { month: "long" }).format(NOW);
    await expect(l).toHaveText(`1 | ${YEAR} | ${month}`);
  });

  test("a soft hyphen shows a hyphen only where the line breaks", async ({ page }) => {
    await openBuilt(page, { sections: [{ size: [3000, 15840], margins: [1440, 720, 1440, 720], paragraphs: [{ raw: `<w:r><w:t>Short</w:t><w:t xml:space="preserve"> inter</w:t><w:softHyphen/><w:t>nationalization</w:t></w:r>` }, "The end"] }] });
    const l = line(page, 0);
    // The soft hyphen is a character of the text.
    expect(await l.evaluate((e) => e.textContent)).toContain("inter\u00ADnationalization");
    // The page is narrow enough that the word breaks at the hyphen: "inter" stays with "Short", the rest goes down.
    const measure = () =>
      l.evaluate((e) => {
        const n = document.createTreeWalker(e, NodeFilter.SHOW_TEXT).nextNode() as Text;
        const top = (from: number, to: number) => {
          const r = document.createRange();
          r.setStart(n, from);
          r.setEnd(n, to);
          // The last box: a hyphen shown at the end of the line is the first.
          const boxes = r.getClientRects();
          return boxes[boxes.length - 1]?.top ?? 0;
        };
        const i = n.data.indexOf("\u00AD");
        return { short: top(0, 5), inter: top(i - 5, i), rest: top(i + 1, i + 6) };
      });
    await expect.poll(async () => (await measure()).rest > (await measure()).short).toBe(true);
    const tops = await measure();
    expect(tops.inter).toBe(tops.short);
    expect(tops.rest).toBeGreaterThan(tops.short);
  });

  test("a footnote with a custom mark shows only that mark", async ({ page }) => {
    await openBuilt(page, doc([run("Text") + `<w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteReference w:customMarkFollows="1" w:id="5"/><w:t>*</w:t></w:r>`]));
    await shownText(line(page, 0)).toBe("Text*");
    await expect(line(page, 0).locator(".atom.note.custom")).toHaveCount(1);
  });

  test("legacy check boxes are drawn as boxes, checked or not; the paragraph text is unchanged", async ({ page }) => {
    const box = (checked: boolean) =>
      `<w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="c"/><w:enabled/><w:calcOnExit w:val="0"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/>${checked ? "<w:checked/>" : ""}</w:checkBox></w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> FORMCHECKBOX </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`;
    await openBuilt(page, doc([box(false) + run(" off   ") + box(true) + run(" on")]));
    const boxes = line(page, 0).locator(".ghost.ff");
    await expect(boxes).toHaveCount(2);
    await expect(boxes.nth(0)).toHaveAttribute("data-checked", "0");
    await expect(boxes.nth(1)).toHaveAttribute("data-checked", "1");
    const sizes = await boxes.evaluateAll((els) => els.map((e) => ({ w: e.getBoundingClientRect().width, tick: getComputedStyle(e, "::after").content })));
    expect(sizes[0].w).toBeGreaterThan(6);
    expect(sizes[0].tick).toBe("none");
    expect(sizes[1].tick).not.toBe("none");
  });

  test("an EQ field is drawn as an equation", async ({ page }) => {
    await openBuilt(page, doc([run("x = ") + `<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> EQ \\f(1,2) </w:instrText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`]));
    await expect(line(page, 0).locator(".ghost.eq math mfrac")).toHaveCount(1);
    const r = await line(page, 0).locator(".ghost.eq").boundingBox();
    expect(r!.height).toBeGreaterThan(15);
  });
});

test.describe("fields in a file", () => {
  test("the dialogs' file helpers build documents the engine accepts", () => {
    expect(buildDocx(doc([run("x")])).length).toBeGreaterThan(100);
  });
});
