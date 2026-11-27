// The native counting-allocator run (crates/docxtor-core/tests/perf_counts.rs): allocations, bytes and peak per step.
import { spawnSync } from "node:child_process";
import type { Metrics } from "./gate.mts";

export interface NativeResult {
  metrics: Metrics;
  info: Record<string, unknown>;
  seconds: number;
}

/** `plant` is an environment object for the planted regressions (needs `--features perf-plant`). */
export function runNative(sizes: number[], plant: Record<string, string> = {}): NativeResult {
  const t = Date.now();
  const features = Object.keys(plant).length ? ["--features", "perf-plant"] : [];
  const r = spawnSync("cargo", ["test", "--release", "-p", "docxtor-core", "--test", "perf_counts", ...features, "--", "--nocapture", "--test-threads=1"], {
    encoding: "utf8",
    env: { ...process.env, DOCXTOR_PERF: "1", DOCXTOR_PERF_SIZES: sizes.join(","), ...plant },
    maxBuffer: 1 << 26,
  });
  if (r.status !== 0) throw new Error("the native counts failed:\n" + r.stdout.slice(-2000) + r.stderr.slice(-3000));
  const metrics: Metrics = {};
  const info: Record<string, unknown> = {};
  for (const line of r.stdout.split("\n")) {
    const at = line.indexOf("@@perf ");
    if (at < 0) continue;
    const j = JSON.parse(line.slice(at + 7));
    const n = j.paragraphs;
    info[`docx_bytes.${n}`] = j.docx_bytes;
    for (const step of ["open", "key", "split", "undo", "save"]) for (const [k, v] of Object.entries(j[step] as Record<string, number>)) metrics[`native.${n}.${step}.${k}`] = v;
  }
  if (!Object.keys(metrics).length) throw new Error("no native counts were printed");
  return { metrics, info, seconds: (Date.now() - t) / 1000 };
}
