// Interface text in the reader's language. Source strings are the English text itself
// (gettext style): `t("Find and replace")`, `t("Page {page} of {pages}", { page, pages })`,
// `tn(n, "{n} word", "{n} words")`. A language pack maps that English to a translation.
// One language is active per page; switching means rebuilding the interface (Docxtor reloads).

/** A translation; plural messages give a form per plural category (`one`, `few`, `other`…). */
export type Message = string | Partial<Record<Intl.LDMLPluralRule, string>>;

export interface LanguagePack {
  /** BCP 47 tag: `es`, `pt-BR`, `zh-Hans`. */
  locale: string;
  /** The language's name in itself: "Español", "Português (Brasil)". */
  label: string;
  /** English source text to translation. Anything missing falls back to English. */
  messages: Record<string, Message>;
  /** Which way the language is written; when left out it follows the locale (Arabic, Hebrew, Persian and Urdu are right to left). */
  dir?: "ltr" | "rtl";
}

/**
 * A pseudo-language for checking the right-to-left interface: the interface turns round as it does for Arabic or Hebrew,
 * and every translated string is shown backwards, so text that isn't translatable yet stands out.
 */
export const PSEUDO_RTL_LOCALE = "ar-XB";

/** The languages written right to left (by their primary tag). */
const RTL = new Set(["ar", "he", "iw", "fa", "ur", "ps", "sd", "ug", "yi", "dv", "ckb", "syr"]);

/** Which way a language is written. */
export function directionOf(locale: string): "ltr" | "rtl" {
  return RTL.has(locale.toLowerCase().split("-")[0]) ? "rtl" : "ltr";
}

/** Which way the interface reads now: the active pack's direction, else its language's. */
export function textDirection(): "ltr" | "rtl" {
  return active?.dir ?? directionOf(currentLocale());
}

/** A pseudo-language for finding text that isn't translatable yet: every translated string is accented and bracketed. */
export const PSEUDO_LOCALE = "en-XA";

let active: LanguagePack | null = null;
let rules: Intl.PluralRules | null = null;
let numbers: Intl.NumberFormat | null = null;

/** The active language's tag (`en` when none is set). */
export function currentLocale(): string {
  return active?.locale ?? "en";
}

/** Make a pack the active language (null for English). Build the interface after calling this. */
export function setLanguage(pack: LanguagePack | null) {
  active = pack;
  const locale = currentLocale();
  const pseudoTag = locale === PSEUDO_LOCALE || locale === PSEUDO_RTL_LOCALE;
  rules = new Intl.PluralRules(pseudoTag ? "en" : locale);
  numbers = new Intl.NumberFormat(pseudoTag ? "en" : locale);
}

/** The best pack for a requested locale: an exact match, then the same language (`pt-BR` → `pt`), else null. */
export function pickLanguage(requested: string | undefined, packs: LanguagePack[]): LanguagePack | null {
  if (!requested) return null;
  if (requested === PSEUDO_LOCALE) return { locale: PSEUDO_LOCALE, label: "Pseudo", messages: {} };
  if (requested === PSEUDO_RTL_LOCALE) return { locale: PSEUDO_RTL_LOCALE, label: "Pseudo RTL", messages: {}, dir: "rtl" };
  const want = requested.toLowerCase();
  const base = want.split("-")[0];
  return packs.find((p) => p.locale.toLowerCase() === want) ?? packs.find((p) => p.locale.toLowerCase().split("-")[0] === base) ?? null;
}

const fill = (text: string, vars?: Record<string, string | number>) =>
  vars ? text.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? (typeof vars[k] === "number" ? (numbers?.format(vars[k] as number) ?? String(vars[k])) : String(vars[k])) : m)) : text;

const PSEUDO: Record<string, string> = { a: "á", e: "é", i: "î", o: "ö", u: "ü", A: "Å", E: "É", I: "Î", O: "Ö", U: "Ü", c: "ç", n: "ñ", y: "ý" };
const pseudo = (s: string) => `⟦${s.replace(/\{\w+\}|[aeiouAEIOUcny]/g, (m) => (m.length > 1 ? m : PSEUDO[m]))}⟧`;
/** Right-to-left override around the text (U+202E ... U+202C): it shows backwards, as Chrome's own ar-XB does. */
const pseudoRtl = (s: string) => `\u202e${s}\u202c`;

/** Marks text for translation without translating it yet (for tables built before a language is set). Pass it through `t()` when shown. */
export const msg = (source: string): string => source;

/** Translate interface text. `{name}` placeholders are filled from `vars` (numbers are formatted for the language). */
export function t(source: string, vars?: Record<string, string | number>): string {
  if (active?.locale === PSEUDO_LOCALE) return fill(pseudo(source), vars);
  if (active?.locale === PSEUDO_RTL_LOCALE) return fill(pseudoRtl(source), vars);
  const m = active?.messages[source];
  const text = typeof m === "string" ? m : (m?.other ?? source);
  return fill(text, vars);
}

/** Translate text that depends on a count: `tn(n, "{n} word", "{n} words")`. `{n}` is filled in. */
export function tn(n: number, one: string, other: string, vars?: Record<string, string | number>): string {
  const all = { n, ...vars };
  if (active?.locale === PSEUDO_LOCALE) return fill(pseudo(n === 1 ? one : other), all);
  if (active?.locale === PSEUDO_RTL_LOCALE) return fill(pseudoRtl(n === 1 ? one : other), all);
  const m = active?.messages[other];
  if (m && typeof m === "object") {
    const form = rules?.select(n) ?? "other";
    return fill(m[form] ?? m.other ?? other, all);
  }
  if (typeof m === "string") return fill(n === 1 ? (typeof active?.messages[one] === "string" ? (active.messages[one] as string) : m) : m, all);
  return fill(n === 1 ? one : other, all);
}
