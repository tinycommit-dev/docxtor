#!/usr/bin/env node
// Runs Microsoft's Open XML SDK validator (tools/ooxml-validate) on the corpus and on what Docxtor writes
// A gate: it exits 1 when Docxtor adds an error that is not in
// scripts/validate-allow.json.
//
//   npm run corpus:validate          the pinned sample of 100 files, Docxtor's own files and the feature files
//   DOCXTOR_CORPUS=all npm run corpus:validate
//
// The rule: an error in Docxtor's output fails the run unless (a) the original file has the same error (id + part +
// description, counted: the path moves when a paragraph is added), or (b) the allow-list names it, with the reason and
// the date. Files Docxtor makes from nothing (fixtures, `docxtor new`, the feature files) have no original, so every
// error in them counts. A file Docxtor cannot open also fails, unless the allow-list names the file.
//
// The feature files are one saved file per op kind and per recorded shape plus a few built documents
// (crates/docxtor-core/tests/common/features.rs). DOCXTOR_FEATURES_DIR=<dir> uses a folder already made; otherwise they are
// made with cargo test (profile gate) into a temporary folder. DOCXTOR_VALIDATE_SETS=corpus,own,features picks the sets.
//
// For each corpus file it validates the original, Docxtor's save after a forced re-write of the body
// (`docxtor roundtrip`), and Docxtor's save after a few edits (text inserted and formatted in the first paragraph, a new
// paragraph). What matters is the errors in Docxtor's output that are NOT in the original: they are counted per error
// id, matched on id + part + description (the path moves when a paragraph is added). It also validates Docxtor's own
// fixtures and files made from scratch (`docxtor new`): there every error is Docxtor's.
//
// Needs dotnet (SDK 7 here, 8 in CI: the program is built for net7.0 and rolls forward) and the docxtor binary (built with cargo when it is missing). The work files go to a
// temporary folder, never into the repository or the corpus cache.

import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { corpusDir, findDocxtor, withExtras } from "./corpus.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dotnetRoot = process.env.DOTNET_ROOT || (fs.existsSync("/usr/local/share/dotnet") ? "/usr/local/share/dotnet" : null);
const env = { ...process.env, ...(dotnetRoot ? { DOTNET_ROOT: dotnetRoot, PATH: `${dotnetRoot}:${process.env.PATH}` } : {}), DOTNET_CLI_TELEMETRY_OPTOUT: "1", DOTNET_NOLOGO: "1" };
const sets = new Set((process.env.DOCXTOR_VALIDATE_SETS || "corpus,own,features").split(","));

function have(cmd, args = ["--version"]) {
  const r = spawnSync(cmd, args, { env, stdio: "ignore" });
  return r.status === 0;
}

if (!have("dotnet")) {
  // A gate that silently passes is no gate: CI sets DOCXTOR_REQUIRE=1.
  console.log("validate: " + (process.env.DOCXTOR_REQUIRE ? "FAILED, no dotnet" : "skipped (no dotnet)"));
  process.exit(process.env.DOCXTOR_REQUIRE ? 1 : 0);
}

const tool = path.join(root, "tools/ooxml-validate");
const dll = path.join(tool, "bin/Release/net7.0/ooxml-validate.dll");
if (!fs.existsSync(dll) || fs.statSync(dll).mtimeMs < fs.statSync(path.join(tool, "ooxml-validate.csproj")).mtimeMs) {
  console.log("validate: building the validator (first run downloads DocumentFormat.OpenXml)");
  execFileSync("dotnet", ["build", "-c", "Release", "-v", "q", tool], { env, stdio: "inherit" });
}

const docxtor = findDocxtor(root, "validate");

const work = fs.mkdtempSync(path.join(os.tmpdir(), "docxtor-validate-"));
const run = (args, opts = {}) => spawnSync(docxtor, args, { encoding: "utf8", maxBuffer: 1 << 28, timeout: 120000, ...opts });

// ---- the files ---------------------------------------------------------------------------------------------------

const dir = corpusDir();
let rels = [];
const manifestPath = path.join(dir, "manifest.json");
if (!sets.has("corpus")) {
  // not asked for
} else if (fs.existsSync(manifestPath)) {
  const m = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (process.env.DOCXTOR_CORPUS === "all") {
    const walk = (d, out) => {
      for (const e of fs.existsSync(d) ? fs.readdirSync(d, { withFileTypes: true }) : []) {
        const p = path.join(d, e.name);
        if (e.isDirectory()) walk(p, out);
        else if (e.name.toLowerCase().endsWith(".docx")) out.push(path.relative(dir, p));
      }
    };
    for (const s of m.sources) walk(path.join(dir, s.name), rels);
    rels.sort();
  } else rels = withExtras(m.sample, dir);
} else {
  if (process.env.DOCXTOR_REQUIRE) {
    console.log(`validate: FAILED, no corpus at ${dir} (run npm run corpus:fetch)`);
    process.exit(1);
  }
  console.log(`validate: no corpus at ${dir} (run npm run corpus:fetch); checking Docxtor's own files only`);
}
if (process.env.DOCXTOR_CORPUS_ONLY) rels = rels.filter((r) => r.includes(process.env.DOCXTOR_CORPUS_ONLY));
if (sets.has("corpus") && rels.length === 0 && fs.existsSync(manifestPath)) {
  console.log("validate: FAILED, no corpus files to check (DOCXTOR_CORPUS_ONLY matches nothing?)");
  process.exit(1);
}

// Files Docxtor makes itself.
const own = [];
for (const f of !sets.has("own") ? [] : fs.readdirSync(path.join(root, "fixtures")).filter((f) => f.endsWith(".docx")).sort()) own.push({ name: `fixtures/${f}`, path: path.join(root, "fixtures", f) });
const md = "# A title\n\nA paragraph with **bold**, *italic* and a [link](https://example.com).\n\n## A list\n\n- one\n- two\n  - nested\n\n1. first\n2. second\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n> A quotation\n\nA note.[^1]\n\n[^1]: The note.\n";
for (const [name, args] of sets.has("own") ? [["new: blank", []], ["new: from markdown", ["--markdown", "-"]]] : []) {
  const out = path.join(work, name.replace(/\W+/g, "_") + ".docx");
  const r = run(["new", out, ...args], args.length ? { input: md } : {});
  if (r.status === 0) own.push({ name, path: out });
  else console.log(`validate: docxtor new failed (${name}): ${r.stderr}`);
}

// The feature files: one saved file per op kind, per recorded shape, and a few built documents.
const featuresInfo = { files: 0, kinds: 0, uncovered: [] };
if (sets.has("features")) {
  let fdir = process.env.DOCXTOR_FEATURES_DIR;
  if (!fdir) {
    fdir = path.join(work, "features");
    console.log("validate: making the feature files (cargo test --profile gate --test random_ops feature_files)");
    const r = spawnSync("cargo", ["test", "--profile", "gate", "-q", "-p", "docxtor-core", "--test", "random_ops", "feature_files", "--", "--nocapture"], { cwd: root, encoding: "utf8", env: { ...process.env, DOCXTOR_FEATURES_OUT: fdir }, maxBuffer: 1 << 28, timeout: 1500000 });
    if (r.status !== 0) {
      console.error("validate: FAILED, the feature files could not be made:\n" + (r.stdout || "").split("\n").slice(-30).join("\n") + (r.stderr || "").split("\n").slice(-30).join("\n"));
      process.exit(1);
    }
  }
  const index = JSON.parse(fs.readFileSync(path.join(fdir, "index.json"), "utf8"));
  featuresInfo.kinds = Object.keys(index.covered).length;
  featuresInfo.uncovered = index.uncovered;
  for (const rel of Object.keys(index.files).sort()) own.push({ name: `features/${rel}`, path: path.join(fdir, rel), feature: true });
  featuresInfo.files = Object.keys(index.files).length;
}

// ---- making Docxtor's versions -------------------------------------------------------------------------------------

function firstParagraph(file) {
  const r = run(["json", file]);
  if (r.status !== 0) return null;
  try {
    const v = JSON.parse(r.stdout);
    const doc = v.doc || v;
    const b = (doc.blocks || []).find((x) => x.t === "p");
    return b ? b.id : null;
  } catch {
    return null;
  }
}

const jobs = []; // {name, original, roundtrip?, edited?, skipped?}
rels.forEach((rel, i) => {
  const orig = path.join(dir, rel);
  const job = { name: rel, original: orig };
  const rt = path.join(work, `rt-${i}.docx`);
  const r = run(["roundtrip", orig, rt]);
  if (r.status === 0 && fs.existsSync(rt)) job.roundtrip = rt;
  else job.skipped = (r.stderr || r.stdout || "").trim().split("\n")[0].slice(0, 120) || "docxtor could not open it";
  if (job.roundtrip) {
    const ed = path.join(work, `ed-${i}.docx`);
    const id = firstParagraph(orig);
    if (id !== null && id !== undefined) {
      const ops = [
        { op: "insert_text", at: { p: id, o: 0 }, text: "Docxtor check. " },
        { op: "format_text", from: { p: id, o: 0 }, to: { p: id, o: 5 }, format: { bold: true } },
        { op: "insert_paragraph", after: id, text: "A paragraph added by the check." },
      ];
      const r2 = run(["apply", orig, "-", "-o", ed], { input: JSON.stringify(ops) });
      if (r2.status === 0 && fs.existsSync(ed)) job.edited = ed;
      else job.editRefused = (r2.stderr || r2.stdout || "").trim().split("\n")[0].slice(0, 120);
    }
  }
  jobs.push(job);
});
for (const f of own) jobs.push({ name: f.name, original: f.path, own: true });

// ---- validating --------------------------------------------------------------------------------------------------

const toValidate = [];
for (const j of jobs) for (const k of ["original", "roundtrip", "edited"]) if (j[k]) toValidate.push(j[k]);
const results = new Map();
const CHUNK = 40;
for (let i = 0; i < toValidate.length; i += CHUNK) {
  const chunk = toValidate.slice(i, i + CHUNK);
  const r = spawnSync("dotnet", [dll, ...chunk], { env, encoding: "utf8", maxBuffer: 1 << 29, timeout: 600000 });
  if (r.status !== 0) {
    console.error(`validate: the validator failed: ${r.stderr}`);
    process.exit(1);
  }
  for (const line of r.stdout.split("\n")) {
    if (!line.startsWith("{")) continue;
    const v = JSON.parse(line);
    results.set(v.file, v.errors);
  }
  if ((i / CHUNK) % 5 === 4) process.stderr.write(`validate: ${Math.min(i + CHUNK, toValidate.length)} of ${toValidate.length} files\n`);
}

const key = (e) => `${e.id}|${e.part}|${e.description}`;
function newErrors(before, after) {
  const have = new Map();
  for (const e of before) have.set(key(e), (have.get(key(e)) || 0) + 1);
  const out = [];
  for (const e of after) {
    const n = have.get(key(e)) || 0;
    if (n > 0) have.set(key(e), n - 1);
    else out.push(e);
  }
  return out;
}

const report = { files: 0, skipped: [], original_with_errors: 0, original_unreadable: [], roundtrip_new: [], edited_new: [], own: [] };
for (const j of jobs) {
  if (j.own) {
    const errs = results.get(j.original) || [];
    report.own.push({ file: j.name, errors: errs });
    continue;
  }
  report.files++;
  if (j.skipped) {
    report.skipped.push({ file: j.name, why: j.skipped });
    continue;
  }
  const orig = results.get(j.original) || [];
  if (orig.length) report.original_with_errors++;
  // The validator could not open the original at all (a part its relationships name is missing), so it has no list
  // of the original's errors to take away: everything it says about Docxtor's file would look new. Docxtor's re-write
  // opens, which is the finding; the rest cannot be compared.
  if (orig.some((e) => e.id === "Open")) {
    report.original_unreadable.push(j.name);
    continue;
  }
  const rt = newErrors(orig, results.get(j.roundtrip) || []);
  if (rt.length) report.roundtrip_new.push({ file: j.name, errors: rt });
  if (j.edited) {
    const ed = newErrors(orig, results.get(j.edited) || []);
    // What the edit added on top of the plain re-write.
    const extra = newErrors(results.get(j.roundtrip) || [], results.get(j.edited) || []);
    if (ed.length) report.edited_new.push({ file: j.name, errors: ed, onlyFromEdit: extra.length });
  }
}

function bucket(list) {
  const b = new Map();
  for (const f of list) {
    for (const e of f.errors) {
      const k = e.id;
      const x = b.get(k) || { id: k, errors: 0, files: new Set(), example: null };
      x.errors++;
      x.files.add(f.file);
      if (!x.example) x.example = { file: f.file, part: e.part, path: e.path, description: e.description };
      b.set(k, x);
    }
  }
  return [...b.values()].map((x) => ({ id: x.id, errors: x.errors, files: x.files.size, example: x.example })).sort((a, b) => b.files - a.files || b.errors - a.errors);
}

report.buckets = { roundtrip: bucket(report.roundtrip_new), edited: bucket(report.edited_new), own: bucket(report.own.filter((o) => o.errors.length)) };

// ---- the gate ------------------------------------------------------------------------------------------------------

const allowPath = process.env.DOCXTOR_VALIDATE_ALLOW || path.join(root, "scripts/validate-allow.json");
const allow = JSON.parse(fs.readFileSync(allowPath, "utf8"));
// An exception without its reason and date is not an explanation.
for (const a of [...(allow.errors || []), ...(allow.skipped || [])]) {
  if (!a.reason || !/^\d{4}-\d{2}-\d{2}$/.test(a.date || "")) {
    console.log(`validate: FAILED, an entry of ${path.relative(root, allowPath)} has no reason or no date (YYYY-MM-DD): ${JSON.stringify(a)}`);
    process.exit(1);
  }
}
// An entry names an error by id and, when given, part and description (exact) or a text the description holds.
const entryFor = (e, area) => (allow.errors || []).find((a) => a.id === e.id && (!a.part || a.part === e.part) && (!a.description || a.description === e.description) && (!a.contains || e.description.includes(a.contains)) && (!a.in || a.in === area));
const used = new Set();
const failures = []; // {area, file, error}
const unallowed = (area, file, errors) => {
  for (const e of errors) {
    const a = entryFor(e, area);
    if (a) used.add(a);
    else failures.push({ area, file, error: e });
  }
};
for (const f of report.roundtrip_new) unallowed("corpus", `${f.file} (re-write)`, f.errors);
for (const f of report.edited_new) unallowed("corpus", `${f.file} (after edits)`, f.errors);
for (const o of report.own) unallowed(o.file.startsWith("features/") ? "features" : "own", o.file, o.errors);
const skippedFailures = report.skipped.filter((s) => !(allow.skipped || []).some((a) => a.file === s.file));
// Files the validator cannot open as originals (a part their relationships name is missing; fuzzer output): there is
// no list of the original's errors to take away, so they are named and not judged.

const out = process.env.DOCXTOR_VALIDATE_OUT || path.join(dir, "validate.json");
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify({ ...report, failures, features: featuresInfo }, null, 2));

const show = (title, list) => {
  console.log(`\n${title}: ${list.length} error ids`);
  for (const b of list) console.log(`  ${String(b.files).padStart(4)} files ${String(b.errors).padStart(5)} errors  ${b.id}\n        e.g. ${b.example.file}\n        ${b.example.part} ${b.example.path}\n        ${b.example.description.slice(0, 200)}`);
};
console.log(`validate: ${report.files} corpus files; ${report.skipped.length} Docxtor could not open; ${report.original_with_errors} originals already have errors`);
console.log(`validate: ${report.original_unreadable.length} originals the validator cannot open (not compared): ${report.original_unreadable.map((f) => path.basename(f)).join(", ")}`);
console.log(`validate: errors Docxtor's re-write adds: ${report.roundtrip_new.length} files; after edits: ${report.edited_new.length} files`);
show("errors a re-write adds (roundtrip)", report.buckets.roundtrip);
show("errors edits add", report.buckets.edited);
const ownFiles = report.own.filter((o) => !o.file.startsWith("features/"));
const featFiles = report.own.filter((o) => o.file.startsWith("features/"));
console.log(`\nDocxtor's own files (fixtures, docxtor new): ${ownFiles.length}, ${ownFiles.filter((o) => o.errors.length).length} with errors`);
console.log(`feature files (one per op kind and per recorded shape, and built documents): ${featFiles.length}, ${featFiles.filter((o) => o.errors.length).length} with errors; ${featuresInfo.kinds} op kinds covered${featuresInfo.uncovered.length ? ", not covered: " + featuresInfo.uncovered.join(", ") : ""}`);
show("errors in Docxtor's own files and feature files (all are Docxtor's)", report.buckets.own);
console.log(`\nvalidate: details in ${out}`);
fs.rmSync(work, { recursive: true, force: true });

const unusedAllow = (allow.errors || []).filter((a) => !used.has(a));
if (unusedAllow.length && !process.env.DOCXTOR_VALIDATE_SETS) console.log(`validate: note: ${unusedAllow.length} allow-list entries matched nothing in this run: ${unusedAllow.map((a) => a.id).join(", ")}`);

if (failures.length || skippedFailures.length) {
  console.log(`\nvalidate: FAILED. ${failures.length} errors Docxtor adds are not in ${path.relative(root, allowPath)}:`);
  const seen = new Map();
  for (const f of failures) {
    const k = `${f.error.id}|${f.error.part}|${f.error.description}`;
    const x = seen.get(k) || { ...f, n: 0, files: new Set() };
    x.n++;
    x.files.add(f.file);
    seen.set(k, x);
  }
  for (const x of seen.values()) console.log(`  ${x.error.id} in ${x.area}, ${x.n} times in ${x.files.size} files, e.g. ${x.file}\n      ${x.error.part} ${x.error.path}\n      ${x.error.description.slice(0, 240)}`);
  for (const s of skippedFailures) console.log(`  Docxtor could not open ${s.file}: ${s.why}`);
  console.log("validate: fix the cause, or add the error to the allow-list with a reason and a date (one entry per id, part and description).");
  process.exit(1);
}
console.log("validate: passed");
