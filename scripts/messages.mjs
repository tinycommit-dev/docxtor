#!/usr/bin/env node
// Collect every piece of interface text (t("…"), tn(n, "…", "…"), msg("…")) into locales/source.json,
// the list a language pack translates.
//
//   node scripts/messages.mjs                 write locales/source.json
//   node scripts/messages.mjs --check         fail if it's out of date (CI)
//   node scripts/messages.mjs --pack es.json  list what a pack is missing, and keys it has that no longer exist
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const SOURCES = ["packages/editor/src", "web/src"];
const OUT = join(root, "locales/source.json");

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* files(p);
    else if (/\.ts$/.test(name) && !/\.d\.ts$/.test(name)) yield p;
  }
}

// A string literal: "…", '…' or `…` without ${}.
const LIT = String.raw`("(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|\`(?:[^\`\\$]|\\.|\$(?!\{))*\`)`;
const CALLS = [new RegExp(String.raw`\b(?:t|msg)\(\s*${LIT}`, "g"), new RegExp(String.raw`\btn\([^,()]+(?:\([^()]*\))?,\s*${LIT}\s*,\s*${LIT}`, "g")];

function unquote(lit) {
  const q = lit[0];
  const body = lit.slice(1, -1);
  if (q === "`") return body.replace(/\\([`\\$])/g, "$1");
  if (q === "'") return JSON.parse(`"${body.replace(/\\'/g, "'").replace(/"/g, '\\"')}"`);
  return JSON.parse(lit);
}

const found = new Map();
for (const dir of SOURCES) {
  for (const file of files(join(root, dir))) {
    const src = readFileSync(file, "utf8");
    for (const re of CALLS) {
      for (const m of src.matchAll(re)) {
        for (const lit of m.slice(1).filter(Boolean)) {
          const text = unquote(lit);
          if (!found.has(text)) found.set(text, relative(root, file));
        }
      }
    }
  }
}
const list = [...found.keys()].sort((a, b) => a.localeCompare(b));
const json = `${JSON.stringify(list, null, 2)}\n`;
const args = process.argv.slice(2);

if (args[0] === "--check") {
  let current = "";
  try {
    current = readFileSync(OUT, "utf8");
  } catch {}
  if (current !== json) {
    console.error("locales/source.json is out of date: run `npm run messages`.");
    process.exit(1);
  }
  console.log(`locales/source.json is up to date (${list.length} messages).`);
} else if (args[0] === "--pack") {
  const pack = JSON.parse(readFileSync(args[1], "utf8"));
  const messages = pack.messages ?? pack;
  const missing = list.filter((k) => !(k in messages));
  const unknown = Object.keys(messages).filter((k) => !found.has(k));
  for (const k of missing) console.log(`missing  ${JSON.stringify(k)}`);
  for (const k of unknown) console.log(`unknown  ${JSON.stringify(k)}`);
  console.log(`${list.length - missing.length} of ${list.length} translated, ${unknown.length} unknown.`);
  process.exit(unknown.length ? 1 : 0);
} else {
  writeFileSync(OUT, json);
  console.log(`Wrote ${list.length} messages to locales/source.json.`);
}
