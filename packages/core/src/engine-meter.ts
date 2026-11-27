// The two optional wrappers around the binding: the invariant watch for tests and the performance meter.
import type { DocxtorDoc } from "../wasm/docxtor.js";

/**
 * The engine's own check of its rules after every edit (see `Document::check_invariants` in docxtor-core), for tests.
 * It is on when `globalThis.__DOCXTOR_CHECKS__ === true` before an Engine is made. A rule an edit breaks is reported with
 * `console.error("Docxtor invariant broken: " + JSON.stringify(violation))`; the browser test watchdog fails on that prefix.
 * Every call on the document (edits, undo, tools, save...) is followed by a look at what the engine found, so none is missed.
 */
export function watched(raw: DocxtorDoc): DocxtorDoc {
  raw.setInvariantChecks(true);
  const report = (): void => {
    let found: { rule: string; message: string }[];
    try {
      found = JSON.parse(raw.takeInvariantViolations());
    } catch {
      return; // a freed document has nothing to say
    }
    for (const v of found) console.error("Docxtor invariant broken: " + JSON.stringify(v));
  };
  return new Proxy(raw, {
    get(target, key) {
      const value = Reflect.get(target, key, target);
      if (typeof value !== "function" || key === "free" || key === "setInvariantChecks" || key === "takeInvariantViolations") return value;
      return (...args: unknown[]) => {
        try {
          return (value as (...a: unknown[]) => unknown).apply(target, args);
        } finally {
          report();
        }
      };
    },
  });
}

/**
 * What the performance harness reads (see docs/performance/index.md). It is on when `globalThis.__DOCXTOR_PERF__` is an object
 * before an Engine is made: every call into the engine is then counted by method (calls, JSON characters sent and
 * received, milliseconds inside the call). Off, nothing here runs.
 */
export interface PerfMeter {
  /** `inChars` and `outChars` count JSON and other text; `inBytes` and `outBytes` the files that cross as bytes (a save, an open). */
  calls: Record<string, { n: number; inChars: number; outChars: number; inBytes?: number; outBytes?: number; ms: number }>;
}
export function metered(raw: DocxtorDoc, meter: PerfMeter): DocxtorDoc {
  const size = (v: unknown): number => (typeof v === "string" ? v.length : 0);
  const bytes = (v: unknown): number => (v instanceof Uint8Array ? v.byteLength : 0);
  return new Proxy(raw, {
    get(target, key) {
      const value = Reflect.get(target, key, target);
      if (typeof value !== "function" || key === "free" || typeof key !== "string") return value;
      return (...args: unknown[]) => {
        const c = (meter.calls[key] ??= { n: 0, inChars: 0, outChars: 0, ms: 0 });
        const t = performance.now();
        const out = (value as (...a: unknown[]) => unknown).apply(target, args);
        c.ms += performance.now() - t;
        c.n++;
        for (const a of args) {
          c.inChars += size(a);
          c.inBytes = (c.inBytes ?? 0) + bytes(a);
        }
        c.outChars += size(out);
        c.outBytes = (c.outBytes ?? 0) + bytes(out);
        return out;
      };
    },
  });
}
