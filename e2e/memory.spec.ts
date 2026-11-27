// Memory: editing for a long time must not make the page use more and more memory. Each test repeats one kind of
// work (typing and undoing, opening and closing a dialog, inserting and deleting a picture, switching documents,
// zooming, spell checking) many times in Chrome, and compares what the page holds after a warm-up with what it
// holds at the end. A bound is not zero: Chrome's heap and the engine's linear memory (which only ever grows)
// move by a little for reasons that are not leaks. What the bounds catch is growth per cycle.
//
// What is read, after a forced garbage collection each time (Chrome DevTools Protocol):
//  - the JavaScript heap (Runtime.getHeapUsage),
//  - documents, DOM nodes and event listeners (Memory.getDOMCounters),
//  - the engine's WebAssembly memory (its buffer's byteLength, caught as the engine is instantiated),
//  - the object URLs alive (URL.createObjectURL minus URL.revokeObjectURL, wrapped in an init script).
// The numbers for each kind of work are printed in the test output.
import type { CDPSession } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { buildDocx } from "./support/docx-builder";
import { chooseMenu, dialog, documentReady, expect, inPage, openFixture, test, type Page, CHROMIUM_ONLY, chromiumOnlyFile } from "./helpers";

chromiumOnlyFile(CHROMIUM_ONLY.memory);

const MOD = "ControlOrMeta";
const KB = 1024;
const MB = KB * KB;

interface Reading {
  heap: number;
  nodes: number;
  documents: number;
  listeners: number;
  wasm: number;
  urls: number;
}

/** What `window.__mem` (the init script) keeps. */
interface MemWindow {
  __mem: { urls: Set<string>; memories: WebAssembly.Memory[] };
}

/** Wrap the object URL calls and catch the engine's memory, before the app's own scripts run. */
async function watch(page: Page) {
  await page.addInitScript(() => {
    const mem = { urls: new Set<string>(), memories: [] as WebAssembly.Memory[] };
    (window as unknown as MemWindow).__mem = mem;
    const make = URL.createObjectURL.bind(URL);
    const revoke = URL.revokeObjectURL.bind(URL);
    URL.createObjectURL = (obj: Blob | MediaSource) => {
      const url = make(obj);
      mem.urls.add(url);
      return url;
    };
    URL.revokeObjectURL = (url: string) => {
      mem.urls.delete(url);
      revoke(url);
    };
    const note = (r: unknown) => {
      const out = (r as { instance?: WebAssembly.Instance }).instance ?? (r as WebAssembly.Instance);
      const m = (out as { exports?: { memory?: WebAssembly.Memory } }).exports?.memory;
      if (m instanceof WebAssembly.Memory) mem.memories.push(m);
      return r;
    };
    const inst = WebAssembly.instantiate.bind(WebAssembly) as (...a: unknown[]) => Promise<unknown>;
    (WebAssembly as unknown as Record<string, unknown>).instantiate = (...a: unknown[]) => inst(...a).then(note);
    if (WebAssembly.instantiateStreaming) {
      const stream = WebAssembly.instantiateStreaming.bind(WebAssembly) as (...a: unknown[]) => Promise<unknown>;
      (WebAssembly as unknown as Record<string, unknown>).instantiateStreaming = (...a: unknown[]) => stream(...a).then(note);
    }
  });
}

/** Let timers and animation frames run out, collect garbage, and read the counters. */
async function read(page: Page, cdp: CDPSession): Promise<Reading> {
  await page.evaluate(() => new Promise((done) => setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(done)), 400)));
  await cdp.send("HeapProfiler.collectGarbage");
  await cdp.send("HeapProfiler.collectGarbage");
  const heap = await cdp.send("Runtime.getHeapUsage");
  const dom = await cdp.send("Memory.getDOMCounters");
  const { wasm, urls } = await page.evaluate(() => {
    const m = (window as unknown as MemWindow).__mem;
    return { wasm: m.memories.reduce((sum, x) => sum + x.buffer.byteLength, 0), urls: m.urls.size };
  });
  return { heap: heap.usedSize, nodes: dom.nodes, documents: dom.documents, listeners: dom.jsEventListeners, wasm, urls };
}

/** How much a reading grew over another. */
const grew = (a: Reading, b: Reading): Reading => ({
  heap: b.heap - a.heap,
  nodes: b.nodes - a.nodes,
  documents: b.documents - a.documents,
  listeners: b.listeners - a.listeners,
  wasm: b.wasm - a.wasm,
  urls: b.urls - a.urls,
});

/** The most the page may grow between the warm-up and the end. */
interface Bounds {
  heap: number;
  nodes: number;
  listeners: number;
  wasm: number;
  urls: number;
  documents: number;
}
const LIMITS: Bounds = { heap: 2 * MB, nodes: 300, listeners: 60, wasm: 4 * MB, urls: 3, documents: 1 };

const pad = (s: string | number, n: number) => String(s).padStart(n);
const fmt = (n: number, unit: number) => (n / unit).toFixed(2);

interface Run {
  /** Name shown in the report. */
  name: string;
  /** Cycles after the warm-up. */
  cycles?: number;
  warmup?: number;
  limits?: Partial<Bounds>;
  /** Memory the undo history may take, in bytes: each step holds a copy of the document, and the default (128 MB) is more than a short test fills. */
  undoBudget?: number;
  /** Make the document and whatever the cycle needs. */
  setup: (page: Page) => Promise<void>;
  /** One round of the work. */
  cycle: (page: Page, i: number) => Promise<void>;
}

/** A long document: this many paragraphs of ordinary prose, with headings now and then. */
const longDocument = (paragraphs = 300) =>
  buildDocx({
    sections: [
      {
        paragraphs: Array.from({ length: paragraphs }, (_, i) =>
          i % 25 === 0
            ? { text: `Chapter ${i / 25 + 1}`, ppr: '<w:pStyle w:val="Heading1"/>' }
            : `Paragraph ${i} of the long document: the quick brown fox jumps over the lazy dog, and then the dog, tired of it all, sleeps through the rest of the afternoon in the sun.`,
        ),
      },
    ],
  });

async function openLong(page: Page, paragraphs = 300) {
  const bytes = Array.from(longDocument(paragraphs));
  await page.goto("/?open=simple.docx");
  await documentReady(page);
  await page.evaluate(async (b) => {
    await (window as unknown as { docxtor: { fe: { open(bytes: Uint8Array, name?: string): Promise<unknown> } } }).docxtor.fe.open(new Uint8Array(b), "long.docx");
  }, bytes);
  await expect(page.locator(".flow .fp").first()).toContainText("Chapter 1");
  await page.evaluate(() => document.fonts.ready);
}

/** Run a kind of work, print what it did to the page's memory, and hold it to its bounds. */
function measure(run: Run) {
  test(run.name, async ({ page }) => {
    test.setTimeout(240_000);
    await watch(page);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("HeapProfiler.enable");
    await run.setup(page);
    // The engine holds the undo history to a memory budget; a small one lets the history reach its bound while the test warms up.
    await page.evaluate((bytes) => (window as unknown as { docxtor: { fe: { editor: { engine: { raw: { setUndoBudget(n: number): void } } | null } } } }).docxtor.fe.editor.engine?.raw.setUndoBudget(bytes), run.undoBudget ?? 4 * MB);
    const warmup = run.warmup ?? 5;
    // DOCXTOR_MEMORY_CYCLES=200 repeats the work longer, to see whether a small drift is a leak.
    const cycles = Number(process.env.DOCXTOR_MEMORY_CYCLES) || (run.cycles ?? 30);
    for (let i = 0; i < warmup; i++) await run.cycle(page, i);
    // One save before the first reading: a save needs the most room the engine takes (its linear memory only ever grows), and an autosave
    // that falls inside the cycles on a busy machine would otherwise count as growth (1.8 MB in a quiet run, 3.5 to 5.7 MB when it fell
    // there). A save that leaks still shows: every autosave in the cycles would add to it.
    await page.evaluate(() => (window as unknown as { docxtor: { fe: { editor: { engine: { save(): Uint8Array } | null } } } }).docxtor.fe.editor.engine?.save());
    const start = await read(page, cdp);
    const marks: Reading[] = [];
    for (let i = 0; i < cycles; i++) {
      await run.cycle(page, warmup + i);
      if ((i + 1) % 10 === 0 && i + 1 < cycles) marks.push(await read(page, cdp));
    }
    const end = await read(page, cdp);
    const d = grew(start, end);
    const per = (n: number) => n / cycles;
    const lines = [
      `memory: ${run.name} (${cycles} cycles after ${warmup} to warm up)`,
      `  start   heap ${fmt(start.heap, MB)} MB, wasm ${fmt(start.wasm, MB)} MB, ${start.nodes} nodes, ${start.listeners} listeners, ${start.documents} documents, ${start.urls} object URLs`,
      ...marks.map((m, i) => `  +${pad((i + 1) * 10, 3)}    heap ${fmt(m.heap - start.heap, KB)} KB, wasm ${fmt(m.wasm - start.wasm, KB)} KB, nodes ${m.nodes - start.nodes}, listeners ${m.listeners - start.listeners}, object URLs ${m.urls - start.urls}`),
      `  growth  heap ${fmt(d.heap, KB)} KB (${fmt(per(d.heap), KB)} KB per cycle), wasm ${fmt(d.wasm, KB)} KB (${fmt(per(d.wasm), KB)} KB per cycle), nodes ${d.nodes} (${per(d.nodes).toFixed(2)} per cycle), listeners ${d.listeners} (${per(d.listeners).toFixed(2)} per cycle), documents ${d.documents}, object URLs ${d.urls}`,
    ];
    console.log(lines.join("\n"));
    const limit = { ...LIMITS, ...run.limits };
    expect.soft(d.heap, "JavaScript heap growth (bytes)").toBeLessThanOrEqual(limit.heap);
    expect.soft(d.nodes, "DOM node growth").toBeLessThanOrEqual(limit.nodes);
    expect.soft(d.listeners, "event listener growth").toBeLessThanOrEqual(limit.listeners);
    expect.soft(d.wasm, "engine memory growth (bytes)").toBeLessThanOrEqual(limit.wasm);
    expect.soft(d.urls, "object URLs left alive").toBeLessThanOrEqual(limit.urls);
    expect.soft(d.documents, "documents (frames) growth").toBeLessThanOrEqual(limit.documents);
  });
}

const type = (page: Page, text: string) => page.keyboard.type(text);
const press = (page: Page, key: string, times = 1) => page.keyboard.press(key, { delay: 0 }).then(async () => {
  for (let i = 1; i < times; i++) await page.keyboard.press(key);
});

/** Click at the end of the Nth paragraph of the page (puts the caret there). */
async function caretInParagraph(page: Page, index: number) {
  const p = page.locator(".flow .fp").nth(index);
  await p.scrollIntoViewIfNeeded();
  const box = (await p.boundingBox())!;
  await p.click({ position: { x: Math.max(2, box.width - 3), y: Math.min(8, box.height / 2) } });
  await page.keyboard.press("End");
}

test.describe("memory", () => {
  // The measurements run one after another in one browser, since the numbers are only meaningful when the page is not
  // starved by its neighbours (the config runs files fully parallel, so this has to be said here). Run the file alone
  // (`npm run test:memory`, one worker) so other spec files do not share the machine either.
  test.describe.configure({ mode: "serial" });

  measure({
    name: "typing, deleting, undo and redo in a long document",
    warmup: 8,
    undoBudget: 8 * MB,
    setup: (page) => openLong(page),
    cycle: async (page) => {
      await caretInParagraph(page, 3);
      await type(page, "memory test words ");
      await press(page, "Enter");
      await type(page, "a new line of text");
      await press(page, "Backspace", 18);
      await press(page, "Backspace");
      await press(page, "Backspace", 18);
      await press(page, `${MOD}+Z`, 3);
      await press(page, `${MOD}+Y`, 3);
      await press(page, `${MOD}+Z`, 3);
      await press(page, `${MOD}+Y`, 3);
    },
  });

  // -- dialogs -------------------------------------------------------------------------------------

  const dialogRun = (name: string, title: string | RegExp, open: (page: Page) => Promise<void>, setup?: (page: Page) => Promise<void>) =>
    measure({
      name: `opening and closing ${name}`,
      setup: async (page) => {
        await openFixture(page, "rich.docx");
        await setup?.(page);
      },
      cycle: async (page) => {
        await open(page);
        const d = dialog(page, title);
        await expect(d).toBeVisible();
        await page.keyboard.press("Escape");
        await expect(d).toBeHidden();
      },
    });

  dialogRun("the Font dialog", "Font", (page) => chooseMenu(page, "Format", "Font…"));
  dialogRun("the Paragraph dialog", "Paragraph", (page) => chooseMenu(page, "Format", "Paragraph…"));
  dialogRun("the Page setup dialog", "Page setup", (page) => chooseMenu(page, "File", "Page setup…"));
  dialogRun("the Insert table dialog", "Insert table", (page) => chooseMenu(page, "Format", "Table", "Insert table…"));

  measure({
    name: "opening and closing more of the dialogs, one after another",
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page) => {
      for (const [title, path] of [
        ["Word count", ["Tools", "Word count"]],
        ["Document details", ["File", "Document details…"]],
        ["Manage styles", ["Format", "Paragraph styles", "Manage styles…"]],
        ["Proofing options", ["Tools", "Spelling and grammar", "Proofing options…"]],
      ] as [string, [string, ...string[]]][]) {
        await chooseMenu(page, path[0], ...path.slice(1));
        const d = dialog(page, title);
        await expect(d).toBeVisible();
        await page.keyboard.press("Escape");
        await expect(d).toBeHidden();
      }
    },
  });

  measure({
    name: "opening and closing Find and replace",
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page) => {
      await page.locator(".flow .fp").first().click({ position: { x: 4, y: 8 } });
      await page.keyboard.press(`${MOD}+F`);
      const bar = page.getByRole("search");
      await expect(bar).toBeVisible();
      await bar.getByRole("searchbox", { name: "Find" }).fill("the");
      await expect(bar.locator(".find-count")).not.toBeEmpty();
      await page.keyboard.press("Escape");
      await expect(bar).toBeHidden();
    },
  });

  measure({
    name: "opening and closing the Format shape panel",
    setup: async (page) => {
      await openFixture(page, "rich.docx");
      await inPage(page, (w) => w.fe.editor.setZoom(1));
      await inPage(page, (w) => w.fe.editor.exec({ op: "insert_shape", at: { p: 2, o: 0 }, geometry: "rect", w: 100, h: 60, fill: "4472C4", line: { color: "2F528F", width_pt: 1 } }));
    },
    cycle: async (page) => {
      await page.locator(".flow .atom").first().click();
      await chooseMenu(page, "Format", "Shape", "Format shape…");
      const panel = page.locator(".dock").filter({ hasText: "Format shape" }).first();
      await expect(panel).toBeVisible();
      await panel.getByRole("button", { name: "Close" }).click();
      await expect(panel).toBeHidden();
    },
  });

  measure({
    name: "opening and closing the Styles panel",
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page) => {
      await chooseMenu(page, "Format", "Paragraph styles", "Styles panel");
      await expect(page.locator(".styles-panel")).toBeVisible();
      await page.locator(".sidebar").getByRole("tab", { name: "Files", exact: true }).click();
      await expect(page.locator(".styles-panel")).toBeHidden();
    },
  });

  measure({
    name: "opening and closing the Navigation pane",
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page) => {
      const sidebar = page.locator(".sidebar");
      await sidebar.getByRole("tab", { name: "Navigation", exact: true }).click();
      await expect(page.locator(".nav-pane")).toBeVisible();
      await sidebar.getByRole("tab", { name: "Files", exact: true }).click();
      await expect(page.locator(".nav-pane")).toBeHidden();
    },
  });

  // -- pictures, a chart, a table ------------------------------------------------------------------

  measure({
    name: "inserting and deleting a picture, a chart and a table",
    warmup: 80,
    // A deleted picture stays in the engine's package so undo can bring it back, but the object URL that shows it is let go once the pictures pile up (see `Engine.releaseUnusedMedia`), so the URLs alive stay near the sweep size (16) whatever the number of rounds.
    limits: { urls: 16 },
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page, i) => {
      await inPage(
        page,
        async (w, n) => {
          const ed = w.fe.editor;
          // A small picture that differs every time, so each one is a new part of the document.
          const c = document.createElement("canvas");
          c.width = 64;
          c.height = 48;
          const g = c.getContext("2d")!;
          g.fillStyle = `hsl(${(n * 37) % 360} 70% 50%)`;
          g.fillRect(0, 0, 64, 48);
          g.fillStyle = "#fff";
          g.fillText(String(n), 4, 24);
          const png = await new Promise<Uint8Array>((done) => c.toBlob(async (b) => done(new Uint8Array(await b!.arrayBuffer())), "image/png"));
          const last = () => ed.engine!.renderAll().order.at(-1)!;
          // A picture and a table go in, and are deleted.
          ed.exec({ op: "insert_paragraph", text: "" });
          const holder = last();
          ed.exec({ op: "insert_image", at: { p: holder, o: 0 }, media: ed.engine!.addMedia("png", png), width: 80, height: 60 });
          ed.exec({ op: "insert_table", after: holder, rows: 4, cols: 3, data: [["a", "b", "c"]] });
          const order = ed.engine!.renderAll().order;
          ed.exec({ op: "delete_blocks", ids: order.slice(order.indexOf(holder)) });
          // A chart goes in and is deleted, then another goes in and is taken back with undo. The parts of a deleted chart stay
          // in the package for undo, but a chart operation only copies the parts it changes, not every chart ever made.
          ed.exec({ op: "insert_paragraph", text: "" });
          const chartHolder = last();
          ed.exec({ op: "insert_chart", at: { p: chartHolder, o: 0 }, kind: "column", title: "Sales", categories: ["A", "B", "C"], series: [{ name: "One", values: [1, 2, 3] }], width: 240, height: 140 });
          ed.exec({ op: "delete_blocks", ids: [chartHolder] });
          ed.exec({ op: "insert_paragraph", text: "" });
          ed.exec({ op: "insert_chart", at: { p: last(), o: 0 }, kind: "column", title: "Sales", categories: ["A", "B", "C"], series: [{ name: "One", values: [1, 2, 3] }], width: 240, height: 140 });
          ed.undo();
          ed.undo();
        },
        i,
      );
    },
  });

  // -- documents -----------------------------------------------------------------------------------

  /** Two documents in the Library, for the app to open and close. */
  const twoDocuments = async (page: Page) => {
    await openFixture(page, "rich.docx");
    const files = ["rich.docx", "complex.docx"].map((n) => Array.from(readFileSync(resolve(process.cwd(), "fixtures", n))));
    await page.evaluate(async (list) => {
      const w = (window as unknown as { docxtor: { files: { library: { write(p: string, b: Uint8Array): Promise<void> } } } }).docxtor;
      await w.files.library.write("rich.docx", new Uint8Array(list[0]!));
      await w.files.library.write("complex.docx", new Uint8Array(list[1]!));
    }, files);
  };
  const openLibrary = (page: Page, path: string) =>
    page
      .evaluate(async (p) => {
        const app = (window as unknown as { docxtor: { files: { library: unknown }; openRef(r: unknown): Promise<void> } }).docxtor;
        await app.openRef({ backend: app.files.library, path: p });
      }, path)
      .then(() => expect(page.locator(".flow .fp").first()).toBeVisible());

  measure({
    name: "switching between two documents",
    setup: twoDocuments,
    cycle: async (page, i) => {
      await openLibrary(page, i % 2 ? "complex.docx" : "rich.docx");
      await page.keyboard.press("End");
    },
  });

  measure({
    name: "opening documents and going back to the start screen",
    setup: twoDocuments,
    cycle: async (page, i) => {
      await openLibrary(page, i % 2 ? "complex.docx" : "rich.docx");
      // Leaving the document shows the start screen; opening one again brings the editor back.
      await page.evaluate(() => {
        location.hash = "#/";
      });
      await expect(page.locator(".start")).toBeVisible();
    },
  });

  // -- views and zoom ------------------------------------------------------------------------------

  measure({
    name: "changing zoom and views",
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page) => {
      await page.evaluate(async () => {
        const fe = (window as unknown as { docxtor: { fe: { setView(id: string): Promise<void>; editor: { setZoom(z: number): void } } } }).docxtor.fe;
        for (const z of [0.5, 1.5, 1]) fe.editor.setZoom(z);
        for (const v of ["pageless", "reading", "outline", "print"]) await fe.setView(v);
      });
    },
  });

  measure({
    name: "spell checking",
    warmup: 15,
    setup: (page) => openFixture(page, "rich.docx"),
    cycle: async (page, i) => {
      await page.evaluate((n) => {
        const ed = (window as unknown as { docxtor: { fe: { editor: { engine: { renderAll(): { order: number[] } }; exec(o: unknown): unknown } } } }).docxtor.fe.editor;
        // Text with misspelled words goes in and comes out again, a few times, so the checker has new paragraphs to look at.
        for (let k = 0; k < 10; k++) {
          ed.exec({ op: "insert_paragraph", text: `I will recieve the paracel number ${n}.${k} tomorow, and teh rest of it seperately.` });
          ed.exec({ op: "delete_blocks", ids: [ed.engine.renderAll().order.at(-1)!] });
        }
      }, i);
      await chooseMenu(page, "Tools", "Spelling and grammar", "Recheck document");
    },
  });
});

