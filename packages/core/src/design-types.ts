// Styles, themes, style sets and Reveal formatting: the shapes of the `set_style_definition`, `create_style`,
// `update_style`, `delete_style`, `set_theme`, `set_style_set` and `set_paragraph_spacing` ops, and of what
// the engine reports about styles and the theme. Field names are the ones on the wire.
import type { ParaFmt, ParaPatch, RunFmt, RunPatch } from "./types.ts";

/** A style's definition, as the Modify style dialog edits it. A field left out stays as it is. */
export interface StyleDefinition {
  /** The name people see. Only styles a person made can be renamed. */
  name?: string;
  /** For a new style: `paragraph`, `character`, or `linked` (a paragraph style with a character style of the same look). */
  kind?: "paragraph" | "character" | "linked";
  /** The style it builds on (id or name); "" builds on none. */
  basedOn?: string;
  /** The style of the paragraph after this one; "" (or the style itself) writes none. */
  next?: string;
  /** Show in the style gallery (`w:qFormat`). */
  quick?: boolean;
  /** Update the style when text with it is formatted by hand (`w:autoRedefine`). */
  autoUpdate?: boolean;
  hidden?: boolean;
  /** Where the style sorts in the recommended order; lower comes first. */
  priority?: number;
  ppr?: ParaPatch;
  rpr?: RunPatch;
  /** BCP 47, `en-US`; "" removes it. */
  lang?: string;
}

/** One tile of the style gallery or row of the Styles panel: a style and how it looks. */
export interface StyleLook {
  id: string;
  name: string;
  kind: "paragraph" | "character" | "table" | "numbering" | string;
  quick: boolean;
  hidden: boolean;
  priority: number;
  /** Made by a person (not one of Word's built-in styles). */
  custom: boolean;
  /** Word's built-in style that the file does not define yet; choosing it adds Word's definition. */
  latent: boolean;
  /** The style it is linked to: a paragraph style and its character style apply as one in Word. */
  link?: string;
  /** Paragraphs (or, for a character style, runs) that use it in the main text. */
  uses: number;
  /** Fonts and colours are resolved: names and hex. */
  run: RunFmt;
  para: ParaFmt;
}

/** Everything the Modify style dialog shows about one style. */
export interface StyleDetails {
  id: string;
  name: string;
  kind: string;
  basedOn?: string;
  next?: string;
  link?: string;
  quick: boolean;
  autoUpdate: boolean;
  hidden: boolean;
  priority: number;
  custom: boolean;
  latent: boolean;
  /** The formatting the style sets itself. */
  ownPara: ParaFmt;
  ownRun: RunFmt;
  /** What its text looks like: document defaults, the styles it builds on and the style itself. */
  para: ParaFmt;
  run: RunFmt;
  lang?: string;
  uses: number;
}

/** A place a style is used: a paragraph and the part of its text, as UTF-16 offsets. */
export interface StyleUse {
  p: number;
  from: number;
  to: number;
}

/** The twelve slots of a colour scheme. */
export const THEME_COLOR_SLOTS = ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"] as const;
export type ThemeColorSlot = (typeof THEME_COLOR_SLOTS)[number];

/** A colour scheme: a name and hex colours (no `#`) by slot. A slot left out keeps the colour the document has. */
export interface ColorScheme {
  name: string;
  colors: Partial<Record<ThemeColorSlot, string>>;
}

/** A font scheme: the heading font (major) and the body font (minor). */
export interface FontScheme {
  name: string;
  major: string;
  minor: string;
}

/** A whole theme as the Themes gallery offers it. */
export interface ThemeDef {
  id?: string;
  name: string;
  colors: ColorScheme;
  fonts: FontScheme;
}

/** What a document's theme is now. */
export interface ThemeInfo {
  name: string;
  colors: ColorScheme;
  fonts: FontScheme;
}

/** Word's built-in themes, colour schemes and font schemes (`Engine.themeLibrary()`). */
export interface ThemeLibrary {
  themes: ThemeDef[];
  colorSchemes: ColorScheme[];
  fontSchemes: FontScheme[];
}

/** Spacing between paragraphs and lines: `before` and `after` in twips, `line` in 240ths of a line. */
export interface Spacing {
  before?: number;
  after?: number;
  line?: number;
}

/** A paragraph spacing preset (`Engine.spacingPresets()`). */
export interface SpacingPreset extends Required<Spacing> {
  id: string;
  name: string;
}

/** A style set an extension brings: `w:style` elements as XML and the default spacing. */
export interface StyleSetDef {
  id?: string;
  name: string;
  /** One or more `w:style` elements. A style the document has keeps its name; its formatting is replaced. A style it lacks is added. */
  styles: string;
  spacing?: Spacing;
}

/** A style set drawn in a document's theme: the looks of its title, headings and body text. */
export interface StyleSetPreview {
  id: string;
  name: string;
  description: string;
  samples: StyleLook[];
}

/** Where a setting comes from. */
export type RevealSource = "default" | "paragraphStyle" | "characterStyle" | "direct";

export interface RevealRow {
  /** `font`, `size`, `bold`, `alignment`, `spaceAfter`... */
  key: string;
  group: "font" | "paragraph";
  value: string | number | boolean | { line: number; rule: string };
  source: RevealSource;
  /** The name of the style the setting comes from, for the two style sources. */
  style?: string;
  /** That style is the person's own: its name is shown as written. */
  custom?: boolean;
}

export interface RevealStyle {
  id: string;
  name: string;
  /** The style is the person's own: its name is shown as written. */
  custom?: boolean;
}

/** The layers of formatting at a position and every setting the text has with the layer it comes from (`Engine.revealFormatting`). */
export interface Reveal {
  paragraphStyle: RevealStyle;
  /** The styles the paragraph style builds on. */
  basedOn: RevealStyle[];
  characterStyle?: RevealStyle;
  directParagraph: boolean;
  directRun: boolean;
  rows: RevealRow[];
}

/** The design as it is now: what Set as default keeps for new documents. */
export interface DesignSnapshot {
  theme: ThemeDef;
  styleSet: StyleSetDef;
}
