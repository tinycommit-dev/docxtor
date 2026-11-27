// Light and dark: one button on the start screen and in the top bar switches between them, follows the system until it is used, and
// remembers the choice. View, Theme still offers every theme, Match the system among them.
import { expect, openFixture, test, type Page } from "./helpers";

const root = (page: Page) => page.locator(".docxtor").first();

test.describe("light and dark", () => {
  test("the start screen's button switches to dark and back, shows the other one's icon, and the choice survives a reload", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto("/");
    const button = page.locator(".start-tools").getByRole("button", { name: "Switch to dark" });
    await expect(button).toBeVisible();
    await button.click();
    await expect(root(page)).toHaveAttribute("data-theme", "dark");
    const back = page.locator(".start-tools").getByRole("button", { name: "Switch to light" });
    await expect(back).toBeVisible();
    await page.reload();
    await expect(root(page)).toHaveAttribute("data-theme", "dark");
    await page.locator(".start-tools").getByRole("button", { name: "Switch to light" }).click();
    await expect(root(page)).toHaveAttribute("data-theme", "light");
  });

  test("it starts from what the system shows: on a dark system the first click makes it light", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await page.goto("/");
    await page.locator(".start-tools").getByRole("button", { name: "Switch to light" }).click();
    await expect(root(page)).toHaveAttribute("data-theme", "light");
  });

  test("the top bar has the same button while a document is open, and it follows a theme chosen in View, Theme", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openFixture(page, "simple.docx");
    const top = page.locator(".topbar .top-actions");
    await top.getByRole("button", { name: "Switch to dark" }).click();
    await expect(root(page)).toHaveAttribute("data-theme", "dark");
    await expect(top.getByRole("button", { name: "Switch to light" })).toBeVisible();
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { setTheme(id: string): void } } }).docxtor.fe.setTheme("light"));
    await expect(top.getByRole("button", { name: "Switch to dark" })).toBeVisible();
  });
});
