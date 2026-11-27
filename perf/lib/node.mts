// The engine in Node (perf/engine.mts), as metrics.
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { PERF_DIR, type Metrics } from "./gate.mts";

export interface NodeResult {
  metrics: Metrics;
  times: Record<string, number>;
  raw: unknown;
  seconds: number;
}

export function runNode(docs: string[], opts: { wasm?: string; plant?: string; keys?: number; repeats?: number } = {}): NodeResult {
  const t = Date.now();
  const args = ["--experimental-strip-types", "--no-warnings", join(PERF_DIR, "engine.mts"), "--docs", docs.join(",")];
  if (opts.wasm) args.push("--wasm", opts.wasm);
  if (opts.plant) args.push("--plant", opts.plant);
  if (opts.keys) args.push("--keys", String(opts.keys));
  if (opts.repeats) args.push("--repeats", String(opts.repeats));
  const r = spawnSync(process.execPath, args, { encoding: "utf8", maxBuffer: 1 << 26, timeout: 20 * 60_000 });
  if (r.status !== 0) throw new Error("engine.mts failed:\n" + r.stdout.slice(-1500) + r.stderr.slice(-2500));
  const raw = JSON.parse(r.stdout.trim().split("\n").pop()!);
  const metrics: Metrics = {};
  const times: Record<string, number> = {};
  for (const [name, d] of Object.entries(raw.docs as Record<string, any>)) {
    const k = `node.${name}`;
    metrics[`${k}.open.jsonOutChars`] = d.open.jsonOutChars;
    for (const where of ["keyMiddle", "keyNearEnd"] as const) {
      metrics[`${k}.${where}.bodyClones`] = d[where].bodyClones;
      if (Number.isFinite(d[where].paragraphsWalked)) metrics[`${k}.${where}.paragraphsWalked`] = d[where].paragraphsWalked;
      if (Number.isFinite(d[where].wordCountWalked)) metrics[`${k}.${where}.wordCountWalked`] = d[where].wordCountWalked;
      if (Number.isFinite(d[where].stampWalked)) metrics[`${k}.${where}.stampWalked`] = d[where].stampWalked;
      if (Number.isFinite(d[where].blocksWritten)) metrics[`${k}.${where}.blocksWritten`] = d[where].blocksWritten;
      if (Number.isFinite(d[where].stampMs)) times[`${k}.${where}.stampMs`] = d[where].stampMs;
      if (Number.isFinite(d[where].savePackedMs)) times[`${k}.${where}.savePackedMs`] = d[where].savePackedMs;
      metrics[`${k}.${where}.jsonInChars`] = d[where].jsonInChars;
      metrics[`${k}.${where}.jsonOutChars`] = d[where].jsonOutChars;
      metrics[`${k}.${where}.calls`] = Object.values(d[where].calls as Record<string, number>).reduce((a, b) => a + b, 0);
      times[`${k}.${where}.ms`] = d[where].ms;
    }
    metrics[`${k}.wasmMemoryBytes`] = d.wasmMemoryBytes;
    times[`${k}.open.ms`] = d.open.ms;
    times[`${k}.open.readyMs`] = d.open.readyMs;
    times[`${k}.undoMs`] = d.undoMs;
    times[`${k}.saveMs`] = d.saveMs;
  }
  return { metrics, times, raw, seconds: (Date.now() - t) / 1000 };
}
