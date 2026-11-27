// The cells a table command works on: the ones the selection takes in, with what the engine knows about the table.
// Dialogs read this when they open, since a dialog takes the focus and the cell marks go.
import type { TableDetails } from "@docxtor/core";
import type { Editor } from "../editor/editor";

export interface CellSelection {
  /** Paragraphs in the two cells at the ends of the selection (the same cell when the caret is in one). */
  from: number;
  to: number;
  /** More than one cell is selected. */
  multi: boolean;
  /** The first and last row and grid column of the selection, counting from 0. */
  rows: [number, number];
  cols: [number, number];
  /** The table's details as seen from the first cell. */
  details: TableDetails;
}

const tableOf = (ed: Editor, p: number) => ed.root.querySelector(`[data-p="${p}"]`)?.closest("table.ft") ?? null;

/** The cells the selection takes in, or null when it is not inside one table. */
export function cellSelection(ed: Editor): CellSelection | null {
  const s = ed.sel;
  if (!s || !ed.engine) return null;
  const t = tableOf(ed, s.anchor.p);
  if (!t || t !== tableOf(ed, s.focus.p)) return null;
  const a = ed.engine.tableDetails(s.anchor.p);
  const b = ed.engine.tableDetails(s.focus.p);
  if (!a || !b) return null;
  const rows: [number, number] = [Math.min(a.row, b.row), Math.max(a.row, b.row)];
  const cols: [number, number] = [Math.min(a.col, b.col), Math.max(a.col + a.span - 1, b.col + b.span - 1)];
  return { from: s.anchor.p, to: s.focus.p, multi: a.row !== b.row || a.col !== b.col, rows, cols, details: a };
}

/** The rows of the rendered table holding paragraph `p`, in model order (spacers and repeated headers left out). */
export function tableRows(ed: Editor, p: number): HTMLTableRowElement[] {
  const t = tableOf(ed, p) as HTMLTableElement | null;
  return t ? Array.from(t.rows).filter((r) => !r.classList.contains("pgsp") && !r.classList.contains("hdr-repeat")) : [];
}

/** Make the selected rows (all of them when one row or less is selected) as tall as each other: every row at least the average height. */
export function distributeRows(ed: Editor): boolean {
  const sel = cellSelection(ed);
  if (!sel) return false;
  const rows = tableRows(ed, sel.from);
  const spread = sel.rows[1] > sel.rows[0];
  const [r0, r1] = spread ? sel.rows : [0, rows.length - 1];
  const picked = rows.slice(r0, r1 + 1);
  if (picked.length < 2) return false;
  const px = picked.reduce((sum, r) => sum + r.getBoundingClientRect().height / ed.zoom, 0) / picked.length;
  const height = { value: Math.round(px * 15), rule: "atLeast" };
  const op = spread ? { op: "table_rows", from: sel.from, to: sel.to, height } : { op: "table_rows", from: sel.from, all: true, height };
  return !!ed.exec(op);
}
