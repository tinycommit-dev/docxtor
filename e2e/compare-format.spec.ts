// Compare documents notices formatting (bold, alignment and so on) and says what changed; Combine documents marks what
// differs from the original and keeps the tracked changes the revised version already has; and how documents are compared
// is a contribution point (a comparer an extension registers is the one that is used).
import { buildDocx } from "./support/docx-builder";
import { chooseMenu, dialog, documentReady, expect, inPage, savedPart, test, type Page } from "./helpers";

const compareDialog = (page: Page) => dialog(page, "Compare documents").filter({ has: page.locator(".cmp-list") });
const combineDialog = (page: Page) => dialog(page, "Combine documents").filter({ has: page.locator(".cmp-list") });


/** Two Library documents, the older one made first; the newer one is left open. */
async function twoVersions(page: Page, older: string, newer: string, after?: (page: Page) => Promise<void>) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Draft v1.docx", md), older);
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Draft v1");
  await inPage(page, (w, md) => w.createIn(w.files.library, "", "Draft v2.docx", md), newer);
  await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Draft v2");
  await documentReady(page);
  await after?.(page);
}

async function compareWithV1(page: Page) {
  await chooseMenu(page, "Tools", "Compare documents…");
  await page.locator(".qp-item", { hasText: "Draft v1" }).click();
  await expect(compareDialog(page)).toBeVisible();
}

test.describe("compare: formatting", () => {
  test("words that kept their text and changed their formatting are marked, with what changed", async ({ page }) => {
    await twoVersions(page, "A plain word here.\n\nSecond stays.", "A **plain** word here.\n\nSecond stays.");
    await compareWithV1(page);
    const view = compareDialog(page);
    await expect(view.locator(".cmp-count")).toHaveText("1 change");
    const row = view.locator('.cmp-row[data-kind="formatted"]');
    await expect(row).toHaveCount(1);
    await expect(row.locator(".cmp-tag")).toHaveText("Formatted");
    // The words did not change: no insertion or deletion, and the whole text reads as it is.
    await expect(row.locator("ins, del")).toHaveCount(0);
    await expect(row.locator(".fmt")).toHaveText("plain");
    await expect(row.locator(".fmt")).toHaveAttribute("title", "Formatting changed: bold");
    await expect(row.locator(".cmp-note")).toHaveText("Formatting changed: bold");
    // It is not told by colour alone: a dotted underline.
    const deco = await row.locator(".fmt").evaluate((el) => {
      const s = getComputedStyle(el);
      return [s.textDecorationLine, s.textDecorationStyle];
    });
    expect(deco).toEqual(["underline", "dotted"]);
    // The other paragraph is the same.
    await expect(view.locator('.cmp-row[data-kind="same"]')).toHaveCount(1);
    await expect(view.getByText("No differences")).toHaveCount(0);
  });

  test("a changed paragraph property is named, and formatting next to changed words is shown with them", async ({ page }) => {
    await twoVersions(page, "The quick brown fox.\n\nCentre me.", "The slow **brown** fox.\n\nCentre me.", async (p) => {
      await inPage(p, (w) => {
        const ed = w.fe.editor;
        const ids = Array.from(document.querySelectorAll<HTMLElement>(".flow .fp")).map((e) => Number(e.dataset.p));
        ed.exec({ op: "format_paragraphs", ids: [ids[1]], format: { align: "center" } });
      });
    });
    await compareWithV1(page);
    const view = compareDialog(page);
    // Neighbouring changed paragraphs are one change, the way Word counts revisions.
    await expect(view.locator(".cmp-count")).toHaveText("1 change");
    const edited = view.locator('.cmp-row[data-kind="changed"]');
    await expect(edited.locator("del")).toHaveText("quick");
    await expect(edited.locator("ins")).toHaveText("slow");
    await expect(edited.locator(".fmt")).toHaveText("brown");
    const centred = view.locator('.cmp-row[data-kind="formatted"]');
    await expect(centred.locator(".cmp-note")).toHaveText("Paragraph formatting changed: alignment");
    await expect(centred.locator(".cmp-tag")).toHaveText("Formatted");
  });

  test("the tracked-changes copy carries the formatting changes as Word's tracked formatting", async ({ page }) => {
    await twoVersions(page, "A plain word here.", "A **plain** word here.");
    await compareWithV1(page);
    await compareDialog(page).getByRole("button", { name: "Open as a document with tracked changes" }).click();
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Comparison of Draft v2 and Draft v1");
    await documentReady(page);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain("<w:rPrChange ");
    expect(xml).toContain('w:author="Docxtor"');
    expect(xml).not.toContain("<w:ins ");
    expect(xml).not.toContain("<w:del ");
  });

  test("the author of the changes can be typed", async ({ page }) => {
    await twoVersions(page, "A plain word here.", "A **plain** word here.");
    await compareWithV1(page);
    const view = compareDialog(page);
    await view.getByLabel("Mark changes as").fill("Dana");
    await view.getByRole("button", { name: "Open as a document with tracked changes" }).click();
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Comparison of Draft v2 and Draft v1");
    await documentReady(page);
    const xml = await savedPart(page, "word/document.xml");
    expect(xml).toContain('w:author="Dana"');
    expect(xml).not.toContain('w:author="Docxtor"');
  });

  test("two documents that look and read the same say so", async ({ page }) => {
    await twoVersions(page, "Same **bold** and *italic*.", "Same **bold** and *italic*.");
    await compareWithV1(page);
    const view = compareDialog(page);
    await expect(view.locator(".cmp-count")).toHaveText("No changes");
    await expect(view.getByText("No differences. The two documents read the same and look the same.")).toBeVisible();
  });
});

test.describe("combine documents", () => {
  const BOB = 'w:author="Bob" w:date="2026-10-01T10:00:00Z"';
  const original = ["The first line.", "The second line stays.", "A third line."];
  /** Bob's copy: a tracked insertion in the first line, and the last line edited without tracking. */
  const revised = buildDocx({
    sections: [
      {
        paragraphs: [
          { raw: `<w:r><w:t xml:space="preserve">The first </w:t></w:r><w:ins w:id="7" ${BOB}><w:r><w:t xml:space="preserve">new </w:t></w:r></w:ins><w:r><w:t>line.</w:t></w:r>` },
          "The second line stays.",
          "A fourth line.",
        ],
      },
    ],
  });

  async function setUp(page: Page) {
    await page.goto("/");
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    await inPage(page, async (w, bytes) => w.files.library.write("Bob.docx", new Uint8Array(bytes)), Array.from(revised));
    await inPage(page, (w, md) => w.createIn(w.files.library, "", "Original.docx", md), original.join("\n\n"));
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Original");
    await documentReady(page);
  }

  test("marks what differs from the original and keeps the revised version's own tracked changes", async ({ page }) => {
    await setUp(page);
    await chooseMenu(page, "Tools", "Combine documents…");
    await page.locator(".qp-item", { hasText: "Bob" }).click();
    const view = combineDialog(page);
    await expect(view).toBeVisible();
    await expect(view.locator(".sub")).toContainText("“Bob” is the revised version and “Original” the original");
    // The revised version is compared with the original: the untracked edit is a change, and Bob's tracked one reads as an insertion.
    await expect(view.locator('.cmp-row[data-kind="changed"]')).toHaveCount(2);
    await expect(view.getByRole("button", { name: "Swap" })).toHaveCount(0);
    await view.getByLabel("Label unmarked changes with").fill("Reviewer");
    await view.getByRole("button", { name: "Open the combined document" }).click();
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("Combination of Original and Bob");
    await documentReady(page);
    const xml = await savedPart(page, "word/document.xml");
    // Bob's insertion is still Bob's, once; the edit nobody tracked is the reviewer's.
    expect(xml.match(/w:author="Bob"/g)!.length).toBe(1);
    expect(xml).toContain('<w:ins w:id="7" w:author="Bob"');
    expect(xml).toContain('w:author="Reviewer"');
    expect(xml).toContain('<w:delText xml:space="preserve">third</w:delText>');
    expect(xml).toContain('<w:t xml:space="preserve">fourth</w:t>');
    // The first paragraph was left alone: nothing in it is the reviewer's.
    expect(xml.split("</w:p>")[0]).not.toContain("Reviewer");
  });

  test("Combine is in the Tools menu next to Compare, and offers the revised version, not the open document", async ({ page }) => {
    await setUp(page);
    await chooseMenu(page, "Tools", "Combine documents…");
    await expect(page.locator(".qp-item", { hasText: "Original" })).toHaveCount(0);
    await expect(page.locator(".qp-input, .qp input").first()).toHaveAttribute("placeholder", "Choose the revised version to combine with this document");
    await page.keyboard.press("Escape");
  });
});

test.describe("comparers are an extension point", () => {
  test("the latest comparer is the one the dialog uses", async ({ page }) => {
    await twoVersions(page, "Alpha.", "Alpha.");
    await inPage(page, async (w) => {
      await (w.fe as unknown as { use(e: unknown): Promise<void> }).use({
        id: "test.comparer",
        name: "Test comparer",
        activate(ctx: { comparers: { register(c: unknown): void } }) {
          ctx.comparers.register({
            id: "test.comparer",
            label: "Test comparer",
            compare: () => [{ kind: "changed", index: 0, parts: [{ t: "same", text: "Made " }, { t: "del", text: "up" }, { t: "ins", text: "up differently" }] }],
          });
        },
      });
    });
    await compareWithV1(page);
    const view = compareDialog(page);
    await expect(view.locator(".cmp-count")).toHaveText("1 change");
    await expect(view.locator('.cmp-row[data-kind="changed"] ins')).toHaveText("up differently");
    // This comparer can't make a document, so the one that can does, from the same two files.
    await view.getByRole("button", { name: "Close" }).click();
  });

  test("with every comparer off there is a plain message", async ({ page }) => {
    await twoVersions(page, "Alpha.", "Alpha.");
    await inPage(page, (w) => (w.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("docxtor.compare"));
    await chooseMenu(page, "Tools", "Compare documents…");
    await expect(page.getByText("No way to compare documents is on.")).toBeVisible();
    await expect(compareDialog(page)).toHaveCount(0);
  });
});
