// Who may change the document. Read-only is a set of reasons held by features (the window lock, a view, the host), and
// the engine refuses edits from every caller while any reason is held: the typing, the assistant, extensions and scripts.
// A read-only document still switches tabs; modes end in any order and leave the text editable; the lock of a second
// window belongs to its document and View, Editing does not undo it.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, documentReady, dialog, expect, inPage, LINE_END, test, type Page } from "./helpers";
import { onFe } from "./support/view-helpers";

const many = (word: string) => Array.from({ length: 6 }, (_, i) => `${word} paragraph ${i + 1}, long enough to fill a line of the page.`).join("\n\n");

type Raw = { callTool(name: string, args: string): string; access(): string; apply(ops: unknown): unknown; text(): string; isModified(): boolean };
type FeLike = { readOnly: boolean; lockReasons(): string[]; lock(r: string, o?: { scope: string }): () => void; setReadOnly(on: boolean): void; extensions: { execute(id: string): Promise<unknown> }; editor: { engine: { raw: Raw; text(): string; access(): string; apply(ops: unknown): unknown; renderAll(): { order: number[] } } } };

const access = (page: Page) => onFe(page, (fe) => (fe as unknown as FeLike).editor.engine.access());
const readOnly = (page: Page) => onFe(page, (fe) => fe.readOnly);

async function twoDocs(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Notes.docx", md), many("Notes"));
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Notes");
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Report.docx", md), many("Report"));
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Report");
  await documentReady(page);
}

/** A second window on the document the first has open: it is told it is read-only. */
async function lockedWindow(page: Page, context: ReturnType<Page["context"]>) {
  await twoDocs(page);
  const second = await context.newPage();
  await second.goto(page.url());
  await documentReady(second);
  await expect(second.locator(".toast")).toContainText("open in another window", { timeout: 5000 });
  return second;
}

const firstText = (page: Page) => onFe(page, (fe) => (fe as unknown as FeLike).editor.engine.text().split("\n")[0]);

test.describe("a read-only document", () => {
  test("still switches tabs", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Overview text", "More of the overview"] }] });
    await inPage(page, (w) => w.showPanel("navigation"));
    await chooseMenu(page, "Insert", "Tab…");
    const d = dialog(page, "Add tab");
    await d.getByRole("textbox").fill("Details");
    await d.getByRole("button", { name: "Add" }).click();
    const flow = page.locator(".editor-host .flow").first();
    const pane = page.locator(".doc-tabs-pane");
    await expect(flow.locator("[data-p]", { hasText: "Overview text" })).toBeHidden();
    // Nothing can be typed now, and the engine says why; the tabs still open.
    await onFe(page, (fe) => (fe as unknown as FeLike).setReadOnly(true));
    expect(await access(page)).toBe("view");
    await pane.getByRole("treeitem", { name: /Tab 1/ }).click();
    await expect(flow.locator("[data-p]", { hasText: "Overview text" })).toBeVisible();
    await pane.getByRole("treeitem", { name: /Details/ }).click();
    await expect(flow.locator("[data-p]", { hasText: "Overview text" })).toBeHidden();
    await expect(pane.getByRole("treeitem", { name: /Details/ })).toHaveAttribute("aria-selected", "true");
    // Adding a tab is an edit: refused.
    const refused = await onFe(page, (fe) => {
      try {
        (fe as unknown as FeLike).editor.engine.apply({ op: "tab", action: "add", name: "More" });
        return "";
      } catch (e) {
        return String((e as Error).message);
      }
    });
    expect(refused).toContain("read-only");
    await onFe(page, (fe) => (fe as unknown as FeLike).setReadOnly(false));
    expect(await access(page)).toBe("edit");
  });

  test("the engine refuses edits from every caller, and says so in plain words", async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["Kept as it is"] }] });
    await onFe(page, (fe) => (fe as unknown as FeLike).setReadOnly(true));
    const out = await onFe(page, (fe) => {
      const eng = (fe as unknown as FeLike).editor.engine;
      const p = eng.renderAll().order[0];
      const tries: Record<string, string> = {};
      const attempt = (name: string, f: () => unknown) => {
        try {
          f();
          tries[name] = "allowed";
        } catch (e) {
          tries[name] = String((e as Error).message);
        }
      };
      attempt("apply", () => eng.apply({ op: "insert_text", at: { p, o: 0 }, text: "X" }));
      attempt("tool", () => eng.raw.callTool("insert_content", JSON.stringify({ markdown: "More" })));
      return tries;
    });
    expect(out.apply).toContain("read-only");
    expect(out.tool).toContain("read-only");
    expect(await firstText(page)).toBe("Kept as it is");
  });
});

test.describe("modes that lock the text", () => {
  test.beforeEach(async ({ page }) => {
    await openBuilt(page, { sections: [{ paragraphs: ["First line of text", "Second line of text", "Third line of text"] }] });
  });

  const lineFocus = (page: Page) => onFe(page, (fe) => (fe as unknown as FeLike).extensions.execute("view.lineFocus"));

  async function typeWorks(page: Page) {
    await page.locator(".flow [data-p]", { hasText: "Second line" }).click();
    await page.keyboard.press("End");
    await page.keyboard.type("!");
    await expect(page.locator(".flow")).toContainText("Second line of text!");
  }

  test("Reading, then line focus, closed in either order, leave the document editable", async ({ page }) => {
    for (const reverse of [false, true]) {
      await chooseMenu(page, "View", "Layout", "Reading");
      await lineFocus(page);
      expect(await readOnly(page)).toBe(true);
      expect(await access(page)).toBe("view");
      if (!reverse) {
        await lineFocus(page);
        await chooseMenu(page, "View", "Layout", "Print layout");
      } else {
        await chooseMenu(page, "View", "Layout", "Print layout");
        expect(await readOnly(page)).toBe(true);
        await lineFocus(page);
      }
      expect(await readOnly(page)).toBe(false);
      expect(await access(page)).toBe("edit");
    }
    await typeWorks(page);
  });

  test("View, Viewing holds its own reason: Reading ends without undoing it, and Editing lets it go", async ({ page }) => {
    await chooseMenu(page, "View", "Mode", "Viewing");
    await chooseMenu(page, "View", "Layout", "Reading");
    await chooseMenu(page, "View", "Layout", "Print layout");
    expect(await readOnly(page)).toBe(true);
    await chooseMenu(page, "View", "Mode", "Editing");
    expect(await readOnly(page)).toBe(false);
    expect(await access(page)).toBe("edit");
    await typeWorks(page);
  });

  test("in a split window a view locks its own pane, and the document stays editable in the other", async ({ page }) => {
    await chooseMenu(page, "View", "Window", "Split");
    const flows = page.locator(".flow");
    await expect(flows).toHaveCount(2);
    // The top pane (the one with the keyboard when the split opens) goes to Reading.
    await flows.nth(0).locator("[data-p]").first().click();
    await chooseMenu(page, "View", "Layout", "Reading");
    await expect(flows.nth(0)).toHaveAttribute("contenteditable", "false");
    await expect(flows.nth(1)).toHaveAttribute("contenteditable", "true");
    expect(await access(page)).toBe("edit");
    // The other pane is typed in, and the Reading pane shows it.
    await flows.nth(1).locator("[data-p]", { hasText: "Second line" }).click();
    await page.keyboard.press(LINE_END);
    await page.keyboard.type("!");
    await expect(flows.nth(0)).toContainText("Second line of text!");
    // Both panes in Reading: the engine is view only; leaving one frees nothing in the other.
    await chooseMenu(page, "View", "Layout", "Reading");
    expect(await access(page)).toBe("view");
    await flows.nth(0).locator("[data-p]").first().click();
    await chooseMenu(page, "View", "Layout", "Print layout");
    await expect(flows.nth(0)).toHaveAttribute("contenteditable", "true");
    await expect(flows.nth(1)).toHaveAttribute("contenteditable", "false");
    expect(await access(page)).toBe("edit");
    await chooseMenu(page, "View", "Window", "Remove split");
    await expect(page.locator(".flow")).toHaveCount(1);
    await expect(page.locator(".flow")).toHaveAttribute("contenteditable", "true");
    expect(await access(page)).toBe("edit");
  });
});

test.describe("the lock of a second window", () => {
  test("the engine and the assistant refuse to change the document, and nothing is saved", async ({ page, context }) => {
    const second = await lockedWindow(page, context);
    expect(await readOnly(second)).toBe(true);
    expect(await access(second)).toBe("view");
    const before = await firstText(second);
    const refused = await onFe(second, (fe) => {
      const eng = (fe as unknown as FeLike).editor.engine;
      const p = eng.renderAll().order[0];
      const r: string[] = [];
      for (const f of [() => eng.apply({ op: "insert_text", at: { p, o: 0 }, text: "X" }), () => eng.raw.callTool("insert_content", JSON.stringify({ markdown: "More" }))]) {
        try {
          f();
          r.push("allowed");
        } catch (e) {
          r.push(String((e as Error).message));
        }
      }
      return r;
    });
    expect(refused.every((m) => m.includes("read-only"))).toBe(true);
    // The assistant says so instead of running.
    await inPage(second, async (w) => {
      const app = w as unknown as { ai: { toggle(open?: boolean): void; settings: object; input: HTMLTextAreaElement; send(): Promise<void> } };
      app.ai.toggle(true);
      app.ai.settings = { ...app.ai.settings, apiKey: "test-key" };
      app.ai.input.value = "Add words";
      await app.ai.send();
    });
    await expect(second.locator(".err-line")).toContainText("read-only");
    await expect(second.locator(".tool-row")).toHaveCount(0);
    expect(await firstText(second)).toBe(before);
    expect(await onFe(second, (fe) => (fe as unknown as FeLike).editor.engine.raw.isModified())).toBe(false);
    // The file in the library is the one the first window has.
    const saved = await inPage(second, async (w) => {
      const Engine = w.fe.editor.engine!.constructor as unknown as { open(b: Uint8Array, n: string): { text(): string; dispose(): void } };
      const e = Engine.open(await w.files.library.read("Report.docx"), "Report.docx");
      const text = e.text();
      e.dispose();
      return text;
    });
    expect(saved).not.toContain("Add words");
    await second.close();
  });

  test("printing there brings its fields up to date, but that window never saves the document", async ({ page, context }) => {
    await twoDocs(page);
    // The first window gives the document a field whose result is out of date (the author changes after it), and saves.
    const first = await onFe(page, (fe) => {
      const eng = (fe as unknown as FeLike).editor.engine;
      const p = eng.renderAll().order[0];
      eng.apply([{ op: "set_doc_props", author: "Ada" }]);
      eng.apply([{ op: "insert_field", at: { p, o: 0 }, field: "AUTHOR" }]);
      eng.apply([{ op: "set_doc_props", author: "Grace" }]);
      return eng.text().split("\n")[0];
    });
    expect(first.startsWith("Ada")).toBe(true);
    await inPage(page, (w) => (w as unknown as { save(explicit: boolean): Promise<void> }).save(true));
    const second = await context.newPage();
    await second.goto(page.url());
    await documentReady(second);
    await expect(second.locator(".toast")).toContainText("open in another window", { timeout: 5000 });
    // Download from the read-only window: the field is updated there.
    const updated = await onFe(second, (fe) => (fe as unknown as { editor: { updateFieldsForOutput(): Promise<number> } }).editor.updateFieldsForOutput());
    expect(updated).toBeGreaterThan(0);
    expect((await firstText(second)).startsWith("Grace")).toBe(true);
    // Neither an autosave nor Save writes it: the file is still the one the first window saved.
    await inPage(second, (w) => (w as unknown as { save(explicit: boolean): Promise<void> }).save(true));
    await second.waitForTimeout(300);
    const saved = await inPage(second, async (w) => {
      const Engine = w.fe.editor.engine!.constructor as unknown as { open(b: Uint8Array, n: string): { text(): string; dispose(): void } };
      const e = Engine.open(await w.files.library.read("Report.docx"), "Report.docx");
      const text = e.text();
      e.dispose();
      return text.split("\n")[0];
    });
    expect(saved.startsWith("Ada")).toBe(true);
    await second.close();
  });

  test("View, Editing does not unlock it, and a document opened in that window is editable", async ({ page, context }) => {
    const second = await lockedWindow(page, context);
    // Editing is unavailable while another window has the document, and running it anyway changes nothing.
    await onFe(second, (fe) => (fe as unknown as FeLike).extensions.execute("view.editing").catch(() => undefined));
    expect(await readOnly(second)).toBe(true);
    expect(await access(second)).toBe("view");
    expect(await onFe(second, (fe) => (fe as unknown as FeLike).lockReasons())).toContain("window-lock");
    // Another document in the same window is its own: the lock ended with the one it belonged to.
    await inPage(second, (w) => w.createIn(w.files.library, "", "Third.docx", "Third document text"));
    await expect(second.getByRole("textbox", { name: "Document name" })).toHaveValue("Third");
    await documentReady(second);
    expect(await readOnly(second)).toBe(false);
    expect(await access(second)).toBe("edit");
    expect(await onFe(second, (fe) => (fe as unknown as FeLike).lockReasons())).toEqual([]);
    await second.locator(".flow [data-p]").first().click();
    await second.keyboard.press("End");
    await second.keyboard.type("!");
    await expect(second.locator(".flow")).toContainText("Third document text!");
    await second.close();
  });
});
