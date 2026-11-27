// Spelling dictionaries and grammar checkers: the two kinds of proofing an extension can contribute.
// These are pure helpers (no DOM), used by the host and tested in Node.
import type { GrammarIssue, GrammarParagraph } from "./types.ts";

/** A language tag in the form used for comparing: lower case, `-` between parts. */
const norm = (lang: string) => lang.trim().toLowerCase().replace(/_/g, "-");

/**
 * Whether a contribution that covers `languages` can serve `lang`. `undefined` and `"*"` cover every
 * language. A base tag covers its regions (`en` covers `en-US` and `en-GB`); a tag with a region covers
 * that region only (`en-GB` does not cover `en-US`).
 */
export function languageMatches(languages: string[] | "*" | undefined, lang: string): boolean {
  if (languages === undefined || languages === "*") return true;
  const want = norm(lang);
  return languages.some((spec) => {
    const s = norm(spec);
    return s === "*" || want === s || want.startsWith(`${s}-`);
  });
}

/** Two issues that say the same thing about the same words. */
const sameIssue = (a: GrammarIssue, b: GrammarIssue) =>
  a.paragraph === b.paragraph && a.from === b.from && a.to === b.to && a.message === b.message && a.replacements.length === b.replacements.length && a.replacements.every((r, i) => r === b.replacements[i]);

/**
 * Merge the issues several checkers found: exact duplicates (same words, same message, same
 * replacements) are dropped, the first one found is kept, and the rest are put in reading order.
 */
export function mergeIssues(lists: GrammarIssue[][]): GrammarIssue[] {
  const out: GrammarIssue[] = [];
  const seen = new Map<string, GrammarIssue[]>();
  for (const list of lists) {
    for (const issue of list) {
      const key = `${issue.paragraph}:${issue.from}:${issue.to}`;
      const same = seen.get(key);
      if (same?.some((x) => sameIssue(x, issue))) continue;
      if (same) same.push(issue);
      else seen.set(key, [issue]);
      out.push(issue);
    }
  }
  return out.sort((a, b) => (a.paragraph < b.paragraph ? -1 : a.paragraph > b.paragraph ? 1 : 0) || a.from - b.from || a.to - b.to);
}

/**
 * An issue as a checker returned it, made safe to use: a paragraph that was asked about, a range inside
 * its text, a message and a list of replacements. Returns null for anything that can't be used.
 */
export function cleanIssue(raw: unknown, paragraphs: Map<number, GrammarParagraph>, source: string): GrammarIssue | null {
  const x = raw as Partial<GrammarIssue> | null;
  if (!x || typeof x.paragraph !== "number" || typeof x.message !== "string" || !x.message) return null;
  const para = paragraphs.get(x.paragraph);
  if (!para || typeof x.from !== "number" || typeof x.to !== "number") return null;
  if (!Number.isInteger(x.from) || !Number.isInteger(x.to) || x.from < 0 || x.to > para.text.length || x.from >= x.to) return null;
  const rule = x.rule && typeof x.rule.id === "string" ? x.rule : { id: source };
  const kind = x.kind === "style" || x.kind === "spelling" ? x.kind : "grammar";
  return {
    paragraph: x.paragraph,
    from: x.from,
    to: x.to,
    message: x.message,
    ...(x.shortMessage ? { shortMessage: String(x.shortMessage) } : {}),
    replacements: Array.isArray(x.replacements) ? x.replacements.filter((r) => typeof r === "string") : [],
    rule,
    kind,
    source: x.source ?? source,
  };
}
