// Tools, Preferences and the typing substitutions: smart quotes, capital letters, links, Markdown
// shortcuts and symbols such as (c) to ©, each on and off; undo and Backspace take a substitution
// back; nothing changes while composing, pasting or typing in code.
import { openBuilt } from "./support/docx-builder";
import { CHROMIUM_ONLY, chooseMenu, dialog, documentReady, expect, inPage, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const KEY = "docxtor.ext.docxtor.editing";

interface Prefs {
  smartQuotes: boolean;
  capitalize: boolean;
  links: boolean;
  markdown: boolean;
  substitutions: boolean;
}
const LABELS: Record<keyof Prefs, string> = {
  smartQuotes: "Use smart quotes",
  capitalize: "Automatically capitalize words",
  links: "Automatically detect links",
  markdown: "Use Markdown shortcuts",
  substitutions: "Use substitutions",
};
/** Only the substitution under test is on, unless the test says otherwise. */
const ONLY = (on: keyof Prefs): Prefs => ({ smartQuotes: false, capitalize: false, links: false, markdown: false, substitutions: false, [on]: true });

/** Start with these preferences (as if saved in an earlier session), unless some are saved already. */
async function withPrefs(page: Page, prefs: Partial<Prefs>) {
  await page.addInitScript(
    ([key, value]) => {
      if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify({ autoformat: value }));
    },
    [KEY, prefs] as const,
  );
}

/** A new blank document with the caret in its empty paragraph. */
async function blank(page: Page, prefs?: Partial<Prefs>) {
  if (prefs) await withPrefs(page, prefs);
  await page.goto("/");
  await page.locator(".doc-tile.new").click();
  await documentReady(page);
  await focusFirst(page);
}

async function focusFirst(page: Page) {
  await page.locator(".flow .fp").first().click();
  await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
}

/** The text of every paragraph, in order (list labels and spacers left out). */
const lines = (page: Page): Promise<string[]> =>
  page.evaluate(() =>
    Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")).map((el) => {
      const copy = el.cloneNode(true) as HTMLElement;
      copy.querySelectorAll(".lbl, .pgsp, .ghost").forEach((n) => n.remove());
      return copy.textContent ?? "";
    }),
  );
/** The text of the first paragraph. */
const line = async (page: Page) => (await lines(page))[0];

/** What the model says about paragraph number `n` (from 0) at an offset. */
const stateAt = (page: Page, n: number, o = 0) =>
  inPage(
    page,
    (w, a) => {
      const ids = Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")).map((el) => Number(el.dataset.p));
      return w.fe.editor.engine!.formatAt({ p: ids[a.n], o: a.o });
    },
    { n, o },
  );

const type = (page: Page, text: string) => page.keyboard.type(text);

async function choose(page: Page, change: Partial<Prefs>) {
  await chooseMenu(page, "Tools", "Preferences…");
  const d = dialog(page, "Preferences");
  await expect(d).toBeVisible();
  for (const [k, v] of Object.entries(change)) await d.getByLabel(LABELS[k as keyof Prefs]).setChecked(v);
  await d.getByRole("button", { name: "Apply" }).click();
  await expect(d).toBeHidden();
}

test.describe("preferences", () => {
  test("Tools, Preferences lists the choices, all on by default, and the document gets focus back", async ({ page }) => {
    await blank(page);
    await chooseMenu(page, "Tools", "Preferences…");
    const d = dialog(page, "Preferences");
    await expect(d).toBeVisible();
    for (const label of Object.values(LABELS)) await expect(d.getByLabel(label)).toBeChecked();
    await expect(d.locator(".pref-sub:not(.head)")).toHaveCount(15);
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(d).toBeHidden();
    await expect(page.getByRole("textbox", { name: "Document", exact: true })).toBeFocused();
  });

  test("Cancel keeps the old choices; Apply is remembered after a reload", async ({ page }) => {
    await blank(page);
    await chooseMenu(page, "Tools", "Preferences…");
    const d = dialog(page, "Preferences");
    await d.getByLabel(LABELS.smartQuotes).uncheck();
    await d.getByRole("button", { name: "Cancel" }).click();
    await type(page, '"a"');
    await expect.poll(() => line(page)).toBe("“a”");

    await choose(page, { smartQuotes: false, markdown: false });
    await page.reload();
    await documentReady(page);
    await chooseMenu(page, "Tools", "Preferences…");
    const again = dialog(page, "Preferences");
    await expect(again.getByLabel(LABELS.smartQuotes)).not.toBeChecked();
    await expect(again.getByLabel(LABELS.markdown)).not.toBeChecked();
    await expect(again.getByLabel(LABELS.links)).toBeChecked();
    await again.getByRole("button", { name: "Cancel" }).click();
    // And they are in force in the reopened document.
    await focusFirst(page);
    await page.keyboard.press(`${MOD}+End`);
    await type(page, '"b"');
    await expect.poll(async () => (await lines(page)).join("").endsWith('"b"')).toBe(true);
  });

  test("the Preferences line is in the Tools menu", async ({ page }) => {
    await blank(page);
    await chooseMenu(page, "Tools", "Preferences…");
    await expect(dialog(page, "Preferences")).toBeVisible();
  });
});

test.describe("smart quotes", () => {
  test.beforeEach(async ({ page }) => blank(page, ONLY("smartQuotes")));

  test("double and single quotes open after a space or at the start and close otherwise; apostrophes close", async ({ page }) => {
    await type(page, `He said "it's 'fine'" (and "ok")`);
    await expect.poll(() => line(page)).toBe("He said “it’s ‘fine’” (and “ok”)");
  });

  test("a quote typed at the start of a paragraph opens, and after a dash", async ({ page }) => {
    await type(page, '"one"');
    await page.keyboard.press("Enter");
    await type(page, "and—\"two\"");
    await expect.poll(() => lines(page)).toEqual(["“one”", "and—“two”"]);
  });

  test("off: quotes stay straight", async ({ page }) => {
    await choose(page, { smartQuotes: false });
    await type(page, `"it's"`);
    await expect.poll(() => line(page)).toBe(`"it's"`);
  });

  test("a German document gets „ “ and ‚ ‘", async ({ page }) => {
    await openBuilt(page, { lang: "de-DE", sections: [{ paragraphs: [""] }] });
    await focusFirst(page);
    await type(page, `Er sagte "ja" und 'nein'`);
    await expect.poll(() => line(page)).toBe("Er sagte „ja“ und ‚nein‘");
  });

  test("a French document gets « » with a narrow no-break space inside", async ({ page }) => {
    await openBuilt(page, { lang: "fr-FR", sections: [{ paragraphs: [""] }] });
    await focusFirst(page);
    await type(page, `Il dit "oui"`);
    await expect.poll(() => line(page)).toBe("Il dit « oui »");
  });

  test("a Spanish document keeps “ ”", async ({ page }) => {
    await openBuilt(page, { lang: "es-ES", sections: [{ paragraphs: [""] }] });
    await focusFirst(page);
    await type(page, `Dijo "sí"`);
    await expect.poll(() => line(page)).toBe("Dijo “sí”");
  });
});

test.describe("capital letters", () => {
  test.beforeEach(async ({ page }) => blank(page, ONLY("capitalize")));

  test("the first word of a paragraph and of each sentence, when the word is finished", async ({ page }) => {
    await type(page, "hello there. this is fine! really? yes, it is");
    await expect.poll(() => line(page)).toBe("Hello there. This is fine! Really? Yes, it is");
  });

  test('a lone "i" and its contractions', async ({ page }) => {
    await type(page, "Then i said i'm sure that i'll go, and i ");
    await expect.poll(() => line(page)).toBe("Then I said I'm sure that I'll go, and I ");
  });

  test("not after an abbreviation, an initial or a number; not inside a URL or a mixed-case word", async ({ page }) => {
    await type(page, "Use fruit, e.g. apples, i.e. red ones, etc. and mr. smith; see J. smith and 1. item at example.com/page and iPhone ");
    await expect.poll(() => line(page)).toBe("Use fruit, e.g. apples, i.e. red ones, etc. and mr. smith; see J. smith and 1. item at example.com/page and iPhone ");
  });

  test("Enter finishes the last word", async ({ page }) => {
    await type(page, "yes");
    await page.keyboard.press("Enter");
    await type(page, "no");
    await expect.poll(() => lines(page)).toEqual(["Yes", "no"]);
  });

  test("off: nothing is capitalised", async ({ page }) => {
    await choose(page, { capitalize: false });
    await type(page, "hello. i think so ");
    await expect.poll(() => line(page)).toBe("hello. i think so ");
  });
});

test.describe("links", () => {
  test.beforeEach(async ({ page }) => blank(page, ONLY("links")));

  const linkAt = async (page: Page, o: number) => (await stateAt(page, 0, o))?.link;

  test("an address becomes a link when Space follows it", async ({ page }) => {
    // "see " is 4 characters; the address starts at offset 4.
    await type(page, "see https://example.com/page now");
    expect(await linkAt(page, 8)).toBe("https://example.com/page");
    expect(await linkAt(page, 2)).toBeFalsy();
    // " now" is not part of it.
    expect(await linkAt(page, 4 + "https://example.com/page".length + 3)).toBeFalsy();
    await expect.poll(() => line(page)).toBe("see https://example.com/page now");
  });

  test("www. addresses link to https, and an e-mail address to mailto, leaving the final full stop out", async ({ page }) => {
    await type(page, "www.example.org, then me@example.com. ");
    expect(await linkAt(page, 3)).toBe("https://www.example.org");
    // "www.example.org" is 15 characters; the comma after it is the 16th.
    expect(await linkAt(page, 16)).toBeFalsy();
    expect(await linkAt(page, 24)).toBe("mailto:me@example.com");
    // "me@example.com" fills characters 23 to 36; the full stop is the 37th.
    expect(await linkAt(page, 37)).toBeFalsy();
  });

  test("Enter after an address links it too", async ({ page }) => {
    await type(page, "http://example.net");
    await page.keyboard.press("Enter");
    expect(await linkAt(page, 5)).toBe("http://example.net");
    await expect.poll(() => lines(page)).toEqual(["http://example.net", ""]);
  });

  test("ordinary words and unfinished addresses are left alone", async ({ page }) => {
    await type(page, "example.com www. http:// a@b ");
    for (const o of [3, 14, 20, 26]) expect(await linkAt(page, o)).toBeFalsy();
  });

  test("off: no links", async ({ page }) => {
    await choose(page, { links: false });
    await type(page, "https://example.com ");
    expect(await linkAt(page, 4)).toBeFalsy();
  });
});

test.describe("Markdown shortcuts", () => {
  test.beforeEach(async ({ page }) => blank(page, ONLY("markdown")));

  test("# to ###### at the start of a paragraph make Heading 1 to 6", async ({ page }) => {
    const marks = ["#", "##", "###", "####", "#####", "######"];
    for (const [i, m] of marks.entries()) {
      await type(page, `${m} Title ${i + 1}`);
      await page.keyboard.press("Enter");
    }
    await expect.poll(() => lines(page)).toEqual([...marks.map((_, i) => `Title ${i + 1}`), ""]);
    for (const i of [0, 1, 2, 3, 4, 5]) expect((await stateAt(page, i, 1))?.style).toMatch(new RegExp(`Heading ?${i + 1}$`));
  });

  test("- and * start a bulleted list; the next Enter carries it on", async ({ page }) => {
    await type(page, "- one");
    await page.keyboard.press("Enter");
    await type(page, "two");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await type(page, "* three");
    // The second Enter on the empty item leaves the list, and "* " starts one in that paragraph.
    await expect.poll(() => lines(page)).toEqual(["one", "two", "three"]);
    for (const i of [0, 1, 2]) expect((await stateAt(page, i, 1))?.list).toBe("bullet");
  });

  test("1. and 1) start numbered lists, in their own styles", async ({ page }) => {
    await type(page, "1. first");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await type(page, "1) other");
    await expect.poll(() => lines(page)).toEqual(["first", "other"]);
    expect(await stateAt(page, 0, 1)).toMatchObject({ list: "decimal", listStyle: "decimal" });
    expect(await stateAt(page, 1, 1)).toMatchObject({ list: "decimal", listStyle: "paren" });
  });

  test("[] and [ ] start a checklist", async ({ page }) => {
    await type(page, "[] buy milk");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await type(page, "[ ] call");
    await expect.poll(() => lines(page)).toEqual(["buy milk", "call"]);
    expect((await stateAt(page, 0, 1))?.list).toBe("checklist");
    expect((await stateAt(page, 1, 1))?.list).toBe("checklist");
  });

  test("> makes a Quote", async ({ page }) => {
    await type(page, "> said");
    await expect.poll(() => line(page)).toBe("said");
    expect((await stateAt(page, 0, 1))?.style).toMatch(/Quote/);
  });

  test("--- and Enter makes a horizontal line and leaves the caret under it", async ({ page }) => {
    await type(page, "---");
    await page.keyboard.press("Enter");
    await type(page, "after");
    expect(await page.locator(".flow [data-hr]").count()).toBe(1);
    await expect.poll(() => lines(page)).toEqual(["", "after"]);
  });

  test("the markers need to be at the start: a dash or hash later in a paragraph stays", async ({ page }) => {
    await type(page, "a - b # c > d 1. e");
    await expect.poll(() => line(page)).toBe("a - b # c > d 1. e");
    expect((await stateAt(page, 0, 1))?.list).toBeFalsy();
  });

  test("**bold**, *italic*, _italic_, ~~strike~~ and `code` lose their markers and format the words", async ({ page }) => {
    await type(page, "a **bold** b *it* c _it2_ d ~~gone~~ e `mono` f");
    await expect.poll(() => line(page)).toBe("a bold b it c it2 d gone e mono f");
    const at = (s: string) => "a bold b it c it2 d gone e mono f".indexOf(s) + 2;
    expect((await stateAt(page, 0, at("bold")))?.run.bold).toBe(true);
    expect((await stateAt(page, 0, at("it ")))?.run.italic).toBe(true);
    expect((await stateAt(page, 0, at("it2")))?.run.italic).toBe(true);
    expect((await stateAt(page, 0, at("gone")))?.run.strike).toBe(true);
    expect((await stateAt(page, 0, at("mono")))?.run.font).toBe("Consolas");
    // What's typed outside the markers is plain.
    for (const w of ["a", "b ", "c ", "d ", "e ", "f"]) {
      const run = (await stateAt(page, 0, "a bold b it c it2 d gone e mono f".indexOf(` ${w}`.trimEnd()) + 1))?.run;
      expect(run?.bold).toBeFalsy();
    }
  });

  test("what is typed after a closed span is not bold", async ({ page }) => {
    await type(page, "**bold** plain");
    await expect.poll(() => line(page)).toBe("bold plain");
    expect((await stateAt(page, 0, 2))?.run.bold).toBe(true);
    expect((await stateAt(page, 0, 8))?.run.bold).toBeFalsy();
    expect((await stateAt(page, 0, 10))?.run.bold).toBeFalsy();
  });

  test("snake_case_names and 2 * 3 * 4 are left alone", async ({ page }) => {
    await type(page, "snake_case_name and 2 * 3 * 4");
    await expect.poll(() => line(page)).toBe("snake_case_name and 2 * 3 * 4");
  });

  test("off: the markers stay", async ({ page }) => {
    await choose(page, { markdown: false });
    await type(page, "# a **b**");
    await expect.poll(() => line(page)).toBe("# a **b**");
    expect((await stateAt(page, 0, 4))?.style ?? "").not.toMatch(/Heading/);
  });
});

test.describe("substitutions", () => {
  test.beforeEach(async ({ page }) => blank(page, ONLY("substitutions")));

  const DEFAULTS: [string, string][] = [
    ["(c) ", "© "],
    ["(C) ", "© "],
    ["(r) ", "® "],
    ["(tm) ", "™ "],
    ["a--b", "a—b"],
    ["a -- b", "a — b"],
    ["->", "→"],
    ["<-", "←"],
    ["=>", "⇒"],
    ["...", "…"],
    ["1/2 ", "½ "],
    ["1/4, ", "¼, "],
    ["3/4. ", "¾. "],
    ["!=", "≠"],
    ["<=", "≤"],
    [">=", "≥"],
    ["+-", "±"],
  ];
  for (const [typed, expected] of DEFAULTS) {
    test(`${typed.trim()} becomes ${expected.trim()}`, async ({ page }) => {
      await type(page, typed);
      await expect.poll(() => line(page)).toBe(expected);
    });
  }

  test("fractions inside numbers, and --- for a line, stay as typed", async ({ page }) => {
    await type(page, "11/2 3/45 ---");
    await expect.poll(() => line(page)).toBe("11/2 3/45 ---");
  });

  test("a substitution keeps the formatting of the text it replaces", async ({ page }) => {
    await page.keyboard.press(`${MOD}+B`);
    await type(page, "(c) ");
    await expect.poll(() => line(page)).toBe("© ");
    expect((await stateAt(page, 0, 1))?.run.bold).toBe(true);
  });

  test("a custom substitution added in Preferences works, and a removed default no longer does", async ({ page }) => {
    await chooseMenu(page, "Tools", "Preferences…");
    const d = dialog(page, "Preferences");
    await d.getByRole("button", { name: "Add substitution" }).click();
    const last = d.locator(".pref-sub").last();
    await last.getByLabel("Replace").fill("(ok)");
    await last.getByLabel("With").fill("✓");
    await d.getByRole("button", { name: "Remove (c)", exact: true }).click();
    await d.getByRole("button", { name: "Apply" }).click();
    await expect(d).toBeHidden();

    await type(page, "(ok) (c) ");
    await expect.poll(() => line(page)).toBe("✓ (c) ");
  });

  test("Restore defaults brings the list back", async ({ page }) => {
    await chooseMenu(page, "Tools", "Preferences…");
    const d = dialog(page, "Preferences");
    await d.getByRole("button", { name: "Remove (c)", exact: true }).click();
    await d.getByRole("button", { name: "Add substitution" }).click();
    await expect(d.locator(".pref-sub:not(.head)")).toHaveCount(15);
    await d.getByRole("button", { name: "Restore defaults" }).click();
    await expect(d.locator(".pref-sub:not(.head)")).toHaveCount(15);
    await expect(d.locator(".pref-sub input").first()).toHaveValue("(c)");
    await d.getByRole("button", { name: "Apply" }).click();
    await type(page, "(c) ");
    await expect.poll(() => line(page)).toBe("© ");
  });

  test("switching substitutions off leaves the list but changes nothing", async ({ page }) => {
    await choose(page, { substitutions: false });
    await type(page, "(c) -> ... ");
    await expect.poll(() => line(page)).toBe("(c) -> ... ");
  });
});

test.describe("taking a substitution back", () => {
  test.beforeEach(async ({ page }) => blank(page));

  test("undo puts back what was typed, and only that", async ({ page }) => {
    await type(page, "Cost (c) ");
    await expect.poll(() => line(page)).toBe("Cost © ");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => line(page)).toBe("Cost (c) ");
    // The caret is after the space: typing goes on from there.
    await type(page, "x");
    await expect.poll(() => line(page)).toBe("Cost (c) x");
    // Redo still has nothing to redo after new typing; undo again removes only the "x" step...
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => line(page)).toBe("Cost (c) ");
  });

  test("redo brings the substitution back", async ({ page }) => {
    await type(page, "Cost (c) ");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => line(page)).toBe("Cost (c) ");
    await page.keyboard.press(`${MOD}+Shift+Z`);
    await expect.poll(() => line(page)).toBe("Cost © ");
  });

  test("undo after a smart quote leaves the straight quote", async ({ page }) => {
    await type(page, 'say "');
    await expect.poll(() => line(page)).toBe("Say “");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => line(page)).toBe('Say "');
  });

  test("undo after a capital letter leaves the lower-case word; undo after # leaves the hash", async ({ page }) => {
    await type(page, "hello ");
    await expect.poll(() => line(page)).toBe("Hello ");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => line(page)).toBe("hello ");

    await page.keyboard.press("Enter");
    await type(page, "## ");
    await expect.poll(() => lines(page)).toEqual(["hello ", ""]);
    expect((await stateAt(page, 1, 0))?.style).toMatch(/Heading ?2/);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => lines(page)).toEqual(["hello ", "## "]);
    expect((await stateAt(page, 1, 1))?.style ?? "").not.toMatch(/Heading/);
  });

  test("undo after **bold** leaves the asterisks, and the word is not bold", async ({ page }) => {
    await type(page, "**bold**");
    await expect.poll(() => line(page)).toBe("bold");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => line(page)).toBe("**bold**");
    expect((await stateAt(page, 0, 4))?.run.bold).toBeFalsy();
  });

  test("undo after --- and Enter brings back the dashes", async ({ page }) => {
    await type(page, "---");
    await page.keyboard.press("Enter");
    expect(await page.locator(".flow [data-hr]").count()).toBe(1);
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => lines(page)).toEqual(["---"]);
    expect(await page.locator(".flow [data-hr]").count()).toBe(0);
  });

  test("Backspace right after a substitution takes it back; the next Backspace deletes as usual", async ({ page }) => {
    await type(page, "Cost (c) ");
    await expect.poll(() => line(page)).toBe("Cost © ");
    await page.keyboard.press("Backspace");
    await expect.poll(() => line(page)).toBe("Cost (c) ");
    await page.keyboard.press("Backspace");
    await expect.poll(() => line(page)).toBe("Cost (c)");
  });

  test("Backspace after a smart quote, a capital, a heading and a line", async ({ page }) => {
    await type(page, 'ok "');
    await expect.poll(() => line(page)).toBe("Ok “");
    await page.keyboard.press("Backspace");
    await expect.poll(() => line(page)).toBe('Ok "');

    await page.keyboard.press("Enter");
    await type(page, "# ");
    await page.keyboard.press("Backspace");
    await expect.poll(() => lines(page)).toEqual(['Ok "', "# "]);
    expect((await stateAt(page, 1, 1))?.style ?? "").not.toMatch(/Heading/);

    await page.keyboard.press("Enter");
    await type(page, "---");
    await page.keyboard.press("Enter");
    expect(await page.locator(".flow [data-hr]").count()).toBe(1);
    await page.keyboard.press("Backspace");
    expect(await page.locator(".flow [data-hr]").count()).toBe(0);
    await expect.poll(() => lines(page)).toEqual(['Ok "', "# ", "---"]);
  });

  test("Backspace after typing something else deletes just that", async ({ page }) => {
    await type(page, "Cost (c) x");
    await page.keyboard.press("Backspace");
    await expect.poll(() => line(page)).toBe("Cost © ");
  });

  test("Backspace after moving the caret away and back deletes normally", async ({ page }) => {
    await type(page, "Cost (c) ");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Backspace");
    await expect.poll(() => line(page)).toBe("Cost ©");
  });
});

test.describe("where nothing changes", () => {
  test.beforeEach(async ({ page }) => blank(page));

  test("while composing (an input method)", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.cdp } }, async ({ page }) => {
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Input.imeSetComposition", { text: '"', selectionStart: 1, selectionEnd: 1 });
    await cdp.send("Input.insertText", { text: '"' });
    await expect.poll(() => line(page)).toBe('"');
    await cdp.send("Input.imeSetComposition", { text: "(c)", selectionStart: 3, selectionEnd: 3 });
    await cdp.send("Input.insertText", { text: "(c)" });
    await expect.poll(() => line(page)).toBe('"(c)');
    // Typing is autoformatted again afterwards.
    await type(page, " (r) ");
    await expect.poll(() => line(page)).toBe('"© ® ');
  });

  test("when pasting", { annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }, async ({ page }) => {
    const text = '"quoted" (c) i think www.example.com ';
    await page.evaluate((v) => navigator.clipboard.writeText(v), text);
    await page.keyboard.press(`${MOD}+V`);
    await expect.poll(() => line(page)).toBe(text);
    expect((await stateAt(page, 0, 20))?.link).toBeFalsy();
  });

  test("in a paragraph typed in a monospace font", async ({ page }) => {
    await type(page, "x");
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow [data-p]")!.dataset.p);
      w.fe.editor.exec({ op: "format_text", from: { p, o: 0 }, to: { p, o: 1 }, format: { font: "Consolas" } });
    });
    await page.keyboard.press("End");
    await type(page, ' (c) "q" **b** 1/2 -> https://example.com ');
    await expect.poll(() => line(page)).toBe('x (c) "q" **b** 1/2 -> https://example.com ');
  });

  test("in a paragraph in a code style", async ({ page }) => {
    const ok = await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow [data-p]")!.dataset.p);
      return !!w.fe.editor.exec({ op: "set_style", ids: [p], style: "Code" });
    });
    expect(ok).toBe(true);
    await type(page, 'x (c) "q" **b**');
    await expect.poll(() => line(page)).toBe('x (c) "q" **b**');
  });

  test("when a selection is typed over", async ({ page }) => {
    await type(page, "abc");
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.type('"');
    await expect.poll(() => line(page)).toBe('"');
  });
});

test.describe("embedding", () => {
  test("⌘S in an embedded editor hands onSave the same file save() gives, made in steps", async ({ page }) => {
    await page.goto("/examples/embed.html");
    await page.waitForFunction(() => !!(window as unknown as { demo?: unknown }).demo);
    const right = page.locator("#api-editor .flow .fp").last();
    await right.click();
    await page.keyboard.press(`${MOD}+End`);
    await page.keyboard.type(" Signed.");
    await page.keyboard.press(`${MOD}+s`);
    await expect(page.locator("#log")).toContainText(/right: ⌘S, \d+ bytes/);
    const got = await page.evaluate(async () => {
      type Ed = { save(): Uint8Array; saveInSteps(): Promise<Uint8Array> };
      const ed = (window as unknown as { demo: { editor: Ed } }).demo.editor;
      const log = document.getElementById("log")!.textContent!;
      const keyed = Number(log.match(/right: ⌘S, (\d+) bytes/)![1]);
      const stepped = await ed.saveInSteps();
      return { keyed, stepped: stepped.length, plain: ed.save().length, zip: stepped[0] === 0x50 && stepped[1] === 0x4b };
    });
    // The dates in the file are the same width whenever it is saved, so the same document gives files of the same size.
    expect(got.zip).toBe(true);
    expect(got.stepped).toBe(got.plain);
    expect(got.keyed).toBe(got.plain);
  });

  test("a DocxtorEditor created with autoformat: false types exactly what is typed and has no Preferences command", async ({ page }) => {
    await page.goto("/examples/embed.html");
    await page.waitForFunction(() => !!(window as unknown as { demo?: unknown }).demo);
    const preferences = await page.evaluate(async () => {
      type Host = { mount(el: HTMLElement): Host; create(name?: string): Promise<unknown>; ready: Promise<void>; extensions: { command(id: string): unknown } };
      const DocxtorEditor = (window as unknown as { demo: { editor: { constructor: new (o: object) => Host } } }).demo.editor.constructor;
      const host = document.createElement("div");
      host.id = "plain-editor";
      host.style.height = "400px";
      document.body.prepend(host);
      const fe = new DocxtorEditor({ autoformat: false }).mount(host);
      await fe.ready;
      await fe.create("plain.docx");
      return { preferences: !!fe.extensions.command("tools.preferences") };
    });
    expect(preferences).toEqual({ preferences: false });
    const plain = page.locator("#plain-editor .flow .fp").first();
    await plain.click();
    await page.keyboard.type('"hello" (c) -> i');
    await expect(plain).toHaveText('"hello" (c) -> i');
    // The editor next to it, made with the defaults, still substitutes.
    const normal = page.locator("#api-editor .flow .fp").last();
    await normal.click();
    await page.keyboard.press(`${MOD}+End`);
    await page.keyboard.type(" (c) ");
    await expect(normal).toContainText("©");
  });
});
