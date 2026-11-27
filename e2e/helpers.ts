// Shared setup for the browser tests: a hermetic page, ways to open a fixture, and the
// handful of locators every spec needs (menus, paragraphs, dialogs).
import { expect, test as base, type Locator, type Page } from "@playwright/test";
import { KNOWN_ISSUES } from "./support/known-issues";

/** The parts of a section the tests read. */
interface Section {
  end?: number;
  start: string;
  width: number;
  height: number;
  margin: number[];
  lineNumbers?: { countBy: number; start: number; restart: string; distance?: number };
  cols: { num: number; space: number; sep: boolean };
}
/** A watermark as the page setup reports it. */
export interface Watermark {
  kind: "text" | "picture";
  text?: string;
  font?: string;
  color?: string;
  opacity?: number;
  layout: "diagonal" | "horizontal";
  angle: number;
  width?: number;
  height?: number;
  media?: string;
}
export interface Bookmark {
  name: string;
  p: number;
  o: number;
  endP: number;
  endO: number;
  text: string;
}
interface PageSetup {
  width: number;
  height: number;
  margin: number[];
  sections: (Section & { watermark?: Watermark })[];
  watermark?: Watermark;
  pageColor?: string;
  hyphenation: boolean;
  lang?: string;
}

/** What `window.docxtor` exposes (see web/src/main.ts); only the parts the tests read. */
interface Engine {
  counts(): { words: number; chars: number; charsNoSpaces: number; paragraphs: number; notes: { words: number; paragraphs: number }; boxes: { words: number; paragraphs: number } };
  fontsUsed(): { family: string; chars: number }[];
  formatAt(p: { p: number; o: number }): { run: { bold?: boolean; italic?: boolean; strike?: boolean; font?: string }; para: Record<string, unknown>; inTable: boolean; style?: string; list?: string; listStyle?: string; checked?: boolean; link?: string } | null;
  pageSetup(): PageSetup;
  bookmarks(): Bookmark[];
  /** Everything a cross-reference can point at. */
  xrefTargets(): { headings: { p: number; text: string }[]; captions: { p: number; label: string; number: string; text: string }[] };
  /** Every block as the engine renders it (paragraphs have `hr` when they are a horizontal line). */
  renderAll(): { order: number[]; blocks: { t: string; id: number; hr?: boolean; len?: number; bookmarks?: string[] }[] };
  apply(ops: unknown): unknown;
  save(): Uint8Array;
  undo(): boolean;
  text(): string;
  textRange(a: { p: number; o: number }, b: { p: number; o: number }): string;
  addMedia(ext: string, bytes: Uint8Array): string;
  /** Every drawing object of the text (the Selection pane lists them). */
  objects(): { p: number; o: number; kind: string; name: string; id: number; hidden?: boolean; wrap: string; floating: boolean; z?: number; behind?: boolean; relX?: string; relY?: string; align?: string; alignY?: string; x?: number; y?: number }[];
  accessibility(): { kind: string; severity: string; p?: number; o?: number; message: string; detail?: string }[];
  docProps(): { title?: string; subject?: string; author?: string; keywords?: string; description?: string };
  fileName: string;
}
interface DocxtorWindow {
  docxtor: {
    /** The document the app opened. */
    engine: Engine | null;
    /** Make a document (from Markdown) in a place in the files panel and open it. */
    createIn(backend: unknown, dir: string, name?: string, markdown?: string): Promise<void>;
    docKey(): string | null;
    files: { library: { write(path: string, bytes: Uint8Array): Promise<void>; read(path: string): Promise<Uint8Array>; list(dir: string): Promise<{ name: string; path: string; kind: string }[]> } };
    fe: {
      /** The document on screen (differs from `engine` after `fe.open`). */
      editor: {
        engine: Engine | null;
        sel: { anchor: { p: number; o: number }; focus: { p: number; o: number } } | null;
        pages: number;
        zoom: number;
        exec(ops: unknown): unknown;
        /** Undo the last step, as Ctrl+Z does. */
        undo(): void;
        setZoom(zoom: number): void;
        /** The page (from 1) each paragraph starts on. */
        pageMap(): Record<string, number>;
        /** A paragraph inside section i, to address that section in ops. */
        sectionAnchor(i: number): number | undefined;
      };
      extensions: { menu(id: string): { title: string }[][]; execute(id: string, ...args: unknown[]): Promise<{ status: "done" | "declined" | "unavailable"; value?: unknown }> };
      open(bytes: Uint8Array, name?: string): Promise<unknown>;
    };
  };
}

// index.html asks Google Fonts for Inter and the look-alikes of Word's fonts. Tests must not
// depend on the network (or on which fonts the machine has), so the stylesheet is replaced by
// faces that exist but never load: Docxtor then sees the stand-ins as present, as it does online.
const STAND_INS = ["Inter", "Carlito", "Caladea", "Arimo", "Tinos", "Cousine", "Gelasio"];
const FONT_CSS = STAND_INS.map((f) => `@font-face{font-family:"${f}";src:url("data:font/woff2;base64,AAAA") format("woff2");unicode-range:U+10FFFF}`).join("\n");

// -- the watchdog (architecture review, rule U1) -------------------------------------------------
// Every test, with no change to the spec, fails on an uncaught page error, a console.error, or a
// page that stops answering. Tests that cause an error on purpose say so with `expectErrors`;
// real bugs the watchdog found are listed in known-issues.ts until they are fixed.

interface Watch {
  /** What was recorded: where it came from, the message, and (for page errors) the stack. */
  errors: { kind: "pageerror" | "console"; text: string; stack?: string }[];
  expected: { pattern: RegExp; hit: boolean }[];
  /** How many times the test has navigated the page (goto, reload, back, forward), and when it last did. */
  navs: number;
  navAt: number;
}
const watches = new WeakMap<Page, Watch>();

/** Whether an address is on the server the tests run against (Playwright's baseURL). */
function sameOrigin(url: string): boolean {
  try {
    return new URL(url).origin === `http://localhost:${process.env.DOCXTOR_E2E_PORT ?? 5180}`;
  } catch {
    return false;
  }
}

/** Console messages that are not bugs. Each one says why. */
const NOISE: ((m: { text: string; url: string }) => boolean)[] = [
  // The fixture aborts the font files (fonts.gstatic.com) on purpose; the browser logs the abort.
  (m) => /^Failed to load resource/.test(m.text) && /fonts\.gstatic\.com/.test(m.url),
  // A test that aborts or fails a request with page.route makes the browser log it; that is the
  // test's own doing (the request's own error is what the test asserts on).
  (m) => /^Failed to load resource: net::ERR_(FAILED|ABORTED|INTERNET_DISCONNECTED|BLOCKED_BY_CLIENT)/.test(m.text),
  // The browser logs every failed request to another site (a 404 from doi.org, a 503 from Wiktionary, Zotero's
  // local API refusing the connection). Docxtor handles those answers itself, and tests stub them on purpose; the
  // browser's log line is not an error in Docxtor. A failed request to Docxtor's own server still counts.
  (m) => /^Failed to load resource/.test(m.text) && !!m.url && !sameOrigin(m.url),
  // Firefox wraps its own log lines (network, security, fonts, CSS) as `[JavaScript Error: "…" {file: …}]` and Playwright reports
  // them as console errors. Each is judged by the line inside, and only the ones about a request to another site, which the
  // test stubbed or aborted on purpose, are noise (as the "Failed to load resource" lines above are in Chromium):
  //  - a request to another site that did not succeed (Firefox says "Cross-Origin Request Blocked … CORS request did not succeed")
  //  - a font file from fonts.gstatic.com, which the fixture aborts on purpose
  (m) => {
    const inner = firefoxLine(m.text);
    if (inner === null) return false;
    const cors = /^Cross-Origin Request Blocked: .* resource at (\S+?)\. \(Reason: CORS request did not succeed\)/.exec(inner);
    if (cors) return !sameOrigin(cors[1]);
    return /^downloadable font: download failed .* source: https:\/\/fonts\.gstatic\.com\//.test(inner);
  },
  // WebKit logs the SecurityError of reading a canvas that an SVG picture tainted (it taints on a foreignObject or a picture from outside).
  // The PDF writer (pdf/images.ts) catches it and puts a transparent dot where the picture was, so the file stays valid; the line is the
  // browser's note of an error that was handled, and only the writer's own call counts.
  (m) => /^Unable to get image data from canvas because the canvas has been tainted/.test(m.text) && /\/pdf\/images\.ts/.test(m.url),
  // WebKit logs a request to another site that did not succeed twice: "Could not connect to the server." (no address) and
  // "Fetch API cannot load <address> due to access control checks" (an address). Only the second names the site, so the first
  // is accepted when the test's page made such a request; a failed request to Docxtor's own server never matches either.
  (m) => /^Could not connect to the server\.?$/.test(m.text) && !sameOrigin(m.url || "http://other.invalid/"),
];

/** Text inside Firefox's `[JavaScript Error: "…" {file: "…" line: n}]` wrapper, or null when the text is not one. */
function firefoxLine(text: string): string | null {
  const m = /^\[JavaScript Error: "([\s\S]*?)"(?: \{file: [^\]]*\})?\]/.exec(text);
  return m ? m[1] : null;
}

/**
 * What a browser says of a load that a navigation cut off. A test that goes to another address while the page is still fetching (a
 * dictionary, the engine, a module) makes WebKit report each cut-off fetch as an error ("Fetch API cannot load … due to access control
 * checks", "Importing a module script failed", "Load failed") and Firefox as NS_BINDING_ABORTED. They count only in the seconds after a
 * second navigation of the same page; a load that fails at any other time is still a failure.
 */
const ABORTED_LOAD: RegExp[] = [
  /^(?:[^:\n]*: )?Fetch API cannot load .+ due to access control checks/,
  /^Cannot load \S+ due to access control checks/,
  /^(?:[^:\n]*: )?Importing a module script failed/,
  /^Docxtor could not start: TypeError: Load failed/,
  /^NS_BINDING_ABORTED/,
];
const abortedLoad = (w: Watch, text: string) => w.navs >= 2 && Date.now() - w.navAt < 5000 && ABORTED_LOAD.some((re) => re.test(firefoxLine(text) ?? text));

/** Page errors (uncaught, not console) that are the browser's own notice and not a failure of the page. */
const PAGE_NOISE: ((text: string) => boolean)[] = [
  // Safari reports a resize loop that the browser finished in the next frame as an error event; the specification calls it benign,
  // Docxtor's own handler ignores it too (launch.spec.ts), and no layout is wrong because of it.
  (t) => /ResizeObserver loop completed with undelivered notifications/.test(t),
  // WebKit makes a page error of a request to another site that did not succeed, naming the address ("Fetch API cannot load http: /host/path
  // due to access control checks"): the page's own code handled the rejected fetch. A request to Docxtor's own server never matches.
  (t) => {
    const m = /^(?:[^:\n]*: )?Fetch API cannot load (https?): ?\/?\/?(\S+) due to access control checks/.exec(t);
    return !!m && !sameOrigin(`${m[1]}://${m[2]}`);
  },
];

/**
 * Mark errors as expected for this test: a test that causes an error on purpose lists what it
 * expects (matched against the message, and for a page error its stack). An expected error that
 * never happens fails the test, so the list cannot go stale.
 */
export function expectErrors(page: Page, ...patterns: RegExp[]) {
  const w = watches.get(page);
  if (!w) throw new Error("expectErrors: this page is not watched (use the `page` of the test)");
  for (const pattern of patterns) w.expected.push({ pattern, hit: false });
}

const HEARTBEAT_MS = 5000;

/**
 * Why a test runs in Chromium only. A test marked `{ annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }`
 * (or a whole file with `chromiumOnlyFile(...)`) is skipped, with that reason, on Firefox and WebKit; see docs/browsers.md.
 */
export const CHROMIUM_ONLY = {
  clipboard: "the test reads or writes the system clipboard from the page (navigator.clipboard), which only Chromium lets a test allow (clipboard-read, clipboard-write)",
  cdp: "the test drives Chromium's DevTools protocol (a CDP session: input-method events, touch gestures, accessibility tree), which Firefox and WebKit don't have",
  columns: "the test lays a section out in two or more columns, which needs CSS column-wrap and column-height (Chromium only); elsewhere the section shows as one column (COLUMNS in editor/layout.ts, docs/browsers.md)",
  mathml: "the test reads where the browser's own MathML layout puts an equation's bars and whether it stretches them (the PDF copy follows what the browser drew): Firefox stretches a bar or a fence where Chromium does not, and Safari sets a fraction's bar lower than the math axis",
  memory: "the test reads Chromium's own memory and performance counters (CDP, performance.memory)",
} as const;

/** Skip every test of the file outside Chromium, with the reason. */
export function chromiumOnlyFile(reason: string) {
  base.beforeEach(({ browserName }) => {
    base.skip(browserName !== "chromium", `Chromium only: ${reason}`);
  });
}

/**
 * The keys for the start and the end of the line, as the computer's own convention has them. On a Mac, Home and End scroll the page in
 * Safari and Firefox and leave the caret where it is (Chrome moves it, which is why tests that wrote Home passed there only); the Mac's
 * key for the line is Command with the arrow. Elsewhere Home and End are the keys. Use these where a test means "the line", and
 * ControlOrMeta+Home and ControlOrMeta+End where it means the document.
 */
export const LINE_START = process.platform === "darwin" ? "Meta+ArrowLeft" : "Home";
export const LINE_END = process.platform === "darwin" ? "Meta+ArrowRight" : "End";
export const SELECT_TO_LINE_START = `Shift+${LINE_START}`;
export const SELECT_TO_LINE_END = `Shift+${LINE_END}`;

/**
 * The key that moves the focus to the next control. Safari's default, "Press Tab to highlight each item on a webpage", is off: Tab then
 * visits text fields and menus only and passes buttons and links, and Option+Tab visits everything (Playwright's WebKit does the same).
 * Tests that walk every control by keyboard press Option+Tab in WebKit and Tab elsewhere. Firefox on a Mac visits buttons with Tab.
 */
export function tabKey(page: Page, shift = false): string {
  const webkit = page.context().browser()?.browserType().name() === "webkit";
  return `${shift ? "Shift+" : ""}${webkit ? "Alt+" : ""}Tab`.replace("Shift+Alt+", "Alt+Shift+");
}

export const test = base.extend({
  // A test annotated "chromium-only" is skipped on the other browsers, and says why.
  chromiumOnly: [
    async ({ browserName }, use, testInfo) => {
      const mark = testInfo.annotations.find((a) => a.type === "chromium-only");
      testInfo.skip(!!mark && browserName !== "chromium", `Chromium only: ${mark?.description ?? ""}`);
      await use();
    },
    { auto: true },
  ],
  page: async ({ page, context }, use, testInfo) => {
    const watch: Watch = { errors: [], expected: [], navs: 0, navAt: 0 };
    watches.set(page, watch);
    for (const name of ["goto", "reload", "goBack", "goForward"] as const) {
      const original = (page[name] as (...a: unknown[]) => Promise<unknown>).bind(page);
      (page as unknown as Record<string, unknown>)[name] = (...a: unknown[]) => {
        watch.navs++;
        watch.navAt = Date.now();
        return original(...a);
      };
    }
    const attach = (pg: Page) => {
      pg.on("pageerror", (e) => PAGE_NOISE.some((n) => n(`${e.name}: ${e.message}`)) || abortedLoad(watch, `${e.name}: ${e.message}`) || watch.errors.push({ kind: "pageerror", text: `${e.name}: ${e.message}`, stack: e.stack }));
      pg.on("console", (m) => {
        if (m.type() !== "error") return;
        const text = m.text();
        const url = m.location().url ?? "";
        if (NOISE.some((n) => n({ text, url })) || abortedLoad(watch, text)) return;
        watch.errors.push({ kind: "console", text: url ? `${text} (${url})` : text });
      });
    };
    attach(page);
    // popups and extra pages of the same browser context
    const onPage = (pg: Page) => { if (pg !== page) attach(pg); };
    context.on("page", onPage);
    // Outside Chrome and Edge the app says once per session that it is made for them (checkBrowser in web/src/failsafe.ts); that notice would
    // sit in every test's way (a test that counts the toasts, or waits for the one a command raises), so the session starts having heard it.
    await page.addInitScript(() => { try { sessionStorage.setItem("docxtor-browser-warned", "1"); } catch { /* no session storage */ } });
    // the engine's own invariant checks (the engine reads this flag)
    await page.addInitScript(() => { (globalThis as unknown as { __DOCXTOR_CHECKS__: boolean }).__DOCXTOR_CHECKS__ = true; });
    // The paginator's own check: every incremental layout is compared with a full one made in place (see Paginator.verify).
    await page.addInitScript(() => { (globalThis as unknown as { __DOCXTOR_PAGINATION_CHECKS__: boolean }).__DOCXTOR_PAGINATION_CHECKS__ = true; });
    // DOCXTOR_FAR=n runs any spec with far pages left out of the layout, keeping n pages either side of the view (see far.ts).
    if (process.env.DOCXTOR_FAR !== undefined && process.env.DOCXTOR_FAR !== "") {
      const keep = Number(process.env.DOCXTOR_FAR);
      await page.addInitScript((n) => { (globalThis as unknown as { __DOCXTOR_FAR_PAGES__: number }).__DOCXTOR_FAR_PAGES__ = n; }, keep);
    }
    await page.route("https://fonts.googleapis.com/**", (route) => route.fulfill({ contentType: "text/css", body: FONT_CSS }));
    await page.route("https://fonts.gstatic.com/**", (route) => route.abort());
    await use(page);
    context.off("page", onPage);

    // does the page still answer?
    let frozen = false;
    if (!page.isClosed()) {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const answered = await Promise.race([
        page.evaluate(() => 1).then(() => true, () => true), // a page that closed meanwhile is not frozen
        new Promise<boolean>((resolve) => { timer = setTimeout(() => resolve(false), HEARTBEAT_MS); }),
      ]);
      clearTimeout(timer);
      frozen = !answered;
    }

    // sort what was recorded: expected (by this test), known (known-issues.ts), real
    const id = [testInfo.file.split("/").pop(), ...testInfo.titlePath.slice(1)].join(" › ");
    const known = KNOWN_ISSUES.filter((k) => k.test === id);
    const real = watch.errors.filter((e) => {
      const hay = `${e.text}\n${e.stack ?? ""}`;
      const exp = watch.expected.filter((x) => x.pattern.test(hay));
      for (const x of exp) x.hit = true;
      if (exp.length) return false;
      return !known.some((k) => k.pattern.test(hay));
    });
    const stale = watch.expected.filter((x) => !x.hit);

    const parts: string[] = [];
    if (frozen) parts.push(`The page stopped answering (frozen): page.evaluate(() => 1) did not resolve within ${HEARTBEAT_MS / 1000} seconds.`);
    const invariants = real.filter((e) => e.text.includes("Docxtor invariant broken:"));
    const others = real.filter((e) => !invariants.includes(e));
    const list = (es: Watch["errors"]) =>
      es.map((e) => (e.kind === "pageerror" ? `  - uncaught error: ${e.text}\n${(e.stack ?? "").split("\n").slice(1, 4).join("\n")}` : `  - console.error: ${e.text}`)).join("\n");
    if (invariants.length) parts.push(`The engine's invariant checks failed (${invariants.length}):\n${list(invariants)}`);
    if (others.length) parts.push(`The page reported errors (${others.length}):\n${list(others)}`);
    if (stale.length) parts.push(`expectErrors listed errors that never happened: ${stale.map((x) => x.pattern).join(", ")}`);
    if (parts.length) throw new Error(parts.join("\n\n"));
  },
});
export { expect };
export type { Locator, Page };

/** Paragraph ids in fixtures/rich.docx. */
export const RICH = {
  title: 1,
  intro: 2,
  highlights: 3,
  plan: 8,
  quote: 12,
  /** First cell of the table (a "Metric" header cell). */
  cell: 16,
  image: 28,
  last: 32,
} as const;

/**
 * Open a fixture from fixtures/ in a fresh Library (each test has its own browser profile) and wait
 * until it is on screen: laid out, saved, scored. `query` is appended to the URL, e.g. "&lang=es".
 */
export async function openFixture(page: Page, name = "rich.docx", query = "") {
  await page.goto(`/?open=${name}${query}`);
  await documentReady(page);
}

/** Wait for the open document to be laid out and its status readouts to settle. */
export async function documentReady(page: Page) {
  await expect(page.locator(".flow .fp").first()).toBeVisible();
  await page.waitForFunction(() => !!(window as unknown as DocxtorWindow).docxtor?.engine);
  await expect(page.locator(".statusbar .fidelity .readout")).not.toBeEmpty();
  await page.evaluate(() => document.fonts.ready);
}

/**
 * Bring the n-th paragraph (by its place in the text) into view. With far pages left out (the DOCXTOR_FAR switch) a paragraph far from the view has no
 * layout box, so a test that clicks one scrolls to it first, as a person does.
 */
export async function showPara(page: Page, n: number) {
  await page.evaluate((i) => {
    const w = window as unknown as { docxtor: { fe: { editor: { scrollParaIntoView(p: number): void } } } };
    const el = document.querySelectorAll<HTMLElement>(".flow [data-p]")[i];
    if (el) w.docxtor.fe.editor.scrollParaIntoView(Number(el.dataset.p));
  }, n);
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

/** As `showPara`, for a paragraph found by a locator. */
export async function reveal(locator: Locator) {
  await locator.evaluate(async (el) => {
    const w = window as unknown as { docxtor: { fe: { editor: { scrollParaIntoView(p: number): void } } } };
    const p = (el.closest("[data-p]") as HTMLElement | null)?.dataset.p;
    if (p) w.docxtor.fe.editor.scrollParaIntoView(Number(p));
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  });
}

/** The paragraph with this model id. */
export const para = (page: Page, id: number): Locator => page.locator(`.flow [data-p="${id}"]`);

/** The text of a paragraph as the model has it (list labels and page-split spacers left out). */
export const paraText = (page: Page, id: number): Promise<string> =>
  para(page, id).evaluate((el) => {
    const copy = el.cloneNode(true) as HTMLElement;
    copy.querySelectorAll(".lbl, .pgsp, .ghost").forEach((n) => n.remove());
    return copy.textContent ?? "";
  });

/** Click past the last word of a one-line paragraph, which puts the caret at its end. */
export async function caretAtEnd(page: Page, id: number) {
  const p = para(page, id);
  await p.scrollIntoViewIfNeeded();
  const box = (await p.boundingBox())!;
  await p.click({ position: { x: box.width - 2, y: Math.min(8, box.height / 2) } });
}

/** The screen position of the middle of `word` inside a paragraph (after scrolling it into view). */
export async function wordPoint(page: Page, id: number, word: string): Promise<{ x: number; y: number }> {
  const p = para(page, id);
  await p.scrollIntoViewIfNeeded();
  const point = await p.evaluate((el, w) => {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      const i = (n as Text).data.indexOf(w);
      if (i < 0) continue;
      const range = document.createRange();
      range.setStart(n, i);
      range.setEnd(n, i + w.length);
      const r = range.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    }
    return null;
  }, word);
  if (!point) throw new Error(`"${word}" is not in paragraph ${id}`);
  return point;
}

/** Select a word with the mouse (double-click). */
export async function selectWord(page: Page, id: number, word: string) {
  const { x, y } = await wordPoint(page, id, word);
  await page.mouse.dblclick(x, y);
}

/** Run something against the app in the page (setup and model assertions only). */
export const inPage = <R, A = undefined>(page: Page, fn: (w: DocxtorWindow["docxtor"], arg: A) => R | Promise<R>, arg?: A): Promise<R> =>
  page.evaluate(([src, a]) => new Function("w", "a", `return (${src})(window.docxtor, a)`)(src, a) as R, [fn.toString(), arg] as const) as Promise<R>;

/** The bold/italic/font of the character at a position, from the document model. */
export const runAt = (page: Page, p: number, o: number) => inPage(page, (w, pos) => w.engine!.formatAt(pos)?.run, { p, o });

// -- menu bar ------------------------------------------------------------------------------------

export const menuBar = (page: Page) => page.getByRole("menubar", { name: "Menu bar" });
export const topMenu = (page: Page, name: string) => menuBar(page).getByRole("menuitem", { name, exact: true });

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** A line of an open menu (or submenu) by its label. */
export const menuItem = (page: Page, label: string | RegExp): Locator =>
  page
    .locator('.menu-drop [role="menuitem"], .menu-drop [role="menuitemcheckbox"]')
    .filter({ has: page.locator(".menu-label").filter({ hasText: typeof label === "string" ? new RegExp(`^${escape(label)}$`) : label }) });

/** Open a top-level menu with the mouse. */
export async function openMenu(page: Page, name: string) {
  await topMenu(page, name).click();
  await expect(topMenu(page, name)).toHaveAttribute("aria-expanded", "true");
}

/** Choose a command by its path, e.g. `chooseMenu(page, "Format", "Text", "Bold")`. */
export async function chooseMenu(page: Page, top: string, ...path: string[]) {
  await openMenu(page, top);
  for (const label of path) await menuItem(page, label).click();
  await expect(page.locator(".menu-drop")).toHaveCount(0);
}

// -- right-click menu ----------------------------------------------------------------------------

export const contextMenu = (page: Page) => page.locator(".ctx-menu");
/** A line of the right-click menu by its label (the whole label, for a string). */
export const contextItem = (page: Page, label: string | RegExp): Locator =>
  contextMenu(page)
    .getByRole("menuitem")
    .filter({ has: page.locator(".menu-label").filter({ hasText: typeof label === "string" ? new RegExp(`^${escape(label)}$`) : label }) });

/** The page's dialogs (word count, details, page setup, replace fonts). */
export const dialog = (page: Page, name: string | RegExp) => page.getByRole("dialog", { name });

/** The text of a part (`word/document.xml`) of the document on screen as it would be saved, or "" when the saved file has no such part. */
export const savedPart = (page: Page, name: string): Promise<string> =>
  inPage(
    page,
    async (w, part) => {
      const bytes = w.fe.editor.engine!.save();
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      const dec = new TextDecoder();
      let at = 0;
      while (at + 30 < bytes.length && view.getUint32(at, true) === 0x04034b50) {
        const method = view.getUint16(at + 8, true);
        const csize = view.getUint32(at + 18, true);
        const nlen = view.getUint16(at + 26, true);
        const elen = view.getUint16(at + 28, true);
        const fname = dec.decode(bytes.subarray(at + 30, at + 30 + nlen));
        const start = at + 30 + nlen + elen;
        if (fname === part) {
          const data = bytes.subarray(start, start + csize);
          if (method === 0) return dec.decode(data);
          const out = new Response(new Blob([data as unknown as BlobPart]).stream().pipeThrough(new DecompressionStream("deflate-raw")));
          return dec.decode(await out.arrayBuffer());
        }
        at = start + csize;
      }
      return "";
    },
    name,
  );

/** Contrast ratio of the colour of each element's text against the first opaque background behind it. */
export async function contrasts(page: Page, selectors: string[]): Promise<{ selector: string; ratio: number }[]> {
  return page.evaluate((list) => {
    const parse = (c: string): [number, number, number, number] => {
      const m = c.match(/color\(srgb ([\d.e-]+) ([\d.e-]+) ([\d.e-]+)(?: \/ ([\d.e-]+))?\)/);
      if (m) return [Number(m[1]) * 255, Number(m[2]) * 255, Number(m[3]) * 255, m[4] === undefined ? 1 : Number(m[4])];
      const n = c.match(/rgba?\(([^)]+)\)/)!;
      const p = n[1].split(/[ ,/]+/).filter(Boolean).map(Number);
      return [p[0], p[1], p[2], p[3] === undefined ? 1 : p[3]];
    };
    const lum = ([r, g, b]: number[]) => {
      const f = (v: number) => {
        const s = v / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
    };
    const over = (top: number[], bottom: number[]) => {
      const a = top[3];
      return [top[0] * a + bottom[0] * (1 - a), top[1] * a + bottom[1] * (1 - a), top[2] * a + bottom[2] * (1 - a), 1];
    };
    const background = (el: Element): number[] => {
      const layers: number[][] = [];
      for (let e: Element | null = el; e; e = e.parentElement) {
        const c = parse(getComputedStyle(e).backgroundColor);
        if (c[3] > 0) layers.push(c);
        if (c[3] === 1) break;
      }
      let base = [255, 255, 255, 1];
      for (const l of layers.reverse()) base = over(l, base);
      return base;
    };
    return list.map((selector) => {
      const el = document.querySelector(selector);
      if (!el) return { selector, ratio: 0 };
      const fg = over(parse(getComputedStyle(el).color), background(el));
      const bg = background(el);
      const [a, b] = [lum(fg), lum(bg)];
      return { selector, ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) };
    });
  }, selectors);
}

/** Answer the question Docxtor asks before a risky action (builtins/cautions.ts, web/src/cautions.ts) with the button named `button`. */
export async function answer(page: Page, button: string) {
  const q = page.getByRole("alertdialog");
  await expect(q).toBeVisible();
  await q.getByRole("button", { name: button, exact: true }).click();
  await expect(q).toHaveCount(0);
}

// -- the checks after an action (architecture review, section 2, rules U1 to U6, U2, U9 and U10) --------
// `checkApp` is what the matrix (matrix.spec.ts) calls after every action; any test may call it too.

/** What the watchdog recorded since the last call, and forget it (the matrix reports errors for each action, not at the end of the test). */
export function takeErrors(page: Page): { kind: "pageerror" | "console"; text: string; stack?: string }[] {
  return watches.get(page)?.errors.splice(0) ?? [];
}

/** Remember the open document and its revision, so `checkApp` can tell a change of the document from the opening of another. */
export async function markBefore(page: Page): Promise<number | undefined> {
  return page.evaluate(() => {
    const ed = (window as unknown as { docxtor?: { fe?: { editor: { engine: { revision: number } | null } } } }).docxtor?.fe?.editor;
    const g = window as unknown as { __checkBefore?: { engine: unknown; revision: number } | null };
    g.__checkBefore = ed?.engine ? { engine: ed.engine, revision: ed.engine.revision } : null;
    return ed?.engine?.revision;
  });
}

export interface CheckOptions {
  /** The engine's revision before the action (from `markBefore`), to check an offer of Undo (U10) and, with `locked`, that nothing changed (U2). */
  revisionBefore?: number;
  /** Whether the document was read-only while the action ran: the revision must be the same (U2). */
  locked?: boolean;
  /** Nothing of the action is still open (a dialog, a menu): the keyboard focus must be in the document or on a control (U6). */
  settled?: boolean;
}

/**
 * The rules that must hold in the browser after any action: no uncaught error, no console.error and an answering page
 * (U1); while a lock holds the revision does not change (U2); the caret is in a visible block of every pane and nothing
 * of a hidden tab is visible (U5); focus is in the document, not on the page body, once a popup has closed (U6); an
 * offer of Undo means the revision went up (U10). Returns what is wrong, one line each; empty when all is well.
 */
export async function checkApp(page: Page, opts: CheckOptions = {}): Promise<string[]> {
  const problems: string[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;
  const answered = await Promise.race([
    page.evaluate(() => 1).then(() => true, () => true),
    new Promise<boolean>((resolve) => { timer = setTimeout(() => resolve(false), HEARTBEAT_MS); }),
  ]);
  clearTimeout(timer);
  if (!answered) return [`U1: the page stopped answering for ${HEARTBEAT_MS / 1000} seconds`];
  if (page.isClosed()) return [];
  for (const e of takeErrors(page)) problems.push(`U1: ${e.kind === "pageerror" ? "uncaught error" : "console.error"}: ${e.text.split("\n")[0].slice(0, 240)}`);
  const found = await page.evaluate((o) => {
    const out: string[] = [];
    const w = window as unknown as { docxtor?: { fe?: { editor: Pane; split?: { on: boolean; editor: Pane } } } };
    interface Pane {
      root: HTMLElement;
      sel: { anchor: { p: number }; focus: { p: number } } | null;
      engine: { revision: number } | null;
    }
    const fe = w.docxtor?.fe;
    if (!fe) return out;
    const panes: Pane[] = [fe.editor];
    if (fe.split?.on && fe.split.editor && fe.split.editor !== fe.editor) panes.push(fe.split.editor);
    const rev = fe.editor.engine?.revision;
    // Another document opened by the action is not a change of this one.
    const before = (window as unknown as { __checkBefore?: { engine: unknown } | null }).__checkBefore;
    const same = !before || before.engine === fe.editor.engine;
    if (o.locked && same && o.revisionBefore !== undefined && rev !== undefined && rev !== o.revisionBefore) out.push(`U2: the revision changed from ${o.revisionBefore} to ${rev} while the document was read-only`);
    panes.forEach((pane, i) => {
      const name = panes.length > 1 ? `pane ${i + 1}` : "the document";
      for (const el of pane.root.querySelectorAll<HTMLElement>("[data-tab-hidden]")) {
        if (getComputedStyle(el).display !== "none") {
          out.push(`U5: a block of a hidden tab is visible in ${name}`);
          break;
        }
      }
      if (pane.sel) {
        for (const end of ["anchor", "focus"] as const) {
          const el = pane.root.querySelector(`[data-p="${pane.sel[end].p}"]`);
          if (!el) out.push(`U5: the ${end} of the selection in ${name} is in paragraph ${pane.sel[end].p}, which is not drawn`);
          else if (el.closest("[data-tab-hidden]")) out.push(`U5: the ${end} of the selection in ${name} is in a hidden tab`);
        }
      }
    });
    if (o.settled) {
      const open = document.querySelector('[role="dialog"], [role="alertdialog"], .menu-drop, .ctx-menu, .popover, .qp-item');
      const active = document.activeElement;
      if (!open && (!active || active === document.body)) out.push("U6: after the popup closed, the keyboard focus is on the page body");
    }
    if (o.revisionBefore !== undefined && same && rev !== undefined && rev <= o.revisionBefore && [...document.querySelectorAll(".toast button")].some((b) => /^undo$/i.test(b.textContent ?? ""))) out.push("U10: an offer to undo is showing, but the revision did not go up");
    return out;
  }, { revisionBefore: opts.revisionBefore, locked: opts.locked, settled: opts.settled });
  return [...problems, ...found];
}

// A stand-in for Anthropic's Messages API, for the assistant's Anthropic provider (ai-anthropic.spec.ts, production.spec.ts).
const ANTHROPIC_CORS = { "access-control-allow-origin": "*", "access-control-allow-headers": "*", "access-control-allow-methods": "POST, OPTIONS" };

/** One text block, streamed as the Messages API streams it. */
function anthropicStream(text: string): string {
  const ev = (type: string, data: object) => `event: ${type}\ndata: ${JSON.stringify({ type, ...data })}\n\n`;
  return [
    ev("message_start", {
      message: { id: "msg_test", type: "message", role: "assistant", model: "claude-opus-5-5", content: [], stop_reason: null, stop_sequence: null, usage: { input_tokens: 10, output_tokens: 0 } },
    }),
    ev("content_block_start", { index: 0, content_block: { type: "text", text: "" } }),
    ev("content_block_delta", { index: 0, delta: { type: "text_delta", text } }),
    ev("content_block_stop", { index: 0 }),
    ev("message_delta", { delta: { stop_reason: "end_turn", stop_sequence: null }, usage: { output_tokens: 5 } }),
    ev("message_stop", {}),
  ].join("");
}

/** Answer the assistant's requests to `base` with `text`; returns the headers and bodies of the requests made. */
export async function fakeAnthropic(page: Page, base: string, text: string) {
  const seen: { headers: Record<string, string>; body: Record<string, unknown> }[] = [];
  await page.route(`${base}/**`, async (route) => {
    const req = route.request();
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers: ANTHROPIC_CORS });
    seen.push({ headers: await req.allHeaders(), body: req.postDataJSON() });
    await route.fulfill({ status: 200, headers: { ...ANTHROPIC_CORS, "content-type": "text/event-stream" }, body: anthropicStream(text) });
  });
  return seen;
}

/**
 * Paste `formats` (type to data) where `target` is, as a person's paste would arrive. In Chromium a paste event made by a script
 * carries what it is given; Firefox gives such an event an empty clipboard, whatever it carries, so there the formats go through
 * the browser's own clipboard (each headless browser has one of its own, so parallel tests don't meet): the next copy writes them,
 * ahead of the page's own copy handler, and a real Ctrl+V pastes them into the focused `target`.
 */
export async function pasteData(page: Page, formats: Record<string, string>, target = ".flow") {
  if (page.context().browser()?.browserType().name() === "chromium") {
    await page.evaluate(
      ([f, sel]) => {
        const dt = new DataTransfer();
        for (const [type, data] of Object.entries(f)) dt.setData(type, data);
        document.querySelector<HTMLElement>(sel)!.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
      },
      [formats, target] as const,
    );
    return;
  }
  // Firefox's headless clipboard on Linux keeps only plain text and HTML: any other format (RTF, Docxtor's own) never arrives.
  const textAndHtmlOnly = Object.keys(formats).every((t) => t === "text/plain" || t === "text/html");
  base.skip(process.platform === "linux" && page.context().browser()?.browserType().name() === "firefox" && !textAndHtmlOnly, "Firefox's headless clipboard on Linux keeps only plain text and HTML");
  await page.evaluate(
    ([f, sel]) => {
      window.addEventListener(
        "copy",
        (e) => {
          for (const [type, data] of Object.entries(f)) e.clipboardData!.setData(type, data);
          e.preventDefault();
          e.stopImmediatePropagation();
        },
        { capture: true, once: true },
      );
      const el = document.querySelector<HTMLElement>(sel)!;
      if (!el.contains(document.activeElement)) el.focus();
    },
    [formats, target] as const,
  );
  await page.keyboard.press("ControlOrMeta+c");
  await page.keyboard.press("ControlOrMeta+v");
}
