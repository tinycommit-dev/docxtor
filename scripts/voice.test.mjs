// The reader-voice guard: text in the repository is read by people outside the project, so it describes what the code is and why,
// and carries no internal voice (who decided, which tool wrote it, dated work logs, pointers to private notes).
// Scans every tracked text file and fails with file:line for each hit. Legitimate hits go in scripts/voice-allow.json with a reason.
// Run by `npm test`. `VOICE_LIST=1 node scripts/voice.test.mjs` prints every hit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");


const MONTHS = "(?:January|February|March|April|May|June|July|August|September|October|November|December)";

/** The internal voice, as patterns. */
const PATTERNS = [
  ["owner", /\bowner(?:'s|’s)?\b/i],
  ["model or agent name", /\b(?:Sonnet|Fable|Opus)\b|\bClaude Code\b/],
  ["decided", /\bdecided\b/i],
  ["not kept", /\bnot kept\b/i],
  ["private notes folder", /(?:^|[^\w.-])\/?notes\//],
  ["private document", /\bHANDOFF\b|\bREMAINING\b|\blong-documents plan\b/],
  ["dated review", /\breview of \d+ \w+ 20\d\d\b/],
  ["internal phase or step", /\bPhase \d|\bStep [A-D]\d?\b/],
  ["date with day", new RegExp(`\\b\\d{1,2} ${MONTHS} 20\\d\\d\\b`)],
  ["month and year", new RegExp(`\\b${MONTHS} 20\\d\\d\\b`)],
];

const allow = JSON.parse(readFileSync(join(root, "scripts/voice-allow.json"), "utf8"));

function isAllowed(file, text, label) {
  return allow.some(
    (a) => a.paths.includes(file) && (!a.labels || a.labels.includes(label)) && (!a.texts || a.texts.some((t) => text.includes(t))),
  );
}

function trackedTextFiles() {
  const out = execFileSync("git", ["ls-files", "-z"], { cwd: root, maxBuffer: 1 << 28 }).toString("utf8");
  return out
    .split("\0")
    .filter((f) => f && f !== "scripts/voice.test.mjs" && f !== "scripts/voice-allow.json");
}

/** Every hit as "file:line  label: text". */
export function findHits() {
  const hits = [];
  for (const file of trackedTextFiles()) {
    let buf;
    try {
      buf = readFileSync(join(root, file));
    } catch {
      continue;
    }
    if (buf.subarray(0, 8000).includes(0)) continue; // binary
    buf
      .toString("utf8")
      .split("\n")
      .forEach((line, i) => {
        if (line.length > 20000) return; // generated one-line files
        for (const [label, re] of PATTERNS) {
          if (re.test(line) && !isAllowed(file, line, label)) hits.push(`${file}:${i + 1}  ${label}: ${line.trim().slice(0, 160)}`);
        }
      });
  }
  return hits;
}

if (process.env.VOICE_LIST) {
  for (const h of findHits()) console.log(h);
  process.exit(0);
}

test("the allow-list is short and every entry says why", () => {
  assert.ok(allow.length <= 40, "keep the allow-list short: rewrite the text instead");
  for (const a of allow) assert.ok(Array.isArray(a.paths) && a.paths.length && a.reason, `entry without paths and a reason: ${JSON.stringify(a)}`);
  for (const a of allow) for (const f of a.paths) assert.ok(existsSync(join(root, f)), `${f} is in scripts/voice-allow.json and does not exist`);
});

test("no tracked file carries the internal voice", () => {
  const hits = findHits();
  assert.equal(hits.length, 0, `\n${hits.join("\n")}\n`);
});
