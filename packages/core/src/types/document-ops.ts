// Document-level ops: breaks, page setup, headers, properties, accessibility, comparison, fonts
import type { WatermarkLayout } from "./page.ts";
import type { MPos, RunPatch } from "./patches.ts";

export type BreakKind = "page" | "column" | "next_page" | "continuous" | "even_page" | "odd_page";

/** Typed shapes of the breaks and layout ops (all are plain `Op`s on the wire). */
export interface InsertBreakOp {
  op: "insert_break";
  at: MPos;
  kind: BreakKind;
}

/** `section` is a paragraph id inside the section; omit it for every section. */
export interface SetPageSetupOp {
  op: "set_page_setup";
  section?: number | string;
  width?: number;
  height?: number;
  margins?: [number, number, number, number];
}

export interface SetColumnsOp {
  op: "set_columns";
  section?: number | string;
  /** 1 to 6 */
  count: number;
  space?: number;
  sep?: boolean;
  /** Unequal columns: a width for each column (twips), and the space after each but the last. */
  widths?: number[];
  spaces?: number[];
}

export interface SetLineNumbersOp {
  op: "set_line_numbers";
  section?: number | string;
  on: boolean;
  count_by?: number;
  restart?: "newPage" | "newSection" | "continuous";
}

export interface SetPageColorOp {
  op: "set_page_color";
  /** Hex with or without `#`; null removes the colour. */
  color: string | null;
}

export interface SetHyphenationOp {
  op: "set_hyphenation";
  on: boolean;
}

export type PageNumberFormat = "decimal" | "lowerRoman" | "upperRoman" | "lowerLetter" | "upperLetter";

export interface SetPageNumberingOp {
  op: "set_page_numbering";
  /** A paragraph id in the section to change; absent for every section. */
  section?: number;
  /** Restart the numbers at this value; absent continues from the section before. */
  start?: number;
  /** Absent leaves the format as it is. */
  format?: PageNumberFormat;
  /** Chapter page numbers: the Heading level (1 to 9) whose number goes before the page number; 0 removes it. */
  chap_style?: number;
  chap_sep?: "hyphen" | "period" | "colon" | "emDash" | "enDash";
}

export interface HeaderFooterOp {
  op: "add_header_footer" | "remove_header_footer";
  kind: "header" | "footer";
  variant?: "default" | "first" | "even";
  /** A paragraph id in the section; absent for the last section. */
  section?: number;
}

/**
 * Word's "Link to previous" for a header or footer of a section after the first. `linked: false` gives the section
 * its own copy of what it shows now (so the two can differ); `true` drops its own, so it shows the previous
 * section's again.
 */
export interface LinkHeaderFooterOp {
  op: "link_header_footer";
  kind: "header" | "footer";
  variant?: "default" | "first" | "even";
  /** A paragraph id in the section; absent for the last section. */
  section?: number;
  linked: boolean;
}

/**
 * Bookmark the range `from`..`to` (collapsed for a point bookmark; it may span paragraphs). Names start with a
 * letter, then letters, digits or underscores, at most 40 characters, unique ignoring case; hidden names
 * (starting with `_`) can't be made. A name in use is an error: rename or delete that bookmark first.
 */
export interface InsertBookmarkOp {
  op: "insert_bookmark";
  from: MPos;
  to: MPos;
  name: string;
}

/** Remove a bookmark (both markers). Links that point at it stay as they are. */
export interface DeleteBookmarkOp {
  op: "delete_bookmark";
  name: string;
}

/** Rename a bookmark. Internal links and `REF`, `PAGEREF` and `NOTEREF` fields that point at it follow. */
export interface RenameBookmarkOp {
  op: "rename_bookmark";
  from: string;
  to: string;
}

/**
 * An equation written in LaTeX at `at`. It takes one position in the paragraph's text, like an image; the result's
 * `caret` is after it. With `display` it gets a paragraph of its own (the paragraph is split at `at`) and is
 * centred, as Word does. Fails with a plain message when the LaTeX can't be read.
 */
export interface InsertEquationOp {
  op: "insert_equation";
  at: MPos;
  latex: string;
  display?: boolean;
}

/**
 * Replace the equation at `at` (the character there must be an equation) with new LaTeX. `display` moves it onto a
 * line of its own or back into the text; leave it out to keep how it is shown. Deleting an equation is a plain `delete`.
 */
export interface SetEquationOp {
  op: "set_equation";
  at: MPos;
  latex: string;
  display?: boolean;
}

/**
 * A horizontal line across the text width at `at`: an empty Normal paragraph with a bottom border. An empty
 * paragraph becomes the line; otherwise the line goes before, after or (splitting it) inside the paragraph.
 * The result's `caret` is the start of the paragraph after the line (one is made if the line would be last).
 */
export interface InsertHorizontalLineOp {
  op: "insert_horizontal_line";
  at: MPos;
}

/**
 * A text watermark behind the text of every section (Word's WordArt shape in each header, made if missing;
 * the headers' own content stays). `text: null` removes the watermark (an error when there is none).
 * Defaults: diagonal, colour `C0C0C0`, the body font, opacity 0.5.
 */
export interface SetWatermarkOp {
  op: "set_watermark";
  text: string | null;
  layout?: WatermarkLayout;
  /** Hex, with or without `#`; VML colour names such as `silver` work too. */
  color?: string;
  font?: string;
  /** 0 (clear) to 1 (solid). */
  opacity?: number;
}

/**
 * A picture watermark behind the text of every section (Word's `WordPictureWatermark` shape in each header, with the
 * image related from the header). `media` comes from `Engine.addMedia`; `width` and `height` are the picture's size in
 * points at 100%. Without `scale` it is shrunk (never enlarged) to fit the text area; `scale` is 1 for 100%. `washout`
 * (default true) lightens it as Word's Washout does. Replaces any watermark; remove it with `set_watermark` and `text: null`.
 */
export interface SetPictureWatermarkOp {
  op: "set_picture_watermark";
  media: string;
  width: number;
  height: number;
  scale?: number;
  washout?: boolean;
}

/**
 * The file's properties (`docProps/core.xml`, made if the file has none). A property that is absent stays as it
 * is; an empty string removes it. Nothing else in the file changes. Undoable.
 */
export interface SetDocPropsOp {
  op: "set_doc_props";
  title?: string;
  subject?: string;
  author?: string;
  keywords?: string;
  /** Word's Comments. */
  description?: string;
  category?: string;
  status?: string;
  /** Company and manager live in `docProps/app.xml`. */
  company?: string;
  manager?: string;
}

/** A custom document property, as Word's Properties dialog shows it on its Custom tab. */
export interface CustomProp {
  name: string;
  type: "text" | "number" | "date" | "bool";
  /** Text; a number as written; a date as `2026-10-14`; `true` or `false` for yes or no. */
  value: string;
}

/**
 * Add or change custom properties (`docProps/custom.xml`) and remove others by name. Names are compared ignoring case;
 * ones that start with an underscore belong to Word and are refused. A value that does not fit its type is an error.
 * One undo step.
 */
export interface SetCustomPropsOp {
  op: "set_custom_props";
  set?: CustomProp[];
  remove?: string[];
}

/**
 * The language of the text as a BCP 47 tag (`en-US`, `es-ES`, `fr-FR`; `en-us` is tidied to `en-US`, a bad tag is
 * an error). Without `ids` it is the document's default (`PageSetup.lang`); with `ids` it is set on the text of
 * those paragraphs only, for passages in another language. Undoable.
 */
export interface SetLanguageOp {
  op: "set_language";
  lang: string;
  ids?: (number | string)[];
}

/**
 * Change the case of the text between two positions, keeping its formatting (one undo step). Casing follows the
 * paragraph's language: Turkish and Azeri have a dotted and a dotless i.
 */
export interface ChangeCaseOp {
  op: "change_case";
  from: MPos;
  to: MPos;
  mode: "sentence" | "lower" | "upper" | "title" | "toggle";
}

/** Set the character formatting every paragraph starts from (`w:docDefaults`): font, size, colour and the rest. */
export interface SetDefaultFormatOp {
  op: "set_default_format";
  format: RunPatch;
}

/** The file's properties; absent ones are not set. See `Engine.docProps()`. */
export interface DocProps {
  title?: string;
  subject?: string;
  author?: string;
  keywords?: string;
  description?: string;
  /** When the file was made (`dcterms:created`), as the file has it: `2026-10-13T09:30:00Z`. */
  created?: string;
  /** When the file was last changed (`dcterms:modified`); Docxtor sets it on save when the document was edited. */
  modified?: string;
  /** Who changed it last (`cp:lastModifiedBy`); Docxtor sets it from the user name on save. */
  lastModifiedBy?: string;
  category?: string;
  status?: string;
  company?: string;
  manager?: string;
  /** Custom properties, without the private ones Word keeps (names that start with an underscore). */
  custom?: CustomProp[];
}

export type A11yKind =
  | "missing_alt"
  | "heading_skip"
  | "no_headings"
  | "empty_heading"
  | "no_title"
  | "table_header"
  | "merged_cells"
  | "link_text"
  | "contrast"
  | "fake_list"
  | "blank_lines"
  | "language";

export type A11ySeverity = "error" | "warning" | "tip";

/** One accessibility problem; see `Engine.accessibility()`. The `message` depends only on `kind`, so it can be translated by kind. */
export interface A11yIssue {
  kind: A11yKind;
  severity: A11ySeverity;
  /** The paragraph to go to (for a table its first paragraph, for an image the paragraph holding it); absent for problems with the whole document. */
  p?: number;
  /** UTF-16 offset of the image, link or text in that paragraph. */
  o?: number;
  /** What is wrong and how to fix it, in English. */
  message: string;
  /** The variable part: the link text, `#999999 on #FFFFFF (2.8:1)`, `Heading 1 to Heading 3`, a count of typed items or blank lines, `1600 words`. */
  detail?: string;
}

/** A stretch of a compared paragraph: in both versions (`same`), only in the newer (`ins`), only in the older (`del`), or the same words with other character formatting (`fmt`). */
export interface ComparePart {
  t: "same" | "ins" | "del" | "fmt";
  text: string;
  /** For `fmt`: what changed, by property name (`bold`, `italic`, `underline`, `strike`, `color`, `size`, `font`, `highlight`, `other`...). */
  fmt?: string[];
}

/** One paragraph of `Engine.compareView()`, in reading order. Removed paragraphs sit where they were removed. */
export interface CompareBlock {
  /** `formatted`: the words are the same, the formatting is not. */
  kind: "same" | "added" | "removed" | "changed" | "formatted";
  /** One part for same, added and removed paragraphs (none if the paragraph is empty); word-level changes for a changed one. */
  parts: ComparePart[];
  /** The paragraph in the newer document; absent for a removed one. */
  p?: number;
  /** Position among the newer document's paragraphs (for a removed paragraph, where it would have been). */
  index: number;
  /** The paragraph is in a table. */
  table?: boolean;
  /** Paragraph properties that changed (`style`, `align`, `indLeft`, `spaceBefore`...), for a paragraph found in both versions. */
  paraFmt?: string[];
}

/** What a file carries besides what the page shows, one category of `Engine.inspect()`. */
export interface InspectFinding {
  id: InspectCategory;
  /** How many there are; 0 means none. */
  count: number;
  /** What they are, where that is worth showing: names (`author: Ana`, `company: Acme`), parts. */
  detail: string[];
  /** Whether `remove_hidden_data` can take them out. */
  removable: boolean;
}

export type InspectCategory = "properties" | "authors" | "comments" | "revisions" | "hiddenText" | "headersFooters" | "customXml" | "embedded";

/** Take what a file carries besides the page out of the document (one undo step); `count` of the result says how many things went. */
export interface RemoveHiddenDataOp {
  op: "remove_hidden_data";
  what: Exclude<InspectCategory, "embedded">[];
}

/** Paragraphs, words and characters of some text. */
export interface TextCounts {
  paragraphs: number;
  words: number;
  chars: number;
  charsNoSpaces: number;
}

/** `Engine.counts()`: the main text, and the notes and text boxes counted apart. The top-level four are the main text's. */
export interface TextStats extends TextCounts {
  notes: TextCounts;
  boxes: TextCounts;
}

export interface SetTitlePageOp {
  op: "set_title_page";
  on: boolean;
  /** A paragraph id in the section; absent for the last section. */
  section?: number;
}

/** One paragraph-level change between two versions. */
export interface DiffEntry {
  kind: "added" | "removed" | "changed";
  before?: string;
  after?: string;
  index: number;
}

export interface DiffSummary {
  added: number;
  removed: number;
  changed: number;
  entries: DiffEntry[];
}

export interface PartDiff {
  name: string;
  similarity: number;
  textEqual: boolean;
}

/** How faithfully a saved package preserves the original, part by part (score is 0-100). */
export interface FidelityReport {
  partsTotal: number;
  partsIdentical: number;
  missing: string[];
  added: string[];
  changed: PartDiff[];
  score: number;
}

/** An AI tool in the Anthropic tool-use shape. */
export interface ToolDefinition {
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
}

/** A font embedded in a .docx. */
export interface FontUse {
  family: string;
  /** Characters of text in this font. */
  chars: number;
}

export interface EmbeddedFont {
  family: string;
  style: "regular" | "bold" | "italic" | "boldItalic";
  /** The font file (TrueType or OpenType). */
  data: Uint8Array;
}

