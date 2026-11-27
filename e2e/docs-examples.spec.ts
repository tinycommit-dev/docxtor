// The browser examples of the documents (docs/embedding.md, docs/extensions/index.md) run in a real page.
//
// Each example is taken out of the Markdown by its marker (`<!-- example: name -->`, see scripts/docs-blocks.mjs), put in a
// page that the dev server serves from web/examples/_docs/ (so `@docxtor/editor` resolves as it does for any code in the
// checkout), and its own sentences are checked: the element opens the file and saves it back, the component calls what it
// was given, the spelling provider's dictionaries load and underline a word, the password example opens a protected file,
// the CSS changes the colours. Break the example (rename an option or an event) and its test fails.
//
// Run just this file: npx playwright test e2e/docs-examples.spec.ts
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { BROWSER_EXAMPLES, example } from "../scripts/docs-blocks.mjs";
import { expect, test } from "./helpers";

// Playwright runs from the repository root (see playwright.config.ts).
const ROOT = process.cwd();
const OUT = join(ROOT, "web/examples/_docs");
const rich = readFileSync(join(ROOT, "fixtures/rich.docx"));
const protectedDoc = readFileSync(join(ROOT, "fixtures/encrypted/encrypted-agile.docx"));
const PASSWORD = "docxtor-test-pw"; // the password of the vectors in fixtures/encrypted/ (see e2e/encryption.spec.ts)

/** TypeScript to JavaScript: the types are taken out and nothing else changes. */
async function strip(code: string): Promise<string> {
  const { stripTypeScriptTypes } = (await import("node:module")) as unknown as { stripTypeScriptTypes: (code: string) => string };
  return stripTypeScriptTypes(code);
}

/** Writes a page the dev server serves, and returns its address. */
function writePage(name: string, body: string): string {
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, `${name}.html`), `<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${name}</title></head>\n<body style="margin:0">\n${body}\n</body></html>\n`);
  return `/examples/_docs/${name}.html`;
}

// One worker runs the file in order: the pages are files in one folder, which the last test removes.
test.describe.configure({ mode: "serial" });
test.afterAll(() => rmSync(OUT, { recursive: true, force: true }));

test("every browser example of the documents has a test here", () => {
  const tested = ["embed-element", "embed-component", "embed-spelling", "embed-password", "embed-theming", "extensions-use"];
  expect(tested.sort()).toEqual([...BROWSER_EXAMPLES].sort());
  for (const name of tested) expect(example(name).code.length).toBeGreaterThan(20);
});

test("embed-element: the <docxtor-editor> element opens its src, tells the page about edits and hands back the bytes on save", async ({ page }) => {
  const saved: Buffer[] = [];
  await page.route("**/files/contract.docx", async (route) => {
    if (route.request().method() === "PUT") {
      saved.push(route.request().postDataBuffer() ?? Buffer.alloc(0));
      return route.fulfill({ status: 200, body: "ok" });
    }
    return route.fulfill({ contentType: "application/vnd.openxmlformats-officedocument.wordprocessingml.document", body: rich });
  });
  const logs: string[] = [];
  page.on("console", (m) => logs.push(m.text()));
  // The example as written: the element (with its style) and the script.
  const url = writePage("embed-element", example("embed-element").code);
  await page.goto(url);
  const paragraphs = page.locator("docxtor-editor .flow .fp");
  await expect(paragraphs.first()).toContainText("Quarterly Product Report");
  await expect(page.locator("docxtor-editor")).toHaveAttribute("theme", "system");
  // docxtor-ready: the document opened, and save() works from then on
  await expect.poll(() => logs.some((l) => /^opened, \d{4,} bytes$/.test(l))).toBe(true);

  // docxtor-change
  await paragraphs.first().click();
  await page.keyboard.type("x");
  await expect.poll(() => logs.includes("edited")).toBe(true);

  // docxtor-save: Ctrl-S or Cmd-S inside the editor gives the bytes, and the example PUTs them
  await page.keyboard.press("ControlOrMeta+s");
  await expect.poll(() => saved.length).toBe(1);
  expect(saved[0].subarray(0, 2).toString()).toBe("PK");
  expect(saved[0].length).toBeGreaterThan(5000);

  // save() on the element
  const size = await page.evaluate(() => (document.getElementById("doc") as unknown as { save(): Uint8Array }).save().length);
  expect(size).toBeGreaterThan(5000);
});

test("embed-component: the DocxtorEditor example opens a file, calls onSave, ask and the change listener", async ({ page }) => {
  await page.route("**/report.docx", (route) => route.fulfill({ body: rich }));
  const js = await strip(example("embed-component").code);
  const url = writePage(
    "embed-component",
    `<div id="editor" style="height: 700px"></div>
<script type="module">
  // What the sentences around the example supply.
  const calls = (window.calls = { upload: [], dirty: 0, asked: 0 });
  const upload = (bytes) => calls.upload.push(bytes.length);
  const markDirty = () => calls.dirty++;
  const myAssistant = { open: () => calls.asked++ };
${js}
  window.example = { editor, bytes };
</script>`,
  );
  await page.goto(url);
  await page.waitForFunction(() => (window as unknown as { example?: unknown }).example);
  const first = page.locator("#editor .flow .fp").first();
  await expect(first).toContainText("Quarterly Product Report");
  const bytesLength = await page.evaluate(() => (window as unknown as { example: { bytes: Uint8Array } }).example.bytes.length);
  expect(bytesLength).toBeGreaterThan(5000);

  await first.click();
  await page.keyboard.type("x");
  await expect.poll(() => page.evaluate(() => (window as unknown as { calls: { dirty: number } }).calls.dirty)).toBeGreaterThan(0);

  await page.keyboard.press("ControlOrMeta+s");
  await expect.poll(() => page.evaluate(() => (window as unknown as { calls: { upload: number[] } }).calls.upload.length)).toBe(1);

  // `ask` adds an Ask button to the toolbar, and clicking it calls the host's assistant.
  await page.locator("#editor").getByRole("button", { name: "Ask" }).first().click();
  expect(await page.evaluate(() => (window as unknown as { calls: { asked: number } }).calls.asked)).toBe(1);
});

test("embed-spelling: the provider's dictionaries load and underline a misspelled word, and a language it has none for is not checked", async ({ page }) => {
  const dictionary = (ext: string) => readFileSync(join(ROOT, "node_modules/dictionary-en", `index.${ext}`));
  const asked: string[] = [];
  await page.route("**/dictionaries/**", (route) => {
    asked.push(new URL(route.request().url()).pathname);
    const ext = route.request().url().endsWith(".aff") ? "aff" : "dic";
    return route.fulfill({ contentType: "text/plain", body: dictionary(ext) });
  });
  const js = await strip(example("embed-spelling").code);
  const url = writePage(
    "embed-spelling",
    `<div id="editor" style="height: 500px"></div>
<script type="module">
${js}
  window.spelling = spelling;
  const { Engine } = await import("@docxtor/editor");
  await DocxtorEditor.ready();
  const shown = new DocxtorEditor({ spelling }).mount(document.getElementById("editor"));
  await shown.open(Engine.fromMarkdown("I will recieve the parcel tomorrow.", "spelling.docx").save(), "spelling.docx");
  window.ready = true;
</script>`,
  );
  await page.goto(url);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready);
  await expect(page.locator('.sp-layer .sp-u[data-word="recieve"]')).toHaveCount(1);
  expect(asked.some((p) => p.includes("/dictionaries/en_US/index.aff")) && asked.some((p) => p.endsWith("index.dic"))).toBe(true);
  // "Returning null means no dictionary."
  expect(await page.evaluate(() => (window as unknown as { spelling: { load(l: string): Promise<unknown> } }).spelling.load("tlh"))).toBeNull();
});

test("embed-password: a protected file rejects with EncryptedError until its password is given, and the document is saved protected again", async ({ page }) => {
  await page.route("**/protected.docx", (route) => route.fulfill({ body: protectedDoc }));
  const js = await strip(example("embed-password").code);
  const url = writePage(
    "embed-password",
    `<div id="editor" style="height: 500px"></div>
<script type="module">
  import { DocxtorEditor } from "@docxtor/editor";
  // What the sentences around the example supply: an editor, the file's bytes, and a way to ask the person.
  const editor = new DocxtorEditor().mount(document.getElementById("editor"));
  const bytes = new Uint8Array(await (await fetch("/protected.docx")).arrayBuffer());
  const asked = (window.asked = []);
  const askForPassword = async () => (asked.push(true), ${JSON.stringify(PASSWORD)});
${js}
  window.example = { editor };
  window.done = true;
</script>`,
  );
  await page.goto(url);
  await page.waitForFunction(() => (window as unknown as { done?: boolean }).done);
  expect(await page.evaluate(() => (window as unknown as { asked: unknown[] }).asked.length)).toBe(1);
  await expect(page.locator("#editor .flow .fp").first()).toBeVisible();
  const state = await page.evaluate(() => {
    const { editor } = (window as unknown as { example: { editor: { hasPassword: boolean; save(): Uint8Array } } }).example;
    // setPassword("new password") ran: the saved bytes are an OLE compound file (Word's own container for a protected file), not a zip.
    return { hasPassword: editor.hasPassword, magic: Array.from(editor.save().subarray(0, 4)) };
  });
  expect(state.hasPassword).toBe(true);
  expect(state.magic).toEqual([0xd0, 0xcf, 0x11, 0xe0]);
});

test("embed-theming: the CSS of the guide changes the editor's colours", async ({ page }) => {
  const css = example("embed-theming").code;
  const url = writePage(
    "embed-theming",
    `<style>${css}</style>
<div id="editor" style="height: 500px"></div>
<script type="module">
  import "@docxtor/editor/style.css";
  import { DocxtorEditor } from "@docxtor/editor";
  const editor = new DocxtorEditor().mount(document.getElementById("editor"), { theme: "light" });
  await editor.create("theme.docx");
  window.ready = true;
</script>`,
  );
  await page.goto(url);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready);
  const vars = await page.evaluate(() => {
    const s = getComputedStyle(document.getElementById("editor")!);
    return { signal: s.getPropertyValue("--signal").trim(), canvas: s.getPropertyValue("--canvas").trim(), button: s.getPropertyValue("--button").trim() };
  });
  expect(vars).toEqual({ signal: "#2563eb", canvas: "#f1f5f9", button: "#0f172a" });
  // The canvas colour is what shows behind the pages.
  const painted = await page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#editor *")].some((e) => getComputedStyle(e).backgroundColor === "rgb(241, 245, 249)"));
  expect(painted).toBe(true);
});

test("extensions-use: extensions go in with the option, with use(), and on the element's editor", async ({ page }) => {
  const js = await strip(example("extensions-use").code);
  const url = writePage(
    "extensions-use",
    `<docxtor-editor style="height: 400px"></docxtor-editor>
<script type="module">
  import { DocxtorEditor, defineExtension } from "@docxtor/editor";
  const make = (id) => defineExtension({ id, name: id, activate(ctx) { ctx.commands.register(id + ".run", () => id, { title: id }); } });
  const shout = make("acme.shout");
  const anotherOne = make("acme.other");
  window.DocxtorEditor = DocxtorEditor;
${js}
  window.example = { editor, fromElement: document.querySelector("docxtor-editor").editor };
</script>`,
  );
  await page.goto(url);
  await page.waitForFunction(() => (window as unknown as { example?: unknown }).example);
  const ids = await page.evaluate(() => {
    const { editor, fromElement } = (window as unknown as { example: { editor: { extensions: { list(): { id: string }[] } }; fromElement: { extensions: { list(): { id: string }[] } } } }).example;
    return { editor: editor.extensions.list().map((e) => e.id), element: fromElement.extensions.list().map((e) => e.id) };
  });
  expect(ids.editor).toEqual(expect.arrayContaining(["acme.shout", "acme.other"]));
  expect(ids.element).toEqual(expect.arrayContaining(["acme.shout"]));
});
