// East Asian typography as the engine reports and takes it (mirrors east_asian.rs and ruby.rs).
import type { RunFmt } from "./types.ts";

/** `w:eastAsianLayout`: two lines in one, and text upright in a vertical line. */
export interface EaLayout {
  /** Runs with the same id are one combined box. */
  id?: number;
  /** Two lines in one. */
  combine?: boolean;
  /** `round`, `square`, `angle` or `curly`; absent is none. */
  combineBrackets?: string;
  /** Horizontal in vertical. */
  vert?: boolean;
  /** Horizontal in vertical, fitted into the line's height. */
  vertCompress?: boolean;
}

/** Run properties of East Asian text (fields of `RunFmt`). */
export interface EaRunFmt {
  /** The East Asian font of the run (`w:eastAsia`), whatever the text is. */
  fontEa?: string;
  eaLayout?: EaLayout;
  /** Off: the text keeps its own spacing and doesn't sit on the document grid. */
  snapToGrid?: boolean;
  /** The East Asian language (`ja-JP`); on East Asian text only. */
  langEa?: string;
  /** Set on East Asian text: it is drawn in `fontEa` where the run has one. */
  eaText?: boolean;
}

/** Paragraph properties of the Asian typography tab (fields of `ParaFmt`). Word's defaults apply to what is absent. */
export interface EaParaFmt {
  /** Follow the rules for the first and last character of a line. */
  kinsoku?: boolean;
  /** Break Latin words only at their ends. */
  wordWrap?: boolean;
  /** Punctuation may hang past the margin. */
  overflowPunct?: boolean;
  /** Compress punctuation at the start of a line. */
  topLinePunct?: boolean;
  /** Space between Asian and Latin text, and between Asian text and numbers. */
  autoSpaceDe?: boolean;
  autoSpaceDn?: boolean;
  snapToGrid?: boolean;
  /** Vertical text in one paragraph: `tbRl`, `btLr` and the like. */
  textDirection?: string;
  /** `top`, `center`, `baseline` or `bottom`. */
  textAlignment?: string;
}

export type EaRunPatch = Partial<{
  /** The East Asian font; an empty string removes it. */
  fontEa: string;
  eaLayout: { combine?: boolean; combineBrackets?: string; vert?: boolean; vertCompress?: boolean };
  snapToGrid: boolean;
}>;

export type EaParaPatch = Partial<EaParaFmt>;

/** A section's document grid. */
export interface DocGrid {
  type: "default" | "lines" | "linesAndChars" | "snapToChars";
  /** Distance between lines (twips). */
  linePitch?: number;
  /** Added to the base font size to get a character's width, in 4096ths of a point. */
  charSpace?: number;
}

/** A phonetic guide as it is drawn and edited. */
export interface RRuby {
  base: string;
  text: string;
  align: "center" | "distributeLetter" | "distributeSpace" | "left" | "right" | "rightVertical";
  /** Points. */
  size?: number;
  baseSize?: number;
  /** How far above the baseline the guide sits, in points. */
  raise?: number;
  lang?: string;
  font?: string;
  /** How the guide is formatted (only in what the renderer draws). */
  rtF?: RunFmt;
}

/** What the Phonetic guide box sets. */
export interface RubyProps {
  text: string;
  align?: RRuby["align"];
  size?: number;
  baseSize?: number;
  raise?: number;
  lang?: string;
  font?: string;
}

export interface LineBreakChars {
  lang: "ja-JP" | "ko-KR" | "zh-CN" | "zh-TW";
  /** Characters that may not start a line. */
  before: string;
  /** Characters that may not end a line. */
  after: string;
}

/** The document's East Asian line-breaking settings. */
export interface AsianSettings {
  /** The strict rules (level 2). */
  strict?: boolean;
  custom?: LineBreakChars[];
}
