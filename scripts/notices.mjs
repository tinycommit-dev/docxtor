// Third-party licence notices: generated, and checked.
//
//   npm run notices          writes every notices file
//   npm run check:notices    fails when a file differs from what is committed, or when something shipped has a licence
//                            outside the allow-list (and, if web/dist exists, when the built app lacks its licence file)
//
// What it writes (each ships with the thing it names):
//   packages/core/THIRD_PARTY_LICENSES.txt      the Rust crates compiled into the engine (docxtor-wasm, for the browser)
//   packages/editor/THIRD_PARTY_LICENSES.txt    the Rust crates in the spelling checker's WebAssembly, and the Brotli tables
//   crates/docxtor-cli/THIRD_PARTY_LICENSES.txt, crates/docxtor-mcp/THIRD_PARTY_LICENSES.txt   the crates in each program
//   web/public/licenses.txt                     the app: both WebAssembly programs, the npm packages in the build (scripts,
//                                               fonts, dictionaries), THIRD_PARTY_NOTICES.md; Vite copies it to web/dist
//
// The Rust crates come from `cargo about` (pinned in CI: cargo-about 0.9.3; install with
// `cargo install --locked cargo-about --version 0.9.3 --features cli`), configured in about/*.toml, where the allow-list for
// crates lives. The npm packages come from the Vite build (`npm run build` writes node_modules/.cache/docxtor-notices/
// bundled-packages.json from the modules and assets it bundles), so devDependencies never appear.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const checkOnly = process.argv.includes("--check");

/** SPDX ids a shipped dependency may use. The same list is `accepted` in about/*.toml for the Rust crates. */
const ALLOWED = new Set([
  "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "MPL-2.0", "OFL-1.1", "Unicode-3.0", "CC0-1.0",
  "Apache-2.0 WITH LLVM-exception",
]);

/**
 * Packages whose licence is outside the allow-list or is not an SPDX id, each accepted by name with the reason.
 * The Hunspell dictionaries are data the app fetches at run time (THIRD_PARTY_NOTICES.md says so); the German and Spanish
 * ones are GPL.
 */
const EXCEPTIONS = {
  "dictionary-de": { license: "(GPL-2.0 OR GPL-3.0)", why: "GPL dictionary, shipped as data and unchanged (THIRD_PARTY_NOTICES.md)" },
  "dictionary-es": { license: "(GPL-3.0 OR LGPL-3.0 OR MPL-1.1)", why: "GPL, LGPL or MPL-1.1 dictionary, shipped as data and unchanged (THIRD_PARTY_NOTICES.md)" },
  "fast-sha256": { license: "Unlicense", why: "public-domain dedication with no conditions (pulled in by the Anthropic SDK's standardwebhooks)" },
  "dictionary-en": { license: "(MIT AND BSD)", why: "SCOWL: MIT and BSD, written without an SPDX version; its licence file says which" },
  "dictionary-en-gb": { license: "(MIT AND BSD)", why: "SCOWL: MIT and BSD, written without an SPDX version; its licence file says which" },
};

const RUST = [
  { name: "engine", manifest: "crates/docxtor-wasm/Cargo.toml", config: "about/wasm.toml", out: "packages/core/THIRD_PARTY_LICENSES.txt", title: "@docxtor/core: the engine (Rust compiled to WebAssembly)" },
  { name: "spelling", manifest: "crates/docxtor-spell/Cargo.toml", config: "about/wasm.toml", out: "packages/editor/THIRD_PARTY_LICENSES.txt", title: "@docxtor/editor: the spelling checker (Rust compiled to WebAssembly)" },
  { name: "cli", manifest: "crates/docxtor-cli/Cargo.toml", config: "about/native.toml", out: "crates/docxtor-cli/THIRD_PARTY_LICENSES.txt", title: "the docxtor command-line program" },
  { name: "mcp", manifest: "crates/docxtor-mcp/Cargo.toml", config: "about/native.toml", out: "crates/docxtor-mcp/THIRD_PARTY_LICENSES.txt", title: "the docxtor MCP server" },
];

const BROTLI_NOTE = `Brotli tables (@docxtor/editor, the PDF writer)
packages/editor/src/pdf/brotli-data.ts holds data from the Brotli reference decoder: the static dictionary, the context
table and the word transforms of RFC 7932. Those are MIT licensed, Copyright 2013 Google Inc. The rest of the PDF
writer's WOFF2 reader is Docxtor's own code.
`;

const MIT_TEXT = `Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to
whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.`;

const rule = (c) => c.repeat(78);
const section = (title) => `${rule("=")}\n${title}\n${rule("=")}\n\n`;
const lf = (s) => s.replace(/\r\n?/g, "\n").replace(/\s+$/, "") + "\n";

// ---------------------------------------------------------------------------------------------------------------- Rust

/** The crates a program compiles in, from cargo about: [{ key, name, version, repository, license, text }], sorted. */
function rustCrates(entry) {
  let raw;
  try {
    raw = execFileSync(
      "cargo",
      ["about", "generate", "--manifest-path", join(root, entry.manifest), "--config", join(root, entry.config), "--format", "json", "--locked", "--fail"],
      { cwd: root, encoding: "utf8", maxBuffer: 1 << 28, stdio: ["ignore", "pipe", "inherit"] },
    );
  } catch {
    console.error(`cargo about failed for ${entry.manifest} (above): a crate has a licence outside the allow-list in ${entry.config}, or cargo-about is not installed (cargo install --locked cargo-about --version 0.9.3 --features cli).`);
    process.exit(1);
  }
  const data = JSON.parse(raw);
  const crates = [];
  for (const lic of data.licenses) {
    for (const use of lic.used_by) {
      const c = use.crate;
      if (c.name.startsWith("docxtor-")) continue; // Docxtor's own crates
      crates.push({ name: c.name, version: c.version, repository: c.repository ?? "", license: lic.id, text: lf(lic.text) });
    }
  }
  crates.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version) || a.license.localeCompare(b.license));
  return crates;
}

/** Crates grouped by identical licence text, so a text that many crates share is printed once. */
function renderCrates(crates) {
  const byText = new Map();
  for (const c of crates) {
    const key = `${c.license}\n${c.text}`;
    if (!byText.has(key)) byText.set(key, { license: c.license, text: c.text, crates: [] });
    byText.get(key).crates.push(c);
  }
  const groups = [...byText.values()].sort((a, b) => a.license.localeCompare(b.license) || a.crates[0].name.localeCompare(b.crates[0].name));
  let out = "";
  for (const g of groups) {
    out += `${rule("-")}\n${g.license} licence, used by:\n`;
    for (const c of g.crates) out += `  ${c.name} ${c.version}${c.repository ? ` (${c.repository})` : ""}\n`;
    out += `${rule("-")}\n\n${g.text}\n`;
  }
  return out;
}

const own = (what) => `Docxtor is dual-licensed under MIT or Apache-2.0 (LICENSE-MIT, LICENSE-APACHE in the repository).
This file credits the third-party software that ships in ${what}, with the licence texts it asks to be kept.
It is generated by \`npm run notices\`; do not edit it by hand.

`;

// ------------------------------------------------------------------------------------------------------------------ npm

/** Does an SPDX expression pass the allow-list? Handles OR, AND, WITH and parentheses. */
function spdxAllowed(expr) {
  const tokens = expr.match(/\(|\)|[^\s()]+/g) ?? [];
  let i = 0;
  const parseOr = () => {
    let ok = parseAnd();
    while (tokens[i] === "OR") { i++; const r = parseAnd(); ok = ok || r; }
    return ok;
  };
  const parseAnd = () => {
    let ok = parseTerm();
    while (tokens[i] === "AND") { i++; const r = parseTerm(); ok = ok && r; }
    return ok;
  };
  const parseTerm = () => {
    if (tokens[i] === "(") { i++; const ok = parseOr(); i++; return ok; }
    let id = tokens[i++] ?? "";
    if (tokens[i] === "WITH") { id += ` WITH ${tokens[i + 1]}`; i += 2; }
    return ALLOWED.has(id);
  };
  return parseOr();
}

/** The npm packages in the build, each with its licence expression and licence files. Returns { packages, problems }. */
function npmPackages() {
  const listFile = join(root, "node_modules/.cache/docxtor-notices/bundled-packages.json");
  if (!existsSync(listFile)) throw new Error("No list of bundled packages: run `npm run build` first (the Vite build writes it).");
  const listed = JSON.parse(readFileSync(listFile, "utf8"));
  const problems = [];
  const packages = listed.map(({ name }) => {
    const dir = join(root, "node_modules", name);
    const info = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const license = typeof info.license === "string" ? info.license : (info.license?.type ?? "");
    const exception = EXCEPTIONS[name];
    if (exception) {
      if (exception.license !== license) problems.push(`${name}: its licence is now "${license}", the exception in scripts/notices.mjs is for "${exception.license}"`);
    } else if (!license || !spdxAllowed(license)) {
      problems.push(`${name}@${info.version}: licence "${license || "none declared"}" is outside the allow-list`);
    }
    const files = readdirSync(dir).filter((f) => /^(licen[cs]e|copying|notice)(\.|-|$)/i.test(f)).sort();
    const texts = files.map((f) => ({ file: f, text: lf(readFileSync(join(dir, f), "utf8")) }));
    if (!files.length) {
      // A package that declares MIT but ships no licence file: print the MIT text under the author its package.json names.
      const author = typeof info.author === "string" ? info.author : info.author?.name;
      if (license === "MIT" && author) texts.push({ file: "package.json (the package has no licence file)", text: lf(`Copyright (c) ${author}\n\n${MIT_TEXT}`) });
      else problems.push(`${name}@${info.version}: no licence file in the package to copy`);
    }
    return { name, version: info.version, license, homepage: info.homepage ?? info.repository?.url ?? "", texts };
  });
  packages.sort((a, b) => a.name.localeCompare(b.name));
  return { packages, problems };
}

function renderNpm(packages) {
  let out = "";
  for (const p of packages) {
    out += `${rule("-")}\n${p.name} ${p.version}\nLicence: ${EXCEPTIONS[p.name]?.license ?? p.license}${p.homepage ? `\n${p.homepage}` : ""}\n${rule("-")}\n\n`;
    for (const t of p.texts) out += `${p.texts.length > 1 ? `[${t.file}]\n\n` : ""}${t.text}\n`;
  }
  return out;
}

// ------------------------------------------------------------------------------------------------------------- the files

function generate() {
  const outputs = new Map();
  const problems = [];
  const crates = {};
  for (const entry of RUST) crates[entry.name] = rustCrates(entry);

  for (const entry of RUST) {
    let text = `${section(`Third-party licences: ${entry.title}`)}${own(entry.title)}`;
    text += `${section("Rust crates")}${renderCrates(crates[entry.name])}`;
    if (entry.name === "spelling") text += `${section("Other")}${BROTLI_NOTE}`;
    outputs.set(entry.out, lf(text));
  }

  // The app: both WebAssembly programs, each crate once (the same crate in both is listed once), then npm.
  const merged = new Map();
  for (const c of [...crates.engine, ...crates.spelling]) merged.set(`${c.name}@${c.version}@${c.license}`, c);
  const { packages, problems: npmProblems } = npmPackages();
  problems.push(...npmProblems);
  const notices = lf(readFileSync(join(root, "THIRD_PARTY_NOTICES.md"), "utf8"));
  let app = `${section("Docxtor: third-party licences")}${own("the Docxtor web app")}`;
  app += `${section("Contents")}1. Spelling dictionaries, citation styles, other libraries and data (THIRD_PARTY_NOTICES.md)\n2. npm packages in the build: scripts, fonts and dictionaries\n3. Rust crates compiled into the engine and the spelling checker (WebAssembly)\n4. Icons and Brotli tables\n\n`;
  app += `${section("1. Notices")}${notices}\n`;
  app += `${section("2. npm packages in the build")}${renderNpm(packages)}`;
  app += `${section("3. Rust crates in the WebAssembly programs")}${renderCrates([...merged.values()].sort((a, b) => a.name.localeCompare(b.name)))}`;
  app += `${section("4. Icons and Brotli tables")}Icons: every icon in the app and the editor is drawn for Docxtor (inline SVG in packages/editor/src/ui/*icons.ts and\nweb/public/favicon.svg), released with it under MIT or Apache-2.0. No third-party icon set is bundled.\n\n${BROTLI_NOTE}`;
  outputs.set("web/public/licenses.txt", lf(app));
  return { outputs, problems, packages, crates };
}

const { outputs, problems, packages, crates } = generate();

let failed = false;
const fail = (msg) => { console.error(msg); failed = true; };

for (const p of problems) fail(`licence problem: ${p}`);

if (checkOnly) {
  for (const [file, text] of outputs) {
    const path = join(root, file);
    if (!existsSync(path) || readFileSync(path, "utf8") !== text) fail(`${file} is out of date: run \`npm run notices\` and commit it`);
  }
  const dist = join(root, "web/dist/licenses.txt");
  if (existsSync(join(root, "web/dist")) && (!existsSync(dist) || readFileSync(dist, "utf8") !== outputs.get("web/public/licenses.txt"))) {
    fail("web/dist/licenses.txt is missing or differs from web/public/licenses.txt: rebuild with `npm run build`");
  }
} else {
  for (const [file, text] of outputs) writeFileSync(join(root, file), text);
}

const kinds = new Map();
for (const c of new Set(Object.values(crates).flat().map((c) => `${c.name}@${c.version}\t${c.license}`))) {
  const kind = c.split("\t")[1];
  kinds.set(kind, (kinds.get(kind) ?? 0) + 1);
}
console.log(`Rust crates by licence: ${[...kinds].sort().map(([k, n]) => `${k} ${n}`).join(", ")}`);
const npmKinds = new Map();
for (const p of packages) npmKinds.set(p.license, (npmKinds.get(p.license) ?? 0) + 1);
console.log(`npm packages in the build by licence: ${[...npmKinds].sort().map(([k, n]) => `${k} ${n}`).join(", ")}`);
console.log(checkOnly ? (failed ? "Notices check FAILED." : "Notices check passed.") : `Wrote ${outputs.size} notices files.`);
process.exit(failed ? 1 : 0);
