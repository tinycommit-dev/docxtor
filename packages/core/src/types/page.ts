// Page setup, sections, columns, bookmarks, notes and stories
import type { PageNumberFormat } from "./document-ops.ts";
import type { RBlock } from "./render.ts";
import type { DocGrid } from "../east-asian-types.ts";

export interface PageSetup {
  width: number;
  height: number;
  margin: [number, number, number, number];
  header: number;
  footer: number;
  cols: number;
  titlePage: boolean;
  headers: Record<string, RStory>;
  footers: Record<string, RStory>;
  /** Footnotes in reference order. */
  notes: RNote[];
  /** Endnotes in reference order, for the end of the document. */
  endnotes: RNote[];
  /** Every section in document order; the fields above describe the last one. */
  sections: SectionInfo[];
  /** Every header and footer story by part name (what `sections[].headers` and `footers` point to). */
  parts: Record<string, RStory>;
  /** Page background colour (hex), if the document sets one. */
  pageColor?: string;
  hyphenation: boolean;
  /** The default tab stop width in twips (`defaultTabStop`, 720 unless the file says otherwise). */
  defaultTab: number;
  evenAndOddHeaders: boolean;
  /** The document's default language, e.g. `en-US`. */
  lang?: string;
  /** The watermark: the last section's, else the first section's that has one (see `sections[].watermark`). */
  watermark?: Watermark;
  /** Inside and outside margins instead of left and right. */
  mirrorMargins?: boolean;
  /** The gutter is at the top of the page. */
  gutterAtTop?: boolean;
  bookFold?: "bookFold" | "bookFoldRev";
  bookFoldSheets?: number;
  printTwoOnOne?: boolean;
  /** Hyphenation: the zone and the most hyphens in a row (twips, count), and capitals left alone. */
  hyphenationZone?: number;
  consecutiveHyphenLimit?: number;
  doNotHyphenateCaps?: boolean;
  displayBackgroundShape?: boolean;
  /** Spelling and grammar underlines are hidden in this document (Word's "Hide spelling errors in this document only"). */
  hideSpellingErrors?: boolean;
  hideGrammaticalErrors?: boolean;
  /** The document's own note options (before any section's). */
  footnotePr?: NotePr;
  endnotePr?: NotePr;
  /** The page background when the document has one. */
  pageFill?: PageFill;
  /** The notes parts' separators that differ from Word's own. */
  noteSeps?: { footnote?: KindSeps; endnote?: KindSeps };
}

/** A watermark found in a header: Word's text watermark (a WordArt shape) or a picture. */
export interface Watermark {
  kind: "text" | "picture";
  text?: string;
  font?: string;
  /** Hex without `#`. */
  color?: string;
  /** 0 to 1; absent means opaque. */
  opacity?: number;
  layout: WatermarkLayout;
  /** Degrees clockwise as the file has it (Word's diagonal is 315). */
  angle: number;
  /** The font size in points when the file sets one; otherwise the height of the shape (Word fits the text to the shape). */
  size?: number;
  /** The shape's size in points. */
  width?: number;
  height?: number;
  /** For pictures: the image's part name (`word/media/image1.png`). */
  media?: string;
  /** For pictures: whether Word's Washout is on (the image data has `gain` and `blacklevel`). */
  washout?: boolean;
}

export type WatermarkLayout = "diagonal" | "horizontal";

/** A bookmark people can see; `Engine.bookmarks()` lists them in document order (body first, then headers, footers and notes). */
export interface BookmarkInfo {
  name: string;
  /** The paragraph where the bookmark starts and the UTF-16 offset of its start. */
  p: number;
  o: number;
  /** Where it ends; the same position for a point bookmark. */
  endP: number;
  endO: number;
  /** The bookmarked text (the start of the paragraph for a point bookmark), about 60 characters at most. */
  text: string;
  /** Where it lives when not in the main text: a header, footer or note. */
  story?: BookmarkStory;
}

/** The header, footer or note a bookmark is in. */
export interface BookmarkStory {
  kind: "header" | "footer" | "footnote" | "endnote";
  /** The package part, such as `word/header1.xml`. */
  part: string;
  /** For a note, its id (as in `PageSetup.notes[].id`). */
  noteId?: string;
}

/** How a section starts: `nextPage` (the default), `continuous`, `evenPage`, `oddPage` or `nextColumn`. */
export type SectionStart = "nextPage" | "continuous" | "evenPage" | "oddPage" | "nextColumn";

export interface SectionInfo {
  /** The paragraph carrying the section's properties; absent for the final section. */
  end?: number;
  start: SectionStart | string;
  width: number;
  height: number;
  /** top, right, bottom, left (twips) */
  margin: [number, number, number, number];
  header: number;
  footer: number;
  /** Extra margin for binding (twips), and whether it sits on the right (`rtlGutter`). */
  gutter?: number;
  rtlGutter?: boolean;
  cols: Columns;
  lineNumbers?: LineNumbers;
  titlePage: boolean;
  /** The page number of the section's first page, when the section restarts the numbering. */
  pageStart?: number;
  /** Page number format; absent for 1, 2, 3. */
  pageFormat?: PageNumberFormat;
  /** Header part names by variant (`default`, `first`, `even`), including ones inherited from earlier sections. */
  headers: Record<string, string>;
  footers: Record<string, string>;
  /**
   * The header and footer variants (`default`, `first`, `even`) this section shows from the section before
   * ("Link to previous" is on). Empty for the first section, which has nothing to link to.
   */
  linkedHeaders?: string[];
  linkedFooters?: string[];
  /** The watermark in the headers this section shows (default, then first-page, then even-page header). */
  watermark?: Watermark;
  /** Where the text sits between the top and bottom margins; absent is the top. */
  vAlign?: "center" | "both" | "bottom";
  /** `tbRl`, `btLr` and the like; absent is left to right, top to bottom. */
  textDirection?: string;
  pageBorders?: PageBorders;
  /** Chapter page numbers: the Heading level whose number goes before the page number, and the character between. */
  chapStyle?: number;
  chapSep?: "hyphen" | "period" | "colon" | "emDash" | "enDash";
  /** The section's note options (the document's, where the section says nothing). */
  footnotePr?: NotePr;
  endnotePr?: NotePr;
  noEndnote?: boolean;
  /** The document grid (lines of a fixed pitch, with characters a fixed width per character), and the size its character width is added to (points). */
  docGrid?: DocGrid;
  gridBase?: number;
}

/** How a kind of note is numbered and where it goes. */
export interface NotePr {
  fmt: string;
  start: number;
  restart: "continuous" | "eachSect" | "eachPage";
  /** Footnotes: `pageBottom` or `beneathText`. Endnotes: `sectEnd` or `docEnd`. */
  pos: string;
}

export interface PageBorder {
  /** A line style (`single`, `double`...) or an art border name (`hearts`). */
  val: string;
  /** Eighths of a point (an art border: points). */
  sz: number;
  /** Points from the text or the page edge. */
  space: number;
  color: string;
  shadow?: boolean;
  frame?: boolean;
}

export interface PageBorders {
  offsetFrom: "page" | "text";
  display: "allPages" | "firstPage" | "notFirstPage";
  zOrder: "front" | "back";
  top?: PageBorder;
  left?: PageBorder;
  bottom?: PageBorder;
  right?: PageBorder;
}

/** The page background: a colour, a gradient, a pattern, a texture or a picture. */
export interface PageFill {
  kind: "solid" | "gradient" | "radial" | "pattern" | "texture" | "picture";
  color?: string;
  color2?: string;
  /** VML degrees: 0 changes from top to bottom, -90 from left to right. */
  angle?: number;
  from?: "center" | "topLeft" | "topRight" | "bottomLeft" | "bottomRight";
  /** The image part for a pattern, texture or picture. */
  media?: string;
  title?: string;
}

/** `w:framePr`. Sizes and distances in twips. */
export interface FrameFmt {
  dropCap?: "drop" | "margin";
  lines?: number;
  w?: number;
  h?: number;
  hRule?: "auto" | "exact" | "atLeast";
  hSpace?: number;
  vSpace?: number;
  wrap?: "auto" | "notBeside" | "around" | "tight" | "through" | "none";
  hAnchor?: "text" | "margin" | "page";
  vAnchor?: "text" | "margin" | "page";
  x?: number;
  y?: number;
  xAlign?: "left" | "center" | "right" | "inside" | "outside";
  yAlign?: "inline" | "top" | "center" | "bottom" | "inside" | "outside";
  anchorLock?: boolean;
}

/** One separator note: whether it still draws Word's line, and what else it holds. */
export interface NoteSep {
  line: boolean;
  blocks?: RBlock[];
}

export interface KindSeps {
  separator?: NoteSep;
  continuation?: NoteSep;
  notice?: NoteSep;
}

export interface Columns {
  num: number;
  /** Gap between columns (twips). */
  space: number;
  sep: boolean;
  /** Column widths (twips) when unequal; empty otherwise. */
  widths: number[];
  /** The space after each of those columns (twips), one for each but the last. */
  spaces?: number[];
}

export interface LineNumbers {
  countBy: number;
  start: number;
  /** `newPage`, `newSection` or `continuous`. */
  restart: string;
  /** Distance from the text (twips). */
  distance?: number;
}

export interface RStory {
  part: string;
  blocks: RBlock[];
}

export interface RNote {
  /** The note's id in its part (what `footnote` and `endnote` cross-reference targets take). */
  id: string;
  /** 1-based position in reference order. */
  num: number;
  /** The number as the document formats it: `1`, `iv`, `C`, `*` (see `set_note_format`). */
  label: string;
  blocks: RBlock[];
}

