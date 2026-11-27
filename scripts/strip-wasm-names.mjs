// Removes the "name" section from WebAssembly files: the Rust function names the browser shows in a stack trace. In the engine it
// is 1.5 MB of 15.8 (243 KB of 3.03 MB brotli), downloaded by every first visit and used by nothing else; panics still say what
// went wrong and where, from their message. `DOCXTOR_WASM_NAMES=1` keeps the names, for reading a stack trace while debugging.
// node scripts/strip-wasm-names.mjs <file.wasm>...
import { readFileSync, writeFileSync } from "node:fs";

function leb(b, i) {
  let r = 0;
  let s = 0;
  for (;;) {
    const x = b[i++];
    r += (x & 0x7f) * 2 ** s;
    s += 7;
    if (x < 0x80) return [r, i];
  }
}

if (process.env.DOCXTOR_WASM_NAMES === "1") process.exit(0);
for (const file of process.argv.slice(2)) {
  const b = readFileSync(file);
  if (b.readUInt32LE(0) !== 0x6d736100) throw new Error(`${file} is not WebAssembly`);
  const keep = [b.subarray(0, 8)];
  let removed = 0;
  for (let i = 8; i < b.length; ) {
    const start = i;
    const id = b[i++];
    const [n, body] = leb(b, i);
    const end = body + n;
    let name = "";
    if (id === 0) {
      const [len, at] = leb(b, body);
      name = b.subarray(at, at + len).toString("utf8");
    }
    if (name === "name") removed += end - start;
    else keep.push(b.subarray(start, end));
    i = end;
  }
  if (removed) writeFileSync(file, Buffer.concat(keep));
  console.log(`${file}: ${removed ? `names removed (${Math.round(removed / 1024)} KB)` : "no names"}`);
}
