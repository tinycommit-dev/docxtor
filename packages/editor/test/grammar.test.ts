// Docxtor's own grammar rules: what they flag, what they leave alone, and the offsets they report.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { checkText, ruleLanguage } from "../src/grammar/rules.ts";

/** The rule ids found in a text, in order. */
const ids = (text: string, lang = "en-US") => checkText(text, lang).map((i) => i.rule.id);
/** The words each issue covers. */
const covered = (text: string, lang = "en-US") => checkText(text, lang).map((i) => text.slice(i.from, i.to));
const fixes = (text: string, lang = "en-US") => checkText(text, lang).map((i) => i.replacements[0]);

/** Apply the first replacement of the first issue, as the right-click menu does. */
const fixed = (text: string, lang = "en-US") => {
  const i = checkText(text, lang)[0];
  return text.slice(0, i.from) + i.replacements[0] + text.slice(i.to);
};

test("languages: the four with rules, regions and underscores included", () => {
  assert.equal(ruleLanguage("en-US"), "en");
  assert.equal(ruleLanguage("es_MX"), "es");
  assert.equal(ruleLanguage("fr"), "fr");
  assert.equal(ruleLanguage("DE-at"), "de");
  assert.equal(ruleLanguage("ja-JP"), null);
  assert.deepEqual(checkText("the the cat", "ja-JP"), []);
  assert.deepEqual(checkText("", "en"), []);
});

test("repeated words", () => {
  assert.deepEqual(ids("This is is a test"), ["REPEATED_WORD"]);
  assert.deepEqual(covered("This is is a test"), ["is is"]);
  assert.deepEqual(fixes("This is is a test"), ["is"]);
  assert.equal(fixed("Go to the the store"), "Go to the store");
  // Capital letters differ, the word is the same.
  assert.deepEqual(covered("The the end"), ["The the"]);
  assert.equal(fixed("The the end"), "The end");
  // Legitimate repeats and different words.
  assert.deepEqual(ids("I know that that is true, and she had had enough. No no, bye bye."), []);
  assert.deepEqual(ids("one two two three"), ["REPEATED_WORD"]);
  assert.deepEqual(ids("It was the best of times, times were hard."), []);
  // Punctuation between them: not a repeat. Neither are one-letter words or numbers.
  assert.deepEqual(ids("Yes. Yes, that is it. Stop, stop."), []);
  assert.deepEqual(ids("Plan a a b b 10 10"), []);
  // Two spaces between them still count (and the extra space too).
  assert.deepEqual(ids("the  the cat").sort(), ["DOUBLE_SPACE", "REPEATED_WORD"]);
  // Objects (images, tabs) between words break the run.
  assert.deepEqual(ids("the ￼ the cat"), []);
  assert.deepEqual(ids("nous nous aimons", "fr-FR"), []);
  assert.deepEqual(ids("el el perro", "es-ES"), ["REPEATED_WORD"]);
  assert.deepEqual(ids("ich ich gehe", "de-DE"), ["REPEATED_WORD"]);
  assert.deepEqual(ids("Ich weiß, dass das das Haus ist", "de-DE"), []);
});

test("a and an", () => {
  assert.deepEqual(covered("I have a apple and an banana"), ["a", "an"]);
  assert.deepEqual(fixes("I have a apple and an banana"), ["an", "a"]);
  assert.equal(fixed("She is a honest person"), "She is an honest person");
  // Vowel letters with a consonant sound, and consonant letters with a vowel sound.
  assert.deepEqual(ids("It is an hour, a university, a one, a unit, a user, a European, a unique chance, a unanimous vote, a useful tool, a UFO"), []);
  assert.deepEqual(ids("an honest hour, an heir, an umbrella, an unhappy day, an uncle, an X-ray, an F, an MBA, an LED screen, an SQL query, an FBI agent, an 8-year-old"), []);
  assert.deepEqual(ids("a hotel, an hotel, a herb, an herb, a historic day, an historic day"), []);
  assert.deepEqual(covered("an university and a hour and an one and a umbrella and a 8-year-old and an European"), ["an", "a", "an", "a", "a", "an"]);
  assert.deepEqual(covered("a unidentified object and an unique idea"), ["a", "an"]);
  assert.deepEqual(ids("an unimportant detail, an uninteresting talk, a unidirectional flow"), []);
  // An initialism could be said either way: not flagged.
  assert.deepEqual(ids("a NASA mission, a MBA, an NBA fan, an USB cable, a LED"), []);
  // Case follows the article.
  assert.equal(fixed("A idea is born."), "An idea is born.");
  // Letters and plans are not articles: "Plan A is", "A and B", "a or b".
  assert.deepEqual(ids("Plan A is best. Choose A and B, or a or b. Vitamin A is good."), []);
  // A capital A starts a sentence.
  assert.deepEqual(ids("It rained. A umbrella helped."), ["A_BEFORE_VOWEL"]);
  // Numbers said with a vowel sound.
  assert.deepEqual(covered("a 8 percent rise, an 8 percent rise, a 11 year old, a 10 percent rise, a 1,800 fee"), ["a", "a"]);
  assert.deepEqual(ids("a apple", "es-ES"), []);
  assert.deepEqual(ids("an der Tür", "de-DE"), []);
});

test("sentences start with a capital letter", () => {
  assert.deepEqual(ids("It works. but it fails."), ["SENTENCE_CAPITAL"]);
  assert.deepEqual(covered("It works. but it fails."), ["but"]);
  assert.equal(fixed("It works. but it fails."), "It works. But it fails.");
  assert.deepEqual(ids("Really? yes! ok."), ["SENTENCE_CAPITAL", "SENTENCE_CAPITAL"]);
  assert.deepEqual(ids('He said. "then we left."'), ["SENTENCE_CAPITAL"]);
  // Abbreviations, initials, numbers, ellipses, quotes with a tag and the start of the paragraph are fine.
  assert.deepEqual(ids("Mr. smith, Dr. jones and Mrs. brown met J. edgar at 5 p.m. on tuesday, e.g. in May, i.e. soon, etc. and so on."), []);
  assert.deepEqual(ids("We waited... then left. Wait… and go."), []);
  assert.deepEqual(ids('"Stop!" he said. "Why?" she asked.'), []);
  assert.deepEqual(ids("see 3. apples and 4. pears"), []);
  assert.deepEqual(ids("lower case at the start is left alone"), []);
  assert.deepEqual(ids("Visit example.com. www.example.org is ours. iPhone sales rose. eBay did too."), []);
  assert.deepEqual(ids("Hola. adiós", "es-ES"), ["SENTENCE_CAPITAL"]);
  assert.deepEqual(ids("Das ist z. B. ein Haus, d. h. ein Gebäude, bzw. usw. mehr.", "de-DE"), []);
  assert.deepEqual(ids("C'est fini. mais non.", "fr-FR"), ["SENTENCE_CAPITAL"]);
});

test("the pronoun I", () => {
  assert.deepEqual(ids("i think so"), ["LOWERCASE_I"]);
  assert.deepEqual(covered("so do i, and i'm sure i’ve seen it. Then i will go"), ["i", "i", "i", "i"]);
  assert.equal(fixed("when i arrive"), "when I arrive");
  assert.deepEqual(ids("Section i. See ii. See i.e. that, and x = i + 1, i < n, i, j and k."), []);
  assert.deepEqual(ids("the radii, hi there, I think"), []);
  assert.deepEqual(ids("i think so", "es-ES"), []);
});

test("spaces", () => {
  assert.deepEqual(ids("Two  spaces here"), ["DOUBLE_SPACE"]);
  assert.deepEqual(fixes("Two   spaces here"), [" "]);
  assert.deepEqual(ids("  indented at the start and trailing   "), []);
  assert.deepEqual(ids("No  break spaces"), []);
  assert.deepEqual(ids("Hello , world ."), ["SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION"]);
  assert.equal(fixed("Hello , world"), "Hello, world");
  assert.deepEqual(ids("Really ? Yes ! Fine ; ok : done"), ["SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION"]);
  // Ellipses, smileys, file extensions and decimals are left alone.
  assert.deepEqual(ids("Well ... maybe :) and see .net and costs ,50"), []);
  assert.deepEqual(ids("wait ."), ["SPACE_BEFORE_PUNCTUATION"]);
  assert.deepEqual(checkText("Hello , world", "en")[0].kind, "style");
  // "word  ." is one problem, not two.
  assert.deepEqual(ids("word  ."), ["SPACE_BEFORE_PUNCTUATION"]);
});

test("French puts a space before : ; ? and !", () => {
  assert.deepEqual(ids("Vraiment ? Oui ! Voici : un exemple ; un autre.", "fr-FR"), []);
  assert.deepEqual(ids("Bonjour , monde .", "fr-FR"), ["SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION"]);
  assert.deepEqual(ids("Vraiment ? Oui !", "en-US"), ["SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION"]);
  assert.deepEqual(ids("Wirklich ? Ja !", "de-DE"), ["SPACE_BEFORE_PUNCTUATION", "SPACE_BEFORE_PUNCTUATION"]);
  // A space in front of « and » is French too, and quotes still balance.
  assert.deepEqual(ids("Il a dit « bonjour » à tous.", "fr-FR"), []);
  assert.deepEqual(ids("Il a dit « bonjour à tous.", "fr-FR"), ["UNMATCHED_OPENING_QUOTE"]);
});

test("a space after punctuation", () => {
  assert.deepEqual(ids("It ended.Then it began."), ["MISSING_SPACE"]);
  assert.deepEqual(covered("It ended.Then it began."), ["d.T"]);
  assert.equal(fixed("It ended.Then it began."), "It ended. Then it began.");
  assert.equal(fixed("Red,green and blue"), "Red, green and blue");
  assert.deepEqual(ids("Why?Because."), ["MISSING_SPACE"]);
  // Addresses, versions, code, abbreviations and lists of three.
  assert.deepEqual(ids("Go to example.com or www.Example.org, node.js, U.S.A, e.g.This, 3.14, Node.JS, a.b.c"), []);
  assert.deepEqual(ids("Mix red,green,blue and 1,000 and file.txt"), []);
  assert.deepEqual(ids("(see below.)"), []);
});

test("brackets", () => {
  assert.deepEqual(ids("A (fine) sentence [with] {all} kinds."), []);
  assert.deepEqual(ids("An (unclosed bracket"), ["UNMATCHED_OPENING"]);
  assert.deepEqual(covered("An (unclosed bracket"), ["("]);
  assert.deepEqual(ids("A stray) bracket"), ["UNMATCHED_CLOSING"]);
  // Crossed brackets: the one left open is named, and the closer nothing opened.
  assert.deepEqual(covered("Crossed (one [two) three]"), ["[", "]"]);
  assert.deepEqual(ids("Nested (one (two) three) and [a (b)]"), []);
  // Smileys and numbered lists close nothing.
  assert.deepEqual(ids("Thanks :) see you ;-) :( bye"), []);
  assert.deepEqual(ids("1) first"), []);
  assert.deepEqual(ids("a) one b) two"), ["UNMATCHED_CLOSING"]);
  assert.deepEqual(ids("It cost (see page 8) a lot"), []);
  assert.deepEqual(ids("Hola (mundo", "es-ES"), ["UNMATCHED_OPENING"]);
});

test("quotes", () => {
  assert.deepEqual(ids("He said “hello” and left."), []);
  assert.deepEqual(ids("He said “hello and left."), ["UNMATCHED_OPENING_QUOTE"]);
  assert.deepEqual(ids("He said hello” and left."), ["UNMATCHED_CLOSING_QUOTE"]);
  assert.deepEqual(ids('He said "hello and left.'), ["UNMATCHED_OPENING_QUOTE"]);
  assert.deepEqual(ids('He said "hello" and left.'), []);
  // A quotation that goes on into the next paragraph opens it without closing it.
  assert.deepEqual(ids("“The speech went on and on"), []);
  assert.deepEqual(ids('"The speech went on and on'), []);
  // Inches are not quotes.
  assert.deepEqual(ids('A 12" pipe and a 3" nail'), []);
  assert.deepEqual(ids("Er sagte „Hallo“ und ging.", "de-DE"), []);
  assert.deepEqual(ids("Er sagte „Hallo und ging.", "de-DE"), ["UNMATCHED_OPENING_QUOTE"]);
  assert.deepEqual(ids("Er sagte “Hallo” und ging.", "de-DE"), []);
  assert.deepEqual(ids("Dijo «hola» y se fue.", "es-ES"), []);
  assert.deepEqual(ids("Dijo «hola y se fue.", "es-ES"), ["UNMATCHED_OPENING_QUOTE"]);
});

test("words that are mixed up, only when certain", () => {
  assert.deepEqual(ids("You could of told me"), ["COULD_OF"]);
  assert.deepEqual(fixes("You could of told me"), ["could have"]);
  assert.equal(fixed("They should of left"), "They should have left");
  assert.equal(fixed("It would of been"), "It would have been");
  assert.deepEqual(ids("You could of course ask. A kind of magic."), []);
  assert.equal(fixed("I have alot of work"), "I have a lot of work");
  assert.equal(fixed("Alot happened"), "A lot happened");

  assert.equal(fixed("Its a nice day"), "It's a nice day");
  assert.equal(fixed("I think its been fine"), "I think it's been fine");
  assert.deepEqual(covered("its not here, and its going to rain"), ["its", "its"]);
  assert.deepEqual(ids("The dog wagged its tail. Its name is Rex. Its time zone is odd. Its true meaning is lost."), []);
  assert.equal(fixed("The dog has it's own bed"), "The dog has its own bed");
  assert.deepEqual(ids("It's a fine day and it's been long."), []);

  assert.equal(fixed("I don't want to loose weight"), "I don't want to lose weight");
  assert.equal(fixed("you will loose the game"), "you will lose the game");
  assert.deepEqual(ids("The knot is loose. Cut it loose. Wear loose clothes. A loose tooth."), []);
  assert.equal(fixed("a lose fitting shirt"), "a loose fitting shirt");
  assert.deepEqual(ids("a win-lose game, lose-lose"), []);

  assert.equal(fixed("It is better then nothing"), "It is better than nothing");
  assert.equal(fixed("more then once"), "more than once");
  assert.deepEqual(ids("First this, then that. Back then it was better. And then some."), []);
  assert.deepEqual(ids("could of", "es-ES"), []);
});

test("Spanish questions and exclamations open with an inverted mark", () => {
  assert.deepEqual(ids("¿Cómo estás?", "es-ES"), []);
  assert.deepEqual(ids("Cómo estás?", "es-ES"), ["MISSING_INVERTED_QUESTION"]);
  assert.deepEqual(covered("Cómo estás?", "es-ES"), ["?"]);
  assert.deepEqual(ids("Qué bien!", "es-ES"), ["MISSING_INVERTED_EXCLAMATION"]);
  assert.deepEqual(ids("¡Qué bien! ¿Y tú? Sí, claro.", "es-ES"), []);
  assert.deepEqual(ids("¿Qué quieres? ¡Dímelo!", "es-ES"), []);
  assert.deepEqual(ids("Si no estás seguro, ¿cómo vas a elegir?", "es-ES"), []);
  assert.deepEqual(ids("Visita https://example.com/?q=1 ahora", "es-ES"), []);
  assert.deepEqual(ids("¿Qué?! Sí", "es-ES"), []);
  assert.deepEqual(ids("Cómo estás?", "en-US"), []);
});

test("reading order, offsets and kinds", () => {
  const text = "i think that the the answer is alot better then a apple.";
  const issues = checkText(text, "en");
  const from = issues.map((i) => i.from);
  assert.deepEqual(from, [...from].sort((a, b) => a - b));
  for (const i of issues) {
    assert.ok(i.from >= 0 && i.to <= text.length && i.from < i.to, JSON.stringify(i));
    assert.ok(i.message.length > 0 && i.rule.id.length > 0 && i.rule.category.length > 0);
  }
  assert.deepEqual(issues.map((i) => i.rule.id), ["LOWERCASE_I", "REPEATED_WORD", "ALOT", "THEN_THAN", "A_BEFORE_VOWEL"]);
  // Surrogate pairs count as two in the offsets, like the model's.
  const emoji = "😀 the the end";
  const [rep] = checkText(emoji, "en");
  assert.equal(emoji.slice(rep.from, rep.to), "the the");
  assert.equal(rep.from, 3);
});

test("ordinary prose is left alone", () => {
  const prose = [
    "The quick brown fox jumps over the lazy dog. It was a sunny day, and an owl watched from an oak tree.",
    "“We should leave,” she said. “Why?” he asked. They left at 5 p.m. on Tuesday (it rained), and nobody minded.",
    "Dr. Smith's report (see Figure 3) shows a 12% rise: more than expected, less than hoped.",
    "Visit https://example.com/path?x=1 or write to me@example.org for details.",
    "She said it's fine; its colour is lovely and its edges are smooth.",
  ];
  for (const p of prose) assert.deepEqual(checkText(p, "en-US"), [], p);
  assert.deepEqual(checkText("Voilà un texte simple : il n'y a rien à signaler ! N'est-ce pas ?", "fr-FR"), []);
  assert.deepEqual(checkText("Das ist ein einfacher Text. Er enthält keine Fehler, denn „alles“ ist in Ordnung.", "de-DE"), []);
  assert.deepEqual(checkText("Este es un texto sencillo. ¿Hay algún error? ¡No, ninguno!", "es-ES"), []);
});
