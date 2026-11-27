// Tools > Inspect document: what the file carries besides the page (personal information, names, comments, tracked
// changes, hidden text, headers and footers, custom data), removing it as one undo step, and inspectors that extensions add.
import { buildDocx, type DocSpec } from "./support/docx-builder";
import { answer, chooseMenu, dialog, expect, inPage, type Locator, type Page, savedPart, test } from "./helpers";

const MOD = "ControlOrMeta";
const BOB = 'w:author="Bob" w:date="2026-10-01T10:00:00Z"';

const spec: DocSpec = {
  sections: [
    {
      paragraphs: [
        {
          raw:
            '<w:r><w:t xml:space="preserve">Visible </w:t></w:r><w:commentRangeStart w:id="0"/><w:r><w:t>commented</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="0"/></w:r>' +
            `<w:ins w:id="5" ${BOB}><w:r><w:t xml:space="preserve"> inserted</w:t></w:r></w:ins><w:del w:id="6" ${BOB}><w:r><w:delText xml:space="preserve"> removed</w:delText></w:r></w:del>` +
            '<w:r><w:rPr><w:vanish/></w:rPr><w:t xml:space="preserve"> secret</w:t></w:r>',
        },
        "A second paragraph.",
      ],
      headers: { default: "Confidential draft" },
    },
  ],
  props: { creator: "Erin Example", lastModifiedBy: "Sam Saver", title: "Report" },
  app: "<Application>Word</Application><Manager>Dana Boss</Manager><Company>Acme Corp</Company>",
  custom: '<property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Project"><vt:lpwstr>Falcon</vt:lpwstr></property>',
  comments: '<w:comment w:id="0" w:author="Carol" w:initials="C" w:date="2026-10-02T10:00:00Z"><w:p><w:r><w:t>Check this</w:t></w:r></w:p></w:comment>',
  customXml: '<?xml version="1.0" encoding="UTF-8"?><data xmlns="urn:acme"><secret>1</secret></data>',
};

async function openInspected(page: Page, s: DocSpec = spec, first = "Visible") {
  const bytes = Array.from(buildDocx(s));
  await page.goto("/?open=simple.docx");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: { engine?: unknown } }).docxtor?.engine);
  await page.evaluate(async (b) => {
    const w = (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor;
    await w.fe.open(new Uint8Array(b), "inspected.docx");
  }, bytes);
  await expect(page.locator(".flow .fp").first()).toContainText(first);
  await page.evaluate(() => document.fonts.ready);
}

const insp = (page: Page) => dialog(page, "Inspect document");
const rowOf = (d: Locator, id: string) => d.locator(`.insp-row[data-inspector="${id}"]`);
const kind = (name: string) => `docxtor.inspect.${name}`;
const bodyText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text().trim());


test.describe("inspect document", () => {
  test("lists what the file carries, with counts and what they are", async ({ page }) => {
    await openInspected(page);
    await chooseMenu(page, "Tools", "Inspect document…");
    const d = insp(page);
    await expect(d).toBeVisible();
    await expect(d.locator(".insp-row")).toHaveCount(8);
    await expect(rowOf(d, kind("properties")).locator(".insp-status")).toHaveText("5 items found");
    const items = rowOf(d, kind("properties")).locator(".insp-detail li");
    await expect(items).toHaveText(["Author: Erin Example", "Last saved by: Sam Saver", "Company: Acme Corp", "Manager: Dana Boss", "1 custom property"]);
    await expect(rowOf(d, kind("authors")).locator(".insp-detail li")).toHaveText(["Bob", "Carol"]);
    await expect(rowOf(d, kind("comments")).locator(".insp-status")).toHaveText("1 item found");
    await expect(rowOf(d, kind("revisions")).locator(".insp-status")).toHaveText("2 items found");
    await expect(rowOf(d, kind("hiddenText")).locator(".insp-status")).toHaveText("1 item found");
    await expect(rowOf(d, kind("headersFooters")).locator(".insp-status")).toHaveText("1 item found");
    await expect(rowOf(d, kind("customXml")).locator(".insp-status")).toHaveText("1 item found");
    await expect(rowOf(d, "docxtor.inspect.embedded").locator(".insp-status")).toHaveText("Nothing found");
    // Each finding has its own Remove button; the one with nothing found has none.
    await expect(rowOf(d, kind("comments")).getByRole("button", { name: "Remove comments" })).toBeVisible();
    await expect(rowOf(d, "docxtor.inspect.embedded").getByRole("button")).toHaveCount(0);
    await d.getByRole("button", { name: "Done" }).click();
    await expect(d).toBeHidden();
  });

  test("Remove takes one kind out, as one undo step, and the dialog shows what is left", async ({ page }) => {
    await openInspected(page);
    expect(await bodyText(page)).toContain("secret");
    await chooseMenu(page, "Tools", "Inspect document…");
    const d = insp(page);
    await rowOf(d, kind("hiddenText")).getByRole("button", { name: "Remove hidden text" }).click();
    await expect(rowOf(d, kind("hiddenText")).locator(".insp-status")).toHaveText("Nothing found");
    // Everything else is still there.
    await expect(rowOf(d, kind("comments")).locator(".insp-status")).toHaveText("1 item found");
    await d.getByRole("button", { name: "Done" }).click();
    await expect.poll(() => bodyText(page)).not.toContain("secret");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => bodyText(page)).toContain("secret");
  });

  test("Remove everything found cleans the file, the parts leave the package, and one undo brings it all back", async ({ page }) => {
    await openInspected(page);
    expect(await savedPart(page, "word/comments.xml")).toContain("Carol");
    await chooseMenu(page, "Tools", "Inspect document…");
    const d = insp(page);
    await d.getByRole("button", { name: "Remove everything found" }).click();
    await answer(page, "Remove everything");
    for (const k of ["properties", "authors", "comments", "revisions", "hiddenText", "headersFooters", "customXml"]) {
      await expect(rowOf(d, kind(k)).locator(".insp-status")).toHaveText("Nothing found");
    }
    await expect(d.getByRole("button", { name: "Remove everything found" })).toBeDisabled();
    await d.getByRole("button", { name: "Done" }).click();

    // The text: the insertion was accepted, the deletion and the hidden text are gone.
    await expect.poll(() => bodyText(page)).toBe("Visible commented inserted\nA second paragraph.");
    const xml = await savedPart(page, "word/document.xml");
    for (const gone of ["commentRange", "commentReference", "<w:ins", "<w:del", "vanish", "Bob", "headerReference"]) expect(xml).not.toContain(gone);
    expect(await savedPart(page, "word/comments.xml")).toBe("");
    expect(await savedPart(page, "customXml/item1.xml")).toBe("");
    const core = await savedPart(page, "docProps/core.xml");
    expect(core).not.toContain("Erin Example");
    expect(core).not.toContain("Sam Saver");
    const app = await savedPart(page, "docProps/app.xml");
    expect(app).not.toContain("Acme Corp");
    expect(app).not.toContain("Dana Boss");
    expect(await savedPart(page, "docProps/custom.xml")).toBe("");
    expect(await savedPart(page, "word/settings.xml")).toContain("<w:removePersonalInformation/>");

    // One undo puts back the lot.
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => bodyText(page)).toContain("secret");
    expect(await savedPart(page, "word/comments.xml")).toContain("Carol");
    expect(await savedPart(page, "docProps/core.xml")).toContain("Erin Example");
    expect(await savedPart(page, "word/document.xml")).toContain("Bob");
    expect(await savedPart(page, "customXml/item1.xml")).toContain("urn:acme");
  });

  test("a clean document has nothing to remove", async ({ page }) => {
    await openInspected(page, { sections: [{ paragraphs: ["Just text."] }], props: { title: "Plain" } }, "Just text.");
    await chooseMenu(page, "Tools", "Inspect document…");
    const d = insp(page);
    await expect(d.locator(".insp-row")).toHaveCount(8);
    await expect(d.locator(".insp-status")).toHaveText(Array(8).fill("Nothing found"));
    await expect(d.getByRole("button", { name: "Remove everything found" })).toBeDisabled();
    await expect(d.getByRole("button", { name: /^Remove (?!everything)/ })).toHaveCount(0);
  });

  test("an extension adds an inspector beside Docxtor's, and its Remove goes in as the same undo step", async ({ page }) => {
    await openInspected(page);
    await inPage(page, async (w) => {
      await (w.fe as unknown as { use(e: unknown): Promise<void> }).use({
        id: "test.inspector",
        name: "Test inspector",
        activate(ctx: { inspectors: { register(i: unknown): void } }) {
          ctx.inspectors.register({
            id: "test.inspector.todo",
            label: "Unfinished passages",
            description: "Text marked TODO.",
            order: 5,
            inspect: ({ document }: { document: { text(): string } }) => ({ count: (document.text().match(/second/g) ?? []).length, detail: ["second"] }),
            remove: ({ exec }: { exec(ops: unknown): void }) => exec({ op: "replace_text", find: "second", replace: "2nd", all: true }),
          });
          ctx.inspectors.register({
            id: "test.inspector.broken",
            label: "A check that fails",
            order: 6,
            inspect: () => {
              throw new Error("no access");
            },
          });
        },
      });
    });
    await chooseMenu(page, "Tools", "Inspect document…");
    const d = insp(page);
    // Listed first (its order is lowest), with its detail.
    await expect(d.locator(".insp-row").first()).toHaveAttribute("data-inspector", "test.inspector.todo");
    await expect(rowOf(d, "test.inspector.todo").locator(".insp-status")).toHaveText("1 item found");
    await expect(rowOf(d, "test.inspector.todo").locator(".insp-detail li")).toHaveText(["second"]);
    await expect(rowOf(d, "test.inspector.broken").locator(".insp-status")).toHaveText("Couldn't check: no access");
    // Remove everything found: Docxtor's and the extension's edits are one step.
    await d.getByRole("button", { name: "Remove everything found" }).click();
    await answer(page, "Remove everything");
    await expect(rowOf(d, "test.inspector.todo").locator(".insp-status")).toHaveText("Nothing found");
    await d.getByRole("button", { name: "Done" }).click();
    await expect.poll(() => bodyText(page)).toBe("Visible commented inserted\nA 2nd paragraph.");
    await page.keyboard.press(`${MOD}+Z`);
    await expect.poll(() => bodyText(page)).toContain("A second paragraph.");
    await expect.poll(() => bodyText(page)).toContain("secret");
  });

  test("the command is in the palette and the dialog works in the dark theme", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await openInspected(page);
    await page.keyboard.press(`${MOD}+Shift+P`);
    await page.keyboard.type("inspect");
    await expect(page.getByRole("option", { name: /Inspect document/ })).toBeVisible();
    await page.keyboard.press("Enter");
    const d = insp(page);
    await expect(d).toBeVisible();
    const color = await rowOf(d, kind("comments")).locator(".insp-status").evaluate((el) => getComputedStyle(el).color);
    expect(color).not.toBe("rgba(0, 0, 0, 0)");
    await page.keyboard.press("Escape");
    await expect(d).toBeHidden();
  });
});
