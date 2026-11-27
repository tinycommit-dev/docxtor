// The Special menu of the find bar: codes that stand for things that are not letters (a paragraph mark, a tab, a
// field), for the text to find and for the replacement, plus the pattern symbols of wildcards and regular expressions.
// Word's names and codes; the engine reads them (crates/docxtor-core/src/matcher.rs, document/find.rs).
import { msg } from "@docxtor/extensions";

export interface SpecialItem {
  label: string;
  code: string;
}

/** The text being written: plain text with ^ codes, Word's wildcards, or a regular expression. */
export type PatternMode = "plain" | "wildcards" | "regex";

const find: SpecialItem[] = [
  { label: msg("Paragraph mark"), code: "^p" },
  { label: msg("Tab"), code: "^t" },
  { label: msg("Line break"), code: "^l" },
  { label: msg("Page break"), code: "^m" },
  { label: msg("Section break"), code: "^b" },
  { label: msg("Column break"), code: "^n" },
  { label: msg("Any character"), code: "^?" },
  { label: msg("Any digit"), code: "^#" },
  { label: msg("Any letter"), code: "^$" },
  { label: msg("White space"), code: "^w" },
  { label: msg("Caret"), code: "^^" },
  { label: msg("Em dash"), code: "^+" },
  { label: msg("En dash"), code: "^=" },
  { label: msg("Non-breaking space"), code: "^s" },
  { label: msg("Optional hyphen"), code: "^-" },
  { label: msg("Non-breaking hyphen"), code: "^~" },
  { label: msg("Footnote mark"), code: "^f" },
  { label: msg("Endnote mark"), code: "^e" },
  { label: msg("Field"), code: "^d" },
  { label: msg("Graphic"), code: "^g" },
];

const replace: SpecialItem[] = [
  { label: msg("Paragraph mark"), code: "^p" },
  { label: msg("Tab"), code: "^t" },
  { label: msg("Line break"), code: "^l" },
  { label: msg("Page break"), code: "^m" },
  { label: msg("Column break"), code: "^n" },
  { label: msg("Caret"), code: "^^" },
  { label: msg("Em dash"), code: "^+" },
  { label: msg("En dash"), code: "^=" },
  { label: msg("Non-breaking space"), code: "^s" },
  { label: msg("Optional hyphen"), code: "^-" },
  { label: msg("Non-breaking hyphen"), code: "^~" },
  { label: msg("Find what text"), code: "^&" },
];

const wildcardFind: SpecialItem[] = [
  { label: msg("Any single character"), code: "?" },
  { label: msg("Any string of characters"), code: "*" },
  { label: msg("One of the characters"), code: "[]" },
  { label: msg("Any character but"), code: "[!]" },
  { label: msg("Start of word"), code: "<" },
  { label: msg("End of word"), code: ">" },
  { label: msg("One or more of the previous"), code: "@" },
  { label: msg("Exactly n of the previous"), code: "{1}" },
  { label: msg("n to m of the previous"), code: "{1,3}" },
  { label: msg("Expression"), code: "()" },
  { label: msg("Same as expression 1"), code: "\\1" },
];

const wildcardReplace: SpecialItem[] = [{ label: msg("Expression 1"), code: "\\1" }, { label: msg("Expression 2"), code: "\\2" }, { label: msg("Whole match"), code: "\\0" }];

const regexFind: SpecialItem[] = [
  { label: msg("Any character"), code: "." },
  { label: msg("Digit"), code: "\\d" },
  { label: msg("Word character"), code: "\\w" },
  { label: msg("White space"), code: "\\s" },
  { label: msg("Word boundary"), code: "\\b" },
  { label: msg("Start of paragraph"), code: "^" },
  { label: msg("End of paragraph"), code: "$" },
  { label: msg("Tab"), code: "\\t" },
  { label: msg("New line"), code: "\\n" },
  { label: msg("Group"), code: "()" },
  { label: msg("Either"), code: "|" },
  { label: msg("One of the characters"), code: "[]" },
  { label: msg("One or more"), code: "+" },
  { label: msg("Zero or more"), code: "*" },
  { label: msg("Optional"), code: "?" },
];

const regexReplace: SpecialItem[] = [{ label: msg("Group 1"), code: "$1" }, { label: msg("Group 2"), code: "$2" }, { label: msg("Whole match"), code: "$&" }];

/** The items the Special menu offers for a field. In wildcard and regular expression mode the pattern's symbols come first (the ^ codes still work in wildcards). */
export function specialItems(field: "find" | "replace", mode: PatternMode): SpecialItem[] {
  if (mode === "regex") return field === "find" ? regexFind : regexReplace;
  if (mode === "wildcards") return field === "find" ? [...wildcardFind, ...find.slice(0, 6)] : [...wildcardReplace, ...replace];
  return field === "find" ? find : replace;
}

/** Where the caret goes inside a pair of brackets after a code like `[]` is put in (one step back from the end). */
export const paired = (code: string) => code === "[]" || code === "()" || code === "[!]" || code === "{1}" || code === "{1,3}";
