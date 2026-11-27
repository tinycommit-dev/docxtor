// Which pages go on which side of which sheet when a document prints two pages to a sheet (Page setup, Multiple pages):
// "2 pages per sheet" puts pages one after another, two to a side; "Book fold" makes booklets to fold in the middle, so the
// pages read in order once the sheets are stacked and folded. Plain functions, tested in Node.

/** A side of a sheet: the page on its left (or top) and right (or bottom) half, from 0, or null for a blank half. */
export type SheetSide = [number | null, number | null];

export type Multiple = "twoUp" | "bookFold" | "bookFoldRev";

/**
 * The sides to print, in order, for `pages` pages. For a book fold, `sheetsPerBooklet` sheets make one booklet (0: all of
 * them in one); each sheet carries four pages, two on the front and two on the back, so a booklet's page count is
 * padded with blank pages to a multiple of four. `bookFoldRev` is for text read right to left: the halves swap.
 */
export function imposition(pages: number, mode: Multiple, sheetsPerBooklet = 0): SheetSide[] {
  const out: SheetSide[] = [];
  if (pages <= 0) return out;
  if (mode === "twoUp") {
    for (let k = 0; k < pages; k += 2) out.push([k, k + 1 < pages ? k + 1 : null]);
    return out;
  }
  const per = sheetsPerBooklet > 0 ? sheetsPerBooklet * 4 : Math.ceil(pages / 4) * 4;
  const page = (k: number) => (k < pages ? k : null);
  for (let start = 0; start < pages; start += per) {
    const n = Math.min(per, Math.ceil((pages - start) / 4) * 4);
    for (let i = 0; i < n / 4; i++) {
      // Front: the last page of the booklet still to place, and the first; back: the second, and the one before the last.
      const front: SheetSide = [page(start + n - 1 - 2 * i), page(start + 2 * i)];
      const back: SheetSide = [page(start + 2 * i + 1), page(start + n - 2 - 2 * i)];
      out.push(mode === "bookFoldRev" ? [front[1], front[0]] : front, mode === "bookFoldRev" ? [back[1], back[0]] : back);
    }
  }
  return out;
}

/** The page as laid out when a sheet holds two: half the paper, across a landscape sheet or down a portrait one. */
export function halfPage(w: number, h: number): { w: number; h: number } {
  return w >= h ? { w: w / 2, h } : { w, h: h / 2 };
}
