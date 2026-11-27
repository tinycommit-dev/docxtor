// The spelling checker: holds the dictionaries and answers the page's requests. One dictionary per key. It is spellbook in
// WebAssembly (crates/docxtor-spell), which reads a Hunspell dictionary without listing every form of every word: English holds about
// 2 MB and French about 5 (nspell, before it, held 12 and 295, and took hundreds of milliseconds to build one, so it ran in a worker;
// spellbook builds English in about 12 ms and checks a word in microseconds, so it runs on the page, in a chunk loaded when spelling
// first starts, and a worker's own copy of the runtime is saved).
import init, { Speller } from "./wasm/spell.js";
import { accentedCapitals } from "./accents";
import type { SpellRequest, SpellResponse } from "./types";

const dictionaries = new Map<string, Speller>();
let ready: Promise<unknown> | null = null;

/** Text from bytes: UTF-8, or the charset the affix file names (`SET ISO8859-1`). */
function decode(data: string | Uint8Array, charset?: string): string {
  if (typeof data === "string") return data;
  try {
    return new TextDecoder(charset ?? "utf-8").decode(data);
  } catch {
    return new TextDecoder().decode(data);
  }
}

function charsetOf(aff: string | Uint8Array): string | undefined {
  const head = typeof aff === "string" ? aff.slice(0, 4000) : new TextDecoder("latin1").decode(aff.subarray(0, 4000));
  const set = /^SET\s+(\S+)/m.exec(head)?.[1];
  if (!set || /^utf-?8$/i.test(set)) return undefined;
  return set.replace(/^ISO8859-/i, "iso-8859-");
}

/** The affix text read as UTF-8: the checker reads text, and the dictionary's own charset line no longer applies to it. */
const asUtf8 = (aff: string) => aff.replace(/^SET\s+\S+/m, "SET UTF-8");

/** The flags a word has in the word list (`robot/MS` gives `MS`), or "" when it has none or isn't there. */
function flagsOf(dic: string, word: string): string | null {
  for (const end of ["/", "\n", "\r", "\t", " "]) {
    const at = dic.indexOf(`\n${word}${end}`);
    if (at < 0) continue;
    if (end !== "/") return "";
    const from = at + word.length + 2;
    const stop = dic.slice(from, from + 64).search(/[\s]/);
    return dic.slice(from, stop < 0 ? from + 64 : from + stop);
  }
  return null;
}

/** Hunspell dictionaries write the typographic apostrophe as a straight one. */
const straight = (word: string) => word.replace(/[’ʼ]/g, "'");

async function handle(req: SpellRequest): Promise<SpellResponse> {
  // A request waits a turn: a page full of words checked at once never runs in the task that asked.
  await new Promise((done) => setTimeout(done, 0));
  if (req.type === "load") {
    if (!dictionaries.has(req.key)) {
      await (ready ??= init());
      const charset = charsetOf(req.aff);
      const aff = asUtf8(decode(req.aff, charset));
      const dic = decode(req.dic, charset);
      const dict = new Speller(aff, dic);
      // Newer everyday words the dictionary lacks (see extra-words.ts), with the endings of a word it has.
      for (const [word, like] of req.extra ?? []) {
        const flags = like ? flagsOf(dic, like) : "";
        if (!(flags && dict.add(`${word}/${flags}`))) dict.add(word);
      }
      dictionaries.set(req.key, dict);
    }
    return { id: req.id, ok: true };
  }
  const dict = dictionaries.get(req.key);
  if (!dict) return { id: req.id, ok: false, error: "No dictionary" };
  if (req.type === "check") {
    const words = req.words.map(straight);
    // Words are one a line for one call into the checker (they never hold a line break).
    const found = dict.checkLines(words.join("\n"));
    // A capital without its accent counts when the accented form is a word (French, unless accents are asked for).
    const ok = (w: string, i: number) => found[i] === 1 || (!!req.frenchCapitals && /^\p{Lu}/u.test(w) && accentedCapitals(w).some((f) => dict.check(f)));
    return { id: req.id, ok: true, result: words.map(ok) };
  }
  const curly = /[’ʼ]/.test(req.word);
  const found = dict.suggest(straight(req.word));
  return { id: req.id, ok: true, result: curly ? found.map((s) => s.replace(/'/g, "’")) : found };
}

// Requests are answered in the order they came: a check that arrives while its dictionary loads waits for it.
let queue: Promise<unknown> = Promise.resolve();

/** Free the dictionaries whose keys start with `prefix` (a spelling client's, when it closes). */
export function drop(prefix: string) {
  queue = queue.then(() => {
    for (const [key, dict] of dictionaries) {
      if (!key.startsWith(prefix)) continue;
      dict.free();
      dictionaries.delete(key);
    }
  });
}

/** Answer one request. */
export function answer(req: SpellRequest): Promise<SpellResponse> {
  const next = queue.then(() => handle(req).catch((err): SpellResponse => ({ id: req.id, ok: false, error: String((err as Error)?.message ?? err) })));
  queue = next;
  return next;
}
