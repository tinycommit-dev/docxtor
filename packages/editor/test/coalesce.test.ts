import { strict as assert } from "node:assert";
import { test } from "node:test";
import { coalesce } from "../src/ui/dom.ts";

test("a coalesced redraw runs once after many changes in a row, and again for later changes", async () => {
  let runs = 0;
  const redraw = coalesce(() => runs++);
  for (let i = 0; i < 600; i++) redraw();
  assert.equal(runs, 0, "not while the changes are still coming");
  await Promise.resolve();
  assert.equal(runs, 1);
  redraw();
  await Promise.resolve();
  assert.equal(runs, 2);
});
