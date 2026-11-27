// The page's side of the spelling checker: loads it when a document first needs it (checker.ts, its own chunk), asks the host
// for a language's dictionary once, and turns lookups into promises.
import type { SpellingProvider, SpellRequest } from "./types";
import { extraWords } from "./extra-words";

/** A request without its id (distributes over the union, which Omit does not). */
type Body<T> = T extends unknown ? Omit<T, "id"> : never;

/** Each client's dictionaries are its own in the checker, which the page shares: keys start with the client's number. */
let clients = 0;

export class SpellClient {
  private checker: Promise<typeof import("./checker")> | null = null;
  private readonly prefix = `${++clients}/`;
  private seq = 0;
  private loaded = new Map<string, Promise<boolean>>();
  /** The checker's key for each language tag: tags that share a dictionary (en-AU and en-GB) share one. */
  private keys = new Map<string, string>();
  /** The checker's key for each dictionary the provider returned, by its affix file. */
  private byDictionary = new Map<unknown, string>();
  private broken = false;

  constructor(private provider: SpellingProvider) {}

  private async send(req: Body<SpellRequest>): Promise<boolean[] | string[] | undefined> {
    if (this.broken) throw new Error("The spelling checker was closed");
    const checker = await (this.checker ??= import("./checker"));
    const res = await checker.answer({ ...req, key: this.prefix + req.key, id: ++this.seq } as SpellRequest);
    if (this.broken) throw new Error("The spelling checker was closed");
    if (!res.ok) throw new Error(res.error);
    return res.result;
  }

  /** Get a language's dictionary ready (once). Resolves false when the host has none for it or it can't be built. */
  ready(lang: string): Promise<boolean> {
    const key = lang.toLowerCase();
    let p = this.loaded.get(key);
    if (!p) {
      p = (async () => {
        try {
          const dict = await this.provider.load(lang);
          if (!dict) return false;
          // The same dictionary for another tag (en-AU after en-GB) is not built twice.
          const shared = this.byDictionary.get(dict.aff);
          if (shared) {
            this.keys.set(key, shared);
            return true;
          }
          await this.send({ type: "load", key, aff: dict.aff, dic: dict.dic, extra: extraWords(lang) });
          this.byDictionary.set(dict.aff, key);
          this.keys.set(key, key);
          return true;
        } catch {
          return false;
        }
      })();
      this.loaded.set(key, p);
    }
    return p;
  }

  private keyOf(lang: string): string {
    const key = lang.toLowerCase();
    return this.keys.get(key) ?? key;
  }

  /** For each word, whether it is spelled correctly. `frenchCapitals`: capitals may go without their accents. */
  async check(lang: string, words: string[], frenchCapitals = false): Promise<boolean[]> {
    return ((await this.send({ type: "check", key: this.keyOf(lang), words, frenchCapitals })) as boolean[]) ?? words.map(() => true);
  }

  async suggest(lang: string, word: string): Promise<string[]> {
    try {
      return ((await this.send({ type: "suggest", key: this.keyOf(lang), word })) as string[]) ?? [];
    } catch {
      return [];
    }
  }

  destroy() {
    this.broken = true;
    void this.checker?.then((c) => c.drop(this.prefix));
  }
}
