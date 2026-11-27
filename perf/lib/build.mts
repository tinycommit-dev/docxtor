// Production builds for the browser tier: built by the harness (never the dev server), into a folder outside the repo,
// and served by a small static server. Variants (an engine built another way, a source tree with one change undone)
// are made here too.
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { join, resolve, extname } from "node:path";
import { PERF_DIR } from "./gate.mts";
import { CACHE } from "./paths.mts";

export const ROOT = resolve(PERF_DIR, "..");

export interface Build {
  /** Folder with the built app. */
  dir: string;
  label: string;
  seconds: number;
}

/** Vite's command, wherever npm put it: the app's own packages, or the workspace's when npm hoisted it there. */
function viteBin(): string {
  for (const dir of [join(ROOT, "web", "node_modules", ".bin"), join(ROOT, "node_modules", ".bin")]) if (existsSync(join(dir, "vite"))) return join(dir, "vite");
  throw new Error("Vite is not installed: run npm install");
}

/** `vite build` of web/ (in `tree`, the repo itself unless a variant tree is given) into `out`. */
export function viteBuild(out: string, tree = ROOT, label = "head"): Build {
  const t = Date.now();
  rmSync(out, { recursive: true, force: true });
  const r = spawnSync(viteBin(), ["build", "--outDir", out, "--emptyOutDir", "--logLevel", "warn"], { cwd: join(tree, "web"), encoding: "utf8", env: { ...process.env, NODE_ENV: "production" }, maxBuffer: 1 << 26 });
  if (r.error) throw new Error(`the production build could not start (${label}): ${r.error.message}`);
  if (r.status !== 0) throw new Error(`the production build failed (${label}):\n${(r.stdout ?? "").slice(-1500)}${(r.stderr ?? "").slice(-2500)}`);
  return { dir: out, label, seconds: (Date.now() - t) / 1000 };
}

/** The engine file in a build. */
export function wasmIn(dir: string): string {
  // The engine's, not the spelling checker's (`spell_bg`), which a build has too.
  const f = readdirSync(join(dir, "assets")).find((n) => n.startsWith("docxtor_bg") && n.endsWith(".wasm"));
  if (!f) throw new Error("no .wasm in the build");
  return join(dir, "assets", f);
}

/** Copy a build and put another engine binary in it (the glue is the same: the binary is a drop-in). */
export function withEngine(from: Build, wasm: string, label: string, out: string): Build {
  rmSync(out, { recursive: true, force: true });
  cpSync(from.dir, out, { recursive: true });
  cpSync(wasm, wasmIn(out));
  return { dir: out, label, seconds: 0 };
}

/**
 * A copy of the source (web and packages, from git, so only what is committed) with `undo` (a commit's changes to
 * paths under packages/ or web/) applied backwards. node_modules is rebuilt with its own `@docxtor` links, so the build
 * uses the copy's packages and not the checkout's (otherwise the change would silently stay in).
 */
export function variantTree(dir: string, undoCommit?: string, onlyPaths: string[] = []): string {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  execFileSync("sh", ["-c", `git -C '${ROOT}' archive HEAD web packages package.json | tar -x -C '${dir}'`], { stdio: "pipe" });
  // The engine files are not committed: use the ones built here.
  cpSync(join(ROOT, "packages/core/wasm"), join(dir, "packages/core/wasm"), { recursive: true });
  // Nor is the spelling checker's.
  cpSync(join(ROOT, "packages/editor/src/spelling/wasm"), join(dir, "packages/editor/src/spelling/wasm"), { recursive: true });
  const nm = join(dir, "node_modules");
  mkdirSync(join(nm, "@docxtor"), { recursive: true });
  for (const name of readdirSync(join(ROOT, "node_modules"))) if (name !== "@docxtor" && name !== ".bin") symlinkSync(join(ROOT, "node_modules", name), join(nm, name));
  for (const p of readdirSync(join(ROOT, "node_modules/@docxtor"))) symlinkSync(`../../packages/${p}`, join(nm, "@docxtor", p));
  if (existsSync(join(ROOT, "web/node_modules"))) symlinkSync(join(ROOT, "web/node_modules"), join(dir, "web/node_modules"));
  if (undoCommit) {
    const patch = execFileSync("git", ["-C", ROOT, "show", undoCommit, "--", ...(onlyPaths.length ? onlyPaths : ["packages", "web"])], { encoding: "utf8", maxBuffer: 1 << 26 });
    const p = join(dir, "undo.patch");
    writeFileSync(p, patch);
    execFileSync("git", ["apply", "-R", "--unsafe-paths", "--directory=.", p], { cwd: dir, stdio: "pipe" });
  }
  return dir;
}

const TYPES: Record<string, string> = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".woff2": "font/woff2", ".woff": "font/woff", ".webmanifest": "application/manifest+json", ".aff": "text/plain", ".dic": "text/plain", ".ico": "image/x-icon" };

export interface Served {
  url: string;
  /** Paths requested since the last `reset`, with their sizes (what a first visit fetches). */
  requested: Map<string, number>;
  reset(): void;
  close(): Promise<void>;
}

/** Static server for a built folder, on a free port of its own. */
export async function serve(dir: string, docs?: string): Promise<Served> {
  const requested = new Map<string, number>();
  const server: Server = createServer((req, res) => {
    const path = decodeURIComponent((req.url ?? "/").split("?")[0]);
    // The documents the scenarios open: not part of the app, so not counted in what a first visit fetches.
    if (docs && path.startsWith("/__perf/")) {
      const f = resolve(docs, path.slice(8).replace(/[^\w.-]/g, ""));
      if (!existsSync(f) || !statSync(f).isFile()) {
        res.statusCode = 404;
        return res.end("no such document");
      }
      res.setHeader("content-type", "application/octet-stream");
      return res.end(readFileSync(f));
    }
    let file = resolve(dir, "." + (path.endsWith("/") ? path + "index.html" : path));
    if (!file.startsWith(resolve(dir)) || !existsSync(file) || !statSync(file).isFile()) {
      res.statusCode = 404;
      return res.end("not found");
    }
    const body = readFileSync(file);
    requested.set(path, body.byteLength);
    res.setHeader("content-type", TYPES[extname(file)] ?? "application/octet-stream");
    res.setHeader("cache-control", "no-store");
    res.end(body);
  });
  await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
  const port = (server.address() as { port: number }).port;
  return { url: `http://127.0.0.1:${port}`, requested, reset: () => requested.clear(), close: () => new Promise((ok) => server.close(() => ok())) };
}

export const WORK = join(CACHE, "work");
export const isBuilt = (b: Build) => existsSync(join(b.dir, "index.html"));
void rmSync;
