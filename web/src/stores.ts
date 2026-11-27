// Where the app keeps what is shared between documents: saved quick parts, the shared list of sources and the person's
// own templates. Each is a contribution (`snippets.stores`, `citations.libraries`, `templates.stores`), so an extension
// can keep them somewhere else (a server, a team drive). These keep them in this browser; the editor itself never
// touches storage.
import { defineExtension, msg, t, type Extension, type Snippet, type Source, type TemplateInfo } from "@docxtor/editor";
import { del, get, getAll, put } from "./files/db";

const SNIPPETS_KEY = "docxtor.snippets";
const SOURCES_KEY = "docxtor.shared-sources";

function read<T>(key: string): T[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? "[]");
    return Array.isArray(v) ? (v as T[]) : [];
  } catch {
    return [];
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    throw new Error(t("This browser has no room left to keep it. Remove something saved, or free some space."));
  }
}

/** Saved quick parts, kept in this browser. */
export function localSnippets(): Extension {
  return defineExtension({
    id: "docxtor.snippets.local",
    name: msg("Quick parts on this device"),
    description: msg("Keeps your saved quick parts and AutoText in this browser."),
    activate(ctx) {
      ctx.snippets.stores.register({
        id: "docxtor.snippets.local",
        label: t("On this device"),
        note: t("Kept in this browser only. Clearing site data removes them."),
        list: () => read<Snippet>(SNIPPETS_KEY),
        save: (all) => write(SNIPPETS_KEY, all),
      });
    },
  });
}

/** The shared list of sources (Word's master list), kept in this browser. */
export function localSourceLibrary(): Extension {
  return defineExtension({
    id: "docxtor.library.local",
    name: msg("Shared sources on this device"),
    description: msg("Keeps a list of sources that every document can use, in this browser."),
    activate(ctx) {
      ctx.citations.libraries.register({
        id: "docxtor.library.local",
        label: t("On this device"),
        note: t("Kept in this browser only. Clearing site data removes it."),
        list: () => read<Source>(SOURCES_KEY),
        save: (all) => write(SOURCES_KEY, all),
      });
    },
  });
}

interface StoredTemplate {
  /** The database numbers them. */
  id?: number;
  name: string;
  description?: string;
  created: string;
  bytes: Uint8Array;
}

/** The person's own templates (Word template files), kept in this browser. */
export function localTemplates(): Extension {
  return defineExtension({
    id: "docxtor.templates.local",
    name: msg("Templates on this device"),
    description: msg("Keeps the templates you save in this browser."),
    activate(ctx) {
      const info = (r: StoredTemplate): TemplateInfo => ({ id: String(r.id), name: r.name, description: r.description, created: r.created });
      ctx.templates.stores.register({
        id: "docxtor.templates.local",
        label: t("On this device"),
        note: t("Kept in this browser only. Clearing site data removes them."),
        async list() {
          return (await getAll<StoredTemplate>("templates")).sort((a, b) => a.name.localeCompare(b.name)).map(info);
        },
        async read(id) {
          const r = await get<StoredTemplate>("templates", Number(id));
          if (!r) throw new Error(t("That template isn't kept here any more."));
          return r.bytes;
        },
        async save({ name, description }, bytes) {
          const record: StoredTemplate = { name, description, created: new Date().toISOString(), bytes };
          const id = (await put("templates", record)) as number;
          return info({ ...record, id });
        },
        async remove(id) {
          await del("templates", Number(id));
        },
      });
    },
  });
}
