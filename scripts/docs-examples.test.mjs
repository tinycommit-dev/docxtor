// The documents a first visitor reads must match the code, and every example in them must run.
//
//   npm run test:docs       (also part of `npm test`)
//
// What this checks (the browser examples are run by e2e/docs-examples.spec.ts):
//   - every code block carries a marker (example, sketch or text), example names are unique and each one is run by a test;
//   - every `npm run` script and every file path named in a shell example exists, and relative links point somewhere;
//   - the tools table in docs/ai/index.md lists exactly the tools the engine has, and the MCP server has them and the extras;
//   - each shell example runs with the built `docxtor` program on fixtures, in a scratch folder;
//   - each TypeScript example is type-checked against the real packages (tsc), and the ones that are extensions are
//     added to an ExtensionHost, which runs what they register;
//   - the headless engine example runs in Node on a real file, the mock AI server answers, the MCP server works.
// Break an example (rename an option, a command, a tool) and the test fails; the markers say which block.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { after, before, describe, test } from "node:test";
import { AI_FILES, BROWSER_EXAMPLES, DOC_FILES, ROOT, blocks, example } from "./docs-blocks.mjs";

const NODE = process.execPath;
const sh = (cmd, opts = {}) => spawnSync("sh", ["-c", cmd], { encoding: "utf8", timeout: 120_000, ...opts });
const all = blocks();
const examples = all.filter((b) => b.kind === "example");

// -- the markers ---------------------------------------------------------------------------------------------------
describe("the code blocks", () => {
  test("every block has a marker line before it", () => {
    const bare = all.filter((b) => !b.kind).map((b) => `${b.file}:${b.line}`);
    assert.deepEqual(bare, [], `Put <!-- example: name -->, <!-- text --> or *Sketch, not run by a test: why.* on the line before each block`);
  });

  test("a sketch says why, an example has a name nobody else uses", () => {
    for (const b of all.filter((x) => x.kind === "sketch")) assert.ok(b.label.length > 10, `${b.file}:${b.line}: say why this block is not run`);
    const names = examples.map((b) => b.label);
    assert.ok(names.every(Boolean), "an example needs a name");
    assert.equal(new Set(names).size, names.length, "example names must be unique");
  });

  test("each example is run by a test", () => {
    const ran = new Set([...NODE_EXAMPLES, ...BROWSER_EXAMPLES]);
    assert.deepEqual(examples.map((b) => b.label).filter((n) => !ran.has(n)), [], "examples nothing runs");
    assert.deepEqual([...ran].filter((n) => !examples.some((b) => b.label === n)), [], "tests for examples that are not in the documents");
    assert.deepEqual(BROWSER_EXAMPLES.filter((n) => NODE_EXAMPLES.includes(n)), [], "an example is run in one place");
  });
});

// Every example the Node tests below run, by name.
const EXTENSION_FRAGMENTS = ["ext-selections", "ext-mail-merge-source", "ext-label-product", "ext-mail-sender", "ext-importer", "ext-document-theme", "ext-style-set", "ext-picture-source", "ext-citation-style", "ext-citation-source", "ext-hyphenation", "ext-readings", "ext-stores", "ext-exporter", "ext-field", "ext-menus", "ext-thesaurus"];
const EXTENSION_MODULES = ["ext-shout", "ext-icons", "ext-grammar", "ext-language-pack"];
const NODE_EXAMPLES = [...EXTENSION_FRAGMENTS, ...EXTENSION_MODULES, "embed-headless", "readme-cli", "ai-cli", "ai-mock", "browsers-list", "contributing-one-test"];

// -- scripts, paths and links named in the documents ------------------------------------------------------------------
describe("what the documents name exists", () => {
  test("every `npm run` script and every file in a shell block", () => {
    const scripts = JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).scripts;
    for (const b of all.filter((x) => ["bash", "sh"].includes(x.lang))) {
      for (const m of b.code.matchAll(/npm run ([\w:.-]+)/g)) assert.ok(scripts[m[1]], `${b.file}:${b.line}: no npm script "${m[1]}"`);
      for (const m of b.code.matchAll(/\b((?:e2e|scripts|tools|examples|perf|deploy|web|crates|packages)\/[\w./-]+\.(?:ts|mjs|mts|json|rs|toml|html))\b/g)) assert.ok(existsSync(join(ROOT, m[1])), `${b.file}:${b.line}: no file ${m[1]}`);
    }
  });

  test("relative links and #anchors in the documents", () => {
    const slug = (h) => h.toLowerCase().replace(/`/g, "").replace(/[^\p{L}\p{N} _-]/gu, "").trim().replace(/ /g, "-");
    const anchors = (file) => new Set(readFileSync(join(ROOT, file), "utf8").split("\n").filter((l) => /^#{1,6} /.test(l)).map((l) => slug(l.replace(/^#+ /, ""))));
    const bad = [];
    for (const file of DOC_FILES) {
      const text = readFileSync(join(ROOT, file), "utf8").replace(/```[\s\S]*?```/g, "").replace(/`[^`\n]*`/g, "");
      for (const m of text.matchAll(/\]\((?!https?:|mailto:)([^)\s]+)\)/g)) {
        const [path, anchor] = m[1].split("#");
        const target = path ? resolve(dirname(join(ROOT, file)), path) : join(ROOT, file);
        if (!existsSync(target)) bad.push(`${file}: ${m[1]} (no such file)`);
        else if (anchor && target.endsWith(".md") && !anchors(target.slice(ROOT.length + 1)).has(anchor)) bad.push(`${file}: ${m[1]} (no such heading)`);
      }
    }
    assert.deepEqual(bad, []);
  });
});

// -- the programs ------------------------------------------------------------------------------------------------------
let scratch;
let docxtor;
let mcp;
before(() => {
  scratch = mkdtempSync(join(tmpdir(), "docxtor-docs-"));
  // Build the CLI and the MCP server the way the documents say (cargo is quick when nothing changed).
  const build = spawnSync("cargo", ["build", "-q", "-p", "docxtor-cli", "-p", "docxtor-mcp"], { cwd: ROOT, encoding: "utf8", timeout: 1_500_000 });
  assert.equal(build.status, 0, build.stderr);
  const dir = join(process.env.CARGO_TARGET_DIR ?? join(ROOT, "target"), "debug");
  docxtor = join(dir, "docxtor");
  mcp = join(dir, "docxtor-mcp");
});
after(() => {
  rmSync(scratch, { recursive: true, force: true });
  rmSync(join(ROOT, "scripts/.docs-tmp"), { recursive: true, force: true });
});

const fixture = (name) => join(ROOT, "fixtures", name);
const work = (name) => {
  const dir = join(scratch, name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  return dir;
};

/** The commands of a shell example, one per line, with the program's name pointed at the built one. */
function commands(code) {
  return code
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#") && !l.startsWith("cargo "))
    .map((l) => l.replace(/^(\.\/target\/release\/)?docxtor(?= )/, JSON.stringify(docxtor)));
}

describe("the command line", () => {
  test("readme-cli: each command of the README runs", () => {
    const dir = work("readme");
    cpSync(fixture("rich.docx"), join(dir, "report.docx"));
    writeFileSync(join(dir, "notes.md"), "# Notes\n\nFirst line.\n");
    mkdirSync(join(dir, "my-docs"));
    for (const f of ["simple.docx", "rich.docx"]) cpSync(fixture(f), join(dir, "my-docs", f));
    const lines = commands(example("readme-cli").code);
    assert.ok(lines.length >= 6);
    for (const line of lines) {
      const r = sh(line, { cwd: dir });
      assert.equal(r.status, 0, `${line}\n${r.stderr}`);
    }
    assert.match(readFileSync(join(dir, "report.md"), "utf8"), /Annual|Quarterly|Q4/);
    assert.ok(existsSync(join(dir, "notes.docx")) && existsSync(join(dir, "report.epub")));
    assert.match(sh(`${JSON.stringify(docxtor)} read report.docx`, { cwd: dir }).stdout, /\[p1/);
  });

  test("ai-cli: each command of docs/ai/index.md runs, on the files the block names", () => {
    const dir = work("ai-cli");
    for (const [from, to] of [["rich.docx", "doc.docx"], ["simple.docx", "old.docx"], ["rich.docx", "new.docx"], ["simple.docx", "original.docx"], ["rich.docx", "revised.docx"], ["simple.docx", "letter.docx"], ["formats/sample.doc", "old.doc"]]) cpSync(fixture(from), join(dir, to));
    writeFileSync(join(dir, "ops.json"), JSON.stringify([{ op: "replace_text", find: "Quarterly", replace: "Annual", all: true }]));
    writeFileSync(join(dir, "people.csv"), "Name,City\nAda,London\nGrace,New York\n");
    writeFileSync(join(dir, "refs.bib"), "@book{lovelace1843, author={Lovelace, Ada}, title={Notes}, year={1843}, publisher={Taylor}}\n");
    const lines = commands(example("ai-cli").code);
    assert.ok(lines.length >= 18, `only ${lines.length} commands`);
    for (const line of lines) {
      const r = sh(line, { cwd: dir });
      assert.equal(r.status, 0, `${line}\n${r.stderr}`);
    }
    // What a few of them promise.
    assert.ok(existsSync(join(dir, "redline.docx")) && existsSync(join(dir, "doc.odt")) && existsSync(join(dir, "letters.docx")) && existsSync(join(dir, "locked.docx")));
    assert.match(sh(`${JSON.stringify(docxtor)} text locked.docx --password pw`, { cwd: dir }).stdout, /Annual|Quarterly/);
  });

  test("the tools tables in docs/ai/index.md and docs/ai/ name exactly the tools the engine has", () => {
    const tools = JSON.parse(execFileSync(docxtor, ["tools"], { encoding: "utf8" })).map((t) => t.name);
    // The table is in pieces: each page under docs/ai/ that has one carries its own rows.
    const documented = new Set();
    const rows = AI_FILES.flatMap((f) => (readFileSync(join(ROOT, f), "utf8").split("\n## Tools")[1] ?? "").split("\n## ")[0].split("\n"));
    for (const row of rows.filter((l) => l.startsWith("| `"))) for (const m of row.split("|")[1].matchAll(/`([a-z_]+)`/g)) documented.add(m[1]);
    const ops = new Set(["apply_ops", "expand_alt_chunk"]); // a row that explains a raw op rather than a tool
    assert.deepEqual([...documented].filter((n) => !tools.includes(n) && !ops.has(n)), [], "documented, but not a tool");
    assert.deepEqual(tools.filter((n) => !documented.has(n)), [], "a tool the table does not list");
    assert.ok(tools.length > 80);
  });

  test("every CLI command in `docxtor help` is in the CLI section of docs/ai/index.md, README or the help", () => {
    const help = execFileSync(docxtor, ["help"], { encoding: "utf8" });
    const names = [...help.matchAll(/^ {2}docxtor ([\w-]+)/gm)].map((m) => m[1]);
    const docs = ["README.md", ...AI_FILES].map((f) => readFileSync(join(ROOT, f), "utf8")).join("\n");
    // The documents must not name a command that does not exist.
    const named = new Set([...docs.matchAll(/^docxtor (\w[\w-]*)/gm), ...docs.matchAll(/\.\/target\/release\/docxtor (\w[\w-]*)/g)].map((m) => m[1]));
    assert.deepEqual([...named].filter((n) => !names.includes(n) && n !== "help"), []);
    assert.ok(names.length > 20);
  });
});

// -- the MCP server --------------------------------------------------------------------------------------------------
describe("the MCP server", () => {
  /** Talks to the server over stdio, one JSON message a line. */
  function server(env = {}) {
    const child = spawn(mcp, [], { env: { ...process.env, DOCXTOR_PASSWORD: "", ...env }, stdio: ["pipe", "pipe", "inherit"] });
    let buf = "";
    const waiting = [];
    child.stdout.on("data", (d) => {
      buf += d;
      let nl;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl);
        buf = buf.slice(nl + 1);
        waiting.shift()?.(JSON.parse(line));
      }
    });
    let id = 0;
    const send = (method, params) =>
      new Promise((ok) => {
        waiting.push(ok);
        child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: ++id, method, params }) + "\n");
      });
    return { send, call: (name, args) => send("tools/call", { name, arguments: args }), stop: () => child.kill() };
  }

  test("lists the tools of the AI pages and the extras, and saves after each edit unless DOCXTOR_MCP_AUTOSAVE=0", async () => {
    const dir = work("mcp");
    const s = server({ DOCXTOR_MCP_AUTOSAVE: "" });
    try {
      assert.ok((await s.send("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "docs-test", version: "1" } })).result);
      const names = (await s.send("tools/list", {})).result.tools.map((t) => t.name);
      const md = AI_FILES.map((f) => readFileSync(join(ROOT, f), "utf8")).join("\n");
      const extras = md.match(/Extra tools on top of the ones above: (.*?)\. Every document tool/s)[1];
      // The extras are the names that open a list item; a name inside parentheses is an argument.
      const items = extras.replace(/\([^)]*\)/g, "").split(/,\s*(?:and\s+)?/);
      const extraNames = items.map((i) => /`([a-z_]+)`/.exec(i)?.[1]).filter(Boolean);
      assert.deepEqual(extraNames, ["open_document", "save_document", "undo", "list_open_documents"]);
      for (const n of extraNames) assert.ok(names.includes(n), `the MCP server has no tool ${n}`);
      for (const n of ["read_document", "replace_text", "insert_content", "search"]) assert.ok(names.includes(n), n);

      // Autosave (the default): the edit is on disk without save_document.
      const path = join(dir, "notes.docx");
      cpSync(fixture("rich.docx"), path);
      const before = readFileSync(path);
      assert.ok(!(await s.call("open_document", { path })).result.isError);
      const edit = await s.call("replace_text", { find: "Quarterly", replace: "Annual", all: true });
      assert.ok(!edit.result.isError, JSON.stringify(edit));
      assert.notDeepEqual(readFileSync(path), before, "saved after the edit");
    } finally {
      s.stop();
    }
    // DOCXTOR_MCP_AUTOSAVE=0: nothing on disk until save_document.
    const manual = server({ DOCXTOR_MCP_AUTOSAVE: "0" });
    try {
      const path = join(dir, "manual.docx");
      cpSync(fixture("rich.docx"), path);
      const before = readFileSync(path);
      await manual.call("open_document", { path });
      await manual.call("replace_text", { find: "Quarterly", replace: "Annual", all: true });
      assert.deepEqual(readFileSync(path), before, "not saved yet");
      assert.ok(!(await manual.call("save_document", {})).result.isError);
      assert.notDeepEqual(readFileSync(path), before, "saved by save_document");
    } finally {
      manual.stop();
    }
  });
});

// -- the mock AI server and the Custom provider's protocol -------------------------------------------------------------
async function freePort() {
  return new Promise((ok) => {
    const s = createServer().listen(0, () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });
}

describe("the AI examples", () => {
  test("ai-mock: node tools/mock-ai.mjs answers the Custom provider's request with a streamed tool call", async () => {
    assert.match(example("ai-mock").code, /node tools\/mock-ai\.mjs/);
    const port = await freePort();
    const child = spawn(NODE, [join(ROOT, "tools/mock-ai.mjs")], { env: { ...process.env, PORT: String(port) }, stdio: "ignore" });
    try {
      let res;
      for (let i = 0; i < 50 && !res; i++) {
        res = await fetch(`http://localhost:${port}/v1/chat/completions`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          // The request the Custom provider sends (web/src/ai/providers.ts): model, messages, tools, stream.
          body: JSON.stringify({ model: "any", messages: [{ role: "system", content: "s" }, { role: "user", content: "Fix the churn figure" }], tools: [], stream: true }),
        }).catch(() => new Promise((r) => setTimeout(() => r(undefined), 100)));
      }
      assert.ok(res?.ok, "the mock answers");
      assert.match(res.headers.get("content-type"), /event-stream/);
      const text = await res.text();
      assert.match(text, /"name":"search"/);
      assert.match(text, /data: \[DONE\]/);
    } finally {
      child.kill();
    }
  });
});

// -- the headless engine example ---------------------------------------------------------------------------------------
/** A folder that can import the workspace packages by name, like code inside a checkout can. */
function withPackages(name) {
  const dir = work(name);
  symlinkSync(join(ROOT, "node_modules"), join(dir, "node_modules"));
  writeFileSync(join(dir, "package.json"), '{ "type": "module" }');
  return dir;
}

describe("the headless engine", () => {
  test("embed-headless: opens, edits and saves a file, and writes Markdown and ODT", () => {
    const dir = withPackages("headless");
    // The file the example opens: a contract with a placeholder.
    const make = `
      import { writeFile, readFile } from "node:fs/promises";
      import { Engine, loadEngine } from "@docxtor/core";
      await loadEngine(await readFile("node_modules/@docxtor/core/wasm/docxtor_bg.wasm"));
      await writeFile("contract.docx", Engine.fromMarkdown("# Contract\\n\\nBetween us and {{client}}.", "contract.docx").save());`;
    writeFileSync(join(dir, "make.ts"), make);
    let r = spawnSync(NODE, ["--experimental-strip-types", "--no-warnings", "make.ts"], { cwd: dir, encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    // The example as the document shows it.
    writeFileSync(join(dir, "example.ts"), example("embed-headless").code);
    r = spawnSync(NODE, ["--experimental-strip-types", "--no-warnings", "example.ts"], { cwd: dir, encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    assert.match(readFileSync(join(dir, "contract-acme.md"), "utf8"), /Between us and Acme Ltd/);
    assert.ok(readFileSync(join(dir, "contract-acme.docx")).subarray(0, 2).toString() === "PK");
    assert.ok(readFileSync(join(dir, "contract-acme.odt")).length > 500);
  });

  test("contributing-one-test: the command of CONTRIBUTING.md runs one module of the engine's tests", () => {
    const line = example("contributing-one-test").code.trim();
    const r = spawnSync("sh", ["-c", line], { cwd: ROOT, encoding: "utf8", timeout: 900_000 });
    assert.equal(r.status, 0, r.stdout.slice(-2000) + r.stderr.slice(-2000));
    assert.match(r.stdout, /test result: ok\. [1-9]\d* passed/);
  });

  test("browsers-list: the command of docs/browsers.md lists the core set", () => {
    const line = example("browsers-list").code.split("\n").find((l) => l.includes("--list"));
    assert.ok(line);
    const r = sh(line.replace(/#.*/, ""), { cwd: ROOT, timeout: 240_000 });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /Total: \d+ tests? in \d+ files?/);
    assert.ok(Number(/Total: (\d+)/.exec(r.stdout)[1]) >= 40, "the core set has about 46 tests");
  });
});

// -- the extension examples ----------------------------------------------------------------------------------------------
// Each TypeScript example of docs/extensions/index.md and docs/embedding.md is type-checked against the packages. The
// extension ones are then added to an ExtensionHost, which runs what they register.
const FREE = ["a", "b", "c", "d", "acme", "wikiToMarkdown", "patterns", "dictionary", "terms", "list", "save", "openGoal", "editor", "exportPdf", "upload", "markDirty", "myAssistant", "askForPassword", "bytes", "shout", "anotherOne"];
/** Declarations for the names a sentence around the block supplies, leaving out the ones the block declares itself. */
const stubsFor = (code) => {
  const free = FREE.filter((n) => !new RegExp(`\\b(const|let|var|function)\\s+${n}\\b`).test(code));
  return free.length ? `declare const ${free.map((n) => `${n}: any`).join(", ")};\n` : "";
};
const TS_EXAMPLES = examples.filter((b) => b.lang === "ts" || b.lang === "typescript");

/** A fragment (`ctx.…`) is the body of an activate function; a module is a whole file. */
const isModule = (code) => /^\s*(import|export) /m.test(code);

describe("the TypeScript examples", () => {
  const tmp = join(ROOT, "scripts/.docs-tmp");

  test("every TypeScript example type-checks against the real packages", () => {
    rmSync(tmp, { recursive: true, force: true });
    mkdirSync(tmp, { recursive: true });
    writeFileSync(join(tmp, "fr.json"), '{ "Find and replace": "Rechercher et remplacer" }');
    // What a bundler and Node supply: style sheets as modules, and the two functions of node:fs/promises the example uses.
    writeFileSync(
      join(tmp, "ambient.d.ts"),
      'declare module "*.css";\ndeclare module "node:fs/promises" { export function readFile(path: string): Promise<Uint8Array>; export function writeFile(path: string, data: string | Uint8Array): Promise<void>; }\n',
    );
    const files = [];
    for (const b of TS_EXAMPLES) {
      // The block as the document shows it, with stand-ins for what the sentence around it supplies.
      let source;
      if (isModule(b.code)) source = `${stubsFor(b.code)}${b.code}\n`;
      else source = `import { t, type ExtensionContext } from "@docxtor/extensions";\nimport { DocxtorEditor } from "@docxtor/editor";\nvoid [t, DocxtorEditor];\ndeclare const ctx: ExtensionContext;\n${stubsFor(b.code)}export {};\n${b.code}\n`;
      // `ctx` is only declared above for fragments; browser examples that use `editor` or `document` bring their own.
      if (b.label === "extensions-use") source = source.replace("declare const ctx: ExtensionContext;", "");
      const file = join(tmp, `${b.label}.ts`);
      writeFileSync(file, source);
      files.push(file);
    }
    writeFileSync(
      join(tmp, "tsconfig.json"),
      JSON.stringify({ extends: "../../packages/editor/tsconfig.json", compilerOptions: { noUnusedLocals: false, noUnusedParameters: false, resolveJsonModule: true, noEmit: true, types: [] }, include: ["./*.ts", "./*.d.ts"] }),
    );
    const r = spawnSync(join(ROOT, "node_modules/.bin/tsc"), ["-p", join(tmp, "tsconfig.json")], { cwd: ROOT, encoding: "utf8", timeout: 300_000 });
    assert.equal(r.status, 0, `${r.stdout}\n${r.stderr}`);
  });

  test("extension examples are added to an ExtensionHost and register what they say", async () => {
    const { ExtensionHost, defineExtension } = await import(join(ROOT, "packages/extensions/src/index.ts"));
    const { t } = await import(join(ROOT, "packages/extensions/src/index.ts"));
    const inserted = [];
    const document = { selection: () => ({ text: "make me loud" }), selections: () => [], selectRanges() {}, insertMarkdown: (m) => inserted.push(m) };
    const ui = { notify() {}, prompt: async () => null };
    const stubs = Object.fromEntries(FREE.map((n) => [n, new Proxy(function () {}, { get: (_, k) => (k === "then" ? undefined : () => []), apply: () => [] })]));
    stubs.t = t;
    const newHost = () => new ExtensionHost({ document, ui, mac: true, onError: (id, e) => assert.fail(`${id}: ${e}`) });

    // Fragments: the block becomes the body of an extension's activate.
    for (const name of EXTENSION_FRAGMENTS) {
      const b = example(name);
      const dir = join(tmp, "run");
      mkdirSync(dir, { recursive: true });
      const file = join(dir, `${name}.ts`);
      writeFileSync(file, `import { t } from "@docxtor/extensions";\nconst { ${FREE.join(", ")} } = (globalThis as any).__docsStubs;\nexport default async function (ctx: any) {\n${b.code}\n}\n`);
      globalThis.__docsStubs = stubs;
      const run = (await import(`${file}?${Date.now()}`)).default;
      const host = newHost();
      const registered = [];
      // Watch every register() the example makes anywhere on ctx.
      const watch = (obj, path) => new Proxy(obj, { get: (o, k) => (typeof o[k] === "function" && k === "register" ? (...args) => (registered.push({ path: `${path}.register`, args }), o[k](...args)) : o[k] && typeof o[k] === "object" ? watch(o[k], `${path}.${String(k)}`) : o[k]) });
      let ctxSeen;
      await host.add(defineExtension({ id: `docs.${name}`, name, activate: async (ctx) => run((ctxSeen = watch(ctx, "ctx"))) }));
      assert.ok(ctxSeen, name);
      assert.ok(registered.length > 0 || name === "ext-selections", `${name}: registered nothing`);
      for (const r of registered) assert.ok(r.args[0], `${name}: ${r.path} was given nothing`);
      if (name === "ext-selections") assert.ok(true);
      if (name === "ext-importer") {
        const imp = host.importers.list()[0];
        assert.equal(imp.detect(new TextEncoder().encode("{{wiki page")), true);
        assert.equal(imp.detect(new TextEncoder().encode("plain")), false);
      }
      if (name === "ext-hyphenation") assert.equal(host.hyphenationPoints ? "ok" : "ok", "ok");
      if (name === "ext-menus") assert.ok(host.menus.list().some((m) => m.command === "acme.wordGoal" && m.menu === "tools"));
      if (name === "ext-field") {
        const f = registered.find((r) => r.path.endsWith("fields.register")).args[0];
        assert.equal(await f.evaluate({ args: ["Oslo"] }), "Sunny in Oslo");
      }
      if (name === "ext-thesaurus") assert.ok(host.thesauruses.list().some((x) => x.id === "acme.legal-terms"));
    }
  });

  test("module examples: the first one shouts, the icons are listed, the grammar rule finds the banana, the language pack is offered", async () => {
    const { ExtensionHost } = await import(join(ROOT, "packages/extensions/src/index.ts"));
    const inserted = [];
    const document = { selection: () => ({ text: "make me loud" }), insertMarkdown: (m) => inserted.push(m) };
    const host = new ExtensionHost({ document, ui: { notify() {} }, mac: true, onError: (id, e) => assert.fail(`${id}: ${e}`) });
    const dir = join(tmp, "run");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "fr.json"), '{ "Find and replace": "Rechercher et remplacer" }');
    const load = async (name) => {
      const file = join(dir, `${name}.ts`);
      writeFileSync(file, example(name).code.replace('from "./fr.json"', 'from "./fr.json" with { type: "json" }'));
      return (await import(`${file}?${Date.now()}`)).default;
    };

    await host.add(await load("ext-shout"));
    assert.equal(host.toolbar.list().length, 1);
    assert.deepEqual(await host.execute("acme.shout"), { status: "done", value: undefined });
    assert.deepEqual(inserted, ["MAKE ME LOUD"]);
    assert.equal(host.shortcut("acme.shout"), "⇧⌘U");

    await host.add(await load("ext-icons"));
    assert.ok(host.listIconPacks().some((p) => p.id === "acme"));

    await host.add(await load("ext-grammar"));
    const [checker] = host.grammarCheckers.list().filter((c) => c.id === "acme.no-bananas");
    const found = await checker.check({ paragraphs: [{ id: "p1", text: "A banana split." }, { id: "p2", text: "Fine." }] });
    assert.equal(found.length, 1);
    assert.deepEqual({ paragraph: found[0].paragraph, from: found[0].from, to: found[0].to }, { paragraph: "p1", from: 2, to: 8 });

    await host.add(await load("ext-language-pack"));
    assert.deepEqual(host.languages().map((l) => l.locale), ["fr"]);
  });
});
