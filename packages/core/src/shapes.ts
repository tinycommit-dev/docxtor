// Shapes and text boxes: what the engine reports (`RSeg.shape`, `Engine.shapeAt`) and the ops that make and change them.
import type { MPos, RBlock } from "./types.ts";
import type { RDText, RDiagram, RWarp } from "./display-types.ts";

/** A gradient stop: `pos` 0 to 100, `alpha` 0 to 1 (omitted when opaque). */
export interface RStop {
  pos: number;
  color: string;
  alpha?: number;
}

/** Four percentages from the left, top, right and bottom edges (can be negative or above 100). */
export interface RRect4 {
  l: number;
  t: number;
  r: number;
  b: number;
}

/** A tiled picture fill: `tx`/`ty` offsets in points, `sx`/`sy` scale in percent. */
export interface RTile {
  tx: number;
  ty: number;
  sx: number;
  sy: number;
  /** `tl`, `t`, `tr`, `l`, `ctr`, `r`, `bl`, `b` or `br`. */
  align: string;
  flip: "none" | "x" | "y" | "xy";
}

/** Fill of a shape. A shape without a fill has no `fill`. */
export interface RFill {
  kind: "solid" | "gradient" | "picture" | "pattern";
  /** Hex without `#`: the colour; a gradient's first stop; a pattern's foreground; `FFFFFF` for a picture. */
  color: string;
  /** 0 to 1; omitted when opaque. */
  alpha?: number;
  /** A gradient's last stop; a pattern's background. */
  color2?: string;
  /** A linear gradient's direction in degrees: 0 left to right, 90 top to bottom. */
  angle?: number;
  /** A gradient's stops, all of them. */
  stops?: RStop[];
  /** The linear angle scales with the shape's aspect ratio. */
  scaled?: boolean;
  /** A path gradient (then `angle` is unused). */
  path?: "circle" | "rect" | "shape";
  /** The focus rectangle of a path gradient, percent. */
  fillTo?: RRect4;
  /** The gradient turns with the shape. */
  rotWithShape?: boolean;
  /** A picture fill: its media part name (`engine.mediaUrl(src)` gives the URL). */
  src?: string;
  /** Stretched picture: where the picture sits in the shape (`fillRect`), percent. */
  stretch?: RRect4;
  /** The part of the picture shown (`srcRect`), percent. */
  crop?: RRect4;
  /** Present when the picture is tiled; absent when stretched. */
  tile?: RTile;
  /** A pattern fill's DrawingML preset (`pct5`, `ltDnDiag`, `dkHorz`, ...). */
  pattern?: string;
}

/** A shadow or inner shadow. Distances points, `angle` degrees clockwise from the right, `sx`/`sy` percent. */
export interface RShadow {
  color: string;
  alpha?: number;
  blurPt: number;
  distPt: number;
  angle: number;
  sx?: number;
  sy?: number;
  /** Where a scaled shadow stays anchored: `b`, `ctr`, `tl`, ... */
  align?: string;
  /** The shadow turns with the shape; omitted when the file doesn't say (Word writes `false`). */
  rotWithShape?: boolean;
}

export interface RGlow {
  color: string;
  alpha?: number;
  radiusPt: number;
}

/** A reflection below the shape. Alphas 0 to 1, positions percent, directions degrees. */
export interface RReflection {
  blurPt: number;
  startAlpha: number;
  endAlpha: number;
  distPt: number;
  direction: number;
  fadeDirection: number;
  sx?: number;
  sy?: number;
  startPos?: number;
  endPos?: number;
  rotWithShape?: boolean;
}

export interface RBevel {
  /** `circle`, `relaxedInset`, `cross`, `coolSlant`, `angle`, `softRound`, `convex`, `slope`, `divot`, `riblet`, `hardEdge`, `artDeco`. */
  preset: string;
  wPt: number;
  hPt: number;
}

export interface RExtrusion {
  heightPt: number;
  color?: string;
  /** Unit step `[dx, dy]` per point of depth, from the 3-D camera. */
  dir?: [number, number];
}

/** The effects of a shape (its own, else those of its theme style). */
export interface REffects {
  shadow?: RShadow;
  innerShadow?: RShadow;
  glow?: RGlow;
  softEdgePt?: number;
  reflection?: RReflection;
  bevel?: RBevel;
  extrusion?: RExtrusion;
}

/** Outline of a shape. A shape without an outline has no `line`. */
export interface RLine {
  color: string;
  alpha?: number;
  widthPt: number;
  /** DrawingML's dash names: `solid`, `dot`, `dash`, `lgDash`, `dashDot`, `lgDashDot`, `lgDashDotDot`, `sysDash`, `sysDot`, ... */
  dash: string;
  /** Arrow at the start of an open line: `triangle`, `stealth`, `diamond`, `oval` or `arrow`. */
  headArrow?: string;
  tailArrow?: string;
  /** How the ends of the line are finished; omitted for flat. */
  cap?: "round" | "square";
  /** How corners are joined; omitted for mitred. */
  join?: "round" | "bevel";
  /** A compound line; omitted for a single line. */
  compound?: "dbl" | "thickThin" | "thinThick" | "tri";
  /** Arrowhead sizes at the start (`head`) and the end (`tail`); omitted for medium. */
  headW?: "sm" | "med" | "lg";
  headLen?: "sm" | "med" | "lg";
  tailW?: "sm" | "med" | "lg";
  tailLen?: "sm" | "med" | "lg";
  /** Miter limit in percent; omitted for Word's default (800). */
  miter?: number;
}

/** Text frame insets in points. */
export interface RInsets {
  l: number;
  t: number;
  r: number;
  b: number;
}

/** One path of a custom shape: SVG path data (`M`, `L`, `C`, `Q`, `A`, `Z`) in a `w` by `h` space. */
export interface RPath {
  w: number;
  h: number;
  d: string;
  /** The same path as structured commands in the same space: what "Edit points" works on. Arcs and quadratic curves are cubic curves here. */
  cmds?: RCmd[];
  fill: boolean;
  stroke: boolean;
}

/** A path command of a custom shape: `M` and `L` have one point, `C` two handles and an end point, `Z` none. */
export interface RCmd {
  t: "M" | "L" | "C" | "Z";
  pts: [number, number][];
}

/** A path command in an op (`insert_freeform`, `set_shape_path`): as `RCmd`, and `S` for a smooth curve through the points. */
export interface PathCmd {
  t: "M" | "L" | "C" | "S" | "Z";
  pts?: [number, number][];
}

/** What the ends of a connector are glued to: the drawing id (`RShape.id`) of a shape and the index of one of its connection sites. */
export interface RConnect {
  start?: { id: number; idx: number };
  end?: { id: number; idx: number };
}

/** A connector's preset and box between two points (`Engine.connectorRoute`): the unturned box, its turn and mirroring, and the adjust values that differ from the default. */
export interface ConnectorRoute {
  prst: string;
  x: number;
  y: number;
  w: number;
  h: number;
  rot: number;
  flipH: boolean;
  flipV: boolean;
  adj: Record<string, number>;
}

/** A connection site of a shape: points from its top-left corner (before any turn or mirroring), and the direction a connector leaves in (degrees: 0 right, 90 down, 180 left, 270 up). */
export interface RSite {
  x: number;
  y: number;
  ang: number;
}

/**
 * A drawing shape, text box, group or canvas, as the renderer draws it (the `shape` of an `RSeg` with `k: "shape"`).
 * Sizes and offsets are points, angles degrees. A shape is an inline object of length 1 in its paragraph; ops address
 * it by the position of that object (`{ p, o }` with `o` the segment's offset). The text inside is a story: its
 * paragraphs have ids, and every text op (`insert_text`, `split_paragraph`, `format_text`, ...) works on them.
 */
export interface RShape {
  /** The drawing's id (`wp:docPr`), 0 if there is none. */
  id: number;
  /** `shape`, `group`, `canvas`, `picture` for a picture inside a group or canvas, or `diagram` for SmartArt. */
  kind: "shape" | "group" | "canvas" | "picture" | "diagram";
  /** Where the markup came from. */
  source: "drawingml" | "vml";
  name?: string;
  alt?: string;
  /** A preset name (`rect`, `roundRect`, `ellipse`, `line`, `straightConnector1`, `triangle`, `rightArrow`, ...) or `custom` (see `custom`). */
  geometry: string;
  /** The preset's adjust values (`adj`: 16667) in the file's units. */
  adj?: Record<string, number>;
  custom?: { paths: RPath[] };
  w: number;
  h: number;
  /** Degrees clockwise; omitted when 0. */
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  fill?: RFill;
  line?: RLine;
  /** A text box (`txBox`), as opposed to a shape that happens to hold text. */
  textBox: boolean;
  /** The blocks of the text inside, with paragraph ids. Omitted when the shape has no text frame. */
  text?: RBlock[];
  insets: RInsets;
  vAnchor: "top" | "middle" | "bottom";
  /** Text wraps inside the shape; false for a single line. */
  wrapText: boolean;
  /** The shape grows to fit its text. */
  autoFit?: boolean;
  /** `shape` (the shape resizes to its text) or `text` (the text shrinks to fit); omitted for none. */
  autoFitMode?: "shape" | "text";
  /** With `autoFitMode: "text"`: the font scale and the line spacing cut Word saved, in percent. */
  fontScale?: number;
  lineSpacingReduction?: number;
  /** The text block is centred in the shape as a whole. */
  anchorCenter?: boolean;
  /** Text columns and the gap between them in points. */
  columns?: number;
  columnGapPt?: number;
  /** The text stays upright when the shape turns. */
  upright?: boolean;
  /** Vertical or rotated text (`vert270`, `eaVert`, `wordArtVert`, `mongolianVert`, ...); omitted for horizontal text. */
  vert?: string;
  /** Shadow, glow, reflection, soft edges, bevel and extrusion; omitted when there are none. */
  effects?: REffects;
  /** inline | square | tight | topBottom | behind | front */
  wrap: string;
  /** `left`, `center` or `right` for a floating shape placed by alignment. */
  align?: string;
  /** `top`, `center` or `bottom` for a floating shape placed vertically by alignment. */
  alignY?: string;
  /** Offsets in points from `relX` (column, page, margin, character, leftMargin, rightMargin, insideMargin, outsideMargin) and `relY` (paragraph, page, margin, line, topMargin, bottomMargin, insideMargin, outsideMargin). */
  x?: number;
  y?: number;
  relX?: string;
  relY?: string;
  /** Stacking order of a floating shape (`relativeHeight`): the higher, the nearer. Behind-text and in-front-of-text shapes are two stacks. */
  z?: number;
  /** For a picture inside a group or canvas: its part name. */
  src?: string;
  /** Members of a group or canvas, back to front. */
  children?: RShape[];
  /** For a member of a group or canvas: its position inside the parent's box, from its top-left corner, already scaled to the parent's size. */
  childX?: number;
  childY?: number;
  /** SmartArt (`kind: "diagram"`): where its shapes (`children`) came from. The graphic is drawn, not edited. */
  diagram?: RDiagram;
  /** A member of a diagram: its text, drawn read only. */
  dtext?: RDText;
  /** WordArt: the text bent to a preset. */
  warp?: RWarp;
  /** For a connector glued to shapes: what its ends are glued to. */
  connect?: RConnect;
}

/** An outline in an op: `"none"` or the parts to set (those left out stay as they are). */
export type LineSpec =
  | "none"
  | {
      color?: string;
      width_pt?: number;
      dash?: string;
      head_arrow?: string;
      tail_arrow?: string;
      /** 0 to 1. */
      alpha?: number;
      compound?: "single" | "dbl" | "thickThin" | "thinThick" | "tri";
      cap?: "flat" | "round" | "square";
      join?: "miter" | "round" | "bevel";
      /** Miter limit in percent. */
      miter?: number;
      head_w?: "sm" | "med" | "lg";
      head_len?: "sm" | "med" | "lg";
      tail_w?: "sm" | "med" | "lg";
      tail_len?: "sm" | "med" | "lg";
    };

/** A gradient stop in an op: `pos` 0 to 100. */
export interface StopIn {
  pos: number;
  color: string;
  alpha?: number;
}

/** A fill in an op: `"none"`, a hex colour (opaque, as before) or one of these. */
export type FillSpec =
  | "none"
  | string
  | { kind: "solid"; color: string; alpha?: number }
  | {
      kind: "gradient";
      stops: StopIn[];
      /** Linear direction in degrees: 0 left to right, 90 top to bottom. */
      angle?: number;
      /** A path gradient; then `angle` is unused. */
      path?: "circle" | "rect" | "shape";
      fill_to?: RRect4;
      scaled?: boolean;
      rot_with_shape?: boolean;
    }
  | {
      kind: "picture";
      /** The name `engine.addMedia` returned. */
      media: string;
      /** Present to tile the picture; leave out to stretch it. */
      tile?: { tx?: number; ty?: number; sx?: number; sy?: number; align?: string; flip?: "none" | "x" | "y" | "xy" };
      stretch?: RRect4;
      crop?: RRect4;
      alpha?: number;
    }
  | { kind: "pattern"; pattern: string; fg: string; bg: string; alpha?: number };

export interface ShadowIn {
  color?: string;
  alpha?: number;
  blur_pt?: number;
  dist_pt?: number;
  angle?: number;
  sx?: number;
  sy?: number;
  align?: string;
}

/** The parts of `effects` to change in an op. A part left out stays; `"none"` removes it. */
export interface EffectsIn {
  shadow?: ShadowIn | "none";
  inner_shadow?: ShadowIn | "none";
  glow?: { color?: string; alpha?: number; radius_pt?: number } | "none";
  soft_edge_pt?: number | "none" | null;
  reflection?:
    | {
        blur_pt?: number;
        start_alpha?: number;
        end_alpha?: number;
        dist_pt?: number;
        direction?: number;
        fade_direction?: number;
        sx?: number;
        sy?: number;
        start_pos?: number;
        end_pos?: number;
      }
    | "none";
  bevel?: { preset?: string; w_pt?: number; h_pt?: number } | "none";
  extrusion?: { height_pt?: number; color?: string } | "none";
}

/** The text frame's settings in an op; those left out stay. */
export interface BodyIn {
  /** Insets in points. */
  ins_l?: number;
  ins_t?: number;
  ins_r?: number;
  ins_b?: number;
  v_anchor?: "top" | "middle" | "bottom";
  anchor_center?: boolean;
  auto_fit?: "none" | "shape" | "text";
  wrap_text?: boolean;
  vert?: "horz" | "vert" | "vert270" | "eaVert" | "wordArtVert" | "mongolianVert";
  columns?: number;
  column_gap_pt?: number;
  upright?: boolean;
  /** With `auto_fit: "text"`: percent (100 = none). */
  font_scale?: number;
  line_spacing_reduction?: number;
}

/** Add a shape at a position (main text only). `fill` is hex or `"none"`. Floating (the default) anchors it to the paragraph with square wrapping, `x`/`y` points from the column and the paragraph. The caret lands after it. */
export interface InsertShapeOp {
  op: "insert_shape";
  at: MPos;
  /** A Word preset (`rect`, `roundRect`, `ellipse`, `triangle`, `rtTriangle`, `diamond`, `pentagon`, `hexagon`, `star5`, `rightArrow`, `leftArrow`, `upArrow`, `downArrow`, `chevron`, `cloud`, `heart`, `line`, `straightConnector1`, ...). */
  geometry: string;
  w: number;
  h: number;
  fill?: string;
  line?: LineSpec;
  text?: string;
  floating?: boolean;
  x?: number;
  y?: number;
}

/** Add a text box (white, thin black outline). One paragraph per line of `text`. The result's `caret` is in the first one and `created` lists their ids. */
export interface InsertTextBoxOp {
  op: "insert_text_box";
  at: MPos;
  w: number;
  h: number;
  text: string;
  floating?: boolean;
  x?: number;
  y?: number;
}

/** Move, resize, rotate, recolour or re-wrap the shape whose object is at `at`. Every field is optional. Delete a shape with `delete` over its one position. */
export interface SetShapeOp {
  op: "set_shape";
  at: MPos;
  w?: number;
  h?: number;
  /** Degrees. */
  rot?: number;
  /** Hex, `"none"`, or a fill object (solid with alpha, gradient, picture, pattern). */
  fill?: FillSpec;
  line?: LineSpec;
  /** Shadow, glow, soft edges, reflection, bevel, extrusion: `"none"` removes all, else the parts to change. */
  effects?: "none" | EffectsIn;
  /** The text frame: insets, anchor, autofit, wrap, vertical text, columns. */
  body?: BodyIn;
  geometry?: string;
  x?: number;
  y?: number;
  /** inline | square | tight | topBottom | behind | front */
  wrap?: string;
  v_anchor?: "top" | "middle" | "bottom";
  flip_h?: boolean;
  flip_v?: boolean;
  alt?: string;
  /** The drawing's name (`wp:docPr name`). */
  name?: string;
  /** Change a member of a group or canvas instead: its index, then the index of its member if that is a group too. `x`, `y`, `w` and `h` are then points inside the box of the group that holds it. */
  member?: number[];
}

/** Move a floating shape, text box, chart or picture in the stacking order of its kind. */
export interface SetObjectOrderOp {
  op: "set_object_order";
  at: MPos;
  order: "front" | "back" | "forward" | "backward";
}

/** Put a floating object behind the text or in front of it, where it is. */
export interface SetBehindTextOp {
  op: "set_behind_text";
  at: MPos;
  behind: boolean;
}

/** Group shapes into one (it goes where the first is). `offsets` are the top-left corners in points, in the frame the first shape's position counts from. */
export interface GroupShapesOp {
  op: "group_shapes";
  at: MPos[];
  offsets?: [number, number][];
}

/** Take a group apart. `offset` is the group's top-left corner in points when it is placed by alignment. */
export interface UngroupShapeOp {
  op: "ungroup_shape";
  at: MPos;
  offset?: [number, number];
}

/** Insert a freehand drawing: strokes of `[x, y]` points (points from the column and the paragraph), one pen. */
export interface InsertInkOp {
  op: "insert_ink";
  at: MPos;
  strokes: [number, number][][];
  /** Hex. */
  color?: string;
  width_pt?: number;
  /** How much of the colour shows, 0 to 1 (a highlighter is about 0.5). */
  alpha?: number;
  /** How the pen ends the line: `round` (the default), `square` or `flat`. */
  cap?: "round" | "square" | "flat";
}

/** Insert a curve, freeform or scribble: give `points` (`smooth` draws a curve through them) or `cmds`. The first point sits at `x`, `y` (points from the column and the paragraph). */
export interface InsertFreeformOp {
  op: "insert_freeform";
  at: MPos;
  points?: [number, number][];
  cmds?: PathCmd[];
  smooth?: boolean;
  closed?: boolean;
  fill?: string;
  line?: LineSpec;
  floating?: boolean;
  x?: number;
  y?: number;
  /** The name Word shows before the number (`Freeform: Shape`, `Freeform: Scribble`). */
  label?: string;
}

/** Give a shape a new outline ("Edit points"). Each path's commands are in its own `w` by `h` space (points of the shape's box when 0). */
export interface SetShapePathOp {
  op: "set_shape_path";
  at: MPos;
  paths: { w?: number; h?: number; cmds: PathCmd[]; fill?: boolean; stroke?: boolean }[];
  /** Where the shape's new top-left corner lands, for a floating shape. */
  x?: number;
  y?: number;
}

/** What an end of a connector is glued to in an op: `"free"`, or a shape (the position of its object) and the index of one of its sites. */
export type EndSpec = "free" | { shape: MPos; idx: number };

/** Insert a connector between two points (or two shapes' sites). The engine keeps glued ends on their sites when the shapes move. */
export interface InsertConnectorOp {
  op: "insert_connector";
  at: MPos;
  kind?: "straight" | "elbow" | "curved";
  start?: EndSpec;
  end?: EndSpec;
  /** Where the ends are, in points from the column and the paragraph. */
  from?: [number, number];
  to?: [number, number];
  line?: LineSpec;
}

/** Change a connector: its kind, where its ends are, or what they are glued to (`"free"` lets go). */
export interface SetConnectorOp {
  op: "set_connector";
  at: MPos;
  kind?: "straight" | "elbow" | "curved";
  start?: EndSpec;
  end?: EndSpec;
  from?: [number, number];
  to?: [number, number];
}
