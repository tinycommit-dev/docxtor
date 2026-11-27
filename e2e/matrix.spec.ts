// The matrix: modes by actions (architecture review, section 3.4). Every mode the app has (views, window, where the
// caret is, the document's state, read-only by each of its sources, light and dark, four languages) is crossed with
// every action: each command the host lists, run through `host.execute`, and the things a person does that are not
// commands (type, Enter, Backspace, paste, select all then type, undo, redo, open another document, open another tab, save,
// find, one assistant tool call). After each one `checkApp` (helpers.ts) checks rules U1 to U6, U2 in the read-only
// modes and U10; the actions are built from the host's registry, so a new command joins the matrix by itself.
//
// It is not part of the normal browser run: it takes 15 to 30 minutes. Turn it on with DOCXTOR_MATRIX=1 (every mode
// alone by every action, then every pair of modes by the short list), or name the modes:
//   DOCXTOR_MATRIX_MODES=reading,split npx playwright test e2e/matrix.spec.ts
// which runs those modes alone and, unless DOCXTOR_MATRIX_PAIRS=0, the pairs among them. DOCXTOR_MATRIX_ACTIONS=<text>
// keeps the actions whose names contain the text (comma separated), to look at one thing quickly. A dialog that a
// command opens gets Escape (DOCXTOR_MATRIX_SLICE and DOCXTOR_MATRIX_THEN, below, narrow down a finding that needs an earlier action); DOCXTOR_MATRIX_CONFIRM=1 runs a second pass that presses its confirm button instead and
// checks the revision did not change (rule U7).
import { checkApp, expect, inPage, markBefore, pasteData, takeErrors, test, type Page } from "./helpers";
import { MATRIX_KNOWN } from "./support/matrix-known";
import { LEAVE, MODES, NotHere, openBase, windowState, type Mode } from "./support/matrix-modes";

/* eslint-disable @typescript-eslint/no-explicit-any */
const ON = !!(process.env.DOCXTOR_MATRIX || process.env.DOCXTOR_MATRIX_MODES);
const wanted = process.env.DOCXTOR_MATRIX_MODES?.split(",").map((s) => s.trim()).filter(Boolean);
const modes = wanted ? MODES.filter((m) => wanted.includes(m.id)) : MODES;
const pairsOn = process.env.DOCXTOR_MATRIX_PAIRS ? process.env.DOCXTOR_MATRIX_PAIRS !== "0" : true;
const confirmPass = !!process.env.DOCXTOR_MATRIX_CONFIRM;
const onlyActions = process.env.DOCXTOR_MATRIX_ACTIONS?.split(",").map((s) => s.trim()).filter(Boolean);
// To find which earlier action leaves the state a finding needs: DOCXTOR_MATRIX_SLICE=a:b keeps the actions a to b-1 of the whole list (a:b,c:d keeps
// several stretches, for a finding that needs one action from each)
// (`DOCXTOR_MATRIX_SLICE=list` prints it), and DOCXTOR_MATRIX_THEN names the actions to run after them, comma separated, exactly as a finding
// names them ("command view.viewing"). Halve the slice until one action is left.
const slice = process.env.DOCXTOR_MATRIX_SLICE;
const then = process.env.DOCXTOR_MATRIX_THEN?.split(",").map((s) => s.trim()).filter(Boolean) ?? [];

/** Commands that cannot be run by a test, each with the reason. */
const SKIP_COMMANDS: Record<string, string> = {
  "file.close": "leaves the document, which the matrix needs for the next action (opening another document is its own action)",
};

interface Action {
  name: string;
  /** Run it; `settle` has already been called by the runner afterwards. Returns a note when the action did not apply. */
  run(page: Page): Promise<void>;
  /** The short list: the actions run for every pair of modes. */
  short?: boolean;
}

const dialogOpen = (page: Page) => page.evaluate(() => !!document.querySelector('[role="dialog"], [role="alertdialog"]'));

async function settle(page: Page) {
  for (let i = 0; i < 4; i++) {
    const open = await page.evaluate(() => !!document.querySelector('[role="dialog"], [role="alertdialog"], .menu-drop, .ctx-menu, .popover, .qp-item, .toast-menu'));
    if (!open) break;
    await page.keyboard.press("Escape");
    await page.waitForTimeout(60);
  }
  // The keyboard goes back to the document a moment after a popup closes.
  await page.waitForTimeout(120);
  for (const extra of page.context().pages()) if (extra !== page) await extra.close().catch(() => undefined);
}

/** The commands that opened a dialog, by mode: the second pass presses their confirm button (rule U7). */
const opened = new Map<string, Set<string>>();

const commandAction = (id: string, mode?: string, confirm = false): Action => ({
  name: confirm ? `command ${id}, then its confirm button` : `command ${id}`,
  async run(page) {
    const revision = await page.evaluate(() => (window as any).docxtor.fe.editor.engine?.revision as number | undefined);
    await page.evaluate((cid) => {
      const w = window as any;
      w.__matrix = { done: false, status: "" };
      w.docxtor.fe.extensions.execute(cid).then(
        (r: any) => Object.assign(w.__matrix, { done: true, status: r.status }),
        (e: any) => Object.assign(w.__matrix, { done: true, status: `threw: ${e?.message ?? e}` }),
      );
    }, id);
    await page.waitForTimeout(120);
    if (await dialogOpen(page)) {
      if (mode) (opened.get(mode) ?? opened.set(mode, new Set()).get(mode)!).add(id);
      if (confirm) {
        const ok = page.locator('[role="dialog"] .btn.primary, [role="alertdialog"] .btn.primary').first();
        if (await ok.isVisible().catch(() => false) && (await ok.isEnabled().catch(() => false))) await ok.click({ timeout: 2000 }).catch(() => undefined);
        await page.waitForTimeout(150);
        const after = await page.evaluate(() => (window as any).docxtor.fe.editor.engine?.revision as number | undefined);
        if (revision !== undefined && after !== undefined && after !== revision) throw new Error(`U7: pressing the confirm button of the dialog without changing anything changed the revision (${revision} to ${after})`);
      }
      await page.keyboard.press("Escape");
    }
    // A command that waits for a dialog finishes when it closes; one that never finishes is a freeze of its own.
    const done = await page.waitForFunction(() => (window as any).__matrix.done, null, { timeout: 4000 }).then(() => true, () => false);
    if (!done) {
      await settle(page);
      const again = await page.evaluate(() => (window as any).__matrix.done);
      if (!again) throw new Error("the command never finished, even after Escape");
    }
    const status = await page.evaluate(() => (window as any).__matrix.status as string);
    if (status.startsWith("threw:")) throw new Error(`the command ${status}`);
  },
});

const key = (name: string, keys: string, short = true): Action => ({ name, short, run: (page) => page.keyboard.press(keys) });

const typed = (text: string) => async (page: Page) => {
  await page.keyboard.type(text);
};

const ACTIONS: Action[] = [
  { name: "type", short: true, run: typed("hello matrix") },
  key("Enter", "Enter"),
  { name: "Backspace", short: true, run: async (page) => void (await page.keyboard.press("Backspace"), await page.keyboard.press("Backspace")) },
  {
    name: "paste",
    short: true,
    async run(page) {
      // Only Chromium lets a test write the system clipboard; elsewhere the paste event carries the text.
      if (page.context().browser()?.browserType().name() === "chromium") {
        await page.evaluate(() => navigator.clipboard.writeText("pasted by the matrix"));
        await page.keyboard.press("ControlOrMeta+v");
      } else {
        await pasteData(page, { "text/plain": "pasted by the matrix" });
      }
    },
  },
  { name: "select all then type", short: true, run: async (page) => void (await page.keyboard.press("ControlOrMeta+a"), await page.keyboard.type("x")) },
  key("undo", "ControlOrMeta+z"),
  key("redo", "ControlOrMeta+Shift+z"),
  {
    name: "open another document",
    short: true,
    async run(page) {
      await inPage(page, (w: any) => w.createIn(w.files.library, "", `Other${Date.now() % 100000}.docx`, "Another document the matrix opened."));
      await page.waitForTimeout(300);
    },
  },
  {
    name: "open another tab",
    short: true,
    async run(page) {
      await inPage(page, (w: any) => {
        const ed = w.fe.editor;
        const { tabs, active } = ed.tabs;
        const other = tabs.find((t: any) => t.id !== active);
        if (other) ed.openTab(other.id);
      });
    },
  },
  key("save", "ControlOrMeta+s"),
  {
    name: "find and jump",
    short: true,
    async run(page) {
      await page.keyboard.press("ControlOrMeta+f");
      await page.keyboard.type("paragraph");
      await page.keyboard.press("Enter");
      await page.keyboard.press("Enter");
    },
  },
  {
    name: "one assistant tool call",
    short: true,
    async run(page) {
      await inPage(page, (w: any) => {
        try {
          // As the assistant runs a tool (web/src/ai/agent.ts): the engine's tool, then the pane drawn again from what changed.
          w.fe.editor.engine.raw.callTool("replace_text", JSON.stringify({ find: "matrix", replace: "grid" }));
          w.fe.editor.refresh({ keepScroll: true });
        } catch {
          /* a refusal (read-only) is the right answer */
        }
      });
    },
  },
];

async function listCommands(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as any).docxtor.fe.extensions.commands.list().map((c: any) => c.id as string));
}

interface Finding {
  mode: string;
  action: string;
  text: string;
}

const known = (f: Finding) => MATRIX_KNOWN.some((k) => (k.mode === "*" || k.mode === f.mode) && (k.action === "*" || f.action.startsWith(k.action)) && k.pattern.test(f.text));

/** Open the base document, put the modes in place and remember the document so it can be put back after an action. */
async function prepare(page: Page, ms: Mode[]): Promise<void> {
  const query = ms.map((m) => m.query ?? "").join("");
  await page.addInitScript(() => void ((window as any).print = () => undefined));
  // Commands that ask a server (the dictionary, the thesaurus, a DOI) get a refusal at once: the matrix checks Docxtor, not the network.
  await page.route(/^https?:\/\/(?!localhost|127\.0\.0\.1|fonts\.)/, (route) => route.abort("internetdisconnected"));
  await openBase(page, query);
  for (const m of ms) await m.apply(page);
  await focusDocument(page);
  await inPage(page, (w: any) => {
    const ed = w.fe.editor;
    const g = window as any;
    g.__base = { bytes: ed.engine.save(), engine: ed.engine, revision: ed.engine.revision };
  });
}

/** After an action: the dialogs closed, the document put back if it changed or was replaced, the modes in place again. */
async function restore(page: Page, ms: Mode[]): Promise<void> {
  await settle(page);
  const changed = await inPage(page, (w: any) => {
    const g = window as any;
    const ed = w.fe.editor;
    return ed.engine !== g.__base.engine || ed.engine.revision !== g.__base.revision;
  });
  if (changed) {
    await inPage(page, async (w: any) => {
      const g = window as any;
      await w.fe.open(g.__base.bytes, "Matrix.docx");
      g.__base.engine = w.fe.editor.engine;
      g.__base.revision = w.fe.editor.engine.revision;
    });
    await settle(page);
  }
  // View, Viewing holds the window read-only until View, Editing: that is that command's mode, not the next action's.
  if (!ms.some((m) => m.id === "viewing")) await page.evaluate(() => (window as any).docxtor.fe.extensions.execute("view.editing").catch(() => undefined));
  for (const m of ms) await m.apply(page).catch(() => undefined);
  // The keyboard is in the document before every action, so that a popup that loses it is the popup's doing (rule U6).
  await focusDocument(page);
}

/**
 * The first half of rule U5: what is on screen is what the engine says, so a pane drawn from nothing (a second pane of a split window, drawn
 * with the same code) shows each paragraph as the step-by-step drawing of this pane does. Skipped while the window is split already or a
 * header, footer or note is being edited (the second pane would not be drawn from nothing then).
 */
async function redrawCheck(page: Page): Promise<string[]> {
  return inPage(page, async (w: any) => {
    const fe = w.fe;
    const ed = fe.editor;
    if (!ed.engine || ed.story || fe.split?.on || fe.view !== "print" || ed.grid?.on) return [];
    const texts = (root: HTMLElement) => {
      const out = new Map<string, string>();
      for (const p of root.querySelectorAll<HTMLElement>("[data-p]")) if (!p.closest("[data-tab-hidden], [data-fold]")) out.set(p.dataset.p!, (p.textContent ?? "").replace(/\s+/g, " "));
      return out;
    };
    const mine = texts(ed.flow);
    fe.split.open();
    try {
      const other = fe.split.editor;
      if (!other || other === ed) return [];
      other.layoutNow?.();
      const fresh = texts(other.flow);
      const out: string[] = [];
      for (const [id, t] of fresh) {
        const m = mine.get(id);
        if (m === undefined) out.push(`U5: paragraph ${id} ("${t.slice(0, 40)}") is drawn from nothing but not in the pane drawn step by step`);
        else if (m !== t) {
          let i = 0;
          while (i < m.length && m[i] === t[i]) i++;
          const at = Math.max(0, i - 20);
          out.push(`U5: paragraph ${id} shows "…${m.slice(at, i + 30)}" but a pane drawn from nothing shows "…${t.slice(at, i + 30)}" (from character ${i})`);
        }
        if (out.length > 3) break;
      }
      for (const [id, t] of mine) if (!fresh.has(id) && out.length <= 3) out.push(`U5: paragraph ${id} ("${t.slice(0, 40)}") is in the pane drawn step by step but not drawn from nothing`);
      return out;
    } finally {
      fe.split.close();
      ed.focus();
    }
  });
}

async function focusDocument(page: Page) {
  await inPage(page, (w: any) => {
    const ed = w.fe.editor;
    if (ed.root.contains(document.activeElement) || ed.canvas?.contains(document.activeElement)) return;
    ed.focus();
  });
}

async function runMatrix(page: Page, label: string, ms: Mode[], actions: Action[]): Promise<Finding[]> {
  const findings: Finding[] = [];
  // Read-only for the whole document: a window-wide reason, or a pane's own (Reading) with no second pane that can still edit.
  const split = ms.some((m) => m.group === "window" && m.id.startsWith("split"));
  const locked = ms.some((m) => m.readOnly && m.group !== "view") || (ms.some((m) => m.readOnly && m.group === "view") && !split);
  let prev = "the start";
  for (const a of actions) {
    const before = await markBefore(page);
    takeErrors(page);
    let thrown: string | null = null;
    try {
      await a.run(page);
    } catch (e) {
      thrown = (e as Error).message.split("\n")[0];
    }
    await settle(page);
    const problems = await checkApp(page, { revisionBefore: before, locked: locked && !ms.some((m) => m.exits?.includes(a.name)), settled: true });
    problems.push(...(await redrawCheck(page)));
    if (thrown) problems.unshift(/^U\d/.test(thrown) ? thrown : `U1: ${thrown}`);
    for (const text of problems) findings.push({ mode: label, action: `${a.name} (the action before was ${prev})`, text });
    prev = a.name;
    await restore(page, ms);
  }
  return findings;
}

function report(findings: Finding[]): string {
  const real = findings.filter((f) => !known(f));
  const by = new Map<string, Finding[]>();
  for (const f of real) by.set(`${f.text}`, [...(by.get(f.text) ?? []), f]);
  return [...by.entries()].map(([text, fs]) => `${text}\n    in ${fs.map((f) => `${f.mode} / ${f.action}`).slice(0, 6).join("; ")}${fs.length > 6 ? ` (and ${fs.length - 6} more)` : ""}`).join("\n");
}

test.describe("matrix", () => {
  test.skip(!ON, "the matrix takes 15 to 30 minutes: turn it on with DOCXTOR_MATRIX=1 or DOCXTOR_MATRIX_MODES=reading,split");
  test.describe.configure({ mode: "parallel" });

  for (const mode of modes) {
    test(`mode ${mode.id}, every action`, async ({ page }) => {
      test.setTimeout(45 * 60_000);
      try {
        await prepare(page, [mode]);
      } catch (e) {
        if (e instanceof NotHere) test.skip(true, e.message);
        throw e;
      }
      const ids = (await listCommands(page)).filter((id) => !SKIP_COMMANDS[id]);
      let actions = [...ids.map((id) => commandAction(id, mode.id)), ...ACTIONS];
      if (slice) {
        if (slice === "list") console.log(actions.map((a, i) => `${i} ${a.name}`).join("\n"));
        // One range or several, comma separated: `0:308,450:460` keeps both stretches, in order.
        const ranges = slice === "list" ? [] : slice.split(",").map((r) => r.split(":").map(Number));
        actions = [...ranges.flatMap(([lo, hi]) => actions.slice(lo, hi)), ...then.map((name) => actions.find((a) => a.name === name)!).filter(Boolean)];
      } else if (onlyActions) actions = actions.filter((a) => onlyActions.some((s) => a.name.includes(s)));
      const findings = await runMatrix(page, mode.id, [mode], actions);
      if (confirmPass) {
        // The second pass: each command that opened a dialog again, pressing the dialog's confirm button.
        const again = [...(opened.get(mode.id) ?? [])].map((id) => commandAction(id, mode.id, true));
        findings.push(...(await runMatrix(page, mode.id, [mode], again)));
      }
      expect(report(findings), `${findings.length} findings in ${actions.length} actions`).toBe("");
    });
  }

  if (pairsOn) {
    for (let i = 0; i < modes.length; i++) {
      for (let j = i + 1; j < modes.length; j++) {
        const [a, b] = [modes[i], modes[j]];
        if (a.group === b.group) continue;
        test(`modes ${a.id} with ${b.id}, the short list`, async ({ page }) => {
          test.setTimeout(10 * 60_000);
          try {
            await prepare(page, [a, b]);
          } catch (e) {
            if (e instanceof NotHere) test.skip(true, e.message);
            throw e;
          }
          const findings = await runMatrix(page, `${a.id}+${b.id}`, [a, b], ACTIONS.filter((x) => x.short));
          expect(report(findings)).toBe("");
        });
      }
    }
  }

  // Rule U3: after entering and leaving any set of modes, in any order, the lock reasons, the view and the pane classes are what they were,
  // and a document opened next starts with its own reasons only. Each mode with a way out (`LEAVE`) alone, then each pair of different groups
  // left in both orders.
  test("U3: entering and leaving modes gives back the window it started from", async ({ page }) => {
    test.setTimeout(40 * 60_000);
    const leavable = modes.filter((m) => LEAVE[m.id]);
    const findings: string[] = [];
    let start: string[] = [];
    const fresh = async () => {
      await prepare(page, []);
      start = await windowState(page);
    };
    const diff = (label: string, now: string[]) => {
      const lost = start.filter((l) => !now.includes(l));
      const got = now.filter((l) => !start.includes(l));
      if (lost.length || got.length) findings.push(`U3: ${label}: ${got.join("; ")} (was ${lost.join("; ")})`);
      return !!(lost.length || got.length);
    };
    /** Enter the modes, leave them in the order given; false when a mode can't be set up here. */
    const run = async (enter: Mode[], leave: Mode[]) => {
      try {
        for (const m of enter) {
          await m.apply(page);
          await settle(page);
        }
      } catch (e) {
        if (e instanceof NotHere) {
          await fresh();
          return;
        }
        throw e;
      }
      for (const m of leave) {
        await LEAVE[m.id](page);
        await settle(page);
      }
      const label = `entering ${enter.map((m) => m.id).join(", then ")} and leaving ${leave.map((m) => m.id).join(", then ")}`;
      if (diff(label, await windowState(page))) await fresh();
    };
    await fresh();
    for (const m of leavable) await run([m], [m]);
    if (pairsOn) {
      for (let i = 0; i < leavable.length; i++) {
        for (let j = i + 1; j < leavable.length; j++) {
          const [a, b] = [leavable[i], leavable[j]];
          if (a.group === b.group) continue;
          await run([a, b], [b, a]);
          await run([a, b], [a, b]);
        }
      }
    }
    // A reason that belongs to the document (final, protection, a lock with the document's scope) goes with it: the next one starts clean.
    for (const m of modes.filter((x) => ["final", "protect-readonly", "protect-forms", "window-lock"].includes(x.id))) {
      await fresh();
      try {
        await m.apply(page);
      } catch (e) {
        if (e instanceof NotHere) continue;
        throw e;
      }
      await inPage(page, async (w: any) => {
        const g = window as any;
        await w.fe.open(g.__base.bytes, "Another.docx");
      });
      await settle(page);
      const reasons = (await windowState(page)).filter((l) => l.startsWith("reasons") || l.startsWith("pane"));
      const was = start.filter((l) => l.startsWith("reasons") || l.startsWith("pane"));
      if (reasons.join() !== was.join()) findings.push(`U3: ${m.id}, then another document opened: ${reasons.join("; ")} (was ${was.join("; ")})`);
    }
    expect(findings.filter((f) => !MATRIX_KNOWN.some((k) => k.mode === "*" && k.action === "U3" && k.pattern.test(f))).join("\n")).toBe("");
  });
});
