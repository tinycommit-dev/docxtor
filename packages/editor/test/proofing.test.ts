// The parts of Proofing options that are plain functions: telling a paragraph's language, the custom dictionaries,
// readability statistics, and the frequently confused words.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { checkText } from "../src/grammar/rules.ts";
import { accentedCapitals } from "../src/spelling/accents.ts";
import { detectByWords } from "../src/spelling/detect.ts";
import { CustomDictionaries, editDistance, forLanguage, loadDictionaries } from "../src/spelling/dictionaries.ts";
import { isPassive, readability, sentencesOf, syllables } from "../src/spelling/readability.ts";

test("the word detector tells seven languages apart, and says nothing about short or wordless text", () => {
  const samples: [string, string][] = [
    ["The committee will review the proposal and send their answer to the board by the end of the week.", "en"],
    ["El comité revisará la propuesta y enviará su respuesta a la junta antes del final de la semana.", "es"],
    ["Le comité examinera la proposition et enverra sa réponse au conseil avant la fin de la semaine.", "fr"],
    ["Der Ausschuss wird den Vorschlag prüfen und seine Antwort bis Ende der Woche an den Vorstand senden.", "de"],
    ["Il comitato esaminerà la proposta e invierà la sua risposta al consiglio entro la fine della settimana.", "it"],
    ["O comitê vai analisar a proposta e enviar a sua resposta ao conselho até o fim da semana.", "pt"],
    ["De commissie zal het voorstel bekijken en haar antwoord voor het einde van de week naar het bestuur sturen.", "nl"],
  ];
  for (const [text, lang] of samples) {
    const found = detectByWords(text);
    assert.equal(found?.lang, lang, text);
    assert.ok(found!.confidence >= 0.6, `${lang} is sure enough`);
  }
  assert.equal(detectByWords("Hello world"), null);
  assert.equal(detectByWords("Quarterly revenue grew twelve percent year over year"), null);
});

test("custom dictionaries: the old personal list becomes the default one, a language-only one counts only for its language, off counts for none", () => {
  let saved: unknown = null;
  const d = new CustomDictionaries(loadDictionaries(null, ["Docxtor"], "Personal"), (s) => (saved = s), "Personal");
  assert.deepEqual(d.all().map((x) => x.name), ["Personal"]);
  assert.ok(d.has("docxtor", "en-US"));
  const fr = d.create("French words", "fr");
  d.addWord("glorbe", fr.id);
  assert.ok(d.has("Glorbe", "fr-CA"));
  assert.ok(!d.has("glorbe", "en"));
  d.update(fr.id, { on: false });
  assert.ok(!d.has("glorbe", "fr"));
  // Kept as it is now, and read back the same.
  const back = loadDictionaries(saved, null, "Personal");
  assert.deepEqual(back.list.map((x) => [x.name, x.lang, x.on, x.words]), [["Personal", "", true, ["Docxtor"]], ["French words", "fr", false, ["glorbe"]]]);
  // The default goes to the first left when it is removed; the last can't be removed.
  d.setDefault(fr.id);
  d.remove(fr.id);
  assert.equal(d.defaultId, "personal");
  d.remove("personal");
  assert.equal(d.all().length, 1);
});

test("words close to a misspelling are suggested from the custom dictionaries", () => {
  const d = new CustomDictionaries(loadDictionaries(null, ["Zorglex", "Docxtor", "Quillon"], "Personal"), () => {}, "Personal");
  assert.deepEqual(d.similar("zorgle", "en"), ["Zorglex"]);
  assert.deepEqual(d.similar("Docxtr", "en"), ["Docxtor"]);
  assert.deepEqual(d.similar("banana", "en"), []);
  assert.equal(editDistance("recieve", "receive"), 1);
  assert.ok(forLanguage("en", "en-GB") && !forLanguage("en-GB", "en-US") && forLanguage("", "ja"));
});

test("readability: sentences, syllables, passives and the Flesch scores", () => {
  assert.deepEqual(sentencesOf("Dr. Smith arrived at 5 p.m. today. He was greeted by the staff! Was it fun?"), ["Dr. Smith arrived at 5 p.m. today.", "He was greeted by the staff!", "Was it fun?"]);
  assert.deepEqual(["cat", "table", "beautiful", "readability"].map(syllables), [1, 2, 3, 5]);
  assert.ok(isPassive("The report was written by the team."));
  assert.ok(!isPassive("The team wrote the report."));
  const r = readability("The cat sat on the mat. The report was written by the team.\nShort one.", "en-US");
  assert.equal(r.words, 15);
  assert.equal(r.sentences, 3);
  assert.equal(r.paragraphs, 2);
  assert.equal(r.english!.passive, 33);
  assert.ok(r.english!.readingEase > 80);
  assert.equal(readability("El gato duerme.", "es").english, null);
});

test("frequently confused words are flagged in English, and the option turns them off", () => {
  const ids = (text: string, opts = {}) => checkText(text, "en", opts).map((i) => i.rule.id);
  assert.ok(ids("Your welcome to stay.").includes("YOUR_YOURE"));
  assert.ok(ids("Their is a problem.").includes("THEIR_THERE"));
  assert.ok(ids("It costs to much.").includes("TO_TOO"));
  assert.ok(!ids("I sent it to many people.").includes("TO_TOO"));
  assert.ok(ids("I wonder weather or not it rains.").includes("WEATHER_WHETHER"));
  assert.ok(ids("The affect of the drug was strong.").includes("AFFECT_EFFECT"));
  assert.ok(!ids("She has a flat affect and a calm voice.").includes("AFFECT_EFFECT"));
  assert.deepEqual(ids("Your welcome.", { confusedWords: false }), []);
  assert.ok(ids("Thanks! Your welcome.").includes("YOUR_YOURE"));
  assert.ok(ids("I think your going to like it.").includes("YOUR_YOURE"));
  // Right as they are.
  for (const ok of [
    "Don't overstay your welcome.",
    "Check your welcome email for the link.",
    "It doesn't amount to much.",
    "The result is known to many.",
    "A society that is more just than ours.",
    "I would rather go back than stay.",
    "I wonder whose that is.",
  ])
    assert.deepEqual(ids(ok).filter((id) => ["YOUR_YOURE", "TO_TOO", "THAN_THEN", "WHOSE_WHOS"].includes(id)), [], ok);
  assert.ok(ids("There are to many to count.").includes("TO_TOO"));
  assert.ok(ids("It is to late.").includes("TO_TOO"));
});

test("French capitals: the accented forms a word in capitals could stand for", () => {
  assert.ok(accentedCapitals("ECOLE").includes("ÉCOLE"));
  assert.ok(accentedCapitals("Ecole").includes("École"));
  assert.ok(!accentedCapitals("ECOLE").includes("ECOLE"));
  assert.deepEqual(accentedCapitals("xyz"), []);
  assert.ok(accentedCapitals("ACCELERATEUR").length <= 400);
});

test("the word detector doesn't take English with a few Dutch-looking words for Dutch", () => {
  assert.equal(detectByWords("He met Dan in Boston. She was in Paris. It is of note.")?.lang ?? "en", "en");
});
