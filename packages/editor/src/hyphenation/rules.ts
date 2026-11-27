// Docxtor's own hyphenator: syllable breaks from a few rules, for English, Spanish, French, German and Italian. It is no
// match for a pattern dictionary, but it breaks words where a reader would expect, and it is what Hyphenate manually
// offers when no other hyphenator knows the language (another one can be added: see `ctx.hyphenation`).

const VOWELS = "aeiouyáéíóúàèìòùâêîôûäöüæœ";
const isVowel = (c: string) => VOWELS.includes(c);

/** Two letters that are one sound or a stop with l or r: they start a syllable together (ma-chine, a-pron). */
const CLUSTERS = new Set(["ch", "sh", "th", "ph", "gh", "wh", "qu", "tr", "pr", "br", "cr", "dr", "fr", "gr", "bl", "cl", "fl", "gl", "pl", "sl", "nh", "ll", "rr"]);

/** Two letters that end a syllable (pock-et, sing-er). */
const CLOSERS = new Set(["ck", "ng"]);

/** Where `word` may be broken, as the number of letters before the hyphen; null for a word that isn't made of letters. */
export function ruleBreaks(word: string, lang = "en"): number[] | null {
  if (!/^[\p{L}]+$/u.test(word)) return null;
  const w = word.toLowerCase();
  if (w.length < 5) return [];
  // Vowel groups: each is the core of a syllable.
  const groups: [number, number][] = [];
  for (let i = 0; i < w.length; ) {
    if (!isVowel(w[i])) {
      i++;
      continue;
    }
    let j = i;
    while (j < w.length && isVowel(w[j])) j++;
    groups.push([i, j]);
    i = j;
  }
  const points = new Set<number>();
  // Between two groups the consonants between them go to one side or the other.
  for (let g = 0; g + 1 < groups.length; g++) {
    const end = groups[g][1];
    const start = groups[g + 1][0];
    const run = w.slice(end, start);
    if (run.length === 1) points.add(end);
    else if (run.length >= 2) {
      const last2 = run.slice(-2);
      if (run.length >= 2 && (CLOSERS.has(last2) || last2[0] === last2[1])) points.add(last2[0] === last2[1] ? start - 1 : start);
      else if (CLUSTERS.has(last2)) points.add(start - 2);
      else points.add(end + 1);
    }
  }
  // Two vowels that don't make one sound split in Spanish, Italian and Portuguese (le-er, ma-es-tro), never in English.
  if (/^(es|it|pt)/.test(lang)) {
    for (let i = 1; i < w.length; i++) if ("aeo".includes(w[i - 1]) && "aeo".includes(w[i])) points.add(i);
  }
  // Leave at least two letters before and three after, so no break leaves a stub on either line.
  return [...points].filter((p) => p >= 2 && w.length - p >= 3).sort((a, b) => a - b);
}
