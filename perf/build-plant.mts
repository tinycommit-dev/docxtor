// Builds the engine with the planted regressions (feature perf-plant) into ~/.cache/docxtor-perf/wasm-plant. Never touches packages/core/wasm.
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { CACHE } from "./lib/paths.mts";
import { ROOT } from "./lib/build.mts";

export const PLANT_WASM = join(CACHE, "wasm-plant", "docxtor_bg.wasm");
export function buildPlant(): string {
  mkdirSync(join(CACHE, "wasm-plant"), { recursive: true });
  execFileSync("wasm-pack", ["build", "crates/docxtor-wasm", "--target", "web", "--release", "--no-opt", "--out-dir", join(CACHE, "wasm-plant"), "--out-name", "docxtor", "--", "--features", "perf-plant"], { cwd: ROOT, stdio: "inherit" });
  return PLANT_WASM;
}
if (import.meta.url === `file://${process.argv[1]}`) console.log(buildPlant());
