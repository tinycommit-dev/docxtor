// Docxtor's own grammar rules: plain functions from a paragraph's text to the problems in it. No network,
// no state, no DOM; the same text gives the same answer. Positions are UTF-16 offsets into the text, as
// everywhere in the model, and an object (image, tab, note mark) is U+FFFC.
//
// The rules are few and careful. A rule is here only when it is right nearly every time; a checker that
// cries wolf teaches people to ignore it. English has the most; Spanish, French and German get the rules
// that do not depend on the language, plus the ones that are certain for them. French puts a space
// before : ; ? and !, so the spacing rule leaves those alone there.
import { t } from "@docxtor/extensions";

export interface RuleIssue {
  from: number;
  to: number;
  message: string;
  shortMessage?: string;
  replacements: string[];
  rule: { id: string; description: string; category: string };
  /** `style` for hints about spacing; `grammar` for the rest. */
  kind: "grammar" | "style";
}

export type Language = "en" | "es" | "fr" | "de";

/** The languages with rules. */
export const LANGUAGES: Language[] = ["en", "es", "fr", "de"];

/** `en-US`, `es_MX`, `fr` become the language the rules know, or null for any other. */
export function ruleLanguage(lang: string): Language | null {
  const base = lang.trim().toLowerCase().split(/[-_]/)[0] as Language;
  return LANGUAGES.includes(base) ? base : null;
}

/** Check one paragraph's text. Issues come back in reading order. */
export function checkText(text: string, lang: string, options: { repeatedWords?: boolean; confusedWords?: boolean } = {}): RuleIssue[] {
  const L = ruleLanguage(lang);
  if (!L || text.length < 2) return [];
  const out: RuleIssue[] = [];
  if (options.repeatedWords !== false) repeatedWords(text, L, out);
  doubleSpaces(text, out);
  spaceBeforePunctuation(text, L, out);
  missingSpace(text, out);
  sentenceCapital(text, L, out);
  brackets(text, out);
  quotes(text, L, out);
  if (L === "en") {
    articles(text, out);
    capitalI(text, out);
    if (options.confusedWords !== false) confusions(text, out);
  }
  if (L === "es") invertedMarks(text, out);
  return out.sort((a, b) => a.from - b.from || a.to - b.to);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SPACES = "[ \\u00a0]";
/** Not inside a longer word: the characters that can join a word to its neighbour. */
const BEFORE = "(?<![\\p{L}\\p{N}_'’-])";
const AFTER = "(?![\\p{L}\\p{N}_-])";

function issue(out: RuleIssue[], from: number, to: number, id: string, category: string, description: string, message: string, replacements: string[], kind: "grammar" | "style" = "grammar") {
  out.push({ from, to, message, replacements, rule: { id, description, category }, kind });
}

/** Give `word` the capital letter that `like` has. */
function matchCase(word: string, like: string): string {
  if (!like) return word;
  const first = like[0];
  const upper = first !== first.toLowerCase() && first === first.toUpperCase();
  return upper ? word[0].toUpperCase() + word.slice(1) : word;
}

const isUpper = (s: string) => s.length > 0 && s === s.toUpperCase() && s !== s.toLowerCase();

// ---------------------------------------------------------------------------
// Repeated words
// ---------------------------------------------------------------------------

const WORD = /[\p{L}\p{M}]+(?:['’][\p{L}\p{M}]+)*/gu;

/** Words that are right when written twice. */
const MAY_REPEAT: Record<Language, Set<string>> = {
  en: new Set(["had", "that", "no", "bye", "so", "very", "really", "there", "ha", "yes", "well", "blah", "knock", "go", "now", "boom"]),
  es: new Set(["que", "no", "ya", "ha", "ja", "muy", "la", "si", "sí", "bien"]),
  fr: new Set(["nous", "vous", "que", "si", "non", "ha", "ah", "oh", "tout", "très", "bien", "bye"]),
  de: new Set(["das", "die", "der", "sie", "wir", "ihr", "ja", "nein", "so", "ha"]),
};

function repeatedWords(text: string, L: Language, out: RuleIssue[]) {
  let prev: RegExpExecArray | null = null;
  for (const m of text.matchAll(WORD)) {
    if (prev && prev[0].length > 1 && /^[  ]+$/.test(text.slice(prev.index + prev[0].length, m.index)) && prev[0].toLowerCase() === m[0].toLowerCase() && !MAY_REPEAT[L].has(m[0].toLowerCase())) {
      const from = prev.index;
      const to = m.index + m[0].length;
      issue(out, from, to, "REPEATED_WORD", t("Repeated words"), t("The same word twice in a row"), t("Repeated word: “{word}”", { word: prev[0] }), [prev[0]]);
    }
    prev = m;
  }
}

// ---------------------------------------------------------------------------
// Spacing
// ---------------------------------------------------------------------------

function doubleSpaces(text: string, out: RuleIssue[]) {
  for (const m of text.matchAll(/(?<=[^\s]) {2,}(?=[^\s,.;:!?])/g)) {
    issue(out, m.index, m.index + m[0].length, "DOUBLE_SPACE", t("Spacing"), t("More than one space in a row"), t("Extra space"), [" "], "style");
  }
}

function spaceBeforePunctuation(text: string, L: Language, out: RuleIssue[]) {
  // French puts a space before these four.
  const allowed = L === "fr" ? ":;?!" : "";
  for (const m of text.matchAll(/(?<=[\p{L}\p{N})\]"”’'»]) +([,.;:!?])/gu)) {
    const p = m[1];
    const at = m.index + m[0].length;
    if (allowed.includes(p)) continue;
    const next = text[at];
    // "wait ..." and "see .com", ":)" and "costs ,50" are not mistakes.
    if (p === "." && next === ".") continue;
    if (next !== undefined && /[\p{L}\p{N}]/u.test(next)) continue;
    if ((p === ":" || p === ";") && next !== undefined && /[)(\\/|-]/.test(next)) continue;
    issue(out, m.index, at, "SPACE_BEFORE_PUNCTUATION", t("Spacing"), t("A space before a punctuation mark"), t("Remove the space before “{mark}”", { mark: p }), [p], "style");
  }
}

/** Two words run together by a full stop, question mark or exclamation mark, or by a comma or semicolon. */
function missingSpace(text: string, out: RuleIssue[]) {
  for (const m of text.matchAll(/\S+/g)) {
    const token = m[0].replace(/^[("“‘'\[¿¡«„]+|[.,;:!?)"”’'\]»]+$/gu, "");
    const found = /^([\p{L}'’-]{2,})([.!?])(\p{Lu}\p{Ll}[\p{Ll}'’-]*)$/u.exec(token) ?? /^([\p{L}'’-]{2,})([,;])([\p{L}'’-]{2,})$/u.exec(token);
    if (!found || !/\p{Ll}$/u.test(found[1])) continue;
    const start = m.index + m[0].indexOf(token);
    const at = start + found[1].length;
    const before = text[at - 1];
    const after = text[at + 1];
    issue(
      out,
      at - 1,
      at + 2,
      "MISSING_SPACE",
      t("Spacing"),
      t("No space after punctuation"),
      t("Add a space after “{mark}”", { mark: text[at] }),
      [`${before}${text[at]} ${after}`],
    );
  }
}

// ---------------------------------------------------------------------------
// Capital letters
// ---------------------------------------------------------------------------

/** Abbreviations that end in a full stop in the middle of a sentence. */
const ABBREVIATIONS = new Set([
  // English
  "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "inc", "ltd", "co", "corp", "no", "nos", "fig", "figs", "eq", "eqs", "vol", "vols", "ch", "sec", "pp", "ed", "eds", "al", "cf", "ca", "approx", "dept", "est", "gen", "gov", "hon", "lt", "col", "capt", "sgt", "rev", "mt", "ft", "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "sept", "oct", "nov", "dec",
  // Spanish
  "sra", "srta", "dra", "ud", "uds", "pág", "págs", "núm", "cap", "ej", "aprox", "av", "avda", "tel",
  // French
  "mme", "mlle", "pr", "env", "bd", "ex",
  // German
  "usw", "bzw", "vgl", "nr", "str", "evtl", "ggf", "inkl", "max", "min", "mrd", "mio", "abb", "hrsg",
]);

/** A lower-case letter that starts a sentence. */
function sentenceCapital(text: string, _L: Language, out: RuleIssue[]) {
  for (const m of text.matchAll(/([.!?])(\s+)["“‘'(\[«¿¡„]*(\p{Ll})/gu)) {
    const punct = m.index;
    const wordStart = m.index + m[0].length - m[3].length;
    if (text[punct - 1] === "." || text[punct - 1] === "…") continue;
    if (m[1] === ".") {
      const before = /(\S*)$/.exec(text.slice(0, punct))![1];
      const bare = before.replace(/^[("“‘'\[¿¡«„]+/u, "");
      // Initials, numbers, "e.g." and "Dr.": the sentence goes on.
      if (!bare || /[\p{N}.]/u.test(bare) || /^\p{L}$/u.test(bare)) continue;
      if (ABBREVIATIONS.has(bare.toLowerCase()) && (isUpper(bare[0]) || bare.length > 2)) continue;
      // "Dr", "St", "Al": a capital and at most one more letter.
      if (/^\p{Lu}\p{Ll}?$/u.test(bare)) continue;
    }
    const word = /^\S+/.exec(text.slice(wordStart))![0].replace(/[.,;:!?)"”’'\]»…]+$/u, "");
    if (!/^\p{Ll}[\p{Ll}'’-]*$/u.test(word)) continue;
    issue(out, wordStart, wordStart + word.length, "SENTENCE_CAPITAL", t("Capital letters"), t("A sentence starts with a capital letter"), t("Start the sentence with a capital letter"), [matchCase(word, "A")]);
  }
}

/** The pronoun "I" written in lower case. */
function capitalI(text: string, out: RuleIssue[]) {
  const code = /[=<>+*/^%[({\\|&~]/;
  for (const m of text.matchAll(/(?<![\p{L}\p{N}_.'’/@\\-])i(?=$|[\s,;!?]|['’](?:m|ll|ve|d)(?![\p{L}]))/gu)) {
    const i = m.index;
    const before = text.slice(0, i).trimEnd();
    const after = text.slice(i + 1).trimStart();
    // Code and sums: "i = 0", "i < n". Lists of letters: "i, j, k".
    if (code.test(before.slice(-1)) || code.test(after[0] ?? "") || /^-\s/.test(after)) continue;
    const prevWord = /(\S+)$/.exec(before)?.[1] ?? "";
    const nextWord = /^\S+/.exec(after)?.[0] ?? "";
    if (/^[a-z][,;:]?$/i.test(prevWord) && prevWord.toLowerCase() !== "a") continue;
    if (/^[a-z][,;:]?$/i.test(nextWord) && nextWord.toLowerCase() !== "a") continue;
    issue(out, i, i + 1, "LOWERCASE_I", t("Capital letters"), t("The word “I” is always a capital"), t("Write “I” with a capital letter"), ["I"]);
  }
}

// ---------------------------------------------------------------------------
// a and an
// ---------------------------------------------------------------------------

/** Words that cannot mean the letter or an article: "a and b", "a is 3". */
const NOT_A_NOUN = new Set(["and", "or", "is", "are", "was", "were", "as", "at", "if", "in", "on", "of", "it", "be", "by", "not", "am", "to"]);
/** Letters whose name starts with a vowel sound: "an F", "an X-ray". */
const VOWEL_NAMES = "aefhilmnorsx";
/** Words that start with a vowel letter and a consonant sound ("a university"), or the other way round. */
const CONSONANT_SOUND = /^(uni(?!n|mp|mag|dent)|unanim|unesco|use|usu|usa|uti|uto|ute|uri|ura|uro|ure|uku|ukr|uvu|ufo|ubi|eu|ewe|once?$|ones?$|oneself$)/;
const VOWEL_SOUND_U = /^(un(?!i|anim|esco)|uni(?=n|mp|mag|dent)|up|ug|ul|um|ud|uh|us(?![aeu])|ut(?![ioea])|ur(?![iaoe]))/;
const SILENT_H = /^(hour|honest|honou?r|heir)/;
/** Words where "a" and "an" are both heard (an herb, a herb; an historic, a historic). */
const EITHER_H = /^(herb|histor|hotel|homage|hallelu|hyster|hospital)/;

/** Whether `word` starts with a vowel sound: true, false, or null when it can't be told. */
function vowelSound(raw: string): boolean | null {
  if (/^\d/.test(raw)) {
    const digits = /^[\d,.]+/.exec(raw)![0].replace(/,/g, "");
    return digits.startsWith("8") || digits === "11" || digits === "18" ? true : null;
  }
  const word = /^\p{L}+/u.exec(raw)?.[0];
  if (!word) return null;
  // MBA, NASA, FBI: said by letter or as a word, there is no telling.
  if (word.length > 1 && isUpper(word)) return null;
  if (word.length === 1) {
    const letter = word.toLowerCase();
    return letter !== "u" && VOWEL_NAMES.includes(letter);
  }
  const w = word.toLowerCase();
  if (SILENT_H.test(w)) return true;
  if (w[0] === "h") return EITHER_H.test(w) ? null : false;
  if (CONSONANT_SOUND.test(w)) return false;
  if (w[0] === "u") return VOWEL_SOUND_U.test(w) ? true : null;
  return "aeiou".includes(w[0]);
}

function articles(text: string, out: RuleIssue[]) {
  for (const m of text.matchAll(new RegExp(`${BEFORE}(an?)(?![\\p{L}\\p{N}_'’-])${SPACES}+(?=[\\p{L}\\p{N}])`, "giu"))) {
    const article = m[1];
    const end = m.index + m[0].length;
    // "A" and "An" with a capital are only articles at the start of a sentence ("Plan A is best").
    if (article[0] === article[0].toUpperCase()) {
      const before = text.slice(0, m.index).trimEnd();
      if (before && !/[.!?:]["”’')]?$|^["“‘'(]$/u.test(before)) continue;
    }
    const raw = /^[\p{L}\p{N}][\p{L}\p{N}'’.,%-]*/u.exec(text.slice(end))![0];
    // "a a", "a b": letters of a list or a formula, not words.
    if (/^\p{Ll}(?![\p{L}\p{N}'’])/u.test(raw)) continue;
    if (NOT_A_NOUN.has(raw.toLowerCase().replace(/[^\p{L}].*$/u, ""))) continue;
    const vowel = vowelSound(raw);
    if (vowel === null) continue;
    const isA = article.toLowerCase() === "a";
    if (isA === vowel) {
      const fixed = isA ? matchCase("an", article) : matchCase("a", article);
      issue(
        out,
        m.index,
        m.index + article.length,
        isA ? "A_BEFORE_VOWEL" : "AN_BEFORE_CONSONANT",
        t("Articles"),
        t("Use “a” before a consonant sound and “an” before a vowel sound"),
        isA ? t("Use “an” before a vowel sound") : t("Use “a” before a consonant sound"),
        [fixed],
      );
    }
  }
}

// ---------------------------------------------------------------------------
// Words that are often mixed up, only where the sentence leaves no doubt
// ---------------------------------------------------------------------------

function confusions(text: string, out: RuleIssue[]) {
  const apostrophe = text.includes("’") ? "’" : "'";
  const re = (source: string) => new RegExp(source, "giu");
  const category = t("Commonly confused words");

  // "could of" for "could have". "could of course" is fine.
  for (const m of text.matchAll(re(`${BEFORE}(could|should|would|must|might)${SPACES}+of${AFTER}(?!${SPACES}+course)`))) {
    const fixed = `${m[1]} have`;
    issue(out, m.index, m.index + m[0].length, "COULD_OF", category, t("“Could of” should be “could have”"), t("Did you mean “{phrase}”?", { phrase: fixed }), [fixed]);
  }

  for (const m of text.matchAll(re(`${BEFORE}alot${AFTER}`))) {
    issue(out, m.index, m.index + m[0].length, "ALOT", category, t("“A lot” is two words"), t("Did you mean “a lot”?"), [matchCase("a lot", m[0])]);
  }

  // "its a", "its been": a possessive is never followed by an article or "been".
  for (const m of text.matchAll(re(`${BEFORE}its(?=${SPACES}+(?:an?|the|been|not|also|never|always|really|very|because|going${SPACES}+to)${AFTER})`))) {
    issue(out, m.index, m.index + 3, "ITS_ITS", category, t("“It’s” is “it is” or “it has”"), t("Did you mean “it’s” (it is or it has)?"), [matchCase(`it${apostrophe}s`, m[0])]);
  }
  for (const m of text.matchAll(re(`${BEFORE}it['’]s(?=${SPACES}+own${AFTER})`))) {
    issue(out, m.index, m.index + m[0].length, "ITS_OWN", category, t("“Its” without an apostrophe shows ownership"), t("Did you mean “its”?"), [matchCase("its", m[0])]);
  }

  // "to loose weight": after "to", "will", "don't" and the like, the verb is "lose".
  const helpers = "to|will|would|can|could|might|may|should|must|don['’]t|won['’]t|doesn['’]t|didn['’]t|never|not|gonna";
  const objects = "the|my|your|our|his|her|their|a|an|weight|money|track|sight|hope|faith|touch|control|patience|interest|time|face|it|them|you|him|me|us|this|that";
  for (const m of text.matchAll(re(`${BEFORE}(?:${helpers})${SPACES}+(loose)(?=${SPACES}+(?:${objects})${AFTER})`))) {
    const at = m.index + m[0].length - 5;
    issue(out, at, at + 5, "LOOSE_LOSE", category, t("“Lose” means to not win or to misplace"), t("Did you mean “lose”?"), [matchCase("lose", "loose")]);
  }
  for (const m of text.matchAll(re(`${BEFORE}(lose)(?=[ -]fitting${AFTER})`))) {
    issue(out, m.index, m.index + 4, "LOSE_LOOSE", category, t("“Loose” means not tight"), t("Did you mean “loose”?"), [matchCase("loose", m[1])]);
  }

  // "your going", and "Your welcome." or "Your welcome to stay." starting a sentence: "you're" is "you are".
  // "Overstay your welcome" and "your welcome email" are right, so "welcome" counts only at the start.
  const yourWelcome = `(?<=(?:^|[.!?]["”’)]?${SPACES}+))(your)(?=${SPACES}+welcome(?:${SPACES}*(?:[.!,;]|$)|${SPACES}+to${SPACES}+(?!(?:the|a|an|our|my|your|his|her|their|this|that)${AFTER})))`;
  for (const m of text.matchAll(re(`${BEFORE}(?:(your)(?=${SPACES}+(?:going|doing|being|getting|coming|trying|not)${AFTER})|${yourWelcome})`))) {
    issue(out, m.index, m.index + 4, "YOUR_YOURE", category, t("“You’re” is “you are”"), t("Did you mean “you’re”?"), [matchCase(`you${apostrophe}re`, m[1] ?? m[2])]);
  }
  // "you're own", "they're own", "there own": ownership is "your" and "their".
  for (const m of text.matchAll(re(`${BEFORE}(you['’]re)(?=${SPACES}+own${AFTER})`))) {
    issue(out, m.index, m.index + m[1].length, "YOURE_YOUR", category, t("“Your” shows ownership"), t("Did you mean “your”?"), [matchCase("your", m[1])]);
  }
  for (const m of text.matchAll(re(`${BEFORE}(they['’]re|there)(?=${SPACES}+own${AFTER})`))) {
    issue(out, m.index, m.index + m[1].length, "THERE_THEIR", category, t("“Their” shows ownership"), t("Did you mean “their”?"), [matchCase("their", m[1])]);
  }
  // "their is", "their are": a place or a fact is "there".
  for (const m of text.matchAll(re(`${BEFORE}(their)(?=${SPACES}+(?:is|are|was|were|has${SPACES}+been|have${SPACES}+been)${AFTER})`))) {
    issue(out, m.index, m.index + 5, "THEIR_THERE", category, t("“There is” and “there are” use “there”"), t("Did you mean “there”?"), [matchCase("there", m[1])]);
  }
  // "to much", "to many", "to late": "too" means more than enough. Only at the end of a clause ("it costs to much.")
  // or before "to" or "for": "sent to many people" is right, and so are "amount to much." and "known to many." after a
  // word that takes "to".
  const takesTo = "amounts?|amounted|known|unknown|familiar|available|open|appl(?:y|ies|ied)|appeals?|appealed|belongs?|mean(?:s|t)?|matters?|mattered|happens?|happened|speaks?|spoke|talks?|talked|sells?|sold|gives?|gave|given|lends?|lent|owes?|owed|due|common|useful|visible|dear|similar|new|grateful|thanks";
  for (const m of text.matchAll(re(`${BEFORE}(?<!(?:${takesTo})${SPACES}+)(to)(?=${SPACES}+(?:(?:much|many|late)(?:${SPACES}*[.,;:!?]|${SPACES}*$)|(?:much|many|late)${SPACES}+(?:to|for)${AFTER}))`))) {
    issue(out, m.index, m.index + 2, "TO_TOO", category, t("“Too” means more than enough"), t("Did you mean “too”?"), [matchCase("too", m[1])]);
  }
  // "weather or not": the choice is "whether".
  for (const m of text.matchAll(re(`${BEFORE}(weather)(?=${SPACES}+or${SPACES}+not${AFTER})`))) {
    issue(out, m.index, m.index + 7, "WEATHER_WHETHER", category, t("“Whether” introduces a choice"), t("Did you mean “whether”?"), [matchCase("whether", m[1])]);
  }
  // "whose going": "who's" is "who is" or "who has".
  for (const m of text.matchAll(re(`${BEFORE}(whose)(?=${SPACES}+(?:going|coming|been)${AFTER})`))) {
    issue(out, m.index, m.index + 5, "WHOSE_WHOS", category, t("“Who’s” is “who is” or “who has”"), t("Did you mean “who’s”?"), [matchCase(`who${apostrophe}s`, m[1])]);
  }
  // "an affect", "the affect of": the result is an "effect".
  for (const m of text.matchAll(re(`${BEFORE}(?:an|the)${SPACES}+(affects?)(?=${SPACES}+(?:of|on)${AFTER}|${SPACES}*[.,;:!?]|$)`))) {
    const at = m.index + m[0].length - m[1].length;
    const fixed = m[1].toLowerCase().endsWith("s") ? "effects" : "effect";
    issue(out, at, at + m[1].length, "AFFECT_EFFECT", category, t("The noun is usually “effect”"), t("Did you mean “{phrase}”?", { phrase: fixed }), [matchCase(fixed, m[1])]);
  }
  // "suppose to": the past participle, "supposed to".
  for (const m of text.matchAll(re(`${BEFORE}(?:is|was|are|were|am|be|been|being|not)${SPACES}+(suppose)(?=${SPACES}+to${AFTER})`))) {
    const at = m.index + m[0].length - 7;
    issue(out, at, at + 7, "SUPPOSE_SUPPOSED", category, t("Use “supposed to”"), t("Did you mean “supposed”?"), [matchCase("supposed", m[1])]);
  }
  // "and than", "since than": the time word is "then" ("more just than", "rather go back than" are comparisons).
  for (const m of text.matchAll(re(`${BEFORE}(?:and|since|until)${SPACES}+(than)${AFTER}`))) {
    const at = m.index + m[0].length - 4;
    issue(out, at, at + 4, "THAN_THEN", category, t("“Then” is about time or order"), t("Did you mean “then”?"), [matchCase("then", m[1])]);
  }

  // "better then", "more then": a comparison takes "than".
  for (const m of text.matchAll(re(`${BEFORE}(?:more|less|better|worse|rather|other|greater|bigger|smaller|faster|slower|higher|lower|larger|older|younger|easier|harder)${SPACES}+(then)${AFTER}`))) {
    const at = m.index + m[0].length - 4;
    issue(out, at, at + 4, "THEN_THAN", category, t("Use “than” to compare"), t("Did you mean “than”?"), [matchCase("than", m[1])]);
  }
}

// ---------------------------------------------------------------------------
// Brackets and quotes
// ---------------------------------------------------------------------------

const PAIRS: Record<string, string> = { "(": ")", "[": "]", "{": "}" };
const CLOSERS = new Set(Object.values(PAIRS));

function brackets(text: string, out: RuleIssue[]) {
  const category = t("Punctuation");
  const stack: { char: string; at: number }[] = [];
  const emoticon = (i: number) => /[:;=8]-?$/.test(text.slice(Math.max(0, i - 2), i));
  const listMarker = (i: number) => /^\s*[\p{L}\p{N}]{1,3}$/u.test(text.slice(0, i));
  const closing = (char: string, at: number) =>
    issue(out, at, at + 1, "UNMATCHED_CLOSING", category, t("A closing bracket without an opening one"), t("Missing opening “{mark}”", { mark: Object.keys(PAIRS).find((k) => PAIRS[k] === char)! }), []);
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c in PAIRS) {
      if (!(c === "(" && emoticon(i))) stack.push({ char: c, at: i });
    } else if (CLOSERS.has(c)) {
      const k = stack.map((s) => PAIRS[s.char]).lastIndexOf(c);
      if (k < 0) {
        // A smiley or a numbered list ("1)", "a)") closes nothing.
        if (!(c === ")" && (emoticon(i) || listMarker(i)))) closing(c, i);
        continue;
      }
      // Anything opened inside it and not closed is missing its closer.
      for (const s of stack.splice(k)) if (PAIRS[s.char] !== c) unclosed(s);
    }
  }
  for (const s of stack) unclosed(s);
  function unclosed(s: { char: string; at: number }) {
    issue(out, s.at, s.at + 1, "UNMATCHED_OPENING", category, t("An opening bracket without a closing one"), t("Missing closing “{mark}”", { mark: PAIRS[s.char] }), []);
  }
}

/** Opening and closing quotation marks by language. */
const QUOTES: Record<Language, { open: string; close: string }> = {
  en: { open: "“", close: "”" },
  es: { open: "“«", close: "”»" },
  fr: { open: "“«", close: "”»" },
  de: { open: "„", close: "“”" },
};

function quotes(text: string, L: Language, out: RuleIssue[]) {
  const category = t("Punctuation");
  const { open, close } = QUOTES[L];
  const first = text.search(/\S/);
  const stack: number[] = [];
  const straight: number[] = [];
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (open.includes(c)) stack.push(i);
    // German “…” in English order: an opening mark when a closing one follows.
    else if (L === "de" && c === "“" && !stack.length && text.indexOf("”", i + 1) > 0) stack.push(i);
    else if (close.includes(c)) {
      if (stack.length) stack.pop();
      else issue(out, i, i + 1, "UNMATCHED_CLOSING_QUOTE", category, t("A closing quotation mark without an opening one"), t("Missing opening quotation mark"), []);
    } else if (c === '"' && !/\d/.test(text[i - 1] ?? "")) {
      straight.push(i);
    }
  }
  // A quotation can run on into the next paragraph, which starts with its opening mark: only a mark in the middle of the text is a mistake.
  const lone = [...stack, ...(straight.length % 2 ? [straight[straight.length - 1]] : [])];
  for (const at of lone) {
    if (at === first) continue;
    issue(out, at, at + 1, "UNMATCHED_OPENING_QUOTE", category, t("An opening quotation mark without a closing one"), t("Missing closing quotation mark"), []);
  }
}

// ---------------------------------------------------------------------------
// Spanish
// ---------------------------------------------------------------------------

function invertedMarks(text: string, out: RuleIssue[]) {
  const open = { "?": 0, "!": 0 };
  const category = t("Punctuation");
  const marks: Record<string, { inverted: string; id: string; message: string }> = {
    "?": { inverted: "¿", id: "MISSING_INVERTED_QUESTION", message: t("Add the opening question mark ¿") },
    "!": { inverted: "¡", id: "MISSING_INVERTED_EXCLAMATION", message: t("Add the opening exclamation mark ¡") },
  };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === "¿") open["?"]++;
    else if (c === "¡") open["!"]++;
    else if (c === "?" || c === "!") {
      if (open[c] > 0) {
        open[c]--;
        continue;
      }
      // Web addresses and "?!" after a question are not sentences of their own.
      const next = text[i + 1];
      if (next !== undefined && !/[\s)\]"”»'’.,;:!?…]/.test(next)) continue;
      const token = /(\S*)$/.exec(text.slice(0, i + 1))![1];
      if (/:\/\/|www\.|[=&]/.test(token) || (text[i - 1] === "?" || text[i - 1] === "!")) continue;
      issue(out, i, i + 1, marks[c].id, category, t("Questions and exclamations open with an inverted mark in Spanish"), marks[c].message, []);
    }
  }
}
