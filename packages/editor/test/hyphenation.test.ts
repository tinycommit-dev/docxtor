// Docxtor's rule-based hyphenator: breaks where a reader expects, nothing in short words, nothing in what isn't a word.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ruleBreaks } from "../src/hyphenation/rules.ts";

/** The word with a hyphen at every break. */
const hyphenated = (word: string, lang = "en") => {
  const points = ruleBreaks(word, lang) ?? [];
  return [...word].map((c, i) => (points.includes(i) ? `-${c}` : c)).join("");
};

test("English words break between syllables", () => {
  assert.equal(hyphenated("hyphenate"), "hy-phe-nate");
  assert.equal(hyphenated("master"), "mas-ter");
  assert.equal(hyphenated("packaging"), "pack-a-ging");
  assert.equal(hyphenated("teacher"), "tea-cher");
  assert.equal(hyphenated("example"), "exam-ple");
  assert.equal(hyphenated("better"), "bet-ter");
});

test("Spanish splits vowels that don't make one sound", () => {
  assert.equal(hyphenated("maestro", "es"), "ma-es-tro");
  // The last two letters are never left alone on a line.
  assert.equal(hyphenated("computadora", "es"), "com-pu-ta-dora");
});

test("no break leaves a stub, short words stay whole, and things that aren't words are left alone", () => {
  for (const w of ["cat", "tiger", "table", "paper"]) {
    for (const p of ruleBreaks(w) ?? []) assert.ok(p >= 2 && w.length - p >= 3, `${w} at ${p}`);
  }
  assert.deepEqual(ruleBreaks("a"), []);
  assert.equal(ruleBreaks("co-op"), null);
  assert.equal(ruleBreaks("abc123"), null);
  assert.equal(ruleBreaks(""), null);
});
