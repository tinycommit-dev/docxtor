// CSL citation styles, Chicago notes (citations as footnotes), source providers and Zotero. The formatting itself is
// tested in Rust (crates/docxtor-core/tests/all/csl.rs and citations_csl.rs); here is what a person sees and does.
import { openBuilt } from "./support/docx-builder";
import { caretAtEnd, chooseMenu, dialog, expect, inPage, paraText, test, type Page } from "./helpers";

interface Src {
  tag: string;
  title: string;
}
type Eng = { sources(): Src[]; citations(): { p: number; o: number; len: number; tags: string[]; page?: string }[]; citationStyle(): string; renderAll(): { order: number[] }; pageSetup(): { notes: { id: string; label: string; blocks: unknown[] }[] } };

const panel = (page: Page) => page.locator(".cite-panel");
const items = (page: Page) => panel(page).locator(".src-item");
const item = (page: Page, title: string) => items(page).filter({ hasText: title });
const cite = (page: Page) => page.locator(".flow .cite");
const styleSelect = (page: Page) => panel(page).getByLabel("Citation style");
const noteCite = (page: Page) => page.locator(".sheet .notes .cite");

const style = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).citationStyle());
const citations = (page: Page) => inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).citations());

/** The text of each footnote, in the order of their marks. */
const noteTexts = (page: Page) =>
  inPage(page, (w) => {
    const walk = (v: unknown, out: string[]) => {
      if (Array.isArray(v)) v.forEach((c) => walk(c, out));
      else if (v && typeof v === "object") {
        const o = v as Record<string, unknown>;
        if (o.k === "t" && typeof o.x === "string") out.push(o.x);
        Object.values(o).forEach((c) => walk(c, out));
      }
    };
    return (w.fe.editor.engine as unknown as Eng).pageSetup().notes.map((n) => {
      const out: string[] = [];
      walk(n.blocks, out);
      return out.join("").trim();
    });
  });

async function start(page: Page) {
  await openBuilt(page, { sections: [{ paragraphs: ["Citing is a habit.", "Another paragraph follows."] }] });
  const [first, second] = await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).renderAll().order);
  await caretAtEnd(page, first);
  return { first, second };
}

async function openPanel(page: Page) {
  await chooseMenu(page, "Tools", "Citations");
  await expect(panel(page)).toBeVisible();
}

/** Add a book through the form. */
async function addBook(page: Page, o: { title?: string; family?: string; given?: string; year?: string; publisher?: string } = {}) {
  await panel(page).getByRole("button", { name: "Fill in a form…" }).click();
  const f = dialog(page, "Add a source");
  await f.getByLabel("Title", { exact: true }).fill(o.title ?? "The Art of Citing");
  await f.getByLabel("Family name").first().fill(o.family ?? "Roe");
  await f.getByLabel("Given names").first().fill(o.given ?? "Jane");
  await f.getByLabel("Year").fill(o.year ?? "2019");
  await f.getByLabel("Publisher").fill(o.publisher ?? "Penguin");
  await f.getByRole("button", { name: "Add source" }).click();
  await expect(f).toHaveCount(0);
}

test.describe("CSL styles", () => {
  test("the style picker lists Docxtor's styles and the CSL styles in groups", async ({ page }) => {
    await start(page);
    await openPanel(page);
    const select = styleSelect(page);
    await expect(select.locator("> option")).toHaveText(["APA 7th edition", "MLA 9th edition", "Chicago 17th edition (author-date)", "IEEE", "Harvard"]);
    await expect(select.locator("optgroup")).toHaveCount(3);
    await expect(select.locator('optgroup[label="Notes"] option')).toHaveText(["Chicago 16th edition (notes and bibliography)", "Chicago 18th edition (notes and bibliography)"]);
    await expect(select.locator('optgroup[label="Numbered"] option')).toHaveText(["Vancouver (Elsevier)", "Nature", "AMA 11th edition"]);
  });

  test("choosing a numbered CSL style rewrites the citations and the bibliography, and undo is one step", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await addBook(page, { title: "Sleep Well", family: "Park", year: "2018", publisher: "Hachette" });
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await item(page, "Sleep Well").getByRole("button", { name: /^Cite/ }).click();
    await chooseMenu(page, "Insert", "Bibliography");
    await expect(cite(page)).toHaveText(["(Roe, 2019)", "(Park, 2018)"]);

    await styleSelect(page).selectOption("vancouver");
    await expect(cite(page)).toHaveText(["[1]", "[2]"]);
    expect(await style(page)).toBe("vancouver");
    const bib = page.locator(".flow .fc");
    await expect(bib).toContainText("Roe J. The Art of Citing. Penguin; 2019.");
    await expect(bib).toContainText("Park J. Sleep Well. Hachette; 2018.");
    expect(await paraText(page, first)).toBe("Citing is a habit.[1][2]");

    // Superscript numbers are drawn raised.
    await styleSelect(page).selectOption("nature");
    await expect(cite(page).first()).toHaveText("1");
    expect(await cite(page).first().evaluate((el) => el.querySelector("sup") !== null || getComputedStyle(el).verticalAlign)).toBeTruthy();

    // One undo step goes back to the style before.
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(cite(page)).toHaveText(["[1]", "[2]"]);
    // And Docxtor's own styles are still there.
    await styleSelect(page).selectOption("apa");
    await expect(cite(page)).toHaveText(["(Roe, 2019)", "(Park, 2018)"]);
  });

  test("a document remembers its CSL style when it is saved and opened again", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await styleSelect(page).selectOption("vancouver");
    await expect(cite(page)).toHaveText("[1]");
    const bytes = await inPage(page, (w) => Array.from(w.fe.editor.engine!.save()));
    await inPage(page, (w, b) => w.fe.open(new Uint8Array(b), "again.docx"), bytes);
    await expect.poll(() => style(page)).toBe("vancouver");
    await expect(cite(page)).toHaveText("[1]");
    // The style is added to the reopened document, so new citations use it.
    await openPanel(page);
    await expect(styleSelect(page)).toHaveValue("vancouver");
    await addBook(page, { title: "Sleep Well", family: "Park", year: "2018" });
    await expect.poll(() => inPage(page, (w) => (w.fe.editor.engine as unknown as { citationStyles(): { id: string }[] }).citationStyles().map((s) => s.id))).toEqual(["vancouver"]);
  });
});

test.describe("Chicago notes", () => {
  test("a citation is a footnote; the second is the short form and the next one is Ibid.", async ({ page }) => {
    const { first, second } = await start(page);
    await openPanel(page);
    await addBook(page);
    await addBook(page, { title: "Sleep Well", family: "Park", year: "2018", publisher: "Hachette" });
    await styleSelect(page).selectOption("chicago-nb-16");
    await expect.poll(() => style(page)).toBe("chicago-nb-16");

    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    // The text keeps only the note's mark.
    expect(await paraText(page, first)).toBe("Citing is a habit.1");
    await expect(page.locator(".flow .atom.note")).toHaveCount(1);
    expect(await noteTexts(page)).toEqual(["Jane Roe, The Art of Citing (Penguin, 2019)."]);
    await expect(noteCite(page).first()).toBeVisible();

    await caretAtEnd(page, second);
    await item(page, "Sleep Well").getByRole("button", { name: /^Cite/ }).click();
    await caretAtEnd(page, second);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await caretAtEnd(page, second);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    const notes = await noteTexts(page);
    expect(notes).toHaveLength(4);
    expect(notes[2]).toBe("Roe, The Art of Citing.");
    expect(notes[3]).toBe("Ibid.");
    // Each citation in a note is still a citation Word can edit.
    expect((await citations(page)).length).toBe(4);

    // The bibliography has the full entries, with the style's heading.
    await chooseMenu(page, "Insert", "Bibliography");
    const bib = page.locator(".flow .fc");
    await expect(bib).toContainText("Bibliography");
    await expect(bib).toContainText("Park, Jane. Sleep Well. Hachette, 2018.");
    await expect(bib).toContainText("Roe, Jane. The Art of Citing. Penguin, 2019.");
  });

  test("switching to a note style and back converts the citations in one step each", async ({ page }) => {
    const { first } = await start(page);
    await openPanel(page);
    await addBook(page);
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(cite(page)).toHaveText("(Roe, 2019)");

    await styleSelect(page).selectOption("chicago-nb-16");
    await expect(page.locator(".flow .atom.note")).toHaveCount(1);
    await expect(cite(page)).toHaveCount(0);
    expect(await paraText(page, first)).toBe("Citing is a habit.1");
    expect(await noteTexts(page)).toEqual(["Jane Roe, The Art of Citing (Penguin, 2019)."]);

    await styleSelect(page).selectOption("apa");
    await expect(page.locator(".flow .atom.note")).toHaveCount(0);
    await expect(cite(page)).toHaveText("(Roe, 2019)");
    expect(await noteTexts(page)).toEqual([]);

    // One undo step brings the note back.
    await page.keyboard.press("ControlOrMeta+Z");
    await expect(page.locator(".flow .atom.note")).toHaveCount(1);
  });

  test("clicking a citation in a note opens its popover; removing it takes the note away", async ({ page }) => {
    await start(page);
    await openPanel(page);
    await addBook(page);
    await styleSelect(page).selectOption("chicago-nb-16");
    await expect.poll(() => style(page)).toBe("chicago-nb-16");
    await item(page, "The Art of Citing").getByRole("button", { name: /^Cite/ }).click();
    await expect(noteCite(page).first()).toBeVisible();
    // A note is edited in place: double-click it, then click the citation.
    await page.locator(".sheet .notes").first().dblclick({ force: true });
    await expect(page.locator(".story-edit")).toBeVisible();
    await page.locator(".story-edit .cite").first().click();
    const pop = page.locator(".cite-pop");
    await expect(pop).toBeVisible();
    await pop.getByLabel("Page").fill("23");
    await pop.getByLabel("Page").press("Enter");
    expect(await noteTexts(page)).toEqual(["Jane Roe, The Art of Citing (Penguin, 2019), 23."]);
    await pop.getByRole("button", { name: "Remove citation" }).click();
    await expect(page.locator(".flow .atom.note")).toHaveCount(0);
    expect(await noteTexts(page)).toEqual([]);
    expect(await citations(page)).toEqual([]);
  });
});

test.describe("source providers", () => {
  test("the panel lists a button for each provider that is on, and turning one off removes it", async ({ page }) => {
    await start(page);
    await openPanel(page);
    for (const name of ["Fill in a form…", "From a DOI or URL…", "Import BibTeX…", "From Zotero…"]) await expect(panel(page).getByRole("button", { name })).toBeVisible();
    await inPage(page, (w) => w.fe.extensions.disable("docxtor.citations.doi"));
    await expect(panel(page).getByRole("button", { name: "From a DOI or URL…" })).toHaveCount(0);
    await expect(panel(page)).not.toContainText("sends the DOI to doi.org");
    await expect(panel(page).getByRole("button", { name: "Import BibTeX…" })).toBeVisible();
    await inPage(page, (w) => w.fe.extensions.enable("docxtor.citations.doi"));
    await expect(panel(page).getByRole("button", { name: "From a DOI or URL…" })).toBeVisible();
  });
});

const ITEMS = [
  {
    key: "ABCD1234",
    data: { itemType: "book", title: "The Art of Citing" },
    csljson: { id: "x", type: "book", title: "The Art of Citing", author: [{ family: "Roe", given: "Jane" }], publisher: "Penguin", issued: { "date-parts": [[2019]] } },
  },
  {
    key: "EFGH5678",
    data: { itemType: "journalArticle", title: "A study of things" },
    csljson: { id: "y", type: "article-journal", title: "A study of things", author: [{ family: "Smith", given: "John" }, { family: "Lee", given: "Karen" }], "container-title": "Journal of Stuff", issued: { "date-parts": [[2020, 5]] }, DOI: "10.1000/xyz123" },
  },
  { key: "ATT00001", data: { itemType: "attachment", title: "paper.pdf" } },
];

test.describe("Zotero", () => {
  test("searches Zotero on this computer as you type and adds the ticked items", async ({ page }) => {
    const asked: string[] = [];
    await page.route("http://localhost:23119/**", (route) => {
      const url = new URL(route.request().url());
      asked.push(url.pathname + url.search);
      const q = url.searchParams.get("q") ?? "";
      const body = q ? ITEMS.filter((i) => JSON.stringify(i).toLowerCase().includes(q.toLowerCase())) : ITEMS;
      return route.fulfill({ status: 200, contentType: "application/json", headers: { "access-control-allow-origin": "*" }, body: JSON.stringify(body) });
    });
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "From Zotero…" }).click();
    const z = dialog(page, "Add from Zotero");
    // The newest items are listed before anything is typed; attachments are not sources.
    await expect(z.locator(".src-found")).toHaveCount(2);
    expect(asked[0]).toContain("/api/users/0/items/top");
    expect(asked[0]).toContain("include=csljson");
    await expect(z.locator(".src-found").first()).toContainText("The Art of Citing");
    await expect(z.locator(".src-found").first()).toContainText("Roe, 2019");
    await expect(z.locator(".src-found").nth(1)).toContainText("Smith and Lee, 2020");
    await expect(z.getByRole("button", { name: "Add sources" })).toBeDisabled();

    await z.getByLabel("Search your Zotero library").fill("study");
    await expect(z.locator(".src-found")).toHaveCount(1);
    expect(asked.at(-1)).toContain("q=study");
    await z.locator(".src-found").first().getByRole("checkbox").check();
    await expect(z.getByRole("button", { name: "Add 1 source" })).toBeEnabled();

    // Another search keeps nothing ticked; tick two.
    await z.getByLabel("Search your Zotero library").fill("");
    await expect(z.locator(".src-found")).toHaveCount(2);
    await z.locator(".src-found").nth(0).getByRole("checkbox").check();
    await z.locator(".src-found").nth(1).getByRole("checkbox").check();
    await z.getByRole("button", { name: "Add 2 sources" }).click();
    await expect(z).toHaveCount(0);
    await expect(items(page)).toHaveCount(2);
    expect((await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).sources())).map((s) => s.title)).toEqual(["The Art of Citing", "A study of things"]);

    // Searching again, the two are already there.
    await panel(page).getByRole("button", { name: "From Zotero…" }).click();
    const again = dialog(page, "Add from Zotero");
    await expect(again.locator(".src-found.dup")).toHaveCount(2);
    await again.getByRole("button", { name: "Cancel" }).click();
  });

  test("says how to turn on Zotero's local API when it can't be reached", async ({ page }) => {
    await page.route("http://localhost:23119/**", (route) => route.abort("connectionrefused"));
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "From Zotero…" }).click();
    const z = dialog(page, "Add from Zotero");
    await expect(z.getByRole("alert")).toContainText("Couldn't reach Zotero on this computer");
    await expect(z.getByRole("alert")).toContainText("Allow other applications on this computer to communicate with Zotero");
  });

  test("searches the Zotero web library with a user ID and an API key kept in this browser", async ({ page }) => {
    const asked: { url: string; key: string | undefined }[] = [];
    await page.route("https://api.zotero.org/**", (route) => {
      asked.push({ url: route.request().url(), key: route.request().headers()["zotero-api-key"] });
      if (route.request().method() === "OPTIONS") return route.fulfill({ status: 204, headers: { "access-control-allow-origin": "*", "access-control-allow-headers": "*" } });
      return route.fulfill({ status: 200, contentType: "application/json", headers: { "access-control-allow-origin": "*" }, body: JSON.stringify([ITEMS[1]]) });
    });
    await start(page);
    await openPanel(page);
    await panel(page).getByRole("button", { name: "From Zotero…" }).click();
    const z = dialog(page, "Add from Zotero");
    await z.getByRole("button", { name: "Zotero settings…" }).click();
    const settings = dialog(page, "Zotero settings");
    await expect(settings.getByLabel("User ID")).toBeHidden();
    await settings.getByLabel("Zotero web library").check();
    await expect(settings.getByLabel("API key")).toHaveAttribute("type", "password");
    await expect(settings).toContainText("The key stays in this browser");
    await settings.getByRole("button", { name: "Save" }).click();
    // Without an ID and key it asks for them.
    await z.getByLabel("Search your Zotero library").fill("study");
    await expect(z.getByRole("alert")).toContainText("Enter your Zotero user ID and an API key");
    expect(asked).toEqual([]);

    await z.getByRole("button", { name: "Zotero settings…" }).click();
    const again = dialog(page, "Zotero settings");
    await expect(again.getByLabel("Zotero web library")).toBeChecked();
    await again.getByLabel("User ID").fill("987654");
    await again.getByLabel("API key").fill("secret-key");
    await again.getByRole("button", { name: "Save" }).click();
    await z.getByLabel("Search your Zotero library").fill("stud");
    await expect(z.locator(".src-found")).toHaveCount(1);
    await expect(z.locator(".src-found").first()).toContainText("A study of things");
    const call = asked.find((a) => a.url.includes("/users/987654/items/top"));
    expect(call).toBeTruthy();
    expect(call!.url).toContain("q=stud");
    // The key goes in a header, not in the address.
    expect(call!.url).not.toContain("secret-key");
    expect(call!.key).toBe("secret-key");
    await z.locator(".src-found").first().getByRole("checkbox").check();
    await z.getByRole("button", { name: "Add 1 source" }).click();
    await expect(items(page)).toHaveCount(1);
    expect((await inPage(page, (w) => (w.fe.editor.engine as unknown as Eng).sources()))[0].title).toBe("A study of things");
  });

  test("explains a refused key", async ({ page }) => {
    await page.route("https://api.zotero.org/**", (route) => route.fulfill({ status: 403, headers: { "access-control-allow-origin": "*" }, body: "Forbidden" }));
    await start(page);
    await inPage(page, (w) => {
      w.fe.extensions.list(); // the app is ready
    });
    await openPanel(page);
    await panel(page).getByRole("button", { name: "From Zotero…" }).click();
    const z = dialog(page, "Add from Zotero");
    await z.getByRole("button", { name: "Zotero settings…" }).click();
    const s = dialog(page, "Zotero settings");
    await s.getByLabel("Zotero web library").check();
    await s.getByLabel("User ID").fill("1");
    await s.getByLabel("API key").fill("bad");
    await s.getByRole("button", { name: "Save" }).click();
    await z.getByLabel("Search your Zotero library").fill("x");
    await expect(z.getByRole("alert")).toContainText("Zotero didn't accept that API key");
  });
});
