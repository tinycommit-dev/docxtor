// The time limit every request to another server goes through (net.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { timeLimit } from "../src/net.ts";

/** The name of what ended the signal. Node's time-outs don't keep it running, so a timer of our own does until then. */
const reason = (signal: AbortSignal) =>
  new Promise<string>((done) => {
    const keep = setTimeout(() => done("still running after 5 s"), 5000);
    signal.addEventListener("abort", () => {
      clearTimeout(keep);
      done((signal.reason as Error).name);
    });
  });

test("a request that takes too long ends with a TimeoutError, not an AbortError", async () => {
  assert.equal(await reason(timeLimit(10)), "TimeoutError");
});

test("the caller's own cancel still ends it, as an AbortError, before the time is up", async () => {
  const mine = new AbortController();
  const signal = timeLimit(60_000, mine.signal);
  mine.abort();
  assert.equal(signal.aborted, true);
  assert.equal((signal.reason as Error).name, "AbortError");
});

test("with a caller's signal, the time limit still holds", async () => {
  assert.equal(await reason(timeLimit(10, new AbortController().signal)), "TimeoutError");
});
