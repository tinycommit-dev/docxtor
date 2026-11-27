// The modes of the matrix (matrix.spec.ts): each one is a small function that puts the app in a state. `apply` must be
// safe to run again (the matrix runs it after every action to put the mode back), and it may throw `NotHere` when the
// document cannot be put in that state, which skips the mode instead of failing.
import { openBuilt } from "./docx-builder";
import { documentReady, inPage, type Page } from "../helpers";

/* eslint-disable @typescript-eslint/no-explicit-any */
export class NotHere extends Error {}

export type Group = "view" | "window" | "caret" | "doc" | "lock" | "interface";

export interface Mode {
  id: string;
  /** Modes of the same group replace each other, so they are never paired. */
  group: Group;
  /** The document is read-only while the mode holds (rule U2 is checked in it). */
  readOnly?: boolean;
  /** Added to the address when the document is opened: the interface language. */
  query?: string;
  /** The actions whose job is to lift the mode's lock (File, Mark as final again is Edit anyway): rule U2 does not hold for them. */
  exits?: string[];
  apply(page: Page): Promise<void>;
}

const exec = (page: Page, id: string, ...args: unknown[]) => inPage(page, (w: any, a: any) => w.fe.extensions.execute(a.id, ...a.args).then((r: any) => r.status), { id, args });
const toggled = (page: Page, id: string) => inPage(page, (w: any, i: string) => !!w.fe.extensions.command(i)?.toggled?.(), id);

/** The document every mode starts from: two sections with a header and footer, notes, a table, a text box and a content control. */
export async function openBase(page: Page, query = "") {
  await page.goto(`/?${query.replace(/^&/, "")}`);
  await page.waitForFunction(() => !!(window as any).docxtor);
  await openBuilt(page, {
    sections: [
      { headers: { default: "Header of the matrix" }, footers: { default: "Footer {PAGE}" }, paragraphs: ["First paragraph of the matrix document", "Second paragraph with a footnote here", "Third paragraph with an endnote here", "Fourth paragraph before the table", "Fifth paragraph with a control in it", "Sixth paragraph at the end"] },
    ],
  });
  await documentReady(page);
  await inPage(page, (w: any) => {
    const ed = w.fe.editor;
    const ids = [...ed.root.querySelectorAll("[data-p]")].map((e: any) => Number(e.dataset.p));
    const [p1, p2, p3, p4, p5] = ids;
    ed.exec([
      { op: "insert_footnote", at: { p: p2, o: 20 }, text: "A footnote of the matrix" },
      { op: "insert_endnote", at: { p: p3, o: 20 }, text: "An endnote of the matrix" },
    ]);
    ed.exec({ op: "insert_table", after: p4, rows: 2, cols: 2, data: [["a", "b"], ["c", "d"]] });
    ed.exec({ op: "insert_text_box", at: { p: p1, o: 3 }, w: 120, h: 60, text: "Inside a text box", floating: false });
    ed.exec({ op: "control", action: "insert", at: { p: p5, o: 6 }, to: { p: p5, o: 15 }, kind: "text" });
  });
}

const inRoot = (page: Page, selector: string) =>
  inPage(page, (w: any, s: string) => {
    const ed = w.fe.editor;
    const el = ed.root.querySelector(s) as HTMLElement | null;
    if (!el) return false;
    const p = el.dataset.p ?? (el.querySelector("[data-p]") as HTMLElement | null)?.dataset.p;
    if (!p) return false;
    ed.select({ p: Number(p), o: 0 });
    return true;
  }, selector);

const caretIn = (id: string, selector: string): Mode => ({
  id,
  group: "caret",
  async apply(page) {
    if (!(await inRoot(page, selector))) throw new NotHere(`no ${selector} in the document`);
  },
});

const story = (id: string, zone: string): Mode => ({
  id,
  group: "caret",
  async apply(page) {
    const ok = await inPage(page, (w: any, z: string) => {
      const ed = w.fe.editor;
      if (ed.readOnly) return false;
      ed.enterStory(z, 0);
      return !!ed.story;
    }, zone);
    if (!ok) throw new NotHere(`could not enter the ${zone}`);
  },
});

const setView = (id: string, readOnly = false): Mode => ({
  id,
  group: "view",
  readOnly,
  async apply(page) {
    await inPage(page, (w: any, v: string) => void w.fe.setView(v), id);
  },
});

const lock = (id: string, apply: (page: Page) => Promise<void>): Mode => ({ id, group: "lock", readOnly: true, apply });

const protect = (settings: Record<string, unknown>) => async (page: Page) => {
  const done = await inPage(page, (w: any, s: any) => {
    const ed = w.fe.editor;
    if (ed.engine.protectionInfo().enforced) return true;
    // A window that is already read-only (Reading, Viewing) can't change the document's protection: Word can't either.
    if (ed.readOnly) return false;
    ed.engine.apply({ op: "protection", action: "set", settings: s });
    w.fe.refreshProtection?.();
    return true;
  }, settings);
  if (!done) throw new NotHere("the window is read-only, so the protection can't be set");
};

export const MODES: Mode[] = [
  // views
  setView("print"),
  setView("pageless"),
  setView("reading", true),
  setView("outline"),
  { id: "draft", group: "view", async apply(page) { await exec(page, "view.mode.draft"); } },
  {
    id: "linefocus",
    group: "view",
    readOnly: true,
    async apply(page) {
      if (!(await toggled(page, "view.lineFocus"))) await exec(page, "view.lineFocus");
    },
  },
  { id: "pagegrid", group: "view", async apply(page) { if (!(await toggled(page, "view.zoomPages.2"))) await exec(page, "view.zoomPages.2"); } },
  { id: "fullscreen", group: "view", async apply(page) { if (!(await toggled(page, "view.fullScreen"))) await exec(page, "view.fullScreen").catch(() => undefined); } },
  { id: "compact", group: "view", async apply(page) { await inPage(page, (w: any) => w.fe.setCompact(true)); } },
  // window
  {
    id: "split",
    group: "window",
    async apply(page) {
      await inPage(page, (w: any) => { if (!w.fe.split.on) w.fe.split.open(); });
    },
  },
  {
    id: "split-second",
    group: "window",
    async apply(page) {
      await inPage(page, (w: any) => {
        if (!w.fe.split.on) w.fe.split.open();
        const second = w.fe.split.editor;
        const first = second.root.querySelector("[data-p]") as HTMLElement | null;
        if (first) second.select({ p: Number(first.dataset.p), o: 0 });
        second.focus();
      });
    },
  },
  {
    id: "sidebyside",
    group: "window",
    async apply(page) {
      if (await inPage(page, (w: any) => !!w.side?.on)) return;
      // A copy of the matrix document beside it, as from a file on the computer (no place to save it back to).
      const ok = await inPage(page, (w: any) => w.side.open({ name: "Beside.docx", bytes: w.fe.editor.engine.save() }));
      if (!ok) throw new NotHere("side by side did not open");
    },
  },
  lock("window-lock", (page) =>
    inPage(page, (w: any) => {
      // The release is kept, so the mode can be left again (rule U3).
      // A document opened since (the matrix puts its document back) has dropped the lock: take it again.
      if (!w.fe.lockReasons().includes("window-lock")) (window as any).__windowLock = w.fe.lock("window-lock", { scope: "document" });
    }),
  ),
  // where the caret is
  caretIn("body", ".flow > .fp"),
  caretIn("table-cell", ".flow td"),
  caretIn("textbox", ".flow .atom.textbox"),
  caretIn("control", ".flow .fc-ctl"),
  story("header", "header"),
  story("footer", "footer"),
  story("footnote", "notes"),
  story("endnote", "endnotes"),
  // the document
  {
    id: "tabs",
    group: "doc",
    async apply(page) {
      const n = await inPage(page, (w: any) => w.fe.editor.tabs.tabs.length);
      if (n > 1) return;
      await inPage(page, (w: any) => w.fe.editor.exec({ op: "tab", action: "add", name: "Second", first_name: "First" }));
    },
  },
  {
    id: "merge-preview",
    group: "doc",
    async apply(page) {
      if (await toggled(page, "mailings.preview")) return;
      // A letter merge with four recipients from a CSV file (kept in the document, so a document put back still has them), then Preview results.
      if ((await exec(page, "mailings.preview")) === "unavailable") {
        // Starting a merge changes the document: not in a window that is read-only.
        if (await inPage(page, (w: any) => w.fe.editor.readOnly)) throw new NotHere("a merge can't be started in a read-only window");
        await exec(page, "mailings.start.letters");
        const chooser = page.waitForEvent("filechooser", { timeout: 5000 });
        await exec(page, "mailings.recipients.from.docxtor.csv");
        await (await chooser).setFiles({ name: "people.csv", mimeType: "text/csv", buffer: Buffer.from("First Name,Last Name,State\nAda,Lovelace,CA\nGrace,Hopper,NY\nAlan,Turing,CA\nEdsger,Dijkstra,NL\n") });
        await page.waitForFunction(() => (window as any).docxtor.fe.extensions.command("mailings.preview")?.when?.() !== false, null, { timeout: 5000 });
        await exec(page, "mailings.preview");
      }
      if (!(await toggled(page, "mailings.preview"))) throw new NotHere("the merge preview did not start");
    },
  },
  { id: "protect-readonly", group: "lock", readOnly: false, apply: protect({ edit: "readOnly", enforce: true }) },
  { id: "protect-forms", group: "lock", readOnly: false, apply: protect({ edit: "forms", enforce: true }) },
  {
    // As a person does it (File, Mark as final), so the window learns the document is final.
    ...lock("final", async (page) => {
      if (!(await toggled(page, "file.markAsFinal"))) await exec(page, "file.markAsFinal");
      if (!(await toggled(page, "file.markAsFinal"))) throw new NotHere("the window is read-only, so the document can't be marked as final");
    }),
    exits: ["command file.markAsFinal"],
  },
  lock("host-readonly", (page) => inPage(page, (w: any) => w.fe.setReadOnly(true))),
  lock("viewing", (page) => exec(page, "view.viewing").then(() => undefined)),
  // the interface
  { id: "dark", group: "interface", async apply(page) { await inPage(page, (w: any) => w.fe.setTheme("dark")); } },
  { id: "light", group: "interface", async apply(page) { await inPage(page, (w: any) => w.fe.setTheme("light")); } },
  { id: "es", group: "interface", query: "&lang=es", async apply() {} },
  { id: "en-XA", group: "interface", query: "&lang=en-XA", async apply() {} },
  { id: "ar-XB", group: "interface", query: "&lang=ar-XB", async apply() {} },
];

/**
 * How to leave each mode that changes the window (rule U3: entering and leaving modes gives back the lock reasons, the view and the pane
 * classes it started with). Modes that only put the caret somewhere, change the document or set the interface language have none.
 */
/** Views and the zoom belong to a pane: the matrix enters them in the first pane, so it leaves them there (the keyboard goes back to it). */
const firstPane = (page: Page) => inPage(page, (w: any) => void (w.fe.split?.on && w.fe.split.host?.primary?.focus()));
const setViewBack = async (page: Page) => {
  await firstPane(page);
  await inPage(page, (w: any) => void w.fe.setView("print"));
};
export const LEAVE: Record<string, (page: Page) => Promise<void>> = {
  pageless: setViewBack,
  reading: setViewBack,
  outline: setViewBack,
  draft: setViewBack,
  linefocus: async (page) => {
    await firstPane(page);
    if (await toggled(page, "view.lineFocus")) await exec(page, "view.lineFocus");
  },
  pagegrid: async (page) => {
    await firstPane(page);
    await inPage(page, (w: any) => w.fe.editor.zoomer.setMode((window as any).__u3?.zoom ?? "width"));
  },
  fullscreen: async (page) => void ((await toggled(page, "view.fullScreen")) && (await exec(page, "view.fullScreen").catch(() => undefined))),
  compact: (page) => inPage(page, (w: any) => w.fe.setCompact(false)),
  split: (page) => inPage(page, (w: any) => void (w.fe.split.on && w.fe.split.close())),
  "split-second": (page) => inPage(page, (w: any) => void (w.fe.split.on && w.fe.split.close())),
  sidebyside: (page) => inPage(page, async (w: any) => void (w.side?.on && (await w.side.close()))),
  "window-lock": (page) =>
    inPage(page, () => {
      (window as any).__windowLock?.();
      (window as any).__windowLock = null;
    }),
  header: (page) => inPage(page, (w: any) => w.fe.editor.exitStory()),
  footer: (page) => inPage(page, (w: any) => w.fe.editor.exitStory()),
  footnote: (page) => inPage(page, (w: any) => w.fe.editor.exitStory()),
  endnote: (page) => inPage(page, (w: any) => w.fe.editor.exitStory()),
  "merge-preview": async (page) => void ((await toggled(page, "mailings.preview")) && (await exec(page, "mailings.preview"))),
  "protect-readonly": async (page) => void (await exec(page, "protection.stop")),
  "protect-forms": async (page) => void (await exec(page, "protection.stop")),
  final: async (page) => void ((await toggled(page, "file.markAsFinal")) && (await exec(page, "file.markAsFinal"))),
  "host-readonly": (page) => inPage(page, (w: any) => w.fe.setReadOnly(false)),
  viewing: async (page) => void (await exec(page, "view.editing")),
  dark: (page) => inPage(page, (w: any) => w.fe.setTheme((window as any).__u3?.theme ?? "system")),
  light: (page) => inPage(page, (w: any) => w.fe.setTheme((window as any).__u3?.theme ?? "system")),
};

/** What rule U3 compares: the view, the lock reasons of every pane, the pane classes and the zoom, as one line each. */
export async function windowState(page: Page): Promise<string[]> {
  return inPage(page, (w: any) => {
    const fe = w.fe;
    const ed = fe.editor;
    // Classes that follow the pointer, the keyboard or an animation, not a mode.
    const passing = /^(focus|focused|hover|dragging|busy|pop-in|scrolling|selecting|typing|caret-.*)$/;
    const cls = (el: Element | null | undefined) => (el ? [...el.classList].filter((c) => !passing.test(c)).sort().join(" ") : "-");
    const g = window as any;
    if (!g.__u3) g.__u3 = { zoom: ed.zoomer.mode, theme: fe.currentTheme };
    const panes = [ed, ...(fe.split?.on && fe.split.editor ? [fe.split.editor] : [])];
    return [
      `view ${fe.view}`,
      `zoom ${ed.zoomer.mode}`,
      `theme ${fe.currentTheme}`,
      `reasons ${[...fe.lockReasons()].sort().join(", ") || "none"}`,
      ...panes.map((p: any, i: number) => `pane ${i + 1}: read-only ${p.readOnly}, reasons ${[...p.lockReasons()].sort().join(", ") || "none"}`),
      `split ${!!fe.split?.on}, side by side ${!!w.side?.on}`,
      `docxtor ${cls(fe.surface?.closest(".docxtor"))}`,
      `canvas ${cls(ed.canvas)}`,
      `stage ${cls(ed.stage)}`,
      `host ${cls(document.querySelector(".editor-host"))}`,
    ];
  });
}
