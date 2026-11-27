// The random clicker (architecture review, section 3.7). Seeded and replayable: at each step it picks, from the app as
// it is on screen, one visible enabled control (a menu bar item or a submenu line, a toolbar button, a context menu
// line, a button, checkbox, select or tab inside an open dialog, popover or strip, a navigation pane item), a place in
// the text (also in a table cell, and the header or footer by double-click), a key or chord, or a short word to type
// (sometimes with emoji or right-to-left text). Typing and clicks in the text are weighted up so the document changes.
// After each step: the page answers and nothing was reported (`takeErrors`: no uncaught error, no console.error, and
// the engine's own invariant check, which logs "Docxtor invariant broken"). Every 25 steps and at the end: `checkApp`.
//
// Off by default (a 200-step run takes about a minute). Turn it on with DOCXTOR_MONKEY=1. Knobs:
//   DOCXTOR_MONKEY_SEED=7,8      seeds (default 1,2,3)
//   DOCXTOR_MONKEY_STEPS=200     steps per run
//   DOCXTOR_MONKEY_DOC=mix       starting document: a fixture name (rich.docx), "mix" (a built document with tabs, a
//                              table, a list, a header, a footnote and a content control), or both by default
//   DOCXTOR_MONKEY_SHRINK=40     most replays spent shrinking a failure (0 turns shrinking off)
//   DOCXTOR_MONKEY_PLANT=F      (a self-test: the page logs an error when that key is pressed, to see it found and shrunk)
//   DOCXTOR_MONKEY_REPLAY=<file> run the steps of a saved failure (monkey-results/<seed>-<doc>.json) exactly
// A failure writes (to monkey-results/, not test-results/, which Playwright empties at the start of every run, replays included) the file with the seed, the steps up to the failure and the error, and prints the replay command.
// Real bugs that are not fixed yet go in monkey-known.ts.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { checkApp, inPage, openFixture, takeErrors, test, type Page, CHROMIUM_ONLY, chromiumOnlyFile } from "./helpers";
import { MONKEY_KNOWN } from "./support/monkey-known";
import { openBase } from "./support/matrix-modes";

chromiumOnlyFile(CHROMIUM_ONLY.cdp);

/* eslint-disable @typescript-eslint/no-explicit-any */
const ON = !!(process.env.DOCXTOR_MONKEY || process.env.DOCXTOR_MONKEY_REPLAY);
const SEEDS = (process.env.DOCXTOR_MONKEY_SEED ?? "1,2,3").split(",").map((s) => Number(s.trim())).filter((n) => Number.isFinite(n));
const STEPS = Number(process.env.DOCXTOR_MONKEY_STEPS ?? 200);
const DOCS = (process.env.DOCXTOR_MONKEY_DOC ?? "rich.docx,mix").split(",").map((s) => s.trim()).filter(Boolean);
const SHRINK_TRIES = Number(process.env.DOCXTOR_MONKEY_SHRINK ?? 40);
const REPLAY = process.env.DOCXTOR_MONKEY_REPLAY;
const CHECK_EVERY = 25;
const STEP_LIMIT_MS = 20_000;

// -- the steps: small JSON records, so a run can be saved, replayed and shrunk -------------------------------------

type Step =
  /** Click a control by its path in the page (see `controls` below). */
  | { t: "click"; id: string; label: string }
  /** Choose an option of a select. */
  | { t: "select"; id: string; label: string; index: number }
  /** Replace the text of an input. */
  | { t: "fill"; id: string; label: string; text: string }
  /** Click in the n-th visible paragraph (also those of table cells) at a fraction of its width; `dbl` double-clicks, `right` right-clicks. */
  | { t: "text"; n: number; fx: number; fy: number; mode: "click" | "dbl" | "right" | "shift" }
  /** Double-click the top or bottom margin of the first page: the header or footer. */
  | { t: "margin"; zone: "header" | "footer" }
  | { t: "key"; key: string }
  | { t: "type"; text: string };

const label = (s: Step): string => {
  switch (s.t) {
    case "click": return `click ${s.label}`;
    case "select": return `choose option ${s.index} of ${s.label}`;
    case "fill": return `fill ${s.label} with "${s.text}"`;
    case "text": return `${s.mode} in paragraph ${s.n} at ${s.fx.toFixed(2)}`;
    case "margin": return `double-click the ${s.zone} margin`;
    case "key": return `press ${s.key}`;
    case "type": return `type "${s.text}"`;
  }
};

// -- the seeded random numbers -------------------------------------------------------------------------------------

function rng(seed: number) {
  let a = (seed * 2654435761) >>> 0;
  const next = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return { next, int: (n: number) => Math.floor(next() * n), pick: <T,>(xs: T[]) => xs[Math.floor(next() * xs.length)] };
}

const KEYS = [
  "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "Enter", "Backspace", "Delete", "Tab", "Escape",
  "Shift+ArrowLeft", "Shift+ArrowRight", "Shift+ArrowUp", "Shift+ArrowDown", "Shift+Tab",
  "ControlOrMeta+z", "ControlOrMeta+y", "ControlOrMeta+a", "ControlOrMeta+b", "ControlOrMeta+i", "ControlOrMeta+c", "ControlOrMeta+v", "ControlOrMeta+x",
];
const WORDS = ["hello", "world ", "Docxtor", "a", "12 34", "tab\tstop", "résumé", "naïve café", "emoji 😀🎉", "👩‍👩‍👧 family", "שלום עולם", "مرحبا بالعالم", "日本語のテキスト", "x y z", "...", "​zero"];

// -- what is on screen, read in the page ---------------------------------------------------------------------------
// Installed before the app loads. `controls()` lists what a person could click right now, sorted by a path of the
// element in the page (its ancestors with their position among their siblings, and its name), which does not depend
// on timing, so the same seed meets the same list.

function installProbe() {
  const w = window as any;
  const POPUPS = ".menu-drop, .ctx-menu, .popover, .toast-menu, .qp-list, .qp";
  const CONTROL =
    'button, [role="button"], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"], [role="tab"], [role="option"], [role="switch"], [role="treeitem"], ' +
    'input[type="checkbox"], input[type="radio"], input[type="text"], input[type="number"], input[type="search"], input:not([type]), textarea, select, summary, .qp-item';
  const shown = (el: Element): boolean => {
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return false;
    const cs = getComputedStyle(el);
    if (cs.visibility === "hidden" || cs.display === "none" || cs.pointerEvents === "none") return false;
    if (el.closest('[hidden], [inert], [aria-hidden="true"]')) return false;
    for (let p: Element | null = el; p; p = p.parentElement) if (getComputedStyle(p).display === "none") return false;
    return true;
  };
  const nameOf = (el: Element): string =>
    ((el.getAttribute("aria-label") || (el as HTMLElement).innerText || el.textContent || el.getAttribute("title") || (el as HTMLInputElement).placeholder || "") as string).replace(/\s+/g, " ").trim().slice(0, 40);
  const pathOf = (el: Element): string => {
    const parts: string[] = [];
    for (let e: Element | null = el; e && e !== document.body; e = e.parentElement) {
      const sibs = e.parentElement ? [...e.parentElement.children].filter((c) => c.tagName === e!.tagName) : [e];
      const cls = (e.getAttribute("class") ?? "").split(/\s+/)[0];
      parts.unshift(`${e.tagName.toLowerCase()}${cls ? "." + cls : ""}${sibs.length > 1 ? ":" + sibs.indexOf(e) : ""}`);
    }
    return parts.join(">");
  };
  const scope = (): { roots: Element[]; modal: boolean } => {
    const dialogs = [...document.querySelectorAll('[role="dialog"], [role="alertdialog"]')].filter(shown);
    if (dialogs.length) return { roots: [dialogs[dialogs.length - 1]], modal: true };
    const popups = [...document.querySelectorAll(POPUPS)].filter(shown);
    if (popups.length) return { roots: popups, modal: false };
    return { roots: [document.body], modal: false };
  };
  const hit = (el: Element): boolean => {
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2, y = r.top + r.height / 2;
    if (x < 0 || y < 0 || x > innerWidth || y > innerHeight) return false;
    const top = document.elementFromPoint(x, y);
    return !!top && (top === el || el.contains(top) || top.contains(el));
  };
  const usable = (el: Element): boolean => {
    if ((el as HTMLButtonElement).disabled || el.getAttribute("aria-disabled") === "true") return false;
    if (el.closest(".flow")) return false; // the document itself is reached by clicking in the text
    if (el instanceof HTMLInputElement && el.type === "file") return false;
    return shown(el);
  };
  w.__monkey = {
    state() {
      const s = scope();
      return { modal: s.modal, popup: !s.modal && s.roots[0] !== document.body };
    },
    controls() {
      const s = scope();
      const seen = new Set<Element>();
      const out: { id: string; label: string; kind: string; options?: number }[] = [];
      for (const root of s.roots) {
        for (const el of root.querySelectorAll(CONTROL)) {
          if (seen.has(el)) continue;
          seen.add(el);
          if (!usable(el)) continue;
          // Out of the window: reachable only by scrolling inside a dialog or popup.
          if (s.roots[0] === document.body && !hit(el)) continue;
          const kind = el instanceof HTMLSelectElement ? "select" : el instanceof HTMLTextAreaElement || (el instanceof HTMLInputElement && /^(text|number|search)$/.test(el.type || "text")) ? "fill" : "click";
          out.push({ id: `${pathOf(el)}|${nameOf(el)}`, label: nameOf(el) || el.tagName.toLowerCase(), kind, options: el instanceof HTMLSelectElement ? el.options.length : undefined });
        }
      }
      out.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
      return out;
    },
    /** Scroll the control into view and give its centre, or null when it is gone or covered. */
    locate(id: string) {
      const s = scope();
      for (const root of s.roots) {
        for (const el of root.querySelectorAll(CONTROL)) {
          if (!usable(el) || `${pathOf(el)}|${nameOf(el)}` !== id) continue;
          (el as HTMLElement).scrollIntoView({ block: "nearest", inline: "nearest" });
          if (!hit(el)) return null;
          const r = el.getBoundingClientRect();
          return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
        }
      }
      return null;
    },
    /** Choose an option of a select as a person would (the page hears input and change). */
    choose(id: string, index: number) {
      const s = scope();
      for (const root of s.roots) {
        for (const el of root.querySelectorAll("select")) {
          if (!usable(el) || `${pathOf(el)}|${nameOf(el)}` !== id) continue;
          el.selectedIndex = Math.min(index, el.options.length - 1);
          el.dispatchEvent(new Event("input", { bubbles: true }));
          el.dispatchEvent(new Event("change", { bubbles: true }));
          return true;
        }
      }
      return false;
    },
    paragraphs(): number {
      return [...document.querySelectorAll(".flow [data-p]")].filter((e) => !e.closest("[data-tab-hidden]") && shown(e)).length;
    },
    paragraph(n: number, fx: number, fy: number) {
      const list = [...document.querySelectorAll(".flow [data-p]")].filter((e) => !e.closest("[data-tab-hidden]") && shown(e));
      if (!list.length) return null;
      const el = list[Math.min(n, list.length - 1)] as HTMLElement;
      el.scrollIntoView({ block: "center" });
      const r = el.getBoundingClientRect();
      const x = r.left + Math.max(1, Math.min(r.width - 1, r.width * fx));
      const y = r.top + Math.max(1, Math.min(r.height - 1, r.height * fy));
      return { x, y };
    },
    margin(zone: "header" | "footer") {
      const f = document.querySelector(".flow") as HTMLElement | null;
      if (!f) return null;
      // The first page's own height, as drawn (zoom included); its bottom margin scrolled into view.
      const pg = (window as any).docxtor?.fe?.editor?.paginator;
      const scale = f.offsetWidth ? f.getBoundingClientRect().width / f.offsetWidth : 1;
      const pageH = (pg?.page(0)?.h ?? 1056) * scale;
      f.scrollIntoView({ block: "start" });
      let r2 = f.getBoundingClientRect();
      const canvas = f.closest(".canvas") as HTMLElement | null;
      if (zone === "footer" && canvas && r2.top + pageH - 40 > innerHeight - 60) {
        canvas.scrollTop += r2.top + pageH - 40 - (innerHeight - 200);
        r2 = f.getBoundingClientRect();
      }
      const y = zone === "header" ? r2.top + 40 : r2.top + pageH - 40;
      if (y < 8 || y > innerHeight - 8) return null;
      return { x: r2.left + r2.width / 2, y };
    },
  };
}

// -- stubs: what must never leave the page -------------------------------------------------------------------------

function installStubs() {
  const w = window as any;
  const cancel = () => Promise.reject(new DOMException("The user aborted a request.", "AbortError"));
  w.showOpenFilePicker = cancel;
  w.showSaveFilePicker = cancel;
  w.showDirectoryPicker = cancel;
  w.print = () => undefined;
  w.__monkeyOpened = [] as string[];
  w.open = (url?: string) => (w.__monkeyOpened.push(String(url ?? "")), null);
  HTMLInputElement.prototype.showPicker = function () {};
  const click = HTMLInputElement.prototype.click;
  HTMLInputElement.prototype.click = function () {
    if (this.type === "file") return;
    return click.call(this);
  };
}

/** Everything the page may not do: other origins, new windows, downloads, native dialogs, file choosers. */
function stubPage(page: Page, problems: string[]) {
  const here = (url: string) => {
    try {
      const u = new URL(url);
      return u.origin === `http://localhost:${process.env.DOCXTOR_E2E_PORT ?? 5180}` || u.protocol === "data:" || u.protocol === "blob:" || u.protocol === "about:";
    } catch {
      return true;
    }
  };
  return page.route("**/*", async (route) => {
    const req = route.request();
    if (here(req.url())) return route.fallback();
    // The fixture of helpers.ts answers the font requests; they are not the app reaching out.
    if (/^https:\/\/fonts\.(googleapis|gstatic)\.com\//.test(req.url())) return route.fallback();
    if (req.isNavigationRequest() && req.frame() === page.mainFrame()) problems.push(`navigation: the app tried to leave for ${req.url().slice(0, 160)}`);
    return route.abort();
  }).then(() => {
    page.on("dialog", (d) => void (d.type() === "prompt" ? d.dismiss() : d.accept()).catch(() => undefined));
    page.on("filechooser", (fc) => void fc.setFiles([]).catch(() => undefined));
    page.on("download", (d) => void d.cancel().catch(() => undefined));
    page.context().on("page", (p) => { if (p !== page) void p.close().catch(() => undefined); });
    page.on("framenavigated", (f) => {
      if (f === page.mainFrame() && !here(f.url())) problems.push(`navigation: the page is now at ${f.url().slice(0, 160)}`);
    });
  });
}

// -- the start ----------------------------------------------------------------------------------------------------

async function clearStorage(page: Page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Storage.clearDataForOrigin", { origin: new URL(page.url()).origin, storageTypes: "all" }).catch(() => undefined);
  await cdp.detach().catch(() => undefined);
}

/** A built document with tabs, a table, a list, a header, a footnote, an endnote, a text box and a content control. */
async function openMix(page: Page) {
  await openBase(page);
  await inPage(page, (w: any) => {
    const ed = w.fe.editor;
    const ids = [...ed.root.querySelectorAll("[data-p]")].map((e: any) => Number(e.dataset.p));
    ed.exec({ op: "set_list_style", ids: [ids[0], ids[1]], style: "disc" });
    ed.exec({ op: "tab", action: "add", name: "Second", first_name: "First" });
  });
}

async function start(page: Page, doc: string, problems: string[]) {
  await page.addInitScript(installProbe);
  await page.addInitScript(installStubs);
  await stubPage(page, problems);
  // A self-test of the clicker: DOCXTOR_MONKEY_PLANT=<text> makes the page log an error when that text is typed, to see it found, shrunk and replayed.
  if (process.env.DOCXTOR_MONKEY_PLANT) await page.addInitScript((k) => document.addEventListener("keydown", (e) => { if (e.key === k) console.error(`planted by DOCXTOR_MONKEY_PLANT: ${k}`); }, true), process.env.DOCXTOR_MONKEY_PLANT);
  await page.goto("/?open=simple.docx");
  await clearStorage(page);
  if (doc === "mix") await openMix(page);
  else await openFixture(page, doc);
  takeErrors(page);
}

// -- doing a step -------------------------------------------------------------------------------------------------

async function perform(page: Page, s: Step): Promise<boolean> {
  let reached = true;
  const ok = async (at: { x: number; y: number } | null, fn: (x: number, y: number) => Promise<void>) => {
    if (at) await fn(at.x, at.y);
    else reached = false;
  };
  switch (s.t) {
    case "click": {
      await ok(await page.evaluate((id) => (window as any).__monkey.locate(id), s.id), (x, y) => page.mouse.click(x, y));
      break;
    }
    case "select": {
      reached = await page.evaluate((a) => (window as any).__monkey.choose(a.id, a.index), s);
      break;
    }
    case "fill": {
      await ok(await page.evaluate((id) => (window as any).__monkey.locate(id), s.id), async (x, y) => {
        await page.mouse.click(x, y, { clickCount: 3 });
        await page.keyboard.type(s.text);
      });
      break;
    }
    case "text": {
      await ok(await page.evaluate((a) => (window as any).__monkey.paragraph(a.n, a.fx, a.fy), s), async (x, y) => {
        if (s.mode === "dbl") await page.mouse.dblclick(x, y);
        else if (s.mode === "right") await page.mouse.click(x, y, { button: "right" });
        else if (s.mode === "shift") { await page.keyboard.down("Shift"); await page.mouse.click(x, y); await page.keyboard.up("Shift"); }
        else await page.mouse.click(x, y);
      });
      break;
    }
    case "margin": {
      await ok(await page.evaluate((z) => (window as any).__monkey.margin(z), s.zone), (x, y) => page.mouse.dblclick(x, y));
      break;
    }
    case "key":
      await page.keyboard.press(s.key);
      break;
    case "type":
      await page.keyboard.type(s.text);
      break;
  }
  await page.waitForTimeout(50);
  return reached;
}

/** Pick the next step from what is on screen now. */
async function choose(page: Page, r: ReturnType<typeof rng>): Promise<Step> {
  const state: { modal: boolean; popup: boolean } = await page.evaluate(() => (window as any).__monkey.state());
  const paras: number = state.modal ? 0 : await page.evaluate(() => (window as any).__monkey.paragraphs());
  // Weights: typing and clicks in the text change the document, so they weigh most; controls explore the app.
  const kinds: [string, number][] = [
    ["control", state.modal ? 70 : state.popup ? 55 : 32],
    ["type", state.modal ? 8 : 24],
    ["text", paras ? (state.popup ? 8 : 20) : 0],
    ["key", state.modal ? 22 : 18],
    ["margin", paras && !state.popup ? 2 : 0],
  ];
  let roll = r.int(kinds.reduce((n, [, w]) => n + w, 0));
  let kind = "key";
  for (const [k, w] of kinds) { if (roll < w) { kind = k; break; } roll -= w; }
  if (kind === "control") {
    const list: { id: string; label: string; kind: string; options?: number }[] = await page.evaluate(() => (window as any).__monkey.controls());
    if (list.length) {
      const c = r.pick(list);
      if (c.kind === "select") return { t: "select", id: c.id, label: c.label, index: r.int(Math.max(1, c.options ?? 1)) };
      if (c.kind === "fill") return { t: "fill", id: c.id, label: c.label, text: r.pick(["1", "12", "0", "-1", "99999", "abc", "Hello", "a b", "", "😀", "1.5", "50%"]) };
      return { t: "click", id: c.id, label: c.label };
    }
    kind = "key";
  }
  if (kind === "text") {
    const mode = r.pick<"click" | "dbl" | "right" | "shift">(["click", "click", "click", "click", "dbl", "dbl", "right", "shift"]);
    return { t: "text", n: r.int(Math.max(1, paras)), fx: Math.round(r.next() * 100) / 100, fy: Math.round((0.2 + r.next() * 0.6) * 100) / 100, mode };
  }
  if (kind === "margin") return { t: "margin", zone: r.pick<"header" | "footer">(["header", "footer"]) };
  if (kind === "type") return { t: "type", text: r.pick(WORDS) };
  return { t: "key", key: r.pick(KEYS) };
}

// -- checking after a step ----------------------------------------------------------------------------------------

const known = (text: string) => MONKEY_KNOWN.find((k) => k.pattern.test(text));

async function frozen(page: Page, ms = 5000): Promise<boolean> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const answered = await Promise.race([
    page.evaluate(() => 1).then(() => true, () => true),
    new Promise<boolean>((resolve) => { timer = setTimeout(() => resolve(false), ms); }),
  ]);
  clearTimeout(timer);
  return !answered;
}

async function settle(page: Page) {
  for (let i = 0; i < 4; i++) {
    const open = await page.evaluate(() => !!document.querySelector('[role="dialog"], [role="alertdialog"], .menu-drop, .ctx-menu, .popover, .qp-item, .toast-menu')).catch(() => false);
    if (!open) break;
    await page.keyboard.press("Escape");
    await page.waitForTimeout(60);
  }
  await page.waitForTimeout(100);
}

/** What is wrong after the step just done (known bugs left out and counted). */
async function inspect(page: Page, problems: string[], checkpoint: boolean, skippedKnown: Map<string, number>): Promise<string[]> {
  const found: string[] = [];
  if (await frozen(page)) return ["U1: the page stopped answering for 5 seconds"];
  if (page.isClosed()) return ["U1: the page closed"];
  for (const e of takeErrors(page)) found.push(`U1: ${e.kind === "pageerror" ? "uncaught error" : "console.error"}: ${e.text.split("\n")[0].slice(0, 240)}`);
  found.push(...problems.splice(0));
  if (checkpoint) {
    await settle(page);
    found.push(...(await checkApp(page, { settled: true })));
  }
  return found.filter((f) => {
    const k = known(f);
    if (k) skippedKnown.set(k.reason, (skippedKnown.get(k.reason) ?? 0) + 1);
    return !k;
  });
}

// -- a run, a replay, a shrink --------------------------------------------------------------------------------------

interface Failure {
  /** The step after which it failed (an index in the steps), and what was wrong. */
  at: number;
  errors: string[];
}

/** The same failure in other words: numbers and ids taken out, so a shrunk list can be compared with the first one. */
const signature = (e: string) => e.replace(/\d+/g, "#").replace(/\s+/g, " ").slice(0, 100);

/** Run steps (given, or made by the seed until `count`) on a fresh start. Returns the steps done and the failure, if any. */
interface Stats { missed: number; dialogs: number; popups: number; revision: number; controls: Set<string> }

async function run(page: Page, doc: string, how: { steps?: Step[]; seed?: number; count?: number }, skippedKnown: Map<string, number>, stats?: Stats): Promise<{ done: Step[]; failure: Failure | null }> {
  const problems: string[] = [];
  const done: Step[] = [];
  try {
    await start(page, doc, problems);
  } catch (e) {
    return { done, failure: { at: -1, errors: [`U1: could not start: ${(e as Error).message.split("\n")[0]}`] } };
  }
  const r = how.seed !== undefined ? rng(how.seed) : null;
  const total = how.steps ? how.steps.length : how.count ?? STEPS;
  for (let i = 0; i < total; i++) {
    let step: Step;
    let errors: string[] = [];
    // Even a step that hangs must end: the limit is on the step, not on the whole test.
    const body = (async () => {
      step = how.steps ? how.steps[i] : await choose(page, r!);
      done.push(step);
      if (process.env.DOCXTOR_MONKEY_LOG) console.log(`  ${done.length}. ${label(step)}`);
      const reached = await perform(page, step);
      if (stats) {
        if (!reached) stats.missed++;
        if (step.t === "click" && reached) stats.controls.add(step.label);
        const st = await page.evaluate(() => (window as any).__monkey.state());
        if (st.modal) stats.dialogs++;
        else if (st.popup) stats.popups++;
      }
    })();
    const limit = new Promise<"hung">((resolve) => setTimeout(() => resolve("hung"), STEP_LIMIT_MS));
    const result = await Promise.race([body.then(() => "ok" as const, (e: Error) => `threw: ${e.message.split("\n")[0]}`), limit]);
    if (result === "hung") return { done, failure: { at: done.length - 1, errors: [`U1: the step did not finish in ${STEP_LIMIT_MS / 1000} seconds (the page is frozen or a wait never ended)`] } };
    if (result !== "ok") {
      // The page went away under the step (closed, crashed); anything else is the harness failing to reach a control, which is not a finding.
      if (page.isClosed()) return { done, failure: { at: done.length - 1, errors: ["U1: the page closed"] } };
    }
    errors = await inspect(page, problems, (i + 1) % CHECK_EVERY === 0, skippedKnown);
    if (errors.length) return { done, failure: { at: done.length - 1, errors } };
  }
  if (stats) stats.revision = await page.evaluate(() => (window as any).docxtor?.fe?.editor?.engine?.revision ?? -1).catch(() => -1);
  const end = await inspect(page, problems, true, skippedKnown);
  return { done, failure: end.length ? { at: done.length - 1, errors: end } : null };
}

/** Drop steps while the same failure remains: halves, then smaller pieces, within `tries` replays. */
async function shrink(page: Page, doc: string, steps: Step[], failure: Failure, tries: number): Promise<{ steps: Step[]; used: number }> {
  let best = steps.slice(0, failure.at + 1);
  const want = signature(failure.errors[0]);
  let used = 0;
  const same = async (candidate: Step[]) => {
    used++;
    const out = await run(page, doc, { steps: candidate }, new Map());
    return !!out.failure && out.failure.errors.some((e) => signature(e) === want);
  };
  for (let size = Math.max(1, Math.floor(best.length / 2)); size >= 1 && used < tries; size = size === 1 ? 0 : Math.max(1, Math.floor(size / 2))) {
    for (let from = 0; from < best.length && used < tries; ) {
      const candidate = [...best.slice(0, from), ...best.slice(from + size)];
      if (candidate.length && (await same(candidate))) best = candidate;
      else from += size;
    }
  }
  return { steps: best, used };
}

function save(seed: number | string, doc: string, steps: Step[], errors: string[], extra: Record<string, unknown> = {}): string {
  mkdirSync("monkey-results", { recursive: true });
  const file = `monkey-results/${seed}-${doc.replace(/\W+/g, "_")}.json`;
  writeFileSync(file, JSON.stringify({ seed, doc, errors, ...extra, steps }, null, 1));
  return file;
}

const describeSteps = (steps: Step[]) => steps.map((s, i) => `  ${i + 1}. ${label(s)}`).join("\n");

async function execute(page: Page, doc: string, seed: number | string, how: { steps?: Step[]; seed?: number }) {
  const skipped = new Map<string, number>();
  const t0 = Date.now();
  const stats: Stats = { missed: 0, dialogs: 0, popups: 0, revision: 0, controls: new Set() };
  const { done, failure } = await run(page, doc, how, skipped, stats);
  const secs = ((Date.now() - t0) / 1000).toFixed(1);
  const note = [...skipped].map(([reason, n]) => `${n}x known: ${reason.slice(0, 60)}`).join("; ");
  console.log(`monkey ${doc} seed ${seed}: ${done.length} steps in ${secs} s${note ? ` (${note})` : ""}${failure ? " FAILED" : " ok"}; ${stats.controls.size} different controls clicked, ${stats.missed} steps found nothing to act on, ${stats.dialogs} steps ended in a dialog and ${stats.popups} in a menu or popup, revision ${stats.revision}`);
  if (!failure) return;
  const full = save(seed, doc, done, failure.errors, { shrunk: false });
  let shrunk = done.slice(0, failure.at + 1);
  let used = 0;
  if (SHRINK_TRIES > 0 && shrunk.length > 1) {
    const out = await shrink(page, doc, done, failure, SHRINK_TRIES);
    shrunk = out.steps;
    used = out.used;
  }
  const small = save(`${seed}-shrunk`, doc, shrunk, failure.errors, { shrunk: true, shrinkReplays: used });
  throw new Error(
    `The random clicker found a failure (${doc}, seed ${seed}, step ${failure.at + 1} of ${done.length}):\n${failure.errors.map((e) => `  - ${e}`).join("\n")}\n\n` +
      `Shortest list found (${shrunk.length} steps, ${used} replays):\n${describeSteps(shrunk)}\n\n` +
      `Saved: ${full} and ${small}\nReplay: DOCXTOR_MONKEY_REPLAY=${small} npx playwright test e2e/monkey.spec.ts`,
  );
}

test.describe("monkey", () => {
  test.skip(!ON, "the random clicker is off by default: DOCXTOR_MONKEY=1 runs it (see the top of this file)");
  test.describe.configure({ mode: "parallel" });

  if (REPLAY) {
    test(`replay ${REPLAY}`, async ({ page }) => {
      test.setTimeout(10 * 60_000);
      const saved = JSON.parse(readFileSync(REPLAY, "utf8")) as { seed: number | string; doc: string; steps: Step[] };
      await execute(page, saved.doc, `${saved.seed} (replay)`, { steps: saved.steps });
    });
  } else {
    for (const doc of DOCS) {
      for (const seed of SEEDS) {
        test(`${doc}, seed ${seed}, ${STEPS} steps`, async ({ page }) => {
          test.setTimeout(20 * 60_000);
          await execute(page, doc, seed, { seed });
        });
      }
    }
  }
});
