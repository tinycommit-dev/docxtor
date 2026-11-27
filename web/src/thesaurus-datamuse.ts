// Datamuse, as an app extension: more English synonyms and opposites from the Datamuse service (datamuse.com),
// listed next to Docxtor's own thesaurus. It is its own extension, and off until someone turns it on, because it is the
// one place a word from the document leaves the page: each word looked up in the thesaurus panel is sent to
// api.datamuse.com. Datamuse sorts words by how strongly they are related, not by meaning, so its answer is one
// list for each part of speech instead of one for each meaning.
import { timeLimit, defineExtension, msg, type Extension, type ThesaurusSense } from "@docxtor/editor";

export const DATAMUSE = "https://api.datamuse.com/words";

interface Hit {
  word: string;
  tags?: string[];
}

const POS: Record<string, string> = { n: "noun", v: "verb", adj: "adjective", adv: "adverb" };

/** Datamuse's words as meanings: a list for each part of speech it names, nouns first. Words it gives no part of speech for come last. */
export function sensesFrom(synonyms: Hit[], opposites: Hit[]): ThesaurusSense[] {
  const order = ["noun", "verb", "adjective", "adverb", ""];
  const by = new Map<string, { synonyms: string[]; antonyms: string[] }>();
  const slot = (pos: string) => {
    let s = by.get(pos);
    if (!s) by.set(pos, (s = { synonyms: [], antonyms: [] }));
    return s;
  };
  const posOf = (hit: Hit) => hit.tags?.map((t) => POS[t]).find(Boolean) ?? "";
  for (const hit of synonyms) slot(posOf(hit)).synonyms.push(hit.word);
  for (const hit of opposites) slot(posOf(hit)).antonyms.push(hit.word);
  return order
    .filter((pos) => by.get(pos)?.synonyms.length)
    .map((pos) => {
      const s = by.get(pos)!;
      return { ...(pos ? { pos } : {}), synonyms: s.synonyms, ...(s.antonyms.length ? { antonyms: s.antonyms } : {}) };
    });
}

async function ask(relation: "rel_syn" | "rel_ant", word: string, signal: AbortSignal): Promise<Hit[]> {
  const res = await fetch(`${DATAMUSE}?${relation}=${encodeURIComponent(word)}&md=p&max=40`, { signal });
  if (!res.ok) throw new Error(`Datamuse answered ${res.status}`);
  const data: unknown = await res.json();
  return Array.isArray(data) ? data.filter((h): h is Hit => !!h && typeof (h as Hit).word === "string") : [];
}

export const datamuse: Extension = defineExtension({
  id: "docxtor.thesaurus.datamuse",
  name: msg("Datamuse thesaurus"),
  description: msg("More English synonyms and opposites from the Datamuse service, next to Docxtor's own list."),
  privacy: msg("Sends each word you look up in the thesaurus to api.datamuse.com."),
  enabledByDefault: false,
  activate(ctx) {
    ctx.thesaurus.register({
      id: "docxtor.thesaurus.datamuse",
      label: "Datamuse",
      languages: ["en"],
      note: msg("Sends the word to api.datamuse.com."),
      async lookup(word) {
        const signal = timeLimit(8000);
        const [synonyms, opposites] = await Promise.all([ask("rel_syn", word, signal), ask("rel_ant", word, signal)]);
        const senses = sensesFrom(synonyms, opposites);
        return senses.length ? { senses } : null;
      },
    });
  },
});
