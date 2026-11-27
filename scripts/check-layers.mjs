// The layers, checked: core <- extensions <- editor <- app. Each package imports only the packages below it, never a file
// outside its own folder by a relative path, and the lower layers stay free of what belongs to a host:
// - @docxtor/core and @docxtor/extensions run in Node too, so they use no browser objects at run time (types are fine:
//   a view contribution hands its host an element).
// - @docxtor/editor is bytes in, bytes out: it keeps nothing in the browser's storage and never changes the address. The host
//   gives it a place to keep settings (`ctx.state`) and decides about routing.
// Run by `npm test`; prints each break as file:line and exits 1.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");

/** What each package may import by name (besides its own files and outside npm packages). */
const LAYERS = {
  "packages/core": { name: "@docxtor/core", may: [] },
  "packages/extensions": { name: "@docxtor/extensions", may: ["@docxtor/core"] },
  "packages/editor": { name: "@docxtor/editor", may: ["@docxtor/core", "@docxtor/extensions"] },
  web: { name: "docxtor-web", may: ["@docxtor/core", "@docxtor/extensions", "@docxtor/editor"] },
};
const OURS = new Set(Object.values(LAYERS).map((l) => l.name));
/** The subpaths each package exports on purpose (its package.json `exports`, such as the editor's style.css). */
const EXPORTED = new Set(
  Object.entries(LAYERS).flatMap(([dir, l]) => {
    const exp = JSON.parse(readFileSync(join(ROOT, dir, "package.json"), "utf8")).exports;
    return exp && typeof exp === "object" ? Object.keys(exp).filter((k) => k !== ".").map((k) => l.name + k.slice(1)) : [];
  }),
);

/** Run-time uses that a layer must not make, with the reason shown when one is found. */
const BROWSER = /(?<![.\w$])(?:document|window|navigator|localStorage|sessionStorage|indexedDB|location|history)\s*\.|\bnew\s+(?:Worker|Image|MutationObserver|ResizeObserver)\b/;
const HOST_ONLY = /(?<![.\w$])(?:localStorage|sessionStorage|indexedDB)\b|(?<![.\w$])history\s*\.\s*(?:pushState|replaceState)\b|(?<![.\w$])location\s*\.\s*(?:assign|replace|hash\s*=|href\s*=)/;
const FORBIDDEN = {
  "packages/core": [BROWSER, "runs in Node too: no browser objects at run time (engine.ts may load the engine in a worker, see ALLOW)"],
  "packages/extensions": [BROWSER, "runs in Node too: no browser objects at run time"],
  "packages/editor": [HOST_ONLY, "the host keeps settings and owns the address: use ctx.state, or a hook the host sets"],
};
/** Files that load the engine in a browser or a worker, where the browser is the point. Each with its reason. */
const ALLOW = new Set([
  "packages/core/src/engine.ts", // starts the engine's worker and fetches its WebAssembly in a browser; plain Node takes the other path
  "packages/core/src/zip-client.ts", // the save worker, in a browser only
  "packages/core/src/crash.ts", // reports a trapped engine to the page
  "packages/core/src/engine-wasm.ts", // loads the engine: compiles it in a worker in a browser, plain Node takes the other path
]);

const walk = (dir) =>
  readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (name === "node_modules" || name === "dist" || name === "wasm" || name.startsWith(".")) return [];
    return statSync(p).isDirectory() ? walk(p) : /\.(ts|tsx|mts|js|mjs)$/.test(name) && !name.endsWith(".d.ts") ? [p] : [];
  });

/** The code of a line without its comment (good enough for the patterns above: they never sit in strings that matter here). */
const code = (line) => line.replace(/\/\/.*$/, "").replace(/^\s*\*.*$/, "").replace(/\/\*.*?\*\//g, "");

const breaks = [];
for (const [dir, layer] of Object.entries(LAYERS)) {
  const src = join(ROOT, dir, "src");
  for (const file of walk(src)) {
    const rel = relative(ROOT, file).split(sep).join("/");
    const lines = readFileSync(file, "utf8").split("\n");
    let inBlock = false;
    lines.forEach((raw, i) => {
      let line = raw;
      if (inBlock) {
        if (!line.includes("*/")) return;
        line = line.slice(line.indexOf("*/") + 2);
        inBlock = false;
      }
      if (line.includes("/*") && !line.includes("*/", line.indexOf("/*"))) {
        inBlock = true;
        line = line.slice(0, line.indexOf("/*"));
      }
      for (const m of line.matchAll(/(?:\bfrom\s*|\bimport\s*\(\s*|^\s*import\s+)["']([^"']+)["']/g)) {
        const spec = m[1];
        if (spec.startsWith(".")) {
          const target = resolve(file, "..", spec);
          if (!target.startsWith(join(ROOT, dir) + sep)) breaks.push(`${rel}:${i + 1}: imports ${spec}, outside ${dir}/ (import a package by its name instead)`);
        } else {
          const pkg = spec.startsWith("@") ? spec.split("/").slice(0, 2).join("/") : spec.split("/")[0];
          if (OURS.has(pkg) && pkg !== layer.name && !layer.may.includes(pkg)) breaks.push(`${rel}:${i + 1}: ${layer.name} imports ${pkg}, a layer above it`);
          if (OURS.has(pkg) && spec !== pkg && !EXPORTED.has(spec)) breaks.push(`${rel}:${i + 1}: imports ${spec}: use the package's entry point, ${pkg}`);
        }
      }
      const rule = FORBIDDEN[dir];
      // A use guarded by `typeof x !== "undefined"` is safe where x is missing (Node), so it is not a break.
      const guarded = (l) => /typeof\s+(?:navigator|window|document|location|history)\b/.test(l);
      if (rule && !ALLOW.has(rel) && rule[0].test(code(line)) && !guarded(line)) breaks.push(`${rel}:${i + 1}: ${rule[1]}\n    ${raw.trim()}`);
    });
  }
}

if (breaks.length) {
  console.error(`The layers (core <- extensions <- editor <- app) are broken in ${breaks.length} place(s):\n${breaks.map((b) => `  ${b}`).join("\n")}`);
  process.exit(1);
}
console.log("Layers: core <- extensions <- editor <- app, no break.");
