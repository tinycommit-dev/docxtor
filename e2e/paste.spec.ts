// Paste keeps formatting: from Word, Google Docs, web pages, RTF and other Docxtor documents; the Paste options
// button; Paste special; the default in Preferences. The clipboard is simulated with paste events carrying the
// formats each program writes, so the tests see exactly what the editor does with them.
import { openBuilt } from "./support/docx-builder";
import { GDOCS_HTML, RTF, WEB_HTML, WORD_HTML } from "./support/paste-samples";
import { caretAtEnd, chooseMenu, CHROMIUM_ONLY, dialog, expect, inPage, menuItem, openFixture, openMenu, para, paraText, pasteData, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** Paste these formats (MIME type to data) into the document, as the browser does for Ctrl+V. */
const paste = (page: Page, formats: Record<string, string>) => pasteData(page, formats);

/** The text of the document on screen, without the line break the engine ends it with. */
const text = async (page: Page) => (await inPage(page, (w) => w.fe.editor.engine!.text())).replace(/\n$/, "");
/** The formatting of a character, in the document on screen. */
const runAt = (page: Page, p: number, o: number) => inPage(page, (w, pos) => w.fe.editor.engine!.formatAt(pos), { p, o });
/** Every paragraph on screen (table cells included) with its id and text, in order. */
const paras = (page: Page) =>
  page.locator(".flow .fp").evaluateAll((els) =>
    els.map((el) => {
      const copy = el.cloneNode(true) as HTMLElement;
      copy.querySelectorAll(".lbl, .pgsp, .ghost").forEach((n) => n.remove());
      return { id: Number((el as HTMLElement).dataset.p), text: copy.textContent ?? "" };
    }),
  );
const lines = async (page: Page) => (await paras(page)).map((p) => p.text);
/** The id of the paragraph with exactly this text. */
const idOf = async (page: Page, line: string | RegExp) => {
  const hit = (await paras(page)).find((p) => (typeof line === "string" ? p.text === line : line.test(p.text)));
  if (!hit) throw new Error(`no paragraph reads ${line}`);
  return hit.id;
};
const chip = (page: Page) => page.locator(".paste-chip");
/** Paragraph ids on screen, in order. */
const ids = (page: Page) => page.locator(".flow .fp").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));

/** A document of these paragraphs with the caret at the end of one of them (the first by default). */
async function blank(page: Page, paragraphs = ["Start here"], at = 0) {
  await openBuilt(page, { sections: [{ paragraphs }] });
  const id = (await ids(page))[at];
  await caretAtEnd(page, id);
  return id;
}

test.describe("paste from Word", () => {
  test("keeps fonts, sizes, colours, highlight, emphasis, alignment, lists and tables", { tag: "@core" }, async ({ page }) => {
    const first = await blank(page, [""]);
    await paste(page, { "text/html": WORD_HTML, "text/plain": "Bold red Arial\nCentered and underlined" });
    await expect.poll(async () => (await lines(page)).includes("Bold red Arial")).toBe(true);
    const all = await ids(page);
    expect(await paraText(page, all[0])).toBe("Bold red Arial");
    const red = (await runAt(page, all[0], 2))!;
    expect(red.run.font).toBe("Arial");
    expect(red.run.size).toBe(14);
    expect(red.run.color).toBe("C00000");
    expect(red.run.bold).toBe(true);
    // The page shows it: computed styles of the first word.
    const css = await para(page, all[0]).evaluate((el) => {
      const s = getComputedStyle(el.querySelector("span, b, i, u") ?? el);
      return { family: s.fontFamily, color: s.color, weight: s.fontWeight };
    });
    expect(css.family).toContain("Arial");
    expect(css.color).toBe("rgb(192, 0, 0)");
    expect(Number(css.weight)).toBeGreaterThanOrEqual(600);
    // Second paragraph: centred, italic, underlined, highlighted, super- and subscript.
    const p2 = await idOf(page, /^Centered/);
    expect((await runAt(page, p2, 1))!.run.italic).toBe(true);
    const t2 = await paraText(page, p2);
    expect((await runAt(page, p2, t2.indexOf("underlined") + 2))!.run.underline).toBe("single");
    expect((await runAt(page, p2, 1))!.para.align).toBe("center");
    expect((await runAt(page, p2, t2.indexOf("highlighted") + 2))!.run.highlight).toBe("yellow");
    expect((await runAt(page, p2, t2.indexOf("x2") + 2))!.run.vertAlign).toBe("superscript");
    expect((await runAt(page, p2, t2.indexOf("H2O") + 2))!.run.vertAlign).toBe("subscript");
    // Bullets: real list paragraphs, without the hand-written dot.
    expect((await runAt(page, await idOf(page, "First bullet"), 1))!.list).toBe("bullet");
    // The table has its cells, shading and the spanned cell.
    expect(await page.locator(".flow table").count()).toBe(1);
    await expect(page.locator(".flow table td").first()).toHaveCSS("background-color", "rgb(217, 226, 243)");
    expect(await page.locator(".flow table tr").nth(1).locator("td").count()).toBe(1);
    // The link is a link; the picture Word names by file path can't be read, and the person is told.
    await expect(page.locator(".flow [data-href]", { hasText: "A link" })).toHaveAttribute("data-href", "https://example.com/page");
    await expect(page.locator(".toast")).toContainText("1 picture couldn't be read");
    void first;
  });

  test("pastes as one undo step", { tag: "@core" }, async ({ page }) => {
    const first = await blank(page, ["Before"]);
    const before = await text(page);
    await paste(page, { "text/html": WORD_HTML, "text/plain": "x" });
    await expect.poll(async () => (await text(page)).length).toBeGreaterThan(before.length + 20);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => text(page)).toBe(before);
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(async () => (await text(page)).includes("Spans both")).toBe(true);
    void first;
  });
});

test.describe("paste from Google Docs and web pages", () => {
  test("Google Docs: headings, character formatting, nested lists and tables", { tag: "@core" }, async ({ page }) => {
    await blank(page, [""]);
    await paste(page, { "text/html": GDOCS_HTML, "text/plain": "Big heading" });
    await expect.poll(async () => (await lines(page)).includes("Big heading")).toBe(true);
    const h = (await runAt(page, await idOf(page, "Big heading"), 1))!;
    expect(h.style).toBe("Heading1");
    // The wrapper's `font-weight: normal` leaves the text alone; the span's own weight applies.
    const bi = (await runAt(page, await idOf(page, "Bold italic blue on yellow"), 3))!;
    expect(bi.run.bold).toBe(true);
    expect(bi.run.italic).toBe(true);
    expect(bi.run.font).toBe("Georgia");
    expect(bi.run.color).toBe("1155CC");
    expect(bi.run.highlight).toBe("yellow");
    expect((await runAt(page, await idOf(page, "Item one"), 1))!.list).toBe("bullet");
    const nested = (await runAt(page, await idOf(page, "Nested"), 1))!;
    expect(nested.list).toBe("bullet");
    expect(nested.para.ilvl).toBe(1);
    const cells = page.locator(".flow table td");
    await expect(cells).toHaveCount(2);
    await expect(cells.first()).toHaveCSS("background-color", "rgb(207, 226, 243)");
  });

  test("a web page: heading style, link, inline code, quote and a picture from a data address", { tag: "@core" }, async ({ page }) => {
    await blank(page, [""]);
    await paste(page, { "text/html": WEB_HTML, "text/plain": "x" });
    await expect.poll(async () => (await lines(page)).includes("Section two")).toBe(true);
    const ls = await lines(page);
    expect((await runAt(page, await idOf(page, "Section two"), 3))!.style).toBe("Heading2");
    await expect(page.locator(".flow [data-href]", { hasText: "a link" })).toHaveAttribute("data-href", "https://example.com/x");
    expect(ls.some((l) => l.includes('code & "entities" — done.'))).toBe(true);
    const plainLine = ls.find((l) => l.startsWith("Plain paragraph"))!;
    expect((await runAt(page, await idOf(page, plainLine), plainLine.indexOf("code") + 1))!.run.font).toBe("Consolas");
    const img = page.locator(".flow img[alt='dot']");
    await expect(img).toHaveCount(1);
    expect(await img.evaluate((el) => (el as HTMLImageElement).width)).toBe(40);
  });

  test("inline text from a web page stays inside the paragraph it is pasted into", async ({ page }) => {
    const first = await blank(page, ["Hello world"]);
    await paste(page, { "text/html": '<span style="color:#ff0000;font-weight:700">red words</span>', "text/plain": "red words" });
    await expect.poll(() => paraText(page, first)).toBe("Hello worldred words");
    expect((await runAt(page, first, 16))!.run.color).toBe("FF0000");
    expect((await runAt(page, first, 3))!.run.color).toBeFalsy();
  });

  test("RTF alone is read when there is no HTML", { tag: "@core" }, async ({ page }) => {
    const first = await blank(page, [""]);
    await paste(page, { "text/rtf": RTF, "text/plain": "x" });
    await expect.poll(async () => (await text(page)).includes("Bold red Arial and plain")).toBe(true);
    const f = (await runAt(page, first, 2))!;
    expect(f.run.bold).toBe(true);
    expect(f.run.color).toBe("C00000");
    expect(f.run.font).toBe("Arial");
    expect(f.run.size).toBe(14);
    expect((await runAt(page, first, 18))!.run.bold).toBeFalsy();
  });
});

test.describe("Docxtor content", () => {
  test("copying a selection and pasting it keeps styles, lists, tables, links and pictures", { tag: "@core" }, async ({ page }) => {
    await openFixture(page);
    const before = await text(page);
    await page.locator(".flow").click({ position: { x: 10, y: 10 } });
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press(`${MOD}+C`);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press(`${MOD}+End`);
    await page.keyboard.press("Enter");
    await page.keyboard.press(`${MOD}+V`);
    await expect.poll(async () => (await text(page)).length).toBeGreaterThan(before.length * 1.8);
    const heading = await page.locator(".flow h1, .flow [data-p].h1, .flow .fp").first().textContent();
    expect(heading).toBeTruthy();
    // Twice the tables, twice the pictures.
    await expect(page.locator(".flow table")).toHaveCount(2);
    // Pictures load on their own time: wait for them.
    await expect.poll(() => page.locator(".flow img").count()).toBeGreaterThanOrEqual(2);
    // The title is still the Title style in the pasted copy.
    const styles = await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      const order = e.renderAll().order;
      return order.map((p) => e.formatAt({ p, o: 0 })?.style ?? "");
    });
    expect(styles.filter((s) => s === "Title").length).toBe(2);
  });

  test("copy puts Docxtor's format, HTML and text on the clipboard", { tag: "@core" }, async ({ page }) => {
    await blank(page, ["Alpha beta gamma"]);
    await page.evaluate(() => {
      (window as unknown as { __types: string[] }).__types = [];
      document.addEventListener("copy", (e) => ((window as unknown as { __types: string[] }).__types = Array.from(e.clipboardData?.types ?? [])));
    });
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press(`${MOD}+C`);
    await expect.poll(() => page.evaluate(() => (window as unknown as { __types: string[] }).__types)).toEqual(expect.arrayContaining(["text/plain", "text/html", "application/x-docxtor-fragment"]));
  });

  test("pasting Docxtor's format into another document maps styles and adds the ones it lacks", { tag: "@core" }, async ({ page }) => {
    await openFixture(page);
    const frag = await inPage(page, (w) => {
      const e = w.fe.editor.engine as unknown as { copyFragment(a: unknown, b: unknown): unknown; renderAll(): { order: number[] } };
      const order = e.renderAll().order;
      return JSON.stringify(e.copyFragment({ p: order[0], o: 0 }, { p: order[3], o: 5 }));
    });
    await blank(page, [""]);
    await paste(page, { "application/x-docxtor-fragment": frag, "text/plain": "x" });
    await expect.poll(async () => (await lines(page)).length).toBeGreaterThan(2);
    const styles = await inPage(page, (w) => {
      const e = w.fe.editor.engine!;
      return e.renderAll().order.map((p) => e.formatAt({ p, o: 0 })?.style ?? "");
    });
    expect(styles).toContain("Title");
    const known = await inPage(page, (w) => w.fe.editor.engine!.styles().map((s) => s.id));
    expect(known).toContain("Title");
  });
});

test.describe("a clipboard fragment is untrusted", () => {
  const W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
  const field = (instr: string, result: string) =>
    `<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> ${instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>${result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>`;

  test("a page's fragment keeps its words but loses dangerous fields, objects and scripted links", async ({ page }) => {
    await blank(page, [""]);
    const frag = JSON.stringify({
      version: 1,
      ns: [["w", W]],
      body: `<w:p><w:r><w:t>Hello </w:t></w:r>${field('INCLUDETEXT "C:\\secret.docx"', "shown")}<w:fldSimple w:instr=" DDEAUTO cmd calc "><w:r><w:t> simple</w:t></w:r></w:fldSimple><w:r><w:object><w:t>ole</w:t></w:object></w:r></w:p>`,
    });
    await paste(page, { "application/x-docxtor-fragment": frag, "text/plain": "x" });
    await expect.poll(() => text(page)).toContain("Hello shown simple");
    // No field is left: the dangerous ones kept only their result text.
    expect(await inPage(page, (w) => JSON.stringify(w.fe.editor.engine!.listFields()))).not.toMatch(/INCLUDETEXT|DDEAUTO/);
    expect(await text(page)).not.toContain("ole");
  });

  test("a link to a file copied inside Docxtor pastes back as a link; the same link from any other clipboard does not", async ({ page }) => {
    const url = "file://server/share/Q3 report.docx";
    await blank(page, ["Open the report.", "", "", ""], 1);
    const frag = await inPage(
      page,
      (w, u) => {
        const e = w.fe.editor;
        const [a] = e.engine!.renderAll().order;
        e.exec({ op: "set_link", from: { p: a, o: 0 }, to: { p: a, o: 4 }, url: u });
        return JSON.stringify(e.engine!.copyFragment({ p: a, o: 0 }, { p: a, o: 16 }));
      },
      url,
    );
    // Docxtor marks what it copies with a secret that only this page load knows.
    expect(JSON.parse(frag).origin).toMatch(/^[0-9a-f]{48}$/);
    const linkOf = async (i: number) => inPage(page, (w, n) => w.fe.editor.engine!.formatAt({ p: w.fe.editor.engine!.renderAll().order[n], o: 2 })?.link ?? null, i);
    await caretAtEnd(page, (await ids(page))[1]);
    await paste(page, { "application/x-docxtor-fragment": frag, "text/plain": "x" });
    await expect.poll(async () => (await lines(page)).filter((l) => l.includes("Open the report.")).length).toBe(2);
    expect(await linkOf(1)).toBe(url);
    // Someone else's fragment (no secret, or a guess) keeps the words and loses the file link.
    for (const [i, origin] of [undefined, "0".repeat(48)].entries()) {
      await caretAtEnd(page, (await ids(page))[2 + i]);
      await paste(page, { "application/x-docxtor-fragment": JSON.stringify({ ...JSON.parse(frag), origin }), "text/plain": "x" });
      await expect.poll(async () => (await lines(page)).filter((l) => l.includes("Open the report.")).length).toBe(3 + i);
      expect(await linkOf(2 + i)).toBeNull();
    }
  });

  test("a malformed fragment is ignored and the plain text is pasted", async ({ page }) => {
    await blank(page, [""]);
    await paste(page, { "application/x-docxtor-fragment": JSON.stringify({ version: 1, body: "<w:p/>", media: [], ns: "w" }), "text/plain": "plain words" });
    await expect.poll(() => text(page)).toContain("plain words");
  });
});

test.describe("Paste options", () => {
  for (const scheme of ["light", "dark"] as const) {
    test(`the button stays visible under the pointer (${scheme})`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await blank(page, ["Start", ""], 1);
      await paste(page, { "text/html": WORD_HTML, "text/plain": "x" });
      await expect(chip(page)).toBeVisible();
      await chip(page).hover();
      await expect(chip(page)).toBeVisible();
      // The hover tint sits on the button's own solid background, so the icon keeps its contrast over the white page.
      const look = await chip(page).evaluate((b) => {
        const cs = getComputedStyle(b);
        const rgb = (v: string) => (v.match(/[\d.]+/g) ?? []).map(Number);
        const lum = ([r, g, b]: number[]) => {
          const f = (c: number) => ((c /= 255) <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
          return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
        };
        const bg = rgb(cs.backgroundColor);
        const [a, c] = [lum(bg), lum(rgb(cs.color))];
        return { alpha: bg[3] ?? 1, gradient: cs.backgroundImage.includes("gradient"), contrast: (Math.max(a, c) + 0.05) / (Math.min(a, c) + 0.05) };
      });
      expect(look.alpha).toBe(1);
      expect(look.gradient).toBe(true);
      expect(look.contrast).toBeGreaterThan(4.5);
    });
  }

  test("a button appears next to a rich paste; Merge formatting replaces the paste with one that takes the destination's look", async ({ page }) => {
    const first = await blank(page, ["Start", ""], 1);
    const original = await text(page);
    await paste(page, { "text/html": WORD_HTML, "text/plain": "x" });
    await expect(chip(page)).toBeVisible();
    // Fonts and colours as they were.
    const all = await ids(page);
    expect((await runAt(page, all[1], 2))!.run.color).toBe("C00000");
    await chip(page).click();
    await page.locator(".paste-menu .menu-item", { hasText: "Merge formatting" }).click();
    await expect.poll(async () => (await runAt(page, (await ids(page))[1], 2))!.run.color).toBeFalsy();
    const merged = (await runAt(page, (await ids(page))[1], 2))!;
    expect(merged.run.font).not.toBe("Arial");
    // Bold and lists stay.
    expect(merged.run.bold).toBe(true);
    expect((await runAt(page, await idOf(page, "First bullet"), 1))!.list).toBe("bullet");
    // One undo takes the whole paste away: the first paste was replaced, not added to.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => text(page)).toBe(original);
    void first;
  });

  test("Keep text only pastes the words alone; Keep source formatting brings everything back", { tag: "@core" }, async ({ page }) => {
    await blank(page, ["Start", ""], 1);
    await paste(page, { "text/html": WORD_HTML, "text/plain": "Bold red Arial\nCentered and underlined" });
    await chip(page).click();
    await page.locator(".paste-menu .menu-item", { hasText: "Keep text only" }).click();
    await expect.poll(() => text(page)).toContain("Bold red Arial");
    await expect(page.locator(".flow table")).toHaveCount(0);
    await chip(page).click();
    await expect(page.locator(".paste-menu .menu-item.active")).toHaveText("Keep text only");
    await page.locator(".paste-menu .menu-item", { hasText: "Keep source formatting" }).click();
    await expect(page.locator(".flow table")).toHaveCount(1);
  });

  test("the button goes away when you type or press Escape", async ({ page }) => {
    await blank(page, ["Start"]);
    await paste(page, { "text/html": WORD_HTML, "text/plain": "x" });
    await expect(chip(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(chip(page)).toHaveCount(0);
    await paste(page, { "text/html": WEB_HTML, "text/plain": "x" });
    await expect(chip(page)).toBeVisible();
    await page.keyboard.type("z");
    await expect(chip(page)).toHaveCount(0);
  });

  test("a plain text paste has no options", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await blank(page, ["Start"]);
    await paste(page, { "text/plain": " and more" });
    await expect.poll(() => text(page)).toBe("Start and more");
    await expect(chip(page)).toHaveCount(0);
  });
});

test.describe("Paste special and preferences", () => {
  async function writeClipboard(page: Page, html: string, plain: string) {
    await page.evaluate(
      async ([h, p]) => {
        await navigator.clipboard.write([new ClipboardItem({ "text/html": new Blob([h], { type: "text/html" }), "text/plain": new Blob([p], { type: "text/plain" }) })]);
      },
      [html, plain] as const,
    );
  }

  test("Edit, Paste special lists the formats and pastes with the chosen formatting", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await blank(page, ["Start"]);
    await writeClipboard(page, '<p><b style="color:#c00000">Red bold</b> text</p>', "Red bold text");
    await chooseMenu(page, "Edit", "Paste special…");
    const d = dialog(page, "Paste special");
    await expect(d.getByLabel("Formatted text (HTML)")).toBeChecked();
    await expect(d.getByLabel("Plain text")).toBeVisible();
    await d.getByLabel("Merge formatting").check();
    await d.getByRole("button", { name: "Paste" }).click();
    await expect.poll(() => text(page)).toContain("Red bold text");
    const first = (await ids(page))[0];
    expect((await runAt(page, first, 7))!.run.bold).toBe(true);
    expect((await runAt(page, first, 7))!.run.color).toBeFalsy();
  });

  test("Paste special can paste the text as Markdown", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    await blank(page, [""]);
    await writeClipboard(page, "<p>**not bold**</p>", "**bold** and *italic*");
    await chooseMenu(page, "Edit", "Paste special…");
    const d = dialog(page, "Paste special");
    await d.getByLabel("Text read as Markdown").check();
    await d.getByRole("button", { name: "Paste" }).click();
    await expect.poll(() => text(page)).toBe("bold and italic");
    expect((await runAt(page, (await ids(page))[0], 2))!.run.bold).toBe(true);
  });

  test("Preferences sets what a plain paste does", async ({ page }) => {
    await blank(page, ["Start"]);
    await chooseMenu(page, "Tools", "Preferences…");
    const d = dialog(page, "Preferences");
    await d.getByLabel("When you paste").selectOption("text");
    await d.getByRole("button", { name: "Apply" }).click();
    await paste(page, { "text/html": WORD_HTML, "text/plain": "Plain words" });
    await expect.poll(() => text(page)).toContain("Plain words");
    await expect(page.locator(".flow table")).toHaveCount(0);
    // And it is remembered.
    await chooseMenu(page, "Tools", "Preferences…");
    await expect(dialog(page, "Preferences").getByLabel("When you paste")).toHaveValue("text");
  });

  test("Ctrl+Shift+V pastes text only", async ({ page }) => {
    await blank(page, [""]);
    // The key sets the mode; the browser then sends the paste.
    await page.evaluate(() => {
      const mac = navigator.platform.toLowerCase().includes("mac");
      document.querySelector(".flow")!.dispatchEvent(new KeyboardEvent("keydown", { key: "V", code: "KeyV", shiftKey: true, ctrlKey: !mac, metaKey: mac, bubbles: true, cancelable: true }));
    });
    await paste(page, { "text/html": '<p><b style="color:#c00000">Red bold</b></p>', "text/plain": "Red bold" });
    await expect.poll(() => text(page)).toBe("Red bold");
    expect((await runAt(page, (await ids(page))[0], 2))!.run.bold).toBeFalsy();
    expect((await runAt(page, (await ids(page))[0], 2))!.run.color).toBeFalsy();
  });
});

test.describe("converters are a contribution point", () => {
  test("an extension reads its own clipboard format", async ({ page }) => {
    await blank(page, [""]);
    await inPage(page, async (w) => {
      const fe = w.fe as unknown as { use(ext: unknown): Promise<void> };
      await fe.use({
        id: "test.shout",
        name: "Shout",
        activate(ctx: { clipboard: { converters: { register(c: unknown): void } } }) {
          ctx.clipboard.converters.register({ id: "test.shout", label: "Shout", mime: "application/x-shout", priority: 200, convert: ({ data }: { data: string }) => ({ markdown: `**${data.toUpperCase()}**` }) });
        },
      });
    });
    await paste(page, { "application/x-shout": "hello there", "text/plain": "ignored" });
    await expect.poll(() => text(page)).toBe("HELLO THERE");
    expect((await runAt(page, (await ids(page))[0], 2))!.run.bold).toBe(true);
    // Turned off, its format is left alone and the plain text is used.
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("test.shout"));
    await paste(page, { "application/x-shout": "again", "text/plain": " plain" });
    await expect.poll(() => text(page)).toBe("HELLO THERE plain");
  });
});

test("the menu keeps Paste special near Paste", async ({ page }) => {
  await openFixture(page);
  await openMenu(page, "Edit");
  await expect(menuItem(page, "Paste special…")).toBeVisible();
});
