// Inspectors and document comparison
import type { DocumentApi } from "./api.ts";
import type { CompareBlock, Op } from "@docxtor/core";

/** What an inspector finds. */
export interface InspectorFinding {
  /** How many there are. 0 means none. */
  count: number;
  /** What they are, in a few short items (names, counts), where that helps. */
  detail?: string[];
}

/** What an inspector is given. */
export interface InspectorContext {
  /** The open document. */
  document: DocumentApi;
  /** Aborted when the person closes the dialog. */
  signal: AbortSignal;
  /**
   * Edits to make, for `remove`. Nothing is applied until every inspector the person chose has finished: then all
   * the edits go in together as one undoable step. `inspect` can't use it.
   */
  exec(ops: Op | Op[]): void;
}

/**
 * A check for something a file carries that the page doesn't show (Review, Inspect document): personal information,
 * hidden text, comments, internal links, a company's own marks... Docxtor's own are inspectors too. `remove` takes
 * what the check found out of the document, through ops so it can be undone; without it the inspector only reports.
 */
export interface Inspector {
  id: string;
  /** The kind of thing it looks for, sentence case: "Comments". */
  label: string;
  /** What is in the file and why it might matter, in a sentence. */
  description?: string;
  /** Inspectors are listed from the lowest `order` up; Docxtor's own run from 10 to 80. */
  order?: number;
  inspect(ctx: InspectorContext): InspectorFinding | Promise<InspectorFinding>;
  /** Take what `inspect` found out of the document. */
  remove?(ctx: InspectorContext): void | Promise<void>;
}

/** An inspector's result, as the host hands it on. */
export interface InspectorReport {
  id: string;
  label: string;
  description?: string;
  count: number;
  detail: string[];
  /** The inspector can take what it found out. */
  removable: boolean;
  /** Set when the inspector failed; `count` is 0 then. */
  error?: string;
}

// ---------------------------------------------------------------------------
// Compare and combine
// ---------------------------------------------------------------------------

/**
 * Something that compares two versions of a document: Docxtor's own engine (words, paragraphs and formatting), or
 * another way of telling what changed. Review, Compare and Combine use the latest registered comparer that has what
 * they need. The documents are .docx bytes.
 */
export interface DocumentComparer {
  id: string;
  label: string;
  /** Paragraph by paragraph, the newer document against the older one (see `Engine.compareView`). */
  compare(newer: Uint8Array, older: Uint8Array): CompareBlock[] | Promise<CompareBlock[]>;
  /** A .docx of the newer document with tracked changes showing how it differs from the older one, by `author`. */
  redline?(newer: Uint8Array, older: Uint8Array, author: string): Uint8Array | Promise<Uint8Array>;
  /** A .docx of the revised document with what differs from the original marked as tracked changes by `author`; the revised document's own tracked changes stay. */
  combine?(revised: Uint8Array, original: Uint8Array, author: string): Uint8Array | Promise<Uint8Array>;
}

// ---------------------------------------------------------------------------
// Fields
// ---------------------------------------------------------------------------

