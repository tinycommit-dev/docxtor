// Smart chips in the document. A chip is an inline content control (`w:sdt`) whose tag starts with `docxtor:chip:` and
// whose text is real text, so Word shows it: a person or file chip is a rich text control around a hyperlink (`mailto:`
// for a person, the file's address for a file), a date chip is a date picker, a drop-down chip is a drop-down list.
// This file makes and changes them with ops, and says how the page draws them; the cards that open from a chip are in
// ui/chip-cards.ts and the commands in builtins/smart-chips.ts.
import type { ControlProps, MPos, Op, RControl } from "@docxtor/core";
import { CHIP_TAG, choiceValue, chipKind, parseChoice, type DropdownOption } from "@docxtor/extensions";
import type { Editor } from "./editor";

/** A chip made of text, with a link or without. */
export interface TextChip {
  kind: string;
  text: string;
  /** Where the text links to: `mailto:` for a person, an address for a file or a place. */
  url?: string;
  /** The control's title; Word shows it in the control's tab. */
  alias?: string;
}

/** What the person has selected, or the caret. */
function where(ed: Editor): { from: MPos; to: MPos } | null {
  const r = ed.range();
  return r ? { from: r[0], to: r[1] } : null;
}

/** Whether a space should follow a chip put at `at`: the text there does not already start with whitespace. */
function wantsSpace(ed: Editor, at: MPos): boolean {
  const next = ed.engine.textRange(at, { p: at.p, o: at.o + 1 });
  return next === "" || !/^[\s.,;:!?)\]]/.test(next);
}

/**
 * Put a text chip in at `at`, replacing `to` when given. The text goes in first, then the link, then the control around
 * both, and a space after it, all as one undo step. Returns the position just after the chip's space.
 */
export function insertTextChip(ed: Editor, chip: TextChip, opts: { at?: MPos; to?: MPos; merge?: boolean } = {}): MPos | null {
  const sel = opts.at ? { from: opts.at, to: opts.to ?? opts.at } : where(ed);
  if (!sel || !ed.engine || ed.readOnly) return null;
  const { from, to } = sel;
  const ops: Op[] = [];
  const collapsed = from.p === to.p && from.o === to.o;
  if (!collapsed) ops.push({ op: "delete", from, to });
  const end = { p: from.p, o: from.o + chip.text.length };
  // The space is written with the text so the control wraps the text only.
  const space = wantsSpace(ed, to) ? " " : "";
  ops.push({ op: "insert_text", at: from, text: chip.text + space });
  if (chip.url) ops.push({ op: "set_link", from, to: end, url: chip.url });
  ops.push({ op: "control", action: "insert", at: from, to: end, kind: "richText", props: { alias: chip.alias, tag: CHIP_TAG + chip.kind } });
  const caret = { p: from.p, o: end.o + space.length };
  return ed.exec(ops, { select: caret, merge: opts.merge }) ? caret : null;
}

/** The date picker chip for a day (`2026-10-14`), in a format and language. */
export function insertDateChip(ed: Editor, iso: string, opts: { format: string; lid?: string; at?: MPos; to?: MPos; merge?: boolean; alias?: string }): MPos | null {
  return insertPickerChip(ed, "date", { alias: opts.alias ?? "Date", dateFormat: opts.format, lid: opts.lid }, opts, (n, p) => ({ op: "control", action: "set", target: { p, n }, date: iso }));
}

/** A drop-down chip with its options, showing the option at `selected` (an index into `options`). */
export function insertDropdownChip(ed: Editor, title: string, options: DropdownOption[], selected = 0, opts: { at?: MPos; to?: MPos; merge?: boolean } = {}): MPos | null {
  // The engine puts the prompt (the placeholder) first in the list itself; the choices follow it.
  const items = options.map((o) => ({ display: o.label, value: choiceValue(o) }));
  return insertPickerChip(ed, "dropDown", { alias: title, items, placeholder: title }, opts, (n, p) => ({ op: "control", action: "set", target: { p, n }, index: selected + 1 }), "dropdown");
}

function insertPickerChip(ed: Editor, kind: "date" | "dropDown", props: ControlProps, opts: { at?: MPos; to?: MPos; merge?: boolean }, fill: (n: number, p: number) => Op, tagKind: string = kind): MPos | null {
  const sel = opts.at ? { from: opts.at, to: opts.to ?? opts.at } : where(ed);
  if (!sel || !ed.engine || ed.readOnly) return null;
  const { from, to } = sel;
  const tag = CHIP_TAG + tagKind;
  const ops: Op[] = [];
  if (from.p !== to.p || from.o !== to.o) ops.push({ op: "delete", from, to });
  const space = wantsSpace(ed, to) ? " " : "";
  if (space) ops.push({ op: "insert_text", at: from, text: space });
  ops.push({ op: "control", action: "insert", at: from, kind, props: { ...props, tag } });
  if (!ed.exec(ops, { select: from, merge: opts.merge })) return null;
  // The control's number is known only now.
  const c = ed.paragraphView(from.p)?.controls?.find((x) => x.tag === tag && x.start === from.o);
  if (!c) return null;
  ed.exec(fill(c.n, from.p), { select: { p: from.p, o: c.end }, merge: true });
  // The chip's text changed (the prompt became the date or the choice): the caret goes after it as it is now.
  const now = ed.paragraphView(from.p)?.controls?.find((x) => x.n === c.n) ?? c;
  const caret = { p: from.p, o: now.end + space.length };
  ed.select(caret);
  return caret;
}

/** Take the chip away but keep its words: the control goes, and so does the link of a person or file chip. */
export function chipToText(ed: Editor, p: number, c: RControl): boolean {
  const ops: Op[] = [{ op: "control", action: "remove", target: { p, n: c.n } }];
  if (c.kind === "richText") ops.push({ op: "set_link", from: { p, o: c.start }, to: { p, o: c.end }, url: null });
  return !!ed.exec(ops, { select: { p, o: c.end } });
}

/** Delete the chip and its words. */
export function removeChip(ed: Editor, p: number, c: RControl): boolean {
  return !!ed.exec([{ op: "control", action: "remove", target: { p, n: c.n }, delete_content: true }], { select: { p, o: c.start } });
}

/** The chip around a range, if the whole range is inside one. */
export function chipAround(ed: Editor, from: MPos, to: MPos): { p: number; c: RControl } | null {
  if (from.p !== to.p) return null;
  const c = ed.paragraphView(from.p)?.controls?.find((x) => chipKind(x.tag) !== null && from.o >= x.start && to.o <= x.end && (from.o < x.end || to.o === x.end));
  return c ? { p: from.p, c } : null;
}

// ---------------------------------------------------------------------------
// How the page draws a chip
// ---------------------------------------------------------------------------

/**
 * Mark the segments of a chip so the stylesheet can draw the pill (chips.css): `data-chip` names the kind, a drop-down
 * chip's `data-chip-color` is its choice's colour, and a chip that still shows its prompt is `data-chip-empty`.
 * Nothing is wrapped around the text; the paragraph's segments stay its direct children.
 */
export function decorateChip(segs: HTMLElement[], c: RControl, label: (kind: string) => string) {
  const kind = chipKind(c.tag);
  if (!kind || !segs.length) return;
  const text = segs.map((s) => s.textContent ?? "").join("");
  let color = "gray";
  if (c.kind === "dropDown") {
    const hit = c.items?.find((i) => i.display === text);
    if (hit) color = parseChoice(hit.value).color;
  }
  const name = kind === "dropdown" && c.alias ? c.alias : label(kind);
  segs.forEach((s) => {
    s.classList.add("chip");
    s.dataset.chip = kind;
    s.dataset.chipColor = color;
    s.classList.toggle("chip-empty", !!c.placeholder);
    s.setAttribute("role", "button");
    s.setAttribute("aria-haspopup", "dialog");
    s.setAttribute("aria-label", `${name}: ${text}`);
  });
  segs[0].title = `${name}: ${text}`;
}
