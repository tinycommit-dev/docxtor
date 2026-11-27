// The Review contribution points: thesauruses, inspectors (Inspect document) and comparers (Compare, Combine).
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { defineExtension, ExtensionHost, type DocumentApi, type DocumentComparer, type Inspector, type Op, type Thesaurus, type UiApi } from "../src/index.ts";

const ui = { notify() {} } as unknown as UiApi;

function newHost() {
  const errors: unknown[] = [];
  const batches: Op[][] = [];
  const doc = { exec: (ops: Op | Op[]) => void batches.push(Array.isArray(ops) ? ops : [ops]) } as unknown as DocumentApi;
  const host = new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });
  return { host, errors, batches };
}

const ext = (id: string, activate: Parameters<typeof defineExtension>[0]["activate"]) => defineExtension({ id, name: id, activate });

// -- thesaurus -----------------------------------------------------------------------------------------------

test("every thesaurus that serves the language answers, listed by source", async () => {
  const { host } = newHost();
  const asked: string[] = [];
  const th = (id: string, languages: Thesaurus["languages"], words: Record<string, string[]>): Thesaurus => ({
    id,
    label: id.toUpperCase(),
    languages,
    lookup(word, lang) {
      asked.push(`${id}:${word}:${lang}`);
      const synonyms = words[word];
      return synonyms ? { senses: [{ pos: "adjective", synonyms }] } : null;
    },
  });
  await host.add(
    ext("test.thesaurus", (ctx) => {
      ctx.thesaurus.register(th("en", ["en"], { quick: ["fast", "rapid"] }));
      ctx.thesaurus.register(th("any", "*", { quick: ["speedy"] }));
      ctx.thesaurus.register(th("es", ["es"], { quick: ["rápido"] }));
    }),
  );
  const answer = await host.thesaurusLookup("  quick ", "en-US");
  assert.equal(answer.word, "quick");
  assert.deepEqual(answer.results.map((r) => [r.source, r.label, r.senses[0].synonyms]), [
    ["en", "EN", ["fast", "rapid"]],
    ["any", "ANY", ["speedy"]],
  ]);
  // The Spanish one was not asked about an English word.
  assert.ok(!asked.some((a) => a.startsWith("es:")));
  assert.deepEqual(host.thesaurusesFor("es").map((t) => t.id), ["any", "es"]);
  assert.deepEqual(host.thesaurusesFor("de").map((t) => t.id), ["any"]);
});

test("synonyms are tidied and a thesaurus with nothing left is left out", async () => {
  const { host } = newHost();
  await host.add(
    ext("test.tidy", (ctx) => {
      ctx.thesaurus.register({
        id: "messy",
        label: "Messy",
        lookup: () => ({
          word: "run",
          senses: [
            { meaning: "move fast", synonyms: ["sprint", " dash ", "Sprint", "", "run", "RUN", "jog"], antonyms: ["walk", "walk"] },
            { synonyms: ["", "  "] },
            { pos: "noun", synonyms: ["asked"] },
          ],
        }),
      });
      ctx.thesaurus.register({ id: "empty", label: "Empty", lookup: () => ({ senses: [{ synonyms: ["asked"] }] }) });
      ctx.thesaurus.register({ id: "none", label: "None", lookup: () => null });
    }),
  );
  const a = await host.thesaurusLookup("asked", "en");
  assert.equal(a.results.length, 1);
  const r = a.results[0];
  assert.equal(r.word, "run");
  assert.deepEqual(r.senses, [{ meaning: "move fast", synonyms: ["sprint", "dash", "jog"], antonyms: ["walk"] }]);
});

test("a thesaurus that fails is reported and the others still answer", async () => {
  const { host, errors } = newHost();
  await host.add(
    ext("test.fail", (ctx) => {
      ctx.thesaurus.register({
        id: "down",
        label: "Down",
        lookup: async () => {
          throw new Error("network down");
        },
      });
      ctx.thesaurus.register({ id: "up", label: "Up", lookup: () => ({ senses: [{ synonyms: ["ok"] }] }) });
    }),
  );
  const a = await host.thesaurusLookup("fine", "en");
  assert.deepEqual(a.results.map((r) => r.source), ["up"]);
  assert.equal(errors.length, 1);
});

test("a thesaurus needs an id and a lookup", async () => {
  const { host } = newHost();
  await host.add(ext("test.bad1", (ctx) => ctx.thesaurus.register({ id: "", label: "x", lookup: () => null })));
  await host.add(ext("test.bad2", (ctx) => ctx.thesaurus.register({ id: "x", label: "x" } as unknown as Thesaurus)));
  const info = host.list().filter((e) => e.id.startsWith("test.bad"));
  assert.ok(info.every((i) => i.error));
});

test("disabling the extension takes its thesaurus away", async () => {
  const { host } = newHost();
  await host.add(ext("test.gone", (ctx) => ctx.thesaurus.register({ id: "t", label: "T", lookup: () => ({ senses: [{ synonyms: ["a"] }] }) })));
  assert.equal((await host.thesaurusLookup("x", "en")).results.length, 1);
  await host.disable("test.gone");
  assert.equal((await host.thesaurusLookup("x", "en")).results.length, 0);
});

// -- inspectors ----------------------------------------------------------------------------------------------

const inspector = (id: string, count: number, over: Partial<Inspector> = {}): Inspector => ({ id, label: id, inspect: () => ({ count }), ...over });

test("inspectors run in order and report what they found", async () => {
  const { host } = newHost();
  await host.add(
    ext("test.inspect", (ctx) => {
      ctx.inspectors.register(inspector("late", 1, { order: 200 }));
      ctx.inspectors.register(inspector("first", 3, { order: 10, description: "Names", inspect: () => ({ count: 3.9, detail: ["Ana", "Ben"] }), remove() {} }));
      ctx.inspectors.register(inspector("middle", 0));
      ctx.inspectors.register(inspector("odd", -4, { inspect: () => ({ count: Number.NaN }) }));
    }),
  );
  const report = await host.inspect();
  assert.deepEqual(report.map((r) => r.id), ["first", "middle", "odd", "late"]);
  assert.deepEqual(report[0], { id: "first", label: "first", description: "Names", count: 3, detail: ["Ana", "Ben"], removable: true });
  assert.equal(report[1].removable, false);
  assert.equal(report[2].count, 0);
});

test("an inspector that fails is listed with its error", async () => {
  const { host, errors } = newHost();
  await host.add(
    ext("test.inspect2", (ctx) => {
      ctx.inspectors.register(
        inspector("broken", 0, {
          inspect: async () => {
            throw new Error("no access");
          },
        }),
      );
      ctx.inspectors.register(inspector("fine", 2));
    }),
  );
  const report = await host.inspect();
  assert.equal(report.find((r) => r.id === "broken")?.error, "no access");
  assert.equal(report.find((r) => r.id === "fine")?.count, 2);
  assert.equal(errors.length, 1);
});

test("inspect() cannot change the document", async () => {
  const { host, batches } = newHost();
  await host.add(
    ext("test.inspect3", (ctx) => {
      ctx.inspectors.register(
        inspector("sneaky", 1, {
          inspect: (c) => {
            c.exec({ op: "x" });
            return { count: 1 };
          },
        }),
      );
    }),
  );
  const report = await host.inspect();
  assert.ok(report[0].error);
  assert.equal(batches.length, 0);
});

test("everything the chosen inspectors remove goes in as one step", async () => {
  const { host, batches, errors } = newHost();
  await host.add(
    ext("test.remove", (ctx) => {
      ctx.inspectors.register(inspector("a", 1, { order: 1, remove: (c) => c.exec({ op: "remove_hidden_data", what: ["comments"] }) }));
      ctx.inspectors.register(inspector("b", 1, { order: 2, remove: async (c) => c.exec([{ op: "one" }, { op: "two" }]) }));
      ctx.inspectors.register(
        inspector("c", 1, {
          order: 3,
          remove: (c) => {
            c.exec({ op: "half done" });
            throw new Error("cannot");
          },
        }),
      );
      ctx.inspectors.register(inspector("report-only", 1, { order: 4 }));
      ctx.inspectors.register(inspector("not-chosen", 1, { order: 5, remove: (c) => c.exec({ op: "no" }) }));
    }),
  );
  const done = await host.removeInspected(["c", "b", "a", "report-only"]);
  assert.deepEqual(done, ["a", "b"]);
  assert.equal(batches.length, 1);
  assert.deepEqual(batches[0].map((o) => o.op), ["remove_hidden_data", "one", "two"]);
  assert.equal(errors.length, 1);
  // Nothing chosen, nothing applied.
  await host.removeInspected([]);
  assert.equal(batches.length, 1);
});

// -- comparers -----------------------------------------------------------------------------------------------

test("the latest comparer that can do the job is used", async () => {
  const { host } = newHost();
  const bytes = new Uint8Array(0);
  const c = (id: string, parts: Partial<DocumentComparer>): DocumentComparer => ({ id, label: id, compare: () => [], ...parts });
  await host.add(
    ext("test.compare", (ctx) => {
      ctx.comparers.register(c("full", { redline: () => bytes, combine: () => bytes }));
      ctx.comparers.register(c("words-only", {}));
    }),
  );
  assert.equal(host.comparerFor("compare")?.id, "words-only");
  assert.equal(host.comparerFor("redline")?.id, "full");
  assert.equal(host.comparerFor("combine")?.id, "full");
  await host.disable("test.compare");
  assert.equal(host.comparerFor("compare"), undefined);
});
