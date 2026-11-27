// Docxtor's English thesaurus: looks a word up in the groups of english-data.ts. It finds the word itself or, when
// the word is an inflected form (running, stopped, cities, quickly, bigger), its base word, and gives the synonyms
// the same ending. No outside code and no network. Pure functions, no DOM.
import type { ThesaurusEntry, ThesaurusSense } from "@docxtor/extensions";

interface Group {
  pos: "n" | "v" | "a" | "r";
  words: string[];
}

interface Opposites {
  pos: Group["pos"];
  a: string[];
  b: string[];
}

/** The endings a word can have. `s` is the plural of a noun or the -s of a verb. */
type Ending = "s" | "past" | "pp" | "ing" | "er" | "est" | "ly";

const POS_NAMES: Record<Group["pos"], string> = { n: "noun", v: "verb", a: "adjective", r: "adverb" };

/** Verbs whose forms don't follow the rules: base, past, past participle. */
const IRREGULAR_VERBS = `
bear bore borne;beat beat beaten;become became become;begin began begun;bend bent bent;bite bit bitten;blow blew blown;
break broke broken;bring brought brought;build built built;burn burnt burnt;buy bought bought;catch caught caught;
choose chose chosen;come came come;deal dealt dealt;dig dug dug;do did done;draw drew drawn;drink drank drunk;
drive drove driven;eat ate eaten;fall fell fallen;feel felt felt;fight fought fought;find found found;fly flew flown;
forbid forbade forbidden;forget forgot forgotten;forgive forgave forgiven;get got got;give gave given;go went gone;
grow grew grown;hang hung hung;have had had;hear heard heard;hide hid hidden;hit hit hit;hold held held;keep kept kept;
know knew known;lay laid laid;lead led led;leave left left;lend lent lent;let let let;lie lay lain;light lit lit;
lose lost lost;make made made;mean meant meant;meet met met;mistake mistook mistaken;overcome overcame overcome;
pay paid paid;put put put;read read read;ride rode ridden;ring rang rung;rise rose risen;run ran run;say said said;
see saw seen;seek sought sought;sell sold sold;send sent sent;set set set;shake shook shaken;shine shone shone;
shoot shot shot;show showed shown;shut shut shut;sing sang sung;sink sank sunk;sit sat sat;sleep slept slept;
slide slid slid;speak spoke spoken;spend spent spent;spin spun spun;split split split;spread spread spread;
stand stood stood;steal stole stolen;strike struck struck;swear swore sworn;swim swam swum;take took taken;
teach taught taught;tear tore torn;tell told told;think thought thought;throw threw thrown;understand understood understood;
undertake undertook undertaken;wake woke woken;wear wore worn;win won won;withdraw withdrew withdrawn;write wrote written
`;

/** Nouns with their own plural. */
const IRREGULAR_PLURALS: Record<string, string> = { man: "men", woman: "women", child: "children", foot: "feet", tooth: "teeth", mouse: "mice", goose: "geese", person: "people" };

const VOWELS = "aeiou";
const isVowel = (c: string | undefined) => !!c && VOWELS.includes(c);
const isConsonant = (c: string | undefined) => !!c && /[a-z]/.test(c) && !isVowel(c);

/** How many groups of vowels a word has, as a rough count of its syllables. */
function syllables(w: string): number {
  // A final e is silent, and so is the e of -ely (lovely).
  const n = (w.toLowerCase().replace(/e$/, "").replace(/([^aeiouy])e(?=[^aeiouy]y$)/, "$1").match(/[aeiouy]+/g) ?? []).length;
  return Math.max(1, n);
}

/** A short word that ends consonant, vowel, consonant doubles the last letter before an ending that starts with a vowel (stop, stopped). */
function doubles(w: string): boolean {
  const n = w.length;
  return syllables(w) === 1 && n >= 3 && isConsonant(w[n - 1]) && !"wxy".includes(w[n - 1]) && isVowel(w[n - 2]) && isConsonant(w[n - 3]);
}

type Table = { past: Map<string, string>; pp: Map<string, string>; fromPast: Map<string, string>; fromPp: Map<string, string> };

function irregulars(): Table {
  const t: Table = { past: new Map(), pp: new Map(), fromPast: new Map(), fromPp: new Map() };
  for (const row of IRREGULAR_VERBS.split(";")) {
    const [base, past, pp] = row.trim().split(/\s+/);
    if (!base || !past || !pp) continue;
    t.past.set(base, past);
    t.pp.set(base, pp);
    if (past !== base) t.fromPast.set(past, base);
    if (pp !== base) t.fromPp.set(pp, base);
  }
  return t;
}

/** `word` with an ending, by the rules of English spelling. Null when the word can't take it. */
export function inflect(word: string, ending: Ending, table: Table = TABLE): string | null {
  const w = word;
  const last = w[w.length - 1];
  const before = w[w.length - 2];
  switch (ending) {
    case "s":
      if (IRREGULAR_PLURALS[w]) return IRREGULAR_PLURALS[w];
      if (/(s|x|z|ch|sh)$/.test(w)) return `${w}es`;
      if (last === "y" && isConsonant(before)) return `${w.slice(0, -1)}ies`;
      return `${w}s`;
    case "past":
    case "pp": {
      const irregular = (ending === "past" ? table.past : table.pp).get(w);
      if (irregular) return irregular;
      if (last === "e") return `${w}d`;
      if (last === "y" && isConsonant(before)) return `${w.slice(0, -1)}ied`;
      return doubles(w) ? `${w}${last}ed` : `${w}ed`;
    }
    case "ing":
      if (w.endsWith("ie")) return `${w.slice(0, -2)}ying`;
      if (last === "e" && !/(ee|ye|oe)$/.test(w) && w.length > 2) return `${w.slice(0, -1)}ing`;
      return doubles(w) ? `${w}${last}ing` : `${w}ing`;
    case "er":
    case "est": {
      const suffix = ending === "er" ? "er" : "est";
      // Longer adjectives take more and most, which the caller can't use as one word.
      if (syllables(w) > 2 || (syllables(w) === 2 && !/(y|le|ow|er)$/.test(w))) return null;
      if (last === "e") return `${w}${suffix.slice(1)}`;
      if (last === "y" && isConsonant(before)) return `${w.slice(0, -1)}i${suffix}`;
      return doubles(w) ? `${w}${last}${suffix}` : `${w}${suffix}`;
    }
    case "ly":
      if (last === "y" && isConsonant(before)) return `${w.slice(0, -1)}ily`;
      if (/ic$/.test(w)) return `${w}ally`;
      if (/le$/.test(w) && isConsonant(before)) return `${w.slice(0, -1)}y`;
      if (/ll$/.test(w)) return `${w}y`;
      return `${w}ly`;
  }
}

const TABLE = irregulars();

/** A candidate base word for an inflected one, and the ending that was taken off. */
interface Candidate {
  stem: string;
  ending?: Ending;
  /** The parts of speech the base word must be to count (a verb for -ing). */
  pos?: Group["pos"][];
}

/** The base words an inflected word might come from, most likely first. The word itself is first. */
export function candidates(word: string, table: Table = TABLE): Candidate[] {
  const w = word.toLowerCase();
  const out: Candidate[] = [{ stem: w }];
  const add = (stem: string, ending: Ending, pos: Group["pos"][]) => {
    if (stem.length >= 2 && !out.some((c) => c.stem === stem && c.ending === ending)) out.push({ stem, ending, pos });
  };
  const undouble = (s: string) => (s.length > 2 && s[s.length - 1] === s[s.length - 2] && isConsonant(s[s.length - 1]) ? s.slice(0, -1) : null);
  const plural = Object.entries(IRREGULAR_PLURALS).find(([, p]) => p === w);
  if (plural) add(plural[0], "s", ["n"]);
  const pastBase = table.fromPast.get(w);
  if (pastBase) add(pastBase, "past", ["v"]);
  const ppBase = table.fromPp.get(w);
  if (ppBase) add(ppBase, "pp", ["v"]);
  if (w.endsWith("ies")) add(`${w.slice(0, -3)}y`, "s", ["n", "v"]);
  if (/(ves)$/.test(w)) {
    add(`${w.slice(0, -3)}f`, "s", ["n"]);
    add(`${w.slice(0, -3)}fe`, "s", ["n"]);
  }
  if (/(ches|shes|sses|xes|zes)$/.test(w)) add(w.slice(0, -2), "s", ["n", "v"]);
  if (w.endsWith("s") && !w.endsWith("ss")) add(w.slice(0, -1), "s", ["n", "v"]);
  if (w.endsWith("ied")) {
    add(`${w.slice(0, -3)}y`, "past", ["v"]);
    add(`${w.slice(0, -3)}y`, "pp", ["v"]);
  }
  if (w.endsWith("ed")) {
    for (const e of ["past", "pp"] as const) {
      add(w.slice(0, -2), e, ["v"]);
      add(w.slice(0, -1), e, ["v"]);
      const u = undouble(w.slice(0, -2));
      if (u) add(u, e, ["v"]);
    }
  }
  if (w.endsWith("ying") && w.length > 5) add(`${w.slice(0, -4)}ie`, "ing", ["v"]);
  if (w.endsWith("ing") && w.length > 4) {
    const s = w.slice(0, -3);
    add(s, "ing", ["v"]);
    add(`${s}e`, "ing", ["v"]);
    const u = undouble(s);
    if (u) add(u, "ing", ["v"]);
  }
  if (w.endsWith("ier") || w.endsWith("iest")) {
    const cut = w.endsWith("ier") ? 3 : 4;
    add(`${w.slice(0, -cut)}y`, w.endsWith("ier") ? "er" : "est", ["a"]);
  }
  for (const [suffix, ending] of [["er", "er"], ["est", "est"]] as const) {
    if (!w.endsWith(suffix) || w.length < suffix.length + 3) continue;
    const s = w.slice(0, -suffix.length);
    add(s, ending, ["a"]);
    add(`${s}e`, ending, ["a"]);
    const u = undouble(s);
    if (u) add(u, ending, ["a"]);
  }
  if (w.endsWith("ily")) add(`${w.slice(0, -3)}y`, "ly", ["a"]);
  if (w.endsWith("ally")) add(w.slice(0, -2), "ly", ["a"]);
  if (w.endsWith("ly") && w.length > 4) {
    add(w.slice(0, -2), "ly", ["a"]);
    if (w.endsWith("bly") || w.endsWith("ply") || w.endsWith("tly")) add(`${w.slice(0, -1)}e`, "ly", ["a"]);
  }
  return out;
}

export class EnglishThesaurus {
  private groups: Group[] = [];
  private opposites: Opposites[] = [];
  private index = new Map<string, number[]>();
  private oppositeIndex = new Map<string, number[]>();

  /** Read the groups (see english-data.ts for the format). */
  constructor(data: string) {
    for (const raw of data.split("\n")) {
      const line = raw.trim();
      if (!line) continue;
      if (line.startsWith("!")) {
        const [pos, a, b] = line.slice(1).split("|");
        if (!pos || !a || !b) continue;
        const o: Opposites = { pos: pos as Group["pos"], a: a.split(",").map(clean), b: b.split(",").map(clean) };
        const i = this.opposites.push(o) - 1;
        for (const w of [...o.a, ...o.b]) push(this.oppositeIndex, w, i);
        continue;
      }
      const [pos, list] = line.split("|");
      if (!pos || !list) continue;
      const words = [...new Set(list.split(",").map(clean).filter(Boolean))];
      if (words.length < 2) continue;
      const i = this.groups.push({ pos: pos as Group["pos"], words }) - 1;
      for (const w of words) push(this.index, w, i);
    }
  }

  /** How many words the thesaurus has an entry for. */
  get size(): number {
    return this.index.size;
  }

  /** The senses of a word, as they are written in the groups. */
  private senses(stem: string, pos?: Group["pos"][]): { group: Group; first: boolean }[] {
    const found = (this.index.get(stem) ?? []).map((i) => this.groups[i]);
    const wanted = pos ? found.filter((g) => pos.includes(g.pos)) : found;
    return wanted.map((group) => ({ group, first: group.words[0] === stem })).sort((a, b) => Number(b.first) - Number(a.first));
  }

  private opposite(stem: string, pos: Group["pos"]): string[] {
    const out: string[] = [];
    for (const i of this.oppositeIndex.get(stem) ?? []) {
      const o = this.opposites[i];
      if (o.pos !== pos) continue;
      const side = o.a.includes(stem) ? o.b : o.b.includes(stem) ? o.a : [];
      for (const w of side) if (!out.includes(w)) out.push(w);
    }
    return out;
  }

  /**
   * The synonyms and opposites of a word, or null when it has none. A word that is a word in its own right (running is
   * a noun) gets those meanings first; when it is also an inflected form of another word (run) that word's meanings
   * follow, with the ending on their synonyms (sprinting, racing) and `word` naming the base word.
   */
  lookup(word: string): ThesaurusEntry | null {
    const asked = clean(word);
    if (!asked) return null;
    const senses: ThesaurusSense[] = [];
    const used = new Set<string>();
    let inflected = 0;
    for (const c of candidates(asked)) {
      if (used.has(c.stem)) continue;
      const found = this.senses(c.stem, c.pos);
      if (!found.length) continue;
      if (c.ending && ++inflected > 2) break;
      const mine: ThesaurusSense[] = [];
      for (const { group } of found.slice(0, 8)) {
        // An ending the word had is put on its synonyms too (stopped: halted, ceased).
        const give = (words: string[]) =>
          words
            .filter((w) => w !== c.stem)
            .flatMap((w) => {
              if (!c.ending) return [w];
              const out = inflectPhrase(w, c.ending, group.pos);
              return out ? [out] : [];
            });
        const synonyms = give(group.words);
        if (!synonyms.length) continue;
        const antonyms = give(this.opposite(c.stem, group.pos));
        const label = group.words[0] !== c.stem ? group.words[0] : group.words[1];
        mine.push({ pos: c.ending === "ly" ? "adverb" : POS_NAMES[group.pos], meaning: label, synonyms, ...(antonyms.length ? { antonyms } : {}), ...(c.stem === asked ? {} : { word: c.stem }) });
      }
      if (mine.length) {
        used.add(c.stem);
        senses.push(...mine);
      }
    }
    if (!senses.length) return null;
    // All of it is for one base word: the entry says so.
    const bases = new Set(senses.map((s) => s.word ?? asked));
    if (bases.size === 1 && !bases.has(asked)) {
      const [base] = bases;
      return { word: base, senses: senses.map(({ word: _word, ...rest }) => rest) };
    }
    return { senses };
  }
}

/** A word or short phrase with an ending: a verb phrase takes it on its first word (give up, gave up), a noun phrase on its last. */
function inflectPhrase(phrase: string, ending: Ending, pos: Group["pos"]): string | null {
  if (ending === "ly") return pos === "a" ? inflect(phrase, "ly") : null;
  const parts = phrase.split(" ");
  const verb = ending === "past" || ending === "pp" || ending === "ing" || (ending === "s" && pos === "v");
  if (verb && pos !== "v") return null;
  if ((ending === "er" || ending === "est") && (pos !== "a" || parts.length > 1)) return null;
  const at = verb ? 0 : parts.length - 1;
  const done = inflect(parts[at], ending);
  if (!done) return null;
  parts[at] = done;
  return parts.join(" ");
}

function clean(w: string): string {
  return w.trim().toLowerCase().replace(/[’ʼ]/g, "'");
}

function push(map: Map<string, number[]>, key: string, value: number) {
  const list = map.get(key);
  if (list) list.push(value);
  else map.set(key, [value]);
}

/** Give a replacement the capitals the word it replaces had: Big becomes Large, BIG becomes LARGE. */
export function matchCase(original: string, replacement: string): string {
  if (!original || !replacement) return replacement;
  const letters = original.replace(/[^\p{L}]/gu, "");
  if (letters.length > 1 && letters === letters.toUpperCase() && letters !== letters.toLowerCase()) return replacement.toUpperCase();
  const first = original.match(/\p{L}/u)?.[0];
  if (first && first === first.toUpperCase() && first !== first.toLowerCase()) return replacement.charAt(0).toUpperCase() + replacement.slice(1);
  return replacement;
}
