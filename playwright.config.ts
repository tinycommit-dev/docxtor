import { defineConfig, devices } from "@playwright/test";
import { resolve } from "node:path";

// DOCXTOR_E2E_PORT lets several checkouts (git worktrees) run their tests at the same time.
const PORT = Number(process.env.DOCXTOR_E2E_PORT ?? 5180);
// Playwright runs from the repo root.
const fixtures = resolve(process.cwd(), "fixtures");

// Chromium always; Firefox and WebKit (Safari's engine) only when asked: DOCXTOR_BROWSERS=firefox,webkit, or
// `--project=firefox` (an explicit --project adds it too). The normal `npm run test:e2e` stays Chromium only.
// The clipboard permissions exist in Chromium alone (Firefox and WebKit throw at "Unknown permission"), so only
// the Chromium project grants them; tests that need the clipboard API skip elsewhere (CHROMIUM_ONLY in e2e/helpers.ts).
const asked = new Set((process.env.DOCXTOR_BROWSERS ?? "").split(",").map((b) => b.trim()).filter(Boolean));
const argProjects = process.argv.filter((a) => a.startsWith("--project=")).map((a) => a.slice("--project=".length));
for (const a of argProjects) asked.add(a);
// Test workers read this file again with other arguments: pass the choice on through the environment.
if (asked.size) process.env.DOCXTOR_BROWSERS = [...asked].join(",");
const projects = [
  {
    name: "chromium",
    use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 900 }, permissions: ["clipboard-read", "clipboard-write"] },
  },
  ...(asked.has("firefox") ? [{ name: "firefox", use: { ...devices["Desktop Firefox"], viewport: { width: 1280, height: 900 }, ...(process.env.DOCXTOR_FIREFOX_PATH ? { launchOptions: { executablePath: process.env.DOCXTOR_FIREFOX_PATH } } : {}) } }] : []),
  ...(asked.has("webkit") ? [{ name: "webkit", use: { ...devices["Desktop Safari"], viewport: { width: 1280, height: 900 } } }] : []),
];

// DOCXTOR_DIST_URL runs e2e/production.spec.ts against a copy that is already running (the Docker image, a Cloudflare preview address):
// nothing of ours is started or built, so no dev server and no warm-up.
const remote = !!process.env.DOCXTOR_DIST_URL;

// Browser tests for the Docxtor app. They start their own Vite dev server on a port of their own
// (never the 5173 you develop on); the WASM engine must already be built (`npm run wasm`).
export default defineConfig({
  testDir: "./e2e",
  // Load the app once on the fresh server before any test (see e2e/support/warmup.ts).
  globalSetup: remote ? undefined : "./e2e/support/warmup.ts",
  outputDir: "./test-results",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [["list"], ["html", { open: "never" }]],
  timeout: 30_000,
  expect: { timeout: 7_000 },
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "on-first-retry",
    viewport: { width: 1280, height: 900 },
  },
  projects,
  webServer: remote ? undefined : {
    command: `npm run dev -w docxtor-web -- --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    // ?open=NAME reads fixtures through Vite's /@fs; the app's own .env.development.local is not in a fresh checkout.
    // DOCXTOR_BUILT_DIR: where e2e/offline.spec.ts puts a production build, which the dev server then serves under /__built/.
    env: { VITE_FIXTURES: fixtures, DOCXTOR_BUILT_DIR: resolve(process.cwd(), "web/dist-test") },
  },
});
