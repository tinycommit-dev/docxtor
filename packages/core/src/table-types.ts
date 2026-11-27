// Types for the table dialogs and the table ops (properties, rows, cells, borders, splitting, sorting, converting).
import type { Border, TableBorders } from "./types.ts";

/** A preferred width: automatic, twips (`dxa`) or fiftieths of a percent (`pct`, 5000 is all of the width). */
export interface TableWidth {
  type: "auto" | "dxa" | "pct";
  value: number;
}

/** One edge of a table or cell for `table_format` and the `borders` cell action: `none` draws no line, `clear` takes the setting off. */
export interface BorderEdge {
  val: string;
  /** Eighths of a point. */
  sz?: number;
  /** Hex, or `auto`. */
  color?: string;
  space?: number;
}

/** Border edges by name; one left out stays as it is. `insideH` and `insideV` are the lines between rows and between columns. */
export interface BorderEdges {
  top?: BorderEdge;
  bottom?: BorderEdge;
  left?: BorderEdge;
  right?: BorderEdge;
  insideH?: BorderEdge;
  insideV?: BorderEdge;
  /** Cells only: top left to bottom right, and top right to bottom left. */
  diagDown?: BorderEdge;
  diagUp?: BorderEdge;
}

/** `table_format`: table-wide properties; a field left out stays as it is. */
export interface TableFormatOp {
  op: "table_format";
  at: number;
  width?: TableWidth;
  align?: "left" | "center" | "right";
  indent?: number;
  wrap?: "none" | "around";
  autofit?: boolean;
  /** top, right, bottom, left */
  cellMargins?: [number, number, number, number];
  spacing?: number;
  title?: string;
  description?: string;
  borders?: BorderEdges;
}

/** `table_rows`: row properties for the rows from the one holding `from` to the one holding `to` (or every row with `all`). */
export interface TableRowsOp {
  op: "table_rows";
  from: number;
  to?: number;
  all?: boolean;
  height?: { value?: number; rule: "auto" | "atLeast" | "exact" };
  cantSplit?: boolean;
  header?: boolean;
}

/** What the table dialogs read (`Engine.tableDetails`). Lengths are twips. */
export interface TableDetails {
  table: {
    width: TableWidth;
    align: "left" | "center" | "right";
    indent: number;
    /** Text flows around the table. */
    wrap: boolean;
    autofit: boolean;
    /** Default cell margins: top, right, bottom, left. */
    cellMargins: [number, number, number, number];
    spacing: number;
    title: string;
    description: string;
    style?: string;
  };
  /** Grid column widths. */
  grid: number[];
  rows: { height: number; rule: "auto" | "atLeast" | "exact"; cantSplit: boolean; header: boolean }[];
  /** The row the paragraph is in, the grid column its cell starts at, and how many columns the cell spans. */
  row: number;
  col: number;
  span: number;
  cell: { width: TableWidth; valign: string; margins?: [number, number, number, number] | null; noWrap: boolean; fitText: boolean; textDirection: string };
  columnWidth: TableWidth;
  /** The borders the table draws, and the ones the cell draws. */
  borders: TableBorders;
  cellBorders: TableBorders;
  diagDown?: Border;
  diagUp?: Border;
  shading?: string;
}
