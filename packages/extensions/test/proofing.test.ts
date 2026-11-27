// Spelling dictionaries and grammar checkers: the registries, which provider serves a language, and how
// several checkers' findings are merged.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  defineExtension,
  ExtensionHost,
  languageMatches,
  mergeIssues,
  type DocumentApi,
  type GrammarChecker,
  type GrammarIssue,
  type SpellingDictionaryProvider,
  type UiApi,
} from "../src/index.ts";

const doc = {} as DocumentApi;
const ui = { notify() {} } as unknown as UiApi;
const newHost = (errors: unknown[] = []) => new ExtensionHost({ document: doc, ui, mac: true, onError: (_id, err) => errors.push(err) });

const issue = (over: Partial<GrammarIssue> = {}): GrammarIssue => ({
  paragraph: 1,
  from: 0,
  to: 3,
  message: "Repeated word",
  replacements: ["the"],
  rule: { id: "REPEAT" },
  ...over,
});

test("a base language covers its regions, a region covers itself", () => {
  assert.ok(languageMatches(undefined, "ja"));
  assert.ok(languageMatches("*", "ja"));
  assert.ok(languageMatches(["en"], "en-US"));
  assert.ok(languageMatches(["en"], "en"));
  assert.ok(languageMatches(["en"], "en_GB"));
  assert.ok(languageMatches(["en-GB"], "en-gb"));
  assert.ok(!languageMatches(["en-GB"], "en-US"));
  assert.ok(!languageMatches(["en"], "es"));
  assert.ok(!languageMatches(["en"], "eng"));
  assert.ok(languageMatches(["es", "fr"], "fr-CA"));
  assert.ok(!languageMatches([], "en"));
});

test("the latest dictionary provider that has the language wins", async () => {
  const host = newHost();
  const asked: string[] = [];
  const provider = (id: string, languages: SpellingDictionaryProvider["languages"], has: string[]): SpellingDictionaryProvider => ({
    id,
    label: id,
    languages,
    async load(lang) {
      asked.push(`${id}:${lang}`);
      return has.includes(lang) ? { aff: id, dic: id } : null;
    },
  });
  await host.add(
    defineExtension({
      id: "test.base",
      name: "Base",
      activate(ctx) {
        ctx.spelling.dictionaries.register(provider("base", ["en", "es"], ["en-US", "es"]));
      },
    }),
  );
  const better = defineExtension({
    id: "test.better",
    name: "Better",
    activate(ctx) {
      ctx.spelling.dictionaries.register(provider("better", ["en-US"], ["en-US"]));
    },
  });
  await host.add(better);

  assert.equal((await host.spellingDictionary("en-US"))?.aff, "better");
  // The later provider doesn't cover Spanish: it isn't asked.
  assert.equal((await host.spellingDictionary("es"))?.aff, "base");
  assert.ok(!asked.includes("better:es"));
  // Nobody has Italian.
  assert.equal(await host.spellingDictionary("it"), null);

  // Turning the better one off gives the language back to the first.
  await host.disable("test.better");
  assert.equal((await host.spellingDictionary("en-US"))?.aff, "base");
});

test("a provider that returns nothing or throws hands the language on", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.dicts",
      name: "Dicts",
      activate(ctx) {
        ctx.spelling.dictionaries.register({ id: "old", label: "Old", load: async () => ({ aff: "old", dic: "old" }) });
        ctx.spelling.dictionaries.register({ id: "empty", label: "Empty", languages: "*", load: async () => null });
        ctx.spelling.dictionaries.register({
          id: "broken",
          label: "Broken",
          load: async () => {
            throw new Error("offline");
          },
        });
      },
    }),
  );
  assert.equal((await host.spellingDictionary("fr"))?.aff, "old");
  assert.equal(errors.length, 1);
});

test("registrations need an id and a function", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.bad",
      name: "Bad",
      activate(ctx) {
        ctx.grammar.register({ id: "", label: "x", check: async () => [] });
      },
    }),
  );
  assert.equal(errors.length, 1);
  assert.equal(host.grammarCheckers.list().length, 0);
  await host.add(
    defineExtension({
      id: "test.bad2",
      name: "Bad 2",
      activate(ctx) {
        ctx.spelling.dictionaries.register({ id: "d", label: "d" } as unknown as SpellingDictionaryProvider);
      },
    }),
  );
  assert.equal(errors.length, 2);
});

test("several grammar checkers are asked, each only about the languages it covers, and their issues are merged", async () => {
  const host = newHost();
  const seen: Record<string, number[]> = {};
  const checker = (id: string, languages: GrammarChecker["languages"], found: (ids: number[]) => GrammarIssue[]): GrammarChecker => ({
    id,
    label: id,
    languages,
    async check({ paragraphs }) {
      seen[id] = paragraphs.map((p) => p.id);
      return found(seen[id]);
    },
  });
  await host.add(
    defineExtension({
      id: "test.a",
      name: "A",
      activate(ctx) {
        ctx.grammar.register(checker("a", ["en"], () => [issue({ paragraph: 2, from: 4, to: 8 }), issue({ paragraph: 1 })]));
      },
    }),
  );
  await host.add(
    defineExtension({
      id: "test.b",
      name: "B",
      activate(ctx) {
        // The same finding as A's first one (dropped), a different message on the same words (kept), a hint.
        ctx.grammar.register(
          checker("b", "*", () => [issue({ paragraph: 2, from: 4, to: 8 }), issue({ paragraph: 2, from: 4, to: 8, message: "Say it once" }), issue({ paragraph: 3, kind: "style", message: "Wordy" })]),
        );
      },
    }),
  );
  const paragraphs = [
    { id: 1, text: "the the cat", lang: "en-US" },
    { id: 2, text: "one two three four", lang: "en-GB" },
    { id: 3, text: "una frase larga aquí", lang: "es-ES" },
  ];
  const found = await host.checkGrammar(paragraphs);
  assert.deepEqual(seen.a, [1, 2]);
  assert.deepEqual(seen.b, [1, 2, 3]);
  assert.deepEqual(
    found.map((i) => [i.paragraph, i.from, i.message, i.source, i.kind]),
    [
      [1, 0, "Repeated word", "a", "grammar"],
      [2, 4, "Repeated word", "a", "grammar"],
      [2, 4, "Say it once", "b", "grammar"],
      [3, 0, "Wordy", "b", "style"],
    ],
  );
  assert.deepEqual(host.grammarCheckersFor("es-MX").map((c) => c.id), ["b"]);
  assert.deepEqual(host.grammarCheckersFor("en").map((c) => c.id), ["a", "b"]);

  // Turning one off takes its issues away.
  await host.disable("test.a");
  assert.deepEqual((await host.checkGrammar(paragraphs)).map((i) => i.source), ["b", "b", "b"]);
});

test("a checker that fails doesn't take the others down, and nonsense issues are dropped", async () => {
  const errors: unknown[] = [];
  const host = newHost(errors);
  await host.add(
    defineExtension({
      id: "test.mixed",
      name: "Mixed",
      activate(ctx) {
        ctx.grammar.register({
          id: "down",
          label: "Down",
          check: async () => {
            throw new Error("offline");
          },
        });
        ctx.grammar.register({
          id: "sloppy",
          label: "Sloppy",
          check: async () =>
            [
              issue({ from: 0, to: 3 }),
              issue({ from: 5, to: 50 }), // past the end of the text
              issue({ from: 4, to: 4 }), // nothing selected
              issue({ paragraph: 99 }), // a paragraph that wasn't asked about
              issue({ message: "" }),
              { paragraph: 1, from: 0, to: 2, message: "No replacements or rule" },
              null,
            ] as unknown as GrammarIssue[],
        });
      },
    }),
  );
  const found = await host.checkGrammar([{ id: 1, text: "abc def", lang: "en" }]);
  assert.equal(errors.length, 1);
  assert.deepEqual(
    found.map((i) => [i.from, i.to, i.message, i.replacements, i.rule.id]),
    [
      [0, 2, "No replacements or rule", [], "sloppy"],
      [0, 3, "Repeated word", ["the"], "REPEAT"],
    ],
  );
});

test("partial results arrive as each checker answers, so a slow one does not hold back a fast one", async () => {
  const host = newHost();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => (release = resolve));
  await host.add(
    defineExtension({
      id: "test.pair",
      name: "Pair",
      activate(ctx) {
        ctx.grammar.register({
          id: "slow",
          label: "Slow",
          async check() {
            await gate;
            return [issue({ rule: { id: "SLOW" }, message: "From the slow one" })];
          },
        });
        ctx.grammar.register({ id: "fast", label: "Fast", check: async () => [issue({ rule: { id: "FAST" }, message: "From the fast one" })] });
      },
    }),
  );
  const seen: string[][] = [];
  const done = host.checkGrammar([{ id: 1, text: "the the cat", lang: "en" }], (issues) => seen.push(issues.map((i) => i.source!)));
  await new Promise((r) => setTimeout(r, 10));
  // The fast checker's issues are already out while the slow one waits.
  assert.deepEqual(seen, [["fast"]]);
  release();
  const all = await done;
  assert.deepEqual(seen.at(-1), ["slow", "fast"]);
  assert.deepEqual(all.map((i) => i.source), ["slow", "fast"]);
});

test("mergeIssues keeps issues that differ in their replacements", () => {
  const merged = mergeIssues([[issue({ replacements: ["a"] })], [issue({ replacements: ["b"] }), issue({ replacements: ["a"] })]]);
  assert.equal(merged.length, 2);
});

test("an extension can start off, and says what it sends away", async () => {
  const host = newHost();
  await host.add(
    defineExtension({ id: "test.remote", name: "Remote", enabledByDefault: false, privacy: "Sends your text to example.org.", activate() {} }),
    { enabled: false },
  );
  const info = host.list().find((x) => x.id === "test.remote")!;
  assert.equal(info.enabledByDefault, false);
  assert.equal(info.privacy, "Sends your text to example.org.");
  assert.equal(info.enabled, false);
});
