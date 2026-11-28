// The three npm packages as they would be published: build/npm/<core|extensions|editor>/ (the staged folder) and build/npm/*.tgz.
//   npm run pack:packages                   build the packages from the sources and pack them; refuses when the engine in packages/core/wasm
//                                           is not the shipped build (npm run wasm:ship), or the spelling checker is missing or out of date
//   npm run pack:packages -- --build-engine build whatever of the engine and the spelling checker is missing or stale first (about 4 minutes)
//
// The repository keeps consuming the packages as source (their package.json exports ./src/index.ts), so the published form is made here:
//   1. tsc emits JavaScript and .d.ts into packages/<p>/dist (tsconfig.build.json: relative imports end in .js there).
//   2. Fixes tsc leaves: the relative imports of the editor have no extension (and tsc leaves .ts in .d.ts files), strings that name a file
//      (`new Worker(new URL("./zip-worker.ts", import.meta.url))`) and `import.meta.env.DEV`, which only Vite defines.
//   3. Staging: dist/, the engine (core: wasm/), the spelling checker and the stylesheets (editor), README, licences, a package.json that
//      points at dist.
//   4. Checks on the staged folder and then on what npm would put in each tarball (nothing of src/, tests, maps; the files a user needs).
// A reader of dist/ never sees the workspace: every name a file mentions must be a file in the same tarball (checked here).
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { engineStale, spellStale, stampEngine } from "./build-web.mjs";
import { homePathsUnder } from "./home-paths.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const OUT = join(root, "build/npm");
export const PACKAGES = ["core", "extensions", "editor"];
const HOMEPAGE = "https://docxtor.tinycommit.dev/docs/";
const REPO = "https://github.com/tinycommit-dev/docxtor";
/** The largest a tarball may be, in bytes, packed. The engine is most of core's. */
export const CEILING = { core: 6_000_000, extensions: 600_000, editor: 3_000_000 };

const read = (p) => readFileSync(p, "utf8");
const manifest = (name) => JSON.parse(read(join(root, "packages", name, "package.json")));

/** Every file under `dir` (absolute paths). */
export function walk(dir) {
  const out = [];
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Rewriting what tsc leaves behind

/** A relative module specifier: after `from`, `import(`, `import` (a side-effect import). Works on .js and .d.ts alike. */
const SPECIFIER = /(\bfrom\s*|\bimport\s*\(\s*|\bimport\s+)(["'])(\.{1,2}\/[^"'\n]*)\2/g;
/** A file named by a string: new URL("./zip-worker.ts", import.meta.url). */
const FILE_URL = /new URL\((["'])(\.{1,2}\/[^"'\n]+)\1\s*,\s*import\.meta\.url\)/g;

/** The specifier as it must be written in the published file, or throws when it names nothing. */
function resolveSpecifier(file, spec) {
  const at = (s) => resolve(dirname(file), s);
  if (/\.d\.ts$/.test(spec)) throw new Error(`${file}: ${spec}`);
  if (/\.ts$/.test(spec) && !/\.d\.ts$/.test(spec)) return spec.replace(/\.ts$/, ".js");
  if (/\.(js|json|css|mjs)$/.test(spec)) return spec;
  if (existsSync(at(spec + ".js"))) return spec + ".js";
  if (existsSync(join(at(spec), "index.js"))) return spec.replace(/\/$/, "") + "/index.js";
  throw new Error(`${relative(root, file)}: nothing for the import "${spec}"`);
}

/** Rewrites one emitted file (.js or .d.ts) in place. */
function fixFile(file) {
  const before = read(file);
  let text = before.replace(SPECIFIER, (_, lead, q, spec) => `${lead}${q}${resolveSpecifier(file, spec)}${q}`);
  text = text.replace(FILE_URL, (_, q, spec) => `new URL(${q}${spec.replace(/\.ts$/, ".js")}${q}, import.meta.url)`);
  // Vite defines import.meta.env; nothing else does, and there it is undefined (reading .DEV throws). Published code is not a development build.
  text = text.replace(/\bimport\.meta\.env\.DEV\b/g, "false");
  if (text !== before) writeFileSync(file, text);
}

/**
 * wasm-bindgen's .d.ts files declare `[Symbol.dispose]`, which a TypeScript project whose `lib` is older than ES2023 cannot read
 * (error TS2550 in a file it has to read, unless it skips checking .d.ts files). The reference makes the file bring its own lib.
 */
function bringDisposeLib(file) {
  const text = read(file);
  if (text.includes("Symbol.dispose") && !text.includes('lib="esnext.disposable"')) writeFileSync(file, `/// <reference lib="esnext.disposable" />\n${text}`);
}

/** tsc for one package, then the fixes. `dist` is emptied first. */
function build(name) {
  const dir = join(root, "packages", name);
  rmSync(join(dir, "dist"), { recursive: true, force: true });
  execFileSync("npx", ["tsc", "-p", "tsconfig.build.json"], { cwd: dir, stdio: "inherit", timeout: 300_000 });
  if (name === "editor") {
    // The spelling checker's glue and WebAssembly are generated files tsc does not copy; checker.js imports ./wasm/spell.js.
    cpSync(join(dir, "src/spelling/wasm"), join(dir, "dist/spelling/wasm"), { recursive: true });
    bringDisposeLib(join(dir, "dist/spelling/wasm/spell.d.ts"));
  }
  for (const f of walk(join(dir, "dist"))) if (/\.(js|d\.ts)$/.test(f) && !f.includes("spelling/wasm")) fixFile(f);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Checks on dist

/** Everything in dist a bundler or Node will look for must be in the tarball: imports, new URL(..., import.meta.url), stylesheets. */
export function assertResolves(pkgDir) {
  const problems = [];
  for (const f of walk(pkgDir)) {
    if (/\.(js|d\.ts)$/.test(f)) {
      const text = read(f);
      for (const m of text.matchAll(SPECIFIER)) {
        const target = resolve(dirname(f), m[3]);
        if (!/\.(js|json|css|mjs)$/.test(m[3])) problems.push(`${relative(pkgDir, f)}: import "${m[3]}" has no extension`);
        else if (!existsSync(target) && !(f.endsWith(".d.ts") && existsSync(target.replace(/\.js$/, ".d.ts")))) problems.push(`${relative(pkgDir, f)}: import "${m[3]}" is not in the package`);
      }
      for (const m of text.matchAll(/new URL\((["'])([^"':]+)\1\s*,\s*import\.meta\.url\)/g)) {
        if (!existsSync(resolve(dirname(f), m[2]))) problems.push(`${relative(pkgDir, f)}: new URL("${m[2]}", import.meta.url) is not in the package`);
      }
      if (/import\.meta\.env/.test(text)) problems.push(`${relative(pkgDir, f)}: import.meta.env (only Vite defines it)`);
    } else if (f.endsWith(".css")) {
      const text = read(f);
      for (const m of text.matchAll(/@import\s+(?:url\()?["']([^"')]+)["']/g)) {
        if (!existsSync(resolve(dirname(f), m[1]))) problems.push(`${relative(pkgDir, f)}: @import "${m[1]}" is not in the package`);
      }
      for (const m of text.matchAll(/url\(\s*(["']?)([^"')]+)\1\s*\)/g)) {
        if (/^(data:|https?:|#|%)/.test(m[2])) continue;
        if (!existsSync(resolve(dirname(f), m[2]))) problems.push(`${relative(pkgDir, f)}: url(${m[2]}) is not in the package`);
      }
    }
  }
  if (problems.length) throw new Error(`The staged package has references to files that are not in it:\n  ${problems.join("\n  ")}`);
}

/** What the tarball holds, from `npm pack --json`: the rules of what ships. */
export function assertTarball(name, files) {
  const paths = files.map((f) => f.path);
  const problems = [];
  const need = ["package.json", "README.md", "LICENSE-MIT", "LICENSE-APACHE", "dist/index.js", "dist/index.d.ts"];
  if (name === "core") need.push("wasm/docxtor_bg.wasm", "wasm/docxtor.js", "wasm/docxtor.d.ts", "THIRD_PARTY_LICENSES.txt", "dist/compile-worker.js", "dist/zip-worker.js");
  if (name === "editor") need.push("dist/spelling/wasm/spell_bg.wasm", "dist/spelling/wasm/spell.js", "style/editor.css", "THIRD_PARTY_LICENSES.txt", "dist/element.js");
  for (const n of need) if (!paths.includes(n)) problems.push(`missing ${n}`);
  for (const p of paths) {
    if (/\.map$/.test(p)) problems.push(`a source map: ${p}`);
    if (/^(src|test|tests)\//.test(p)) problems.push(`not for users: ${p}`);
    if (/\.ts$/.test(p) && !/\.d\.ts$/.test(p)) problems.push(`TypeScript source: ${p}`);
    if (/(^|\/)\./.test(p)) problems.push(`a hidden file: ${p}`);
    if (/\/(test|__tests__)\//.test(p)) problems.push(`a test: ${p}`);
  }
  if (problems.length) throw new Error(`${name}'s tarball: ${problems.join("; ")}`);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Staging

const DESCRIPTION = {
  core: manifest("core").description,
  extensions: manifest("extensions").description,
  editor: manifest("editor").description,
};
const KEYWORDS = {
  core: ["docx", "word", "ooxml", "wasm", "webassembly", "document", "editor", "lossless"],
  extensions: ["docx", "word", "editor", "extensions", "plugin", "api"],
  editor: ["docx", "word", "editor", "wysiwyg", "web-component", "custom-element", "document", "ooxml"],
};

function stagedManifest(name) {
  const src = manifest(name);
  const version = src.version;
  const dependencies = Object.fromEntries(Object.keys(src.dependencies ?? {}).map((d) => [d, manifest(d.replace("@docxtor/", "")).version]));
  const exports = {
    ".": { types: "./dist/index.d.ts", default: "./dist/index.js" },
    ...(name === "core" ? { "./wasm/docxtor_bg.wasm": "./wasm/docxtor_bg.wasm" } : {}),
    ...(name === "editor" ? { "./style.css": "./style/editor.css" } : {}),
    "./package.json": "./package.json",
  };
  const files = ["dist", "README.md", "LICENSE-MIT", "LICENSE-APACHE"];
  if (name === "core") files.push("wasm");
  if (name === "editor") files.push("style");
  if (existsSync(join(root, "packages", name, "THIRD_PARTY_LICENSES.txt"))) files.push("THIRD_PARTY_LICENSES.txt");
  return {
    name: src.name,
    version,
    description: DESCRIPTION[name],
    license: src.license,
    type: "module",
    main: "./dist/index.js",
    types: "./dist/index.d.ts",
    exports,
    files,
    // Importing the package runs nothing, with three exceptions: the stylesheets, the custom element (registered when element.js loads) and
    // the entry that re-exports it: `import "@docxtor/editor"` on its own (the <docxtor-editor> way) must not be dropped as unused.
    sideEffects: name === "editor" ? ["**/*.css", "./dist/index.js", "./dist/element.js"] : false,
    ...(Object.keys(dependencies).length ? { dependencies } : {}),
    engines: { node: ">=20" },
    keywords: ["docxtor", ...KEYWORDS[name]],
    homepage: HOMEPAGE,
    repository: { type: "git", url: `git+${REPO}.git`, directory: `packages/${name}` },
    bugs: { url: `${REPO}/issues` },
    publishConfig: { access: "public" },
  };
}

function stage(name) {
  const pkg = join(root, "packages", name);
  const out = join(OUT, name);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  cpSync(join(pkg, "dist"), join(out, "dist"), { recursive: true });
  if (name === "core") {
    mkdirSync(join(out, "wasm"));
    for (const f of ["docxtor.js", "docxtor.d.ts", "docxtor_bg.wasm", "docxtor_bg.wasm.d.ts"]) cpSync(join(pkg, "wasm", f), join(out, "wasm", f));
    bringDisposeLib(join(out, "wasm/docxtor.d.ts"));
  }
  if (name === "editor") {
    // The stylesheets keep their folders, so editor.css's @imports hold.
    for (const f of walk(join(pkg, "src")).filter((f) => f.endsWith(".css"))) {
      const to = join(out, "style", relative(join(pkg, "src"), f));
      mkdirSync(dirname(to), { recursive: true });
      cpSync(f, to);
    }
  }
  cpSync(join(pkg, "README.md"), join(out, "README.md"));
  cpSync(join(root, "LICENSE-MIT"), join(out, "LICENSE-MIT"));
  cpSync(join(root, "LICENSE-APACHE"), join(out, "LICENSE-APACHE"));
  if (existsSync(join(pkg, "THIRD_PARTY_LICENSES.txt"))) cpSync(join(pkg, "THIRD_PARTY_LICENSES.txt"), join(out, "THIRD_PARTY_LICENSES.txt"));
  writeFileSync(join(out, "package.json"), JSON.stringify(stagedManifest(name), null, 2) + "\n");
  assertResolves(out);
  const home = homePathsUnder(out);
  if (home.length) throw new Error(`@docxtor/${name} holds home folder paths (rebuild the wasm: npm run pack:packages -- --build-engine):\n  ${home.join("\n  ")}`);
  return out;
}

const mb = (n) => (n / 1e6).toFixed(2) + " MB";

/** Builds, stages and packs the three packages. Returns [{ name, dir, tgz, packed, unpacked, files }]. */
export function packPackages({ buildEngine = false, say = console.log } = {}) {
  const why = engineStale();
  if (why) {
    if (!buildEngine) throw new Error(`The engine in packages/core/wasm is not the shipped build: ${why}.\nRun npm run pack:packages -- --build-engine (about 4 minutes).`);
    say(`Building the shipped engine (wasm:ship, about 4 minutes): ${why}.`);
    execFileSync("npm", ["run", "wasm:ship"], { cwd: root, stdio: "inherit", timeout: 1_200_000 });
    stampEngine();
  }
  const spell = spellStale();
  if (spell) {
    if (!buildEngine) throw new Error(`The spelling checker is not built: ${spell}. Run npm run wasm -w @docxtor/editor (or --build-engine).`);
    execFileSync("npm", ["run", "wasm", "-w", "@docxtor/editor"], { cwd: root, stdio: "inherit", timeout: 600_000 });
  }
  rmSync(OUT, { recursive: true, force: true });
  mkdirSync(OUT, { recursive: true });
  const results = [];
  for (const name of PACKAGES) {
    say(`\n== @docxtor/${name}`);
    build(name);
    const dir = stage(name);
    const json = JSON.parse(execFileSync("npm", ["pack", "--json", "--pack-destination", OUT], { cwd: dir, encoding: "utf8", timeout: 300_000 }));
    const [info] = json;
    assertTarball(name, info.files);
    if (info.size > CEILING[name]) throw new Error(`${info.filename} is ${mb(info.size)} packed; the ceiling for @docxtor/${name} is ${mb(CEILING[name])}.`);
    say(`${info.filename}: ${mb(info.size)} packed, ${mb(info.unpackedSize)} unpacked, ${info.files.length} files`);
    results.push({ name, dir, tgz: join(OUT, info.filename), packed: info.size, unpacked: info.unpackedSize, files: info.files.length });
  }
  return results;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    packPackages({ buildEngine: process.argv.includes("--build-engine") });
  } catch (e) {
    console.error(e.message ?? e);
    process.exit(1);
  }
}
