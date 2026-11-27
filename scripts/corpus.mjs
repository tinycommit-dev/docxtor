#!/usr/bin/env node
// Fetches the public .docx test corpora Docxtor is checked against.
//
//   npm run corpus:fetch
//
// The files go to $DOCXTOR_CORPUS_DIR (default ~/.cache/docxtor-corpus), never into the repository: they are other
// people's files (Apache-2.0 and MPL-2.0), untrusted input, and they must not feed the fidelity gate on `fixtures/`.
// Idempotent: when everything is already there it does nothing.
//
// Sources are pinned to a commit, so the corpus is the same on every machine:
//   poi          apache/poi              test-data/document/*.docx
//   libreoffice  LibreOffice/core        every sw/qa/**/*.docx (only those blobs are downloaded)
//
// Layout of the cache folder:
//   <dir>/poi/<path inside the repository>
//   <dir>/libreoffice/<path inside the repository>
//   <dir>/manifest.json     the sources (repo, commit, licence, file count, bytes) and a pinned sample of 100 files

import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const SOURCES = [
  {
    name: "poi",
    repo: "apache/poi",
    commit: "f66927954a29cf1f5b86614fdf1816e6321c8aea",
    licence: "Apache-2.0",
    patterns: ["/test-data/document/*.docx"],
    // Roughly 130 files, 7 MB.
    maxDownloadMB: 200,
  },
  {
    name: "libreoffice",
    repo: "LibreOffice/core",
    commit: "53b6fdd626750ee50f8b5abc50552f9fc7008c7a",
    licence: "MPL-2.0",
    patterns: ["/sw/qa/**/*.docx"],
    // Roughly 1,500 files, 30 MB. The trees of the commit are most of the download; stop if it is far more.
    maxDownloadMB: 600,
  },
];

export const SAMPLE_SIZE = 100;

/**
 * Files of the fetched corpus that the Word checks always run on, besides the sample of 100, because known gaps
 * rest on them: the 7 files saved as Strict Open XML (their measures carry units, which the validator names in the original
 * too) and two where updating a table of contents gives other entries than the file has. Paths in the cache folder.
 */
export const EXTRA = [
  "libreoffice/sw/qa/extras/ooxmlimport/data/strict-lockedcanvas.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/strict.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/tdf79272_strictDxa.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/strict-smartart.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/tdf116410.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/tdf82065_Ind_start_strict.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/tdf150822.docx",
  "libreoffice/sw/qa/core/layout/data/floattable-from-bottom.docx",
  "libreoffice/sw/qa/extras/ooxmlexport/data/tdf32363.docx",
];

/**
 * The docxtor command the Word checks run: DOCXTOR_BIN, else the newest of target/release and target/gate; built again (profile
 * gate: release without link-time optimisation) when it is older than the engine's or the command's source, so a check never
 * judges an old build.
 */
export function findDocxtor(root, tag) {
  if (process.env.DOCXTOR_BIN) return process.env.DOCXTOR_BIN;
  const newest = (dir) => {
    let t = 0;
    for (const e of fs.existsSync(dir) ? fs.readdirSync(dir, { withFileTypes: true }) : []) {
      const p = path.join(dir, e.name);
      t = Math.max(t, e.isDirectory() ? newest(p) : fs.statSync(p).mtimeMs);
    }
    return t;
  };
  const sources = Math.max(newest(path.join(root, "crates/docxtor-core/src")), newest(path.join(root, "crates/docxtor-cli/src")), fs.statSync(path.join(root, "Cargo.toml")).mtimeMs);
  const built = ["release", "gate"].map((p) => path.join(root, "target", p, "docxtor")).filter((f) => fs.existsSync(f)).sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
  if (built.length && fs.statSync(built[0]).mtimeMs >= sources) return built[0];
  console.log(`${tag}: building the docxtor command (profile gate)${built.length ? ", the one built is older than the source" : ""}`);
  execFileSync("cargo", ["build", "--profile", "gate", "-q", "-p", "docxtor-cli"], { cwd: root, stdio: "inherit" });
  return path.join(root, "target/gate/docxtor");
}

/** The files a run takes: the sample, then the extras that are in the cache and not in the sample already. */
export function withExtras(sample, dir) {
  const have = new Set(sample);
  return [...sample, ...EXTRA.filter((r) => !have.has(r) && fs.existsSync(path.join(dir, r)))];
}

export function corpusDir() {
  return process.env.DOCXTOR_CORPUS_DIR || path.join(os.homedir(), ".cache", "docxtor-corpus");
}

function dirBytes(dir) {
  let total = 0;
  const walk = (d) => {
    let entries = [];
    try {
      entries = fs.readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.isFile()) total += fs.statSync(p).size;
    }
  };
  walk(dir);
  return total;
}

function listDocx(dir) {
  const out = [];
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.isFile() && e.name.toLowerCase().endsWith(".docx")) out.push(p);
    }
  };
  if (fs.existsSync(dir)) walk(dir);
  return out.sort();
}

function git(cwd, args) {
  return execFileSync("git", args, { cwd, stdio: ["ignore", "pipe", "pipe"], maxBuffer: 1 << 26 }).toString();
}

/** Runs a git command and stops it when the repository it fills grows past a limit. */
function gitGuarded(cwd, args, watchDir, maxMB) {
  return new Promise((resolve, reject) => {
    const child = spawn("git", args, { cwd, stdio: ["ignore", "ignore", "pipe"] });
    let err = "";
    child.stderr.on("data", (d) => (err += d));
    const timer = setInterval(() => {
      const mb = dirBytes(watchDir) / 1048576;
      if (mb > maxMB) {
        child.kill("SIGKILL");
        clearInterval(timer);
        reject(new Error(`download passed ${maxMB} MB (${Math.round(mb)} MB so far); stopped`));
      }
    }, 2000);
    child.on("exit", (code) => {
      clearInterval(timer);
      if (code === 0) resolve();
      else reject(new Error(`git ${args.join(" ")} failed: ${err.trim().split("\n").slice(-3).join(" | ")}`));
    });
  });
}

function sourceDone(dir, manifest, src) {
  const m = manifest?.sources?.find((s) => s.name === src.name);
  if (!m || m.commit !== src.commit) return false;
  return listDocx(path.join(dir, src.name)).length === m.files;
}

async function fetchSource(dir, src) {
  const work = path.join(dir, `.work-${src.name}`);
  fs.rmSync(work, { recursive: true, force: true });
  fs.mkdirSync(work, { recursive: true });
  const url = `https://github.com/${src.repo}.git`;
  console.log(`corpus: ${src.name}: fetching ${src.repo} @ ${src.commit.slice(0, 10)} (only the .docx files)`);
  try {
    git(work, ["init", "-q"]);
    git(work, ["remote", "add", "origin", url]);
    git(work, ["config", "core.sparseCheckout", "true"]);
    git(work, ["sparse-checkout", "init", "--no-cone"]);
    git(work, ["sparse-checkout", "set", "--no-cone", ...src.patterns]);
    await gitGuarded(work, ["fetch", "-q", "--depth", "1", "--filter=blob:none", "origin", src.commit], path.join(work, ".git"), src.maxDownloadMB);
    // The checkout downloads the blobs of the matching files, in one batch.
    await gitGuarded(work, ["checkout", "-q", "--detach", "FETCH_HEAD"], path.join(work, ".git"), src.maxDownloadMB);
    const head = git(work, ["rev-parse", "HEAD"]).trim();
    if (head !== src.commit) throw new Error(`got commit ${head}, wanted ${src.commit}`);
    const target = path.join(dir, src.name);
    fs.rmSync(target, { recursive: true, force: true });
    fs.mkdirSync(target, { recursive: true });
    for (const f of listDocx(work)) {
      if (f.includes(`${path.sep}.git${path.sep}`)) continue;
      const rel = path.relative(work, f);
      fs.mkdirSync(path.dirname(path.join(target, rel)), { recursive: true });
      fs.copyFileSync(f, path.join(target, rel));
    }
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

function sha256(s) {
  return createHash("sha256").update(s).digest("hex");
}

export function buildManifest(dir) {
  const sources = [];
  const all = [];
  for (const src of SOURCES) {
    const files = listDocx(path.join(dir, src.name));
    const bytes = files.reduce((n, f) => n + fs.statSync(f).size, 0);
    sources.push({ name: src.name, repo: src.repo, commit: src.commit, licence: src.licence, files: files.length, bytes });
    for (const f of files) all.push(`${src.name}/${path.relative(path.join(dir, src.name), f).split(path.sep).join("/")}`);
  }
  // Deterministic: the first 100 by the hash of the path, which mixes the sources in proportion to their size.
  const sample = all
    .map((p) => [sha256(p), p])
    .sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0))
    .slice(0, SAMPLE_SIZE)
    .map((x) => x[1])
    .sort();
  return { fetched: new Date().toISOString(), sources, sample_rule: `first ${SAMPLE_SIZE} by sha256 of "<source>/<path>"`, sample };
}

async function main() {
  const dir = corpusDir();
  fs.mkdirSync(dir, { recursive: true });
  const manifestPath = path.join(dir, "manifest.json");
  let manifest = null;
  try {
    manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  } catch {}
  let did = false;
  for (const src of SOURCES) {
    if (sourceDone(dir, manifest, src)) {
      console.log(`corpus: ${src.name}: already there`);
      continue;
    }
    await fetchSource(dir, src);
    did = true;
  }
  if (did || !manifest) {
    const m = buildManifest(dir);
    fs.writeFileSync(manifestPath, JSON.stringify(m, null, 2) + "\n");
    manifest = m;
  }
  for (const s of manifest.sources) console.log(`corpus: ${s.name}: ${s.files} files, ${(s.bytes / 1048576).toFixed(1)} MB, ${s.licence}`);
  console.log(`corpus: ${manifest.sample.length} files in the sample; folder ${dir}`);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((e) => {
    console.error(`corpus: ${e.message}`);
    process.exit(1);
  });
}
