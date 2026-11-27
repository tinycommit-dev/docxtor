// Readability statistics, as Word shows them after a spelling and grammar check: counts, averages, and for English
// the share of passive sentences and the Flesch scores. Worked out from the document's plain text, one paragraph a
// line. Plain functions, tested in Node.

export interface Readability {
  words: number;
  characters: number;
  paragraphs: number;
  sentences: number;
  sentencesPerParagraph: number;
  wordsPerSentence: number;
  charactersPerWord: number;
  /** For English text only: null otherwise. */
  english: {
    /** Passive sentences, as a share of all the sentences, 0 to 100. */
    passive: number;
    /** Flesch Reading Ease, 0 (hard) to 100 (easy). */
    readingEase: number;
    /** Flesch-Kincaid Grade Level: the school year a reader needs. */
    gradeLevel: number;
  } | null;
}

/** Shortenings whose full stop doesn't end a sentence. */
const ABBREVIATIONS = new Set(["mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "e.g", "i.e", "approx", "no", "fig", "p", "pp", "vol", "inc", "ltd", "co", "dept", "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "sept", "oct", "nov", "dec"]);

const WORD = /[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu;

/** The sentences of a paragraph. */
export function sentencesOf(paragraph: string): string[] {
  const out: string[] = [];
  let start = 0;
  const re = /[.!?…]+["'”’)\]]*(?=\s+|$)/gu;
  for (const m of paragraph.matchAll(re)) {
    const end = m.index + m[0].length;
    const before = paragraph.slice(start, m.index);
    const last = /([\p{L}.]+)$/u.exec(before)?.[1]?.toLowerCase() ?? "";
    // "Dr. Smith", "e.g. this", a single initial ("J. Smith"): the stop is part of a word.
    if (m[0] === "." && (ABBREVIATIONS.has(last.replace(/\.$/, "")) || /^\p{L}$/u.test(last))) continue;
    // A stop followed by a word in small letters ("at 5 p.m. today") doesn't end the sentence either.
    if (/^\s+\p{Ll}/u.test(paragraph.slice(end))) continue;
    const s = paragraph.slice(start, end).trim();
    if (s.match(WORD)) out.push(s);
    start = end;
  }
  const rest = paragraph.slice(start).trim();
  if (rest.match(WORD)) out.push(rest);
  return out;
}

/** About how many syllables an English word has: groups of vowels, less a silent final e. */
export function syllables(word: string): number {
  const w = word.toLowerCase().replace(/[^a-z]/g, "");
  if (!w) return 0;
  if (w.length <= 3) return 1;
  const trimmed = w.replace(/(?:[^laeiouy]es|[^laeiouy]ed|[^laeiouy]e)$/, "").replace(/^y/, "");
  const groups = trimmed.match(/[aeiouy]+/g);
  return Math.max(1, groups ? groups.length : 1);
}

const BE = "am|is|are|was|were|be|been|being|get|gets|got|gotten|getting";
const IRREGULAR =
  "born|built|bought|brought|caught|chosen|done|drawn|driven|eaten|fallen|felt|found|forgotten|given|gone|grown|heard|held|hidden|hit|hurt|kept|known|laid|led|left|lent|lost|made|meant|met|paid|put|read|ridden|run|said|seen|sent|set|shaken|shown|shut|sold|spent|spoken|stolen|struck|sung|taken|taught|thought|thrown|told|understood|woken|won|worn|written";
const PASSIVE = new RegExp(`\\b(?:${BE})\\s+(?:\\w+ly\\s+)?(?:\\w+ed|${IRREGULAR})\\b`, "i");

/** Whether an English sentence is in the passive voice ("was written", "is being reviewed"). */
export function isPassive(sentence: string): boolean {
  return PASSIVE.test(sentence);
}

const round = (n: number, places = 1) => Math.round(n * 10 ** places) / 10 ** places;

/** The statistics of a text, one paragraph a line, in a language (only English gets the Flesch scores and passives). */
export function readability(text: string, lang: string): Readability {
  const paragraphs = text.split(/\r?\n/).filter((p) => p.match(WORD));
  let words = 0;
  let letters = 0;
  let syl = 0;
  let passives = 0;
  const sentences: string[] = [];
  for (const p of paragraphs) {
    for (const s of sentencesOf(p)) {
      sentences.push(s);
      if (isPassive(s)) passives++;
    }
    for (const w of p.match(WORD) ?? []) {
      words++;
      letters += [...w.replace(/['’-]/g, "")].length;
      syl += syllables(w);
    }
  }
  const nS = sentences.length;
  const english = /^en\b/i.test(lang);
  const wps = nS ? words / nS : 0;
  const spw = words ? syl / words : 0;
  return {
    words,
    characters: letters,
    paragraphs: paragraphs.length,
    sentences: nS,
    sentencesPerParagraph: round(paragraphs.length ? nS / paragraphs.length : 0),
    wordsPerSentence: round(wps),
    charactersPerWord: round(words ? letters / words : 0),
    english:
      english && words
        ? {
            passive: Math.round(nS ? (passives / nS) * 100 : 0),
            readingEase: round(Math.max(0, Math.min(100, 206.835 - 1.015 * wps - 84.6 * spw))),
            gradeLevel: round(Math.max(0, 0.39 * wps + 11.8 * spw - 15.59)),
          }
        : null,
  };
}
