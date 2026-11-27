// Planted regressions: the harness must catch each one (docs/performance/index.md, "Planted regressions").
//
//   node --experimental-strip-types perf/plants.mts
//
// The plants live in a build made with the `perf-plant` feature (never shipped): a busy loop on every edit, megabytes
// allocated and freed on open, a second copy of the whole body on every edit. Each is run through the same gates as
// `perf:quick` against the accepted baseline; the busy loop changes no count, so it is looked for the way times are
// looked for: interleaved against a clean build, in the same session, with a spread from clean against clean.
// Writes perf/plants.json (committed: the evidence).
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { machine, quiet } from "./lib/env.mts";
import { compare, PERF_DIR, readBaseline, readNoise } from "./lib/gate.mts";
import { runNode } from "./lib/node.mts";
import { median, min, max, round } from "./lib/stats.mts";
import { collectQuick } from "./run.mts";
import { buildPlant, PLANT_WASM } from "./build-plant.mts";

const base = readBaseline();
const noise = readNoise();
if (!base) throw new Error("accept a baseline first (npm run perf:accept)");
if (!existsSync(PLANT_WASM)) buildPlant();
const load = machine();
const out: Record<string, unknown> = { date: new Date().toISOString(), commit: base.commit, load, results: [] };
const results = out.results as unknown[];

// 1 and 2: counts.
for (const p of [
  { id: "allocate 20 MB on every open", native: { DOCXTOR_PLANT_ALLOC_MB: "20" }, node: "alloc_mb=20" },
  { id: "copy the whole body a second time on every edit", native: { DOCXTOR_PLANT_DOUBLE_CLONE: "1" }, node: "double_clone=1" },
]) {
  const c = collectQuick("quick", { native: p.native, node: p.node, wasm: PLANT_WASM });
  const worse = compare(c.metrics, base.metrics, noise).filter((v) => v.status === "worse");
  results.push({ plant: p.id, caught: worse.length > 0, gatesFailed: worse.length, examples: worse.slice(0, 8).map((v) => `${v.metric}: ${v.base} to ${v.now} (allowed +${v.tol})`) });
  console.log(`${p.id}: ${worse.length ? "CAUGHT by " + worse.length + " gates" : "NOT CAUGHT"}`);
}

// 3: the busy loop, by time. Interleaved: clean, planted, clean, planted...
const CLEAN = join(PERF_DIR, "..", "packages", "core", "wasm", "docxtor_bg.wasm");
const REPS = 7;
const clean: number[] = [];
const planted = new Map<number, number[]>();
const loops = [20_000, 60_000, 200_000, 600_000, 2_000_000];
for (let i = 0; i < REPS; i++) {
  clean.push(runNode(["series-1000"], { wasm: CLEAN, keys: 100, repeats: 1 }).times["node.series-1000.keyMiddle.ms"]);
  for (const n of loops) {
    const a = planted.get(n) ?? [];
    a.push(runNode(["series-1000"], { wasm: PLANT_WASM, plant: `busy=${n}`, keys: 100, repeats: 1 }).times["node.series-1000.keyMiddle.ms"]);
    planted.set(n, a);
  }
}
const aa = (max(clean) - min(clean)) / median(clean);
const rows = loops.map((n) => {
  const p = planted.get(n)!;
  return { iterations: n, plantedMs: round(median(p), 3), cleanMs: round(median(clean), 3), addedMs: round(median(p) - median(clean), 3), slower: round(median(p) / median(clean), 2), caught: min(p) > max(clean) };
});
const smallest = rows.find((r) => r.caught);
results.push({ plant: "a busy loop on every edit (caught by time, interleaved: every planted run slower than every clean run)", cleanSpread: round(aa, 3), reps: REPS, trusted: quiet(load), rows, smallestCaughtAddedMs: smallest?.addedMs ?? null });
console.log(JSON.stringify(rows));
// 4: the same busy loop in the browser, by Chrome's own script time per key (interleaved, production build, blank document).
{
  const { viteBuild, withEngine, serve, WORK } = await import("./lib/build.mts");
  const { runScenario } = await import("./lib/scenarios.mts");
  const { DOCS } = await import("./lib/browser-run.mts");
  const head = viteBuild(join(WORK, "head"));
  const planted = withEngine(head, PLANT_WASM, "plant", join(WORK, "plant"));
  const a = await serve(head.dir, DOCS);
  const b = await serve(planted.dir, DOCS);
  const iterations = (smallest?.iterations ?? 600_000) * 2;
  const own = (r: any) => r.keys.at50.scriptMs + r.keys.at50.layoutMs + r.keys.at50.styleMs;
  const cleanMs: number[] = [];
  const plantMs: number[] = [];
  try {
    for (let i = 0; i < 4; i++) {
      cleanMs.push(own(await runScenario(a, { doc: "blank", keys: 40 }, "")));
      plantMs.push(own(await runScenario(b, { doc: "blank", keys: 40, plant: [1, iterations] }, "")));
    }
  } finally {
    await a.close();
    await b.close();
  }
  results.push({ plant: `busy loop of ${iterations} iterations on every edit, in the browser (Chrome's script + layout + style time per key, interleaved)`, cleanMs: cleanMs.map((x) => round(x, 2)), plantedMs: plantMs.map((x) => round(x, 2)), caught: min(plantMs) > max(cleanMs), trusted: quiet(load) });
  console.log("browser busy:", cleanMs, plantMs);
}
writeFileSync(join(PERF_DIR, "plants.json"), JSON.stringify(out, null, 2) + "\n");
