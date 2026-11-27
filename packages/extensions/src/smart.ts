// Smart chips, the @ menu, building blocks and link previews: the contribution points behind them, written as plain
// data and functions so they work without a DOM. Docxtor's own providers (people named in the document, the files list,
// the drop-down presets) are registered through these like anyone else's.

// ---------------------------------------------------------------------------
// The @ menu
// ---------------------------------------------------------------------------

/** One line of the @ menu. */
export interface AtMenuItem {
  /** Unique within its provider. */
  id: string;
  label: string;
  /** Shown after the label, quieter: an email address, a date. */
  detail?: string;
  /** An icon id (see `icons`) or SVG markup. */
  icon?: string;
  /** The heading it is listed under. Default: the provider's label. */
  section?: string;
  /** Other words that find it. */
  keywords?: string[];
  /** Do it. The `@` and what was typed after it are already gone, so the caret is where the item goes. */
  run(): void | Promise<void>;
}

/** A source of lines for the @ menu. It is asked again each time the person types another letter. */
export interface AtMenuProvider {
  id: string;
  /** The heading of its lines (People, Dates, Files...). */
  label: string;
  /** Sections are listed from the lowest order up. Docxtor's own run from 10 to 90. */
  order?: number;
  /** The lines that fit `query` (what was typed after the @, possibly empty). Return few: the menu shows the first ones. */
  items(query: string): AtMenuItem[] | Promise<AtMenuItem[]>;
}

/** A heading of the @ menu with its lines. */
export interface AtMenuSection {
  id: string;
  title: string;
  items: AtMenuItem[];
}

/** Whether `query` fits a label: every word typed starts a word of the label or of its keywords. Letters match without their case or accents. */
export function atMenuMatches(query: string, label: string, keywords: readonly string[] = []): boolean {
  const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const parts = (s: string) => fold(s).split(/[\s,.;:()/@<>-]+/).filter(Boolean);
  const words = parts(query);
  if (!words.length) return true;
  const hay = parts([label, ...keywords].join(" "));
  return words.every((w) => hay.some((h) => h.startsWith(w)));
}

// ---------------------------------------------------------------------------
// People
// ---------------------------------------------------------------------------

/** A person a chip can name. */
export interface Person {
  name: string;
  email?: string;
  /** A picture's address. */
  avatar?: string;
  /** A line under the name: a role, a team, "Author of this document". */
  detail?: string;
}

/**
 * A place people come from for person chips. Docxtor's own is local and needs no account (people named in the document,
 * the person's own name, people added by hand); a directory, a server's accounts or a contacts service can be another.
 */
export interface PeopleProvider {
  id: string;
  label: string;
  /** People whose name or email fits `query` (empty: the ones to suggest). */
  search(query: string): Person[] | Promise<Person[]>;
  /** Keep a person the author typed in by hand, so the next search finds them. */
  remember?(person: Person): void | Promise<void>;
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/** A document a file chip can point to. */
export interface FileRef {
  title: string;
  /** Where it opens: an address in the app, or a web address. */
  url: string;
  /** A line under the title: where it is kept, when it changed. */
  detail?: string;
  /** When it was last opened or changed, as an ISO date. */
  modified?: string;
}

/** A place file chips can point to (the app's files list, a drive, a wiki). The editor never reads storage itself. */
export interface FileProvider {
  id: string;
  label: string;
  /** Files whose name fits `query` (empty: the recent ones). */
  search(query: string): FileRef[] | Promise<FileRef[]>;
}

// ---------------------------------------------------------------------------
// Drop-down chips
// ---------------------------------------------------------------------------

/** The colours a drop-down chip's choice can have (tokens in `editor.css`). */
export const CHIP_COLORS = ["gray", "blue", "green", "yellow", "red", "purple"] as const;
export type ChipColor = (typeof CHIP_COLORS)[number];

export interface DropdownOption {
  label: string;
  color?: ChipColor;
}

/** A ready-made drop-down chip: Status, Project status, or one of your own. */
export interface DropdownPreset {
  id: string;
  /** Its name in the menus; it is also the chip's title in the file. */
  label: string;
  description?: string;
  options: DropdownOption[];
}

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

/**
 * A ready-made piece of document for Insert, Building blocks and the @ menu: Docs-style meeting notes, an email draft,
 * a roadmap. It is Markdown (headings, tables, lists, checklists), and these tokens turn into smart chips where they stand:
 * `{{date}}` (today), `{{person}}` (an empty person chip, or `{{person:Ada Lovelace <ada@example.com>}}`),
 * `{{status:In progress}}` (a Status chip), `{{project-status:On track}}` (a Project status chip), `{{dropdown:id:Option}}`
 * (any registered drop-down) and `{{chip:kind:Text}}` (any other chip).
 */
export interface DocBuildingBlock {
  id: string;
  label: string;
  description?: string;
  /** The group it is listed in. */
  category?: string;
  /** An icon id (see `icons`) or SVG markup. */
  icon?: string;
  markdown: string | (() => string);
}

// ---------------------------------------------------------------------------
// Link previews
// ---------------------------------------------------------------------------

/** What is known about the page a link goes to. */
export interface LinkPreview {
  title?: string;
  description?: string;
  /** The site's name. */
  site?: string;
  /** A picture's address. */
  image?: string;
}

/**
 * Looks a link up for its title and description. Docxtor's own card works offline (the address, its domain and, for
 * a link inside the document, the target's text); a provider here needs the network, so it is an extension that is
 * off until someone turns it on and says what it sends.
 */
export interface LinkPreviewProvider {
  id: string;
  label: string;
  /** Skip addresses it cannot preview (default: all web addresses). */
  matches?(url: string): boolean;
  preview(url: string, signal?: AbortSignal): LinkPreview | null | Promise<LinkPreview | null>;
}

// ---------------------------------------------------------------------------
// Chips in the document
// ---------------------------------------------------------------------------

/** The tag of every smart chip's content control starts with this; the rest names the kind (`person`, `date`, `file`, `place`, `dropdown`, or your own). */
export const CHIP_TAG = "docxtor:chip:";

/** The kind of chip a content control's tag names, or null when the tag is not a chip's. */
export function chipKind(tag: string | undefined): string | null {
  return tag && tag.startsWith(CHIP_TAG) && tag.length > CHIP_TAG.length ? tag.slice(CHIP_TAG.length) : null;
}

/**
 * A drop-down chip keeps each choice's colour in the entry's value (`green:Done`), so the file shows the text and
 * Docxtor shows the colour. Returns the colour and the text of a value.
 */
export function parseChoice(value: string): { color: ChipColor; label: string } {
  const i = value.indexOf(":");
  const head = i > 0 ? value.slice(0, i) : "";
  return (CHIP_COLORS as readonly string[]).includes(head) ? { color: head as ChipColor, label: value.slice(i + 1) } : { color: "gray", label: value };
}

export function choiceValue(o: DropdownOption): string {
  return `${o.color ?? "gray"}:${o.label}`;
}
