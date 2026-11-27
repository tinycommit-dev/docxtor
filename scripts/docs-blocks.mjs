// Finds the code blocks in the documents a first visitor reads, and the marker each one must carry.
//
// A fenced block is preceded by one line:
//   <!-- example: name -->   a program or commands that a test runs (scripts/docs-examples.test.mjs, or
//                            e2e/docs-examples.spec.ts for the ones that need a browser: BROWSER_EXAMPLES)
//   *Sketch, not run by a test: why.*   not run, and the reason, in a visible line (it needs an account, a key, or someone's own files)
//   <!-- text -->            not a program: output, a diagram, a tree of folders
// scripts/docs-examples.test.mjs fails on a block with no marker, on a name two blocks share, and on an example
// that nothing runs.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

// Both `npm test` and Playwright run from the repository root. (No import.meta here: Playwright loads this file too.)
export const ROOT = process.cwd();
/** A section of the docs: its overview (index.md) first, then the pages beside it in the folder. */
const section = (dir) => [`${dir}/index.md`, ...readdirSync(join(process.cwd(), dir)).filter((f) => f.endsWith(".md") && f !== "index.md").sort().map((f) => `${dir}/${f}`)];
/** The pages of the AI guide (docs/ai/). The tools tables and the CLI examples are on them. */
export const AI_FILES = section("docs/ai");
/** Every Markdown file under a folder of the repository (hidden folders and the site's public files are not pages), as paths from the root. */
const walk = (dir) =>
  readdirSync(join(process.cwd(), dir), { withFileTypes: true })
    .sort((a, b) => (a.name < b.name ? -1 : 1))
    .flatMap((e) => (e.name.startsWith(".") || e.name === "public" ? [] : e.isDirectory() ? walk(`${dir}/${e.name}`) : e.name.endsWith(".md") ? [`${dir}/${e.name}`] : []));
/** Every Markdown file with code blocks or links the tests read: the root files people read first, and every page of docs/ (found by walking it, so a new page is tested without being listed). */
export const DOC_FILES = ["README.md", "CONTRIBUTING.md", "PRIVACY.md", "SECURITY.md", ...walk("docs")];

/** The examples that need a browser: e2e/docs-examples.spec.ts runs each one by name. */
export const BROWSER_EXAMPLES = ["embed-element", "embed-component", "embed-spelling", "embed-password", "embed-theming", "extensions-use"];

const MARKER = /^<!--\s*(example|text)(?::\s*(.*?))?\s*-->$/;
const SKETCH = /^\*Sketch, not run by a test: (.*?)\.?\*$/;

/** Every fenced block of the documents: { file, line, lang, code, kind, label } (kind is undefined when the marker is missing). */
export function blocks(files = DOC_FILES) {
  const found = [];
  for (const file of files) {
    const lines = readFileSync(join(ROOT, file), "utf8").split("\n");
    for (let i = 0; i < lines.length; i++) {
      const open = /^```(\S*)\s*$/.exec(lines[i]);
      if (!open) continue;
      let j = i - 1;
      while (j >= 0 && lines[j].trim() === "") j--;
      const text = j >= 0 ? lines[j].trim() : "";
      const marker = MARKER.exec(text) ?? (SKETCH.test(text) ? ["", "sketch", SKETCH.exec(text)[1]] : null);
      let end = i + 1;
      while (end < lines.length && !/^```\s*$/.test(lines[end])) end++;
      found.push({ file, line: i + 1, lang: open[1], code: lines.slice(i + 1, end).join("\n"), kind: marker?.[1], label: marker?.[2] ?? "" });
      i = end;
    }
  }
  return found;
}

/** The code of the example with this name (the test fails when there is none, or more than one). */
export function example(name) {
  const hits = blocks().filter((b) => b.kind === "example" && b.label === name);
  if (hits.length !== 1) throw new Error(`Expected one block marked "example: ${name}", found ${hits.length}`);
  return hits[0];
}
