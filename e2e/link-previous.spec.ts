// "Link to previous" for headers and footers: a section after the first shows the header of the section before
// it until it is unlinked (it then gets a copy of its own), as in Word. The engine's side (the op, the parts,
// undo and the saved file) is tested in Rust.
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { chooseMenu, expect, inPage, menuItem, openMenu, test, type Page } from "./helpers";

const MOD = "ControlOrMeta";

const setup = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.pageSetup());
const sheet = (page: Page, k: number) => page.locator(".sheet").nth(k);
const hf = (page: Page, k: number, zone: "hdr" | "ftr") => sheet(page, k).locator(`.${zone}`).textContent();
const strip = (page: Page) => page.locator(".story-strip");
const linkButton = (page: Page) => strip(page).getByRole("button", { name: "Link to previous" });
const portrait = (paragraphs: string[], more: Partial<SectionSpec> = {}): SectionSpec => ({ paragraphs, ...more });

/** Double-click in the top margin of a sheet, which edits the header of the page. */
async function editHeader(page: Page, k: number) {
  const zoom = await inPage(page, (w) => w.fe.editor.zoom);
  // The page may still be scrolling to the caret it came back to: scroll once that has stopped.
  await page.evaluate(
    (i) =>
      new Promise<void>((done) => {
        const c = document.querySelector(".canvas")!;
        let last = -1;
        let still = 0;
        const step = () => {
          still = c.scrollTop === last ? still + 1 : 0;
          last = c.scrollTop;
          if (still < 4) return requestAnimationFrame(step);
          c.scrollTop += document.querySelectorAll(".sheet")[i].getBoundingClientRect().top - c.getBoundingClientRect().top - 60;
          requestAnimationFrame(() => requestAnimationFrame(() => done()));
        };
        step();
      }),
    k,
  );
  const b = (await sheet(page, k).boundingBox())!;
  await page.mouse.dblclick(b.x + b.width / 2, b.y + 20 * zoom);
  await expect(strip(page)).toBeVisible();
}

test.describe("Link to previous", () => {
  test.beforeEach(async ({ page }) => {
    // Two sections; the second has no header of its own, so it shows the first's.
    await openBuilt(page, { sections: [portrait(["Section one"], { headers: { default: "Header one" } }), portrait(["Section two"], { start: "nextPage" })] });
  });

  test("a later section starts linked: the area says Same as previous, the button is on, and typing changes the shared header", async ({ page }) => {
    expect(await hf(page, 1, "hdr")).toBe("Header one");
    await editHeader(page, 1);
    await expect(strip(page)).toContainText("Header on page 2");
    await expect(page.locator(".story-edit")).toHaveAttribute("data-linked", "Same as previous");
    await expect(linkButton(page)).toHaveAttribute("aria-pressed", "true");
    await expect(linkButton(page)).toBeEnabled();
    // Typing here edits the one header both sections show.
    await page.keyboard.press("End");
    await page.keyboard.type(" shared");
    await page.keyboard.press("Escape");
    expect(await hf(page, 0, "hdr")).toBe("Header one shared");
    expect(await hf(page, 1, "hdr")).toBe("Header one shared");
  });

  test("unlinking gives the section its own copy: typing there leaves the first section's header alone; linking again shows the first's again", async ({ page }) => {
    await editHeader(page, 1);
    await linkButton(page).click();
    // The same text to start with, in a header of its own: the label is gone and the button is off.
    await expect(page.locator(".story-edit")).toHaveText("Header one");
    await expect(page.locator(".story-edit")).not.toHaveAttribute("data-linked", /.*/);
    await expect(linkButton(page)).toHaveAttribute("aria-pressed", "false");
    await expect(strip(page)).toContainText("Header on page 2");
    const parts = (await setup(page)).sections.map((s) => s.headers.default);
    expect(parts[0]).not.toBe(parts[1]);
    await page.keyboard.press("End");
    await page.keyboard.type(" for chapter two");
    await page.keyboard.press("Escape");
    expect(await hf(page, 0, "hdr")).toBe("Header one");
    expect(await hf(page, 1, "hdr")).toBe("Header one for chapter two");
    // The first section's header is still its own and edits on its own.
    await editHeader(page, 0);
    await expect(page.locator(".story-edit")).toHaveText("Header one");
    await expect(page.locator(".story-edit")).not.toHaveAttribute("data-linked", /.*/);
    await page.keyboard.press("Escape");

    // Link again: the section shows the first's header, and its own text is dropped.
    await editHeader(page, 1);
    await expect(linkButton(page)).toHaveAttribute("aria-pressed", "false");
    await linkButton(page).click();
    await expect(page.locator(".story-edit")).toHaveText("Header one");
    await expect(page.locator(".story-edit")).toHaveAttribute("data-linked", "Same as previous");
    await expect(linkButton(page)).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press("Escape");
    expect(await hf(page, 1, "hdr")).toBe("Header one");
    const again = (await setup(page)).sections.map((s) => s.headers.default);
    expect(again[0]).toBe(again[1]);
    // Undo brings the section's own header back.
    await page.keyboard.press(`${MOD}+Z`);
    expect(await hf(page, 1, "hdr")).toBe("Header one for chapter two");
    expect(await hf(page, 0, "hdr")).toBe("Header one");
  });

  test("the first section has nothing to link to: the button is there and off", async ({ page }) => {
    await editHeader(page, 0);
    await expect(linkButton(page)).toBeDisabled();
    await expect(page.locator(".story-edit")).not.toHaveAttribute("data-linked", /.*/);
  });

  test("a document with one section has no Link to previous button", async ({ page }) => {
    await openBuilt(page, { sections: [portrait(["Only section"], { headers: { default: "Header" } })] });
    await editHeader(page, 0);
    await expect(strip(page).getByRole("button", { name: "Different first page" })).toBeVisible();
    await expect(linkButton(page)).toHaveCount(0);
  });

  test("footers link on their own: unlinking the header leaves the footer shared", async ({ page }) => {
    await openBuilt(page, {
      sections: [portrait(["Section one"], { headers: { default: "Header one" }, footers: { default: "Footer one" } }), portrait(["Section two"], { start: "nextPage" })],
    });
    await editHeader(page, 1);
    await linkButton(page).click();
    await page.keyboard.press("End");
    await page.keyboard.type(" two");
    await page.keyboard.press("Escape");
    expect(await hf(page, 1, "hdr")).toBe("Header one two");
    expect(await hf(page, 1, "ftr")).toBe("Footer one");
    const ps = await setup(page);
    expect((ps.sections[1] as unknown as { linkedHeaders: string[] }).linkedHeaders).not.toContain("default");
    expect((ps.sections[1] as unknown as { linkedFooters: string[] }).linkedFooters).toContain("default");
  });

  test("Insert, Page elements, Link to previous works from the menu while a header is edited, and is off the rest of the time", async ({ page }) => {
    await openMenu(page, "Insert");
    await menuItem(page, "Page elements").click();
    await expect(menuItem(page, "Link to previous")).toBeDisabled();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await editHeader(page, 1);
    await chooseMenu(page, "Insert", "Page elements", "Link to previous");
    await expect(linkButton(page)).toHaveAttribute("aria-pressed", "false");
    // The menu line shows it checked while linked.
    await openMenu(page, "Insert");
    await menuItem(page, "Page elements").click();
    await expect(menuItem(page, "Link to previous")).toHaveAttribute("aria-checked", "false");
    await menuItem(page, "Link to previous").click();
    await expect(linkButton(page)).toHaveAttribute("aria-pressed", "true");
  });

  test("the assistant's header tool can link and unlink", async ({ page }) => {
    const section = await inPage(page, (w) => w.fe.editor.sectionAnchor(1));
    const run = (args: unknown) => inPage(page, (w, a) => (w.fe.editor.engine as unknown as { raw: { callTool(name: string, args: string): string } }).raw.callTool("set_header_footer", JSON.stringify(a)), args);
    await run({ kind: "header", section: `p${section}`, link_to_previous: false });
    let ps = await setup(page);
    expect(ps.sections[1].headers.default).not.toBe(ps.sections[0].headers.default);
    await run({ kind: "header", section: `p${section}`, link_to_previous: true });
    ps = await setup(page);
    expect(ps.sections[1].headers.default).toBe(ps.sections[0].headers.default);
  });
});
