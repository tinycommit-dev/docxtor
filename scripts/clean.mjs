// Frees disk space taken by builds, test output and finished agent worktrees.
//
//   npm run clean            remove test output, finished worktrees and Rust's build output
//   npm run clean -- --keep-build   the same, but keep target/ (faster next build)
//   npm run clean -- --paused       also clear the build output of unfinished worktrees
//
// A worktree is removed only when its branch is already merged into main, it has no changes
// (untracked files count), and it isn't locked (Claude locks an agent's worktree while it runs).
// Other worktrees are left alone; with --paused (only when no agent is running) they keep their
// code but lose their build output. Nothing under a running cargo or wasm-pack build is deleted.
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, rmSync, statSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const keepBuild = process.argv.includes("--keep-build");
const paused = process.argv.includes("--paused");
const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();

function size(path) {
  if (!existsSync(path)) return 0;
  try { return Number(execFileSync("du", ["-sk", path], { encoding: "utf8" }).split("\t")[0]) * 1024; } catch { return 0; }
}
const gb = (n) => `${(n / 1024 ** 3).toFixed(1)} GB`;
const remove = (path) => rmSync(path, { recursive: true, force: true });

const before = size(root);
const buildDirs = ["target", "node_modules", "web/dist", "web/dist-test", "test-results", "playwright-report"];

// Builds that are running right now: never delete build output under them.
let building = false;
try { building = execFileSync("pgrep", ["-x", "cargo|rustc|wasm-pack"], { encoding: "utf8" }).trim() !== ""; } catch { building = false; }

// Locked worktrees, from `git worktree list --porcelain`.
const locked = new Set();
try {
  let path = "";
  for (const line of git("worktree", "list", "--porcelain").split("\n")) {
    if (line.startsWith("worktree ")) path = line.slice(9);
    else if (line.startsWith("locked")) locked.add(path);
  }
} catch {}

// Agent worktrees.
const wtDir = join(root, ".claude/worktrees");
if (existsSync(wtDir)) {
  for (const name of readdirSync(wtDir)) {
    const wt = join(wtDir, name);
    if (!statSync(wt).isDirectory()) continue;
    if (locked.has(wt)) { console.log(`left worktree ${name} alone (locked: an agent is using it)`); continue; }
    let merged = false, dirty = true;
    try {
      const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: wt, encoding: "utf8" }).trim();
      execFileSync("git", ["merge-base", "--is-ancestor", head, "main"], { cwd: root, stdio: "ignore" }); // throws when not merged
      merged = true;
    } catch { merged = false; }
    try {
      dirty = execFileSync("git", ["status", "--porcelain"], { cwd: wt, encoding: "utf8" }).trim() !== "";
    } catch { dirty = true; }
    if (merged && !dirty) {
      try { git("worktree", "remove", wt); console.log(`removed worktree ${name} (merged)`); }
      catch { console.log(`left worktree ${name} alone (git would not remove it)`); }
    } else if (!paused) {
      console.log(`left worktree ${name} alone (${merged ? "uncommitted changes" : "not merged"})`);
    } else if (building) {
      console.log(`left worktree ${name} alone (a build is running)`);
    } else {
      for (const d of buildDirs) remove(join(wt, d));
      if (existsSync(join(wt, "packages"))) for (const p of readdirSync(join(wt, "packages"))) remove(join(wt, "packages", p, "node_modules"));
      console.log(`kept worktree ${name} (${merged ? "uncommitted changes" : "not merged"}), cleared its build output`);
    }
  }
  try { git("worktree", "prune"); } catch {}
}

// The main checkout's test output, and Rust's build output unless asked to keep it.
for (const d of ["test-results", "playwright-report", "web/dist-test", "docs/.vitepress/dist", "docs/.vitepress/cache", "scripts/.docs-tmp"]) remove(join(root, d));
if (!keepBuild && building) console.log("kept target/: a cargo or wasm-pack build is running; run clean again when it ends");
else if (!keepBuild) remove(join(root, "target"));

console.log(`freed ${gb(Math.max(0, before - size(root)))}; the project now takes ${gb(size(root))}`);
