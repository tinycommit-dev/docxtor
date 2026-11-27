// Runs once before the browser tests. A fresh Vite server compiles the app and its dependencies on the
// first page load, and may reload the page when it finds more to prepare. Doing that here, before any
// test, keeps the first tests from timing out on a cold server.
import { chromium, firefox, webkit, type FullConfig } from "@playwright/test";

export default async function warmup(config: FullConfig) {
  const baseURL = config.projects[0]?.use.baseURL;
  if (!baseURL) return;
  // The first browser asked for (DOCXTOR_BROWSERS, set from --project in playwright.config.ts), so a run on Firefox
  // or WebKit alone does not need Chromium installed.
  const first = (process.env.DOCXTOR_BROWSERS ?? "").split(",")[0];
  const browser = await (first === "firefox" ? firefox : first === "webkit" ? webkit : chromium).launch();
  try {
    const page = await browser.newPage();
    for (let i = 0; i < 2; i++) {
      await page.goto(`${baseURL}/?open=rich.docx`);
      await page.waitForFunction(() => !!(window as unknown as { docxtor?: { engine?: unknown } }).docxtor?.engine, undefined, { timeout: 120_000 });
      await page.locator(".flow .fp").first().waitFor({ timeout: 120_000 });
    }
    // Open each menu once, so lazily loaded parts (dialogs, the picker's data) are compiled too.
    for (const name of ["File", "Edit", "View", "Insert", "Format", "Tools"]) {
      const menu = page.getByRole("menuitem", { name, exact: true });
      if (await menu.isVisible()) {
        await menu.click();
        await page.keyboard.press("Escape");
      }
    }
  } finally {
    await browser.close();
  }
}
