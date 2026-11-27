// The core of editing, for Firefox and Safari (Playwright's WebKit). The first release says Chrome and Edge are
// supported fully and Firefox and Safari for the core of editing: the core set is every test tagged `@core`
// (a tag, not a list of files, so a spec can hold both a core test and a Chrome-only one). Run it with
// `npm run test:e2e:core` (Chromium, Firefox and WebKit). These tests fill the steps no older spec covered:
// a file from the computer, deleting, selecting, a table, composing text with an input method, and Download
// then reopen. docs/browsers.md has the rest.
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { openBuilt } from "./support/docx-builder";
import { caretAtEnd, chooseMenu, documentReady, expect, inPage, openFixture, paraText, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";
const fixtures = resolve(process.cwd(), "fixtures");
const line = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().replace(/\n$/, ""));
const firstId = (page: Page) => page.locator(".flow .fp").first().evaluate((el) => Number((el as HTMLElement).dataset.p));

/** Choose File, Download, Word document and read the file the browser saves. */
async function downloadDocx(page: Page) {
  const [file] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "Word document (.docx)")]);
  return { name: file.suggestedFilename(), bytes: await readFile((await file.path())!) };
}

test.describe("the core of editing, in every browser", () => {
  test("a file from the computer opens, as the Open dialog's file input hands it over", { tag: "@core" }, async ({ page }) => {
    await page.goto("/?open=simple.docx");
    await documentReady(page);
    await page.locator("#app > input[type=file]").setInputFiles({ name: "from-disk.docx", mimeType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", buffer: await readFile(resolve(fixtures, "rich.docx")) });
    await expect(page.locator(".flow")).toContainText("Quarterly Product Report");
    await expect(page.locator(".flow")).toContainText("Final paragraph on page two.");
  });

  test("shift and the arrows select, and Backspace and Delete take the selection and single characters away", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Alpha beta gamma"] }] });
    const id = await firstId(page);
    await caretAtEnd(page, id);
    await page.keyboard.press("Shift+ArrowLeft");
    await page.keyboard.press("Shift+ArrowLeft");
    await page.keyboard.press("Shift+ArrowLeft");
    await page.keyboard.press("Shift+ArrowLeft");
    await page.keyboard.press("Shift+ArrowLeft");
    await page.keyboard.press("Backspace");
    await expect.poll(() => paraText(page, id)).toBe("Alpha beta ");
    // Back to the start with the arrow keys (Home and End mean different things on a Mac and elsewhere).
    for (let i = 0; i < 11; i++) await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("Delete");
    await expect.poll(() => paraText(page, id)).toBe("lpha beta ");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => paraText(page, id)).toBe("Alpha beta ");
  });

  test("Insert, Table makes a table, Tab goes to the next cell, and one undo takes it away", { tag: "@core" }, async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Before the table"] }] });
    await caretAtEnd(page, await firstId(page));
    await chooseMenu(page, "Insert", "Table");
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    const cells = page.locator(".flow table.ft td");
    await cells.first().click();
    await page.keyboard.type("one");
    await page.keyboard.press("Tab");
    await page.keyboard.type("two");
    await expect(cells.nth(0)).toContainText("one");
    await expect(cells.nth(1)).toContainText("two");
    await page.keyboard.press(`${MOD}+Z`);
    await page.keyboard.press(`${MOD}+Z`);
    await page.keyboard.press(`${MOD}+Z`);
    await expect(page.locator(".flow table.ft")).toHaveCount(0);
  });

  test("text composed with an input method lands in the document once, as the browser reports it", { tag: "@core" }, async ({ page }) => {
    // Playwright can't drive a real input method outside Chromium, so this stands in for one: the events an
    // input method makes (start, the text drawn into the page while composing, end with the final text).
    await openBuilt(page, { sections: [{ paragraphs: ["Start"] }] });
    const id = await firstId(page);
    await caretAtEnd(page, id);
    await page.evaluate(() => {
      const flow = document.querySelector<HTMLElement>(".flow")!;
      flow.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true, data: "" }));
      const sel = getSelection()!;
      const node = sel.anchorNode!;
      const host = node.nodeType === 3 ? (node as Text) : null;
      const text = document.createTextNode("ni");
      if (host) host.parentNode!.insertBefore(text, host.nextSibling);
      else node.appendChild(text);
      flow.dispatchEvent(new CompositionEvent("compositionupdate", { bubbles: true, data: "ni" }));
      flow.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "你好" }));
    });
    await expect.poll(() => paraText(page, id)).toBe("Start你好");
    await expect.poll(() => line(page)).toBe("Start你好");
    // Typing goes on after it, and one undo takes the composed text back out.
    await page.keyboard.type("!");
    await expect.poll(() => paraText(page, id)).toBe("Start你好!");
  });

  test("Download as Word saves the edited document, and the file opens again with the same text and the same bytes on the next save", { tag: "@core" }, async ({ page }) => {
    await page.goto("/?open=rich.docx");
    await documentReady(page);
    const id = Number(await page.locator(".flow .fp").last().getAttribute("data-p"));
    await caretAtEnd(page, id);
    await page.keyboard.type(" Typed in the test.");
    await expect.poll(() => line(page)).toContain("Typed in the test.");
    const first = await downloadDocx(page);
    expect(first.name).toBe("rich.docx");
    expect(first.bytes.length).toBeGreaterThan(1000);
    // A .docx is a zip: it starts with PK.
    expect(first.bytes.subarray(0, 2).toString("latin1")).toBe("PK");

    // Open it again as a file from the computer.
    await page.locator("#app > input[type=file]").setInputFiles({ name: "again.docx", mimeType: "application/octet-stream", buffer: first.bytes });
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("again");
    await expect.poll(() => line(page)).toContain("Typed in the test.");
    const second = await downloadDocx(page);
    expect(second.bytes.equals(first.bytes), "saving what was just opened, unchanged, gives the same file").toBe(true);
  });

  test("a document with over 1 MB of text to compress is compressed in the save worker, which shares the page's engine, and saves whole", { tag: "@core" }, async ({ page }) => {
    await countWorkers(page);
    // A file whose parts are compressed (the fixtures are), with 1.2 MB more text: the save worker is made only when 1 MB or more is to be compressed.
    await openFixture(page, "rich.docx");
    const words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua".split(" ");
    let x = 7;
    // 1,800 paragraphs of 100 words (about 300 pages, so far pages are left out of the layout, in every browser).
    const para = () => Array.from({ length: 100 }, () => words[(x = (x * 1103515245 + 12345) >>> 0) % words.length]).join(" ");
    const markdown = Array.from({ length: 1800 }, para).join("\n\n");
    await page.evaluate((md) => {
      const ed = (window as any).docxtor.fe.editor;
      ed.exec({ op: "insert_markdown", before: ed.engine.renderAll().order[0], markdown: md });
    }, markdown);
    // The save autosave, the recovery copy and version history make (Download makes the same bytes on the page).
    const bytes = await page.evaluate(async () => Array.from((await (window as any).docxtor.fe.editor.engine.saveInSteps()).bytes as Uint8Array));
    const saved = Buffer.from(bytes);
    expect(await workers(page), "the save ran in the worker").toContain("zip-worker");
    await page.locator("#app > input[type=file]").setInputFiles({ name: "long.docx", mimeType: "application/octet-stream", buffer: saved });
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("long");
    await expect.poll(() => line(page)).toContain("lorem ipsum");
    expect(await page.evaluate(() => (window as any).docxtor.fe.editor.engine.raw.wordCount()), "the text is all there").toBeGreaterThan(180_000);
  });

  test("1.5 MB of pictures alone is zipped on the page: a picture is stored as it is, so it starts no save worker", { tag: "@core" }, async ({ page }) => {
    await countWorkers(page);
    // Bytes that don't compress, as a photograph's.
    const big = new Uint8Array(1_500_000);
    let x = 12345;
    for (let i = 0; i < big.length; i++) big[i] = (x = (x * 1103515245 + 12345) >>> 0) >>> 24;
    big.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    const spec = { sections: [{ paragraphs: ["A document with a big picture.", "Second paragraph."] }], media: [{ name: "big.png", rid: "rIdBig", bytes: big }] };
    await openBuilt(page, spec);
    await caretAtEnd(page, await firstId(page));
    await page.keyboard.type(" Edited.");
    await expect.poll(() => line(page)).toContain("Edited.");
    const bytes = await page.evaluate(async () => Array.from((await (window as any).docxtor.fe.editor.engine.saveInSteps()).bytes as Uint8Array));
    const saved = Buffer.from(bytes);
    expect(saved.length, "the picture is in the saved file").toBeGreaterThan(1_400_000);
    expect(await workers(page), "no save worker for pictures").not.toContain("zip-worker");
    await page.locator("#app > input[type=file]").setInputFiles({ name: "big.docx", mimeType: "application/octet-stream", buffer: saved });
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("big");
    await expect.poll(() => line(page)).toContain("A document with a big picture. Edited.");
  });

  // The start screen makes the editor inert while a document loads; a focus given before it closed was lost, and keys typed went nowhere.
  test("a document that opens takes typing at once, without a click", { tag: "@core" }, async ({ page }) => {
    await page.goto("/?open=simple.docx");
    await documentReady(page);
    await expect.poll(() => page.evaluate(() => !!document.activeElement?.closest(".flow"))).toBe(true);
    await page.keyboard.type("Typed first ");
    await expect(page.locator(".flow .fp").first()).toContainText("Typed first ");
  });

  // The test fixture marks every session as having heard this word (e2e/helpers.ts), so it does not sit in other tests' way; here it is let through.
  test("outside Chrome and Edge the app says once a session what it is made for", { tag: "@core" }, async ({ page, browserName }) => {
    await page.addInitScript(() => { if (!sessionStorage.getItem("test-warned-once")) { sessionStorage.setItem("test-warned-once", "1"); sessionStorage.removeItem("docxtor-browser-warned"); } });
    await page.goto("/?open=simple.docx");
    await documentReady(page);
    const word = page.locator(".toast", { hasText: "Docxtor is made for Chrome and Edge for now" });
    if (browserName === "chromium") await expect(word).toHaveCount(0);
    else await expect(word).toHaveCount(1);
    await page.reload();
    await documentReady(page);
    await expect(word).toHaveCount(0);
  });
});

/** Count the workers the page starts, by their script's address (read with `workers`). */
async function countWorkers(page: Page) {
  await page.addInitScript(() => {
    const W = window.Worker;
    (window as unknown as { __workers: string[] }).__workers = [];
    window.Worker = class extends W {
      constructor(url: string | URL, opts?: WorkerOptions) {
        super(url, opts);
        (window as unknown as { __workers: string[] }).__workers.push(String(url));
      }
    } as typeof Worker;
  });
}

const workers = (page: Page) => page.evaluate(() => (window as unknown as { __workers: string[] }).__workers.join(" "));
