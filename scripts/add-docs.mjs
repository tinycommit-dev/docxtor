// Builds the documentation site (docs/, VitePress) into a built app's folder, so one deploy serves the app at / and the docs at /docs/.
//   node scripts/add-docs.mjs               into web/dist/docs (what `npm run deploy` and the release workflow do after the app is built)
//   node scripts/add-docs.mjs <dist>        into <dist>/docs
// Not part of scripts/build-web.mjs: the self-host archive and the Docker image carry the app only (they stay small); their README points
// at the hosted docs. A dead link in the docs fails this build, as it does `npm run docs:build`.
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Builds the site into <dist>/docs and returns that folder. */
export function addDocs(dist = join(root, "web/dist")) {
  const out = join(resolve(dist), "docs");
  const vitepress = join(root, "node_modules/vitepress/bin/vitepress.js");
  const r = spawnSync(process.execPath, [vitepress, "build", "docs"], {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, DOCXTOR_DOCS_OUT: relative(root, out) },
  });
  if (r.status !== 0) throw new Error("The documentation build failed.");
  if (!existsSync(join(out, "index.html"))) throw new Error(`The build wrote no ${out}/index.html.`);
  return out;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(`The documentation site is in ${addDocs(process.argv[2])}`);
}
