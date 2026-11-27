// Driving Chromium for the browser tier: a session on one production build, with the measures of docs/performance/index.md.
import { chromium, type Browser, type BrowserContext, type CDPSession, type Page } from "@playwright/test";
import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { promisify } from "node:util";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { PERF_DIR } from "./gate.mts";
import { max, median, percentile } from "./stats.mts";

const run = promisify(execFile);
const probeSource = readFileSync(join(PERF_DIR, "lib", "probe.js"), "utf8");
export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1048576;

export interface Chromium {
  path: string;
  version: string;
  kind: string;
}

export class Session {
  browser!: Browser;
  context!: BrowserContext;
  page!: Page;
  cdp!: CDPSession;
  bcdp!: CDPSession;
  pid = 0;
  info!: Chromium;
  private sampling = false;
  private samples: number[] = [];

  static async launch(url: string): Promise<Session> {
    const s = new Session();
    s.browser = await chromium.launch({ headless: true });
    const path = chromium.executablePath();
    s.info = { path, version: s.browser.version(), kind: /headless_shell/.test(path) ? "chromium headless shell" : "chromium (new headless)" };
    // The offline service worker (which precaches everything) is blocked, so a visit is the same every time.
    s.context = await s.browser.newContext({ viewport: { width: 1280, height: 900 }, serviceWorkers: "block", permissions: ["clipboard-read", "clipboard-write"] });
    await s.context.addInitScript(probeSource);
    s.page = await s.context.newPage();
    s.cdp = await s.context.newCDPSession(s.page);
    s.bcdp = await s.browser.newBrowserCDPSession();
    await s.cdp.send("Performance.enable");
    void url;
    return s;
  }

  async close() {
    await this.browser.close();
  }

  /** Time (ms since navigation start) when `docxtor` exists: the start screen is up and the engine is loaded. */
  async load(url: string): Promise<number> {
    await this.page.goto(url, { waitUntil: "load" });
    await this.page.waitForFunction(() => !!(window as any).docxtor, null, { timeout: 120_000 });
    return this.page.evaluate(() => performance.now());
  }

  /** The renderer that holds the page: the one with the biggest footprint of those the browser lists. */
  async findPid(): Promise<number> {
    const info = (await this.bcdp.send("SystemInfo.getProcessInfo" as never)) as unknown as { processInfo: { type: string; id: number }[] };
    let best = 0;
    let bestMB = -1;
    for (const p of info.processInfo.filter((p) => p.type === "renderer")) {
      const f = await footprint(p.id);
      if (f > bestMB) [best, bestMB] = [p.id, f];
    }
    this.pid = best;
    return best;
  }

  /** Physical footprint of the tab in MB (macOS `footprint`: what Activity Monitor and Chrome's Task Manager show). */
  async footprintMB(): Promise<number> {
    return footprint(this.pid);
  }

  /** Sample the footprint every `every` ms until `stopSampling`. */
  startSampling(every = 500) {
    this.sampling = true;
    this.samples = [];
    void (async () => {
      while (this.sampling) {
        const f = await footprint(this.pid);
        if (f >= 0 && this.sampling) this.samples.push(f);
        await sleep(every);
      }
    })();
  }
  stopSampling(): number[] {
    this.sampling = false;
    return this.samples;
  }

  /** Let timers and frames run out, then read the footprint: what the tab holds at rest. */
  async settled(ms = 2500): Promise<number> {
    await this.page.evaluate((w) => new Promise((d) => setTimeout(() => requestAnimationFrame(() => requestAnimationFrame(d)), w)), ms);
    return this.footprintMB();
  }

  /** Chrome's own memory dump of the renderer: its private footprint (works on every platform) and what holds it. */
  async memoryDump(): Promise<{ privateFootprintMB: number; v8: number; blinkGC: number; malloc: number; partitionAlloc: number } | null> {
    const b = this.bcdp;
    try {
      await b.send("Tracing.start", { traceConfig: { recordMode: "recordUntilFull", includedCategories: ["-*", "disabled-by-default-memory-infra"], memoryDumpConfig: { triggers: [] } }, transferMode: "ReturnAsStream" } as never);
      await b.send("Tracing.requestMemoryDump" as never, { levelOfDetail: "detailed" } as never);
      const done = new Promise<{ stream: string }>((ok) => b.once("Tracing.tracingComplete" as never, ok as never));
      await b.send("Tracing.end");
      const { stream } = await done;
      let text = "";
      for (;;) {
        const r = (await b.send("IO.read" as never, { handle: stream } as never)) as unknown as { data: string; eof: boolean };
        text += r.data;
        if (r.eof) break;
      }
      await b.send("IO.close" as never, { handle: stream } as never);
      const events = (JSON.parse(text) as { traceEvents?: any[] }).traceEvents ?? (JSON.parse(text) as any[]);
      // The totals and the allocators come in separate events of the same dump (the allocators from the biggest one).
      let totals: any = null;
      let allocators: any = {};
      for (const e of events) {
        if (e.ph !== "v" || e.pid !== this.pid) continue;
        if (e.args?.dumps?.process_totals) totals = e.args.dumps.process_totals;
        const al = e.args?.dumps?.allocators;
        if (al && Object.keys(al).length > Object.keys(allocators).length) allocators = al;
      }
      if (!totals) return null;
      const size = (name: string) => {
        const a = allocators[name]?.attrs?.effective_size ?? allocators[name]?.attrs?.size;
        return a ? parseInt(a.value, 16) / MB : 0;
      };
      if (process.env.DOCXTOR_PERF_DEBUG) console.log("ALLOC", Object.keys(allocators).filter((k) => !k.includes("/")).map((k) => `${k}=${size(k).toFixed(1)}`).join(" "));
      if (process.env.DOCXTOR_PERF_DEBUG === "2") console.log("SUB", Object.keys(allocators).filter((k) => (k.split("/").length === 2 || k.startsWith("v8/") && k.split("/").length <= 3) && size(k) > 1).map((k) => `${k}=${size(k).toFixed(1)}`).join(" "));
      if (process.env.DOCXTOR_PERF_DEBUG === "3") console.log("ALL", Object.keys(allocators).filter((k) => size(k) > (Number(process.env.DUMP_MIN) || 1.5)).map((k) => `${k}=${size(k).toFixed(1)}`).join(" "));
      return { privateFootprintMB: parseInt(totals.private_footprint_bytes, 16) / MB, v8: size("v8"), blinkGC: size("blink_gc"), malloc: size("malloc"), partitionAlloc: size("partition_alloc") };
    } catch {
      return null;
    }
  }

  async metrics(): Promise<Record<string, number>> {
    const { metrics } = await this.cdp.send("Performance.getMetrics");
    return Object.fromEntries(metrics.map((m) => [m.name, m.value]));
  }

  /** Open a document the harness serves: write it to the Library and open it. Resolves when its first page is on screen. */
  async openBytes(url: string, name: string): Promise<{ openMs: number; startedAt: number }> {
    return this.page.evaluate(
      async ([u, n]) => {
        const f = (window as any).docxtor;
        const bytes = new Uint8Array(await (await fetch(u)).arrayBuffer());
        const t = performance.now();
        await f.files.library.mkdir("Perf").catch(() => {});
        await f.files.library.write(`Perf/${n}`, bytes);
        await f.openRef({ backend: f.files.library, path: `Perf/${n}` });
        await new Promise<void>((ok) => {
          const check = () => (document.querySelector(".flow .fp") ? ok() : setTimeout(check, 5));
          check();
        });
        return { openMs: performance.now() - t, startedAt: t };
      },
      [url, name] as const,
    );
  }

  async newBlank(): Promise<{ openMs: number; startedAt: number }> {
    return this.page.evaluate(async () => {
      const f = (window as any).docxtor;
      const t = performance.now();
      await f.createIn(f.files.library, "", "Perf.docx");
      await new Promise<void>((ok) => {
        const check = () => (document.querySelector(".flow .fp") ? ok() : setTimeout(check, 5));
        check();
      });
      return { openMs: performance.now() - t, startedAt: t };
    });
  }

  /** Type one key and wait until the model has it: the document accepts input. Resolves to ms since navigation start. */
  async firstKeyAccepted(): Promise<number> {
    // The key is in when the document's revision moves (one batch). Asking the engine for the whole text, or a render of everything, to see
    // it would cost the measurement 200 ms a look in Proust, and a render of everything also resets what the engine has drawn.
    await this.page.evaluate(() => ((window as any).__before = (window as any).docxtor.engine.revision));
    const p = await this.page.locator(".flow [data-p]").first();
    await p.click();
    await this.page.keyboard.press("x");
    await this.page.waitForFunction(() => (window as any).docxtor.engine.revision > (window as any).__before, null, { timeout: 120_000 });
    return this.page.evaluate(() => performance.now());
  }

  /** Put the caret at the end of the paragraph at `fraction` of the document (scrolling it into view). */
  async caretAt(fraction: number): Promise<number> {
    const id = await this.page.evaluate((f) => {
      const order = (window as any).docxtor.engine.renderAll().order as number[];
      return order[Math.min(order.length - 1, Math.floor(order.length * f))];
    }, fraction);
    // With far pages left out the paragraph has no box until the view is there: the editor takes the view to it (a person scrolls).
    await this.page.evaluate(async (pid) => {
      (window as any).docxtor.fe.editor.scrollParaIntoView?.(pid);
      await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
    }, id);
    const p = this.page.locator(`.flow [data-p="${id}"]`);
    await p.scrollIntoViewIfNeeded({ timeout: 60_000 });
    const box = (await p.boundingBox())!;
    await p.click({ position: { x: Math.max(1, box.width - 2), y: Math.min(8, box.height / 2) } });
    await sleep(300);
    return id;
  }

  /**
   * Keys typed a frame apart never reach the editor's typing pause (120 ms after the last key, `scheduleLayout`), so no layout runs between
   * them. The harness's own round trips can stall for longer than that on a busy machine, and then the pause's layout ran in the middle of
   * the measured keys (the 979-page thesis made 8.3 layouts a key instead of 1, one run in four of the same build). While held, the editor
   * schedules no pause; `releasePause` schedules the last one asked for, as it would have run after the last key. The pause itself is
   * measured by `pauseAfterKey`.
   */
  async holdPause() {
    await this.page.evaluate(() => {
      const ed = (window as any).docxtor?.fe?.editor;
      if (!ed || ed.__heldPause) return;
      const orig = ed.scheduleLayout;
      const held = { orig, asked: false, whole: false };
      ed.__heldPause = held;
      ed.scheduleLayout = (incremental = false) => {
        held.asked = true;
        if (!incremental) held.whole = true;
        window.clearTimeout(ed.layoutTimer);
      };
    });
  }

  async releasePause() {
    await this.page.evaluate(() => {
      const ed = (window as any).docxtor?.fe?.editor;
      const held = ed?.__heldPause;
      if (!held) return;
      delete ed.scheduleLayout;
      delete ed.__heldPause;
      if (held.asked) held.orig.call(ed, !held.whole);
    });
  }

  /** Type `n` characters one at a time, each given its own frame; report per key exact numbers. */
  async typeKeys(n: number, key = "x", mod = false): Promise<KeyResult> {
    await this.page.evaluate(() => (window as any).__perf.take());
    const before = await this.metrics();
    const eng: number[] = [];
    const paint: number[] = [];
    const delay: number[] = [];
    const proc: number[] = [];
    let prevEng = await this.page.evaluate(() => (window as any).__perf.engineMs());
    const calls0 = await this.page.evaluate(() => (window as any).__perf.meter());
    const far0 = await this.farWork();
    this.startSampling(400);
    for (let i = 0; i < n; i++) {
      await this.page.keyboard.press(mod ? `ControlOrMeta+${key}` : key);
      const r = await this.page.evaluate(async () => {
        const P = (window as any).__perf;
        const done = await P.settle();
        const ev = P.events.splice(0);
        const kd = ev.find((e: any) => e.type === "keydown");
        const own = ev.filter((e: any) => e.type !== "keyup");
        return { done, ts: kd?.ts ?? ev[0]?.ts ?? done, delay: kd?.delay ?? 0, proc: own.reduce((s: number, e: any) => s + (e.end - e.t0), 0), eng: P.engineMs() };
      });
      eng.push(r.eng - prevEng);
      prevEng = r.eng;
      paint.push(r.done - r.ts);
      delay.push(r.delay);
      proc.push(proc_(r.proc));
    }
    const peaks = this.stopSampling();
    const after = await this.metrics();
    const calls1 = await this.page.evaluate(() => (window as any).__perf.meter());
    const far1 = await this.farWork();
    const sum = (f: (c: any) => number, m: Record<string, any>) => Object.values(m).reduce((a: number, c) => a + f(c), 0);
    const taken = await this.page.evaluate(() => (window as any).__perf.take());
    const d = (k: string) => (after[k] - before[k]) / n;
    return {
      n,
      // Chrome's own counters over the run, per key (CDP Performance.getMetrics).
      scriptMs: d("ScriptDuration") * 1000,
      layoutMs: d("LayoutDuration") * 1000,
      styleMs: d("RecalcStyleDuration") * 1000,
      taskMs: d("TaskDuration") * 1000,
      layouts: d("LayoutCount"),
      styles: d("RecalcStyleCount"),
      nodesDelta: after.Nodes - before.Nodes,
      nodes: after.Nodes,
      listeners: after.JSEventListeners,
      listenersDelta: after.JSEventListeners - before.JSEventListeners,
      // Our own timing of the editor's handlers and the engine calls inside them.
      engineMs: median(eng),
      procMs: median(proc),
      procMax: max(proc),
      delayMs: median(delay),
      paintMs: median(paint),
      paintP90: percentile(paint, 90),
      eventTimingPerKey: taken.timing.filter((e: any) => e.name === "keydown" || e.name === "beforeinput" || e.name === "input").length / n,
      eventTimingProcMs: median(taken.timing.map((e: any) => e.proc)),
      eventTimingDelayMs: median(taken.timing.map((e: any) => e.delay)),
      peakFootprintMB: peaks.length ? max(peaks) : NaN,
      // The editor's real call sequence into the engine, per key (calls and JSON characters, from the meter in Engine).
      engineCalls: (sum((c) => c.n, calls1) - sum((c) => c.n, calls0)) / n,
      jsonIn: (sum((c) => c.inChars, calls1) - sum((c) => c.inChars, calls0)) / n,
      jsonOut: (sum((c) => c.outChars, calls1) - sum((c) => c.outChars, calls0)) / n,
      farVisits: (far1.visits - far0.visits) / n,
      farLong: (far1.long - far0.long) / n,
      callNames: Object.keys(calls1).filter((k) => (calls1[k].n - (calls0[k]?.n ?? 0)) / n >= 0.5).sort(),
    };
  }

  /** What the bookkeeping of far pages has done so far: the blocks it looked at (exact), and the times it went over every block (zeros in a build from before the records). */
  async farWork(): Promise<{ visits: number; long: number }> {
    return this.page.evaluate(() => {
      const pg = (window as any).docxtor.fe.editor.paginator;
      return { visits: pg.farVisits ?? 0, long: pg.farLong ?? 0 };
    });
  }

  /** The top-level blocks of the text with a layout box now, and the ones left out of the layout (exact: the same at every length of document when far pages are left out). */
  async layoutBlocks(): Promise<{ laidOut: number; far: number }> {
    return this.page.evaluate(() => {
      const ed = (window as any).docxtor.fe.editor;
      // The blocks: a run of far blocks is kept in one box (a child of the flow that is not a block), counted by its blocks.
      const kids = (Array.from(ed.flow.children) as HTMLElement[]).flatMap((k) => (k.classList.contains("far-box") ? (Array.from(k.children) as HTMLElement[]) : [k]));
      return { laidOut: kids.filter((k) => k.getClientRects().length > 0).length, far: ed.paginator.farBlocks ?? 0 };
    });
  }

  /** What the paginator has done so far (the page map's counts: see `PaginatorStats` in the editor). */
  async paginationStats(): Promise<PaginationStats> {
    // A build from before the page map has no counts: zeros.
    return this.page.evaluate(() => JSON.parse(JSON.stringify((window as any).docxtor.fe.editor.paginator.stats ?? { fullPasses: 0, incPasses: 0, synced: 0, measured: 0, lastMeasured: 0, lastIncremental: false, fallbacks: {} })));
  }

  /** Zoom to `z` (about the middle of the window) and wait for the layout that follows: what it did (exact counts) and how long it took. */
  async zoomOnce(z: number): Promise<{ fullPasses: number; incPasses: number; measured: number; layoutMs: number }> {
    const before = await this.paginationStats();
    const layoutMs = await this.page.evaluate(async (z) => {
      const ed = (window as any).docxtor.fe.editor;
      let ms = 0;
      const orig = ed.layoutNow.bind(ed);
      ed.layoutNow = (o: any) => {
        const a = performance.now();
        const v = orig(o);
        ms += performance.now() - a;
        return v;
      };
      ed.zoomer.set(z, "centre");
      await new Promise((ok) => setTimeout(ok, 1500));
      ed.layoutNow = orig;
      return ms;
    }, z);
    const after = await this.paginationStats();
    return { fullPasses: after.fullPasses - before.fullPasses, incPasses: after.incPasses - before.incPasses, measured: after.measured - before.measured, layoutMs };
  }

  /**
   * One key, then a pause: the pagination that follows it 120 ms later. Per key, the blocks that pass measured, whether it started
   * part-way down, the passes it took and the longest stall (the longest gap in the page's turns of the event loop) from the key
   * to the end of what the pause brings.
   */
  async pauseAfterKey(keys = 5, key = "x"): Promise<PauseResult> {
    const out: PauseResult = { n: keys, measured: [], incremental: [], fullPasses: 0, passes: 0, longestMs: [], rectReads: [], rectBlocks: [] };
    for (let i = 0; i < keys; i++) {
      // The layout of the keys before it is done, and the page is quiet.
      await sleep(600);
      const before = await this.paginationStats();
      await this.page.evaluate(() => {
        (window as any).__perf.lagStart();
        (window as any).__perf.rectsStart?.();
      });
      await this.page.keyboard.press(key);
      let after = before;
      for (let k = 0; k < 400; k++) {
        await sleep(50);
        after = await this.paginationStats();
        if (after.fullPasses + after.incPasses > before.fullPasses + before.incPasses) break;
      }
      // What the pass starts after it (fields, floats, the overlays) runs in the next moments.
      await sleep(400);
      out.longestMs.push(await this.page.evaluate(() => (window as any).__perf.lagStop()));
      const reads = await this.page.evaluate(() => (window as any).__perf.rectsStop?.() ?? { rects: 0, offsets: 0, blocks: 0 });
      out.rectReads.push(reads.rects + reads.offsets);
      out.rectBlocks.push(reads.blocks);
      after = await this.paginationStats();
      out.measured.push(after.lastMeasured);
      out.incremental.push(after.lastIncremental);
      out.fullPasses += after.fullPasses - before.fullPasses;
      out.passes += after.fullPasses + after.incPasses - before.fullPasses - before.incPasses;
    }
    return out;
  }

  /**
   * Type, wait for the autosave to begin, type again while it runs, and wait out the second autosave: the longest stall of the main
   * thread over all of it (the longest gap between two turns of a loop that does nothing else, so a task of any length shows) and the
   * time the second key waited before the page first looked at it. The Library autosaves a second after the last edit.
   */
  async autosaveStall(): Promise<AutosaveResult> {
    // The page is quiet: the pagination after the keys before it, and their spelling and grammar, are done.
    await sleep(1500);
    await this.page.evaluate(() => {
      // Note the moment a save begins: a short document saves in a few milliseconds now, too quickly to be seen writing by polling.
      const q = (window as any).docxtor.queue;
      const save = q.save.bind(q);
      (window as any).__saveBegun = false;
      q.save = (...a: unknown[]) => (((window as any).__saveBegun = true), save(...a));
      (window as any).__perf.take();
      (window as any).__perf.lagStart();
    });
    await this.page.keyboard.press("x");
    // The autosave has begun; the second key goes in while it runs, or straight after it when it was that quick.
    await this.page.waitForFunction(() => (window as any).__saveBegun === true || (window as any).docxtor.queue?.writing === true, null, { timeout: 30_000, polling: 1 });
    await this.page.keyboard.press("x");
    // The second key is the last: its autosave follows a second later; wait until the queue has written it and nothing is running.
    await this.page.waitForFunction(() => (window as any).docxtor.queue?.status === "saved", null, { timeout: 60_000, polling: 100 });
    await sleep(500);
    return this.page.evaluate(() => {
      const P = (window as any).__perf;
      const longest = P.lagStop();
      const t = P.take();
      const downs = t.events.filter((e: { type: string }) => e.type === "keydown");
      return { longestMs: longest, keyDelayMs: downs.length ? downs[downs.length - 1].delay : 0, longTasks: t.long.length };
    });
  }

  /** Scroll from the top to the bottom, a screen or two a frame. */
  async scroll(step = 2400, dir: 1 | -1 = 1): Promise<ScrollResult> {
    await this.page.evaluate(() => (window as any).__perf.take());
    this.startSampling(500);
    const before = await this.metrics();
    const r = await this.page.evaluate(async ([stepPx, dir]) => {
      const flow = document.querySelector(".flow") as HTMLElement;
      let el: HTMLElement | null = flow;
      // The scrolling box is the first ancestor that scrolls (the app's `.canvas`).
      while (el && !(/(auto|scroll)/.test(getComputedStyle(el).overflowY) && el.scrollHeight > el.clientHeight + 1)) el = el.parentElement;
      const sc = (el ?? document.scrollingElement) as HTMLElement;
      if (dir > 0) sc.scrollTop = 0;
      await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
      const total = sc.scrollHeight - sc.clientHeight;
      if (dir < 0) {
        sc.scrollTop = total;
        await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
      }
      const times: number[] = [];
      let stuck = 0;
      let lastTop = -1;
      let last = performance.now();
      const t0 = last;
      await new Promise<void>((done) => {
        const tick = () => {
          const now = performance.now();
          times.push(now - last);
          last = now;
          sc.scrollTop = dir > 0 ? Math.min(total, sc.scrollTop + stepPx) : Math.max(0, sc.scrollTop - stepPx);
          const here = sc.scrollTop;
          if ((dir > 0 ? here >= total - 1 : here <= 1) || (stuck += here === lastTop ? 1 : 0) > 20 || times.length > total / stepPx + 200) return done();
          lastTop = here;
          requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
      return { times, total, ms: performance.now() - t0 };
    }, [step, dir] as [number, number]);
    const after = await this.metrics();
    const taken = await this.page.evaluate(() => (window as any).__perf.take());
    const peaks = this.stopSampling();
    const t = r.times.slice(1);
    return {
      pixels: r.total,
      frames: t.length,
      ms: r.ms,
      medianFrameMs: median(t),
      p95FrameMs: percentile(t, 95),
      worstFrameMs: max(t),
      slowFrames: t.filter((x) => x > 25).length,
      longTasks: taken.long.length,
      longTaskMs: taken.long.reduce((s: number, e: any) => s + e.dur, 0),
      scriptMs: (after.ScriptDuration - before.ScriptDuration) * 1000,
      layoutMs: (after.LayoutDuration - before.LayoutDuration) * 1000,
      styleMs: (after.RecalcStyleDuration - before.RecalcStyleDuration) * 1000,
      peakFootprintMB: peaks.length ? max(peaks) : NaN,
    };
  }
}

const proc_ = (x: number) => x;

export interface KeyResult {
  n: number;
  scriptMs: number;
  layoutMs: number;
  styleMs: number;
  taskMs: number;
  layouts: number;
  styles: number;
  nodesDelta: number;
  nodes: number;
  listeners: number;
  listenersDelta: number;
  engineMs: number;
  procMs: number;
  procMax: number;
  delayMs: number;
  paintMs: number;
  paintP90: number;
  eventTimingPerKey: number;
  eventTimingProcMs: number;
  eventTimingDelayMs: number;
  peakFootprintMB: number;
  engineCalls: number;
  jsonIn: number;
  jsonOut: number;
  /** Blocks the far-pages bookkeeping looked at per key (exact), and how often it went over every block. */
  farVisits: number;
  farLong: number;
  callNames: string[];
}
/** The paginator's counts (see `PaginatorStats` in packages/editor/src/editor/layout.ts). */
export interface PaginationStats {
  fullPasses: number;
  incPasses: number;
  synced: number;
  measured: number;
  lastMeasured: number;
  lastIncremental: boolean;
  fallbacks: Record<string, number>;
}
/** Typing across an autosave (see `Session.autosaveStall`). */
export interface AutosaveResult {
  /** The longest the main thread was held, from the first key to the end of the second autosave (ms). */
  longestMs: number;
  /** How long the key typed while the autosave ran waited before the page looked at it (ms). */
  keyDelayMs: number;
  /** Tasks of 50 ms or more (Chrome's long tasks) in that time. */
  longTasks: number;
}
/** One key and the pause after it, `n` times (see `Session.pauseAfterKey`). */
export interface PauseResult {
  n: number;
  /** Per key: the blocks the pass after it measured, and whether it started part-way down. */
  measured: number[];
  incremental: boolean[];
  /** Over all the keys: passes made, of which full. */
  passes: number;
  fullPasses: number;
  /** Per key: the longest stall from the key to the end of the pause's work (ms). */
  longestMs: number[];
  /** Per key: reads of a rectangle or an offset (getBoundingClientRect, getClientRects, offsetTop...) from the key to the end of the pause's work. */
  rectReads: number[];
  /** Per key: how many top-level blocks of the text had a rectangle or offset read (the paragraphs the overlays draw for: those in and near the view). */
  rectBlocks: number[];
}
export interface ScrollResult {
  pixels: number;
  frames: number;
  ms: number;
  medianFrameMs: number;
  p95FrameMs: number;
  worstFrameMs: number;
  slowFrames: number;
  longTasks: number;
  longTaskMs: number;
  scriptMs: number;
  layoutMs: number;
  styleMs: number;
  peakFootprintMB: number;
}

/** `footprint` for a process, in MB; -1 when it cannot be read (not macOS, or the process is gone). */
export async function footprint(pid: number): Promise<number> {
  if (process.platform !== "darwin" || !pid) return -1;
  const f = join(tmpdir(), `docxtor-fp-${process.pid}.json`);
  try {
    await run("footprint", ["-j", f, "-p", String(pid)], { timeout: 30_000 });
    return (JSON.parse(readFileSync(f, "utf8")).processes[0].footprint as number) / MB;
  } catch {
    return -1;
  }
}
