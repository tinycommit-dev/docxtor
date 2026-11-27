// The database every Docxtor tab shares is upgraded by one build at a time: a tab on the old build lets go when a newer one asks,
// and a tab that has to wait says so in words instead of showing an empty start screen for ever.
import { expect, expectErrors, inPage, openFixture, test } from "./helpers";

test.describe("the browser database across tabs", () => {
  test("a tab lets go of the database when a newer Docxtor needs it, and says to reload", async ({ page }) => {
    await openFixture(page, "simple.docx");
    // A newer build asks for a higher version while this page holds its connection.
    const opened = await page.evaluate(
      () =>
        new Promise<number>((resolve, reject) => {
          const req = indexedDB.open("docxtor", 4);
          req.onsuccess = () => {
            const v = req.result.version;
            req.result.close();
            resolve(v);
          };
          req.onblocked = () => reject(new Error("blocked: the page did not let go"));
          req.onerror = () => reject(req.error);
        }),
    );
    expect(opened).toBe(4);
    await expect(page.locator(".toast")).toHaveText(/Docxtor was updated in another tab\. Reload this page/);
    // The page doesn't pretend it can still reach the database.
    const error = await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { files: { library: { list(p: string): Promise<unknown> } } } }).docxtor;
      try {
        await w.files.library.list("");
        return "";
      } catch (e) {
        return (e as Error).message;
      }
    });
    expect(error).toMatch(/reload this page/);
  });

  test("a tab that let go with unsaved changes says how to keep them and asks before it is closed", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.evaluate(() => {
      const w = window as unknown as { docxtor: { prefs: { autosaveSeconds: number } } };
      w.docxtor.prefs.autosaveSeconds = 600;
    });
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "Unsaved " });
    });
    await expect(page.locator(".led")).toHaveText("Unsaved changes");
    // Closing now would save to the Library, so the page does not hold the closing up.
    const closing = () => page.evaluate(() => {
      const ev = new Event("beforeunload", { cancelable: true });
      window.dispatchEvent(ev);
      return ev.defaultPrevented;
    });
    expect(await closing()).toBe(false);
    await inPage(page, (w) => {
      const e = w.fe.editor;
      e.exec({ op: "insert_text", at: { p: e.engine!.renderAll().order[0], o: 0 }, text: "More " });
    });
    await page.evaluate(
      () =>
        new Promise<void>((resolve, reject) => {
          const req = indexedDB.open("docxtor", 4);
          req.onsuccess = () => {
            req.result.close();
            resolve();
          };
          req.onblocked = () => reject(new Error("blocked"));
          req.onerror = () => reject(req.error);
        }),
    );
    // The toast says to download a copy first, and closing the page asks, since the changes can't be saved to the browser now.
    await expect(page.locator(".toast")).toHaveText(/Download a copy to keep your changes, then reload/);
    expect(await closing()).toBe(true);
    // The document is still whole in the page, so a copy can be made from it.
    expect(await inPage(page, (w) => w.fe.editor.engine!.save().length > 1000)).toBe(true);
  });

  test("a tab waiting on an older tab says so, and carries on when that tab closes its connection", async ({ page, browserName }) => {
    // An address of the same site that is not the app: it holds the database the way a tab on the old build does (no versionchange handler).
    // That address is a 404 on purpose (only its origin matters), and Chrome and Safari log the 404 as an error (Firefox does not).
    if (browserName !== "firefox") expectErrors(page, /status of 404 .*__built\/holder/);
    await page.goto("/__built/holder");
    await page.evaluate(
      () =>
        new Promise<void>((resolve, reject) => {
          const del = indexedDB.deleteDatabase("docxtor");
          del.onsuccess = del.onblocked = () => {
            const req = indexedDB.open("docxtor", 2);
            req.onupgradeneeded = () => {
              for (const name of ["library", "handles", "recent", "versions", "blobs"]) req.result.createObjectStore(name, name === "library" ? { keyPath: "path" } : name === "recent" ? { keyPath: "key" } : undefined);
            };
            req.onsuccess = () => {
              (window as unknown as { held: IDBDatabase }).held = req.result;
              resolve();
            };
            req.onerror = () => reject(req.error);
          };
          del.onerror = () => reject(del.error);
        }),
    );
    const app = await page.context().newPage();
    await app.goto("/");
    await expect(app.locator(".toast")).toHaveText(/Docxtor is updating\. Close your other Docxtor tabs/);
    // The old tab closes: the new one finishes the upgrade and works.
    await page.evaluate(() => (window as unknown as { held: IDBDatabase }).held.close());
    await app.waitForFunction(() => !!(window as unknown as { docxtor?: { engine?: unknown } }).docxtor);
    const names = await app.evaluate(
      () =>
        new Promise<string[]>((resolve, reject) => {
          const req = indexedDB.open("docxtor");
          req.onsuccess = () => {
            const n = [...req.result.objectStoreNames];
            req.result.close();
            resolve(n);
          };
          req.onerror = () => reject(req.error);
        }),
    );
    expect(names).toEqual(expect.arrayContaining(["library", "trash", "recovery", "templates"]));
  });
  // The other way round: this page is the older build, and the database was already upgraded by a newer one (a rollback, or a
  // tab that kept the old build after an update). The browser refuses to open a database at a lower version than it has.
  test("an older build that meets a database from a newer Docxtor says so in words, and leaves the data as it was", async ({ page, browserName }) => {
    if (browserName !== "firefox") expectErrors(page, /status of 404 .*__built\/holder/);
    await page.goto("/__built/holder");
    const stored = () =>
      page.evaluate(
        () =>
          new Promise<{ version: number; stores: string[]; library: { path: string; bytes: number[] }[]; future: unknown[] }>((resolve, reject) => {
            const req = indexedDB.open("docxtor");
            req.onerror = () => reject(req.error);
            req.onsuccess = () => {
              const db = req.result;
              const names = [...db.objectStoreNames];
              const tx = db.transaction(["library", "future"], "readonly");
              const lib = tx.objectStore("library").getAll();
              const fut = tx.objectStore("future").getAll();
              tx.oncomplete = () => {
                db.close();
                resolve({
                  version: db.version,
                  stores: names,
                  library: (lib.result as { path: string; bytes: Uint8Array }[]).map((e) => ({ path: e.path, bytes: [...e.bytes] })),
                  future: fut.result,
                });
              };
            };
          }),
      );
    // A database one version past the one this build knows (3), with a document in it and a store this build has never heard of.
    await page.evaluate(
      () =>
        new Promise<void>((resolve, reject) => {
          const del = indexedDB.deleteDatabase("docxtor");
          del.onsuccess = () => {
            const req = indexedDB.open("docxtor", 4);
            req.onupgradeneeded = () => {
              const db = req.result;
              db.createObjectStore("library", { keyPath: "path" }).put({ path: "From the future.docx", kind: "file", bytes: new Uint8Array([80, 75, 3, 4, 9, 9, 9]), modified: 1, size: 7 });
              for (const name of ["handles", "blobs"]) db.createObjectStore(name);
              db.createObjectStore("recent", { keyPath: "key" });
              db.createObjectStore("versions", { keyPath: "id", autoIncrement: true }).createIndex("doc", "doc");
              for (const name of ["trash", "templates"]) db.createObjectStore(name, { keyPath: "id", autoIncrement: true });
              db.createObjectStore("recovery", { keyPath: "key" });
              db.createObjectStore("future", { keyPath: "id" }).put({ id: 1, note: "only a newer Docxtor knows this" });
            };
            req.onsuccess = () => {
              req.result.close();
              resolve();
            };
            req.onerror = () => reject(req.error);
          };
          del.onerror = () => reject(del.error);
        }),
    );
    const before = await stored();
    expect(before.version).toBe(4);
    expect(before.library).toHaveLength(1);

    // This build opens at version 3: it says why it can't go on, once, and offers the reload.
    await page.goto("/");
    const message = page.locator(".newer-data");
    await expect(message).toBeVisible();
    await expect(message).toContainText("Your documents were saved by a newer Docxtor");
    await expect(message).toContainText("Nothing was changed or deleted");
    await expect(message.getByRole("button", { name: "Reload" })).toBeVisible();
    // No crash loop: the page settles, and a second look finds the same page.
    await page.waitForTimeout(1500);
    await expect(message).toHaveCount(1);
    // The start screen and the editor are not behind it, so nothing can be saved from here.
    await expect(page.locator(".flow")).toHaveCount(0);

    // Reload: the same page again, still without touching anything.
    await message.getByRole("button", { name: "Reload" }).click();
    await expect(page.locator(".newer-data")).toBeVisible();

    // The data is exactly as the newer build left it: same version, same stores, the document's bytes, the store only it knows.
    await page.goto("/__built/holder");
    expect(await stored()).toEqual(before);
  });
});
