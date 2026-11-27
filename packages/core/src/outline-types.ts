// Types for the outline: the blocks of the main text with their levels, and the ops that change levels and order
// (see crates/docxtor-core/src/document/outline.rs).

/** One top-level block of the main text. */
export interface OutlineEntry {
  id: number;
  /** 0 is body text, a table or any other block; 1 to 9 are outline levels (headings). */
  level: number;
  /** A paragraph, a table, or a wrapper such as a content control. */
  kind: "p" | "t" | "o";
  /** The start of a paragraph's text. */
  text: string;
}

/** `set_outline_level`: give paragraphs a level, or move each by `delta` levels (negative promotes). */
export interface SetOutlineLevelOp {
  op: "set_outline_level";
  ids: number[];
  level?: number;
  delta?: number;
}

/** `move_blocks`: move top-level blocks, in their own order, to just after `after` or just before `before`. */
export interface MoveBlocksOp {
  op: "move_blocks";
  ids: number[];
  after?: number;
  before?: number;
}
