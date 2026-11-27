// Telling what language a paragraph is in, for Detect language automatically. Two detectors ship with Docxtor:
// the browser's own (Chrome's LanguageDetector, when its model is already on the computer) and a small one that
// works everywhere and offline: it counts the short words every language is full of (the, de, und, che…) and the
// letters only some languages use. It knows English, Spanish, French, German, Italian, Portuguese and Dutch,
// and answers only when the text is long enough and one language clearly leads. Other detectors are extensions
// (`ctx.spelling.languageDetectors`).
import type { DetectedLanguage } from "@docxtor/extensions";

const WORDS: Record<string, string> = {
  en: "the and of to a in is that it for was on are with as this be by at have not from or an they which you were has but their will would there we been can more if about than these then into its could should",
  es: "el la de que y en los se del las un por con no una su para es al lo como más pero sus le ya o este sí porque esta entre cuando muy sin sobre también me hasta hay donde desde todo nos durante",
  fr: "le la les de des et en un une du est que qui dans pour pas sur au avec ce il elle par plus ne se sont mais nous vous aux ont cette comme été fait leur ou je ces être tout",
  de: "der die und in den von zu das mit sich des auf für ist im dem nicht ein eine als auch es an werden aus er hat dass sie nach wird bei einer um am sind noch wie einem über",
  it: "il di che e la per un in non una sono mi ho ma lo ha le si della con del da gli al anche come alla dei questo più nel io delle ci se ne cosa essere molto",
  pt: "de a o que e do da em um para é com não uma os no se na por mais as dos como mas foi ao ele das tem à seu sua ou ser quando muito há nos já está",
  nl: "de en van het een in is dat op te zijn met voor niet aan er die maar om ook als dan bij of uit nog worden door naar hij wordt kan ze zich tot wat was werd heeft deze",
};

const LISTS = Object.fromEntries(Object.entries(WORDS).map(([lang, list]) => [lang, new Set(list.split(" "))]));

/** Letters that point to a language: each one found adds to its score. */
const LETTERS: [RegExp, string][] = [
  [/[ñ¿¡]/gu, "es"],
  [/[ß]/gu, "de"],
  [/[äöü]/gu, "de"],
  [/[çœ]|[èêëîïôûù]/gu, "fr"],
  [/[ãõ]/gu, "pt"],
  [/[ìò]/gu, "it"],
  [/\bij/gu, "nl"],
];

/** Fewer words than this and nothing is said: a short line is too easy to get wrong. */
const MIN_WORDS = 6;

/** The language of a piece of text by its common words, or null when it is too short or no language clearly leads. */
export function detectByWords(text: string): DetectedLanguage | null {
  const words = text.toLowerCase().match(/\p{L}+(?:['’]\p{L}+)?/gu) ?? [];
  if (words.length < MIN_WORDS) return null;
  const score: Record<string, number> = {};
  for (const lang of Object.keys(LISTS)) score[lang] = 0;
  for (const w of words) for (const [lang, list] of Object.entries(LISTS)) if (list.has(w)) score[lang]++;
  const lower = text.toLowerCase();
  for (const [re, lang] of LETTERS) score[lang] += Math.min(3, (lower.match(re) ?? []).length) * 0.75;
  const ranked = Object.entries(score).sort((a, b) => b[1] - a[1]);
  const [lang, best] = ranked[0];
  const second = ranked[1][1];
  // Enough of the text is common words, and the leader is well ahead of the next language: by two words at least,
  // and the lead counts for more than how many common words there are ("He met Dan in Boston" has Dutch words too).
  if (best < 2 || best / words.length < 0.12 || best - second < 2) return null;
  const lead = 1 - second / best;
  const coverage = Math.min(1, best / (words.length * 0.3));
  const confidence = Math.round(Math.min(1, lead * 0.8 + coverage * 0.4) * 100) / 100;
  return { lang, confidence };
}

interface BrowserDetector {
  detect(text: string): Promise<{ detectedLanguage: string; confidence: number }[]>;
}
interface BrowserDetectorClass {
  availability(): Promise<string>;
  create(): Promise<BrowserDetector>;
}

/**
 * The browser's own detector (Chrome's LanguageDetector), when it is ready on this computer. It is never made to
 * download its model: when the model isn't there, it says nothing and the next detector answers.
 */
export function browserDetector(): { detect(text: string): Promise<DetectedLanguage | null> } {
  let made: Promise<BrowserDetector | null> | null = null;
  const get = () =>
    (made ??= (async () => {
      const cls = (globalThis as unknown as { LanguageDetector?: BrowserDetectorClass }).LanguageDetector;
      if (!cls) return null;
      try {
        return (await cls.availability()) === "available" ? await cls.create() : null;
      } catch {
        return null;
      }
    })());
  return {
    async detect(text) {
      const d = await get();
      if (!d) return null;
      const [top] = await d.detect(text);
      return top && top.detectedLanguage !== "und" ? { lang: top.detectedLanguage, confidence: top.confidence } : null;
    },
  };
}
