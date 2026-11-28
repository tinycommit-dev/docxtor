// The one build of the Docxtor web app as it ships. The hosted app (scripts/deploy.mjs), the self-host archive (scripts/package-web.mjs)
// and the Docker image (deploy/Dockerfile) all call this, so they carry one and the same build: the engine from `npm run wasm:ship`
// (the one-unit release profile, about 13% smaller than `npm run wasm`), then the app (`npm run build`, which writes web/dist).
//   node scripts/build-web.mjs               build; reuse the shipped engine when it is newer than its sources
//   node scripts/build-web.mjs --no-reuse    build the engine again whatever is there (the Docker image and CI do)
// The shipped engine takes about 4 minutes. A stamp (packages/core/wasm/.ship.json: the engine file's hash) says which build is in
// packages/core/wasm, so a later `npm run wasm` (the default build, for development) is never mistaken for the shipped one.
import { execFileSync } from "node:child_process";
import { homePathsIn, homePathsUnder } from "./home-paths.mjs";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ENGINE = join(root, "packages/core/wasm/docxtor_bg.wasm");
const STAMP = join(root, "packages/core/wasm/.ship.json");
const SPELL = join(root, "packages/editor/src/spelling/wasm/spell_bg.wasm");

/** The newest modification time of any file under these paths (a missing path counts as nothing). */
function newest(paths, keep = (f) => /\.(rs|toml|lock|mjs)$/.test(f)) {
  let max = 0;
  const walk = (p) => {
    if (!existsSync(p)) return;
    const st = statSync(p);
    if (st.isDirectory()) return readdirSync(p).forEach((n) => walk(join(p, n)));
    if (keep(p)) max = Math.max(max, st.mtimeMs);
  };
  paths.forEach((p) => walk(join(root, p)));
  return max;
}
const sha256 = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");

/** Why the shipped engine in packages/core/wasm cannot be reused, or null when it can. */
export function engineStale() {
  if (!existsSync(ENGINE)) return "there is no engine build";
  if (!existsSync(STAMP)) return "the engine there is not marked as the shipped build";
  if (JSON.parse(readFileSync(STAMP, "utf8")).sha256 !== sha256(ENGINE)) return "the engine there is a different build (npm run wasm?) than the one marked shipped";
  const sources = newest(["crates/docxtor-core", "crates/docxtor-wasm", "Cargo.toml", "Cargo.lock", "scripts/strip-wasm-names.mjs"]);
  if (sources > statSync(ENGINE).mtimeMs) return "the engine's sources are newer than the build";
  if (homePathsIn(ENGINE).length) return "the engine there holds a home folder path (built before the wasm builds remapped $HOME)";
  return null;
}

/** Why the spelling checker's WebAssembly must be built again, or null. */
export function spellStale() {
  if (!existsSync(SPELL)) return "there is no spelling build";
  if (newest(["crates/docxtor-spell", "Cargo.toml", "Cargo.lock", "scripts/strip-wasm-names.mjs"]) > statSync(SPELL).mtimeMs) return "its sources are newer than the build";
  if (homePathsIn(SPELL).length) return "it holds a home folder path (built before the wasm builds remapped $HOME)";
  return null;
}

/** Marks the engine in packages/core/wasm as the shipped build. */
export function stampEngine() {
  writeFileSync(STAMP, JSON.stringify({ profile: "wasm-ship", sha256: sha256(ENGINE) }) + "\n");
}

const run = (args, say) => {
  say(`$ npm ${args.join(" ")}`);
  execFileSync("npm", args, { cwd: root, stdio: "inherit" });
};

/**
 * Builds the app as it ships into web/dist and says which engine it used.
 * Returns { engine: "reused" | "built", dist }.
 */
export function buildWeb({ reuse = true, say = console.log } = {}) {
  const why = reuse ? engineStale() : "a fresh build was asked for";
  let engine = "reused";
  if (why) {
    say(`Building the shipped engine (wasm:ship, about 4 minutes): ${why}.`);
    run(["run", "wasm:ship"], say);
    stampEngine();
    engine = "built";
  } else {
    say("Reusing the shipped engine already built (newer than its sources).");
    const spell = reuse ? spellStale() : "a fresh build was asked for";
    if (spell) run(["run", "wasm", "-w", "@docxtor/editor"], say);
  }
  run(["run", "build"], say);
  if (!existsSync(join(root, "web/dist/index.html"))) throw new Error("The build wrote no web/dist/index.html.");
  const home = homePathsUnder(join(root, "web/dist"));
  if (home.length) throw new Error(`The built app holds home folder paths:\n  ${home.join("\n  ")}`);
  return { engine, dist: join(root, "web/dist") };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const r = buildWeb({ reuse: !process.argv.includes("--no-reuse") });
  console.log(`web/dist is ready (shipped engine ${r.engine}).`);
}
