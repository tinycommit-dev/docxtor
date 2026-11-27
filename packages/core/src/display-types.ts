// SmartArt and WordArt: what the engine reports besides the shapes themselves (`RShape.diagram`, `RShape.dtext`,
// `RShape.warp`). Read only: the editor draws them; nothing here is an op.
import type { RFill, RInsets, RLine } from "./shapes.ts";

/** Where the shapes of a SmartArt graphic (`RShape.children` of a shape with `kind: "diagram"`) came from. */
export interface RDiagram {
  /** `drawing` (the drawing Word saved with the graphic), `layout` (laid out from its data model) or `placeholder` (nothing to lay out). */
  source: "drawing" | "layout" | "placeholder";
  /** The last part of the layout's id (`default`, `process1`, `cycle2`, ...) when the file names one. */
  layout?: string;
}

/** The text of a shape in a diagram, drawn read only. Positions are points inside the shape's box. */
export interface RDText {
  /** The text rectangle. */
  x: number;
  y: number;
  w: number;
  h: number;
  insets: RInsets;
  vAnchor: "top" | "middle" | "bottom";
  /** Lines wrap at the rectangle's width; false for one line. */
  wrap: boolean;
  /** Degrees; the text turns about the middle of its rectangle. */
  rot?: number;
  paras: RDPara[];
}

export interface RDPara {
  align: "left" | "center" | "right" | "justify";
  /** Line spacing as a percentage of the font's own (90 is tight); omitted for single. */
  linePct?: number;
  /** Space before and after, points. */
  before?: number;
  after?: number;
  /** Left margin and the first line's extra indent (negative for a hanging bullet), points. */
  margin?: number;
  indent?: number;
  bullet?: string;
  runs: RDRun[];
}

export interface RDRun {
  text: string;
  /** Points. */
  size: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  /** Hex; omitted for black. */
  color?: string;
  font?: string;
}

/** Text bent to a preset (WordArt). */
export interface RWarp {
  /** A DrawingML preset: `textPlain`, `textArchUp`, `textWave1`, ... (the 40 of `ST_TextShapeType`). */
  preset: string;
  /** The preset's adjust values in the file's units. */
  adj?: Record<string, number>;
  /** `drawingml`: the text is the shape's own (`RShape.text`). `vml`: the text is `text`. */
  source: "drawingml" | "vml";
  text?: string;
  font?: string;
  bold?: boolean;
  italic?: boolean;
  /** The text fill and outline; omitted when the text has none of its own. */
  fill?: RFill;
  line?: RLine;
  shadow?: { color: string; alpha: number; blurPt: number; dx: number; dy: number };
  glow?: { color: string; alpha: number; radiusPt: number };
  /** A reflection under the text: it fades from `startAlpha` to `endAlpha` over `size` percent of the text's height. */
  reflection?: { startAlpha: number; endAlpha: number; size: number; distPt: number };
}
