// Run, paragraph and list formatting
import type { FrameFmt } from "./page.ts";
import type { EaLayout, EaParaFmt } from "../east-asian-types.ts";

export interface RunFmt {
  style?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: string;
  strike?: boolean;
  dstrike?: boolean;
  caps?: boolean;
  smallCaps?: boolean;
  vanish?: boolean;
  /** Spelling and grammar are not checked in this text (`w:noProof`, "Do not check spelling or grammar"). */
  noProof?: boolean;
  font?: string;
  size?: number;
  color?: string;
  highlight?: string;
  shading?: string;
  vertAlign?: string;
  spacing?: number;
  rtl?: boolean;
  /** The theme colour behind `color` (already resolved to hex in `color`). */
  colorTheme?: ThemeRef;
  /** The underline's own colour (hex, or `auto`: the text colour). */
  underlineColor?: string;
  underlineTheme?: ThemeRef;
  /** Raised (positive) or lowered (negative) text, in half-points. */
  position?: number;
  /** Character width in percent (`w:w`); 100 is normal. */
  scale?: number;
  /** Kern text from this size up, in half-points; 0 is off. */
  kern?: number;
  outline?: boolean;
  shadow?: boolean;
  emboss?: boolean;
  imprint?: boolean;
  /** A box around the text (`w:bdr`). */
  border?: Border;
  /** The text shading with its pattern; `shading` is the flat colour. */
  shd?: Shd;
  /** Emphasis mark: `dot`, `comma`, `circle`, `underDot`. */
  em?: string;
  /** Fit the text into this width in twips (`w:fitText`), and the id that groups runs. */
  fitText?: number;
  fitTextId?: number;
  /** Word 2010 text effects and OpenType settings. */
  fx?: TextFx;
  /** East Asian text: its font, two lines in one and upright text in vertical lines, grid snapping, language (see `EaRunFmt`). */
  fontEa?: string;
  eaLayout?: EaLayout;
  snapToGrid?: boolean;
  langEa?: string;
  /** Set on East Asian text (ideographs, kana, Hangul and their punctuation): drawn in `fontEa`. */
  eaText?: boolean;
}

/** A theme colour reference: `name` is `accent1`, `text2`, ...; tint and shade are hex 00 to FF. */
export interface ThemeRef {
  name: string;
  tint?: string;
  shade?: string;
}

/** A colour in a text effect, resolved: hex and alpha (0 to 1). */
export interface FxColor {
  hex?: string;
  alpha?: number;
}

export interface TextFx {
  glow?: { radius: number; color?: FxColor };
  shadow?: { blur: number; dist: number; dir: number; color?: FxColor };
  reflection?: { blur: number; dist: number; startAlpha: number; endAlpha: number };
  outline?: { width: number; color?: FxColor };
  fill?: { solid?: FxColor; stops?: { pos: number; color: FxColor }[]; angle?: number };
  ligatures?: string;
  numForm?: string;
  numSpacing?: string;
  styleSets?: number[];
  cntxtAlts?: boolean;
}

export interface Border {
  val: string;
  sz: number;
  color: string;
  space: number;
}

export interface ParaBorders {
  top?: Border;
  bottom?: Border;
  left?: Border;
  right?: Border;
  /** Drawn between paragraphs that share the other borders. */
  between?: Border;
  /** The bar border: a line along the outside edge of the paragraph. */
  bar?: Border;
}

/** Shading: a pattern (`clear`, `solid`, `pct25`, `diagStripe`, ...) of `color` over `fill`, both hex. */
export interface Shd {
  val: string;
  color?: string;
  fill?: string;
}

export interface ParaFmt {
  style?: string;
  align?: string;
  spaceBefore?: number;
  spaceAfter?: number;
  beforeAuto?: boolean;
  afterAuto?: boolean;
  line?: number;
  lineRule?: string;
  indLeft?: number;
  indRight?: number;
  indFirstLine?: number;
  indHanging?: number;
  keepNext?: boolean;
  keepLines?: boolean;
  pageBreakBefore?: boolean;
  contextualSpacing?: boolean;
  numId?: number;
  ilvl?: number;
  outlineLvl?: number;
  shading?: string;
  borders?: ParaBorders;
  bidi?: boolean;
  /** Widow/orphan control as the XML says; Word treats an unspecified value as on. */
  widowControl?: boolean;
  suppressLineNumbers?: boolean;
  suppressAutoHyphens?: boolean;
  /** Tab stops in position order (twips from the left margin). */
  tabs?: TabStop[];
  /** Indents in hundredths of a character; they win over the twips. */
  indLeftChars?: number;
  indRightChars?: number;
  indFirstLineChars?: number;
  indHangingChars?: number;
  /** Spacing in hundredths of a line; it wins over the twips. */
  spaceBeforeLines?: number;
  spaceAfterLines?: number;
  /** Left and right indents are inside and outside indents: they swap on even pages. */
  mirrorIndents?: boolean;
  /** The paragraph is a positioned frame, or a drop cap's letter (`w:framePr`). */
  frame?: FrameFmt;
  /** The shading with its pattern (theme colours resolved). `shading` is the flat colour. */
  shd?: Shd;
  /** Asian typography: line-breaking rules, auto spacing, grid snapping, vertical text (see `EaParaFmt`). */
  kinsoku?: EaParaFmt["kinsoku"];
  wordWrap?: EaParaFmt["wordWrap"];
  overflowPunct?: EaParaFmt["overflowPunct"];
  topLinePunct?: EaParaFmt["topLinePunct"];
  autoSpaceDe?: EaParaFmt["autoSpaceDe"];
  autoSpaceDn?: EaParaFmt["autoSpaceDn"];
  snapToGrid?: EaParaFmt["snapToGrid"];
  textDirection?: EaParaFmt["textDirection"];
  textAlignment?: EaParaFmt["textAlignment"];
}

export interface TabStop {
  /** `left`, `center`, `right`, `decimal`, `bar` (or `clear`, which removes an inherited stop). */
  val: string;
  pos: number;
  /** `dot`, `hyphen`, `underscore`, `heavy`, `middleDot`. */
  leader?: string;
}

export interface ListLabel {
  text: string;
  level: number;
  numId: number;
  /** The count the label shows (1 for "1.", 3 for "c)"). */
  value: number;
  bullet: boolean;
  /** A checklist item: a bullet whose glyph is a checkbox (☐, ☑ or ☒). Omitted when false. */
  checklist?: boolean;
  /** A checked checklist item. Omitted when false. */
  checked?: boolean;
  font?: string;
  color?: string;
  size?: number;
  bold?: boolean;
  italic?: boolean;
  /** Underline type (`single`, `double`, ...). */
  underline?: string;
  strike?: boolean;
  highlight?: string;
  caps?: boolean;
  smallCaps?: boolean;
  /** `center` or `right` when the label isn't left-aligned in the space before the text. */
  align?: "center" | "right";
  /** A picture bullet: the label is this picture (`src` is a media part, see `Engine.mediaUrl`), `w` by `h` points. */
  pic?: { src: string; w: number; h: number };
  suffix: string;
}

