// French capitals may go without their accents (ECOLE, Ecole for ÉCOLE, École), as the Académie allows and Word
// accepts unless asked not to. These are the forms a word in capitals could stand for, to look up one by one: far
// cheaper than asking the dictionary for suggestions. Plain functions, tested in Node.

/** The accented capitals each plain capital may stand for in French. */
const ACCENTED: Record<string, string[]> = {
  A: ["À", "Â"],
  C: ["Ç"],
  E: ["É", "È", "Ê", "Ë"],
  I: ["Î", "Ï"],
  O: ["Ô"],
  U: ["Ù", "Û", "Ü"],
  Y: ["Ÿ"],
};

/**
 * The forms of a word with accents put back on its plain capitals, at most `limit` of them (a long word in capitals
 * has many). None for a word with no capital that could take one.
 */
export function accentedCapitals(word: string, limit = 400): string[] {
  let forms = [""];
  let changed = false;
  for (const ch of word) {
    const options = ACCENTED[ch];
    if (!options) {
      forms = forms.map((f) => f + ch);
      continue;
    }
    changed = true;
    const next: string[] = [];
    for (const f of forms) for (const o of [ch, ...options]) if (next.length < limit) next.push(f + o);
    forms = next;
  }
  return changed ? forms.filter((f) => f !== word) : [];
}
