// Smart chips and the @ menu: @ opens a menu of people, dates, files, drop-downs, building blocks and things to insert,
// filtered as you type; chips are content controls Word shows as text and Docxtor draws as pills with a card; drop-down
// chips have coloured choices; building blocks bring chips with them; a link shows a card when the caret is in it.
import { openBuilt } from "./support/docx-builder";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

async function open(page: Page, paragraphs: (string | { raw: string })[] = ["Meeting with the team", "Second line"]) {
  await openBuilt(page, { sections: [{ paragraphs }] });
  await page.locator(".flow [data-p]").first().click();
  await page.keyboard.press("End");
}

const menu = (page: Page) => page.locator(".at-menu");
const savedXml = (page: Page) =>
  inPage(page, async (w) => {
    const bytes = w.fe.editor.engine!.save();
    const Engine = w.fe.editor.engine!.constructor as unknown as { open(b: Uint8Array, n: string): { renderAll(): { blocks: unknown[] }; dispose(): void } };
    const again = Engine.open(bytes, "again.docx");
    const json = JSON.stringify(again.renderAll().blocks);
    again.dispose();
    return json;
  });

test.describe("the @ menu", () => {
  test("@ after a space opens the menu, typing filters it, and Enter puts a date chip in", async ({ page }) => {
    await open(page);
    await page.keyboard.type(" @");
    await expect(menu(page)).toBeVisible();
    for (const s of ["People", "Dates", "Drop-downs", "Building blocks", "Insert"]) await expect(menu(page).getByRole("group", { name: s })).toHaveCount(1);
    await page.keyboard.type("tod");
    await expect(menu(page).getByRole("option")).toHaveCount(1);
    await expect(menu(page).getByRole("option", { name: /Today/ })).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    await expect(menu(page)).toHaveCount(0);
    const chip = page.locator('.flow [data-chip="date"]').first();
    await expect(chip).toBeVisible();
    await expect(page.locator(".flow [data-p]").first()).not.toContainText("@");
    // A date picker control with the chip's tag, kept when the file opens again.
    const json = await savedXml(page);
    expect(json).toContain("docxtor:chip:date");
    await page.screenshot({ path: "test-results/chip-date.png" });
  });

  test("Escape keeps the @ as typed; a space after text nothing matches closes the menu", async ({ page }) => {
    await open(page);
    await page.keyboard.type(" @");
    await expect(menu(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(menu(page)).toHaveCount(0);
    await expect(page.locator(".flow [data-p]").first()).toContainText("team @");
    await page.keyboard.type(" mail@example.com");
    await expect(menu(page)).toHaveCount(0);
  });

  test("the arrow keys move through the lines and the text field points at the one chosen", async ({ page }) => {
    await open(page);
    await page.keyboard.type(" @");
    await expect(menu(page)).toBeVisible();
    const first = await page.locator(".flow").getAttribute("aria-activedescendant");
    await page.keyboard.press("ArrowDown");
    const second = await page.locator(".flow").getAttribute("aria-activedescendant");
    expect(first).not.toBe(second);
    await expect(page.locator(`#${second}`)).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Escape");
  });
});

test.describe("chips", () => {
  test("a person added by hand is a chip with a card: their email, and Convert to text", async ({ page }) => {
    await open(page);
    await page.keyboard.type(" @add");
    await expect(menu(page).getByRole("option", { name: /Add a person/ })).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    const d = dialog(page, "Add a person");
    await d.getByLabel("Name").fill("Ada Lovelace");
    await d.getByLabel("Email address").fill("ada@example.com");
    await d.getByRole("button", { name: "Add" }).click();
    const chip = page.locator('.flow [data-chip="person"]').first();
    await expect(chip).toHaveText("Ada Lovelace");
    // Next time, @ finds her.
    await page.keyboard.type(" @ada");
    await expect(menu(page).getByRole("option", { name: /Ada Lovelace/ })).toBeVisible();
    await page.keyboard.press("Escape");
    await chip.click();
    const card = page.locator(".chip-card");
    await expect(card).toContainText("ada@example.com");
    await card.getByRole("button", { name: "Convert to text" }).click();
    await expect(page.locator('.flow [data-chip="person"]')).toHaveCount(0);
    await expect(page.locator(".flow [data-p]").first()).toContainText("Ada Lovelace");
  });

  test("a Status chip shows its choice in colour, and the list changes it", async ({ page }) => {
    await open(page);
    await page.keyboard.type(" ");
    await chooseMenu(page, "Insert", "Smart chips", "Status");
    const chip = page.locator('.flow [data-chip="dropdown"]').first();
    await expect(chip).toHaveText("Not started");
    await chip.click();
    await page.locator(".chip-card").getByRole("option", { name: "Done" }).click();
    await expect(chip).toHaveText("Done");
    await expect(chip).toHaveAttribute("data-chip-color", "green");
    expect(await savedXml(page)).toContain("docxtor:chip:dropdown");
    await page.screenshot({ path: "test-results/chip-status.png" });
  });
});

test.describe("building blocks", () => {
  test("Meeting notes come with a date chip, person chips and a checklist", async ({ page }) => {
    await open(page);
    await chooseMenu(page, "Insert", "Building blocks", "Meeting notes");
    await expect(page.locator(".flow")).toContainText("Action items");
    await expect(page.locator('.flow [data-chip="date"]')).toHaveCount(1);
    expect(await page.locator('.flow [data-chip="person"]').count()).toBeGreaterThanOrEqual(2);
    await expect(page.locator(".flow")).not.toContainText("{{");
    await page.screenshot({ path: "test-results/meeting-notes.png" });
  });
});

test.describe("link card", () => {
  test("the caret in a link shows where it goes, and Remove link takes it away", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: [{ raw: '<w:r><w:t xml:space="preserve">Read </w:t></w:r><w:hyperlink r:id="rLink"><w:r><w:rPr><w:rStyle w:val="Hyperlink"/></w:rPr><w:t>the guide</w:t></w:r></w:hyperlink><w:r><w:t xml:space="preserve"> first.</w:t></w:r>' }] }],
      extraRels: [{ id: "rLink", type: "hyperlink", target: "https://www.example.org/guide", external: true }],
    });
    await page.locator(".flow [data-href]").first().click();
    const card = page.locator(".link-card");
    await expect(card).toContainText("example.org");
    await expect(card).toContainText("https://www.example.org/guide");
    await card.getByRole("button", { name: "Remove link" }).click();
    await expect(page.locator(".flow [data-href]")).toHaveCount(0);
    await expect(page.locator(".flow")).toContainText("Read the guide first.");
  });
});

test.describe("opening an address from a document", () => {
  test("a file chip is saved with the document's full address, and Open opens it in the app", async ({ page }) => {
    await page.goto("/");
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    // Two documents in the Library: the first is in the recent list, the second is open.
    await inPage(page, async (w) => {
      await w.createIn(w.files.library, "");
    });
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.type("The plan");
    const first = await page.evaluate(() => location.hash);
    await inPage(page, async (w) => {
      await w.createIn(w.files.library, "");
    });
    await expect.poll(() => page.evaluate(() => location.hash)).not.toBe(first);
    await page.locator(".flow [data-p]").first().click();
    await page.keyboard.type("See @");
    const file = menu(page).getByRole("option", { name: /Untitled/ }).first();
    await expect(file).toBeVisible();
    await file.click();
    const chip = page.locator('.flow [data-chip="file"]').first();
    await expect(chip).toBeVisible();
    // Saved as a web address of this app, not a link to a bookmark (which Word would show as a dead link).
    const xml = await savedXml(page);
    const here = await page.evaluate(() => `${location.origin}${location.pathname}`);
    expect(xml).toContain(`${here}${first}`);
    expect(xml).not.toContain('"#/library');
    await chip.click();
    await page.locator(".chip-card").getByRole("button", { name: "Open" }).click();
    await expect.poll(() => page.evaluate(() => location.hash)).toBe(first);
    await expect(page.locator(".flow")).toContainText("The plan");
  });

  test("Ctrl-click opens a web address in a new tab and refuses other kinds with a notice", async ({ page }) => {
    await openBuilt(page, {
      sections: [{ paragraphs: [{ raw: '<w:hyperlink r:id="rWeb"><w:r><w:t>web</w:t></w:r></w:hyperlink><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:hyperlink r:id="rScript"><w:r><w:t>script</w:t></w:r></w:hyperlink>' }] }],
      extraRels: [
        { id: "rWeb", type: "hyperlink", target: "https://www.example.org/", external: true },
        { id: "rScript", type: "hyperlink", target: "javascript:alert(1)", external: true },
      ],
    });
    await page.evaluate(() => {
      const w = window as unknown as { __opened: string[] };
      w.__opened = [];
      window.open = ((u: string) => (w.__opened.push(String(u)), null)) as typeof window.open;
    });
    const opened = () => page.evaluate(() => (window as unknown as { __opened: string[] }).__opened);
    const mod = process.platform === "darwin" ? "Meta" : "Control";
    await page.locator('.flow [data-href]').filter({ hasText: "web" }).click({ modifiers: [mod] });
    await expect.poll(opened).toEqual(["https://www.example.org/"]);
    await page.locator('.flow [data-href]').filter({ hasText: "script" }).click({ modifiers: [mod] });
    await expect(page.locator(".toast")).toContainText("Docxtor only opens web and email addresses.");
    expect(await opened()).toEqual(["https://www.example.org/"]);
  });

  test("Escape closes a chip card and the keyboard is back in the document", async ({ page }) => {
    const chip = '<w:sdt><w:sdtPr><w:alias w:val="Person"/><w:tag w:val="docxtor:chip:person"/></w:sdtPr><w:sdtContent><w:hyperlink r:id="rBob"><w:r><w:t>Bob</w:t></w:r></w:hyperlink></w:sdtContent></w:sdt>';
    await openBuilt(page, {
      sections: [{ paragraphs: [{ raw: `<w:r><w:t xml:space="preserve">Ask </w:t></w:r>${chip}<w:r><w:t xml:space="preserve"> today</w:t></w:r>` }] }],
      extraRels: [{ id: "rBob", type: "hyperlink", target: "mailto:bob@example.com", external: true }],
    });
    await page.locator('.flow [data-chip="person"]').click();
    const card = page.locator(".chip-card");
    await expect(card.getByRole("button").first()).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(card).toHaveCount(0);
    expect(await page.evaluate(() => document.activeElement !== document.body && !!document.activeElement?.closest(".docxtor"))).toBe(true);
    // Typing goes into the document at once, next to the chip.
    await page.keyboard.type("!");
    await expect(page.locator(".flow [data-p]").first()).toContainText("!");
  });

  test("a person chip whose address hides more recipients offers no Send email", async ({ page }) => {
    const chip = (rid: string, name: string) => `<w:sdt><w:sdtPr><w:alias w:val="Person"/><w:tag w:val="docxtor:chip:person"/></w:sdtPr><w:sdtContent><w:hyperlink r:id="${rid}"><w:r><w:t>${name}</w:t></w:r></w:hyperlink></w:sdtContent></w:sdt>`;
    await openBuilt(page, {
      sections: [{ paragraphs: [{ raw: `${chip("rAda", "Ada")}<w:r><w:t xml:space="preserve"> and </w:t></w:r>${chip("rBob", "Bob")}` }] }],
      extraRels: [
        { id: "rAda", type: "hyperlink", target: "mailto:ada%40example.com%3Fbcc%3Dx%40evil.example", external: true },
        { id: "rBob", type: "hyperlink", target: "mailto:bob@example.com", external: true },
      ],
    });
    const card = page.locator(".chip-card");
    await page.locator('.flow [data-chip="person"]').filter({ hasText: "Bob" }).click();
    await expect(card.getByRole("link", { name: "Send email" })).toHaveAttribute("href", "mailto:bob@example.com");
    await page.keyboard.press("Escape");
    await page.locator('.flow [data-chip="person"]').filter({ hasText: "Ada" }).click();
    await expect(card).toBeVisible();
    await expect(card.getByRole("link", { name: "Send email" })).toHaveCount(0);
    await expect(card).not.toContainText("evil.example");
  });
});
