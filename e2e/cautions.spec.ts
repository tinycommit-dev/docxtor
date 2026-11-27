// Warnings before risky actions (DESIGN.md, Risky actions; the wording is in builtins/cautions.ts and
// web/src/cautions.ts). What can't be undone, or changes the whole document, asks first and says what changes; a
// local delete is done at once with Undo; Help, Open the tour in a new tab leaves the document where it is.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, openFixture, test, type Page } from "./helpers";

type Host = { execute(id: string): Promise<unknown>; command(id: string): unknown; cautions: { list(): { id: string; kind: string }[] } };
const host = (page: Page) => (window as unknown as { docxtor: { fe: { extensions: Host } } }).docxtor.fe.extensions;
const run = (page: Page, id: string) => page.evaluate((cid) => void (window as unknown as { docxtor: { fe: { extensions: Host } } }).docxtor.fe.extensions.execute(cid), id);
const question = (page: Page) => page.getByRole("alertdialog");
const status = (page: Page, id: string) => page.evaluate((cid) => (window as unknown as { docxtor: { fe: { extensions: { execute(id: string): Promise<{ status: string }> } } } }).docxtor.fe.extensions.execute(cid).then((r) => r.status), id);

test("removing the header asks first, says it goes from every page, and Cancel keeps it", async ({ page }) => {
  await openBuilt(page, { sections: [{ headers: { default: "Running head" }, paragraphs: ["Body text"] }] });
  await expect(page.locator(".sheet .hf.hdr").first()).toContainText("Running head");
  await page.locator(".flow .fp").first().click();
  await run(page, "insert.removeHeader");
  await expect(question(page)).toContainText("Remove the header?");
  await expect(question(page)).toContainText("It goes from every page of this section.");
  // Cancel has the focus, and the button names the action.
  await expect(question(page).getByRole("button", { name: "Cancel" })).toBeFocused();
  await question(page).getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator(".sheet .hf.hdr").first()).toContainText("Running head");
  await run(page, "insert.removeHeader");
  await question(page).getByRole("button", { name: "Remove header" }).click();
  await expect(page.locator(".sheet .hf.hdr")).toHaveCount(0);
  // Now there is no header: the command is unavailable and asks nothing (the menu greys it out).
  expect(await status(page, "insert.removeHeader")).toBe("unavailable");
  await expect(question(page)).toHaveCount(0);
});

test("a document with no header or footer has nothing to remove: the commands ask nothing", async ({ page }) => {
  await openBuilt(page, { sections: [{ paragraphs: ["Body text"] }] });
  await page.locator(".flow .fp").first().click();
  expect(await status(page, "insert.removeHeader")).toBe("unavailable");
  expect(await status(page, "insert.removeFooter")).toBe("unavailable");
  await expect(question(page)).toHaveCount(0);
});

test("deleting a table is done at once, with Undo in the notice", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await expect(page.locator(".flow table")).toHaveCount(1);
  await page.locator(".flow td").first().click();
  await run(page, "table.deleteTable");
  await expect(question(page)).toHaveCount(0);
  await expect(page.locator(".flow table")).toHaveCount(0);
  const notice = page.locator(".toast");
  await expect(notice).toContainText("Table deleted.");
  await notice.getByRole("button", { name: "Undo" }).click();
  await expect(page.locator(".flow table")).toHaveCount(1);
});

// Rule 1.7: a button that does what a command does runs the command, so it asks, offers Undo and answers the same way.
test("the strip over a header asks before removing it, and the header stays when the answer is no", async ({ page }) => {
  await openBuilt(page, { sections: [{ headers: { default: "Running head" }, paragraphs: ["Body text"] }] });
  await page.locator(".flow .fp").first().click();
  await run(page, "insert.header");
  const remove = page.locator(".story-strip").getByRole("button", { name: "Remove header" });
  await remove.click();
  await expect(question(page)).toContainText("Remove the header?");
  await question(page).getByRole("button", { name: "Cancel" }).click();
  // Still editing the header, and the header is still there.
  await expect(page.locator(".story-strip")).toBeVisible();
  await remove.click();
  await question(page).getByRole("button", { name: "Remove header" }).click();
  await expect(page.locator(".sheet .hf.hdr")).toHaveCount(0);
});

test("Remove in the table of contents dialog asks first, as the Insert menu does", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await chooseMenu(page, "Insert", "Page elements", "Table of contents");
  await expect(page.locator('.flow [data-fld="TOC"]').first()).toBeVisible();
  await page.evaluate(() => {
    const w = (window as unknown as { docxtor: { fe: { editor: { select(p: { p: number; o: number }): void; engine: { tocAt(): number } } } } }).docxtor;
    w.fe.editor.select({ p: w.fe.editor.engine.tocAt(), o: 0 });
  });
  await chooseMenu(page, "Insert", "Page elements", "Table of contents options…");
  await dialog(page, "Table of contents").getByRole("button", { name: "Remove table of contents" }).click();
  await expect(question(page)).toContainText("Remove the table of contents?");
  await question(page).getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator('.flow [data-fld="TOC"]').first()).toBeVisible();
});

test("deleting a table from the toolbar offers Undo, and Undo does not take back later typing", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await page.locator(".flow td").first().click();
  await page.getByRole("button", { name: "Table", exact: true }).first().click();
  await page.getByRole("button", { name: "Delete table" }).click();
  await expect(page.locator(".flow table")).toHaveCount(0);
  const notice = page.locator(".toast");
  await expect(notice).toContainText("Table deleted.");
  // Type a word: the notice's Undo no longer belongs to the delete, so it does nothing.
  await page.keyboard.type("zz");
  await notice.getByRole("button", { name: "Undo" }).click({ timeout: 2000 }).catch(() => undefined);
  await expect(page.locator(".flow table")).toHaveCount(0);
  await expect(page.locator(".flow")).toContainText("zz");
});

test("a command with nothing to act on says it is not available, and no notice offers Undo over an error", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await page.locator(".flow .fp").first().click();
  const answer = await page.evaluate(async () => {
    const h = (window as unknown as { docxtor: { fe: { extensions: { execute(id: string): Promise<{ status: string }> } } } }).docxtor.fe.extensions;
    return (await h.execute("table.deleteRow")).status;
  });
  expect(answer).toBe("unavailable");
  await expect(page.locator(".toast")).toHaveCount(0);
});

test("the assistant is told when the person said no", async ({ page }) => {
  await openBuilt(page, { sections: [{ headers: { default: "Running head" }, paragraphs: ["Body text"] }] });
  await page.locator(".flow .fp").first().click();
  const run1 = () =>
    page.evaluate(async () => {
      const h = (window as unknown as { docxtor: { fe: { extensions: { aiTools: { list(): { name: string; run(i: Record<string, unknown>): Promise<string> }[] } } } } }).docxtor.fe.extensions;
      return h.aiTools.list().find((x) => x.name === "run_command")!.run({ command: "insert.removePageNumbers" });
    });
  const told = run1();
  await question(page).getByRole("button", { name: "Cancel" }).click();
  expect(await told).toMatch(/said no/);
});

test("what can't be undone says so: resetting every shortcut", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await chooseMenu(page, "Tools", "Customize keyboard shortcuts…");
  await dialog(page, "Keyboard shortcuts").getByRole("button", { name: "Reset all" }).click();
  await expect(question(page)).toContainText("Reset all shortcuts?");
  await expect(question(page)).toContainText("This can't be undone.");
  await question(page).getByRole("button", { name: "Cancel" }).click();
  await expect(question(page)).toHaveCount(0);
  await expect(dialog(page, "Keyboard shortcuts")).toBeVisible();
});

test("every warning for a command names a command that exists", async ({ page }) => {
  await openFixture(page, "rich.docx");
  const missing = await page.evaluate(() => {
    const h = (window as unknown as { docxtor: { fe: { extensions: Host } } }).docxtor.fe.extensions;
    // Buttons in dialogs ask by names of their own; the rest are commands.
    const dialogs = new Set(["inspect.removeAll", "style.delete", "mailMerge.makeLabels", "mailMerge.makeEnvelope", "shortcuts.resetAll", "dictionary.remove", "quickParts.delete", "files.delete", "files.deleteFolder", "trash.empty", "trash.deleteForever", "history.restore", "password.remove", "citations.deleteSource", "templates.delete", "citations.forgetShared", "citations.removeStyle"]);
    return h.cautions.list().filter((c) => !dialogs.has(c.id) && !h.command(c.id)).map((c) => c.id);
  });
  expect(missing).toEqual([]);
  void host;
});

test.describe("Help, Open the tour in a new tab", () => {
  const welcomes = (page: Page) => inPage(page, async (w) => (await w.files.library.list("")).filter((n) => n.name.startsWith("Welcome to Docxtor")).map((n) => n.name));

  test("opens the tour in a new tab and leaves the document where it is; an unchanged tour is opened again, not copied", async ({ page, context }) => {
    await openFixture(page, "rich.docx");
    const before = await page.locator(".flow").textContent();
    for (let i = 0; i < 2; i++) {
      const [tab] = await Promise.all([context.waitForEvent("page"), chooseMenu(page, "Help", "Open the tour in a new tab")]);
      await tab.waitForLoadState();
      await expect(tab.locator(".flow")).toContainText("Welcome to Docxtor");
      await tab.close();
    }
    // No question on the way, the document is untouched, and there is one tour.
    await expect(page.getByRole("dialog")).toHaveCount(0);
    expect(await page.locator(".flow").textContent()).toBe(before);
    expect(await welcomes(page)).toEqual(["Welcome to Docxtor.docx"]);
  });

  test("when the browser won't open a tab, the tour opens here and says the document is saved", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await page.evaluate(() => (window.open = () => null));
    await chooseMenu(page, "Help", "Open the tour in a new tab");
    await expect(page.locator(".flow")).toContainText("Welcome to Docxtor");
    await expect(page.locator(".toast")).toContainText("“rich” is saved");
  });

  test("when the browser won't open a tab and the document can't be saved, it stays open and says so", async ({ page }) => {
    await openFixture(page, "rich.docx");
    await page.locator(".flow .fp").first().click();
    await page.keyboard.type("Unsaved words ");
    await page.evaluate(() => {
      window.open = () => null;
      // Writing this document fails; the Library still takes the tour.
      const w = window as unknown as { docxtor: { ref: { path: string; backend: { write(path: string, b: Uint8Array): Promise<void> } } } };
      const { backend, path } = w.docxtor.ref;
      const write = backend.write.bind(backend);
      backend.write = (p, b) => (p === path ? Promise.reject(new Error("the disk is full")) : write(p, b));
    });
    await chooseMenu(page, "Help", "Open the tour in a new tab");
    await expect(page.locator(".toast")).toContainText("couldn't be saved, so it stays open");
    await expect(page.locator(".flow")).toContainText("Unsaved words");
    await expect(page.locator(".flow")).not.toContainText("Welcome to Docxtor");
  });
});
