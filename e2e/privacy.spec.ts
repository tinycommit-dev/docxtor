// Every address the app calls. The test uses the app the way a person does (open it, open a document, type, spelling, fonts,
// download, the thesaurus, the dictionary, citations, Zotero, grammar, each AI provider, a picture from a web address, a paste with
// a picture on the web, the editor embedded in another page) and records every request to an address that is not the app's own.
// The hosts it saw must be the hosts in e2e/data/outside-hosts.json, each with what calls it, when, and what it is sent; PRIVACY.md
// says the same list in words (the last test checks it). A new outside call fails the test until it is listed.
//
// What leaves with no one asking is what happens in the phases marked `automatic`; every other call happens after a person
// chooses it (looks up a word, turns an extension on, sends a message to the assistant).
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { openBuilt } from "./support/docx-builder";
import { EVIL_HOST, hostileDocx } from "./support/hostile-docx";
import { chooseMenu, dialog, expect, expectErrors, fakeAnthropic, inPage, openFixture, pasteData, selectWord, test, type Page } from "./helpers";

interface HostEntry {
  host: string;
  what: string;
  calledBy: string;
  when: "automatic" | "when you ask" | "after you turn it on" | "after you set it up";
  onByDefault: boolean;
  sends: string;
  /** Whether the text of the document (beyond a word or a name the person gave) goes in the request. */
  documentText: boolean;
  /** What the phases below must show for this host. */
  seenIn: string[];
}
interface Listing {
  own: string[];
  hosts: HostEntry[];
  anyAddress: { what: string; calledBy: string; when: string; sends: string; seenIn: string[] }[];
}

const MARK = "Zephyrquill";

interface Seen {
  phase: string;
  method: string;
  url: string;
  host: string;
  body: string;
  headers: string;
}

const MOD = "ControlOrMeta";
const CORS = { "access-control-allow-origin": "*", "access-control-allow-headers": "*", "access-control-allow-methods": "GET, POST, OPTIONS" };

/** CSS as Google Fonts sends it: faces whose files are on gstatic. */
const GOOGLE_CSS = ["Inter", "Carlito", "Caladea", "Arimo", "Tinos", "Cousine", "Gelasio", "Roboto"]
  .map((f) => `@font-face{font-family:'${f}';font-style:normal;font-weight:400;src:url(https://fonts.gstatic.com/s/${f.toLowerCase()}/v1/regular.woff2) format('woff2')}`)
  .join("\n");

const sse = (events: string[]) => events.map((e) => `data: ${e}\n\n`).join("");
const OPENAI_STREAM = sse([JSON.stringify({ choices: [{ delta: { content: "Hello from the stand-in." }, finish_reason: "stop" }] }), "[DONE]"]);

/** What each outside address answers with, so the app goes on as it would online. */
function answerFor(url: URL, method: string): { status?: number; contentType: string; body: string | Buffer } {
  const host = url.host;
  if (host === "fonts.googleapis.com") return { contentType: "text/css", body: GOOGLE_CSS };
  if (host === "fonts.gstatic.com") return { contentType: "font/woff2", body: Buffer.alloc(0), status: 404 };
  if (host === "api.datamuse.com") return { contentType: "application/json", body: JSON.stringify([{ word: "mammoth", tags: ["syn", "adj"] }]) };
  if (host === "en.wiktionary.org") return { contentType: "application/json", body: JSON.stringify({ en: [{ partOfSpeech: "Noun", language: "English", definitions: [{ definition: "A word." }] }] }) };
  if (host === "doi.org")
    return {
      contentType: "application/vnd.citationstyles.csl+json",
      body: JSON.stringify({ type: "article-journal", title: "A study of things", author: [{ family: "Smith", given: "Jane" }], issued: { "date-parts": [[2020]] }, DOI: "10.1000/xyz123" }),
    };
  if (host === "api.languagetool.org") return { contentType: "application/json", body: JSON.stringify({ matches: [] }) };
  if (host === "localhost:23119" || host === "api.zotero.org") return { contentType: "application/json", body: "[]" };
  if (/(api\.openai\.com|openrouter\.ai|generativelanguage\.googleapis\.com|api\.groq\.com|api\.mistral\.ai|localhost:11434|localhost:1234)$/.test(host)) return { contentType: "text/event-stream", body: OPENAI_STREAM };
  if (host === "pictures.example.test") return { contentType: "image/png", body: Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64") };
  void method;
  return { status: 404, contentType: "text/plain", body: "not here" };
}

/** Record every request to another address, and answer it. Returns the list, and a way to name the phase that is running. */
async function watch(page: Page) {
  const seen: Seen[] = [];
  const state = { phase: "setup" };
  const own = `http://localhost:${process.env.DOCXTOR_E2E_PORT ?? 5180}`;
  page.on("request", (r) => {
    let u: URL;
    try {
      u = new URL(r.url());
    } catch {
      return;
    }
    if (u.origin === own || !/^https?:$/.test(u.protocol) || r.method() === "OPTIONS") return;
    seen.push({ phase: state.phase, method: r.method(), url: r.url(), host: u.host, body: (r.postData() ?? "") + " " + u.search, headers: JSON.stringify(r.headers()) });
  });
  await page.route("**/*", async (route) => {
    const req = route.request();
    const u = new URL(req.url());
    if (u.origin === own || !/^https?:$/.test(u.protocol)) return route.fallback();
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers: CORS });
    const a = answerFor(u, req.method());
    return route.fulfill({ status: a.status ?? 200, contentType: a.contentType, headers: CORS, body: a.body });
  });
  return {
    seen,
    phase: (name: string) => {
      state.phase = name;
    },
    hostsIn: (...phases: string[]) => [...new Set(seen.filter((s) => phases.includes(s.phase)).map((s) => s.host))].sort(),
    during: (phase: string) => seen.filter((s) => s.phase === phase),
  };
}

const listing = async (): Promise<Listing> => JSON.parse(await readFile(join(process.cwd(), "e2e/data/outside-hosts.json"), "utf8"));

async function panelAI(page: Page, settings: Record<string, unknown>, question: string) {
  await inPage(
    page,
    async (w, [s, q]) => {
      const ai = (w as unknown as { ai: { settings: Record<string, unknown>; input: HTMLTextAreaElement; send(): Promise<void>; toggle(open?: boolean): void } }).ai;
      ai.toggle(true);
      ai.settings = { ...ai.settings, ...s };
      ai.input.value = q;
      await ai.send();
    },
    [settings, question] as const,
  );
}

const PHASES_AUTOMATIC = ["start screen", "open, type and spelling", "font menu", "a document with a Google font", "download as PDF", "embedded editor"] as const;

test.describe("every address the app calls", () => {
  test.setTimeout(240_000);

  test("the hosts the app and the embedded editor call are the hosts in the list, and nothing else", async ({ page }) => {
    const w = await watch(page);
    const phase = w.phase;
    const settle = (ms = 1200) => page.waitForTimeout(ms);

    // ---- what happens with no one asking --------------------------------------------------------------------------
    phase("start screen");
    await page.goto("/");
    await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
    await settle();

    phase("open, type and spelling");
    await openBuilt(page, { sections: [{ paragraphs: [`The word ${MARK} is here, and so is a mispeled one.`, "A second paragraph for the thesaurus: a big dog."] }] });
    await page.locator(".flow .fp").first().click();
    await page.keyboard.press(`${MOD}+End`);
    await page.keyboard.type(" More text typed by a person.");
    await settle(2500);

    phase("a document with a Google font");
    await inPage(page, (wnd) => {
      const id = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      wnd.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 8 }, format: { font: "Roboto" } });
    });
    await settle();

    phase("download as PDF");
    const [pdf] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
    await pdf.path();
    await settle();

    phase("font menu");
    await page.locator(".font-pick").first().click();
    await expect(page.locator(".menu-title", { hasText: "More fonts" })).toBeVisible();
    await settle();
    await page.keyboard.press("Escape");

    phase("embedded editor");
    await page.goto("/examples/embed.html");
    await expect(page.locator("#log")).toContainText("left: opened", { timeout: 20_000 });
    await settle(2000);

    // ---- what happens when a person asks, or turns something on ---------------------------------------------------
    await openBuilt(page, { sections: [{ paragraphs: [`The word ${MARK} is here: a big dog.`, "Second paragraph."] }] });
    const firstId = async () => Number(await page.locator(".flow .fp").first().getAttribute("data-p"));

    phase("thesaurus before Datamuse is on");
    await selectWord(page, await firstId(), "big");
    await page.keyboard.press("Shift+F7");
    await settle();
    await page.keyboard.press("Escape");

    phase("the dictionary");
    await selectWord(page, await firstId(), "dog");
    await page.keyboard.press(`${MOD}+Shift+Y`);
    await expect(page.locator(".dict .dict-word")).toHaveText("dog");
    await settle();

    phase("thesaurus with Datamuse on");
    await inPage(page, (wnd) => (wnd.fe as unknown as { extensions: { enable(id: string): Promise<void> } }).extensions.enable("docxtor.thesaurus.datamuse"));
    await selectWord(page, await firstId(), "big");
    await page.keyboard.press("Shift+F7");
    await expect(page.locator(".th-panel .th-source").filter({ hasText: "Datamuse" })).toBeVisible();
    await page.keyboard.press("Escape");

    phase("citations: a DOI");
    await chooseMenu(page, "Tools", "Citations");
    const cite = page.locator(".cite-panel");
    await cite.getByRole("button", { name: "From a DOI or URL…" }).click();
    const lookup = dialog(page, "Add from a DOI or URL");
    await lookup.getByLabel("DOI or link").fill("10.1000/xyz123");
    await lookup.getByRole("button", { name: "Look up" }).click();
    await expect(dialog(page, "Check the source")).toBeVisible();
    await page.keyboard.press("Escape");

    phase("citations: Zotero on this computer");
    await cite.getByRole("button", { name: "From Zotero…" }).click();
    const z = dialog(page, "Add from Zotero");
    await expect(z).toBeVisible();
    await settle();

    phase("citations: the Zotero web library");
    await z.getByRole("button", { name: "Zotero settings…" }).click();
    const zs = dialog(page, "Zotero settings");
    await zs.getByLabel("Zotero web library").check();
    await zs.getByLabel("User ID").fill("987654");
    await zs.getByLabel("API key").fill("fake-zotero-key");
    await zs.getByRole("button", { name: "Save" }).click();
    await z.getByLabel("Search your Zotero library").fill("study");
    await settle();
    await page.keyboard.press("Escape");

    phase("a citation style from an address");
    await cite.getByLabel("Citation style").selectOption({ label: "Add a style from a CSL file…" });
    const addStyle = dialog(page, "Add a citation style");
    await addStyle.getByLabel("Address of a .csl file").fill("https://styles.example.test/nature.csl");
    await addStyle.getByRole("button", { name: "Add from address" }).click();
    await settle();
    await page.keyboard.press("Escape");

    phase("grammar with LanguageTool");
    await inPage(page, (wnd) => (wnd.fe as unknown as { extensions: { enable(id: string): Promise<void> } }).extensions.enable("docxtor.languagetool"));
    await settle(3000);

    phase("a picture from a web address");
    await page.locator(".flow .fp").first().click();
    await chooseMenu(page, "Insert", "Image", "From a URL…");
    const pic = page.getByRole("dialog", { name: "Picture from a URL" });
    await pic.getByLabel("Picture address").fill("https://pictures.example.test/logo.png");
    await pic.getByRole("button", { name: "Insert" }).click();
    await expect(pic).toHaveCount(0);

    phase("a paste with a picture on the web");
    await pasteData(page, { "text/html": `<p>Pasted</p><img src="https://pictures.example.test/pasted.png" alt="pasted" width="40" height="40">`, "text/plain": "Pasted" });
    await settle(2000);

    // The assistant, with each provider and a key made up for the test (or none, for the ones on this computer).
    const providers: [string, Record<string, unknown>][] = [
      ["anthropic", { provider: "anthropic", model: "claude-opus-5-5", apiKey: "fake-anthropic-key" }],
      ["openai", { provider: "openai", model: "gpt-x", apiKey: "fake-openai-key" }],
      ["openrouter", { provider: "openrouter", model: "x/y", apiKey: "fake-openrouter-key" }],
      ["gemini", { provider: "gemini", model: "gemini-x", apiKey: "fake-gemini-key" }],
      ["groq", { provider: "groq", model: "llama-x", apiKey: "fake-groq-key" }],
      ["mistral", { provider: "mistral", model: "mistral-x", apiKey: "fake-mistral-key" }],
      ["ollama", { provider: "ollama", model: "llama3", apiKey: "" }],
      ["lmstudio", { provider: "lmstudio", model: "local", apiKey: "" }],
      ["custom address", { provider: "custom", model: "m", apiKey: "fake-custom-key", baseUrl: "https://my-gateway.example.test/v1" }],
    ];
    await fakeAnthropic(page, "https://api.anthropic.com", "Hello from the stand-in.");
    for (const [name, settings] of providers) {
      phase(`assistant: ${name}`);
      await panelAI(page, settings, "Say hello.");
      await settle(800);
    }

    // A picture a document links on the web is fetched only when the person asks (the button on its placeholder).
    phase("a picture the document links");
    await inPage(page, async (wnd, b) => void (await wnd.fe.open(new Uint8Array(b), "linked.docx")), Array.from(hostileDocx(["external"]).bytes));
    await expect(page.locator(".pic-missing .pic-missing-action").first()).toBeVisible();
    await settle(1000);
    expect(w.during("a picture the document links").filter((r) => r.host === EVIL_HOST), "nothing is fetched before the person asks").toEqual([]);
    await page.locator(".pic-missing .pic-missing-action").first().click();
    await expect.poll(() => w.during("a picture the document links").some((r) => r.host === EVIL_HOST)).toBe(true);

    // DOCXTOR_PRIVACY_DUMP=file writes what was seen, to write the list from.
    if (process.env.DOCXTOR_PRIVACY_DUMP) {
      const { writeFile } = await import("node:fs/promises");
      await writeFile(process.env.DOCXTOR_PRIVACY_DUMP, JSON.stringify(w.seen.map((s) => ({ ...s, body: s.body.slice(0, 160) })), null, 1));
    }
    // ---- the list --------------------------------------------------------------------------------------------------
    const list = await listing();
    const typed = ["styles.example.test", "pictures.example.test", "my-gateway.example.test", EVIL_HOST];
    const seenHosts = [...new Set(w.seen.map((s) => s.host))].filter((h) => !typed.includes(h)).sort();
    const listed = list.hosts.map((h) => h.host).sort();
    expect(seenHosts, "the hosts the app called are the hosts in e2e/data/outside-hosts.json").toEqual(listed);

    // Addresses a person types (a style, a picture, a gateway) are called, and only after they type them.
    for (const [phaseName, host] of [["a citation style from an address", "styles.example.test"], ["a picture from a web address", "pictures.example.test"], ["a paste with a picture on the web", "pictures.example.test"], ["assistant: custom address", "my-gateway.example.test"], ["a picture the document links", EVIL_HOST]] as const) {
      expect(w.hostsIn(phaseName), phaseName).toContain(host);
    }
    for (const entry of list.anyAddress) for (const p of entry.seenIn) expect(w.during(p).length, `${p} reached the network`).toBeGreaterThan(0);

    // When: what happens with no one asking is exactly the automatic hosts.
    const automatic = new Set(w.hostsIn(...PHASES_AUTOMATIC));
    for (const h of list.hosts) {
      expect(automatic.has(h.host), `${h.host}: ${h.when === "automatic" ? "is called without anyone asking" : `is only called ${h.when}`}`).toBe(h.when === "automatic");
      // On by default: the feature is switched on in a fresh install (what it takes before a request goes out is `when`).
      expect(h.onByDefault, `${h.host}: onByDefault is ${h.when === "after you turn it on" ? "false: the extension is off until it is turned on" : "true"}`).toBe(h.when !== "after you turn it on");
      for (const p of h.seenIn) expect(w.hostsIn(p), `${h.host} is called in "${p}"`).toContain(h.host);
    }

    // What: the text of the document goes only where the list says, and only after the person asked.
    for (const h of list.hosts) {
      const calls = w.seen.filter((s) => s.host === h.host);
      const carriesText = calls.some((c) => c.body.includes(MARK));
      if (h.documentText) expect(carriesText, `${h.host} is listed as getting the text of the document, and did not`).toBe(true);
      else expect(carriesText, `${h.host} got the text of the document and is not listed as getting it`).toBe(false);
      if (h.documentText) expect(h.when, `${h.host} gets document text, so a person must turn it on or set it up`).not.toBe("automatic");
    }
    // Keys go where they belong: a key made up for one provider is in no other provider's request, and not in an address.
    const keyHost: Record<string, string> = {
      "fake-anthropic-key": "api.anthropic.com",
      "fake-openai-key": "api.openai.com",
      "fake-openrouter-key": "openrouter.ai",
      "fake-gemini-key": "generativelanguage.googleapis.com",
      "fake-groq-key": "api.groq.com",
      "fake-mistral-key": "api.mistral.ai",
      "fake-custom-key": "my-gateway.example.test",
      "fake-zotero-key": "api.zotero.org",
    };
    for (const [key, host] of Object.entries(keyHost)) {
      const carrying = w.seen.filter((s) => s.headers.includes(key) || s.body.includes(key) || s.url.includes(key));
      expect(carrying.length, `${key} was sent`).toBeGreaterThan(0);
      for (const c of carrying) {
        expect(c.host, `${key} went to ${c.host}`).toBe(host);
        expect(c.url, `${key} is in an address`).not.toContain(key);
      }
    }
    // What the assistant sends is the document, with the key: the first request holds the text of the document.
    for (const h of ["api.anthropic.com", "api.openai.com", "openrouter.ai", "generativelanguage.googleapis.com", "api.groq.com", "api.mistral.ai", "localhost:11434", "localhost:1234"]) {
      expect(w.seen.some((s) => s.host === h && s.body.includes(MARK)), `${h} gets the text of the document`).toBe(true);
    }
  });

  test("self-hosted: with the Google Fonts extension off and a policy that allows only the site itself, nothing is called and nothing is refused", async ({ page, browserName }) => {
    // The policy PRIVACY.md describes: deploy/_headers with every outside address taken out of it.
    const headers = await readFile(join(process.cwd(), "deploy/_headers"), "utf8");
    const policy = /Content-Security-Policy:\s*(.+)/.exec(headers)![1]
      .replace(/ https:\/\/fonts\.googleapis\.com/g, "")
      .replace(/ https:\/\/fonts\.gstatic\.com/g, "")
      .replace(/(img-src [^;]*?) https: http:/, "$1")
      .replace(/(media-src [^;]*?) https:/, "$1")
      .replace(/connect-src [^;]*/, "connect-src 'self' data: blob:");
    expect(policy).not.toMatch(/https?:/);
    // (The dev server this test runs against talks to its own page over a WebSocket for hot reloading; a built copy has no such thing.)
    const served = policy.replace("connect-src 'self'", `connect-src 'self' ws://localhost:${process.env.DOCXTOR_E2E_PORT ?? 5180}`);
    await page.addInitScript(() => {
      const w = window as unknown as { __csp: string[] };
      w.__csp = [];
      document.addEventListener("securitypolicyviolation", (e) => w.__csp.push(`${e.violatedDirective} ${e.blockedURI}`));
    });
    // The dev server's hot-reload socket cannot connect to a page that Playwright re-serves with the policy (the browser's local network
    // checks refuse it); a built copy has none. These are the only errors the page may report.
    // The browsers word the refusal differently: Chromium says it twice (the console, the rejected fetch), Firefox and WebKit once.
    // Vite's client then tries to report the failure to its server, which fails too, but only when its timing allows: one pattern covers both.
    if (browserName === "chromium") expectErrors(page, /WebSocket connection to 'ws:\/\/localhost/, /failed to connect to websocket|Failed to send error to Vite server/, /violates the following Content Security Policy directive: "connect-src[^"]*"/, /Fetch API cannot load https:\/\/elsewhere\.example\.test/);
    else expectErrors(page, browserName === "firefox" ? /blocked the loading of a resource \(connect-src\) at https:\/\/elsewhere\.example\.test/ : /Refused to connect to https:\/\/elsewhere\.example\.test\S* because it does not appear in the connect-src directive/);
    const w = await watch(page);
    await page.route("**/*", async (route) => {
      if (route.request().resourceType() !== "document") return route.fallback();
      const res = await route.fetch();
      return route.fulfill({ response: res, headers: { ...res.headers(), "content-security-policy": served } });
    });
    w.phase("self-hosted");
    await openBuilt(page, { sections: [{ paragraphs: [`The word ${MARK} is here: a big dog.`] }] });
    await inPage(page, (wnd) => (wnd.fe as unknown as { extensions: { disable(id: string): Promise<void> } }).extensions.disable("docxtor.google-fonts"));
    await inPage(page, (wnd) => {
      const id = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      wnd.fe.editor.exec({ op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 8 }, format: { font: "Roboto" } });
    });
    await page.locator(".font-pick").first().click();
    await expect(page.locator(".menu-item", { hasText: "Calibri" }).first()).toBeVisible();
    await expect(page.locator(".menu-title", { hasText: "More fonts" })).toHaveCount(0);
    await page.keyboard.press("Escape");
    const [pdf] = await Promise.all([page.waitForEvent("download"), chooseMenu(page, "File", "Download", "PDF document (.pdf)")]);
    await pdf.path();
    await page.waitForTimeout(1500);
    expect(w.seen.map((s) => s.url)).toEqual([]);
    expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp)).toEqual([]);
    // The policy is really there: a request the page should not make is refused, and the listener hears of it.
    await page.evaluate(() => void fetch("https://elsewhere.example.test/x").catch(() => undefined));
    await expect.poll(() => page.evaluate(() => (window as unknown as { __csp: string[] }).__csp.length)).toBe(1);
    w.seen.length = 0;
  });
});
