// Fields in the editor: what the engine can't know and the page tells it (the time, the user, the file), updating when a
// file opens and before printing or downloading, and the fields extensions evaluate. The engine works out the rest.
import type { FieldInput, Op } from "@docxtor/core";
import { parseInstruction, type FieldCall } from "@docxtor/extensions";
import type { Editor } from "./editor";

/** What the host around the editor provides for fields. */
export interface FieldServices {
  /** The person: `USERNAME`, `USERINITIALS` and `USERADDRESS` show these (empty when unknown). */
  user(): { name?: string; initials?: string; address?: string };
  /** An extension evaluates fields with this name. */
  evaluates(code: string): boolean;
  /** Work out a field with an extension's evaluator; null leaves the result as it is. */
  evaluate(call: FieldCall): Promise<string | null>;
  /** Whether fields are updated before printing and downloading (a setting, on by default). */
  updateBeforeOutput(): boolean;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** The time as the reader's clock shows it, in the form the engine reads: `2026-10-13T09:30:00`. */
export function localIso(d = new Date()): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/**
 * What the engine needs from the page to work out fields. The file's size costs a save, so it is only worked out
 * (`withSize`) when fields are being updated and one of them asks for it.
 */
export function fieldInput(ed: Editor, withSize = false): FieldInput {
  const user = ed.fieldServices?.user() ?? {};
  const input: FieldInput = { now: localIso(), pages: ed.pages, file_name: ed.engine.fileName };
  if (user.name) input.user_name = user.name;
  if (user.initials) input.user_initials = user.initials;
  if (user.address) input.user_address = user.address;
  try {
    if (withSize && ed.engine.listFields().some((f) => f.code === "FILESIZE")) input.file_size = ed.engine.save().length;
  } catch {
    // The size stays unknown and the field keeps its result.
  }
  return input;
}

/**
 * Update the fields as a file opens: dates and times, fields marked dirty, and every field when the file asks for it. It
 * leaves nothing to undo and doesn't count as a change by the person.
 */
export function updateOnOpen(ed: Editor) {
  try {
    // Most documents have no fields: nothing to look at.
    if (!ed.engine.listFields().length) return;
    const r = ed.engine.apply({ op: "update_fields", open: true, pages: ed.withDrawn(() => ed.pageMap()), input: fieldInput(ed, true) }, false);
    if ((r[0]?.count ?? 0) > 0) ed.refresh({ quiet: true });
  } catch {
    // A field the engine can't work out leaves the document as it was.
  }
}

/**
 * Update the fields before printing or downloading, when the setting is on. One undoable step; nothing is recorded when
 * nothing changes. A read-only document is updated too (the engine lets this update through and records no undo step):
 * page numbers and cross-references print right whoever may edit. A second pass follows the first, which can move
 * headings the page numbers measure. Returns how many changed.
 */
export function updateForOutput(ed: Editor): number {
  if (!ed.engine || !(ed.fieldServices?.updateBeforeOutput() ?? true)) return 0;
  if (ed.story) ed.exitStory(false);
  try {
    const run = (merge: boolean) => ed.engine.apply({ op: "update_fields", quiet: true, pages: ed.withDrawn(() => ed.pageMap()), input: fieldInput(ed, true) }, merge)[0]?.count ?? 0;
    const a = run(false);
    if (!a) return 0;
    ed.refresh();
    const b = run(true);
    if (b) ed.refresh();
    return a + b;
  } catch {
    return 0;
  }
}

/** Update the fields that extensions evaluate. They go into the undo step the update before them made, when `merge` is set. */
export async function updateExtensionFields(ed: Editor, merge: boolean): Promise<number> {
  const svc = ed.fieldServices;
  if (!svc || ed.readOnly || !ed.engine) return 0;
  const todo = ed.engine.listFields().filter((f) => !f.story && f.plain && !f.locked && svc.evaluates(f.code));
  if (!todo.length) return 0;
  const now = new Date();
  const ops: (Op & { at: { p: number; o: number } })[] = [];
  for (const f of todo) {
    const p = parseInstruction(f.instr);
    const text = await svc.evaluate({ instr: f.instr, code: f.code, args: p.args, switches: p.switches, formats: p.formats, datePicture: p.datePicture, numberPicture: p.numberPicture, engine: ed.engine, now });
    if (text !== null && text !== f.result) ops.push({ op: "set_field_result", at: { p: f.p, o: f.o }, text });
  }
  if (!ops.length) return 0;
  // Later fields first, so an earlier one's new length doesn't move the offsets of the rest.
  ops.sort((a, b) => a.at.p - b.at.p || b.at.o - a.at.o);
  return ed.exec(ops, { merge, select: ed.sel }) ? ops.length : 0;
}
