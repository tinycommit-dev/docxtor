// Reading providers: the registry, which one serves a language, and what happens when one has no answer or fails.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { defineExtension, ExtensionHost, type DocumentApi, type ReadingProvider, type UiApi } from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const newHost = (errors: unknown[] = []) => new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });

const provider = (id: string, languages: string[] | "*" | undefined, answer: string | null): ReadingProvider => ({ id, label: id, languages, reading: async () => answer });

test("the latest provider that knows the language answers, and says who it was", async () => {
  const host = newHost();
  await host.add(defineExtension({ id: "test.base", name: "Base", activate: (ctx) => void ctx.readings.register(provider("base", ["ja"], "かんじ")) }));
  await host.add(defineExtension({ id: "test.better", name: "Better", activate: (ctx) => void ctx.readings.register(provider("better", ["ja-JP"], "カンジ")) }));
  assert.deepEqual(await host.readingFor("漢字", "ja-JP"), { text: "カンジ", source: "better" });
  // The later one only knows the Japan variant: for plain Japanese the first answers.
  assert.deepEqual(await host.readingFor("漢字", "ja"), { text: "かんじ", source: "base" });
  // Nobody knows Korean.
  assert.equal(await host.readingFor("한자", "ko"), null);
  await host.disable("test.better");
  assert.deepEqual(await host.readingFor("漢字", "ja-JP"), { text: "かんじ", source: "base" });
});

test("one with no answer or one that fails hands the text on", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.many",
      name: "Many",
      activate(ctx) {
        ctx.readings.register(provider("rules", "*", "ほん"));
        ctx.readings.register(provider("silent", "*", null));
        ctx.readings.register({
          id: "broken",
          label: "Broken",
          reading: async () => {
            throw new Error("offline");
          },
        });
      },
    }),
  );
  assert.deepEqual(await host.readingFor("本", "ja"), { text: "ほん", source: "rules" });
  assert.equal(errors.length, 1);
});

test("a provider needs an id and a function", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.bad",
      name: "Bad",
      activate(ctx) {
        ctx.readings.register({ id: "", label: "x", reading: () => null });
        ctx.readings.register({ id: "y", label: "y" } as unknown as ReadingProvider);
      },
    }),
  );
  assert.ok(errors.length >= 1);
  assert.equal(host.readingProviders.list().length, 0);
});
