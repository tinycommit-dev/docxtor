// Lists a person defines and paragraph sorting: the shapes of the `define_list`, `apply_list`,
// `set_numbering_value` and `sort_paragraphs` ops. Field names are the ones on the wire.

/**
 * One level of a list being defined (`define_list`). Indents and the tab are in twips (1/20 pt), `size` in points.
 * The nine levels of a list are the ones given, then copies of the last given one.
 */
export interface LevelDef {
  /** `w:numFmt`: `bullet`, `decimal`, `lowerRoman`, `upperLetter`, `cardinalText`, `japaneseCounting`, ... */
  format: string;
  /** The bullet character, or the label with counts in it (`%1.`, `%1.%2.`, `(%3)`). */
  text: string;
  start?: number;
  align?: "left" | "center" | "right";
  /** What follows the label: `tab` (default), `space` or `nothing`. */
  suffix?: "tab" | "space" | "nothing";
  left?: number;
  hanging?: number;
  /** A tab stop for the text after the label. */
  tab?: number;
  font?: string;
  size?: number;
  bold?: boolean;
  italic?: boolean;
  /** Hex colour. */
  color?: string;
  /** 0 never restarts the level; n restarts it after level n or higher. Omitted: after any higher level. */
  restart?: number;
  /** Legal numbering: counts of earlier levels show as 1, 2, 3 whatever their format. */
  legal?: boolean;
  /** A paragraph style (a heading) linked to the level. */
  style?: string;
  /** A picture bullet: a media part from `Engine.addMedia`. */
  picture?: string;
  /** The picture's size in points. */
  picture_size?: [number, number];
}

export interface ListDef {
  name?: string;
  levels: LevelDef[];
}

/** A list the document uses that wasn't made from one of Docxtor's presets (`Engine.listDefinitions`). */
export interface ListDefinition {
  /** A num that uses the definition (what `apply_list` takes), and every num that does. */
  numId: number;
  nums: number[];
  name?: string;
  bullet: boolean;
  /** The first three levels as they show. */
  levels: { text: string; font?: string; pic?: { src: string; w: number; h: number } }[];
}

export interface SortKey {
  /** `text` (default), `number` or `date`. */
  type?: "text" | "number" | "date";
  descending?: boolean;
  /** 0 (default) sorts by the whole paragraph; n by its nth field. */
  field?: number;
}

/** The options of `sort_paragraphs` (besides the `ids`). */
export interface SortOptions {
  /** Up to three keys; the next breaks ties in the one before. */
  keys?: SortKey[];
  case_sensitive?: boolean;
  /** The first paragraph is a header and stays. */
  header?: boolean;
  /** What separates fields: `tabs` (default), `commas` or `other` (with `separator_char`). */
  separator?: "tabs" | "commas" | "other";
  separator_char?: string;
}
