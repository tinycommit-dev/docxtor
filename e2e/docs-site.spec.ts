// The documentation site (docs/ built by VitePress, served at /docs/ on the same address as the app).
// Part 1 builds the site itself and serves it with the headers of deploy/_headers: every page of every section must load with no console
// error and no Content-Security-Policy violation, the theme switch and the search must work, and nothing may be asked of another address.
// Part 2 needs the production build with the docs in it (`npm run build`, `npm run docs:add`, then `DOCXTOR_DIST=1`): the app at / must
// still work offline, and its service worker must neither answer for /docs/ nor keep anything of it.
import { spawn, type ChildProcess } from "node:child_process";
import { expect, test as base, type Page } from "@playwright/test";
import { startDocsServer, type DocsServer } from "./support/docs-server";

// One page of each section, and the two that lead into them.
const PAGES = [
  { path: "/docs/", heading: "Docxtor documentation" },
  { path: "/docs/use/", heading: "Use Docxtor" },
  { path: "/docs/use/writing", heading: "Write and format" },
  { path: "/docs/extensions/document-look", heading: "Extensions for design and content" },
  { path: "/docs/embedding", heading: "Embedding Docxtor" },
  { path: "/docs/deploy", heading: "Putting Docxtor online" },
  { path: "/docs/contributing", heading: "Contributing to Docxtor" },
  { path: "/docs/roadmap", heading: "Roadmap" },
];

/** What a page reports: console errors and warnings, uncaught errors, the browser's policy violations, and every address it asked for. */
function watch(page: Page) {
  const problems: string[] = [];
  const requests: string[] = [];
  page.on("console", (m) => (m.type() === "error" || m.type() === "warning") && problems.push(`console.${m.type()}: ${m.text()}`));
  page.on("pageerror", (e) => problems.push(`uncaught: ${e.message}`));
  page.on("request", (r) => requests.push(r.url()));
  void page.addInitScript(() => {
    document.addEventListener("securitypolicyviolation", (e) => {
      (window as unknown as { __csp: string[] }).__csp = [...((window as unknown as { __csp?: string[] }).__csp ?? []), `${e.violatedDirective} ${e.blockedURI}`];
    });
  });
  return { problems, requests };
}
const violations = (page: Page) => page.evaluate(() => (window as unknown as { __csp?: string[] }).__csp ?? []);

const test = base;

test.describe("the documentation site, served with the production headers", () => {
  test.describe.configure({ mode: "serial" });
  let server: DocsServer;
  test.beforeAll(async () => {
    test.setTimeout(180_000);
    server = await startDocsServer("site");
  });
  test.afterAll(() => server?.stop());

  for (const p of PAGES) {
    test(`${p.path} loads with no error, no policy violation and nothing from another address`, async ({ page }) => {
      const seen = watch(page);
      const res = await page.goto(`${server.url}${p.path}`);
      expect(res?.status()).toBe(200);
      // The docs' own policy: the hashes of its two inline scripts, no engine, no outside host; the cache says look again.
      const csp = res?.headers()["content-security-policy"] ?? "";
      expect(csp).toContain("script-src 'self' 'sha256-");
      expect(csp).not.toContain("wasm-unsafe-eval");
      expect(csp).not.toMatch(/https?:\/\/(?!localhost)/);
      expect(csp, "one policy, not the app's joined to the docs'").not.toContain(", default-src");
      expect(res?.headers()["cache-control"]).toBe("no-cache");
      await expect(page.getByRole("heading", { level: 1 })).toContainText(p.heading);
      // The page came alive: the theme switch answers (hydration ran, so the scripts the policy allows did run).
      await expect(page.locator(".VPSwitchAppearance")).toBeVisible();
      await page.waitForLoadState("networkidle");
      expect(await violations(page)).toEqual([]);
      expect(seen.problems).toEqual([]);
      const outside = seen.requests.filter((u) => !u.startsWith(server.url) && !u.startsWith("data:"));
      expect(outside, "every request goes to the site itself").toEqual([]);
    });
  }

  test("a built file is kept for a year and a page is looked at again", async ({ request }) => {
    const html = await (await request.get(`${server.url}/docs/embedding`)).text();
    const css = /href="(\/docs\/assets\/[^"]+\.css)"/.exec(html)?.[1];
    expect(css, "the page names a style file under /docs/assets/").toBeTruthy();
    const res = await request.get(`${server.url}${css}`);
    expect(res.headers()["cache-control"]).toBe("public, max-age=31536000, immutable");
    expect(res.headers()["content-security-policy"]).toContain("script-src 'self' 'sha256-");
    // The font is a file of the site's own, under the same rule.
    const font = /\/docs\/assets\/[^"')\s]+\.woff2/.exec(await res.text())?.[0];
    expect(font, "the style names Inter's file").toBeTruthy();
    const f = await request.get(`${server.url}${font}`);
    expect(f.status()).toBe(200);
    expect(f.headers()["cache-control"]).toBe("public, max-age=31536000, immutable");
  });

  test("the theme switch changes the theme and the choice is there again after a reload, before any script of the framework", async ({ page }) => {
    const seen = watch(page);
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto(`${server.url}/docs/embedding`);
    await expect(page.locator("html")).not.toHaveClass(/dark/);
    const light = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    await page.locator(".VPSwitchAppearance").first().click();
    await expect(page.locator("html")).toHaveClass(/dark/);
    const dark = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(dark).not.toBe(light);
    // The inline script of the page (allowed by its hash) puts the theme back before the page is drawn.
    await page.reload();
    await expect(page.locator("html")).toHaveClass(/dark/);
    await page.locator(".VPSwitchAppearance").first().click();
    await expect(page.locator("html")).not.toHaveClass(/dark/);
    expect(await violations(page)).toEqual([]);
    expect(seen.problems).toEqual([]);
  });

  test("a system that prefers the dark theme gets it with no choice made", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark" });
    await page.goto(`${server.url}/docs/`);
    await expect(page.locator("html")).toHaveClass(/dark/);
    expect(await violations(page)).toEqual([]);
  });

  test("the search finds a word of the embedding page and opens its page", async ({ page }) => {
    const seen = watch(page);
    await page.goto(`${server.url}/docs/`);
    await page.getByRole("button", { name: "Search" }).click();
    const box = page.getByRole("searchbox");
    await expect(box).toBeVisible();
    // A heading that only the embedding page has.
    await box.fill("Living alongside your page");
    const hit = page.locator(".VPLocalSearchBox a").filter({ hasText: "Living alongside your page" }).first();
    await expect(hit).toBeVisible();
    await expect(hit).toHaveAttribute("href", /\/docs\/embedding#/);
    await box.press("Enter");
    await expect(page).toHaveURL(/\/docs\/embedding#/);
    await expect(page.getByRole("heading", { name: "Living alongside your page" })).toBeVisible();
    expect(await violations(page)).toEqual([]);
    expect(seen.problems).toEqual([]);
    expect(seen.requests.filter((u) => !u.startsWith(server.url) && !u.startsWith("data:"))).toEqual([]);
  });

  test("every link between the pages leads to a page that exists, and the links out are full addresses", async ({ request }) => {
    const seenPages = new Set<string>();
    const queue = ["/docs/"];
    const outside = new Set<string>();
    while (queue.length) {
      const path = queue.pop()!;
      if (seenPages.has(path)) continue;
      seenPages.add(path);
      const res = await request.get(`${server.url}${path}`);
      expect(res.status(), path).toBe(200);
      for (const m of (await res.text()).matchAll(/<a [^>]*href="([^"#]*)(?:#[^"]*)?"/g)) {
        const href = m[1];
        if (!href) continue;
        if (href.startsWith("/docs")) queue.push(href);
        else if (/^https?:\/\//.test(href)) outside.add(new URL(href).host);
        else throw new Error(`${path} has a link that is neither a docs page nor a full address: ${href}`);
      }
    }
    // The 14 pages (the landing page, and 13 with the sidebar's own) all reached from the landing page and the sidebar.
    expect(seenPages.size).toBeGreaterThanOrEqual(14);
    expect(outside.has("github.com")).toBe(true);
  });
});

// -- the app and the docs on one address ----------------------------------------------------------------------------
const DIST = !!process.env.DOCXTOR_DIST;
const PORT = 5192;

test.describe("the app and the docs on one address", () => {
  test.skip(!DIST, "needs the production build with the docs in it: npm run build, npm run docs:add, then DOCXTOR_DIST=1");
  let child: ChildProcess;
  test.beforeAll(async () => {
    child = spawn(process.execPath, ["scripts/serve-dist.mjs"], { env: { ...process.env, PORT: String(PORT) }, stdio: "ignore" });
    for (let i = 0; i < 50; i++) {
      if (await fetch(`http://localhost:${PORT}/docs/`).then((r) => r.ok, () => false)) return;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("the production server did not start, or web/dist has no docs (npm run docs:add)");
  });
  test.afterAll(() => void child?.kill());

  test("the app works offline with the docs there, and its service worker neither answers for /docs/ nor keeps it", async ({ page }) => {
    test.setTimeout(120_000);
    const base = `http://localhost:${PORT}`;
    await page.goto(`${base}/`);
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    await page.evaluate(() => navigator.serviceWorker.ready.then(() => true));
    await page.reload();
    await page.waitForFunction(() => !!navigator.serviceWorker.controller, null, { timeout: 15_000 });
    // Open the docs from the app's address, with the service worker in control of it: the answer comes from the network.
    const docs = await page.goto(`${base}/docs/embedding`);
    expect(docs?.status()).toBe(200);
    expect(docs?.fromServiceWorker(), "the service worker does not answer for /docs/").toBe(false);
    await expect(page.getByRole("heading", { level: 1 })).toContainText("Embedding Docxtor");
    const asset = await page.evaluate(async () => {
      const css = [...document.querySelectorAll('link[rel~="stylesheet"]')].map((l) => (l as HTMLLinkElement).href)[0];
      return css;
    });
    expect(asset).toContain("/docs/assets/");
    // Nothing of /docs/ is in any cache of the app.
    const kept = await page.evaluate(async () => {
      const urls: string[] = [];
      for (const name of await caches.keys()) for (const req of await (await caches.open(name)).keys()) urls.push(req.url);
      return urls;
    });
    expect(kept.length).toBeGreaterThan(10);
    expect(kept.filter((u) => new URL(u).pathname.startsWith("/docs"))).toEqual([]);
    // Offline: the app opens from its copy; the docs are not cached, so they do not open.
    await page.goto(`${base}/`);
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    await page.context().setOffline(true);
    await page.reload();
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor, null, { timeout: 15_000 });
    await expect(page.locator(".start")).toBeVisible();
    await expect(page.goto(`${base}/docs/embedding`)).rejects.toThrow(/ERR_INTERNET_DISCONNECTED/);
    await page.context().setOffline(false);
  });
});
