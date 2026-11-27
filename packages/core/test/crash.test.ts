// What happens when the engine traps (see src/crash.ts). Its own file because the test breaks the WebAssembly instance for good:
// nothing after it in the same process can use the engine, so this runs in a process of its own (`npm test` runs it last).
import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { Engine, EngineCrashedError, engineCrash, assertEngineSound, loadEngine, onEngineCrash, fidelityReport } from "../src/index.ts";

const here = new URL(".", import.meta.url);
await loadEngine(await readFile(new URL("../wasm/docxtor_bg.wasm", here)));

test("an engine that trapped refuses every later call, and nothing it returns can be saved", async () => {
  const bytes = new Uint8Array(await readFile(new URL("../../../fixtures/rich.docx", here)));
  const doc = Engine.open(bytes, "rich.docx");
  const other = Engine.blank("other.docx");
  const good = doc.save();
  assert.equal(fidelityReport(bytes, good).score, 100);
  const goodCopy = Uint8Array.from(good);

  // Errors that are not traps do not end the engine: a file that is not a document, an op that does not apply.
  assert.throws(() => Engine.open(new Uint8Array([1, 2, 3]), "bad.docx"));
  assert.throws(() => doc.apply({ op: "replace_text", find: "", replace: "x", all: true }));
  // Nor does a call on a document that was closed: the host's mistake, caught before any Rust runs.
  const gone = Engine.blank("gone.docx");
  const goneRaw = gone.raw;
  gone.dispose();
  assert.throws(() => goneRaw.wordCount());
  assert.throws(() => gone.save(), /was closed/);
  assert.equal(engineCrash(), null);
  assert.doesNotThrow(() => assertEngineSound());
  assert.equal(doc.save().length > 0, true);

  const told: EngineCrashedError[] = [];
  onEngineCrash((e) => told.push(e));
  const consoleError = console.error;
  const logged: unknown[][] = [];
  console.error = (...a: unknown[]) => void logged.push(a);
  try {
    // The deliberate panic: the first trap marks the engine, and the error says so with a code.
    assert.throws(
      () => (doc.raw as unknown as { __panicForTests(): void }).__panicForTests(),
      (e: unknown) => e instanceof EngineCrashedError && e.code === "engine_crashed" && e.cause instanceof WebAssembly.RuntimeError,
    );
  } finally {
    console.error = consoleError;
  }
  assert.ok(logged.some((a) => String(a[0]).includes("engine crashed")), "the crash is written to the console");
  assert.equal(told.length, 1);
  assert.equal(engineCrash(), told[0]);

  // Every call on every document now throws the same error: the one that crashed, and one that was never touched.
  const refused = (e: unknown) => e instanceof EngineCrashedError && e.code === "engine_crashed";
  for (const d of [doc, other]) {
    assert.throws(() => d.save(), refused);
    assert.throws(() => d.renderAll(), refused);
    assert.throws(() => d.revision, refused);
    assert.throws(() => d.raw.wordCount(), refused);
    assert.throws(() => d.apply({ op: "replace_text", find: "a", replace: "b", all: true }), refused);
    await assert.rejects(d.saveInSteps(), refused);
  }
  // Nothing new can be made either, so a host cannot swap in a fresh engine from the broken memory.
  assert.throws(() => Engine.open(good, "again.docx"), refused);
  assert.throws(() => Engine.blank(), refused);
  assert.throws(() => assertEngineSound(), refused);
  // A host that asks late is told at once; closing a crashed document is quiet.
  let late = 0;
  onEngineCrash(() => late++);
  assert.equal(late, 1);
  assert.doesNotThrow(() => doc.dispose());
  assert.equal(doc.isDisposed(), true);
  assert.equal(told.length, 1, "the crash is told once");
  // What was saved before the crash is untouched. The fidelity check is refused too: it needs the engine.
  assert.deepEqual(good, goodCopy);
  assert.throws(() => fidelityReport(bytes, good), refused);
});
