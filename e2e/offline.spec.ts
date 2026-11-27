// Works offline: the app is an installable web app with a service worker that keeps its files, and its own copy of the
// interface fonts. These tests build the app, have the dev server serve that build under /__built/ (see web/vite.config.ts),
// and run the real service worker: visit once, go offline, reload, and work.
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";

const BUILT = resolve(process.cwd(), "web/dist-test");

test.describe.configure({ mode: "serial" });

test.beforeAll(() => {
  test.setTimeout(240_000);
  execFileSync("npx", ["vite", "build", "--outDir", "dist-test", "--emptyOutDir"], { cwd: resolve(process.cwd(), "web"), stdio: "pipe" });
});

const list = (): string[] => {
  const sw = readFileSync(resolve(BUILT, "sw.js"), "utf8");
  return JSON.parse(/const PRECACHE = (\[[\s\S]*?\]);\nconst CACHE/.exec(sw)![1]!);
};

test.describe("works offline", () => {
  test("the build has no outside font or script address, and its service worker lists everything the app needs", () => {
    const files = list();
    expect(files).toContain("./");
    expect(files).toContain("index.html");
    expect(files).toContain("manifest.webmanifest");
    expect(files).toContain("icons/icon-192.png");
    expect(files).toContain("icons/icon-512.png");
    expect(files.some((f) => /^assets\/docxtor_bg-.*\.wasm$/.test(f))).toBe(true);
    expect(files.some((f) => /^assets\/index-.*\.js$/.test(f))).toBe(true);
    expect(files.some((f) => /^assets\/index-.*\.css$/.test(f))).toBe(true);
    // The interface font and Word's stand-ins, and the spelling dictionaries.
    for (const family of ["inter-latin-opsz-normal", "carlito-latin-400-normal", "caladea-latin-400-normal", "arimo-latin-400-normal", "tinos-latin-400-normal", "cousine-latin-400-normal", "gelasio-latin-400-normal"]) {
      expect(files.some((f) => f.includes(family) && f.endsWith(".woff2")), family).toBe(true);
    }
    expect(files.filter((f) => f.endsWith(".dic"))).toHaveLength(5);
    // Every file it names exists, and nothing in the page or its styles asks another server for a font.
    for (const f of files.filter((x) => x !== "./")) expect(existsSync(resolve(BUILT, f)), f).toBe(true);
    const index = readFileSync(resolve(BUILT, "index.html"), "utf8");
    expect(index).not.toContain("fonts.googleapis.com");
    expect(index).not.toContain("fonts.gstatic.com");
    expect(index).toContain('rel="manifest"');
    const css = readdirSync(resolve(BUILT, "assets")).filter((f) => f.endsWith(".css"));
    for (const f of css) expect(readFileSync(resolve(BUILT, "assets", f), "utf8")).not.toMatch(/https?:\/\/fonts\./);
  });

  test("the manifest names the app and its icons, which are all there", async ({ page, request }) => {
    const res = await request.get("/__built/manifest.webmanifest");
    expect(res.ok()).toBe(true);
    const manifest = await res.json();
    expect(manifest).toMatchObject({ name: "Docxtor", short_name: "Docxtor", display: "standalone", start_url: "./", scope: "./" });
    const sizes = manifest.icons.map((i: { sizes: string; purpose: string }) => `${i.sizes}:${i.purpose}`);
    expect(sizes).toContain("192x192:any");
    expect(sizes).toContain("512x512:any");
    expect(sizes).toContain("512x512:maskable");
    for (const icon of manifest.icons as { src: string }[]) expect((await request.get(`/__built/${icon.src}`)).ok(), icon.src).toBe(true);
    await page.goto("/__built/");
    await expect(page.locator('link[rel="manifest"]')).toHaveAttribute("href", "./manifest.webmanifest");
  });

  test("after one visit Docxtor opens and edits a document with no connection, in its own fonts", async ({ page, context, browserName }) => {
    // The Google Fonts extension asks Google for fonts when they are used; it is an extension of its own, and off here.
    await page.addInitScript(() => localStorage.setItem("docxtor.extensions", JSON.stringify({ disabled: ["docxtor.google-fonts"], enabled: [] })));
    const outside: string[] = [];
    page.on("request", (r) => {
      if (!r.url().startsWith("http://localhost")) outside.push(r.url());
    });
    await page.goto("/__built/");
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    // The service worker installs, keeps the files, and takes the page over.
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.waitForFunction(() => !!navigator.serviceWorker.controller);
    const kept = await page.evaluate(async () => {
      const names = await caches.keys();
      const cache = await caches.open(names.find((n) => n.startsWith("docxtor-app-"))!);
      return (await cache.keys()).map((r) => new URL(r.url).pathname.replace("/__built/", ""));
    });
    expect(kept.some((p) => p.endsWith(".wasm"))).toBe(true);
    expect(kept.some((p) => p.includes("carlito-latin-400-normal"))).toBe(true);
    expect(kept.length).toBe(list().length);

    await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { createIn(b: unknown, d: string, n: string, md: string): Promise<void>; files: { library: unknown } } }).docxtor;
      await w.createIn(w.files.library, "", "Offline.docx", "# Works offline\n\nWritten with no connection.");
    });
    await expect(page.locator(".flow .fp").first()).toContainText("Works offline");
    await expect(page.locator(".led")).toHaveText(/Saved/);

    await context.setOffline(true);
    await page.reload();
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: { engine?: unknown } }).docxtor?.engine);
    await expect(page.locator(".flow .fp").first()).toContainText("Works offline");
    await expect(page.locator(".stat.offline")).toBeVisible();
    await expect(page.locator(".stat.offline")).toHaveText("Offline");
    // Editing and saving work.
    await page.locator(".flow .fp").nth(1).click();
    await page.keyboard.press("End");
    await page.keyboard.type(" Still works.");
    await expect(page.locator(".flow .fp").nth(1)).toContainText("Still works.");
    await expect(page.locator(".led")).toHaveText(/Saved/);
    // The interface and the stand-ins for Word's fonts come from the kept files.
    const fonts = await page.evaluate(async () => {
      await document.fonts.ready;
      const loaded = [...document.fonts].filter((f) => f.status === "loaded").map((f) => f.family.replace(/"/g, ""));
      return { loaded: [...new Set(loaded)], inter: document.fonts.check('13px "Inter Variable"'), ui: getComputedStyle(document.querySelector(".docxtor")!).getPropertyValue("--font-ui") };
    });
    expect(fonts.ui).toContain("Inter Variable");
    expect(fonts.loaded).toContain("Inter Variable");
    expect(fonts.inter).toBe(true);
    // A spelling dictionary loads from the kept files too.
    const dict = await page.evaluate(async () => {
      const w = (window as unknown as { docxtor: { fe: { extensions: { spellingDictionary(l: string): Promise<{ aff: unknown } | null> } } } }).docxtor;
      return !!(await w.fe.extensions.spellingDictionary("en-US"));
    });
    expect(dict).toBe(true);
    // Nothing was asked of another server, online or off.
    expect(outside).toEqual([]);
    // Back online, the note goes away.
    await context.setOffline(false);
    // (Playwright's Firefox flips `navigator.onLine` back but sends no `online` event to a page that was loaded while offline, which is what the
    // note listens for; a real Firefox sends it. So there the test stops at what Firefox does.)
    if (browserName === "firefox") await expect.poll(() => page.evaluate(() => navigator.onLine)).toBe(true);
    else await expect(page.locator(".stat.offline")).toBeHidden();
  });

  test("only Docxtor's own page opens the app; other addresses get what the server says, and kept copies are versioned", async ({ page, browserName }) => {
    await page.goto("/__built/");
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.waitForFunction(() => !!navigator.serviceWorker.controller);
    // Another page on the same server is not answered with the start screen.
    const missing = await page.goto("/__built/privacy.html");
    expect(missing?.status()).toBe(404);
    await expect(page.locator("body")).toHaveText("not found");
    expect(await page.locator(".docxtor").count()).toBe(0);
    // A kept file opens as itself (the manifest), not as the app.
    if (browserName === "firefox") {
      // (Firefox saves a .webmanifest address as a download instead of showing it, so the kept file is asked for from the page, through the service worker.)
      const kept = await page.evaluate(() => fetch("/__built/manifest.webmanifest").then((r) => r.text()));
      expect(kept).toContain('"short_name"');
    } else {
      await page.goto("/__built/manifest.webmanifest");
      await expect(page.locator("body")).toContainText('"short_name"');
      expect(await page.locator(".docxtor").count()).toBe(0);
    }
    // Docxtor's own address, with a query and a hash, is still the app.
    await page.goto("/__built/index.html?x=1#/somewhere");
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    // What is kept as it is used goes in a cache named for the version, so an update can let go of it.
    const late = readdirSync(resolve(BUILT, "assets")).find((f) => f.includes("-cyrillic-") && f.endsWith(".woff2"))!;
    expect(late).toBeTruthy();
    const names = await page.evaluate(async (file) => {
      await fetch(`/__built/assets/${file}`);
      await new Promise((r) => setTimeout(r, 300));
      return caches.keys();
    }, late);
    expect(names).not.toContain("docxtor-runtime");
    expect(names.some((n) => /^docxtor-runtime-.+/.test(n))).toBe(true);
    expect(names.filter((n) => n.startsWith("docxtor-app-"))).toHaveLength(1);
  });
});
