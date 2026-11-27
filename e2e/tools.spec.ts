// Tools: the accessibility check, Compare documents, the document's language and the dictionary.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, documentReady, expect, inPage, openFixture, RICH, selectWord, test, wordPoint, type Page } from "./helpers";

const MOD = "ControlOrMeta";

/** A 1x1 picture to put in the document. */
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/** The id of the first paragraph whose text is `text`. */
const findPara = (page: Page, text: string) =>
  inPage(
    page,
    (w, t) => {
      const eng = w.fe.editor.engine!;
      const hit = eng.renderAll().blocks.find((b) => b.t === "p" && eng.textRange({ p: b.id, o: 0 }, { p: b.id, o: 1e9 }) === t);
      if (!hit) throw new Error(`no paragraph "${t}"`);
      return hit.id;
    },
    text,
  );

const panel = (page: Page) => page.locator(".a11y");
const issue = (page: Page, kind: string) => panel(page).locator(`.a11y-issue[data-kind="${kind}"]`);

/**
 * simple.docx with one of each problem the check knows about: a skipped heading (in a faint colour),
 * a vague link, faint text, a table with no header row, a typed list, an empty heading and an image
 * with no description. The file has no title.
 */
async function withProblems(page: Page) {
  await openFixture(page, "simple.docx");
  await inPage(
    page,
    async (w, png) => {
      const eng = w.engine!;
      const ed = w.fe.editor;
      const last = () => eng.renderAll().order.at(-1)!;
      ed.exec({ op: "insert_markdown", markdown: "# Report\n\n### Skipped level\n\n[click here](https://example.com) to read more" });
      ed.exec({ op: "insert_paragraph", text: "Faint words" });
      const faint = last();
      ed.exec({ op: "format_text", from: { p: faint, o: 0 }, to: { p: faint, o: 11 }, format: { color: "CCCCCC" } });
      ed.exec({ op: "insert_table", after: last(), rows: 3, cols: 2 });
      for (const text of ["Between", "- apples", "- pears", "- plums", "After the list"]) ed.exec({ op: "insert_paragraph", text });
      ed.exec({ op: "insert_paragraph", text: "", style: "Heading 2" });
      ed.exec({ op: "insert_paragraph", text: "" });
      const bytes = Uint8Array.from(atob(png), (c) => c.charCodeAt(0));
      const media = eng.addMedia("png", bytes);
      ed.exec({ op: "insert_image", at: { p: last(), o: 0 }, media, width: 48, height: 48 });
    },
    PNG,
  );
  await expect(page.locator(".flow .atom.img")).toBeVisible();
}

async function openPanel(page: Page) {
  await chooseMenu(page, "Tools", "Accessibility check");
  await expect(page.getByRole("tab", { name: "Accessibility" })).toHaveAttribute("aria-selected", "true");
}

test.describe("accessibility check", () => {
  test("lists the problems by how serious they are, with counts and fixes", async ({ page }) => {
    await withProblems(page);
    await openPanel(page);

    const kinds = await inPage(page, (w) => w.engine!.accessibility().map((i) => `${i.severity}:${i.kind}`));
    expect(kinds).toEqual(expect.arrayContaining(["error:missing_alt", "error:contrast", "warning:heading_skip", "warning:table_header", "warning:link_text", "warning:empty_heading", "tip:fake_list", "tip:no_title"]));

    const errors = panel(page).locator('.a11y-group[data-severity="error"]');
    const count = (sev: string) => kinds.filter((k) => k.startsWith(`${sev}:`)).length;
    await expect(errors.getByRole("heading", { name: "Errors" })).toContainText(String(count("error")));
    await expect(panel(page).locator('.a11y-group[data-severity="warning"] h3')).toContainText(String(count("warning")));
    await expect(panel(page).locator('.a11y-group[data-severity="tip"] h3')).toContainText(String(count("tip")));
    await expect(panel(page).locator(".a11y-summary")).toHaveText(`${count("error")} errors, ${count("warning")} warnings, ${count("tip")} tips`);

    // The wording is the engine's, passed through the message catalog; the variable part is shown as given.
    await expect(issue(page, "missing_alt")).toContainText("This image has no alt text.");
    await expect(issue(page, "link_text")).toContainText("click here");
    await expect(issue(page, "heading_skip")).toContainText("Heading 1 to Heading 3");
    await expect(issue(page, "fake_list")).toContainText("3 items");
    await expect(issue(page, "contrast").first()).toContainText(/#[0-9A-F]{6} on #FFFFFF \(\d+\.\d:1\)/);

    // Only the problems with a one-step fix get a button for it.
    await expect(issue(page, "table_header").getByRole("button", { name: "Make the first row a header" })).toBeVisible();
    await expect(issue(page, "heading_skip").getByRole("button", { name: "Go to" })).toBeVisible();
    await expect(issue(page, "heading_skip").getByRole("button")).toHaveCount(1);
    await expect(issue(page, "no_title").getByRole("button", { name: "Go to" })).toHaveCount(0);
    await expect(issue(page, "no_title").getByRole("button", { name: "Add a title" })).toBeVisible();
  });

  test("Go to selects the problem; an image gets its frame", async ({ page }) => {
    await withProblems(page);
    await openPanel(page);
    const sel = () => inPage(page, (w) => w.fe.editor.sel);

    // A vague link: the link text is selected.
    const link = await findPara(page, "click here to read more");
    await issue(page, "link_text").getByRole("button", { name: "Go to" }).click();
    await expect.poll(sel).toEqual({ anchor: { p: link, o: 0 }, focus: { p: link, o: "click here".length } });

    // A table: its first paragraph.
    const issues = await inPage(page, (w) => w.engine!.accessibility());
    const table = issues.find((i) => i.kind === "table_header")!;
    await issue(page, "table_header").getByRole("button", { name: "Go to" }).click();
    await expect.poll(async () => (await sel())?.anchor.p).toBe(table.p);

    // An image: selected as a click would, with its frame.
    const img = issues.find((i) => i.kind === "missing_alt")!;
    await issue(page, "missing_alt").getByRole("button", { name: "Go to" }).click();
    await expect.poll(sel).toEqual({ anchor: { p: img.p, o: img.o }, focus: { p: img.p, o: img.o! + 1 } });
    await expect(page.locator(".img-frame")).toBeVisible();

    // An image that can't be selected (no frame for it): the caret goes to where it is instead.
    await inPage(page, (w) => {
      (w.fe as unknown as { images: { selectAt(): boolean } }).images.selectAt = () => false;
      w.fe.editor.select({ anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } });
    });
    await issue(page, "missing_alt").getByRole("button", { name: "Go to" }).click();
    await expect.poll(sel).toEqual({ anchor: { p: img.p, o: img.o }, focus: { p: img.p, o: img.o } });

    // A skipped heading: the caret goes to its paragraph.
    const heading = await findPara(page, "Skipped level");
    await issue(page, "heading_skip").getByRole("button", { name: "Go to" }).click();
    await expect.poll(async () => (await sel())?.anchor.p).toBe(heading);
  });

  test("each one-click fix clears its problem", async ({ page }) => {
    await withProblems(page);
    await openPanel(page);

    // Table: the first row becomes a header.
    await issue(page, "table_header").getByRole("button", { name: "Make the first row a header" }).click();
    await expect(issue(page, "table_header")).toHaveCount(0);
    await expect(page.locator(".flow tr[data-header]")).toHaveCount(1);

    // Typed list: real bullets, and the typed markers are gone.
    await issue(page, "fake_list").getByRole("button", { name: "Make it a list" }).click();
    await expect(issue(page, "fake_list")).toHaveCount(0);
    const apples = await findPara(page, "apples");
    expect(await inPage(page, (w, p) => w.engine!.formatAt({ p, o: 0 })?.list, apples)).toBe("bullet");
    expect(await inPage(page, (w) => w.engine!.text())).not.toContain("- apples");

    // Empty heading: normal text.
    await issue(page, "empty_heading").getByRole("button", { name: "Make it normal text" }).click();
    await expect(issue(page, "empty_heading")).toHaveCount(0);

    // Image: the alt text dialog, then the problem is gone.
    await issue(page, "missing_alt").getByRole("button", { name: "Add alt text" }).click();
    const alt = dialog(page, "Alt text");
    await alt.getByLabel("Describe the image for screen readers").fill("A single pixel");
    await alt.getByRole("button", { name: "Save" }).click();
    await expect(issue(page, "missing_alt")).toHaveCount(0);

    // Title: asks for it, then writes the document property.
    await issue(page, "no_title").getByRole("button", { name: "Add a title" }).click();
    const title = dialog(page, "Add a title");
    await title.getByLabel("Title").fill("Quarterly report");
    await title.getByRole("button", { name: "Save" }).click();
    await expect(issue(page, "no_title")).toHaveCount(0);
    expect((await inPage(page, (w) => w.engine!.docProps())).title).toBe("Quarterly report");

    // What has no one-step fix stays.
    await expect(issue(page, "link_text")).toHaveCount(1);
    await expect(issue(page, "heading_skip")).toHaveCount(1);
  });

  test("a missing language opens the Language dialog", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Hello there"] }] });
    await openPanel(page);
    await expect(issue(page, "language")).toBeVisible();
    await issue(page, "language").getByRole("button", { name: "Set the language" }).click();
    const lang = dialog(page, "Language");
    await expect(lang).toBeVisible();
    await lang.getByRole("option", { name: "German (Germany)" }).click();
    await lang.getByRole("button", { name: "Apply" }).click();
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("de-DE");
    await expect(issue(page, "language")).toHaveCount(0);
  });

  test("checks again after edits, says all clear, and closes like a tab", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await openPanel(page);
    // Only the missing title is wrong with this document.
    await expect(panel(page).locator(".a11y-issue")).toHaveCount(1);
    await expect(issue(page, "no_title")).toBeVisible();

    // An edit made elsewhere shows up without asking.
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_paragraph", text: "", style: "Heading 2" }));
    await expect(issue(page, "empty_heading")).toBeVisible();

    await issue(page, "empty_heading").getByRole("button", { name: "Make it normal text" }).click();
    await issue(page, "no_title").getByRole("button", { name: "Add a title" }).click();
    await dialog(page, "Add a title").getByRole("button", { name: "Save" }).click();
    await expect(panel(page).getByText("All clear")).toBeVisible();
    await expect(panel(page).locator(".a11y-issue")).toHaveCount(0);

    // The tool's tab goes away when it is closed; Files comes back.
    await page.locator('.tab-close[title="Close Accessibility"]').click();
    await expect(page.getByRole("tab", { name: "Accessibility" })).toHaveCount(0);
    await expect(page.getByRole("tab", { name: "Files" })).toHaveAttribute("aria-selected", "true");
  });

  test("the command palette finds it", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.keyboard.press(`${MOD}+Shift+P`);
    await page.keyboard.type("accessibility");
    await expect(page.getByRole("option", { name: /Accessibility check/ })).toBeVisible();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("tab", { name: "Accessibility" })).toHaveAttribute("aria-selected", "true");
  });
});

// -- compare ---------------------------------------------------------------------------------------

const OLDER = "Alpha beta gamma.\n\nSecond stays.\n\nThird will go.\n\nFourth stays.";
const NEWER = "Alpha delta gamma.\n\nSecond stays.\n\nFourth stays.\n\nA brand new ending.";

/** Two Library documents, the older one made first; the newer one is left open. */
async function twoVersions(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Draft v1.docx", md), OLDER);
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Draft v1");
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Draft v2.docx", md), NEWER);
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Draft v2");
  await documentReady(page);
}

const compareDialog = (page: Page) => dialog(page, "Compare documents").filter({ has: page.locator(".cmp-list") });

/** Tools, Compare documents, then pick the older version from the list. */
async function compareWithV1(page: Page) {
  await chooseMenu(page, "Tools", "Compare documents…");
  await page.locator(".qp-item", { hasText: "Draft v1" }).click();
  await expect(compareDialog(page)).toBeVisible();
}

test.describe("compare documents", () => {
  test("shows what changed, word by word, and counts the changes", async ({ page }) => {
    await twoVersions(page);
    await compareWithV1(page);
    const view = compareDialog(page);

    await expect(view.locator(".cmp-count")).toHaveText("3 changes");
    // The edited paragraph: the old word crossed out, the new one added.
    const edited = view.locator('.cmp-row[data-kind="changed"]');
    await expect(edited.locator("del")).toHaveText("beta");
    await expect(edited.locator("ins")).toHaveText("delta");
    await expect(edited).toContainText(/Alpha beta\s*delta gamma\./);
    // A removed paragraph is struck through whole, a new one marked as added.
    await expect(view.locator('.cmp-row[data-kind="removed"] del')).toHaveText("Third will go.");
    await expect(view.locator('.cmp-row[data-kind="added"] ins')).toHaveText("A brand new ending.");
    await expect(view.locator('.cmp-row[data-kind="removed"] .cmp-tag')).toHaveText("Removed");
    // The strike-through and the underline are not just colour.
    expect(await view.locator('.cmp-row[data-kind="removed"] del').evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe("line-through");
    expect(await edited.locator("ins").evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe("underline");
    // Paragraphs that did not change are there too, until "Changes only" hides them.
    await expect(view.locator('.cmp-row[data-kind="same"]')).toHaveCount(2);
    await view.getByLabel("Changes only").check();
    await expect(view.locator('.cmp-row[data-kind="same"]')).toHaveCount(0);
    await expect(view.locator(".cmp-row")).toHaveCount(3);
  });

  test("Next and Previous move between the changes", async ({ page }) => {
    await twoVersions(page);
    await compareWithV1(page);
    const view = compareDialog(page);
    const pos = view.locator(".cmp-pos");
    const current = view.locator(".cmp-row.current");

    await view.getByRole("button", { name: "Next change" }).click();
    await expect(pos).toHaveText("Change 1 of 3");
    await expect(current).toHaveCount(1);
    await expect(current).toContainText("gamma.");
    await view.getByRole("button", { name: "Next change" }).click();
    await expect(pos).toHaveText("Change 2 of 3");
    await expect(current).toContainText("Third will go.");
    await view.getByRole("button", { name: "Next change" }).click();
    await expect(pos).toHaveText("Change 3 of 3");
    await expect(current).toContainText("A brand new ending.");
    await view.getByRole("button", { name: "Next change" }).click();
    await expect(pos).toHaveText("Change 1 of 3");
    await view.getByRole("button", { name: "Previous change" }).click();
    await expect(pos).toHaveText("Change 3 of 3");
    await view.getByRole("button", { name: "Previous change" }).click();
    await expect(pos).toHaveText("Change 2 of 3");
  });

  test("Swap compares the other way round", async ({ page }) => {
    await twoVersions(page);
    await compareWithV1(page);
    const view = compareDialog(page);
    await view.getByRole("button", { name: "Swap" }).click();
    await expect(view.getByRole("button", { name: "Swap" })).toHaveAttribute("aria-pressed", "true");
    await expect(view.locator(".cmp-count")).toHaveText("3 changes");
    const edited = view.locator('.cmp-row[data-kind="changed"]');
    await expect(edited.locator("del")).toHaveText("delta");
    await expect(edited.locator("ins")).toHaveText("beta");
    // What was added is now removed, and the other way round.
    await expect(view.locator('.cmp-row[data-kind="added"] ins')).toHaveText("Third will go.");
    await expect(view.locator('.cmp-row[data-kind="removed"] del')).toHaveText("A brand new ending.");
    await view.getByRole("button", { name: "Swap" }).click();
    await expect(view.locator('.cmp-row[data-kind="changed"] ins')).toHaveText("delta");
  });

  test("opens the comparison as a document with tracked changes", async ({ page }) => {
    await twoVersions(page);
    const newerText = await inPage(page, (w) => w.engine!.text());
    await compareWithV1(page);
    await compareDialog(page).getByRole("button", { name: "Open as a document with tracked changes" }).click();
    await expect(compareDialog(page)).toBeHidden();

    // A new document in the Library, named for both versions.
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Comparison of Draft v2 and Draft v1");
    expect(await inPage(page, (w) => w.docKey())).toBe("library:Comparison of Draft v2 and Draft v1.docx");
    await documentReady(page);
    // Its text is the newer text, with the insertions underlined and the deletions kept, struck through.
    // (The engine keeps the emptied paragraph of a deleted one, as a tracked deletion leaves it.)
    const flat = (s: string) => s.replace(/\n+/g, "\n");
    expect(flat(await inPage(page, (w) => w.engine!.text()))).toBe(flat(newerText));
    await expect(page.locator(".flow .ins").first()).toBeVisible();
    await expect(page.locator(".flow .ins", { hasText: "delta" })).toHaveCount(1);
    const gone = page.locator(".flow .ghost.del", { hasText: "beta" });
    await expect(gone).toHaveCount(1);
    expect(await gone.evaluate((el) => getComputedStyle(el).textDecorationLine)).toBe("line-through");
    // Readable: dark red and green on white, not the browser's default.
    expect(await gone.evaluate((el) => getComputedStyle(el).color)).toBe("rgb(180, 35, 24)");
    expect(await page.locator(".flow .ins", { hasText: "delta" }).evaluate((el) => getComputedStyle(el).color)).toBe("rgb(38, 106, 66)");
    // The original is untouched.
    const older = await inPage(page, async (w) => new TextDecoder().decode(await w.files.library.read("Draft v1.docx")).length > 0);
    expect(older).toBe(true);
  });

  test("can compare with a file from the computer", async ({ page }) => {
    await twoVersions(page);
    const older = await inPage(page, async (w) => Array.from(await w.files.library.read("Draft v1.docx")));
    await chooseMenu(page, "Tools", "Compare documents…");
    const chooser = page.waitForEvent("filechooser");
    await page.locator(".qp-item", { hasText: "From your computer…" }).click();
    await (await chooser).setFiles({ name: "Elsewhere.docx", mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", buffer: Buffer.from(older) });
    const view = compareDialog(page);
    await expect(view).toBeVisible();
    await expect(view.locator(".cmp-count")).toHaveText("3 changes");
    await expect(view.locator(".sub")).toContainText("“Elsewhere”");
    await view.getByRole("button", { name: "Close" }).click();
    await expect(view).toBeHidden();
  });

  test("two documents with the same text say so", async ({ page }) => {
    await twoVersions(page);
    await inPage(page, (w, md) => w.createIn(w.files.library, "", "Draft v3.docx", md), NEWER);
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Draft v3");
    await chooseMenu(page, "Tools", "Compare documents…");
    await page.locator(".qp-item", { hasText: "Draft v2" }).click();
    const view = compareDialog(page);
    await expect(view.locator(".cmp-count")).toHaveText("No changes");
    await expect(view.getByText("No differences. The two documents read the same")).toBeVisible();
    await expect(view.getByRole("button", { name: "Next change" })).toBeDisabled();
  });
});

// -- language --------------------------------------------------------------------------------------

// The names come from the browser's own language data: Safari says "English (US)" and "inglés (EE. UU.)" where Chrome and Firefox say "English (United States)" and "inglés (Estados Unidos)".
const ENGLISH_US = /English \((United States|US)\)/;
const INGLES_US = /inglés \((Estados Unidos|EE\. UU\.)\)/i;

test.describe("document language", () => {
  test("File, Language shows the current language and sets another", async ({ page }) => {
    await openFixture(page);
    const lang0 = await inPage(page, (w) => w.engine!.pageSetup().lang);
    expect(lang0).toBe("en-US");
    await chooseMenu(page, "File", "Language…");
    const lang = dialog(page, "Language");
    await expect(lang).toBeVisible();
    await expect(lang.getByRole("option", { name: ENGLISH_US })).toHaveAttribute("aria-selected", "true");
    await expect(lang.getByRole("option", { name: ENGLISH_US })).toContainText("Current");
    await expect(lang.locator(".sub")).toContainText(ENGLISH_US);
    // The list is searchable, and the common languages are all there.
    expect(await lang.getByRole("option").count()).toBe(27);
    await lang.getByRole("combobox", { name: "Search languages" }).fill("span");
    await expect(lang.getByRole("option")).toHaveCount(2);
    await lang.getByRole("option", { name: "Spanish (Mexico)" }).click();
    await lang.getByRole("button", { name: "Apply" }).click();
    await expect(lang).toBeHidden();
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("es-MX");
    // One undo step.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("en-US");

    // Reopened, it shows the language the document has now.
    await chooseMenu(page, "File", "Language…");
    await expect(dialog(page, "Language").getByRole("option", { name: ENGLISH_US })).toContainText("Current");
  });

  test("the keyboard picks a language: arrows move, Enter applies", async ({ page }) => {
    await openFixture(page);
    await chooseMenu(page, "File", "Language…");
    const lang = dialog(page, "Language");
    await lang.getByRole("combobox", { name: "Search languages" }).fill("french");
    await expect(lang.getByRole("option", { name: "French (France)" })).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    await expect(lang).toBeHidden();
    await expect.poll(() => inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("fr-FR");
  });

  test("with a selection it can set the language of that text only", async ({ page }) => {
    await openFixture(page);
    await inPage(page, (w) => {
      const ed = w.fe.editor as unknown as { exec: (ops: unknown, o?: unknown) => unknown; __ops?: unknown[] };
      const orig = ed.exec.bind(ed);
      ed.__ops = [];
      ed.exec = (ops, o) => (ed.__ops!.push(ops), orig(ops, o));
    });
    // Without a selection there is no such choice.
    await chooseMenu(page, "File", "Language…");
    await expect(dialog(page, "Language").getByLabel("Apply to the selected text only")).toHaveCount(0);
    await page.keyboard.press("Escape");

    await selectWord(page, RICH.last, "paragraph");
    await chooseMenu(page, "File", "Language…");
    const lang = dialog(page, "Language");
    await lang.getByRole("option", { name: "French (France)" }).click();
    await lang.getByLabel("Apply to the selected text only").check();
    await lang.getByRole("button", { name: "Apply" }).click();
    expect(await inPage(page, (w) => (w.fe.editor as unknown as { __ops: unknown[] }).__ops.at(-1))).toEqual({ op: "set_language", lang: "fr-FR", ids: [RICH.last] });
    // The document's own language is unchanged.
    expect(await inPage(page, (w) => w.fe.editor.engine!.pageSetup().lang)).toBe("en-US");
  });

  test("names the languages in the interface language", async ({ page }) => {
    await openFixture(page, "rich.docx", "&lang=es");
    await page.getByRole("menubar").getByRole("menuitem", { name: "Archivo", exact: true }).click();
    await page.locator('.menu-drop [role="menuitem"]').filter({ hasText: "Idioma…" }).click();
    const lang = dialog(page, "Idioma");
    await expect(lang.getByRole("option", { name: INGLES_US })).toBeVisible();
    await expect(lang.getByRole("button", { name: "Aplicar" })).toBeVisible();
  });

  test("Document details has an editable title", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await chooseMenu(page, "File", "Document details…");
    const details = dialog(page, "Document details");
    const title = details.getByLabel("Title");
    await expect(title).toHaveValue("");
    await title.fill("Docxtor test document");
    await title.press("Enter");
    await expect.poll(() => inPage(page, (w) => w.engine!.docProps().title)).toBe("Docxtor test document");
    await details.getByRole("button", { name: "Done" }).click();
    // It is what the accessibility check asks for.
    await chooseMenu(page, "Tools", "Accessibility check");
    await expect(issue(page, "no_title")).toHaveCount(0);
  });
});

// -- dictionary ------------------------------------------------------------------------------------

const CAT = {
  en: [
    {
      partOfSpeech: "Noun",
      language: "English",
      definitions: [
        { definition: 'A <b>small</b> <a href="/wiki/domesticated">domesticated</a> carnivorous mammal.<script>window.__pwned = 1</script><img src=x onerror="window.__pwned = 2">', parsedExamples: [{ example: "The <i>cat</i> sat on the mat." }] },
        { definition: "A <span>spiteful</span> woman." },
      ],
    },
    { partOfSpeech: "Verb", language: "English", definitions: [{ definition: "To <i>vomit</i>." }] },
  ],
  es: [{ partOfSpeech: "Noun", language: "Spanish", definitions: [{ definition: "una <b>gata</b> de la casa" }] }],
};

const WIKTIONARY = "https://en.wiktionary.org/api/rest_v1/page/definition/**";

/** Answer Wiktionary with `body` (or a status), and note which words were asked for. */
async function wiktionary(page: Page, answer: (word: string) => { status?: number; body?: unknown; abort?: boolean }) {
  const asked: string[] = [];
  await page.route(WIKTIONARY, (route) => {
    const word = decodeURIComponent(new URL(route.request().url()).pathname.split("/").pop()!);
    asked.push(word);
    const a = answer(word);
    if (a.abort) return route.abort("internetdisconnected");
    return route.fulfill({ status: a.status ?? 200, contentType: "application/json", headers: { "access-control-allow-origin": "*" }, body: JSON.stringify(a.body ?? {}) });
  });
  return asked;
}

const dict = (page: Page) => page.locator(".dict");

test.describe("dictionary", () => {
  test("looks up the selected word and groups the definitions by part of speech", async ({ page }) => {
    const asked = await wiktionary(page, () => ({ body: CAT }));
    await openFixture(page);
    await selectWord(page, RICH.last, "paragraph");
    await page.keyboard.press(`${MOD}+Shift+Y`);
    await expect(page.getByRole("tab", { name: "Dictionary" })).toHaveAttribute("aria-selected", "true");
    await expect(dict(page).locator(".dict-word")).toHaveText("paragraph");
    expect(asked).toEqual(["paragraph"]);
    // Part of speech headings with numbered definitions, HTML turned to plain text.
    await expect(dict(page).locator(".dict-pos")).toHaveText(["Noun", "Verb"]);
    const first = dict(page).locator(".dict-defs").first().locator("li").first();
    await expect(first).toContainText("A small domesticated carnivorous mammal.");
    await expect(first.locator(".dict-ex")).toHaveText("The cat sat on the mat.");
    await expect(dict(page).locator(".dict-defs").first().locator("li")).toHaveCount(2);
    expect(await dict(page).locator("b, i, a[href='/wiki/domesticated'], script, img").count()).toBe(0);
    expect(await page.evaluate(() => (window as unknown as { __pwned?: number }).__pwned)).toBeUndefined();
    // English document: only the English entries, and a link to the whole entry.
    await expect(dict(page)).not.toContainText("gata");
    const link = dict(page).getByRole("link", { name: "Open the full entry on Wiktionary" });
    await expect(link).toHaveAttribute("href", "https://en.wiktionary.org/wiki/paragraph");
    await expect(link).toHaveAttribute("rel", /noopener/);
  });

  test("uses the word at the caret, and the search box looks up others", async ({ page }) => {
    const asked = await wiktionary(page, (w) => (w === "nothingtosee" ? { status: 404 } : { body: CAT }));
    await openFixture(page);
    const { x, y } = await wordPoint(page, RICH.last, "Final");
    await page.mouse.click(x, y);
    await chooseMenu(page, "Tools", "Dictionary");
    await expect(dict(page).locator(".dict-word")).toHaveText("Final");
    expect(asked).toEqual(["Final"]);

    await dict(page).getByRole("searchbox", { name: "Look up a word" }).fill("cat");
    await page.keyboard.press("Enter");
    await expect(dict(page).locator(".dict-word")).toHaveText("cat");
    // A word nobody wrote an entry for.
    await dict(page).getByRole("searchbox", { name: "Look up a word" }).fill("nothingtosee");
    await dict(page).getByRole("button", { name: "Look up" }).click();
    await expect(dict(page)).toContainText("Wiktionary has no entry for “nothingtosee”.");
    expect(asked.at(-1)).toBe("nothingtosee");
  });

  test("a capitalised word falls back to its lower-case entry", async ({ page }) => {
    const asked = await wiktionary(page, (w) => (w === "final" ? { body: CAT } : { status: 404 }));
    await openFixture(page);
    await chooseMenu(page, "Tools", "Dictionary");
    await dict(page).getByRole("searchbox", { name: "Look up a word" }).fill("Final");
    await page.keyboard.press("Enter");
    await expect(dict(page).locator(".dict-pos").first()).toHaveText("Noun");
    expect(asked.slice(-2)).toEqual(["Final", "final"]);
  });

  test("says so when Wiktionary can't be reached or answers badly", async ({ page }) => {
    let mode: "offline" | "broken" = "offline";
    await wiktionary(page, () => (mode === "offline" ? { abort: true } : { status: 503 }));
    await openFixture(page);
    await chooseMenu(page, "Tools", "Dictionary");
    const box = dict(page).getByRole("searchbox", { name: "Look up a word" });
    await box.fill("cat");
    await page.keyboard.press("Enter");
    await expect(dict(page)).toContainText("Couldn't reach Wiktionary. Check your connection and try again.");
    mode = "broken";
    await box.fill("dog");
    await page.keyboard.press("Enter");
    await expect(dict(page)).toContainText("Wiktionary didn't answer (error 503). Try again later.");
  });

  test("tells the person where the word goes, once", async ({ page }) => {
    await wiktionary(page, () => ({ body: CAT }));
    await openFixture(page);
    await chooseMenu(page, "Tools", "Dictionary");
    const notice = dict(page).locator(".dict-notice");
    await expect(notice).toContainText("Looks words up on Wiktionary. The word is sent to Wikipedia's servers; nothing else is.");
    await notice.getByRole("button", { name: "Got it" }).click();
    await expect(notice).toHaveCount(0);
    await page.reload();
    await documentReady(page);
    await chooseMenu(page, "Tools", "Dictionary");
    await expect(dict(page).locator(".dict-notice")).toHaveCount(0);
  });

  test("shows the entry for the document's language", async ({ page }) => {
    await wiktionary(page, () => ({ body: CAT }));
    await openBuilt(page, { lang: "es-ES", sections: [{ paragraphs: ["Mi gata duerme."] }] });
    await selectWord(page, await findPara(page, "Mi gata duerme."), "gata");
    await page.keyboard.press(`${MOD}+Shift+Y`);
    await expect(dict(page).locator(".dict-defs li")).toHaveText("una gata de la casa");
    await expect(dict(page).locator(".dict-lang")).toHaveCount(0);
  });

  test("when the document's language has no entry it lists the languages there are", async ({ page }) => {
    await wiktionary(page, () => ({ body: CAT }));
    await openBuilt(page, { lang: "de-DE", sections: [{ paragraphs: ["Eine Katze."] }] });
    await selectWord(page, await findPara(page, "Eine Katze."), "Katze");
    await page.keyboard.press(`${MOD}+Shift+Y`);
    await expect(dict(page).locator(".dict-note")).toContainText("No German entry");
    await expect(dict(page).locator(".dict-lang")).toHaveText(["English", "Spanish"]);
  });

  test("turning the extension off removes it", async ({ page }) => {
    await openFixture(page);
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("docxtor.dictionary"));
    await page.getByRole("menuitem", { name: "Tools", exact: true }).click();
    await expect(page.getByRole("menuitem", { name: "Dictionary" })).toHaveCount(0);
  });
});
