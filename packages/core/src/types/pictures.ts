// Picture formatting and effects

export interface RImage {
  src: string;
  w: number;
  h: number;
  floating?: boolean;
  alt?: string;
  /** The drawing's name (`wp:docPr name`), as the Selection pane lists it. */
  name?: string;
  /** inline | square | tight | topBottom | behind | front */
  wrap: string;
  align?: string;
  alignY?: string;
  x?: number;
  y?: number;
  relX?: string;
  relY?: string;
  /** Stacking order of a floating object (`relativeHeight`). */
  z?: number;
  /** How the picture is formatted; absent for a plain one. */
  pic?: PicFmt;
  /** Text distances and anchor behaviour of a floating object. */
  place?: PlacePlus;
}

/** A picture's outline. */
export interface PicLine {
  /** Hex, no `#`. */
  color: string;
  widthPt: number;
  /** `solid`, `dot`, `dash`, `lgDash`, `dashDot`, `lgDashDot`, `lgDashDotDot`, `sysDash`, `sysDot`, ... */
  dash: string;
}

export interface PicRecolor {
  /** `grayscale`, `blackWhite` or `duotone`. */
  kind: string;
  threshold?: number;
  color1?: string;
  color2?: string;
}

export interface PicShadow {
  blurPt: number;
  distPt: number;
  /** Degrees clockwise from the right. */
  angle: number;
  color: string;
  alpha: number;
}

export interface PicGlow {
  radiusPt: number;
  color: string;
  alpha: number;
}

export interface PicReflection {
  size: number;
  alpha: number;
  distPt: number;
  blurPt: number;
}

/** A picture's formatting, as the engine reads it from the file. Fractions are of the whole picture. */
export interface PicFmt {
  /** Left, top, right, bottom; negative values pad. */
  crop?: [number, number, number, number];
  rot?: number;
  flipH?: boolean;
  flipV?: boolean;
  /** The picture's shape (`ellipse`, `roundRect`, ...). */
  geom?: string;
  adj?: number;
  line?: PicLine;
  opacity?: number;
  bright?: number;
  contrast?: number;
  saturation?: number;
  recolor?: PicRecolor;
  clrChange?: { from: string; to?: string };
  shadow?: PicShadow;
  glow?: PicGlow;
  softEdge?: number;
  reflection?: PicReflection;
  art?: [string, number];
  /** The media part of the picture's SVG version. */
  svg?: string;
  /** The picture is only linked: where it points. */
  link?: string;
  /** Room Word keeps around the picture: left, top, right, bottom in points. */
  extent?: [number, number, number, number];
}

/** Placement details of a floating object. */
export interface PlacePlus {
  /** Top, bottom, left, right distance of the text, in points. */
  dist: [number, number, number, number];
  wrapText?: "bothSides" | "left" | "right" | "largest";
  through?: boolean;
  locked?: boolean;
  overlap: boolean;
  alignH?: string;
}

