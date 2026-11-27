// The Help menu: written help pages that work offline, what's new, and links to report a problem and to the source code.
import { chooseMenu, dialog, expect, openFixture, openMenu, menuItem, test } from "./helpers";

// The repository's address comes from the build (VITE_DOCXTOR_REPO); the shipped app has none until it is public, and no links.
const REPO = (process.env.VITE_DOCXTOR_REPO ?? "").replace(/\/+$/, "");

test.describe("help", () => {
  test.beforeEach(async ({ page, context }) => {
    // The links open the project's pages: the tests don't go to the network for them.
    await context.route("https://github.com/**", (route) => route.fulfill({ contentType: "text/html", body: "<title>Docxtor</title>" }));
    await openFixture(page, "simple.docx");
  });

  test("the Help menu lists the help pages and what's new, and the two links only when the repository's address is set", async ({ page }) => {
    await openMenu(page, "Help");
    for (const label of ["Search the menus", "Keyboard shortcuts", "Help", "What's new", "Open the tour in a new tab"]) await expect(menuItem(page, label)).toBeVisible();
    for (const label of ["Report a problem", "Source code"]) await expect(menuItem(page, label)).toHaveCount(REPO ? 1 : 0);
  });

  test("Help opens the pages, one topic at a time, and F1 opens it too", async ({ page }) => {
    await chooseMenu(page, "Help", "Help");
    const d = dialog(page, "Help");
    await expect(d).toBeVisible();
    await expect(d.locator(".dialog-head .sub")).toHaveText(/^Docxtor \d+\.\d+\.\d+$/);
    await expect(d.getByRole("tab", { name: "Getting started" })).toHaveAttribute("aria-selected", "true");
    await expect(d.locator(".help-page h3")).toHaveText("Getting started");
    for (const topic of ["Your documents", "Templates", "Working with Word", "The assistant", "Working offline", "Privacy"]) {
      await d.getByRole("tab", { name: topic }).click();
      await expect(d.locator(".help-page h3")).toHaveText(topic);
      await expect(d.locator(".help-page p").first()).not.toBeEmpty();
    }
    await d.getByRole("button", { name: "Done" }).click();
    await expect(d).toBeHidden();
    await page.keyboard.press("F1");
    await expect(dialog(page, "Help")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog(page, "Help")).toBeHidden();
  });

  test("What's new lists the changes", async ({ page }) => {
    await chooseMenu(page, "Help", "What's new");
    const d = dialog(page, "What's new");
    await expect(d.getByRole("heading", { name: "Properties, templates, offline and more" })).toBeVisible();
    await expect(d.getByRole("listitem")).not.toHaveCount(0);
    await expect(d.getByText("The Undo button has a list of what you did")).toBeVisible();
  });

  test("Report a problem opens a new issue in the project's repository with the version and browser, and nothing from the document", async ({ page, context }) => {
    test.skip(!REPO, "no repository address in this build (VITE_DOCXTOR_REPO)");
    const [tab] = await Promise.all([context.waitForEvent("page"), chooseMenu(page, "Help", "Report a problem")]);
    await tab.waitForURL(/github\.com/);
    const url = new URL(tab.url());
    expect(url.origin + url.pathname).toBe(`${REPO}/issues/new`);
    const body = url.searchParams.get("body") ?? "";
    expect(body).toContain("What happened:");
    expect(body).toMatch(/Docxtor \d+\.\d+\.\d+, .*Chrome/);
    expect(body).not.toContain("simple");
    await tab.close();
  });

  test("Source code opens the repository", async ({ page, context }) => {
    test.skip(!REPO, "no repository address in this build (VITE_DOCXTOR_REPO)");
    const [tab] = await Promise.all([context.waitForEvent("page"), chooseMenu(page, "Help", "Source code")]);
    await tab.waitForURL((u) => u.toString().startsWith(REPO));
    await tab.close();
  });
});
