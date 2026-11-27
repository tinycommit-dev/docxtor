// The lists Word builds from the text (table of contents, index) and the marks that feed them, as editor actions:
// each one is an op through `exec`, so it is one undo step, and the fields are updated after it.
import type { AutoMarkEntry, IndexOptions, TocOptions } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "./editor";

/** The first paragraph of the table of contents the caret is in, or null. */
export function caretToc(ed: Editor): number | null {
  if (ed.story || !ed.engine) return null;
  const r = ed.range();
  return r ? ed.engine.tocAt(r[0].p) : null;
}

/** The first paragraph of the index the caret is in, or null. */
export function caretIndex(ed: Editor): number | null {
  if (ed.story || !ed.engine) return null;
  const r = ed.range();
  if (!r) return null;
  return ed.engine.indexOptions(r[0].p) ? r[0].p : null;
}

/** Change the table of contents the caret is in (or the first one) and build it again. */
export function setToc(ed: Editor, options: TocOptions): boolean {
  if (ed.readOnly) return false;
  const at = caretToc(ed) ?? undefined;
  const caret = ed.sel;
  if (!ed.exec({ op: "set_toc", ...(at !== undefined ? { at } : {}), options, pages: ed.pageMap() }, { select: caret })) return false;
  // The table moves the headings down: measure again and correct the numbers, in the same undo step.
  ed.exec({ op: "update_toc", pages: ed.pageMap() }, { merge: true, select: caret });
  return true;
}

/** Remove the table of contents the caret is in (or the first one). */
export function removeToc(ed: Editor): boolean {
  if (ed.readOnly) return false;
  const at = caretToc(ed) ?? undefined;
  return !!ed.exec({ op: "remove_toc", ...(at !== undefined ? { at } : {}) }, { select: ed.sel });
}

/** Word's "Update page numbers only": the entries stay, the numbers are measured again. */
export function updateTocPages(ed: Editor): boolean {
  if (ed.readOnly) return false;
  const caret = ed.sel;
  if (!ed.exec({ op: "update_toc", pages_only: true, pages: ed.pageMap() }, { select: caret })) return false;
  ed.exec({ op: "update_toc", pages_only: true, pages: ed.pageMap() }, { merge: true, select: caret });
  return true;
}

/** Add a line to the table of contents at the end of the selection (a hidden `TC` field). */
export function addTocEntry(ed: Editor, o: { text: string; level: number; id?: string; noPage?: boolean }): boolean {
  if (ed.readOnly || ed.story) return false;
  const r = ed.range();
  if (!r) return false;
  return !!ed.exec({ op: "insert_toc_entry", at: r[1], text: o.text, level: o.level, ...(o.id ? { id: o.id } : {}), ...(o.noPage ? { no_page: true } : {}) }, { select: ed.sel });
}

/** Change the index the caret is in (or the first one) and build it again. */
export function setIndex(ed: Editor, options: IndexOptions): boolean {
  if (ed.readOnly) return false;
  const at = caretIndex(ed) ?? undefined;
  const caret = ed.sel;
  if (!ed.exec({ op: "set_index", ...(at !== undefined ? { at } : {}), options, pages: ed.pageMap() }, { select: caret })) return false;
  ed.runUpdateFields(true, caret);
  return true;
}

/** Word's AutoMark: mark the listed texts for the index. Returns how many places were marked. */
export function autoMark(ed: Editor, entries: AutoMarkEntry[]): number {
  if (ed.readOnly || ed.story) return 0;
  const res = ed.exec({ op: "auto_mark_index", entries }, { select: ed.sel });
  return res?.[0]?.count ?? 0;
}

/** Cite a placeholder source (a tag and nothing else yet) at the caret. Returns the tag it got. */
export function insertCitationPlaceholder(ed: Editor, tag?: string): string | null {
  if (ed.readOnly || ed.story) return null;
  const r = ed.range();
  if (!r) return null;
  const before = new Set(ed.engine.sources().map((s) => s.tag));
  const ops = ed.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
  if (!ed.exec([...ops, { op: "insert_citation_placeholder", at: r[0], ...(tag ? { tag } : {}) }])) return null;
  return ed.engine.sources().find((s) => !before.has(s.tag))?.tag ?? tag ?? t("Placeholder");
}

/** What AutoCaption adds when a table or a picture is inserted: the label and where the caption goes. */
export type AutoCaptions = { table?: { label: string; position: "above" | "below" }; image?: { label: string; position: "above" | "below" } };

/** Add the caption AutoCaption asks for to the table or picture that was just inserted. */
export function autoCaption(ed: Editor, kind: "table" | "image") {
  const a = ed.autoCaptions[kind];
  if (a?.label) ed.insertCaption({ label: a.label, text: "", position: a.position });
}

/** The separators Word offers between a chapter number and the caption number. */
export const CHAPTER_SEPARATORS: [string, string][] = [
  ["-", msg("Hyphen")],
  [".", msg("Period")],
  [":", msg("Colon")],
  ["—", msg("Em dash")],
  ["–", msg("En dash")],
];

/**
 * The number a new caption would get: `3` or, numbered by chapter, `2-3`. Mirrors the engine, for the preview in the
 * Caption dialog. Returns null when no heading of the level is numbered above the place.
 */
export function captionNumber(ed: Editor, label: string, target: { id: number; kind: string }, position: "above" | "below", chapter?: { level: number; separator: string }): string | null {
  if (!chapter) return String(ed.nextCaptionNumber(label, target, position));
  const order = Array.from(ed.flow.querySelectorAll<HTMLElement>("[data-p]")).map((e) => Number(e.dataset.p));
  let at: number;
  if (target.kind === "table") {
    const ids = Array.from(ed.flow.querySelectorAll<HTMLElement>(`.ftw[data-id="${target.id}"] [data-p]`)).map((e) => Number(e.dataset.p));
    const edge = position === "above" ? ids[0] : ids[ids.length - 1];
    at = order.indexOf(edge) + (position === "above" ? -0.5 : 0.5);
  } else at = order.indexOf(target.id) + (position === "above" ? -0.5 : 0.5);
  const targets = ed.xrefTargets();
  const heads = targets.headings.filter((x) => x.level === chapter.level && x.number && order.indexOf(x.p) < at);
  const head = heads[heads.length - 1];
  if (!head) return null;
  const from = order.indexOf(head.p);
  const n = targets.captions.filter((c) => c.label === label && order.indexOf(c.p) > from && order.indexOf(c.p) < at).length + 1;
  return `${head.number!.replace(/\.$/, "")}${chapter.separator}${n}`;
}

/** The leaders the dialogs offer, in the order Word lists them. */
export const LEADERS: [import("@docxtor/core").TabLeader, string][] = [
  ["dot", msg("Dots")],
  ["hyphen", msg("Dashes")],
  ["underscore", msg("Line")],
  ["none", msg("None")],
];
