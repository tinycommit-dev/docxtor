// Screen reader support (Tools, Screen reader support, Ctrl+Alt+Z / Cmd+Option+Z as in Google Docs): a long document draws only the pages
// near the view, so the rest is out of the accessibility tree and a screen reader's "read all" stops at the edge. With it on, every page is
// drawn. Here a 40-page document with the test switch on (one page either side of the view), where most blocks are far without it.
import { openBuilt, type SectionSpec } from "./support/docx-builder";
import { chooseMenu, expect, inPage, menuItem, openMenu, test, type Page, CHROMIUM_ONLY, chromiumOnlyFile } from "./helpers";

chromiumOnlyFile(CHROMIUM_ONLY.cdp);

const MOD = "ControlOrMeta";
const PER_PAGE = 20;
const TOTAL = 40 * PER_PAGE;

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    (globalThis as unknown as { __DOCXTOR_FAR_PAGES__: number }).__DOCXTOR_FAR_PAGES__ = 1;
  });
});

function longDoc(): SectionSpec {
  const paragraphs: SectionSpec["paragraphs"] = [];
  for (let i = 1; i <= TOTAL; i++) paragraphs.push(i > 1 && i % PER_PAGE === 1 ? { text: `P ${i}`, ppr: "<w:pageBreakBefore/>" } : `P ${i}`);
  return { paragraphs };
}

/** Blocks with no layout box (out of the accessibility tree). */
const hidden = (page: Page) =>
  inPage(page, () => Array.from(document.querySelectorAll<HTMLElement>(".flow [data-p]")).filter((p) => p.getClientRects().length === 0).length);
/** The texts in Chrome's accessibility tree (what a screen reader reads). */
async function axTexts(page: Page): Promise<Set<string>> {
  const cdp = await page.context().newCDPSession(page);
  const { nodes } = (await cdp.send("Accessibility.getFullAXTree")) as { nodes: { ignored?: boolean; role?: { value?: string }; name?: { value?: string } }[] };
  await cdp.detach();
  return new Set(nodes.filter((n) => !n.ignored && n.role?.value === "StaticText").map((n) => String(n.name?.value ?? "")));
}
const on = (page: Page) => inPage(page, (w) => (w.fe.editor as unknown as { drawEveryPage: boolean }).drawEveryPage);

test("screen reader support draws every page, so all the text is in the accessibility tree, and is kept for the next visit", async ({ page }) => {
  await openBuilt(page, { sections: [longDoc()] });
  await page.locator(".flow [data-p]").first().click();
  await expect.poll(() => hidden(page)).toBeGreaterThan(TOTAL / 2);
  // A far paragraph is not something a screen reader can reach: it is not in the text's accessibility tree.
  await expect(page.getByText(`P ${TOTAL - 5}`, { exact: true })).toBeHidden();
  expect((await axTexts(page)).has(`P ${TOTAL - 5}`)).toBe(false);

  // While it is off, the text tells a screen reader how to turn it on (read when the text gets the keyboard).
  const said = await page.locator(".flow").getAttribute("aria-description");
  expect(said).toContain("screen reader support with");
  expect(said).toMatch(/(Ctrl\+Alt|Command\+Option)\+Z/);
  await page.keyboard.press(`${MOD}+Alt+z`);
  await expect(page.locator(".toast")).toContainText("Screen reader support is on");
  expect(await on(page)).toBe(true);
  await expect.poll(() => hidden(page)).toBe(0);
  await expect(page.locator(".flow")).not.toHaveAttribute("aria-description", /.+/);
  await expect(page.getByText(`P ${TOTAL - 5}`, { exact: true })).toBeVisible();
  const tree = await axTexts(page);
  for (let n = 1; n <= TOTAL; n++) expect(tree.has(`P ${n}`), `P ${n} is in the accessibility tree`).toBe(true);
  // Scrolling and typing leave everything drawn (the paginator's own checks run in every spec).
  await page.locator(".canvas").evaluate((c) => (c.scrollTop = c.scrollHeight / 2));
  await page.keyboard.type("abc");
  await expect.poll(() => hidden(page)).toBe(0);
  // The Tools menu shows it on.
  await openMenu(page, "Tools");
  await expect(menuItem(page, "Screen reader support")).toHaveAttribute("aria-checked", "true");
  await page.keyboard.press("Escape");

  // The next visit keeps it.
  await openBuilt(page, { sections: [longDoc()] });
  expect(await on(page)).toBe(true);
  await expect.poll(() => hidden(page)).toBe(0);

  // Off again from the menu: far pages are left out again.
  await chooseMenu(page, "Tools", "Screen reader support");
  expect(await on(page)).toBe(false);
  await page.locator(".flow [data-p]").first().click();
  await expect.poll(() => hidden(page)).toBeGreaterThan(TOTAL / 2);
});
