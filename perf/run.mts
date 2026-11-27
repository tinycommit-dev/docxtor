// The performance harness. `npm run perf:quick`, `npm run perf`, `npm run perf:full` (docs/performance/index.md).
//
//   node --experimental-strip-types perf/run.mts quick|standard|full [--accept] [--aa N] [--force]
//
// quick     engine counts: native allocator counts, Node engine counters, download size of the engine. With every change.
// standard  quick plus the browser (production build): footprint, own work per key, layout counts, long documents. Before each step lands.
// full      everything: all sizes, Proust length, long runs. When nothing else is waiting.
import { appendFileSync, cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { gitHead, machine, quiet, type Load } from "./lib/env.mts";
import { applyBudgets, compare, PERF_DIR, readBaseline, readBudgets, readNoise, write, type Metrics, type Noise } from "./lib/gate.mts";
import { runNative } from "./lib/native.mts";
import { runNode } from "./lib/node.mts";
import { report, type Goal } from "./lib/report.mts";
import { wasmSizes } from "./lib/sizes.mts";
import { median, min, max, round, slope } from "./lib/stats.mts";

import { CACHE, type Collected, type Tier } from "./lib/paths.mts";
export { CACHE };
const WASM = join(PERF_DIR, "..", "packages", "core", "wasm", "docxtor_bg.wasm");
const SIZES: Record<Tier, number[]> = { quick: [100, 1000, 5000], standard: [100, 1000, 5000], full: [100, 1000, 5000, 20000] };

/** The engine tier (everything quick runs). `plant` runs the same with planted regressions on. */
export function collectQuick(tier: Tier, plant?: { native: Record<string, string>; node: string; wasm?: string }): Collected {
  const sizes = SIZES[tier];
  const t0 = Date.now();
  const nat = runNative(sizes, plant?.native);
  const nod = runNode(sizes.map((n) => `series-${n}`), { wasm: plant?.wasm, plant: plant?.node });
  const metrics: Metrics = { ...nat.metrics, ...nod.metrics, ...wasmSizes(plant?.wasm ?? WASM) };
  const reported: Record<string, number> = { ...nod.times };
  // Per-key cost as a slope over document size (the goal is flat).
  const xs = sizes.map((n) => n / 1000);
  const keyMs = sizes.map((n) => nod.times[`node.series-${n}.keyMiddle.ms`]);
  const keyJson = sizes.map((n) => metrics[`node.series-${n}.keyMiddle.jsonOutChars`]);
  const keyAllocs = sizes.map((n) => metrics[`native.${n}.key.allocs`]);
  reported["slope.node.keyMs.perThousandParagraphs"] = round(slope(xs, keyMs), 3);
  reported["slope.node.jsonOutChars.perThousandParagraphs"] = round(slope(xs, keyJson), 0);
  reported["slope.native.keyAllocs.perThousandParagraphs"] = round(slope(xs, keyAllocs), 0);
  const row = (label: string, f: (n: number) => string) => `| ${label} | ${sizes.map(f).join(" | ")} |`;
  const head = `| Paragraphs | ${sizes.map((n) => n.toLocaleString("en")).join(" | ")} |\n|---|${sizes.map(() => "---").join("|")}|`;
  const body = [
    `Per keystroke (an edit that joins the last undo step, then the changed render, as the editor does). Counts are exact; milliseconds are reported only.`,
    ``,
    head,
    row("Allocations per key", (n) => String(metrics[`native.${n}.key.allocs`])),
    row("Bytes requested per key", (n) => String(metrics[`native.${n}.key.bytes`])),
    row("Peak extra bytes per key (50 keys)", (n) => String(metrics[`native.${n}.key.peak`])),
    row("Whole-document copies per key", (n) => String(metrics[`native.${n}.key.clones`])),
    row("Paragraphs visited by document walks per key", (n) => String(metrics[`native.${n}.key.walked`])),
    row("JSON characters out per key", (n) => String(metrics[`node.series-${n}.keyMiddle.jsonOutChars`])),
    row("JSON characters in per key", (n) => String(metrics[`node.series-${n}.keyMiddle.jsonInChars`])),
    row("Engine calls per key", (n) => String(metrics[`node.series-${n}.keyMiddle.calls`])),
    row("Engine time per key (ms, typing in the middle)", (n) => String(nod.times[`node.series-${n}.keyMiddle.ms`])),
    row("Engine time per key (ms, near the end)", (n) => String(nod.times[`node.series-${n}.keyNearEnd.ms`])),
    row("Open (ms)", (n) => String(nod.times[`node.series-${n}.open.ms`])),
    row("Open and first render (ms)", (n) => String(nod.times[`node.series-${n}.open.readyMs`])),
    row("Undo a typed word (ms)", (n) => String(nod.times[`node.series-${n}.undoMs`])),
    row("Save (ms)", (n) => String(nod.times[`node.series-${n}.saveMs`])),
    row("Engine memory high-water (MB)", (n) => String(round(metrics[`node.series-${n}.wasmMemoryBytes`] / 1048576, 1))),
    ``,
    `Slope over document size: engine time per key grows by **${reported["slope.node.keyMs.perThousandParagraphs"]} ms per 1,000 paragraphs**; JSON per key by **${reported["slope.node.jsonOutChars.perThousandParagraphs"]} characters per 1,000 paragraphs**; allocations per key by **${reported["slope.native.keyAllocs.perThousandParagraphs"]} per 1,000 paragraphs**. Flat would be 0.`,
    ``,
    `Engine download: ${(metrics["size.engine.gzip"] / 1e6).toFixed(2)} MB gzip, ${(metrics["size.engine.brotli"] / 1e6).toFixed(2)} MB brotli, ${(metrics["size.engine.raw"] / 1e6).toFixed(2)} MB raw.`,
  ].join("\n");
  return { metrics, reported, sections: [{ title: "Engine: cost per operation and over document size", body }], notes: [], seconds: (Date.now() - t0) / 1000 };
}

async function collect(tier: Tier, load: Load): Promise<Collected> {
  const q = collectQuick(tier);
  if (tier === "quick") return q;
  const { collectBrowser } = await import("./lib/browser-run.mts");
  const b = await collectBrowser(tier, load);
  return { metrics: { ...q.metrics, ...b.metrics }, reported: { ...q.reported, ...b.reported }, sections: [...q.sections, ...b.sections], notes: [...q.notes, ...b.notes], seconds: q.seconds + b.seconds };
}

function spreadTable(runs: Metrics[]): Noise {
  const n: Noise = {};
  for (const k of Object.keys(runs[0])) {
    const v = runs.map((r) => r[k]).filter((x) => x !== undefined);
    n[k] = { min: min(v), max: max(v), median: median(v), runs: v.length };
  }
  return n;
}

async function main() {
  const tier = (process.argv[2] ?? "quick") as Tier;
  const flag = (f: string) => process.argv.includes(`--${f}`);
  const aa = Number(process.argv[process.argv.indexOf("--aa") + 1]) || 0;
  const load = machine();
  const loadOk = quiet(load);
  const t0 = Date.now();
  if (aa) {
    const runs: Metrics[] = [];
    for (let i = 0; i < aa; i++) {
      console.log(`A/A run ${i + 1} of ${aa}`);
      runs.push((await collect(tier, machine())).metrics);
    }
    const spread = spreadTable(runs);
    const old = readNoise();
    write("noise.json", { ...old, ...spread });
    const moved = Object.entries(spread).filter(([, s]) => s.max !== s.min);
    console.log(`${Object.keys(spread).length} metrics; ${moved.length} moved between identical runs:`);
    for (const [k, s] of moved) console.log(`  ${k}: ${s.min} to ${s.max}`);
    return;
  }
  const c = await collect(tier, load);
  const base = readBaseline();
  const noise = readNoise();
  const verdicts = applyBudgets(compare(c.metrics, base?.metrics, noise), { ...c.reported, ...c.metrics }, readBudgets());
  const goals = JSON.parse(readFileSync(join(PERF_DIR, "goals.json"), "utf8")) as Goal[];
  const values: Record<string, number> = { ...c.metrics, ...c.reported };
  const commit = gitHead();
  const date = new Date().toISOString();
  const seconds = (Date.now() - t0) / 1000;
  const md = report({ tier, commit, date, seconds, load, loadOk, verdicts, reported: c.reported, goals, values, notes: c.notes, baselineCommit: base?.commit, sections: c.sections });
  writeFileSync(join(PERF_DIR, "report.md"), md);
  const worse = verdicts.filter((v) => v.status === "worse");
  appendFileSync(join(PERF_DIR, "history.jsonl"), JSON.stringify({ commit, date, tier, seconds: Math.round(seconds), load: { load1: load.load1, perCore: load.perCore, free: load.memoryFreePercent }, failed: worse.map((v) => v.metric), key: keyNumbers(c) }) + "\n");
  if (flag("accept")) {
    const merged = { ...(base?.metrics ?? {}), ...c.metrics };
    write("baseline.json", { accepted: date, commit, load: { load1: load.load1, perCore: load.perCore, free: load.memoryFreePercent, busiest: load.busiest }, metrics: Object.fromEntries(Object.entries(merged).sort()) });
    // Keep the build that was accepted (outside the repo) so later runs can be compared with it side by side.
    if (tier !== "quick") {
      const dest = join(CACHE, "baseline", "dist");
      rmSync(dest, { recursive: true, force: true });
      mkdirSync(join(CACHE, "baseline"), { recursive: true });
      cpSync(join(CACHE, "work", "head"), dest, { recursive: true });
    }
    console.log(`Baseline accepted (${Object.keys(merged).length} metrics).`);
  }
  console.log(`${tier}: ${verdicts.length} gates, ${worse.length} worse, ${seconds.toFixed(0)} s. Report: perf/report.md`);
  if (!loadOk) console.log(`WARNING: the machine is busy (${(load.perCore * 100).toFixed(0)}% of cores): times and memory are not to be trusted; counts are fine.`);
  for (const v of worse) console.log(`  WORSE ${v.metric}: ${v.base} -> ${v.now} (allowed +${v.tol})`);
  if (worse.length && !flag("accept")) process.exit(1);
}

function keyNumbers(c: Collected) {
  const pick = ["native.1000.key.allocs", "native.1000.key.clones", "node.series-1000.keyMiddle.jsonOutChars", "size.engine.gzip"];
  return { ...Object.fromEntries(pick.filter((k) => k in c.metrics).map((k) => [k, c.metrics[k]])), ...Object.fromEntries(Object.entries(c.reported).filter(([k]) => /^(slope|browser\.(blank|series-1000|war-and-peace)\.(keyMiddle\.ownMs|footprintMB|readyMs))/.test(k))) };
}

if (import.meta.url === `file://${process.argv[1]}`) await main();
