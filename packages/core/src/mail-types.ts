// Mail merge, labels and envelopes: the shapes the engine hands over (see crates/docxtor-core/src/merge).

/** Word's kinds of main document (`w:mainDocumentType`). */
export type MergeMainType = "formLetters" | "mailingLabels" | "envelopes" | "catalog" | "email" | "fax";

/** Where a list of recipients came from. */
export interface MergeSource {
  /** `csv`, `xlsx`, or the id of a data source an extension added. */
  kind: string;
  /** The file's name, or a name for a source that isn't a file. */
  name: string;
  /** The sheet of a workbook. */
  table?: string;
  /** The separator of a text file. */
  delimiter?: string;
}

/** A list of recipients read from a file. */
export interface RecipientList {
  source: MergeSource;
  columns: string[];
  rows: string[][];
  /** The sheets of a workbook (empty for a text file). */
  sheets: string[];
  /** Records past the limit were left out. */
  truncated: boolean;
}

/** One rule of a filter. */
export interface MergeFilterRule {
  field: string;
  /** `eq`, `ne`, `lt`, `gt`, `le`, `ge`, `empty`, `not_empty`, `contains` or `not_contains`. */
  op: string;
  value: string;
  /** How it joins the rule before it: `and` (default) or `or`. */
  join?: "and" | "or";
}

export interface MergeSortKey {
  field: string;
  desc?: boolean;
}

/** What the interface needs to know about the mail merge (`Engine.mergeInfo`). */
export interface MergeInfo {
  /** The document is a mail merge main document. */
  active: boolean;
  mainType: MergeMainType;
  sourceName: string | null;
  /** Docxtor holds the recipients (not only a link to a file Word wrote). */
  hasRecipients: boolean;
  source: MergeSource | null;
  columns: string[];
  /** Records in the list. */
  records: number;
  /** Records in the merge: ticked and passing the filter. */
  included: number;
  /** The position of the record shown in place of the template. */
  previewing: number | null;
  mapping: Record<string, string>;
  /** Word shows merged results when the file opens. */
  viewMerged: boolean;
}

/** The recipients for the list editor (`Engine.mergeData`). */
export interface MergeData {
  source: MergeSource;
  columns: string[];
  rows: string[][];
  /** A tick for each record. */
  active: boolean[];
  filter: MergeFilterRule[];
  sort: MergeSortKey[];
  mapping: Record<string, string>;
  /** The records in the merge, in order, as indexes into `rows`. */
  order: number[];
  /** Each of Word's address fields with the column that stands for it now (mapped or guessed). */
  matched: [field: string, column: string | null][];
}

/** A question the merge asks (an `ASK` or `FILLIN` field). */
export interface MergePrompt {
  /** The key of the answer: `ask:<bookmark>` or `fill:<prompt>`. */
  key: string;
  kind: "ask" | "fillin";
  prompt: string;
  default: string;
  /** Asked once for the whole merge. */
  once: boolean;
}

/** The answers to the questions: for every record, or for one (its position in the list). */
export interface MergeAnswers {
  once?: Record<string, string>;
  records?: Record<number, Record<string, string>>;
}

/** Which records to merge: positions in the list from 0, `to` exclusive. */
export interface MergeRange {
  from?: number;
  to?: number;
}

export interface MergePreview {
  position: number;
  count: number;
  /** How many records the copy used (more than one for labels). */
  used: number;
  /** A rule leaves this record out of the merge. */
  skipped: boolean;
}

export interface MergeResult {
  /** Records merged. */
  records: number;
  /** Copies of the document made (a sheet of labels takes several records). */
  copies: number;
  /** Records left out by a rule or by a skip. */
  skipped: number;
}

/** A merge field or rule to insert (`insert_merge_field`). */
export type MergeFieldSpec =
  | { kind: "field"; name: string; before?: string; after?: string; case?: "upper" | "lower" | "first_cap" | "caps"; picture?: string }
  | { kind: "address_block"; name?: string; company?: boolean; country?: "never" | "always" | "different"; except?: string; format?: string }
  | { kind: "greeting_line"; salutation?: string; name?: string; punctuation?: string; invalid?: string }
  | { kind: "if"; field: string; op: string; value?: string; then: string; otherwise?: string }
  | { kind: "ask"; bookmark: string; prompt: string; default?: string; once?: boolean }
  | { kind: "fill_in"; prompt: string; default?: string; once?: boolean }
  | { kind: "next" }
  | { kind: "next_if"; field: string; op: string; value?: string }
  | { kind: "skip_if"; field: string; op: string; value?: string }
  | { kind: "merge_rec" }
  | { kind: "merge_seq" }
  | { kind: "set"; bookmark: string; value: string };

/** A kind of label sheet, as the engine builds it. Sizes are in twips. */
export interface LabelSpec {
  id: string;
  vendor: string;
  name: string;
  page_w: number;
  page_h: number;
  /** From the top of the page to the top of the first label. */
  top: number;
  /** From the left of the page to the left of the first label. */
  left: number;
  label_w: number;
  label_h: number;
  /** From the left of one label to the left of the next. */
  pitch_h: number;
  /** From the top of one label to the top of the next. */
  pitch_v: number;
  across: number;
  down: number;
}

/** What goes on the labels. */
export type LabelFill = { kind: "same"; lines: string[] } | { kind: "single"; lines: string[]; row: number; col: number } | { kind: "merge" };

/** How the text on a label or envelope looks. */
export interface TextLook {
  font?: string;
  size_pt?: number;
  bold?: boolean;
  italic?: boolean;
}

export interface EnvelopeSize {
  id: string;
  name: string;
  /** The long side, in twips. */
  w: number;
  h: number;
}

export interface EnvelopeLayout {
  delivery: string;
  return?: string;
  delivery_look?: TextLook;
  return_look?: TextLook;
  delivery_x?: number;
  delivery_y?: number;
  return_x?: number;
  return_y?: number;
}

/** Word's address fields and the formats for a name in an address block. */
export interface MergeVocabulary {
  fields: string[];
  nameFormats: [id: string, example: string][];
}
