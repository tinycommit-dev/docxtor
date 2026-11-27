// The accessibility gate: axe-core (WCAG 2.2 A and AA rules) on every screen and every dialog, in the light and the dark
// theme, and the keyboard checks axe cannot make. A violation fails the build. The only exceptions are in
// e2e/data/a11y-allow.json (rule, where, reason, date), each one explained; the file starts empty.
//
// What is scanned: everything round the document. The pages themselves (`.flow`) are the author's colours and structure, not
// our interface; the editor's own controls (rulers, status bar, page chips) are scanned. Dialogs are found through the
// command registry, not by hand: every command is run and, when it puts a dialog on screen, that dialog is scanned, then
// closed with Escape. A new dialog is therefore scanned the day it is added.
import AxeBuilder from "@axe-core/playwright";
import { appendFileSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { openBuilt } from "./support/docx-builder";
import { startDocsServer, type DocsServer } from "./support/docs-server";
import { chooseMenu, dialog, documentReady, expect, inPage, menuBar, openFixture, para, RICH, selectWord, tabKey, test, type Page } from "./helpers";

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];
interface Allowed {
  rule: string;
  /** A selector fragment of the element (matched against axe's selector path) or the screen's name. */
  where: string;
  reason: string;
  date: string;
}
const ALLOW: Allowed[] = JSON.parse(readFileSync(join(process.cwd(), "e2e/data/a11y-allow.json"), "utf8"));
const THEMES = ["light", "dark"] as const;
type Theme = (typeof THEMES)[number];

interface Finding {
  screen: string;
  rule: string;
  impact: string;
  nodes: string[];
}

/** One run of the spec: what it found, and what it has already reported (a header on every dialog is one finding, not forty). */
class Run {
  found: Finding[] = [];
  scanned: string[] = [];
  private seen = new Set<string>();

  /** Scan what is on screen (or inside `within`). The document's pages are left out. */
  async scan(page: Page, screen: string, opts: { within?: string } = {}) {
    let b = new AxeBuilder({ page }).withTags(TAGS).exclude(".flow");
    if (opts.within) b = b.include(opts.within);
    const res = await b.analyze();
    this.scanned.push(screen);
    for (const v of res.violations) {
      const nodes: string[] = [];
      for (const n of v.nodes) {
        const target = n.target.join(" ");
        if (ALLOW.some((a) => a.rule === v.id && (screen.includes(a.where) || target.includes(a.where)))) continue;
        const key = `${v.id} ${target}`;
        if (this.seen.has(key)) continue;
        this.seen.add(key);
        const why = (n.any[0] ?? n.all[0] ?? n.none[0])?.message ?? "";
        // A11Y_DUMP=file keeps every finding, one line each, for sorting by rule (the failure message shows the first few).
        if (process.env.A11Y_DUMP) appendFileSync(process.env.A11Y_DUMP, `${v.id}\t${v.impact}\t${screen}\t${target}\t${(n.any[0] ?? n.all[0] ?? n.none[0])?.message ?? ""}\n`);
        nodes.push(`${target}   ${n.html.slice(0, 150).replace(/\s+/g, " ")}   [${why}]`);
      }
      if (nodes.length) this.found.push({ screen, rule: v.id, impact: v.impact ?? "", nodes });
    }
  }

  expectClean() {
    const text = this.found.map((f) => `${f.rule} (${f.impact}) on ${f.screen}\n${f.nodes.slice(0, 8).map((n) => `    ${n}`).join("\n")}${f.nodes.length > 8 ? `\n    and ${f.nodes.length - 8} more` : ""}`).join("\n");
    expect(text, "axe found violations (fix them, or explain an exception in e2e/data/a11y-allow.json)").toBe("");
  }
}

/** Set the app's theme the way a person's system does: the colour scheme of the page ("system" is the app's default). */
async function setTheme(page: Page, t: Theme) {
  await page.emulateMedia({ colorScheme: t, reducedMotion: "reduce" });
}

/** Check the theme really is the one asked for (a scan of a light page called dark would pass for the wrong reason). */
async function expectTheme(page: Page, t: Theme) {
  const bg = await page.evaluate(() => {
    const root = document.querySelector(".docxtor") as HTMLElement;
    const m = getComputedStyle(root).backgroundColor.match(/[\d.]+/g)!.map(Number);
    return m[0] + m[1] + m[2];
  });
  if (t === "dark") expect(bg, "dark theme on").toBeLessThan(300);
  else expect(bg, "light theme on").toBeGreaterThan(450);
}

async function startScreen(page: Page) {
  await page.goto("/");
  await page.waitForFunction(() => !!(window as unknown as { docxtor?: unknown }).docxtor);
  await expect(page.locator(".start")).toBeVisible();
}

/** Press Escape and wait until nothing is left open (a menu, a popover, a dialog). */
async function closeAll(page: Page) {
  for (let i = 0; i < 3; i++) {
    if (!(await page.locator(".docxtor-layer .scrim, .docxtor-layer .pop, .menu-drop, .ctx-menu").count())) return;
    await page.keyboard.press("Escape");
    await page.waitForTimeout(60);
  }
}

// -- the screens ----------------------------------------------------------------------------------------------------

for (const t of THEMES) {
  test.describe(`screens, ${t} theme`, () => {
    test.beforeEach(async ({ page }) => {
      test.setTimeout(150_000);
      await setTheme(page, t);
    });

    test("the start screen, with no documents and with recent ones", async ({ page }) => {
      const run = new Run();
      await startScreen(page);
      await expectTheme(page, t);
      await run.scan(page, `start screen, empty (${t})`);
      await inPage(page, (w) => w.createIn(w.files.library, "", "Alpha.docx", "# Alpha\n\nFirst."));
      await inPage(page, (w) => w.createIn(w.files.library, "", "Beta.docx", "Second."));
      await inPage(page, (w) => w.createIn(w.files.library, "", "Gamma.docx", ""));
      await startScreen(page);
      await expect(page.locator(".start .doc-tile")).not.toHaveCount(0);
      await run.scan(page, `start screen, with recent documents (${t})`);
      run.expectClean();
    });

    test("the editor with a real document: the chrome, the rulers, the status bar, the menus and the toolbar", async ({ page }) => {
      const run = new Run();
      await openFixture(page);
      await expectTheme(page, t);
      await run.scan(page, `editor, rich.docx (${t})`);
      // The ruler and the non-printing marks add controls of their own.
      await inPage(page, (w) => w.fe.extensions.execute("view.ruler"));
      await run.scan(page, `editor, rich.docx with the ruler (${t})`);
      // Each top menu, open.
      const tops = await menuBar(page).getByRole("menuitem").all();
      for (const top of tops) {
        const name = (await top.textContent())!.trim();
        if (!name || (await top.getAttribute("class"))?.includes("menubar-fold")) continue;
        await top.click();
        await expect(page.locator(".menu-drop").first()).toBeVisible();
        await run.scan(page, `menu ${name} (${t})`);
        await closeAll(page);
      }
      // Each toolbar button that opens something.
      const popups = page.locator('.toolbar [aria-haspopup]');
      const n = await popups.count();
      for (let i = 0; i < n; i++) {
        const b = popups.nth(i);
        if (!(await b.isVisible()) || !(await b.isEnabled())) continue;
        const label = (await b.getAttribute("aria-label")) ?? (await b.getAttribute("title")) ?? `button ${i}`;
        await b.click();
        await page.waitForTimeout(80);
        if (await page.locator(".docxtor-layer .pop, .docxtor-layer .scrim").count()) await run.scan(page, `toolbar, ${label} (${t})`);
        await closeAll(page);
      }
      run.expectClean();
    });

    test("the context menu, on text, in a table and on a picture", async ({ page }) => {
      const run = new Run();
      await openFixture(page);
      await para(page, RICH.intro).click({ button: "right", position: { x: 40, y: 8 } });
      await expect(page.locator(".ctx-menu")).toBeVisible();
      await run.scan(page, `context menu on text (${t})`);
      await closeAll(page);
      await para(page, RICH.cell).click({ button: "right" });
      await expect(page.locator(".ctx-menu")).toBeVisible();
      await run.scan(page, `context menu in a table (${t})`);
      await closeAll(page);
      const image = page.locator(".flow img").first();
      await image.scrollIntoViewIfNeeded();
      await image.click({ button: "right" });
      await expect(page.locator(".ctx-menu")).toBeVisible();
      await run.scan(page, `context menu on a picture (${t})`);
      await closeAll(page);
      run.expectClean();
    });

    test("the find bar, with replace and the more options, and its results", async ({ page }) => {
      const run = new Run();
      await openFixture(page);
      await page.keyboard.press("ControlOrMeta+f");
      const bar = page.locator(".find-bar");
      await expect(bar).toBeVisible();
      await run.scan(page, `find bar (${t})`);
      await bar.getByRole("textbox").first().fill("the");
      await page.keyboard.press("Enter");
      for (const name of [/more/i, /list|results/i]) {
        const b = bar.getByRole("button", { name }).first();
        if (await b.count()) await b.click();
      }
      await page.waitForTimeout(150);
      await run.scan(page, `find bar, expanded, with results (${t})`);
      await page.keyboard.press("Escape");
      run.expectClean();
    });

    test("a themed document, with the Design galleries open", async ({ page }) => {
      const run = new Run();
      await openFixture(page, "complex.docx");
      await expectTheme(page, t);
      await run.scan(page, `editor, complex.docx (${t})`);
      for (const gallery of ["design.themes", "design.colors", "design.fonts", "design.styleSet", "design.spacing"]) {
        await page.evaluate((c) => void (window as any).docxtor.fe.extensions.execute(c).catch(() => undefined), gallery);
        await expect(page.locator(".docxtor-layer .pop").first()).toBeVisible();
        await run.scan(page, `${gallery} gallery (${t})`, { within: ".docxtor-layer .pop" });
        // Choose the second theme or scheme in the list: the page then carries a theme's colours and fonts.
        if (gallery === "design.themes") {
          await page.locator(".docxtor-layer .pop button").nth(1).click();
          await page.waitForTimeout(300);
          await run.scan(page, `editor, a theme applied (${t})`);
        } else await closeAll(page);
      }
      run.expectClean();
    });

    test("the document tabs pane with tabs, and the other panes' own lists", async ({ page }) => {
      const run = new Run();
      await openBuilt(page, { sections: [{ paragraphs: ["Overview text", "More of the overview"] }] });
      await page.evaluate(() => (window as any).docxtor.showPanel("navigation"));
      for (const name of ["Details", "Appendix"]) {
        await chooseMenu(page, "Insert", "Tab…");
        const d = dialog(page, "Add tab");
        await d.getByRole("textbox").fill(name);
        await d.getByRole("button", { name: "Add" }).click();
      }
      await expect(page.locator(".doc-tabs-pane").getByRole("treeitem")).toHaveCount(3);
      await run.scan(page, `navigation pane with document tabs (${t})`);
      run.expectClean();
    });

    test("the assistant panel, with its settings", async ({ page }) => {
      const run = new Run();
      await openFixture(page);
      await page.evaluate(() => void (window as any).docxtor.fe.extensions.execute("view.assistant"));
      const panel = page.locator(".ai-panel, .ai").first();
      await expect(panel).toBeVisible();
      await run.scan(page, `assistant (${t})`);
      // Its settings, where the panel has them.
      const settings = panel.getByRole("button", { name: /settings|provider|model|key/i }).first();
      if (await settings.count()) {
        await settings.click();
        await page.waitForTimeout(150);
        await run.scan(page, `assistant settings (${t})`);
      }
      run.expectClean();
    });

    test("the side panes", async ({ page }) => {
      const run = new Run();
      await openFixture(page);
      const ids = await page.evaluate(() => (window as any).docxtor.fe.extensions.panels.list().map((p: { id: string }) => p.id) as string[]);
      expect(ids.length, "the registry lists the side panes").toBeGreaterThan(3);
      for (const id of ids) {
        await page.evaluate((i) => (window as any).docxtor.showPanel(i), id);
        await page.waitForTimeout(150);
        await run.scan(page, `side pane ${id} (${t})`);
      }
      // The panes the commands open (styles, reveal formatting, selection, navigation, dictionary, citations, accessibility check).
      for (const id of ["format.stylesPane", "arrange.selectionPane", "view.outline", "tools.accessibility", "tools.citations", "tools.dictionary", "tools.thesaurus"]) {
        await page.evaluate((c) => void (window as any).docxtor.fe.extensions.execute(c).catch(() => undefined), id);
        await page.waitForTimeout(250);
        await run.scan(page, `pane from ${id} (${t})`);
      }
      run.expectClean();
    });
  });
}

// -- every dialog, found through the command registry -----------------------------------------------------------------

const SHARDS = 12;
const SCRIM = ".docxtor-layer .scrim";

/** Where the keyboard is, as a marker the page keeps (compared after Escape). */
const markFocus = (page: Page) => page.evaluate(() => { document.querySelectorAll("[data-a11y-opener]").forEach((e) => e.removeAttribute("data-a11y-opener")); const a = document.activeElement as HTMLElement; (a.closest(".flow, .story-edit") ?? a).setAttribute("data-a11y-opener", "1"); });
/** Whether the keyboard is back where `markFocus` found it (anywhere in the same text, or the same control): "" when it is, else what has it. */
const focusBack = (page: Page) =>
  page.evaluate(() => {
    const a = document.activeElement as HTMLElement | null;
    return ((a?.closest(".flow, .story-edit") ?? a)?.hasAttribute("data-a11y-opener") ?? false) ? "" : `${a?.tagName}.${a?.className}#${a?.id}`;
  });

/** Where the caret is when a command runs: in the text with a word selected, in a table cell, on a picture. */
const CONTEXTS: { name: string; put: (page: Page) => Promise<void> }[] = [
  { name: "text", put: async (page) => { await selectWord(page, RICH.last, "paragraph"); } },
  { name: "a table", put: async (page) => { await para(page, RICH.cell).click(); } },
  { name: "a picture", put: async (page) => { await page.locator(".flow img").first().click(); } },
];

/** What shows the app is in its usual state: the page, the view mode, the panes, the open story (header, footer, notes) and the text. */
const fingerprint = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { docxtor: { fe: { editor: { story: unknown; engine: { revision: number } | null; readOnly?: boolean } } } };
    const ed = w.docxtor.fe.editor;
    const root = document.querySelector(".docxtor") as HTMLElement;
    const attrs = Array.from(root.attributes).map((a) => `${a.name}=${a.value}`).join(" ");
    return [location.hash, attrs, document.querySelector(".sidebar")?.className, document.querySelector(".editor-host")?.className, !!ed.story, ed.engine?.revision, document.querySelectorAll(".flow").length, document.fullscreenElement ? "fs" : ""].join("|");
  });

/** Tab through a dialog twice round: the focus must never leave it. */
async function trapsFocus(page: Page): Promise<string> {
  const inside = () => page.evaluate(() => {
    const a = document.activeElement;
    const scrims = document.querySelectorAll(".docxtor-layer .scrim");
    return !!a && scrims.length > 0 && scrims[scrims.length - 1].contains(a);
  });
  if (!(await inside())) return `the focus is not inside the dialog when it opens (it is on ${await page.evaluate(() => { const a = document.activeElement as HTMLElement; return `${a?.tagName}.${a?.className}#${a?.id}, ${document.querySelectorAll(".docxtor-layer .scrim").length} dialogs`; })})`;
  for (let i = 0; i < 60; i++) {
    await page.keyboard.press(tabKey(page));
    if (!(await inside())) return `Tab ${i + 1} left the dialog`;
  }
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press(tabKey(page, true));
    if (!(await inside())) return `Shift+Tab ${i + 1} left the dialog`;
  }
  return "";
}

for (const t of THEMES) {
  for (let shard = 0; shard < SHARDS; shard++) {
    test(`dialogs, ${t} theme, part ${shard + 1} of ${SHARDS}`, async ({ page, browserName }) => {
      test.setTimeout(300_000);
      // Print… hands the page to the browser's own print dialog. Chromium's headless build lets it go at once; Firefox's holds the page until
      // the dialog is closed, which a test can't do (the part stood still at Print… for the five minutes it had). The dialog is the browser's,
      // not the app's, so there is nothing for this sweep to look at in it: the call just ends, as the browser's does.
      if (browserName === "firefox") await page.addInitScript(() => { window.print = () => window.dispatchEvent(new Event("afterprint")); });
      // A click that can't be made (the picture moved, something is over it) must not wait out the test: that attempt is given up.
      page.setDefaultTimeout(4_000);
      // A command that opens another window (View, New window) must not take the keyboard from this one: the window is closed at once.
      page.context().on("page", (other) => void other.opener().then((from) => (from === page ? other.close() : undefined)).catch(() => undefined));
      await setTheme(page, t);
      const run = new Run();
      const keyboard: string[] = [];
      let plain = "";
      let starts = 0;
      const t0 = Date.now();
      const start = async () => {
        starts++;
        await page.bringToFront();
        await openFixture(page);
        await selectWord(page, RICH.last, "paragraph");
        plain = await fingerprint(page);
      };
      await start();
      const cmds: { id: string; title: string }[] = await page.evaluate(() => (window as any).docxtor.fe.extensions.commands.list().map((c: any) => ({ id: c.id, title: c.title })));
      // A11Y_ONLY=format.tabs,palette.show runs just those commands (to look at one dialog).
      const only = (process.env.A11Y_ONLY ?? "").split(",").filter(Boolean);
      const mine = cmds.filter((c, i) => i % SHARDS === shard && (!only.length || only.includes(c.id)));
      const seen: string[] = [];
      for (const c of mine) {
        if (process.env.A11Y_TRACE) console.log(`${Math.round((Date.now() - t0) / 1000)} s, ${starts} starts: ${c.id}`);
        // A command that needs a table or a picture offers nothing on plain text: each command is tried where a person would use it.
        for (const where of CONTEXTS) {
          // The result of one attempt: where to go next, and what the keyboard did wrong (if anything).
          const attempt = async (): Promise<{ next: "try-next-place" | "done"; problems: string[] }> => {
            const problems: string[] = [];
            // Every attempt starts from the same document in the same state, as a person's does: a command that changed the view (full
            // screen, reading mode, a side pane), the text or the page it is on leaves the next one a fresh start.
            if ((await fingerprint(page).catch(() => "")) !== plain) await start();
            await where.put(page).catch(() => undefined);
            await markFocus(page).catch(() => undefined);
            // The answer: "unavailable" says the command is for somewhere else (a table, a picture), so the next place is tried; a dialog's
            // command answers only when the dialog is closed, so no answer in a moment means it is open.
            const answer = await page.evaluate((id) => Promise.race([(window as any).docxtor.fe.extensions.execute(id).then((r: { status: string }) => r.status, () => "failed"), new Promise<string>((res) => setTimeout(() => res("pending"), 120))]), c.id).catch(() => "lost");
            if (answer === "lost") { await start(); return { next: "try-next-place", problems }; }
            if (answer === "unavailable") return { next: "try-next-place", problems };
            const shown = await page.waitForSelector(`${SCRIM}, .docxtor-layer .pop`, { state: "visible", timeout: answer === "pending" ? 700 : 150 }).then(() => true, () => false);
            if (!shown) return { next: "done", problems };
            const isDialog = (await page.locator(SCRIM).count()) > 0;
            await page.waitForTimeout(120);
            const name = await page.evaluate(() => {
              const d = document.querySelector(".docxtor-layer .scrim [role=dialog], .docxtor-layer .scrim [role=alertdialog]") as HTMLElement | null;
              return d?.getAttribute("aria-label") ?? d?.querySelector("h1,h2,h3")?.textContent ?? (d ? "(no name)" : "(popup)");
            });
            const screen = `${isDialog ? "dialog" : "popup"} ${c.id} "${name}" on ${where.name} (${t})`;
            if (!seen.includes(screen)) seen.push(screen);
            await run.scan(page, screen, { within: isDialog ? SCRIM : ".docxtor-layer .pop" });
            if (isDialog && t === "light") {
              const trap = await trapsFocus(page);
              if (trap) problems.push(`${screen}: ${trap}`);
            }
            await page.keyboard.press("Escape");
            const gone = await page.waitForSelector(`${SCRIM}, .docxtor-layer .pop`, { state: "detached", timeout: 800 }).then(() => true, () => false);
            if (!gone) problems.push(`${screen}: Escape did not close it`);
            else if (isDialog && t === "light") {
              const back = await focusBack(page);
              if (back) problems.push(`${screen}: Escape did not give the focus back to where it was (it is on ${back})`);
            }
            return { next: "done", problems };
          };
          let result = await attempt();
          // Commands before this one may have left the page in a state of their own (a mode, a pane that took the keyboard): a keyboard
          // problem counts only if it happens again on a fresh page.
          if (result.problems.length) {
            await start();
            result = await attempt();
            keyboard.push(...result.problems);
            if (result.problems.length) await start();
          }
          if (result.next === "done") break;
        }
      }
      if (process.env.A11Y_LIST) appendFileSync(process.env.A11Y_LIST, seen.map((x) => `${x}\n`).join(""));
      console.log(`part ${shard + 1} ${t}: ${mine.length} commands, ${seen.length} dialogs and popups, ${starts} fresh starts, ${Math.round((Date.now() - t0) / 1000)} s`);
      // The sweep must find dialogs: a harness that stopped seeing them would pass for the wrong reason.
      if (!only.length) expect(seen.length, "dialogs and popups this part found").toBeGreaterThan(1);
      run.expectClean();
      expect(keyboard.join("\n"), "dialogs and the keyboard").toBe("");
    });
  }
}

// -- the keyboard, on the main paths ----------------------------------------------------------------------------------

test.describe("the keyboard", () => {
  test("the start screen's tiles and buttons are reached with Tab", async ({ page }) => {
    await startScreen(page);
    await inPage(page, (w) => w.createIn(w.files.library, "", "Alpha.docx", "# Alpha"));
    await inPage(page, (w) => w.createIn(w.files.library, "", "Beta.docx", "Beta"));
    await startScreen(page);
    const tiles = await page.locator(".start .doc-tile").count();
    expect(tiles).toBeGreaterThanOrEqual(2);
    const wanted = await page.evaluate(() => Array.from(document.querySelectorAll(".start button, .start a[href], .start [tabindex='0']")).filter((e) => (e as HTMLElement).offsetParent).length);
    const reached = new Set<string>();
    for (let i = 0; i < 300 && reached.size < wanted; i++) {
      await page.keyboard.press(tabKey(page));
      const where = await page.evaluate(() => {
        const a = document.activeElement as HTMLElement | null;
        if (!a || !a.closest(".start")) return null;
        const all = Array.from(document.querySelectorAll(".start *"));
        return String(all.indexOf(a));
      });
      if (where) reached.add(where);
    }
    expect(reached.size, `the start screen has ${wanted} controls; Tab reached ${reached.size}`).toBe(wanted);
    // Every tile is among them: focus each by Tab and open it with Enter.
    const tile = page.locator(".start .doc-tile").first();
    await tile.focus();
    await expect(tile).toBeFocused();
    await page.keyboard.press("Enter");
    await documentReady(page);
  });

  test("a dialog keeps the focus with the plain Tab key too, where the browser's Tab passes buttons and links (Safari's default)", async ({ page }) => {
    await openFixture(page);
    await chooseMenu(page, "File", "Language…");
    const box = dialog(page, "Language");
    await expect(box).toBeVisible();
    const inside = () => page.evaluate(() => { const a = document.activeElement; return !!a && !!a.closest(".docxtor-layer .scrim"); });
    for (let i = 0; i < 12; i++) {
      await page.keyboard.press("Tab");
      await page.waitForTimeout(30);
      expect(await inside(), `after Tab ${i + 1}`).toBe(true);
    }
    for (let i = 0; i < 4; i++) {
      await page.keyboard.press("Shift+Tab");
      await page.waitForTimeout(30);
      expect(await inside(), `after Shift+Tab ${i + 1}`).toBe(true);
    }
  });

  test("a tool's tab closes with Delete, and the style list opens its menu from the keyboard", async ({ page }) => {
    await openFixture(page);
    await page.evaluate(() => (window as any).docxtor.showPanel("accessibility"));
    const tab = page.getByRole("tab", { name: "Accessibility", exact: true });
    await expect(tab).toBeVisible();
    await tab.focus();
    await page.keyboard.press("Delete");
    await expect(tab).toHaveCount(0);
    // The Styles pane: the option's own menu from the Menu key (the key beside each row is for the pointer).
    await page.evaluate(() => void (window as any).docxtor.fe.extensions.execute("format.stylesPane"));
    const option = page.locator(".styles-main").first();
    await option.focus();
    await page.keyboard.press("Shift+F10");
    await expect(page.locator(".docxtor-layer .pop")).toBeVisible();
    await page.keyboard.press("Escape");
  });

  test("the menu bar opens with the keyboard and the arrows move", async ({ page }) => {
    await openFixture(page);
    const first = menuBar(page).getByRole("menuitem").filter({ hasNot: page.locator(".menubar-fold") }).first();
    await menuBar(page).getByRole("menuitem", { name: "File", exact: true }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(menuBar(page).getByRole("menuitem", { name: "Edit", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(menuBar(page).getByRole("menuitem", { name: "Edit", exact: true })).toHaveAttribute("aria-expanded", "true");
    const items = page.locator('.menu-drop [role="menuitem"], .menu-drop [role="menuitemcheckbox"]');
    await expect(items.first()).toBeFocused();
    const firstLabel = await items.first().textContent();
    await page.keyboard.press("ArrowDown");
    expect(await items.first().textContent()).toBe(firstLabel);
    await expect(items.first()).not.toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(items.first()).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.locator(".menu-drop")).toHaveCount(0);
    await expect(menuBar(page).getByRole("menuitem", { name: "Edit", exact: true })).toBeFocused();
    void first;
  });

  test("the command palette opens from the keyboard, traps focus and gives it back", async ({ page }) => {
    await openFixture(page);
    await para(page, RICH.intro).click();
    await markFocus(page);
    await page.keyboard.press("ControlOrMeta+Shift+p");
    await expect(page.locator(SCRIM)).toBeVisible();
    expect(await trapsFocus(page)).toBe("");
    await page.keyboard.press("Escape");
    await expect(page.locator(SCRIM)).toHaveCount(0);
    expect(await focusBack(page)).toBe("");
  });
});

// -- the documentation site (docs/, served at /docs/) ------------------------------------------------------------------
// The landing page and content pages with code, tables and a long outline, the open search box, and a phone-width page with its menu open,
// in both themes. The site is built and served here with the production headers (e2e/docs-server.ts).
test.describe("the documentation site", () => {
  test.describe.configure({ mode: "serial" });
  let docs: DocsServer;
  test.beforeAll(async () => {
    test.setTimeout(180_000);
    docs = await startDocsServer("a11y");
  });
  test.afterAll(() => docs?.stop());

  for (const t of THEMES) {
    test(`the landing page and the content pages, ${t} theme`, async ({ page }) => {
      test.setTimeout(120_000);
      await setTheme(page, t);
      const run = new Run();
      for (const path of ["/docs/", "/docs/use/", "/docs/use/writing", "/docs/use/shortcuts", "/docs/embedding", "/docs/ai/", "/docs/performance/", "/docs/deploy", "/docs/contributing", "/docs/privacy", "/docs/roadmap"]) {
        await page.goto(`${docs.url}${path}`);
        await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
        // The theme asked for is the theme drawn (a scan of a light page called dark would pass for the wrong reason).
        await expect(page.locator("html")).toHaveClass(t === "dark" ? /dark/ : /^(?!.*dark)/);
        await run.scan(page, `docs ${path} (${t})`);
      }
      run.expectClean();
    });

    test(`the search box and a page at phone width, ${t} theme`, async ({ page }) => {
      test.setTimeout(120_000);
      await setTheme(page, t);
      const run = new Run();
      await page.goto(`${docs.url}/docs/`);
      await page.getByRole("button", { name: "Search" }).click();
      await page.getByRole("searchbox").fill("embed");
      await expect(page.locator(".VPLocalSearchBox a").first()).toBeVisible();
      await run.scan(page, `docs search results (${t})`);
      await page.keyboard.press("Escape");
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(`${docs.url}/docs/embedding`);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      await run.scan(page, `docs /docs/embedding at phone width (${t})`);
      await page.getByRole("button", { name: "Menu" }).first().click();
      await run.scan(page, `docs sidebar menu at phone width (${t})`);
      run.expectClean();
    });
  }

  test("the keyboard reaches the skip link, the menu, the search and the page", async ({ page }) => {
    await page.goto(`${docs.url}/docs/embedding`);
    await page.keyboard.press("Tab");
    await expect(page.getByRole("link", { name: "Skip to content" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("#VPContent")).toBeVisible();
    // Control+K opens the search from anywhere and Escape gives the focus back.
    await page.keyboard.press("ControlOrMeta+k");
    await expect(page.getByRole("searchbox")).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("searchbox")).toHaveCount(0);
  });
});

void chooseMenu;
void selectWord;
