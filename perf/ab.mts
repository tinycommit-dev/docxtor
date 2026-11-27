// A/B comparisons of two builds, interleaved in one session (A, B, A, B...) so the machine's mood hits both alike.
//
//   node --experimental-strip-types perf/ab.mts wasm-opt [--reps 5]     the engine built without wasm-opt (shipped) against with it
//   node --experimental-strip-types perf/ab.mts coalesce [--reps 7]     the menu bar and toolbar drawing once (commit a1036a4) against every change
//
// Writes perf/ab/<name>.md and .json. Times and memory are only trusted when the machine is quiet; the report says so.
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { machine, quiet } from "./lib/env.mts";
import { PERF_DIR } from "./lib/gate.mts";
import { br, gz } from "./lib/sizes.mts";
import { median, min, max, round } from "./lib/stats.mts";
import { runScenario, type Result, type Spec } from "./lib/scenarios.mts";
import { serve, variantTree, viteBuild, withEngine, wasmIn, WORK, type Build } from "./lib/build.mts";
import { DOCS, docFile } from "./lib/browser-run.mts";
import { runNode } from "./lib/node.mts";

const name = process.argv[2];
const reps = Number(process.argv[process.argv.indexOf("--reps") + 1]) || (name === "coalesce" ? 7 : 5);
const OUT = join(PERF_DIR, "ab");
mkdirSync(OUT, { recursive: true });
const load = machine();

interface Variant {
  label: string;
  build: Build;
}

async function interleave(vs: Variant[], spec: Spec, n: number): Promise<Record<string, Result[]>> {
  const servers = await Promise.all(vs.map((v) => serve(v.build.dir, DOCS)));
  const file = spec.doc === "start" || spec.doc === "blank" ? "" : await docFile({ doc: spec.doc, reps: 1, spec });
  const out: Record<string, Result[]> = Object.fromEntries(vs.map((v) => [v.label, []]));
  try {
    for (let i = 0; i < n; i++) for (let j = 0; j < vs.length; j++) out[vs[j].label].push(await runScenario(servers[j], spec, file));
  } finally {
    await Promise.all(servers.map((s) => s.close()));
  }
  return out;
}

const col = (rs: Result[], f: (r: Result) => number | undefined) => rs.map(f).filter((x): x is number => typeof x === "number" && Number.isFinite(x));
const cell = (xs: number[], d = 1) => (xs.length ? `${round(median(xs), d)} (${round(min(xs), d)} to ${round(max(xs), d)})` : "-");

function wasmOptBinary(): string {
  const dir = join(homedir(), "Library", "Caches", ".wasm-pack");
  const hit = existsSync(dir) ? execFileSync("find", [dir, "-name", "wasm-opt", "-type", "f"], { encoding: "utf8" }).trim().split("\n")[0] : "";
  if (!hit) throw new Error("wasm-opt was not found in wasm-pack's cache");
  return hit;
}

const lines: string[] = [];
const json: Record<string, unknown> = { name, reps, load, date: new Date().toISOString() };
const flush = () => {
  writeFileSync(join(OUT, `${name}.md`), lines.join("\n") + "\n");
  writeFileSync(join(OUT, `${name}.json`), JSON.stringify(json));
};

async function main() {
  // The documents are made by the engine, here in Node.
  const { loadEngine } = await import("../packages/core/src/index.ts");
  await loadEngine(readFileSync(new URL("../packages/core/wasm/docxtor_bg.wasm", import.meta.url)));
  mkdirSync(WORK, { recursive: true });
  if (name === "wasm-opt") {
    const head = viteBuild(join(WORK, "head"), undefined, "no wasm-opt (shipped)");
    const opt = join(WORK, "docxtor_opt.wasm");
    const headWasm = wasmIn(head.dir);
    execFileSync(wasmOptBinary(), ["-O", "--enable-bulk-memory", "--enable-nontrapping-float-to-int", "--enable-sign-ext", "--enable-mutable-globals", "--enable-reference-types", "--enable-multivalue", headWasm, "-o", opt], { stdio: "pipe", maxBuffer: 1 << 26 });
    const withOpt = withEngine(head, opt, "wasm-opt -O", join(WORK, "opt"));
    const vs: Variant[] = [{ label: "no wasm-opt (shipped)", build: head }, { label: "wasm-opt -O", build: withOpt }];
    lines.push(`# A/B: the engine built without wasm-opt (shipped) against with it`, ``, `${reps} interleaved repeats, production build, ${new Date().toISOString()}.`, ``);
    lines.push(`## Download`, ``, `| | raw | gzip | brotli |`, `|---|---|---|---|`);
    for (const v of vs) {
      const b = readFileSync(wasmIn(v.build.dir));
      lines.push(`| ${v.label} | ${(b.byteLength / 1e6).toFixed(2)} MB | ${(gz(b) / 1e6).toFixed(2)} MB | ${(br(b) / 1e6).toFixed(2)} MB |`);
      (json as any)[`size ${v.label}`] = { raw: b.byteLength, gzip: gz(b), brotli: br(b) };
    }
    // The engine alone in Node: memory high-water and typing time, interleaved.
    lines.push(``, `## The engine alone (Node, 1,000 and 5,000 paragraphs, ${reps} interleaved runs each)`, ``, `| | engine memory 5,000 (MB) | typing per key, 1,000 (ms) | typing per key, 5,000 (ms) | open 5,000 (ms) |`, `|---|---|---|---|---|`);
    const eng: Record<string, { mem: number[]; k1: number[]; k5: number[]; o5: number[] }> = Object.fromEntries(vs.map((v) => [v.label, { mem: [], k1: [], k5: [], o5: [] }]));
    for (let i = 0; i < reps; i++)
      for (const v of vs) {
        const r = runNode(["series-1000", "series-5000"], { wasm: wasmIn(v.build.dir), repeats: 3, keys: 100 });
        eng[v.label].mem.push(r.metrics["node.series-5000.wasmMemoryBytes"] / 1048576);
        eng[v.label].k1.push(r.times["node.series-1000.keyMiddle.ms"]);
        eng[v.label].k5.push(r.times["node.series-5000.keyMiddle.ms"]);
        eng[v.label].o5.push(r.times["node.series-5000.open.ms"]);
      }
    for (const v of vs) lines.push(`| ${v.label} | ${cell(eng[v.label].mem)} | ${cell(eng[v.label].k1, 2)} | ${cell(eng[v.label].k5, 2)} | ${cell(eng[v.label].o5)} |`);
    json.engineAlone = eng;
    flush();
    // The whole app: footprint on the start screen, with a blank document, after typing; own work per key.
    const start = await interleave(vs, { doc: "start" }, reps);
    const blank = await interleave(vs, { doc: "blank", keys: 100, dump: false }, reps);
    lines.push(``, `## The whole app (footprint in MB, median and range)`, ``, `| | start screen | blank document | peak while typing 100 keys | settled after typing | own work per key (ms) | time to next paint (ms) |`, `|---|---|---|---|---|---|---|`);
    for (const v of vs) {
      const s = start[v.label];
      const b = blank[v.label];
      lines.push(`| ${v.label} | ${cell(col(s, (r) => r.startFootprintMB), 0)} | ${cell(col(b, (r) => r.openedFootprintMB), 0)} | ${cell(col(b, (r) => r.keys.at50?.peakFootprintMB), 0)} | ${cell(col(b, (r) => r.afterFootprintMB), 0)} | ${cell(col(b, (r) => (r.keys.at50?.scriptMs ?? NaN) + (r.keys.at50?.layoutMs ?? NaN) + (r.keys.at50?.styleMs ?? NaN)), 2)} | ${cell(col(b, (r) => r.keys.at50?.paintMs), 1)} |`);
    }
    json.start = start;
    json.blank = blank;
    flush();
    // And a long document, where the optimizer's inlining mattered most.
    const long = await interleave(vs, { doc: "series-1000", keys: 100 }, Math.max(3, Math.ceil(reps / 2)));
    lines.push(``, `## A 1,000-paragraph document (typing in the middle, ${Math.max(3, Math.ceil(reps / 2))} interleaved runs)`, ``, `| | footprint at rest (MB) | peak while typing (MB) | settled after (MB) | own work per key (ms) | engine per key (ms) |`, `|---|---|---|---|---|---|`);
    for (const v of vs) {
      const b = long[v.label];
      lines.push(`| ${v.label} | ${cell(col(b, (r) => r.openedFootprintMB), 0)} | ${cell(col(b, (r) => r.keys.at50?.peakFootprintMB), 0)} | ${cell(col(b, (r) => r.afterFootprintMB), 0)} | ${cell(col(b, (r) => (r.keys.at50?.scriptMs ?? NaN) + (r.keys.at50?.layoutMs ?? NaN) + (r.keys.at50?.styleMs ?? NaN)), 2)} | ${cell(col(b, (r) => r.keys.at50?.engineMs), 2)} |`);
    }
    json.long = long;
  } else if (name === "coalesce") {
    const head = viteBuild(join(WORK, "head"), undefined, "coalesce (shipped)");
    const tree = variantTree(join(WORK, "tree-undo"), "a1036a4", ["packages/editor/src"]);
    const undone = viteBuild(join(WORK, "undo"), tree, "every change draws");
    const vs: Variant[] = [{ label: "draws once (shipped, a1036a4)", build: head }, { label: "draws on every change (a1036a4 undone)", build: undone }];
    lines.push(`# A/B: the menu bar and toolbar drawing once after start (commit a1036a4)`, ``, `${reps} interleaved repeats, production build, ${new Date().toISOString()}. The two builds are the same source but for that commit's change to packages/editor/src (the variant has its own copy of the packages).`, ``);
    const start = await interleave(vs, { doc: "start" }, reps);
    const blank = await interleave(vs, { doc: "blank", keys: 60 }, reps);
    const fast = await interleave(vs, { doc: "blank", fast: true }, reps);
    lines.push(`| | DOM nodes added during startup (proof the change is in) | start screen footprint (MB) | blank document footprint (MB) | page load to start screen (ms) | navigation until a blank document takes a key (ms) |`, `|---|---|---|---|---|---|`);
    for (const v of vs)
      lines.push(`| ${v.label} | ${cell(col(start[v.label], (r) => r.startupNodesAdded), 0)} | ${cell(col(start[v.label], (r) => r.startFootprintMB), 0)} | ${cell(col(blank[v.label], (r) => r.openedFootprintMB), 0)} | ${cell(col(start[v.label], (r) => r.loadMs), 0)} | ${cell(col(fast[v.label], (r) => r.firstEditableMs), 0)} |`);
    json.start = start;
    json.blank = blank;
    json.fast = fast;
  } else {
    throw new Error("usage: ab.mts wasm-opt|coalesce");
  }
  lines.push(``, `Machine: load ${load.load1} on ${load.cpus} cores (${(load.perCore * 100).toFixed(0)}% busy). ${quiet(load) ? "Quiet enough to trust." : "**Busy: footprints and times here can be tens of MB / tens of percent off; read the ranges, and compare rows only within this one session.**"}`);
  writeFileSync(join(OUT, `${name}.md`), lines.join("\n") + "\n");
  writeFileSync(join(OUT, `${name}.json`), JSON.stringify(json));
  console.log(lines.join("\n"));
  void cpSync;
}
await main();
