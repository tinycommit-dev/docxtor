// Document protection: a read-only document that can be edited only in its ranges, a forms-only document, Mark as final,
// and Restrict editing set from Docxtor (with and without a password, with exceptions and limited styles).
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { checkbox, endOfLine, startOfLine, protect, run, savedPart, text } from "./support/controls-markup";
import { chooseMenu, dialog, expect, inPage, openMenu, menuItem, test, type Page } from "./helpers";

const RANGE = (id: number, words: string, group = "everyone") => `<w:permStart w:id="${id}" w:edGrp="${group}"/>${run(words)}<w:permEnd w:id="${id}"/>`;
const STYLES = '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:qFormat/></w:style>';
const open = (page: Page, spec: Partial<DocSpec> & { paragraphs: DocSpec["sections"][number]["paragraphs"] }) => openBuilt(page, { settings: spec.settings, styles: STYLES, sections: [{ paragraphs: spec.paragraphs }] });
const strip = (page: Page) => page.locator(".editor-host .notice-strip");
const toast = (page: Page) => page.locator(".docxtor-layer .toast");
const para = (page: Page, i: number) => page.locator(".flow .fp").nth(i);

test.describe("document protection", () => {
  test("a read-only document is edited only in its ranges, which are tinted with brackets", async ({ page }) => {
    await open(page, { settings: protect("readOnly"), paragraphs: ["Locked line", { raw: run("Before ") + RANGE(1, "open words") + run(" after") }, "Another locked line"] });
    await expect(strip(page)).toContainText("This document is protected. You can only edit the highlighted parts.");
    await expect(page.locator(".statusbar .protect")).toHaveText("Protected: read only");
    // The range has a bracket at each end and a tint behind it.
    await expect(page.locator(".flow .perm-start")).toHaveText(/open words|Before open words after/);
    expect(await page.evaluate(() => (CSS as unknown as { highlights: Map<string, unknown> }).highlights.has("docxtor-perm"))).toBe(true);
    await page.locator(".flow").screenshot({ path: "test-results/protection-ranges.png" });
    // Typing in a locked line is refused, and the line is as it was.
    await para(page, 0).click();
    await endOfLine(page);
    await page.keyboard.type("zzz");
    await expect(toast(page)).toContainText("This document is protected. You can only change the parts marked as editable.");
    await expect(para(page, 0)).toHaveText("Locked line");
    // Typing and Enter inside the range work.
    await page.locator(".flow .fp").nth(1).getByText("open words").click();
    await endOfLine(page);
    await page.keyboard.press("ArrowLeft");
    expect(await inPage(page, (w) => w.fe.editor.sel !== null)).toBe(true);
    const range = await inPage(page, (w) => (w.fe.editor.engine as unknown as { protectionInfo(): { ranges: { from: { p: number; o: number }; to: { p: number; o: number } }[] } }).protectionInfo().ranges[0]);
    await inPage(page, (w, r) => w.fe.editor.exec({ op: "insert_text", at: { p: r.from.p, o: r.from.o + 4 }, text: "-" }), range);
    await expect(para(page, 1)).toContainText("open- words");
    const xml = await savedPart(page);
    expect(xml).toContain("w:permStart");
    expect(xml).toContain("w:permEnd");
  });

  test("a forms document lets controls be filled and nothing else be typed", async ({ page }) => {
    await open(page, { settings: protect("forms"), paragraphs: ["Fixed words", { raw: run("Agree ") + checkbox() }, { raw: run("Name ") + text() }] });
    await expect(strip(page)).toContainText("You can only fill in the form fields.");
    await expect(page.locator(".statusbar .protect")).toHaveText("Protected: forms only");
    await para(page, 0).click();
    await endOfLine(page);
    await page.keyboard.type("x");
    await expect(toast(page)).toContainText("fill in the form fields and content controls");
    await expect(para(page, 0)).toHaveText("Fixed words");
    await page.locator(".flow .ctl-check").click();
    await expect(page.locator(".flow .ctl-check")).toHaveText("☒");
    await page.locator('.flow .ctl[data-ck="text"]').click();
    await page.keyboard.type("Ada");
    await expect(para(page, 2)).toHaveText("Name Ada");
  });

  test("tracked changes protection says Docxtor can't track changes yet and still allows editing", async ({ page }) => {
    await open(page, { settings: protect("trackedChanges"), paragraphs: ["Some text"] });
    await expect(strip(page)).toContainText("Docxtor can't track changes yet");
    await para(page, 0).click();
    await endOfLine(page);
    await page.keyboard.type("!");
    await expect(para(page, 0)).toHaveText("Some text!");
    await strip(page).getByRole("button", { name: "Got it" }).click();
    await expect(strip(page)).toBeHidden();
  });

  test("Mark as final opens read-only with a bar, and Edit anyway turns editing back on", async ({ page }) => {
    await open(page, { paragraphs: ["A finished document"] });
    await chooseMenu(page, "File", "Mark as final");
    await expect(strip(page)).toContainText("This document is marked as final. Editing is turned off.");
    await expect(page.locator(".statusbar .protect")).toHaveText("Marked as final");
    await expect(page.locator(".flow")).toHaveAttribute("contenteditable", "false");
    const part = await savedPart(page, "docProps/custom.xml");
    expect(part).toContain('name="_MarkAsFinal"');
    expect(part).toContain("<vt:bool>true</vt:bool>");
    // Opened again, the file is read-only with the bar.
    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } }; open(b: Uint8Array, n?: string): Promise<unknown> } } }).docxtor;
      await w.fe.open(w.fe.editor.engine.save(), "final.docx");
    });
    await expect(strip(page)).toContainText("marked as final");
    await expect(page.locator(".flow")).toHaveAttribute("contenteditable", "false");
    await page.screenshot({ path: "test-results/protection-final.png" });
    await strip(page).getByRole("button", { name: "Edit anyway" }).click();
    await expect(page.locator(".flow")).toHaveAttribute("contenteditable", "true");
    await expect(strip(page)).toBeHidden();
    await expect(page.locator(".statusbar .protect")).toBeHidden();
    expect(await savedPart(page, "docProps/custom.xml")).not.toContain("_MarkAsFinal");
    await para(page, 0).click();
    await endOfLine(page);
    await page.keyboard.type("!");
    await expect(para(page, 0)).toHaveText("A finished document!");
  });

  test("Restrict editing starts and stops protection with a password", async ({ page }) => {
    await open(page, { paragraphs: ["First line", "Second line"] });
    await chooseMenu(page, "Tools", "Restrict editing…");
    const d = dialog(page, "Restrict editing");
    await d.getByRole("button", { name: "Start enforcing protection" }).click();
    await expect(d).toContainText("Choose a restriction first.");
    await d.getByLabel("Allow only this type of editing in the document").check();
    await d.getByLabel("New password (optional)").fill("secret");
    await d.getByLabel("Confirm password").fill("different");
    await d.getByRole("button", { name: "Start enforcing protection" }).click();
    await expect(d).toContainText("The passwords don't match.");
    await d.getByLabel("Confirm password").fill("secret");
    await d.getByRole("button", { name: "Start enforcing protection" }).click();
    await expect(d).toBeHidden();
    await expect(strip(page)).toContainText("It can't be edited");
    const settings = await savedPart(page, "word/settings.xml");
    for (const want of ['w:edit="readOnly"', 'w:enforcement="1"', 'w:cryptProviderType="rsaAES"', 'w:cryptAlgorithmSid="14"', 'w:cryptSpinCount="100000"', "w:hash=", "w:salt="]) expect(settings).toContain(want);
    await para(page, 0).click();
    await page.keyboard.type("x");
    await expect(para(page, 0)).toHaveText("First line");
    // Stopping needs the password.
    await strip(page).getByRole("button", { name: "Stop protection…" }).click();
    const stop = dialog(page, "Restrict editing");
    await stop.getByLabel("Password").fill("nope");
    await stop.getByRole("button", { name: "Stop protection" }).click();
    await expect(stop).toContainText("That password isn't right. Try again.");
    await stop.getByLabel("Password").fill("secret");
    await stop.getByRole("button", { name: "Stop protection" }).click();
    await expect(strip(page)).toBeHidden();
    await expect(page.locator(".statusbar .protect")).toBeHidden();
    await para(page, 0).click();
    await endOfLine(page);
    await page.keyboard.type("!");
    await expect(para(page, 0)).toHaveText("First line!");
    expect(await savedPart(page, "word/settings.xml")).toContain('w:enforcement="0"');
  });

  test("Restrict editing marks the selection as an editable range for everyone", async ({ page }) => {
    await open(page, { paragraphs: ["Alpha beta gamma", "Delta"] });
    const first = para(page, 0);
    await first.getByText("Alpha beta gamma").click();
    await startOfLine(page);
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await page.keyboard.press("Shift+ArrowRight");
    await chooseMenu(page, "Tools", "Restrict editing…");
    const d = dialog(page, "Restrict editing");
    await d.getByLabel("Allow only this type of editing in the document").check();
    await d.getByRole("button", { name: "Mark the selection as editable" }).click();
    await expect(d).toContainText("1 editable range.");
    await d.getByRole("button", { name: "Start enforcing protection" }).click();
    await expect(strip(page)).toContainText("highlighted parts");
    // Only "Alpha" is open.
    await first.getByText("Alpha").click();
    await startOfLine(page);
    await page.keyboard.type("A");
    await expect(first).toContainText("AAlpha");
    await para(page, 1).click();
    await page.keyboard.type("z");
    await expect(para(page, 1)).toHaveText("Delta");
    const xml = await savedPart(page);
    expect(xml).toMatch(/<w:permStart w:id="\d+" w:edGrp="everyone"\/>/);
  });

  test("limiting formatting to styles blocks direct formatting and locked styles", async ({ page }) => {
    await open(page, { paragraphs: ["Some text"] });
    await chooseMenu(page, "Tools", "Restrict editing…");
    const d = dialog(page, "Restrict editing");
    await d.getByLabel("Limit formatting to a selection of styles").check();
    await d.getByRole("button", { name: "Allow none" }).click();
    await d.getByLabel(/^heading 1$/i).check();
    await d.getByRole("button", { name: "Start enforcing protection" }).click();
    await expect(strip(page)).toContainText("Formatting is limited to some styles");
    await para(page, 0).click();
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.press("ControlOrMeta+b");
    await expect(toast(page)).toContainText("Formatting is limited in this document");
    expect(await savedPart(page, "word/styles.xml")).toContain("<w:locked/>");
    // Text still edits.
    await endOfLine(page);
    await page.keyboard.type("!");
    await expect(para(page, 0)).toHaveText("Some text!");
  });
});
