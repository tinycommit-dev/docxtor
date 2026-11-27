import { expect, openFixture, test } from "./helpers";

test.describe("open and render", () => {
  test("rich.docx opens with its title, two pages and its scores", { tag: "@core" }, async ({ page }) => {
    await openFixture(page, "rich.docx");

    // The document has its own address, and its name is in the top bar and the tab.
    await expect(page).toHaveURL(/#\/library\/Fixtures\/rich\.docx$/);
    await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue("rich");
    await expect(page).toHaveTitle("rich — Docxtor");

    // Two sheets, and the title paragraph is the first thing on the first one.
    await expect(page.locator(".sheet")).toHaveCount(2);
    await expect(page.locator(".flow .fp").first()).toHaveText("Quarterly Product Report");

    const status = page.locator(".statusbar");
    await expect(status).toContainText("Page 1 of 2");
    await expect(status).toContainText("98 words");
    await expect(status).toContainText("100.0% round-trip fidelity");
    await expect(page.locator(".led")).toHaveText("Saved in your Library");
  });

  test("shows the document's parts: headings, a table, an image, a header and a footer", { tag: "@core" }, async ({ page }) => {
    await openFixture(page, "rich.docx");
    await expect(page.locator('.flow [data-style="Heading1"]').first()).toHaveText("Highlights");
    await expect(page.locator(".flow table.ft")).toHaveCount(1);
    await expect(page.locator(".flow table.ft tr")).toHaveCount(4);
    await expect(page.locator(".flow .atom.img")).toHaveCount(1);
    await expect(page.locator(".sheet .hdr").first()).toContainText("Docxtor — confidential draft");
    await expect(page.locator(".sheet .ftr").first()).toContainText("Page 1");
    await expect(page.locator(".sheet .ftr").nth(1)).toContainText("Page 2");
  });

  test("the other fixtures open too", { tag: "@core" }, async ({ page }) => {
    for (const name of ["simple.docx", "complex.docx", "long.docx"]) {
      await openFixture(page, name);
      await expect(page.getByRole("textbox", { name: "Document name" })).toHaveValue(name.replace(".docx", ""));
      await expect(page.locator(".statusbar")).toContainText("round-trip fidelity");
    }
  });
});
