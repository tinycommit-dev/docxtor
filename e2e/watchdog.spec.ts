// The watchdog in helpers.ts: every test fails on an uncaught error, a console.error or a frozen
// page. These specs prove it, by making each happen on purpose (test.fail() = the fixture must fail).
import { expect, expectErrors, test } from "./helpers";

test.describe("watchdog", () => {
  test("an uncaught error in the page fails the test", async ({ page }) => {
    test.fail();
    await page.goto("/");
    await page.evaluate(() => { setTimeout(() => { throw new Error("boom on purpose"); }, 0); });
    await page.waitForTimeout(200);
  });

  test("a console.error fails the test", async ({ page }) => {
    test.fail();
    await page.goto("/");
    await page.evaluate(() => console.error("logged on purpose"));
  });

  test("a frozen page fails the test", async ({ page }) => {
    test.fail();
    await page.goto("/");
    await page.evaluate(() => { setTimeout(() => { for (;;) { /* busy */ } }, 0); });
    await page.waitForTimeout(300);
  });

  test("expectErrors lets an expected error pass", async ({ page }) => {
    await page.goto("/");
    expectErrors(page, /boom expected/, /logged expected/);
    await page.evaluate(() => { setTimeout(() => { throw new Error("boom expected"); }, 0); console.error("logged expected"); });
    await page.waitForTimeout(200);
  });

  test("expectErrors fails when the expected error never happens", async ({ page }) => {
    test.fail();
    await page.goto("/");
    expectErrors(page, /never happens/);
  });

  test("a request the test aborts itself is not an error", async ({ page }) => {
    await page.route("**/missing-on-purpose.js", (route) => route.abort());
    await page.goto("/");
    await page.evaluate(() => fetch("/missing-on-purpose.js").catch(() => null));
    expect(true).toBe(true);
  });
});
