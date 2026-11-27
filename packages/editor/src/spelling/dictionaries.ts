// The custom dictionaries: lists of words the spelling checker accepts, kept for the person (not in a document).
// As in Word (File, Options, Proofing, Custom dictionaries) there can be several: each has a name, is for one
// language or all of them, and can be switched off; Add to dictionary puts a word in the default one. The words
// are also offered as corrections, unless the person asks for suggestions from the main dictionary only.
// Plain data and functions: no DOM, so it is tested in Node; where it is kept is the caller's business.

export interface CustomDictionary {
  id: string;
  name: string;
  /** A language tag (`en`, `fr-CA`), or "" for every language. */
  lang: string;
  /** In use: its words are accepted. */
  on: boolean;
  words: string[];
}

export interface DictionaryState {
  list: CustomDictionary[];
  /** The id of the dictionary Add to dictionary puts words in. */
  default: string;
}

/** Two spellings of a word count as the same word: case and the kind of apostrophe don't matter. */
export const sameWord = (word: string) => word.replace(/[’ʼ]/g, "'").toLowerCase();

/** Whether a dictionary is for a language: one for every language is, `en` is for `en-GB`, `en-GB` only for itself. */
export function forLanguage(dictLang: string, lang: string): boolean {
  if (!dictLang) return true;
  const d = dictLang.toLowerCase().replace(/_/g, "-");
  const l = lang.toLowerCase().replace(/_/g, "-");
  return d === l || (!d.includes("-") && l.split("-")[0] === d);
}

/** How many single-letter edits (a letter added, dropped, changed, or two swapped) turn a into b; stops past `max`. */
export function editDistance(a: string, b: string, max = 2): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  const rows: number[][] = [];
  for (let i = 0; i <= a.length; i++) rows.push([i]);
  for (let j = 1; j <= b.length; j++) rows[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    let best = Infinity;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let v = Math.min(rows[i - 1][j] + 1, rows[i][j - 1] + 1, rows[i - 1][j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) v = Math.min(v, rows[i - 2][j - 2] + 1);
      rows[i][j] = v;
      best = Math.min(best, v);
    }
    if (best > max) return max + 1;
  }
  return rows[a.length][b.length];
}

/** Read what was kept: the dictionaries, or (from before there were several) the one list of personal words. */
export function loadDictionaries(saved: unknown, personal: unknown, defaultName: string): DictionaryState {
  const list: CustomDictionary[] = [];
  const s = saved as Partial<DictionaryState> | null;
  if (s && Array.isArray(s.list)) {
    for (const d of s.list) {
      if (!d || typeof d.id !== "string" || !d.id || typeof d.name !== "string") continue;
      if (list.some((x) => x.id === d.id)) continue;
      const words = Array.isArray(d.words) ? d.words.filter((w): w is string => typeof w === "string" && !!w.trim()) : [];
      list.push({ id: d.id, name: d.name, lang: typeof d.lang === "string" ? d.lang : "", on: d.on !== false, words });
    }
  }
  if (!list.length) {
    const words = Array.isArray(personal) ? personal.filter((w): w is string => typeof w === "string" && !!w.trim()) : [];
    list.push({ id: "personal", name: defaultName, lang: "", on: true, words });
  }
  const def = s && typeof s.default === "string" && list.some((d) => d.id === s.default) ? s.default : list[0].id;
  return { list, default: def };
}

export class CustomDictionaries {
  private state: DictionaryState;
  private listeners = new Set<() => void>();
  private next = 1;
  private save: (state: DictionaryState) => void;
  private defaultName: string;
  /** Each dictionary's words as `sameWord` keys, for `has`, which runs for every word checked. */
  private keys = new Map<string, Set<string>>();

  constructor(initial: DictionaryState, save: (state: DictionaryState) => void, defaultName: string) {
    this.state = initial;
    this.save = save;
    this.defaultName = defaultName;
    this.index();
  }

  /** Every dictionary, the default first. */
  all(): readonly CustomDictionary[] {
    const d = this.state.default;
    return [...this.state.list].sort((a, b) => Number(b.id === d) - Number(a.id === d));
  }

  get defaultId(): string {
    return this.state.default;
  }

  find(id: string): CustomDictionary | undefined {
    return this.state.list.find((d) => d.id === id);
  }

  /** Whether a word is in a dictionary in use for a language. */
  has(word: string, lang: string): boolean {
    const k = sameWord(word);
    return this.state.list.some((d) => d.on && forLanguage(d.lang, lang) && !!this.keys.get(d.id)?.has(k));
  }

  /** Words in the dictionaries in use for a language that are close to a word (one or two letters off), closest first. */
  similar(word: string, lang: string, limit = 3): string[] {
    const k = sameWord(word);
    const found: { w: string; d: number }[] = [];
    for (const dict of this.state.list) {
      if (!dict.on || !forLanguage(dict.lang, lang)) continue;
      for (const w of dict.words) {
        const s = sameWord(w);
        if (s === k || found.some((f) => sameWord(f.w) === s)) continue;
        const d = editDistance(k, s, 2);
        if (d <= 2) found.push({ w, d });
      }
    }
    return found.sort((a, b) => a.d - b.d || a.w.localeCompare(b.w)).slice(0, limit).map((f) => f.w);
  }

  /** A new dictionary; its id is unique. */
  create(name: string, lang = ""): CustomDictionary {
    let id = "";
    do id = `dict-${Date.now().toString(36)}-${this.next++}`;
    while (this.find(id));
    const dict: CustomDictionary = { id, name: name.trim() || this.defaultName, lang, on: true, words: [] };
    this.state = { ...this.state, list: [...this.state.list, dict] };
    this.changed();
    return dict;
  }

  update(id: string, patch: Partial<Pick<CustomDictionary, "name" | "lang" | "on">>) {
    const d = this.find(id);
    if (!d) return;
    const name = patch.name === undefined ? d.name : patch.name.trim() || d.name;
    this.replace({ ...d, ...patch, name });
  }

  /** Take a dictionary away. The last one can't go; a default that goes passes to the first left. */
  remove(id: string) {
    if (this.state.list.length <= 1 || !this.find(id)) return;
    const list = this.state.list.filter((d) => d.id !== id);
    this.state = { list, default: this.state.default === id ? list[0].id : this.state.default };
    this.changed();
  }

  setDefault(id: string) {
    if (!this.find(id) || this.state.default === id) return;
    this.state = { ...this.state, default: id };
    this.changed();
  }

  /** Put a word in a dictionary (the default one when none is named), unless it is there already. */
  addWord(word: string, id = this.state.default) {
    const d = this.find(id);
    const w = word.trim();
    if (!d || !w || d.words.some((x) => sameWord(x) === sameWord(w))) return;
    this.replace({ ...d, words: [...d.words, w] });
  }

  removeWord(id: string, word: string) {
    const d = this.find(id);
    if (!d) return;
    this.replace({ ...d, words: d.words.filter((w) => sameWord(w) !== sameWord(word)) });
  }

  onChange(fn: () => void): { dispose(): void } {
    this.listeners.add(fn);
    return { dispose: () => void this.listeners.delete(fn) };
  }

  private replace(d: CustomDictionary) {
    this.state = { ...this.state, list: this.state.list.map((x) => (x.id === d.id ? d : x)) };
    this.changed();
  }

  private index() {
    this.keys = new Map(this.state.list.map((d) => [d.id, new Set(d.words.map(sameWord))]));
  }

  private changed() {
    this.index();
    this.save(this.state);
    for (const fn of this.listeners) fn();
  }
}
