// Fails when a tracked file holds an absolute home path (/Users/<name>/ or /home/<name>/): a private path that should be
// repo-relative or `~`. Run by CI (`npm run check:paths`). Sample paths in test data are allowed: Windows paths
// (C:/Users/...) and the made-up names below.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const MADE_UP = new Set(["me", "x", "ana", "name", "user", "username", "you"]);
const PATH = /(?<![A-Za-z]:)(?<![A-Za-z0-9_.-])\/(?:Users|home)\/([A-Za-z0-9._-]+)\//g;

const files = execFileSync("git", ["ls-files", "-z"], { cwd: root, encoding: "utf8", maxBuffer: 1 << 28 }).split("\0").filter(Boolean);
const found = [];
for (const file of files) {
  let text;
  try { text = readFileSync(resolve(root, file), "utf8"); } catch { continue; }
  if (text.includes("\0")) continue; // binary
  text.split("\n").forEach((line, i) => {
    for (const m of line.matchAll(PATH)) if (!MADE_UP.has(m[1])) found.push(`${file}:${i + 1}: ${m[0]}`);
  });
}
if (found.length) {
  console.error("Absolute home paths in tracked files (use a repo-relative path or ~):\n" + found.join("\n"));
  process.exit(1);
}
console.log(`No absolute home paths in ${files.length} tracked files.`);
