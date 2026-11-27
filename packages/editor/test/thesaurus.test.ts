// Docxtor's English thesaurus: the data's format, look-ups of inflected words, endings on synonyms, opposites and capitals.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ENGLISH_GROUPS } from "../src/thesaurus/english-data.ts";
import { candidates, EnglishThesaurus, inflect, matchCase } from "../src/thesaurus/english.ts";

const th = new EnglishThesaurus(ENGLISH_GROUPS);
const synonyms = (word: string) => th.lookup(word)?.senses.flatMap((s) => s.synonyms) ?? [];

test("the data is well formed", () => {
  for (const line of ENGLISH_GROUPS.split("\n").map((l) => l.trim()).filter(Boolean)) {
    if (line.startsWith("!")) {
      const parts = line.slice(1).split("|");
      assert.equal(parts.length, 3, line);
      assert.match(parts[0], /^[nvar]$/, line);
      assert.ok(parts[1] && parts[2], line);
      continue;
    }
    const [pos, list, ...rest] = line.split("|");
    assert.match(pos, /^[nvar]$/, line);
    assert.equal(rest.length, 0, line);
    const words = list.split(",").map((w) => w.trim());
    assert.ok(words.length >= 2, line);
    assert.ok(words.every((w) => w && w === w.toLowerCase() && !/[|;]/.test(w)), line);
  }
  // Big enough to be useful, small enough to load when asked.
  assert.ok(th.size > 4000, `${th.size} words`);
  assert.ok(ENGLISH_GROUPS.length < 260_000);
});

test("a word gets its synonyms, one meaning for each group", () => {
  const entry = th.lookup("big")!;
  assert.equal(entry.word, undefined);
  assert.ok(entry.senses.length >= 1);
  assert.equal(entry.senses[0].pos, "adjective");
  assert.ok(entry.senses[0].synonyms.includes("large") && !entry.senses[0].synonyms.includes("big"));
  // A word in groups of different kinds has a meaning for each.
  const set = th.lookup("hard")!;
  assert.ok(set.senses.some((s) => s.pos === "adjective"));
  assert.ok(set.senses.some((s) => s.pos === "adverb"));
  assert.equal(th.lookup("zzzzzz"), null);
  assert.equal(th.lookup("   "), null);
});

test("capitals and spaces don't matter in the look-up", () => {
  assert.deepEqual(synonyms("  BIG "), synonyms("big"));
  assert.ok(synonyms("Big").includes("large"));
});

test("a word that starts its group comes first", () => {
  const entry = th.lookup("good")!;
  assert.equal(entry.senses[0].meaning, "fine");
});

test("inflected words are found through their base word and the synonyms get the ending", () => {
  // Running is a noun of its own, and the -ing form of run: both are listed, the noun first.
  const running = th.lookup("running")!;
  assert.equal(running.word, undefined);
  assert.equal(running.senses[0].pos, "noun");
  assert.equal(running.senses[0].word, undefined);
  const ofRun = running.senses.filter((s) => s.word === "run");
  assert.ok(ofRun.length > 0 && ofRun.every((s) => s.pos === "verb"));
  const all = ofRun.flatMap((s) => s.synonyms);
  assert.ok(all.includes("sprinting") && all.includes("racing") && all.includes("dashing"), all.join());
  assert.ok(!all.includes("sprint"));
  const stopped = th.lookup("stopped")!;
  assert.equal(stopped.word, "stop");
  assert.ok(stopped.senses.flatMap((s) => s.synonyms).includes("halted"));
  const says = th.lookup("says")!;
  assert.equal(says.word, "say");
  assert.ok(says.senses.flatMap((s) => s.synonyms).includes("states"));
  const cities = th.lookup("cities")!;
  assert.equal(cities.word, "city");
  assert.ok(cities.senses.flatMap((s) => s.synonyms).includes("towns"));
});

test("irregular verbs are found from their past and their participle", () => {
  const said = th.lookup("said")!;
  assert.equal(said.word, "say");
  assert.ok(said.senses.flatMap((s) => s.synonyms).includes("stated"));
  // Gone is an adjective too: the verb go follows it.
  const gone = th.lookup("gone")!;
  assert.ok(gone.senses.filter((s) => s.word === "go").flatMap((s) => s.synonyms).includes("left"));
  const took = th.lookup("took")!;
  assert.equal(took.word, "take");
  assert.ok(took.senses.flatMap((s) => s.synonyms).includes("grabbed"));
  // Irregular synonyms take their own forms.
  assert.ok(synonyms("gave").includes("provided") || synonyms("gave").includes("handed over"));
  const thought = th.lookup("thought");
  assert.ok(thought, "thought is a noun and a verb");
});

test("adjectives take -er, -est and -ly", () => {
  const bigger = th.lookup("bigger")!;
  assert.equal(bigger.word, "big");
  assert.ok(bigger.senses.flatMap((s) => s.synonyms).includes("larger"));
  const quickest = th.lookup("quickest")!;
  assert.equal(quickest.word, "quick");
  assert.ok(quickest.senses.flatMap((s) => s.synonyms).includes("fastest"));
  const happily = th.lookup("happily")!;
  // Happily is an adverb of its own too (fortunately); the -ly form of happy follows.
  const ofHappy = happily.senses.filter((s) => s.word === "happy");
  assert.ok(ofHappy.length > 0 && ofHappy.every((s) => s.pos === "adverb"));
  assert.ok(ofHappy.flatMap((s) => s.synonyms).includes("cheerfully"));
});

test("a long adjective that can't take -er is left out of a comparative", () => {
  // "beautiful" has no -er form, so "prettier" is shown with the ones that do.
  const prettier = th.lookup("prettier")!;
  const all = prettier.senses.flatMap((s) => s.synonyms);
  assert.ok(all.includes("lovelier"));
  assert.ok(!all.some((w) => w.includes("beautifuler")));
});

test("opposites come with the meaning that has them", () => {
  const big = th.lookup("big")!;
  assert.ok(big.senses[0].antonyms?.includes("small"));
  const start = th.lookup("begin")!;
  assert.ok(start.senses[0].antonyms?.includes("end"));
  // An opposite follows the ending too.
  const began = th.lookup("began")!;
  assert.ok(began.senses[0].antonyms?.includes("ended") || began.senses[0].antonyms?.includes("finished"));
});

test("spelling rules for endings", () => {
  assert.equal(inflect("stop", "ing"), "stopping");
  assert.equal(inflect("make", "ing"), "making");
  assert.equal(inflect("die", "ing"), "dying");
  assert.equal(inflect("see", "ing"), "seeing");
  assert.equal(inflect("open", "past"), "opened");
  assert.equal(inflect("plan", "past"), "planned");
  assert.equal(inflect("study", "past"), "studied");
  assert.equal(inflect("use", "past"), "used");
  assert.equal(inflect("go", "past"), "went");
  assert.equal(inflect("go", "pp"), "gone");
  assert.equal(inflect("box", "s"), "boxes");
  assert.equal(inflect("try", "s"), "tries");
  assert.equal(inflect("day", "s"), "days");
  assert.equal(inflect("child", "s"), "children");
  assert.equal(inflect("big", "er"), "bigger");
  assert.equal(inflect("large", "er"), "larger");
  assert.equal(inflect("happy", "est"), "happiest");
  assert.equal(inflect("beautiful", "er"), null);
  assert.equal(inflect("quick", "ly"), "quickly");
  assert.equal(inflect("happy", "ly"), "happily");
  assert.equal(inflect("basic", "ly"), "basically");
  assert.equal(inflect("able", "ly"), "ably");
});

test("candidates start with the word itself", () => {
  const list = candidates("Stopped").map((c) => `${c.stem}${c.ending ? `:${c.ending}` : ""}`);
  assert.equal(list[0], "stopped");
  assert.ok(list.includes("stop:past") && list.includes("stop:pp"));
  assert.ok(candidates("happier").some((c) => c.stem === "happy" && c.ending === "er"));
  assert.ok(candidates("lives").some((c) => c.stem === "life"));
});

test("a phrase takes the ending on the right word", () => {
  const giving = th.lookup("giving up")?.senses ?? [];
  assert.equal(giving.length, 0, "a phrase that is not a word has no entry");
  const quitting = th.lookup("quitting")!;
  assert.equal(quitting.word, "quit");
  assert.ok(quitting.senses.flatMap((s) => s.synonyms).some((w) => /^giving up|^abandoning|^leaving|^resigning/.test(w)));
});

test("the replacement takes the capitals of the word it replaces", () => {
  assert.equal(matchCase("Big", "large"), "Large");
  assert.equal(matchCase("BIG", "large"), "LARGE");
  assert.equal(matchCase("big", "large"), "large");
  assert.equal(matchCase("I", "me"), "Me");
  assert.equal(matchCase("Big", "a lot"), "A lot");
  assert.equal(matchCase("", "x"), "x");
});
