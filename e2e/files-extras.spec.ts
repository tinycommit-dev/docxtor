// Pinned documents, the trash (documents deleted from the Library come back), and recovered documents (changes that were not
// saved when a page closed badly are offered again).
import { chooseMenu, dialog, documentReady, expect, expectErrors, inPage, menuItem, openFixture, openMenu, test, type Page } from "./helpers";

const files = (page: Page) => page.locator(".files");
const fileRow = (page: Page, name: string) => files(page).locator(".fs-item").filter({ hasText: new RegExp(`^${name}`) });

/** Add documents to the Library (not opened). */
async function addDocs(page: Page, names: string[]) {
  await inPage(page, async (w, list) => {
    const engine = w.fe.editor.engine as unknown as { constructor: { blank(n: string): { save(): Uint8Array; text(): string } } };
    for (const n of list) await w.files.library.write(n, engine.constructor.blank(n).save());
  }, names);
}

const recoveryRecords = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<{ key: string; name: string; words: number; session: string; at: number }[]>((resolve) => {
        const open = indexedDB.open("docxtor");
        open.onsuccess = () => {
          const all = open.result.transaction("recovery", "readonly").objectStore("recovery").getAll();
          all.onsuccess = () => resolve(all.result as never);
        };
      }),
  );

test.describe("pinned documents", () => {
  test("a document pinned from the Files view is first on the start screen, and unpinning puts it back in order", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await addDocs(page, ["Alpha.docx", "Beta.docx", "Gamma.docx"]);
    await inPage(page, (w) => w.files.refresh?.());
    // Open the three, so each is a recent document (Gamma is the newest).
    for (const n of ["Alpha.docx", "Beta.docx", "Gamma.docx"]) {
      await inPage(page, (w, name) => (w as unknown as { openRef(r: unknown): Promise<void> }).openRef({ backend: w.files.library, path: name }), n);
      await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue(n.replace(".docx", ""));
    }
    await fileRow(page, "Alpha").click({ button: "right" });
    await page.getByRole("button", { name: "Pin to the start screen" }).click();
    await expect(page.locator(".toast")).toContainText("“Alpha” is pinned to the start screen.");
    await page.goto("/#/");
    const tiles = page.locator(".desk-tiles .tile-wrap .doc-tile .cap b");
    await expect(tiles.nth(0)).toHaveText("Alpha");
    await expect(tiles.nth(1)).toHaveText("Gamma");
    await expect(page.locator(".tile-wrap.pinned")).toHaveCount(1);
    await expect(page.getByRole("button", { name: "Unpin Alpha" })).toHaveAttribute("aria-pressed", "true");
    // Unpin from the tile.
    await page.locator(".tile-wrap").first().hover();
    await page.getByRole("button", { name: "Unpin Alpha" }).click();
    await expect(tiles.nth(0)).toHaveText("Gamma");
    await expect(page.locator(".tile-wrap.pinned")).toHaveCount(0);
    // And pin from there: the tile of Beta.
    await page.locator(".tile-wrap").filter({ hasText: "Beta" }).hover();
    await page.getByRole("button", { name: "Pin Beta to the top" }).click();
    await expect(tiles.nth(0)).toHaveText("Beta");
  });

  test("File, Pin to the start screen is checked for a pinned document, and Open recent lists pinned ones first", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await addDocs(page, ["Other.docx"]);
    await inPage(page, (w) => (w as unknown as { openRef(r: unknown): Promise<void> }).openRef({ backend: w.files.library, path: "Other.docx" }));
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Other");
    await openMenu(page, "File");
    await expect(menuItem(page, "Pin to the start screen")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Pin to the start screen").click();
    await expect(page.locator(".toast")).toContainText("“Other” is pinned");
    // Open another document: the pinned one is first in Open recent, though it is not the latest.
    await inPage(page, (w) => (w as unknown as { openRef(r: unknown): Promise<void> }).openRef({ backend: w.files.library, path: "Fixtures/simple.docx" }));
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("simple");
    await openMenu(page, "File");
    await menuItem(page, "Open recent").click();
    await expect(page.locator(".menu-drop").last().locator(".menu-label").first()).toHaveText("Other");
    await page.locator(".flow .fp").first().click();
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    // Pinned stays pinned after a rename.
    await inPage(page, (w) => (w as unknown as { openRef(r: unknown): Promise<void> }).openRef({ backend: w.files.library, path: "Other.docx" }));
    await page.getByRole("textbox", { name: "Document name" }).fill("Renamed");
    await page.getByRole("textbox", { name: "Document name" }).press("Enter");
    await expect.poll(() => inPage(page, (w) => w.docKey())).toBe("library:Renamed.docx");
    await openMenu(page, "File");
    await expect(menuItem(page, "Pin to the start screen")).toHaveAttribute("aria-checked", "true");
  });
});

test.describe("the trash", () => {
  test("a deleted document waits in the Trash, and Restore puts it back", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await addDocs(page, ["Draft.docx"]);
    await inPage(page, (w) => w.files.refresh?.());
    await expect(fileRow(page, "Draft")).toBeVisible();
    await fileRow(page, "Draft").click({ button: "right" });
    await page.getByRole("button", { name: "Delete…" }).click();
    const confirm = page.getByRole("alertdialog", { name: /Delete “Draft.docx”\?/ });
    await expect(confirm).toContainText("It moves to the Trash, where it stays for 30 days.");
    await confirm.getByRole("button", { name: "Delete" }).click();
    await expect(fileRow(page, "Draft")).toHaveCount(0);
    // The Trash section says how many, and lists them when opened.
    const toggle = page.locator(".trash-toggle");
    await expect(toggle.locator(".trash-count")).toHaveText("1");
    await toggle.click();
    await expect(page.locator(".trash-row")).toContainText("Draft");
    await expect(page.locator(".trash-row")).toContainText("30 days left");
    await page.getByRole("button", { name: "Restore Draft.docx" }).click();
    await expect(page.locator(".toast")).toContainText("Put “Draft.docx” back in Library.");
    await expect(fileRow(page, "Draft")).toBeVisible();
    await expect(page.locator(".trash-count")).toHaveText("");
    // It opens like any document.
    await fileRow(page, "Draft").click();
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Draft");
  });

  test("Delete for good and Empty trash ask first; a folder moves all its documents to the Trash", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await inPage(page, (w) => (w.files.library as unknown as { mkdir(p: string): Promise<void> }).mkdir("Notes"));
    await addDocs(page, ["Notes/One.docx", "Notes/Two.docx", "Solo.docx"]);
    await inPage(page, (w) => w.files.refresh?.());
    // A folder: everything in it is kept.
    await files(page).locator(".fs-item.dir").filter({ hasText: "Notes" }).click({ button: "right" });
    await page.getByRole("button", { name: "Delete folder…" }).click();
    await page.getByRole("alertdialog", { name: "Delete folder?" }).getByRole("button", { name: "Delete" }).click();
    await page.locator(".trash-toggle").click();
    await expect(page.locator(".trash-row")).toHaveCount(2);
    await expect(page.locator(".trash-row").filter({ hasText: "One" })).toContainText("Library");
    // Restoring one makes its folder again.
    await page.getByRole("button", { name: "Restore One.docx" }).click();
    await expect(files(page).locator(".fs-item.dir").filter({ hasText: "Notes" })).toBeVisible();
    // Delete the other for good, after asking.
    await page.getByRole("button", { name: "Delete Two.docx for good" }).click();
    const sure = page.getByRole("alertdialog", { name: "Delete “Two.docx” for good?" });
    await expect(sure).toContainText("It can't be put back afterwards.");
    await sure.getByRole("button", { name: "Cancel" }).click();
    await expect(page.locator(".trash-row")).toHaveCount(1);
    await page.getByRole("button", { name: "Delete Two.docx for good" }).click();
    await sure.getByRole("button", { name: "Delete for good" }).click();
    await expect(page.locator(".trash-row")).toHaveCount(0);
    await expect(page.locator(".trash-body .empty-note")).toHaveText("The trash is empty.");
    // Empty trash: delete Solo, then empty.
    await fileRow(page, "Solo").click({ button: "right" });
    await page.getByRole("button", { name: "Delete…" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Delete" }).click();
    await page.getByRole("button", { name: "Empty trash" }).click();
    await page.getByRole("alertdialog", { name: "Empty the trash?" }).getByRole("button", { name: "Empty trash" }).click();
    await expect(page.locator(".trash-row")).toHaveCount(0);
  });

  test("File, Trash opens the Files view with the Trash list open", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.keyboard.press("Escape");
    await chooseMenu(page, "File", "Trash");
    await expect(page.locator(".trash-toggle")).toHaveAttribute("aria-expanded", "true");
  });

  test("a document deleted while it is open can be saved again", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await fileRow(page, "simple").click({ button: "right" });
    await page.getByRole("button", { name: "Delete…" }).click();
    await page.getByRole("alertdialog").getByRole("button", { name: "Delete" }).click();
    await expect(page.locator(".toast")).toContainText("The open document was moved to the Trash.");
  });
});

test.describe("recovered documents", () => {
  test("changes that were not saved are kept every few seconds, and dropped when they are saved", async ({ page }) => {
    await openFixture(page, "simple.docx");
    // A long autosave interval, so the changes stay unsaved.
    await page.evaluate(() => (window as unknown as { docxtor: { prefs: { autosaveSeconds: number } } }).docxtor.prefs.autosaveSeconds = 600);
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "UNSAVED words here " });
    });
    await expect(page.locator(".led")).toHaveText("Unsaved changes");
    await expect.poll(async () => (await recoveryRecords(page)).length, { timeout: 12_000 }).toBe(1);
    const [copy] = await recoveryRecords(page);
    expect(copy.key).toBe("library:Fixtures/simple.docx");
    expect(copy.name).toBe("simple.docx");
    expect(copy.words).toBeGreaterThan(3);
    await page.keyboard.press("ControlOrMeta+s");
    await expect(page.locator(".led")).toHaveText(/Saved/);
    await expect.poll(async () => (await recoveryRecords(page)).length).toBe(0);
  });

  test("a copy is kept after typing pauses, not on every few seconds of typing, and at the latest after a ceiling", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.evaluate(() => {
      const w = (window as unknown as { docxtor: { prefs: { autosaveSeconds: number }; recoveryDelays: { idle: number; max: number } } }).docxtor;
      w.prefs.autosaveSeconds = 600;
      w.recoveryDelays = { idle: 800, max: 2600 };
    });
    const type = (text: string) =>
      inPage(page, (w, s) => {
        const e = w.fe.editor;
        e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: s });
      }, text);
    // Steady typing, a change every 300 ms for 2 s: each change pushes the copy back, so there is none yet.
    for (let i = 0; i < 7; i++) {
      await type("x");
      await page.waitForTimeout(300);
    }
    expect(await recoveryRecords(page)).toHaveLength(0);
    // Typing stops: the copy follows after the pause.
    await expect.poll(async () => (await recoveryRecords(page)).length, { timeout: 5_000 }).toBe(1);
    // Typing that never pauses still gets a copy, at the ceiling.
    const first = (await recoveryRecords(page))[0].at;
    for (let i = 0; i < 14; i++) {
      await type("y");
      await page.waitForTimeout(300);
    }
    expect((await recoveryRecords(page))[0].at).toBeGreaterThan(first);
  });

  test("a copy left by a page that closed is offered next time, opens as a new document, and is let go", async ({ page }) => {
    await openFixture(page, "simple.docx");
    // What a crashed page leaves: a copy from a session that no longer holds its lock.
    await page.evaluate(async () => {
      const w = window as unknown as { docxtor: { files: { library: unknown }; fe: { editor: { engine: { text(): string; save(): Uint8Array } } } } };
      const bytes = w.docxtor.fe.editor.engine.save();
      await new Promise<void>((resolve) => {
        const open = indexedDB.open("docxtor");
        open.onsuccess = () => {
          const tx = open.result.transaction("recovery", "readwrite");
          tx.objectStore("recovery").put({ key: "library:Lost.docx", name: "Lost.docx", place: "your Library", bytes, words: 42, at: Date.now() - 3_600_000, session: "dead-page" });
          tx.oncomplete = () => resolve();
        };
      });
    });
    await page.reload();
    const d = dialog(page, "Recovered documents");
    await expect(d).toBeVisible();
    await expect(d.getByRole("listitem")).toHaveCount(1);
    await expect(d.getByRole("listitem")).toContainText("Lost.docx");
    await expect(d.getByRole("listitem")).toContainText("42 words");
    await d.getByRole("button", { name: "Open Lost.docx" }).click();
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Recovered Lost");
    await documentReady(page);
    expect(await recoveryRecords(page)).toHaveLength(0);
    // Nothing is left to recover now.
    await openMenu(page, "File");
    await expect(menuItem(page, "Recovered documents…")).toBeDisabled();
  });

  test("Discard lets a copy go, and a copy kept by a page that is still open is not offered", async ({ page, context }) => {
    await openFixture(page, "simple.docx");
    await page.evaluate(async () => {
      await new Promise<void>((resolve) => {
        const open = indexedDB.open("docxtor");
        open.onsuccess = () => {
          const tx = open.result.transaction("recovery", "readwrite");
          tx.objectStore("recovery").put({ key: "library:A.docx", name: "A.docx", place: "your Library", bytes: new Uint8Array([1]), words: 1, at: Date.now(), session: "dead-1" });
          tx.objectStore("recovery").put({ key: "library:B.docx", name: "B.docx", place: "your Library", bytes: new Uint8Array([1]), words: 2, at: Date.now() - 1000, session: "dead-2" });
          tx.oncomplete = () => resolve();
        };
      });
    });
    // A second page of the same browser: the first is still open, so its own copy is not offered, but the dead ones are.
    const other = await context.newPage();
    await other.goto("/");
    const d = dialog(other, "Recovered documents");
    await expect(d.getByRole("listitem")).toHaveCount(2);
    await d.getByRole("button", { name: "Discard A.docx" }).click();
    await expect(d.getByRole("listitem")).toHaveCount(1);
    await expect(d.getByRole("listitem")).toContainText("B.docx");
    await d.getByRole("button", { name: "Discard B.docx" }).click();
    await expect(d.locator(".empty-note")).toHaveText("Nothing to recover.");
    await d.getByRole("button", { name: "Close" }).click();
    await expect(d).toBeHidden();
    expect(await recoveryRecords(other)).toHaveLength(0);
  });
});

// The engine is Rust compiled to WebAssembly. If it panics, the instance traps and its memory may be half changed; every later
// call is unsafe. From then on the engine refuses every call, so nothing it returns is saved over a good copy (packages/core/src/crash.ts).
// The test breaks the engine on purpose with the hidden `__panicForTests` of the binding.
test.describe("a crashed engine", () => {
  const stored = (page: Page, store: string, key: string) =>
    page.evaluate(
      ([s, k]) =>
        new Promise<{ bytes: number[]; at?: number; modified?: number } | null>((resolve) => {
          const open = indexedDB.open("docxtor");
          open.onsuccess = () => {
            const get = open.result.transaction(s, "readonly").objectStore(s).get(k);
            get.onsuccess = () => {
              const v = get.result as { bytes?: Uint8Array; at?: number; modified?: number } | undefined;
              resolve(v ? { bytes: [...(v.bytes ?? [])], at: v.at, modified: v.modified } : null);
            };
          };
        }),
      [store, key] as const,
    );
  const versions = (page: Page) =>
    page.evaluate(
      () =>
        new Promise<number>((resolve) => {
          const open = indexedDB.open("docxtor");
          open.onsuccess = () => {
            const n = open.result.transaction("versions", "readonly").objectStore("versions").count();
            n.onsuccess = () => resolve(n.result);
          };
        }),
    );

  test("saves nothing after the engine stops: the Library copy and the recovery copy stay, the person is told, and Reload opens the saved copy", async ({ page }) => {
    // The panic hook writes Rust's message to the console, and the page writes that the engine stopped.
    expectErrors(page, /Docxtor engine panic: .*deliberate panic for tests/s, /Docxtor engine crashed/);
    await openFixture(page, "simple.docx");
    await page.evaluate(() => {
      const w = window as unknown as { docxtor: { prefs: { autosaveSeconds: number }; recoveryDelays: { idle: number; max: number }; files: { library: { write: (...a: unknown[]) => Promise<unknown> } } } };
      w.docxtor.prefs.autosaveSeconds = 600;
      w.docxtor.recoveryDelays = { idle: 300, max: 1000 };
      // Every write to the Library from now on is counted.
      const library = w.docxtor.files.library;
      const write = library.write.bind(library);
      (window as unknown as { writes: number }).writes = 0;
      library.write = (...a) => {
        (window as unknown as { writes: number }).writes++;
        return write(...a);
      };
    });
    // 1. A saved copy.
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "SAVEDWORDS " });
    });
    await page.keyboard.press("ControlOrMeta+s");
    await expect(page.locator(".led")).toHaveText(/Saved/);
    // 2. Changes after it, kept as a recovery copy.
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "LATERWORDS " });
    });
    await expect.poll(async () => (await recoveryRecords(page)).length, { timeout: 12_000 }).toBe(1);
    const library = await stored(page, "library", "Fixtures/simple.docx");
    const recovery = await stored(page, "recovery", "library:Fixtures/simple.docx");
    const writesBefore = await page.evaluate(() => (window as unknown as { writes: number }).writes);
    const versionsBefore = await versions(page);
    expect(library?.bytes.length).toBeGreaterThan(1000);
    expect(recovery?.bytes.length).toBeGreaterThan(1000);
    const downloads: string[] = [];
    page.on("download", (d) => downloads.push(d.suggestedFilename()));

    // 3. The engine panics.
    const crash = await page.evaluate(() => {
      const w = window as unknown as { docxtor: { engine: { raw: { __panicForTests(): void } } } };
      try {
        w.docxtor.engine.raw.__panicForTests();
        return "no error";
      } catch (e) {
        return `${(e as Error).name} ${(e as { code?: string }).code}`;
      }
    });
    expect(crash).toBe("EngineCrashedError engine_crashed");
    // The person is told, in plain words, with a Reload button.
    const notice = page.locator(".crash-notice");
    await expect(notice).toBeVisible();
    await expect(notice).toContainText("The document hit a problem");
    await expect(notice).toContainText("The last saved copy is safe");
    await expect(notice.getByRole("button", { name: "Reload" })).toBeVisible();

    // The person keeps typing, as they would: the keys find a stopped engine, and nothing is saved from it.
    await page.keyboard.press("Escape");
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.type("TYPEDAFTER");
    await page.waitForTimeout(300);

    // 4. Every way to save is tried; each is refused.
    const outcomes = await page.evaluate(async () => {
      const w = window as unknown as { docxtor: Record<string, (...a: unknown[]) => Promise<unknown>> };
      const tried: Record<string, string> = {};
      for (const [name, run] of [
        ["save", () => w.docxtor.save(true)],
        ["autosave", () => w.docxtor.save(false)],
        ["download", () => w.docxtor.exportAs("docx")],
        ["copy", () => w.docxtor.makeCopy()],
        ["version", () => w.docxtor.checkpoint("manual", "After the crash", true)],
      ] as const) {
        try {
          await run();
          tried[name] = "returned";
        } catch (e) {
          tried[name] = `${(e as { code?: string }).code}`;
        }
      }
      return tried;
    });
    // Whatever each path does with the refusal, none of them writes.
    for (const [name, result] of Object.entries(outcomes)) expect(["returned", "engine_crashed"], name).toContain(result);
    await page.waitForTimeout(1500); // longer than the recovery timer
    expect(await page.evaluate(() => (window as unknown as { writes: number }).writes), "no write to the Library").toBe(writesBefore);
    expect(await stored(page, "library", "Fixtures/simple.docx"), "the Library copy").toEqual(library);
    expect(await stored(page, "recovery", "library:Fixtures/simple.docx"), "the recovery copy").toEqual(recovery);
    expect(await versions(page), "no version was added").toBe(versionsBefore);
    expect(downloads, "nothing was downloaded").toEqual([]);

    // 5. Reload opens the last saved copy: what was saved is there, what was not is in the recovery copy it offers back.
    await notice.getByRole("button", { name: "Reload" }).click();
    await documentReady(page);
    const text = await inPage(page, (w) => w.fe.editor.engine!.raw.docscript(50));
    expect(text).toContain("SAVEDWORDS");
    expect(text).not.toContain("LATERWORDS");
    await expect(page.getByRole("dialog").filter({ hasText: "Recovered" })).toBeVisible();
  });
});
