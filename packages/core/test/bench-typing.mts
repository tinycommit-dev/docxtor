// Not a test: times the engine on what the editor does most. `node --experimental-strip-types packages/core/test/bench-typing.mts [path/to/docxtor_bg.wasm]`
import { readFile } from "node:fs/promises";
import { Engine, loadEngine } from "../src/index.ts";

const wasmPath = process.argv[2] ?? new URL("../wasm/docxtor_bg.wasm", import.meta.url).pathname;
await loadEngine(await readFile(wasmPath));
const ms = () => performance.now();

// A long document: 2,000 paragraphs with some formatting and a list.
let t = ms();
const md = Array.from({ length: 2000 }, (_, i) => (i % 50 === 0 ? `## Heading ${i}` : i % 7 === 0 ? `- item **${i}** of the list` : `Paragraph ${i} with *some* words to lay out and a little more text.`)).join("\n\n");
const e = Engine.blank();
const first = e.renderAll().order[0];
e.apply({ op: "insert_markdown", before: first, markdown: md } as never);
const bytes = e.save();
const build = ms() - t;

t = ms();
const doc = Engine.open(bytes, "long.docx");
doc.renderAll();
const open = ms() - t;

// Type 300 characters into a paragraph in the middle, as the editor does: one op, then the changed render.
const mid = doc.renderAll().order[1000];
t = ms();
for (let i = 0; i < 300; i++) {
  doc.apply({ op: "insert_text", at: { p: mid, o: i }, text: "x" } as never, true);
  doc.renderChanged();
}
const typing = (ms() - t) / 300;

t = ms();
doc.save();
const save = ms() - t;
console.log(JSON.stringify({ wasm: wasmPath.split("/").slice(-2).join("/"), build: Math.round(build), open: Math.round(open), typing: +typing.toFixed(2), save: Math.round(save) }));
