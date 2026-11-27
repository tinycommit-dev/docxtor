// Citations: the Citations panel (a form, a DOI, BibTeX), citing at the caret, the popover on a citation, the
// bibliography, and the panel being optional.
import { openBuilt } from "./support/docx-builder";
import { caretAtEnd, chooseMenu, dialog, expect, inPage, menuItem, openMenu, paraText, test, type Page } from "./helpers";

interface Src {
  tag: string;
  type: string;
  title: string;
  year?: string;
  authors: ({ family: string; given?: string } | { literal: string })[];
  doi?: string;
  pages?: string;
}
interface Cite {
  p: number;
  o: number;
  len: number;
  tags: string[];
  page?: string;
  suppressAuthor?: boolean;
}
type Eng = { sources(): Src[]; citations(): Cite[]; citationStyle(): string; sourcesBibtex(): string; text(): string; renderAll(): { order: number[] } };

const panel = (page: Page) => page.locator(".cite-panel");
const items = (page: Page) => panel(page).locator(".src-item");
const item = (page: Page, title: string) => items(page).filter({ hasText: title });
const form = (page: Page, name: string | RegExp = "Add a source") => dialog(page, name);
const popover = (page: Page) => page.locator(".cite-pop");
const cite = (page: Page) => page.locator(".flow .cite");

const sources = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).sources());
const citations = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).citations());
const style = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).citationStyle());
const undo = (page: Page) => inPage(page, (w) => void (w.fe.editor as unknown as { undo(): void }).undo());

/** A document with two short paragraphs; the caret is at the end of the first. Returns the paragraph ids. */
async function start(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Citing is a habit.", "Another paragraph follows."] }] });
  const [first, second] = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
  await caretAtEnd(page, first);
  return { first, second };
}

async function openPanel(page: Page) {
  await chooseMenu(page, "Tools", "Citations");
  await expect(page.getByRole("tab", { name: "Citations" })).toHaveAttribute("aria-selected", "true");
  await expect(panel(page)).toBeVisible();
}

/** Add a book through the form. */
async function addBook(page: Page, o: { title?: string; family?: string; given?: string; year?: string; publisher?: string } = {}) {
  await panel(page).getByRole("button", { name: "Fill in a form…" }).click();
  const f = form(page);
  await f.getByLabel("Title", { exact: true }).fill(o.title ?? "The Art of Citing");
  await f.getByLabel("Family name").first().fill(o.family ?? "Roe");
  await f.getByLabel("Given names").first().fill(o.given ?? "Jane");
  await f.getByLabel("Year").fill(o.year ?? "2019");
  await f.getByLabel("Publisher").fill(o.publisher ?? "Penguin");
  await f.getByRole("button", { name: "Add source" }).click();
  await expect(f).toHaveCount(0);
}

const DOI = "10.1000/xyz123";
const CSL = {
  type: "article-journal",
  title: "A study of things",
  author: [{ family: "Smith", given: "Jane" }, { family: "Lee", given: "Karen" }],
  "container-title": "Journal of Stuff",
  volume: "12",
  issue: "3",
  page: "45-67",
  issued: { "date-parts": [[2020, 5]] },
  DOI,
};

/** Answer doi.org with CSL-JSON (or a status) and note what was asked. */
async function doiOrg(page: Page, answer: () => { status?: number; body?: unknown } = () => ({ body: CSL })) {
  const asked: { url: string; accept: string }[] = [];
  await page.route("https://doi.org/**", (route) => {
    asked.push({ url: route.request().url(), accept: route.request().headers()["accept"] ?? "" });
    const a = answer();
    return route.fulfill({ status: a.status ?? 200, contentType: "application/vnd.citationstyles.csl+json", headers: { "access-control-allow-origin": "*" }, body: JSON.stringify(a.body ?? {}) });
  });
  return asked;
}

const BIB = `@book{roe2019, author = {Roe, Jane}, title = {The Art of Citing}, publisher = {Penguin}, year = {2019}}

@article{smith2020, author = {Smith, John and Lee, Karen}, title = {A study of things}, journal = {Journal of Stuff}, volume = {12}, number = {3}, pages = {45--67}, year = {2020}, doi = {${DOI}}}`;

test.describe("citations panel", () => {
  test("adds a book from the form, cites it in APA and switches to MLA", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await expect(panel(page)).toContainText("No sources yet");
    await expect(panel(page).getByLabel("Citation style")).toHaveValue("apa");

    await addBook(page);
    await expect(items(page)).toHaveCount(1);
    await expect(items(page).first().locator(".src-title")).toHaveText("The Art of Citing");
    await expect(items(page).first().locator(".src-detail").first()).toHaveText("Roe, 2019");
    expect(await sources(page)).toMatchObject([{ tag: "Roe19", type: "book", title: "The Art of Citing", year: "2019", authors: [{ family: "Roe", given: "Jane" }] }]);

    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveText("(Roe, 2019)");
    expect(await paraText(page, first)).toBe("Citing is a habit.(Roe, 2019)");
    await expect(item(page, "The Art of Citing")).toContainText("Cited 1 time");

    // The style names, and a new style rewrites the citation.
    await panel(page).getByLabel("Citation style").selectOption("mla");
    await expect(cite(page)).toHaveText("(Roe)");
    expect(await style(page)).toBe("mla");
    await panel(page).getByLabel("Citation style").selectOption("ieee");
    await expect(cite(page)).toHaveText("[1]");
    await expect(panel(page).getByLabel("Citation style").locator("> option")).toHaveText(["APA 7th edition", "MLA 9th edition", "Chicago 17th edition (author-date)", "IEEE", "Harvard"]);
  });

  test("the form shows the fields each kind of source uses, and contributors as a list", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "Fill in a form…" }).click();
    const f = form(page);
    await expect(f.getByLabel("Kind of source").locator("option")).toHaveText(["Book", "Journal article", "Chapter", "Conference paper", "Report", "Website", "Thesis", "Other"]);
    await expect(f.getByLabel("Publisher")).toBeVisible();
    await expect(f.getByLabel("Journal")).toHaveCount(0);

    await f.getByLabel("Kind of source").selectOption("article");
    await expect(f.getByLabel("Article title")).toBeVisible();
    for (const label of ["Journal", "Volume", "Issue", "Pages", "DOI", "Month"]) await expect(f.getByLabel(label, { exact: true })).toBeVisible();
    await expect(f.getByLabel("Publisher")).toHaveCount(0);

    await f.getByLabel("Kind of source").selectOption("website");
    await expect(f.getByLabel("Page title")).toBeVisible();
    await expect(f.getByLabel("Website name")).toBeVisible();
    await expect(f.getByLabel("Date you read it")).toBeVisible();

    // An organisation as the author and an editor; the title is needed.
    await f.getByRole("button", { name: "Add a contributor" }).click();
    await f.getByRole("button", { name: "Add a contributor" }).click();
    const rows = f.locator(".src-person");
    await expect(rows).toHaveCount(3);
    await rows.nth(0).getByLabel("Person or organisation").selectOption("organisation");
    await rows.nth(0).getByLabel("Organisation name").fill("World Health Organization");
    await rows.nth(1).getByLabel("Role").selectOption("editor");
    await rows.nth(1).getByLabel("Family name").fill("Lee");
    await rows.nth(2).getByLabel("Family name").fill("Spare");
    await f.getByRole("button", { name: "Add source" }).click();
    await expect(f).toContainText("Give the source a title.");
    await f.getByLabel("Page title").fill("Facts about sleep");
    await f.getByLabel("Website name").fill("Health Pages");
    await f.getByLabel("Web address").fill("https://example.org/sleep");
    await f.getByLabel("Date you read it").fill("2024-05-02");
    await rows.nth(2).getByRole("button", { name: "Remove this contributor" }).click();
    await f.getByRole("button", { name: "Add source" }).click();
    await expect(f).toHaveCount(0);

    expect(await sources(page)).toMatchObject([
      {
        type: "website",
        title: "Facts about sleep",
        containerTitle: "Health Pages",
        url: "https://example.org/sleep",
        accessed: "2024-05-02",
        authors: [{ literal: "World Health Organization" }],
        editors: [{ family: "Lee" }],
      },
    ]);
    // Opened again, the contributors are back, and a row can be taken away.
    await item(page, "Facts about sleep").getByRole("button", { name: /^Edit/ }).click();
    const again = form(page, "Edit source");
    await expect(again.locator(".src-person")).toHaveCount(2);
    await again.locator(".src-person").nth(1).getByRole("button", { name: "Remove this contributor" }).click();
    await expect(again.locator(".src-person")).toHaveCount(1);
    await again.getByRole("button", { name: "Cancel" }).click();
  });

  test("adds a source from a DOI, to check before saving", async ({ page }) => {
    const asked = await doiOrg(page);
    await start(page);
    await openPanel(page);
    await expect(panel(page)).toContainText("sends the DOI to doi.org");
    await panel(page).getByRole("button", { name: "From a DOI or URL…" }).click();
    const lookup = dialog(page, "Add from a DOI or URL");

    // Not a DOI: no request.
    await lookup.getByLabel("DOI or link").fill("https://example.com/not-a-doi");
    await lookup.getByRole("button", { name: "Look up" }).click();
    await expect(lookup).toContainText("That doesn't look like a DOI");
    expect(asked).toEqual([]);

    await lookup.getByLabel("DOI or link").fill(`https://doi.org/${DOI}`);
    await lookup.getByRole("button", { name: "Look up" }).click();
    const check = form(page, "Check the source");
    await expect(check.getByLabel("Article title")).toHaveValue("A study of things");
    await expect(check.getByLabel("Journal")).toHaveValue("Journal of Stuff");
    await expect(check.getByLabel("Year")).toHaveValue("2020");
    await expect(check.locator(".src-person")).toHaveCount(2);
    expect(asked).toHaveLength(1);
    expect(asked[0].url).toBe(`https://doi.org/${DOI}`);
    expect(asked[0].accept).toBe("application/vnd.citationstyles.csl+json");
    // Nothing is in the document until it is saved.
    expect(await sources(page)).toEqual([]);

    await check.getByLabel("Pages").fill("45-68");
    await check.getByRole("button", { name: "Add source" }).click();
    await expect(item(page, "A study of things")).toBeVisible();
    expect(await sources(page)).toMatchObject([{ tag: "Smi20", type: "article", title: "A study of things", doi: DOI, pages: "45-68", authors: [{ family: "Smith" }, { family: "Lee" }] }]);
  });

  test("says so when doi.org has no record or can't be reached", async ({ page }) => {
    let mode: "missing" | "down" | "broken" = "missing";
    await page.route("https://doi.org/**", (route) => (mode === "down" ? route.abort("internetdisconnected") : route.fulfill({ status: mode === "missing" ? 404 : 200, headers: { "access-control-allow-origin": "*" }, body: mode === "missing" ? "" : "not json" })));
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "From a DOI or URL…" }).click();
    const lookup = dialog(page, "Add from a DOI or URL");
    await lookup.getByLabel("DOI or link").fill(DOI);
    await lookup.getByRole("button", { name: "Look up" }).click();
    await expect(lookup).toContainText(`doi.org has no record of ${DOI}.`);
    mode = "down";
    await lookup.getByRole("button", { name: "Look up" }).click();
    await expect(lookup).toContainText("Couldn't reach doi.org. Check your connection and try again.");
    mode = "broken";
    await lookup.getByRole("button", { name: "Look up" }).click();
    await expect(lookup.getByRole("alert")).toBeVisible();
    await expect(lookup.getByRole("alert")).not.toContainText("has no record");
    expect(await sources(page)).toEqual([]);
  });

  test("imports BibTeX that is pasted or chosen as a file, and skips what is already there", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "Import BibTeX…" }).click();
    const imp = dialog(page, "Import BibTeX");
    await expect(imp.getByRole("button", { name: "Add sources" })).toBeDisabled();
    await imp.getByLabel("BibTeX", { exact: true }).fill("nothing to see");
    await expect(imp.getByRole("alert")).toContainText("No BibTeX entries found");

    await imp.getByLabel("BibTeX", { exact: true }).fill(BIB);
    await expect(imp.locator(".src-found")).toHaveCount(2);
    await expect(imp.locator(".src-found").first()).toContainText("The Art of Citing");
    await expect(imp.locator(".src-found").first()).toContainText("Roe, 2019");
    // Leave one out.
    await imp.locator(".src-found").nth(1).getByRole("checkbox").uncheck();
    await expect(imp.getByRole("button", { name: "Add 1 source" })).toBeEnabled();
    await imp.locator(".src-found").nth(1).getByRole("checkbox").check();
    await imp.getByRole("button", { name: "Add 2 sources" }).click();
    await expect(imp).toHaveCount(0);
    await expect(items(page)).toHaveCount(2);
    expect((await sources(page)).map((s) => s.tag)).toEqual(["Roe19", "Smi20"]);

    // The same file again, from disk, with one new entry: the two that are there are skipped.
    await panel(page).getByRole("button", { name: "Import BibTeX…" }).click();
    const again = dialog(page, "Import BibTeX");
    const more = `${BIB}\n\n@book{lee2021, author = {Lee, Karen}, title = {More Citing}, year = {2021}}`;
    await again.getByLabel("BibTeX file").setInputFiles({ name: "refs.bib", mimeType: "text/plain", buffer: Buffer.from(more) });
    await expect(again.locator(".src-found")).toHaveCount(3);
    await expect(again.locator(".src-found.dup")).toHaveCount(2);
    await expect(again.locator(".src-found.dup").first()).toContainText("Already in this document");
    await expect(again.locator(".src-found.dup").first().getByRole("checkbox")).toBeDisabled();
    await again.getByRole("button", { name: "Add 1 source" }).click();
    await expect(items(page)).toHaveCount(3);
    expect((await sources(page)).map((s) => s.title)).toEqual(["The Art of Citing", "A study of things", "More Citing"]);

    // A DOI that is already there counts too, even when the title differs.
    await panel(page).getByRole("button", { name: "Import BibTeX…" }).click();
    const doi = dialog(page, "Import BibTeX");
    await doi.getByLabel("BibTeX", { exact: true }).fill(`@article{x, author = {Other, A}, title = {Another title}, year = {2001}, doi = {${DOI.toUpperCase()}}}`);
    await expect(doi.locator(".src-found.dup")).toHaveCount(1);
    await doi.getByRole("button", { name: "Cancel" }).click();
  });

  test("filters the sources by author or title and shows an empty state", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "Import BibTeX…" }).click();
    await dialog(page, "Import BibTeX").getByLabel("BibTeX", { exact: true }).fill(BIB);
    await dialog(page, "Import BibTeX").getByRole("button", { name: "Add 2 sources" }).click();
    await expect(items(page)).toHaveCount(2);
    const search = panel(page).getByRole("searchbox", { name: "Search sources" });
    await search.fill("lee");
    await expect(items(page)).toHaveCount(1);
    await expect(items(page).first()).toContainText("A study of things");
    await search.fill("art citing");
    await expect(items(page)).toHaveCount(1);
    await expect(items(page).first()).toContainText("The Art of Citing");
    await search.fill("zebra");
    await expect(items(page)).toHaveCount(0);
    await expect(panel(page)).toContainText("No source matches “zebra”.");
  });

  test("editing a source updates its citations", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveText("(Roe, 2019)");

    await item(page, "The Art of Citing").getByRole("button", { name: /^Edit/ }).click();
    const edit = form(page, "Edit source");
    await expect(edit.getByLabel("Title", { exact: true })).toHaveValue("The Art of Citing");
    await expect(edit.getByLabel("Family name")).toHaveValue("Roe");
    await edit.getByLabel("Year").fill("2021");
    await edit.getByLabel("Family name").fill("Doe");
    // A field that is emptied is gone from the source.
    await edit.getByLabel("Publisher").fill("");
    await edit.getByRole("button", { name: "Save changes" }).click();
    await expect(edit).toHaveCount(0);
    await expect(cite(page)).toHaveText("(Doe, 2021)");
    expect(await paraText(page, first)).toBe("Citing is a habit.(Doe, 2021)");
    await expect(item(page, "The Art of Citing").locator(".src-detail").first()).toHaveText("Doe, 2021");
    // The tag stays, so the citation still points at the source.
    expect((await citations(page))[0].tags).toEqual(["Roe19"]);
    expect((await sources(page))[0].tag).toBe("Roe19");
    expect((await sources(page))[0]).not.toHaveProperty("publisher");
  });

  test("deleting a cited source asks first and leaves the citation as plain text", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await addBook(page, { title: "Never cited", family: "Park", year: "2018" });
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveCount(1);

    // A source nobody cites goes without asking.
    await item(page, "Never cited").getByRole("button", { name: /^More actions/ }).click();
    await page.getByRole("menuitem", { name: "Delete source" }).click();
    await expect(items(page)).toHaveCount(1);

    // The cited one: the engine's message is shown, and Cancel keeps everything.
    await item(page, "The Art of Citing").getByRole("button", { name: /^More actions/ }).click();
    await page.getByRole("menuitem", { name: "Delete source" }).click();
    const sure = page.getByRole("alertdialog", { name: "Delete this source?" });
    await expect(sure).toContainText("is cited 1 time in the document");
    await sure.getByRole("button", { name: "Cancel" }).click();
    await expect(items(page)).toHaveCount(1);
    await expect(cite(page)).toHaveCount(1);

    await item(page, "The Art of Citing").getByRole("button", { name: /^More actions/ }).click();
    await page.getByRole("menuitem", { name: "Delete source" }).click();
    await page.getByRole("alertdialog", { name: "Delete this source?" }).getByRole("button", { name: "Delete source" }).click();
    await expect(items(page)).toHaveCount(0);
    await expect(panel(page)).toContainText("No sources yet");
    await expect(cite(page)).toHaveCount(0);
    expect(await paraText(page, first)).toBe("Citing is a habit.(Roe, 2019)");
    expect(await citations(page)).toEqual([]);
    expect(await sources(page)).toEqual([]);
  });

  test("cites with a page number from the source's menu", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^More actions/ }).click();
    await page.getByRole("menuitem", { name: "Cite with page…" }).click();
    const ask = dialog(page, "Cite with page");
    await ask.getByLabel("Page, for example 23 or 23-25").fill("23");
    await ask.getByRole("button", { name: "Cite" }).click();
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 23)");
    expect((await citations(page))[0].page).toBe("23");
  });

  test("exports the sources as references.bib", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await expect(panel(page).getByRole("button", { name: "Export BibTeX" })).toBeDisabled();
    await addBook(page);
    const [download] = await Promise.all([page.waitForEvent("download"), panel(page).getByRole("button", { name: "Export BibTeX" }).click()]);
    expect(download.suggestedFilename()).toBe("references.bib");
    const text = await (await import("node:fs/promises")).readFile((await download.path())!, "utf8");
    expect(text).toContain("@book{Roe19,");
    expect(text).toContain("The Art of Citing");
  });
});

test.describe("citing in the text", () => {
  test("Insert, Citation opens the panel on its search box; Enter cites the first source", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await addBook(page, { title: "Sleep Well", family: "Park", year: "2018" });
    await caretAtEnd(page, first);

    await chooseMenu(page, "Insert", "Citation…");
    const search = panel(page).getByRole("searchbox", { name: "Search sources" });
    await expect(search).toBeFocused();
    await search.fill("park");
    await expect(items(page)).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(cite(page)).toHaveText("(Park, 2018)");

    // It starts again from the whole list, and the first source is the one Enter cites.
    await caretAtEnd(page, first);
    await chooseMenu(page, "Insert", "Citation…");
    await expect(search).toBeFocused();
    await expect(search).toHaveValue("");
    await expect(items(page)).toHaveCount(2);
    await page.keyboard.press("Enter");
    await expect(cite(page)).toHaveCount(2);
    await expect(cite(page).last()).toHaveText("(Roe, 2019)");

    // The arrow keys choose between sources.
    await caretAtEnd(page, first);
    await chooseMenu(page, "Insert", "Citation…");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(cite(page)).toHaveCount(3);
    await expect(cite(page).last()).toHaveText("(Park, 2018)");
    expect(await paraText(page, first)).toBe("Citing is a habit.(Park, 2018)(Roe, 2019)(Park, 2018)");
  });

  test("clicking a citation selects it as one unit and opens its popover", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await addBook(page, { title: "Sleep Well", family: "Park", year: "2018" });
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();

    // A light outline on hover.
    await cite(page).hover();
    expect(await cite(page).evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe("none");

    await cite(page).click();
    await expect(popover(page)).toBeVisible();
    const c = (await citations(page))[0];
    expect(await inPage(page, (w) => w.fe.editor.sel)).toEqual({ anchor: { p: first, o: c.o }, focus: { p: first, o: c.o + c.len } });
    await expect(popover(page).locator(".cite-src")).toHaveCount(1);
    await expect(popover(page).locator(".cite-src-title")).toHaveText("The Art of Citing");

    // A page, then leave the author out.
    await popover(page).getByLabel("Page").fill("23");
    await popover(page).getByLabel("Page").press("Enter");
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 23)");
    await popover(page).getByLabel("Omit author").check();
    await expect(cite(page)).toHaveText("(2019, p. 23)");
    expect(await citations(page)).toMatchObject([{ page: "23", suppressAuthor: true }]);
    await popover(page).getByLabel("Omit author").uncheck();
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 23)");
    await popover(page).getByLabel("Omit year").check();
    await expect(cite(page)).toHaveText("(Roe, p. 23)");
    await popover(page).getByLabel("Omit year").uncheck();

    // Another source in the same citation, and out again.
    await popover(page).getByLabel("Add a source").selectOption({ label: "Sleep Well, Park, 2018" });
    await expect(popover(page).locator(".cite-src")).toHaveCount(2);
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 23; Park, 2018)");
    expect((await citations(page))[0].tags.sort()).toEqual(["Par18", "Roe19"]);
    await popover(page).getByRole("button", { name: "Remove Sleep Well from this citation" }).click();
    await expect(popover(page).locator(".cite-src")).toHaveCount(1);
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 23)");
    // The last source can't be taken out; the citation can.
    await expect(popover(page).getByRole("button", { name: "Remove The Art of Citing from this citation" })).toBeDisabled();

    await popover(page).getByRole("button", { name: "Remove citation" }).click();
    await expect(popover(page)).toHaveCount(0);
    await expect(cite(page)).toHaveCount(0);
    expect(await paraText(page, first)).toBe("Citing is a habit.");
    expect(await citations(page)).toEqual([]);
  });

  test("the popover's Edit source opens the form, and Escape closes it with the caret after the citation", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await cite(page).click();
    await popover(page).getByRole("button", { name: "Edit source" }).click();
    const edit = form(page, "Edit source");
    await edit.getByLabel("Year").fill("2022");
    await edit.getByRole("button", { name: "Save changes" }).click();
    await expect(cite(page)).toHaveText("(Roe, 2022)");

    await cite(page).click();
    await expect(popover(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(popover(page)).toHaveCount(0);
    const c = (await citations(page))[0];
    expect(await inPage(page, (w) => w.fe.editor.sel)).toEqual({ anchor: { p: first, o: c.o + c.len }, focus: { p: first, o: c.o + c.len } });
    // Typing carries on after the citation and stays outside it.
    await page.keyboard.type(" Next.");
    expect(await paraText(page, first)).toBe("Citing is a habit.(Roe, 2022) Next.");
    await expect(cite(page)).toHaveText("(Roe, 2022)");
  });

  test("Edit citation from the keyboard opens the popover for the citation at the caret", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveCount(1);
    // Right after the citation, the caret is at its end.
    await page.keyboard.press("ControlOrMeta+Shift+P");
    await page.keyboard.type("Edit citation");
    await page.keyboard.press("Enter");
    await expect(popover(page)).toBeVisible();
  });

  test("deleting the citation's text removes the whole citation", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveCount(1);
    await page.keyboard.press("Backspace");
    await expect(cite(page)).toHaveCount(0);
    expect(await paraText(page, first)).toBe("Citing is a habit.");
    expect(await citations(page)).toEqual([]);
  });
});

test.describe("bibliography", () => {
  test("Insert, Bibliography lists the cited sources; a second one scrolls to the first", async ({ page }) => {
    const { first, second } = await start(page);
    await openPanel(page);
    await addBook(page);
    await addBook(page, { title: "Sleep Well", family: "Park", year: "2018" });
    await addBook(page, { title: "Never cited", family: "Zed", year: "2000" });
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await item(page, "Sleep Well").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveCount(2);

    // After the paragraph with the caret, with the style's heading.
    await chooseMenu(page, "Insert", "Bibliography");
    const bib = page.locator(".flow .fc").filter({ has: page.locator(".fc-label", { hasText: "Bibliography" }) });
    await expect(bib).toHaveCount(1);
    await expect(bib).toContainText("References");
    await expect(bib).toContainText("The Art of Citing");
    await expect(bib).toContainText("Sleep Well");
    await expect(bib).not.toContainText("Never cited");
    // It sits between the two paragraphs: after the one with the caret.
    const order = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
    const block = Number(await bib.getAttribute("data-id"));
    const heading = Number(await bib.locator("[data-p]").first().getAttribute("data-p"));
    expect(order.indexOf(first)).toBeLessThan(order.indexOf(block));
    expect(order.indexOf(block)).toBeLessThan(order.indexOf(second));

    // The button at the bottom of the panel turns into a way back to it; a second bibliography is not made.
    await expect(panel(page).getByRole("button", { name: "Go to bibliography" })).toBeVisible();
    await caretAtEnd(page, second);
    await chooseMenu(page, "Insert", "Bibliography");
    await expect(page.locator(".flow .fc-label", { hasText: "Bibliography" })).toHaveCount(1);
    expect(await inPage(page, (w) => w.fe.editor.sel!.focus.p)).toBe(heading);
  });

  test("the panel's button inserts the bibliography too, in the style of the document", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await panel(page).getByLabel("Citation style").selectOption("mla");
    await panel(page).getByRole("button", { name: "Insert bibliography" }).click();
    await expect(page.locator(".flow .fc")).toContainText("Works Cited");
    await expect(page.locator(".flow .fc")).toContainText("Roe, Jane. The Art of Citing.");
    await panel(page).getByLabel("Citation style").selectOption("apa");
    await expect(page.locator(".flow .fc")).toContainText("References");
    await expect(page.locator(".flow .fc")).toContainText("Roe, J. (2019).");
  });
});

test.describe("undo", () => {
  test("takes back each change", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    expect(await sources(page)).toHaveLength(1);
    await undo(page);
    expect(await sources(page)).toHaveLength(0);
    await expect(items(page)).toHaveCount(0);
    await inPage(page, (w) => void (w.fe.editor as unknown as { redo(): void }).redo());
    await expect(items(page)).toHaveCount(1);

    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveCount(1);
    await undo(page);
    await expect(cite(page)).toHaveCount(0);
    await inPage(page, (w) => void (w.fe.editor as unknown as { redo(): void }).redo());
    await expect(cite(page)).toHaveCount(1);

    await panel(page).getByLabel("Citation style").selectOption("mla");
    await expect(cite(page)).toHaveText("(Roe)");
    await undo(page);
    await expect(cite(page)).toHaveText("(Roe, 2019)");
    await expect(panel(page).getByLabel("Citation style")).toHaveValue("apa");

    await cite(page).click();
    await popover(page).getByLabel("Page").fill("9");
    await popover(page).getByLabel("Page").press("Enter");
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 9)");
    await page.keyboard.press("Escape");
    await undo(page);
    await expect(cite(page)).toHaveText("(Roe, 2019)");

    await item(page, "The Art of Citing").getByRole("button", { name: /^Edit/ }).click();
    await form(page, "Edit source").getByLabel("Year").fill("1999");
    await form(page, "Edit source").getByRole("button", { name: "Save changes" }).click();
    await expect(cite(page)).toHaveText("(Roe, 1999)");
    await undo(page);
    await expect(cite(page)).toHaveText("(Roe, 2019)");

    await chooseMenu(page, "Insert", "Bibliography");
    await expect(page.locator(".flow .fc")).toHaveCount(1);
    await undo(page);
    await expect(page.locator(".flow .fc")).toHaveCount(0);

    await item(page, "The Art of Citing").getByRole("button", { name: /^More actions/ }).click();
    await page.getByRole("menuitem", { name: "Delete source" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Delete source" }).click();
    await expect(items(page)).toHaveCount(0);
    await undo(page);
    await expect(items(page)).toHaveCount(1);
    await expect(cite(page)).toHaveCount(1);

    // One BibTeX import is one undo step.
    await panel(page).getByRole("button", { name: "Import BibTeX…" }).click();
    await dialog(page, "Import BibTeX").getByLabel("BibTeX", { exact: true }).fill(BIB);
    await dialog(page, "Import BibTeX").getByRole("button", { name: "Add 1 source" }).click();
    await expect(items(page)).toHaveCount(2);
    await undo(page);
    await expect(items(page)).toHaveCount(1);
  });
});

test.describe("the panel is optional", () => {
  test("turning the extension off removes Tools, Citations; Insert, Citation still offers the sources", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("docxtor.citations"));
    await expect(panel(page)).toHaveCount(0);
    await openMenu(page, "Tools");
    await expect(menuItem(page, "Citations")).toHaveCount(0);
    await page.keyboard.press("Escape");

    // The editor's own part stays: a plain list of the sources to cite from.
    await caretAtEnd(page, first);
    await chooseMenu(page, "Insert", "Citation…");
    const pick = page.getByRole("dialog", { name: "Cite a source" });
    await expect(pick).toBeVisible();
    await expect(pick).toContainText("The Art of Citing");
    await page.keyboard.press("Enter");
    await expect(cite(page)).toHaveText("(Roe, 2019)");

    // The popover has no Edit source without the panel, but the rest works.
    await cite(page).click();
    await expect(popover(page)).toBeVisible();
    await expect(popover(page).getByRole("button", { name: "Edit source" })).toHaveCount(0);
    await popover(page).getByLabel("Page").fill("4");
    await popover(page).getByLabel("Page").press("Enter");
    await expect(cite(page)).toHaveText("(Roe, 2019, p. 4)");
  });
});
