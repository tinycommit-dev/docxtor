// The production build as it ships (web/dist, the engine's release build) served the way Cloudflare Pages serves it, with the headers of
// deploy/_headers: the Content-Security-Policy must let everything Docxtor does through, the service worker must install, and a real file must
// open, take an edit and save. Not part of the normal run: `npm run build`, then `DOCXTOR_DIST=1 npx playwright test e2e/production.spec.ts`.
// DOCXTOR_DIST_URL=<address> runs the same test against a server that is already running (the Docker image, a Cloudflare preview address)
// instead of scripts/serve-dist.mjs: `DOCXTOR_DIST_URL=http://localhost:8080/ npx playwright test e2e/production.spec.ts`.
// DOCXTOR_DOCS=1 adds the documentation site at /docs/ (the hosted build has it: `npm run docs:add`; the Docker image does not).
// The shared fixture fails a test on any console error, and a resource the policy blocks is one.
import { spawn, type ChildProcess } from "node:child_process";
import { readFileSync } from "node:fs";
import { expect, fakeAnthropic, inPage, test } from "./helpers";

const REMOTE = process.env.DOCXTOR_DIST_URL;
const ON = !!process.env.DOCXTOR_DIST || !!REMOTE;
const DOCS = !!process.env.DOCXTOR_DOCS;
const PORT = 5191;
const URL0 = REMOTE ? (REMOTE.endsWith("/") ? REMOTE : `${REMOTE}/`) : `http://localhost:${PORT}/`;
let server: ChildProcess | null = null;

test.describe("the production build", () => {
  test.skip(!ON, "needs a production build: npm run build, then DOCXTOR_DIST=1 (or DOCXTOR_DIST_URL=<address of a running copy>)");
  test.beforeAll(async () => {
    if (REMOTE) {
      const ok = await fetch(URL0).then((r) => r.ok, () => false);
      if (!ok) throw new Error(`nothing answers at ${URL0}`);
      return;
    }
    server = spawn("node", ["scripts/serve-dist.mjs"], { env: { ...process.env, PORT: String(PORT) }, stdio: "ignore" });
    for (let i = 0; i < 50; i++) {
      const ok = await fetch(URL0).then((r) => r.ok, () => false);
      if (ok) return;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("the production server did not start");
  });
  test.afterAll(() => void server?.kill());

  test("opens with the shipping headers, opens a real file, takes an edit, saves and works offline", async ({ page }) => {
    test.setTimeout(120_000);
    // The engine's file: the right type, and kept for a year.
    const wasm: { type: string; cache: string }[] = [];
    page.on("response", (r) => {
      if (/docxtor_bg-.*\.wasm$/.test(new URL(r.url()).pathname)) wasm.push({ type: r.headers()["content-type"] ?? "", cache: r.headers()["cache-control"] ?? "" });
    });
    const res = await page.goto(URL0);
    expect(res?.headers()["content-security-policy"]).toContain("script-src 'self' 'wasm-unsafe-eval'");
    expect(res?.headers()["cache-control"]).toBe("no-cache");
    expect(res?.headers()["x-content-type-options"]).toBe("nosniff");
    expect(res?.headers()["x-frame-options"]).toBe("SAMEORIGIN");
    await page.waitForFunction(() => !!(window as any).docxtor);
    expect(wasm.length).toBeGreaterThan(0);
    expect(wasm[0].type).toContain("application/wasm");
    expect(wasm[0].cache).toBe("public, max-age=31536000, immutable");
    // A document in the Library, kept in this browser: it leaves the start screen.
    await inPage(page, (w: any) => w.createIn(w.files.library, "", "Production.docx", "A document made on the live site."));
    await expect(page.locator(".flow")).toContainText("A document made on the live site.");
    // A real file, opened in its place.
    const bytes = Array.from(readFileSync("fixtures/rich.docx"));
    await inPage(page, async (w: any, b: number[]) => void (await w.fe.open(new Uint8Array(b), "rich.docx")), bytes);
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press("Home");
    await page.keyboard.type("Edited in production. ");
    await expect(page.locator(".flow .fp").first()).toContainText("Edited in production.");
    // Saved: the bytes the engine writes open again with the edit.
    const again = await inPage(page, async (w: any) => {
      const ed = w.fe.editor;
      const Engine = ed.engine.constructor;
      const e = Engine.open(ed.engine.save(), "again.docx");
      const text = e.text();
      e.dispose();
      return text;
    });
    expect(again).toContain("Edited in production.");
    // The assistant loads the Anthropic SDK on its first question (a file of its own the policy must let in) and shows the answer.
    const seen = await fakeAnthropic(page, "https://api.anthropic.com", "Answered in production.");
    await inPage(page, async (w: any) => {
      w.ai.toggle(true);
      w.ai.settings = { ...w.ai.settings, provider: "anthropic", model: "claude-opus-5-5", apiKey: "test-key", baseUrl: "" };
      w.ai.input.value = "Say hello";
      await w.ai.send();
    });
    await expect(page.locator(".entry.bot")).toContainText("Answered in production.");
    expect(seen[0]?.headers["x-api-key"]).toBe("test-key");
    // The service worker installs and the page opens with the network off.
    await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
    await page.reload();
    await page.waitForFunction(() => !!navigator.serviceWorker.controller, null, { timeout: 15000 });
    await page.context().setOffline(true);
    await page.reload();
    await page.waitForFunction(() => !!(window as any).docxtor, null, { timeout: 15000 });
    await page.context().setOffline(false);
  });
  test("the docs at /docs/ have their own policy and caching, and the app's service worker leaves them alone", async ({ page }) => {
    test.skip(!DOCS, "the docs are in the hosted build only: DOCXTOR_DOCS=1 (npm run docs:add)");
    test.setTimeout(120_000);
    // The app first: its service worker is in control of the address.
    await page.goto(URL0);
    await page.waitForFunction(() => !!(window as any).docxtor);
    // Its Documentation link goes to these docs, beside it, not to another copy's.
    await expect(page.locator(".start-links a", { hasText: "Documentation" })).toHaveAttribute("href", new URL("docs/", URL0).href);
    await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
    await page.reload();
    await page.waitForFunction(() => !!navigator.serviceWorker.controller, null, { timeout: 15000 });
    const res = await page.goto(`${URL0}docs/`);
    expect(res?.status()).toBe(200);
    expect(res?.fromServiceWorker(), "the service worker does not answer for /docs/").toBe(false);
    const h = res?.headers() ?? {};
    // One policy, the docs': the hashes of the two inline scripts, no engine, no outside address.
    expect(h["content-security-policy"]).toMatch(/^default-src 'self'; script-src 'self' 'sha256-[^']+' 'sha256-[^']+'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; connect-src 'self'/);
    expect(h["content-security-policy"]).not.toContain("wasm-unsafe-eval");
    expect(h["cache-control"]).toBe("no-cache");
    expect(h["x-content-type-options"]).toBe("nosniff");
    expect(h["x-frame-options"]).toBe("SAMEORIGIN");
    await expect(page.getByRole("heading", { level: 1 })).toContainText("Docxtor documentation");
    await expect(page.locator(".VPSwitchAppearance")).toBeVisible();
    // A built file of the docs: a year, immutable, and the app's own rule for /assets/ is not the one that answers.
    const css = await page.evaluate(() => (document.querySelector('link[rel~="stylesheet"][href*="/docs/assets/"]') as HTMLLinkElement | null)?.href ?? "");
    expect(css).toContain("/docs/assets/");
    const asset = await fetch(css);
    expect(asset.headers.get("cache-control")).toBe("public, max-age=31536000, immutable");
    expect(asset.headers.get("content-security-policy")).toContain("script-src 'self' 'sha256-");
    // And the app is still the app: its page keeps its own policy (the engine's allowance) and its cache rule.
    const app = await fetch(URL0);
    expect(app.headers.get("content-security-policy")).toContain("script-src 'self' 'wasm-unsafe-eval'");
    expect(app.headers.get("content-security-policy")).not.toContain("sha256-");
    expect(app.headers.get("cache-control")).toBe("no-cache");
  });
});
