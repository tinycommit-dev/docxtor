// Password-protected documents: opening asks for the password, saving keeps the file protected, and File,
// Protect with password sets, changes or removes it. The vectors in fixtures/encrypted/ were made by other software
// (fixtures/encrypted/make_vectors.py); their password is "docxtor-test-pw".
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { answer, chooseMenu, dialog, documentReady, expect, inPage, openFixture, type Page, paraText, RICH, test } from "./helpers";

const PASSWORD = "docxtor-test-pw";
const SECRET = "xyzzy-secret-7";
const CFB = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
const FIXTURE = "Fixtures/encrypted-agile.docx";

const askDialog = (page: Page) => dialog(page, "Password needed");
const protectDlg = (page: Page) => dialog(page, "Protect with password");
const saved = (page: Page) => page.locator(".led");

/** The first bytes of a file in the Library. */
const head = (page: Page, path: string, n = 8) => inPage(page, async (w, a) => Array.from((await w.files.library.read(a.path)).slice(0, a.n)), { path, n });

/** What the open document says about its password. */
const state = (page: Page) => inPage(page, (w) => (w.engine as unknown as { hasPassword: boolean }).hasPassword);

/** Whether any file or version kept in the browser is a plain .docx zip (a protected file has no zip signature in it). */
const storedZip = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<boolean>((done) => {
        const open = indexedDB.open("docxtor");
        open.onsuccess = () => {
          const db = open.result;
          const isZip = (v: unknown): boolean => {
            if (v instanceof Uint8Array || v instanceof ArrayBuffer) {
              const b = new Uint8Array(v as ArrayBuffer);
              for (let i = 0; i + 3 < b.length; i++) if (b[i] === 0x50 && b[i + 1] === 0x4b && b[i + 2] === 3 && b[i + 3] === 4) return true;
              return false;
            }
            return !!v && typeof v === "object" && Object.values(v).some(isZip);
          };
          const stores = ["library", "blobs"];
          let pending = stores.length;
          let zip = false;
          for (const s of stores) {
            const all = db.transaction(s, "readonly").objectStore(s).getAll();
            all.onsuccess = () => {
              if (all.result.some(isZip)) zip = true;
              if (--pending === 0) done(zip);
            };
          }
        };
      }),
  );

async function unlock(page: Page, password = PASSWORD) {
  const ask = askDialog(page);
  await expect(ask).toBeVisible();
  await ask.getByLabel("Password").fill(password);
  await ask.getByRole("button", { name: "Open" }).click();
}

test.describe("password-protected documents", () => {
  test("asks for the password, refuses a wrong one inside the same dialog, then opens", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    const ask = askDialog(page);
    await expect(ask).toBeVisible();
    await expect(ask).toContainText("This document is protected with a password. Enter it to open it.");
    await expect(ask).toContainText("encrypted-agile.docx");
    await expect(ask.getByLabel("Password")).toHaveAttribute("type", "password");
    await expect(ask.getByRole("button", { name: "Cancel" })).toBeVisible();

    await ask.getByLabel("Password").fill("not the password");
    await ask.getByLabel("Password").press("Enter");
    await expect(ask.getByRole("alert")).toHaveText("That password isn't right. Try again.");
    await expect(ask).toBeVisible();
    // Typing again clears the message.
    await ask.getByLabel("Password").fill("x");
    await expect(ask.getByRole("alert")).toBeHidden();

    await ask.getByLabel("Password").fill(PASSWORD);
    await ask.getByRole("button", { name: "Open" }).click();
    await expect(ask).toHaveCount(0);
    await documentReady(page);
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain("marmalade");
    expect(await state(page)).toBe(true);
    await expect(page.locator(".lock-mark")).toBeVisible();
    await expect(page).toHaveURL(/#\/library\/Fixtures\/encrypted-agile\.docx$/);
    // The address and the page carry no trace of the password.
    expect(page.url()).not.toContain(PASSWORD);
  });

  test("edits save protected, and the next visit asks again", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    await unlock(page);
    await documentReady(page);
    expect(await head(page, FIXTURE)).toEqual(CFB);

    await inPage(page, (w) => w.fe.editor.exec({ op: "replace_text", find: "marmalade", replace: "quince", all: true }));
    await expect(saved(page)).toHaveText("Saved in your Library");
    // What is in the Library is still a protected file, and so is every version kept of it.
    expect(await head(page, FIXTURE)).toEqual(CFB);
    await expect.poll(() => storedZip(page)).toBe(false);

    await page.reload();
    await expect(askDialog(page)).toBeVisible();
    await unlock(page);
    await documentReady(page);
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain("quince");
  });

  test("Cancel goes back to the document that was open", async ({ page }) => {
    await openFixture(page);
    const bytes = Array.from(await readFile(resolve(process.cwd(), "fixtures/encrypted/encrypted-agile.docx")));
    await inPage(page, (w, b) => w.files.library.write("Locked.docx", new Uint8Array(b)), bytes);
    await page.evaluate(() => (location.hash = "#/library/Locked.docx"));
    const ask = askDialog(page);
    await expect(ask).toBeVisible();
    await expect(ask).toContainText("Locked.docx");
    await ask.getByRole("button", { name: "Cancel" }).click();
    await expect(ask).toHaveCount(0);
    await expect(page).toHaveURL(/#\/library\/Fixtures\/rich\.docx$/);
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("rich");
    await expect.poll(() => paraText(page, RICH.title)).toBe("Quarterly Product Report");
    // Escape does the same.
    await page.evaluate(() => (location.hash = "#/library/Locked.docx"));
    await expect(ask).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(ask).toHaveCount(0);
    await expect(page).toHaveURL(/#\/library\/Fixtures\/rich\.docx$/);
  });

  test("Cancel on a file opened straight from its address shows your documents", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    await askDialog(page).getByRole("button", { name: "Cancel" }).click();
    await expect(page.locator(".start")).toBeVisible();
    await expect(page).toHaveURL(/#\/$/);
  });

  test("Standard encryption from Word 2007 and AES-128 files open too", async ({ page }) => {
    for (const name of ["encrypted-standard.docx", "encrypted-agile-aes128-sha1.docx"]) {
      await page.goto(`/?open=encrypted/${name}`);
      await unlock(page);
      await documentReady(page);
      await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain("marmalade");
    }
  });

  test("File, Protect with password sets a password and the file saves protected", async ({ page }) => {
    await openFixture(page);
    expect(await head(page, "Fixtures/rich.docx", 2)).toEqual([0x50, 0x4b]);
    await chooseMenu(page, "File", "Protect with password…");
    const dlg = protectDlg(page);
    await expect(dlg).toBeVisible();
    // A plain document has no password to remove.
    await expect(dlg.getByRole("button", { name: "Remove password" })).toHaveCount(0);
    await expect(dlg.getByLabel("New password")).toHaveAttribute("type", "password");
    await expect(dlg.getByLabel("Confirm password")).toHaveAttribute("type", "password");

    await dlg.getByRole("button", { name: "Protect" }).click();
    await expect(dlg.getByRole("alert")).toHaveText("Type a password.");
    await dlg.getByLabel("New password").fill(SECRET);
    await dlg.getByLabel("Confirm password").fill("something else");
    await dlg.getByRole("button", { name: "Protect" }).click();
    await expect(dlg.getByRole("alert")).toHaveText("The passwords don't match.");
    await expect(dlg).toBeVisible();

    await dlg.getByLabel("Confirm password").fill(SECRET);
    await dlg.getByRole("button", { name: "Protect" }).click();
    await expect(dlg).toHaveCount(0);
    expect(await state(page)).toBe(true);
    await expect(page.locator(".lock-mark")).toBeVisible();
    await expect.poll(() => head(page, "Fixtures/rich.docx")).toEqual(CFB);

    // Nothing the app keeps in the browser holds the password.
    const leaks = await page.evaluate(async (secret) => {
      const found: string[] = [];
      for (const [k, v] of Object.entries(localStorage)) if (k.includes(secret) || v.includes(secret)) found.push(`localStorage ${k}`);
      if (location.href.includes(secret)) found.push("address");
      const db = await new Promise<IDBDatabase>((ok) => {
        const r = indexedDB.open("docxtor");
        r.onsuccess = () => ok(r.result);
      });
      for (const s of Array.from(db.objectStoreNames)) {
        const all = await new Promise<unknown[]>((ok) => {
          const r = db.transaction(s, "readonly").objectStore(s).getAll();
          r.onsuccess = () => ok(r.result);
        });
        if (JSON.stringify(all, (_k, v) => (v instanceof Uint8Array || v instanceof ArrayBuffer ? undefined : v)).includes(secret)) found.push(`indexedDB ${s}`);
      }
      return found;
    }, SECRET);
    expect(leaks).toEqual([]);

    await page.reload();
    const ask = askDialog(page);
    await expect(ask).toBeVisible();
    await ask.getByLabel("Password").fill(PASSWORD);
    await ask.getByRole("button", { name: "Open" }).click();
    await expect(ask.getByRole("alert")).toHaveText("That password isn't right. Try again.");
    await ask.getByLabel("Password").fill(SECRET);
    await ask.getByRole("button", { name: "Open" }).click();
    await documentReady(page);
    await expect.poll(() => paraText(page, RICH.title)).toBe("Quarterly Product Report");
  });

  test("the dialog changes the password, and Remove password saves the file plain again", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    await unlock(page);
    await documentReady(page);

    // Change it.
    await chooseMenu(page, "File", "Protect with password…");
    let dlg = protectDlg(page);
    await expect(dlg.getByRole("button", { name: "Change password" })).toBeVisible();
    await dlg.getByLabel("New password").fill("second password");
    await dlg.getByLabel("Confirm password").fill("second password");
    await dlg.getByRole("button", { name: "Change password" }).click();
    await expect(dlg).toHaveCount(0);
    await expect(saved(page)).toHaveText("Saved in your Library");
    await page.reload();
    await unlock(page);
    await expect(askDialog(page).getByRole("alert")).toHaveText("That password isn't right. Try again.");
    await askDialog(page).getByLabel("Password").fill("second password");
    await askDialog(page).getByRole("button", { name: "Open" }).click();
    await documentReady(page);
    expect(await head(page, FIXTURE)).toEqual(CFB);

    // Remove it.
    await chooseMenu(page, "File", "Protect with password…");
    dlg = protectDlg(page);
    await dlg.getByRole("button", { name: "Remove password" }).click();
    await answer(page, "Remove password");
    await expect(dlg).toHaveCount(0);
    expect(await state(page)).toBe(false);
    await expect(page.locator(".lock-mark")).toBeHidden();
    await expect.poll(() => head(page, FIXTURE, 2)).toEqual([0x50, 0x4b]);
    await page.reload();
    await documentReady(page);
    await expect(askDialog(page)).toHaveCount(0);
    await expect.poll(() => inPage(page, (w) => w.engine!.text())).toContain("marmalade");
  });

  test("Cancel in the Protect dialog changes nothing", async ({ page }) => {
    await openFixture(page);
    await chooseMenu(page, "File", "Protect with password…");
    const dlg = protectDlg(page);
    await dlg.getByLabel("New password").fill("abc");
    await dlg.getByRole("button", { name: "Cancel" }).click();
    await expect(dlg).toHaveCount(0);
    expect(await state(page)).toBe(false);
    expect(await head(page, "Fixtures/rich.docx", 2)).toEqual([0x50, 0x4b]);
  });

  test("a copy of a protected document is protected and opens without asking again", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    await unlock(page);
    await documentReady(page);
    await chooseMenu(page, "File", "Make a copy");
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Copy of encrypted-agile");
    await expect(askDialog(page)).toHaveCount(0);
    expect(await head(page, "Fixtures/Copy of encrypted-agile.docx")).toEqual(CFB);
    expect(await state(page)).toBe(true);
  });

  test("the start screen shows a padlock for a protected document and keeps none of its text", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    await unlock(page);
    await documentReady(page);
    await expect(saved(page)).toHaveText("Saved in your Library");
    await page.evaluate(() => (location.hash = "#/"));
    const tile = page.locator(".doc-tile.locked");
    await expect(tile).toHaveCount(1);
    await expect(tile).toContainText("encrypted-agile");
    await expect(tile.locator(".page")).toContainText("Protected with a password");
    await expect(tile.locator(".page")).not.toContainText("marmalade");
    await expect(tile.locator(".page")).not.toContainText("Protected note");

    // Clicking it asks for the password.
    await tile.click();
    await expect(askDialog(page)).toBeVisible();
    await unlock(page);
    await documentReady(page);
  });

  test("Download, Word document of a protected document is protected", async ({ page }) => {
    await page.goto("/?open=encrypted/encrypted-agile.docx");
    await unlock(page);
    await documentReady(page);
    const [file] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "Word document (.docx)")]);
    const bytes = await readFile((await file.path())!);
    expect(Array.from(bytes.subarray(0, 8))).toEqual(CFB);
  });

  test("Tools, Compare documents with a protected file asks for its password", async ({ page }) => {
    await openFixture(page);
    const bytes = Array.from(await readFile(resolve(process.cwd(), "fixtures/encrypted/encrypted-agile.docx")));
    await inPage(page, (w, b) => w.files.library.write("Locked.docx", new Uint8Array(b)), bytes);
    await chooseMenu(page, "Tools", "Compare documents…");
    await page.locator(".qp-item", { hasText: "Locked" }).click();
    await expect(askDialog(page)).toContainText("Locked.docx");
    await unlock(page);
    const view = dialog(page, "Compare documents").filter({ has: page.locator(".cmp-list") });
    await expect(view).toBeVisible();
    await expect(view.locator(".cmp-count")).not.toBeEmpty();
    await expect(view).toContainText("Protected note");
  });

  test("the check for stored zips does find a plain document", async ({ page }) => {
    await openFixture(page);
    expect(await storedZip(page)).toBe(true);
  });

  test("a document opened without a password keeps no lock", async ({ page }) => {
    await openFixture(page);
    await expect(page.locator(".lock-mark")).toBeHidden();
    expect(await state(page)).toBe(false);
  });
});
