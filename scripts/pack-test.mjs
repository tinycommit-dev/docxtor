// Installs the three packed tarballs (build/npm/*.tgz, made by npm run pack:packages) into throwaway apps and uses them the way a
// stranger would. Nothing here touches the workspace: every app resolves @docxtor/* from node_modules, from the tarballs.
//   npm run test:packages                      every check
//   npm run test:packages -- --only=node,vite  some of them: node, types, publint, attw, vite, webpack, esbuild, next
//   npm run test:packages -- --keep            leave the throwaway folder (its path is printed)
// The checks:
//   node      @docxtor/core opens a fixture, edits it, saves and reopens it; the editor and the extensions import in plain Node (no DOM)
//   types     tsc --noEmit on a file that uses the public types of all three, with moduleResolution bundler, node16 and nodenext, TypeScript 5 and 7
//   publint, attw   the packages' own checks (publint, arethetypeswrong)
//   vite, webpack, esbuild   the app is built, loaded in Chromium, and an editor mounts, opens a document, takes typed text and saves; the saved
//                  bytes must hold the text. The engine's WebAssembly, the two workers and the spelling checker's WebAssembly are asked for
//                  (esbuild bundles no workers: the editor must save without them)
//   next      `next build` of a page whose server component imports @docxtor/core and @docxtor/editor
import { execFileSync, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(root, "build/npm");
const argv = process.argv.slice(2);
const only = (argv.find((a) => a.startsWith("--only=")) ?? "").slice(7).split(",").filter(Boolean);
const keep = argv.includes("--keep");

/** What the throwaway apps install from the npm registry. Pinned: a result must not change because a tool released. */
const TOOLS = {
  vite: "8.4.0",
  webpack: "5.113.0",
  "webpack-cli": "7.3.1",
  "css-loader": "7.1.5",
  "style-loader": "4.0.0",
  esbuild: "0.28.2",
  publint: "0.3.25",
  "@arethetypeswrong/cli": "0.18.5",
  typescript5: "npm:typescript@5.9.3",
  typescript: "7.0.2",
};
const NEXT = { next: "16.4.6", react: "19.3.1", "react-dom": "19.3.1" };
const STEP_LIMIT = 600_000;

const sizeMB = (n) => (n / 1e6).toFixed(2);
const tarballs = () => {
  if (!existsSync(OUT)) throw new Error("No build/npm: run npm run pack:packages first.");
  const files = readdirSync(OUT).filter((f) => f.endsWith(".tgz"));
  const find = (n) => {
    const f = files.find((x) => x.startsWith(`docxtor-${n}-`));
    if (!f) throw new Error(`No tarball for @docxtor/${n} in build/npm.`);
    return join(OUT, f);
  };
  return { core: find("core"), extensions: find("extensions"), editor: find("editor") };
};

/** Runs a program with a time limit; throws with the tail of its output when it fails. */
function sh(cmd, args, { cwd, timeout = STEP_LIMIT, env } = {}) {
  const r = spawnSync(cmd, args, { cwd, encoding: "utf8", timeout, maxBuffer: 1 << 28, env: { ...process.env, ...env, CI: "1", npm_config_update_notifier: "false" } });
  const out = `${r.stdout ?? ""}${r.stderr ?? ""}`;
  if (r.error) throw new Error(`${cmd} ${args.join(" ")}: ${r.error.message}\n${out.slice(-3000)}`);
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(" ")} exited ${r.status}\n${out.slice(-4000)}`);
  return out;
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The throwaway folder

const work = mkdtempSync(join(tmpdir(), "docxtor-pack-test-"));
const site = join(work, "site");
const data = join(site, "data");

function install(dir, deps, tgz) {
  mkdirSync(dir, { recursive: true });
  const docxtor = { "@docxtor/core": `file:${tgz.core}`, "@docxtor/extensions": `file:${tgz.extensions}`, "@docxtor/editor": `file:${tgz.editor}` };
  // The editor's own dependencies are pinned to versions that are not on npm yet: point them at the tarballs.
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "pack-test-app", private: true, type: "module", dependencies: { ...docxtor, ...deps }, overrides: docxtor }, null, 2));
  sh("npm", ["install", "--no-audit", "--no-fund", "--loglevel=error"], { cwd: dir });
}

// ---------------------------------------------------------------------------------------------------------------------------------
// A static file server for the built apps, which also says what was asked for and not found

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json", ".docx": "application/octet-stream", ".aff": "text/plain", ".dic": "text/plain", ".svg": "image/svg+xml" };

function serve(dirs) {
  const log = [];
  const server = createServer((req, res) => {
    const path = decodeURIComponent((req.url ?? "/").split("?")[0]);
    let file = null;
    for (const [prefix, dir] of dirs) {
      if (path.startsWith(prefix)) {
        const f = join(dir, path.slice(prefix.length));
        if (f.startsWith(dir) && existsSync(f) && statSync(f).isFile()) {
          file = f;
          break;
        }
      }
    }
    log.push({ path, status: file ? 200 : 404, size: file ? statSync(file).size : 0 });
    if (!file) {
      res.writeHead(404, { "content-type": "text/plain" });
      res.end("not found");
      return;
    }
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(readFileSync(file));
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok({ port: server.address().port, log, close: () => server.close() })));
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The browser run, the same for every bundler

const PHRASE = "Packed tarball typed this";
const HTML = (script, extra = "") => `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>pack test</title>${extra}<style>body{margin:0}#editor,docxtor-editor{display:block;height:90vh}</style></head><body><div id="editor"></div><script ${script}></script></body></html>`;

/** The page code every bundler builds: mounts an editor from the packages, opens the big document, and says when it is ready. */
const APP_CLASS = `
import { DocxtorEditor } from "@docxtor/editor";
import "@docxtor/editor/style.css";
const spelling = { async load(lang) {
  if (!lang.toLowerCase().startsWith("en")) return null;
  const get = async (ext) => new Uint8Array(await (await fetch("/data/en." + ext)).arrayBuffer());
  return { aff: await get("aff"), dic: await get("dic") };
} };
const errors = [];
const editor = new DocxtorEditor({ spelling, fonts: false, onError: (m) => errors.push(m) }).mount(document.getElementById("editor"));
window.__errors = errors;
window.__save = () => editor.saveInSteps();
window.__focus = () => editor.focus();
await editor.open(await (await fetch("/data/big.docx")).arrayBuffer(), "big.docx");
window.__ready = true;
`;
/** The same through the <docxtor-editor> element. */
const APP_ELEMENT = `
import "@docxtor/editor";
import "@docxtor/editor/style.css";
const spelling = { async load(lang) {
  if (!lang.toLowerCase().startsWith("en")) return null;
  const get = async (ext) => new Uint8Array(await (await fetch("/data/en." + ext)).arrayBuffer());
  return { aff: await get("aff"), dic: await get("dic") };
} };
const errors = [];
const el = document.createElement("docxtor-editor");
el.setAttribute("fonts", "none");
el.spelling = spelling;
el.addEventListener("docxtor-error", (e) => errors.push(e.detail.message));
el.addEventListener("docxtor-ready", () => { window.__ready = true; });
el.setAttribute("src", "/data/big.docx");
document.getElementById("editor").replaceWith(el);
window.__errors = errors;
window.__save = () => el.saveInSteps();
window.__focus = () => el.editor.focus();
`;

async function inBrowser(name, base, { workers }) {
  const { chromium } = await import("@playwright/test");
  const srv = await serve([["/data/", data], ["/", base]]);
  const browser = await chromium.launch();
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    const problems = [];
    const started = [];
    const wasm = [];
    page.on("pageerror", (e) => problems.push(`page error: ${e.message}`));
    page.on("console", (m) => m.type() === "error" && !/Failed to load resource/.test(m.text()) && problems.push(`console: ${m.text()}`));
    page.on("worker", (w) => started.push(w.url()));
    await page.goto(`http://127.0.0.1:${srv.port}/index.html`);
    await page.waitForFunction(() => window.__ready === true, null, { timeout: 90_000 });
    const errors = await page.evaluate(() => window.__errors);
    if (errors.length) problems.push(`editor errors: ${errors.join("; ")}`);
    await page.evaluate(() => window.__focus());
    await page.keyboard.type(PHRASE, { delay: 5 });
    const b64 = await page.evaluate(async () => {
      const bytes = await window.__save();
      let s = "";
      for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      return btoa(s);
    });
    const saved = Buffer.from(b64, "base64");
    const file = join(work, `${name}-saved.docx`);
    writeFileSync(file, saved);
    const xml = sh("unzip", ["-p", file, "word/document.xml"]).replace(/<[^>]+>/g, "");
    if (!xml.includes(PHRASE)) problems.push(`the saved document does not contain "${PHRASE}"`);
    if (!xml.includes("Paragraph 1 ")) problems.push("the saved document lost its text");
    // The engine's and the spelling checker's WebAssembly, fetched and served.
    const engine = statSync(join(OUT, "core/wasm/docxtor_bg.wasm")).size;
    const spell = statSync(join(OUT, "editor/dist/spelling/wasm/spell_bg.wasm")).size;
    const wasmServed = () => srv.log.filter((r) => /\.wasm$/.test(r.path) && r.status === 200).map((r) => r.size);
    for (let i = 0; i < 100 && !wasmServed().includes(spell); i++) await page.waitForTimeout(100);
    wasm.push(...wasmServed());
    if (!wasm.includes(engine)) problems.push(`the engine (${engine} bytes) was not fetched; wasm responses: ${wasm.join(", ")}`);
    if (!wasm.includes(spell)) problems.push(`the spelling checker (${spell} bytes) was not fetched; wasm responses: ${wasm.join(", ")}`);
    const missing = srv.log.filter((r) => r.status === 404).map((r) => r.path);
    // A worker whose file is not there still shows up as started (and fails at once): count the ones that ran.
    const ran = started.filter((u) => !missing.includes(new URL(u).pathname));
    if (workers) {
      if (ran.length < 2) problems.push(`expected the compile and zip workers to start, saw ${ran.length}: ${ran.join(", ")}`);
    } else {
      if (ran.length) problems.push(`workers ran in a bundle that has none: ${ran.join(", ")}`);
      for (const w of ["compile-worker", "zip-worker"]) if (!missing.some((m) => m.includes(w))) problems.push(`expected the page to ask for ${w} and not find it; 404s: ${missing.join(", ")}`);
    }
    if (workers && missing.length) problems.push(`404s: ${missing.join(", ")}`);
    if (problems.length) throw new Error(problems.join("\n"));
    return `${saved.length} bytes saved, ${ran.length} workers ran, wasm ${wasm.map((n) => sizeMB(n) + " MB").join(" + ")}${missing.length ? `, 404: ${missing.join(" ")}` : ""}`;
  } finally {
    await browser.close();
    srv.close();
  }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The checks

const results = [];
async function step(name, fn) {
  if (only.length && !only.includes(name)) return;
  const t0 = Date.now();
  try {
    const note = await fn();
    results.push({ name, ok: true, secs: (Date.now() - t0) / 1000, note: note ?? "" });
  } catch (e) {
    results.push({ name, ok: false, secs: (Date.now() - t0) / 1000, note: String(e.message ?? e) });
  }
  const r = results.at(-1);
  console.log(`${r.ok ? "PASS" : "FAIL"} ${name} (${r.secs.toFixed(1)} s)${r.note ? `: ${r.note.split("\n")[0]}` : ""}`);
  if (!r.ok) console.log(r.note.split("\n").slice(1).map((l) => `     ${l}`).join("\n"));
}

const tgz = tarballs();
const t0 = Date.now();
console.log(`Throwaway folder: ${work}`);
let ready = false;
function prepare() {
  if (ready) return;
  const t = Date.now();
  install(site, TOOLS, tgz);
  mkdirSync(data, { recursive: true });
  cpSync(join(root, "fixtures/simple.docx"), join(data, "simple.docx"));
  cpSync(join(root, "node_modules/dictionary-en/index.aff"), join(data, "en.aff"));
  cpSync(join(root, "node_modules/dictionary-en/index.dic"), join(data, "en.dic"));
  // A document long enough for the save to go through the zip worker (it zips on the page below 1 MB of XML).
  writeFileSync(
    join(site, "make-big.mjs"),
    `import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { Engine, loadEngine } from "@docxtor/core";
await loadEngine(await readFile(fileURLToPath(import.meta.resolve("@docxtor/core/wasm/docxtor_bg.wasm"))));
const md = Array.from({ length: 12000 }, (_, i) => "Paragraph " + (i + 1) + " is here with some words to fill a line or two of the page for the test.").join("\\n\\n");
const doc = Engine.fromMarkdown(md);
await writeFile(process.argv[2], doc.save());
`,
  );
  sh("node", ["make-big.mjs", join(data, "big.docx")], { cwd: site });
  ready = true;
  console.log(`(installed the tarballs and the tools in ${((Date.now() - t) / 1000).toFixed(0)} s)`);
}

await step("node", () => {
  prepare();
  writeFileSync(
    join(site, "node-check.mjs"),
    `import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { Engine, loadEngine, toolDefinitions } from "@docxtor/core";
await loadEngine(await readFile(fileURLToPath(import.meta.resolve("@docxtor/core/wasm/docxtor_bg.wasm"))));
const bytes = new Uint8Array(await readFile("data/simple.docx"));
const doc = Engine.open(bytes, "simple.docx");
const before = doc.raw.docscript(40);
const word = /[A-Z][a-z]{4,}/.exec(before)?.[0];
assert.ok(word, "no word to replace in " + before);
const [r] = doc.apply({ op: "replace_text", find: word, replace: "Packaged", all: true });
assert.ok(r.count >= 1);
const again = Engine.open(doc.save(), "simple.docx");
assert.match(again.raw.docscript(40), /Packaged/);
assert.ok(toolDefinitions().length > 0);
doc.dispose(); again.dispose();
// Importing the packages where there is no DOM must not throw (a server renders the page).
assert.equal(typeof document, "undefined");
const ext = await import("@docxtor/extensions");
assert.equal(typeof ext.defineExtension, "function");
const ed = await import("@docxtor/editor");
assert.equal(typeof ed.DocxtorEditor, "function");
assert.equal(typeof ed.Engine, "function"); // the editor re-exports the engine
console.log("ok");
`,
  );
  return sh("node", ["node-check.mjs"], { cwd: site, timeout: 120_000 }).trim();
});

await step("types", () => {
  prepare();
  const dir = join(site, "types");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "use.ts"),
    `import { Engine, loadEngine, type Op, type OpResult } from "@docxtor/core";
import { defineExtension, type Extension, type ExtensionContext } from "@docxtor/extensions";
import { DocxtorEditor, DocxtorEditorElement, type DocxtorEditorOptions, type SpellingProvider } from "@docxtor/editor";

const spelling: SpellingProvider = { load: async () => null };
const options: DocxtorEditorOptions = { spelling, onSave: (bytes: Uint8Array) => void bytes.length };
export const ext: Extension = defineExtension({ id: "acme.x", name: "X", activate(ctx: ExtensionContext) { void ctx; } });
export async function run(host: HTMLElement, bytes: Uint8Array): Promise<Uint8Array> {
  await loadEngine();
  const op: Op = { op: "replace_text", find: "a", replace: "b", all: true };
  const doc = Engine.open(bytes, "a.docx");
  const results: OpResult[] = doc.apply(op);
  void results;
  const editor = new DocxtorEditor(options).mount(host);
  await editor.open(bytes, "a.docx");
  const el: DocxtorEditorElement | null = null;
  void el;
  return editor.save();
}
`,
  );
  const configs = [
    ["bundler", { module: "esnext", moduleResolution: "bundler" }],
    ["node16", { module: "node16", moduleResolution: "node16" }],
    ["nodenext", { module: "nodenext", moduleResolution: "nodenext" }],
  ];
  const lines = [];
  for (const [label, tsc] of [["ts5", "typescript5/bin/tsc"], ["ts7", "typescript/bin/tsc"]]) {
    for (const [mode, opts] of configs) {
      const file = join(dir, `tsconfig.${label}-${mode}.json`);
      writeFileSync(file, JSON.stringify({ compilerOptions: { ...opts, target: "es2022", lib: ["es2022", "dom", "dom.iterable"], strict: true, skipLibCheck: false, noEmit: true, types: [] }, files: ["use.ts"] }));
      sh("node", [join(site, "node_modules", tsc), "-p", file], { cwd: dir, timeout: 180_000 });
      lines.push(`${label}/${mode}`);
    }
  }
  return `${lines.join(", ")} clean (skipLibCheck off)`;
});

await step("publint", () => {
  prepare();
  const lines = [];
  for (const [n, file] of Object.entries(tgz)) lines.push(`${n}: ${sh("node", [join(site, "node_modules/publint/src/cli.js"), file, "--strict"], { cwd: site, timeout: 120_000 }).trim().split("\n").pop()}`);
  return lines.join("; ");
});

await step("attw", () => {
  prepare();
  const lines = [];
  for (const [n, file] of Object.entries(tgz)) {
    // The stylesheet and the engine's .wasm are not modules: they have no types to check.
    const out = sh("node", [join(site, "node_modules/@arethetypeswrong/cli/dist/index.js"), "--pack", file, "--profile", "esm-only", "--exclude-entrypoints", "style.css", "wasm/docxtor_bg.wasm", "--format", "ascii"], { cwd: site, timeout: 180_000 });
    // attw exits with an error when a resolution that counts (Node ESM, bundlers) has a problem; the CommonJS ones are ignored by the profile.
    lines.push(`${n}: ${/🟢/.test(out) && !/❌|🚨|💀/.test(out) ? "no problems (esm-only profile)" : out.trim()}`);
  }
  return lines.join("; ");
});

await step("vite", async () => {
  prepare();
  const app = join(site, "apps/vite");
  mkdirSync(app, { recursive: true });
  writeFileSync(join(app, "main.js"), APP_CLASS);
  writeFileSync(join(app, "index.html"), HTML('type="module" src="/main.js"'));
  sh("node", [join(site, "node_modules/vite/bin/vite.js"), "build", "--logLevel", "warn"], { cwd: app, timeout: 300_000 });
  return inBrowser("vite", join(app, "dist"), { workers: true });
});

await step("webpack", async () => {
  prepare();
  const app = join(site, "apps/webpack");
  mkdirSync(join(app, "dist"), { recursive: true });
  writeFileSync(join(app, "main.js"), APP_ELEMENT);
  writeFileSync(join(app, "dist/index.html"), HTML('src="bundle.js"'));
  writeFileSync(
    join(app, "webpack.config.js"),
    `export default { mode: "production", entry: "./main.js", output: { filename: "bundle.js", path: new URL("./dist", import.meta.url).pathname, publicPath: "auto", clean: { keep: /index\\.html/ } },
  module: { rules: [{ test: /\\.css$/, use: ["style-loader", "css-loader"] }] }, performance: { hints: false }, stats: "errors-warnings" };
`,
  );
  sh("node", [join(site, "node_modules/webpack-cli/bin/cli.js"), "--config", "webpack.config.js"], { cwd: app, timeout: 300_000 });
  return inBrowser("webpack", join(app, "dist"), { workers: true });
});

await step("esbuild", async () => {
  prepare();
  const app = join(site, "apps/esbuild");
  mkdirSync(join(app, "dist"), { recursive: true });
  writeFileSync(join(app, "main.js"), APP_CLASS);
  writeFileSync(join(app, "dist/index.html"), HTML('type="module" src="main.js"', '<link rel="stylesheet" href="main.css">'));
  sh(join(site, "node_modules/.bin/esbuild"), ["main.js", "--bundle", "--format=esm", "--outdir=dist", "--log-level=warning"], { cwd: app, timeout: 300_000 });
  // esbuild leaves new URL("x.wasm", import.meta.url) alone: the two engines go next to the bundle, as the packages' READMEs say.
  cpSync(join(site, "node_modules/@docxtor/core/wasm/docxtor_bg.wasm"), join(app, "dist/docxtor_bg.wasm"));
  cpSync(join(site, "node_modules/@docxtor/editor/dist/spelling/wasm/spell_bg.wasm"), join(app, "dist/spell_bg.wasm"));
  return inBrowser("esbuild", join(app, "dist"), { workers: false });
});

await step("next", () => {
  const app = join(work, "next");
  install(app, NEXT, tgz);
  mkdirSync(join(app, "app"), { recursive: true });
  writeFileSync(join(app, "app/layout.jsx"), `export default function Layout({ children }) { return <html lang="en"><body>{children}</body></html>; }\n`);
  writeFileSync(join(app, "app/editor.jsx"), `"use client";\nimport { useEffect, useRef } from "react";\nimport "@docxtor/editor/style.css";\nexport default function Editor() {\n  const el = useRef(null);\n  useEffect(() => {\n    let editor;\n    import("@docxtor/editor").then(({ DocxtorEditor }) => { editor = new DocxtorEditor({ fonts: false, spelling: false }).mount(el.current); });\n    return () => editor?.destroy();\n  }, []);\n  return <div ref={el} style={{ height: 400 }} />;\n}\n`);
  writeFileSync(
    join(app, "app/page.jsx"),
    `import { Engine, loadEngine } from "@docxtor/core";\nimport { DocxtorEditor } from "@docxtor/editor";\nimport { defineExtension } from "@docxtor/extensions";\nimport Editor from "./editor.jsx";\nexport const dynamic = "force-dynamic";\nexport default function Page() {\n  const names = [typeof Engine, typeof loadEngine, typeof DocxtorEditor, typeof defineExtension].join(",");\n  return <main><p>{names}</p><Editor /></main>;\n}\n`,
  );
  writeFileSync(join(app, "next.config.mjs"), `export default {};\n`);
  const out = sh("node", [join(app, "node_modules/next/dist/bin/next"), "build"], { cwd: app, timeout: 480_000, env: { NEXT_TELEMETRY_DISABLED: "1" } });
  // Next's other bundler, too (Turbopack is the default).
  rmSync(join(app, ".next"), { recursive: true, force: true });
  sh("node", [join(app, "node_modules/next/dist/bin/next"), "build", "--webpack"], { cwd: app, timeout: 480_000, env: { NEXT_TELEMETRY_DISABLED: "1" } });
  // Render it on a server too: the page's server component runs the packages' modules.
  const start = spawnSync("node", ["-e", `
    const { spawn } = require("node:child_process");
    const p = spawn("node", ["node_modules/next/dist/bin/next", "start", "-p", "43217"], { stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, NEXT_TELEMETRY_DISABLED: "1" } });
    let out = "";
    p.stdout.on("data", (d) => (out += d)); p.stderr.on("data", (d) => (out += d));
    const done = (code, msg) => { p.kill(); console.log(msg); process.exit(code); };
    const tick = async (n) => {
      try { const r = await fetch("http://127.0.0.1:43217/"); const t = await r.text(); return done(t.includes("function,function,function,function") ? 0 : 1, r.status + " " + (t.includes("function,function,function,function") ? "rendered" : "unexpected body: " + t.slice(0, 300))); } catch {}
      if (n > 60) return done(1, "no answer from next start: " + out.slice(-500));
      setTimeout(() => tick(n + 1), 500);
    };
    tick(0);
  `], { cwd: app, encoding: "utf8", timeout: 60_000 });
  if (start.status !== 0) throw new Error(`next start: ${start.stdout}${start.stderr}`);
  return `next build ok with Turbopack and webpack (${out.match(/Next\.js [\d.]+/)?.[0] ?? "next"}), server render: ${start.stdout.trim()}`;
});

console.log(`\n${results.map((r) => `${r.ok ? "PASS" : "FAIL"}  ${r.name.padEnd(9)} ${r.secs.toFixed(1).padStart(6)} s`).join("\n")}\nTotal ${((Date.now() - t0) / 1000).toFixed(0)} s`);
if (keep) console.log(`Kept ${work}`);
else rmSync(work, { recursive: true, force: true });
process.exit(results.every((r) => r.ok) ? 0 : 1);
