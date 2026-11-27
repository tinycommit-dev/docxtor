// Saving is safe: an edit made while a save is writing is written too, the page-close warning shows while changes are not
// saved, the side-by-side pane saves through the same queue and is covered by the warning, and opening another document
// while the window is split (in any view) loads it. The engine keeps a revision number; "saved" means the revision that
// was written is the revision now.
import { chooseMenu, expect, inPage, LINE_START, test, type Page } from "./helpers";
import { chooseLayout } from "./support/view-helpers";

/** The parts of the app these tests reach into. */
interface App {
  dirty: boolean;
  prefs: { autosaveSeconds: number };
  files: { library: { write(path: string, bytes: Uint8Array): Promise<void>; read(path: string): Promise<Uint8Array> } };
  engine: { revision: number } | null;
  createIn(backend: unknown, dir: string, name: string, md?: string): Promise<void>;
  openRef(ref: { backend: unknown; path: string }): Promise<void>;
  side: { on: boolean; key: string | null; unsaved: boolean; open(doc: unknown): Promise<boolean> };
}

async function start(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w) => {
    (w as unknown as App).prefs.autosaveSeconds = 1;
  });
}

async function create(page: Page, name: string, text: string) {
  await inPage(page, (w, a) => (w as unknown as App).createIn((w as unknown as App).files.library, "", a.name, a.text), { name, text });
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue(name.replace(/\.docx$/, ""));
  await expect(page.locator(".flow")).toContainText(text.slice(0, 20));
}

/** Make the Library's writes take `ms` (or fail), and keep the text the statuses went through. */
async function slowLibrary(page: Page, mode: { ms: number; fail?: boolean }) {
  await inPage(
    page,
    (w, m) => {
      const lib = (w as unknown as App).files.library as unknown as { write(p: string, b: Uint8Array): Promise<void>; __orig?: (p: string, b: Uint8Array) => Promise<void> };
      lib.__orig ??= lib.write.bind(lib);
      lib.write = async (p, b) => {
        await new Promise((r) => setTimeout(r, m.ms));
        if (m.fail) throw new Error("the disk is full");
        return lib.__orig!(p, b);
      };
    },
    mode,
  );
}

async function normalLibrary(page: Page) {
  await inPage(page, (w) => {
    const lib = (w as unknown as App).files.library as unknown as { write: unknown; __orig?: unknown };
    if (lib.__orig) lib.write = lib.__orig;
  });
}

/** Does closing the page ask first? (The page's own handler decides; the event is not a real close.) */
const closeAsks = (page: Page) =>
  page.evaluate(() => {
    const e = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(e);
    return e.defaultPrevented;
  });

const savedText = (page: Page, path: string) =>
  inPage(
    page,
    async (w, p) => {
      const Engine = (w as unknown as { engine: { constructor: unknown } }).engine!.constructor as unknown as { open(b: Uint8Array, n: string): { renderAll(): { blocks: { segs?: { x?: string }[] }[] }; dispose(): void } };
      const e = Engine.open(await (w as unknown as App).files.library.read(p), p);
      const text = e.renderAll().blocks.map((b) => (b.segs ?? []).map((s) => s.x ?? "").join("")).join("\n");
      e.dispose();
      return text;
    },
    path,
  );

test.describe("saving", () => {
  test("an edit made while a save is writing is saved too, and the status says unsaved until it is", async ({ page }) => {
    await start(page);
    await create(page, "Slow.docx", "Start of the document");
    // Record every status the document went through.
    await page.evaluate(() => {
      const seen: string[] = [];
      (window as unknown as { __seen: string[] }).__seen = seen;
      const led = document.querySelector(".led")!;
      new MutationObserver(() => seen.push(led.textContent ?? "")).observe(led, { childList: true, characterData: true, subtree: true });
    });
    await slowLibrary(page, { ms: 2000 });
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.type(" first");
    await expect(page.locator(".led")).toHaveText("Saving…", { timeout: 10000 });
    // Typed while the write runs.
    await page.keyboard.type(" second");
    // The write holding "first" ends; the document is not saved yet, and the second edit is written next.
    await expect(page.locator(".led")).toHaveText("Saved in your Library", { timeout: 20000 });
    expect(await inPage(page, (w) => (w as unknown as App).dirty)).toBe(false);
    expect(await savedText(page, "Slow.docx")).toContain("Start of the document first second");
    const seen = await page.evaluate(() => (window as unknown as { __seen: string[] }).__seen);
    // "Saved" shows once, at the end: never while the second edit was waiting.
    expect(seen.filter((s) => s.startsWith("Saved")).length).toBe(1);
    expect(seen.at(-1)).toBe("Saved in your Library");
    await normalLibrary(page);
  });

  test("closing the page asks while changes are not saved, and not once they are", async ({ page }) => {
    await start(page);
    await create(page, "Close.docx", "Some words to start with.");
    expect(await closeAsks(page)).toBe(false);
    await slowLibrary(page, { ms: 2500 });
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.type(" more");
    await expect(page.locator(".led")).toHaveText("Saving…", { timeout: 10000 });
    expect(await closeAsks(page), "a write is running").toBe(true);
    await page.keyboard.type(" and more");
    expect(await closeAsks(page), "and a newer edit waits behind it").toBe(true);
    await expect(page.locator(".led")).toHaveText("Saved in your Library", { timeout: 20000 });
    expect(await closeAsks(page)).toBe(false);
    await normalLibrary(page);
  });

  test("a save that fails keeps the warning and says so; a later save clears both", async ({ page }) => {
    await start(page);
    await create(page, "Fails.docx", "A document that can't be written.");
    await slowLibrary(page, { ms: 10, fail: true });
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.type(" edit");
    await expect(page.locator(".led")).toHaveText("Couldn't save", { timeout: 10000 });
    expect(await closeAsks(page)).toBe(true);
    await normalLibrary(page);
    await page.keyboard.press("ControlOrMeta+s");
    await expect(page.locator(".led")).toHaveText("Saved in your Library", { timeout: 10000 });
    expect(await closeAsks(page)).toBe(false);
    expect(await savedText(page, "Fails.docx")).toContain("edit");
  });

  test("undoing back to the saved text is still a change that needs saving", async ({ page }) => {
    await start(page);
    await create(page, "Undo.docx", "Words that stay.");
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.type("X");
    await expect(page.locator(".led")).toHaveText("Saved in your Library", { timeout: 10000 });
    const saved = await inPage(page, (w) => (w as unknown as App).engine!.revision);
    await page.keyboard.press("ControlOrMeta+z");
    expect(await inPage(page, (w) => (w as unknown as App).engine!.revision)).toBeGreaterThan(saved);
    await expect(page.locator(".led")).toHaveText("Saved in your Library", { timeout: 10000 });
    expect(await savedText(page, "Undo.docx")).not.toContain("X");
  });
});

test.describe("opening another document while the window is split", () => {
  for (const layout of ["Draft", "Reading", "Pageless", "Outline"]) {
    test(`with the top pane in ${layout}, the other document loads and the split is gone`, async ({ page }) => {
      await start(page);
      await create(page, "First.docx", "First document, a line of text.");
      await create(page, "Second.docx", "Second document, a line of text.");
      await chooseMenu(page, "View", "Window", "Split");
      await expect(page.locator(".flow")).toHaveCount(2);
      await chooseLayout(page, layout);
      await inPage(page, (w) => (w as unknown as App).openRef({ backend: (w as unknown as App).files.library, path: "First.docx" }));
      await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("First");
      await expect(page.locator(".flow")).toHaveCount(1);
      await expect(page.locator(".flow")).toContainText("First document");
      // And the window still works.
      await chooseLayout(page, "Print layout");
      await expect(page.locator(".flow")).toContainText("First document");
    });
  }
});

test.describe("side by side saves like the open document", () => {
  async function twoDocs(page: Page) {
    await start(page);
    await create(page, "Notes.docx", "Notes paragraph one, long enough to be a line.");
    await create(page, "Report.docx", "Report paragraph one, long enough to be a line.");
  }
  const openNotesBeside = (page: Page) =>
    inPage(page, async (w) => {
      const a = (w as unknown as App);
      return a.side.open({ key: "library:Notes.docx", name: "Notes.docx", bytes: await a.files.library.read("Notes.docx"), write: (b: Uint8Array) => a.files.library.write("Notes.docx", b) });
    });

  test("an edit in the pane is saved, and closing the page asks until it is", async ({ page }) => {
    await twoDocs(page);
    expect(await openNotesBeside(page)).toBe(true);
    const pane = page.locator(".sbs-pane");
    await expect(pane.locator(".flow")).toContainText("Notes paragraph one");
    await pane.locator(".flow [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Edited ");
    // Straight after typing, before the pause that saves: closing the page asks.
    expect(await closeAsks(page)).toBe(true);
    await expect(pane.locator(".sbs-status")).toHaveText("Saved", { timeout: 10000 });
    expect(await closeAsks(page)).toBe(false);
    expect(await savedText(page, "Notes.docx")).toContain("Edited Notes paragraph one");
  });

  test("an edit made while the pane's save is writing is saved too", async ({ page }) => {
    await twoDocs(page);
    expect(await openNotesBeside(page)).toBe(true);
    const pane = page.locator(".sbs-pane");
    await slowLibrary(page, { ms: 1500 });
    await pane.locator(".flow [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("One ");
    await expect(pane.locator(".sbs-status")).toHaveText("Saving…", { timeout: 10000 });
    await page.keyboard.type("Two ");
    await expect(pane.locator(".sbs-status")).toHaveText("Saved", { timeout: 20000 });
    expect(await savedText(page, "Notes.docx")).toContain("One Two Notes paragraph one");
    await normalLibrary(page);
  });

  test("closing the pane while its save fails keeps it open, with the changes", async ({ page }) => {
    await twoDocs(page);
    expect(await openNotesBeside(page)).toBe(true);
    const pane = page.locator(".sbs-pane");
    await slowLibrary(page, { ms: 10, fail: true });
    await pane.locator(".flow [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Lost? ");
    await expect(pane.locator(".sbs-status")).toContainText("Couldn't save", { timeout: 10000 });
    expect(await closeAsks(page)).toBe(true);
    await pane.getByRole("button", { name: "Close" }).click();
    await expect(page.getByText("so it stays open beside this document")).toBeVisible();
    await expect(pane).toBeVisible();
    await normalLibrary(page);
    await pane.getByRole("button", { name: "Close" }).click();
    await expect(pane).toHaveCount(0);
    expect(await savedText(page, "Notes.docx")).toContain("Lost? Notes paragraph one");
  });

  test("the document that is open in the main editor is not opened a second time beside it", async ({ page }) => {
    await twoDocs(page);
    const opened = await inPage(page, async (w) => {
      const a = (w as unknown as App);
      return a.side.open({ key: a.side.key ?? "library:Report.docx", name: "Report.docx", bytes: await a.files.library.read("Report.docx"), write: (b: Uint8Array) => a.files.library.write("Report.docx", b) });
    });
    expect(opened).toBe(false);
    await expect(page.getByText("already open in the main window")).toBeVisible();
    await expect(page.locator(".sbs-pane")).toHaveCount(0);
  });

  test("opening in the main editor the document that is beside it saves and closes the pane", async ({ page }) => {
    await twoDocs(page);
    expect(await openNotesBeside(page)).toBe(true);
    const pane = page.locator(".sbs-pane");
    await pane.locator(".flow [data-p]").first().click();
    await page.keyboard.press(LINE_START);
    await page.keyboard.type("Beside ");
    await inPage(page, (w) => (w as unknown as App).openRef({ backend: (w as unknown as App).files.library, path: "Notes.docx" }));
    await expect(pane).toHaveCount(0);
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Notes");
    // The main editor read the file after the pane was saved, so the edit is there.
    await expect(page.locator(".flow")).toContainText("Beside Notes paragraph one");
    await expect(page.getByText("is open here now")).toBeVisible();
  });
});
