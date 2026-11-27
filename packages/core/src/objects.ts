// Drawing objects as a list (the Selection pane): what `Engine.objects()` returns, and the ops that name, hide,
// place and stack any shape, text box, picture, chart or group.
import type { MPos } from "./types.ts";

/** What kind of drawing an object is. */
export type ObjectKind = "shape" | "textbox" | "picture" | "chart" | "group" | "canvas";

/** One drawing object of the main text. It occupies `[o, o+1)` of paragraph `p`. */
export interface ObjectInfo {
  p: number;
  o: number;
  kind: ObjectKind;
  /** The name Word shows in the Selection pane (a default such as `Picture 2` when the file gives none). */
  name: string;
  /** The drawing's id (`wp:docPr`), 0 if there is none. */
  id: number;
  /** Hidden: not drawn or printed, still in the file. Omitted when shown. */
  hidden?: boolean;
  /** inline | square | tight | topBottom | behind | front */
  wrap: string;
  /** The object floats over the text, so it has a place in the stacking order. */
  floating: boolean;
  /** The stacking value (`relativeHeight`) of a floating object: the higher, the nearer. */
  z?: number;
  /** Drawn behind the text (a different stack from the objects in front of it). */
  behind?: boolean;
  /** Where a floating object sits: what the position counts from (`margin`, `page`, `column`, `paragraph`, ...), an alignment from that, or an offset in points. */
  relX?: string;
  relY?: string;
  align?: string;
  alignY?: string;
  x?: number;
  y?: number;
}

/** Name an object or hide it, or show it again. */
export interface SetObjectPropsOp {
  op: "set_object_props";
  at: MPos;
  name?: string;
  hidden?: boolean;
}

/**
 * Place a floating object the way Word does: `rel_x` / `rel_y` say what the position counts from, `align` /
 * `align_y` align it within that, and `x` / `y` are offsets in points when there is no alignment.
 */
export interface SetObjectPositionOp {
  op: "set_object_position";
  at: MPos;
  rel_x?: "margin" | "page" | "column" | "character" | "leftMargin" | "rightMargin" | "insideMargin" | "outsideMargin";
  align?: "left" | "center" | "right" | "inside" | "outside";
  x?: number;
  rel_y?: "margin" | "page" | "paragraph" | "line" | "topMargin" | "bottomMargin" | "insideMargin" | "outsideMargin";
  align_y?: "top" | "center" | "bottom" | "inside" | "outside";
  y?: number;
}

/** Put a floating object directly above or below another one of the same stack (behind the text, or in front of it). */
export interface MoveObjectOp {
  op: "move_object";
  at: MPos;
  above?: MPos;
  below?: MPos;
}
