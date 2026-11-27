// Citation sources, snippets and fonts
import type { Fragment, Source } from "@docxtor/core";

/** A source found by a provider: what to show, and the source to add. */
export interface SourceResult {
  /** Stable within the provider (a Zotero item key, a DOI); used to tell results apart. */
  id: string;
  source: Source;
}

/**
 * Somewhere sources can come from. The Citations panel lists the providers under "Add a source", each as a button
 * that opens a dialog fitted to what the provider can do:
 *
 * - `search` gives a search box: results appear as the person types and they tick the ones to add.
 * - `resolve` alone gives an input for something to look up or read (a DOI, BibTeX text): with `multiline` a text
 *   box that can also read a file, otherwise one line.
 *
 * Throw an `Error` with a message fit to show ("Couldn't reach doi.org") when something goes wrong.
 */
export interface SourceProvider {
  id: string;
  /** The button's text in "Add a source", e.g. "From a DOI or URL…". Sentence case, ending with an ellipsis. */
  label: string;
  /** Heading of the dialog; defaults to the label without its ellipsis. */
  title?: string;
  /** A sentence under the heading. */
  intro?: string;
  /** Label of the input. */
  inputLabel?: string;
  placeholder?: string;
  /** Help under the input: say what leaves the device. */
  help?: string;
  /** A line under the buttons in the panel, for what the provider does with the person's data. */
  note?: string;
  /** Text of the button that starts a lookup (default "Look up"). Not used with `search`. */
  action?: string;
  /**
   * Find sources matching what the person typed. Called after they pause typing, with a signal that is aborted
   * when they type more or close the dialog.
   */
  search?(query: string, signal: AbortSignal): Promise<SourceResult[]>;
  /** Turn what was typed, pasted or read from a file into sources. */
  resolve?(input: string, signal: AbortSignal): Promise<Source[]> | Source[];
  /** For `resolve`: a text box instead of one line. */
  multiline?: boolean;
  /** For `resolve` with `multiline`: a file the person can choose instead of pasting. `accept` lists the types (`.bib,.txt`). */
  files?: { accept: string; button: string; label: string };
  /** For `resolve`: run it as the text changes rather than when the person presses the button. */
  live?: boolean;
  /** For `resolve` that finds one source: open it in the form to check before adding (default: list it to tick). */
  review?: boolean;
  /** For `review`: the sentence under the heading of that form ("This is what doi.org has. Fix anything that looks wrong…"). */
  reviewNote?: string;
  /** A line of settings the dialog offers beside the input, such as "Zotero settings…". */
  settings?: { label: string; open(): void | Promise<void> };
}

/**
 * A list of sources kept outside any one document, so every document can use it: Word's master list in the Source
 * Manager. The Citations panel copies sources between the document's list and a library. Docxtor's own library is kept
 * in the browser (`docxtor.library.local`); another can keep it in a cloud store or a shared file. Anything that leaves
 * the device is said in `note`.
 */
export interface SourceLibrary {
  id: string;
  /** Shown in the panel: "On this device". */
  label: string;
  /** One plain sentence about where the list is kept and who can read it. */
  note?: string;
  /** The sources in the library. */
  list(): Promise<Source[]> | Source[];
  /** Replace the library's sources with these. */
  save(sources: Source[]): Promise<void> | void;
}

/** A saved piece of a document: a quick part or an AutoText entry. */
export interface Snippet {
  /** Stable and unique within the store. */
  id: string;
  name: string;
  /** The gallery it is listed in: `Quick parts`, `AutoText`, or one of the person's own. */
  gallery: string;
  category?: string;
  description?: string;
  /** The content, as the fragment the engine copies a selection into (`Engine.copyFragment`). */
  fragment: Fragment;
  /** The start of its text, for lists. */
  text: string;
  /** ISO date it was saved. */
  created: string;
}

/**
 * Where saved snippets (Insert, Quick parts) are kept. Like a source library it holds the whole list: `list` reads
 * it and `save` writes it back, so a store can be a folder, a cloud file or a server. Docxtor's own store keeps them in
 * the browser (`docxtor.snippets.local`). When several stores are on, the person saves into the one they choose and
 * sees the snippets of all of them.
 */
export interface SnippetStore {
  id: string;
  label: string;
  /** One plain sentence about where snippets are kept and who can read them. */
  note?: string;
  list(): Promise<Snippet[]> | Snippet[];
  save(snippets: Snippet[]): Promise<void> | void;
}

export interface FontContribution {
  /** The family name documents use: "Lato", "Acme Sans". */
  family: string;
  /** A stylesheet with its @font-face rules (Google Fonts, your own CDN). */
  stylesheet?: string;
  /** Or the font files themselves. */
  faces?: { src: string; weight?: string; style?: "normal" | "italic" }[];
  /** Word fonts it can stand in for, with matching metrics (e.g. Carlito for `["Calibri"]`). */
  substitutes?: string[];
  /** `sans`, `serif`, `mono`, `display` or `handwriting`: picks the fallback while it loads. */
  category?: "sans" | "serif" | "mono" | "display" | "handwriting";
  /** List it in the font menu (default true). Pure stand-ins can set false. */
  menu?: boolean;
}

// ---------------------------------------------------------------------------
// AI
// ---------------------------------------------------------------------------

