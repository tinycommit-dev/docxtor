// What a public launch needs: a backup of the whole Library, a plain message when the browser has no room left to save,
// a word when something nothing caught goes wrong, and a page that says what happened when Docxtor can't start.
import { readFileSync } from "node:fs";
import { chromium } from "@playwright/test";
import { chooseMenu, documentReady, expect, expectErrors, inPage, openFixture, test } from "./helpers";

test.describe("launch", () => {
  test("Download all documents saves every document of the Library in one .zip, in its folders", { tag: "@core" }, async ({ page }) => {
    await openFixture(page, "simple.docx");
    await inPage(page, async (w: any) => {
      await w.files.library.mkdir("Notes");
      await w.files.library.write("Notes/Second.docx", w.fe.editor.engine.save());
    });
    const [download] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "Download all documents")]);
    expect(download.suggestedFilename()).toMatch(/^Docxtor[ _]documents[ _]\d{4}-\d{2}-\d{2}\.zip$/); // WebKit on Linux turns the spaces into underscores
    const zip = readFileSync((await download.path())!);
    // Stored entries: names follow each local header (30 bytes in).
    const names: string[] = [];
    for (let i = 0; i + 30 < zip.length; ) {
      if (zip.readUInt32LE(i) !== 0x04034b50) break;
      const size = zip.readUInt32LE(i + 18);
      const nameLen = zip.readUInt16LE(i + 26);
      const extra = zip.readUInt16LE(i + 28);
      names.push(zip.subarray(i + 30, i + 30 + nameLen).toString("utf8"));
      i += 30 + nameLen + extra + size;
    }
    expect(names).toContain("Fixtures/simple.docx");
    expect(names).toContain("Notes/Second.docx");
    await expect(page.locator(".toast").last()).toContainText(/Downloaded \d+ documents/);
  });

  test("when the browser has no room left, a save says so and offers a copy to download", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.locator(".flow .fp").first().click();
    // The next write to the browser's storage is refused as the browser refuses it when the disk is full.
    await page.evaluate(() => {
      const put = IDBObjectStore.prototype.put;
      IDBObjectStore.prototype.put = function (this: IDBObjectStore, ...args: Parameters<typeof put>) {
        if (this.name === "library") throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
        return put.apply(this, args);
      };
    });
    await page.keyboard.type("more text");
    await inPage(page, (w: any) => w.save(true));
    const toast = page.locator(".toast", { hasText: "no room left in this browser" });
    await expect(toast).toBeVisible();
    await expect(toast.getByRole("button", { name: "Download a copy" })).toBeVisible();
    await expect(page.locator(".statusbar")).not.toContainText(/^Saved$/);
  });

  test("a write the browser drops when it commits it is not counted as saved", async ({ page }) => {
    await openFixture(page, "simple.docx");
    await page.locator(".flow .fp").first().click();
    // The request succeeds and then the transaction is aborted, as a full disk does at the commit.
    await page.evaluate(() => {
      const put = IDBObjectStore.prototype.put;
      IDBObjectStore.prototype.put = function (this: IDBObjectStore, ...args: Parameters<typeof put>) {
        const req = put.apply(this, args);
        if (this.name === "library") req.addEventListener("success", () => this.transaction.abort());
        return req;
      };
    });
    await page.keyboard.type("more text");
    await inPage(page, (w: any) => w.save(true));
    await expect.poll(() => inPage(page, (w: any) => w.saveState)).toBe("error");
  });

  test("an error nothing caught says that the work is kept, once, and a harmless notice says nothing", async ({ page }) => {
    await openFixture(page, "simple.docx");
    expectErrors(page, /Something broke on purpose/);
    await page.evaluate(() => {
      setTimeout(() => {
        throw new Error("Something broke on purpose");
      });
    });
    const said = page.locator(".toast", { hasText: "Something went wrong in Docxtor" });
    await expect(said).toBeVisible();
    await expect(said).toContainText("Your work is kept");
    await expect(said.getByRole("button", { name: "Reload" })).toBeVisible();
    // Chrome's notice about a resize loop is not a failure.
    await page.evaluate(() => window.dispatchEvent(new ErrorEvent("error", { message: "ResizeObserver loop completed with undelivered notifications." })));
    await expect(page.locator(".toast", { hasText: "Something went wrong in Docxtor" })).toHaveCount(1);
  });

  test("a browser that can't run Docxtor gets a page that says so, not a blank one", async ({ page }) => {
    expectErrors(page, /Docxtor could not start/);
    await page.addInitScript(() => void delete (window as unknown as { WebAssembly?: unknown }).WebAssembly);
    await page.goto("/");
    const box = page.getByRole("alert");
    await expect(box).toContainText("Docxtor couldn't start");
    await expect(box).toContainText("Your documents are safe");
    await expect(box).toContainText("WebAssembly");
    await expect(box.getByRole("button", { name: "Reload" })).toBeVisible();
  });

  test("on localhost, which the browser calls secure, nothing is said about HTTPS", async ({ page }) => {
    await openFixture(page, "simple.docx");
    expect(await page.evaluate(() => window.isSecureContext)).toBe(true);
    await page.waitForTimeout(500);
    await expect(page.locator(".toast", { hasText: "served over HTTPS" })).toHaveCount(0);
  });
});

// A copy of Docxtor on a home server is plain http on a network address, which the browser does not call secure (localhost does count). The name
// docxtor.test is mapped to this computer inside the browser itself (no network, no DNS), so the page really is not a secure context. The
// mapping is a browser launch option, which a test cannot change, so this test starts its own Chromium.
const PORT = Number(process.env.DOCXTOR_E2E_PORT ?? 5180);
test.describe("served without HTTPS", () => {
  test("says once what is switched off, and still opens, edits and keeps a document", async ({ browserName }) => {
    test.skip(browserName !== "chromium", "the host mapping is a Chromium switch");
    const browser = await chromium.launch({ args: ["--host-resolver-rules=MAP docxtor.test localhost"] });
    try {
      const page = await (await browser.newContext({ viewport: { width: 1280, height: 900 } })).newPage();
      const problems: string[] = [];
      page.on("pageerror", (e) => problems.push(e.message));
      page.on("console", (m) => m.type() === "error" && problems.push(m.text()));
      await page.goto(`http://docxtor.test:${PORT}/?open=simple.docx`);
      await documentReady(page);
      expect(await page.evaluate(() => window.isSecureContext)).toBe(false);
      const notice = page.locator(".toast", { hasText: "served over HTTPS" });
      await expect(notice).toBeVisible();
      await expect(notice).toContainText("working offline, the clipboard buttons");
      await expect(notice).toContainText("Serve it over HTTPS, or from localhost");
      // The document still takes an edit and is kept in the Library.
      await page.locator(".flow .fp").first().click();
      await page.keyboard.press("Home");
      await page.keyboard.type("Typed over plain http. ");
      await inPage(page, (w: any) => w.save(true));
      await expect(page.locator(".flow .fp").first()).toContainText("Typed over plain http.");
      expect(await inPage(page, async (w: any) => (await w.files.library.list("")).length)).toBeGreaterThan(0);
      // Version history records without crypto.subtle (which a non-secure page lacks).
      expect(await page.evaluate(() => typeof crypto.subtle)).toBe("undefined");
      const versions = () =>
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
      await expect.poll(versions).toBeGreaterThan(0);
      expect(problems).toEqual([]);
    } finally {
      await browser.close();
    }
  });
});
