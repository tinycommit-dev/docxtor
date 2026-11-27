// Types for find and replace and for Go to (see crates/docxtor-core/src/document/find.rs and goto.rs).
import type { MPos, ParaPatch, RunPatch } from "./types.ts";

/** Formatting to look for. A field left out is not looked at. */
export interface FindFormat {
  bold?: boolean;
  italic?: boolean;
  /** Any underline (true) or none (false). */
  underline?: boolean;
  strike?: boolean;
  caps?: boolean;
  smallCaps?: boolean;
  font?: string;
  /** Points. */
  size?: number;
  /** Hex, or `auto`. */
  color?: string;
  /** A highlight name, or `none`. */
  highlight?: string;
  /** `superscript`, `subscript` or `baseline`. */
  vertAlign?: string;
  /** A character style id. */
  style?: string;
  /** A language tag set on the text. */
  lang?: string;
  /** A paragraph style id. */
  paraStyle?: string;
  align?: string;
}

/** The options of a search, as the engine's `find` and the `replace_text` op take them. Everything is off by default. */
export interface FindOptions {
  /** Letters match in either case ("Match case" off). */
  ignore_case?: boolean;
  whole_word?: boolean;
  /** Word's wildcards: ? * [a-z] [!x] {n,m} @ < > and (groups) with \1 in the replacement. */
  wildcards?: boolean;
  /** JavaScript regular expressions, with $1 in the replacement. */
  regex?: boolean;
  prefix?: boolean;
  suffix?: boolean;
  ignore_punct?: boolean;
  ignore_space?: boolean;
  ignore_marks?: boolean;
  sounds_like?: boolean;
  /** Give a replacement the capitals of the text it replaces (Word's behaviour when "Match case" is off). */
  keep_case?: boolean;
  /** Leave out headers, footers, notes and text boxes. */
  main_only?: boolean;
  direction?: "all" | "down" | "up";
  from?: MPos;
  find_format?: FindFormat;
}

/** The `replace_text` op: the options plus what to find and what to put there. */
export interface ReplaceTextOp extends FindOptions {
  op: "replace_text";
  find: string;
  replace?: string;
  ids?: number[];
  all?: boolean;
  /** Formatting for the new text; with no replacement text it changes the matched text. */
  format?: RunPatch;
  para_format?: ParaPatch;
  language?: string;
}

/** The text a match is in when it is not the main text. A text box's text is drawn in the page, not in a part of its own. */
export interface FindStory {
  kind: "header" | "footer" | "footnote" | "endnote" | "textbox";
  /** The package part, such as `word/header1.xml`. */
  part: string;
  noteId?: string;
}

export interface FindHit {
  p: number;
  o: number;
  endP: number;
  endO: number;
  /** The matched text, a paragraph mark shown as ¶. */
  text: string;
  before: string;
  after: string;
  /** Where it is when not in the main text. */
  story?: FindStory;
}

export interface FindResult {
  hits: FindHit[];
  /** There were more matches than the list holds. */
  truncated: boolean;
}

export type GotoKind = "heading" | "table" | "graphic" | "equation" | "field" | "footnote" | "endnote" | "section";

export interface GotoTarget {
  p: number;
  o: number;
  endP: number;
  endO: number;
  label: string;
}
