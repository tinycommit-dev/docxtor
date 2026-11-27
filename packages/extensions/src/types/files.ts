// Exporters, importers and templates
import type { Engine } from "@docxtor/core";

export interface Exporter {
  id: string;
  /** "Markdown", "Plain text", "PDF"… */
  label: string;
  /** File extension without the dot. */
  extension: string;
  mime: string;
  export(engine: Engine): Uint8Array | string | Blob | Promise<Uint8Array | string | Blob>;
}

/**
 * What opening a file did that the person should hear about: a repair, or a conversion and what it left out. `title`
 * is one sentence; `notes` are plain-language lines, already in the person's language.
 */
export interface OpenNotice {
  title: string;
  notes: string[];
}

/**
 * What importers and templates produce: a .docx (with `notice` when the importer converted another kind of file and
 * wants the person told), or Markdown to build one from.
 */
export type DocumentSource = { docx: Uint8Array; notice?: OpenNotice } | { markdown: string };

export interface Importer {
  id: string;
  label: string;
  /** File extensions it reads, without dots: `["md", "markdown"]`. */
  extensions: string[];
  /**
   * Whether the bytes are this kind of file, for files whose name doesn't say (an RTF file called .docx) and for
   * extensions several importers share (.xml). Importers without it are chosen by extension alone.
   */
  detect?(bytes: Uint8Array): boolean;
  import(bytes: Uint8Array, name: string): DocumentSource | Promise<DocumentSource>;
}

export interface Template {
  id: string;
  name: string;
  description?: string;
  /** File name for documents made from it (default: the template's name). */
  fileName?: string;
  create(): DocumentSource | Promise<DocumentSource>;
}

/** A template as the New from a template dialog lists it: built in (registered) or kept in a template store. */
export interface TemplateEntry {
  id: string;
  name: string;
  description?: string;
  fileName?: string;
  /** Where it comes from, in words: "Built in", or a store's label. */
  source: string;
  /** Set for a template a store keeps: removes it from there. */
  remove?(): Promise<void>;
  create(): DocumentSource | Promise<DocumentSource>;
}

/** A saved template, without its file. */
export interface TemplateInfo {
  id: string;
  name: string;
  description?: string;
  /** ISO date it was saved. */
  created: string;
}

/**
 * Where the person's own templates are kept (File, Save as template). A template is a Word template file (`.dotx`),
 * so its styles, page setup, headers and text come with it and any other program can open it. Docxtor's own store keeps
 * them in the browser (`docxtor.templates.local`); another can keep them in a folder, a cloud drive or on a server. When
 * several stores are on, the person saves into the one they choose and sees the templates of all of them.
 */
export interface TemplateStore {
  id: string;
  label: string;
  /** One plain sentence about where templates are kept and who can read them. */
  note?: string;
  list(): Promise<TemplateInfo[]> | TemplateInfo[];
  /** The template's `.dotx` bytes. */
  read(id: string): Promise<Uint8Array> | Uint8Array;
  /** Keep a new template; the store gives it its id. */
  save(info: { name: string; description?: string }, bytes: Uint8Array): Promise<TemplateInfo> | TemplateInfo;
  remove(id: string): Promise<void> | void;
}

