// Notes on the page: how their marks are numbered where a section starts them again on each page, and the
// separators the notes part may give for its own (a custom line, text, a notice that notes go on).
import type { NoteSep, RBlock, RNote } from "@docxtor/core";
import { pageLabel } from "./layout";

/** *, †, ‡, §, then the same doubled, tripled and so on (Word's `chicago` numbering). */
function symbols(n: number): string {
  const sym = ["*", "†", "‡", "§"];
  if (n < 1) return String(n);
  return sym[(n - 1) % 4].repeat(Math.floor((n - 1) / 4) + 1);
}

/** The mark of the nth note in an OOXML number format. */
export function noteLabel(n: number, fmt: string): string {
  if (fmt === "chicago") return symbols(n);
  return pageLabel(n, fmt);
}

/** A note drawn with another number: the mark in front of its text changes, nothing else. */
export function withLabel(n: RNote, label: string): RNote {
  if (n.label === label) return n;
  const blocks: RBlock[] = n.blocks.map((b, i) => {
    if (i !== 0 || b.t !== "p") return b;
    return { ...b, segs: b.segs.map((s) => (s.k === "note" && !s.x ? { ...s, label } : s)) };
  });
  return { ...n, label, blocks };
}

/** A separator the notes part has of its own, as the blocks to draw (null where Word's own line stands). */
export function customSep(sep: NoteSep | undefined): NoteSep | null {
  return sep ?? null;
}
