// The engine in Node: what one open, one keystroke, one split, one undo and one save cost, counted exactly.
//
//   node --experimental-strip-types perf/engine.mts [--wasm path] [--docs series-100,series-1000] [--keys 100] [--repeats 3]
//        [--plant busy=4000000,alloc_mb=20,double_clone=1]
//
// Prints one JSON object on the last line of stdout. Exact (they gate): body copies, JSON characters crossing the
// boundary per keystroke, calls per keystroke, linear-memory high-water. Reported only: milliseconds.
import { readFile } from "node:fs/promises";
import { Engine, loadEngine } from "../packages/core/src/index.ts";
import { named } from "./lib/docs.mts";
import { median, round } from "./lib/stats.mts";

const arg = (name: string, d: string) => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : d;
};
const wasmPath = arg("wasm", new URL("../packages/core/wasm/docxtor_bg.wasm", import.meta.url).pathname);
const docs = arg("docs", "series-100,series-1000,series-5000").split(",");
const KEYS = Number(arg("keys", "100"));
const REPEATS = Number(arg("repeats", "3"));
const plants = Object.fromEntries(arg("plant", "").split(",").filter(Boolean).map((p) => p.split("=")).map(([k, v]) => [k, Number(v)]));

// Catch the engine's linear memory (it only grows, so its size at the end is its high-water mark).
const memories: WebAssembly.Memory[] = [];
const exportsSeen: Record<string, unknown>[] = [];
const inst = WebAssembly.instantiate.bind(WebAssembly) as (...a: unknown[]) => Promise<unknown>;
(WebAssembly as unknown as Record<string, unknown>).instantiate = (...a: unknown[]) =>
  inst(...a).then((r) => {
    const i = (r as { instance?: WebAssembly.Instance }).instance ?? (r as WebAssembly.Instance);
    const ex = (i as unknown as { exports: Record<string, unknown> }).exports;
    // Node's own HTTP parser is WebAssembly too; only the engine's instance (it has wasm-bindgen's allocator) counts.
    if (!("__wbindgen_malloc" in ex)) return r;
    if (ex.memory instanceof WebAssembly.Memory) memories.push(ex.memory);
    exportsSeen.push(ex);
    return r;
  });
const wasmBytes = () => memories.reduce((s, m) => s + m.buffer.byteLength, 0);

const meter = { calls: {} as Record<string, { n: number; inChars: number; outChars: number; ms: number }> };
(globalThis as Record<string, unknown>).__DOCXTOR_PERF__ = meter;
const reset = () => {
  for (const k of Object.keys(meter.calls)) delete meter.calls[k];
};
const snap = () => JSON.parse(JSON.stringify(meter.calls)) as typeof meter.calls;

await loadEngine(await readFile(wasmPath));
const PLANT_CODE: Record<string, number> = { busy: 1, alloc_mb: 2, double_clone: 3 };
for (const [k, v] of Object.entries(plants)) {
  const ex = exportsSeen[0];
  const fn = ex.setPerfPlant ?? ex.set_perf_plant;
  if (typeof fn !== "function") throw new Error("this wasm was not built with the perf-plant feature (" + exportsSeen.length + " instances; exports: " + Object.keys(ex).join(",").slice(0, 300) + ")");
  (fn as (a: number, b: number) => void)(PLANT_CODE[k], v);
}
const counters = () => {
  // perfCounters is a plain export of the glue; the meter does not see it.
  return (globalThis as Record<string, unknown>).__perfCounters as () => { bodyClones: number };
};
const glue = (await import("../packages/core/wasm/docxtor.js")) as unknown as { perfCounters?: () => string };
const clones = () => (glue.perfCounters ? (JSON.parse(glue.perfCounters()) as { bodyClones: number }).bodyClones : NaN);
const walked = () => (glue.perfCounters ? ((JSON.parse(glue.perfCounters()) as { paragraphsWalked?: number }).paragraphsWalked ?? NaN) : NaN);
const counter = (name: string) => (glue.perfCounters ? ((JSON.parse(glue.perfCounters()) as Record<string, number>)[name] ?? NaN) : NaN);
void counters;

const out: Record<string, unknown> = { wasm: wasmPath.split("/").slice(-2).join("/"), plants, docs: {} };
const ms = () => performance.now();

for (const name of docs) {
  const built = named(name);
  const bytes = built.bytes;
  const opens: number[] = [];
  const ready: number[] = [];
  let doc!: Engine;
  let openCounts: unknown;
  for (let r = 0; r < REPEATS; r++) {
    doc?.dispose();
    reset();
    let t = ms();
    doc = Engine.open(bytes, `${name}.docx`);
    opens.push(ms() - t);
    doc.renderAll();
    ready.push(ms() - t);
    if (r === 0) openCounts = snap();
  }
  const order = doc.renderAll().order;
  const typeAt = (frac: number) => order[Math.floor(order.length * frac)];
  const measureKeys = (p: number) => {
    // The first key starts an undo step; the rest merge, as typing does.
    doc.apply({ op: "insert_text", at: { p, o: 0 }, text: "x" } as never, false);
    doc.renderChanged();
    reset();
    const c0 = clones();
    const w0 = walked();
    const per: number[] = [];
    for (let i = 1; i <= KEYS; i++) {
      const t = ms();
      doc.apply({ op: "insert_text", at: { p, o: i }, text: "x" } as never, true);
      doc.renderChanged();
      per.push(ms() - t);
    }
    const calls = snap();
    const walkedPerKey = round((walked() - w0) / KEYS, 3);
    const clonesPerKey = round((clones() - c0) / KEYS, 3);
    // What the status bar's word count walks after a key (the editor asks 300 ms after the last one), counted apart from the key's own work.
    const raw = (doc as unknown as { raw: { wordCount?: () => number } }).raw;
    let wordWalked = NaN;
    if (raw.wordCount) {
      raw.wordCount();
      wordWalked = 0;
      for (let i = KEYS + 1; i <= KEYS + 20; i++) {
        doc.apply({ op: "insert_text", at: { p, o: i }, text: "x" } as never, true);
        doc.renderChanged();
        const before = walked();
        raw.wordCount();
        wordWalked += walked() - before;
      }
    }
    // What an autosave does after a key: the first half (dates and statistics) and the second (the parts written out). One key
    // between each, after one save that makes everything kept (the first writes every block).
    const rawSave = (doc as unknown as { raw: { stamp?: (now: string) => void; savePackedPieces?: (into: Uint8Array[]) => unknown } }).raw;
    let stampWalked = NaN;
    let blocksWritten = NaN;
    let stampMs = NaN;
    let savePackedMs = NaN;
    if (rawSave.stamp && rawSave.savePackedPieces) {
      rawSave.stamp("2026-11-21T10:00:00Z");
      rawSave.savePackedPieces([]);
      stampWalked = 0;
      blocksWritten = 0;
      const sts: number[] = [];
      const sps: number[] = [];
      const SAVES = 5;
      for (let i = KEYS + 21; i < KEYS + 21 + SAVES; i++) {
        doc.apply({ op: "insert_text", at: { p, o: i }, text: "x" } as never, true);
        doc.renderChanged();
        const w = walked();
        const b = counter("blocksWritten");
        let t0 = ms();
        rawSave.stamp(`2026-11-21T10:00:0${i - KEYS - 20}Z`);
        sts.push(ms() - t0);
        stampWalked += walked() - w;
        t0 = ms();
        rawSave.savePackedPieces([]);
        sps.push(ms() - t0);
        blocksWritten += counter("blocksWritten") - b;
      }
      stampWalked = round(stampWalked / SAVES, 3);
      blocksWritten = round(blocksWritten / SAVES, 3);
      stampMs = round(median(sts), 2);
      savePackedMs = round(median(sps), 2);
    }
    const jsonIn = Object.values(calls).reduce((s, c) => s + c.inChars, 0);
    const jsonOut = Object.values(calls).reduce((s, c) => s + c.outChars, 0);
    return {
      ms: round(median(per), 3),
      msMax: round(Math.max(...per), 3),
      bodyClones: clonesPerKey,
      paragraphsWalked: walkedPerKey,
      wordCountWalked: round(wordWalked / 20, 3),
      stampWalked,
      blocksWritten,
      stampMs,
      savePackedMs,
      jsonInChars: Math.round(jsonIn / KEYS),
      jsonOutChars: Math.round(jsonOut / KEYS),
      calls: Object.fromEntries(Object.entries(calls).map(([k, c]) => [k, round(c.n / KEYS, 2)])),
    };
  };
  const mid = measureKeys(typeAt(0.5));
  const end = measureKeys(typeAt(0.95));
  let t = ms();
  doc.undo();
  doc.renderChanged();
  const undo = ms() - t;
  t = ms();
  doc.save();
  const save = ms() - t;
  (out.docs as Record<string, unknown>)[name] = {
    paragraphs: built.paragraphs,
    docxBytes: bytes.byteLength,
    open: { ms: round(median(opens), 1), readyMs: round(median(ready), 1), jsonOutChars: (openCounts as typeof meter.calls).renderAll?.outChars ?? 0 },
    keyMiddle: mid,
    keyNearEnd: end,
    undoMs: round(undo, 1),
    saveMs: round(save, 1),
    wasmMemoryBytes: wasmBytes(),
  };
  doc.dispose();
}
console.log(JSON.stringify(out));
