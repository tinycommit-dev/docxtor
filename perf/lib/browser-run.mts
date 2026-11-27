// The browser tier: build the production app, serve it, run the scenarios, turn the results into metrics and tables.
import { existsSync, mkdirSync, readFileSync, copyFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { execFileSync } from "node:child_process";
import { CACHE } from "./paths.mts";
import type { Load } from "./env.mts";
import { quiet } from "./env.mts";
import type { Metrics } from "./gate.mts";
import { br, gz } from "./sizes.mts";
import { median, round } from "./stats.mts";
import { runScenario, type Result, type Spec } from "./scenarios.mts";
import { serve, viteBuild, WORK, type Served } from "./build.mts";
import { cached } from "./docs.mts";
import { median as med } from "./stats.mts";
import type { Tier, Collected } from "./paths.mts";

export const DOCS = join(CACHE, "docs");

/** Compressed size of what a first visit fetched. */
export function sizeMetrics(buildDir: string, requested: Record<string, number>): Metrics {
  const m: Metrics = {};
  const groups: Record<string, { gz: number; br: number; raw: number; n: number }> = {};
  for (const path of Object.keys(requested)) {
    const f = join(buildDir, "." + path);
    if (!existsSync(f) || path === "/") continue;
    const b = readFileSync(f);
    // The spelling checker's WebAssembly (`spell_bg`) is its own group, not the engine's.
    const g = /\.js$/.test(path) ? "js" : /spell_bg[^/]*\.wasm$/.test(path) ? "spelling" : /\.wasm$/.test(path) ? "engine" : /\.css$/.test(path) ? "css" : /\.woff2?$/.test(path) ? "fonts" : "other";
    for (const k of [g, "firstVisit"]) {
      const o = (groups[k] ??= { gz: 0, br: 0, raw: 0, n: 0 });
      o.gz += gz(b);
      o.br += br(b);
      o.raw += b.byteLength;
      o.n++;
    }
  }
  for (const [k, o] of Object.entries(groups)) {
    m[`size.visit.${k}.gzip`] = o.gz;
    m[`size.visit.${k}.brotli`] = o.br;
    m[`size.visit.${k}.raw`] = o.raw;
    m[`size.visit.${k}.files`] = o.n;
  }
  return m;
}

const pick = (rs: Result[], f: (r: Result) => number | undefined): number => {
  const v = rs.map(f).filter((x): x is number => typeof x === "number" && Number.isFinite(x));
  return v.length ? median(v) : NaN;
};

/** Metrics (exact) and reported numbers (times, memory) for one document's repeats. */
export function summarize(doc: string, rs: Result[], metrics: Metrics, reported: Record<string, number>) {
  const k = `browser.${doc}`;
  const R = (name: string, f: (r: Result) => number | undefined) => {
    const v = pick(rs, f);
    if (Number.isFinite(v)) reported[`${k}.${name}`] = round(v, 2);
  };
  const M = (name: string, f: (r: Result) => number | undefined) => {
    const v = pick(rs, f);
    if (Number.isFinite(v)) metrics[`${k}.${name}`] = round(v, 3);
  };
  R("loadMs", (r) => r.loadMs);
  R("startFootprintMB", (r) => r.startFootprintMB);
  R("firstEditableMs", (r) => r.firstEditableMs);
  R("openMs", (r) => r.openMs);
  R("readyMs", (r) => r.readyMs);
  R("footprintMB", (r) => r.openedFootprintMB);
  R("peakFootprintMB", (r) => r.peakFootprintMB);
  R("settledMB", (r) => r.afterFootprintMB);
  R("afterGcMB", (r) => r.afterGcFootprintMB);
  R("saveMs", (r) => r.saveMs);
  R("undoMs", (r) => r.undo?.procMs);
  R("undoPaintMs", (r) => r.undo?.paintMs);
  // The blocks the far-pages bookkeeping looked at for the one undo, and the times it went over every block (exact).
  M("undo.farVisits", (r) => (r.undo ? r.undo.farVisits * r.undo.n : undefined));
  M("undo.farLong", (r) => (r.undo ? r.undo.farLong * r.undo.n : undefined));
  R("undoLocalMs", (r) => r.undoLocal?.procMs);
  M("undoLocal.farVisits", (r) => (r.undoLocal ? r.undoLocal.farVisits * r.undoLocal.n : undefined));
  M("undoLocal.farLong", (r) => (r.undoLocal ? r.undoLocal.farLong * r.undoLocal.n : undefined));
  if (doc === "start") M("startupNodesAdded", (r) => r.startupNodesAdded);
  for (const at of ["at50", "at95"]) {
    const g = (f: (x: Result["keys"][string]) => number) => (r: Result) => (r.keys[at] ? f(r.keys[at]) : undefined);
    // Near the end, what the keys cost depends on what else lands among them: a grammar answer, the spelling pass of a line that wraps,
    // the autosave the keys in the middle started (five runs of one build: layouts 1.05 to 1.15 a key, JSON 2,430 to 71,745 characters).
    // Those counts are reported there, not gated; in the middle, where nothing else is pending, they are exact and gated.
    const T = at === "at95" ? R : M;
    R(`${at}.ownMs`, g((x) => x.scriptMs + x.layoutMs + x.styleMs));
    R(`${at}.scriptMs`, g((x) => x.scriptMs));
    R(`${at}.layoutMs`, g((x) => x.layoutMs));
    R(`${at}.styleMs`, g((x) => x.styleMs));
    R(`${at}.engineMs`, g((x) => x.engineMs));
    R(`${at}.procMs`, g((x) => x.procMs));
    R(`${at}.procMax`, g((x) => x.procMax));
    R(`${at}.delayMs`, g((x) => x.delayMs));
    R(`${at}.paintMs`, g((x) => x.paintMs));
    R(`${at}.paintP90`, g((x) => x.paintP90));
    R(`${at}.eventTimingPerKey`, g((x) => x.eventTimingPerKey));
    R(`${at}.eventTimingProcMs`, g((x) => x.eventTimingProcMs));
    R(`${at}.peakMB`, g((x) => x.peakFootprintMB));
    T(`${at}.layouts`, g((x) => x.layouts));
    M(`${at}.layoutBlocks`, (r) => r.layoutBlocks?.[at]?.laidOut);
    M(`${at}.farBlocks`, (r) => r.layoutBlocks?.[at]?.far);
    T(`${at}.styles`, g((x) => x.styles));
    M(`${at}.farVisits`, g((x) => x.farVisits));
    M(`${at}.farLong`, g((x) => x.farLong));
    R(`${at}.nodesDelta`, g((x) => x.nodesDelta));
    R(`${at}.listenersDelta`, g((x) => x.listenersDelta));
    T(`${at}.engineCalls`, g((x) => x.engineCalls));
    T(`${at}.jsonIn`, g((x) => x.jsonIn));
    T(`${at}.jsonOut`, g((x) => x.jsonOut));
  }
  // The paginator: passes while typing one key a frame (full ones must be none), and the pass after one key and a pause.
  for (const at of ["at50", "at95"]) {
    M(`${at}.fullPasses`, (r) => r.typing?.[at]?.fullPasses);
  }
  // The layouts of opening the document (and of the fonts that arrive after it) and of one zoom: passes over the text and blocks measured, exact.
  M("open.fullPasses", (r) => r.opened?.fullPasses);
  M("open.incPasses", (r) => r.opened?.incPasses);
  M("open.measured", (r) => r.opened?.measured);
  M("zoom.fullPasses", (r) => r.zoomed?.fullPasses);
  M("zoom.incPasses", (r) => r.zoomed?.incPasses);
  M("zoom.measured", (r) => r.zoomed?.measured);
  R("zoom.layoutMs", (r) => r.zoomed?.layoutMs);
  M("pause.measured", (r) => (r.pause ? median(r.pause.measured) : undefined));
  M("pause.maxMeasured", (r) => (r.pause ? Math.max(...r.pause.measured) : undefined));
  // Reads of a rectangle or an offset from one key to the end of the pause's work, and the blocks they were of. Reported, not gated yet (the window is bounded by timers and the overlays
  // draw for the paragraphs in view, so it moves with how many fit on a screen): `pause.rectBlocks` is the one to watch, the same at every length, where a read of every block would grow with it.
  R("pause.rectReads", (r) => (r.pause?.rectReads?.length ? median(r.pause.rectReads) : undefined));
  R("pause.maxRectReads", (r) => (r.pause?.rectReads?.length ? Math.max(...r.pause.rectReads) : undefined));
  R("pause.rectBlocks", (r) => (r.pause?.rectBlocks?.length ? median(r.pause.rectBlocks) : undefined));
  R("pause.maxRectBlocks", (r) => (r.pause?.rectBlocks?.length ? Math.max(...r.pause.rectBlocks) : undefined));
  M("pause.fullPasses", (r) => r.pause?.fullPasses);
  M("pause.incrementalKeys", (r) => (r.pause ? r.pause.incremental.filter(Boolean).length : undefined));
  R("pause.longestMs", (r) => (r.pause ? median(r.pause.longestMs) : undefined));
  R("pause.worstMs", (r) => (r.pause ? Math.max(...r.pause.longestMs) : undefined));
  // Typing across an autosave: the longest stall of the main thread from the first key to the end of the second autosave, and the delay of the key typed while one runs. Times, so reported, but gated by a budget in perf/budgets.json (a save that holds the page for long is the failure this catches).
  R("autosave.longestMs", (r) => r.autosave?.longestMs);
  R("autosave.keyDelayMs", (r) => r.autosave?.keyDelayMs);
  R("autosave.longTasks", (r) => r.autosave?.longTasks);
  // Page elements and listeners move between identical runs (timers, spelling, the pages drawn around the caret), so they are reported, not gated.
  R("nodes", (r) => r.keys.at50?.nodes);
  R("listeners", (r) => r.keys.at50?.listeners);
  const sc = (f: (s: NonNullable<Result["scroll"]>) => number) => (r: Result) => (r.scroll ? f(r.scroll) : undefined);
  R("scroll.ms", sc((s) => s.ms));
  R("scroll.frames", sc((s) => s.frames));
  R("scroll.medianFrameMs", sc((s) => s.medianFrameMs));
  R("scroll.p95FrameMs", sc((s) => s.p95FrameMs));
  R("scroll.worstFrameMs", sc((s) => s.worstFrameMs));
  R("scroll.slowFrames", sc((s) => s.slowFrames));
  R("scroll.longTasks", sc((s) => s.longTasks));
  R("scroll.longTaskMs", sc((s) => s.longTaskMs));
  R("scroll.peakMB", sc((s) => s.peakFootprintMB));
  // The memory runs (docs/performance/index.md, "Memory of a tab"): footprint by phase, megabytes, reported; the budgets in perf/budgets.json gate them.
  const mm_ = (name: string, f: (m: NonNullable<Result["mem"]>) => number | undefined) => R(`mem.${name}`, (r) => (r.mem ? f(r.mem) : undefined));
  mm_("pages", (m) => m.pages);
  mm_("restMB", (m) => m.restMB);
  mm_("openPeakMB", (m) => m.openPeakMB);
  mm_("typePeakMB", (m) => m.typePeakMB);
  mm_("savePeakMB", (m) => m.savePeakMB);
  mm_("scrollPeakMB", (m) => Math.max(m.scrollDownPeakMB || 0, m.scrollUpPeakMB || 0));
  mm_("wasmEndMB", (m) => m.wasmScrolledMB);
  mm_("afterGcMB", (m) => m.dumpGc?.privateFootprintMB);
  mm_("v8MB", (m) => m.dumpScrolled?.v8);
  mm_("blinkMB", (m) => m.dumpScrolled?.blinkGC);
  mm_("mallocMB", (m) => m.dumpScrolled?.malloc);
  mm_("partitionMB", (m) => m.dumpScrolled?.partitionAlloc);
  const dump = (r: Result) => r.dumpAfter as Record<string, number> | undefined;
  for (const f of ["privateFootprintMB", "v8", "blinkGC", "malloc", "partitionAlloc"]) R(`dump.${f}`, (r) => dump(r)?.[f]);
}

export function table(rows: string[][]): string {
  return [`| ${rows[0].join(" | ")} |`, `|${rows[0].map(() => "---").join("|")}|`, ...rows.slice(1).map((r) => `| ${r.join(" | ")} |`)].join("\n");
}

export interface Plan {
  doc: string;
  reps: number;
  spec: Spec;
  /** For a corpus file: where it is. */
  source?: string;
  /** How long one run may take before it is called stuck (ms). */
  limitMs?: number;
}

export function plan(tier: Tier): Plan[] {
  const p: Plan[] = [
    { doc: "start", reps: tier === "full" ? 5 : 3, spec: { doc: "start" } },
    { doc: "blank", reps: tier === "full" ? 5 : 3, spec: { doc: "blank", keys: 60, dump: true } },
    { doc: "first-blank", reps: tier === "full" ? 7 : 5, spec: { doc: "blank", fast: true } },
    { doc: "first-bytes", reps: tier === "full" ? 7 : 5, spec: { doc: "series-1000", fast: true }, source: "series-1000" },
    { doc: "series-1000", reps: 3, spec: { doc: "series-1000", keys: 60, scroll: true, dump: true, autosave: true } },
    // The middle of the series: a count per key that grows with the length of the document shows between 1,000 and 5,000 paragraphs (layouts, style recalculations, JSON).
    { doc: "series-5000", reps: tier === "full" ? 3 : 1, spec: { doc: "series-5000", keys: tier === "full" ? 60 : 40, scroll: tier === "full" } },
    { doc: "war-and-peace", reps: tier === "full" ? 3 : 1, limitMs: 900_000, spec: { doc: "war-and-peace", keys: 40, scroll: true, dump: true, zoom: true, autosave: true } },
    // With far pages left out of the layout (two pages either side of the view): own work per key, layouts and style
    // recalculations per key, the stall after a pause, and an exact count of the blocks with a layout box, which does not grow with the length of the document.
    { doc: "series-1000-far", reps: 3, spec: { doc: "series-1000", far: 2, keys: 60, scroll: true, autosave: true } },
    { doc: "series-5000-far", reps: 1, spec: { doc: "series-5000", far: 2, keys: tier === "full" ? 60 : 40 } },
    { doc: "war-and-peace-far", reps: tier === "full" ? 3 : 1, limitMs: 900_000, spec: { doc: "war-and-peace", far: 2, keys: 40, scroll: true, zoom: true, autosave: true } },
  ];
  // Memory of a tab by size (a built thesis: styles, tables, pictures, footnotes; War and Peace is text). 300 keys in the middle, a save, a scroll down and back.
  const mem = (doc: string, limitMs = 900_000): Plan => ({ doc: `mem-${doc}`, reps: 1, limitMs, spec: { doc, memory: true, keys: 300, positions: [0.5], pause: false, scroll: true, dump: true, autosave: doc !== "thesis-3000" } });
  p.push(mem("thesis-10"), mem("thesis-100"), mem("thesis-1000"), mem("war-and-peace"));
  if (tier === "full") {
    p.push(mem("thesis-3000", 2_400_000));
    p.push(
      { doc: "series-100", reps: 3, spec: { doc: "series-100", keys: 60 } },
      { doc: "series-20000", reps: 2, limitMs: 900_000, spec: { doc: "series-20000", keys: 40, scroll: true } },
      { doc: "proust", reps: 1, limitMs: 2_400_000, spec: { doc: "proust", keys: 40, scroll: true, dump: true } },
      { doc: "proust-far", reps: 1, limitMs: 2_400_000, spec: { doc: "proust", far: 2, keys: 40, scroll: true } },
    );
    const poi = findCorpus("bug65649.docx");
    if (poi) p.push({ doc: "poi-bug65649", reps: 3, spec: { doc: "poi-bug65649", keys: 40, scroll: true }, source: poi });
  }
  return p;
}

function findCorpus(name: string): string | null {
  const root = join(homedir(), ".cache", "docxtor-corpus");
  if (!existsSync(root)) return null;
  try {
    const out = execFileSync("find", [root, "-name", name], { encoding: "utf8" }).trim().split("\n")[0];
    return out || null;
  } catch {
    return null;
  }
}

/** The file a scenario opens, as a name the harness server hands out. */
export async function docFile(p: Plan): Promise<string> {
  mkdirSync(DOCS, { recursive: true });
  if (p.source) {
    const name = basename(p.source).replace(/[^\w.-]/g, "_");
    copyFileSync(p.source, join(DOCS, name));
    return name;
  }
  const c = await cached(p.doc, DOCS);
  return basename(c.file);
}

export async function runPlan(app: Served, p: Plan, onResult?: (r: Result) => void): Promise<Result[]> {
  const file = p.spec.doc === "start" || p.spec.doc === "blank" ? "" : await docFile({ ...p, doc: p.spec.doc, source: p.source && p.source !== p.spec.doc ? p.source : undefined });
  const out: Result[] = [];
  for (let i = 0; i < p.reps; i++) {
    const r = await runScenario(app, p.spec, file, p.limitMs);
    out.push(r);
    onResult?.(r);
  }
  return out;
}

export async function collectBrowser(tier: Tier, load: Load): Promise<Collected> {
  const t0 = Date.now();
  // The documents are made by the engine, here in Node.
  const { loadEngine } = await import("../../packages/core/src/index.ts");
  await loadEngine(readFileSync(new URL("../../packages/core/wasm/docxtor_bg.wasm", import.meta.url)));
  mkdirSync(WORK, { recursive: true });
  const build = viteBuild(join(WORK, "head"), undefined, "head");
  const app = await serve(build.dir, DOCS);
  const metrics: Metrics = {};
  const reported: Record<string, number> = {};
  const notes: string[] = [];
  const all: Record<string, Result[]> = {};
  try {
    for (const p of plan(tier)) {
      const t = Date.now();
      try {
        all[p.doc] = await runPlan(app, p);
      } catch (e) {
        // A document that cannot be run today is a finding: say how far it got.
        const msg = String((e as Error).message ?? e).slice(0, 400);
        notes.push(`**${p.doc} did not complete: ${msg}** (${((Date.now() - t) / 1000).toFixed(0)} s in)`);
        console.log(`  browser ${p.doc}: FAILED: ${msg}`);
        continue;
      }
      summarize(p.doc, all[p.doc], metrics, reported);
      if (p.doc === "start") Object.assign(metrics, sizeMetrics(build.dir, all.start[0].firstVisitBytes));
      for (const r of all[p.doc]) for (const e of r.pageErrors) notes.push(`${p.doc}: page error: ${e}`);
      console.log(`  browser ${p.doc}: ${((Date.now() - t) / 1000).toFixed(0)} s`);
    }
    await againstBaseline(build.dir, app, reported, notes);
  } finally {
    await app.close();
  }
  reported["browser.firstEditableBlankMs"] = reported["browser.first-blank.firstEditableMs"];
  reported["browser.firstEditableBytesMs"] = reported["browser.first-bytes.firstEditableMs"];
  const ch = all.start[0].chromium;
  notes.push(`Browser: ${ch.kind}, ${ch.version} (${ch.path}). Production build, engine built without wasm-opt, offline service worker blocked, window 1280 by 900, served from 127.0.0.1.`);
  if (!quiet(load)) notes.push("The machine was busy: every time and memory figure in the browser tables is unreliable.");
  const sections = [{ title: "Browser: production build", body: browserTables(all, metrics, reported) }];
  if (reported["vsBaseline.blank.ownMs.now"] !== undefined) {
    const g = (k: string, w: string) => String(reported[`vsBaseline.blank.${k}.${w}`]);
    sections.push({ title: "Against the accepted baseline build (interleaved in this session, blank document, medians of 3)", body: table([["", "This build", "Baseline build"], ["Own work per key (ms)", g("ownMs", "now"), g("ownMs", "baseline")], ["Footprint with a blank document (MB)", g("footprintMB", "now"), g("footprintMB", "baseline")], ["Peak footprint (MB)", g("peakMB", "now"), g("peakMB", "baseline")], ["Page load to start screen (ms)", g("loadMs", "now"), g("loadMs", "baseline")]]) + (quiet(load) ? "" : "\n\nThe machine was busy: read this table as a rough direction only.") });
  }
  return { metrics, reported, sections, notes, seconds: (Date.now() - t0) / 1000 };
}

function browserTables(all: Record<string, Result[]>, metrics: Metrics, reported: Record<string, number>): string {
  const docs = Object.keys(all).filter((d) => d !== "start" && !d.startsWith("first-") && !d.startsWith("mem-"));
  const v = (d: string, k: string, dp = 1) => (reported[`browser.${d}.${k}`] === undefined ? "-" : String(round(reported[`browser.${d}.${k}`], dp)));
  const mm = (d: string, k: string) => { const x = metrics[`browser.${d}.${k}`] ?? reported[`browser.${d}.${k}`]; return x === undefined ? "-" : String(x); };
  const l: string[] = [];
  l.push(`Medians of the repeats (counts are exact; times and memory are reported only).`, ``);
  l.push(`**Time to first editable** (no pauses; navigation until the document accepts a keystroke; medians of ${all["first-blank"]?.length ?? 0})`, ``);
  l.push(table([["", "Blank document", "A 1,000-paragraph document opened from bytes"], ["Page load until the start screen is up (ms)", v("first-blank", "loadMs", 0), v("first-bytes", "loadMs", 0)], ["Open until it accepts a keystroke (ms)", v("first-blank", "readyMs", 0), v("first-bytes", "readyMs", 0)], ["**Navigation until it accepts a keystroke (ms)**", v("first-blank", "firstEditableMs", 0), v("first-bytes", "firstEditableMs", 0)]]));
  l.push(``, `**Start and open**`, ``);
  l.push(table([["", "Start screen", ...docs], ["Page load to start screen (ms)", v("start", "loadMs", 0), ...docs.map((d) => v(d, "loadMs", 0))], ["Open until first page is drawn (ms)", "-", ...docs.map((d) => v(d, "openMs", 0))], ["Open until it accepts a keystroke (ms)", "-", ...docs.map((d) => v(d, "readyMs", 0))], ["Navigation until it accepts a keystroke (ms)", "-", ...docs.map((d) => v(d, "firstEditableMs", 0))], ["Footprint at rest (MB)", v("start", "startFootprintMB", 0), ...docs.map((d) => v(d, "footprintMB", 0))], ["Footprint, peak while opening, typing, scrolling (MB)", "-", ...docs.map((d) => v(d, "peakFootprintMB", 0))], ["Footprint, settled at the end (MB)", v("start", "settledMB", 0), ...docs.map((d) => v(d, "settledMB", 0))], ["Footprint after a forced collection (MB)", v("start", "afterGcMB", 0), ...docs.map((d) => v(d, "afterGcMB", 0))], ["Save (ms)", "-", ...docs.map((d) => v(d, "saveMs", 0))], ["Undo what was typed, own processing (ms)", "-", ...docs.map((d) => v(d, "undoMs", 1))], ["Undo of typing in one place, own processing (ms)", "-", ...docs.map((d) => v(d, "undoLocalMs", 1))], ["Page elements (nodes)", "-", ...docs.map((d) => mm(d, "nodes"))]]));
  for (const at of ["at50", "at95"]) {
    l.push(``, `**Typing ${at === "at50" ? "in the middle" : "near the end"}, per key**`, ``);
    l.push(table([["", ...docs], ["Own work: script + layout + style (ms)", ...docs.map((d) => v(d, `${at}.ownMs`, 2))], ["of which script (ms)", ...docs.map((d) => v(d, `${at}.scriptMs`, 2))], ["of which layout (ms)", ...docs.map((d) => v(d, `${at}.layoutMs`, 2))], ["of which style (ms)", ...docs.map((d) => v(d, `${at}.styleMs`, 2))], ["Engine calls inside it (ms)", ...docs.map((d) => v(d, `${at}.engineMs`, 2))], ["Editor's event handling, median (ms)", ...docs.map((d) => v(d, `${at}.procMs`, 2))], ["Input delay, median (ms)", ...docs.map((d) => v(d, `${at}.delayMs`, 2))], ["Time to next paint, median (ms) *secondary, 16.7 ms frames*", ...docs.map((d) => v(d, `${at}.paintMs`, 1))], ["Time to next paint, slowest 10% (ms)", ...docs.map((d) => v(d, `${at}.paintP90`, 1))], ["Layouts per key", ...docs.map((d) => mm(d, `${at}.layouts`))], ["Top-level blocks with a layout box after the keys (exact)", ...docs.map((d) => mm(d, `${at}.layoutBlocks`))], ["Blocks left out of the layout (exact)", ...docs.map((d) => mm(d, `${at}.farBlocks`))], ["Style recalculations per key", ...docs.map((d) => mm(d, `${at}.styles`))], ["Nodes added over the run", ...docs.map((d) => mm(d, `${at}.nodesDelta`))], ["Event listeners added over the run", ...docs.map((d) => mm(d, `${at}.listenersDelta`))], ["JSON characters out of the engine per key", ...docs.map((d) => mm(d, `${at}.jsonOut`))], ["Engine calls per key", ...docs.map((d) => mm(d, `${at}.engineCalls`))], ["Event Timing entries per key (only events over 16 ms are delivered)", ...docs.map((d) => v(d, `${at}.eventTimingPerKey`, 2))]]));
  }
  const pd = docs.filter((d) => metrics[`browser.${d}.pause.measured`] !== undefined);
  if (pd.length) {
    l.push(``, `**One key and the pause after it** (5 keys in the middle, each waited out; the paginator starts at the block before the first one drawn and stops where a block is reached as the last pass reached it)`, ``);
    l.push(table([["", ...pd], ["Blocks measured by the pass after a key (median; exact)", ...pd.map((d) => mm(d, "pause.measured"))], ["Most blocks measured after any of the keys (exact)", ...pd.map((d) => mm(d, "pause.maxMeasured"))], ["Rectangle and offset reads from a key to the end of its pause, median *reported*", ...pd.map((d) => mm(d, "pause.rectReads"))], ["Most rectangle and offset reads after any of the keys *reported*", ...pd.map((d) => mm(d, "pause.maxRectReads"))], ["Top-level blocks those reads were of, median *reported*", ...pd.map((d) => mm(d, "pause.rectBlocks"))], ["Most blocks read after any of the keys *reported*", ...pd.map((d) => mm(d, "pause.maxRectBlocks"))], ["Keys whose pass started part-way down, of 5 (exact)", ...pd.map((d) => mm(d, "pause.incrementalKeys"))], ["Full passes over the 5 keys (exact)", ...pd.map((d) => mm(d, "pause.fullPasses"))], ["Full passes while the keys were typed a frame apart: middle, near the end (exact)", ...pd.map((d) => `${mm(d, "at50.fullPasses")}, ${mm(d, "at95.fullPasses")}`)], ["Longest stall from the key to the end of the pause's work, median (ms) *reported*", ...pd.map((d) => v(d, "pause.longestMs", 0))], ["Longest stall, worst of the 5 (ms) *reported*", ...pd.map((d) => v(d, "pause.worstMs", 0))]]));
  }
  const ad = docs.filter((d) => reported[`browser.${d}.autosave.longestMs`] !== undefined);
  if (ad.length) {
    l.push(``, `**Typing across an autosave** (a key, the autosave a second later, a key typed while it runs, and its autosave; budgets in perf/budgets.json)`, ``);
    l.push(table([["", ...ad], ["Longest stall of the main thread (ms) *reported, budgeted*", ...ad.map((d) => v(d, "autosave.longestMs", 0))], ["Delay of the key typed during the autosave (ms) *reported, budgeted*", ...ad.map((d) => v(d, "autosave.keyDelayMs", 1))], ["Tasks of 50 ms or more *reported*", ...ad.map((d) => v(d, "autosave.longTasks", 0))]]));
  }
  const od = docs.filter((d) => metrics[`browser.${d}.open.fullPasses`] !== undefined);
  if (od.length) {
    l.push(``, `**Layouts of opening and of one zoom** (the settled document after open, then a zoom to 87%; passes over the text and blocks measured are exact)`, ``);
    l.push(table([["", ...od], ["Full passes from open until settled (exact)", ...od.map((d) => mm(d, "open.fullPasses"))], ["Passes from part-way down at open (exact)", ...od.map((d) => mm(d, "open.incPasses"))], ["Blocks measured at open (exact)", ...od.map((d) => mm(d, "open.measured"))], ["Full passes for one zoom (exact)", ...od.map((d) => mm(d, "zoom.fullPasses"))], ["Passes from part-way down for one zoom (exact)", ...od.map((d) => mm(d, "zoom.incPasses"))], ["Blocks measured for one zoom (exact)", ...od.map((d) => mm(d, "zoom.measured"))], ["Layout time after the zoom (ms) *reported*", ...od.map((d) => v(d, "zoom.layoutMs", 0))]]));
  }
  l.push(``, `**Scrolling from the first page to the last** (2,400 px a frame, a hard fling)`, ``);
  const sd = docs.filter((d) => reported[`browser.${d}.scroll.frames`] !== undefined);
  l.push(table([["", ...sd], ["Frames", ...sd.map((d) => v(d, "scroll.frames", 0))], ["Total (ms)", ...sd.map((d) => v(d, "scroll.ms", 0))], ["Median frame (ms)", ...sd.map((d) => v(d, "scroll.medianFrameMs", 1))], ["95th percentile frame (ms)", ...sd.map((d) => v(d, "scroll.p95FrameMs", 1))], ["Worst frame (ms)", ...sd.map((d) => v(d, "scroll.worstFrameMs", 0))], ["Frames over 25 ms", ...sd.map((d) => v(d, "scroll.slowFrames", 0))], ["Long tasks (over 50 ms)", ...sd.map((d) => v(d, "scroll.longTasks", 0))], ["Time in long tasks (ms)", ...sd.map((d) => v(d, "scroll.longTaskMs", 0))], ["Peak footprint (MB)", ...sd.map((d) => v(d, "scroll.peakMB", 0))]]));
  l.push(``, `**Where the memory is** (Chrome's own memory dump of the tab, at the end; private footprint is the number that works on every platform)`, ``);
  const dd = ["blank", ...docs.filter((d) => d !== "blank")].filter((d) => reported[`browser.${d}.dump.privateFootprintMB`] !== undefined);
  l.push(table([["", ...dd], ["Private footprint by Chrome (MB)", ...dd.map((d) => v(d, "dump.privateFootprintMB", 0))], ["Footprint by macOS `footprint` (MB)", ...dd.map((d) => v(d, "settledMB", 0))], ["V8 (MB)", ...dd.map((d) => v(d, "dump.v8", 0))], ["Blink heap (MB)", ...dd.map((d) => v(d, "dump.blinkGC", 0))], ["malloc (MB)", ...dd.map((d) => v(d, "dump.malloc", 0))], ["PartitionAlloc (MB)", ...dd.map((d) => v(d, "dump.partitionAlloc", 0))]]));
  const md = Object.keys(all).filter((d) => d.startsWith("mem-"));
  if (md.length) {
    l.push(``, `**Memory of a tab by size** (footprint in MB, one run each, 300 keys in the middle, a save, a scroll down and back; the app's own far-pages rule; the targets are in perf/goals.json and the limits in perf/budgets.json)`, ``);
    l.push(table([["", ...md.map((d) => d.slice(4))], ["Pages", ...md.map((d) => v(d, "mem.pages", 0))], ["At rest after opening", ...md.map((d) => v(d, "mem.restMB", 0))], ["Peak while opening", ...md.map((d) => v(d, "mem.openPeakMB", 0))], ["Peak while typing", ...md.map((d) => v(d, "mem.typePeakMB", 0))], ["Peak while saving", ...md.map((d) => v(d, "mem.savePeakMB", 0))], ["Peak while scrolling down and back", ...md.map((d) => v(d, "mem.scrollPeakMB", 0))], ["After a forced collection", ...md.map((d) => v(d, "mem.afterGcMB", 0))], ["Engine's WebAssembly memory at the end", ...md.map((d) => v(d, "mem.wasmEndMB", 0))], ["After scrolling: V8 / Blink / malloc / PartitionAlloc", ...md.map((d) => ["v8MB", "blinkMB", "mallocMB", "partitionMB"].map((k) => v(d, `mem.${k}`, 0)).join(" / "))]]));
  }
  const sz = (k: string) => (metrics[`size.visit.${k}`] === undefined ? "-" : (metrics[`size.visit.${k}`] / 1e6).toFixed(2) + " MB");
  l.push(``, `**Download of a first visit** (compressed; exact)`, ``, table([["", "gzip", "brotli", "raw", "files"], ["JavaScript", sz("js.gzip"), sz("js.brotli"), sz("js.raw"), String(metrics["size.visit.js.files"] ?? "-")], ["Engine", sz("engine.gzip"), sz("engine.brotli"), sz("engine.raw"), "1"], ["Everything a first visit fetches", sz("firstVisit.gzip"), sz("firstVisit.brotli"), sz("firstVisit.raw"), String(metrics["size.visit.firstVisit.files"] ?? "-")]]));
  return l.join("\n");
}

/** Where `perf:accept` keeps the build it accepted, so the next runs can be put side by side with it. */
export const BASELINE_BUILD = join(CACHE, "baseline", "dist");

/**
 * The current build against the accepted baseline build, interleaved in this session (current, baseline, current...).
 * Reported only, and only believed when the machine is quiet. Without a saved baseline build this says so.
 */
async function againstBaseline(headDir: string, head: Served, reported: Record<string, number>, notes: string[]) {
  if (!existsSync(join(BASELINE_BUILD, "index.html"))) {
    notes.push("No baseline build is saved (npm run perf:accept saves one), so times and memory were not compared against the accepted build; only the counts were.");
    return;
  }
  const base = await serve(BASELINE_BUILD, DOCS);
  try {
    const rs: Record<string, Result[]> = { head: [], base: [] };
    try {
      for (let i = 0; i < 3; i++) {
        rs.head.push(await runScenario(head, { doc: "blank", keys: 60 }, ""));
        rs.base.push(await runScenario(base, { doc: "blank", keys: 60 }, ""));
      }
    } catch (e) {
      // A baseline build the harness can't drive must not end the
      // whole run: the counts and budgets above are what gate. Saying so loudly is enough; npm run perf:accept saves a new one.
      const why = (e as Error).message.split("\n")[0];
      notes.push(`The saved baseline build could not be run (${why}), so this build was not compared with it. It is probably older than the harness; npm run perf:accept saves a new one.`);
      console.error(`perf: WARNING, the baseline build could not be run: ${why}`);
      return;
    }
    const own = (r: Result) => (r.keys.at50 ? r.keys.at50.scriptMs + r.keys.at50.layoutMs + r.keys.at50.styleMs : NaN);
    const m = (k: string, f: (r: Result) => number) => {
      reported[`vsBaseline.blank.${k}.now`] = round(med(rs.head.map(f)), 2);
      reported[`vsBaseline.blank.${k}.baseline`] = round(med(rs.base.map(f)), 2);
    };
    m("ownMs", own);
    m("footprintMB", (r) => r.openedFootprintMB ?? NaN);
    m("peakMB", (r) => r.peakFootprintMB);
    m("loadMs", (r) => r.loadMs);
    void headDir;
  } finally {
    await base.close();
  }
}
