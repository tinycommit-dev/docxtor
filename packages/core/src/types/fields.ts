// Notes, captions, cross-references, fields, the index and building blocks
import type { FrameFmt, PageBorders, PageFill, SectionStart } from "./page.ts";
import type { Fragment, MPos } from "./patches.ts";
import type { DocGrid } from "../east-asian-types.ts";

// ---------------------------------------------------------------------------
// Notes, captions, cross-references, fields, the index
// ---------------------------------------------------------------------------

/** Paragraph ids by page number, measured by a layout: `{ "12": 3 }` means paragraph 12 is on page 3. Without it the engine estimates pages from explicit page breaks. */
export type PageMap = Record<string, number>;

/**
 * An endnote reference at a position (the note is listed at the end of the document and numbered i, ii, iii unless
 * the document says otherwise). Word's markup: `word/endnotes.xml` with its two separator notes, a
 * `w:endnoteReference` run in style `EndnoteReference`, notes in style `EndnoteText`. The result's `caret` is in the
 * new note's text; typing there works as in any story. Main text only. Undoable.
 */
export interface InsertEndnoteOp {
  op: "insert_endnote";
  at: MPos;
  text?: string;
}

/**
 * Move footnotes to endnotes or the other way (Word's Convert). `ids` are note ids (`RNote.id`) of the other kind;
 * without them every note moves. Content and paragraph ids are kept; references renumber. The result's `count` is the
 * number of notes moved.
 */
export interface ConvertNotesOp {
  op: "convert_notes";
  to: "footnote" | "endnote";
  ids?: string[];
}

/**
 * Layout of a section beyond size and margins. `section` is a paragraph in the section (absent: every section);
 * with `from` the section and every one after it change.
 */
export interface SetSectionOp {
  op: "set_section";
  section?: number;
  from?: boolean;
  /** Extra margin for binding, and whether it is on the right (twips). */
  gutter?: number;
  rtl_gutter?: boolean;
  /** Distance of the header and footer from the paper's edge (twips). */
  header?: number;
  footer?: number;
  v_align?: "top" | "center" | "both" | "bottom";
  text_direction?: "lrTb" | "tbRl" | "btLr" | "lrTbV" | "tbRlV" | "tbLrV";
  start?: SectionStart;
  no_endnote?: boolean;
  /** The document grid; the type `default` removes it. */
  doc_grid?: DocGrid;
}

/** Page borders for a section; `borders: null` removes them. */
export interface SetPageBordersOp {
  op: "set_page_borders";
  section?: number;
  from?: boolean;
  borders: PageBorders | null;
}

/**
 * Footnote or endnote options: number format, first number, when numbering starts again and the position.
 * For the whole document, or for one section (and with `from`, the ones after it).
 */
export interface SetNotePropsOp {
  op: "set_note_props";
  kind: "footnote" | "endnote";
  section?: number;
  from?: boolean;
  format?: "decimal" | "lowerRoman" | "upperRoman" | "lowerLetter" | "upperLetter" | "symbol";
  start?: number;
  restart?: "continuous" | "eachSect" | "eachPage";
  pos?: "pageBottom" | "beneathText" | "sectEnd" | "docEnd";
}

/** Document-wide layout settings. A number of 0 removes a numeric setting. */
export interface SetLayoutSettingsOp {
  op: "set_layout_settings";
  mirror_margins?: boolean;
  gutter_at_top?: boolean;
  even_and_odd_headers?: boolean;
  book_fold?: "none" | "bookFold" | "bookFoldRev";
  book_fold_sheets?: number;
  print_two_on_one?: boolean;
  hyphenation_zone?: number;
  consecutive_hyphen_limit?: number;
  do_not_hyphenate_caps?: boolean;
  hide_spelling_errors?: boolean;
  hide_grammatical_errors?: boolean;
}

/** The page background; `fill: null` removes it. Pattern, texture and picture fills name an image added with `add_media`. */
export interface SetPageBackgroundOp {
  op: "set_page_background";
  fill: PageFill | null;
}

/** Set (or with `frame: null`, remove) the frame of paragraphs. */
export interface SetFrameOp {
  op: "set_frame";
  ids: number[];
  frame: FrameFmt | null;
}

/** A drop cap: the first letter moves into a frame of its own, as Word does it. `id` is the text paragraph (or the frame). */
export interface SetDropCapOp {
  op: "set_drop_cap";
  id: number;
  kind: "none" | "drop" | "margin";
  lines?: number;
  /** Distance from the text (twips). */
  distance?: number;
  font?: string;
}

/**
 * A page number in the header or footer (or in the margin, as a frame) of a section: just the number, or "Page 1 of 5",
 * written with PAGE and NUMPAGES fields. A page number already there is replaced. `section` is a paragraph in the
 * section (absent: the last one); `page_word` and `of_word` are the words around the numbers in the document's language.
 */
export interface InsertPageNumberOp {
  op: "insert_page_number";
  section?: number;
  place: "top" | "bottom" | "margin_left" | "margin_right";
  align?: "left" | "center" | "right";
  style?: "plain" | "x_of_y";
  page_word?: string;
  of_word?: string;
  variant?: "default" | "first" | "even";
}

/** Take the page numbers out of every header and footer. */
export interface RemovePageNumbersOp {
  op: "remove_page_numbers";
}

/**
 * The placeholder texts and words of cover pages, headers and footers in the interface language, by key: `title`,
 * `subtitle`, `author`, `company`, `date`, `abstract`, `course`, `supervisor`, `type`, `page`, `of`, `submitted_by`, and
 * `dateFormat` and `lang` for the date boxes. Anything missing is English.
 */
export type GalleryLabels = Record<string, string>;

/** A design in the cover page, header or footer gallery. `name` is English; the interface translates it. */
export interface GalleryDesign {
  id: string;
  name: string;
}

/**
 * Insert, Cover page: one of Docxtor's designs (`Engine.pageGallery("cover")`) or a saved cover page (`fragment`) at the start
 * of the document, in place of the cover page there. Its boxes are content controls bound to the document's properties.
 */
export interface InsertCoverPageOp {
  op: "insert_cover_page";
  design?: string;
  fragment?: Fragment;
  labels?: GalleryLabels;
}

/** Remove current cover page. */
export interface RemoveCoverPageOp {
  op: "remove_cover_page";
}

/** Insert, Header or Footer: one of Docxtor's designs in place of what the section's header or footer holds. */
export interface InsertPagePartOp {
  op: "insert_page_part";
  kind: "header" | "footer";
  design: string;
  section?: number;
  labels?: GalleryLabels;
}

/** A footnote or endnote whose mark is text you choose (`*`, a symbol) instead of a number. */
export interface InsertCustomNoteOp {
  op: "insert_custom_note";
  at: MPos;
  kind: "footnote" | "endnote";
  mark: string;
  text?: string;
}

/**
 * How footnotes or endnotes are numbered, for the whole document (`w:footnotePr` / `w:endnotePr` in settings.xml, and
 * in any section that has its own). `symbol` is Word's `chicago` (*, †, ‡, §, then doubled). Without it footnotes are
 * 1, 2, 3 and endnotes i, ii, iii. `start` is the number of the first note.
 */
export interface SetNoteFormatOp {
  op: "set_note_format";
  kind: "footnote" | "endnote";
  format: "decimal" | "lowerRoman" | "upperRoman" | "lowerLetter" | "upperLetter" | "symbol";
  start?: number;
}

/**
 * A caption paragraph (style `Caption`) next to a block: `Figure ` + a `SEQ Figure \* ARABIC` field + `: text`.
 * Give `after` (caption below) or `before` (caption above); with `position`, the one block given is the thing being
 * captioned and `position` says where the caption goes. Numbers are per label, in document order, and are renumbered
 * at once. The `caret` is at the end of the caption; its paragraph id is in `created`. `separator` goes between the
 * number and the text (default `": "`).
 */
export interface InsertCaptionOp {
  op: "insert_caption";
  after?: number | string;
  before?: number | string;
  label: string;
  text?: string;
  position?: "above" | "below";
  separator?: string;
  /** Number the caption with its chapter ("Figure 2-1"): the heading level whose number it is, and what goes between. */
  chapter?: { level: number; separator?: string };
}

/** What a cross-reference points at. Ids and names come from `Engine.xrefTargets()`. */
export type XrefTarget =
  | { type: "heading"; p: number | string }
  | { type: "numbered"; p: number | string }
  | { type: "bookmark"; name: string }
  | { type: "caption"; p: number | string }
  | { type: "footnote"; id: string }
  | { type: "endnote"; id: string };

/**
 * What the reference shows: `text` (the heading, the bookmarked text, the whole caption), `number` (a heading or list
 * paragraph's number, a caption's number, a note's number), `number_no_context` / `number_full_context` (the paragraph
 * number alone / with all its levels), `page` (the page of the target), `label_number` (a caption's "Figure 1"),
 * `caption_text` (a caption's text after its number), `number_formatted` (a note's number drawn like its mark).
 */
export type XrefShow = "text" | "number" | "number_no_context" | "number_full_context" | "page" | "label_number" | "caption_text" | "number_formatted";

/**
 * A cross-reference field at a position, its result computed now. Word's markup: `REF _Ref123456789 \h` (with `\r`,
 * `\n` or `\w` for numbers), `PAGEREF _Ref… \h`, `NOTEREF _Ref… \h [\f]`, and a hidden `_Ref` bookmark around the target
 * (made if the target doesn't have one). `link` (default true) adds `\h`; `above_below` adds a second field
 * (`REF _Ref… \p \h`) after a space, which says "above" or "below". `pages` gives pages for `page`. The `caret` is
 * after the inserted text. Main text only.
 */
export interface InsertCrossReferenceOp {
  op: "insert_cross_reference";
  at: MPos;
  target: XrefTarget;
  show: XrefShow;
  link?: boolean;
  above_below?: boolean;
  pages?: PageMap;
}

/**
 * Recompute the results of `SEQ`, `REF`, `PAGEREF` and `NOTEREF` fields (the sequence numbers first) and rebuild the
 * table of contents, tables of figures and the index. Everything else (`DATE`, `PAGE`, unknown fields) is left alone,
 * and a document with nothing to update is written back unchanged. `scope` limits it to fields in those paragraphs.
 * The result's `count` is the number of fields or lists that changed.
 */
export interface UpdateFieldsOp {
  op: "update_fields";
  pages?: PageMap;
  scope?: (number | string)[];
  /** Leave no undo step when nothing changed (updating before printing or downloading). */
  quiet?: boolean;
  /** As when the file opens: dates, dirty fields, and every field when the file asks for it. Leaves no undo step. */
  open?: boolean;
  /** Only fields with these codes. */
  only?: string[];
  input?: FieldInput;
}

/** What the engine can't know and the page tells it when fields are worked out. */
export interface FieldInput {
  /** The time now, as the reader's clock shows it: `2026-10-13T09:30:00`. */
  now?: string;
  user_name?: string;
  user_initials?: string;
  user_address?: string;
  file_name?: string;
  file_path?: string;
  file_size?: number;
  /** How many pages the layout has. */
  pages?: number;
}

/** A field found by `Engine.listFields`. */
export interface FieldInfo {
  p: number;
  o: number;
  instr: string;
  result: string;
  code: string;
  locked: boolean;
  plain: boolean;
  story: boolean;
}

/** Change the code of the field that starts at `at` and work its result out again. */
export interface SetFieldCodeOp {
  op: "set_field_code";
  at: MPos;
  instr: string;
  input?: FieldInput;
  pages?: PageMap;
}

/** Replace the plain-text result of the field that starts at `at`. */
export interface SetFieldResultOp {
  op: "set_field_result";
  at: MPos;
  text: string;
}

/** A hidden index entry (`XE` field) at a position. `sub` is a sub-entry; `see` replaces the page number, e.g. `"See Fruit"`. */
export interface MarkIndexEntryOp {
  op: "mark_index_entry";
  at: MPos;
  entry: string;
  sub?: string;
  see?: string;
  bold?: boolean;
  italic?: boolean;
}

/** Mark the first occurrence of `text` in every paragraph as an index entry (Word's Mark All). The result's `count` is how many were marked. */
export interface MarkAllIndexEntriesOp {
  op: "mark_all_index_entries";
  text: string;
  entry?: string;
  sub?: string;
  see?: string;
  bold?: boolean;
  italic?: boolean;
  ignore_case?: boolean;
  whole_word?: boolean;
}

/**
 * An index (`INDEX \e "<tab>" \c "2" \z "1033"`) after or before a block (default: the end), in a continuous section of
 * its own with `columns` columns (default 2, max 4). Entries are sorted, sub-entries use styles `Index 2`…, page numbers
 * merge into ranges (`4–6`), "See" text replaces page numbers. `run_in` puts sub-entries in their entry's paragraph;
 * `headings` adds a letter heading before each letter. Rebuilt by `update_fields`.
 */
export interface InsertIndexOp {
  op: "insert_index";
  after?: number | string;
  before?: number | string;
  columns?: number;
  run_in?: boolean;
  headings?: boolean;
  pages?: PageMap;
}

/** A table of figures (`TOC \h \z \c "Figure"`) listing the captions with this label (default `Figure`), in style `Table of Figures`. */
export interface InsertTableOfFiguresOp {
  op: "insert_table_of_figures";
  after?: number | string;
  before?: number | string;
  label?: string;
  pages?: PageMap;
}

export interface XrefHeading {
  p: number;
  text: string;
  level: number;
  /** The heading's list number, when it is numbered (`1.2`). */
  number?: string;
}

export interface XrefBookmark {
  name: string;
  text: string;
}

export interface XrefCaption {
  p: number;
  /** `Figure`, `Table`, … */
  label: string;
  /** The number as shown. */
  number: string;
  /** The whole caption. */
  text: string;
}

export interface XrefNote {
  /** The id a `footnote` / `endnote` target takes. */
  id: string;
  num: number;
  /** The number as the document formats it. */
  label: string;
  /** The start of the note's text. */
  text: string;
}

/** Everything a cross-reference can point at, in document order. See `Engine.xrefTargets()`. */
export interface XrefTargets {
  headings: XrefHeading[];
  bookmarks: XrefBookmark[];
  captions: XrefCaption[];
  footnotes: XrefNote[];
  endnotes: XrefNote[];
}

/** A numbered paragraph a cross-reference can point at (`Engine.numberedItems()`). */
export interface NumberedItem {
  p: number;
  /** Its own number (`ii`). */
  number: string;
  /** The number with the levels above it (`1.a.ii`). */
  full: string;
  text: string;
  /** The list level, 0 for the outermost. */
  level: number;
}

/** The line between an entry and its page number in a table of contents or index. */
export type TabLeader = "dot" | "hyphen" | "underscore" | "none";

/** What the "Table of authorities" dialog sets (`Engine.toaOptions()`, the `toa` op's `insert` and `set` actions). */
export interface ToaOptions {
  /** 0 lists every category, 1 to 16 one of them. */
  category?: number;
  /** "passim" in place of five or more page numbers. */
  passim?: boolean;
  /** Entries keep the formatting of their long citation. */
  keepFormatting?: boolean;
  /** A heading for each category (default on). */
  headings?: boolean;
  /** Page numbers at the right edge, after a leader (default on). */
  rightAlign?: boolean;
  leader?: "none" | "dot" | "hyphen" | "underscore";
  pageSep?: string;
  rangeSep?: string;
  /** A sequence (`SEQ` name) whose number goes before each page number (`\s`), and the text between (`\d`). */
  seq?: string;
  seqSep?: string;
  /** List only the citations inside this bookmark (`\b`). */
  bookmark?: string;
  /** `classic`, `distinctive`, `formal` or `simple`; none leaves the styles as they are. */
  look?: string;
}

/**
 * A document tab (Google Docs' tabs): `start` is its first top-level block, `blocks` how many body blocks it holds.
 * `ids` are the blocks it shows, in the units of `RenderOut.order` (invisible body elements left out, an imported
 * chunk as its own blocks): a pane hides every block of the document that a tab's `ids` don't list.
 */
export interface TabInfo {
  id: string;
  name: string;
  icon?: string;
  parent?: string;
  /** 0 for the top level, up to 2. */
  depth: number;
  start: number;
  blocks: number;
  ids: number[];
}

/** The tabs of a document and the one the file was saved on (a pane keeps its own open tab). A document without tabs has none. */
export interface TabsInfo {
  tabs: TabInfo[];
  active?: string;
}

/** What the "Table of contents" dialog sets (`Engine.tocOptions()`, the `insert_toc` and `set_toc` ops). */
export interface TocOptions {
  /** Deepest built-in heading level to list; 0 lists none. */
  levels: number;
  /** Also list paragraphs that have an outline level. */
  outline: boolean;
  /** Other styles and the level each one makes. */
  styles: { style: string; level: number }[];
  /** List table entry (`TC`) fields. */
  entries: boolean;
  entryId?: string;
  links: boolean;
  pageNumbers: boolean;
  rightAlign: boolean;
  leader: TabLeader;
  /** Levels without a page number, `[first, last]`. */
  hidePages?: [number, number];
  /** `classic`, `formal`, `modern` or `simple`; leave out to keep the document's TOC styles. */
  look?: string;
}

/** What the "Index" dialog sets (`Engine.indexOptions()`, the `insert_index` and `set_index` ops). */
export interface IndexOptions {
  columns: number;
  runIn: boolean;
  headings: boolean;
  rightAlign: boolean;
  leader: TabLeader;
  pageSep?: string;
  rangeSep?: string;
  /** A Windows locale id (3082 Spanish): decides how entries sort. */
  language?: number;
  kind?: string;
  look?: string;
}

/** One line of an AutoMark file: the text to mark and its index entry (`Fruit:Apple`). */
export interface AutoMarkEntry {
  text: string;
  entry?: string;
  bold?: boolean;
  italic?: boolean;
}

/** A building block (quick part) of the document's glossary (`word/glossary/document.xml`). */
export interface BuildingBlock {
  index: number;
  name: string;
  /** `autoText`, `docParts`, `tblOfContents`, `coverPg`, `custQuickParts`... */
  gallery?: string;
  category?: string;
  description?: string;
  guid?: string;
  text: string;
  /** A run of text that goes in at the caret, not whole paragraphs. */
  inline: boolean;
}

/** A content control made from a gallery (`w:docPartObj`). */
export interface DocPartControl {
  p: number;
  gallery: string;
  category?: string;
  title?: string;
  unique: boolean;
}

