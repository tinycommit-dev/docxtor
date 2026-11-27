// The render model: segments, paragraphs, tables, blocks
import type { RControl, RForm } from "./controls.ts";
import type { Border, ListLabel, ParaFmt, RunFmt } from "./format.ts";
import type { RImage } from "./pictures.ts";
import type { RShape } from "../shapes.ts";
import type { ChartModel } from "../chart-types.ts";
import type { ChartExModel } from "../chartex-types.ts";
import type { RRuby } from "../east-asian-types.ts";

export interface RSeg {
  o: number;
  k: "t" | "tab" | "br" | "pbr" | "cbr" | "img" | "obj" | "ghost" | "note" | "shape" | "math" | "chart" | "sym" | "val" | "eq" | "ff" | "form" | "ruby" | "hrule";
  x?: string;
  f: RunFmt;
  link?: string;
  img?: RImage;
  label?: string;
  fld?: string;
  /** The whole instruction of a field, on the first piece of its result. */
  fi?: string;
  /** Text inside a field's code: it keeps its place but isn't shown. */
  hid?: boolean;
  /** For the result of a citation: the tags of the sources it cites, so the text can be treated as one unit. */
  cite?: string[];
  ins?: boolean;
  /** For note marks (`k: "note"`): `"endnote"` for an endnote, absent for a footnote. `x` is the note's id and `label` its formatted number (`1`, `iv`, `*`). */
  note?: "endnote";
  /** For shapes and text boxes (`k: "shape"`): geometry, colours, position and the text inside (see `RShape`). */
  shape?: RShape;
  /** For charts (`k: "chart"`): what to draw. `img` holds the size and placement; `img.src` is the chart part. */
  chart?: ChartModel;
  /** For the new chart types (`k: "chart"` too; waterfall, histogram, treemap...): what to draw. `img` holds the size and placement. */
  chartex?: ChartExModel;
  /** For an equation (`k: "math"`; one position, like an image). */
  math?: RMath;
  /** A drawing object that is hidden: not drawn or printed, still in the file. */
  hidden?: boolean;
  /** For a phonetic guide (`k: "ruby"`; one position, like an image): the base text, the guide and its settings. */
  ruby?: RRuby;
  /** For a horizontal line drawn as a VML rule (`k: "hrule"`; one position): Word's and Google Docs' horizontal lines. */
  hrule?: RHrule;
}

/** A horizontal line: a width (percent of the column, or points), a thickness in points, an alignment and a colour. */
export interface RHrule {
  pct?: number;
  w?: number;
  h: number;
  align: "left" | "center" | "right" | string;
  /** Hex without `#`; absent for Word's grey. */
  color?: string;
}

/** An equation as it is drawn and edited. */
export interface RMath {
  /** One MathML Core `<math>` element. Parse it with `DOMParser` and keep only MathML (see the editor's `mathNode`). */
  mathml: string;
  /** The equation as LaTeX (best effort for markup Word made that LaTeX can't say). */
  latex: string;
  /** On its own line (Word's `m:oMathPara`). */
  display?: boolean;
  /** Justification of a display equation: `center` (Word's default), `left`, `right` or `centerGroup`. */
  jc?: string;
}

/** What `Engine.equationAt` finds. */
export interface EquationAt {
  latex: string;
  display: boolean;
  /** For charts (`k: "chart"`): what to draw. `img` holds the size and placement; `img.src` is the chart part. */
  chart?: ChartModel;
}

export interface RPara {
  t: "p";
  id: number;
  style?: string;
  fmt: ParaFmt;
  mark: RunFmt;
  segs: RSeg[];
  len: number;
  list?: ListLabel;
  /** The section break after this paragraph: how the next section starts (`nextPage`, `continuous`, `evenPage`, `oddPage`, `nextColumn`). */
  sectBreak?: string;
  /** Bookmarks starting in this paragraph, hidden `_Toc…` and `_Ref…` ones included (targets of internal links, e.g. a table of contents). Use `Engine.bookmarks()` for the ones people see. */
  bookmarks?: string[];
  /** A horizontal line: an empty paragraph whose only border is one along the bottom. */
  hr?: boolean;
  /** The content controls inside the paragraph, outer before inner. `n` is the number to address one by in a `control` op. */
  controls?: RControl[];
  /** Legacy form fields (`FORMTEXT`, `FORMCHECKBOX`, `FORMDROPDOWN`) in the paragraph. A check box or drop-down is drawn from the `form` segment at its place. */
  forms?: RForm[];
  /** The language of the paragraph's text as a BCP 47 tag (`en-US`): its first run with text, else the paragraph mark, else its style, else the document default. Absent when the file gives none. */
  lang?: string;
}

export interface TableBorders {
  top?: Border;
  bottom?: Border;
  left?: Border;
  right?: Border;
  insideH?: Border;
  insideV?: Border;
}

export interface RCell {
  span: number;
  vmerge?: string;
  width?: number;
  shading?: string;
  valign?: string;
  /** The cell's own margins in twips (top, right, bottom, left), where they differ from the table's. */
  mar?: [number, number, number, number];
  /** Diagonal lines: top left to bottom right, and top right to bottom left. */
  diagDown?: Border;
  diagUp?: Border;
  /** The name of the content control around the cell (empty when it has none). */
  sdt?: string;
  /** The direction the text runs in: `tbRl` (down) or `btLr` (up); across is left out. */
  textDir?: string;
  /** The text stays on one line. */
  noWrap?: boolean;
  /** Each side settled by the table's style and the cell's own formatting; a border with `val` `nil` is no line. */
  borders: TableBorders;
  blocks: RBlock[];
}

export interface RTableRow {
  height?: number;
  /** `atLeast` or `exact`. */
  heightRule?: string;
  header?: boolean;
  /** The row moves to the next page whole instead of splitting. */
  cantSplit?: boolean;
  /** Grid columns left empty before the first cell and after the last. */
  gridBefore?: number;
  gridAfter?: number;
  /** The name of the content control around the row (empty when it has none). */
  sdt?: string;
  /** The innermost content control around the row (`ctl` addresses it), and the repeating section the row is an item of. */
  control?: RControl;
  section?: RControl;
  cells: RCell[];
}

/** Where a floating table sits and how close the text comes (twips). */
export interface RTableFloat {
  horzAnchor: string;
  vertAnchor: string;
  x?: number;
  y?: number;
  xAlign?: string;
  yAlign?: string;
  left: number;
  right: number;
  top: number;
  bottom: number;
  neverOverlap?: boolean;
}

/** The options of a table design (`w:tblLook`). */
export interface RTableLook {
  firstRow: boolean;
  lastRow: boolean;
  firstColumn: boolean;
  lastColumn: boolean;
  bandedRows: boolean;
  bandedColumns: boolean;
}

export interface RTable {
  t: "tbl";
  id: number;
  grid: number[];
  borders: TableBorders;
  align?: string;
  indent?: number;
  width?: number;
  widthPct?: boolean;
  cellMar: [number, number, number, number];
  /** Space between cells in twips. */
  spacing?: number;
  /** The columns keep the grid's widths (otherwise they grow to what their content needs). */
  fixed?: boolean;
  float?: RTableFloat;
  /** The table style's id and name, and which parts of it the table shows. */
  style?: string;
  styleName?: string;
  look?: RTableLook;
  /** A right-to-left table (`bidiVisual`): the first column is on the right. */
  bidiVisual?: boolean;
  rows: RTableRow[];
}

export interface RContainer {
  t: "c";
  id: number;
  label?: string;
  /** The content control's building block gallery (`Bibliographies` for a bibliography). */
  gallery?: string;
  /** What the content control is (see `RControl`); `ctl` is the id to address it by. */
  control?: RControl;
  blocks: RBlock[];
}

export interface ROpaque {
  t: "x";
  id: number;
  label?: string;
}

export type RBlock = RPara | RTable | RContainer | ROpaque;

export interface RenderOut {
  /**
   * The visible top-level block ids in order. Absent when `orderKept` (typing moves no block: the client keeps the
   * list it has), and an absent `order` without `orderKept` is an empty list (nothing is visible).
   */
  order?: number[];
  /** Only in a changed render: the order is the one the last render gave. */
  orderKept?: boolean;
  blocks: RBlock[];
  full: boolean;
  /** Headers, footers, notes or the page layout changed: re-fetch the page setup. */
  stories: boolean;
}

