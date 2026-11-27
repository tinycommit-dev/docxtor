// Settings, themes, icons, charts, content controls, table designs and styles
import type { ControlProps, ControlRef, MPos, Op, OpResult, Spacing, ThemeColorSlot } from "@docxtor/core";

/**
 * One line in Preferences. The extension keeps the value wherever it likes (`get` and `set`), so the setting can live in
 * the browser, in the host app or on a server. Lines with the same `group` sit under one heading.
 */
export interface SettingDefinition {
  id: string;
  label: string;
  /** A sentence under the control. */
  help?: string;
  /** The heading it sits under (default "General"). */
  group?: string;
  kind: "toggle" | "choice" | "number" | "text" | "font";
  /** For a choice. */
  choices?: { value: string; label: string }[];
  /** For a number. */
  min?: number;
  max?: number;
  step?: number;
  /** Shown after a number: "seconds". */
  unit?: string;
  /** The value now: a boolean, a string (a choice's value, a text or a font family) or a number. */
  get(): unknown;
  /** The person chose a value. */
  set(value: unknown): void | Promise<void>;
}

export interface ThemeContribution {
  id: string;
  label: string;
  /** The built-in theme it starts from. */
  base: "light" | "dark";
  /** Token overrides, e.g. `{ "--signal": "#2563eb" }`. See DESIGN.md for every token. */
  tokens: Record<`--${string}`, string>;
}

/** A font family for documents: shown in the font menu and loaded when text uses it. */
/**
 * An icon that commands, menus and the toolbar refer to by `id`. Registered on its own (`ctx.icons`), it
 * replaces that icon while your extension is on, whichever icon pack is chosen (the most recently added
 * wins): a way to restyle one icon. A set of icons people can switch to is an `IconPack`.
 */
export interface IconContribution {
  id: string;
  /** An SVG with a 24×24 viewBox that draws in `currentColor`. */
  svg: string;
}

/**
 * A named set of icons, offered under View, Icons. Docxtor's own icons are the pack `docxtor`. A pack needs
 * only the icons it redraws: the ones it leaves out come from Docxtor's pack. Registering a pack under an id
 * that exists adds to that pack, and the most recent icon wins.
 */
export interface IconPack {
  id: string;
  /** What the picker shows (English; it is translated like other text). */
  label: string;
  icons: IconContribution[];
}

/** A colour palette for the Change colors gallery of the Edit chart dialog. */
export interface ChartPaletteContribution {
  id: string;
  /** Shown as the tile's name. */
  label: string;
  /**
   * The colours, in series order; the list repeats when a chart has more series (and a pie, or a chart with varied
   * colours, takes them by point). Each is `RRGGBB`, or a theme colour written `scheme:accent2`, optionally with
   * luminance changes (`scheme:accent2:lumMod=60000:lumOff=40000`), so the chart follows the document's theme.
   */
  colors: string[];
}
/** A content control as an extension sees it (the engine's `RControl`, with what it shows). */
export interface ContentControlInfo {
  /** `text`, `richText`, `picture`, `date`, `dropDown`, `comboBox`, `checkbox`, `repeating`, `repeatingItem`, `group` or `other`. */
  kind: string;
  /** The control's title and tag (`w:alias`, `w:tag`). */
  alias?: string;
  tag?: string;
  /** What it shows now, and whether that is its placeholder. */
  text: string;
  placeholder: boolean;
  /** The entries of a list, the date picker's format, the check box's state. */
  items?: { display: string; value: string }[];
  dateFormat?: string;
  checked?: boolean;
  /** The control is locked against editing its content. */
  locked: boolean;
  /** The date picker's chosen day (`2026-10-14T00:00:00Z`) and its language. */
  fullDate?: string;
  lid?: string;
  /** Where the link around the control's text goes (a person chip's `mailto:`, a file chip's address). */
  href?: string;
}

/** What an extension's `edit` can do with the control it was opened on. */
export interface ContentControlEditApi {
  readonly control: ContentControlInfo;
  /** The element to anchor a popover to. */
  readonly anchor: HTMLElement;
  /** Give the control a value: text, a check box state, an entry of a list (by position) or a date (`2026-10-14`). */
  set(value: { text?: string; checked?: boolean; index?: number; date?: string }): boolean;
  /** Apply ops (the same language as `DocumentApi.exec`). */
  exec(ops: Op | Op[]): OpResult[] | null;
  /** Show a popover under the anchor with `content`; call the result to close it. */
  popover(content: HTMLElement): () => void;
  /** Where the control's text is, for ops that act on it. */
  readonly range: { from: MPos; to: MPos };
  /** The control as an op names it (`{ op: "control", action: "props", target: api.ref, ... }`). */
  readonly ref: ControlRef;
  /** Take the control away: its text stays, or goes with `deleteContent`. */
  remove(deleteContent?: boolean): boolean;
  /** Change its properties (title, tag, date format, list entries...). */
  props(props: ControlProps): boolean;
}


/**
 * A kind of content control. `kind` is the engine's kind (`checkbox`, `date`...) to take over how that kind is drawn
 * and edited, or the control's `w:tag` for a kind of your own: Docxtor writes it as a rich text or plain text control
 * with that tag, so Word keeps the content, and your extension gives it a look and an editor.
 */
export interface ContentControlContribution {
  kind: string;
  /** Shown in the control's tab when it has no title. */
  label?: string;
  /** Called after the control is drawn: decorate its elements (the text segments it covers), for example add a class or a badge. Return a function to undo it. */
  render?(elements: HTMLElement[], control: ContentControlInfo): void | (() => void);
  /** Called when the person clicks the control or presses its button: open an editor and call `api.set` or `api.exec`. */
  edit?(api: ContentControlEditApi): void;
}

/** A table design for the Table design gallery, beside Word's built-in ones. */
export interface TableDesignContribution {
  /** The table style's id in the document (`AcmeTable`); the document keeps it, so keep it stable. */
  id: string;
  /** Shown as the tile's name. */
  label: string;
  /**
   * The design as one `w:style` element of type `table`, written as Word writes a table style: conditional formatting in
   * `w:tblStylePr` for the header row, banded rows and the rest. It may be based on `TableNormal`. The first time a table
   * gets the design it is added to the document's styles; the document then has its own copy.
   */
  xml: string;
  /** The gallery section it is shown in; designs without one come first. */
  group?: string;
}

/**
 * A document theme. With colours and fonts it is a whole theme (Design, Themes); with only colours, a colour scheme
 * (Design, Colors); with only fonts, a font scheme (Design, Fonts). Applying one changes the document's theme part, so
 * text, headings, tables, shapes and charts that use theme colours or fonts follow it.
 */
export interface DocumentThemeContribution {
  /** Stable; the engine knows Docxtor's own by their id (`facet`, `blue-warm`). */
  id: string;
  /** Shown as the tile's name (English; translated like other text). */
  label: string;
  /** Hex colours (no `#`) by slot: `dk1`, `lt1`, `dk2`, `lt2`, `accent1` to `accent6`, `hlink`, `folHlink`. A slot left out keeps the document's colour. */
  colors?: Partial<Record<ThemeColorSlot, string>>;
  /** The heading font and the body font. */
  fonts?: { major: string; minor: string };
  /** The gallery section it is shown in; ones without come first. */
  group?: string;
}

/**
 * A style set: the definitions of Normal, Title, Subtitle, Heading 1 to 9 and Quote (or any styles) written together, with
 * the paragraph spacing of the document defaults. Docxtor's own are registered without `styles`; the engine has them by `id`.
 */
export interface StyleSetContribution {
  id: string;
  label: string;
  description?: string;
  /** One or more `w:style` elements as XML. A style the document has keeps its name and what it is based on; its formatting is replaced. One it lacks is added. */
  styles?: string;
  /** The paragraph spacing of the document defaults. */
  spacing?: Spacing;
}

/**
 * A citation style. With `csl` it is a CSL 1.0.2 style that Docxtor's own processor formats; without it, it is one of
 * the five styles the engine formats itself (`apa`, `mla`, `chicago`, `ieee`, `harvard`), and `id` must be that one.
 * The document remembers the `id`, so keep it stable.
 */
export interface CitationStyleContribution {
  id: string;
  /** Shown in the style picker. */
  label: string;
  /**
   * The style's CSL XML, or a function that returns it. A function is called when a person picks the style or
   * opens a document that uses it, so a large style can be fetched or imported on demand.
   */
  csl?: string | (() => string | Promise<string>);
  /** Groups the style in the picker; styles without one come first. */
  group?: string;
}

