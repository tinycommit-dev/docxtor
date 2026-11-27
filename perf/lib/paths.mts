// Places and shared types (a leaf module: run.mts and the browser tier both need them, and neither may import the other).
import { homedir } from "node:os";
import { join } from "node:path";
import type { Metrics } from "./gate.mts";

/** Everything the harness builds or saves lives here, outside the repo. */
export const CACHE = join(homedir(), ".cache", "docxtor-perf");

export type Tier = "quick" | "standard" | "full";

export interface Collected {
  metrics: Metrics;
  /** Reported numbers: times and memory. Never gated. */
  reported: Record<string, number>;
  sections: { title: string; body: string }[];
  notes: string[];
  seconds: number;
}
