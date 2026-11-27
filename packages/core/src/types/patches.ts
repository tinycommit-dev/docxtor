// Format state, run and paragraph patches, table designs, fragments
import type { Border, ParaBorders, ParaFmt, RunFmt, Shd, TabStop } from "./format.ts";
import type { EaParaPatch, EaRunPatch } from "../east-asian-types.ts";

export interface FormatState {
  run: RunFmt;
  para: ParaFmt;
  style?: string;
  /** The character style (`w:rStyle`) of the text at the position. */
  charStyle?: string;
  list?: "bullet" | "decimal" | "checklist";
  /** The list style preset (`disc`, `decimal`, `checklist`, ...) when Docxtor made the list; checked checklist items report `checklist`. */
  listStyle?: string;
  /** For checklist items: whether the item is checked. */
  checked?: boolean;
  /** The list the paragraph is in (its numId) and its level, for the list dialogs and galleries. */
  listNum?: number;
  listLevel?: number;
  /** The count the paragraph's label shows (3 for "c)"). */
  listValue?: number;
  link?: string;
  inTable: boolean;
  /** Index into `PageSetup.sections` of the section holding the position. */
  section: number;
}

export interface StyleInfo {
  id: string;
  name: string;
  kind: string;
  basedOn?: string;
  next?: string;
  priority: number;
  hidden: boolean;
  quick: boolean;
  /** Made by a person, not one of Word's built-in styles. */
  custom?: boolean;
  /** The style it is linked to: a paragraph style and its character style apply as one in Word. */
  link?: string;
}

export interface MPos {
  p: number;
  o: number;
}

export interface OpResult {
  caret?: { p: number; o: number };
  created?: number[];
  count?: number;
}

export type RunPatch = Partial<{
  /** A character style by id or name; a linked paragraph style applies as its character style; empty clears. */
  style: string;
  bold: boolean;
  italic: boolean;
  /** `single`, `words`, `double`, `thick`, `dotted`, ... `none` (see `UNDERLINES` in the editor). */
  underline: string;
  /** Hex, or `auto` for the text colour. */
  underlineColor: string;
  underlineTheme: string;
  underlineTint: string;
  underlineShade: string;
  strike: boolean;
  dstrike: boolean;
  caps: boolean;
  smallCaps: boolean;
  vanish: boolean;
  /** Do not check spelling or grammar. */
  noProof: boolean;
  outline: boolean;
  shadow: boolean;
  emboss: boolean;
  imprint: boolean;
  font: string;
  size: number;
  /** Hex, or `auto`. */
  color: string;
  /** A theme colour written beside `color` (its resolved hex); tint and shade are hex 00 to FF. */
  themeColor: string;
  themeTint: string;
  themeShade: string;
  highlight: string;
  vertAlign: string;
  /** Expanded (positive) or condensed (negative), in twentieths of a point. */
  spacing: number;
  /** Raised (positive) or lowered (negative), in half-points. */
  position: number;
  /** Percent; 100 is normal. */
  scale: number;
  /** Half-points; 0 is off. */
  kern: number;
  /** A flat shading colour (hex), or `none`. */
  shading: string;
  shd: Shd;
  border: Border;
  em: string;
  fitText: number;
  ligatures: string;
  numForm: string;
  numSpacing: string;
  styleSets: number[];
  cntxtAlts: boolean;
  clear: boolean;
}> &
  EaRunPatch;

export type ParaPatch = Partial<{
  style: string;
  align: string;
  spaceBefore: number;
  spaceAfter: number;
  lineMultiple: number;
  indLeft: number;
  indRight: number;
  indFirst: number;
  keepNext: boolean;
  keepLines: boolean;
  pageBreakBefore: boolean;
  widowControl: boolean;
  suppressLineNumbers: boolean;
  suppressAutoHyphens: boolean;
  /** Line spacing as Word stores it: with `lineRule` `auto` in 240ths of a line, `exact` and `atLeast` in twips. */
  line: number;
  lineRule: "auto" | "exact" | "atLeast";
  /** Automatic spacing before or after (what Word uses for HTML); false removes it. */
  beforeAuto: boolean;
  afterAuto: boolean;
  /** "Don't add space between paragraphs of the same style". */
  contextualSpacing: boolean;
  mirrorIndents: boolean;
  /** 0 to 8 are outline levels 1 to 9, 9 is body text, negative removes the setting. */
  outlineLvl: number;
  /** Right to left. */
  bidi: boolean;
  /** The tab stops the paragraph should have, inherited ones included. */
  tabs: TabStop[];
  /** Shading with a pattern; replaces `shading` when set. `{ val: "nil" }` removes it. */
  shd: Shd;
  /** Background fill (hex); an empty string removes it. */
  shading: string;
  /** Replaces the paragraph borders; no sides removes them. */
  borders: ParaBorders;
  num: [number, number];
  clear: boolean;
}> &
  EaParaPatch;

export type Op = { op: string; [k: string]: unknown };

/** Table style options: a field left out stays as it is. */
export interface TableLookPatch {
  firstRow?: boolean;
  lastRow?: boolean;
  firstColumn?: boolean;
  lastColumn?: boolean;
  bandedRows?: boolean;
  bandedColumns?: boolean;
}

/** `table_design`: give the table holding paragraph `at` one of Word's designs and/or switch parts of it on or off. */
export interface TableDesignOp {
  op: "table_design";
  at: number;
  /** A design's style id (`GridTable4-Accent1`) or name (`Grid Table 4 Accent 1`). */
  style?: string;
  /** A design an extension offers: one `w:style` of type table as XML. The document gets it if it has no style of that id. */
  definition?: string;
  look?: TableLookPatch;
}

/** One side of a cell in a design preview: hex colour and width in points. */
export type PreviewLine = [string, number];

/** A cell of a design drawn small. */
export interface PreviewCell {
  fill?: string;
  top?: PreviewLine;
  bottom?: PreviewLine;
  left?: PreviewLine;
  right?: PreviewLine;
  bold: boolean;
  /** Hex colour of the text. */
  color: string;
}

/** One of Word's built-in table designs, drawn as rows of cells. */
export interface TableDesign {
  /** The style id (`GridTable4-Accent1`). */
  id: string;
  /** The name people see (`Grid Table 4 Accent 1`). */
  name: string;
  /** `custom` for a design an extension offers. */
  group: "plain" | "grid" | "list" | "custom";
  /** 1 to 6, absent for the grey one. */
  accent?: number;
  cells: PreviewCell[][];
}

/**
 * Content that moves between places: the Docxtor clipboard format and what the HTML and RTF readers make.
 * `body` is Word's `w:p` and `w:tbl` elements as XML text; pictures and links in it are named by keys that
 * `media` and `links` define. Insert one with the `insert_fragment` op.
 */
export interface Fragment {
  version: number;
  ns: [string, string][];
  body: string;
  styles: string;
  numbering: string;
  media: Record<string, { ext: string; data: string }>;
  links: Record<string, string>;
  notes: Record<string, { kind: string; xml: string }>;
  /** The content is whole paragraphs, not a run of text inside one. */
  block: boolean;
  reuseLists: boolean;
  defaults?: { font?: string; size?: number } | null;
  /** Things to tell the person: `picture-unreadable`, `picture-unsupported`, `object-dropped`, each with a count. */
  warnings: { kind: string; count: number }[];
  /** A secret that only this page load's engine knows, set on fragments it copies. A paste that sees it keeps what a stranger's clipboard may not (links to `file:` addresses). Leave it alone. */
  origin?: string;
}

