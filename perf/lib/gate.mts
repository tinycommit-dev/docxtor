// Gates: a run's exact counts against the accepted baseline, with a tolerance taken from the A/A spread.
//
// A metric is a name (`native.1000.key.allocs`) and a number where lower is better. `perf/noise.json` holds, for each
// metric, what the same build gave against itself (`npm run perf:aa`); the tolerance is twice that spread, but never
// less than `FLOOR` of the value (a count that never moves still gets a 0.5% margin for rounding) and never less than
// `absFloor` (a handful of nodes is not a regression). `perf/baseline.json` holds the last accepted numbers.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const PERF_DIR = join(dirname(fileURLToPath(import.meta.url)), "..");
export const FLOOR = 0.005;

export type Metrics = Record<string, number>;
export interface Noise {
  [metric: string]: { min: number; max: number; median: number; runs: number };
}
export interface Baseline {
  accepted: string;
  commit: string;
  load: unknown;
  metrics: Metrics;
}

const read = <T,>(f: string, d: T): T => (existsSync(join(PERF_DIR, f)) ? (JSON.parse(readFileSync(join(PERF_DIR, f), "utf8")) as T) : d);
export const readBaseline = () => read<Baseline | null>("baseline.json", null);
export const readNoise = () => read<Noise>("noise.json", {});
export const write = (f: string, v: unknown) => writeFileSync(join(PERF_DIR, f), JSON.stringify(v, null, 2) + "\n");

/** How far above the baseline a metric may be before it fails. */
export function tolerance(metric: string, base: number, noise: Noise): number {
  const n = noise[metric];
  const spread = n ? (n.max - n.min) * 2 : 0;
  return Math.max(spread, Math.abs(base) * FLOOR, absFloor(metric));
}
function absFloor(metric: string): number {
  if (/\.clones$|bodyClones/.test(metric)) return 0.01;
  if (/\.walked$|paragraphsWalked/.test(metric)) return 0.5;
  return 0;
}

export interface Verdict {
  metric: string;
  base: number;
  now: number;
  tol: number;
  delta: number;
  status: "ok" | "worse" | "better" | "new";
}

export function compare(now: Metrics, base: Metrics | undefined, noise: Noise): Verdict[] {
  return Object.keys(now)
    .sort()
    .map((metric) => {
      const b = base?.[metric];
      if (b === undefined) return { metric, base: NaN, now: now[metric], tol: NaN, delta: NaN, status: "new" as const };
      const tol = tolerance(metric, b, noise);
      const delta = now[metric] - b;
      return { metric, base: b, now: now[metric], tol, delta, status: delta > tol ? ("worse" as const) : delta < -tol ? ("better" as const) : ("ok" as const) };
    });
}

/**
 * Hard limits (`perf/budgets.json`: metric to its most), mostly on exact counts; a few are on times that must stay far from a limit (the longest stall during an autosave), checked whatever the baseline says. A baseline
 * only says "no worse than last time", and when a count improves it stays slack until someone accepts the new one; a
 * budget keeps a gain that was made on purpose, such as layouts per key in the browser, from creeping back.
 */
export const readBudgets = () => read<Record<string, { max: number; why: string }>>("budgets.json", {});

/** The verdicts, with every budget a run exceeded marked as worse (a budget on a metric the run did not measure is skipped). */
export function applyBudgets(verdicts: Verdict[], now: Metrics, budgets: Record<string, { max: number; why: string }>): Verdict[] {
  const out = [...verdicts];
  for (const [metric, b] of Object.entries(budgets)) {
    const v = now[metric];
    if (v === undefined || v <= b.max) continue;
    const i = out.findIndex((x) => x.metric === metric);
    const verdict: Verdict = { metric: `${metric} (budget: ${b.why})`, base: b.max, now: v, tol: 0, delta: v - b.max, status: "worse" };
    if (i >= 0 && out[i].status === "worse") continue;
    out.push(verdict);
  }
  return out;
}

/** Aim a size-free label at a metric name for humans. */
export const fmt = (n: number): string => (!Number.isFinite(n) ? "-" : Math.abs(n) >= 1e6 ? (n / 1e6).toFixed(2) + "M" : Math.abs(n) >= 1e4 ? (n / 1e3).toFixed(1) + "k" : String(+n.toFixed(3)));
