#!/usr/bin/env node
// Converts corpus files, and what Docxtor saves from them, to PDF with LibreOffice and compares the two PDFs
// A gate: it exits 1 when a file breaks the rules below and
// scripts/lo-allow.json does not name it.
//
//   npm run corpus:lo            the pinned sample of 100 files, then the feature files
//   DOCXTOR_CORPUS=all npm run corpus:lo
//
// The rules, per file (a PDF from LibreOffice of the original and one of Docxtor's save):
//   1. Docxtor's save must convert when the original does.
//   2. The text of the two PDFs must be identical once whitespace is made one space.
//   3. The page count must be equal; for a document of more than 50 pages it may differ by 2 percent (rounded up).
//      The run does not call for it: in the sample of 100 files every file that converts has the same page count and
//      text, so zero is what the data supports. The 2 percent is a margin chosen in advance for the long files of a
//      bigger run (DOCXTOR_CORPUS=all), where LibreOffice's layout may move a page break with the smallest change;
//      if a file needs it, look at the file first, then widen the rule or name the file in lo-allow.json.
// The feature files (crates/docxtor-core/tests/common/features.rs: one saved file per op kind, and built documents)
// have no original: the file itself is the original, so the same rules say Docxtor reads back what it wrote; and the
// file must convert at all, make at least one page, and a built document must show the text its index.json lists.
// The files of the other recorded shapes (shapes/) are only converted when DOCXTOR_LO_SHAPES=1: they number hundreds.

// For each file: LibreOffice makes a PDF of the original and a PDF of Docxtor's save (`docxtor roundtrip`, which writes the
// whole body again; an untouched save is byte-identical and would prove nothing). Both go through the same converter
// with the same fonts, so a difference in page count or text means Docxtor changed something. A file that converts
// as an original and fails as Docxtor's save is a strong finding. LibreOffice forgives a lot that Word does not, so a
// pass is weak proof.
//
// Needs soffice (LibreOffice) and pdftotext (poppler); the docxtor binary is built with cargo when it is missing. Each worker
// has its own throwaway profile and every conversion has a time limit (60 s) that kills the process.
//
// Environment: DOCXTOR_LO_WORKERS (default 3), DOCXTOR_LO_TIMEOUT_S (60), DOCXTOR_LO_OUT (default <cache>/lo-check.json),
// DOCXTOR_CORPUS_ONLY (a text the path holds), DOCXTOR_BIN, DOCXTOR_CORPUS_DIR.

import { execFileSync, spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { corpusDir, findDocxtor, withExtras } from "./corpus.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function findSoffice() {
  for (const p of ["/Applications/LibreOffice.app/Contents/MacOS/soffice", "/opt/homebrew/bin/soffice", "/usr/local/bin/soffice", "/usr/bin/soffice"]) if (fs.existsSync(p)) return p;
  const r = spawnSync("which", ["soffice"], { encoding: "utf8" });
  return r.status === 0 ? r.stdout.trim() : null;
}
const soffice = findSoffice();
// A gate that silently passes is no gate: CI sets DOCXTOR_REQUIRE=1.
const required = !!process.env.DOCXTOR_REQUIRE;
const skip = (why) => {
  console.log(`lo-check: ${required ? "FAILED" : "skipped"} (${why})`);
  process.exit(required ? 1 : 0);
};
if (!soffice) skip("no LibreOffice");
if (spawnSync("pdftotext", ["-v"], { stdio: "ignore" }).error) skip("no pdftotext");

const docxtor = findDocxtor(root, "lo-check");

const dir = corpusDir();
const manifestPath = path.join(dir, "manifest.json");
const sets = new Set((process.env.DOCXTOR_LO_SETS || "corpus,features").split(","));
if (sets.has("corpus") && !fs.existsSync(manifestPath)) skip(`no files at ${dir}, run npm run corpus:fetch`);
const manifest = sets.has("corpus") ? JSON.parse(fs.readFileSync(manifestPath, "utf8")) : { sample: [], sources: [] };
let rels = withExtras(manifest.sample, dir);
if (process.env.DOCXTOR_CORPUS === "all") {
  rels = [];
  const walk = (d) => {
    for (const e of fs.existsSync(d) ? fs.readdirSync(d, { withFileTypes: true }) : []) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.toLowerCase().endsWith(".docx")) rels.push(path.relative(dir, p));
    }
  };
  for (const s of manifest.sources) walk(path.join(dir, s.name));
  rels.sort();
}
if (process.env.DOCXTOR_CORPUS_ONLY) rels = rels.filter((r) => r.includes(process.env.DOCXTOR_CORPUS_ONLY));

// The jobs: corpus files (area "corpus") and feature files (area "features").
const jobs = rels.map((rel) => ({ file: rel, orig: path.join(dir, rel), area: "corpus" }));
const featureDir = (() => {
  if (!sets.has("features")) return null;
  let f = process.env.DOCXTOR_FEATURES_DIR;
  if (!f) {
    f = fs.mkdtempSync(path.join(os.tmpdir(), "docxtor-lo-features-"));
    console.log("lo-check: making the feature files (cargo test --profile gate --test random_ops feature_files)");
    const r = spawnSync("cargo", ["test", "--profile", "gate", "-q", "-p", "docxtor-core", "--test", "random_ops", "feature_files", "--", "--nocapture"], { cwd: root, encoding: "utf8", env: { ...process.env, DOCXTOR_FEATURES_OUT: f }, maxBuffer: 1 << 28, timeout: 1500000 });
    if (r.status !== 0) {
      console.error("lo-check: FAILED, the feature files could not be made:\n" + (r.stdout || "").split("\n").slice(-30).join("\n"));
      process.exit(1);
    }
  }
  return f;
})();
if (featureDir) {
  const index = JSON.parse(fs.readFileSync(path.join(featureDir, "index.json"), "utf8"));
  for (const rel of Object.keys(index.files).sort()) {
    if (rel.startsWith("shapes/") && !process.env.DOCXTOR_LO_SHAPES) continue;
    if (process.env.DOCXTOR_CORPUS_ONLY && !rel.includes(process.env.DOCXTOR_CORPUS_ONLY)) continue;
    jobs.push({ file: `features/${rel}`, orig: path.join(featureDir, rel), area: "features", expect: index.files[rel].expect || [] });
  }
}
rels = jobs;
if (rels.length === 0) {
  console.log("lo-check: FAILED, no files to check (DOCXTOR_CORPUS_ONLY matches nothing?)");
  process.exit(1);
}

const WORKERS = Number(process.env.DOCXTOR_LO_WORKERS || 3);
const TIMEOUT_MS = Number(process.env.DOCXTOR_LO_TIMEOUT_S || 60) * 1000;
const work = fs.mkdtempSync(path.join(os.tmpdir(), "docxtor-lo-"));

/** One conversion with a time limit that kills the whole process group. */
function convert(profile, input, outDir) {
  return new Promise((resolve) => {
    const args = [`-env:UserInstallation=file://${profile}`, "--headless", "--norestore", "--nolockcheck", "--convert-to", "pdf", "--outdir", outDir, input];
    const child = spawn(soffice, args, { detached: true, stdio: ["ignore", "ignore", "pipe"] });
    let err = "";
    child.stderr.on("data", (d) => (err += d));
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch {}
    }, TIMEOUT_MS);
    child.on("exit", (code) => {
      clearTimeout(timer);
      // Anything the converter left running under this profile goes too.
      try {
        process.kill(-child.pid, "SIGKILL");
      } catch {}
      const pdf = path.join(outDir, path.basename(input).replace(/\.docx$/i, ".pdf"));
      if (timedOut) resolve({ ok: false, why: `no answer in ${TIMEOUT_MS / 1000} s`, killed: true });
      else if (code === 0 && fs.existsSync(pdf)) resolve({ ok: true, pdf });
      else resolve({ ok: false, why: `soffice ended with code ${code}${err ? ": " + err.trim().split("\n")[0].slice(0, 100) : ""}` });
    });
  });
}

/** Rule 3: equal up to 50 pages, then 2 percent of them, rounded up. */
function pageTolerance(pages) {
  return pages > 50 ? Math.ceil(pages * 0.02) : 0;
}

function pdfInfo(pdf) {
  const text = spawnSync("pdftotext", [pdf, "-"], { encoding: "utf8", maxBuffer: 1 << 28 });
  const info = spawnSync("pdfinfo", [pdf], { encoding: "utf8" });
  const m = /Pages:\s+(\d+)/.exec(info.stdout || "");
  const raw = text.stdout || "";
  const pages = m ? Number(m[1]) : (raw.match(/\f/g) || []).length;
  return { pages, text: raw.replace(/\s+/g, " ").trim() };
}

function excerpt(a, b) {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  const cut = (s) => s.slice(Math.max(0, i - 30), i + 60);
  return { at: i, original: cut(a), docxtor: cut(b) };
}

const results = [];
let next = 0;
let done = 0;

async function worker(id) {
  const base = path.join(work, `w${id}`);
  fs.mkdirSync(base, { recursive: true });
  let profile = path.join(base, "profile-0");
  let profiles = 0;
  const fresh = () => {
    profiles++;
    profile = path.join(base, `profile-${profiles}`);
  };
  while (true) {
    const i = next++;
    if (i >= rels.length) break;
    const job = rels[i];
    const rel = job.file;
    const orig = job.orig;
    const d = path.join(base, `f${i}`);
    fs.mkdirSync(d, { recursive: true });
    fs.copyFileSync(orig, path.join(d, "original.docx"));
    const row = { file: rel, area: job.area };
    // Docxtor's save.
    const r = spawnSync(docxtor, ["roundtrip", orig, path.join(d, "docxtor.docx")], { encoding: "utf8", timeout: 120000 });
    if (r.status !== 0 || !fs.existsSync(path.join(d, "docxtor.docx"))) {
      row.docxtor = { ok: false, why: "docxtor could not open or save it: " + ((r.stderr || r.stdout || "").trim().split("\n")[0] || "").slice(0, 100), notDocxtorConversion: true };
    }
    const a = await convert(profile, path.join(d, "original.docx"), d);
    if (a.killed) fresh();
    row.original = a.ok ? { ok: true } : { ok: false, why: a.why };
    let infoA = null;
    if (a.ok) infoA = pdfInfo(a.pdf);
    if (a.ok && job.expect?.length) {
      row.missing = job.expect.filter((t) => !infoA.text.includes(t.replace(/\s+/g, " ")));
      row.pagesOfOriginal = infoA.pages;
    }
    if (!row.docxtor) {
      const b = await convert(profile, path.join(d, "docxtor.docx"), d);
      if (b.killed) fresh();
      row.docxtor = b.ok ? { ok: true } : { ok: false, why: b.why };
      if (b.ok && infoA) {
        const infoB = pdfInfo(b.pdf);
        row.pages = [infoA.pages, infoB.pages];
        row.words = [infoA.text.split(" ").filter(Boolean).length, infoB.text.split(" ").filter(Boolean).length];
        row.samePages = Math.abs(infoA.pages - infoB.pages) <= pageTolerance(infoA.pages);
        row.pageTolerance = pageTolerance(infoA.pages);
        row.sameText = infoA.text === infoB.text;
        if (!row.sameText) row.diff = excerpt(infoA.text, infoB.text);
      }
    }
    row.pagesOfOriginal ??= infoA?.pages;
    row.kind = !row.original.ok && !row.docxtor.ok ? "both-fail" : row.original.ok && !row.docxtor.ok ? "docxtor-fails" : !row.original.ok && row.docxtor.ok ? "only-docxtor-converts" : row.sameText === false ? "text-differs" : row.samePages === false ? "pages-differ" : "same";
    results.push(row);
    fs.rmSync(d, { recursive: true, force: true });
    done++;
    if (done % 10 === 0) process.stderr.write(`lo-check: ${done} of ${rels.length} files\n`);
  }
}

const t0 = Date.now();
await Promise.all(Array.from({ length: WORKERS }, (_, i) => worker(i)));
results.sort((a, b) => (a.file < b.file ? -1 : 1));
const secs = Math.round((Date.now() - t0) / 1000);

const count = (k) => results.filter((r) => r.kind === k).length;
const summary = { files: results.length, seconds: secs, same: count("same"), pagesDiffer: count("pages-differ"), textDiffers: count("text-differs"), docxtorFails: count("docxtor-fails"), bothFail: count("both-fail"), onlyDocxtorConverts: count("only-docxtor-converts"), soffice: spawnSync(soffice, ["--version"], { encoding: "utf8" }).stdout.trim() };
const out = process.env.DOCXTOR_LO_OUT || path.join(dir, "lo-check.json");

// ---- the gate ------------------------------------------------------------------------------------------------------

const allowPath = process.env.DOCXTOR_LO_ALLOW || path.join(root, "scripts/lo-allow.json");
const allow = JSON.parse(fs.readFileSync(allowPath, "utf8"));
// An exception without its reason and date is not an explanation.
for (const a of allow.files || []) {
  if (!a.what || !a.reason || !/^\d{4}-\d{2}-\d{2}$/.test(a.date || "")) {
    console.log(`lo-check: FAILED, an entry of ${path.relative(root, allowPath)} needs file, what, reason and date (YYYY-MM-DD): ${JSON.stringify(a)}`);
    process.exit(1);
  }
}
const allowed = (r, what) => (allow.files || []).find((a) => a.file === r.file && a.what === what);
const failures = []; // {file, what, text}
const used = new Set();
const fail = (r, what, text) => {
  const a = allowed(r, what);
  if (a) used.add(a);
  else failures.push({ file: r.file, what, text });
};
for (const r of results) {
  if (r.area === "features" && !r.original.ok) fail(r, "feature-does-not-convert", `LibreOffice cannot convert the file Docxtor wrote: ${r.original.why}`);
  if (r.area === "features" && r.original.ok && !(r.pagesOfOriginal >= 1)) fail(r, "feature-no-pages", "the PDF has no page");
  if (r.missing?.length) fail(r, "feature-text-missing", `the PDF does not show: ${r.missing.map((t) => JSON.stringify(t)).join(", ")}`);
  if (r.kind === "docxtor-fails") fail(r, "docxtor-fails", `converts as the original, not as Docxtor's save: ${r.docxtor.why}`);
  if (r.kind === "text-differs") fail(r, "text-differs", `text differs after normalising whitespace (pages ${r.pages?.join(" -> ")}, words ${r.words?.join(" -> ")}): original ...${r.diff.original} / Docxtor ...${r.diff.docxtor}`);
  if (r.kind === "pages-differ") fail(r, "pages-differ", `page count ${r.pages.join(" -> ")} (allowed difference ${r.pageTolerance})`);
}
fs.writeFileSync(out, JSON.stringify({ summary, failures, files: results }, null, 2));

console.log(`\nlo-check: ${summary.files} files in ${secs} s with ${summary.soffice}`);
for (const area of ["corpus", "features"]) {
  const rs = results.filter((r) => r.area === area);
  if (rs.length) console.log(`  ${area}: ${rs.length} files, same pages and text ${rs.filter((r) => r.kind === "same").length}, text differs ${rs.filter((r) => r.kind === "text-differs").length}, page count differs ${rs.filter((r) => r.kind === "pages-differ").length}, Docxtor's save fails ${rs.filter((r) => r.kind === "docxtor-fails").length}, fails as the original too ${rs.filter((r) => r.kind === "both-fail").length}, only Docxtor's converts ${rs.filter((r) => r.kind === "only-docxtor-converts").length}`);
}
console.log(`lo-check: details in ${out}`);
fs.rmSync(work, { recursive: true, force: true });
if (featureDir && !process.env.DOCXTOR_FEATURES_DIR) fs.rmSync(featureDir, { recursive: true, force: true });
const unusedAllow = (allow.files || []).filter((a) => !used.has(a));
if (unusedAllow.length && !process.env.DOCXTOR_CORPUS_ONLY) console.log(`lo-check: note: ${unusedAllow.length} allow-list entries matched nothing in this run: ${unusedAllow.map((a) => a.file).join(", ")}`);
if (failures.length) {
  console.log(`\nlo-check: FAILED. ${failures.length} files break the rules and are not in ${path.relative(root, allowPath)}:`);
  for (const f of failures.slice(0, 60)) console.log(`  ${f.file}  [${f.what}]\n      ${f.text}`);
  if (failures.length > 60) console.log(`  ... and ${failures.length - 60} more in ${out}`);
  console.log('lo-check: fix the cause, or add {"file", "what", "reason", "date"} to the allow-list.');
  process.exit(1);
}
console.log("lo-check: passed");
