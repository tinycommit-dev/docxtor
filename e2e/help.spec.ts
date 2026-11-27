// The Help menu: written help pages that work offline, what's new, and links to report a problem and to the source code.
import { chooseMenu, dialog, expect, openFixture, openMenu, menuItem, test, type Page } from "./helpers";

// The repository's and the documentation's addresses: the public ones unless the build names others (VITE_DOCXTOR_REPO, VITE_DOCXTOR_DOCS).
const REPO = (process.env.VITE_DOCXTOR_REPO ?? "https://github.com/tinycommit-dev/docxtor").replace(/\/+$/, "");
const DOCS = process.env.VITE_DOCXTOR_DOCS ?? "https://docxtor.tinycommit.dev/docs/";
// Whether this copy has its own documentation at ./docs/: its hashmap.json answers as JSON. The dev server answers every address with
// the app's page, which is no documentation.
const ownDocs = (page: Page, has: boolean) =>
  page.route("**/docs/hashmap.json", (route) => route.fulfill(has ? { contentType: "application/json", body: "{}" } : { contentType: "text/html", body: "<!doctype html>" }));

test.describe("help", () => {
  test.beforeEach(async ({ page, context }) => {
    // The links open the project's pages: the tests don't go to the network for them.
    await context.route("https://github.com/**", (route) => route.fulfill({ contentType: "text/html", body: "<title>Docxtor</title>" }));
    await context.route("https://docxtor.tinycommit.dev/**", (route) => route.fulfill({ contentType: "text/html", body: "<title>Docxtor docs</title>" }));
    await ownDocs(page, false);
    await openFixture(page, "simple.docx");
  });

  test("the Help menu lists the help pages and what's new, and the links when their addresses are set", async ({ page }) => {
    await openMenu(page, "Help");
    for (const label of ["Search the menus", "Keyboard shortcuts", "Help", "What's new", "Open the tour in a new tab"]) await expect(menuItem(page, label)).toBeVisible();
    for (const label of ["Report a problem", "Source code"]) await expect(menuItem(page, label)).toHaveCount(REPO ? 1 : 0);
    await expect(menuItem(page, "Documentation")).toHaveCount(DOCS ? 1 : 0);
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

  test("Documentation opens the documentation", async ({ page, context }) => {
    test.skip(!DOCS, "no documentation address in this build (VITE_DOCXTOR_DOCS)");
    const [tab] = await Promise.all([context.waitForEvent("page"), chooseMenu(page, "Help", "Documentation")]);
    await tab.waitForURL((u) => u.toString().startsWith(DOCS));
    await tab.close();
  });

  test("the start screen links to the documentation and the source code at the top", async ({ page }) => {
    await page.goto("/");
    const links = page.getByRole("navigation", { name: "About Docxtor" });
    if (DOCS) await expect(links.getByRole("link", { name: "Documentation" })).toHaveAttribute("href", DOCS);
    if (REPO) await expect(links.getByRole("link", { name: "Source code on GitHub" })).toHaveAttribute("href", REPO);
    await expect(links.getByRole("link")).toHaveCount((DOCS ? 1 : 0) + (REPO ? 1 : 0));
    // At the top, beside the wordmark and above the heading: a long list of documents never pushes them out of sight.
    const nav = (await links.boundingBox())!;
    const heading = (await page.getByRole("heading", { name: "Your documents" }).boundingBox())!;
    expect(nav.y + nav.height).toBeLessThan(heading.y);
  });

  test("a copy with its own documentation links to it, one without links to the public documentation", async ({ page, context }) => {
    test.skip(process.env.VITE_DOCXTOR_DOCS !== undefined, "this build names its documentation's address");
    await page.unroute("**/docs/hashmap.json");
    await ownDocs(page, true);
    await page.goto("/");
    const own = new URL("/docs/", page.url()).href;
    await expect(page.getByRole("navigation", { name: "About Docxtor" }).getByRole("link", { name: "Documentation" })).toHaveAttribute("href", own);
    await page.route(own, (route) => route.fulfill({ contentType: "text/html", body: "<title>Own docs</title>" }));
    await openFixture(page, "simple.docx");
    const [tab] = await Promise.all([context.waitForEvent("page"), chooseMenu(page, "Help", "Documentation")]);
    await tab.waitForURL(own);
    await tab.close();
  });

  test("Source code opens the repository", async ({ page, context }) => {
    test.skip(!REPO, "no repository address in this build (VITE_DOCXTOR_REPO)");
    const [tab] = await Promise.all([context.waitForEvent("page"), chooseMenu(page, "Help", "Source code")]);
    await tab.waitForURL((u) => u.toString().startsWith(REPO));
    await tab.close();
  });
});
