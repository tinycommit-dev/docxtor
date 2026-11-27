// Quick parts: saved snippets (Word's building blocks and AutoText). They live in snippet stores that extensions
// register (the app keeps its own in the browser), plus the building blocks the open document's glossary holds, which
// can be inserted but not changed. The editor never touches storage itself: it asks the stores.
import type { Op } from "@docxtor/core";
import { msg, type ExtensionHost, type Snippet, type SnippetStore } from "@docxtor/extensions";
import type { Editor } from "./editor";

/** The galleries Word starts with. A snippet can name any other. */
export const QUICK_PARTS = "Quick parts";
export const AUTO_TEXT = "AutoText";

/** Names for the galleries a glossary document uses (`w:gallery`). */
const GALLERY_NAMES: Record<string, string> = {
  autoText: msg("AutoText"),
  docParts: msg("Quick parts"),
  custQuickParts: msg("Quick parts"),
  tblOfContents: msg("Table of contents"),
  coverPg: msg("Cover pages"),
  hdrs: msg("Headers"),
  ftrs: msg("Footers"),
  pgNum: msg("Page numbers"),
  tbls: msg("Tables"),
  txtBox: msg("Text boxes"),
  eq: msg("Equations"),
  watermarks: msg("Watermarks"),
  bib: msg("Bibliographies"),
};

/** A part the person can insert: a saved snippet, or a building block of the open document. */
export interface Part {
  /** The snippet's id, or `glossary:<index>` for a building block of the document. */
  id: string;
  name: string;
  gallery: string;
  category?: string;
  description?: string;
  text: string;
  /** The store that holds it; null for the document's glossary. */
  store: string | null;
  snippet?: Snippet;
  /** The glossary index, for a building block. */
  block?: number;
}

export function stores(host: ExtensionHost): SnippetStore[] {
  return host.snippetStores.list();
}

/** Every part: the saved snippets of all stores, then the document's own building blocks. */
export async function listParts(host: ExtensionHost, ed: Editor): Promise<Part[]> {
  const out: Part[] = [];
  for (const store of stores(host)) {
    let list: Snippet[] = [];
    try {
      list = await store.list();
    } catch {
      // A store that can't be read lists nothing.
    }
    for (const s of list) out.push({ id: s.id, name: s.name, gallery: s.gallery, category: s.category, description: s.description, text: s.text, store: store.id, snippet: s });
  }
  if (ed.engine) {
    for (const b of ed.engine.buildingBlocks()) {
      out.push({ id: `glossary:${b.index}`, name: b.name, gallery: b.gallery ? (GALLERY_NAMES[b.gallery] ?? b.gallery) : QUICK_PARTS, category: b.category, description: b.description, text: b.text, store: null, block: b.index });
    }
  }
  return out;
}

const newId = () => (typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `s${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`);

/** The first words of a text, for a name to suggest and for lists. */
export function snippetName(text: string): string {
  const words = text.replace(/\s+/g, " ").trim().split(" ").slice(0, 4).join(" ");
  return words.length > 40 ? `${words.slice(0, 39).trimEnd()}…` : words;
}

/** Save the selection as a part in a store. Returns the snippet, or null when nothing is selected. */
export async function savePart(host: ExtensionHost, ed: Editor, storeId: string, o: { name: string; gallery: string; category?: string; description?: string }): Promise<Snippet | null> {
  const r = ed.range();
  if (!r || ed.collapsed() || !ed.engine) return null;
  const fragment = ed.engine.copyFragment(r[0], r[1]);
  const store = stores(host).find((s) => s.id === storeId);
  if (!fragment || !store) return null;
  const text = ed.selectedText().replace(/\s+/g, " ").trim();
  const snippet: Snippet = {
    id: newId(),
    name: o.name.trim(),
    gallery: o.gallery.trim() || QUICK_PARTS,
    ...(o.category?.trim() ? { category: o.category.trim() } : {}),
    ...(o.description?.trim() ? { description: o.description.trim() } : {}),
    fragment,
    text: text.length > 120 ? `${text.slice(0, 119)}…` : text,
    created: new Date().toISOString(),
  };
  await store.save([...(await store.list()), snippet]);
  return snippet;
}

/** Change a saved part's name, gallery, category or description. */
export async function updatePart(host: ExtensionHost, part: Part, patch: { name: string; gallery: string; category?: string; description?: string }): Promise<boolean> {
  const store = stores(host).find((s) => s.id === part.store);
  if (!store) return false;
  const list = await store.list();
  const hit = list.find((s) => s.id === part.id);
  if (!hit) return false;
  hit.name = patch.name.trim() || hit.name;
  hit.gallery = patch.gallery.trim() || QUICK_PARTS;
  if (patch.category?.trim()) hit.category = patch.category.trim();
  else delete hit.category;
  if (patch.description?.trim()) hit.description = patch.description.trim();
  else delete hit.description;
  await store.save(list);
  return true;
}

export async function removePart(host: ExtensionHost, part: Part): Promise<boolean> {
  const store = stores(host).find((s) => s.id === part.store);
  if (!store) return false;
  const list = await store.list();
  if (!list.some((s) => s.id === part.id)) return false;
  await store.save(list.filter((s) => s.id !== part.id));
  return true;
}

/** Put a part in at the caret (a selection is replaced). */
export function insertPart(ed: Editor, part: Part): boolean {
  if (ed.readOnly || ed.story || !ed.engine) return false;
  const r = ed.range();
  if (!r) return false;
  const fragment = part.snippet ? part.snippet.fragment : part.block !== undefined ? ed.engine.buildingBlockFragment(part.block) : null;
  if (!fragment) return false;
  const ops: Op[] = ed.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
  ops.push({ op: "insert_fragment", at: r[0], fragment });
  return !!ed.exec(ops);
}

/** Word's F3: the AutoText named by the word before the caret takes its place. Returns whether one did. */
export async function expandAutoText(host: ExtensionHost, ed: Editor): Promise<boolean> {
  if (ed.readOnly || ed.story || !ed.engine || !ed.collapsed()) return false;
  const r = ed.range();
  if (!r) return false;
  const before = ed.engine.textRange({ p: r[0].p, o: 0 }, r[0]);
  const word = /(\S+)$/.exec(before)?.[1];
  if (!word) return false;
  const parts = await listParts(host, ed);
  const hit = parts.find((p) => p.name.toLowerCase() === word.toLowerCase() && p.gallery === AUTO_TEXT);
  if (!hit) return false;
  const fragment = hit.snippet ? hit.snippet.fragment : hit.block !== undefined ? ed.engine.buildingBlockFragment(hit.block) : null;
  if (!fragment) return false;
  const start = { p: r[0].p, o: r[0].o - word.length };
  return !!ed.exec([
    { op: "delete", from: start, to: r[0] },
    { op: "insert_fragment", at: start, fragment },
  ]);
}
