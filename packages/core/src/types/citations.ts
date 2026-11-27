// Citations and the bibliography
import type { MPos } from "./patches.ts";

// ---------------------------------------------------------------------------
// Citations and the bibliography (Word's References tab: Manage Sources, Insert Citation, Bibliography)
// ---------------------------------------------------------------------------

export type SourceType = "book" | "article" | "chapter" | "conference" | "report" | "website" | "thesis" | "misc";

/** A person (`family`, `given?`) or an organisation (`literal`). */
export type SourceName = { family: string; given?: string } | { literal: string };

/**
 * One source in the document's list (`Engine.sources()`); Word's `b:Source` in `customXml/item1.xml`.
 * On input `year`, `month`, `day`, `volume`, `issue`, `pages` and `edition` may be numbers, and `authors` may be
 * strings ("Smith, John" or "John Smith"); on output they are strings and name objects.
 */
export interface Source {
  /** The short key citations use (`Smi20`); made like Word does when empty on `add_source`. */
  tag: string;
  type: SourceType;
  authors: SourceName[];
  editors?: SourceName[];
  title: string;
  /** The journal, book, proceedings or website the source sits in. */
  containerTitle?: string;
  year?: string;
  /** A month number ("3") or name ("March"). */
  month?: string;
  day?: string;
  publisher?: string;
  /** City of publication. */
  place?: string;
  volume?: string;
  issue?: string;
  pages?: string;
  edition?: string;
  url?: string;
  doi?: string;
  /** When a web source was read, `YYYY-MM-DD` (month and day optional). */
  accessed?: string;
  /** "Doctoral dissertation", "Technical report", ... */
  genre?: string;
  /** Word's `{GUID}` for the source. */
  guid?: string;
}

/** Input for `add_source` / `update_source`: the same, with everything optional. */
export type SourceInput = Partial<Omit<Source, "authors" | "editors" | "year" | "month" | "day" | "volume" | "issue" | "pages" | "edition">> & {
  authors?: (SourceName | string)[];
  editors?: (SourceName | string)[];
  year?: string | number;
  month?: string | number;
  day?: string | number;
  volume?: string | number;
  issue?: string | number;
  pages?: string | number;
  edition?: string | number;
};

/** One of Docxtor's five styles (`apa`, `mla`, `chicago`, `ieee`, `harvard`), or the id of a CSL style added with `Engine.registerCitationStyle`. */
export type CitationStyle = "apa" | "mla" | "chicago" | "ieee" | "harvard" | (string & {});

/** A CSL style added to a document (`Engine.registerCitationStyle`, `Engine.citationStyles`). */
export interface CitationStyleInfo {
  id: string;
  title: string;
  /** Its citations are footnotes. */
  note: boolean;
  /** Its citations are numbers. */
  numeric: boolean;
}

/** A citation in the text (`Engine.citations()`): a content control around a `CITATION` field. */
export interface CitationInfo {
  /** The paragraph holding it and the UTF-16 offset where its text starts. */
  p: number;
  o: number;
  /** How many characters its text takes (the formatted result, e.g. `(Smith, 2020)`). */
  len: number;
  tags: string[];
  page?: string;
  suppressAuthor?: boolean;
  suppressYear?: boolean;
  suppressTitle?: boolean;
}

/**
 * Add a source. An empty `tag` is made like Word does (three letters of the first author's surname and two digits
 * of the year, then `a`, `b`, ... if taken); read it back from `Engine.sources()` (the last one). Undoable.
 */
export interface AddSourceOp {
  op: "add_source";
  source: SourceInput;
}

/** Replace a source. Citations and bibliographies using it are written again; a new `source.tag` renames it everywhere. */
export interface UpdateSourceOp {
  op: "update_source";
  tag: string;
  source: SourceInput;
}

/** Remove a source. An error while citations use it, unless `force` (they become plain text). */
export interface DeleteSourceOp {
  op: "delete_source";
  tag: string;
  force?: boolean;
}

/**
 * Choose the style; every citation and bibliography is written again. The result's `count` is how many fields.
 * Switching to a note style turns the citations into footnotes; switching away turns footnotes that hold only a
 * citation back into citations.
 */
export interface SetCitationStyleOp {
  op: "set_citation_style";
  style: CitationStyle;
}

/** Insert a citation at `at`; the result's `caret` is just after it. `page` is the locator ("23" or "23-25"). */
export interface InsertCitationOp {
  op: "insert_citation";
  at: MPos;
  tags: string[];
  page?: string;
  suppress_author?: boolean;
  suppress_year?: boolean;
  suppress_title?: boolean;
}

/** Change the citation at (inside, at the end of, or at the start of) `at`. Empty/absent `tags` keep the sources; `page: ""` removes the locator. */
export interface EditCitationOp {
  op: "edit_citation";
  at: MPos;
  tags?: string[];
  page?: string;
  suppress_author?: boolean;
  suppress_year?: boolean;
  suppress_title?: boolean;
}

/** Insert the bibliography (a content control with the heading and a `BIBLIOGRAPHY` field); `created` lists its paragraphs. It lists cited sources only. */
export interface InsertBibliographyOp {
  op: "insert_bibliography";
  after?: number | string;
  before?: number | string;
  /** Defaults to the style's heading (References, Works Cited, Bibliography); "" leaves it out. */
  heading?: string;
}

/** Write every citation and the bibliography again; the result's `count` is how many fields changed. */
export interface UpdateBibliographyOp {
  op: "update_bibliography";
}

