// What opening a file did, and why a file could not be opened. The engine reports codes, not sentences:
// the editor words them in the person's language (see `describeOpenReport` in @docxtor/editor).

/** The kinds of file the engine opens. `docx` covers .docx, .docm and .dotx (Strict Open XML is a note, not a format). */
export type OpenFormat = "docx" | "flat-opc" | "word2003" | "odt" | "rtf" | "html" | "doc";

/**
 * One thing that was done or lost while opening. Codes: `zip-directory`, `zip-damaged`, `xml-recovered`, `xml-unreadable`,
 * `text-cut`, `parts-missing`, `rels-restored`, `types-restored`, `strict` (repairs and reading Strict Open XML);
 * `picture-unreadable`, `picture-unsupported`, `pictures-linked`, `object-dropped`, `chunk-objects` (a chart or object of imported content the page can't show), `lost-drawings`, `lost-text-boxes`,
 * `lost-headers`, `lost-comments`, `doc-word6`, `nothing-found` (what a conversion left out).
 */
export interface OpenNote {
  code: string;
  /** Names or a short fact (the damaged parts, say). */
  detail?: string;
  count: number;
}

export interface OpenReport {
  format: OpenFormat;
  /** A new .docx was made from another kind of file. */
  converted: boolean;
  notes: OpenNote[];
}

/** What a file's bytes say it is (`Engine.sniff`). */
export type SniffedFormat = "word" | "odt" | "flat-opc" | "word-2003" | "rtf" | "html" | "doc" | "encrypted" | `foreign:${string}` | "unknown";

/**
 * Why a file could not be opened. `code`: `empty`, `unknown` (not a kind of file Docxtor knows), `not-word` (`detail` says
 * what it is: `spreadsheet`, `presentation`, `pdf`, `epub`, `image`, `archive`...), `damaged` (nothing could be read),
 * `no-text` (no document part could be found), `too-big` (past the input budget: too large, too many parts or nested too deeply), `doc-encrypted` (an old Word file with a password) or `encrypted`.
 */
export class OpenError extends Error {
  readonly code: string;
  readonly detail: string;
  constructor(code: string, detail: string, message: string) {
    super(message);
    this.name = "OpenError";
    this.code = code;
    this.detail = detail;
  }
}

/** The engine's `docxtor-open:<code>:<detail>: <text>` errors as `OpenError`s; anything else comes back as it is. */
export function openError(e: unknown): unknown {
  const m = /^docxtor-open:([\w-]+):([^:]*): ([\s\S]*)$/.exec(e instanceof Error ? e.message : String(e));
  return m ? new OpenError(m[1], m[2], m[3]) : e;
}
