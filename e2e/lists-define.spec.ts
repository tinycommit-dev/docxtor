// Defining lists (a bullet, a number format, a multilevel list), setting a list item's number, and sorting.
import { caretAtEnd, chooseMenu, contextItem, dialog, expect, inPage, menuItem, openFixture, openMenu, RICH, test, type Locator, type Page } from "./helpers";
import { openBuilt } from "./support/docx-builder";

const MOD = "ControlOrMeta";

/** The parts of the engine these tests read. */
interface Eng {
  listDefinition(n: number): { name?: string; levels: Record<string, unknown>[] } | null;
  listDefinitions(): { numId: number; nums: number[]; bullet: boolean; levels: { text: string }[] }[];
  formatAt(p: { p: number; o: number }): { listNum?: number; listValue?: number; list?: string } | null;
  save(): Uint8Array;
}

const item = (page: Page, text: string): Locator => page.locator(".flow .fp").filter({ hasText: text });
const marker = (page: Page, text: string): Locator => item(page, text).locator(".lbl");
const bulletStyles = (page: Page) => page.getByRole("button", { name: "Bullet styles" });
const numberStyles = (page: Page) => page.getByRole("button", { name: "Numbering styles" });

/** Three new paragraphs after the document's last one, all selected. */
async function threeItems(page: Page, words = ["Apple", "Banana", "Cherry"]) {
  await caretAtEnd(page, RICH.last);
  for (const w of words) {
    await page.keyboard.press("Enter");
    await page.keyboard.type(w);
  }
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");
}

/** The .docx the page would save, as the text of one part. */
const part = (page: Page, name: string) =>
  inPage(
    page,
    async (w, n) => {
      const bytes = (w.fe.editor.engine as unknown as Eng).save();
      // A tiny reader for stored or deflated zip entries.
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      let at = 0;
      while (at + 30 < bytes.length && view.getUint32(at, true) === 0x04034b50) {
        const method = view.getUint16(at + 8, true);
        const size = view.getUint32(at + 18, true);
        const nameLen = view.getUint16(at + 26, true);
        const extra = view.getUint16(at + 28, true);
        const entry = new TextDecoder().decode(bytes.subarray(at + 30, at + 30 + nameLen));
        const data = bytes.subarray(at + 30 + nameLen + extra, at + 30 + nameLen + extra + size);
        if (entry === n) {
          if (method === 0) return new TextDecoder().decode(data);
          const stream = new Blob([data as unknown as BlobPart]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
          return await new Response(stream).text();
        }
        at += 30 + nameLen + extra + size;
      }
      return null;
    },
    name,
  );

test.describe("define your own lists", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
  });

  test("Define new bullet: a symbol of your own, its alignment and indents, in one undo step", async ({ page }) => {
    await threeItems(page);
    await bulletStyles(page).click();
    await page.getByRole("menuitem", { name: "Define new bullet…" }).click();
    const d = dialog(page, "Define new bullet");
    await expect(d).toBeVisible();
    // The symbol row, then a symbol of one's own typed in the box.
    await d.getByRole("button", { name: "✓", exact: true }).click();
    await expect(d.getByRole("button", { name: "✓", exact: true })).toHaveAttribute("aria-pressed", "true");
    await d.getByLabel("Other", { exact: true }).fill("➤");
    await expect(d.locator(".dl-preview .dl-label").first()).toHaveText("➤");
    await d.getByRole("radio", { name: "Center" }).click();
    await d.getByLabel("Bullet position (inches)").fill("0.25");
    await d.getByLabel("Text indent (inches)").fill("0.75");
    await d.getByRole("radio", { name: "Space" }).click();
    await d.getByLabel("Font", { exact: true }).selectOption("Georgia");
    await d.getByLabel("Bold").check();
    await d.getByRole("button", { name: "Automatic" }).click();
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toHaveCount(0);

    for (const w of ["Apple", "Banana", "Cherry"]) await expect(marker(page, w)).toHaveText("➤");
    const style = await marker(page, "Apple").evaluate((e) => {
      const c = getComputedStyle(e);
      return { weight: c.fontWeight, family: c.fontFamily.split(",")[0].replace(/['"]/g, "") };
    });
    expect(style).toEqual({ weight: "700", family: "Georgia" });
    // The definition as the file keeps it.
    const def = await inPage(page, (w) => {
      const e = w.fe.editor.engine as unknown as Eng;
      const ids = [...document.querySelectorAll(".flow .fp")].map((p) => Number((p as HTMLElement).dataset.p));
      const n = e.formatAt({ p: ids[ids.length - 1], o: 0 })!.listNum!;
      return e.listDefinition(n)!.levels[0];
    });
    expect(def).toMatchObject({ format: "bullet", text: "➤", align: "center", suffix: "space", left: 1080, hanging: 720, font: "Georgia", bold: true });
    const numbering = (await part(page, "word/numbering.xml"))!;
    expect(numbering).toContain('w:lvlJc w:val="center"');
    expect(numbering).toContain('w:suff w:val="space"');
    // The gallery lists it under In this document, and marks it as the current one.
    await bulletStyles(page).click();
    await expect(page.locator(".lg-title")).toHaveText("In this document");
    // The six styles, the lists the document had already, and this one last.
    expect(await page.locator(".list-tile").count()).toBeGreaterThanOrEqual(7);
    await expect(page.locator(".list-tile").last()).toHaveAttribute("aria-checked", "true");
    await expect(page.locator(".list-tile").last()).toHaveAccessibleName(/➤/);
    await page.keyboard.press("Escape");
    // One step to undo.
    await page.keyboard.press(`${MOD}+Z`);
    for (const w of ["Apple", "Banana", "Cherry"]) await expect(marker(page, w)).toHaveCount(0);
  });

  test("Define new bullet: a picture from a file is drawn at the label size, and cancelling leaves nothing", async ({ page }) => {
    await threeItems(page);
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new bullet…");
    const d = dialog(page, "Define new bullet");
    const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEklEQVR4nGP8z8Dwn4EIwDiqEAA/rQP/gdCAnQAAAABJRU5ErkJggg==", "base64");
    await d.locator('input[type="file"]').setInputFiles({ name: "star.png", mimeType: "image/png", buffer: png });
    await expect(d.getByText("The bullet is a picture.")).toBeVisible();
    await expect(d.locator(".dl-preview .dl-label img").first()).toBeVisible();
    // Cancel: no list, no picture in the file.
    await d.getByRole("button", { name: "Cancel" }).click();
    await expect(marker(page, "Apple")).toHaveCount(0);
    expect(await part(page, "word/media/docxtor1.png")).toBeNull();

    await chooseMenu(page, "Format", "Bullets and numbering", "Define new bullet…");
    await d.locator('input[type="file"]').setInputFiles({ name: "star.png", mimeType: "image/png", buffer: png });
    await d.getByRole("button", { name: "OK" }).click();
    const pics = page.locator(".flow .fp .lbl img.lbl-pic");
    await expect(pics).toHaveCount(3);
    // Ten points on the long side: 2 by 2 pixels scaled to 10pt, drawn at 13.3px.
    const box = await pics.first().boundingBox();
    expect(box!.width).toBeCloseTo(13.3, 0);
    await expect(pics.first()).toHaveJSProperty("complete", true);
    const rels = await part(page, "word/_rels/numbering.xml.rels");
    expect(rels).toContain("media/docxtor1.png");
    expect(await part(page, "word/numbering.xml")).toContain("w:numPicBullet");
  });

  test("Define new number format: the style, the text around the number and where it starts, with a live preview", async ({ page }) => {
    await threeItems(page);
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new number format…");
    const d = dialog(page, "Define new number format");
    await d.getByLabel("Number style", { exact: true }).selectOption({ label: "I, II, III" });
    await d.getByLabel("Number format", { exact: true }).fill("Step %1:");
    await d.getByLabel("Start at").fill("3");
    await expect(d.locator(".dl-preview .dl-label").first()).toHaveText("Step III:");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Apple")).toHaveText("Step III:");
    await expect(marker(page, "Banana")).toHaveText("Step IV:");
    await expect(marker(page, "Cherry")).toHaveText("Step V:");
    // The text must have a number in it.
    await item(page, "Apple").click();
    await page.keyboard.press("Shift+ArrowDown");
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new number format…");
    await d.getByLabel("Number format", { exact: true }).fill("no number here");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d.getByRole("alert")).toContainText("needs a number in it");
    await d.getByRole("button", { name: "Cancel" }).click();
    // The gallery for numbers has the new list, and the define lines.
    await numberStyles(page).click();
    // The document had a numbered list of its own already: it shows too.
    await expect(page.locator(".lg-title")).toHaveText("In this document");
    expect(await page.locator(".list-tile").count()).toBeGreaterThanOrEqual(7);
    await expect(page.locator(".list-tile").last()).toHaveAttribute("aria-checked", "true");
    await expect(page.getByRole("menuitem", { name: "Define new multilevel list…" })).toBeVisible();
    await page.keyboard.press("Escape");
  });

  test("a number style written in another script, and one that spells numbers out, show in the preview and the page", async ({ page }) => {
    await threeItems(page);
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new number format…");
    const d = dialog(page, "Define new number format");
    await d.getByLabel("Number style", { exact: true }).selectOption({ label: "One, Two, Three" });
    await d.getByLabel("Number format", { exact: true }).fill("%1)");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Banana")).toHaveText("Two)");
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new number format…");
    await d.getByLabel("Number style", { exact: true }).selectOption({ label: "一, 二, 三" });
    await expect(d.locator(".dl-preview .dl-label").first()).toContainText("一");
    await d.getByRole("button", { name: "Cancel" }).click();
  });

  test("Define new multilevel list: levels with earlier numbers, restart rules, legal numbering and headings linked to levels", async ({ page }) => {
    // Three headings and some text from the document: link level 1 to Heading 1 and level 2 to Heading 2.
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press("Backspace");
    await page.keyboard.type("Intro");
    await page.keyboard.press(`${MOD}+Alt+1`);
    await page.keyboard.press("Enter");
    await page.keyboard.type("Scope");
    await page.keyboard.press(`${MOD}+Alt+2`);
    await page.keyboard.press("Enter");
    await page.keyboard.type("Aim");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Method");
    await page.keyboard.press(`${MOD}+Alt+1`);
    await item(page, "Intro").click();

    await chooseMenu(page, "Format", "Bullets and numbering", "Define new multilevel list…");
    const d = dialog(page, "Define new multilevel list");
    await expect(d.getByRole("radio", { name: "Level 9" })).toBeVisible();
    // Level 1 is linked to Heading 1; level 2 reads 1.1 and is linked to Heading 2, in legal numbering.
    await d.getByLabel("Link level to style").selectOption("Heading 1");
    await d.getByRole("radio", { name: "Level 2" }).click();
    await expect(d.getByLabel("Number format", { exact: true })).toHaveValue("%1.%2.");
    await d.getByLabel("Link level to style").selectOption("Heading 2");
    await d.getByLabel("Legal style numbering (1, 2, 3 for earlier levels)").check();
    await d.getByLabel("Number style", { exact: true }).selectOption({ label: "a, b, c" });
    await expect(d.locator(".dl-preview .dl-line").nth(1).locator(".dl-label")).toHaveText("1.1.");
    // Level 3 never restarts.
    await d.getByRole("radio", { name: "Level 3" }).click();
    await d.getByLabel("Restart the list after").selectOption({ label: "Never" });
    await expect(d.getByLabel("Include the number from level")).toBeVisible();
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d).toHaveCount(0);

    await expect(marker(page, "Intro")).toHaveText("1.");
    await expect(marker(page, "Scope")).toHaveText("1.1.");
    await expect(marker(page, "Method")).toHaveText("2.");
    // The file: the level names its style, the style names the list and the level.
    const numbering = (await part(page, "word/numbering.xml"))!;
    expect(numbering).toContain('<w:pStyle w:val="Heading1"/>');
    expect(numbering).toContain('<w:pStyle w:val="Heading2"/>');
    expect(numbering).toContain('<w:lvlRestart w:val="0"/>');
    expect(numbering).toContain("<w:isLgl/>");
    expect(numbering).toContain('<w:multiLevelType w:val="multilevel"/>');
    const styles = (await part(page, "word/styles.xml"))!;
    expect(styles).toMatch(/w:styleId="Heading2"[\s\S]*?<w:numPr><w:ilvl w:val="1"\/><w:numId w:val="\d+"\/><\/w:numPr>/);
    // Starting from a list that is there: the dialog opens with its levels.
    await item(page, "Scope").click();
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new multilevel list…");
    await d.getByRole("radio", { name: "Level 2" }).click();
    await expect(d.getByLabel("Link level to style")).toHaveValue("Heading 2");
    await d.getByRole("button", { name: "Cancel" }).click();
    await page.keyboard.press(`${MOD}+Z`);
    await expect(marker(page, "Intro")).toHaveCount(0);
  });

  test("a list from In this document is applied to other paragraphs from the gallery", async ({ page }) => {
    await threeItems(page);
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new number format…");
    await dialog(page, "Define new number format").getByLabel("Number format", { exact: true }).fill("[%1]");
    await dialog(page, "Define new number format").getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Cherry")).toHaveText("[3]");
    // A plain paragraph above, and the gallery's new tile.
    const above = page.locator(`.flow [data-p="${RICH.intro}"]`);
    await above.click();
    await numberStyles(page).click();
    await page.locator(".list-tile").last().click();
    await expect(above.locator(".lbl")).toHaveText("[1]");
  });

  test("Escape in the symbol chooser closes only the chooser", async ({ page }) => {
    await threeItems(page);
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new bullet…");
    const d = dialog(page, "Define new bullet");
    await d.getByRole("button", { name: "More symbols…" }).click();
    const picker = dialog(page, "Special characters");
    await expect(picker).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(picker).toHaveCount(0);
    await expect(d).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(d).toHaveCount(0);
  });

  test("a symbol picked from the character chooser becomes the bullet", async ({ page }) => {
    await threeItems(page);
    await chooseMenu(page, "Format", "Bullets and numbering", "Define new bullet…");
    const d = dialog(page, "Define new bullet");
    await d.getByRole("button", { name: "More symbols…" }).click();
    const picker = dialog(page, "Special characters");
    await picker.getByRole("button", { name: /^check mark$/i }).first().click();
    await expect(picker).toHaveCount(0);
    await expect(d.getByLabel("Other", { exact: true })).toHaveValue("✓");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Apple")).toHaveText("✓");
  });
});

test.describe("set the number of a list item", () => {
  test.beforeEach(async ({ page }) => {
    await openFixture(page);
    await threeItems(page);
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Cherry")).toHaveText("3.");
    await item(page, "Banana").click();
  });

  test("start a new list at a number, from the menu and from the right-click menu", async ({ page }) => {
    await chooseMenu(page, "Format", "Bullets and numbering", "Set numbering value…");
    const d = dialog(page, "Set numbering value");
    await expect(d.getByText("This item is number 2 now.")).toBeVisible();
    await d.getByLabel("Set value to").fill("10");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Apple")).toHaveText("1.");
    await expect(marker(page, "Banana")).toHaveText("10.");
    await expect(marker(page, "Cherry")).toHaveText("11.");
    await page.keyboard.press(`${MOD}+Z`);
    await expect(marker(page, "Banana")).toHaveText("2.");

    await item(page, "Cherry").click({ button: "right" });
    await contextItem(page, "Set numbering value…").click();
    await expect(d).toBeVisible();
    await d.getByRole("button", { name: "Cancel" }).click();
  });

  test("continue the previous list and skip numbers", async ({ page }) => {
    // Split the list first, so there is a previous list to carry on.
    await chooseMenu(page, "Format", "Bullets and numbering", "Restart numbering");
    await expect(marker(page, "Banana")).toHaveText("1.");
    await chooseMenu(page, "Format", "Bullets and numbering", "Set numbering value…");
    const d = dialog(page, "Set numbering value");
    await d.getByRole("radio", { name: "Continue previous list" }).click();
    await expect(d.getByLabel("Set value to")).toBeDisabled();
    await expect(d.getByLabel("Set value to")).toHaveValue("1");
    await d.getByLabel("Advance value (skip numbers)").check();
    await expect(d.getByLabel("Set value to")).toBeEnabled();
    await d.getByLabel("Set value to").fill("5");
    await d.getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Banana")).toHaveText("5.");
    await expect(marker(page, "Cherry")).toHaveText("6.");
    // Carrying on without skipping joins the lists again.
    await chooseMenu(page, "Format", "Bullets and numbering", "Set numbering value…");
    await d.getByRole("radio", { name: "Continue previous list" }).click();
    await d.getByRole("button", { name: "OK" }).click();
    await expect(marker(page, "Banana")).toHaveText("2.");
    await expect(marker(page, "Cherry")).toHaveText("3.");
    // It already does: the dialog says there is nothing to join, and stays open.
    await chooseMenu(page, "Format", "Bullets and numbering", "Set numbering value…");
    await d.getByRole("radio", { name: "Continue previous list" }).click();
    await d.getByRole("button", { name: "OK" }).click();
    await expect(d.getByRole("alert")).toContainText("no earlier list");
    await d.getByRole("button", { name: "Cancel" }).click();
  });

  test("the command is for numbered list items only", async ({ page }) => {
    await item(page, "Apple").click();
    await openMenu(page, "Format");
    await menuItem(page, "Bullets and numbering").hover();
    await expect(menuItem(page, "Set numbering value…")).toBeEnabled();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    // A paragraph that isn't in a list.
    await page.locator(".flow .fp").first().click();
    await openMenu(page, "Format");
    await menuItem(page, "Bullets and numbering").hover();
    await expect(menuItem(page, "Set numbering value…")).toBeDisabled();
  });
});

test.describe("sort paragraphs", () => {
  const open = (page: Page, paragraphs: string[]) => openBuilt(page, { sections: [{ paragraphs }] });
  const texts = (page: Page) => page.locator(".flow .fp").evaluateAll((els) => els.map((e) => (e.textContent ?? "").replace(/ /g, " ").trim()).filter(Boolean));

  test("sort the selected paragraphs as text, ascending then descending, each in one undo step", async ({ page }) => {
    await open(page, ["pear", "Apple", "banana", "cherry"]);
    await page.keyboard.press(`${MOD}+A`);
    await chooseMenu(page, "Tools", "Sort paragraphs…");
    const d = dialog(page, "Sort paragraphs");
    await expect(d).toBeVisible();
    await d.getByRole("button", { name: "Sort" }).click();
    await expect(d).toHaveCount(0);
    expect(await texts(page)).toEqual(["Apple", "banana", "cherry", "pear"]);
    // The paragraphs stay selected.
    expect(await page.evaluate(() => window.getSelection()?.toString().replace(/\s+/g, " ").trim())).toBe("Apple banana cherry pear");

    await chooseMenu(page, "Tools", "Sort paragraphs…");
    await d.getByRole("radio", { name: "Descending" }).first().click();
    await d.getByRole("button", { name: "Sort" }).click();
    expect(await texts(page)).toEqual(["pear", "cherry", "banana", "Apple"]);
    await page.keyboard.press(`${MOD}+Z`);
    expect(await texts(page)).toEqual(["Apple", "banana", "cherry", "pear"]);
    await page.keyboard.press(`${MOD}+Z`);
    expect(await texts(page)).toEqual(["pear", "Apple", "banana", "cherry"]);
  });

  test("sort by numbers or dates, keep a header row and sort by a field", async ({ page }) => {
    await open(page, ["Name\tCount", "b\t10", "a\t9", "c\t100", "d\t"]);
    await page.keyboard.press(`${MOD}+A`);
    await chooseMenu(page, "Tools", "Sort paragraphs…");
    const d = dialog(page, "Sort paragraphs");
    await d.getByLabel("Sort by").selectOption("2");
    await d.locator("#so-type-0").selectOption("number");
    await d.getByRole("radio", { name: "Header row", exact: true }).click();
    await d.getByRole("button", { name: "Sort" }).click();
    expect(await texts(page)).toEqual(["Name\tCount", "d", "a\t9", "b\t10", "c\t100"]);

    // A second key breaks ties; dates are read from the text.
    await open(page, ["Due 5 June 2024", "Due 1 May 2023", "Due 2024-01-31", "later"]);
    await page.keyboard.press(`${MOD}+A`);
    await chooseMenu(page, "Tools", "Sort paragraphs…");
    await d.locator("#so-type-0").selectOption("date");
    await d.getByRole("radio", { name: "Descending" }).first().click();
    await d.getByRole("button", { name: "Sort" }).click();
    expect(await texts(page)).toEqual(["Due 5 June 2024", "Due 2024-01-31", "Due 1 May 2023", "later"]);
  });

  test("sorting list items keeps their list, and the command needs two paragraphs", async ({ page }) => {
    await open(page, ["Mango", "Fig", "Date"]);
    await page.keyboard.press(`${MOD}+A`);
    await page.keyboard.press(`${MOD}+Shift+7`);
    await expect(marker(page, "Mango")).toHaveText("1.");
    await chooseMenu(page, "Tools", "Sort paragraphs…");
    await dialog(page, "Sort paragraphs").getByRole("button", { name: "Sort" }).click();
    expect(await texts(page)).toEqual(["1.Date", "2.Fig", "3.Mango"]);
    await expect(marker(page, "Date")).toHaveText("1.");
    await expect(marker(page, "Mango")).toHaveText("3.");
    // With only the caret in a paragraph the command is off.
    await page.keyboard.press("ArrowDown");
    await openMenu(page, "Tools");
    await expect(menuItem(page, "Sort paragraphs…")).toBeDisabled();
  });
});
