// Extensions, Manage extensions: every extension with a switch, and a search over their names, descriptions and what
// they add, which ignores case and accents.
import { chooseMenu, dialog, expect, openFixture, test, type Page } from "./helpers";

const ext = (page: Page) => dialog(page, "Extensions");
const rows = (page: Page) => ext(page).locator(".ext-row");
const search = (page: Page) => ext(page).getByRole("searchbox", { name: "Search extensions" });
const note = (page: Page) => ext(page).locator(".ext-note");

test("the search narrows the list and says how many match, and nothing matching says so", async ({ page }) => {
  await openFixture(page, "rich.docx");
  await chooseMenu(page, "Extensions", "Manage extensions…");
  // The search box has the focus, and the count covers every extension.
  await expect(search(page)).toBeFocused();
  const all = await rows(page).count();
  expect(all).toBeGreaterThan(10);
  await expect(note(page)).toHaveText(`${all} extensions`);

  // By name, in any case; the dialog keeps its height while the list gets shorter.
  const height = () => ext(page).evaluate((d) => Math.round(d.getBoundingClientRect().height));
  const before = await height();
  await search(page).fill("languagetool");
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first()).toContainText("LanguageTool");
  await expect(note(page)).toHaveText("1 extension");
  expect(await height()).toBe(before);

  // Every word counts, wherever it is (here, in what an extension adds).
  await search(page).fill("export format");
  const some = await rows(page).count();
  expect(some).toBeGreaterThan(0);
  expect(some).toBeLessThan(all);
  for (const r of await rows(page).all()) await expect(r).toContainText(/export format/i);

  await search(page).fill("zzzz nothing like this");
  await expect(rows(page)).toHaveCount(0);
  await expect(note(page)).toHaveText("No extension matches.");

  // Cleared, the whole list is back; the search box stays put above it while the list scrolls.
  await search(page).fill("");
  await expect(rows(page)).toHaveCount(all);
  const body = ext(page).locator(".dialog-body");
  await body.evaluate((b) => b.scrollTo(0, b.scrollHeight));
  await expect(search(page)).toBeInViewport();
});
