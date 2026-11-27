// Language services the host asks its registries for: spelling, detection, hyphenation, reading, thesaurus, grammar.
import type { ExtensionHost } from "./host.ts";
import type { DetectedLanguage, GrammarChecker, GrammarIssue, GrammarParagraph, SpellingDictionary, Thesaurus, ThesaurusAnswer } from "./types.ts";
import { cleanIssue, languageMatches, mergeIssues } from "./checkers.ts";

/** How long a grammar checker may take before the others' answers are used without it (milliseconds). */
const GRAMMAR_TIMEOUT = 30000;

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error("The grammar checker took too long")), ms);
  });
  return Promise.race([promise, late]).finally(() => clearTimeout(timer));
}

/**
 * A Hunspell dictionary for a language. Providers that cover the language are asked latest first; the
 * first to return one wins. A provider that throws is skipped.
 */
export async function spellingDictionary(host: ExtensionHost, lang: string): Promise<SpellingDictionary | null> {
  for (const entry of [...host.spellingDictionaries.all()].reverse()) {
    if (!languageMatches(entry.item.languages, lang)) continue;
    try {
      const dict = await entry.item.load(lang);
      if (dict) return dict;
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
  return null;
}

/**
 * The language of a piece of text, for Detect language automatically. Detectors are asked latest first; the first
 * answer at least `sure` confident wins. A detector that throws is reported and skipped. Null when none is sure.
 */
export async function detectLanguage(host: ExtensionHost, text: string, sure = 0.6): Promise<DetectedLanguage | null> {
  for (const entry of [...host.languageDetectors.all()].reverse()) {
    try {
      const found = await entry.item.detect(text);
      if (found && typeof found.lang === "string" && found.lang && found.confidence >= sure) return { lang: found.lang, confidence: Math.min(1, found.confidence) };
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
  return null;
}

/**
 * Where a word can be hyphenated in a language: the offsets from the latest registered hyphenator that knows the
 * language and has an opinion. A hyphenator that throws is reported and skipped. No opinion at all is an empty list.
 */
export async function hyphenationPoints(host: ExtensionHost, word: string, lang: string): Promise<number[]> {
  for (const entry of [...host.hyphenators.all()].reverse()) {
    if (!languageMatches(entry.item.languages, lang)) continue;
    try {
      const points = await entry.item.hyphenate(word, lang);
      if (points) return [...new Set(points)].filter((p) => Number.isInteger(p) && p > 0 && p < word.length).sort((a, b) => a - b);
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
  return [];
}

/**
 * The reading of East Asian text in a language: the answer of the latest registered provider that knows the language
 * and has one, with the id of the provider. A provider that throws is reported and skipped. No answer is null.
 */
export async function readingFor(host: ExtensionHost, text: string, lang: string): Promise<{ text: string; source: string } | null> {
  for (const entry of [...host.readingProviders.all()].reverse()) {
    if (!languageMatches(entry.item.languages, lang)) continue;
    try {
      const reading = await entry.item.reading(text, lang);
      if (typeof reading === "string" && reading) return { text: reading, source: entry.item.id };
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
  return null;
}

/** The thesauruses that can serve a language, in the order they were registered. */
export function thesaurusesFor(host: ExtensionHost, lang: string): Thesaurus[] {
  return host.thesauruses.list().filter((th) => languageMatches(th.languages, lang));
}

/**
 * Ask every thesaurus that serves the language about a word. The answers are listed by thesaurus, in the order they
 * were registered. Synonyms are tidied (blank ones, repeats and the word itself are dropped) and a thesaurus with
 * nothing left for the word is left out. One that fails is reported and left out; the others still count.
 */
export async function thesaurusLookup(host: ExtensionHost, word: string, lang: string): Promise<ThesaurusAnswer> {
  const asked = word.trim();
  const entries = host.thesauruses.all().filter((e) => languageMatches(e.item.languages, lang));
  const answers = await Promise.all(
    entries.map(async (entry) => {
      try {
        const found = await entry.item.lookup(asked, lang);
        if (!found || !Array.isArray(found.senses)) return null;
        const head = (typeof found.word === "string" && found.word.trim()) || asked;
        const senses = found.senses.flatMap((sense) => {
          const seen = new Set([head.toLowerCase(), asked.toLowerCase()]);
          const tidy = (list: unknown) =>
            (Array.isArray(list) ? list : []).flatMap((w) => {
              const x = typeof w === "string" ? w.trim() : "";
              if (!x || seen.has(x.toLowerCase())) return [];
              seen.add(x.toLowerCase());
              return [x];
            });
          const synonyms = tidy(sense?.synonyms);
          if (!synonyms.length) return [];
          const antonyms = tidy(sense.antonyms);
          return [{ ...(sense.word ? { word: String(sense.word) } : {}), ...(sense.pos ? { pos: String(sense.pos) } : {}), ...(sense.meaning ? { meaning: String(sense.meaning) } : {}), synonyms, ...(antonyms.length ? { antonyms } : {}) }];
        });
        return senses.length ? { source: entry.item.id, label: entry.item.label, word: head, senses } : null;
      } catch (err) {
        host.report(entry.extension, err);
        return null;
      }
    }),
  );
  return { word: asked, results: answers.filter((a): a is NonNullable<typeof a> => !!a) };
}

/** The grammar checkers that can check a language, in the order they were registered. */
export function grammarCheckersFor(host: ExtensionHost, lang: string): GrammarChecker[] {
  return host.grammarCheckers.list().filter((c) => languageMatches(c.languages, lang));
}

/**
 * Ask every grammar checker about the paragraphs it can check and merge what comes back: exact duplicates
 * are dropped and the rest is in reading order. A checker that fails, or takes longer than half a minute,
 * is reported and left out; the others still count. Issues that don't fit the text they name are dropped.
 * `partial` is called with what is known each time another checker has answered, so a slow checker (one
 * on a server) does not hold back a fast one.
 */
export async function checkGrammar(host: ExtensionHost, paragraphs: GrammarParagraph[], partial?: (issues: GrammarIssue[]) => void): Promise<GrammarIssue[]> {
  const entries = host.grammarCheckers.all();
  const results: (GrammarIssue[] | undefined)[] = entries.map(() => undefined);
  const merged = () => mergeIssues(results.filter((r): r is GrammarIssue[] => !!r));
  await Promise.all(
    entries.map(async (entry, i) => {
      const mine = paragraphs.filter((p) => languageMatches(entry.item.languages, p.lang));
      if (!mine.length) return;
      try {
        const own = new Map(mine.map((p) => [p.id, p] as const));
        const found = await withTimeout(entry.item.check({ paragraphs: mine }), GRAMMAR_TIMEOUT);
        results[i] = (Array.isArray(found) ? found : []).flatMap((raw) => cleanIssue(raw, own, entry.item.id) ?? []);
      } catch (err) {
        host.report(entry.extension, err);
        results[i] = [];
      }
      partial?.(merged());
    }),
  );
  return merged();
}
