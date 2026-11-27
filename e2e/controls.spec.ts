// Content controls and legacy form fields: check boxes that tick, lists and calendars that open, placeholder text
// that is grey and goes when you type, locks, repeating sections, pictures, data binding, Word's older form fields,
// the Insert menu and Properties dialog, and kinds that extensions add.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { checkbox, combo, endOfLine, startOfLine, date, dropdown, filled, formCheck, formDrop, formText, p, picture, repeating, run, savedPart, text } from "./support/controls-markup";
import { chooseMenu, dialog, expect, inPage, test, type Page } from "./helpers";

const PLACEHOLDER_STYLE = '<w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/></w:style><w:style w:type="character" w:styleId="PlaceholderText"><w:name w:val="Placeholder Text"/><w:basedOn w:val="DefaultParagraphFont"/><w:rPr><w:color w:val="808080"/></w:rPr></w:style>';

const open = (page: Page, ...blocks: ({ raw: string } | { body: string } | string)[]) => openBuilt(page, { sections: [{ paragraphs: ["Intro line", ...blocks] }], styles: PLACEHOLDER_STYLE });
const openSpec = (page: Page, spec: DocSpec) => openBuilt(page, { styles: PLACEHOLDER_STYLE, ...spec });
const ctl = (page: Page, kind: string) => page.locator(`.flow .ctl[data-ck="${kind}"]`);
const popup = (page: Page) => page.locator(".docxtor-layer .pop");
const lastPara = (page: Page) => page.locator(".flow .fp").last();

test.describe("content controls", () => {
  test("a check box ticks on a click and on Space, and the file says so", async ({ page }) => {
    await open(page, { raw: run("Agree ") + checkbox() });
    const box = ctl(page, "checkbox");
    await expect(box).toHaveText("☐");
    await expect(box).toHaveAttribute("aria-checked", "false");
    await box.click();
    await expect(box).toHaveText("☒");
    await expect(box).toHaveAttribute("aria-checked", "true");
    expect(await savedPart(page)).toContain('<w14:checked w14:val="1"/>');
    // The box stays selected, so Space ticks it again.
    await page.keyboard.press("Space");
    await expect(box).toHaveText("☐");
    expect(await savedPart(page)).toContain('<w14:checked w14:val="0"/>');
    // Typing other letters into it does nothing.
    await page.keyboard.type("x");
    await expect(box).toHaveText("☐");
    await expect(lastPara(page)).toHaveText("Agree ☐");
    // Its tab names it.
    await expect(page.locator(".docxtor-layer .ctl-tab .ctl-name")).toHaveText("Agree");
  });

  test("a drop-down list opens, the choice goes in and the placeholder goes", async ({ page }) => {
    await open(page, { raw: run("Colour: ") + dropdown() });
    const el = ctl(page, "dropDown");
    await expect(el).toHaveClass(/ctl-ph/);
    await el.click();
    const items = popup(page).locator(".menu-item");
    await expect(items).toHaveText(["Choose an item.", "Red", "Blue"]);
    await items.nth(2).click();
    await expect(lastPara(page)).toHaveText("Colour: Blue");
    await expect(ctl(page, "dropDown")).not.toHaveClass(/ctl-ph/);
    const xml = await savedPart(page);
    expect(xml).not.toContain("showingPlcHdr");
    expect(xml).not.toContain("PlaceholderText");
    // The tab's arrow opens the list again, with the choice marked; the first entry brings the placeholder back.
    await ctl(page, "dropDown").click();
    await expect(popup(page).locator('.menu-item[aria-selected="true"]')).toHaveText("Blue");
    await popup(page).locator(".menu-item").first().click();
    await expect(lastPara(page)).toHaveText("Colour: Choose an item.");
    expect(await savedPart(page)).toContain("showingPlcHdr");
  });

  test("a combo box takes typing as well as a choice", async ({ page }) => {
    await open(page, { raw: run("Fruit: ") + combo() });
    const el = ctl(page, "comboBox");
    await el.click();
    // The text is selected whole, so typing replaces the placeholder.
    await page.keyboard.type("Plum");
    await expect(lastPara(page)).toHaveText("Fruit: Plum");
    expect(await savedPart(page)).not.toContain("showingPlcHdr");
    await page.locator(".docxtor-layer .ctl-tab .ctl-btn").first().click();
    await popup(page).locator(".menu-item").filter({ hasText: "Pear" }).click();
    await expect(lastPara(page)).toHaveText("Fruit: Pear");
  });

  test("a date picker opens a calendar and writes the date and its text", async ({ page }) => {
    await open(page, { raw: run("Due: ") + date() });
    await ctl(page, "date").click();
    const cal = popup(page).locator(".cal");
    await expect(cal).toBeVisible();
    // A day of this month.
    await cal.locator(".cal-day").nth(14).click();
    const out = (await lastPara(page).textContent()) ?? "";
    expect(out).toMatch(/^Due: \w+day, \w+ 15, \d{4}$/);
    expect(await savedPart(page)).toMatch(/w:fullDate="\d{4}-\d{2}-15T00:00:00Z"/);
    // Clear puts the placeholder back.
    await ctl(page, "date").click();
    await popup(page).getByRole("button", { name: "Clear" }).click();
    await expect(lastPara(page)).toHaveText("Due: Click or tap to enter a date.");
    expect(await savedPart(page)).not.toContain("fullDate");
    await ctl(page, "date").click();
    await page.screenshot({ path: "test-results/controls-calendar.png" });
  });

  test("placeholder text is grey, selected whole, replaced by typing and comes back when emptied", async ({ page }) => {
    await open(page, { raw: run("Name: ") + text() });
    const el = ctl(page, "text");
    await expect(el).toHaveClass(/ctl-ph/);
    expect(await el.evaluate((e) => getComputedStyle(e).color)).toBe("rgb(128, 128, 128)");
    await el.click();
    expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("Click or tap here to enter text.");
    await page.keyboard.type("Ada");
    await expect(lastPara(page)).toHaveText("Name: Ada");
    await expect(ctl(page, "text")).not.toHaveClass(/ctl-ph/);
    const xml = await savedPart(page);
    expect(xml).not.toContain("showingPlcHdr");
    expect(xml).not.toContain("PlaceholderText");
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    await page.keyboard.press("Backspace");
    await expect(lastPara(page)).toHaveText("Name: Click or tap here to enter text.");
    await expect(ctl(page, "text")).toHaveClass(/ctl-ph/);
    expect(await savedPart(page)).toContain("showingPlcHdr");
    await page.locator(".flow").screenshot({ path: "test-results/controls-placeholder.png" });
  });

  test("locks are honoured: a locked control can't be deleted and locked content can't be edited", async ({ page }) => {
    await open(page, { raw: run("A ") + filled("sdtLocked", "kept") + run(" B") }, { raw: run("C ") + filled("contentLocked", "frozen", 107) + run(" D") });
    // sdtLocked: edit the words, but not remove the control.
    const first = page.locator(".flow .fp").nth(1);
    await first.locator(".ctl").click();
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.press("Delete");
    await expect(page.locator(".docxtor-layer .toast")).toContainText("can't be deleted");
    await expect(first).toHaveText("A kept B");
    await expect(first.locator(".ctl")).toHaveClass(/ctl-locked/);
    // contentLocked: the words are frozen.
    const second = page.locator(".flow .fp").nth(2);
    await second.locator(".ctl").click();
    await page.keyboard.type("zzz");
    await expect(page.locator(".docxtor-layer .toast")).toContainText("can't be edited");
    await expect(second).toHaveText("C frozen D");
    // The tab says it is locked.
    await expect(page.locator(".docxtor-layer .ctl-tab .ctl-lock")).toBeVisible();
  });

  test("a repeating section gets another item with the Add item button", async ({ page }) => {
    await open(page, { body: repeating(2) });
    const lines = page.locator(".flow .fp").filter({ hasText: /^Line \d/ });
    await expect(lines).toHaveCount(2);
    await lines.first().click();
    const add = page.locator('.docxtor-layer .ctl-tab .ctl-btn[aria-label="Add item"]');
    await expect(add).toBeVisible();
    await add.click();
    await expect(page.locator(".flow .fp").filter({ hasText: /^Line \d/ })).toHaveCount(3);
    // The new item starts empty: its control shows the placeholder.
    await expect(page.locator(".flow .fp").filter({ hasText: /^Line 1/ }).nth(1)).toContainText("Click or tap here to enter text.");
    const xml = await savedPart(page);
    expect(xml.match(/<w15:repeatingSectionItem\/>/g)).toHaveLength(3);
    // Remove an item; the last one can't go.
    await page.locator('.docxtor-layer .ctl-tab .ctl-btn[aria-label="Remove item"]').click();
    await expect(page.locator(".flow .fp").filter({ hasText: /^Line \d/ })).toHaveCount(2);
  });

  test("a picture control takes a picture from a file", async ({ page }) => {
    await open(page, { raw: run("Logo: ") + picture() });
    const chooser = page.waitForEvent("filechooser");
    await ctl(page, "picture").click();
    const file = await chooser;
    // A 2x2 PNG.
    const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8Dwn4EIwESMolGFxCkEACKRAQ0Sd5bCAAAAAElFTkSuQmCC", "base64");
    await file.setFiles({ name: "logo.png", mimeType: "image/png", buffer: png });
    await expect(page.locator(".flow .fp").last().locator("img")).toHaveCount(1);
    const xml = await savedPart(page);
    expect(xml).toContain("<w:picture/>");
    expect(xml).not.toContain("showingPlcHdr");
    expect(xml).toContain("<w:drawing");
  });

  test("a control bound to custom XML shows its value and writes edits back", async ({ page }) => {
    const bound = `<w:sdt><w:sdtPr><w:alias w:val="Customer"/><w:id w:val="300"/><w:dataBinding w:prefixMappings="xmlns:ns0='urn:acme'" w:xpath="/ns0:order[1]/ns0:customer[1]" w:storeItemID="{11111111-2222-3333-4444-555555555555}"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>stale</w:t></w:r></w:sdtContent></w:sdt>`;
    await openSpec(page, {
      sections: [{ paragraphs: ["Intro line", { raw: run("Customer: ") + bound }] }],
      extra: [
        { name: "customXml/item1.xml", xml: '<order xmlns="urn:acme"><customer>Acme</customer></order>', rel: { type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml", target: "../customXml/item1.xml" }, override: "application/xml", partRel: { owner: "customXml/item1.xml", type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps", target: "itemProps1.xml" } },
        { name: "customXml/itemProps1.xml", xml: '<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{11111111-2222-3333-4444-555555555555}"/>', override: "application/vnd.openxmlformats-officedocument.customXmlProperties+xml" },
      ],
    });
    await expect(lastPara(page)).toHaveText("Customer: Acme");
    await ctl(page, "text").click();
    await endOfLine(page);
    await page.keyboard.type("!");
    await expect(lastPara(page)).toHaveText("Customer: Acme!");
    expect(await savedPart(page, "customXml/item1.xml")).toContain("<customer>Acme!</customer>");
  });

  test("Insert, Form controls puts a control in, and Properties writes what Word writes", async ({ page }) => {
    await open(page, "Some text to add to");
    await page.locator(".flow .fp").last().click();
    await endOfLine(page);
    await chooseMenu(page, "Insert", "Form controls", "Plain text");
    const el = ctl(page, "text");
    await expect(el).toHaveText("Click or tap here to enter text.");
    expect(await page.evaluate(() => window.getSelection()?.toString())).toBe("Click or tap here to enter text.");
    await page.locator('.docxtor-layer .ctl-tab .ctl-btn[aria-label="Properties"]').click();
    const d = dialog(page, /Plain text properties/);
    await d.getByLabel("Title").fill("Applicant");
    await d.getByLabel("Tag").fill("applicant");
    await d.getByLabel("Show as").selectOption("tags");
    await d.getByLabel("Automatic").uncheck();
    await d.getByLabel("Colour").fill("#ff0000");
    await d.getByLabel("The contents can't be edited").check();
    await d.getByLabel("Placeholder text").fill("Your full name");
    await d.getByRole("button", { name: "OK" }).click();
    const xml = await savedPart(page);
    for (const want of ['<w:alias w:val="Applicant"/>', '<w:tag w:val="applicant"/>', '<w:lock w:val="contentLocked"/>', '<w15:appearance w15:val="tags"/>', '<w15:color w:val="FF0000"/>', "<w:text/>"]) expect(xml).toContain(want);
    expect(await savedPart(page, "word/glossary/document.xml")).toContain("Your full name");
    await expect(page.locator(".flow .fp").last()).toContainText("Your full name");
  });

  test("the Insert menu offers every kind and legacy form fields", async ({ page }) => {
    await open(page, "Some text");
    await page.locator(".flow .fp").last().click();
    await endOfLine(page);
    for (const [label, kind] of [["Check box", "checkbox"], ["Drop-down list", "dropDown"], ["Combo box", "comboBox"], ["Date picker", "date"], ["Rich text", "richText"], ["Picture", "picture"]] as const) {
      await chooseMenu(page, "Insert", "Form controls", label);
      await expect(ctl(page, kind).first()).toBeVisible();
      // Collapse the selection (the new control's placeholder is selected), then go to the end of the line.
      await page.keyboard.press("ArrowRight");
      await endOfLine(page);
    }
    const xml = await savedPart(page);
    for (const want of ["<w14:checkbox", "<w:dropDownList", "<w:comboBox", "<w:date", "<w:richText/>", "<w:picture/>"]) expect(xml).toContain(want);
    await chooseMenu(page, "Insert", "Form controls", "Repeating section");
    await expect(page.locator(".flow .fc-ctl[data-ck='repeating']")).toHaveCount(1);
    await page.keyboard.press("Escape");
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Insert", "Form controls", "Legacy form fields", "Check box field");
    await expect(page.locator(".flow .ghost.ff")).toHaveCount(1);
  });

  test("Word's older form fields: a blank takes typing, a box ticks and a list opens", async ({ page }) => {
    await open(page, { raw: run("Name: ") + formText('<w:maxLength w:val="6"/><w:format w:val="UPPERCASE"/>') + run("  Tick: ") + formCheck() + run("  Drink: ") + formDrop() });
    const row = lastPara(page);
    // The blank is shaded, and typing at its start goes into it, in capitals and no longer than its limit.
    await expect(row.locator('[data-fld="FORMTEXT"]')).toBeVisible();
    // Put the caret at the start of the blank by clicking its left edge.
    const box = (await row.locator('[data-fld="FORMTEXT"]').boundingBox())!;
    await page.mouse.click(box.x + 1, box.y + box.height / 2);
    await page.keyboard.type("abcdefghi");
    await expect(row.locator('[data-fld="FORMTEXT"]')).toHaveText("ABCDEF");
    // The check box.
    const cb = row.locator(".ghost.ff");
    await expect(cb).toHaveAttribute("aria-checked", "false");
    await cb.click();
    await expect(cb).toHaveAttribute("aria-checked", "true");
    expect(await savedPart(page)).toContain("<w:checked/>");
    // The drop-down.
    const sel = row.locator(".frm-select");
    await expect(sel).toContainText("Tea");
    await sel.click();
    await popup(page).locator(".menu-item").filter({ hasText: "Coffee" }).click();
    await expect(row.locator(".frm-select")).toContainText("Coffee");
    expect(await savedPart(page)).toContain('<w:result w:val="1"/>');
    await page.locator(".flow").screenshot({ path: "test-results/controls-legacy.png" });
  });

  test("an extension can add a kind of control and draw and edit it", async ({ page }) => {
    await open(page, { raw: run("Rating: ") + `<w:sdt><w:sdtPr><w:alias w:val="Stars"/><w:tag w:val="acme-rating"/><w:id w:val="400"/><w:richText/></w:sdtPr><w:sdtContent><w:r><w:t>3</w:t></w:r></w:sdtContent></w:sdt>` });
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { use(x: unknown): Promise<void> } } }).docxtor;
      await w.fe.use({
        id: "acme.rating",
        name: "Rating",
        activate(ctx: { contentControls: { register(c: unknown): void } }) {
          ctx.contentControls.register({
            kind: "acme-rating",
            label: "Rating",
            render: (els: HTMLElement[]) => els.forEach((e) => e.classList.add("acme-stars")),
            edit: (api: { set(v: { text: string }): void; popover(c: HTMLElement): void }) => {
              const b = document.createElement("button");
              b.textContent = "5 stars";
              b.className = "acme-pick";
              b.onclick = () => api.set({ text: "5" });
              api.popover(b);
            },
          });
        },
      });
    });
    await expect(page.locator(".flow .acme-stars")).toHaveText("3");
    await page.locator(".flow .acme-stars").click();
    await page.locator(".acme-pick").click();
    await expect(page.locator(".flow .acme-stars, .flow .ctl[data-ck='richText']").first()).toHaveText("5");
    expect(await inPage(page, (w) => w.fe.editor.engine!.text())).toContain("Rating: 5");
  });
});
