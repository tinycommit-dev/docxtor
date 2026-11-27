// LanguageTool, with a mocked server: off by default and silent until turned on (it sends text away, so it asks
// first), its findings join Docxtor's own, requests are batched and remembered, it backs off when told to, and the
// settings dialog says where the text goes. No test reaches a real server.
import { openBuilt, type DocSpec } from "./support/docx-builder";
import { chooseMenu, contextItem, contextMenu, dialog, expect, inPage, menuItem, openMenu, test, type Locator, type Page } from "./helpers";

const PUBLIC = "https://api.languagetool.org/v2/check";
const OWN = "http://lt.test:8081";

interface Reply {
  status?: number;
  headers?: Record<string, string>;
  body?: unknown;
  /** Wait this long before answering. */
  delay?: number;
}
type Handler = (params: URLSearchParams, call: number) => Reply | Promise<Reply>;

/** Answer a LanguageTool server's /v2/check; returns every request's form data. */
async function serve(page: Page, url: string, handler: Handler = () => ({})) {
  const calls: URLSearchParams[] = [];
  const stats = { inFlight: 0, mostInFlight: 0 };
  await page.route(url, async (route) => {
    const request = route.request();
    const cors = { "access-control-allow-origin": "*", "access-control-allow-headers": "*" };
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers: cors });
    const params = new URLSearchParams(request.postData() ?? "");
    calls.push(params);
    stats.mostInFlight = Math.max(stats.mostInFlight, ++stats.inFlight);
    const reply = await handler(params, calls.length);
    if (reply.delay) await new Promise((r) => setTimeout(r, reply.delay));
    stats.inFlight--;
    await route.fulfill({ status: reply.status ?? 200, contentType: "application/json", headers: { ...cors, ...reply.headers }, body: JSON.stringify(reply.body ?? { matches: [] }) });
  });
  return Object.assign(calls, { stats });
}

/** A LanguageTool match for the first `needle` in `text`. */
const match = (text: string, needle: string, message: string, replacements: string[], rule: Record<string, unknown> = {}) => ({
  message,
  shortMessage: "",
  offset: text.indexOf(needle),
  length: needle.length,
  replacements: replacements.map((value) => ({ value })),
  rule: { id: "TEST_RULE", description: "A test rule", issueType: "grammar", category: { id: "GRAMMAR", name: "Grammar" }, ...rule },
});

/** Settings the extension finds in the browser when the app starts (a server of its own). */
async function seedSettings(page: Page, settings: Record<string, unknown>) {
  await page.addInitScript(([s]) => localStorage.setItem("docxtor.ext.docxtor.languagetool", JSON.stringify(s)), [settings] as const);
}

/** Turn the extension on from the page, once the document is open (the Extensions dialog is tested on its own). */
const turnOn = (page: Page) => inPage(page, (w) => (w.fe as unknown as { extensions: { enable(id: string): Promise<void> } }).extensions.enable("docxtor.languagetool"));

const doc = (paragraphs: string[], extra: Partial<DocSpec> = {}): DocSpec => ({ sections: [{ paragraphs }], lang: "en-US", ...extra });
const openDoc = (page: Page, paragraphs: string[], extra: Partial<DocSpec> = {}) => openBuilt(page, doc(paragraphs, extra));

const marks = (page: Page) => page.locator(".sp-layer .sp-g");
const mark = (page: Page, word: string) => page.locator(`.sp-layer .sp-g[data-word="${word}"]`);
const engineText = (page: Page) => inPage(page, (w) => w.fe.editor.engine!.text());

async function rightClick(page: Page, u: Locator) {
  const b = (await u.first().boundingBox())!;
  await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2, { button: "right" });
  await expect(contextMenu(page)).toBeVisible();
}

const extensionSwitch = (page: Page) => dialog(page, "Extensions").locator('.switch[data-ext="docxtor.languagetool"]');

test.describe("LanguageTool is off until it is turned on", () => {
  test("nothing is sent, no settings are offered, and the Extensions dialog says what it would send", async ({ page }) => {
    const calls = await serve(page, PUBLIC, (p) => ({ body: { matches: [match(p.get("text") ?? "", "go", "Wrong verb form", ["goes"])] } }));
    await openDoc(page, ["He go to school."]);
    await page.waitForTimeout(1500);
    expect(calls.length).toBe(0);
    await expect(marks(page)).toHaveCount(0);

    await openMenu(page, "Tools");
    await menuItem(page, "Spelling and grammar").click();
    await expect(menuItem(page, "Show grammar suggestions")).toBeVisible();
    await expect(menuItem(page, "LanguageTool settings…")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");

    await chooseMenu(page, "Extensions", "Manage extensions…");
    const row = dialog(page, "Extensions").locator(".ext-row").filter({ hasText: "LanguageTool" });
    await expect(row.locator(".switch")).toHaveAttribute("aria-checked", "false");
    await expect(row).toContainText("Sends the text of your document to the LanguageTool server named in its settings (languagetool.org unless you change it).");
    expect(calls.length).toBe(0);
  });

  test("turning it on says plainly that text is sent, and Cancel keeps it off", async ({ page }) => {
    const calls = await serve(page, PUBLIC, (p) => ({ body: { matches: [match(p.get("text") ?? "", "go", "Wrong verb form", ["goes"])] } }));
    await openDoc(page, ["He go to school."]);
    await chooseMenu(page, "Extensions", "Manage extensions…");
    await extensionSwitch(page).click();
    const ask = page.getByRole("alertdialog", { name: "Send your text to a server?" });
    await expect(ask).toContainText("Sends the text of your document to the LanguageTool server named in its settings (languagetool.org unless you change it).");
    await ask.getByRole("button", { name: "Cancel" }).click();
    await expect(extensionSwitch(page)).toHaveAttribute("aria-checked", "false");
    await page.waitForTimeout(800);
    expect(calls.length).toBe(0);
  });

  test("once on, its findings join Docxtor's own, with the message and the fixes in the right-click menu", async ({ page }) => {
    const calls = await serve(page, PUBLIC, (p) => {
      const text = p.get("text") ?? "";
      return { body: { matches: [match(text, "go", "The verb does not agree with “He”.", ["goes", "went", "going"])] } };
    });
    await openDoc(page, ["He go to school and the the cat."]);
    await chooseMenu(page, "Extensions", "Manage extensions…");
    await extensionSwitch(page).click();
    await page.getByRole("alertdialog", { name: "Send your text to a server?" }).getByRole("button", { name: "Turn on" }).click();
    await expect(extensionSwitch(page)).toHaveAttribute("aria-checked", "true");
    await dialog(page, "Extensions").getByRole("button", { name: "Done" }).click();

    await expect(mark(page, "go")).toHaveCount(1);
    // Docxtor's own rule is still there beside it.
    await expect(mark(page, "the the")).toHaveCount(1);
    expect(calls.length).toBe(1);
    expect(calls[0].get("text")).toBe("He go to school and the the cat.");
    expect(calls[0].get("language")).toBe("en-US");
    expect(calls[0].get("username")).toBeNull();
    expect(calls[0].get("apiKey")).toBeNull();

    await rightClick(page, mark(page, "go"));
    await expect(contextMenu(page).locator(".menu-note")).toHaveText("The verb does not agree with “He”.");
    await expect(contextMenu(page).locator(".menu-label.strong")).toHaveText(["goes", "went", "going"]);
    await contextItem(page, "goes").click();
    await expect.poll(() => engineText(page)).toContain("He goes to school");
    await expect(mark(page, "go")).toHaveCount(0);

    // It stays on after a reload.
    await page.goto("/?open=simple.docx");
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await chooseMenu(page, "Extensions", "Manage extensions…");
    await expect(extensionSwitch(page)).toHaveAttribute("aria-checked", "true");
  });

  test("turning it off again takes its underlines away and stops the requests", async ({ page }) => {
    const calls = await serve(page, PUBLIC, (p) => ({ body: { matches: [match(p.get("text") ?? "", "go", "Wrong verb form", ["goes"])] } }));
    await openDoc(page, ["He go to school."]);
    await turnOn(page);
    await expect(mark(page, "go")).toHaveCount(1);
    await chooseMenu(page, "Extensions", "Manage extensions…");
    await extensionSwitch(page).click();
    await expect(extensionSwitch(page)).toHaveAttribute("aria-checked", "false");
    await expect(marks(page)).toHaveCount(0);
    const before = calls.length;
    await dialog(page, "Extensions").getByRole("button", { name: "Done" }).click();
    await inPage(page, (w) => w.fe.editor.exec({ op: "insert_text", at: { p: Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p), o: 0 }, text: "Okay. " }));
    await page.waitForTimeout(1200);
    expect(calls.length).toBe(before);
  });
});

test.describe("LanguageTool settings", () => {
  test("the dialog says where the text goes, checks the address, and saves the server and the keys", async ({ page }) => {
    const publicCalls = await serve(page, PUBLIC, (p) => ({ body: { matches: [match(p.get("text") ?? "", "go", "Wrong verb form", ["goes"])] } }));
    const ownCalls = await serve(page, `${OWN}/v2/check`, (p) => ({ body: { matches: [match(p.get("text") ?? "", "school", "Own server says hello", ["college"])] } }));
    await openDoc(page, ["He go to school."]);
    await turnOn(page);
    await expect(mark(page, "go")).toHaveCount(1);

    await chooseMenu(page, "Tools", "Spelling and grammar", "LanguageTool settings…");
    const d = dialog(page, "LanguageTool settings");
    await expect(d).toContainText("The text of the paragraphs being checked is sent to api.languagetool.org.");
    await expect(d).toContainText("Nothing is sent while this extension is off.");
    await expect(d.getByLabel("Server")).toHaveValue("https://api.languagetool.org");

    // The sentence follows the address typed, and an address that isn't one can't be saved.
    await d.getByLabel("Server").fill("ftp://nope");
    await expect(d.locator(".lt-problem")).toBeVisible();
    await expect(d.getByRole("button", { name: "Save" })).toBeDisabled();
    await d.getByLabel("Server").fill("http://lt.test:8081/");
    await expect(d.locator(".lt-problem")).toBeHidden();
    await expect(d).toContainText("is sent to lt.test:8081.");
    await d.getByLabel("Username (optional)").fill("me@example.org");
    await d.getByLabel("API key (optional)").fill("secret-key");
    await expect(d.getByLabel("API key (optional)")).toHaveAttribute("type", "password");
    await d.getByRole("button", { name: "Save" }).click();
    await expect(d).toHaveCount(0);

    // Everything is asked again, of the new server, with the keys; the old server hears nothing more.
    await expect(mark(page, "school")).toHaveCount(1);
    await expect(mark(page, "go")).toHaveCount(0);
    expect(ownCalls[0].get("username")).toBe("me@example.org");
    expect(ownCalls[0].get("apiKey")).toBe("secret-key");
    expect(ownCalls[0].get("text")).toBe("He go to school.");
    expect(publicCalls.length).toBe(1);

    // Kept across reloads.
    await page.goto("/?open=simple.docx");
    await expect(page.locator(".flow .fp").first()).toBeVisible();
    await turnOn(page);
    await chooseMenu(page, "Tools", "Spelling and grammar", "LanguageTool settings…");
    const again = dialog(page, "LanguageTool settings");
    await expect(again.getByLabel("Server")).toHaveValue("http://lt.test:8081");
    await expect(again.getByLabel("Username (optional)")).toHaveValue("me@example.org");
    await again.getByRole("button", { name: "Cancel" }).click();
  });

  test("LanguageTool's own spelling findings are left out unless asked for", async ({ page }) => {
    const calls = await serve(page, PUBLIC, (p) => ({
      body: { matches: [match(p.get("text") ?? "", "world", "Possible spelling mistake found.", ["word"], { issueType: "misspelling", category: { id: "TYPOS", name: "Possible Typo" } })] },
    }));
    await openDoc(page, ["Hello world today."]);
    await turnOn(page);
    await expect.poll(() => calls.length).toBe(1);
    await page.waitForTimeout(500);
    await expect(marks(page)).toHaveCount(0);
    await chooseMenu(page, "Tools", "Spelling and grammar", "LanguageTool settings…");
    const d = dialog(page, "LanguageTool settings");
    await d.getByLabel("Also show LanguageTool's spelling suggestions").check();
    await d.getByRole("button", { name: "Save" }).click();
    // Drawn like a misspelling.
    await expect(page.locator('.sp-layer .sp-g[data-kind="spelling"][data-word="world"]')).toHaveCount(1);
  });
});

test.describe("LanguageTool requests", () => {
  test("paragraphs go together in one request and come back to the right paragraph, and a paragraph already checked is not sent again", async ({ page }) => {
    await seedSettings(page, { server: OWN });
    const calls = await serve(page, `${OWN}/v2/check`, (p) => {
      const text = p.get("text") ?? "";
      const found = [] as unknown[];
      if (text.includes("beta")) found.push(match(text, "beta", "In the second", ["b"]));
      if (text.includes("gamma")) found.push(match(text, "gamma", "In the third", ["g"]));
      return { body: { matches: found } };
    });
    await openDoc(page, ["alpha one is here.", "the beta is here.", "the gamma is here."]);
    await turnOn(page);
    await expect(mark(page, "beta")).toHaveCount(1);
    await expect(mark(page, "gamma")).toHaveCount(1);
    expect(calls.length).toBe(1);
    // A blank line between paragraphs.
    expect(calls[0].get("text")).toBe("alpha one is here.\n\nthe beta is here.\n\nthe gamma is here.");
    const ids = await page.locator(".flow .fp").evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.p));
    expect(await mark(page, "beta").getAttribute("data-p")).toBe(ids[1]);
    expect(await mark(page, "gamma").getAttribute("data-p")).toBe(ids[2]);

    // Changing one paragraph sends that paragraph alone.
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "insert_text", at: { p: id, o: 0 }, text: "Well, " }), Number(ids[0]));
    await expect.poll(() => calls.length).toBe(2);
    expect(calls[1].get("text")).toBe("Well, alpha one is here.");
    expect(await mark(page, "beta").count()).toBe(1);
  });

  test("the language is the paragraph's own, mapped to a code LanguageTool knows", async ({ page }) => {
    await seedSettings(page, { server: OWN });
    const calls = await serve(page, `${OWN}/v2/check`);
    await openDoc(page, ["Hola amigos, buenos días.", "Good day to you, my friends."], { lang: "es-MX" });
    // The second paragraph is English.
    const ids = await page.locator(".flow .fp").evaluateAll((els) => els.map((e) => Number((e as HTMLElement).dataset.p)));
    await inPage(page, (w, id) => w.fe.editor.exec({ op: "set_language", lang: "en-GB", ids: [id] }), ids[1]);
    await turnOn(page);
    // One request for each language, never mixed.
    await expect.poll(() => calls.length).toBe(2);
    const byLanguage = Object.fromEntries(calls.map((c) => [c.get("language"), c.get("text")]));
    expect(byLanguage).toEqual({ es: "Hola amigos, buenos días.", "en-GB": "Good day to you, my friends." });
  });

  test("a language the server does not know is dropped quietly", async ({ page }) => {
    await seedSettings(page, { server: OWN });
    const calls = await serve(page, `${OWN}/v2/check`, () => ({ status: 400, body: "Error: 'xx' is not a language code known to LanguageTool." }));
    await openDoc(page, ["Some text here.", "Another paragraph here."], { lang: "xx-XX" });
    await turnOn(page);
    await expect.poll(() => calls.length).toBe(1);
    await page.waitForTimeout(800);
    expect(calls.length).toBe(1);
    await expect(marks(page)).toHaveCount(0);
  });

  test("one request at a time", async ({ page }) => {
    await seedSettings(page, { server: OWN });
    const calls = await serve(page, `${OWN}/v2/check`, () => ({ delay: 700 }));
    await openDoc(page, ["First paragraph here."]);
    await turnOn(page);
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "Alpha. " });
    });
    await page.waitForTimeout(500);
    await inPage(page, (w) => {
      const p = Number(document.querySelector<HTMLElement>(".flow .fp")!.dataset.p);
      w.fe.editor.exec({ op: "insert_text", at: { p, o: 0 }, text: "Beta. " });
    });
    await expect.poll(() => calls.length, { timeout: 12000 }).toBeGreaterThanOrEqual(2);
    expect(calls.stats.mostInFlight).toBe(1);
  });

  test("it backs off when the server says too many requests, then tries again", async ({ page }) => {
    await seedSettings(page, { server: OWN });
    const calls = await serve(page, `${OWN}/v2/check`, (p, n) =>
      n === 1 ? { status: 429, headers: { "retry-after": "1" }, body: { error: "too many" } } : { body: { matches: [match(p.get("text") ?? "", "go", "Wrong verb form", ["goes"])] } },
    );
    await openDoc(page, ["He go to school."]);
    const t0 = Date.now();
    await turnOn(page);
    await expect(mark(page, "go")).toHaveCount(1, { timeout: 15000 });
    expect(calls.length).toBe(2);
    // It waited at least the two seconds it starts with, not a busy loop.
    expect(Date.now() - t0).toBeGreaterThan(1800);
    expect(calls[1].get("text")).toBe(calls[0].get("text"));
  });

  test("a server that cannot be reached leaves Docxtor's own grammar working", async ({ page }) => {
    await seedSettings(page, { server: OWN });
    await page.route(`${OWN}/v2/check`, (route) => route.abort());
    await openDoc(page, ["Say the the word."]);
    await turnOn(page);
    await expect(mark(page, "the the")).toHaveCount(1);
    await page.waitForTimeout(500);
    await expect(marks(page)).toHaveCount(1);
  });
});
