// Hyphenators: the registry, which one serves a language, and what happens when one has no opinion or fails.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { defineExtension, ExtensionHost, type DocumentApi, type Hyphenator, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const newHost = (errors: unknown[] = []) => new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });

const provider = (id: string, languages: string[] | "*" | undefined, points: number[] | null): Hyphenator => ({ id, label: id, languages, hyphenate: async () => points });

test("the latest hyphenator that knows the language answers", async () => {
  const host = newHost();
  await host.add(defineExtension({ id: "test.base", name: "Base", activate: (ctx) => void ctx.hyphenation.register(provider("base", ["en"], [2, 5])) }));
  await host.add(defineExtension({ id: "test.better", name: "Better", activate: (ctx) => void ctx.hyphenation.register(provider("better", ["en-US"], [3])) }));
  assert.deepEqual(await host.hyphenationPoints("hyphenate", "en-US"), [3]);
  // The later one doesn't know British English: the first does.
  assert.deepEqual(await host.hyphenationPoints("hyphenate", "en-GB"), [2, 5]);
  // Nobody knows Italian.
  assert.deepEqual(await host.hyphenationPoints("hyphenate", "it"), []);
  await host.disable("test.better");
  assert.deepEqual(await host.hyphenationPoints("hyphenate", "en-US"), [2, 5]);
});

test("one with no opinion or one that fails hands the word on, and bad offsets are dropped", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.many",
      name: "Many",
      activate(ctx) {
        ctx.hyphenation.register(provider("rules", "*", [9, 0, 3, 3, 2.5, -1, 6]));
        ctx.hyphenation.register(provider("silent", "*", null));
        ctx.hyphenation.register({
          id: "broken",
          label: "Broken",
          hyphenate: async () => {
            throw new Error("offline");
          },
        });
      },
    }),
  );
  // The broken one is reported and skipped, the silent one passes, the first one's list is cleaned up and sorted.
  assert.deepEqual(await host.hyphenationPoints("example", "en"), [3, 6]);
  assert.equal(errors.length, 1);
});

test("a hyphenator needs an id and a function", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.bad",
      name: "Bad",
      activate(ctx) {
        ctx.hyphenation.register({ id: "", label: "x", hyphenate: () => [] });
        ctx.hyphenation.register({ id: "y", label: "y" } as unknown as Hyphenator);
      },
    }),
  );
  assert.ok(errors.length >= 1);
  assert.equal(host.hyphenators.list().length, 0);
});
