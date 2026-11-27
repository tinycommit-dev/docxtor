// Which words of a paragraph's text are worth checking. Positions are UTF-16 offsets, like the model's.

export interface Word {
  word: string;
  from: number;
  to: number;
}

/** A stretch of text that isn't checked: [from, to). */
export type Span = [number, number];

const segmenters = new Map<string, Intl.Segmenter>();

function segmenter(locale: string): Intl.Segmenter {
  let s = segmenters.get(locale);
  if (!s) {
    try {
      s = new Intl.Segmenter(locale, { granularity: "word" });
    } catch {
      s = new Intl.Segmenter(undefined, { granularity: "word" });
    }
    segmenters.set(locale, s);
  }
  return s;
}

/** Web addresses and e-mail addresses, wherever they are in the text. */
const ADDRESS = /(?:\b(?:https?|ftp|file):\/\/|\bwww\.)[^\s<>]+|[^\s<>@]+@[^\s<>@]+\.[^\s<>@]+/giu;
/** Letters of a script the dictionaries cover (all five are Latin), with the marks and apostrophes that join them. Abbreviations with dots (e.g.) and domain names fail it. */
const CHECKABLE = /^[\p{Script=Latin}\p{M}'’ʼ]+$/u;
const DIGIT = /\p{N}/u;
const LETTER = /\p{L}/u;

/** The web and e-mail addresses in `text`, as spans. */
export function addressSpans(text: string): Span[] {
  return Array.from(text.matchAll(ADDRESS), (m) => [m.index!, m.index! + m[0].length] as Span);
}

const overlaps = (spans: Span[], from: number, to: number) => spans.some(([a, b]) => from < b && to > a);

/** What the spelling checker leaves alone (File, Tools, Spelling and grammar, Proofing options). All three are on until someone turns them off, as in Word. */
export interface WordOptions {
  /** Words in capitals (initialisms such as NASA). */
  ignoreUppercase: boolean;
  /** Words with digits or underscores in them (COVID19, file_name). */
  ignoreNumbers: boolean;
  /** Web, e-mail and file addresses. */
  ignoreAddresses: boolean;
}

export const DEFAULT_WORD_OPTIONS: WordOptions = { ignoreUppercase: true, ignoreNumbers: true, ignoreAddresses: true };

/** Latin letters with digits among them, for when words with numbers are checked too. */
const CHECKABLE_WITH_DIGITS = /^[\p{Script=Latin}\p{M}\p{N}'’ʼ]+$/u;

/**
 * The words to check in `text`. Skipped: anything in `skip` (links, fields, code fonts, text marked as not to be
 * checked), single letters, and by default addresses, words with digits or underscores and words in capitals
 * (initialisms such as NASA), as Google Docs and Word do. An apostrophe inside a word keeps it whole (don't, it’s).
 */
export function wordsIn(text: string, locale: string, skip: Span[] = [], options: WordOptions = DEFAULT_WORD_OPTIONS): Word[] {
  if (!text) return [];
  const spans = skip.slice();
  if (options.ignoreAddresses) for (const m of text.matchAll(ADDRESS)) spans.push([m.index!, m.index! + m[0].length]);
  const out: Word[] = [];
  const consider = (word: string, from: number) => {
    if (word.length < 2 || !LETTER.test(word)) return;
    const digits = DIGIT.test(word) || word.includes("_");
    if (digits && options.ignoreNumbers) return;
    if (!(digits ? CHECKABLE_WITH_DIGITS : CHECKABLE).test(word.replace(/_/g, ""))) return;
    if (options.ignoreUppercase && word === word.toUpperCase() && word !== word.toLowerCase()) return;
    const to = from + word.length;
    if (spans.length && overlaps(spans, from, to)) return;
    out.push({ word, from, to });
  };
  for (const seg of segmenter(locale).segment(text)) {
    if (!seg.isWordLike) continue;
    // Chrome's segmenter breaks letters on either side of a full stop (www.example.com is www, example, com; e.g is e, g); Firefox's and
    // Safari's keep them as one word. Split them here, so that what is flagged does not depend on the browser.
    if (seg.segment.includes(".")) {
      let at = 0;
      for (const piece of seg.segment.split(/(?<=\p{L})\.(?=\p{L})/u)) {
        consider(piece, seg.index + at);
        at += piece.length + 1;
      }
    } else consider(seg.segment, seg.index);
  }
  return out;
}

/** A 53-bit hash of a string (cyrb53), to tell whether a paragraph's text changed without keeping a copy. */
export function hash(s: string): number {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 2654435761);
    h2 = Math.imul(h2 ^ c, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return 4294967296 * (2097151 & h2) + (h1 >>> 0);
}
