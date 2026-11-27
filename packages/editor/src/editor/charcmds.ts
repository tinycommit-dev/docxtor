// Commands on the text itself that reach past a patch of formatting: changing case (and Shift+F3, which cycles it),
// applying a style from the style picker (character styles, and linked styles that act as paragraph or character
// styles depending on the selection), and clearing a character style.
import { msg } from "@docxtor/extensions";
import type { MPos, StyleInfo } from "@docxtor/core";
import type { Editor } from "./editor";

export type CaseMode = "sentence" | "lower" | "upper" | "title" | "toggle";

export const CASE_MODES: { mode: CaseMode; title: string }[] = [
  { mode: "sentence", title: msg("Sentence case") },
  { mode: "lower", title: msg("lowercase") },
  { mode: "upper", title: msg("UPPERCASE") },
  { mode: "title", title: msg("Capitalize Each Word") },
  { mode: "toggle", title: msg("tOGGLE cASE") },
];

const WORD = /[\p{L}\p{N}'’_]/u;

/** The paragraph's text up to its end (the engine clamps the offset). */
function paragraphText(ed: Editor, p: number): string {
  return ed.engine.textRange({ p, o: 0 }, { p, o: 1_000_000 });
}

/** The word a caret is in or touches (null between spaces). */
export function wordAround(ed: Editor, at: MPos): [MPos, MPos] | null {
  const text = paragraphText(ed, at.p);
  let a = Math.min(at.o, text.length);
  let b = a;
  while (a > 0 && WORD.test(text[a - 1])) a--;
  while (b < text.length && WORD.test(text[b])) b++;
  return a === b ? null : [{ p: at.p, o: a }, { p: at.p, o: b }];
}

/** The ranges a case change reaches: every selected range, or the word at the caret. */
function caseRanges(ed: Editor): [MPos, MPos][] {
  const r = ed.range();
  if (!r) return [];
  if (ed.multi) return ed.ranges().filter((x) => x.from.p !== x.to.p || x.from.o !== x.to.o).map((x) => [x.from, x.to]);
  if (!ed.collapsed()) return [[r[0], r[1]]];
  const w = wordAround(ed, r[0]);
  return w ? [w] : [];
}

/** Change the case of the selected text (the word at the caret without a selection), as one undo step. */
export function changeCase(ed: Editor, mode: CaseMode): boolean {
  const ranges = caseRanges(ed);
  if (!ranges.length) return false;
  ed.exec(
    ranges.map(([from, to]) => ({ op: "change_case", from, to, mode })),
    { keepExtra: true },
  );
  return true;
}

/** What Shift+F3 does to text that looks like this: lower case, then each word capitalised, then upper case, then lower again. */
export function nextCase(text: string): CaseMode {
  const letters = [...text].filter((c) => c.toLowerCase() !== c.toUpperCase());
  if (!letters.length) return "upper";
  if (letters.every((c) => c === c.toUpperCase())) return "lower";
  if (letters.every((c) => c === c.toLowerCase())) return "title";
  return "upper";
}

/** Shift+F3. */
export function cycleCase(ed: Editor): boolean {
  const ranges = caseRanges(ed);
  if (!ranges.length) return false;
  const text = ranges.map(([a, b]) => ed.engine.textRange(a, b)).join(" ");
  return changeCase(ed, nextCase(text));
}

/** Word's built-in character styles, listed even when a document does not define them yet (choosing one adds it). */
export const BUILTIN_CHAR_STYLES: { name: string; label: string; css: string }[] = [
  { name: "Strong", label: msg("Strong"), css: "font-weight:700" },
  { name: "Emphasis", label: msg("Emphasis"), css: "font-style:italic" },
  { name: "Subtle Emphasis", label: msg("Subtle emphasis"), css: "font-style:italic;color:var(--muted)" },
  { name: "Intense Emphasis", label: msg("Intense emphasis"), css: "font-style:italic;font-weight:600" },
  { name: "Subtle Reference", label: msg("Subtle reference"), css: "font-variant:small-caps;color:var(--muted)" },
  { name: "Intense Reference", label: msg("Intense reference"), css: "font-variant:small-caps;font-weight:700" },
  { name: "Book Title", label: msg("Book title"), css: "font-style:italic;font-weight:700" },
];

/** Whether the selection is part of a paragraph (not the whole of it, and not several): where a linked style applies as a character style. */
export function inPartOfParagraph(ed: Editor): boolean {
  const r = ed.range();
  if (!r || ed.collapsed() || r[0].p !== r[1].p) return false;
  return r[0].o > 0 || r[1].o < paragraphText(ed, r[0].p).length;
}

/**
 * Apply a style from the style picker. A character style goes on the selected text. A linked style (a heading and its
 * character twin) is a character style on part of a paragraph and a paragraph style otherwise, as in Word.
 */
export function applyStyle(ed: Editor, style: Pick<StyleInfo, "id" | "kind" | "link">) {
  if (style.kind === "character" || (style.kind === "paragraph" && style.link && inPartOfParagraph(ed))) ed.setRun({ style: style.id });
  else ed.setStyle(style.id);
}

/** Take the character style off the selected text (or off the next typed text, with only a caret). */
export function clearCharStyle(ed: Editor) {
  ed.setRun({ style: "" });
}
