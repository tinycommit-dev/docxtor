// One session on one build: load the app, open a document, type, undo, save, scroll; measure each step.
import { Session, sleep, type KeyResult, type ScrollResult, type Chromium, type PauseResult, type AutosaveResult, type PaginationStats } from "./browser.mts";
import { serve, type Served } from "./build.mts";

export interface Spec {
  /** `start` (just the start screen), `blank`, or a document served from the cache (`file` is its name there). */
  doc: "start" | "blank" | string;
  keys?: number;
  scroll?: boolean;
  /** Where in the document to type, as fractions (default the middle and near the end). */
  positions?: number[];
  /** Skip the one-key-and-a-pause measure (it waits out the pause five times). */
  pause?: boolean;
  /** Type across an autosave and measure the stall and the delay of a key (see `Session.autosaveStall`). */
  autosave?: boolean;
  /** Take Chrome's memory dumps. */
  dump?: boolean;
  /** Only the time to first editable: no pauses, no memory reads, stop after the first key is accepted. */
  fast?: boolean;
  /** After the run, zoom the document once (to 87%) and count what the layout that follows does. */
  zoom?: boolean;
  /** Leave pages far from the view out of the layout, keeping this many either side of the view (the test switch `__DOCXTOR_FAR_PAGES__`). */
  far?: number;
  /** The memory run (docs/performance/index.md, "Memory of a tab"): Chrome's memory dump and the engine's memory at each phase, the pages counted, a scroll down and back. */
  memory?: boolean;
  /** Switch on a planted regression in an engine built with the perf-plant feature: [code, value] (1 busy loop iterations, 2 megabytes on open, 3 second body copy). */
  plant?: [number, number];
}

export interface Result {
  doc: string;
  chromium: Chromium;
  loadMs: number;
  startupNodesAdded: number;
  startFootprintMB: number;
  /** Navigation until the document accepts a keystroke. */
  firstEditableMs?: number;
  /** The call to open until the document accepts a keystroke. */
  readyMs?: number;
  openMs?: number;
  openedFootprintMB?: number;
  openedPeakMB?: number;
  dumpOpened?: unknown;
  keys: Record<string, KeyResult>;
  /** What the paginator did while the keys were typed (one frame each), per place in the document. */
  typing?: Record<string, { fullPasses: number; passes: number }>;
  /** One key and the pause after it, in the middle of the document. */
  pause?: PauseResult;
  /** Typing across an autosave. */
  autosave?: AutosaveResult;
  /** The paginator's counts at the end. */
  pagination?: PaginationStats;
  /** The paginator's counts once the document is open and has settled (the layouts of the open, the fonts arriving after it). */
  opened?: PaginationStats;
  /** What one zoom cost: the passes and blocks of the layout that followed it, and its time. */
  zoomed?: { fullPasses: number; incPasses: number; measured: number; layoutMs: number };
  /** After the keys at each place: the blocks with a layout box, and the ones left out of the layout. */
  layoutBlocks?: Record<string, { laidOut: number; far: number }>;
  /** The memory run: what the tab holds, phase by phase (megabytes). */
  mem?: Mem;
  undo?: KeyResult;
  /** An undo of typing in one place, after the typing before it has settled (the one above follows the autosave typing; both take back typing in one place, since a click elsewhere starts a new undo step). */
  undoLocal?: KeyResult;
  saveMs?: number;
  scroll?: ScrollResult;
  afterFootprintMB?: number;
  afterGcFootprintMB?: number;
  peakFootprintMB: number;
  dumpAfter?: unknown;
  firstVisitBytes: Record<string, number>;
  pageErrors: string[];
}

export interface Dump {
  privateFootprintMB: number;
  v8: number;
  blinkGC: number;
  malloc: number;
  partitionAlloc: number;
}

export interface Mem {
  pages: number;
  /** The engine's linear memory on the page (megabytes) after the open, the typing and the scrolling. */
  wasmOpenedMB: number;
  wasmTypedMB: number;
  wasmScrolledMB: number;
  /** Footprint (`footprint`): at rest after the open, the most while opening, the most while typing, the most while saving, the most scrolling down and back. */
  restMB: number;
  openPeakMB: number;
  typePeakMB: number;
  savePeakMB: number;
  scrollDownPeakMB: number;
  scrollUpPeakMB: number;
  /** Elements in the document (`.flow *`) at the end of the typing and at the bottom of the scroll. */
  nodesTyped: number;
  nodesScrolled: number;
  dumpOpened: Dump | null;
  dumpTyped: Dump | null;
  dumpScrolled: Dump | null;
  dumpGc: Dump | null;
}

/** Where the running scenario is, so a stuck one can say where. */
export const progress = { stage: "", close: undefined as undefined | (() => Promise<void>) };

/** A scenario with a time limit: a stuck browser is a finding with a place, not a harness that never ends. */
export async function runScenario(app: Served, spec: Spec, docUrl?: string, limitMs = 240_000): Promise<Result> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const stuck = new Promise<never>(
    (_, no) =>
      (timer = setTimeout(() => {
        no(new Error(`scenario ${spec.doc} stuck at "${progress.stage}" after ${limitMs / 1000} s`));
        void progress.close?.().catch(() => {});
      }, limitMs)),
  );
  try {
    return await Promise.race([runScenarioInner(app, spec, docUrl), stuck]);
  } finally {
    clearTimeout(timer);
  }
}

async function runScenarioInner(app: Served, spec: Spec, docUrl?: string): Promise<Result> {
  progress.stage = "launch";
  const s = await Session.launch(app.url);
  progress.close = () => s.close();
  const errors: string[] = [];
  // Without `far` every page is drawn (-1), whatever the document's length: the rows without far pages are the comparison.
  // The memory run keeps the app's own rule (far pages left out above 100 pages) unless `far` says otherwise.
  if (!spec.memory || spec.far !== undefined) await s.context.addInitScript((n) => ((globalThis as any).__DOCXTOR_FAR_PAGES__ = n), spec.far ?? -1);
  s.page.on("pageerror", (e) => errors.push(String(e).slice(0, 200)));
  app.reset();
  try {
    progress.stage = "load";
    const loadMs = await s.load(app.url + "/");
    if (spec.fast) {
      const o = spec.doc === "blank" ? await s.newBlank() : await s.openBytes(`${app.url}/__perf/${docUrl}`, "Perf.docx");
      const at = await s.firstKeyAccepted();
      return { doc: spec.doc, chromium: s.info, loadMs, openMs: o.openMs, readyMs: at - o.startedAt, firstEditableMs: at, keys: {}, firstVisitBytes: {}, pageErrors: errors } as unknown as Result;
    }
    if (spec.plant) await s.page.evaluate(([c, v]) => (window as any).__wasmExports[0].setPerfPlant(c, v), spec.plant);
    await sleep(1500);
    const first = new Map(app.requested);
    progress.stage = "find process";
    await s.findPid();
    const r: Partial<Result> = { doc: spec.doc, chromium: s.info, loadMs, keys: {}, firstVisitBytes: Object.fromEntries(first), pageErrors: errors };
    r.startupNodesAdded = await s.page.evaluate(() => (window as any).__perf.added);
    progress.stage = "start footprint";
    r.startFootprintMB = await s.settled();
    let peak = r.startFootprintMB;
    if (spec.doc === "start") return await finish(r, peak, s);
    progress.stage = "open";
    let startedAt: number;
    s.startSampling(400);
    if (spec.doc === "blank") {
      const o = await s.newBlank();
      [r.openMs, startedAt] = [o.openMs, o.startedAt];
    } else {
      const o = await s.openBytes(`${app.url}/__perf/${docUrl}`, "Perf.docx");
      [r.openMs, startedAt] = [o.openMs, o.startedAt];
    }
    // The first key into the opened document: the time until it accepts input.
    progress.stage = "first key";
    const accepted = await s.firstKeyAccepted();
    r.readyMs = accepted - startedAt;
    // Navigation until the document accepts a key, without the harness's own pauses: the load, then the open.
    r.firstEditableMs = loadMs + r.readyMs;
    const openPeaks = s.stopSampling();
    if (openPeaks.length) peak = Math.max(peak, ...openPeaks);
    r.openedFootprintMB = await s.settled();
    peak = Math.max(peak, r.openedFootprintMB);
    r.opened = await s.paginationStats();
    if (spec.dump) r.dumpOpened = await s.memoryDump();
    const mem: Partial<Mem> = {};
    const wasmMB = () => s.page.evaluate(() => ((window as any).__wasmExports as any[]).reduce((t, x) => t + (x.memory?.buffer.byteLength ?? 0), 0) / 1048576);
    if (spec.memory) {
      mem.restMB = r.openedFootprintMB;
      mem.openPeakMB = Math.max(...openPeaks, r.openedFootprintMB);
      mem.pages = await s.page.evaluate(() => (window as any).docxtor.fe?.editor?.pages ?? -1);
      mem.wasmOpenedMB = await wasmMB();
      mem.dumpOpened = await s.memoryDump();
      r.mem = mem as Mem;
    }
    progress.stage = "typing";
    for (const at of spec.positions ?? [0.5, 0.95]) {
      await s.caretAt(at);
      const name = `at${Math.round(at * 100)}`;
      const p0 = await s.paginationStats();
      await s.holdPause();
      r.keys[name] = await s.typeKeys(spec.keys ?? 60);
      (r.layoutBlocks ??= {})[name] = await s.layoutBlocks();
      const p1 = await s.paginationStats();
      await s.releasePause();
      (r.typing ??= {})[name] = { fullPasses: p1.fullPasses - p0.fullPasses, passes: p1.fullPasses + p1.incPasses - p0.fullPasses - p0.incPasses };
      peak = Math.max(peak, r.keys[name].peakFootprintMB || 0);
      // One key and a pause, in the middle: the pagination a pause brings (the stall the keys one frame apart do not show).
      if (spec.memory && at === 0.5) {
        mem.typePeakMB = r.keys[name].peakFootprintMB;
        mem.nodesTyped = await s.page.evaluate(() => document.querySelectorAll(".flow *").length);
        await sleep(1500);
        mem.wasmTypedMB = await wasmMB();
        mem.dumpTyped = await s.memoryDump();
      }
      if (at === 0.5 && spec.pause !== false) r.pause = await s.pauseAfterKey(5);
    }
    if (spec.autosave) {
      progress.stage = "autosave";
      // Typed at the caret where the last keys went (a jump back to the middle with far pages on waits for the view to arrive).
      r.autosave = await s.autosaveStall();
    }
    r.pagination = await s.paginationStats();
    progress.stage = "undo";
    r.undo = await s.typeKeys(1, "z", true);
    if (spec.memory) s.startSampling(150);
    r.saveMs = await s.page.evaluate(() => {
      const t = performance.now();
      (window as any).docxtor.engine.save();
      return performance.now() - t;
    });
    if (spec.memory) {
      await sleep(600);
      mem.savePeakMB = Math.max(...s.stopSampling(), 0);
    }
    progress.stage = "undo in one place";
    await sleep(1800);
    await s.caretAt(0.5);
    await s.typeKeys(5);
    r.undoLocal = await s.typeKeys(1, "z", true);
    if (spec.scroll) {
      progress.stage = "scroll";
      r.scroll = await s.scroll();
      peak = Math.max(peak, r.scroll.peakFootprintMB || 0);
    }
    if (spec.memory) {
      progress.stage = "scroll up";
      mem.scrollDownPeakMB = r.scroll?.peakFootprintMB;
      mem.nodesScrolled = await s.page.evaluate(() => document.querySelectorAll(".flow *").length);
      const up = await s.scroll(2400, -1);
      mem.scrollUpPeakMB = up.peakFootprintMB;
      peak = Math.max(peak, up.peakFootprintMB || 0);
      await sleep(1500);
      mem.wasmScrolledMB = await wasmMB();
      mem.dumpScrolled = await s.memoryDump();
    }
    if (spec.zoom) {
      progress.stage = "zoom";
      r.zoomed = await s.zoomOnce(0.87);
    }
    return await finish(r, peak, s, spec.dump);
  } finally {
    await s.close();
  }
}

async function finish(r: Partial<Result>, peak: number, s: Session, dump?: boolean): Promise<Result> {
  progress.stage = "final footprint";
  r.afterFootprintMB = await s.settled(2500);
  // Chrome's dump at the same moment as the settled footprint (before the forced collection, which frees a lot).
  if (dump) r.dumpAfter = await s.memoryDump();
  await s.cdp.send("HeapProfiler.collectGarbage");
  r.afterGcFootprintMB = await s.settled(500);
  if (r.mem) r.mem.dumpGc = await s.memoryDump();
  r.peakFootprintMB = Math.max(peak, r.afterFootprintMB);
  return r as Result;
}

export { serve };
