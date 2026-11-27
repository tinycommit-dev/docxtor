// The documentation site (docs/, built by VitePress) served the way Cloudflare Pages serves it: the headers of deploy/_headers, under /docs/.
// Each caller builds its own copy into test-results/ (the build takes about two seconds) and starts scripts/serve-dist.mjs on a free port,
// so nothing depends on an earlier `npm run docs:build` and two specs never write the same files.
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { join, resolve } from "node:path";

export interface DocsServer {
  /** The address of the server, with no slash at the end: http://localhost:51234 */
  url: string;
  /** The folder served: the site is in <dir>/docs. */
  dir: string;
  stop(): void;
}

async function freePort(): Promise<number> {
  return new Promise((ok, fail) => {
    const s = createServer();
    s.listen(0, () => {
      const port = (s.address() as { port: number }).port;
      s.close(() => ok(port));
    });
    s.on("error", fail);
  });
}

/** Builds the site into test-results/docs-<name>/docs (a dead link fails the build, and so the test) and serves it. */
export async function startDocsServer(name: string): Promise<DocsServer> {
  const root = process.cwd();
  const dir = resolve(root, "test-results", `docs-${name}`);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const build = spawnSync(process.execPath, [join(root, "node_modules/vitepress/bin/vitepress.js"), "build", "docs"], {
    cwd: root,
    encoding: "utf8",
    timeout: 120_000,
    env: { ...process.env, DOCXTOR_DOCS_OUT: join("test-results", `docs-${name}`, "docs") },
  });
  if (build.status !== 0) throw new Error(`the documentation build failed:\n${build.stdout}\n${build.stderr}`);
  // The site names the app's icon, which the app's own folder serves at /favicon.svg.
  copyFileSync(join(root, "web/public/favicon.svg"), join(dir, "favicon.svg"));
  const port = await freePort();
  const child: ChildProcess = spawn(process.execPath, ["scripts/serve-dist.mjs", dir], { cwd: root, env: { ...process.env, PORT: String(port) }, stdio: "ignore" });
  const url = `http://localhost:${port}`;
  for (let i = 0; i < 100; i++) {
    if (await fetch(`${url}/docs/`).then((r) => r.ok, () => false)) return { url, dir, stop: () => void child.kill() };
    await new Promise((r) => setTimeout(r, 100));
  }
  child.kill();
  throw new Error("the documentation server did not start");
}
